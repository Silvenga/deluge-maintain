use deluge_maintain::Config;
use std::fs;

#[test]
fn when_reference_config_parsed_then_should_succeed() {
    let contents = fs::read_to_string("deluge-maintain.toml")
        .expect("reference config should exist in repo root");

    let config = Config::load(&contents).expect("reference config should parse successfully");

    assert_eq!(config.hosts.len(), 1);
    assert_eq!(config.hosts[0].name, "default");
    assert_eq!(config.hosts[0].host, "127.0.0.1");
    assert_eq!(config.hosts[0].port, 58846);
    assert_eq!(config.hosts[0].username, "localclient");
    assert_eq!(config.hosts[0].password, "password");

    assert_eq!(config.policies.len(), 1);
    assert_eq!(config.policies[0].name, "default");
    assert_eq!(config.policies[0].cron, "0 */1 * * *");

    assert!(config.policies[0].filter.completed);
    assert!(config.policies[0].filter.age.is_none());
    assert!(config.policies[0].filter.ratio.is_none());
    assert!(config.policies[0].filter.min_total_seeds.is_none());
    assert!(config.policies[0].filter.min_distributed_copies.is_none());

    assert!(config.policies[0].conditions.available_space.is_none());
    assert!(config.policies[0].conditions.used_space.is_none());
    assert!(config.policies[0].conditions.total_count.is_none());
}
