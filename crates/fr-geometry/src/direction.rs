use std::cmp::Ordering;
use std::fmt;

use crate::bigint_direction::BigIntDirection;
use crate::int_direction::IntDirection;
use crate::int_vector::IntVector;
use crate::limits::java_round;
use crate::point::Point;
use crate::side::Side;
use crate::signum::Signum;
use crate::vector::Vector;

#[derive(Debug, Clone)]
pub enum Direction {
        Int(IntDirection),
        Big(BigIntDirection),
}

impl Direction {
        pub fn from_vector(vector: &Vector) -> Direction {
        vector.to_normalized_direction()
    }

            pub fn between(from: &Point, to: &Point) -> Option<Direction> {
        if from == to {
            return None;
        }
        Some(Direction::from_vector(&to.difference_by(from)))
    }

                            pub fn from_angle_approx(angle: f64) -> Direction {
        const SCALE_FACTOR: f64 = 10000.0;
        let x = java_round(angle.cos() * SCALE_FACTOR) as i32;
        let y = java_round(angle.sin() * SCALE_FACTOR) as i32;
        Direction::Int(IntVector::new(x, y).to_normalized_direction())
    }

        pub fn get_vector(&self) -> Vector {
        match self {
            Direction::Int(d) => Vector::Int(d.get_vector()),
            Direction::Big(d) => d.get_vector(),
        }
    }

        pub fn is_orthogonal(&self) -> bool {
        match self {
            Direction::Int(d) => d.is_orthogonal(),
            Direction::Big(d) => d.is_orthogonal(),
        }
    }

        pub fn is_diagonal(&self) -> bool {
        match self {
            Direction::Int(d) => d.is_diagonal(),
            Direction::Big(d) => d.is_diagonal(),
        }
    }

        pub fn is_multiple_of_45_degree(&self) -> bool {
        self.is_orthogonal() || self.is_diagonal()
    }

                pub fn turn_45_degree(&self, factor: i32) -> Direction {
        match self {
            Direction::Int(d) => Direction::Int(d.turn_45_degree(factor)),
            Direction::Big(d) => Direction::Big(d.turn_45_degree(factor)),
        }
    }

        pub fn opposite(&self) -> Direction {
        match self {
            Direction::Int(d) => Direction::Int(d.opposite()),
            Direction::Big(d) => Direction::Big(d.opposite()),
        }
    }

                pub fn side_of(&self, other: &Direction) -> Side {
        self.get_vector().side_of(&other.get_vector())
    }

                pub fn projection(&self, other: &Direction) -> Signum {
        self.get_vector().projection(&other.get_vector())
    }

                    pub fn middle_approx(&self, other: &Direction) -> Direction {
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
        Direction::Int(vm.to_normalized_direction())
    }

                pub fn compare_from(&self, p1: &Direction, p2: &Direction) -> Ordering {
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

        pub fn angle_approx(&self) -> f64 {
        self.get_vector().angle_approx()
    }

                                                            pub fn compare_to(&self, other: &Direction) -> Ordering {
        match (self, other) {
            (Direction::Int(a), Direction::Int(b)) => a.compare_to(b),
            (Direction::Int(a), Direction::Big(b)) => b.compare_to_int(a).reverse(),
            (Direction::Big(a), Direction::Int(b)) => a.compare_to_int(b),
            (Direction::Big(a), Direction::Big(b)) => b.compare_to_big(a).reverse(),
        }
    }
}

impl From<IntDirection> for Direction {
    fn from(d: IntDirection) -> Direction {
        Direction::Int(d)
    }
}

impl From<BigIntDirection> for Direction {
    fn from(d: BigIntDirection) -> Direction {
        Direction::Big(d)
    }
}

impl PartialEq for Direction {
                            fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Direction::Int(a), Direction::Int(b)) => a == b,
            (Direction::Big(a), Direction::Big(b)) => a == b,
            _ => false,
        }
    }
}

impl Eq for Direction {}

impl fmt::Display for Direction {
                fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        const NAMED: [(IntDirection, &str); 9] = [
            (IntDirection::RIGHT, "RIGHT"),
            (IntDirection::RIGHT45, "UP-RIGHT"),
            (IntDirection::UP, "UP"),
            (IntDirection::UP45, "UP-LEFT"),
            (IntDirection::LEFT, "LEFT"),
            (IntDirection::LEFT45, "DOWN-LEFT"),
            (IntDirection::DOWN, "DOWN"),
            (IntDirection::DOWN45, "DOWN-RIGHT"),
            (IntDirection::NULL, "NULL"),
        ];
        for (direction, name) in NAMED {
            if self.compare_to(&Direction::Int(direction)) == Ordering::Equal {
                return f.write_str(name);
            }
        }
        f.write_str("UNKNOWN")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::int_direction::IntDirection;
    use crate::int_point::IntPoint;
    use crate::point::Point;
    use crate::vector::Vector;

    #[test]
    fn from_vector_normalises_and_between_is_none_for_equal_points() {
        assert_eq!(
            Direction::from_vector(&Vector::new(4, 0)),
            Direction::Int(IntDirection::RIGHT)
        );
        assert_eq!(
            Direction::from_vector(&Vector::new(-6, 6)),
            Direction::Int(IntDirection::UP45)
        );
        let p = Point::Int(IntPoint::new(1, 1));
        assert!(Direction::between(&p, &p).is_none());
        assert_eq!(
            Direction::between(&p, &Point::Int(IntPoint::new(1, 9))),
            Some(Direction::Int(IntDirection::UP))
        );
    }

    #[test]
    fn from_angle_approx() {
        assert_eq!(
            Direction::from_angle_approx(0.0),
            Direction::Int(IntDirection::RIGHT)
        );
        assert_eq!(
            Direction::from_angle_approx(std::f64::consts::FRAC_PI_2),
            Direction::Int(IntDirection::UP)
        );
        assert_eq!(
            Direction::from_angle_approx(std::f64::consts::FRAC_PI_4),
            Direction::Int(IntDirection::RIGHT45)
        );
    }

    #[test]
    fn big_direction_orders_against_int() {
        let big = Direction::from_vector(&Vector::new(crate::CRIT_INT + 5, crate::CRIT_INT + 7));
        assert!(matches!(big, Direction::Big(_)));
        assert_eq!(
            Direction::Int(IntDirection::RIGHT).compare_to(&big),
            std::cmp::Ordering::Less
        );
        assert_eq!(
            big.compare_to(&Direction::Int(IntDirection::UP)),
            std::cmp::Ordering::Less
        );
    }
}

#[cfg(test)]
mod cross_representation_tests {
    use super::*;
    use num_bigint::BigInt;

    const NAMED: [IntDirection; 8] = [
        IntDirection::RIGHT,
        IntDirection::RIGHT45,
        IntDirection::UP,
        IntDirection::UP45,
        IntDirection::LEFT,
        IntDirection::LEFT45,
        IntDirection::DOWN,
        IntDirection::DOWN45,
    ];

    fn big(d: &IntDirection) -> Direction {
        Direction::Big(BigIntDirection::from_int(d))
    }

                #[test]
    fn all_four_dispatch_arms_agree_on_the_angular_order() {
        let extra = [
            IntDirection::new(3, 1),
            IntDirection::new(1, 3),
            IntDirection::new(-5, 2),
        ];
        for a in NAMED.iter().chain(extra.iter()) {
            for c in NAMED.iter().chain(extra.iter()) {
                let expected = a.compare_to(c);
                assert_eq!(
                    Direction::Int(*a).compare_to(&Direction::Int(*c)),
                    expected,
                    "int/int {a:?} {c:?}"
                );
                assert_eq!(
                    Direction::Int(*a).compare_to(&big(c)),
                    expected,
                    "int/big {a:?} {c:?}"
                );
                assert_eq!(
                    big(a).compare_to(&Direction::Int(*c)),
                    expected,
                    "big/int {a:?} {c:?}"
                );
                assert_eq!(big(a).compare_to(&big(c)), expected, "big/big {a:?} {c:?}");
            }
        }
    }

                                    #[test]
    fn the_dispatch_arms_diverge_at_the_zero_direction_as_in_java() {
        let right = IntDirection::RIGHT;
        let null = IntDirection::NULL;
        assert_eq!(
            Direction::Int(right).compare_to(&Direction::Int(null)),
            Ordering::Less
        );
        assert_eq!(
            Direction::Int(right).compare_to(&big(&null)),
            Ordering::Less
        );
        assert_eq!(big(&right).compare_to(&big(&null)), Ordering::Less);
        assert_eq!(
            big(&right).compare_to(&Direction::Int(null)),
            Ordering::Equal
        );
    }

    #[test]
    fn named_directions_are_in_counterclockwise_order() {
        for w in NAMED.windows(2) {
            assert_eq!(
                big(&w[0]).compare_to(&big(&w[1])),
                Ordering::Less,
                "{:?} < {:?}",
                w[0],
                w[1]
            );
        }
    }

    #[test]
    fn predicates_and_unary_operations_agree() {
        for d in &NAMED {
            let (i, b) = (Direction::Int(*d), big(d));
            assert_eq!(i.is_orthogonal(), b.is_orthogonal(), "{d:?}");
            assert_eq!(i.is_diagonal(), b.is_diagonal(), "{d:?}");
            assert_eq!(
                i.is_multiple_of_45_degree(),
                b.is_multiple_of_45_degree(),
                "{d:?}"
            );
            assert_eq!(i.angle_approx(), b.angle_approx(), "{d:?}");
            assert_eq!(
                i.opposite().compare_to(&b.opposite()),
                Ordering::Equal,
                "{d:?}"
            );
            for c in &NAMED {
                assert_eq!(
                    i.side_of(&Direction::Int(*c)),
                    b.side_of(&big(c)),
                    "{d:?} {c:?}"
                );
                assert_eq!(
                    i.projection(&Direction::Int(*c)),
                    b.projection(&big(c)),
                    "{d:?} {c:?}"
                );
                assert_eq!(
                    i.middle_approx(&Direction::Int(*c)),
                    b.middle_approx(&big(c)),
                    "{d:?} {c:?}"
                );
            }
        }
    }

    #[test]
    fn turn_45_degree_is_a_stub_for_big_directions() {
        let b = big(&IntDirection::RIGHT);
        for factor in 0..8 {
            assert_eq!(b.turn_45_degree(factor), b, "factor {factor}");
        }
        assert_eq!(
            Direction::Int(IntDirection::RIGHT).turn_45_degree(2),
            Direction::Int(IntDirection::UP)
        );
    }

    #[test]
    fn equality_is_representation_sensitive_but_magnitude_blind() {
        assert_ne!(
            Direction::Int(IntDirection::RIGHT),
            big(&IntDirection::RIGHT)
        );
        assert_eq!(
            Direction::Big(BigIntDirection::new(BigInt::from(4), BigInt::from(4))),
            Direction::Big(BigIntDirection::new(BigInt::from(1), BigInt::from(1)))
        );
        assert_ne!(
            Direction::Big(BigIntDirection::new(BigInt::from(-1), BigInt::from(-1))),
            Direction::Big(BigIntDirection::new(BigInt::from(1), BigInt::from(1)))
        );
        let zero = Direction::Big(BigIntDirection::new(BigInt::from(0), BigInt::from(0)));
        assert_eq!(zero.compare_to(&big(&IntDirection::RIGHT)), Ordering::Equal);
        assert_ne!(zero, big(&IntDirection::RIGHT));
        assert_eq!(zero, zero.clone());
    }

    #[test]
    fn compare_from_and_get_vector() {
        assert_eq!(
            big(&IntDirection::RIGHT45)
                .compare_from(&big(&IntDirection::UP), &big(&IntDirection::RIGHT)),
            Ordering::Less
        );
        assert_eq!(
            big(&IntDirection::RIGHT).get_vector(),
            Vector::Rational(crate::rational_vector::RationalVector::new(
                BigInt::from(1),
                BigInt::from(0),
                BigInt::from(1)
            ))
        );
        assert_eq!(
            Direction::Int(IntDirection::RIGHT).get_vector(),
            Vector::Int(IntVector::new(1, 0))
        );
    }

    #[test]
    fn between_mixes_representations() {
        let from = Point::new(-crate::CRIT_INT, -crate::CRIT_INT);
        let to = Point::new(crate::CRIT_INT, crate::CRIT_INT);
        assert_eq!(
            Direction::between(&from, &to),
            Some(Direction::Int(IntDirection::RIGHT45))
        );
        assert!(Direction::between(&from, &from).is_none());
    }
}

#[cfg(test)]
mod display_tests {
    use super::*;
    use crate::bigint_direction::BigIntDirection;
    use num_bigint::BigInt;

    #[test]
    fn display_names_match_int_direction() {
        assert_eq!(Direction::Int(IntDirection::UP45).to_string(), "UP-LEFT");
        assert_eq!(
            Direction::Big(BigIntDirection::from_int(&IntDirection::DOWN)).to_string(),
            "DOWN"
        );
        assert_eq!(
            Direction::Big(BigIntDirection::new(BigInt::from(3), BigInt::from(1))).to_string(),
            "UNKNOWN"
        );
    }
}
