//! Plan 6 Task 2: the expansion rooms, the doors, `ExpandableObject`, `MazeSearchElement` — and
//! `fr-board`'s reserved `TreeObject::Room` becoming real.
//!
//! The five `getId()` implementations are **hashes, never identities** (quirk #8's family), and
//! four of the five overflow silently. Each is transcribed with its `wrapping_*` and pinned here.

use fr_board::datastructures::ShapeTree;
use fr_board::ids::{ItemId, RoomId, TreeObject};
use fr_board::prelude::*;
use fr_dsn::{BoardReadResult, DsnReadOptions};
use fr_geometry::{IntBox, ShapeBoundingDirections, TileShape};
use fr_router::autoroute::expansion::{
    ExpandableRef, ExpansionDoor, ExpansionRoomStore, ObstacleExpansionRoom, RoomRef,
};
use fr_router::autoroute::item_info;
use fr_router::autoroute::maze::{MazeAdjustment, MazeSearchElement};

// ---------------------------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------------------------

const SPLITTER: &str = "Issue143-rpi_splitter.dsn";

/// A real board, read the way `crates/fr-drc/tests` reads one.
fn fixture_board(name: &str) -> Board {
    let path = parity::fixture(name);
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("cannot read fixture {}: {e}", path.display()));
    match fr_dsn::read_board(&bytes[..], None, Some(name), &DsnReadOptions::default()) {
        BoardReadResult::Success { board, .. } | BoardReadResult::OutlineMissing { board, .. } => {
            *board.unwrap_or_else(|| panic!("{name} produced no board"))
        }
        other => panic!("{name} did not read: {other:?}"),
    }
}

fn boxed(llx: i32, lly: i32, urx: i32, ury: i32) -> TileShape {
    TileShape::Box(IntBox::from_coords(llx, lly, urx, ury))
}

// ---------------------------------------------------------------------------------------------
// The five getId()s
// ---------------------------------------------------------------------------------------------

#[test]
fn room_ids_are_the_engine_counter_and_items_are_not() {
    // AutorouteEngine.generateRoomIdNo (AutorouteEngine.java:672-674) is `++count`, so the first
    // id is 1, and two rooms created back to back carry consecutive ids.
    let mut store = ExpansionRoomStore::new();
    assert_eq!(store.next_room_id_no(), 1);
    assert_eq!(store.next_room_id_no(), 2);

    let mut store = ExpansionRoomStore::new();
    let first_id = store.next_room_id_no();
    let a = store.new_complete_room(Some(boxed(0, 0, 10, 10)), 0, first_id);
    let second_id = store.next_room_id_no();
    let b = store.new_complete_room(Some(boxed(20, 0, 30, 10)), 0, second_id);

    assert_eq!(store.complete_room(a).unwrap().get_id(), 1);
    assert_eq!(store.complete_room(b).unwrap().get_id(), 2);

    // The arena key is *not* the Java id: `RoomId` is the arena index and the Java id is the
    // engine counter. They are both minted in creation order, which is all the tree ordering
    // needs — but they are not the same number, and nothing may assume they are.
    assert_eq!((a, b), (RoomId(0), RoomId(1)));
}

#[test]
fn obstacle_room_id_aliases_above_1023_shapes() {
    // ObstacleExpansionRoom.java:49-51: `(item.getId() << 10) | indexInItem` — an **or**, not a
    // sum, so an index of 1024 or more spills into the item's bits (quirk #160).
    assert_eq!(
        ObstacleExpansionRoom::id(ItemId(1), 1024),
        ObstacleExpansionRoom::id(ItemId(1), 0),
        "index 1024 sets exactly the bit item 1 already owns, so it aliases index 0"
    );
    assert_eq!(
        ObstacleExpansionRoom::id(ItemId(1), 2048),
        ObstacleExpansionRoom::id(ItemId(3), 0),
        "index 2048 turns item 1 into item 3"
    );
    // The brief's literal (`id(1, 1024) == id(2, 0)`) is arithmetically false: `1<<10 | 1024`
    // is 1024 and `2<<10` is 2048. `|` never carries.
    assert_ne!(
        ObstacleExpansionRoom::id(ItemId(1), 1024),
        ObstacleExpansionRoom::id(ItemId(2), 0)
    );
    // The other half of the hazard: the shift overflows a Java `int` at item id 2^21.
    assert!(ObstacleExpansionRoom::id(ItemId(1 << 21), 0) < 0);
    assert_eq!(
        ObstacleExpansionRoom::id(ItemId(1 << 22), 0),
        ObstacleExpansionRoom::id(ItemId(0), 0)
    );
}

#[test]
fn door_id_is_symmetric_in_its_two_rooms() {
    // ExpansionDoor.java:185-190: `min(id1, id2) * 31 + max(id1, id2)`.
    assert_eq!(ExpansionDoor::id(7, 11), ExpansionDoor::id(11, 7));
    assert_eq!(ExpansionDoor::id(7, 11), 7 * 31 + 11);
    assert_eq!(ExpansionDoor::id(5, 5), 5 * 31 + 5);
    // `int` multiplication, so it wraps rather than panicking in release *and* debug.
    assert_eq!(
        ExpansionDoor::id(i32::MAX, i32::MAX),
        i32::MAX.wrapping_mul(31).wrapping_add(i32::MAX)
    );
}

#[test]
fn a_target_door_id_folds_in_the_item_and_the_room() {
    // TargetItemExpansionDoor.java:71-74: `31 * item.getId() + (room != null ? room.getId() : 0)`.
    assert_eq!(
        fr_router::autoroute::expansion::target_door_id(ItemId(3), 4),
        3 * 31 + 4
    );
    assert_eq!(
        fr_router::autoroute::expansion::target_door_id(ItemId(3), 0),
        3 * 31
    );
}

// ---------------------------------------------------------------------------------------------
// TreeObject::Room in the shared tree
// ---------------------------------------------------------------------------------------------

#[test]
fn rooms_sort_before_items_and_descending_among_themselves() {
    // CompleteFreeSpaceExpansionRoom.compareTo:46-53 returns -1 against a non-room and
    // `other.id - this.id` against a room; Item.compareTo:93-103 is the mirror image. So the
    // shared tree's result set is Room(hi), Room(lo), Item(hi), Item(lo).
    let mut tree: ShapeTree<TreeObject> = ShapeTree::new(ShapeBoundingDirections::Orthogonal);
    let probe = boxed(0, 0, 100, 100);
    for object in [
        TreeObject::Item(ItemId(1)),
        TreeObject::Room(RoomId(1)),
        TreeObject::Item(ItemId(2)),
        TreeObject::Room(RoomId(2)),
    ] {
        tree.insert_tiles(object, std::slice::from_ref(&probe));
    }
    let bounds = tree.bounding_shape(&probe).unwrap();
    let order: Vec<TreeObject> = tree
        .overlaps(&bounds)
        .into_iter()
        .map(|e| e.object)
        .collect();
    assert_eq!(
        order,
        vec![
            TreeObject::Room(RoomId(2)),
            TreeObject::Room(RoomId(1)),
            TreeObject::Item(ItemId(2)),
            TreeObject::Item(ItemId(1)),
        ]
    );
}

#[test]
fn a_room_and_an_item_with_the_same_numeric_id_do_not_collide() {
    // Room ids and item ids are minted by different counters and collide numerically all the
    // time; the order is total only because the type discriminator is the primary key.
    assert!(TreeObject::Room(RoomId(7)) < TreeObject::Item(ItemId(7)));

    let mut tree: ShapeTree<TreeObject> = ShapeTree::new(ShapeBoundingDirections::Orthogonal);
    let probe = boxed(0, 0, 10, 10);
    tree.insert_tiles(TreeObject::Item(ItemId(7)), std::slice::from_ref(&probe));
    tree.insert_tiles(TreeObject::Room(RoomId(7)), std::slice::from_ref(&probe));
    let bounds = tree.bounding_shape(&probe).unwrap();
    let order: Vec<TreeObject> = tree
        .overlaps(&bounds)
        .into_iter()
        .map(|e| e.object)
        .collect();
    assert_eq!(
        order,
        vec![TreeObject::Room(RoomId(7)), TreeObject::Item(ItemId(7))]
    );
}

#[test]
fn a_room_enters_the_boards_own_compensated_tree_before_its_items() {
    if !parity::require_java_dir() {
        return;
    }
    let mut board = fixture_board(SPLITTER);
    let bbox = board.bounding_box;
    let probe = TileShape::Box(bbox);

    let mut store = ExpansionRoomStore::new();
    let id_a = store.next_room_id_no();
    let room_a = store.new_complete_room(Some(probe.clone()), 0, id_a);
    let id_b = store.next_room_id_no();
    let room_b = store.new_complete_room(Some(probe.clone()), 0, id_b);

    let tree = board.trees.get_default_tree_mut();
    store.insert_complete_room(tree, room_a);
    store.insert_complete_room(tree, room_b);

    let bounds = tree.tree().bounding_shape(&probe).unwrap();
    let objects: Vec<TreeObject> = tree
        .tree()
        .overlaps(&bounds)
        .into_iter()
        .map(|e| e.object)
        .collect();
    let rooms: Vec<TreeObject> = objects
        .iter()
        .copied()
        .filter(|o| matches!(o, TreeObject::Room(_)))
        .collect();
    assert_eq!(
        rooms,
        vec![TreeObject::Room(room_b), TreeObject::Room(room_a)],
        "descending among themselves"
    );
    let first_item = objects
        .iter()
        .position(|o| matches!(o, TreeObject::Item(_)))
        .expect("the splitter board has items");
    assert_eq!(first_item, rooms.len(), "every room precedes every item");
}

#[test]
fn removing_a_room_twice_is_a_no_op_not_a_panic() {
    // Plan 2 ruling 8 / quirk #39: `ShapeTree::remove_leaf` panics on a second removal where
    // Java's `MinAreaTree.removeLeaf` silently discards the whole tree. `AutorouteEngine.clear`
    // (:307-317) can never reach that, because `removeCompleteExpansionRoom` takes the room out
    // of `completeExpansionRooms` in the same method (:406). The port proves it by construction:
    // the room leaves the arena and the tree in one operation.
    if !parity::require_java_dir() {
        return;
    }
    let mut board = fixture_board(SPLITTER);
    let shape = TileShape::Box(board.bounding_box);
    let mut store = ExpansionRoomStore::new();
    let id = store.next_room_id_no();
    let room = store.new_complete_room(Some(shape), 0, id);

    let tree = board.trees.get_default_tree_mut();
    let before = tree.tree().leaf_count();
    store.insert_complete_room(tree, room);
    assert_eq!(tree.tree().leaf_count(), before + 1);

    assert!(store.remove_complete_room(tree, room));
    assert_eq!(tree.tree().leaf_count(), before);
    assert!(
        !store.remove_complete_room(tree, room),
        "the second removal is a no-op returning false, not a panic"
    );
    assert_eq!(tree.tree().leaf_count(), before);
    assert!(store.complete_room(room).is_none());
}

// ---------------------------------------------------------------------------------------------
// ItemAutorouteInfo end to end (Task 1 left the closure path untested)
// ---------------------------------------------------------------------------------------------

/// The first item of the board that has at least one tree shape in the default tree.
fn first_item_with_shapes(board: &mut Board) -> (ItemId, usize) {
    let tree = board.default_tree_id();
    let ids: Vec<ItemId> = board.items_in_board_order();
    for id in ids {
        let count = board.item_tree_shape_count(id, tree);
        if count > 0 {
            return (id, count);
        }
    }
    panic!("no item with tree shapes on the fixture board");
}

#[test]
fn expansion_room_array_resizes_and_preserves() {
    if !parity::require_java_dir() {
        return;
    }
    let mut board = fixture_board(SPLITTER);
    let tree = board.default_tree_id();
    let (item, count) = first_item_with_shapes(&mut board);

    let mut store = ExpansionRoomStore::new();
    let mut created = 0usize;
    let make = |store: &mut ExpansionRoomStore, board: &mut Board, created: &mut usize| {
        item_info::get_expansion_room(board, item, 0, tree, |b, i, idx, t| {
            *created += 1;
            store.new_obstacle_room(b, i, idx, t)
        })
    };

    // The closure path Task 1 could not reach: an empty array is allocated to the tree-shape
    // count and slot 0 is filled by the closure (ItemAutorouteInfo.java:57, :77-80).
    let first = make(&mut store, &mut board, &mut created).expect("index 0 is in range");
    assert_eq!(created, 1);
    assert_eq!(
        board
            .get_item(item)
            .unwrap()
            .get_autoroute_info_pur()
            .unwrap()
            .expansion_rooms
            .len(),
        count
    );

    // A second call reuses it — the closure does not run again (:77-80).
    let second = make(&mut store, &mut board, &mut created).expect("index 0 is in range");
    assert_eq!(second, first);
    assert_eq!(created, 1);

    // Grow the array behind the accessor's back: the next call resizes back to the tree-shape
    // count and the overlapping prefix survives (:59-66, the HEAD-only arraycopy branch).
    board
        .get_item_mut(item)
        .unwrap()
        .get_autoroute_info()
        .expansion_rooms
        .push(None);
    let third = make(&mut store, &mut board, &mut created).expect("index 0 is in range");
    assert_eq!(third, first, "the prefix survived the resize");
    assert_eq!(created, 1);
    assert_eq!(
        board
            .get_item(item)
            .unwrap()
            .get_autoroute_info_pur()
            .unwrap()
            .expansion_rooms
            .len(),
        count
    );

    // Shrink it to nothing: the resize refills with `None` and the closure runs again.
    board
        .get_item_mut(item)
        .unwrap()
        .get_autoroute_info()
        .expansion_rooms
        .clear();
    let fourth = make(&mut store, &mut board, &mut created).expect("index 0 is in range");
    assert_eq!(created, 2);
    assert_ne!(
        fourth, first,
        "a fresh room, because the slot was truncated"
    );

    // An out-of-range index is Java's silent `FRLogger.warn` + `null` (:68-76).
    let out_of_range =
        item_info::get_expansion_room(&mut board, item, count, tree, |_, _, _, _| {
            panic!("the closure must not run for an out-of-range index")
        });
    assert_eq!(out_of_range, None);
}

#[test]
fn a_stale_index_drops_the_autoroute_info() {
    // Plan 6 ruling 10: `Item.getTreeShape`'s out-of-range branch calls `clearDerivedData()`
    // (Item.java:218-221), which nulls `autorouteInfo` (Item.java:1060-1064) — dropping
    // `startInfo`, the precalculated connection and the whole room array.
    if !parity::require_java_dir() {
        return;
    }
    let mut board = fixture_board(SPLITTER);
    let tree = board.default_tree_id();
    let (item, count) = first_item_with_shapes(&mut board);

    let mut store = ExpansionRoomStore::new();
    item_info::set_start_info(&mut board, item, true);
    item_info::get_expansion_room(&mut board, item, 0, tree, |b, i, idx, t| {
        store.new_obstacle_room(b, i, idx, t)
    })
    .expect("index 0 is in range");
    assert!(
        board
            .get_item(item)
            .unwrap()
            .get_autoroute_info_pur()
            .is_some()
    );

    assert_eq!(board.item_tree_shape(item, tree, count + 5), None);
    assert!(
        board
            .get_item(item)
            .unwrap()
            .get_autoroute_info_pur()
            .is_none(),
        "the stale index dropped the whole autoroute scratch"
    );
}

// ---------------------------------------------------------------------------------------------
// Doors, ExpandableObject and MazeSearchElement
// ---------------------------------------------------------------------------------------------

#[test]
fn a_door_knows_its_other_room_and_only_a_complete_one() {
    let mut store = ExpansionRoomStore::new();
    let id = store.next_room_id_no();
    let complete = store.new_complete_room(Some(boxed(0, 0, 10, 10)), 0, id);
    let incomplete = store.new_incomplete_room(Some(boxed(10, 0, 20, 10)), 0, None);
    let door = store.new_door(
        RoomRef::Complete(complete),
        RoomRef::Incomplete(incomplete),
        1,
    );

    let d = store.door(door).unwrap();
    // ExpansionDoor.otherRoom(ExpansionRoom) (:62-72) answers either side, and `None` for a
    // room that is neither.
    assert_eq!(
        d.other_room(RoomRef::Complete(complete)),
        Some(RoomRef::Incomplete(incomplete))
    );
    assert_eq!(
        d.other_room(RoomRef::Incomplete(incomplete)),
        Some(RoomRef::Complete(complete))
    );
    assert_eq!(d.other_room(RoomRef::Complete(RoomId(99))), None);
    // The `CompleteExpansionRoom` overload (:79-92) additionally drops an incomplete answer.
    assert_eq!(d.other_complete_room(RoomRef::Complete(complete)), None);
    assert_eq!(
        d.other_complete_room(RoomRef::Incomplete(incomplete)),
        Some(RoomRef::Complete(complete))
    );
}

#[test]
fn a_doors_sections_are_allocated_once_and_reset_in_place() {
    let mut door = ExpansionDoor::new(
        RoomRef::Complete(RoomId(0)),
        RoomRef::Complete(RoomId(1)),
        1,
    );
    // ExpansionDoor.java:177-181: `reset()` on a null sectionArr is a no-op.
    assert_eq!(door.maze_search_element_count(), None);
    door.reset();

    door.allocate_sections(3);
    assert_eq!(door.maze_search_element_count(), Some(3));
    door.get_maze_search_element_mut(1).unwrap().is_occupied = true;
    door.get_maze_search_element_mut(1).unwrap().adjustment = MazeAdjustment::Left;

    // :194-195 — an allocation of the same length keeps the array as it is.
    door.allocate_sections(3);
    assert!(door.get_maze_search_element(1).unwrap().is_occupied);

    // :197-200 — a different length reallocates and every element is fresh.
    door.allocate_sections(4);
    assert_eq!(door.maze_search_element_count(), Some(4));
    assert_eq!(
        door.get_maze_search_element(1),
        Some(&MazeSearchElement::default())
    );

    door.get_maze_search_element_mut(0).unwrap().room_ripped = true;
    door.reset();
    assert!(!door.get_maze_search_element(0).unwrap().room_ripped);
}

#[test]
fn a_maze_search_element_resets_to_its_default() {
    // MazeSearchElement.java:25-32 — HEAD has six fields, and `ripupCost` is one of them; there
    // is no `alreadyChecked`.
    let mut e = MazeSearchElement::default();
    assert!(!e.is_occupied);
    assert_eq!(e.backtrack_door, None);
    assert_eq!(e.section_no_of_backtrack_door, 0);
    assert!(!e.room_ripped);
    assert_eq!(e.adjustment, MazeAdjustment::None);
    assert_eq!(e.ripup_cost, 0);

    e.is_occupied = true;
    e.section_no_of_backtrack_door = 4;
    e.room_ripped = true;
    e.adjustment = MazeAdjustment::Right;
    e.ripup_cost = 17;
    e.reset();
    assert_eq!(e, MazeSearchElement::default());
}

#[test]
fn door_exists_and_remove_door_walk_the_rooms_door_list() {
    let mut store = ExpansionRoomStore::new();
    let a_id = store.next_room_id_no();
    let a = store.new_complete_room(Some(boxed(0, 0, 10, 10)), 0, a_id);
    let b_id = store.next_room_id_no();
    let b = store.new_complete_room(Some(boxed(10, 0, 20, 10)), 0, b_id);
    let (a, b) = (RoomRef::Complete(a), RoomRef::Complete(b));

    assert!(!store.door_exists(a, b));
    let door = store.new_door(a, b, 1);
    store.add_door(a, door);
    store.add_door(b, door);
    assert!(store.door_exists(a, b));
    assert!(store.door_exists(b, a));

    assert!(store.remove_door(a, ExpandableRef::Door(door)));
    assert!(
        !store.remove_door(a, ExpandableRef::Door(door)),
        "removing twice answers false"
    );
    assert!(!store.door_exists(a, b));
    assert!(store.door_exists(b, a), "the other side still holds it");

    store.clear_doors(b);
    assert!(!store.door_exists(b, a));
}

#[test]
fn an_obstacle_room_reads_its_shape_once_and_its_layer_every_time() {
    // ObstacleExpansionRoom.java:29 caches the tree shape in the constructor; `:38-41`
    // recomputes the layer from the item on every call.
    if !parity::require_java_dir() {
        return;
    }
    let mut board = fixture_board(SPLITTER);
    let tree = board.default_tree_id();
    let (item, _) = first_item_with_shapes(&mut board);

    let mut store = ExpansionRoomStore::new();
    let id = store.new_obstacle_room(&mut board, item, 0, tree);
    let room = store.obstacle_room(id).unwrap();
    assert_eq!(room.get_item(), item);
    assert_eq!(room.get_index_in_item(), 0);
    assert_eq!(
        room.get_shape().cloned(),
        board.item_tree_shape(item, tree, 0)
    );
    assert_eq!(
        store.room_layer(&board, RoomRef::Obstacle(id)),
        board.item_shape_layer(item, 0)
    );
    // getObject() is the *item*, not the room (ObstacleExpansionRoom.java:131-134).
    assert_eq!(
        store.get_object(RoomRef::Obstacle(id)),
        Some(TreeObject::Item(item))
    );
    // getId() is the aliasing hash, and the door id folds two of those together.
    assert_eq!(
        store.room_id_no(RoomRef::Obstacle(id)),
        Some(ObstacleExpansionRoom::id(item, 0))
    );
    // `allDoorsCalculated` is a plain latch (:141-148).
    assert!(!store.obstacle_room(id).unwrap().all_doors_calculated());
    store
        .obstacle_room_mut(id)
        .unwrap()
        .set_doors_calculated(true);
    assert!(store.obstacle_room(id).unwrap().all_doors_calculated());
}

#[test]
fn a_target_door_intersects_the_item_with_its_room_and_a_null_room_is_empty() {
    // TargetItemExpansionDoor.java:25-30.
    if !parity::require_java_dir() {
        return;
    }
    let mut board = fixture_board(SPLITTER);
    let tree = board.default_tree_id();
    let (item, _) = first_item_with_shapes(&mut board);
    let item_shape = board.item_tree_shape(item, tree, 0).unwrap();

    let mut store = ExpansionRoomStore::new();
    let room_id_no = store.next_room_id_no();
    let room = store.new_complete_room(Some(item_shape.clone()), 0, room_id_no);

    let door = store.new_target_door(&mut board, item, 0, Some(RoomRef::Complete(room)), tree);
    let d = store.target_door(door).unwrap();
    assert_eq!(d.get_dimension(), 2);
    assert_eq!(d.maze_search_element_count(), 1);
    assert_eq!(d.get_shape(), &item_shape.intersection(&item_shape));
    assert_eq!(d.other_room(RoomRef::Complete(room)), None);
    assert_eq!(
        store.target_door_id_no(door),
        Some(fr_router::autoroute::expansion::target_door_id(
            item, room_id_no
        ))
    );

    // A null room is `Simplex.EMPTY` and hashes with a room id of 0 (:25-27, :73).
    let orphan = store.new_target_door(&mut board, item, 0, None, tree);
    assert!(store.target_door(orphan).unwrap().get_shape().is_empty());
    assert_eq!(
        store.target_door_id_no(orphan),
        Some(fr_router::autoroute::expansion::target_door_id(item, 0))
    );

    // isDestinationDoor is `!isStartInfo`, read through the *creating* accessor.
    assert!(
        store
            .target_door(door)
            .unwrap()
            .is_destination_door(&mut board)
    );
    item_info::set_start_info(&mut board, item, true);
    assert!(
        !store
            .target_door(door)
            .unwrap()
            .is_destination_door(&mut board)
    );
}

#[test]
fn a_door_built_from_the_room_shapes_takes_the_intersections_dimension() {
    // ExpansionDoor.java:35-39 through the store, which resolves both shapes.
    let mut store = ExpansionRoomStore::new();
    let a_id = store.next_room_id_no();
    let a = RoomRef::Complete(store.new_complete_room(Some(boxed(0, 0, 10, 10)), 0, a_id));
    let b_id = store.next_room_id_no();
    let b = RoomRef::Complete(store.new_complete_room(Some(boxed(10, 0, 20, 10)), 0, b_id));

    let door = store.new_door_from_shapes(a, b).unwrap();
    assert_eq!(store.door(door).unwrap().get_dimension(), 1);
    assert_eq!(store.door_shape(door), Some(boxed(10, 0, 10, 10)));
    assert_eq!(store.door_id_no(door), Some(ExpansionDoor::id(a_id, b_id)));

    // A room with the null shape of ExpansionDrill.java:77 has no computable dimension, where
    // Java throws.
    let plane = RoomRef::Incomplete(store.new_incomplete_room(None, 0, None));
    assert_eq!(store.new_door_from_shapes(a, plane), None);
}
