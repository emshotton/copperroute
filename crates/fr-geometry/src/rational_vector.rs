//! Port of `app.freerouting.geometry.planar.RationalVector`: analog to `RationalPoint`, but
//! implementing the functionality of a `Vector` instead of the functionality of a `Point`.
//!
//! Java's double dispatch (`add(Vector)` → `other.add(this)` → concrete overload) is collapsed
//! here: this file holds only the concrete, package-private overloads (`*_int` / `*_rational`);
//! the four-way `match` that Java spells as two levels of virtual dispatch lives in
//! [`crate::vector::Vector`].

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

/// `BigInteger.signum()` as an `i64`, so it can be fed to `Side::of_i64` / `Signum::of_i64`.
pub(crate) fn big_sign(value: &BigInt) -> i64 {
    match value.sign() {
        Sign::Minus => -1,
        Sign::NoSign => 0,
        Sign::Plus => 1,
    }
}

/// `BigInteger.doubleValue()`. `num-bigint` returns `None` only if the value is not representable;
/// Java saturates to an infinity there, so do the same.
pub(crate) fn big_to_f64(value: &BigInt) -> f64 {
    use num_traits::ToPrimitive;
    value.to_f64().unwrap_or(match value.sign() {
        Sign::Minus => f64::NEG_INFINITY,
        _ => f64::INFINITY,
    })
}

/// Implementation of a `Vector` in the projective plane: the 2-dimensional vector with the
/// rational-number tuple `(x / z, y / z)`.
#[derive(Debug, Clone)]
pub struct RationalVector {
    /// The x numerator of this vector.
    pub x: BigInt,
    /// The y numerator of this vector.
    pub y: BigInt,
    /// The common denominator of this vector. Kept `>= 0` by [`RationalVector::new`].
    pub z: BigInt,
}

impl RationalVector {
    /// Creates a RationalVector from 3 BigIntegers x, y and z. They represent the 2-dimensional
    /// vector with the rational number tuple `(x / z, y / z)`.
    ///
    /// Java normalises the sign here (unlike `RationalPoint`, which throws instead): a negative
    /// denominator negates all three coordinates.
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

    /// Creates a RationalVector from an IntVector.
    pub fn from_int(vector: &IntVector) -> RationalVector {
        RationalVector {
            x: BigInt::from(vector.x),
            y: BigInt::from(vector.y),
            z: BigInt::one(),
        }
    }

    /// Returns true, if the x and y coordinates of this vector are 0.
    pub fn is_zero(&self) -> bool {
        self.x.is_zero() && self.y.is_zero()
    }

    /// Returns the Vector such that this plus this.negate() is zero.
    pub fn negate(&self) -> RationalVector {
        RationalVector::new(-self.x.clone(), -self.y.clone(), self.z.clone())
    }

    /// Java `RationalVector.add(IntVector other)`: promotes `other` and adds.
    pub fn add_int(&self, other: &IntVector) -> Vector {
        self.add_rational(&RationalVector::from_int(other))
    }

    /// Java `RationalVector.add(RationalVector other)`.
    pub fn add_rational(&self, other: &RationalVector) -> Vector {
        let result = bigint_aux::add_rational_coordinates(
            &[self.x.clone(), self.y.clone(), self.z.clone()],
            &[other.x.clone(), other.y.clone(), other.z.clone()],
        );
        let [rx, ry, rz] = result;
        // Java returns `new RationalVector(...)` directly — no `Vector.getInstance` reduction.
        Vector::Rational(RationalVector::new(rx, ry, rz))
    }

    /// Java `RationalVector.sideOf(IntVector other)`: promotes `other` and delegates.
    pub fn side_of_int(&self, other: &IntVector) -> Side {
        self.side_of_rational(&RationalVector::from_int(other))
    }

    /// Java `RationalVector.sideOf(RationalVector other)`:
    /// `Side.of(this.y * other.x - this.x * other.y)`. Note the denominators are ignored, exactly
    /// as in Java — they are non-negative by construction, so they cannot flip the sign.
    pub fn side_of_rational(&self, other: &RationalVector) -> Side {
        let tmp1 = &self.y * &other.x;
        let tmp2 = &self.x * &other.y;
        let determinant = tmp1 - tmp2;
        Side::of_i64(big_sign(&determinant))
    }

    /// Returns true, if the vector is horizontal or vertical.
    pub fn is_orthogonal(&self) -> bool {
        self.x.is_zero() || self.y.is_zero()
    }

    /// Returns true, if the vector is diagonal.
    pub fn is_diagonal(&self) -> bool {
        // Java: `x.abs().equals(y.abs())`
        self.x.abs() == self.y.abs()
    }

    /// Returns true, if the vector is orthogonal or diagonal.
    pub fn is_multiple_of_45_degree(&self) -> bool {
        self.is_orthogonal() || self.is_diagonal()
    }

    /// Java `RationalVector.projection(IntVector other)`: promotes `other` and, as Java does,
    /// makes the promoted vector the receiver.
    pub fn projection_int(&self, other: &IntVector) -> Signum {
        RationalVector::from_int(other).projection_rational(self)
    }

    /// Java `RationalVector.projection(RationalVector other)`: the signum of the dot product of
    /// the numerators.
    pub fn projection_rational(&self, other: &RationalVector) -> Signum {
        let tmp1 = &self.x * &other.x;
        let tmp2 = &self.y * &other.y;
        Signum::of_i64(big_sign(&(tmp1 + tmp2)))
    }

    /// Java `RationalVector.scalarProduct(IntVector other)`.
    pub fn scalar_product_int(&self, other: &IntVector) -> f64 {
        RationalVector::from_int(other).scalar_product_rational(self)
    }

    /// Java `RationalVector.scalarProduct(RationalVector other)`: computed on the `toFloat()`
    /// approximations of both vectors.
    pub fn scalar_product_rational(&self, other: &RationalVector) -> f64 {
        let v1 = self.to_float();
        let v2 = other.to_float();
        v1.x * v2.x + v1.y * v2.y
    }

    /// Java `RationalVector.toFloat()`. Note Java does not special-case `z == 0` here (unlike
    /// `RationalPoint::to_float`), so a zero denominator yields infinities/NaN — reproduced.
    pub fn to_float(&self) -> FloatPoint {
        let zd = big_to_f64(&self.z);
        FloatPoint::new(big_to_f64(&self.x) / zd, big_to_f64(&self.y) / zd)
    }

    /// Turns this vector by factor times 90 degree.
    pub fn turn_90_degree(&self, factor: i32) -> RationalVector {
        // Java normalises `factor` into 0..4 with two `while` loops, i.e. a euclidean remainder.
        match factor.rem_euclid(4) {
            0 => self.clone(), // 0 degrees
            1 => RationalVector::new(-self.y.clone(), self.x.clone(), self.z.clone()), // 90 degrees
            2 => RationalVector::new(-self.x.clone(), -self.y.clone(), self.z.clone()), // 180 degrees
            3 => RationalVector::new(self.y.clone(), -self.x.clone(), self.z.clone()), // 270 degrees
            _ => self.clone(),
        }
    }

    /// Mirrors this vector at the y axis.
    pub fn mirror_at_y_axis(&self) -> RationalVector {
        RationalVector::new(-self.x.clone(), self.y.clone(), self.z.clone())
    }

    /// Mirrors this vector at the x axis.
    pub fn mirror_at_x_axis(&self) -> RationalVector {
        RationalVector::new(self.x.clone(), -self.y.clone(), self.z.clone())
    }

    /// Java `RationalVector.toNormalizedDirection()`: divides both numerators by their gcd and
    /// demotes to an [`IntDirection`] when both fit within `CRIT_INT`.
    ///
    /// Quirks kept from Java: the gcd is taken of `x` and `y` unconditionally (no `gcd > 1`
    /// guard as in `IntVector`), so a zero vector divides by zero — Java throws
    /// `ArithmeticException`, Rust panics. The denominator `z` is ignored: it scales both
    /// coordinates equally and so does not change the direction.
    pub fn to_normalized_direction(&self) -> Direction {
        let gcd = self.x.gcd(&self.y);
        let dx = &self.x / &gcd;
        let dy = &self.y / &gcd;
        let crit = crit_int_big();
        if dx.abs() <= crit && dy.abs() <= crit {
            // `intValue()` is safe here: both are <= CRIT_INT == 2^25.
            Direction::Int(IntDirection::new(
                dx.to_i32().expect("|dx| <= CRIT_INT"),
                dy.to_i32().expect("|dy| <= CRIT_INT"),
            ))
        } else {
            Direction::Big(BigIntDirection::new(dx, dy))
        }
    }

    /// Java `RationalVector.addTo(IntPoint point)`.
    pub fn add_to_int(&self, point: &IntPoint) -> Point {
        let new_x = &self.z * BigInt::from(point.x) + &self.x;
        let new_y = &self.z * BigInt::from(point.y) + &self.y;
        Point::Rational(RationalPoint::new(new_x, new_y, self.z.clone()))
    }

    /// Java `RationalVector.addTo(RationalPoint point)`.
    pub fn add_to_rational(&self, point: &RationalPoint) -> Point {
        let [rx, ry, rz] = bigint_aux::add_rational_coordinates(
            &[self.x.clone(), self.y.clone(), self.z.clone()],
            &[point.x.clone(), point.y.clone(), point.z.clone()],
        );
        Point::Rational(RationalPoint::new(rx, ry, rz))
    }

    /// Calculates the determinant of the matrix consisting of the numerators of this vector and
    /// other: `x1 * y2 - x2 * y1`.
    ///
    /// Java's `RationalVector` has no `determinant` method of its own (it inlines the same
    /// expression inside `sideOf`); this mirrors `IntVector.determinant(IntVector)` via
    /// `BigIntAux.determinant`, and is required by the task interface.
    pub fn determinant(&self, other: &RationalVector) -> BigInt {
        bigint_aux::determinant(&self.x, &self.y, &other.x, &other.y)
    }

    /// Java `RationalVector.changeLengthApprox(double)`: not actually implemented in Java either
    /// — it logs "RationalVector: change_length_approx not yet implemented" and returns `this`
    /// unchanged (RationalVector.java:196-200). `fr-geometry` has no `tracing` dependency, so the
    /// log is dropped; the no-op return value is reproduced.
    pub fn change_length_approx(&self, _length: f64) -> Vector {
        Vector::Rational(self.clone())
    }
}

impl PartialEq for RationalVector {
    /// Port of `RationalVector.equals` (RationalVector.java:54-73): two rational vectors are equal
    /// if the determinants `(x1, x2 | z1, z2)` and `(y1, y2 | z1, z2)` both vanish, i.e. if the
    /// projective triples are proportional.
    ///
    /// Java also requires `getClass() == other.getClass()`, so a `RationalVector` is never equal
    /// to an `IntVector`; that cross-representation check lives in `Vector`'s `PartialEq`.
    fn eq(&self, other: &Self) -> bool {
        // Java argument order: `BigIntAux.determinant(x, other.x, z, other.z)`.
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
        // z scales both coordinates equally, so it cannot change the direction.
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
        // Java throws ArithmeticException here: `gcd(0, 0)` is 0 and the divide is unguarded
        // (RationalVector.java:235-237). `IntVector.toNormalizedDirection` guards with `gcd > 1`
        // and returns Direction.NULL instead — the asymmetry is Java's.
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
        // Java: RationalVector.changeLengthApprox logs a warning and returns `this` unchanged.
        let v = RationalVector::new(b(3), b(4), b(2));
        assert_eq!(v.change_length_approx(100.0), Vector::Rational(v));
    }
}
