//! Rust twin of `scripts/differential/java/P6T2.java` (Plan 6 Task 3).
//!
//! `AutorouteSearchTreeExt::{complete_shape, divide_large_room}` in all three angle regimes —
//! the two `ShapeSearchTree` methods `p2t10` explicitly skipped.
//!
//! It builds the same board `P2T10.java`/`p2t10.rs` build (two layers, a two-pin component, two
//! traces, an empty outline) plus `n` random obstacle areas from the shared xorshift stream,
//! seeds the autoroute tree with three complete expansion rooms, and prints every room
//! `complete_shape` and `divide_large_room` answer for `rooms` random seed rooms.
//!
//! args: `seed n rooms`.

use std::collections::BTreeMap;

use fr_board::ids::TreeObject;
use fr_board::prelude::*;
use fr_dsn::format::java_double_to_string;
use fr_geometry::{
    Area, IntBox, IntOctagon, IntPoint, IntVector, Point, Polyline, Shape, TileShape, Vector,
};
use fr_router::autoroute::expansion::{ExpansionRoomStore, IncompleteFreeSpaceExpansionRoom};
use fr_router::autoroute::tree_ext::AutorouteSearchTreeExt;

const RANGE: i32 = 9000;
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

// -------------------------------------------------------------------------------------------
// The shared xorshift stream
// -------------------------------------------------------------------------------------------

struct Rng {
    state: u64,
}

impl Rng {
    fn new(seed: i64) -> Rng {
        Rng {
            state: if seed == 0 {
                0x9E37_79B9_7F4A_7C15
            } else {
                seed as u64
            },
        }
    }

    fn next(&mut self) -> u64 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;
        self.state
    }

    fn rnd(&mut self, bound: i32) -> i32 {
        (self.next() % bound as u64) as i32
    }

    fn coord(&mut self, range: i32) -> i32 {
        self.rnd(2 * range + 1) - range
    }

    fn box_(&mut self, range: i32, min_size: i32, max_size: i32) -> IntBox {
        let w = min_size + self.rnd(max_size - min_size + 1);
        let h = min_size + self.rnd(max_size - min_size + 1);
        let x = self.coord(range);
        let y = self.coord(range);
        IntBox::from_coords(x, y, x + w, y + h)
    }

    /// A box with its four corners clipped by independent random amounts, then normalized.
    fn octagon(&mut self, b: &IntBox) -> IntOctagon {
        let d1 = self.rnd(300);
        let d2 = self.rnd(300);
        let d3 = self.rnd(300);
        let d4 = self.rnd(300);
        IntOctagon::new(
            b.ll.x,
            b.ll.y,
            b.ur.x,
            b.ur.y,
            b.ll.x - b.ur.y + d1,
            b.ur.x - b.ll.y - d2,
            b.ll.x + b.ll.y + d3,
            b.ur.x + b.ur.y - d4,
        )
        .normalize()
    }

    /// A small box inside `outer`, or a degenerate one at its lower-left corner if it is tiny.
    fn inside(&mut self, outer: &IntBox) -> IntBox {
        let w = 10 + self.rnd(300);
        let h = 10 + self.rnd(300);
        let span_x = (outer.ur.x - outer.ll.x - w).max(1);
        let span_y = (outer.ur.y - outer.ll.y - h).max(1);
        let x = outer.ll.x + self.rnd(span_x);
        let y = outer.ll.y + self.rnd(span_y);
        IntBox::from_coords(x, y, x + w, y + h)
    }
}

// -------------------------------------------------------------------------------------------
// The board
// -------------------------------------------------------------------------------------------

struct Board {
    library: BoardLibrary,
    components: Components,
    rules: BoardRules,
    bounding_box: IntBox,
    items: BTreeMap<ItemId, Item>,
    manager: SearchTreeManager,
}

fn build(mode: i32, rng: &mut Rng, obstacle_count: i32) -> Board {
    let layers = || LayerStructure::new(vec![Layer::new("front", true), Layer::new("back", true)]);
    let mut clearance_matrix = ClearanceMatrix::get_default_instance(&layers(), 200);
    assert!(clearance_matrix.append_class("wide"));
    clearance_matrix.set_value_on_all_layers(2, 1, 600);
    clearance_matrix.set_value_on_all_layers(2, 2, 800);
    let mut rules = BoardRules::new(layers(), clearance_matrix);
    rules.trace_angle_restriction = match mode {
        1 => AngleRestriction::NinetyDegree,
        2 => AngleRestriction::FortyFiveDegree,
        _ => AngleRestriction::None,
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

    // `BasicBoard.insertObstacle(Area, layer, clearanceClass, FixedState)`: no nets, clearance
    // class 1, no component.
    for i in 0..obstacle_count {
        let shape = rng.box_(RANGE, 100, 2500);
        let layer = rng.rnd(2) as usize;
        let id = ItemId(6 + i as u32);
        items.insert(
            id,
            Item::ObstacleArea(ObstacleArea::new(
                ItemHeader::new(id, Vec::new(), 1, 0, FixedState::Unfixed),
                ObstacleAreaData::new(
                    Area::Shape(Shape::Tile(TileShape::Box(shape))),
                    layer,
                    Vector::ZERO,
                    0.0,
                    false,
                    None,
                ),
            )),
        );
    }

    let mut board = Board {
        library,
        components,
        rules,
        bounding_box: BOUNDING_BOX,
        items,
        manager: SearchTreeManager::new(),
    };
    let mut taken = std::mem::take(&mut board.items);
    {
        let ctx = ItemCtx {
            library: &board.library,
            components: &board.components,
            rules: &board.rules,
            bounding_box: &board.bounding_box,
            max_tree_shape_width: DEFAULT_MAX_TREE_SHAPE_WIDTH,
        };
        for item in taken.values_mut().rev() {
            board.manager.insert(item, &ctx);
        }
    }
    board.items = taken;
    board
}

// -------------------------------------------------------------------------------------------

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let seed: i64 = args.first().map_or(42, |s| s.parse().expect("seed"));
    let obstacle_count: i32 = args.get(1).map_or(20, |s| s.parse().expect("n"));
    let room_count: i32 = args.get(2).map_or(2000, |s| s.parse().expect("rooms"));

    println!("mode=p6t2 seed={seed} obstacles={obstacle_count} rooms={room_count}");

    for mode in 0..3 {
        let mut rng = Rng::new(seed);
        let mut board = build(mode, &mut rng, obstacle_count);

        let ctx_owner = (
            board.library,
            board.components,
            board.rules,
            board.bounding_box,
        );
        let (library, components, rules, bounding_box) = &ctx_owner;
        let ctx = ItemCtx {
            library,
            components,
            rules,
            bounding_box,
            max_tree_shape_width: DEFAULT_MAX_TREE_SHAPE_WIDTH,
        };
        let mut items = board.items;
        let mut manager = board.manager;

        // SearchTreeManager.getAutorouteTree(1) — the item list is walked in board order
        // (descending id, quirk #63).
        let tree_id = {
            let mut refs: Vec<&mut Item> = items.values_mut().rev().collect();
            manager.get_autoroute_tree(1, &mut refs, &ctx).id()
        };
        {
            let tree = manager
                .trees()
                .find(|tree| tree.id() == tree_id)
                .expect("the autoroute tree");
            println!(
                "regime={mode} angle={} tree={tree} size={} items={}",
                angle_name(rules.trace_angle_restriction),
                tree.size(),
                items.len()
            );
        }
        for id in items.keys().rev() {
            let item = &items[id];
            println!(
                "  item id={} class={} layers={}..{} tileShapeCount={} bbox={}",
                id.0,
                class_name(item),
                item.first_layer(&ctx),
                item.last_layer(&ctx),
                item.tile_shape_count(&ctx),
                b(&item.bounding_box(&ctx))
            );
        }

        // Three complete expansion rooms, inserted exactly as `AutorouteEngine.addCompleteRoom`
        // does (AutorouteEngine.java:534).
        let mut rooms = ExpansionRoomStore::new();
        let mut seed_rooms: Vec<RoomId> = Vec::new();
        let mut seed_room_boxes: Vec<IntBox> = Vec::new();
        {
            let tree = manager
                .trees_mut()
                .find(|tree| tree.id() == tree_id)
                .expect("the autoroute tree");
            for i in 0..3 {
                let shape = rng.box_(RANGE, 500, 3000);
                let layer = rng.rnd(2) as usize;
                let id_no = rooms.next_room_id_no();
                let room = rooms.new_complete_room(Some(TileShape::Box(shape)), layer, id_no);
                rooms.insert_complete_room(tree, room);
                seed_rooms.push(room);
                seed_room_boxes.push(shape);
                println!(
                    "  seedRoom {} layer={layer} shape={}",
                    i + 1,
                    shp(&TileShape::Box(shape))
                );
            }
            println!("  treeSizeWithRooms={}", tree.size());
        }

        let tree = manager
            .trees()
            .find(|tree| tree.id() == tree_id)
            .expect("the autoroute tree");
        for index in 0..room_count {
            run_one(
                tree,
                &items,
                &rooms,
                &seed_rooms,
                &seed_room_boxes,
                &ctx,
                &mut rng,
                mode,
                index,
            );
        }

        board.items = items;
        board.manager = manager;
        let _ = board.items.len();
    }
}

#[allow(clippy::too_many_arguments)]
fn run_one(
    tree: &ShapeSearchTree,
    items: &BTreeMap<ItemId, Item>,
    rooms: &ExpansionRoomStore,
    seed_rooms: &[RoomId],
    seed_room_boxes: &[IntBox],
    ctx: &ItemCtx<'_>,
    rng: &mut Rng,
    mode: i32,
    index: i32,
) {
    let shape_kind = rng.rnd(3);
    let shape_box = rng.box_(RANGE, 200, 6000);
    let shape_oct = rng.octagon(&shape_box);
    let inside_box = rng.inside(&shape_box);
    let loose_box = rng.box_(RANGE, 10, 400);
    let contained_box = if rng.rnd(4) == 0 {
        loose_box
    } else {
        inside_box
    };
    let layer = rng.rnd(2) as usize;
    let net_number = 1 + rng.rnd(3);
    let ignore_kind = rng.rnd(4);
    let ignore_room = rng.rnd(3) as usize;
    let ignore_shape_kind = rng.rnd(3);
    let ignore_box = rng.box_(RANGE, 100, 4000);
    // A third of the ignore shapes are one of the seed expansion rooms grown by a random margin,
    // which is what makes `ignoreShape.contains(intersection)` — the one branch of all three
    // `completeShape`s that reads `ignoreShape` at all — actually fire.
    let grow = rng.rnd(2000);
    let seed_box = seed_room_boxes[ignore_room];
    let grown_room_box = IntBox::from_coords(
        seed_box.ll.x - grow,
        seed_box.ll.y - grow,
        seed_box.ur.x + grow,
        seed_box.ur.y + grow,
    );

    let room_shape = match shape_kind {
        0 => None,
        1 => Some(TileShape::Box(shape_box)),
        _ => Some(TileShape::Octagon(shape_oct)),
    };
    let room = IncompleteFreeSpaceExpansionRoom::new(
        room_shape.clone(),
        layer,
        Some(TileShape::Box(contained_box)),
    );

    let ignore_object = match ignore_kind {
        1 => Some(TreeObject::Room(seed_rooms[ignore_room])),
        2 => Some(TreeObject::Item(ItemId(2 + ignore_room as u32))),
        _ => None,
    };
    let ignore_shape = match ignore_shape_kind {
        0 => None,
        1 => Some(TileShape::Box(ignore_box)),
        _ => Some(TileShape::Box(grown_room_box)),
    };

    println!(
        "call regime={mode} i={index} layer={layer} net={net_number} shapeKind={shape_kind} \
         shape={} contained={} ignoreObject={} ignoreShape={}",
        opt_shp(room_shape.as_ref()),
        shp(&TileShape::Box(contained_box)),
        describe_ignore(ignore_object, rooms),
        opt_shp(ignore_shape.as_ref())
    );

    let completed = tree.complete_shape(
        &room,
        net_number,
        ignore_object,
        ignore_shape.as_ref(),
        items,
        rooms,
        ctx,
    );
    dump_rooms("  completeShape", &completed);
    let divided = tree.divide_large_room(completed, ctx.bounding_box);
    dump_rooms("  divideLargeRoom", &divided);
}

fn describe_ignore(object: Option<TreeObject>, rooms: &ExpansionRoomStore) -> String {
    match object {
        None => "null".to_string(),
        Some(TreeObject::Item(id)) => format!("item{}", id.0),
        Some(TreeObject::Room(id)) => format!(
            "room{}",
            rooms.complete_room(id).expect("a live room").get_id()
        ),
    }
}

fn dump_rooms(label: &str, rooms: &[IncompleteFreeSpaceExpansionRoom]) {
    println!("{label} n={}", rooms.len());
    for (i, room) in rooms.iter().enumerate() {
        let shape = room.get_shape();
        let contained = room.get_contained_shape();
        println!(
            "    [{i}] layer={} dim={} shape={} corners={} contained={} containedCorners={}",
            room.get_layer(),
            shape.map_or("null".to_string(), |s| s.dimension().to_string()),
            opt_shp(shape),
            corners(shape),
            opt_shp(contained),
            corners(contained)
        );
    }
}

// -------------------------------------------------------------------------------------------
// Formatting, byte-for-byte with `P6T2.java`
// -------------------------------------------------------------------------------------------

fn corners(shape: Option<&TileShape>) -> String {
    let Some(shape) = shape else {
        return "null".to_string();
    };
    let mut out = String::from("(");
    for (i, corner) in shape.corner_approx_arr().iter().enumerate() {
        if i > 0 {
            out.push(';');
        }
        out.push_str(&java_double_to_string(corner.x));
        out.push(',');
        out.push_str(&java_double_to_string(corner.y));
    }
    out.push(')');
    out
}

fn b(x: &IntBox) -> String {
    format!("[{},{}..{},{}]", x.ll.x, x.ll.y, x.ur.x, x.ur.y)
}

fn opt_shp(s: Option<&TileShape>) -> String {
    s.map_or("null".to_string(), shp)
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
            let mut out = String::from("Simplex{");
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
