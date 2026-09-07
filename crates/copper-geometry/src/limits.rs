use num_bigint::BigInt;

pub const CRIT_INT: i32 = 33_554_432;

pub const CRIT_DOUBLE: f64 = 9_007_199_254_740_992.0;

pub const SQRT2: f64 = std::f64::consts::SQRT_2;

pub const SMALLEST_SUBNORMAL_F64: f64 = f64::from_bits(1);

pub fn crit_int_big() -> BigInt {
    BigInt::from(CRIT_INT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smallest_subnormal_is_below_min_positive_and_above_zero() {
        assert_eq!(SMALLEST_SUBNORMAL_F64, 4.9E-324);
        assert_eq!(SMALLEST_SUBNORMAL_F64.to_bits(), 1);
        assert_eq!(
            SMALLEST_SUBNORMAL_F64.partial_cmp(&f64::MIN_POSITIVE),
            Some(std::cmp::Ordering::Less)
        );
        assert_eq!(
            SMALLEST_SUBNORMAL_F64.partial_cmp(&0.0),
            Some(std::cmp::Ordering::Greater)
        );
        assert_eq!(f64::MIN_POSITIVE, 2.2250738585072014E-308);
    }

    #[test]
    fn constants() {
        assert_eq!(CRIT_INT, 1 << 25);
        assert_eq!(crit_int_big(), num_bigint::BigInt::from(CRIT_INT));
    }
}
