//! Port of `app.freerouting.geometry.planar.RationalPoint`.
//!
//! Implementation of points in the projective plane represented by 3 coordinates x, y, z, which
//! are infinite precision integers. Two projective points (x1, y1, z1) and (x2, y2, z2) are equal
//! if they are located on the same line through the zero point, that means there exists a number r
//! with x2 = r*x1, y2 = r*y1 and z2 = r*z1. The affine point with rational coordinates represented
//! by the projective point (x, y, z) is (x/z, y/z). The projective plane with integer coordinates
//! contains, in addition to the affine plane with rational coordinates, the so-called line at
//! infinity, which consists of all projective points (x, y, z) with z = 0.
//!
//! As in [`crate::rational_vector`], only the concrete (package-private in Java) overloads live
//! here; the four-way dispatch lives in [`crate::point::Point`].

use std::cmp::Ordering;
use std::hash::{Hash, Hasher};

use num_bigint::{BigInt, Sign};
use num_integer::Integer;
use num_traits::{One, Signed, Zero};

use crate::bigint_aux;
use crate::float_point::FloatPoint;
use crate::int_point::IntPoint;
use crate::int_vector::IntVector;
use crate::rational_vector::{RationalVector, big_to_f64};

/// Implementation of a `Point` in the projective plane, with infinite-precision coordinates.
#[derive(Debug, Clone)]
pub struct RationalPoint {
    /// The x numerator of this point.
    pub x: BigInt,
    /// The y numerator of this point.
    pub y: BigInt,
    /// The common denominator of this point. `z == 0` means a point at infinity.
    pub z: BigInt,
}

impl RationalPoint {
    /// Creates a RationalPoint from 3 BigIntegers x, y and z. They represent the 2-dimensional
    /// point with the rational number tuple `(x / z, y / z)`.
    ///
    /// Unlike [`RationalVector::new`] there is **no** sign normalisation: Java stores the three
    /// values as given and then throws `IllegalArgumentException` if `z < 0`
    /// (RationalPoint.java:32-39). Ported as a panic with Java's message.
    pub fn new(x: BigInt, y: BigInt, z: BigInt) -> RationalPoint {
        assert!(
            z.sign() != Sign::Minus,
            "RationalPoint: z is expected to be >= 0"
        );
        RationalPoint { x, y, z }
    }

    /// Creates a RationalPoint from an IntPoint.
    pub fn from_int(point: &IntPoint) -> RationalPoint {
        RationalPoint {
            x: BigInt::from(point.x),
            y: BigInt::from(point.y),
            z: BigInt::one(),
        }
    }

    /// Returns true, if this Point is a RationalPoint with denominator z = 0.
    pub fn is_infinite(&self) -> bool {
        self.z.is_zero()
    }

    /// Java `RationalPoint.translateBy(IntVector vector)`: promotes and delegates. (Java writes
    /// this as `translateBy(new RationalVector(vector))`, which binds statically to the
    /// `RationalVector` overload, not back to the public dispatcher.)
    pub fn translate_by_int(&self, vector: &IntVector) -> RationalPoint {
        self.translate_by_rational(&RationalVector::from_int(vector))
    }

    /// Java `RationalPoint.translateBy(RationalVector vector)`.
    pub fn translate_by_rational(&self, vector: &RationalVector) -> RationalPoint {
        let [rx, ry, rz] = bigint_aux::add_rational_coordinates(
            &[self.x.clone(), self.y.clone(), self.z.clone()],
            &[vector.x.clone(), vector.y.clone(), vector.z.clone()],
        );
        RationalPoint::new(rx, ry, rz)
    }

    /// Java `RationalPoint.differenceBy(IntPoint other)`: promotes and delegates.
    pub fn difference_by_int(&self, other: &IntPoint) -> RationalVector {
        self.difference_by_rational(&RationalPoint::from_int(other))
    }

    /// Java `RationalPoint.differenceBy(RationalPoint other)`: adds `(-other.x, -other.y,
    /// other.z)` to this point's projective triple.
    pub fn difference_by_rational(&self, other: &RationalPoint) -> RationalVector {
        let [rx, ry, rz] = bigint_aux::add_rational_coordinates(
            &[self.x.clone(), self.y.clone(), self.z.clone()],
            &[-other.x.clone(), -other.y.clone(), other.z.clone()],
        );
        RationalVector::new(rx, ry, rz)
    }

    /// Java `RationalPoint.compareX(IntPoint other)`: `this.x` against `this.z * other.x`.
    /// (No sign handling on `z` — `z >= 0` is a class invariant, enforced by the constructor.)
    pub fn compare_x_int(&self, other: &IntPoint) -> Ordering {
        let tmp1 = &self.z * BigInt::from(other.x);
        self.x.cmp(&tmp1)
    }

    /// Java `RationalPoint.compareX(RationalPoint other)`: cross-multiplication.
    pub fn compare_x_rational(&self, other: &RationalPoint) -> Ordering {
        let tmp1 = &self.x * &other.z;
        let tmp2 = &other.x * &self.z;
        tmp1.cmp(&tmp2)
    }

    /// Java `RationalPoint.compareY(IntPoint other)`.
    pub fn compare_y_int(&self, other: &IntPoint) -> Ordering {
        let tmp1 = &self.z * BigInt::from(other.y);
        self.y.cmp(&tmp1)
    }

    /// Java `RationalPoint.compareY(RationalPoint other)`.
    pub fn compare_y_rational(&self, other: &RationalPoint) -> Ordering {
        let tmp1 = &self.y * &other.z;
        let tmp2 = &other.y * &self.z;
        tmp1.cmp(&tmp2)
    }

    /// Approximates the coordinates of this point by float coordinates.
    ///
    /// Java special-cases `z == 0` (the line at infinity) and substitutes `Float.MAX_VALUE` — a
    /// `float` constant, implicitly widened to `double` — for both coordinates, rather than
    /// letting the division produce `Infinity`/`NaN` (RationalPoint.java:48-59). Reproduced
    /// verbatim, including the `f32::MAX` (not `f64::MAX`) magnitude.
    pub fn to_float(&self) -> FloatPoint {
        let zd = big_to_f64(&self.z);
        if zd == 0.0 {
            FloatPoint::new(f32::MAX as f64, f32::MAX as f64)
        } else {
            FloatPoint::new(big_to_f64(&self.x) / zd, big_to_f64(&self.y) / zd)
        }
    }

    // added in Task 10: side_of_line, perpendicular_projection, perpendicular_direction
    // added in Task 11: surrounding_box, is_contained_in
    // added in Task 12: surrounding_octagon
    // not ported: getId — deterministic tie-breaking id, unused outside geometry/planar.
}

impl PartialEq for RationalPoint {
    /// Port of `RationalPoint.equals` (RationalPoint.java:72-90): equal iff the projective triples
    /// are proportional, tested by two 2x2 determinants.
    ///
    /// Two Java quirks are inherited verbatim:
    /// * every point with `z == 0` (the line at infinity) compares equal to every other such
    ///   point, because both determinants collapse to `x1*0 - x2*0 = 0`;
    /// * the degenerate triple `(0, 0, 0)` compares equal to *every* rational point, which makes
    ///   this relation non-transitive (and inconsistent with `Hash` for that one value). Java has
    ///   exactly the same hole; it is never constructed by any factory here.
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

impl Eq for RationalPoint {}

impl Hash for RationalPoint {
    /// Port of `RationalPoint.hashCode` (RationalPoint.java:92-109): infinite points all hash to
    /// 0, everything else hashes its triple reduced by `gcd(|x|, |y|, z)`, which is what keeps the
    /// hash consistent with the proportionality-based `PartialEq` above.
    fn hash<H: Hasher>(&self, state: &mut H) {
        if self.z.is_zero() {
            0u8.hash(state);
            return;
        }
        let gcd = self.x.abs().gcd(&self.y.abs()).gcd(&self.z);
        if gcd > BigInt::one() {
            (&self.x / &gcd).hash(state);
            (&self.y / &gcd).hash(state);
            (&self.z / &gcd).hash(state);
        } else {
            self.x.hash(state);
            self.y.hash(state);
            self.z.hash(state);
        }
    }
}
