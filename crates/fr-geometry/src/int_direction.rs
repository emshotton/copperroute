//! Port of `app.freerouting.geometry.planar.IntDirection`, plus the parts of the abstract
//! `Direction` class relevant to it (constants, `turn45Degree`, `opposite`, `compareFrom`,
//! `middleApprox`, `angleApprox`, `toString`, `equals`).
//!
//! Implements a `Direction` as an equivalence class of `IntVector`s: two vectors define the same
//! `IntDirection` if they point the same way (collinear, same sense), regardless of magnitude.

use crate::int_vector::IntVector;
use crate::limits::java_round;
use crate::side::Side;
use crate::signum::Signum;
use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};

/// Implements `Direction` as an equivalence class of `IntVector`s.
#[derive(Debug, Clone, Copy)]
pub struct IntDirection {
    pub x: i32,
    pub y: i32,
}

impl IntDirection {
    pub const NULL: IntDirection = IntDirection { x: 0, y: 0 };
    /// The direction to the east.
    pub const RIGHT: IntDirection = IntDirection { x: 1, y: 0 };
    /// The direction to the northeast.
    pub const RIGHT45: IntDirection = IntDirection { x: 1, y: 1 };
    /// The direction to the north.
    pub const UP: IntDirection = IntDirection { x: 0, y: 1 };
    /// The direction to the northwest.
    pub const UP45: IntDirection = IntDirection { x: -1, y: 1 };
    /// The direction to the west.
    pub const LEFT: IntDirection = IntDirection { x: -1, y: 0 };
    /// The direction to the southwest.
    pub const LEFT45: IntDirection = IntDirection { x: -1, y: -1 };
    /// The direction to the south.
    pub const DOWN: IntDirection = IntDirection { x: 0, y: -1 };
    /// The direction to the southeast.
    pub const DOWN45: IntDirection = IntDirection { x: 1, y: -1 };

    /// Creates an IntDirection from two integer coordinates.
    pub fn new(x: i32, y: i32) -> IntDirection {
        IntDirection { x, y }
    }

    /// Returns true, if the direction is horizontal or vertical.
    pub fn is_orthogonal(&self) -> bool {
        self.x == 0 || self.y == 0
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
    pub fn get_vector(&self) -> IntVector {
        IntVector::new(self.x, self.y)
    }

    /// Returns the opposite direction of this direction.
    pub fn opposite(&self) -> IntDirection {
        IntDirection::new(-self.x, -self.y)
    }

    /// Turns the direction by factor times 45 degree.
    ///
    /// Java quirk, deliberately reproduced: Java computes `n = factor % 8`, and Java's `%` (like
    /// Rust's) takes the sign of the dividend, so a negative `factor` that is not itself a
    /// multiple of 8 produces a negative `n` that matches none of the `0..=7` switch arms and
    /// falls to `default -> new IntDirection(0, 0)`. E.g. `turn_45_degree(-1)` returns `NULL`,
    /// not the same result as `turn_45_degree(7)`. This is not "fixed" here.
    pub fn turn_45_degree(&self, factor: i32) -> IntDirection {
        match factor % 8 {
            0 => IntDirection::new(self.x, self.y), // 0 degrees
            1 => IntDirection::new(self.x - self.y, self.x + self.y), // 45 degrees
            2 => IntDirection::new(-self.y, self.x), // 90 degrees
            3 => IntDirection::new(-self.x - self.y, self.x - self.y), // 135 degrees
            4 => IntDirection::new(-self.x, -self.y), // 180 degrees
            5 => IntDirection::new(self.y - self.x, -self.x - self.y), // 225 degrees
            6 => IntDirection::new(self.y, -self.x), // 270 degrees
            7 => IntDirection::new(self.x + self.y, self.y - self.x), // 315 degrees
            _ => IntDirection::new(0, 0),
        }
    }

    /// `self.x * other.y - self.y * other.x`, in `i64` (Java computes this as a `double`, but the
    /// values are bounded by `CRIT_INT` so `i64` is exact).
    pub fn determinant(&self, other: &IntDirection) -> i64 {
        self.x as i64 * other.y as i64 - self.y as i64 * other.x as i64
    }

    /// Let L be the line from the Zero Vector to `other.get_vector()`. The function returns
    /// `Side::OnTheLeft`, if `self.get_vector()` is on the left of L, `Side::OnTheRight`, if it is
    /// on the right of L, and `Side::Collinear`, if it is collinear with L. See
    /// `IntVector::side_of` for the derivation of the sign convention.
    pub fn side_of(&self, other: &IntDirection) -> Side {
        self.get_vector().side_of(&other.get_vector())
    }

    /// Returns `Signum::Positive`, if the scalar product of a vector representing this direction
    /// and a vector representing other is > 0, `Signum::Negative`, if it is < 0, and
    /// `Signum::Zero`, if it is equal 0.
    pub fn projection(&self, other: &IntDirection) -> Signum {
        self.get_vector().projection(&other.get_vector())
    }

    /// Returns `Ordering::Greater`, if the angle between p1 and this direction is bigger than the
    /// angle between p2 and this direction, `Ordering::Equal`, if p1 is equal to p2, and
    /// `Ordering::Less` otherwise.
    pub fn compare_from(&self, p1: &IntDirection, p2: &IntDirection) -> Ordering {
        if p1.cmp(self) != Ordering::Less {
            if p2.cmp(self) != Ordering::Less {
                p1.cmp(p2)
            } else {
                Ordering::Less
            }
        } else if p2.cmp(self) != Ordering::Less {
            Ordering::Greater
        } else {
            p1.cmp(p2)
        }
    }

    /// Calculates an approximation of the direction in the middle of this direction and other.
    ///
    /// `FloatPoint` does not exist yet (Task 9), so this is written with plain `f64` math instead
    /// of `getVector().toFloat()` as Java does; the arithmetic is identical.
    pub fn middle_approx(&self, other: &IntDirection) -> IntDirection {
        let (x1, y1) = (self.x as f64, self.y as f64);
        let (x2, y2) = (other.x as f64, other.y as f64);
        let length1 = f64::hypot(x1, y1);
        let length2 = f64::hypot(x2, y2);
        let x = x1 / length1 + x2 / length2;
        let y = y1 / length1 + y2 / length2;
        const SCALE_FACTOR: f64 = 1000.0;
        let vm = IntVector::new(
            java_round(x * SCALE_FACTOR) as i32,
            java_round(y * SCALE_FACTOR) as i32,
        );
        vm.to_normalized_direction()
    }

    /// Returns an approximation of the signed angle corresponding to this direction.
    pub fn angle_approx(&self) -> f64 {
        self.get_vector().angle_approx()
    }
}

impl IntDirection {
    /// Literal port of the package-private `IntDirection.compareTo(IntDirection other)`.
    /// `receiver`'s fields are Java's bare `x`/`y` inside that method body; `param` is Java's
    /// `other` parameter.
    ///
    /// **This function is NOT antisymmetric in general** — `compare_direct(a, b)` is not always
    /// `compare_direct(b, a).reverse()`. In particular it disagrees with its own mirror image
    /// whenever one side is `NULL` (the zero vector has no angle, so the algorithm's half-plane
    /// case split treats it inconsistently depending on which argument it appears as): e.g.
    /// `compare_direct(RIGHT, NULL) == Equal` but `compare_direct(NULL, RIGHT) == Greater`. Java's
    /// public `Direction.compareTo(Direction)` does not call this with `(self, other)` directly —
    /// see `Ord::cmp` below, which reproduces the public method's actual double-dispatch order.
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
            // receiver.y == 0
            if receiver.x > 0 {
                return if param.y != 0 || param.x < 0 {
                    Ordering::Less
                } else {
                    Ordering::Equal
                };
            }
            // receiver.x <= 0 (Java's comment says "x < 0", but the code covers x == 0, i.e.
            // NULL, too — it is simply the `else` of the `x > 0` check above)
            if param.y > 0 || (param.y == 0 && param.x > 0) {
                return Ordering::Greater;
            }
            if param.y < 0 {
                return Ordering::Less;
            }
            return Ordering::Equal;
        }

        // now receiver and param are located in the same open horizontal half plane
        let determinant = param.x as i64 * receiver.y as i64 - param.y as i64 * receiver.x as i64;
        match determinant.signum() {
            1 => Ordering::Greater,
            -1 => Ordering::Less,
            _ => Ordering::Equal,
        }
    }
}

impl Ord for IntDirection {
    /// Port of the public `Direction.compareTo(Direction other)`:
    /// ```java
    /// public int compareTo(Direction otherDirection) {
    ///   return -otherDirection.compareTo(this);   // double dispatch
    /// }
    /// ```
    /// For two `IntDirection`s, `otherDirection.compareTo(this)` dispatches virtually on
    /// `otherDirection` (= our `other`) and invokes its package-private direct algorithm with
    /// receiver = `other`, param = `this` (= our `self`). So `self.compareTo(other) =
    /// -compare_direct(other, self)`. Because `compare_direct` is not antisymmetric (see its doc
    /// comment), this must evaluate it with the arguments swapped exactly as Java does — it is
    /// NOT equivalent to `compare_direct(self, other)`.
    fn cmp(&self, other: &Self) -> Ordering {
        Self::compare_direct(other, self).reverse()
    }
}

impl PartialOrd for IntDirection {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for IntDirection {
    /// Port of `Direction.equals`:
    /// ```java
    /// public final boolean equals(Object other) {
    ///   ...
    ///   if (this.sideOf(otherDirection) != Side.COLLINEAR) return false;
    ///   // check, that dir and other_dir do not point into opposite directions
    ///   return thisVector.projection(otherVector) == Signum.POSITIVE;
    /// }
    /// ```
    /// (the reference-identity shortcut is implied by the structural check below). Deliberately
    /// NOT `self.cmp(other) == Ordering::Equal`: `Ord::cmp`'s underlying `compare_direct` is not a
    /// reliable equality test at the `NULL` boundary (see its doc comment) — that mismatch is
    /// exactly why Java defines `equals` independently of `compareTo`, and this must match, since
    /// later code (e.g. `Simplex`, `Point`) tests `dir == Direction.NULL`. This formulation
    /// correctly makes `RIGHT != NULL`, `LEFT != NULL`, `NULL == NULL`, and `IntDirection::new(2,
    /// 2) == RIGHT45`.
    fn eq(&self, other: &Self) -> bool {
        (self.x == other.x && self.y == other.y)
            || (self.side_of(other) == Side::Collinear
                && self.projection(other) == Signum::Positive)
    }
}

impl Eq for IntDirection {}

impl Hash for IntDirection {
    /// Hashes the normalized (gcd-divided) coordinate pair (with `(0, 0)` for `NULL`), so that
    /// `Hash` stays consistent with the `PartialEq` above: any two directions with `self ==
    /// other` are collinear with the same sense (or structurally identical, i.e. both `NULL`), so
    /// they always reduce to the same primitive coordinate pair.
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
    /// Port of `Direction.toString`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = if *self == IntDirection::RIGHT {
            "RIGHT"
        } else if *self == IntDirection::RIGHT45 {
            "UP-RIGHT"
        } else if *self == IntDirection::UP {
            "UP"
        } else if *self == IntDirection::UP45 {
            "UP-LEFT"
        } else if *self == IntDirection::LEFT {
            "LEFT"
        } else if *self == IntDirection::LEFT45 {
            "DOWN-LEFT"
        } else if *self == IntDirection::DOWN {
            "DOWN"
        } else if *self == IntDirection::DOWN45 {
            "DOWN-RIGHT"
        } else if *self == IntDirection::NULL {
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
            assert_eq!(w[0].cmp(&w[1]), Ordering::Less, "{} < {}", w[0], w[1]);
        }
        assert_eq!(
            IntDirection::DOWN45.cmp(&IntDirection::RIGHT),
            Ordering::Greater
        );
        assert_eq!(
            IntDirection::new(3, 1).cmp(&IntDirection::new(1, 3)),
            Ordering::Less
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
        // Corrected per brief: Java's `factor % 8` is negative for factor == -1 (Java/Rust `%`
        // both take the sign of the dividend), which does not match any of the 0..=7 switch arms
        // and falls to the `default -> new IntDirection(0, 0)` case. So turn_45_degree(-1) is
        // NULL, not turn_45_degree(7) (which normalizes differently). Java's quirk, not "fixed".
        assert_eq!(
            IntDirection::new(1, -3).turn_45_degree(-1),
            IntDirection::NULL
        );
    }

    #[test]
    fn side_and_projection() {
        // Derivation (see int_vector.rs `side_of` doc comment for the general derivation):
        // Direction.sideOf(other) = this.getVector().sideOf(other.getVector()), and
        // IntVector's public sideOf collapses to Side::of_i64(self.x*other.y - self.y*other.x).negate().
        // RIGHT=(1,0), UP=(0,1): self.x*other.y - self.y*other.x = 1*1 - 0*0 = 1 -> OnTheLeft,
        // negated -> OnTheRight. Java wins: the naive "up is left of right" reading is wrong
        // because the javadoc's line L runs through `other`, not `self`.
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
        // From RIGHT45, UP comes before RIGHT (RIGHT wraps to the end).
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
        assert_eq!(IntDirection::new(2, 2).to_string(), "UP-RIGHT"); // equal under angular compare
        assert_eq!(IntDirection::new(5, 1).to_string(), "UNKNOWN");
    }

    #[test]
    fn angle_approx_of_up_is_half_pi() {
        assert!((IntDirection::UP.angle_approx() - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
    }

    #[test]
    fn null_is_not_equal_to_axis_directions_but_equals_itself() {
        // Per Java's `Direction.equals`: NULL (the zero vector) has no angle, so a non-zero
        // direction's projection onto it is always Signum.ZERO, never POSITIVE — NULL is never
        // equal to a real direction, even though it is trivially collinear with everything.
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
