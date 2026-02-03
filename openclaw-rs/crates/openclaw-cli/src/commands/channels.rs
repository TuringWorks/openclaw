//! Channel management commands.

use clap::Args;

/// Channels command arguments.
#[derive(Args)]
pub struct ChannelsArgs {
    #[command(subcommand)]
    pub command: ChannelsCommand,
}

#[derive(clap::Subcommand)]
pub enum ChannelsCommand {
    /// List configured channels
    List,

    /// Show channel status
    Status {
        /// Probe channels for connectivity
        #[arg(long)]
        probe: bool,
    },

    /// Enable a channel
    Enable {
        /// Channel name
        channel: String,
    },

    /// Disable a channel
    Disable {
        /// Channel name
        channel: String,
    },

    /// Configure a channel
    Configure {
        /// Channel name
        channel: String,

        /// Configuration key=value pairs
        #[arg(short, long)]
        set: Vec<String>,
    },
}

/// Run the channels command.
pub async fn run(args: ChannelsArgs) -> anyhow::Result<()> {
    match args.command {
        ChannelsCommand::List => {
            println!("Configured channels:");
            println!("  telegram");
            println!("  discord");
            println!("  slack");
            println!("  signal");
            // List from config
        }

        ChannelsCommand::Status { probe } => {
            println!("Channel status:");
            if probe {
                println!("  Probing channels...");
            }
            // Show status from gateway
        }

        ChannelsCommand::Enable { channel } => {
            println!("Enabling channel: {}", channel);
            // Enable in config
        }

        ChannelsCommand::Disable { channel } => {
            println!("Disabling channel: {}", channel);
            // Disable in config
        }

        ChannelsCommand::Configure { channel, set } => {
            println!("Configuring channel: {}", channel);
            for kv in set {
                println!("  {}", kv);
            }
            // Update config
        }
    }

    Ok(())
}
