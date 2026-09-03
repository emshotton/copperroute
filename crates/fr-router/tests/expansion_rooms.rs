//! Plan 6 Task 2: the expansion rooms, the doors, `ExpandableObject`, `MazeSearchElement` — and
//! `fr-board`'s reserved `TreeObject::Room` becoming real.
//!
//! The five `getId()` implementations are **hashes, never identities** (quirk #8's family), and
//! four of the five overflow silently. Each is transcribed with its `wrapping_*` and pinned here.

use fr_board::datastructures::ShapeTree;
use fr_board::ids::{ItemId, RoomId, TreeObject};
use fr_board::prelude::*;
use fr_dsn::{BoardReadResult, DsnReadOptions};
use fr_geometry::{FloatLine, IntBox, ShapeBoundingDirections, TileShape};
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

/// Quirk #156, **fixed: T8** — and the arithmetic that made it necessary, kept as
/// `ObstacleExpansionRoom::java_id` so the aliasing can still be pinned.
///
/// `ObstacleExpansionRoom.java:49-51` is `(item.getId() << 10) | indexInItem` — an **or**, not a
/// sum, so an index of 1024 or more spills into the item's bits, and the shift overflows a Java
/// `int` at item id 2^21. The id reaches `ExpansionDoor.getId`, which is a sort key of
/// `MazeListElement`, so an aliased id is a routing decision taken on a collision.
#[test]
fn the_java_obstacle_room_id_aliases_above_1023_shapes() {
    assert_eq!(
        ObstacleExpansionRoom::java_id(ItemId(1), 1024),
        ObstacleExpansionRoom::java_id(ItemId(1), 0),
        "index 1024 sets exactly the bit item 1 already owns, so it aliases index 0"
    );
    assert_eq!(
        ObstacleExpansionRoom::java_id(ItemId(1), 2048),
        ObstacleExpansionRoom::java_id(ItemId(3), 0),
        "index 2048 turns item 1 into item 3"
    );
    // The plan's literal (`id(1, 1024) == id(2, 0)`) is arithmetically false: `1<<10 | 1024`
    // is 1024 and `2<<10` is 2048. `|` never carries.
    assert_ne!(
        ObstacleExpansionRoom::java_id(ItemId(1), 1024),
        ObstacleExpansionRoom::java_id(ItemId(2), 0)
    );
    // `docs/plan-9-prep/fixtures/task-8/expected-outcomes.md`'s first witness, verbatim.
    assert_eq!(ObstacleExpansionRoom::java_id(ItemId(1), 1024), 1024);
    assert_eq!(ObstacleExpansionRoom::java_id(ItemId(0), 1024), 1024);
    // **Its second witness is arithmetically wrong and is corrected here.** The bank writes
    // `id(5, 1024) = 5120 | 1024 = 6144`, but 5120 is 4096 + 1024, so bit 10 is *already* set and
    // the `|` changes nothing: `id(5, 1024) == 5120 == id(5, 0)`. The witness the bank was
    // reaching for is one item lower — 4096 has no bit 10, so setting it carries the room into
    // item 5's number.
    assert_eq!(ObstacleExpansionRoom::java_id(ItemId(5), 1024), 5120);
    assert_eq!(
        ObstacleExpansionRoom::java_id(ItemId(4), 1024),
        ObstacleExpansionRoom::java_id(ItemId(5), 0),
        "item 4's shape 1024 and item 5's shape 0 share id 5120"
    );
    // The other half of the hazard: the shift overflows a Java `int` at item id 2^21.
    assert!(ObstacleExpansionRoom::java_id(ItemId(1 << 21), 0) < 0);
    assert_eq!(
        ObstacleExpansionRoom::java_id(ItemId(1 << 22), 0),
        ObstacleExpansionRoom::java_id(ItemId(0), 0),
        "id(item, index) == id(item + 2^22, index) for every item and index"
    );
}

/// Quirk **#156 + #167 + #158**, **fixed: T8** — the binding property test.
///
/// Three ids that were hashes over mutable state become one thing: a **per-engine counter**, as
/// `CompleteFreeSpaceExpansionRoom`'s already was. The counter is shared by all four kinds of
/// expandable object the engine owns, so the ids are injective *across* the kinds and not only
/// within each.
///
/// The domain includes the two witnesses a naive small-input test misses — `indexInItem >= 1024`
/// and `itemId >= 2^21` — because under Java's formula both are silently correct until they are
/// not.
#[test]
fn every_expandable_id_is_stable_and_injective() {
    use std::collections::BTreeSet;

    let mut board = fixture_board(SPLITTER);
    let mut engine = fr_router::autoroute::maze::AutorouteEngine::new(&mut board, 1, false);
    let mut ids: Vec<i32> = Vec::new();

    // The page grid's ids come first — they are drawn when the engine is built.
    let array = engine.drill_pages();
    let pages: Vec<_> = (0..array.row_count())
        .flat_map(|j| (0..array.column_count()).map(move |i| (i, j)))
        .map(|(i, j)| array.page_id(i, j))
        .collect();
    assert!(!pages.is_empty(), "the probe board has a page grid");
    for page in &pages {
        ids.push(engine.drill_pages().page(*page).get_id());
    }

    // #156: obstacle rooms, over a domain that includes both of Java's aliasing witnesses.
    for (item, index) in [
        (ItemId(1), 0usize),
        (ItemId(1), 1024),
        (ItemId(0), 1024),
        (ItemId(5), 1024),
        (ItemId(6), 0),
        (ItemId(1 << 21), 0),
        (ItemId(1 << 22), 0),
    ] {
        let room = engine
            .rooms
            .new_obstacle_room(&mut board, item, index, engine.tree);
        ids.push(engine.rooms.obstacle_room(room).unwrap().get_id());
    }
    // Under Java's formula this domain collapses: seven rooms, four distinct ids.
    let java_ids: BTreeSet<i32> = [
        (ItemId(1), 0usize),
        (ItemId(1), 1024),
        (ItemId(0), 1024),
        (ItemId(5), 1024),
        (ItemId(6), 0),
        (ItemId(1 << 21), 0),
        (ItemId(1 << 22), 0),
    ]
    .into_iter()
    .map(|(item, index)| ObstacleExpansionRoom::java_id(item, index))
    .collect();
    assert_eq!(java_ids.len(), 5, "Java gives these seven rooms five ids");

    // #158: incomplete rooms, including the **whole-plane** room, whose shape is `None` and whose
    // `getId` Java NPEs on.
    let whole_plane = engine
        .rooms
        .new_incomplete_room(None, 0, Some(boxed(0, 0, 10, 10)));
    ids.push(engine.rooms.incomplete_room(whole_plane).unwrap().get_id());
    let shaped =
        engine
            .rooms
            .new_incomplete_room(Some(boxed(0, 0, 10, 10)), 0, Some(boxed(0, 0, 10, 10)));
    ids.push(engine.rooms.incomplete_room(shaped).unwrap().get_id());

    // #165's own space: complete rooms draw from the same counter.
    let id_no = engine.rooms.next_room_id_no();
    let complete = engine
        .rooms
        .new_complete_room(Some(boxed(0, 0, 10, 10)), 0, id_no);
    ids.push(engine.rooms.complete_room(complete).unwrap().get_id());

    // **Injective.**
    let distinct: BTreeSet<i32> = ids.iter().copied().collect();
    assert_eq!(
        distinct.len(),
        ids.len(),
        "every expandable object the engine owns has its own id: {ids:?}"
    );

    // **Stable** — nothing about the object moves its id.
    //
    // #158: `set_shape` used to move an incomplete room's id, because Java hashes the shape.
    let before = engine.rooms.incomplete_room(shaped).unwrap().get_id();
    engine
        .rooms
        .incomplete_rooms
        .get_mut(shaped.0)
        .unwrap()
        .set_shape(Some(boxed(100, 100, 200, 200)));
    assert_eq!(
        engine.rooms.incomplete_room(shaped).unwrap().get_id(),
        before,
        "an object's id must not change while it is an element of an ordered collection"
    );

    // #167: `getDrills` writes `netNumber`, which Java's `DrillPage.getId` hashes.
    let page = pages[0];
    // `getDrills:65` writes `netNumber = engine.getNetNumber()`, so the engine has to be on a net
    // for the write to be a change — which is exactly the production sequence.
    engine.init_connection(&mut board, 1, None);
    let before = engine.drill_pages().page(page).get_id();
    let java_before = engine.drill_pages().page(page).java_id();
    let never = || false;
    let _ = engine.drill_page_drills(&mut board, page, false, &never);
    assert_eq!(
        engine.drill_pages().page(page).get_id(),
        before,
        "recomputing a page must not change its identity"
    );
    assert_ne!(
        engine.drill_pages().page(page).java_id(),
        java_before,
        "Java's hash does move — this is the defect, kept pinned"
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
    // getId() was Java's aliasing hash; fixed: T8 (#156) it is the engine's own counter, and
    // this room is the first object this store handed one to.
    assert_eq!(store.room_id_no(RoomRef::Obstacle(id)), Some(1));
    assert_ne!(
        store.room_id_no(RoomRef::Obstacle(id)),
        Some(ObstacleExpansionRoom::java_id(item, 0)),
        "and it is no longer the hash — the hash is what aliased"
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

// ---------------------------------------------------------------------------------------------
// ExpansionDoor.getSectionSegments (ExpansionDoor.java:104-143)
// ---------------------------------------------------------------------------------------------

/// The total length of a section list, for the "the sections tile the shrunk segment" assertion.
fn total_length(sections: &[FloatLine]) -> f64 {
    sections.iter().map(|s| s.b.distance(&s.a)).sum()
}

#[test]
fn a_dimension_one_door_divides_into_sections_and_allocates_them() {
    // ExpansionDoor.java:115-117 then :138-142. The door is the shared edge of two rooms that
    // touch along x = 1000, from y = 0 to y = 4000. With offset_param = 8 the offset is
    // 8 + TRACE_WIDTH_TOLERANCE = 10, so maxDoorSectionWidth is 100 and the segment length is
    // 4000: sectionCount = (int)(4000 / 100) + 1 = 41.
    let mut store = ExpansionRoomStore::new();
    let a_id = store.next_room_id_no();
    let a = store.new_complete_room(Some(boxed(0, 0, 1000, 4000)), 0, a_id);
    let b_id = store.next_room_id_no();
    let b = store.new_complete_room(Some(boxed(1000, 0, 3000, 4000)), 0, b_id);
    let door = store
        .new_door_from_shapes(RoomRef::Complete(a), RoomRef::Complete(b))
        .unwrap();
    assert_eq!(store.door(door).unwrap().get_dimension(), 1);
    assert_eq!(store.door(door).unwrap().maze_search_element_count(), None);

    let sections = store.door_section_segments(door, 8.0);
    assert_eq!(sections.len(), 41);
    assert!(sections.len() >= 2, "the brief's >= 2 sections");

    // :141 — `getSectionSegments` is `allocateSections`' only caller, so the array is now there
    // and exactly as long as the returned list.
    assert_eq!(
        store.door(door).unwrap().maze_search_element_count(),
        Some(41)
    );

    // The sections tile the *shrunk* segment (:117, :142): 4000 shortened by `offset` at each
    // end, and consecutive sections meet end to end.
    assert!((total_length(&sections) - (4000.0 - 2.0 * 10.0)).abs() < 1e-6);
    for pair in sections.windows(2) {
        assert_eq!(pair[0].b, pair[1].a);
    }
    assert_eq!(sections[0].a.x, 1000.0);
    assert!(
        (sections[0].a.y - 10.0).abs() < 1e-6,
        "shrunk by the offset"
    );
    assert!((sections[40].b.y - 3990.0).abs() < 1e-6);
}

#[test]
fn a_re_section_of_the_same_width_keeps_the_maze_state() {
    // :141 -> :194-195. Calling `getSectionSegments` twice with the same offset must not throw
    // the section array away, because the maze search writes into it between the two calls.
    let mut store = ExpansionRoomStore::new();
    let a_id = store.next_room_id_no();
    let a = store.new_complete_room(Some(boxed(0, 0, 1000, 4000)), 0, a_id);
    let b_id = store.next_room_id_no();
    let b = store.new_complete_room(Some(boxed(1000, 0, 3000, 4000)), 0, b_id);
    let door = store
        .new_door_from_shapes(RoomRef::Complete(a), RoomRef::Complete(b))
        .unwrap();

    store.door_section_segments(door, 8.0);
    store
        .door_mut(door)
        .unwrap()
        .get_maze_search_element_mut(3)
        .unwrap()
        .is_occupied = true;
    store.door_section_segments(door, 8.0);
    assert!(
        store
            .door(door)
            .unwrap()
            .get_maze_search_element(3)
            .unwrap()
            .is_occupied
    );
    // A different offset gives a different count, and :197-200 replaces the whole array.
    store.door_section_segments(door, 98.0);
    assert_eq!(
        store.door(door).unwrap().maze_search_element_count(),
        Some(5),
        "(int)(4000 / (10 * 100)) + 1"
    );
    assert!(
        !store
            .door(door)
            .unwrap()
            .get_maze_search_element(3)
            .unwrap()
            .is_occupied
    );
}

#[test]
fn a_two_dimensional_door_between_two_free_space_rooms_uses_the_restraint_line() {
    // ExpansionDoor.java:118-132 with `calcDoorLineSegment` (:145-172). Two overlapping
    // free-space rooms: the door shape is their 2-dimensional intersection, and the restraint
    // line runs between the two corners of it that lie inside neither room.
    let mut store = ExpansionRoomStore::new();
    let a_id = store.next_room_id_no();
    let a = store.new_complete_room(Some(boxed(0, 0, 2000, 2000)), 0, a_id);
    let b_id = store.next_room_id_no();
    let b = store.new_complete_room(Some(boxed(1000, 1000, 3000, 3000)), 0, b_id);
    let door = store
        .new_door_from_shapes(RoomRef::Complete(a), RoomRef::Complete(b))
        .unwrap();
    assert_eq!(store.door(door).unwrap().get_dimension(), 2);

    let sections = store.door_section_segments(door, 8.0);
    assert!(
        !sections.is_empty(),
        "the door is far larger than 2 * offset"
    );
    assert_eq!(
        store.door(door).unwrap().maze_search_element_count(),
        Some(sections.len())
    );
    // The restraint line is the diagonal of the 1000x1000 overlap: (2000,1000)-(1000,2000).
    let full = (2000.0f64 - 1000.0).hypot(1000.0 - 2000.0);
    assert!((total_length(&sections) - (full - 2.0 * 10.0)).abs() < 1e-6);

    // :128-131 — a door shorter than 2 * offset is dropped, and no sections are allocated.
    let mut store = ExpansionRoomStore::new();
    let c_id = store.next_room_id_no();
    let c = store.new_complete_room(Some(boxed(0, 0, 2000, 2000)), 0, c_id);
    let d_id = store.next_room_id_no();
    let d = store.new_complete_room(Some(boxed(1999, 1999, 3000, 3000)), 0, d_id);
    let tiny = store
        .new_door_from_shapes(RoomRef::Complete(c), RoomRef::Complete(d))
        .unwrap();
    assert!(store.door_section_segments(tiny, 8.0).is_empty());
    assert_eq!(
        store.door(tiny).unwrap().maze_search_element_count(),
        None,
        ":130 returns before :141, so the array is never allocated"
    );
}

#[test]
fn a_two_dimensional_door_touching_an_obstacle_room_falls_to_the_gravity_branch() {
    // :118-120 tests `instanceof CompleteFreeSpaceExpansionRoom` on **both** rooms, so an
    // `ObstacleExpansionRoom` — the other `CompleteExpansionRoom` — takes the :133-137 branch:
    // a degenerate segment at the door's centre of gravity, i.e. exactly one zero-length
    // section.
    if !parity::require_java_dir() {
        return;
    }
    let mut board = fixture_board(SPLITTER);
    let tree = board.default_tree_id();
    let (item, _) = first_item_with_shapes(&mut board);
    let item_shape = board.item_tree_shape(item, tree, 0).unwrap();

    let mut store = ExpansionRoomStore::new();
    let obstacle = RoomRef::Obstacle(store.new_obstacle_room(&mut board, item, 0, tree));
    let free_id = store.next_room_id_no();
    let free = RoomRef::Complete(store.new_complete_room(Some(item_shape.clone()), 0, free_id));

    let door = store.new_door_from_shapes(free, obstacle).unwrap();
    assert_eq!(store.door(door).unwrap().get_dimension(), 2);
    let sections = store.door_section_segments(door, 8.0);
    assert_eq!(sections.len(), 1);
    assert_eq!(sections[0].a, sections[0].b, "a zero-length section");
    assert_eq!(sections[0].a, item_shape.centre_of_gravity());
    assert_eq!(
        store.door(door).unwrap().maze_search_element_count(),
        Some(1)
    );
}

#[test]
fn an_empty_door_shape_yields_no_sections_at_all() {
    // :108-112 — the guard before every branch.
    let mut store = ExpansionRoomStore::new();
    let a_id = store.next_room_id_no();
    let a = store.new_complete_room(Some(boxed(0, 0, 1000, 1000)), 0, a_id);
    let b_id = store.next_room_id_no();
    let b = store.new_complete_room(Some(boxed(5000, 5000, 6000, 6000)), 0, b_id);
    let door = store.new_door(RoomRef::Complete(a), RoomRef::Complete(b), 1);
    assert!(store.door_section_segments(door, 8.0).is_empty());
    assert_eq!(store.door(door).unwrap().maze_search_element_count(), None);
}

// ---------------------------------------------------------------------------------------------
// AutorouteEngine.clear (AutorouteEngine.java:306-317)
// ---------------------------------------------------------------------------------------------

#[test]
fn clear_takes_the_rooms_out_of_the_boards_tree_before_draining_the_arenas() {
    // AutorouteEngine.java:308-312 runs *before* :313-315. Without it the shared tree would keep
    // `TreeObject::Room` leaves naming arena slots that no longer exist.
    if !parity::require_java_dir() {
        return;
    }
    let mut board = fixture_board(SPLITTER);
    let shape = TileShape::Box(board.bounding_box);
    let mut store = ExpansionRoomStore::new();
    let a_id = store.next_room_id_no();
    let a = store.new_complete_room(Some(shape.clone()), 0, a_id);
    let b_id = store.next_room_id_no();
    let b = store.new_complete_room(Some(shape.clone()), 0, b_id);

    let tree = board.trees.get_default_tree_mut();
    let before = tree.tree().leaf_count();
    store.insert_complete_room(tree, a);
    store.insert_complete_room(tree, b);
    assert_eq!(tree.tree().leaf_count(), before + 2);

    store.clear(tree);

    assert_eq!(
        tree.tree().leaf_count(),
        before,
        "both room leaves are gone"
    );
    let bounds = tree.tree().bounding_shape(&shape).unwrap();
    assert!(
        tree.tree()
            .overlaps(&bounds)
            .into_iter()
            .all(|entry| matches!(entry.object, TreeObject::Item(_))),
        "no TreeObject::Room survives the clear"
    );
    assert!(store.complete_rooms.is_empty());
}
