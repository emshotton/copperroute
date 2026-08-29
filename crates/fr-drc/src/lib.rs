//! `fr-drc`: freerouting's design-rule checker — a faithful port of
//! `drc/{DesignRulesChecker,ClearanceViolation,NetIncompletes,AirLine,UnconnectedItems}.java`,
//! the four `io/kicad/KiCadDrc*.java` report DTOs and
//! `core/scoring/BoardStatisticsClearanceViolations.java` (freerouting, clone HEAD — plan-5
//! ruling 1 pins both the sources and the parity jar to HEAD).
//!
//! It answers the three questions Java's `-drc` mode answers — *which clearances are violated,
//! which items are unconnected, and how many connections are still incomplete* — and answers
//! them the way Java does, bug for bug.
//!
//! See the plan doc (`docs/superpowers/plans/2026-08-29-plan-5-drc.md`) for scope, architecture
//! and rulings; `README.md` for the API surface and the ported-vs-deferred table.
//!
//! # Where things live
//!
//! [`ClearanceViolation`] is defined in `fr-board`, not here (plan-5 ruling 9):
//! `Item.clearanceViolations` returns it and `fr-board` cannot depend upward on `fr-drc`. It is
//! re-exported below, so `fr_drc::ClearanceViolation` is the name callers use. The four item
//! methods behind it — `Board::{clearance_violations, clearance_violation_count,
//! calculate_clearance_between_two_shapes, aggregate_violations_sorted_by_severity,
//! smallest_clearance}` — are `fr-board`'s for the same reason.
//!
//! This crate sits on `fr-board` and on `fr-dsn` (plan-5 ruling 7): `CoordinateTransform` lives
//! in `fr-dsn`, the report's floats need `java_double_to_string`, and the report's JSON goes
//! through `fr_dsn::format::json`. `fr-drc -> fr-dsn -> fr-board -> fr-geometry` stays strict and
//! acyclic. There is **no `fr-settings` dependency** (plan-5 ruling 12), no `tracing` — every
//! `FRLogger` call in the Java sources is dropped, and `DesignRulesChecker` is 40 % of them — no
//! GUI, no static mutable state and **no clock**: `KiCadDrcReport`'s `ZonedDateTime.now()`
//! (KiCadDrcReport.java:70) becomes an injected `String` (plan-5 ruling 5).
//!
//! # Task scope
//!
//! Task 3 ships the crate skeleton, [`DesignRulesChecker`] and
//! [`DesignRulesChecker::get_all_clearance_violations`]; Task 4 adds [`UnconnectedItems`] and
//! [`DesignRulesChecker::get_all_unconnected_items`]. Task 5 adds `AirLine`, `NetIncompletes`
//! and the ratsnest; Task 6 adds `calculate_all_incompletes`, the counters and
//! `BoardStatisticsClearanceViolations`;
//! Tasks 7-8 add the KiCad DTOs, `generate_report` and `report_to_json`; Task 11 adds this
//! crate's `README.md` and the `// not ported:` roster at the foot of this file.

pub mod airline;
pub mod checker;
pub mod error;
pub mod net_incompletes;
pub mod unconnected;

pub use airline::AirLine;
pub use checker::DesignRulesChecker;
pub use error::DrcError;
pub use net_incompletes::NetIncompletes;
pub use unconnected::{UnconnectedItems, UnconnectedKind};

/// `drc.ClearanceViolation`, defined in `fr-board` (plan-5 ruling 9) and re-exported here so
/// callers of this crate spell it `fr_drc::ClearanceViolation`, as Java's `drc` package does.
pub use fr_board::ClearanceViolation;

/// Everything a caller normally needs in one `use`.
pub mod prelude {
    pub use crate::airline::AirLine;
    pub use crate::checker::DesignRulesChecker;
    pub use crate::error::DrcError;
    pub use crate::net_incompletes::NetIncompletes;
    pub use crate::unconnected::{UnconnectedItems, UnconnectedKind};
    pub use fr_board::ClearanceViolation;
}
