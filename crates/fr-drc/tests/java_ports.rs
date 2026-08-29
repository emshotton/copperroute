//! **The five ported Java DRC suites, by name.**
//!
//! Plan 5 Task 11. Every test below is a named port of a Java test method, so the audit trail
//! from a Java `@Test` to its Rust counterpart is one `grep`. The detailed assertions live in the
//! task suites this crate already has (`clearance_list.rs`, `unconnected.rs`,
//! `net_incompletes.rs`, `incompletes.rs`, `report.rs`, `report_json.rs`, `reference_parity.rs`);
//! these are the *tests Java writes*, transcribed, with Java's own numbers.
//!
//! | Java suite | loc | ported here as |
//! |---|---|---|
//! | `drc/DesignRulesCheckerTest.java:28-62` (`testDrcReportStructure`) | 97 | [`report_structure_on_issue555_bbd_mars_64`] |
//! | `drc/DesignRulesCheckerTest.java:64-96` (`testDrcReportJsonFormat`) | | [`report_json_has_every_head_key`] |
//! | `drc/DrcCoordinateTest.java:26-72` (`testDrcCoordinatesAreInCorrectRange`) | 73 | [`coordinates_are_in_a_plausible_mm_range`] |
//! | `drc/RatsnestClearanceHeadlessTest.java:50-74` | 128 | [`incompletes_are_computable_via_the_checker_alone`] |
//! | `drc/RatsnestClearanceHeadlessTest.java:78-88` | | [`clearance_violations_are_computable_via_the_checker_alone`] |
//! | `drc/RatsnestClearanceHeadlessTest.java:90-114` | | [`aggregation_is_headless_and_severity_sorted`] |
//! | `drc/RatsnestClearanceHeadlessTest.java:116-127` | | [`empty_board_has_no_incompletes_and_no_violations`] |
//! | `drc/UnconnectedItemsReproductionTest.java:49-209` | 210 | [`issue575_drc_reproduction_single_pass`] |
//! | `fixtures/KiCadDrcViolationRoutingTest.java:51-65` (three `@Test`s) | 66 | [`the_board_statistics_oracle`] |
//!
//! # `KiCadDrcViolationRoutingTest`'s numbers are not the report's numbers (plan-5 ruling 11)
//!
//! The Java test asserts `BoardStatistics.connections.incompleteCount` and
//! `BoardStatistics.clearanceViolations.totalCount`; the *report* carries `unconnectedItems` and
//! `violations`, and every one of the four is a different quantity. Both families are measured on
//! the clone's HEAD jar — the left half by `new BoardStatistics(board)` itself, the right half by
//! `tests/data/*.report.txt`'s `ReportProbe` — and both are pinned here:
//!
//! | fixture | `clearanceViolations.totalCount` | report `violations` | breakdown | `incompleteCount` | report `unconnectedItems` |
//! |---|---|---|---|---|---|
//! | `…dev-board_4_hole_clearance_violations` | **2** | 10 | 2 `holeClearance` + 8 `track_dangling` | **9** | 4 |
//! | `…BBD_Mars-64_6_track_1_hole…` | **76** | 96 | 64 `holeClearance` + 12 `clearance` + 18 `via_dangling` + 2 `track_dangling` | **3** | 3 |
//! | `…Natural_Tone_Preamp_7_unconnected_items` | **0** | 114 (the JVM; **112** in the port, quirk #146) | 110 (JVM) / 108 (port) `track_dangling` + 4 `via_dangling` | **145** | 44 |
//!
//! The reconciliation is exact and mechanical, and it is why the survey's "dev board gives 10 / 4"
//! and this test's "9, 2" are **both right**:
//!
//! - `report.violations` = the deduplicated clearance list (`DesignRulesChecker.java:216`,
//!   `:231-233`) **plus** the `track_dangling`/`via_dangling` entries `generateReport` moves out of
//!   `getAllUnconnectedItems` (`:268-276`). `clearanceViolations.totalCount` is only the first
//!   summand — 2 of the dev board's 10, 76 of BBD Mars-64's 96.
//! - `report.unconnectedItems` = one entry per net with ≥ 2 connected sets
//!   (`DesignRulesChecker.java:118-146`), whereas `incompleteCount` is the number of **airlines**,
//!   Σ (groups − 1) over the nets (`NetIncompletes`) — which is why the dev board's one 7-group
//!   net contributes 1 entry and 6 airlines. `KiCadDrcViolationRoutingTest.java:41-48` explains it
//!   in its own words.
//!
//! Nobody should "fix" the apparent contradiction: [`the_board_statistics_oracle`] and
//! [`the_report_arrays_are_longer_than_the_statistics_counters`] assert both halves of every row
//! against the same board, in the same test binary.
//!
//! # Deliberate divergences from the Java text
//!
//! - **No `DesignRulesCheckerSettings` argument.** Java's constructor takes one and never reads it
//!   (plan-5 ruling 12); `RatsnestClearanceHeadlessTest` passes `null` and
//!   `UnconnectedItemsReproductionTest` passes a fresh one, and the two agree — which is the
//!   ruling's evidence.
//! - **`DrcCoordinateTest`'s `System.out.println`s, and `UnconnectedItemsReproductionTest`'s six,
//!   are dropped.** They assert nothing.
//! - **`generateReport`/`generateReportJson` take `DrcCoordinates` and `DrcReportOptions`**
//!   (rulings 5 and 7): Java reaches the transform through `board.communication` and the date
//!   through `ZonedDateTime.now()`, neither of which exists here.
//! - **`RoutingFixtureTest`/`BoardLoader` are not ported**; every board here is read with
//!   `fr_dsn::read_board`, which is what `RatsnestClearanceHeadlessTest.java:36-46` does directly.

mod common;
use common::JAR_VERSION;

use fr_board::prelude::*;
use fr_drc::report::{DrcCoordinates, DrcReportOptions};
use fr_drc::{
    BoardStatisticsClearanceViolations, DesignRulesChecker, DrcJsonFlavor, UnconnectedKind,
};
use fr_dsn::{BoardReadResult, CoordinateTransform, DsnReadOptions};

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

/// `DesignRulesCheckerTest.java:31` and `:67`.
const ISSUE555_BBD_MARS_64: &str = "Issue555-BBD_Mars-64.dsn";
/// `RatsnestClearanceHeadlessTest.java:30-31`, `KiCadDrcViolationRoutingTest.java:59`.
const DEV_BOARD: &str = "Issue575-drc_dev-board_4_hole_clearance_violations.dsn";
/// `KiCadDrcViolationRoutingTest.java:54`.
const BBD_MARS_64: &str = "Issue575-drc_BBD_Mars-64_6_track_1_hole_clearance_violations.dsn";
/// `DrcCoordinateTest.java:29`, `UnconnectedItemsReproductionTest.java:34-35`,
/// `KiCadDrcViolationRoutingTest.java:64`.
const NATURAL_TONE_PREAMP: &str = "Issue575-drc_Natural_Tone_Preamp_7_unconnected_items.dsn";
/// `RatsnestClearanceHeadlessTest.java:118`.
const EMPTY_BOARD: &str = "empty_board.dsn";

/// `RatsnestClearanceHeadlessTest.loadBoard` (`:36-46`): `DsnReader.readBoard`, accepting both
/// `Success` and `OutlineMissing`.
fn fixture_board(name: &str) -> (Board, CoordinateTransform) {
    let path = parity::fixture(name);
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("cannot read fixture {}: {e}", path.display()));
    match fr_dsn::read_board(&bytes[..], None, Some(name), &DsnReadOptions::default()) {
        BoardReadResult::Success {
            board,
            coordinate_transform,
            ..
        }
        | BoardReadResult::OutlineMissing {
            board,
            coordinate_transform,
            ..
        } => (
            *board.unwrap_or_else(|| panic!("{name} produced no board")),
            coordinate_transform.unwrap_or_else(|| panic!("{name} produced no transform")),
        ),
        other => panic!("{name} did not read: {other:?}"),
    }
}

/// The pair Java reads off `board.communication` (DesignRulesChecker.java:507, `:510`).
fn coords(board: &Board, transform: CoordinateTransform) -> DrcCoordinates {
    DrcCoordinates {
        board_unit: board.communication.unit,
        transform,
    }
}

/// What `Freerouting.initializeDrc` injects (plan-5 ruling 5). The Java tests call
/// `generateReport("test.dsn", "mm")`, i.e. only the first two; `date` and
/// `freerouting_version` are the constructor's own in Java and the score is set by the CLI
/// afterwards, so `None` here is exactly `generateReportJson`'s shape.
fn options(source: &str) -> DrcReportOptions {
    DrcReportOptions {
        source: source.to_string(),
        coordinate_unit: "mm".to_string(),
        date: "2026-08-29T00:00:00Z".to_string(),
        freerouting_version: JAR_VERSION.to_string(),
        quality_score: None,
    }
}

// ---------------------------------------------------------------------------------------------
// DesignRulesCheckerTest.java (97 loc)
// ---------------------------------------------------------------------------------------------

/// Port of `DesignRulesCheckerTest.testDrcReportStructure` (`DesignRulesCheckerTest.java:28-62`).
///
/// Java's seven `assertNotNull`s over `job`, `job.board`, `report`, `report.violations`,
/// `report.unconnectedItems` and `report.schematicParity` are all `Option`/`Vec`-typed here, so
/// only the four value assertions survive as assertions; the rest are the type system's. The
/// fixture is `Issue555-BBD_Mars-64.dsn` — **not** the `Issue575-drc_BBD_Mars-64…` board the other
/// four suites use.
#[test]
fn report_structure_on_issue555_bbd_mars_64() {
    if !parity::require_java_dir() {
        return;
    }
    let (mut board, transform) = fixture_board(ISSUE555_BBD_MARS_64);
    let coords = coords(&board, transform);
    // `DesignRulesCheckerTest.java:43-47`, without the settings argument (ruling 12).
    let report = DesignRulesChecker::new(&mut board).generate_report(&coords, &options("test.dsn"));

    // `:51-54`.
    assert_eq!(report.json_schema, "https://schemas.kicad.org/drc.v1.json");
    // `:55`.
    assert_eq!(report.coordinate_units, "mm");
    // `:56`.
    assert_eq!(report.source, "test.dsn");
    // `:60-61`.
    assert!(
        report.freerouting_version.contains("Freerouting"),
        "version should contain Freerouting, was {:?}",
        report.freerouting_version
    );
}

/// Port of `DesignRulesCheckerTest.testDrcReportJsonFormat`
/// (`DesignRulesCheckerTest.java:64-96`): the eight `json.has(...)` assertions, which are the
/// Java test that pins ruling 1's **camelCase** spelling (`:89`, `:91`, `:94`, `:95`).
#[test]
fn report_json_has_every_head_key() {
    if !parity::require_java_dir() {
        return;
    }
    let (mut board, transform) = fixture_board(ISSUE555_BBD_MARS_64);
    let coords = coords(&board, transform);
    let text = DesignRulesChecker::new(&mut board)
        .report_to_json(
            &coords,
            &options("test.dsn"),
            // `generateReportJson` has no flavor; HEAD's spelling is what the Java test asserts.
            DrcJsonFlavor::FreeroutingHead,
        )
        .expect("the report serialises");

    // `:83-84`.
    assert!(!text.is_empty());
    // `:87` — valid JSON, and an object.
    let json: serde_json::Value = serde_json::from_str(&text).expect("valid JSON");
    let object = json.as_object().expect("a JSON object");

    // `:88-95`, in Java's order.
    for key in [
        "$schema",
        "coordinateUnits",
        "date",
        "kicadVersion",
        "source",
        "violations",
        "unconnectedItems",
        "schematicParity",
    ] {
        assert!(object.contains_key(key), "JSON should have {key}");
    }
}

// ---------------------------------------------------------------------------------------------
// DrcCoordinateTest.java (73 loc)
// ---------------------------------------------------------------------------------------------

/// Port of `DrcCoordinateTest.testDrcCoordinatesAreInCorrectRange`
/// (`DrcCoordinateTest.java:26-72`): the first `unconnectedItems` entry's first item's `pos` is a
/// plausible millimetre coordinate — `10 < |x|, |y| < 500` — which catches the 10×-too-large
/// `convertCoordinate` the test was written against.
///
/// Java guards the whole body with `if (unconnectedItems != null && size > 0)`, so on a board with
/// no unconnected items it asserts **nothing**. Natural Tone Preamp has 44 entries, and this port
/// asserts that too rather than inheriting the vacuous case.
#[test]
fn coordinates_are_in_a_plausible_mm_range() {
    if !parity::require_java_dir() {
        return;
    }
    let (mut board, transform) = fixture_board(NATURAL_TONE_PREAMP);
    let coords = coords(&board, transform);
    // `:42` — through the JSON, as Java does, so `convertCoordinate` is exercised end to end.
    let text = DesignRulesChecker::new(&mut board)
        .report_to_json(
            &coords,
            &options("test.dsn"),
            DrcJsonFlavor::FreeroutingHead,
        )
        .expect("the report serialises");
    let json: serde_json::Value = serde_json::from_str(&text).expect("valid JSON");

    // `:48-53`.
    let unconnected = json["unconnectedItems"].as_array().expect("an array");
    assert!(
        !unconnected.is_empty(),
        "Java's guard makes this test vacuous on a board with no unconnected items; \
         Natural Tone Preamp has 44 entries and must keep having them"
    );
    let items = unconnected[0]["items"].as_array().expect("an array");
    assert!(!items.is_empty());
    let pos = &items[0]["pos"];
    let x = pos["x"].as_f64().expect("a number");
    let y = pos["y"].as_f64().expect("a number");

    // `:62-67`.
    assert!(x.abs() < 500.0, "X should be less than 500mm, but was {x}");
    assert!(y.abs() < 500.0, "Y should be less than 500mm, but was {y}");
    assert!(x.abs() > 10.0, "X should be greater than 10mm, but was {x}");
    assert!(y.abs() > 10.0, "Y should be greater than 10mm, but was {y}");
}

// ---------------------------------------------------------------------------------------------
// RatsnestClearanceHeadlessTest.java (128 loc)
// ---------------------------------------------------------------------------------------------

/// `RatsnestClearanceHeadlessTest.java:33`.
const EXPECTED_UNCONNECTED: usize = 9;
/// `RatsnestClearanceHeadlessTest.java:34`.
const EXPECTED_UNIQUE_VIOLATIONS: usize = 2;

/// Port of `incompletesAreComputableViaDesignRulesCheckerWithoutGuiFacade`
/// (`RatsnestClearanceHeadlessTest.java:50-74`).
///
/// The Java test's point is architectural — the ratsnest is reachable without
/// `interactive.RatsNest` — and in the port it is a tautology: `gui/workspace/progress/RatsNest`
/// is a `not ported:` line in `src/lib.rs`'s roster and there is no GUI to import. What survives
/// is the arithmetic: `getIncompleteCount() == getAllAirlines().length == Σ per-net count == 9`.
#[test]
fn incompletes_are_computable_via_the_checker_alone() {
    if !parity::require_java_dir() {
        return;
    }
    let (mut board, _) = fixture_board(DEV_BOARD);
    let max_net_number = board.rules.nets.max_net_number();
    let mut drc = DesignRulesChecker::new(&mut board);
    // `:55`.
    drc.calculate_all_incompletes();

    // `:57-60`.
    assert_eq!(drc.get_incomplete_count(), EXPECTED_UNCONNECTED);

    // `:62-66`.
    let airlines = drc.get_all_airlines();
    assert!(!airlines.is_empty());
    assert_eq!(airlines.len(), EXPECTED_UNCONNECTED);

    // `:69-73`.
    let sum_per_net: usize = (1..=max_net_number)
        .map(|n| drc.get_incomplete_count_for_net(n))
        .sum();
    assert_eq!(sum_per_net, EXPECTED_UNCONNECTED);
}

/// Port of `clearanceViolationsAreComputableViaDrcWithoutGuiFacade`
/// (`RatsnestClearanceHeadlessTest.java:78-88`): the **deduplicated** count, which is the one
/// `getAllClearanceViolations` returns (`DesignRulesChecker.java:100-116`).
#[test]
fn clearance_violations_are_computable_via_the_checker_alone() {
    if !parity::require_java_dir() {
        return;
    }
    let (mut board, _) = fixture_board(DEV_BOARD);
    let deduped = DesignRulesChecker::new(&mut board).get_all_clearance_violations();
    // `:84-87`.
    assert_eq!(deduped.len(), EXPECTED_UNIQUE_VIOLATIONS);
}

/// Port of `clearanceViolationAggregationHelpersAreHeadlessAndSeveritySorted`
/// (`RatsnestClearanceHeadlessTest.java:90-114`).
///
/// `ClearanceViolation.aggregateSortedBySeverity(board.getItems())` and
/// `ClearanceViolation.smallestClearance(board.getItems())` are `Board` methods here (plan-5
/// ruling 9): both take the whole item set and both are `fr-board`'s, because
/// `Item.clearanceViolations` is.
///
/// The order of the two calls is Java's and it matters: `smallestClearance` reads each item's
/// `smallestClearance` **field**, which only a `clearanceViolations()` query lowers
/// (`Item.java:451-453`, quirk #153). Called first it would answer `Double.MAX_VALUE` and the
/// assertion at `:111-113` would fail.
#[test]
fn aggregation_is_headless_and_severity_sorted() {
    if !parity::require_java_dir() {
        return;
    }
    let (mut board, _) = fixture_board(DEV_BOARD);

    // `:95-96`.
    let aggregated = board.aggregate_violations_sorted_by_severity();
    // `:97`.
    assert!(!aggregated.is_empty());
    // `:99-101` — the per-item aggregation double-counts each pair.
    assert!(aggregated.len() >= EXPECTED_UNIQUE_VIOLATIONS);

    // `:104-108`: descending by `expected - actual`.
    for pair in aggregated.windows(2) {
        let previous = pair[0].expected_clearance - pair[0].actual_clearance;
        let current = pair[1].expected_clearance - pair[1].actual_clearance;
        assert!(
            previous >= current,
            "aggregated violations must be sorted by severity, descending"
        );
    }

    // `:110-113`.
    let smallest = board.smallest_clearance();
    assert!(
        (0.0..f64::MAX).contains(&smallest),
        "smallestClearance must be non-negative and finite, was {smallest}"
    );
}

/// Port of `emptyBoardHasNoIncompletesAndNoViolations`
/// (`RatsnestClearanceHeadlessTest.java:116-127`).
#[test]
fn empty_board_has_no_incompletes_and_no_violations() {
    if !parity::require_java_dir() {
        return;
    }
    let (mut board, _) = fixture_board(EMPTY_BOARD);
    {
        let mut drc = DesignRulesChecker::new(&mut board);
        // `:121`.
        assert_eq!(drc.get_incomplete_count(), 0);
        // `:122-123`.
        assert!(drc.get_all_clearance_violations().is_empty());
    }
    // `:124-126`.
    assert!(board.aggregate_violations_sorted_by_severity().is_empty());
}

// ---------------------------------------------------------------------------------------------
// UnconnectedItemsReproductionTest.java (210 loc)
// ---------------------------------------------------------------------------------------------

/// `UnconnectedItemsReproductionTest.java:38`.
const EXPECTED_UNCONNECTED_NET_GROUPS: usize = 9;
/// `UnconnectedItemsReproductionTest.java:40`.
const EXPECTED_DANGLING_TRACKS: usize = 24;
/// `UnconnectedItemsReproductionTest.java:41`.
const EXPECTED_DANGLING_VIAS: usize = 4;

/// Port of `testIssue575DrcReproductionSinglePass`
/// (`UnconnectedItemsReproductionTest.java:49-209`) — one pass over the expensive fixture, as
/// Java's own comment (`:28-30`) asks for.
///
/// Java asserts **lower bounds** throughout (24 / 4 / 9), because its author expected
/// normalisation artefacts to inflate the counts — and they are *historical* bounds besides: the
/// "reference JSON" they were read from is the stale 2.1.2-era
/// `../freerouting/fixtures/*-freerouting_drc.json`, which plan-5 ruling 10 disqualifies as a
/// reference. Every bound is kept, because the Java test keeps them, and the exact numbers are
/// pinned beside each one: a lower bound cannot fail on over-detection, which is the regression
/// this fixture is most likely to grow.
///
/// The same three quantities appear twice in the Java test — once off `getAllUnconnectedItems`
/// (`:104-107`) and once off the report (`:179-186`) — and they **differ**, by exactly the three
/// `track_dangling` entries `generateReport`'s second pass drops. Both are asserted.
#[test]
fn issue575_drc_reproduction_single_pass() {
    if !parity::require_java_dir() {
        return;
    }
    let (mut board, transform) = fixture_board(NATURAL_TONE_PREAMP);
    let coords = coords(&board, transform);

    // `:59-72`: two GND traces on opposite layers, same net.
    let trace_2402 = ItemId(2402);
    let trace_2411 = ItemId(2411);
    for id in [trace_2402, trace_2411] {
        let item = board
            .get_item(id)
            .unwrap_or_else(|| panic!("item {id} not found"));
        assert!(item.is_trace(), "item {id} should be a Trace");
    }
    assert_eq!(
        board.get_item(trace_2402).unwrap().get_net_number(0),
        board.get_item(trace_2411).unwrap().get_net_number(0),
        "traces 2402 and 2411 must belong to the same net"
    );

    // `:76-79`: 2402 is genuinely dangling.
    assert!(
        board.is_tail(trace_2402),
        "trace 2402 (GND) is expected to be a dangling trace"
    );

    // `:81-89`: pin 321 is *not* among 2402's normal contacts — the correct board state, not a
    // false negative from the DRC.
    let pin_321 = ItemId(321);
    if board.get_item(pin_321).is_some() {
        assert!(
            !board.normal_contacts(trace_2402).contains(&pin_321),
            "trace 2402 correctly has NO connection to pin 321 in this board state"
        );
    }

    // `:91-95`: via 2522 exists and is a via. Java's three `println`s about its layers, tail state
    // and contact count (`:97-99`) assert nothing and are dropped; `:101-102` says so.
    let via_2522 = ItemId(2522);
    assert!(
        matches!(
            board
                .get_item(via_2522)
                .unwrap_or_else(|| panic!("via 2522 should be found in the board"))
                .kind(),
            ItemKind::Via
        ),
        "item 2522 should be a Via"
    );

    // `:104-107`.
    let entries = DesignRulesChecker::new(&mut board).get_all_unconnected_items();
    let count = |kind| entries.iter().filter(|e| e.kind == kind).count();
    let dangling_tracks = count(UnconnectedKind::TrackDangling);
    let dangling_vias = count(UnconnectedKind::ViaDangling);
    let net_groups = count(UnconnectedKind::UnconnectedItems);

    // `:115-133` and `:144-150` — Java's three lower bounds…
    assert!(dangling_tracks >= EXPECTED_DANGLING_TRACKS);
    assert!(dangling_vias >= EXPECTED_DANGLING_VIAS);
    assert!(net_groups >= EXPECTED_UNCONNECTED_NET_GROUPS);
    // …and the exact numbers behind them. 108 dangling tracks, not the JVM transcript's 111
    // (`tests/data/Issue575-drc_Natural_Tone_Preamp_7_unconnected_items.unconnected.txt` lists
    // 111 candidates): the trace phase drops a candidate that is already some net entry's
    // `firstItem` (`DesignRulesChecker.java:157-160`), and *which* candidate that is depends on
    // `findRepresentativeItem`, which is hash-ordered in Java (0-3 dropped over the six hash
    // modes) and ascending-id in the port (exactly 3 dropped — plan-5 rulings 3 and S, quirk
    // #146). Java's own bounds above are satisfied either way, which is why they cannot see this.
    assert_eq!((net_groups, dangling_tracks, dangling_vias), (44, 108, 4));

    // `:152-170`: the four ids the reference JSON names — GND/Top, +5V/Top, GND/Bottom,
    // +5V/Bottom — are traces, are tails, and are reported dangling.
    let dangling_ids: Vec<ItemId> = entries
        .iter()
        .filter(|e| e.kind == UnconnectedKind::TrackDangling)
        .map(|e| e.first_item)
        .collect();
    for id in [2340, 1869, 2372, 1802].map(ItemId) {
        let item = board
            .get_item(id)
            .unwrap_or_else(|| panic!("track {id} should exist in the board"));
        assert!(item.is_trace(), "item {id} should be a Trace");
        assert!(board.is_tail(id), "track {id} should be dangling (is_tail)");
        assert!(
            dangling_ids.contains(&id),
            "DRC should detect track {id} as a dangling track"
        );
    }

    // `:172-208`: the same three quantities again, through the report this time.
    let report =
        DesignRulesChecker::new(&mut board).generate_report(&coords, &options(NATURAL_TONE_PREAMP));
    let violations_of = |kind: &str| report.violations.iter().filter(|v| v.kind == kind).count();
    let track_dangling = violations_of("track_dangling");
    let via_dangling = violations_of("via_dangling");

    assert!(track_dangling >= EXPECTED_DANGLING_TRACKS);
    assert!(via_dangling >= EXPECTED_DANGLING_VIAS);
    assert!(report.unconnected_items.len() >= EXPECTED_UNCONNECTED_NET_GROUPS);
    // The exact numbers. 108, not the entry list's 111: `generateReport` re-runs
    // `getAllUnconnectedItems`, whose trace phase drops a candidate that is some net entry's
    // `firstItem` (`DesignRulesChecker.java:160`). Java's choice of representative is hash-ordered
    // and drops 0-3; the port's is ascending-id and drops exactly 3 (plan-5 ruling 3, quirk #146).
    assert_eq!(
        (report.unconnected_items.len(), track_dangling, via_dangling),
        (44, 108, 4)
    );
    assert_eq!(report.violations.len(), 112);
}

// ---------------------------------------------------------------------------------------------
// fixtures/KiCadDrcViolationRoutingTest.java (66 loc) — the numeric oracle
// ---------------------------------------------------------------------------------------------

/// Port of all three `@Test`s of `KiCadDrcViolationRoutingTest`
/// (`KiCadDrcViolationRoutingTest.java:51-65`), through the same
/// `assertDrcOnLoadedBoard(filename, expectedUnconnected, expectedViolations)` shape (`:14-39`).
///
/// The Java helper builds `new BoardStatistics(board)` and reads
/// `stats.connections.incompleteCount` and `stats.clearanceViolations.totalCount`. Plan-5 ruling 5
/// leaves `BoardStatistics` itself to Plan 8, so the port reads the same two numbers from the two
/// pieces of it this crate owns: `DesignRulesChecker::get_incomplete_count`
/// (`BoardStatistics.java`'s connections block calls exactly that) and
/// `BoardStatisticsClearanceViolations::from_violations` over `get_all_clearance_violations()`
/// (`BoardStatistics.java:338-367`).
///
/// **All six numbers re-measured on the clone's HEAD jar** with `new BoardStatistics(board)`
/// itself, JDK 25, `-Djava.awt.headless=true -Duser.language=en -Duser.country=US
/// -XX:hashCode=2`:
///
/// ```text
/// Issue575-drc_BBD_Mars-64_6_track_1_hole_clearance_violations.dsn incompleteCount=3   totalCount=76
/// Issue575-drc_dev-board_4_hole_clearance_violations.dsn           incompleteCount=9   totalCount=2
/// Issue575-drc_Natural_Tone_Preamp_7_unconnected_items.dsn         incompleteCount=145 totalCount=0
/// ```
///
/// The third row is in the Java suite (`:62-65`) and was omitted by the port survey; it is pinned
/// here with the other two.
#[test]
fn the_board_statistics_oracle() {
    if !parity::require_java_dir() {
        return;
    }
    // `(fixture, expectedUnconnected, expectedViolations)`, in the Java file's order.
    for (fixture, expected_unconnected, expected_violations) in [
        (BBD_MARS_64, 3, 76),
        (DEV_BOARD, 9, 2),
        (NATURAL_TONE_PREAMP, 145, 0),
    ] {
        let (mut board, _) = fixture_board(fixture);
        let factor = Unit::scale(1.0, board.communication.unit, Unit::Um)
            / f64::from(board.communication.resolution);
        let mut drc = DesignRulesChecker::new(&mut board);

        // `:31-34` — `stats.connections.incompleteCount`.
        assert_eq!(
            drc.get_incomplete_count(),
            expected_unconnected,
            "mismatch in unconnected items for {fixture}"
        );

        // `:35-38` — `stats.clearanceViolations.totalCount`.
        let violations = drc.get_all_clearance_violations();
        let stats = BoardStatisticsClearanceViolations::from_violations(&violations, factor);
        assert_eq!(
            stats.total_count,
            Some(expected_violations),
            "mismatch in clearance violations for {fixture}"
        );
    }
}

/// The other half of ruling 11's table: the same three boards' **report** arrays, which are longer
/// than [`the_board_statistics_oracle`]'s counters and measure something else. Committed so that
/// the two families are asserted against one board in one binary and nobody reconciles them by
/// changing a number.
///
/// Natural Tone Preamp's `violations` is the port's 112, not the JVM's 114-115 — the one number in
/// this file that is deliberately not the jar's, per plan-5 rulings 3 and S and quirk #146.
#[test]
fn the_report_arrays_are_longer_than_the_statistics_counters() {
    if !parity::require_java_dir() {
        return;
    }
    // `(fixture, violations, unconnectedItems, holeClearance, clearance, track_dangling,
    //   via_dangling)`.
    for (fixture, violations, unconnected, hole, clearance, track, via) in [
        (DEV_BOARD, 10, 4, 2, 0, 8, 0),
        (BBD_MARS_64, 96, 3, 64, 12, 2, 18),
        (NATURAL_TONE_PREAMP, 112, 44, 0, 0, 108, 4),
    ] {
        let (mut board, transform) = fixture_board(fixture);
        let coords = coords(&board, transform);
        let report =
            DesignRulesChecker::new(&mut board).generate_report(&coords, &options(fixture));
        let of = |kind: &str| report.violations.iter().filter(|v| v.kind == kind).count();

        assert_eq!(report.violations.len(), violations, "violations {fixture}");
        assert_eq!(
            report.unconnected_items.len(),
            unconnected,
            "unconnectedItems {fixture}"
        );
        assert_eq!(
            (
                of("holeClearance"),
                of("clearance"),
                of("track_dangling"),
                of("via_dangling")
            ),
            (hole, clearance, track, via),
            "breakdown {fixture}"
        );
        // The reconciliation itself: `violations` is the clearance list plus the two dangling
        // kinds, and nothing else (`DesignRulesChecker.java:231-233`, `:271-276`).
        assert_eq!(hole + clearance + track + via, violations, "sum {fixture}");
    }
}
