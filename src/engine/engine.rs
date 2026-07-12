use crate::config::{HostConfig, Policy};
use crate::engine::plan_deletions::{DeletionPlan, plan_deletions};
use crate::service::{DelugeService, DelugeServiceFactory};
use anyhow::Context;
use async_trait::async_trait;
use std::time::{Duration, SystemTime};
use tokio::time::sleep;
use tracing::{info, warn};

#[async_trait]
pub trait Engine: Send + Sync {
    async fn run_policy(&self, policy: &Policy, host: &HostConfig) -> anyhow::Result<()>;
}

pub struct DelugeClientEngine<F: DelugeServiceFactory> {
    service_factory: F,
    dry_run: bool,
    delete_delay: Duration,
}

impl<F: DelugeServiceFactory> DelugeClientEngine<F> {
    pub fn new(service_factory: F, dry_run: bool, delete_delay: Duration) -> Self {
        Self {
            service_factory,
            dry_run,
            delete_delay,
        }
    }
}

#[async_trait]
impl<F: DelugeServiceFactory> Engine for DelugeClientEngine<F> {
    async fn run_policy(&self, policy: &Policy, host: &HostConfig) -> anyhow::Result<()> {
        let now = SystemTime::now();

        let service =
            self.service_factory
                .create(&host.host, host.port, &host.username, &host.password);

        let torrents = service.get_torrents().await.context(format!(
            "Failed to fetch torrents for policy '{}'.",
            policy.name
        ))?;

        let free_space = service.get_free_space().await.context(format!(
            "Failed to fetch free space for policy '{}'.",
            policy.name
        ))?;

        let plan = plan_deletions(policy, &torrents, free_space, now);

        match plan {
            DeletionPlan::NothingToDo => {
                info!(
                    "No conditions met for policy '{}', nothing to do.",
                    policy.name
                );
            }
            DeletionPlan::Impossible => {
                warn!(
                    "Conditions cannot be satisfied for policy '{}'. \
                     Consider adjusting condition thresholds or filter criteria.",
                    policy.name
                );
            }
            DeletionPlan::Deletions(to_delete) => {
                info!(
                    "Planned {} deletion(s) for policy '{}' (dry_run: {}).",
                    to_delete.len(),
                    policy.name,
                    self.dry_run
                );

                for (i, torrent) in to_delete.iter().enumerate() {
                    info!(
                        "Deleting torrent '{}' (hash: {}, distributed_copies: {}, \
                         total_wanted: {}) for policy '{}' (dry_run: {}).",
                        torrent.name,
                        torrent.info_hash,
                        torrent.distributed_copies,
                        torrent.total_wanted,
                        policy.name,
                        self.dry_run
                    );

                    if !self.dry_run {
                        service
                            .remove_torrent(&torrent.info_hash, true)
                            .await
                            .with_context(|| {
                                format!(
                                    "Failed to delete torrent '{}' (hash: {}) for policy '{}'.",
                                    torrent.name, torrent.info_hash, policy.name
                                )
                            })?;
                        if i + 1 < to_delete.len() {
                            sleep(self.delete_delay).await;
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Condition, Filter, HostConfig, Policy};
    use crate::service::{DelugeService, DelugeServiceFactory, TorrentEntry};
    use async_trait::async_trait;
    use std::sync::{Arc, Mutex};

    #[tokio::test]
    async fn when_conditions_met_and_not_dry_run_then_torrents_should_be_deleted() {
        let torrents = vec![
            make_torrent("highest_dc", 5.0),
            make_torrent("mid_dc", 3.0),
            make_torrent("lowest_dc", 1.0),
        ];
        let (factory, removed) = make_factory(torrents);
        let engine = DelugeClientEngine::new(factory, false, Duration::ZERO);

        engine
            .run_policy(&make_policy(), &make_host())
            .await
            .unwrap();

        let removed = removed.lock().unwrap().clone();

        assert_eq!(removed.len(), 1, "Should delete exactly one torrent");
        assert_eq!(
            removed[0], "highest_dc",
            "Should delete the torrent with the highest distributed copies first"
        );
    }

    #[tokio::test]
    async fn when_dry_run_then_no_torrents_should_be_deleted() {
        let torrents = vec![
            make_torrent("highest_dc", 5.0),
            make_torrent("mid_dc", 3.0),
            make_torrent("lowest_dc", 1.0),
        ];
        let (factory, removed) = make_factory(torrents);
        let engine = DelugeClientEngine::new(factory, true, Duration::ZERO);

        engine
            .run_policy(&make_policy(), &make_host())
            .await
            .unwrap();

        let removed = removed.lock().unwrap();

        assert!(
            removed.is_empty(),
            "No torrents should be deleted during dry run"
        );
    }

    struct MockService {
        torrents: Vec<TorrentEntry>,
        free_space: i64,
        removed: Arc<Mutex<Vec<String>>>,
        fail_on_hash: Option<String>,
    }

    #[async_trait]
    impl DelugeService for MockService {
        async fn get_torrents(&self) -> anyhow::Result<Vec<TorrentEntry>> {
            Ok(self.torrents.clone())
        }

        async fn get_free_space(&self) -> anyhow::Result<i64> {
            Ok(self.free_space)
        }

        async fn remove_torrent(&self, hash: &str, _remove_data: bool) -> anyhow::Result<()> {
            if self.fail_on_hash.as_deref() == Some(hash) {
                anyhow::bail!("Mock failure for hash {hash}");
            }
            self.removed.lock().unwrap().push(hash.to_owned());
            Ok(())
        }
    }

    #[derive(Clone)]
    struct MockFactory {
        torrents: Vec<TorrentEntry>,
        free_space: i64,
        removed: Arc<Mutex<Vec<String>>>,
        fail_on_hash: Option<String>,
    }

    impl DelugeServiceFactory for MockFactory {
        fn create(
            &self,
            _host: &str,
            _port: u16,
            _username: &str,
            _password: &str,
        ) -> impl DelugeService + Send {
            MockService {
                torrents: self.torrents.clone(),
                free_space: self.free_space,
                removed: self.removed.clone(),
                fail_on_hash: self.fail_on_hash.clone(),
            }
        }
    }

    fn make_torrent(hash: &str, dc: f64) -> TorrentEntry {
        TorrentEntry {
            info_hash: hash.to_owned(),
            name: hash.to_owned(),
            time_added: 900_000,
            ratio: Some(2.0),
            is_finished: true,
            total_seeds: 10,
            total_peers: 5,
            distributed_copies: dc,
            total_wanted: 1024,
        }
    }

    fn make_host() -> HostConfig {
        HostConfig {
            name: "test-host".to_owned(),
            host: "127.0.0.1".to_owned(),
            port: 58846,
            username: "user".to_owned(),
            password: "pass".to_owned(),
        }
    }

    fn make_factory(torrents: Vec<TorrentEntry>) -> (MockFactory, Arc<Mutex<Vec<String>>>) {
        let removed = Arc::new(Mutex::new(Vec::new()));
        let factory = MockFactory {
            torrents,
            free_space: 1_000_000_000,
            removed: removed.clone(),
            fail_on_hash: None,
        };
        (factory, removed)
    }

    fn make_policy() -> Policy {
        Policy {
            name: "test-policy".to_owned(),
            cron: "*/1 * * * *".to_owned(),
            filter: Filter::default(),
            conditions: Condition {
                total_count: Some(3),
                ..Default::default()
            },
        }
    }

    #[tokio::test]
    async fn when_remove_torrent_fails_then_run_policy_should_return_err_and_stop_deleting() {
        let torrents = vec![
            make_torrent("highest_dc", 5.0),
            make_torrent("mid_dc", 3.0),
            make_torrent("lowest_dc", 1.0),
        ];
        let removed = Arc::new(Mutex::new(Vec::new()));
        let factory = MockFactory {
            torrents,
            free_space: 1_000_000_000,
            removed: removed.clone(),
            fail_on_hash: Some("highest_dc".to_owned()),
        };
        let engine = DelugeClientEngine::new(factory, false, Duration::ZERO);

        let policy = Policy {
            name: "test-policy".to_owned(),
            cron: "*/1 * * * *".to_owned(),
            filter: Filter::default(),
            conditions: Condition {
                total_count: Some(2),
                ..Default::default()
            },
        };

        let result = engine.run_policy(&policy, &make_host()).await;

        assert!(
            result.is_err(),
            "Should return Err on first deletion failure"
        );

        let removed = removed.lock().unwrap().clone();
        assert_eq!(
            removed.len(),
            0,
            "No torrent should be successfully deleted, and second deletion should not be attempted"
        );
    }
}
