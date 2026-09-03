//! Plan 6 Task 6: `AutorouteEngine`'s expansion-room lifecycle
//! (`autoroute/maze/AutorouteEngine.java:39-675`).
//!
//! # Where the numbers come from
//!
//! Every literal below — room ids, room shapes, door counts, the engine's room-instance counter,
//! the surviving incomplete-room count and the search tree's leaf count — is **read off the HEAD
//! jar**, not off this port. The probe is `scripts/differential/java/probes/P6T6Probe.java`,
//! which is committed with the exact `javac`/`java` invocation in its header; it reflects into
//! `AutorouteEngine`'s three private lists, which are the state these tests assert on.
//!
//! Each test names its probe mode and pastes the stdout it asserts against.

use std::collections::BTreeSet;

use fr_board::ids::{ConnectionId, ItemId, RoomId};
use fr_board::prelude::*;
use fr_geometry::{Area, IntBox, IntPoint, Point, Polyline, Shape, TileShape};
use fr_router::autoroute::expansion::RoomRef;
use fr_router::autoroute::item_info;
use fr_router::autoroute::maze::engine::AutorouteEngine;

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

/// `P6T6Probe.buildBare`: two layers, a 200-unit default clearance matrix, an any-angle board of
/// [`BOUNDING_BOX`] and nothing on it.
fn bare_board() -> Board {
    let layers = || LayerStructure::new(vec![Layer::new("front", true), Layer::new("back", true)]);
    let clearance_matrix = ClearanceMatrix::get_default_instance(&layers(), 200);
    let mut rules = BoardRules::new(layers(), clearance_matrix);
    rules.trace_angle_restriction = AngleRestriction::None;
    Board::new(
        Vec::new(),
        0,
        BOUNDING_BOX,
        rules,
        BoardLibrary::new(Padstacks::new(layers()), Packages::new()),
        Components::new(),
        Communication::default(),
    )
}

/// `board.insertObstacle(new IntBox(...), layer, 1, FixedState.UNFIXED)`.
fn insert_obstacle(board: &mut Board, llx: i32, lly: i32, urx: i32, ury: i32, layer: usize) {
    board.insert_obstacle(
        Area::Shape(Shape::Tile(TileShape::Box(IntBox::from_coords(
            llx, lly, urx, ury,
        )))),
        layer,
        1,
        FixedState::Unfixed,
    );
}

/// The bounding box of a complete room, as `(llx, lly, urx, ury)`.
fn room_bounds(engine: &AutorouteEngine, room: RoomId) -> (i32, i32, i32, i32) {
    let shape = engine
        .rooms
        .complete_room(room)
        .and_then(|r| r.get_shape())
        .expect("a complete room with a shape");
    let b = shape.bounding_box();
    (b.ll.x, b.ll.y, b.ur.x, b.ur.y)
}

/// `(java room id, bounding box, door count)` — one row of [`complete_rooms`], in the shape the
/// probe prints it.
type RoomRow = (i32, (i32, i32, i32, i32), usize);

/// `(java room id, bounding box, door count)` for every room in `completeExpansionRooms`
/// (AutorouteEngine.java:74), in list order.
fn complete_rooms(engine: &AutorouteEngine) -> Vec<RoomRow> {
    engine
        .complete_expansion_rooms()
        .iter()
        .map(|id| {
            let room = engine.rooms.complete_room(*id).expect("a live room");
            (
                room.get_id(),
                room_bounds(engine, *id),
                room.get_doors().len(),
            )
        })
        .collect()
}

/// The `CompleteFreeSpaceExpansionRoom`s the autoroute tree holds, as a set — the other half of
/// quirk #165's I1 invariant, which is set equality against `completeExpansionRooms`.
fn tree_rooms(board: &Board, engine: &AutorouteEngine) -> std::collections::BTreeSet<RoomId> {
    let tree = board
        .trees
        .trees()
        .find(|tree| tree.id() == engine.tree)
        .expect("the autoroute tree");
    let ctx = board.ctx();
    let probe = TileShape::Box(board.get_bounding_box());
    tree.overlapping_tree_entries_with_rooms(&probe, None, &[], &board.items, &engine.rooms, &ctx)
        .into_iter()
        .filter_map(|entry| match entry.object {
            fr_board::ids::TreeObject::Room(id) => Some(id),
            fr_board::ids::TreeObject::Item(_) => None,
        })
        .collect()
}

fn tree_size(board: &Board, engine: &AutorouteEngine) -> usize {
    board
        .trees
        .trees()
        .find(|tree| tree.id() == engine.tree)
        .expect("the autoroute tree")
        .size()
}

// =================================================================================================
// `completeExpansionRoom` (AutorouteEngine.java:418-522)
// =================================================================================================

/// Probe mode 0, verbatim:
///
/// ```text
/// mode=0 emptyBoard treeSize=0
/// before counter=0 complete=null incomplete=1 treeSize=0
///     incomplete layer=0 shape=null contained=IntBox[2000,2000..2100,2100]dim=2 doors=0
///   result n=0
/// after counter=0 complete=null incomplete=0 treeSize=0
/// ```
///
/// **Java wins over the task brief**, which names this test
/// `completing_a_seed_room_on_an_empty_board_yields_one_room_covering_the_board`. It does not:
/// `ShapeSearchTree.completeShape` returns immediately when the tree has no root
/// (`ShapeSearchTree.java:589-591`), so an empty board yields **no** rooms at all, the
/// room-instance counter never ticks, and `completeExpansionRooms` is still `null` afterwards.
/// The seed is removed all the same — `:469` runs before the result loop.
#[test]
fn completing_a_seed_room_on_an_empty_board_yields_no_rooms_at_all() {
    let mut board = bare_board();
    let mut engine = AutorouteEngine::new(&mut board, 1, false);
    assert_eq!(tree_size(&board, &engine), 0);
    engine.init_connection(&mut board, 1, None);

    let seed = engine.add_incomplete_expansion_room(
        None,
        0,
        Some(TileShape::Box(IntBox::from_coords(2000, 2000, 2100, 2100))),
    );
    assert_eq!(engine.rooms.incomplete_rooms.len(), 1);

    let result = engine
        .complete_expansion_room(&mut board, seed)
        .expect("no failure boundary is reached");
    assert!(result.is_empty(), "result n=0");
    assert_eq!(engine.complete_expansion_rooms().len(), 0, "complete=null");
    assert_eq!(engine.rooms.incomplete_rooms.len(), 0, "incomplete=0");
    assert_eq!(tree_size(&board, &engine), 0);
    assert_eq!(
        engine.generate_room_id_no(),
        1,
        "counter=0 before this tick"
    );
}

/// Probe mode 1, verbatim:
///
/// ```text
/// mode=1 oneObstacle treeSize=1
///   result n=6
///     room id=1 layer=0 shape=Simplex[-10000,-10000..10000,-100]dim=2
///     room id=2 layer=0 shape=Simplex[1041,-100..10000,-41]dim=2
///     room id=6 layer=0 shape=Simplex[-10000,1100..10000,10000]dim=2
///     room id=7 layer=0 shape=Simplex[-10000,-100..-41,1100]dim=2
///     room id=8 layer=0 shape=Simplex[-1241,-100..-100,1041]dim=2
///     room id=9 layer=0 shape=Simplex[-100,-100..-41,-41]dim=2
/// after counter=9 complete=6 incomplete=25 treeSize=7
///     complete id=1 ... doors=9
///     complete id=2 ... doors=3
///     complete id=6 ... doors=6
///     complete id=7 ... doors=9
///     complete id=8 ... doors=4
///     complete id=9 ... doors=2
/// ```
#[test]
fn an_obstacle_splits_the_seed_into_the_java_room_set() {
    let (board, mut engine, rooms) = one_obstacle_run();

    let expected: [RoomRow; 6] = [
        (1, (-10_000, -10_000, 10_000, -100), 9),
        (2, (1041, -100, 10_000, -41), 3),
        (6, (-10_000, 1100, 10_000, 10_000), 6),
        (7, (-10_000, -100, -41, 1100), 9),
        (8, (-1241, -100, -100, 1041), 4),
        (9, (-100, -100, -41, -41), 2),
    ];
    assert_eq!(rooms.len(), 6, "result n=6");
    assert_eq!(complete_rooms(&engine), expected.to_vec());
    // The returned collection is `completeExpansionRooms` in the same order.
    let returned: Vec<i32> = rooms
        .iter()
        .map(|r| engine.rooms.complete_room(*r).unwrap().get_id())
        .collect();
    assert_eq!(returned, vec![1, 2, 6, 7, 8, 9]);

    assert_eq!(
        engine.generate_room_id_no(),
        10,
        "counter=9 before this tick"
    );
    assert_eq!(engine.rooms.incomplete_rooms.len(), 25, "incomplete=25");
    assert_eq!(tree_size(&board, &engine), 7, "treeSize=7");
}

/// `:492-515`: **only the first dimension-2 candidate is added directly**; every later one is fed
/// back through `completeShape` against a tree that now holds the rooms already added.
///
/// Probe mode 4 lists `completeShape`'s raw output for the same seed — **eight** candidates, all
/// of dimension 2:
///
/// ```text
/// mode=4 rawCandidates n=8
///     [0] Simplex[-10000,-10000..10000,-100]
///     [1] Simplex[1041,-100..10000,8859]
///     [2] Simplex[1100,-41..10000,10000]
///     [3] Simplex[-7859,1041..1100,10000]
///     [4] Simplex[-10000,1100..1041,10000]
///     [5] Simplex[-10000,-100..-41,1100]
///     [6] Simplex[-1241,-100..-100,1041]
///     [7] Simplex[-100,-100..-41,-41]
/// ```
///
/// Mode 1 turns those eight into **six** rooms. Candidate `[0]` is the one `addCompleteRoom` takes
/// straight (`:487-491`) and it survives unchanged as room 1. Candidates `[1]`..`[4]` are
/// re-completed and collapse into just two rooms — `[1041,-100..10000,-41]` and
/// `[-10000,1100..10000,10000]`, neither of which is any candidate's shape — while the retries
/// burn room ids 3, 4 and 5. A port that added every candidate directly would answer eight rooms
/// with the eight candidate shapes and consecutive ids 1..8.
#[test]
fn only_the_first_two_dimensional_candidate_is_added_directly() {
    let (_board, engine, rooms) = one_obstacle_run();

    // The eight raw candidates `completeShape` produced, none of which is filtered by `:472`.
    const RAW: [(i32, i32, i32, i32); 8] = [
        (-10_000, -10_000, 10_000, -100),
        (1041, -100, 10_000, 8859),
        (1100, -41, 10_000, 10_000),
        (-7859, 1041, 1100, 10_000),
        (-10_000, 1100, 1041, 10_000),
        (-10_000, -100, -41, 1100),
        (-1241, -100, -100, 1041),
        (-100, -100, -41, -41),
    ];
    assert_eq!(rooms.len(), 6, "eight candidates, six rooms");

    let final_bounds: Vec<(i32, i32, i32, i32)> =
        rooms.iter().map(|r| room_bounds(&engine, *r)).collect();
    // Candidate [0] survives untouched — it is the one added directly.
    assert_eq!(final_bounds[0], RAW[0]);
    // Rooms 2 and 6 are the recalculation of candidates [1]..[4] and match none of them.
    assert!(!RAW.contains(&final_bounds[1]));
    assert!(!RAW.contains(&final_bounds[2]));
    // The room ids skip 3, 4 and 5: `SortedRoomNeighbours.calculate` takes a fresh id per retry.
    let ids: Vec<i32> = rooms
        .iter()
        .map(|r| engine.rooms.complete_room(*r).unwrap().get_id())
        .collect();
    assert_eq!(ids, vec![1, 2, 6, 7, 8, 9]);
}

/// Ruling 7's first recovery boundary: `AutorouteEngine.java:518-521`.
///
/// **Java wins over the task brief and over the controller's note**, both of which say the catch
/// "returns the rooms completed so far". It does not: `result` is declared *inside* the `try`
/// (`:422`) and the catch at `:520` returns `new ArrayList<>()` — a **fresh empty** collection.
/// The rooms completed before the throw stay in `completeExpansionRooms` and in the search tree
/// (those are side effects, not the return value), but the caller is handed nothing.
///
/// So the port's `Err` *is* Java's empty collection, and `unwrap_or_default()` reproduces Java
/// exactly. The injection is a stale [`fr_router::IncompleteRoomId`], which is where Java holds a
/// reference the port cannot: dereferencing it panics, and the boundary catches the panic.
#[test]
fn complete_expansion_room_answers_javas_empty_collection_on_an_injected_failure() {
    let (mut board, mut engine, rooms) = one_obstacle_run();
    let before = complete_rooms(&engine);

    let stale = engine.add_incomplete_expansion_room(
        None,
        0,
        Some(TileShape::Box(IntBox::from_coords(0, 0, 10, 10))),
    );
    engine.remove_incomplete_expansion_room(stale);

    let answer = engine.complete_expansion_room(&mut board, stale);
    assert!(answer.is_err(), "the boundary caught the panic");
    assert!(
        answer.unwrap_or_default().is_empty(),
        "AutorouteEngine.java:520 returns a fresh empty ArrayList"
    );
    // The rooms completed before the failure are untouched — Java's side effects survive.
    assert_eq!(complete_rooms(&engine), before);
    assert_eq!(rooms.len(), 6);
}

// =================================================================================================
// `completeNeighbourRooms` (AutorouteEngine.java:567-592)
// =================================================================================================

/// Probe mode 2, verbatim:
///
/// ```text
/// mode=2 neighbourRestart treeSize=2
///   result n=2
///     room id=2 layer=0 shape=Simplex[-2959,-2959..-41,-70]dim=2
///     room id=4 layer=0 shape=Simplex[-2959,-2900..-71,-41]dim=2
/// afterComplete   counter=4 complete=2 incomplete=4 treeSize=4
/// afterNeighbours counter=7 complete=4 incomplete=5 treeSize=6
/// ```
///
/// `:573-584` re-reads `room.getDoors()` after every completed neighbour, because completing one
/// mutates the door list. Both rooms' neighbours are walked here and two further rooms appear —
/// a port that snapshotted the door list once would stop after the first pass and answer
/// `complete=3`.
#[test]
fn completing_a_neighbour_restarts_the_iterator() {
    let mut board = bare_board();
    insert_obstacle(&mut board, 0, 0, 1000, 1000, 0);
    insert_obstacle(&mut board, -4000, -4000, -3000, -3000, 0);
    let mut engine = AutorouteEngine::new(&mut board, 1, false);
    assert_eq!(tree_size(&board, &engine), 2);
    engine.init_connection(&mut board, 1, None);

    let seed = engine.add_incomplete_expansion_room(
        None,
        0,
        Some(TileShape::Box(IntBox::from_coords(-100, -100, -50, -50))),
    );
    let rooms = engine.complete_expansion_room(&mut board, seed).unwrap();
    assert_eq!(rooms.len(), 2, "result n=2");
    assert_eq!(
        rooms
            .iter()
            .map(|r| engine.rooms.complete_room(*r).unwrap().get_id())
            .collect::<Vec<_>>(),
        vec![2, 4]
    );
    assert_eq!(room_bounds(&engine, rooms[0]), (-2959, -2959, -41, -70));
    assert_eq!(room_bounds(&engine, rooms[1]), (-2959, -2900, -71, -41));
    assert_eq!(engine.complete_expansion_rooms().len(), 2);
    assert_eq!(engine.rooms.incomplete_rooms.len(), 4);
    assert_eq!(tree_size(&board, &engine), 4);

    for room in &rooms {
        engine.complete_neighbour_rooms(&mut board, RoomRef::Complete(*room));
    }
    assert_eq!(
        engine.generate_room_id_no(),
        8,
        "counter=7 before this tick"
    );
    assert_eq!(engine.complete_expansion_rooms().len(), 4, "complete=4");
    assert_eq!(engine.rooms.incomplete_rooms.len(), 5, "incomplete=5");
    assert_eq!(tree_size(&board, &engine), 6, "treeSize=6");
}

// =================================================================================================
// `initConnection` (AutorouteEngine.java:96-124) and `removeCompleteExpansionRoom` (:377-412)
// =================================================================================================

/// Probe mode 3, verbatim:
///
/// ```text
/// mode=3 netDependent maintainDatabase=true
///   result n=2
///     room id=1 layer=0 shape=IntBox[-10000,-10000..0,0]dim=2
///     room id=5 layer=0 shape=Simplex[0,0..10000,10000]dim=2
/// afterComplete   counter=5 complete=2 incomplete=9 treeSize=4
///     complete id=1 netDependent=true doors=2 targetDoors=2
///     complete id=5 netDependent=true doors=4 targetDoors=2
/// afterInitNet2   counter=5 complete=0 incomplete=7 treeSize=2
/// ```
///
/// The two rooms overlap the net-1 trace, so both are net-dependent and `:99-110` drops both when
/// the net changes. `:111-117`'s `additionalUpdateAfterChange` loop is a no-op here — no item
/// carries net 2 — which is what makes this test insensitive to that method still being Task 9's.
///
/// The incomplete count falls from 9 to 7 rather than to 5: `removeCompleteExpansionRoom` drops
/// the incomplete neighbours it unlinks *and* creates a fresh incomplete room for each
/// 1-dimensional neighbour (`:390-401`).
#[test]
fn init_connection_on_a_new_net_drops_the_net_dependent_rooms() {
    let (mut board, mut engine) = net_dependent_run(true);
    assert_eq!(engine.complete_expansion_rooms().len(), 2);
    assert_eq!(engine.rooms.incomplete_rooms.len(), 9);
    assert_eq!(tree_size(&board, &engine), 4);
    assert!(
        engine.complete_expansion_rooms().iter().all(|r| engine
            .rooms
            .complete_room(*r)
            .unwrap()
            .is_net_dependent())
    );

    engine.init_connection(&mut board, 2, None);
    assert_eq!(engine.get_net_number(), 2);
    assert_eq!(engine.complete_expansion_rooms().len(), 0, "complete=0");
    assert_eq!(engine.rooms.incomplete_rooms.len(), 7, "incomplete=7");
    assert_eq!(tree_size(&board, &engine), 2, "treeSize=2");
    assert_eq!(
        engine.generate_room_id_no(),
        6,
        "counter=5 before this tick"
    );
}

/// Quirk #164, named — **fixed: T8**. Probe mode 5, verbatim:
///
/// ```text
/// removing id=1 doors=2
///     door dim=1 other=CompleteFreeSpaceExpansionRoom   interDim=1 touchingSides=1/3
///     door dim=1 other=CompleteFreeSpaceExpansionRoom   interDim=1 touchingSides=2/0
///   afterRemoving1 counter=5 complete=1 incomplete=11 treeSize=3
/// removing id=5 doors=4
///     door dim=1 other=IncompleteFreeSpaceExpansionRoom interDim=1 touchingSides=EMPTY
///     door dim=1 other=IncompleteFreeSpaceExpansionRoom interDim=1 touchingSides=EMPTY
///     door dim=1 other=IncompleteFreeSpaceExpansionRoom interDim=1 touchingSides=1/0
///     door dim=1 other=IncompleteFreeSpaceExpansionRoom interDim=1 touchingSides=0/0
///   afterRemoving5 counter=5 complete=0 incomplete=7 treeSize=2
/// ```
///
/// `removeCompleteExpansionRoom`'s parameter is declared `CompleteFreeSpaceExpansionRoom`, so
/// `currentDoor.otherRoom(room)` at `:383` bound `ExpansionDoor`'s **narrowing**
/// `otherRoom(CompleteExpansionRoom)` overload (ExpansionDoor.java:78-92) and `:385` skipped every
/// door whose far side is incomplete. `completeNeighbourRooms` casts its argument back to
/// `(ExpansionRoom)` at `:578` for exactly this reason and says so in a comment; there is no such
/// cast here, and the port now takes the wide overload as if there were.
///
/// **The two live counts do not move, and that is the finding.** Java's own `:403`
/// `removeAllDoors` removes every door of the room being removed, drops the incomplete room on
/// each door's far side, and `removeIncompleteExpansionRoom` recurses into `removeAllDoors` — so
/// the incomplete room `:396-400` regenerates *for an incomplete neighbour* hangs off a room that
/// `:403` is about to delete, and goes with it. The regeneration is only permanent for a
/// **complete** neighbour, which is what the first removal above shows (`incomplete` 9 -> 11).
/// So the visible effect of the skip was nil and its invisible effect was the crash below.
///
/// What moves is the arena, and that is what these two tests read: two rooms and two doors are
/// **constructed** for the two incomplete neighbours whose `touchingSides` is not empty, and then
/// destroyed. `Arena` never reuses an index, so `slot_count()` is the honest record of "this door
/// was visited".
#[test]
fn a_door_onto_an_incomplete_room_is_not_skipped() {
    let (mut board, mut engine) = net_dependent_run(true);
    let rooms: Vec<RoomId> = engine.complete_expansion_rooms().to_vec();

    // The first removal's two neighbours are complete rooms; it is the control, and the two rooms
    // it regenerates survive because nothing deletes a complete neighbour.
    assert!(engine.remove_complete_expansion_room(&mut board, rooms[0]));
    assert_eq!(engine.rooms.incomplete_rooms.len(), 11, "9 + 2 regenerated");

    // The second removal's four neighbours are all incomplete — the doors `:385` used to skip.
    let room_slots_before = engine.rooms.incomplete_rooms.slot_count();
    let door_slots_before = engine.rooms.doors.slot_count();
    assert!(engine.remove_complete_expansion_room(&mut board, rooms[1]));
    assert_eq!(
        engine.rooms.incomplete_rooms.slot_count() - room_slots_before,
        2,
        "the two incomplete neighbours with a non-empty touchingSides each get their \
         regenerated room built — before the fix the wide overload was never taken and this \
         difference is 0"
    );
    assert_eq!(
        engine.rooms.doors.slot_count() - door_slots_before,
        2,
        "and a door apiece"
    );

    // The live counts are Java's and do not move: `:403`'s cascade deletes the two regenerated
    // rooms along with the incomplete neighbours they hang off.
    assert_eq!(engine.complete_expansion_rooms().len(), 0);
    assert_eq!(engine.rooms.incomplete_rooms.len(), 7, "11 - 4, none added");
    assert_eq!(tree_size(&board, &engine), 2);
}

/// The other half of #164, and the reason it cannot be half-fixed. `:394` indexes
/// `touchingSides[1]` with nothing guaranteeing the array has two entries;
/// `TileShape.touchingSides` answers `new int[0]` whenever its search fails
/// (TileShape.java:588-591, logged as `touching_side : dir2 not found`), and a 1-dimensional
/// intersection is no guarantee that it will not. **What kept the index in range was the
/// narrowing overload**: two of the four doors above answer `touchingSides=EMPTY`, and they are
/// exactly the incomplete neighbours `:385` used to skip. Fixing only the overload turns a silent
/// skip into an `ArrayIndexOutOfBoundsException`, so both are fixed together.
///
/// The port's `touching_sides` answers `Option<[usize; 2]>`, so "length >= 2" is a type-level
/// guarantee and Java's length check is the `None` arm. A `len() == 1` array is not
/// representable, and Java's only short answer is `new int[0]`.
#[test]
fn touching_sides_is_length_checked() {
    let (mut board, mut engine) = net_dependent_run(true);
    let rooms: Vec<RoomId> = engine.complete_expansion_rooms().to_vec();
    assert!(engine.remove_complete_expansion_room(&mut board, rooms[0]));

    // Four doors onto incomplete rooms, all with a 1-dimensional intersection; two of them answer
    // an empty `touchingSides`. The pre-fix port could not reach them at all, and a port that
    // took the wide overload without this check panics here with
    // "AutorouteEngine.removeCompleteExpansionRoom: a 1-dimensional intersection with no touching
    // sides — Java throws an ArrayIndexOutOfBoundsException at AutorouteEngine.java:394".
    let room_ref = fr_router::autoroute::expansion::RoomRef::Complete(rooms[1]);
    let neighbour_shapes: Vec<_> = engine
        .rooms
        .room_doors(room_ref)
        .to_vec()
        .into_iter()
        .filter_map(|door| engine.rooms.door(door)?.other_room(room_ref))
        .filter_map(|other| engine.rooms.room_shape(other).cloned())
        .collect();
    let room_shape = engine
        .rooms
        .room_shape(room_ref)
        .cloned()
        .expect("the room has a shape");
    let empty = neighbour_shapes
        .iter()
        .filter(|shape| room_shape.touching_sides(shape).is_none())
        .count();
    assert_eq!(
        empty, 2,
        "the fixture must carry the short-array case, or this test asserts nothing"
    );

    assert!(engine.remove_complete_expansion_room(&mut board, rooms[1]));
    assert_eq!(engine.rooms.incomplete_rooms.len(), 7);
}

/// The same run with `maintainDatabase == false`: `:97` gates the whole invalidation, so the two
/// net-dependent rooms survive the net change untouched.
#[test]
fn init_connection_leaves_the_rooms_when_maintain_database_is_false() {
    let (mut board, mut engine) = net_dependent_run(false);
    let before = complete_rooms(&engine);
    let incomplete_before = engine.rooms.incomplete_rooms.len();
    let tree_before = tree_size(&board, &engine);

    engine.init_connection(&mut board, 2, None);
    assert_eq!(engine.get_net_number(), 2);
    assert_eq!(complete_rooms(&engine), before);
    assert_eq!(engine.rooms.incomplete_rooms.len(), incomplete_before);
    assert_eq!(tree_size(&board, &engine), tree_before);
}

// =================================================================================================
// `clear` (:307-318), `getRoomsWithTargetItems` (:620-635), `validate` (:637-652)
// =================================================================================================

/// `:307-317`: every complete room leaves the search tree first, then all three lists go and the
/// counter is reset — and `:316` clears the items' scratch, which is what stops an item keeping an
/// `ObstacleRoomId` into a restarted arena.
#[test]
fn clear_empties_the_room_database_the_tree_and_the_items_scratch() {
    let (mut board, mut engine) = net_dependent_run(true);
    assert_eq!(tree_size(&board, &engine), 4);
    // Nothing on this run creates the per-item scratch — `calculateTargetDoors` builds target
    // doors without touching `ItemAutorouteInfo` — so give the trace one the way
    // `MazeSearchEngine.java:1012` does, through the *creating* accessor.
    let item_with_scratch = board
        .items_in_board_order()
        .into_iter()
        .find(|id| matches!(board.get_item(*id), Some(Item::Trace(_))))
        .expect("the net-1 trace");
    item_info::set_start_info(&mut board, item_with_scratch, true);
    assert!(
        board
            .get_item(item_with_scratch)
            .and_then(|i| i.get_autoroute_info_pur())
            .is_some()
    );

    engine.clear(&mut board);

    assert_eq!(engine.complete_expansion_rooms().len(), 0);
    assert_eq!(engine.rooms.incomplete_rooms.len(), 0);
    assert_eq!(engine.rooms.doors.len(), 0);
    assert_eq!(engine.rooms.target_doors.len(), 0);
    assert_eq!(tree_size(&board, &engine), 2, "only the trace's two leaves");
    assert_eq!(engine.generate_room_id_no(), 1, "the counter restarted");
    assert!(
        board
            .get_item(item_with_scratch)
            .and_then(|i| i.get_autoroute_info_pur())
            .is_none()
    );
}

/// `:620-635`. Java's `TreeSet<CompleteFreeSpaceExpansionRoom>` sorts by
/// `CompleteFreeSpaceExpansionRoom.compareTo`, which is `other.id - this.id` — **descending**. The
/// port answers a `BTreeSet<RoomId>`, whose ascending order over arena indices is the same
/// relation reversed, so the Java iteration order is `.rev()`.
#[test]
fn rooms_with_target_items_iterates_descending() {
    let (board, engine) = net_dependent_run(true);

    // Both rooms carry two target doors to the trace (probe mode 3, `targetDoors=2`).
    let trace = board
        .items_in_board_order()
        .into_iter()
        .find(|id| matches!(board.get_item(*id), Some(Item::Trace(_))))
        .expect("the net-1 trace");
    let mut items = BTreeSet::new();
    items.insert(trace);

    let found = engine.rooms_with_target_items(&items);
    assert_eq!(found.len(), 2);

    let java_order: Vec<i32> = found
        .iter()
        .rev()
        .map(|r| engine.rooms.complete_room(*r).unwrap().get_id())
        .collect();
    assert_eq!(
        java_order,
        vec![5, 1],
        "descending by room id, as Java's TreeSet"
    );

    // An item with no target door answers the empty set.
    let mut absent = BTreeSet::new();
    absent.insert(ItemId(999));
    assert!(engine.rooms_with_target_items(&absent).is_empty());
}

/// `:637-648` plus `CompleteFreeSpaceExpansionRoom.validate` (`:165-194`): `completeShape` builds
/// rooms that overlap no trace obstacle of the routed net 2-dimensionally, so a freshly completed
/// database validates.
#[test]
fn a_freshly_completed_database_validates() {
    let (board, engine) = net_dependent_run(true);
    assert!(engine.validate(&board));

    // An empty database is valid by `:638-640`.
    let mut empty_board = bare_board();
    let empty = AutorouteEngine::new(&mut empty_board, 1, false);
    assert!(empty.validate(&empty_board));
}

// =================================================================================================
// The small accessors
// =================================================================================================

/// `generateRoomIdNo` (`:672-674`) is `++expansionRoomInstanceCount`, so the first id is 1; and
/// `isStopRequested` (`:294-304`) checks the time limit first and the stop flag second.
#[test]
fn the_room_id_counter_starts_at_one_and_the_stop_check_is_two_tests() {
    let mut board = bare_board();
    let mut engine = AutorouteEngine::new(&mut board, 1, false);
    assert_eq!(engine.generate_room_id_no(), 1);
    assert_eq!(engine.generate_room_id_no(), 2);

    // `:300-302`: a null `stoppableThread` is `false`; the port's stand-in is `|| false`.
    assert!(!engine.is_stop_requested(&|| false));
    assert!(engine.is_stop_requested(&|| true));

    // `:295-298`: an exceeded time limit short-circuits before the stop flag is read.
    engine.init_connection(&mut board, 1, Some(TimeLimit::new(-1)));
    assert!(engine.is_stop_requested(&|| false));
}

/// `getFirstIncompleteExpansionRoom` (`:356-366`) is `iterator().next()` over the list, i.e.
/// insertion order, and `removeIncompleteExpansionRoom` (`:368-372`) takes one out.
#[test]
fn the_incomplete_room_list_is_a_queue_in_insertion_order() {
    let mut board = bare_board();
    let mut engine = AutorouteEngine::new(&mut board, 1, false);
    assert_eq!(engine.get_first_incomplete_expansion_room(), None);

    let boxes = [(0, 0, 10, 10), (20, 20, 30, 30), (40, 40, 50, 50)];
    let ids: Vec<_> = boxes
        .iter()
        .map(|(a, b, c, d)| {
            engine.add_incomplete_expansion_room(
                None,
                0,
                Some(TileShape::Box(IntBox::from_coords(*a, *b, *c, *d))),
            )
        })
        .collect();

    for id in &ids {
        assert_eq!(engine.get_first_incomplete_expansion_room(), Some(*id));
        engine.remove_incomplete_expansion_room(*id);
    }
    assert_eq!(engine.get_first_incomplete_expansion_room(), None);
}

/// `resetAllDoors` (AutorouteEngine.java:650-668) clears the maze scratch of every door of every
/// room in `completeExpansionRooms`, and of every item that **already has** an
/// `ItemAutorouteInfo` — through `getAutorouteInfoPur()` (`:662`), the nullable accessor that
/// creates nothing. That is the plan-1 obligation this task discharges: an item with no scratch
/// must still have none afterwards.
#[test]
fn reset_all_doors_clears_the_scratch_it_finds_and_creates_none() {
    let (mut board, mut engine) = net_dependent_run(true);
    let trace = board
        .items_in_board_order()
        .into_iter()
        .find(|id| matches!(board.get_item(*id), Some(Item::Trace(_))))
        .expect("the net-1 trace");

    // No item has scratch yet — `calculateTargetDoors` never asks for any.
    assert!(board.items_in_board_order().into_iter().all(|id| {
        board
            .get_item(id)
            .and_then(|i| i.get_autoroute_info_pur())
            .is_none()
    }));
    engine.reset_all_doors(&mut board);
    assert!(
        board.items_in_board_order().into_iter().all(|id| board
            .get_item(id)
            .and_then(|i| i.get_autoroute_info_pur())
            .is_none()),
        "AutorouteEngine.java:662 uses getAutorouteInfoPur, which creates nothing"
    );

    // Give the trace a scratch with a precalculated connection, and it is cleared (`:665`).
    item_info::set_precalculated_connection(&mut board, trace, Some(ConnectionId(7)));
    assert_eq!(
        item_info::get_precalculated_connection(&mut board, trace),
        Some(ConnectionId(7))
    );
    engine.reset_all_doors(&mut board);
    assert_eq!(
        item_info::get_precalculated_connection(&mut board, trace),
        None
    );
}

// =================================================================================================
// Shared runs
/// Quirk #165's I2, **fixed: T8** — an abandoned room leaves no live doors.
///
/// `SortedRoomNeighbours.calculate` constructs a `CompleteFreeSpaceExpansionRoom` at `:191-192`
/// *before* it knows whether the room survives, and `AutorouteEngine.addCompleteRoom` returns
/// `null` at `:528-530` when the completed shape is no longer 2-dimensional. Java's `null` path
/// removes the room from nothing — it was never added to `completeExpansionRooms` — but
/// `calculateNewIncompleteRooms` has already built doors from it to real rooms, so it stays
/// **reachable** through them. Everything that walks the list then misses it: `clear`
/// (`:308-312`), `validate` (`:642`), `getRoomsWithTargetItems` (`:623`) and `initConnection`'s
/// net-dependent invalidation (`:102`) — and `completeExpansionRoom`'s own `:426-432` scan can
/// pick it as `ignoreObject`, handing `completeShape` a room that is not in the tree it is
/// querying.
///
/// The invariant asserted here is the one that closes that: **every complete room on the far side
/// of a live door is a room the engine knows about**, i.e. is in `completeExpansionRooms`. Before
/// the fix, this run leaves doors onto rooms that are in none of the engine's lists.
///
/// The fix is `detach_all_doors`, **not** `removeAllDoors`, and that distinction is measured:
/// see `ExpansionRoomStore::detach_all_doors`, and
/// [`an_abandoned_rooms_incomplete_neighbours_are_not_deleted_with_it`] below.
#[test]
fn an_abandoned_room_leaves_no_live_doors() {
    let (_board, engine, rooms) = one_obstacle_run();
    let known: std::collections::BTreeSet<RoomId> =
        engine.complete_expansion_rooms().iter().copied().collect();
    assert_eq!(known.len(), rooms.len());

    // Every door reachable from a room the engine knows about must lead to a room it also knows
    // about (or to an incomplete room, which is a live expansion frontier by construction).
    let mut dangling = Vec::new();
    for room in &known {
        let room_ref = RoomRef::Complete(*room);
        for door in engine.rooms.room_doors(room_ref).to_vec() {
            let Some(other) = engine.rooms.door(door).and_then(|d| d.other_room(room_ref)) else {
                continue;
            };
            if let RoomRef::Complete(other) = other
                && !known.contains(&other)
            {
                dangling.push((*room, other));
            }
        }
    }
    assert!(
        dangling.is_empty(),
        "a live room holds a door onto a complete room the engine has abandoned: {dangling:?}"
    );
}

/// Quirk #165's I1, **fixed: T8** — `completeExpansionRooms` **is** the set of complete rooms that
/// exist, in both directions.
///
/// The register's title is literally this: the list and the search tree disagreed. The assertion
/// is set equality — every room in the list is a room the tree holds, and every room the tree
/// holds is in the list — which is what `addCompleteRoom` committing to both at once (`:531-535`)
/// is supposed to guarantee and what an abandoned room broke.
///
/// The id half of the improvement column is here too: `SortedRoomNeighbours.calculate` draws its
/// room id **once** and reuses it across the `edgeRemoved` retry, so a room that is thrown away no
/// longer consumes one. On this run the ids used to read `[1, 2, 6, 7, 8, 9]` — three numbers
/// burnt by retries — and now read `[1, 2, 3, 4, 5, 6]`.
#[test]
fn the_complete_room_list_is_exactly_the_rooms_in_the_tree() {
    let (board, engine, rooms) = one_obstacle_run();
    let listed: std::collections::BTreeSet<RoomId> =
        engine.complete_expansion_rooms().iter().copied().collect();
    let in_tree: std::collections::BTreeSet<RoomId> = tree_rooms(&board, &engine);
    assert_eq!(listed, in_tree, "the list and the tree hold the same rooms");
    assert_eq!(listed.len(), rooms.len());

    let ids: Vec<i32> = rooms
        .iter()
        .map(|r| engine.rooms.complete_room(*r).unwrap().get_id())
        .collect();
    assert_eq!(
        ids,
        vec![1, 2, 3, 4, 5, 6],
        "a room thrown away by the edgeRemoved retry no longer burns an id (the jar's are \
         [1, 2, 6, 7, 8, 9])"
    );
}

/// Why `addCompleteRoom`'s `null` path uses `detach_all_doors` and not Java's `removeAllDoors`.
///
/// The register's improvement column asks for `removeAllDoors`, and it is the wrong tool: the
/// room being abandoned is one `calculateDoors` has already wired to **newly built incomplete
/// rooms**, which are the engine's expansion frontier, and `removeAllDoors` deletes an incomplete
/// room on the far side of every door it walks (`AutorouteEngine.java:610-612`, recursing through
/// `removeIncompleteExpansionRoom`). Measured: with `remove_all_doors` on that path, six of
/// `tests/locator.rs`'s cases stop finding a connection at all. Unlinking removes the room's
/// *reachability*, which is all the row asks for.
#[test]
fn an_abandoned_rooms_incomplete_neighbours_are_not_deleted_with_it() {
    let mut store = fr_router::autoroute::expansion::ExpansionRoomStore::new();
    let abandoned = RoomRef::Complete(store.new_complete_room(
        Some(TileShape::Box(IntBox::from_coords(0, 0, 100, 100))),
        0,
        1,
    ));
    let frontier = RoomRef::Incomplete(store.new_incomplete_room(
        Some(TileShape::Box(IntBox::from_coords(100, 0, 200, 100))),
        0,
        None,
    ));
    let door = store.new_door(abandoned, frontier, 1);
    store.add_door(abandoned, door);
    store.add_door(frontier, door);

    store.detach_all_doors(abandoned);
    assert!(store.room_doors(abandoned).is_empty(), "unlinked");
    assert!(store.room_doors(frontier).is_empty(), "on both sides");
    let RoomRef::Incomplete(id) = frontier else {
        unreachable!()
    };
    assert!(
        store.incomplete_room(id).is_some(),
        "the frontier room survives — `remove_all_doors` would have deleted it"
    );
}

/// Quirk #166, **fixed: T8** — the rooms a partially failed `completeExpansionRoom` committed are
/// returned, not replaced by an empty collection.
///
/// **Java wins over the task brief and over the controller's note**, both of which say the catch
/// "returns the rooms completed so far". It does not: `result` is declared *inside* the `try`
/// (`:422`) and the catch at `:520` returns `new ArrayList<>()` — a **fresh empty** collection,
/// about rooms that exist, are in the search tree and have doors on them. `result` is hoisted out
/// of the closure in the port and comes back from the catch, with the panic message beside it in
/// [`fr_router::RouterError::PanickedWithRooms`].
///
/// **What this test can and cannot reach.** The two arms are distinguished and both are asserted:
/// a run that commits rooms answers them, and a run that panics before committing anything
/// answers `Panicked` with nothing. A panic *after* a commit is not constructible through the
/// public API on any fixture here — every injection point available (a stale `IncompleteRoomId`,
/// a dangling tree leaf) is reached by the **first** `completeShape`, before the commit loop
/// starts — so the third row of the table is asserted at the boundary itself rather than through
/// a fixture. That limitation is named rather than hidden; what is asserted is that the success
/// arm's value **is** the committed set (which is only true because `result` is the caller's), and
/// that the failure arm distinguishes "committed nothing" from "committed something".
#[test]
fn a_committed_room_survives_the_catch() {
    let (mut board, mut engine, rooms) = one_obstacle_run();

    // The success arm: what the method answered is exactly what it committed, to the list and to
    // the tree. Before the fix this was also true — but only because nothing threw.
    let listed: std::collections::BTreeSet<RoomId> =
        engine.complete_expansion_rooms().iter().copied().collect();
    assert_eq!(
        rooms
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>(),
        listed,
        "the returned rooms are the committed rooms"
    );
    assert_eq!(listed, tree_rooms(&board, &engine));

    // The failure arm, with nothing committed: a stale `IncompleteRoomId` is where Java holds a
    // reference the port cannot, and dereferencing it panics inside the boundary — before the
    // commit loop is reached. `Panicked`, not `PanickedWithRooms`, and the rooms already in the
    // database are untouched.
    let before = complete_rooms(&engine);
    let stale = engine.add_incomplete_expansion_room(
        None,
        0,
        Some(TileShape::Box(IntBox::from_coords(0, 0, 10, 10))),
    );
    engine.remove_incomplete_expansion_room(stale);
    let answer = engine.complete_expansion_room(&mut board, stale);
    assert!(
        matches!(answer, Err(fr_router::RouterError::Panicked(_))),
        "nothing was committed, so the boundary has no rooms to carry: {answer:?}"
    );
    assert!(
        engine
            .complete_expansion_room_or_committed(&mut board, stale)
            .is_empty(),
        "and the caller's reading of it is empty"
    );
    assert_eq!(
        complete_rooms(&engine),
        before,
        "Java's side effects survive"
    );
    assert_eq!(rooms.len(), 6);
}

// =================================================================================================

/// Probe mode 1: the bare board with one obstacle, one seed completed.
fn one_obstacle_run() -> (Board, AutorouteEngine, Vec<RoomId>) {
    let mut board = bare_board();
    insert_obstacle(&mut board, 0, 0, 1000, 1000, 0);
    let mut engine = AutorouteEngine::new(&mut board, 1, false);
    assert_eq!(tree_size(&board, &engine), 1);
    engine.init_connection(&mut board, 1, None);
    let seed = engine.add_incomplete_expansion_room(
        None,
        0,
        Some(TileShape::Box(IntBox::from_coords(
            -3000, -3000, 3000, 3000,
        ))),
    );
    let rooms = engine
        .complete_expansion_room(&mut board, seed)
        .expect("no failure boundary is reached");
    (board, engine, rooms)
}

/// Probe mode 3: the bare board plus one net-1 trace, one seed completed on top of it.
fn net_dependent_run(maintain_database: bool) -> (Board, AutorouteEngine) {
    let mut board = bare_board();
    board.insert_trace_without_cleaning(
        Polyline::from_points(&[Point::new(-500, 0), Point::new(0, 0), Point::new(0, 400)]),
        0,
        30,
        vec![1],
        1,
        FixedState::Unfixed,
    );
    let mut engine = AutorouteEngine::new(&mut board, 1, maintain_database);
    engine.init_connection(&mut board, 1, None);
    let seed = engine.add_incomplete_expansion_room(
        None,
        0,
        Some(TileShape::Box(IntBox::from_coords(-600, -100, -400, 100))),
    );
    let rooms = engine.complete_expansion_room(&mut board, seed).unwrap();
    assert_eq!(rooms.len(), 2, "result n=2");
    assert_eq!(room_bounds(&engine, rooms[0]), (-10_000, -10_000, 0, 0));
    assert_eq!(room_bounds(&engine, rooms[1]), (0, 0, 10_000, 10_000));
    (board, engine)
}
