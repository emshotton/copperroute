//! Plan 2 Task 15: property-style consistency tests for `Board` and its search trees.
//!
//! These are not transcribed from a Java driver line-for-line (unlike every other file in
//! `tests/`) — they check invariants the port must hold *by construction*, the way
//! `scripts/differential/rust/src/bin/p2t15.rs` checks them empirically against the JVM. Every
//! random input here comes from a tiny xorshift generator (no `rand` dependency), seeded so a
//! failure is reproducible; every test is well under a second in debug.

mod board_builder;

use board_builder::{WIDE_CLEARANCE_CLASS, p2t11_board};
use fr_board::prelude::*;
use fr_geometry::{Area, IntBox, Shape, TileShape};

/// A minimal xorshift64* stream — the same shape as every differential driver's `Rng`
/// (`scripts/differential/rust/src/bin/p2t3r.rs` and `p2t15.rs`), kept local here because these
/// are unit tests, not a differential driver: no `rand` crate, no shared seed with any Java side.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn rnd(&mut self, bound: u64) -> i64 {
        (self.next() % bound) as i64
    }
}

fn probe() -> TileShape {
    TileShape::Box(IntBox::from_coords(-100, -100, 100, 100))
}

/// `SearchTreeManager::get_autoroute_tree` needs `&mut [&mut Item]` (from `board.items`) and an
/// [`ItemCtx`] (from `board.{library,components,rules,bounding_box}`) simultaneously, plus
/// `&mut board.trees` — four disjoint fields. `Board::ctx()` cannot be used here: it takes
/// `&self`, so it borrows the *whole* board, which would conflict with `&mut board.trees` and
/// `&mut board.items` right after. Building the [`ItemCtx`] from the fields directly (as
/// [`board_builder::BoardFixture::build_autoroute_tree`] does for its own, non-`Board` fields)
/// keeps the four borrows disjoint instead. `max_tree_shape_width` is
/// [`DEFAULT_MAX_TREE_SHAPE_WIDTH`] because every board built by this file uses
/// `Communication::default()` (no host CAD) — the one value `Board::new` would have filled in.
fn build_autoroute_tree(board: &mut Board, clearance_class_index: usize) -> TreeId {
    let mut items = std::mem::take(&mut board.items);
    let ctx = ItemCtx {
        library: &board.library,
        components: &board.components,
        rules: &board.rules,
        bounding_box: &board.bounding_box,
        max_tree_shape_width: DEFAULT_MAX_TREE_SHAPE_WIDTH,
    };
    let mut refs: Vec<&mut Item> = items.values_mut().rev().collect();
    let id = board
        .trees
        .get_autoroute_tree(clearance_class_index, &mut refs, &ctx)
        .id();
    drop(refs);
    board.items = items;
    id
}

// ---------------------------------------------------------------------------------------------
// Insert/remove round-trips
// ---------------------------------------------------------------------------------------------

/// Inserting an item and immediately removing it must leave the default tree's leaf count and
/// every overlap query exactly as they were — a round trip is observationally a no-op.
#[test]
fn insert_remove_round_trips_leave_leaf_count_and_queries_unchanged() {
    let mut board = p2t11_board();
    let leaf_count_before = board.trees.get_default_tree().tree().leaf_count();
    let query_0_before = board.overlapping_objects(&probe(), Some(0));
    let query_1_before = board.overlapping_objects(&probe(), Some(1));

    let mut rng = Rng(0x2545_F491_4F6C_DD1D);
    for _ in 0..40 {
        let x = rng.rnd(8000) as i32 - 4000;
        let y = rng.rnd(8000) as i32 - 4000;
        let w = 50 + rng.rnd(400) as i32;
        let h = 50 + rng.rnd(400) as i32;
        let layer = rng.rnd(2) as usize;
        let cc = 1 + rng.rnd(2) as usize;

        let id = board.insert_obstacle(
            Area::Shape(Shape::Tile(TileShape::Box(IntBox::from_coords(
                x,
                y,
                x + w,
                y + h,
            )))),
            layer,
            cc,
            FixedState::Unfixed,
        );
        assert!(board.remove_item(id));

        assert_eq!(
            board.trees.get_default_tree().tree().leaf_count(),
            leaf_count_before,
            "leaf count must be restored after an insert/remove round trip"
        );
        assert_eq!(
            board.overlapping_objects(&probe(), Some(0)),
            query_0_before,
            "layer-0 overlap query must be restored after an insert/remove round trip"
        );
        assert_eq!(
            board.overlapping_objects(&probe(), Some(1)),
            query_1_before,
            "layer-1 overlap query must be restored after an insert/remove round trip"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// deep_copy
// ---------------------------------------------------------------------------------------------

#[test]
fn deep_copy_is_hash_equal_and_query_equal() {
    let mut board = p2t11_board();
    let copy = board.deep_copy();

    // `structural_hash`-equality is the comparison the brief asks for: the two hashes are not
    // byte-comparable to anything on the Java side (`docs/java-quirks.md`), only to each other.
    assert_eq!(
        board.structural_hash(),
        copy.structural_hash(),
        "a fresh deep_copy must hash-equal its original"
    );
    assert_eq!(
        board.overlapping_objects(&probe(), Some(0)),
        copy.overlapping_objects(&probe(), Some(0))
    );
    assert_eq!(
        board.overlapping_objects(&probe(), Some(1)),
        copy.overlapping_objects(&probe(), Some(1))
    );

    // The copy must be independent: mutating the original afterwards must not touch it, and the
    // hash must actually be sensitive to the mutation (otherwise the equality check above would
    // be vacuous). Item 4 is a trace — `structural_hash` only hashes traces and vias (its own
    // doc comment, `board/snapshot.rs`), so removing an `ObstacleArea` like item 7 would not
    // move the hash at all and the check would be meaningless.
    assert!(matches!(board.get_item(ItemId(4)), Some(Item::Trace(_))));
    assert!(board.remove_item(ItemId(4)));
    assert_ne!(
        board.structural_hash(),
        copy.structural_hash(),
        "removing a trace from the original must actually change its hash, otherwise the \
         equality check above would be vacuous"
    );
    assert!(
        copy.get_item(ItemId(4)).is_some(),
        "the copy must still have item 4 after the original removes it"
    );
}

// ---------------------------------------------------------------------------------------------
// normalize_traces idempotence
// ---------------------------------------------------------------------------------------------

#[test]
fn normalize_traces_is_idempotent() {
    let mut board = p2t11_board();
    // Net 1 carries the two traces the via joins (`board_builder::p2t11_board`'s doc table);
    // the first call is expected to actually do something (BasicBoard.java's own contract), the
    // second must be a no-op.
    board
        .normalize_traces(1)
        .expect("normalize_traces must not fail on a well-formed board");
    let hash_after_first = board.structural_hash();

    let changed_second = board
        .normalize_traces(1)
        .expect("a second normalize_traces call must not fail either");
    assert!(
        !changed_second,
        "normalizing an already-normal net a second time must report no change"
    );
    assert_eq!(
        board.structural_hash(),
        hash_after_first,
        "a second normalize_traces call must not alter the board"
    );
}

// ---------------------------------------------------------------------------------------------
// 45-degree vs. 90-degree autoroute tree
// ---------------------------------------------------------------------------------------------

/// The 45-degree tree's per-item tile shapes are cut against four supporting direction families
/// (horizontal, vertical, both diagonals); the 90-degree tree's are cut against only two
/// (horizontal, vertical). More supporting directions means a tighter (or equal) fit, so — for
/// the very same items, inserted through the very same board otherwise — every 90-degree
/// bounding box must contain its 45-degree counterpart.
#[test]
fn forty_five_degree_tile_shapes_are_never_looser_than_ninety_degree_ones() {
    let mut board_45 = p2t11_board();
    assert_eq!(
        board_45.rules.trace_angle_restriction,
        AngleRestriction::FortyFiveDegree,
        "BoardRules' own default (BoardRules.java:31)"
    );
    let mut board_90 = board_45.clone();
    board_90.rules.trace_angle_restriction = AngleRestriction::NinetyDegree;

    // `WIDE_CLEARANCE_CLASS` (2) is not the default tree's own compensated class (0), so this
    // actually builds a fresh, angle-restricted autoroute tree on each board rather than handing
    // back the (angle-independent) default tree.
    let tree_45 = build_autoroute_tree(&mut board_45, WIDE_CLEARANCE_CLASS);
    let tree_90 = build_autoroute_tree(&mut board_90, WIDE_CLEARANCE_CLASS);

    let ids = board_45.items_in_board_order();
    assert_eq!(ids, board_90.items_in_board_order());

    let mut compared_at_least_one_multi_shape_item = false;
    for id in ids {
        let count_45 = board_45.item_tree_shape_count(id, tree_45);
        let count_90 = board_90.item_tree_shape_count(id, tree_90);
        assert_eq!(
            count_45, count_90,
            "tile-shape count must be angle-independent for item {id}"
        );
        if count_45 > 1 {
            compared_at_least_one_multi_shape_item = true;
        }
        for i in 0..count_45 {
            let shape_45 = board_45
                .item_tree_shape(id, tree_45, i)
                .expect("a 45-degree tile shape");
            let shape_90 = board_90
                .item_tree_shape(id, tree_90, i)
                .expect("a 90-degree tile shape");
            let box_45 = shape_45.bounding_box();
            let box_90 = shape_90.bounding_box();
            assert!(
                box_90.contains(&box_45),
                "item {id} shape {i}: the 90-degree bounding box {box_90:?} must contain the \
                 45-degree one {box_45:?}"
            );
        }
    }
    assert!(
        compared_at_least_one_multi_shape_item,
        "the fixture must exercise at least one item with more than one tile shape"
    );
}

// ---------------------------------------------------------------------------------------------
// Item iteration order (quirk #63)
// ---------------------------------------------------------------------------------------------

#[test]
fn item_iteration_is_descending_board_order() {
    let board = p2t11_board();
    let via_get_items: Vec<u32> = board.get_items().map(|item| item.id().0).collect();
    assert!(
        via_get_items.windows(2).all(|w| w[0] > w[1]),
        "Board::get_items() must iterate in strictly descending id order: {via_get_items:?}"
    );
    let via_board_order: Vec<u32> = board
        .items_in_board_order()
        .into_iter()
        .map(|id| id.0)
        .collect();
    assert_eq!(
        via_get_items, via_board_order,
        "get_items() and items_in_board_order() must agree"
    );
}

// ---------------------------------------------------------------------------------------------
// Send + Sync + Clone
// ---------------------------------------------------------------------------------------------

#[test]
fn board_is_send_sync_and_clone() {
    fn assert_bounds<T: Send + Sync + Clone>() {}
    assert_bounds::<Board>();
}
