//! So these two tests are not inversions of a `#[should_panic]` — there was none to invert, which
use copper_geometry::{
    FloatPoint, IntBox, IntPoint, Line, Point, Polygon, Polyline, Simplex, TileShape,
};

#[test]
fn an_empty_polyline_has_no_corners_and_no_negative_array() {
    let collapsed = Polyline::from_lines(vec![
        Line::new(IntPoint::new(0, 0), IntPoint::new(10, 0)),
        Line::new(IntPoint::new(0, 0), IntPoint::new(10, 0)),
    ])
    .expect("two parallel lines normalise to the empty polyline, they do not throw");

    assert_eq!(collapsed.lines().len(), 0);
    assert_eq!(
        collapsed.corner_count(),
        0,
        "Java's `lines.length - 1` is -1 here; the port saturates at 0"
    );
    assert!(collapsed.is_empty());
    assert_eq!(collapsed.corners(), Vec::<Point>::new());
}

#[test]
fn the_two_corner_branch_answers_an_empty_simplex() {
    let angle = std::f64::consts::FRAC_PI_4;
    let pole = FloatPoint::ZERO;

    let shape = TileShape::Simplex(Simplex::from_lines(vec![
        Line::new(IntPoint::new(4, 0), IntPoint::new(0, 2)),
        Line::new(IntPoint::new(4, 0), IntPoint::new(0, 3)),
        Line::new(IntPoint::new(4, -1), IntPoint::new(3, -2)),
        Line::new(IntPoint::new(0, 4), IntPoint::new(2, 2)),
    ]));

    let corners: Vec<Point> = (0..shape.border_line_count())
        .map(|i| {
            Point::Int(
                shape
                    .corner_approx(i)
                    .expect("i < border_line_count")
                    .rotate(angle, &pole)
                    .round(),
            )
        })
        .collect();
    assert_eq!(
        Polygon::new(corners).corner_array().len(),
        2,
        "this is TileShape.java:692-695, the two-corner branch"
    );

    assert!(
        shape.rotate_approx(angle, &pole).is_empty(),
        "the two-corner branch answers Simplex::EMPTY where Java throws"
    );

    assert_eq!(shape.rotate_approx(0.0, &pole), shape);

    let box_shape = TileShape::Box(IntBox::from_coords(0, 0, 10, 10));
    assert_eq!(
        box_shape.rotate_approx(std::f64::consts::FRAC_PI_2, &pole),
        TileShape::Box(IntBox::from_coords(-10, 0, 0, 10))
    );
}

#[test]
fn remove_overlaps_on_a_degenerate_array_normalises_to_a_literal() {
    let h = Line::new(IntPoint::new(0, 0), IntPoint::new(1000, 0));
    let v = Line::new(IntPoint::new(0, 0), IntPoint::new(0, 1000));

    let normalised = Polyline::from_lines(vec![h, v, h, v, h, v])
        .expect("quirk #22: this is the array that used to read tmpArr[-1] and throw");

    assert_eq!(normalised.lines().len(), 0);
    assert_eq!(normalised.corner_count(), 0);
    assert!(normalised.is_empty());

    assert!(
        Polyline::from_lines(vec![h, v, h, v, h, v]).is_ok(),
        "the pass-level catch has nothing left to catch here"
    );

    let corner = Polyline::from_points(&[
        Point::Int(IntPoint::new(0, 0)),
        Point::Int(IntPoint::new(1000, 0)),
        Point::Int(IntPoint::new(1000, 1000)),
    ]);
    assert_eq!(corner.corner_count(), 3);
}
