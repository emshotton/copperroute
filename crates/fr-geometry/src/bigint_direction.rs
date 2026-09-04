use std::cmp::Ordering;

use num_bigint::BigInt;
use num_traits::{One, Signed, Zero};

use crate::int_direction::IntDirection;
use crate::rational_vector::{RationalVector, big_sign};
use crate::side::Side;
use crate::signum::Signum;
use crate::vector::Vector;

#[derive(Debug, Clone)]
pub struct BigIntDirection {
    pub x: BigInt,
    pub y: BigInt,
}

impl BigIntDirection {
    pub fn new(x: BigInt, y: BigInt) -> BigIntDirection {
        BigIntDirection { x, y }
    }

    pub fn from_int(dir: &IntDirection) -> BigIntDirection {
        BigIntDirection {
            x: BigInt::from(dir.x),
            y: BigInt::from(dir.y),
        }
    }

    pub fn from_rational_vector(vector: &RationalVector) -> BigIntDirection {
        BigIntDirection {
            x: vector.x.clone(),
            y: vector.y.clone(),
        }
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

    pub fn get_vector(&self) -> Vector {
        Vector::Rational(RationalVector::new(
            self.x.clone(),
            self.y.clone(),
            BigInt::one(),
        ))
    }

    pub fn turn_45_degree(&self, _factor: i32) -> BigIntDirection {
        self.clone()
    }

    pub fn opposite(&self) -> BigIntDirection {
        BigIntDirection::new(-self.x.clone(), -self.y.clone())
    }

    pub fn compare_to_int(&self, other: &IntDirection) -> Ordering {
        self.compare_to_big(&BigIntDirection::from_int(other))
    }

    pub fn compare_to_big(&self, other: &BigIntDirection) -> Ordering {
        let x1 = big_sign(&self.x);
        let y1 = big_sign(&self.y);
        let x2 = big_sign(&other.x);
        let y2 = big_sign(&other.y);
        if y1 > 0 {
            if y2 < 0 {
                return Ordering::Less;
            }
            if y2 == 0 {
                return if x2 > 0 {
                    Ordering::Greater
                } else {
                    Ordering::Less
                };
            }
        } else if y1 < 0 {
            if y2 >= 0 {
                return Ordering::Greater;
            }
        } else {
            if x1 > 0 {
                return if y2 != 0 || x2 < 0 {
                    Ordering::Less
                } else {
                    Ordering::Equal
                };
            }
            if y2 > 0 || (y2 == 0 && x2 > 0) {
                return Ordering::Greater;
            }
            if y2 < 0 {
                return Ordering::Less;
            }
            return Ordering::Equal;
        }

        let tmp1 = &self.y * &other.x;
        let tmp2 = &self.x * &other.y;
        let determinant = tmp1 - tmp2;
        match big_sign(&determinant) {
            1 => Ordering::Greater,
            -1 => Ordering::Less,
            _ => Ordering::Equal,
        }
    }

    pub fn side_of(&self, other: &BigIntDirection) -> Side {
        self.get_vector().side_of(&other.get_vector())
    }

    pub fn projection(&self, other: &BigIntDirection) -> Signum {
        self.get_vector().projection(&other.get_vector())
    }
}

impl PartialEq for BigIntDirection {
    fn eq(&self, other: &Self) -> bool {
        (self.x == other.x && self.y == other.y)
            || (self.side_of(other) == Side::Collinear
                && self.projection(other) == Signum::Positive)
    }
}

impl Eq for BigIntDirection {}
