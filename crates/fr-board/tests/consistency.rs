mod board_builder;

use board_builder::{WIDE_CLEARANCE_CLASS, p2t11_board};
use fr_board::prelude::*;
use fr_geometry::{
    Area, IntBox, Point, PolygonShape, Polyline, PolylineShapeRef, Shape, TileShape,
};

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


#[test]
fn deep_copy_is_hash_equal_and_query_equal() {
    let mut board = p2t11_board();
    let copy = board.deep_copy();

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


fn normalize_fixture_board() -> Board {
    let ls = LayerStructure::new(vec![Layer::new("l0", true)]);
    let cm = ClearanceMatrix::get_default_instance(&ls, 10);
    let mut rules = BoardRules::new(ls.clone(), cm);
    rules.create_default_net_class();
    let default_class = rules.get_default_net_class();

    let padstacks = Padstacks::new(ls);
    let library = BoardLibrary::new(padstacks, Packages::new());
    let outline = vec![PolylineShapeRef::Polygon(PolygonShape::from_points(&[
        Point::new(-1_000_000, -1_000_000),
        Point::new(1_000_000, -1_000_000),
        Point::new(1_000_000, 1_000_000),
        Point::new(-1_000_000, 1_000_000),
    ]))];
    let mut board = Board::new(
        outline,
        0,
        IntBox::from_coords(-2_000_000, -2_000_000, 2_000_000, 2_000_000),
        rules,
        library,
        Components::new(),
        Communication::default(),
    );
    board.rules.nets.add("N1", 1, false, default_class);
    board.rules.nets.add("N2", 1, false, default_class);

    board
        .insert_trace_without_cleaning(
            Polyline::from_points(&[
                Point::new(0, 0),
                Point::new(10_000, 0),
                Point::new(20_000, 0),
                Point::new(30_000, 0),
            ]),
            0,
            1000,
            vec![1],
            1,
            FixedState::Unfixed,
        )
        .expect("insertTraceWithoutCleaning");
    board
        .insert_trace_without_cleaning(
            Polyline::from_points(&[Point::new(10_000, 0), Point::new(20_000, 0)]),
            0,
            1000,
            vec![1],
            1,
            FixedState::Unfixed,
        )
        .expect("insertTraceWithoutCleaning");
    board
}

#[test]
fn normalize_traces_is_idempotent() {
    let mut board = normalize_fixture_board();

    let changed_first = board
        .normalize_traces(1)
        .expect("normalize_traces must not fail on a well-formed board");
    assert!(
        changed_first,
        "the first call must actually combine the two overlapping traces"
    );
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

    let tree_45 = build_autoroute_tree(&mut board_45, WIDE_CLEARANCE_CLASS);
    let tree_90 = build_autoroute_tree(&mut board_90, WIDE_CLEARANCE_CLASS);

    let ids = board_45.items_in_board_order();
    assert_eq!(ids, board_90.items_in_board_order());

    let mut compared_at_least_one_multi_shape_item = false;
    let mut discriminating_shape_pairs = 0usize;
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
            assert!(
                shape_90.contains_tile(&shape_45),
                "item {id} shape {i}: the 90-degree tile shape {shape_90:?} must contain the \
                 45-degree one {shape_45:?}"
            );
            if !shape_45.contains_tile(&shape_90) {
                discriminating_shape_pairs += 1;
            }
        }
    }
    assert!(
        compared_at_least_one_multi_shape_item,
        "the fixture must exercise at least one item with more than one tile shape"
    );
    assert!(
        discriminating_shape_pairs > 0,
        "the fixture must exercise at least one shape pair where the reverse containment does \
         not hold, otherwise this comparison would not actually discriminate the two trees"
    );
}


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


#[test]
fn board_is_send_sync_and_clone() {
    fn assert_bounds<T: Send + Sync + Clone>() {}
    assert_bounds::<Board>();
}
