//! Plan 9 Task 6, the geometry half of the guard cluster: quirks #24 and #25.
//!
//! Both rows describe Java crashes on a **degenerate** polyline — one that normalisation has
//! emptied or reduced to two corners — and for both, the guard the register asks for was already
//! in this port before Plan 9. `Polyline.cornerCount()` cannot return Java's -1 because it returns
//! `usize`, and `TileShape.rotateApprox`'s two-corner arm has answered `Simplex::EMPTY` since
//! Plan 2 rather than building the three-`null` `LineSegment` Java's `:694` builds.
//!
//! So these two tests are not inversions of a `#[should_panic]` — there was none to invert, which
//! is a correction to the task brief's premise rather than a gap in it. What they add is a
//! *binding* statement of the guarded answer, so that a later change which reintroduces Java's
//! arithmetic has something to fail against.

use fr_geometry::{
    FloatPoint, IntBox, IntPoint, Line, Point, Polygon, Polyline, Simplex, TileShape,
};

/// Quirk #25. `Polyline.cornerCount()` (Polyline.java:178-181) is `lines.length - 1`, so an empty
/// polyline answers **-1** in Java. That flowed into `new IntPoint[cornerCount()]` in
/// `rotateApprox` (a `NegativeArraySizeException`) and into `boundingBox(0, -2)`.
#[test]
fn an_empty_polyline_has_no_corners_and_no_negative_array() {
    // `Polyline::from_lines` answers the empty polyline for any input that normalises below three
    // lines — Java's own two early returns, which is where the -1 came from.
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
    // The consumer the register names: nothing downstream can be asked for a negative-length
    // array, because the count it would have been sized from is 0.
    assert_eq!(collapsed.corners(), Vec::<Point>::new());
}

/// Quirk #24. `TileShape.rotateApprox`'s two-corner branch (TileShape.java:692-695) builds
/// `new LineSegment(currentPolyline, 0)`, but that constructor's valid range starts at 1, so it
/// stores three `null` lines and `toSimplex()` throws. About 2 % of random degenerate 2..4-line
/// shapes reach it.
///
/// The port answers `Simplex::EMPTY`, matching the **zero**-corner arm immediately below it
/// rather than the register's suggested "pass 1 instead of 0": a two-corner polyline bounds no
/// area, so a real segment built from it would be a shape with no inside.
#[test]
fn the_two_corner_branch_answers_an_empty_simplex() {
    let angle = std::f64::consts::FRAC_PI_4;
    let pole = FloatPoint::ZERO;

    // A witness found by the same kind of random sweep the register measured its ~2 % with: four
    // lines over the integer pool -4..=4, whose corners rotated by 45 degrees and rounded make a
    // polygon with **exactly two** corners. That is the branch, and reaching it is not something a
    // hand-picked box does — a two-line shape has only one corner (its two lines meet once), so
    // the branch needs three or more border lines that collapse to two on rounding.
    let shape = TileShape::Simplex(Simplex::from_lines(vec![
        Line::new(IntPoint::new(4, 0), IntPoint::new(0, 2)),
        Line::new(IntPoint::new(4, 0), IntPoint::new(0, 3)),
        Line::new(IntPoint::new(4, -1), IntPoint::new(3, -2)),
        Line::new(IntPoint::new(0, 4), IntPoint::new(2, 2)),
    ]));

    // Prove the branch is the one taken, by rebuilding `rotate_approx`'s own corner polygon
    // (TileShape.java:686-691) rather than trusting the answer to imply the path — the
    // zero-corner arm just below it returns `EMPTY` too.
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

    // Java builds `new LineSegment(currentPolyline, 0)` here, out of that constructor's
    // `1 ..= lineCount - 2` range, and `toSimplex()` throws a NullPointerException on the three
    // null lines it stored. The port answers the zero-corner arm's own value.
    assert!(
        shape.rotate_approx(angle, &pole).is_empty(),
        "the two-corner branch answers Simplex::EMPTY where Java throws"
    );

    // The angle-0 short circuit (`:678-680`) is untouched by the guard.
    assert_eq!(shape.rotate_approx(0.0, &pole), shape);

    // And a real shape still rotates to a real one — the guard did not swallow the live path.
    let box_shape = TileShape::Box(IntBox::from_coords(0, 0, 10, 10));
    assert_eq!(
        box_shape.rotate_approx(std::f64::consts::FRAC_PI_2, &pole),
        TileShape::Box(IntBox::from_coords(-10, 0, 0, 10))
    );
}

// =================================================================================================
// Quirk #22 — the one with blast radius
// =================================================================================================

/// `Polyline(Line[])`'s `removeOverlaps` (Polyline.java:133-176) decrements `newLength` to 0 and
/// then reads `tmpArr[newLength - 1]` — index **-1** — throwing an
/// `ArrayIndexOutOfBoundsException` that Java's pass-level `catch (Exception)` turns into an
/// aborted routing pass. The trailing access at `:160` already carries the guard this loop wanted.
///
/// This is the row with blast radius, because it changes what a degenerate line array
/// **normalises to** and not only whether it crashes. Measured on a 280 000-case sweep over
/// 4..8-line arrays drawn from a pool of four equal/opposite axis lines, seeded `JavaRandom(1)`:
///
/// ```text
///                 threw        -> 0 lines   -> 3 lines
///   before        20 630        177 678      81 692
///   after              0        198 308      81 692
/// ```
///
/// The change is **strictly additive**: `177 678 + 20 630 = 198 308`, and the 81 692 three-line
/// results are identical on both sides. Every input that used to throw now normalises to the
/// empty polyline; nothing that already succeeded moved. (The register quotes ~11 % from its own
/// sweep; 7.37 % is what this pool and this length distribution give. The shape of the result is
/// what matters, not the rate.)
#[test]
fn remove_overlaps_on_a_degenerate_array_normalises_to_a_literal() {
    // The register's minimal case: six lines `h, v, h, v, h, v` over the same two axes.
    let h = Line::new(IntPoint::new(0, 0), IntPoint::new(1000, 0));
    let v = Line::new(IntPoint::new(0, 0), IntPoint::new(0, 1000));

    let normalised = Polyline::from_lines(vec![h, v, h, v, h, v])
        .expect("quirk #22: this is the array that used to read tmpArr[-1] and throw");

    // The hand-computed answer. `removeOverlaps` cancels the array down to nothing: each `h` is
    // equal-or-opposite to the `h` two positions later and each `v` to its own successor, so every
    // keep is undone by the next comparison and `newLength` returns to 0. Fewer than three lines
    // survive, which is Java's own "empty polyline" exit at Polyline.java:96.
    assert_eq!(normalised.lines().len(), 0);
    assert_eq!(normalised.corner_count(), 0);
    assert!(normalised.is_empty());

    // The distinction the register turns on: `PolylineTrace.combineAtEnd` (PolylineTrace.java:
    // 303-311) tests `joinedPolyline.lines.length != newLineCount` right after the constructor, so
    // a zero-line polyline and a thrown exception were *different* observable outcomes — the first
    // is a refused combine, the second aborted the pass. Only the first is reachable now.
    assert!(
        Polyline::from_lines(vec![h, v, h, v, h, v]).is_ok(),
        "the pass-level catch has nothing left to catch here"
    );

    // Non-degenerate arrays are untouched — the guard only fires where Java read index -1.
    let corner = Polyline::from_points(&[
        Point::Int(IntPoint::new(0, 0)),
        Point::Int(IntPoint::new(1000, 0)),
        Point::Int(IntPoint::new(1000, 1000)),
    ]);
    assert_eq!(corner.corner_count(), 3);
}
