//! Agent configuration types.

use super::AgentId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Agent configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    /// Agent ID (normalized).
    pub id: AgentId,

    /// Display name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Workspace directory.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_dir: Option<PathBuf>,

    /// Primary model.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// Fallback models.
    #[serde(default)]
    pub fallback_models: Vec<String>,

    /// System prompt override.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,

    /// Thinking level.
    #[serde(default)]
    pub thinking_level: ThinkingLevel,

    /// Tool policy.
    #[serde(default)]
    pub tools: ToolPolicyConfig,

    /// Sandbox configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<SandboxConfig>,

    /// Subagent settings.
    #[serde(default)]
    pub subagents: SubagentConfig,

    /// Identity (name, emoji, avatar).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identity: Option<AgentIdentity>,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            id: AgentId::default(),
            name: None,
            workspace_dir: None,
            model: None,
            fallback_models: Vec::new(),
            system_prompt: None,
            thinking_level: ThinkingLevel::default(),
            tools: ToolPolicyConfig::default(),
            sandbox: None,
            subagents: SubagentConfig::default(),
            identity: None,
        }
    }
}

/// Extended thinking level.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThinkingLevel {
    Off,
    Minimal,
    #[default]
    Low,
    Medium,
    High,
    XHigh,
}

impl ThinkingLevel {
    /// Get the token budget for this thinking level.
    pub fn budget_tokens(&self) -> Option<usize> {
        match self {
            Self::Off => None,
            Self::Minimal => Some(1024),
            Self::Low => Some(4096),
            Self::Medium => Some(8192),
            Self::High => Some(16384),
            Self::XHigh => Some(32768),
        }
    }

    /// Check if thinking is enabled.
    pub fn is_enabled(&self) -> bool {
        !matches!(self, Self::Off)
    }
}

/// Tool policy configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolPolicyConfig {
    /// Tool profile.
    #[serde(default)]
    pub profile: ToolProfile,

    /// Explicit allow patterns.
    #[serde(default)]
    pub allow: Vec<String>,

    /// Explicit deny patterns.
    #[serde(default)]
    pub deny: Vec<String>,

    /// Additional allow (union with profile).
    #[serde(default)]
    pub also_allow: Vec<String>,
}

/// Tool profile presets.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolProfile {
    /// Only session_status.
    Minimal,

    /// Filesystem, runtime, sessions, memory.
    Coding,

    /// Messaging with limited sessions.
    Messaging,

    /// All tools allowed.
    #[default]
    Full,
}

impl ToolProfile {
    /// Get the tools included in this profile.
    pub fn included_tools(&self) -> &'static [&'static str] {
        match self {
            Self::Minimal => &["session_status"],
            Self::Coding => &[
                "read", "write", "edit", "glob", "grep", "apply_patch",
                "exec", "process",
                "sessions_list", "sessions_history", "sessions_send", "sessions_spawn",
                "memory_search", "memory_get",
                "image",
            ],
            Self::Messaging => &[
                "message",
                "sessions_list", "sessions_send",
            ],
            Self::Full => &[], // All tools allowed
        }
    }

    /// Check if this profile allows all tools.
    pub fn allows_all(&self) -> bool {
        matches!(self, Self::Full)
    }
}

/// Sandbox configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxConfig {
    /// Whether sandbox is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Sandbox profile.
    #[serde(default)]
    pub profile: SandboxProfile,

    /// Docker container name (if using Docker).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_name: Option<String>,

    /// Workspace directory in container.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_workdir: Option<PathBuf>,

    /// Resource limits.
    #[serde(default)]
    pub limits: ResourceLimits,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            profile: SandboxProfile::default(),
            container_name: None,
            container_workdir: None,
            limits: ResourceLimits::default(),
        }
    }
}

/// Sandbox security profile.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SandboxProfile {
    /// Maximum isolation.
    Strict,

    /// Standard isolation.
    #[default]
    Standard,

    /// Relaxed for trusted tools.
    Trusted,

    /// No sandbox (requires approval).
    None,
}

/// Resource limits for sandboxed execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// Maximum CPU seconds.
    pub max_cpu_seconds: u64,

    /// Maximum memory in bytes.
    pub max_memory_bytes: u64,

    /// Maximum number of processes.
    pub max_processes: u32,

    /// Maximum open files.
    pub max_open_files: u64,

    /// Maximum output bytes.
    pub max_output_bytes: u64,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_cpu_seconds: 30,
            max_memory_bytes: 512 * 1024 * 1024, // 512MB
            max_processes: 10,
            max_open_files: 100,
            max_output_bytes: 1024 * 1024, // 1MB
        }
    }
}

/// Subagent configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SubagentConfig {
    /// Allowed agent IDs that can be spawned.
    #[serde(default)]
    pub allow_agents: Vec<String>,

    /// Default model for spawned subagents.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// Default thinking level for subagents.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<ThinkingLevel>,

    /// Tool policy overrides for subagents.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_policy: Option<ToolPolicyConfig>,
}

/// Agent identity for display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentIdentity {
    /// Display name.
    pub name: String,

    /// Emoji identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub emoji: Option<String>,

    /// Avatar URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,

    /// Theme color.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme_color: Option<String>,
}

/// Workspace files loaded for an agent.
#[derive(Debug, Clone, Default)]
pub struct Workspace {
    /// Path to workspace directory.
    pub path: PathBuf,

    /// AGENTS.md content.
    pub agents_md: Option<String>,

    /// SOUL.md content.
    pub soul_md: Option<String>,

    /// TOOLS.md content.
    pub tools_md: Option<String>,

    /// IDENTITY.md content.
    pub identity_md: Option<String>,

    /// Additional markdown files.
    pub additional_files: HashMap<String, String>,
}

impl Workspace {
    /// Build system prompt from workspace files.
    pub fn build_system_prompt(&self) -> String {
        let mut prompt = String::new();

        if let Some(agents) = &self.agents_md {
            prompt.push_str(agents);
            prompt.push_str("\n\n");
        }

        if let Some(soul) = &self.soul_md {
            prompt.push_str(soul);
            prompt.push_str("\n\n");
        }

        if let Some(tools) = &self.tools_md {
            prompt.push_str(tools);
        }

        prompt
    }
}

fn default_true() -> bool {
    true
}
