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
