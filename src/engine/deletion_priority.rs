use crate::service::TorrentEntry;
use std::cmp::Ordering;
use std::time::{Duration, SystemTime};

pub fn sort_by_deletion_priority(torrents: &mut [TorrentEntry], now: SystemTime) {
    torrents.sort_by(|a, b| {
        let avail_cmp = b
            .availability
            .partial_cmp(&a.availability)
            .unwrap_or(Ordering::Equal);

        if avail_cmp != Ordering::Equal {
            return avail_cmp;
        }

        let seeds_cmp = b.total_seeds.cmp(&a.total_seeds);

        if seeds_cmp != Ordering::Equal {
            return seeds_cmp;
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
    fn when_higher_availability_then_should_be_sorted_first() {
        let now = now();
        let mut torrents = vec![
            make_torrent(900_000, 0.1, 10, 1024, "low_avail"),
            make_torrent(900_000, 1.0, 10, 1024, "high_avail"),
        ];

        sort_by_deletion_priority(&mut torrents, now);

        assert_eq!(torrents[0].info_hash, "high_avail");
        assert_eq!(torrents[1].info_hash, "low_avail");
    }

    #[test]
    fn when_equal_availability_then_higher_seeds_should_be_sorted_first() {
        let now = now();
        let mut torrents = vec![
            make_torrent(950_000, 0.5, 5, 1024, "fewer_seeds"),
            make_torrent(900_000, 0.5, 50, 1024, "more_seeds"),
        ];

        sort_by_deletion_priority(&mut torrents, now);

        assert_eq!(torrents[0].info_hash, "more_seeds");
        assert_eq!(torrents[1].info_hash, "fewer_seeds");
    }

    #[test]
    fn when_equal_availability_and_seeds_then_oldest_should_be_sorted_first() {
        let now = now();
        let mut torrents = vec![
            make_torrent(950_000, 0.5, 10, 1024, "newer"),
            make_torrent(800_000, 0.5, 10, 1024, "older"),
        ];

        sort_by_deletion_priority(&mut torrents, now);

        assert_eq!(torrents[0].info_hash, "older");
        assert_eq!(torrents[1].info_hash, "newer");
    }

    #[test]
    fn when_mixed_availability_and_seeds_then_availability_should_take_priority() {
        let now = now();
        let mut torrents = vec![
            make_torrent(800_000, 0.1, 100, 1024, "old_low_avail_many_seeds"),
            make_torrent(950_000, 1.0, 5, 1024, "new_high_avail_few_seeds"),
        ];

        sort_by_deletion_priority(&mut torrents, now);

        assert_eq!(torrents[0].info_hash, "new_high_avail_few_seeds");
        assert_eq!(torrents[1].info_hash, "old_low_avail_many_seeds");
    }

    #[test]
    fn when_single_torrent_then_should_remain() {
        let now = now();
        let mut torrents = vec![make_torrent(900_000, 0.5, 10, 1024, "only")];

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
}
