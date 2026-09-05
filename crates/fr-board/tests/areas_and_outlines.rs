use fr_board::prelude::*;
use fr_geometry::{
    Area, FloatPoint, IntBox, IntPoint, Point, PolygonShape, PolylineShapeRef, Shape, TileShape,
    Vector,
};

// ---------------------------------------------------------------------------------------------
// Fixture
// ---------------------------------------------------------------------------------------------

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
            bounding_box: IntBox::from_coords(0, 0, 1000, 1000),
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

    fn set_flip_style_rotate_first(&mut self, value: bool) {
        self.components.set_flip_style_rotate_first(value);
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
    ItemHeader::new(ItemId(id), Vec::new(), 1, 0, FixedState::Unfixed)
}

fn hdr_with_nets(id: u32, net_nos: Vec<i32>) -> ItemHeader {
    ItemHeader::new(ItemId(id), net_nos, 1, 0, FixedState::Unfixed)
}

fn unit_polyline() -> fr_geometry::Polyline {
    fr_geometry::Polyline::from_two_points(&Point::new(0, 0), &Point::new(100, 0))
}

/// The L-shaped relative area used by most of the tests: corners `(0,0) (20,0) (20,10) (10,10)
/// (10,20) (0,20)`, which `PolygonShape.splitToConvex` cuts into exactly two boxes.
fn l_shape() -> Area {
    Area::Shape(Shape::Polygon(PolygonShape::from_points(&[
        Point::new(0, 0),
        Point::new(20, 0),
        Point::new(20, 10),
        Point::new(10, 10),
        Point::new(10, 20),
        Point::new(0, 20),
    ])))
}

/// A deliberately *asymmetric* variant of [`l_shape`] (the long arm is 30 wide, not 20), because
/// the L is diagonally symmetric: mirroring it after a 90° turn gives the L back, which would
/// make the `flipStyleRotateFirst` test vacuous.
fn f_shape() -> Area {
    Area::Shape(Shape::Polygon(PolygonShape::from_points(&[
        Point::new(0, 0),
        Point::new(30, 0),
        Point::new(30, 10),
        Point::new(10, 10),
        Point::new(10, 20),
        Point::new(0, 20),
    ])))
}

fn area_data(rotation_in_degree: f64, side_changed: bool) -> ObstacleAreaData {
    ObstacleAreaData::new(
        l_shape(),
        1,
        Vector::new(100, 200),
        rotation_in_degree,
        side_changed,
        Some("keepout1".to_string()),
    )
}

fn obstacle_area(id: u32, rotation_in_degree: f64, side_changed: bool) -> ObstacleArea {
    ObstacleArea::new(hdr(id), area_data(rotation_in_degree, side_changed))
}

fn boxes(tiles: &[TileShape]) -> Vec<IntBox> {
    tiles.iter().map(TileShape::bounding_box).collect()
}

fn bx(llx: i32, lly: i32, urx: i32, ury: i32) -> IntBox {
    IntBox::from_coords(llx, lly, urx, ury)
}

// ---------------------------------------------------------------------------------------------
// ObstacleArea.getArea / splitToConvex
// ---------------------------------------------------------------------------------------------

#[test]
fn an_l_shaped_obstacle_area_splits_into_two_tiles_and_carries_the_translation() {
    let f = Fixture::new();
    let area = obstacle_area(1, 0.0, false);

    assert_eq!(area.tile_shape_count(&f.ctx()), 2);
    let tiles = area
        .split_to_convex(&f.ctx())
        .expect("a polygon splits without error");
    assert_eq!(
        boxes(tiles),
        vec![bx(100, 210, 110, 220), bx(100, 200, 120, 210)]
    );
    assert_eq!(
        area.get_tile_shape(0, &f.ctx()).map(|t| t.bounding_box()),
        Some(bx(100, 210, 110, 220))
    );
    assert_eq!(
        area.get_tile_shape(1, &f.ctx()).map(|t| t.bounding_box()),
        Some(bx(100, 200, 120, 210))
    );
    // ObstacleArea.getTileShape (:197-205) warns and returns null out of range.
    assert_eq!(area.get_tile_shape(2, &f.ctx()), None);

    // ObstacleArea.boundingBox (:169-172) is `getArea().boundingBox()`.
    assert_eq!(area.bounding_box(&f.ctx()), bx(100, 200, 120, 220));
    assert_eq!(area.get_relative_area().bounding_box(), bx(0, 0, 20, 20));
}

#[test]
fn a_rotation_that_is_a_multiple_of_90_degrees_uses_turn_90_degree() {
    let f = Fixture::new();
    let area = obstacle_area(1, 90.0, false);
    let tiles = area.split_to_convex(&f.ctx()).expect("splits");
    assert_eq!(
        boxes(tiles),
        vec![bx(80, 200, 90, 210), bx(90, 200, 100, 220)]
    );
    assert_eq!(area.bounding_box(&f.ctx()), bx(80, 200, 100, 220));
}

#[test]
fn a_rotation_that_is_not_a_multiple_of_90_degrees_uses_rotate_approx() {
    let f = Fixture::new();
    let area = obstacle_area(1, 45.0, false);
    let tiles = area.split_to_convex(&f.ctx()).expect("splits");
    assert_eq!(
        boxes(tiles),
        vec![
            bx(86, 214, 100, 221),
            bx(86, 200, 100, 214),
            bx(100, 200, 114, 221),
        ]
    );
    assert!(tiles.iter().all(|t| matches!(t, TileShape::Octagon(_))));
}

#[test]
fn flip_style_rotate_first_decides_whether_the_mirror_runs_before_or_after_the_rotation() {
    let mut f = Fixture::new();
    let area = obstacle_area(1, 90.0, true);

    let tiles = area.split_to_convex(&f.ctx()).expect("splits");
    assert_eq!(
        boxes(tiles),
        vec![bx(90, 180, 100, 190), bx(80, 190, 100, 200)]
    );

    f.set_flip_style_rotate_first(true);
    let area = obstacle_area(1, 90.0, true);
    let tiles = area.split_to_convex(&f.ctx()).expect("splits");
    assert_eq!(
        boxes(tiles),
        vec![bx(100, 210, 110, 220), bx(100, 200, 120, 210)]
    );
}

#[test]
fn the_mirror_is_a_vertical_mirror_through_the_origin() {
    let f = Fixture::new();
    let area = ObstacleArea::new(
        hdr(1),
        ObstacleAreaData::new(f_shape(), 0, Vector::ZERO, 0.0, true, None),
    );
    let tiles = area.split_to_convex(&f.ctx()).expect("splits");
    assert_eq!(boxes(tiles), vec![bx(-30, 0, -10, 10), bx(-10, 0, 0, 20)]);
}

// ---------------------------------------------------------------------------------------------
// ObstacleArea transforms
// ---------------------------------------------------------------------------------------------

#[test]
fn translate_by_adds_to_the_translation_and_drops_the_absolute_area_cache() {
    let f = Fixture::new();
    let mut area = obstacle_area(1, 0.0, false);
    assert_eq!(area.bounding_box(&f.ctx()), bx(100, 200, 120, 220));
    area.translate_by(&Vector::new(5, -7));
    assert_eq!(*area.get_translation(), Vector::new(105, 193));
    assert_eq!(area.bounding_box(&f.ctx()), bx(105, 193, 125, 213));
}

#[test]
fn turn_90_degree_wraps_the_rotation_and_turns_the_translation_around_the_pole() {
    let mut area = obstacle_area(1, 300.0, false);
    area.turn_90_degree(2, &IntPoint::new(50, 50));
    assert_eq!(area.get_rotation_in_degree(), 120.0);

    let mut area = obstacle_area(1, 30.0, false);
    area.turn_90_degree(-2, &IntPoint::new(50, 50));
    assert_eq!(area.get_rotation_in_degree(), 210.0);

    let mut area = obstacle_area(1, 0.0, false);
    area.turn_90_degree(1, &IntPoint::new(50, 50));
    assert_eq!(*area.get_translation(), Vector::new(-100, 100));
    assert_eq!(area.get_rotation_in_degree(), 90.0);
}

#[test]
fn rotate_approx_rounds_the_new_translation_and_complements_the_angle_when_flipped() {
    let mut f = Fixture::new();
    let mut area = obstacle_area(1, 0.0, false);
    area.rotate_approx(30.0, &FloatPoint::new(50.0, 50.0), &f.ctx());
    assert_eq!(area.get_rotation_in_degree(), 30.0);
    assert_eq!(*area.get_translation(), Vector::new(18, 205));

    // Flipped, with `flipStyleRotateFirst`: the stored rotation is 330 and the translation now
    // follows it. `(100,200)` about `(50,50)` is `(50,150)` relative; rotated by -30 degrees that
    // is `(50·cos30 + 150·sin30, -50·sin30 + 150·cos30) = (118.30, 104.90)`, so `(168.30, 154.90)`
    // absolute, rounding to `(168, 155)`.
    f.set_flip_style_rotate_first(true);
    let mut area = obstacle_area(1, 0.0, true);
    area.rotate_approx(30.0, &FloatPoint::new(50.0, 50.0), &f.ctx());
    assert_eq!(area.get_rotation_in_degree(), 330.0);
    assert_eq!(
        *area.get_translation(),
        Vector::new(168, 155),
        "fixed: T11 (#57) — was (18, 205), the +30 answer under a -30 rotation field"
    );
}

#[test]
fn change_placement_side_flips_the_layer_and_mirrors_the_translation() {
    let f = Fixture::new();
    let mut area = obstacle_area(1, 0.0, false);
    assert_eq!(area.get_layer(), 1);
    assert!(!area.get_side_changed());

    area.change_placement_side(&IntPoint::new(50, 50), &f.ctx());
    assert!(area.get_side_changed());
    // 3 layers: 3 - 1 - 1 = 1.
    assert_eq!(area.get_layer(), 1);
    assert_eq!(*area.get_translation(), Vector::new(0, 200));

    // On a layer that is not the middle one the flip is visible.
    let mut area = ObstacleArea::new(
        hdr(2),
        ObstacleAreaData::new(l_shape(), 0, Vector::ZERO, 0.0, false, None),
    );
    area.change_placement_side(&IntPoint::new(0, 0), &f.ctx());
    assert_eq!(area.get_layer(), 2);
}

// ---------------------------------------------------------------------------------------------
// ObstacleArea copy and the family's `isObstacle`
// ---------------------------------------------------------------------------------------------

#[test]
fn copy_carries_the_whole_geometry_and_the_name() {
    let f = Fixture::new();
    let original = obstacle_area(1, 90.0, true);
    let copy = original.copy(ItemId(2));
    assert_eq!(copy.hdr.id(), ItemId(2));
    assert_eq!(copy.get_layer(), 1);
    assert_eq!(*copy.get_translation(), Vector::new(100, 200));
    assert_eq!(copy.get_rotation_in_degree(), 90.0);
    assert!(copy.get_side_changed());
    assert_eq!(copy.name(), Some("keepout1"));
    assert_eq!(copy.bounding_box(&f.ctx()), original.bounding_box(&f.ctx()));
}

#[test]
fn the_three_subclasses_share_the_obstacle_area_geometry() {
    let f = Fixture::new();
    let via_keepout = ViaObstacleArea::new(hdr(1), area_data(0.0, false));
    let component_keepout = ComponentObstacleArea::new(hdr(2), area_data(0.0, false));
    let conduction = ConductionArea::new(hdr(3), area_data(0.0, false), true);
    for bb in [
        via_keepout.bounding_box(&f.ctx()),
        component_keepout.bounding_box(&f.ctx()),
        conduction.bounding_box(&f.ctx()),
    ] {
        assert_eq!(bb, bx(100, 200, 120, 220));
    }
    assert_eq!(via_keepout.tile_shape_count(&f.ctx()), 2);
    assert_eq!(component_keepout.tile_shape_count(&f.ctx()), 2);
    assert_eq!(conduction.tile_shape_count(&f.ctx()), 2);
}

// ---------------------------------------------------------------------------------------------
// ConductionArea
// ---------------------------------------------------------------------------------------------

#[test]
fn conduction_area_is_obstacle_toggles_with_its_own_flag() {
    let f = Fixture::new();
    let trace = Item::Trace(PolylineTrace::new(
        hdr_with_nets(9, vec![7]),
        unit_polyline(),
        0,
        50,
        None,
    ));

    let mut area = ConductionArea::new(hdr_with_nets(1, vec![5]), area_data(0.0, false), true);
    let mut item = Item::ConductionArea(area.clone());
    assert!(item.is_obstacle(&trace, &f.ctx()));
    assert!(item.is_trace_obstacle(7));
    assert!(!item.is_drillable(7));

    area.set_is_obstacle(false);
    item = Item::ConductionArea(area);
    assert!(!item.is_obstacle(&trace, &f.ctx()));
    assert!(!item.is_trace_obstacle(7));
    assert!(item.is_drillable(7));
}

#[test]
fn conduction_area_trace_connection_shape_is_the_tree_shape() {
    let tree = TreeId(0);
    let mut area = ConductionArea::new(hdr_with_nets(1, vec![5]), area_data(0.0, false), true);
    let f = Fixture::new();

    // Cold cache: `treeShapeCount` is 0, so every index is out of range.
    assert_eq!(area.get_trace_connection_shape(tree, 0, &f.ctx()), None);

    let shapes = vec![
        Some(TileShape::Box(bx(0, 0, 10, 10))),
        Some(TileShape::Box(bx(10, 0, 20, 10))),
    ];
    area.hdr.set_precalculated_tree_shapes(tree, shapes.clone());
    assert_eq!(
        area.get_trace_connection_shape(tree, 1, &f.ctx()),
        shapes[1].clone()
    );
    assert_eq!(area.get_trace_connection_shape(tree, 2, &f.ctx()), None);
}

#[test]
fn conduction_area_clear_derived_data_drops_the_absolute_area() {
    let f = Fixture::new();
    let mut item = Item::ConductionArea(ConductionArea::new(
        hdr_with_nets(1, vec![5]),
        area_data(0.0, false),
        true,
    ));
    assert_eq!(item.bounding_box(&f.ctx()), bx(100, 200, 120, 220));
    item.set_precalculated_tree_shapes(TreeId(0), vec![Some(TileShape::Box(bx(0, 0, 1, 1)))]);
    item.get_autoroute_info();

    item.clear_derived_data();
    assert_eq!(item.tree_shape_count(TreeId(0)), 0);
    assert_eq!(item.get_autoroute_info_pur(), None);
    // The area is recomputed from the (unchanged) relative area, so it is the same box.
    assert_eq!(item.bounding_box(&f.ctx()), bx(100, 200, 120, 220));
}

// ---------------------------------------------------------------------------------------------
// ComponentOutline
// ---------------------------------------------------------------------------------------------

fn component_outline(id: u32, is_front: bool, rotation_in_degree: f64) -> ComponentOutline {
    ComponentOutline::new(
        hdr(id),
        l_shape(),
        is_front,
        Vector::new(100, 200),
        rotation_in_degree,
        true,
        false,
        true,
    )
}

#[test]
fn component_outline_layer_is_zero_on_the_front_and_the_last_layer_on_the_back() {
    let f = Fixture::new();
    assert_eq!(component_outline(1, true, 0.0).get_layer(&f.ctx()), 0);
    assert_eq!(component_outline(2, false, 0.0).get_layer(&f.ctx()), 2);

    let front = Item::ComponentOutline(component_outline(1, true, 0.0));
    assert_eq!(front.first_layer(&f.ctx()), 0);
    assert_eq!(front.last_layer(&f.ctx()), 0);
    assert!(front.is_on_layer(0, &f.ctx()));
    assert!(!front.is_on_layer(2, &f.ctx()));
    assert_eq!(front.shape_layer(17, &f.ctx()), 0);
}

#[test]
fn component_outline_flags_and_area_come_from_the_constructor() {
    let f = Fixture::new();
    let outline = component_outline(1, true, 0.0);
    assert!(outline.is_front());
    assert!(outline.is_courtyard());
    assert!(!outline.is_fabrication());
    assert!(outline.is_closed());
    assert_eq!(outline.bounding_box(&f.ctx()), bx(100, 200, 120, 220));
    assert_eq!(outline.tile_shape_count(), 0);
}

#[test]
fn component_outline_mirrors_when_it_is_on_the_back_not_when_side_changed() {
    let f = Fixture::new();
    let outline = component_outline(1, false, 90.0);
    assert_eq!(outline.bounding_box(&f.ctx()), bx(80, 180, 100, 200));
}

#[test]
fn component_outline_change_placement_side_flips_is_front_and_mirrors_the_translation() {
    let f = Fixture::new();
    let mut outline = component_outline(1, true, 0.0);
    outline.change_placement_side(&IntPoint::new(50, 50));
    assert!(!outline.is_front());
    assert_eq!(outline.get_layer(&f.ctx()), 2);
    assert_eq!(*outline.get_translation(), Vector::new(0, 200));
}

#[test]
fn component_outline_rotate_approx_complements_the_angle_on_the_back() {
    let mut f = Fixture::new();
    f.set_flip_style_rotate_first(true);
    let mut outline = component_outline(1, false, 0.0);
    outline.rotate_approx(30.0, &FloatPoint::new(50.0, 50.0), &f.ctx());
    assert_eq!(outline.get_rotation_in_degree(), 330.0);
    assert_eq!(*outline.get_translation(), Vector::new(168, 155));

    // On the front the angle is used as given.
    let mut outline = component_outline(2, true, 0.0);
    outline.rotate_approx(30.0, &FloatPoint::new(50.0, 50.0), &f.ctx());
    assert_eq!(outline.get_rotation_in_degree(), 30.0);
}

#[test]
fn clear_derived_data_chains_to_the_header() {
    let mut item = Item::ComponentOutline(component_outline(1, true, 0.0));
    item.set_precalculated_tree_shapes(TreeId(0), vec![Some(TileShape::Box(bx(0, 0, 1, 1)))]);
    item.get_autoroute_info();

    item.clear_derived_data();
    assert_eq!(item.tree_shape_count(TreeId(0)), 0);
    assert!(item.get_autoroute_info_pur().is_none());
}

#[test]
fn component_outline_copy_carries_the_geometry_but_drops_the_nets() {
    let f = Fixture::new();
    let original = component_outline(1, false, 90.0);
    let copy = original.copy(ItemId(2));
    assert_eq!(copy.hdr.id(), ItemId(2));
    assert!(copy.hdr.net_nos.is_empty());
    assert_eq!(copy.hdr.clearance_class(), 0);
    assert!(!copy.is_front());
    assert!(copy.is_courtyard());
    assert_eq!(copy.get_rotation_in_degree(), 90.0);
    assert_eq!(copy.bounding_box(&f.ctx()), original.bounding_box(&f.ctx()));
}

// ---------------------------------------------------------------------------------------------
// BoardOutline
// ---------------------------------------------------------------------------------------------

/// The outline square `(100,100)..(900,900)` inside the fixture's `(0,0)..(1000,1000)` board box.
fn outline_square() -> PolylineShapeRef {
    PolylineShapeRef::Polygon(PolygonShape::from_points(&[
        Point::new(100, 100),
        Point::new(900, 100),
        Point::new(900, 900),
        Point::new(100, 900),
    ]))
}

fn two_outline_squares() -> Vec<PolylineShapeRef> {
    vec![
        PolylineShapeRef::Polygon(PolygonShape::from_points(&[
            Point::new(100, 100),
            Point::new(400, 100),
            Point::new(400, 400),
            Point::new(100, 400),
        ])),
        PolylineShapeRef::Polygon(PolygonShape::from_points(&[
            Point::new(600, 600),
            Point::new(900, 600),
            Point::new(900, 900),
            Point::new(600, 900),
        ])),
    ]
}

#[test]
fn board_outline_keepout_area_is_the_board_box_with_the_outlines_as_holes() {
    let f = Fixture::new();
    let outline = BoardOutline::new(hdr(1), vec![outline_square()]);

    let keepout = outline.get_keepout_area(&f.ctx());
    assert_eq!(keepout.bounding_box(), bx(0, 0, 1000, 1000));
    let tiles = keepout.split_to_convex().expect("the cutout succeeds");
    assert_eq!(
        boxes(&tiles),
        vec![
            bx(0, 0, 900, 100),
            bx(0, 100, 100, 1000),
            bx(900, 0, 1000, 900),
            bx(100, 900, 1000, 1000),
        ]
    );
}

#[test]
fn board_outline_tile_shape_count_switches_between_keepout_tiles_and_outline_lines() {
    let f = Fixture::new();
    let mut outline = BoardOutline::new(hdr(1), two_outline_squares());

    assert_eq!(outline.line_count(), 8);
    assert!(!outline.keepout_outside_outline_generated());
    assert_eq!(outline.tile_shape_count(&f.ctx()), 8 * 3);

    outline.generate_keepout_outside(true);
    assert!(outline.keepout_outside_outline_generated());
    assert_eq!(
        outline
            .get_keepout_area(&f.ctx())
            .split_to_convex()
            .expect("the cutout succeeds")
            .len(),
        7
    );
    assert_eq!(outline.tile_shape_count(&f.ctx()), 7 * 3);
}

#[test]
fn board_outline_shape_layer_spreads_the_index_over_the_layers() {
    let f = Fixture::new();
    let outline = BoardOutline::new(hdr(1), vec![outline_square()]);
    // 4 border lines × 3 layers = 12 tile shapes.
    assert_eq!(outline.tile_shape_count(&f.ctx()), 12);
    assert_eq!(outline.shape_layer(0, &f.ctx()), 0);
    assert_eq!(outline.shape_layer(3, &f.ctx()), 0);
    assert_eq!(outline.shape_layer(4, &f.ctx()), 1);
    assert_eq!(outline.shape_layer(11, &f.ctx()), 2);

    let empty = BoardOutline::new(hdr(2), Vec::new());
    assert_eq!(empty.tile_shape_count(&f.ctx()), 0);
    assert_eq!(empty.shape_layer(5, &f.ctx()), 0);
}

#[test]
fn board_outline_bounding_box_is_the_union_of_its_shapes() {
    let f = Fixture::new();
    let outline = BoardOutline::new(hdr(1), two_outline_squares());
    assert_eq!(outline.bounding_box(), bx(100, 100, 900, 900));
    assert_eq!(
        BoardOutline::new(hdr(2), Vec::new()).bounding_box(),
        IntBox::EMPTY
    );

    let item = Item::BoardOutline(outline);
    assert_eq!(item.first_layer(&f.ctx()), 0);
    assert_eq!(item.last_layer(&f.ctx()), 2);
    assert!(item.is_on_layer(1, &f.ctx()));
    assert!(item.is_on_layer(99, &f.ctx()));
}

#[test]
fn board_outline_shape_accessors_match_java() {
    let outline = BoardOutline::new(hdr(1), two_outline_squares());
    assert_eq!(outline.shape_count(), 2);
    assert_eq!(
        outline.get_shape(0).map(|s| s.as_ops().bounding_box()),
        Some(bx(100, 100, 400, 400))
    );
    assert_eq!(
        outline.get_shape(1).map(|s| s.as_ops().bounding_box()),
        Some(bx(600, 600, 900, 900))
    );
    assert!(outline.get_shape(2).is_none());
    assert_eq!(outline.get_half_width(), 100);
}

#[test]
fn the_outline_moves_turns_rotates_and_mirrors() {
    let f = Fixture::new();
    let square = || BoardOutline::new(hdr(1), vec![outline_square()]);
    let base = square();
    let base_box = base.bounding_box();
    let base_lines = base.line_count();
    let base_shape = base.get_shape(0).map(|s| s.as_ops().bounding_box());
    assert_eq!(
        base_box,
        bx(100, 100, 900, 900),
        "the untransformed outline"
    );

    // 1. The outline moved: its bounding box is the transform of its bounding box.
    let mut moved = square();
    moved.translate_by(&Vector::new(10_000, 0));
    assert_eq!(
        moved.bounding_box(),
        bx(10_100, 100, 10_900, 900),
        "translate: the outline moved"
    );

    // 2. Every shape moved, not merely the box.
    assert_eq!(
        moved.get_shape(0).map(|s| s.as_ops().bounding_box()),
        base_shape.map(|b| bx(b.ll.x + 10_000, b.ll.y, b.ur.x + 10_000, b.ur.y)),
        "translate: the shape itself moved"
    );

    // 3. Nothing was lost on the way.
    assert_eq!(
        moved.line_count(),
        base_lines,
        "translate: the line count is preserved"
    );

    // 4. The outline and its keepout stay together — the assertion that was false before the fix
    //    for a *reason* rather than by accident. Build the keepout first, so the transform has to
    //    move an already-materialised one, and check it against the outline's own new box.
    let mut both = square();
    assert_eq!(
        both.get_keepout_area(&f.ctx()).bounding_box(),
        bx(0, 0, 1000, 1000),
        "the keepout is the board box with the outline as its hole"
    );
    both.translate_by(&Vector::new(10_000, 0));
    assert_eq!(
        both.get_keepout_area(&f.ctx()).bounding_box(),
        bx(10_000, 0, 11_000, 1000),
        "the keepout moved"
    );
    assert_eq!(
        both.bounding_box(),
        moved.bounding_box(),
        "and the outline moved with it — an outline that had its keepout built and one that did \
         not now agree, where before the fix only the keepout followed the transform"
    );

    // The other three transforms move the outline too. A quarter turn about the origin sends
    // (100,100 .. 900,900) to (-900,100 .. -100,900); the mirror in x = 0 sends it back.
    let mut turned = square();
    turned.turn_90_degree(1, &IntPoint::new(0, 0));
    assert_eq!(
        turned.bounding_box(),
        bx(-900, 100, -100, 900),
        "turn_90_degree: the outline turned"
    );
    assert_eq!(turned.line_count(), base_lines);

    let mut mirrored = square();
    mirrored.change_placement_side(&IntPoint::new(0, 0));
    assert_eq!(
        mirrored.bounding_box(),
        bx(-900, 100, -100, 900),
        "change_placement_side: the outline mirrored"
    );
    assert_eq!(mirrored.line_count(), base_lines);

    // A 180-degree rotation about the origin is exact even through the float path, so it can be
    // asserted as a literal rather than a tolerance.
    let mut rotated = square();
    rotated.rotate_approx(180.0, &FloatPoint::new(0.0, 0.0));
    assert_eq!(
        rotated.bounding_box(),
        bx(-900, -900, -100, -100),
        "rotate_approx: the outline rotated"
    );
    assert_eq!(rotated.line_count(), base_lines);
}

#[test]
fn board_outline_is_an_obstacle_to_everything_but_outlines_and_areas() {
    let f = Fixture::new();
    let outline = Item::BoardOutline(BoardOutline::new(hdr(1), vec![outline_square()]));
    let trace = Item::Trace(PolylineTrace::new(
        hdr_with_nets(2, vec![1]),
        unit_polyline(),
        0,
        50,
        None,
    ));
    let keepout = Item::ObstacleArea(obstacle_area(3, 0.0, false));
    let conduction = Item::ConductionArea(ConductionArea::new(hdr(4), area_data(0.0, false), true));
    let other_outline = Item::BoardOutline(BoardOutline::new(hdr(5), Vec::new()));

    assert!(outline.is_obstacle(&trace, &f.ctx()));
    assert!(!outline.is_obstacle(&keepout, &f.ctx()));
    assert!(!outline.is_obstacle(&conduction, &f.ctx()));
    assert!(!outline.is_obstacle(&other_outline, &f.ctx()));
}

#[test]
fn board_outline_copy_carries_the_shapes() {
    let original = BoardOutline::new(hdr(1), two_outline_squares());
    let copy = original.copy(ItemId(2));
    assert_eq!(copy.shape_count(), 2);
    assert_eq!(copy.bounding_box(), original.bounding_box());
    assert_eq!(copy.hdr.get_fixed_state(), FixedState::SystemFixed);
}

#[test]
fn generate_keepout_outside_is_a_no_op_when_the_value_does_not_change() {
    let mut outline = BoardOutline::new(hdr(1), vec![outline_square()]);
    assert!(!outline.keepout_outside_outline_generated());
    outline.generate_keepout_outside(false);
    assert!(!outline.keepout_outside_outline_generated());
    outline.generate_keepout_outside(true);
    assert!(outline.keepout_outside_outline_generated());
}

// ---------------------------------------------------------------------------------------------
// Item-level dispatch
// ---------------------------------------------------------------------------------------------

#[test]
fn item_dispatch_reaches_every_area_body() {
    let f = Fixture::new();
    let items = vec![
        Item::ObstacleArea(obstacle_area(1, 0.0, false)),
        Item::ConductionArea(ConductionArea::new(hdr(2), area_data(0.0, false), true)),
        Item::ViaObstacleArea(ViaObstacleArea::new(hdr(3), area_data(0.0, false))),
        Item::ComponentObstacleArea(ComponentObstacleArea::new(hdr(4), area_data(0.0, false))),
    ];
    for item in &items {
        assert_eq!(
            item.bounding_box(&f.ctx()),
            bx(100, 200, 120, 220),
            "{item}"
        );
        assert_eq!(item.tile_shape_count(&f.ctx()), 2, "{item}");
        assert_eq!(item.first_layer(&f.ctx()), 1, "{item}");
        assert_eq!(item.last_layer(&f.ctx()), 1, "{item}");
        assert!(item.is_on_layer(1, &f.ctx()), "{item}");
        assert_eq!(item.shape_layer(0, &f.ctx()), 1, "{item}");
        assert_eq!(
            item.get_tile_shape(TreeId(0), 0, &f.ctx())
                .map(|t| t.bounding_box()),
            Some(bx(100, 210, 110, 220)),
            "{item}"
        );
    }

    // The whole enum stays transform-dispatchable.
    for mut item in items {
        item.translate_by(&Vector::new(1, 1)).unwrap();
        item.turn_90_degree(1, &IntPoint::new(0, 0)).unwrap();
        item.rotate_approx(10.0, &FloatPoint::new(0.0, 0.0), &f.ctx());
        item.change_placement_side(&IntPoint::new(0, 0), &f.ctx())
            .unwrap();
        item.clear_derived_data();
    }
}

#[test]
fn split_to_convex_is_memoised_and_hands_back_the_same_slice() {
    let f = Fixture::new();
    let area = obstacle_area(1, 0.0, false);
    let first = area.split_to_convex(&f.ctx()).expect("splits").as_ptr();
    let second = area.split_to_convex(&f.ctx()).expect("splits").as_ptr();
    assert_eq!(first, second, "the second call must not re-divide the area");
}

#[test]
fn every_obstacle_area_invalidation_point_drops_the_convex_pieces_memo() {
    let f = Fixture::new();

    let mut area = obstacle_area(1, 0.0, false);
    assert_eq!(
        boxes(area.split_to_convex(&f.ctx()).expect("splits")),
        vec![bx(100, 210, 110, 220), bx(100, 200, 120, 210)]
    );
    area.translate_by(&Vector::new(5, 7));
    assert_eq!(
        boxes(area.split_to_convex(&f.ctx()).expect("splits")),
        vec![bx(105, 217, 115, 227), bx(105, 207, 125, 217)]
    );

    let mut area = obstacle_area(2, 0.0, false);
    area.split_to_convex(&f.ctx()).expect("splits");
    area.turn_90_degree(1, &IntPoint::new(0, 0));
    assert_eq!(
        union_box(area.split_to_convex(&f.ctx()).expect("splits")),
        bx(-220, 100, -200, 120)
    );

    let mut area = obstacle_area(3, 0.0, false);
    area.split_to_convex(&f.ctx()).expect("splits");
    area.rotate_approx(90.0, &FloatPoint::new(0.0, 0.0), &f.ctx());
    assert_eq!(
        union_box(area.split_to_convex(&f.ctx()).expect("splits")),
        bx(-220, 100, -200, 120)
    );

    let mut area = obstacle_area(4, 0.0, false);
    area.split_to_convex(&f.ctx()).expect("splits");
    area.change_placement_side(&IntPoint::new(0, 0), &f.ctx());
    assert_eq!(
        union_box(area.split_to_convex(&f.ctx()).expect("splits")),
        bx(-120, 200, -100, 220)
    );

    let mut area = obstacle_area(5, 0.0, false);
    area.split_to_convex(&f.ctx()).expect("splits");
    area.clear_derived_data();
    assert_eq!(
        union_box(area.split_to_convex(&f.ctx()).expect("splits")),
        bx(100, 200, 120, 220)
    );
}

#[test]
fn the_board_outline_keepout_pieces_are_memoised_and_dropped_by_every_transform() {
    let f = Fixture::new();
    let mut outline = BoardOutline::new(
        ItemHeader::new(ItemId(1), Vec::new(), 1, 0, FixedState::SystemFixed),
        vec![PolylineShapeRef::Tile(TileShape::Box(bx(
            100, 100, 300, 300,
        )))],
    );
    let first = outline.keepout_convex_pieces(&f.ctx()).expect("splits");
    let piece_count = first.len();
    let first_ptr = first.as_ptr();
    assert_eq!(
        outline
            .keepout_convex_pieces(&f.ctx())
            .expect("splits")
            .as_ptr(),
        first_ptr,
        "the second call must not re-divide the keepout area"
    );

    let before = union_box(first);
    outline.translate_by(&Vector::new(1000, 0));
    let moved = outline.keepout_convex_pieces(&f.ctx()).expect("splits");
    assert_eq!(moved.len(), piece_count);
    assert_eq!(
        union_box(moved),
        bx(
            before.ll.x + 1000,
            before.ll.y,
            before.ur.x + 1000,
            before.ur.y
        ),
        "translate_by must drop the keepout-pieces memo"
    );
}

fn union_box(tiles: &[TileShape]) -> IntBox {
    tiles
        .iter()
        .fold(IntBox::EMPTY, |acc, t| acc.union(&t.bounding_box()))
}
