//! `core.scoring.BoardStatisticsClearanceViolations`: the four numbers a board's clearance
//! violations contribute to its quality score.
//!
//! Java: `core/scoring/BoardStatisticsClearanceViolations.java` (a 20-line DTO of four boxed
//! fields and no methods) plus the block of `BoardStatistics`' constructor that fills it
//! (`BoardStatistics.java:338-367`).
//!
//! `BoardStatistics` itself is **not** ported here (plan-5 ruling 5): it needs trace lengths, via
//! counts and bend counts, which spec §4 puts in `fr-core`/Plan 8. Only this block is, because it
//! is the DRC's own contribution to the score and because `fr-drc` already has the list it
//! aggregates. That is also why the unit factor is a parameter rather than a field: Java derives
//! `boardUnitToUmFactor` from `board.communication` at `BoardStatistics.java:200-202`, and
//! `BoardStatistics` is the object that owns it.

use serde::Serialize;

use fr_board::ClearanceViolation;

/// Port of `core.scoring.BoardStatisticsClearanceViolations`
/// (BoardStatisticsClearanceViolations.java:7-20).
///
/// Every field is `Option` because Java's are boxed (`Integer`, `Double`) and start null; Gson
/// omits a null field, which is what `skip_serializing_if` reproduces (the workspace convention,
/// `fr-settings`), and [`Default`] is that state — `a_default_block_serialises_to_nothing` pins
/// it. [`Self::from_violations`] never produces one: `BoardStatistics.java:338-367` writes all
/// four on every path, including the empty one.
///
/// The `@SerializedName`s are snake_case and match the schema; unlike the DRC report (plan-5
/// ruling 1, quirks row #154) this class has no camelCase drift.
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct BoardStatisticsClearanceViolations {
    /// Java `totalCount` (BoardStatisticsClearanceViolations.java:9-10): the size of the
    /// **deduplicated** list `DesignRulesChecker.getAllClearanceViolations` returns
    /// (BoardStatistics.java:341-342), not the report's `violations` array — plan-5 ruling 11
    /// tabulates the two count families.
    #[serde(rename = "total_count", skip_serializing_if = "Option::is_none")]
    pub total_count: Option<i32>,
    /// Java `minViolationUm` (`:12-13`).
    #[serde(rename = "min_violation_um", skip_serializing_if = "Option::is_none")]
    pub min_violation_um: Option<f64>,
    /// Java `maxViolationUm` (`:15-16`).
    #[serde(rename = "max_violation_um", skip_serializing_if = "Option::is_none")]
    pub max_violation_um: Option<f64>,
    /// Java `avgViolationUm` (`:18-19`).
    #[serde(rename = "avg_violation_um", skip_serializing_if = "Option::is_none")]
    pub avg_violation_um: Option<f64>,
}

impl BoardStatisticsClearanceViolations {
    /// Port of the `includeClearanceViolations` block of `BoardStatistics`' constructor
    /// (BoardStatistics.java:338-367).
    ///
    /// `board_unit_to_um_factor` is Java's local of the same name (`BoardStatistics.java:200-202`):
    /// `Unit.scale(1.0, board.communication.unit, Unit.UM) / max(1, board.communication.resolution)`.
    /// It is a parameter here because `BoardStatistics` — which computes it — is Plan 8's.
    ///
    /// Three details of `:344-356` that a paraphrase loses, and that the tests pin:
    ///
    /// * the per-violation quantity is `max(0, expected - actual)` (`:348`), so a violation whose
    ///   actual clearance *exceeds* the expected one contributes `0` rather than a negative;
    /// * the average divides by `violationsList.size()` (`:356`) — **every** violation, not just
    ///   the ones with a positive shortfall;
    /// * the empty list writes `0.0` into all three doubles (`:357-361`), not null, so the JSON
    ///   carries them. That is the `violationsList.isEmpty()` arm; the four zeroes at `:362-367`
    ///   are a *different* arm — `includeClearanceViolations == false` — which this function
    ///   cannot reach, because a caller who does not want the block does not call it.
    ///
    // renamed: BoardStatistics' clearance block (BoardStatistics.java:338-367) -> this associated function, because plan-5 ruling 5 keeps `BoardStatistics` itself out of `fr-drc`.
    pub fn from_violations(
        violations: &[ClearanceViolation],
        board_unit_to_um_factor: f64,
    ) -> BoardStatisticsClearanceViolations {
        // BoardStatistics.java:342.
        let total_count = violations.len() as i32;

        // BoardStatistics.java:343-361. Java seeds the minimum at `Double.MAX_VALUE` and the
        // maximum at `0.0`; the maximum's seed is not observable, because every shortfall is
        // already `>= 0`.
        let (min_violation, max_violation, avg_violation) = if violations.is_empty() {
            // BoardStatistics.java:357-361.
            (0.0, 0.0, 0.0)
        } else {
            // `Math.min`/`Math.max` (`:348`, `:350-351`) **propagate** NaN, where Rust's
            // `f64::min`/`f64::max` return the non-NaN operand. Transcribed with Java's
            // semantics: a NaN clearance is unreachable here — the expected value comes from
            // the clearance matrix and the actual one from `Board::clearance_violations`'
            // bisection, both finite — but the divergence would otherwise be silent.
            let java_min = |a: f64, b: f64| {
                if a.is_nan() || b.is_nan() {
                    f64::NAN
                } else {
                    a.min(b)
                }
            };
            let java_max = |a: f64, b: f64| {
                if a.is_nan() || b.is_nan() {
                    f64::NAN
                } else {
                    a.max(b)
                }
            };

            let mut minimum = f64::MAX;
            let mut maximum = 0.0_f64;
            let mut sum = 0.0;
            for violation in violations {
                // BoardStatistics.java:348-349.
                let shortfall = java_max(
                    0.0,
                    violation.expected_clearance - violation.actual_clearance,
                );
                let shortfall_um = shortfall * board_unit_to_um_factor;
                // BoardStatistics.java:350-352.
                minimum = java_min(minimum, shortfall_um);
                maximum = java_max(maximum, shortfall_um);
                sum += shortfall_um;
            }
            // BoardStatistics.java:356: the divisor is the whole list.
            (minimum, maximum, sum / violations.len() as f64)
        };

        BoardStatisticsClearanceViolations {
            total_count: Some(total_count),
            min_violation_um: Some(min_violation),
            max_violation_um: Some(max_violation),
            avg_violation_um: Some(avg_violation),
        }
    }
}
