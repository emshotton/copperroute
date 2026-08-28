//! Port of the abstract class `app.freerouting.geometry.planar.Vector`.
//!
//! Vectors are used for translating Points in the plane. Java models the two representations as
//! two subclasses (`IntVector`, `RationalVector`) plus a package-private overload per operand
//! type, so every binary operation is resolved by two levels of dispatch:
//!
//! ```java
//! public Side sideOf(Vector other) { return other.sideOf(this).negate(); }  // level 1
//! Side sideOf(IntVector other)      { ... }                                 // level 2
//! Side sideOf(RationalVector other) { ... }                                 // level 2
//! ```
//!
//! Here that collapses into an `enum` plus one `match (self, other)` with four arms per
//! operation. Each arm below spells out which Java overload chain it corresponds to, because the
//! two-level dispatch swaps receiver and argument and is easy to get backwards.

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

/// A vector in the plane, in whichever representation is exact for its magnitude.
#[derive(Debug, Clone)]
pub enum Vector {
    /// A vector with `i32` coordinates.
    Int(IntVector),
    /// A vector with infinite-precision rational coordinates.
    Rational(RationalVector),
}

impl Vector {
    /// Standard implementation of the zero vector (an `IntVector`, as in Java).
    pub const ZERO: Vector = Vector::Int(IntVector::ZERO);

    /// Creates a Vector (x, y) in the plane. Java `Vector.getInstance(int, int)`: promotes to a
    /// `RationalVector` when either coordinate exceeds `CRIT_INT` in magnitude.
    ///
    /// `wrapping_abs` rather than `abs`, to reproduce Java's `Math.abs(Integer.MIN_VALUE) ==
    /// Integer.MIN_VALUE`: that value does *not* trip the range check in Java either.
    pub fn new(x: i32, y: i32) -> Vector {
        let result = IntVector::new(x, y);
        if x.wrapping_abs() > CRIT_INT || y.wrapping_abs() > CRIT_INT {
            Vector::Rational(RationalVector::from_int(&result))
        } else {
            Vector::Int(result)
        }
    }

    /// Creates a 2-dimensional Vector from the 3 input values. If `z != 0` it corresponds to the
    /// vector in the plane with rational number coordinates `(x / z, y / z)`. Java
    /// `Vector.getInstance(BigInteger, BigInteger, BigInteger)`.
    ///
    /// Java quirk kept verbatim (Vector.java:36-41): the divisibility test looks at `x.mod(z)`
    /// **only**, but then divides both `x` and `y` by `z`. When `y` is not a multiple of `z` the
    /// integer division truncates and the result is a different vector than the input. Panics on
    /// `z == 0`, as `BigInteger.mod` throws `ArithmeticException` there.
    pub fn from_big(x: BigInt, y: BigInt, z: BigInt) -> Vector {
        let (mut x, mut y, mut z) = if z.is_negative() {
            // the dominator z of a RationalVector is expected to be positive
            (-x, -y, -z)
        } else {
            (x, y, z)
        };
        // Java `BigInteger.mod` is the non-negative remainder => `mod_floor` (z > 0 here).
        if x.mod_floor(&z).is_zero() {
            // x and y can be divided by z
            x = &x / &z;
            y = &y / &z;
            z = BigInt::one();
        }
        if z.is_one() {
            let crit = crit_int_big();
            if x.abs() <= crit && y.abs() <= crit {
                // the Vector fits into an IntVector
                return Vector::Int(IntVector::new(
                    x.to_i32().expect("|x| <= CRIT_INT"),
                    y.to_i32().expect("|y| <= CRIT_INT"),
                ));
            }
        }
        Vector::Rational(RationalVector::new(x, y, z))
    }

    /// Returns true, if this vector is equal to the zero vector.
    pub fn is_zero(&self) -> bool {
        match self {
            Vector::Int(v) => v.is_zero(),
            Vector::Rational(v) => v.is_zero(),
        }
    }

    /// Returns the Vector such that this plus this.negate() is zero.
    pub fn negate(&self) -> Vector {
        match self {
            Vector::Int(v) => Vector::Int(v.negate()),
            Vector::Rational(v) => Vector::Rational(v.negate()),
        }
    }

    /// Adds other to this vector.
    pub fn add(&self, other: &Vector) -> Vector {
        match (self, other) {
            (Vector::Int(a), Vector::Int(b)) => Vector::Int(a.add(b)),
            // IntVector.add(Vector) -> other.add(this) -> RationalVector.add(IntVector)
            (Vector::Int(a), Vector::Rational(b)) => b.add_int(a),
            // RationalVector.add(Vector) -> IntVector.add(RationalVector) -> other.add(this)
            (Vector::Rational(a), Vector::Int(b)) => a.add_int(b),
            // RationalVector.add(Vector) -> other.add(this)
            (Vector::Rational(a), Vector::Rational(b)) => b.add_rational(a),
        }
    }

    /// Let L be the line from the Zero Vector to `other`. The function returns `Side::OnTheLeft`,
    /// if this Vector is on the left of L, `Side::OnTheRight`, if this Vector is on the right of
    /// L, and `Side::Collinear`, if this Vector is collinear with L.
    ///
    /// See `IntVector::side_of` for the derivation of the sign convention out of Java's
    /// double dispatch; each mixed arm below applies the same reasoning.
    pub fn side_of(&self, other: &Vector) -> Side {
        match (self, other) {
            (Vector::Int(a), Vector::Int(b)) => a.side_of(b),
            // IntVector.sideOf(Vector) -> RationalVector.sideOf(IntVector).negate()
            (Vector::Int(a), Vector::Rational(b)) => b.side_of_int(a).negate(),
            // RationalVector.sideOf(Vector) -> IntVector.sideOf(RationalVector).negate()
            //   -> (RationalVector.sideOf(IntVector).negate()).negate()
            (Vector::Rational(a), Vector::Int(b)) => a.side_of_int(b),
            // RationalVector.sideOf(Vector) -> other.sideOf(this).negate()
            (Vector::Rational(a), Vector::Rational(b)) => b.side_of_rational(a).negate(),
        }
    }

    /// Returns true, if the vector is horizontal or vertical.
    pub fn is_orthogonal(&self) -> bool {
        match self {
            Vector::Int(v) => v.is_orthogonal(),
            Vector::Rational(v) => v.is_orthogonal(),
        }
    }

    /// Returns true, if the vector is diagonal.
    pub fn is_diagonal(&self) -> bool {
        match self {
            Vector::Int(v) => v.is_diagonal(),
            Vector::Rational(v) => v.is_diagonal(),
        }
    }

    /// Returns true, if the vector is orthogonal or diagonal.
    pub fn is_multiple_of_45_degree(&self) -> bool {
        self.is_orthogonal() || self.is_diagonal()
    }

    /// Returns `Signum::Positive`, if the scalar product of this vector and other is > 0,
    /// `Signum::Negative`, if it is < 0, and `Signum::Zero`, if it is 0. (The dot product is
    /// symmetric, so Java's dispatch order is immaterial to the result here.)
    pub fn projection(&self, other: &Vector) -> Signum {
        match (self, other) {
            (Vector::Int(a), Vector::Int(b)) => a.projection(b),
            (Vector::Int(a), Vector::Rational(b)) => b.projection_int(a),
            (Vector::Rational(a), Vector::Int(b)) => a.projection_int(b),
            (Vector::Rational(a), Vector::Rational(b)) => b.projection_rational(a),
        }
    }

    /// Returns an approximation of the scalar product of this vector with other by a double.
    pub fn scalar_product(&self, other: &Vector) -> f64 {
        match (self, other) {
            (Vector::Int(a), Vector::Int(b)) => a.scalar_product(b),
            (Vector::Int(a), Vector::Rational(b)) => b.scalar_product_int(a),
            (Vector::Rational(a), Vector::Int(b)) => a.scalar_product_int(b),
            (Vector::Rational(a), Vector::Rational(b)) => b.scalar_product_rational(a),
        }
    }

    /// Turns this vector by factor times 90 degree.
    // not ported: Vector.turn90Degree — implemented as turn_90_degree below (and identically on
    // IntVector/RationalVector/Point/Line/IntBox/TileShape/PolygonShape/Polyline/PolylineArea/
    // PolylineShape/Circle/FloatPoint); audit-script false positive: the digit-adjacent camelCase
    // heuristic misplaces the underscore around "90" the same way it does for "45" above.
    pub fn turn_90_degree(&self, factor: i32) -> Vector {
        match self {
            Vector::Int(v) => Vector::Int(v.turn_90_degree(factor)),
            Vector::Rational(v) => Vector::Rational(v.turn_90_degree(factor)),
        }
    }

    /// Mirrors this vector at the x axis.
    pub fn mirror_at_x_axis(&self) -> Vector {
        match self {
            Vector::Int(v) => Vector::Int(v.mirror_at_x_axis()),
            Vector::Rational(v) => Vector::Rational(v.mirror_at_x_axis()),
        }
    }

    /// Mirrors this vector at the y axis.
    pub fn mirror_at_y_axis(&self) -> Vector {
        match self {
            Vector::Int(v) => Vector::Int(v.mirror_at_y_axis()),
            Vector::Rational(v) => Vector::Rational(v.mirror_at_y_axis()),
        }
    }

    /// Approximates the coordinates of this vector by float coordinates. Java `Vector.toFloat()`.
    pub fn to_float(&self) -> FloatPoint {
        match self {
            Vector::Int(v) => v.to_float(),
            Vector::Rational(v) => v.to_float(),
        }
    }

    /// Returns an approximation of the Euclidean length of this vector.
    pub fn length_approx(&self) -> f64 {
        self.to_float().size()
    }

    /// Returns an approximation of the cosinus of the angle between this vector and other.
    pub fn cos_angle(&self, other: &Vector) -> f64 {
        let mut result = self.scalar_product(other);
        result /= self.to_float().size() * other.to_float().size();
        result
    }

    /// Returns an approximation of the signed angle between this vector and other. (Java calls
    /// both this and the no-argument variant `angleApprox`; the names are split here as in
    /// `IntVector`.)
    pub fn angle_approx_to(&self, other: &Vector) -> f64 {
        let mut result = self.cos_angle(other).acos();
        if self.side_of(other) == Side::OnTheLeft {
            result = -result;
        }
        result
    }

    /// Returns an approximation of the signed angle between this vector and the x axis.
    pub fn angle_approx(&self) -> f64 {
        // Java: `new IntVector(1, 0).angleApprox(this)` — note the receiver is the x-axis vector.
        Vector::Int(IntVector::new(1, 0)).angle_approx_to(self)
    }

    /// Converts this vector to a normalized Direction.
    pub fn to_normalized_direction(&self) -> Direction {
        match self {
            Vector::Int(v) => Direction::Int(v.to_normalized_direction()),
            Vector::Rational(v) => v.to_normalized_direction(),
        }
    }

    /// Returns the Point which results from adding this vector to point. Java `Vector.addTo`.
    pub fn add_to(&self, point: &Point) -> Point {
        match (self, point) {
            (Vector::Int(v), Point::Int(p)) => Point::Int(IntPoint::new(p.x + v.x, p.y + v.y)),
            // IntVector.addTo(RationalPoint) -> point.translateBy(this)
            (Vector::Int(v), Point::Rational(p)) => Point::Rational(p.translate_by_int(v)),
            (Vector::Rational(v), Point::Int(p)) => v.add_to_int(p),
            (Vector::Rational(v), Point::Rational(p)) => v.add_to_rational(p),
        }
    }

    /// Returns an approximation vector of this vector with the same direction and length
    /// `length`. Java `Vector.changeLengthApprox(double)` is abstract and implemented per
    /// concrete subclass:
    /// * `IntVector.changeLengthApprox` (IntVector.java:188-191) computes
    ///   `this.toFloat().changeSize(length).round().differenceBy(Point.ZERO)` — translated
    ///   directly below via `Point::difference_by`, so it reduces through the same
    ///   `Point`/`Vector` dispatch Java uses.
    /// * `RationalVector.changeLengthApprox` is an unimplemented no-op that returns `this`
    ///   unchanged (see `RationalVector::change_length_approx`).
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
    /// Java's `IntVector.equals` and `RationalVector.equals` both start with a
    /// `getClass() != other.getClass()` test, so a vector in one representation is **never**
    /// equal to a vector in the other, even when they denote the same point of the plane. That
    /// matters: `Point.translateBy` short-circuits on `vector.equals(Vector.ZERO)`, and
    /// `Vector.ZERO` is an `IntVector`, so a rational zero vector does not take the shortcut.
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
        // Java's `Math.abs(Integer.MIN_VALUE)` is negative, so the range check does not fire.
        assert!(matches!(Vector::new(i32::MIN, 0), Vector::Int(_)));
    }

    #[test]
    fn from_big_reduces_and_demotes() {
        assert_eq!(
            Vector::from_big(b(100), b(200), b(50)),
            Vector::Int(IntVector::new(2, 4))
        );
        // negative denominator is normalised first
        assert_eq!(
            Vector::from_big(b(-6), b(9), b(-3)),
            Vector::Int(IntVector::new(2, -3))
        );
        assert!(matches!(
            Vector::from_big(b(3), b(3), b(2)),
            Vector::Rational(_)
        ));
        // too big for an IntVector even after reduction
        assert!(matches!(
            Vector::from_big(b(CRIT_INT as i64 + 1), b(0), b(1)),
            Vector::Rational(_)
        ));
    }

    #[test]
    fn from_big_only_tests_x_for_divisibility() {
        // Java quirk (Vector.java:36-41): `x.mod(z) == 0` is the only test, yet BOTH x and y are
        // divided by z. Here y = 5 is not a multiple of z = 2, and 5 / 2 truncates to 2, so the
        // result is (3, 2) rather than the true (3, 2.5).
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
        // Java's `equals` is class-sensitive: a rational zero vector is not `Vector.ZERO`.
        assert_ne!(
            Vector::Rational(RationalVector::new(b(0), b(0), b(1))),
            Vector::ZERO
        );
    }

    /// The four dispatch arms of every binary operation must agree with the plain `IntVector`
    /// result whenever the operands are small enough for both representations. This is the
    /// strongest available check that Java's two-level dispatch was unfolded correctly.
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

                // `add` stays in the rational representation, so compare the values, not the
                // representations.
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
        // Java `IntVector.changeLengthApprox`: toFloat().changeSize(length).round().differenceBy
        // (Point.ZERO). (3, 4) has length 5, scaled to 10 gives exactly (6, 8).
        assert_eq!(
            Vector::Int(IntVector::new(3, -4)).change_length_approx(10.0),
            Vector::Int(IntVector::new(6, -8))
        );
        // Java `RationalVector.changeLengthApprox` is an unimplemented no-op returning `this`.
        let v = RationalVector::new(b(3), b(4), b(2));
        assert_eq!(
            Vector::Rational(v.clone()).change_length_approx(100.0),
            Vector::Rational(v)
        );
    }

    #[test]
    fn to_normalized_direction_promotes_only_when_it_must() {
        // gcd(C + 5, C + 5) = C + 5, so this reduces all the way down to an IntDirection.
        assert!(matches!(
            Vector::new(CRIT_INT + 5, CRIT_INT + 5).to_normalized_direction(),
            Direction::Int(_)
        ));
        // gcd(C + 5, C + 7) = 1 (C is even, so C + 5 is odd), so this cannot reduce.
        assert!(matches!(
            Vector::new(CRIT_INT + 5, CRIT_INT + 7).to_normalized_direction(),
            Direction::Big(_)
        ));
    }
}
