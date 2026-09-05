//! Plan 9 Task 11, the **#26 tail** — one register row, eight sites, each a small wrong answer in
//! a place nothing looked.
//!
//! Seven are fixed here and each has its own named inversion below. The eighth, **#11**, is not:
//! see [`the_cutout_merge_branches_are_still_dead`] at the bottom, which measures what it does and
//! records why completing it would have been a guess rather than a fix.

use fr_geometry::{
    Circle, FloatPoint, IntBox, IntOctagon, IntPoint, Point, PolygonShape, Polyline, TileShape,
    Vector,
};

// =================================================================================================
// #26 — `PolygonShape::area()` always returned 0
// =================================================================================================

/// **fixed: T11 (#26).** The guard was `if dimension() <= 2 { return 0 }` and
/// `PolygonShape::dimension()` never exceeds 2, so the shoelace sum below it was dead code and
/// `area()` answered `0` for every polygon. It is reached from `DsnFile.java:70,89` through
/// `Shape.area()`, so the plane autoroute settings derived from a board area were derived from
/// zero.
///
/// The two expectations are the answer key's, hand-computed by shoelace and then checked a second
/// way against the obvious decomposition — which is the point of choosing these two shapes.
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

/// The guard the register's column asks for alongside the `<= 2` change: `corners[len - 2]` would
/// underflow for a **1-corner** polygon, which no longer returns early now that `dimension() == 0`
/// passes the `< 2` test. A shape with fewer than three corners encloses no area.
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

/// **fixed: T11 (#23).** `Polyline.java:69` recomputed the end closing direction as
/// `fromCorner -> toCorner`, a verbatim repeat of `:66`, where `Polyline(Polygon)` uses
/// `last -> second-last`. The two closing lines came out **parallel and co-directed**, so the
/// polyline's two ends were handed differently — and the rest of the code assumes the shape lies
/// on the right of every line.
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

/// **fixed: T11 (#17), and the decision recorded.** The octagon's override accepted border points;
/// the generic `TileShape::contains_float` the box and the simplex take rejects them. The same
/// point on the same border therefore answered differently depending on which representation held
/// the shape — measured at all three border points of the answer key's octagon.
///
/// **The decision: exclusive, applied to the octagon.** The alternative — making the box and the
/// simplex inclusive — would have meant changing `contains_float_tol`, the shared primitive the
/// router calls with a tolerance at many sites, on the strength of a convention question. The
/// octagon's inclusive answer is the *override*, the odd one out, and aligning the odd one to the
/// majority is the smallest change that satisfies "the two representations must not disagree with
/// each other". The integer `TileShape::contains(&Point)` keeps its own, deliberately inclusive
/// contract (`!is_outside`); nothing here asks to change it, and it is what a caller who wants
/// border-inclusive containment should be asking.
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

/// **fixed: T11 (#18).** The covariant override grids the box directly and kept every cell,
/// including the degenerate ones the base algorithm filters out (TileShape.java:908-913). They
/// arise whenever `sectionLength · (count − 1) == length`: a `6 x 6` box at
/// `max_section_width = 1.6` has `xcount = ycount = 4` but `sectionLength = 2`, so the fourth row
/// and the fourth column are zero-width — **16 raw sections where 9 have area**, the register's
/// own example.
///
/// The invariant is that dividing never loses or invents area, and never emits a section that is
/// not a section.
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

/// The healthy case is untouched — the same arithmetic Task 8's #159 depends on.
#[test]
fn a_healthy_box_still_divides_the_way_it_did() {
    let b = IntBox::from_coords(-10_000, -10_000, 10_000, 10_000);
    let sections = b.divide_into_sections(10_000.0);
    assert_eq!(sections.len(), 4);
    assert_eq!(sections.iter().map(IntBox::area).sum::<f64>(), b.area());
}

/// A **degenerate** box answers 0 sections, and did before the fix too.
///
/// The answer key expects `1` here, on the stated ground that "the base class filters
/// `dimension() != 2` and returns the shape itself". Checked against the Java source, that is not
/// what the base class does: `TileShape.divideIntoSections:900-905`'s guard is `isEmpty()`, and
/// its `dimension() == 2` filter at `:910` would drop a degenerate shape's only section as well.
/// So the base answers 0 for this input too, and the override's 0 — which comes from
/// `ycount = ceil(0 / 30) = 0`, so the grid loop never runs — is the same answer for a different
/// reason. Area is conserved either way, because a degenerate box has none.
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

/// **fixed: T11 (#32), and the decision recorded: translate or fail, like the siblings.**
///
/// Java warned and returned the **untranslated** circle for a `RationalVector`, where `Line`,
/// `IntBox` and `Polyline` all throw. Silently returning the wrong shape is the one answer no
/// caller can detect.
///
/// Implementing rational translation was not available: a `Circle`'s centre is an `IntPoint`, so a
/// rational translation has no representable result in general — which is why Java gave up. The
/// remaining honest answers were a hard error or a documented rounding, and rounding invents a
/// shape the caller did not ask for. So it is an error.
#[test]
#[should_panic(expected = "not implemented for a RationalVector")]
fn a_rational_translation_of_a_circle_fails_rather_than_lying() {
    let c = Circle::new(IntPoint::new(10, 20), 5);
    let rational = Vector::Rational(fr_geometry::RationalVector::new(
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

/// **#11 is deliberately not fixed in Task 11**, and this test records the state it is left in so
/// the decision is measured rather than asserted.
///
/// `Simplex.cutoutFrom`'s `prevDivisionLine` is never assigned — the only statement that could
/// assign it is `:860`'s reversed `nextDivisionLine = prevDivisionLine`, which is itself dead — so
/// both `mergePrevDivisionLine` branches are unreachable and the method always returns the maximal
/// division. The register offers two remedies, "delete the dead code **or** finish the intended
/// merge logic", and the second is not recoverable from the source: the literal transposition
/// carries a self-referential value, and the semantically coherent candidate
/// (`prevDivisionLine = lastCurrDivisionLine`) is a guess about intent.
///
/// That candidate was implemented and measured. It changes the convex decomposition and through it
/// the maze: `every_drill_of_the_page_whose_room_matches_is_expanded_once` moved 53 -> 50
/// expansions, and `the_reversed_search_with_vias_splits_into_three_traces` routed a materially
/// different connection. The new route may be better; there is no oracle that says so, and a guess
/// that moves the router is not a fix. See `simplex.rs`'s comment at the site for the full
/// argument.
///
/// The observable the answer key proposes — corner-cut and middle-cut piece counts must *differ*
/// once the merge works — is the reachability statement this pins in its current, un-merged form.
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
