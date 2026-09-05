use fr_board::prelude::*;
use fr_geometry::{Area, FloatPoint, IntPoint, Point, PolygonShape, Shape, Vector};

fn layer_structure(count: usize) -> LayerStructure {
    LayerStructure::new(
        (0..count)
            .map(|i| Layer::new(format!("layer{i}"), true))
            .collect(),
    )
}

/// The board state the area bodies read through [`ItemCtx`], with `flipStyleRotateFirst` set —
/// the regime both quirks live in.
struct Fixture {
    library: BoardLibrary,
    components: Components,
    rules: BoardRules,
    bounding_box: fr_geometry::IntBox,
}

impl Fixture {
    fn new() -> Fixture {
        let ls = layer_structure(3);
        let cm = ClearanceMatrix::get_default_instance(&ls, 100);
        let mut components = Components::new();
        components.set_flip_style_rotate_first(true);
        Fixture {
            library: BoardLibrary::new(Padstacks::new(layer_structure(3)), Packages::new()),
            components,
            rules: BoardRules::new(ls, cm),
            bounding_box: fr_geometry::IntBox::from_coords(0, 0, 1000, 1000),
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

fn hdr(id: u32) -> ItemHeader {
    ItemHeader::new(ItemId(id), Vec::new(), 1, 0, FixedState::Unfixed)
}

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

#[test]
fn a_back_side_rotation_keeps_the_outline_and_the_pads_together() {
    let f = Fixture::new();
    let pole_int = IntPoint::new(50, 50);
    let pole_float = FloatPoint::new(50.0, 50.0);
    // The one point all three sites carry, so their answers are directly comparable.
    //
    // `Component` stores an absolute `location`; the two area bodies store a `translation` vector
    // and rotate `ZERO.translateBy(translation)` — the same arithmetic on the same numbers. Giving
    // all three `(150, 250)` therefore puts them on the same input rather than merely on inputs of
    // the same shape, which is what lets the three answers be compared directly below.
    let placement = Point::new(150, 250);
    let offset = Vector::new(150, 250);
    let angle = 90.0;

    let mut components = Components::new();
    components.set_flip_style_rotate_first(true);
    components.add("B1", Some(placement), 0.0, false, 1, 1, false, None);
    components.rotate(1, angle, &pole_int);
    let component_rotation = components.get(1).get_rotation_in_degree();
    let component_point = components
        .get(1)
        .get_location()
        .expect("the component is placed")
        .clone();

    // 2. `ObstacleArea.rotateApprox` — the component's keepout.
    let mut area = ObstacleArea::new(
        hdr(1),
        ObstacleAreaData::new(
            l_shape(),
            1,
            offset.clone(),
            0.0,
            true, // side_changed: the back side, as above
            Some("keepout1".to_string()),
        ),
    );
    area.rotate_approx(angle, &pole_float, &f.ctx());
    let area_rotation = area.get_rotation_in_degree();
    let area_point = Point::ZERO.translate_by(area.get_translation());

    // 3. `ComponentOutline.rotateApprox` — the component's outline.
    let mut outline = ComponentOutline::new(
        hdr(2),
        l_shape(),
        false, // is_front: the back side
        offset,
        0.0,
        true,
        false,
        true,
    );
    outline.rotate_approx(angle, &pole_float, &f.ctx());
    let outline_rotation = outline.get_rotation_in_degree();
    let outline_point = Point::ZERO.translate_by(outline.get_translation());

    // All three record the complement, as they always did.
    assert_eq!(component_rotation, 270.0);
    assert_eq!(area_rotation, 270.0);
    assert_eq!(outline_rotation, 270.0);

    // And all three now *move* by it. `(150,250)` about `(50,50)` is `(100,200)` relative; turned
    // by -90 degrees that is `(200,-100)`, so `(250,-50)` absolute. Before the fix every one of
    // them answered `(-150,150)` — the `+90` point — while recording `270`.
    let expected = Point::new(250, -50);
    assert_eq!(
        component_point, expected,
        "fixed: T11 (#48) — the component's location follows its own rotation field"
    );
    assert_eq!(
        area_point, expected,
        "fixed: T11 (#57) — the keepout's translation follows it too"
    );
    assert_eq!(
        outline_point, expected,
        "fixed: T11 (#57) — and so does the outline's"
    );

    // The invariant, stated on its own: whatever the three answer, they answer it together. This
    // is the assertion a one-sided fix fails, and it fails it even if the literal above is
    // updated to match one of the three.
    assert_eq!(
        (component_rotation, component_point.clone()),
        (area_rotation, area_point),
        "the component and its keepout are rotated by the same rotation"
    );
    assert_eq!(
        (component_rotation, component_point),
        (outline_rotation, outline_point),
        "the component and its outline are rotated by the same rotation"
    );
}

/// The control the answer key insists on: at **180 degrees** the disagreement is `360 - 2·180 = 0`,
/// so the buggy and the fixed code agree exactly. This test would have passed before the fix, and
/// it is here to say so — a reader who reaches for 180 as a "nice round angle" needs to know that
/// it measures nothing.
#[test]
fn at_one_hundred_and_eighty_degrees_the_bug_was_invisible() {
    let mut components = Components::new();
    components.set_flip_style_rotate_first(true);
    components.add(
        "B1",
        Some(Point::new(150, 250)),
        0.0,
        false,
        1,
        1,
        false,
        None,
    );
    components.rotate(1, 180.0, &IntPoint::new(50, 50));
    // `turnAngle = 360 - 180 = 180`, which is `angleInDegree`. Both readings give this.
    assert_eq!(components.get(1).get_rotation_in_degree(), 180.0);
    assert_eq!(
        components.get(1).get_location(),
        Some(&Point::new(-50, -150)),
        "(150,250) about (50,50) turned by 180 is (-50,-150), under either reading"
    );
}
