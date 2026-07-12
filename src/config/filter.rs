use crate::service::TorrentEntry;
use serde::Deserialize;
use std::time::{Duration, SystemTime};
use tracing::warn;

#[derive(Debug, Clone, Deserialize)]
pub struct Filter {
    #[serde(with = "humantime_serde", default)]
    pub age: Option<Duration>,
    #[serde(default)]
    pub ratio: Option<f32>,
    #[serde(default = "default_true")]
    pub completed: bool,
    #[serde(default)]
    pub min_total_seeds: Option<u32>,
    #[serde(default = "default_full_availability")]
    pub min_availability: f32,
}

impl Default for Filter {
    fn default() -> Self {
        Self {
            age: None,
            ratio: None,
            completed: true,
            min_total_seeds: None,
            min_availability: 1.0,
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_full_availability() -> f32 {
    1.0
}

impl Filter {
    pub fn matches(&self, torrent: &TorrentEntry, now: SystemTime) -> bool {
        if let Some(min_age) = self.age {
            if torrent.time_added < 0 {
                warn!(
                    "Torrent {} has invalid negative time_added ({}); \
                     it will not match the age filter.",
                    torrent.info_hash, torrent.time_added
                );
                return false;
            }
            let added = SystemTime::UNIX_EPOCH + Duration::from_secs(torrent.time_added as u64);
            let age = now.duration_since(added).unwrap_or_default();
            if age < min_age {
                return false;
            }
        }

        if let Some(min_ratio) = self.ratio {
            let ratio = torrent.ratio.unwrap_or(f64::INFINITY);
            if ratio < (min_ratio as f64) {
                return false;
            }
        }

        if self.completed && !torrent.is_finished {
            return false;
        }

        if let Some(min_seeds) = self.min_total_seeds {
            if torrent.total_seeds < 0 {
                warn!(
                    "Torrent {} has invalid negative total_seeds ({}); \
                     it will not match the seed filter.",
                    torrent.info_hash, torrent.total_seeds
                );
                return false;
            }
            if torrent.total_seeds < (min_seeds as i64) {
                return false;
            }
        }

        if torrent.availability < (self.min_availability as f64) {
            return false;
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn age_dur(secs: u64) -> Duration {
        Duration::from_secs(secs)
    }

    fn now() -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000)
    }

    fn make_torrent() -> TorrentEntry {
        TorrentEntry {
            info_hash: "abc123".to_owned(),
            name: "test".to_owned(),
            time_added: 900_000,
            ratio: Some(2.0),
            is_finished: true,
            total_seeds: 10,
            total_peers: 5,
            availability: 1.0,
            total_wanted: 1024,
        }
    }

    #[test]
    fn when_no_filters_then_should_match() {
        let filter = Filter::default();
        let torrent = make_torrent();

        assert!(filter.matches(&torrent, now()));
    }

    #[test]
    fn when_age_filter_and_torrent_too_new_then_should_not_match() {
        let filter = Filter {
            age: Some(age_dur(200_000)),
            ..Default::default()
        };
        let torrent = make_torrent();

        assert!(!filter.matches(&torrent, now()));
    }

    #[test]
    fn when_age_filter_and_torrent_old_enough_then_should_match() {
        let filter = Filter {
            age: Some(age_dur(50_000)),
            ..Default::default()
        };
        let torrent = make_torrent();

        assert!(filter.matches(&torrent, now()));
    }

    #[test]
    fn when_age_filter_and_negative_time_added_then_should_not_match() {
        let filter = Filter {
            age: Some(age_dur(50_000)),
            ..Default::default()
        };
        let mut torrent = make_torrent();
        torrent.time_added = -1;

        assert!(!filter.matches(&torrent, now()));
    }

    #[test]
    fn when_ratio_filter_and_ratio_below_threshold_then_should_not_match() {
        let filter = Filter {
            ratio: Some(3.0),
            ..Default::default()
        };
        let torrent = make_torrent();

        assert!(!filter.matches(&torrent, now()));
    }

    #[test]
    fn when_ratio_filter_and_ratio_above_threshold_then_should_match() {
        let filter = Filter {
            ratio: Some(1.5),
            ..Default::default()
        };
        let torrent = make_torrent();

        assert!(filter.matches(&torrent, now()));
    }

    #[test]
    fn when_completed_filter_and_torrent_not_finished_then_should_not_match() {
        let filter = Filter {
            completed: true,
            ..Default::default()
        };
        let mut torrent = make_torrent();
        torrent.is_finished = false;

        assert!(!filter.matches(&torrent, now()));
    }

    #[test]
    fn when_completed_filter_false_and_torrent_not_finished_then_should_match() {
        let filter = Filter {
            completed: false,
            ..Default::default()
        };
        let mut torrent = make_torrent();
        torrent.is_finished = false;

        assert!(filter.matches(&torrent, now()));
    }

    #[test]
    fn when_min_total_seeds_filter_and_seeds_below_threshold_then_should_not_match() {
        let filter = Filter {
            min_total_seeds: Some(20),
            ..Default::default()
        };
        let torrent = make_torrent();

        assert!(!filter.matches(&torrent, now()));
    }

    #[test]
    fn when_min_total_seeds_filter_and_negative_seeds_then_should_not_match() {
        let filter = Filter {
            min_total_seeds: Some(20),
            ..Default::default()
        };
        let mut torrent = make_torrent();
        torrent.total_seeds = -1;

        assert!(!filter.matches(&torrent, now()));
    }

    #[test]
    fn when_min_availability_filter_and_below_threshold_then_should_not_match() {
        let filter = Filter {
            min_availability: 0.75,
            ..Default::default()
        };
        let mut torrent = make_torrent();
        torrent.availability = 0.5;

        assert!(!filter.matches(&torrent, now()));
    }

    #[test]
    fn when_multiple_filters_and_all_match_then_should_match() {
        let filter = Filter {
            age: Some(age_dur(50_000)),
            ratio: Some(1.5),
            completed: true,
            min_total_seeds: Some(5),
            min_availability: 0.5,
        };
        let torrent = make_torrent();

        assert!(filter.matches(&torrent, now()));
    }

    #[test]
    fn when_multiple_filters_and_one_fails_then_should_not_match() {
        let filter = Filter {
            age: Some(age_dur(50_000)),
            ratio: Some(3.0),
            completed: true,
            min_total_seeds: Some(5),
            min_availability: 0.5,
        };
        let torrent = make_torrent();

        assert!(!filter.matches(&torrent, now()));
    }

    #[test]
    fn when_ratio_is_none_then_should_treat_as_infinity_and_pass() {
        let filter = Filter {
            ratio: Some(100.0),
            ..Default::default()
        };
        let mut torrent = make_torrent();
        torrent.ratio = None;

        assert!(filter.matches(&torrent, now()));
    }
}
