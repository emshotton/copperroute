//! Plan 9 Task 10: the two `ShapeTraceEntries` precedence/self-comparison defects, #65 and #69.
//!
//! Both are decisions about **what blocks a shove**, and both failed open: Java's version of each
//! test could only ever answer "not an obstacle", so the router shoved copper past an obstacle it
//! had been asked to respect. They are the survey's two named live sources of clearance
//! violations in this group, which is why G2's "violations stay 0" line is the acceptance for
//! both.
//!
//! * **#65** — `storeItems`' first `continue` (`ShapeTraceEntries.java:180-183`) reads
//!   `!isPadCheck && a || b`, and Java's `&&` binds tighter than `||`, so a
//!   `ComponentObstacleArea` was skipped **unconditionally**. During a pad check — the arm that
//!   decides whether a via may be placed — a component keepout therefore never blocked anything.
//! * **#69** — `storeTrace`'s three-way block test (`:379-382`) compares
//!   `contactItem.clearanceClassIndex() != contactTrace.clearanceClassIndex()`, where
//!   `contactItem` **is** `contactTrace` (the pattern variable bound at `:377`). The disjunct
//!   compares an item with itself, so a contact whose clearance class differs never blocked.
//!
//! The board is `tests/board_builder.rs`'s `shove_board()` — the three-trace, two-net fixture
//! `P2T11.java` mode 5 builds, whose every number comes from the JVM driver.

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

/// Quirk #65, fixed at Plan 9 Task 10.
///
/// `ShapeTraceEntries.java:180-183` parses as `((!isPadCheck && a) || b)`, so the second arm — a
/// `ComponentObstacleArea` — short-circuits the `continue` for **every** call, pad check or not.
/// A component keepout is exactly a footprint's courtyard: the region a KiCad user has declared
/// nothing may be placed in. Java could not honour it, because the one arm that would have made
/// it an obstacle was unreachable.
///
/// The two halves are asserted against each other on one board, so the test cannot pass by the
/// keepout being invisible for some unrelated reason: **not** a pad check, the keepout is skipped
/// and the traces still sort; a pad check, and it blocks and names itself.
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

    // Not a pad check: the keepout is skipped, exactly as Java intended and as Java also did.
    // (`get_found_obstacle` is **not** asserted here: `ShapeTraceEntries.java:440` leaves the
    // last trace stored in that field even on success, so it is only meaningful beside a
    // `false` return.)
    let mut entries = ShapeTraceEntries::new(shape.clone(), 0, vec![1], 1, None);
    assert!(
        entries.store_items(&board, &overlaps, false, false),
        "outside a pad check a component keepout is not an obstacle"
    );

    // A pad check: it blocks. This is the arm Java's precedence made unreachable.
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

/// The `ViaObstacleArea` half of the same expression, unchanged by the fix.
///
/// It was already `!isPadCheck`-gated in Java — it is the arm the `&&` bound to — so the
/// parenthesisation moves it not at all. Asserted here so that a future edit to the same line
/// cannot quietly change one arm while the other's test looks green.
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

/// Quirk #69, fixed at Plan 9 Task 10.
///
/// The third disjunct now reads `trace.clearanceClassIndex() != contactTrace
/// .clearanceClassIndex()` — the symmetry the second disjunct
/// (`contactTrace.getHalfWidth() != trace.getHalfWidth()`) makes obvious, and the only reading
/// under which the line says anything at all.
///
/// Both directions are asserted on the same geometry, at the same half width, so the clearance
/// class is the *only* variable: same class, the shove proceeds; different class, it is refused
/// and the trace on the other side of the boundary is named. Java answered "proceed" in both,
/// which is how the router came to shove copper across a clearance-class boundary.
///
/// Which of the pair ends up in `foundObstacle` is decided by `getItems()` order (descending id,
/// quirk #63): the *later-inserted* trace is stored first, so the trace named is the one it found
/// as its contact — the `wide`-class trace's contact, i.e. the default-class one. That is a fact
/// about the walk, not about the predicate, and it is asserted so the test says which item the
/// ripup resolver would be handed rather than merely that something blocked.
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
