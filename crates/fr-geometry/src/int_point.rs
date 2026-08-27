//! Port of `app.freerouting.geometry.planar.IntPoint`: an implementation of the (abstract, in
//! Java) `Point` class via a tuple of `i32` coordinates. Also ports the `IntPoint`-relevant
//! default methods of `Point.java` (`sideOf(p1,p2)`, `compareXY`, `turn90Degree`,
//! `mirrorVertical`/`mirrorHorizontal`).

use std::cmp::Ordering;
use std::fmt;

use crate::float_point::FloatPoint;
use crate::int_vector::IntVector;
use crate::side::Side;

/// Implementation of a Point via a tuple of integers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IntPoint {
    /// The x coordinate of this point.
    pub x: i32,
    /// The y coordinate of this point.
    pub y: i32,
}

impl IntPoint {
    /// Standard implementation of the zero point.
    pub const ZERO: IntPoint = IntPoint { x: 0, y: 0 };

    /// Creates an IntPoint from two integer coordinates.
    ///
    /// Note: unlike Java, this does not range-check against `CRIT_INT` and log — `fr-geometry`
    /// must not depend on `tracing`, and out-of-range promotion to a rational representation
    /// happens one level up (in a later task).
    pub fn new(x: i32, y: i32) -> IntPoint {
        IntPoint { x, y }
    }

    // not ported: `equals`/`hashCode` — replaced by `#[derive(PartialEq, Eq, Hash)]` (structural
    // equality on x/y matches Java's `equals`; Java's custom `31 * x + y` hash is not needed by
    // any behavior we port).
    // not ported: `isInfinite` — always `false` for IntPoint, unused outside geometry/planar.

    /// Returns a deterministic tie-breaking id for this point (Java `31 * x + y`,
    /// IntPoint.java:126-128). Java `int` arithmetic wraps silently on overflow; this is a
    /// hash-shaped value, not a magnitude, so `wrapping_*` reproduces that (plain `*`/`+` would
    /// panic on overflow in a debug/test build).
    pub fn get_id(&self) -> i32 {
        31i32.wrapping_mul(self.x).wrapping_add(self.y)
    }

    /// Returns the translation of this point by vector.
    pub fn translate_by(&self, vector: &IntVector) -> IntPoint {
        IntPoint::new(self.x + vector.x, self.y + vector.y)
    }

    /// Returns the difference vector of this point and other.
    pub fn difference_by(&self, other: &IntPoint) -> IntVector {
        IntVector::new(self.x - other.x, self.y - other.y)
    }

    /// Returns the determinant of the vectors (x, y) and (other.x, other.y).
    pub fn determinant(&self, other: &IntPoint) -> i64 {
        self.x as i64 * other.y as i64 - self.y as i64 * other.x as i64
    }

    /// Returns the signed area of the parallelogramm spanned by the vectors p2 - p1 and this -
    /// p1. Java returns the `long` determinant as `double`; kept as `i64` here (see brief).
    pub fn signed_area(&self, p1: &IntPoint, p2: &IntPoint) -> i64 {
        let d21 = p2.difference_by(p1);
        let d01 = self.difference_by(p1);
        d21.determinant(&d01)
    }

    /// Calculates the square of the distance between this point and to_point.
    pub fn distance_square(&self, to_point: &IntPoint) -> f64 {
        let dx = (to_point.x - self.x) as f64;
        let dy = (to_point.y - self.y) as f64;
        dx * dx + dy * dy
    }

    /// Calculates the distance between this point and to_point.
    pub fn distance(&self, to_point: &IntPoint) -> f64 {
        self.distance_square(to_point).sqrt()
    }

    /// Calculates the nearest point to this point on the horizontal or vertical line through
    /// other (Snaps this point to on orthogonal line through other).
    pub fn orthogonal_projection(&self, other: &IntPoint) -> IntPoint {
        let horizontal_distance = (self.x - other.x).abs();
        let vertical_distance = (self.y - other.y).abs();
        if horizontal_distance <= vertical_distance {
            // projection onto the vertical line through other
            IntPoint::new(other.x, self.y)
        } else {
            // projection onto the horizontal line through other
            IntPoint::new(self.x, other.y)
        }
    }

    /// Calculates the nearest point to this point on an orthogonal or diagonal line through
    /// other (Snaps this point to on 45 degree line through other).
    pub fn fortyfive_degree_projection(&self, other: &IntPoint) -> IntPoint {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dist_arr = [
            dx.abs() as f64,
            dy.abs() as f64,
            0.0, // placeholder, filled below
            0.0,
        ];
        let diagonal1 = (dy as f64 - dx as f64) / 2.0;
        let diagonal2 = (dy as f64 + dx as f64) / 2.0;
        let dist_arr = [dist_arr[0], dist_arr[1], diagonal1.abs(), diagonal2.abs()];
        let mut min_dist = dist_arr[0];
        for &d in &dist_arr[1..4] {
            if d < min_dist {
                min_dist = d;
            }
        }
        if min_dist == dist_arr[0] {
            // projection onto the vertical line through other
            IntPoint::new(other.x, self.y)
        } else if min_dist == dist_arr[1] {
            // projection onto the horizontal line through other
            IntPoint::new(self.x, other.y)
        } else if min_dist == dist_arr[2] {
            // projection onto the right diagonal line through other
            let diagonal_value = diagonal2 as i32;
            IntPoint::new(other.x + diagonal_value, other.y + diagonal_value)
        } else {
            // projection onto the left diagonal line through other
            let diagonal_value = diagonal1 as i32;
            IntPoint::new(other.x - diagonal_value, other.y + diagonal_value)
        }
    }

    /// Calculates a corner point p so that the lines through this point and p and from p to
    /// to_point are multiples of 45 degree, and that the angle at p will be 45 degree. If
    /// left_turn, to_point will be on the left of the line from this point to p, else on the
    /// right. Returns `None`, if the line from this point to to_point is already a multiple of
    /// 45 degree.
    pub fn fortyfive_degree_corner(
        &self,
        to_point: &IntPoint,
        left_turn: bool,
    ) -> Option<IntPoint> {
        let dx = to_point.x - self.x;
        let dy = to_point.y - self.y;

        // handle the 8 sections between the 45 degree lines

        let result = if dy > 0 && dy < dx {
            if left_turn {
                IntPoint::new(to_point.x - dy, self.y)
            } else {
                IntPoint::new(self.x + dy, to_point.y)
            }
        } else if dx > 0 && dy > dx {
            if left_turn {
                IntPoint::new(to_point.x, self.y + dx)
            } else {
                IntPoint::new(self.x, to_point.y - dx)
            }
        } else if dx < 0 && dy > -dx {
            if left_turn {
                IntPoint::new(self.x, to_point.y + dx)
            } else {
                IntPoint::new(to_point.x, self.y - dx)
            }
        } else if dy > 0 && dy < -dx {
            if left_turn {
                IntPoint::new(self.x - dy, to_point.y)
            } else {
                IntPoint::new(to_point.x + dy, self.y)
            }
        } else if dy < 0 && dy > dx {
            if left_turn {
                IntPoint::new(to_point.x - dy, self.y)
            } else {
                IntPoint::new(self.x + dy, to_point.y)
            }
        } else if dx < 0 && dy < dx {
            if left_turn {
                IntPoint::new(to_point.x, self.y + dx)
            } else {
                IntPoint::new(self.x, to_point.y - dx)
            }
        } else if dx > 0 && dy < -dx {
            if left_turn {
                IntPoint::new(self.x, to_point.y + dx)
            } else {
                IntPoint::new(to_point.x, self.y - dx)
            }
        } else if dy < 0 && dy > -dx {
            if left_turn {
                IntPoint::new(self.x - dy, to_point.y)
            } else {
                IntPoint::new(to_point.x + dy, self.y)
            }
        } else {
            // the line from this point to to_point is already a multiple of 45 degree
            return None;
        };
        Some(result)
    }

    /// Calculates a corner point p so that the lines through this point and p and from p to
    /// to_point are horizontal or vertical, and that the angle at p will be 90 degree. If
    /// left_turn, to_point will be on the left of the line from this point to p, else on the
    /// right. Returns `None`, if the line from this point to to_point is already orthogonal.
    pub fn ninety_degree_corner(&self, to_point: &IntPoint, left_turn: bool) -> Option<IntPoint> {
        let dx = to_point.x - self.x;
        let dy = to_point.y - self.y;

        // handle the 4 quadrants

        let result = if (dx > 0 && dy > 0) || (dx < 0 && dy < 0) {
            if left_turn {
                IntPoint::new(to_point.x, self.y)
            } else {
                IntPoint::new(self.x, to_point.y)
            }
        } else if (dx < 0 && dy > 0) || (dx > 0 && dy < 0) {
            if left_turn {
                IntPoint::new(self.x, to_point.y)
            } else {
                IntPoint::new(to_point.x, self.y)
            }
        } else {
            // the line from this point to to_point is already orthogonal
            return None;
        };
        Some(result)
    }

    /// Returns `Ordering::Greater`, if this Point has a strict bigger x coordinate than other,
    /// `Ordering::Equal`, if the x coordinates are equal, and `Ordering::Less` otherwise.
    pub fn compare_x(&self, other: &IntPoint) -> Ordering {
        self.x.cmp(&other.x)
    }

    /// Returns `Ordering::Greater`, if this Point has a strict bigger y coordinate than other,
    /// `Ordering::Equal`, if the y coordinates are equal, and `Ordering::Less` otherwise.
    pub fn compare_y(&self, other: &IntPoint) -> Ordering {
        self.y.cmp(&other.y)
    }

    /// Returns `compare_x(other)`, if the result is not `Ordering::Equal`. Otherwise, it returns
    /// `compare_y(other)`.
    pub fn compare_xy(&self, other: &IntPoint) -> Ordering {
        match self.compare_x(other) {
            Ordering::Equal => self.compare_y(other),
            result => result,
        }
    }

    /// The function returns `Side::OnTheLeft`, if this Point is on the left of the line from p1
    /// to p2; `Side::OnTheRight`, if this Point is on the right of the line from p1 to p2; and
    /// `Side::Collinear`, if this Point is collinear with p1 and p2.
    pub fn side_of(&self, p1: &IntPoint, p2: &IntPoint) -> Side {
        let v1 = self.difference_by(p1);
        let v2 = p2.difference_by(p1);
        v1.side_of(&v2)
    }

    /// Turns this point by factor times 90 degree around pole.
    pub fn turn_90_degree(&self, factor: i32, pole: &IntPoint) -> IntPoint {
        let v = self.difference_by(pole);
        let v = v.turn_90_degree(factor);
        pole.translate_by(&v)
    }

    /// Mirrors this point at the vertical line through pole.
    pub fn mirror_vertical(&self, pole: &IntPoint) -> IntPoint {
        let v = self.difference_by(pole);
        let v = v.mirror_at_y_axis();
        pole.translate_by(&v)
    }

    /// Mirrors this point at the horizontal line through pole.
    pub fn mirror_horizontal(&self, pole: &IntPoint) -> IntPoint {
        let v = self.difference_by(pole);
        let v = v.mirror_at_x_axis();
        pole.translate_by(&v)
    }

    /// Converts this point to a FloatPoint.
    pub fn to_float(&self) -> FloatPoint {
        FloatPoint::new(self.x as f64, self.y as f64)
    }

    /// Returns the side of a line on which this point lies. Java `IntPoint.sideOf(Line)`.
    ///
    /// Note the direction of the answer: this is the *point's* view (`v1.sideOf(v2)` with
    /// `v1 = this - line.a`, `v2 = line.b - line.a`). `Line::side_of` negates it to get the
    /// line's view.
    pub fn side_of_line(&self, line: &crate::line::Line) -> Side {
        let v1 = self.difference_by(&line.a);
        let v2 = line.b.difference_by(&line.a);
        v1.side_of(&v2)
    }

    /// Returns the nearest point to this point on line. Java `IntPoint.perpendicularProjection`.
    ///
    /// Ported verbatim: the exact projection is
    /// `((vx*vx*px + vx*vy*py + det*vy) / D, (vx*vy*px + vy*vy*py - det*vx) / D)` with
    /// `v = line.b - line.a`, `det = line.a.determinant(line.b)` and `D = vx*vx + vy*vy`.
    /// Unlike Java's `RationalPoint` counterpart, which adds where this subtracts (see
    /// `RationalPoint::perpendicular_projection`), this is the mathematically correct formula.
    ///
    /// Java has no `CRIT_INT` guard on the demoted result here (unlike `RationalPoint`), so a
    /// projection that divides out exactly always becomes an `IntPoint`.
    pub fn perpendicular_projection(&self, line: &crate::line::Line) -> crate::point::Point {
        use num_bigint::BigInt;
        use num_integer::Integer;
        use num_traits::{Signed, ToPrimitive, Zero};

        // this function is at the moment only implemented for lines consisting of IntPoints.
        // The general implementation is still missing.
        let v = line.b.difference_by(&line.a);
        let vxvx = BigInt::from(v.x as i64 * v.x as i64);
        let vyvy = BigInt::from(v.y as i64 * v.y as i64);
        let vxvy = BigInt::from(v.x as i64 * v.y as i64);
        let mut denominator = &vxvx + &vyvy;
        let det = BigInt::from(line.a.determinant(&line.b));
        let point_x = BigInt::from(self.x);
        let point_y = BigInt::from(self.y);

        let tmp1 = &vxvx * &point_x;
        let tmp2 = &vxvy * &point_y;
        let tmp1 = tmp1 + tmp2;
        let tmp2 = &det * BigInt::from(v.y);
        let mut proj_x = tmp1 + tmp2;

        let tmp1 = &vxvy * &point_x;
        let tmp2 = &vyvy * &point_y;
        let tmp1 = tmp1 + tmp2;
        let tmp2 = &det * BigInt::from(v.x);
        let mut proj_y = tmp1 - tmp2;

        if !denominator.is_zero() {
            if denominator.is_negative() {
                denominator = -denominator;
                proj_x = -proj_x;
                proj_y = -proj_y;
            }
            if proj_x.mod_floor(&denominator).is_zero() && proj_y.mod_floor(&denominator).is_zero()
            {
                proj_x /= &denominator;
                proj_y /= &denominator;
                return crate::point::Point::Int(IntPoint::new(
                    proj_x.to_i32().expect("projection fits an i32"),
                    proj_y.to_i32().expect("projection fits an i32"),
                ));
            }
        }
        crate::point::Point::Rational(crate::rational_point::RationalPoint::new(
            proj_x,
            proj_y,
            denominator,
        ))
    }

    // not ported here: `Point.perpendicularDirection(Line)` is a concrete method of the
    // abstract `Point` class in Java, inherited (not overridden) by `IntPoint`; it lives on
    // `Point` in this port (see `point.rs`).

    /// Creates the smallest Box with integer coordinates containing this point. Java
    /// `IntPoint.surroundingBox()`.
    pub fn surrounding_box(&self) -> crate::int_box::IntBox {
        crate::int_box::IntBox::new(*self, *self)
    }

    /// Returns true, if this point lies in the interior or on the border of box. Java
    /// `IntPoint.isContainedIn(IntBox)`.
    pub fn is_contained_in(&self, box_: &crate::int_box::IntBox) -> bool {
        self.x >= box_.ll.x && self.y >= box_.ll.y && self.x <= box_.ur.x && self.y <= box_.ur.y
    }

    /// Creates the smallest `IntOctagon` containing this point. Java
    /// `IntPoint.surroundingOctagon()` (IntPoint.java:62-68): a degenerate octagon whose four
    /// diagonal bounds all collapse onto the point's own `x - y` and `x + y`.
    pub fn surrounding_octagon(&self) -> crate::int_octagon::IntOctagon {
        let tmp1 = self.x - self.y;
        let tmp2 = self.x + self.y;

        crate::int_octagon::IntOctagon::new(self.x, self.y, self.x, self.y, tmp1, tmp1, tmp2, tmp2)
    }
}

impl fmt::Display for IntPoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({},{})", self.x, self.y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Side;
    use crate::int_vector::IntVector;
    use std::cmp::Ordering;

    #[test]
    fn translate_and_difference() {
        let p = IntPoint::new(2, 3);
        assert_eq!(p.translate_by(&IntVector::new(-5, 1)), IntPoint::new(-3, 4));
        assert_eq!(IntPoint::new(7, 7).difference_by(&p), IntVector::new(5, 4));
        assert_eq!(p.to_string(), "(2,3)");
    }

    #[test]
    fn determinant_and_area() {
        assert_eq!(IntPoint::new(1, 2).determinant(&IntPoint::new(3, 4)), -2);
        // signed_area of (0,0) relative to p1=(0,0)... use a real triangle:
        // p=(0,1), p1=(0,0), p2=(1,0): d21=(1,0), d01=(0,1): det = 1*1 - 0*0 = 1
        assert_eq!(
            IntPoint::new(0, 1).signed_area(&IntPoint::new(0, 0), &IntPoint::new(1, 0)),
            1
        );
        assert_eq!(
            IntPoint::new(0, 0).distance_square(&IntPoint::new(3, 4)),
            25.0
        );
        assert_eq!(IntPoint::new(0, 0).distance(&IntPoint::new(3, 4)), 5.0);
    }

    #[test]
    fn projections() {
        // horizontal distance 2 <= vertical 5 → snap x to other
        assert_eq!(
            IntPoint::new(0, 0).orthogonal_projection(&IntPoint::new(2, 5)),
            IntPoint::new(2, 0)
        );
        assert_eq!(
            IntPoint::new(0, 0).orthogonal_projection(&IntPoint::new(5, 2)),
            IntPoint::new(0, 2)
        );
        // 45°: from (0,0) to (10,1): dx=-10, dy=-1 → distArr=[10,1,4.5,5.5] → min=dy → (this.x, other.y)
        assert_eq!(
            IntPoint::new(0, 0).fortyfive_degree_projection(&IntPoint::new(10, 1)),
            IntPoint::new(0, 1)
        );
    }

    #[test]
    fn corners() {
        // (0,0)->(10,4): dy>0 && dy<dx → left: (to.x - dy, this.y) = (6,0); right: (this.x+dy, to.y) = (4,4)
        let a = IntPoint::new(0, 0);
        let b = IntPoint::new(10, 4);
        assert_eq!(
            a.fortyfive_degree_corner(&b, true),
            Some(IntPoint::new(6, 0))
        );
        assert_eq!(
            a.fortyfive_degree_corner(&b, false),
            Some(IntPoint::new(4, 4))
        );
        assert_eq!(a.fortyfive_degree_corner(&IntPoint::new(5, 5), true), None); // already diagonal
        assert_eq!(a.ninety_degree_corner(&b, true), Some(IntPoint::new(10, 0)));
        assert_eq!(a.ninety_degree_corner(&b, false), Some(IntPoint::new(0, 4)));
        assert_eq!(a.ninety_degree_corner(&IntPoint::new(0, 9), true), None);
    }

    #[test]
    fn comparisons_and_side() {
        assert_eq!(
            IntPoint::new(1, 9).compare_xy(&IntPoint::new(2, 0)),
            Ordering::Less
        );
        assert_eq!(
            IntPoint::new(1, 9).compare_xy(&IntPoint::new(1, 0)),
            Ordering::Greater
        );
        // Point.sideOf(p1, p2): v1 = this - p1, v2 = p2 - p1, v1.sideOf(v2)
        // this=(0,1), p1=(0,0), p2=(1,0): v1=(0,1), v2=(1,0).
        // Java: IntVector(0,1).sideOf(IntVector(1,0)):
        //   determinant(v1,v2) = v1.x*v2.y - v1.y*v2.x = 0*0 - 1*1 = -1 → Side.of(-1) = ON_THE_RIGHT,
        //   then IntVector.sideOf negates the double-dispatch result → ON_THE_LEFT.
        let s = IntPoint::new(0, 1).side_of(&IntPoint::new(0, 0), &IntPoint::new(1, 0));
        assert_eq!(s, Side::OnTheLeft);
    }

    #[test]
    fn side_of_line_and_perpendicular_projection() {
        use crate::line::Line;
        use crate::point::Point;
        // IntPoint.sideOf(Line): v1 = this - line.a, v2 = line.b - line.a, then v1.sideOf(v2)
        // (which negates the determinant's Side, see `IntVector::side_of`).
        // this=(3,4), line=(0,0)->(10,0): v1=(3,4), v2=(10,0); 3*0 - 4*10 = -40 -> OnTheRight,
        // negated -> OnTheLeft. (`Line::side_of` negates once more; this is the raw point view.)
        let line = Line::from_coords(0, 0, 10, 0);
        assert_eq!(IntPoint::new(3, 4).side_of_line(&line), Side::OnTheLeft);
        assert_eq!(IntPoint::new(3, -4).side_of_line(&line), Side::OnTheRight);
        assert_eq!(IntPoint::new(3, 0).side_of_line(&line), Side::Collinear);
        // Perpendicular projection onto the right diagonal through the origin.
        let diag = Line::from_coords(0, 0, 1, 1);
        assert_eq!(
            IntPoint::new(4, 0).perpendicular_projection(&diag),
            Point::Int(IntPoint::new(2, 2))
        );
    }

    #[test]
    fn turn_and_mirror_about_pole() {
        let pole = IntPoint::new(1, 1);
        assert_eq!(
            IntPoint::new(2, 1).turn_90_degree(1, &pole),
            IntPoint::new(1, 2)
        );
        assert_eq!(
            IntPoint::new(3, 5).mirror_vertical(&pole),
            IntPoint::new(-1, 5)
        );
        assert_eq!(
            IntPoint::new(3, 5).mirror_horizontal(&pole),
            IntPoint::new(3, -3)
        );
    }
}
