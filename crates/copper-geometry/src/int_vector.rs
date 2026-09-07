use crate::float_point::FloatPoint;
use crate::int_direction::IntDirection;
use crate::side::Side;
use crate::signum::Signum;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IntVector {
    pub x: i32,
    pub y: i32,
}

impl IntVector {
    pub const ZERO: IntVector = IntVector { x: 0, y: 0 };

    pub fn new(x: i32, y: i32) -> IntVector {
        IntVector { x, y }
    }

    pub fn is_zero(&self) -> bool {
        self.x == 0 && self.y == 0
    }

    pub fn negate(&self) -> IntVector {
        IntVector::new(-self.x, -self.y)
    }

    pub fn is_orthogonal(&self) -> bool {
        self.x == 0 || self.y == 0
    }

    pub fn is_diagonal(&self) -> bool {
        self.x.abs() == self.y.abs()
    }

    pub fn is_multiple_of_45_degree(&self) -> bool {
        self.is_orthogonal() || self.is_diagonal()
    }

    pub fn determinant(&self, other: &IntVector) -> i64 {
        self.x as i64 * other.y as i64 - self.y as i64 * other.x as i64
    }

    pub fn turn_90_degree(&self, factor: i32) -> IntVector {
        match factor.rem_euclid(4) {
            0 => IntVector::new(self.x, self.y),
            1 => IntVector::new(-self.y, self.x),
            2 => IntVector::new(-self.x, -self.y),
            3 => IntVector::new(self.y, -self.x),
            _ => IntVector::ZERO,
        }
    }

    pub fn mirror_at_y_axis(&self) -> IntVector {
        IntVector::new(-self.x, self.y)
    }

    pub fn mirror_at_x_axis(&self) -> IntVector {
        IntVector::new(self.x, -self.y)
    }

    pub fn add(&self, other: &IntVector) -> IntVector {
        IntVector::new(self.x + other.x, self.y + other.y)
    }

    pub fn side_of(&self, other: &IntVector) -> Side {
        Side::of_i64(self.determinant(other)).negate()
    }

    pub fn projection(&self, other: &IntVector) -> Signum {
        Signum::of_i64(self.x as i64 * other.x as i64 + self.y as i64 * other.y as i64)
    }

    pub fn scalar_product(&self, other: &IntVector) -> f64 {
        self.x as f64 * other.x as f64 + self.y as f64 * other.y as f64
    }

    pub fn to_float(&self) -> FloatPoint {
        FloatPoint::new(self.x as f64, self.y as f64)
    }

    pub fn length_approx(&self) -> f64 {
        self.to_float().size()
    }

    pub fn cos_angle(&self, other: &IntVector) -> f64 {
        self.scalar_product(other) / (self.length_approx() * other.length_approx())
    }

    pub fn angle_approx_to(&self, other: &IntVector) -> f64 {
        let mut result = self.cos_angle(other).acos();
        if self.side_of(other) == Side::OnTheLeft {
            result = -result;
        }
        result
    }

    pub fn angle_approx(&self) -> f64 {
        IntVector::new(1, 0).angle_approx_to(self)
    }

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
    #[allow(clippy::identity_op)]
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
        assert_eq!(a.side_of(&b), Side::OnTheRight);
    }
}
