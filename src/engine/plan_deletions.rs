use crate::config::{ConditionContext, Policy};
use crate::engine::deletion_priority::sort_by_deletion_priority;
use crate::service::TorrentEntry;
use bytesize::ByteSize;
use std::time::SystemTime;
use tracing::{debug, info};

#[derive(Debug)]
pub enum DeletionPlan {
    NothingToDo,
    Deletions(Vec<TorrentEntry>),
    Impossible(Vec<TorrentEntry>),
}

pub fn plan_deletions(
    policy: &Policy,
    torrents: &[TorrentEntry],
    free_space: i64,
    now: SystemTime,
) -> DeletionPlan {
    let used_space: i64 = torrents.iter().map(|t| t.total_wanted).sum();
    let torrent_count = torrents.len();

    let ctx = ConditionContext {
        free_space,
        used_space,
        torrent_count,
    };

    info!(
        "Discovered {} torrents (free_space: {}, used_space: {}).",
        torrents.len(),
        ByteSize::b(free_space as u64),
        ByteSize::b(used_space as u64),
    );

    for torrent in torrents.iter() {
        debug!("Torrent: {:?}", torrent);
    }

    if !policy.conditions.is_met(&ctx) {
        return DeletionPlan::NothingToDo;
    }

    let mut filtered: Vec<TorrentEntry> = torrents
        .iter()
        .filter(|t| policy.filter.matches(t, now))
        .cloned()
        .collect();

    info!(
        "After filtering, {} torrents are eligible for removal.",
        filtered.len(),
    );

    sort_by_deletion_priority(&mut filtered, now);

    let mut to_delete = Vec::new();
    let mut simulated_free_space = free_space;
    let mut simulated_used_space = used_space;
    let mut simulated_count = torrent_count;

    for torrent in filtered.into_iter() {
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

        to_delete.push(torrent);
    }

    info!(
        "Simulated deletion plan: {} torrents will be deleted, freeing {}.",
        to_delete.len(),
        ByteSize::b((simulated_free_space - free_space) as u64),
    );

    if policy.conditions.is_met(&ConditionContext {
        free_space: simulated_free_space,
        used_space: simulated_used_space,
        torrent_count: simulated_count,
    }) {
        return DeletionPlan::Impossible(to_delete);
    }

    DeletionPlan::Deletions(to_delete)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Condition, Filter};
    use std::time::Duration as StdDuration;

    #[test]
    fn when_no_conditions_met_then_should_return_nothing_to_do() {
        let policy = make_policy(Condition {
            total_count: Some(100),
            ..Default::default()
        });
        let torrents = vec![make_torrent(900_000, 0.5, 10, 1024, "a")];
        let now = now();

        let result = plan_deletions(&policy, &torrents, 1_000_000_000, now);

        assert!(matches!(result, DeletionPlan::NothingToDo));
    }

    #[test]
    fn when_condition_met_and_deleting_resolves_then_should_return_deletions() {
        let policy = make_policy(Condition {
            available_space: Some(bytesize::ByteSize::gib(6)),
            ..Default::default()
        });
        let torrents = vec![
            make_torrent(900_000, 1.0, 50, 5_000_000_000, "high_avail"),
            make_torrent(800_000, 1.0, 10, 5_000_000_000, "low_avail"),
        ];
        let now = now();
        let free_space = 5_000_000_000;

        let result = plan_deletions(&policy, &torrents, free_space, now);

        match result {
            DeletionPlan::Deletions(deletions) => {
                assert_eq!(deletions.len(), 1);
                assert_eq!(deletions[0].info_hash, "high_avail");
            }
            other => panic!("expected Deletions, got {other:?}"),
        }
    }

    #[test]
    fn when_conditions_met_but_no_torrents_pass_filter_then_should_return_impossible() {
        let policy = Policy {
            name: "test".to_owned(),
            cron: "*/1 * * * *".to_owned(),
            filter: Filter {
                min_availability: 2.0,
                ..Default::default()
            },
            conditions: Condition {
                total_count: Some(1),
                ..Default::default()
            },
        };
        let torrents = vec![make_torrent(900_000, 0.5, 10, 1024, "a")];
        let now = now();

        let result = plan_deletions(&policy, &torrents, 1_000_000_000, now);

        assert!(matches!(result, DeletionPlan::Impossible(_)));
    }

    #[test]
    fn when_deleting_all_filtered_does_not_resolve_then_should_return_impossible() {
        let policy = make_policy(Condition {
            available_space: Some(bytesize::ByteSize::tb(1)),
            ..Default::default()
        });
        let torrents = vec![
            make_torrent(900_000, 1.0, 10, 1024, "a"),
            make_torrent(800_000, 1.0, 10, 1024, "b"),
        ];
        let now = now();
        let free_space = 0;

        let result = plan_deletions(&policy, &torrents, free_space, now);

        assert!(matches!(result, DeletionPlan::Impossible(_)));
    }

    #[test]
    fn when_used_space_condition_then_should_delete_until_below_threshold() {
        let policy = make_policy(Condition {
            used_space: Some(bytesize::ByteSize::gib(10)),
            ..Default::default()
        });
        let gib = 1_073_741_824i64;
        let torrents = vec![
            make_torrent(900_000, 1.0, 50, gib * 5, "highest_avail"),
            make_torrent(850_000, 1.0, 30, gib * 5, "mid_avail"),
            make_torrent(800_000, 1.0, 10, gib * 5, "lowest_avail"),
        ];
        let now = now();

        let result = plan_deletions(&policy, &torrents, 1_000_000_000, now);

        match result {
            DeletionPlan::Deletions(deletions) => {
                assert_eq!(
                    deletions.len(),
                    2,
                    "Should delete 2 torrents to bring used_space below 10 GiB"
                );
                assert_eq!(deletions[0].info_hash, "highest_avail");
                assert_eq!(deletions[1].info_hash, "mid_avail");
            }
            other => panic!("expected Deletions with 2 items, got {other:?}"),
        }
    }

    #[test]
    fn when_multiple_deletions_needed_then_should_delete_in_priority_order() {
        let policy = make_policy(Condition {
            total_count: Some(2),
            ..Default::default()
        });
        let torrents = vec![
            make_torrent(900_000, 1.0, 50, 1024, "highest_avail"),
            make_torrent(850_000, 1.0, 30, 1024, "mid_avail"),
            make_torrent(800_000, 1.0, 10, 1024, "lowest_avail"),
        ];
        let now = now();

        let result = plan_deletions(&policy, &torrents, 1_000_000_000, now);

        match result {
            DeletionPlan::Deletions(deletions) => {
                assert_eq!(deletions.len(), 2);
                assert_eq!(deletions[0].info_hash, "highest_avail");
                assert_eq!(deletions[1].info_hash, "mid_avail");
            }
            other => panic!("expected Deletions with 2 items, got {other:?}"),
        }
    }

    fn now() -> SystemTime {
        SystemTime::UNIX_EPOCH + StdDuration::from_secs(1_000_000)
    }

    fn make_torrent(
        time_added: i64,
        availability: f64,
        total_seeds: i64,
        wanted: i64,
        hash: &str,
    ) -> TorrentEntry {
        TorrentEntry {
            info_hash: hash.to_owned(),
            name: hash.to_owned(),
            time_added,
            ratio: Some(2.0),
            is_finished: true,
            total_seeds,
            total_peers: 5,
            availability,
            total_wanted: wanted,
        }
    }

    fn make_policy(conditions: Condition) -> Policy {
        Policy {
            name: "test".to_owned(),
            cron: "*/1 * * * *".to_owned(),
            filter: Filter::default(),
            conditions,
        }
    }
}
