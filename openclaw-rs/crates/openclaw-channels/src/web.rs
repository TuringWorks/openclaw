//! Web channel implementation using WebSocket.

#![cfg(feature = "web")]

use crate::attachment::Attachment;
use crate::error::ChannelError;
use crate::traits::{
    Channel, ChannelConfig, ChannelLifecycle, ChannelReceiver, ChannelSender, MessageHandler,
    SendResult,
};
use crate::Result;
use async_trait::async_trait;
use chrono::Utc;
use openclaw_core::types::{
    ChannelCapabilities, ChannelFeatures, ChannelHealth, ChannelLimits, ChatType,
    HealthStatus, InboundMessage, MediaCapabilities, MessageTarget, OutboundMessage,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, RwLock};
use tracing::{info, warn};

/// Web channel implementation using WebSocket.
pub struct WebChannel {
    /// Channel instance ID.
    instance_id: String,

    /// Bind address for WebSocket server.
    bind_address: String,

    /// Connection state.
    connected: Arc<RwLock<bool>>,

    /// Connected clients.
    clients: Arc<RwLock<HashMap<String, WebClient>>>,

    /// Incoming message channel.
    #[allow(dead_code)]
    message_tx: mpsc::Sender<InboundMessage>,
    message_rx: Arc<RwLock<mpsc::Receiver<InboundMessage>>>,

    /// Broadcast channel for outgoing messages.
    broadcast_tx: broadcast::Sender<String>,

    /// Message handler.
    handler: Arc<RwLock<Option<Box<dyn MessageHandler>>>>,

    /// Shutdown signal.
    shutdown: Arc<RwLock<Option<tokio::sync::oneshot::Sender<()>>>>,
}

/// A connected web client.
#[derive(Debug, Clone)]
pub struct WebClient {
    /// Client ID.
    pub id: String,

    /// Display name.
    pub name: Option<String>,

    /// Connected timestamp.
    pub connected_at: chrono::DateTime<Utc>,
}

impl std::fmt::Debug for WebChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebChannel")
            .field("instance_id", &self.instance_id)
            .field("bind_address", &self.bind_address)
            .finish()
    }
}

impl WebChannel {
    /// Create a new Web channel.
    pub fn new(instance_id: impl Into<String>, bind_address: impl Into<String>) -> Self {
        let (message_tx, message_rx) = mpsc::channel(1000);
        let (broadcast_tx, _) = broadcast::channel(1000);

        Self {
            instance_id: instance_id.into(),
            bind_address: bind_address.into(),
            connected: Arc::new(RwLock::new(false)),
            clients: Arc::new(RwLock::new(HashMap::new())),
            message_tx,
            message_rx: Arc::new(RwLock::new(message_rx)),
            broadcast_tx,
            handler: Arc::new(RwLock::new(None)),
            shutdown: Arc::new(RwLock::new(None)),
        }
    }

    /// Create from configuration.
    pub fn from_config(config: ChannelConfig) -> Self {
        let bind_address = config
            .options
            .get("bind_address")
            .and_then(|v| v.as_str())
            .unwrap_or("127.0.0.1:8080")
            .to_string();

        Self::new(config.instance_id, bind_address)
    }

    /// Get the number of connected clients.
    pub async fn client_count(&self) -> usize {
        self.clients.read().await.len()
    }

    /// Broadcast a message to all connected clients.
    pub fn broadcast(&self, message: &str) -> std::result::Result<usize, broadcast::error::SendError<String>> {
        self.broadcast_tx.send(message.to_string())
    }
}

#[async_trait]
impl Channel for WebChannel {
    fn channel_type(&self) -> &str {
        "web"
    }

    fn instance_id(&self) -> &str {
        &self.instance_id
    }

    fn capabilities(&self) -> ChannelCapabilities {
        ChannelCapabilities {
            chat_types: vec![ChatType::Direct],
            media: MediaCapabilities {
                images: true,
                audio: true,
                video: true,
                files: true,
                stickers: false,
                voice_notes: true,
                max_file_size_mb: 100,
            },
            features: ChannelFeatures {
                reactions: true,
                threads: false,
                edits: true,
                deletes: true,
                typing_indicators: true,
                read_receipts: true,
                mentions: false,
                polls: false,
                native_commands: false,
            },
            limits: ChannelLimits {
                text_max_length: 100000,
                caption_max_length: 1000,
                messages_per_second: 100.0,
                messages_per_minute: 6000,
            },
        }
    }
}

#[async_trait]
impl ChannelSender for WebChannel {
    async fn send(&self, message: OutboundMessage) -> Result<SendResult> {
        let payload = serde_json::json!({
            "type": "message",
            "text": message.text,
            "target": message.target.chat_id,
            "timestamp": Utc::now().to_rfc3339(),
        });

        let json = serde_json::to_string(&payload)
            .map_err(|e| ChannelError::Internal(e.to_string()))?;

        let _ = self.broadcast_tx.send(json);

        let msg_id = uuid::Uuid::new_v4().to_string();
        Ok(SendResult::new(msg_id))
    }

    async fn send_with_attachments(
        &self,
        message: OutboundMessage,
        attachments: Vec<Attachment>,
    ) -> Result<SendResult> {
        // For web channel, attachments would be sent as base64 or URLs
        warn!(
            "Web channel attachments: {} files (implementation simplified)",
            attachments.len()
        );
        self.send(message).await
    }

    async fn edit(&self, message_id: &str, new_content: &str) -> Result<()> {
        let payload = serde_json::json!({
            "type": "edit",
            "message_id": message_id,
            "text": new_content,
            "timestamp": Utc::now().to_rfc3339(),
        });

        let json = serde_json::to_string(&payload)
            .map_err(|e| ChannelError::Internal(e.to_string()))?;

        let _ = self.broadcast_tx.send(json);
        Ok(())
    }

    async fn delete(&self, message_id: &str) -> Result<()> {
        let payload = serde_json::json!({
            "type": "delete",
            "message_id": message_id,
            "timestamp": Utc::now().to_rfc3339(),
        });

        let json = serde_json::to_string(&payload)
            .map_err(|e| ChannelError::Internal(e.to_string()))?;

        let _ = self.broadcast_tx.send(json);
        Ok(())
    }

    async fn react(&self, message_id: &str, emoji: &str) -> Result<()> {
        let payload = serde_json::json!({
            "type": "react",
            "message_id": message_id,
            "emoji": emoji,
            "timestamp": Utc::now().to_rfc3339(),
        });

        let json = serde_json::to_string(&payload)
            .map_err(|e| ChannelError::Internal(e.to_string()))?;

        let _ = self.broadcast_tx.send(json);
        Ok(())
    }

    async fn unreact(&self, message_id: &str, emoji: &str) -> Result<()> {
        let payload = serde_json::json!({
            "type": "unreact",
            "message_id": message_id,
            "emoji": emoji,
            "timestamp": Utc::now().to_rfc3339(),
        });

        let json = serde_json::to_string(&payload)
            .map_err(|e| ChannelError::Internal(e.to_string()))?;

        let _ = self.broadcast_tx.send(json);
        Ok(())
    }

    async fn send_typing(&self, target: &MessageTarget) -> Result<()> {
        let payload = serde_json::json!({
            "type": "typing",
            "target": target.chat_id,
            "timestamp": Utc::now().to_rfc3339(),
        });

        let json = serde_json::to_string(&payload)
            .map_err(|e| ChannelError::Internal(e.to_string()))?;

        let _ = self.broadcast_tx.send(json);
        Ok(())
    }

    fn max_message_length(&self) -> usize {
        100000
    }
}

#[async_trait]
impl ChannelReceiver for WebChannel {
    async fn start_receiving(&self) -> Result<()> {
        let (shutdown_tx, _shutdown_rx) = tokio::sync::oneshot::channel();

        {
            let mut shutdown = self.shutdown.write().await;
            *shutdown = Some(shutdown_tx);
        }

        // Note: Full WebSocket server implementation would go here
        // This would use tokio-tungstenite or axum to handle WebSocket connections
        warn!(
            "Web channel WebSocket server not fully implemented - bind address: {}",
            self.bind_address
        );

        {
            let mut connected = self.connected.write().await;
            *connected = true;
        }

        info!(
            "Started Web channel on {} (instance: {})",
            self.bind_address, self.instance_id
        );
        Ok(())
    }

    async fn stop_receiving(&self) -> Result<()> {
        let mut shutdown = self.shutdown.write().await;
        if let Some(tx) = shutdown.take() {
            let _ = tx.send(());
        }

        let mut connected = self.connected.write().await;
        *connected = false;

        Ok(())
    }

    async fn receive(&self) -> Result<InboundMessage> {
        let mut rx = self.message_rx.write().await;
        rx.recv()
            .await
            .ok_or_else(|| ChannelError::Internal("Channel closed".to_string()))
    }

    async fn try_receive(&self) -> Result<Option<InboundMessage>> {
        let mut rx = self.message_rx.write().await;
        match rx.try_recv() {
            Ok(msg) => Ok(Some(msg)),
            Err(mpsc::error::TryRecvError::Empty) => Ok(None),
            Err(mpsc::error::TryRecvError::Disconnected) => {
                Err(ChannelError::Internal("Channel closed".to_string()))
            }
        }
    }

    fn set_handler(&self, handler: Box<dyn MessageHandler>) {
        let handler_arc = self.handler.clone();
        tokio::spawn(async move {
            let mut h = handler_arc.write().await;
            *h = Some(handler);
        });
    }
}

#[async_trait]
impl ChannelLifecycle for WebChannel {
    async fn connect(&self) -> Result<()> {
        let mut connected = self.connected.write().await;
        *connected = true;

        info!("Web channel connected: {}", self.instance_id);
        Ok(())
    }

    async fn disconnect(&self) -> Result<()> {
        self.stop_receiving().await?;

        let mut connected = self.connected.write().await;
        *connected = false;

        // Clear clients
        let mut clients = self.clients.write().await;
        clients.clear();

        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected.blocking_read().clone()
    }

    async fn health(&self) -> Result<ChannelHealth> {
        let connected = *self.connected.read().await;
        let _client_count = self.clients.read().await.len();

        Ok(ChannelHealth {
            status: if connected {
                HealthStatus::Healthy
            } else {
                HealthStatus::Unhealthy
            },
            latency_ms: Some(0), // Local connection
            last_message_at: None,
            error: if connected {
                None
            } else {
                Some("Not connected".to_string())
            },
        })
    }
}

impl Clone for WebChannel {
    fn clone(&self) -> Self {
        let (message_tx, message_rx) = mpsc::channel(1000);
        Self {
            instance_id: self.instance_id.clone(),
            bind_address: self.bind_address.clone(),
            connected: self.connected.clone(),
            clients: self.clients.clone(),
            message_tx,
            message_rx: Arc::new(RwLock::new(message_rx)),
            broadcast_tx: self.broadcast_tx.clone(),
            handler: self.handler.clone(),
            shutdown: self.shutdown.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_web_channel_creation() {
        let channel = WebChannel::new("test_web", "127.0.0.1:8080");
        assert_eq!(channel.channel_type(), "web");
        assert_eq!(channel.instance_id(), "test_web");
    }

    #[test]
    fn test_capabilities() {
        let channel = WebChannel::new("test_web", "127.0.0.1:8080");
        let caps = channel.capabilities();
        assert!(caps.media.images);
        assert!(caps.features.typing_indicators);
        assert!(caps.features.edits);
        assert_eq!(caps.limits.text_max_length, 100000);
    }

    #[tokio::test]
    async fn test_connect_disconnect() {
        let channel = WebChannel::new("test_web", "127.0.0.1:8080");

        // Check initial state via the internal lock
        assert!(!*channel.connected.read().await);

        channel.connect().await.unwrap();
        assert!(*channel.connected.read().await);

        channel.disconnect().await.unwrap();
        assert!(!*channel.connected.read().await);
    }
}
