mod board_builder;

use fr_board::prelude::*;
use fr_board::{Item, ShapeTraceEntries};
use fr_geometry::{Area, IntBox, Point, Polyline, Shape, TileShape};

/// The window `store_items` is asked to clear, and the items that overlap it. Every test here
/// uses the same pair, so a difference in an answer is a difference in the predicate.
fn window_and_overlaps(board: &mut Board) -> (TileShape, Vec<ItemId>) {
    let shape = TileShape::Box(IntBox::from_coords(-500, -500, 500, 500));
    let overlaps = board.overlapping_items_with_clearance(&shape, Some(0), &[1], 1);
    (shape, overlaps)
}

// -------------------------------------------------------------------------------------------------
// #65
// -------------------------------------------------------------------------------------------------

#[test]
fn a_component_keepout_blocks_a_via() {
    let mut board = board_builder::shove_board();
    let keepout = board.insert_component_obstacle(
        Area::Shape(Shape::Tile(TileShape::Box(IntBox::from_coords(
            -400, -400, 400, 400,
        )))),
        0,
        1,
        FixedState::Unfixed,
    );
    assert!(
        matches!(
            board.get_item(keepout),
            Some(Item::ComponentObstacleArea(_))
        ),
        "the fixture must actually hold a ComponentObstacleArea, not an ObstacleArea — the two \
         take different arms of the `continue` and only one of them is this row"
    );
    let (shape, overlaps) = window_and_overlaps(&mut board);
    assert!(
        overlaps.contains(&keepout),
        "the keepout must reach store_items at all"
    );

    let mut entries = ShapeTraceEntries::new(shape.clone(), 0, vec![1], 1, None);
    assert!(
        entries.store_items(&board, &overlaps, false, false),
        "outside a pad check a component keepout is not an obstacle"
    );

    let mut entries = ShapeTraceEntries::new(shape, 0, vec![1], 1, None);
    assert!(
        !entries.store_items(&board, &overlaps, true, false),
        "a component keepout must block a via placement — quirk #65"
    );
    assert_eq!(
        entries.get_found_obstacle(),
        Some(keepout),
        "and it must name itself as the obstacle, so the ripup resolver tears up the right thing"
    );
}

#[test]
fn a_via_keepout_still_blocks_only_during_a_pad_check() {
    let mut board = board_builder::shove_board();
    let via_keepout = board.insert_via_obstacle(
        Area::Shape(Shape::Tile(TileShape::Box(IntBox::from_coords(
            -400, -400, 400, 400,
        )))),
        0,
        1,
        FixedState::Unfixed,
    );
    let (shape, overlaps) = window_and_overlaps(&mut board);
    assert!(overlaps.contains(&via_keepout));

    let mut entries = ShapeTraceEntries::new(shape.clone(), 0, vec![1], 1, None);
    assert!(entries.store_items(&board, &overlaps, false, false));

    let mut entries = ShapeTraceEntries::new(shape, 0, vec![1], 1, None);
    assert!(!entries.store_items(&board, &overlaps, true, false));
    assert_eq!(entries.get_found_obstacle(), Some(via_keepout));
}

// -------------------------------------------------------------------------------------------------
// #69
// -------------------------------------------------------------------------------------------------

/// Two traces of one foreign net that contact each other inside the window, with a `wide`
/// clearance class on the contact and the default class on the trace being stored.
///
/// The geometry is `storeTrace`'s `:355-439` arm: an **end point** of the stored trace lies
/// inside the offset shape, so the method walks that end's contact list and asks, per contact,
/// whether it blocks. `end_inside` is the corner both traces share; it sits well inside the
/// window so `containsInside` — the `&&`'s other half at `:382` — is unambiguously true.
fn contacting_traces_in_two_classes(contact_class: usize) -> (Board, ItemId, ItemId) {
    let mut board = board_builder::shove_board();
    let default_class = board.rules.get_default_net_class();
    board.rules.nets.add("N3", 1, false, default_class);
    let net = 3;

    // The stored trace: from outside the window to a corner inside it.
    let stored = board.insert_trace_without_cleaning(
        Polyline::from_points(&[Point::new(-2000, -2000), Point::new(100, 100)]),
        0,
        30,
        vec![net],
        1,
        FixedState::Unfixed,
    );
    // The contact: starts at the same corner and leaves the window the other way. Same half
    // width (30), so the *second* disjunct cannot be what refuses — only the clearance class
    // can, which is the whole point.
    let contact = board.insert_trace_without_cleaning(
        Polyline::from_points(&[Point::new(100, 100), Point::new(2000, 2000)]),
        0,
        30,
        vec![net],
        contact_class,
        FixedState::Unfixed,
    );
    (
        board,
        stored.expect("the stored trace inserts"),
        contact.expect("the contact trace inserts"),
    )
}

#[test]
fn a_contact_in_another_clearance_class_blocks_the_shove() {
    let wide = board_builder::WIDE_CLEARANCE_CLASS;
    assert_ne!(wide, 1, "the two classes must actually differ");

    // Same clearance class on both traces: nothing about the class can refuse.
    let (mut board, stored, contact) = contacting_traces_in_two_classes(1);
    let (shape, mut overlaps) = window_and_overlaps(&mut board);
    assert!(overlaps.contains(&stored) && overlaps.contains(&contact));
    // Store only the pair under test; the fixture's own three traces are a different net's and
    // would sort first, which would make the answer below about them instead.
    overlaps.retain(|id| *id == stored || *id == contact);
    let mut entries = ShapeTraceEntries::new(shape, 0, vec![1], 1, None);
    assert!(
        entries.store_items(&board, &overlaps, false, false),
        "two contacting traces in the same clearance class are shoveable"
    );

    // The same board with the contact moved to `wide`: the third disjunct fires.
    let (mut board, stored, contact) = contacting_traces_in_two_classes(wide);
    let (shape, mut overlaps) = window_and_overlaps(&mut board);
    overlaps.retain(|id| *id == stored || *id == contact);
    let mut entries = ShapeTraceEntries::new(shape, 0, vec![1], 1, None);
    assert!(
        !entries.store_items(&board, &overlaps, false, false),
        "a contact in a different clearance class must block the shove — quirk #69"
    );
    assert!(
        contact > stored,
        "the `wide` trace is inserted second, so it holds the higher id"
    );
    assert_eq!(
        entries.get_found_obstacle(),
        Some(stored),
        "descending-id order stores the `wide` trace first, so the trace it names is its \
         default-class contact"
    );
}
