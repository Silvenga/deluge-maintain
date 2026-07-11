use crate::policy::Policy;
use crate::policy::condition::ConditionContext;
use crate::service::{DelugeService, TorrentEntry};
use anyhow::Result;
use std::cmp::Ordering;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::time::sleep;

pub struct Engine<S: DelugeService> {
    service: Arc<S>,
    dry_run: bool,
    delete_delay: Duration,
}

#[derive(Debug)]
pub enum DeletionResult {
    NothingToDo,
    Deletions(Vec<TorrentEntry>),
    Impossible,
}

pub fn sort_by_deletion_priority(torrents: &mut [TorrentEntry], now: SystemTime) {
    torrents.sort_by(|a, b| {
        let dc_cmp = b
            .distributed_copies
            .partial_cmp(&a.distributed_copies)
            .unwrap_or(Ordering::Equal);

        if dc_cmp != Ordering::Equal {
            return dc_cmp;
        }

        let age_a = now
            .duration_since(SystemTime::UNIX_EPOCH + Duration::from_secs(a.time_added as u64))
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let age_b = now
            .duration_since(SystemTime::UNIX_EPOCH + Duration::from_secs(b.time_added as u64))
            .map(|d| d.as_secs())
            .unwrap_or(0);

        age_b.cmp(&age_a)
    });
}

impl<S: DelugeService> Engine<S> {
    pub fn new(service: Arc<S>, dry_run: bool, delete_delay: Duration) -> Self {
        Self {
            service,
            dry_run,
            delete_delay,
        }
    }

    pub fn service(&self) -> &S {
        &self.service
    }

    pub fn plan_deletions(
        policy: &Policy,
        torrents: &[TorrentEntry],
        free_space: i64,
        now: SystemTime,
    ) -> DeletionResult {
        let used_space: i64 = torrents.iter().map(|t| t.total_wanted).sum();
        let torrent_count = torrents.len();

        let ctx = ConditionContext {
            free_space,
            used_space,
            torrent_count,
        };

        if !policy.conditions.is_met(&ctx) {
            return DeletionResult::NothingToDo;
        }

        let mut filtered: Vec<TorrentEntry> = torrents
            .iter()
            .filter(|t| policy.filter.matches(t, now))
            .cloned()
            .collect();

        if filtered.is_empty() {
            tracing::warn!(
                policy = %policy.name,
                "conditions are met but no torrents pass the filter"
            );
            return DeletionResult::Impossible;
        }

        sort_by_deletion_priority(&mut filtered, now);

        let mut to_delete = Vec::new();
        let mut simulated_free_space = free_space;
        let mut simulated_used_space = used_space;
        let mut simulated_count = torrent_count;

        for torrent in &filtered {
            let ctx = ConditionContext {
                free_space: simulated_free_space,
                used_space: simulated_used_space,
                torrent_count: simulated_count,
            };

            if !policy.conditions.is_met(&ctx) {
                break;
            }

            simulated_free_space += torrent.total_wanted;
            simulated_used_space -= torrent.total_wanted;
            simulated_count -= 1;

            to_delete.push(torrent.clone());
        }

        if to_delete.is_empty() {
            tracing::warn!(
                policy = %policy.name,
                "conditions are met but no torrents were selected for deletion"
            );
            return DeletionResult::Impossible;
        }

        let final_ctx = ConditionContext {
            free_space: simulated_free_space,
            used_space: simulated_used_space,
            torrent_count: simulated_count,
        };

        if policy.conditions.is_met(&final_ctx) {
            tracing::warn!(
                policy = %policy.name,
                "conditions cannot be satisfied even after deleting all filtered torrents"
            );
            return DeletionResult::Impossible;
        }

        DeletionResult::Deletions(to_delete)
    }

    pub async fn run_policy(&self, policy: &Policy) -> Result<()> {
        let now = SystemTime::now();

        let torrents = match self.service.get_torrents().await {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!(policy = %policy.name, error = %e, "failed to fetch torrents, skipping");
                return Ok(());
            }
        };

        let free_space = match self.service.get_free_space().await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(policy = %policy.name, error = %e, "failed to fetch free space, skipping");
                return Ok(());
            }
        };

        let plan = Self::plan_deletions(policy, &torrents, free_space, now);

        match plan {
            DeletionResult::NothingToDo => {
                tracing::info!(policy = %policy.name, "no conditions met, nothing to do");
            }
            DeletionResult::Impossible => {
                tracing::warn!(policy = %policy.name, "conditions cannot be satisfied");
            }
            DeletionResult::Deletions(to_delete) => {
                tracing::info!(
                    policy = %policy.name,
                    count = to_delete.len(),
                    dry_run = self.dry_run,
                    "planned deletions"
                );

                for (i, torrent) in to_delete.iter().enumerate() {
                    tracing::info!(
                        policy = %policy.name,
                        hash = %torrent.info_hash,
                        name = %torrent.name,
                        distributed_copies = torrent.distributed_copies,
                        total_wanted = torrent.total_wanted,
                        dry_run = self.dry_run,
                        "deleting torrent"
                    );

                    if !self.dry_run {
                        if let Err(e) = self.service.remove_torrent(&torrent.info_hash, true).await
                        {
                            tracing::error!(
                                policy = %policy.name,
                                hash = %torrent.info_hash,
                                error = %e,
                                "failed to delete torrent"
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::{Condition, Filter};
    use crate::service::DelugeClientService;
    use std::time::Duration as StdDuration;

    fn now() -> SystemTime {
        SystemTime::UNIX_EPOCH + StdDuration::from_secs(1_000_000)
    }

    fn make_torrent(time_added: i64, dc: f64, wanted: i64, hash: &str) -> TorrentEntry {
        TorrentEntry {
            info_hash: hash.to_owned(),
            name: hash.to_owned(),
            time_added,
            ratio: Some(2.0),
            is_finished: true,
            total_seeds: 10,
            total_peers: 5,
            distributed_copies: dc,
            total_wanted: wanted,
        }
    }

    fn make_policy(conditions: Condition) -> Policy {
        Policy {
            name: "test".to_owned(),
            filter: Filter::default(),
            conditions,
        }
    }

    #[test]
    fn when_no_conditions_met_then_should_return_nothing_to_do() {
        let policy = make_policy(Condition {
            total_count: Some(100),
            ..Default::default()
        });
        let torrents = vec![make_torrent(900_000, 2.0, 1024, "a")];
        let now = now();

        let result =
            Engine::<DelugeClientService>::plan_deletions(&policy, &torrents, 1_000_000_000, now);

        assert!(matches!(result, DeletionResult::NothingToDo));
    }

    #[test]
    fn when_condition_met_and_deleting_resolves_then_should_return_deletions() {
        let policy = make_policy(Condition {
            available_space: Some(bytesize::ByteSize::gib(6)),
            ..Default::default()
        });
        let torrents = vec![
            make_torrent(900_000, 5.0, 5_000_000_000, "high_dc"),
            make_torrent(800_000, 1.0, 5_000_000_000, "low_dc"),
        ];
        let now = now();
        let free_space = 5_000_000_000;

        let result =
            Engine::<DelugeClientService>::plan_deletions(&policy, &torrents, free_space, now);

        match result {
            DeletionResult::Deletions(deletions) => {
                assert_eq!(deletions.len(), 1);
                assert_eq!(deletions[0].info_hash, "high_dc");
            }
            other => panic!("expected Deletions, got {other:?}"),
        }
    }

    #[test]
    fn when_conditions_met_but_no_torrents_pass_filter_then_should_return_impossible() {
        let policy = Policy {
            name: "test".to_owned(),
            filter: Filter {
                min_distributed_copies: Some(100.0),
                ..Default::default()
            },
            conditions: Condition {
                total_count: Some(1),
                ..Default::default()
            },
        };
        let torrents = vec![make_torrent(900_000, 2.0, 1024, "a")];
        let now = now();

        let result =
            Engine::<DelugeClientService>::plan_deletions(&policy, &torrents, 1_000_000_000, now);

        assert!(matches!(result, DeletionResult::Impossible));
    }

    #[test]
    fn when_deleting_all_filtered_does_not_resolve_then_should_return_impossible() {
        let policy = make_policy(Condition {
            available_space: Some(bytesize::ByteSize::tb(1)),
            ..Default::default()
        });
        let torrents = vec![
            make_torrent(900_000, 5.0, 1024, "a"),
            make_torrent(800_000, 1.0, 1024, "b"),
        ];
        let now = now();
        let free_space = 0;

        let result =
            Engine::<DelugeClientService>::plan_deletions(&policy, &torrents, free_space, now);

        assert!(matches!(result, DeletionResult::Impossible));
    }

    #[test]
    fn when_total_count_condition_then_should_delete_until_below_threshold() {
        let policy = make_policy(Condition {
            total_count: Some(3),
            ..Default::default()
        });
        let torrents = vec![
            make_torrent(900_000, 5.0, 1024, "highest_dc"),
            make_torrent(850_000, 3.0, 1024, "mid_dc"),
            make_torrent(800_000, 1.0, 1024, "lowest_dc"),
        ];
        let now = now();

        let result =
            Engine::<DelugeClientService>::plan_deletions(&policy, &torrents, 1_000_000_000, now);

        match result {
            DeletionResult::Deletions(deletions) => {
                assert_eq!(deletions.len(), 1);
                assert_eq!(deletions[0].info_hash, "highest_dc");
            }
            other => panic!("expected Deletions with 1 item, got {other:?}"),
        }
    }

    #[test]
    fn when_multiple_deletions_needed_then_should_delete_in_priority_order() {
        let policy = make_policy(Condition {
            total_count: Some(2),
            ..Default::default()
        });
        let torrents = vec![
            make_torrent(900_000, 5.0, 1024, "highest_dc"),
            make_torrent(850_000, 3.0, 1024, "mid_dc"),
            make_torrent(800_000, 1.0, 1024, "lowest_dc"),
        ];
        let now = now();

        let result =
            Engine::<DelugeClientService>::plan_deletions(&policy, &torrents, 1_000_000_000, now);

        match result {
            DeletionResult::Deletions(deletions) => {
                assert_eq!(deletions.len(), 2);
                assert_eq!(deletions[0].info_hash, "highest_dc");
                assert_eq!(deletions[1].info_hash, "mid_dc");
            }
            other => panic!("expected Deletions with 2 items, got {other:?}"),
        }
    }

    #[test]
    fn when_higher_dc_then_should_be_sorted_first() {
        let now = now();
        let mut torrents = vec![
            make_torrent(900_000, 1.0, 1024, "low_dc"),
            make_torrent(900_000, 5.0, 1024, "high_dc"),
        ];

        sort_by_deletion_priority(&mut torrents, now);

        assert_eq!(torrents[0].info_hash, "high_dc");
        assert_eq!(torrents[1].info_hash, "low_dc");
    }

    #[test]
    fn when_equal_dc_then_oldest_should_be_sorted_first() {
        let now = now();
        let mut torrents = vec![
            make_torrent(950_000, 3.0, 1024, "newer"),
            make_torrent(800_000, 3.0, 1024, "older"),
        ];

        sort_by_deletion_priority(&mut torrents, now);

        assert_eq!(torrents[0].info_hash, "older");
        assert_eq!(torrents[1].info_hash, "newer");
    }

    #[test]
    fn when_mixed_dc_and_age_then_dc_should_take_priority() {
        let now = now();
        let mut torrents = vec![
            make_torrent(800_000, 1.0, 1024, "old_low_dc"),
            make_torrent(950_000, 10.0, 1024, "new_high_dc"),
        ];

        sort_by_deletion_priority(&mut torrents, now);

        assert_eq!(torrents[0].info_hash, "new_high_dc");
        assert_eq!(torrents[1].info_hash, "old_low_dc");
    }

    #[test]
    fn when_single_torrent_then_should_remain() {
        let now = now();
        let mut torrents = vec![make_torrent(900_000, 2.0, 1024, "only")];

        sort_by_deletion_priority(&mut torrents, now);

        assert_eq!(torrents.len(), 1);
        assert_eq!(torrents[0].info_hash, "only");
    }

    #[test]
    fn when_empty_list_then_should_remain_empty() {
        let now = now();
        let mut torrents: Vec<TorrentEntry> = vec![];

        sort_by_deletion_priority(&mut torrents, now);

        assert!(torrents.is_empty());
    }
}
