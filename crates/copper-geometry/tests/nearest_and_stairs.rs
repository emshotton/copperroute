use copper_geometry::{FloatPoint, IntBox, IntPoint, Line, LineSegment, Point, TileShape};

fn bx() -> TileShape {
    TileShape::Box(IntBox::from_coords(0, 0, 10, 10))
}

// =================================================================================================
// #15 — `indexOfNearestCorner` seeded the running minimum with `Double.MIN_VALUE`
// =================================================================================================

/// The case the plan names, and the one that was **already** correct — see the module doc. The box
/// `[0,0 .. 10,10]` has corners `(0,0) (10,0) (10,10) (0,10)`; a point sitting exactly on corner 2
/// is at distance 0, which is the one distance the old seed admitted.
#[test]
fn nearest_corner_at_distance_zero_is_nearest() {
    assert_eq!(
        bx().index_of_nearest_corner(&Point::Int(IntPoint::new(10, 10))),
        2
    );
    assert_eq!(
        bx().index_of_nearest_corner(&Point::Int(IntPoint::new(0, 10))),
        3
    );
}

#[test]
fn nearest_corner_at_non_zero_distance_is_nearest() {
    // (point, the corner it is nearest to). Every one of these answered 0 before the fix.
    let rows = [
        ((9, 9), 2usize), // (10,10), at sqrt(2)
        ((1, 9), 3),      // (0,10),  at sqrt(2)
        ((11, 11), 2),    // (10,10), at sqrt(2) — outside the shape, which does not matter
    ];
    for ((x, y), expected) in rows {
        assert_eq!(
            bx().index_of_nearest_corner(&Point::Int(IntPoint::new(x, y))),
            expected,
            "fixed: T11 (#15) — ({x},{y}) answered 0 before the fix"
        );
    }
}

/// A tie keeps the first minimum, because the comparison is strict `<`. `(5,5)` is at `sqrt(50)`
/// from all four corners of the box, so the answer is 0 — the same `0` the bug produced, but for
/// the right reason. Pinned so that a future change to the comparison has something to fail
/// against.
#[test]
fn a_tie_between_corners_keeps_the_first() {
    assert_eq!(
        bx().index_of_nearest_corner(&Point::Int(IntPoint::new(5, 5))),
        0
    );
}

// =================================================================================================
// #16 — the upward insertion shift copied the wrong element
// =================================================================================================

#[test]
fn the_insertion_shift_copies_the_right_element() {
    let shape = bx();
    let from = FloatPoint::new(-5.0, 5.0);

    for count in 2..=4usize {
        let points = shape.nearest_border_points_approx(&from, count);
        assert_eq!(points.len(), count, "count = {count}");

        // All distinct: a duplicate is exactly what the smeared shift produced.
        for i in 0..points.len() {
            for j in (i + 1)..points.len() {
                assert_ne!(
                    points[i], points[j],
                    "fixed: T11 (#16) — points {i} and {j} are the same point (count = {count})"
                );
            }
        }

        // And sorted by ascending distance, which the shift is there to maintain.
        let dists: Vec<f64> = points.iter().map(|p| p.distance_square(&from)).collect();
        for w in dists.windows(2) {
            assert!(
                w[0] <= w[1],
                "not sorted by distance: {dists:?} (count = {count})"
            );
        }
    }
}

/// `count == 1` never enters the shift, so it answered correctly before the fix and must still.
/// This is the control that says the fix did not move the case every caller actually uses.
#[test]
fn a_single_nearest_border_point_is_unchanged() {
    let shape = bx();
    let from = FloatPoint::new(-5.0, 5.0);
    let one = shape.nearest_border_points_approx(&from, 1);
    assert_eq!(one.len(), 1);
    // It is the nearest of the `count = 4` answer, so the two agree about the winner.
    let four = shape.nearest_border_points_approx(&from, 4);
    assert_eq!(one[0], four[0]);
}

// =================================================================================================
// #13 — `stairApproximation45` called a function of x with a y coordinate
// =================================================================================================

#[test]
fn stair_approximation_45_uses_the_y_function() {
    let width = 2.0;
    for to_the_right in [true, false] {
        // The broken branch: |dy| > |dx|, so `function_of_x` is false.
        let seg = segment(0, 0, 7, 20);
        let stairs = seg.stair_approximation_45(width, to_the_right);
        assert!(!stairs.is_empty(), "the staircase is not empty");
        let margin = width.ceil() as i32 + 1;
        for p in &stairs {
            assert!(
                p.x >= -margin && p.x <= 7 + margin && p.y >= -margin && p.y <= 20 + margin,
                "fixed: T11 (#13) — ({}, {}) is outside the segment's neighbourhood; the whole \
                 staircase was {stairs:?}",
                p.x,
                p.y
            );
        }
        // The staircase runs from one end of the segment to the other.
        assert_eq!(stairs[0], IntPoint::new(0, 0));
        assert_eq!(stairs[stairs.len() - 1], IntPoint::new(7, 20));
    }
}

/// **Invariant 2, transpose symmetry — the strong one.** For a segment and its transpose (x and y
/// swapped), the staircases are transposes of each other. The healthy `function_of_x` branch
/// therefore supplies the answer for the branch #13 broke, with no hand-computed staircase and no
/// jar.
///
/// The transpose swaps handedness, so the comparison is against the **opposite** `to_the_right`.
/// The answer key flags that as the one thing the implementer has to resolve by running the fixed
/// code in both orientations; both are checked here, and the assertion says which one holds.
#[test]
fn the_staircase_of_a_transposed_segment_is_the_transposed_staircase() {
    let width = 2.0;
    for to_the_right in [true, false] {
        let healthy = segment(0, 0, 20, 7).stair_approximation_45(width, to_the_right);
        let broken = segment(0, 0, 7, 20).stair_approximation_45(width, !to_the_right);
        let transposed: Vec<IntPoint> = healthy.iter().map(|p| IntPoint::new(p.y, p.x)).collect();
        assert_eq!(
            broken, transposed,
            "fixed: T11 (#13) — the function-of-y staircase must be the transpose of the \
             function-of-x one at the opposite handedness (to_the_right = {to_the_right})"
        );
    }
}

/// The healthy branch is unmoved by the fix — the answer key's measured
/// `(0,0) -> (20,7)` staircase, kept as a literal so a change to the *other* branch cannot be
/// mistaken for a change to this one.
#[test]
fn the_function_of_x_branch_is_unchanged() {
    let stairs = segment(0, 0, 20, 7).stair_approximation_45(2.0, true);
    assert_eq!(
        stairs,
        vec![
            IntPoint::new(0, 0),
            IntPoint::new(4, 0),
            IntPoint::new(6, 2),
            IntPoint::new(10, 2),
            IntPoint::new(12, 4),
            IntPoint::new(16, 4),
            IntPoint::new(18, 6),
            IntPoint::new(19, 6),
            IntPoint::new(20, 7),
        ]
    );
}

#[test]
fn a_forty_five_degree_segment_is_still_its_two_end_points() {
    assert_eq!(
        segment(0, 0, 10, 10).stair_approximation_45(2.0, true),
        vec![IntPoint::new(0, 0), IntPoint::new(10, 10)]
    );
    assert_eq!(
        segment(0, 0, 10, 0).stair_approximation_45(2.0, true),
        vec![IntPoint::new(0, 0), IntPoint::new(10, 0)]
    );
}

#[test]
fn circle_center_of_a_horizontal_or_vertical_input_is_finite() {
    let a = FloatPoint::new(0.0, 0.0);
    let b = FloatPoint::new(1000.0, 0.0);
    let c = FloatPoint::new(1000.0, 1000.0);
    let expected = Some(FloatPoint::new(500.0, 500.0));

    assert_eq!(a.circle_center(&b, &c), expected);
    assert_eq!(a.circle_center(&c, &b), expected);
    assert_eq!(b.circle_center(&a, &c), expected);
    assert_eq!(b.circle_center(&c, &a), expected);
    assert_eq!(c.circle_center(&a, &b), expected);
    assert_eq!(c.circle_center(&b, &a), expected);
}

/// A `LineSegment` from `(ax, ay)` to `(bx, by)`, closed at each end by the perpendicular through
/// that end point — the same construction `line_segment.rs`'s own tests use.
fn segment(ax: i32, ay: i32, bx: i32, by: i32) -> LineSegment {
    let middle = Line::from_coords(ax, ay, bx, by);
    let perp = middle.direction().turn_45_degree(2);
    let start = Line::from_direction(IntPoint::new(ax, ay), &perp);
    let end = Line::from_direction(IntPoint::new(bx, by), &perp);
    LineSegment::new(start, middle, end)
}
