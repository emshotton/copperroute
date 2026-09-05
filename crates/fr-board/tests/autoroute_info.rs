mod board_builder;

use board_builder::p2t11_board;
use fr_board::prelude::*;

fn populate(board: &mut Board, id: ItemId) {
    let info = board
        .get_item_mut(id)
        .expect("the fixture item")
        .get_autoroute_info();
    info.start_info = true;
    info.precalculated_connection = Some(ConnectionId(7));
    info.expansion_rooms = vec![Some(ObstacleRoomId(3)), None];
}

#[test]
fn default_is_javas_freshly_constructed_item_autoroute_info() {
    let info = AutorouteInfo::default();
    assert!(!info.start_info);
    assert_eq!(info.precalculated_connection, None);
    assert!(info.expansion_rooms.is_empty());
}

#[test]
fn get_autoroute_info_creates_the_default_body_on_demand() {
    let mut board = p2t11_board();
    let id = ItemId(4);
    assert_eq!(
        board.get_item(id).unwrap().get_autoroute_info_pur(),
        None,
        "Item.getAutorouteInfoPur (Item.java:1046-1049) does not create"
    );
    assert_eq!(
        board.get_item_mut(id).unwrap().get_autoroute_info(),
        &AutorouteInfo::default()
    );
    assert!(
        board
            .get_item(id)
            .unwrap()
            .get_autoroute_info_pur()
            .is_some()
    );
}

#[test]
fn item_tree_shape_with_an_out_of_range_index_drops_the_autoroute_info() {
    let mut board = p2t11_board();
    let tree = board.default_tree_id();
    let id = ItemId(4);
    populate(&mut board, id);

    let count = board.item_tree_shape_count(id, tree);
    assert!(count > 0, "the fixture trace has tree shapes");
    assert!(
        board
            .get_item(id)
            .unwrap()
            .get_autoroute_info_pur()
            .is_some(),
        "an in-range fill leaves the scratch alone"
    );
    assert!(board.item_tree_shape(id, tree, count - 1).is_some());
    assert!(
        board
            .get_item(id)
            .unwrap()
            .get_autoroute_info_pur()
            .is_some()
    );

    assert_eq!(board.item_tree_shape(id, tree, count + 5), None);
    assert_eq!(
        board.get_item(id).unwrap().get_autoroute_info_pur(),
        None,
        "the stale index dropped the autoroute scratch, exactly as Java does"
    );
}

#[test]
fn deep_copy_drops_a_populated_autoroute_info() {
    let mut board = p2t11_board();
    let id = ItemId(4);
    populate(&mut board, id);
    assert!(
        board
            .get_item(id)
            .unwrap()
            .get_autoroute_info_pur()
            .is_some()
    );

    let copy = board.deep_copy();
    assert_eq!(copy.get_item(id).unwrap().get_autoroute_info_pur(), None);
    assert!(
        board
            .get_item(id)
            .unwrap()
            .get_autoroute_info_pur()
            .is_some(),
        "the original keeps its scratch"
    );
}

#[test]
fn get_autoroute_info_pur_mut_writes_without_creating() {
    let mut board = p2t11_board();
    let untouched = ItemId(5);
    assert_eq!(
        board
            .get_item_mut(untouched)
            .unwrap()
            .get_autoroute_info_pur_mut(),
        None
    );
    assert_eq!(
        board.get_item(untouched).unwrap().get_autoroute_info_pur(),
        None,
        "the read created nothing"
    );

    let id = ItemId(4);
    populate(&mut board, id);
    let info = board
        .get_item_mut(id)
        .unwrap()
        .get_autoroute_info_pur_mut()
        .expect("populated");
    info.precalculated_connection = None;
    assert_eq!(
        board
            .get_item(id)
            .unwrap()
            .get_autoroute_info_pur()
            .unwrap()
            .precalculated_connection,
        None
    );
}

#[test]
fn clear_autoroute_info_drops_a_populated_body() {
    let mut board = p2t11_board();
    let id = ItemId(4);
    populate(&mut board, id);
    board.get_item_mut(id).unwrap().clear_autoroute_info();
    assert_eq!(board.get_item(id).unwrap().get_autoroute_info_pur(), None);
}
