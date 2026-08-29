//! The `fr-drc` error type.
//!
//! The DRC compute itself cannot fail — `getAllClearanceViolations`,
//! `getAllUnconnectedItems` and `calculateAllIncompletes` are total over any board Java will
//! hand them — so nothing in Task 3 returns this type. It exists for the report layer: Task 8's
//! `report_to_json` renders through `fr_dsn::format::json`, and that is the one place a `serde`
//! failure can surface.

/// Errors this crate's fallible operations can produce.
#[derive(Debug, thiserror::Error)]
pub enum DrcError {
    /// The KiCad DRC report could not be serialised (Task 8's `report_to_json`, the port of
    /// `generateReportJson` — DesignRulesChecker.java:817-820).
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}
