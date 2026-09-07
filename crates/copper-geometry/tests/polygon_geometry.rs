use copper_geometry::{Circle, FloatPoint, IntPoint, Point, PolygonShape, Polyline, Shape};

fn polygon(points: &[(i32, i32)]) -> PolygonShape {
    PolygonShape::from_points(
        &points
            .iter()
            .map(|&(x, y)| Point::Int(IntPoint::new(x, y)))
            .collect::<Vec<_>>(),
    )
}

fn square(x0: i32, y0: i32, x1: i32, y1: i32) -> PolygonShape {
    polygon(&[(x0, y0), (x1, y0), (x1, y1), (x0, y1)])
}

#[test]
fn contains_on_border_distinguishes_inside_from_on_the_edge() {
    let shape = square(0, 0, 10, 10);
    assert!(shape.contains_on_border(&Point::Int(IntPoint::new(5, 0))));
    assert!(shape.contains_on_border(&Point::Int(IntPoint::new(0, 0))));
    assert!(!shape.contains_on_border(&Point::Int(IntPoint::new(5, 5))));
    assert!(shape.contains_inside(&Point::Int(IntPoint::new(5, 5))));
    assert!(!shape.contains_inside(&Point::Int(IntPoint::new(5, 0))));
}

#[test]
fn smallest_radius_of_a_concave_polygon_is_not_zero() {
    let shape = polygon(&[
        (2, 0),
        (4, 0),
        (4, 2),
        (6, 2),
        (6, 4),
        (4, 4),
        (4, 6),
        (2, 6),
        (2, 4),
        (0, 4),
        (0, 2),
        (2, 2),
    ]);
    assert!((shape.smallest_radius() - 2.0_f64.sqrt()).abs() < 1e-9);
}

#[test]
fn enlarge_grows_every_edge_by_the_offset() {
    let enlarged = square(0, 0, 10, 10).enlarge(2.0).unwrap();
    assert_eq!(enlarged.bounding_box().ll, IntPoint::new(-2, -2));
    assert_eq!(enlarged.bounding_box().ur, IntPoint::new(12, 12));
    assert!((enlarged.area() - 196.0).abs() < 1e-9);
}

#[test]
fn border_distance_and_distance_agree_outside_and_differ_inside() {
    let shape = square(0, 0, 10, 10);
    assert_eq!(shape.border_distance(&FloatPoint::new(15.0, 5.0)), 5.0);
    assert_eq!(shape.distance(&FloatPoint::new(15.0, 5.0)), 5.0);
    assert_eq!(shape.border_distance(&FloatPoint::new(5.0, 5.0)), 5.0);
    assert_eq!(shape.distance(&FloatPoint::new(5.0, 5.0)), 0.0);
}

#[test]
fn polygon_cutout_removes_the_inside_part_of_a_polyline() {
    let line = Polyline::from_points(&[
        Point::Int(IntPoint::new(-5, 5)),
        Point::Int(IntPoint::new(15, 5)),
    ]);
    let pieces = square(0, 0, 10, 10).cutout(&line).unwrap();
    assert_eq!(pieces.len(), 2);
    assert_eq!(
        pieces[0].first_corner(),
        Some(Point::Int(IntPoint::new(-5, 5)))
    );
    assert_eq!(
        pieces[0].last_corner(),
        Some(Point::Int(IntPoint::new(0, 5)))
    );
    assert_eq!(
        pieces[1].first_corner(),
        Some(Point::Int(IntPoint::new(10, 5)))
    );
    assert_eq!(
        pieces[1].last_corner(),
        Some(Point::Int(IntPoint::new(15, 5)))
    );
}

#[test]
fn circle_nearest_point_approx_lands_on_the_circumference() {
    let circle = Circle::new(IntPoint::new(2, 3), 10);
    let point = circle
        .nearest_point_approx(&FloatPoint::new(20.0, 3.0))
        .unwrap();
    assert_eq!(point, FloatPoint::new(12.0, 3.0));
    let from_center = circle
        .nearest_point_approx(&FloatPoint::new(2.0, 3.0))
        .unwrap();
    assert_eq!(from_center, FloatPoint::new(12.0, 3.0));
}

#[test]
fn circle_cutout_removes_the_middle_of_a_diameter() {
    let line = Polyline::from_points(&[
        Point::Int(IntPoint::new(-20, 0)),
        Point::Int(IntPoint::new(20, 0)),
    ]);
    let pieces = Circle::new(IntPoint::ZERO, 10).cutout(&line).unwrap();
    assert_eq!(pieces.len(), 2);
    assert!((pieces[0].last_corner().unwrap().to_float().x + 10.0).abs() < 1e-9);
    assert!((pieces[1].first_corner().unwrap().to_float().x - 10.0).abs() < 1e-9);
}

#[test]
fn polygon_against_polygon_intersects_without_recursing() {
    let base = square(0, 0, 10, 10);
    for (other, expected) in [
        (square(20, 20, 30, 30), false),
        (square(10, 2, 12, 8), true),
        (square(8, 8, 12, 12), true),
        (square(2, 2, 8, 8), true),
    ] {
        assert_eq!(base.intersects(&Shape::Polygon(other)), expected);
    }
}
