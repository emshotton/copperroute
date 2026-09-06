use fr_dsn::format::double::{
    format_placement_rotation, java_double_to_string, java_float_to_string,
};

#[test]
fn double_to_string_special_values() {
    assert_eq!(java_double_to_string(f64::NAN), "NaN");
    assert_eq!(java_double_to_string(f64::INFINITY), "Infinity");
    assert_eq!(java_double_to_string(f64::NEG_INFINITY), "-Infinity");
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

#[test]
fn double_to_string_subnormals_take_two_significant_digits() {
    assert_eq!(java_double_to_string(1.0e-323), "9.9E-324");
    assert_eq!(java_double_to_string(4.9e-324), "4.9E-324");
    assert_eq!(java_double_to_string(f64::from_bits(1)), "4.9E-324");
}

#[test]
fn to_string_breaks_exact_ties_to_an_even_last_digit() {
    let tied = f64::from_bits(0x430e_0351_1748_5bb2);
    assert_eq!(format!("{tied:e}"), "1.0559870148965023e15");
    assert_eq!(java_double_to_string(tied), "1.0559870148965022E15");
    assert_eq!(java_double_to_string(-tied), "-1.0559870148965022E15");

    let tied_float = f32::from_bits(0x4825_ebe8);
    assert_eq!(format!("{tied_float:e}"), "1.6990363e5");
    assert_eq!(java_float_to_string(tied_float), "169903.62");

    assert_eq!(java_double_to_string(0.3), "0.3");
    assert_eq!(java_double_to_string(2.675), "2.675");
    assert_eq!(java_double_to_string(1.5), "1.5");
    assert_eq!(java_double_to_string(0.125), "0.125");
}

#[test]
fn float_to_string_is_not_the_widened_double() {
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

#[test]
fn fixed_format_rounds_the_shortest_digits_half_up() {
    assert_eq!(format!("{:.3}", 0.0625f64), "0.062");
    assert_eq!(
        fr_dsn::format::double::java_format_fixed(0.0625, 3),
        "0.063"
    );
    assert_eq!(format!("{:.0}", 2.5f64), "2");
    assert_eq!(fr_dsn::format::double::java_format_fixed(2.5, 0), "3");
    assert_eq!(fr_dsn::format::double::java_format_fixed(-2.5, 0), "-3");
    assert_eq!(format!("{:.2}", 8.475f64), "8.47");
    assert_eq!(fr_dsn::format::double::java_format_fixed(8.475, 2), "8.48");
    assert_eq!(fr_dsn::format::double::java_format_fixed(1.005, 2), "1.01");
    assert_eq!(fr_dsn::format::double::java_format_fixed(0.1235, 2), "0.12");
    assert_eq!(
        fr_dsn::format::double::java_format_fixed(0.1235, 3),
        "0.124"
    );
    assert_eq!(format!("{:.3}", 0.1235f64), "0.123");
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
