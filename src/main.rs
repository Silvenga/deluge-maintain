mod cli;
mod config;
mod engine;
mod scheduler;
mod service;

use crate::cli::Cli;
use std::process;
use tracing::error;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    log_panics::init();

    if let Err(e) = Cli::run().await {
        error!("Unrecoverable error occurred: {:#}", e);
        process::exit(1);
    }
}
