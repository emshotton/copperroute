use std::cmp::Ordering;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

use num_bigint::BigInt;
use num_integer::Integer;
use num_traits::{One, Signed, ToPrimitive, Zero};

use crate::direction::Direction;
use crate::float_point::FloatPoint;
use crate::int_direction::IntDirection;
use crate::int_point::IntPoint;
use crate::int_vector::IntVector;
use crate::limits::CRIT_INT;
use crate::point::Point;
use crate::rational_point::RationalPoint;
use crate::rational_vector::big_to_f64;
use crate::side::Side;
use crate::signum::Signum;
use crate::vector::Vector;

#[derive(Clone, Copy)]
pub struct Line {
    pub a: IntPoint,
    pub b: IntPoint,
    identity: u64,
}

static LINE_IDENTITY: AtomicU64 = AtomicU64::new(1);

fn next_line_identity() -> u64 {
    LINE_IDENTITY.fetch_add(1, AtomicOrdering::Relaxed)
}

impl PartialEq for Line {
    fn eq(&self, other: &Line) -> bool {
        self.a == other.a && self.b == other.b
    }
}

impl Eq for Line {}

impl std::hash::Hash for Line {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.a.hash(state);
        self.b.hash(state);
    }
}

impl std::fmt::Debug for Line {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Line")
            .field("a", &self.a)
            .field("b", &self.b)
            .finish()
    }
}

impl Line {
    pub fn is_same_object(&self, other: &Line) -> bool {
        self.identity == other.identity
    }

    pub fn new(a: IntPoint, b: IntPoint) -> Line {
        Line {
            a,
            b,
            identity: next_line_identity(),
        }
    }

    pub fn from_coords(ax: i32, ay: i32, bx: i32, by: i32) -> Line {
        Line::new(IntPoint::new(ax, ay), IntPoint::new(bx, by))
    }

    pub fn from_direction(a: IntPoint, dir: &IntDirection) -> Line {
        Line::new(a, a.translate_by(&dir.get_vector()))
    }

    pub fn from_direction_any(a: IntPoint, dir: &Direction) -> Option<Line> {
        match dir {
            Direction::Int(d) => Some(Line::from_direction(a, d)),
            Direction::Big(_) => None,
        }
    }

    pub fn direction(&self) -> IntDirection {
        let d = self.b.difference_by(&self.a);
        d.to_normalized_direction()
    }

    pub fn side_of(&self, point: &Point) -> Side {
        point.side_of_line(self).negate()
    }

    fn side_of_int_point(&self, point: &IntPoint) -> Side {
        point.side_of_line(self).negate()
    }

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

    pub fn side_of_float_exact(&self, point: &FloatPoint) -> Side {
        self.side_of_float(point, 0.0)
    }

    pub fn side_of_intersection(&self, p1: &Line, p2: &Line) -> Side {
        let intersection_approx = p1.intersection_approx(p2);
        let result = self.side_of_float(&intersection_approx, 1.0);
        if result == Side::Collinear {
            let intersection = p1.intersection(p2);
            return self.side_of(&intersection);
        }
        result
    }

    pub fn signed_distance(&self, point: &FloatPoint) -> f64 {
        let dx = (self.b.x - self.a.x) as f64;
        let dy = (self.b.y - self.a.y) as f64;
        let det = dy * (point.x - self.a.x as f64) - dx * (point.y - self.a.y as f64);
        let length = (dx * dx + dy * dy).sqrt();
        det / length
    }

    pub fn overlaps(&self, other: &Line) -> bool {
        self.side_of_int_point(&other.a) == Side::Collinear
            && self.side_of_int_point(&other.b) == Side::Collinear
    }

    pub fn opposite(&self) -> Line {
        Line::new(self.b, self.a)
    }

    pub fn intersection(&self, other: &Line) -> Point {
        let delta1 = self.b.difference_by(&self.a);
        let delta2 = other.b.difference_by(&other.a);
        if delta1.x == 0 {
            if delta2.y == 0 {
                return Point::Int(IntPoint::new(self.a.x, other.a.y));
            }
            if delta2.x == delta2.y {
                let this_x = self.a.x;
                return Point::Int(IntPoint::new(this_x, other.a.y + this_x - other.a.x));
            }
            if delta2.x == -delta2.y {
                let this_x = self.a.x;
                return Point::Int(IntPoint::new(this_x, other.a.y + other.a.x - this_x));
            }
        } else if delta1.y == 0 {
            if delta2.x == 0 {
                return Point::Int(IntPoint::new(other.a.x, self.a.y));
            }
            if delta2.x == delta2.y {
                let this_y_coordinate = self.a.y;
                return Point::Int(IntPoint::new(
                    other.a.x + this_y_coordinate - other.a.y,
                    this_y_coordinate,
                ));
            }
            if delta2.x == -delta2.y {
                let this_y_coordinate = self.a.y;
                return Point::Int(IntPoint::new(
                    other.a.x + other.a.y - this_y_coordinate,
                    this_y_coordinate,
                ));
            }
        } else if delta1.x == delta1.y {
            if delta2.x == 0 {
                let other_x = other.a.x;
                return Point::Int(IntPoint::new(other_x, self.a.y + other_x - self.a.x));
            }
            if delta2.y == 0 {
                let other_y = other.a.y;
                return Point::Int(IntPoint::new(self.a.x + other_y - self.a.y, other_y));
            }
        } else if delta1.x == -delta1.y {
            if delta2.x == 0 {
                let other_x = other.a.x;
                return Point::Int(IntPoint::new(other_x, self.a.y + self.a.x - other_x));
            }
            if delta2.y == 0 {
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

    pub fn intersection_approx(&self, other: &Line) -> FloatPoint {
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

    pub fn perpendicular_projection(&self, point: &Point) -> Point {
        point.perpendicular_projection(self)
    }

    pub fn translate(&self, dist: f64) -> Line {
        let ai = self.a;
        let direction = self.direction();
        let v = direction.get_vector();
        let vxvx = v.x as f64 * v.x as f64;
        let vyvy = v.y as f64 * v.y as f64;
        let length = (vxvx + vyvy).sqrt();
        let new_a = if vxvx <= vyvy {
            let rel_x = ((dist * length) / v.y as f64).round() as i64 as i32;
            IntPoint::new(ai.x - rel_x, ai.y)
        } else {
            let rel_y = ((dist * length) / v.x as f64).round() as i64 as i32;
            IntPoint::new(ai.x, ai.y + rel_y)
        };
        Line::from_direction(new_a, &direction)
    }

    pub fn translate_by(&self, vector: &IntVector) -> Line {
        if *vector == IntVector::ZERO {
            return *self;
        }
        Line::new(self.a.translate_by(vector), self.b.translate_by(vector))
    }

    pub fn translate_by_any(&self, vector: &Vector) -> Option<Line> {
        match vector {
            Vector::Int(v) => Some(self.translate_by(v)),
            Vector::Rational(_) => None,
        }
    }

    pub fn is_orthogonal(&self) -> bool {
        self.direction().is_orthogonal()
    }

    pub fn is_diagonal(&self) -> bool {
        self.direction().is_diagonal()
    }

    pub fn is_multiple_of_45_degree(&self) -> bool {
        self.direction().is_multiple_of_45_degree()
    }

    pub fn is_parallel(&self, other: &Line) -> bool {
        self.direction().side_of(&other.direction()) == Side::Collinear
    }

    pub fn is_perpendicular(&self, other: &Line) -> bool {
        let v1 = self.direction().get_vector();
        let v2 = other.direction().get_vector();
        v1.projection(&v2) == Signum::Zero
    }

    pub fn is_equal_or_opposite(&self, other: &Line) -> bool {
        self.side_of_int_point(&other.a) == Side::Collinear
            && self.side_of_int_point(&other.b) == Side::Collinear
    }

    pub fn cos_angle(&self, other: &Line) -> f64 {
        let v1 = self.b.difference_by(&self.a);
        let v2 = other.b.difference_by(&other.a);
        v1.cos_angle(&v2)
    }

    pub fn compare_to(&self, other: &Line) -> Ordering {
        let d1 = self.b.difference_by(&self.a);
        let d2 = other.b.difference_by(&other.a);
        IntDirection::new(d2.x, d2.y)
            .compare_to(&IntDirection::new(d1.x, d1.y))
            .reverse()
    }

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

    pub fn turn_90_degree(&self, factor: i32, pole: &IntPoint) -> Line {
        Line::new(
            self.a.turn_90_degree(factor, pole),
            self.b.turn_90_degree(factor, pole),
        )
    }

    pub fn mirror_vertical(&self, pole: &IntPoint) -> Line {
        Line::new(self.b.mirror_vertical(pole), self.a.mirror_vertical(pole))
    }

    pub fn mirror_horizontal(&self, pole: &IntPoint) -> Line {
        Line::new(
            self.b.mirror_horizontal(pole),
            self.a.mirror_horizontal(pole),
        )
    }

    pub fn length(&self) -> f32 {
        let dx = self.b.x.wrapping_sub(self.a.x);
        let dy = self.b.y.wrapping_sub(self.a.y);
        let sum = dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy));
        (sum as f64).sqrt() as f32
    }

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

    pub fn get_id(&self) -> i32 {
        31i32
            .wrapping_mul(self.a.get_id())
            .wrapping_add(self.b.get_id())
    }
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
        assert!(l(0, 0, 1, 1).intersection(&l(0, 1, 1, 2)).is_infinite());
    }

    #[test]
    fn side_of_and_direction() {
        let line = l(0, 0, 10, 0);
        assert_eq!(line.direction(), IntDirection::RIGHT);
        let above = Point::Int(IntPoint::new(3, 4));
        let below = Point::Int(IntPoint::new(3, -4));
        assert_eq!(line.side_of(&above), Side::OnTheRight);
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
        let line = l(0, 0, 2, 4);
        assert_eq!(line.function_value_approx(3.0), 6.0);
        assert_eq!(line.function_in_y_value_approx(6.0), 3.0);
    }

    #[test]
    fn intersection_keeps_a_rational_point_when_the_result_exceeds_crit_int() {
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
        let parallel = l(0, 0, 1, 1).intersection_approx(&l(0, 1, 1, 2));
        assert_eq!(parallel, FloatPoint::new(i32::MAX as f64, i32::MAX as f64));
    }

    #[test]
    fn side_of_float_and_signed_distance() {
        let line = l(0, 0, 10, 0);
        let above = FloatPoint::new(3.0, 4.0);
        assert_eq!(line.side_of_float(&above, 0.0), Side::OnTheRight);
        assert_eq!(line.side_of_float_exact(&above), Side::OnTheRight);
        assert_eq!(line.side_of_float(&above, 100.0), Side::Collinear);
        assert_eq!(line.signed_distance(&above), -4.0);
    }

    #[test]
    fn side_of_intersection_falls_back_to_the_exact_check() {
        let p1 = l(0, 0, 1, 1);
        let p2 = l(0, 10, 1, 9);
        assert_eq!(
            l(0, 5, 1, 5).side_of_intersection(&p1, &p2),
            Side::Collinear
        );
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
        assert_eq!(
            line.translate(5.0),
            Line::new(IntPoint::new(0, 5), IntPoint::new(1, 5))
        );
        assert_eq!(
            line.translate_by(&crate::IntVector::new(1, 2)),
            Line::new(IntPoint::new(1, 2), IntPoint::new(11, 2))
        );
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
        let big = Direction::Big(crate::BigIntDirection::new(
            BigInt::from(crate::CRIT_INT + 5),
            BigInt::from(crate::CRIT_INT + 7),
        ));
        assert_eq!(Line::from_direction_any(a, &big), None);
    }

    #[test]
    fn compare_to_is_the_counterclockwise_angular_order() {
        let ccw = [
            l(0, 0, 1, 0),
            l(0, 0, 1, 1),
            l(0, 0, 0, 1),
            l(0, 0, -1, 1),
            l(0, 0, -1, 0),
            l(0, 0, -1, -1),
            l(0, 0, 0, -1),
            l(0, 0, 1, -1),
        ];
        for i in 0..ccw.len() {
            for j in 0..ccw.len() {
                let expected = i.cmp(&j);
                assert_eq!(ccw[i].compare_to(&ccw[j]), expected, "{i} vs {j}");
            }
        }
        let a = l(0, 0, 1, 0);
        let b = l(5, 5, 9, 5);
        assert_eq!(a.compare_to(&b), std::cmp::Ordering::Equal);
        assert_ne!(a, b);
    }

    #[test]
    fn compare_to_is_not_antisymmetric_at_the_degenerate_line() {
        let degenerate = l(0, 0, 0, 0);
        let right = l(0, 0, 1, 0);
        assert_eq!(degenerate.compare_to(&right), std::cmp::Ordering::Greater);
        assert_eq!(right.compare_to(&degenerate), std::cmp::Ordering::Equal);
    }

    #[test]
    fn equals_geometric_and_fast_equals() {
        let base = l(0, 0, 10, 0);
        let same_line_other_points = l(-7, 0, 3, 0);
        assert_ne!(base, same_line_other_points);
        assert!(base.equals_geometric(&same_line_other_points));
        assert!(base.fast_equals(&same_line_other_points));
        let opposite = base.opposite();
        assert!(!base.equals_geometric(&opposite));
        assert!(!base.fast_equals(&opposite));
        let parallel = l(0, 3, 10, 3);
        assert!(!base.equals_geometric(&parallel));
        assert!(!base.fast_equals(&parallel));
        assert!(!base.equals_geometric(&l(0, 0, 0, 10)));
        assert!(!base.fast_equals(&l(0, 0, 0, 10)));
        let degenerate = l(4, 4, 4, 4);
        assert!(!degenerate.equals_geometric(&degenerate));
    }

    #[test]
    fn get_id_is_the_java_hash_and_wraps() {
        assert_eq!(l(1, 2, 3, 4).get_id(), 1120);
        assert_eq!(IntPoint::new(1, 2).get_id(), 33);
        let _ = l(i32::MAX, i32::MAX, i32::MIN, i32::MIN).get_id();
    }

    #[test]
    fn perpendicular_direction_from_a_point() {
        let line = l(0, 0, 10, 0);
        let above = Point::Int(IntPoint::new(3, 4));
        assert_eq!(
            line.perpendicular_direction(&above),
            Some(Direction::Int(IntDirection::DOWN))
        );
        assert_eq!(
            line.perpendicular_direction(&Point::Int(IntPoint::new(3, 1))),
            Some(Direction::Int(IntDirection::DOWN))
        );
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

    #[test]
    fn is_same_object_is_java_reference_identity_and_not_value_equality() {
        let line = l(0, 0, 10, 0);
        let copy = line;
        let rebuilt = l(0, 0, 10, 0);

        assert!(line.is_same_object(&copy));
        assert!(!line.is_same_object(&rebuilt));
        assert_eq!(line, rebuilt);
        assert_eq!(line, copy);

        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let hash = |x: &Line| {
            let mut h = DefaultHasher::new();
            x.hash(&mut h);
            h.finish()
        };
        assert_eq!(hash(&line), hash(&rebuilt));

        let arr = [line, rebuilt];
        assert!(arr[0].is_same_object(&line));
        assert!(arr[1].is_same_object(&rebuilt));
        assert!(!arr[1].is_same_object(&line));

        assert!(!line.opposite().is_same_object(&line));
    }

    #[test]
    fn debug_does_not_show_the_identity_token() {
        let rendered = format!("{:?}", l(1, 2, 3, 4));
        assert_eq!(rendered, format!("{:?}", l(1, 2, 3, 4)));
        assert!(!rendered.contains("identity"), "{rendered}");
    }
}
