//! Model provider integrations.

pub mod anthropic;

use crate::error::AgentError;
use crate::Result;
use async_trait::async_trait;
use futures::Stream;
use openclaw_core::types::{ContentBlock, Message, TokenUsage, ToolDefinition};
use serde::{Deserialize, Serialize};
use std::pin::Pin;

/// Response from a model generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateResponse {
    /// Generated content.
    pub content: Vec<ContentBlock>,

    /// Token usage.
    pub usage: TokenUsage,

    /// Stop reason.
    pub stop_reason: Option<String>,

    /// Model used.
    pub model: String,
}

/// Streaming event from model generation.
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// Text delta.
    TextDelta { text: String },

    /// Thinking text (for extended thinking).
    ThinkingDelta { text: String },

    /// Tool use start.
    ToolUseStart { id: String, name: String },

    /// Tool use input delta.
    ToolUseInput { input: serde_json::Value },

    /// Tool use end.
    ToolUseEnd,

    /// Tool result.
    ToolResult {
        tool_use_id: String,
        content: String,
        is_error: bool,
    },

    /// Token usage update.
    Usage(TokenUsage),

    /// Message complete.
    Done,

    /// Error occurred.
    Error(String),
}

/// Trait for model providers.
#[async_trait]
pub trait ModelProvider: Send + Sync {
    /// Get the provider name.
    fn name(&self) -> &str;

    /// List available models.
    async fn list_models(&self) -> Result<Vec<String>>;

    /// Generate a response (non-streaming).
    async fn generate(
        &self,
        model: &str,
        messages: &[Message],
        system: &Option<String>,
        tools: &[ToolDefinition],
        max_tokens: usize,
        temperature: f32,
    ) -> Result<GenerateResponse>;

    /// Generate a response (streaming).
    fn generate_stream(
        &self,
        model: &str,
        messages: &[Message],
        system: &Option<String>,
        tools: &[ToolDefinition],
        max_tokens: usize,
        temperature: f32,
    ) -> Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send + '_>>;

    /// Count tokens for messages.
    async fn count_tokens(&self, messages: &[Message]) -> Result<usize>;
}

pub use anthropic::AnthropicProvider;
