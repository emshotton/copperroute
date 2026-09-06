use fr_dsn::format::double::{
    format_double, format_fixed, format_float, format_placement_rotation,
};

#[test]
fn double_to_string_special_values() {
    assert_eq!(format_double(f64::NAN), "NaN");
    assert_eq!(format_double(f64::INFINITY), "Infinity");
    assert_eq!(format_double(f64::NEG_INFINITY), "-Infinity");
    assert_eq!(format_double(0.0), "0");
    assert_eq!(format_double(-0.0), "-0");
}

#[test]
fn double_to_string_plain_decimal_range() {
    assert_eq!(format_double(1.0), "1");
    assert_eq!(format_double(5.0), "5");
    assert_eq!(format_double(-5.0), "-5");
    assert_eq!(format_double(0.5), "0.5");
    assert_eq!(format_double(100.0), "100");
    assert_eq!(format_double(1e6), "1000000");
    assert_eq!(format_double(9999999.0), "9999999");
    assert_eq!(format_double(1234567.0), "1234567");
    assert_eq!(format_double(0.001), "0.001");
    assert_eq!(format_double(0.1), "0.1");
    assert_eq!(format_double(0.3), "0.3");
    assert_eq!(format_double(123456.789), "123456.789");
    assert_eq!(format_double(-1234.5), "-1234.5");
    assert_eq!(format_double(1.0 / 3.0), "0.3333333333333333");
    assert_eq!(format_double(100.0 / 3.0), "33.333333333333336");
}

#[test]
fn double_to_string_never_uses_scientific_notation() {
    assert_eq!(format_double(1.0e7), "10000000");
    assert_eq!(format_double(12345678.0), "12345678");
    assert_eq!(format_double(12345670.0), "12345670");
    assert_eq!(format_double(2.0e7), "20000000");
    assert_eq!(format_double(9.9e-4), "0.00099");
    assert_eq!(format_double(1.0e-4), "0.0001");
    assert_eq!(format_double(3.0e-7), "0.0000003");
    assert_eq!(format_double(1e23), "100000000000000000000000");
    assert_eq!(format_double(f64::MAX), "179769313486231570000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000");
    assert_eq!(
        format_double(2.2250738585072014e-308),
        "0.000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000022250738585072014"
    );
}

#[test]
fn double_to_string_subnormals_print_as_plain_decimals() {
    assert_eq!(format_double(1.0e-323), "0.00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001");
    assert_eq!(format_double(4.9e-324), "0.000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000005");
    assert_eq!(format_double(f64::from_bits(1)), "0.000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000005");
}

#[test]
fn to_string_breaks_exact_ties_to_an_even_last_digit() {
    let tied = f64::from_bits(0x430e_0351_1748_5bb2);
    assert_eq!(format!("{tied:e}"), "1.0559870148965023e15");
    assert_eq!(format_double(tied), "1055987014896502.3");
    assert_eq!(format_double(-tied), "-1055987014896502.3");

    let tied_float = f32::from_bits(0x4825_ebe8);
    assert_eq!(format!("{tied_float:e}"), "1.6990363e5");
    assert_eq!(format_float(tied_float), "169903.63");

    assert_eq!(format_double(0.3), "0.3");
    assert_eq!(format_double(2.675), "2.675");
    assert_eq!(format_double(1.5), "1.5");
    assert_eq!(format_double(0.125), "0.125");
}

#[test]
fn float_to_string_is_not_the_widened_double() {
    assert_eq!(format_float(0.1f32), "0.1");
    assert_eq!(format_double(0.1f32 as f64), "0.10000000149011612");
    assert_eq!(format_float(1.0f32 / 3.0f32), "0.33333334");
    assert_eq!(format_float((100.0f64 / 3.0) as f32), "33.333332");
    assert_eq!(format_float(0.0f32), "0");
    assert_eq!(format_float(-0.0f32), "-0");
    assert_eq!(format_float(1.0f32), "1");
    assert_eq!(format_float(1e7f32), "10000000");
    assert_eq!(format_float(1.2345678e7f32), "12345678");
    assert_eq!(format_float(1e-4f32), "0.0001");
    assert_eq!(format_float(9.9e-4f32), "0.00099");
    assert_eq!(format_float(f32::MAX), "340282350000000000000000000000000000000");
    assert_eq!(format_float(f32::from_bits(1)), "0.000000000000000000000000000000000000000000001");
    assert_eq!(format_float(f32::NAN), "NaN");
    assert_eq!(format_float(f32::INFINITY), "Infinity");
    assert_eq!(format_float(f32::NEG_INFINITY), "-Infinity");
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
fn fixed_format_matches_std_precision_rounding() {
    assert_eq!(format!("{:.3}", 0.0625f64), "0.062");
    assert_eq!(format_fixed(0.0625, 3), "0.062");
    assert_eq!(format!("{:.0}", 2.5f64), "2");
    assert_eq!(format_fixed(2.5, 0), "2");
    assert_eq!(format_fixed(-2.5, 0), "-2");
    assert_eq!(format!("{:.2}", 8.475f64), "8.47");
    assert_eq!(format_fixed(8.475, 2), "8.47");
    assert_eq!(format_fixed(1.005, 2), "1.00");
    assert_eq!(format_fixed(0.1235, 2), "0.12");
    assert_eq!(format_fixed(0.1235, 3), "0.123");
    assert_eq!(format!("{:.3}", 0.1235f64), "0.123");
    assert_eq!(format_fixed(-0.0, 0), "-0");
    assert_eq!(format_fixed(-0.0, 3), "-0.000");
    assert_eq!(format_fixed(0.0, 3), "0.000");
    assert_eq!(format_fixed(f64::NAN, 3), "NaN");
    assert_eq!(format_fixed(f64::INFINITY, 3), "Infinity");
    assert_eq!(format_fixed(f64::NEG_INFINITY, 0), "-Infinity");
    assert_eq!(format_fixed(1e18, 0), "1000000000000000000");
    assert_eq!(format_fixed(1.0e23, 3), "99999999999999991611392.000");
    assert_eq!(format_fixed(0.4, 0), "0");
    assert_eq!(format_fixed(0.5, 0), "0");
}
