//! Port of `app.freerouting.geometry.planar.BigIntDirection`: implements the abstract `Direction`
//! as a tuple of infinite precision integers.

use std::cmp::Ordering;

use num_bigint::BigInt;
use num_traits::{One, Signed, Zero};

use crate::int_direction::IntDirection;
use crate::rational_vector::{RationalVector, big_sign};
use crate::side::Side;
use crate::signum::Signum;
use crate::vector::Vector;

/// Implements `Direction` as a tuple of infinite precision integers.
#[derive(Debug, Clone)]
pub struct BigIntDirection {
    /// The x coordinate of this direction.
    pub x: BigInt,
    /// The y coordinate of this direction.
    pub y: BigInt,
}

impl BigIntDirection {
    /// Creates a BigIntDirection from two BigIntegers.
    pub fn new(x: BigInt, y: BigInt) -> BigIntDirection {
        BigIntDirection { x, y }
    }

    /// Creates a BigIntDirection from an IntDirection.
    pub fn from_int(dir: &IntDirection) -> BigIntDirection {
        BigIntDirection {
            x: BigInt::from(dir.x),
            y: BigInt::from(dir.y),
        }
    }

    /// Creates a BigIntDirection from the numerators of a RationalVector.
    ///
    /// The denominator scales both coordinates equally and so does not change the direction; this
    /// is the shape in which `RationalVector.toNormalizedDirection` builds one.
    pub fn from_rational_vector(vector: &RationalVector) -> BigIntDirection {
        BigIntDirection {
            x: vector.x.clone(),
            y: vector.y.clone(),
        }
    }

    /// Returns true, if the direction is horizontal or vertical.
    pub fn is_orthogonal(&self) -> bool {
        self.x.is_zero() || self.y.is_zero()
    }

    /// Returns true, if the direction is diagonal.
    pub fn is_diagonal(&self) -> bool {
        self.x.abs() == self.y.abs()
    }

    /// Returns true, if the direction is orthogonal or diagonal.
    pub fn is_multiple_of_45_degree(&self) -> bool {
        self.is_orthogonal() || self.is_diagonal()
    }

    /// Returns any Vector pointing into this direction.
    pub fn get_vector(&self) -> Vector {
        Vector::Rational(RationalVector::new(
            self.x.clone(),
            self.y.clone(),
            BigInt::one(),
        ))
    }

    /// Turns the direction by factor times 45 degree.
    ///
    /// Java quirk, deliberately reproduced (BigIntDirection.java:42-46): the method is a stub that
    /// logs "BigIntDirection: turn_45_degree not yet implemented" and returns `this` unchanged,
    /// whatever `factor` is. `fr-geometry` must not depend on `tracing`, so the diagnostic is
    /// dropped and only the behaviour is kept.
    pub fn turn_45_degree(&self, _factor: i32) -> BigIntDirection {
        self.clone()
    }

    /// Returns the opposite direction of this direction.
    pub fn opposite(&self) -> BigIntDirection {
        BigIntDirection::new(-self.x.clone(), -self.y.clone())
    }

    /// Java `BigIntDirection.compareTo(IntDirection other)`: promotes `other` and delegates.
    pub fn compare_to_int(&self, other: &IntDirection) -> Ordering {
        self.compare_to_big(&BigIntDirection::from_int(other))
    }

    /// Verbatim port of the package-private `BigIntDirection.compareTo(BigIntDirection other)`
    /// (BigIntDirection.java:69-113): the angular order counted counterclockwise from the positive
    /// x-axis. `self` is Java's receiver (bare `x`/`y`), `other` is Java's parameter.
    ///
    /// Like `IntDirection`'s equivalent, this is **not** antisymmetric when one operand is the
    /// zero direction, which is why neither type implements `Ord`.
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
            // y1 == 0
            if x1 > 0 {
                return if y2 != 0 || x2 < 0 {
                    Ordering::Less
                } else {
                    Ordering::Equal
                };
            }
            // x1 <= 0 (Java's comment says "x1 < 0", but this is simply the `else` of `x1 > 0`,
            // so it covers the zero direction too)
            if y2 > 0 || (y2 == 0 && x2 > 0) {
                return Ordering::Greater;
            }
            if y2 < 0 {
                return Ordering::Less;
            }
            return Ordering::Equal;
        }

        // now this direction and other are located in the same open horizontal half plane
        let tmp1 = &self.y * &other.x;
        let tmp2 = &self.x * &other.y;
        let determinant = tmp1 - tmp2;
        match big_sign(&determinant) {
            1 => Ordering::Greater,
            -1 => Ordering::Less,
            _ => Ordering::Equal,
        }
    }

    /// Java's `Direction.sideOf(Direction)` specialised to two BigIntDirections:
    /// `this.getVector().sideOf(other.getVector())`.
    pub fn side_of(&self, other: &BigIntDirection) -> Side {
        self.get_vector().side_of(&other.get_vector())
    }

    /// Java's `Direction.projection(Direction)` specialised to two BigIntDirections.
    pub fn projection(&self, other: &BigIntDirection) -> Signum {
        self.get_vector().projection(&other.get_vector())
    }
}

impl PartialEq for BigIntDirection {
    /// Port of `Direction.equals` (Direction.java:82-105): collinear **and** not pointing into
    /// opposite directions. The structural shortcut stands in for Java's `this == other`
    /// reference check and makes the zero direction equal to itself (its projection with itself
    /// is `Signum.ZERO`, not `POSITIVE`).
    ///
    /// Java's `equals` starts with `getClass() != other.getClass()`, so a `BigIntDirection` is
    /// never equal to an `IntDirection`; that cross-representation check lives in `Direction`'s
    /// `PartialEq`.
    fn eq(&self, other: &Self) -> bool {
        (self.x == other.x && self.y == other.y)
            || (self.side_of(other) == Side::Collinear
                && self.projection(other) == Signum::Positive)
    }
}

impl Eq for BigIntDirection {}
