//! `TreeObject::Room` in the shared search tree — the `fr-board` half of Plan 2's obligation,
//! discharged by Plan 6 Task 2.
//!
//! Java: `autoroute/expansion/CompleteFreeSpaceExpansionRoom.java:19-20` (the room `implements
//! SearchTreeObject`), `:56-59` (`removeFromTree`), `autoroute/maze/AutorouteEngine.java:534`
//! (`autorouteSearchTree.insert(completedRoom)`), and `board/model/items/Item.java:809-814`
//! (`shapeLayer`).
//!
//! The room objects themselves live in `fr-router`; what `fr-board` owns is the tree key, its
//! ordering and the two methods that put a room in and take it out.

mod board_builder;

use board_builder::p2t11_board;
use fr_board::ids::{RoomId, TreeObject};
use fr_board::prelude::*;
use fr_geometry::{IntBox, TileShape};

fn probe() -> TileShape {
    TileShape::Box(IntBox::from_coords(-10_000, -10_000, 10_000, 10_000))
}

#[test]
fn a_room_goes_into_the_default_tree_and_sorts_before_every_item() {
    let mut board = p2t11_board();
    let shape = probe();

    let tree = board.trees.get_default_tree_mut();
    let before = tree.tree().leaf_count();
    let low = tree
        .insert_room(RoomId(1), &shape)
        .expect("a bounded shape");
    let high = tree
        .insert_room(RoomId(2), &shape)
        .expect("a bounded shape");
    assert_ne!(low, high);
    assert_eq!(tree.tree().leaf_count(), before + 2);

    let bounds = tree.tree().bounding_shape(&shape).expect("bounded");
    let objects: Vec<TreeObject> = tree
        .tree()
        .overlaps(&bounds)
        .into_iter()
        .map(|entry| entry.object)
        .collect();

    // CompleteFreeSpaceExpansionRoom.compareTo:50 returns -1 against a non-room, and
    // Item.compareTo:100 returns 1 against a non-item: every room precedes every item.
    assert_eq!(
        objects[0..2],
        [TreeObject::Room(RoomId(2)), TreeObject::Room(RoomId(1))],
        "rooms first, descending by id"
    );
    assert!(
        objects[2..]
            .iter()
            .all(|o| matches!(o, TreeObject::Item(_))),
        "and nothing but items after them"
    );
    assert!(objects.len() > 2, "the fixture board has items");
}

#[test]
fn remove_room_takes_the_leaf_out_and_a_none_entry_is_a_no_op() {
    let mut board = p2t11_board();
    let shape = probe();
    let tree = board.trees.get_default_tree_mut();
    let before = tree.tree().leaf_count();

    let leaf = tree.insert_room(RoomId(1), &shape);
    assert!(leaf.is_some());
    assert_eq!(tree.tree().leaf_count(), before + 1);

    // MinAreaTree.java:121-123 — a null entry is skipped, which is what a room whose shape had
    // no bound in this tree's directions leaves behind.
    tree.remove_room(None);
    assert_eq!(tree.tree().leaf_count(), before + 1);

    tree.remove_room(leaf);
    assert_eq!(tree.tree().leaf_count(), before);

    let bounds = tree.tree().bounding_shape(&shape).expect("bounded");
    assert!(
        tree.tree()
            .overlaps(&bounds)
            .into_iter()
            .all(|entry| matches!(entry.object, TreeObject::Item(_))),
        "the room is gone"
    );
}

#[test]
fn item_shape_layer_answers_every_item_and_none_for_a_stranger() {
    // `Item.shapeLayer(int)` needs an `ItemCtx`, which only a `Board` can build; `fr-router`'s
    // `ObstacleExpansionRoom.getLayer` (ObstacleExpansionRoom.java:38-41) needs it from outside
    // the crate, and recomputes it on every call rather than caching it.
    let board = p2t11_board();
    let ids = board.items_in_board_order();
    assert!(!ids.is_empty());
    let layer_count = board.rules.layer_structure().count();
    for id in ids {
        let layer = board
            .item_shape_layer(id, 0)
            .expect("every item on the board answers a layer for shape 0");
        assert!(layer < layer_count, "item {id} claims layer {layer}");
    }
    // An id the board does not hold answers `None` rather than panicking; Java would have NPE'd
    // on the `Item` reference its caller already holds.
    assert_eq!(board.item_shape_layer(ItemId(u32::MAX), 0), None);
}
