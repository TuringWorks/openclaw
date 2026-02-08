//! Memory tools.
//!
//! - [`MemorySearchTool`] - Semantic search of memory
//! - [`MemoryGetTool`] - Read memory files

use super::{Tool, ToolContext};
use crate::error::AgentError;
use crate::Result;
use async_trait::async_trait;
use openclaw_core::types::{ToolDefinition, ToolExecutionConfig, ToolGroup, ToolResult};
use openclaw_memory::VectorStore;
use std::sync::Arc;
use std::time::Instant;
use tracing::debug;

/// Memory search tool - Semantic search of memory.
pub struct MemorySearchTool {
    /// Maximum results to return.
    max_results: usize,

    /// Vector store for searching.
    store: Option<Arc<dyn VectorStore>>,
}

impl Default for MemorySearchTool {
    fn default() -> Self {
        Self::new()
    }
}

impl MemorySearchTool {
    /// Create a new memory search tool.
    pub fn new() -> Self {
        Self {
            max_results: 10,
            store: None,
        }
    }

    /// Set the maximum results to return.
    pub fn with_max_results(mut self, max: usize) -> Self {
        self.max_results = max;
        self
    }

    /// Set the vector store.
    pub fn with_store(mut self, store: Arc<dyn VectorStore>) -> Self {
        self.store = Some(store);
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
        context: &ToolContext,
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
            .unwrap_or(0.7) as f32;

        let categories: Option<Vec<String>> = args
            .get("categories")
            .and_then(|v| serde_json::from_value(v.clone()).ok());

        debug!(
            "Memory search: query='{}', limit={}, threshold={}, categories={:?}",
            query, limit, threshold, categories
        );

        // Check if we have a vector store configured
        let store = match &self.store {
            Some(s) => s.clone(),
            None => {
                // Try to get store from context
                match context.data.get("memory_store") {
                    Some(_store_value) => {
                        // If no store available, return empty results
                        debug!("No memory store configured, returning empty results");
                        let duration = start.elapsed();
                        return Ok(
                            ToolResult::success(tool_use_id, serde_json::json!({
                                "query": query,
                                "results": [],
                                "count": 0,
                                "threshold": threshold,
                                "message": "Memory store not configured"
                            }))
                            .with_duration(duration),
                        );
                    }
                    None => {
                        let duration = start.elapsed();
                        return Ok(
                            ToolResult::success(tool_use_id, serde_json::json!({
                                "query": query,
                                "results": [],
                                "count": 0,
                                "threshold": threshold,
                                "message": "Memory store not configured"
                            }))
                            .with_duration(duration),
                        );
                    }
                }
            }
        };

        // Generate a simple query embedding (in production, use an embedding model)
        // For now, we'll create a placeholder embedding
        let query_embedding = generate_simple_embedding(query);

        // Search the store
        let results = store
            .search(&query_embedding, limit)
            .await
            .map_err(|e| AgentError::tool_execution(format!("Memory search failed: {}", e)))?;

        // Filter by threshold and convert to JSON
        let filtered_results: Vec<serde_json::Value> = results
            .into_iter()
            .filter(|(_, score)| *score >= threshold)
            .filter(|(entry, _)| {
                // Filter by categories if specified
                if let Some(ref cats) = categories {
                    if let Some(entry_cat) = entry.metadata.get("category") {
                        if let Some(cat_str) = entry_cat.as_str() {
                            return cats.iter().any(|c| c == cat_str);
                        }
                    }
                    false
                } else {
                    true
                }
            })
            .map(|(entry, score)| {
                serde_json::json!({
                    "id": entry.id,
                    "content": entry.content,
                    "score": score,
                    "metadata": entry.metadata,
                    "created_at": entry.created_at.to_rfc3339(),
                })
            })
            .collect();

        let count = filtered_results.len();

        let duration = start.elapsed();
        Ok(
            ToolResult::success(tool_use_id, serde_json::json!({
                "query": query,
                "results": filtered_results,
                "count": count,
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
pub struct MemoryGetTool {
    /// Vector store for retrieval.
    store: Option<Arc<dyn VectorStore>>,
}

impl Default for MemoryGetTool {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryGetTool {
    /// Create a new memory get tool.
    pub fn new() -> Self {
        Self { store: None }
    }

    /// Set the vector store.
    pub fn with_store(mut self, store: Arc<dyn VectorStore>) -> Self {
        self.store = Some(store);
        self
    }
}

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

        // Get the store
        let store = match &self.store {
            Some(s) => s.clone(),
            None => {
                let duration = start.elapsed();
                return Ok(
                    ToolResult::error(tool_use_id, "Memory store not configured")
                        .with_duration(duration),
                );
            }
        };

        // If we have an ID, look it up directly
        if let Some(entry_id) = id {
            match store.get(entry_id).await {
                Ok(Some(entry)) => {
                    let duration = start.elapsed();
                    return Ok(
                        ToolResult::success(tool_use_id, serde_json::json!({
                            "id": entry.id,
                            "content": entry.content,
                            "metadata": entry.metadata,
                            "created_at": entry.created_at.to_rfc3339(),
                        }))
                        .with_duration(duration),
                    );
                }
                Ok(None) => {
                    let duration = start.elapsed();
                    return Ok(
                        ToolResult::error(tool_use_id, format!("Memory entry not found: {}", entry_id))
                            .with_duration(duration),
                    );
                }
                Err(e) => {
                    let duration = start.elapsed();
                    return Ok(
                        ToolResult::error(tool_use_id, format!("Failed to get memory: {}", e))
                            .with_duration(duration),
                    );
                }
            }
        }

        // If we have a path, search for it in metadata
        if let Some(_memory_path) = path {
            // Would search for entries with matching path in metadata
            let duration = start.elapsed();
            return Ok(
                ToolResult::error(tool_use_id, "Path-based lookup not yet implemented")
                    .with_duration(duration),
            );
        }

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

/// Generate a simple embedding for a query.
/// In production, this would use an actual embedding model.
fn generate_simple_embedding(text: &str) -> Vec<f32> {
    // Simple character-based embedding for demonstration
    // In production, use OpenAI embeddings or similar
    let mut embedding = vec![0.0f32; 128];

    for (i, ch) in text.chars().enumerate() {
        let idx = (ch as usize + i) % embedding.len();
        embedding[idx] += 1.0;
    }

    // Normalize
    let magnitude: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
    if magnitude > 0.0 {
        for x in &mut embedding {
            *x /= magnitude;
        }
    }

    embedding
}

#[cfg(test)]
mod tests {
    use super::*;
    use openclaw_memory::MemoryVectorStore;

    #[test]
    fn test_memory_search_tool_creation() {
        let tool = MemorySearchTool::new();
        assert_eq!(tool.name(), "memory_search");
    }

    #[test]
    fn test_memory_get_tool_creation() {
        let tool = MemoryGetTool::new();
        assert_eq!(tool.name(), "memory_get");
    }

    #[test]
    fn test_memory_search_custom_max_results() {
        let tool = MemorySearchTool::new().with_max_results(20);
        assert_eq!(tool.max_results, 20);
    }

    #[test]
    fn test_memory_search_with_store() {
        let store = Arc::new(MemoryVectorStore::new());
        let tool = MemorySearchTool::new().with_store(store);
        assert!(tool.store.is_some());
    }

    #[test]
    fn test_memory_get_with_store() {
        let store = Arc::new(MemoryVectorStore::new());
        let tool = MemoryGetTool::new().with_store(store);
        assert!(tool.store.is_some());
    }

    #[test]
    fn test_simple_embedding() {
        let embedding = generate_simple_embedding("hello world");
        assert_eq!(embedding.len(), 128);

        // Check it's normalized (magnitude ~= 1.0)
        let magnitude: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((magnitude - 1.0).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_memory_search_with_store_integration() {
        use openclaw_memory::MemoryEntry;

        let store = Arc::new(MemoryVectorStore::new());

        // Add some test entries
        let entry1 = MemoryEntry::new("Hello world", generate_simple_embedding("Hello world"));
        let entry2 = MemoryEntry::new("Goodbye world", generate_simple_embedding("Goodbye world"));
        store.insert(entry1).await.unwrap();
        store.insert(entry2).await.unwrap();

        let tool = MemorySearchTool::new().with_store(store);

        let args = serde_json::json!({
            "query": "Hello",
            "limit": 5
        });

        let result = tool
            .execute("test-id", args, &ToolContext::default())
            .await
            .unwrap();

        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn test_memory_get_with_store_integration() {
        use openclaw_memory::MemoryEntry;

        let store = Arc::new(MemoryVectorStore::new());

        // Add a test entry
        let entry = MemoryEntry::new("Test content", vec![1.0, 0.0, 0.0]);
        let entry_id = entry.id.clone();
        store.insert(entry).await.unwrap();

        let tool = MemoryGetTool::new().with_store(store);

        let args = serde_json::json!({
            "id": entry_id
        });

        let result = tool
            .execute("test-id", args, &ToolContext::default())
            .await
            .unwrap();

        assert!(!result.is_error);
    }
}
