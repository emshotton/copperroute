use copper_board::prelude::*;
use copper_drc::constraints::{
    apply_kicad_project, canonical_class_name, from_dsn, from_kicad_project, pair_clearance,
    search_radius, severity, track_width_min,
};
use copper_drc::{DrcError, DrcViolationKind};
use copper_dsn::{BoardReadResult, DsnReadOptions};

fn spike_board() -> Board {
    let path = parity::workspace_root().join("benchmark/tests/data/spike/spike.dsn");
    let bytes =
        std::fs::read(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    match copper_dsn::read_board(&bytes[..], None, Some("spike"), &DsnReadOptions::default()) {
        BoardReadResult::Success { board, .. } | BoardReadResult::OutlineMissing { board, .. } => {
            *board.expect("the spike DSN produces a board")
        }
        other => panic!("the spike DSN did not read: {other:?}"),
    }
}

#[test]
fn the_default_class_has_two_names() {
    assert_eq!(canonical_class_name("kicad_default"), "Default");
    assert_eq!(canonical_class_name("Default"), "Default");
    assert_eq!(canonical_class_name("Power"), "Power");
}

#[test]
fn from_dsn_reads_the_kicad_default_class_clearance_and_width() {
    let board = spike_board();
    let constraints = from_dsn(&board);
    assert_eq!(constraints.netclass_clearance.get("Default"), Some(&2000));
    assert_eq!(constraints.netclass_track_width.get("Default"), Some(&2000));
    assert_eq!(constraints.min_clearance, None);
    assert_eq!(constraints.hole_to_hole, None);
}

#[test]
fn from_dsn_sets_the_kicad_drc_epsilon() {
    let board = spike_board();
    let constraints = from_dsn(&board);
    assert_eq!(constraints.epsilon, 5);
}

#[test]
fn from_kicad_project_sets_the_kicad_drc_epsilon() {
    let (board, transform) = spike_board_with_transform();
    let constraints = from_kicad_project(&spike_project(), &board, &transform).expect("parses");
    assert_eq!(constraints.epsilon, 5);
}

#[test]
fn pair_clearance_is_the_larger_netclass_value_floored_by_the_board_minimum() {
    let board = spike_board();
    let mut constraints = from_dsn(&board);
    constraints
        .netclass_clearance
        .insert("Power".to_string(), 3000);
    let items: Vec<&Item> = board.get_items().collect();
    let a = items
        .iter()
        .find(|item| item.net_count() > 0)
        .expect("a netted item");
    assert_eq!(pair_clearance(&board, &constraints, a, a), Some(2000));
    constraints.min_clearance = Some(2500);
    assert_eq!(pair_clearance(&board, &constraints, a, a), Some(2500));
}

#[test]
fn track_width_minimum_and_severity_default() {
    let board = spike_board();
    let mut constraints = from_dsn(&board);
    let net = board
        .rules
        .nets
        .iter()
        .next()
        .expect("the spike has nets")
        .net_number;
    assert_eq!(track_width_min(&board, &constraints, net), None);
    constraints.min_track_width = Some(2200);
    assert_eq!(track_width_min(&board, &constraints, net), Some(2200));
    assert_eq!(
        severity(&constraints, DrcViolationKind::Clearance),
        DrcSeverity::Error
    );
    constraints
        .severities
        .insert("clearance".to_string(), DrcSeverity::Ignore);
    assert_eq!(
        severity(&constraints, DrcViolationKind::Clearance),
        DrcSeverity::Ignore
    );
    assert!(search_radius(&constraints) >= 2000);
}

fn spike_board_with_transform() -> (Board, copper_dsn::CoordinateTransform) {
    let path = parity::workspace_root().join("benchmark/tests/data/spike/spike.dsn");
    let bytes = std::fs::read(&path).expect("the spike DSN is in the repo");
    match copper_dsn::read_board(&bytes[..], None, Some("spike"), &DsnReadOptions::default()) {
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
            *board.expect("a board"),
            coordinate_transform.expect("a transform"),
        ),
        other => panic!("the spike DSN did not read: {other:?}"),
    }
}

fn spike_project() -> String {
    std::fs::read_to_string(
        parity::workspace_root().join("benchmark/tests/data/spike/stripped.kicad_pro"),
    )
    .expect("the spike project is in the repo")
}

#[test]
fn the_spike_project_rules_convert_to_board_units() {
    let (board, transform) = spike_board_with_transform();
    let constraints = from_kicad_project(&spike_project(), &board, &transform).expect("parses");
    assert_eq!(constraints.min_clearance, None);
    assert_eq!(constraints.min_track_width, None);
    assert_eq!(constraints.hole_clearance, Some(2500));
    assert_eq!(constraints.hole_to_hole, Some(2500));
    assert_eq!(constraints.copper_edge_clearance, Some(5000));
    assert_eq!(constraints.min_via_diameter, Some(5000));
    assert_eq!(constraints.min_via_annular_width, Some(1000));
    assert_eq!(constraints.min_through_hole_diameter, Some(3000));
    assert_eq!(constraints.min_microvia_diameter, Some(2000));
    assert_eq!(constraints.min_microvia_drill, Some(1000));
    assert_eq!(constraints.netclass_clearance.get("Default"), Some(&2000));
    assert_eq!(constraints.netclass_clearance.get("Power"), Some(&2000));
    assert_eq!(constraints.netclass_track_width.get("Power"), Some(&4000));
    assert_eq!(
        constraints.severities.get("clearance"),
        Some(&DrcSeverity::Error)
    );
}

#[test]
fn a_zero_hole_clearance_is_kept_and_other_zeros_are_dropped() {
    let (board, transform) = spike_board_with_transform();
    let json = r#"{"board":{"design_settings":{"rules":{"min_hole_clearance":0.0,"min_track_width":0.0,"min_hole_to_hole":0.0}}}}"#;
    let constraints = from_kicad_project(json, &board, &transform).expect("parses");
    assert_eq!(constraints.hole_clearance, Some(0));
    assert_eq!(constraints.min_track_width, None);
    assert_eq!(constraints.hole_to_hole, None);
}

#[test]
fn a_project_without_design_settings_is_an_error() {
    let (board, transform) = spike_board_with_transform();
    let error = from_kicad_project(r#"{"meta":{}}"#, &board, &transform).unwrap_err();
    assert!(matches!(error, DrcError::Project(_)), "{error}");
    let error = from_kicad_project("not json", &board, &transform).unwrap_err();
    assert!(matches!(error, DrcError::Json(_)), "{error}");
}

#[test]
fn apply_stores_the_merged_constraints_on_the_board() {
    let (mut board, transform) = spike_board_with_transform();
    assert!(board.rules.drc_constraints.is_none());
    apply_kicad_project(&spike_project(), &mut board, &transform).expect("applies");
    let stored = board.rules.drc_constraints.as_ref().expect("stored");
    assert_eq!(stored.hole_to_hole, Some(2500));
    assert_eq!(stored.netclass_track_width.get("Default"), Some(&2000));
}
