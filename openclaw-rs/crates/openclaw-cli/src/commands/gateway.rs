//! Gateway command.

use clap::Args;
use openclaw_core::config::BindMode;
use openclaw_gateway::{Gateway, GatewayConfig};
use tracing::info;

/// Gateway command arguments.
#[derive(Args)]
pub struct GatewayArgs {
    /// Run the gateway
    #[command(subcommand)]
    pub command: GatewayCommand,
}

#[derive(clap::Subcommand)]
pub enum GatewayCommand {
    /// Start the gateway server
    Run {
        /// Bind mode (loopback, lan, tailnet, auto)
        #[arg(short, long, default_value = "loopback")]
        bind: String,

        /// Port number
        #[arg(short, long, default_value = "18789")]
        port: u16,

        /// Force start even if another instance is running
        #[arg(short, long)]
        force: bool,
    },

    /// Stop the gateway server
    Stop,

    /// Show gateway status
    Status,
}

/// Run the gateway command.
pub async fn run(args: GatewayArgs) -> anyhow::Result<()> {
    match args.command {
        GatewayCommand::Run { bind, port, force } => {
            let bind_mode = match bind.as_str() {
                "loopback" => BindMode::Loopback,
                "lan" => BindMode::Lan,
                "tailnet" => BindMode::Tailnet,
                "auto" => BindMode::Auto,
                _ => {
                    anyhow::bail!("Invalid bind mode: {}", bind);
                }
            };

            let config = GatewayConfig {
                bind: bind_mode,
                port,
                ..Default::default()
            };

            info!("Starting gateway on port {}", port);

            let gateway = Gateway::new(config);
            gateway.run().await?;
        }

        GatewayCommand::Stop => {
            println!("Stopping gateway...");
            // Would send stop signal to running gateway
        }

        GatewayCommand::Status => {
            println!("Gateway status:");
            // Would query running gateway
        }
    }

    Ok(())
}
