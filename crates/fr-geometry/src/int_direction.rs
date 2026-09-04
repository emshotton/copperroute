use crate::int_vector::IntVector;
use crate::limits::java_round;
use crate::side::Side;
use crate::signum::Signum;
use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone, Copy)]
pub struct IntDirection {
    pub x: i32,
    pub y: i32,
}

impl IntDirection {
    pub const NULL: IntDirection = IntDirection { x: 0, y: 0 };
        pub const RIGHT: IntDirection = IntDirection { x: 1, y: 0 };
        pub const RIGHT45: IntDirection = IntDirection { x: 1, y: 1 };
        pub const UP: IntDirection = IntDirection { x: 0, y: 1 };
        pub const UP45: IntDirection = IntDirection { x: -1, y: 1 };
        pub const LEFT: IntDirection = IntDirection { x: -1, y: 0 };
        pub const LEFT45: IntDirection = IntDirection { x: -1, y: -1 };
        pub const DOWN: IntDirection = IntDirection { x: 0, y: -1 };
        pub const DOWN45: IntDirection = IntDirection { x: 1, y: -1 };

        pub fn new(x: i32, y: i32) -> IntDirection {
        IntDirection { x, y }
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

        pub fn get_vector(&self) -> IntVector {
        IntVector::new(self.x, self.y)
    }

        pub fn opposite(&self) -> IntDirection {
        IntDirection::new(-self.x, -self.y)
    }

                                pub fn turn_45_degree(&self, factor: i32) -> IntDirection {
        match factor % 8 {
            0 => IntDirection::new(self.x, self.y), 
            1 => IntDirection::new(self.x - self.y, self.x + self.y), 
            2 => IntDirection::new(-self.y, self.x), 
            3 => IntDirection::new(-self.x - self.y, self.x - self.y), 
            4 => IntDirection::new(-self.x, -self.y), 
            5 => IntDirection::new(self.y - self.x, -self.x - self.y), 
            6 => IntDirection::new(self.y, -self.x), 
            7 => IntDirection::new(self.x + self.y, self.y - self.x), 
            _ => IntDirection::new(0, 0),
        }
    }

            pub fn determinant(&self, other: &IntDirection) -> i64 {
        self.x as i64 * other.y as i64 - self.y as i64 * other.x as i64
    }

                    pub fn side_of(&self, other: &IntDirection) -> Side {
        self.get_vector().side_of(&other.get_vector())
    }

                pub fn projection(&self, other: &IntDirection) -> Signum {
        self.get_vector().projection(&other.get_vector())
    }

                    pub fn compare_from(&self, p1: &IntDirection, p2: &IntDirection) -> Ordering {
        if p1.compare_to(self) != Ordering::Less {
            if p2.compare_to(self) != Ordering::Less {
                p1.compare_to(p2)
            } else {
                Ordering::Less
            }
        } else if p2.compare_to(self) != Ordering::Less {
            Ordering::Greater
        } else {
            p1.compare_to(p2)
        }
    }

                    pub fn middle_approx(&self, other: &IntDirection) -> IntDirection {
        let v1 = self.get_vector().to_float();
        let v2 = other.get_vector().to_float();
        let length1 = v1.size();
        let length2 = v2.size();
        let x = v1.x / length1 + v2.x / length2;
        let y = v1.y / length1 + v2.y / length2;
        const SCALE_FACTOR: f64 = 1000.0;
        let vm = IntVector::new(
            java_round(x * SCALE_FACTOR) as i32,
            java_round(y * SCALE_FACTOR) as i32,
        );
        vm.to_normalized_direction()
    }

        pub fn angle_approx(&self) -> f64 {
        self.get_vector().angle_approx()
    }
}

impl IntDirection {
                                                fn compare_direct(receiver: &IntDirection, param: &IntDirection) -> Ordering {
        if receiver.y > 0 {
            if param.y < 0 {
                return Ordering::Less;
            }
            if param.y == 0 {
                return if param.x > 0 {
                    Ordering::Greater
                } else {
                    Ordering::Less
                };
            }
        } else if receiver.y < 0 {
            if param.y >= 0 {
                return Ordering::Greater;
            }
        } else {
            if receiver.x > 0 {
                return if param.y != 0 || param.x < 0 {
                    Ordering::Less
                } else {
                    Ordering::Equal
                };
            }
            if param.y > 0 || (param.y == 0 && param.x > 0) {
                return Ordering::Greater;
            }
            if param.y < 0 {
                return Ordering::Less;
            }
            return Ordering::Equal;
        }

        let determinant = param.x as i64 * receiver.y as i64 - param.y as i64 * receiver.x as i64;
        match determinant.signum() {
            1 => Ordering::Greater,
            -1 => Ordering::Less,
            _ => Ordering::Equal,
        }
    }

                                                                    pub fn compare_to(&self, other: &IntDirection) -> Ordering {
        Self::compare_direct(other, self).reverse()
    }
}

impl PartialEq for IntDirection {
                                                                    fn eq(&self, other: &Self) -> bool {
        (self.x == other.x && self.y == other.y)
            || (self.side_of(other) == Side::Collinear
                && self.projection(other) == Signum::Positive)
    }
}

impl Eq for IntDirection {}

impl Hash for IntDirection {
                    fn hash<H: Hasher>(&self, state: &mut H) {
        let gcd = crate::bigint_aux::binary_gcd(self.x.abs(), self.y.abs());
        let normalized = if gcd > 0 {
            (self.x / gcd, self.y / gcd)
        } else {
            (0, 0)
        };
        normalized.hash(state);
    }
}

impl fmt::Display for IntDirection {
                                    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = if self.compare_to(&IntDirection::RIGHT) == Ordering::Equal {
            "RIGHT"
        } else if self.compare_to(&IntDirection::RIGHT45) == Ordering::Equal {
            "UP-RIGHT"
        } else if self.compare_to(&IntDirection::UP) == Ordering::Equal {
            "UP"
        } else if self.compare_to(&IntDirection::UP45) == Ordering::Equal {
            "UP-LEFT"
        } else if self.compare_to(&IntDirection::LEFT) == Ordering::Equal {
            "LEFT"
        } else if self.compare_to(&IntDirection::LEFT45) == Ordering::Equal {
            "DOWN-LEFT"
        } else if self.compare_to(&IntDirection::DOWN) == Ordering::Equal {
            "DOWN"
        } else if self.compare_to(&IntDirection::DOWN45) == Ordering::Equal {
            "DOWN-RIGHT"
        } else if self.compare_to(&IntDirection::NULL) == Ordering::Equal {
            "NULL"
        } else {
            "UNKNOWN"
        };
        f.write_str(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Side;
    use std::cmp::Ordering;

    #[test]
    fn angular_order_is_counterclockwise_from_right() {
        let order = [
            IntDirection::RIGHT,
            IntDirection::RIGHT45,
            IntDirection::UP,
            IntDirection::UP45,
            IntDirection::LEFT,
            IntDirection::LEFT45,
            IntDirection::DOWN,
            IntDirection::DOWN45,
        ];
        for w in order.windows(2) {
            assert_eq!(
                w[0].compare_to(&w[1]),
                Ordering::Less,
                "{} < {}",
                w[0],
                w[1]
            );
        }
        assert_eq!(
            IntDirection::DOWN45.compare_to(&IntDirection::RIGHT),
            Ordering::Greater
        );
        assert_eq!(
            IntDirection::new(3, 1).compare_to(&IntDirection::new(1, 3)),
            Ordering::Less
        );
    }

    #[test]
    fn compare_to_is_not_antisymmetric_at_null() {
        assert_eq!(
            IntDirection::RIGHT.compare_to(&IntDirection::NULL),
            Ordering::Less
        );
        assert_eq!(
            IntDirection::NULL.compare_to(&IntDirection::RIGHT),
            Ordering::Equal
        );
    }

    #[test]
    fn turn_45_and_opposite() {
        assert_eq!(IntDirection::RIGHT.turn_45_degree(1), IntDirection::RIGHT45);
        assert_eq!(IntDirection::RIGHT.turn_45_degree(2), IntDirection::UP);
        assert_eq!(IntDirection::RIGHT.turn_45_degree(6), IntDirection::DOWN);
        assert_eq!(
            IntDirection::new(2, 1).turn_45_degree(1),
            IntDirection::new(1, 3)
        );
        assert_eq!(IntDirection::UP.opposite(), IntDirection::DOWN);
        assert_eq!(
            IntDirection::new(1, -3).turn_45_degree(-1),
            IntDirection::NULL
        );
    }

    #[test]
    fn side_and_projection() {
        assert_eq!(
            IntDirection::RIGHT.side_of(&IntDirection::UP),
            Side::OnTheRight
        );
        assert_eq!(
            IntDirection::UP.side_of(&IntDirection::RIGHT),
            Side::OnTheLeft
        );
        assert_eq!(
            IntDirection::RIGHT.side_of(&IntDirection::LEFT),
            Side::Collinear
        );
        assert_eq!(
            IntDirection::RIGHT.projection(&IntDirection::RIGHT45),
            crate::Signum::Positive
        );
    }

    #[test]
    fn compare_from_orders_relative_to_self() {
        assert_eq!(
            IntDirection::RIGHT45.compare_from(&IntDirection::UP, &IntDirection::RIGHT),
            Ordering::Less
        );
        assert_eq!(
            IntDirection::RIGHT45.compare_from(&IntDirection::RIGHT, &IntDirection::UP),
            Ordering::Greater
        );
    }

    #[test]
    fn display_names() {
        assert_eq!(IntDirection::UP45.to_string(), "UP-LEFT");
        assert_eq!(IntDirection::new(2, 2).to_string(), "UP-RIGHT"); 
        assert_eq!(IntDirection::new(5, 1).to_string(), "UNKNOWN");
    }

    #[test]
    fn angle_approx_of_up_is_half_pi() {
        assert!((IntDirection::UP.angle_approx() - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
    }

    #[test]
    fn null_is_not_equal_to_axis_directions_but_equals_itself() {
        assert_ne!(IntDirection::RIGHT, IntDirection::NULL);
        assert_ne!(IntDirection::NULL, IntDirection::RIGHT);
        assert_ne!(IntDirection::LEFT, IntDirection::NULL);
        assert_eq!(IntDirection::NULL, IntDirection::NULL);
    }

    #[test]
    fn angular_equality_ignores_magnitude_and_hashes_match() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::Hasher as _;

        assert_eq!(IntDirection::new(2, 2), IntDirection::RIGHT45);

        let mut h1 = DefaultHasher::new();
        IntDirection::new(2, 2).hash(&mut h1);
        let mut h2 = DefaultHasher::new();
        IntDirection::RIGHT45.hash(&mut h2);
        assert_eq!(h1.finish(), h2.finish());
    }

    #[test]
    fn middle_approx_of_right_and_up_is_right45() {
        assert_eq!(
            IntDirection::RIGHT.middle_approx(&IntDirection::UP),
            IntDirection::RIGHT45
        );
    }
}
