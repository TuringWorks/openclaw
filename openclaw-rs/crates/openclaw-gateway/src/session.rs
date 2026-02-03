//! Gateway session management.

use std::collections::HashMap;
use tokio::sync::RwLock;

/// Gateway session for a WebSocket connection.
#[derive(Debug, Clone)]
pub struct GatewaySession {
    /// Session ID.
    pub id: String,

    /// Associated agent ID.
    pub agent_id: Option<String>,

    /// Session metadata.
    pub metadata: HashMap<String, serde_json::Value>,

    /// Created timestamp.
    pub created_at: chrono::DateTime<chrono::Utc>,

    /// Last activity timestamp.
    pub last_activity: chrono::DateTime<chrono::Utc>,
}

impl GatewaySession {
    /// Create a new gateway session.
    pub fn new(id: impl Into<String>) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: id.into(),
            agent_id: None,
            metadata: HashMap::new(),
            created_at: now,
            last_activity: now,
        }
    }

    /// Set the associated agent.
    pub fn with_agent(mut self, agent_id: impl Into<String>) -> Self {
        self.agent_id = Some(agent_id.into());
        self
    }

    /// Update last activity time.
    pub fn touch(&mut self) {
        self.last_activity = chrono::Utc::now();
    }
}

/// Manager for gateway sessions.
pub struct SessionRegistry {
    /// Active sessions.
    sessions: RwLock<HashMap<String, GatewaySession>>,
}

impl Default for SessionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionRegistry {
    /// Create a new session registry.
    pub fn new() -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
        }
    }

    /// Create and register a new session.
    pub async fn create(&self) -> GatewaySession {
        let session = GatewaySession::new(uuid::Uuid::new_v4().to_string());
        let mut sessions = self.sessions.write().await;
        sessions.insert(session.id.clone(), session.clone());
        session
    }

    /// Get a session by ID.
    pub async fn get(&self, id: &str) -> Option<GatewaySession> {
        let sessions = self.sessions.read().await;
        sessions.get(id).cloned()
    }

    /// Remove a session.
    pub async fn remove(&self, id: &str) {
        let mut sessions = self.sessions.write().await;
        sessions.remove(id);
    }

    /// Update a session.
    pub async fn update(&self, session: GatewaySession) {
        let mut sessions = self.sessions.write().await;
        sessions.insert(session.id.clone(), session);
    }

    /// Get all session IDs.
    pub async fn list(&self) -> Vec<String> {
        let sessions = self.sessions.read().await;
        sessions.keys().cloned().collect()
    }

    /// Get session count.
    pub async fn count(&self) -> usize {
        let sessions = self.sessions.read().await;
        sessions.len()
    }
}
