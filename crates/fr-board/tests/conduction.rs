//! Plan 9 Task 10: quirk #50 — `RoutingBoard.changeConductionIsObstacle` and the flag that
//! decides whether copper pours obstruct foreign-net routing.
//!
//! This is the most user-visible boolean on a KiCad board with ground pours, and the per-item
//! `ConductionArea.isObstacle` flag is what carries it: `Item::is_obstacle`, `is_trace_obstacle`
//! and `is_drillable` all read it (Trace.java:98, Via.java:156, ConductionArea.java:381,398,403),
//! and `ShapeTraceEntries.storeItems` excuses a conduction area from being an obstacle when it is
//! clear (`:185-187`).
//!
//! # What Java does, and why it cannot be asked to stop
//!
//! `changeConductionIsObstacle(value)` guards on `if (getIgnoreConduction() != value) return;`
//! (`RoutingBoard.java:1254`). That reads the **board-level** flag as a proxy for the **per-item**
//! ones, and the two desynchronise: `BasicBoard.unfillConductionAreas:1427` writes every area,
//! while this method writes only signal-layer ones, and a board arrives desynchronised in the
//! first place — `BoardRules::new` starts at `ignoreConduction = true` while a conduction area
//! constructed as an obstacle starts at `isObstacle = true`.
//!
//! The consequence, on a board at its own defaults: **"stop treating pours as obstacles" does
//! nothing at all.** `change(false)` sees `ignoreConduction == true != false` and returns. The
//! next call flips which of the two arguments works, so the pair alternates rather than mirrors.
//!
//! Plan 9 Task 10 removes the guard. The store stays `setIgnoreConduction(!value)`: the field's
//! name is its specification, and `unfillConductionAreas` and the Java caller
//! (`GuiBoardRoutingSettings:33-37`, which calls `changeConductionIsObstacle(!value)`) both agree
//! that `ignoreConduction == !isObstacle`. See the `// fixed: T10 (#50)` note at the site for the
//! reasoning, including why the register sketch's "store `value`" is not adopted.
//!
//! # The fixture
//!
//! [`pour_board`] is the new one the task calls for: a **signal-layer** copper pour on net 1 and a
//! **foreign-net** trace of net 2 crossing it, on a two-layer board. Everything the tests assert
//! is derived from those two items and from the flag, so a change in either is visible.

mod board_builder;

use fr_board::prelude::*;
use fr_board::{Item, ShapeTraceEntries};
use fr_geometry::{
    Area, IntBox, Point, PolygonShape, Polyline, PolylineShapeRef, Shape, TileShape,
};

/// A signal-layer copper pour on net 1 and a foreign-net (net 2) trace crossing it.
///
/// Returns `(board, pour, foreign_trace)`. The pour is inserted with `is_obstacle = true`, which
/// is what `BasicBoard.makeConductive:1210` hard-codes and what a DSN `(plane …)` on a signal
/// layer becomes; the board's `rules.ignore_conduction` is at `BoardRules::new`'s `true`. Those
/// two together are the desynchronised state quirk #50's guard cannot recover from.
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

/// Quirk #50, fixed at Plan 9 Task 10.
///
/// The property in the name is the one a user cares about, and it is asserted in **both**
/// directions on one board, because a flag that can only be set one way is exactly the defect:
///
/// * `change_conduction_is_obstacle(false)` on a freshly built board makes the pour stop
///   obstructing. **Java returns immediately here** — `ignoreConduction` is `true` at
///   `BoardRules::new` and `true != false`, so the request is silently dropped and the pour goes
///   on obstructing;
/// * `change_conduction_is_obstacle(true)` makes it obstruct again;
/// * and the pair is idempotent: repeating either call changes nothing, where Java alternated.
#[test]
fn a_signal_layer_pour_obstructs_a_foreign_net() {
    let (mut board, pour, foreign) = pour_board();

    // The starting state, which is also the desynchronised state Java cannot recover from: the
    // board-level flag says "ignore", every per-item flag says "obstacle".
    assert!(board.rules.get_ignore_conduction());
    assert!(is_obstacle(&board, pour));
    assert!(the_pour_obstructs_the_foreign_net(&mut board, pour));

    // Ask it to stop. Java drops this request on the floor.
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

    // Idempotent, where Java alternated.
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

/// The flag is a *mirror*, not a latch: after the fix, the two board-level and per-item states
/// agree after every call, from either starting point.
///
/// Java's `ignoreConduction` and `isObstacle` alternated — the guard let a call through only when
/// they were in one particular relation, and the store then put them into the other — so the
/// board's answer to "do pours obstruct?" depended on how many times the method had been called
/// rather than on what it was last asked.
#[test]
fn the_conduction_flag_mirrors_the_per_item_state_from_either_start() {
    for start in [true, false] {
        let (mut board, pour, _) = pour_board();
        board.rules.set_ignore_conduction(start);
        // The sequence opens with `false`, deliberately: the fixture is born desynchronised
        // (`ignoreConduction = true` beside `isObstacle = true`), so that is the request Java's
        // guard drops on the floor, and a sequence that opened with `true` would pass on the jar
        // by luck.
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

/// `unfillConductionAreas` (BasicBoard.java:1426-1440) is the other writer of the same pair, and
/// after the fix the two agree on what the pair means: it leaves `ignoreConduction = true` beside
/// `isObstacle = false`, which is exactly `change_conduction_is_obstacle(false)`'s end state.
///
/// This is the evidence for the reading the fix adopts, and against the register sketch's "store
/// `value`": under that reading `unfillConductionAreas` would be writing the two halves in
/// opposite directions.
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
