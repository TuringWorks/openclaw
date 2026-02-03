# OpenClaw Rust Data Models Specification

## 1. Core Types

### 1.1 Identifiers

```rust
use uuid::Uuid;
use std::fmt;

/// Strongly-typed agent identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AgentId(String);

impl AgentId {
    pub fn new(id: impl Into<String>) -> Self {
        let normalized = id.into().to_lowercase().replace([' ', '-'], "_");
        Self(normalized)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Session key for conversation isolation
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SessionKey(String);

impl SessionKey {
    pub fn new(key: impl Into<String>) -> Self {
        Self(key.into())
    }

    /// Create for channel message
    pub fn for_channel(channel: &str, account: &str, peer: &str, agent: &AgentId) -> Self {
        Self(format!("{}:{}:{}:{}", agent, channel, account, peer))
    }

    /// Create for subagent
    pub fn for_subagent(parent: &AgentId) -> Self {
        let uuid = Uuid::new_v4();
        Self(format!("{}:subagent:{}", parent, uuid))
    }

    pub fn is_subagent(&self) -> bool {
        self.0.contains(":subagent:")
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Approval request identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ApprovalId(String);

impl ApprovalId {
    pub fn new() -> Self {
        let bytes: [u8; 4] = rand::random();
        Self(base64_url::encode(&bytes))
    }
}

/// Message identifier (channel-specific)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MessageId(String);
```

### 1.2 Timestamps

```rust
use chrono::{DateTime, Utc};

/// Wrapper for DateTime with convenient serialization
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Timestamp(DateTime<Utc>);

impl Timestamp {
    pub fn now() -> Self {
        Self(Utc::now())
    }

    pub fn from_millis(millis: i64) -> Self {
        Self(DateTime::from_timestamp_millis(millis).unwrap_or_default())
    }

    pub fn as_millis(&self) -> i64 {
        self.0.timestamp_millis()
    }
}

impl Default for Timestamp {
    fn default() -> Self {
        Self::now()
    }
}
```

## 2. Configuration Types

### 2.1 Main Config

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    /// Agent configurations
    #[serde(default)]
    pub agents: AgentsConfig,

    /// Channel configurations
    #[serde(default)]
    pub channels: ChannelsConfig,

    /// Gateway settings
    #[serde(default)]
    pub gateway: GatewayConfig,

    /// Session management
    #[serde(default)]
    pub session: SessionConfig,

    /// Security settings
    #[serde(default)]
    pub security: SecurityConfig,

    /// Memory/search settings
    #[serde(default)]
    pub memory: MemoryConfig,

    /// Logging settings
    #[serde(default)]
    pub logging: LoggingConfig,

    /// Routing bindings
    #[serde(default)]
    pub routing: RoutingConfig,
}

impl Config {
    /// Load from file path
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path)?;
        let config: Self = json5::from_str(&content)?;
        config.validate()?;
        Ok(config)
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Validate agent IDs are unique
        // Validate model references
        // Validate tool policies
        // etc.
        Ok(())
    }
}
```

### 2.2 Agents Config

```rust
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct AgentsConfig {
    /// Default agent ID
    pub default: Option<String>,

    /// Per-agent configurations
    #[serde(default)]
    pub agents: HashMap<String, AgentConfig>,

    /// Default settings for all agents
    #[serde(default)]
    pub defaults: AgentDefaults,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AgentConfig {
    /// Agent ID (normalized)
    pub id: AgentId,

    /// Display name
    pub name: Option<String>,

    /// Workspace directory
    pub workspace_dir: Option<PathBuf>,

    /// Primary model
    pub model: Option<ModelRef>,

    /// Fallback models
    #[serde(default)]
    pub fallback_models: Vec<ModelRef>,

    /// System prompt override
    pub system_prompt: Option<String>,

    /// Thinking level
    #[serde(default)]
    pub thinking_level: ThinkingLevel,

    /// Tool policy
    #[serde(default)]
    pub tools: ToolPolicyConfig,

    /// Sandbox configuration
    pub sandbox: Option<SandboxConfig>,

    /// Subagent settings
    #[serde(default)]
    pub subagents: SubagentConfig,

    /// Identity (name, emoji, avatar)
    pub identity: Option<AgentIdentity>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct AgentDefaults {
    /// Default model
    pub model: Option<ModelRef>,

    /// Model aliases
    #[serde(default)]
    pub models: HashMap<String, ModelRef>,

    /// Default thinking level
    #[serde(default)]
    pub thinking_level: ThinkingLevel,

    /// CLI backend credentials
    #[serde(default)]
    pub cli_backends: HashMap<String, String>,

    /// Default tool policy
    #[serde(default)]
    pub tools: ToolPolicyConfig,

    /// Cache settings
    pub cache: Option<CacheConfig>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ThinkingLevel {
    Off,
    Minimal,
    #[default]
    Low,
    Medium,
    High,
    XHigh,
}

impl ThinkingLevel {
    pub fn budget_tokens(&self) -> Option<usize> {
        match self {
            Self::Off => None,
            Self::Minimal => Some(1024),
            Self::Low => Some(4096),
            Self::Medium => Some(8192),
            Self::High => Some(16384),
            Self::XHigh => Some(32768),
        }
    }
}
```

### 2.3 Channels Config

```rust
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ChannelsConfig {
    /// Telegram configuration
    pub telegram: Option<TelegramConfig>,

    /// Discord configuration
    pub discord: Option<DiscordConfig>,

    /// Slack configuration
    pub slack: Option<SlackConfig>,

    /// Signal configuration
    pub signal: Option<SignalConfig>,

    /// WhatsApp configuration
    pub whatsapp: Option<WhatsAppConfig>,

    /// Extension channels (dynamic)
    #[serde(flatten)]
    pub extensions: HashMap<String, Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TelegramConfig {
    /// Enable/disable
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Bot accounts
    #[serde(default)]
    pub accounts: HashMap<String, TelegramAccountConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TelegramAccountConfig {
    /// Bot token
    pub bot_token: SecretString,

    /// Bot username
    pub username: Option<String>,

    /// Webhook URL (if using webhooks)
    pub webhook_url: Option<String>,

    /// Enable/disable this account
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}
```

### 2.4 Security Config

```rust
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SecurityConfig {
    /// Audit logging
    #[serde(default)]
    pub audit: AuditConfig,

    /// Execution security
    #[serde(default)]
    pub exec: ExecSecurityConfig,

    /// DM policy
    #[serde(default)]
    pub dm_policy: DmPolicy,

    /// External content handling
    #[serde(default)]
    pub external_content: ExternalContentConfig,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ExecSecurityConfig {
    /// Execution mode
    #[serde(default)]
    pub mode: ExecMode,

    /// Ask mode
    #[serde(default)]
    pub ask: AskMode,

    /// Command allowlist patterns
    #[serde(default)]
    pub allowlist: Vec<String>,

    /// Safe binaries (always allowed)
    #[serde(default)]
    pub safe_bins: Vec<String>,

    /// Approval timeout in seconds
    #[serde(default = "default_approval_timeout")]
    pub approval_timeout_secs: u64,

    /// Fallback behavior when approval fails
    #[serde(default)]
    pub ask_fallback: AskFallback,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ExecMode {
    #[default]
    Deny,
    Allowlist,
    Full,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AskMode {
    Off,
    #[default]
    OnMiss,
    Always,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AskFallback {
    #[default]
    Deny,
    Allow,
}

fn default_approval_timeout() -> u64 {
    120
}
```

### 2.5 Tool Policy Config

```rust
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ToolPolicyConfig {
    /// Tool profile
    #[serde(default)]
    pub profile: ToolProfile,

    /// Explicit allow patterns
    #[serde(default)]
    pub allow: Vec<String>,

    /// Explicit deny patterns
    #[serde(default)]
    pub deny: Vec<String>,

    /// Additional allow (union with profile)
    #[serde(default)]
    pub also_allow: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolProfile {
    Minimal,
    Coding,
    Messaging,
    #[default]
    Full,
}

impl ToolProfile {
    pub fn included_tools(&self) -> &'static [&'static str] {
        match self {
            Self::Minimal => &["session_status"],
            Self::Coding => &[
                "read", "write", "edit", "glob", "grep", "apply_patch",
                "exec", "process",
                "sessions_list", "sessions_history", "sessions_send", "sessions_spawn",
                "memory_search", "memory_get",
                "image",
            ],
            Self::Messaging => &[
                "message",
                "sessions_list", "sessions_send",
            ],
            Self::Full => &[], // All tools allowed
        }
    }
}
```

## 3. Message Types

### 3.1 Inbound Message

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InboundMessage {
    /// Message ID (channel-specific)
    pub id: MessageId,

    /// Timestamp
    pub timestamp: Timestamp,

    /// Source channel
    pub channel: String,

    /// Account ID (bot account)
    pub account_id: String,

    /// Sender information
    pub sender: SenderInfo,

    /// Chat information
    pub chat: ChatInfo,

    /// Text content
    pub text: String,

    /// Media attachments
    #[serde(default)]
    pub media: Vec<MediaAttachment>,

    /// Quoted message
    pub quote: Option<QuotedMessage>,

    /// Thread information
    pub thread: Option<ThreadInfo>,

    /// Channel-specific metadata
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SenderInfo {
    pub id: String,
    pub username: Option<String>,
    pub display_name: Option<String>,
    pub phone_number: Option<String>,
    #[serde(default)]
    pub is_bot: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChatInfo {
    pub id: String,
    pub chat_type: ChatType,
    pub title: Option<String>,
    pub guild_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatType {
    Direct,
    Group,
    Channel,
    Thread,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MediaAttachment {
    pub id: String,
    pub media_type: MediaType,
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<u8>>,
    pub filename: Option<String>,
    pub size_bytes: Option<u64>,
    pub mime_type: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MediaType {
    Image,
    Audio,
    Video,
    Voice,
    Document,
    Sticker,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct QuotedMessage {
    pub id: String,
    pub text: Option<String>,
    pub sender_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ThreadInfo {
    pub id: String,
    pub parent_id: Option<String>,
}
```

### 3.2 Outbound Message

```rust
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct OutboundMessage {
    /// Text content
    pub text: String,

    /// Media attachments
    #[serde(default)]
    pub media: Vec<MediaPayload>,

    /// Mentions
    #[serde(default)]
    pub mentions: Vec<Mention>,

    /// Reply to message ID
    pub reply_to: Option<String>,

    /// Send options
    #[serde(default)]
    pub options: SendOptions,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MediaPayload {
    pub media_type: MediaType,
    pub source: MediaSource,
    pub filename: Option<String>,
    pub caption: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", content = "value")]
pub enum MediaSource {
    Url(String),
    Path(PathBuf),
    #[serde(with = "base64_serde")]
    Bytes(Vec<u8>),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Mention {
    pub user_id: String,
    pub username: Option<String>,
    pub offset: usize,
    pub length: usize,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SendOptions {
    #[serde(default)]
    pub disable_preview: bool,
    #[serde(default)]
    pub silent: bool,
    pub parse_mode: Option<ParseMode>,
    pub keyboard: Option<Keyboard>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ParseMode {
    Markdown,
    Html,
    Plain,
}
```

## 4. Session Types

### 4.1 Session

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Session {
    /// Session key
    pub key: SessionKey,

    /// Agent ID
    pub agent_id: AgentId,

    /// Creation time
    pub created_at: Timestamp,

    /// Last activity
    pub last_message_at: Timestamp,

    /// Conversation messages
    #[serde(default)]
    pub messages: Vec<Message>,

    /// Token usage
    #[serde(default)]
    pub tokens: TokenUsage,

    /// Cost tracking
    pub cost: Option<CostUsage>,

    /// Model override
    pub model: Option<ModelRef>,

    /// Thinking level override
    pub thinking_level: Option<ThinkingLevel>,

    /// Type mode
    #[serde(default)]
    pub type_mode: TypeMode,

    /// Session metadata
    #[serde(default)]
    pub metadata: SessionMetadata,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct TokenUsage {
    pub input: u64,
    pub output: u64,
    pub cache_creation: u64,
    pub cache_read: u64,
}

impl TokenUsage {
    pub fn total(&self) -> u64 {
        self.input + self.output + self.cache_creation + self.cache_read
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct CostUsage {
    pub input_usd: f64,
    pub output_usd: f64,
    pub total_usd: f64,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TypeMode {
    #[default]
    Typing,
    Never,
    Thinking,
    Message,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SessionMetadata {
    /// Channel context
    pub channel: Option<String>,
    pub account_id: Option<String>,
    pub peer_id: Option<String>,

    /// Labels
    #[serde(default)]
    pub labels: HashMap<String, String>,
}
```

### 4.2 Message

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Message {
    pub role: Role,
    pub content: MessageContent,
    pub name: Option<String>,
    pub tool_use_id: Option<String>,
    pub timestamp: Timestamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
    System,
    Tool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

impl MessageContent {
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(s) => Some(s),
            Self::Blocks(blocks) => {
                if blocks.len() == 1 {
                    if let ContentBlock::Text { text } = &blocks[0] {
                        return Some(text);
                    }
                }
                None
            }
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text {
        text: String,
    },
    Image {
        source: ImageSource,
    },
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(default)]
        is_error: bool,
    },
    Thinking {
        thinking: String,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ImageSource {
    #[serde(rename = "type")]
    pub source_type: String,
    pub media_type: String,
    pub data: String, // base64
}
```

## 5. Tool Types

### 5.1 Tool Definition

```rust
#[derive(Debug, Clone)]
pub struct ToolDefinition {
    /// Tool name
    pub name: String,

    /// Description
    pub description: String,

    /// Input schema (JSON Schema)
    pub input_schema: Value,

    /// Handler function
    pub handler: ToolHandler,

    /// Execution settings
    pub execution: ToolExecution,
}

pub type ToolHandler = Arc<
    dyn Fn(Value, ToolContext) -> BoxFuture<'static, Result<Value, ToolError>>
        + Send
        + Sync
>;

#[derive(Debug, Clone)]
pub struct ToolExecution {
    pub host: ExecutionHost,
    pub requires_approval: bool,
    pub sandbox_profile: SandboxProfile,
    pub resource_limits: Option<ResourceLimits>,
}

#[derive(Debug, Clone)]
pub enum ExecutionHost {
    Sandbox,
    Gateway,
    Node(String),
    Docker(String),
}

#[derive(Debug, Clone, Copy, Default)]
pub enum SandboxProfile {
    Strict,
    #[default]
    Standard,
    Trusted,
    None,
}
```

### 5.2 Tool Context

```rust
#[derive(Clone)]
pub struct ToolContext {
    /// Session key
    pub session_key: SessionKey,

    /// Agent ID
    pub agent_id: AgentId,

    /// Working directory
    pub cwd: PathBuf,

    /// Environment variables
    pub env: HashMap<String, String>,

    /// Sandbox (if applicable)
    pub sandbox: Option<Arc<Sandbox>>,

    /// Approval manager
    pub approval_manager: Arc<ApprovalManager>,

    /// Audit log
    pub audit_log: Arc<AuditLog>,

    /// Request ID for tracing
    pub request_id: String,
}
```

### 5.3 Tool Result

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ToolResult {
    pub tool_use_id: String,
    pub output: Value,
    #[serde(default)]
    pub is_error: bool,
    pub duration_ms: Option<u64>,
}
```

## 6. Model Types

### 6.1 Model Reference

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct ModelRef {
    pub provider: String,
    pub model_id: String,
}

impl ModelRef {
    pub fn new(provider: impl Into<String>, model_id: impl Into<String>) -> Self {
        Self {
            provider: provider.into(),
            model_id: model_id.into(),
        }
    }

    pub fn parse(s: &str) -> Result<Self, ModelError> {
        let parts: Vec<&str> = s.splitn(2, '/').collect();
        if parts.len() != 2 {
            return Err(ModelError::InvalidFormat(s.to_string()));
        }
        Ok(Self::new(parts[0], parts[1]))
    }
}

impl fmt::Display for ModelRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.provider, self.model_id)
    }
}
```

### 6.2 Model Info

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ModelInfo {
    pub id: String,
    pub provider: String,
    pub display_name: String,
    pub capabilities: ModelCapabilities,
    pub context_window: usize,
    pub max_output_tokens: usize,
    pub pricing: Option<ModelPricing>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ModelCapabilities {
    pub vision: bool,
    pub tool_use: bool,
    pub extended_thinking: bool,
    pub streaming: bool,
    pub json_mode: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ModelPricing {
    /// Price per 1M input tokens
    pub input_per_1m: f64,
    /// Price per 1M output tokens
    pub output_per_1m: f64,
    /// Price per 1M cache creation tokens
    pub cache_creation_per_1m: Option<f64>,
    /// Price per 1M cache read tokens
    pub cache_read_per_1m: Option<f64>,
}
```

## 7. Security Types

### 7.1 Credentials

```rust
use zeroize::{Zeroize, ZeroizeOnDrop};

/// Secret string that zeros memory on drop
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct SecretString {
    inner: String,
}

impl SecretString {
    pub fn new(value: impl Into<String>) -> Self {
        Self { inner: value.into() }
    }

    pub fn expose_secret(&self) -> &str {
        &self.inner
    }
}

// Never print secrets
impl fmt::Debug for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}

impl<'de> Deserialize<'de> for SecretString {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(Self::new(s))
    }
}

impl Serialize for SecretString {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        // Serialize as the actual value (for config files)
        self.inner.serialize(serializer)
    }
}
```

### 7.2 Auth Context

```rust
#[derive(Debug, Clone)]
pub struct AuthContext {
    pub client_id: String,
    pub scopes: HashSet<Scope>,
    pub identity: Option<Identity>,
    pub authenticated_at: Timestamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Scope {
    Admin,
    Read,
    Write,
    Approvals,
    Pairing,
}

#[derive(Debug, Clone)]
pub struct Identity {
    pub user_id: String,
    pub username: Option<String>,
    pub email: Option<String>,
    pub provider: String,
}
```

### 7.3 Audit Events

```rust
#[derive(Debug, Clone, Serialize)]
pub struct AuditEntry {
    pub timestamp: Timestamp,
    pub event: AuditEvent,
    pub hostname: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AuditEvent {
    pub event_type: AuditEventType,
    pub actor: String,
    pub session_id: Option<String>,
    pub request_id: Option<String>,
    pub outcome: AuditOutcome,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AuditEventType {
    ExecCommandRequested { command: String, sandbox: bool },
    ExecCommandApproved { approval_id: String },
    ExecCommandDenied { approval_id: String, reason: String },
    ExecCommandCompleted { exit_code: i32, duration_ms: u64 },
    AuthSuccess { method: String, identity: Option<String> },
    AuthFailure { method: String, reason: String },
    ChannelLogin { channel: String, account: String },
    MessageSent { channel: String, target: String },
    SandboxViolation { violation_type: String, details: String },
    ConfigChanged { key: String },
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AuditOutcome {
    Success,
    Failure,
    Denied,
    Timeout,
}
```

## 8. Gateway Types

### 8.1 WebSocket Messages

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl JsonRpcError {
    pub fn parse_error() -> Self {
        Self { code: -32700, message: "Parse error".to_string(), data: None }
    }

    pub fn invalid_request() -> Self {
        Self { code: -32600, message: "Invalid Request".to_string(), data: None }
    }

    pub fn method_not_found(method: &str) -> Self {
        Self { code: -32601, message: format!("Method not found: {}", method), data: None }
    }

    pub fn invalid_params(msg: &str) -> Self {
        Self { code: -32602, message: msg.to_string(), data: None }
    }

    pub fn internal_error(msg: &str) -> Self {
        Self { code: -32603, message: msg.to_string(), data: None }
    }
}
```

### 8.2 Gateway Events

```rust
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GatewayEvent {
    Connected {
        client_id: String,
    },
    Disconnected {
        client_id: String,
        reason: Option<String>,
    },
    AgentEvent {
        session_key: String,
        event: AgentEvent,
    },
    ChannelStatus {
        channel: String,
        status: ChannelHealth,
    },
    ApprovalRequest {
        approval_id: String,
        command: String,
        context: Value,
    },
    ApprovalResponse {
        approval_id: String,
        response: String,
    },
}
```

## 9. Error Types

```rust
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    #[error("Gateway error: {0}")]
    Gateway(#[from] GatewayError),

    #[error("Agent error: {0}")]
    Agent(#[from] AgentError),

    #[error("Channel error: {0}")]
    Channel(#[from] ChannelError),

    #[error("Security error: {0}")]
    Security(#[from] SecurityError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("File not found: {0}")]
    NotFound(PathBuf),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum SecurityError {
    #[error("Blocked environment variable: {0}")]
    BlockedEnvVar(String),

    #[error("Path traversal attempt: {attempted} escapes {workspace}")]
    PathTraversal { attempted: PathBuf, workspace: PathBuf },

    #[error("Absolute path not allowed")]
    AbsolutePathNotAllowed,

    #[error("Insecure file permissions: {0:o}")]
    InsecurePermissions(u32),

    #[error("Authentication failed: {0}")]
    AuthFailed(String),

    #[error("Insufficient scope: required {required:?}, have {available:?}")]
    InsufficientScope { required: Scope, available: HashSet<Scope> },
}
```

## 10. Utility Traits

```rust
/// Conversion to/from JSON for storage
pub trait JsonStorage: Sized + Serialize + for<'de> Deserialize<'de> {
    fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    fn from_json(s: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(s)
    }
}

// Implement for all types that are serializable
impl<T> JsonStorage for T where T: Serialize + for<'de> Deserialize<'de> {}

/// Normalize identifiers
pub trait Normalize {
    fn normalize(&self) -> String;
}

impl Normalize for str {
    fn normalize(&self) -> String {
        self.to_lowercase()
            .replace([' ', '-'], "_")
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '_')
            .collect()
    }
}
```
