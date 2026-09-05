use std::collections::BTreeSet;

use fr_board::ids::{ConnectionId, ItemId, RoomId};
use fr_board::prelude::*;
use fr_geometry::{Area, IntBox, IntPoint, Point, Polyline, Shape, TileShape};
use fr_router::autoroute::expansion::RoomRef;
use fr_router::autoroute::item_info;
use fr_router::autoroute::maze::engine::AutorouteEngine;

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

fn room_bounds(engine: &AutorouteEngine, room: RoomId) -> (i32, i32, i32, i32) {
    let shape = engine
        .rooms
        .complete_room(room)
        .and_then(|r| r.get_shape())
        .expect("a complete room with a shape");
    let b = shape.bounding_box();
    (b.ll.x, b.ll.y, b.ur.x, b.ur.y)
}

type RoomRow = (i32, (i32, i32, i32, i32), usize);

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
        6,
        "counter=5 before this tick"
    );
}

#[test]
fn an_obstacle_splits_the_seed_into_the_java_room_set() {
    let (board, mut engine, rooms) = one_obstacle_run();

    let expected: [RoomRow; 6] = [
        (6, (-10_000, -10_000, 10_000, -100), 9),
        (12, (1041, -100, 10_000, -41), 3),
        (23, (-10_000, 1100, 10_000, 10_000), 6),
        (29, (-10_000, -100, -41, 1100), 9),
        (36, (-1241, -100, -100, 1041), 4),
        (38, (-100, -100, -41, -41), 2),
    ];
    assert_eq!(rooms.len(), 6, "result n=6");
    assert_eq!(complete_rooms(&engine), expected.to_vec());
    let returned: Vec<i32> = rooms
        .iter()
        .map(|r| engine.rooms.complete_room(*r).unwrap().get_id())
        .collect();
    assert_eq!(returned, vec![6, 12, 23, 29, 36, 38]);

    assert_eq!(
        engine.generate_room_id_no(),
        39,
        "counter=38 before this tick"
    );
    assert_eq!(engine.rooms.incomplete_rooms.len(), 25, "incomplete=25");
    assert_eq!(tree_size(&board, &engine), 7, "treeSize=7");
}

#[test]
fn only_the_first_two_dimensional_candidate_is_added_directly() {
    let (_board, engine, rooms) = one_obstacle_run();

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
    assert_eq!(final_bounds[0], RAW[0]);
    assert!(!RAW.contains(&final_bounds[1]));
    assert!(!RAW.contains(&final_bounds[2]));
    let ids: Vec<i32> = rooms
        .iter()
        .map(|r| engine.rooms.complete_room(*r).unwrap().get_id())
        .collect();
    assert_eq!(ids, vec![6, 12, 23, 29, 36, 38]);
    assert!(
        ids.windows(2).any(|w| w[1] > w[0] + 1),
        "the point of this assert is that the ids SKIP"
    );
}

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
    assert_eq!(complete_rooms(&engine), before);
    assert_eq!(rooms.len(), 6);
}

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
        vec![6, 10]
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
        20,
        "counter=19 before this tick"
    );
    assert_eq!(engine.complete_expansion_rooms().len(), 4, "complete=4");
    assert_eq!(engine.rooms.incomplete_rooms.len(), 5, "incomplete=5");
    assert_eq!(tree_size(&board, &engine), 6, "treeSize=6");
}

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
    assert_eq!(engine.rooms.incomplete_rooms.len(), 5, "incomplete=5");
    assert_eq!(tree_size(&board, &engine), 2, "treeSize=2");
    assert_eq!(
        engine.generate_room_id_no(),
        21,
        "counter=20 before this tick"
    );
}

#[test]
fn a_door_onto_an_incomplete_room_is_not_skipped() {
    let (mut board, mut engine) = net_dependent_run(true);
    let rooms: Vec<RoomId> = engine.complete_expansion_rooms().to_vec();
    assert_eq!(engine.rooms.incomplete_rooms.len(), 9);

    assert!(
        engine
            .rooms
            .room_doors(RoomRef::Complete(rooms[0]))
            .is_empty(),
        "the first room's door list is empty on this fixture since the ids moved"
    );
    assert!(engine.remove_complete_expansion_room(&mut board, rooms[0]));
    assert_eq!(
        engine.rooms.incomplete_rooms.len(),
        9,
        "no doors, so nothing regenerated and nothing cascaded"
    );

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

    assert_eq!(engine.complete_expansion_rooms().len(), 0);
    assert_eq!(engine.rooms.incomplete_rooms.len(), 5, "9 - 4, none added");
    assert_eq!(tree_size(&board, &engine), 2);
}

#[test]
fn touching_sides_is_length_checked() {
    let (mut board, mut engine) = net_dependent_run(true);
    let rooms: Vec<RoomId> = engine.complete_expansion_rooms().to_vec();
    assert!(engine.remove_complete_expansion_room(&mut board, rooms[0]));

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
    assert_eq!(engine.rooms.incomplete_rooms.len(), 5);
}

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

#[test]
fn clear_empties_the_room_database_the_tree_and_the_items_scratch() {
    let (mut board, mut engine) = net_dependent_run(true);
    assert_eq!(tree_size(&board, &engine), 4);
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

#[test]
fn rooms_with_target_items_iterates_descending() {
    let (board, engine) = net_dependent_run(true);

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
        vec![14, 6],
        "descending by room id, as Java's TreeSet"
    );
    assert!(
        java_order[0] > java_order[1],
        "the point of this assert is the DESCENDING order, not the ids"
    );

    let mut absent = BTreeSet::new();
    absent.insert(ItemId(999));
    assert!(engine.rooms_with_target_items(&absent).is_empty());
}

#[test]
fn a_freshly_completed_database_validates() {
    let (board, engine) = net_dependent_run(true);
    assert!(engine.validate(&board));

    let mut empty_board = bare_board();
    let empty = AutorouteEngine::new(&mut empty_board, 1, false);
    assert!(empty.validate(&empty_board));
}

#[test]
fn the_room_id_counter_is_consecutive_and_the_stop_check_is_two_tests() {
    let mut board = bare_board();
    let mut engine = AutorouteEngine::new(&mut board, 1, false);
    let first = engine.generate_room_id_no();
    assert_eq!(first, 5);
    assert_eq!(engine.generate_room_id_no(), first + 1);

    assert!(!engine.is_stop_requested(&|| false));
    assert!(engine.is_stop_requested(&|| true));

    engine.init_connection(&mut board, 1, Some(TimeLimit::new(-1)));
    assert!(engine.is_stop_requested(&|| false));
}

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

#[test]
fn reset_all_doors_clears_the_scratch_it_finds_and_creates_none() {
    let (mut board, mut engine) = net_dependent_run(true);
    let trace = board
        .items_in_board_order()
        .into_iter()
        .find(|id| matches!(board.get_item(*id), Some(Item::Trace(_))))
        .expect("the net-1 trace");

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

#[test]
fn an_abandoned_room_leaves_no_live_doors() {
    let (_board, engine, rooms) = one_obstacle_run();
    let known: std::collections::BTreeSet<RoomId> =
        engine.complete_expansion_rooms().iter().copied().collect();
    assert_eq!(known.len(), rooms.len());

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
    assert_eq!(ids, vec![6, 12, 23, 29, 36, 38]);
    assert!(
        ids.windows(2).all(|w| w[0] < w[1]),
        "strictly increasing in creation order, which is the property RoomId has to agree with"
    );
}

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

#[test]
fn a_committed_room_survives_the_catch() {
    let (mut board, mut engine, rooms) = one_obstacle_run();

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
