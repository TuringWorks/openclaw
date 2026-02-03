//! Session management and persistence.

use crate::error::AgentError;
use crate::Result;
use chrono::{DateTime, Utc};
use openclaw_core::types::{
    AgentId, ContentBlock, Message, Role, SessionKey, TokenUsage,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::RwLock;
use tracing::{debug, warn};

/// A conversation session with an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// Session key.
    pub key: SessionKey,

    /// Associated agent ID.
    pub agent_id: AgentId,

    /// Conversation messages.
    pub messages: Vec<Message>,

    /// Session metadata.
    pub metadata: SessionMetadata,

    /// Session state.
    pub state: SessionState,

    /// Total token usage.
    pub total_tokens: TokenUsage,

    /// Creation timestamp.
    pub created_at: DateTime<Utc>,

    /// Last activity timestamp.
    pub last_activity: DateTime<Utc>,
}

/// Session metadata.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionMetadata {
    /// Custom key-value pairs.
    #[serde(default)]
    pub custom: HashMap<String, serde_json::Value>,

    /// System prompt override.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,

    /// Model override.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// Temperature override.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
}

/// Session state.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionState {
    /// Session is active.
    #[default]
    Active,

    /// Session is paused.
    Paused,

    /// Session is processing a request.
    Processing,

    /// Session is waiting for approval.
    WaitingApproval,

    /// Session is archived.
    Archived,
}

impl Session {
    /// Create a new session.
    pub fn new(key: SessionKey, agent_id: AgentId) -> Self {
        let now = Utc::now();
        Self {
            key,
            agent_id,
            messages: Vec::new(),
            metadata: SessionMetadata::default(),
            state: SessionState::Active,
            total_tokens: TokenUsage::default(),
            created_at: now,
            last_activity: now,
        }
    }

    /// Add a user message.
    pub fn add_user_message(&mut self, content: impl Into<String>) {
        self.messages.push(Message {
            role: Role::User,
            content: vec![ContentBlock::Text {
                text: content.into(),
            }],
        });
        self.last_activity = Utc::now();
    }

    /// Add an assistant message.
    pub fn add_assistant_message(&mut self, content: impl Into<String>) {
        self.messages.push(Message {
            role: Role::Assistant,
            content: vec![ContentBlock::Text {
                text: content.into(),
            }],
        });
        self.last_activity = Utc::now();
    }

    /// Add a message with content blocks.
    pub fn add_message(&mut self, role: Role, content: Vec<ContentBlock>) {
        self.messages.push(Message { role, content });
        self.last_activity = Utc::now();
    }

    /// Get the last message.
    pub fn last_message(&self) -> Option<&Message> {
        self.messages.last()
    }

    /// Get the last assistant message.
    pub fn last_assistant_message(&self) -> Option<&Message> {
        self.messages
            .iter()
            .rev()
            .find(|m| m.role == Role::Assistant)
    }

    /// Update token usage.
    pub fn update_tokens(&mut self, usage: TokenUsage) {
        self.total_tokens.input += usage.input;
        self.total_tokens.output += usage.output;
        if let Some(cache) = usage.cache_read {
            *self.total_tokens.cache_read.get_or_insert(0) += cache;
        }
        if let Some(cache) = usage.cache_write {
            *self.total_tokens.cache_write.get_or_insert(0) += cache;
        }
    }

    /// Get message count.
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    /// Truncate to a maximum number of messages (keeping system message if present).
    pub fn truncate(&mut self, max_messages: usize) {
        if self.messages.len() <= max_messages {
            return;
        }

        // Keep the most recent messages
        let start = self.messages.len() - max_messages;
        self.messages = self.messages.split_off(start);
    }

    /// Clear all messages.
    pub fn clear(&mut self) {
        self.messages.clear();
        self.total_tokens = TokenUsage::default();
    }

    /// Archive the session.
    pub fn archive(&mut self) {
        self.state = SessionState::Archived;
    }
}

/// Session storage trait.
#[async_trait::async_trait]
pub trait SessionStore: Send + Sync {
    /// Load a session by key.
    async fn load(&self, key: &SessionKey) -> Result<Option<Session>>;

    /// Save a session.
    async fn save(&self, session: &Session) -> Result<()>;

    /// Delete a session.
    async fn delete(&self, key: &SessionKey) -> Result<()>;

    /// List session keys for an agent.
    async fn list(&self, agent_id: &AgentId) -> Result<Vec<SessionKey>>;

    /// Check if a session exists.
    async fn exists(&self, key: &SessionKey) -> Result<bool>;
}

/// File-based session store using JSON Lines.
pub struct FileSessionStore {
    /// Base directory for sessions.
    base_dir: PathBuf,
}

impl FileSessionStore {
    /// Create a new file session store.
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self {
            base_dir: base_dir.into(),
        }
    }

    /// Get the path for a session file.
    fn session_path(&self, key: &SessionKey) -> PathBuf {
        self.base_dir
            .join(&key.agent_id.to_string())
            .join("sessions")
            .join(format!("{}.jsonl", key.session_id))
    }

    /// Ensure directory exists.
    async fn ensure_dir(&self, key: &SessionKey) -> Result<()> {
        let dir = self
            .base_dir
            .join(&key.agent_id.to_string())
            .join("sessions");
        fs::create_dir_all(&dir).await?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl SessionStore for FileSessionStore {
    async fn load(&self, key: &SessionKey) -> Result<Option<Session>> {
        let path = self.session_path(key);

        if !path.exists() {
            return Ok(None);
        }

        let file = fs::File::open(&path).await?;
        let reader = BufReader::new(file);
        let mut lines = reader.lines();

        // First line is session metadata
        let first_line = match lines.next_line().await? {
            Some(line) => line,
            None => return Ok(None),
        };

        let mut session: Session = serde_json::from_str(&first_line)?;

        // Remaining lines are messages
        while let Some(line) = lines.next_line().await? {
            if let Ok(msg) = serde_json::from_str::<Message>(&line) {
                session.messages.push(msg);
            }
        }

        Ok(Some(session))
    }

    async fn save(&self, session: &Session) -> Result<()> {
        self.ensure_dir(&session.key).await?;
        let path = self.session_path(&session.key);

        // Write to temp file first
        let temp_path = path.with_extension("tmp");
        let mut file = fs::File::create(&temp_path).await?;

        // Write session metadata (without messages)
        let mut session_meta = session.clone();
        let messages = std::mem::take(&mut session_meta.messages);
        let meta_json = serde_json::to_string(&session_meta)?;
        file.write_all(meta_json.as_bytes()).await?;
        file.write_all(b"\n").await?;

        // Write each message
        for msg in &messages {
            let msg_json = serde_json::to_string(msg)?;
            file.write_all(msg_json.as_bytes()).await?;
            file.write_all(b"\n").await?;
        }

        file.flush().await?;
        drop(file);

        // Atomic rename
        fs::rename(&temp_path, &path).await?;

        debug!("Saved session {} to {:?}", session.key.session_id, path);
        Ok(())
    }

    async fn delete(&self, key: &SessionKey) -> Result<()> {
        let path = self.session_path(key);
        if path.exists() {
            fs::remove_file(&path).await?;
        }
        Ok(())
    }

    async fn list(&self, agent_id: &AgentId) -> Result<Vec<SessionKey>> {
        let dir = self.base_dir.join(&agent_id.to_string()).join("sessions");

        if !dir.exists() {
            return Ok(Vec::new());
        }

        let mut keys = Vec::new();
        let mut entries = fs::read_dir(&dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().map(|e| e == "jsonl").unwrap_or(false) {
                if let Some(stem) = path.file_stem() {
                    keys.push(SessionKey {
                        agent_id: agent_id.clone(),
                        session_id: stem.to_string_lossy().to_string(),
                    });
                }
            }
        }

        Ok(keys)
    }

    async fn exists(&self, key: &SessionKey) -> Result<bool> {
        Ok(self.session_path(key).exists())
    }
}

/// In-memory session store (for testing).
pub struct MemorySessionStore {
    sessions: RwLock<HashMap<String, Session>>,
}

impl Default for MemorySessionStore {
    fn default() -> Self {
        Self::new()
    }
}

impl MemorySessionStore {
    /// Create a new in-memory session store.
    pub fn new() -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
        }
    }

    fn key_string(key: &SessionKey) -> String {
        format!("{}:{}", key.agent_id, key.session_id)
    }
}

#[async_trait::async_trait]
impl SessionStore for MemorySessionStore {
    async fn load(&self, key: &SessionKey) -> Result<Option<Session>> {
        let sessions = self.sessions.read().await;
        Ok(sessions.get(&Self::key_string(key)).cloned())
    }

    async fn save(&self, session: &Session) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        sessions.insert(Self::key_string(&session.key), session.clone());
        Ok(())
    }

    async fn delete(&self, key: &SessionKey) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        sessions.remove(&Self::key_string(key));
        Ok(())
    }

    async fn list(&self, agent_id: &AgentId) -> Result<Vec<SessionKey>> {
        let sessions = self.sessions.read().await;
        let prefix = format!("{}:", agent_id);
        Ok(sessions
            .keys()
            .filter(|k| k.starts_with(&prefix))
            .map(|k| {
                let parts: Vec<&str> = k.splitn(2, ':').collect();
                SessionKey {
                    agent_id: agent_id.clone(),
                    session_id: parts.get(1).unwrap_or(&"").to_string(),
                }
            })
            .collect())
    }

    async fn exists(&self, key: &SessionKey) -> Result<bool> {
        let sessions = self.sessions.read().await;
        Ok(sessions.contains_key(&Self::key_string(key)))
    }
}

/// Session manager for creating and managing sessions.
pub struct SessionManager {
    /// Session store.
    store: Arc<dyn SessionStore>,

    /// Active sessions cache.
    active: RwLock<HashMap<String, Arc<RwLock<Session>>>>,
}

impl SessionManager {
    /// Create a new session manager.
    pub fn new(store: Arc<dyn SessionStore>) -> Self {
        Self {
            store,
            active: RwLock::new(HashMap::new()),
        }
    }

    /// Get or create a session.
    pub async fn get_or_create(
        &self,
        key: SessionKey,
    ) -> Result<Arc<RwLock<Session>>> {
        // Check active cache
        {
            let active = self.active.read().await;
            if let Some(session) = active.get(&Self::key_string(&key)) {
                return Ok(session.clone());
            }
        }

        // Try to load from store
        let session = match self.store.load(&key).await? {
            Some(s) => s,
            None => Session::new(key.clone(), key.agent_id.clone()),
        };

        let session = Arc::new(RwLock::new(session));

        // Add to active cache
        let mut active = self.active.write().await;
        active.insert(Self::key_string(&key), session.clone());

        Ok(session)
    }

    /// Get an existing session.
    pub async fn get(&self, key: &SessionKey) -> Result<Option<Arc<RwLock<Session>>>> {
        // Check active cache
        {
            let active = self.active.read().await;
            if let Some(session) = active.get(&Self::key_string(key)) {
                return Ok(Some(session.clone()));
            }
        }

        // Try to load from store
        match self.store.load(key).await? {
            Some(session) => {
                let session = Arc::new(RwLock::new(session));
                let mut active = self.active.write().await;
                active.insert(Self::key_string(key), session.clone());
                Ok(Some(session))
            }
            None => Ok(None),
        }
    }

    /// Save a session.
    pub async fn save(&self, key: &SessionKey) -> Result<()> {
        let active = self.active.read().await;
        if let Some(session_lock) = active.get(&Self::key_string(key)) {
            let session = session_lock.read().await;
            self.store.save(&session).await?;
        }
        Ok(())
    }

    /// Delete a session.
    pub async fn delete(&self, key: &SessionKey) -> Result<()> {
        // Remove from active cache
        {
            let mut active = self.active.write().await;
            active.remove(&Self::key_string(key));
        }

        // Delete from store
        self.store.delete(key).await
    }

    /// List sessions for an agent.
    pub async fn list(&self, agent_id: &AgentId) -> Result<Vec<SessionKey>> {
        self.store.list(agent_id).await
    }

    fn key_string(key: &SessionKey) -> String {
        format!("{}:{}", key.agent_id, key.session_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_creation() {
        let key = SessionKey {
            agent_id: AgentId::new("agent1"),
            session_id: "session1".to_string(),
        };
        let session = Session::new(key.clone(), AgentId::new("agent1"));

        assert_eq!(session.key.session_id, "session1");
        assert_eq!(session.state, SessionState::Active);
        assert!(session.messages.is_empty());
    }

    #[test]
    fn test_session_messages() {
        let key = SessionKey {
            agent_id: AgentId::new("agent1"),
            session_id: "session1".to_string(),
        };
        let mut session = Session::new(key, AgentId::new("agent1"));

        session.add_user_message("Hello");
        session.add_assistant_message("Hi there!");

        assert_eq!(session.messages.len(), 2);
        assert_eq!(session.messages[0].role, Role::User);
        assert_eq!(session.messages[1].role, Role::Assistant);
    }

    #[tokio::test]
    async fn test_memory_store() {
        let store = MemorySessionStore::new();
        let key = SessionKey {
            agent_id: AgentId::new("agent1"),
            session_id: "session1".to_string(),
        };

        let mut session = Session::new(key.clone(), AgentId::new("agent1"));
        session.add_user_message("Test");

        store.save(&session).await.unwrap();

        let loaded = store.load(&key).await.unwrap().unwrap();
        assert_eq!(loaded.messages.len(), 1);
    }
}
