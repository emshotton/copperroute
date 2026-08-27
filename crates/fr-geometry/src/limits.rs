use num_bigint::BigInt;

/// Coordinates with |value| above this promote to rational (BigInt) arithmetic.
pub const CRIT_INT: i32 = 33_554_432; // 2^25

/// The biggest double value (2^53), so that all integers smaller than this value are represented
/// exactly as double values.
pub const CRIT_DOUBLE: f64 = 9_007_199_254_740_992.0; // 2^53

/// Square root of 2.
pub const SQRT2: f64 = std::f64::consts::SQRT_2;

/// Returns the CRIT_INT value as a BigInt.
pub fn crit_int_big() -> BigInt {
    BigInt::from(CRIT_INT)
}

/// Java `Math.round(double)`: rounds half towards positive infinity (not away from zero).
/// For negative half-values, this rounds towards zero. NaN returns 0.
pub fn java_round(x: f64) -> i64 {
    if x.is_nan() {
        0
    } else {
        let f = x.floor();
        if x - f >= 0.5 { f as i64 + 1 } else { f as i64 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn java_round_rounds_half_up_not_away_from_zero() {
        assert_eq!(java_round(1.5), 2);
        assert_eq!(java_round(2.5), 3);
        assert_eq!(java_round(-1.5), -1); // Java Math.round(-1.5) == -1
        assert_eq!(java_round(-2.5), -2);
        assert_eq!(java_round(-0.4), 0);
        assert_eq!(java_round(0.49999999999999994), 0);
    }

    #[test]
    fn constants() {
        assert_eq!(CRIT_INT, 1 << 25);
        assert_eq!(crit_int_big(), num_bigint::BigInt::from(CRIT_INT));
    }
}
