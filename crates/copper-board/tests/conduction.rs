mod board_builder;

use copper_board::prelude::*;
use copper_board::{Item, ShapeTraceEntries};
use copper_geometry::{
    Area, IntBox, Point, PolygonShape, Polyline, PolylineShapeRef, Shape, TileShape,
};

fn pour_board(is_obstacle: bool) -> (Board, ItemId, ItemId) {
    let ls = LayerStructure::new(vec![
        Layer::new("front".to_string(), true),
        Layer::new("back".to_string(), true),
    ]);
    let cm = ClearanceMatrix::get_default_instance(&ls, 200);
    let mut rules = BoardRules::new(ls.clone(), cm);
    rules.create_default_net_class();
    let default_class = rules.get_default_net_class();

    let outline = vec![PolylineShapeRef::Polygon(PolygonShape::from_points(&[
        Point::new(-10_000, -10_000),
        Point::new(10_000, -10_000),
        Point::new(10_000, 10_000),
        Point::new(-10_000, 10_000),
    ]))];
    let mut board = Board::new(
        outline,
        0,
        IntBox::from_coords(-20_000, -20_000, 20_000, 20_000),
        rules,
        BoardLibrary::new(Padstacks::new(ls), Packages::new()),
        Components::new(),
        Communication::default(),
    );
    board.rules.nets.add("GND", 1, false, default_class);
    board.rules.nets.add("SIG", 1, false, default_class);

    // The pour: net 1, layer 0 (a **signal** layer).
    let pour = board.insert_conduction_area(
        Area::Shape(Shape::Tile(TileShape::Box(IntBox::from_coords(
            -4000, -4000, 4000, 4000,
        )))),
        0,
        vec![1],
        1,
        is_obstacle,
        FixedState::Unfixed,
    );
    // The foreign-net trace: net 2, straight across the pour on the same layer.
    let foreign = board
        .insert_trace_without_cleaning(
            Polyline::from_points(&[Point::new(-8000, 0), Point::new(8000, 0)]),
            0,
            100,
            vec![2],
            1,
            FixedState::Unfixed,
        )
        .expect("the foreign-net trace inserts");

    assert!(
        board.rules.layer_structure().layers[0].is_signal,
        "the pour must be on a signal layer — the non-signal case is a separate question the fix \
         deliberately leaves open"
    );
    (board, pour, foreign)
}

/// Whether the pour obstructs a foreign net, asked the way the shove path asks it:
/// `ShapeTraceEntries::store_items` over a window on the pour, with the **foreign** net as the own
/// net. `:185-187` excuses a conduction area that is not an obstacle; anything else falls through
/// to `:211-214` and refuses.
fn the_pour_obstructs_the_foreign_net(board: &mut Board, pour: ItemId) -> bool {
    let shape = TileShape::Box(IntBox::from_coords(-500, -500, 500, 500));
    let overlaps = board.overlapping_items_with_clearance(&shape, Some(0), &[], 1);
    assert!(
        overlaps.contains(&pour),
        "the window must reach the pour at all"
    );
    let mut entries = ShapeTraceEntries::new(shape, 0, vec![2], 1, None);
    // Only the pour: the foreign trace is shoveable and would answer for itself.
    let just_the_pour = vec![pour];
    let _ = overlaps;
    !entries.store_items(board, &just_the_pour, false, false)
}

fn is_obstacle(board: &Board, id: ItemId) -> bool {
    match board.get_item(id) {
        Some(Item::ConductionArea(area)) => area.get_is_obstacle(),
        other => panic!("{id:?} is not a conduction area: {other:?}"),
    }
}

#[test]
fn a_signal_layer_pour_obstructs_a_foreign_net_only_when_it_is_an_obstacle() {
    let (mut board, pour, foreign) = pour_board(true);
    assert!(is_obstacle(&board, pour));
    assert!(the_pour_obstructs_the_foreign_net(&mut board, pour));

    let (mut board, pour, _) = pour_board(false);
    assert!(!is_obstacle(&board, pour));
    assert!(
        !the_pour_obstructs_the_foreign_net(&mut board, pour),
        "a pour that is not an obstacle must not block a foreign net"
    );

    for obstacle in [true, false] {
        let (board, pour, _) = pour_board(obstacle);
        let shape = TileShape::Box(IntBox::from_coords(-500, -500, 500, 500));
        let mut entries = ShapeTraceEntries::new(shape, 0, vec![1], 1, None);
        assert!(entries.store_items(&board, &[pour], false, false));
    }
    let _ = foreign;
}
