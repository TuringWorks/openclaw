//! Tool execution framework and built-in tools.
//!
//! This module provides:
//! - [`Tool`] trait for implementing tools
//! - [`ToolRegistry`] for managing available tools
//! - [`ToolExecutor`] for executing tools with sandboxing
//! - Built-in tools for file system, execution, and more

mod ask;
mod automation;
mod browser;
mod channel_actions;
mod diagnostic;
mod filesystem;
mod lsp;
mod media;
mod memory;
mod messaging;
mod notebook;
mod plan;
mod skill;
mod system;
mod tasks;
mod web;

pub use ask::{AskUserTool, ConfirmTool};
pub use automation::{CronTool, GatewayTool, NodesTool};
pub use browser::BrowserTool;
pub use channel_actions::{DiscordActionsTool, SlackActionsTool, TelegramActionsTool};
pub use diagnostic::{DiagnosticTool, HealthCheckTool, SystemInfoTool};
pub use filesystem::{EditTool, GlobTool, GrepTool, ReadTool, WriteTool};
pub use lsp::LspTool;
pub use media::{ImageTool, TtsTool};
pub use memory::{MemoryGetTool, MemorySearchTool};
pub use messaging::{
    MessageTool, SessionStatusTool, SessionsHistoryTool, SessionsListTool, SessionsSendTool,
    SessionsSpawnTool,
};
pub use notebook::NotebookEditTool;
pub use plan::{EnterPlanModeTool, ExitPlanModeTool, PlanState, SharedPlanState};
pub use skill::{Skill, SkillListTool, SkillRegistry, SkillTool, SharedSkillRegistry};
pub use system::BashTool;
pub use tasks::{TaskCreateTool, TaskGetTool, TaskListTool, TaskStore, TaskUpdateTool};
pub use web::{WebFetchTool, WebSearchTool};

use crate::error::AgentError;
use crate::Result;
use async_trait::async_trait;
use openclaw_core::types::{ToolDefinition, ToolGroup, ToolResult};
use openclaw_sandbox::{CommandExecutor, ExecutionContext, SandboxProfile};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tracing::debug;

/// A tool that can be executed by an agent.
#[async_trait]
pub trait Tool: Send + Sync {
    /// Get the tool name.
    fn name(&self) -> &str;

    /// Get the tool definition for the model.
    fn definition(&self) -> ToolDefinition;

    /// Execute the tool with given arguments.
    async fn execute(
        &self,
        tool_use_id: &str,
        args: serde_json::Value,
        context: &ToolContext,
    ) -> Result<ToolResult>;

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

    /// Create a registry with default tools.
    pub async fn with_defaults() -> Self {
        let registry = Self::new();

        // File system tools
        registry.register(Arc::new(ReadTool)).await;
        registry.register(Arc::new(WriteTool)).await;
        registry.register(Arc::new(EditTool::new())).await;
        registry.register(Arc::new(GlobTool::new())).await;
        registry.register(Arc::new(GrepTool::new())).await;

        // System tools
        registry.register(Arc::new(BashTool::new())).await;

        // Web tools
        registry.register(Arc::new(WebFetchTool::new())).await;
        registry.register(Arc::new(WebSearchTool::new())).await;

        // Messaging tools
        registry.register(Arc::new(MessageTool::new())).await;
        registry.register(Arc::new(SessionsSpawnTool)).await;
        registry.register(Arc::new(SessionsSendTool)).await;
        registry.register(Arc::new(SessionsListTool)).await;
        registry.register(Arc::new(SessionsHistoryTool)).await;
        registry.register(Arc::new(SessionStatusTool)).await;

        // Memory tools
        registry.register(Arc::new(MemorySearchTool::new())).await;
        registry.register(Arc::new(MemoryGetTool::new())).await;

        // Automation tools
        registry.register(Arc::new(CronTool::new())).await;
        registry.register(Arc::new(GatewayTool::new())).await;
        registry.register(Arc::new(NodesTool::new())).await;

        // Media tools
        registry.register(Arc::new(ImageTool::new())).await;
        registry.register(Arc::new(TtsTool::new())).await;

        // Browser tools
        registry.register(Arc::new(BrowserTool::new())).await;

        // Channel action tools
        registry.register(Arc::new(TelegramActionsTool::new())).await;
        registry.register(Arc::new(DiscordActionsTool::new())).await;
        registry.register(Arc::new(SlackActionsTool::new())).await;

        // Notebook tools
        registry.register(Arc::new(NotebookEditTool::new())).await;

        // LSP tools
        registry.register(Arc::new(LspTool::new())).await;

        // Task tools (shared store)
        let task_store = Arc::new(TaskStore::new());
        registry.register(Arc::new(TaskCreateTool::new(task_store.clone()))).await;
        registry.register(Arc::new(TaskListTool::new(task_store.clone()))).await;
        registry.register(Arc::new(TaskUpdateTool::new(task_store.clone()))).await;
        registry.register(Arc::new(TaskGetTool::new(task_store))).await;

        // Interactive tools
        registry.register(Arc::new(AskUserTool::new())).await;
        registry.register(Arc::new(ConfirmTool::new())).await;

        // Planning tools (shared state)
        let plan_state = Arc::new(tokio::sync::RwLock::new(PlanState::default()));
        registry.register(Arc::new(EnterPlanModeTool::new(plan_state.clone()))).await;
        registry.register(Arc::new(ExitPlanModeTool::new(plan_state))).await;

        // Skill tools (shared registry)
        let skill_registry = Arc::new(tokio::sync::RwLock::new(SkillRegistry::with_defaults()));
        registry.register(Arc::new(SkillTool::new(skill_registry.clone()))).await;
        registry.register(Arc::new(SkillListTool::new(skill_registry))).await;

        // Diagnostic tools
        registry.register(Arc::new(SystemInfoTool::new())).await;
        registry.register(Arc::new(HealthCheckTool::new())).await;
        registry.register(Arc::new(DiagnosticTool::new())).await;

        registry
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
    pub async fn execute_command(
        &self,
        command: &str,
    ) -> Result<openclaw_core::types::ExecutionResult> {
        let executor = self
            .command_executor
            .as_ref()
            .ok_or_else(|| AgentError::config("Command executor not configured"))?;

        let start = Instant::now();
        let output = executor.execute(command).await?;
        let duration_ms = start.elapsed().as_millis() as u64;

        Ok(openclaw_core::types::ExecutionResult {
            exit_code: output.exit_code,
            stdout: output.stdout,
            stderr: output.stderr,
            duration_ms,
            resource_usage: None,
        })
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
    async fn test_registry_with_defaults() {
        let registry = ToolRegistry::with_defaults().await;
        let tools = registry.list().await;

        // Check file system tools
        assert!(tools.contains(&"read".to_string()));
        assert!(tools.contains(&"write".to_string()));
        assert!(tools.contains(&"edit".to_string()));
        assert!(tools.contains(&"glob".to_string()));
        assert!(tools.contains(&"grep".to_string()));

        // Check system tools
        assert!(tools.contains(&"bash".to_string()));

        // Check web tools
        assert!(tools.contains(&"web_fetch".to_string()));
        assert!(tools.contains(&"web_search".to_string()));

        // Check messaging tools
        assert!(tools.contains(&"message".to_string()));
        assert!(tools.contains(&"sessions_spawn".to_string()));

        // Check memory tools
        assert!(tools.contains(&"memory_search".to_string()));
        assert!(tools.contains(&"memory_get".to_string()));

        // Check automation tools
        assert!(tools.contains(&"cron".to_string()));
        assert!(tools.contains(&"gateway".to_string()));
        assert!(tools.contains(&"nodes".to_string()));

        // Check media tools
        assert!(tools.contains(&"image".to_string()));
        assert!(tools.contains(&"tts".to_string()));

        // Check browser tools
        assert!(tools.contains(&"browser".to_string()));

        // Check channel action tools
        assert!(tools.contains(&"telegram_actions".to_string()));
        assert!(tools.contains(&"discord_actions".to_string()));
        assert!(tools.contains(&"slack_actions".to_string()));

        // Check notebook tools
        assert!(tools.contains(&"notebook_edit".to_string()));

        // Check LSP tools
        assert!(tools.contains(&"lsp".to_string()));

        // Check task tools
        assert!(tools.contains(&"task_create".to_string()));
        assert!(tools.contains(&"task_list".to_string()));
        assert!(tools.contains(&"task_update".to_string()));
        assert!(tools.contains(&"task_get".to_string()));

        // Check interactive tools
        assert!(tools.contains(&"ask_user".to_string()));
        assert!(tools.contains(&"confirm".to_string()));

        // Check planning tools
        assert!(tools.contains(&"enter_plan_mode".to_string()));
        assert!(tools.contains(&"exit_plan_mode".to_string()));

        // Check skill tools
        assert!(tools.contains(&"skill".to_string()));
        assert!(tools.contains(&"skill_list".to_string()));

        // Check diagnostic tools
        assert!(tools.contains(&"system_info".to_string()));
        assert!(tools.contains(&"health_check".to_string()));
        assert!(tools.contains(&"diagnostic".to_string()));

        // Total: 40 tools
        assert_eq!(tools.len(), 40);
    }
}
