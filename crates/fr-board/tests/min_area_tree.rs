//! Integration tests for [`fr_board::datastructures::ShapeTree`] (Java `MinAreaTree`).
//!
//! Every expected value below is copied verbatim from the Java ground-truth driver
//! `scripts/differential/java/P2T3.java`, whose output is checked in as
//! `scripts/differential/P2T3.expected.out` and reproduced by
//! `./scripts/differential/run.sh p2t3`. The driver is compiled against the *real*, unmodified
//! `datastructures/{ShapeTree,MinAreaTree,ArrayStack}.java` from the sibling `../freerouting`
//! checkout, so these are Java's answers, not a re-derivation of them.
//!
//! The last test in the file is the port of `MinAreaTreeConcurrencyTest.java`.

use fr_board::datastructures::{LeafId, Node, ShapeTree, TreeEntry};
use fr_board::ids::{ItemId, TreeObject};
use fr_geometry::bounding_directions::ShapeBoundingDirections;
use fr_geometry::int_box::IntBox;
use fr_geometry::regular_tile_shape::RegularTileShape;
use std::collections::BTreeSet;

/// `TreeObject::Item(ItemId(n))` — the driver's `Obj(n)`, whose `compareTo` is verbatim
/// `Item.compareTo` (Item.java:93-103), i.e. `other.id - this.id`: objects sort by *descending*
/// id.
fn obj(n: u32) -> TreeObject {
    TreeObject::Item(ItemId(n))
}

fn boxed(ll_x: i32, ll_y: i32, ur_x: i32, ur_y: i32) -> RegularTileShape {
    RegularTileShape::Box(IntBox::from_coords(ll_x, ll_y, ur_x, ur_y))
}

/// Renders the tree in exactly the format `P2T3.dump` prints, so the strings below can be
/// pasted straight out of `scripts/differential/P2T3.expected.out`.
fn dump(tree: &ShapeTree<TreeObject>) -> String {
    fn bounds_str(shape: &RegularTileShape) -> String {
        let b = shape.bounding_box();
        format!("({},{},{},{})", b.ll.x, b.ll.y, b.ur.x, b.ur.y)
    }
    fn walk(
        tree: &ShapeTree<TreeObject>,
        id: fr_board::datastructures::NodeId,
        prefix: &str,
        out: &mut String,
    ) {
        match tree.node(id) {
            Node::Leaf {
                bounds,
                object,
                shape_index,
                ..
            } => {
                let TreeObject::Item(ItemId(n)) = object else {
                    unreachable!("the fixture only stores items")
                };
                out.push_str(&format!(
                    "{prefix}Leaf #{n}/{shape_index} {}\n",
                    bounds_str(bounds)
                ));
            }
            Node::Inner {
                bounds,
                first_child,
                second_child,
                ..
            } => {
                out.push_str(&format!("{prefix}Inner {}\n", bounds_str(bounds)));
                let deeper = format!("{prefix}  ");
                walk(tree, *first_child, &deeper, out);
                walk(tree, *second_child, &deeper, out);
            }
            Node::Free { .. } => unreachable!("a reachable node is never a free slot"),
        }
    }
    let mut out = String::new();
    match tree.root() {
        None => out.push_str("(empty)\n"),
        Some(root) => walk(tree, root, "", &mut out),
    }
    out
}

/// Renders an `overlaps` result the way `P2T3.printOverlaps` does.
fn entries_str(found: &BTreeSet<TreeEntry<TreeObject>>) -> String {
    found
        .iter()
        .map(|e| {
            let TreeObject::Item(ItemId(n)) = e.object else {
                unreachable!("the fixture only stores items")
            };
            format!("#{n}/{}", e.shape_index)
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The 8 boxes of `P2T3.eightBoxes`, in insertion order. Object ids are 1..=8, shape index 0.
const BOXES: [(u32, [i32; 4]); 8] = [
    (1, [0, 0, 10, 10]),   // A
    (2, [20, 0, 30, 10]),  // B
    (3, [0, 20, 10, 30]),  // C
    (4, [20, 20, 30, 30]), // D
    (5, [40, 0, 50, 10]),  // E
    (6, [40, 20, 50, 30]), // F
    (7, [5, 5, 15, 15]),   // G
    (8, [60, 60, 70, 70]), // H
];

fn eight_box_tree() -> (ShapeTree<TreeObject>, Vec<LeafId>) {
    let mut tree = ShapeTree::new(ShapeBoundingDirections::Orthogonal);
    let mut leaves = Vec::new();
    for (id, c) in BOXES {
        let inserted = tree.insert(obj(id), &[boxed(c[0], c[1], c[2], c[3])]);
        assert_eq!(inserted.len(), 1);
        leaves.push(inserted[0]);
    }
    (tree, leaves)
}

/// The hand-derived tree shape, step by step.
///
/// Areas are `width * height` because the bounding directions are orthogonal, so every bound is
/// an `IntBox` (`OrthogonalBoundingDirections.bounds(IntBox)` is the identity). At every inner
/// node `positionLocate` (MinAreaTree.java:89-116) first widens that node's own bounds to
/// include the new leaf, then descends into the child whose bounds grow *least* in area after
/// the union — `firstAreaIncrease <= secondAreaIncrease` (MinAreaTree.java:109), so an exact
/// tie goes to the FIRST child.
///
/// 1. `#1 (0,0,10,10)` — empty tree, so the leaf becomes the root (MinAreaTree.java:55-58).
/// 2. `#2 (20,0,30,10)` — the root is a leaf, so it is replaced by
///    `Inner(#1 ∪ #2) = (0,0,30,10)` holding `[#1, #2]`.
/// 3. `#3 (0,20,10,30)` — root `(0,0,30,10)` widens to `(0,0,30,30)`;
///    increase(#1) = area(0,0,10,30) − 100 = 300 − 100 = **200**,
///    increase(#2) = area(0,0,30,30) − 100 = 900 − 100 = 800 → first child `#1`.
///    `#1` is replaced by `Inner(0,0,10,30)` holding `[#1, #3]`.
/// 4. `#4 (20,20,30,30)` — root already `(0,0,30,30)`;
///    increase(Inner(0,0,10,30)) = area(0,0,30,30) − 300 = 600,
///    increase(#2) = area(20,0,30,30) − 100 = 300 − 100 = **200** → second child `#2`,
///    replaced by `Inner(20,0,30,30)` holding `[#2, #4]`.
/// 5. `#5 (40,0,50,10)` — root widens to `(0,0,50,30)`;
///    increase(Inner(0,0,10,30)) = area(0,0,50,30) − 300 = 1200,
///    increase(Inner(20,0,30,30)) = area(20,0,50,30) − 300 = **600** → second.
///    That node widens to `(20,0,50,30)`;
///    increase(#2) = area(20,0,50,10) − 100 = **200**,
///    increase(#4) = area(20,0,50,30) − 100 = 800 → first child `#2`,
///    replaced by `Inner(20,0,50,10)` holding `[#2, #5]`.
/// 6. `#6 (40,20,50,30)` — root unchanged `(0,0,50,30)`;
///    increase(Inner(0,0,10,30)) = area(0,0,50,30) − 300 = 1200,
///    increase(Inner(20,0,50,30)) = 900 − 900 = **0** → second (already contains it).
///    Inside: increase(Inner(20,0,50,10)) = area(20,0,50,30) − 300 = 600,
///    increase(#4) = area(20,20,50,30) − 100 = **200** → second child `#4`,
///    replaced by `Inner(20,20,50,30)` holding `[#4, #6]`.
/// 7. `#7 (5,5,15,15)` — root unchanged;
///    increase(Inner(0,0,10,30)) = area(0,0,15,30) − 300 = **150**,
///    increase(Inner(20,0,50,30)) = area(5,0,50,30) − 900 = 450 → first.
///    That node widens to `(0,0,15,30)`;
///    increase(#1) = area(0,0,15,15) − 100 = **125**,
///    increase(#3) = area(0,5,15,30) − 100 = 275 → first child `#1`,
///    replaced by `Inner(0,0,15,15)` holding `[#1, #7]`.
/// 8. `#8 (60,60,70,70)` — root widens to `(0,0,70,70)`;
///    increase(Inner(0,0,15,30)) = area(0,0,70,70) − 450 = 4450,
///    increase(Inner(20,0,50,30)) = area(20,0,70,70) − 900 = **2600** → second.
///    That node widens to `(20,0,70,70)`;
///    increase(Inner(20,0,50,10)) = area(20,0,70,70) − 300 = 3200,
///    increase(Inner(20,20,50,30)) = area(20,20,70,70) − 300 = **2200** → second.
///    That node widens to `(20,20,70,70)`;
///    increase(#4) = area(20,20,70,70) − 100 = 2400,
///    increase(#6) = area(40,20,70,70) − 100 = **1400** → second child `#6`,
///    replaced by `Inner(40,20,70,70)` holding `[#6, #8]`.
#[rustfmt::skip]
const EIGHT_BOX_TREE: &str = concat!(
    "Inner (0,0,70,70)\n",
    "  Inner (0,0,15,30)\n",
    "    Inner (0,0,15,15)\n",
    "      Leaf #1/0 (0,0,10,10)\n",
    "      Leaf #7/0 (5,5,15,15)\n",
    "    Leaf #3/0 (0,20,10,30)\n",
    "  Inner (20,0,70,70)\n",
    "    Inner (20,0,50,10)\n",
    "      Leaf #2/0 (20,0,30,10)\n",
    "      Leaf #5/0 (40,0,50,10)\n",
    "    Inner (20,20,70,70)\n",
    "      Leaf #4/0 (20,20,30,30)\n",
    "      Inner (40,20,70,70)\n",
    "        Leaf #6/0 (40,20,50,30)\n",
    "        Leaf #8/0 (60,60,70,70)\n",
);

#[test]
fn eight_box_insert_builds_the_java_tree() {
    let (tree, _) = eight_box_tree();
    assert_eq!(tree.leaf_count(), 8);
    assert_eq!(dump(&tree), EIGHT_BOX_TREE);
}

/// Every intermediate tree of `P2T3.eightBoxes`, so a wrong `positionLocate` decision is caught
/// at the step that made it rather than only at the end.
#[test]
fn eight_box_insert_matches_java_at_every_step() {
    let expected: [&str; 8] = [
        "Leaf #1/0 (0,0,10,10)\n",
        concat!(
            "Inner (0,0,30,10)\n",
            "  Leaf #1/0 (0,0,10,10)\n",
            "  Leaf #2/0 (20,0,30,10)\n",
        ),
        concat!(
            "Inner (0,0,30,30)\n",
            "  Inner (0,0,10,30)\n",
            "    Leaf #1/0 (0,0,10,10)\n",
            "    Leaf #3/0 (0,20,10,30)\n",
            "  Leaf #2/0 (20,0,30,10)\n",
        ),
        concat!(
            "Inner (0,0,30,30)\n",
            "  Inner (0,0,10,30)\n",
            "    Leaf #1/0 (0,0,10,10)\n",
            "    Leaf #3/0 (0,20,10,30)\n",
            "  Inner (20,0,30,30)\n",
            "    Leaf #2/0 (20,0,30,10)\n",
            "    Leaf #4/0 (20,20,30,30)\n",
        ),
        concat!(
            "Inner (0,0,50,30)\n",
            "  Inner (0,0,10,30)\n",
            "    Leaf #1/0 (0,0,10,10)\n",
            "    Leaf #3/0 (0,20,10,30)\n",
            "  Inner (20,0,50,30)\n",
            "    Inner (20,0,50,10)\n",
            "      Leaf #2/0 (20,0,30,10)\n",
            "      Leaf #5/0 (40,0,50,10)\n",
            "    Leaf #4/0 (20,20,30,30)\n",
        ),
        concat!(
            "Inner (0,0,50,30)\n",
            "  Inner (0,0,10,30)\n",
            "    Leaf #1/0 (0,0,10,10)\n",
            "    Leaf #3/0 (0,20,10,30)\n",
            "  Inner (20,0,50,30)\n",
            "    Inner (20,0,50,10)\n",
            "      Leaf #2/0 (20,0,30,10)\n",
            "      Leaf #5/0 (40,0,50,10)\n",
            "    Inner (20,20,50,30)\n",
            "      Leaf #4/0 (20,20,30,30)\n",
            "      Leaf #6/0 (40,20,50,30)\n",
        ),
        concat!(
            "Inner (0,0,50,30)\n",
            "  Inner (0,0,15,30)\n",
            "    Inner (0,0,15,15)\n",
            "      Leaf #1/0 (0,0,10,10)\n",
            "      Leaf #7/0 (5,5,15,15)\n",
            "    Leaf #3/0 (0,20,10,30)\n",
            "  Inner (20,0,50,30)\n",
            "    Inner (20,0,50,10)\n",
            "      Leaf #2/0 (20,0,30,10)\n",
            "      Leaf #5/0 (40,0,50,10)\n",
            "    Inner (20,20,50,30)\n",
            "      Leaf #4/0 (20,20,30,30)\n",
            "      Leaf #6/0 (40,20,50,30)\n",
        ),
        EIGHT_BOX_TREE,
    ];

    let mut tree = ShapeTree::new(ShapeBoundingDirections::Orthogonal);
    for (step, (id, c)) in BOXES.into_iter().enumerate() {
        tree.insert(obj(id), &[boxed(c[0], c[1], c[2], c[3])]);
        assert_eq!(tree.leaf_count(), step + 1);
        assert_eq!(dump(&tree), expected[step], "tree after inserting #{id}");
    }
}

#[test]
fn overlaps_matches_java() {
    let (tree, _) = eight_box_tree();
    let query = |c: [i32; 4]| entries_str(&tree.overlaps(&boxed(c[0], c[1], c[2], c[3])));

    // Descending object id throughout: `Leaf.compareTo` (ShapeTree.java:216-223) delegates to
    // `Item.compareTo` (Item.java:98), whose subtraction is reversed.
    assert_eq!(query([0, 0, 10, 10]), "#7/0 #1/0");
    assert_eq!(query([15, 15, 45, 45]), "#7/0 #6/0 #4/0");
    assert_eq!(query([55, 55, 65, 65]), "#8/0");
    assert_eq!(query([200, 200, 210, 210]), "");
    assert_eq!(
        query([-5, -5, 100, 100]),
        "#8/0 #7/0 #6/0 #5/0 #4/0 #3/0 #2/0 #1/0"
    );
}

#[test]
fn overlaps_on_an_empty_tree_is_empty() {
    let tree: ShapeTree<TreeObject> = ShapeTree::new(ShapeBoundingDirections::Orthogonal);
    assert!(tree.overlaps(&boxed(0, 0, 10, 10)).is_empty());
    assert_eq!(tree.leaf_count(), 0);
    assert_eq!(tree.root(), None);
}

#[test]
fn distance_to_root_matches_java() {
    let (tree, leaves) = eight_box_tree();
    let expected = [3, 3, 2, 3, 3, 4, 3, 4];
    for (i, leaf) in leaves.iter().enumerate() {
        assert_eq!(
            tree.distance_to_root(*leaf),
            expected[i],
            "distanceToRoot #{}",
            i + 1
        );
    }
}

#[test]
fn remove_relinks_the_sibling_and_shrinks_ancestors() {
    let (mut tree, leaves) = eight_box_tree();

    // Removing #7 collapses `Inner(0,0,15,15)`: its sibling `#1` moves up to the grandparent,
    // whose bounds shrink from (0,0,15,30) back to (0,0,10,30). The root's recomputed bounds
    // (0,0,70,70) still contain the old (0,0,70,70), so `MinAreaTree.java:172-175` stops there.
    tree.remove(&[leaves[6]]);
    assert_eq!(tree.leaf_count(), 7);
    assert_eq!(
        dump(&tree),
        concat!(
            "Inner (0,0,70,70)\n",
            "  Inner (0,0,10,30)\n",
            "    Leaf #1/0 (0,0,10,10)\n",
            "    Leaf #3/0 (0,20,10,30)\n",
            "  Inner (20,0,70,70)\n",
            "    Inner (20,0,50,10)\n",
            "      Leaf #2/0 (20,0,30,10)\n",
            "      Leaf #5/0 (40,0,50,10)\n",
            "    Inner (20,20,70,70)\n",
            "      Leaf #4/0 (20,20,30,30)\n",
            "      Inner (40,20,70,70)\n",
            "        Leaf #6/0 (40,20,50,30)\n",
            "        Leaf #8/0 (60,60,70,70)\n",
        )
    );
    assert_eq!(entries_str(&tree.overlaps(&boxed(0, 0, 10, 10))), "#1/0");
    assert_eq!(
        entries_str(&tree.overlaps(&boxed(-5, -5, 100, 100))),
        "#8/0 #6/0 #5/0 #4/0 #3/0 #2/0 #1/0"
    );

    // Removing #8 shrinks three ancestors in a row, all the way to the root.
    tree.remove(&[leaves[7]]);
    assert_eq!(tree.leaf_count(), 6);
    assert_eq!(
        dump(&tree),
        concat!(
            "Inner (0,0,50,30)\n",
            "  Inner (0,0,10,30)\n",
            "    Leaf #1/0 (0,0,10,10)\n",
            "    Leaf #3/0 (0,20,10,30)\n",
            "  Inner (20,0,50,30)\n",
            "    Inner (20,0,50,10)\n",
            "      Leaf #2/0 (20,0,30,10)\n",
            "      Leaf #5/0 (40,0,50,10)\n",
            "    Inner (20,20,50,30)\n",
            "      Leaf #4/0 (20,20,30,30)\n",
            "      Leaf #6/0 (40,20,50,30)\n",
        )
    );
}

#[test]
fn to_array_walks_leftmost_leaf_first() {
    let (mut tree, leaves) = eight_box_tree();
    tree.remove(&[leaves[6], leaves[7]]);
    let order: Vec<String> = tree
        .to_array()
        .into_iter()
        .map(|id| entries_str(&BTreeSet::from([tree.leaf_entry(id)])))
        .collect();
    // Java `toArray: #1/0 #3/0 #2/0 #5/0 #4/0 #6/0`.
    assert_eq!(order.join(" "), "#1/0 #3/0 #2/0 #5/0 #4/0 #6/0");
}

#[test]
fn removing_every_leaf_empties_the_tree() {
    let (mut tree, leaves) = eight_box_tree();
    tree.remove(&[leaves[6], leaves[7]]);
    // Java prints `rootNull=false` until the very last removal.
    for (i, leaf) in leaves.iter().take(6).enumerate() {
        tree.remove(std::slice::from_ref(leaf));
        assert_eq!(tree.leaf_count(), 5 - i);
        assert_eq!(tree.root().is_none(), i == 5, "after removing #{}", i + 1);
    }
    assert_eq!(tree.leaf_count(), 0);
    assert!(tree.overlaps(&boxed(-5, -5, 100, 100)).is_empty());
}

#[test]
fn removing_the_only_leaf_empties_the_tree() {
    let mut tree = ShapeTree::new(ShapeBoundingDirections::Orthogonal);
    let leaves = tree.insert(obj(9), &[boxed(0, 0, 1, 1)]);
    tree.remove(&leaves);
    assert_eq!(tree.leaf_count(), 0);
    assert_eq!(tree.root(), None);
    // Java `ShapeTree.remove(null)` (ShapeTree.java:96-98) is a no-op; the Rust slice form is an
    // empty slice.
    tree.remove(&[]);
    assert_eq!(tree.leaf_count(), 0);
}

#[test]
fn an_exact_area_tie_goes_to_the_first_child() {
    // `firstAreaIncrease <= secondAreaIncrease` (MinAreaTree.java:109).
    // increase(#1) = area(0,0,20,20) − 100 = 300; increase(#2) = area(10,10,30,30) − 100 = 300.
    let mut tree = ShapeTree::new(ShapeBoundingDirections::Orthogonal);
    tree.insert(obj(1), &[boxed(0, 0, 10, 10)]);
    tree.insert(obj(2), &[boxed(20, 20, 30, 30)]);
    tree.insert(obj(3), &[boxed(10, 10, 20, 20)]);
    assert_eq!(
        dump(&tree),
        concat!(
            "Inner (0,0,30,30)\n",
            "  Inner (0,0,20,20)\n",
            "    Leaf #1/0 (0,0,10,10)\n",
            "    Leaf #3/0 (10,10,20,20)\n",
            "  Leaf #2/0 (20,20,30,30)\n",
        )
    );
}

#[test]
fn inserting_an_object_with_no_shapes_does_nothing() {
    // `ShapeTree.insert(Storable)` returns early when `treeShapeCount() <= 0`
    // (ShapeTree.java:32-35).
    let mut tree = ShapeTree::new(ShapeBoundingDirections::Orthogonal);
    assert!(tree.insert(obj(1), &[]).is_empty());
    assert_eq!(tree.leaf_count(), 0);
    assert_eq!(tree.root(), None);
}

#[test]
fn entries_order_by_object_then_shape_index() {
    // Java `Leaf.compareTo` (ShapeTree.java:216-223): object first, then shapeIndexInObject.
    // Object #2 owns two shapes, object #1 one. Objects sort by *descending* id
    // (`Item.compareTo`, Item.java:98) and shape indices ascending, so the TreeSet yields
    // `#2/0 #2/1 #1/0` — verbatim from `P2T3.java`'s `Q6 multi-shape` line.
    let mut tree = ShapeTree::new(ShapeBoundingDirections::Orthogonal);
    let two = tree.insert(obj(2), &[boxed(0, 0, 10, 10), boxed(100, 100, 110, 110)]);
    assert_eq!(two.len(), 2);
    tree.insert(obj(1), &[boxed(5, 5, 15, 15)]);
    assert_eq!(
        entries_str(&tree.overlaps(&boxed(-5, -5, 200, 200))),
        "#2/0 #2/1 #1/0"
    );
}

#[test]
fn leaf_bounds_and_entry_report_what_was_inserted() {
    let (tree, leaves) = eight_box_tree();
    assert_eq!(tree.leaf_bounds(leaves[7]), boxed(60, 60, 70, 70));
    assert_eq!(
        tree.leaf_entry(leaves[7]),
        TreeEntry {
            object: obj(8),
            shape_index: 0,
        }
    );
}

#[test]
fn identical_insert_sequences_produce_identical_trees() {
    let (a, _) = eight_box_tree();
    let (b, _) = eight_box_tree();
    assert_eq!(format!("{a:?}"), format!("{b:?}"));
    assert_eq!(a, b);

    // ... and the arena survives a clone bit-for-bit, which is what `Board::clone` relies on
    // (global-constraints.md: "search-tree entries ARE cloned since leaf ids are arena indices
    // that clone with the tree").
    let cloned = a.clone();
    assert_eq!(format!("{cloned:?}"), format!("{a:?}"));
    assert_eq!(
        cloned.overlaps(&boxed(-5, -5, 100, 100)),
        a.overlaps(&boxed(-5, -5, 100, 100))
    );
}

#[test]
fn removal_frees_arena_slots_for_reuse() {
    // The arena is the port's replacement for Java's garbage collector: after a removal the two
    // freed slots (the leaf and its parent inner node) are handed back out by the next inserts,
    // so the node vector does not grow without bound.
    let (mut tree, leaves) = eight_box_tree();
    let node_count = tree.node_count();
    tree.remove(&[leaves[7]]);
    assert_eq!(tree.node_count(), node_count, "freed slots stay allocated");
    assert_eq!(tree.free_slot_count(), 2);
    tree.insert(obj(9), &[boxed(60, 60, 70, 70)]);
    assert_eq!(tree.free_slot_count(), 0);
    assert_eq!(tree.node_count(), node_count);
    assert_eq!(tree.leaf_count(), 8);
}

/// Renders only the *structure* of the tree — bounds and shape, no leaf keys — so a test can
/// prove a key rewrite left the layout alone.
fn structure(tree: &ShapeTree<TreeObject>) -> String {
    fn walk(
        tree: &ShapeTree<TreeObject>,
        id: fr_board::datastructures::NodeId,
        prefix: &str,
        out: &mut String,
    ) {
        let (tag, bounds) = match tree.node(id) {
            Node::Leaf { bounds, .. } => ("Leaf", bounds),
            Node::Inner { bounds, .. } => ("Inner", bounds),
            Node::Free { .. } => unreachable!("a reachable node is never a free slot"),
        };
        let b = bounds.bounding_box();
        out.push_str(&format!(
            "{prefix}{tag} ({},{},{},{})\n",
            b.ll.x, b.ll.y, b.ur.x, b.ur.y
        ));
        if let Node::Inner {
            first_child,
            second_child,
            ..
        } = tree.node(id)
        {
            let deeper = format!("{prefix}  ");
            walk(tree, *first_child, &deeper, out);
            walk(tree, *second_child, &deeper, out);
        }
    }
    let mut out = String::new();
    if let Some(root) = tree.root() {
        walk(tree, root, "", &mut out);
    }
    out
}

/// `ShapeSearchTree` re-keys leaves that stay in the tree — `changeEntries`
/// (`ShapeSearchTree.java:150`), `mergeEntriesInFront` (`:214-215`, `:221`),
/// `mergeEntriesAtEnd` (`:294-295`) and `reuseEntriesAfterCutout` (`:325-326`, `:342-343`) all
/// assign `leaf.object` and/or `leaf.shapeIndexInObject` in place. The tree must not move.
#[test]
fn set_leaf_entry_rewrites_the_key_and_nothing_else() {
    let (mut tree, leaves) = eight_box_tree();
    let structure_before = structure(&tree);
    let order_before = tree.to_array();
    let depths_before: Vec<usize> = order_before
        .iter()
        .map(|l| tree.distance_to_root(*l))
        .collect();
    let bounds_before: Vec<_> = leaves.iter().map(|l| tree.leaf_bounds(*l)).collect();

    // Hand leaf #4's shape to object #99 at shape index 7, the way a merged trace takes over
    // another trace's tree entries.
    tree.set_leaf_entry(leaves[3], obj(99), 7);

    assert_eq!(structure(&tree), structure_before, "layout must not move");
    assert_eq!(tree.to_array(), order_before, "leaf order must not move");
    assert_eq!(
        order_before
            .iter()
            .map(|l| tree.distance_to_root(*l))
            .collect::<Vec<_>>(),
        depths_before
    );
    assert_eq!(
        leaves
            .iter()
            .map(|l| tree.leaf_bounds(*l))
            .collect::<Vec<_>>(),
        bounds_before,
        "bounding shapes must not move"
    );
    assert_eq!(tree.leaf_count(), 8);

    // The new key is what the tree now reports, in its new sort position: ids sort descending
    // (`Item.compareTo`, Item.java:98), so #99 sorts *first*.
    assert_eq!(
        tree.leaf_entry(leaves[3]),
        TreeEntry {
            object: obj(99),
            shape_index: 7,
        }
    );
    assert_eq!(
        entries_str(&tree.overlaps(&boxed(-5, -5, 100, 100))),
        "#99/7 #8/0 #7/0 #6/0 #5/0 #3/0 #2/0 #1/0"
    );
    // The leaf is still found by a query that only touches its own box.
    assert_eq!(entries_str(&tree.overlaps(&boxed(21, 21, 22, 22))), "#99/7");
    assert_bounds_are_the_union_of_children(&tree);
}

/// A merge-style re-key of a whole run of leaves: every entry of one object is handed to
/// another and renumbered, and the tree still has the shape the original inserts produced.
#[test]
fn set_leaf_entry_can_rehome_a_whole_object() {
    let mut tree = ShapeTree::new(ShapeBoundingDirections::Orthogonal);
    let from = tree.insert(
        obj(2),
        &[
            boxed(0, 0, 10, 10),
            boxed(20, 0, 30, 10),
            boxed(40, 0, 50, 10),
        ],
    );
    tree.insert(obj(1), &[boxed(0, 20, 10, 30)]);
    let structure_before = structure(&tree);

    // `mergeEntriesInFront` renumbers from index 0 upward onto the destination trace.
    for (new_index, leaf) in from.iter().enumerate() {
        tree.set_leaf_entry(*leaf, obj(1), new_index + 1);
    }

    assert_eq!(structure(&tree), structure_before);
    assert_eq!(
        entries_str(&tree.overlaps(&boxed(-5, -5, 100, 100))),
        "#1/0 #1/1 #1/2 #1/3"
    );
}

/// Every inner node's bounds must equal the union of its two children's bounds.
///
/// This is what makes `MinAreaTree.removeLeaf`'s upward loop stop early: it breaks as soon as
/// `newBounds.contains(node.boundingShape)` (MinAreaTree.java:172-175), and under this
/// invariant `newBounds` is never *larger* than the old bounds, so `contains` can only mean
/// "equal" — the break is a pure optimisation, not a behavioural rule. Insertion preserves the
/// invariant (`positionLocate` widens every node on the descent path by exactly the new leaf's
/// bounds, and the leaf lands below all of them, MinAreaTree.java:94-95), and removal restores
/// it (MinAreaTree.java:167-171).
fn assert_bounds_are_the_union_of_children(tree: &ShapeTree<TreeObject>) {
    fn walk(tree: &ShapeTree<TreeObject>, id: fr_board::datastructures::NodeId) {
        let Node::Inner {
            bounds,
            first_child,
            second_child,
            ..
        } = tree.node(id)
        else {
            return;
        };
        let child_bounds = |c| match tree.node(c) {
            Node::Inner { bounds, .. } | Node::Leaf { bounds, .. } => *bounds,
            Node::Free { .. } => unreachable!("a reachable node is never a free slot"),
        };
        assert_eq!(
            *bounds,
            child_bounds(*second_child).union(&child_bounds(*first_child)),
            "inner node {} is not the union of its children",
            id.index()
        );
        walk(tree, *first_child);
        walk(tree, *second_child);
    }
    if let Some(root) = tree.root() {
        walk(tree, root);
    }
}

#[test]
fn inner_bounds_stay_the_union_of_their_children() {
    let mut tree = ShapeTree::new(ShapeBoundingDirections::Orthogonal);
    let mut leaves = Vec::new();
    for (id, c) in BOXES {
        leaves.push(tree.insert(obj(id), &[boxed(c[0], c[1], c[2], c[3])])[0]);
        assert_bounds_are_the_union_of_children(&tree);
    }
    // Remove in an order that forces the upward loop to run at several depths.
    for order in [7usize, 0, 5, 2, 6, 1, 4, 3] {
        tree.remove(&[leaves[order]]);
        assert_bounds_are_the_union_of_children(&tree);
    }
    assert_eq!(tree.leaf_count(), 0);
}

/// Port of `datastructures/MinAreaTreeConcurrencyTest.java`.
///
/// The Java regression it guards is a `MinAreaTree` that kept its traversal `ArrayStack` in an
/// *instance field*, so two threads querying the same tree corrupted each other's stack. The
/// Rust port cannot express that bug: [`ShapeTree::overlaps`] takes `&self` and allocates its
/// `Vec` stack as a local, and `&ShapeTree<O>` is `Sync` (asserted below), so the compiler
/// rejects any shared-mutable traversal state before a test could ever run.
///
/// What survives as a behavioural check is the observable property the Java test asserts:
/// repeating the same query many times over an unchanged tree yields the same answer every
/// time and leaves the tree untouched (`assertDoesNotThrow` over 8 × 200 `overlaps` calls).
/// The brief makes this the single-threaded form.
#[test]
fn repeated_overlaps_queries_do_not_disturb_the_tree() {
    fn assert_sync<T: Sync>() {}
    fn assert_send<T: Send>() {}
    assert_sync::<ShapeTree<TreeObject>>();
    assert_send::<ShapeTree<TreeObject>>();

    let (tree, _) = eight_box_tree();
    let before = format!("{tree:?}");
    // The Java test queries the board's own bounding box, i.e. a shape covering every leaf.
    let query = boxed(-5, -5, 100, 100);
    let expected = tree.overlaps(&query);
    assert_eq!(expected.len(), 8);

    for _ in 0..8 {
        for _ in 0..200 {
            assert_eq!(tree.overlaps(&query), expected);
        }
    }
    assert_eq!(format!("{tree:?}"), before);
    assert_eq!(tree.leaf_count(), 8);
}
