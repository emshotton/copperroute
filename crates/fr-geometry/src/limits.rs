use num_bigint::BigInt;

pub const CRIT_INT: i32 = 33_554_432; 

pub const CRIT_DOUBLE: f64 = 9_007_199_254_740_992.0; 

pub const SQRT2: f64 = std::f64::consts::SQRT_2;

pub const JAVA_DOUBLE_MIN_VALUE: f64 = f64::from_bits(1);

pub fn crit_int_big() -> BigInt {
    BigInt::from(CRIT_INT)
}

pub fn java_round(x: f64) -> i64 {
    if x.is_nan() {
        0
    } else {
        let f = x.floor();
        if x - f >= 0.5 { f as i64 + 1 } else { f as i64 }
    }
}

pub fn java_min(a: f64, b: f64) -> f64 {
    if a.is_nan() {
        return a;
    }
    if a == 0.0 && b == 0.0 && b.is_sign_negative() {
        return b;
    }
    if a <= b { a } else { b }
}

pub fn java_max(a: f64, b: f64) -> f64 {
    if a.is_nan() {
        return a;
    }
    if a == 0.0 && b == 0.0 && a.is_sign_negative() {
        return b;
    }
    if a >= b { a } else { b }
}

pub fn java_min_f32(a: f32, b: f32) -> f32 {
    if a.is_nan() {
        return a;
    }
    if a == 0.0 && b == 0.0 && b.is_sign_negative() {
        return b;
    }
    if a <= b { a } else { b }
}

pub fn java_max_f32(a: f32, b: f32) -> f32 {
    if a.is_nan() {
        return a;
    }
    if a == 0.0 && b == 0.0 && a.is_sign_negative() {
        return b;
    }
    if a >= b { a } else { b }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn java_min_and_max_propagate_nan_and_prefer_negative_zero() {
        assert_eq!(java_min(3.0, 5.0), 3.0);
        assert_eq!(java_max(3.0, 5.0), 5.0);
        assert!(java_min(3.0, f64::NAN).is_nan());
        assert!(java_min(f64::NAN, 3.0).is_nan());
        assert!(java_max(3.0, f64::NAN).is_nan());
        assert!(java_max(f64::NAN, 3.0).is_nan());
        assert_eq!(3.0_f64.min(f64::NAN), 3.0); 
        assert!(java_min(0.0, -0.0).is_sign_negative());
        assert!(java_min(-0.0, 0.0).is_sign_negative());
        assert!(java_max(0.0, -0.0).is_sign_positive());
        assert!(java_max(-0.0, 0.0).is_sign_positive());
    }

    #[test]
    fn the_f32_siblings_answer_exactly_what_the_jdk_answers() {
        assert_eq!(java_min_f32(3.0, 5.0), 3.0);
        assert_eq!(java_max_f32(3.0, 5.0), 5.0);
        assert!(java_min_f32(3.0, f32::NAN).is_nan());
        assert!(java_min_f32(f32::NAN, 3.0).is_nan());
        assert!(java_max_f32(3.0, f32::NAN).is_nan());
        assert!(java_max_f32(f32::NAN, 3.0).is_nan());
        assert_eq!(3.0_f32.min(f32::NAN), 3.0); 
        assert!(java_min_f32(0.0, -0.0).is_sign_negative());
        assert!(java_min_f32(-0.0, 0.0).is_sign_negative());
        assert!(java_max_f32(0.0, -0.0).is_sign_positive());
        assert!(java_max_f32(-0.0, 0.0).is_sign_positive());
    }

    #[test]
    fn java_double_min_value_is_the_smallest_subnormal() {
        assert_eq!(JAVA_DOUBLE_MIN_VALUE, 4.9E-324);
        assert_eq!(JAVA_DOUBLE_MIN_VALUE.to_bits(), 1);
        assert_eq!(
            JAVA_DOUBLE_MIN_VALUE.partial_cmp(&f64::MIN_POSITIVE),
            Some(std::cmp::Ordering::Less)
        );
        assert_eq!(
            JAVA_DOUBLE_MIN_VALUE.partial_cmp(&0.0),
            Some(std::cmp::Ordering::Greater)
        );
        assert_eq!(f64::MIN_POSITIVE, 2.2250738585072014E-308);
    }

    #[test]
    fn java_round_rounds_half_up_not_away_from_zero() {
        assert_eq!(java_round(1.5), 2);
        assert_eq!(java_round(2.5), 3);
        assert_eq!(java_round(-1.5), -1); 
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
