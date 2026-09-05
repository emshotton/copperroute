//! Rust twin of `../../java/P2T3.java`: the fixed 8-box `ShapeTree`/`MinAreaTree` script whose
//! output `crates/fr-board/tests/min_area_tree.rs` was written from.
//!
//! No arguments. The two programs must print byte-identical stdout.

use fr_board::datastructures::{LeafId, Node, NodeId, ShapeTree};
use fr_board::ids::{ItemId, TreeObject};
use fr_geometry::bounding_directions::ShapeBoundingDirections;
use fr_geometry::int_box::IntBox;
use fr_geometry::int_point::IntPoint;
use fr_geometry::regular_tile_shape::RegularTileShape;
use fr_geometry::tile_shape::TileShape;

fn obj(n: u32) -> TreeObject {
    TreeObject::Item(ItemId(n))
}

fn obj_str(o: &TreeObject) -> String {
    match o {
        TreeObject::Item(ItemId(n)) => format!("#{n}"),
        other => format!("{other:?}"),
    }
}

fn tile(c: [i32; 4]) -> TileShape {
    TileShape::Box(IntBox::from_coords(c[0], c[1], c[2], c[3]))
}

fn bounds_str(s: &RegularTileShape) -> String {
    let b = s.bounding_box();
    format!("({},{},{},{})", b.ll.x, b.ll.y, b.ur.x, b.ur.y)
}

fn dump(tree: &ShapeTree<TreeObject>, id: NodeId, prefix: &str, out: &mut String) {
    match tree.node(id) {
        Node::Leaf {
            bounds,
            object,
            shape_index,
            ..
        } => out.push_str(&format!(
            "{prefix}Leaf {}/{} {}\n",
            obj_str(object),
            shape_index,
            bounds_str(bounds)
        )),
        Node::Inner {
            bounds,
            first_child,
            second_child,
            ..
        } => {
            out.push_str(&format!("{prefix}Inner {}\n", bounds_str(bounds)));
            let deeper = format!("{prefix}  ");
            dump(tree, *first_child, &deeper, out);
            dump(tree, *second_child, &deeper, out);
        }
        Node::Free { .. } => unreachable!("a reachable node is never a free slot"),
    }
}

fn print_tree(tree: &ShapeTree<TreeObject>, label: &str) {
    println!("--- {label} leafCount={}", tree.leaf_count());
    match tree.root() {
        None => println!("(empty)"),
        Some(root) => {
            let mut s = String::new();
            dump(tree, root, "", &mut s);
            print!("{s}");
        }
    }
}

fn print_overlaps(tree: &ShapeTree<TreeObject>, query: [i32; 4], label: &str) {
    let q = tree
        .bounding_shape(&tile(query))
        .expect("a box is always boundable");
    let mut s = format!("{label} -> ");
    for e in &tree.overlaps(&q) {
        s.push_str(&format!("{}/{} ", obj_str(&e.object), e.shape_index));
    }
    println!("{}", s.trim_end());
}

/// One shape per object, inserted through `insert_tiles` so the tree applies its own bounding
/// directions — the port of Java's `insert(Storable, int)`.
fn insert_one(tree: &mut ShapeTree<TreeObject>, id: u32, c: [i32; 4]) -> LeafId {
    tree.insert_tiles(obj(id), &[tile(c)])[0].expect("a box is always boundable")
}

fn main() {
    eight_boxes();
    tie_goes_to_first_child();
    edge_cases();
}

fn eight_boxes() {
    let mut tree = ShapeTree::new(ShapeBoundingDirections::Orthogonal);

    let boxes: [[i32; 4]; 8] = [
        [0, 0, 10, 10],
        [20, 0, 30, 10],
        [0, 20, 10, 30],
        [20, 20, 30, 30],
        [40, 0, 50, 10],
        [40, 20, 50, 30],
        [5, 5, 15, 15],
        [60, 60, 70, 70],
    ];

    let mut leaves = Vec::new();
    for (i, c) in boxes.into_iter().enumerate() {
        leaves.push(insert_one(&mut tree, i as u32 + 1, c));
        print_tree(&tree, &format!("after inserting #{}", i + 1));
    }

    print_overlaps(&tree, [0, 0, 10, 10], "Q1 (0,0,10,10)");
    print_overlaps(&tree, [15, 15, 45, 45], "Q2 (15,15,45,45)");
    print_overlaps(&tree, [55, 55, 65, 65], "Q3 (55,55,65,65)");
    print_overlaps(&tree, [200, 200, 210, 210], "Q4 (200,200,210,210)");
    print_overlaps(&tree, [-5, -5, 100, 100], "Q5 all");

    for (i, leaf) in leaves.iter().enumerate() {
        println!(
            "distanceToRoot #{} = {}",
            i + 1,
            tree.distance_to_root(*leaf)
        );
    }

    tree.remove_leaf(leaves[6]);
    print_tree(&tree, "after removing #7");
    print_overlaps(&tree, [0, 0, 10, 10], "Q1' (0,0,10,10)");
    print_overlaps(&tree, [-5, -5, 100, 100], "Q5' all");

    tree.remove_leaf(leaves[7]);
    print_tree(&tree, "after removing #8");

    let mut ta = String::from("toArray:");
    for l in tree.to_array() {
        let e = tree.leaf_entry(l);
        ta.push_str(&format!(" {}/{}", obj_str(&e.object), e.shape_index));
    }
    println!("{ta}");

    for (i, leaf) in leaves.iter().take(6).enumerate() {
        tree.remove_leaf(*leaf);
        println!(
            "after removing #{} leafCount={} rootNull={}",
            i + 1,
            tree.leaf_count(),
            tree.root().is_none()
        );
    }

    // A multi-shape object: `insert_tiles` makes one leaf per shape index; the BTreeSet then
    // orders (object, shape_index).
    let mut t2 = ShapeTree::new(ShapeBoundingDirections::Orthogonal);
    t2.insert_tiles(obj(2), &[tile([0, 0, 10, 10]), tile([100, 100, 110, 110])]);
    t2.insert_tiles(obj(1), &[tile([5, 5, 15, 15])]);
    print_overlaps(&t2, [-5, -5, 200, 200], "Q6 multi-shape");
}

fn tie_goes_to_first_child() {
    let mut tree = ShapeTree::new(ShapeBoundingDirections::Orthogonal);
    insert_one(&mut tree, 1, [0, 0, 10, 10]);
    insert_one(&mut tree, 2, [20, 20, 30, 30]);
    insert_one(&mut tree, 3, [10, 10, 20, 20]);
    print_tree(&tree, "tie tree");
}

fn edge_cases() {
    let mut t2: ShapeTree<TreeObject> = ShapeTree::new(ShapeBoundingDirections::Orthogonal);
    t2.insert_tiles(obj(1), &[]);
    println!(
        "empty insert leafCount={} rootNull={}",
        t2.leaf_count(),
        t2.root().is_none()
    );

    println!(
        "touching intersects = {}",
        tile([0, 0, 10, 10]).intersects(&tile([10, 0, 20, 10]))
    );

    let mut t3 = ShapeTree::new(ShapeBoundingDirections::Orthogonal);
    let only = insert_one(&mut t3, 9, [0, 0, 1, 1]);
    // Java bug: `Leaf.distanceToRoot` NPEs on the leaf of a one-element tree
    // (ShapeTree.java:228-229). The port panics; catch it so the two sides print the same line.
    // The hook is silenced first so the expected panic does not spam stderr.
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let panicked =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| t3.distance_to_root(only)));
    std::panic::set_hook(previous_hook);
    match panicked {
        Ok(d) => println!("root leaf distanceToRoot = {d}"),
        Err(_) => println!("root leaf distanceToRoot threw java.lang.NullPointerException"),
    }
    t3.remove_leaf(only);
    println!(
        "single removeLeaf leafCount={} rootNull={}",
        t3.leaf_count(),
        t3.root().is_none()
    );
    // Java `remove(null)` (ShapeTree.java:98-100) is a no-op; the port's holed-array form is an
    // all-`None` slice.
    t3.remove_opt(&[None]);
    println!("remove(null) ok, leafCount={}", t3.leaf_count());

    // Not in the Java driver (it has no way to express it): `insert_tiles` on a shape with no
    // bound leaves a hole exactly where Java's `leafArr` keeps a null (ShapeTree.java:51-55).
    let mut t4 = ShapeTree::new(ShapeBoundingDirections::FortyfiveDegree);
    let unbounded = TileShape::Simplex(fr_geometry::simplex::Simplex::from_points(&[
        IntPoint::new(0, 0),
        IntPoint::new(10, 0),
    ]));
    let entries = t4.insert_tiles(obj(1), &[tile([0, 0, 10, 10]), unbounded]);
    eprintln!(
        "rust-only: unboundable shape -> {:?}, leafCount={}",
        entries[1],
        t4.leaf_count()
    );
}
