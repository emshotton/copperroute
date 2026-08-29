//! Plan 5 Task 6: `DesignRulesChecker.calculateAllIncompletes` (`DesignRulesChecker.java:542-623`),
//! the eight lazy accessors that hang off it (`:630-815`) and `BoardStatistics`' clearance
//! statistics block (`BoardStatistics.java:338-367`) as
//! [`BoardStatisticsClearanceViolations::from_violations`].
//!
//! # What is compared to the JVM
//!
//! Everything here. Plan-5 ruling 4 splits `NetIncompletes` into a hash-dependent half (the
//! airline endpoints) and a hash-independent one (the counts); Task 6 ports only counters, so
//! **all** of it is parity. `tests/data/IncompletesProbe.java` measures `maxConnections`,
//! `getIncompleteCount()`, `getAllAirlines().length`, `getLengthViolationCount()`,
//! `recalculateLengthViolations()`, the per-net `getIncompleteCount(int)`/`getLengthViolation(int)`
//! and the clearance block on the HEAD jar; the transcripts are committed as
//! `tests/data/*.incompletes.txt` and were re-verified under
//! `-XX:+UnlockExperimentalVMOptions -XX:hashCode=0..4` (one digest per fixture over all five
//! modes plus the default). See `tests/data/README.md` for the recorded command.
//!
//! The three fixture totals also reproduce `KiCadDrcViolationRoutingTest`'s
//! `connections.incompleteCount`/`clearanceViolations.totalCount` assertions (9/2, 3/76, 145/0)
//! and `RatsnestClearanceHeadlessTest.java:52-75`'s "per-net counts must sum to total".

//! Every test that reads a fixture opens with `parity::require_java_dir()`: the fixtures live in
//! the sibling Java checkout, which is not vendored, and `tests/parity`'s contract is that such a
//! suite skips loudly rather than panicking on a missing file.

use fr_board::prelude::*;
use fr_drc::{BoardStatisticsClearanceViolations, DesignRulesChecker};
use fr_dsn::{BoardReadResult, DsnReadOptions};
use fr_geometry::{Area, IntBox, IntPoint, IntVector, Point, Polyline, Shape, TileShape};

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

/// A real board, read the way `RoutingFixtureTest` reads one; see `tests/net_incompletes.rs`.
fn fixture_board(name: &str) -> Board {
    let path = parity::fixture(name);
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("cannot read fixture {}: {e}", path.display()));
    match fr_dsn::read_board(&bytes[..], None, Some(name), &DsnReadOptions::default()) {
        BoardReadResult::Success { board, .. } | BoardReadResult::OutlineMissing { board, .. } => {
            *board.unwrap_or_else(|| panic!("{name} produced no board"))
        }
        other => panic!("{name} did not read: {other:?}"),
    }
}

/// The four counters `RatsnestClearanceHeadlessTest.java:52-75` asserts, in its order:
/// `maxConnections`, `getIncompleteCount()`, `getAllAirlines().length` and the sum of
/// `getIncompleteCount(netNumber)` over `1..=maxNetNumber`.
fn counters(board: &mut Board) -> (i32, usize, usize, usize) {
    let max_net_number = board.rules.nets.max_net_number();
    let mut drc = DesignRulesChecker::new(board);
    // `RatsnestClearanceHeadlessTest.java:55` calls this explicitly; every accessor below would
    // have done it lazily anyway (`lazy_initialisation_matches_java`).
    drc.calculate_all_incompletes();
    let per_net = (1..=max_net_number)
        .map(|n| drc.get_incomplete_count_for_net(n))
        .sum();
    (
        drc.max_connections(),
        drc.get_incomplete_count(),
        drc.get_all_airlines().len(),
        per_net,
    )
}

#[test]
fn dev_board_counts() {
    if !parity::require_java_dir() {
        return;
    }
    // `IncompletesProbe` on the HEAD jar; also `RatsnestClearanceHeadlessTest`'s
    // `EXPECTED_UNCONNECTED = 9` and `KiCadDrcViolationRoutingTest.issue5754HoleClearanceViolations`.
    let mut board = fixture_board("Issue575-drc_dev-board_4_hole_clearance_violations.dsn");
    assert_eq!(counters(&mut board), (96, 9, 9, 9));
}

#[test]
fn bbd_mars_64_counts() {
    if !parity::require_java_dir() {
        return;
    }
    // `KiCadDrcViolationRoutingTest.issue5756TrackAnd1HoleClearanceViolations` asserts the 3.
    let mut board =
        fixture_board("Issue575-drc_BBD_Mars-64_6_track_1_hole_clearance_violations.dsn");
    assert_eq!(counters(&mut board), (106, 3, 3, 3));
}

#[test]
fn natural_tone_preamp_counts() {
    if !parity::require_java_dir() {
        return;
    }
    // `KiCadDrcViolationRoutingTest.issue5757UnconnectedItems` asserts the 145.
    let mut board = fixture_board("Issue575-drc_Natural_Tone_Preamp_7_unconnected_items.dsn");
    assert_eq!(counters(&mut board), (218, 145, 145, 145));
}

#[test]
fn empty_board_has_no_incompletes() {
    if !parity::require_java_dir() {
        return;
    }
    // `RatsnestClearanceHeadlessTest.emptyBoardHasNoIncompletesAndNoViolations` (`:117-121`).
    let mut board = fixture_board("empty_board.dsn");
    assert_eq!(counters(&mut board), (0, 0, 0, 0));
    let mut drc = DesignRulesChecker::new(&mut board);
    assert!(drc.get_all_clearance_violations().is_empty());
}

// ---------------------------------------------------------------------------------------------
// The JVM golden
// ---------------------------------------------------------------------------------------------

const FIXTURES: [&str; 4] = [
    "Issue575-drc_dev-board_4_hole_clearance_violations",
    "Issue575-drc_BBD_Mars-64_6_track_1_hole_clearance_violations",
    "Issue575-drc_Natural_Tone_Preamp_7_unconnected_items",
    "empty_board",
];

/// Renders the port's counters in `IncompletesProbe`'s transcript format.
fn transcript(board: &mut Board) -> String {
    let max_net_number = board.rules.nets.max_net_number();
    // `BoardStatistics.java:200-202`, the factor the clearance block multiplies by.
    let communication = board.communication.clone();
    let board_unit_to_um_factor = Unit::scale(1.0, communication.unit, Unit::Um)
        / f64::from(if communication.resolution > 0 {
            communication.resolution
        } else {
            1
        });

    let mut drc = DesignRulesChecker::new(board);
    drc.calculate_all_incompletes();

    let mut out = String::new();
    out.push_str(&format!("maxConnections {}\n", drc.max_connections()));
    out.push_str(&format!("incompleteCount {}\n", drc.get_incomplete_count()));
    out.push_str(&format!("airlines {}\n", drc.get_all_airlines().len()));
    out.push_str(&format!(
        "lengthViolationCount {}\n",
        drc.get_length_violation_count()
    ));
    out.push_str(&format!(
        "recalculateLengthViolations {}\n",
        drc.recalculate_length_violations()
    ));

    let mut per_net = String::new();
    let mut sum = 0usize;
    for net_number in 1..=max_net_number {
        let count = drc.get_incomplete_count_for_net(net_number);
        sum += count;
        per_net.push_str(&format!(
            "net={net_number} incompleteCount={count} lengthViolation={}\n",
            fr_dsn::java_double_to_string(drc.get_length_violation(net_number)),
        ));
    }
    out.push_str(&format!("perNetIncompleteSum {sum}\n"));
    out.push_str(&per_net);
    out.push_str(&format!(
        "boardUnitToUmFactor {}\n",
        fr_dsn::java_double_to_string(board_unit_to_um_factor),
    ));

    let violations = drc.get_all_clearance_violations();
    let stats =
        BoardStatisticsClearanceViolations::from_violations(&violations, board_unit_to_um_factor);
    let d = |v: Option<f64>| fr_dsn::java_double_to_string(v.expect("the block is never partial"));
    out.push_str(&format!(
        "clearanceViolations totalCount={} min={} max={} avg={}\n",
        stats.total_count.expect("the block is never partial"),
        d(stats.min_violation_um),
        d(stats.max_violation_um),
        d(stats.avg_violation_um),
    ));
    out
}

#[test]
fn the_four_fixtures_match_the_jvm() {
    if !parity::require_java_dir() {
        return;
    }
    for stem in FIXTURES {
        let mut board = fixture_board(&format!("{stem}.dsn"));
        let expected = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/data")
                .join(format!("{stem}.incompletes.txt")),
        )
        .expect("the JVM transcript is committed next to the probe");
        let actual = transcript(&mut board);
        for (i, (a, e)) in actual.lines().zip(expected.lines()).enumerate() {
            assert_eq!(a, e, "{stem}: transcript line {}", i + 1);
        }
        assert_eq!(
            actual.lines().count(),
            expected.lines().count(),
            "{stem}: transcript line count",
        );
    }
}

// ---------------------------------------------------------------------------------------------
// calculateAllIncompletes, on synthetic boards
// ---------------------------------------------------------------------------------------------

#[test]
fn a_multi_net_item_lands_in_every_net_list() {
    // `DesignRulesChecker.java:556-561`: the loop is over `netCount()`, not over "the first net",
    // so a pin on two nets is appended to **both** lists and counted twice by `maxConnections`.
    let mut board = two_net_pin_board();

    let max_connections = {
        let mut drc = DesignRulesChecker::new(&mut board);
        drc.calculate_all_incompletes();
        // Net 1 holds pins 2 and 3, net 2 holds pins 3 and 4 — the shared pin 3 is in both.
        // `(2 - 1) + (2 - 1) = 2`. Were pin 3 filed under its first net only, net 2 would hold
        // one pin and contribute 0.
        assert_eq!(drc.get_incomplete_count_for_net(1), 1);
        assert_eq!(drc.get_incomplete_count_for_net(2), 1);
        drc.max_connections()
    };
    assert_eq!(max_connections, 2);

    // The same item, seen from the ratsnest: it is an endpoint of both nets' airlines.
    let mut drc = DesignRulesChecker::new(&mut board);
    let airlines = drc.get_all_airlines();
    assert_eq!(airlines.len(), 2);
    // `getAllAirlines` flattens in ascending net number (`:787-793`).
    assert_eq!(
        airlines.iter().map(|a| a.net_number).collect::<Vec<_>>(),
        vec![1, 2],
    );
    for airline in &airlines {
        assert!(
            [airline.from_item, airline.to_item].contains(&ItemId(4)),
            "the two-net pin is an endpoint of both airlines",
        );
    }
}

#[test]
fn max_connections_counts_only_pins_and_conduction_areas() {
    // `:573-575`: the endpoint filter is `item instanceof Pin || item instanceof ConductionArea`.
    // A net of three traces and nothing else has three items and **zero** endpoints, so
    // `max(0, 0 - 1) = 0` — the `Math.max` is what keeps it from being -1.
    let mut board = three_trace_net_board();
    let mut drc = DesignRulesChecker::new(&mut board);
    drc.calculate_all_incompletes();
    assert_eq!(drc.max_connections(), 0);

    // The same board with a conduction area added to net 1: one endpoint, still `max(0, 0) = 0`.
    let mut board = three_trace_net_board();
    insert_conduction_area(&mut board);
    let mut drc = DesignRulesChecker::new(&mut board);
    drc.calculate_all_incompletes();
    assert_eq!(drc.max_connections(), 0);

    // And with a pin as well: two endpoints, one connection.
    let mut board = three_trace_net_board();
    insert_conduction_area(&mut board);
    board.insert_pin(1, 0, vec![1], 1, FixedState::Unfixed);
    let mut drc = DesignRulesChecker::new(&mut board);
    drc.calculate_all_incompletes();
    assert_eq!(drc.max_connections(), 1);
}

#[test]
fn an_empty_net_contributes_nothing() {
    // `:569-571`: the `filter(list -> !list.isEmpty())` is what the Java comment at `:564-567`
    // is about — the old formula counted empty nets in the denominator. Net 2 of this board has
    // no items at all and must not push the sum down.
    let mut board = three_trace_net_board();
    board.insert_pin(1, 0, vec![1], 1, FixedState::Unfixed);
    board.insert_pin(1, 1, vec![1], 1, FixedState::Unfixed);
    assert_eq!(
        board.rules.nets.max_net_number(),
        2,
        "net 2 exists and is empty"
    );
    let mut drc = DesignRulesChecker::new(&mut board);
    drc.calculate_all_incompletes();
    assert_eq!(drc.max_connections(), 1);
}

#[test]
fn lazy_initialisation_matches_java() {
    if !parity::require_java_dir() {
        return;
    }
    // Every one of the eight accessors opens `if (netIncompletes == null) calculateAllIncompletes();`
    // (`:664-666`, `:709-711`, `:737-739`, `:751-753`, `:766-769`, `:781-783`, `:801-803`), so a
    // fresh checker answers the same as one that was initialised by hand.
    let mut board = fixture_board("Issue575-drc_dev-board_4_hole_clearance_violations.dsn");
    let mut drc = DesignRulesChecker::new(&mut board);

    // `maxConnections` is **not** lazy: it is a plain field Java leaves at the `int` default
    // until `calculateAllIncompletes` writes it (`:567`), which is why `BoardStatistics.java:269`
    // calls that method before reading it.
    assert_eq!(drc.max_connections(), 0);
    assert_eq!(drc.get_incomplete_count(), 9);
    assert_eq!(
        drc.max_connections(),
        96,
        "the accessor ran the initialiser"
    );

    // Each of the others, on its own fresh checker.
    let mut drc = DesignRulesChecker::new(&mut board);
    assert_eq!(drc.get_incomplete_count_for_net(1), 6);
    let mut drc = DesignRulesChecker::new(&mut board);
    assert_eq!(drc.get_all_airlines().len(), 9);
    let mut drc = DesignRulesChecker::new(&mut board);
    assert_eq!(drc.get_length_violation_count(), 0);
    let mut drc = DesignRulesChecker::new(&mut board);
    assert_eq!(drc.get_length_violation(1), 0.0);
    let mut drc = DesignRulesChecker::new(&mut board);
    assert!(
        drc.recalculate_length_violations(),
        "`:768` returns true when it had to initialise — \"changed from nothing to something\"",
    );
    assert!(
        !drc.recalculate_length_violations(),
        "and false on the second call, where nothing changed",
    );
    let mut drc = DesignRulesChecker::new(&mut board);
    assert_eq!(drc.get_net_incompletes(1).map(|ni| ni.count()), Some(6));
}

#[test]
fn out_of_range_net_numbers_answer_the_java_defaults() {
    if !parity::require_java_dir() {
        return;
    }
    // `:712-714` returns 0, `:754-756` returns 0, `:804-806` returns null. Java's bound is the
    // **array length**, i.e. `maxNetNumber`, and 0 and negatives are rejected by the same test.
    let mut board = fixture_board("Issue575-drc_dev-board_4_hole_clearance_violations.dsn");
    let max_net_number = board.rules.nets.max_net_number();
    let mut drc = DesignRulesChecker::new(&mut board);
    for net_number in [-1, 0, max_net_number + 1] {
        assert_eq!(drc.get_incomplete_count_for_net(net_number), 0);
        assert_eq!(drc.get_length_violation(net_number), 0.0);
        assert!(drc.get_net_incompletes(net_number).is_none());
    }
    assert!(drc.get_net_incompletes(max_net_number).is_some());
}

#[test]
fn recalculating_one_net_replaces_only_that_net() {
    // `:630-643`: the one-argument overload re-reads the net's items from the board
    // (`getConnectableItems`), so removing a trace and recalculating turns one airline into two.
    let mut board = two_group_net_board();
    let mut drc = DesignRulesChecker::new(&mut board);
    assert_eq!(drc.get_incomplete_count(), 1);

    // The two-argument overload (`:647-660`) takes the caller's list instead. Handing it a
    // single item leaves the net with one `NetItem` and therefore no airline.
    drc.recalculate_net_incompletes_with(1, &[ItemId(2)]);
    assert_eq!(drc.get_incomplete_count_for_net(1), 0);

    // And the one-argument overload puts it back from the board.
    drc.recalculate_net_incompletes(1);
    assert_eq!(drc.get_incomplete_count_for_net(1), 1);

    // Out of range: both overloads leave the array alone (`:635`, `:654`).
    drc.recalculate_net_incompletes(99);
    drc.recalculate_net_incompletes_with(99, &[]);
    assert_eq!(drc.get_incomplete_count(), 1);
}

#[test]
fn recalculate_net_incompletes_initialises_and_returns() {
    // `:631-634`: on a null array the one-argument overload initialises **and returns**, so the
    // net it was asked about keeps `calculateAllIncompletes`' answer rather than a freshly
    // computed one. The two-argument overload has no such `return` (`:648-652`) and goes on to
    // overwrite the slot. Transcribed as written; on these inputs both answers agree.
    let mut board = two_group_net_board();
    let mut drc = DesignRulesChecker::new(&mut board);
    drc.recalculate_net_incompletes(1);
    assert_eq!(drc.get_incomplete_count(), 1);
    assert_eq!(drc.max_connections(), 1, "the initialiser ran");

    let mut drc = DesignRulesChecker::new(&mut board);
    drc.recalculate_net_incompletes_with(1, &[ItemId(2)]);
    assert_eq!(
        drc.get_incomplete_count_for_net(1),
        0,
        "no early return: the caller's list won",
    );
}

// ---------------------------------------------------------------------------------------------
// BoardStatisticsClearanceViolations (BoardStatistics.java:338-367)
// ---------------------------------------------------------------------------------------------

#[test]
fn statistics_block() {
    if !parity::require_java_dir() {
        return;
    }
    // The dev board's two violations are `expected=500.0 actual=0.0` each
    // (`tests/data/Issue575-drc_dev-board_4_hole_clearance_violations.list.txt`), and the board
    // is 0.1 um per board unit, so every one of min/max/avg is `500.0 * 0.1 = 50.0`.
    let mut board = fixture_board("Issue575-drc_dev-board_4_hole_clearance_violations.dsn");
    let factor = Unit::scale(1.0, board.communication.unit, Unit::Um)
        / f64::from(board.communication.resolution);
    assert_eq!(factor, 0.1);

    let violations = DesignRulesChecker::new(&mut board).get_all_clearance_violations();
    assert_eq!(violations.len(), 2);
    let stats = BoardStatisticsClearanceViolations::from_violations(&violations, factor);
    assert_eq!(stats.total_count, Some(2));
    assert_eq!(stats.min_violation_um, Some(50.0));
    assert_eq!(stats.max_violation_um, Some(50.0));
    assert_eq!(stats.avg_violation_um, Some(50.0));
}

#[test]
fn an_empty_violation_list_is_four_zeroes_not_four_nulls() {
    // `BoardStatistics.java:357-361` — the `violationsList.isEmpty()` arm, **not** the
    // superficially identical `:362-367`, which is the `includeClearanceViolations == false` one.
    // It writes `0.0` into all three doubles rather than leaving them null, so Gson emits them.
    // `Default` — Java's uninitialised `Integer` and `Double` fields — is the *other* state, and
    // is what `BoardStatistics` holds before the block runs.
    let stats = BoardStatisticsClearanceViolations::from_violations(&[], 0.1);
    assert_eq!(stats.total_count, Some(0));
    assert_eq!(stats.min_violation_um, Some(0.0));
    assert_eq!(stats.max_violation_um, Some(0.0));
    assert_eq!(stats.avg_violation_um, Some(0.0));
    assert_eq!(
        BoardStatisticsClearanceViolations::default().total_count,
        None
    );
}

#[test]
fn a_negative_shortfall_is_clamped_but_still_averaged_over_every_violation() {
    // `:348` is `Math.max(0.0, expected - actual)`, and `:357` divides by
    // `violationsList.size()` — **not** by the number of positive shortfalls. So a violation
    // that is not actually short pulls the average down and sets the minimum to 0.
    let violation = |expected: f64, actual: f64| ClearanceViolation {
        first_item: ItemId(2),
        second_item: ItemId(3),
        shape: TileShape::Box(IntBox::from_coords(0, 0, 10, 10)),
        layer: 0,
        expected_clearance: expected,
        actual_clearance: actual,
    };
    let synthetic = vec![violation(100.0, 0.0), violation(100.0, 400.0)];
    let stats = BoardStatisticsClearanceViolations::from_violations(&synthetic, 2.0);
    assert_eq!(stats.total_count, Some(2));
    assert_eq!(stats.min_violation_um, Some(0.0), "the clamped one");
    assert_eq!(stats.max_violation_um, Some(200.0));
    assert_eq!(stats.avg_violation_um, Some(100.0), "200 / 2, not 200 / 1");
}

#[test]
fn a_default_block_serialises_to_nothing() {
    // The doc's "Gson omits a null field" claim, made testable: `Default` is Java's uninitialised
    // state — four boxed nulls — and `skip_serializing_if = "Option::is_none"` is what reproduces
    // Gson's omission of them. `from_violations` never produces this state, but `BoardStatistics`
    // holds it before its clearance block runs (`BoardStatistics.java:338`).
    assert_eq!(
        serde_json::to_string(&BoardStatisticsClearanceViolations::default()).unwrap(),
        "{}",
    );
}

#[test]
fn the_serialised_keys_are_gsons() {
    // `BoardStatisticsClearanceViolations.java:9-19`: four `@SerializedName`s, all snake_case.
    // Unlike the DRC report (plan-5 ruling 1) this class has no camelCase drift.
    let stats = BoardStatisticsClearanceViolations::from_violations(&[], 1.0);
    assert_eq!(
        serde_json::to_string(&stats).unwrap(),
        r#"{"total_count":0,"min_violation_um":0.0,"max_violation_um":0.0,"avg_violation_um":0.0}"#,
    );
}

// ---------------------------------------------------------------------------------------------
// The synthetic boards
// ---------------------------------------------------------------------------------------------

const BOUNDING_BOX: IntBox = IntBox {
    ll: IntPoint {
        x: -10_000,
        y: -10_000,
    },
    ur: IntPoint {
        x: 10_000,
        y: 10_000,
    },
};

fn layers() -> LayerStructure {
    LayerStructure::new(vec![Layer::new("front", true)])
}

/// The `tests/net_incompletes.rs` fixture builder: a one-layer board with one component whose
/// pins sit at the given offsets, two nets (`N1` = 1, `A0` = 2) and a spare via padstack.
///
/// **Item 1 is the board outline**, so the caller's first insertion is item 2.
fn bare_board(pin_offsets: &[i32]) -> Board {
    let mut padstacks = Padstacks::new(layers());
    let mut pins = Vec::new();
    for (i, offset) in pin_offsets.iter().enumerate() {
        let shape = Shape::Tile(TileShape::Box(IntBox::from_coords(-50, -50, 50, 50)));
        let name = format!("p{i}");
        let padstack = padstacks.add(&name, vec![Some(shape)], false, false);
        pins.push(PackagePin::new(
            &name,
            padstack,
            IntVector::new(*offset, 0).into(),
            0.0,
        ));
    }

    let mut packages = Packages::new();
    let package = packages.add(
        "pkg",
        pins,
        None,
        None,
        None,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        true,
    );
    let mut components = Components::new();
    components.add_with_generated_name(Some(Point::new(0, 0)), 0.0, true, package);

    let ls = layers();
    let matrix = ClearanceMatrix::get_default_instance(&ls, 200);
    let mut rules = BoardRules::new(ls, matrix);
    rules.create_default_net_class();
    let default_class = rules.get_default_net_class();
    let mut board = Board::new(
        Vec::new(),
        1,
        BOUNDING_BOX,
        rules,
        BoardLibrary::new(padstacks, packages),
        components,
        Communication::default(),
    );
    board.rules.nets.add("N1", 1, false, default_class);
    board.rules.nets.add("A0", 1, false, default_class);
    board
}

fn insert_trace(board: &mut Board, from: (i32, i32), to: (i32, i32), net: i32) -> ItemId {
    board
        .insert_trace_without_cleaning(
            Polyline::from_points(&[Point::new(from.0, from.1), Point::new(to.0, to.1)]),
            0,
            30,
            vec![net],
            1,
            FixedState::Unfixed,
        )
        .expect("the synthetic trace is neither degenerate nor closed")
}

/// Three pins, far enough apart that each is its own connected group: item 2 on net 1 alone,
/// item 3 on net 2 alone, and item 4 on **both** nets. So `calculateAllIncompletes` files item 4
/// twice — net 1's list is `[2, 4]` and net 2's is `[3, 4]`, two pins each — and every net
/// contributes `max(0, 2 - 1) = 1` to `maxConnections` and one airline.
fn two_net_pin_board() -> Board {
    let mut board = bare_board(&[0, 4000, 8000]);
    board.insert_pin(1, 0, vec![1], 1, FixedState::Unfixed); // item 2, net 1
    board.insert_pin(1, 2, vec![2], 1, FixedState::Unfixed); // item 3, net 2
    board.insert_pin(1, 1, vec![1, 2], 1, FixedState::Unfixed); // item 4, both nets
    board
}

/// Net 1 as three mutually disjoint traces and nothing else: three items, zero endpoints.
fn three_trace_net_board() -> Board {
    let mut board = bare_board(&[0, 5000]);
    insert_trace(&mut board, (0, 1000), (1000, 1000), 1);
    insert_trace(&mut board, (3000, 1000), (4000, 1000), 1);
    insert_trace(&mut board, (6000, 1000), (7000, 1000), 1);
    board
}

/// Net 1 as two far-apart pins (items 2 and 3), so one airline, plus a lone pin of net 2
/// (item 4), which contributes `max(0, 1 - 1) = 0`. `maxConnections` is therefore 1.
fn two_group_net_board() -> Board {
    let mut board = bare_board(&[0, 8000, 4000]);
    board.insert_pin(1, 0, vec![1], 1, FixedState::Unfixed); // item 2
    board.insert_pin(1, 1, vec![1], 1, FixedState::Unfixed); // item 3
    board.insert_pin(1, 2, vec![2], 1, FixedState::Unfixed); // item 4
    board
}

/// A conduction area on net 1, off to one side of [`three_trace_net_board`]'s traces.
fn insert_conduction_area(board: &mut Board) {
    board.insert_conduction_area(
        Area::Shape(Shape::Tile(TileShape::Box(IntBox::from_coords(
            6000, 6000, 7000, 7000,
        )))),
        0,
        vec![1],
        1,
        false,
        FixedState::Unfixed,
    );
}
