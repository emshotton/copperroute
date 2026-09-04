//! `:293` dereferences it; `:329` guards the same value, so the author knew it is nullable.
use fr_geometry::{FloatLine, FloatPoint};

fn right_turn_lines(
    from_corner: FloatPoint,
    dist: f64,
    to_corner: FloatPoint,
    next_corner: FloatPoint,
) -> (FloatLine, FloatLine) {
    let first_tangential = from_corner
        .left_tangential_point(&to_corner, dist)
        .expect("the fixture's fromCorner is outside the circle around toCorner");
    let first_line = FloatLine::new(from_corner, first_tangential);
    let second_tangential = to_corner
        .right_tangential_point(&next_corner, 2.0 * dist + 1.0)
        .expect("the fixture's toCorner is outside the circle around nextCorner");
    let second_line = FloatLine::new(to_corner, second_tangential).translate(dist);
    (first_line, second_line)
}

fn left_turn_lines(
    from_corner: FloatPoint,
    dist: f64,
    to_corner: FloatPoint,
    next_corner: FloatPoint,
) -> (FloatLine, FloatLine) {
    let first_tangential = from_corner
        .right_tangential_point(&to_corner, dist)
        .expect("the fixture's fromCorner is outside the circle around toCorner");
    let first_line = FloatLine::new(from_corner, first_tangential);
    let second_tangential = to_corner
        .left_tangential_point(&next_corner, 2.0 * dist + 1.0)
        .expect("the fixture's toCorner is outside the circle around nextCorner");
    let second_line = FloatLine::new(to_corner, second_tangential).translate(-dist);
    (first_line, second_line)
}

#[test]
fn a_null_intersection_skips_the_correction_loop() {
    let origin = FloatPoint::new(0.0, 0.0);
    let dist = 30.0;

    let (first, second) = right_turn_lines(
        origin,
        dist,
        FloatPoint::new(30.0, 100.0),
        FloatPoint::new(-31.0, 200.0),
    );
    assert_eq!(first.a, origin);
    assert_eq!(first.b, FloatPoint::new(0.0, 100.0));
    assert_eq!(second.a, FloatPoint::new(0.0, 100.0));
    assert_eq!(second.b, FloatPoint::new(0.0, 200.0));
    assert_eq!(
        first.intersection(&second),
        None,
        "`:382`'s intersection of two parallel lines is Java's null resultCorner (quirk #181)"
    );

    let (first, second) = left_turn_lines(
        origin,
        dist,
        FloatPoint::new(-30.0, 100.0),
        FloatPoint::new(31.0, 200.0),
    );
    assert_eq!(first.b, FloatPoint::new(0.0, 100.0));
    assert_eq!(second.a, FloatPoint::new(0.0, 100.0));
    assert_eq!(second.b, FloatPoint::new(0.0, 200.0));
    assert_eq!(first.intersection(&second), None);

    let (first, second) = right_turn_lines(
        origin,
        dist,
        FloatPoint::new(30.0, 100.0),
        FloatPoint::new(91.0, 200.0),
    );
    assert_eq!(
        first.intersection(&second),
        Some(FloatPoint::new(0.0, 118.3))
    );
    let (first, second) = left_turn_lines(
        origin,
        dist,
        FloatPoint::new(-30.0, 100.0),
        FloatPoint::new(-91.0, 200.0),
    );
    assert_eq!(
        first.intersection(&second),
        Some(FloatPoint::new(0.0, 118.3))
    );
}
