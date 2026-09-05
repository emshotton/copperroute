use num_bigint::BigInt;
use num_integer::Integer;
use num_traits::{One, Signed, ToPrimitive, Zero};

use crate::direction::Direction;
use crate::float_point::FloatPoint;
use crate::int_point::IntPoint;
use crate::int_vector::IntVector;
use crate::limits::{CRIT_INT, crit_int_big};
use crate::point::Point;
use crate::rational_vector::RationalVector;
use crate::side::Side;
use crate::signum::Signum;

#[derive(Debug, Clone)]
pub enum Vector {
    Int(IntVector),
    Rational(RationalVector),
}

impl Vector {
    pub const ZERO: Vector = Vector::Int(IntVector::ZERO);

    pub fn new(x: i32, y: i32) -> Vector {
        let result = IntVector::new(x, y);
        if x.wrapping_abs() > CRIT_INT || y.wrapping_abs() > CRIT_INT {
            Vector::Rational(RationalVector::from_int(&result))
        } else {
            Vector::Int(result)
        }
    }

    pub fn from_big(x: BigInt, y: BigInt, z: BigInt) -> Vector {
        let (mut x, mut y, mut z) = if z.is_negative() {
            (-x, -y, -z)
        } else {
            (x, y, z)
        };
        if x.mod_floor(&z).is_zero() {
            x = &x / &z;
            y = &y / &z;
            z = BigInt::one();
        }
        if z.is_one() {
            let crit = crit_int_big();
            if x.abs() <= crit && y.abs() <= crit {
                return Vector::Int(IntVector::new(
                    x.to_i32().expect("|x| <= CRIT_INT"),
                    y.to_i32().expect("|y| <= CRIT_INT"),
                ));
            }
        }
        Vector::Rational(RationalVector::new(x, y, z))
    }

    pub fn is_zero(&self) -> bool {
        match self {
            Vector::Int(v) => v.is_zero(),
            Vector::Rational(v) => v.is_zero(),
        }
    }

    pub fn negate(&self) -> Vector {
        match self {
            Vector::Int(v) => Vector::Int(v.negate()),
            Vector::Rational(v) => Vector::Rational(v.negate()),
        }
    }

    pub fn add(&self, other: &Vector) -> Vector {
        match (self, other) {
            (Vector::Int(a), Vector::Int(b)) => Vector::Int(a.add(b)),
            (Vector::Int(a), Vector::Rational(b)) => b.add_int(a),
            (Vector::Rational(a), Vector::Int(b)) => a.add_int(b),
            (Vector::Rational(a), Vector::Rational(b)) => b.add_rational(a),
        }
    }

    pub fn side_of(&self, other: &Vector) -> Side {
        match (self, other) {
            (Vector::Int(a), Vector::Int(b)) => a.side_of(b),
            (Vector::Int(a), Vector::Rational(b)) => b.side_of_int(a).negate(),
            (Vector::Rational(a), Vector::Int(b)) => a.side_of_int(b),
            (Vector::Rational(a), Vector::Rational(b)) => b.side_of_rational(a).negate(),
        }
    }

    pub fn is_orthogonal(&self) -> bool {
        match self {
            Vector::Int(v) => v.is_orthogonal(),
            Vector::Rational(v) => v.is_orthogonal(),
        }
    }

    pub fn is_diagonal(&self) -> bool {
        match self {
            Vector::Int(v) => v.is_diagonal(),
            Vector::Rational(v) => v.is_diagonal(),
        }
    }

    pub fn is_multiple_of_45_degree(&self) -> bool {
        self.is_orthogonal() || self.is_diagonal()
    }

    pub fn projection(&self, other: &Vector) -> Signum {
        match (self, other) {
            (Vector::Int(a), Vector::Int(b)) => a.projection(b),
            (Vector::Int(a), Vector::Rational(b)) => b.projection_int(a),
            (Vector::Rational(a), Vector::Int(b)) => a.projection_int(b),
            (Vector::Rational(a), Vector::Rational(b)) => b.projection_rational(a),
        }
    }

    pub fn scalar_product(&self, other: &Vector) -> f64 {
        match (self, other) {
            (Vector::Int(a), Vector::Int(b)) => a.scalar_product(b),
            (Vector::Int(a), Vector::Rational(b)) => b.scalar_product_int(a),
            (Vector::Rational(a), Vector::Int(b)) => a.scalar_product_int(b),
            (Vector::Rational(a), Vector::Rational(b)) => b.scalar_product_rational(a),
        }
    }

    pub fn turn_90_degree(&self, factor: i32) -> Vector {
        match self {
            Vector::Int(v) => Vector::Int(v.turn_90_degree(factor)),
            Vector::Rational(v) => Vector::Rational(v.turn_90_degree(factor)),
        }
    }

    pub fn mirror_at_x_axis(&self) -> Vector {
        match self {
            Vector::Int(v) => Vector::Int(v.mirror_at_x_axis()),
            Vector::Rational(v) => Vector::Rational(v.mirror_at_x_axis()),
        }
    }

    pub fn mirror_at_y_axis(&self) -> Vector {
        match self {
            Vector::Int(v) => Vector::Int(v.mirror_at_y_axis()),
            Vector::Rational(v) => Vector::Rational(v.mirror_at_y_axis()),
        }
    }

    pub fn to_float(&self) -> FloatPoint {
        match self {
            Vector::Int(v) => v.to_float(),
            Vector::Rational(v) => v.to_float(),
        }
    }

    pub fn length_approx(&self) -> f64 {
        self.to_float().size()
    }

    pub fn cos_angle(&self, other: &Vector) -> f64 {
        let mut result = self.scalar_product(other);
        result /= self.to_float().size() * other.to_float().size();
        result
    }

    pub fn angle_approx_to(&self, other: &Vector) -> f64 {
        let mut result = self.cos_angle(other).acos();
        if self.side_of(other) == Side::OnTheLeft {
            result = -result;
        }
        result
    }

    pub fn angle_approx(&self) -> f64 {
        Vector::Int(IntVector::new(1, 0)).angle_approx_to(self)
    }

    pub fn to_normalized_direction(&self) -> Direction {
        match self {
            Vector::Int(v) => Direction::Int(v.to_normalized_direction()),
            Vector::Rational(v) => v.to_normalized_direction(),
        }
    }

    pub fn add_to(&self, point: &Point) -> Point {
        match (self, point) {
            (Vector::Int(v), Point::Int(p)) => Point::Int(IntPoint::new(p.x + v.x, p.y + v.y)),
            (Vector::Int(v), Point::Rational(p)) => Point::Rational(p.translate_by_int(v)),
            (Vector::Rational(v), Point::Int(p)) => v.add_to_int(p),
            (Vector::Rational(v), Point::Rational(p)) => v.add_to_rational(p),
        }
    }

    pub fn change_length_approx(&self, length: f64) -> Vector {
        match self {
            Vector::Int(v) => {
                let new_point = v.to_float().change_size(length).round();
                Point::Int(new_point).difference_by(&Point::ZERO)
            }
            Vector::Rational(v) => v.change_length_approx(length),
        }
    }
}

impl From<IntVector> for Vector {
    fn from(v: IntVector) -> Vector {
        Vector::Int(v)
    }
}

impl From<RationalVector> for Vector {
    fn from(v: RationalVector) -> Vector {
        Vector::Rational(v)
    }
}

impl PartialEq for Vector {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Vector::Int(a), Vector::Int(b)) => a == b,
            (Vector::Rational(a), Vector::Rational(b)) => a == b,
            _ => false,
        }
    }
}

impl Eq for Vector {}

#[cfg(test)]
mod tests {
    use super::*;
    use num_bigint::BigInt;

    fn b(v: i64) -> BigInt {
        BigInt::from(v)
    }

    fn rat(v: &IntVector) -> Vector {
        Vector::Rational(RationalVector::from_int(v))
    }

    #[test]
    fn new_promotes_beyond_crit_int() {
        assert!(matches!(Vector::new(5, 5), Vector::Int(_)));
        assert!(matches!(Vector::new(CRIT_INT, -CRIT_INT), Vector::Int(_)));
        assert!(matches!(Vector::new(0, CRIT_INT + 1), Vector::Rational(_)));
        assert!(matches!(Vector::new(i32::MIN, 0), Vector::Int(_)));
    }

    #[test]
    fn from_big_reduces_and_demotes() {
        assert_eq!(
            Vector::from_big(b(100), b(200), b(50)),
            Vector::Int(IntVector::new(2, 4))
        );
        assert_eq!(
            Vector::from_big(b(-6), b(9), b(-3)),
            Vector::Int(IntVector::new(2, -3))
        );
        assert!(matches!(
            Vector::from_big(b(3), b(3), b(2)),
            Vector::Rational(_)
        ));
        assert!(matches!(
            Vector::from_big(b(CRIT_INT as i64 + 1), b(0), b(1)),
            Vector::Rational(_)
        ));
    }

    #[test]
    fn from_big_only_tests_x_for_divisibility() {
        assert_eq!(
            Vector::from_big(b(6), b(5), b(2)),
            Vector::Int(IntVector::new(3, 2))
        );
    }

    #[test]
    fn zero_vector_and_predicates() {
        assert!(Vector::ZERO.is_zero());
        assert!(Vector::from_big(b(0), b(0), b(7)).is_zero());
        assert!(Vector::new(0, 5).is_orthogonal());
        assert!(Vector::Rational(RationalVector::new(b(5), b(-5), b(3))).is_diagonal());
        assert!(
            Vector::Rational(RationalVector::new(b(5), b(-5), b(3))).is_multiple_of_45_degree()
        );
        assert_ne!(
            Vector::Rational(RationalVector::new(b(0), b(0), b(1))),
            Vector::ZERO
        );
    }

    #[test]
    fn representations_agree_on_binary_operations() {
        let samples = [
            IntVector::new(1, 0),
            IntVector::new(0, 1),
            IntVector::new(3, -4),
            IntVector::new(-6, 6),
            IntVector::new(-2, -7),
            IntVector::new(0, 0),
        ];
        for a in &samples {
            for c in &samples {
                let (ai, ci) = (Vector::Int(*a), Vector::Int(*c));
                let (ar, cr) = (rat(a), rat(c));

                let expected_side = ai.side_of(&ci);
                assert_eq!(
                    ai.side_of(&cr),
                    expected_side,
                    "side_of {a:?} {c:?} int/rat"
                );
                assert_eq!(
                    ar.side_of(&ci),
                    expected_side,
                    "side_of {a:?} {c:?} rat/int"
                );
                assert_eq!(
                    ar.side_of(&cr),
                    expected_side,
                    "side_of {a:?} {c:?} rat/rat"
                );

                let expected_proj = ai.projection(&ci);
                assert_eq!(ai.projection(&cr), expected_proj, "projection {a:?} {c:?}");
                assert_eq!(ar.projection(&ci), expected_proj, "projection {a:?} {c:?}");
                assert_eq!(ar.projection(&cr), expected_proj, "projection {a:?} {c:?}");

                let expected_dot = ai.scalar_product(&ci);
                assert_eq!(ai.scalar_product(&cr), expected_dot, "dot {a:?} {c:?}");
                assert_eq!(ar.scalar_product(&ci), expected_dot, "dot {a:?} {c:?}");
                assert_eq!(ar.scalar_product(&cr), expected_dot, "dot {a:?} {c:?}");

                let expected_sum = match ai.add(&ci) {
                    Vector::Int(v) => v,
                    Vector::Rational(_) => unreachable!(),
                };
                for mixed in [ai.add(&cr), ar.add(&ci), ar.add(&cr)] {
                    assert_eq!(mixed, rat(&expected_sum), "add {a:?} {c:?}");
                }
            }
        }
    }

    #[test]
    fn unary_operations_agree_across_representations() {
        let v = IntVector::new(3, -4);
        for factor in -5..6 {
            assert_eq!(
                rat(&v).turn_90_degree(factor),
                rat(&match Vector::Int(v).turn_90_degree(factor) {
                    Vector::Int(t) => t,
                    Vector::Rational(_) => unreachable!(),
                }),
                "turn_90_degree({factor})"
            );
        }
        assert_eq!(rat(&v).negate(), rat(&v.negate()));
        assert_eq!(rat(&v).mirror_at_x_axis(), rat(&v.mirror_at_x_axis()));
        assert_eq!(rat(&v).mirror_at_y_axis(), rat(&v.mirror_at_y_axis()));
        assert_eq!(rat(&v).length_approx(), Vector::Int(v).length_approx());
        assert_eq!(rat(&v).angle_approx(), Vector::Int(v).angle_approx());
        assert_eq!(
            rat(&v).to_normalized_direction(),
            Vector::Int(v).to_normalized_direction()
        );
    }

    #[test]
    fn change_length_approx_scales_int_vectors_and_no_ops_rational_ones() {
        assert_eq!(
            Vector::Int(IntVector::new(3, -4)).change_length_approx(10.0),
            Vector::Int(IntVector::new(6, -8))
        );
        let v = RationalVector::new(b(3), b(4), b(2));
        assert_eq!(
            Vector::Rational(v.clone()).change_length_approx(100.0),
            Vector::Rational(v)
        );
    }

    #[test]
    fn to_normalized_direction_promotes_only_when_it_must() {
        assert!(matches!(
            Vector::new(CRIT_INT + 5, CRIT_INT + 5).to_normalized_direction(),
            Direction::Int(_)
        ));
        assert!(matches!(
            Vector::new(CRIT_INT + 5, CRIT_INT + 7).to_normalized_direction(),
            Direction::Big(_)
        ));
    }
}
