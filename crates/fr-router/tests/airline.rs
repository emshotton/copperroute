//! Plan 9 Task 2 (R1, register row #293) — `AutorouteAirlineCalculator.calculateItemDistance`
//! (`:162-177`) and the two private helpers it reaches, `calculateMinDistance` (`:179-202`) and
//! `getItemReferencePoint` (`:204-213`).
//!
//! These three were rostered `// not ported:` by Plan 7 on correct evidence — a tree-wide grep
//! finds no caller for any of them at HEAD — and the absent caller is exactly the regression
//! `benchmark/reports/java-regressions-2026-09.md` §"Regression 1" measures. Task 2 ports them
//! and gives them their caller back, so they need a test of the **formula** rather than of the
//! sort that consumes it; the sort's own tests are in `batch_autorouter.rs`.
//!
//! # The expectations are hand-computed, not read from the jar (recommendation 9)
//!
//! Every number below is derived from the Java source in this file's doc comments and checked by
//! hand against the board the test builds. Nothing here was produced by running the program:
//! `calculateItemDistance` has no caller in the jar, so there is no jar transcript to read, and
//! a probe written to create one would be testing a driver rather than the method.
//!
//! # Why the fixture is via / via / trace rather than pin / via / trace
//!
//! `getItemReferencePoint`'s first arm is `item instanceof DrillItem` (`:205-206`), and the port
//! dispatches it through [`fr_board::Board::drill_center`], whose whole job is the `Via`/`Pin`
//! split and which `crates/fr-board/tests/board.rs` already pins on both. A `Pin` here would
//! reach the identical line through the identical call and would cost this file a package
//! library, a component and a placement to say so. The three arms this file must distinguish are
//! **drill / trace / neither**, and a via, a trace and an obstacle area distinguish them.

use std::collections::BTreeSet;

use fr_board::items::Item;
use fr_board::prelude::*;
use fr_geometry::{Area, IntBox, IntOctagon, IntPoint, Point, Polyline, Shape, TileShape};
use fr_router::pipeline::calculate_item_distance;

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

fn layers() -> LayerStructure {
    LayerStructure::new(vec![Layer::new("front", true), Layer::new("back", true)])
}

fn p(x: i32, y: i32) -> Point {
    Point::new(x, y)
}

/// A two-layer board with one through-via padstack, so `insert_via` has something to place.
fn via_board() -> Board {
    let clearance_matrix = ClearanceMatrix::get_default_instance(&layers(), 200);
    let mut rules = BoardRules::new(layers(), clearance_matrix);
    rules.trace_angle_restriction = AngleRestriction::None;
    let mut padstacks = Padstacks::new(layers());
    let shape = Shape::Tile(TileShape::Octagon(IntOctagon::new(
        -100, -100, 100, 100, -200, 200, -200, 200,
    )));
    let via = padstacks.add("via", vec![Some(shape.clone()), Some(shape)], true, false);
    assert_eq!(PadstackId(1), via, "the port's padstack ids start at 1");
    Board::new(
        Vec::new(),
        0,
        BOUNDING_BOX,
        rules,
        BoardLibrary::new(padstacks, Packages::new()),
        Components::new(),
        Communication::default(),
    )
}

fn add_net(board: &mut Board, name: &str) {
    let default_class = board.rules.get_default_net_class();
    board.rules.nets.add(name, 0, false, default_class);
}

fn add_via(board: &mut Board, at: Point, net: i32) -> ItemId {
    board
        .insert_via(PadstackId(1), at, vec![net], 1, FixedState::Unfixed, true)
        .expect("the padstack spans both layers")
}

fn add_trace(board: &mut Board, corners: &[Point], nets: Vec<i32>) -> ItemId {
    board
        .insert_trace_without_cleaning(
            Polyline::from_points(corners),
            0,
            30,
            nets,
            1,
            FixedState::Unfixed,
        )
        .expect("a two-corner polyline always inserts")
}

/// `:162-213`, every arm, on one board.
///
/// # The board
///
/// Three nets, all on layer 0, none of whose items touch any other's (the via octagon is
/// `±100 / ±200` about its centre and the traces are half width 30, so 1 000 units of separation
/// is a wide margin; nothing here is in contact and every `getConnectedSet` is therefore the
/// singleton the item itself makes).
///
/// ```text
///   net 1   via A  (0, 0)          via B  (4000, 0)      trace T  (0, 1000) -> (2000, 1000)
///   net 2   via C  (0, -4000)      -- alone on its net
///   net 3   trace U (5000, 5000) -> (5000, 9000)         via D  (9000, 5000)
/// ```
///
/// # The hand-computed answers
///
/// `getItemReferencePoint` (`:204-213`) gives `A = (0, 0)`, `B = (4000, 0)`, `D = (9000, 5000)`
/// (`:206`, the drill centre) and `T = (1000, 1000)`, `U = (5000, 7000)` (`:209-211`, the mean of
/// the first and last corner — **not** the polyline's centroid).
///
/// * `calculateItemDistance(A)`: net 1, `unconnectedSet = {B, T}`, `connectedSet = {A}`, so
///   `calculateMinDistance({A}, {B, T})` = `min(|AB|, |AT|)` =
///   `min(4000, sqrt(1000² + 1000²))` = `sqrt(2 000 000)` = **1414.2135623730951** — the trace,
///   not the via, because `:191` measures the true distance and the trace's midpoint is nearer.
/// * `calculateItemDistance(B)` = `min(|BA|, |BT|)` = `min(4000, sqrt(3000² + 1000²))` =
///   `min(4000, sqrt(10 000 000) = 3162.27…)` = **3162.2776601683795**.
/// * `calculateItemDistance(C)`: net 2 holds only `C`, so `getUnconnectedSet` is empty and
///   `:173` answers exactly **0**.
/// * `calculateItemDistance(U)` = `|U D|` = `sqrt(4000² + 2000²)` = `sqrt(20 000 000)` =
///   **4472.13595499958** — the trace end, taken from its midpoint `(5000, 7000)`.
/// * An item on **no net** answers `Double.MAX_VALUE` (`:163-165`).
/// * An obstacle area is neither a `DrillItem` nor a `PolylineTrace`, so
///   `getItemReferencePoint` answers `null` (`:212`) and `calculateMinDistance` skips it — a set
///   of nothing but such items leaves `minDistance` at its `Double.MAX_VALUE` seed (`:180`,
///   `:201`).
#[test]
fn calculate_item_distance_matches_the_java_formula() {
    let mut board = via_board();
    add_net(&mut board, "N1");
    add_net(&mut board, "N2");
    add_net(&mut board, "N3");

    let via_a = add_via(&mut board, p(0, 0), 1);
    let via_b = add_via(&mut board, p(4000, 0), 1);
    let trace_t = add_trace(&mut board, &[p(0, 1000), p(2000, 1000)], vec![1]);
    let via_c = add_via(&mut board, p(0, -4000), 2);
    let trace_u = add_trace(&mut board, &[p(5000, 5000), p(5000, 9000)], vec![3]);
    add_via(&mut board, p(9000, 5000), 3);

    // Nothing is in contact, so every connected set is a singleton and every item of a net is in
    // every other item's unconnected set. If this ever fails the distances below are measuring a
    // different board.
    for id in [via_a, via_b, trace_t] {
        assert_eq!(
            board.connected_set(id, 1, false),
            [id].into_iter().collect::<BTreeSet<ItemId>>(),
            "the fixture's net-1 items must not touch"
        );
    }

    // `:176-177` with the closest reference point being the **trace's midpoint**, not the via.
    assert_eq!(
        calculate_item_distance(&board, via_a),
        2_000_000.0_f64.sqrt(),
        ":191 — min(|AB| = 4000, |AT| = sqrt(1000^2 + 1000^2))"
    );
    assert_eq!(
        calculate_item_distance(&board, via_b),
        10_000_000.0_f64.sqrt(),
        ":191 — min(|BA| = 4000, |BT| = sqrt(3000^2 + 1000^2))"
    );
    // `:171-174` — nothing unconnected, so exactly 0 and the item sorts first.
    assert_eq!(
        calculate_item_distance(&board, via_c),
        0.0,
        ":173 — net 2 holds only C, so getUnconnectedSet is empty"
    );
    // `:209-211` — the trace's own reference point is the midpoint of its **end corners**.
    assert_eq!(
        calculate_item_distance(&board, trace_u),
        20_000_000.0_f64.sqrt(),
        ":209-211 — U's midpoint is (5000, 7000), and D is at (9000, 5000)"
    );

    // `:163-165` — an item on no net at all.
    let netless = add_trace(&mut board, &[p(-8000, -8000), p(-6000, -8000)], Vec::new());
    assert_eq!(
        board.get_item(netless).expect("just inserted").net_count(),
        0
    );
    assert_eq!(
        calculate_item_distance(&board, netless),
        f64::MAX,
        ":164 — no net, no connections, so it sorts last"
    );

    // `:212` — a `ConductionArea` is `Connectable` (so it enters an unconnected set) and is
    // neither a `DrillItem` nor a `PolylineTrace`, so `getItemReferencePoint` answers `null` and
    // `calculateMinDistance` skips it. A pair whose only members are such items therefore leaves
    // `minDistance` at its `Double.MAX_VALUE` seed (`:180`, `:201`) — the one way this method
    // returns the seed with a *non-empty* unconnected set.
    add_net(&mut board, "N4");
    let pour = board.insert_conduction_area(
        Area::Shape(Shape::Tile(TileShape::Box(IntBox::from_coords(
            -9000, 2000, -8000, 3000,
        )))),
        0,
        vec![4],
        1,
        false,
        FixedState::Unfixed,
    );
    let other_pour = board.insert_conduction_area(
        Area::Shape(Shape::Tile(TileShape::Box(IntBox::from_coords(
            -9000, 6000, -8000, 7000,
        )))),
        0,
        vec![4],
        1,
        false,
        FixedState::Unfixed,
    );
    assert!(matches!(
        board.get_item(pour),
        Some(Item::ConductionArea(_))
    ));
    assert_eq!(
        board.unconnected_set(pour, 4),
        [other_pour].into_iter().collect::<BTreeSet<ItemId>>(),
        "the two pours are on net 4 and do not touch, so each is the other's unconnected set"
    );
    assert_eq!(
        calculate_item_distance(&board, pour),
        f64::MAX,
        ":180, :201 — both sides answer null at :212, so minDistance never leaves its seed"
    );
}
