//! Vector storage implementations.

use crate::embeddings::cosine_similarity;
use crate::error::MemoryError;
use crate::{MemoryEntry, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use tokio::sync::RwLock;

/// Trait for vector stores.
#[async_trait]
pub trait VectorStore: Send + Sync {
    /// Insert an entry.
    async fn insert(&self, entry: MemoryEntry) -> Result<()>;

    /// Insert multiple entries.
    async fn insert_batch(&self, entries: Vec<MemoryEntry>) -> Result<()>;

    /// Get an entry by ID.
    async fn get(&self, id: &str) -> Result<Option<MemoryEntry>>;

    /// Delete an entry by ID.
    async fn delete(&self, id: &str) -> Result<()>;

    /// Search for similar entries.
    async fn search(&self, query: &[f32], limit: usize) -> Result<Vec<(MemoryEntry, f32)>>;

    /// Count entries.
    async fn count(&self) -> Result<usize>;

    /// Clear all entries.
    async fn clear(&self) -> Result<()>;
}

/// In-memory vector store.
pub struct MemoryVectorStore {
    entries: RwLock<HashMap<String, MemoryEntry>>,
}

impl Default for MemoryVectorStore {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryVectorStore {
    /// Create a new in-memory vector store.
    pub fn new() -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl VectorStore for MemoryVectorStore {
    async fn insert(&self, entry: MemoryEntry) -> Result<()> {
        let mut entries = self.entries.write().await;
        entries.insert(entry.id.clone(), entry);
        Ok(())
    }

    async fn insert_batch(&self, batch: Vec<MemoryEntry>) -> Result<()> {
        let mut entries = self.entries.write().await;
        for entry in batch {
            entries.insert(entry.id.clone(), entry);
        }
        Ok(())
    }

    async fn get(&self, id: &str) -> Result<Option<MemoryEntry>> {
        let entries = self.entries.read().await;
        Ok(entries.get(id).cloned())
    }

    async fn delete(&self, id: &str) -> Result<()> {
        let mut entries = self.entries.write().await;
        entries.remove(id);
        Ok(())
    }

    async fn search(&self, query: &[f32], limit: usize) -> Result<Vec<(MemoryEntry, f32)>> {
        let entries = self.entries.read().await;

        let mut results: Vec<(MemoryEntry, f32)> = entries
            .values()
            .map(|entry| {
                let score = cosine_similarity(query, &entry.embedding);
                (entry.clone(), score)
            })
            .collect();

        // Sort by score descending
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Take top k
        results.truncate(limit);

        Ok(results)
    }

    async fn count(&self) -> Result<usize> {
        let entries = self.entries.read().await;
        Ok(entries.len())
    }

    async fn clear(&self) -> Result<()> {
        let mut entries = self.entries.write().await;
        entries.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_memory_store() {
        let store = MemoryVectorStore::new();

        let entry = MemoryEntry::new("test content", vec![1.0, 0.0, 0.0]);
        store.insert(entry.clone()).await.unwrap();

        let loaded = store.get(&entry.id).await.unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().content, "test content");
    }

    #[tokio::test]
    async fn test_search() {
        let store = MemoryVectorStore::new();

        store
            .insert(MemoryEntry::new("first", vec![1.0, 0.0, 0.0]))
            .await
            .unwrap();
        store
            .insert(MemoryEntry::new("second", vec![0.0, 1.0, 0.0]))
            .await
            .unwrap();
        store
            .insert(MemoryEntry::new("third", vec![0.9, 0.1, 0.0]))
            .await
            .unwrap();

        let results = store.search(&[1.0, 0.0, 0.0], 2).await.unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0.content, "first");
    }
}
