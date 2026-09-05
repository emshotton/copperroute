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

fn check_the_window(board: &mut Board) -> bool {
    TraceShover::check(board, &WINDOW, None, None, 0, &[1], 1, 20, 5, 20, None)
}

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
