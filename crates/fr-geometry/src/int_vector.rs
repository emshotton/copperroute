//! Port of `app.freerouting.geometry.planar.IntVector`: an implementation of the (abstract, in
//! Java) `Vector` interface via a tuple of `i32` coordinates.

use crate::float_point::FloatPoint;
use crate::int_direction::IntDirection;
use crate::side::Side;
use crate::signum::Signum;

/// Implementation of a Vector via a tuple of integers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IntVector {
    /// The x coordinate of this vector.
    pub x: i32,
    /// The y coordinate of this vector.
    pub y: i32,
}

impl IntVector {
    /// Standard implementation of the zero vector.
    pub const ZERO: IntVector = IntVector { x: 0, y: 0 };

    /// Creates an IntVector from two integer coordinates.
    ///
    /// Note: unlike `Vector::new` (Task 8), this does not range-check against `CRIT_INT` — Java's
    /// constructor omits the check "for performance reasons"; promotion to rational arithmetic
    /// happens one level up, in `Vector::new`.
    pub fn new(x: i32, y: i32) -> IntVector {
        IntVector { x, y }
    }

    /// Returns true, if both coordinates of this vector are 0.
    pub fn is_zero(&self) -> bool {
        self.x == 0 && self.y == 0
    }

    /// Returns the Vector such that this plus this.negate() is zero.
    pub fn negate(&self) -> IntVector {
        IntVector::new(-self.x, -self.y)
    }

    /// Returns true, if the vector is horizontal or vertical.
    pub fn is_orthogonal(&self) -> bool {
        self.x == 0 || self.y == 0
    }

    /// Returns true, if the vector is diagonal.
    pub fn is_diagonal(&self) -> bool {
        self.x.abs() == self.y.abs()
    }

    /// Returns true, if the vector is orthogonal or diagonal.
    pub fn is_multiple_of_45_degree(&self) -> bool {
        self.is_orthogonal() || self.is_diagonal()
    }

    /// Calculates the determinant of the matrix consisting of this Vector and other.
    ///
    /// `self.x * other.y - self.y * other.x`, computed in `i64` (values are bounded by
    /// `CRIT_INT` (2^25) so the products fit exactly, matching Java's exact `long` arithmetic).
    pub fn determinant(&self, other: &IntVector) -> i64 {
        self.x as i64 * other.y as i64 - self.y as i64 * other.x as i64
    }

    /// Turns this vector by factor times 90 degree.
    pub fn turn_90_degree(&self, factor: i32) -> IntVector {
        match factor.rem_euclid(4) {
            0 => IntVector::new(self.x, self.y),   // 0 degrees
            1 => IntVector::new(-self.y, self.x),  // 90 degrees
            2 => IntVector::new(-self.x, -self.y), // 180 degrees
            3 => IntVector::new(self.y, -self.x),  // 270 degrees
            _ => IntVector::ZERO,
        }
    }

    /// Mirrors this vector at the y axis.
    pub fn mirror_at_y_axis(&self) -> IntVector {
        IntVector::new(-self.x, self.y)
    }

    /// Mirrors this vector at the x axis.
    pub fn mirror_at_x_axis(&self) -> IntVector {
        IntVector::new(self.x, -self.y)
    }

    /// Adds other to this vector.
    pub fn add(&self, other: &IntVector) -> IntVector {
        IntVector::new(self.x + other.x, self.y + other.y)
    }

    /// Let L be the line from the Zero Vector to `other`. The function returns
    /// `Side::OnTheLeft`, if this Vector is on the left of L, `Side::OnTheRight`, if this Vector
    /// is on the right of L, and `Side::Collinear`, if this Vector is collinear with L.
    ///
    /// Derivation of the sign convention (Java uses a double-dispatch trick here that is easy to
    /// get backwards):
    ///
    /// ```text
    /// // IntVector.java
    /// public Side sideOf(Vector other) {      // the public, dispatched method
    ///   Side tmp = other.sideOf(this);        // note: calls `other`'s package-private overload
    ///   return tmp.negate();
    /// }
    /// Side sideOf(IntVector other) {          // package-private overload, receiver fields x/y
    ///   double determinant = (double) other.x * y - (double) other.y * x;
    ///   return Side.of(determinant);
    /// }
    /// ```
    ///
    /// For `a.sideOf(b)` (both `IntVector`), Java's overload resolution binds `this` (i.e. `a`)
    /// to the `IntVector` parameter at compile time, so `other.sideOf(this)` dispatches at
    /// runtime on `b` (`other`) and calls `b`'s package-private `sideOf(IntVector other=a)`:
    /// `determinant = a.x*b.y - a.y*b.x` (b's fields are bare `x`/`y`, a is the `other` param).
    /// So `a.sideOf(b) = Side.of(a.x*b.y - a.y*b.x).negate()`, i.e. in terms of `self`/`other`:
    /// `Side::of_i64(self.determinant(other)).negate()`. This collapses the double negation from
    /// the two-level dispatch into a single negated determinant.
    pub fn side_of(&self, other: &IntVector) -> Side {
        Side::of_i64(self.determinant(other)).negate()
    }

    /// Returns `Signum::Positive`, if the scalar product of this vector and other is > 0,
    /// `Signum::Negative`, if the scalar product is < 0, and `Signum::Zero`, if the scalar
    /// product is equal 0.
    ///
    /// Unlike `side_of`, the dot product is symmetric, so Java's double-dispatch here (`other.
    /// projection(this)` calling `other`'s package-private `projection(IntVector)`) produces the
    /// same value as the plain dot product of `self` and `other` in either order.
    pub fn projection(&self, other: &IntVector) -> Signum {
        Signum::of_i64(self.x as i64 * other.x as i64 + self.y as i64 * other.y as i64)
    }

    /// Returns an approximation of the scalar product of this vector with other by a double.
    pub fn scalar_product(&self, other: &IntVector) -> f64 {
        self.x as f64 * other.x as f64 + self.y as f64 * other.y as f64
    }

    /// Converts this vector to a FloatPoint.
    pub fn to_float(&self) -> FloatPoint {
        FloatPoint::new(self.x as f64, self.y as f64)
    }

    /// Returns an approximation of the Euclidean length of this vector.
    pub fn length_approx(&self) -> f64 {
        self.to_float().size()
    }

    /// Returns an approximation of the cosinus of the angle between this vector and other.
    pub fn cos_angle(&self, other: &IntVector) -> f64 {
        self.scalar_product(other) / (self.length_approx() * other.length_approx())
    }

    /// Returns an approximation of the signed angle between this vector and other.
    pub fn angle_approx_to(&self, other: &IntVector) -> f64 {
        let mut result = self.cos_angle(other).acos();
        if self.side_of(other) == Side::OnTheLeft {
            result = -result;
        }
        result
    }

    /// Returns an approximation of the signed angle between this vector and the x axis.
    pub fn angle_approx(&self) -> f64 {
        IntVector::new(1, 0).angle_approx_to(self)
    }

    /// Converts this vector to a normalized Direction (dividing out the gcd of |x| and |y|).
    pub fn to_normalized_direction(&self) -> IntDirection {
        let mut dx = self.x;
        let mut dy = self.y;
        let gcd = crate::bigint_aux::binary_gcd(dx.abs(), dy.abs());
        if gcd > 1 {
            dx /= gcd;
            dy /= gcd;
        }
        IntDirection::new(dx, dy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Side, Signum};

    #[test]
    #[allow(clippy::identity_op)] // `3 * 2 - (-4) * 1` spells out the expected formula verbatim
    fn basics() {
        let v = IntVector::new(3, -4);
        assert!(!v.is_zero());
        assert_eq!(v.negate(), IntVector::new(-3, 4));
        assert!(IntVector::new(0, 5).is_orthogonal());
        assert!(IntVector::new(-5, 5).is_diagonal());
        assert!(!v.is_multiple_of_45_degree());
        assert_eq!(v.determinant(&IntVector::new(1, 2)), 3 * 2 - (-4) * 1);
        assert_eq!(v.add(&IntVector::new(1, 1)), IntVector::new(4, -3));
    }

    #[test]
    fn turns_and_mirrors() {
        let v = IntVector::new(1, 2);
        assert_eq!(v.turn_90_degree(1), IntVector::new(-2, 1));
        assert_eq!(v.turn_90_degree(-1), IntVector::new(2, -1));
        assert_eq!(v.turn_90_degree(5), v.turn_90_degree(1));
        assert_eq!(v.mirror_at_x_axis(), IntVector::new(1, -2));
        assert_eq!(v.mirror_at_y_axis(), IntVector::new(-1, 2));
    }

    #[test]
    fn side_projection_scalar() {
        let right = IntVector::new(1, 0);
        let up = IntVector::new(0, 1);
        // Corrected per brief (see `side_of` doc comment for the full derivation of Java's
        // double-dispatch `sideOf(Vector)` collapsing to
        // `Side::of_i64(self.x*other.y - self.y*other.x).negate()`):
        // right.side_of(up):  1*1 - 0*0 = 1  -> OnTheLeft, negated -> OnTheRight.
        // up.side_of(right):  0*0 - 1*1 = -1 -> OnTheRight, negated -> OnTheLeft.
        assert_eq!(right.side_of(&up), Side::OnTheRight);
        assert_eq!(up.side_of(&right), Side::OnTheLeft);
        assert_eq!(right.projection(&IntVector::new(-1, 7)), Signum::Negative);
        assert_eq!(right.scalar_product(&IntVector::new(4, 9)), 4.0);
    }

    #[test]
    fn normalized_direction_divides_by_gcd() {
        assert_eq!(
            IntVector::new(6, -4).to_normalized_direction(),
            IntDirection::new(3, -2)
        );
        assert_eq!(
            IntVector::new(0, -8).to_normalized_direction(),
            IntDirection::DOWN
        );
    }

    #[test]
    fn large_determinant_does_not_overflow() {
        let a = IntVector::new(crate::CRIT_INT, crate::CRIT_INT);
        let b = IntVector::new(-crate::CRIT_INT, crate::CRIT_INT);
        assert_eq!(
            a.determinant(&b),
            2_i64 * (crate::CRIT_INT as i64) * (crate::CRIT_INT as i64)
        );
        // Corrected per Java's double-dispatch derivation (see `side_of` doc comment):
        // a=(C,C), b=(-C,C): self.x*other.y - self.y*other.x = C*C - C*(-C) = 2*C^2 -> OnTheLeft,
        // negated -> OnTheRight. The brief's original expectation (OnTheLeft) disagreed with
        // Java; Java wins.
        assert_eq!(a.side_of(&b), Side::OnTheRight);
    }
}
