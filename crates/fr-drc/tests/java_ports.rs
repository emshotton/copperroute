//! |---|---|---|
mod common;
use common::JAR_VERSION;

use fr_board::prelude::*;
use fr_drc::report::{DrcCoordinates, DrcReportOptions};
use fr_drc::{
    BoardStatisticsClearanceViolations, DesignRulesChecker, DrcJsonFlavor, UnconnectedKind,
};
use fr_dsn::{BoardReadResult, CoordinateTransform, DsnReadOptions};


const ISSUE555_BBD_MARS_64: &str = "Issue555-BBD_Mars-64.dsn";
const DEV_BOARD: &str = "Issue575-drc_dev-board_4_hole_clearance_violations.dsn";
const BBD_MARS_64: &str = "Issue575-drc_BBD_Mars-64_6_track_1_hole_clearance_violations.dsn";
const NATURAL_TONE_PREAMP: &str = "Issue575-drc_Natural_Tone_Preamp_7_unconnected_items.dsn";
const EMPTY_BOARD: &str = "empty_board.dsn";

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

fn coords(board: &Board, transform: CoordinateTransform) -> DrcCoordinates {
    DrcCoordinates {
        board_unit: board.communication.unit,
        transform,
    }
}

fn options(source: &str) -> DrcReportOptions {
    DrcReportOptions {
        source: source.to_string(),
        coordinate_unit: "mm".to_string(),
        date: "2026-08-29T00:00:00Z".to_string(),
        freerouting_version: JAR_VERSION.to_string(),
        quality_score: None,
    }
}


#[test]
fn report_structure_on_issue555_bbd_mars_64() {
    if !parity::require_java_dir() {
        return;
    }
    let (mut board, transform) = fixture_board(ISSUE555_BBD_MARS_64);
    let coords = coords(&board, transform);
    let report = DesignRulesChecker::new(&mut board).generate_report(&coords, &options("test.dsn"));

    assert_eq!(report.json_schema, "https://schemas.kicad.org/drc.v1.json");
    assert_eq!(report.coordinate_units, "mm");
    assert_eq!(report.source, "test.dsn");
    assert!(
        report.freerouting_version.contains("Freerouting"),
        "version should contain Freerouting, was {:?}",
        report.freerouting_version
    );
}

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
            DrcJsonFlavor::FreeroutingHead,
        )
        .expect("the report serialises");

    assert!(!text.is_empty());
    let json: serde_json::Value = serde_json::from_str(&text).expect("valid JSON");
    let object = json.as_object().expect("a JSON object");

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


#[test]
fn coordinates_are_in_a_plausible_mm_range() {
    if !parity::require_java_dir() {
        return;
    }
    let (mut board, transform) = fixture_board(NATURAL_TONE_PREAMP);
    let coords = coords(&board, transform);
    let text = DesignRulesChecker::new(&mut board)
        .report_to_json(
            &coords,
            &options("test.dsn"),
            DrcJsonFlavor::FreeroutingHead,
        )
        .expect("the report serialises");
    let json: serde_json::Value = serde_json::from_str(&text).expect("valid JSON");

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

    assert!(x.abs() < 500.0, "X should be less than 500mm, but was {x}");
    assert!(y.abs() < 500.0, "Y should be less than 500mm, but was {y}");
    assert!(x.abs() > 10.0, "X should be greater than 10mm, but was {x}");
    assert!(y.abs() > 10.0, "Y should be greater than 10mm, but was {y}");
}


const EXPECTED_UNCONNECTED: usize = 9;
const EXPECTED_UNIQUE_VIOLATIONS: usize = 2;

#[test]
fn incompletes_are_computable_via_the_checker_alone() {
    if !parity::require_java_dir() {
        return;
    }
    let (mut board, _) = fixture_board(DEV_BOARD);
    let max_net_number = board.rules.nets.max_net_number();
    let mut drc = DesignRulesChecker::new(&mut board);
    drc.calculate_all_incompletes();

    assert_eq!(drc.get_incomplete_count(), EXPECTED_UNCONNECTED);

    let airlines = drc.get_all_airlines();
    assert!(!airlines.is_empty());
    assert_eq!(airlines.len(), EXPECTED_UNCONNECTED);

    let sum_per_net: usize = (1..=max_net_number)
        .map(|n| drc.get_incomplete_count_for_net(n))
        .sum();
    assert_eq!(sum_per_net, EXPECTED_UNCONNECTED);
}

#[test]
fn clearance_violations_are_computable_via_the_checker_alone() {
    if !parity::require_java_dir() {
        return;
    }
    let (mut board, _) = fixture_board(DEV_BOARD);
    let deduped = DesignRulesChecker::new(&mut board).get_all_clearance_violations();
    assert_eq!(deduped.len(), EXPECTED_UNIQUE_VIOLATIONS);
}

#[test]
fn aggregation_is_headless_and_severity_sorted() {
    if !parity::require_java_dir() {
        return;
    }
    let (mut board, _) = fixture_board(DEV_BOARD);

    let aggregated = board.aggregate_violations_sorted_by_severity();
    assert!(!aggregated.is_empty());
    assert!(aggregated.len() >= EXPECTED_UNIQUE_VIOLATIONS);

    for pair in aggregated.windows(2) {
        let previous = pair[0].expected_clearance - pair[0].actual_clearance;
        let current = pair[1].expected_clearance - pair[1].actual_clearance;
        assert!(
            previous >= current,
            "aggregated violations must be sorted by severity, descending"
        );
    }

    let smallest = board.smallest_clearance();
    assert!(
        (0.0..f64::MAX).contains(&smallest),
        "smallestClearance must be non-negative and finite, was {smallest}"
    );
}

#[test]
fn empty_board_has_no_incompletes_and_no_violations() {
    if !parity::require_java_dir() {
        return;
    }
    let (mut board, _) = fixture_board(EMPTY_BOARD);
    {
        let mut drc = DesignRulesChecker::new(&mut board);
        assert_eq!(drc.get_incomplete_count(), 0);
        assert!(drc.get_all_clearance_violations().is_empty());
    }
    assert!(board.aggregate_violations_sorted_by_severity().is_empty());
}


const EXPECTED_UNCONNECTED_NET_GROUPS: usize = 9;
const EXPECTED_DANGLING_TRACKS: usize = 24;
const EXPECTED_DANGLING_VIAS: usize = 4;

#[test]
fn issue575_drc_reproduction_single_pass() {
    if !parity::require_java_dir() {
        return;
    }
    let (mut board, transform) = fixture_board(NATURAL_TONE_PREAMP);
    let coords = coords(&board, transform);

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

    assert!(
        board.is_tail(trace_2402),
        "trace 2402 (GND) is expected to be a dangling trace"
    );

    let pin_321 = ItemId(321);
    if board.get_item(pin_321).is_some() {
        assert!(
            !board.normal_contacts(trace_2402).contains(&pin_321),
            "trace 2402 correctly has NO connection to pin 321 in this board state"
        );
    }

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

    let entries = DesignRulesChecker::new(&mut board).get_all_unconnected_items();
    let count = |kind| entries.iter().filter(|e| e.kind == kind).count();
    let dangling_tracks = count(UnconnectedKind::TrackDangling);
    let dangling_vias = count(UnconnectedKind::ViaDangling);
    let net_groups = count(UnconnectedKind::UnconnectedItems);

    assert!(dangling_tracks >= EXPECTED_DANGLING_TRACKS);
    assert!(dangling_vias >= EXPECTED_DANGLING_VIAS);
    assert!(net_groups >= EXPECTED_UNCONNECTED_NET_GROUPS);
    assert_eq!((net_groups, dangling_tracks, dangling_vias), (44, 108, 4));

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

    let report =
        DesignRulesChecker::new(&mut board).generate_report(&coords, &options(NATURAL_TONE_PREAMP));
    let violations_of = |kind: &str| report.violations.iter().filter(|v| v.kind == kind).count();
    let track_dangling = violations_of("track_dangling");
    let via_dangling = violations_of("via_dangling");

    assert!(track_dangling >= EXPECTED_DANGLING_TRACKS);
    assert!(via_dangling >= EXPECTED_DANGLING_VIAS);
    assert!(report.unconnected_items.len() >= EXPECTED_UNCONNECTED_NET_GROUPS);
    assert_eq!(
        (report.unconnected_items.len(), track_dangling, via_dangling),
        (44, 108, 4)
    );
    assert_eq!(report.violations.len(), 112);
}


#[test]
fn the_board_statistics_oracle() {
    if !parity::require_java_dir() {
        return;
    }
    for (fixture, expected_unconnected, expected_violations) in [
        (BBD_MARS_64, 3, 76),
        (DEV_BOARD, 9, 2),
        (NATURAL_TONE_PREAMP, 145, 0),
    ] {
        let (mut board, _) = fixture_board(fixture);
        let factor = Unit::scale(1.0, board.communication.unit, Unit::Um)
            / f64::from(board.communication.resolution);
        let mut drc = DesignRulesChecker::new(&mut board);

        assert_eq!(
            drc.get_incomplete_count(),
            expected_unconnected,
            "mismatch in unconnected items for {fixture}"
        );

        let violations = drc.get_all_clearance_violations();
        let stats = BoardStatisticsClearanceViolations::from_violations(&violations, factor);
        assert_eq!(
            stats.total_count,
            Some(expected_violations),
            "mismatch in clearance violations for {fixture}"
        );
    }
}

#[test]
fn the_report_arrays_are_longer_than_the_statistics_counters() {
    if !parity::require_java_dir() {
        return;
    }
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
        assert_eq!(hole + clearance + track + via, violations, "sum {fixture}");
    }
}
