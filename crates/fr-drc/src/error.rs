//! The `fr-drc` error type.
//!
//! The DRC compute itself cannot fail — `getAllClearanceViolations`,
//! `getAllUnconnectedItems` and `calculateAllIncompletes` are total over any board Java will
//! hand them — so only the report layer returns this type: [`KiCadDrcReport::to_json`] and
//! [`DesignRulesChecker::report_to_json`] render through `fr_dsn::format::json`, and that is the
//! one place a `serde` failure can surface (a non-finite `f64`, which Gson rejects too).
//!
//! [`KiCadDrcReport::to_json`]: crate::KiCadDrcReport::to_json
//! [`DesignRulesChecker::report_to_json`]: crate::DesignRulesChecker::report_to_json

/// Errors this crate's fallible operations can produce.
#[derive(Debug, thiserror::Error)]
pub enum DrcError {
    /// The KiCad DRC report could not be serialised (`report_to_json`, the port of
    /// `generateReportJson` — DesignRulesChecker.java:817-820).
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}
