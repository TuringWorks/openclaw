//! Gateway command.

use clap::Args;
use openclaw_core::config::BindMode;
use openclaw_gateway::{Gateway, GatewayConfig};
use openclaw_providers::{
    anthropic::AnthropicProvider, google::GoogleProvider, openai::OpenAIProvider, Provider,
};
use std::sync::Arc;
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

        /// Model provider (anthropic, openai, google)
        #[arg(long, env = "OPENCLAW_PROVIDER", default_value = "anthropic")]
        provider: String,

        /// Default model to use
        #[arg(long, env = "OPENCLAW_MODEL")]
        model: Option<String>,

        /// Authentication token for non-loopback connections (CVE-2026-25253 mitigation)
        #[arg(long, env = "OPENCLAW_AUTH_TOKEN")]
        auth_token: Option<String>,
    },

    /// Stop the gateway server
    Stop,

    /// Show gateway status
    Status,
}

/// Run the gateway command.
pub async fn run(args: GatewayArgs) -> anyhow::Result<()> {
    match args.command {
        GatewayCommand::Run {
            bind,
            port,
            force: _,
            provider,
            model,
            auth_token,
        } => {
            let bind_mode = match bind.as_str() {
                "loopback" => BindMode::Loopback,
                "lan" => BindMode::Lan,
                "tailnet" => BindMode::Tailnet,
                "auto" => BindMode::Auto,
                _ => {
                    anyhow::bail!("Invalid bind mode: {}", bind);
                }
            };

            // Require auth for non-loopback binds when a token is provided
            let require_auth = auth_token.is_some() && bind_mode != BindMode::Loopback;

            let config = GatewayConfig {
                bind: bind_mode,
                port,
                auth_token,
                require_auth,
                ..Default::default()
            };

            // Try to create provider from environment
            let provider_instance: Option<Arc<dyn Provider>> = match provider.as_str() {
                "anthropic" => {
                    match AnthropicProvider::from_env() {
                        Ok(p) => {
                            let p = if let Some(ref m) = model {
                                p.with_default_model(m.clone())
                            } else {
                                p
                            };
                            info!("Using Anthropic provider");
                            Some(Arc::new(p))
                        }
                        Err(e) => {
                            info!("Anthropic provider not configured: {}", e);
                            None
                        }
                    }
                }
                "openai" => {
                    match OpenAIProvider::from_env() {
                        Ok(p) => {
                            let p = if let Some(ref m) = model {
                                p.with_default_model(m.clone())
                            } else {
                                p
                            };
                            info!("Using OpenAI provider");
                            Some(Arc::new(p))
                        }
                        Err(e) => {
                            info!("OpenAI provider not configured: {}", e);
                            None
                        }
                    }
                }
                "google" => {
                    match GoogleProvider::from_env() {
                        Ok(p) => {
                            let p = if let Some(ref m) = model {
                                p.with_default_model(m.clone())
                            } else {
                                p
                            };
                            info!("Using Google provider");
                            Some(Arc::new(p))
                        }
                        Err(e) => {
                            info!("Google provider not configured: {}", e);
                            None
                        }
                    }
                }
                other => {
                    anyhow::bail!("Unknown provider: {}. Valid options: anthropic, openai, google", other);
                }
            };

            info!("Starting gateway on port {} with 54 RPC methods", port);

            // Create gateway with provider if available
            let gateway = if let Some(provider) = provider_instance {
                Gateway::with_provider(config, provider).await
            } else {
                info!("No provider configured, chat will return echo responses");
                Gateway::with_default_handlers(config).await
            };

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
