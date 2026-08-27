//! Port of `app.freerouting.geometry.planar.IntPoint`: an implementation of the (abstract, in
//! Java) `Point` class via a tuple of `i32` coordinates. Also ports the `IntPoint`-relevant
//! default methods of `Point.java` (`sideOf(p1,p2)`, `compareXY`, `turn90Degree`,
//! `mirrorVertical`/`mirrorHorizontal`).

use std::cmp::Ordering;
use std::fmt;

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
    // not ported: `getId` — tie-breaking id unused outside geometry/planar.

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

    // added in Task 9: to_float
    // added in Task 10: side_of_line, perpendicular_projection, perpendicular_direction
    // added in Task 11: surrounding_box, is_contained_in
    // added in Task 12: surrounding_octagon
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
