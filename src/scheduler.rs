use crate::config::{Config, HostConfig, Policy};
use crate::engine::Engine;
use anyhow::{Context, Result};
use std::sync::Arc;
use std::time::Duration;
use tokio::signal::ctrl_c;
use tokio::time::timeout;
use tokio_cron_scheduler::{Job, JobScheduler};
use tracing::{info, warn};

pub struct Scheduler<E: Engine> {
    config: Config,
    engine: Arc<E>,
}

impl<E: Engine + 'static> Scheduler<E> {
    pub fn new(config: Config, engine: E) -> Self {
        Self {
            config,
            engine: Arc::from(engine),
        }
    }

    pub async fn start(&self) -> Result<()> {
        let mut sched = JobScheduler::new().await?;

        for policy in &self.config.policies {
            let job = Job::new_async(&policy.cron, {
                let policy = policy.clone();
                let hosts = self.config.hosts.clone();
                let engine = self.engine.clone();
                move |_uuid, _l| {
                    let policy = policy.clone();
                    let hosts = hosts.clone();
                    let engine = engine.clone();
                    Box::pin(async move {
                        run_policy_across_hosts(&policy, &hosts, engine.clone()).await;
                    })
                }
            })
            .with_context(|| format!("Failed to create job '{}'", policy.name))?;

            sched
                .add(job)
                .await
                .with_context(|| format!("Failed to add job '{}'", policy.name))?;
        }

        sched.start().await?;
        info!("Scheduler started, waiting for shutdown signal.");

        ctrl_c().await?;
        info!("Shutdown signal received, stopping scheduler.");

        sched.shutdown().await?;

        Ok(())
    }
}

async fn run_policy_across_hosts<E: Engine>(policy: &Policy, hosts: &[HostConfig], engine: Arc<E>) {
    for host in hosts {
        info!("Running policy '{}' for host '{}'.", policy.name, host.name);

        let result = timeout(Duration::from_secs(300), engine.run_policy(policy, host)).await;
        match result {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                warn!(
                    "Policy '{}' failed for host '{}': {:#}",
                    policy.name, host.name, e
                );
            }
            Err(_) => {
                warn!(
                    "Policy '{}' timed out for host '{}' after 300 seconds. \
                     Check if the Deluge instance is responsive.",
                    policy.name, host.name
                );
            }
        }
    }
}
