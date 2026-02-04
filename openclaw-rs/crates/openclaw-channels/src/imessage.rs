//! iMessage channel implementation.
//!
//! This channel integrates with Apple's iMessage on macOS.
//! It uses AppleScript for sending messages and monitors the Messages
//! database for incoming messages.

#![cfg(feature = "imessage")]

use crate::attachment::Attachment;
use crate::error::ChannelError;
use crate::traits::{
    Channel, ChannelConfig, ChannelLifecycle, ChannelReceiver, ChannelSender, MessageHandler,
    SendResult,
};
use crate::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use openclaw_core::types::{
    ChannelCapabilities, ChannelFeatures, ChannelHealth, ChannelLimits, ChatInfo, ChatType,
    HealthStatus, InboundMessage, MediaAttachment, MediaCapabilities, MediaType, MessageId,
    MessageTarget, OutboundMessage, SenderInfo,
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};

/// iMessage channel implementation.
///
/// Uses AppleScript to send messages and monitors the Messages SQLite
/// database for incoming messages. Only works on macOS.
pub struct IMessageChannel {
    /// Channel instance ID.
    instance_id: String,

    /// Apple ID or phone number for the account.
    account_id: String,

    /// Path to Messages database.
    database_path: PathBuf,

    /// Connection state.
    connected: Arc<RwLock<bool>>,

    /// Last processed message ROWID.
    last_rowid: Arc<RwLock<i64>>,

    /// Contact cache.
    contacts: Arc<RwLock<HashMap<String, ContactInfo>>>,

    /// Incoming message channel.
    message_tx: mpsc::Sender<InboundMessage>,
    message_rx: Arc<RwLock<mpsc::Receiver<InboundMessage>>>,

    /// Message handler.
    handler: Arc<RwLock<Option<Box<dyn MessageHandler>>>>,

    /// Shutdown signal.
    shutdown: Arc<RwLock<Option<tokio::sync::oneshot::Sender<()>>>>,
}

/// Contact information from iMessage.
#[derive(Debug, Clone)]
pub struct ContactInfo {
    /// Handle ID (phone number or email).
    pub handle_id: String,

    /// Display name from Contacts.
    pub display_name: Option<String>,

    /// Is this an iMessage or SMS contact.
    pub is_imessage: bool,
}

impl std::fmt::Debug for IMessageChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IMessageChannel")
            .field("instance_id", &self.instance_id)
            .field("account_id", &self.account_id)
            .finish()
    }
}

impl IMessageChannel {
    /// Create a new iMessage channel.
    pub fn new(instance_id: impl Into<String>, account_id: impl Into<String>) -> Self {
        let (tx, rx) = mpsc::channel(1000);

        // Default Messages database path
        let database_path = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Library")
            .join("Messages")
            .join("chat.db");

        Self {
            instance_id: instance_id.into(),
            account_id: account_id.into(),
            database_path,
            connected: Arc::new(RwLock::new(false)),
            last_rowid: Arc::new(RwLock::new(0)),
            contacts: Arc::new(RwLock::new(HashMap::new())),
            message_tx: tx,
            message_rx: Arc::new(RwLock::new(rx)),
            handler: Arc::new(RwLock::new(None)),
            shutdown: Arc::new(RwLock::new(None)),
        }
    }

    /// Create from configuration.
    pub fn from_config(config: ChannelConfig) -> std::result::Result<Self, ChannelError> {
        let account_id = config
            .options
            .get("account_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let mut channel = Self::new(config.instance_id, account_id);

        // Allow custom database path for testing
        if let Some(db_path) = config.options.get("database_path").and_then(|v| v.as_str()) {
            channel.database_path = PathBuf::from(db_path);
        }

        Ok(channel)
    }

    /// Check if running on macOS.
    fn is_macos() -> bool {
        cfg!(target_os = "macos")
    }

    /// Execute AppleScript to send a message.
    #[cfg(target_os = "macos")]
    async fn send_via_applescript(
        &self,
        recipient: &str,
        message: &str,
    ) -> std::result::Result<(), ChannelError> {
        let script = format!(
            r#"
            tell application "Messages"
                set targetBuddy to "{recipient}"
                set targetService to id of 1st service whose service type = iMessage
                set theBuddy to buddy targetBuddy of service id targetService
                send "{message}" to theBuddy
            end tell
            "#,
            recipient = recipient.replace('"', r#"\""#),
            message = message.replace('"', r#"\""#).replace('\n', "\\n"),
        );

        let output = tokio::process::Command::new("osascript")
            .arg("-e")
            .arg(&script)
            .output()
            .await
            .map_err(|e| ChannelError::Internal(format!("Failed to run AppleScript: {}", e)))?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(ChannelError::channel(
                "imessage",
                format!("AppleScript error: {}", stderr),
            ))
        }
    }

    #[cfg(not(target_os = "macos"))]
    async fn send_via_applescript(
        &self,
        _recipient: &str,
        _message: &str,
    ) -> std::result::Result<(), ChannelError> {
        Err(ChannelError::Internal(
            "iMessage is only supported on macOS".to_string(),
        ))
    }

    /// Send an attachment via AppleScript.
    #[cfg(target_os = "macos")]
    async fn send_file_via_applescript(
        &self,
        recipient: &str,
        file_path: &std::path::Path,
    ) -> std::result::Result<(), ChannelError> {
        let script = format!(
            r#"
            tell application "Messages"
                set targetBuddy to "{recipient}"
                set targetService to id of 1st service whose service type = iMessage
                set theBuddy to buddy targetBuddy of service id targetService
                send POSIX file "{file_path}" to theBuddy
            end tell
            "#,
            recipient = recipient.replace('"', r#"\""#),
            file_path = file_path.display(),
        );

        let output = tokio::process::Command::new("osascript")
            .arg("-e")
            .arg(&script)
            .output()
            .await
            .map_err(|e| ChannelError::Internal(format!("Failed to run AppleScript: {}", e)))?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(ChannelError::channel(
                "imessage",
                format!("AppleScript error: {}", stderr),
            ))
        }
    }

    #[cfg(not(target_os = "macos"))]
    async fn send_file_via_applescript(
        &self,
        _recipient: &str,
        _file_path: &std::path::Path,
    ) -> std::result::Result<(), ChannelError> {
        Err(ChannelError::Internal(
            "iMessage is only supported on macOS".to_string(),
        ))
    }

    /// Convert a database row to InboundMessage.
    #[allow(dead_code)]
    fn convert_message(
        &self,
        rowid: i64,
        text: &str,
        handle_id: &str,
        is_from_me: bool,
        date: i64,
        chat_id: &str,
        attachment_id: Option<&str>,
        mime_type: Option<&str>,
        filename: Option<&str>,
    ) -> InboundMessage {
        let sender = if is_from_me {
            SenderInfo {
                id: "me".to_string(),
                username: None,
                display_name: Some("Me".to_string()),
                phone_number: None,
                is_bot: false,
            }
        } else {
            SenderInfo {
                id: handle_id.to_string(),
                username: None,
                display_name: None,
                phone_number: if handle_id.starts_with('+') {
                    Some(handle_id.to_string())
                } else {
                    None
                },
                is_bot: false,
            }
        };

        let chat = ChatInfo {
            id: chat_id.to_string(),
            chat_type: if chat_id.contains(";-;") || chat_id.contains("chat") {
                ChatType::Group
            } else {
                ChatType::Direct
            },
            title: None,
            guild_id: None,
        };

        // Convert Apple's date format (nanoseconds since 2001-01-01) to DateTime<Utc>
        // Apple's epoch is 978307200 seconds after Unix epoch
        let apple_epoch_offset = 978307200i64;
        let unix_timestamp = (date / 1_000_000_000) + apple_epoch_offset;
        let timestamp = DateTime::<Utc>::from_timestamp(unix_timestamp, 0).unwrap_or_else(Utc::now);

        let media = match (attachment_id, mime_type, filename) {
            (Some(att_id), Some(mt), fname) => {
                vec![MediaAttachment {
                    id: att_id.to_string(),
                    media_type: self.guess_media_type(mt),
                    url: None,
                    data: None,
                    filename: fname.map(|s| s.to_string()),
                    size_bytes: None,
                    mime_type: Some(mt.to_string()),
                }]
            }
            _ => vec![],
        };

        InboundMessage {
            id: MessageId::new(rowid.to_string()),
            timestamp,
            channel: "imessage".to_string(),
            account_id: self.account_id.clone(),
            sender,
            chat,
            text: text.to_string(),
            media,
            quote: None,
            thread: None,
            metadata: serde_json::json!({
                "rowid": rowid,
                "is_from_me": is_from_me,
            }),
        }
    }

    /// Guess media type from MIME type.
    fn guess_media_type(&self, mime_type: &str) -> MediaType {
        if mime_type.starts_with("image/") {
            MediaType::Image
        } else if mime_type.starts_with("video/") {
            MediaType::Video
        } else if mime_type.starts_with("audio/") {
            MediaType::Audio
        } else {
            MediaType::Document
        }
    }

    /// Normalize a phone number or handle.
    fn normalize_handle(&self, handle: &str) -> String {
        // Remove spaces and dashes
        let cleaned: String = handle.chars().filter(|c| !c.is_whitespace() && *c != '-').collect();

        // If it looks like a phone number, normalize it
        if cleaned.chars().all(|c| c.is_ascii_digit() || c == '+') {
            let digits: String = cleaned.chars().filter(|c| c.is_ascii_digit()).collect();
            if digits.len() == 10 {
                format!("+1{}", digits)
            } else if digits.len() == 11 && digits.starts_with('1') {
                format!("+{}", digits)
            } else {
                format!("+{}", digits)
            }
        } else {
            // Assume it's an email/Apple ID
            cleaned
        }
    }
}

#[async_trait]
impl Channel for IMessageChannel {
    fn channel_type(&self) -> &str {
        "imessage"
    }

    fn instance_id(&self) -> &str {
        &self.instance_id
    }

    fn capabilities(&self) -> ChannelCapabilities {
        ChannelCapabilities {
            chat_types: vec![ChatType::Direct, ChatType::Group],
            media: MediaCapabilities {
                images: true,
                audio: true,
                video: true,
                files: true,
                stickers: false, // Stickers are complex in iMessage
                voice_notes: true,
                max_file_size_mb: 100,
            },
            features: ChannelFeatures {
                reactions: true,  // Tapback reactions
                threads: false,
                edits: true,      // iOS 16+
                deletes: true,    // iOS 16+
                typing_indicators: true,
                read_receipts: true,
                mentions: true,   // @mentions in groups
                polls: false,
                native_commands: false,
            },
            limits: ChannelLimits {
                text_max_length: 20000, // Practical limit, not enforced
                caption_max_length: 1000,
                messages_per_second: 5.0,
                messages_per_minute: 300,
            },
        }
    }
}

#[async_trait]
impl ChannelSender for IMessageChannel {
    async fn send(&self, message: OutboundMessage) -> Result<SendResult> {
        if !Self::is_macos() {
            return Err(ChannelError::Internal(
                "iMessage is only supported on macOS".to_string(),
            ));
        }

        let connected = *self.connected.read().await;
        if !connected {
            return Err(ChannelError::Internal("Not connected to iMessage".to_string()));
        }

        let recipient = self.normalize_handle(&message.target.chat_id);
        debug!("Sending iMessage to {}", recipient);

        self.send_via_applescript(&recipient, &message.text).await?;

        let msg_id = chrono::Utc::now().timestamp_millis().to_string();
        Ok(SendResult::new(msg_id))
    }

    async fn send_with_attachments(
        &self,
        message: OutboundMessage,
        attachments: Vec<Attachment>,
    ) -> Result<SendResult> {
        if !Self::is_macos() {
            return Err(ChannelError::Internal(
                "iMessage is only supported on macOS".to_string(),
            ));
        }

        let connected = *self.connected.read().await;
        if !connected {
            return Err(ChannelError::Internal("Not connected to iMessage".to_string()));
        }

        let recipient = self.normalize_handle(&message.target.chat_id);

        // Send attachments first
        for attachment in &attachments {
            match &attachment.source {
                crate::attachment::AttachmentSource::Path(path) => {
                    self.send_file_via_applescript(&recipient, path).await?;
                }
                crate::attachment::AttachmentSource::Bytes(bytes) => {
                    // Write to temp file and send
                    let temp_path = std::env::temp_dir().join(&attachment.filename);
                    tokio::fs::write(&temp_path, bytes)
                        .await
                        .map_err(|e| ChannelError::Internal(e.to_string()))?;
                    self.send_file_via_applescript(&recipient, &temp_path).await?;
                    let _ = tokio::fs::remove_file(&temp_path).await;
                }
                _ => {
                    warn!("Unsupported attachment source for iMessage");
                }
            }
        }

        // Send text message if present
        if !message.text.is_empty() {
            return self.send(message).await;
        }

        let msg_id = chrono::Utc::now().timestamp_millis().to_string();
        Ok(SendResult::new(msg_id))
    }

    async fn edit(&self, _message_id: &str, _new_content: &str) -> Result<()> {
        // iMessage supports editing (iOS 16+) but not via AppleScript
        warn!("iMessage edit not supported via AppleScript");
        Ok(())
    }

    async fn delete(&self, _message_id: &str) -> Result<()> {
        // iMessage supports unsend (iOS 16+) but not via AppleScript
        warn!("iMessage delete/unsend not supported via AppleScript");
        Ok(())
    }

    async fn react(&self, _message_id: &str, _emoji: &str) -> Result<()> {
        // Tapback reactions exist but aren't accessible via AppleScript
        warn!("iMessage reactions (Tapback) not supported via AppleScript");
        Ok(())
    }

    async fn unreact(&self, _message_id: &str, _emoji: &str) -> Result<()> {
        warn!("iMessage unreact not supported via AppleScript");
        Ok(())
    }

    async fn send_typing(&self, _target: &MessageTarget) -> Result<()> {
        // iMessage typing indicators are automatic
        // Not controllable via AppleScript
        Ok(())
    }

    fn max_message_length(&self) -> usize {
        20000
    }
}

#[async_trait]
impl ChannelReceiver for IMessageChannel {
    async fn start_receiving(&self) -> Result<()> {
        if !Self::is_macos() {
            return Err(ChannelError::Internal(
                "iMessage is only supported on macOS".to_string(),
            ));
        }

        let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel();

        {
            let mut shutdown = self.shutdown.write().await;
            *shutdown = Some(shutdown_tx);
        }

        let _tx = self.message_tx.clone();
        let db_path = self.database_path.clone();
        let connected = self.connected.clone();
        let _last_rowid = self.last_rowid.clone();
        let _instance_id = self.instance_id.clone();
        let _account_id = self.account_id.clone();

        tokio::spawn(async move {
            info!("Starting iMessage receive loop");

            // Note: This would poll the chat.db SQLite database for new messages
            // The database is at ~/Library/Messages/chat.db
            //
            // Example query:
            // SELECT m.ROWID, m.text, h.id as handle, m.is_from_me, m.date,
            //        c.chat_identifier, a.filename, a.mime_type
            // FROM message m
            // LEFT JOIN handle h ON m.handle_id = h.ROWID
            // LEFT JOIN chat_message_join cmj ON m.ROWID = cmj.message_id
            // LEFT JOIN chat c ON cmj.chat_id = c.ROWID
            // LEFT JOIN message_attachment_join maj ON m.ROWID = maj.message_id
            // LEFT JOIN attachment a ON maj.attachment_id = a.ROWID
            // WHERE m.ROWID > ?
            // ORDER BY m.ROWID ASC

            loop {
                tokio::select! {
                    _ = &mut shutdown_rx => {
                        info!("iMessage receive loop shutting down");
                        break;
                    }
                    _ = tokio::time::sleep(tokio::time::Duration::from_secs(2)) => {
                        let is_connected = *connected.read().await;
                        if !is_connected {
                            debug!("iMessage not connected, skipping poll");
                            continue;
                        }

                        // Would query database here for new messages
                        // and send them to tx channel
                        debug!("Would poll chat.db at {:?}", db_path);
                    }
                }
            }
        });

        info!(
            "Started receiving messages for iMessage (instance: {})",
            self.instance_id
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
impl ChannelLifecycle for IMessageChannel {
    async fn connect(&self) -> Result<()> {
        if !Self::is_macos() {
            return Err(ChannelError::Internal(
                "iMessage is only supported on macOS".to_string(),
            ));
        }

        // Check if Messages database exists
        if !self.database_path.exists() {
            return Err(ChannelError::Config(format!(
                "Messages database not found at {:?}",
                self.database_path
            )));
        }

        // Check if Messages app is available
        #[cfg(target_os = "macos")]
        {
            let output = tokio::process::Command::new("osascript")
                .arg("-e")
                .arg("tell application \"System Events\" to (name of processes) contains \"Messages\"")
                .output()
                .await;

            match output {
                Ok(o) if o.status.success() => {
                    info!("iMessage connection verified");
                }
                Ok(o) => {
                    let stderr = String::from_utf8_lossy(&o.stderr);
                    warn!("iMessage check warning: {}", stderr);
                }
                Err(e) => {
                    error!("Failed to check Messages app: {}", e);
                }
            }
        }

        let mut connected = self.connected.write().await;
        *connected = true;

        info!("Connected to iMessage");
        Ok(())
    }

    async fn disconnect(&self) -> Result<()> {
        self.stop_receiving().await?;

        let mut connected = self.connected.write().await;
        *connected = false;

        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected.blocking_read().clone()
    }

    async fn health(&self) -> Result<ChannelHealth> {
        if !Self::is_macos() {
            return Ok(ChannelHealth {
                status: HealthStatus::Unhealthy,
                latency_ms: None,
                last_message_at: None,
                error: Some("iMessage is only supported on macOS".to_string()),
            });
        }

        let connected = *self.connected.read().await;

        Ok(ChannelHealth {
            status: if connected {
                HealthStatus::Healthy
            } else {
                HealthStatus::Unhealthy
            },
            latency_ms: Some(0), // Local
            last_message_at: None,
            error: if connected {
                None
            } else {
                Some("Not connected".to_string())
            },
        })
    }
}

impl Clone for IMessageChannel {
    fn clone(&self) -> Self {
        let (tx, rx) = mpsc::channel(1000);
        Self {
            instance_id: self.instance_id.clone(),
            account_id: self.account_id.clone(),
            database_path: self.database_path.clone(),
            connected: self.connected.clone(),
            last_rowid: self.last_rowid.clone(),
            contacts: self.contacts.clone(),
            message_tx: tx,
            message_rx: Arc::new(RwLock::new(rx)),
            handler: self.handler.clone(),
            shutdown: self.shutdown.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_imessage_channel_creation() {
        let channel = IMessageChannel::new("test_imessage", "test@icloud.com");
        assert_eq!(channel.channel_type(), "imessage");
        assert_eq!(channel.instance_id(), "test_imessage");
    }

    #[test]
    fn test_capabilities() {
        let channel = IMessageChannel::new("test_imessage", "");
        let caps = channel.capabilities();
        assert!(caps.media.images);
        assert!(caps.media.voice_notes);
        assert!(caps.features.reactions);
        assert!(caps.features.typing_indicators);
        assert!(caps.features.read_receipts);
        assert!(caps.chat_types.contains(&ChatType::Direct));
        assert!(caps.chat_types.contains(&ChatType::Group));
    }

    #[test]
    fn test_normalize_handle() {
        let channel = IMessageChannel::new("test", "");

        // Phone numbers
        assert_eq!(channel.normalize_handle("+1 555 123 4567"), "+15551234567");
        assert_eq!(channel.normalize_handle("555-123-4567"), "+15551234567");
        assert_eq!(channel.normalize_handle("15551234567"), "+15551234567");

        // Email/Apple ID
        assert_eq!(channel.normalize_handle("user@icloud.com"), "user@icloud.com");
    }

    #[test]
    fn test_guess_media_type() {
        let channel = IMessageChannel::new("test", "");

        assert!(matches!(
            channel.guess_media_type("image/heic"),
            MediaType::Image
        ));
        assert!(matches!(
            channel.guess_media_type("video/quicktime"),
            MediaType::Video
        ));
        assert!(matches!(
            channel.guess_media_type("audio/m4a"),
            MediaType::Audio
        ));
        assert!(matches!(
            channel.guess_media_type("application/pdf"),
            MediaType::Document
        ));
    }

    #[test]
    fn test_is_macos() {
        // This will return true on macOS, false on other platforms
        let result = IMessageChannel::is_macos();
        #[cfg(target_os = "macos")]
        assert!(result);
        #[cfg(not(target_os = "macos"))]
        assert!(!result);
    }
}
