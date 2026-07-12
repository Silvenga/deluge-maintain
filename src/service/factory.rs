use crate::service::{DelugeClientService, DelugeService};

pub trait DelugeServiceFactory: Send + Sync {
    fn create(
        &self,
        host: &str,
        port: u16,
        username: &str,
        password: &str,
    ) -> impl DelugeService + Send;
}

#[derive(Default)]
pub struct DelugeClientServiceFactory;

impl DelugeServiceFactory for DelugeClientServiceFactory {
    fn create(
        &self,
        host: &str,
        port: u16,
        username: &str,
        password: &str,
    ) -> impl DelugeService + Send {
        DelugeClientService::new(host, port, username, password)
    }
}
