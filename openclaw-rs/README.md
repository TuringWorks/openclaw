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
| `openclaw-providers` | Model provider integrations (Anthropic, OpenAI, Google) |

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
# Start the gateway with Anthropic (default)
ANTHROPIC_API_KEY=your-key cargo run -p openclaw-cli -- gateway run

# Start with OpenAI
OPENAI_API_KEY=your-key cargo run -p openclaw-cli -- gateway run --provider openai

# Start with Google Gemini
GOOGLE_API_KEY=your-key cargo run -p openclaw-cli -- gateway run --provider google

# Start on a specific port with custom model
ANTHROPIC_API_KEY=xxx cargo run -p openclaw-cli -- gateway run --port 18789 --model claude-opus-4-20250514

# Show help
cargo run -p openclaw-cli -- --help
```

Environment variables:
- `ANTHROPIC_API_KEY` - Anthropic Claude API key
- `OPENAI_API_KEY` - OpenAI API key
- `GOOGLE_API_KEY` or `GEMINI_API_KEY` - Google Gemini API key
- `OPENCLAW_PROVIDER` - Default provider (anthropic, openai, google)
- `OPENCLAW_MODEL` - Default model to use

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

The agent includes 99 built-in tools:

### File System
- `read` - Read file contents
- `write` - Write file contents
- `edit` - Edit files with diff-based changes
- `glob` - Find files by pattern
- `grep` - Search file contents

### File Operations
- `file_copy` - Copy files to a new location
- `file_move` - Move or rename files
- `file_stat` - Get file/directory information (size, timestamps, permissions)
- `file_delete` - Delete files or directories (requires approval)

### Archive
- `zip` - Create/extract zip archives
- `tar` - Create/extract tar archives (with gzip support)

### Checksum
- `file_checksum` - Compute file hash (MD5, SHA1, SHA256, SHA512)
- `file_verify` - Verify file matches expected hash

### Template
- `template` - Substitute variables in template strings
- `format` - Format values (JSON, numbers, bytes, durations)

### System
- `bash` - Execute shell commands

### Process
- `process_list` - List running processes
- `process_info` - Get current process information

### Utility
- `sleep` - Wait for a specified duration
- `temp_file` - Create a temporary file
- `temp_dir` - Create a temporary directory
- `echo` - Echo a value back (useful for testing)

### Environment
- `env_get` - Get environment variable value
- `env_list` - List environment variables
- `env_check` - Check if environment variables exist

### HTTP
- `http_request` - Make HTTP requests to APIs
- `url_parse` - Parse URL into components
- `url_build` - Build URL with query parameters

### Network
- `dns_lookup` - DNS record lookup
- `port_check` - Check TCP port connectivity
- `http_ping` - Check HTTP/HTTPS endpoint reachability
- `net_info` - Get network interface information

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

### Notebook
- `notebook_edit` - Edit Jupyter notebook cells

### Code Intelligence
- `lsp` - Language Server Protocol for go-to-definition, find-references, hover

### Task Management
- `task_create` - Create tasks to track work
- `task_list` - List all tasks
- `task_update` - Update task status
- `task_get` - Get task details

### Interactive
- `ask_user` - Ask user questions with multiple choice options
- `confirm` - Request user confirmation for actions

### Planning
- `enter_plan_mode` - Enter planning mode for implementation design
- `exit_plan_mode` - Exit planning mode and submit plan for approval

### Skills
- `skill` - Invoke a registered skill (slash command)
- `skill_list` - List available skills

### Diagnostics
- `system_info` - Get system information (OS, architecture, environment)
- `health_check` - Check agent health and status
- `diagnostic` - Run diagnostics to troubleshoot issues

### Context Management
- `context_add` - Add information to working context for later reference
- `context_get` - Retrieve entries from working context
- `context_clear` - Clear the working context

### Diff & Patch
- `diff` - Generate diffs between text or files
- `patch` - Preview and apply search/replace changes

### Git
- `git_status` - Get repository status (staged, modified, untracked)
- `git_log` - View commit history
- `git_diff` - View changes
- `git_branch` - List and manage branches

### JSON/YAML
- `json_query` - Query JSON data using path expressions
- `json_transform` - Transform JSON (pick, omit, rename, flatten)
- `yaml` - Parse and convert between YAML and JSON

### Encoding & Hashing
- `base64` - Base64 encode/decode
- `hex` - Hexadecimal encode/decode
- `hash` - Compute hashes (MD5, SHA1, SHA256, SHA512)
- `url_encode` - URL encode/decode

### Time & Date
- `now` - Get current date and time
- `date_parse` - Parse and format date strings
- `date_calc` - Date calculations (add, subtract, diff)

### String Manipulation
- `case` - Convert case (upper, lower, camel, snake, kebab)
- `split_join` - Split and join strings
- `replace` - Text replacement with regex support
- `trim_pad` - Trim whitespace or pad strings

### Math & Random
- `calc` - Mathematical calculations (add, sqrt, power, etc.)
- `random` - Generate random numbers, strings, or pick items
- `uuid` - Generate UUIDs

### Validation
- `validate` - Validate formats (email, URL, JSON, UUID, IP, etc.)
- `is_empty` - Check if value is empty, null, or blank

### Comparison & Assertions
- `compare` - Compare two values (eq, ne, lt, gt, contains, starts_with, ends_with)
- `assert` - Assert a condition is true (returns error if false)
- `match` - Match text against regex patterns with capture groups
- `version_compare` - Compare semantic version strings

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

## Model Providers

The `openclaw-providers` crate includes integrations for major AI providers:

### Anthropic Claude
- Models: `claude-opus-4-20250514`, `claude-sonnet-4-20250514`, `claude-3-5-haiku-20241022`
- Features: Streaming, tool calling, vision, 200K context

### OpenAI GPT
- Models: `gpt-4o`, `gpt-4o-mini`, `o1`, `o3-mini`
- Features: Streaming, tool calling, vision, 128K context

### Google Gemini
- Models: `gemini-2.0-flash`, `gemini-1.5-pro`
- Features: Streaming, tool calling, vision, 2M context

```rust
use openclaw_providers::{anthropic::AnthropicProvider, Provider};

let provider = AnthropicProvider::from_env()?;
let response = provider.chat(
    "claude-sonnet-4-20250514",
    &[Message::user("Hello!")],
    None,
).await?;
```

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
