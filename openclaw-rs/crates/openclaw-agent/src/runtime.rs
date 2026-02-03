//! Agent runtime for executing conversations.

use crate::approval::ApprovalManager;
use crate::error::AgentError;
use crate::providers::{ModelProvider, StreamEvent};
use crate::session::{Session, SessionManager};
use crate::tools::{Tool, ToolContext, ToolExecutor, ToolRegistry};
use crate::Result;
use async_stream::stream;
use futures::Stream;
use openclaw_core::types::{
    AgentConfig, AgentId, ContentBlock, Message, Role, SessionKey, ThinkingLevel,
    TokenUsage, ToolDefinition,
};
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Configuration for the agent runtime.
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    /// Maximum turns per request.
    pub max_turns: usize,

    /// Maximum output tokens.
    pub max_output_tokens: usize,

    /// Temperature for generation.
    pub temperature: f32,

    /// Thinking level.
    pub thinking_level: ThinkingLevel,

    /// System prompt.
    pub system_prompt: Option<String>,

    /// Stop sequences.
    pub stop_sequences: Vec<String>,

    /// Enable tool use.
    pub enable_tools: bool,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            max_turns: 10,
            max_output_tokens: 4096,
            temperature: 0.7,
            thinking_level: ThinkingLevel::default(),
            system_prompt: None,
            stop_sequences: vec![],
            enable_tools: true,
        }
    }
}

/// The agent runtime executes conversations with model providers.
pub struct AgentRuntime {
    /// Agent ID.
    agent_id: AgentId,

    /// Agent configuration.
    config: AgentConfig,

    /// Runtime configuration.
    runtime_config: RuntimeConfig,

    /// Model provider.
    provider: Arc<dyn ModelProvider>,

    /// Session manager.
    sessions: Arc<SessionManager>,

    /// Tool registry.
    tools: Arc<ToolRegistry>,

    /// Tool executor.
    executor: Arc<ToolExecutor>,

    /// Approval manager.
    approvals: Arc<ApprovalManager>,
}

impl AgentRuntime {
    /// Create a new agent runtime.
    pub fn new(
        agent_id: AgentId,
        config: AgentConfig,
        provider: Arc<dyn ModelProvider>,
        sessions: Arc<SessionManager>,
    ) -> Self {
        let tools = Arc::new(ToolRegistry::new());
        let executor = Arc::new(ToolExecutor::new(tools.clone()));

        Self {
            agent_id,
            config,
            runtime_config: RuntimeConfig::default(),
            provider,
            sessions,
            tools,
            executor,
            approvals: Arc::new(ApprovalManager::new()),
        }
    }

    /// Set the runtime configuration.
    pub fn with_config(mut self, config: RuntimeConfig) -> Self {
        self.runtime_config = config;
        self
    }

    /// Set the tool registry.
    pub fn with_tools(mut self, tools: Arc<ToolRegistry>) -> Self {
        self.tools = tools.clone();
        self.executor = Arc::new(ToolExecutor::new(tools));
        self
    }

    /// Set the approval manager.
    pub fn with_approvals(mut self, approvals: Arc<ApprovalManager>) -> Self {
        self.approvals = approvals;
        self
    }

    /// Get the agent ID.
    pub fn agent_id(&self) -> &AgentId {
        &self.agent_id
    }

    /// Get the tool registry.
    pub fn tools(&self) -> &Arc<ToolRegistry> {
        &self.tools
    }

    /// Get the approval manager.
    pub fn approvals(&self) -> &Arc<ApprovalManager> {
        &self.approvals
    }

    /// Process a message and return the response.
    pub async fn process(&self, session_key: &SessionKey, message: &str) -> Result<String> {
        let session = self.sessions.get_or_create(session_key.clone()).await?;

        // Add user message
        {
            let mut s = session.write().await;
            s.add_user_message(message);
        }

        // Run the agentic loop
        let mut turns = 0;
        let mut final_response = String::new();

        while turns < self.runtime_config.max_turns {
            turns += 1;
            debug!("Agent turn {}/{}", turns, self.runtime_config.max_turns);

            // Get current messages
            let messages = {
                let s = session.read().await;
                s.messages.clone()
            };

            // Get tool definitions
            let tools = if self.runtime_config.enable_tools {
                self.tools.definitions().await
            } else {
                vec![]
            };

            // Call the model
            let response = self
                .provider
                .generate(
                    &self.config.model.as_deref().unwrap_or("claude-sonnet-4-20250514"),
                    &messages,
                    &self.runtime_config.system_prompt,
                    &tools,
                    self.runtime_config.max_output_tokens,
                    self.runtime_config.temperature,
                )
                .await?;

            // Update token usage
            {
                let mut s = session.write().await;
                s.update_tokens(response.usage);
            }

            // Process response content
            let mut has_tool_use = false;
            let mut assistant_content = Vec::new();
            let mut text_response = String::new();

            for block in &response.content {
                match block {
                    ContentBlock::Text { text } => {
                        text_response.push_str(text);
                        assistant_content.push(block.clone());
                    }
                    ContentBlock::ToolUse { id, name, input } => {
                        has_tool_use = true;
                        assistant_content.push(block.clone());

                        // Execute the tool
                        let result = self.execute_tool(session_key, name, input.clone()).await;

                        // Add tool result
                        assistant_content.push(ContentBlock::ToolResult {
                            tool_use_id: id.clone(),
                            content: match &result {
                                Ok(r) => r.output.as_ref()
                                    .map(|v| v.to_string())
                                    .unwrap_or_else(|| "Success".to_string()),
                                Err(e) => format!("Error: {}", e),
                            },
                            is_error: result.is_err() || result.as_ref().map(|r| !r.success).unwrap_or(false),
                        });
                    }
                    _ => {
                        assistant_content.push(block.clone());
                    }
                }
            }

            // Add assistant message
            {
                let mut s = session.write().await;
                s.add_message(Role::Assistant, assistant_content);
            }

            // Check if we should continue
            if !has_tool_use || response.stop_reason == Some("end_turn".to_string()) {
                final_response = text_response;
                break;
            }
        }

        // Save the session
        self.sessions.save(session_key).await?;

        Ok(final_response)
    }

    /// Process a message with streaming response.
    pub fn process_stream(
        &self,
        session_key: SessionKey,
        message: String,
    ) -> Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send + '_>> {
        Box::pin(stream! {
            let session = match self.sessions.get_or_create(session_key.clone()).await {
                Ok(s) => s,
                Err(e) => {
                    yield Err(e);
                    return;
                }
            };

            // Add user message
            {
                let mut s = session.write().await;
                s.add_user_message(&message);
            }

            let mut turns = 0;

            while turns < self.runtime_config.max_turns {
                turns += 1;

                // Get current messages
                let messages = {
                    let s = session.read().await;
                    s.messages.clone()
                };

                // Get tool definitions
                let tools = if self.runtime_config.enable_tools {
                    self.tools.definitions().await
                } else {
                    vec![]
                };

                // Stream from the model
                let mut stream = self.provider.generate_stream(
                    self.config.model.as_deref().unwrap_or("claude-sonnet-4-20250514"),
                    &messages,
                    &self.runtime_config.system_prompt,
                    &tools,
                    self.runtime_config.max_output_tokens,
                    self.runtime_config.temperature,
                );

                let mut has_tool_use = false;
                let mut assistant_content = Vec::new();
                let mut current_text = String::new();
                let mut current_tool_use: Option<(String, String, serde_json::Value)> = None;

                use futures::StreamExt;

                while let Some(event) = stream.next().await {
                    match event {
                        Ok(StreamEvent::TextDelta { text }) => {
                            current_text.push_str(&text);
                            yield Ok(StreamEvent::TextDelta { text });
                        }
                        Ok(StreamEvent::ToolUseStart { id, name }) => {
                            has_tool_use = true;
                            current_tool_use = Some((id, name.clone(), serde_json::Value::Null));
                            yield Ok(StreamEvent::ToolUseStart { id: id.clone(), name });
                        }
                        Ok(StreamEvent::ToolUseInput { input }) => {
                            if let Some((_, _, ref mut args)) = current_tool_use {
                                *args = input.clone();
                            }
                            yield Ok(StreamEvent::ToolUseInput { input });
                        }
                        Ok(StreamEvent::ToolUseEnd) => {
                            if let Some((id, name, input)) = current_tool_use.take() {
                                // Add text before tool use
                                if !current_text.is_empty() {
                                    assistant_content.push(ContentBlock::Text {
                                        text: std::mem::take(&mut current_text),
                                    });
                                }

                                assistant_content.push(ContentBlock::ToolUse {
                                    id: id.clone(),
                                    name: name.clone(),
                                    input: input.clone(),
                                });

                                // Execute tool
                                let result = self.execute_tool(&session_key, &name, input).await;

                                let (content, is_error) = match &result {
                                    Ok(r) => (
                                        r.output.as_ref()
                                            .map(|v| v.to_string())
                                            .unwrap_or_else(|| "Success".to_string()),
                                        !r.success,
                                    ),
                                    Err(e) => (format!("Error: {}", e), true),
                                };

                                assistant_content.push(ContentBlock::ToolResult {
                                    tool_use_id: id.clone(),
                                    content: content.clone(),
                                    is_error,
                                });

                                yield Ok(StreamEvent::ToolResult {
                                    tool_use_id: id,
                                    content,
                                    is_error,
                                });
                            }
                            yield Ok(StreamEvent::ToolUseEnd);
                        }
                        Ok(StreamEvent::Usage(usage)) => {
                            let mut s = session.write().await;
                            s.update_tokens(usage.clone());
                            yield Ok(StreamEvent::Usage(usage));
                        }
                        Ok(StreamEvent::Done) => {
                            yield Ok(StreamEvent::Done);
                            break;
                        }
                        Err(e) => {
                            yield Err(e);
                            return;
                        }
                        other => {
                            yield other;
                        }
                    }
                }

                // Add remaining text
                if !current_text.is_empty() {
                    assistant_content.push(ContentBlock::Text { text: current_text });
                }

                // Add assistant message
                if !assistant_content.is_empty() {
                    let mut s = session.write().await;
                    s.add_message(Role::Assistant, assistant_content);
                }

                if !has_tool_use {
                    break;
                }
            }

            // Save session
            if let Err(e) = self.sessions.save(&session_key).await {
                warn!("Failed to save session: {}", e);
            }
        })
    }

    /// Execute a tool.
    async fn execute_tool(
        &self,
        session_key: &SessionKey,
        name: &str,
        args: serde_json::Value,
    ) -> Result<openclaw_core::types::ToolResult> {
        debug!("Executing tool '{}' with args: {:?}", name, args);

        // Check if approval is required
        if self.executor.requires_approval(name, &args).await? {
            let request = self
                .approvals
                .request(
                    session_key.session_id.clone(),
                    name.to_string(),
                    args.clone(),
                    format!("Tool '{}' requires approval", name),
                )
                .await?;

            // If not auto-approved, wait for response
            if request.status == crate::approval::ApprovalStatus::Pending {
                let response = self
                    .approvals
                    .wait_for_response(&request.id, None)
                    .await?;

                if !response.approved {
                    return Err(AgentError::ApprovalDenied(name.to_string()));
                }
            } else if request.status == crate::approval::ApprovalStatus::Denied {
                return Err(AgentError::ApprovalDenied(name.to_string()));
            }
        }

        // Create tool context
        let context = ToolContext {
            session_id: session_key.session_id.clone(),
            agent_id: session_key.agent_id.to_string(),
            ..Default::default()
        };

        // Execute the tool
        self.executor.execute(name, args, Some(&context)).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::MemorySessionStore;

    // Test helper to create a mock provider
    // In real tests, you'd use mockall or similar

    #[test]
    fn test_runtime_config_default() {
        let config = RuntimeConfig::default();
        assert_eq!(config.max_turns, 10);
        assert_eq!(config.max_output_tokens, 4096);
        assert!(config.enable_tools);
    }
}
