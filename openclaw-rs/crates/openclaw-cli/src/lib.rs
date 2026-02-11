//! OpenClaw command-line interface.

pub mod commands;
pub mod onboard;
pub mod render;
pub mod repl;

use clap::{Parser, Subcommand};

/// OpenClaw - AI agent gateway
#[derive(Parser)]
#[command(name = "openclaw")]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Increase logging verbosity
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Path to config file
    #[arg(short, long, env = "OPENCLAW_CONFIG")]
    pub config: Option<std::path::PathBuf>,

    #[command(subcommand)]
    pub command: Commands,
}

/// Available commands
#[derive(Subcommand)]
pub enum Commands {
    /// Start the gateway server
    Gateway(commands::gateway::GatewayArgs),

    /// Manage agents
    Agent(commands::agent::AgentArgs),

    /// Manage channels
    Channels(commands::channels::ChannelsArgs),

    /// Configuration management
    Config(commands::config::ConfigArgs),

    /// Run diagnostics
    Doctor(commands::doctor::DoctorArgs),

    /// Manage encrypted secrets
    Secrets(commands::secrets::SecretsArgs),

    /// Initialize OpenClaw configuration
    Init {
        /// Overwrite existing configuration
        #[arg(long)]
        force: bool,
    },

    /// Show version information
    Version,
}

/// Run the CLI with the given arguments.
pub async fn run(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        Commands::Gateway(args) => commands::gateway::run(args).await,
        Commands::Agent(args) => commands::agent::run(args).await,
        Commands::Channels(args) => commands::channels::run(args).await,
        Commands::Config(args) => commands::config::run(args).await,
        Commands::Doctor(args) => commands::doctor::run(args).await,
        Commands::Secrets(args) => commands::secrets::run(args).await,
        Commands::Init { force } => {
            onboard::OnboardWizard::new(force).run().await
        }
        Commands::Version => {
            println!("openclaw {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
    }
}
