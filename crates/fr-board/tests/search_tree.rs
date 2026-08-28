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
