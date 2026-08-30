//! `calculateScore`, `getMaximumScore` and `getNormalizedScore`
//! (BoardStatistics.java:593-635) — the three methods the whole batch pipeline steers by.
//!
//! **`float`, not `f64`, and Java's association.** All three return Java `float`; every
//! intermediate below is either a `float` or an explicitly-`double` sub-expression that Java
//! narrows with a `(float)` cast at `:611`. An `f64` that is never narrowed changes the score in
//! the last bits, and both `AutorouteBatchLoop:425`'s `> lastBestScore + 0.5` and
//! `BatchOptimizer:220-230`'s `< improvementThreshold` are threshold comparisons on it.

use fr_settings::ScoringSettings;

use super::statistics::BoardStatistics;

impl BoardStatistics {
    /// Port of `calculateScore(ScoringSettings)` (BoardStatistics.java:593-616).
    ///
    /// Java's arithmetic, term by term, because the widths are the point:
    ///
    /// * `penalties` (`:599-602`) is three `int * float` products added **in `float`**, left to
    ///   right — three roundings, not one;
    /// * `costs` (`:610-613`) is a **`double`** sum — `float * Double` promotes, and
    ///   `vias.totalCount * viaCosts` is an `Integer * Integer` **`int`** product widened after
    ///   the fact — narrowed once by the `(float)` at `:611`;
    /// * the return (`:615`) is two `float` subtractions.
    ///
    /// The three penalty weights, the via cost and the trace cost are boxed in Java, so a `null`
    /// unboxes to a `NullPointerException`; the port panics on the same input rather than
    /// inventing a default, because a default would silently change the board the pass loop
    /// picks.
    pub fn calculate_score(&self, scoring: &ScoringSettings) -> f32 {
        // :598.
        let maximum_score = self.maximum_score(scoring);

        let unrouted_net_penalty = scoring
            .unrouted_net_penalty
            .expect("ScoringSettings.unroutedNetPenalty is null (Java: NullPointerException)");
        let clearance_violation_penalty = scoring.clearance_violation_penalty.expect(
            "ScoringSettings.clearanceViolationPenalty is null (Java: NullPointerException)",
        );
        let bend_penalty = scoring
            .bend_penalty
            .expect("ScoringSettings.bendPenalty is null (Java: NullPointerException)");

        // :599-602.
        let incomplete_count = self
            .connections
            .incomplete_count
            .expect("connections.incompleteCount is null (Java: NullPointerException)");
        let violation_count = self
            .clearance_violations
            .total_count
            .expect("clearanceViolations.totalCount is null (Java: NullPointerException)");
        let bend_count = self
            .bends
            .total_count
            .expect("bends.totalCount is null (Java: NullPointerException)");
        let penalties = incomplete_count as f32 * unrouted_net_penalty
            + violation_count as f32 * clearance_violation_penalty
            + bend_count as f32 * bend_penalty;

        // :608-609. `totalLengthMm` is `null` only on a `BoardStatistics` the computing
        // constructor never touched, which is the exact case Java's ternary exists for.
        let trace_length_for_cost = self
            .traces
            .total_length_mm
            .or(self.traces.total_length)
            .expect("traces.totalLength is null (Java: NullPointerException)");

        // :610-613. `double` throughout, then one narrowing.
        let trace_cost = scoring.default_preferred_direction_trace_cost.expect(
            "ScoringSettings.defaultPreferredDirectionTraceCost is null \
             (Java: NullPointerException)",
        );
        let via_costs = scoring
            .via_costs
            .expect("ScoringSettings.viaCosts is null (Java: NullPointerException)");
        let via_count = self
            .vias
            .total_count
            .expect("vias.totalCount is null (Java: NullPointerException)");
        // `Integer * Integer` is an `int` multiplication in Java, so it wraps rather than
        // saturating or widening; `wrapping_mul` is that, and only then does it widen.
        let via_term = f64::from(via_count.wrapping_mul(via_costs));
        let costs = (f64::from(trace_length_for_cost) * trace_cost + via_term) as f32;

        // :615.
        maximum_score - penalties - costs
    }

    /// Port of `getMaximumScore(ScoringSettings)` (BoardStatistics.java:618-621):
    /// `connections.maximumCount * unroutedNetPenalty`.
    ///
    /// **A multiplication — it does not divide.** It answers `0` for a board with no connections,
    /// and it is [`Self::normalized_score`] that would then divide by it.
    ///
    // renamed: BoardStatistics.getMaximumScore -> BoardStatistics::maximum_score (the plan's interface table names it that; Rust getters drop the `get_`).
    pub fn maximum_score(&self, scoring: &ScoringSettings) -> f32 {
        let maximum_count = self
            .connections
            .maximum_count
            .expect("connections.maximumCount is null (Java: NullPointerException)");
        let unrouted_net_penalty = scoring
            .unrouted_net_penalty
            .expect("ScoringSettings.unroutedNetPenalty is null (Java: NullPointerException)");
        maximum_count as f32 * unrouted_net_penalty
    }

    /// Port of `getNormalizedScore(ScoringSettings)` (BoardStatistics.java:623-635) — the whole
    /// plan's loop condition.
    ///
    /// **The `maximumScore <= 0f` guard at `:626-633` is real, and it is what HEAD does.** The
    /// plan and this task's brief both said a connection-less board scores `NaN` here
    /// (`Math.max(0, 0/0)`); it does not. HEAD returns `0f` before the division, with a six-line
    /// comment saying why — "this also prevents NaN propagation which could silently break
    /// stagnation detection". Java wins: there is no NaN quirk, and `empty_board`-shaped fixtures
    /// score a defined `0`. See the task report §Java-wins.
    ///
    /// **That is not the same as "this method never answers NaN".** The guard tests
    /// `maximumScore <= 0f`, and `Infinity <= 0f` is false — so a board whose
    /// `maximumCount * unroutedNetPenalty` overflows `float` walks straight past it, and
    /// `calculateScore` is then `Inf - Inf = NaN`. `Math.max(0, NaN)` (`:634`) is the
    /// two-argument `float` overload after binary numeric promotion and it **propagates** that
    /// NaN, where Rust's `f32::max` would return the non-NaN `0.0` — which is why
    /// `java_max_f32` below is transcribed rather than delegated. `p7t7`'s synthetic case `S5` is
    /// the JVM's own answer for it, and `float_narrowing_matches_java` fails without the
    /// transcription.
    ///
    // renamed: BoardStatistics.getNormalizedScore -> BoardStatistics::normalized_score (the plan's interface table names it that; Rust getters drop the `get_`).
    pub fn normalized_score(&self, scoring: &ScoringSettings) -> f32 {
        // :625.
        let maximum_score = self.maximum_score(scoring);
        // :626-633.
        if maximum_score <= 0.0 {
            return 0.0;
        }
        // :634.
        java_max_f32(0.0, self.calculate_score(scoring) / maximum_score) * 1000.0
    }
}

/// `Math.max(float, float)` (java.lang.Math), transcribed statement for statement rather than
/// delegated to `f32::max`, because the two differ on **both** of the inputs this call can see:
///
/// * a NaN quotient — Rust's `f32::max` returns the *other* operand, so it would answer `0.0`
///   where Java answers `NaN`;
/// * a `-0.0` quotient — a small negative `calculateScore` over a large `maximumScore`
///   underflows to `-0.0f` (`p7t7`'s synthetic case `S4`). Java's second clause makes
///   `max(+0.0, -0.0)` return `+0.0`, while Rust's documents that for inputs which compare
///   equal "either input may be returned non-deterministically". The difference is visible:
///   `Float.toString` renders the two as `0.0` and `-0.0`.
fn java_max_f32(a: f32, b: f32) -> f32 {
    // `if (a != a) return a;`
    if a.is_nan() {
        return a;
    }
    // `if ((a == 0.0f) && (b == 0.0f) && (floatToRawIntBits(b) == negativeZeroFloatBits)) return a;`
    if a == 0.0 && b == 0.0 && b.to_bits() == (-0.0_f32).to_bits() {
        return a;
    }
    // `return (a >= b) ? a : b;` — false against a NaN `b`, so the NaN is returned.
    if a >= b { a } else { b }
}
