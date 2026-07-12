use crate::config::Config;
use crate::scheduler::Scheduler;
use anyhow::{Context, Result};
use clap::Parser;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;
use tracing::debug;

#[derive(Parser, Debug)]
#[command(
    name = "deluge-maintain",
    version,
    about = "A service that puts deluge on autopilot using retention policies",
    author = "Mark Lopez <m@silvenga.com>"
)]
pub struct CliConfig {
    /// Path to the TOML configuration file.
    #[arg(long, env = "DELUGE_MAINTAIN_CONFIG")]
    pub config: PathBuf,

    /// Simulate policy enforcement without making changes.
    #[arg(long, env = "DELUGE_MAINTAIN_DRY_RUN", default_value_t = false)]
    pub dry_run: bool,

    /// Delay between torrent deletions, in seconds.
    #[arg(long, env = "DELUGE_MAINTAIN_DELETE_DELAY", default_value_t = 1)]
    pub delete_delay: u64,
}

pub struct Cli;

impl Cli {
    pub async fn run() -> Result<()> {
        let (cli, config) = build_config().await?;
        Scheduler::new(config, cli.dry_run, Duration::from_secs(cli.delete_delay))
            .start()
            .await?;
        Ok(())
    }
}

async fn build_config() -> Result<(CliConfig, Config)> {
    let cli = CliConfig::parse();

    let config_contents = fs::read_to_string(&cli.config)
        .with_context(|| format!("Failed to read config file {}.", cli.config.display()))?;
    let config = Config::load(&config_contents)?;

    debug!("Loaded configuration: {:#?}.", config);
    Ok((cli, config))
}
