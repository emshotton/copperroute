//! Rust twin of `scripts/differential/java/P2T10.java` (Plan 2 Task 10).
//!
//! Builds the same two-layer board — two pins from one component, two traces, an empty board
//! outline — through `copper-board`'s public API, then prints the same lines the Java driver prints:
//! every item's identity, every tree's key/leaves/stored shapes, and the result of every query.
//!
//! Modes: `0` = 45-degree board, `1` = 90-degree, `2` = no angle restriction, `3` = the
//! clearance-matrix dump plus the in-place mutation methods.

use std::collections::BTreeMap;

use copper_board::prelude::*;
use copper_board::LeafId;
use copper_geometry::{
    Area, IntBox, IntOctagon, IntPoint, IntVector, Point, PolygonShape, Polyline, PolylineShapeRef,
    Shape, Simplex, TileShape, Vector,
};

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

struct Board {
    library: BoardLibrary,
    components: Components,
    rules: BoardRules,
    bounding_box: IntBox,
    items: BTreeMap<ItemId, Item>,
    manager: SearchTreeManager,
}

impl Board {
    fn ctx(&self) -> ItemCtx<'_> {
        ItemCtx {
            library: &self.library,
            components: &self.components,
            rules: &self.rules,
            bounding_box: &self.bounding_box,
            max_tree_shape_width: DEFAULT_MAX_TREE_SHAPE_WIDTH,
        }
    }

    /// `board.itemList` order: descending id (UndoableObjects' `ConcurrentSkipListMap` keyed by
    /// `Item.compareTo`).
    fn board_order(&self) -> Vec<ItemId> {
        self.items.keys().rev().copied().collect()
    }

    fn insert_all(&mut self) {
        let mut items = std::mem::take(&mut self.items);
        let ctx = ItemCtx {
            library: &self.library,
            components: &self.components,
            rules: &self.rules,
            bounding_box: &self.bounding_box,
            max_tree_shape_width: DEFAULT_MAX_TREE_SHAPE_WIDTH,
        };
        for item in items.values_mut() {
            self.manager.insert(item, &ctx);
        }
        self.items = items;
    }

    fn autoroute_tree(&mut self, clearance_class_index: usize) -> TreeId {
        let mut items = std::mem::take(&mut self.items);
        let ctx = ItemCtx {
            library: &self.library,
            components: &self.components,
            rules: &self.rules,
            bounding_box: &self.bounding_box,
            max_tree_shape_width: DEFAULT_MAX_TREE_SHAPE_WIDTH,
        };
        let mut refs: Vec<&mut Item> = items.values_mut().rev().collect();
        let id = self
            .manager
            .get_autoroute_tree(clearance_class_index, &mut refs, &ctx)
            .id();
        drop(refs);
        self.items = items;
        id
    }

    fn tree(&self, id: TreeId) -> &ShapeSearchTree {
        self.manager
            .trees()
            .find(|tree| tree.id() == id)
            .expect("tree exists")
    }
}

fn build(mode: i32) -> Board {
    let layers = || LayerStructure::new(vec![Layer::new("front", true), Layer::new("back", true)]);
    let mut clearance_matrix = ClearanceMatrix::get_default_instance(&layers(), 200);
    assert!(clearance_matrix.append_class("wide"));
    clearance_matrix.set_value_on_all_layers(2, 1, 600);
    clearance_matrix.set_value_on_all_layers(2, 2, 800);
    let mut rules = BoardRules::new(layers(), clearance_matrix);
    rules.trace_angle_restriction = match mode {
        1 => AngleRestriction::NinetyDegree,
        2 => AngleRestriction::None,
        _ => AngleRestriction::FortyFiveDegree,
    };

    let mut padstacks = Padstacks::new(layers());
    let smd = padstacks.add(
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
    let through_shape = Shape::Tile(TileShape::Octagon(IntOctagon::new(
        -70, -70, 70, 70, -140, 140, -140, 140,
    )));
    let through = padstacks.add(
        "thru",
        vec![Some(through_shape.clone()), Some(through_shape)],
        true,
        false,
    );
    let mut packages = Packages::new();
    let package = packages.add(
        "pkg",
        vec![
            PackagePin::new("P1", smd, IntVector::new(-500, 0).into(), 0.0),
            PackagePin::new("P2", through, IntVector::new(500, 0).into(), 0.0),
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
    components.add_with_generated_name(Some(Point::new(0, 0)), 0.0, true, package);

    let mut items = BTreeMap::new();
    items.insert(
        ItemId(1),
        Item::BoardOutline(BoardOutline::new(
            ItemHeader::new(ItemId(1), Vec::new(), 0, 0, FixedState::SystemFixed),
            Vec::new(),
        )),
    );
    items.insert(
        ItemId(2),
        Item::Pin(Pin::new(
            ItemHeader::new(ItemId(2), vec![1], 1, 1, FixedState::Unfixed),
            0,
        )),
    );
    items.insert(
        ItemId(3),
        Item::Pin(Pin::new(
            ItemHeader::new(ItemId(3), vec![1], 1, 1, FixedState::Unfixed),
            1,
        )),
    );
    items.insert(
        ItemId(4),
        Item::Trace(PolylineTrace::new(
            ItemHeader::new(ItemId(4), vec![1], 1, 0, FixedState::Unfixed),
            Polyline::from_points(&[
                Point::new(-500, 0),
                Point::new(0, 0),
                Point::new(0, 400),
                Point::new(500, 400),
            ]),
            0,
            30,
            None,
        )),
    );
    items.insert(
        ItemId(5),
        Item::Trace(PolylineTrace::new(
            ItemHeader::new(ItemId(5), vec![2], 2, 0, FixedState::Unfixed),
            Polyline::from_points(&[
                Point::new(-800, 300),
                Point::new(-800, 900),
                Point::new(300, 900),
            ]),
            0,
            40,
            None,
        )),
    );

    let mut board = Board {
        library,
        components,
        rules,
        bounding_box: BOUNDING_BOX,
        items,
        manager: SearchTreeManager::new(),
    };
    board.insert_all();
    board
}

/// Mode 4: a three-layer board with a real outline polygon, an obstacle area, a conduction area
/// and a through via whose middle layer has no pad.
fn build_areas() -> Board {
    let layers = || {
        LayerStructure::new(vec![
            Layer::new("front", true),
            Layer::new("inner", true),
            Layer::new("back", true),
        ])
    };
    let clearance_matrix = ClearanceMatrix::get_default_instance(&layers(), 200);
    let mut rules = BoardRules::new(layers(), clearance_matrix);
    rules.set_hole_clearance(300);
    let bounding_box = IntBox::from_coords(-5000, -5000, 5000, 5000);

    let mut padstacks = Padstacks::new(layers());
    let pad = Shape::Tile(TileShape::Box(IntBox::from_coords(-80, -80, 80, 80)));
    let via_pad = padstacks.add(
        "via",
        vec![Some(pad.clone()), None, Some(pad)],
        true,
        false,
    );
    let library = BoardLibrary::new(padstacks, Packages::new());

    let mut items = BTreeMap::new();
    items.insert(
        ItemId(1),
        Item::BoardOutline(BoardOutline::new(
            ItemHeader::new(ItemId(1), Vec::new(), 1, 0, FixedState::SystemFixed),
            vec![PolylineShapeRef::Polygon(PolygonShape::from_points(&[
                Point::new(-3000, -2000),
                Point::new(3000, -2000),
                Point::new(3000, 2000),
                Point::new(-3000, 2000),
            ]))],
        )),
    );
    items.insert(
        ItemId(2),
        Item::Via(Via::new(
            ItemHeader::new(ItemId(2), vec![1], 1, 0, FixedState::Unfixed),
            via_pad,
            Point::new(1000, 0),
            true,
        )),
    );
    let l_shape = Area::Shape(Shape::Polygon(PolygonShape::from_points(&[
        Point::new(0, 0),
        Point::new(2000, 0),
        Point::new(2000, 1000),
        Point::new(1000, 1000),
        Point::new(1000, 2000),
        Point::new(0, 2000),
    ])));
    items.insert(
        ItemId(3),
        Item::ObstacleArea(ObstacleArea::new(
            ItemHeader::new(ItemId(3), Vec::new(), 1, 0, FixedState::Unfixed),
            ObstacleAreaData::new(l_shape, 0, Vector::ZERO, 0.0, false, None),
        )),
    );
    let square = Area::Shape(Shape::Tile(TileShape::Box(IntBox::from_coords(
        -2000, -1500, -1000, -500,
    ))));
    items.insert(
        ItemId(4),
        Item::ConductionArea(ConductionArea::new(
            ItemHeader::new(ItemId(4), vec![2], 1, 0, FixedState::Unfixed),
            ObstacleAreaData::new(square, 2, Vector::ZERO, 0.0, false, None),
            true,
        )),
    );

    let mut board = Board {
        library,
        components: Components::new(),
        rules,
        bounding_box,
        items,
        manager: SearchTreeManager::new(),
    };
    board.insert_all();
    board
}

fn dump_areas(board: &mut Board) {
    println!("mode=4 holeClearance={}", board.rules.get_hole_clearance());
    {
        let ctx = board.ctx();
        for id in board.board_order() {
            let item = &board.items[&id];
            println!(
                "item id={} class={} layers={}..{} tileShapeCount={}",
                id.0,
                class_name(item),
                item.first_layer(&ctx),
                item.last_layer(&ctx),
                item.tile_shape_count(&ctx)
            );
        }
    }
    let default_id = board.manager.get_default_tree().id();
    dump_tree(board, "default", default_id);
    let auto1 = board.autoroute_tree(1);
    dump_tree(board, "autoroute_cc1", auto1);

    println!("--- generateKeepoutOutside(true)");
    // BoardOutline.generateKeepoutOutside (BoardOutline.java:229-243) flips the flag and then
    // re-inserts itself into every search tree; the port's `Item` half only flips the flag (the
    // tree half is Task 11's `Board::generate_keepout_outside`), so the driver does both.
    let mut outline = board.items.remove(&ItemId(1)).expect("the outline");
    board.manager.remove(&mut outline);
    if let Item::BoardOutline(o) = &mut outline {
        o.generate_keepout_outside(true);
    }
    let ctx = ItemCtx {
        library: &board.library,
        components: &board.components,
        rules: &board.rules,
        bounding_box: &board.bounding_box,
        max_tree_shape_width: DEFAULT_MAX_TREE_SHAPE_WIDTH,
    };
    board.manager.insert(&mut outline, &ctx);
    board.items.insert(ItemId(1), outline);
    dump_tree(board, "default_keepout", default_id);

    let probe = TileShape::Box(IntBox::from_coords(500, -500, 1500, 1500));
    query(
        board,
        "default_keepout",
        default_id,
        &probe,
        Some(0),
        &[],
        1,
    );
}

/// Mode 5: the entry-surgery methods. `traceA` is item 2, `traceB` item 3; the empty board
/// outline keeps item 1, matching `BasicBoard`'s own numbering.
fn build_traces() -> Board {
    let layers = || LayerStructure::new(vec![Layer::new("front", true), Layer::new("back", true)]);
    let clearance_matrix = ClearanceMatrix::get_default_instance(&layers(), 200);
    let rules = BoardRules::new(layers(), clearance_matrix);
    let mut items = BTreeMap::new();
    items.insert(
        ItemId(1),
        Item::BoardOutline(BoardOutline::new(
            ItemHeader::new(ItemId(1), Vec::new(), 0, 0, FixedState::SystemFixed),
            Vec::new(),
        )),
    );
    items.insert(
        ItemId(2),
        Item::Trace(PolylineTrace::new(
            ItemHeader::new(ItemId(2), vec![1], 1, 0, FixedState::Unfixed),
            Polyline::from_points(&[
                Point::new(-500, 0),
                Point::new(0, 0),
                Point::new(0, 400),
            ]),
            0,
            30,
            None,
        )),
    );
    items.insert(
        ItemId(3),
        Item::Trace(PolylineTrace::new(
            ItemHeader::new(ItemId(3), vec![1], 1, 0, FixedState::Unfixed),
            Polyline::from_points(&[
                Point::new(0, 400),
                Point::new(500, 400),
                Point::new(500, 900),
            ]),
            0,
            30,
            None,
        )),
    );
    let mut board = Board {
        library: BoardLibrary::new(Padstacks::new(layers()), Packages::new()),
        components: Components::new(),
        rules,
        bounding_box: BOUNDING_BOX,
        items,
        manager: SearchTreeManager::new(),
    };
    board.insert_all();
    board
}

fn take_trace(board: &mut Board, id: u32) -> PolylineTrace {
    match board.items.remove(&ItemId(id)).expect("trace") {
        Item::Trace(trace) => trace,
        _ => unreachable!(),
    }
}

fn dump_traces(board: &mut Board) {
    let default_id = board.manager.get_default_tree().id();
    println!("mode=5");
    let (a, b) = (
        match &board.items[&ItemId(2)] {
            Item::Trace(t) => t.tile_shape_count(),
            _ => unreachable!(),
        },
        match &board.items[&ItemId(3)] {
            Item::Trace(t) => t.tile_shape_count(),
            _ => unreachable!(),
        },
    );
    println!("traceA shapes={a} traceB shapes={b}");
    dump_tree(board, "before", default_id);

    let joined = Polyline::from_points(&[
        Point::new(-500, 0),
        Point::new(0, 0),
        Point::new(0, 400),
        Point::new(500, 400),
        Point::new(500, 900),
    ]);
    println!("joined lines={}", joined.lines().len());
    println!("--- mergeEntriesAtEnd(from=traceB, to=traceA, joined, 1, 4)");
    let mut trace_a = take_trace(board, 2);
    let mut trace_b = take_trace(board, 3);
    {
        let rules = &board.rules;
        board.manager.get_default_tree_mut().merge_entries_at_end(
            &mut trace_b,
            &mut trace_a,
            &joined,
            1,
            4,
            rules,
        );
    }
    board.items.insert(ItemId(2), Item::Trace(trace_a));
    board.items.insert(ItemId(3), Item::Trace(trace_b));
    dump_tree(board, "after_mergeAtEnd", default_id);
    println!(
        "validateEntries(traceA)={}",
        board
            .tree(default_id)
            .validate_entries(&board.items[&ItemId(2)])
    );
    println!(
        "traceA treeShapeCount={} traceB treeShapeCount={}",
        board.items[&ItemId(2)].tree_shape_count(default_id),
        board.items[&ItemId(3)].tree_shape_count(default_id)
    );

    let mut board = build_traces();
    let default_id = board.manager.get_default_tree().id();
    println!("--- mergeEntriesInFront(from=traceA, to=traceB, joined, 1, 4)");
    let mut trace_a = take_trace(&mut board, 2);
    let mut trace_b = take_trace(&mut board, 3);
    {
        let rules = &board.rules;
        board.manager.get_default_tree_mut().merge_entries_in_front(
            &mut trace_a,
            &mut trace_b,
            &joined,
            1,
            4,
            rules,
        );
    }
    board.items.insert(ItemId(2), Item::Trace(trace_a));
    board.items.insert(ItemId(3), Item::Trace(trace_b));
    dump_tree(&board, "after_mergeInFront", default_id);
    println!(
        "validateEntries(traceB)={}",
        board
            .tree(default_id)
            .validate_entries(&board.items[&ItemId(3)])
    );
    println!(
        "traceA treeShapeCount={} traceB treeShapeCount={}",
        board.items[&ItemId(2)].tree_shape_count(default_id),
        board.items[&ItemId(3)].tree_shape_count(default_id)
    );

    let mut board = build_traces();
    let default_id = board.manager.get_default_tree().id();
    let long_trace = PolylineTrace::new(
        ItemHeader::new(ItemId(4), vec![1], 1, 0, FixedState::Unfixed),
        Polyline::from_points(&[
            Point::new(-1000, -1000),
            Point::new(-1000, 0),
            Point::new(0, 0),
            Point::new(0, 1000),
            Point::new(1000, 1000),
            Point::new(1000, 2000),
        ]),
        0,
        30,
        None,
    );
    let mut long_item = Item::Trace(long_trace);
    {
        let ctx = ItemCtx {
            library: &board.library,
            components: &board.components,
            rules: &board.rules,
            bounding_box: &board.bounding_box,
            max_tree_shape_width: DEFAULT_MAX_TREE_SHAPE_WIDTH,
        };
        board.manager.insert(&mut long_item, &ctx);
    }
    board.items.insert(ItemId(4), long_item);
    println!("--- reuseEntriesAfterCutout(long, start, end)");
    println!(
        "long shapes={}",
        match &board.items[&ItemId(4)] {
            Item::Trace(t) => t.tile_shape_count(),
            _ => unreachable!(),
        }
    );
    let mut start_piece = PolylineTrace::new(
        ItemHeader::new(ItemId(5), vec![1], 1, 0, FixedState::Unfixed),
        Polyline::from_points(&[
            Point::new(-1000, -1000),
            Point::new(-1000, 0),
            Point::new(0, 0),
        ]),
        0,
        30,
        None,
    );
    let mut end_piece = PolylineTrace::new(
        ItemHeader::new(ItemId(6), vec![1], 1, 0, FixedState::Unfixed),
        Polyline::from_points(&[
            Point::new(0, 1000),
            Point::new(1000, 1000),
            Point::new(1000, 2000),
        ]),
        0,
        30,
        None,
    );
    let mut long_trace = take_trace(&mut board, 4);
    {
        let ctx = ItemCtx {
            library: &board.library,
            components: &board.components,
            rules: &board.rules,
            bounding_box: &board.bounding_box,
            max_tree_shape_width: DEFAULT_MAX_TREE_SHAPE_WIDTH,
        };
        board
            .manager
            .get_default_tree_mut()
            .reuse_entries_after_cutout(
                &mut long_trace,
                &mut start_piece,
                &mut end_piece,
                &ctx,
            );
    }
    let long_entries: Vec<Option<LeafId>> = long_trace
        .hdr
        .get_tree_entries(default_id)
        .expect("entries")
        .to_vec();
    let start_len = start_piece.hdr.get_tree_entries(default_id).expect("entries").len();
    let end_len = end_piece.hdr.get_tree_entries(default_id).expect("entries").len();
    let start_item = Item::Trace(start_piece);
    let end_item = Item::Trace(end_piece);
    board.items.insert(ItemId(4), Item::Trace(long_trace));
    dump_tree(&board, "after_cutout", default_id);
    let mut line = String::from("long entries:");
    for entry in &long_entries {
        match entry {
            None => line.push_str(" null"),
            Some(leaf) => {
                let e = board.tree(default_id).tree().leaf_entry(*leaf);
                let TreeObject::Item(ItemId(id)) = e.object else {
                    unreachable!()
                };
                line.push_str(&format!(" {id}/{}", e.shape_index));
            }
        }
    }
    println!("{line}");
    println!("start entries={start_len} end entries={end_len}");
    println!(
        "validateEntries(start)={} validateEntries(end)={}",
        board.tree(default_id).validate_entries(&start_item),
        board.tree(default_id).validate_entries(&end_item)
    );
}

/// Mode 6: `reduceTraceShapeAtTiePin` — the SMD pin sits exactly on the trace's first corner.
fn dump_tie_pin(board: &mut Board) {
    let default_id = board.manager.get_default_tree().id();
    println!("mode=6");
    let pin = match &board.items[&ItemId(2)] {
        Item::Pin(pin) => pin.clone(),
        _ => unreachable!(),
    };
    let trace_layer = match &board.items[&ItemId(4)] {
        Item::Trace(t) => t.get_layer(),
        _ => unreachable!(),
    };
    {
        let ctx = board.ctx();
        println!(
            "pin center={} trace firstCorner={}",
            pt(&pin.get_center(&ctx)),
            pt(&match &board.items[&ItemId(4)] {
                Item::Trace(t) => t.first_corner().expect("a corner"),
                _ => unreachable!(),
            })
        );
        println!(
            "pinShape={}",
            shp(pin
                .get_tree_shape_on_layer(default_id, trace_layer, &ctx)
                .expect("a shape"))
        );
    }
    dump_tree(board, "before", default_id);
    println!("--- reduceTraceShapeAtTiePin(pin 2, trace 4)");
    let mut trace = take_trace(board, 4);
    {
        let ctx = ItemCtx {
            library: &board.library,
            components: &board.components,
            rules: &board.rules,
            bounding_box: &board.bounding_box,
            max_tree_shape_width: DEFAULT_MAX_TREE_SHAPE_WIDTH,
        };
        board
            .manager
            .get_default_tree_mut()
            .reduce_trace_shape_at_tie_pin(&pin, &mut trace, &ctx);
    }
    board.items.insert(ItemId(4), Item::Trace(trace));
    dump_tree(board, "after", default_id);
    println!(
        "validateEntries(4)={}",
        board
            .tree(default_id)
            .validate_entries(&board.items[&ItemId(4)])
    );
}

/// Mode 7: an outline whose edges run in none of the tree's directions.
/// Mode 8: the `changeOrder == true` half of `mergeEntriesInFront` (ShapeSearchTree.java:176)
/// and `mergeEntriesAtEnd` (:251) — two traces meeting head-to-head or tail-to-tail, so
/// `fromTrace`'s entries have to be transferred in reverse.
fn dump_change_order_merges() {
    let joined = Polyline::from_points(&[
        Point::new(-500, -500),
        Point::new(-500, 0),
        Point::new(0, 0),
        Point::new(500, 0),
        Point::new(500, 500),
    ]);
    println!("mode=8");

    let mut board = build_merge_pair(
        &[
            Point::new(0, 0),
            Point::new(-500, 0),
            Point::new(-500, -500),
        ],
        &[Point::new(0, 0), Point::new(500, 0), Point::new(500, 500)],
    );
    let default_id = board.manager.get_default_tree().id();
    let (first_a, first_b) = (trace_first_corner(&board, 2), trace_first_corner(&board, 3));
    println!(
        "headToHead changeOrder={} traceA shapes={} traceB shapes={}",
        first_a == first_b,
        trace_shape_count(&board, 2),
        trace_shape_count(&board, 3)
    );
    dump_tree(&board, "before_inFront", default_id);
    println!("--- mergeEntriesInFront(from=traceA, to=traceB, joined, 1, 4)");
    let mut trace_a = take_trace(&mut board, 2);
    let mut trace_b = take_trace(&mut board, 3);
    {
        let rules = &board.rules;
        board.manager.get_default_tree_mut().merge_entries_in_front(
            &mut trace_a,
            &mut trace_b,
            &joined,
            1,
            4,
            rules,
        );
    }
    board.items.insert(ItemId(2), Item::Trace(trace_a));
    board.items.insert(ItemId(3), Item::Trace(trace_b));
    dump_tree(&board, "after_inFront", default_id);
    println!(
        "validateEntries(traceB)={}",
        board
            .tree(default_id)
            .validate_entries(&board.items[&ItemId(3)])
    );

    let mut board = build_merge_pair(
        &[
            Point::new(-500, -500),
            Point::new(-500, 0),
            Point::new(0, 0),
        ],
        &[Point::new(500, 500), Point::new(500, 0), Point::new(0, 0)],
    );
    let default_id = board.manager.get_default_tree().id();
    let (last_a, last_b) = (trace_last_corner(&board, 2), trace_last_corner(&board, 3));
    println!(
        "tailToTail changeOrder={} traceA shapes={} traceB shapes={}",
        last_a == last_b,
        trace_shape_count(&board, 2),
        trace_shape_count(&board, 3)
    );
    dump_tree(&board, "before_atEnd", default_id);
    println!("--- mergeEntriesAtEnd(from=traceA, to=traceB, joined, 1, 4)");
    let mut trace_a = take_trace(&mut board, 2);
    let mut trace_b = take_trace(&mut board, 3);
    {
        let rules = &board.rules;
        board.manager.get_default_tree_mut().merge_entries_at_end(
            &mut trace_a,
            &mut trace_b,
            &joined,
            1,
            4,
            rules,
        );
    }
    board.items.insert(ItemId(2), Item::Trace(trace_a));
    board.items.insert(ItemId(3), Item::Trace(trace_b));
    dump_tree(&board, "after_atEnd", default_id);
    println!(
        "validateEntries(traceB)={}",
        board
            .tree(default_id)
            .validate_entries(&board.items[&ItemId(3)])
    );
}

fn trace_shape_count(board: &Board, id: u32) -> usize {
    match &board.items[&ItemId(id)] {
        Item::Trace(t) => t.tile_shape_count(),
        _ => unreachable!(),
    }
}

fn trace_first_corner(board: &Board, id: u32) -> Point {
    match &board.items[&ItemId(id)] {
        Item::Trace(t) => t.first_corner().expect("a corner"),
        _ => unreachable!(),
    }
}

fn trace_last_corner(board: &Board, id: u32) -> Point {
    match &board.items[&ItemId(id)] {
        Item::Trace(t) => t.last_corner().expect("a corner"),
        _ => unreachable!(),
    }
}

/// A fresh two-layer board carrying exactly the two traces described by `a` and `b`.
fn build_merge_pair(a: &[Point], b: &[Point]) -> Board {
    let layers = || LayerStructure::new(vec![Layer::new("front", true), Layer::new("back", true)]);
    let clearance_matrix = ClearanceMatrix::get_default_instance(&layers(), 200);
    let rules = BoardRules::new(layers(), clearance_matrix);
    let mut items = BTreeMap::new();
    items.insert(
        ItemId(1),
        Item::BoardOutline(BoardOutline::new(
            ItemHeader::new(ItemId(1), Vec::new(), 0, 0, FixedState::SystemFixed),
            Vec::new(),
        )),
    );
    for (index, corners) in [a, b].into_iter().enumerate() {
        let id = ItemId(index as u32 + 2);
        items.insert(
            id,
            Item::Trace(PolylineTrace::new(
                ItemHeader::new(id, vec![1], 1, 0, FixedState::Unfixed),
                Polyline::from_points(corners),
                0,
                30,
                None,
            )),
        );
    }
    let mut board = Board {
        library: BoardLibrary::new(Padstacks::new(layers()), Packages::new()),
        components: Components::new(),
        rules,
        bounding_box: BOUNDING_BOX,
        items,
        manager: SearchTreeManager::new(),
    };
    board.insert_all();
    board
}

fn build_skewed_outline() -> Board {
    let layers = || LayerStructure::new(vec![Layer::new("front", true), Layer::new("back", true)]);
    let rules = BoardRules::new(
        layers(),
        ClearanceMatrix::get_default_instance(&layers(), 200),
    );
    let mut items = BTreeMap::new();
    items.insert(
        ItemId(1),
        Item::BoardOutline(BoardOutline::new(
            ItemHeader::new(ItemId(1), Vec::new(), 1, 0, FixedState::SystemFixed),
            vec![PolylineShapeRef::Polygon(PolygonShape::from_points(&[
                Point::new(0, 0),
                Point::new(3000, 500),
                Point::new(1000, 2500),
            ]))],
        )),
    );
    let mut board = Board {
        library: BoardLibrary::new(Padstacks::new(layers()), Packages::new()),
        components: Components::new(),
        rules,
        bounding_box: IntBox::from_coords(-5000, -5000, 5000, 5000),
        items,
        manager: SearchTreeManager::new(),
    };
    board.insert_all();
    board
}

fn angle_name(angle: AngleRestriction) -> &'static str {
    match angle {
        AngleRestriction::None => "NONE",
        AngleRestriction::FortyFiveDegree => "FORTYFIVE_DEGREE",
        AngleRestriction::NinetyDegree => "NINETY_DEGREE",
    }
}

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

/// Java `IntPoint.toString()` is `"(" + x + "," + y + ")"`.
fn pt(p: &Point) -> String {
    match p {
        Point::Int(p) => format!("({},{})", p.x, p.y),
        other => format!("{other:?}"),
    }
}

fn b(x: &IntBox) -> String {
    format!("[{},{}..{},{}]", x.ll.x, x.ll.y, x.ur.x, x.ur.y)
}

fn shp(s: &TileShape) -> String {
    match s {
        TileShape::Box(x) => format!("Box{}", b(x)),
        TileShape::Octagon(o) => format!(
            "Oct[{},{},{},{},{},{},{},{}]",
            o.left_x,
            o.bottom_y,
            o.right_x,
            o.top_y,
            o.upper_left_diagonal_x,
            o.lower_right_diagonal_x,
            o.lower_left_diagonal_x,
            o.upper_right_diagonal_x
        ),
        TileShape::Simplex(sx) => {
            let mut out = format!("Simplex{}{{", b(&s.bounding_box()));
            for i in 0..sx.border_line_count() {
                if let Some(line) = sx.border_line(i) {
                    out.push_str(&format!("({}->{})", line.a, line.b));
                }
            }
            out.push('}');
            out
        }
    }
}

fn bounds_class(s: &TileShape) -> &'static str {
    match s {
        TileShape::Box(_) => "IntBox",
        TileShape::Octagon(_) => "IntOctagon",
        TileShape::Simplex(_) => "Simplex",
    }
}

fn nets(item: &Item) -> String {
    let list: Vec<String> = item.net_nos().iter().map(i32::to_string).collect();
    format!("[{}]", list.join(", "))
}

fn dump_items(board: &Board) {
    let ctx = board.ctx();
    for id in board.board_order() {
        let item = &board.items[&id];
        println!(
            "item id={} class={} nets={} cc={} layers={}..{} tileShapeCount={} bbox={}",
            id.0,
            class_name(item),
            nets(item),
            item.header().clearance_class(),
            item.first_layer(&ctx),
            item.last_layer(&ctx),
            item.tile_shape_count(&ctx),
            b(&item.bounding_box(&ctx))
        );
    }
}

fn dump_tree(board: &Board, label: &str, id: TreeId) {
    let tree = board.tree(id);
    println!("tree {label} key={tree} size={}", tree.size());
    for leaf in tree.tree().to_array() {
        let entry = tree.tree().leaf_entry(leaf);
        let TreeObject::Item(ItemId(obj)) = entry.object else {
            unreachable!()
        };
        let bounds = tree.tree().leaf_bounds(leaf).to_tile_shape();
        println!(
            "  leaf obj={obj} idx={} boundsClass={} bounds={}",
            entry.shape_index,
            bounds_class(&bounds),
            shp(&bounds)
        );
    }
    for item_id in board.board_order() {
        let item = &board.items[&item_id];
        let n = item.tree_shape_count(id);
        let mut line = String::new();
        for i in 0..n {
            line.push_str(&format!(
                " [{i}]={}",
                item.get_tree_shape(id, i).map_or("null".to_string(), shp)
            ));
        }
        println!("  shapes id={} n={n}{line}", item_id.0);
    }
}

fn query(
    board: &mut Board,
    label: &str,
    id: TreeId,
    shape: &TileShape,
    layer: Option<usize>,
    ignore: &[i32],
    cc: usize,
) {
    let layer_text = layer.map_or(-1, |l| l as i32);
    let ignore_text: Vec<String> = ignore.iter().map(i32::to_string).collect();
    println!(
        "query tree={label} shape={} layer={layer_text} ignore=[{}] cc={cc}",
        shp(shape),
        ignore_text.join(", ")
    );
    let items = std::mem::take(&mut board.items);
    let ctx = ItemCtx {
        library: &board.library,
        components: &board.components,
        rules: &board.rules,
        bounding_box: &board.bounding_box,
        max_tree_shape_width: DEFAULT_MAX_TREE_SHAPE_WIDTH,
    };
    let tree = board
        .manager
        .trees()
        .find(|tree| tree.id() == id)
        .expect("tree exists");
    let mut counter = 0;

    let objects = tree.overlapping_objects(shape, layer, ignore, &items, &ctx);
    let mut line = String::from("  overlappingObjects:");
    for object in &objects {
        if let TreeObject::Item(ItemId(id)) = object {
            line.push_str(&format!(" {id}"));
        }
    }
    println!("{line}");

    let entries = tree.overlapping_tree_entries(shape, layer, ignore, &items, &ctx);
    println!("  overlappingTreeEntries:{}", pair_text(&entries));

    let with_clearance = tree.overlapping_tree_entries_with_clearance(
        shape,
        layer,
        ignore,
        cc,
        &items,
        &ctx,
        &mut counter,
    );
    println!(
        "  overlappingTreeEntriesWithClearance:{}",
        pair_text(&with_clearance)
    );

    let item_ids =
        tree.overlapping_items_with_clearance(shape, layer, ignore, cc, &items, &ctx, &mut counter);
    let mut line = String::from("  overlappingItemsWithClearance:");
    for ItemId(id) in item_ids {
        line.push_str(&format!(" {id}"));
    }
    println!("{line}");
    board.items = items;
}

fn pair_text(entries: &[TreeEntry<TreeObject>]) -> String {
    let mut out = String::new();
    for entry in entries {
        if let TreeObject::Item(ItemId(id)) = entry.object {
            out.push_str(&format!(" {id}/{}", entry.shape_index));
        }
    }
    out
}

fn dump(board: &mut Board, mode: i32) {
    println!(
        "mode={mode} angle={}",
        angle_name(board.rules.trace_angle_restriction)
    );
    dump_items(board);
    let default_id = board.manager.get_default_tree().id();
    dump_tree(board, "default", default_id);
    let auto1 = board.autoroute_tree(1);
    dump_tree(board, "autoroute_cc1", auto1);
    let auto2 = board.autoroute_tree(2);
    dump_tree(board, "autoroute_cc2", auto2);

    let probe = TileShape::Box(IntBox::from_coords(-600, -100, 600, 500));
    query(board, "default", default_id, &probe, Some(0), &[], 1);
    query(board, "default", default_id, &probe, Some(0), &[1], 1);
    query(board, "default", default_id, &probe, None, &[], 1);
    query(board, "default", default_id, &probe, Some(1), &[], 1);
    query(board, "default", default_id, &probe, Some(0), &[], 2);
    query(board, "autoroute_cc1", auto1, &probe, Some(0), &[], 1);

    let small = TileShape::Box(IntBox::from_coords(-520, -20, -480, 20));
    query(board, "default", default_id, &small, Some(0), &[], 1);
    query(board, "default", default_id, &small, Some(0), &[1], 1);
}

fn mutate(board: &mut Board) {
    let class_count = board.rules.clearance_matrix.get_class_count();
    println!("matrix classCount={class_count}");
    for i in 0..class_count {
        let mut line = format!("  getValue({i},j,0,false):");
        for j in 0..class_count {
            line.push_str(&format!(
                " {}",
                board.rules.clearance_matrix.get_value(i, j, 0, false)
            ));
        }
        line.push_str(" | withMargin:");
        for j in 0..class_count {
            line.push_str(&format!(
                " {}",
                board.rules.clearance_matrix.get_value(i, j, 0, true)
            ));
        }
        line.push_str(&format!(
            " | maxValue={}",
            board.rules.clearance_matrix.max_value(i, 0)
        ));
        line.push_str(&format!(
            " | compensation={}",
            board.rules.clearance_matrix.clearance_compensation_value(i, 0)
        ));
        println!("{line}");
    }

    let default_id = board.manager.get_default_tree().id();
    let auto1 = board.autoroute_tree(1);
    let auto2 = board.autoroute_tree(2);
    for id in [default_id, auto1, auto2] {
        for cc in 0..=2 {
            println!(
                "clearanceCompensationValue tree={} cc={cc} layer0={}",
                board.tree(id),
                board
                    .tree(id)
                    .clearance_compensation_value(cc, 0, &board.rules)
            );
        }
    }

    let trace = match &board.items[&ItemId(4)] {
        Item::Trace(trace) => trace.clone(),
        _ => unreachable!(),
    };
    println!(
        "validateEntries(4)={}",
        board.tree(default_id).validate_entries(&board.items[&ItemId(4)])
    );
    println!(
        "compensatedHalfWidth(4, default)={}",
        board
            .tree(default_id)
            .compensated_half_width(&trace, &board.rules)
    );
    println!(
        "compensatedHalfWidth(4, auto1)={}",
        board.tree(auto1).compensated_half_width(&trace, &board.rules)
    );

    println!("--- changeItemShape(trace 4, 1, Box[-20,-20..20,420])");
    let mut item = board.items.remove(&ItemId(4)).expect("item 4");
    board.manager.get_default_tree_mut().change_item_shape(
        &mut item,
        1,
        TileShape::Box(IntBox::from_coords(-20, -20, 20, 420)),
    );
    board.items.insert(ItemId(4), item);
    dump_tree(board, "default_after_changeItemShape", default_id);

    println!("--- changeEntries(trace 4, shifted polyline, keepStart=1, keepEnd=1)");
    let shifted = Polyline::from_points(&[
        Point::new(-500, 0),
        Point::new(0, 0),
        Point::new(0, 600),
        Point::new(500, 600),
    ]);
    let mut item = board.items.remove(&ItemId(4)).expect("item 4");
    {
        let Item::Trace(trace) = &mut item else {
            unreachable!()
        };
        let rules = &board.rules;
        board
            .manager
            .get_default_tree_mut()
            .change_entries(trace, &shifted, 1, 1, rules);
    }
    board.items.insert(ItemId(4), item);
    dump_tree(board, "default_after_changeEntries", default_id);
    println!(
        "validateEntries(4)={}",
        board.tree(default_id).validate_entries(&board.items[&ItemId(4)])
    );

    println!("--- setClearanceCompensationUsed(true)");
    let mut items = std::mem::take(&mut board.items);
    let ctx = ItemCtx {
        library: &board.library,
        components: &board.components,
        rules: &board.rules,
        bounding_box: &board.bounding_box,
        max_tree_shape_width: DEFAULT_MAX_TREE_SHAPE_WIDTH,
    };
    let mut refs: Vec<&mut Item> = items.values_mut().rev().collect();
    board
        .manager
        .set_clearance_compensation_used(true, &mut refs, &ctx);
    drop(refs);
    board.items = items;
    println!(
        "isClearanceCompensationUsed={}",
        board.manager.is_clearance_compensation_used()
    );
    let compensated = board.manager.get_default_tree().id();
    dump_tree(board, "default_compensated", compensated);
    let probe = TileShape::Box(IntBox::from_coords(-600, -100, 600, 500));
    query(
        board,
        "default_compensated",
        compensated,
        &probe,
        Some(0),
        &[],
        1,
    );
}

fn main() {
    // Silence the unused import in builds where no Simplex is printed.
    let _ = Simplex::EMPTY;
    let mode: i32 = std::env::args()
        .nth(1)
        .map_or(0, |a| a.parse().expect("mode is an integer"));
    if mode == 4 {
        let mut board = build_areas();
        dump_areas(&mut board);
        return;
    }
    if mode == 5 {
        let mut board = build_traces();
        dump_traces(&mut board);
        return;
    }
    if mode == 6 {
        let mut board = build(0);
        dump_tie_pin(&mut board);
        return;
    }
    if mode == 8 {
        dump_change_order_merges();
        return;
    }
    if mode == 7 {
        let mut board = build_skewed_outline();
        println!("mode=7");
        let default_id = board.manager.get_default_tree().id();
        dump_tree(&board, "default", default_id);
        let auto = board.autoroute_tree(1);
        dump_tree(&board, "autoroute_cc1", auto);
        return;
    }
    let mut board = build(mode);
    if mode == 3 {
        mutate(&mut board);
    } else {
        dump(&mut board, mode);
    }
}
