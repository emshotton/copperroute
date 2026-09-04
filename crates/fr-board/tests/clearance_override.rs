mod board_builder;

use board_builder::p2t11_board;
use fr_board::{
    BOARD_EDGE_CLEARANCE_CLASS_NAME, Board, DEFAULT_COPPER_TO_EDGE_CLEARANCE_UM,
    HOLE_EDGE_CLEARANCE_CLASS_NAME, ItemClass,
};

fn um_for(board: &Board, board_units: i32) -> f64 {
    let um = f64::from(board_units) * 25.4 / f64::from(board.communication.resolution.max(1));
    assert_eq!(
        board.clearance_override_board_units(um),
        board_units,
        "the fixture's unit/resolution pair changed under this helper"
    );
    um
}

fn outline_and_default_area_class(board: &mut Board) -> (usize, usize) {
    let outline = board.get_outline().expect("p2t11_board has an outline");
    let outline_class = board
        .get_item(outline)
        .expect("the outline is an item")
        .clearance_class();
    let default_net_class = board.rules.get_default_net_class();
    let default_area_class = board
        .rules
        .net_classes
        .get(default_net_class)
        .default_item_clearance_classes
        .get(ItemClass::Area);
    (outline_class, default_area_class)
}

#[test]
fn a_negative_copper_clearance_changes_nothing() {
    let mut board = p2t11_board();
    let before = board.rules.clearance_matrix.clone();
    assert!(!board.apply_copper_to_edge_clearance_override(-1.0));
    assert_eq!(board.rules.clearance_matrix, before);
}

#[test]
fn the_default_value_is_the_only_one_the_guard_can_ignore() {
    let mut board = p2t11_board();
    let outline = board.get_outline().expect("p2t11_board has an outline");
    assert!(board.change_clearance_class_index(outline, board_builder::WIDE_CLEARANCE_CLASS));
    let (outline_class, default_area_class) = outline_and_default_area_class(&mut board);
    assert_ne!(outline_class, default_area_class);

    let mut at_default = board.clone();
    let before = at_default.rules.clearance_matrix.clone();
    assert!(
        !at_default.apply_copper_to_edge_clearance_override(DEFAULT_COPPER_TO_EDGE_CLEARANCE_UM)
    );
    assert_eq!(at_default.rules.clearance_matrix, before);

    let mut off_default = board.clone();
    assert!(
        off_default
            .apply_copper_to_edge_clearance_override(DEFAULT_COPPER_TO_EDGE_CLEARANCE_UM + 1e-6)
    );
    assert!(
        off_default
            .rules
            .clearance_matrix
            .get_no(BOARD_EDGE_CLEARANCE_CLASS_NAME)
            .is_some()
    );
}

#[test]
fn an_existing_board_edge_class_is_reused_rather_than_appended() {
    let mut board = p2t11_board();
    assert!(
        board
            .rules
            .clearance_matrix
            .append_class(BOARD_EDGE_CLEARANCE_CLASS_NAME)
    );
    let class_count = board.rules.clearance_matrix.get_class_count();
    let board_edge = board
        .rules
        .clearance_matrix
        .get_no(BOARD_EDGE_CLEARANCE_CLASS_NAME)
        .expect("just appended");

    assert!(board.apply_copper_to_edge_clearance_override(0.0));
    assert_eq!(
        board.rules.clearance_matrix.get_class_count(),
        class_count,
        "the class already existed, so nothing is appended"
    );
    let outline = board.get_outline().expect("an outline");
    assert_eq!(
        board.get_item(outline).expect("an item").clearance_class(),
        board_edge
    );
}

#[test]
fn the_copper_override_writes_the_whole_row_and_column_on_every_layer() {
    let mut board = p2t11_board();
    let clearance_um = um_for(&board, 300);
    assert!(board.apply_copper_to_edge_clearance_override(clearance_um));
    let matrix = &board.rules.clearance_matrix;
    let board_edge = matrix
        .get_no(BOARD_EDGE_CLEARANCE_CLASS_NAME)
        .expect("appended");
    assert_eq!(board_edge, matrix.get_class_count() - 1, "appended last");
    for layer in 0..matrix.get_layer_count() {
        assert_eq!(matrix.get_value(board_edge, 0, layer, false), 0);
        assert_eq!(matrix.get_value(0, board_edge, layer, false), 0);
        for class_no in 1..matrix.get_class_count() {
            assert_eq!(matrix.get_value(board_edge, class_no, layer, false), 300);
            assert_eq!(matrix.get_value(class_no, board_edge, layer, false), 300);
        }
    }
}

#[test]
fn an_odd_board_unit_value_lands_in_the_matrix_rounded_up() {
    let mut board = p2t11_board();
    let clearance_um = um_for(&board, 301);
    assert!(board.apply_copper_to_edge_clearance_override(clearance_um));
    let matrix = &board.rules.clearance_matrix;
    let board_edge = matrix
        .get_no(BOARD_EDGE_CLEARANCE_CLASS_NAME)
        .expect("appended");
    assert_eq!(matrix.get_value(board_edge, 1, 0, false), 302);
}

#[test]
fn a_negative_hole_clearance_changes_nothing() {
    let mut board = p2t11_board();
    assert!(!board.apply_hole_clearance_override(-1.0));
    assert_eq!(board.rules.get_hole_clearance(), 0);
    assert!(
        board
            .rules
            .clearance_matrix
            .get_no(HOLE_EDGE_CLEARANCE_CLASS_NAME)
            .is_none()
    );
}

#[test]
fn the_hole_override_writes_unconditionally_and_reports_only_real_changes() {
    let mut board = p2t11_board();
    let clearance_um = um_for(&board, 40);
    assert!(board.apply_hole_clearance_override(clearance_um));
    assert_eq!(board.rules.get_hole_clearance(), 40);
    assert!(!board.apply_hole_clearance_override(clearance_um));
    assert_eq!(board.rules.get_hole_clearance(), 40);
}

#[test]
fn the_default_zero_hole_clearance_appends_nothing() {
    let mut board = p2t11_board();
    let class_count = board.rules.clearance_matrix.get_class_count();
    assert!(!board.apply_hole_clearance_override(0.0));
    assert_eq!(board.rules.get_hole_clearance(), 0);
    assert_eq!(board.rules.clearance_matrix.get_class_count(), class_count);
}

#[test]
fn a_board_without_circular_component_keepouts_gets_no_hole_edge_class() {
    let mut board = p2t11_board();
    assert!(!board.assign_hole_keepout_clearance_class(1000.0));
    assert!(
        board
            .rules
            .clearance_matrix
            .get_no(HOLE_EDGE_CLEARANCE_CLASS_NAME)
            .is_none()
    );
}
