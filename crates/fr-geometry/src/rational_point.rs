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
use crate::side::Side;

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

    /// Returns a deterministic tie-breaking id for this point (RationalPoint.java:66-70):
    /// `31 * (31 * x.hashCode() + y.hashCode()) + z.hashCode()`, unrolled as Java writes it.
    ///
    /// The three hashes are `java.math.BigInteger.hashCode()`, reproduced by
    /// [`java_big_integer_hash_code`]; every step is `int` arithmetic and wraps.
    pub fn get_id(&self) -> i32 {
        let mut result = java_big_integer_hash_code(&self.x);
        result = 31i32
            .wrapping_mul(result)
            .wrapping_add(java_big_integer_hash_code(&self.y));
        31i32
            .wrapping_mul(result)
            .wrapping_add(java_big_integer_hash_code(&self.z))
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

    /// Returns the side of a line on which this point lies. Java `RationalPoint.sideOf(Line)`
    /// forwards to `Point.sideOf(line.a, line.b)`; with `line.a`/`line.b` being `IntPoint`s that
    /// unfolds to the `(Rational, Int)` arms of `Point::difference_by` and `Vector::side_of`.
    pub fn side_of_line(&self, line: &crate::line::Line) -> Side {
        let v1 = self.difference_by_int(&line.a);
        let v2 = line.b.difference_by(&line.a);
        v1.side_of_int(&v2)
    }

    /// Returns the nearest point to this point on line. Java
    /// `RationalPoint.perpendicularProjection`.
    ///
    /// **Java bug, reproduced verbatim.** RationalPoint.java:262 computes
    /// `projY = tmp1.add(tmp2)` where the otherwise identical `IntPoint.perpendicularProjection`
    /// (IntPoint.java:160) computes `projY = tmp1.subtract(tmp2)`. The `IntPoint` sign is the
    /// mathematically correct one, so for any line not through the origin (`det != 0`) this
    /// method returns a point that is not the perpendicular projection. It is kept as-is for
    /// behavioural parity; see the test in `point.rs`.
    ///
    /// The other difference to the `IntPoint` version is real and intentional in Java: this one
    /// guards the demotion to `IntPoint` with `Limits.CRIT_INT_BIG` and falls back to
    /// `denominator = ONE`.
    pub fn perpendicular_projection(&self, line: &crate::line::Line) -> crate::point::Point {
        use num_traits::ToPrimitive;

        // this function is at the moment only implemented for lines consisting of IntPoints.
        // The general implementation is still missing.
        let v = line.b.difference_by(&line.a);
        let vxvx = BigInt::from(v.x as i64 * v.x as i64);
        let vyvy = BigInt::from(v.y as i64 * v.y as i64);
        let vxvy = BigInt::from(v.x as i64 * v.y as i64);
        let mut denominator = &vxvx + &vyvy;
        let det = BigInt::from(line.a.determinant(&line.b));

        let tmp1 = &vxvx * &self.x;
        let tmp2 = &vxvy * &self.y;
        let tmp1 = tmp1 + tmp2;
        let tmp2 = &det * BigInt::from(v.y) * &self.z;
        let mut proj_x = tmp1 + tmp2;

        let tmp1 = &vxvy * &self.x;
        let tmp2 = &vyvy * &self.y;
        let tmp1 = tmp1 + tmp2;
        let tmp2 = &det * BigInt::from(v.x) * &self.z;
        // `add`, not `subtract` — see the doc comment above.
        let mut proj_y = tmp1 + tmp2;

        if !denominator.is_zero() {
            if denominator.is_negative() {
                denominator = -denominator;
                proj_x = -proj_x;
                proj_y = -proj_y;
            }
            if proj_x.mod_floor(&denominator).is_zero() && proj_y.mod_floor(&denominator).is_zero()
            {
                proj_x /= &denominator;
                proj_y /= &denominator;
                let crit = crate::limits::crit_int_big();
                if proj_x.abs() <= crit && proj_y.abs() <= crit {
                    return crate::point::Point::Int(IntPoint::new(
                        proj_x.to_i32().expect("|proj_x| <= CRIT_INT"),
                        proj_y.to_i32().expect("|proj_y| <= CRIT_INT"),
                    ));
                }
                denominator = BigInt::one();
            }
        }
        crate::point::Point::Rational(RationalPoint::new(proj_x, proj_y, denominator))
    }

    // not ported here: `Point.perpendicularDirection(Line)` is inherited from the abstract
    // `Point` class in Java; it lives on `Point` in this port (see `point.rs`).

    /// Creates the smallest Box with integer coordinates containing this point. Java
    /// `RationalPoint.surroundingBox()`.
    pub fn surrounding_box(&self) -> crate::int_box::IntBox {
        let fp = self.to_float();
        crate::int_box::IntBox::from_coords(
            fp.x.floor() as i32,
            fp.y.floor() as i32,
            fp.x.ceil() as i32,
            fp.y.ceil() as i32,
        )
    }

    /// Returns true, if this point lies in the interior or on the border of box. Java
    /// `RationalPoint.isContainedIn(IntBox)`. `z` is always `>= 0` (`RationalPoint::new`), so
    /// multiplying through by `z` preserves comparison direction.
    pub fn is_contained_in(&self, box_: &crate::int_box::IntBox) -> bool {
        let tmp = BigInt::from(box_.ll.x) * &self.z;
        if self.x < tmp {
            return false;
        }
        let tmp = BigInt::from(box_.ll.y) * &self.z;
        if self.y < tmp {
            return false;
        }
        let tmp = BigInt::from(box_.ur.x) * &self.z;
        if self.x > tmp {
            return false;
        }
        let tmp = BigInt::from(box_.ur.y) * &self.z;
        self.y <= tmp
    }

    /// Creates the smallest `IntOctagon` containing this point. Java
    /// `RationalPoint.surroundingOctagon()` (RationalPoint.java:126-142). As in Java, the
    /// rounding goes through the `double` approximation of this rational point.
    pub fn surrounding_octagon(&self) -> crate::int_octagon::IntOctagon {
        let fp = self.to_float();
        let lx = fp.x.floor() as i32;
        let ly = fp.y.floor() as i32;
        let rx = fp.x.ceil() as i32;
        let uy = fp.y.ceil() as i32;

        let tmp = fp.x - fp.y;
        let ulx = tmp.floor() as i32;
        let lrx = tmp.ceil() as i32;

        let tmp = fp.x + fp.y;
        let llx = tmp.floor() as i32;
        let urx = tmp.ceil() as i32;
        crate::int_octagon::IntOctagon::new(lx, ly, rx, uy, ulx, lrx, llx, urx)
    }

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

/// `java.math.BigInteger.hashCode()`, which `RationalPoint.getId` (RationalPoint.java:66-70) folds
/// three of together.
///
/// The JDK's implementation is
/// ```java
/// int hashCode = 0;
/// for (int i = 0; i < mag.length; i++) {
///   hashCode = (int) (31 * hashCode + (mag[i] & LONG_MASK));
/// }
/// return hashCode * signum;
/// ```
/// where `mag` is the magnitude as `int` words in **big-endian** order with no leading zero word,
/// and `signum` is -1, 0 or 1. [`BigInt::to_u32_digits`] answers the same magnitude words
/// little-endian, so the fold runs over them reversed; `(mag[i] & LONG_MASK)` widens the word to
/// an unsigned `long` and the `(int)` cast throws the widening away again, which is exactly
/// `word as i32`.
///
/// Not on [`RationalPoint`] itself because it is `BigInteger`'s method, not the point's, and
/// nothing else in the port needs a `BigInteger` hash.
pub fn java_big_integer_hash_code(value: &BigInt) -> i32 {
    let (sign, digits) = value.to_u32_digits();
    let mut hash_code: i32 = 0;
    for word in digits.iter().rev() {
        hash_code = 31i32.wrapping_mul(hash_code).wrapping_add(*word as i32);
    }
    let signum = match sign {
        Sign::Minus => -1,
        Sign::NoSign => 0,
        Sign::Plus => 1,
    };
    hash_code.wrapping_mul(signum)
}

#[cfg(test)]
mod get_id_tests {
    use super::*;
    use std::str::FromStr;

    /// Ground truth from the HEAD jar under JDK 25 (`BigInteger.hashCode()` and
    /// `RationalPoint.getId()`, run from a probe in `app.freerouting.geometry.planar`):
    ///
    /// ```text
    /// 0 -> 0                                          1 -> 1                     -1 -> -1
    /// 5 -> 5                                         -5 -> -5           4294967296 -> 31
    /// -4294967296 -> -31                  1234567890123 -> 1912285068
    /// 123456789012345678901234567890 -> 1915528825
    /// -98765432109876543210987654321 -> 617350118
    /// rationalPoint(7,-11,3).getId() = 6389
    /// rationalPoint(123456789012345678901234567890, -5, 4294967296).getId() = -1717769283
    /// ```
    #[test]
    fn the_big_integer_hash_code_is_javas() {
        let cases: [(&str, i32); 10] = [
            ("0", 0),
            ("1", 1),
            ("-1", -1),
            ("5", 5),
            ("-5", -5),
            ("4294967296", 31),
            ("-4294967296", -31),
            ("1234567890123", 1_912_285_068),
            ("123456789012345678901234567890", 1_915_528_825),
            ("-98765432109876543210987654321", 617_350_118),
        ];
        for (text, expected) in cases {
            let value = BigInt::from_str(text).expect("a decimal literal");
            assert_eq!(
                java_big_integer_hash_code(&value),
                expected,
                "BigInteger({text}).hashCode()"
            );
        }
    }

    #[test]
    fn a_rational_points_id_folds_the_three_hashes() {
        let point = RationalPoint::new(BigInt::from(7), BigInt::from(-11), BigInt::from(3));
        assert_eq!(point.get_id(), 6389);

        let big = BigInt::from_str("123456789012345678901234567890").expect("a decimal literal");
        let point = RationalPoint::new(big, BigInt::from(-5), BigInt::from(4_294_967_296i64));
        assert_eq!(point.get_id(), -1_717_769_283);
    }
}
