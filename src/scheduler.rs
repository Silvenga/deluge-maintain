use crate::config::{Config, HostConfig, Policy};
use crate::engine::Engine;
use anyhow::{Context, Result};
use std::sync::Arc;
use std::time::Duration;
use tokio::signal::ctrl_c;
use tokio::time::timeout;
use tokio_cron_scheduler::{Job, JobScheduler};
use tracing::{info, warn};

const TIMEOUT: Duration = Duration::from_secs(300);

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

        let result = timeout(TIMEOUT, engine.run_policy(policy, host)).await;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, HostConfig, Policy};
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::{Arc, Mutex};
    use tokio::time::sleep;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn when_scheduler_started_then_engine_should_be_invoked_for_configured_host_and_policy() {
        let config = test_config();
        let engine = MockEngine::new();

        let scheduler = Scheduler::new(config, engine.clone());

        let handle = tokio::spawn(async move {
            let _ = scheduler.start().await;
        });

        sleep(Duration::from_millis(1200)).await;

        handle.abort();

        let calls = engine.call_count();

        assert!(
            calls >= 1,
            "Expected engine to be invoked at least once, got {calls} calls"
        );

        assert_eq!(
            engine.last_policy().as_deref(),
            Some("test-policy"),
            "Engine should receive the configured policy name"
        );
        assert_eq!(
            engine.last_host().as_deref(),
            Some("test-host"),
            "Engine should receive the configured host name"
        );
    }

    struct MockState {
        call_count: AtomicU32,
        last_policy: Mutex<Option<String>>,
        last_host: Mutex<Option<String>>,
    }

    impl MockState {
        fn new() -> Self {
            Self {
                call_count: AtomicU32::new(0),
                last_policy: Mutex::new(None),
                last_host: Mutex::new(None),
            }
        }
    }

    #[derive(Clone)]
    struct MockEngine {
        state: Arc<MockState>,
    }

    impl MockEngine {
        fn new() -> Self {
            Self {
                state: Arc::new(MockState::new()),
            }
        }

        fn call_count(&self) -> u32 {
            self.state.call_count.load(Ordering::SeqCst)
        }

        fn last_policy(&self) -> Option<String> {
            self.state.last_policy.lock().unwrap().clone()
        }

        fn last_host(&self) -> Option<String> {
            self.state.last_host.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl Engine for MockEngine {
        async fn run_policy(&self, policy: &Policy, host: &HostConfig) -> Result<()> {
            self.state.call_count.fetch_add(1, Ordering::SeqCst);
            *self.state.last_policy.lock().unwrap() = Some(policy.name.clone());
            *self.state.last_host.lock().unwrap() = Some(host.name.clone());
            Ok(())
        }
    }

    fn test_config() -> Config {
        Config {
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
        }
    }
}
