//! Plan 2 Task 10: `ShapeSearchTree` and `SearchTreeManager`.
//!
//! Java: `board/searchtree/{ShapeSearchTree,ShapeSearchTree45Degree,ShapeSearchTree90Degree,
//! SearchTreeManager}.java`.
//!
//! # Provenance of every expectation below
//!
//! `scripts/differential/java/P2T10.java` builds the same board as
//! [`board_builder::BoardFixture`] through the **real** `app.freerouting.board.facade.BasicBoard`
//! (JDK 25, the clone's own compiled classes) and prints, for each tree: its key, its leaves in
//! `ShapeTree.toArray()` order with their bounding shapes, every item's precalculated tree
//! shapes, and the result of every query. The numbers here are copied from that output; each
//! test names the driver mode it came from.

#[path = "board_builder.rs"]
mod board_builder;

use board_builder::{BoardFixture, WIDE_CLEARANCE_CLASS};
use fr_board::prelude::*;
use fr_geometry::{IntBox, IntOctagon, Point, Polyline, TileShape};

// ---------------------------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------------------------

fn bx(llx: i32, lly: i32, urx: i32, ury: i32) -> TileShape {
    TileShape::Box(IntBox::from_coords(llx, lly, urx, ury))
}

#[allow(clippy::too_many_arguments)]
fn oct(lx: i32, by: i32, rx: i32, ty: i32, ulx: i32, lrx: i32, llx: i32, urx: i32) -> TileShape {
    TileShape::Octagon(IntOctagon::new(lx, by, rx, ty, ulx, lrx, llx, urx))
}

/// `ShapeTree.toArray()` as `(item id, shape index, bounding shape)` triples — Java's
/// `leaf obj=… idx=… bounds=…` lines.
fn leaves(tree: &ShapeSearchTree) -> Vec<(u32, usize, TileShape)> {
    tree.tree()
        .to_array()
        .into_iter()
        .map(|leaf| {
            let entry = tree.tree().leaf_entry(leaf);
            let TreeObject::Item(ItemId(id)) = entry.object else {
                panic!("the fixture stores no expansion rooms")
            };
            (
                id,
                entry.shape_index,
                tree.tree().leaf_bounds(leaf).to_tile_shape(),
            )
        })
        .collect()
}

/// An item's precalculated tree shapes for one tree — Java's `shapes id=… n=… […]=…` line.
fn shapes(f: &BoardFixture, tree: TreeId, id: u32) -> Vec<Option<TileShape>> {
    let item = &f.items[&ItemId(id)];
    (0..item.tree_shape_count(tree))
        .map(|i| item.get_tree_shape(tree, i).cloned())
        .collect()
}

/// `(item id, shape index)` pairs, the way the driver prints a tree-entry collection.
fn pairs(entries: &[TreeEntry<TreeObject>]) -> Vec<(u32, usize)> {
    entries
        .iter()
        .map(|entry| {
            let TreeObject::Item(ItemId(id)) = entry.object else {
                panic!("the fixture stores no expansion rooms")
            };
            (id, entry.shape_index)
        })
        .collect()
}

fn object_ids(objects: &std::collections::BTreeSet<TreeObject>) -> Vec<u32> {
    objects
        .iter()
        .map(|object| match object {
            TreeObject::Item(ItemId(id)) => *id,
            TreeObject::Room(_) => panic!("the fixture stores no expansion rooms"),
        })
        .collect()
}

/// The probe box every query in the driver uses.
fn probe() -> TileShape {
    bx(-600, -100, 600, 500)
}

// ---------------------------------------------------------------------------------------------
// Keys and bounding directions (ShapeSearchTree.java:80-87, and the three constructors)
// ---------------------------------------------------------------------------------------------

#[test]
fn tree_keys_reproduce_the_java_strings() {
    // P2T10 prints `tree … key=…` for each of the three classes, from `ShapeSearchTree.getKey`
    // (ShapeSearchTree.java:80-87) via `toString` (:89-92).
    assert_eq!(
        ShapeSearchTree::new(TreeId(0), AngleRestriction::None, 0).get_key(),
        "ShapeSearchTree_FortyfiveDegree_cc0"
    );
    assert_eq!(
        ShapeSearchTree::new(TreeId(1), AngleRestriction::FortyFiveDegree, 1).get_key(),
        "ShapeSearchTree45Degree_FortyfiveDegree_cc1"
    );
    assert_eq!(
        ShapeSearchTree::new(TreeId(2), AngleRestriction::NinetyDegree, 1).get_key(),
        "ShapeSearchTree90Degree_Orthogonal_cc1"
    );
    assert_eq!(
        ShapeSearchTree::new(TreeId(3), AngleRestriction::FortyFiveDegree, 2).to_string(),
        "ShapeSearchTree45Degree_FortyfiveDegree_cc2"
    );
}

#[test]
fn the_default_tree_is_the_base_class_whatever_the_board_angle_is() {
    // SearchTreeManager.java:33: `new ShapeSearchTree(FortyfiveDegreeBoundingDirections.INSTANCE,
    // board, 0)` — the default tree never depends on `rules.traceAngleRestriction`. P2T10 modes
    // 0, 1 and 2 all print `tree default key=ShapeSearchTree_FortyfiveDegree_cc0`.
    for angle in [
        AngleRestriction::None,
        AngleRestriction::FortyFiveDegree,
        AngleRestriction::NinetyDegree,
    ] {
        let f = BoardFixture::with_angle(angle);
        assert_eq!(
            f.manager.get_default_tree().get_key(),
            "ShapeSearchTree_FortyfiveDegree_cc0"
        );
        assert!(!f.manager.is_clearance_compensation_used());
    }
}

// ---------------------------------------------------------------------------------------------
// Clearance compensation (ShapeSearchTree.java:104-114)
// ---------------------------------------------------------------------------------------------

#[test]
fn clearance_compensation_values_match_the_jvm() {
    // P2T10 mode 3, the `clearanceCompensationValue tree=… cc=… layer0=…` lines. The matrix it
    // dumps for layer 0 is
    //   getValue(1, j) = [0, 200, 200], getValue(2, j) = [0, 600, 800],
    //   clearanceCompensationValue(1) = 100, (2) = 400.
    let mut f = BoardFixture::new();
    f.insert_all();
    let default_id = f.manager.get_default_tree().id();
    let auto1 = f.build_autoroute_tree(1);
    let auto2 = f.build_autoroute_tree(2);

    for cc in 0..=2 {
        assert_eq!(
            f.tree(default_id)
                .clearance_compensation_value(cc, 0, &f.rules),
            0,
            "an uncompensated tree never inflates (cc={cc})"
        );
    }
    // getValue(1, 1) - compensation(1) = 200 - 100.
    assert_eq!(
        f.tree(auto1).clearance_compensation_value(1, 0, &f.rules),
        100
    );
    // getValue(2, 1) - compensation(1) = 600 - 100.
    assert_eq!(
        f.tree(auto1).clearance_compensation_value(2, 0, &f.rules),
        500
    );
    assert_eq!(
        f.tree(auto1).clearance_compensation_value(0, 0, &f.rules),
        0
    );
    // getValue(1, 2) - compensation(2) = 200 - 400, clamped to 0 (ShapeSearchTree.java:113).
    assert_eq!(
        f.tree(auto2).clearance_compensation_value(1, 0, &f.rules),
        0
    );
    // getValue(2, 2) - compensation(2) = 800 - 400.
    assert_eq!(
        f.tree(auto2).clearance_compensation_value(2, 0, &f.rules),
        400
    );

    // `Trace.getCompensatedHalfWidth` (Trace.java:86-89): 30 and 30 + 100.
    let Item::Trace(trace) = &f.items[&ItemId(4)] else {
        panic!("item 4 is the signal trace")
    };
    assert_eq!(
        f.tree(default_id).compensated_half_width(trace, &f.rules),
        30
    );
    assert_eq!(f.tree(auto1).compensated_half_width(trace, &f.rules), 130);
}

// ---------------------------------------------------------------------------------------------
// The stored shapes and the tree structure
// ---------------------------------------------------------------------------------------------

#[test]
fn the_default_tree_stores_what_the_jvm_stores() {
    // P2T10 mode 0, `tree default`. The leaf list is `ShapeTree.toArray()` order, so it also
    // pins the tree *structure* the insertion heuristic built.
    let mut f = BoardFixture::new();
    f.insert_all();
    let id = f.manager.get_default_tree().id();
    let tree = f.tree(id);
    assert_eq!(tree.size(), 8);

    let pin2 = oct(-550, -50, -450, 50, -600, -400, -600, -400);
    let pin3 = oct(430, -70, 570, 70, 360, 640, 360, 640);
    let trace4 = [
        oct(-530, -30, 30, 30, -542, 42, -542, 42),
        oct(-30, -30, 30, 430, -442, 42, -42, 442),
        oct(-30, 370, 530, 430, -442, 142, 358, 942),
    ];
    let trace5 = [
        oct(-840, 260, -760, 940, -1757, -1043, -557, 157),
        oct(-840, 860, 340, 940, -1757, -543, 43, 1257),
    ];

    assert_eq!(
        leaves(tree),
        vec![
            (2, 0, pin2.clone()),
            (5, 0, trace5[0].clone()),
            (5, 1, trace5[1].clone()),
            (4, 0, trace4[0].clone()),
            (4, 1, trace4[1].clone()),
            (4, 2, trace4[2].clone()),
            (3, 0, pin3.clone()),
            (3, 1, pin3.clone()),
        ]
    );

    assert_eq!(shapes(&f, id, 1), Vec::<Option<TileShape>>::new());
    assert_eq!(shapes(&f, id, 2), vec![Some(pin2)]);
    assert_eq!(shapes(&f, id, 3), vec![Some(pin3.clone()), Some(pin3)]);
    assert_eq!(
        shapes(&f, id, 4),
        trace4.into_iter().map(Some).collect::<Vec<_>>()
    );
    assert_eq!(
        shapes(&f, id, 5),
        trace5.into_iter().map(Some).collect::<Vec<_>>()
    );
}

#[test]
fn a_45_degree_autoroute_tree_is_octagon_keyed_and_a_90_degree_one_is_box_keyed() {
    // P2T10 mode 0 `tree autoroute_cc1 key=ShapeSearchTree45Degree_FortyfiveDegree_cc1` vs mode 1
    // `key=ShapeSearchTree90Degree_Orthogonal_cc1`. Same board, same compensation (100 on the
    // pins and traces of class 1), different stored shape family — and the leaves' bounding
    // shapes follow the tree's own bounding directions
    // (`OrthogonalBoundingDirections` gives `IntBox`, ShapeSearchTree90Degree.java:28).
    let mut f45 = BoardFixture::with_angle(AngleRestriction::FortyFiveDegree);
    f45.insert_all();
    let id45 = f45.build_autoroute_tree(1);
    let tree45 = f45.tree(id45);
    assert_eq!(
        tree45.get_key(),
        "ShapeSearchTree45Degree_FortyfiveDegree_cc1"
    );
    assert!(
        leaves(tree45)
            .iter()
            .all(|(_, _, bounds)| matches!(bounds, TileShape::Octagon(_))),
        "a 45-degree tree keys every leaf on an IntOctagon"
    );
    assert_eq!(
        leaves(tree45),
        vec![
            (5, 0, oct(-1340, -240, -260, 1440, -2464, -336, -1264, 864)),
            (2, 0, oct(-650, -150, -350, 150, -800, -200, -800, -200)),
            (4, 0, oct(-630, -130, 130, 130, -684, 184, -684, 184)),
            (3, 0, oct(330, -170, 670, 170, 160, 840, 160, 840)),
            (3, 1, oct(330, -170, 670, 170, 160, 840, 160, 840)),
            (4, 1, oct(-130, -130, 130, 530, -584, 184, -184, 584)),
            (5, 1, oct(-1340, 360, 840, 1440, -2464, 164, -664, 1964)),
            (4, 2, oct(-130, 270, 630, 530, -584, 284, 216, 1084)),
        ]
    );

    let mut f90 = BoardFixture::with_angle(AngleRestriction::NinetyDegree);
    f90.insert_all();
    let id90 = f90.build_autoroute_tree(1);
    let tree90 = f90.tree(id90);
    assert_eq!(tree90.get_key(), "ShapeSearchTree90Degree_Orthogonal_cc1");
    assert!(
        leaves(tree90)
            .iter()
            .all(|(_, _, bounds)| matches!(bounds, TileShape::Box(_))),
        "a 90-degree tree keys every leaf on an IntBox"
    );
    assert_eq!(
        leaves(tree90),
        vec![
            (5, 0, bx(-1340, -240, -260, 1440)),
            (2, 0, bx(-650, -150, -350, 150)),
            (4, 0, bx(-630, -130, 130, 130)),
            (3, 0, bx(330, -170, 670, 170)),
            (3, 1, bx(330, -170, 670, 170)),
            (4, 1, bx(-130, -130, 130, 530)),
            (5, 1, bx(-1340, 360, 840, 1440)),
            (4, 2, bx(-130, 270, 630, 530)),
        ]
    );
    // The trace shapes come from `offsetBox`, not `offsetShape`
    // (ShapeSearchTree90Degree.java:486-490).
    assert_eq!(
        shapes(&f90, id90, 4),
        vec![
            Some(bx(-630, -130, 130, 130)),
            Some(bx(-130, -130, 130, 530)),
            Some(bx(-130, 270, 630, 530)),
        ]
    );
}

#[test]
fn the_base_class_enlarges_where_the_45_degree_subclass_offsets() {
    // P2T10 mode 2 (`AngleRestriction.NONE`) `tree autoroute_cc1` vs mode 0's 45-degree tree.
    // Both inflate the same 100x100 SMD pad by 100, but the base class runs
    // `boundingTile().enlarge(100)` (ShapeSearchTree.java:891,900) — which cuts the corners of
    // the box off, giving diagonals -741/-259 — while the 45-degree override takes the
    // *bounding box* and `offset`s it (ShapeSearchTree45Degree.java:503-514), giving the full
    // -800/-200. Java's own comment at :506-507 says why: "to avoid small corner cutoffs".
    let mut base = BoardFixture::with_angle(AngleRestriction::None);
    base.insert_all();
    let base_id = base.build_autoroute_tree(1);
    assert_eq!(
        base.tree(base_id).get_key(),
        "ShapeSearchTree_FortyfiveDegree_cc1"
    );
    assert_eq!(
        shapes(&base, base_id, 2),
        vec![Some(oct(-650, -150, -350, 150, -741, -259, -741, -259))]
    );
    // The through pad is already an octagon, so the difference shows on its diagonals too:
    // `enlarge` moves a diagonal border line by 100 along its normal, `offset` by 100 in x.
    assert_eq!(
        shapes(&base, base_id, 3),
        vec![
            Some(oct(330, -170, 670, 170, 219, 781, 219, 781)),
            Some(oct(330, -170, 670, 170, 219, 781, 219, 781)),
        ]
    );

    let mut f45 = BoardFixture::with_angle(AngleRestriction::FortyFiveDegree);
    f45.insert_all();
    let id45 = f45.build_autoroute_tree(1);
    assert_eq!(
        shapes(&f45, id45, 2),
        vec![Some(oct(-650, -150, -350, 150, -800, -200, -800, -200))]
    );
    assert_eq!(
        shapes(&f45, id45, 3),
        vec![
            Some(oct(330, -170, 670, 170, 160, 840, 160, 840)),
            Some(oct(330, -170, 670, 170, 160, 840, 160, 840)),
        ]
    );
}

// ---------------------------------------------------------------------------------------------
// Queries
// ---------------------------------------------------------------------------------------------

#[test]
fn overlapping_objects_and_entries_come_back_in_java_order() {
    // P2T10 mode 0, `query tree=default shape=Box[-600,-100..600,500] layer=0 ignore=[] cc=1`:
    //   overlappingObjects: 4 3 2
    //   overlappingTreeEntries: 4/0 4/1 4/2 3/0 2/0
    // Descending item id (Item.compareTo's reversed subtraction, Item.java:98 — quirk #44) and
    // ascending shape index within an item (ShapeTree.Leaf.compareTo, ShapeTree.java:216-223).
    let mut f = BoardFixture::new();
    f.insert_all();
    let id = f.manager.get_default_tree().id();
    let tree = f.tree(id);
    let ctx = f.ctx();

    let objects = tree.overlapping_objects(&probe(), Some(0), &[], &f.items, &ctx);
    assert_eq!(object_ids(&objects), vec![4, 3, 2]);
    let entries = tree.overlapping_tree_entries(&probe(), Some(0), &[], &f.items, &ctx);
    assert_eq!(
        pairs(&entries),
        vec![(4, 0), (4, 1), (4, 2), (3, 0), (2, 0)]
    );
}

#[test]
fn the_layer_filter_matches_java_including_the_ignored_layer() {
    // Same driver block, `layer=-1` and `layer=1`. A negative layer means "ignore the layer"
    // (ShapeSearchTree.java:410), which is `None` here; the through pad then contributes both of
    // its shapes, and on layer 1 only its second one.
    let mut f = BoardFixture::new();
    f.insert_all();
    let id = f.manager.get_default_tree().id();
    let tree = f.tree(id);
    let ctx = f.ctx();

    let entries = tree.overlapping_tree_entries(&probe(), None, &[], &f.items, &ctx);
    assert_eq!(
        pairs(&entries),
        vec![(4, 0), (4, 1), (4, 2), (3, 0), (3, 1), (2, 0)]
    );
    let entries = tree.overlapping_tree_entries(&probe(), Some(1), &[], &f.items, &ctx);
    assert_eq!(pairs(&entries), vec![(3, 1)]);
    assert_eq!(
        object_ids(&tree.overlapping_objects(&probe(), Some(1), &[], &f.items, &ctx)),
        vec![3]
    );
}

#[test]
fn ignore_net_nos_drops_every_item_that_sits_on_one_of_them() {
    // Same driver block, `ignore=[1]`: items 2, 3 and 4 are all on net 1, so
    // `Item.isObstacle(1)` (Item.java:161-164) is false for each and Java sets `ignoreObject`
    // (ShapeSearchTree.java:412-416). Item 5, on net 2, survives — and *is* returned by the
    // clearance query, whose 1.2x-enlarged bounds reach it.
    let mut f = BoardFixture::new();
    f.insert_all();
    let id = f.manager.get_default_tree().id();
    let tree = f.tree(id);
    let ctx = f.ctx();
    let mut counter = 0;

    assert!(
        tree.overlapping_tree_entries(&probe(), Some(0), &[1], &f.items, &ctx)
            .is_empty()
    );
    assert!(
        tree.overlapping_objects(&probe(), Some(0), &[1], &f.items, &ctx)
            .is_empty()
    );
    let with_clearance = tree.overlapping_tree_entries_with_clearance(
        &probe(),
        Some(0),
        &[1],
        1,
        &f.items,
        &ctx,
        &mut counter,
    );
    assert_eq!(pairs(&with_clearance), vec![(5, 0)]);
    assert_eq!(
        tree.overlapping_items_with_clearance(
            &probe(),
            Some(0),
            &[1],
            1,
            &f.items,
            &ctx,
            &mut counter
        ),
        vec![ItemId(5)]
    );
}

#[test]
fn overlapping_tree_entries_with_clearance_reproduces_the_1_2x_walk() {
    // P2T10 mode 0. With `cc=1` every candidate has the same clearance to class 1
    //   (getValue(1,1)=200+16 for items 2/3/4 and getValue(1,2)=200+16 for item 5),
    // so the `TreeSet<EntrySortedByClearance>` falls back to the entry-id tie-break and the
    // result keeps the order `MinAreaTree.overlaps` produced: 5/0 first, then 4/0 4/1 4/2 3/0
    // 2/0. 5/1 is dropped by the half-clearance intersection test.
    //
    // With `cc=2` the clearances differ — getValue(2,1)=600+16 for items 2/3/4 and
    // getValue(2,2)=800+16 for item 5 — so the sort puts the class-1 items first and item 5
    // last, and both of its shapes now survive.
    let mut f = BoardFixture::new();
    f.insert_all();
    let id = f.manager.get_default_tree().id();
    let tree = f.tree(id);
    let ctx = f.ctx();
    let mut counter = 0;

    let entries = tree.overlapping_tree_entries_with_clearance(
        &probe(),
        Some(0),
        &[],
        1,
        &f.items,
        &ctx,
        &mut counter,
    );
    assert_eq!(
        pairs(&entries),
        vec![(5, 0), (4, 0), (4, 1), (4, 2), (3, 0), (2, 0)]
    );

    let entries = tree.overlapping_tree_entries_with_clearance(
        &probe(),
        Some(0),
        &[],
        WIDE_CLEARANCE_CLASS,
        &f.items,
        &ctx,
        &mut counter,
    );
    assert_eq!(
        pairs(&entries),
        vec![(4, 0), (4, 1), (4, 2), (3, 0), (2, 0), (5, 0), (5, 1)]
    );

    // `overlappingItemsWithClearance` is a `TreeSet<Item>`, i.e. descending id (:562).
    assert_eq!(
        tree.overlapping_items_with_clearance(
            &probe(),
            Some(0),
            &[],
            1,
            &f.items,
            &ctx,
            &mut counter
        ),
        vec![ItemId(5), ItemId(4), ItemId(3), ItemId(2)]
    );
}

#[test]
fn an_ignored_layer_makes_every_clearance_zero() {
    // P2T10 mode 0, `layer=-1`: `ClearanceMatrix.getValue` returns 0 for a negative layer
    // (ClearanceMatrix.java:132-161), so every entry sorts equal at clearance 0 and the walk
    // compares the *unenlarged* shapes — item 5 no longer reaches the probe, unlike the
    // `layer=0` run above, even though `maxValue` still clamps to layer 0 and pulls it into the
    // candidate set.
    let mut f = BoardFixture::new();
    f.insert_all();
    let id = f.manager.get_default_tree().id();
    let tree = f.tree(id);
    let ctx = f.ctx();
    let mut counter = 0;

    let entries = tree.overlapping_tree_entries_with_clearance(
        &probe(),
        None,
        &[],
        1,
        &f.items,
        &ctx,
        &mut counter,
    );
    assert_eq!(
        pairs(&entries),
        vec![(4, 0), (4, 1), (4, 2), (3, 0), (3, 1), (2, 0)]
    );
}

#[test]
fn a_compensated_tree_answers_the_plain_overlap_query() {
    // ShapeSearchTree.java:517-522: `overlappingTreeEntriesWithClearance(shape, layer, ignore,
    // cc)` short-circuits to `overlappingTreeEntries` when the tree's shapes already carry the
    // clearance. P2T10 mode 0, `query tree=autoroute_cc1`: both lines read
    // `5/0 5/1 4/0 4/1 4/2 3/0 2/0`.
    let mut f = BoardFixture::new();
    f.insert_all();
    let id = f.build_autoroute_tree(1);
    let tree = f.tree(id);
    let ctx = f.ctx();
    let mut counter = 0;

    assert!(tree.is_clearance_compensation_used());
    let plain = tree.overlapping_tree_entries(&probe(), Some(0), &[], &f.items, &ctx);
    let auto = tree.overlapping_tree_entries_with_clearance_auto(
        &probe(),
        Some(0),
        &[],
        1,
        &f.items,
        &ctx,
        &mut counter,
    );
    assert_eq!(
        pairs(&plain),
        vec![(5, 0), (5, 1), (4, 0), (4, 1), (4, 2), (3, 0), (2, 0)]
    );
    assert_eq!(pairs(&plain), pairs(&auto));
    assert_eq!(
        counter, 0,
        "the compensated branch mints no EntrySortedByClearance ids"
    );
    assert_eq!(
        object_ids(&tree.overlapping_objects(&probe(), Some(0), &[], &f.items, &ctx)),
        vec![5, 4, 3, 2]
    );
}

#[test]
fn a_small_probe_reaches_only_what_it_touches() {
    // P2T10 mode 0, `shape=Box[-520,-20..-480,20]`: `4/0 2/0` with and without clearance, and
    // nothing at all once net 1 is ignored.
    let mut f = BoardFixture::new();
    f.insert_all();
    let id = f.manager.get_default_tree().id();
    let tree = f.tree(id);
    let ctx = f.ctx();
    let mut counter = 0;
    let small = bx(-520, -20, -480, 20);

    assert_eq!(
        pairs(&tree.overlapping_tree_entries(&small, Some(0), &[], &f.items, &ctx)),
        vec![(4, 0), (2, 0)]
    );
    assert_eq!(
        pairs(&tree.overlapping_tree_entries_with_clearance(
            &small,
            Some(0),
            &[],
            1,
            &f.items,
            &ctx,
            &mut counter
        )),
        vec![(4, 0), (2, 0)]
    );
    assert!(
        tree.overlapping_tree_entries_with_clearance(
            &small,
            Some(0),
            &[1],
            1,
            &f.items,
            &ctx,
            &mut counter
        )
        .is_empty()
    );
}

#[test]
fn the_entry_id_counter_only_ever_grows() {
    // ShapeSearchTree.java:1141-1150: every `EntrySortedByClearance` takes the next id, and the
    // id is the tie-break when two candidates have equal clearance. Java's counter is a JVM-wide
    // static; the port's is per-manager (global-constraints.md), and it must stay monotonic
    // across queries so that a second query cannot reorder equal-clearance entries relative to
    // the first.
    let mut f = BoardFixture::new();
    f.insert_all();
    let id = f.manager.get_default_tree().id();
    let ctx_owner = BoardFixture::new();
    let ctx = ctx_owner.ctx();

    let mut counter = 0;
    let first = f.tree(id).overlapping_tree_entries_with_clearance(
        &probe(),
        Some(0),
        &[],
        1,
        &f.items,
        &ctx,
        &mut counter,
    );
    let after_first = counter;
    assert!(after_first >= first.len() as u64);
    let second = f.tree(id).overlapping_tree_entries_with_clearance(
        &probe(),
        Some(0),
        &[],
        1,
        &f.items,
        &ctx,
        &mut counter,
    );
    assert!(
        counter > after_first,
        "the second query must mint fresh ids, not reuse the first query's"
    );
    assert_eq!(
        pairs(&first),
        pairs(&second),
        "a monotonic counter keeps the answer stable across queries"
    );
}

// ---------------------------------------------------------------------------------------------
// In-place entry surgery
// ---------------------------------------------------------------------------------------------

#[test]
fn change_item_shape_replaces_one_leaf_and_leaves_the_rest_alone() {
    // P2T10 mode 3, `--- changeItemShape(trace 4, 1, Box[-20,-20..20,420])`: the stored shape
    // becomes the box verbatim, its leaf's bounding shape becomes that box's octagon, and the
    // `toArray()` order is unchanged — the removed slot is handed straight back by the arena's
    // LIFO free list, which is why `LeafId` carries a generation counter.
    let mut f = BoardFixture::new();
    f.insert_all();
    let id = f.manager.get_default_tree().id();

    let mut item = f.items.remove(&ItemId(4)).expect("item 4");
    f.manager
        .get_default_tree_mut()
        .change_item_shape(&mut item, 1, bx(-20, -20, 20, 420));
    f.items.insert(ItemId(4), item);

    assert_eq!(
        shapes(&f, id, 4),
        vec![
            Some(oct(-530, -30, 30, 30, -542, 42, -542, 42)),
            Some(bx(-20, -20, 20, 420)),
            Some(oct(-30, 370, 530, 430, -442, 142, 358, 942)),
        ]
    );
    let observed: Vec<(u32, usize)> = leaves(f.tree(id))
        .into_iter()
        .map(|(id, index, _)| (id, index))
        .collect();
    assert_eq!(
        observed,
        vec![
            (2, 0),
            (5, 0),
            (5, 1),
            (4, 0),
            (4, 1),
            (4, 2),
            (3, 0),
            (3, 1)
        ]
    );
    assert_eq!(
        leaves(f.tree(id))[4].2,
        oct(-20, -20, 20, 420, -440, 40, -40, 440)
    );
    assert!(f.manager.validate_entries(&f.items[&ItemId(4)]));
}

#[test]
fn change_entries_rebuilds_only_the_middle_of_a_trace() {
    // P2T10 mode 3, `--- changeEntries(trace 4, shifted polyline, keepStart=1, keepEnd=1)`: the
    // trace's middle segment is stretched from y=400 to y=600, and only that one entry is
    // replaced. `validateEntries` stays true (ShapeSearchTree.java:1120-1130).
    let mut f = BoardFixture::new();
    f.insert_all();
    let id = f.manager.get_default_tree().id();

    let shifted = Polyline::from_points(&[
        Point::new(-500, 0),
        Point::new(0, 0),
        Point::new(0, 600),
        Point::new(500, 600),
    ]);
    let mut item = f.items.remove(&ItemId(4)).expect("item 4");
    {
        let Item::Trace(trace) = &mut item else {
            panic!("item 4 is the signal trace")
        };
        let rules = &f.rules;
        f.manager
            .get_default_tree_mut()
            .change_entries(trace, &shifted, 1, 1, rules);
    }
    f.items.insert(ItemId(4), item);

    assert_eq!(
        shapes(&f, id, 4),
        vec![
            Some(oct(-530, -30, 30, 30, -542, 42, -542, 42)),
            Some(oct(-30, -30, 30, 630, -642, 42, -42, 642)),
            Some(oct(-30, 370, 530, 430, -442, 142, 358, 942)),
        ]
    );
    let observed: Vec<(u32, usize)> = leaves(f.tree(id))
        .into_iter()
        .map(|(id, index, _)| (id, index))
        .collect();
    assert_eq!(
        observed,
        vec![
            (2, 0),
            (5, 0),
            (5, 1),
            (4, 0),
            (4, 1),
            (4, 2),
            (3, 0),
            (3, 1)
        ]
    );
    assert!(f.manager.validate_entries(&f.items[&ItemId(4)]));
}

// ---------------------------------------------------------------------------------------------
// SearchTreeManager
// ---------------------------------------------------------------------------------------------

#[test]
fn insert_and_remove_track_on_the_board_and_the_entry_arrays() {
    // SearchTreeManager.java:39-62.
    let mut f = BoardFixture::new();
    assert!(f.items.values().all(|item| !item.is_on_the_board()));
    f.insert_all();
    assert!(f.items.values().all(Item::is_on_the_board));
    let id = f.manager.get_default_tree().id();
    assert_eq!(f.tree(id).size(), 8);

    let mut item = f.items.remove(&ItemId(4)).expect("item 4");
    f.manager.remove(&mut item);
    assert!(!item.is_on_the_board());
    assert!(item.get_search_tree_entries(id).is_none());
    // `clearSearchTreeEntries` drops the cached shapes too (Item.java:1033-1036).
    assert_eq!(item.tree_shape_count(id), 0);
    f.items.insert(ItemId(4), item);
    assert_eq!(f.tree(id).size(), 5);

    // A second remove is a no-op (SearchTreeManager.java:48-50).
    let mut item = f.items.remove(&ItemId(4)).expect("item 4");
    f.manager.remove(&mut item);
    f.items.insert(ItemId(4), item);
    assert_eq!(f.tree(id).size(), 5);
}

#[test]
fn get_autoroute_tree_reuses_a_tree_with_the_same_compensated_class() {
    // SearchTreeManager.java:141-145: the search runs over `compensatedSearchTrees`, which
    // *includes* the default tree, so asking for the default tree's own class answers it.
    let mut f = BoardFixture::new();
    f.insert_all();
    let default_id = f.manager.get_default_tree().id();
    assert_eq!(f.build_autoroute_tree(0), default_id);

    let first = f.build_autoroute_tree(1);
    assert_ne!(first, default_id);
    assert_eq!(f.build_autoroute_tree(1), first);
    let second = f.build_autoroute_tree(2);
    assert_ne!(second, first);
    assert_eq!(
        f.tree(second).get_key(),
        "ShapeSearchTree45Degree_FortyfiveDegree_cc2"
    );

    // SearchTreeManager.java:178-180.
    f.manager.reset_compensated_trees();
    assert_eq!(f.manager.trees().count(), 1);
    assert_eq!(f.manager.get_default_tree().id(), default_id);
}

#[test]
fn clearance_class_removed_refuses_to_drop_the_default_tree() {
    // SearchTreeManager.java:122-134.
    let mut f = BoardFixture::new();
    f.insert_all();
    let auto = f.build_autoroute_tree(1);
    assert_eq!(f.manager.trees().count(), 2);

    // The default tree is compensated for class 0; Java warns and returns.
    f.manager.clearance_class_removed(0);
    assert_eq!(f.manager.trees().count(), 2);

    f.manager.clearance_class_removed(1);
    assert_eq!(f.manager.trees().count(), 1);
    assert!(f.manager.trees().all(|tree| tree.id() != auto));
}

#[test]
fn set_clearance_compensation_used_rebuilds_the_default_tree_as_the_base_class() {
    // P2T10 mode 3, `--- setClearanceCompensationUsed(true)`: the new default tree's key is
    // `ShapeSearchTree_FortyfiveDegree_cc1` — still the **base** class, so the SMD pad's stored
    // shape is `enlarge`d (diagonals -741/-259), not `offset` as the 45-degree subclass would.
    // Every autoroute tree is discarded with it (SearchTreeManager.java:96).
    let mut f = BoardFixture::new();
    f.insert_all();
    f.build_autoroute_tree(1);
    assert_eq!(f.manager.trees().count(), 2);

    let mut items = std::mem::take(&mut f.items);
    let ctx = ItemCtx {
        library: &f.library,
        components: &f.components,
        rules: &f.rules,
        bounding_box: &f.bounding_box,
        max_tree_shape_width: DEFAULT_MAX_TREE_SHAPE_WIDTH,
    };
    let mut refs: Vec<&mut Item> = items.values_mut().rev().collect();
    f.manager
        .set_clearance_compensation_used(true, &mut refs, &ctx);
    drop(refs);
    f.items = items;

    assert!(f.manager.is_clearance_compensation_used());
    assert_eq!(f.manager.trees().count(), 1);
    let id = f.manager.get_default_tree().id();
    assert_eq!(
        f.manager.get_default_tree().get_key(),
        "ShapeSearchTree_FortyfiveDegree_cc1"
    );
    assert_eq!(f.tree(id).size(), 8);
    // The rebuild walks `board.itemList`, i.e. descending id, so the tree comes out with the
    // same structure the autoroute trees have rather than the default tree's.
    let observed: Vec<(u32, usize)> = leaves(f.tree(id))
        .into_iter()
        .map(|(id, index, _)| (id, index))
        .collect();
    assert_eq!(
        observed,
        vec![
            (5, 0),
            (2, 0),
            (4, 0),
            (3, 0),
            (3, 1),
            (4, 1),
            (5, 1),
            (4, 2)
        ]
    );
    assert_eq!(
        shapes(&f, id, 2),
        vec![Some(oct(-650, -150, -350, 150, -741, -259, -741, -259))]
    );
    assert_eq!(
        shapes(&f, id, 3),
        vec![
            Some(oct(330, -170, 670, 170, 219, 781, 219, 781)),
            Some(oct(330, -170, 670, 170, 219, 781, 219, 781)),
        ]
    );
    // The compensated tree answers the plain overlap query (:517-522), and now reaches item 5.
    let ctx = f.ctx();
    let mut counter = 0;
    assert_eq!(
        pairs(&f.tree(id).overlapping_tree_entries_with_clearance_auto(
            &probe(),
            Some(0),
            &[],
            1,
            &f.items,
            &ctx,
            &mut counter
        )),
        vec![(5, 0), (5, 1), (4, 0), (4, 1), (4, 2), (3, 0), (2, 0)]
    );

    // Setting the same value again is a no-op (SearchTreeManager.java:90-92).
    let before = f.manager.get_default_tree().id();
    let mut items = std::mem::take(&mut f.items);
    let ctx = ItemCtx {
        library: &f.library,
        components: &f.components,
        rules: &f.rules,
        bounding_box: &f.bounding_box,
        max_tree_shape_width: DEFAULT_MAX_TREE_SHAPE_WIDTH,
    };
    let mut refs: Vec<&mut Item> = items.values_mut().rev().collect();
    f.manager
        .set_clearance_compensation_used(true, &mut refs, &ctx);
    drop(refs);
    f.items = items;
    assert_eq!(f.manager.get_default_tree().id(), before);
}

#[test]
fn reinsert_tree_shapes_recomputes_after_a_rule_change() {
    // SearchTreeManager.java:186-200: removal clears the tree entries but not the precalculated
    // shapes, so `reinsertTreeItems` calls `clearDerivedData()` in between. Raising the hole
    // clearance changes the drill items' shapes (ShapeSearchTree.java:1035-1074), which can only
    // reach the tree through that clear.
    let mut f = BoardFixture::new();
    f.insert_all();
    let id = f.manager.get_default_tree().id();
    let before = shapes(&f, id, 3);

    f.rules.set_hole_clearance(500);
    let mut items = std::mem::take(&mut f.items);
    let ctx = ItemCtx {
        library: &f.library,
        components: &f.components,
        rules: &f.rules,
        bounding_box: &f.bounding_box,
        max_tree_shape_width: DEFAULT_MAX_TREE_SHAPE_WIDTH,
    };
    let mut refs: Vec<&mut Item> = items.values_mut().rev().collect();
    f.manager.reinsert_tree_shapes(&mut refs, &ctx);
    drop(refs);
    f.items = items;

    let after = shapes(&f, id, 3);
    assert_ne!(
        before, after,
        "a hole-clearance change must reach the tree shapes"
    );
    assert_eq!(f.tree(id).size(), 8);
    assert!(f.items.values().all(Item::is_on_the_board));
}

// ---------------------------------------------------------------------------------------------
// Areas, outlines and drill holes (`P2T10.java` mode 4)
// ---------------------------------------------------------------------------------------------

fn area_shapes(f: &board_builder::AreaFixture, tree: TreeId, id: u32) -> Vec<Option<TileShape>> {
    let item = &f.items[&ItemId(id)];
    (0..item.tree_shape_count(tree))
        .map(|i| item.get_tree_shape(tree, i).cloned())
        .collect()
}

#[test]
fn obstacle_and_conduction_areas_are_split_enlarged_and_regularised() {
    // P2T10 mode 4. The base tree stores the convex pieces verbatim (`enlarge(0)`); the
    // 45-degree tree enlarges each by the compensation (100) and then takes the bounding
    // octagon (ShapeSearchTree.java:922-931 + ShapeSearchTree45Degree.java:522-529). The L
    // splits into two boxes, in `PolygonShape.splitToConvex` order.
    let mut f = board_builder::AreaFixture::new();
    let default_id = f.manager.get_default_tree().id();
    assert_eq!(
        area_shapes(&f, default_id, 3),
        vec![Some(bx(0, 1000, 1000, 2000)), Some(bx(0, 0, 2000, 1000))]
    );
    assert_eq!(
        area_shapes(&f, default_id, 4),
        vec![Some(bx(-2000, -1500, -1000, -500))]
    );

    let auto = f.build_autoroute_tree(1);
    assert_eq!(
        area_shapes(&f, auto, 3),
        vec![
            Some(oct(-100, 900, 1100, 2100, -2141, 141, 859, 3141)),
            Some(oct(-100, -100, 2100, 1100, -1141, 2141, -141, 3141)),
        ]
    );
    assert_eq!(
        area_shapes(&f, auto, 4),
        vec![Some(oct(
            -2100, -1600, -900, -400, -1641, 641, -3641, -1359
        ))]
    );
}

#[test]
fn a_board_outline_contributes_line_bands_per_layer_and_keepout_pieces_when_asked() {
    // P2T10 mode 4. Without the keepout the outline is `lineCount() * layerCount` = 4 * 3 = 12
    // bands, layer-major, each `Polyline.offsetShape(halfWidth + cmp, 0)` over a border-line
    // triple (ShapeSearchTree.java:966-987). With it, the convex pieces of the *outside* area,
    // again once per layer (:945-964).
    let mut f = board_builder::AreaFixture::new();
    let default_id = f.manager.get_default_tree().id();
    let bands = area_shapes(&f, default_id, 1);
    assert_eq!(bands.len(), 12);
    // Layer-major: index i and i + 4 and i + 8 are the same band on three layers.
    for i in 0..4 {
        assert_eq!(bands[i], bands[i + 4]);
        assert_eq!(bands[i], bands[i + 8]);
    }
    assert_eq!(
        bands[0],
        Some(oct(-3100, -2100, 3100, -1900, -1141, 5141, -5141, 1141))
    );
    assert_eq!(
        bands[3],
        Some(oct(-3100, -2100, -2900, 2100, -5141, -859, -5141, -859))
    );
    assert_eq!(f.tree(default_id).size(), 18);

    f.generate_keepout_outside();
    let keepout = area_shapes(&f, default_id, 1);
    assert_eq!(keepout.len(), 12);
    assert_eq!(
        keepout[0],
        Some(oct(-5000, -5000, 5000, -2000, -3000, 10000, -10000, 3000))
    );
    assert_eq!(
        keepout[2],
        Some(oct(3000, -2000, 5000, 2000, 1000, 7000, 1000, 7000))
    );
    assert_eq!(f.tree(default_id).size(), 18);
}

#[test]
fn a_drill_layer_without_a_pad_falls_back_to_the_synthesised_hole_obstacle() {
    // P2T10 mode 4. The via's padstack has a pad on layers 0 and 2 and `null` on layer 1, so
    // `calculateTreeShapes(DrillItem)` takes `drillHoleObstacle` for index 1
    // (ShapeSearchTree.java:877-880) — a circle of the drill radius around the centre — and
    // inflates every index by `drillHoleClearanceDelta` (:896). The two pad layers come out
    // 146 wide either side of x=1000, the hole layer 146 as well but with different diagonals,
    // because one is a box's octagon and the other a circle's.
    let f = board_builder::AreaFixture::new();
    let default_id = f.manager.get_default_tree().id();
    assert_eq!(
        area_shapes(&f, default_id, 2),
        vec![
            Some(oct(854, -146, 1146, 146, 747, 1253, 747, 1253)),
            Some(oct(854, -146, 1146, 146, 794, 1207, 794, 1207)),
            Some(oct(854, -146, 1146, 146, 747, 1253, 747, 1253)),
        ]
    );
    // Nothing is `None`: with `holeClearance > 0` and a drill radius, every layer gets a shape.
    assert_eq!(f.items[&ItemId(2)].tree_shape_count(default_id), 3);
}

#[test]
fn a_drill_layer_without_a_pad_has_no_leaf_when_hole_clearance_is_off() {
    // The other half of ShapeSearchTree.java:877-882: with `holeClearance == 0`,
    // `drillHoleObstacle` returns null (:1015) and the tree shape for that index is `null`, so
    // `ShapeTree.insert` leaves the index leafless (ShapeTree.java:46-49) while keeping its slot
    // — `DrillItem.shapeLayer(index)` is `firstLayer() + index` (DrillItem.java:147-154), so
    // dropping it would renumber the layers.
    let mut f = board_builder::AreaFixture::new();
    f.rules.set_hole_clearance(0);
    let mut items = std::mem::take(&mut f.items);
    let ctx = ItemCtx {
        library: &f.library,
        components: &f.components,
        rules: &f.rules,
        bounding_box: &f.bounding_box,
        max_tree_shape_width: DEFAULT_MAX_TREE_SHAPE_WIDTH,
    };
    let mut refs: Vec<&mut Item> = items.values_mut().rev().collect();
    f.manager.reinsert_tree_shapes(&mut refs, &ctx);
    drop(refs);
    f.items = items;

    let default_id = f.manager.get_default_tree().id();
    let shapes = area_shapes(&f, default_id, 2);
    assert_eq!(shapes.len(), 3, "the empty layer keeps its slot");
    assert!(
        shapes[1].is_none(),
        "layer 1 has neither pad nor hole shape"
    );
    assert_eq!(
        f.items[&ItemId(2)]
            .get_search_tree_entries(default_id)
            .expect("entries")
            .iter()
            .filter(|leaf| leaf.is_some())
            .count(),
        2,
        "only the two pad layers get a leaf"
    );
    // The via's own shapes are `enlarge(0)`d, i.e. the pad's plain octagon.
    assert_eq!(
        shapes[0],
        Some(oct(920, -80, 1080, 80, 840, 1160, 840, 1160))
    );
}

// ---------------------------------------------------------------------------------------------
// The entry-surgery family (`P2T10.java` modes 5 and 6)
// ---------------------------------------------------------------------------------------------

/// The joined polyline both merge tests hand over: the two fixture traces end to end.
fn joined_polyline() -> Polyline {
    Polyline::from_points(&[
        Point::new(-500, 0),
        Point::new(0, 0),
        Point::new(0, 400),
        Point::new(500, 400),
        Point::new(500, 900),
    ])
}

fn trace_shapes(f: &board_builder::TraceFixture, id: u32) -> Vec<Option<TileShape>> {
    let tree = f.tree().id();
    let item = &f.items[&ItemId(id)];
    (0..item.tree_shape_count(tree))
        .map(|i| item.get_tree_shape(tree, i).cloned())
        .collect()
}

#[test]
fn merge_entries_at_end_appends_the_source_trace_behind_the_link_shapes() {
    // P2T10 mode 5. `toTrace` keeps its head entry, the two link shapes are inserted, and
    // `fromTrace`'s tail entry is re-keyed onto `toTrace` at its new index
    // (ShapeSearchTree.java:279-296). `fromTrace` keeps its own (now stale) entry array, which
    // is what the caller removes next.
    let mut f = board_builder::TraceFixture::new();
    assert_eq!(f.tree().size(), 4);
    let mut trace_a = f.take_trace(2);
    let mut trace_b = f.take_trace(3);
    let rules_snapshot = f.rules.clone();
    let tree = f.manager.get_default_tree_mut();
    tree.merge_entries_at_end(
        &mut trace_b,
        &mut trace_a,
        &joined_polyline(),
        1,
        4,
        &rules_snapshot,
    );
    f.put_trace(2, trace_a);
    f.put_trace(3, trace_b);

    assert_eq!(f.tree().size(), 4);
    assert_eq!(
        leaves(f.tree())
            .into_iter()
            .map(|(id, index, _)| (id, index))
            .collect::<Vec<_>>(),
        vec![(2, 0), (2, 1), (2, 2), (2, 3)],
        "every leaf now belongs to the target trace"
    );
    assert_eq!(
        trace_shapes(&f, 2),
        vec![
            Some(oct(-530, -30, 30, 30, -542, 42, -542, 42)),
            Some(oct(-30, -30, 30, 430, -442, 42, -42, 442)),
            Some(oct(-30, 370, 530, 430, -442, 142, 358, 942)),
            Some(oct(470, 370, 530, 930, -442, 142, 858, 1442)),
        ]
    );
    assert_eq!(trace_shapes(&f, 3).len(), 2);
    assert!(f.manager.validate_entries(&f.items[&ItemId(2)]));
}

#[test]
fn merge_entries_in_front_prepends_the_source_trace_before_the_link_shapes() {
    // P2T10 mode 5, the mirror image: `fromTrace`'s entries move to the *front* of `toTrace`,
    // re-keyed onto it (ShapeSearchTree.java:205-216), and `toTrace`'s own tail entries are
    // renumbered in place (:217-222).
    let mut f = board_builder::TraceFixture::new();
    let mut trace_a = f.take_trace(2);
    let mut trace_b = f.take_trace(3);
    let rules_snapshot = f.rules.clone();
    let tree = f.manager.get_default_tree_mut();
    tree.merge_entries_in_front(
        &mut trace_a,
        &mut trace_b,
        &joined_polyline(),
        1,
        4,
        &rules_snapshot,
    );
    f.put_trace(2, trace_a);
    f.put_trace(3, trace_b);

    assert_eq!(f.tree().size(), 4);
    assert_eq!(
        leaves(f.tree())
            .into_iter()
            .map(|(id, index, _)| (id, index))
            .collect::<Vec<_>>(),
        vec![(3, 0), (3, 1), (3, 2), (3, 3)]
    );
    assert_eq!(
        trace_shapes(&f, 3),
        vec![
            Some(oct(-530, -30, 30, 30, -542, 42, -542, 42)),
            Some(oct(-30, -30, 30, 430, -442, 42, -42, 442)),
            Some(oct(-30, 370, 530, 430, -442, 142, 358, 942)),
            Some(oct(470, 370, 530, 930, -442, 142, 858, 1442)),
        ]
    );
    assert!(f.manager.validate_entries(&f.items[&ItemId(3)]));
}

#[test]
fn reuse_entries_after_cutout_hands_the_ends_over_and_holes_the_source_array() {
    // P2T10 mode 5. Only the *first* entry of the start piece and the *last* of the end piece
    // are transferred (`startPieceLeafArr.length - 1` and the `1..endLen` loop,
    // ShapeSearchTree.java:323-345); the two leaves at the new cut are minted. The transferred
    // slots are nulled in the source array, which is exactly the holed `Leaf[]`
    // `ShapeTree.remove` has to tolerate — the middle entries stay, so removing the source trace
    // afterwards deletes them and nothing else.
    let mut f = board_builder::TraceFixture::new();
    let mut long_item = Item::Trace(board_builder::trace_piece(
        4,
        &[
            (-1000, -1000),
            (-1000, 0),
            (0, 0),
            (0, 1000),
            (1000, 1000),
            (1000, 2000),
        ],
    ));
    let ctx = ItemCtx {
        library: &f.library,
        components: &f.components,
        rules: &f.rules,
        bounding_box: &f.bounding_box,
        max_tree_shape_width: DEFAULT_MAX_TREE_SHAPE_WIDTH,
    };
    f.manager.insert(&mut long_item, &ctx);
    assert_eq!(f.manager.get_default_tree().size(), 9);

    let Item::Trace(mut long_trace) = long_item else {
        unreachable!()
    };
    let mut start_piece = board_builder::trace_piece(5, &[(-1000, -1000), (-1000, 0), (0, 0)]);
    let mut end_piece = board_builder::trace_piece(6, &[(0, 1000), (1000, 1000), (1000, 2000)]);
    f.manager.get_default_tree_mut().reuse_entries_after_cutout(
        &mut long_trace,
        &mut start_piece,
        &mut end_piece,
        &ctx,
    );

    let tree_id = f.manager.get_default_tree().id();
    let from_entries = long_trace
        .hdr
        .get_tree_entries(tree_id)
        .expect("entries")
        .to_vec();
    assert_eq!(from_entries.len(), 5);
    assert!(
        from_entries[0].is_none(),
        "index 0 was handed to the start piece"
    );
    assert!(
        from_entries[4].is_none(),
        "index 4 was handed to the end piece"
    );
    assert!(from_entries[1..4].iter().all(Option::is_some));
    assert_eq!(
        start_piece
            .hdr
            .get_tree_entries(tree_id)
            .expect("entries")
            .len(),
        2
    );
    assert_eq!(
        end_piece
            .hdr
            .get_tree_entries(tree_id)
            .expect("entries")
            .len(),
        2
    );
    // Two leaves were minted (the two new cut ends), none removed.
    assert_eq!(f.manager.get_default_tree().size(), 11);
    assert!(
        f.manager
            .get_default_tree()
            .validate_entries(&Item::Trace(start_piece))
    );
    assert!(
        f.manager
            .get_default_tree()
            .validate_entries(&Item::Trace(end_piece))
    );
}

#[test]
fn reduce_trace_shape_at_tie_pin_cuts_the_pin_out_of_the_end_tile() {
    // P2T10 mode 6: the SMD pin of the main fixture sits exactly on the trace's first corner, so
    // `reduceTraceShapeAtTiePin` (ShapeSearchTree.java:817-846) cuts the pin's tree shape out of
    // the trace's first tile. `[-530, -30 .. 30, 30]` becomes `[-450, -30 .. 30, 30]` — the pin
    // shape reaches to x = -450 — and the leaf is replaced through `changeItemShape`.
    let mut f = BoardFixture::new();
    f.insert_all();
    let id = f.manager.get_default_tree().id();

    let Item::Pin(pin) = f.items[&ItemId(2)].clone() else {
        panic!("item 2 is the SMD pin")
    };
    let Item::Trace(mut trace) = f.items.remove(&ItemId(4)).expect("item 4") else {
        panic!("item 4 is the signal trace")
    };
    {
        let ctx = ItemCtx {
            library: &f.library,
            components: &f.components,
            rules: &f.rules,
            bounding_box: &f.bounding_box,
            max_tree_shape_width: DEFAULT_MAX_TREE_SHAPE_WIDTH,
        };
        f.manager
            .get_default_tree_mut()
            .reduce_trace_shape_at_tie_pin(&pin, &mut trace, &ctx);
    }
    f.items.insert(ItemId(4), Item::Trace(trace));

    assert_eq!(
        shapes(&f, id, 4),
        vec![
            Some(oct(-450, -30, 30, 30, -480, 42, -480, 42)),
            Some(oct(-30, -30, 30, 430, -442, 42, -42, 442)),
            Some(oct(-30, 370, 530, 430, -442, 142, 358, 942)),
        ]
    );
    assert_eq!(f.tree(id).size(), 8);
    assert!(f.manager.validate_entries(&f.items[&ItemId(4)]));
}

#[test]
fn the_45_degree_override_regularises_the_outline_line_bands_too() {
    // P2T10 mode 7. `ShapeSearchTree45Degree.calculateTreeShapes(BoardOutline)`
    // (ShapeSearchTree45Degree.java:532-541) maps `boundingOctagon()` over the *whole* result of
    // `super.calculateTreeShapes(outline)`, which includes the line-band branch
    // (ShapeSearchTree.java:966-987) — not just the keepout one. Around an edge that runs in
    // none of the tree's directions the band is a `Simplex`, so the mapping is not a no-op: the
    // base tree stores `Simplex`es and the 45-degree tree the octagons around them.
    let mut f = board_builder::TraceFixture::skewed_outline();
    let default_id = f.manager.get_default_tree().id();
    let outline = &f.items[&ItemId(1)];
    assert_eq!(
        outline.tree_shape_count(default_id),
        6,
        "3 edges x 2 layers"
    );
    assert!(
        (0..6).all(|i| matches!(
            outline.get_tree_shape(default_id, i),
            Some(TileShape::Simplex(_))
        )),
        "the base tree keeps the raw offset shapes"
    );

    let auto = f.build_autoroute_tree(1);
    let outline = &f.items[&ItemId(1)];
    assert!(
        (0..6).all(|i| matches!(outline.get_tree_shape(auto, i), Some(TileShape::Octagon(_)))),
        "the 45-degree tree regularises every band"
    );
    assert_eq!(
        outline.get_tree_shape(auto, 0).cloned(),
        Some(oct(-200, -200, 3200, 700, -283, 2783, -283, 3783))
    );
    assert_eq!(
        outline.get_tree_shape(auto, 1).cloned(),
        Some(oct(800, 300, 3200, 2700, -1783, 2783, 3217, 3783))
    );
    assert_eq!(
        outline.get_tree_shape(auto, 2).cloned(),
        Some(oct(-200, -200, 1200, 2700, -1783, 283, -283, 3783))
    );
    // Layer-major: indices 3..6 repeat 0..3.
    for i in 0..3 {
        assert_eq!(
            outline.get_tree_shape(auto, i).cloned(),
            outline.get_tree_shape(auto, i + 3).cloned()
        );
    }
}
