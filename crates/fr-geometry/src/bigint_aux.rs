use num_bigint::BigInt;
use num_integer::Integer;

/// Calculates the determinant of the vectors (x1, y1) and (x2, y2).
/// Returns x1*y2 - x2*y1.
pub fn determinant(x1: &BigInt, y1: &BigInt, x2: &BigInt, y2: &BigInt) -> BigInt {
    let tmp1 = x1 * y2;
    let tmp2 = x2 * y1;
    tmp1 - tmp2
}

/// Auxiliary function to implement addition and translation with rational coordinates.
/// Inputs and outputs are [numerator_x, numerator_y, denominator].
pub fn add_rational_coordinates(first: &[BigInt; 3], second: &[BigInt; 3]) -> [BigInt; 3] {
    if first[2] == second[2] {
        // both rational numbers have the same denominator
        [
            first[0].clone() + &second[0],
            first[1].clone() + &second[1],
            first[2].clone(),
        ]
    } else {
        // multiply both denominators for the new denominator
        let result_denom = &first[2] * &second[2];
        let tmp1 = &first[0] * &second[2];
        let tmp2 = &second[0] * &first[2];
        let result_x = tmp1 + tmp2;
        let tmp1 = &first[1] * &second[2];
        let tmp2 = &second[1] * &first[2];
        let result_y = tmp1 + tmp2;
        [result_x, result_y, result_denom]
    }
}

/// Calculate GCD of a and b using binary GCD algorithm.
/// Inputs are interpreted as non-negative integers.
pub fn binary_gcd(a: i32, b: i32) -> i32 {
    debug_assert!(a >= 0 && b >= 0);
    a.gcd(&b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b(v: i64) -> BigInt {
        BigInt::from(v)
    }

    #[test]
    fn gcd_matches_euclid() {
        assert_eq!(binary_gcd(12, 18), 6);
        assert_eq!(binary_gcd(0, 7), 7);
        assert_eq!(binary_gcd(7, 0), 7);
        assert_eq!(binary_gcd(1, 1), 1);
        assert_eq!(binary_gcd(1 << 20, 3 << 10), 1 << 10);
        assert_eq!(binary_gcd(33_554_432, 33_554_432), 33_554_432);
    }

    #[test]
    fn determinant_and_rational_add() {
        assert_eq!(determinant(&b(1), &b(2), &b(3), &b(4)), b(-2));
        let r = add_rational_coordinates(&[b(1), b(2), b(3)], &[b(1), b(1), b(3)]);
        assert_eq!(r, [b(2), b(3), b(3)]);
        let r = add_rational_coordinates(&[b(1), b(2), b(3)], &[b(1), b(1), b(2)]);
        assert_eq!(r, [b(5), b(7), b(6)]);
    }
}
