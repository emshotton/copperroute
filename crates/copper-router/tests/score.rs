use copper_board::prelude::*;
use copper_board::structure::{FixedState, Unit};
use copper_drc::{DesignRulesChecker, DrcViolation};
use copper_dsn::{BoardReadResult, DsnReadOptions};
use copper_router::route_connection;
use copper_router::score::BoardStatistics;
use copper_settings::sources::DefaultSettings;
use copper_settings::{HostEnvironment, RouterSettings, ScoringSettings, SettingsSource};

use std::collections::{BTreeMap, BTreeSet};

fn load_board(rel_path: &str) -> Board {
    let path = testkit::corpus_dir().join(rel_path);
    let file = std::fs::File::open(&path)
        .unwrap_or_else(|e| panic!("cannot open {}: {e}", path.display()));
    let design_name = path
        .file_name()
        .expect("a file name")
        .to_string_lossy()
        .into_owned();
    match copper_dsn::read_board(file, None, Some(&design_name), &DsnReadOptions::default()) {
        BoardReadResult::Success { board, .. } | BoardReadResult::OutlineMissing { board, .. } => {
            *board.unwrap_or_else(|| panic!("{design_name} produced no board"))
        }
        other => panic!("{design_name} did not read: {other:?}"),
    }
}

fn load_test_board(rel_path: &str) -> Board {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(rel_path);
    let file = std::fs::File::open(&path)
        .unwrap_or_else(|e| panic!("cannot open {}: {e}", path.display()));
    let design_name = path
        .file_name()
        .expect("a file name")
        .to_string_lossy()
        .into_owned();
    match copper_dsn::read_board(file, None, Some(&design_name), &DsnReadOptions::default()) {
        BoardReadResult::Success { board, .. } | BoardReadResult::OutlineMissing { board, .. } => {
            *board.unwrap_or_else(|| panic!("{design_name} produced no board"))
        }
        other => panic!("{design_name} did not read: {other:?}"),
    }
}

fn build_settings(board: &Board) -> RouterSettings {
    let mut settings = DefaultSettings::new(&HostEnvironment::detect())
        .get_settings()
        .expect("DefaultSettings always answers a table")
        .clone();
    settings.set_layer_count(board.get_layer_count());
    settings.apply_board_specific_optimizations(board);
    settings
}

fn route_first(board: &mut Board, settings: &RouterSettings, max_items: usize) {
    if max_items == 0 {
        return;
    }
    let mut connections: Vec<(ItemId, i32)> = Vec::new();
    'outer: for item_id in board.items_in_board_order() {
        let Some(item) = board.get_item(item_id) else {
            continue;
        };
        if item.as_connectable().is_none() {
            continue;
        }
        for net_no in item.net_nos().to_vec() {
            if board.unconnected_set(item_id, net_no).is_empty() {
                continue;
            }
            connections.push((item_id, net_no));
            if connections.len() >= max_items {
                break 'outer;
            }
        }
    }
    let trace_costs = settings.get_trace_costs();
    for (item_id, net_no) in connections {
        if board.get_item(item_id).is_none() {
            continue;
        }
        board.start_marking_changed_area();
        let mut ripped: BTreeSet<ItemId> = BTreeSet::new();
        let mut ripup_costs: BTreeMap<ItemId, i32> = BTreeMap::new();
        let mut engine = None;
        route_connection(
            board,
            &mut engine,
            item_id,
            net_no,
            settings,
            &trace_costs,
            &mut ripped,
            &mut ripup_costs,
            1,
            settings.get_start_ripup_costs(),
            !settings.is_fanout_enabled(),
            false,
            &|| false,
        );
    }
}

fn board_of(rel_path: &str, route_k: usize) -> Board {
    let mut board = load_board(rel_path);
    let settings = build_settings(&board);
    route_first(&mut board, &settings, route_k);
    board
}

struct Row {
    dsn: &'static str,
    route_k: usize,
    items_total: i32,
    maximum_count: i32,
    incomplete_count: i32,
    trace_count: i32,
    segment_count: i32,
    total_length: f32,
    total_length_mm: f32,
    total_weighted_length: f32,
    bend_count: i32,
    via_count: i32,
    violation_count: i32,
    fanout: (i32, i32, i32),
    normalized_score: f32,
}

fn check(row: &Row) {
    let mut board = board_of(row.dsn, row.route_k);
    let settings = build_settings(&board);
    let scoring = settings
        .scoring
        .clone()
        .expect("DefaultSettings always writes a scoring block");
    let stats = BoardStatistics::new(&mut board);
    let what = format!("{} (routeK = {})", row.dsn, row.route_k);

    assert_eq!(
        stats.items.total_count,
        Some(row.items_total),
        "{what}: items.totalCount"
    );
    assert_eq!(
        stats.connections.maximum_count,
        Some(row.maximum_count),
        "{what}: connections.maximumCount"
    );
    assert_eq!(
        stats.connections.incomplete_count,
        Some(row.incomplete_count),
        "{what}: connections.incompleteCount"
    );
    assert_eq!(
        stats.traces.total_count,
        Some(row.trace_count),
        "{what}: traces.totalCount"
    );
    assert_eq!(
        stats.traces.total_segment_count,
        Some(row.segment_count),
        "{what}: traces.totalSegmentCount"
    );
    assert_eq!(
        stats.traces.total_length,
        Some(row.total_length),
        "{what}: traces.totalLength"
    );
    assert_eq!(
        stats.traces.total_length_mm,
        Some(row.total_length_mm),
        "{what}: traces.totalLengthMm"
    );
    assert_eq!(
        stats.traces.total_weighted_length,
        Some(row.total_weighted_length),
        "{what}: traces.totalWeightedLength"
    );
    assert_eq!(
        stats.bends.total_count,
        Some(row.bend_count),
        "{what}: bends.totalCount"
    );
    assert_eq!(
        stats.vias.total_count,
        Some(row.via_count),
        "{what}: vias.totalCount"
    );
    assert_eq!(
        stats.clearance_violations.total_count,
        Some(row.violation_count),
        "{what}: clearanceViolations.totalCount"
    );
    assert_eq!(
        (
            stats.fanout.total_smd_pins,
            stats.fanout.pins_to_escape,
            stats.fanout.escaped_count
        ),
        row.fanout,
        "{what}: fanout"
    );
    assert_eq!(
        stats.normalized_score(&scoring),
        row.normalized_score,
        "{what}: getNormalizedScore"
    );
}

#[test]
fn normalized_score_matches_the_jvm_on_the_unrouted_stems() {
    let rows = [
        Row {
            dsn: "examples/tutorial_board/tutorial_board.dsn",
            route_k: 0,
            items_total: 439,
            maximum_count: 0,
            incomplete_count: 0,
            trace_count: 0,
            segment_count: 0,
            total_length: 0.0,
            total_length_mm: 0.0,
            total_weighted_length: 0.0,
            bend_count: 0,
            via_count: 0,
            violation_count: 0,
            fanout: (0, 0, 0),
            normalized_score: 0.0,
        },
        Row {
            dsn: "fixtures/Issue143-rpi_splitter.dsn",
            route_k: 0,
            items_total: 33,
            maximum_count: 5,
            incomplete_count: 5,
            trace_count: 0,
            segment_count: 0,
            total_length: 0.0,
            total_length_mm: 0.0,
            total_weighted_length: 0.0,
            bend_count: 0,
            via_count: 0,
            violation_count: 0,
            fanout: (10, 9, 0),
            normalized_score: 0.0,
        },
        Row {
            dsn: "fixtures/Issue413-test.dsn",
            route_k: 0,
            items_total: 37,
            maximum_count: 5,
            incomplete_count: 1,
            trace_count: 11,
            segment_count: 20,
            total_length: 17604.82,
            total_length_mm: 1760.482,
            total_weighted_length: 4.6386614E8,
            bend_count: 9,
            via_count: 4,
            violation_count: 0,
            fanout: (10, 2, 8),
            normalized_score: 799.91797,
        },
        Row {
            dsn: "fixtures/Issue508-DAC2020_bm01.dsn",
            route_k: 0,
            items_total: 320,
            maximum_count: 195,
            incomplete_count: 195,
            trace_count: 0,
            segment_count: 0,
            total_length: 0.0,
            total_length_mm: 0.0,
            total_weighted_length: 0.0,
            bend_count: 0,
            via_count: 0,
            violation_count: 0,
            fanout: (187, 187, 0),
            normalized_score: 0.0,
        },
        Row {
            dsn: "fixtures/Issue103-Board-Unrouted.dsn",
            route_k: 0,
            items_total: 1849,
            maximum_count: 702,
            incomplete_count: 702,
            trace_count: 0,
            segment_count: 0,
            total_length: 0.0,
            total_length_mm: 0.0,
            total_weighted_length: 0.0,
            bend_count: 0,
            via_count: 0,
            violation_count: 0,
            fanout: (60, 60, 0),
            normalized_score: 0.0,
        },
    ];
    for row in &rows {
        check(row);
    }
}

#[test]
#[cfg_attr(debug_assertions, ignore)]
fn normalized_score_matches_the_jvm_on_the_routed_stems() {
    let rows = [
        Row {
            dsn: "fixtures/Issue143-rpi_splitter.dsn",
            route_k: 8,
            items_total: 49,
            maximum_count: 5,
            incomplete_count: 2,
            trace_count: 10,
            segment_count: 24,
            total_length: 130610.65,
            total_length_mm: 51.421516,
            total_weighted_length: 6.637111E9,
            bend_count: 14,
            via_count: 6,
            violation_count: 0,
            fanout: (10, 4, 7),
            normalized_score: 599.98035,
        },
        Row {
            dsn: "fixtures/Issue508-DAC2020_bm01.dsn",
            route_k: 40,
            items_total: 359,
            maximum_count: 195,
            incomplete_count: 165,
            trace_count: 36,
            segment_count: 291,
            total_length: 11023.574,
            total_length_mm: 1102.3574,
            total_weighted_length: 3.3247096E7,
            bend_count: 255,
            via_count: 3,
            violation_count: 0,
            fanout: (187, 164, 27),
            normalized_score: 153.84225,
        },
        Row {
            dsn: "fixtures/Issue026-J2_reference.dsn",
            route_k: 45,
            items_total: 131,
            maximum_count: 33,
            incomplete_count: 7,
            trace_count: 42,
            segment_count: 161,
            total_length: 3098.9275,
            total_length_mm: 309.89276,
            total_weighted_length: 1.0127295E7,
            bend_count: 119,
            via_count: 8,
            violation_count: 0,
            fanout: (51, 20, 31),
            normalized_score: 787.86725,
        },
        Row {
            dsn: "fixtures/Issue649-kicad_ecc83-pp_input_board_v1.dsn",
            route_k: 22,
            items_total: 368,
            maximum_count: 21,
            incomplete_count: 0,
            trace_count: 14,
            segment_count: 28,
            total_length: 1615.3136,
            total_length_mm: 161.53136,
            total_weighted_length: 1.2948354E7,
            bend_count: 14,
            via_count: 0,
            violation_count: 0,
            fanout: (0, 0, 0),
            normalized_score: 999.9971,
        },
        Row {
            dsn: "fixtures/Issue753-CPU-85_r104.dsn",
            route_k: 20,
            items_total: 2415,
            maximum_count: 569,
            incomplete_count: 365,
            trace_count: 65,
            segment_count: 66,
            total_length: 1754.4889,
            total_length_mm: 175.44888,
            total_weighted_length: 0.0,
            bend_count: 1,
            via_count: 19,
            violation_count: 78,
            fanout: (102, 79, 0),
            normalized_score: 331.1068,
        },
    ];
    for row in &rows {
        check(row);
    }
}

#[test]
fn a_shove_fixed_trace_is_weighted_at_half() {
    let mut board = load_board("fixtures/Issue413-test.dsn");
    let trace_ids = board.get_traces();
    assert!(
        !trace_ids.is_empty(),
        "the fixture must have traces to weigh"
    );

    let set_all = |board: &mut Board, state: FixedState| {
        for id in board.get_traces() {
            board
                .get_item_mut(id)
                .expect("the trace was just listed")
                .set_fixed_state(state);
        }
    };

    set_all(&mut board, FixedState::Unfixed);
    let unfixed = BoardStatistics::new(&mut board)
        .traces
        .total_weighted_length
        .expect("the ctor always sets it");
    assert!(unfixed > 0.0, "an unfixed board must weigh something");

    set_all(&mut board, FixedState::ShoveFixed);
    let shove_fixed = BoardStatistics::new(&mut board)
        .traces
        .total_weighted_length
        .expect("the ctor always sets it");
    assert!(
        (shove_fixed * 2.0 - unfixed).abs() <= unfixed * 1e-6,
        "SHOVE_FIXED should weigh half: {shove_fixed} vs {unfixed}"
    );

    for state in [FixedState::UserFixed, FixedState::SystemFixed] {
        set_all(&mut board, state);
        assert_eq!(
            BoardStatistics::new(&mut board)
                .traces
                .total_weighted_length,
            Some(0.0),
            "{state:?} is outside `:252-253`'s filter"
        );
    }
}

#[test]
fn the_unit_normalisation_uses_the_resolution_guard() {
    let mut board = load_board("fixtures/Issue413-test.dsn");
    assert_eq!(board.communication.unit, Unit::Um);
    assert_eq!(board.communication.resolution, 10);

    let converted = BoardStatistics::new(&mut board);
    assert_eq!(converted.traces.total_length, Some(17604.82));
    assert_eq!(converted.traces.total_length_mm, Some(1760.482));

    let mm_per_board_unit = Unit::scale(1.0, Unit::Um, Unit::Mm);
    let native = BoardStatistics::compute(&mut board, Some(Unit::Um), true, true);
    let raw = native.traces.total_length.expect("the ctor always sets it");
    assert_eq!(
        native.traces.total_length_mm,
        Some((f64::from(raw) * (mm_per_board_unit / 10.0)) as f32),
        "resolution 10 divides the factor by 10"
    );

    for resolution in [0, -1] {
        board.communication.resolution = resolution;
        let guarded = BoardStatistics::compute(&mut board, Some(Unit::Um), true, true);
        assert_eq!(
            guarded.traces.total_length,
            Some(raw),
            "resolution does not touch the raw length"
        );
        assert_eq!(
            guarded.traces.total_length_mm,
            Some((f64::from(raw) * mm_per_board_unit) as f32),
            "resolution {resolution} must divide by 1, not by {resolution}"
        );
    }
}

#[test]
fn an_empty_board_scores_zero_not_nan() {
    let mut board = load_board("examples/tutorial_board/tutorial_board.dsn");
    let settings = build_settings(&board);
    let scoring = settings
        .scoring
        .clone()
        .expect("DefaultSettings always writes a scoring block");
    let stats = BoardStatistics::new(&mut board);

    assert_eq!(stats.connections.maximum_count, Some(0));
    assert_eq!(stats.maximum_score(&scoring), 0.0);
    assert!((stats.calculate_score(&scoring) / stats.maximum_score(&scoring)).is_nan());
    let normalized = stats.normalized_score(&scoring);
    assert!(!normalized.is_nan(), "HEAD's guard returns 0f, not NaN");
    assert_eq!(normalized, 0.0);
}

#[test]
fn a_pin_unconnected_on_its_second_net_needs_an_escape() {
    let mut board = load_test_board("tests/data/p9t13-multi-net-smd-pin.dsn");
    let fanout = BoardStatistics::new(&mut board).fanout;

    assert_eq!(fanout.total_smd_pins, 4);
    assert_eq!(fanout.pins_to_escape, 3);
    assert_eq!(
        fanout.escaped_count, 1,
        "only U2 is escaped: U1's NFIRST trace does not escape NSECOND, and U3/U4 have no wiring at all"
    );
}

#[test]
fn float_narrowing_produces_the_expected_bit_patterns() {
    struct Synth {
        tag: &'static str,
        maximum_count: i32,
        incomplete_count: i32,
        violation_count: i32,
        bend_count: i32,
        total_length_mm: f32,
        via_count: i32,
        unrouted_net_penalty: f32,
        clearance_violation_penalty: f32,
        bend_penalty: f32,
        trace_cost: f64,
        via_costs: i32,
        expected: (f32, f32, f32),
    }

    let cases = [
        Synth {
            tag: "S0",
            maximum_count: 16_777_217,
            incomplete_count: 1,
            violation_count: 3,
            bend_count: 7,
            total_length_mm: 1.1,
            via_count: 5,
            unrouted_net_penalty: 1.0,
            clearance_violation_penalty: 0.1,
            bend_penalty: 0.1,
            trace_cost: 0.1,
            via_costs: 3,
            expected: (1.6777199E7, 1.6777216E7, 999.99896),
        },
        Synth {
            tag: "S1",
            maximum_count: 3,
            incomplete_count: 3,
            violation_count: 11,
            bend_count: 129,
            total_length_mm: 12345.678,
            via_count: 17,
            unrouted_net_penalty: 1.0E7,
            clearance_violation_penalty: 1.5,
            bend_penalty: 0.75,
            trace_cost: 3.3,
            via_costs: 42,
            expected: (-41566.74, 3.0E7, 0.0),
        },
        Synth {
            tag: "S2",
            maximum_count: 0,
            incomplete_count: 4,
            violation_count: 0,
            bend_count: 0,
            total_length_mm: 2.5,
            via_count: 1,
            unrouted_net_penalty: 5.0E6,
            clearance_violation_penalty: 1.0,
            bend_penalty: 1.0,
            trace_cost: 1.0,
            via_costs: 50,
            expected: (-2.0000052E7, 0.0, 0.0),
        },
        Synth {
            tag: "S3",
            maximum_count: 7,
            incomplete_count: 0,
            violation_count: 0,
            bend_count: 0,
            total_length_mm: 0.0,
            via_count: 3000,
            unrouted_net_penalty: 1.0,
            clearance_violation_penalty: 1.0,
            bend_penalty: 1.0,
            trace_cost: 1.0,
            via_costs: 1_000_000,
            expected: (1.2949673E9, 7.0, 1.8499533E11),
        },
        Synth {
            tag: "S4",
            maximum_count: 1,
            incomplete_count: 1,
            violation_count: 0,
            bend_count: 0,
            total_length_mm: 1.0E-40,
            via_count: 0,
            unrouted_net_penalty: 3.4E38,
            clearance_violation_penalty: 1.0,
            bend_penalty: 1.0,
            trace_cost: 1.0,
            via_costs: 1,
            expected: (-1.0E-40, 3.4E38, 0.0),
        },
        Synth {
            tag: "S5",
            maximum_count: 2,
            incomplete_count: 2,
            violation_count: 0,
            bend_count: 0,
            total_length_mm: 0.0,
            via_count: 0,
            unrouted_net_penalty: 3.4E38,
            clearance_violation_penalty: 1.0,
            bend_penalty: 1.0,
            trace_cost: 1.0,
            via_costs: 1,
            expected: (f32::NAN, f32::INFINITY, 0.0),
        },
    ];

    for case in &cases {
        let mut stats = BoardStatistics::default();
        stats.connections.maximum_count = Some(case.maximum_count);
        stats.connections.incomplete_count = Some(case.incomplete_count);
        stats.clearance_violations.total_count = Some(case.violation_count);
        stats.bends.total_count = Some(case.bend_count);
        stats.traces.total_length_mm = Some(case.total_length_mm);
        stats.vias.total_count = Some(case.via_count);

        let scoring = ScoringSettings {
            unrouted_net_penalty: Some(case.unrouted_net_penalty),
            clearance_violation_penalty: Some(case.clearance_violation_penalty),
            bend_penalty: Some(case.bend_penalty),
            default_preferred_direction_trace_cost: Some(case.trace_cost),
            via_costs: Some(case.via_costs),
            ..ScoringSettings::default()
        };

        let actual = (
            stats.calculate_score(&scoring),
            stats.maximum_score(&scoring),
            stats.normalized_score(&scoring),
        );
        let same = |a: f32, b: f32| (a.is_nan() && b.is_nan()) || a.to_bits() == b.to_bits();
        assert!(
            same(actual.0, case.expected.0)
                && same(actual.1, case.expected.1)
                && same(actual.2, case.expected.2),
            "{}: got {actual:?}, want {:?}",
            case.tag,
            case.expected
        );
    }
}

#[test]
fn the_connection_counters_are_fr_drcs_own() {
    let mut board = load_board("fixtures/Issue413-test.dsn");
    let stats = BoardStatistics::new(&mut board);

    let mut drc = DesignRulesChecker::new(&mut board);
    drc.calculate_all_incompletes();
    assert_eq!(stats.connections.maximum_count, Some(drc.max_connections()));
    assert_eq!(
        stats.connections.incomplete_count,
        Some(drc.get_incomplete_count() as i32)
    );
    let violations = drc.get_all_violations();
    let routing_involved: Vec<DrcViolation> = violations
        .into_iter()
        .filter(|violation| violation.involves_routing(&board))
        .collect();
    assert_eq!(
        stats.clearance_violations.total_count,
        Some(routing_involved.len() as i32)
    );
}
