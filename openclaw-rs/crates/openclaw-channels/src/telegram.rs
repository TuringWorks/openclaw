//! Telegram channel implementation.

#![cfg(feature = "telegram")]

use crate::attachment::{Attachment, AttachmentType};
use crate::error::ChannelError;
use crate::traits::{
    Channel, ChannelConfig, ChannelFeature, ChannelLifecycle, ChannelReceiver, ChannelSender,
    MessageHandler, SendResult,
};
use crate::Result;
use async_trait::async_trait;
use openclaw_core::types::{
    ChannelCapabilities, ChannelHealth, ChatType, InboundMessage, MediaAttachment, MessageSender,
    MessageTarget, OutboundMessage, SenderType,
};
use std::collections::HashMap;
use std::sync::Arc;
use teloxide::prelude::*;
use teloxide::types::{ChatId, InputFile, MediaKind, MessageKind, ParseMode, UpdateKind};
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};

/// Telegram channel implementation.
pub struct TelegramChannel {
    /// Bot instance.
    bot: Bot,

    /// Channel instance ID.
    instance_id: String,

    /// Bot username.
    username: Option<String>,

    /// Connection state.
    connected: Arc<RwLock<bool>>,

    /// Incoming message channel.
    message_tx: mpsc::Sender<InboundMessage>,
    message_rx: Arc<RwLock<mpsc::Receiver<InboundMessage>>>,

    /// Message handler.
    handler: Arc<RwLock<Option<Box<dyn MessageHandler>>>>,

    /// Shutdown signal.
    shutdown: Arc<RwLock<Option<tokio::sync::oneshot::Sender<()>>>>,
}

impl std::fmt::Debug for TelegramChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TelegramChannel")
            .field("instance_id", &self.instance_id)
            .field("username", &self.username)
            .finish()
    }
}

impl TelegramChannel {
    /// Create a new Telegram channel.
    pub fn new(bot_token: impl Into<String>, instance_id: impl Into<String>) -> Self {
        let (tx, rx) = mpsc::channel(1000);

        Self {
            bot: Bot::new(bot_token),
            instance_id: instance_id.into(),
            username: None,
            connected: Arc::new(RwLock::new(false)),
            message_tx: tx,
            message_rx: Arc::new(RwLock::new(rx)),
            handler: Arc::new(RwLock::new(None)),
            shutdown: Arc::new(RwLock::new(None)),
        }
    }

    /// Create from configuration.
    pub fn from_config(config: ChannelConfig, bot_token: String) -> Self {
        Self::new(bot_token, config.instance_id)
    }

    /// Convert Telegram message to InboundMessage.
    async fn convert_message(&self, msg: &teloxide::types::Message) -> Option<InboundMessage> {
        let from = msg.from.as_ref()?;

        let sender = MessageSender {
            sender_type: if from.is_bot {
                SenderType::Bot
            } else {
                SenderType::User
            },
            id: from.id.to_string(),
            username: from.username.clone(),
            display_name: Some(
                from.last_name
                    .as_ref()
                    .map(|ln| format!("{} {}", from.first_name, ln))
                    .unwrap_or_else(|| from.first_name.clone()),
            ),
            phone: None,
        };

        let chat_type = match &msg.chat.kind {
            teloxide::types::ChatKind::Private(_) => ChatType::Private,
            teloxide::types::ChatKind::Public(public) => match &public.kind {
                teloxide::types::PublicChatKind::Group(_) => ChatType::Group,
                teloxide::types::PublicChatKind::Supergroup(_) => ChatType::Group,
                teloxide::types::PublicChatKind::Channel(_) => ChatType::Channel,
            },
        };

        let text = msg.text().unwrap_or_default().to_string();

        let attachments = self.extract_attachments(msg).await;

        Some(InboundMessage {
            channel: "telegram".to_string(),
            account: self.instance_id.clone(),
            sender,
            chat_type,
            chat_id: msg.chat.id.to_string(),
            guild: None,
            message_id: msg.id.to_string(),
            reply_to: msg.reply_to_message.as_ref().map(|m| m.id.to_string()),
            text,
            attachments,
            timestamp: chrono::Utc::now(),
            raw: Some(serde_json::to_value(msg).ok()?),
        })
    }

    /// Extract attachments from a message.
    async fn extract_attachments(&self, msg: &teloxide::types::Message) -> Vec<MediaAttachment> {
        let mut attachments = Vec::new();

        if let MessageKind::Common(common) = &msg.kind {
            match &common.media_kind {
                MediaKind::Photo(photo) => {
                    if let Some(largest) = photo.photo.last() {
                        attachments.push(MediaAttachment {
                            media_type: "image".to_string(),
                            url: None,
                            file_id: Some(largest.file.id.clone()),
                            filename: None,
                            size: largest.file.size.map(|s| s as u64),
                            caption: photo.caption.clone(),
                        });
                    }
                }
                MediaKind::Document(doc) => {
                    attachments.push(MediaAttachment {
                        media_type: doc
                            .document
                            .mime_type
                            .as_ref()
                            .map(|m| m.to_string())
                            .unwrap_or_else(|| "application/octet-stream".to_string()),
                        url: None,
                        file_id: Some(doc.document.file.id.clone()),
                        filename: doc.document.file_name.clone(),
                        size: doc.document.file.size.map(|s| s as u64),
                        caption: doc.caption.clone(),
                    });
                }
                MediaKind::Audio(audio) => {
                    attachments.push(MediaAttachment {
                        media_type: audio
                            .audio
                            .mime_type
                            .as_ref()
                            .map(|m| m.to_string())
                            .unwrap_or_else(|| "audio/mpeg".to_string()),
                        url: None,
                        file_id: Some(audio.audio.file.id.clone()),
                        filename: audio.audio.file_name.clone(),
                        size: audio.audio.file.size.map(|s| s as u64),
                        caption: audio.caption.clone(),
                    });
                }
                MediaKind::Voice(voice) => {
                    attachments.push(MediaAttachment {
                        media_type: voice
                            .voice
                            .mime_type
                            .as_ref()
                            .map(|m| m.to_string())
                            .unwrap_or_else(|| "audio/ogg".to_string()),
                        url: None,
                        file_id: Some(voice.voice.file.id.clone()),
                        filename: None,
                        size: voice.voice.file.size.map(|s| s as u64),
                        caption: None,
                    });
                }
                MediaKind::Video(video) => {
                    attachments.push(MediaAttachment {
                        media_type: video
                            .video
                            .mime_type
                            .as_ref()
                            .map(|m| m.to_string())
                            .unwrap_or_else(|| "video/mp4".to_string()),
                        url: None,
                        file_id: Some(video.video.file.id.clone()),
                        filename: video.video.file_name.clone(),
                        size: video.video.file.size.map(|s| s as u64),
                        caption: video.caption.clone(),
                    });
                }
                _ => {}
            }
        }

        attachments
    }
}

#[async_trait]
impl Channel for TelegramChannel {
    fn channel_type(&self) -> &str {
        "telegram"
    }

    fn instance_id(&self) -> &str {
        &self.instance_id
    }

    fn capabilities(&self) -> &ChannelCapabilities {
        static CAPS: ChannelCapabilities = ChannelCapabilities {
            markdown: true,
            html: true,
            images: true,
            audio: true,
            video: true,
            files: true,
            reactions: true,
            threads: true,
            edits: true,
            deletes: true,
            buttons: true,
        };
        &CAPS
    }
}

#[async_trait]
impl ChannelSender for TelegramChannel {
    async fn send(&self, message: OutboundMessage) -> Result<SendResult> {
        let chat_id = ChatId(
            message
                .target
                .chat_id
                .parse::<i64>()
                .map_err(|e| ChannelError::InvalidMessage(e.to_string()))?,
        );

        let mut request = self.bot.send_message(chat_id, &message.text);

        // Set parse mode
        if message.format.as_deref() == Some("html") {
            request = request.parse_mode(ParseMode::Html);
        } else if message.format.as_deref() == Some("markdown")
            || message.format.as_deref() == Some("markdown_v2")
        {
            request = request.parse_mode(ParseMode::MarkdownV2);
        }

        // Set reply
        if let Some(ref reply_to) = message.reply_to {
            if let Ok(id) = reply_to.parse::<i32>() {
                request = request.reply_to_message_id(teloxide::types::MessageId(id));
            }
        }

        let sent = request
            .await
            .map_err(|e| ChannelError::channel("telegram", e.to_string()))?;

        Ok(SendResult::new(sent.id.to_string()))
    }

    async fn send_with_attachments(
        &self,
        message: OutboundMessage,
        attachments: Vec<Attachment>,
    ) -> Result<SendResult> {
        let chat_id = ChatId(
            message
                .target
                .chat_id
                .parse::<i64>()
                .map_err(|e| ChannelError::InvalidMessage(e.to_string()))?,
        );

        // Send attachments first
        let mut last_msg_id = None;

        for attachment in attachments {
            let input_file = match &attachment.source {
                crate::attachment::AttachmentSource::FileId(id) => InputFile::file_id(id.clone()),
                crate::attachment::AttachmentSource::Url(url) => InputFile::url(url.parse().unwrap()),
                crate::attachment::AttachmentSource::Bytes(bytes) => {
                    InputFile::memory(bytes.to_vec()).file_name(attachment.filename.clone())
                }
                crate::attachment::AttachmentSource::Path(path) => InputFile::file(path),
            };

            let result = match attachment.attachment_type {
                AttachmentType::Image => {
                    self.bot
                        .send_photo(chat_id, input_file)
                        .caption(attachment.caption.unwrap_or_default())
                        .await
                }
                AttachmentType::Audio => {
                    self.bot
                        .send_audio(chat_id, input_file)
                        .caption(attachment.caption.unwrap_or_default())
                        .await
                }
                AttachmentType::Video => {
                    self.bot
                        .send_video(chat_id, input_file)
                        .caption(attachment.caption.unwrap_or_default())
                        .await
                }
                AttachmentType::Voice => self.bot.send_voice(chat_id, input_file).await,
                _ => {
                    self.bot
                        .send_document(chat_id, input_file)
                        .caption(attachment.caption.unwrap_or_default())
                        .await
                }
            };

            match result {
                Ok(msg) => last_msg_id = Some(msg.id.to_string()),
                Err(e) => warn!("Failed to send attachment: {}", e),
            }
        }

        // Send text if present
        if !message.text.is_empty() {
            return self.send(message).await;
        }

        Ok(SendResult::new(last_msg_id.unwrap_or_default()))
    }

    async fn edit(&self, message_id: &str, new_content: &str) -> Result<()> {
        // Would need chat_id stored somewhere
        warn!("Edit not fully implemented - need chat_id context");
        Ok(())
    }

    async fn delete(&self, message_id: &str) -> Result<()> {
        warn!("Delete not fully implemented - need chat_id context");
        Ok(())
    }

    async fn react(&self, message_id: &str, emoji: &str) -> Result<()> {
        warn!("React not implemented for Telegram");
        Ok(())
    }

    async fn unreact(&self, message_id: &str, emoji: &str) -> Result<()> {
        warn!("Unreact not implemented for Telegram");
        Ok(())
    }

    async fn send_typing(&self, target: &MessageTarget) -> Result<()> {
        let chat_id = ChatId(
            target
                .chat_id
                .parse::<i64>()
                .map_err(|e| ChannelError::InvalidMessage(e.to_string()))?,
        );

        self.bot
            .send_chat_action(chat_id, teloxide::types::ChatAction::Typing)
            .await
            .map_err(|e| ChannelError::channel("telegram", e.to_string()))?;

        Ok(())
    }

    fn max_message_length(&self) -> usize {
        4096
    }
}

#[async_trait]
impl ChannelReceiver for TelegramChannel {
    async fn start_receiving(&self) -> Result<()> {
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();

        {
            let mut shutdown = self.shutdown.write().await;
            *shutdown = Some(shutdown_tx);
        }

        let bot = self.bot.clone();
        let tx = self.message_tx.clone();
        let channel = Arc::new(self.clone());

        tokio::spawn(async move {
            let handler = Update::filter_message().endpoint(
                move |bot: Bot, msg: teloxide::types::Message| {
                    let tx = tx.clone();
                    let channel = channel.clone();
                    async move {
                        if let Some(inbound) = channel.convert_message(&msg).await {
                            let _ = tx.send(inbound).await;
                        }
                        Ok(())
                    }
                },
            );

            Dispatcher::builder(bot, handler)
                .enable_ctrlc_handler()
                .build()
                .dispatch()
                .await;
        });

        info!("Started receiving messages for Telegram bot: {}", self.instance_id);
        Ok(())
    }

    async fn stop_receiving(&self) -> Result<()> {
        let mut shutdown = self.shutdown.write().await;
        if let Some(tx) = shutdown.take() {
            let _ = tx.send(());
        }
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
impl ChannelLifecycle for TelegramChannel {
    async fn connect(&self) -> Result<()> {
        // Verify bot token by getting bot info
        let me = self
            .bot
            .get_me()
            .await
            .map_err(|e| ChannelError::Auth(e.to_string()))?;

        info!(
            "Connected to Telegram as @{}",
            me.username.as_deref().unwrap_or("unknown")
        );

        let mut connected = self.connected.write().await;
        *connected = true;

        Ok(())
    }

    async fn disconnect(&self) -> Result<()> {
        self.stop_receiving().await?;

        let mut connected = self.connected.write().await;
        *connected = false;

        Ok(())
    }

    fn is_connected(&self) -> bool {
        // This is a blocking read, should be fine for status check
        self.connected.blocking_read().clone()
    }

    async fn health(&self) -> Result<ChannelHealth> {
        let start = std::time::Instant::now();

        match self.bot.get_me().await {
            Ok(_) => Ok(ChannelHealth {
                connected: true,
                latency_ms: Some(start.elapsed().as_millis() as u64),
                last_message: None,
                error: None,
            }),
            Err(e) => Ok(ChannelHealth {
                connected: false,
                latency_ms: None,
                last_message: None,
                error: Some(e.to_string()),
            }),
        }
    }
}

impl Clone for TelegramChannel {
    fn clone(&self) -> Self {
        let (tx, rx) = mpsc::channel(1000);
        Self {
            bot: self.bot.clone(),
            instance_id: self.instance_id.clone(),
            username: self.username.clone(),
            connected: self.connected.clone(),
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
    fn test_telegram_channel_creation() {
        let channel = TelegramChannel::new("test_token", "test_bot");
        assert_eq!(channel.channel_type(), "telegram");
        assert_eq!(channel.instance_id(), "test_bot");
    }

    #[test]
    fn test_capabilities() {
        let channel = TelegramChannel::new("test_token", "test_bot");
        let caps = channel.capabilities();
        assert!(caps.markdown);
        assert!(caps.images);
        assert!(caps.edits);
    }
}
