mod common;

use common::synthetic::{PadSpec, SyntheticBoard};
use fr_board::prelude::FixedState;
use fr_board::{DrcConstraints, ItemId};
use fr_drc::checks::edge;
use fr_drc::checks::geometry::{gap_below, hole_of, is_microvia, is_through_hole_pin, item_shapes};
use fr_drc::checks::{copper, holes, single};
use fr_drc::report::{DrcCoordinates, DrcJsonFlavor, DrcReportOptions};
use fr_drc::{
    BoardStatisticsClearanceViolations, DesignRulesChecker, DrcSeverity, DrcViolation,
    DrcViolationKind,
};
use fr_geometry::{IntBox, IntVector, TileShape};

fn boxes(gap: i32) -> (TileShape, TileShape) {
    (
        TileShape::Box(IntBox::from_coords(0, 0, 1000, 1000)),
        TileShape::Box(IntBox::from_coords(1000 + gap, 0, 2000 + gap, 1000)),
    )
}

#[test]
fn gap_below_reports_the_actual_gap_only_when_it_is_under_the_clearance() {
    let (a, b) = boxes(300);
    assert!(gap_below(&a, &b, 300).is_none());
    let (actual, _) = gap_below(&a, &b, 500).expect("300 is under 500");
    assert!((actual - 300.0).abs() < 2.0, "actual {actual}");
    let (a, b) = boxes(-100);
    let (actual, _) = gap_below(&a, &b, 500).expect("overlap is under any clearance");
    assert_eq!(actual, 0.0);
    assert!(
        gap_below(&a, &b, 0).is_some(),
        "overlap is reported even at zero clearance"
    );
    let (a, b) = boxes(10);
    assert!(
        gap_below(&a, &b, 0).is_none(),
        "a gap is fine at zero clearance"
    );
}

#[test]
fn via_holes_are_exact_and_pad_holes_are_estimated() {
    let mut synthetic = SyntheticBoard::new(
        &[
            PadSpec {
                name: "1",
                half: 800,
                offset: IntVector::new(0, 0),
                through_hole: true,
            },
            PadSpec {
                name: "2",
                half: 800,
                offset: IntVector::new(5000, 0),
                through_hole: false,
            },
        ],
        1,
        2000,
    );
    let via = synthetic.via(20_000, 0, 1);
    let micro = synthetic.microvia(30_000, 0, 1);
    let th = synthetic.pin(0, 1);
    let smd = synthetic.pin(1, 1);
    let board = &synthetic.board;

    let hole = hole_of(board, via).expect("a via has a hole");
    assert!(!hole.estimated);
    assert!((hole.radius - 1500.0).abs() < 1.0, "radius {}", hole.radius);
    assert!(is_through_hole_pin(board, th));
    assert!(!is_through_hole_pin(board, smd));
    let pad_hole = hole_of(board, th).expect("a through-hole pin has a hole");
    assert!(pad_hole.estimated);
    assert!(hole_of(board, smd).is_none());
    assert!(!is_microvia(board, via));
    assert!(!is_microvia(board, micro));
}

#[test]
fn item_shapes_lists_one_shape_per_layer_for_a_via_and_one_for_a_trace() {
    let mut synthetic = SyntheticBoard::new(&[], 1, 2000);
    let via = synthetic.via(0, 0, 1);
    let trace = synthetic.trace(&[(0, 0), (10_000, 0)], 1, 500, 1);
    let via_shapes = item_shapes(&mut synthetic.board, via);
    assert_eq!(
        via_shapes
            .iter()
            .map(|(layer, _)| *layer)
            .collect::<Vec<_>>(),
        vec![0, 1]
    );
    let trace_shapes = item_shapes(&mut synthetic.board, trace);
    assert_eq!(trace_shapes.len(), 1);
    assert_eq!(trace_shapes[0].0, 1);
}

fn kinds(violations: &[DrcViolation]) -> Vec<DrcViolationKind> {
    let mut kinds: Vec<DrcViolationKind> = violations.iter().map(|v| v.kind).collect();
    kinds.sort();
    kinds
}

fn constraints_with(clearance: i32) -> DrcConstraints {
    let mut constraints = DrcConstraints::default();
    constraints
        .netclass_clearance
        .insert("Default".to_string(), clearance);
    constraints
}

#[test]
fn two_traces_on_different_nets_closer_than_the_clearance_are_a_clearance_violation() {
    let mut synthetic = SyntheticBoard::new(&[], 2, 2000);
    synthetic.trace(&[(0, 0), (10_000, 0)], 0, 500, 1);
    synthetic.trace(&[(0, 1500), (10_000, 1500)], 0, 500, 2);
    let mut out = Vec::new();
    copper::run(&mut synthetic.board, &constraints_with(2000), &mut out);
    assert_eq!(kinds(&out), vec![DrcViolationKind::Clearance]);
    assert_eq!(out[0].expected, 2000.0);
    assert!(
        (out[0].actual - 500.0).abs() < 2.0,
        "actual {}",
        out[0].actual
    );
    assert_eq!(out[0].layer, Some(0));
}

#[test]
fn a_gap_within_the_epsilon_of_the_clearance_is_not_a_violation() {
    let mut synthetic = SyntheticBoard::new(&[], 2, 2000);
    synthetic.trace(&[(0, 0), (10_000, 0)], 0, 500, 1);
    synthetic.trace(&[(0, 2998), (10_000, 2998)], 0, 500, 2);
    let mut constraints = constraints_with(2000);
    let mut out = Vec::new();
    copper::run(&mut synthetic.board, &constraints, &mut out);
    assert_eq!(kinds(&out), vec![DrcViolationKind::Clearance]);
    constraints.epsilon = 5;
    out.clear();
    copper::run(&mut synthetic.board, &constraints, &mut out);
    assert!(out.is_empty(), "{out:?}");
}

#[test]
fn the_same_pair_is_reported_once() {
    let mut synthetic = SyntheticBoard::new(&[], 2, 2000);
    synthetic.trace(&[(0, 0), (10_000, 0)], 0, 500, 1);
    synthetic.trace(&[(0, 1500), (10_000, 1500)], 0, 500, 2);
    let mut out = Vec::new();
    copper::run(&mut synthetic.board, &constraints_with(2000), &mut out);
    assert_eq!(out.len(), 1);
}

#[test]
fn same_net_items_are_never_clearance_violations() {
    let mut synthetic = SyntheticBoard::new(&[], 1, 2000);
    synthetic.trace(&[(0, 0), (10_000, 0)], 0, 500, 1);
    synthetic.trace(&[(0, 1500), (10_000, 1500)], 0, 500, 1);
    let mut out = Vec::new();
    copper::run(&mut synthetic.board, &constraints_with(2000), &mut out);
    assert!(out.is_empty(), "{out:?}");
}

#[test]
fn overlapping_items_on_different_nets_are_shorting_items() {
    let mut synthetic = SyntheticBoard::new(&[], 2, 2000);
    synthetic.trace(&[(0, 0), (10_000, 0)], 0, 500, 1);
    synthetic.trace(&[(0, 200), (10_000, 200)], 0, 500, 2);
    let mut out = Vec::new();
    copper::run(&mut synthetic.board, &constraints_with(2000), &mut out);
    assert_eq!(kinds(&out), vec![DrcViolationKind::ShortingItems]);
    assert_eq!(out[0].actual, 0.0);
}

#[test]
fn crossing_traces_are_tracks_crossing_and_nothing_else() {
    let mut synthetic = SyntheticBoard::new(&[], 2, 2000);
    synthetic.trace(&[(-5000, 0), (5000, 0)], 0, 500, 1);
    synthetic.trace(&[(0, -5000), (0, 5000)], 0, 500, 2);
    let mut out = Vec::new();
    copper::run(&mut synthetic.board, &constraints_with(2000), &mut out);
    assert_eq!(kinds(&out), vec![DrcViolationKind::TracksCrossing]);
    assert!(
        out[0]
            .position
            .distance(&fr_geometry::FloatPoint::new(0.0, 0.0))
            < 1.0
    );
}

#[test]
fn a_trace_near_a_via_hole_on_another_net_is_a_hole_clearance_violation() {
    let mut synthetic = SyntheticBoard::new(&[], 2, 2000);
    synthetic.via(0, 0, 1);
    synthetic.trace(&[(-10_000, 4000), (10_000, 4000)], 0, 300, 2);
    let mut constraints = constraints_with(100);
    constraints.hole_clearance = Some(2500);
    let mut out = Vec::new();
    copper::run(&mut synthetic.board, &constraints, &mut out);
    assert_eq!(kinds(&out), vec![DrcViolationKind::HoleClearance]);
    assert_eq!(out[0].expected, 2500.0);
    assert!(!out[0].estimated);
}

#[test]
fn stacked_same_net_sub_pads_report_nothing() {
    let mut synthetic = SyntheticBoard::new(
        &[
            PadSpec {
                name: "41@1",
                half: 900,
                offset: IntVector::new(0, 0),
                through_hole: true,
            },
            PadSpec {
                name: "41@2",
                half: 900,
                offset: IntVector::new(0, 1400),
                through_hole: true,
            },
        ],
        1,
        2000,
    );
    synthetic.pin(0, 1);
    synthetic.pin(1, 1);
    let mut constraints = constraints_with(2000);
    constraints.hole_clearance = Some(500);
    let mut out = Vec::new();
    copper::run(&mut synthetic.board, &constraints, &mut out);
    assert!(out.is_empty(), "{out:?}");
}

#[test]
fn different_net_pads_of_one_footprint_still_get_clearance_checked() {
    let mut synthetic = SyntheticBoard::new(
        &[
            PadSpec {
                name: "1",
                half: 900,
                offset: IntVector::new(0, 0),
                through_hole: false,
            },
            PadSpec {
                name: "2",
                half: 900,
                offset: IntVector::new(2500, 0),
                through_hole: false,
            },
        ],
        2,
        2000,
    );
    synthetic.pin(0, 1);
    synthetic.pin(1, 2);
    let mut out = Vec::new();
    copper::run(&mut synthetic.board, &constraints_with(2000), &mut out);
    assert_eq!(kinds(&out), vec![DrcViolationKind::Clearance]);
}

#[test]
#[allow(clippy::field_reassign_with_default)]
fn two_via_holes_closer_than_the_minimum_are_hole_to_hole_regardless_of_net() {
    let mut synthetic = SyntheticBoard::new(&[], 1, 2000);
    synthetic.via(0, 0, 1);
    synthetic.via(4500, 0, 1);
    let mut constraints = DrcConstraints::default();
    constraints.hole_to_hole = Some(2500);
    let mut out = Vec::new();
    holes::run(&mut synthetic.board, &constraints, &mut out);
    assert_eq!(kinds(&out), vec![DrcViolationKind::HoleToHole]);
    assert_eq!(out[0].layer, None);
    assert!(
        (out[0].actual - 1500.0).abs() < 60.0,
        "actual {}",
        out[0].actual
    );
}

#[test]
#[allow(clippy::field_reassign_with_default)]
fn hole_to_hole_is_skipped_without_a_rule_and_for_far_holes() {
    let mut synthetic = SyntheticBoard::new(&[], 1, 2000);
    synthetic.via(0, 0, 1);
    synthetic.via(4500, 0, 1);
    let mut out = Vec::new();
    holes::run(&mut synthetic.board, &DrcConstraints::default(), &mut out);
    assert!(out.is_empty());
    let mut constraints = DrcConstraints::default();
    constraints.hole_to_hole = Some(1000);
    holes::run(&mut synthetic.board, &constraints, &mut out);
    assert!(out.is_empty());
}

#[test]
#[allow(clippy::field_reassign_with_default)]
fn a_thin_trace_is_a_track_width_violation() {
    let mut synthetic = SyntheticBoard::new(&[], 1, 2000);
    synthetic.trace(&[(0, 0), (10_000, 0)], 0, 500, 1);
    let mut constraints = DrcConstraints::default();
    constraints.min_track_width = Some(2000);
    let mut out = Vec::new();
    single::run(&mut synthetic.board, &constraints, &mut out);
    assert_eq!(kinds(&out), vec![DrcViolationKind::TrackWidth]);
    assert_eq!(out[0].expected, 2000.0);
    assert_eq!(out[0].actual, 1000.0);
    assert_eq!(out[0].second_item, None);
}

/// `kicad-cli` never floors `track_width` at a net class's `track width` — only the board's own
/// `min_track_width` is enforced — so a class value wider than the board minimum must not raise a
/// violation the board minimum alone would not.
#[test]
fn a_netclass_width_wider_than_the_board_minimum_is_not_enforced() {
    let mut synthetic = SyntheticBoard::new(&[], 1, 2000);
    synthetic.trace(&[(0, 0), (10_000, 0)], 0, 750, 1);
    let mut constraints = DrcConstraints::default();
    constraints
        .netclass_track_width
        .insert("Default".to_string(), 5000);
    constraints.min_track_width = Some(1000);
    let mut out = Vec::new();
    single::run(&mut synthetic.board, &constraints, &mut out);
    assert!(out.is_empty(), "{out:?}");
}

#[test]
#[allow(clippy::field_reassign_with_default)]
fn via_diameter_annular_width_and_drill_are_checked_against_project_minimums() {
    let mut synthetic = SyntheticBoard::new(&[], 1, 2000);
    synthetic.via(0, 0, 1);
    let mut constraints = DrcConstraints::default();
    constraints.min_via_diameter = Some(7000);
    constraints.min_via_annular_width = Some(2000);
    constraints.min_through_hole_diameter = Some(4000);
    let mut out = Vec::new();
    single::run(&mut synthetic.board, &constraints, &mut out);
    assert_eq!(
        kinds(&out),
        vec![
            DrcViolationKind::ViaDiameter,
            DrcViolationKind::AnnularWidth,
            DrcViolationKind::DrillOutOfRange,
        ]
    );
    let diameter = out
        .iter()
        .find(|v| v.kind == DrcViolationKind::ViaDiameter)
        .unwrap();
    assert_eq!(diameter.actual, 6000.0);
    let annular = out
        .iter()
        .find(|v| v.kind == DrcViolationKind::AnnularWidth)
        .unwrap();
    assert!((annular.actual - 1500.0).abs() < 1.0);
    let drill = out
        .iter()
        .find(|v| v.kind == DrcViolationKind::DrillOutOfRange)
        .unwrap();
    assert!((drill.actual - 3000.0).abs() < 1.0);
    assert!(!drill.estimated);
}

#[test]
#[allow(clippy::field_reassign_with_default)]
fn a_compliant_via_reports_nothing() {
    let mut synthetic = SyntheticBoard::new(&[], 1, 2000);
    synthetic.via(0, 0, 1);
    let mut constraints = DrcConstraints::default();
    constraints.min_via_diameter = Some(5000);
    constraints.min_via_annular_width = Some(1000);
    constraints.min_through_hole_diameter = Some(3000);
    let mut out = Vec::new();
    single::run(&mut synthetic.board, &constraints, &mut out);
    assert!(out.is_empty(), "{out:?}");
}

#[test]
#[allow(clippy::field_reassign_with_default)]
fn a_through_hole_pad_annular_violation_is_marked_estimated() {
    let mut synthetic = SyntheticBoard::new(
        &[PadSpec {
            name: "1",
            half: 800,
            offset: IntVector::new(0, 0),
            through_hole: true,
        }],
        1,
        2000,
    );
    synthetic.pin(0, 1);
    let mut constraints = DrcConstraints::default();
    constraints.min_via_annular_width = Some(1000);
    let mut out = Vec::new();
    single::run(&mut synthetic.board, &constraints, &mut out);
    assert_eq!(kinds(&out), vec![DrcViolationKind::AnnularWidth]);
    assert!(out[0].estimated);
}

#[test]
#[allow(clippy::field_reassign_with_default)]
fn a_trace_near_the_board_edge_is_a_copper_edge_clearance_violation() {
    let mut synthetic = SyntheticBoard::new(&[], 1, 2000);
    synthetic.trace(&[(-40_000, 48_000), (40_000, 48_000)], 0, 500, 1);
    let mut constraints = DrcConstraints::default();
    constraints.copper_edge_clearance = Some(5000);
    let mut out = Vec::new();
    edge::run(&mut synthetic.board, &constraints, &mut out);
    assert_eq!(kinds(&out), vec![DrcViolationKind::CopperEdgeClearance]);
    assert_eq!(out[0].expected, 5000.0);
    assert!(
        (out[0].actual - 1500.0).abs() < 2.0,
        "actual {}",
        out[0].actual
    );
    assert_eq!(out[0].second_item, synthetic.board.get_outline());
}

#[test]
#[allow(clippy::field_reassign_with_default)]
fn edge_clearance_is_silent_without_a_rule_or_away_from_the_edge() {
    let mut synthetic = SyntheticBoard::new(&[], 1, 2000);
    synthetic.trace(&[(-10_000, 0), (10_000, 0)], 0, 500, 1);
    let mut out = Vec::new();
    edge::run(&mut synthetic.board, &DrcConstraints::default(), &mut out);
    assert!(out.is_empty());
    let mut constraints = DrcConstraints::default();
    constraints.copper_edge_clearance = Some(5000);
    edge::run(&mut synthetic.board, &constraints, &mut out);
    assert!(out.is_empty(), "{out:?}");
}

#[test]
fn get_all_violations_runs_every_family_in_a_deterministic_order() {
    let mut synthetic = SyntheticBoard::new(&[], 2, 2000);
    synthetic.trace(&[(0, 0), (10_000, 0)], 0, 300, 1);
    synthetic.trace(&[(0, 1500), (10_000, 1500)], 0, 500, 2);
    synthetic.via(30_000, 30_000, 1);
    synthetic.via(30_000, 34_500, 1);
    let mut constraints = DrcConstraints::default();
    constraints
        .netclass_clearance
        .insert("Default".to_string(), 2000);
    constraints.min_track_width = Some(1000);
    constraints.hole_to_hole = Some(2500);
    synthetic.board.rules.drc_constraints = Some(constraints);

    let first = DesignRulesChecker::new(&mut synthetic.board).get_all_violations();
    let second = DesignRulesChecker::new(&mut synthetic.board).get_all_violations();
    assert_eq!(first, second);
    assert_eq!(
        kinds(&first),
        vec![
            DrcViolationKind::Clearance,
            DrcViolationKind::HoleToHole,
            DrcViolationKind::TrackWidth,
        ]
    );
    let ids: Vec<u32> = first.iter().map(|v| v.first_item.0).collect();
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    assert_eq!(ids, sorted, "ordered by first item");
}

#[test]
fn an_ignored_severity_drops_the_kind() {
    let mut synthetic = SyntheticBoard::new(&[], 2, 2000);
    synthetic.trace(&[(0, 0), (10_000, 0)], 0, 500, 1);
    synthetic.trace(&[(0, 1500), (10_000, 1500)], 0, 500, 2);
    let mut constraints = DrcConstraints::default();
    constraints
        .netclass_clearance
        .insert("Default".to_string(), 2000);
    constraints
        .severities
        .insert("clearance".to_string(), DrcSeverity::Ignore);
    synthetic.board.rules.drc_constraints = Some(constraints);
    assert!(
        DesignRulesChecker::new(&mut synthetic.board)
            .get_all_violations()
            .is_empty()
    );
}

fn report_options() -> DrcReportOptions {
    DrcReportOptions {
        source: "synthetic.dsn".to_string(),
        coordinate_unit: "mm".to_string(),
        date: "2026-09-04T00:00Z".to_string(),
        freerouting_version: "test".to_string(),
        quality_score: None,
    }
}

#[test]
fn the_report_carries_kicad_types_and_severities() {
    let mut synthetic = SyntheticBoard::new(&[], 2, 2000);
    synthetic.trace(&[(0, 0), (10_000, 0)], 0, 300, 1);
    synthetic.trace(&[(0, 1500), (10_000, 1500)], 0, 500, 2);
    let mut constraints = DrcConstraints::default();
    constraints
        .netclass_clearance
        .insert("Default".to_string(), 2000);
    constraints.min_track_width = Some(1000);
    constraints
        .severities
        .insert("track_width".to_string(), DrcSeverity::Warning);
    synthetic.board.rules.drc_constraints = Some(constraints);
    let transform = fr_dsn::CoordinateTransform::new(10.0, 0.0, 0.0).expect("a scale");
    let coords = DrcCoordinates {
        board_unit: fr_board::Unit::Um,
        transform,
    };
    let mut checker = DesignRulesChecker::new(&mut synthetic.board);
    let report = checker.generate_report(&coords, &report_options());
    let types: Vec<(&str, &str)> = report
        .violations
        .iter()
        .map(|v| (v.kind.as_str(), v.severity))
        .collect();
    assert_eq!(
        &types[..2],
        [("clearance", "error"), ("track_width", "warning")]
    );
    assert!(
        report.violations[0]
            .description
            .starts_with("Clearance violation between Trace [N1] and Trace [N2]")
    );
    assert_eq!(report.violations[0].items.len(), 2);
    assert_eq!(report.violations[1].items.len(), 1);

    let kicad = report.to_json(DrcJsonFlavor::KiCad).expect("serialises");
    assert!(kicad.contains("\"type\": \"track_width\""));
    let head = report
        .to_json(DrcJsonFlavor::FreeroutingHead)
        .expect("serialises");
    assert!(head.contains("\"type\": \"track_width\""));
}

#[test]
fn hole_clearance_keeps_its_camel_case_name_in_the_head_flavour_only() {
    let mut synthetic = SyntheticBoard::new(&[], 2, 2000);
    synthetic.via(0, 0, 1);
    synthetic.trace(&[(-10_000, 4000), (10_000, 4000)], 0, 300, 2);
    let mut constraints = DrcConstraints::default();
    constraints
        .netclass_clearance
        .insert("Default".to_string(), 100);
    constraints.hole_clearance = Some(2500);
    synthetic.board.rules.drc_constraints = Some(constraints);
    let transform = fr_dsn::CoordinateTransform::new(10.0, 0.0, 0.0).expect("a scale");
    let coords = DrcCoordinates {
        board_unit: fr_board::Unit::Um,
        transform,
    };
    let report =
        DesignRulesChecker::new(&mut synthetic.board).generate_report(&coords, &report_options());
    assert_eq!(report.violations[0].kind, "hole_clearance");
    let head = report
        .to_json(DrcJsonFlavor::FreeroutingHead)
        .expect("serialises");
    assert!(head.contains("\"type\": \"holeClearance\""));
    let kicad = report.to_json(DrcJsonFlavor::KiCad).expect("serialises");
    assert!(kicad.contains("\"type\": \"hole_clearance\""));
}

#[test]
fn statistics_sum_the_shortfall_in_micrometres() {
    let violations = vec![
        DrcViolation {
            kind: DrcViolationKind::Clearance,
            severity: DrcSeverity::Error,
            first_item: ItemId(1),
            second_item: Some(ItemId(2)),
            layer: Some(0),
            position: fr_geometry::FloatPoint::new(0.0, 0.0),
            expected: 2000.0,
            actual: 500.0,
            estimated: false,
        },
        DrcViolation {
            kind: DrcViolationKind::TrackWidth,
            severity: DrcSeverity::Error,
            first_item: ItemId(3),
            second_item: None,
            layer: Some(0),
            position: fr_geometry::FloatPoint::new(0.0, 0.0),
            expected: 1000.0,
            actual: 600.0,
            estimated: false,
        },
    ];
    let stats = BoardStatisticsClearanceViolations::from_violations(&violations, 0.1);
    assert_eq!(stats.total_count, Some(2));
    assert_eq!(stats.min_violation_um, Some(40.0));
    assert_eq!(stats.max_violation_um, Some(150.0));
    assert_eq!(stats.avg_violation_um, Some(95.0));
}

#[test]
fn a_trace_crossing_a_conduction_area_on_another_net_reports_nothing() {
    let mut synthetic = SyntheticBoard::new(&[], 2, 2000);
    synthetic.board.insert_conduction_area(
        fr_geometry::Area::Shape(fr_geometry::Shape::Tile(TileShape::Box(
            IntBox::from_coords(-20_000, -20_000, 20_000, 20_000),
        ))),
        0,
        vec![2],
        1,
        false,
        FixedState::Unfixed,
    );
    synthetic.trace(&[(-30_000, 0), (30_000, 0)], 0, 500, 1);
    let mut out = Vec::new();
    copper::run(&mut synthetic.board, &constraints_with(2000), &mut out);
    assert!(out.is_empty(), "{out:?}");
}

#[test]
#[allow(clippy::field_reassign_with_default)]
fn exact_kicad_drills_remove_false_positives_but_still_detect_small_holes() {
    let json = r#"{"layers":[{"name":"F.Cu"},{"name":"B.Cu"}],
      "nets":[{"id":1,"name":"GND"}],
      "components":[{"reference":"USB","position":{"x":0,"y":0},"pads":[
        {"name":"mount","shape":"circle","size":{"x":0.65,"y":0.65},"drill":0.65,"nonPlated":true,"layers":["F.Cu","B.Cu"]}]}],
      "vias":[{"netName":"GND","position":{"x":10,"y":10},"diameter":0.5,"drill":0.3,"startLayerIndex":0,"endLayerIndex":1},
              {"netName":"GND","position":{"x":20,"y":10},"diameter":0.5,"drill":0.2,"startLayerIndex":0,"endLayerIndex":1}] }"#;
    let fr_dsn::error::BoardReadResult::Success {
        board: Some(mut board),
        ..
    } = fr_dsn::kicad::read_board(json, None)
    else {
        panic!("import failed")
    };
    let mut constraints = DrcConstraints::default();
    constraints.min_through_hole_diameter = Some(3000);
    constraints.min_via_annular_width = Some(1000);
    let mut out = Vec::new();
    single::run(&mut board, &constraints, &mut out);
    assert_eq!(out.len(), 1, "{out:?}");
    assert_eq!(out[0].kind, DrcViolationKind::DrillOutOfRange);
    assert_eq!(out[0].actual, 2000.0);
    assert!(!out[0].estimated);
}

#[test]
#[allow(clippy::field_reassign_with_default)]
fn drc_uses_physical_pad_shapes_and_does_not_treat_bare_holes_as_copper() {
    let json = r#"{"layers":[{"name":"F.Cu"},{"name":"B.Cu"}],
      "nets":[{"id":1,"name":"N"}],
      "components":[
      {"reference":"H","position":{"x":0,"y":0},"pads":[{"name":"1","shape":"circle","size":{"x":1,"y":1},"drill":1,"nonPlated":true,"layers":["F.Cu","B.Cu"]}]},
      {"reference":"P","position":{"x":1,"y":0},"pads":[{"name":"1","netName":"N","shape":"rect","size":{"x":0.4,"y":0.4},"layers":["F.Cu"]}]}]}"#;
    let fr_dsn::BoardReadResult::Success {
        board: Some(mut board),
        ..
    } = fr_dsn::kicad::read_board(json, None)
    else {
        panic!("fixture import failed")
    };
    let hole = board.get_pins()[0];
    let before = item_shapes(&mut board, hole);
    board.apply_hole_clearance_override(500.0);
    assert_eq!(
        before,
        item_shapes(&mut board, hole),
        "routing margins must not change measured copper"
    );
    let mut constraints = DrcConstraints::default();
    constraints.min_clearance = Some(4000);
    constraints.hole_clearance = Some(5000);
    let mut out = Vec::new();
    copper::run(&mut board, &constraints, &mut out);
    assert_eq!(out.len(), 1, "{out:?}");
    assert_eq!(out[0].kind, DrcViolationKind::HoleClearance);
    assert!((out[0].actual - 3000.0).abs() < 2.0);
}

#[test]
fn round_kicad_vias_do_not_have_square_corners_but_real_diagonal_gaps_fail() {
    // Via radius .25 + trace half-width .125 + required clearance .128 = .503 mm.
    // A diagonal with x+y=.746 is .5275 mm from the origin, safely outside that.
    // Against a square via it would appear only about .049 mm clear.
    for (intercept, expect_violation) in [(0.746, false), (0.67, true)] {
        let json = serde_json::json!({
            "layers": [{"name":"F.Cu"},{"name":"B.Cu"}],
            "netClasses": [{"name":"Default","clearance":0.128,"traceWidth":0.25,"viaDiameter":0.5,"viaDrill":0.3}],
            "nets": [{"id":1,"name":"A"},{"id":2,"name":"B"}],
            "vias": [{"netName":"A","position":{"x":0,"y":0},"diameter":0.5,"drill":0.3,"startLayerIndex":0,"endLayerIndex":1}],
            "traces": [{"netName":"B","layerIndex":0,"width":0.25,"points":[{"x":-1,"y":intercept+1.0},{"x":intercept+1.0,"y":-1}]}]
        });
        let fr_dsn::BoardReadResult::Success {
            board: Some(mut board),
            ..
        } = fr_dsn::kicad::read_board(&json.to_string(), None)
        else {
            panic!("fixture import failed")
        };
        let violations = DesignRulesChecker::new(&mut board).get_all_violations();
        assert_eq!(
            violations
                .iter()
                .any(|v| v.kind == DrcViolationKind::Clearance),
            expect_violation,
            "intercept={intercept}: {violations:?}"
        );
        let ctx = board.ctx();
        let fr_board::Item::Via(via) = board.get_item(board.get_vias()[0]).unwrap() else {
            panic!("via")
        };
        assert!(matches!(
            via.get_shape_on_layer(0, &ctx),
            Some(fr_geometry::Shape::Circle(_))
        ));
        assert!(matches!(
            board
                .library
                .padstacks
                .get_by_name("defaultVia")
                .unwrap()
                .get_shape(0),
            Some(fr_geometry::Shape::Circle(_))
        ));
    }
}

fn kicad_board(json: &str) -> fr_board::Board {
    match fr_dsn::kicad::read_board(json, None) {
        fr_dsn::BoardReadResult::Success {
            board: Some(board), ..
        } => *board,
        other => panic!("fixture import failed: {other:?}"),
    }
}

#[test]
fn a_through_hole_pad_without_a_drill_still_has_an_estimated_hole() {
    let board = kicad_board(
        r#"{"layers":[{"name":"F.Cu"},{"name":"B.Cu"}],
          "components":[{"reference":"J","position":{"x":5,"y":5},"pads":[
          {"name":"1","shape":"circle","size":{"x":1.7,"y":1.7},"layers":["F.Cu","B.Cu"]}]}]}"#,
    );
    let pin = board.get_pins()[0];
    let hole = hole_of(&board, pin).expect("a pad spanning both layers has a hole");
    assert!(hole.estimated);
    assert!(hole.radius > 0.0);
}

#[test]
#[allow(clippy::field_reassign_with_default)]
fn a_bare_hole_near_the_board_edge_is_not_a_copper_edge_violation() {
    let mut board = kicad_board(
        r#"{"layers":[{"name":"F.Cu"},{"name":"B.Cu"}],
          "outline":{"corners":[{"x":0,"y":0},{"x":20,"y":0},{"x":20,"y":20},{"x":0,"y":20}]},
          "components":[{"reference":"H","position":{"x":0.6,"y":10},"pads":[
          {"name":"1","shape":"circle","size":{"x":1,"y":1},"drill":1,"nonPlated":true,"layers":["F.Cu","B.Cu"]}]}]}"#,
    );
    let mut constraints = DrcConstraints::default();
    constraints.copper_edge_clearance = Some(5000);
    let mut out = Vec::new();
    edge::run(&mut board, &constraints, &mut out);
    assert!(
        out.is_empty(),
        "a hole with no copper has no copper to clear: {out:?}"
    );
}

#[test]
fn usb_hole_clearance_uses_physical_rectangles_and_rounded_corners_in_both_pair_orders() {
    for (shape, dy, expected_gap) in [
        ("rect", 0.31, 0.1750999900),
        ("rect", 0.49, 0.2098831648),
        ("roundrect", 0.31, 0.1944027188),
        ("roundrect", 0.49, 0.2585529974),
    ] {
        for rotation in [0.0, 45.0, 90.0, 135.0, 37.0] {
            for side in ["F.Cu", "B.Cu"] {
                for reverse in [false, true] {
                    let hole = serde_json::json!({"name":"H","shape":"circle","size":{"x":0.65,"y":0.65},
                        "drill":0.65,"nonPlated":true,"layers":["F.Cu","B.Cu"]});
                    let pad = serde_json::json!({"name":"1","shape":shape,"roundRectRatio":0.25,
                        "size":{"x":1.24,"y":0.6},"offset":{"x":1.12,"y":dy},
                        "netName":"GND","layers":[side]});
                    let pads = if reverse {
                        vec![pad, hole]
                    } else {
                        vec![hole, pad]
                    };
                    let json = serde_json::json!({"layers":[{"name":"F.Cu"},{"name":"B.Cu"}],
                        "nets":[{"id":1,"name":"GND"}],
                        "components":[{"reference":"J1","position":{"x":10,"y":10},
                            "rotation":rotation,"layer":side,"pads":pads}]});
                    let mut board = kicad_board(&json.to_string());
                    let constraints = DrcConstraints {
                        hole_clearance: Some(2500),
                        ..Default::default()
                    };
                    let mut violations = Vec::new();
                    copper::run(&mut board, &constraints, &mut violations);
                    let holes: Vec<_> = violations
                        .iter()
                        .filter(|v| v.kind == DrcViolationKind::HoleClearance)
                        .collect();
                    let label =
                        format!("{shape} {dy} rotation={rotation} {side} reverse={reverse}");
                    assert_eq!(
                        holes.len(),
                        usize::from(expected_gap < 0.25),
                        "{label}: {holes:?}"
                    );
                    if let Some(v) = holes.first() {
                        assert!(
                            (v.actual / 10000.0 - expected_gap).abs() < 0.001,
                            "{label}: {}",
                            v.actual
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn physical_hole_overlap_is_not_hidden_by_a_zero_clearance_setting() {
    let mut board = kicad_board(
        r#"{"layers":[{"name":"F.Cu"},{"name":"B.Cu"}],
      "components":[{"reference":"J","position":{"x":5,"y":5},"pads":[
        {"name":"H","shape":"circle","size":{"x":1,"y":1},"drill":1,"nonPlated":true,"layers":["F.Cu","B.Cu"]},
        {"name":"1","shape":"rect","size":{"x":1,"y":1},"layers":["F.Cu"],"offset":{"x":0.75,"y":0}}
      ]}]}"#,
    );
    let constraints = DrcConstraints {
        hole_clearance: Some(0),
        ..Default::default()
    };
    let mut violations = Vec::new();
    copper::run(&mut board, &constraints, &mut violations);
    let holes: Vec<_> = violations
        .iter()
        .filter(|v| v.kind == DrcViolationKind::HoleClearance)
        .collect();
    assert_eq!(holes.len(), 1);
    assert_eq!(holes[0].actual, 0.0);
}

#[test]
fn diagonal_hole_spacing_uses_circular_drills() {
    for (distance, minimum, expected_count) in [(1.53, 0.5, 0), (1.53, 0.55, 1), (0.9, 0.0, 1)] {
        let angle = std::f64::consts::PI / 8.0;
        let json = serde_json::json!({"layers":[{"name":"F.Cu"},{"name":"B.Cu"}],
        "components":[{"reference":"H","position":{"x":5,"y":5},"pads":[
            {"name":"1","shape":"circle","size":{"x":1,"y":1},"drill":1,"nonPlated":true,"layers":["F.Cu","B.Cu"]},
            {"name":"2","shape":"circle","size":{"x":1,"y":1},"drill":1,"nonPlated":true,
                "offset":{"x":distance*angle.cos(),"y":distance*angle.sin()},"layers":["F.Cu","B.Cu"]}
        ]}]});
        let mut board = kicad_board(&json.to_string());
        let constraints = DrcConstraints {
            hole_to_hole: Some((minimum * 10000.0) as i32),
            ..Default::default()
        };
        let mut out = Vec::new();
        holes::run(&mut board, &constraints, &mut out);
        assert_eq!(out.len(), expected_count, "{distance} {minimum}: {out:?}");
        if let Some(v) = out.first() {
            assert!((v.actual / 10000.0 - (distance - 1.0).max(0.0)).abs() < 0.0002);
        }
    }
}

#[test]
fn a_multisegment_trace_reports_the_nearest_hole_gap_once_in_both_item_orders() {
    for trace_first in [false, true] {
        let mut s = SyntheticBoard::new(&[], 2, 100);
        if !trace_first {
            s.via(0, 0, 1);
        }
        s.trace(
            &[(-10000, 4200), (10000, 4200), (10000, 4000), (-10000, 4000)],
            0,
            300,
            2,
        );
        if trace_first {
            s.via(0, 0, 1);
        }
        let mut constraints = constraints_with(100);
        constraints.hole_clearance = Some(2500);
        let mut out = Vec::new();
        copper::run(&mut s.board, &constraints, &mut out);
        let holes: Vec<_> = out
            .iter()
            .filter(|v| v.kind == DrcViolationKind::HoleClearance)
            .collect();
        assert_eq!(holes.len(), 1, "{trace_first}: {out:?}");
        assert!((holes[0].actual - 2200.0).abs() < 1.0, "{holes:?}");
    }
}
