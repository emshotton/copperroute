//! Plan 9 Task 10: quirk #174 — `TraceShover.check`'s via arm and the stale
//! `shoveFailingObstacle`.
//!
//! `shoveFailingObstacle` (`RoutingBoard.java:72`) is not a diagnostic. `MazeRipupResolver` reads
//! it to decide **what copper to tear up**, so a wrong value there is a wrong ripup — the router
//! removing a trace that had nothing to do with the failure. Java left it wrong in two ways at
//! once:
//!
//! * `TraceShover.check`'s via arm (`TraceShover.java:348-350`) — every candidate centre from
//!   `tryShoveViaPoints` failed `DrillItemMover.check` — is a bare `return false` that sets
//!   nothing, where every other refusal in the method records its culprit (`:251` the outline,
//!   `:263`/`:306`/`:357` `shapeEntries.getFoundObstacle()`, `:318` the via itself one branch up);
//! * the field is **never cleared on entry**, so whatever it held from an earlier call survives —
//!   possibly an item from a different `check` on a different net — and on a fresh board it is
//!   simply null.
//!
//! Both are fixed at Plan 9 Task 10, and the two tests below are one per half. Each pre-loads the
//! field with a sentinel that no `check` on this board could ever legitimately answer, so
//! "unchanged" and "correct" cannot be confused.
//!
//! # The fixture
//!
//! `boxed_in_via_board()`: a two-layer board holding one **foreign-net** through via at the
//! origin, penned in by four **shove-fixed** net-2 traces at +/-700. The pen is sized against the
//! numbers rather than by eye:
//!
//! * the check window is the 300x300 box at the origin. Expanded by the 200-unit clearance it
//!   reaches +/-350, which clears the pen's inner edge at 600 — so `storeItems` does **not**
//!   refuse first with one of the fixed traces as the obstacle, and the via arm is genuinely the
//!   arm under test;
//! * `tryShoveViaPoints` answers the four centres `(+/-468, 0)` and `(0, +/-468)`. A via moved to
//!   any of them spans 368..568, and with clearance 168..768, which overlaps the pen at 600..800.
//!   The pen is `ShoveFixed`, so it cannot yield: all four candidates fail and the arm is reached.
//!
//! Both facts are asserted in the tests rather than trusted, so a change to `tryShoveViaPoints`
//! that emptied the candidate list would not quietly turn these into vacuous passes.

use fr_board::ids::ItemId;
use fr_board::prelude::*;
use fr_geometry::{IntBox, IntPoint, Point, Polyline, Shape, TileShape};
use fr_router::board_ext::{DrillItemMover, TraceShover};

const BOUNDING_BOX: IntBox = IntBox {
    ll: IntPoint {
        x: -10_000,
        y: -10_000,
    },
    ur: IntPoint {
        x: 10_000,
        y: 10_000,
    },
};

/// An id no item on this board has. Pre-loaded into `shove_failing_obstacle` so that a test can
/// tell "the fix wrote the culprit" from "Java's leftover happened to look right".
const SENTINEL: ItemId = ItemId(999);

/// The window `check` is asked to clear: the 300x300 box at the origin, which contains the via
/// and clears the pen.
const WINDOW: TileShape = TileShape::Box(IntBox {
    ll: IntPoint { x: -150, y: -150 },
    ur: IntPoint { x: 150, y: 150 },
});

fn layers() -> LayerStructure {
    LayerStructure::new(vec![Layer::new("front", true), Layer::new("back", true)])
}

/// See the module docs for the geometry and why each number is what it is.
fn boxed_in_via_board(pen: bool) -> (Board, ItemId) {
    let mut padstacks = Padstacks::new(layers());
    let thru_shape = Shape::Tile(TileShape::Box(IntBox::from_coords(-100, -100, 100, 100)));
    let thru = padstacks.add(
        "thru",
        vec![Some(thru_shape.clone()), Some(thru_shape)],
        true,
        false,
    );
    let clearance_matrix = ClearanceMatrix::get_default_instance(&layers(), 200);
    let mut rules = BoardRules::new(layers(), clearance_matrix);
    rules.trace_angle_restriction = AngleRestriction::None;
    let mut board = Board::new(
        Vec::new(),
        0,
        BOUNDING_BOX,
        rules,
        BoardLibrary::new(padstacks, Packages::new()),
        Components::new(),
        Communication::default(),
    );
    let default_class = board.rules.get_default_net_class();
    board.rules.nets.add("N1", 1, false, default_class);
    board.rules.nets.add("N2", 1, false, default_class);
    board.rules.nets.add("N3", 1, false, default_class);

    let via = board
        .insert_via(
            thru,
            Point::new(0, 0),
            vec![3],
            1,
            FixedState::Unfixed,
            false,
        )
        .expect("the foreign-net via inserts");

    if pen {
        // Four shove-fixed net-2 traces at +/-700, half width 100 — spans 600..800, outside the
        // window's clearance reach and inside every candidate centre's.
        for corners in [
            [(-1200, -700), (1200, -700)],
            [(700, -1200), (700, 1200)],
            [(1200, 700), (-1200, 700)],
            [(-700, 1200), (-700, -1200)],
        ] {
            board.insert_trace_without_cleaning(
                Polyline::from_points(&corners.map(|(x, y)| Point::new(x, y))),
                0,
                100,
                vec![2],
                1,
                FixedState::ShoveFixed,
            );
        }
    }
    (board, via)
}

/// `TraceShover::check` over [`WINDOW`] on net 1 — a net neither the via nor the pen carries, so
/// the via is a foreign-net obstacle and reaches the via arm.
///
/// `max_via_recursion_depth` is **5**, deliberately: at `0` the refusal would come from `:317-320`,
/// which already named the via in Java, and the test would prove nothing.
fn check_the_window(board: &mut Board) -> bool {
    TraceShover::check(board, &WINDOW, None, None, 0, &[1], 1, 20, 5, 20, None)
}

/// Quirk #174, first half, fixed at Plan 9 Task 10: the via arm records its culprit.
///
/// The refusal is `TraceShover.java:348-350` — every candidate centre failed
/// `DrillItemMover.check` — and Java returned `false` from it without touching
/// `shoveFailingObstacle`, so `MazeRipupResolver` was handed whatever the field already held. It
/// now holds `currentShoveVia`, exactly as `:318` already writes it one branch up.
///
/// **Recorded before the fix, and the answer is the row's claim demonstrated rather than argued:**
/// this test answered `Some(ItemId(4))` — a **shove-fixed net-2 trace of the pen**, written by an
/// inner `DrillItemMover::check` on its way to refusing, and belonging to neither the net being
/// checked nor the via that actually blocked. Not the sentinel, and not the via. That is the item
/// `MazeRipupResolver` would have torn up.
#[test]
fn a_failed_via_shove_names_its_own_obstacle() {
    let (mut board, via) = boxed_in_via_board(true);

    // The arm under test is reachable and is the one that refuses — asserted, not assumed.
    let candidates = DrillItemMover::try_shove_via_points(&mut board, &WINDOW, 0, via, 1, true);
    assert_eq!(
        candidates.len(),
        4,
        "the via arm is only interesting when there are candidate centres to fail: {candidates:?}"
    );

    board.set_shove_failing_obstacle(Some(SENTINEL));
    assert!(
        !check_the_window(&mut board),
        "the penned-in via cannot be shoved anywhere, so the check must refuse"
    );
    assert_eq!(
        board.get_shove_failing_obstacle(),
        Some(via),
        "the refusal must name the via it could not move — quirk #174. A `Some(ItemId(999))` \
         here is Java's behaviour: the field left exactly as an earlier, unrelated call wrote it."
    );
}

/// Quirk #174, second half, fixed at Plan 9 Task 10: the field is cleared on entry.
///
/// Java writes `shoveFailingObstacle` only at a refusal and never clears it, so a `check` that
/// **succeeds** leaves the previous call's culprit in place — and a caller that reads the field
/// without first checking the return value tears up an item belonging to another net, or
/// dereferences a null on a fresh board.
///
/// Same fixture without the pen, so the via has somewhere to go and the check succeeds.
#[test]
fn the_failing_obstacle_is_cleared_on_entry() {
    let (mut board, via) = boxed_in_via_board(false);

    board.set_shove_failing_obstacle(Some(SENTINEL));
    assert!(
        check_the_window(&mut board),
        "with nothing penning it in, the via shoves and the check succeeds"
    );
    assert_eq!(
        board.get_shove_failing_obstacle(),
        None,
        "a successful check must not leave an earlier call's culprit behind — quirk #174"
    );

    // And the clear is on *entry*, not on success: a refusal still leaves this call's own
    // culprit, never the sentinel.
    let (mut board, via_penned) = boxed_in_via_board(true);
    board.set_shove_failing_obstacle(Some(SENTINEL));
    assert!(!check_the_window(&mut board));
    assert_eq!(board.get_shove_failing_obstacle(), Some(via_penned));
    assert_eq!(via, via_penned, "the two fixtures number the via the same");
}
