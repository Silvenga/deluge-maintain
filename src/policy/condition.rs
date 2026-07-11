use bytesize::ByteSize;
use serde::Deserialize;

#[derive(Debug, Default, Clone, Deserialize)]
pub struct Condition {
    pub available_space: Option<ByteSize>,
    pub used_space: Option<ByteSize>,
    pub total_count: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct ConditionContext {
    pub free_space: i64,
    pub used_space: i64,
    pub torrent_count: usize,
}

impl Condition {
    pub fn is_met(&self, ctx: &ConditionContext) -> bool {
        if let Some(threshold) = self.available_space {
            if ctx.free_space < 0 {
                tracing::warn!(
                    free_space = ctx.free_space,
                    "invalid negative free_space; available_space condition will not be met"
                );
            }
            if ctx.free_space >= 0 && (ctx.free_space as u64) <= threshold.as_u64() {
                return true;
            }
        }

        if let Some(threshold) = self.used_space {
            if ctx.used_space < 0 {
                tracing::warn!(
                    used_space = ctx.used_space,
                    "invalid negative used_space; used_space condition will not be met"
                );
            }
            if ctx.used_space >= 0 && (ctx.used_space as u64) >= threshold.as_u64() {
                return true;
            }
        }

        if let Some(max_count) = self.total_count {
            if ctx.torrent_count as u64 >= max_count as u64 {
                return true;
            }
        }

        false
    }

    pub fn has_any(&self) -> bool {
        self.available_space.is_some() || self.used_space.is_some() || self.total_count.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(free_space: i64, used_space: i64, torrent_count: usize) -> ConditionContext {
        ConditionContext {
            free_space,
            used_space,
            torrent_count,
        }
    }

    #[test]
    fn when_no_conditions_then_should_not_be_met() {
        let condition = Condition::default();

        assert!(!condition.is_met(&ctx(0, 0, 0)));
    }

    #[test]
    fn when_available_space_at_threshold_then_should_be_met() {
        let condition = Condition {
            available_space: Some(ByteSize::gb(50)),
            ..Default::default()
        };

        assert!(condition.is_met(&ctx(50_000_000_000, 0, 0)));
    }

    #[test]
    fn when_available_space_above_threshold_then_should_not_be_met() {
        let condition = Condition {
            available_space: Some(ByteSize::gb(50)),
            ..Default::default()
        };

        assert!(!condition.is_met(&ctx(60_000_000_000, 0, 0)));
    }

    #[test]
    fn when_used_space_at_threshold_then_should_be_met() {
        let condition = Condition {
            used_space: Some(ByteSize::gb(800)),
            ..Default::default()
        };

        assert!(condition.is_met(&ctx(1_000_000_000_000, 800_000_000_000, 0)));
    }

    #[test]
    fn when_used_space_below_threshold_then_should_not_be_met() {
        let condition = Condition {
            used_space: Some(ByteSize::gb(800)),
            ..Default::default()
        };

        assert!(!condition.is_met(&ctx(1_000_000_000_000, 700_000_000_000, 0)));
    }

    #[test]
    fn when_total_count_at_threshold_then_should_be_met() {
        let condition = Condition {
            total_count: Some(500),
            ..Default::default()
        };

        assert!(condition.is_met(&ctx(1_000_000_000_000, 0, 500)));
    }

    #[test]
    fn when_total_count_below_threshold_then_should_not_be_met() {
        let condition = Condition {
            total_count: Some(500),
            ..Default::default()
        };

        assert!(!condition.is_met(&ctx(1_000_000_000_000, 0, 499)));
    }

    #[test]
    fn when_any_condition_met_then_should_be_met() {
        let condition = Condition {
            available_space: Some(ByteSize::gb(50)),
            used_space: Some(ByteSize::gb(800)),
            total_count: Some(500),
        };

        assert!(condition.is_met(&ctx(0, 0, 0)));
    }

    #[test]
    fn when_all_conditions_false_then_should_not_be_met() {
        let condition = Condition {
            available_space: Some(ByteSize::gb(50)),
            used_space: Some(ByteSize::gb(800)),
            total_count: Some(500),
        };

        assert!(!condition.is_met(&ctx(60_000_000_000, 700_000_000_000, 499)));
    }

    #[test]
    fn when_negative_free_space_then_available_space_should_not_be_met() {
        let condition = Condition {
            available_space: Some(ByteSize::gb(50)),
            ..Default::default()
        };

        assert!(!condition.is_met(&ctx(-1, 0, 0)));
    }

    #[test]
    fn when_negative_used_space_then_used_space_should_not_be_met() {
        let condition = Condition {
            used_space: Some(ByteSize::gb(800)),
            ..Default::default()
        };

        assert!(!condition.is_met(&ctx(1_000_000_000_000, -1, 0)));
    }

    #[test]
    fn when_has_any_with_conditions_then_should_return_true() {
        let condition = Condition {
            total_count: Some(100),
            ..Default::default()
        };

        assert!(condition.has_any());
    }

    #[test]
    fn when_has_any_without_conditions_then_should_return_false() {
        let condition = Condition::default();

        assert!(!condition.has_any());
    }
}
