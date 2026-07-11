use crate::config::{Config, HostConfig, PolicyConfig};
use crate::engine::Engine;
use crate::policy::{Condition, Filter, Policy};
use crate::service::DelugeClientService;
use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;
use tokio_cron_scheduler::{Job, JobScheduler};

pub async fn start(config: &Config, dry_run: bool, delete_delay: Duration) -> Result<()> {
    let sched = JobScheduler::new().await?;

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

        if let Err(e) = engine.run_policy(policy).await {
            tracing::warn!(
                host = %host.name,
                policy = %policy.name,
                error = %e,
                "policy execution failed"
            );
        }
    }
}

fn convert_policy(config: &PolicyConfig) -> Policy {
    Policy {
        name: config.name.clone(),
        filter: Filter {
            age: config.filter.age,
            ratio: config.filter.ratio,
            completed: config.filter.completed,
            min_total_seeds: config.filter.min_total_seeds,
            min_distributed_copies: config.filter.min_distributed_copies,
        },
        conditions: Condition {
            available_space: config.conditions.available_space,
            used_space: config.conditions.used_space,
            total_count: config.conditions.total_count,
        },
    }
}
