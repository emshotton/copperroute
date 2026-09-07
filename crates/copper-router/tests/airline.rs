use std::collections::BTreeSet;

use copper_board::items::Item;
use copper_board::prelude::*;
use copper_geometry::{Area, IntBox, IntOctagon, IntPoint, Point, Polyline, Shape, TileShape};
use copper_router::pipeline::calculate_item_distance;

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

    for id in [via_a, via_b, trace_t] {
        assert_eq!(
            board.connected_set(id, 1, false),
            [id].into_iter().collect::<BTreeSet<ItemId>>(),
            "the fixture's net-1 items must not touch"
        );
    }

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
    assert_eq!(
        calculate_item_distance(&board, via_c),
        0.0,
        ":173 — net 2 holds only C, so getUnconnectedSet is empty"
    );
    assert_eq!(
        calculate_item_distance(&board, trace_u),
        20_000_000.0_f64.sqrt(),
        ":209-211 — U's midpoint is (5000, 7000), and D is at (9000, 5000)"
    );

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
