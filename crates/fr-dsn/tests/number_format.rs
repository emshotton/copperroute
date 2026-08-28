//! JVM-verified test vectors for the Java number-formatting primitives every DSN/SES writer
//! depends on: `java.lang.Double.toString`, `java.lang.Float.toString`, `Math.rint`,
//! `Math.round`, and `SesWriter.formatPlacementRotation` (SesWriter.java:259-269).
//!
//! Every expectation below was re-derived for this task with a small headless driver on
//! **JDK 25** (`/opt/homebrew/opt/openjdk@25`), not merely read off the Java source. The
//! randomised counterpart of this file is the `p3t2` differential driver
//! (`scripts/differential/{java/P3T2.java,rust/src/bin/p3t2.rs}`), which walks tens of millions
//! of doubles through the real `Double.toString`/`Float.toString`/`formatPlacementRotation`.

use fr_dsn::format::double::{
    format_placement_rotation, java_double_to_string, java_float_to_string, java_rint, java_round,
    java_round_to_int,
};

#[test]
fn double_to_string_special_values() {
    assert_eq!(java_double_to_string(f64::NAN), "NaN");
    assert_eq!(java_double_to_string(f64::INFINITY), "Infinity");
    assert_eq!(java_double_to_string(f64::NEG_INFINITY), "-Infinity");
    // The sign of zero comes from the sign *bit*, so `-0.0` is negative.
    assert_eq!(java_double_to_string(0.0), "0.0");
    assert_eq!(java_double_to_string(-0.0), "-0.0");
}

#[test]
fn double_to_string_plain_decimal_range() {
    assert_eq!(java_double_to_string(1.0), "1.0");
    assert_eq!(java_double_to_string(5.0), "5.0");
    assert_eq!(java_double_to_string(-5.0), "-5.0");
    assert_eq!(java_double_to_string(0.5), "0.5");
    assert_eq!(java_double_to_string(100.0), "100.0");
    assert_eq!(java_double_to_string(1e6), "1000000.0");
    assert_eq!(java_double_to_string(9999999.0), "9999999.0");
    assert_eq!(java_double_to_string(1234567.0), "1234567.0");
    assert_eq!(java_double_to_string(0.001), "0.001");
    assert_eq!(java_double_to_string(0.1), "0.1");
    assert_eq!(java_double_to_string(0.3), "0.3");
    assert_eq!(java_double_to_string(123456.789), "123456.789");
    assert_eq!(java_double_to_string(-1234.5), "-1234.5");
    assert_eq!(java_double_to_string(1.0 / 3.0), "0.3333333333333333");
    assert_eq!(java_double_to_string(100.0 / 3.0), "33.333333333333336");
}

#[test]
fn double_to_string_scientific_range() {
    // The switch is at 10^7 on the high side and 10^-3 on the low side.
    assert_eq!(java_double_to_string(1.0e7), "1.0E7");
    assert_eq!(java_double_to_string(12345678.0), "1.2345678E7");
    assert_eq!(java_double_to_string(12345670.0), "1.234567E7");
    assert_eq!(java_double_to_string(2.0e7), "2.0E7");
    assert_eq!(java_double_to_string(9.9e-4), "9.9E-4");
    assert_eq!(java_double_to_string(1.0e-4), "1.0E-4");
    assert_eq!(java_double_to_string(3.0e-7), "3.0E-7");
    assert_eq!(java_double_to_string(1e23), "1.0E23");
    assert_eq!(java_double_to_string(f64::MAX), "1.7976931348623157E308");
    assert_eq!(
        java_double_to_string(2.2250738585072014e-308),
        "2.2250738585072014E-308"
    );
}

/// The subnormal tail is where Java's "at least two significant digits" rule bites: Rust's
/// shortest round-trip repr of `Double.MIN_VALUE` is the single digit `5e-324`, but Java's spec
/// widens the candidate set to length 1 *or* 2 whenever the minimal length is 1 and then picks
/// the decimal closest to the actual value — `4.9E-324`.
#[test]
fn double_to_string_subnormals_take_two_significant_digits() {
    // Note: the *literal* `1.0e-323` is not representable; the nearest double prints `9.9E-324`.
    assert_eq!(java_double_to_string(1.0e-323), "9.9E-324");
    assert_eq!(java_double_to_string(4.9e-324), "4.9E-324");
    assert_eq!(java_double_to_string(f64::from_bits(1)), "4.9E-324");
}

/// Java breaks an exact tie between the two shortest decimals towards an **even** last digit
/// (the `Double.toString` spec: "if there are two, the one whose least significant digit is
/// even"); Rust's shortest formatter picks the other one. Both rows below came out of the `p3t2`
/// differential's first smoke run, and both are exact halfway cases:
/// `1055987014896502.25` and `169903.625`.
#[test]
fn to_string_breaks_exact_ties_to_an_even_last_digit() {
    let tied = f64::from_bits(0x430e_0351_1748_5bb2);
    assert_eq!(format!("{tied:e}"), "1.0559870148965023e15");
    assert_eq!(java_double_to_string(tied), "1.0559870148965022E15");
    assert_eq!(java_double_to_string(-tied), "-1.0559870148965022E15");

    let tied_float = f32::from_bits(0x4825_ebe8); // exactly 169903.625
    assert_eq!(format!("{tied_float:e}"), "1.6990363e5");
    assert_eq!(java_float_to_string(tied_float), "169903.62");

    // The correction must not fire where the shorter candidate does not round-trip: `0.3` is
    // *not* a tie (its exact expansion runs on well past one extra digit), and neither is any
    // value whose exact expansion is too long to end in a lone trailing `5`.
    assert_eq!(java_double_to_string(0.3), "0.3");
    assert_eq!(java_double_to_string(2.675), "2.675");
    assert_eq!(java_double_to_string(1.5), "1.5");
    assert_eq!(java_double_to_string(0.125), "0.125");
}

#[test]
fn float_to_string_is_not_the_widened_double() {
    // `Float.toString` runs the shortest-round-trip search on the *float*, which yields far fewer
    // digits than `Double.toString` of the same value widened to a double.
    assert_eq!(java_float_to_string(0.1f32), "0.1");
    assert_eq!(java_double_to_string(0.1f32 as f64), "0.10000000149011612");
    assert_eq!(java_float_to_string(1.0f32 / 3.0f32), "0.33333334");
    assert_eq!(java_float_to_string((100.0f64 / 3.0) as f32), "33.333332");
    assert_eq!(java_float_to_string(0.0f32), "0.0");
    assert_eq!(java_float_to_string(-0.0f32), "-0.0");
    assert_eq!(java_float_to_string(1.0f32), "1.0");
    assert_eq!(java_float_to_string(1e7f32), "1.0E7");
    assert_eq!(java_float_to_string(1.2345678e7f32), "1.2345678E7");
    assert_eq!(java_float_to_string(1e-4f32), "1.0E-4");
    assert_eq!(java_float_to_string(9.9e-4f32), "9.9E-4");
    assert_eq!(java_float_to_string(f32::MAX), "3.4028235E38");
    assert_eq!(java_float_to_string(f32::from_bits(1)), "1.4E-45");
    assert_eq!(java_float_to_string(f32::NAN), "NaN");
    assert_eq!(java_float_to_string(f32::INFINITY), "Infinity");
    assert_eq!(java_float_to_string(f32::NEG_INFINITY), "-Infinity");
}

#[test]
fn java_round_rounds_half_towards_positive_infinity() {
    assert_eq!(java_round(2.5), 3);
    assert_eq!(java_round(-2.5), -2);
    assert_eq!(java_round(0.5), 1);
    assert_eq!(java_round(-0.5), 0);
    assert_eq!(java_round(1.5), 2);
    assert_eq!(java_round(-1.5), -1);
    assert_eq!(java_round(-0.4), 0);
    assert_eq!(java_round(2.6), 3);
    assert_eq!(java_round(-2.6), -3);
    assert_eq!(java_round(-0.0), 0);
    assert_eq!(java_round(f64::NAN), 0);
    assert_eq!(java_round(1e18), 1_000_000_000_000_000_000);
    // The JDK-7 fix (JDK-6430675): a naive `floor(x + 0.5)` answers 1 here.
    assert_eq!(java_round(0.49999999999999994), 0);
}

/// Java's `(int) Math.round(double)` narrows a `long` to an `int` by *truncation*, not by
/// saturation: `(int) 1000000000000000000L` is `-1486618624`. Rust's `as` on `i64 -> i32` wraps
/// identically.
#[test]
fn java_round_to_int_truncates_the_long_rather_than_saturating() {
    assert_eq!(java_round_to_int(1e18), -1_486_618_624);
    assert_eq!(java_round_to_int(-1e18), 1_486_618_624);
    assert_eq!(java_round_to_int(2.5), 3);
    assert_eq!(java_round_to_int(-2.5), -2);
    assert_eq!(java_round_to_int(f64::NAN), 0);
}

#[test]
fn java_rint_breaks_ties_to_even() {
    assert_eq!(java_rint(2.5), 2.0);
    assert_eq!(java_rint(-2.5), -2.0);
    assert_eq!(java_rint(0.5), 0.0);
    assert_eq!(java_rint(-0.5), -0.0);
    assert_eq!(java_rint(1.5), 2.0);
    // `rint(-0.5)` is *negative* zero, which `assert_eq!` alone would not catch.
    assert!(java_rint(-0.5).is_sign_negative());
    assert!(java_rint(0.5).is_sign_positive());
    assert!(java_rint(f64::NAN).is_nan());
}

#[test]
fn placement_rotation_matches_ses_writer() {
    assert_eq!(format_placement_rotation(0.0), "0");
    assert_eq!(format_placement_rotation(90.0), "90");
    assert_eq!(format_placement_rotation(180.0), "180");
    assert_eq!(format_placement_rotation(270.0), "270");
    assert_eq!(format_placement_rotation(-90.0), "-90");
    assert_eq!(format_placement_rotation(45.5), "45.5");
    assert_eq!(format_placement_rotation(12.3456), "12.346");
    assert_eq!(format_placement_rotation(5.0e-4), "0");
    assert_eq!(format_placement_rotation(359.9995), "360");
    assert_eq!(format_placement_rotation(1.0005), "1");
}

/// `String.format(Locale.ENGLISH, "%.Nf", d)` does **not** round the double's exact binary value:
/// `java.util.Formatter` first takes the *shortest round-trip decimal digits* (the same digits
/// `Double.toString` prints) and then rounds those `HALF_UP`. Both halves of that differ from
/// Rust's `{:.N}`, which rounds the exact binary value half-to-even. These rows pin the
/// difference; without the port's own formatter they would all come out one ulp low.
#[test]
fn fixed_format_rounds_the_shortest_digits_half_up() {
    // HALF_UP vs half-to-even on an exactly-representable tie: `0.0625` is exact, and Java
    // answers `0.063` where Rust's `format!("{:.3}", 0.0625)` answers `0.062`.
    assert_eq!(format!("{:.3}", 0.0625f64), "0.062");
    assert_eq!(
        fr_dsn::format::double::java_format_fixed(0.0625, 3),
        "0.063"
    );
    assert_eq!(format!("{:.0}", 2.5f64), "2");
    assert_eq!(fr_dsn::format::double::java_format_fixed(2.5, 0), "3");
    assert_eq!(fr_dsn::format::double::java_format_fixed(-2.5, 0), "-3");
    // Shortest-digits vs exact expansion: `8.475` is really 8.47499999999999964..., so rounding
    // the *exact* value at two decimals gives 8.47; Java rounds the digits `8475` and gives 8.48.
    assert_eq!(format!("{:.2}", 8.475f64), "8.47");
    assert_eq!(fr_dsn::format::double::java_format_fixed(8.475, 2), "8.48");
    assert_eq!(fr_dsn::format::double::java_format_fixed(1.005, 2), "1.01");
    // ...and the same rule rounds *down* here, where the exact expansion would too.
    assert_eq!(fr_dsn::format::double::java_format_fixed(0.1235, 2), "0.12");
    assert_eq!(
        fr_dsn::format::double::java_format_fixed(0.1235, 3),
        "0.124"
    );
    assert_eq!(format!("{:.3}", 0.1235f64), "0.123");
    // Signed zero, the non-finite triple, and a value far outside the shortest digits' span.
    assert_eq!(fr_dsn::format::double::java_format_fixed(-0.0, 0), "-0");
    assert_eq!(fr_dsn::format::double::java_format_fixed(-0.0, 3), "-0.000");
    assert_eq!(fr_dsn::format::double::java_format_fixed(0.0, 3), "0.000");
    assert_eq!(
        fr_dsn::format::double::java_format_fixed(f64::NAN, 3),
        "NaN"
    );
    assert_eq!(
        fr_dsn::format::double::java_format_fixed(f64::INFINITY, 3),
        "Infinity"
    );
    assert_eq!(
        fr_dsn::format::double::java_format_fixed(f64::NEG_INFINITY, 0),
        "-Infinity"
    );
    assert_eq!(
        fr_dsn::format::double::java_format_fixed(1e18, 0),
        "1000000000000000000"
    );
    assert_eq!(
        fr_dsn::format::double::java_format_fixed(1.0e23, 3),
        "100000000000000000000000.000"
    );
    assert_eq!(fr_dsn::format::double::java_format_fixed(0.4, 0), "0");
    assert_eq!(fr_dsn::format::double::java_format_fixed(0.5, 0), "1");
}
