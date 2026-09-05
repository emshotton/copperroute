use fr_board::prelude::*;
use fr_drc::constraints::{canonical_class_name, from_dsn, pair_clearance, search_radius, severity, track_width_min};
use fr_drc::DrcViolationKind;
use fr_dsn::{BoardReadResult, DsnReadOptions};

fn spike_board() -> Board {
    let path = parity::workspace_root().join("benchmark/tests/data/spike/spike.dsn");
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    match fr_dsn::read_board(&bytes[..], None, Some("spike"), &DsnReadOptions::default()) {
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
fn pair_clearance_is_the_larger_netclass_value_floored_by_the_board_minimum() {
    let board = spike_board();
    let mut constraints = from_dsn(&board);
    constraints.netclass_clearance.insert("Power".to_string(), 3000);
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
    assert_eq!(track_width_min(&board, &constraints, net), Some(2000));
    constraints.min_track_width = Some(2200);
    assert_eq!(track_width_min(&board, &constraints, net), Some(2200));
    assert_eq!(severity(&constraints, DrcViolationKind::Clearance), DrcSeverity::Error);
    constraints
        .severities
        .insert("clearance".to_string(), DrcSeverity::Ignore);
    assert_eq!(severity(&constraints, DrcViolationKind::Clearance), DrcSeverity::Ignore);
    assert!(search_radius(&constraints) >= 2000);
}
