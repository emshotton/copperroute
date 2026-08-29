//! `AutorouteInfo`'s real body (plan-6 ruling 15) and the two places the board drops it wholesale
//! (plan-6 ruling 10).
//!
//! Java: `autoroute/ItemAutorouteInfo.java:10-105`, `board/model/items/Item.java:212-226`
//! (`getTreeShape`'s `clearDerivedData()` retry) and `:1060-1064` (`clearDerivedData` nulling
//! `autorouteInfo`).

mod board_builder;

use board_builder::p2t11_board;
use fr_board::prelude::*;

/// Fills item `id`'s autoroute info with something recognisable.
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
    // `new ItemAutorouteInfo(item)` leaves every field at its Java default: `startInfo` false,
    // `precalculatedConnection` null and `expansionRoomArr` null (ItemAutorouteInfo.java:15-25).
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
    // Plan-6 ruling 10, the whole ruling in one test: `Item.getTreeShape` (Item.java:212-226)
    // calls `clearDerivedData()` on an out-of-range index, and `clearDerivedData` sets
    // `autorouteInfo = null` (Item.java:1060-1064) — dropping `startInfo`, the precalculated
    // connection and the whole `ObstacleExpansionRoom` array. `fr-router` must therefore never
    // reach for the `&self` twin, which cannot do this.
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
    // Plan 2's snapshot tests already pin this for the empty placeholder; with a body it still
    // holds (RoutingBoard.java:901-904 clears the engine, `deep_copy` clears the scratch).
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
fn clear_autoroute_info_drops_a_populated_body() {
    let mut board = p2t11_board();
    let id = ItemId(4);
    populate(&mut board, id);
    board.get_item_mut(id).unwrap().clear_autoroute_info();
    assert_eq!(board.get_item(id).unwrap().get_autoroute_info_pur(), None);
}
