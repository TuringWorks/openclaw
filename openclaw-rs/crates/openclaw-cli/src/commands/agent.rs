//! Agent management commands.

use crate::repl::{Repl, ReplConfig};
use clap::Args;
use openclaw_agent::providers::anthropic::AnthropicProvider;
use openclaw_agent::runtime::AgentRuntime;
use openclaw_agent::session::SessionManager;
use openclaw_agent::tools::ToolRegistry;
use openclaw_core::types::{AgentConfig, AgentId, SessionKey};
use std::sync::Arc;

/// Agent command arguments.
#[derive(Args)]
pub struct AgentArgs {
    #[command(subcommand)]
    pub command: AgentCommand,
}

#[derive(clap::Subcommand)]
pub enum AgentCommand {
    /// List configured agents
    List,

    /// Show agent details
    Show {
        /// Agent ID
        id: String,
    },

    /// Create a new agent
    Create {
        /// Agent ID
        id: String,

        /// Model to use
        #[arg(short, long)]
        model: Option<String>,

        /// System prompt
        #[arg(short, long)]
        system: Option<String>,
    },

    /// Delete an agent
    Delete {
        /// Agent ID
        id: String,
    },

    /// Send a message to an agent
    Message {
        /// Agent ID
        #[arg(short, long)]
        agent: Option<String>,

        /// Message text
        message: String,
    },

    /// Start an interactive chat session
    Chat {
        /// Agent ID
        #[arg(short, long)]
        agent: Option<String>,

        /// Model override
        #[arg(short, long)]
        model: Option<String>,

        /// System prompt
        #[arg(short, long)]
        system: Option<String>,

        /// Resume session ID
        #[arg(long)]
        session: Option<String>,

        /// Provider (default: anthropic)
        #[arg(short, long, default_value = "anthropic")]
        provider: String,
    },
}

/// Run the agent command.
pub async fn run(args: AgentArgs) -> anyhow::Result<()> {
    match args.command {
        AgentCommand::List => {
            println!("Configured agents:");
            // List agents from config
        }

        AgentCommand::Show { id } => {
            println!("Agent: {}", id);
            // Show agent details
        }

        AgentCommand::Create { id, model, system: _ } => {
            println!("Creating agent: {}", id);
            if let Some(m) = model {
                println!("  Model: {}", m);
            }
            // Create agent in config
        }

        AgentCommand::Delete { id } => {
            println!("Deleting agent: {}", id);
            // Delete agent from config
        }

        AgentCommand::Message { agent, message } => {
            let agent_id = agent.unwrap_or_else(|| "default".to_string());
            println!("Sending to {}: {}", agent_id, message);
            // Send message via gateway
        }

        AgentCommand::Chat {
            agent,
            model,
            system,
            session,
            provider: _,
        } => {
            let agent_id_str = agent.unwrap_or_else(|| "default".to_string());
            let agent_id = AgentId::new(&agent_id_str);

            // Build agent config
            let config = AgentConfig {
                id: agent_id.clone(),
                model,
                system_prompt: system,
                ..AgentConfig::default()
            };

            // Resolve API key from env
            let api_key = std::env::var("ANTHROPIC_API_KEY")
                .or_else(|_| std::env::var("OPENAI_API_KEY"))
                .map_err(|_| anyhow::anyhow!(
                    "No API key found. Set ANTHROPIC_API_KEY or run `openclaw init`."
                ))?;

            // Create provider
            let provider: Arc<dyn openclaw_agent::providers::ModelProvider> =
                Arc::new(AnthropicProvider::new(api_key));

            // Create tool registry and session manager
            let tool_registry = Arc::new(ToolRegistry::new());
            let sessions_dir = openclaw_core::paths::sessions_dir()
                .map_err(|e| anyhow::anyhow!("Failed to get sessions dir: {}", e))?;
            let session_manager = Arc::new(SessionManager::new(sessions_dir));

            // Create runtime
            let runtime = Arc::new(
                AgentRuntime::new(config, provider, tool_registry, session_manager)
            );

            // Create or resume session
            let session_key = match session {
                Some(id) => SessionKey::new(format!("{}:{}", agent_id_str, id)),
                None => SessionKey::new(format!(
                    "{}:{}",
                    agent_id_str,
                    openclaw_core::id::uuid()
                )),
            };

            // Launch REPL
            let mut repl = Repl::new(runtime, session_key, ReplConfig::default());
            repl.run().await?;
        }
    }

    Ok(())
}
