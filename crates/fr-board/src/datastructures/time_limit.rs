//! Port of `datastructures/TimeLimit.java` and `datastructures/Stoppable.java`.

use std::time::Instant;

use fr_geometry::java_min;

/// Cancels a performance-critical algorithm once a time limit is exceeded
/// (TimeLimit.java:6-32).
///
/// # Deviation from the task brief
///
/// The brief sketches `TimeLimit { end: Instant }`. That cannot express
/// [`TimeLimit::multiply`] (TimeLimit.java:25-31), which rescales the limit *relative to the
/// original start instant* — Java keeps `timeStamp` (the construction time) and `timeLimit`
/// (milliseconds) as separate fields precisely so that `multiply` can rewrite the second
/// without moving the first. The port keeps Java's two fields; [`TimeLimit::deadline`] derives
/// the brief's `end` on demand.
///
/// # Deviation from Java
///
/// Java reads the wall clock (`new Date().getTime()`, TimeLimit.java:15,20), which an NTP step
/// or a manual clock change can move backwards or forwards mid-run. [`Instant`] is monotonic,
/// so the port cannot mis-measure that way. See `docs/java-quirks.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeLimit {
    /// Java `timeStamp` (TimeLimit.java:8) — when this limit started counting.
    start: Instant,
    /// Java `timeLimit` (TimeLimit.java:9), in milliseconds. `int`, and deliberately so:
    /// [`TimeLimit::multiply`] clamps to `Integer.MAX_VALUE` and then narrows with a Java
    /// `(int)` cast.
    limit_ms: i32,
}

impl TimeLimit {
    /// Creates a new instance with a time limit of `milli_seconds` milliseconds
    /// (TimeLimit.java:12-15). The clock starts now.
    pub fn new(milli_seconds: i32) -> Self {
        Self {
            start: Instant::now(),
            limit_ms: milli_seconds,
        }
    }

    /// Returns true if the time limit provided in the constructor is exceeded
    /// (TimeLimit.java:18-21).
    ///
    /// renamed: `limitExceeded` -> `is_exceeded` (task brief).
    ///
    /// Java compares `currentTime - timeStamp > this.timeLimit` — strictly greater, so a limit
    /// of 0 milliseconds is *not* exceeded within the first millisecond, and a negative limit
    /// is exceeded immediately.
    pub fn is_exceeded(&self) -> bool {
        // `Instant::elapsed` is monotonic and never negative, so the `as i128` widening below
        // only has to cope with a very long run, not with a backwards clock.
        let elapsed_ms = self.start.elapsed().as_millis() as i128;
        elapsed_ms > i128::from(self.limit_ms)
    }

    /// Multiplies this time limit by `factor` (TimeLimit.java:24-31).
    ///
    /// A `factor <= 0` is ignored (TimeLimit.java:25-27); the product is clamped to
    /// `Integer.MAX_VALUE` and then narrowed by Java's `(int)` cast, which saturates at the
    /// `i32` bounds and maps NaN to 0 — exactly what Rust's `as i32` does. `Math.min` is
    /// NaN-propagating, unlike Rust's `f64::min`, hence [`java_min`] (Plan 1 ruling 10): a NaN
    /// `factor` passes the `<= 0` guard (NaN comparisons are false) and must end up as a limit
    /// of 0, not `Integer.MAX_VALUE`.
    pub fn multiply(&mut self, factor: f64) {
        if factor <= 0.0 {
            return;
        }
        let new_limit = factor * f64::from(self.limit_ms);
        let new_limit = java_min(new_limit, f64::from(i32::MAX));
        self.limit_ms = new_limit as i32;
    }

    /// The instant at which [`TimeLimit::is_exceeded`] starts returning true — the task brief's
    /// `end: Instant`, derived from Java's `timeStamp + timeLimit`.
    ///
    /// `None` if the limit is negative or so far in the future that the platform's [`Instant`]
    /// cannot represent it; `is_exceeded` stays the authoritative check either way. Java has no
    /// counterpart (it recomputes the comparison inline every call).
    pub fn deadline(&self) -> Option<Instant> {
        let millis = u64::try_from(self.limit_ms).ok()?;
        self.start
            .checked_add(std::time::Duration::from_millis(millis))
    }

    /// The current limit in milliseconds (Java `timeLimit`, TimeLimit.java:9 — a private field
    /// with no accessor; exposed here so callers can log or assert on it).
    pub fn limit_ms(&self) -> i32 {
        self.limit_ms
    }
}

/// Replaces `datastructures/Stoppable.java`: a borrowed predicate answering "has a stop been
/// requested?".
///
/// Java's `Stoppable` is a two-method interface — `requestStop()` and `isStopRequested()` —
/// implemented by the router's thread classes, which pass *themselves* down into the
/// algorithms as a cancellation token. The algorithms only ever call `isStopRequested()`, so
/// the port passes just that half down; requesting the stop stays with whoever owns the flag
/// (a shared `AtomicBool`, a channel, a UI button), which is where Java's `requestStop()`
/// implementations live too.
///
/// not ported: `Stoppable.requestStop` / `Stoppable.isStopRequested` as an interface — the
/// port has no trait, only this alias for the read side.
pub type StopCheck<'a> = &'a dyn Fn() -> bool;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_negative_limit_is_exceeded_immediately() {
        // TimeLimit.java:20: `currentTime - timeStamp > timeLimit`, and elapsed >= 0.
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
        // TimeLimit.java:25-27.
        let mut limit = TimeLimit::new(1000);
        limit.multiply(0.0);
        assert_eq!(limit.limit_ms(), 1000);
        limit.multiply(-2.0);
        assert_eq!(limit.limit_ms(), 1000);
    }

    #[test]
    fn multiply_scales_and_truncates() {
        // TimeLimit.java:29-30: `(int) Math.min(factor * timeLimit, Integer.MAX_VALUE)`.
        let mut limit = TimeLimit::new(1000);
        limit.multiply(2.5);
        assert_eq!(limit.limit_ms(), 2500);
        limit.multiply(0.0004);
        assert_eq!(limit.limit_ms(), 1); // 2500 * 0.0004 = 1.0
    }

    #[test]
    fn multiply_clamps_at_integer_max_value() {
        let mut limit = TimeLimit::new(10);
        limit.multiply(1e12);
        assert_eq!(limit.limit_ms(), i32::MAX);
        assert!(!limit.is_exceeded());
    }

    #[test]
    fn multiply_by_nan_collapses_to_zero_like_java() {
        // NaN passes the `factor <= 0` guard, `Math.min(NaN, MAX)` is NaN (Rust's `f64::min`
        // would answer `MAX` — Plan 1 ruling 10), and Java's `(int) NaN` is 0.
        let mut limit = TimeLimit::new(1000);
        limit.multiply(f64::NAN);
        assert_eq!(limit.limit_ms(), 0);
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
