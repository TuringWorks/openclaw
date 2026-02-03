# OpenClaw Rust Module Organization Specification

## 1. Workspace Structure

The Rust implementation uses a Cargo workspace with multiple crates for modularity and compile-time optimization.

```
openclaw-rs/
├── Cargo.toml                      # Workspace manifest
├── Cargo.lock
├── rust-toolchain.toml
├── .cargo/
│   └── config.toml                 # Cargo configuration
│
├── crates/
│   ├── openclaw-core/              # Shared types, config, utilities
│   ├── openclaw-gateway/           # WebSocket server, HTTP endpoints
│   ├── openclaw-agent/             # Agent runtime, tool execution
│   ├── openclaw-sandbox/           # Process isolation (seccomp, landlock)
│   ├── openclaw-channels/          # Channel abstraction layer
│   ├── openclaw-memory/            # Vector search, embeddings
│   ├── openclaw-cli/               # Command-line interface
│   └── openclaw-plugin-sdk/        # Plugin development kit
│
├── channels/                       # Channel implementations
│   ├── openclaw-telegram/
│   ├── openclaw-discord/
│   ├── openclaw-slack/
│   ├── openclaw-signal/
│   └── openclaw-whatsapp/
│
├── plugins/                        # Extension plugins
│   └── example-plugin/
│
├── tests/
│   ├── integration/
│   └── e2e/
│
├── benches/                        # Benchmarks
│   └── gateway_bench.rs
│
└── tools/                          # Development tools
    ├── xtask/                      # Build automation
    └── fuzz/                       # Fuzz testing
```

## 2. Workspace Manifest

```toml
# Cargo.toml (workspace root)
[workspace]
resolver = "2"
members = [
    "crates/*",
    "channels/*",
    "plugins/*",
    "tools/*",
]

[workspace.package]
version = "0.1.0"
edition = "2021"
rust-version = "1.75"
license = "MIT OR Apache-2.0"
repository = "https://github.com/openclaw/openclaw-rs"

[workspace.dependencies]
# Async runtime
tokio = { version = "1.35", features = ["full"] }
tokio-util = "0.7"
futures = "0.3"

# Web framework
axum = { version = "0.7", features = ["ws", "macros"] }
tower = "0.4"
tower-http = { version = "0.5", features = ["cors", "trace"] }

# WebSocket
tokio-tungstenite = "0.21"

# HTTP client
reqwest = { version = "0.11", features = ["json", "stream", "rustls-tls"] }

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
json5 = "0.4"

# Database
sqlx = { version = "0.7", features = ["runtime-tokio", "sqlite", "json"] }

# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }

# Error handling
thiserror = "1.0"
anyhow = "1.0"

# Security
ring = "0.17"
rustls = "0.22"
zeroize = { version = "1.7", features = ["derive"] }

# CLI
clap = { version = "4.4", features = ["derive", "env"] }

# Utilities
uuid = { version = "1.6", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
url = "2.5"
bytes = "1.5"
dashmap = "5.5"

# Testing
proptest = "1.4"
wiremock = "0.5"
```

## 3. Core Crate (`openclaw-core`)

Shared types, configuration, and utilities used across all crates.

```
crates/openclaw-core/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── config/
    │   ├── mod.rs
    │   ├── schema.rs           # Config type definitions
    │   ├── loader.rs           # Config file loading
    │   ├── validation.rs       # Config validation
    │   └── migration.rs        # Config version migration
    ├── types/
    │   ├── mod.rs
    │   ├── message.rs          # Message types
    │   ├── session.rs          # Session types
    │   ├── agent.rs            # Agent types
    │   ├── model.rs            # Model reference types
    │   └── channel.rs          # Channel types
    ├── error.rs                # Common error types
    ├── paths.rs                # Path resolution utilities
    ├── env.rs                  # Environment variable handling
    └── id.rs                   # ID generation utilities
```

### Cargo.toml

```toml
[package]
name = "openclaw-core"
version.workspace = true
edition.workspace = true

[dependencies]
serde.workspace = true
serde_json.workspace = true
json5.workspace = true
thiserror.workspace = true
chrono.workspace = true
uuid.workspace = true
url.workspace = true
zeroize.workspace = true
tracing.workspace = true

[dev-dependencies]
proptest.workspace = true
```

### Module Exports

```rust
// src/lib.rs
pub mod config;
pub mod types;
pub mod error;
pub mod paths;
pub mod env;
pub mod id;

// Re-exports for convenience
pub use config::Config;
pub use error::{Error, Result};
pub use types::*;
```

## 4. Gateway Crate (`openclaw-gateway`)

WebSocket server and HTTP endpoints.

```
crates/openclaw-gateway/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── server/
    │   ├── mod.rs
    │   ├── builder.rs          # Server builder pattern
    │   ├── http.rs             # HTTP routes
    │   ├── websocket.rs        # WebSocket handling
    │   └── state.rs            # Shared server state
    ├── auth/
    │   ├── mod.rs
    │   ├── authenticator.rs    # Auth logic
    │   ├── middleware.rs       # Auth middleware
    │   └── token.rs            # Token management
    ├── methods/
    │   ├── mod.rs
    │   ├── router.rs           # Method routing
    │   ├── chat.rs             # Chat methods
    │   ├── channels.rs         # Channel methods
    │   ├── config.rs           # Config methods
    │   ├── sessions.rs         # Session methods
    │   ├── models.rs           # Model methods
    │   ├── nodes.rs            # Node methods
    │   └── health.rs           # Health check
    ├── broadcast.rs            # Event broadcasting
    ├── client.rs               # Client connection handling
    └── shutdown.rs             # Graceful shutdown
```

### Dependencies

```toml
[package]
name = "openclaw-gateway"

[dependencies]
openclaw-core = { path = "../openclaw-core" }
openclaw-agent = { path = "../openclaw-agent" }
openclaw-channels = { path = "../openclaw-channels" }

tokio.workspace = true
axum.workspace = true
tower.workspace = true
tower-http.workspace = true
tokio-tungstenite.workspace = true
serde.workspace = true
serde_json.workspace = true
tracing.workspace = true
thiserror.workspace = true
dashmap.workspace = true
```

## 5. Agent Crate (`openclaw-agent`)

Agent runtime and tool execution.

```
crates/openclaw-agent/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── runtime/
    │   ├── mod.rs
    │   ├── context.rs          # Execution context
    │   ├── executor.rs         # Agent execution loop
    │   └── stream.rs           # Event streaming
    ├── session/
    │   ├── mod.rs
    │   ├── store.rs            # Session storage
    │   ├── key.rs              # Session key handling
    │   └── transcript.rs       # Transcript persistence
    ├── tools/
    │   ├── mod.rs
    │   ├── registry.rs         # Tool registry
    │   ├── policy.rs           # Tool policy evaluation
    │   ├── schema.rs           # Schema normalization
    │   ├── builtin/
    │   │   ├── mod.rs
    │   │   ├── exec.rs         # Exec tool
    │   │   ├── fs.rs           # File system tools
    │   │   ├── browser.rs      # Browser tool
    │   │   ├── sessions.rs     # Session tools
    │   │   ├── memory.rs       # Memory tools
    │   │   └── web.rs          # Web tools
    │   └── execution.rs        # Tool execution
    ├── models/
    │   ├── mod.rs
    │   ├── registry.rs         # Model registry
    │   ├── provider.rs         # Provider trait
    │   ├── anthropic.rs        # Anthropic provider
    │   ├── openai.rs           # OpenAI provider
    │   └── fallback.rs         # Model fallback logic
    ├── workspace/
    │   ├── mod.rs
    │   └── loader.rs           # Workspace file loading
    └── subagent.rs             # Subagent spawning
```

### Module Structure

```rust
// src/lib.rs
pub mod runtime;
pub mod session;
pub mod tools;
pub mod models;
pub mod workspace;
pub mod subagent;

pub use runtime::{AgentRuntime, AgentEvent, AgentStream};
pub use session::{Session, SessionKey, SessionStore};
pub use tools::{ToolDefinition, ToolRegistry, ToolPolicy};
pub use models::{ModelRegistry, ModelProvider, ModelRef};
```

## 6. Sandbox Crate (`openclaw-sandbox`)

Process isolation with seccomp and landlock.

```
crates/openclaw-sandbox/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── manager.rs              # Sandbox manager
    ├── sandbox.rs              # Sandbox struct
    ├── seccomp/
    │   ├── mod.rs
    │   ├── filter.rs           # Syscall filters
    │   └── profiles.rs         # Predefined profiles
    ├── landlock/
    │   ├── mod.rs
    │   └── rules.rs            # Filesystem rules
    ├── namespace.rs            # Namespace isolation
    ├── limits.rs               # Resource limits
    ├── pty.rs                  # PTY handling
    └── execution.rs            # Command execution
```

### Platform Support

```rust
// src/lib.rs

#[cfg(target_os = "linux")]
mod seccomp;
#[cfg(target_os = "linux")]
mod landlock;
#[cfg(target_os = "linux")]
mod namespace;

mod manager;
mod sandbox;
mod limits;
mod pty;
mod execution;

// Re-export platform-appropriate implementation
pub use manager::SandboxManager;
pub use sandbox::Sandbox;
pub use limits::ResourceLimits;

/// Feature availability check
pub fn check_features() -> SandboxFeatures {
    SandboxFeatures {
        seccomp: cfg!(target_os = "linux") && seccomp::is_available(),
        landlock: cfg!(target_os = "linux") && landlock::is_available(),
        namespaces: cfg!(target_os = "linux") && namespace::is_available(),
    }
}
```

## 7. Channels Crate (`openclaw-channels`)

Channel abstraction layer.

```
crates/openclaw-channels/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── traits/
    │   ├── mod.rs
    │   ├── channel.rs          # Core channel trait
    │   ├── secure.rs           # Security extensions
    │   ├── group.rs            # Group management
    │   ├── threading.rs        # Threading support
    │   └── streaming.rs        # Message streaming
    ├── manager.rs              # Channel manager
    ├── router.rs               # Message routing
    ├── delivery.rs             # Delivery pipeline
    ├── queue.rs                # Outbound queue
    ├── types/
    │   ├── mod.rs
    │   ├── inbound.rs          # Inbound message types
    │   ├── outbound.rs         # Outbound message types
    │   ├── media.rs            # Media types
    │   └── target.rs           # Target types
    └── error.rs                # Channel errors
```

## 8. Memory Crate (`openclaw-memory`)

Vector search and embeddings.

```
crates/openclaw-memory/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── manager.rs              # Memory manager
    ├── embeddings/
    │   ├── mod.rs
    │   ├── provider.rs         # Embedding provider trait
    │   ├── openai.rs           # OpenAI embeddings
    │   └── gemini.rs           # Gemini embeddings
    ├── vector/
    │   ├── mod.rs
    │   ├── store.rs            # Vector store
    │   └── search.rs           # Vector search
    ├── hybrid.rs               # Hybrid search (vector + BM25)
    └── types.rs                # Memory types
```

## 9. CLI Crate (`openclaw-cli`)

Command-line interface.

```
crates/openclaw-cli/
├── Cargo.toml
└── src/
    ├── main.rs
    ├── commands/
    │   ├── mod.rs
    │   ├── gateway.rs          # gateway subcommand
    │   ├── agent.rs            # agent subcommand
    │   ├── send.rs             # send subcommand
    │   ├── channels.rs         # channels subcommand
    │   ├── config.rs           # config subcommand
    │   ├── models.rs           # models subcommand
    │   ├── memory.rs           # memory subcommand
    │   ├── sessions.rs         # sessions subcommand
    │   ├── nodes.rs            # nodes subcommand
    │   └── health.rs           # health subcommand
    ├── output/
    │   ├── mod.rs
    │   ├── table.rs            # Table formatting
    │   ├── json.rs             # JSON output
    │   └── progress.rs         # Progress indicators
    └── prompt.rs               # Interactive prompts
```

### CLI Structure

```rust
// src/main.rs
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "openclaw")]
#[command(about = "OpenClaw AI Agent Gateway")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Configuration profile
    #[arg(long, global = true)]
    profile: Option<String>,

    /// Output format
    #[arg(long, global = true, default_value = "text")]
    format: OutputFormat,

    /// Verbose output
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the gateway server
    Gateway(commands::gateway::Args),

    /// Send a message to an agent
    Agent(commands::agent::Args),

    /// Send a message to a channel
    Send(commands::send::Args),

    /// Manage channels
    Channels(commands::channels::Args),

    /// Manage configuration
    Config(commands::config::Args),

    /// Manage models
    Models(commands::models::Args),

    /// Memory management
    Memory(commands::memory::Args),

    /// Session management
    Sessions(commands::sessions::Args),

    /// Node management
    Nodes(commands::nodes::Args),

    /// Health check
    Health(commands::health::Args),
}
```

## 10. Plugin SDK Crate (`openclaw-plugin-sdk`)

Plugin development kit.

```
crates/openclaw-plugin-sdk/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── plugin.rs               # Plugin trait
    ├── context.rs              # Plugin context
    ├── channel.rs              # Channel plugin helpers
    ├── tool.rs                 # Tool plugin helpers
    ├── config.rs               # Config schema helpers
    └── macros.rs               # Convenience macros
```

### Plugin Interface

```rust
// src/lib.rs
pub mod plugin;
pub mod context;
pub mod channel;
pub mod tool;
pub mod config;

pub use plugin::Plugin;
pub use context::PluginContext;

/// Plugin entry point macro
#[macro_export]
macro_rules! declare_plugin {
    ($plugin_type:ty) => {
        #[no_mangle]
        pub extern "C" fn _openclaw_plugin_create() -> *mut dyn Plugin {
            let plugin: Box<dyn Plugin> = Box::new(<$plugin_type>::default());
            Box::into_raw(plugin)
        }
    };
}
```

## 11. Channel Implementations

Each channel is a separate crate.

```
channels/openclaw-telegram/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── channel.rs              # TelegramChannel implementation
    ├── client.rs               # Bot API client
    ├── types.rs                # Telegram-specific types
    ├── polling.rs              # Long polling
    └── webhook.rs              # Webhook handler
```

### Channel Crate Pattern

```toml
# channels/openclaw-telegram/Cargo.toml
[package]
name = "openclaw-telegram"

[dependencies]
openclaw-channels = { path = "../../crates/openclaw-channels" }
openclaw-core = { path = "../../crates/openclaw-core" }

# Telegram-specific
teloxide = { version = "0.12", features = ["macros"] }

tokio.workspace = true
serde.workspace = true
tracing.workspace = true
```

```rust
// channels/openclaw-telegram/src/lib.rs
mod channel;
mod client;
mod types;
mod polling;
mod webhook;

pub use channel::TelegramChannel;

/// Create a Telegram channel from configuration
pub fn create(config: &TelegramConfig) -> Result<TelegramChannel, ChannelError> {
    TelegramChannel::new(config)
}
```

## 12. Feature Flags

Use Cargo features for optional functionality.

```toml
# crates/openclaw-gateway/Cargo.toml
[features]
default = ["telegram", "discord"]

# Channel features
telegram = ["openclaw-telegram"]
discord = ["openclaw-discord"]
slack = ["openclaw-slack"]
signal = ["openclaw-signal"]
whatsapp = ["openclaw-whatsapp"]

# Security features
sandbox = ["openclaw-sandbox"]
audit = []

# Storage features
sqlite = ["sqlx/sqlite"]
postgres = ["sqlx/postgres"]

# Observability
metrics = ["prometheus"]
tracing-otel = ["opentelemetry"]
```

## 13. Build Configuration

### `.cargo/config.toml`

```toml
[build]
# Use mold linker for faster builds (Linux)
# rustflags = ["-C", "link-arg=-fuse-ld=mold"]

[target.x86_64-unknown-linux-gnu]
rustflags = ["-C", "target-feature=+crt-static"]

[target.x86_64-unknown-linux-musl]
rustflags = ["-C", "target-feature=+crt-static"]

[alias]
xtask = "run --package xtask --"
```

### `rust-toolchain.toml`

```toml
[toolchain]
channel = "stable"
components = ["rustfmt", "clippy"]
```

## 14. Testing Organization

```
tests/
├── integration/
│   ├── gateway_test.rs
│   ├── agent_test.rs
│   ├── channel_test.rs
│   └── sandbox_test.rs
└── e2e/
    ├── telegram_e2e.rs
    └── discord_e2e.rs
```

### Test Utilities

```rust
// tests/common/mod.rs
use openclaw_gateway::Gateway;
use openclaw_core::Config;

pub struct TestGateway {
    pub gateway: Gateway,
    pub config: Config,
    pub port: u16,
}

impl TestGateway {
    pub async fn new() -> Self {
        let port = portpicker::pick_unused_port().unwrap();
        let config = Config::default();
        let gateway = Gateway::builder()
            .config(config.clone())
            .bind(([127, 0, 0, 1], port).into())
            .build()
            .await
            .unwrap();

        Self { gateway, config, port }
    }

    pub fn ws_url(&self) -> String {
        format!("ws://127.0.0.1:{}/", self.port)
    }
}
```

## 15. Documentation

### Crate Documentation

Each crate should have comprehensive docs:

```rust
//! # openclaw-core
//!
//! Core types and utilities for the OpenClaw AI Agent Gateway.
//!
//! ## Overview
//!
//! This crate provides shared functionality used across all OpenClaw crates:
//!
//! - **Configuration**: Loading, validation, and management of config files
//! - **Types**: Common type definitions for messages, sessions, and agents
//! - **Utilities**: Path resolution, ID generation, and environment handling
//!
//! ## Example
//!
//! ```rust
//! use openclaw_core::{Config, paths};
//!
//! // Load configuration
//! let config_path = paths::config_file()?;
//! let config = Config::load(&config_path)?;
//! ```
```

### Module Organization Rules

1. **Public API at crate root**: Re-export commonly used types from `lib.rs`
2. **Internal modules**: Use `mod.rs` pattern for organization
3. **Feature gating**: Use `#[cfg(feature = "...")]` for optional code
4. **Error types**: Define errors in dedicated `error.rs` files
5. **Tests**: Inline unit tests, integration tests in `tests/`

## 16. Dependency Management

### Version Pinning

```toml
# Use workspace dependencies for consistency
[workspace.dependencies]
tokio = "1.35"  # Pin major.minor

# Pin exact versions for security-critical deps
ring = "=0.17.7"
```

### Dependency Audit

```bash
# Regular audit
cargo audit

# Check for advisories
cargo deny check advisories

# Check for duplicate deps
cargo deny check bans
```

### Minimum Supported Rust Version

```toml
[package]
rust-version = "1.75"  # Explicit MSRV
```

## 17. Release Configuration

```toml
# Cargo.toml (workspace)
[profile.release]
lto = "thin"
codegen-units = 1
panic = "abort"
strip = true

[profile.release-debug]
inherits = "release"
debug = true
strip = false
```
