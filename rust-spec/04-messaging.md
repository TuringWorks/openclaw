# OpenClaw Rust Messaging & Channels Specification

## 1. Channel Architecture Overview

The messaging system provides a unified interface for multiple messaging platforms while maintaining channel-specific capabilities.

```
┌─────────────────────────────────────────────────────────────────┐
│                      Channel Manager                             │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                    Channel Registry                       │   │
│  │  - Plugin loading                                        │   │
│  │  - Account management                                    │   │
│  │  - Health monitoring                                     │   │
│  └─────────────────────────────────────────────────────────┘   │
│                              │                                   │
│  ┌──────────┬──────────┬──────────┬──────────┬──────────┐     │
│  │ Telegram │ Discord  │  Slack   │  Signal  │ WhatsApp │     │
│  │ Adapter  │ Adapter  │ Adapter  │ Adapter  │ Adapter  │     │
│  └──────────┴──────────┴──────────┴──────────┴──────────┘     │
│                              │                                   │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                   Message Router                          │   │
│  │  - Agent binding resolution                              │   │
│  │  - Session key generation                                │   │
│  │  - DM policy enforcement                                 │   │
│  └─────────────────────────────────────────────────────────┘   │
│                              │                                   │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                   Delivery Pipeline                       │   │
│  │  - Message formatting                                    │   │
│  │  - Chunking                                              │   │
│  │  - Rate limiting                                         │   │
│  │  - Retry logic                                           │   │
│  └─────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

## 2. Channel Trait

### 2.1 Core Channel Interface

```rust
#[async_trait]
pub trait Channel: Send + Sync {
    /// Unique channel identifier (e.g., "telegram", "discord")
    fn id(&self) -> &str;

    /// Channel metadata
    fn meta(&self) -> &ChannelMeta;

    /// Channel capabilities
    fn capabilities(&self) -> &ChannelCapabilities;

    /// Connect to the channel with given configuration
    async fn connect(&mut self, config: &ChannelAccountConfig) -> Result<(), ChannelError>;

    /// Disconnect from the channel
    async fn disconnect(&mut self) -> Result<(), ChannelError>;

    /// Send a text message
    async fn send_text(
        &self,
        target: &MessageTarget,
        text: &str,
        options: &SendOptions,
    ) -> Result<DeliveryResult, ChannelError>;

    /// Send media (image, audio, video, file)
    async fn send_media(
        &self,
        target: &MessageTarget,
        media: &MediaPayload,
        options: &SendOptions,
    ) -> Result<DeliveryResult, ChannelError>;

    /// Health check
    async fn health_check(&self) -> ChannelHealth;

    /// Subscribe to inbound messages
    fn subscribe(&self) -> broadcast::Receiver<InboundMessage>;
}
```

### 2.2 Channel Metadata

```rust
#[derive(Debug, Clone)]
pub struct ChannelMeta {
    /// Display name
    pub label: String,

    /// Documentation URL
    pub docs_url: Option<String>,

    /// Aliases (e.g., "tg" for "telegram")
    pub aliases: Vec<String>,

    /// Setup complexity (1-5)
    pub setup_complexity: u8,

    /// Whether this is a core or extension channel
    pub is_extension: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ChannelCapabilities {
    /// Supported chat types
    pub chat_types: Vec<ChatType>,

    /// Media support
    pub media: MediaCapabilities,

    /// Feature support
    pub features: ChannelFeatures,

    /// Message limits
    pub limits: ChannelLimits,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatType {
    Direct,     // 1:1 DM
    Group,      // Group chat
    Channel,    // Broadcast channel
    Thread,     // Thread within group/channel
}

#[derive(Debug, Clone, Default)]
pub struct MediaCapabilities {
    pub images: bool,
    pub audio: bool,
    pub video: bool,
    pub files: bool,
    pub stickers: bool,
    pub voice_notes: bool,
    pub max_file_size_mb: u32,
}

#[derive(Debug, Clone, Default)]
pub struct ChannelFeatures {
    pub reactions: bool,
    pub threads: bool,
    pub edits: bool,
    pub deletes: bool,
    pub typing_indicators: bool,
    pub read_receipts: bool,
    pub mentions: bool,
    pub polls: bool,
    pub native_commands: bool,
}

#[derive(Debug, Clone)]
pub struct ChannelLimits {
    pub text_max_length: usize,
    pub caption_max_length: usize,
    pub messages_per_second: f32,
    pub messages_per_minute: u32,
}

impl Default for ChannelLimits {
    fn default() -> Self {
        Self {
            text_max_length: 4096,
            caption_max_length: 1024,
            messages_per_second: 1.0,
            messages_per_minute: 30,
        }
    }
}
```

### 2.3 Extended Channel Adapters

```rust
/// Channel with security features
#[async_trait]
pub trait SecureChannel: Channel {
    /// Check if a sender is allowed
    async fn is_sender_allowed(
        &self,
        sender_id: &str,
        account_id: &str,
    ) -> Result<bool, ChannelError>;

    /// Initiate pairing flow
    async fn initiate_pairing(
        &self,
        sender_id: &str,
    ) -> Result<PairingSession, ChannelError>;

    /// Complete pairing
    async fn complete_pairing(
        &self,
        session: &PairingSession,
        code: &str,
    ) -> Result<(), ChannelError>;
}

/// Channel with group management
#[async_trait]
pub trait GroupChannel: Channel {
    /// List groups the bot is in
    async fn list_groups(&self, account_id: &str) -> Result<Vec<GroupInfo>, ChannelError>;

    /// Get group members
    async fn get_members(&self, group_id: &str) -> Result<Vec<MemberInfo>, ChannelError>;

    /// Check mention requirements
    fn requires_mention(&self, group_id: &str) -> bool;

    /// Get group tool policy
    fn group_tool_policy(&self, group_id: &str) -> Option<ToolPolicy>;
}

/// Channel with threading support
#[async_trait]
pub trait ThreadingChannel: Channel {
    /// Reply to a specific message
    async fn reply_to(
        &self,
        target: &MessageTarget,
        reply_to_id: &str,
        text: &str,
        options: &SendOptions,
    ) -> Result<DeliveryResult, ChannelError>;

    /// Get reply-to mode for context
    fn reply_to_mode(&self, context: &MessageContext) -> ReplyToMode;
}

/// Channel with message streaming
#[async_trait]
pub trait StreamingChannel: Channel {
    /// Start streaming a response (typing indicator + progressive updates)
    async fn start_stream(&self, target: &MessageTarget) -> Result<StreamHandle, ChannelError>;

    /// Update stream with new content
    async fn update_stream(
        &self,
        handle: &StreamHandle,
        content: &str,
    ) -> Result<(), ChannelError>;

    /// Finalize stream
    async fn finish_stream(
        &self,
        handle: StreamHandle,
        final_content: &str,
    ) -> Result<DeliveryResult, ChannelError>;
}
```

## 3. Message Types

### 3.1 Inbound Messages

```rust
#[derive(Debug, Clone)]
pub struct InboundMessage {
    /// Unique message ID (channel-specific)
    pub id: String,

    /// Timestamp
    pub timestamp: DateTime<Utc>,

    /// Source channel
    pub channel: String,

    /// Account ID (bot account)
    pub account_id: String,

    /// Sender information
    pub sender: SenderInfo,

    /// Chat/conversation information
    pub chat: ChatInfo,

    /// Message text content
    pub text: String,

    /// Attached media
    pub media: Vec<MediaAttachment>,

    /// Quoted/replied message
    pub quote: Option<QuotedMessage>,

    /// Thread information
    pub thread: Option<ThreadInfo>,

    /// Channel-specific metadata
    pub metadata: Value,
}

#[derive(Debug, Clone)]
pub struct SenderInfo {
    pub id: String,
    pub username: Option<String>,
    pub display_name: Option<String>,
    pub phone_number: Option<String>,
    pub is_bot: bool,
}

#[derive(Debug, Clone)]
pub struct ChatInfo {
    pub id: String,
    pub chat_type: ChatType,
    pub title: Option<String>,

    /// Guild/team ID (Discord server, Slack workspace)
    pub guild_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct MediaAttachment {
    pub id: String,
    pub media_type: MediaType,
    pub url: Option<String>,
    pub data: Option<Vec<u8>>,
    pub filename: Option<String>,
    pub size_bytes: Option<u64>,
    pub mime_type: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub enum MediaType {
    Image,
    Audio,
    Video,
    Voice,
    Document,
    Sticker,
}

#[derive(Debug, Clone)]
pub struct QuotedMessage {
    pub id: String,
    pub text: Option<String>,
    pub sender_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ThreadInfo {
    pub id: String,
    pub parent_id: Option<String>,
}
```

### 3.2 Outbound Messages

```rust
#[derive(Debug, Clone)]
pub struct OutboundMessage {
    /// Message text
    pub text: String,

    /// Media attachments
    pub media: Vec<MediaPayload>,

    /// Mentions
    pub mentions: Vec<Mention>,

    /// Reply to message ID
    pub reply_to: Option<String>,

    /// Delivery options
    pub options: SendOptions,
}

#[derive(Debug, Clone)]
pub struct MediaPayload {
    pub media_type: MediaType,
    pub source: MediaSource,
    pub filename: Option<String>,
    pub caption: Option<String>,
}

#[derive(Debug, Clone)]
pub enum MediaSource {
    Url(String),
    Path(PathBuf),
    Bytes(Vec<u8>),
}

#[derive(Debug, Clone)]
pub struct Mention {
    pub user_id: String,
    pub username: Option<String>,
    pub offset: usize,
    pub length: usize,
}

#[derive(Debug, Clone, Default)]
pub struct SendOptions {
    /// Disable link previews
    pub disable_preview: bool,

    /// Silent/no notification
    pub silent: bool,

    /// Parse mode (markdown, html, none)
    pub parse_mode: Option<ParseMode>,

    /// Keyboard/buttons
    pub keyboard: Option<Keyboard>,
}

#[derive(Debug, Clone)]
pub enum ParseMode {
    Markdown,
    Html,
    Plain,
}
```

### 3.3 Delivery Results

```rust
#[derive(Debug, Clone)]
pub struct DeliveryResult {
    /// Channel
    pub channel: String,

    /// Message ID assigned by channel
    pub message_id: String,

    /// Chat ID
    pub chat_id: String,

    /// Timestamp
    pub timestamp: DateTime<Utc>,

    /// Channel-specific metadata
    pub metadata: Value,
}

#[derive(Debug, Clone)]
pub struct ChannelHealth {
    pub status: HealthStatus,
    pub latency_ms: Option<u64>,
    pub last_message_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}
```

## 4. Message Routing

### 4.1 Route Resolution

```rust
pub struct MessageRouter {
    /// Configuration
    config: Arc<RwLock<Config>>,

    /// Agent registry
    agents: Arc<AgentRegistry>,
}

impl MessageRouter {
    /// Resolve which agent should handle a message
    pub async fn resolve_route(
        &self,
        message: &InboundMessage,
    ) -> Result<ResolvedRoute, RoutingError> {
        let config = self.config.read().await;

        // Check bindings in order of specificity
        for binding in &config.routing.bindings {
            if self.binding_matches(binding, message) {
                return Ok(ResolvedRoute {
                    agent_id: binding.agent_id.clone(),
                    session_key: self.build_session_key(message, &binding.agent_id),
                    matched_by: binding.match_type(),
                });
            }
        }

        // Fall back to default agent
        let default_agent = config.agents.default.as_ref()
            .ok_or(RoutingError::NoDefaultAgent)?;

        Ok(ResolvedRoute {
            agent_id: default_agent.clone(),
            session_key: self.build_session_key(message, default_agent),
            matched_by: MatchType::Default,
        })
    }

    fn binding_matches(&self, binding: &RouteBinding, message: &InboundMessage) -> bool {
        // Channel match
        if let Some(channel) = &binding.match_channel {
            if channel != &message.channel {
                return false;
            }
        }

        // Account match
        if let Some(account) = &binding.match_account {
            if account != &message.account_id {
                return false;
            }
        }

        // Peer match (exact chat ID)
        if let Some(peer) = &binding.match_peer {
            if peer != &message.chat.id {
                return false;
            }
        }

        // Guild/team match
        if let Some(guild) = &binding.match_guild {
            if message.chat.guild_id.as_ref() != Some(guild) {
                return false;
            }
        }

        true
    }

    fn build_session_key(&self, message: &InboundMessage, agent_id: &str) -> SessionKey {
        let config = self.config.blocking_read();

        match config.session.dm_scope {
            DmScope::Main => {
                SessionKey::new(format!("{}:main", agent_id))
            }
            DmScope::PerPeer => {
                SessionKey::new(format!("{}:{}:{}", agent_id, message.channel, message.chat.id))
            }
            DmScope::PerChannelPeer => {
                SessionKey::new(format!(
                    "{}:{}:{}:{}",
                    agent_id, message.channel, message.account_id, message.chat.id
                ))
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedRoute {
    pub agent_id: String,
    pub session_key: SessionKey,
    pub matched_by: MatchType,
}

#[derive(Debug, Clone, Copy)]
pub enum MatchType {
    Peer,
    Guild,
    Account,
    Channel,
    Default,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RouteBinding {
    pub agent_id: String,
    pub match_channel: Option<String>,
    pub match_account: Option<String>,
    pub match_peer: Option<String>,
    pub match_guild: Option<String>,
}
```

### 4.2 DM Policy

```rust
#[derive(Debug, Clone, Copy, Deserialize, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DmPolicy {
    /// Allow all DMs
    Open,

    /// Require pairing for new senders
    #[default]
    Pairing,

    /// Only allow whitelisted senders
    Allowlist,

    /// Block all DMs
    Blocked,
}

impl MessageRouter {
    /// Check if a DM should be processed
    pub async fn check_dm_policy(
        &self,
        message: &InboundMessage,
    ) -> Result<DmPolicyResult, RoutingError> {
        if message.chat.chat_type != ChatType::Direct {
            return Ok(DmPolicyResult::Allowed);
        }

        let config = self.config.read().await;
        let policy = config.security.dm_policy;

        match policy {
            DmPolicy::Open => Ok(DmPolicyResult::Allowed),

            DmPolicy::Pairing => {
                let is_paired = self.check_pairing(&message.sender.id, &message.account_id).await?;
                if is_paired {
                    Ok(DmPolicyResult::Allowed)
                } else {
                    Ok(DmPolicyResult::RequiresPairing)
                }
            }

            DmPolicy::Allowlist => {
                let is_allowed = self.check_allowlist(&message.sender.id, &message.account_id).await?;
                if is_allowed {
                    Ok(DmPolicyResult::Allowed)
                } else {
                    Ok(DmPolicyResult::Blocked)
                }
            }

            DmPolicy::Blocked => Ok(DmPolicyResult::Blocked),
        }
    }
}

#[derive(Debug, Clone)]
pub enum DmPolicyResult {
    Allowed,
    RequiresPairing,
    Blocked,
}
```

## 5. Message Delivery

### 5.1 Delivery Pipeline

```rust
pub struct DeliveryPipeline {
    /// Channel manager
    channels: Arc<ChannelManager>,

    /// Rate limiter per channel
    rate_limiters: HashMap<String, RateLimiter>,

    /// Retry configuration
    retry_config: RetryConfig,
}

impl DeliveryPipeline {
    /// Deliver a message to a channel
    pub async fn deliver(
        &self,
        channel_id: &str,
        target: &MessageTarget,
        message: &OutboundMessage,
    ) -> Result<DeliveryResult, DeliveryError> {
        let channel = self.channels.get(channel_id)
            .ok_or(DeliveryError::ChannelNotFound(channel_id.to_string()))?;

        // Check rate limit
        self.rate_limiters
            .get(channel_id)
            .unwrap()
            .check(&target.chat_id)
            .await?;

        // Chunk message if needed
        let chunks = self.chunk_message(channel.capabilities(), message);

        let mut last_result = None;

        for chunk in chunks {
            let result = self.deliver_chunk(channel.as_ref(), target, &chunk).await?;
            last_result = Some(result);

            // Small delay between chunks
            if chunks.len() > 1 {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }

        last_result.ok_or(DeliveryError::NoContent)
    }

    async fn deliver_chunk(
        &self,
        channel: &dyn Channel,
        target: &MessageTarget,
        chunk: &MessageChunk,
    ) -> Result<DeliveryResult, DeliveryError> {
        let mut retries = 0;

        loop {
            let result = match chunk {
                MessageChunk::Text(text) => {
                    channel.send_text(target, text, &SendOptions::default()).await
                }
                MessageChunk::Media(media) => {
                    channel.send_media(target, media, &SendOptions::default()).await
                }
            };

            match result {
                Ok(delivery) => return Ok(delivery),
                Err(e) if self.should_retry(&e, retries) => {
                    retries += 1;
                    let delay = self.retry_config.delay_for_attempt(retries);
                    tokio::time::sleep(delay).await;
                }
                Err(e) => return Err(DeliveryError::Channel(e)),
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct MessageTarget {
    pub chat_id: String,
    pub thread_id: Option<String>,
}

enum MessageChunk {
    Text(String),
    Media(MediaPayload),
}
```

### 5.2 Message Chunking

```rust
impl DeliveryPipeline {
    fn chunk_message(
        &self,
        capabilities: &ChannelCapabilities,
        message: &OutboundMessage,
    ) -> Vec<MessageChunk> {
        let mut chunks = Vec::new();

        // Chunk text
        let max_len = capabilities.limits.text_max_length;
        let text_chunks = self.chunk_text(&message.text, max_len);

        for chunk in text_chunks {
            chunks.push(MessageChunk::Text(chunk));
        }

        // Add media
        for media in &message.media {
            chunks.push(MessageChunk::Media(media.clone()));
        }

        chunks
    }

    fn chunk_text(&self, text: &str, max_len: usize) -> Vec<String> {
        if text.len() <= max_len {
            return vec![text.to_string()];
        }

        let mut chunks = Vec::new();
        let mut remaining = text;

        while !remaining.is_empty() {
            if remaining.len() <= max_len {
                chunks.push(remaining.to_string());
                break;
            }

            // Find break point (prefer line breaks, then spaces)
            let break_at = self.find_break_point(remaining, max_len);
            let (chunk, rest) = remaining.split_at(break_at);
            chunks.push(chunk.to_string());
            remaining = rest.trim_start();
        }

        chunks
    }

    fn find_break_point(&self, text: &str, max_len: usize) -> usize {
        // Look for line break
        if let Some(pos) = text[..max_len].rfind('\n') {
            return pos + 1;
        }

        // Look for space
        if let Some(pos) = text[..max_len].rfind(' ') {
            return pos + 1;
        }

        // Hard break
        max_len
    }
}
```

### 5.3 Retry Logic

```rust
#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_retries: u32,
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub exponential_base: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(10),
            exponential_base: 2.0,
        }
    }
}

impl RetryConfig {
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        let delay_ms = self.initial_delay.as_millis() as f64
            * self.exponential_base.powi(attempt as i32 - 1);

        Duration::from_millis(delay_ms.min(self.max_delay.as_millis() as f64) as u64)
    }
}

impl DeliveryPipeline {
    fn should_retry(&self, error: &ChannelError, retries: u32) -> bool {
        if retries >= self.retry_config.max_retries {
            return false;
        }

        match error {
            ChannelError::RateLimited { .. } => true,
            ChannelError::NetworkError(_) => true,
            ChannelError::ServiceUnavailable => true,
            _ => false,
        }
    }
}
```

## 6. Channel Implementations

### 6.1 Telegram

```rust
pub struct TelegramChannel {
    /// Bot API client
    bot: Bot,

    /// Account configuration
    config: TelegramAccountConfig,

    /// Inbound message broadcaster
    inbound_tx: broadcast::Sender<InboundMessage>,

    /// Connection status
    connected: AtomicBool,
}

impl TelegramChannel {
    pub async fn new(config: TelegramAccountConfig) -> Result<Self, ChannelError> {
        let bot = Bot::new(&config.bot_token);

        let (inbound_tx, _) = broadcast::channel(256);

        Ok(Self {
            bot,
            config,
            inbound_tx,
            connected: AtomicBool::new(false),
        })
    }
}

#[async_trait]
impl Channel for TelegramChannel {
    fn id(&self) -> &str {
        "telegram"
    }

    fn meta(&self) -> &ChannelMeta {
        &ChannelMeta {
            label: "Telegram".to_string(),
            docs_url: Some("https://docs.openclaw.ai/channels/telegram".to_string()),
            aliases: vec!["tg".to_string()],
            setup_complexity: 2,
            is_extension: false,
        }
    }

    fn capabilities(&self) -> &ChannelCapabilities {
        &ChannelCapabilities {
            chat_types: vec![ChatType::Direct, ChatType::Group, ChatType::Channel],
            media: MediaCapabilities {
                images: true,
                audio: true,
                video: true,
                files: true,
                stickers: true,
                voice_notes: true,
                max_file_size_mb: 50,
            },
            features: ChannelFeatures {
                reactions: true,
                threads: true, // Forum topics
                edits: true,
                deletes: true,
                typing_indicators: true,
                read_receipts: false,
                mentions: true,
                polls: true,
                native_commands: true,
            },
            limits: ChannelLimits {
                text_max_length: 4096,
                caption_max_length: 1024,
                messages_per_second: 1.0,
                messages_per_minute: 30,
            },
        }
    }

    async fn connect(&mut self, _config: &ChannelAccountConfig) -> Result<(), ChannelError> {
        // Start long polling
        let bot = self.bot.clone();
        let tx = self.inbound_tx.clone();

        tokio::spawn(async move {
            let mut offset = 0;

            loop {
                let updates = bot
                    .get_updates()
                    .offset(offset)
                    .timeout(30)
                    .await;

                match updates {
                    Ok(updates) => {
                        for update in updates {
                            offset = update.id + 1;
                            if let Some(message) = Self::convert_update(&update) {
                                let _ = tx.send(message);
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("Telegram poll error: {}", e);
                        tokio::time::sleep(Duration::from_secs(5)).await;
                    }
                }
            }
        });

        self.connected.store(true, Ordering::SeqCst);
        Ok(())
    }

    async fn send_text(
        &self,
        target: &MessageTarget,
        text: &str,
        options: &SendOptions,
    ) -> Result<DeliveryResult, ChannelError> {
        let chat_id = ChatId(target.chat_id.parse()?);

        let mut request = self.bot.send_message(chat_id, text);

        if options.disable_preview {
            request = request.disable_web_page_preview(true);
        }

        if options.silent {
            request = request.disable_notification(true);
        }

        if let Some(thread_id) = &target.thread_id {
            request = request.message_thread_id(thread_id.parse()?);
        }

        let result = request.await?;

        Ok(DeliveryResult {
            channel: "telegram".to_string(),
            message_id: result.id.to_string(),
            chat_id: target.chat_id.clone(),
            timestamp: Utc::now(),
            metadata: json!({}),
        })
    }

    async fn send_media(
        &self,
        target: &MessageTarget,
        media: &MediaPayload,
        _options: &SendOptions,
    ) -> Result<DeliveryResult, ChannelError> {
        let chat_id = ChatId(target.chat_id.parse()?);

        let input_file = match &media.source {
            MediaSource::Url(url) => InputFile::url(url.parse()?),
            MediaSource::Path(path) => InputFile::file(path),
            MediaSource::Bytes(data) => InputFile::memory(data.clone()),
        };

        let result = match media.media_type {
            MediaType::Image => self.bot.send_photo(chat_id, input_file).await?,
            MediaType::Video => self.bot.send_video(chat_id, input_file).await?,
            MediaType::Audio => self.bot.send_audio(chat_id, input_file).await?,
            MediaType::Document => self.bot.send_document(chat_id, input_file).await?,
            MediaType::Voice => self.bot.send_voice(chat_id, input_file).await?,
            _ => return Err(ChannelError::UnsupportedMediaType),
        };

        Ok(DeliveryResult {
            channel: "telegram".to_string(),
            message_id: result.id.to_string(),
            chat_id: target.chat_id.clone(),
            timestamp: Utc::now(),
            metadata: json!({}),
        })
    }

    fn subscribe(&self) -> broadcast::Receiver<InboundMessage> {
        self.inbound_tx.subscribe()
    }

    async fn health_check(&self) -> ChannelHealth {
        match self.bot.get_me().await {
            Ok(_) => ChannelHealth {
                status: HealthStatus::Healthy,
                latency_ms: None,
                last_message_at: None,
                error: None,
            },
            Err(e) => ChannelHealth {
                status: HealthStatus::Unhealthy,
                latency_ms: None,
                last_message_at: None,
                error: Some(e.to_string()),
            },
        }
    }

    async fn disconnect(&mut self) -> Result<(), ChannelError> {
        self.connected.store(false, Ordering::SeqCst);
        Ok(())
    }
}
```

### 6.2 Discord

```rust
pub struct DiscordChannel {
    client: Client,
    config: DiscordAccountConfig,
    inbound_tx: broadcast::Sender<InboundMessage>,
}

#[async_trait]
impl Channel for DiscordChannel {
    fn id(&self) -> &str {
        "discord"
    }

    fn capabilities(&self) -> &ChannelCapabilities {
        &ChannelCapabilities {
            chat_types: vec![ChatType::Direct, ChatType::Channel, ChatType::Thread],
            media: MediaCapabilities {
                images: true,
                audio: true,
                video: true,
                files: true,
                stickers: true,
                voice_notes: false,
                max_file_size_mb: 8, // or 100 for Nitro
            },
            features: ChannelFeatures {
                reactions: true,
                threads: true,
                edits: true,
                deletes: true,
                typing_indicators: true,
                read_receipts: false,
                mentions: true,
                polls: true,
                native_commands: true,
            },
            limits: ChannelLimits {
                text_max_length: 2000,
                caption_max_length: 2000,
                messages_per_second: 5.0,
                messages_per_minute: 50,
            },
        }
    }

    // ... implementation similar to Telegram
}
```

## 7. Channel Manager

### 7.1 Manager Implementation

```rust
pub struct ChannelManager {
    /// Registered channels
    channels: RwLock<HashMap<String, Arc<dyn Channel>>>,

    /// Channel configurations
    configs: RwLock<HashMap<String, HashMap<String, ChannelAccountConfig>>>,

    /// Plugin loader for extension channels
    plugin_loader: Arc<PluginLoader>,
}

impl ChannelManager {
    pub async fn new(plugin_loader: Arc<PluginLoader>) -> Self {
        Self {
            channels: RwLock::new(HashMap::new()),
            configs: RwLock::new(HashMap::new()),
            plugin_loader,
        }
    }

    /// Initialize channels from configuration
    pub async fn initialize(&self, config: &ChannelsConfig) -> Result<(), ChannelError> {
        // Initialize built-in channels
        if let Some(telegram) = &config.telegram {
            self.init_telegram(telegram).await?;
        }

        if let Some(discord) = &config.discord {
            self.init_discord(discord).await?;
        }

        // ... other channels

        // Load extension channels
        for plugin_id in &config.extensions {
            self.load_extension_channel(plugin_id).await?;
        }

        Ok(())
    }

    /// Get a channel by ID
    pub async fn get(&self, channel_id: &str) -> Option<Arc<dyn Channel>> {
        self.channels.read().await.get(channel_id).cloned()
    }

    /// List all channels with status
    pub async fn list_status(&self) -> Vec<ChannelStatus> {
        let channels = self.channels.read().await;
        let mut statuses = Vec::new();

        for (id, channel) in channels.iter() {
            let health = channel.health_check().await;
            statuses.push(ChannelStatus {
                id: id.clone(),
                label: channel.meta().label.clone(),
                health,
                accounts: self.list_accounts(id).await,
            });
        }

        statuses
    }

    /// Subscribe to all channel messages
    pub async fn subscribe_all(&self) -> mpsc::Receiver<InboundMessage> {
        let (tx, rx) = mpsc::channel(256);
        let channels = self.channels.read().await;

        for channel in channels.values() {
            let mut receiver = channel.subscribe();
            let tx = tx.clone();

            tokio::spawn(async move {
                while let Ok(msg) = receiver.recv().await {
                    if tx.send(msg).await.is_err() {
                        break;
                    }
                }
            });
        }

        rx
    }
}

#[derive(Debug, Clone)]
pub struct ChannelStatus {
    pub id: String,
    pub label: String,
    pub health: ChannelHealth,
    pub accounts: Vec<AccountStatus>,
}

#[derive(Debug, Clone)]
pub struct AccountStatus {
    pub id: String,
    pub enabled: bool,
    pub username: Option<String>,
}
```

## 8. Message Queue

### 8.1 Outbound Queue

```rust
pub struct OutboundQueue {
    /// Pending messages
    queue: Arc<Mutex<VecDeque<QueuedMessage>>>,

    /// Queue configuration
    config: QueueConfig,

    /// Processing task handle
    processor: Option<JoinHandle<()>>,
}

#[derive(Debug, Clone)]
pub struct QueuedMessage {
    pub id: String,
    pub channel: String,
    pub target: MessageTarget,
    pub message: OutboundMessage,
    pub priority: MessagePriority,
    pub created_at: Instant,
    pub retry_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MessagePriority {
    Low = 0,
    Normal = 1,
    High = 2,
}

#[derive(Debug, Clone)]
pub struct QueueConfig {
    pub mode: QueueMode,
    pub max_size: usize,
    pub batch_delay: Duration,
    pub dedup_mode: DedupMode,
    pub drop_policy: DropPolicy,
}

#[derive(Debug, Clone, Copy)]
pub enum QueueMode {
    Off,       // Deliver immediately
    Batch,     // Batch messages
    Immediate, // Queue but process immediately
}

#[derive(Debug, Clone, Copy)]
pub enum DedupMode {
    Off,
    Session,
    Peer,
}

#[derive(Debug, Clone, Copy)]
pub enum DropPolicy {
    Off,
    Newest,
    Oldest,
}

impl OutboundQueue {
    pub async fn enqueue(&self, message: QueuedMessage) -> Result<(), QueueError> {
        let mut queue = self.queue.lock().await;

        // Check size limit
        if queue.len() >= self.config.max_size {
            match self.config.drop_policy {
                DropPolicy::Off => return Err(QueueError::QueueFull),
                DropPolicy::Newest => return Ok(()), // Drop this message
                DropPolicy::Oldest => { queue.pop_front(); }
            }
        }

        // Deduplication
        if self.config.dedup_mode != DedupMode::Off {
            let should_dedup = match self.config.dedup_mode {
                DedupMode::Session => queue.iter().any(|m| m.target.chat_id == message.target.chat_id),
                DedupMode::Peer => queue.iter().any(|m| m.target == message.target),
                DedupMode::Off => false,
            };

            if should_dedup {
                return Ok(());
            }
        }

        queue.push_back(message);
        Ok(())
    }

    pub async fn drain(&self, delivery: &DeliveryPipeline) -> Result<usize, QueueError> {
        let messages: Vec<_> = {
            let mut queue = self.queue.lock().await;
            queue.drain(..).collect()
        };

        let count = messages.len();

        for message in messages {
            if let Err(e) = delivery.deliver(
                &message.channel,
                &message.target,
                &message.message,
            ).await {
                tracing::error!("Queue delivery failed: {}", e);
                // Re-queue failed messages if retryable
            }
        }

        Ok(count)
    }
}
```

## 9. Error Types

```rust
#[derive(Debug, thiserror::Error)]
pub enum ChannelError {
    #[error("Channel not found: {0}")]
    NotFound(String),

    #[error("Channel not connected")]
    NotConnected,

    #[error("Authentication failed: {0}")]
    AuthFailed(String),

    #[error("Rate limited: retry after {retry_after:?}")]
    RateLimited { retry_after: Duration },

    #[error("Network error: {0}")]
    NetworkError(#[from] reqwest::Error),

    #[error("Service unavailable")]
    ServiceUnavailable,

    #[error("Invalid target: {0}")]
    InvalidTarget(String),

    #[error("Unsupported media type")]
    UnsupportedMediaType,

    #[error("Message too long: {len} > {max}")]
    MessageTooLong { len: usize, max: usize },

    #[error("API error: {0}")]
    ApiError(String),
}

#[derive(Debug, thiserror::Error)]
pub enum RoutingError {
    #[error("No default agent configured")]
    NoDefaultAgent,

    #[error("Agent not found: {0}")]
    AgentNotFound(String),

    #[error("DM blocked by policy")]
    DmBlocked,
}

#[derive(Debug, thiserror::Error)]
pub enum DeliveryError {
    #[error("Channel not found: {0}")]
    ChannelNotFound(String),

    #[error("Channel error: {0}")]
    Channel(#[from] ChannelError),

    #[error("Rate limited")]
    RateLimited,

    #[error("No content to deliver")]
    NoContent,
}

#[derive(Debug, thiserror::Error)]
pub enum QueueError {
    #[error("Queue full")]
    QueueFull,

    #[error("Queue closed")]
    Closed,
}
```
