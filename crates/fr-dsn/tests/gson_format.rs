//! `to_gson_string_pretty`/`JavaNumberFormatter` parity with `GsonProvider.GSON` — moved here from
//! `fr-settings` in Plan 5 (`docs/superpowers/plans/2026-08-29-plan-5-drc.md` ruling 7). This is a
//! generic-`Serialize` restatement of the RouterSettings-specific vectors already pinned in
//! `crates/fr-settings/tests/json.rs`; that file is the byte-for-byte guard for the move
//! (unedited, per Task 1's brief) and stays the authority for the full `RouterSettings` shape.
//! This file exercises the same four points against small ad hoc types instead, so `fr-dsn` has
//! its own regression coverage independent of `fr-settings`.

use fr_dsn::format::json::to_gson_string_pretty;
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

/// `Double.toString`, not Rust's shortest-round-trip formatter: `1.0e7` prints `1.0E7` (not
/// `10000000.0`) and `0.0001` prints `1.0E-4` (not `0.0001`), because both fall outside Java's
/// plain-decimal range `[1e-3, 1e7)`.
#[test]
fn f64_fields_use_java_double_to_string() {
    let value = json!({"a": 1.0e7_f64, "b": 0.0001_f64});
    let text = to_gson_string_pretty(&value).expect("serialises");
    assert_eq!(text, "{\n  \"a\": 1.0E7,\n  \"b\": 1.0E-4\n}");
}

/// `Float.toString`: an `f32` field is routed through the float (not double) formatter.
#[test]
fn f32_field_uses_java_float_to_string() {
    let text = to_gson_string_pretty(&WithF32 {
        value: 3.402_823_5e38_f32,
    })
    .expect("serialises");
    assert_eq!(text, "{\n  \"value\": 3.4028235E38\n}");
}

/// `Gson.toJson` throws `IllegalArgumentException` for a non-finite float (no
/// `serializeSpecialFloatingPointValues()`); the port returns `Err` at the same point.
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

/// `U+2028`/`U+2029` are escaped (legal in JSON, illegal in JavaScript); `disableHtmlEscaping()`
/// leaves `<`, `>`, `&`, `'` raw.
#[test]
fn line_separators_are_escaped_and_the_html_set_is_not() {
    let value = WithString {
        value: "a\u{2028}b\u{2029}c<>&'".to_string(),
    };
    let text = to_gson_string_pretty(&value).expect("serialises");
    assert_eq!(text, "{\n  \"value\": \"a\\u2028b\\u2029c<>&'\"\n}");
}

/// Two-space indent, `": "` after the key, no trailing newline — `PrettyFormatter`'s own
/// behaviour, unchanged by the Java number/escape overrides.
#[test]
fn pretty_printing_matches_gsons_json_writer() {
    let text = to_gson_string_pretty(&json!({"a": 1, "b": [1, 2]})).expect("serialises");
    assert_eq!(text, "{\n  \"a\": 1,\n  \"b\": [\n    1,\n    2\n  ]\n}");
    assert!(!text.ends_with('\n'));
}
