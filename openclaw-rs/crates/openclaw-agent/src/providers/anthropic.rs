//! Anthropic Claude API provider.

use super::{GenerateResponse, ModelProvider, StreamEvent};
use crate::error::AgentError;
use crate::Result;
use async_stream::stream;
use async_trait::async_trait;
use futures::Stream;
use openclaw_core::types::{ContentBlock, Message, Role, TokenUsage, ToolDefinition};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::pin::Pin;
use tracing::{debug, error, warn};

/// Anthropic API provider.
pub struct AnthropicProvider {
    /// HTTP client.
    client: Client,

    /// API key.
    api_key: String,

    /// Base URL.
    base_url: String,

    /// API version.
    api_version: String,
}

impl AnthropicProvider {
    /// Create a new Anthropic provider.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            api_key: api_key.into(),
            base_url: "https://api.anthropic.com".to_string(),
            api_version: "2023-06-01".to_string(),
        }
    }

    /// Set a custom base URL.
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    /// Build the request.
    fn build_request(
        &self,
        model: &str,
        messages: &[Message],
        system: &Option<String>,
        tools: &[ToolDefinition],
        max_tokens: usize,
        temperature: f32,
        stream: bool,
    ) -> ApiRequest {
        let api_messages: Vec<ApiMessage> = messages
            .iter()
            .map(|m| ApiMessage {
                role: match m.role {
                    Role::User => "user".to_string(),
                    Role::Assistant => "assistant".to_string(),
                },
                content: m
                    .content
                    .iter()
                    .map(|c| match c {
                        ContentBlock::Text { text } => ApiContent::Text {
                            text: text.clone(),
                        },
                        ContentBlock::ToolUse { id, name, input } => ApiContent::ToolUse {
                            id: id.clone(),
                            name: name.clone(),
                            input: input.clone(),
                        },
                        ContentBlock::ToolResult {
                            tool_use_id,
                            content,
                            is_error,
                        } => ApiContent::ToolResult {
                            tool_use_id: tool_use_id.clone(),
                            content: content.clone(),
                            is_error: Some(*is_error),
                        },
                        ContentBlock::Image { source, media_type } => ApiContent::Image {
                            source: ImageSource {
                                source_type: "base64".to_string(),
                                media_type: media_type.clone(),
                                data: source.clone(),
                            },
                        },
                        ContentBlock::Thinking { thinking } => ApiContent::Thinking {
                            thinking: thinking.clone(),
                        },
                    })
                    .collect(),
            })
            .collect();

        let api_tools: Vec<ApiTool> = tools
            .iter()
            .map(|t| ApiTool {
                name: t.name.clone(),
                description: t.description.clone(),
                input_schema: t.input_schema.clone(),
            })
            .collect();

        ApiRequest {
            model: model.to_string(),
            messages: api_messages,
            system: system.clone(),
            max_tokens,
            temperature: Some(temperature),
            tools: if api_tools.is_empty() {
                None
            } else {
                Some(api_tools)
            },
            stream: Some(stream),
            metadata: None,
        }
    }

    /// Parse API response.
    fn parse_response(&self, response: ApiResponse) -> GenerateResponse {
        let content: Vec<ContentBlock> = response
            .content
            .into_iter()
            .map(|c| match c {
                ApiContent::Text { text } => ContentBlock::Text { text },
                ApiContent::ToolUse { id, name, input } => ContentBlock::ToolUse { id, name, input },
                ApiContent::Thinking { thinking } => ContentBlock::Thinking { thinking },
                _ => ContentBlock::Text {
                    text: String::new(),
                },
            })
            .collect();

        GenerateResponse {
            content,
            usage: TokenUsage {
                input: response.usage.input_tokens,
                output: response.usage.output_tokens,
                cache_read: response.usage.cache_read_input_tokens,
                cache_write: response.usage.cache_creation_input_tokens,
            },
            stop_reason: Some(response.stop_reason),
            model: response.model,
        }
    }
}

#[async_trait]
impl ModelProvider for AnthropicProvider {
    fn name(&self) -> &str {
        "anthropic"
    }

    async fn list_models(&self) -> Result<Vec<String>> {
        // Anthropic doesn't have a models endpoint, return known models
        Ok(vec![
            "claude-opus-4-20250514".to_string(),
            "claude-sonnet-4-20250514".to_string(),
            "claude-3-5-sonnet-20241022".to_string(),
            "claude-3-5-haiku-20241022".to_string(),
            "claude-3-opus-20240229".to_string(),
            "claude-3-sonnet-20240229".to_string(),
            "claude-3-haiku-20240307".to_string(),
        ])
    }

    async fn generate(
        &self,
        model: &str,
        messages: &[Message],
        system: &Option<String>,
        tools: &[ToolDefinition],
        max_tokens: usize,
        temperature: f32,
    ) -> Result<GenerateResponse> {
        let request = self.build_request(model, messages, system, tools, max_tokens, temperature, false);

        debug!("Sending request to Anthropic API");

        let response = self
            .client
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", &self.api_version)
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| AgentError::model_api(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();

            // Check for rate limit
            if status.as_u16() == 429 {
                // Try to parse retry-after
                return Err(AgentError::rate_limit(60));
            }

            return Err(AgentError::model_api(format!(
                "API error {}: {}",
                status, text
            )));
        }

        let api_response: ApiResponse = response
            .json()
            .await
            .map_err(|e| AgentError::model_api(e.to_string()))?;

        Ok(self.parse_response(api_response))
    }

    fn generate_stream(
        &self,
        model: &str,
        messages: &[Message],
        system: &Option<String>,
        tools: &[ToolDefinition],
        max_tokens: usize,
        temperature: f32,
    ) -> Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send + '_>> {
        let request = self.build_request(model, messages, system, tools, max_tokens, temperature, true);
        let url = format!("{}/v1/messages", self.base_url);
        let api_key = self.api_key.clone();
        let api_version = self.api_version.clone();
        let client = self.client.clone();

        Box::pin(stream! {
            let response = match client
                .post(&url)
                .header("x-api-key", &api_key)
                .header("anthropic-version", &api_version)
                .header("content-type", "application/json")
                .json(&request)
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    yield Err(AgentError::model_api(e.to_string()));
                    return;
                }
            };

            if !response.status().is_success() {
                let status = response.status();
                let text = response.text().await.unwrap_or_default();
                yield Err(AgentError::model_api(format!("API error {}: {}", status, text)));
                return;
            }

            let mut stream = response.bytes_stream();
            let mut buffer = String::new();
            let mut current_tool_id = String::new();
            let mut current_tool_name = String::new();
            let mut current_tool_input = String::new();

            use futures::StreamExt;

            while let Some(chunk) = stream.next().await {
                let chunk = match chunk {
                    Ok(c) => c,
                    Err(e) => {
                        yield Err(AgentError::model_api(e.to_string()));
                        return;
                    }
                };

                buffer.push_str(&String::from_utf8_lossy(&chunk));

                // Process SSE events
                while let Some(line_end) = buffer.find('\n') {
                    let line = buffer[..line_end].to_string();
                    buffer = buffer[line_end + 1..].to_string();

                    if line.starts_with("data: ") {
                        let data = &line[6..];

                        if data == "[DONE]" {
                            yield Ok(StreamEvent::Done);
                            continue;
                        }

                        if let Ok(event) = serde_json::from_str::<StreamingEvent>(data) {
                            match event {
                                StreamingEvent::ContentBlockStart { index, content_block } => {
                                    match content_block {
                                        StreamingContentBlock::ToolUse { id, name } => {
                                            current_tool_id = id.clone();
                                            current_tool_name = name.clone();
                                            current_tool_input.clear();
                                            yield Ok(StreamEvent::ToolUseStart { id, name });
                                        }
                                        _ => {}
                                    }
                                }
                                StreamingEvent::ContentBlockDelta { index, delta } => {
                                    match delta {
                                        StreamingDelta::TextDelta { text } => {
                                            yield Ok(StreamEvent::TextDelta { text });
                                        }
                                        StreamingDelta::ThinkingDelta { thinking } => {
                                            yield Ok(StreamEvent::ThinkingDelta { text: thinking });
                                        }
                                        StreamingDelta::InputJsonDelta { partial_json } => {
                                            current_tool_input.push_str(&partial_json);
                                        }
                                    }
                                }
                                StreamingEvent::ContentBlockStop { index } => {
                                    if !current_tool_id.is_empty() {
                                        if let Ok(input) = serde_json::from_str(&current_tool_input) {
                                            yield Ok(StreamEvent::ToolUseInput { input });
                                        }
                                        yield Ok(StreamEvent::ToolUseEnd);
                                        current_tool_id.clear();
                                        current_tool_name.clear();
                                        current_tool_input.clear();
                                    }
                                }
                                StreamingEvent::MessageDelta { delta, usage } => {
                                    if let Some(u) = usage {
                                        yield Ok(StreamEvent::Usage(TokenUsage {
                                            input: u.input_tokens.unwrap_or(0),
                                            output: u.output_tokens,
                                            cache_read: None,
                                            cache_write: None,
                                        }));
                                    }
                                }
                                StreamingEvent::MessageStop => {
                                    yield Ok(StreamEvent::Done);
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        })
    }

    async fn count_tokens(&self, messages: &[Message]) -> Result<usize> {
        // Rough estimation: ~4 characters per token
        let total_chars: usize = messages
            .iter()
            .flat_map(|m| &m.content)
            .map(|c| match c {
                ContentBlock::Text { text } => text.len(),
                _ => 100, // Estimate for other content types
            })
            .sum();

        Ok(total_chars / 4)
    }
}

// API types

#[derive(Debug, Serialize)]
struct ApiRequest {
    model: String,
    messages: Vec<ApiMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    max_tokens: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<ApiTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct ApiMessage {
    role: String,
    content: Vec<ApiContent>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ApiContent {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
    Image {
        source: ImageSource,
    },
    Thinking {
        thinking: String,
    },
}

#[derive(Debug, Serialize, Deserialize)]
struct ImageSource {
    #[serde(rename = "type")]
    source_type: String,
    media_type: String,
    data: String,
}

#[derive(Debug, Serialize)]
struct ApiTool {
    name: String,
    description: String,
    input_schema: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct ApiResponse {
    id: String,
    model: String,
    content: Vec<ApiContent>,
    stop_reason: String,
    usage: ApiUsage,
}

#[derive(Debug, Deserialize)]
struct ApiUsage {
    input_tokens: usize,
    output_tokens: usize,
    #[serde(default)]
    cache_read_input_tokens: Option<usize>,
    #[serde(default)]
    cache_creation_input_tokens: Option<usize>,
}

// Streaming types

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum StreamingEvent {
    MessageStart {
        message: StreamingMessage,
    },
    ContentBlockStart {
        index: usize,
        content_block: StreamingContentBlock,
    },
    ContentBlockDelta {
        index: usize,
        delta: StreamingDelta,
    },
    ContentBlockStop {
        index: usize,
    },
    MessageDelta {
        delta: StreamingMessageDelta,
        usage: Option<StreamingUsage>,
    },
    MessageStop,
    Ping,
    Error {
        error: StreamingError,
    },
}

#[derive(Debug, Deserialize)]
struct StreamingMessage {
    id: String,
    model: String,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum StreamingContentBlock {
    Text { text: String },
    ToolUse { id: String, name: String },
    Thinking { thinking: String },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum StreamingDelta {
    TextDelta { text: String },
    ThinkingDelta { thinking: String },
    InputJsonDelta { partial_json: String },
}

#[derive(Debug, Deserialize)]
struct StreamingMessageDelta {
    stop_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StreamingUsage {
    input_tokens: Option<usize>,
    output_tokens: usize,
}

#[derive(Debug, Deserialize)]
struct StreamingError {
    message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_creation() {
        let provider = AnthropicProvider::new("test-key");
        assert_eq!(provider.name(), "anthropic");
    }

    #[tokio::test]
    async fn test_list_models() {
        let provider = AnthropicProvider::new("test-key");
        let models = provider.list_models().await.unwrap();
        assert!(!models.is_empty());
        assert!(models.iter().any(|m| m.contains("claude")));
    }
}
