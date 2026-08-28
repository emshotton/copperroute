//! Cross-representation consistency tests (Plan 1, Task 19).
//!
//! These are cheap property-style checks over deterministic pseudo-random inputs (a tiny LCG —
//! no `rand` dependency) that catch the class of bug a behavioral port is most prone to: two code
//! paths that Java keeps in agreement diverging in the port.

use fr_geometry::prelude::*;

struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn coord(&mut self, range: i32) -> i32 {
        (self.next() % (2 * range as u64 + 1)) as i32 - range
    }
    fn point(&mut self, range: i32) -> IntPoint {
        IntPoint::new(self.coord(range), self.coord(range))
    }
}

#[test]
fn line_intersection_exact_matches_approx_when_integral() {
    let mut rng = Lcg(1);
    for _ in 0..2000 {
        let l1 = Line::new(rng.point(1000), rng.point(1000));
        let l2 = Line::new(rng.point(1000), rng.point(1000));
        if l1.a == l1.b || l2.a == l2.b || l1.is_parallel(&l2) {
            continue;
        }
        let exact = l1.intersection(&l2);
        let approx = l1.intersection_approx(&l2);
        let f = exact.to_float();
        assert!(
            (f.x - approx.x).abs() < 1e-6 && (f.y - approx.y).abs() < 1e-6,
            "{l1:?} {l2:?}"
        );
        // the exact point lies on both lines
        assert_eq!(l1.side_of(&exact), Side::Collinear);
        assert_eq!(l2.side_of(&exact), Side::Collinear);
    }
}

#[test]
fn box_octagon_simplex_agree_on_containment_and_intersection() {
    let mut rng = Lcg(2);
    for _ in 0..500 {
        let (a, b) = (rng.point(500), rng.point(500));
        let bx = IntBox::new(
            IntPoint::new(a.x.min(b.x), a.y.min(b.y)),
            IntPoint::new(a.x.max(b.x), a.y.max(b.y)),
        );
        let oct = bx.to_int_octagon();
        let sx = bx.to_simplex();
        for _ in 0..20 {
            let p = Point::Int(rng.point(600));
            let t_box = TileShape::Box(bx);
            let t_oct = TileShape::Octagon(oct);
            let t_sx = TileShape::Simplex(sx.clone());
            assert_eq!(t_box.contains(&p), t_oct.contains(&p), "{bx:?} {p:?}");
            assert_eq!(t_box.contains(&p), t_sx.contains(&p), "{bx:?} {p:?}");
        }
        let (c, d) = (rng.point(500), rng.point(500));
        let other = IntBox::new(
            IntPoint::new(c.x.min(d.x), c.y.min(d.y)),
            IntPoint::new(c.x.max(d.x), c.y.max(d.y)),
        );
        let i1 = bx.intersection(&other);
        let i2 = oct.intersection_box(&other);
        let i3 = sx.intersection_box(&other);
        assert_eq!(i1.is_empty(), i2.is_empty());
        assert_eq!(i1.is_empty(), i3.is_empty());
        if !i1.is_empty() {
            assert_eq!(i2.bounding_box(), i1);
            assert_eq!(i3.bounding_box(), i1);
        }
    }
}

#[test]
fn polyline_offset_shapes_contain_their_segments() {
    let mut rng = Lcg(3);
    for _ in 0..200 {
        let n = 2 + (rng.next() % 5) as usize;
        let pts: Vec<Point> = (0..n).map(|_| Point::Int(rng.point(2000))).collect();
        let pl = Polyline::from_points(&pts);
        // Polygon::new (which from_points funnels through) removes duplicate consecutive points
        // and collinear middle points (docs/java-quirks.md row 25 covers the resulting empty
        // polyline reporting corner_count() == 0, vs. Java's -1); either collapse leaves fewer
        // than 2 corners, which this guard skips exactly as the brief specifies.
        if pl.corner_count() < 2 {
            continue;
        }
        let hw = 1 + (rng.next() % 50) as i32;
        let shapes = pl.offset_shapes(hw);
        assert_eq!(shapes.len(), pl.corner_count() - 1);
        for (i, s) in shapes.iter().enumerate() {
            let start = pl.corner(i).expect("corner_count() >= 2 guarantees Some");
            let end = pl
                .corner(i + 1)
                .expect("corner_count() >= 2 guarantees Some");
            assert!(
                s.contains(&start),
                "segment {i} start not inside its offset shape"
            );
            assert!(
                s.contains(&end),
                "segment {i} end not inside its offset shape"
            );
        }
    }
}

#[test]
fn rational_and_int_points_compare_consistently() {
    let mut rng = Lcg(4);
    for _ in 0..2000 {
        let a = rng.point(CRIT_INT);
        let b = rng.point(CRIT_INT);
        let pa = Point::Int(a);
        let pb = Point::Int(b);
        let ra = Point::Rational(RationalPoint::from_int(&a));
        assert_eq!(pa.compare_xy(&pb), ra.compare_xy(&pb));
        // Not `assert_eq!(pa, ra)`: Java's `IntPoint.equals`/`RationalPoint.equals` both open with
        // `getClass() != other.getClass()` (verified against
        // freerouting/src/main/java/.../IntPoint.java:40 and RationalPoint.java:80), so an
        // `IntPoint` is never `equals` to a `RationalPoint` even when they denote the same point;
        // `point.rs`'s `PartialEq for Point` deliberately reproduces that (its doc comment notes
        // `Direction.getInstance` relies on the relation). `compare_xy`, which *is* a value-level
        // comparison in both Java and this port, is the right way to assert the two
        // representations agree on the point they denote.
        assert_eq!(pa.compare_xy(&ra), std::cmp::Ordering::Equal);
        // Same reasoning applies to the `Vector` that `difference_by` returns: `pa.difference_by`
        // (Int, Int) is a `Vector::Int`, `ra.difference_by` (Rational, Int) is a
        // `Vector::Rational` (see `point.rs::difference_by`'s match arms), and Java's
        // `IntVector.equals`/`RationalVector.equals` carry the identical `getClass()` guard
        // (verified against IntVector.java:34), so `==` is never true across the two. Compare the
        // values they denote via `to_float()` instead.
        let fa = pa.difference_by(&pb).to_float();
        let fb = ra.difference_by(&pb).to_float();
        assert_eq!(fa.x, fb.x);
        assert_eq!(fa.y, fb.y);
    }
}
