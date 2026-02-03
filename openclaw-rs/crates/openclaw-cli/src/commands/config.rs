//! Configuration management commands.

use clap::Args;
use openclaw_core::config::Config;
use openclaw_core::paths;

/// Config command arguments.
#[derive(Args)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub command: ConfigCommand,
}

#[derive(clap::Subcommand)]
pub enum ConfigCommand {
    /// Show configuration
    Show,

    /// Get a configuration value
    Get {
        /// Configuration key (dot-separated path)
        key: String,
    },

    /// Set a configuration value
    Set {
        /// Configuration key
        key: String,

        /// Value to set
        value: String,
    },

    /// Initialize configuration
    Init {
        /// Force overwrite existing config
        #[arg(short, long)]
        force: bool,
    },

    /// Show configuration file path
    Path,

    /// Validate configuration
    Validate,
}

/// Run the config command.
pub async fn run(args: ConfigArgs) -> anyhow::Result<()> {
    match args.command {
        ConfigCommand::Show => {
            let config = Config::load_default().unwrap_or_default();
            let json = serde_json::to_string_pretty(&config)?;
            println!("{}", json);
        }

        ConfigCommand::Get { key } => {
            let config = Config::load_default().unwrap_or_default();
            let json = serde_json::to_value(&config)?;

            let value = key.split('.').fold(Some(&json), |acc, k| {
                acc.and_then(|v| v.get(k))
            });

            match value {
                Some(v) => println!("{}", serde_json::to_string_pretty(v)?),
                None => anyhow::bail!("Key not found: {}", key),
            }
        }

        ConfigCommand::Set { key, value } => {
            println!("Setting {} = {}", key, value);
            // Would update config file
        }

        ConfigCommand::Init { force } => {
            let path = paths::config_file()?;

            if path.exists() && !force {
                anyhow::bail!(
                    "Config file already exists: {:?}. Use --force to overwrite.",
                    path
                );
            }

            paths::ensure_dirs()?;

            let config = Config::default();
            config.save_default()?;

            println!("Created config file: {:?}", path);
        }

        ConfigCommand::Path => {
            let path = paths::config_file()?;
            println!("{}", path.display());
        }

        ConfigCommand::Validate => {
            match Config::load_default() {
                Ok(config) => {
                    match config.validate() {
                        Ok(_) => println!("Configuration is valid"),
                        Err(e) => anyhow::bail!("Configuration error: {}", e),
                    }
                }
                Err(e) => anyhow::bail!("Failed to load config: {}", e),
            }
        }
    }

    Ok(())
}
