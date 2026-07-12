use crate::config::{Config, HostConfig, Policy};
use crate::engine::Engine;
use crate::service::DelugeClientService;
use anyhow::{Context, Result};
use std::sync::Arc;
use std::time::Duration;
use tokio::signal::ctrl_c;
use tokio::time::timeout;
use tokio_cron_scheduler::{Job, JobScheduler};
use tracing::{info, warn};

pub struct Scheduler {
    config: Config,
    dry_run: bool,
    delete_delay: Duration,
}

impl Scheduler {
    pub fn new(config: Config, dry_run: bool, delete_delay: Duration) -> Self {
        Self {
            config,
            dry_run,
            delete_delay,
        }
    }

    pub async fn start(&self) -> Result<()> {
        let mut sched = JobScheduler::new().await?;

        for policy in &self.config.policies {
            let job = Job::new_async(&policy.cron, {
                let dry_run = self.dry_run;
                let delete_delay = self.delete_delay;
                let hosts = self.config.hosts.clone();
                let policy = policy.clone();
                move |_uuid, _l| {
                    let policy = policy.clone();
                    let hosts = hosts.clone();

                    Box::pin(async move {
                        run_policy_across_hosts(&policy, &hosts, dry_run, delete_delay).await;
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

async fn run_policy_across_hosts(
    policy: &Policy,
    hosts: &[HostConfig],
    dry_run: bool,
    delete_delay: Duration,
) {
    for host in hosts {
        info!("Running policy '{}' for host '{}'.", policy.name, host.name);

        let service = Arc::new(DelugeClientService::new(
            &host.host,
            host.port,
            &host.username,
            &host.password,
        ));
        let engine = Engine::new(service, dry_run, delete_delay);

        let result = timeout(Duration::from_secs(300), engine.run_policy(policy)).await;

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
