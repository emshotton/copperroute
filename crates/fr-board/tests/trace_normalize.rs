//! Plan 2 Task 9: trace normalisation — `PolylineTrace.combine`, `split`, `change` and
//! `normalize`, plus `BasicBoard`'s four normalisation loops.
//!
//! Java: `board/trace/PolylineTrace.java` (`combine` :174, `combineAtStart` :201, `combineAtEnd`
//! :341, `split(IntOctagon)` :464, `split(Point)` :698, the private `split(int, Line)` :719,
//! `splitInsideDrillPadProhibited` :768, `normalize` :801, `change` :936),
//! `board/trace/PolylineTraceNormalization.java` and `board/facade/BasicBoard.java`
//! (`combineTraces` :683, `normalizeTraces` :709, `normalizeAllTraces` :798, `splitTraces` :891).
//!
//! Every expectation below is a line of `scripts/differential/java/P2T11.java`'s output, printed
//! by the **real** `app.freerouting.board.facade.RoutingBoard` on the JVM; each test's first
//! comment names the mode and the scenario letter. `p2t11` mode 7 is `combine`, mode 8 is
//! `split`/`change`/`normalize`, mode 9 is the four board loops, and mode 10 is the
//! `CombineStackOverflowTest` fixture. The handful with no driver line cite the Java source
//! instead — they are the paths the driver cannot reach.
//!
//! # The two Java tests ported here
//!
//! * `src/test/java/app/freerouting/board/PolylineTraceSplitTest.java`'s four board-dependent
//!   cases (`testSplitDoesNotRemoveValidSegments` :61, `testSplitPreservesNonOverlappingSegments`
//!   :220, `testCycleDetectionDuringOverlap` :300 and
//!   `testCombineAtEndRecoversMissingDefaultTreeEntries` :353); the fifth,
//!   `testTraceGeometryCharacterization` (:384), is pure geometry and was ported in Task 8
//!   (`tests/polyline_trace.rs`).
//! * `src/test/java/app/freerouting/fixtures/CombineStackOverflowTest.java`. That test is
//!   **DSN-only** — it drives `DsnReader.readBoard` over
//!   `fixtures/Issue723-CombineStackOverflow.dsn`, and the DSN reader is Plan 3 — so its wiring
//!   is rebuilt by hand here from the fixture's `(wiring …)` section: 4000 collinear 200-unit
//!   segments in a 15-row boustrophedon starting at (130000, -107000), inserted one by one with
//!   `insertTraceWithoutCleaning` and then `normalizeAllTraces`, which is exactly the pair of
//!   board calls `Wiring.java:530-535,347` makes. See `combine_stack_overflow_fixture`.

use fr_board::board::{MAX_NORMALIZATION_DEPTH, MAX_NORMALIZE_ITERATIONS};
use fr_board::error::BoardError;
use fr_board::prelude::*;
use fr_geometry::{
    Area, IntBox, IntVector, Line, Point, PolygonShape, Polyline, PolylineError, PolylineShapeRef,
    Shape, TileShape, Vector,
};

// ---------------------------------------------------------------------------------------------
// Fixture — the twin of `P2T11.traceBoard`, itself the shape
// `PolylineTraceSplitTest.createTestBoard` (:31-49) builds.
// ---------------------------------------------------------------------------------------------

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

/// `[(x,y) …]` for one trace, the format `P2T11.corners` prints.
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

/// The traces still in the item list, in board (descending id) order.
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

/// The trace's polyline, for the tests that need its `Line` objects themselves (quirk #74).
fn board_polyline(board: &Board, id: ItemId) -> &Polyline {
    let Some(Item::Trace(trace)) = board.get_item(id) else {
        panic!("not a trace on the board: {id:?}")
    };
    trace.polyline()
}

/// The trace's default-tree leaves, in order.
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

// ---------------------------------------------------------------------------------------------
// combine (mode 7)
// ---------------------------------------------------------------------------------------------

#[test]
fn combine_at_start_joins_two_collinear_segments() {
    // Mode 7 scenario A: `A combine(#2)=true`, `#2 … corners=[(0,0) (20000,0)]`, `A items=[2 1]
    // revision=4`. `skipLine` is true here (PolylineTrace.java:289), so the joined polyline still
    // has three lines.
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
    // Mode 7 scenario B (PolylineTrace.java:249-252 sets `reverseOrder`).
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
    // Mode 7 scenario C: `C changedArea=0:Oct[10000,0,10000,0,10000,10000,10000,10000]` — the
    // join point, from `routingBoard.joinChangedArea(endCorner.toFloat(), getLayer())`
    // (PolylineTrace.java:474).
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
    // Mode 7 scenario D (PolylineTrace.java:391-394).
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
    // Mode 7 scenario E: `skipLine` is false, so the joined polyline has four lines and two tile
    // shapes (PolylineTrace.java:431-436).
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
    // Mode 7 scenarios F (three traces at one point → `contacts.size() != 1`,
    // PolylineTrace.java:232), G (half width), H (fixed state) and I (`netsEqual`) — all
    // `combine(#2)=false` with the item list untouched.
    for build in [
        // F: a third trace forks the join point.
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
        // G: a different half width.
        |board: &mut Board| {
            tr(board, 500, 1, FixedState::Unfixed, &[10000, 0, 20000, 0]);
        },
        // H: a different fixed state.
        |board: &mut Board| {
            tr(
                board,
                1000,
                1,
                FixedState::ShoveFixed,
                &[10000, 0, 20000, 0],
            );
        },
        // I: a different net.
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
    // Mode 7 scenario J: `J combine(#4)=true`, leaving one trace `[(0,0) (50000,0)]`. This is the
    // iterative loop of PolylineTrace.java:181 — `combineAtStart` first, then `combineAtEnd`,
    // until neither end grows. `CombineStackOverflowTest` is what made it a loop.
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
    // `PolylineTraceSplitTest.testCombineAtEndRecoversMissingDefaultTreeEntries` (:353-379), and
    // mode 7 scenario K: `K entriesBefore=null`, `K combine(#2)=true`, `K firstOnBoard=true
    // secondOnBoard=false`, `K entriesAfter=1`.
    //
    // This is the "path 2 requires tree entries in the default tree" guard
    // (PolylineTrace.java:450-457): in Java a missing entry array is `null` and the optimised
    // `mergeEntriesAtEnd` would dereference it; here it is `None` on the
    // `Vec<Option<LeafId>>`, and the branch it selects — `replaceGeometry` — is the same.
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
    // Java: `board.searchTreeManager.remove(first); first.setOnTheBoard(true);` — a live trace
    // whose board entry was dropped before the geometry mutation.
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
    // Mode 7 scenario L: five lines, three tile shapes, four corners.
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
    // Mode 7 scenario M: `M combine(#3)=true`, `M items=[3 2 1]` — the area survives, because
    // `combine` passes `ignoreAreas = true` and `contacts.removeIf(ConductionArea…)` drops it
    // before the size test (PolylineTrace.java:206-209).
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

// ---------------------------------------------------------------------------------------------
// split and normalize (mode 8)
// ---------------------------------------------------------------------------------------------

#[test]
fn split_preserves_non_overlapping_segments() {
    // `PolylineTraceSplitTest.testSplitPreservesNonOverlappingSegments` (:220-293), and mode 8
    // scenario S1: `S1 split=[#4 #6 #7]`, leaving `[(0,0) (10000,0)]`, `[(10000,0) (20000,0)]`
    // and `[(20000,0) (30000,0)]`. Java's assertion is that a piece touching p1 and a piece
    // touching p4 both survive; the driver pins the whole result.
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
    // `PolylineTraceSplitTest.testSplitDoesNotRemoveValidSegments` (:61-216), and mode 8 scenario
    // S2. Its own assertion is "after the split a piece is still connected to p1"; the driver
    // pins the exact outcome: the combined trace splits at (1243227,-964893) into `#5`
    // (which keeps p1) and `#6`, and nothing is removed as a cycle.
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
    // `S2 pick=#3 first=(1291423,-987076) last=(1241414,-964893)`: the Java test picks the first
    // on-board net-98 trace in `getItems()` order, which is the combined one.
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
    // The segment from p1 is still there, exactly as the Java test demands.
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
    // `PolylineTraceSplitTest.testCycleDetectionDuringOverlap` (:300-349): a trace A-B-C and an
    // overlapping trace B-C must not make A-B-C a cycle.
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
    // Mode 8 scenario S3: `S3 split(clip away)=[#2]` leaves the board untouched, and the same
    // trace with a clip that covers the overlap splits into `[#4 #6 #7]`
    // (PolylineTrace.java:475-479).
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
    // Mode 8 scenario S4: `S4 split=[#3(off)]` — the `DrillItem` branch
    // (PolylineTrace.java:652-659) throws away `split(i + 1, splitLine)`'s result, so
    // `ownTraceSplit` stays false and `result.add(this)` adds the trace that was just removed.
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
    // Mode 8 scenario S10: `S10 split=[#6 #7]`, and the found trace became `#4`/`#5`.
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
    // Mode 8 scenario S8: `S8 split=[]`, `S8 onBoard=false items=[2 1]`
    // (PolylineTrace.java:660-681).
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
    // Mode 8 scenario S9: `S9 split=[#2]` and the board is untouched
    // (PolylineTrace.java:466-470).
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
    // Mode 8 scenario S11: `S11 split=[#2]`, `S11 normalize=false`, board untouched. The refusal
    // is the private `split(int, Line)`'s `isDeletionForbidden` guard
    // (PolylineTrace.java:723-729), added so the outer `normalizeTraces` loop converges.
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
    // Mode 8 scenario S5: `S5 normalize=true` and the board is left with one trace
    // `[(0,0) (30000,0)]` — the split of S1 followed by `combine` on every piece.
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
    // Mode 8 scenario S6.
    let (mut board, _) = trace_board(1);
    let alone = tr(&mut board, 1000, 1, FixedState::Unfixed, &[0, 0, 10000, 0]);
    assert!(!board.normalize_trace(alone, None).expect("no failure"));
    assert_eq!(trace_ids(&board), vec![2]);
}

#[test]
fn normalize_reports_true_when_the_only_change_is_a_combine() {
    // Mode 8 scenario S7 (PolylineTraceNormalization.java:122-125).
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
    // `PolylineTraceNormalization.java:26-41`: over `MAX_NORMALIZATION_DEPTH` the method
    // **returns false** — Java's own comment explains it is safe to do so, because the outer
    // `normalizeTraces` loop reads `false` as "nothing changed". Entered here at the first depth
    // past the cap, on a board where depth 0 would have answered `true` (scenario S5).
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
    // 16 is still under the cap and does the work; 17 is over it and does nothing at all.
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
    // Mode 8 scenario S14, which the JVM prints as
    // `S14 combine=threw ArrayIndexOutOfBoundsException` with the board unchanged.
    //
    // The joined line array `combineAtStart` builds here is `[D, C, B, C, D, X, Y]`, on which
    // `Polyline.removeOverlaps` cancels its way down to `newLength == 0` and then reads
    // `tmpArr[-1]` (Polyline.java:148, quirk #22). `global-constraints.md` requires that this
    // reach the caller as `BoardError::Normalization` and never be swallowed into an empty
    // trace — `combineAtEnd` itself tests `joinedPolyline.lines.length != newLineCount` right
    // after the constructor (PolylineTrace.java:303), so the two outcomes are distinguishable.
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
        Err(BoardError::Normalization(
            PolylineError::NormalizationIndexUnderflow
        ))
    );
    // Java throws before it touches anything (the constructor is at PolylineTrace.java:303, the
    // first mutation at :312), and so does the port.
    assert_eq!(item_ids(&board), before);
    assert_eq!(
        corners(&board, six),
        vec![(0, 0), (0, 0), (2000, 2000), (4000, 2000), (4000, 5000)]
    );

    // `combineTraces`, which is `combine` in a loop, threads it out as well
    // (BasicBoard.java:683-706 has no catch).
    assert_eq!(
        board.combine_traces(1),
        Err(BoardError::Normalization(
            PolylineError::NormalizationIndexUnderflow
        ))
    );

    // Mode 8 scenario S15: `normalize` does **not** propagate it on this board, because its
    // `split` runs first and removes both traces before any `combine` can build the fatal line
    // array — `S15 normalize=true`, `S15 after: (none)`, `S15 normalizeTraces(1)=false`.
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
    // `BasicBoard.insertTrace`'s own `catch (Exception e)` (BasicBoard.java:230-241) — "the
    // segment is skipped and the connection may remain unrouted". It is the only caller of
    // `normalize` that stops a `BoardError`; the new trace stays on the board un-normalised.
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
    // Mode 8 scenario S16: `S16 after: (none)`, `S16 items=[1]` — the insert succeeds and
    // returns the new id, and the normalisation that follows removes both traces. What matters
    // for this test is that `insert_trace` answers `Some` rather than propagating: it is the one
    // caller of `normalize` with a `catch` of its own.
    assert_eq!(inserted, Some(ItemId(3)));
    assert!(trace_ids(&board).is_empty());
    assert_eq!(item_ids(&board), vec![1]);
}

#[test]
fn change_replaces_the_geometry_and_reuses_what_it_can() {
    // Mode 8 scenario S12: a three-corner trace changed to a different three-corner one; the new
    // polyline's first line differs, so `changeEntries(0, 0)` rebuilds both entries
    // (PolylineTrace.java:955-985).
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

/// Quirk #74, and the root cause of the `router-dac2020-bm01` `ripupPassNo >= 2` divergence
/// (Plan 6 Task 17b): `PolylineTrace.change` compares the two line arrays with `!=` — **object
/// identity** (PolylineTrace.java:960, :972) — so a *freshly built* polyline differs at index 0
/// however equal its values are, and Java always falls through to `changeEntries` and the
/// `normalize(clipShape)` tail (`:1001`). Probed on the JVM over `p2t11` mode 8's S5 geometry:
/// Java is left with the single trace `[(0,0) (30000,0)]`.
#[test]
fn change_to_a_value_equal_but_freshly_built_polyline_still_normalizes() {
    let (mut board, _) = trace_board(1);
    // `P2T11` S1/S5: a four-corner trace and a second trace lying on its middle segment.
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
    // A new `Polyline` over the same points: value-equal, but not one of the same `Line`
    // objects.
    let rebuilt = Polyline::from_points(&pts(&[0, 0, 10000, 0, 20000, 0, 30000, 0]));
    assert_eq!(rebuilt, *board_polyline(&board, s5), "value-equal");
    board.change_trace(s5, rebuilt);
    // `S5 after: [(0,0) (30000,0)]` — the split-and-recombine of the `normalize` tail. A value
    // comparison would have taken the ":963" early return and left both traces standing.
    let remaining = trace_ids(&board);
    assert_eq!(remaining.len(), 1, "traces left: {remaining:?}");
    assert_eq!(
        corners(&board, ItemId(remaining[0])),
        vec![(0, 0), (30000, 0)]
    );
}

/// The other half of quirk #74, and the half a value comparison gets wrong: `keepAtStartCount`.
///
/// The new polyline reuses the old `Line` **objects** everywhere except index 2, which is rebuilt
/// with an unchanged *value*, and index 4, which really changes. Java's identity comparison stops
/// at index 2, so `keepAtStartCount` is 0 and every leaf is removed and re-inserted; a value
/// comparison would stop at index 4, keep two leaves, and leave the search tree a different
/// shape — which is exactly how the `router-dac2020-bm01` divergence started.
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
    // Index 2: a fresh `Line` carrying the old one's value — "unchanged" to a value comparison,
    // a different object to Java's `!=`.
    lines[2] = Line::new(old[2].a, old[2].b);
    assert_eq!(lines[2], old[2]);
    assert!(!lines[2].is_same_object(&old[2]));
    // Index 4: a real change, so neither model can take `change`'s early return and the two
    // models are compared on the keep counts alone.
    lines[4] = Line::new(
        fr_geometry::IntPoint::new(25000, 0),
        fr_geometry::IntPoint::new(25000, 20000),
    );

    let entries_before = tree_entries(&board, trace);
    let new_polyline = Polyline::from_lines(lines).expect("a valid polyline");
    assert_eq!(new_polyline.lines().len(), 5, "nothing was normalised away");
    board.change_trace(trace, new_polyline);

    let entries_after = tree_entries(&board, trace);
    assert_eq!(entries_after.len(), 3);
    // `keepAtStartCount == 0`: not one leaf survived. Under a value comparison the first two
    // would have.
    for (i, (after, before)) in entries_after.iter().zip(&entries_before).enumerate() {
        assert_ne!(
            after, before,
            "leaf {i} was reused, so keepAtStartCount > 0"
        );
    }
}

#[test]
fn change_on_a_trace_that_is_off_the_board_only_swaps_the_polyline() {
    // Mode 8 scenario S13: `S13 onBoard=false corners=[(0,0) (30000,0)] entries=null`
    // (PolylineTrace.java:937-941).
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
    // `PolylineTrace.split(Point)` (:698-712) has no `P2T11` line — Java's `Trace.split(Point)`
    // is only reached from the GUI — so this pins the port's own loop against the Java body: the
    // first segment containing the point wins, and the perpendicular through it is the cut.
    let (mut board, _) = trace_board(1);
    let trace = tr(&mut board, 1000, 1, FixedState::Unfixed, &[0, 0, 20000, 0]);
    let pieces = board
        .split_trace_at_point(trace, &Point::new(10000, 0))
        .expect("no normalisation failure")
        .expect("the two pieces");
    assert_eq!(pieces, [Some(ItemId(3)), Some(ItemId(4))]);
    assert_eq!(corners(&board, ItemId(3)), vec![(0, 0), (10000, 0)]);
    assert_eq!(corners(&board, ItemId(4)), vec![(10000, 0), (20000, 0)]);
    // A point off the trace splits nothing (PolylineTrace.java:711).
    assert_eq!(
        board
            .split_trace_at_point(ItemId(3), &Point::new(5000, 5000))
            .expect("no normalisation failure"),
        None
    );
}

// ---------------------------------------------------------------------------------------------
// The board loops (mode 9)
// ---------------------------------------------------------------------------------------------

#[test]
fn insert_trace_normalizes_the_new_trace() {
    // Mode 9 scenarios N1 and N2 — the second with the changed area marked, so the `clipShape`
    // branch (BasicBoard.java:222-229) is taken. Both leave one trace `[(0,0) (20000,0)]`.
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
    // Mode 9 scenario N3: `combineTraces(1)=true` leaves the five net-1 segments as `#6`
    // `[(0,0) (50000,0)]` and the two net-2 segments alone; `combineTraces(-1)=true` then joins
    // those into `#8`, and a third call answers `false`.
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
    // Mode 9 scenario N4: `normalizeTraces(1)=true` then `false`, leaving one trace.
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
    // Mode 9 scenario N5.
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
    // `BasicBoard.normalizeTraces`' first branch (BasicBoard.java:713-727): a net that already
    // hit `MAX_NORMALIZE_ITERATIONS` on this board is never normalised again — the port's
    // `normalize_suppressed_net_nos` is Java's `normalizeSuppressedNetNos` (:96). The cap itself
    // is unreachable on any board this suite can build, so the set is seeded directly, exactly
    // as :747 would.
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
    // `normalizeAllTraces` neither reads nor writes the set (BasicBoard.java:798-885).
    assert!(board.normalize_all_traces().expect("no failure"));
    assert_eq!(trace_ids(&board), vec![3]);
}

#[test]
fn normalize_traces_terminates_within_2000() {
    // The safety valve of BasicBoard.java:64,728-749. A 400-segment chain crossed by a second
    // net's trace converges in a handful of passes, so the cap is never touched — which is what
    // the assertion below checks: the net does **not** end up in the suppressed set, and the
    // board reaches a fixed point (a second call answers `false`).
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
    // Mode 9 scenario N6: `splitTraces(hit)=true` splits both crossing traces at (10000,0), and
    // a location no trace covers answers `false` (BasicBoard.java:891-907).
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
    // Mode 9 scenario N7 — on a **two-layer** board, because `insertVia`'s loop is
    // `fromLayer..toLayer` (BasicBoard.java:289) and a one-layer padstack has `toLayer == 0`,
    // which makes the loop empty.
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
    // Mode 9 scenario N8: `connectToTrace=true`, and the board is left with `#4`
    // `[(10000,5000) (10000,0)]` — the original trace was split by the new stub's normalisation
    // and both halves were then removed as tails by `connectToTrace`'s own tail loop
    // (RoutingBoard.java:1157-1168).
    let (mut board, _) = trace_board(1);
    let trace = tr(&mut board, 1000, 1, FixedState::Unfixed, &[0, 0, 20000, 0]);
    assert!(board.connect_to_trace(&Point::new(10000, 5000), trace, 1000, 1));
    assert_eq!(trace_ids(&board), vec![4]);
    assert_eq!(corners(&board, ItemId(4)), vec![(10000, 5000), (10000, 0)]);
    assert_eq!(item_ids(&board), vec![4, 1]);
}

#[test]
fn remove_trace_tails_combines_what_the_stub_leaves_behind() {
    // Mode 9 scenario N9: the stub `[(10000,0) (20000,0)]` goes, and `combineTraces(netNumber)`
    // (RoutingBoard.java:1236) then joins the triangle's remaining traces into one closed
    // `#4` `[(0,0) (10000,0) (10000,10000) (0,0)]`.
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
    // Mode 9 scenario N10: `DrillItem.moveBy`'s `board.insertTrace` tail
    // (DrillItem.java:137-143). The connecting trace from the old centre to the new one is
    // normalised straight into the trace that was contacting the via, leaving `#4`
    // `[(20000,0) (10000,0) (10000,10000)]`.
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

// ---------------------------------------------------------------------------------------------
// The ported `CombineStackOverflowTest` (mode 10)
// ---------------------------------------------------------------------------------------------

/// The wiring of `fixtures/Issue723-CombineStackOverflow.dsn`, rebuilt by hand: a single GND net
/// drawn as a boustrophedon of 200-unit collinear segments, 280 horizontal per row plus one
/// vertical connector, starting at (130000, -107000). `emitted` is the fixture's own count.
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
        // `Wiring.java:530-535`.
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
    // `CombineStackOverflowTest.combineDoesNotOverflowOnLongCollinearTrace` (:42-78), and mode 10
    // at its full 4000 segments: `inserted=4000`, `normalizeAllTraces=true`, `traces=1`, and a
    // single 31-line trace whose corners are the boustrophedon's turns.
    //
    // Java runs the DSN read on a 256 KiB stack so the pre-fix, once-per-merge recursion in
    // `combine()` overflows deterministically; the port's `combine_trace` is the same `while`
    // loop the fix introduced, so it uses O(1) stack by construction and the whole chain folds
    // into one trace.
    let mut board = combine_stack_overflow_board(4000);
    assert_eq!(board.get_traces().len(), 4000);
    // `Wiring.java:347`.
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

// ---------------------------------------------------------------------------------------------
// The non-terminating ladder (quirk #76) — reproduced, not fixed
// ---------------------------------------------------------------------------------------------

/// **This test does not terminate**, in Java and in the port alike, which is why it is
/// `#[ignore]`d: it is a reproduction of quirk #76, kept executable so that whoever fixes the
/// underlying Java defect has a one-command check.
///
/// Two rails joined by **four or more** rungs on one net make a single
/// `PolylineTrace.normalize(null)` on the last rung loop forever. The loop is inside `split`:
/// each split of a found trace re-reads the overlapping tree entries by *appending* them to the
/// list it is already walking and resets the iterator to its head (quirk #71,
/// PolylineTrace.java:584-588), and with four rungs the board keeps producing fresh
/// intersections faster than the walk retires them. Neither cap helps —
/// `MAX_NORMALIZATION_DEPTH` counts `normalize` recursions and
/// `MAX_NORMALIZE_ITERATIONS` counts `normalizeTraces` passes, and this never leaves the first
/// of either.
///
/// Verified on the JVM against `app.freerouting.board.facade.RoutingBoard` at the same
/// geometry: three rungs answer `normalize=true` with two traces left, on both engines; four
/// rungs return from neither. `RUNGS=3` here is the terminating control.
#[test]
#[ignore = "reproduces quirk #76: this call does not terminate, in Java or in the port"]
fn a_four_rung_ladder_never_finishes_normalizing() {
    let rungs: i32 = std::env::var("RUNGS").map_or(4, |v| v.parse().expect("RUNGS"));
    let (mut board, _) = trace_board(1);
    // The two rails.
    tr(&mut board, 1000, 1, FixedState::Unfixed, &[0, 0, 30000, 0]);
    tr(
        &mut board,
        1000,
        1,
        FixedState::Unfixed,
        &[0, 10000, 30000, 10000],
    );
    // The rungs, at x = 0, 10000, 20000, 30000.
    for i in 0..rungs {
        tr(
            &mut board,
            1000,
            1,
            FixedState::Unfixed,
            &[i * 10000, 0, i * 10000, 10000],
        );
    }
    // The last rung is the highest id: 1 outline + 2 rails + `rungs`.
    let last_rung = ItemId(3 + rungs as u32);
    assert!(board.normalize_trace(last_rung, None).expect("no failure"));
    // Only reached for `RUNGS <= 3`.
    assert!(trace_ids(&board).len() <= 2);
}

/// The terminating sibling of `a_four_rung_ladder_never_finishes_normalizing`, and the first
/// reproduction of quirk #76 in this suite that actually returns: Plan 3 ruling 4 gives
/// `split`'s entry walk a [`StopCheck`], so the same four-rung ladder that hangs
/// `normalize_trace` answers [`BoardError::Stopped`] from `normalize_trace_checked`.
///
/// The stop check is a **counter**, not a clock, so the test is deterministic and cheap: it
/// trips after 500 steps, eighteen times the 28 the three-rung control needs (asserted below,
/// so a regression that makes the terminating case wander is caught too).
///
/// # Where the ladder actually hangs
///
/// Not where quirk #76's row says. The port reaches `Item.getConnectionItems`' walk along the
/// contacts (Item.java:721-777) — which has **no visited set** and therefore circles a closed
/// connection for ever — from `BasicBoard.removeIfCycle` (:1354) long before
/// `PolylineTrace.split`'s entry re-walk becomes a problem: with the stop check only on the
/// entry walk, sixty of its steps took over a minute, all of it inside one `removeIfCycle`.
/// Hence `Board::connection_items_checked`. Java has the identical unguarded walk.
#[test]
fn a_four_rung_ladder_stops_when_the_stop_check_trips() {
    fn ladder(rungs: i32) -> (Board, ItemId) {
        let (mut board, _) = trace_board(1);
        // The two rails.
        tr(&mut board, 1000, 1, FixedState::Unfixed, &[0, 0, 30000, 0]);
        tr(
            &mut board,
            1000,
            1,
            FixedState::Unfixed,
            &[0, 10000, 30000, 10000],
        );
        // The rungs, at x = 0, 10000, 20000, 30000.
        for i in 0..rungs {
            tr(
                &mut board,
                1000,
                1,
                FixedState::Unfixed,
                &[i * 10000, 0, i * 10000, 10000],
            );
        }
        // The last rung is the highest id: 1 outline + 2 rails + `rungs`.
        let last_rung = ItemId(3 + rungs as u32);
        (board, last_rung)
    }

    const BUDGET: u64 = 500;

    // Four rungs: the walk never retires its entries, so the budget is what ends it.
    let (mut board, last_rung) = ladder(4);
    let steps = std::cell::Cell::new(0u64);
    let stop = || {
        steps.set(steps.get() + 1);
        steps.get() > BUDGET
    };
    assert_eq!(
        board.normalize_trace_checked(last_rung, None, &stop),
        Err(BoardError::Stopped),
        "a four-rung ladder must be stopped, not normalised"
    );
    assert_eq!(
        steps.get(),
        BUDGET + 1,
        "the walk ran until the budget ran out"
    );

    // Three rungs, the terminating control: the same budget is never touched, and the
    // normalisation answers exactly what the unchecked call answers.
    let (mut board, last_rung) = ladder(3);
    let steps = std::cell::Cell::new(0u64);
    let stop = || {
        steps.set(steps.get() + 1);
        steps.get() > BUDGET
    };
    assert_eq!(
        board.normalize_trace_checked(last_rung, None, &stop),
        Ok(true),
        "a three-rung ladder normalises"
    );
    assert!(
        steps.get() < BUDGET,
        "the control must finish well inside the budget, used {}",
        steps.get()
    );
    assert!(trace_ids(&board).len() <= 2);
}

/// The same trip through the entry point `fr-dsn` actually calls (Plan 3 ruling 4):
/// `Board::normalize_all_traces_checked`, the port of `Wiring.java:347`'s
/// `board.normalizeAllTraces()`.
#[test]
fn normalize_all_traces_checked_stops_on_the_ladder() {
    let (mut board, _) = trace_board(1);
    tr(&mut board, 1000, 1, FixedState::Unfixed, &[0, 0, 30000, 0]);
    tr(
        &mut board,
        1000,
        1,
        FixedState::Unfixed,
        &[0, 10000, 30000, 10000],
    );
    for i in 0..4 {
        tr(
            &mut board,
            1000,
            1,
            FixedState::Unfixed,
            &[i * 10000, 0, i * 10000, 10000],
        );
    }
    let steps = std::cell::Cell::new(0u64);
    let stop = || {
        steps.set(steps.get() + 1);
        steps.get() > 500
    };
    assert_eq!(
        board.normalize_all_traces_checked(&stop),
        Err(BoardError::Stopped)
    );
}

/// `normalize_all_traces_checked(&|| false)` is what the no-argument
/// [`Board::normalize_all_traces`] delegates to, so every Plan 2 expectation must survive the
/// change: a board with nothing to normalise answers `false` either way.
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
