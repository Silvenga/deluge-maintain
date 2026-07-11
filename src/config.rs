use crate::policy::{Condition, Filter};
use clap::Parser;
use croner::Cron;
use serde::Deserialize;
use std::fmt;
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

        if config.hosts.is_empty() {
            anyhow::bail!("config must contain at least one host");
        }

        for host in &config.hosts {
            if host.name.is_empty() {
                anyhow::bail!("host has an empty name");
            }
            if host.host.is_empty() {
                anyhow::bail!("host '{}' has an empty host address", host.name);
            }
            if host.port == 0 {
                anyhow::bail!("host '{}' has port 0", host.name);
            }
        }

        for policy in &config.policies {
            if policy.name.is_empty() {
                anyhow::bail!("policy has an empty name");
            }
            Cron::from_str(&policy.cron).map_err(|e| {
                anyhow::anyhow!("invalid cron expression for policy '{}': {e}", policy.name)
            })?;
        }

        Ok(config)
    }
}

#[derive(Deserialize, Clone)]
pub struct HostConfig {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
}

impl fmt::Debug for HostConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HostConfig")
            .field("name", &self.name)
            .field("host", &self.host)
            .field("port", &self.port)
            .field("username", &self.username)
            .field("password", &"<redacted>")
            .finish()
    }
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

    #[test]
    fn when_no_hosts_then_should_fail() {
        let toml = r#"
[[policies]]
name = "default"
cron = "0 */6 * * *"
"#;

        let result = Config::load(toml);

        assert!(result.is_err());
    }

    #[test]
    fn when_empty_host_address_then_should_fail() {
        let toml = r#"
[[hosts]]
name = "test"
host = ""
port = 58846
username = "localclient"
password = "secret"

[[policies]]
name = "default"
cron = "0 */6 * * *"
"#;

        let result = Config::load(toml);

        assert!(result.is_err());
    }

    #[test]
    fn when_port_zero_then_should_fail() {
        let toml = r#"
[[hosts]]
name = "test"
host = "127.0.0.1"
port = 0
username = "localclient"
password = "secret"

[[policies]]
name = "default"
cron = "0 */6 * * *"
"#;

        let result = Config::load(toml);

        assert!(result.is_err());
    }

    #[test]
    fn when_empty_policy_name_then_should_fail() {
        let toml = r#"
[[hosts]]
name = "test"
host = "127.0.0.1"
port = 58846
username = "localclient"
password = "secret"

[[policies]]
name = ""
cron = "0 */6 * * *"
"#;

        let result = Config::load(toml);

        assert!(result.is_err());
    }

    #[test]
    fn when_host_config_debug_then_password_should_be_redacted() {
        let host = HostConfig {
            name: "test".to_owned(),
            host: "127.0.0.1".to_owned(),
            port: 58846,
            username: "user".to_owned(),
            password: "secret".to_owned(),
        };

        let debug_output = format!("{host:?}");

        assert!(!debug_output.contains("secret"));
        assert!(debug_output.contains("<redacted>"));
    }
}
