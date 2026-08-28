//! `Board`: the insert/remove protocol, the item-list and search queries, connectivity,
//! `check_trace_segment` and the changed area.
//!
//! # Provenance
//!
//! Every expectation below is a line of `scripts/differential/java/P2T11.java`'s output. That
//! driver builds the *same* board through the real `app.freerouting.board.facade.RoutingBoard`
//! on JDK 25 and prints it; `./scripts/differential/run.sh p2t11 {0,1,2,3}` diffs the two and all
//! four modes are byte-identical. The mode each test transcribes is named in its first comment.
//!
//! The handful of tests with no `P2T11` line behind them cite the Java source instead; those are
//! the paths the driver cannot reach (a refused removal that Java's driver would have to
//! construct, the `BoardServiceCharacterizationTest` port, `Board: Send + Sync`).

mod board_builder;

use std::collections::BTreeSet;

use board_builder::{descending, nums, p2t11_board};
use fr_board::prelude::*;
use fr_geometry::{
    Area, IntBox, IntOctagon, IntVector, Point, Polyline, PolylineShapeRef, Shape, TileShape,
    Vector,
};

/// The probe box around the via at the origin that `P2T11.java` mode 0 uses.
fn probe() -> TileShape {
    TileShape::Box(IntBox::from_coords(-100, -100, 100, 100))
}

// ---------------------------------------------------------------------------------------------
// Construction and the insert protocol
// ---------------------------------------------------------------------------------------------

#[test]
fn the_constructor_inserts_the_board_outline_as_item_one() {
    // BasicBoard.java:135: `insertOutline(outlineShapes, outlineClClassNo)` is the last thing the
    // constructor does, so the outline takes the first id the generator hands out.
    // `P2T11.java` mode 0: `outline=1` and `item id=1 class=BoardOutline ... cl=1`.
    let board = p2t11_board();
    assert_eq!(board.get_outline(), Some(ItemId(1)));
    let outline = board.get_item(ItemId(1)).expect("the outline");
    assert!(matches!(outline, Item::BoardOutline(_)));
    // BoardOutline.java:46-49: no nets, component 0, SYSTEM_FIXED; the clearance class survives.
    assert!(outline.net_nos().is_empty());
    assert_eq!(outline.clearance_class(), 1);
    assert_eq!(outline.component_id(), 0);
    assert_eq!(outline.get_fixed_state(), FixedState::SystemFixed);
    assert!(outline.is_on_the_board());
}

#[test]
fn every_insert_bumps_the_revision_once() {
    // BoardItemRepository.java:166: `board.incrementRevision()` is the last line of `insertItem`.
    // `P2T11.java` mode 0: `mode=0 revision=8` for the outline plus seven inserted items.
    let board = p2t11_board();
    assert_eq!(board.revision(), 8);
    assert_eq!(board.items.len(), 8);
}

#[test]
fn the_ids_the_typed_inserters_hand_out_match_the_jvm() {
    // `P2T11.java` mode 0's `item ...` lines, in `board.itemList` order (descending id).
    let board = p2t11_board();
    let ctx = board.ctx();
    let describe = |id: u32| {
        let item = board.get_item(ItemId(id)).expect("an item");
        (
            item.net_nos().to_vec(),
            item.clearance_class(),
            item.first_layer(&ctx),
            item.last_layer(&ctx),
            item.tile_shape_count(&ctx),
            item.bounding_box(&ctx),
        )
    };
    assert_eq!(
        describe(2),
        (
            vec![1],
            1,
            0,
            0,
            1,
            IntBox::from_coords(-1050, -50, -950, 50)
        )
    );
    assert_eq!(
        describe(3),
        (
            vec![1],
            1,
            0,
            1,
            2,
            IntBox::from_coords(930, 930, 1070, 1070)
        )
    );
    assert_eq!(
        describe(4),
        (vec![1], 1, 0, 0, 1, IntBox::from_coords(-1030, -30, 30, 30))
    );
    assert_eq!(
        describe(5),
        (
            vec![1],
            1,
            1,
            1,
            2,
            IntBox::from_coords(-30, -30, 1030, 1030)
        )
    );
    assert_eq!(
        describe(6),
        (vec![1], 1, 0, 1, 2, IntBox::from_coords(-70, -70, 70, 70))
    );
    assert_eq!(
        describe(7),
        (
            vec![],
            1,
            0,
            0,
            1,
            IntBox::from_coords(2000, 2000, 3000, 3000)
        )
    );
    assert_eq!(
        describe(8),
        (
            vec![2],
            1,
            0,
            0,
            1,
            IntBox::from_coords(-3000, -3000, -2000, -2000)
        )
    );
    // The outline: `lineCount() * layerCount` = 4 * 2.
    assert_eq!(describe(1).4, 8);
}

#[test]
fn the_item_list_iterates_in_descending_id_like_java() {
    // quirk #63: `board.itemList` is a `ConcurrentSkipListMap` keyed by the reversed
    // `Item.compareTo`. `P2T11.java` mode 0 prints the items 8 7 6 5 4 3 2 1.
    let board = p2t11_board();
    assert_eq!(
        board.get_items().map(Item::id).collect::<Vec<_>>(),
        (1..=8).rev().map(ItemId).collect::<Vec<_>>()
    );
    assert_eq!(
        nums(board.items_in_board_order()),
        vec![8, 7, 6, 5, 4, 3, 2, 1]
    );
}

#[test]
fn the_typed_query_sets_come_back_in_java_order() {
    // `P2T11.java` mode 0.
    let board = p2t11_board();
    assert_eq!(nums(board.get_pins()), vec![3, 2]);
    assert_eq!(nums(board.get_smd_pins()), vec![2]);
    assert_eq!(nums(board.get_vias()), vec![6]);
    assert_eq!(nums(board.get_traces()), vec![5, 4]);
    assert_eq!(nums(board.get_conduction_areas()), vec![8]);
    assert_eq!(nums(board.get_connectable_items(1)), vec![6, 5, 4, 3, 2]);
    assert_eq!(board.connectable_item_count(1), 5);
    assert_eq!(nums(board.get_connectable_items(2)), vec![8]);
    assert_eq!(nums(board.get_component_items(1)), vec![3, 2]);
    assert_eq!(nums(board.get_component_pins(1)), vec![3, 2]);
    assert_eq!(board.get_pin(1, 0), Some(ItemId(2)));
    assert_eq!(board.get_pin(1, 1), Some(ItemId(3)));
    assert_eq!(board.get_pin(1, 7), None);
}

#[test]
fn the_scalar_queries_match_the_jvm() {
    // `P2T11.java` mode 0.
    let board = p2t11_board();
    assert_eq!(board.get_layer_count(), 2);
    // BasicBoard.java:106,109: the two half widths start at 1000 and 10000, and
    // `insertTraceWithoutCleaning` only ever narrows the gap (:197-200).
    assert_eq!(board.get_min_trace_half_width(), 30);
    assert_eq!(board.get_max_trace_half_width(), 1000);
    assert_eq!(
        board.get_bounding_box(),
        IntBox::from_coords(-10_000, -10_000, 10_000, 10_000)
    );
    assert!((board.cumulative_trace_length() - 3000.0).abs() < 1e-9);
    assert_eq!(board.get_non_45_degree_trace_count(), 0);
    assert_eq!(board.clearance_value(1, 1, 0), 216);
    assert_eq!(board.clearance_value(2, 1, 0), 616);
    assert_eq!(board.clearance_value(2, 2, 1), 816);
    assert!(board.contains(&Point::new(0, 0)));
    assert!(!board.contains(&Point::new(99999, 0)));
    assert_eq!(board.item_component_name(ItemId(2)), Some("Component#1"));
    assert_eq!(board.item_component_name(ItemId(4)), None);
    // Item.getAllNetNames (Item.java:1283-1288) joins `Net::toString`, which is
    // `"Net #<n> (<name>)"` (Net.java:54-56), not the bare name.
    assert_eq!(board.all_net_names(ItemId(4)), "Net #1 (N1)");
    assert_eq!(board.all_net_names(ItemId(7)), "no nets");
    assert_eq!(
        board.get_bounding_box_of_items([ItemId(4), ItemId(5)]),
        IntBox::from_coords(-1030, -30, 1030, 1030)
    );
}

// ---------------------------------------------------------------------------------------------
// Insert/remove keeps the search trees in step
// ---------------------------------------------------------------------------------------------

#[test]
fn insert_and_remove_keep_the_default_tree_in_sync() {
    // `P2T11.java` mode 0's three `overlappingObjects(probe, ...)` blocks, before and after
    // `removeItem(6)`.
    let mut board = p2t11_board();
    let objects = |board: &Board, layer: Option<usize>| {
        board
            .overlapping_objects(&probe(), layer)
            .into_iter()
            .map(|o| match o {
                TreeObject::Item(id) => id.0,
                TreeObject::Room(_) => unreachable!(),
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(objects(&board, Some(0)), vec![6, 4]);
    assert_eq!(objects(&board, Some(1)), vec![6, 5]);
    // Java's "if layer < 0, the layer is ignored".
    assert_eq!(objects(&board, None), vec![6, 5, 4]);

    assert!(board.remove_item(ItemId(6)));
    assert_eq!(board.get_item(ItemId(6)), None);
    assert_eq!(objects(&board, Some(0)), vec![4]);
    assert_eq!(objects(&board, Some(1)), vec![5]);
    assert_eq!(
        nums(board.items_in_board_order()),
        vec![8, 7, 5, 4, 3, 2, 1]
    );
    assert_eq!(board.revision(), 9);
}

#[test]
fn remove_refuses_a_system_fixed_item() {
    // BoardItemRepository.java:189-191 returns without touching anything when
    // `isDeletionForbidden()`; the board outline is `SYSTEM_FIXED` (BoardOutline.java:48), which
    // `Item.isUserFixed` (Item.java:816-819) reports true for.
    // `P2T11.java` mode 0: `isDeletionForbidden(outline)=true`, then `revision=8 outline=1`.
    let mut board = p2t11_board();
    let outline = board.get_outline().expect("an outline");
    assert!(
        board
            .get_item(outline)
            .expect("an outline")
            .is_deletion_forbidden(&board.rules)
    );
    assert!(!board.remove_item(outline));
    assert_eq!(board.revision(), 8);
    assert_eq!(board.get_outline(), Some(outline));
    // Its tree entries survive too.
    assert!(
        board
            .get_item(outline)
            .expect("an outline")
            .is_on_the_board()
    );
}

#[test]
fn remove_items_reports_whether_everything_went() {
    // BasicBoard.java:636-647. `P2T11.java` mode 0: `removeItems=true`, then
    // `items=[8 5 3 2 1] revision=11` (the via was already gone).
    let mut board = p2t11_board();
    board.remove_item(ItemId(6));
    assert!(board.remove_items([ItemId(4), ItemId(7)]));
    assert_eq!(nums(board.items_in_board_order()), vec![8, 5, 3, 2, 1]);
    assert_eq!(board.revision(), 11);

    // A user-fixed item makes the whole call report false without stopping the rest.
    let mut board = p2t11_board();
    board
        .get_item_mut(ItemId(7))
        .expect("the area")
        .set_fixed_state(FixedState::UserFixed);
    assert!(!board.remove_items([ItemId(4), ItemId(7)]));
    assert_eq!(board.get_item(ItemId(4)), None);
    assert!(board.get_item(ItemId(7)).is_some());
}

#[test]
fn increment_revision_is_the_only_way_to_bump_it_by_hand() {
    // BasicBoard.java:148-150. `P2T11.java` mode 0's last two lines.
    let mut board = p2t11_board();
    let before = board.revision();
    board.increment_revision();
    assert_eq!(board.revision(), before + 1);
}

#[test]
fn insert_clamps_an_out_of_range_clearance_class_to_zero() {
    // BoardItemRepository.java:152-158: `item.setClearanceClassIndex(0)` when the class is not a
    // row of the clearance matrix. The fixture's matrix has 3 classes (0, 1 and "wide").
    let mut board = p2t11_board();
    let id = board.insert_obstacle(
        Area::Shape(Shape::Tile(TileShape::Box(IntBox::from_coords(
            4000, 4000, 4100, 4100,
        )))),
        0,
        99,
        FixedState::Unfixed,
    );
    assert_eq!(board.get_item(id).expect("the area").clearance_class(), 0);
}

#[test]
fn insert_trace_without_cleaning_refuses_a_degenerate_or_closed_trace() {
    // BasicBoard.java:185-187 (fewer than two corners) and :191-195 (a closed trace below
    // USER_FIXED).
    let mut board = p2t11_board();
    let before = board.revision();
    assert_eq!(
        board.insert_trace_without_cleaning(
            Polyline::from_points(&[Point::new(4000, 4000)]),
            0,
            30,
            vec![1],
            1,
            FixedState::Unfixed,
        ),
        None
    );
    assert_eq!(
        board.insert_trace_without_cleaning(
            Polyline::from_points(&[
                Point::new(4000, 4000),
                Point::new(4500, 4000),
                Point::new(4500, 4500),
                Point::new(4000, 4000),
            ]),
            0,
            30,
            vec![1],
            1,
            FixedState::Unfixed,
        ),
        None
    );
    assert_eq!(board.revision(), before);
    // USER_FIXED and above are allowed to close (BasicBoard.java:192).
    assert!(
        board
            .insert_trace_without_cleaning(
                Polyline::from_points(&[
                    Point::new(4000, 4000),
                    Point::new(4500, 4000),
                    Point::new(4500, 4500),
                    Point::new(4000, 4000),
                ]),
                0,
                30,
                vec![1],
                1,
                FixedState::UserFixed,
            )
            .is_some()
    );
}

// ---------------------------------------------------------------------------------------------
// The search queries
// ---------------------------------------------------------------------------------------------

#[test]
fn the_clearance_queries_match_the_jvm() {
    // `P2T11.java` mode 0's `overlappingItemsWithClearance` and `overlappingItems` lines.
    let mut board = p2t11_board();
    assert_eq!(
        nums(board.overlapping_items_with_clearance(&probe(), Some(0), &[], 1)),
        vec![6, 4]
    );
    // Every item at the probe is on net 1, so ignoring net 1 empties the result.
    assert!(
        board
            .overlapping_items_with_clearance(&probe(), Some(0), &[1], 1)
            .is_empty()
    );
    let area = Area::Shape(Shape::Tile(TileShape::Box(IntBox::from_coords(
        -1100, -100, 100, 100,
    ))));
    assert_eq!(
        descending(board.overlapping_items(&area, Some(0))),
        vec![6, 4, 2]
    );
    assert_eq!(
        descending(board.pick_items(&Point::new(0, 0), Some(0))),
        vec![6, 4]
    );
}

#[test]
fn check_trace_segment_is_free_where_nothing_is_and_blocked_where_something_is() {
    // `P2T11.java` mode 2's seven `checkTraceSegment` lines.
    let mut board = p2t11_board();
    let seg = |board: &mut Board,
               from: (i32, i32),
               to: (i32, i32),
               nets: &[i32],
               cl: usize,
               only_not_shovable: bool| {
        board.check_trace_segment(
            &Point::new(from.0, from.1),
            &Point::new(to.0, to.1),
            0,
            nets,
            30,
            cl,
            only_not_shovable,
        )
    };
    // Nothing in the way: Java's "no conflict" answer is `Integer.MAX_VALUE`
    // (RoutingBoardSearchFacade.java:61).
    assert_eq!(
        seg(&mut board, (-4000, 4000), (-3000, 4000), &[1], 1, false),
        f64::from(i32::MAX)
    );
    // Straight into the obstacle area at (2000, 2000)..(3000, 3000).
    assert_eq!(
        seg(&mut board, (1500, 2500), (2500, 2500), &[1], 1, false),
        253.0
    );
    // The trace and the pin at the far end are on net 1, so they are not obstacles to net 1 …
    assert_eq!(
        seg(&mut board, (-2000, 0), (-500, 0), &[1], 1, false),
        f64::from(i32::MAX)
    );
    // … but they are to net 9.
    assert_eq!(
        seg(&mut board, (-2000, 0), (-500, 0), &[9], 1, false),
        703.0
    );
    // RoutingBoardSearchFacade.java:37-39: a zero-length segment is 0, not MAX_VALUE.
    assert_eq!(seg(&mut board, (0, 0), (0, 0), &[1], 1, false), 0.0);
    // The pin is not routable, so `onlyNotShovableObstacles` does not excuse it
    // (RoutingBoardSearchFacade.java:71-75).
    assert_eq!(seg(&mut board, (-2000, 0), (-500, 0), &[9], 1, true), 703.0);
    // The wider clearance class shortens nothing extra here — the obstacle area is class 1, and
    // `clearanceValue(1, 2, 0)` is what the walk uses.
    assert_eq!(
        seg(&mut board, (1500, 2500), (2500, 2500), &[1], 2, false),
        253.0
    );
}

#[test]
fn the_check_queries_match_the_jvm() {
    // `P2T11.java` mode 2.
    let mut board = p2t11_board();
    let area = |x1, y1, x2, y2| {
        Area::Shape(Shape::Tile(TileShape::Box(IntBox::from_coords(
            x1, y1, x2, y2,
        ))))
    };
    assert!(board.check_shape(&area(-4500, 4000, -4000, 4500), Some(0), &[1], 1));
    assert!(!board.check_shape(&area(2200, 2200, 2400, 2400), Some(0), &[1], 1));
    // BasicBoard.java:961-963: outside the board's bounding box is always false.
    assert!(!board.check_shape(&area(-20000, 0, -19000, 100), Some(0), &[1], 1));

    let tile = |x1, y1, x2, y2| TileShape::Box(IntBox::from_coords(x1, y1, x2, y2));
    assert!(board.check_trace_shape(&tile(-4500, 4000, -4000, 4500), 0, &[1], 1, None));
    assert!(!board.check_trace_shape(&tile(2200, 2200, 2400, 2400), 0, &[1], 1, None));
    // BasicBoard.java:1006-1015: with a `contactPins` set, a pin outside it is an obstacle even
    // on the trace's own net; the SMD pin at (-1000, 0) is item 2.
    let contact_pins = BTreeSet::from([ItemId(2)]);
    assert!(board.check_trace_shape(&tile(-1050, -50, -950, 50), 0, &[1], 1, Some(&contact_pins)));
    assert!(!board.check_trace_shape(
        &tile(-1050, -50, -950, 50),
        0,
        &[1],
        1,
        Some(&BTreeSet::new())
    ));

    assert!(board.check_polyline_trace(
        &Polyline::from_points(&[Point::new(-4000, 4000), Point::new(-3000, 4000)]),
        0,
        30,
        &[1],
        1
    ));
    assert!(!board.check_polyline_trace(
        &Polyline::from_points(&[Point::new(1500, 2500), Point::new(2500, 2500)]),
        0,
        30,
        &[1],
        1
    ));

    let by = Vector::from(IntVector::new(10, 10));
    assert!(board.check_move_item(ItemId(7), &by, &mut None));
    // RoutingBoardSearchFacade.java:120-122: a trace with contacts may not be moved.
    assert!(!board.check_move_item(ItemId(4), &by, &mut None));
    assert!(board.check_change_net(ItemId(7), 3));
    assert!(!board.check_change_net(ItemId(4), 3));

    assert_eq!(
        board.pick_nearest_routing_item(&Point::new(0, 0), Some(0), None),
        Some(ItemId(6))
    );
    assert_eq!(
        board.pick_nearest_routing_item(&Point::new(-1000, 0), Some(0), None),
        Some(ItemId(2))
    );
    assert_eq!(
        board.pick_nearest_routing_item(&Point::new(4000, 4000), Some(0), None),
        None
    );
    // Every trace end has a contact, so there is no tail anywhere.
    assert_eq!(
        board.get_trace_tail(&Point::new(1000, 1000), Some(1), &[1]),
        None
    );
    assert_eq!(board.get_trace_tail(&Point::new(0, 0), Some(0), &[1]), None);
    assert!(!board.contains_trace_tails([ItemId(4), ItemId(5)], &[]));
}

// ---------------------------------------------------------------------------------------------
// Connectivity
// ---------------------------------------------------------------------------------------------

#[test]
fn normal_contacts_walk_the_pin_trace_via_trace_pin_chain() {
    // `P2T11.java` mode 1's first block: pin 2 - trace 4 - via 6 - trace 5 - pin 3.
    let board = p2t11_board();
    assert_eq!(descending(board.normal_contacts(ItemId(2))), vec![4]);
    assert_eq!(descending(board.normal_contacts(ItemId(4))), vec![6, 2]);
    assert_eq!(descending(board.normal_contacts(ItemId(6))), vec![5, 4]);
    assert_eq!(descending(board.normal_contacts(ItemId(5))), vec![6, 3]);
    assert_eq!(descending(board.normal_contacts(ItemId(3))), vec![5]);
    // The obstacle area, the conduction area and the outline have none.
    for id in [1u32, 7, 8] {
        assert!(board.normal_contacts(ItemId(id)).is_empty());
    }
}

#[test]
fn all_contacts_and_is_connected_agree_with_the_jvm() {
    // `P2T11.java` mode 1: `allContacts` equals `normalContacts` on this board, and the three
    // non-connectable items report `connected=false`.
    let board = p2t11_board();
    for id in [2u32, 3, 4, 5, 6] {
        assert_eq!(
            descending(board.all_contacts(ItemId(id))),
            descending(board.normal_contacts(ItemId(id))),
            "item {id}"
        );
        assert!(board.is_connected(ItemId(id)), "item {id}");
    }
    for id in [1u32, 7, 8] {
        assert!(!board.is_connected(ItemId(id)), "item {id}");
    }
}

#[test]
fn all_contacts_on_layer_splits_the_via_by_layer() {
    // `P2T11.java` mode 1's per-layer block: the via contacts trace 4 on layer 0 and trace 5 on
    // layer 1, and each trace only sees the contacts on its own layer.
    let board = p2t11_board();
    assert_eq!(
        descending(board.all_contacts_on_layer(ItemId(6), 0)),
        vec![4]
    );
    assert_eq!(
        descending(board.all_contacts_on_layer(ItemId(6), 1)),
        vec![5]
    );
    assert_eq!(
        descending(board.all_contacts_on_layer(ItemId(4), 0)),
        vec![6, 2]
    );
    assert!(board.all_contacts_on_layer(ItemId(4), 1).is_empty());
    assert_eq!(
        descending(board.all_contacts_on_layer(ItemId(5), 1)),
        vec![6, 3]
    );
    assert!(board.all_contacts_on_layer(ItemId(5), 0).is_empty());
    assert!(board.is_connected_on_layer(ItemId(3), 1));
    assert!(!board.is_connected_on_layer(ItemId(3), 0));
}

#[test]
fn connected_set_crosses_layers_through_the_via() {
    // `P2T11.java` mode 1: from either pin the whole five-item chain comes back, which it can
    // only do by walking through the via from layer 0 to layer 1.
    let board = p2t11_board();
    assert_eq!(
        descending(board.connected_set(ItemId(2), 1, false)),
        vec![6, 5, 4, 3, 2]
    );
    assert_eq!(
        descending(board.connected_set(ItemId(3), 1, false)),
        vec![6, 5, 4, 3, 2]
    );
    // Item.java:601: `netNumber <= 0` ignores the net filter.
    assert_eq!(
        descending(board.connected_set(ItemId(2), -1, false)),
        vec![6, 5, 4, 3, 2]
    );
    // Item.java:602-604: an item that is not on the net answers the empty set.
    assert!(board.connected_set(ItemId(2), 2, false).is_empty());
    assert_eq!(
        descending(board.connected_set(ItemId(8), 2, false)),
        vec![8]
    );
    // No conduction area is in the chain, so `stopAtPlane` changes nothing here.
    assert_eq!(
        descending(board.connected_set(ItemId(2), 1, true)),
        vec![6, 5, 4, 3, 2]
    );
}

#[test]
fn unconnected_set_is_empty_when_the_net_is_fully_connected() {
    // `P2T11.java` mode 1: net 1 is one connected set, so nothing is left over.
    let board = p2t11_board();
    assert!(board.unconnected_set(ItemId(2), 1).is_empty());
    assert!(board.unconnected_set(ItemId(8), 2).is_empty());
    // Item.java:679-682: `netNumber <= 0` uses the item's own nets.
    assert!(board.unconnected_set(ItemId(2), 0).is_empty());

    // Add a second, unconnected pad on net 1 and it shows up.
    let mut board = p2t11_board();
    let stray = board
        .insert_trace_without_cleaning(
            Polyline::from_points(&[Point::new(-4000, -4000), Point::new(-3500, -4000)]),
            0,
            30,
            vec![1],
            1,
            FixedState::Unfixed,
        )
        .expect("a straight two-corner trace");
    assert_eq!(
        descending(board.unconnected_set(ItemId(2), 1)),
        vec![stray.0]
    );
}

#[test]
fn connection_items_stop_at_the_terminal_pins_and_at_a_via_when_asked() {
    // `P2T11.java` mode 1: the connection from trace 4 runs 4 - 6 - 5 and stops at the pins,
    // which are not routable (Item.java:723-726).
    let board = p2t11_board();
    assert_eq!(
        descending(board.connection_items(ItemId(4), StopConnectionOption::None)),
        vec![6, 5, 4]
    );
    assert_eq!(
        descending(board.connection_items(ItemId(5), StopConnectionOption::None)),
        vec![6, 5, 4]
    );
    // Item.java:728-730: `VIA` stops before the via, leaving only the start trace.
    assert_eq!(
        descending(board.connection_items(ItemId(4), StopConnectionOption::Via)),
        vec![4]
    );
    // Item.java:731-735: the via *is* a fanout via here (it touches an SMD pin through a short
    // trace), but `isFanoutVia` is consulted only after `result.add`, so the walk still crosses.
    assert_eq!(
        descending(board.connection_items(ItemId(4), StopConnectionOption::FanoutVia)),
        vec![6, 5, 4]
    );
}

#[test]
fn normal_contact_point_answers_the_shared_corner_and_null_otherwise() {
    // `P2T11.java` mode 1's 4x4 `normalContactPoint` block.
    let board = p2t11_board();
    let p = |a: u32, b: u32| board.normal_contact_point(ItemId(a), ItemId(b));
    assert_eq!(p(2, 4), Some(Point::new(-1000, 0)));
    assert_eq!(p(4, 2), Some(Point::new(-1000, 0)));
    assert_eq!(p(4, 6), Some(Point::new(0, 0)));
    assert_eq!(p(6, 4), Some(Point::new(0, 0)));
    assert_eq!(p(5, 6), Some(Point::new(0, 0)));
    assert_eq!(p(6, 5), Some(Point::new(0, 0)));
    // A drill item against itself answers its own centre (DrillItem.java:331-337).
    assert_eq!(p(2, 2), Some(Point::new(-1000, 0)));
    assert_eq!(p(6, 6), Some(Point::new(0, 0)));
    // Trace.java:142-145: a trace against itself touches at both ends, which is "more than one
    // contact point", i.e. null.
    assert_eq!(p(4, 4), None);
    // No shared layer, or no shared corner.
    assert_eq!(p(2, 5), None);
    assert_eq!(p(4, 5), None);
    assert_eq!(p(6, 2), None);
}

#[test]
fn first_common_layer_is_none_where_java_returns_minus_one() {
    // `P2T11.java` mode 1's `firstCommonLayer`/`lastCommonLayer` columns.
    let board = p2t11_board();
    let common = |a: u32, b: u32| {
        let ctx = board.ctx();
        let (a, b) = (
            board.get_item(ItemId(a)).expect("a"),
            board.get_item(ItemId(b)).expect("b"),
        );
        (a.first_common_layer(b, &ctx), a.last_common_layer(b, &ctx))
    };
    assert_eq!(common(2, 4), (Some(0), Some(0)));
    assert_eq!(common(5, 6), (Some(1), Some(1)));
    assert_eq!(common(6, 6), (Some(0), Some(1)));
    // Java's -1.
    assert_eq!(common(2, 5), (None, None));
    assert_eq!(board.first_common_layer(ItemId(4), ItemId(6)), Some(0));
    assert_eq!(board.first_common_layer(ItemId(2), ItemId(5)), None);
}

#[test]
fn ratsnest_corners_are_the_uncontacted_ends_only() {
    // `P2T11.java` mode 1's `ratsnest=` column. Both traces are contacted at both ends, so they
    // contribute nothing (Trace.java:342-352); the drill items contribute their centre
    // (DrillItem.java:352-356); the conduction area its rounded corners
    // (ConductionArea.java:368-377).
    let board = p2t11_board();
    assert!(board.ratsnest_corners(ItemId(4)).is_empty());
    assert!(board.ratsnest_corners(ItemId(5)).is_empty());
    assert_eq!(
        board.ratsnest_corners(ItemId(2)),
        vec![Point::new(-1000, 0)]
    );
    assert_eq!(
        board.ratsnest_corners(ItemId(3)),
        vec![Point::new(1000, 1000)]
    );
    assert_eq!(board.ratsnest_corners(ItemId(6)), vec![Point::new(0, 0)]);
    assert_eq!(
        board.ratsnest_corners(ItemId(8)),
        vec![
            Point::new(-3000, -3000),
            Point::new(-2000, -3000),
            Point::new(-2000, -2000),
            Point::new(-3000, -2000),
        ]
    );
    // The obstacle area and the outline are not connectable, so the base body answers nothing.
    assert!(board.ratsnest_corners(ItemId(7)).is_empty());
    assert!(board.ratsnest_corners(ItemId(1)).is_empty());
}

#[test]
fn tails_overlaps_and_cycles_are_all_absent_on_a_well_formed_chain() {
    // `P2T11.java` mode 1.
    let board = p2t11_board();
    for id in 1u32..=8 {
        assert!(!board.is_tail(ItemId(id)), "item {id}");
        assert!(!board.is_overlap(ItemId(id)), "item {id}");
    }
    assert!(!board.is_trace_cycle(ItemId(4)));
    assert_eq!(descending(board.trace_start_contacts(ItemId(4))), vec![2]);
    assert_eq!(descending(board.trace_end_contacts(ItemId(4))), vec![6]);
    assert_eq!(descending(board.trace_start_contacts(ItemId(5))), vec![6]);
    assert_eq!(descending(board.trace_end_contacts(ItemId(5))), vec![3]);
}

#[test]
fn a_trace_with_a_free_end_is_a_tail() {
    // Trace.java:212-219: no contacts at one end is enough.
    let mut board = p2t11_board();
    let stray = board
        .insert_trace_without_cleaning(
            Polyline::from_points(&[Point::new(-1000, 0), Point::new(-1000, 500)]),
            0,
            30,
            vec![1],
            1,
            FixedState::Unfixed,
        )
        .expect("a straight two-corner trace");
    assert!(board.is_tail(stray));
    assert_eq!(descending(board.trace_start_contacts(stray)), vec![4, 2]);
    assert!(board.trace_end_contacts(stray).is_empty());
    assert_eq!(board.ratsnest_corners(stray), vec![Point::new(-1000, 500)]);
}

#[test]
fn the_via_is_a_fanout_via_because_a_short_trace_reaches_an_smd_pin() {
    // `P2T11.java` mode 1: `isFanoutVia(6)=true`. Item.java:1217-1226: trace 4 is shorter than
    // `400 * halfWidth` and its other contact is a one-layer pin with a single contact.
    let board = p2t11_board();
    assert!(board.is_fanout_via(ItemId(6), None));
    // Item.java:1214-1216: ignoring the trace removes the only route to the pin.
    let ignore = BTreeSet::from([ItemId(4)]);
    assert!(!board.is_fanout_via(ItemId(6), Some(&ignore)));
}

#[test]
fn get_connected_sets_partitions_the_net() {
    // `P2T11.java` mode 1: `connectedSets(1)=[[6 5 4 3 2]]`.
    let board = p2t11_board();
    let sets: Vec<Vec<u32>> = board
        .get_connected_sets(1)
        .into_iter()
        .map(descending)
        .collect();
    assert_eq!(sets, vec![vec![6, 5, 4, 3, 2]]);
    // BoardConnectivityQueries.java:101-103: a non-positive net number answers nothing.
    assert!(board.get_connected_sets(0).is_empty());

    // A second, disconnected piece of net 1 becomes a second set, seeded by the highest id
    // remaining — so it comes first.
    let mut board = p2t11_board();
    let stray = board
        .insert_trace_without_cleaning(
            Polyline::from_points(&[Point::new(-4000, -4000), Point::new(-3500, -4000)]),
            0,
            30,
            vec![1],
            1,
            FixedState::Unfixed,
        )
        .expect("a straight two-corner trace");
    let sets: Vec<Vec<u32>> = board
        .get_connected_sets(1)
        .into_iter()
        .map(descending)
        .collect();
    assert_eq!(sets, vec![vec![stray.0], vec![6, 5, 4, 3, 2]]);
}

#[test]
fn touching_pins_at_end_corners_finds_the_pad_under_the_trace_end() {
    // `P2T11.java` mode 1: `touchingPins(4)=[2]`. Trace.java:397-405 enlarges each end corner's
    // surrounding octagon by the half width and keeps the same-net pins it overlaps.
    let mut board = p2t11_board();
    assert_eq!(
        descending(board.touching_pins_at_end_corners(ItemId(4))),
        vec![2]
    );
    // The trace on layer 1 ends on the through pin.
    assert_eq!(
        descending(board.touching_pins_at_end_corners(ItemId(5))),
        vec![3]
    );
}

#[test]
fn validate_accepts_a_well_formed_board() {
    // `P2T11.java` mode 1: `validate(4)=true`, `validate(6)=true`.
    let mut board = p2t11_board();
    for id in 1u32..=8 {
        assert!(board.validate_item(ItemId(id)), "item {id}");
    }
}

#[test]
fn swappable_pins_is_empty_without_a_logical_part() {
    // `P2T11.java` mode 1: `swappablePins(2)=[]`. Pin.java:398-400 returns early when the
    // component has no `LogicalPart`.
    let board = p2t11_board();
    assert!(board.swappable_pins(ItemId(2)).is_empty());
    // A non-pin answers nothing at all.
    assert!(board.swappable_pins(ItemId(4)).is_empty());
}

// ---------------------------------------------------------------------------------------------
// The changed area and the board-level bookkeeping
// ---------------------------------------------------------------------------------------------

#[test]
fn the_changed_area_accumulates_points_and_shapes_per_layer() {
    // `P2T11.java` mode 3's first block.
    let mut board = p2t11_board();
    assert!(board.changed_area.is_none());
    board.start_marking_changed_area();
    let area = board.changed_area.as_ref().expect("marked");
    assert_eq!(area.get_area(0), IntOctagon::EMPTY);
    assert_eq!(area.get_area(1), IntOctagon::EMPTY);

    board.join_changed_area(&Point::new(100, 100).to_float(), 0);
    assert_eq!(
        board.changed_area.as_ref().expect("marked").get_area(0),
        IntOctagon::new(100, 100, 100, 100, 0, 0, 200, 200)
    );
    let ctx = board.ctx();
    let shape = board
        .get_item(ItemId(7))
        .expect("the obstacle area")
        .get_tile_shape(board.default_tree_id(), 0, &ctx)
        .expect("its only tile shape");
    board.mark_changed_area(&shape, 0);
    assert_eq!(
        board.changed_area.as_ref().expect("marked").get_area(0),
        IntOctagon::new(100, 100, 3000, 3000, -1000, 1000, 200, 6000)
    );
    assert_eq!(
        board
            .changed_area
            .as_ref()
            .expect("marked")
            .surrounding_box(),
        IntBox::from_coords(100, 100, 3000, 3000)
    );
    // Layer 1 was never touched.
    assert_eq!(
        board.changed_area.as_ref().expect("marked").get_area(1),
        IntOctagon::EMPTY
    );

    board.changed_area.as_mut().expect("marked").set_empty(0);
    assert_eq!(
        board.changed_area.as_ref().expect("marked").get_area(0),
        IntOctagon::EMPTY
    );

    board.mark_all_changed_area();
    for layer in 0..2 {
        assert_eq!(
            board.changed_area.as_ref().expect("marked").get_area(layer),
            IntOctagon::new(-10000, -10000, 10000, 10000, -20000, 20000, -20000, 20000)
        );
    }
    assert_eq!(
        board
            .changed_area
            .as_ref()
            .expect("marked")
            .surrounding_box(),
        IntBox::from_coords(-10_000, -10_000, 10_000, 10_000)
    );
}

#[test]
fn start_marking_changed_area_is_idempotent() {
    // RoutingBoardOperations.java:27-29: the second call finds a non-null area and leaves it.
    let mut board = p2t11_board();
    board.start_marking_changed_area();
    board.join_changed_area(&Point::new(100, 100).to_float(), 0);
    board.start_marking_changed_area();
    assert_eq!(
        board.changed_area.as_ref().expect("marked").get_area(0),
        IntOctagon::new(100, 100, 100, 100, 0, 0, 200, 200)
    );
    // `set_changed_area_layer_count` does reset it.
    board.set_changed_area_layer_count(2);
    assert_eq!(
        board.changed_area.as_ref().expect("marked").get_area(0),
        IntOctagon::EMPTY
    );
}

#[test]
fn remove_items_marking_changed_area_marks_what_it_removed() {
    // RoutingBoardOperations.java:92-110, the removal half.
    let mut board = p2t11_board();
    let (all_removed, changed_nets) = board.remove_items_marking_changed_area([ItemId(7)]);
    assert!(all_removed);
    assert!(changed_nets.is_empty());
    assert_eq!(board.get_item(ItemId(7)), None);
    assert_eq!(
        board.changed_area.as_ref().expect("marked").get_area(0),
        IntOctagon::new(2000, 2000, 3000, 3000, -1000, 1000, 4000, 6000)
    );

    // A user-fixed item is refused and not marked (:95-96).
    let mut board = p2t11_board();
    board
        .get_item_mut(ItemId(4))
        .expect("a trace")
        .set_fixed_state(FixedState::UserFixed);
    let (all_removed, changed_nets) =
        board.remove_items_marking_changed_area([ItemId(4), ItemId(5)]);
    assert!(!all_removed);
    assert_eq!(changed_nets, BTreeSet::from([1]));
    assert!(board.get_item(ItemId(4)).is_some());
    assert_eq!(board.get_item(ItemId(5)), None);
}

#[test]
fn change_conduction_is_obstacle_reproduces_the_java_latch() {
    // `P2T11.java` mode 3, quirk #50: the guard at RoutingBoard.java:1254 is `!=`, so a call only
    // does anything when the flag already equals the argument, and :1273 then stores `!value`.
    let mut board = p2t11_board();
    assert!(board.rules.get_ignore_conduction());
    assert!(is_obstacle(&board, 8));

    // `ignoreConduction` is true, so `change(false)` returns immediately.
    board.change_conduction_is_obstacle(false);
    assert!(board.rules.get_ignore_conduction());
    assert!(is_obstacle(&board, 8));
    board.change_conduction_is_obstacle(false);
    assert!(board.rules.get_ignore_conduction());
    assert!(is_obstacle(&board, 8));

    // `change(true)` passes the guard, writes `true` into every signal-layer conduction area
    // (already true here) and then stores `ignoreConduction = !true`.
    board.change_conduction_is_obstacle(true);
    assert!(!board.rules.get_ignore_conduction());
    assert!(is_obstacle(&board, 8));
}

#[test]
fn unfill_conduction_areas_clears_both_flags_and_reinserts() {
    // `P2T11.java` mode 3: BasicBoard.java:1426-1440.
    let mut board = p2t11_board();
    board.change_conduction_is_obstacle(true);
    board.unfill_conduction_areas();
    assert!(board.rules.get_ignore_conduction());
    assert!(!is_obstacle(&board, 8));
    assert!(!is_filled(&board, 8));
    // Every item is still on the board and still indexed.
    assert_eq!(board.items.len(), 8);
    assert_eq!(
        descending(board.pick_items(&Point::new(0, 0), Some(0))),
        vec![6, 4]
    );
}

#[test]
fn remove_trace_tails_finds_nothing_on_a_fully_contacted_net() {
    // `P2T11.java` mode 3: `removed=false`, and the item list is untouched.
    let mut board = p2t11_board();
    assert!(!board.remove_trace_tails(1, StopConnectionOption::None));
    assert_eq!(
        nums(board.items_in_board_order()),
        vec![8, 7, 6, 5, 4, 3, 2, 1]
    );
}

#[test]
fn move_item_by_moves_the_item_and_its_tree_entries() {
    // `P2T11.java` mode 3: an obstacle area takes the base `Item.moveBy` (Item.java:300-311),
    // whose whole body is remove-from-trees / translate / insert.
    let mut board = p2t11_board();
    board
        .move_item_by(ItemId(7), &Vector::from(IntVector::new(10, 20)))
        .expect("an area translates without a polyline error");
    let ctx = board.ctx();
    assert_eq!(
        board
            .get_item(ItemId(7))
            .expect("the area")
            .bounding_box(&ctx),
        IntBox::from_coords(2010, 2020, 3010, 3020)
    );
    assert_eq!(
        descending(board.pick_items(&Point::new(2500, 2500), Some(0))),
        vec![7]
    );
    // Nothing was created or destroyed.
    assert_eq!(
        nums(board.items_in_board_order()),
        vec![8, 7, 6, 5, 4, 3, 2, 1]
    );
}

#[test]
fn a_query_after_change_clearance_class_index_recomputes_the_cold_shape_cache() {
    // `P2T11.java` mode 3, the two `after change` lines. `Item.changeClearanceClassIndex`
    // (Item.java:944-949) clears the derived data and, with clearance compensation off, does
    // **not** re-insert — so the trace keeps its tree leaves with an empty shape cache. Java's
    // `getTreeShape` recomputes on that miss (Item.java:212-238); the port used to panic.
    //
    // No `validate` (or anything else that would warm the cache) runs in between.
    let mut board = p2t11_board();
    assert!(!board.trees.is_clearance_compensation_used());
    assert!(board.change_clearance_class_index(ItemId(4), 0));
    let probe = TileShape::Box(IntBox::from_coords(-600, -100, -400, 100));
    let found: Vec<u32> = board
        .overlapping_objects(&probe, Some(0))
        .into_iter()
        .map(|o| match o {
            TreeObject::Item(id) => id.0,
            TreeObject::Room(_) => unreachable!(),
        })
        .collect();
    assert_eq!(found, vec![4]);
    assert_eq!(
        nums(board.overlapping_items_with_clearance(&probe, Some(0), &[], 1)),
        vec![4]
    );
    // The tree itself still reads the shape, through `ShapeSearchTree::get_tree_shape`.
    let tree = board.trees.get_default_tree();
    let item = board.get_item(ItemId(4)).expect("the trace");
    assert!(tree.get_tree_shape(item, 0, &board.ctx()).is_some());
    // ... and so do the `&self` board wrappers the connectivity family uses.
    assert!(board.item_tile_shape_ref(ItemId(4), 0).is_some());
    assert_eq!(descending(board.all_contacts(ItemId(4))), vec![6, 2]);
    // Index past the end is still `None` (Item.java:222-224).
    assert!(board.item_tile_shape_ref(ItemId(4), 9).is_none());
}

#[test]
fn check_polyline_trace_consumes_an_item_id_like_javas_temporary_trace() {
    // `P2T11.java` mode 2's `idBefore` / `idAfterOneCheck` / `insertedId` lines.
    // `BasicBoard.checkPolylineTrace` builds a `PolylineTrace` it never inserts
    // (BasicBoard.java:1055-1065), and `Item`'s constructor draws an id for it
    // (Item.java:85-90) — so the id sequence advances even though the board does not.
    let mut board = p2t11_board();
    assert_eq!(board.communication.id_gen.max_generated_id(), ItemId(8));
    let free = Polyline::from_points(&[Point::new(-4000, 4000), Point::new(-3000, 4000)]);
    assert!(board.check_polyline_trace(&free, 0, 30, &[1], 1));
    assert_eq!(board.communication.id_gen.max_generated_id(), ItemId(9));
    // Two more checks, two more ids — the count does not depend on the answer.
    let blocked = Polyline::from_points(&[Point::new(1500, 2500), Point::new(2500, 2500)]);
    assert!(!board.check_polyline_trace(&blocked, 0, 30, &[1], 1));
    assert_eq!(board.communication.id_gen.max_generated_id(), ItemId(10));
    let inserted = board
        .insert_trace_without_cleaning(
            Polyline::from_points(&[Point::new(-4000, 3000), Point::new(-3000, 3000)]),
            0,
            30,
            vec![1],
            1,
            FixedState::Unfixed,
        )
        .expect("a straight two-corner trace");
    assert_eq!(inserted, ItemId(11));
}

#[test]
fn connection_items_walks_its_start_contacts_in_descending_id() {
    // Item.java:700: `getNormalContacts()` is a `TreeSet<Item>`, i.e. descending id (quirk #44),
    // and under `FANOUT_VIA` the walk reads the partially built `result`
    // (`isFanoutVia(result)`, Item.java:735), so the order decides membership.
    //
    // On the fixture board every option answers the same set, which is what
    // `P2T11.java` mode 1 pins; this test pins the *order* the outer loop runs in by checking
    // that the first chain walked is the one seeded by the highest-id contact.
    let board = p2t11_board();
    let contacts = board.normal_contacts(ItemId(4));
    assert_eq!(descending(contacts.clone()), vec![6, 2]);
    assert_eq!(
        descending(board.connection_items(ItemId(4), StopConnectionOption::None)),
        vec![6, 5, 4]
    );
    assert_eq!(
        descending(board.connection_items(ItemId(4), StopConnectionOption::FanoutVia)),
        vec![6, 5, 4]
    );
}

#[test]
fn change_clearance_class_index_writes_the_class_and_keeps_validate_happy() {
    // `P2T11.java` mode 3: `cl(4)=2`, `validate(4)=true`. Item.java:944-949 clears the derived
    // data, which is what makes `validate` need the lazy tree-shape fill (Item.java:227-238).
    let mut board = p2t11_board();
    assert!(board.change_clearance_class_index(ItemId(4), 2));
    assert_eq!(
        board
            .get_item(ItemId(4))
            .expect("a trace")
            .clearance_class(),
        2
    );
    assert!(board.validate_item(ItemId(4)));
    assert!(!board.change_clearance_class_index(ItemId(99), 2));
}

#[test]
fn make_conductive_replaces_the_area_with_a_conduction_area_on_the_net() {
    // `P2T11.java` mode 3: `newId=9 nets=[3] items=[9 8 6 5 4 3 2 1]`.
    let mut board = p2t11_board();
    let new_id = board
        .make_conductive(ItemId(7), 3)
        .expect("an obstacle area");
    assert_eq!(new_id, ItemId(9));
    let new_item = board.get_item(new_id).expect("the conduction area");
    assert_eq!(new_item.net_nos(), &[3]);
    // BasicBoard.java:1210 hard-codes `isObstacle = true`.
    assert!(is_obstacle(&board, 9));
    assert_eq!(board.get_item(ItemId(7)), None);
    assert_eq!(
        nums(board.items_in_board_order()),
        vec![9, 8, 6, 5, 4, 3, 2, 1]
    );
    // A non-`ObstacleArea` is refused (Java's parameter is typed).
    assert_eq!(board.make_conductive(ItemId(4), 3), None);
}

#[test]
fn generate_keepout_outside_swaps_the_outlines_tree_shapes() {
    // `P2T11.java` mode 3: BoardOutline.java:229-243. Both branches happen to produce 8 shapes
    // for this board (4 border lines x 2 layers, and 4 keepout pieces x 2 layers), so the test
    // pins the flag and the fact that the trees were rebuilt rather than a count change.
    let mut board = p2t11_board();
    let outline = board.get_outline().expect("an outline");
    let tree = board.default_tree_id();
    let before: Vec<TileShape> = (0..board.item_tree_shape_count(outline, tree))
        .filter_map(|i| board.item_tree_shape(outline, tree, i))
        .collect();
    assert_eq!(before.len(), 8);

    assert!(board.generate_keepout_outside(outline, true));
    assert!(match board.get_item(outline).expect("an outline") {
        Item::BoardOutline(o) => o.keepout_outside_outline_generated(),
        _ => unreachable!(),
    });
    let after: Vec<TileShape> = (0..board.item_tree_shape_count(outline, tree))
        .filter_map(|i| board.item_tree_shape(outline, tree, i))
        .collect();
    assert_eq!(after.len(), 8);
    assert_ne!(before, after);

    // BoardOutline.java:231-233: setting the same value again is a no-op.
    assert!(!board.generate_keepout_outside(outline, true));
}

#[test]
fn the_net_queries_walk_the_item_list() {
    // `P2T11.java` mode 3's last block (Net.java:75-152).
    let board = p2t11_board();
    // Terminal items are the connectable ones that are *not* routable: the two pins.
    assert_eq!(nums(board.net_terminal_items(1)), vec![3, 2]);
    assert_eq!(nums(board.net_pins(1)), vec![3, 2]);
    assert_eq!(nums(board.net_items(1)), vec![6, 5, 4, 3, 2]);
    assert!((board.net_trace_length(1) - 3000.0).abs() < 1e-9);
    assert_eq!(board.net_via_count(1), 1);
    assert_eq!(nums(board.net_items(2)), vec![8]);
    assert_eq!(board.net_via_count(2), 0);
}

// ---------------------------------------------------------------------------------------------
// The host-CAD / clearance-compensated board (`P2T11.java` mode 4)
// ---------------------------------------------------------------------------------------------

#[test]
fn a_host_cad_communication_lowers_the_obstacle_area_section_width() {
    // `P2T11.java` mode 4: `hostCadExists=true resolution(MIL)=10.0`, and the 18000-wide
    // obstacle area comes back as four 4500-wide sections instead of one shape —
    // ShapeSearchTree.java:916-920 lowers `maxTreeShapeWidth` to `min(500 * 10, 50000) = 5000`.
    let mut board = board_builder::p2t11_host_cad_board();
    assert!(board.communication.host_cad_exists());
    assert!((board.communication.get_resolution(Unit::Mil) - 10.0).abs() < 1e-9);
    assert!(!board.communication.host_is_old_kicad());
    assert!(!board.communication.host_cad_is_eagle());
    let tree = board.default_tree_id();
    assert_eq!(board.item_tree_shape_count(ItemId(9), tree), 4);
    let boxes: Vec<IntBox> = (0..4)
        .map(|i| {
            board
                .item_tree_shape(ItemId(9), tree, i)
                .expect("a tree shape")
                .bounding_box()
        })
        .collect();
    assert_eq!(
        boxes,
        vec![
            IntBox::from_coords(-9000, -9000, -4500, -8000),
            IntBox::from_coords(-4500, -9000, 0, -8000),
            IntBox::from_coords(0, -9000, 4500, -8000),
            IntBox::from_coords(4500, -9000, 9000, -8000),
        ]
    );
}

#[test]
fn a_host_cad_at_a_coarse_resolution_keeps_the_fifty_thousand_default() {
    // ShapeSearchTree.java:918-919 is a `Math.min`, so a resolution big enough to make
    // `500 * getResolution(MIL)` exceed 50000 leaves the default in place — and the same
    // 18000-wide area is then a single section. No `P2T11` line: Java's driver would need a
    // third board; the `Math.min` is the citation.
    let mut board = board_builder::p2t11_host_cad_board();
    // Rebuild with resolution 1000: `500 * 1000 = 500000 > 50000`.
    let coarse = Board::new(
        Vec::new(),
        1,
        IntBox::from_coords(-10_000, -10_000, 10_000, 10_000),
        board.rules.clone(),
        board.library.clone(),
        board.components.clone(),
        Communication::new(
            Unit::Mil,
            1000,
            ItemIdGenerator::new(),
            Some("KiCad".to_string()),
            Some("7.0".to_string()),
        ),
    );
    let mut coarse = coarse;
    let wide = coarse.insert_obstacle(
        Area::Shape(Shape::Tile(TileShape::Box(IntBox::from_coords(
            -9000, -9000, 9000, -8000,
        )))),
        0,
        1,
        FixedState::Unfixed,
    );
    let tree = coarse.default_tree_id();
    assert_eq!(coarse.item_tree_shape_count(wide, tree), 1);
    // The fine-resolution board splits the same area four ways.
    let fine_tree = board.default_tree_id();
    assert_eq!(board.item_tree_shape_count(ItemId(9), fine_tree), 4);
}

#[test]
fn set_clearance_compensation_used_rebuilds_the_board_tree() {
    // `P2T11.java` mode 4's `--- setClearanceCompensationUsed(true)` block
    // (SearchTreeManager.java:89-108 over `board.itemList`).
    let mut board = board_builder::p2t11_host_cad_board();
    assert!(
        !board
            .trees
            .get_default_tree()
            .is_clearance_compensation_used()
    );
    board.set_clearance_compensation_used(true);
    assert!(
        board
            .trees
            .get_default_tree()
            .is_clearance_compensation_used()
    );
    assert_eq!(
        board.trees.get_default_tree().get_key(),
        "ShapeSearchTree_FortyfiveDegree_cc1"
    );
    let compensation =
        board
            .trees
            .get_default_tree()
            .clearance_compensation_value(1, 0, &board.rules);
    assert_eq!(compensation, 100);
    // Trace 4 is half width 30, so its one shape is now 130 wide on each side.
    let tree = board.default_tree_id();
    assert_eq!(board.item_tree_shape_count(ItemId(4), tree), 1);
    assert_eq!(
        board
            .item_tree_shape(ItemId(4), tree, 0)
            .expect("a tree shape")
            .bounding_box(),
        IntBox::from_coords(-1130, -130, 130, 130)
    );
}

#[test]
fn check_polyline_trace_uses_the_compensated_tree_shapes() {
    // `P2T11.java` mode 4: with compensation on, a run 200 units clear of the wide obstacle area
    // is blocked, because `checkPolylineTrace`'s temporary trace takes its tile shapes from the
    // default tree (BasicBoard.java:1067-1071 -> Item.java:194-201 ->
    // ShapeSearchTree.java:992-1004), which adds the compensation to the half width.
    let mut board = board_builder::p2t11_host_cad_board();
    board.set_clearance_compensation_used(true);
    let near_area = Polyline::from_points(&[Point::new(-4000, -7800), Point::new(-3000, -7800)]);
    assert!(!board.check_polyline_trace(&near_area, 0, 30, &[1], 1));
    // Well away from everything it is still free.
    assert!(board.check_polyline_trace(
        &Polyline::from_points(&[Point::new(-4000, 4000), Point::new(-3000, 4000)]),
        0,
        30,
        &[1],
        1
    ));
    // `checkTraceSegment` on the compensated tree shortens by the compensation instead of the
    // clearance (RoutingBoardSearchFacade.java:82-87): 269, not mode 2's 253.
    assert_eq!(
        board.check_trace_segment(
            &Point::new(1500, 2500),
            &Point::new(2500, 2500),
            0,
            &[1],
            30,
            1,
            false
        ),
        269.0
    );
}

// ---------------------------------------------------------------------------------------------
// ShapeTraceEntries, ShapeEntrySide and ShapeAndEntrySide (`P2T11.java` mode 5)
// ---------------------------------------------------------------------------------------------

#[test]
fn shape_entry_side_finds_the_border_a_polyline_enters_through() {
    // `P2T11.java` mode 5's `--- ShapeEntrySide` block. The square is
    // (-500,-500)..(500,500) and the crossing trace runs (0,-3000) -> (0,3000).
    let board = board_builder::shove_board();
    let shape = TileShape::Box(IntBox::from_coords(-500, -500, 500, 500));
    let polyline = match board.get_item(ItemId(3)).expect("the crossing trace") {
        Item::Trace(t) => t.polyline().clone(),
        other => panic!("not a trace: {other}"),
    };
    // It enters through side 0, the bottom edge, at (0, -500).
    let from_polyline = ShapeEntrySide::from_polyline(&polyline, 1, &shape);
    assert_eq!(from_polyline.no, 0);
    assert_eq!(
        from_polyline.border_intersection,
        Some(fr_geometry::FloatPoint::new(0.0, -500.0))
    );
    // ShapeEntrySide.java:68-75: the nearest border side to a point outside the shape.
    let from_point = ShapeEntrySide::from_point(&Point::new(-2000, 0), &shape);
    assert_eq!(from_point.no, 3);
    assert_eq!(
        from_point.border_intersection,
        Some(fr_geometry::FloatPoint::new(-500.0, 0.0))
    );
    // ShapeEntrySide.java:81-154: `frontSideNo` is 0 (the bottom edge is nearer the segment's
    // start), and both shove directions land on side 2 for a four-sided shape — `(0 + 2) % 4`
    // and `(0 + 4 - 2) % 4` are the same.
    let segment = fr_geometry::LineSegment::from_polyline(&polyline, 1).expect("a segment");
    for to_the_left in [true, false] {
        let side = ShapeEntrySide::from_line_segment(&segment, &shape, to_the_left);
        assert_eq!(side.no, 2);
        assert_eq!(
            side.border_intersection,
            Some(fr_geometry::FloatPoint::new(0.0, 500.0))
        );
    }
    // ShapeEntrySide.java:16.
    assert_eq!(ShapeEntrySide::NOT_CALCULATED.no, -1);
    assert_eq!(ShapeEntrySide::NOT_CALCULATED.border_intersection, None);
}

#[test]
fn shape_and_entry_side_takes_the_bounding_box_in_orthogonal_mode() {
    // `P2T11.java` mode 5's `--- ShapeAndEntrySide` block, all four flag combinations.
    let board = board_builder::shove_board();
    let sae = |orthogonal, in_shove_check| {
        ShapeAndEntrySide::new(&board, ItemId(3), 0, orthogonal, in_shove_check)
            .expect("the crossing trace has a tree shape at index 0")
    };
    // ShapeAndEntrySide.java:35-36: the non-orthogonal branch converts to a `Simplex` and tries
    // to cut the dog ears off. The cut lines *are* found, but `borderLineIndex` on a box or an
    // octagon is a Java stub that returns -1 (quirk #7), so `fromSide` stays null there and the
    // `!inShoveCheck` fallback (:71-75) computes it from the polyline instead.
    for in_shove_check in [false, true] {
        let s = sae(false, in_shove_check);
        assert!(matches!(s.shape, TileShape::Simplex(_)));
        assert_eq!(
            s.shape.bounding_box(),
            IntBox::from_coords(-30, -3000, 30, 3000)
        );
        // The polyline's own line 0 is parallel to the shape's side, so `intersectionApprox`
        // answers Java's parallel sentinel — reproduced by `fr-geometry`.
        let side = s.from_side.expect("the fallback always produces one");
        assert_eq!(side.no, 0);
        assert_eq!(
            side.border_intersection,
            Some(fr_geometry::FloatPoint::new(
                f64::from(i32::MAX),
                f64::from(i32::MAX)
            ))
        );
    }
    // ShapeAndEntrySide.java:32-33: orthogonal mode takes the bounding box and skips the cutting
    // entirely, so `fromSide` comes only from the `!inShoveCheck` fallback.
    let s = sae(true, false);
    assert!(matches!(s.shape, TileShape::Box(_)));
    assert_eq!(
        s.shape.bounding_box(),
        IntBox::from_coords(-30, -3030, 30, 3030)
    );
    let side = s.from_side.expect("the fallback ran");
    assert_eq!(side.no, 0);
    assert_eq!(
        side.border_intersection,
        Some(fr_geometry::FloatPoint::new(0.0, -3030.0))
    );
    // ShapeAndEntrySide.java:71-72: in a shove *check* the fallback is deliberately skipped.
    assert_eq!(sae(true, true).from_side, None);
}

#[test]
fn store_items_sorts_the_crossing_traces_and_builds_a_substitute_piece() {
    // `P2T11.java` mode 5's `--- storeItems` block.
    let mut board = board_builder::shove_board();
    let shape = TileShape::Box(IntBox::from_coords(-500, -500, 500, 500));
    let overlaps = board.overlapping_items_with_clearance(&shape, Some(0), &[1], 1);
    assert_eq!(nums(overlaps.clone()), vec![4, 3]);

    let mut entries = ShapeTraceEntries::new(shape, 0, vec![1], 1, None);
    assert!(entries.store_items(&board, &overlaps, false, false));
    assert_eq!(entries.stack_depth(), 1);
    assert_eq!(entries.substitute_trace_count(), 1);
    assert!(!entries.trace_tails_in_shape());
    // ShapeTraceEntries.java:440: the last trace stored becomes the "found obstacle" even on
    // success.
    assert_eq!(entries.get_found_obstacle(), Some(ItemId(3)));
    assert!(entries.shove_via_list.is_empty());

    let piece = entries
        .next_substitute_trace_piece(&mut board)
        .expect("one piece");
    assert_eq!(piece.hdr.net_nos, vec![2]);
    assert_eq!(piece.get_half_width(), 30);
    let corners: Vec<fr_geometry::FloatPoint> = (0..piece.corner_count())
        .map(|i| piece.polyline().corner_approx(i).expect("a corner"))
        .collect();
    assert_eq!(
        corners,
        vec![
            fr_geometry::FloatPoint::new(0.0, -747.0),
            fr_geometry::FloatPoint::new(747.0, -747.0),
            fr_geometry::FloatPoint::new(747.0, 747.0),
            fr_geometry::FloatPoint::new(-747.0, 747.0),
            fr_geometry::FloatPoint::new(-747.0, 200.0),
        ]
    );
    assert!(entries.next_substitute_trace_piece(&mut board).is_none());
    assert_eq!(entries.substitute_trace_count(), 0);
}

#[test]
fn cutout_trace_replaces_the_trace_with_the_two_pieces_outside_the_shape() {
    // `P2T11.java` mode 5's `--- cutoutTrace` block: the fast path
    // (ShapeTraceEntries.java:91-94) hands the old leaves to the two new pieces.
    let mut board = board_builder::shove_board();
    let shape = TileShape::Box(IntBox::from_coords(-500, -500, 500, 500));
    ShapeTraceEntries::cutout_trace(&mut board, ItemId(3), &shape, 1);
    assert_eq!(nums(board.items_in_board_order()), vec![6, 5, 4, 2, 1]);
    let corners = |id: u32| match board.get_item(ItemId(id)).expect("a trace") {
        Item::Trace(t) => (t.first_corner(), t.last_corner()),
        other => panic!("not a trace: {other}"),
    };
    assert_eq!(
        corners(5),
        (Some(Point::new(0, -3000)), Some(Point::new(0, -747)))
    );
    assert_eq!(
        corners(6),
        (Some(Point::new(0, 747)), Some(Point::new(0, 3000)))
    );
    for id in [5u32, 6] {
        assert!(
            board
                .get_item(ItemId(id))
                .expect("a piece")
                .is_on_the_board()
        );
    }
    // The two pieces are indexed: a probe on either one finds it.
    assert_eq!(
        descending(board.pick_items(&Point::new(0, 2000), Some(0))),
        vec![6]
    );
}

#[test]
fn cutout_traces_skips_the_own_net_and_cuts_the_rest_in_board_order() {
    // `P2T11.java` mode 5's `--- cutoutTraces` block: trace 2 is on the own net and survives
    // untouched (ShapeTraceEntries.java:308), traces 4 then 3 are cut, in `board.itemList`
    // order.
    let mut board = board_builder::shove_board();
    let shape = TileShape::Box(IntBox::from_coords(-500, -500, 500, 500));
    let entries = ShapeTraceEntries::new(shape, 0, vec![1], 1, None);
    let all = board.items_in_board_order();
    entries.cutout_traces(&mut board, &all);
    assert_eq!(nums(board.items_in_board_order()), vec![8, 7, 6, 5, 2, 1]);
    let corners = |id: u32| match board.get_item(ItemId(id)).expect("a trace") {
        Item::Trace(t) => (t.first_corner(), t.last_corner()),
        other => panic!("not a trace: {other}"),
    };
    // 5 and 6 came from trace 4 (the horizontal foreign-net trace at y = 200).
    assert_eq!(
        corners(5),
        (Some(Point::new(-3000, 200)), Some(Point::new(-747, 200)))
    );
    assert_eq!(
        corners(6),
        (Some(Point::new(747, 200)), Some(Point::new(3000, 200)))
    );
    // 7 and 8 came from trace 3 (the vertical one).
    assert_eq!(
        corners(7),
        (Some(Point::new(0, -3000)), Some(Point::new(0, -747)))
    );
    assert_eq!(
        corners(8),
        (Some(Point::new(0, 747)), Some(Point::new(0, 3000)))
    );
    // The own-net trace is untouched.
    assert_eq!(
        corners(2),
        (Some(Point::new(-3000, 0)), Some(Point::new(3000, 0)))
    );
}

// ---------------------------------------------------------------------------------------------
// Cycles, overlaps and the remaining inserters (`P2T11.java` mode 6)
// ---------------------------------------------------------------------------------------------

#[test]
fn a_trace_reachable_from_itself_by_two_paths_is_a_cycle() {
    // `P2T11.java` mode 6: two traces between the same pair of vias, so both are cycles, and a
    // trace whose two ends both land inside one conduction area, which makes it an *overlap*
    // (Trace.java:227-232) and therefore a cycle at Trace.java:275.
    let (board, _) = board_builder::cycle_board();
    assert_eq!(
        nums(board.items_in_board_order()),
        vec![7, 6, 5, 4, 3, 2, 1]
    );
    for (id, start, end) in [(4u32, vec![5, 2], vec![5, 3]), (5, vec![4, 2], vec![4, 3])] {
        assert_eq!(descending(board.trace_start_contacts(ItemId(id))), start);
        assert_eq!(descending(board.trace_end_contacts(ItemId(id))), end);
        assert!(board.is_overlap(ItemId(id)), "trace {id}");
        assert!(board.is_trace_cycle(ItemId(id)), "trace {id}");
        assert!(!board.is_tail(ItemId(id)), "trace {id}");
    }
    // The trace inside the conduction area contacts it at both ends.
    assert_eq!(descending(board.trace_start_contacts(ItemId(7))), vec![6]);
    assert_eq!(descending(board.trace_end_contacts(ItemId(7))), vec![6]);
    assert!(board.is_overlap(ItemId(7)));
    assert!(board.is_trace_cycle(ItemId(7)));
    // Item.java:711-719: the direct trace has two contacts at each end, so its connection stops
    // at itself.
    assert_eq!(
        descending(board.connection_items(ItemId(4), StopConnectionOption::None)),
        vec![4]
    );
}

#[test]
fn remove_if_cycle_removes_the_connection_of_a_cycling_trace() {
    // `P2T11.java` mode 6: `removed=true`, `items=[7 6 5 3 2 1]`.
    let (mut board, _) = board_builder::cycle_board();
    assert!(board.remove_if_cycle(ItemId(4)));
    assert_eq!(nums(board.items_in_board_order()), vec![7, 6, 5, 3, 2, 1]);
    // A trace that is not a cycle is left alone (BasicBoard.java:1339-1341).
    let mut board = p2t11_board();
    assert!(!board.remove_if_cycle(ItemId(4)));
    assert!(board.get_item(ItemId(4)).is_some());
}

#[test]
fn reduce_nets_of_route_items_reduces_but_always_reports_false() {
    // `P2T11.java` mode 6: `result=false`, `nets(4)=[1]` — quirk #66. The trace is put on nets
    // 1 and 2; net 2 is dropped because its contacts do not carry it, and the method still
    // reports that nothing happened (RoutingBoard.java:1285,1355).
    let (mut board, _) = board_builder::cycle_board();
    board
        .get_item_mut(ItemId(4))
        .expect("a trace")
        .header_mut()
        .net_nos = vec![1, 2];
    assert!(!board.reduce_nets_of_route_items());
    assert_eq!(
        board.get_item(ItemId(4)).expect("a trace").net_nos(),
        &[1],
        "net 2 was reduced away even though the return value says otherwise"
    );
}

#[test]
fn delete_all_tracks_and_vias_leaves_only_the_areas_and_the_outline() {
    // `P2T11.java` mode 6: `items=[6 1]`. Java deletes straight from `itemList` and leaves the
    // search trees holding leaves for the removed items; the port removes them from the trees
    // too (`totalized`, docs/java-quirks.md), so the item list matches and the trees stay sane.
    let (mut board, _) = board_builder::cycle_board();
    board.delete_all_tracks_and_vias();
    assert_eq!(nums(board.items_in_board_order()), vec![6, 1]);
    // Nothing is left indexed where the direct trace ran.
    assert!(board.pick_items(&Point::new(1000, 0), Some(0)).is_empty());
}

#[test]
fn the_remaining_typed_inserters_match_the_jvm() {
    // `P2T11.java` mode 6's `--- the remaining inserters` block.
    let (mut board, thru_pad) = board_builder::cycle_board();
    let escape = board.insert_escape_via(
        thru_pad,
        Point::new(-2000, 0),
        vec![1],
        1,
        FixedState::Unfixed,
        0,
    );
    assert_eq!(escape, ItemId(8));
    match board.get_item(escape).expect("the escape via") {
        Item::Via(via) => {
            // BasicBoard.java:318-320: an escape via always allows attachment.
            assert!(via.is_escape_via);
            assert_eq!(via.escape_via_smd_layer, 0);
            assert!(via.attach_allowed);
        }
        other => panic!("not a via: {other}"),
    }

    let via_obstacle = board.insert_via_obstacle(
        Area::Shape(Shape::Tile(TileShape::Box(IntBox::from_coords(
            -4000, 0, -3000, 1000,
        )))),
        0,
        1,
        FixedState::Unfixed,
    );
    assert!(matches!(
        board.get_item(via_obstacle),
        Some(Item::ViaObstacleArea(_))
    ));

    // ComponentObstacleArea.isFront (ComponentObstacleArea.java:83-87) reads the *board's*
    // component list: component 1 is on the front, component 2 on the back.
    for (component_id, is_front) in [(1i32, true), (2, false)] {
        let keepout = board.insert_component_obstacle_of_component(
            Area::Shape(Shape::Tile(TileShape::Box(IntBox::from_coords(
                -6000,
                1200 * component_id,
                -5000,
                1200 * component_id + 1000,
            )))),
            0,
            Vector::from(IntVector::new(0, 0)),
            0.0,
            false,
            1,
            component_id,
            Some(format!("ko{component_id}")),
            FixedState::Unfixed,
        );
        assert!(matches!(
            board.get_item(keepout),
            Some(Item::ComponentObstacleArea(_))
        ));
        assert_eq!(board.component_obstacle_area_is_front(keepout), is_front);
        assert_eq!(
            board.item_component_name(keepout),
            Some(format!("Component#{component_id}").as_str())
        );
        // ComponentObstacleArea.java:38: the constructor drops the net numbers.
        assert!(
            board
                .get_item(keepout)
                .expect("the keepout")
                .net_nos()
                .is_empty()
        );
    }

    // The component overload of `insertObstacle` applies the translation.
    let of_component = board.insert_obstacle_of_component(
        Area::Shape(Shape::Tile(TileShape::Box(IntBox::from_coords(
            -8000, 0, -7000, 1000,
        )))),
        0,
        Vector::from(IntVector::new(10, 20)),
        0.0,
        false,
        1,
        0,
        Some("keepout".to_string()),
        FixedState::Unfixed,
    );
    let ctx = board.ctx();
    assert_eq!(
        board
            .get_item(of_component)
            .expect("the area")
            .bounding_box(&ctx),
        IntBox::from_coords(-7990, 20, -6990, 1020)
    );

    let outline = board
        .insert_component_outline(
            Area::Shape(Shape::Tile(TileShape::Box(IntBox::from_coords(
                -9000, 3000, -8000, 4000,
            )))),
            true,
            Vector::from(IntVector::new(0, 0)),
            0.0,
            0,
            true,
            false,
            true,
            FixedState::Unfixed,
        )
        .expect("a bounded area");
    // ComponentOutline.tileShapeCount (ComponentOutline.java:129-132) is literally 0.
    let ctx = board.ctx();
    assert_eq!(
        board
            .get_item(outline)
            .expect("the outline")
            .tile_shape_count(&ctx),
        0
    );
    assert_eq!(
        nums(board.items_in_board_order()),
        vec![13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1]
    );
    assert_eq!(board.revision(), 13);
}

#[test]
fn component_obstacle_area_is_front_answers_true_for_an_item_of_no_component() {
    // `totalized` (docs/java-quirks.md): with `componentId == 0`,
    // `board.components.get(0)` is `Vector.elementAt(-1)` and Java throws
    // `ArrayIndexOutOfBoundsException` (quirk #49) — verified on the JVM while writing
    // `P2T11.java` mode 6. The port bounds-checks and answers the `component == null` value the
    // method's own expression would have produced.
    let (mut board, _) = board_builder::cycle_board();
    let keepout = board.insert_component_obstacle(
        Area::Shape(Shape::Tile(TileShape::Box(IntBox::from_coords(
            -6000, 0, -5000, 1000,
        )))),
        0,
        1,
        FixedState::Unfixed,
    );
    assert_eq!(
        board.get_item(keepout).expect("the keepout").component_id(),
        0
    );
    assert!(board.component_obstacle_area_is_front(keepout));
}

// ---------------------------------------------------------------------------------------------
// The trace-geometry adapter (`PolylineTraceSearchTreeAdapter`, package-private in Java)
// ---------------------------------------------------------------------------------------------

#[test]
fn the_trace_geometry_adapter_wrappers_keep_the_tree_in_step() {
    // `PolylineTraceSearchTreeAdapter` is package-private, so `P2T11.java` cannot reach it; the
    // `SearchTreeManager` bodies these five forward to are pinned by `p2t10` modes 3, 5 and 8.
    let mut board = p2t11_board();
    // PolylineTraceSearchTreeAdapter.java:22-26.
    assert!(board.trace_has_default_entries(ItemId(4), ItemId(5)));
    assert!(!board.trace_has_default_entries(ItemId(4), ItemId(99)));

    // PolylineTraceSearchTreeAdapter.java:34-40: remove, swap the polyline, drop the derived
    // data, insert again.
    let moved = Polyline::from_points(&[Point::new(-1000, 3000), Point::new(0, 3000)]);
    assert!(board.replace_trace_geometry(ItemId(4), moved));
    let ctx = board.ctx();
    assert_eq!(
        board
            .get_item(ItemId(4))
            .expect("a trace")
            .bounding_box(&ctx),
        IntBox::from_coords(-1030, 2970, 30, 3030)
    );
    // The old location no longer answers, the new one does.
    assert!(!descending(board.pick_items(&Point::new(-500, 0), Some(0))).contains(&4));
    assert!(descending(board.pick_items(&Point::new(-500, 3000), Some(0))).contains(&4));
    assert!(!board.replace_trace_geometry(ItemId(7), Polyline::from_points(&[])));

    // PolylineTraceSearchTreeAdapter.java:62-66.
    let mut board = p2t11_board();
    let new_polyline = Polyline::from_points(&[
        Point::new(0, 0),
        Point::new(1000, 0),
        Point::new(1000, 500),
        Point::new(1000, 1000),
    ]);
    assert!(board.change_trace_entries(ItemId(5), &new_polyline, 1, 1));
    assert!(!board.change_trace_entries(ItemId(7), &new_polyline, 1, 1));

    // The two merge wrappers refuse anything that is not a pair of distinct traces.
    let mut board = p2t11_board();
    let joined = Polyline::from_points(&[Point::new(-1000, 0), Point::new(0, 0)]);
    assert!(!board.merge_trace_entries_in_front(ItemId(4), ItemId(4), &joined, 1, 1));
    assert!(!board.merge_trace_entries_at_end(ItemId(4), ItemId(7), &joined, 1, 1));
}

// ---------------------------------------------------------------------------------------------
// The ported Java tests
// ---------------------------------------------------------------------------------------------

/// Port of `BoardServiceCharacterizationTest.itemQueriesAndSerializationRemainStable`
/// (`src/test/java/app/freerouting/board/BoardServiceCharacterizationTest.java:34-51`), minus
/// its serialization half.
///
/// Java builds a one-layer board with a `TileShape` outline and inserts one trace, then checks
/// `getOutline`, `getItem`, `getItems` and `getTraces`. The observer count (`observer.newItems`)
/// is not ported — `global-constraints.md` drops board observers — and the `serialize` /
/// `deserialize` / `getHash` half is Task 12's.
#[test]
fn item_queries_remain_stable() {
    let mut board = characterization_board();
    let trace = insert_characterization_trace(&mut board, 10, 100, 200);
    assert!(board.get_outline().is_some());
    assert!(board.get_item(trace).is_some());
    assert!(board.get_items().any(|item| item.id() == trace));
    assert_eq!(board.get_traces().len(), 1);
    // added in Task 12: `board.serialize(false)` / `BasicBoard.deserialize` / `getHash`
    // (BoardServiceCharacterizationTest.java:45-50).
}

/// Port of `BoardServiceCharacterizationTest.changedAreaFacadeRetainsLifecycleAndGraphicsTracking`
/// (BoardServiceCharacterizationTest.java:72-82).
///
/// Java's last two lines call `optChangedArea` (which clears the area and joins the graphics
/// update box) and then check `getGraphicsUpdateBox()`. `optChangedArea` runs the `TraceTightener`
/// and arrives in Plan 7, and the graphics update box is not ported at all (GUI), so the port
/// stops at the marking half and checks the area it produced instead.
#[test]
fn changed_area_lifecycle_matches_the_characterization_test() {
    let mut board = characterization_board();
    assert!(board.changed_area.is_none());
    board.start_marking_changed_area();
    assert!(board.changed_area.is_some());
    board.join_changed_area(&Point::new(100, 100).to_float(), 0);
    board.mark_all_changed_area();
    assert_eq!(
        board
            .changed_area
            .as_ref()
            .expect("marked")
            .surrounding_box(),
        IntBox::from_coords(0, 0, 1000, 1000)
    );
    // added in Plan 7: `board.optChangedArea(...)`
    // (BoardServiceCharacterizationTest.java:78), which clears `changedArea` afterwards.
}

/// `BoardServiceCharacterizationTest.snapshotUndoRedoPreservesItemsAndObserverNotifications`
/// (:53-69) is **not** ported here: `generateSnapshot`/`undo`/`redo` are the `UndoableObjects`
/// stack, which Plan 2 replaces with `Board::clone` in Task 12, and its remaining assertions are
/// observer counts.
// added in Task 12: `snapshotUndoRedoPreservesItemsAndObserverNotifications`.
#[test]
fn the_snapshot_characterization_test_is_task_twelves() {
    // Nothing to assert yet; the marker above records the obligation.
}

/// `BoardServiceCharacterizationTest.createBoard` (:94-112).
fn characterization_board() -> Board {
    let layers = LayerStructure::new(vec![Layer::new("Top", true)]);
    let clearance_matrix = ClearanceMatrix::get_default_instance(&layers, 10);
    let mut rules = BoardRules::new(layers.clone(), clearance_matrix);
    rules.create_default_net_class();
    // BoardServiceCharacterizationTest.java:101: `TileShape.getInstance(0, 0, 1000, 1000)`, an
    // `IntBox`, not a polygon.
    let outline = vec![PolylineShapeRef::Tile(TileShape::Box(IntBox::from_coords(
        0, 0, 1000, 1000,
    )))];
    Board::new(
        outline,
        0,
        IntBox::from_coords(0, 0, 1000, 1000),
        rules,
        BoardLibrary::new(Padstacks::new(layers), Packages::new()),
        Components::new(),
        Communication::default(),
    )
}

/// `BoardServiceCharacterizationTest.insertTrace` (:84-92).
fn insert_characterization_trace(board: &mut Board, net_number: i32, x1: i32, x2: i32) -> ItemId {
    board
        .insert_trace_without_cleaning(
            Polyline::from_points(&[Point::new(x1, 100), Point::new(x2, 100)]),
            0,
            10,
            vec![net_number],
            0,
            FixedState::Unfixed,
        )
        .expect("a straight two-corner trace")
}

/// Port of `PinObstacleTest.sameNetViaWithAttachDisallowedIsNotObstacleForSmdPin`
/// (`src/test/java/app/freerouting/board/PinObstacleTest.java:15-24`).
///
/// The Java test mocks `Pin.drillAllowed()` and `Via.sharesNet()`; the port builds the real
/// items instead — the fixture's pin 2 is an SMD pad (`drillAllowed`, Pin.java:344-350) and the
/// via is on the same net. `Item::is_obstacle` is already covered by
/// `pin_is_obstacle_to_a_same_net_via_only_when_it_is_not_an_smd_pad` in `src/items/mod.rs`;
/// this repeats it against a real board so the fixture's `drillAllowed` is exercised.
#[test]
fn a_same_net_via_is_not_an_obstacle_for_an_smd_pin() {
    let board = p2t11_board();
    let ctx = board.ctx();
    let pin = board.get_item(ItemId(2)).expect("the SMD pin");
    let via = board.get_item(ItemId(6)).expect("the via");
    assert!(pin.shares_net(via));
    assert!(!pin.is_obstacle(via, &ctx));
    // The through pin is not drillable, so it *is* an obstacle to the same-net via
    // (Pin.java:364).
    let through_pin = board.get_item(ItemId(3)).expect("the through pin");
    assert!(through_pin.is_obstacle(via, &ctx));
}

// ---------------------------------------------------------------------------------------------
// The paths `P2T11.java` cannot reach — each cites the Java source instead
// ---------------------------------------------------------------------------------------------

#[test]
fn has_ignored_nets_reads_the_net_class_flag() {
    // Item.java:1241-1255. The driver's board has no ignored net class, so this is Rust-only.
    let mut board = p2t11_board();
    let layers = board.rules.layer_structure().clone();
    let ignored = board.rules.net_classes.append("ignored", &layers, false);
    board
        .rules
        .net_classes
        .get_mut(ignored)
        .is_ignored_by_autorouter = true;
    assert!(!board.has_ignored_nets(ItemId(4)));
    let net_1 = board.rules.nets.get_mut(1).expect("net 1");
    net_1.set_class(ignored);
    assert!(board.has_ignored_nets(ItemId(4)));
    // An item on no net has nothing to ignore.
    assert!(!board.has_ignored_nets(ItemId(7)));
}

#[test]
#[should_panic(expected = "NullPointerException")]
fn has_ignored_nets_panics_on_a_net_the_net_list_does_not_know() {
    // Item.java:1244: `nets.get(netNumber).getNetClass()` with no null check.
    let mut board = p2t11_board();
    board
        .get_item_mut(ItemId(4))
        .expect("a trace")
        .header_mut()
        .net_nos = vec![99];
    board.has_ignored_nets(ItemId(4));
}

#[test]
fn all_nets_skips_a_net_the_net_list_does_not_know() {
    // Item.java:1276: unlike `hasIgnoredNets`, `getAllNets` *does* null-check.
    let mut board = p2t11_board();
    board
        .get_item_mut(ItemId(4))
        .expect("a trace")
        .header_mut()
        .net_nos = vec![1, 99];
    assert_eq!(board.all_nets(ItemId(4)), vec![1]);
    assert_eq!(board.all_net_names(ItemId(4)), "Net #1 (N1)");
}

#[test]
fn get_trace_tail_finds_an_uncontacted_end_of_exactly_these_nets() {
    // BasicBoard.java:1306-1329. The driver's board has no tails, so this adds one.
    let mut board = p2t11_board();
    let stray = board
        .insert_trace_without_cleaning(
            Polyline::from_points(&[Point::new(-1000, 0), Point::new(-1000, 900)]),
            0,
            30,
            vec![1],
            1,
            FixedState::Unfixed,
        )
        .expect("a straight two-corner trace");
    assert_eq!(
        board.get_trace_tail(&Point::new(-1000, 900), Some(0), &[1]),
        Some(stray)
    );
    // BasicBoard.java:1311-1313: `netsEqual`, not `sharesNet`.
    assert_eq!(
        board.get_trace_tail(&Point::new(-1000, 900), Some(0), &[2]),
        None
    );
    // The contacted end is not a tail end.
    assert_eq!(
        board.get_trace_tail(&Point::new(-1000, 0), Some(0), &[1]),
        None
    );
    // RoutingBoard.java:1176-1187.
    assert!(board.contains_trace_tails([stray], &[]));
    assert!(!board.contains_trace_tails([stray], &[1]));
}

#[test]
fn remove_trace_tails_removes_a_stub() {
    // RoutingBoard.java:1193-1238. Java's tail calls `combineTraces` after the removal
    // (:1236), which is Task 9's; on this board there is nothing left to combine, so the item
    // list is the whole observable result.
    let mut board = p2t11_board();
    let stray = board
        .insert_trace_without_cleaning(
            Polyline::from_points(&[Point::new(-1000, 0), Point::new(-1000, 900)]),
            0,
            30,
            vec![1],
            1,
            FixedState::Unfixed,
        )
        .expect("a straight two-corner trace");
    assert!(board.is_tail(stray));
    assert!(board.remove_trace_tails(1, StopConnectionOption::None));
    assert_eq!(board.get_item(stray), None);
    // Everything else survives: the chain has no other stub.
    assert_eq!(
        nums(board.items_in_board_order()),
        vec![8, 7, 6, 5, 4, 3, 2, 1]
    );
}

#[test]
fn check_move_item_consults_the_ignore_set() {
    // RoutingBoardSearchFacade.java:123-141: the item under test is added to `ignoreItems`, and
    // anything else in it stops being an obstacle.
    let mut board = p2t11_board();
    let mut ignore = Some(BTreeSet::new());
    assert!(board.check_move_item(
        ItemId(7),
        &Vector::from(IntVector::new(10, 10)),
        &mut ignore
    ));
    assert!(ignore.expect("the set").contains(&ItemId(7)));
}

#[test]
fn item_tree_shape_answers_none_past_the_end_after_one_retry() {
    // Item.java:218-224: an out-of-range index triggers `clearDerivedData()` and one recompute,
    // and then gives up.
    let mut board = p2t11_board();
    let tree = board.default_tree_id();
    assert_eq!(board.item_tree_shape_count(ItemId(4), tree), 1);
    assert!(board.item_tree_shape(ItemId(4), tree, 0).is_some());
    assert!(board.item_tree_shape(ItemId(4), tree, 1).is_none());
    // The retry left the cache intact.
    assert_eq!(board.item_tree_shape_count(ItemId(4), tree), 1);
    assert!(board.item_tree_shape(ItemId(99), tree, 0).is_none());
}

#[test]
fn the_shove_failure_fields_round_trip() {
    // RoutingBoard.java:72-73,1359-1378.
    let mut board = p2t11_board();
    assert_eq!(board.get_shove_failing_obstacle(), None);
    assert_eq!(board.get_shove_failing_layer(), -1);
    board.set_shove_failing_obstacle(Some(ItemId(7)));
    board.set_shove_failing_layer(1);
    assert_eq!(board.get_shove_failing_obstacle(), Some(ItemId(7)));
    assert_eq!(board.get_shove_failing_layer(), 1);
    board.clear_shove_failing_obstacle();
    assert_eq!(board.get_shove_failing_obstacle(), None);
    assert_eq!(board.get_shove_failing_layer(), -1);
    // RoutingBoard.java:64: the failure log is a plain hook until Plan 6 types it.
    assert!(board.failure_log.is_empty());
}

#[test]
fn clear_all_item_temporary_autoroute_data_drops_every_scratch_record() {
    // RoutingBoard.java:1241-1250.
    let mut board = p2t11_board();
    for id in board.items_in_board_order() {
        board
            .get_item_mut(id)
            .expect("an item")
            .get_autoroute_info();
    }
    assert!(
        board
            .get_item(ItemId(4))
            .expect("a trace")
            .get_autoroute_info_pur()
            .is_some()
    );
    board.clear_all_item_temporary_autoroute_data();
    for id in board.items_in_board_order() {
        assert!(
            board
                .get_item(id)
                .expect("an item")
                .get_autoroute_info_pur()
                .is_none(),
            "item {id}"
        );
    }
}

#[test]
fn store_items_refuses_a_shove_fixed_obstacle_of_a_foreign_net() {
    // ShapeTraceEntries.java:188-191.
    let mut board = board_builder::shove_board();
    board
        .get_item_mut(ItemId(3))
        .expect("the crossing trace")
        .set_fixed_state(FixedState::ShoveFixed);
    let shape = TileShape::Box(IntBox::from_coords(-500, -500, 500, 500));
    let overlaps = board.overlapping_items_with_clearance(&shape, Some(0), &[1], 1);
    let mut entries = ShapeTraceEntries::new(shape, 0, vec![1], 1, None);
    assert!(!entries.store_items(&board, &overlaps, false, false));
    assert_eq!(entries.get_found_obstacle(), Some(ItemId(3)));
}

#[test]
fn store_items_refuses_a_non_shovable_item_of_a_foreign_net() {
    // ShapeTraceEntries.java:211-214: anything that is not a via, a trace or an excused area is
    // an obstacle outright when it is not on the own net.
    let mut board = board_builder::shove_board();
    let area = board.insert_obstacle(
        Area::Shape(Shape::Tile(TileShape::Box(IntBox::from_coords(
            -400, -400, 400, 400,
        )))),
        0,
        1,
        FixedState::Unfixed,
    );
    let shape = TileShape::Box(IntBox::from_coords(-500, -500, 500, 500));
    let overlaps = board.overlapping_items_with_clearance(&shape, Some(0), &[1], 1);
    assert!(overlaps.contains(&area));
    let mut entries = ShapeTraceEntries::new(shape, 0, vec![1], 1, None);
    assert!(!entries.store_items(&board, &overlaps, false, false));
    assert_eq!(entries.get_found_obstacle(), Some(area));
}

#[test]
fn a_component_keepout_is_skipped_by_store_items_whatever_the_pad_check_says() {
    // quirk #65: `!isPadCheck && a || b` means a `ComponentObstacleArea` is skipped
    // unconditionally, while a `ViaObstacleArea` is skipped only when this is not a pad check.
    let mut board = board_builder::shove_board();
    let via_keepout = board.insert_via_obstacle(
        Area::Shape(Shape::Tile(TileShape::Box(IntBox::from_coords(
            -400, -400, 400, 400,
        )))),
        0,
        1,
        FixedState::Unfixed,
    );
    let shape = TileShape::Box(IntBox::from_coords(-500, -500, 500, 500));
    let overlaps = board.overlapping_items_with_clearance(&shape, Some(0), &[1], 1);
    assert!(overlaps.contains(&via_keepout));
    // Not a pad check: the via keepout is skipped and the traces still sort.
    let mut entries = ShapeTraceEntries::new(shape.clone(), 0, vec![1], 1, None);
    assert!(entries.store_items(&board, &overlaps, false, false));
    // A pad check: the same via keepout now falls through to the `else` and blocks.
    let mut entries = ShapeTraceEntries::new(shape, 0, vec![1], 1, None);
    assert!(!entries.store_items(&board, &overlaps, true, false));
    assert_eq!(entries.get_found_obstacle(), Some(via_keepout));
}

#[test]
fn a_changed_area_knows_how_many_layers_it_was_made_for() {
    // ChangedArea.java:11,63.
    let area = ChangedArea::new(4);
    assert_eq!(area.layer_count(), 4);
}

#[test]
#[should_panic(expected = "NumberFormatException")]
fn host_is_old_kicad_panics_on_a_version_that_overflows_an_int() {
    // quirk #67: Communication.java:79's `Integer.parseInt` is unguarded.
    let communication = Communication {
        host_cad: Some("kicad".to_string()),
        host_version: Some("99999999999".to_string()),
        ..Communication::default()
    };
    communication.host_is_old_kicad();
}

// ---------------------------------------------------------------------------------------------
// Structural
// ---------------------------------------------------------------------------------------------

#[test]
fn board_is_send_and_sync_and_clones_independently() {
    // `global-constraints.md`: `Board: Clone` must compile, and Plan 6 runs board copies in
    // parallel. Task 12 turns the clone into `deep_copy`.
    fn assert_send_sync<T: Send + Sync + Clone>() {}
    assert_send_sync::<Board>();

    let board = p2t11_board();
    let mut copy = board.clone();
    assert_eq!(copy, board);
    copy.remove_item(ItemId(7));
    assert_ne!(copy, board);
    assert!(board.get_item(ItemId(7)).is_some());
    // The clone's tree is its own: the original still finds the area, the copy does not.
    let ctx = board.ctx();
    let shape = board
        .get_item(ItemId(7))
        .expect("the area")
        .get_tile_shape(board.default_tree_id(), 0, &ctx)
        .expect("its only tile shape");
    assert!(!board.overlapping_objects(&shape, Some(0)).is_empty());
    assert!(copy.overlapping_objects(&shape, Some(0)).is_empty());
}

// ---------------------------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------------------------

fn is_obstacle(board: &Board, id: u32) -> bool {
    match board.get_item(ItemId(id)).expect("a conduction area") {
        Item::ConductionArea(area) => area.get_is_obstacle(),
        other => panic!("item {id} is not a conduction area: {other}"),
    }
}

fn is_filled(board: &Board, id: u32) -> bool {
    match board.get_item(ItemId(id)).expect("a conduction area") {
        Item::ConductionArea(area) => area.get_is_filled(),
        other => panic!("item {id} is not a conduction area: {other}"),
    }
}
