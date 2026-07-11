use crate::config::{Config, HostConfig, PolicyConfig};
use crate::engine::Engine;
use crate::policy::Policy;
use crate::service::DelugeClientService;
use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;
use tokio::signal::ctrl_c;
use tokio::time::timeout;
use tokio_cron_scheduler::{Job, JobScheduler};

pub async fn start(config: &Config, dry_run: bool, delete_delay: Duration) -> Result<()> {
    let mut sched = JobScheduler::new().await?;

    for policy_config in &config.policies {
        let policy = convert_policy(policy_config);
        let hosts = config.hosts.clone();

        let job = Job::new_async(&policy_config.cron, move |_uuid, _l| {
            let policy = policy.clone();
            let hosts = hosts.clone();

            Box::pin(async move {
                run_policy_across_hosts(&policy, &hosts, dry_run, delete_delay).await;
            })
        })?;

        sched.add(job).await?;
    }

    sched.start().await?;

    tracing::info!("scheduler started, waiting for shutdown signal");
    ctrl_c().await?;
    tracing::info!("shutdown signal received, stopping scheduler");
    sched.shutdown().await?;

    Ok(())
}

async fn run_policy_across_hosts(
    policy: &Policy,
    hosts: &[HostConfig],
    dry_run: bool,
    delete_delay: Duration,
) {
    for host in hosts {
        tracing::info!(
            host = %host.name,
            policy = %policy.name,
            "running policy"
        );

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
                tracing::warn!(
                    host = %host.name,
                    policy = %policy.name,
                    error = %e,
                    "policy execution failed"
                );
            }
            Err(_) => {
                tracing::warn!(
                    host = %host.name,
                    policy = %policy.name,
                    "policy execution timed out after 300 seconds"
                );
            }
        }
    }
}

fn convert_policy(config: &PolicyConfig) -> Policy {
    Policy {
        name: config.name.clone(),
        filter: config.filter.clone(),
        conditions: config.conditions.clone(),
    }
}
