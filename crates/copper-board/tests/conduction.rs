mod board_builder;

use copper_board::prelude::*;
use copper_board::{Item, ShapeTraceEntries};
use copper_geometry::{
    Area, IntBox, Point, PolygonShape, Polyline, PolylineShapeRef, Shape, TileShape,
};

fn pour_board() -> (Board, ItemId, ItemId) {
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

    // The pour: net 1, layer 0 (a **signal** layer), an obstacle as constructed.
    let pour = board.insert_conduction_area(
        Area::Shape(Shape::Tile(TileShape::Box(IntBox::from_coords(
            -4000, -4000, 4000, 4000,
        )))),
        0,
        vec![1],
        1,
        true,
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
fn a_signal_layer_pour_obstructs_a_foreign_net() {
    let (mut board, pour, foreign) = pour_board();

    assert!(board.rules.get_ignore_conduction());
    assert!(is_obstacle(&board, pour));
    assert!(the_pour_obstructs_the_foreign_net(&mut board, pour));

    board.change_conduction_is_obstacle(false);
    assert!(!is_obstacle(&board, pour), "quirk #50");
    assert!(
        !the_pour_obstructs_the_foreign_net(&mut board, pour),
        "a pour that is not an obstacle must not block a foreign net — quirk #50"
    );
    assert!(
        board.rules.get_ignore_conduction(),
        "`ignoreConduction` is the negation of `isObstacle`, so it stays true here"
    );

    board.change_conduction_is_obstacle(false);
    assert!(!is_obstacle(&board, pour));
    assert!(board.rules.get_ignore_conduction());

    // And back again.
    board.change_conduction_is_obstacle(true);
    assert!(is_obstacle(&board, pour));
    assert!(
        the_pour_obstructs_the_foreign_net(&mut board, pour),
        "and an obstacle pour blocks it again"
    );
    assert!(
        !board.rules.get_ignore_conduction(),
        "the board-level flag mirrors the per-item one, negated, on every call"
    );

    board.change_conduction_is_obstacle(true);
    assert!(is_obstacle(&board, pour));
    assert!(!board.rules.get_ignore_conduction());

    // The pour's own net is never obstructed by it, before or after — `storeItems:185-187`'s
    // `containsOwnNet` arm, which the fix does not touch.
    let shape = TileShape::Box(IntBox::from_coords(-500, -500, 500, 500));
    let mut entries = ShapeTraceEntries::new(shape, 0, vec![1], 1, None);
    assert!(entries.store_items(&board, &[pour], false, false));
    let _ = foreign;
}

#[test]
fn the_conduction_flag_mirrors_the_per_item_state_from_either_start() {
    for start in [true, false] {
        let (mut board, pour, _) = pour_board();
        board.rules.set_ignore_conduction(start);
        for request in [false, false, true, true, false, true] {
            board.change_conduction_is_obstacle(request);
            assert_eq!(
                is_obstacle(&board, pour),
                request,
                "start={start}: the pour must carry what was last requested"
            );
            assert_eq!(
                board.rules.get_ignore_conduction(),
                !request,
                "start={start}: and the board-level flag must be its negation"
            );
        }
    }
}

#[test]
fn unfill_and_change_agree_on_what_the_pair_means() {
    let (mut unfilled, pour, _) = pour_board();
    unfilled.unfill_conduction_areas();

    let (mut changed, _, _) = pour_board();
    changed.change_conduction_is_obstacle(false);

    assert_eq!(
        (
            unfilled.rules.get_ignore_conduction(),
            is_obstacle(&unfilled, pour)
        ),
        (true, false)
    );
    assert_eq!(
        (
            changed.rules.get_ignore_conduction(),
            is_obstacle(&changed, pour)
        ),
        (true, false)
    );
}
