use std::cmp::Ordering;
use std::hash::{Hash, Hasher};

use num_bigint::BigInt;
use num_integer::Integer;
use num_traits::{One, Signed, ToPrimitive, Zero};

use crate::float_point::FloatPoint;
use crate::int_point::IntPoint;
use crate::limits::{CRIT_INT, crit_int_big};
use crate::rational_point::RationalPoint;
use crate::side::Side;
use crate::vector::Vector;

/// A point in the plane, in whichever representation is exact for its coordinates.
#[derive(Debug, Clone)]
pub enum Point {
    /// A point with `i32` coordinates.
    Int(IntPoint),
    /// A point of the projective plane with infinite-precision coordinates.
    Rational(RationalPoint),
}

impl Point {
    pub const ZERO: Point = Point::Int(IntPoint::ZERO);

    pub fn inside_circumcircle(&self, a: &Point, b: &Point, c: &Point) -> bool {
        fn row(point: &Point) -> [BigInt; 4] {
            let (x, y, z) = match point {
                Point::Int(point) => (BigInt::from(point.x), BigInt::from(point.y), BigInt::one()),
                Point::Rational(point) => (point.x.clone(), point.y.clone(), point.z.clone()),
            };
            [&x * &z, &y * &z, &x * &x + &y * &y, &z * &z]
        }

        fn determinant(mut matrix: [[BigInt; 4]; 4]) -> BigInt {
            let mut previous = BigInt::one();
            let mut sign = 1_i8;
            for pivot_index in 0..3 {
                let Some(pivot_row) =
                    (pivot_index..4).find(|&row| !matrix[row][pivot_index].is_zero())
                else {
                    return BigInt::zero();
                };
                if pivot_row != pivot_index {
                    matrix.swap(pivot_row, pivot_index);
                    sign = -sign;
                }
                let pivot = matrix[pivot_index][pivot_index].clone();
                for row in pivot_index + 1..4 {
                    for column in pivot_index + 1..4 {
                        matrix[row][column] = (&matrix[row][column] * &pivot
                            - &matrix[row][pivot_index] * &matrix[pivot_index][column])
                            / &previous;
                    }
                }
                previous = pivot;
            }
            if sign < 0 {
                -matrix[3][3].clone()
            } else {
                matrix[3][3].clone()
            }
        }

        let value = determinant([row(a), row(b), row(c), row(self)]);
        match c.side_of(a, b) {
            Side::OnTheLeft => value.is_positive(),
            Side::OnTheRight => value.is_negative(),
            Side::Collinear => false,
        }
    }

    pub fn new(x: i32, y: i32) -> Point {
        let result = IntPoint::new(x, y);
        if x.wrapping_abs() > CRIT_INT || y.wrapping_abs() > CRIT_INT {
            Point::Rational(RationalPoint::from_int(&result))
        } else {
            Point::Int(result)
        }
    }

    pub fn from_big(x: BigInt, y: BigInt, z: BigInt) -> Point {
        let (mut x, mut y, mut z) = if z.is_negative() {
            // the dominator z of a RationalPoint is expected to be positive
            (-x, -y, -z)
        } else {
            (x, y, z)
        };
        if x.mod_floor(&z).is_zero() {
            // x and y can be divided by z
            x = &x / &z;
            y = &y / &z;
            z = BigInt::one();
        }
        if z.is_one() {
            let crit = crit_int_big();
            if x.abs() <= crit && y.abs() <= crit {
                // the Point fits into an IntPoint
                return Point::Int(IntPoint::new(
                    x.to_i32().expect("|x| <= CRIT_INT"),
                    y.to_i32().expect("|y| <= CRIT_INT"),
                ));
            }
        }
        Point::Rational(RationalPoint::new(x, y, z))
    }

    /// Returns true, if this Point is a RationalPoint with denominator z = 0.
    pub fn is_infinite(&self) -> bool {
        match self {
            Point::Int(_) => false,
            Point::Rational(p) => p.is_infinite(),
        }
    }

    /// Returns the translation of this point by vector.
    pub fn translate_by(&self, vector: &Vector) -> Point {
        if *vector == Vector::ZERO {
            return self.clone();
        }
        vector.add_to(self)
    }

    /// Returns the difference vector of this point and other.
    pub fn difference_by(&self, other: &Point) -> Vector {
        match (self, other) {
            (Point::Int(a), Point::Int(b)) => Vector::Int(a.difference_by(b)),
            // IntPoint.differenceBy(Point) -> RationalPoint.differenceBy(IntPoint), negated
            (Point::Int(a), Point::Rational(b)) => {
                Vector::Rational(b.difference_by_int(a).negate())
            }
            // RationalPoint.differenceBy(Point) -> IntPoint.differenceBy(RationalPoint) -> negated
            // twice, i.e. the plain rational overload
            (Point::Rational(a), Point::Int(b)) => Vector::Rational(a.difference_by_int(b)),
            // RationalPoint.differenceBy(Point) -> other.differenceBy(this), negated
            (Point::Rational(a), Point::Rational(b)) => {
                Vector::Rational(b.difference_by_rational(a).negate())
            }
        }
    }

    /// Returns `Ordering::Greater`, if this Point has a strict bigger x coordinate than other,
    /// `Ordering::Equal`, if the x coordinates are equal, and `Ordering::Less` otherwise.
    pub fn compare_x(&self, other: &Point) -> Ordering {
        match (self, other) {
            (Point::Int(a), Point::Int(b)) => a.compare_x(b),
            // IntPoint.compareX(Point) -> -other.compareX(this)
            (Point::Int(a), Point::Rational(b)) => b.compare_x_int(a).reverse(),
            // RationalPoint.compareX(Point) -> IntPoint.compareX(RationalPoint) -> negated twice
            (Point::Rational(a), Point::Int(b)) => a.compare_x_int(b),
            // RationalPoint.compareX(Point) -> -other.compareX(this), which is symmetric to
            // this.compareX(other) because the rational overload cross-multiplies
            (Point::Rational(a), Point::Rational(b)) => a.compare_x_rational(b),
        }
    }

    /// Returns `Ordering::Greater`, if this Point has a strict bigger y coordinate than other,
    /// `Ordering::Equal`, if the y coordinates are equal, and `Ordering::Less` otherwise.
    pub fn compare_y(&self, other: &Point) -> Ordering {
        match (self, other) {
            (Point::Int(a), Point::Int(b)) => a.compare_y(b),
            (Point::Int(a), Point::Rational(b)) => b.compare_y_int(a).reverse(),
            (Point::Rational(a), Point::Int(b)) => a.compare_y_int(b),
            (Point::Rational(a), Point::Rational(b)) => a.compare_y_rational(b),
        }
    }

    /// Returns `compare_x(other)`, if the result is not `Ordering::Equal`. Otherwise, it returns
    /// `compare_y(other)`.
    pub fn compare_xy(&self, other: &Point) -> Ordering {
        match self.compare_x(other) {
            Ordering::Equal => self.compare_y(other),
            result => result,
        }
    }

    /// The function returns `Side::OnTheLeft`, if this Point is on the left of the line from p1 to
    /// p2; `Side::OnTheRight`, if this Point is on the right of the line from p1 to p2; and
    /// `Side::Collinear`, if this Point is collinear with p1 and p2.
    pub fn side_of(&self, p1: &Point, p2: &Point) -> Side {
        let v1 = self.difference_by(p1);
        let v2 = p2.difference_by(p1);
        v1.side_of(&v2)
    }

    /// Turns this point by factor times 90 degree around pole.
    pub fn turn_90_degree(&self, factor: i32, pole: &Point) -> Point {
        let v = self.difference_by(pole);
        let v = v.turn_90_degree(factor);
        pole.translate_by(&v)
    }

    /// Mirrors this point at the vertical line through pole.
    pub fn mirror_vertical(&self, pole: &Point) -> Point {
        let v = self.difference_by(pole);
        let v = v.mirror_at_y_axis();
        pole.translate_by(&v)
    }

    /// Mirrors this point at the horizontal line through pole.
    pub fn mirror_horizontal(&self, pole: &Point) -> Point {
        let v = self.difference_by(pole);
        let v = v.mirror_at_x_axis();
        pole.translate_by(&v)
    }

    pub fn get_id(&self) -> i32 {
        match self {
            Point::Int(p) => p.get_id(),
            Point::Rational(p) => p.get_id(),
        }
    }

    pub fn to_float(&self) -> FloatPoint {
        match self {
            Point::Int(p) => p.to_float(),
            Point::Rational(p) => p.to_float(),
        }
    }

    pub fn side_of_line(&self, line: &crate::line::Line) -> Side {
        match self {
            Point::Int(p) => p.side_of_line(line),
            Point::Rational(p) => p.side_of_line(line),
        }
    }

    pub fn perpendicular_projection(&self, line: &crate::line::Line) -> Point {
        match self {
            Point::Int(p) => p.perpendicular_projection(line),
            Point::Rational(p) => p.perpendicular_projection(line),
        }
    }

    pub fn perpendicular_direction(&self, line: &crate::line::Line) -> crate::direction::Direction {
        let side = self.side_of_line(line);
        if side == Side::Collinear {
            return crate::direction::Direction::Int(crate::int_direction::IntDirection::NULL);
        }
        if side == Side::OnTheRight {
            crate::direction::Direction::Int(line.direction().turn_45_degree(2))
        } else {
            crate::direction::Direction::Int(line.direction().turn_45_degree(6))
        }
    }

    pub fn surrounding_box(&self) -> crate::int_box::IntBox {
        match self {
            Point::Int(p) => p.surrounding_box(),
            Point::Rational(p) => p.surrounding_box(),
        }
    }

    pub fn is_contained_in(&self, box_: &crate::int_box::IntBox) -> bool {
        match self {
            Point::Int(p) => p.is_contained_in(box_),
            Point::Rational(p) => p.is_contained_in(box_),
        }
    }

    pub fn surrounding_octagon(&self) -> crate::int_octagon::IntOctagon {
        match self {
            Point::Int(p) => p.surrounding_octagon(),
            Point::Rational(p) => p.surrounding_octagon(),
        }
    }
}

impl From<IntPoint> for Point {
    fn from(p: IntPoint) -> Point {
        Point::Int(p)
    }
}

impl From<RationalPoint> for Point {
    fn from(p: RationalPoint) -> Point {
        Point::Rational(p)
    }
}

impl PartialEq for Point {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Point::Int(a), Point::Int(b)) => a == b,
            (Point::Rational(a), Point::Rational(b)) => a == b,
            _ => false,
        }
    }
}

impl Eq for Point {}

impl Hash for Point {
    /// The representation is part of the identity (see `PartialEq`), so it is hashed too.
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Point::Int(p) => {
                0u8.hash(state);
                p.hash(state);
            }
            Point::Rational(p) => {
                1u8.hash(state);
                p.hash(state);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CRIT_INT;
    use crate::int_point::IntPoint;
    use crate::int_vector::IntVector;
    use crate::rational_vector::RationalVector;
    use crate::vector::Vector;
    use num_bigint::BigInt;

    #[test]
    fn new_promotes_beyond_crit_int() {
        assert!(matches!(Point::new(5, 5), Point::Int(_)));
        assert!(matches!(Point::new(CRIT_INT + 1, 0), Point::Rational(_)));
        assert!(matches!(Point::new(CRIT_INT, 0), Point::Int(_)));
    }

    #[test]
    fn circumcircle_membership_is_exact_and_orientation_independent() {
        let a = Point::new(0, 0);
        let b = Point::new(CRIT_INT, 0);
        let c = Point::new(0, CRIT_INT);
        let inside = Point::new(1, 1);
        let boundary = Point::new(CRIT_INT, CRIT_INT);
        let outside = Point::new(CRIT_INT * 2, CRIT_INT * 2);

        assert!(inside.inside_circumcircle(&a, &b, &c));
        assert!(inside.inside_circumcircle(&a, &c, &b));
        assert!(!boundary.inside_circumcircle(&a, &b, &c));
        assert!(!outside.inside_circumcircle(&a, &b, &c));

        let rational_inside = Point::from_big(BigInt::from(1), BigInt::from(1), BigInt::from(2));
        assert!(rational_inside.inside_circumcircle(&a, &b, &c));
    }

    #[test]
    fn from_big_reduces_and_demotes() {
        let p = Point::from_big(BigInt::from(100), BigInt::from(200), BigInt::from(50));
        assert_eq!(p, Point::Int(IntPoint::new(2, 4)));
        let p = Point::from_big(BigInt::from(-6), BigInt::from(9), BigInt::from(-3));
        assert_eq!(p, Point::Int(IntPoint::new(2, -3)));
        let p = Point::from_big(BigInt::from(3), BigInt::from(3), BigInt::from(2));
        assert!(matches!(p, Point::Rational(_)));
    }

    #[test]
    fn rational_equality_is_by_value() {
        let a = Point::Rational(RationalPoint::new(
            BigInt::from(100),
            BigInt::from(200),
            BigInt::from(50),
        ));
        let b = Point::Rational(RationalPoint::new(
            BigInt::from(2),
            BigInt::from(4),
            BigInt::from(1),
        ));
        assert_eq!(a, b);
        let inf1 = RationalPoint::new(BigInt::from(10), BigInt::from(20), BigInt::from(0));
        let inf2 = RationalPoint::new(BigInt::from(30), BigInt::from(40), BigInt::from(0));
        assert_eq!(inf1, inf2);
        assert!(inf1.is_infinite());
    }

    #[test]
    fn translate_and_difference_mix_representations() {
        let half = Point::from_big(BigInt::from(1), BigInt::from(1), BigInt::from(2)); // (0.5, 0.5)
        let moved = half.translate_by(&Vector::Int(IntVector::new(1, 1)));
        assert_eq!(
            moved,
            Point::from_big(BigInt::from(3), BigInt::from(3), BigInt::from(2))
        );
        let d = moved.difference_by(&half);
        assert!(matches!(d, Vector::Rational(_)));
        assert_eq!(
            d,
            Vector::Rational(RationalVector::new(
                BigInt::from(2),
                BigInt::from(2),
                BigInt::from(2)
            ))
        );
        let big = Point::new(CRIT_INT + 1, 0);
        assert_eq!(
            big.compare_x(&Point::new(CRIT_INT, 0)),
            std::cmp::Ordering::Greater
        );
    }
}

#[cfg(test)]
mod cross_representation_tests {
    use super::*;
    use crate::int_vector::IntVector;
    use crate::rational_vector::RationalVector;
    use num_bigint::BigInt;

    fn b(v: i64) -> BigInt {
        BigInt::from(v)
    }

    fn rat(p: &IntPoint) -> Point {
        Point::Rational(RationalPoint::from_int(p))
    }

    fn as_rational_point(p: &Point) -> RationalPoint {
        match p {
            Point::Int(q) => RationalPoint::from_int(q),
            Point::Rational(q) => q.clone(),
        }
    }

    fn as_rational_vector(v: &Vector) -> RationalVector {
        match v {
            Vector::Int(w) => RationalVector::from_int(w),
            Vector::Rational(w) => w.clone(),
        }
    }

    const SAMPLES: [IntPoint; 5] = [
        IntPoint { x: 0, y: 0 },
        IntPoint { x: 3, y: -4 },
        IntPoint { x: -6, y: 6 },
        IntPoint { x: 3, y: 9 },
        IntPoint { x: -2, y: -7 },
    ];

    /// `Point.surroundingOctagon()` dispatches to `IntPoint`/`RationalPoint`; an integral
    /// rational point must round to exactly the same degenerate octagon as the `IntPoint`
    /// (`floor == ceil` on every one of `RationalPoint.surroundingOctagon`'s eight bounds).
    #[test]
    fn surrounding_octagon_dispatches_and_agrees_on_integral_points() {
        for p in &SAMPLES {
            let expected = p.surrounding_octagon();
            assert_eq!(Point::Int(*p).surrounding_octagon(), expected, "int {p}");
            assert_eq!(rat(p).surrounding_octagon(), expected, "rational {p}");
        }
        // A genuinely fractional point: 7/2, 3/2 -> x-y = 2, x+y = 5
        let half = RationalPoint::new(b(7), b(3), b(2));
        assert_eq!(
            half.surrounding_octagon(),
            crate::int_octagon::IntOctagon::new(3, 1, 4, 2, 2, 2, 5, 5)
        );
    }

    #[test]
    fn representations_agree_on_binary_operations() {
        for a in &SAMPLES {
            for c in &SAMPLES {
                let (ai, ci) = (Point::Int(*a), Point::Int(*c));
                let (ar, cr) = (rat(a), rat(c));

                for (lhs, rhs) in [(&ai, &cr), (&ar, &ci), (&ar, &cr)] {
                    assert_eq!(lhs.compare_x(rhs), ai.compare_x(&ci), "compare_x {a} {c}");
                    assert_eq!(lhs.compare_y(rhs), ai.compare_y(&ci), "compare_y {a} {c}");
                    assert_eq!(
                        lhs.compare_xy(rhs),
                        ai.compare_xy(&ci),
                        "compare_xy {a} {c}"
                    );
                    assert_eq!(
                        as_rational_vector(&lhs.difference_by(rhs)),
                        as_rational_vector(&ai.difference_by(&ci)),
                        "difference_by {a} {c}"
                    );
                }
            }
        }
    }

    #[test]
    fn representations_agree_on_translate_and_side_of() {
        let v = IntVector::new(2, -3);
        let vr = Vector::Rational(RationalVector::from_int(&v));
        for p in &SAMPLES {
            let expected = as_rational_point(&Point::Int(*p).translate_by(&Vector::Int(v)));
            for (point, vector) in [
                (Point::Int(*p), &vr),
                (rat(p), &Vector::Int(v)),
                (rat(p), &vr),
            ] {
                assert_eq!(
                    as_rational_point(&point.translate_by(vector)),
                    expected,
                    "translate_by {p}"
                );
            }
        }
        // side_of / turn_90_degree / mirrors go through difference_by + translate_by
        let (p1, p2) = (
            Point::Int(IntPoint::new(0, 0)),
            Point::Int(IntPoint::new(1, 0)),
        );
        let (p1r, p2r) = (rat(&IntPoint::new(0, 0)), rat(&IntPoint::new(1, 0)));
        let up = Point::Int(IntPoint::new(0, 1));
        assert_eq!(
            up.side_of(&p1, &p2),
            rat(&IntPoint::new(0, 1)).side_of(&p1r, &p2r)
        );
        assert_eq!(
            as_rational_point(&up.turn_90_degree(1, &p2)),
            as_rational_point(&rat(&IntPoint::new(0, 1)).turn_90_degree(1, &p2r))
        );
        assert_eq!(
            as_rational_point(&up.mirror_vertical(&p2)),
            as_rational_point(&rat(&IntPoint::new(0, 1)).mirror_vertical(&p2r))
        );
        assert_eq!(
            as_rational_point(&up.mirror_horizontal(&p2)),
            as_rational_point(&rat(&IntPoint::new(0, 1)).mirror_horizontal(&p2r))
        );
    }

    #[test]
    fn from_big_only_tests_x_for_divisibility() {
        assert_eq!(
            Point::from_big(b(6), b(5), b(2)),
            Point::Int(IntPoint::new(3, 2))
        );
    }

    #[test]
    fn equality_is_representation_sensitive_and_hash_agrees() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        fn hash_of(p: &Point) -> u64 {
            let mut h = DefaultHasher::new();
            p.hash(&mut h);
            h.finish()
        }

        let int_point = Point::Int(IntPoint::new(2, 4));
        let rational_point = Point::from_big(b(2), b(4), b(1));
        assert_eq!(rational_point, int_point); // from_big demoted it back to an IntPoint
        assert_ne!(
            Point::Rational(RationalPoint::new(b(2), b(4), b(1))),
            int_point
        );

        // proportional triples are equal and hash the same
        let a = Point::Rational(RationalPoint::new(b(100), b(200), b(50)));
        let c = Point::Rational(RationalPoint::new(b(2), b(4), b(1)));
        assert_eq!(a, c);
        assert_eq!(hash_of(&a), hash_of(&c));

        // all points at infinity are equal, and all hash alike
        let inf1 = Point::Rational(RationalPoint::new(b(10), b(20), b(0)));
        let inf2 = Point::Rational(RationalPoint::new(b(30), b(40), b(0)));
        assert_eq!(inf1, inf2);
        assert_eq!(hash_of(&inf1), hash_of(&inf2));
        assert!(inf1.is_infinite());
        assert!(!int_point.is_infinite());
        assert!(!a.is_infinite());
    }

    #[test]
    #[should_panic(expected = "RationalPoint: z is expected to be >= 0")]
    fn rational_point_rejects_a_negative_denominator() {
        let _ = RationalPoint::new(b(1), b(1), b(-1));
    }

    #[test]
    fn side_of_line_perpendicular_projection_and_direction() {
        use crate::direction::Direction;
        use crate::int_direction::IntDirection;
        use crate::line::Line;
        use crate::side::Side;

        let line = Line::from_coords(0, 0, 10, 0);
        let above = Point::Int(IntPoint::new(3, 4));
        let above_rational = Point::Rational(RationalPoint::new(b(3), b(4), b(1)));
        // Both representations take the same `Point.sideOf(p1, p2)` route and must agree.
        assert_eq!(above.side_of_line(&line), Side::OnTheLeft);
        assert_eq!(above_rational.side_of_line(&line), Side::OnTheLeft);
        assert_eq!(
            Point::Int(IntPoint::new(3, 0)).side_of_line(&line),
            Side::Collinear
        );

        let diag = Line::from_coords(0, 0, 1, 1);
        assert_eq!(
            Point::Int(IntPoint::new(4, 0)).perpendicular_projection(&diag),
            Point::Int(IntPoint::new(2, 2))
        );

        // Point.perpendicularDirection(Line): ON_THE_LEFT -> direction().turn45Degree(6).
        assert_eq!(
            above.perpendicular_direction(&line),
            Direction::Int(IntDirection::DOWN)
        );
        assert_eq!(
            Point::Int(IntPoint::new(3, -4)).perpendicular_direction(&line),
            Direction::Int(IntDirection::UP)
        );
        assert_eq!(
            Point::Int(IntPoint::new(3, 0)).perpendicular_direction(&line),
            Direction::Int(IntDirection::NULL)
        );
    }

    #[test]
    fn rational_perpendicular_projection_agrees_with_the_int_point_one() {
        use crate::line::Line;
        // Line y = x + 1 through (0,1) and (1,2): v = (1,1), det = a.determinant(b) = -1, D = 2.
        let line = Line::from_coords(0, 1, 1, 2);
        let point = IntPoint::new(2, 0);
        // The projection of (2,0) onto y = x + 1 is (0.5, 1.5) = (1, 3, 2) — and (0.5, 1.5) does
        // lie on the line, which the old rational answer (0.5, 0.5) did not.
        assert_eq!(
            Point::Int(point).perpendicular_projection(&line),
            Point::Rational(RationalPoint::new(b(1), b(3), b(2)))
        );
        assert_eq!(
            Point::Rational(RationalPoint::new(b(2), b(0), b(1))).perpendicular_projection(&line),
            Point::Rational(RationalPoint::new(b(1), b(3), b(2))),
            "(1, 1, 2), i.e. (0.5, 0.5), would be off the line"
        );
    }
}
