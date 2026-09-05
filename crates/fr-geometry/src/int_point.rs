use std::cmp::Ordering;
use std::fmt;

use crate::float_point::FloatPoint;
use crate::int_vector::IntVector;
use crate::side::Side;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IntPoint {
    pub x: i32,
    pub y: i32,
}

impl IntPoint {
    pub const ZERO: IntPoint = IntPoint { x: 0, y: 0 };

    pub fn new(x: i32, y: i32) -> IntPoint {
        IntPoint { x, y }
    }

    pub fn get_id(&self) -> i32 {
        31i32.wrapping_mul(self.x).wrapping_add(self.y)
    }

    pub fn translate_by(&self, vector: &IntVector) -> IntPoint {
        IntPoint::new(self.x + vector.x, self.y + vector.y)
    }

    pub fn difference_by(&self, other: &IntPoint) -> IntVector {
        IntVector::new(self.x - other.x, self.y - other.y)
    }

    pub fn determinant(&self, other: &IntPoint) -> i64 {
        self.x as i64 * other.y as i64 - self.y as i64 * other.x as i64
    }

    pub fn signed_area(&self, p1: &IntPoint, p2: &IntPoint) -> i64 {
        let d21 = p2.difference_by(p1);
        let d01 = self.difference_by(p1);
        d21.determinant(&d01)
    }

    pub fn distance_square(&self, to_point: &IntPoint) -> f64 {
        let dx = (to_point.x - self.x) as f64;
        let dy = (to_point.y - self.y) as f64;
        dx * dx + dy * dy
    }

    pub fn distance(&self, to_point: &IntPoint) -> f64 {
        self.distance_square(to_point).sqrt()
    }

    pub fn orthogonal_projection(&self, other: &IntPoint) -> IntPoint {
        let horizontal_distance = (self.x - other.x).abs();
        let vertical_distance = (self.y - other.y).abs();
        if horizontal_distance <= vertical_distance {
            IntPoint::new(other.x, self.y)
        } else {
            IntPoint::new(self.x, other.y)
        }
    }

    pub fn fortyfive_degree_projection(&self, other: &IntPoint) -> IntPoint {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dist_arr = [dx.abs() as f64, dy.abs() as f64, 0.0, 0.0];
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
            IntPoint::new(other.x, self.y)
        } else if min_dist == dist_arr[1] {
            IntPoint::new(self.x, other.y)
        } else if min_dist == dist_arr[2] {
            let diagonal_value = diagonal2 as i32;
            IntPoint::new(other.x + diagonal_value, other.y + diagonal_value)
        } else {
            let diagonal_value = diagonal1 as i32;
            IntPoint::new(other.x - diagonal_value, other.y + diagonal_value)
        }
    }

    pub fn fortyfive_degree_corner(
        &self,
        to_point: &IntPoint,
        left_turn: bool,
    ) -> Option<IntPoint> {
        let dx = to_point.x - self.x;
        let dy = to_point.y - self.y;

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
            return None;
        };
        Some(result)
    }

    pub fn ninety_degree_corner(&self, to_point: &IntPoint, left_turn: bool) -> Option<IntPoint> {
        let dx = to_point.x - self.x;
        let dy = to_point.y - self.y;

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
            return None;
        };
        Some(result)
    }

    pub fn compare_x(&self, other: &IntPoint) -> Ordering {
        self.x.cmp(&other.x)
    }

    pub fn compare_y(&self, other: &IntPoint) -> Ordering {
        self.y.cmp(&other.y)
    }

    pub fn compare_xy(&self, other: &IntPoint) -> Ordering {
        match self.compare_x(other) {
            Ordering::Equal => self.compare_y(other),
            result => result,
        }
    }

    pub fn side_of(&self, p1: &IntPoint, p2: &IntPoint) -> Side {
        let v1 = self.difference_by(p1);
        let v2 = p2.difference_by(p1);
        v1.side_of(&v2)
    }

    pub fn turn_90_degree(&self, factor: i32, pole: &IntPoint) -> IntPoint {
        let v = self.difference_by(pole);
        let v = v.turn_90_degree(factor);
        pole.translate_by(&v)
    }

    pub fn mirror_vertical(&self, pole: &IntPoint) -> IntPoint {
        let v = self.difference_by(pole);
        let v = v.mirror_at_y_axis();
        pole.translate_by(&v)
    }

    pub fn mirror_horizontal(&self, pole: &IntPoint) -> IntPoint {
        let v = self.difference_by(pole);
        let v = v.mirror_at_x_axis();
        pole.translate_by(&v)
    }

    pub fn to_float(&self) -> FloatPoint {
        FloatPoint::new(self.x as f64, self.y as f64)
    }

    pub fn side_of_line(&self, line: &crate::line::Line) -> Side {
        let v1 = self.difference_by(&line.a);
        let v2 = line.b.difference_by(&line.a);
        v1.side_of(&v2)
    }

    pub fn perpendicular_projection(&self, line: &crate::line::Line) -> crate::point::Point {
        use num_bigint::BigInt;
        use num_integer::Integer;
        use num_traits::{Signed, ToPrimitive, Zero};

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

    pub fn surrounding_box(&self) -> crate::int_box::IntBox {
        crate::int_box::IntBox::new(*self, *self)
    }

    pub fn is_contained_in(&self, box_: &crate::int_box::IntBox) -> bool {
        self.x >= box_.ll.x && self.y >= box_.ll.y && self.x <= box_.ur.x && self.y <= box_.ur.y
    }

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
        assert_eq!(
            IntPoint::new(0, 0).orthogonal_projection(&IntPoint::new(2, 5)),
            IntPoint::new(2, 0)
        );
        assert_eq!(
            IntPoint::new(0, 0).orthogonal_projection(&IntPoint::new(5, 2)),
            IntPoint::new(0, 2)
        );
        assert_eq!(
            IntPoint::new(0, 0).fortyfive_degree_projection(&IntPoint::new(10, 1)),
            IntPoint::new(0, 1)
        );
    }

    #[test]
    fn corners() {
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
        assert_eq!(a.fortyfive_degree_corner(&IntPoint::new(5, 5), true), None);
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
        let s = IntPoint::new(0, 1).side_of(&IntPoint::new(0, 0), &IntPoint::new(1, 0));
        assert_eq!(s, Side::OnTheLeft);
    }

    #[test]
    fn side_of_line_and_perpendicular_projection() {
        use crate::line::Line;
        use crate::point::Point;
        let line = Line::from_coords(0, 0, 10, 0);
        assert_eq!(IntPoint::new(3, 4).side_of_line(&line), Side::OnTheLeft);
        assert_eq!(IntPoint::new(3, -4).side_of_line(&line), Side::OnTheRight);
        assert_eq!(IntPoint::new(3, 0).side_of_line(&line), Side::Collinear);
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
