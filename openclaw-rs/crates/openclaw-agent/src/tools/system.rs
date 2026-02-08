//! System execution tools.
//!
//! - [`BashTool`] - Execute shell commands

use super::{Tool, ToolContext};
use crate::error::AgentError;
use crate::Result;
use async_trait::async_trait;
use openclaw_core::types::{ToolDefinition, ToolExecutionConfig, ToolGroup, ToolResult};
use openclaw_sandbox::{CommandExecutor, ExecutionContext};
use regex::Regex;
use std::time::Instant;
use tracing::warn;

/// Bash tool - Execute shell commands with sandboxing.
pub struct BashTool {
    /// Allowed commands (regex patterns).
    allowed_patterns: Vec<String>,

    /// Blocked commands (regex patterns).
    blocked_patterns: Vec<String>,

    /// Compiled blocked regexes.
    blocked_regexes: Vec<Regex>,
}

impl Default for BashTool {
    fn default() -> Self {
        Self::new()
    }
}

impl BashTool {
    /// Create a new Bash tool with default security patterns.
    pub fn new() -> Self {
        let blocked_patterns = vec![
            r"^rm\s+-rf\s+/".to_string(),
            r"^sudo\s+".to_string(),
            r"^chmod\s+777".to_string(),
            r">\s*/dev/".to_string(),
            r"mkfs".to_string(),
            r"dd\s+if=".to_string(),
        ];

        let blocked_regexes = blocked_patterns
            .iter()
            .filter_map(|p| Regex::new(p).ok())
            .collect();

        Self {
            allowed_patterns: vec![],
            blocked_patterns,
            blocked_regexes,
        }
    }

    /// Add an allowed pattern.
    pub fn allow(mut self, pattern: impl Into<String>) -> Self {
        self.allowed_patterns.push(pattern.into());
        self
    }

    /// Add a blocked pattern.
    pub fn block(mut self, pattern: impl Into<String>) -> Self {
        let pattern_str = pattern.into();
        if let Ok(re) = Regex::new(&pattern_str) {
            self.blocked_regexes.push(re);
        }
        self.blocked_patterns.push(pattern_str);
        self
    }

    /// Check if a command is blocked.
    fn is_blocked(&self, command: &str) -> bool {
        for re in &self.blocked_regexes {
            if re.is_match(command) {
                warn!("Blocked command: {}", command);
                return true;
            }
        }
        false
    }

    /// Check if command matches dangerous patterns requiring approval.
    fn is_dangerous(&self, command: &str) -> bool {
        let dangerous_patterns = [
            "rm ", "rmdir", "mv ", "cp ", "> ", ">> ",
            "curl ", "wget ", "pip install", "npm install",
            "chmod", "chown", "kill ", "pkill",
            "git push", "git reset",
        ];

        for pattern in &dangerous_patterns {
            if command.contains(pattern) {
                return true;
            }
        }
        false
    }
}

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str {
        "bash"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "bash".to_string(),
            description: "Execute a bash command in a sandboxed environment".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The bash command to execute"
                    },
                    "timeout": {
                        "type": "integer",
                        "description": "Timeout in seconds (default: 120, max: 600)"
                    },
                    "cwd": {
                        "type": "string",
                        "description": "Working directory for the command"
                    }
                },
                "required": ["command"]
            }),
            execution: ToolExecutionConfig::default(),
        }
    }

    async fn execute(
        &self,
        tool_use_id: &str,
        args: serde_json::Value,
        context: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();

        let command = args
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AgentError::tool_execution("Missing 'command' argument"))?;

        // Check if command is blocked
        if self.is_blocked(command) {
            return Ok(ToolResult::error(
                tool_use_id,
                "Command is blocked by security policy",
            ));
        }

        // Determine working directory
        let cwd = args
            .get("cwd")
            .and_then(|v| v.as_str())
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| context.cwd.clone());

        // Set up execution context
        let exec_context = ExecutionContext::new(&cwd)
            .with_profile(context.sandbox_profile.clone())
            .with_envs(context.env.clone());

        let executor = CommandExecutor::new(exec_context);

        // Get timeout (default 120s, max 600s)
        let timeout = args
            .get("timeout")
            .and_then(|v| v.as_u64())
            .map(|t| t.min(600))
            .unwrap_or(120);

        // Execute command
        let output = executor.execute_with_timeout(command, Some(timeout)).await?;
        let duration = start.elapsed();

        let result_output = serde_json::json!({
            "stdout": output.stdout,
            "stderr": output.stderr,
            "exit_code": output.exit_code,
            "timed_out": output.timed_out,
            "duration_ms": duration.as_millis() as u64,
        });

        if output.success() {
            Ok(ToolResult::success(tool_use_id, result_output).with_duration(duration))
        } else {
            Ok(ToolResult {
                tool_use_id: tool_use_id.to_string(),
                output: result_output,
                is_error: true,
                duration_ms: Some(duration.as_millis() as u64),
            })
        }
    }

    fn requires_approval(&self, args: &serde_json::Value) -> bool {
        if let Some(command) = args.get("command").and_then(|v| v.as_str()) {
            self.is_dangerous(command)
        } else {
            true // Require approval if we can't parse the command
        }
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::System
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bash_tool_creation() {
        let tool = BashTool::new();
        assert_eq!(tool.name(), "bash");
    }

    #[test]
    fn test_bash_tool_blocked() {
        let tool = BashTool::new();

        assert!(tool.is_blocked("rm -rf /"));
        assert!(tool.is_blocked("sudo rm -rf /"));
        assert!(!tool.is_blocked("ls -la"));
        assert!(!tool.is_blocked("echo hello"));
    }

    #[test]
    fn test_bash_tool_dangerous() {
        let tool = BashTool::new();

        assert!(tool.is_dangerous("rm -rf ./build"));
        assert!(tool.is_dangerous("curl http://example.com"));
        assert!(tool.is_dangerous("git push origin main"));
        assert!(!tool.is_dangerous("ls -la"));
        assert!(!tool.is_dangerous("cat file.txt"));
    }

    #[test]
    fn test_bash_tool_requires_approval() {
        let tool = BashTool::new();

        assert!(tool.requires_approval(&serde_json::json!({ "command": "rm -rf ./build" })));
        assert!(tool.requires_approval(&serde_json::json!({ "command": "git push" })));
        assert!(!tool.requires_approval(&serde_json::json!({ "command": "ls -la" })));
    }

    #[test]
    fn test_custom_blocked_patterns() {
        let tool = BashTool::new()
            .block(r"^docker\s+")
            .block(r"^kubectl\s+");

        assert!(tool.is_blocked("docker run"));
        assert!(tool.is_blocked("kubectl delete"));
    }
}
