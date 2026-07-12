use deluge_rpc_client::models::TorrentEntry as RpcTorrentEntry;

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
    pub distributed_copies: f64,
    pub total_wanted: i64,
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
            distributed_copies: entry.status.distributed_copies,
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
                distributed_copies: 2.5,
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
        assert!((entry.distributed_copies - 2.5).abs() < f64::EPSILON);
        assert_eq!(entry.total_wanted, 1_073_741_824);
    }
}
