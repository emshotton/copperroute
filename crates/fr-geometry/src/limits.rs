use num_bigint::BigInt;

/// Coordinates with |value| above this promote to rational (BigInt) arithmetic.
pub const CRIT_INT: i32 = 33_554_432; // 2^25

/// The biggest double value (2^53), so that all integers smaller than this value are represented
/// exactly as double values.
pub const CRIT_DOUBLE: f64 = 9_007_199_254_740_992.0; // 2^53

/// Square root of 2.
pub const SQRT2: f64 = std::f64::consts::SQRT_2;

/// Java `Double.MIN_VALUE`: the smallest positive **subnormal** double, `4.9e-324`.
///
/// This is deliberately *not* `f64::MIN_POSITIVE`, which is the smallest positive *normal*
/// double (`2.2251e-308`) — a different number, and Java spells that one `Double.MIN_NORMAL`.
/// Needed because `TileShape.indexOfNearestCorner` seeds its running minimum with
/// `Double.MIN_VALUE` (TileShape.java:453).
pub const JAVA_DOUBLE_MIN_VALUE: f64 = f64::from_bits(1);

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

/// Java `Math.min(double, double)`, transcribed from the JDK:
///
/// ```java
/// public static double min(double a, double b) {
///   if (a != a) return a;                       // a is NaN
///   if ((a == 0.0d) && (b == 0.0d)
///       && (Double.doubleToRawLongBits(b) == negativeZeroDoubleBits)) return b;
///   return (a <= b) ? a : b;
/// }
/// ```
///
/// Rust's `f64::min` differs twice. It *absorbs* NaN — `x.min(f64::NAN) == x` (IEEE 754-2019
/// `minimumNumber`) — where Java propagates it, and when the two operands compare equal (`0.0`
/// against `-0.0`) it may return either operand, where Java always answers `-0.0`.
pub fn java_min(a: f64, b: f64) -> f64 {
    if a.is_nan() {
        return a;
    }
    if a == 0.0 && b == 0.0 && b.is_sign_negative() {
        return b;
    }
    // A NaN `b` falls through to here: `a <= b` is false, so `b` (the NaN) is returned.
    if a <= b { a } else { b }
}

/// Java `Math.max(double, double)`; the mirror of [`java_min`]. Note that the JDK tests the sign
/// bit of **`a`** here (not of `b` as in `min`):
///
/// ```java
/// public static double max(double a, double b) {
///   if (a != a) return a;
///   if ((a == 0.0d) && (b == 0.0d)
///       && (Double.doubleToRawLongBits(a) == negativeZeroDoubleBits)) return b;
///   return (a >= b) ? a : b;
/// }
/// ```
pub fn java_max(a: f64, b: f64) -> f64 {
    if a.is_nan() {
        return a;
    }
    if a == 0.0 && b == 0.0 && a.is_sign_negative() {
        return b;
    }
    if a >= b { a } else { b }
}

/// Java `Math.min(float, float)`; the `f32` sibling of [`java_min`], transcribed from the JDK:
///
/// ```java
/// public static float min(float a, float b) {
///   if (a != a) return a;                       // a is NaN
///   if ((a == 0.0f) && (b == 0.0f)
///       && (Float.floatToRawIntBits(b) == negativeZeroFloatBits)) return b;
///   return (a <= b) ? a : b;
/// }
/// ```
///
/// Same two divergences from `f32::min` as [`java_min`] has from `f64::min`: Rust absorbs a NaN
/// operand where Java propagates it, and for `0.0` against `-0.0` Rust may return either operand
/// where Java always answers `-0.0`.
pub fn java_min_f32(a: f32, b: f32) -> f32 {
    if a.is_nan() {
        return a;
    }
    if a == 0.0 && b == 0.0 && b.is_sign_negative() {
        return b;
    }
    // A NaN `b` falls through to here: `a <= b` is false, so `b` (the NaN) is returned.
    if a <= b { a } else { b }
}

/// Java `Math.max(float, float)`; the `f32` sibling of [`java_max`]. As in the `double` pair, the
/// JDK tests the sign bit of **`a`** here (not of `b` as in `min`) — and, in both, *returns* `b`:
///
/// ```java
/// public static float max(float a, float b) {
///   if (a != a) return a;
///   if ((a == 0.0f) && (b == 0.0f)
///       && (Float.floatToRawIntBits(a) == negativeZeroFloatBits)) return b;
///   return (a >= b) ? a : b;
/// }
/// ```
///
/// `BoardStatistics.getNormalizedScore`'s `Math.max(0, calculateScore / maximumScore)`
/// (`core/scoring/BoardStatistics.java:634`) is the caller this was written for: the quotient can
/// be NaN (an `Infinity` maximum against an `Infinity` penalty sum) and can underflow to `-0.0`,
/// and `Float.toString` renders both differences.
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
        // Java propagates NaN from either side; `f64::min`/`f64::max` would absorb it.
        assert!(java_min(3.0, f64::NAN).is_nan());
        assert!(java_min(f64::NAN, 3.0).is_nan());
        assert!(java_max(3.0, f64::NAN).is_nan());
        assert!(java_max(f64::NAN, 3.0).is_nan());
        assert_eq!(3.0_f64.min(f64::NAN), 3.0); // what Rust does instead
        // Java always answers -0.0 for min(±0.0, ∓0.0) and +0.0 for max; Rust's own min/max may
        // return either operand when they compare equal.
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
        assert_eq!(3.0_f32.min(f32::NAN), 3.0); // what Rust does instead
        // The signed-zero clauses, in the JDK's own asymmetry: `min` tests `b`'s sign bit and
        // `max` tests `a`'s, and **both return `b`**. Getting the mirror image wrong is silent
        // on every input but these four.
        assert!(java_min_f32(0.0, -0.0).is_sign_negative());
        assert!(java_min_f32(-0.0, 0.0).is_sign_negative());
        assert!(java_max_f32(0.0, -0.0).is_sign_positive());
        assert!(java_max_f32(-0.0, 0.0).is_sign_positive());
    }

    #[test]
    fn java_double_min_value_is_the_smallest_subnormal() {
        assert_eq!(JAVA_DOUBLE_MIN_VALUE, 4.9E-324);
        assert_eq!(JAVA_DOUBLE_MIN_VALUE.to_bits(), 1);
        // `assert_eq!` on the ordering rather than `assert!(a < b)`: the latter is a compile-time
        // constant and clippy's `assertions_on_constants` rejects it.
        assert_eq!(
            JAVA_DOUBLE_MIN_VALUE.partial_cmp(&f64::MIN_POSITIVE),
            Some(std::cmp::Ordering::Less)
        );
        assert_eq!(
            JAVA_DOUBLE_MIN_VALUE.partial_cmp(&0.0),
            Some(std::cmp::Ordering::Greater)
        );
        // Java's `Double.MIN_NORMAL` is Rust's `f64::MIN_POSITIVE`.
        assert_eq!(f64::MIN_POSITIVE, 2.2250738585072014E-308);
    }

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
