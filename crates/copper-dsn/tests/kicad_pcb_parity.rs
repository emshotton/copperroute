use copper_dsn::kicad::NetClassJson;
use copper_dsn::kicad::pcb::read_pcb;
use std::path::{Path, PathBuf};
use std::process::Command;

const TOLERANCE: f64 = 0.005;

fn node_available() -> bool {
    Command::new("node")
        .arg("--version")
        .output()
        .is_ok_and(|out| out.status.success())
}

fn boards() -> Vec<PathBuf> {
    let root = testkit::workspace_root();
    let mut paths = vec![root.join("web/example.kicad_pcb")];
    let examples = root.join("web/examples");
    if let Ok(entries) = std::fs::read_dir(&examples) {
        for entry in entries.flatten() {
            let dir = entry.path();
            if let Ok(inner) = std::fs::read_dir(&dir) {
                for file in inner.flatten() {
                    let path = file.path();
                    if path.extension().is_some_and(|e| e == "kicad_pcb") {
                        paths.push(path);
                    }
                }
            }
        }
    }
    paths.retain(|p| p.exists());
    paths.sort();
    paths
}

fn javascript_board(path: &Path, name: &str) -> serde_json::Value {
    let harness =
        testkit::workspace_root().join("crates/copper-dsn/tests/data/kicad_pcb_parity.mjs");
    let out = Command::new("node")
        .arg(&harness)
        .arg(path)
        .arg(name)
        .output()
        .expect("node runs");
    assert!(
        out.status.success(),
        "the JS adapter failed on {}: {}",
        path.display(),
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).expect("the JS emits JSON")
}

/// The centroid of a polygon (an array of `{x, y}` objects), scaled and rounded to thousandths
/// of a millimetre so it can serve as a sort key.
fn polygon_key(polygon: &serde_json::Value) -> (i64, i64) {
    let points = polygon.as_array().map(Vec::as_slice).unwrap_or_default();
    let get = |p: &serde_json::Value, field: &str| {
        p.get(field)
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0)
    };
    let (sum_x, sum_y): (f64, f64) = points.iter().fold((0.0, 0.0), |(sx, sy), p| {
        (sx + get(p, "x"), sy + get(p, "y"))
    });
    let n = points.len().max(1) as f64;
    (
        (sum_x / n * 1000.0).round() as i64,
        (sum_y / n * 1000.0).round() as i64,
    )
}

/// A key missing on one side of an object comparison is allowed only when the *parent object's*
/// path and the key together name one of these exact fields, and the side that has it holds the
/// value listed: each is a documented, one-way schema gap between `importBoard`'s DTO and
/// `read_pcb`'s, not a divergence in what either side computed. Matching on the parent path as
/// well as the key keeps this from also swallowing an unrelated field that happens to share a
/// name, such as `NetClassJson`'s own (already-optional, already-matching) `viaInPadAllowed`.
/// `parent_path` uses `compare`'s `::` field separator, which cannot collide with a filename's
/// own dot the way a plain `.` join would.
///
/// - top-level `viaInPadAllowed` (bool, KiCadBoardJson): `read_pcb` shares its `KiCadBoardJson`
///   with `kicad::writer::write`, whose byte-for-byte golden test depends on the field being
///   omitted when false; `importBoard` always writes it.
/// - `nets[N].containsPlane` (bool, NetJson): the same DTO is shared with `write`, where a true
///   value is load-bearing output; `importBoard` never populates this at all, so on the import
///   path it is always false on the Rust side.
/// - top-level `clearanceRules` (array, KiCadBoardJson): `read_board_json` treats a missing list
///   as a hard error (it mimics a Java `NullPointerException`), so `read_pcb` must always produce
///   `[]`; `importBoard`'s DTO has no such field.
fn allowed_gap(parent_path: &str, key: &str, present: &serde_json::Value) -> bool {
    use serde_json::Value;
    let top_level = !parent_path.contains("::");
    let parent_is_a_net = parent_path
        .rsplit("::")
        .next()
        .unwrap_or("")
        .starts_with("nets[");
    match (key, present) {
        ("viaInPadAllowed", Value::Bool(false)) if top_level => true,
        ("containsPlane", Value::Bool(false)) if parent_is_a_net => true,
        ("clearanceRules", Value::Array(items)) if top_level => items.is_empty(),
        _ => false,
    }
}

/// Recursively diffs two JSON values. Numbers compare within `TOLERANCE`; every other type
/// requires exact equality. Objects are compared over the union of both sides' keys, so a field
/// present on only one side is always reported, in either direction, unless `allowed_gap` names
/// it as one of the documented DTO-shape exceptions. Object fields are joined onto the path with
/// `::` (see `allowed_gap`); array elements get a bracketed index suffixed directly.
///
/// `outline::cutouts` is compared by sorting each side by polygon centroid first: the order the
/// board's internal cutouts come out in depends on the exact floating-point value computed for
/// each polygon's area, which trigonometric functions in V8 and Rust's libm do not guarantee to
/// agree on to the last bit, so the two sides can legitimately assign the same set of holes to
/// different indices.
fn compare(
    path: &str,
    want: &serde_json::Value,
    got: &serde_json::Value,
    failures: &mut Vec<String>,
) {
    use serde_json::Value;
    match (want, got) {
        (Value::Number(a), Value::Number(b)) => {
            let (a, b) = (
                a.as_f64().unwrap_or_default(),
                b.as_f64().unwrap_or_default(),
            );
            if (a - b).abs() > TOLERANCE {
                failures.push(format!("{path}: js {a}, rust {b}"));
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            if a.len() != b.len() {
                failures.push(format!(
                    "{path}: js has {} entries, rust {}",
                    a.len(),
                    b.len()
                ));
                return;
            }
            if path.ends_with("::outline::cutouts") {
                let mut a: Vec<&Value> = a.iter().collect();
                let mut b: Vec<&Value> = b.iter().collect();
                a.sort_by_key(|v| polygon_key(v));
                b.sort_by_key(|v| polygon_key(v));
                for (i, (x, y)) in a.iter().zip(b).enumerate() {
                    compare(&format!("{path}[{i}]"), x, y, failures);
                }
                return;
            }
            for (i, (x, y)) in a.iter().zip(b).enumerate() {
                compare(&format!("{path}[{i}]"), x, y, failures);
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            let mut keys: Vec<&String> = a.keys().chain(b.keys()).collect();
            keys.sort_unstable();
            keys.dedup();
            for key in keys {
                match (a.get(key), b.get(key)) {
                    (Some(x), Some(y)) => compare(&format!("{path}::{key}"), x, y, failures),
                    (Some(x), None) if allowed_gap(path, key, x) => {}
                    (Some(x), None) => {
                        failures.push(format!("{path}::{key}: js {x}, missing from rust"))
                    }
                    (None, Some(y)) if allowed_gap(path, key, y) => {}
                    (None, Some(y)) => {
                        failures.push(format!("{path}::{key}: missing from js, rust {y}"))
                    }
                    (None, None) => unreachable!("key came from one of the two maps"),
                }
            }
        }
        _ if want != got => failures.push(format!("{path}: js {want}, rust {got}")),
        _ => {}
    }
}

#[test]
fn the_port_matches_the_javascript_adapter() {
    if !node_available() {
        eprintln!("skipping: node is not available");
        return;
    }
    let defaults = NetClassJson {
        name: Some("Default".to_string()),
        clearance: 0.2,
        traceWidth: 0.25,
        viaDiameter: 0.6,
        viaDrill: 0.3,
        ..Default::default()
    };
    let mut failures = Vec::new();
    let boards = boards();
    assert!(
        !boards.is_empty(),
        "no example boards were found to compare"
    );
    for path in boards {
        let stem = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let text = std::fs::read_to_string(&path).expect("the board reads");
        let imported = read_pcb(&text, &stem, &defaults).expect("the board imports");
        let rust = serde_json::to_value(&imported.board).expect("the DTO serialises");
        let js = javascript_board(&path, &stem);
        compare(&stem, &js, &rust, &mut failures);
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

#[cfg(test)]
mod comparator_self_test {
    use super::compare;
    use serde_json::json;

    fn diff(want: serde_json::Value, got: serde_json::Value) -> Vec<String> {
        let mut failures = Vec::new();
        compare("t", &want, &got, &mut failures);
        failures
    }

    #[test]
    fn a_field_only_the_js_side_has_is_reported() {
        assert_eq!(
            diff(json!({"a": 1}), json!({})),
            ["t::a: js 1, missing from rust"]
        );
    }

    #[test]
    fn a_field_only_the_rust_side_has_is_reported() {
        assert_eq!(
            diff(json!({}), json!({"a": 1})),
            ["t::a: missing from js, rust 1"]
        );
    }

    #[test]
    fn a_present_value_is_never_equal_to_a_missing_one_even_when_falsy() {
        assert_eq!(
            diff(json!({"a": 0}), json!({})),
            ["t::a: js 0, missing from rust"]
        );
        assert_eq!(
            diff(json!({"a": false}), json!({})),
            ["t::a: js false, missing from rust"]
        );
        assert_eq!(
            diff(json!({"a": []}), json!({})),
            ["t::a: js [], missing from rust"]
        );
    }

    #[test]
    fn arrays_compare_element_by_element_not_as_sets() {
        assert_eq!(
            diff(json!([1, 2]), json!([2, 1])),
            ["t[0]: js 1, rust 2", "t[1]: js 2, rust 1"]
        );
    }

    #[test]
    fn a_length_mismatch_is_reported_without_a_spurious_per_element_diff() {
        assert_eq!(
            diff(json!([1, 2]), json!([1])),
            ["t: js has 2 entries, rust 1"]
        );
    }

    #[test]
    fn cutouts_tolerate_reordering_but_not_a_changed_polygon() {
        let a = json!({"outline": {"cutouts": [[{"x": 0, "y": 0}], [{"x": 10, "y": 10}]]}});
        let b = json!({"outline": {"cutouts": [[{"x": 10, "y": 10}], [{"x": 0, "y": 0}]]}});
        assert!(diff(a, b).is_empty(), "reordering alone must not fail");

        let c = json!({"outline": {"cutouts": [[{"x": 0, "y": 0}], [{"x": 10, "y": 11}]]}});
        let d = json!({"outline": {"cutouts": [[{"x": 10, "y": 10}], [{"x": 0, "y": 0}]]}});
        assert_eq!(
            diff(c, d),
            ["t::outline::cutouts[1][0]::y: js 11, rust 10"],
            "a real change in a cutout must still be caught after sorting"
        );
    }

    #[test]
    fn the_documented_dto_gaps_are_narrow_to_their_own_field_and_parent() {
        // netClasses[N].viaInPadAllowed is a different, already-Option field that both sides
        // already agree on by omitting; the top-level exception must not reach it.
        let want = json!({"netClasses": [{"name": "Default"}]});
        let got = json!({"netClasses": [{"name": "Default", "viaInPadAllowed": false}]});
        assert_eq!(
            diff(want, got),
            ["t::netClasses[0]::viaInPadAllowed: missing from js, rust false"],
            "containsPlane's exemption for nets[N] must not also cover netClasses[N]"
        );
    }
}
