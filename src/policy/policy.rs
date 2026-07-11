use super::{Condition, Filter};

#[derive(Debug, Clone)]
pub struct Policy {
    pub name: String,
    pub filter: Filter,
    pub conditions: Condition,
}
