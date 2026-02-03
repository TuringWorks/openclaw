//! Diagnostic commands.

use clap::Args;
use console::{style, Emoji};
use openclaw_core::config::Config;
use openclaw_core::paths;

static CHECK: Emoji = Emoji("✓", "+");
static CROSS: Emoji = Emoji("✗", "x");
static WARN: Emoji = Emoji("⚠", "!");

/// Doctor command arguments.
#[derive(Args)]
pub struct DoctorArgs {
    /// Run all checks including slow ones
    #[arg(long)]
    pub full: bool,
}

/// Run the doctor command.
pub async fn run(args: DoctorArgs) -> anyhow::Result<()> {
    println!("OpenClaw Doctor\n");

    let mut errors = 0;
    let mut warnings = 0;

    // Check directories
    println!("Checking directories...");

    let base_dir = paths::base_dir();
    match base_dir {
        Ok(dir) => {
            if dir.exists() {
                println!("  {} Base directory exists: {:?}", style(CHECK).green(), dir);
            } else {
                println!("  {} Base directory missing: {:?}", style(WARN).yellow(), dir);
                warnings += 1;
            }
        }
        Err(e) => {
            println!("  {} Failed to determine base directory: {}", style(CROSS).red(), e);
            errors += 1;
        }
    }

    // Check config
    println!("\nChecking configuration...");

    match Config::load_default() {
        Ok(config) => {
            println!("  {} Configuration loaded", style(CHECK).green());

            match config.validate() {
                Ok(_) => {
                    println!("  {} Configuration valid", style(CHECK).green());
                }
                Err(e) => {
                    println!("  {} Configuration invalid: {}", style(CROSS).red(), e);
                    errors += 1;
                }
            }
        }
        Err(openclaw_core::error::ConfigError::NotFound(_)) => {
            println!("  {} Configuration file not found", style(WARN).yellow());
            println!("    Run 'openclaw config init' to create one");
            warnings += 1;
        }
        Err(e) => {
            println!("  {} Configuration error: {}", style(CROSS).red(), e);
            errors += 1;
        }
    }

    // Check environment
    println!("\nChecking environment...");

    if std::env::var("ANTHROPIC_API_KEY").is_ok() {
        println!("  {} ANTHROPIC_API_KEY is set", style(CHECK).green());
    } else {
        println!("  {} ANTHROPIC_API_KEY not set", style(WARN).yellow());
        warnings += 1;
    }

    // Check connectivity (if full)
    if args.full {
        println!("\nChecking connectivity...");
        // Would check API endpoints, etc.
        println!("  {} Skipped (not implemented)", style(WARN).yellow());
    }

    // Summary
    println!("\n{}", style("Summary").bold());
    println!("  Errors: {}", if errors > 0 { style(errors).red() } else { style(errors).green() });
    println!("  Warnings: {}", if warnings > 0 { style(warnings).yellow() } else { style(warnings).green() });

    if errors > 0 {
        anyhow::bail!("{} error(s) found", errors);
    }

    Ok(())
}
