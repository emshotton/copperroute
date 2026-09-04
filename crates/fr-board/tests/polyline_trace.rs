
use fr_board::prelude::*;
use fr_geometry::{FloatPoint, IntBox, IntPoint, IntVector, Point, Polyline, TileShape, Vector};


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
            max_tree_shape_width: DEFAULT_MAX_TREE_SHAPE_WIDTH,
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

fn two_point_polyline() -> Polyline {
    Polyline::from_two_points(&p(10_000, 10_000), &p(20_000, 10_000))
}

fn l_polyline() -> Polyline {
    Polyline::from_points(&[p(0, 0), p(10_000, 0), p(10_000, 10_000)])
}

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


#[test]
fn constructor_clamps_the_layer_into_the_board_stack() {
    let too_high = PolylineTrace::new(hdr(1), two_point_polyline(), 9, 1000, Some(3));
    assert_eq!(too_high.get_layer(), 2);
    let no_board = PolylineTrace::new(hdr(1), two_point_polyline(), 9, 1000, None);
    assert_eq!(no_board.get_layer(), 9);
}

#[test]
fn layer_and_half_width_accessors() {
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
    let lines = l_polyline();
    let t = trace(1, lines.clone(), 0, 500);
    assert_eq!(t.polyline(), &lines);
}


#[test]
fn corners_and_corner_count_follow_the_polyline() {
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
    assert_eq!(trace(1, l_polyline(), 0, 500).get_length(), 20_000.0);
    assert_eq!(
        trace(2, bug_report_polyline(), 0, 1000).get_length(),
        53_690.323_694_598_15
    );
}

#[test]
fn bounding_box_is_the_polyline_box_offset_by_the_half_width() {
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
    assert_eq!(
        trace(1, two_point_polyline(), 0, 1000).tile_shape_count(),
        1
    );
    assert_eq!(trace(2, l_polyline(), 0, 500).tile_shape_count(), 2);
    assert_eq!(
        trace(3, bug_report_polyline(), 0, 1000).tile_shape_count(),
        3
    );
    assert_eq!(
        trace(4, Polyline::from_lines(Vec::new()).unwrap(), 0, 500).tile_shape_count(),
        0
    );
}

#[test]
fn offset_shapes_are_the_traces_tree_shapes_before_clearance_compensation() {
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


#[test]
fn trace_connection_shape_is_the_segments_one_dimensional_simplex() {
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
fn trace_connection_shapes_of_the_bug_report_polyline_are_simplices() {
    let f = Fixture::new();
    let t = trace(1, bug_report_polyline(), 0, 1000);
    let expected = [
        IntBox::from_coords(1_270_000, -987_076, 1_291_423, -975_000),
        IntBox::from_coords(1_250_000, -975_000, 1_270_000, -970_000),
        IntBox::from_coords(1_243_227, -970_000, 1_250_000, -964_893),
    ];
    for (i, want) in expected.into_iter().enumerate() {
        let shape = t
            .get_trace_connection_shape(TreeId(0), i, &f.ctx())
            .unwrap_or_else(|| panic!("index {i} is in range"));
        assert!(
            matches!(shape, TileShape::Simplex(_)),
            "connectionShape[{i}] should stay a Simplex, got {shape:?}"
        );
        assert_eq!(shape.bounding_box(), want, "connectionShape[{i}]");
        assert_eq!(shape.dimension(), 1, "connectionShape[{i}]");
    }
}

#[test]
fn offset_shapes_of_the_bug_report_polyline_are_three_simplices() {
    let t = trace(1, bug_report_polyline(), 0, 1000);
    let shapes = t.offset_shapes(t.get_half_width());
    let expected = [
        IntBox::from_coords(1_269_000, -988_076, 1_292_423, -974_000),
        IntBox::from_coords(1_249_000, -976_000, 1_271_000, -969_000),
        IntBox::from_coords(1_242_227, -971_000, 1_251_000, -963_893),
    ];
    assert_eq!(shapes.len(), 3);
    for (i, want) in expected.into_iter().enumerate() {
        assert!(
            matches!(shapes[i], TileShape::Simplex(_)),
            "offsetShape[{i}] should be a Simplex, got {:?}",
            shapes[i]
        );
        assert_eq!(shapes[i].bounding_box(), want, "offsetShape[{i}]");
    }
}

#[test]
fn trace_connection_shape_out_of_range_is_none() {
    let f = Fixture::new();
    let t = trace(1, l_polyline(), 0, 500);
    assert_eq!(t.get_trace_connection_shape(TreeId(0), 2, &f.ctx()), None);
}


#[test]
fn translate_by_moves_the_polyline_and_clears_the_derived_data() {
    let mut t = trace(1, two_point_polyline(), 0, 1000);
    t.hdr
        .set_precalculated_tree_shapes(TreeId(0), vec![Some(TileShape::Box(IntBox::EMPTY))]);
    t.translate_by(&Vector::Int(IntVector::new(1000, -2000)))
        .expect("translating a valid polyline cannot fail");
    assert_eq!(t.first_corner(), Some(p(11_000, 8000)));
    assert_eq!(t.last_corner(), Some(p(21_000, 8000)));
    assert_eq!(t.hdr.get_precalculated_tree_shapes(TreeId(0)), None);
}

#[test]
fn turn_90_degree_turns_the_polyline_and_clears_the_derived_data() {
    let mut t = trace(1, two_point_polyline(), 0, 1000);
    t.hdr
        .set_precalculated_tree_shapes(TreeId(0), vec![Some(TileShape::Box(IntBox::EMPTY))]);
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
    let mut t = trace(1, two_point_polyline(), 0, 1000);
    let stale = vec![Some(TileShape::Box(IntBox::from_coords(1, 2, 3, 4)))];
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
    let f = Fixture::with_layers(4);
    let mut t = trace(1, two_point_polyline(), 1, 1000);
    t.hdr
        .set_precalculated_tree_shapes(TreeId(0), vec![Some(TileShape::Box(IntBox::EMPTY))]);
    t.change_placement_side(&IntPoint::new(0, 0), &f.ctx())
        .expect("mirroring a valid polyline cannot fail");
    assert_eq!(t.first_corner(), Some(p(-10_000, 10_000)));
    assert_eq!(t.last_corner(), Some(p(-20_000, 10_000)));
    assert_eq!(t.get_layer(), 2);
    assert_eq!(t.hdr.get_precalculated_tree_shapes(TreeId(0)), None);
}


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
    assert!(!c.hdr.is_on_the_board());
}


#[test]
fn nearest_end_point_picks_the_closer_corner_and_ties_go_to_the_last() {
    let t = trace(1, two_point_polyline(), 0, 1000);
    assert_eq!(
        t.nearest_end_point(&p(11_000, 10_000)),
        Some(p(10_000, 10_000))
    );
    assert_eq!(
        t.nearest_end_point(&p(19_000, 10_000)),
        Some(p(20_000, 10_000))
    );
    assert_eq!(
        t.nearest_end_point(&p(15_000, 10_000)),
        Some(p(20_000, 10_000))
    );
}


#[test]
fn split_polyline_at_point_cuts_the_segment_the_point_lies_on() {
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
    let t = trace(1, two_point_polyline(), 0, 1000);
    assert_eq!(t.split_polyline_at_point(&p(15_000, 12_345)), Ok(None));
}

#[test]
fn split_polyline_at_point_at_an_end_corner_is_none() {
    let t = trace(1, two_point_polyline(), 0, 1000);
    assert_eq!(t.split_polyline_at_point(&p(10_000, 10_000)), Ok(None));
}

#[test]
fn split_polyline_at_point_on_an_l_shape_keeps_the_remaining_corner() {
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
fn split_polyline_at_point_at_an_interior_corner_splits_into_the_two_arms() {
    let t = trace(1, l_polyline(), 0, 500);
    let [first, second] = t
        .split_polyline_at_point(&p(10_000, 0))
        .unwrap()
        .expect("the interior corner splits the trace");
    assert_eq!(first.first_corner(), Some(p(0, 0)));
    assert_eq!(first.last_corner(), Some(p(10_000, 0)));
    assert_eq!(first.corner_count(), 2);
    assert_eq!(second.first_corner(), Some(p(10_000, 0)));
    assert_eq!(second.last_corner(), Some(p(10_000, 10_000)));
    assert_eq!(second.corner_count(), 2);
}

#[test]
fn perpendicular_split_line_is_none_off_the_segment() {
    let t = trace(1, l_polyline(), 0, 500);
    assert!(t.perpendicular_split_line(0, &p(5000, 0)).is_some());
    assert!(t.perpendicular_split_line(0, &p(10_000, 5000)).is_none());
    assert!(t.perpendicular_split_line(1, &p(10_000, 5000)).is_some());
    assert!(t.perpendicular_split_line(2, &p(5000, 0)).is_none());
}

#[test]
fn split_polyline_at_line_is_the_private_java_overload() {
    let t = trace(1, two_point_polyline(), 0, 1000);
    let vertical = fr_geometry::Line::new(IntPoint::new(15_000, 0), IntPoint::new(15_000, 1));
    let pieces = t
        .split_polyline_at_line(1, &vertical)
        .unwrap()
        .expect("a perpendicular line splits the segment");
    assert_eq!(pieces[0].last_corner(), Some(p(15_000, 10_000)));
    assert_eq!(pieces[1].first_corner(), Some(p(15_000, 10_000)));

    let horizontal = fr_geometry::Line::new(IntPoint::new(0, 10_000), IntPoint::new(1, 10_000));
    assert_eq!(t.split_polyline_at_line(1, &horizontal), Ok(None));
    assert_eq!(t.split_polyline_at_line(0, &vertical), Ok(None));
}


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

#[test]
fn polyline_trace_is_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<PolylineTrace>();
    assert_send_sync::<Item>();
}
