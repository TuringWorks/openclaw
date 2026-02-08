//! Media tools.
//!
//! - [`ImageTool`] - Analyze images with vision models
//! - [`TtsTool`] - Text to speech conversion

use super::{Tool, ToolContext};
use crate::error::AgentError;
use crate::Result;
use async_trait::async_trait;
use openclaw_core::types::{ToolDefinition, ToolExecutionConfig, ToolGroup, ToolResult};
use std::path::Path;
use std::time::Instant;
use tracing::debug;

/// Image tool - Analyze images with vision models.
pub struct ImageTool;

impl Default for ImageTool {
    fn default() -> Self {
        Self::new()
    }
}

impl ImageTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for ImageTool {
    fn name(&self) -> &str {
        "image"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "image".to_string(),
            description: "Analyze images using vision models. Can describe, extract text (OCR), detect objects, or answer questions about images.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the image file"
                    },
                    "url": {
                        "type": "string",
                        "description": "URL of the image"
                    },
                    "action": {
                        "type": "string",
                        "enum": ["describe", "ocr", "detect", "ask"],
                        "description": "Action to perform on the image"
                    },
                    "question": {
                        "type": "string",
                        "description": "Question to ask about the image (for 'ask' action)"
                    }
                }
            }),
            execution: ToolExecutionConfig::default(),
        }
    }

    async fn execute(
        &self,
        tool_use_id: &str,
        args: serde_json::Value,
        _context: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();

        let path = args.get("path").and_then(|v| v.as_str());
        let url = args.get("url").and_then(|v| v.as_str());
        let action = args
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("describe");

        if path.is_none() && url.is_none() {
            return Err(AgentError::tool_execution(
                "Either 'path' or 'url' must be provided",
            ));
        }

        debug!("Image tool: action={}, path={:?}, url={:?}", action, path, url);

        // Validate file exists if path provided
        if let Some(p) = path {
            if !Path::new(p).exists() {
                return Err(AgentError::tool_execution(format!(
                    "Image file not found: {}",
                    p
                )));
            }
        }

        // TODO: Actually analyze the image using vision model
        let result = match action {
            "describe" => {
                serde_json::json!({
                    "action": "describe",
                    "description": "Image analysis not yet implemented",
                    "source": path.or(url)
                })
            }
            "ocr" => {
                serde_json::json!({
                    "action": "ocr",
                    "text": "",
                    "source": path.or(url)
                })
            }
            "detect" => {
                serde_json::json!({
                    "action": "detect",
                    "objects": [],
                    "source": path.or(url)
                })
            }
            "ask" => {
                let question = args.get("question").and_then(|v| v.as_str());
                serde_json::json!({
                    "action": "ask",
                    "question": question,
                    "answer": "Image Q&A not yet implemented",
                    "source": path.or(url)
                })
            }
            _ => {
                return Err(AgentError::tool_execution(format!(
                    "Unknown action: {}",
                    action
                )));
            }
        };

        let duration = start.elapsed();
        Ok(ToolResult::success(tool_use_id, result).with_duration(duration))
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Custom
    }
}

/// TTS tool - Text to speech conversion.
pub struct TtsTool {
    /// Default voice to use.
    default_voice: String,
}

impl Default for TtsTool {
    fn default() -> Self {
        Self::new()
    }
}

impl TtsTool {
    pub fn new() -> Self {
        Self {
            default_voice: "alloy".to_string(),
        }
    }

    /// Set the default voice.
    pub fn with_default_voice(mut self, voice: impl Into<String>) -> Self {
        self.default_voice = voice.into();
        self
    }
}

#[async_trait]
impl Tool for TtsTool {
    fn name(&self) -> &str {
        "tts"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "tts".to_string(),
            description: "Convert text to speech audio. Generates audio files from text.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "text": {
                        "type": "string",
                        "description": "Text to convert to speech"
                    },
                    "voice": {
                        "type": "string",
                        "enum": ["alloy", "echo", "fable", "onyx", "nova", "shimmer"],
                        "description": "Voice to use"
                    },
                    "output": {
                        "type": "string",
                        "description": "Output file path (optional)"
                    },
                    "speed": {
                        "type": "number",
                        "description": "Speech speed (0.25 to 4.0, default 1.0)"
                    }
                },
                "required": ["text"]
            }),
            execution: ToolExecutionConfig::default(),
        }
    }

    async fn execute(
        &self,
        tool_use_id: &str,
        args: serde_json::Value,
        _context: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();

        let text = args
            .get("text")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AgentError::tool_execution("Missing 'text' argument"))?;

        let voice = args
            .get("voice")
            .and_then(|v| v.as_str())
            .unwrap_or(&self.default_voice);

        let speed = args.get("speed").and_then(|v| v.as_f64()).unwrap_or(1.0);
        let output = args.get("output").and_then(|v| v.as_str());

        debug!(
            "TTS: {} chars, voice={}, speed={}",
            text.len(),
            voice,
            speed
        );

        // Validate speed
        if !(0.25..=4.0).contains(&speed) {
            return Err(AgentError::tool_execution(
                "Speed must be between 0.25 and 4.0",
            ));
        }

        // TODO: Actually generate audio using TTS API
        let output_path = output
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("/tmp/tts_{}.mp3", uuid::Uuid::new_v4()));

        let result = serde_json::json!({
            "text_length": text.len(),
            "voice": voice,
            "speed": speed,
            "output": output_path,
            "generated": false,
            "message": "TTS generation not yet implemented"
        });

        let duration = start.elapsed();
        Ok(ToolResult::success(tool_use_id, result).with_duration(duration))
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Custom
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_image_tool_creation() {
        let tool = ImageTool::new();
        assert_eq!(tool.name(), "image");
    }

    #[test]
    fn test_tts_tool_creation() {
        let tool = TtsTool::new();
        assert_eq!(tool.name(), "tts");
    }

    #[test]
    fn test_tts_tool_custom_voice() {
        let tool = TtsTool::new().with_default_voice("nova");
        assert_eq!(tool.default_voice, "nova");
    }
}
