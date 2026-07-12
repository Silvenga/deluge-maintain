use serde::Deserialize;
use crate::config::{Condition, Filter};

#[derive(Debug, Clone, Deserialize)]
pub struct Policy {
    pub name: String,
    pub cron: String,
    #[serde(default)]
    pub filter: Filter,
    #[serde(default)]
    pub conditions: Condition,
}
