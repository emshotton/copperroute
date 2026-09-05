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

    pub fn translate_by_int(&self, vector: &IntVector) -> RationalPoint {
        self.translate_by_rational(&RationalVector::from_int(vector))
    }

    pub fn translate_by_rational(&self, vector: &RationalVector) -> RationalPoint {
        let [rx, ry, rz] = bigint_aux::add_rational_coordinates(
            &[self.x.clone(), self.y.clone(), self.z.clone()],
            &[vector.x.clone(), vector.y.clone(), vector.z.clone()],
        );
        RationalPoint::new(rx, ry, rz)
    }

    pub fn difference_by_int(&self, other: &IntPoint) -> RationalVector {
        self.difference_by_rational(&RationalPoint::from_int(other))
    }

    pub fn difference_by_rational(&self, other: &RationalPoint) -> RationalVector {
        let [rx, ry, rz] = bigint_aux::add_rational_coordinates(
            &[self.x.clone(), self.y.clone(), self.z.clone()],
            &[-other.x.clone(), -other.y.clone(), other.z.clone()],
        );
        RationalVector::new(rx, ry, rz)
    }

    pub fn compare_x_int(&self, other: &IntPoint) -> Ordering {
        let tmp1 = &self.z * BigInt::from(other.x);
        self.x.cmp(&tmp1)
    }

    pub fn compare_x_rational(&self, other: &RationalPoint) -> Ordering {
        let tmp1 = &self.x * &other.z;
        let tmp2 = &other.x * &self.z;
        tmp1.cmp(&tmp2)
    }

    pub fn compare_y_int(&self, other: &IntPoint) -> Ordering {
        let tmp1 = &self.z * BigInt::from(other.y);
        self.y.cmp(&tmp1)
    }

    pub fn compare_y_rational(&self, other: &RationalPoint) -> Ordering {
        let tmp1 = &self.y * &other.z;
        let tmp2 = &other.y * &self.z;
        tmp1.cmp(&tmp2)
    }

    pub fn get_id(&self) -> i32 {
        let mut result = java_big_integer_hash_code(&self.x);
        result = 31i32
            .wrapping_mul(result)
            .wrapping_add(java_big_integer_hash_code(&self.y));
        31i32
            .wrapping_mul(result)
            .wrapping_add(java_big_integer_hash_code(&self.z))
    }

    pub fn to_float(&self) -> FloatPoint {
        let zd = big_to_f64(&self.z);
        if zd == 0.0 {
            FloatPoint::new(f32::MAX as f64, f32::MAX as f64)
        } else {
            FloatPoint::new(big_to_f64(&self.x) / zd, big_to_f64(&self.y) / zd)
        }
    }

    pub fn side_of_line(&self, line: &crate::line::Line) -> Side {
        let v1 = self.difference_by_int(&line.a);
        let v2 = line.b.difference_by(&line.a);
        v1.side_of_int(&v2)
    }

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
        let mut proj_y = tmp1 - tmp2;

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

    pub fn surrounding_box(&self) -> crate::int_box::IntBox {
        let fp = self.to_float();
        crate::int_box::IntBox::from_coords(
            fp.x.floor() as i32,
            fp.y.floor() as i32,
            fp.x.ceil() as i32,
            fp.y.ceil() as i32,
        )
    }

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
}

impl PartialEq for RationalPoint {
    fn eq(&self, other: &Self) -> bool {
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
