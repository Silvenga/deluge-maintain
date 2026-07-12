use bytesize::ByteSize;
use deluge_rpc_client::models::TorrentEntry as RpcTorrentEntry;
use std::fmt;
use std::time::{Duration, SystemTime};

pub const TORRENT_FIELDS: &[&str] = &[
    "name",
    "hash",
    "time_added",
    "ratio",
    "is_finished",
    "total_seeds",
    "total_peers",
    "distributed_copies",
    "total_wanted",
];

#[derive(Debug, Clone, PartialEq)]
pub struct TorrentEntry {
    pub info_hash: String,
    pub name: String,
    pub time_added: i64,
    pub ratio: Option<f64>,
    pub is_finished: bool,
    pub total_seeds: i64,
    pub total_peers: i64,
    pub availability: f64,
    pub total_wanted: i64,
}

impl fmt::Display for TorrentEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let added_date = SystemTime::UNIX_EPOCH + Duration::from_secs(self.time_added as u64);
        let added_str = humantime::format_rfc3339(added_date);

        write!(
            f,
            "{} (hash={}, added={}, ratio=",
            self.name, self.info_hash, added_str,
        )?;

        match self.ratio {
            Some(r) => write!(f, "{r:.2}, ")?,
            None => write!(f, "inf, ")?,
        }

        write!(
            f,
            "finished={}, seeds={}, peers={}, availability={:.2}, size={})",
            self.is_finished,
            self.total_seeds,
            self.total_peers,
            self.availability,
            ByteSize(self.total_wanted as u64),
        )
    }
}

impl From<RpcTorrentEntry> for TorrentEntry {
    fn from(entry: RpcTorrentEntry) -> Self {
        TorrentEntry {
            info_hash: entry.info_hash,
            name: entry.status.name,
            time_added: entry.status.time_added,
            ratio: entry.status.ratio,
            is_finished: entry.status.is_finished,
            total_seeds: entry.status.total_seeds,
            total_peers: entry.status.total_peers,
            availability: entry.status.distributed_copies,
            total_wanted: entry.status.total_wanted,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use deluge_rpc_client::models::TorrentEntry as RpcTorrentEntry;
    use deluge_rpc_client::models::TorrentStatus;

    #[test]
    fn when_converting_rpc_entry_then_all_fields_should_map_correctly() {
        let rpc_entry = RpcTorrentEntry {
            info_hash: "abc123hash".to_owned(),
            status: TorrentStatus {
                name: "test-torrent".to_owned(),
                time_added: 1_699_000_000,
                ratio: Some(2.5),
                is_finished: true,
                total_seeds: 50,
                total_peers: 20,
                distributed_copies: 0.5,
                total_wanted: 1_073_741_824,
                ..Default::default()
            },
        };

        let entry = TorrentEntry::from(rpc_entry);

        assert_eq!(entry.info_hash, "abc123hash");
        assert_eq!(entry.name, "test-torrent");
        assert_eq!(entry.time_added, 1_699_000_000);
        assert_eq!(entry.ratio, Some(2.5));
        assert!(entry.is_finished);
        assert_eq!(entry.total_seeds, 50);
        assert_eq!(entry.total_peers, 20);
        assert!((entry.availability - 0.5).abs() < f64::EPSILON);
        assert_eq!(entry.total_wanted, 1_073_741_824);
    }

    #[test]
    fn when_displaying_entry_then_should_format_all_fields() {
        let entry = TorrentEntry {
            info_hash: "abc123hash".to_owned(),
            name: "test-torrent".to_owned(),
            time_added: 1_699_000_000,
            ratio: Some(2.5),
            is_finished: true,
            total_seeds: 50,
            total_peers: 20,
            availability: 0.5,
            total_wanted: 1_073_741_824,
        };

        let output = format!("{entry}");

        assert!(output.contains("test-torrent"));
        assert!(output.contains("abc123hash"));
        assert!(output.contains("2023-11-03T"));
        assert!(output.contains("ratio=2.50"));
        assert!(output.contains("finished=true"));
        assert!(output.contains("seeds=50"));
        assert!(output.contains("peers=20"));
        assert!(output.contains("availability=0.50"));
        assert!(output.contains("size=1.0 GiB"));
    }

    #[test]
    fn when_ratio_is_none_then_display_should_show_inf() {
        let entry = TorrentEntry {
            info_hash: "abc".to_owned(),
            name: "test".to_owned(),
            time_added: 0,
            ratio: None,
            is_finished: false,
            total_seeds: 0,
            total_peers: 0,
            availability: 0.0,
            total_wanted: 0,
        };

        let output = format!("{entry}");

        assert!(output.contains("ratio=inf"));
    }
}
