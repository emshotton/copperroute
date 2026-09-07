use copper_board::board::{MAX_NORMALIZATION_DEPTH, MAX_NORMALIZE_ITERATIONS};
use copper_board::error::BoardError;
use copper_board::prelude::*;
use copper_geometry::{
    Area, IntBox, IntVector, Line, Point, PolygonShape, Polyline, PolylineShapeRef, Shape,
    TileShape, Vector,
};

fn trace_board(layer_count: usize) -> (Board, PadstackId) {
    let ls = LayerStructure::new(
        (0..layer_count)
            .map(|i| Layer::new(format!("l{i}"), true))
            .collect(),
    );
    let cm = ClearanceMatrix::get_default_instance(&ls, 10);
    let mut rules = BoardRules::new(ls.clone(), cm);
    rules.create_default_net_class();
    let default_class = rules.get_default_net_class();

    let mut padstacks = Padstacks::new(ls);
    let trace_pad = padstacks.add(
        "via",
        (0..layer_count)
            .map(|_| {
                Some(Shape::Tile(TileShape::Box(IntBox::from_coords(
                    -300, -300, 300, 300,
                ))))
            })
            .collect(),
        true,
        false,
    );
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
    (board, trace_pad)
}

fn pts(xy: &[i32]) -> Vec<Point> {
    xy.chunks(2).map(|c| Point::new(c[0], c[1])).collect()
}

fn tr(board: &mut Board, half_width: i32, net: i32, fixed: FixedState, xy: &[i32]) -> ItemId {
    board
        .insert_trace_without_cleaning(
            Polyline::from_points(&pts(xy)),
            0,
            half_width,
            vec![net],
            1,
            fixed,
        )
        .expect("insertTraceWithoutCleaning")
}

fn corners(board: &Board, id: ItemId) -> Vec<(i32, i32)> {
    let Some(Item::Trace(trace)) = board.get_item(id) else {
        panic!("not a trace on the board: {id:?}")
    };
    let polyline = trace.polyline();
    (0..trace.corner_count())
        .map(|i| match polyline.corner(i) {
            Some(Point::Int(p)) => (p.x, p.y),
            other => panic!("not an int corner: {other:?}"),
        })
        .collect()
}

fn trace_ids(board: &Board) -> Vec<u32> {
    board
        .items_in_board_order()
        .into_iter()
        .filter(|id| board.get_item(*id).is_some_and(Item::is_trace))
        .map(|id| id.0)
        .collect()
}

fn item_ids(board: &Board) -> Vec<u32> {
    board
        .items_in_board_order()
        .into_iter()
        .map(|id| id.0)
        .collect()
}

fn board_polyline(board: &Board, id: ItemId) -> &Polyline {
    let Some(Item::Trace(trace)) = board.get_item(id) else {
        panic!("not a trace on the board: {id:?}")
    };
    trace.polyline()
}

fn tree_entries(board: &Board, id: ItemId) -> Vec<Option<LeafId>> {
    let tree = board.default_tree_id();
    board
        .get_item(id)
        .and_then(|item| item.get_search_tree_entries(tree))
        .expect("the trace is on the board")
        .to_vec()
}

fn entry_count(board: &Board, id: ItemId) -> Option<usize> {
    let tree = board.default_tree_id();
    board
        .get_item(id)
        .and_then(|item| item.get_search_tree_entries(tree))
        .map(<[Option<LeafId>]>::len)
}

#[test]
fn combine_at_start_joins_two_collinear_segments() {
    let (mut board, _) = trace_board(1);
    let second = tr(
        &mut board,
        1000,
        1,
        FixedState::Unfixed,
        &[10000, 0, 20000, 0],
    );
    tr(&mut board, 1000, 1, FixedState::Unfixed, &[0, 0, 10000, 0]);
    assert!(
        board
            .combine_trace(second)
            .expect("no normalisation failure")
    );
    assert_eq!(trace_ids(&board), vec![2]);
    assert_eq!(corners(&board, second), vec![(0, 0), (20000, 0)]);
    assert_eq!(entry_count(&board, second), Some(1));
    assert_eq!(item_ids(&board), vec![2, 1]);
    assert_eq!(board.revision(), 4);
}

#[test]
fn combine_at_start_reverses_the_other_trace_when_it_starts_at_the_same_corner() {
    let (mut board, _) = trace_board(1);
    let second = tr(
        &mut board,
        1000,
        1,
        FixedState::Unfixed,
        &[10000, 0, 20000, 0],
    );
    tr(&mut board, 1000, 1, FixedState::Unfixed, &[10000, 0, 0, 0]);
    assert!(
        board
            .combine_trace(second)
            .expect("no normalisation failure")
    );
    assert_eq!(corners(&board, second), vec![(0, 0), (20000, 0)]);
}

#[test]
fn combine_at_end_joins_and_marks_the_changed_area() {
    let (mut board, _) = trace_board(1);
    board.start_marking_changed_area();
    let first = tr(&mut board, 1000, 1, FixedState::Unfixed, &[0, 0, 10000, 0]);
    tr(
        &mut board,
        1000,
        1,
        FixedState::Unfixed,
        &[10000, 0, 20000, 0],
    );
    assert!(
        board
            .combine_trace(first)
            .expect("no normalisation failure")
    );
    assert_eq!(corners(&board, first), vec![(0, 0), (20000, 0)]);
    let area = board.changed_area.as_ref().expect("marked").get_area(0);
    assert_eq!(
        (area.left_x, area.bottom_y, area.right_x, area.top_y),
        (10000, 0, 10000, 0)
    );
}

#[test]
fn combine_at_end_reverses_the_other_trace_when_it_ends_at_the_same_corner() {
    let (mut board, _) = trace_board(1);
    let first = tr(&mut board, 1000, 1, FixedState::Unfixed, &[0, 0, 10000, 0]);
    tr(
        &mut board,
        1000,
        1,
        FixedState::Unfixed,
        &[20000, 0, 10000, 0],
    );
    assert!(
        board
            .combine_trace(first)
            .expect("no normalisation failure")
    );
    assert_eq!(corners(&board, first), vec![(0, 0), (20000, 0)]);
}

#[test]
fn combine_at_a_corner_keeps_the_join_line() {
    let (mut board, _) = trace_board(1);
    let first = tr(&mut board, 1000, 1, FixedState::Unfixed, &[0, 0, 10000, 0]);
    tr(
        &mut board,
        1000,
        1,
        FixedState::Unfixed,
        &[10000, 0, 10000, 10000],
    );
    assert!(
        board
            .combine_trace(first)
            .expect("no normalisation failure")
    );
    assert_eq!(
        corners(&board, first),
        vec![(0, 0), (10000, 0), (10000, 10000)]
    );
    assert_eq!(entry_count(&board, first), Some(2));
}

#[test]
fn combine_refuses_a_fork_a_wider_trace_a_fixed_trace_and_a_foreign_net() {
    for build in [
        (|board: &mut Board| {
            tr(board, 1000, 1, FixedState::Unfixed, &[10000, 0, 20000, 0]);
            tr(
                board,
                1000,
                1,
                FixedState::Unfixed,
                &[10000, 0, 10000, 10000],
            );
        }) as fn(&mut Board),
        |board: &mut Board| {
            tr(board, 500, 1, FixedState::Unfixed, &[10000, 0, 20000, 0]);
        },
        |board: &mut Board| {
            tr(
                board,
                1000,
                1,
                FixedState::ShoveFixed,
                &[10000, 0, 20000, 0],
            );
        },
        |board: &mut Board| {
            tr(board, 1000, 2, FixedState::Unfixed, &[10000, 0, 20000, 0]);
        },
    ] {
        let (mut board, _) = trace_board(1);
        let first = tr(&mut board, 1000, 1, FixedState::Unfixed, &[0, 0, 10000, 0]);
        build(&mut board);
        let before = item_ids(&board);
        assert!(
            !board
                .combine_trace(first)
                .expect("no normalisation failure")
        );
        assert_eq!(item_ids(&board), before);
        assert_eq!(corners(&board, first), vec![(0, 0), (10000, 0)]);
    }
}

#[test]
fn combine_absorbs_a_whole_chain_from_the_middle_without_recursing() {
    let (mut board, _) = trace_board(1);
    for i in 0..5 {
        tr(
            &mut board,
            1000,
            1,
            FixedState::Unfixed,
            &[i * 10000, 0, (i + 1) * 10000, 0],
        );
    }
    let middle = ItemId(4);
    assert!(
        board
            .combine_trace(middle)
            .expect("no normalisation failure")
    );
    assert_eq!(trace_ids(&board), vec![4]);
    assert_eq!(corners(&board, middle), vec![(0, 0), (50000, 0)]);
}

#[test]
fn combine_falls_back_when_a_trace_has_no_default_tree_entries() {
    let (mut board, _) = trace_board(1);
    let first = tr(
        &mut board,
        1000,
        1,
        FixedState::Unfixed,
        &[10000, 10000, 20000, 10000],
    );
    let second = tr(
        &mut board,
        1000,
        1,
        FixedState::Unfixed,
        &[20000, 10000, 30000, 10000],
    );
    let mut item = board.items.remove(&first).expect("the first trace");
    board.trees.remove(&mut item);
    item.set_on_the_board(true);
    board.items.insert(first, item);
    assert_eq!(entry_count(&board, first), None);

    assert!(
        board
            .combine_trace(first)
            .expect("no normalisation failure")
    );
    assert!(board.get_item(first).is_some_and(Item::is_on_the_board));
    assert!(!board.get_item(second).is_some_and(Item::is_on_the_board));
    assert_eq!(entry_count(&board, first), Some(1));
    assert_eq!(corners(&board, first), vec![(10000, 10000), (30000, 10000)]);
}

#[test]
fn combine_prepends_a_straight_trace_to_an_l_shaped_one() {
    let (mut board, _) = trace_board(1);
    let l = tr(
        &mut board,
        1000,
        1,
        FixedState::Unfixed,
        &[10000, 0, 10000, 10000, 20000, 10000],
    );
    tr(&mut board, 1000, 1, FixedState::Unfixed, &[0, 0, 10000, 0]);
    assert!(board.combine_trace(l).expect("no normalisation failure"));
    assert_eq!(
        corners(&board, l),
        vec![(0, 0), (10000, 0), (10000, 10000), (20000, 10000)]
    );
    assert_eq!(entry_count(&board, l), Some(3));
}

#[test]
fn combine_ignores_a_conduction_area_at_the_join() {
    let (mut board, _) = trace_board(1);
    board.insert_conduction_area(
        Area::Shape(Shape::Tile(TileShape::Box(IntBox::from_coords(
            9000, -1000, 11000, 1000,
        )))),
        0,
        vec![1],
        1,
        true,
        FixedState::Unfixed,
    );
    let first = tr(&mut board, 1000, 1, FixedState::Unfixed, &[0, 0, 10000, 0]);
    tr(
        &mut board,
        1000,
        1,
        FixedState::Unfixed,
        &[10000, 0, 20000, 0],
    );
    assert!(
        board
            .combine_trace(first)
            .expect("no normalisation failure")
    );
    assert_eq!(corners(&board, first), vec![(0, 0), (20000, 0)]);
    assert_eq!(item_ids(&board), vec![3, 2, 1]);
}

#[test]
fn split_preserves_non_overlapping_segments() {
    let (mut board, _) = trace_board(1);
    let long = tr(
        &mut board,
        1000,
        1,
        FixedState::Unfixed,
        &[0, 0, 10000, 0, 20000, 0, 30000, 0],
    );
    tr(
        &mut board,
        1000,
        1,
        FixedState::Unfixed,
        &[10000, 0, 20000, 0],
    );
    let pieces = board
        .split_trace(long, None)
        .expect("no normalisation failure");
    assert_eq!(
        pieces.iter().map(|id| id.0).collect::<Vec<_>>(),
        vec![4, 6, 7]
    );
    assert_eq!(trace_ids(&board), vec![7, 6, 4]);
    assert_eq!(corners(&board, ItemId(4)), vec![(0, 0), (10000, 0)]);
    assert_eq!(corners(&board, ItemId(6)), vec![(10000, 0), (20000, 0)]);
    assert_eq!(corners(&board, ItemId(7)), vec![(20000, 0), (30000, 0)]);
}

#[test]
fn split_does_not_remove_valid_segments() {
    let (mut board, _) = trace_board(1);
    tr(
        &mut board,
        1000,
        98,
        FixedState::Unfixed,
        &[
            1291423, -987076, 1270000, -975000, 1250000, -970000, 1243227, -964893,
        ],
    );
    let short = tr(
        &mut board,
        1000,
        98,
        FixedState::Unfixed,
        &[1243227, -964893, 1241414, -964893],
    );
    assert!(
        board
            .combine_trace(short)
            .expect("no normalisation failure")
    );
    let combined = board
        .items_in_board_order()
        .into_iter()
        .find(|id| {
            board
                .get_item(*id)
                .is_some_and(|item| item.is_trace() && item.contains_net(98))
        })
        .expect("the combined trace");
    assert_eq!(combined, ItemId(3));
    assert_eq!(
        corners(&board, combined).first().copied(),
        Some((1291423, -987076))
    );

    tr(
        &mut board,
        1000,
        98,
        FixedState::Unfixed,
        &[1243227, -964893, 1242000, -960000, 1241171, -952775],
    );
    let pieces = board
        .split_trace(combined, None)
        .expect("no normalisation failure");
    assert_eq!(pieces.iter().map(|id| id.0).collect::<Vec<_>>(), vec![5, 6]);
    assert_eq!(trace_ids(&board), vec![6, 5, 4]);
    assert_eq!(
        corners(&board, ItemId(5)),
        vec![
            (1291423, -987076),
            (1270000, -975000),
            (1250000, -970000),
            (1243227, -964893)
        ]
    );
}

#[test]
fn an_overlapping_trace_is_not_a_cycle() {
    let (mut board, _) = trace_board(1);
    let abc = tr(
        &mut board,
        1000,
        1,
        FixedState::Unfixed,
        &[0, 0, 10000, 0, 20000, 0],
    );
    tr(
        &mut board,
        1000,
        1,
        FixedState::Unfixed,
        &[10000, 0, 20000, 0],
    );
    assert!(!board.is_trace_cycle(abc));
}

#[test]
fn split_honours_the_clip_shape() {
    let (mut board, _) = trace_board(1);
    let long = tr(
        &mut board,
        1000,
        1,
        FixedState::Unfixed,
        &[0, 0, 10000, 0, 20000, 0, 30000, 0],
    );
    tr(
        &mut board,
        1000,
        1,
        FixedState::Unfixed,
        &[10000, 0, 20000, 0],
    );
    let away = IntBox::from_coords(100_000, 100_000, 110_000, 110_000).bounding_octagon();
    let pieces = board
        .split_trace(long, Some(&away))
        .expect("no normalisation failure");
    assert_eq!(pieces, vec![long]);
    assert_eq!(trace_ids(&board), vec![3, 2]);

    let over = IntBox::from_coords(-1000, -1000, 31000, 1000).bounding_octagon();
    let pieces = board
        .split_trace(long, Some(&over))
        .expect("no normalisation failure");
    assert_eq!(
        pieces.iter().map(|id| id.0).collect::<Vec<_>>(),
        vec![4, 6, 7]
    );
}

#[test]
fn a_drill_item_splits_the_trace_but_not_the_returned_collection() {
    let (mut board, pad) = trace_board(1);
    board
        .insert_via(
            pad,
            Point::new(10000, 0),
            vec![1],
            1,
            FixedState::Unfixed,
            true,
        )
        .expect("no normalisation failure");
    let trace = tr(&mut board, 1000, 1, FixedState::Unfixed, &[0, 0, 20000, 0]);
    let pieces = board
        .split_trace(trace, None)
        .expect("no normalisation failure");
    assert_eq!(pieces, vec![trace]);
    assert!(!board.get_item(trace).is_some_and(Item::is_on_the_board));
    assert_eq!(item_ids(&board), vec![5, 4, 2, 1]);
    assert_eq!(corners(&board, ItemId(4)), vec![(0, 0), (10000, 0)]);
    assert_eq!(corners(&board, ItemId(5)), vec![(10000, 0), (20000, 0)]);
}

#[test]
fn two_crossing_traces_are_both_split_at_the_crossing() {
    let (mut board, _) = trace_board(1);
    let horizontal = tr(&mut board, 1000, 1, FixedState::Unfixed, &[0, 0, 20000, 0]);
    tr(
        &mut board,
        1000,
        1,
        FixedState::Unfixed,
        &[10000, -10000, 10000, 10000],
    );
    let pieces = board
        .split_trace(horizontal, None)
        .expect("no normalisation failure");
    assert_eq!(pieces.iter().map(|id| id.0).collect::<Vec<_>>(), vec![6, 7]);
    assert_eq!(trace_ids(&board), vec![7, 6, 5, 4]);
    assert_eq!(
        corners(&board, ItemId(4)),
        vec![(10000, -10000), (10000, 0)]
    );
    assert_eq!(corners(&board, ItemId(5)), vec![(10000, 0), (10000, 10000)]);
}

#[test]
fn a_conduction_area_cycle_removes_the_trace_and_empties_the_result() {
    let (mut board, _) = trace_board(1);
    board.insert_conduction_area(
        Area::Shape(Shape::Tile(TileShape::Box(IntBox::from_coords(
            -1000, -1000, 21000, 1000,
        )))),
        0,
        vec![1],
        1,
        true,
        FixedState::Unfixed,
    );
    let trace = tr(&mut board, 1000, 1, FixedState::Unfixed, &[0, 0, 20000, 0]);
    let pieces = board
        .split_trace(trace, None)
        .expect("no normalisation failure");
    assert!(pieces.is_empty());
    assert!(!board.get_item(trace).is_some_and(Item::is_on_the_board));
    assert_eq!(item_ids(&board), vec![2, 1]);
}

#[test]
fn a_trace_of_a_non_normal_net_is_never_split() {
    let (mut board, _) = trace_board(1);
    let trace = tr(
        &mut board,
        1000,
        0,
        FixedState::Unfixed,
        &[0, 0, 10000, 0, 20000, 0, 30000, 0],
    );
    tr(
        &mut board,
        1000,
        0,
        FixedState::Unfixed,
        &[10000, 0, 20000, 0],
    );
    assert_eq!(
        board
            .split_trace(trace, None)
            .expect("no normalisation failure"),
        vec![trace]
    );
    assert_eq!(trace_ids(&board), vec![3, 2]);
}

#[test]
fn a_user_fixed_trace_refuses_to_split_and_normalizes_to_false() {
    let (mut board, _) = trace_board(1);
    let fixed = tr(
        &mut board,
        1000,
        1,
        FixedState::UserFixed,
        &[0, 0, 10000, 0, 20000, 0, 30000, 0],
    );
    tr(
        &mut board,
        1000,
        1,
        FixedState::Unfixed,
        &[10000, 0, 20000, 0],
    );
    assert_eq!(
        board
            .split_trace(fixed, None)
            .expect("no normalisation failure"),
        vec![fixed]
    );
    assert!(!board.normalize_trace(fixed, None).expect("no failure"));
    assert_eq!(trace_ids(&board), vec![3, 2]);
}

#[test]
fn normalize_splits_then_recombines_into_one_trace() {
    let (mut board, _) = trace_board(1);
    let long = tr(
        &mut board,
        1000,
        1,
        FixedState::Unfixed,
        &[0, 0, 10000, 0, 20000, 0, 30000, 0],
    );
    tr(
        &mut board,
        1000,
        1,
        FixedState::Unfixed,
        &[10000, 0, 20000, 0],
    );
    assert!(board.normalize_trace(long, None).expect("no failure"));
    assert_eq!(trace_ids(&board), vec![4]);
    assert_eq!(corners(&board, ItemId(4)), vec![(0, 0), (30000, 0)]);
}

#[test]
fn normalize_reports_false_when_nothing_overlaps() {
    let (mut board, _) = trace_board(1);
    let alone = tr(&mut board, 1000, 1, FixedState::Unfixed, &[0, 0, 10000, 0]);
    assert!(!board.normalize_trace(alone, None).expect("no failure"));
    assert_eq!(trace_ids(&board), vec![2]);
}

#[test]
fn normalize_reports_true_when_the_only_change_is_a_combine() {
    let (mut board, _) = trace_board(1);
    let first = tr(&mut board, 1000, 1, FixedState::Unfixed, &[0, 0, 10000, 0]);
    tr(
        &mut board,
        1000,
        1,
        FixedState::Unfixed,
        &[10000, 0, 20000, 0],
    );
    assert!(board.normalize_trace(first, None).expect("no failure"));
    assert_eq!(trace_ids(&board), vec![2]);
    assert_eq!(corners(&board, first), vec![(0, 0), (20000, 0)]);
}

#[test]
fn normalize_depth_cap_returns_false_not_error() {
    assert_eq!(MAX_NORMALIZATION_DEPTH, 16);
    let (mut board, _) = trace_board(1);
    let long = tr(
        &mut board,
        1000,
        1,
        FixedState::Unfixed,
        &[0, 0, 10000, 0, 20000, 0, 30000, 0],
    );
    tr(
        &mut board,
        1000,
        1,
        FixedState::Unfixed,
        &[10000, 0, 20000, 0],
    );
    let before = item_ids(&board);
    let mut over = board.clone();
    assert_eq!(
        over.normalize_trace_at_depth(long, None, MAX_NORMALIZATION_DEPTH + 1),
        Ok(false)
    );
    assert_eq!(item_ids(&over), before);
    assert!(
        board
            .normalize_trace_at_depth(long, None, MAX_NORMALIZATION_DEPTH)
            .expect("no failure")
    );
    assert_eq!(trace_ids(&board), vec![4]);
}

#[test]
fn from_lines_err_propagates_as_board_error_normalization() {
    let (mut board, _) = trace_board(1);
    let line_a = Line::from_coords(0, 0, 1000, 0);
    let line_b = Line::from_coords(0, 0, 0, 1000);
    let line_c = Line::from_coords(0, 0, 1000, 1000);
    let line_d = Line::from_coords(2000, 2000, 3000, 2000);
    let line_x = Line::from_coords(4000, 2000, 4000, 3000);
    let line_y = Line::from_coords(4000, 5000, 5000, 5000);
    let six = board
        .insert_trace_without_cleaning(
            Polyline::from_lines(vec![line_a, line_b, line_c, line_d, line_x, line_y])
                .expect("a six-line polyline"),
            0,
            100,
            vec![1],
            1,
            FixedState::Unfixed,
        )
        .expect("insertTraceWithoutCleaning");
    board
        .insert_trace_without_cleaning(
            Polyline::from_lines(vec![line_d, line_c, line_b]).expect("a three-line polyline"),
            0,
            100,
            vec![1],
            1,
            FixedState::Unfixed,
        )
        .expect("insertTraceWithoutCleaning");
    let before = item_ids(&board);

    assert_eq!(
        board.combine_trace(six),
        Ok(true),
        "the combine completes where Java threw out to the pass-level catch"
    );

    assert_eq!(before, vec![3, 2, 1]);
    assert_eq!(item_ids(&board), vec![2, 1], "the joined trace is gone");
    assert_eq!(
        corners(&board, six),
        vec![(4000, 2000), (4000, 5000)],
        "the degenerate head cancelled; pre-fix this was the untouched five-corner polyline \
         [(0,0), (0,0), (2000,2000), (4000,2000), (4000,5000)]"
    );

    assert_eq!(board.combine_traces(1), Ok(false));
    assert_eq!(item_ids(&board), vec![2, 1]);

    let (mut board, _) = trace_board(1);
    let six = board
        .insert_trace_without_cleaning(
            Polyline::from_lines(vec![line_a, line_b, line_c, line_d, line_x, line_y])
                .expect("a six-line polyline"),
            0,
            100,
            vec![1],
            1,
            FixedState::Unfixed,
        )
        .expect("insertTraceWithoutCleaning");
    board
        .insert_trace_without_cleaning(
            Polyline::from_lines(vec![line_d, line_c, line_b]).expect("a three-line polyline"),
            0,
            100,
            vec![1],
            1,
            FixedState::Unfixed,
        )
        .expect("insertTraceWithoutCleaning");
    assert!(board.normalize_trace(six, None).expect("no failure"));
    assert!(trace_ids(&board).is_empty());
    assert!(!board.normalize_traces(1).expect("no failure"));
}

#[test]
fn insert_trace_swallows_a_normalisation_failure_as_java_does() {
    let (mut board, _) = trace_board(1);
    let line_a = Line::from_coords(0, 0, 1000, 0);
    let line_b = Line::from_coords(0, 0, 0, 1000);
    let line_c = Line::from_coords(0, 0, 1000, 1000);
    let line_d = Line::from_coords(2000, 2000, 3000, 2000);
    let line_x = Line::from_coords(4000, 2000, 4000, 3000);
    let line_y = Line::from_coords(4000, 5000, 5000, 5000);
    board
        .insert_trace_without_cleaning(
            Polyline::from_lines(vec![line_a, line_b, line_c, line_d, line_x, line_y])
                .expect("a six-line polyline"),
            0,
            100,
            vec![1],
            1,
            FixedState::Unfixed,
        )
        .expect("insertTraceWithoutCleaning");
    let inserted = board.insert_trace(
        Polyline::from_lines(vec![line_d, line_c, line_b]).expect("a three-line polyline"),
        0,
        100,
        vec![1],
        1,
        FixedState::Unfixed,
    );
    assert_eq!(inserted, Some(ItemId(3)));
    assert!(trace_ids(&board).is_empty());
    assert_eq!(item_ids(&board), vec![1]);
}

#[test]
fn change_replaces_the_geometry_and_reuses_what_it_can() {
    let (mut board, _) = trace_board(1);
    let trace = tr(
        &mut board,
        1000,
        1,
        FixedState::Unfixed,
        &[0, 0, 10000, 0, 20000, 0],
    );
    board.change_trace(
        trace,
        Polyline::from_points(&pts(&[0, 0, 10000, 5000, 20000, 0])),
    );
    assert_eq!(
        corners(&board, trace),
        vec![(0, 0), (10000, 5000), (20000, 0)]
    );
    assert_eq!(entry_count(&board, trace), Some(2));
}

#[test]
fn change_to_a_value_equal_but_freshly_built_polyline_still_normalizes() {
    let (mut board, _) = trace_board(1);
    let s5 = tr(
        &mut board,
        1000,
        1,
        FixedState::Unfixed,
        &[0, 0, 10000, 0, 20000, 0, 30000, 0],
    );
    tr(
        &mut board,
        1000,
        1,
        FixedState::Unfixed,
        &[10000, 0, 20000, 0],
    );
    let rebuilt = Polyline::from_points(&pts(&[0, 0, 10000, 0, 20000, 0, 30000, 0]));
    assert_eq!(rebuilt, *board_polyline(&board, s5), "value-equal");
    board.change_trace(s5, rebuilt);
    let remaining = trace_ids(&board);
    assert_eq!(remaining.len(), 1, "traces left: {remaining:?}");
    assert_eq!(
        corners(&board, ItemId(remaining[0])),
        vec![(0, 0), (30000, 0)]
    );
}

#[test]
fn change_keeps_the_entries_whose_lines_are_the_same_objects() {
    let (mut board, _) = trace_board(1);
    let trace = tr(
        &mut board,
        1000,
        1,
        FixedState::Unfixed,
        &[0, 0, 10000, 0, 10000, 10000, 20000, 10000],
    );
    assert_eq!(entry_count(&board, trace), Some(3));

    let old: Vec<Line> = board_polyline(&board, trace).lines().to_vec();
    assert_eq!(old.len(), 5);
    let mut lines = old.clone();
    lines[2] = Line::new(old[2].a, old[2].b);
    assert_eq!(lines[2], old[2]);
    assert!(!lines[2].is_same_object(&old[2]));
    lines[4] = Line::new(
        copper_geometry::IntPoint::new(25000, 0),
        copper_geometry::IntPoint::new(25000, 20000),
    );

    let entries_before = tree_entries(&board, trace);
    let new_polyline = Polyline::from_lines(lines).expect("a valid polyline");
    assert_eq!(new_polyline.lines().len(), 5, "nothing was normalised away");
    board.change_trace(trace, new_polyline);

    let entries_after = tree_entries(&board, trace);
    assert_eq!(entries_after.len(), 3);
    for (i, (after, before)) in entries_after.iter().zip(&entries_before).enumerate() {
        assert_ne!(
            after, before,
            "leaf {i} was reused, so keepAtStartCount > 0"
        );
    }
}

#[test]
fn change_on_a_trace_that_is_off_the_board_only_swaps_the_polyline() {
    let (mut board, _) = trace_board(1);
    let trace = tr(&mut board, 1000, 1, FixedState::Unfixed, &[0, 0, 10000, 0]);
    let mut item = board.items.remove(&trace).expect("the trace");
    board.trees.remove(&mut item);
    board.items.insert(trace, item);
    board.change_trace(trace, Polyline::from_points(&pts(&[0, 0, 30000, 0])));
    assert!(!board.get_item(trace).is_some_and(Item::is_on_the_board));
    assert_eq!(corners(&board, trace), vec![(0, 0), (30000, 0)]);
    assert_eq!(entry_count(&board, trace), None);
}

#[test]
fn split_at_point_cuts_the_trace_in_two() {
    let (mut board, _) = trace_board(1);
    let trace = tr(&mut board, 1000, 1, FixedState::Unfixed, &[0, 0, 20000, 0]);
    let pieces = board
        .split_trace_at_point(trace, &Point::new(10000, 0))
        .expect("no normalisation failure")
        .expect("the two pieces");
    assert_eq!(pieces, [Some(ItemId(3)), Some(ItemId(4))]);
    assert_eq!(corners(&board, ItemId(3)), vec![(0, 0), (10000, 0)]);
    assert_eq!(corners(&board, ItemId(4)), vec![(10000, 0), (20000, 0)]);
    assert_eq!(
        board
            .split_trace_at_point(ItemId(3), &Point::new(5000, 5000))
            .expect("no normalisation failure"),
        None
    );
}

#[test]
fn insert_trace_normalizes_the_new_trace() {
    for mark_changed_area in [false, true] {
        let (mut board, _) = trace_board(1);
        if mark_changed_area {
            board.start_marking_changed_area();
            board.mark_all_changed_area();
        }
        tr(&mut board, 1000, 1, FixedState::Unfixed, &[0, 0, 10000, 0]);
        board.insert_trace_at_points(
            &pts(&[10000, 0, 20000, 0]),
            0,
            1000,
            vec![1],
            1,
            FixedState::Unfixed,
        );
        assert_eq!(trace_ids(&board), vec![3]);
        assert_eq!(corners(&board, ItemId(3)), vec![(0, 0), (20000, 0)]);
    }
}

#[test]
fn combine_traces_walks_one_net_then_every_net() {
    let (mut board, _) = trace_board(1);
    for i in 0..5 {
        tr(
            &mut board,
            1000,
            1,
            FixedState::Unfixed,
            &[i * 10000, 0, (i + 1) * 10000, 0],
        );
    }
    tr(
        &mut board,
        1000,
        2,
        FixedState::Unfixed,
        &[0, 50000, 10000, 50000],
    );
    tr(
        &mut board,
        1000,
        2,
        FixedState::Unfixed,
        &[10000, 50000, 20000, 50000],
    );

    assert!(board.combine_traces(1).expect("no failure"));
    assert_eq!(trace_ids(&board), vec![8, 7, 6]);
    assert_eq!(corners(&board, ItemId(6)), vec![(0, 0), (50000, 0)]);

    assert!(board.combine_traces(-1).expect("no failure"));
    assert_eq!(trace_ids(&board), vec![8, 6]);
    assert_eq!(corners(&board, ItemId(8)), vec![(0, 50000), (20000, 50000)]);

    assert!(!board.combine_traces(-1).expect("no failure"));
}

#[test]
fn normalize_traces_converges_and_then_reports_false() {
    let (mut board, _) = trace_board(1);
    tr(
        &mut board,
        1000,
        1,
        FixedState::Unfixed,
        &[0, 0, 10000, 0, 20000, 0, 30000, 0],
    );
    tr(
        &mut board,
        1000,
        1,
        FixedState::Unfixed,
        &[10000, 0, 20000, 0],
    );
    assert!(board.normalize_traces(1).expect("no failure"));
    assert_eq!(trace_ids(&board), vec![3]);
    assert_eq!(corners(&board, ItemId(3)), vec![(0, 0), (30000, 0)]);
    assert!(!board.normalize_traces(1).expect("no failure"));
}

#[test]
fn normalize_all_traces_does_every_net_at_once() {
    let (mut board, _) = trace_board(1);
    tr(
        &mut board,
        1000,
        1,
        FixedState::Unfixed,
        &[0, 0, 10000, 0, 20000, 0, 30000, 0],
    );
    tr(
        &mut board,
        1000,
        1,
        FixedState::Unfixed,
        &[10000, 0, 20000, 0],
    );
    tr(
        &mut board,
        1000,
        2,
        FixedState::Unfixed,
        &[0, 50000, 10000, 50000],
    );
    tr(
        &mut board,
        1000,
        2,
        FixedState::Unfixed,
        &[10000, 50000, 20000, 50000],
    );
    assert!(board.normalize_all_traces().expect("no failure"));
    assert_eq!(trace_ids(&board), vec![5, 3]);
    assert_eq!(corners(&board, ItemId(3)), vec![(0, 0), (30000, 0)]);
    assert_eq!(corners(&board, ItemId(5)), vec![(0, 50000), (20000, 50000)]);
    assert!(!board.normalize_all_traces().expect("no failure"));
}

#[test]
fn normalize_traces_skips_a_suppressed_net() {
    assert_eq!(MAX_NORMALIZE_ITERATIONS, 2000);
    let (mut board, _) = trace_board(1);
    tr(
        &mut board,
        1000,
        1,
        FixedState::Unfixed,
        &[0, 0, 10000, 0, 20000, 0, 30000, 0],
    );
    tr(
        &mut board,
        1000,
        1,
        FixedState::Unfixed,
        &[10000, 0, 20000, 0],
    );
    board.normalize_suppressed_net_nos.insert(1);
    assert!(!board.normalize_traces(1).expect("no failure"));
    assert_eq!(trace_ids(&board), vec![3, 2]);
    assert!(board.normalize_all_traces().expect("no failure"));
    assert_eq!(trace_ids(&board), vec![3]);
}

#[test]
fn normalize_traces_terminates_within_2000() {
    let (mut board, _) = trace_board(1);
    for i in 0..400 {
        tr(
            &mut board,
            1000,
            1,
            FixedState::Unfixed,
            &[i * 100, 0, (i + 1) * 100, 0],
        );
    }
    assert!(board.normalize_traces(1).expect("no failure"));
    assert!(board.normalize_suppressed_net_nos.is_empty());
    assert_eq!(trace_ids(&board).len(), 1);
    assert!(!board.normalize_traces(1).expect("no failure"));
    assert!(board.normalize_suppressed_net_nos.is_empty());
}

#[test]
fn split_traces_cuts_at_a_location() {
    let (mut board, _) = trace_board(1);
    tr(&mut board, 1000, 1, FixedState::Unfixed, &[0, 0, 20000, 0]);
    tr(
        &mut board,
        1000,
        1,
        FixedState::Unfixed,
        &[10000, -10000, 10000, 10000],
    );
    assert!(
        board
            .split_traces(&Point::new(10000, 0), 0, 1)
            .expect("no failure")
    );
    assert_eq!(trace_ids(&board), vec![7, 6, 5, 4]);
    assert!(
        !board
            .split_traces(&Point::new(90000, 0), 0, 1)
            .expect("no failure")
    );
}

#[test]
fn insert_via_splits_the_traces_under_it() {
    let (mut board, pad) = trace_board(2);
    tr(&mut board, 1000, 1, FixedState::Unfixed, &[0, 0, 20000, 0]);
    let via = board
        .insert_via(
            pad,
            Point::new(10000, 0),
            vec![1],
            1,
            FixedState::Unfixed,
            true,
        )
        .expect("no failure");
    assert_eq!(via, ItemId(3));
    assert_eq!(trace_ids(&board), vec![5, 4]);
    assert_eq!(item_ids(&board), vec![5, 4, 3, 1]);
    assert_eq!(corners(&board, ItemId(4)), vec![(0, 0), (10000, 0)]);
    assert_eq!(corners(&board, ItemId(5)), vec![(10000, 0), (20000, 0)]);
}

#[test]
fn connect_to_trace_inserts_a_stub_and_normalises() {
    let (mut board, _) = trace_board(1);
    let trace = tr(&mut board, 1000, 1, FixedState::Unfixed, &[0, 0, 20000, 0]);
    assert!(board.connect_to_trace(&Point::new(10000, 5000), trace, 1000, 1));
    assert_eq!(trace_ids(&board), vec![4]);
    assert_eq!(corners(&board, ItemId(4)), vec![(10000, 5000), (10000, 0)]);
    assert_eq!(item_ids(&board), vec![4, 1]);
}

#[test]
fn remove_trace_tails_combines_what_the_stub_leaves_behind() {
    let (mut board, _) = trace_board(1);
    tr(&mut board, 1000, 1, FixedState::Unfixed, &[0, 0, 10000, 0]);
    tr(
        &mut board,
        1000,
        1,
        FixedState::Unfixed,
        &[10000, 0, 10000, 10000],
    );
    tr(
        &mut board,
        1000,
        1,
        FixedState::Unfixed,
        &[10000, 10000, 0, 0],
    );
    tr(
        &mut board,
        1000,
        1,
        FixedState::Unfixed,
        &[10000, 0, 20000, 0],
    );
    assert!(
        board
            .remove_trace_tails(1, StopConnectionOption::None)
            .expect("no failure")
    );
    assert_eq!(trace_ids(&board), vec![4]);
    assert_eq!(
        corners(&board, ItemId(4)),
        vec![(0, 0), (10000, 0), (10000, 10000), (0, 0)]
    );
}

#[test]
fn moving_a_contacted_via_inserts_and_normalises_a_connecting_trace() {
    let (mut board, pad) = trace_board(1);
    let via = board
        .insert_via(
            pad,
            Point::new(10000, 0),
            vec![1],
            1,
            FixedState::Unfixed,
            true,
        )
        .expect("no failure");
    tr(
        &mut board,
        1000,
        1,
        FixedState::Unfixed,
        &[10000, 0, 20000, 0],
    );
    board
        .move_item_by(via, &Vector::Int(IntVector::new(0, 10000)))
        .expect("no failure");
    assert_eq!(trace_ids(&board), vec![4]);
    assert_eq!(
        corners(&board, ItemId(4)),
        vec![(20000, 0), (10000, 0), (10000, 10000)]
    );
    assert_eq!(item_ids(&board), vec![4, 2, 1]);
}

fn combine_stack_overflow_board(segment_count: u32) -> Board {
    let (mut board, _) = trace_board(1);
    let (mut x, mut y, mut dx) = (130_000i32, -107_000i32, 200i32);
    let (mut emitted, mut in_row) = (0u32, 0u32);
    while emitted < segment_count {
        let (next_x, next_y) = if in_row < 280 {
            in_row += 1;
            (x + dx, y)
        } else {
            in_row = 0;
            dx = -dx;
            (x, y + 200)
        };
        board.insert_trace_without_cleaning(
            Polyline::from_two_points(&Point::new(x, y), &Point::new(next_x, next_y)),
            0,
            76,
            vec![1],
            1,
            FixedState::Unfixed,
        );
        x = next_x;
        y = next_y;
        emitted += 1;
    }
    board
}

#[test]
fn combine_stack_overflow_fixture() {
    let mut board = combine_stack_overflow_board(4000);
    assert_eq!(board.get_traces().len(), 4000);
    assert!(
        board
            .normalize_all_traces()
            .expect("no normalisation failure")
    );
    let traces = trace_ids(&board);
    assert_eq!(traces.len(), 1);
    let survivor = ItemId(traces[0]);
    assert_eq!(survivor, ItemId(4001));
    let corners = corners(&board, survivor);
    assert_eq!(corners.len(), 30);
    assert_eq!(corners[0], (130_000, -107_000));
    assert_eq!(corners[1], (186_000, -107_000));
    assert_eq!(corners[2], (186_000, -106_800));
    assert_eq!(corners[3], (130_000, -106_800));
    assert_eq!(corners[29], (143_200, -104_200));
    let Some(Item::Trace(trace)) = board.get_item(survivor) else {
        unreachable!()
    };
    assert_eq!(trace.polyline().lines().len(), 31);
    assert_eq!(trace.get_half_width(), 76);
}

#[test]
fn overlapping_tree_entries_returns_a_fresh_collection() {
    let (mut board, _) = trace_board(1);
    tr(&mut board, 1000, 1, FixedState::Unfixed, &[0, 0, 30000, 0]);
    tr(
        &mut board,
        1000,
        1,
        FixedState::Unfixed,
        &[0, 5000, 30000, 5000],
    );
    tr(
        &mut board,
        1000,
        1,
        FixedState::Unfixed,
        &[10000, -5000, 10000, 10000],
    );

    let probe = TileShape::Box(IntBox::from_coords(-2000, -2000, 32000, 12000));
    let ctx = board.ctx();
    let tree = board.trees.get_default_tree();

    let first = tree.overlapping_tree_entries(&probe, Some(0), &[], &board.items, &ctx);
    assert!(
        !first.is_empty(),
        "the probe must actually reach the three traces, or this test asserts nothing"
    );
    let second = tree.overlapping_tree_entries(&probe, Some(0), &[], &board.items, &ctx);
    assert_eq!(
        first, second,
        "a second call over an unchanged tree answers the same entries, not the same entries \
         appended to the first call's"
    );

    let mut mutated = tree.overlapping_tree_entries(&probe, Some(0), &[], &board.items, &ctx);
    let stolen = mutated[0];
    mutated.clear();
    mutated.extend(std::iter::repeat_n(stolen, 99));
    let after = tree.overlapping_tree_entries(&probe, Some(0), &[], &board.items, &ctx);
    assert_eq!(
        after, first,
        "clearing one answer and stuffing it with 99 copies of one entry must not reach the tree \
         or any later call"
    );

    let walked: usize = [&first, &second].iter().map(|e| e.len()).sum();
    assert_eq!(
        walked,
        2 * first.len(),
        "two reads are two reads' worth of entries, not one read's plus a growing list"
    );
}

fn ladder(rungs: i32) -> (Board, ItemId) {
    let (mut board, _) = trace_board(1);
    tr(&mut board, 1000, 1, FixedState::Unfixed, &[0, 0, 30000, 0]);
    tr(
        &mut board,
        1000,
        1,
        FixedState::Unfixed,
        &[0, 10000, 30000, 10000],
    );
    for i in 0..rungs {
        tr(
            &mut board,
            1000,
            1,
            FixedState::Unfixed,
            &[i * 10000, 0, i * 10000, 10000],
        );
    }
    (board, ItemId(3 + rungs as u32))
}

#[test]
fn a_two_rail_four_rung_ladder_normalizes_and_terminates() {
    let (mut board, last_rung) = ladder(4);
    assert_eq!(
        board.normalize_trace(last_rung, None),
        Ok(true),
        "the four-rung ladder must normalise, not hang"
    );
    assert_eq!(
        trace_ids(&board),
        Vec::<u32>::new(),
        "every piece of a 2-connected ladder is on a cycle, so removeIfCycle consumes all of it"
    );
    assert_eq!(item_ids(&board), vec![1], "only the board outline survives");

    let (mut board, last_rung) = ladder(3);
    assert_eq!(board.normalize_trace(last_rung, None), Ok(true));
    assert_eq!(trace_ids(&board), vec![6, 5]);
}

#[test]
fn a_four_rung_ladder_stops_when_the_stop_check_trips() {
    const MEASURED_STEPS: u64 = 80;

    let (mut board, last_rung) = ladder(4);
    let steps = std::cell::Cell::new(0u64);
    let stop = || {
        steps.set(steps.get() + 1);
        steps.get() > 10
    };
    assert_eq!(
        board.normalize_trace_checked(last_rung, None, &stop),
        Err(BoardError::Stopped),
        "a budget of 10 must stop a walk that needs 80"
    );
    assert_eq!(steps.get(), 11);

    let (mut board, last_rung) = ladder(4);
    let steps = std::cell::Cell::new(0u64);
    let stop = || {
        steps.set(steps.get() + 1);
        steps.get() > 500
    };
    assert_eq!(
        board.normalize_trace_checked(last_rung, None, &stop),
        Ok(true),
        "the four-rung ladder terminates well inside the budget it used to exhaust"
    );
    assert_eq!(
        steps.get(),
        MEASURED_STEPS,
        "the fixed four-rung walk costs exactly {MEASURED_STEPS} stop-check steps"
    );
    assert!(trace_ids(&board).is_empty());

    let (mut board, last_rung) = ladder(3);
    let steps = std::cell::Cell::new(0u64);
    let stop = || {
        steps.set(steps.get() + 1);
        steps.get() > 500
    };
    assert_eq!(
        board.normalize_trace_checked(last_rung, None, &stop),
        Ok(true),
        "a three-rung ladder normalises"
    );
    assert_eq!(
        steps.get(),
        22,
        "and still costs the 22 steps it cost before the visited set — measured against the \
         pre-fix tree, rungs 1/2/3 cost 12/22/22 with the fix and without it, so the guard fires \
         zero times on a walk that already terminated. (The `28` in this test's pre-Task-5 doc \
         comment was a stale Plan 3 figure; it is not what the tree measured.)"
    );
    assert_eq!(trace_ids(&board), vec![6, 5]);
}

#[test]
fn normalize_all_traces_checked_stops_on_the_ladder() {
    let (mut board, _) = ladder(4);
    let steps = std::cell::Cell::new(0u64);
    let stop = || {
        steps.set(steps.get() + 1);
        steps.get() > 10
    };
    assert_eq!(
        board.normalize_all_traces_checked(&stop),
        Err(BoardError::Stopped),
        "a budget of 10 still stops it"
    );

    let (mut board, _) = ladder(4);
    assert_eq!(board.normalize_all_traces_checked(&|| false), Ok(true));
    assert!(trace_ids(&board).is_empty());

    let (mut board, _) = ladder(4);
    assert_eq!(board.normalize_all_traces(), Ok(true));
    assert!(trace_ids(&board).is_empty());
}

#[test]
fn the_checked_and_unchecked_normalisation_entry_points_agree() {
    let (mut a, _) = trace_board(1);
    tr(&mut a, 1000, 1, FixedState::Unfixed, &[0, 0, 30000, 0]);
    let mut b = a.clone();
    assert_eq!(
        a.normalize_all_traces(),
        b.normalize_all_traces_checked(&|| false)
    );
    assert_eq!(
        a.normalize_traces(1),
        b.normalize_traces_checked(1, &|| false)
    );
}

fn pin_pad_board() -> (Board, ItemId, ItemId) {
    let ls = LayerStructure::new(vec![Layer::new("l0".to_string(), true)]);
    let cm = ClearanceMatrix::get_default_instance(&ls, 10);
    let mut rules = BoardRules::new(ls.clone(), cm);
    rules.create_default_net_class();
    let default_class = rules.get_default_net_class();

    let mut padstacks = Padstacks::new(ls);
    // A generous pad: 6000 units across, so `(10000, 0)` is well inside it and `(12000, 0)` — the
    // pin centre — is a different point that the same pad also covers.
    let pin_pad = padstacks.add(
        "pin",
        vec![Some(Shape::Tile(TileShape::Box(IntBox::from_coords(
            -3000, -3000, 3000, 3000,
        ))))],
        true,
        false,
    );
    let mut packages = Packages::new();
    let pkg = packages.add(
        "pkg",
        vec![PackagePin::new(
            "P1",
            pin_pad,
            IntVector::new(12000, 0).into(),
            0.0,
        )],
        None,
        None,
        None,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        true,
    );
    let mut components = Components::new();
    components.add_with_generated_name(Some(Point::new(0, 0)), 0.0, true, pkg);

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
        BoardLibrary::new(padstacks, packages),
        components,
        Communication::default(),
    );
    board.rules.nets.add("N1", 1, false, default_class);

    let pin = board.insert_pin(1, 0, vec![1], 1, FixedState::Unfixed);
    let trace = tr(
        &mut board,
        200,
        1,
        FixedState::Unfixed,
        &[0, 0, 20000, 0, 20000, 20000, 10000, 0],
    );
    (board, trace, pin)
}

#[test]
fn a_trace_is_not_cut_inside_a_pin_pad() {
    let (mut board, trace, pin) = pin_pad_board();
    let split_point = Point::new(10000, 0);

    // The premises, asserted rather than assumed — each is a way the test could pass for the
    // wrong reason.
    assert_ne!(
        board.drill_center(pin),
        Some(split_point.clone()),
        "the split point must be inside the pad but NOT at the pin's centre, or \
         `PolylineTrace.java:780-785` allows the split for a different reason"
    );
    let picked = board.pick_items(&split_point, Some(0));
    assert!(
        picked.contains(&pin),
        "the pin pad must cover the split point"
    );
    assert!(
        picked.contains(&trace),
        "and the trace must be picked there too — it is the `currentTrace == this` case"
    );
    assert_eq!(
        board.get_item(trace).and_then(|item| match item {
            Item::Trace(t) => t.last_corner(),
            _ => None,
        }),
        Some(split_point.clone()),
        "the trace's own last corner is the split point — the condition Java's precedence let \
         answer `split allowed`"
    );

    let before = trace_ids(&board);
    let pieces = board
        .split_trace_at_point(trace, &split_point)
        .expect("no normalisation failure");
    assert_eq!(
        pieces, None,
        "a trace must not be cut inside a pin pad — quirk #72"
    );
    assert_eq!(
        trace_ids(&board),
        before,
        "and the board is left exactly as it was"
    );
}
