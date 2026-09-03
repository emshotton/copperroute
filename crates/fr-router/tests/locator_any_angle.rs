//! Plan 9 Task 6, quirk #181: `FoundConnectionLocatorAnyAngle.calculateNextTraceCorners`'s null
//! `resultCorner` at `:287`.
//!
//! `:287` builds `new FloatLine(this.currentFromPoint, resultCorner)` with no null test and
//! `:293` dereferences it; `:329` guards the same value, so the author knew it is nullable.
//! `resultCorner`'s two producers, `rightTurnNextCorner` (`:365-383`) and `leftTurnNextCorner`
//! (`:391-408`), both end in `firstLine.intersection(secondLine)`, and `FloatLine.intersection`
//! answers `null` for two parallel lines.
//!
//! The fixture is `tests/data/t6-parallel-lines.txt` and this file executes its arithmetic. Read
//! that file's closing section first: the two parallel lines are *producible*, which is what is
//! pinned here, but no board in this repository steers a maze backtrack's door corners into them,
//! so this is an input-class fixture and not an end-to-end connection that survives where it used
//! to fail. The guard itself is `crates/fr-router/src/autoroute/path/locator_any_angle.rs`.

use fr_geometry::{FloatLine, FloatPoint};

/// `rightTurnNextCorner`'s two lines (`:373`, `:380-382`), built out of the same two
/// `FloatPoint` tangential primitives the private helper wraps.
fn right_turn_lines(
    from_corner: FloatPoint,
    dist: f64,
    to_corner: FloatPoint,
    next_corner: FloatPoint,
) -> (FloatLine, FloatLine) {
    // :367-373.
    let first_tangential = from_corner
        .left_tangential_point(&to_corner, dist)
        .expect("the fixture's fromCorner is outside the circle around toCorner");
    let first_line = FloatLine::new(from_corner, first_tangential);
    // :374-382. Java's radius is `2 * dist + c_TOLERANCE` with `c_TOLERANCE = 1`.
    let second_tangential = to_corner
        .right_tangential_point(&next_corner, 2.0 * dist + 1.0)
        .expect("the fixture's toCorner is outside the circle around nextCorner");
    let second_line = FloatLine::new(to_corner, second_tangential).translate(dist);
    (first_line, second_line)
}

/// `leftTurnNextCorner`'s mirror (`:399`, `:405-407`).
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

/// The fixture, both handednesses, with the control rows one perturbation away.
///
/// Every literal is `tests/data/t6-parallel-lines.txt`, "The construction".
#[test]
fn a_null_intersection_skips_the_correction_loop() {
    let origin = FloatPoint::new(0.0, 0.0);
    let dist = 30.0;

    // --- rightTurnNextCorner -------------------------------------------------------------------
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

    // --- leftTurnNextCorner --------------------------------------------------------------------
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

    // --- the controls: one perturbation away the intersection exists again ----------------------
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
