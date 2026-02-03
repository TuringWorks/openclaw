# OpenClaw Rust Agent Execution Specification

## 1. Agent Model Overview

Agents are AI-powered conversational entities that:

- Receive messages from messaging channels
- Maintain conversation context via sessions
- Call AI model APIs (Claude, OpenAI, etc.)
- Execute tools to interact with the environment
- Return responses to messaging channels

## 2. Agent Configuration

### 2.1 Agent Definition

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AgentConfig {
    /// Unique agent identifier (normalized, lowercase)
    pub id: String,

    /// Optional display name
    pub name: Option<String>,

    /// Workspace directory for agent files
    pub workspace_dir: Option<PathBuf>,

    /// Primary model to use
    pub model: Option<ModelRef>,

    /// Fallback models if primary fails
    pub fallback_models: Vec<ModelRef>,

    /// System prompt override
    pub system_prompt: Option<String>,

    /// Extended thinking level
    pub thinking_level: ThinkingLevel,

    /// Tool policy
    pub tools: ToolPolicyConfig,

    /// Memory/search configuration
    pub memory: MemoryConfig,

    /// Sandbox configuration
    pub sandbox: Option<SandboxConfig>,

    /// Subagent spawning rules
    pub subagents: SubagentConfig,

    /// Identity (name, emoji, avatar)
    pub identity: Option<AgentIdentity>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, Default)]
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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SubagentConfig {
    /// Allowed agent IDs that can be spawned
    pub allow_agents: Vec<String>,

    /// Default model for spawned subagents
    pub model: Option<ModelRef>,

    /// Default thinking level for subagents
    pub thinking: Option<ThinkingLevel>,

    /// Tool policy overrides for subagents
    pub tool_policy: Option<ToolPolicyConfig>,
}
```

### 2.2 Agent Identity

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AgentIdentity {
    pub name: String,
    pub emoji: Option<String>,
    pub avatar_url: Option<String>,
    pub theme_color: Option<String>,
}
```

### 2.3 Workspace Files

Agent workspaces contain instruction files:

```text
workspace/
├── AGENTS.md      # Agent behavior instructions
├── SOUL.md        # Personality/tone instructions
├── TOOLS.md       # Tool usage guidelines
├── IDENTITY.md    # Identity configuration
└── *.md           # Additional context files
```

```rust
#[derive(Debug, Clone)]
pub struct Workspace {
    pub path: PathBuf,
    pub agents_md: Option<String>,
    pub soul_md: Option<String>,
    pub tools_md: Option<String>,
    pub identity_md: Option<String>,
    pub additional_files: HashMap<String, String>,
}

impl Workspace {
    pub async fn load(path: &Path) -> Result<Self, WorkspaceError> {
        let mut workspace = Self {
            path: path.to_path_buf(),
            ..Default::default()
        };

        // Load standard files
        workspace.agents_md = Self::read_optional(path.join("AGENTS.md")).await?;
        workspace.soul_md = Self::read_optional(path.join("SOUL.md")).await?;
        workspace.tools_md = Self::read_optional(path.join("TOOLS.md")).await?;
        workspace.identity_md = Self::read_optional(path.join("IDENTITY.md")).await?;

        // Load additional .md files
        let mut entries = tokio::fs::read_dir(path).await?;
        while let Some(entry) = entries.next_entry().await? {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.ends_with(".md") && !["AGENTS.md", "SOUL.md", "TOOLS.md", "IDENTITY.md"].contains(&name.as_str()) {
                if let Ok(content) = tokio::fs::read_to_string(entry.path()).await {
                    workspace.additional_files.insert(name, content);
                }
            }
        }

        Ok(workspace)
    }

    /// Build system prompt from workspace files
    pub fn build_system_prompt(&self) -> String {
        let mut prompt = String::new();

        if let Some(agents) = &self.agents_md {
            prompt.push_str(agents);
            prompt.push_str("\n\n");
        }

        if let Some(soul) = &self.soul_md {
            prompt.push_str(soul);
            prompt.push_str("\n\n");
        }

        if let Some(tools) = &self.tools_md {
            prompt.push_str(tools);
        }

        prompt
    }
}
```

## 3. Session Management

### 3.1 Session Model

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Session {
    /// Unique session key
    pub key: SessionKey,

    /// Agent this session belongs to
    pub agent_id: String,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last activity timestamp
    pub last_message_at: DateTime<Utc>,

    /// Conversation messages
    pub messages: Vec<Message>,

    /// Token usage statistics
    pub tokens: TokenUsage,

    /// Cost tracking (optional)
    pub cost: Option<CostUsage>,

    /// Current model for this session
    pub model: Option<ModelRef>,

    /// Thinking level override
    pub thinking_level: Option<ThinkingLevel>,

    /// Typing indicator mode
    pub type_mode: TypeMode,

    /// Session metadata
    pub metadata: SessionMetadata,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SessionKey {
    /// Format: "channel:account:peer" or "agent:id:subagent:uuid"
    inner: String,
}

impl SessionKey {
    /// Create a session key for a channel message
    pub fn for_channel(channel: &str, account: &str, peer: &str) -> Self {
        Self {
            inner: format!("{}:{}:{}", channel, account, peer),
        }
    }

    /// Create a session key for a subagent
    pub fn for_subagent(parent_agent: &str) -> Self {
        let uuid = Uuid::new_v4();
        Self {
            inner: format!("agent:{}:subagent:{}", parent_agent, uuid),
        }
    }

    /// Parse agent ID from session key
    pub fn agent_id(&self) -> Option<&str> {
        if self.inner.starts_with("agent:") {
            self.inner.split(':').nth(1)
        } else {
            None
        }
    }

    /// Check if this is a subagent session
    pub fn is_subagent(&self) -> bool {
        self.inner.contains(":subagent:")
    }
}
```

### 3.2 Message Types

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Message {
    pub role: Role,
    pub content: MessageContent,
    pub name: Option<String>,
    pub tool_use_id: Option<String>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
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

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text { text: String },
    Image { source: ImageSource },
    ToolUse { id: String, name: String, input: Value },
    ToolResult { tool_use_id: String, content: String, is_error: bool },
    Thinking { thinking: String },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ImageSource {
    pub media_type: String,
    pub data: String, // base64
}
```

### 3.3 Session Storage

```rust
pub struct SessionStore {
    /// SQLite connection pool for metadata
    db: SqlitePool,

    /// Directory for session transcript files
    sessions_dir: PathBuf,
}

impl SessionStore {
    pub async fn new(db_path: &Path, sessions_dir: &Path) -> Result<Self, SessionError> {
        let db = SqlitePool::connect(&format!("sqlite:{}", db_path.display())).await?;

        // Run migrations
        sqlx::migrate!("./migrations").run(&db).await?;

        tokio::fs::create_dir_all(sessions_dir).await?;

        Ok(Self {
            db,
            sessions_dir: sessions_dir.to_path_buf(),
        })
    }

    /// Get or create a session
    pub async fn get_or_create(
        &self,
        key: &SessionKey,
        agent_id: &str,
    ) -> Result<Session, SessionError> {
        // Check if session exists in DB
        let existing = sqlx::query_as::<_, SessionMetadata>(
            "SELECT * FROM sessions WHERE key = ?"
        )
        .bind(key.as_str())
        .fetch_optional(&self.db)
        .await?;

        if let Some(metadata) = existing {
            // Load messages from file
            let messages = self.load_messages(key).await?;
            return Ok(Session::from_metadata(metadata, messages));
        }

        // Create new session
        let session = Session::new(key.clone(), agent_id.to_string());
        self.save(&session).await?;
        Ok(session)
    }

    /// Append a message to session transcript
    pub async fn append_message(
        &self,
        key: &SessionKey,
        message: &Message,
    ) -> Result<(), SessionError> {
        let path = self.transcript_path(key);

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await?;

        let json = serde_json::to_string(message)?;
        file.write_all(json.as_bytes()).await?;
        file.write_all(b"\n").await?;

        // Update last_message_at in DB
        sqlx::query("UPDATE sessions SET last_message_at = ? WHERE key = ?")
            .bind(Utc::now())
            .bind(key.as_str())
            .execute(&self.db)
            .await?;

        Ok(())
    }

    /// Load messages from transcript file
    async fn load_messages(&self, key: &SessionKey) -> Result<Vec<Message>, SessionError> {
        let path = self.transcript_path(key);

        if !path.exists() {
            return Ok(Vec::new());
        }

        let content = tokio::fs::read_to_string(&path).await?;
        let messages: Vec<Message> = content
            .lines()
            .filter(|line| !line.is_empty())
            .map(|line| serde_json::from_str(line))
            .collect::<Result<_, _>>()?;

        Ok(messages)
    }

    fn transcript_path(&self, key: &SessionKey) -> PathBuf {
        // Hash key for filename
        let hash = sha256_hex(key.as_str());
        self.sessions_dir.join(format!("{}.jsonl", &hash[..16]))
    }
}
```

## 4. Agent Runtime

### 4.1 Runtime Structure

```rust
pub struct AgentRuntime {
    /// Model registry for resolving model references
    model_registry: Arc<ModelRegistry>,

    /// Tool registry with available tools
    tool_registry: Arc<ToolRegistry>,

    /// Session storage
    session_store: Arc<SessionStore>,

    /// Sandbox manager for tool execution
    sandbox_manager: Arc<SandboxManager>,

    /// Approval manager for sensitive operations
    approval_manager: Arc<ApprovalManager>,

    /// HTTP client for API calls
    http_client: reqwest::Client,

    /// Configuration
    config: Arc<RwLock<Config>>,
}

impl AgentRuntime {
    /// Invoke an agent with a message
    pub async fn invoke(
        &self,
        session_key: &SessionKey,
        message: InboundMessage,
        agent_config: &AgentConfig,
    ) -> Result<AgentStream, AgentError> {
        // Load or create session
        let session = self.session_store
            .get_or_create(session_key, &agent_config.id)
            .await?;

        // Build execution context
        let context = self.build_context(&session, &message, agent_config).await?;

        // Create agent stream
        let (tx, rx) = mpsc::channel(32);

        // Spawn agent execution task
        let runtime = self.clone();
        let session_key = session_key.clone();
        tokio::spawn(async move {
            if let Err(e) = runtime.run_agent_loop(context, tx.clone()).await {
                let _ = tx.send(AgentEvent::Error { message: e.to_string() }).await;
            }
        });

        Ok(AgentStream { receiver: rx })
    }

    /// Main agent execution loop
    async fn run_agent_loop(
        &self,
        mut context: ExecutionContext,
        tx: mpsc::Sender<AgentEvent>,
    ) -> Result<(), AgentError> {
        loop {
            // Call model API
            let response = self.call_model(&context).await?;

            // Process response blocks
            for block in response.content {
                match block {
                    ContentBlock::Text { text } => {
                        tx.send(AgentEvent::TextBlock { text }).await?;
                    }

                    ContentBlock::ToolUse { id, name, input } => {
                        tx.send(AgentEvent::ToolUse {
                            id: id.clone(),
                            name: name.clone(),
                            input: input.clone(),
                        }).await?;

                        // Execute tool
                        let result = self.execute_tool(&name, input, &context).await;

                        let (output, is_error) = match result {
                            Ok(output) => (output, false),
                            Err(e) => (Value::String(e.to_string()), true),
                        };

                        tx.send(AgentEvent::ToolResult {
                            id: id.clone(),
                            output: output.clone(),
                        }).await?;

                        // Add tool result to context
                        context.messages.push(Message {
                            role: Role::Assistant,
                            content: MessageContent::Blocks(vec![
                                ContentBlock::ToolUse { id: id.clone(), name, input }
                            ]),
                            name: None,
                            tool_use_id: None,
                            timestamp: Utc::now(),
                        });

                        context.messages.push(Message {
                            role: Role::Tool,
                            content: MessageContent::Blocks(vec![
                                ContentBlock::ToolResult {
                                    tool_use_id: id,
                                    content: serde_json::to_string(&output)?,
                                    is_error,
                                }
                            ]),
                            name: None,
                            tool_use_id: None,
                            timestamp: Utc::now(),
                        });
                    }

                    ContentBlock::Thinking { thinking } => {
                        tx.send(AgentEvent::Thinking { text: thinking }).await?;
                    }

                    _ => {}
                }
            }

            // Check if agent is done (no tool use in response)
            if !response.content.iter().any(|b| matches!(b, ContentBlock::ToolUse { .. })) {
                tx.send(AgentEvent::Done { usage: response.usage }).await?;
                break;
            }
        }

        Ok(())
    }
}
```

### 4.2 Agent Events (Streaming)

```rust
#[derive(Debug, Clone)]
pub enum AgentEvent {
    /// Text output from the agent
    TextBlock { text: String },

    /// Agent is calling a tool
    ToolUse { id: String, name: String, input: Value },

    /// Tool execution completed
    ToolResult { id: String, output: Value },

    /// Thinking block (extended thinking)
    Thinking { text: String },

    /// Agent completed
    Done { usage: TokenUsage },

    /// Error occurred
    Error { message: String },
}

pub struct AgentStream {
    receiver: mpsc::Receiver<AgentEvent>,
}

impl AgentStream {
    pub async fn next(&mut self) -> Option<AgentEvent> {
        self.receiver.recv().await
    }
}

impl futures::Stream for AgentStream {
    type Item = AgentEvent;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.receiver).poll_recv(cx)
    }
}
```

### 4.3 Execution Context

```rust
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    /// Session key
    pub session_key: SessionKey,

    /// Agent configuration
    pub agent_config: AgentConfig,

    /// Conversation messages
    pub messages: Vec<Message>,

    /// Available tools (filtered by policy)
    pub tools: Vec<ToolDefinition>,

    /// System prompt
    pub system_prompt: String,

    /// Model to use
    pub model: ModelRef,

    /// Thinking level
    pub thinking_level: ThinkingLevel,

    /// Workspace path
    pub workspace: PathBuf,

    /// Channel context (for channel-aware tools)
    pub channel_context: Option<ChannelContext>,
}

#[derive(Debug, Clone)]
pub struct ChannelContext {
    pub channel: String,
    pub account_id: String,
    pub sender_id: String,
    pub chat_id: String,
    pub is_group: bool,
}
```

## 5. Tool System

### 5.1 Tool Definition

```rust
#[derive(Debug, Clone)]
pub struct ToolDefinition {
    /// Tool name (unique identifier)
    pub name: String,

    /// Human-readable description
    pub description: String,

    /// JSON Schema for input validation
    pub input_schema: Value,

    /// Tool handler
    pub handler: ToolHandler,

    /// Execution requirements
    pub execution: ToolExecution,
}

pub type ToolHandler = Arc<
    dyn Fn(Value, ToolContext) -> BoxFuture<'static, Result<Value, ToolError>>
        + Send
        + Sync
>;

#[derive(Debug, Clone)]
pub struct ToolExecution {
    /// Where to execute this tool
    pub host: ExecutionHost,

    /// Whether this tool requires approval
    pub requires_approval: bool,

    /// Sandbox profile to use
    pub sandbox_profile: SandboxProfile,

    /// Resource limits
    pub resource_limits: Option<ResourceLimits>,
}

#[derive(Debug, Clone)]
pub enum ExecutionHost {
    /// Execute in sandbox on gateway
    Sandbox,

    /// Execute directly on gateway (no sandbox)
    Gateway,

    /// Execute on a remote node
    Node(String),

    /// Execute in Docker container
    Docker(String),
}
```

### 5.2 Built-in Tools

```rust
pub fn create_builtin_tools(config: &ToolsConfig) -> Vec<ToolDefinition> {
    vec![
        // Filesystem tools
        create_read_tool(),
        create_write_tool(),
        create_edit_tool(),
        create_glob_tool(),
        create_grep_tool(),

        // Runtime tools
        create_exec_tool(config),
        create_process_tool(config),

        // Browser tools
        create_browser_tool(config),

        // Session tools
        create_session_status_tool(),
        create_sessions_list_tool(),
        create_sessions_history_tool(),
        create_sessions_send_tool(),
        create_sessions_spawn_tool(),

        // Memory tools
        create_memory_search_tool(),
        create_memory_get_tool(),

        // Web tools
        create_web_search_tool(),
        create_web_fetch_tool(),

        // Messaging tools
        create_message_tool(),

        // Node tools
        create_nodes_tool(),
    ]
}
```

### 5.3 Exec Tool Implementation

```rust
fn create_exec_tool(config: &ToolsConfig) -> ToolDefinition {
    ToolDefinition {
        name: "exec".to_string(),
        description: "Execute a shell command".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The command to execute"
                },
                "timeout_ms": {
                    "type": "integer",
                    "description": "Timeout in milliseconds (default: 120000)"
                },
                "cwd": {
                    "type": "string",
                    "description": "Working directory"
                }
            },
            "required": ["command"]
        }),
        handler: Arc::new(move |input, ctx| {
            Box::pin(exec_handler(input, ctx))
        }),
        execution: ToolExecution {
            host: ExecutionHost::Sandbox,
            requires_approval: true,
            sandbox_profile: SandboxProfile::Standard,
            resource_limits: Some(ResourceLimits::default()),
        },
    }
}

async fn exec_handler(input: Value, ctx: ToolContext) -> Result<Value, ToolError> {
    let command = input["command"].as_str()
        .ok_or(ToolError::InvalidInput("command is required".into()))?;

    let timeout_ms = input["timeout_ms"].as_u64().unwrap_or(120_000);
    let cwd = input["cwd"].as_str().map(PathBuf::from);

    // Validate environment variables
    if let Some(env) = input.get("env").and_then(|e| e.as_object()) {
        let env_map: HashMap<String, String> = env
            .iter()
            .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
            .collect();
        validate_env(&env_map)?;
    }

    // Check approval if required
    if ctx.requires_approval {
        let response = ctx.approval_manager
            .request_approval(command, &ctx.execution_context)
            .await?;

        match response {
            ApprovalResponse::Denied { reason } => {
                return Err(ToolError::ApprovalDenied(reason));
            }
            ApprovalResponse::Timeout => {
                return Err(ToolError::ApprovalTimeout);
            }
            ApprovalResponse::Approved => {}
        }
    }

    // Execute in sandbox
    let result = ctx.sandbox_manager
        .execute(
            &ctx.sandbox,
            command,
            &ctx.env,
            Duration::from_millis(timeout_ms),
        )
        .await?;

    Ok(json!({
        "exit_code": result.exit_code,
        "stdout": String::from_utf8_lossy(&result.stdout),
        "stderr": String::from_utf8_lossy(&result.stderr),
        "duration_ms": result.duration.as_millis(),
    }))
}
```

### 5.4 Tool Context

```rust
#[derive(Clone)]
pub struct ToolContext {
    /// Execution context
    pub execution_context: ExecutionContext,

    /// Sandbox for this execution
    pub sandbox: Sandbox,

    /// Sandbox manager
    pub sandbox_manager: Arc<SandboxManager>,

    /// Approval manager
    pub approval_manager: Arc<ApprovalManager>,

    /// Audit log
    pub audit_log: Arc<AuditLog>,

    /// Whether this tool requires approval
    pub requires_approval: bool,

    /// Environment variables
    pub env: HashMap<String, String>,

    /// Working directory
    pub cwd: PathBuf,
}
```

## 6. Subagent Spawning

### 6.1 Spawn Tool

```rust
fn create_sessions_spawn_tool() -> ToolDefinition {
    ToolDefinition {
        name: "sessions_spawn".to_string(),
        description: "Spawn a subagent to handle a task".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "agent_id": {
                    "type": "string",
                    "description": "Target agent ID to spawn"
                },
                "prompt": {
                    "type": "string",
                    "description": "Instructions for the subagent"
                },
                "model": {
                    "type": "string",
                    "description": "Model override for subagent"
                }
            },
            "required": ["agent_id", "prompt"]
        }),
        handler: Arc::new(|input, ctx| Box::pin(spawn_handler(input, ctx))),
        execution: ToolExecution {
            host: ExecutionHost::Gateway,
            requires_approval: false,
            sandbox_profile: SandboxProfile::None,
            resource_limits: None,
        },
    }
}

async fn spawn_handler(input: Value, ctx: ToolContext) -> Result<Value, ToolError> {
    let agent_id = input["agent_id"].as_str()
        .ok_or(ToolError::InvalidInput("agent_id required".into()))?;
    let prompt = input["prompt"].as_str()
        .ok_or(ToolError::InvalidInput("prompt required".into()))?;

    // Check if current session is already a subagent
    if ctx.execution_context.session_key.is_subagent() {
        return Err(ToolError::NotAllowed(
            "Subagents cannot spawn other subagents".into()
        ));
    }

    // Check if agent is in allowlist
    let parent_config = &ctx.execution_context.agent_config;
    if !parent_config.subagents.allow_agents.contains(&agent_id.to_string()) {
        return Err(ToolError::NotAllowed(format!(
            "Agent '{}' is not in allowed subagents list",
            agent_id
        )));
    }

    // Create subagent session key
    let subagent_session_key = SessionKey::for_subagent(&parent_config.id);

    // Build subagent message
    let subagent_message = InboundMessage {
        text: prompt.to_string(),
        sender_id: "parent".to_string(),
        ..Default::default()
    };

    // Load subagent config with restricted tools
    let subagent_config = ctx.agent_registry
        .get_agent_config(agent_id)
        .await?
        .with_tool_policy(parent_config.subagents.tool_policy.clone());

    // Invoke subagent
    let stream = ctx.agent_runtime
        .invoke(&subagent_session_key, subagent_message, &subagent_config)
        .await?;

    // Collect subagent response
    let mut response_text = String::new();
    let mut stream = stream;
    while let Some(event) = stream.next().await {
        match event {
            AgentEvent::TextBlock { text } => {
                response_text.push_str(&text);
            }
            AgentEvent::Error { message } => {
                return Err(ToolError::SubagentError(message));
            }
            _ => {}
        }
    }

    Ok(json!({
        "response": response_text,
        "session_key": subagent_session_key.as_str(),
    }))
}
```

### 6.2 Subagent Restrictions

Subagents have restricted capabilities by default:

```rust
/// Default tools denied for subagents
pub const DEFAULT_SUBAGENT_TOOL_DENY: &[&str] = &[
    "sessions_spawn",      // No nested spawning
    "sessions_list",       // Parent orchestrates
    "sessions_history",    // Parent orchestrates
    "sessions_send",       // Parent sends messages
    "gateway",             // System admin
    "agents_list",         // System admin
    "memory_search",       // Pass info in prompt instead
    "memory_get",          // Pass info in prompt instead
    "cron",                // No scheduling
    "session_status",      // Parent tracks status
];

impl AgentConfig {
    pub fn with_subagent_restrictions(mut self) -> Self {
        let mut deny = self.tools.deny.clone();
        for tool in DEFAULT_SUBAGENT_TOOL_DENY {
            deny.push(ToolPattern::Exact(tool.to_string()));
        }
        self.tools.deny = deny;
        self
    }
}
```

## 7. Model Integration

### 7.1 Model Registry

```rust
pub struct ModelRegistry {
    /// Available models
    models: HashMap<String, ModelInfo>,

    /// Model aliases
    aliases: HashMap<String, String>,

    /// Provider clients
    providers: HashMap<String, Box<dyn ModelProvider>>,
}

#[derive(Debug, Clone)]
pub struct ModelInfo {
    pub id: String,
    pub provider: String,
    pub display_name: String,
    pub capabilities: ModelCapabilities,
    pub context_window: usize,
    pub max_output_tokens: usize,
}

#[derive(Debug, Clone, Default)]
pub struct ModelCapabilities {
    pub vision: bool,
    pub tool_use: bool,
    pub extended_thinking: bool,
    pub streaming: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ModelRef {
    pub provider: String,
    pub model_id: String,
}

impl ModelRef {
    pub fn parse(s: &str) -> Result<Self, ModelError> {
        let parts: Vec<&str> = s.splitn(2, '/').collect();
        if parts.len() != 2 {
            return Err(ModelError::InvalidFormat(s.to_string()));
        }
        Ok(Self {
            provider: parts[0].to_string(),
            model_id: parts[1].to_string(),
        })
    }
}
```

### 7.2 Model Provider Trait

```rust
#[async_trait]
pub trait ModelProvider: Send + Sync {
    fn id(&self) -> &str;

    /// List available models
    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError>;

    /// Create a chat completion
    async fn create_completion(
        &self,
        request: CompletionRequest,
    ) -> Result<CompletionStream, ProviderError>;
}

pub struct CompletionRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub tools: Vec<ToolDefinition>,
    pub system_prompt: Option<String>,
    pub max_tokens: Option<usize>,
    pub temperature: Option<f32>,
    pub thinking: Option<ThinkingConfig>,
}

pub struct ThinkingConfig {
    pub enabled: bool,
    pub budget_tokens: Option<usize>,
}

pub struct CompletionStream {
    receiver: mpsc::Receiver<CompletionEvent>,
}

pub enum CompletionEvent {
    ContentBlockStart { index: usize, block_type: String },
    ContentBlockDelta { index: usize, delta: ContentDelta },
    ContentBlockStop { index: usize },
    MessageDelta { usage: TokenUsage },
    MessageStop,
    Error { message: String },
}
```

### 7.3 Provider Implementations

```rust
pub struct AnthropicProvider {
    client: reqwest::Client,
    api_key: SecretString,
    base_url: String,
}

#[async_trait]
impl ModelProvider for AnthropicProvider {
    fn id(&self) -> &str {
        "anthropic"
    }

    async fn create_completion(
        &self,
        request: CompletionRequest,
    ) -> Result<CompletionStream, ProviderError> {
        let body = self.build_request_body(&request)?;

        let response = self.client
            .post(&format!("{}/v1/messages", self.base_url))
            .header("x-api-key", self.api_key.expose_secret())
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;

        // Stream SSE events
        let (tx, rx) = mpsc::channel(32);
        tokio::spawn(async move {
            Self::stream_response(response, tx).await;
        });

        Ok(CompletionStream { receiver: rx })
    }
}
```

## 8. Memory Integration

### 8.1 Memory Manager

```rust
pub struct MemoryManager {
    /// Vector store for embeddings
    vector_store: Arc<VectorStore>,

    /// Embedding provider
    embeddings: Arc<dyn EmbeddingProvider>,

    /// SQLite for metadata
    db: SqlitePool,
}

impl MemoryManager {
    /// Search memory for relevant context
    pub async fn search(
        &self,
        query: &str,
        agent_id: &str,
        limit: usize,
    ) -> Result<Vec<MemoryResult>, MemoryError> {
        // Generate embedding for query
        let embedding = self.embeddings.embed(query).await?;

        // Search vector store
        let results = self.vector_store
            .search(&embedding, agent_id, limit)
            .await?;

        Ok(results)
    }

    /// Add memory entry
    pub async fn add(
        &self,
        content: &str,
        agent_id: &str,
        metadata: MemoryMetadata,
    ) -> Result<(), MemoryError> {
        let embedding = self.embeddings.embed(content).await?;

        self.vector_store
            .insert(&embedding, content, agent_id, &metadata)
            .await?;

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct MemoryResult {
    pub content: String,
    pub score: f32,
    pub metadata: MemoryMetadata,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MemoryMetadata {
    pub session_key: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub source: String,
}
```

### 8.2 Vector Store

```rust
pub struct VectorStore {
    /// sqlite-vec for vector operations
    db: SqlitePool,
}

impl VectorStore {
    pub async fn search(
        &self,
        query_embedding: &[f32],
        agent_id: &str,
        limit: usize,
    ) -> Result<Vec<MemoryResult>, VectorError> {
        let embedding_bytes = embedding_to_bytes(query_embedding);

        let results = sqlx::query_as::<_, MemoryRow>(
            r#"
            SELECT content, metadata,
                   vec_distance_cosine(embedding, ?) as distance
            FROM memories
            WHERE agent_id = ?
            ORDER BY distance ASC
            LIMIT ?
            "#
        )
        .bind(&embedding_bytes)
        .bind(agent_id)
        .bind(limit as i64)
        .fetch_all(&self.db)
        .await?;

        Ok(results.into_iter().map(|r| r.into()).collect())
    }
}
```

## 9. Error Handling

### 9.1 Agent Errors

```rust
#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("Session error: {0}")]
    Session(#[from] SessionError),

    #[error("Model error: {0}")]
    Model(#[from] ModelError),

    #[error("Tool error: {tool}: {source}")]
    Tool { tool: String, #[source] source: ToolError },

    #[error("Sandbox error: {0}")]
    Sandbox(#[from] SandboxError),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Rate limited: retry after {retry_after:?}")]
    RateLimited { retry_after: Duration },

    #[error("Context length exceeded: {used} / {max}")]
    ContextLengthExceeded { used: usize, max: usize },
}

#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Approval denied: {0}")]
    ApprovalDenied(String),

    #[error("Approval timeout")]
    ApprovalTimeout,

    #[error("Not allowed: {0}")]
    NotAllowed(String),

    #[error("Subagent error: {0}")]
    SubagentError(String),

    #[error("Sandbox violation: {0}")]
    SandboxViolation(String),
}
```

### 9.2 Recovery Strategies

```rust
impl AgentRuntime {
    async fn handle_model_error(
        &self,
        error: &ModelError,
        context: &ExecutionContext,
    ) -> Result<ModelRef, AgentError> {
        match error {
            ModelError::RateLimited { retry_after } => {
                // Try fallback model
                if let Some(fallback) = self.get_next_fallback(context).await? {
                    return Ok(fallback);
                }
                Err(AgentError::RateLimited { retry_after: *retry_after })
            }

            ModelError::AuthenticationFailed => {
                // Mark credential as failed, try next profile
                self.credential_manager.mark_failed(&context.model).await;
                if let Some(next_profile) = self.get_next_auth_profile(&context.model).await? {
                    return Ok(context.model.clone());
                }
                Err(AgentError::Config("No valid credentials".into()))
            }

            _ => Err(AgentError::Model(error.clone())),
        }
    }
}
```
