mod common;

use common::synthetic::{PadSpec, SyntheticBoard};
use fr_board::DrcConstraints;
use fr_drc::checks::edge;
use fr_drc::checks::geometry::{gap_below, hole_of, is_microvia, is_through_hole_pin, item_shapes};
use fr_drc::checks::{copper, holes, single};
use fr_drc::{DesignRulesChecker, DrcSeverity, DrcViolation, DrcViolationKind};
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
fn a_thin_trace_is_a_track_width_violation() {
    let mut synthetic = SyntheticBoard::new(&[], 1, 2000);
    synthetic.trace(&[(0, 0), (10_000, 0)], 0, 500, 1);
    let mut constraints = DrcConstraints::default();
    constraints
        .netclass_track_width
        .insert("Default".to_string(), 2000);
    let mut out = Vec::new();
    single::run(&mut synthetic.board, &constraints, &mut out);
    assert_eq!(kinds(&out), vec![DrcViolationKind::TrackWidth]);
    assert_eq!(out[0].expected, 2000.0);
    assert_eq!(out[0].actual, 1000.0);
    assert_eq!(out[0].second_item, None);
}

#[test]
fn the_board_minimum_floors_the_netclass_width() {
    let mut synthetic = SyntheticBoard::new(&[], 1, 2000);
    synthetic.trace(&[(0, 0), (10_000, 0)], 0, 1000, 1);
    let mut constraints = DrcConstraints::default();
    constraints
        .netclass_track_width
        .insert("Default".to_string(), 1000);
    constraints.min_track_width = Some(2500);
    let mut out = Vec::new();
    single::run(&mut synthetic.board, &constraints, &mut out);
    assert_eq!(kinds(&out), vec![DrcViolationKind::TrackWidth]);
    assert_eq!(out[0].expected, 2500.0);
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
    constraints
        .netclass_track_width
        .insert("Default".to_string(), 1000);
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
