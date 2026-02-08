# OpenClaw Rust Implementation

High-performance Rust implementation of the OpenClaw AI agent gateway.

## Crates

| Crate | Description |
|-------|-------------|
| `openclaw-core` | Core types, configuration, and shared utilities |
| `openclaw-sandbox` | Command execution sandboxing with platform-specific profiles |
| `openclaw-channels` | Messaging channel abstractions (Telegram, Discord, Slack, etc.) |
| `openclaw-agent` | Agent runtime with tool execution framework |
| `openclaw-memory` | Memory and context management for agents |
| `openclaw-gateway` | JSON-RPC gateway server over WebSocket |
| `openclaw-cli` | Command-line interface |
| `openclaw-plugin-sdk` | Plugin development kit for extensions |

## Building

```bash
# Build all crates
cargo build --workspace

# Build with release optimizations
cargo build --workspace --release

# Build CLI only
cargo build -p openclaw-cli
```

## Testing

```bash
# Run all tests
cargo test --workspace

# Run tests for a specific crate
cargo test -p openclaw-agent

# Run tests with specific channel features
cargo test -p openclaw-channels --features telegram
```

## Running

```bash
# Start the gateway
cargo run -p openclaw-cli -- gateway run

# Start on a specific port
cargo run -p openclaw-cli -- gateway run --port 18789

# Show help
cargo run -p openclaw-cli -- --help
```

## Channel Features

Messaging channels are feature-gated to reduce compile time and dependencies:

```bash
# Build with Telegram support
cargo build -p openclaw-channels --features telegram

# Build with Discord support
cargo build -p openclaw-channels --features discord

# Build with Slack support
cargo build -p openclaw-channels --features slack

# Build with WebSocket support
cargo build -p openclaw-channels --features web

# Build with all channels
cargo build -p openclaw-channels --features "telegram,discord,slack,web"
```

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        openclaw-cli                              │
│                    (Command-line interface)                      │
└─────────────────────────────────────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────────┐
│                      openclaw-gateway                            │
│              (JSON-RPC over WebSocket server)                    │
│                                                                  │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐            │
│  │  Health  │ │   Chat   │ │ Sessions │ │  Config  │  ...       │
│  │ Handler  │ │ Handler  │ │ Handler  │ │ Handler  │            │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘            │
└─────────────────────────────────────────────────────────────────┘
                               │
          ┌────────────────────┼────────────────────┐
          ▼                    ▼                    ▼
┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐
│  openclaw-agent  │  │ openclaw-channels│  │  openclaw-memory │
│  (Agent runtime) │  │ (Messaging)      │  │ (Context store)  │
│                  │  │                  │  │                  │
│  ┌────────────┐  │  │  ┌──────────┐   │  │  ┌────────────┐  │
│  │ Tools      │  │  │  │ Telegram │   │  │  │ Embeddings │  │
│  ├────────────┤  │  │  ├──────────┤   │  │  ├────────────┤  │
│  │ Sessions   │  │  │  │ Discord  │   │  │  │ Vector DB  │  │
│  ├────────────┤  │  │  ├──────────┤   │  │  └────────────┘  │
│  │ Streaming  │  │  │  │ Slack    │   │  │                  │
│  └────────────┘  │  │  ├──────────┤   │  └──────────────────┘
│                  │  │  │ Web      │   │
└──────────────────┘  │  └──────────┘   │
          │           │                  │
          ▼           └──────────────────┘
┌──────────────────┐
│ openclaw-sandbox │
│ (Cmd execution)  │
└──────────────────┘
          │
          ▼
┌──────────────────┐
│  openclaw-core   │
│ (Types & config) │
└──────────────────┘
```

## Agent Tools

The agent includes 23 built-in tools:

### File System
- `read` - Read file contents
- `write` - Write file contents
- `edit` - Edit files with diff-based changes
- `glob` - Find files by pattern
- `grep` - Search file contents

### System
- `bash` - Execute shell commands

### Web
- `web_fetch` - Fetch and process web pages
- `web_search` - Search the web

### Messaging
- `message` - Send messages
- `sessions_spawn` - Create new agent sessions
- `sessions_send` - Send to existing sessions
- `sessions_list` - List active sessions
- `sessions_history` - Get session history
- `session_status` - Get session status

### Memory
- `memory_search` - Search stored memories
- `memory_get` - Retrieve specific memories

### Automation
- `cron` - Schedule recurring tasks
- `gateway` - Control the gateway
- `nodes` - Manage distributed nodes

### Media
- `image` - Analyze images
- `tts` - Text-to-speech

### Browser
- `browser` - Browser automation

### Channel Actions
- `telegram_actions` - Telegram-specific actions
- `discord_actions` - Discord-specific actions
- `slack_actions` - Slack-specific actions

## Gateway RPC Methods

The gateway exposes 54+ RPC methods for:

- Health monitoring (`gateway.health`, `gateway.presence`)
- Agent management (`agent.run`, `agent.run_stream`, `agent.abort`)
- Chat interface (`chat.send`, `chat.abort`)
- Session management (`sessions.list`, `sessions.get`, `sessions.history`)
- Model management (`models.list`, `models.get`, `models.usage`)
- Configuration (`config.get`, `config.set`, `config.allowlist`)
- Channel management (`channels.list`, `channels.status`, `channels.send`)
- Device pairing (`device.list`, `device.pair`, `device.unpair`)
- Cron jobs (`cron.list`, `cron.create`, `cron.update`, `cron.delete`)
- Skill management (`skills.list`, `skills.run`)
- System operations (`system.info`, `system.logs`, `system.restart`)

## Plugin SDK

Create custom plugins using the SDK:

```rust
use openclaw_plugin_sdk::prelude::*;

pub struct MyPlugin;

#[async_trait]
impl Plugin for MyPlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            name: "my-plugin".to_string(),
            version: Version::parse("1.0.0").unwrap(),
            description: "My custom plugin".to_string(),
            author: Some("Author".to_string()),
            homepage: None,
            license: Some("MIT".to_string()),
            capabilities: vec![PluginCapability::Tool],
            min_openclaw_version: None,
        }
    }

    async fn initialize(&mut self, ctx: &PluginContext) -> Result<()> {
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<()> {
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
}
```

Plugin capabilities:
- `Channel` - Custom messaging channels
- `Tool` - Custom agent tools
- `ModelProvider` - Custom AI model providers
- `Hook` - Middleware and interceptors
- `Storage` - Custom storage backends
- `Media` - Media processing

## License

MIT
