//! Rust twin of `../../java/P2T3R.java`: randomised differential driver for
//! `fr_board::datastructures::ShapeTree` against Java's `ShapeTree`/`MinAreaTree`.
//!
//! args: ops seed mode(0=orthogonal,1=45-degree) dumpEvery insertPct
//!
//! Every random draw is mirrored draw-for-draw by the Java side; the two programs must print
//! byte-identical stdout.

use fr_board::datastructures::{LeafId, Node, NodeId, ShapeTree};
use fr_board::ids::{ItemId, TreeObject};
use fr_geometry::bounding_directions::ShapeBoundingDirections;
use fr_geometry::int_box::IntBox;
use fr_geometry::regular_tile_shape::RegularTileShape;
use fr_geometry::tile_shape::TileShape;

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

fn bounds_str(s: &RegularTileShape) -> String {
    match s {
        RegularTileShape::Box(b) => format!("B({},{},{},{})", b.ll.x, b.ll.y, b.ur.x, b.ur.y),
        RegularTileShape::Octagon(o) => format!(
            "O({},{},{},{},{},{},{},{})",
            o.left_x,
            o.bottom_y,
            o.right_x,
            o.top_y,
            o.upper_left_diagonal_x,
            o.lower_right_diagonal_x,
            o.lower_left_diagonal_x,
            o.upper_right_diagonal_x
        ),
    }
}

fn obj_str(o: &TreeObject) -> String {
    match o {
        TreeObject::Item(ItemId(n)) => format!("#{n}"),
        other => format!("{other:?}"),
    }
}

fn key(tree: &ShapeTree<TreeObject>, leaf: LeafId) -> String {
    let e = tree.leaf_entry(leaf);
    format!("{}/{}", obj_str(&e.object), e.shape_index)
}

fn dump(tree: &ShapeTree<TreeObject>, id: NodeId, prefix: &str, out: &mut String) {
    match tree.node(id) {
        Node::Leaf {
            bounds,
            object,
            shape_index,
            ..
        } => {
            out.push_str(&format!(
                "{prefix}L {}/{} {}\n",
                obj_str(object),
                shape_index,
                bounds_str(bounds)
            ));
        }
        Node::Inner {
            bounds,
            first_child,
            second_child,
            ..
        } => {
            out.push_str(&format!("{prefix}I {}\n", bounds_str(bounds)));
            let deeper = format!("{prefix} ");
            dump(tree, *first_child, &deeper, out);
            dump(tree, *second_child, &deeper, out);
        }
        Node::Free { .. } => unreachable!("a reachable node is never a free slot"),
    }
}

fn print_tree(tree: &ShapeTree<TreeObject>, label: &str) {
    println!(
        "== {label} n={} rootNull={}",
        tree.leaf_count(),
        tree.root().is_none()
    );
    if let Some(root) = tree.root() {
        let mut s = String::new();
        dump(tree, root, "", &mut s);
        print!("{s}");
    }
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let ops: usize = a.first().map(|s| s.parse().unwrap()).unwrap_or(200);
    let seed: i64 = a.get(1).map(|s| s.parse().unwrap()).unwrap_or(1);
    let mode: i32 = a.get(2).map(|s| s.parse().unwrap()).unwrap_or(0);
    let dump_every: usize = a.get(3).map(|s| s.parse().unwrap()).unwrap_or(1);
    let insert_pct: i64 = a.get(4).map(|s| s.parse().unwrap()).unwrap_or(62);
    let mut rng = Rng(if seed == 0 {
        0x9E37_79B9_7F4A_7C15
    } else {
        seed as u64
    });

    let dirs = if mode == 0 {
        ShapeBoundingDirections::Orthogonal
    } else {
        ShapeBoundingDirections::FortyfiveDegree
    };
    let mut tree: ShapeTree<TreeObject> = ShapeTree::new(dirs);
    let mut live: Vec<LeafId> = Vec::new();
    let mut next_id: u32 = 1;

    for step in 0..ops {
        let roll = rng.rnd(100);
        if live.is_empty() || roll < insert_pct {
            // `ShapeTree::insert_tiles` — the port of `ShapeTree.insert(Storable)`: the TREE
            // applies its bounding directions to each tile shape.
            let shape_count = 1 + rng.rnd(3) as usize;
            let mut shapes: Vec<TileShape> = Vec::new();
            for _ in 0..shape_count {
                let x = rng.rnd(2000) - 500;
                let y = rng.rnd(2000) - 500;
                let w = 1 + rng.rnd(150);
                let h = 1 + rng.rnd(150);
                shapes.push(TileShape::Box(IntBox::from_coords(
                    x as i32,
                    y as i32,
                    (x + w) as i32,
                    (y + h) as i32,
                )));
            }
            let o = TreeObject::Item(ItemId(next_id));
            next_id += 1;
            let entries = tree.insert_tiles(o, &shapes);
            let mut ib = format!("op{step} insert {} x{shape_count}", obj_str(&o));
            for entry in entries {
                match entry {
                    None => ib.push_str(" -"),
                    Some(id) => {
                        live.push(id);
                        ib.push_str(&format!(" {}", key(&tree, id)));
                    }
                }
            }
            println!("{ib}");
        } else if roll < insert_pct + 12 {
            // In-place re-key of a live leaf: `ShapeSearchTree.java:150,214-215,221,294-295,
            // 325-326,342-343`.
            let idx = rng.rnd(live.len() as u64) as usize;
            let new_index = rng.rnd(4) as usize;
            let leaf = live[idx];
            let new_owner = TreeObject::Item(ItemId(next_id));
            next_id += 1;
            println!(
                "op{step} rekey {} -> {}/{new_index}",
                key(&tree, leaf),
                obj_str(&new_owner)
            );
            tree.set_leaf_entry(leaf, new_owner, new_index);
        } else {
            // `remove_opt` — the port of `remove(Leaf[])` on an array with `null` holes.
            let batch = 1 + rng.rnd(4) as usize;
            let mut batch_arr: Vec<Option<LeafId>> = Vec::new();
            let mut rb = format!("op{step} remove");
            for _ in 0..batch {
                let hole = rng.rnd(5) == 0;
                if hole || live.is_empty() {
                    batch_arr.push(None);
                    rb.push_str(" -");
                } else {
                    let idx = rng.rnd(live.len() as u64) as usize;
                    let leaf = live.remove(idx);
                    rb.push_str(&format!(" {}", key(&tree, leaf)));
                    batch_arr.push(Some(leaf));
                }
            }
            println!("{rb}");
            tree.remove_opt(&batch_arr);
        }

        if step % dump_every == 0 || step == ops - 1 {
            print_tree(&tree, &format!("op{step}"));
        }

        for q in 0..2 {
            let x = rng.rnd(2000) - 500;
            let y = rng.rnd(2000) - 500;
            let w = 1 + rng.rnd(600);
            let h = 1 + rng.rnd(600);
            let tile = TileShape::Box(IntBox::from_coords(
                x as i32,
                y as i32,
                (x + w) as i32,
                (y + h) as i32,
            ));
            let query = tree
                .bounding_shape(&tile)
                .expect("a box is always boundable");
            let res = tree.overlaps(&query);
            let mut s = format!("q{q} {} ->", bounds_str(&query));
            for e in &res {
                s.push_str(&format!(" {}/{}", obj_str(&e.object), e.shape_index));
            }
            println!("{s}");
        }

        let arr = tree.to_array();
        let mut ta = String::from("toArray");
        for l in &arr {
            ta.push_str(&format!(" {}", key(&tree, *l)));
        }
        println!("{ta}");

        let mut dr = String::from("depths");
        for l in &arr {
            let d: i64 = if tree.leaf_count() == 1 {
                -1
            } else {
                tree.distance_to_root(*l) as i64
            };
            dr.push_str(&format!(" {d}"));
        }
        println!("{dr}");
    }
}
