#![forbid(unsafe_code)]

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
// **Plan 8 Tasks 8-10 landed the whole package in `fr-dsn`**, so all six lines below are
// `renamed:` and none is deferred: Task 8 the DTO tree and `readBoard`'s sections 1-8, Task 9
// sections 9-11 and the two helpers only they call, **Task 10 `importSession` and
// `KiCadJsonWriter.write`**. They stay *here* because `scripts/audit-map/fr-drc.map` maps the
// three classes to this file and the `io/kicad` audit runs against `crates/fr-drc/src`;
// `scripts/audit-map/fr-dsn.map` records the real Rust homes so Task 14's sweep can move the
// invocation without re-deriving them.
//
// renamed: KiCadJsonReader.readBoard -> `fr_dsn::kicad::read_board` (crates/fr-dsn/src/kicad/reader.rs), Plan 8 Task 8. It lives in `fr-dsn` and not here because it is a board reader: everything it returns — `fr_board::Board`, `fr_dsn::BoardReadResult`, `fr_dsn::CoordinateTransform` — is that crate's, and `fr-core`'s load path then takes DSN and KiCad JSON through one signature. **Task 8 landed the signature and sections 1-8** (`KiCadJsonReader.java:63-497`); Task 9 extends the same fn body with sections 9-11 (`:498-755`), and the `// obligation:` marker sits where section 9 begins. `scripts/audit-map/fr-drc.map` still maps the class here, which is why this line is here and not there.
// renamed: KiCadJsonReader.importSession -> `fr_dsn::kicad::import_session` (crates/fr-dsn/src/kicad/reader.rs), Plan 8 Task 10 — `KiCadJsonReader.java:757-855`, the traces, vias and conduction areas of a KiCad *session* JSON imported onto an existing board. It has **two** live call sites, both discharged: the `.json` arm of `Freerouting.initializeDrc:301-307` (`crates/freerouting/src/commands/drc.rs`'s `load_session_file`) and the `.json` arm of `RoutingJobScheduler.java:194-207` (`crates/freerouting/src/commands/route.rs`'s `import_session_file`). It lives beside `readBoard` because it is a reader and shares that module's helpers. Quirk #290 (label U) — Java opens the file with `new FileReader`, i.e. the platform default charset — is recorded at the DRC site.
// renamed: KiCadJsonReader.addPoint -> `PointOutline::add_point` in `crates/fr-dsn/src/kicad/reader.rs`, Plan 8 Task 8 — the private `PointOutline` helper class (KiCadJsonReader.java:980-1010), whose two methods section 5 calls to build the outline's bounding box.
// renamed: KiCadJsonReader.boundingBox -> `PointOutline::bounding_box` in `crates/fr-dsn/src/kicad/reader.rs`, Plan 8 Task 8 — see `addPoint` above.
// renamed: KiCadJsonWriter.write -> `fr_dsn::kicad::write` (crates/fr-dsn/src/kicad/writer.rs), Plan 8 Task 10 — `KiCadJsonWriter.java:27-226`, both overloads (the one-argument one is `write(board, fr_dsn::kicad::DEFAULT_DESIGN_NAME)`; it has no caller in the Java tree). It lives in `fr-dsn` beside the reader it is the partial inverse of, and it serialises the same DTO tree through `fr_dsn::format::json::to_gson_string_pretty` — the `GsonProvider.GSON` port. Its CLI call site is `RoutingJobSchedulerActionThread.setJobOutput:275-278`, which is quirk #289 (label T): only the **first** of that method's calls ever writes, so `-do out.json` carries the *pre-routing* board. Byte-for-byte pinned by `crates/fr-dsn/tests/data/p8t10-kicad-writer.txt` over nine boards.
// renamed: KiCadBoardJson.Point2D -> `fr_dsn::kicad::dto::Point2D`, and the other eleven DTOs with it, in `crates/fr-dsn/src/kicad/dto.rs` (Plan 8 Task 8). The audit reads `Point2D` as a method of `KiCadBoardJson` because it is a nested class with a public constructor; the whole 142-line tree moved, not just that one type. Field names are Java's verbatim — they are the JSON wire contract the writer (Task 10) has to write back.

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
// debug block (DesignRulesChecker.java:594-615) is marked at `src/checker.rs:348`.

// --- The REST twin of `-drc` (spec §2: no REST API) ------------------------------------------
//
// not ported: api/v1/JobOutputResource.getDrcReport (api/v1/JobOutputResource.java:590-661) — the `GET /v1/jobs/{jobId}/drc` endpoint. It builds `new DesignRulesChecker(job.board, job.drcSettings)` (`:645`), hard-codes `coordinateUnit = "mm"` (`:648`), takes the source name off `job.input` (`:651`) and returns `drcChecker.generateReportJson(sourceFileName, coordinateUnit)` (`:654`) — the *same* call the CLI's `-drc` makes. Nothing behavioural is missing from this crate; what is missing is the HTTP plumbing and the job store, which spec §2 drops.

// --- Consumers of this crate that belong to other plans --------------------------------------
//
// renamed: autoroute/pipeline/AutorouteUnroutedReport.build (autoroute/pipeline/AutorouteUnroutedReport.java:19-80) -> `fr_router::pipeline::build_unrouted_report` (Plan 7 Task 15) — the diagnostic report the autorouter emits when it stagnates. It is a *consumer* of this crate (`new DesignRulesChecker(board, null)`, `calculateAllIncompletes()`, `getAllAirlines()` at `:20-22`) and belongs to the router, not the DRC layer; it is package-private, so `audit-port.sh` would not see it either way. `describeItem` (`:60-79`) is its private helper, `pipeline::unrouted_report::describe_item`.
// not ported: board/state/BoardComparator.compare (BoardComparator.java, 758 loc) — **controller ruling AS, closed by Plan 8 Task 14.** It diffs two boards. Plan-5 ruling 13 established that nothing in `drc/**` or on the `-drc` path references it, so it is not the DRC layer's; this line records that decision at the crate the ruling was made in. Plan 8 then built the result-manifest/report layer the deferral was pointing at and found no reader there either — see the full evidence at `crates/fr-board/src/board/mod.rs:57`, where the class's own package lives.

// --- Dropped parameters and helpers ----------------------------------------------------------
//
// `DesignRulesChecker`'s `DesignRulesCheckerSettings` constructor parameter is dropped whole
// (plan-5 ruling 12): Java stores the field and never reads it (DesignRulesChecker.java:32, :45),
// 12 of its 14 constructions in `src/main` pass `null` — including both `BoardStatistics` call
// sites and every `autoroute/pipeline/*` one — and `includeWarnings`/`includeErrors` filter
// nothing. Taking it would force `fr-drc -> fr-settings` for a dead field, and Plan 4's quirk
// #115 (a primitive `boolean` `false` is unmergeable) is why it would stay dead. The marker is
// at `src/checker.rs:22`.
//
// Every `FRLogger` call in the ported sources is dropped without a marker, per the plan's global
// constraints — `DesignRulesChecker` is roughly 40 % of them. The ones whose *absence* is
// observable (they build a string a reader might expect to find) carry a marker anyway:
// `src/checker.rs:471`, `src/net_incompletes.rs:67`, `:337`.
