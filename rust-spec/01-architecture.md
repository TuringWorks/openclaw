# OpenClaw Rust Architecture Specification

## 1. System Overview

OpenClaw is a multi-channel AI agent gateway that:

- Receives messages from messaging platforms (Telegram, Discord, Slack, Signal, WhatsApp, etc.)
- Routes messages to configured AI agents
- Executes tools on behalf of agents (bash, file operations, browser, etc.)
- Sends responses back to messaging platforms
- Provides a WebSocket control plane for CLI and UI clients

### High-Level Architecture

```text
┌─────────────────────────────────────────────────────────────────────────┐
│                              GATEWAY                                    │
│  ┌──────────────────────────────────────────────────────────────────┐   │
│  │                    WebSocket Server (Control Plane)              │   │
│  │  - JSON-RPC 2.0 protocol                                         │   │
│  │  - Client authentication (token, password, identity)             │   │
│  │  - Method routing (chat, channels, config, nodes, etc.)          │   │
│  └──────────────────────────────────────────────────────────────────┘   │
│                                    │                                    │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐                   │
│  │   Channel    │  │    Agent     │  │    Tool      │                   │
│  │   Manager    │  │   Runtime    │  │   Executor   │                   │
│  │              │  │              │  │              │                   │
│  │ - Telegram   │  │ - Session    │  │ - Sandbox    │                   │
│  │ - Discord    │  │ - Model      │  │ - Approval   │                   │
│  │ - Slack      │  │ - Streaming  │  │ - Audit      │                   │
│  │ - Signal     │  │ - Memory     │  │ - PTY        │                   │
│  │ - WhatsApp   │  │              │  │              │                   │
│  └──────────────┘  └──────────────┘  └──────────────┘                   │
│                                                                         │
│  ┌───────────────────────────────────────────────────────────────-──┐   │
│  │                    Configuration & State                         │   │
│  │  - Config files (JSON5)                                          │   │
│  │  - Auth profiles (encrypted)                                     │   │
│  │  - Session storage (SQLite)                                      │   │
│  │  - Audit logs                                                    │   │
│  └──────────────────────────────────────────────────────────────-───┘   │
└─────────────────────────────────────────────────────────────────────────┘
         │                    │                    │
         ▼                    ▼                    ▼
    ┌─────────┐          ┌─────────┐          ┌─────────┐
    │   CLI   │          │  Web UI │          │  Nodes  │
    │ Client  │          │ Client  │          │ Devices │
    └─────────┘          └─────────┘          └─────────┘
```

## 2. Component Topology

### 2.1 Gateway Server

The gateway is the central coordinator. It:

- Listens on a configurable port (default: 18789)
- Accepts WebSocket connections from CLI, UI, and node clients
- Manages channel connections (bot tokens, webhooks)
- Routes inbound messages to appropriate agents
- Executes tools and returns results
- Broadcasts events to subscribed clients

**Rust Implementation:**

```rust
pub struct Gateway {
    config: Arc<RwLock<Config>>,
    channel_manager: Arc<ChannelManager>,
    agent_runtime: Arc<AgentRuntime>,
    tool_executor: Arc<ToolExecutor>,
    session_store: Arc<SessionStore>,
    client_registry: Arc<ClientRegistry>,
}

impl Gateway {
    pub async fn start(bind: SocketAddr) -> Result<Self, GatewayError>;
    pub async fn handle_connection(&self, ws: WebSocket, auth: AuthContext);
    pub async fn shutdown(&self) -> Result<(), GatewayError>;
}
```

### 2.2 Channel Manager

Manages connections to messaging platforms.

**Responsibilities:**

- Load channel configurations
- Initialize channel adapters (Telegram bot, Discord client, etc.)
- Route inbound messages to agent runtime
- Deliver outbound messages to channels
- Track channel status and health

**Rust Interface:**

```rust
#[async_trait]
pub trait Channel: Send + Sync {
    fn id(&self) -> &str;
    fn capabilities(&self) -> ChannelCapabilities;

    async fn connect(&mut self, config: &ChannelConfig) -> Result<(), ChannelError>;
    async fn disconnect(&mut self) -> Result<(), ChannelError>;
    async fn send_message(&self, target: &Target, message: &OutboundMessage) -> Result<DeliveryResult, ChannelError>;
    async fn health_check(&self) -> ChannelHealth;
}

pub struct ChannelManager {
    channels: HashMap<String, Box<dyn Channel>>,
    router: MessageRouter,
}
```

### 2.3 Agent Runtime

Executes AI agent conversations with tool calling support.

**Responsibilities:**

- Manage agent sessions (create, load, persist)
- Call AI model APIs (Claude, OpenAI, etc.)
- Handle streaming responses
- Execute tool calls via tool executor
- Manage thinking levels and model selection

**Rust Interface:**

```rust
pub struct AgentRuntime {
    model_registry: Arc<ModelRegistry>,
    tool_registry: Arc<ToolRegistry>,
    session_store: Arc<SessionStore>,
    sandbox_manager: Arc<SandboxManager>,
}

impl AgentRuntime {
    pub async fn invoke(
        &self,
        session_key: &SessionKey,
        message: InboundMessage,
        config: AgentConfig,
    ) -> Result<AgentStream, AgentError>;
}

pub enum AgentEvent {
    TextBlock { text: String },
    ToolUse { id: String, name: String, input: Value },
    ToolResult { id: String, output: Value },
    Thinking { text: String },
    Error { message: String },
    Done { usage: TokenUsage },
}
```

### 2.4 Tool Executor

Executes tools in isolated environments.

**Responsibilities:**

- Validate tool inputs against schemas
- Apply tool policies (allow/deny lists)
- Execute tools in sandboxed processes
- Handle approval workflows for sensitive operations
- Audit all tool executions

**Rust Interface:**

```rust
pub struct ToolExecutor {
    sandbox_manager: Arc<SandboxManager>,
    approval_manager: Arc<ApprovalManager>,
    audit_log: Arc<AuditLog>,
}

impl ToolExecutor {
    pub async fn execute(
        &self,
        tool: &ToolDefinition,
        input: Value,
        context: &ExecutionContext,
    ) -> Result<ToolOutput, ToolError>;
}

pub enum ExecutionHost {
    Sandbox(SandboxConfig),
    Gateway,
    Node(NodeId),
}
```

### 2.5 Session Store

Persists agent conversation sessions.

**Storage Model:**

- SQLite database for metadata and indexes
- JSON Lines files for message history
- Vector embeddings for semantic search

**Rust Interface:**

```rust
pub struct SessionStore {
    db: SqlitePool,
    sessions_dir: PathBuf,
}

impl SessionStore {
    pub async fn get_session(&self, key: &SessionKey) -> Result<Option<Session>, StoreError>;
    pub async fn save_session(&self, session: &Session) -> Result<(), StoreError>;
    pub async fn append_message(&self, key: &SessionKey, msg: &Message) -> Result<(), StoreError>;
    pub async fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>, StoreError>;
}
```

## 3. Data Flow

### 3.1 Inbound Message Flow

```text
Channel (Telegram/Discord/etc.)
    │
    ▼
┌─────────────────────────┐
│ Channel Adapter         │
│ - Normalize message     │
│ - Extract metadata      │
└───────────┬─────────────┘
            │
            ▼
┌─────────────────────────┐
│ Message Router          │
│ - Resolve agent binding │
│ - Check allowlists      │
│ - Apply DM policies     │
└───────────┬─────────────┘
            │
            ▼
┌─────────────────────────┐
│ Agent Runtime           │
│ - Load/create session   │
│ - Build context         │
│ - Call AI model         │
└───────────┬─────────────┘
            │
            ▼ (streaming)
┌─────────────────────────┐
│ Tool Executor           │
│ - Validate tool call    │
│ - Request approval      │
│ - Execute in sandbox    │
│ - Return result         │
└───────────┬─────────────┘
            │
            ▼
┌─────────────────────────┐
│ Channel Manager         │
│ - Format response       │
│ - Chunk if needed       │
│ - Deliver to channel    │
└─────────────────────────┘
```

### 3.2 WebSocket Control Flow

```text
CLI/UI Client
    │
    ▼
┌─────────────────────────┐
│ WebSocket Server        │
│ - Accept connection     │
│ - Authenticate client   │
│ - Assign scopes         │
└───────────┬─────────────┘
            │
            ▼
┌─────────────────────────┐
│ JSON-RPC Router         │
│ - Parse request         │
│ - Validate method       │
│ - Check authorization   │
└───────────┬─────────────┘
            │
            ▼
┌─────────────────────────┐
│ Method Handler          │
│ - Execute operation     │
│ - Stream results        │
│ - Return response       │
└─────────────────────────┘
```

## 4. Configuration Model

### 4.1 Config File Structure

```text
~/.openclaw/
├── openclaw.json5         # Main configuration
├── auth-profiles.json     # Encrypted credentials
├── models.json            # Model catalog cache
├── sessions/              # Session transcripts
│   └── {session-id}.jsonl
├── agents/                # Per-agent state
│   └── {agent-id}/
│       ├── workspace/     # Agent workspace files
│       └── sessions/      # Agent-specific sessions
└── audit/                 # Audit logs
    └── {date}.log
```

### 4.2 Config Schema (Rust Types)

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub agents: AgentsConfig,
    pub channels: ChannelsConfig,
    pub gateway: GatewayConfig,
    pub session: SessionConfig,
    pub security: SecurityConfig,
    pub memory: MemoryConfig,
    pub logging: LoggingConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AgentsConfig {
    pub default: Option<String>,
    pub agents: HashMap<String, AgentConfig>,
    pub defaults: AgentDefaults,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SecurityConfig {
    pub audit: AuditConfig,
    pub exec: ExecSecurityConfig,
    pub sandbox: SandboxConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExecSecurityConfig {
    pub mode: ExecMode,           // deny, allowlist, full
    pub ask: AskMode,             // off, on_miss, always
    pub allowlist: Vec<String>,   // Allowed command patterns
    pub safe_bins: Vec<String>,   // Always-safe binaries
    pub approval_timeout_secs: u64,
}
```

## 5. Authentication & Authorization

### 5.1 Client Authentication

Clients authenticate to the gateway via:

1. **Loopback auto-auth**: Automatic for localhost connections
2. **Token auth**: Bearer token in WebSocket handshake
3. **Password auth**: Timing-safe password comparison
4. **Tailscale identity**: Validated via Tailscale headers + IP

### 5.2 Authorization Scopes

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    Admin,      // Full access
    Read,       // Status, logs, history
    Write,      // Send, agent, models
    Approvals,  // Exec approval gates
    Pairing,    // Device/node pairing
}

pub struct AuthContext {
    pub client_id: String,
    pub scopes: HashSet<Scope>,
    pub identity: Option<Identity>,
}
```

### 5.3 Method Authorization

Each gateway method declares required scopes:

```rust
pub struct MethodSpec {
    pub name: &'static str,
    pub required_scopes: &'static [Scope],
    pub handler: MethodHandler,
}

// Example
const CHAT_SEND: MethodSpec = MethodSpec {
    name: "chat.send",
    required_scopes: &[Scope::Write],
    handler: handle_chat_send,
};
```

## 6. Extension Points

### 6.1 Plugin System

Plugins extend OpenClaw with:

- Additional channels
- Custom tools
- HTTP routes
- Gateway method handlers

**Plugin Interface:**

```rust
#[async_trait]
pub trait Plugin: Send + Sync {
    fn id(&self) -> &str;
    fn version(&self) -> &str;

    async fn activate(&mut self, ctx: PluginContext) -> Result<(), PluginError>;
    async fn deactivate(&mut self) -> Result<(), PluginError>;

    fn channels(&self) -> Vec<Box<dyn Channel>>;
    fn tools(&self) -> Vec<ToolDefinition>;
    fn routes(&self) -> Vec<HttpRoute>;
}
```

### 6.2 Tool Registration

Tools are defined with JSON Schema inputs and async handlers:

```rust
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: Value,  // JSON Schema
    pub handler: ToolHandler,
    pub execution_host: ExecutionHost,
    pub requires_approval: bool,
}

pub type ToolHandler = Arc<dyn Fn(Value, ExecutionContext) -> BoxFuture<'static, Result<Value, ToolError>> + Send + Sync>;
```

## 7. Observability

### 7.1 Structured Logging

Use `tracing` for structured, leveled logging:

```rust
use tracing::{info, warn, error, span, Level};

#[instrument(skip(self), fields(session_key = %key))]
async fn handle_message(&self, key: &SessionKey, msg: Message) -> Result<()> {
    info!(channel = %msg.channel, "Processing inbound message");
    // ...
}
```

### 7.2 Metrics

Expose Prometheus-compatible metrics:

- `openclaw_messages_total{channel, direction}`
- `openclaw_tool_executions_total{tool, status}`
- `openclaw_agent_invocations_total{model, status}`
- `openclaw_gateway_connections{client_type}`

### 7.3 Audit Log

Security-sensitive operations logged to append-only audit trail:

```rust
pub struct AuditEvent {
    pub timestamp: DateTime<Utc>,
    pub event_type: AuditEventType,
    pub actor: String,
    pub target: String,
    pub details: Value,
    pub outcome: AuditOutcome,
}

pub enum AuditEventType {
    ExecCommand,
    SendMessage,
    ChannelLogin,
    ConfigChange,
    ApprovalGrant,
    ApprovalDeny,
    SandboxViolation,
}
```

## 8. Deployment Modes

### 8.1 Standalone

Single binary running gateway, channels, and agent runtime:

```bash
openclaw gateway run --bind loopback --port 18789
```

### 8.2 Distributed

Separate processes for scalability:

- Gateway server (handles WebSocket, routing)
- Channel workers (per-channel processes)
- Agent workers (sandboxed execution pools)
- Node devices (remote tool execution)

### 8.3 Container

Docker/OCI container with minimal attack surface:

```dockerfile
FROM rust:alpine AS builder
# Build with musl for static linking

FROM scratch
COPY --from=builder /app/openclaw /openclaw
USER 65534
ENTRYPOINT ["/openclaw"]
```

## 9. Error Handling Strategy

### 9.1 Error Types

Domain-specific error types with context:

```rust
#[derive(Debug, thiserror::Error)]
pub enum GatewayError {
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    #[error("Channel error: {channel}: {source}")]
    Channel { channel: String, #[source] source: ChannelError },

    #[error("Agent error: {0}")]
    Agent(#[from] AgentError),

    #[error("Authentication failed: {0}")]
    Auth(String),
}
```

### 9.2 Recovery Strategy

- **Transient errors**: Retry with exponential backoff
- **Config errors**: Log and exit with guidance
- **Auth errors**: Prompt for re-authentication
- **Channel errors**: Disable channel, notify operator
- **Sandbox violations**: Log, deny, alert

## 10. Testing Strategy

### 10.1 Unit Tests

Test individual components in isolation:

```rust
#[tokio::test]
async fn test_tool_policy_evaluation() {
    let policy = ToolPolicy::new()
        .allow("group:fs")
        .deny("exec");

    assert!(policy.is_allowed("read"));
    assert!(!policy.is_allowed("exec"));
}
```

### 10.2 Integration Tests

Test component interactions:

```rust
#[tokio::test]
async fn test_agent_tool_execution() {
    let gateway = TestGateway::new().await;
    let result = gateway.invoke_agent("test", "list files").await;
    assert!(result.contains("tool_use"));
}
```

### 10.3 E2E Tests

Full system tests with real (or mocked) channels:

```rust
#[tokio::test]
#[ignore] // Requires LIVE=1
async fn test_telegram_roundtrip() {
    let gateway = Gateway::start_test().await;
    // Send message via Telegram API
    // Verify response received
}
```
