//! Model provider integrations.

pub mod anthropic;

use crate::Result;
use async_trait::async_trait;
use futures::Stream;
use openclaw_core::types::{Message, MessageContent, TokenUsage, ToolDefinition};
use serde::{Deserialize, Serialize};
use std::pin::Pin;

/// Response from a model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelResponse {
    /// Generated content.
    pub content: MessageContent,

    /// Stop reason.
    pub stop_reason: Option<String>,

    /// Token usage.
    pub token_usage: TokenUsage,
}

/// Streaming event from model generation.
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// Stream started.
    Start,

    /// Text content.
    Text(String),

    /// Thinking text (for extended thinking).
    Thinking(String),

    /// Tool use.
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },

    /// Token usage update.
    Usage(TokenUsage),

    /// Stream completed.
    Done,

    /// Error occurred.
    Error(String),
}

/// Trait for model providers.
#[async_trait]
pub trait ModelProvider: Send + Sync {
    /// Get the provider name.
    fn name(&self) -> &str;

    /// Get the current model.
    fn model(&self) -> &str;

    /// Generate a response (non-streaming).
    async fn complete(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Result<ModelResponse>;

    /// Generate a response (streaming).
    fn complete_stream(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send + '_>>;
}

pub use anthropic::AnthropicProvider;
