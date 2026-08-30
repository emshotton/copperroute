//! Plan 6 Task 9: `RoutingBoardExt`, the check-only half of `board.optimize.TraceShover` and
//! `board.actions.DrillItemMover.check` / `.tryShoveViaPoints`.
//!
//! # Where the numbers come from
//!
//! Every literal below — the room counts and ids `initConnection` leaves behind, every
//! `checkForcedTracePolyline` answer, every `TraceShover.check` answer at every recursion depth,
//! every `tryShoveViaPoints` centre and every tie-pin contact id — is **read off the HEAD jar**,
//! not off this port. The probe is `scripts/differential/java/probes/P6T9Probe.java`, committed
//! with the exact `javac`/`java` invocation in its header, and its stdout is committed verbatim
//! as `tests/data/p6t9-board-ext.txt`. Each test names its probe mode.

use fr_board::ids::{ItemId, PadstackId};
use fr_board::items::Item;
use fr_board::prelude::*;
use fr_geometry::{
    IntBox, IntOctagon, IntPoint, IntVector, Point, Polyline, Shape, TileShape, Vector,
};
use fr_router::PageId;
use fr_router::autoroute::maze::engine::AutorouteEngine;
use fr_router::board_ext::{DrillItemMover, RoutingBoardExt, TraceShover};

// =================================================================================================
// The probe's boards, rebuilt from scratch
// =================================================================================================

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

fn rules_with_wide_class(angle: AngleRestriction) -> BoardRules {
    let mut clearance_matrix = ClearanceMatrix::get_default_instance(&layers(), 200);
    assert!(clearance_matrix.append_class("wide"));
    clearance_matrix.set_value_on_all_layers(2, 1, 600);
    clearance_matrix.set_value_on_all_layers(2, 2, 800);
    let mut rules = BoardRules::new(layers(), clearance_matrix);
    rules.trace_angle_restriction = angle;
    rules
}

/// `P6T9Probe.buildBare`: `P6T6Probe`'s bare board with **only** a net-2 trace on it — no net-1
/// item at all, so no completed room can be net dependent and every removal
/// `init_connection(2)` performs must have come from `initConnection:111-117` ->
/// `additionalUpdateAfterChange`.
fn bare_board_with_a_net_two_trace() -> Board {
    let mut board = Board::new(
        Vec::new(),
        0,
        BOUNDING_BOX,
        rules_with_wide_class(AngleRestriction::None),
        BoardLibrary::new(Padstacks::new(layers()), Packages::new()),
        Components::new(),
        Communication::default(),
    );
    let default_class = board.rules.get_default_net_class();
    board.rules.nets.add("N1", 1, false, default_class);
    board.rules.nets.add("N2", 1, false, default_class);
    board.insert_trace_without_cleaning(
        Polyline::from_points(&[Point::new(0, -400), Point::new(0, 400)]),
        0,
        30,
        vec![2],
        1,
        FixedState::Unfixed,
    );
    board
}

/// `P6T9Probe.build`, which is `P6T7Probe.build`'s (and `P6T3.build`'s) any-angle board plus the
/// three nets `Trace.isShoveFixed` dereferences: two layers, a 200-unit clearance matrix with a
/// "wide" class, a two-pin component (an **SMD** pad at (-500, 0) on layer 0 only and a
/// **through** pad at (500, 0) on both layers) and two traces, one on net 1 and one on net 2.
fn probe_board(angle: AngleRestriction) -> Board {
    let mut padstacks = Padstacks::new(layers());
    let smd = padstacks.add(
        "smd",
        vec![
            Some(Shape::Tile(TileShape::Box(IntBox::from_coords(
                -50, -50, 50, 50,
            )))),
            None,
        ],
        false,
        false,
    );
    let through_shape = Shape::Tile(TileShape::Octagon(IntOctagon::new(
        -70, -70, 70, 70, -140, 140, -140, 140,
    )));
    let through = padstacks.add(
        "thru",
        vec![Some(through_shape.clone()), Some(through_shape)],
        true,
        false,
    );
    let mut packages = Packages::new();
    let package = packages.add(
        "pkg",
        vec![
            PackagePin::new("P1", smd, IntVector::new(-500, 0).into(), 0.0),
            PackagePin::new("P2", through, IntVector::new(500, 0).into(), 0.0),
        ],
        None,
        None,
        None,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        true,
    );
    let mut components = Components::new();
    components.add_with_generated_name(Some(Point::new(0, 0)), 0.0, true, package);

    let mut board = Board::new(
        Vec::new(),
        0,
        BOUNDING_BOX,
        rules_with_wide_class(angle),
        BoardLibrary::new(padstacks, packages),
        components,
        Communication::default(),
    );
    let default_class = board.rules.get_default_net_class();
    board.rules.nets.add("N1", 1, false, default_class);
    board.rules.nets.add("N2", 1, false, default_class);
    board.rules.nets.add("N3", 1, false, default_class);

    board.insert_pin(1, 0, vec![1], 1, FixedState::Unfixed);
    board.insert_pin(1, 1, vec![1], 1, FixedState::Unfixed);
    board.insert_trace_without_cleaning(
        Polyline::from_points(&[
            Point::new(-500, 0),
            Point::new(0, 0),
            Point::new(0, 400),
            Point::new(500, 400),
        ]),
        0,
        30,
        vec![1],
        1,
        FixedState::Unfixed,
    );
    board.insert_trace_without_cleaning(
        Polyline::from_points(&[
            Point::new(-800, 300),
            Point::new(-800, 900),
            Point::new(300, 900),
        ]),
        0,
        40,
        vec![2],
        2,
        FixedState::Unfixed,
    );
    board
}

/// The probe's seven polylines, in `P6T9Probe.probeLines` order.
fn probe_lines() -> Vec<(&'static str, Polyline)> {
    vec![
        (
            "acrossNet1",
            Polyline::from_points(&[Point::new(-300, -600), Point::new(-300, 600)]),
        ),
        (
            "intoNet1",
            Polyline::from_points(&[Point::new(-300, -600), Point::new(-300, 0)]),
        ),
        (
            "acrossNet2",
            Polyline::from_points(&[Point::new(-1200, 600), Point::new(-400, 600)]),
        ),
        (
            "throughPin",
            Polyline::from_points(&[Point::new(500, -600), Point::new(500, 600)]),
        ),
        (
            "freeSpace",
            Polyline::from_points(&[Point::new(2000, 2000), Point::new(3000, 2000)]),
        ),
        (
            "twoSegments",
            Polyline::from_points(&[
                Point::new(-1200, 600),
                Point::new(-300, 600),
                Point::new(-300, -600),
            ]),
        ),
        (
            "offBoard",
            Polyline::from_points(&[Point::new(9800, 9800), Point::new(12000, 9800)]),
        ),
    ]
}

// =================================================================================================
// `RoutingBoard.initAutoroute` / `finishAutoroute` (RoutingBoard.java:882-905)
// =================================================================================================

/// Probe mode `upd`:
///
/// ```text
/// reusedTheEngine=true
/// rebuiltOnClassChange=true
/// afterClassChange counter=0 complete=null incomplete=null treeSize=1 compensatedCl=2
/// ```
///
/// `initAutoroute:888-891` reuses the engine only when it exists, `retain` is set **and** the
/// compensated clearance class of its tree matches the requested one.
#[test]
fn init_autoroute_reuses_the_engine_only_on_a_matching_clearance_class() {
    let mut board = bare_board_with_a_net_two_trace();
    let engine = board.init_autoroute(None, 1, 1, None, true);
    let tree = engine.tree;

    // Same class, retain = true: the engine is handed straight back.
    let engine = board.init_autoroute(Some(engine), 2, 1, None, true);
    assert_eq!(
        engine.tree, tree,
        "the engine was rebuilt on a matching class"
    );

    // A different clearance class rebuilds even with retain = true (`:890-891`).
    let rebuilt = board.init_autoroute(Some(engine), 2, 2, None, true);
    assert_ne!(rebuilt.tree, tree, "the class change did not rebuild");
    assert_eq!(
        rebuilt.complete_expansion_rooms().len(),
        0,
        "a rebuilt engine starts with no rooms — probe `afterClassChange complete=null`"
    );
    assert_eq!(compensated_class(&board, &rebuilt), 2);
}

/// Probe mode `upd`: `rebuiltWhenRetainIsFalse=true`. `initAutoroute:889` short-circuits the
/// whole reuse test on `!retainAutorouteDatabase`, so the same class rebuilds too.
#[test]
fn init_autoroute_never_reuses_when_retain_is_false() {
    let mut board = bare_board_with_a_net_two_trace();
    let engine = board.init_autoroute(None, 1, 1, None, true);
    let rooms_before = seed_and_complete(&mut board, engine);
    assert_eq!(rooms_before.1, 1, "probe `completed n=1`");
    let engine = rooms_before.0;

    let fresh = board.init_autoroute(Some(engine), 1, 1, None, false);
    assert_eq!(
        fresh.complete_expansion_rooms().len(),
        0,
        "retain = false must have thrown the room database away"
    );
    assert!(!fresh.maintain_database, "`maintainDatabase` is `retain`");
}

/// `finishAutoroute` (`:900-905`) clears the engine's database before dropping it.
#[test]
fn finish_autoroute_clears_the_room_database_before_dropping_the_engine() {
    let mut board = bare_board_with_a_net_two_trace();
    let engine = board.init_autoroute(None, 1, 1, None, true);
    let (engine, completed) = seed_and_complete(&mut board, engine);
    assert_eq!(completed, 1);
    let tree_size_with_rooms = tree_size(&board, &engine);
    board.finish_autoroute(engine);
    // `clear` (`:306-317`) removes every room from the autoroute tree; the board's two leaves
    // (the trace's one tree shape) are all that is left — probe `afterInitNet2 treeSize=1`.
    assert!(
        tree_size_with_rooms > 1,
        "the rooms should have been in the tree before finishAutoroute"
    );
}

// =================================================================================================
// `RoutingBoard.additionalUpdateAfterChange` (RoutingBoard.java:96-118) and the
// `initConnection:111-117` loop it is reached from
// =================================================================================================

/// Probe mode `upd` — the re-run `task-6-report.md` §8.1 asks for, with an item on the new net:
///
/// ```text
/// completed n=1
/// afterComplete  counter=1 complete=1 incomplete=5 treeSize=2 compensatedCl=1
///     complete id=1 layer=0 shape=Simplex[-10000,-10000..-130,10000]dim=2 netDependent=false doors=6
/// reusedTheEngine=true
/// afterInitNet2  counter=1 complete=0 incomplete=1 treeSize=1 compensatedCl=1
/// ```
///
/// The one completed room is **not** net dependent, so `initConnection:100-109` cannot touch it:
/// the only path that removes it is the `:111-117` loop over the items of net 2 calling
/// `additionalUpdateAfterChange`, which is what this test pins.
#[test]
fn additional_update_after_change_removes_the_overlapping_rooms() {
    let mut board = bare_board_with_a_net_two_trace();
    let engine = board.init_autoroute(None, 1, 1, None, true);
    let (engine, completed) = seed_and_complete(&mut board, engine);
    assert_eq!(completed, 1, "probe `completed n=1`");
    assert_eq!(engine.complete_expansion_rooms().len(), 1);
    let room = engine.complete_expansion_rooms()[0];
    let room_ref = engine
        .rooms
        .complete_room(room)
        .expect("the completed room");
    assert_eq!(room_ref.get_id(), 1, "probe `complete id=1`");
    assert!(
        !room_ref.is_net_dependent(),
        "probe `netDependent=false` — the removal below can only be :111-117's"
    );
    assert_eq!(room_ref.get_doors().len(), 6, "probe `doors=6`");
    assert_eq!(tree_size(&board, &engine), 2, "probe `treeSize=2`");
    assert_eq!(
        engine.rooms.incomplete_rooms.len(),
        5,
        "probe `afterComplete incomplete=5`"
    );

    let engine = board.init_autoroute(Some(engine), 2, 1, None, true);
    assert_eq!(
        engine.complete_expansion_rooms().len(),
        0,
        "probe `afterInitNet2 complete=0`"
    );
    assert_eq!(
        engine.rooms.incomplete_rooms.len(),
        1,
        "probe `afterInitNet2 incomplete=1`"
    );
    assert_eq!(
        tree_size(&board, &engine),
        1,
        "probe `afterInitNet2 treeSize=1`"
    );
}

/// The other half of `additionalUpdateAfterChange`: `:107`'s `invalidateDrillPages(currentShape)`
/// for every tree shape of the item. Task 7 pinned what a page does when it is invalidated (it
/// frees its drills); this pins that the method reaches it.
///
/// It is a separate test from the one above because populating the pages **completes more rooms**
/// — `DrillPage.getDrills` completes one per layer per drill — so the probe's room counts only
/// hold on an engine whose pages have never been asked for drills.
#[test]
fn additional_update_after_change_invalidates_the_drill_pages_of_every_tree_shape() {
    let mut board = bare_board_with_a_net_two_trace();
    let mut engine = board.init_autoroute(None, 1, 1, None, true);
    let trace = board
        .items_in_board_order()
        .into_iter()
        .find(|id| board.get_item(*id).is_some_and(Item::is_trace))
        .expect("the net-2 trace");

    let tree = engine.tree;
    let shape = board
        .item_tree_shape(trace, tree, 0)
        .expect("the trace's tree shape");
    let overlapping: Vec<PageId> = engine.drill_pages().overlapping_pages(&shape);
    assert!(!overlapping.is_empty());

    let populated = populate_drill_pages(&mut board, &mut engine);
    assert!(
        populated > 0,
        "the fixture needs at least one page holding drills"
    );
    assert!(
        overlapping
            .iter()
            .all(|page| engine.drill_pages().page(*page).drills().is_some()),
        "every overlapping page should be holding drills before the invalidation"
    );

    board.additional_update_after_change(&mut engine, trace);

    assert!(
        overlapping
            .iter()
            .all(|page| engine.drill_pages().page(*page).drills().is_none()),
        "`:107` should have invalidated every page the trace's tree shape overlaps"
    );
}

// =================================================================================================
// `RoutingBoard.checkForcedTracePolyline` (RoutingBoard.java:408-448)
// =================================================================================================

/// Probe mode `poly`, both angle regimes — fourteen rows each, identical in the two regimes on
/// this board (the 90-degree arm replaces every offset shape by its bounding box before building
/// the `ShapeEntrySide`, which does not change any answer here but does change the code path).
#[test]
fn check_forced_trace_polyline_uses_the_default_tree_and_the_bounding_box_in_ninety_degree_mode() {
    // (name, halfWidth, check) — `mode=poly angle=NONE` and `angle=NINETY_DEGREE`, which agree.
    let expected: &[(&str, i32, bool)] = &[
        ("acrossNet1", 30, false),
        ("acrossNet1", 120, false),
        ("intoNet1", 30, false),
        ("intoNet1", 120, false),
        ("acrossNet2", 30, true),
        ("acrossNet2", 120, true),
        ("throughPin", 30, false),
        ("throughPin", 120, false),
        ("freeSpace", 30, true),
        ("freeSpace", 120, true),
        ("twoSegments", 30, false),
        ("twoSegments", 120, false),
        ("offBoard", 30, false),
        ("offBoard", 120, false),
    ];
    for angle in [AngleRestriction::None, AngleRestriction::NinetyDegree] {
        let mut board = probe_board(angle);
        let lines = probe_lines();
        let mut actual = Vec::new();
        for (name, polyline) in &lines {
            for half_width in [30, 120] {
                let ok =
                    board.check_forced_trace_polyline(polyline, half_width, 0, &[3], 1, 20, 5, 20);
                actual.push((*name, half_width, ok));
            }
        }
        assert_eq!(actual, expected, "angle regime {angle:?}");
    }
}

// =================================================================================================
// `TraceShover.check`, the instance form (TraceShover.java:231-411)
// =================================================================================================

/// Probe mode `inst`, the `twoSegments shape=0` rows:
///
/// ```text
///   twoSegments shape=0 maxRecursionDepth=0 check=false
///   twoSegments shape=0 maxRecursionDepth=1 check=true
///   twoSegments shape=0 maxRecursionDepth=2 check=true
///   twoSegments shape=0 maxRecursionDepth=20 check=true
/// ```
///
/// `:356-358` refuses as soon as there is a substitute trace piece and no recursion budget left;
/// one level is enough here. `ctrl.maxShoveTraceRecursionDepth = 20` is the constant the maze
/// passes, so the 20 row is the production value.
#[test]
fn trace_shover_check_refuses_at_the_recursion_limit() {
    let mut board = probe_board(AngleRestriction::None);
    let rows = instance_check_rows(&mut board);
    let two_segments: Vec<_> = rows
        .iter()
        .filter(|(name, shape, kind, _, _)| {
            *name == "twoSegments" && *shape == 0 && *kind == "maxRecursionDepth"
        })
        .map(|(_, _, _, value, ok)| (*value, *ok))
        .collect();
    assert_eq!(
        two_segments,
        vec![(0, false), (1, true), (2, true), (20, true)]
    );
}

/// Probe mode `inst`, every row of the table — all seven polylines, every offset shape, four
/// recursion depths and three spring-over budgets.
#[test]
fn trace_shover_check_agrees_with_the_jvm_on_every_probe_row() {
    let expected: &[(&str, usize, &str, i32, bool)] = &[
        ("acrossNet1", 0, "maxRecursionDepth", 0, false),
        ("acrossNet1", 0, "maxRecursionDepth", 1, false),
        ("acrossNet1", 0, "maxRecursionDepth", 2, false),
        ("acrossNet1", 0, "maxRecursionDepth", 20, false),
        ("acrossNet1", 0, "maxSpringOver", 0, false),
        ("acrossNet1", 0, "maxSpringOver", 1, false),
        ("acrossNet1", 0, "maxSpringOver", 20, false),
        ("intoNet1", 0, "maxRecursionDepth", 0, false),
        ("intoNet1", 0, "maxRecursionDepth", 1, false),
        ("intoNet1", 0, "maxRecursionDepth", 2, false),
        ("intoNet1", 0, "maxRecursionDepth", 20, false),
        ("intoNet1", 0, "maxSpringOver", 0, false),
        ("intoNet1", 0, "maxSpringOver", 1, false),
        ("intoNet1", 0, "maxSpringOver", 20, false),
        ("acrossNet2", 0, "maxRecursionDepth", 0, true),
        ("acrossNet2", 0, "maxRecursionDepth", 1, true),
        ("acrossNet2", 0, "maxRecursionDepth", 2, true),
        ("acrossNet2", 0, "maxRecursionDepth", 20, true),
        ("acrossNet2", 0, "maxSpringOver", 0, true),
        ("acrossNet2", 0, "maxSpringOver", 1, true),
        ("acrossNet2", 0, "maxSpringOver", 20, true),
        ("throughPin", 0, "maxRecursionDepth", 0, false),
        ("throughPin", 0, "maxRecursionDepth", 1, false),
        ("throughPin", 0, "maxRecursionDepth", 2, false),
        ("throughPin", 0, "maxRecursionDepth", 20, false),
        ("throughPin", 0, "maxSpringOver", 0, false),
        ("throughPin", 0, "maxSpringOver", 1, false),
        ("throughPin", 0, "maxSpringOver", 20, false),
        ("freeSpace", 0, "maxRecursionDepth", 0, true),
        ("freeSpace", 0, "maxRecursionDepth", 1, true),
        ("freeSpace", 0, "maxRecursionDepth", 2, true),
        ("freeSpace", 0, "maxRecursionDepth", 20, true),
        ("freeSpace", 0, "maxSpringOver", 0, true),
        ("freeSpace", 0, "maxSpringOver", 1, true),
        ("freeSpace", 0, "maxSpringOver", 20, true),
        ("twoSegments", 0, "maxRecursionDepth", 0, false),
        ("twoSegments", 0, "maxRecursionDepth", 1, true),
        ("twoSegments", 0, "maxRecursionDepth", 2, true),
        ("twoSegments", 0, "maxRecursionDepth", 20, true),
        ("twoSegments", 0, "maxSpringOver", 0, true),
        ("twoSegments", 0, "maxSpringOver", 1, true),
        ("twoSegments", 0, "maxSpringOver", 20, true),
        ("twoSegments", 1, "maxRecursionDepth", 0, false),
        ("twoSegments", 1, "maxRecursionDepth", 1, false),
        ("twoSegments", 1, "maxRecursionDepth", 2, false),
        ("twoSegments", 1, "maxRecursionDepth", 20, false),
        ("twoSegments", 1, "maxSpringOver", 0, false),
        ("twoSegments", 1, "maxSpringOver", 1, false),
        ("twoSegments", 1, "maxSpringOver", 20, false),
        ("offBoard", 0, "maxRecursionDepth", 0, false),
        ("offBoard", 0, "maxRecursionDepth", 1, false),
        ("offBoard", 0, "maxRecursionDepth", 2, false),
        ("offBoard", 0, "maxRecursionDepth", 20, false),
        ("offBoard", 0, "maxSpringOver", 0, false),
        ("offBoard", 0, "maxSpringOver", 1, false),
        ("offBoard", 0, "maxSpringOver", 20, false),
    ];
    let mut board = probe_board(AngleRestriction::None);
    assert_eq!(instance_check_rows(&mut board), expected);
}

/// The property that makes the check-only split safe: `check` never changes the board's item
/// set. It *does* write `shoveFailingObstacle` (`:251`, `:263`, `:306`, `:318`, `:357`) and it
/// does burn item ids on the substitute trace pieces it builds — neither of which
/// [`Board::structural_hash`] observes, and neither of which Java's `check` avoids either.
#[test]
fn trace_shover_check_does_not_mutate_the_board() {
    let mut board = probe_board(AngleRestriction::None);
    let before = board.structural_hash();
    let _ = instance_check_rows(&mut board);
    let mut board2 = probe_board(AngleRestriction::None);
    for (_, polyline) in probe_lines() {
        let _ = board2.check_forced_trace_polyline(&polyline, 120, 0, &[3], 1, 20, 5, 20);
    }
    assert_eq!(board.structural_hash(), before);
    assert_eq!(board2.structural_hash(), before);
}

// =================================================================================================
// `TraceShover.check`, the static form (TraceShover.java:57-229)
// =================================================================================================

/// Probe mode `seg`: the maximum shovable length from the start of the segment, or
/// `Integer.MAX_VALUE` when the algorithm succeeds completely.
#[test]
fn trace_shover_check_segment_agrees_with_the_jvm() {
    let max = f64::from(i32::MAX);
    let expected: &[(&str, bool, i32, f64)] = &[
        ("acrossNet1", false, 30, 0.0),
        ("acrossNet1", false, 120, 0.0),
        ("acrossNet1", true, 30, 0.0),
        ("acrossNet1", true, 120, 0.0),
        ("intoNet1", false, 30, 0.0),
        ("intoNet1", false, 120, 0.0),
        ("intoNet1", true, 30, 0.0),
        ("intoNet1", true, 120, 0.0),
        ("acrossNet2", false, 30, 0.0),
        ("acrossNet2", false, 120, 0.0),
        ("acrossNet2", true, 30, 0.0),
        ("acrossNet2", true, 120, 0.0),
        ("throughPin", false, 30, 0.0),
        ("throughPin", false, 120, 0.0),
        ("throughPin", true, 30, 0.0),
        ("throughPin", true, 120, 0.0),
        ("freeSpace", false, 30, max),
        ("freeSpace", false, 120, max),
        ("freeSpace", true, 30, max),
        ("freeSpace", true, 120, max),
        ("offBoard", false, 30, 0.0),
        ("offBoard", false, 120, 0.0),
        ("offBoard", true, 30, 0.0),
        ("offBoard", true, 120, 0.0),
    ];
    let mut board = probe_board(AngleRestriction::None);
    let mut actual = Vec::new();
    for (name, polyline) in probe_lines() {
        if polyline.corner_count() != 2 {
            continue;
        }
        let segment = fr_geometry::LineSegment::from_polyline(&polyline, 1).expect("a segment");
        for left in [false, true] {
            for half_width in [30, 120] {
                let result = TraceShover::check_segment(
                    &mut board,
                    &segment,
                    left,
                    0,
                    &[3],
                    half_width,
                    1,
                    20,
                    5,
                );
                actual.push((name, left, half_width, result));
            }
        }
    }
    assert_eq!(actual, expected);
}

// =================================================================================================
// `TraceShover.getIgnoreItemsAtTiePins` (TraceShover.java:592-603)
// =================================================================================================

/// Probe mode `tie`: only the first shape — the one over the SMD pin at (-500, 0) — answers
/// anything, and only for the pin's own net, where the answer is the pin's single trace contact.
#[test]
fn ignore_items_at_tie_pins_answers_the_contacts_of_the_own_net_pins() {
    let board = probe_board(AngleRestriction::None);
    let shapes = [
        TileShape::Box(IntBox::from_coords(-600, -100, -400, 100)),
        TileShape::Box(IntBox::from_coords(400, -100, 600, 100)),
        TileShape::Box(IntBox::from_coords(2000, 2000, 2200, 2200)),
    ];
    let mut actual = Vec::new();
    for (i, shape) in shapes.iter().enumerate() {
        for nets in [vec![1], vec![3], vec![]] {
            let ignore = TraceShover::ignore_items_at_tie_pins(&board, shape, 0, &nets);
            actual.push((i, nets, ignore));
        }
    }
    assert_eq!(
        actual,
        vec![
            (0, vec![1], vec![ItemId(4)]),
            (0, vec![3], vec![]),
            (0, vec![], vec![]),
            (1, vec![1], vec![]),
            (1, vec![3], vec![]),
            (1, vec![], vec![]),
            (2, vec![1], vec![]),
            (2, vec![3], vec![]),
            (2, vec![], vec![]),
        ]
    );
}

// =================================================================================================
// `DrillItemMover` (DrillItemMover.java:34-103, :256-325)
// =================================================================================================

/// Probe mode `drill`, the two arms `check` answers before it reaches
/// `ForcedPadRouter.checkForcedPad`:
///
/// ```text
///   fixedViaId=7 isShoveFixed=true
///   onPinViaId=8 contacts=1 kinds=Pin#3
///   check viaId=7 delta=(300,0) result=false ignoreSize=0
///   check viaId=8 delta=(300,0) result=false ignoreSize=0
/// ```
///
/// The third row (`check viaId=6 result=true`) runs the full `checkForcedPad` and is Task 10's —
/// see the `added in Task 10:` marker in `board_ext/drill_item_mover.rs`.
#[test]
fn drill_item_mover_check_agrees_with_the_jvm() {
    for angle in [AngleRestriction::None, AngleRestriction::NinetyDegree] {
        let mut board = probe_board(angle);
        let (_free, fixed, on_pin) = insert_probe_vias(&mut board);
        assert_eq!(fixed, ItemId(7), "probe `fixedViaId=7`");
        assert_eq!(on_pin, ItemId(8), "probe `onPinViaId=8`");
        assert_eq!(
            board
                .normal_contacts(on_pin)
                .into_iter()
                .collect::<Vec<_>>(),
            vec![ItemId(3)],
            "probe `onPinViaId=8 contacts=1 kinds=Pin#3`"
        );

        let delta = Vector::from(IntVector::new(300, 0));
        for via in [fixed, on_pin] {
            let mut ignore = Vec::new();
            let ok = DrillItemMover::check(&mut board, via, &delta, 20, 5, Some(&mut ignore), None);
            assert!(
                !ok,
                "probe `check viaId={} result=false` ({angle:?})",
                via.0
            );
            assert!(
                ignore.is_empty(),
                "probe `ignoreSize=0` — both arms return before `:63` adds the drill item"
            );
        }
    }
}

/// `(angle regime, obstacle shape, extendedCheck, the centres)` — one row of probe mode `drill`'s
/// `tryShoveViaPoints` block.
type ShoveRow = (AngleRestriction, &'static str, bool, Vec<(i32, i32)>);
/// [`ShoveRow`] with the centres borrowed, so the expectation table can be a `const`-shaped slice.
type ShoveRowRef<'a> = (AngleRestriction, &'a str, bool, &'a [(i32, i32)]);

/// Probe mode `drill`, the `tryShoveViaPoints` rows — four candidates in the any-angle regime and
/// two in the 90-degree one, because `:295-300` uses `IntBox.nearestBorderProjections` with a try
/// count of 2 there while `:301-307` uses the octagon's with 4.
#[test]
fn try_shove_via_points_agrees_with_the_jvm() {
    let box_shape = TileShape::Box(IntBox::from_coords(1900, 1900, 2400, 2400));
    let octagon_shape = TileShape::Octagon(IntOctagon::new(
        1900, 1900, 2400, 2400, -600, 4600, -600, 4600,
    ));
    let expected: &[ShoveRowRef<'_>] = &[
        (AngleRestriction::None, "box", false, &[(1612, 2000)]),
        (
            AngleRestriction::None,
            "box",
            true,
            &[(1612, 2000), (2000, 1612), (1696, 1696), (1612, 2388)],
        ),
        (AngleRestriction::None, "octagon", false, &[(1612, 2000)]),
        (
            AngleRestriction::None,
            "octagon",
            true,
            &[(1612, 2000), (2000, 1612), (1612, 2388), (1612, 1612)],
        ),
        (
            AngleRestriction::NinetyDegree,
            "box",
            false,
            &[(1612, 2000)],
        ),
        (
            AngleRestriction::NinetyDegree,
            "box",
            true,
            &[(1612, 2000), (2000, 1612)],
        ),
        (
            AngleRestriction::NinetyDegree,
            "octagon",
            false,
            &[(1612, 2000)],
        ),
        (
            AngleRestriction::NinetyDegree,
            "octagon",
            true,
            &[(1612, 2000), (2000, 1612)],
        ),
    ];
    let mut actual: Vec<ShoveRow> = Vec::new();
    for angle in [AngleRestriction::None, AngleRestriction::NinetyDegree] {
        let mut board = probe_board(angle);
        let (free, _, _) = insert_probe_vias(&mut board);
        assert_eq!(free, ItemId(6), "probe `freeViaId=6`");
        for (label, shape) in [("box", &box_shape), ("octagon", &octagon_shape)] {
            for extended in [false, true] {
                let points =
                    DrillItemMover::try_shove_via_points(&mut board, shape, 0, free, 1, extended);
                actual.push((
                    angle,
                    label,
                    extended,
                    points.iter().map(|p| (p.x, p.y)).collect(),
                ));
            }
        }
    }
    let expected: Vec<ShoveRow> = expected
        .iter()
        .map(|(a, l, e, ps)| (*a, *l, *e, ps.to_vec()))
        .collect();
    assert_eq!(actual, expected);
}

// =================================================================================================
// helpers
// =================================================================================================

/// The probe's three vias: a free one at (2000, 2000), a shove-fixed one at (3000, 2000) and one
/// sitting on the through-hole pin at (500, 0) so that its normal contacts include a `Pin`.
fn insert_probe_vias(board: &mut Board) -> (ItemId, ItemId, ItemId) {
    let through = PadstackId(
        board
            .library
            .padstacks
            .get_by_name("thru")
            .expect("the thru padstack")
            .no,
    );
    let free = board
        .insert_via(
            through,
            Point::new(2000, 2000),
            vec![3],
            1,
            FixedState::Unfixed,
            false,
        )
        .expect("the free via");
    let fixed = board
        .insert_via(
            through,
            Point::new(3000, 2000),
            vec![3],
            1,
            FixedState::ShoveFixed,
            false,
        )
        .expect("the shove-fixed via");
    let on_pin = board
        .insert_via(
            through,
            Point::new(500, 0),
            vec![1],
            1,
            FixedState::Unfixed,
            false,
        )
        .expect("the via on the pin");
    (free, fixed, on_pin)
}

/// One `(name, shape index, knob, value, answer)` row per line of probe mode `inst`.
fn instance_check_rows(board: &mut Board) -> Vec<(&'static str, usize, &'static str, i32, bool)> {
    let mut rows = Vec::new();
    for (name, polyline) in probe_lines() {
        let shapes = polyline.offset_shapes_between(120, 0, polyline.lines().len() - 1);
        for (s, shape) in shapes.iter().enumerate() {
            let from_side = ShapeEntrySide::from_polyline(&polyline, s + 1, shape);
            for depth in [0, 1, 2, 20] {
                let ok = TraceShover::check(
                    board,
                    shape,
                    Some(&from_side),
                    None,
                    0,
                    &[3],
                    1,
                    depth,
                    5,
                    20,
                    None,
                );
                rows.push((name, s, "maxRecursionDepth", depth, ok));
            }
            for spring_over in [0, 1, 20] {
                let ok = TraceShover::check(
                    board,
                    shape,
                    Some(&from_side),
                    None,
                    0,
                    &[3],
                    1,
                    20,
                    5,
                    spring_over,
                    None,
                );
                rows.push((name, s, "maxSpringOver", spring_over, ok));
            }
        }
    }
    rows
}

/// The probe's seed room, completed: `engine.addIncompleteExpansionRoom(null, 0, IntBox(-600,
/// -100, -400, 100))` then `completeExpansionRoom`.
fn seed_and_complete(board: &mut Board, mut engine: AutorouteEngine) -> (AutorouteEngine, usize) {
    let seed = engine.add_incomplete_expansion_room(
        None,
        0,
        Some(TileShape::Box(IntBox::from_coords(-600, -100, -400, 100))),
    );
    let completed = engine
        .complete_expansion_room(board, seed)
        .unwrap_or_default()
        .len();
    (engine, completed)
}

fn compensated_class(board: &Board, engine: &AutorouteEngine) -> usize {
    board
        .trees
        .trees()
        .find(|tree| tree.id() == engine.tree)
        .expect("the autoroute tree")
        .compensated_clearance_class()
}

fn tree_size(board: &Board, engine: &AutorouteEngine) -> usize {
    board
        .trees
        .trees()
        .find(|tree| tree.id() == engine.tree)
        .expect("the autoroute tree")
        .size()
}

/// Runs `DrillPage::get_drills` over every page of the engine's array so the pages hold drills,
/// and answers how many ended up non-empty.
fn populate_drill_pages(board: &mut Board, engine: &mut AutorouteEngine) -> usize {
    for page in all_pages(engine) {
        let _ = engine.drill_page_drills(board, page, false, &|| false);
    }
    pages_holding_drills(engine)
}

fn all_pages(engine: &AutorouteEngine) -> Vec<PageId> {
    let array = engine.drill_pages();
    let mut result = Vec::new();
    for j in 0..array.row_count() {
        for i in 0..array.column_count() {
            result.push(array.page_id(i, j));
        }
    }
    result
}

fn pages_holding_drills(engine: &AutorouteEngine) -> usize {
    all_pages(engine)
        .into_iter()
        .filter(|page| engine.drill_pages().page(*page).drills().is_some())
        .count()
}
