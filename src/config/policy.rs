use crate::config::{Condition, Filter};
use serde::Deserialize;
use std::iter::once;

#[derive(Debug, Clone, Deserialize)]
pub struct Policy {
    pub name: String,
    pub cron: String,
    #[serde(default)]
    pub filter: Filter,
    #[serde(default)]
    pub conditions: Condition,
}

impl Policy {
    pub fn cron(&self) -> String {
        let fields: Vec<&str> = self.cron.split_whitespace().collect();
        if fields.len() == 5 {
            once("0")
                .chain(fields.iter().copied())
                .collect::<Vec<_>>()
                .join(" ")
        } else {
            fields.join(" ")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_policy(cron: &str) -> Policy {
        Policy {
            name: "test".to_owned(),
            cron: cron.to_owned(),
            filter: Filter::default(),
            conditions: Condition::default(),
        }
    }

    #[test]
    fn when_five_field_cron_then_should_prepend_seconds() {
        let policy = make_policy("0 */6 * * *");

        assert_eq!(policy.cron(), "0 0 */6 * * *");
    }

    #[test]
    fn when_six_field_cron_then_should_return_unchanged() {
        let policy = make_policy("*/1 * * * * *");

        assert_eq!(policy.cron(), "*/1 * * * * *");
    }

    #[test]
    fn when_five_field_cron_with_extra_whitespace_then_should_normalize() {
        let policy = make_policy("0   */6   *   *   *");

        assert_eq!(policy.cron(), "0 0 */6 * * *");
    }

    #[test]
    fn when_six_field_cron_with_extra_whitespace_then_should_preserve_fields() {
        let policy = make_policy("*/1   *   *   *   *   *");

        assert_eq!(policy.cron(), "*/1 * * * * *");
    }
}
