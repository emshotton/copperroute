//! Plan 7 Task 15b: the arms of `HeadlessBoardManager`'s three clearance overrides that the
//! sixteen-board corpus does **not** reach.
//!
//! The corpus replay lives in `crates/fr-router/tests/clearance_override.rs`, where the
//! committed `P7T15bProbe` transcript pins every board's before/after state against the HEAD
//! jar. This file covers what no corpus board has:
//!
//! * a board that already declares a `board_edge` clearance class, so the
//!   `matrix.getNo(...) < 0` guard reuses it instead of appending (HeadlessBoardManager.java:519);
//! * an outline carrying an explicit, non-fallback clearance class — the `:501-507` guard's
//!   early return, which only `router-rpi-splitter` reaches on the corpus, together with the
//!   1e-9 discontinuity beside it;
//! * `ClearanceMatrix.setValue`'s odd → even rounding meeting an odd board-unit value;
//! * re-applying the same hole clearance twice, i.e. `changed == false` with no keepouts.
//!
//! The `Math.max` floor of `assignHoleKeepoutClearanceClass` and the interleaved read/write on
//! its last column are pinned against the jar instead, by the corpus replay's E and F variants
//! (100 µm, where the existing AREA clearance always wins, and 500 µm, where the floor bites).
//!
//! The board is `tests/board_builder.rs`'s `p2t11_board()` — the two-pin, two-trace, two-layer
//! fixture whose every number comes from `scripts/differential/java/P2T10.java`.

mod board_builder;

use board_builder::p2t11_board;
use fr_board::{
    BOARD_EDGE_CLEARANCE_CLASS_NAME, Board, DEFAULT_COPPER_TO_EDGE_CLEARANCE_UM,
    HOLE_EDGE_CLEARANCE_CLASS_NAME, ItemClass,
};

/// The µm value that converts to exactly `board_units` on this fixture, i.e. the inverse of
/// `HeadlessBoardManager.java:365-372`'s expression. `p2t11_board`'s [`fr_board::Communication`]
/// is `Unit::Mil` at resolution 1 (`Communication::default`), so one board unit is 25.4 µm — the
/// helper spells that out rather than hard-coding a magic literal per test.
fn um_for(board: &Board, board_units: i32) -> f64 {
    let um = f64::from(board_units) * 25.4 / f64::from(board.communication.resolution.max(1));
    assert_eq!(
        board.clearance_override_board_units(um),
        board_units,
        "the fixture's unit/resolution pair changed under this helper"
    );
    um
}

/// `p2t11_board`'s outline class and the default AREA class, read rather than assumed.
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

// -------------------------------------------------------------------------------------------------
// applyCopperToEdgeClearanceOverride
// -------------------------------------------------------------------------------------------------

/// HeadlessBoardManager.java:474-480 — a negative value warns and returns, touching nothing.
#[test]
fn a_negative_copper_clearance_changes_nothing() {
    let mut board = p2t11_board();
    let before = board.rules.clearance_matrix.clone();
    assert!(!board.apply_copper_to_edge_clearance_override(-1.0));
    assert_eq!(board.rules.clearance_matrix, before);
}

/// `:501-507`, the guard that decides everything (quirk #231): the default value is ignored only
/// when the outline carries an explicit, non-fallback class — and any other value fires
/// regardless.
#[test]
fn the_default_value_is_the_only_one_the_guard_can_ignore() {
    let mut board = p2t11_board();
    // The fixture's outline uses the fallback AREA class, i.e. the 15-of-16 corpus shape. Give
    // it the explicit `wide` class and it becomes the `router-rpi-splitter` shape — the one board
    // the guard can stop.
    let outline = board.get_outline().expect("p2t11_board has an outline");
    assert!(board.change_clearance_class_index(outline, board_builder::WIDE_CLEARANCE_CLASS));
    let (outline_class, default_area_class) = outline_and_default_area_class(&mut board);
    assert_ne!(outline_class, default_area_class);

    // The default value + an explicit outline class: the one early return.
    let mut at_default = board.clone();
    let before = at_default.rules.clearance_matrix.clone();
    assert!(
        !at_default.apply_copper_to_edge_clearance_override(DEFAULT_COPPER_TO_EDGE_CLEARANCE_UM)
    );
    assert_eq!(at_default.rules.clearance_matrix, before);

    // A hair off the default, on the same board: it fires.
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

/// `:518-528`: a board that already declares the class reuses it, so the class count does not
/// grow. No corpus DSN declares one, which is why this arm needs a hand-built board.
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

/// `:536-541`: the row **and** the column, on **every** layer, and the `[board_edge][board_edge]`
/// diagonal — but never class 0, whose loop bound starts at 1.
#[test]
fn the_copper_override_writes_the_whole_row_and_column_on_every_layer() {
    let mut board = p2t11_board();
    // 300 board units: distinguishable from every pre-existing entry (200, 600, 800).
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

/// `ClearanceMatrix.setValue` rounds an odd value **up** to an even one
/// (ClearanceMatrix.java:106-113), so an odd board-unit conversion is not what lands in the
/// matrix. Java has the same gap and the port must not "fix" it.
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

// -------------------------------------------------------------------------------------------------
// applyHoleClearanceOverride / assignHoleKeepoutClearanceClass
// -------------------------------------------------------------------------------------------------

/// `:354-359`.
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

/// `:373-374`: the write is unconditional and `changed` is read before it, so re-applying the
/// same value is a no-op that still runs the setter.
#[test]
fn the_hole_override_writes_unconditionally_and_reports_only_real_changes() {
    let mut board = p2t11_board();
    let clearance_um = um_for(&board, 40);
    assert!(board.apply_hole_clearance_override(clearance_um));
    assert_eq!(board.rules.get_hole_clearance(), 40);
    // Second application: the same value, so `changed` is false and — this fixture having no
    // circular component keepouts — `holeKeepouts` is 0 too.
    assert!(!board.apply_hole_clearance_override(clearance_um));
    assert_eq!(board.rules.get_hole_clearance(), 40);
}

/// `:376-378`: the reclassifier runs only above zero, so the default `0.0 µm` never appends
/// `hole_edge`. This is why six plans' worth of parity held without the override.
#[test]
fn the_default_zero_hole_clearance_appends_nothing() {
    let mut board = p2t11_board();
    let class_count = board.rules.clearance_matrix.get_class_count();
    assert!(!board.apply_hole_clearance_override(0.0));
    assert_eq!(board.rules.get_hole_clearance(), 0);
    assert_eq!(board.rules.clearance_matrix.get_class_count(), class_count);
}

/// `:424-426`: a board with no circular component keepout gets no `hole_edge` class at all, no
/// matter how large the clearance.
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
