use crate::config::{HostConfig, Policy};
use anyhow::{bail, Context};
use croner::Cron;
use serde::Deserialize;
use std::str::FromStr;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub hosts: Vec<HostConfig>,
    pub policies: Vec<Policy>,
}

impl Config {
    pub fn load(toml_str: &str) -> anyhow::Result<Self> {
        let config: Config = toml::from_str(toml_str).context("Failed to parse config file.")?;

        if config.hosts.is_empty() {
            bail!("Config must contain at least one host.");
        }

        for host in &config.hosts {
            if host.name.is_empty() {
                bail!("Host has an empty name.");
            }
            if host.host.is_empty() {
                bail!("Host '{}' has an empty host address.", host.name);
            }
            if host.port == 0 {
                bail!("Host '{}' has port 0.", host.name);
            }
        }

        for policy in &config.policies {
            if policy.name.is_empty() {
                bail!("Policy has an empty name.");
            }
            Cron::from_str(&policy.cron)
                .with_context(|| format!("Invalid cron expression for policy '{}'", policy.name))?;
        }

        Ok(config)
    }
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
min_availability = 0.5
"#;

        let config = Config::load(toml).unwrap();

        assert_eq!(
            config.policies[0].filter.age.map(|d| d.as_secs()),
            Some(30 * 86_400)
        );
        assert_eq!(config.policies[0].filter.ratio, Some(2.0));
        assert_eq!(config.policies[0].filter.min_total_seeds, Some(3));
        assert!((config.policies[0].filter.min_availability - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn when_filter_not_specified_then_min_availability_should_default_to_1() {
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

        assert!(
            (config.policies[0].filter.min_availability - 1.0).abs() < f32::EPSILON,
            "min_availability should default to 1.0 when not specified"
        );
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
used_space = "800 GiB"
total_count = 500
"#;

        let config = Config::load(toml).unwrap();

        assert_eq!(
            config.policies[0].conditions.available_space,
            Some(bytesize::ByteSize::gib(50))
        );
        assert_eq!(
            config.policies[0].conditions.used_space,
            Some(bytesize::ByteSize::gib(800))
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
}
