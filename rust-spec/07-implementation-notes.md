# OpenClaw Rust Implementation Notes

## 1. Async Runtime

### 1.1 Tokio Configuration

Use a multi-threaded runtime with tuned parameters:

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configure runtime
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(num_cpus::get())
        .enable_all()
        .thread_name("openclaw-worker")
        .build()?;

    runtime.block_on(async_main())
}
```

### 1.2 Task Spawning Patterns

```rust
// For CPU-bound work, use spawn_blocking
let result = tokio::task::spawn_blocking(move || {
    expensive_computation(&data)
}).await?;

// For I/O-bound work, use regular spawn
let handle = tokio::spawn(async move {
    fetch_data(&url).await
});

// For fire-and-forget with error logging
tokio::spawn(async move {
    if let Err(e) = background_task().await {
        tracing::error!("Background task failed: {}", e);
    }
});
```

### 1.3 Cancellation Handling

Always handle task cancellation gracefully:

```rust
use tokio_util::sync::CancellationToken;

pub struct Service {
    cancel_token: CancellationToken,
}

impl Service {
    pub async fn run(&self) -> Result<(), ServiceError> {
        loop {
            tokio::select! {
                _ = self.cancel_token.cancelled() => {
                    tracing::info!("Service cancelled, shutting down");
                    break;
                }
                result = self.process_next() => {
                    result?;
                }
            }
        }
        Ok(())
    }

    pub fn shutdown(&self) {
        self.cancel_token.cancel();
    }
}
```

## 2. Error Handling

### 2.1 Error Type Design

Use `thiserror` for library errors, `anyhow` for application errors:

```rust
// Library error (in openclaw-core)
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("File not found: {path}")]
    NotFound { path: PathBuf },

    #[error("Parse error at {location}: {message}")]
    Parse { location: String, message: String },

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

// Application error (in openclaw-cli)
use anyhow::{Context, Result};

fn load_config() -> Result<Config> {
    let path = config_path()?;
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read config from {}", path.display()))?;
    // ...
}
```

### 2.2 Result Extensions

Create helpful extension traits:

```rust
pub trait ResultExt<T, E> {
    /// Log error and convert to Option
    fn ok_or_log(self) -> Option<T>;

    /// Log error at warning level and convert to Option
    fn ok_or_warn(self) -> Option<T>;
}

impl<T, E: std::fmt::Display> ResultExt<T, E> for Result<T, E> {
    fn ok_or_log(self) -> Option<T> {
        match self {
            Ok(v) => Some(v),
            Err(e) => {
                tracing::error!("{}", e);
                None
            }
        }
    }

    fn ok_or_warn(self) -> Option<T> {
        match self {
            Ok(v) => Some(v),
            Err(e) => {
                tracing::warn!("{}", e);
                None
            }
        }
    }
}
```

### 2.3 Never Panic in Production

```rust
// BAD: Can panic
let value = some_option.unwrap();
let item = vec[index];

// GOOD: Handle errors explicitly
let value = some_option.ok_or(Error::MissingValue)?;
let item = vec.get(index).ok_or(Error::IndexOutOfBounds)?;

// For truly impossible cases, use expect with explanation
let value = some_option.expect("value always set during initialization");
```

## 3. Memory Safety Patterns

### 3.1 Interior Mutability

Use `Arc<RwLock<T>>` for shared state:

```rust
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct SharedState {
    config: Arc<RwLock<Config>>,
    sessions: Arc<RwLock<HashMap<SessionKey, Session>>>,
}

impl SharedState {
    pub async fn get_config(&self) -> Config {
        self.config.read().await.clone()
    }

    pub async fn update_config(&self, new_config: Config) {
        *self.config.write().await = new_config;
    }
}
```

### 3.2 Avoid Data Races

Use `DashMap` for concurrent hash maps:

```rust
use dashmap::DashMap;

pub struct ClientRegistry {
    clients: DashMap<String, ClientHandle>,
}

impl ClientRegistry {
    pub fn register(&self, id: String, handle: ClientHandle) {
        self.clients.insert(id, handle);
    }

    pub fn get(&self, id: &str) -> Option<ClientHandle> {
        self.clients.get(id).map(|r| r.clone())
    }

    pub fn remove(&self, id: &str) -> Option<ClientHandle> {
        self.clients.remove(id).map(|(_, v)| v)
    }
}
```

### 3.3 Zeroize Sensitive Data

```rust
use zeroize::{Zeroize, ZeroizeOnDrop};

#[derive(Zeroize, ZeroizeOnDrop)]
pub struct Credentials {
    api_key: String,
    #[zeroize(skip)]
    provider: String, // Not sensitive
}

// Manual zeroize for temporary buffers
fn decrypt_credential(encrypted: &[u8], key: &Key) -> Result<String, Error> {
    let mut buffer = vec![0u8; encrypted.len()];
    decrypt_into(&mut buffer, encrypted, key)?;
    let result = String::from_utf8(buffer.clone())?;
    buffer.zeroize(); // Clear buffer after use
    Ok(result)
}
```

## 4. Concurrency Patterns

### 4.1 Channel Patterns

```rust
use tokio::sync::{mpsc, broadcast, oneshot};

// mpsc: Multiple producers, single consumer (task coordination)
let (tx, mut rx) = mpsc::channel::<Message>(100);

// broadcast: Multiple producers, multiple consumers (events)
let (tx, _) = broadcast::channel::<Event>(100);

// oneshot: Single request-response (approvals)
let (tx, rx) = oneshot::channel::<ApprovalResponse>();
```

### 4.2 Semaphore for Rate Limiting

```rust
use tokio::sync::Semaphore;
use std::sync::Arc;

pub struct RateLimiter {
    semaphore: Arc<Semaphore>,
}

impl RateLimiter {
    pub fn new(max_concurrent: usize) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
        }
    }

    pub async fn acquire(&self) -> Result<SemaphorePermit<'_>, Error> {
        self.semaphore.acquire().await.map_err(|_| Error::Shutdown)
    }
}
```

### 4.3 Actor Pattern

```rust
pub struct Actor {
    receiver: mpsc::Receiver<ActorMessage>,
    state: ActorState,
}

enum ActorMessage {
    Process(ProcessRequest, oneshot::Sender<ProcessResult>),
    Shutdown,
}

impl Actor {
    pub fn spawn() -> ActorHandle {
        let (tx, rx) = mpsc::channel(100);
        let actor = Self {
            receiver: rx,
            state: ActorState::default(),
        };
        tokio::spawn(actor.run());
        ActorHandle { sender: tx }
    }

    async fn run(mut self) {
        while let Some(msg) = self.receiver.recv().await {
            match msg {
                ActorMessage::Process(req, reply) => {
                    let result = self.process(req).await;
                    let _ = reply.send(result);
                }
                ActorMessage::Shutdown => break,
            }
        }
    }
}

pub struct ActorHandle {
    sender: mpsc::Sender<ActorMessage>,
}

impl ActorHandle {
    pub async fn process(&self, req: ProcessRequest) -> Result<ProcessResult, Error> {
        let (tx, rx) = oneshot::channel();
        self.sender.send(ActorMessage::Process(req, tx)).await?;
        rx.await.map_err(|_| Error::ActorDied)
    }
}
```

## 5. Serialization

### 5.1 Serde Patterns

```rust
use serde::{Deserialize, Serialize};

// Use rename_all for consistent casing
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Config {
    pub agent_id: String,
    pub max_tokens: usize,
}

// Use tag for enum variants
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    MessageReceived { id: String, text: String },
    ToolCalled { name: String, input: Value },
}

// Use skip_serializing_if for optional fields
#[derive(Debug, Serialize, Deserialize)]
pub struct Message {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<String>,
    #[serde(default)]
    pub metadata: HashMap<String, Value>,
}
```

### 5.2 Custom Serialization

```rust
mod base64_serde {
    use base64::{engine::general_purpose::STANDARD, Engine};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&STANDARD.encode(bytes))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        STANDARD.decode(&s).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BinaryData {
    #[serde(with = "base64_serde")]
    pub data: Vec<u8>,
}
```

## 6. Logging with Tracing

### 6.1 Setup

```rust
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

fn setup_logging() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,openclaw=debug"));

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer()
            .with_target(true)
            .with_thread_ids(true)
            .with_file(true)
            .with_line_number(true))
        .init();
}
```

### 6.2 Structured Logging

```rust
use tracing::{info, warn, error, instrument, Span};

#[instrument(skip(self, message), fields(channel = %channel, session = %session_key))]
pub async fn process_message(
    &self,
    channel: &str,
    session_key: &SessionKey,
    message: &Message,
) -> Result<(), Error> {
    info!("Processing message");

    let result = self.agent.invoke(message).await;

    match &result {
        Ok(response) => {
            info!(response_len = response.text.len(), "Message processed");
        }
        Err(e) => {
            error!(error = %e, "Failed to process message");
        }
    }

    result
}
```

### 6.3 Span Context

```rust
use tracing::{span, Level, Instrument};

async fn handle_request(req: Request) -> Result<Response, Error> {
    let span = span!(Level::INFO, "request", id = %req.id);

    async move {
        // All logs within this block include the request span
        let response = process(req).await?;
        Ok(response)
    }
    .instrument(span)
    .await
}
```

## 7. Testing

### 7.1 Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_key_parsing() {
        let key = SessionKey::new("agent:test:subagent:abc123");
        assert!(key.is_subagent());
    }

    #[tokio::test]
    async fn test_async_function() {
        let result = async_function().await;
        assert_eq!(result, expected);
    }
}
```

### 7.2 Integration Tests

```rust
// tests/integration/gateway_test.rs
use openclaw_gateway::Gateway;
use tokio_tungstenite::connect_async;

#[tokio::test]
async fn test_gateway_connection() {
    let gateway = Gateway::start_test().await.unwrap();

    let (ws, _) = connect_async(gateway.ws_url()).await.unwrap();

    // Test WebSocket communication
}
```

### 7.3 Property-Based Testing

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_session_key_roundtrip(
        channel in "[a-z]+",
        account in "[a-z0-9]+",
        peer in "[a-z0-9]+",
    ) {
        let key = SessionKey::for_channel(&channel, &account, &peer, &AgentId::new("test"));
        let parsed = SessionKey::new(key.as_str());
        assert_eq!(key, parsed);
    }
}
```

### 7.4 Mocking

```rust
use mockall::automock;

#[automock]
#[async_trait]
pub trait ModelProvider {
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse, Error>;
}

#[tokio::test]
async fn test_with_mock_provider() {
    let mut mock = MockModelProvider::new();
    mock.expect_complete()
        .returning(|_| Ok(CompletionResponse::default()));

    let runtime = AgentRuntime::new(Arc::new(mock));
    // Test with mock
}
```

## 8. Performance Optimization

### 8.1 Avoid Allocations in Hot Paths

```rust
// BAD: Allocates on every call
fn format_key(channel: &str, peer: &str) -> String {
    format!("{}:{}", channel, peer)
}

// GOOD: Use a buffer
fn format_key_into(channel: &str, peer: &str, buf: &mut String) {
    buf.clear();
    buf.push_str(channel);
    buf.push(':');
    buf.push_str(peer);
}

// GOOD: Use Cow for sometimes-borrowed data
use std::borrow::Cow;

fn normalize_key(key: &str) -> Cow<'_, str> {
    if key.chars().all(|c| c.is_lowercase()) {
        Cow::Borrowed(key)
    } else {
        Cow::Owned(key.to_lowercase())
    }
}
```

### 8.2 Use Bytes for Binary Data

```rust
use bytes::{Bytes, BytesMut};

// Bytes is cheap to clone (reference counted)
pub struct Message {
    pub data: Bytes,
}

// BytesMut for building data
fn build_response(parts: &[&[u8]]) -> Bytes {
    let total_len: usize = parts.iter().map(|p| p.len()).sum();
    let mut buf = BytesMut::with_capacity(total_len);
    for part in parts {
        buf.extend_from_slice(part);
    }
    buf.freeze()
}
```

### 8.3 Connection Pooling

```rust
use reqwest::Client;

// Create client once and reuse
pub struct HttpClient {
    client: Client,
}

impl HttpClient {
    pub fn new() -> Self {
        let client = Client::builder()
            .pool_max_idle_per_host(10)
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self { client }
    }
}
```

## 9. Platform-Specific Code

### 9.1 Conditional Compilation

```rust
// OS-specific implementations
#[cfg(target_os = "linux")]
mod sandbox {
    pub fn create_sandbox() -> Sandbox {
        // Use seccomp + landlock
    }
}

#[cfg(target_os = "macos")]
mod sandbox {
    pub fn create_sandbox() -> Sandbox {
        // Use sandbox-exec
    }
}

#[cfg(target_os = "windows")]
mod sandbox {
    pub fn create_sandbox() -> Sandbox {
        // Use Windows Job Objects
    }
}

// Feature-gated code
#[cfg(feature = "telegram")]
pub mod telegram;

#[cfg(feature = "metrics")]
pub fn setup_metrics() {
    // Prometheus setup
}
```

### 9.2 Build Scripts

```rust
// build.rs
fn main() {
    // Set version from git
    let output = std::process::Command::new("git")
        .args(["describe", "--tags", "--always"])
        .output()
        .expect("Failed to get git version");

    let version = String::from_utf8_lossy(&output.stdout);
    println!("cargo:rustc-env=BUILD_VERSION={}", version.trim());

    // Platform-specific flags
    #[cfg(target_os = "linux")]
    println!("cargo:rustc-link-lib=seccomp");
}
```

## 10. Documentation

### 10.1 Module Documentation

```rust
//! # openclaw-agent
//!
//! Agent runtime and tool execution for OpenClaw.
//!
//! ## Overview
//!
//! This crate provides the core agent execution functionality:
//!
//! - **Runtime**: Agent conversation loop with tool calling
//! - **Sessions**: Conversation state management and persistence
//! - **Tools**: Tool definition, policy, and execution
//! - **Models**: Model provider integration and fallback
//!
//! ## Example
//!
//! ```rust,no_run
//! use openclaw_agent::{AgentRuntime, AgentConfig};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let runtime = AgentRuntime::new(config).await?;
//!     let stream = runtime.invoke(&session_key, message).await?;
//!     // Process events...
//!     Ok(())
//! }
//! ```
```

### 10.2 Function Documentation

```rust
/// Execute a command in a sandboxed environment.
///
/// # Arguments
///
/// * `command` - The shell command to execute
/// * `env` - Environment variables for the command
/// * `timeout` - Maximum execution time
///
/// # Returns
///
/// Returns the execution result including stdout, stderr, and exit code.
///
/// # Errors
///
/// Returns an error if:
/// - The sandbox fails to initialize
/// - The command times out
/// - The command is blocked by security policy
///
/// # Security
///
/// The command runs in an isolated sandbox with:
/// - Limited syscalls (seccomp)
/// - Restricted filesystem access (landlock)
/// - Resource limits (memory, CPU, processes)
///
/// # Example
///
/// ```rust
/// let result = sandbox.execute("ls -la", &env, Duration::from_secs(30)).await?;
/// println!("Exit code: {}", result.exit_code);
/// ```
pub async fn execute(
    &self,
    command: &str,
    env: &HashMap<String, String>,
    timeout: Duration,
) -> Result<ExecutionResult, SandboxError> {
    // Implementation
}
```

## 11. Recommended Crates Summary

### Core

| Crate | Purpose | Version |
|-------|---------|---------|
| `tokio` | Async runtime | 1.35+ |
| `axum` | HTTP/WebSocket server | 0.7+ |
| `serde` | Serialization | 1.0+ |
| `serde_json` | JSON | 1.0+ |
| `sqlx` | Database | 0.7+ |
| `tracing` | Logging | 0.1+ |
| `thiserror` | Error handling | 1.0+ |
| `anyhow` | Application errors | 1.0+ |

### Security

| Crate | Purpose | Version |
|-------|---------|---------|
| `ring` | Cryptography | 0.17+ |
| `rustls` | TLS | 0.22+ |
| `zeroize` | Memory wiping | 1.7+ |
| `seccompiler` | Syscall filtering | 0.4+ |
| `landlock` | Filesystem sandboxing | 0.3+ |

### Channels

| Crate | Purpose | Version |
|-------|---------|---------|
| `teloxide` | Telegram | 0.12+ |
| `serenity` | Discord | 0.12+ |
| `slack-morphism` | Slack | 1.0+ |

### Utilities

| Crate | Purpose | Version |
|-------|---------|---------|
| `uuid` | UUIDs | 1.6+ |
| `chrono` | Date/time | 0.4+ |
| `dashmap` | Concurrent map | 5.5+ |
| `bytes` | Binary data | 1.5+ |
| `clap` | CLI parsing | 4.4+ |

### Testing

| Crate | Purpose | Version |
|-------|---------|---------|
| `proptest` | Property testing | 1.4+ |
| `wiremock` | HTTP mocking | 0.5+ |
| `mockall` | Trait mocking | 0.12+ |
