use copper_dsn::format::json::to_gson_string_pretty;
use serde::Serialize;
use serde_json::json;

#[derive(Serialize)]
struct WithF32 {
    value: f32,
}

#[derive(Serialize)]
struct WithF64 {
    value: f64,
}

#[derive(Serialize)]
struct WithString {
    value: String,
}

#[test]
fn f64_fields_use_format_double() {
    let value = json!({"a": 1.0e7_f64, "b": 0.0001_f64});
    let text = to_gson_string_pretty(&value).expect("serialises");
    assert_eq!(text, "{\n  \"a\": 10000000,\n  \"b\": 0.0001\n}");
}

#[test]
fn f32_field_uses_format_float() {
    let text = to_gson_string_pretty(&WithF32 {
        value: 3.402_823_5e38_f32,
    })
    .expect("serialises");
    assert_eq!(
        text,
        "{\n  \"value\": 340282350000000000000000000000000000000\n}"
    );
}

#[test]
fn non_finite_f64_is_refused() {
    assert!(to_gson_string_pretty(&WithF64 { value: f64::NAN }).is_err());
    assert!(
        to_gson_string_pretty(&WithF64 {
            value: f64::INFINITY
        })
        .is_err()
    );
    assert!(
        to_gson_string_pretty(&WithF64 {
            value: f64::NEG_INFINITY
        })
        .is_err()
    );
}

#[test]
fn line_separators_are_escaped_and_the_html_set_is_not() {
    let value = WithString {
        value: "a\u{2028}b\u{2029}c<>&'".to_string(),
    };
    let text = to_gson_string_pretty(&value).expect("serialises");
    assert_eq!(text, "{\n  \"value\": \"a\\u2028b\\u2029c<>&'\"\n}");
}

#[test]
fn pretty_printing_matches_gsons_json_writer() {
    let text = to_gson_string_pretty(&json!({"a": 1, "b": [1, 2]})).expect("serialises");
    assert_eq!(text, "{\n  \"a\": 1,\n  \"b\": [\n    1,\n    2\n  ]\n}");
    assert!(!text.ends_with('\n'));
}
