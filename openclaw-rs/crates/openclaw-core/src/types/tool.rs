//! Tool-related types.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

/// Tool groups for categorization and policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolGroup {
    /// File system tools.
    FileSystem,

    /// System/shell tools.
    System,

    /// Network/web tools.
    Web,

    /// Memory/retrieval tools.
    Memory,

    /// Session management tools.
    Session,

    /// UI/browser tools.
    Ui,

    /// Custom/plugin tools.
    Custom,
}

impl Default for ToolGroup {
    fn default() -> Self {
        Self::Custom
    }
}

/// Definition of a tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Tool name (unique identifier).
    pub name: String,

    /// Human-readable description.
    pub description: String,

    /// JSON Schema for input validation.
    pub input_schema: Value,

    /// Execution settings.
    #[serde(default)]
    pub execution: ToolExecutionConfig,
}

/// Tool execution configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolExecutionConfig {
    /// Where to execute this tool.
    #[serde(default)]
    pub host: ExecutionHost,

    /// Whether this tool requires approval.
    #[serde(default)]
    pub requires_approval: bool,

    /// Sandbox profile to use.
    #[serde(default)]
    pub sandbox_profile: super::SandboxProfile,
}

/// Where a tool should be executed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ExecutionHost {
    /// Execute in sandbox on gateway.
    #[default]
    Sandbox,

    /// Execute directly on gateway (no sandbox).
    Gateway,

    /// Execute on a remote node.
    Node { node_id: String },

    /// Execute in Docker container.
    Docker { container: String },
}

/// Result of tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// Tool use ID.
    pub tool_use_id: String,

    /// Output value.
    pub output: Value,

    /// Whether the result is an error.
    #[serde(default)]
    pub is_error: bool,

    /// Execution duration in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
}

impl ToolResult {
    /// Create a successful result.
    pub fn success(tool_use_id: impl Into<String>, output: Value) -> Self {
        Self {
            tool_use_id: tool_use_id.into(),
            output,
            is_error: false,
            duration_ms: None,
        }
    }

    /// Create an error result.
    pub fn error(tool_use_id: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            tool_use_id: tool_use_id.into(),
            output: Value::String(message.into()),
            is_error: true,
            duration_ms: None,
        }
    }

    /// Set the duration.
    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration_ms = Some(duration.as_millis() as u64);
        self
    }
}

/// Result of command execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    /// Exit code.
    pub exit_code: i32,

    /// Standard output.
    pub stdout: String,

    /// Standard error.
    pub stderr: String,

    /// Execution duration in milliseconds.
    pub duration_ms: u64,

    /// Resource usage.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_usage: Option<ResourceUsage>,
}

/// Resource usage during execution.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResourceUsage {
    /// CPU time in milliseconds.
    pub cpu_ms: u64,

    /// Peak memory in bytes.
    pub memory_bytes: u64,

    /// Bytes read.
    pub read_bytes: u64,

    /// Bytes written.
    pub write_bytes: u64,
}

/// Tool groups for policy configuration.
pub mod groups {
    /// Memory tools.
    pub const MEMORY: &[&str] = &["memory_search", "memory_get"];

    /// Web tools.
    pub const WEB: &[&str] = &["web_search", "web_fetch"];

    /// Filesystem tools.
    pub const FS: &[&str] = &["read", "write", "edit", "apply_patch", "glob", "grep"];

    /// Runtime/execution tools.
    pub const RUNTIME: &[&str] = &["exec", "process"];

    /// Session tools.
    pub const SESSIONS: &[&str] = &[
        "sessions_list",
        "sessions_history",
        "sessions_send",
        "sessions_spawn",
        "session_status",
    ];

    /// UI tools.
    pub const UI: &[&str] = &["browser", "canvas"];

    /// Automation tools.
    pub const AUTOMATION: &[&str] = &["cron", "gateway"];

    /// Messaging tools.
    pub const MESSAGING: &[&str] = &["message"];

    /// Node tools.
    pub const NODES: &[&str] = &["nodes"];

    /// Get tools in a group by name.
    pub fn get_group(name: &str) -> Option<&'static [&'static str]> {
        match name {
            "group:memory" => Some(MEMORY),
            "group:web" => Some(WEB),
            "group:fs" => Some(FS),
            "group:runtime" => Some(RUNTIME),
            "group:sessions" => Some(SESSIONS),
            "group:ui" => Some(UI),
            "group:automation" => Some(AUTOMATION),
            "group:messaging" => Some(MESSAGING),
            "group:nodes" => Some(NODES),
            _ => None,
        }
    }
}

/// Default tools denied for subagents.
pub const DEFAULT_SUBAGENT_TOOL_DENY: &[&str] = &[
    "sessions_spawn",    // No nested spawning
    "sessions_list",     // Parent orchestrates
    "sessions_history",  // Parent orchestrates
    "sessions_send",     // Parent sends messages
    "gateway",           // System admin
    "agents_list",       // System admin
    "memory_search",     // Pass info in prompt
    "memory_get",        // Pass info in prompt
    "cron",              // No scheduling
    "session_status",    // Parent tracks status
];
