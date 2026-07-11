use clap::Parser;
use deluge_maintain::{CliConfig, Config, scheduler_start};
use std::fs;
use std::process;
use std::time::Duration;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    log_panics::init();

    if let Err(e) = run().await {
        tracing::error!(error = %e, "fatal error");
        process::exit(1);
    }
}

async fn run() -> anyhow::Result<()> {
    let cli = CliConfig::parse();

    let config_contents = fs::read_to_string(&cli.config)
        .map_err(|e| anyhow::anyhow!("failed to read config file {}: {e}", cli.config.display()))?;

    let config = Config::load(&config_contents)?;

    tracing::debug!(?config, "loaded configuration");

    scheduler_start(&config, cli.dry_run, Duration::from_secs(cli.delete_delay)).await?;

    Ok(())
}
