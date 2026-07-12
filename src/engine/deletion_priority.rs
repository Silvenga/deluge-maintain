use crate::service::TorrentEntry;
use std::cmp::Ordering;
use std::time::{Duration, SystemTime};

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration as StdDuration;

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
}
