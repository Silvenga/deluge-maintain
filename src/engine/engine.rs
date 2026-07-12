use crate::config::{HostConfig, Policy};
use crate::engine::plan_deletions::{plan_deletions, DeletionPlan};
use crate::service::{DelugeService, DelugeServiceFactory};
use async_trait::async_trait;
use std::time::{Duration, SystemTime};
use tokio::time::sleep;
use tracing::{error, info, warn};

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

        let torrents = match service.get_torrents().await {
            Ok(t) => t,
            Err(e) => {
                warn!(
                    "Failed to fetch torrents for policy '{}': {:#}. Skipping.",
                    policy.name, e
                );
                return Ok(());
            }
        };

        let free_space = match service.get_free_space().await {
            Ok(s) => s,
            Err(e) => {
                warn!(
                    "Failed to fetch free space for policy '{}': {:#}. Skipping.",
                    policy.name, e
                );
                return Ok(());
            }
        };

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
                        if let Err(e) = service.remove_torrent(&torrent.info_hash, true).await {
                            error!(
                                "Failed to delete torrent '{}' (hash: {}) for policy '{}': {:#}",
                                torrent.name, torrent.info_hash, policy.name, e
                            );
                        }
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
