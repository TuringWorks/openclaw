//! Memory tools.
//!
//! - [`MemorySearchTool`] - Semantic search of memory
//! - [`MemoryGetTool`] - Read memory files

use super::{Tool, ToolContext};
use crate::error::AgentError;
use crate::Result;
use async_trait::async_trait;
use openclaw_core::types::{ToolDefinition, ToolExecutionConfig, ToolGroup, ToolResult};
use std::time::Instant;
use tracing::debug;

/// Memory search tool - Semantic search of memory.
pub struct MemorySearchTool {
    /// Maximum results to return.
    max_results: usize,
}

impl Default for MemorySearchTool {
    fn default() -> Self {
        Self::new()
    }
}

impl MemorySearchTool {
    /// Create a new memory search tool.
    pub fn new() -> Self {
        Self { max_results: 10 }
    }

    /// Set the maximum results to return.
    pub fn with_max_results(mut self, max: usize) -> Self {
        self.max_results = max;
        self
    }
}

#[async_trait]
impl Tool for MemorySearchTool {
    fn name(&self) -> &str {
        "memory_search"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "memory_search".to_string(),
            description: "Search the memory store for relevant information using semantic similarity".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The search query"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of results to return (default: 10)"
                    },
                    "threshold": {
                        "type": "number",
                        "description": "Minimum similarity threshold (0.0-1.0, default: 0.7)"
                    },
                    "categories": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Filter by categories (optional)"
                    }
                },
                "required": ["query"]
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

        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AgentError::tool_execution("Missing 'query' argument"))?;

        let limit = args
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(self.max_results as u64) as usize;

        let threshold = args
            .get("threshold")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.7);

        let categories: Option<Vec<String>> = args
            .get("categories")
            .and_then(|v| serde_json::from_value(v.clone()).ok());

        debug!(
            "Memory search: query='{}', limit={}, threshold={}, categories={:?}",
            query, limit, threshold, categories
        );

        // TODO: Actually search using openclaw-memory crate
        // For now, return empty results

        let duration = start.elapsed();
        Ok(
            ToolResult::success(tool_use_id, serde_json::json!({
                "query": query,
                "results": [],
                "count": 0,
                "threshold": threshold,
            }))
            .with_duration(duration),
        )
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Memory
    }
}

/// Memory get tool - Read memory files.
pub struct MemoryGetTool;

#[async_trait]
impl Tool for MemoryGetTool {
    fn name(&self) -> &str {
        "memory_get"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "memory_get".to_string(),
            description: "Get a specific memory entry by ID or path".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "The memory entry ID"
                    },
                    "path": {
                        "type": "string",
                        "description": "The memory file path (relative to memory store)"
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

        let id = args.get("id").and_then(|v| v.as_str());
        let path = args.get("path").and_then(|v| v.as_str());

        if id.is_none() && path.is_none() {
            return Err(AgentError::tool_execution(
                "Either 'id' or 'path' must be provided",
            ));
        }

        debug!("Memory get: id={:?}, path={:?}", id, path);

        // TODO: Actually retrieve from openclaw-memory crate
        // For now, return not found

        let duration = start.elapsed();
        Ok(
            ToolResult::error(tool_use_id, "Memory entry not found")
                .with_duration(duration),
        )
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Memory
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_search_tool_creation() {
        let tool = MemorySearchTool::new();
        assert_eq!(tool.name(), "memory_search");
    }

    #[test]
    fn test_memory_get_tool_creation() {
        let tool = MemoryGetTool;
        assert_eq!(tool.name(), "memory_get");
    }

    #[test]
    fn test_memory_search_custom_max_results() {
        let tool = MemorySearchTool::new().with_max_results(20);
        assert_eq!(tool.max_results, 20);
    }
}
