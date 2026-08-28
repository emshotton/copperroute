//! Plan 2 Task 7: the `ObstacleArea` family, `ConductionArea`, `ComponentOutline` and
//! `BoardOutline`.
//!
//! Java: `board/model/items/{ObstacleArea,ConductionArea,ViaObstacleArea,ComponentObstacleArea,
//! ComponentOutline}.java` and `board/model/structure/BoardOutline.java`.
//!
//! Every expectation below was derived from the Java source and confirmed against
//! `scripts/differential/java/historical/{T7,T7b}.java`, which inline `ObstacleArea.getArea`,
//! `ObstacleArea.splitToConvex`, the three transform bodies and
//! `BoardOutline.getKeepoutArea` over the **real** `app.freerouting.geometry.planar` classes on
//! JDK 23 and print the values asserted here. `ObstacleArea` itself cannot be compiled
//! standalone — it drags in `BasicBoard` and the whole board stack.

use fr_board::prelude::*;
use fr_geometry::{
    Area, FloatPoint, IntBox, IntPoint, Point, PolygonShape, PolylineShapeRef, Shape, TileShape,
    Vector,
};

// ---------------------------------------------------------------------------------------------
// Fixture
// ---------------------------------------------------------------------------------------------

/// The board state the area bodies read through [`ItemCtx`] — Java reaches it through
/// `Item.board`. Three layers, so `changePlacementSide`'s `layerCount - layer - 1`
/// (ObstacleArea.java:250) and `ComponentOutline.getLayer`'s back-side answer
/// (ComponentOutline.java:99) are both distinguishable from 0.
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
        }
    }

    /// `Components.setFlipStyleRotateFirst` (Components.java:196-202), which
    /// `ObstacleArea.getArea` (ObstacleArea.java:127,138) and `rotateApprox`
    /// (ObstacleArea.java:230) branch on.
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
    // ObstacleArea.getArea (ObstacleArea.java:119-144) with rotation 0 and sideChanged false is
    // just `relativeArea.translateBy(translation)`; splitToConvex (:320-326) then splits the
    // absolute area. Driver T7: `splitToConvex: count=2`, boxes `[100,210 .. 110,220]` and
    // `[100,200 .. 120,210]`.
    let f = Fixture::new();
    let area = obstacle_area(1, 0.0, false);

    assert_eq!(area.tile_shape_count(&f.ctx()), 2);
    let tiles = area
        .split_to_convex(&f.ctx())
        .expect("a polygon splits without error");
    assert_eq!(
        boxes(&tiles),
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
    // The relative area is untouched by the translation (ObstacleArea.java:146-148).
    assert_eq!(area.get_relative_area().bounding_box(), bx(0, 0, 20, 20));
}

#[test]
fn a_rotation_that_is_a_multiple_of_90_degrees_uses_turn_90_degree() {
    // ObstacleArea.java:132-133: `rotation % 90 == 0` takes `turn90Degree(rotation / 90, ZERO)`,
    // i.e. the exact integer turn, and only then translates. Driver T7 (rot=90):
    // boxes `[80,200 .. 90,210]`, `[90,200 .. 100,220]`.
    let f = Fixture::new();
    let area = obstacle_area(1, 90.0, false);
    let tiles = area.split_to_convex(&f.ctx()).expect("splits");
    assert_eq!(
        boxes(&tiles),
        vec![bx(80, 200, 90, 210), bx(90, 200, 100, 220)]
    );
    assert_eq!(area.bounding_box(&f.ctx()), bx(80, 200, 100, 220));
}

#[test]
fn a_rotation_that_is_not_a_multiple_of_90_degrees_uses_rotate_approx() {
    // ObstacleArea.java:134-136: the `else` arm is `rotateApprox(toRadians(rotation),
    // FloatPoint.ZERO)`, which produces octagons rather than boxes. Driver T7 (rot=45):
    // `count=3`, all `IntOctagon`.
    let f = Fixture::new();
    let area = obstacle_area(1, 45.0, false);
    let tiles = area.split_to_convex(&f.ctx()).expect("splits");
    assert_eq!(
        boxes(&tiles),
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
    // ObstacleArea.java:127-140: `sideChanged && !flipStyleRotateFirst` mirrors *before* the
    // rotation, `sideChanged && flipStyleRotateFirst` mirrors *after* it. Driver T7 (rot=90,
    // sideChanged=true): rotateFirst=false gives `[90,180 .. 100,190]`, `[80,190 .. 100,200]`;
    // rotateFirst=true gives `[100,210 .. 110,220]`, `[100,200 .. 120,210]` — which is the
    // *unrotated* answer, because this particular L is symmetric about its diagonal.
    let mut f = Fixture::new();
    let area = obstacle_area(1, 90.0, true);

    let tiles = area.split_to_convex(&f.ctx()).expect("splits");
    assert_eq!(
        boxes(&tiles),
        vec![bx(90, 180, 100, 190), bx(80, 190, 100, 200)]
    );

    f.set_flip_style_rotate_first(true);
    // A fresh area: `getArea` memoises, and Java does not invalidate on a flip-style change
    // either.
    let area = obstacle_area(1, 90.0, true);
    let tiles = area.split_to_convex(&f.ctx()).expect("splits");
    assert_eq!(
        boxes(&tiles),
        vec![bx(100, 210, 110, 220), bx(100, 200, 120, 210)]
    );
}

#[test]
fn the_mirror_is_a_vertical_mirror_through_the_origin() {
    // ObstacleArea.java:128/139: `turnedArea.mirrorVertical(Point.ZERO)`. Driver T7b on the
    // asymmetric F shape: bbox `[0,0 .. 30,20]` mirrors to `[-30,0 .. 0,20]`, split into
    // `[-30,0 .. -10,10]` and `[-10,0 .. 0,20]`.
    let f = Fixture::new();
    let area = ObstacleArea::new(
        hdr(1),
        ObstacleAreaData::new(f_shape(), 0, Vector::ZERO, 0.0, true, None),
    );
    let tiles = area.split_to_convex(&f.ctx()).expect("splits");
    assert_eq!(boxes(&tiles), vec![bx(-30, 0, -10, 10), bx(-10, 0, 0, 20)]);
}

// ---------------------------------------------------------------------------------------------
// ObstacleArea transforms
// ---------------------------------------------------------------------------------------------

#[test]
fn translate_by_adds_to_the_translation_and_drops_the_absolute_area_cache() {
    // ObstacleArea.translateBy (ObstacleArea.java:207-211).
    let f = Fixture::new();
    let mut area = obstacle_area(1, 0.0, false);
    assert_eq!(area.bounding_box(&f.ctx()), bx(100, 200, 120, 220));
    area.translate_by(&Vector::new(5, -7));
    assert_eq!(*area.get_translation(), Vector::new(105, 193));
    assert_eq!(area.bounding_box(&f.ctx()), bx(105, 193, 125, 213));
}

#[test]
fn turn_90_degree_wraps_the_rotation_and_turns_the_translation_around_the_pole() {
    // ObstacleArea.turn90Degree (ObstacleArea.java:213-225). Driver T7b:
    // `turn90(1, pole(50,50)) -> (-100,100)`, `300 + 2*90 wrapped = 120`,
    // `30 + (-2)*90 wrapped = 210`.
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
    // ObstacleArea.rotateApprox (ObstacleArea.java:227-244). Driver T7b:
    // `rotateApprox(30, pole(50,50)) float=(18.3013, 204.9038) -> (18,205)`, and
    // `turnAngle when sideChanged && rotateFirst = 330`. Note that only the *stored rotation*
    // takes the complement; the translation is rotated by the original `angleInDegree`
    // (ObstacleArea.java:240-241) — the same split Java's `Component.rotate` has (quirk #48).
    let mut f = Fixture::new();
    let mut area = obstacle_area(1, 0.0, false);
    area.rotate_approx(30.0, &FloatPoint::new(50.0, 50.0), &f.ctx());
    assert_eq!(area.get_rotation_in_degree(), 30.0);
    assert_eq!(*area.get_translation(), Vector::new(18, 205));

    f.set_flip_style_rotate_first(true);
    let mut area = obstacle_area(1, 0.0, true);
    area.rotate_approx(30.0, &FloatPoint::new(50.0, 50.0), &f.ctx());
    assert_eq!(area.get_rotation_in_degree(), 330.0);
    assert_eq!(*area.get_translation(), Vector::new(18, 205));
}

#[test]
fn change_placement_side_flips_the_layer_and_mirrors_the_translation() {
    // ObstacleArea.changePlacementSide (ObstacleArea.java:246-255): `layer = layerCount - layer
    // - 1` and the translation is mirrored vertically at the pole. Driver T7b:
    // `mirrorVertical(pole(50,50)) -> (0,200)`.
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
    // ObstacleArea.copy (ObstacleArea.java:100-118) forwards every geometry field plus `name`.
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
    // ViaObstacleArea (ViaObstacleArea.java:16-42), ComponentObstacleArea
    // (ComponentObstacleArea.java:20-45) and ConductionArea (ConductionArea.java:46-74) all
    // reach `getArea`/`splitToConvex` through `ObstacleArea`.
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
    // ConductionArea.isObstacle (ConductionArea.java:379-385) delegates to the `ObstacleArea`
    // body only when the area's own `isObstacle` flag is set; `isTraceObstacle` (:397-400) and
    // `isDrillable` (:402-405) read the same flag. Java bug/quirk #50 records why this flag,
    // and not `BoardRules.ignoreConduction`, is what `isObstacle` reads.
    let f = Fixture::new();
    let trace = Item::Trace(PolylineTrace::new(hdr_with_nets(9, vec![7])));

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
    // ConductionArea.getTraceConnectionShape (ConductionArea.java:358-365): range-check against
    // `treeShapeCount(searchTree)`, then `getTreeShape(searchTree, index)`.
    let tree = TreeId(0);
    let mut area = ConductionArea::new(hdr_with_nets(1, vec![5]), area_data(0.0, false), true);
    let f = Fixture::new();

    // Cold cache: `treeShapeCount` is 0, so every index is out of range.
    assert_eq!(area.get_trace_connection_shape(tree, 0, &f.ctx()), None);

    let shapes = vec![
        TileShape::Box(bx(0, 0, 10, 10)),
        TileShape::Box(bx(10, 0, 20, 10)),
    ];
    area.hdr.set_precalculated_tree_shapes(tree, shapes.clone());
    assert_eq!(
        area.get_trace_connection_shape(tree, 1, &f.ctx()),
        Some(shapes[1].clone())
    );
    assert_eq!(area.get_trace_connection_shape(tree, 2, &f.ctx()), None);
}

#[test]
fn conduction_area_clear_derived_data_drops_the_absolute_area() {
    // ConductionArea.clearDerivedData (ConductionArea.java:76-81) calls
    // `ObstacleArea.clearDerivedData` (:328-332), which drops `precalculatedAbsoluteArea` and
    // then `Item.clearDerivedData` (Item.java:1060-1065).
    let f = Fixture::new();
    let mut item = Item::ConductionArea(ConductionArea::new(
        hdr_with_nets(1, vec![5]),
        area_data(0.0, false),
        true,
    ));
    assert_eq!(item.bounding_box(&f.ctx()), bx(100, 200, 120, 220));
    item.set_precalculated_tree_shapes(TreeId(0), vec![TileShape::Box(bx(0, 0, 1, 1))]);
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
    // ComponentOutline.getLayer (ComponentOutline.java:93-102).
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
    // ComponentOutline.java:34-54, 72-86, 139-142.
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
    // ComponentOutline.getArea (ComponentOutline.java:191-216) is `ObstacleArea.getArea` with
    // `!this.isFront` where the obstacle area has `this.sideChanged`
    // (ComponentOutline.java:199,210). A back-side outline with rotation 90 therefore matches
    // driver T7's `sideChanged=true, rotateFirst=false` row.
    let f = Fixture::new();
    let outline = component_outline(1, false, 90.0);
    assert_eq!(outline.bounding_box(&f.ctx()), bx(80, 180, 100, 200));
}

#[test]
fn component_outline_change_placement_side_flips_is_front_and_mirrors_the_translation() {
    // ComponentOutline.changePlacementSide (ComponentOutline.java:150-156).
    let f = Fixture::new();
    let mut outline = component_outline(1, true, 0.0);
    outline.change_placement_side(&IntPoint::new(50, 50));
    assert!(!outline.is_front());
    assert_eq!(outline.get_layer(&f.ctx()), 2);
    assert_eq!(*outline.get_translation(), Vector::new(0, 200));
}

#[test]
fn component_outline_rotate_approx_complements_the_angle_on_the_back() {
    // ComponentOutline.rotateApprox (ComponentOutline.java:158-175): `!this.isFront &&
    // flipStyleRotateFirst` takes `360 - angleInDegree`, the mirror image of
    // ObstacleArea.java:230-232.
    let mut f = Fixture::new();
    f.set_flip_style_rotate_first(true);
    let mut outline = component_outline(1, false, 0.0);
    outline.rotate_approx(30.0, &FloatPoint::new(50.0, 50.0), &f.ctx());
    assert_eq!(outline.get_rotation_in_degree(), 330.0);
    assert_eq!(*outline.get_translation(), Vector::new(18, 205));

    // On the front the angle is used as given.
    let mut outline = component_outline(2, true, 0.0);
    outline.rotate_approx(30.0, &FloatPoint::new(50.0, 50.0), &f.ctx());
    assert_eq!(outline.get_rotation_in_degree(), 30.0);
}

#[test]
fn component_outline_clear_derived_data_does_not_clear_the_header() {
    // Java quirk: `ComponentOutline.clearDerivedData` (ComponentOutline.java:218-221) does
    // **not** call `super.clearDerivedData()`, unlike every other override, so the cached tree
    // shapes and the autoroute scratch survive.
    let mut item = Item::ComponentOutline(component_outline(1, true, 0.0));
    item.set_precalculated_tree_shapes(TreeId(0), vec![TileShape::Box(bx(0, 0, 1, 1))]);
    item.get_autoroute_info();

    item.clear_derived_data();
    assert_eq!(item.tree_shape_count(TreeId(0)), 1);
    assert!(item.get_autoroute_info_pur().is_some());
}

#[test]
fn component_outline_copy_carries_the_geometry_but_drops_the_nets() {
    // ComponentOutline.copy (ComponentOutline.java:56-70) forwards the geometry; the
    // constructor hard-codes `new int[0], 0` (ComponentOutline.java:46).
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

/// Two disjoint outline squares — `lineCount` 8 but only 7 keepout tiles, so the two
/// `tileShapeCount` branches (BoardOutline.java:51-66) are distinguishable.
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
    // BoardOutline.getKeepoutArea (BoardOutline.java:183-189):
    // `new PolylineArea(board.boundingBox, shapes.clone())`. Driver T7:
    // `keepout splitToConvex: count=4`, boxes `[0,0 .. 900,100]`, `[0,100 .. 100,1000]`,
    // `[900,0 .. 1000,900]`, `[100,900 .. 1000,1000]`.
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
    // BoardOutline.tileShapeCount (BoardOutline.java:51-66): keepout tiles × layers when
    // `keepoutOutsideOutline`, else `lineCount() × layers`. Driver T7: the two-square keepout
    // splits into 7 tiles, and the two squares have 8 border lines between them.
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
    // BoardOutline.shapeLayer (BoardOutline.java:68-81):
    // `index * layerCount / tileShapeCount()`, 0 when there are no shapes.
    let f = Fixture::new();
    let outline = BoardOutline::new(hdr(1), vec![outline_square()]);
    // 4 border lines × 3 layers = 12 tile shapes.
    assert_eq!(outline.tile_shape_count(&f.ctx()), 12);
    assert_eq!(outline.shape_layer(0, &f.ctx()), 0);
    assert_eq!(outline.shape_layer(3, &f.ctx()), 0);
    assert_eq!(outline.shape_layer(4, &f.ctx()), 1);
    assert_eq!(outline.shape_layer(11, &f.ctx()), 2);

    // No shapes: `shapeCount == 0`, so Java's `else` arm answers 0.
    let empty = BoardOutline::new(hdr(2), Vec::new());
    assert_eq!(empty.tile_shape_count(&f.ctx()), 0);
    assert_eq!(empty.shape_layer(5, &f.ctx()), 0);
}

#[test]
fn board_outline_bounding_box_is_the_union_of_its_shapes() {
    // BoardOutline.boundingBox (BoardOutline.java:88-95) starts from `IntBox.EMPTY`.
    let f = Fixture::new();
    let outline = BoardOutline::new(hdr(1), two_outline_squares());
    assert_eq!(outline.bounding_box(), bx(100, 100, 900, 900));
    assert_eq!(
        BoardOutline::new(hdr(2), Vec::new()).bounding_box(),
        IntBox::EMPTY
    );

    // Item-level layer dispatch: BoardOutline.java:97-110.
    let item = Item::BoardOutline(outline);
    assert_eq!(item.first_layer(&f.ctx()), 0);
    assert_eq!(item.last_layer(&f.ctx()), 2);
    assert!(item.is_on_layer(1, &f.ctx()));
    assert!(item.is_on_layer(99, &f.ctx()));
}

#[test]
fn board_outline_shape_accessors_match_java() {
    // BoardOutline.shapeCount / getShape (BoardOutline.java:157-169).
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
    // Java warns and returns null out of range.
    assert!(outline.get_shape(2).is_none());
    assert_eq!(outline.get_half_width(), 100);
}

#[test]
fn board_outline_transforms_leave_the_outline_shapes_where_they_were() {
    // Java bug: `BoardOutline.translateBy` (BoardOutline.java:112-121) — and its three siblings
    // — assign the transformed shape back to the **loop variable**
    // (`for (PolylineShape currentShape : this.shapes) currentShape = currentShape.translateBy(
    // vector);`), so `this.shapes` is never written. Only the cached `keepoutArea` moves.
    let f = Fixture::new();
    let mut outline = BoardOutline::new(hdr(1), vec![outline_square()]);
    // Fill the keepout cache first, so the second half of the Java body has something to do.
    assert_eq!(
        outline.get_keepout_area(&f.ctx()).bounding_box(),
        bx(0, 0, 1000, 1000)
    );

    outline.translate_by(&Vector::new(10_000, 0));
    assert_eq!(outline.bounding_box(), bx(100, 100, 900, 900));
    assert_eq!(
        outline.get_keepout_area(&f.ctx()).bounding_box(),
        bx(10_000, 0, 11_000, 1000)
    );

    let mut outline = BoardOutline::new(hdr(2), vec![outline_square()]);
    outline.turn_90_degree(1, &IntPoint::new(0, 0));
    outline.rotate_approx(30.0, &FloatPoint::new(0.0, 0.0));
    outline.change_placement_side(&IntPoint::new(0, 0));
    assert_eq!(outline.bounding_box(), bx(100, 100, 900, 900));
}

#[test]
fn board_outline_is_an_obstacle_to_everything_but_outlines_and_areas() {
    // BoardOutline.isObstacle (BoardOutline.java:83-86): `!(other instanceof BoardOutline ||
    // other instanceof ObstacleArea)` — and `instanceof ObstacleArea` covers all four area
    // variants.
    let f = Fixture::new();
    let outline = Item::BoardOutline(BoardOutline::new(hdr(1), vec![outline_square()]));
    let trace = Item::Trace(PolylineTrace::new(hdr_with_nets(2, vec![1])));
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
    // BoardOutline.copy (BoardOutline.java:198-201).
    let original = BoardOutline::new(hdr(1), two_outline_squares());
    let copy = original.copy(ItemId(2));
    assert_eq!(copy.shape_count(), 2);
    assert_eq!(copy.bounding_box(), original.bounding_box());
    assert_eq!(copy.hdr.get_fixed_state(), FixedState::SystemFixed);
}

#[test]
fn generate_keepout_outside_is_a_no_op_when_the_value_does_not_change() {
    // BoardOutline.generateKeepoutOutside (BoardOutline.java:233-244) returns early when the
    // flag already has the requested value.
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
        item.translate_by(&Vector::new(1, 1));
        item.turn_90_degree(1, &IntPoint::new(0, 0));
        item.rotate_approx(10.0, &FloatPoint::new(0.0, 0.0), &f.ctx());
        item.change_placement_side(&IntPoint::new(0, 0), &f.ctx());
        item.clear_derived_data();
    }
}
