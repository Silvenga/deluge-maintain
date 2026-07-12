use crate::service::torrent_entry::{TORRENT_FIELDS, TorrentEntry};
use anyhow::Context;
use async_trait::async_trait;
use deluge_rpc_client::models::FilterDict;
use deluge_rpc_client::{DelugeClient, DelugeClientBuilder};

#[async_trait]
pub trait DelugeService {
    async fn get_torrents(&self) -> anyhow::Result<Vec<TorrentEntry>>;
    async fn get_free_space(&self) -> anyhow::Result<i64>;
    async fn remove_torrent(&self, hash: &str, remove_data: bool) -> anyhow::Result<()>;
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

#[async_trait]
impl DelugeService for DelugeClientService {
    async fn get_torrents(&self) -> anyhow::Result<Vec<TorrentEntry>> {
        let keys: Vec<String> = TORRENT_FIELDS.iter().map(|&s| s.to_owned()).collect();
        let entries = self
            .client
            .core
            .torrents
            .get_torrents_status(&FilterDict::default(), &keys, false)
            .await
            .context("Failed to get torrents.")?;

        Ok(entries.into_iter().map(TorrentEntry::from).collect())
    }

    async fn get_free_space(&self) -> anyhow::Result<i64> {
        self.client
            .core
            .session
            .get_free_space(None)
            .await
            .context("Failed to get free space.")
    }

    async fn remove_torrent(&self, hash: &str, remove_data: bool) -> anyhow::Result<()> {
        self.client
            .core
            .torrents
            .remove_torrent(hash, remove_data)
            .await
            .with_context(|| format!("Failed to remove torrent {hash}."))?;
        Ok(())
    }
}
