//! Agent management commands.

use clap::Args;

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
    }

    Ok(())
}
