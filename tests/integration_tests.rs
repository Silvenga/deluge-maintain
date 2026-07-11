use deluge_maintain::{
    Condition, DeletionResult, DelugeService, Engine, Filter, Policy, TorrentEntry,
};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

struct MockService {
    torrents: Vec<TorrentEntry>,
    free_space: i64,
    deleted: Mutex<Vec<String>>,
}

impl MockService {
    fn new(torrents: Vec<TorrentEntry>, free_space: i64) -> Self {
        Self {
            torrents,
            free_space,
            deleted: Mutex::new(Vec::new()),
        }
    }
}

impl DelugeService for MockService {
    async fn get_torrents(&self) -> anyhow::Result<Vec<TorrentEntry>> {
        Ok(self.torrents.clone())
    }

    async fn get_free_space(&self) -> anyhow::Result<i64> {
        Ok(self.free_space)
    }

    async fn remove_torrent(&self, hash: &str, _remove_data: bool) -> anyhow::Result<()> {
        self.deleted.lock().unwrap().push(hash.to_owned());
        Ok(())
    }
}

fn now() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000)
}

fn make_torrent(time_added: i64, dc: f64, wanted: i64, seeds: i64, hash: &str) -> TorrentEntry {
    TorrentEntry {
        info_hash: hash.to_owned(),
        name: hash.to_owned(),
        time_added,
        ratio: Some(2.0),
        is_finished: true,
        total_seeds: seeds,
        total_peers: 0,
        distributed_copies: dc,
        total_wanted: wanted,
    }
}

fn make_policy(filter: Filter, conditions: Condition) -> Policy {
    Policy {
        name: "integration-test".to_owned(),
        filter,
        conditions,
    }
}

#[tokio::test]
async fn when_space_low_and_torrents_eligible_then_should_delete_in_priority_order() {
    let torrents = vec![
        make_torrent(900_000, 5.0, 5_000_000_000, 10, "highest_dc"),
        make_torrent(850_000, 3.0, 5_000_000_000, 10, "mid_dc"),
        make_torrent(800_000, 1.0, 5_000_000_000, 10, "lowest_dc"),
    ];
    let service = MockService::new(torrents, 5_000_000_000);
    let policy = make_policy(
        Filter::default(),
        Condition {
            available_space: Some(bytesize::ByteSize::gib(6)),
            ..Default::default()
        },
    );
    let engine = Engine::new(Arc::new(service), false, Duration::ZERO);

    engine.run_policy(&policy).await.unwrap();

    let deleted = engine.service().deleted.lock().unwrap().clone();

    assert_eq!(deleted.len(), 1);
    assert_eq!(deleted[0], "highest_dc");
}

#[tokio::test]
async fn when_dry_run_then_should_not_delete() {
    let torrents = vec![make_torrent(900_000, 5.0, 5_000_000_000, 10, "torrent_a")];
    let service = MockService::new(torrents, 0);
    let policy = make_policy(
        Filter::default(),
        Condition {
            available_space: Some(bytesize::ByteSize::gib(1)),
            ..Default::default()
        },
    );
    let engine = Engine::new(Arc::new(service), true, Duration::ZERO);

    engine.run_policy(&policy).await.unwrap();

    let deleted = engine.service().deleted.lock().unwrap().clone();

    assert!(deleted.is_empty());
}

#[tokio::test]
async fn when_filter_excludes_all_then_should_not_delete() {
    let torrents = vec![
        make_torrent(900_000, 5.0, 5_000_000_000, 1, "few_seeds"),
        make_torrent(800_000, 1.0, 5_000_000_000, 1, "also_few_seeds"),
    ];
    let service = MockService::new(torrents, 0);
    let policy = make_policy(
        Filter {
            min_total_seeds: Some(100),
            ..Default::default()
        },
        Condition {
            available_space: Some(bytesize::ByteSize::gib(1)),
            ..Default::default()
        },
    );
    let engine = Engine::new(Arc::new(service), false, Duration::ZERO);

    engine.run_policy(&policy).await.unwrap();

    let deleted = engine.service().deleted.lock().unwrap().clone();

    assert!(deleted.is_empty());
}

#[tokio::test]
async fn when_no_conditions_met_then_should_not_delete() {
    let torrents = vec![
        make_torrent(900_000, 5.0, 5_000_000_000, 10, "torrent_a"),
        make_torrent(800_000, 1.0, 5_000_000_000, 10, "torrent_b"),
    ];
    let service = MockService::new(torrents, 100_000_000_000);
    let policy = make_policy(
        Filter::default(),
        Condition {
            available_space: Some(bytesize::ByteSize::gib(10)),
            ..Default::default()
        },
    );
    let engine = Engine::new(Arc::new(service), false, Duration::ZERO);

    engine.run_policy(&policy).await.unwrap();

    let deleted = engine.service().deleted.lock().unwrap().clone();

    assert!(deleted.is_empty());
}

#[tokio::test]
async fn when_multiple_deletions_needed_then_should_delete_all_in_order() {
    let torrents = vec![
        make_torrent(900_000, 5.0, 1_000_000_000, 10, "highest_dc"),
        make_torrent(850_000, 3.0, 1_000_000_000, 10, "mid_dc"),
        make_torrent(800_000, 1.0, 1_000_000_000, 10, "lowest_dc"),
    ];
    let service = MockService::new(torrents, 0);
    let policy = make_policy(
        Filter::default(),
        Condition {
            available_space: Some(bytesize::ByteSize::gib(1)),
            ..Default::default()
        },
    );
    let engine = Engine::new(Arc::new(service), false, Duration::ZERO);

    engine.run_policy(&policy).await.unwrap();

    let deleted = engine.service().deleted.lock().unwrap().clone();

    assert_eq!(deleted.len(), 2);
    assert_eq!(deleted[0], "highest_dc");
    assert_eq!(deleted[1], "mid_dc");
}

#[test]
fn when_plan_deletions_with_realistic_data_then_should_select_correct_torrents() {
    let torrents = vec![
        make_torrent(900_000, 10.0, 2_000_000_000, 50, "very_healthy"),
        make_torrent(850_000, 5.0, 3_000_000_000, 20, "healthy"),
        make_torrent(800_000, 0.5, 1_000_000_000, 2, "struggling"),
    ];
    let policy = make_policy(
        Filter {
            min_total_seeds: Some(1),
            min_distributed_copies: Some(0.1),
            ..Default::default()
        },
        Condition {
            available_space: Some(bytesize::ByteSize::gib(2)),
            ..Default::default()
        },
    );
    let now = now();
    let free_space = 1_000_000_000;

    let result = Engine::<MockService>::plan_deletions(&policy, &torrents, free_space, now);

    match result {
        DeletionResult::Deletions(deletions) => {
            assert_eq!(deletions.len(), 1);
            assert_eq!(deletions[0].info_hash, "very_healthy");
        }
        other => panic!("expected Deletions, got {other:?}"),
    }
}
