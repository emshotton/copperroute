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
    assert_eq!(board.item_shape_layer(ItemId(u32::MAX), 0), None);
}
