//! Rust twin of `scripts/differential/java/P2T15.java` (Plan 2 Task 15).
//!
//! Builds the same randomised two-layer board through `fr_board::Board`, drives the same shared
//! xorshift stream through the same insertion mix, and prints the same lines the Java driver
//! prints. Every random draw below is mirrored draw-for-draw by the Java side; the two programs
//! must produce byte-identical stdout (see `scripts/differential/README.md` for the recorded
//! baseline and the one documented, harness-wide cosmetic exception this driver never triggers,
//! since it never prints `toArray()`).
//!
//! args: seed n. `run.sh p2t15 <seed> <n>` diffs this against `P2T15.java`.

use std::collections::BTreeSet;

use fr_board::prelude::*;
use fr_geometry::{Area, IntBox, IntVector, Point, PolygonShape, Polyline, PolylineShapeRef, Shape, TileShape};

const RANGE: i32 = 12_000;
const QUERY_RANGE: i32 = 15_000;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    /// Java `(int) Long.remainderUnsigned(next(), bound)`.
    fn rnd(&mut self, bound: u64) -> i64 {
        (self.next() % bound) as i64
    }
}

fn rand_coord(rng: &mut Rng, range: i32) -> i32 {
    (rng.rnd((2 * range + 1) as u64) as i32) - range
}

fn random_point(rng: &mut Rng, range: i32) -> Point {
    Point::new(rand_coord(rng, range), rand_coord(rng, range))
}

fn random_box(rng: &mut Rng, range: i32, min_size: i32, max_size: i32) -> IntBox {
    let w = min_size + rng.rnd((max_size - min_size + 1) as u64) as i32;
    let h = min_size + rng.rnd((max_size - min_size + 1) as u64) as i32;
    let x = rand_coord(rng, range);
    let y = rand_coord(rng, range);
    IntBox::from_coords(x, y, x + w, y + h)
}

fn main() {
    let mut args = std::env::args().skip(1);
    let seed: i64 = args.next().map_or(42, |a| a.parse().expect("seed"));
    let n: usize = args.next().map_or(30, |a| a.parse().expect("n"));
    let mut rng = Rng(if seed == 0 {
        0x9E3779B97F4A7C15u64
    } else {
        seed as u64
    });

    let (mut board, thru_pad, pkg) = build();

    for _ in 0..n {
        let roll = rng.rnd(100);
        if roll < 40 {
            insert_random_pin(&mut board, &mut rng, pkg);
        } else if roll < 70 {
            insert_random_via(&mut board, &mut rng, thru_pad);
        } else {
            insert_random_trace(&mut board, &mut rng);
        }
    }
    for _ in 0..3 {
        insert_random_obstacle(&mut board, &mut rng);
    }
    for _ in 0..3 {
        insert_random_conduction(&mut board, &mut rng);
    }

    let normalized = board.normalize_all_traces().expect("normalizeAllTraces");

    println!("mode=p2t15 seed={seed} n={n}");
    println!("normalizeAllTraces={normalized}");
    println!("itemCount={}", board.items.len());

    dump_items(&mut board);

    let mut overlap_queries: Vec<(IntBox, usize)> = Vec::new();
    for layer in 0..2usize {
        for _ in 0..50 {
            overlap_queries.push((random_box(&mut rng, QUERY_RANGE, 50, 3000), layer));
        }
    }
    dump_overlap_queries("overlap", &board, &overlap_queries);

    let mut clearance_queries: Vec<(IntBox, usize, usize, Vec<i32>)> = Vec::new();
    for _ in 0..50 {
        let query_box = random_box(&mut rng, QUERY_RANGE, 50, 3000);
        let layer = rng.rnd(2) as usize;
        let cc = (1 + rng.rnd(2)) as usize;
        let ignore = if rng.rnd(4) == 0 {
            vec![1 + rng.rnd(4) as i32]
        } else {
            Vec::new()
        };
        clearance_queries.push((query_box, layer, cc, ignore));
    }
    dump_clearance_queries(&mut board, "clearance", &clearance_queries);

    println!("--- deepCopy");
    let mut copy = board.deep_copy();
    dump_overlap_queries("copyOverlap", &copy, &overlap_queries);
    dump_clearance_queries(&mut copy, "copyClearance", &clearance_queries);
    println!(
        "hashEqual={}",
        board.structural_hash() == copy.structural_hash()
    );
}

// ---------------------------------------------------------------------------------------------
// The board
// ---------------------------------------------------------------------------------------------

fn layers() -> LayerStructure {
    LayerStructure::new(vec![Layer::new("front", true), Layer::new("back", true)])
}

fn build() -> (Board, PadstackId, usize) {
    let ls = layers();
    let mut cm = ClearanceMatrix::get_default_instance(&ls, 200);
    assert!(cm.append_class("wide"));
    cm.set_value_on_all_layers(2, 1, 600);
    cm.set_value_on_all_layers(2, 2, 800);
    let mut rules = BoardRules::new(layers(), cm);
    rules.create_default_net_class();
    let default_class = rules.get_default_net_class();

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
    let components = Components::new();

    let outline = vec![PolylineShapeRef::Polygon(PolygonShape::from_points(&[
        Point::new(-15000, -15000),
        Point::new(15000, -15000),
        Point::new(15000, 15000),
        Point::new(-15000, 15000),
    ]))];
    let mut board = Board::new(
        outline,
        1,
        IntBox::from_coords(-20_000, -20_000, 20_000, 20_000),
        rules,
        library,
        components,
        Communication::default(),
    );
    // `Nets.add` reads `netList.getBoard().rules` (Net.java:50) on the Java side, so the Java
    // driver creates the nets after the board; the port has no back-pointer, so the order is
    // free — kept the same for symmetry with `P2T15.java`.
    board.rules.nets.add("N1", 1, false, default_class);
    board.rules.nets.add("N2", 1, false, default_class);
    board.rules.nets.add("N3", 1, false, default_class);
    board.rules.nets.add("N4", 1, false, default_class);

    (board, thru_pad, pkg)
}

// ---------------------------------------------------------------------------------------------
// Random item insertion — every draw below is mirrored draw-for-draw by `P2T15.java`.
// ---------------------------------------------------------------------------------------------

fn insert_random_pin(board: &mut Board, rng: &mut Rng, pkg: usize) {
    let loc = random_point(rng, RANGE);
    let rotation = 90.0 * rng.rnd(4) as f64;
    let on_front = rng.rnd(2) == 0;
    let comp_id = board
        .components
        .add_with_generated_name(Some(loc), rotation, on_front, pkg)
        .id;
    let pin_index = rng.rnd(2) as i32;
    let net_no = 1 + rng.rnd(4) as i32;
    let cc = (1 + rng.rnd(2)) as usize;
    board.insert_pin(comp_id, pin_index, vec![net_no], cc, FixedState::Unfixed);
}

fn insert_random_via(board: &mut Board, rng: &mut Rng, thru_pad: PadstackId) {
    let center = random_point(rng, RANGE);
    let net_no = 1 + rng.rnd(4) as i32;
    let cc = (1 + rng.rnd(2)) as usize;
    let attach = rng.rnd(2) == 0;
    board
        .insert_via(thru_pad, center, vec![net_no], cc, FixedState::Unfixed, attach)
        .expect("insertVia");
}

fn insert_random_trace(board: &mut Board, rng: &mut Rng) {
    let layer = rng.rnd(2) as usize;
    let (x1, y1) = (rand_coord(rng, RANGE), rand_coord(rng, RANGE));
    let (mut x2, mut y2) = (rand_coord(rng, RANGE), rand_coord(rng, RANGE));
    while x2 == x1 && y2 == y1 {
        x2 = rand_coord(rng, RANGE);
        y2 = rand_coord(rng, RANGE);
    }
    let half_width = 5 + rng.rnd(30) as i32;
    let net_no = 1 + rng.rnd(4) as i32;
    let cc = (1 + rng.rnd(2)) as usize;
    board.insert_trace_without_cleaning(
        Polyline::from_points(&[Point::new(x1, y1), Point::new(x2, y2)]),
        layer,
        half_width,
        vec![net_no],
        cc,
        FixedState::Unfixed,
    );
}

fn insert_random_obstacle(board: &mut Board, rng: &mut Rng) {
    let query_box = random_box(rng, RANGE, 200, 2000);
    let layer = rng.rnd(2) as usize;
    let cc = (1 + rng.rnd(2)) as usize;
    board.insert_obstacle(
        Area::Shape(Shape::Tile(TileShape::Box(query_box))),
        layer,
        cc,
        FixedState::Unfixed,
    );
}

fn insert_random_conduction(board: &mut Board, rng: &mut Rng) {
    let query_box = random_box(rng, RANGE, 200, 2000);
    let layer = rng.rnd(2) as usize;
    let net_no = 1 + rng.rnd(4) as i32;
    let cc = (1 + rng.rnd(2)) as usize;
    let is_obstacle = rng.rnd(2) == 0;
    board.insert_conduction_area(
        Area::Shape(Shape::Tile(TileShape::Box(query_box))),
        layer,
        vec![net_no],
        cc,
        is_obstacle,
        FixedState::Unfixed,
    );
}

// ---------------------------------------------------------------------------------------------
// Dumping
// ---------------------------------------------------------------------------------------------

/// Every item, in `Board::get_items()` (board) order — descending id, quirk #63.
fn dump_items(board: &mut Board) {
    for id in board.items_in_board_order() {
        let (kind, first_layer, last_layer, nets, cl, bbox, tile_count) = {
            let ctx = board.ctx();
            let item = board.items.get(&id).expect("item exists");
            (
                class_name(item),
                item.first_layer(&ctx),
                item.last_layer(&ctx),
                item.net_nos().to_vec(),
                item.clearance_class(),
                item.bounding_box(&ctx),
                item.tile_shape_count(&ctx),
            )
        };
        let nets_s = net_array(&nets);
        let bbox_s = boxs(&bbox);
        println!(
            "item id={id} kind={kind} layers={first_layer}..{last_layer} nets={nets_s} cl={cl} bbox={bbox_s} tiles={tile_count}"
        );
        for i in 0..tile_count {
            match board.item_tile_shape(id, i) {
                Some(shape) => println!("  tile[{i}]={}", boxs(&shape.bounding_box())),
                None => println!("  tile[{i}]=null"),
            }
        }
    }
}

fn dump_overlap_queries(label: &str, board: &Board, specs: &[(IntBox, usize)]) {
    for (query_box, layer) in specs {
        let shape = TileShape::Box(*query_box);
        let result = board.overlapping_objects(&shape, Some(*layer));
        println!(
            "{label} box={} layer={layer} ids={}",
            boxs(query_box),
            object_ids(&result)
        );
    }
}

/// The board-level equivalent of Java's `board.searchTreeManager.getDefaultTree()
/// .overlappingTreeEntriesWithClearance(...)`: `ShapeSearchTree::overlapping_tree_entries_with_clearance`
/// needs the counter snapshot/write-back dance `Board`'s own (private) wrapper does — see
/// `board/query.rs`'s module doc, "Why several of these take `&mut self`".
fn clearance_query(
    board: &mut Board,
    shape: &TileShape,
    layer: Option<usize>,
    ignore: &[i32],
    cc: usize,
) -> Vec<TreeEntry<TreeObject>> {
    let mut counter = board.trees.entry_counter();
    let result = {
        let ctx = board.ctx();
        board
            .trees
            .get_default_tree()
            .overlapping_tree_entries_with_clearance(shape, layer, ignore, cc, &board.items, &ctx, &mut counter)
    };
    *board.trees.entry_counter_mut() = counter;
    result
}

fn dump_clearance_queries(board: &mut Board, label: &str, specs: &[(IntBox, usize, usize, Vec<i32>)]) {
    for (query_box, layer, cc, ignore) in specs {
        let shape = TileShape::Box(*query_box);
        let entries = clearance_query(board, &shape, Some(*layer), ignore, *cc);
        println!(
            "{label} box={} layer={layer} cc={cc} ignore={} entries={}",
            boxs(query_box),
            net_array(ignore),
            pair_text(&entries)
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Formatting helpers — every one of these has an exact twin in the Java driver.
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

fn object_ids(objects: &BTreeSet<TreeObject>) -> String {
    format!(
        "[{}]",
        objects
            .iter()
            .map(|o| match o {
                TreeObject::Item(ItemId(id)) => id.to_string(),
                TreeObject::Room(_) => unreachable!("no expansion rooms in Plan 2"),
            })
            .collect::<Vec<_>>()
            .join(" ")
    )
}

fn pair_text(entries: &[TreeEntry<TreeObject>]) -> String {
    format!(
        "[{}]",
        entries
            .iter()
            .filter_map(|e| match e.object {
                TreeObject::Item(ItemId(id)) => Some(format!("{id}/{}", e.shape_index)),
                TreeObject::Room(_) => None,
            })
            .collect::<Vec<_>>()
            .join(" ")
    )
}
