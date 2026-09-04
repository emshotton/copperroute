//! Port of `app.freerouting.geometry.planar.FloatPoint`: a point in the plane as a tuple of
//! `f64`s. Because arithmetic with doubles is in general not exact, Java does not derive
//! `FloatPoint` from the abstract `Point` class, and neither does this port.

use std::fmt;

use crate::direction::Direction;
use crate::int_point::IntPoint;
use crate::rational_vector::big_sign;
use crate::side::Side;
use crate::vector::Vector;

/// A point in the plane with `f64` coordinates. `Copy`/`PartialEq` only (see conventions):
/// `f64` has neither a total order nor structural equality suitable for `Eq`/`Hash`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FloatPoint {
    /// The x coordinate of this point.
    pub x: f64,
    /// The y coordinate of this point.
    pub y: f64,
}

/// The signs of a `Vector`'s coordinates, used by `round_to_the_right`/`round_to_the_left`.
/// For `Vector::Rational`, Java reads `dir.getVector().toFloat()` and inspects the signs of the
/// resulting doubles; since a `RationalVector`'s denominator `z` is always `>= 0` (enforced by
/// `RationalVector::new`), the sign of the numerator alone gives the same answer without the
/// intermediate float division.
fn vector_signs(v: &Vector) -> (i32, i32) {
    match v {
        Vector::Int(iv) => (iv.x.signum(), iv.y.signum()),
        Vector::Rational(rv) => (big_sign(&rv.x) as i32, big_sign(&rv.y) as i32),
    }
}

impl FloatPoint {
    /// Standard implementation of the zero point.
    pub const ZERO: FloatPoint = FloatPoint { x: 0.0, y: 0.0 };

    /// Creates an instance of class FloatPoint from two doubles.
    pub fn new(x: f64, y: f64) -> FloatPoint {
        FloatPoint { x, y }
    }

    /// Creates a FloatPoint from an IntPoint.
    pub fn from_int(pt: &IntPoint) -> FloatPoint {
        FloatPoint::new(pt.x as f64, pt.y as f64)
    }

    // not ported: `boundingOctagon(FloatPoint[])` — depends on `IntOctagon` (Task 12).
    // not ported: `toString(Locale)` (FloatPoint.java:469-473) — GUI-only locale variant of
    // the hard-coded English `Display` impl below, unused outside `geometry/planar`.
    // not ported: `toString(Locale, int, int)` (FloatPoint.java:476-484) — padded GUI display
    // string, unused outside `geometry/planar`.

    /// Returns the square of the distance from this point to the zero point.
    pub fn size_square(&self) -> f64 {
        self.x * self.x + self.y * self.y
    }

    /// Returns the distance from this point to the zero point.
    pub fn size(&self) -> f64 {
        self.size_square().sqrt()
    }

    /// Returns the square of the distance from this point to other.
    pub fn distance_square(&self, other: &FloatPoint) -> f64 {
        let dx = other.x - self.x;
        let dy = other.y - self.y;
        dx * dx + dy * dy
    }

    /// Returns the distance from this point to other.
    pub fn distance(&self, other: &FloatPoint) -> f64 {
        self.distance_square(other).sqrt()
    }

    /// Computes the weighted distance to other.
    pub fn weighted_distance(
        &self,
        other: &FloatPoint,
        horizontal_weight: f64,
        vertical_weight: f64,
    ) -> f64 {
        let mut delta_x = self.x - other.x;
        let mut delta_y = self.y - other.y;
        delta_x *= horizontal_weight;
        delta_y *= vertical_weight;
        (delta_x * delta_x + delta_y * delta_y).sqrt()
    }

    /// Rounds the coordinates of this point to an `IntPoint`.
    pub fn round(&self) -> IntPoint {
        IntPoint::new(
            crate::limits::java_round(self.x) as i32,
            crate::limits::java_round(self.y) as i32,
        )
    }

    /// Rounds this point, so that if this point is on the right side of any directed line with
    /// direction dir, the result point will also be on the right side.
    pub fn round_to_the_right(&self, dir: &Direction) -> IntPoint {
        let (dir_x, dir_y) = vector_signs(&dir.get_vector());

        let rounded_x = if dir_y > 0 {
            self.x.ceil() as i32
        } else if dir_y < 0 {
            self.x.floor() as i32
        } else {
            crate::limits::java_round(self.x) as i32
        };

        let rounded_y = if dir_x > 0 {
            self.y.floor() as i32
        } else if dir_x < 0 {
            self.y.ceil() as i32
        } else {
            crate::limits::java_round(self.y) as i32
        };
        IntPoint::new(rounded_x, rounded_y)
    }

    /// Round this point so the x coordinate of the result will be a multiple of
    /// `horizontal_grid` and the y coordinate a multiple of `vertical_grid`.
    ///
    /// Java uses `Math.rint` here (round-half-to-even), unlike the `Math.round`-based (round
    /// half up) methods elsewhere in this file — reproduced with `f64::round_ties_even`, not
    /// `java_round`.
    pub fn round_to_grid(&self, horizontal_grid: i32, vertical_grid: i32) -> IntPoint {
        let rounded_x = if horizontal_grid > 0 {
            (self.x / horizontal_grid as f64).round_ties_even() * horizontal_grid as f64
        } else {
            self.x
        };
        let rounded_y = if vertical_grid > 0 {
            (self.y / vertical_grid as f64).round_ties_even() * vertical_grid as f64
        } else {
            self.y
        };
        IntPoint::new(rounded_x as i32, rounded_y as i32)
    }

    /// Rounds this point, so that if this point is on the left side of any directed line with
    /// direction dir, the result point will also be on the left side.
    pub fn round_to_the_left(&self, dir: &Direction) -> IntPoint {
        let (dir_x, dir_y) = vector_signs(&dir.get_vector());

        let rounded_x = if dir_y > 0 {
            self.x.floor() as i32
        } else if dir_y < 0 {
            self.x.ceil() as i32
        } else {
            crate::limits::java_round(self.x) as i32
        };

        let rounded_y = if dir_x > 0 {
            self.y.ceil() as i32
        } else if dir_x < 0 {
            self.y.floor() as i32
        } else {
            crate::limits::java_round(self.y) as i32
        };
        IntPoint::new(rounded_x, rounded_y)
    }

    /// Adds the coordinates of this FloatPoint and other.
    pub fn add(&self, other: &FloatPoint) -> FloatPoint {
        FloatPoint::new(self.x + other.x, self.y + other.y)
    }

    /// Subtracts the coordinates of other from this FloatPoint.
    pub fn subtract(&self, other: &FloatPoint) -> FloatPoint {
        FloatPoint::new(self.x - other.x, self.y - other.y)
    }

    /// Returns an approximation of the perpendicular projection of this point onto line.
    pub fn projection_approx(&self, line: &crate::line::Line) -> FloatPoint {
        let float_line = crate::float_line::FloatLine::new(line.a.to_float(), line.b.to_float());
        float_line.perpendicular_projection(self)
    }

    /// Calculates the scalar product of (p1 - this) with (p2 - this).
    ///
    /// Java null-checks `p1`/`p2` and logs a warning, returning 0 for a null argument
    /// (FloatPoint.java:207-214) — not applicable here, since Rust references cannot be null.
    pub fn scalar_product(&self, p1: &FloatPoint, p2: &FloatPoint) -> f64 {
        let dx1 = p1.x - self.x;
        let dx2 = p2.x - self.x;
        let dy1 = p1.y - self.y;
        let dy2 = p2.y - self.y;
        dx1 * dx2 + dy1 * dy2
    }

    /// Approximates a FloatPoint on the line from zero to this point with distance new_size
    /// from zero.
    pub fn change_size(&self, new_size: f64) -> FloatPoint {
        if self.x == 0.0 && self.y == 0.0 {
            // the size of the zero point cannot be changed
            return *self;
        }
        let length = (self.x * self.x + self.y * self.y).sqrt();
        let new_x = (self.x * new_size) / length;
        let new_y = (self.y * new_size) / length;
        FloatPoint::new(new_x, new_y)
    }

    /// Approximates a FloatPoint on the line from this point to to_point with distance
    /// new_length from this point.
    ///
    /// Java logs a warning and returns `to_point` unchanged when the two points are equal
    /// (FloatPoint.java:239-246); the returned value is reproduced, the log is not (`fr-geometry`
    /// has no `tracing` dependency).
    pub fn change_length(&self, to_point: &FloatPoint, new_length: f64) -> FloatPoint {
        let dx = to_point.x - self.x;
        let dy = to_point.y - self.y;
        if dx == 0.0 && dy == 0.0 {
            return *to_point;
        }
        let length = (dx * dx + dy * dy).sqrt();
        let new_x = self.x + (dx * new_length) / length;
        let new_y = self.y + (dy * new_length) / length;
        FloatPoint::new(new_x, new_y)
    }

    /// Returns the middle point between this point and to_point.
    ///
    /// Java's `toPoint == this` reference-equality short-circuit (FloatPoint.java:252-254) is not
    /// reproduced: `FloatPoint` is `Copy` value type here, so there is no object identity to
    /// compare, and the plain formula below already returns the same value for equal points.
    pub fn middle_point(&self, to_point: &FloatPoint) -> FloatPoint {
        let middle_x = 0.5 * (self.x + to_point.x);
        let middle_y = 0.5 * (self.y + to_point.y);
        FloatPoint::new(middle_x, middle_y)
    }

    /// The function returns `Side::OnTheLeft`, if this point is on the left of the line from p1
    /// to p2; and `Side::OnTheRight`, if this point is on the right of the line from p1 to p2.
    /// Collinearity is not defined, because numerical calculations are not exact for FloatPoints.
    pub fn side_of(&self, p1: &FloatPoint, p2: &FloatPoint) -> Side {
        let d21x = p2.x - p1.x;
        let d21y = p2.y - p1.y;
        let d01x = self.x - p1.x;
        let d01y = self.y - p1.y;
        let determinant = d21x * d01y - d21y * d01x;
        Side::of_f64(determinant)
    }

    /// Rotates this FloatPoint by angle (in radians) around the pole.
    pub fn rotate(&self, angle: f64, pole: &FloatPoint) -> FloatPoint {
        if angle == 0.0 {
            return *self;
        }
        let dx = self.x - pole.x;
        let dy = self.y - pole.y;
        let sin_angle = angle.sin();
        let cos_angle = angle.cos();
        let new_dx = dx * cos_angle - dy * sin_angle;
        let new_dy = dx * sin_angle + dy * cos_angle;
        FloatPoint::new(pole.x + new_dx, pole.y + new_dy)
    }

    /// Turns this FloatPoint by factor times 90 degrees around ZERO.
    pub fn turn_90_degree(&self, factor: i32) -> FloatPoint {
        match factor.rem_euclid(4) {
            0 => FloatPoint::new(self.x, self.y),   // 0 degrees
            1 => FloatPoint::new(-self.y, self.x),  // 90 degrees
            2 => FloatPoint::new(-self.x, -self.y), // 180 degrees
            3 => FloatPoint::new(self.y, -self.x),  // 270 degrees
            _ => FloatPoint::ZERO,
        }
    }

    /// Turns this FloatPoint by factor times 90 degrees around pole. Java overload
    /// `turn90Degree(int, FloatPoint)`.
    pub fn turn_90_degree_pole(&self, factor: i32, pole: &FloatPoint) -> FloatPoint {
        let v = self.subtract(pole);
        let v = v.turn_90_degree(factor);
        pole.add(&v)
    }

    /// Checks, if this point is contained in the box spanned by p1 and p2 with the input
    /// tolerance.
    pub fn is_contained_in_box(&self, p1: &FloatPoint, p2: &FloatPoint, tolerance: f64) -> bool {
        let (min_x, max_x) = if p1.x < p2.x {
            (p1.x, p2.x)
        } else {
            (p2.x, p1.x)
        };
        if self.x < min_x - tolerance || self.x > max_x + tolerance {
            return false;
        }
        let (min_y, max_y) = if p1.y < p2.y {
            (p1.y, p2.y)
        } else {
            (p2.y, p1.y)
        };
        self.y >= min_y - tolerance && self.y <= max_y + tolerance
    }

    /// Creates the smallest IntBox with integer coordinates containing this point, rounding
    /// outward. Java `FloatPoint.boundingBox()`.
    pub fn bounding_box(&self) -> crate::int_box::IntBox {
        crate::int_box::IntBox::from_coords(
            self.x.floor() as i32,
            self.y.floor() as i32,
            self.x.ceil() as i32,
            self.y.ceil() as i32,
        )
    }

    /// Calculates the touching points of the tangents from this point to a circle around
    /// to_point with radius distance. Solves the quadratic equation which results by
    /// substituting x by the term in y from the equation of the polar line of a circle with
    /// center to_point and radius distance and putting it into the circle equation. The polar
    /// line is the line through the 2 tangential points of the circle looked at from this point
    /// and has the equation `(this.x - toPoint.x) * (x - toPoint.x) + (this.y - toPoint.y) * (y -
    /// toPoint.y) = distance**2`.
    ///
    /// Returns `None` if this point is inside the circle (Java returns an empty array there).
    pub fn tangential_points(
        &self,
        to_point: &FloatPoint,
        distance: f64,
    ) -> Option<[FloatPoint; 2]> {
        // turn the situation 90 degree if the x difference is smaller than the y difference for
        // better numerical stability
        let mut dx = (self.x - to_point.x).abs();
        let dy_abs = (self.y - to_point.y).abs();
        let situation_turned = dy_abs > dx;

        let (pole, circle_center) = if situation_turned {
            // turn the situation by 90 degree
            (
                FloatPoint::new(-self.y, self.x),
                FloatPoint::new(-to_point.y, to_point.x),
            )
        } else {
            (*self, *to_point)
        };

        dx = pole.x - circle_center.x;
        let dy = pole.y - circle_center.y;
        let dx_square = dx * dx;
        let dy_square = dy * dy;
        let dist_square = dx_square + dy_square;
        let radius_square = distance * distance;
        let discriminant = radius_square * dy_square - (radius_square - dx_square) * dist_square;

        if discriminant <= 0.0 {
            // pole is inside the circle.
            return None;
        }
        let square_root = discriminant.sqrt();

        let a1 = radius_square * dy;
        let dy1 = (a1 + distance * square_root) / dist_square;
        let dy2 = (a1 - distance * square_root) / dist_square;

        let first_point_y = dy1 + circle_center.y;
        let first_point_x = (radius_square - dy * dy1) / dx + circle_center.x;
        let second_point_y = dy2 + circle_center.y;
        let second_point_x = (radius_square - dy * dy2) / dx + circle_center.x;

        if situation_turned {
            // turn the result by 270 degree
            Some([
                FloatPoint::new(first_point_y, -first_point_x),
                FloatPoint::new(second_point_y, -second_point_x),
            ])
        } else {
            Some([
                FloatPoint::new(first_point_x, first_point_y),
                FloatPoint::new(second_point_x, second_point_y),
            ])
        }
    }

    /// Calculates the left tangential point of the line from this point to a circle around
    /// to_point with radius distance. Returns `None`, if this point is inside this circle.
    pub fn left_tangential_point(
        &self,
        to_point: &FloatPoint,
        distance: f64,
    ) -> Option<FloatPoint> {
        let tangent_points = self.tangential_points(to_point, distance)?;
        Some(
            if to_point.side_of(self, &tangent_points[0]) == Side::OnTheRight {
                tangent_points[0]
            } else {
                tangent_points[1]
            },
        )
    }

    /// Calculates the right tangential point of the line from this point to a circle around
    /// to_point with radius distance. Returns `None`, if this point is inside this circle.
    pub fn right_tangential_point(
        &self,
        to_point: &FloatPoint,
        distance: f64,
    ) -> Option<FloatPoint> {
        let tangent_points = self.tangential_points(to_point, distance)?;
        Some(
            if to_point.side_of(self, &tangent_points[0]) == Side::OnTheLeft {
                tangent_points[0]
            } else {
                tangent_points[1]
            },
        )
    }

    /// Calculates the center of the circle through this point, p1 and p2 by calculating the
    /// intersection of the two lines perpendicular to and passing through the midpoints of the
    /// lines (this, p1) and (p1, p2).
    ///
    /// Java's `circleCenter` (FloatPoint.java:409-417) never returns `null`: for collinear
    /// input, or when the segment from `this` to `p1` happens to be horizontal, the slope-based
    /// formula divides by zero and Java returns a `FloatPoint` with a `NaN`/infinite coordinate
    /// instead. This port surfaces that as `None` (checked with `f64::is_finite`), which is the
    /// only sensible outcome for a caller — a `FloatPoint` full of `NaN` cannot denote a circle
    /// center.
    ///
    /// # deferred to Task 13 (#82) — quirk #9, and why it is not fixed with #15/#16/#13
    ///
    /// The horizontal case is quirk **#9**, and Plan 9 Task 11 fixes the other three of
    /// #15/#16/#9/#13 and deliberately leaves this one. It is the *mechanism* of **#82**, so
    /// fixing it here would move #82's behaviour without #82's measurement; both ends of the split
    /// name each other, and this arm lands with #82 in **Task 13**.
    ///
    /// The remedy is "swap the point roles for the horizontal case", and the reason that works is
    /// worth recording where the fix will happen: the circumcentre **exists and is computable**,
    /// and only the argument order decides whether it is found. Measured —
    ///
    /// ```text
    /// circle_center((0,0), (1000,0), (1000,1000))   -> None            first pair horizontal
    /// circle_center((1000,0), (1000,1000), (0,0))   -> None            first pair vertical
    /// circle_center((1000,1000), (0,0), (1000,0))   -> Some((500,500)) the correct answer
    /// ```
    ///
    /// — three of the six orders find the same circle the other three refuse.
    /// `crates/fr-geometry/tests/nearest_and_stairs.rs::circle_center_is_deferred_to_task_13`
    /// pins those three rows, so Task 13 inherits a before-picture rather than a promise.
    pub fn circle_center(&self, p1: &FloatPoint, p2: &FloatPoint) -> Option<FloatPoint> {
        let slope1 = (p1.y - self.y) / (p1.x - self.x);
        let slope2 = (p2.y - p1.y) / (p2.x - p1.x);
        let center_x = (slope1 * slope2 * (self.y - p2.y) + slope2 * (self.x + p1.x)
            - slope1 * (p1.x + p2.x))
            / (2.0 * (slope2 - slope1));
        let center_y = (0.5 * (self.x + p1.x) - center_x) / slope1 + 0.5 * (self.y + p1.y);
        if center_x.is_finite() && center_y.is_finite() {
            Some(FloatPoint::new(center_x, center_y))
        } else {
            None
        }
    }

    /// Returns true, if this point is contained in the circle through p1, p2 and p3.
    ///
    /// Java computes `p1.circleCenter(p2, p3)` unconditionally and lets a `NaN` center make the
    /// final comparison `false` (any comparison against `NaN` is `false` in both Java and Rust).
    /// Since `circle_center` returns `None` in exactly that situation, mapping `None` to `false`
    /// reproduces the same observable result.
    pub fn inside_circle(&self, p1: &FloatPoint, p2: &FloatPoint, p3: &FloatPoint) -> bool {
        match p1.circle_center(p2, p3) {
            Some(center) => {
                let radius_square = center.distance_square(p1);
                // -1 is a tolerance for numerical stability.
                self.distance_square(&center) < radius_square - 1.0
            }
            None => false,
        }
    }

    /// Formats one coordinate the way Java's `NumberFormat.getInstance(Locale.ENGLISH)` does with
    /// `setMaximumFractionDigits(4)`: up to 4 fraction digits (trailing zeros dropped), and
    /// thousands grouped with commas. Java's `toString()` always uses `Locale.ENGLISH`
    /// regardless of the platform default, so this hard-codes the same formatting rather than
    /// taking a locale parameter (Java's `toString(Locale)` overload, used only by other
    /// locales, is not ported — GUI-only, unused outside `geometry/planar`).
    ///
    /// `NumberFormat.format` handles non-finite doubles specially rather than throwing: `NaN`
    /// prints as `"NaN"`, and the infinities print as `"\u{221e}"`/`"-\u{221e}"` (verified against
    /// a standalone `javac`/`java` run of `NumberFormat.getInstance(Locale.ENGLISH).format(...)`).
    /// Handled before the fixed-precision path below, which would otherwise panic: `format!("{:.4}",
    /// _)` renders non-finite values as `"NaN"`/`"inf"`/`"-inf"`, none of which contain a `.` for
    /// `split_once` to find.
    fn format_component(value: f64) -> String {
        if value.is_nan() {
            return "NaN".to_string();
        }
        if value.is_infinite() {
            return if value > 0.0 { "\u{221e}" } else { "-\u{221e}" }.to_string();
        }

        let negative = value.is_sign_negative();
        let formatted = format!("{:.4}", value.abs());
        let (int_part, frac_part) = formatted
            .split_once('.')
            .expect("fixed precision always has a '.'");
        let frac_trimmed = frac_part.trim_end_matches('0');

        let mut result = String::with_capacity(int_part.len() + frac_trimmed.len() + 6);
        if negative {
            result.push('-');
        }
        result.push_str(&group_thousands(int_part));
        if !frac_trimmed.is_empty() {
            result.push('.');
            result.push_str(frac_trimmed);
        }
        result
    }
}

/// Inserts `,` every 3 digits from the right, matching `NumberFormat`'s default grouping.
fn group_thousands(digits: &str) -> String {
    let reversed: Vec<char> = digits.chars().rev().collect();
    let mut out: Vec<char> = Vec::with_capacity(reversed.len() + reversed.len() / 3);
    for (i, c) in reversed.iter().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push(',');
        }
        out.push(*c);
    }
    out.reverse();
    out.into_iter().collect()
}

impl fmt::Display for FloatPoint {
    /// Java `toString()` (FloatPoint.java:486-489) always delegates to `toString(Locale.ENGLISH)`
    /// (FloatPoint.java:469-473): `"(" + nf.format(x) + " , " + nf.format(y) + ")"` with
    /// `NumberFormat.getInstance(Locale.ENGLISH)` and `setMaximumFractionDigits(4)`.
    ///
    /// Java's `toString(Locale)` (FloatPoint.java:469-473) and `toString(Locale, int, int)`
    /// (FloatPoint.java:476-484) overloads — used only for other locales / GUI padded display —
    /// are not ported.
    ///
    /// Two known deviations, both from Rust's `{:.4}` formatting the *exact binary expansion* of
    /// the double where Java's `NumberFormat` formats its *shortest round-trip decimal digits*:
    /// * doubles that are exact decimal ties at the 4th fraction digit round differently (e.g.
    ///   `5.0E-5`, whose true binary value sits a hair above the decimal midpoint, rounds to
    ///   `"0"` in Java's `HALF_EVEN` but `"0.0001"` here);
    /// * doubles at or beyond `2^53` (`CRIT_DOUBLE`, e.g. `f32::MAX as f64`, which
    ///   `RationalPoint::to_float` returns for a point at infinity) can render different digits
    ///   entirely, since the exact binary expansion of such a magnitude no longer matches its
    ///   shortest round-trip decimal representation.
    ///
    /// Neither arises from any finite PCB coordinate computed in this crate, so reproducing
    /// Java's shortest-round-trip-then-round-half-even algorithm exactly was judged not worth the
    /// complexity for a diagnostic `Display` impl.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "({} , {})",
            FloatPoint::format_component(self.x),
            FloatPoint::format_component(self.y)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::direction::Direction;
    use crate::int_direction::IntDirection;
    use crate::int_point::IntPoint;

    #[test]
    fn round_uses_java_semantics() {
        assert_eq!(FloatPoint::new(1.5, -1.5).round(), IntPoint::new(2, -1));
        assert_eq!(FloatPoint::new(2.4, 2.6).round(), IntPoint::new(2, 3));
    }

    #[test]
    fn round_to_the_right_of_direction() {
        // Java: for dir UP (x==0, y>0): x is ceil'd, y is Math.round'd.
        let p = FloatPoint::new(1.2, 3.7);
        let r = p.round_to_the_right(&Direction::Int(IntDirection::UP));
        assert_eq!(r, IntPoint::new(2, 4));
        let l = p.round_to_the_left(&Direction::Int(IntDirection::UP));
        assert_eq!(l, IntPoint::new(1, 4));
    }

    #[test]
    fn round_to_grid() {
        assert_eq!(
            FloatPoint::new(17.0, 26.0).round_to_grid(10, 10),
            IntPoint::new(20, 30)
        );
        assert_eq!(
            FloatPoint::new(17.0, 26.0).round_to_grid(0, 10),
            IntPoint::new(17, 30)
        );
    }

    #[test]
    fn size_change_and_middle() {
        let p = FloatPoint::new(3.0, 4.0);
        assert_eq!(p.size(), 5.0);
        let q = p.change_size(10.0);
        assert!((q.x - 6.0).abs() < 1e-12 && (q.y - 8.0).abs() < 1e-12);
        assert_eq!(
            FloatPoint::new(0.0, 0.0).middle_point(&p),
            FloatPoint::new(1.5, 2.0)
        );
    }

    #[test]
    fn circle_center_and_inside_circle() {
        // Corrected per Java (FloatPoint.java:409-416): the brief's original a=(0,0), b=(2,0),
        // c=(0,2) makes slope1 == 0 (the segment `this`->p1 is horizontal), so Java's
        // `centerY = (...)/ slope1 + ...` divides by zero and the *real* Java output for that
        // input is (1.0, NaN), not (1.0, 1.0) — verified by running the formula standalone.
        // Using a well-conditioned (non-axis-aligned) triangle instead; center and radius
        // cross-checked by confirming a, b, c are equidistant from the computed center.
        let a = FloatPoint::new(1.0, 1.0);
        let b = FloatPoint::new(4.0, 2.0);
        let c = FloatPoint::new(2.0, 5.0);
        let center = a.circle_center(&b, &c).unwrap();
        assert!((center.x - 2.045_454_545_454_545_5).abs() < 1e-9);
        assert!((center.y - 2.863_636_363_636_363_8).abs() < 1e-9);

        // Collinear input: this->p1 and p1->p2 both have slope 0, so centerX is 0/0 == NaN.
        assert!(
            FloatPoint::new(0.0, 0.0)
                .circle_center(&FloatPoint::new(2.0, 0.0), &FloatPoint::new(4.0, 0.0))
                .is_none()
        );

        assert!(FloatPoint::new(2.0, 2.0).inside_circle(&a, &b, &c));
        assert!(!FloatPoint::new(20.0, 20.0).inside_circle(&a, &b, &c));
    }

    #[test]
    fn tangential_points_none_when_inside() {
        let origin = FloatPoint::new(0.0, 0.0);
        assert!(
            origin
                .tangential_points(&FloatPoint::new(1.0, 0.0), 5.0)
                .is_none()
        );
        let to_point = FloatPoint::new(10.0, 0.0);
        let t = origin.tangential_points(&to_point, 5.0).unwrap();
        // Corrected per Java (FloatPoint.java:319-327): the tangential points lie on the circle
        // *around to_point*, so their distance from to_point (not from the zero point / origin)
        // is the radius. The brief's `p.size()` checks distance from the zero point, which here
        // happens to equal `origin`, but that is the tangent *length* (sqrt(10^2 - 5^2) ≈ 8.66),
        // not the radius — verified against the Java algorithm directly.
        for p in t {
            assert!((p.distance(&to_point) - 5.0).abs() < 1e-9);
        }
    }

    #[test]
    fn display_matches_java_number_format() {
        // Java `toString()`: "(" + nf.format(x) + " , " + nf.format(y) + ")", with
        // NumberFormat.getInstance(Locale.ENGLISH), setMaximumFractionDigits(4), grouping on.
        assert_eq!(FloatPoint::new(1.5, -2.0).to_string(), "(1.5 , -2)");
        assert_eq!(
            FloatPoint::new(1_234_567.891_234, -0.0).to_string(),
            "(1,234,567.8912 , -0)"
        );
        assert_eq!(FloatPoint::new(100.0, 0.1).to_string(), "(100 , 0.1)");
    }

    #[test]
    fn display_of_non_finite_coordinates_matches_java() {
        // Verified against a standalone `javac`/`java` run of
        // `NumberFormat.getInstance(Locale.ENGLISH).format(...)`:
        //   NaN              -> "NaN"
        //   Double.POSITIVE_INFINITY -> "∞"
        //   Double.NEGATIVE_INFINITY -> "-∞"
        // These are reachable in practice: `RationalVector::to_float` divides by a zero
        // denominator, and `FloatLine::translate` divides by zero on a degenerate segment.
        assert_eq!(
            FloatPoint::new(f64::NAN, f64::INFINITY).to_string(),
            "(NaN , \u{221e})"
        );
        assert_eq!(
            FloatPoint::new(f64::NEG_INFINITY, 0.0).to_string(),
            "(-\u{221e} , 0)"
        );
    }

    #[test]
    fn projection_approx_onto_a_line() {
        use crate::line::Line;
        // FloatPoint.projectionApprox(Line) builds a FloatLine from the line's end points and
        // delegates to FloatLine.perpendicularProjection.
        let line = Line::from_coords(0, 0, 10, 0);
        assert_eq!(
            FloatPoint::new(3.0, 4.0).projection_approx(&line),
            FloatPoint::new(3.0, 0.0)
        );
    }
}
