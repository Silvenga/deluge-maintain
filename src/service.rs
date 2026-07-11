use anyhow::Result;
use deluge_rpc_client::models::{FilterDict, TorrentEntry as RpcTorrentEntry};
use deluge_rpc_client::{DelugeClient, DelugeClientBuilder};

const TORRENT_FIELDS: &[&str] = &[
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

#[expect(
    async_fn_in_trait,
    reason = "using native async traits with generics, not trait objects"
)]
pub trait DelugeService {
    async fn get_torrents(&self) -> Result<Vec<TorrentEntry>>;
    async fn get_free_space(&self) -> Result<i64>;
    async fn remove_torrent(&self, hash: &str, remove_data: bool) -> Result<()>;
}

pub struct DelugeClientService {
    client: DelugeClient,
}

impl DelugeClientService {
    pub fn new(host: &str, port: u16, username: &str, password: &str) -> Self {
        let client = DelugeClientBuilder::new(host, port, username, password).build();
        Self { client }
    }
}

impl DelugeService for DelugeClientService {
    async fn get_torrents(&self) -> Result<Vec<TorrentEntry>> {
        let keys: Vec<String> = TORRENT_FIELDS.iter().map(|&s| s.to_owned()).collect();
        let entries = self
            .client
            .core
            .torrents
            .get_torrents_status(&FilterDict::default(), &keys, false)
            .await
            .map_err(|e| anyhow::anyhow!("failed to get torrents: {e}"))?;

        Ok(entries.into_iter().map(TorrentEntry::from).collect())
    }

    async fn get_free_space(&self) -> Result<i64> {
        self.client
            .core
            .session
            .get_free_space(None)
            .await
            .map_err(|e| anyhow::anyhow!("failed to get free space: {e}"))
    }

    async fn remove_torrent(&self, hash: &str, remove_data: bool) -> Result<()> {
        self.client
            .core
            .torrents
            .remove_torrent(hash, remove_data)
            .await
            .map_err(|e| anyhow::anyhow!("failed to remove torrent {hash}: {e}"))?;
        Ok(())
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
            distributed_copies: entry.status.distributed_copies,
            total_wanted: entry.status.total_wanted,
        }
    }
}
