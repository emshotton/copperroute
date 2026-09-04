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
//!   *(Its test lands with the fix, in the next commit.)*
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
