use copper_geometry::{
    Circle, FloatPoint, IntBox, IntOctagon, IntPoint, Point, PolygonShape, Polyline, TileShape,
    Vector,
};

// =================================================================================================
// #26 — `PolygonShape::area()` always returned 0
// =================================================================================================

#[test]
fn area_is_not_always_zero() {
    // A 100 x 100 square.
    //   shoelace = ½|0 + (100·100 − 100·0) + (100·100 − 0·100) + (0·0 − 0·100)|
    //            = ½|0 + 10000 + 10000 + 0| = 10000
    //   sanity: 100 × 100.
    let square = PolygonShape::from_points(&[
        Point::new(0, 0),
        Point::new(100, 0),
        Point::new(100, 100),
        Point::new(0, 100),
    ]);
    assert_eq!(square.area(), 10000.0, "fixed: T11 (#26) — was 0.0");

    // An L with a 20 x 20 notch out of the top right.
    //   (0,0)→(100,0)      :   0·0   − 100·0   =      0
    //   (100,0)→(100,100)  : 100·100 − 100·0   =  10000
    //   (100,100)→(80,100) : 100·100 −  80·100 =   2000
    //   (80,100)→(80,80)   :  80·80  −  80·100 =  −1600
    //   (80,80)→(0,80)     :  80·80  −   0·80  =   6400
    //   (0,80)→(0,0)       :   0·0   −   0·80  =      0
    //   Σ = 16800  ->  area = 8400
    //   sanity: 100×80 (8000) + the 20×20 notch's complement (400) = 8400.
    let l_shape = PolygonShape::from_points(&[
        Point::new(0, 0),
        Point::new(100, 0),
        Point::new(100, 100),
        Point::new(80, 100),
        Point::new(80, 80),
        Point::new(0, 80),
    ]);
    assert_eq!(l_shape.area(), 8400.0, "fixed: T11 (#26) — was 0.0");
}

#[test]
fn a_degenerate_polygon_has_no_area_and_does_not_underflow() {
    let one = PolygonShape::from_points(&[Point::new(7, 9)]);
    assert_eq!(one.area(), 0.0);
    let two = PolygonShape::from_points(&[Point::new(0, 0), Point::new(10, 0)]);
    assert_eq!(two.area(), 0.0);
}

// =================================================================================================
// #23 — `Polyline(Point, Point)` repeated the start's closing direction
// =================================================================================================

#[test]
fn the_two_point_constructor_agrees_with_the_polygon_one() {
    let a = Point::new(0, 0);
    let b = Point::new(100, 0);
    let two = Polyline::from_two_points(&a, &b);
    let poly = Polyline::from_points(&[a, b]);
    assert_eq!(
        two.lines(),
        poly.lines(),
        "fixed: T11 (#23) — lines[2] was (100,0)->(100,1), the same direction as lines[0]"
    );
    // The measured value, so the inversion says what moved and not only that something did.
    assert_eq!(two.lines()[2].a, IntPoint::new(100, 0));
    assert_eq!(two.lines()[2].b, IntPoint::new(100, -1));
}

// =================================================================================================
// #17 — `IntOctagon::contains(FloatPoint)` was inclusive where its siblings are exclusive
// =================================================================================================

#[test]
fn the_octagon_and_the_box_agree_about_their_border() {
    let oct = TileShape::Octagon(IntOctagon::new(0, 0, 100, 100, -100, 100, 0, 200));
    let bx = TileShape::Box(IntBox::from_coords(0, 0, 100, 100));
    let simplex = TileShape::Simplex(IntBox::from_coords(0, 0, 100, 100).to_simplex());

    // The answer key's three border points, all of which disagreed.
    for p in [
        FloatPoint::new(0.0, 50.0),
        FloatPoint::new(50.0, 0.0),
        FloatPoint::new(100.0, 50.0),
    ] {
        assert!(
            !oct.contains_float(&p),
            "fixed: T11 (#17) — the octagon accepted the border point {p:?}"
        );
        assert_eq!(oct.contains_float(&p), bx.contains_float(&p));
        assert_eq!(oct.contains_float(&p), simplex.contains_float(&p));
    }
    // And the two cases that always agreed still do.
    for p in [FloatPoint::new(50.0, 50.0), FloatPoint::new(-1.0, 50.0)] {
        assert_eq!(oct.contains_float(&p), bx.contains_float(&p), "{p:?}");
        assert_eq!(oct.contains_float(&p), simplex.contains_float(&p), "{p:?}");
    }
}

// =================================================================================================
// #18 — `IntBox::divide_into_sections` skipped the base class's `dimension() == 2` filter
// =================================================================================================

#[test]
fn divide_into_sections_drops_the_degenerate_cells() {
    let b = IntBox::from_coords(0, 0, 6, 6);
    let sections = b.divide_into_sections(1.6);
    assert_eq!(sections.len(), 9, "fixed: T11 (#18) — was 16");
    for s in &sections {
        assert_eq!(s.dimension(), 2, "every section is a section: {s:?}");
    }
    assert_eq!(
        sections.iter().map(IntBox::area).sum::<f64>(),
        b.area(),
        "area is conserved"
    );
}

#[test]
fn a_healthy_box_still_divides_the_way_it_did() {
    let b = IntBox::from_coords(-10_000, -10_000, 10_000, 10_000);
    let sections = b.divide_into_sections(10_000.0);
    assert_eq!(sections.len(), 4);
    assert_eq!(sections.iter().map(IntBox::area).sum::<f64>(), b.area());
}

#[test]
fn a_degenerate_box_answers_no_sections() {
    let b = IntBox::from_coords(0, 0, 100, 0);
    assert_eq!(b.dimension(), 1);
    assert_eq!(b.area(), 0.0);
    assert!(b.divide_into_sections(30.0).is_empty());
}

// =================================================================================================
// #32 — `Circle::translate_by(RationalVector)` returned `this` unchanged
// =================================================================================================

#[test]
#[should_panic(expected = "not implemented for a RationalVector")]
fn a_rational_translation_of_a_circle_fails_rather_than_lying() {
    let c = Circle::new(IntPoint::new(10, 20), 5);
    let rational = Vector::Rational(copper_geometry::RationalVector::new(
        num_bigint::BigInt::from(1),
        num_bigint::BigInt::from(1),
        num_bigint::BigInt::from(2),
    ));
    let _ = c.translate_by(&rational);
}

/// The integer arm is untouched, and so is the zero-vector short circuit that never reached the
/// warning.
#[test]
fn an_integer_translation_of_a_circle_still_translates() {
    let c = Circle::new(IntPoint::new(10, 20), 5);
    assert_eq!(
        c.translate_by(&Vector::new(3, 4)),
        Circle::new(IntPoint::new(13, 24), 5)
    );
    assert_eq!(c.translate_by(&Vector::ZERO), c);
}

// =================================================================================================
// #11 — NOT fixed: the `cutoutFrom` merge branches are still dead
// =================================================================================================

#[test]
fn the_cutout_merge_branches_are_still_dead() {
    let big = IntBox::from_coords(0, 0, 100, 100).to_simplex();

    // A small square cut out of the **corner**: the pieces either side of the cut are adjacent and
    // would merge into one convex piece if the merge logic worked.
    let corner_cut = IntBox::from_coords(0, 0, 20, 20)
        .to_simplex()
        .cutout_from(&big)
        .expect("a corner cut divides");
    // The same square cut out of the **middle**: its pieces cannot merge.
    let middle_cut = IntBox::from_coords(40, 40, 60, 60)
        .to_simplex()
        .cutout_from(&big)
        .expect("a middle cut divides");

    assert_eq!(
        corner_cut.len(),
        middle_cut.len(),
        "both merge branches are dead, so the corner cut is divided as finely as the middle one; \
         when #11 is finished these counts must differ, and that is the observable to check"
    );
}
