//! Port of `app.freerouting.geometry.planar.Line`: implements functionality for lines in the
//! plane.
//!
//! Java declares the two end points as the abstract `Point`, but every arithmetic method casts
//! them straight back to `IntPoint` (`intersection`, `intersectionApprox`, `sideOf(FloatPoint)`,
//! `signedDistance`, `translate`, `compareTo`, `length`) and the two-point constructor only
//! *warns* when handed anything else. This port therefore fixes the end points at `IntPoint`,
//! which turns Java's warning into a compile-time guarantee.

use std::cmp::Ordering;

use num_bigint::BigInt;
use num_integer::Integer;
use num_traits::{One, Signed, ToPrimitive, Zero};

use crate::direction::Direction;
use crate::float_point::FloatPoint;
use crate::int_direction::IntDirection;
use crate::int_point::IntPoint;
use crate::int_vector::IntVector;
use crate::limits::{CRIT_INT, java_round};
use crate::point::Point;
use crate::rational_point::RationalPoint;
use crate::rational_vector::big_to_f64;
use crate::side::Side;
use crate::signum::Signum;
use crate::vector::Vector;

/// A directed line in the plane, defined by two `IntPoint`s.
///
/// **Equality note.** Java overrides `Line.equals` with a *geometric* test (collinear end points
/// plus the same direction sense) but does **not** override `hashCode`, so Java's own
/// `equals`/`hashCode` contract is broken and `Line`s in hash containers behave by identity. This
/// port derives structural `PartialEq`/`Eq`/`Hash` on the two end points instead, which is a
/// lawful pair. Java's geometric test is `overlaps` plus a direction check; it is available as
/// [`Line::equals_geometric`], next to [`Line::fast_equals`] and [`Line::get_id`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Line {
    /// The first point defining this line.
    pub a: IntPoint,
    /// The second point defining this line; the line points from `a` towards `b`.
    pub b: IntPoint,
}

impl Line {
    /// Creates a directed Line from two points.
    pub fn new(a: IntPoint, b: IntPoint) -> Line {
        Line { a, b }
    }

    /// Creates a directed Line from four integer coordinates.
    pub fn from_coords(ax: i32, ay: i32, bx: i32, by: i32) -> Line {
        Line {
            a: IntPoint::new(ax, ay),
            b: IntPoint::new(bx, by),
        }
    }

    /// Creates a directed Line from a point and a direction (`b = a + dir.get_vector()`).
    ///
    /// Java has both `Line(Point a, Direction dir)` (which additionally caches `dir`) and the
    /// static `Line.getInstance(Point a, Direction dir)`; with the direction recomputed rather
    /// than cached, the two are the same function.
    pub fn from_direction(a: IntPoint, dir: &IntDirection) -> Line {
        Line::new(a, a.translate_by(&dir.get_vector()))
    }

    /// Creates a directed Line from a point and a `Direction` of either representation. Returns
    /// `None` for a `Direction::Big`: Java's `a.translateBy(dir.getVector())` would yield a
    /// `RationalPoint` there, which this port's `Line` cannot hold (Java only logs a warning and
    /// then breaks in every arithmetic method).
    pub fn from_direction_any(a: IntPoint, dir: &Direction) -> Option<Line> {
        match dir {
            Direction::Int(d) => Some(Line::from_direction(a, d)),
            Direction::Big(_) => None,
        }
    }

    /// Gets the direction of this directed line. (Java caches the value in a transient field;
    /// this port recomputes it, which is the same function of `a` and `b`.)
    pub fn direction(&self) -> IntDirection {
        let d = self.b.difference_by(&self.a);
        d.to_normalized_direction()
    }

    /// The function returns `Side::OnTheLeft`, if this Line is on the left of point,
    /// `Side::OnTheRight`, if this Line is on the right of point, and `Side::Collinear`, if this
    /// Line contains point.
    pub fn side_of(&self, point: &Point) -> Side {
        point.side_of_line(self).negate()
    }

    /// The `IntPoint` fast path of [`Line::side_of`] (Java reaches it through the virtual
    /// `Point.sideOf(Line)` dispatch).
    fn side_of_int_point(&self, point: &IntPoint) -> Side {
        point.side_of_line(self).negate()
    }

    /// Returns `Side::Collinear`, if point is on the line within `tolerance`. Otherwise
    /// `Side::OnTheLeft`, if this line is on the left of point, or `Side::OnTheRight`, if this
    /// line is on the right of point.
    ///
    /// Java computes the determinant in `double` ("only implemented for IntPoint lines for
    /// performance reasons") and one operand is a `FloatPoint`, so this stays `f64`.
    pub fn side_of_float(&self, point: &FloatPoint, tolerance: f64) -> Side {
        let det = (self.b.y - self.a.y) as f64 * (point.x - self.a.x as f64)
            - (self.b.x - self.a.x) as f64 * (point.y - self.a.y as f64);
        if det - tolerance > 0.0 {
            Side::OnTheLeft
        } else if det + tolerance < 0.0 {
            Side::OnTheRight
        } else {
            Side::Collinear
        }
    }

    /// Java `sideOf(FloatPoint point)`: `sideOf(point, 0)`.
    pub fn side_of_float_exact(&self, point: &FloatPoint) -> Side {
        self.side_of_float(point, 0.0)
    }

    /// Returns `Side::OnTheLeft`, if this line is on the left of the intersection of `p1` and
    /// `p2`, `Side::OnTheRight`, if this line is on the right of the intersection, and
    /// `Side::Collinear`, if all 3 lines intersect in exactly 1 point.
    pub fn side_of_intersection(&self, p1: &Line, p2: &Line) -> Side {
        let intersection_approx = p1.intersection_approx(p2);
        let result = self.side_of_float(&intersection_approx, 1.0);
        if result == Side::Collinear {
            // Previous calculation was with FloatPoints and a tolerance for performance reasons.
            // Make an exact check for collinearity now with class Point instead of FloatPoint.
            let intersection = p1.intersection(p2);
            return self.side_of(&intersection);
        }
        result
    }

    /// Returns the signed distance of this line from point. The result will be positive, if the
    /// line is on the left of point, else negative.
    pub fn signed_distance(&self, point: &FloatPoint) -> f64 {
        let dx = (self.b.x - self.a.x) as f64;
        let dy = (self.b.y - self.a.y) as f64;
        let det = dy * (point.x - self.a.x as f64) - dx * (point.y - self.a.y as f64);
        // area of the parallelogramm spanned by the 3 points
        let length = (dx * dx + dy * dy).sqrt();
        det / length
    }

    /// Returns true if the two lines define the same set of points, but may have opposite
    /// directions.
    pub fn overlaps(&self, other: &Line) -> bool {
        self.side_of_int_point(&other.a) == Side::Collinear
            && self.side_of_int_point(&other.b) == Side::Collinear
    }

    /// Returns the line defining the same set of points, but with opposite direction.
    pub fn opposite(&self) -> Line {
        Line::new(self.b, self.a)
    }

    /// Returns the intersection point of the 2 lines. If the lines are parallel,
    /// `result.is_infinite()` will be true.
    ///
    /// Ported verbatim, including the orthogonal/45-degree fast paths and the `BigInt` general
    /// case. Note that the general case ends in `RationalPoint::new(is_x, is_y, det)` directly —
    /// it does **not** go through `Point::from_big` (Java `Point.getInstance`), so the result can
    /// be a `Point::Rational` with `z == 1` when the exact intersection exceeds `CRIT_INT`.
    pub fn intersection(&self, other: &Line) -> Point {
        // this function is at the moment only implemented for lines consisting of IntPoints.
        // The general implementation is still missing.
        let delta1 = self.b.difference_by(&self.a);
        let delta2 = other.b.difference_by(&other.a);
        // Separate handling for orthogonal and 45 degree lines for better performance
        if delta1.x == 0 {
            // this line is vertical
            if delta2.y == 0 {
                // other line is horizontal
                return Point::Int(IntPoint::new(self.a.x, other.a.y));
            }
            if delta2.x == delta2.y {
                // other line is right diagonal
                let this_x = self.a.x;
                return Point::Int(IntPoint::new(this_x, other.a.y + this_x - other.a.x));
            }
            if delta2.x == -delta2.y {
                // other line is left diagonal
                let this_x = self.a.x;
                return Point::Int(IntPoint::new(this_x, other.a.y + other.a.x - this_x));
            }
        } else if delta1.y == 0 {
            // this line is horizontal
            if delta2.x == 0 {
                // other line is vertical
                return Point::Int(IntPoint::new(other.a.x, self.a.y));
            }
            if delta2.x == delta2.y {
                // other line is right diagonal
                let this_y_coordinate = self.a.y;
                return Point::Int(IntPoint::new(
                    other.a.x + this_y_coordinate - other.a.y,
                    this_y_coordinate,
                ));
            }
            if delta2.x == -delta2.y {
                // other line is left diagonal
                let this_y_coordinate = self.a.y;
                return Point::Int(IntPoint::new(
                    other.a.x + other.a.y - this_y_coordinate,
                    this_y_coordinate,
                ));
            }
        } else if delta1.x == delta1.y {
            // this line is right diagonal
            if delta2.x == 0 {
                // other line is vertical
                let other_x = other.a.x;
                return Point::Int(IntPoint::new(other_x, self.a.y + other_x - self.a.x));
            }
            if delta2.y == 0 {
                // other line is horizontal
                let other_y = other.a.y;
                return Point::Int(IntPoint::new(self.a.x + other_y - self.a.y, other_y));
            }
        } else if delta1.x == -delta1.y {
            // this line is left diagonal
            if delta2.x == 0 {
                // other line is vertical
                let other_x = other.a.x;
                return Point::Int(IntPoint::new(other_x, self.a.y + self.a.x - other_x));
            }
            if delta2.y == 0 {
                // other line is horizontal
                let other_y = other.a.y;
                return Point::Int(IntPoint::new(self.a.x + self.a.y - other_y, other_y));
            }
        }

        let det1 = BigInt::from(self.a.determinant(&self.b));
        let det2 = BigInt::from(other.a.determinant(&other.b));
        let mut det = BigInt::from(delta2.determinant(&delta1));
        let tmp1 = &det1 * BigInt::from(delta2.x);
        let tmp2 = &det2 * BigInt::from(delta1.x);
        let mut is_x = tmp1 - tmp2;
        let tmp1 = &det1 * BigInt::from(delta2.y);
        let tmp2 = &det2 * BigInt::from(delta1.y);
        let mut is_y = tmp1 - tmp2;
        if !det.is_zero() {
            if det.is_negative() {
                det = -det;
                is_x = -is_x;
                is_y = -is_y;
            }
            // Java `BigInteger.mod` is the non-negative remainder; `det > 0` here, so `mod_floor`
            // is the same function.
            if is_x.mod_floor(&det).is_zero() && is_y.mod_floor(&det).is_zero() {
                is_x /= &det;
                is_y /= &det;
                if big_to_f64(&is_x).abs() <= CRIT_INT as f64
                    && big_to_f64(&is_y).abs() <= CRIT_INT as f64
                {
                    return Point::Int(IntPoint::new(
                        is_x.to_i32().expect("|is_x| <= CRIT_INT"),
                        is_y.to_i32().expect("|is_y| <= CRIT_INT"),
                    ));
                }
                det = BigInt::one();
            }
        }
        Point::Rational(RationalPoint::new(is_x, is_y, det))
    }

    /// Returns an approximation of the intersection of the 2 lines by a FloatPoint. If the lines
    /// are parallel the result coordinates will be `Integer.MAX_VALUE` (Java's sentinel, kept
    /// verbatim rather than turned into an `Option`). Useful in situations where performance is
    /// more important than accuracy.
    pub fn intersection_approx(&self, other: &Line) -> FloatPoint {
        // this function is at the moment only implemented for lines consisting of IntPoints.
        // The general implementation is still missing.
        let d1x = (self.b.x - self.a.x) as f64;
        let d1y = (self.b.y - self.a.y) as f64;
        let d2x = (other.b.x - other.a.x) as f64;
        let d2y = (other.b.y - other.a.y) as f64;
        let det1 = self.a.x as f64 * self.b.y as f64 - self.a.y as f64 * self.b.x as f64;
        let det2 = other.a.x as f64 * other.b.y as f64 - other.a.y as f64 * other.b.x as f64;
        let det = d2x * d1y - d2y * d1x;
        let (is_x, is_y) = if det == 0.0 {
            (i32::MAX as f64, i32::MAX as f64)
        } else {
            (
                (d2x * det1 - d1x * det2) / det,
                (d2y * det1 - d1y * det2) / det,
            )
        };
        FloatPoint::new(is_x, is_y)
    }

    /// Returns the perpendicular projection of point onto this line.
    pub fn perpendicular_projection(&self, point: &Point) -> Point {
        point.perpendicular_projection(self)
    }

    /// Translates the line perpendicular by dist. If `dist > 0`, the line is translated to the
    /// left; otherwise, it is translated to the right.
    pub fn translate(&self, dist: f64) -> Line {
        // this function is at the moment only implemented for lines consisting of IntPoints.
        // The general implementation is still missing.
        let ai = self.a;
        let direction = self.direction();
        let v = direction.get_vector();
        let vxvx = v.x as f64 * v.x as f64;
        let vyvy = v.y as f64 * v.y as f64;
        let length = (vxvx + vyvy).sqrt();
        let new_a = if vxvx <= vyvy {
            // translate along the x axis
            let rel_x = java_round((dist * length) / v.y as f64) as i32;
            IntPoint::new(ai.x - rel_x, ai.y)
        } else {
            // translate along the  y axis
            let rel_y = java_round((dist * length) / v.x as f64) as i32;
            IntPoint::new(ai.x, ai.y + rel_y)
        };
        Line::from_direction(new_a, &direction)
    }

    /// Translates the line by vector.
    ///
    /// Java takes the abstract `Vector`; a `RationalVector` would translate the end points into
    /// `RationalPoint`s, which this port's `Line` cannot hold — see [`Line::translate_by_any`].
    pub fn translate_by(&self, vector: &IntVector) -> Line {
        if *vector == IntVector::ZERO {
            return *self;
        }
        Line::new(self.a.translate_by(vector), self.b.translate_by(vector))
    }

    /// [`Line::translate_by`] for a `Vector` of either representation; `None` for
    /// `Vector::Rational` (see [`Line::from_direction_any`] for the same reasoning).
    pub fn translate_by_any(&self, vector: &Vector) -> Option<Line> {
        match vector {
            Vector::Int(v) => Some(self.translate_by(v)),
            Vector::Rational(_) => None,
        }
    }

    /// Returns true if the line is axis-parallel.
    pub fn is_orthogonal(&self) -> bool {
        self.direction().is_orthogonal()
    }

    /// Returns true if this line is diagonal.
    pub fn is_diagonal(&self) -> bool {
        self.direction().is_diagonal()
    }

    /// Returns true if the direction of this line is a multiple of 45 degrees.
    pub fn is_multiple_of_45_degree(&self) -> bool {
        self.direction().is_multiple_of_45_degree()
    }

    /// Checks if this line and other are parallel.
    pub fn is_parallel(&self, other: &Line) -> bool {
        self.direction().side_of(&other.direction()) == Side::Collinear
    }

    /// Checks if this line and other are perpendicular.
    pub fn is_perpendicular(&self, other: &Line) -> bool {
        let v1 = self.direction().get_vector();
        let v2 = other.direction().get_vector();
        v1.projection(&v2) == Signum::Zero
    }

    /// Returns true if this and other define the same line. (Java's `isEqualOrOpposite` has the
    /// same body as `overlaps`.)
    pub fn is_equal_or_opposite(&self, other: &Line) -> bool {
        self.side_of_int_point(&other.a) == Side::Collinear
            && self.side_of_int_point(&other.b) == Side::Collinear
    }

    /// Calculates the cosine of the angle between this line and other.
    pub fn cos_angle(&self, other: &Line) -> f64 {
        let v1 = self.b.difference_by(&self.a);
        let v2 = other.b.difference_by(&other.a);
        v1.cos_angle(&v2)
    }

    /// A line l_1 is defined bigger than a line l_2, if the direction of l_1 is bigger than the
    /// direction of l_2. Java implements `Comparable<Line>` with this; it is the ordering used by
    /// `Simplex.removeRedundantLines`.
    ///
    /// Java's body is a literal, inlined copy of the package-private
    /// `IntDirection.compareTo(IntDirection)` algorithm applied to the raw end-point deltas
    /// (Line.java:426-473 against IntDirection.java) — the branch cascade and the closing
    /// `dx2 * dy1 - dy2 * dx1` determinant agree term by term. That direct algorithm is reached
    /// here as `IntDirection::compare_to`, whose public form is
    /// `compare_to(x, y) = compare_direct(y, x).reverse()`; so the direct call with
    /// `(receiver, param) = (d1, d2)` is `d2.compare_to(d1).reverse()`.
    ///
    /// Normalising the deltas would not change the answer (dividing both coordinates by a
    /// positive gcd preserves every sign test and the determinant's sign), so the raw deltas are
    /// wrapped in an `IntDirection` as-is, exactly as Java uses them.
    ///
    /// **No `Ord`/`PartialOrd` impl**, for two independent reasons — see the tests:
    /// * it is not antisymmetric for a degenerate line (`a == b`, zero direction): the zero
    ///   direction compares `Greater` than `RIGHT`, while `RIGHT` compares `Equal` to it. This is
    ///   the same hole as `IntDirection::compare_to` at `NULL` (Task 6).
    /// * it returns `Ordering::Equal` for any two lines with the same direction, which the
    ///   derived structural `PartialEq` reports as unequal — `Ord` requires the two to agree.
    ///
    /// Sort with `slice::sort_by(|a, b| a.compare_to(b))`, exactly as Java sorts `Line`s.
    pub fn compare_to(&self, other: &Line) -> Ordering {
        let d1 = self.b.difference_by(&self.a);
        let d2 = other.b.difference_by(&other.a);
        IntDirection::new(d2.x, d2.y)
            .compare_to(&IntDirection::new(d1.x, d1.y))
            .reverse()
    }

    /// Calculates an approximation of the function value of this line at x, if the line is not
    /// vertical.
    ///
    /// Java logs "function_value_approx: line is vertical" and returns 0 for a vertical line;
    /// the return value is kept, the log is dropped (`fr-geometry` has no logger).
    pub fn function_value_approx(&self, x: f64) -> f64 {
        let p1 = self.a.to_float();
        let p2 = self.b.to_float();
        let dx = p2.x - p1.x;
        if dx == 0.0 {
            return 0.0;
        }
        let dy = p2.y - p1.y;
        let det = p1.x * p2.y - p2.x * p1.y;
        (dy * x - det) / dx
    }

    /// Calculates an approximation of the function value in y of this line at y, if the line is
    /// not horizontal. Java logs and returns 0 for a horizontal line (see
    /// [`Line::function_value_approx`]).
    pub fn function_in_y_value_approx(&self, y: f64) -> f64 {
        let p1 = self.a.to_float();
        let p2 = self.b.to_float();
        let dy = p2.y - p1.y;
        if dy == 0.0 {
            return 0.0;
        }
        let dx = p2.x - p1.x;
        let det = p1.x * p2.y - p2.x * p1.y;
        (dx * y + det) / dy
    }

    /// Calculates the direction from `from_point` to the nearest point on this line to
    /// `from_point`. Returns `None` (Java: `null`), if `from_point` is contained in this line.
    pub fn perpendicular_direction(&self, from_point: &Point) -> Option<Direction> {
        let line_side = self.side_of(from_point);
        if line_side == Side::Collinear {
            return None;
        }
        let dir1 = self.direction().turn_45_degree(2);
        let dir2 = self.direction().turn_45_degree(6);

        let check_point1 = from_point.translate_by(&Vector::Int(dir1.get_vector()));
        if self.side_of(&check_point1) != line_side {
            return Some(Direction::Int(dir1));
        }
        let check_point2 = from_point.translate_by(&Vector::Int(dir2.get_vector()));
        if self.side_of(&check_point2) != line_side {
            return Some(Direction::Int(dir2));
        }
        let nearest_line_point = from_point.to_float().projection_approx(self);
        if nearest_line_point.distance_square(&check_point1.to_float())
            <= nearest_line_point.distance_square(&check_point2.to_float())
        {
            Some(Direction::Int(dir1))
        } else {
            Some(Direction::Int(dir2))
        }
    }

    /// Turns this line by factor times 90 degree around pole.
    pub fn turn_90_degree(&self, factor: i32, pole: &IntPoint) -> Line {
        Line::new(
            self.a.turn_90_degree(factor, pole),
            self.b.turn_90_degree(factor, pole),
        )
    }

    /// Mirrors this line at the vertical line through pole. (Java swaps the end points, so the
    /// mirrored line keeps a consistent orientation.)
    pub fn mirror_vertical(&self, pole: &IntPoint) -> Line {
        Line::new(self.b.mirror_vertical(pole), self.a.mirror_vertical(pole))
    }

    /// Mirrors this line at the horizontal line through pole. (Java swaps the end points; see
    /// [`Line::mirror_vertical`].)
    pub fn mirror_horizontal(&self, pole: &IntPoint) -> Line {
        Line::new(
            self.b.mirror_horizontal(pole),
            self.a.mirror_horizontal(pole),
        )
    }

    /// Returns the Euclidean length of this line, as a `f32` (Java returns `float`).
    ///
    /// Java computes the sum of squares in `int` arithmetic before widening it for `Math.sqrt`,
    /// so it silently wraps once a coordinate difference exceeds 2^15.5 or so; `wrapping_*`
    /// reproduces that instead of panicking in debug builds.
    pub fn length(&self) -> f32 {
        let dx = self.b.x.wrapping_sub(self.a.x);
        let dy = self.b.y.wrapping_sub(self.a.y);
        let sum = dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy));
        (sum as f64).sqrt() as f32
    }

    /// Java's geometric `Line.equals(Object)` (Line.java:57-79): two lines are equal when each
    /// end point of `other` is collinear with this line **and** the two direction vectors point
    /// the same way. Named `equals_geometric` because the derived `PartialEq`/`Eq`/`Hash` on this
    /// type is the structural end-point comparison (see the type-level note above); Java's own
    /// `equals`/`hashCode` pair is inconsistent, so the two tests are kept apart here.
    ///
    /// Java's leading `if (this == other) return true;` reference shortcut is **not** ported:
    /// Rust has no object identity to test here. The two agree except for a degenerate line
    /// (`a == b`, zero direction) compared with itself, where Java's shortcut returns `true` and
    /// the geometric test below returns `false` (`Signum::Zero`, not `Positive`).
    ///
    /// The only Java caller is `Simplex.borderLineIndex` (Simplex.java:666-673).
    pub fn equals_geometric(&self, other: &Line) -> bool {
        if self.side_of_int_point(&other.a) != Side::Collinear {
            return false;
        }
        if self.side_of_int_point(&other.b) != Side::Collinear {
            return false;
        }
        let dir1 = self.b.difference_by(&self.a);
        let dir2 = other.b.difference_by(&other.a);
        dir1.projection(&dir2) == Signum::Positive
    }

    /// Returns true, if this and other define the same line. Is designed for good performance,
    /// but works only for lines consisting of IntPoints (Line.java:79-97).
    ///
    /// Java computes the determinant in `double`; per the porting conventions a `double` used
    /// only for the *sign* of a product of coordinate differences becomes `i64` here.
    ///
    /// Used by `Simplex.removeRedundantLines` (Simplex.java:892) to skip duplicate lines.
    ///
    /// One deliberate divergence in the final `direction()` comparison: `IntDirection`'s
    /// `PartialEq` starts with a structural `(x, y)` shortcut, so two `NULL` (zero) directions
    /// compare *equal* here, whereas Java's `Direction.equals` on two distinct zero-direction
    /// objects returns `false` (they are collinear, but their projection is `Signum.ZERO`, not
    /// `POSITIVE`). A zero direction only arises from a degenerate line with `a == b`, so the
    /// difference is unreachable for the lines `Simplex` actually builds.
    pub fn fast_equals(&self, other: &Line) -> bool {
        let dx1 = other.a.x as i64 - self.a.x as i64;
        let dy1 = other.a.y as i64 - self.a.y as i64;
        let dx2 = self.b.x as i64 - self.a.x as i64;
        let dy2 = self.b.y as i64 - self.a.y as i64;
        let det = dx1 * dy2 - dx2 * dy1;
        if det != 0 {
            return false;
        }
        self.direction() == other.direction()
    }

    /// Returns a deterministic tie-breaking id for this line (Java `31 * a.getId() + b.getId()`,
    /// Line.java:53-55). Java `int` arithmetic wraps silently on overflow; this is a hash-shaped
    /// value, so `wrapping_*` reproduces that. Used by `Simplex.getId` (Simplex.java:76).
    pub fn get_id(&self) -> i32 {
        31i32
            .wrapping_mul(self.a.get_id())
            .wrapping_add(self.b.get_id())
    }

    // ported in Task 14: `is_on_the_left(&TileShape)` and `is_on_the_right(&TileShape)` live in
    // `tile_shape.rs`, next to the enum they take.
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Side;
    use crate::direction::Direction;
    use crate::float_point::FloatPoint;
    use crate::int_direction::IntDirection;
    use crate::int_point::IntPoint;
    use crate::point::Point;
    use num_bigint::BigInt;

    fn l(ax: i32, ay: i32, bx: i32, by: i32) -> Line {
        Line::from_coords(ax, ay, bx, by)
    }

    #[test]
    fn axis_aligned_and_diagonal_fast_paths() {
        let vertical = l(5, 0, 5, 1);
        let horizontal = l(0, 3, 1, 3);
        assert_eq!(
            vertical.intersection(&horizontal),
            Point::Int(IntPoint::new(5, 3))
        );
        assert_eq!(
            horizontal.intersection(&vertical),
            Point::Int(IntPoint::new(5, 3))
        );
        let right_diag = l(0, 0, 1, 1);
        assert_eq!(
            vertical.intersection(&right_diag),
            Point::Int(IntPoint::new(5, 5))
        );
        let left_diag = l(0, 10, 1, 9);
        assert_eq!(
            horizontal.intersection(&left_diag),
            Point::Int(IntPoint::new(7, 3))
        );
    }

    #[test]
    fn general_intersection_exact() {
        assert_eq!(
            l(0, 0, 2, 2).intersection(&l(0, 2, 2, 0)),
            Point::Int(IntPoint::new(1, 1))
        );
        // (0,0)-(2,1) with (0,1)-(2,0) meet at (1, 0.5).
        //
        // Corrected per Java: `Line.intersection` (Line.java:303) ends with
        // `return new RationalPoint(isX, isY, det);` — it does NOT route through
        // `Point.getInstance(BigInteger, BigInteger, BigInteger)`. The brief's
        // `Point::from_big(2, 1, 2)` would take Java's `Point.getInstance` path, which tests
        // divisibility on x only (Point.java:32-37) and would truncate y from 1/2 to 0, giving
        // `Point::Int(1, 0)` — a different point. Java wins: the exact triple here is
        // (isX, isY, det) = (4, 2, 4), which `RationalPoint`'s proportional equality makes equal
        // to (2, 1, 2).
        let p = l(0, 0, 2, 1).intersection(&l(0, 1, 2, 0));
        assert_eq!(
            p,
            Point::Rational(crate::rational_point::RationalPoint::new(
                BigInt::from(4),
                BigInt::from(2),
                BigInt::from(4)
            ))
        );
        assert_eq!(
            p,
            Point::Rational(crate::rational_point::RationalPoint::new(
                BigInt::from(2),
                BigInt::from(1),
                BigInt::from(2)
            ))
        );
        // parallel lines: z == 0 -> infinite point
        assert!(l(0, 0, 1, 1).intersection(&l(0, 1, 1, 2)).is_infinite());
    }

    #[test]
    fn side_of_and_direction() {
        let line = l(0, 0, 10, 0); // pointing RIGHT
        assert_eq!(line.direction(), IntDirection::RIGHT);
        let above = Point::Int(IntPoint::new(3, 4));
        let below = Point::Int(IntPoint::new(3, -4));
        // Pinned from the Task 6-7 convention (derivation in `IntVector::side_of`):
        //   IntPoint.sideOf(Line): v1 = point - line.a, v2 = line.b - line.a, result v1.sideOf(v2),
        //   and `IntVector::side_of(a, b) = Side::of(a.x*b.y - a.y*b.x).negate()`.
        //   Line.sideOf(Point) then negates once more.
        // above = (3, 4): v1 = (3, 4), v2 = (10, 0); 3*0 - 4*10 = -40 -> OnTheRight
        //   -> negate -> OnTheLeft -> Line negates -> OnTheRight.
        // (Java's documented meaning: "the line is on the right of the point" — correct for a
        // point above a rightward line.)
        assert_eq!(line.side_of(&above), Side::OnTheRight);
        // below = (3, -4): 3*0 - (-4)*10 = 40 -> OnTheLeft -> negate -> OnTheRight
        //   -> Line negates -> OnTheLeft.
        assert_eq!(line.side_of(&below), Side::OnTheLeft);
        assert_eq!(
            line.side_of(&Point::Int(IntPoint::new(99, 0))),
            Side::Collinear
        );
        assert_eq!(line.side_of(&above), line.opposite().side_of(&below));
    }

    #[test]
    fn perpendicular_projection() {
        let diag = l(0, 0, 1, 1);
        assert_eq!(
            IntPoint::new(4, 0).perpendicular_projection(&diag),
            Point::Int(IntPoint::new(2, 2))
        );
        assert_eq!(
            IntPoint::new(3, 0).perpendicular_projection(&diag),
            Point::from_big(BigInt::from(3), BigInt::from(3), BigInt::from(2))
        );
    }

    #[test]
    fn predicates() {
        assert!(l(0, 0, 0, 5).is_orthogonal());
        assert!(l(0, 0, 3, -3).is_diagonal());
        assert!(l(0, 0, 2, 2).is_parallel(&l(5, 5, 9, 9)));
        assert!(l(0, 0, 2, 2).is_perpendicular(&l(0, 0, -1, 1)));
        assert!(l(0, 0, 2, 2).is_equal_or_opposite(&l(9, 9, 1, 1)));
        assert!(l(0, 0, 2, 2).overlaps(&l(9, 9, 1, 1)));
        assert!(!l(0, 0, 2, 2).overlaps(&l(0, 1, 2, 3)));
        assert!(l(0, 0, 3, 3).is_multiple_of_45_degree());
        assert!(!l(0, 0, 3, 1).is_multiple_of_45_degree());
    }

    #[test]
    fn function_values() {
        let line = l(0, 0, 2, 4); // y = 2x
        assert_eq!(line.function_value_approx(3.0), 6.0);
        assert_eq!(line.function_in_y_value_approx(6.0), 3.0);
    }

    #[test]
    fn intersection_keeps_a_rational_point_when_the_result_exceeds_crit_int() {
        // General case, exactly divisible (det == 1), but |isX| > CRIT_INT: Java falls out of the
        // demotion guard, sets `det = BigInteger.ONE` and returns a RationalPoint with z == 1
        // (Line.java:296-303) rather than an IntPoint.
        let k = crate::CRIT_INT;
        let p = l(0, 0, 2, 1).intersection(&l(0, k, 1, k));
        assert_eq!(
            p,
            Point::Rational(crate::rational_point::RationalPoint::new(
                BigInt::from(2 * k as i64),
                BigInt::from(k),
                BigInt::from(1)
            ))
        );
        assert!(matches!(p, Point::Rational(_)));
    }

    #[test]
    fn intersection_approx_uses_the_java_sentinel_when_parallel() {
        let p = l(0, 0, 2, 1).intersection_approx(&l(0, 1, 2, 0));
        assert_eq!(p, FloatPoint::new(1.0, 0.5));
        // Java: `isX = isY = Integer.MAX_VALUE` for parallel lines (Line.java:318-320).
        let parallel = l(0, 0, 1, 1).intersection_approx(&l(0, 1, 1, 2));
        assert_eq!(parallel, FloatPoint::new(i32::MAX as f64, i32::MAX as f64));
    }

    #[test]
    fn side_of_float_and_signed_distance() {
        let line = l(0, 0, 10, 0);
        let above = FloatPoint::new(3.0, 4.0);
        // det = (b.y - a.y) * (p.x - a.x) - (b.x - a.x) * (p.y - a.y) = 0 - 10 * 4 = -40
        assert_eq!(line.side_of_float(&above, 0.0), Side::OnTheRight);
        assert_eq!(line.side_of_float_exact(&above), Side::OnTheRight);
        // a tolerance wider than |det| swallows the sign
        assert_eq!(line.side_of_float(&above, 100.0), Side::Collinear);
        assert_eq!(line.signed_distance(&above), -4.0);
    }

    #[test]
    fn side_of_intersection_falls_back_to_the_exact_check() {
        let p1 = l(0, 0, 1, 1);
        let p2 = l(0, 10, 1, 9); // p1 x p2 == (5, 5)
        // A line through the intersection: the float test is Collinear within tolerance 1.0 and
        // the exact re-check confirms it.
        assert_eq!(
            l(0, 5, 1, 5).side_of_intersection(&p1, &p2),
            Side::Collinear
        );
        // A line well above the intersection.
        assert_eq!(
            l(0, 10, 1, 10).side_of_intersection(&p1, &p2),
            Side::OnTheLeft
        );
        assert_eq!(
            l(0, 0, 1, 0).side_of_intersection(&p1, &p2),
            Side::OnTheRight
        );
    }

    #[test]
    fn translate_and_translate_by() {
        let line = l(0, 0, 10, 0);
        // direction RIGHT -> v = (1, 0); vxvx = 1 > vyvy = 0, so the y-axis branch runs:
        // relY = round(5 * 1 / 1) = 5, newA = (0, 5), and `Line.getInstance(newA, RIGHT)`.
        assert_eq!(
            line.translate(5.0),
            Line::new(IntPoint::new(0, 5), IntPoint::new(1, 5))
        );
        assert_eq!(
            line.translate_by(&crate::IntVector::new(1, 2)),
            Line::new(IntPoint::new(1, 2), IntPoint::new(11, 2))
        );
        // Java returns `this` unchanged for the zero vector.
        assert_eq!(line.translate_by(&crate::IntVector::ZERO), line);
        assert_eq!(
            line.translate_by_any(&crate::Vector::Int(crate::IntVector::new(1, 2))),
            Some(Line::new(IntPoint::new(1, 2), IntPoint::new(11, 2)))
        );
        let rational = crate::Vector::Rational(crate::RationalVector::new(
            BigInt::from(1),
            BigInt::from(1),
            BigInt::from(2),
        ));
        assert_eq!(line.translate_by_any(&rational), None);
    }

    #[test]
    fn turn_mirror_length_and_cos_angle() {
        let line = l(0, 0, 10, 0);
        let pole = IntPoint::ZERO;
        assert_eq!(
            line.turn_90_degree(1, &pole),
            Line::new(IntPoint::new(0, 0), IntPoint::new(0, 10))
        );
        // Java swaps the end points in both mirror methods.
        assert_eq!(
            line.mirror_vertical(&pole),
            Line::new(IntPoint::new(-10, 0), IntPoint::new(0, 0))
        );
        assert_eq!(
            line.mirror_horizontal(&pole),
            Line::new(IntPoint::new(10, 0), IntPoint::new(0, 0))
        );
        assert_eq!(l(0, 0, 3, 4).length(), 5.0_f32);
        assert_eq!(l(0, 0, 1, 0).cos_angle(&l(0, 0, 0, 1)), 0.0);
    }

    #[test]
    fn from_direction_and_from_direction_any() {
        let a = IntPoint::new(2, 3);
        assert_eq!(
            Line::from_direction(a, &IntDirection::UP),
            Line::new(a, IntPoint::new(2, 4))
        );
        assert_eq!(
            Line::from_direction_any(a, &Direction::Int(IntDirection::UP)),
            Some(Line::new(a, IntPoint::new(2, 4)))
        );
        // A BigIntDirection would translate `a` to a RationalPoint, which this port's `Line`
        // (IntPoint end points) cannot hold — Java only logs a warning there.
        let big = Direction::Big(crate::BigIntDirection::new(
            BigInt::from(crate::CRIT_INT + 5),
            BigInt::from(crate::CRIT_INT + 7),
        ));
        assert_eq!(Line::from_direction_any(a, &big), None);
    }

    /// Java `Line.compareTo` orders lines by the angle of their direction, counterclockwise from
    /// RIGHT: RIGHT < RIGHT45 < UP < UP45 < LEFT < LEFT45 < DOWN < DOWN45.
    #[test]
    fn compare_to_is_the_counterclockwise_angular_order() {
        let ccw = [
            l(0, 0, 1, 0),   // RIGHT
            l(0, 0, 1, 1),   // RIGHT45
            l(0, 0, 0, 1),   // UP
            l(0, 0, -1, 1),  // UP45
            l(0, 0, -1, 0),  // LEFT
            l(0, 0, -1, -1), // LEFT45
            l(0, 0, 0, -1),  // DOWN
            l(0, 0, 1, -1),  // DOWN45
        ];
        for i in 0..ccw.len() {
            for j in 0..ccw.len() {
                let expected = i.cmp(&j);
                assert_eq!(ccw[i].compare_to(&ccw[j]), expected, "{i} vs {j}");
            }
        }
        // The magnitude of the direction vector is irrelevant, and so is the base point:
        // parallel-but-distinct lines compare Equal while being structurally unequal.
        let a = l(0, 0, 1, 0);
        let b = l(5, 5, 9, 5);
        assert_eq!(a.compare_to(&b), std::cmp::Ordering::Equal);
        assert_ne!(a, b);
    }

    /// Java `Line.compareTo` is not antisymmetric for a degenerate line (`a == b`, zero
    /// direction) — the same hole as `IntDirection.compareTo` at `NULL`. This is why there is no
    /// `Ord`/`PartialOrd` impl for `Line`.
    #[test]
    fn compare_to_is_not_antisymmetric_at_the_degenerate_line() {
        let degenerate = l(0, 0, 0, 0);
        let right = l(0, 0, 1, 0);
        assert_eq!(degenerate.compare_to(&right), std::cmp::Ordering::Greater);
        assert_eq!(right.compare_to(&degenerate), std::cmp::Ordering::Equal);
    }

    /// Java's `Line.equals` is geometric (collinear end points + same direction sense), unlike
    /// the derived structural `PartialEq` on this type.
    #[test]
    fn equals_geometric_and_fast_equals() {
        let base = l(0, 0, 10, 0);
        let same_line_other_points = l(-7, 0, 3, 0);
        assert_ne!(base, same_line_other_points); // structural
        assert!(base.equals_geometric(&same_line_other_points)); // geometric
        assert!(base.fast_equals(&same_line_other_points));
        // Opposite direction: collinear, but the projection is negative.
        let opposite = base.opposite();
        assert!(!base.equals_geometric(&opposite));
        assert!(!base.fast_equals(&opposite));
        // Parallel but not collinear.
        let parallel = l(0, 3, 10, 3);
        assert!(!base.equals_geometric(&parallel));
        assert!(!base.fast_equals(&parallel));
        // Different direction through the same point.
        assert!(!base.equals_geometric(&l(0, 0, 0, 10)));
        assert!(!base.fast_equals(&l(0, 0, 0, 10)));
        // A degenerate line is *not* geometrically equal to itself: Java only returns true there
        // through the `this == other` reference shortcut, which has no Rust counterpart.
        let degenerate = l(4, 4, 4, 4);
        assert!(!degenerate.equals_geometric(&degenerate));
    }

    #[test]
    fn get_id_is_the_java_hash_and_wraps() {
        // 31 * (31 * 1 + 2) + (31 * 3 + 4) = 31 * 33 + 97 = 1120
        assert_eq!(l(1, 2, 3, 4).get_id(), 1120);
        assert_eq!(IntPoint::new(1, 2).get_id(), 33);
        // Java `int` arithmetic wraps silently; this must not panic in a debug build.
        let _ = l(i32::MAX, i32::MAX, i32::MIN, i32::MIN).get_id();
    }

    #[test]
    fn perpendicular_direction_from_a_point() {
        let line = l(0, 0, 10, 0);
        let above = Point::Int(IntPoint::new(3, 4));
        // Neither +/-1 step in the perpendicular directions crosses the line, so Java falls back
        // to the FloatPoint distance test and picks DOWN (the nearest line point is (3, 0)).
        assert_eq!(
            line.perpendicular_direction(&above),
            Some(Direction::Int(IntDirection::DOWN))
        );
        // One unit away: the DOWN check point lands exactly on the line, so the side changes and
        // Java returns dir2 = DOWN directly.
        assert_eq!(
            line.perpendicular_direction(&Point::Int(IntPoint::new(3, 1))),
            Some(Direction::Int(IntDirection::DOWN))
        );
        // Java returns null for a point on the line.
        assert_eq!(
            line.perpendicular_direction(&Point::Int(IntPoint::new(3, 0))),
            None
        );
    }

    #[test]
    fn line_perpendicular_projection_delegates_to_the_point() {
        let diag = l(0, 0, 1, 1);
        assert_eq!(
            diag.perpendicular_projection(&Point::Int(IntPoint::new(4, 0))),
            Point::Int(IntPoint::new(2, 2))
        );
    }
}
