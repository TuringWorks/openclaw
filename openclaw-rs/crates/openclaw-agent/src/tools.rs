//! Tool execution and registry.

use crate::error::AgentError;
use crate::Result;
use async_trait::async_trait;
use openclaw_core::types::{
    ExecutionResult, ToolDefinition, ToolExecutionConfig, ToolGroup, ToolResult,
};
use openclaw_sandbox::{CommandExecutor, ExecutionContext, SandboxProfile};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tracing::{debug, warn};

/// A tool that can be executed by an agent.
#[async_trait]
pub trait Tool: Send + Sync {
    /// Get the tool name.
    fn name(&self) -> &str;

    /// Get the tool definition for the model.
    fn definition(&self) -> ToolDefinition;

    /// Execute the tool with given arguments.
    async fn execute(&self, tool_use_id: &str, args: serde_json::Value, context: &ToolContext) -> Result<ToolResult>;

    /// Check if the tool requires approval.
    fn requires_approval(&self, _args: &serde_json::Value) -> bool {
        false
    }

    /// Get the tool group.
    fn group(&self) -> ToolGroup {
        ToolGroup::Custom
    }
}

/// Context for tool execution.
#[derive(Debug, Clone)]
pub struct ToolContext {
    /// Working directory.
    pub cwd: std::path::PathBuf,

    /// Environment variables.
    pub env: HashMap<String, String>,

    /// Session ID.
    pub session_id: String,

    /// Agent ID.
    pub agent_id: String,

    /// Sandbox profile.
    pub sandbox_profile: SandboxProfile,

    /// Additional context data.
    pub data: HashMap<String, serde_json::Value>,
}

impl Default for ToolContext {
    fn default() -> Self {
        Self {
            cwd: std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/")),
            env: std::env::vars().collect(),
            session_id: String::new(),
            agent_id: String::new(),
            sandbox_profile: SandboxProfile::standard(),
            data: HashMap::new(),
        }
    }
}

/// Registry for available tools.
pub struct ToolRegistry {
    /// Registered tools by name.
    tools: RwLock<HashMap<String, Arc<dyn Tool>>>,

    /// Tool groups.
    groups: RwLock<HashMap<ToolGroup, Vec<String>>>,
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRegistry {
    /// Create a new tool registry.
    pub fn new() -> Self {
        Self {
            tools: RwLock::new(HashMap::new()),
            groups: RwLock::new(HashMap::new()),
        }
    }

    /// Register a tool.
    pub async fn register(&self, tool: Arc<dyn Tool>) {
        let name = tool.name().to_string();
        let group = tool.group();

        let mut tools = self.tools.write().await;
        tools.insert(name.clone(), tool);

        let mut groups = self.groups.write().await;
        groups.entry(group).or_default().push(name);
    }

    /// Unregister a tool.
    pub async fn unregister(&self, name: &str) {
        let mut tools = self.tools.write().await;
        if let Some(tool) = tools.remove(name) {
            let group = tool.group();
            let mut groups = self.groups.write().await;
            if let Some(group_tools) = groups.get_mut(&group) {
                group_tools.retain(|n| n != name);
            }
        }
    }

    /// Get a tool by name.
    pub async fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        let tools = self.tools.read().await;
        tools.get(name).cloned()
    }

    /// List all tool names.
    pub async fn list(&self) -> Vec<String> {
        let tools = self.tools.read().await;
        tools.keys().cloned().collect()
    }

    /// List tools in a group.
    pub async fn list_group(&self, group: ToolGroup) -> Vec<String> {
        let groups = self.groups.read().await;
        groups.get(&group).cloned().unwrap_or_default()
    }

    /// Get all tool definitions.
    pub async fn definitions(&self) -> Vec<ToolDefinition> {
        let tools = self.tools.read().await;
        tools.values().map(|t| t.definition()).collect()
    }

    /// Get tool definitions for specific groups.
    pub async fn definitions_for_groups(&self, target_groups: &[ToolGroup]) -> Vec<ToolDefinition> {
        let tools = self.tools.read().await;
        tools
            .values()
            .filter(|t| target_groups.contains(&t.group()))
            .map(|t| t.definition())
            .collect()
    }
}

/// Tool executor with sandbox support.
pub struct ToolExecutor {
    /// Tool registry.
    registry: Arc<ToolRegistry>,

    /// Default execution context.
    default_context: ToolContext,

    /// Command executor for shell tools.
    command_executor: Option<CommandExecutor>,
}

impl ToolExecutor {
    /// Create a new tool executor.
    pub fn new(registry: Arc<ToolRegistry>) -> Self {
        Self {
            registry,
            default_context: ToolContext::default(),
            command_executor: None,
        }
    }

    /// Set the default context.
    pub fn with_context(mut self, context: ToolContext) -> Self {
        self.default_context = context;
        self
    }

    /// Set up command executor with sandbox.
    pub fn with_sandbox(mut self, profile: SandboxProfile) -> Self {
        let exec_context = ExecutionContext::new(&self.default_context.cwd)
            .with_profile(profile)
            .with_envs(self.default_context.env.clone());
        self.command_executor = Some(CommandExecutor::new(exec_context));
        self
    }

    /// Execute a tool by name.
    pub async fn execute(
        &self,
        tool_use_id: &str,
        name: &str,
        args: serde_json::Value,
        context: Option<&ToolContext>,
    ) -> Result<ToolResult> {
        let tool = self
            .registry
            .get(name)
            .await
            .ok_or_else(|| AgentError::ToolNotFound(name.to_string()))?;

        let ctx = context.unwrap_or(&self.default_context);

        debug!("Executing tool '{}' with args: {:?}", name, args);
        tool.execute(tool_use_id, args, ctx).await
    }

    /// Check if a tool requires approval.
    pub async fn requires_approval(&self, name: &str, args: &serde_json::Value) -> Result<bool> {
        let tool = self
            .registry
            .get(name)
            .await
            .ok_or_else(|| AgentError::ToolNotFound(name.to_string()))?;

        Ok(tool.requires_approval(args))
    }

    /// Execute a shell command (with sandboxing).
    pub async fn execute_command(&self, command: &str) -> Result<ExecutionResult> {
        let executor = self
            .command_executor
            .as_ref()
            .ok_or_else(|| AgentError::config("Command executor not configured"))?;

        let start = Instant::now();
        let output = executor.execute(command).await?;
        let duration_ms = start.elapsed().as_millis() as u64;

        Ok(ExecutionResult {
            exit_code: output.exit_code,
            stdout: output.stdout,
            stderr: output.stderr,
            duration_ms,
            resource_usage: None,
        })
    }
}

/// Built-in Bash tool.
pub struct BashTool {
    /// Allowed commands (regex patterns).
    allowed_patterns: Vec<String>,

    /// Blocked commands (regex patterns).
    blocked_patterns: Vec<String>,
}

impl Default for BashTool {
    fn default() -> Self {
        Self::new()
    }
}

impl BashTool {
    /// Create a new Bash tool.
    pub fn new() -> Self {
        Self {
            allowed_patterns: vec![],
            blocked_patterns: vec![
                r"^rm\s+-rf\s+/".to_string(),
                r"^sudo\s+".to_string(),
                r"^chmod\s+777".to_string(),
            ],
        }
    }

    /// Add an allowed pattern.
    pub fn allow(mut self, pattern: impl Into<String>) -> Self {
        self.allowed_patterns.push(pattern.into());
        self
    }

    /// Add a blocked pattern.
    pub fn block(mut self, pattern: impl Into<String>) -> Self {
        self.blocked_patterns.push(pattern.into());
        self
    }

    /// Check if a command is blocked.
    fn is_blocked(&self, command: &str) -> bool {
        for pattern in &self.blocked_patterns {
            if let Ok(re) = regex::Regex::new(pattern) {
                if re.is_match(command) {
                    warn!("Blocked command: {}", command);
                    return true;
                }
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
            description: "Execute a bash command".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The bash command to execute"
                    },
                    "timeout": {
                        "type": "integer",
                        "description": "Timeout in seconds (optional)"
                    }
                },
                "required": ["command"]
            }),
            execution: ToolExecutionConfig::default(),
        }
    }

    async fn execute(&self, tool_use_id: &str, args: serde_json::Value, context: &ToolContext) -> Result<ToolResult> {
        let start = Instant::now();

        let command = args
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AgentError::tool_execution("Missing 'command' argument"))?;

        if self.is_blocked(command) {
            return Ok(ToolResult::error(tool_use_id, "Command is blocked by security policy"));
        }

        let exec_context = ExecutionContext::new(&context.cwd)
            .with_profile(context.sandbox_profile.clone())
            .with_envs(context.env.clone());

        let executor = CommandExecutor::new(exec_context);

        let timeout = args
            .get("timeout")
            .and_then(|v| v.as_u64());

        let output = executor.execute_with_timeout(command, timeout).await?;
        let duration = start.elapsed();

        let result_output = serde_json::json!({
            "stdout": output.stdout,
            "stderr": output.stderr,
            "exit_code": output.exit_code,
            "timed_out": output.timed_out,
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
        // Require approval for potentially dangerous commands
        if let Some(command) = args.get("command").and_then(|v| v.as_str()) {
            let dangerous_patterns = [
                "rm ", "rmdir", "mv ", "cp ", "> ", ">> ",
                "curl ", "wget ", "pip install", "npm install",
                "chmod", "chown", "kill ", "pkill",
            ];

            for pattern in &dangerous_patterns {
                if command.contains(pattern) {
                    return true;
                }
            }
        }
        false
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::System
    }
}

/// Built-in Read tool for reading files.
pub struct ReadTool;

#[async_trait]
impl Tool for ReadTool {
    fn name(&self) -> &str {
        "read"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "read".to_string(),
            description: "Read the contents of a file".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The path to the file to read"
                    },
                    "offset": {
                        "type": "integer",
                        "description": "Line offset to start reading from (optional)"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of lines to read (optional)"
                    }
                },
                "required": ["path"]
            }),
            execution: ToolExecutionConfig::default(),
        }
    }

    async fn execute(&self, tool_use_id: &str, args: serde_json::Value, context: &ToolContext) -> Result<ToolResult> {
        let start = Instant::now();

        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AgentError::tool_execution("Missing 'path' argument"))?;

        let full_path = if std::path::Path::new(path).is_absolute() {
            std::path::PathBuf::from(path)
        } else {
            context.cwd.join(path)
        };

        let content = tokio::fs::read_to_string(&full_path).await.map_err(|e| {
            AgentError::tool_execution(format!("Failed to read file: {}", e))
        })?;

        let offset = args.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let limit = args.get("limit").and_then(|v| v.as_u64()).map(|v| v as usize);

        let lines: Vec<&str> = content.lines().skip(offset).collect();
        let lines = match limit {
            Some(l) => &lines[..l.min(lines.len())],
            None => &lines[..],
        };

        // Add line numbers
        let numbered: Vec<String> = lines
            .iter()
            .enumerate()
            .map(|(i, line)| format!("{:>6}\t{}", offset + i + 1, line))
            .collect();

        let duration = start.elapsed();
        Ok(ToolResult::success(tool_use_id, serde_json::json!({
            "content": numbered.join("\n"),
            "lines": lines.len(),
        })).with_duration(duration))
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::FileSystem
    }
}

/// Built-in Write tool for writing files.
pub struct WriteTool;

#[async_trait]
impl Tool for WriteTool {
    fn name(&self) -> &str {
        "write"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "write".to_string(),
            description: "Write content to a file".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The path to the file to write"
                    },
                    "content": {
                        "type": "string",
                        "description": "The content to write to the file"
                    }
                },
                "required": ["path", "content"]
            }),
            execution: ToolExecutionConfig::default(),
        }
    }

    async fn execute(&self, tool_use_id: &str, args: serde_json::Value, context: &ToolContext) -> Result<ToolResult> {
        let start = Instant::now();

        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AgentError::tool_execution("Missing 'path' argument"))?;

        let content = args
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AgentError::tool_execution("Missing 'content' argument"))?;

        let full_path = if std::path::Path::new(path).is_absolute() {
            std::path::PathBuf::from(path)
        } else {
            context.cwd.join(path)
        };

        // Ensure parent directory exists
        if let Some(parent) = full_path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                AgentError::tool_execution(format!("Failed to create directory: {}", e))
            })?;
        }

        tokio::fs::write(&full_path, content).await.map_err(|e| {
            AgentError::tool_execution(format!("Failed to write file: {}", e))
        })?;

        let duration = start.elapsed();
        Ok(ToolResult::success(tool_use_id, serde_json::json!({
            "path": full_path.to_string_lossy(),
            "bytes_written": content.len(),
        })).with_duration(duration))
    }

    fn requires_approval(&self, _args: &serde_json::Value) -> bool {
        true // File writes should require approval
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::FileSystem
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_tool_registry() {
        let registry = ToolRegistry::new();

        registry.register(Arc::new(BashTool::new())).await;
        registry.register(Arc::new(ReadTool)).await;

        let tools = registry.list().await;
        assert!(tools.contains(&"bash".to_string()));
        assert!(tools.contains(&"read".to_string()));
    }

    #[tokio::test]
    async fn test_bash_tool_blocked() {
        let tool = BashTool::new();

        assert!(tool.is_blocked("rm -rf /"));
        assert!(tool.is_blocked("sudo rm -rf /"));
        assert!(!tool.is_blocked("ls -la"));
    }
}
