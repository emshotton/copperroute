//! Plan 2 Task 8: `PolylineTrace` — the geometry half.
//!
//! Java: `board/model/items/Trace.java` and `board/trace/{PolylineTrace,PolylineTraceGeometry}
//! .java`.
//!
//! Every number asserted below was printed by
//! `scripts/differential/java/historical/T8.java`, which inlines the whole of
//! `PolylineTraceGeometry`, `Trace.nearestEndPoint` and the geometry core of
//! `PolylineTrace.split(Point)` over the **real** `app.freerouting.geometry.planar` classes on
//! JDK 23. `PolylineTrace` itself cannot be compiled standalone — it drags in `BasicBoard` and
//! the whole board stack — which is why the bodies are reproduced there, exactly as Tasks 6
//! and 7 did for `Pin` and `ObstacleArea`.
//!
//! The four board-dependent cases of `src/test/java/app/freerouting/board/
//! PolylineTraceSplitTest.java` (`testSplitDoesNotRemoveValidSegments` :61,
//! `testSplitPreservesNonOverlappingSegments` :220, `testCycleDetectionDuringOverlap` :300 and
//! `testCombineAtEndRecoversMissingDefaultTreeEntries` :353) all need a `RoutingBoard`, a search
//! tree and `combine`/`isCycle`.
// ported in Task 9: PolylineTraceSplitTest.testSplitDoesNotRemoveValidSegments (:61),
// testSplitPreservesNonOverlappingSegments (:220), testCycleDetectionDuringOverlap (:300) and
// testCombineAtEndRecoversMissingDefaultTreeEntries (:353) — each drives `board.insertItem`,
// `PolylineTrace.combine`, `Trace.isCycle` or `PolylineTrace.split(IntOctagon)`, none of which
// exists before Tasks 9/11. Their *geometry* (the polylines they build) is reused below.

use fr_board::prelude::*;
use fr_geometry::{FloatPoint, IntBox, IntPoint, IntVector, Point, Polyline, TileShape, Vector};

// ---------------------------------------------------------------------------------------------
// Fixture
// ---------------------------------------------------------------------------------------------

/// The board state the trace bodies read through [`ItemCtx`] — only `changePlacementSide` needs
/// one (`board.getLayerCount()`, PolylineTrace.java:164-166).
struct Fixture {
    library: BoardLibrary,
    components: Components,
    rules: BoardRules,
    bounding_box: IntBox,
}

impl Fixture {
    fn new() -> Fixture {
        Fixture::with_layers(3)
    }

    fn with_layers(count: usize) -> Fixture {
        let ls = layer_structure(count);
        let cm = ClearanceMatrix::get_default_instance(&ls, 100);
        Fixture {
            library: BoardLibrary::new(Padstacks::new(layer_structure(count)), Packages::new()),
            components: Components::new(),
            rules: BoardRules::new(ls, cm),
            bounding_box: IntBox::from_coords(0, 0, 2_000_000, 2_000_000),
        }
    }

    fn ctx(&self) -> ItemCtx<'_> {
        ItemCtx {
            library: &self.library,
            components: &self.components,
            rules: &self.rules,
            bounding_box: &self.bounding_box,
        }
    }
}

fn layer_structure(count: usize) -> LayerStructure {
    LayerStructure::new(
        (0..count)
            .map(|i| Layer::new(format!("layer{i}"), true))
            .collect(),
    )
}

fn hdr(id: u32) -> ItemHeader {
    ItemHeader::new(ItemId(id), vec![1], 1, 0, FixedState::Unfixed)
}

fn p(x: i32, y: i32) -> Point {
    Point::new(x, y)
}

/// Case A of `T8.java`: `new Polyline(new IntPoint(10000, 10000), new IntPoint(20000, 10000))`
/// — the two-point trace of `PolylineTraceSplitTest.testTraceGeometryCharacterization` (:391).
fn two_point_polyline() -> Polyline {
    Polyline::from_two_points(&p(10_000, 10_000), &p(20_000, 10_000))
}

/// Case D of `T8.java`: an L-shaped trace `(0,0) -> (10000,0) -> (10000,10000)`.
fn l_polyline() -> Polyline {
    Polyline::from_points(&[p(0, 0), p(10_000, 0), p(10_000, 10_000)])
}

/// Case C of `T8.java`: the bug-report polyline of
/// `PolylineTraceSplitTest.testSplitDoesNotRemoveValidSegments` (:70-75).
fn bug_report_polyline() -> Polyline {
    Polyline::from_points(&[
        p(1_291_423, -987_076),
        p(1_270_000, -975_000),
        p(1_250_000, -970_000),
        p(1_243_227, -964_893),
    ])
}

fn trace(id: u32, lines: Polyline, layer: usize, half_width: i32) -> PolylineTrace {
    PolylineTrace::new(hdr(id), lines, layer, half_width, None)
}

// ---------------------------------------------------------------------------------------------
// The ported Java test
// ---------------------------------------------------------------------------------------------

/// `PolylineTraceSplitTest.testTraceGeometryCharacterization` (:384-408), minus its
/// `RoutingBoard`: every assertion in it is pure geometry, and the `ShapeSearchTree` argument of
/// `getTraceConnectionShape` is ignored by the body (PolylineTrace.java:917-924).
#[test]
fn trace_geometry_characterization() {
    let t = trace(1, two_point_polyline(), 0, 1000);
    assert_eq!(t.first_corner(), Some(p(10_000, 10_000)));
    assert_eq!(t.last_corner(), Some(p(20_000, 10_000)));
    assert_eq!(t.corner_count(), 2);
    assert_eq!(t.tile_shape_count(), 1);
    assert_eq!(t.get_length(), 10_000.0);
    let f = Fixture::new();
    assert!(
        t.get_trace_connection_shape(TreeId(0), 0, &f.ctx())
            .is_some()
    );
}

// ---------------------------------------------------------------------------------------------
// Constructor and accessors (Trace.java:33-77, PolylineTrace.java:45-60)
// ---------------------------------------------------------------------------------------------

#[test]
fn constructor_clamps_the_layer_into_the_board_stack() {
    // Trace.java:44-47: `layer = Math.max(layer, 0); if (board != null) layer =
    // Math.min(layer, board.getLayerCount() - 1);`
    let too_high = PolylineTrace::new(hdr(1), two_point_polyline(), 9, 1000, Some(3));
    assert_eq!(too_high.get_layer(), 2);
    // With no board Java leaves the layer alone.
    let no_board = PolylineTrace::new(hdr(1), two_point_polyline(), 9, 1000, None);
    assert_eq!(no_board.get_layer(), 9);
}

#[test]
fn layer_and_half_width_accessors() {
    // Trace.java:57-77.
    let mut t = trace(1, two_point_polyline(), 1, 250);
    assert_eq!(t.get_layer(), 1);
    assert_eq!(t.first_layer(), 1);
    assert_eq!(t.last_layer(), 1);
    assert_eq!(t.get_half_width(), 250);
    t.set_layer(2);
    assert_eq!(t.get_layer(), 2);
    assert_eq!(t.last_layer(), 2);
}

#[test]
fn polyline_accessor_returns_the_stored_geometry() {
    // PolylineTrace.java:122-125.
    let lines = l_polyline();
    let t = trace(1, lines.clone(), 0, 500);
    assert_eq!(t.polyline(), &lines);
}

// ---------------------------------------------------------------------------------------------
// PolylineTraceGeometry (T8.java cases A, B, C, D)
// ---------------------------------------------------------------------------------------------

#[test]
fn corners_and_corner_count_follow_the_polyline() {
    // PolylineTraceGeometry.java:23-33.
    let t = trace(1, l_polyline(), 0, 500);
    assert_eq!(t.first_corner(), Some(p(0, 0)));
    assert_eq!(t.last_corner(), Some(p(10_000, 10_000)));
    assert_eq!(t.corner_count(), 3);

    let c = trace(2, bug_report_polyline(), 0, 1000);
    assert_eq!(c.first_corner(), Some(p(1_291_423, -987_076)));
    assert_eq!(c.last_corner(), Some(p(1_243_227, -964_893)));
    assert_eq!(c.corner_count(), 4);
}

#[test]
fn four_collinear_points_collapse_to_a_two_corner_trace() {
    // `PolylineTraceSplitTest.testSplitPreservesNonOverlappingSegments` (:228-233) builds
    // `Polyline(p1..p4)` from four *collinear* points; `Polyline(Line[])`'s
    // remove-consecutive-parallel-lines pass leaves 3 lines, so the trace has 2 corners and 1
    // tile shape, not 4 and 2. T8.java case B.
    let t = trace(
        1,
        Polyline::from_points(&[p(0, 0), p(10_000, 0), p(20_000, 0), p(30_000, 0)]),
        0,
        1000,
    );
    assert_eq!(t.first_corner(), Some(p(0, 0)));
    assert_eq!(t.last_corner(), Some(p(30_000, 0)));
    assert_eq!(t.corner_count(), 2);
    assert_eq!(t.tile_shape_count(), 1);
    assert_eq!(t.get_length(), 30_000.0);
    assert_eq!(
        t.bounding_box(),
        IntBox::from_coords(-1000, -1000, 31_000, 1000)
    );
}

#[test]
fn get_length_is_the_polylines_approximate_length() {
    // PolylineTraceGeometry.java:35-37 = `lines.lengthApprox()`.
    assert_eq!(trace(1, l_polyline(), 0, 500).get_length(), 20_000.0);
    assert_eq!(
        trace(2, bug_report_polyline(), 0, 1000).get_length(),
        53_690.323_694_598_15
    );
}

#[test]
fn bounding_box_is_the_polyline_box_offset_by_the_half_width() {
    // PolylineTraceGeometry.java:39-41 = `lines.boundingBox().offset(halfWidth)`. Note the Task
    // 5 stub's guess (`lines.boundingBox(0, lines.lineCount() - 1)`) is not what Java does.
    assert_eq!(
        trace(1, two_point_polyline(), 0, 1000).bounding_box(),
        IntBox::from_coords(9000, 9000, 21_000, 11_000)
    );
    assert_eq!(
        trace(2, l_polyline(), 0, 500).bounding_box(),
        IntBox::from_coords(-500, -500, 10_500, 10_500)
    );
    assert_eq!(
        trace(3, bug_report_polyline(), 0, 1000).bounding_box(),
        IntBox::from_coords(1_242_227, -988_076, 1_292_423, -963_893)
    );
}

#[test]
fn tile_shape_count_is_line_count_minus_two_floored_at_zero() {
    // PolylineTraceGeometry.java:43-45 = `Math.max(lines.lines.length - 2, 0)`; the Task 5 stub
    // dropped the `max`.
    assert_eq!(
        trace(1, two_point_polyline(), 0, 1000).tile_shape_count(),
        1
    );
    assert_eq!(trace(2, l_polyline(), 0, 500).tile_shape_count(), 2);
    assert_eq!(
        trace(3, bug_report_polyline(), 0, 1000).tile_shape_count(),
        3
    );
    // An empty polyline has no lines at all, where Java's subtraction would be -2.
    assert_eq!(
        trace(4, Polyline::from_lines(Vec::new()).unwrap(), 0, 500).tile_shape_count(),
        0
    );
}

#[test]
fn offset_shapes_are_the_traces_tree_shapes_before_clearance_compensation() {
    // `ShapeSearchTree.calculateTreeShapes(PolylineTrace)` offsets the polyline by the
    // compensated half width; with a compensation of 0 that is `polyline.offsetShapes`.
    // T8.java case D.
    let t = trace(1, l_polyline(), 0, 500);
    let shapes = t.offset_shapes(t.get_half_width());
    assert_eq!(shapes.len(), 2);
    assert_eq!(
        shapes[0].bounding_box(),
        IntBox::from_coords(-500, -500, 10_500, 500)
    );
    assert_eq!(
        shapes[1].bounding_box(),
        IntBox::from_coords(9500, -500, 10_500, 10_500)
    );
}

// ---------------------------------------------------------------------------------------------
// getTraceConnectionShape (PolylineTrace.java:917-924)
// ---------------------------------------------------------------------------------------------

#[test]
fn trace_connection_shape_is_the_segments_one_dimensional_simplex() {
    // T8.java case D: both connection shapes simplify to degenerate `IntBox`es.
    let f = Fixture::new();
    let t = trace(1, l_polyline(), 0, 500);
    let s0 = t
        .get_trace_connection_shape(TreeId(0), 0, &f.ctx())
        .expect("index 0 is in range");
    assert_eq!(s0, TileShape::Box(IntBox::from_coords(0, 0, 10_000, 0)));
    let s1 = t
        .get_trace_connection_shape(TreeId(0), 1, &f.ctx())
        .expect("index 1 is in range");
    assert_eq!(
        s1,
        TileShape::Box(IntBox::from_coords(10_000, 0, 10_000, 10_000))
    );
}

#[test]
fn trace_connection_shape_out_of_range_is_none() {
    // PolylineTrace.java:919-922 warns and returns null.
    let f = Fixture::new();
    let t = trace(1, l_polyline(), 0, 500);
    assert_eq!(t.get_trace_connection_shape(TreeId(0), 2, &f.ctx()), None);
}

// ---------------------------------------------------------------------------------------------
// Transforms (PolylineTrace.java:143-168)
// ---------------------------------------------------------------------------------------------

#[test]
fn translate_by_moves_the_polyline_and_clears_the_derived_data() {
    // PolylineTrace.java:143-147. T8.java case A.
    let mut t = trace(1, two_point_polyline(), 0, 1000);
    t.hdr
        .set_precalculated_tree_shapes(TreeId(0), vec![TileShape::Box(IntBox::EMPTY)]);
    t.translate_by(&Vector::Int(IntVector::new(1000, -2000)))
        .expect("translating a valid polyline cannot fail");
    assert_eq!(t.first_corner(), Some(p(11_000, 8000)));
    assert_eq!(t.last_corner(), Some(p(21_000, 8000)));
    assert_eq!(t.hdr.get_precalculated_tree_shapes(TreeId(0)), None);
}

#[test]
fn turn_90_degree_turns_the_polyline_and_clears_the_derived_data() {
    // PolylineTrace.java:149-153. T8.java case A / D.
    let mut t = trace(1, two_point_polyline(), 0, 1000);
    t.hdr
        .set_precalculated_tree_shapes(TreeId(0), vec![TileShape::Box(IntBox::EMPTY)]);
    t.turn_90_degree(1, &IntPoint::new(0, 0))
        .expect("turning a valid polyline cannot fail");
    assert_eq!(t.first_corner(), Some(p(-10_000, 10_000)));
    assert_eq!(t.last_corner(), Some(p(-10_000, 20_000)));
    assert_eq!(t.hdr.get_precalculated_tree_shapes(TreeId(0)), None);

    let mut l = trace(2, l_polyline(), 0, 500);
    l.turn_90_degree(1, &IntPoint::new(0, 0)).unwrap();
    assert_eq!(l.first_corner(), Some(p(0, 0)));
    assert_eq!(l.last_corner(), Some(p(-10_000, 10_000)));
}

#[test]
fn rotate_approx_rotates_the_polyline_but_does_not_clear_the_derived_data() {
    // Java bug (quirk #60): PolylineTrace.rotateApprox (PolylineTrace.java:155-159) is the only
    // one of the four transforms with no `clearDerivedData()` call, so the cached search-tree
    // shapes survive the rotation. T8.java case A.
    let mut t = trace(1, two_point_polyline(), 0, 1000);
    let stale = vec![TileShape::Box(IntBox::from_coords(1, 2, 3, 4))];
    t.hdr
        .set_precalculated_tree_shapes(TreeId(0), stale.clone());
    t.rotate_approx(90.0, &FloatPoint::new(0.0, 0.0));
    assert_eq!(t.first_corner(), Some(p(-10_000, 10_000)));
    assert_eq!(t.last_corner(), Some(p(-10_000, 20_000)));
    assert_eq!(
        t.hdr.get_precalculated_tree_shapes(TreeId(0)),
        Some(stale.as_slice()),
        "PolylineTrace.rotateApprox does not call clearDerivedData (Java bug, quirk #60)"
    );
}

#[test]
fn change_placement_side_mirrors_vertically_and_flips_the_layer() {
    // PolylineTrace.java:160-168. T8.java case A: mirrorVertical at the origin.
    let f = Fixture::with_layers(4);
    let mut t = trace(1, two_point_polyline(), 1, 1000);
    t.hdr
        .set_precalculated_tree_shapes(TreeId(0), vec![TileShape::Box(IntBox::EMPTY)]);
    t.change_placement_side(&IntPoint::new(0, 0), &f.ctx())
        .expect("mirroring a valid polyline cannot fail");
    assert_eq!(t.first_corner(), Some(p(-10_000, 10_000)));
    assert_eq!(t.last_corner(), Some(p(-20_000, 10_000)));
    // getLayerCount() - layer - 1 = 4 - 1 - 1.
    assert_eq!(t.get_layer(), 2);
    assert_eq!(t.hdr.get_precalculated_tree_shapes(TreeId(0)), None);
}

// ---------------------------------------------------------------------------------------------
// copy (PolylineTrace.java:62-78)
// ---------------------------------------------------------------------------------------------

#[test]
fn copy_carries_the_geometry_and_takes_the_new_id() {
    let mut t = trace(7, l_polyline(), 2, 500);
    t.hdr.set_on_the_board(true);
    let c = t.copy(ItemId(42));
    assert_eq!(c.hdr.id(), ItemId(42));
    assert_eq!(c.polyline(), t.polyline());
    assert_eq!(c.get_layer(), 2);
    assert_eq!(c.get_half_width(), 500);
    assert_eq!(c.hdr.net_nos, vec![1]);
    // Java allocates a fresh object, so `onTheBoard` is not carried (Item.java:258-266 is what
    // puts it back).
    assert!(!c.hdr.is_on_the_board());
}

// ---------------------------------------------------------------------------------------------
// nearestEndPoint (Trace.java:255-269)
// ---------------------------------------------------------------------------------------------

#[test]
fn nearest_end_point_picks_the_closer_corner_and_ties_go_to_the_last() {
    // T8.java case A.
    let t = trace(1, two_point_polyline(), 0, 1000);
    assert_eq!(
        t.nearest_end_point(&p(11_000, 10_000)),
        Some(p(10_000, 10_000))
    );
    assert_eq!(
        t.nearest_end_point(&p(19_000, 10_000)),
        Some(p(20_000, 10_000))
    );
    // `d1 < d2` is false for a tie, so Java answers the *last* corner (Trace.java:263-267).
    assert_eq!(
        t.nearest_end_point(&p(15_000, 10_000)),
        Some(p(20_000, 10_000))
    );
}

// ---------------------------------------------------------------------------------------------
// The pure half of split (PolylineTrace.java:699-712, 719-737)
// ---------------------------------------------------------------------------------------------

#[test]
fn split_polyline_at_point_cuts_the_segment_the_point_lies_on() {
    // T8.java case A.
    let t = trace(1, two_point_polyline(), 0, 1000);
    let [first, second] = t
        .split_polyline_at_point(&p(15_000, 10_000))
        .expect("a valid polyline cannot fail normalisation")
        .expect("the point is on the trace");
    assert_eq!(first.first_corner(), Some(p(10_000, 10_000)));
    assert_eq!(first.last_corner(), Some(p(15_000, 10_000)));
    assert_eq!(first.lines().len(), 3);
    assert_eq!(second.first_corner(), Some(p(15_000, 10_000)));
    assert_eq!(second.last_corner(), Some(p(20_000, 10_000)));
    assert_eq!(second.lines().len(), 3);
}

#[test]
fn split_polyline_at_point_off_the_trace_is_none() {
    // No line segment contains the point, so the Java loop falls through to `return null`.
    let t = trace(1, two_point_polyline(), 0, 1000);
    assert_eq!(t.split_polyline_at_point(&p(15_000, 12_345)), Ok(None));
}

#[test]
fn split_polyline_at_point_at_an_end_corner_is_none() {
    // `Polyline.split` refuses when the new end corner is the polyline's own first/last corner
    // (Polyline.java:770-780), so nothing is split at an endpoint. T8.java case A.
    let t = trace(1, two_point_polyline(), 0, 1000);
    assert_eq!(t.split_polyline_at_point(&p(10_000, 10_000)), Ok(None));
}

#[test]
fn split_polyline_at_point_on_an_l_shape_keeps_the_remaining_corner() {
    // T8.java case D: the second piece keeps the corner at (10000,0), so it has 4 lines.
    let t = trace(1, l_polyline(), 0, 500);
    let [first, second] = t
        .split_polyline_at_point(&p(5000, 0))
        .unwrap()
        .expect("the point is on the first segment");
    assert_eq!(first.first_corner(), Some(p(0, 0)));
    assert_eq!(first.last_corner(), Some(p(5000, 0)));
    assert_eq!(first.corner_count(), 2);
    assert_eq!(second.first_corner(), Some(p(5000, 0)));
    assert_eq!(second.last_corner(), Some(p(10_000, 10_000)));
    assert_eq!(second.corner_count(), 3);
}

#[test]
fn split_polyline_at_line_is_the_private_java_overload() {
    // PolylineTrace.java:730-737: `lines.split(lineIndex, newEndLine)`, with the
    // "array of length 2 expected" guard. A line parallel to the split line cannot split.
    let t = trace(1, two_point_polyline(), 0, 1000);
    let vertical = fr_geometry::Line::new(IntPoint::new(15_000, 0), IntPoint::new(15_000, 1));
    let pieces = t
        .split_polyline_at_line(1, &vertical)
        .unwrap()
        .expect("a perpendicular line splits the segment");
    assert_eq!(pieces[0].last_corner(), Some(p(15_000, 10_000)));
    assert_eq!(pieces[1].first_corner(), Some(p(15_000, 10_000)));

    // Parallel to the trace: Polyline.split returns null.
    let horizontal = fr_geometry::Line::new(IntPoint::new(0, 10_000), IntPoint::new(1, 10_000));
    assert_eq!(t.split_polyline_at_line(1, &horizontal), Ok(None));
    // Out of range.
    assert_eq!(t.split_polyline_at_line(0, &vertical), Ok(None));
}

// ---------------------------------------------------------------------------------------------
// The `Item` dispatch
// ---------------------------------------------------------------------------------------------

#[test]
fn item_dispatch_reaches_the_trace_geometry() {
    let f = Fixture::new();
    let item = Item::Trace(trace(1, l_polyline(), 1, 500));
    assert_eq!(item.first_layer(&f.ctx()), 1);
    assert_eq!(item.last_layer(&f.ctx()), 1);
    assert!(item.is_on_layer(1, &f.ctx()));
    assert!(!item.is_on_layer(0, &f.ctx()));
    assert_eq!(item.shape_layer(0, &f.ctx()), 1);
    assert_eq!(
        item.bounding_box(&f.ctx()),
        IntBox::from_coords(-500, -500, 10_500, 10_500)
    );
    assert_eq!(item.tile_shape_count(&f.ctx()), 2);

    let mut item = item;
    item.translate_by(&Vector::Int(IntVector::new(1, 1)))
        .unwrap();
    item.turn_90_degree(1, &IntPoint::new(0, 0)).unwrap();
    item.rotate_approx(10.0, &FloatPoint::new(0.0, 0.0), &f.ctx());
    item.change_placement_side(&IntPoint::new(0, 0), &f.ctx())
        .unwrap();
    item.clear_derived_data();

    // Item::copy goes through PolylineTrace::copy.
    let copied = Item::Trace(trace(2, two_point_polyline(), 0, 1000))
        .copy(ItemId(9))
        .expect("a trace copy never fails");
    assert_eq!(copied.id(), ItemId(9));
}

#[test]
fn connectable_dispatch_reaches_the_trace_connection_shape() {
    let f = Fixture::new();
    let item = Item::Trace(trace(1, l_polyline(), 0, 500));
    let connectable = item.as_connectable().expect("a trace is connectable");
    assert!(
        connectable
            .as_dyn()
            .get_trace_connection_shape(TreeId(0), 0, &f.ctx())
            .is_some()
    );
}
