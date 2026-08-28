//! Rust twin of `scripts/differential/java/P2T11.java` (Plan 2 Task 11).
//!
//! Builds the same two-layer board — a real outline, a two-pin component, two traces, a via, an
//! obstacle area and a conduction area — through `fr-board`'s `Board`, then prints the same lines
//! the Java driver prints.
//!
//! Modes: `0` insert/remove + the item-list and search queries, `1` connectivity, `2` the check
//! queries including `checkTraceSegment`, `3` the changed area and the board-level bookkeeping.

use std::collections::BTreeSet;

use fr_board::prelude::*;
use fr_geometry::{
    Area, IntBox, IntVector, Point, PolygonShape, Polyline, PolylineShapeRef, Shape, TileShape,
    Vector,
};
use fr_board::ItemIdGenerator;

fn main() {
    let mode: u32 = std::env::args()
        .nth(1)
        .map_or(0, |a| a.parse().expect("mode"));
    let mut board = build(mode == 4);
    match mode {
        0 => dump_insert_remove(&mut board),
        1 => dump_connectivity(&mut board),
        2 => dump_checks(&mut board),
        3 => dump_changed_area(&mut board),
        4 => dump_compensated(&mut board),
        5 => dump_shape_trace_entries(),
        6 => dump_cycles_and_inserters(),
        _ => panic!("mode {mode}"),
    }
}

// ---------------------------------------------------------------------------------------------
// The board
// ---------------------------------------------------------------------------------------------

fn layers() -> LayerStructure {
    LayerStructure::new(vec![Layer::new("front", true), Layer::new("back", true)])
}

fn build(host_cad: bool) -> Board {
    let ls = layers();
    let mut cm = ClearanceMatrix::get_default_instance(&ls, 200);
    assert!(cm.append_class("wide"));
    cm.set_value_on_all_layers(2, 1, 600);
    cm.set_value_on_all_layers(2, 2, 800);
    let mut rules = BoardRules::new(layers(), cm);
    rules.create_default_net_class();
    let default_class = rules.get_default_net_class();
    if host_cad {
        rules.trace_angle_restriction = AngleRestriction::NinetyDegree;
    }

    let mut padstacks = Padstacks::new(layers());
    let smd_pad = padstacks.add(
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
    let thru_shape = Shape::Tile(TileShape::Box(IntBox::from_coords(-70, -70, 70, 70)));
    let thru_pad = padstacks.add(
        "thru",
        vec![Some(thru_shape.clone()), Some(thru_shape)],
        true,
        false,
    );
    let mut packages = Packages::new();
    let pkg = packages.add(
        "pkg",
        vec![
            PackagePin::new("P1", smd_pad, IntVector::new(-1000, 0).into(), 0.0),
            PackagePin::new("P2", thru_pad, IntVector::new(1000, 1000).into(), 0.0),
        ],
        None,
        None,
        None,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        true,
    );
    let library = BoardLibrary::new(padstacks, packages);
    let mut components = Components::new();
    components.add_with_generated_name(Some(Point::new(0, 0)), 0.0, true, pkg);

    let outline = vec![PolylineShapeRef::Polygon(PolygonShape::from_points(&[
        Point::new(-5000, -5000),
        Point::new(5000, -5000),
        Point::new(5000, 5000),
        Point::new(-5000, 5000),
    ]))];
    let mut board = Board::new(
        outline,
        1,
        IntBox::from_coords(-10_000, -10_000, 10_000, 10_000),
        rules,
        library,
        components,
        // Mode 4 gives the board a host CAD name and a resolution of 10, which lowers
        // `ShapeSearchTree.calculateTreeShapes(ObstacleArea)`'s section width from 50000 to
        // `min(500 * 10, 50000) = 5000` (ShapeSearchTree.java:916-920).
        if host_cad {
            Communication::new(
                Unit::Mil,
                10,
                ItemIdGenerator::new(),
                Some("KiCad".to_string()),
                Some("7.0".to_string()),
            )
        } else {
            Communication::default()
        },
    );
    // `Nets.add` reads `netList.getBoard().rules` (Net.java:50), so the Java driver creates the
    // nets after the board; the port has no back-pointer, so the order is free — kept the same.
    board.rules.nets.add("N1", 1, false, default_class);
    board.rules.nets.add("N2", 1, false, default_class);

    board.insert_pin(1, 0, vec![1], 1, FixedState::Unfixed);
    board.insert_pin(1, 1, vec![1], 1, FixedState::Unfixed);
    board.insert_trace_without_cleaning(
        Polyline::from_points(&[Point::new(-1000, 0), Point::new(0, 0)]),
        0,
        30,
        vec![1],
        1,
        FixedState::Unfixed,
    );
    board.insert_trace_without_cleaning(
        Polyline::from_points(&[
            Point::new(0, 0),
            Point::new(1000, 0),
            Point::new(1000, 1000),
        ]),
        1,
        30,
        vec![1],
        1,
        FixedState::Unfixed,
    );
    board.insert_via(
        thru_pad,
        Point::new(0, 0),
        vec![1],
        1,
        FixedState::Unfixed,
        true,
    );
    board.insert_obstacle(
        Area::Shape(Shape::Tile(TileShape::Box(IntBox::from_coords(
            2000, 2000, 3000, 3000,
        )))),
        0,
        1,
        FixedState::Unfixed,
    );
    board.insert_conduction_area(
        Area::Shape(Shape::Tile(TileShape::Box(IntBox::from_coords(
            -3000, -3000, -2000, -2000,
        )))),
        0,
        vec![2],
        1,
        true,
        FixedState::Unfixed,
    );
    if host_cad {
        board.insert_obstacle(
            Area::Shape(Shape::Tile(TileShape::Box(IntBox::from_coords(
                -9000, -9000, 9000, -8000,
            )))),
            0,
            1,
            FixedState::Unfixed,
        );
    }
    board
}

// ---------------------------------------------------------------------------------------------
// Mode 4
// ---------------------------------------------------------------------------------------------

fn dump_compensated(board: &mut Board) {
    println!("mode=4");
    println!(
        "hostCadExists={} resolution(MIL)={:?} hostIsOldKicad={} hostCadIsEagle={}",
        board.communication.host_cad_exists(),
        board.communication.get_resolution(Unit::Mil),
        board.communication.host_is_old_kicad(),
        board.communication.host_cad_is_eagle()
    );
    println!(
        "defaultTree={} compensationUsed={}",
        board.trees.get_default_tree(),
        board.trees.get_default_tree().is_clearance_compensation_used()
    );
    dump_tree_shapes(board, 9, "wideArea", true);

    println!("--- setClearanceCompensationUsed(true)");
    board.set_clearance_compensation_used(true);
    println!(
        "defaultTree={} compensationUsed={}",
        board.trees.get_default_tree(),
        board.trees.get_default_tree().is_clearance_compensation_used()
    );
    let compensation = {
        let rules = &board.rules;
        board
            .trees
            .get_default_tree()
            .clearance_compensation_value(1, 0, rules)
    };
    println!("compensation(1, 0)={compensation}");
    dump_tree_shapes(board, 4, "trace 4", true);
    dump_tree_shapes(board, 9, "wideArea", false);

    println!(
        "checkPolylineTrace(free)={}",
        board.check_polyline_trace(
            &Polyline::from_points(&[Point::new(-4000, 4000), Point::new(-3000, 4000)]),
            0,
            30,
            &[1],
            1
        )
    );
    println!(
        "checkPolylineTrace(obstacle)={}",
        board.check_polyline_trace(
            &Polyline::from_points(&[Point::new(1500, 2500), Point::new(2500, 2500)]),
            0,
            30,
            &[1],
            1
        )
    );
    println!(
        "checkPolylineTrace(nearArea)={}",
        board.check_polyline_trace(
            &Polyline::from_points(&[Point::new(-4000, -7800), Point::new(-3000, -7800)]),
            0,
            30,
            &[1],
            1
        )
    );
    println!(
        "checkTraceShape(free)={}",
        board.check_trace_shape(
            &TileShape::Box(IntBox::from_coords(-4500, 4000, -4000, 4500)),
            0,
            &[1],
            1,
            None
        )
    );
    seg(board, "free", (-4000, 4000), (-3000, 4000), 0, &[1], 30, 1, false);
    seg(board, "blocked", (1500, 2500), (2500, 2500), 0, &[1], 30, 1, false);
}

/// `item.treeShapeCount(def)` plus, when `with_shapes`, each shape's bounding box.
fn dump_tree_shapes(board: &mut Board, id: u32, label: &str, with_shapes: bool) {
    let tree = board.default_tree_id();
    let count = board.item_tree_shape_count(ItemId(id), tree);
    println!("{label} treeShapes={count}");
    if !with_shapes {
        return;
    }
    for i in 0..count {
        let shape = board
            .item_tree_shape(ItemId(id), tree, i)
            .expect("a tree shape");
        println!("  [{i}]={}", boxs(&shape.bounding_box()));
    }
}

// ---------------------------------------------------------------------------------------------
// Mode 0
// ---------------------------------------------------------------------------------------------

fn dump_insert_remove(board: &mut Board) {
    println!("mode=0 revision={}", board.revision());
    println!("layerCount={}", board.get_layer_count());
    println!(
        "minTraceHalfWidth={} maxTraceHalfWidth={}",
        board.get_min_trace_half_width(),
        board.get_max_trace_half_width()
    );
    println!("boundingBox={}", boxs(&board.get_bounding_box()));
    let ctx = board.ctx();
    for item in board.get_items() {
        println!(
            "item id={} class={} nets={} cl={} layers={}..{} onBoard={} tiles={} bbox={}",
            item.id(),
            class_name(item),
            net_array(item.net_nos()),
            item.clearance_class(),
            item.first_layer(&ctx),
            item.last_layer(&ctx),
            item.is_on_the_board(),
            item.tile_shape_count(&ctx),
            boxs(&item.bounding_box(&ctx))
        );
    }
    println!("outline={}", board.get_outline().expect("an outline"));
    println!("pins={}", ids(board.get_pins()));
    println!("smdPins={}", ids(board.get_smd_pins()));
    println!("vias={}", ids(board.get_vias()));
    println!("traces={}", ids(board.get_traces()));
    println!("conductionAreas={}", ids(board.get_conduction_areas()));
    println!("connectableItems(1)={}", ids(board.get_connectable_items(1)));
    println!("connectableItemCount(1)={}", board.connectable_item_count(1));
    println!("connectableItems(2)={}", ids(board.get_connectable_items(2)));
    println!("componentItems(1)={}", ids(board.get_component_items(1)));
    println!("componentPins(1)={}", ids(board.get_component_pins(1)));
    println!("pin(1,0)={}", board.get_pin(1, 0).expect("pin 0"));
    println!("pin(1,1)={}", board.get_pin(1, 1).expect("pin 1"));
    println!("pin(1,7)={}", opt_id(board.get_pin(1, 7)));
    println!("cumulativeTraceLength={:.6}", board.cumulative_trace_length());
    println!("non45={}", board.get_non_45_degree_trace_count());
    println!("clearance(1,1,0)={}", board.clearance_value(1, 1, 0));
    println!("clearance(2,1,0)={}", board.clearance_value(2, 1, 0));
    println!("clearance(2,2,1)={}", board.clearance_value(2, 2, 1));
    println!("contains(0,0)={}", board.contains(&Point::new(0, 0)));
    println!("contains(99999,0)={}", board.contains(&Point::new(99999, 0)));
    println!(
        "componentName(2)={}",
        board.item_component_name(ItemId(2)).unwrap_or("null")
    );
    println!(
        "componentName(4)={}",
        board.item_component_name(ItemId(4)).unwrap_or("null")
    );
    println!("allNetNames(4)={}", board.all_net_names(ItemId(4)));
    println!("allNetNames(7)={}", board.all_net_names(ItemId(7)));
    println!(
        "boundingBoxOf(4,5)={}",
        boxs(&board.get_bounding_box_of_items([ItemId(4), ItemId(5)]))
    );

    let probe = TileShape::Box(IntBox::from_coords(-100, -100, 100, 100));
    println!(
        "overlappingObjects(probe, 0)={}",
        objects(&board.overlapping_objects(&probe, Some(0)))
    );
    println!(
        "overlappingObjects(probe, 1)={}",
        objects(&board.overlapping_objects(&probe, Some(1)))
    );
    println!(
        "overlappingObjects(probe, -1)={}",
        objects(&board.overlapping_objects(&probe, None))
    );
    println!(
        "overlappingItemsWithClearance(probe, 0, [], 1)={}",
        ids(board.overlapping_items_with_clearance(&probe, Some(0), &[], 1))
    );
    println!(
        "overlappingItemsWithClearance(probe, 0, [1], 1)={}",
        ids(board.overlapping_items_with_clearance(&probe, Some(0), &[1], 1))
    );
    let area = Area::Shape(Shape::Tile(TileShape::Box(IntBox::from_coords(
        -1100, -100, 100, 100,
    ))));
    println!(
        "overlappingItems(box, 0)={}",
        ids(descending(board.overlapping_items(&area, Some(0))))
    );
    println!(
        "pickItems((0,0), 0)={}",
        ids(descending(board.pick_items(&Point::new(0, 0), Some(0))))
    );

    println!("--- removeItem(outline) refuses (SYSTEM_FIXED)");
    let outline = board.get_outline().expect("an outline");
    println!(
        "isDeletionForbidden(outline)={}",
        board
            .get_item(outline)
            .expect("an outline")
            .is_deletion_forbidden(&board.rules)
    );
    board.remove_item(outline);
    println!(
        "revision={} outline={}",
        board.revision(),
        board.get_outline().expect("still an outline")
    );

    println!("--- removeItem(6) removes the via");
    board.remove_item(ItemId(6));
    println!("revision={}", board.revision());
    println!(
        "getItem(6)={}",
        if board.get_item(ItemId(6)).is_none() {
            "null"
        } else {
            "?"
        }
    );
    println!("items={}", ids(board.items_in_board_order()));
    println!(
        "overlappingObjects(probe, 0)={}",
        objects(&board.overlapping_objects(&probe, Some(0)))
    );
    println!(
        "overlappingObjects(probe, 1)={}",
        objects(&board.overlapping_objects(&probe, Some(1)))
    );

    println!("--- removeItems([4, 7])");
    println!(
        "removeItems={}",
        board.remove_items([ItemId(4), ItemId(7)])
    );
    println!(
        "items={} revision={}",
        ids(board.items_in_board_order()),
        board.revision()
    );

    println!("--- incrementRevision");
    board.increment_revision();
    println!("revision={}", board.revision());
}

// ---------------------------------------------------------------------------------------------
// Mode 1
// ---------------------------------------------------------------------------------------------

fn dump_connectivity(board: &mut Board) {
    println!("mode=1");
    for id in board.items_in_board_order() {
        println!(
            "id={} normalContacts={} allContacts={} connected={} tail={} overlap={} ratsnest={}",
            id,
            ids(descending(board.normal_contacts(id))),
            ids(descending(board.all_contacts(id))),
            board.is_connected(id),
            board.is_tail(id),
            board.is_overlap(id),
            points(&board.ratsnest_corners(id))
        );
    }
    for layer in 0..board.get_layer_count() {
        for id in board.items_in_board_order() {
            println!(
                "id={} layer={} contactsOnLayer={} connectedOnLayer={}",
                id,
                layer,
                ids(descending(board.all_contacts_on_layer(id, layer))),
                board.is_connected_on_layer(id, layer)
            );
        }
    }
    let set = |id: u32, net: i32| ids(descending(board.connected_set(ItemId(id), net, false)));
    println!("connectedSet(2, 1)={}", set(2, 1));
    println!("connectedSet(2, -1)={}", set(2, -1));
    println!("connectedSet(3, 1)={}", set(3, 1));
    println!("connectedSet(8, 2)={}", set(8, 2));
    println!("connectedSet(2, 2)={}", set(2, 2));
    println!(
        "connectedSetStopAtPlane(2, 1)={}",
        ids(descending(board.connected_set(ItemId(2), 1, true)))
    );
    println!(
        "unconnectedSet(2, 1)={}",
        ids(descending(board.unconnected_set(ItemId(2), 1)))
    );
    println!(
        "unconnectedSet(8, 2)={}",
        ids(descending(board.unconnected_set(ItemId(8), 2)))
    );
    println!(
        "unconnectedSet(2, 0)={}",
        ids(descending(board.unconnected_set(ItemId(2), 0)))
    );
    println!(
        "connectionItems(4)={}",
        ids(descending(
            board.connection_items(ItemId(4), StopConnectionOption::None)
        ))
    );
    println!(
        "connectionItems(5)={}",
        ids(descending(
            board.connection_items(ItemId(5), StopConnectionOption::None)
        ))
    );
    println!(
        "connectionItemsVia(4)={}",
        ids(descending(
            board.connection_items(ItemId(4), StopConnectionOption::Via)
        ))
    );
    println!(
        "connectionItemsFanout(4)={}",
        ids(descending(
            board.connection_items(ItemId(4), StopConnectionOption::FanoutVia)
        ))
    );
    let ctx = board.ctx();
    for a in [2u32, 4, 5, 6] {
        for b in [2u32, 4, 5, 6] {
            let (item_a, item_b) = (
                board.get_item(ItemId(a)).expect("a"),
                board.get_item(ItemId(b)).expect("b"),
            );
            println!(
                "normalContactPoint({a},{b})={} firstCommonLayer={} lastCommonLayer={}",
                point(board.normal_contact_point(ItemId(a), ItemId(b)).as_ref()),
                layer_or_minus_one(item_a.first_common_layer(item_b, &ctx)),
                layer_or_minus_one(item_a.last_common_layer(item_b, &ctx))
            );
        }
    }
    let sets: Vec<String> = board
        .get_connected_sets(1)
        .into_iter()
        .map(|set| ids(descending(set)))
        .collect();
    println!("connectedSets(1)=[{}]", sets.join(", "));
    println!("isFanoutVia(6)={}", board.is_fanout_via(ItemId(6), None));
    println!("hasIgnoredNets(4)={}", board.has_ignored_nets(ItemId(4)));
    println!("isCycle(4)={}", board.is_trace_cycle(ItemId(4)));
    println!(
        "startContacts(4)={}",
        ids(descending(board.trace_start_contacts(ItemId(4))))
    );
    println!(
        "endContacts(4)={}",
        ids(descending(board.trace_end_contacts(ItemId(4))))
    );
    println!(
        "startContacts(5)={}",
        ids(descending(board.trace_start_contacts(ItemId(5))))
    );
    println!(
        "endContacts(5)={}",
        ids(descending(board.trace_end_contacts(ItemId(5))))
    );
    println!(
        "touchingPins(4)={}",
        ids(descending(board.touching_pins_at_end_corners(ItemId(4))))
    );
    println!("validate(4)={}", board.validate_item(ItemId(4)));
    println!("validate(6)={}", board.validate_item(ItemId(6)));
    println!(
        "swappablePins(2)={}",
        ids(descending(board.swappable_pins(ItemId(2))))
    );
}

// ---------------------------------------------------------------------------------------------
// Mode 2
// ---------------------------------------------------------------------------------------------

fn dump_checks(board: &mut Board) {
    println!("mode=2");
    seg(board, "free", (-4000, 4000), (-3000, 4000), 0, &[1], 30, 1, false);
    seg(board, "blocked", (1500, 2500), (2500, 2500), 0, &[1], 30, 1, false);
    seg(board, "blocked_own_net", (-2000, 0), (-500, 0), 0, &[1], 30, 1, false);
    seg(board, "blocked_foreign", (-2000, 0), (-500, 0), 0, &[9], 30, 1, false);
    seg(board, "degenerate", (0, 0), (0, 0), 0, &[1], 30, 1, false);
    seg(board, "shovable_only", (-2000, 0), (-500, 0), 0, &[9], 30, 1, true);
    seg(board, "wide_class", (1500, 2500), (2500, 2500), 0, &[1], 30, 2, false);

    let area = |x1, y1, x2, y2| Area::Shape(Shape::Tile(TileShape::Box(IntBox::from_coords(x1, y1, x2, y2))));
    println!(
        "checkShape(free)={}",
        board.check_shape(&area(-4500, 4000, -4000, 4500), Some(0), &[1], 1)
    );
    println!(
        "checkShape(obstacle)={}",
        board.check_shape(&area(2200, 2200, 2400, 2400), Some(0), &[1], 1)
    );
    println!(
        "checkShape(outside)={}",
        board.check_shape(&area(-20000, 0, -19000, 100), Some(0), &[1], 1)
    );
    let tile = |x1, y1, x2, y2| TileShape::Box(IntBox::from_coords(x1, y1, x2, y2));
    println!(
        "checkTraceShape(free)={}",
        board.check_trace_shape(&tile(-4500, 4000, -4000, 4500), 0, &[1], 1, None)
    );
    println!(
        "checkTraceShape(obstacle)={}",
        board.check_trace_shape(&tile(2200, 2200, 2400, 2400), 0, &[1], 1, None)
    );
    let contact_pins: BTreeSet<ItemId> = BTreeSet::from([ItemId(2)]);
    println!(
        "checkTraceShape(atPin, contactPins)={}",
        board.check_trace_shape(&tile(-1050, -50, -950, 50), 0, &[1], 1, Some(&contact_pins))
    );
    println!(
        "checkTraceShape(atPin, emptyContactPins)={}",
        board.check_trace_shape(&tile(-1050, -50, -950, 50), 0, &[1], 1, Some(&BTreeSet::new()))
    );
    println!(
        "checkPolylineTrace(free)={}",
        board.check_polyline_trace(
            &Polyline::from_points(&[Point::new(-4000, 4000), Point::new(-3000, 4000)]),
            0,
            30,
            &[1],
            1
        )
    );
    println!(
        "checkPolylineTrace(obstacle)={}",
        board.check_polyline_trace(
            &Polyline::from_points(&[Point::new(1500, 2500), Point::new(2500, 2500)]),
            0,
            30,
            &[1],
            1
        )
    );
    println!(
        "checkMoveItem(7, (10,10))={}",
        board.check_move_item(ItemId(7), &Vector::from(IntVector::new(10, 10)), &mut None)
    );
    println!(
        "checkMoveItem(4, (10,10))={}",
        board.check_move_item(ItemId(4), &Vector::from(IntVector::new(10, 10)), &mut None)
    );
    println!("checkChangeNet(7, 3)={}", board.check_change_net(ItemId(7), 3));
    println!("checkChangeNet(4, 3)={}", board.check_change_net(ItemId(4), 3));
    println!(
        "pickNearestRoutingItem((0,0), 0)={}",
        opt_id(board.pick_nearest_routing_item(&Point::new(0, 0), Some(0), None))
    );
    println!(
        "pickNearestRoutingItem((-1000,0), 0)={}",
        opt_id(board.pick_nearest_routing_item(&Point::new(-1000, 0), Some(0), None))
    );
    println!(
        "pickNearestRoutingItem((4000,4000), 0)={}",
        opt_id(board.pick_nearest_routing_item(&Point::new(4000, 4000), Some(0), None))
    );
    println!(
        "traceTail((1000,1000), 1, [1])={}",
        opt_id(board.get_trace_tail(&Point::new(1000, 1000), Some(1), &[1]))
    );
    println!(
        "traceTail((0,0), 0, [1])={}",
        opt_id(board.get_trace_tail(&Point::new(0, 0), Some(0), &[1]))
    );
    println!(
        "containsTraceTails([4,5], [])={}",
        board.contains_trace_tails([ItemId(4), ItemId(5)], &[])
    );

    // `checkPolylineTrace` builds a temporary `PolylineTrace` (BasicBoard.java:1055-1065) whose
    // `Item` constructor draws an id from the generator (Item.java:85-90).
    println!("idBefore={}", board.communication.id_gen.max_generated_id());
    board.check_polyline_trace(
        &Polyline::from_points(&[Point::new(-4000, 4000), Point::new(-3000, 4000)]),
        0,
        30,
        &[1],
        1,
    );
    println!(
        "idAfterOneCheck={}",
        board.communication.id_gen.max_generated_id()
    );
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
    println!(
        "insertedId={inserted} items={}",
        ids(board.items_in_board_order())
    );
}

// ---------------------------------------------------------------------------------------------
// Mode 5: ShapeTraceEntries, ShapeEntrySide and ShapeAndEntrySide
// ---------------------------------------------------------------------------------------------

/// A two-layer board with three traces crossing a square at the origin: one of the own net (1)
/// and two of a foreign net (2).
fn build_shove_board() -> Board {
    let ls = layers();
    let cm = ClearanceMatrix::get_default_instance(&ls, 200);
    let mut rules = BoardRules::new(layers(), cm);
    rules.create_default_net_class();
    let default_class = rules.get_default_net_class();
    let mut board = Board::new(
        Vec::new(),
        0,
        IntBox::from_coords(-10_000, -10_000, 10_000, 10_000),
        rules,
        BoardLibrary::new(Padstacks::new(layers()), Packages::new()),
        Components::new(),
        Communication::default(),
    );
    board.rules.nets.add("N1", 1, false, default_class);
    board.rules.nets.add("N2", 1, false, default_class);
    board.insert_trace_without_cleaning(
        Polyline::from_points(&[Point::new(-3000, 0), Point::new(3000, 0)]),
        0,
        30,
        vec![1],
        1,
        FixedState::Unfixed,
    );
    board.insert_trace_without_cleaning(
        Polyline::from_points(&[Point::new(0, -3000), Point::new(0, 3000)]),
        0,
        30,
        vec![2],
        1,
        FixedState::Unfixed,
    );
    board.insert_trace_without_cleaning(
        Polyline::from_points(&[Point::new(-3000, 200), Point::new(3000, 200)]),
        0,
        30,
        vec![2],
        1,
        FixedState::Unfixed,
    );
    board
}

fn dump_shape_trace_entries() {
    let mut board = build_shove_board();
    println!("mode=5");
    let shape = TileShape::Box(IntBox::from_coords(-500, -500, 500, 500));
    let own_net_nos = vec![1];
    println!("items={}", ids(board.items_in_board_order()));
    let overlaps = board.overlapping_items_with_clearance(&shape, Some(0), &own_net_nos, 1);
    println!("overlaps={}", ids(overlaps.clone()));

    println!("--- ShapeEntrySide");
    let crossing_polyline = match board.get_item(ItemId(3)).expect("the crossing trace") {
        Item::Trace(t) => t.polyline().clone(),
        _ => unreachable!(),
    };
    let from_polyline = ShapeEntrySide::from_polyline(&crossing_polyline, 1, &shape);
    println!(
        "fromPolyline no={} is={}",
        from_polyline.no,
        fp(from_polyline.border_intersection.as_ref())
    );
    let from_point = ShapeEntrySide::from_point(&Point::new(-2000, 0), &shape);
    println!(
        "fromPoint no={} is={}",
        from_point.no,
        fp(from_point.border_intersection.as_ref())
    );
    let seg = fr_geometry::LineSegment::from_polyline(&crossing_polyline, 1).expect("a segment");
    for (label, to_the_left) in [("left", true), ("right", false)] {
        let side = ShapeEntrySide::from_line_segment(&seg, &shape, to_the_left);
        println!(
            "fromSegment({label}) no={} is={}",
            side.no,
            fp(side.border_intersection.as_ref())
        );
    }
    println!(
        "NOT_CALCULATED no={} is={}",
        ShapeEntrySide::NOT_CALCULATED.no,
        fp(ShapeEntrySide::NOT_CALCULATED.border_intersection.as_ref())
    );

    println!("--- ShapeAndEntrySide");
    for orthogonal in [false, true] {
        for in_shove_check in [false, true] {
            let sae = ShapeAndEntrySide::new(&board, ItemId(3), 0, orthogonal, in_shove_check)
                .expect("the crossing trace has a tree shape at index 0");
            println!(
                "orthogonal={orthogonal} inShoveCheck={in_shove_check} shape={} bbox={} fromSide={}",
                shape_class(&sae.shape),
                boxs(&sae.shape.bounding_box()),
                match sae.from_side {
                    None => "null".to_string(),
                    Some(side) =>
                        format!("{}@{}", side.no, fp(side.border_intersection.as_ref())),
                }
            );
        }
    }

    println!("--- storeItems");
    let mut entries = ShapeTraceEntries::new(shape.clone(), 0, own_net_nos.clone(), 1, None);
    let stored = entries.store_items(&board, &overlaps, false, false);
    println!(
        "stored={stored} stackDepth={} substituteTraceCount={} traceTailsInShape={} foundObstacle={} shoveVias={}",
        entries.stack_depth(),
        entries.substitute_trace_count(),
        entries.trace_tails_in_shape(),
        opt_id(entries.get_found_obstacle()),
        entries.shove_via_list.len()
    );
    while let Some(piece) = entries.next_substitute_trace_piece(&mut board) {
        let corners: Vec<String> = (0..piece.corner_count())
            .map(|i| fp(piece.polyline().corner_approx(i).as_ref()))
            .collect();
        println!(
            "piece net={} halfWidth={} corners= {}",
            net_array(&piece.hdr.net_nos),
            piece.get_half_width(),
            corners.join(" ")
        );
    }
    println!(
        "after: substituteTraceCount={}",
        entries.substitute_trace_count()
    );

    println!("--- cutoutTrace");
    let mut board = build_shove_board();
    ShapeTraceEntries::cutout_trace(&mut board, ItemId(3), &shape, 1);
    println!("items={}", ids(board.items_in_board_order()));
    for id in board.items_in_board_order() {
        if let Some(item @ Item::Trace(t)) = board.get_item(id) {
            println!(
                "trace {} corners={} onBoard={}",
                id,
                corner_list(t),
                item.is_on_the_board()
            );
        }
    }

    println!("--- cutoutTraces");
    let mut board = build_shove_board();
    let entries2 = ShapeTraceEntries::new(shape.clone(), 0, own_net_nos, 1, None);
    let all = board.items_in_board_order();
    entries2.cutout_traces(&mut board, &all);
    println!("items={}", ids(board.items_in_board_order()));
    for id in board.items_in_board_order() {
        if let Some(Item::Trace(t)) = board.get_item(id) {
            println!("trace {} corners={}", id, corner_list(t));
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Mode 6: cycles, overlaps, the remaining inserters
// ---------------------------------------------------------------------------------------------

/// A board with a genuine cycle (two traces between the same pair of vias) and a trace whose two
/// ends both land inside one conduction area.
fn build_cycle_board() -> (Board, fr_board::PadstackId) {
    let ls = layers();
    let cm = ClearanceMatrix::get_default_instance(&ls, 200);
    let mut rules = BoardRules::new(layers(), cm);
    rules.create_default_net_class();
    let default_class = rules.get_default_net_class();
    let mut padstacks = Padstacks::new(layers());
    let thru_shape = Shape::Tile(TileShape::Box(IntBox::from_coords(-70, -70, 70, 70)));
    let thru_pad = padstacks.add(
        "thru",
        vec![Some(thru_shape.clone()), Some(thru_shape)],
        true,
        false,
    );
    let mut packages = Packages::new();
    let pkg = packages.add(
        "pkg",
        Vec::new(),
        None,
        None,
        None,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        true,
    );
    let mut components = Components::new();
    components.add_with_generated_name(Some(Point::new(0, 0)), 0.0, true, pkg);
    components.add_with_generated_name(Some(Point::new(0, 0)), 0.0, false, pkg);
    let mut board = Board::new(
        Vec::new(),
        0,
        IntBox::from_coords(-10_000, -10_000, 10_000, 10_000),
        rules,
        BoardLibrary::new(padstacks, packages),
        components,
        Communication::default(),
    );
    board.rules.nets.add("N1", 1, false, default_class);
    board.rules.nets.add("N2", 1, false, default_class);
    board.rules.nets.add("N3", 1, false, default_class);
    board.insert_via(
        thru_pad,
        Point::new(0, 0),
        vec![1],
        1,
        FixedState::Unfixed,
        true,
    );
    board.insert_via(
        thru_pad,
        Point::new(2000, 0),
        vec![1],
        1,
        FixedState::Unfixed,
        true,
    );
    board.insert_trace_without_cleaning(
        Polyline::from_points(&[Point::new(0, 0), Point::new(2000, 0)]),
        0,
        30,
        vec![1],
        1,
        FixedState::Unfixed,
    );
    board.insert_trace_without_cleaning(
        Polyline::from_points(&[
            Point::new(0, 0),
            Point::new(0, 1000),
            Point::new(2000, 1000),
            Point::new(2000, 0),
        ]),
        0,
        30,
        vec![1],
        1,
        FixedState::Unfixed,
    );
    board.insert_conduction_area(
        Area::Shape(Shape::Tile(TileShape::Box(IntBox::from_coords(
            4000, 0, 6000, 2000,
        )))),
        0,
        vec![3],
        1,
        true,
        FixedState::Unfixed,
    );
    board.insert_trace_without_cleaning(
        Polyline::from_points(&[Point::new(4200, 200), Point::new(5800, 1800)]),
        0,
        30,
        vec![3],
        1,
        FixedState::Unfixed,
    );
    (board, thru_pad)
}

fn dump_cycles_and_inserters() {
    let (board, _) = build_cycle_board();
    println!("mode=6");
    println!("items={}", ids(board.items_in_board_order()));
    for id in [4u32, 5, 7] {
        println!(
            "trace {id} startContacts={} endContacts={} isOverlap={} isCycle={} isTail={}",
            ids(descending(board.trace_start_contacts(ItemId(id)))),
            ids(descending(board.trace_end_contacts(ItemId(id)))),
            board.is_overlap(ItemId(id)),
            board.is_trace_cycle(ItemId(id)),
            board.is_tail(ItemId(id))
        );
    }
    println!(
        "connectionItems(4)={}",
        ids(descending(
            board.connection_items(ItemId(4), StopConnectionOption::None)
        ))
    );

    println!("--- removeIfCycle(4)");
    let (mut board, _) = build_cycle_board();
    println!("removed={}", board.remove_if_cycle(ItemId(4)));
    println!("items={}", ids(board.items_in_board_order()));

    println!("--- reduceNetsOfRouteItems");
    let (mut board, _) = build_cycle_board();
    board
        .get_item_mut(ItemId(4))
        .expect("a trace")
        .header_mut()
        .net_nos = vec![1, 2];
    println!("result={}", board.reduce_nets_of_route_items());
    println!(
        "nets(4)={}",
        net_array(board.get_item(ItemId(4)).expect("a trace").net_nos())
    );

    println!("--- deleteAllTracksAndVias");
    let (mut board, _) = build_cycle_board();
    board.delete_all_tracks_and_vias();
    println!("items={}", ids(board.items_in_board_order()));

    println!("--- clearAllItemTemporaryAutorouteData");
    let (mut board, _) = build_cycle_board();
    board.clear_all_item_temporary_autoroute_data();
    println!("items={}", ids(board.items_in_board_order()));

    println!("--- the remaining inserters");
    let (mut board, thru_pad) = build_cycle_board();
    let escape = board.insert_escape_via(
        thru_pad,
        Point::new(-2000, 0),
        vec![1],
        1,
        FixedState::Unfixed,
        0,
    );
    let via = match board.get_item(escape).expect("the escape via") {
        Item::Via(v) => v,
        _ => unreachable!(),
    };
    println!(
        "escapeVia id={escape} isEscapeVia={} smdLayer={} attachAllowed={}",
        via.is_escape_via, via.escape_via_smd_layer, via.attach_allowed
    );
    let via_obstacle = board.insert_via_obstacle(
        Area::Shape(Shape::Tile(TileShape::Box(IntBox::from_coords(
            -4000, 0, -3000, 1000,
        )))),
        0,
        1,
        FixedState::Unfixed,
    );
    println!(
        "viaObstacle id={via_obstacle} class={}",
        class_name(board.get_item(via_obstacle).expect("the keepout"))
    );
    for component_id in [1i32, 2] {
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
        println!(
            "componentObstacle id={keepout} component={component_id} class={} isFront={} name={}",
            class_name(board.get_item(keepout).expect("the keepout")),
            board.component_obstacle_area_is_front(keepout),
            board.item_component_name(keepout).unwrap_or("null")
        );
    }
    let obstacle_of_component = board.insert_obstacle_of_component(
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
    println!(
        "obstacleOfComponent id={obstacle_of_component} bbox={}",
        boxs(
            &board
                .get_item(obstacle_of_component)
                .expect("the keepout")
                .bounding_box(&ctx)
        )
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
    let ctx = board.ctx();
    println!(
        "componentOutline id={outline} class={} tiles={}",
        class_name(board.get_item(outline).expect("the outline")),
        board
            .get_item(outline)
            .expect("the outline")
            .tile_shape_count(&ctx)
    );
    println!(
        "items={} revision={}",
        ids(board.items_in_board_order()),
        board.revision()
    );
}

fn corner_list(t: &PolylineTrace) -> String {
    (0..t.corner_count())
        .map(|i| fp(t.polyline().corner_approx(i).as_ref()))
        .collect::<Vec<_>>()
        .join(" ")
}

fn shape_class(shape: &TileShape) -> &'static str {
    match shape {
        TileShape::Box(_) => "IntBox",
        TileShape::Octagon(_) => "IntOctagon",
        TileShape::Simplex(_) => "Simplex",
    }
}

fn fp(p: Option<&fr_geometry::FloatPoint>) -> String {
    match p {
        None => "null".to_string(),
        Some(p) => format!("({:.4},{:.4})", p.x, p.y),
    }
}

#[allow(clippy::too_many_arguments)]
fn seg(
    board: &mut Board,
    name: &str,
    from: (i32, i32),
    to: (i32, i32),
    layer: usize,
    net_nos: &[i32],
    half_width: i32,
    clearance_class: usize,
    only_not_shovable: bool,
) {
    let result = board.check_trace_segment(
        &Point::new(from.0, from.1),
        &Point::new(to.0, to.1),
        layer,
        net_nos,
        half_width,
        clearance_class,
        only_not_shovable,
    );
    println!("checkTraceSegment({name})={result:.6}");
}

// ---------------------------------------------------------------------------------------------
// Mode 3
// ---------------------------------------------------------------------------------------------

fn dump_changed_area(board: &mut Board) {
    println!("mode=3");
    println!(
        "changedArea={}",
        if board.changed_area.is_none() {
            "null"
        } else {
            "?"
        }
    );
    board.start_marking_changed_area();
    println!("after start: {}", changed_area(board));
    board.join_changed_area(&Point::new(100, 100).to_float(), 0);
    println!("after join(100,100,0): {}", changed_area(board));
    let ctx = board.ctx();
    let shape = board
        .get_item(ItemId(7))
        .expect("the obstacle area")
        .get_tile_shape(board.default_tree_id(), 0, &ctx)
        .expect("its only tile shape");
    board.mark_changed_area(&shape, 0);
    println!("after join(shape of 7, 0): {}", changed_area(board));
    println!(
        "surroundingBox={}",
        boxs(&board.changed_area.as_ref().expect("marked").surrounding_box())
    );
    board
        .changed_area
        .as_mut()
        .expect("marked")
        .set_empty(0);
    println!("after setEmpty(0): {}", changed_area(board));
    board.mark_all_changed_area();
    println!("after markAll: {}", changed_area(board));
    println!(
        "surroundingBox={}",
        boxs(&board.changed_area.as_ref().expect("marked").surrounding_box())
    );

    println!("--- ignoreConduction / changeConductionIsObstacle (quirk 50)");
    println!("ignoreConduction={}", board.rules.get_ignore_conduction());
    println!("isObstacle(8)={}", is_obstacle(board, 8));
    board.change_conduction_is_obstacle(false);
    println!(
        "after change(false): ignoreConduction={} isObstacle(8)={}",
        board.rules.get_ignore_conduction(),
        is_obstacle(board, 8)
    );
    board.change_conduction_is_obstacle(false);
    println!(
        "after change(false) again: ignoreConduction={} isObstacle(8)={}",
        board.rules.get_ignore_conduction(),
        is_obstacle(board, 8)
    );
    board.change_conduction_is_obstacle(true);
    println!(
        "after change(true): ignoreConduction={} isObstacle(8)={}",
        board.rules.get_ignore_conduction(),
        is_obstacle(board, 8)
    );

    println!("--- unfillConductionAreas");
    board.unfill_conduction_areas();
    println!(
        "ignoreConduction={} isObstacle(8)={} isFilled(8)={}",
        board.rules.get_ignore_conduction(),
        is_obstacle(board, 8),
        is_filled(board, 8)
    );

    println!("--- removeTraceTails(1, NONE)");
    println!(
        "removed={}",
        board.remove_trace_tails(1, StopConnectionOption::None)
    );
    println!("items={}", ids(board.items_in_board_order()));

    println!("--- moveBy(item 7, (10, 20))");
    board
        .move_item_by(ItemId(7), &Vector::from(IntVector::new(10, 20)))
        .expect("an obstacle area translates without a polyline error");
    println!("items={}", ids(board.items_in_board_order()));
    let ctx = board.ctx();
    println!(
        "item 7 bbox={}",
        boxs(&board.get_item(ItemId(7)).expect("area").bounding_box(&ctx))
    );
    println!(
        "overlappingObjects at the moved area={}",
        objects(&board.overlapping_objects(
            &TileShape::Box(IntBox::from_coords(2500, 2500, 2600, 2600)),
            Some(0)
        ))
    );

    println!("--- changeClearanceClassIndex(4, 2)");
    board.change_clearance_class_index(ItemId(4), 2);
    println!(
        "cl(4)={}",
        board.get_item(ItemId(4)).expect("trace").clearance_class()
    );
    // No `validate` in between: `changeClearanceClassIndex` left the trace with tree leaves and
    // an empty shape cache, so the two queries below have to recompute (Item.java:212-238).
    let cold_probe = TileShape::Box(IntBox::from_coords(-600, -100, -400, 100));
    println!(
        "overlappingObjects after change={}",
        objects(&board.overlapping_objects(&cold_probe, Some(0)))
    );
    println!(
        "overlappingItemsWithClearance after change={}",
        ids(board.overlapping_items_with_clearance(&cold_probe, Some(0), &[], 1))
    );
    println!("validate(4)={}", board.validate_item(ItemId(4)));

    println!("--- makeConductive(7, 3)");
    let new_id = board.make_conductive(ItemId(7), 3).expect("an area");
    println!(
        "newId={} nets={} items={}",
        new_id,
        net_array(board.get_item(new_id).expect("new item").net_nos()),
        ids(board.items_in_board_order())
    );

    println!("--- generateKeepoutOutside(true)");
    let outline = board.get_outline().expect("an outline");
    let tree = board.default_tree_id();
    let ctx = board.ctx();
    println!(
        "outline tiles before={} treeShapes={}",
        board
            .get_item(outline)
            .expect("outline")
            .tile_shape_count(&ctx),
        board
            .get_item(outline)
            .expect("outline")
            .tree_shape_count(tree)
    );
    board.generate_keepout_outside(outline, true);
    let ctx = board.ctx();
    println!(
        "outline tiles after={} treeShapes={}",
        board
            .get_item(outline)
            .expect("outline")
            .tile_shape_count(&ctx),
        board
            .get_item(outline)
            .expect("outline")
            .tree_shape_count(tree)
    );
    println!(
        "keepoutGenerated={}",
        match board.get_item(outline).expect("outline") {
            Item::BoardOutline(o) => o.keepout_outside_outline_generated(),
            _ => unreachable!(),
        }
    );

    println!("--- net queries");
    println!("net1 terminalItems={}", ids(board.net_terminal_items(1)));
    println!("net1 pins={}", ids(board.net_pins(1)));
    println!("net1 items={}", ids(board.net_items(1)));
    println!("net1 traceLength={:.6}", board.net_trace_length(1));
    println!("net1 viaCount={}", board.net_via_count(1));
}

fn changed_area(board: &Board) -> String {
    let area = board.changed_area.as_ref().expect("marked");
    (0..board.get_layer_count())
        .map(|layer| format!("{}:{}", layer, oct(&area.get_area(layer))))
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_obstacle(board: &Board, id: u32) -> bool {
    match board.get_item(ItemId(id)).expect("a conduction area") {
        Item::ConductionArea(area) => area.get_is_obstacle(),
        _ => unreachable!(),
    }
}

fn is_filled(board: &Board, id: u32) -> bool {
    match board.get_item(ItemId(id)).expect("a conduction area") {
        Item::ConductionArea(area) => area.get_is_filled(),
        _ => unreachable!(),
    }
}

// ---------------------------------------------------------------------------------------------
// Formatting — the exact twin of `P2T11.java`'s helpers.
// ---------------------------------------------------------------------------------------------

fn class_name(item: &Item) -> &'static str {
    match item {
        Item::Trace(_) => "PolylineTrace",
        Item::Via(_) => "Via",
        Item::Pin(_) => "Pin",
        Item::ObstacleArea(_) => "ObstacleArea",
        Item::ConductionArea(_) => "ConductionArea",
        Item::ViaObstacleArea(_) => "ViaObstacleArea",
        Item::ComponentObstacleArea(_) => "ComponentObstacleArea",
        Item::ComponentOutline(_) => "ComponentOutline",
        Item::BoardOutline(_) => "BoardOutline",
    }
}

/// `java.util.Arrays.toString(int[])`.
fn net_array(net_nos: &[i32]) -> String {
    format!(
        "[{}]",
        net_nos
            .iter()
            .map(i32::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn boxs(b: &IntBox) -> String {
    format!("Box[{},{}..{},{}]", b.ll.x, b.ll.y, b.ur.x, b.ur.y)
}

fn oct(o: &fr_geometry::IntOctagon) -> String {
    format!(
        "Oct[{},{},{},{},{},{},{},{}]",
        o.left_x,
        o.bottom_y,
        o.right_x,
        o.top_y,
        o.upper_left_diagonal_x,
        o.lower_right_diagonal_x,
        o.lower_left_diagonal_x,
        o.upper_right_diagonal_x
    )
}

fn point(p: Option<&Point>) -> String {
    match p {
        None => "null".to_string(),
        Some(Point::Int(ip)) => format!("({},{})", ip.x, ip.y),
        Some(other) => panic!("a rational point where Java casts to IntPoint: {other:?}"),
    }
}

fn points(arr: &[Point]) -> String {
    format!(
        "[{}]",
        arr.iter()
            .map(|p| point(Some(p)))
            .collect::<Vec<_>>()
            .join(" ")
    )
}

fn ids(items: impl IntoIterator<Item = ItemId>) -> String {
    format!(
        "[{}]",
        items
            .into_iter()
            .map(|id| id.0.to_string())
            .collect::<Vec<_>>()
            .join(" ")
    )
}

fn objects(objects: &BTreeSet<TreeObject>) -> String {
    format!(
        "[{}]",
        objects
            .iter()
            .map(|o| match o {
                TreeObject::Item(id) => id.0.to_string(),
                TreeObject::Room(_) => unreachable!("no expansion rooms in Plan 2"),
            })
            .collect::<Vec<_>>()
            .join(" ")
    )
}

fn opt_id(id: Option<ItemId>) -> String {
    id.map_or_else(|| "null".to_string(), |id| id.0.to_string())
}

fn layer_or_minus_one(layer: Option<usize>) -> i32 {
    layer.map_or(-1, |l| l as i32)
}

/// Java's `TreeSet<Item>` iterates in **descending** id (quirk #44); the port's `BTreeSet<ItemId>`
/// is ascending, so every set printed here is reversed first.
fn descending(set: BTreeSet<ItemId>) -> Vec<ItemId> {
    set.into_iter().rev().collect()
}
