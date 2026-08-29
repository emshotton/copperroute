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
//! # The surface
//!
//! [`DesignRulesChecker::new`] over a `&mut Board` (ruling 8), then:
//! [`get_all_clearance_violations`](DesignRulesChecker::get_all_clearance_violations),
//! [`get_all_unconnected_items`](DesignRulesChecker::get_all_unconnected_items),
//! [`calculate_all_incompletes`](DesignRulesChecker::calculate_all_incompletes) and the eight
//! counters that hang off it, [`generate_report`](DesignRulesChecker::generate_report) and
//! [`report_to_json`](DesignRulesChecker::report_to_json) in either [`DrcJsonFlavor`].
//! `README.md` has the whole table, the two schema flavors, the determinism rulings and how to
//! regenerate the references.
//!
//! The `// not ported:` / `// added in Plan N:` roster is the comment block at the foot of this
//! file. It is not decoration: `scripts/audit-port.sh` reads those markers, so a deferral is a
//! gate rather than a silence.

pub mod airline;
pub mod checker;
pub mod error;
pub mod net_incompletes;
pub mod report;
pub mod statistics;
pub mod unconnected;

pub use airline::AirLine;
pub use checker::DesignRulesChecker;
pub use error::DrcError;
pub use net_incompletes::NetIncompletes;
pub use report::{
    DrcCoordinates, DrcJsonFlavor, DrcReportOptions, KiCadDrcPosition, KiCadDrcReport,
    KiCadDrcViolation, KiCadDrcViolationItem,
};
pub use statistics::BoardStatisticsClearanceViolations;
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
    pub use crate::report::{
        DrcCoordinates, DrcJsonFlavor, DrcReportOptions, KiCadDrcPosition, KiCadDrcReport,
        KiCadDrcViolation, KiCadDrcViolationItem,
    };
    pub use crate::statistics::BoardStatisticsClearanceViolations;
    pub use crate::unconnected::{UnconnectedItems, UnconnectedKind};
    pub use fr_board::ClearanceViolation;
}

// ---------------------------------------------------------------------------------------------
// The roster: every Java class in this crate's scope that is *not* ported, with its reason.
//
// `scripts/audit-port.sh` reads the markers below, so each is a checked obligation rather than a
// waiver. The three invocations this crate answers to (all exit 0, no `MISSING`, no `UNMAPPED`):
//
//   ./scripts/audit-port.sh drc          crates/fr-drc/src   '*.java'  scripts/audit-map/fr-drc.map
//   ./scripts/audit-port.sh io/kicad     crates/fr-drc/src   '*.java'  scripts/audit-map/fr-drc.map
//   ./scripts/audit-port.sh core/scoring crates/fr-drc/src \
//       'BoardStatisticsClearanceViolations.java' scripts/audit-map/fr-drc.map
//
// plus `./scripts/audit-port.sh drc crates/fr-board/src 'ClearanceViolation.java' …`, because
// plan-5 ruling 9 puts that one class in `fr-board`.
//
// The audit is **per method**, so the `io/kicad` block below is per method too; the packages the
// audit does not walk (`gui/**`, `api/**`, `autoroute/pipeline`, `board/state`) are listed per
// class, because nothing greps them and the reason is what matters.
// ---------------------------------------------------------------------------------------------

// --- `io/kicad`: the KiCad board/session JSON codec (plan-5 ruling 13) -----------------------
//
// Plan 5 ports the four DRC report DTOs (`report/mod.rs`) and nothing else of the package. The
// three remaining classes are the **KiCad board/session JSON** codec — the `.json` branch of
// `Freerouting.initializeDrc` (`Freerouting.java:296-329`) and the board writer beside it — which
// ruling 13 puts in Plan 8 along with the rest of the `-drc` plumbing. Spec §2 drops KiCad session
// JSON *output* entirely; the *input* path is Plan 8's. Listed by method, because that is the
// granularity `audit-port.sh io/kicad` checks.
//
// added in Plan 8: KiCadJsonReader.readBoard (io/kicad/KiCadJsonReader.java) — the KiCad board JSON reader (1011 loc).
// added in Plan 8: KiCadJsonReader.importSession (io/kicad/KiCadJsonReader.java) — the `.json` session branch of `Freerouting.initializeDrc` (Freerouting.java:296-329).
// added in Plan 8: KiCadJsonReader.addPoint (io/kicad/KiCadJsonReader.java) — a helper of the above.
// added in Plan 8: KiCadJsonReader.boundingBox (io/kicad/KiCadJsonReader.java) — a helper of the above.
// added in Plan 8: KiCadJsonWriter.write (io/kicad/KiCadJsonWriter.java) — the KiCad board JSON writer (227 loc).
// added in Plan 8: KiCadBoardJson.Point2D (io/kicad/KiCadBoardJson.java) — the DTO tree those two exchange (142 loc).

// --- The GUI façades over this crate's compute (spec §2: no GUI) -----------------------------
//
// not ported: gui/workspace/progress/RatsNest.java (457 loc) — the GUI façade over the ratsnest. **It is not a second ratsnest algorithm**: its constructor is `this.drc = new DesignRulesChecker(board, null); this.drc.calculateAllIncompletes();` (RatsNest.java:105-107), and every one of its methods delegates to that `DesignRulesChecker`. What it adds is presentation — a per-net `isFiltered[]` visibility array, hidden/shown airline sets, and the drag-time recalculation the interactive router needs. All of the *compute* is [`DesignRulesChecker`]; `crates/fr-drc/tests/java_ports.rs`'s port of `RatsnestClearanceHeadlessTest` is the Java test that says so.
// not ported: gui/workspace/progress/ClearanceViolations.java (51 loc) — the GUI façade over the clearance list, i.e. `ClearanceViolation.aggregateSortedBySeverity` plus a `Graphics` draw. The compute is `Board::aggregate_violations_sorted_by_severity` (plan-5 ruling 9, in `fr-board`).
// not ported: gui/workspace/progress/RatsNestItemInfo.java — a GUI record of one airline endpoint, for the item-info window.
// not ported: gui/workspace/progress/RatsNestItemType.java — the enum that record carries.
// not ported: gui/rendering/NetIncompletesGraphics.java (102 loc) — draws airlines and length-violation markers on a Swing `Graphics2D`.
// not ported: gui/windows/board/AirLineInfo.java (29 loc) — the item-info row for one airline.
// not ported: gui/windows/board/WindowIncompletes.java (53 loc) — the incompletes list window.
// not ported: gui/windows/routing/WindowClearanceViolations.java (157 loc) — the clearance-violations list window.
//
// `ClearanceViolation.printInfo` (ClearanceViolation.java:96-121) is the one GUI method inside a
// ported class; its marker is at `crates/fr-board/src/items/clearance_violation.rs:22`, where the
// rest of that class lives (ruling 9). `DesignRulesChecker`'s hard-coded `focusNets = {98, 99}`
// debug block (DesignRulesChecker.java:594-615) is marked at `src/checker.rs:346`.

// --- The REST twin of `-drc` (spec §2: no REST API) ------------------------------------------
//
// not ported: api/v1/JobOutputResource.getDrcReport (api/v1/JobOutputResource.java:590-661) — the `GET /v1/jobs/{jobId}/drc` endpoint. It builds `new DesignRulesChecker(job.board, job.drcSettings)` (`:645`), hard-codes `coordinateUnit = "mm"` (`:648`), takes the source name off `job.input` (`:651`) and returns `drcChecker.generateReportJson(sourceFileName, coordinateUnit)` (`:654`) — the *same* call the CLI's `-drc` makes. Nothing behavioural is missing from this crate; what is missing is the HTTP plumbing and the job store, which spec §2 drops.

// --- Consumers of this crate that belong to other plans --------------------------------------
//
// added in Plan 6: autoroute/pipeline/AutorouteUnroutedReport.build (autoroute/pipeline/AutorouteUnroutedReport.java:19-80) — the diagnostic report the autorouter emits when it stagnates. It is a *consumer* of this crate (`new DesignRulesChecker(board, null)`, `calculateAllIncompletes()`, `getAllAirlines()` at `:20-22`) and belongs to the router, not the DRC layer; it is package-private, so `audit-port.sh` would not see it either way.
// added in Plan 8: board/state/BoardComparator.java (758 loc) — diffs two boards for the result-manifest/report layer (spec §10). Plan-5 ruling 13 established that nothing in `drc/**` or the `-drc` path references it, so it is **not** the DRC layer's. Its marker at `crates/fr-board/src/board/mod.rs:57` was re-pointed to Plan 8 by Plan 5 Task 12, per the ruling; this line records the decision at the crate the ruling was made in.

// --- Dropped parameters and helpers ----------------------------------------------------------
//
// `DesignRulesChecker`'s `DesignRulesCheckerSettings` constructor parameter is dropped whole
// (plan-5 ruling 12): Java stores the field and never reads it (DesignRulesChecker.java:32, :44),
// 10 of its 15 constructions pass `null` — including both `BoardStatistics` call sites and every
// `autoroute/pipeline/*` one — and `includeWarnings`/`includeErrors` filter nothing. Taking it
// would force `fr-drc -> fr-settings` for a dead field, and Plan 4's quirk #115 (a primitive
// `boolean` `false` is unmergeable) is why it would stay dead. The marker is at
// `src/checker.rs:22`.
//
// Every `FRLogger` call in the ported sources is dropped without a marker, per the plan's global
// constraints — `DesignRulesChecker` is roughly 40 % of them. The ones whose *absence* is
// observable (they build a string a reader might expect to find) carry a marker anyway:
// `src/checker.rs:469`, `src/net_incompletes.rs:67`, `:337`.
