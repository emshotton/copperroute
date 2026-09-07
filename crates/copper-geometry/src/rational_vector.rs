use num_bigint::{BigInt, Sign};
use num_integer::Integer;
use num_traits::{One, Signed, ToPrimitive, Zero};

use crate::bigint_aux;
use crate::bigint_direction::BigIntDirection;
use crate::direction::Direction;
use crate::float_point::FloatPoint;
use crate::int_direction::IntDirection;
use crate::int_point::IntPoint;
use crate::int_vector::IntVector;
use crate::limits::crit_int_big;
use crate::point::Point;
use crate::rational_point::RationalPoint;
use crate::side::Side;
use crate::signum::Signum;
use crate::vector::Vector;

pub(crate) fn big_sign(value: &BigInt) -> i64 {
    match value.sign() {
        Sign::Minus => -1,
        Sign::NoSign => 0,
        Sign::Plus => 1,
    }
}

pub(crate) fn big_to_f64(value: &BigInt) -> f64 {
    use num_traits::ToPrimitive;
    value.to_f64().unwrap_or(match value.sign() {
        Sign::Minus => f64::NEG_INFINITY,
        _ => f64::INFINITY,
    })
}

#[derive(Debug, Clone)]
pub struct RationalVector {
    pub x: BigInt,
    pub y: BigInt,
    pub z: BigInt,
}

impl RationalVector {
    pub fn new(x: BigInt, y: BigInt, z: BigInt) -> RationalVector {
        if z.sign() != Sign::Minus {
            RationalVector { x, y, z }
        } else {
            RationalVector {
                x: -x,
                y: -y,
                z: -z,
            }
        }
    }

    pub fn from_int(vector: &IntVector) -> RationalVector {
        RationalVector {
            x: BigInt::from(vector.x),
            y: BigInt::from(vector.y),
            z: BigInt::one(),
        }
    }

    pub fn is_zero(&self) -> bool {
        self.x.is_zero() && self.y.is_zero()
    }

    pub fn negate(&self) -> RationalVector {
        RationalVector::new(-self.x.clone(), -self.y.clone(), self.z.clone())
    }

    pub fn add_int(&self, other: &IntVector) -> Vector {
        self.add_rational(&RationalVector::from_int(other))
    }

    pub fn add_rational(&self, other: &RationalVector) -> Vector {
        let result = bigint_aux::add_rational_coordinates(
            &[self.x.clone(), self.y.clone(), self.z.clone()],
            &[other.x.clone(), other.y.clone(), other.z.clone()],
        );
        let [rx, ry, rz] = result;
        Vector::Rational(RationalVector::new(rx, ry, rz))
    }

    pub fn side_of_int(&self, other: &IntVector) -> Side {
        self.side_of_rational(&RationalVector::from_int(other))
    }

    pub fn side_of_rational(&self, other: &RationalVector) -> Side {
        let tmp1 = &self.y * &other.x;
        let tmp2 = &self.x * &other.y;
        let determinant = tmp1 - tmp2;
        Side::of_i64(big_sign(&determinant))
    }

    pub fn is_orthogonal(&self) -> bool {
        self.x.is_zero() || self.y.is_zero()
    }

    pub fn is_diagonal(&self) -> bool {
        self.x.abs() == self.y.abs()
    }

    pub fn is_multiple_of_45_degree(&self) -> bool {
        self.is_orthogonal() || self.is_diagonal()
    }

    pub fn projection_int(&self, other: &IntVector) -> Signum {
        RationalVector::from_int(other).projection_rational(self)
    }

    pub fn projection_rational(&self, other: &RationalVector) -> Signum {
        let tmp1 = &self.x * &other.x;
        let tmp2 = &self.y * &other.y;
        Signum::of_i64(big_sign(&(tmp1 + tmp2)))
    }

    pub fn scalar_product_int(&self, other: &IntVector) -> f64 {
        RationalVector::from_int(other).scalar_product_rational(self)
    }

    pub fn scalar_product_rational(&self, other: &RationalVector) -> f64 {
        let v1 = self.to_float();
        let v2 = other.to_float();
        v1.x * v2.x + v1.y * v2.y
    }

    pub fn to_float(&self) -> FloatPoint {
        let zd = big_to_f64(&self.z);
        FloatPoint::new(big_to_f64(&self.x) / zd, big_to_f64(&self.y) / zd)
    }

    pub fn turn_90_degree(&self, factor: i32) -> RationalVector {
        match factor.rem_euclid(4) {
            0 => self.clone(),
            1 => RationalVector::new(-self.y.clone(), self.x.clone(), self.z.clone()),
            2 => RationalVector::new(-self.x.clone(), -self.y.clone(), self.z.clone()),
            3 => RationalVector::new(self.y.clone(), -self.x.clone(), self.z.clone()),
            _ => self.clone(),
        }
    }

    pub fn mirror_at_y_axis(&self) -> RationalVector {
        RationalVector::new(-self.x.clone(), self.y.clone(), self.z.clone())
    }

    pub fn mirror_at_x_axis(&self) -> RationalVector {
        RationalVector::new(self.x.clone(), -self.y.clone(), self.z.clone())
    }

    pub fn to_normalized_direction(&self) -> Direction {
        let gcd = self.x.gcd(&self.y);
        let dx = &self.x / &gcd;
        let dy = &self.y / &gcd;
        let crit = crit_int_big();
        if dx.abs() <= crit && dy.abs() <= crit {
            Direction::Int(IntDirection::new(
                dx.to_i32().expect("|dx| <= CRIT_INT"),
                dy.to_i32().expect("|dy| <= CRIT_INT"),
            ))
        } else {
            Direction::Big(BigIntDirection::new(dx, dy))
        }
    }

    pub fn add_to_int(&self, point: &IntPoint) -> Point {
        let new_x = &self.z * BigInt::from(point.x) + &self.x;
        let new_y = &self.z * BigInt::from(point.y) + &self.y;
        Point::Rational(RationalPoint::new(new_x, new_y, self.z.clone()))
    }

    pub fn add_to_rational(&self, point: &RationalPoint) -> Point {
        let [rx, ry, rz] = bigint_aux::add_rational_coordinates(
            &[self.x.clone(), self.y.clone(), self.z.clone()],
            &[point.x.clone(), point.y.clone(), point.z.clone()],
        );
        Point::Rational(RationalPoint::new(rx, ry, rz))
    }

    pub fn determinant(&self, other: &RationalVector) -> BigInt {
        bigint_aux::determinant(&self.x, &self.y, &other.x, &other.y)
    }

    pub fn change_length_approx(&self, _length: f64) -> Vector {
        Vector::Rational(self.clone())
    }
}

impl PartialEq for RationalVector {
    fn eq(&self, other: &Self) -> bool {
        let det = bigint_aux::determinant(&self.x, &other.x, &self.z, &other.z);
        if !det.is_zero() {
            return false;
        }
        let det = bigint_aux::determinant(&self.y, &other.y, &self.z, &other.z);
        det.is_zero()
    }
}

impl Eq for RationalVector {}

#[cfg(test)]
mod tests {
    use super::*;

    fn b(v: i64) -> BigInt {
        BigInt::from(v)
    }

    #[test]
    fn constructor_normalises_the_sign_of_the_denominator() {
        let v = RationalVector::new(b(1), b(-2), b(-3));
        assert_eq!((v.x, v.y, v.z), (b(-1), b(2), b(3)));
    }

    #[test]
    fn equality_is_proportionality() {
        assert_eq!(
            RationalVector::new(b(2), b(4), b(2)),
            RationalVector::new(b(1), b(2), b(1))
        );
        assert_ne!(
            RationalVector::new(b(2), b(4), b(2)),
            RationalVector::new(b(1), b(2), b(2))
        );
    }

    #[test]
    fn determinant_matches_int_vector() {
        let (a, c) = (IntVector::new(1, 2), IntVector::new(3, 4));
        assert_eq!(
            RationalVector::from_int(&a).determinant(&RationalVector::from_int(&c)),
            b(a.determinant(&c))
        );
    }

    #[test]
    fn to_normalized_direction_ignores_the_denominator() {
        assert_eq!(
            RationalVector::new(b(6), b(-4), b(7)).to_normalized_direction(),
            Direction::Int(IntDirection::new(3, -2))
        );
        let crit = i64::from(crate::CRIT_INT);
        let big = RationalVector::new(b(crit + 5), b(crit + 7), b(1)).to_normalized_direction();
        assert_eq!(
            big,
            Direction::Big(BigIntDirection::new(b(crit + 5), b(crit + 7)))
        );
    }

    #[test]
    #[should_panic(expected = "attempt to divide by zero")]
    fn to_normalized_direction_of_the_zero_vector_divides_by_zero() {
        let _ = RationalVector::new(b(0), b(0), b(1)).to_normalized_direction();
    }

    #[test]
    fn approximations() {
        let v = RationalVector::new(b(3), b(4), b(2));
        let approx = v.to_float();
        assert_eq!((approx.x, approx.y), (1.5, 2.0));
        assert_eq!(
            v.scalar_product_rational(&RationalVector::new(b(2), b(2), b(1))),
            1.5 * 2.0 + 2.0 * 2.0
        );
        assert_eq!(
            v.scalar_product_int(&IntVector::new(2, 2)),
            1.5 * 2.0 + 2.0 * 2.0
        );
    }

    #[test]
    fn change_length_approx_is_an_unimplemented_no_op() {
        let v = RationalVector::new(b(3), b(4), b(2));
        assert_eq!(v.change_length_approx(100.0), Vector::Rational(v));
    }
}
