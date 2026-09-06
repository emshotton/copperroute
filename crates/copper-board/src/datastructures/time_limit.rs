use web_time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeLimit {
    start: Instant,
    limit_ms: i32,
}

impl TimeLimit {
    pub fn new(milli_seconds: i32) -> Self {
        Self {
            start: Instant::now(),
            limit_ms: milli_seconds,
        }
    }

    pub fn is_exceeded(&self) -> bool {
        let elapsed_ms = self.start.elapsed().as_millis() as i128;
        elapsed_ms > i128::from(self.limit_ms)
    }

    pub fn multiply(&mut self, factor: f64) {
        if !(factor > 0.0) {
            return;
        }
        let new_limit = factor * f64::from(self.limit_ms);
        let new_limit = (new_limit).min(f64::from(i32::MAX));
        self.limit_ms = new_limit as i32;
    }

    pub fn deadline(&self) -> Option<Instant> {
        let millis = u64::try_from(self.limit_ms).ok()?;
        self.start
            .checked_add(std::time::Duration::from_millis(millis))
    }

    pub fn limit_ms(&self) -> i32 {
        self.limit_ms
    }
}

pub type StopCheck<'a> = &'a dyn Fn() -> bool;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_negative_limit_is_exceeded_immediately() {
        assert!(TimeLimit::new(-1).is_exceeded());
    }

    #[test]
    fn a_long_limit_is_not_exceeded_yet() {
        let limit = TimeLimit::new(60_000);
        assert!(!limit.is_exceeded());
        assert!(limit.deadline().is_some());
    }

    #[test]
    fn multiply_ignores_non_positive_factors() {
        let mut limit = TimeLimit::new(1000);
        limit.multiply(0.0);
        assert_eq!(limit.limit_ms(), 1000);
        limit.multiply(-2.0);
        assert_eq!(limit.limit_ms(), 1000);
    }

    #[test]
    fn multiply_scales_and_truncates() {
        let mut limit = TimeLimit::new(1000);
        limit.multiply(2.5);
        assert_eq!(limit.limit_ms(), 2500);
        limit.multiply(0.0004);
        assert_eq!(limit.limit_ms(), 1);
    }

    #[test]
    fn multiply_clamps_at_integer_max_value() {
        let mut limit = TimeLimit::new(10);
        limit.multiply(1e12);
        assert_eq!(limit.limit_ms(), i32::MAX);
        assert!(!limit.is_exceeded());
    }

    #[test]
    fn a_nan_factor_leaves_the_limit_unchanged() {
        let mut limit = TimeLimit::new(1000);
        limit.multiply(f64::NAN);
        assert_eq!(limit.limit_ms(), 1000);
    }

    #[test]
    fn stop_check_reads_a_flag() {
        let stopped = std::cell::Cell::new(false);
        let check = || stopped.get();
        let stop: StopCheck<'_> = &check;
        assert!(!stop());
        stopped.set(true);
        assert!(stop());
    }
}
