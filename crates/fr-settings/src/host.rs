//! Injectable stand-in for `Runtime.getRuntime().availableProcessors()`.
//!
//! Plan Global Constraints / plan ruling 6: every machine-dependent default in
//! `settings/RouterSettings.java` and `settings/sources/DefaultSettings.java` calls
//! `Runtime.getRuntime().availableProcessors()` directly, which would make this crate's
//! behaviour depend on the host machine running the test suite. The port threads a
//! `HostEnvironment` through every call site instead; only [`HostEnvironment::detect`] is allowed
//! to call `std::thread::available_parallelism()`, and every test uses
//! [`HostEnvironment::with_processors`].

/// A machine's reported processor count, injectable so tests are deterministic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostEnvironment {
    available_processors: usize,
}

impl HostEnvironment {
    /// Reads the real host's processor count via `std::thread::available_parallelism()`
    /// (`Runtime.getRuntime().availableProcessors()`'s Rust equivalent). Falls back to `1` if the
    /// platform cannot report it, matching `available_parallelism`'s own documented floor.
    pub fn detect() -> Self {
        let available_processors = std::thread::available_parallelism()
            .map(std::num::NonZero::get)
            .unwrap_or(1);
        Self {
            available_processors,
        }
    }

    /// Builds a `HostEnvironment` reporting exactly `n` processors, for deterministic tests. `n`
    /// is stored as given, including `0` — [`Self::default_max_threads`] is where Java's `0`
    /// handling is reproduced, not here.
    pub fn with_processors(n: usize) -> Self {
        Self {
            available_processors: n,
        }
    }

    /// The processor count this environment reports.
    pub fn available_processors(&self) -> usize {
        self.available_processors
    }

    /// `RouterSettings.defaultMaxThreads` (RouterSettings.java:133-135):
    /// `Math.max(1, Runtime.getRuntime().availableProcessors() - 1)`.
    ///
    /// Java's `availableProcessors()` never returns 0 in practice (the JLS guarantees at least
    /// 1), so `availableProcessors - 1` never underflows there; this port's `usize` subtraction
    /// would panic on `0 - 1`, so `0` is special-cased to match the `Math.max(1, ...)` floor
    /// exactly (`with_processors(0).default_max_threads() == 1`).
    pub fn default_max_threads(&self) -> i32 {
        if self.available_processors == 0 {
            return 1;
        }
        std::cmp::max(1, self.available_processors as i32 - 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_max_threads_matches_java_formula() {
        // RouterSettings.java:133-135: Math.max(1, availableProcessors - 1).
        assert_eq!(HostEnvironment::with_processors(1).default_max_threads(), 1);
        assert_eq!(HostEnvironment::with_processors(8).default_max_threads(), 7);
        // Zero is not a value Java's availableProcessors() ever returns, but the port must not
        // panic on it (usize underflow) and must still satisfy Math.max(1, ...).
        assert_eq!(HostEnvironment::with_processors(0).default_max_threads(), 1);
    }

    #[test]
    fn detect_reports_at_least_one_processor() {
        assert!(HostEnvironment::detect().available_processors() >= 1);
    }
}
