use crate::policy::{Condition, Filter};
use clap::Parser;
use croner::Cron;
use serde::Deserialize;
use std::path::PathBuf;
use std::str::FromStr;

#[derive(Parser, Debug)]
#[command(
    name = "deluge-maintain",
    version,
    about = "A service that puts deluge on autopilot using retention policies",
    author = "Mark Lopez <m@silvenga.com>"
)]
pub struct CliConfig {
    /// Path to the TOML configuration file.
    #[arg(long, env = "DELUGE_MAINTAIN_CONFIG")]
    pub config: PathBuf,

    /// Simulate policy enforcement without making changes.
    #[arg(long, env = "DELUGE_MAINTAIN_DRY_RUN", default_value_t = false)]
    pub dry_run: bool,

    /// Delay between torrent deletions, in seconds.
    #[arg(long, env = "DELUGE_MAINTAIN_DELETE_DELAY", default_value_t = 1)]
    pub delete_delay: u64,
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub hosts: Vec<HostConfig>,
    pub policies: Vec<PolicyConfig>,
}

impl Config {
    pub fn load(toml_str: &str) -> anyhow::Result<Self> {
        let config: Config = toml::from_str(toml_str)
            .map_err(|e| anyhow::anyhow!("failed to parse config file: {e}"))?;

        for policy in &config.policies {
            Cron::from_str(&policy.cron).map_err(|e| {
                anyhow::anyhow!("invalid cron expression for policy '{}': {e}", policy.name)
            })?;
        }

        Ok(config)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct HostConfig {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PolicyConfig {
    pub name: String,
    pub cron: String,
    #[serde(default)]
    pub filter: Filter,
    #[serde(default)]
    pub conditions: Condition,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn when_valid_config_then_should_parse() {
        let toml = r#"
[[hosts]]
name = "test"
host = "127.0.0.1"
port = 58846
username = "localclient"
password = "secret"

[[policies]]
name = "default"
cron = "0 */6 * * *"
"#;

        let config = Config::load(toml).unwrap();

        assert_eq!(config.hosts.len(), 1);
        assert_eq!(config.hosts[0].name, "test");
        assert_eq!(config.policies.len(), 1);
        assert_eq!(config.policies[0].name, "default");
        assert_eq!(config.policies[0].cron, "0 */6 * * *");
    }

    #[test]
    fn when_invalid_cron_then_should_fail() {
        let toml = r#"
[[hosts]]
name = "test"
host = "127.0.0.1"
port = 58846
username = "localclient"
password = "secret"

[[policies]]
name = "default"
cron = "not a cron expression"
"#;

        let result = Config::load(toml);

        assert!(result.is_err());
    }

    #[test]
    fn when_filter_specified_then_should_parse() {
        let toml = r#"
[[hosts]]
name = "test"
host = "127.0.0.1"
port = 58846
username = "localclient"
password = "secret"

[[policies]]
name = "default"
cron = "0 */6 * * *"

[policies.filter]
age = "30d"
ratio = 2.0
min_total_seeds = 3
"#;

        let config = Config::load(toml).unwrap();

        assert_eq!(
            config.policies[0].filter.age.map(|d| d.as_secs()),
            Some(30 * 86_400)
        );
        assert_eq!(config.policies[0].filter.ratio, Some(2.0));
        assert_eq!(config.policies[0].filter.min_total_seeds, Some(3));
    }

    #[test]
    fn when_conditions_specified_then_should_parse() {
        let toml = r#"
[[hosts]]
name = "test"
host = "127.0.0.1"
port = 58846
username = "localclient"
password = "secret"

[[policies]]
name = "default"
cron = "0 */6 * * *"

[policies.conditions]
available_space = "50 GiB"
total_count = 500
"#;

        let config = Config::load(toml).unwrap();

        assert_eq!(
            config.policies[0].conditions.available_space,
            Some(bytesize::ByteSize::gib(50))
        );
        assert_eq!(config.policies[0].conditions.total_count, Some(500));
    }
}
