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
    policy_timeout: Duration,
}

impl<E: Engine + 'static> Scheduler<E> {
    pub fn new(config: Config, engine: E, policy_timeout: Duration) -> Self {
        Self {
            config,
            engine: Arc::from(engine),
            policy_timeout,
        }
    }

    pub async fn start(&self) -> Result<()> {
        let mut sched = JobScheduler::new().await?;

        for policy in &self.config.policies {
            let job = Job::new_async(&policy.cron, {
                let policy = policy.clone();
                let hosts = self.config.hosts.clone();
                let engine = self.engine.clone();
                let timeout = self.policy_timeout;
                move |_uuid, _l| {
                    let policy = policy.clone();
                    let hosts = hosts.clone();
                    let engine = engine.clone();
                    Box::pin(async move {
                        run_policy_across_hosts(&policy, &hosts, engine.clone(), timeout).await;
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

async fn run_policy_across_hosts<E: Engine>(
    policy: &Policy,
    hosts: &[HostConfig],
    engine: Arc<E>,
    policy_timeout: Duration,
) {
    for host in hosts {
        info!("Running policy '{}' for host '{}'.", policy.name, host.name);

        let result = timeout(policy_timeout, engine.run_policy(policy, host)).await;
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
                    "Policy '{}' timed out for host '{}' after {} seconds. \
                     Check if the Deluge instance is responsive.",
                    policy.name,
                    host.name,
                    policy_timeout.as_secs()
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, HostConfig, Policy};
    use mockall::mock;
    use std::sync::Arc;
    use tokio::time::sleep;

    #[tokio::test]
    async fn when_engine_returns_err_then_next_host_should_still_be_processed() {
        let mut engine = MockEngine::new();
        engine
            .expect_run_policy()
            .withf(|_, host| host.name == "host-a")
            .times(1)
            .returning(|_, _| Err(anyhow::anyhow!("host unreachable")));
        engine
            .expect_run_policy()
            .withf(|_, host| host.name == "host-b")
            .times(1)
            .returning(|_, _| Ok(()));

        let policy = make_policy();
        let hosts = vec![make_host("host-a"), make_host("host-b")];

        run_policy_across_hosts(&policy, &hosts, Arc::new(engine), Duration::from_secs(300)).await;
    }

    #[tokio::test]
    async fn when_multiple_hosts_then_all_should_be_processed() {
        let mut engine = MockEngine::new();
        engine
            .expect_run_policy()
            .withf(|_, host| host.name == "host-a")
            .times(1)
            .returning(|_, _| Ok(()));
        engine
            .expect_run_policy()
            .withf(|_, host| host.name == "host-b")
            .times(1)
            .returning(|_, _| Ok(()));
        engine
            .expect_run_policy()
            .withf(|_, host| host.name == "host-c")
            .times(1)
            .returning(|_, _| Ok(()));

        let policy = make_policy();
        let hosts = vec![
            make_host("host-a"),
            make_host("host-b"),
            make_host("host-c"),
        ];

        run_policy_across_hosts(&policy, &hosts, Arc::new(engine), Duration::from_secs(300)).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn when_scheduler_started_then_engine_should_be_invoked_for_configured_host_and_policy() {
        let config = Config {
            hosts: vec![HostConfig {
                name: "test-host".to_owned(),
                host: "127.0.0.1".to_owned(),
                port: 58846,
                username: "user".to_owned(),
                password: "pass".to_owned(),
            }],
            policies: vec![Policy {
                name: "test-policy".to_owned(),
                cron: "*/1 * * * * *".to_owned(),
                filter: Default::default(),
                conditions: Default::default(),
            }],
        };

        let mut engine = MockEngine::new();
        engine
            .expect_run_policy()
            .withf(|policy, host| policy.name == "test-policy" && host.name == "test-host")
            .times(1)
            .returning(|_, _| Ok(()));

        let scheduler = Scheduler::new(config, engine, Duration::from_secs(300));

        let handle = tokio::spawn(async move {
            let _ = scheduler.start().await;
        });

        sleep(Duration::from_millis(1200)).await;

        handle.abort();
    }

    mock! {
        pub Engine {}

        #[async_trait::async_trait]
        impl Engine for Engine {
            async fn run_policy(&self, policy: &Policy, host: &HostConfig) -> anyhow::Result<()>;
        }
    }

    fn make_host(name: &str) -> HostConfig {
        HostConfig {
            name: name.to_owned(),
            host: "127.0.0.1".to_owned(),
            port: 58846,
            username: "user".to_owned(),
            password: "pass".to_owned(),
        }
    }

    fn make_policy() -> Policy {
        Policy {
            name: "test-policy".to_owned(),
            cron: "*/1 * * * * *".to_owned(),
            filter: Default::default(),
            conditions: Default::default(),
        }
    }
}
