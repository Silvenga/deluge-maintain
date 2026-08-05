use crate::config::HostConfig;
use crate::service::{DelugeClientService, DelugeService};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub trait DelugeServiceRegistry: Send + Sync {
    fn get(&self, host: &HostConfig) -> anyhow::Result<Arc<dyn DelugeService>>;
}

#[derive(Default)]
pub struct DelugeClientServiceRegistry {
    services: Mutex<HashMap<HostConfig, Arc<dyn DelugeService>>>,
}

impl DelugeClientServiceRegistry {
    pub fn new() -> Self {
        Self::default()
    }
}

impl DelugeServiceRegistry for DelugeClientServiceRegistry {
    fn get(&self, host: &HostConfig) -> anyhow::Result<Arc<dyn DelugeService>> {
        let mut services = self
            .services
            .lock()
            .map_err(|e| anyhow::anyhow!("Registry lock poisoned: {e}"))?;

        let service = services.entry(host.clone()).or_insert_with(|| {
            Arc::from(DelugeClientService::new(
                &host.host,
                host.port,
                &host.username,
                &host.password,
            ))
        });

        Ok(service.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_host(name: &str) -> HostConfig {
        HostConfig {
            name: name.to_owned(),
            host: "127.0.0.1".to_owned(),
            port: 58846,
            username: "user".to_owned(),
            password: "pass".to_owned(),
        }
    }

    #[test]
    fn when_get_called_twice_for_same_host_then_should_return_same_service() {
        let registry = DelugeClientServiceRegistry::new();
        let host = make_host("test-host");

        let first = registry.get(&host).unwrap();
        let second = registry.get(&host).unwrap();

        assert!(
            Arc::ptr_eq(&first, &second),
            "Same host should return the same cached service instance"
        );
    }

    #[test]
    fn when_get_called_for_different_hosts_then_should_return_different_services() {
        let registry = DelugeClientServiceRegistry::new();

        let first = registry.get(&make_host("host-a")).unwrap();
        let second = registry.get(&make_host("host-b")).unwrap();

        assert!(
            !Arc::ptr_eq(&first, &second),
            "Different hosts should return distinct service instances"
        );
    }
}
