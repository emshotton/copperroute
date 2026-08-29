//! Gson parity for `RouterSettings`'s JSON in and out (Plan 4 Task 10).
//!
//! Every expected string below is a verbatim line of `tests/data/JProbe.java`'s transcript —
//! `GsonProvider.GSON` (`util/gson/GsonProvider.java:12-20`: pretty printing, HTML escaping off,
//! `Strictness.LENIENT`, `RouterSettingsTypeAdapterFactory` registered) run against the clone's
//! HEAD jar on JDK 25. The recorded command is in `tests/data/README.md`; the full transcript is
//! in `.superpowers/sdd/2026-08-28-plan-4-settings/task-10-report.md`.

use std::path::{Path, PathBuf};

use fr_settings::prelude::*;

#[path = "matrix/mod.rs"]
mod matrix;

// -------------------------------------------------------------------------------------------
// fixtures
// -------------------------------------------------------------------------------------------

/// `JProbe.java` block A: every field Gson round-trips set to a distinct value, plus all four
/// `transient` fields and the three `transient` `OptimizerSettings`/`ScoringSettings` fields set
/// to values that must not reach the output.
fn fully_populated() -> RouterSettings {
    let mut s = RouterSettings::new();
    s.set_layer_count(2);
    s.enabled = Some(true);
    s.algorithm = Some("freerouting-router-v19".to_string());
    s.copper_to_edge_clearance_um = Some(501.5);
    s.hole_clearance_um = Some(2.0);
    s.neck_width_um = Some(3.0);
    s.strict_drc = Some(true);
    s.job_timeout_string = Some("12:00:00".to_string());
    s.max_passes = Some(11);
    s.trace_pull_tight_accuracy = Some(12);
    s.vias_allowed = Some(false);
    s.automatic_neckdown = Some(false);
    s.max_threads = Some(13);
    s.result_json_path = Some("/tmp/result.json".to_string());
    // `transient` in Java — must not appear in the output.
    s.max_items = Some(14);
    s.save_intermediate_stages = Some(true);
    s.ignore_net_classes = Some(vec!["x".to_string(), "y".to_string()]);
    s.layers = Some(vec![
        LayerSettings::new(Some(false), Some(true)),
        LayerSettings::new(Some(true), Some(false)),
    ]);

    let f = s.fanout.as_mut().expect("new() allocates fanout");
    f.enabled = Some(false);
    f.max_passes = Some(21);
    f.max_items = Some(22);
    f.max_milliseconds_per_pin = Some(23);
    f.ripup_allowed = Some(false);
    f.min_escape_length_mm = Some(2.25);
    f.max_escape_length_mm = Some(4.75);
    f.start_via_diameter_mm = Some(0.5);
    f.end_via_diameter_mm = Some(0.75);
    f.pin_sorting_order = Some("inner_first".to_string());
    f.fallback_to_board_vias = Some(false);
    f.timeout_string = Some("00:01:00".to_string());

    let o = s.optimizer.as_mut().expect("new() allocates optimizer");
    o.enabled = Some(false);
    o.algorithm = Some("freerouting-optimizer-v19".to_string());
    o.max_passes = Some(31);
    o.max_items = Some(32);
    o.max_threads = Some(33);
    o.optimization_improvement_threshold = Some(0.02);
    o.max_consecutive_failures = Some(34);
    o.additional_ripup_cost_factor_at_start = Some(35);
    o.trace_ripup_cost_factor = Some(0.7);
    o.max_autoroute_passes = Some(36);
    o.timeout_string = Some("00:02:00".to_string());
    // `transient` in Java — must not appear in the output.
    o.board_update_strategy = Some(BoardUpdateStrategy::Hybrid);
    o.hybrid_ratio = Some("1:1".to_string());
    o.item_selection_strategy = Some(ItemSelectionStrategy::Prioritized);

    let sc = s.scoring.as_mut().expect("new() allocates scoring");
    sc.default_preferred_direction_trace_cost = Some(1.5);
    sc.default_undesired_direction_trace_cost = Some(2.5);
    sc.via_costs = Some(41);
    sc.plane_via_costs = Some(42);
    sc.start_ripup_costs = Some(43);
    sc.unrouted_net_penalty = Some(44.5);
    sc.clearance_violation_penalty = Some(45.5);
    sc.bend_penalty = Some(46.5);
    sc.default_bend_cost = Some(3.5);
    // `transient` in Java — must not appear in the output.
    sc.preferred_direction_trace_cost = Some(vec![7.0, 8.0]);
    sc.undesired_direction_trace_cost = Some(vec![9.0, 10.0]);

    s
}

/// `JProbe.java` block A's output, byte for byte.
const FULLY_POPULATED_JSON: &str = r#"{
  "enabled": true,
  "algorithm": "freerouting-router-v19",
  "fanout": {
    "enabled": false,
    "max_passes": 21,
    "max_items": 22,
    "max_milliseconds_per_pin": 23,
    "ripup_allowed": false,
    "min_escape_length_mm": 2.25,
    "max_escape_length_mm": 4.75,
    "start_via_diameter_mm": 0.5,
    "end_via_diameter_mm": 0.75,
    "pin_sorting_order": "inner_first",
    "fallback_to_board_vias": false,
    "timeout": "00:01:00"
  },
  "copper_to_edge_clearance_um": 501.5,
  "hole_clearance_um": 2.0,
  "neck_width_um": 3.0,
  "strict_drc": true,
  "job_timeout": "12:00:00",
  "max_passes": 11,
  "trace_pull_tight_accuracy": 12,
  "allowed_via_types": false,
  "automatic_neckdown": false,
  "optimizer": {
    "enabled": false,
    "algorithm": "freerouting-optimizer-v19",
    "max_passes": 31,
    "max_items": 32,
    "max_threads": 33,
    "improvement_threshold": 0.02,
    "max_consecutive_failures": 34,
    "additional_ripup_cost_factor_at_start": 35,
    "trace_ripup_cost_factor": 0.7,
    "max_autoroute_passes": 36,
    "timeout": "00:02:00"
  },
  "scoring": {
    "default_preferred_direction_trace_cost": 1.5,
    "default_undesired_direction_trace_cost": 2.5,
    "via_costs": 41,
    "plane_via_costs": 42,
    "start_ripup_costs": 43,
    "unrouted_net_penalty": 44.5,
    "clearance_violation_penalty": 45.5,
    "bend_penalty": 46.5,
    "default_bend_cost": 3.5
  },
  "max_threads": 13,
  "result_json": "/tmp/result.json"
}"#;

/// `JProbe.java` block B's output, byte for byte: the four places `Double.toString` /
/// `Float.toString` disagree with Rust's shortest-round-trip formatter.
const NUMBER_FORMAT_JSON: &str = r#"{
  "fanout": {
    "min_escape_length_mm": 0.1,
    "max_escape_length_mm": 0.3333333333333333,
    "start_via_diameter_mm": 1.2345678901234568E17
  },
  "copper_to_edge_clearance_um": 0.001,
  "hole_clearance_um": 9.999E-4,
  "neck_width_um": 1.0E7,
  "optimizer": {
    "improvement_threshold": 9.999E-4,
    "trace_ripup_cost_factor": 3.4028235E38
  },
  "scoring": {
    "default_preferred_direction_trace_cost": 9999999.0,
    "default_undesired_direction_trace_cost": -0.0,
    "unrouted_net_penalty": 5000000.0,
    "clearance_violation_penalty": 1.0E7,
    "bend_penalty": 0.001,
    "default_bend_cost": 1.0E300
  }
}"#;

fn number_format_fixture() -> RouterSettings {
    let mut s = RouterSettings::new();
    s.copper_to_edge_clearance_um = Some(1.0e-3);
    s.hole_clearance_um = Some(9.999e-4);
    s.neck_width_um = Some(1.0e7);
    let f = s.fanout.as_mut().expect("new() allocates fanout");
    f.min_escape_length_mm = Some(0.1);
    f.max_escape_length_mm = Some(1.0 / 3.0);
    f.start_via_diameter_mm = Some(123_456_789_012_345_680.0);
    let o = s.optimizer.as_mut().expect("new() allocates optimizer");
    o.optimization_improvement_threshold = Some(9.999e-4);
    o.trace_ripup_cost_factor = Some(3.402_823_5e38);
    let sc = s.scoring.as_mut().expect("new() allocates scoring");
    sc.default_preferred_direction_trace_cost = Some(9_999_999.0);
    sc.default_undesired_direction_trace_cost = Some(-0.0);
    sc.default_bend_cost = Some(1.0e300);
    sc.unrouted_net_penalty = Some(5_000_000.0);
    sc.clearance_violation_penalty = Some(1.0e7);
    sc.bend_penalty = Some(1.0e-3);
    s
}

/// Top-level keys of the pretty-printed object, in **emission** order.
///
/// Read off the text rather than through `serde_json::Value`, whose map sorts its keys unless the
/// `preserve_order` feature is on — and emission order is the whole point here (Gson writes
/// `getDeclaredFields()` order).
fn keys(json: &str) -> Vec<String> {
    keys_at(json, 1)
}

/// The keys of the object under `name`, in emission order.
fn nested_keys(json: &str, name: &str) -> Vec<String> {
    let opener = format!("  \"{name}\": {{\n");
    let start = json
        .find(&opener)
        .unwrap_or_else(|| panic!("{name} is an object in\n{json}"))
        + opener.len();
    let end = start
        + json[start..]
            .find("\n  }")
            .unwrap_or_else(|| panic!("{name} is not closed in\n{json}"));
    keys_at(&json[start..end], 2)
}

/// Every `"key":` sitting at exactly `depth` levels of two-space indentation.
fn keys_at(json: &str, depth: usize) -> Vec<String> {
    let indent = "  ".repeat(depth);
    json.lines()
        .filter_map(|line| {
            let rest = line.strip_prefix(&indent)?;
            if rest.starts_with(' ') {
                return None;
            }
            let rest = rest.strip_prefix('"')?;
            let (key, after) = rest.split_once('"')?;
            after.starts_with(':').then(|| key.to_string())
        })
        .collect()
}

// -------------------------------------------------------------------------------------------
// the write side
// -------------------------------------------------------------------------------------------

/// `JProbe.java` block A. The 16 keys Gson emits, in `getDeclaredFields()` order, with the four
/// `transient` fields absent — `layers` included, which the brief's illustrative key list had
/// wrong (`RouterSettingsSerializationTest.routerSettingsSerializationAndDeserialization` asserts
/// `json` does **not** contain `"layers"`, and the JVM agrees).
#[test]
fn emitted_key_set_matches_gson() {
    let json = fully_populated()
        .to_json_string_pretty()
        .expect("serialises");
    assert_eq!(
        keys(&json),
        [
            "enabled",
            "algorithm",
            "fanout",
            "copper_to_edge_clearance_um",
            "hole_clearance_um",
            "neck_width_um",
            "strict_drc",
            "job_timeout",
            "max_passes",
            "trace_pull_tight_accuracy",
            "allowed_via_types",
            "automatic_neckdown",
            "optimizer",
            "scoring",
            "max_threads",
            "result_json",
        ]
    );
    // The three `transient` scalars and the array — checked against the *top-level* key list,
    // because `max_items` is a legitimate key of `fanout` and `optimizer`.
    let top = keys(&json);
    for absent in [
        "layers",
        "max_items",
        "save_intermediate_stages",
        "ignore_net_classes",
    ] {
        assert!(
            !top.iter().any(|key| key == absent),
            "{absent} must not be serialised, got {top:?}"
        );
    }
}

/// `JProbe.java` block A, nested. `scoring`'s two `transient` cost arrays and `optimizer`'s three
/// `transient` fields are dropped; everything else keeps declaration order.
#[test]
fn scoring_and_optimizer_key_sets() {
    let json = fully_populated()
        .to_json_string_pretty()
        .expect("serialises");
    assert_eq!(
        nested_keys(&json, "scoring"),
        [
            "default_preferred_direction_trace_cost",
            "default_undesired_direction_trace_cost",
            "via_costs",
            "plane_via_costs",
            "start_ripup_costs",
            "unrouted_net_penalty",
            "clearance_violation_penalty",
            "bend_penalty",
            "default_bend_cost",
        ]
    );
    assert_eq!(
        nested_keys(&json, "optimizer"),
        [
            "enabled",
            "algorithm",
            "max_passes",
            "max_items",
            "max_threads",
            "improvement_threshold",
            "max_consecutive_failures",
            "additional_ripup_cost_factor_at_start",
            "trace_ripup_cost_factor",
            "max_autoroute_passes",
            "timeout",
        ]
    );
    assert_eq!(
        nested_keys(&json, "fanout"),
        [
            "enabled",
            "max_passes",
            "max_items",
            "max_milliseconds_per_pin",
            "ripup_allowed",
            "min_escape_length_mm",
            "max_escape_length_mm",
            "start_via_diameter_mm",
            "end_via_diameter_mm",
            "pin_sorting_order",
            "fallback_to_board_vias",
            "timeout",
        ]
    );
}

/// The whole of `JProbe.java` block A, byte for byte: two-space indent, `": "` after every key,
/// no trailing newline, and `null` fields omitted (`GsonProvider` never calls `serializeNulls`).
#[test]
fn fully_populated_output_is_byte_identical_to_gson() {
    assert_eq!(
        fully_populated()
            .to_json_string_pretty()
            .expect("serialises"),
        FULLY_POPULATED_JSON
    );
}

/// `JProbe.java` block B. Gson writes numbers through `JsonWriter.value(Number)`, which is
/// `Number.toString()` — `Double.toString` for a `Double` field and `Float.toString` for a
/// `Float` one. Both switch to scientific notation outside `[1e-3, 1e7)`, which Rust's
/// shortest-round-trip formatter does not, and both keep the sign of `-0.0`.
#[test]
fn number_formatting_matches_java_number_to_string() {
    assert_eq!(
        number_format_fixture()
            .to_json_string_pretty()
            .expect("serialises"),
        NUMBER_FORMAT_JSON
    );
}

/// `JProbe.java` block F: Gson instantiates the target through `RouterSettings()`
/// (`RouterSettings.java:119-124`), so an object that names none of the three nested settings
/// still round-trips as three empty objects.
#[test]
fn an_empty_object_round_trips_to_three_empty_nested_objects() {
    let s = RouterSettings::from_json_str("{}").expect("parses");
    assert_eq!(
        s.to_json_string_pretty().expect("serialises"),
        "{\n  \"fanout\": {},\n  \"optimizer\": {},\n  \"scoring\": {}\n}"
    );
}

/// `JProbe.java` block B4: `GsonProvider` never calls `serializeSpecialFloatingPointValues`, so
/// `Gson.toJson` throws `IllegalArgumentException` on a non-finite `Double`/`Float` even under
/// `Strictness.LENIENT`. The port returns an error rather than emitting `null`.
#[test]
fn non_finite_floats_are_refused_like_gson() {
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut s = RouterSettings::new();
        s.hole_clearance_um = Some(value);
        assert!(
            s.to_json_string_pretty().is_err(),
            "{value} must be refused, as Gson refuses it"
        );
    }
    let mut s = RouterSettings::new();
    s.scoring
        .as_mut()
        .expect("new() allocates scoring")
        .bend_penalty = Some(f32::NAN);
    assert!(s.to_json_string_pretty().is_err());
}

// -------------------------------------------------------------------------------------------
// the read side
// -------------------------------------------------------------------------------------------

/// `JProbe.java` block C4 / F. A partial object is a *settings source*: only the keys it names
/// are non-`None`. The three nested objects are the one exception — Gson's `ObjectConstructor`
/// runs `RouterSettings()` before the reflective adapter writes any field, so they come back
/// allocated-and-empty rather than `None` (JVM-verified; this corrects the brief, which expected
/// "every other field `None`").
#[test]
fn partial_object_deserialises_as_a_source() {
    let s = RouterSettings::from_json_str(r#"{"max_passes": 42}"#).expect("parses");
    assert_eq!(s.max_passes, Some(42));
    assert_eq!(s.fanout, Some(FanoutSettings::default()));
    assert_eq!(s.optimizer, Some(OptimizerSettings::default()));
    assert_eq!(s.scoring, Some(ScoringSettings::default()));

    let mut expected = RouterSettings::new();
    expected.max_passes = Some(42);
    assert_eq!(s, expected);
}

/// `JProbe.java` block F: an explicit `null` does clear a nested object — the constructor runs
/// first, the adapter overwrites second.
#[test]
fn explicit_nulls_clear_the_nested_objects() {
    let s = RouterSettings::from_json_str(r#"{"fanout":null,"optimizer":null,"scoring":null}"#)
        .expect("parses");
    assert_eq!(s.fanout, None);
    assert_eq!(s.optimizer, None);
    assert_eq!(s.scoring, None);
}

/// `JProbe.java` block C3: every `@SerializedName(alternate = …)` in the four structs.
#[test]
fn aliases_round_trip() {
    let s = RouterSettings::from_json_str(
        r#"{"tracePullTightAccuracy":7,"automaticNeckdown":true,
            "scoring":{"viaCosts":7,"startRipupCosts":8},
            "fanout":{"ripupAllowed":false}}"#,
    )
    .expect("parses");
    assert_eq!(s.trace_pull_tight_accuracy, Some(7));
    assert_eq!(s.automatic_neckdown, Some(true));
    let scoring = s.scoring.as_ref().expect("scoring");
    assert_eq!(scoring.via_costs, Some(7));
    assert_eq!(scoring.start_ripup_costs, Some(8));
    assert_eq!(
        s.fanout.as_ref().expect("fanout").ripup_allowed,
        Some(false)
    );

    // The write side always emits the canonical name.
    let json = s.to_json_string_pretty().expect("serialises");
    assert!(json.contains("\"via_costs\": 7"), "{json}");
    assert!(json.contains("\"trace_pull_tight_accuracy\": 7"), "{json}");
    assert!(!json.contains("viaCosts"), "{json}");
}

/// `RouterSettingsSerializationTest.routerSettingsSerializationAndDeserialization` step 4, and
/// `JProbe.java` block C: `layers` is `transient`, so the delegate adapter drops it — but
/// `RouterSettingsTypeAdapterFactory.read` (`:59-64`) re-reads it from the raw tree. The other
/// three `transient` fields have no such rescue and stay `None` (block C2).
#[test]
fn layers_are_read_back_but_the_other_transients_are_not() {
    let s = RouterSettings::from_json_str(
        r#"{
          "max_passes": 42,
          "layers": [
            {"routable": false, "preferred_direction_horizontal": true},
            {"routable": true, "preferred_direction_horizontal": false}
          ]
        }"#,
    )
    .expect("parses");
    assert_eq!(s.max_passes, Some(42));
    assert_eq!(
        s.layers,
        Some(vec![
            LayerSettings::new(Some(false), Some(true)),
            LayerSettings::new(Some(true), Some(false)),
        ])
    );

    let s = RouterSettings::from_json_str(
        r#"{"max_items":99,"layers":[{"routable":false}],
            "save_intermediate_stages":true,"ignore_net_classes":["x","y"]}"#,
    )
    .expect("parses");
    assert_eq!(s.max_items, None);
    assert_eq!(s.save_intermediate_stages, None);
    assert_eq!(s.ignore_net_classes, None);
    assert_eq!(s.layers.as_ref().map(Vec::len), Some(1));
}

/// `JProbe.java` block F: `optimizer`'s three `transient` fields are dropped on read as well as
/// on write — the factory rescues `layers` and nothing else.
#[test]
fn the_transient_optimizer_fields_are_dropped_on_read() {
    let s = RouterSettings::from_json_str(
        r#"{"optimizer":{"board_update_strategy":"HYBRID","hybrid_ratio":"1:1",
            "item_selection_strategy":"PRIORITIZED"}}"#,
    )
    .expect("parses");
    let optimizer = s.optimizer.as_ref().expect("optimizer");
    assert_eq!(optimizer.board_update_strategy, None);
    assert_eq!(optimizer.hybrid_ratio, None);
    assert_eq!(optimizer.item_selection_strategy, None);
}

/// `JProbe.java` block C4: Gson ignores keys it does not know, at every level.
#[test]
fn unknown_keys_are_ignored() {
    let s = RouterSettings::from_json_str(
        r#"{"max_passes":42,"no_such_field":1,"scoring":{"nope":2}}"#,
    )
    .expect("parses");
    assert_eq!(s.max_passes, Some(42));
    assert_eq!(s.scoring, Some(ScoringSettings::default()));
}

/// A round trip of the fully populated object: the write side is the fixed point Java's block E
/// reports (`re-serialised equal: true`), and `layers` does not survive it.
#[test]
fn the_written_form_round_trips_to_itself() {
    let json = fully_populated()
        .to_json_string_pretty()
        .expect("serialises");
    let back = RouterSettings::from_json_str(&json).expect("parses");
    assert_eq!(back.to_json_string_pretty().expect("serialises"), json);
    assert_eq!(back.layers, None);
}

/// `JProbe.java` block D, the documented divergence: Gson's `Strictness.LENIENT` reader accepts
/// four shapes strict JSON rejects, and coerces a quoted scalar to the field's type. The port
/// rejects all five. See the `// not ported:` marker on [`RouterSettings::from_json_str`].
#[test]
fn the_lenient_reader_shapes_are_not_ported() {
    for accepted_by_gson in [
        "{max_passes: 42}",
        "{'max_passes': 42}",
        r#"{"max_passes": "42"}"#,
        "// c\n{\"max_passes\": 42}",
        r#"{"strict_drc": "true"}"#,
    ] {
        assert!(
            RouterSettings::from_json_str(accepted_by_gson).is_err(),
            "{accepted_by_gson:?} is accepted by Gson but not by this port"
        );
    }
    // The one shape Gson rejects too (`JsonSyntaxException`).
    assert!(RouterSettings::from_json_str(r#"{"max_passes": 42,}"#).is_err());
}

// -------------------------------------------------------------------------------------------
// the differential golden
// -------------------------------------------------------------------------------------------

fn golden_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/p4t1-mode1/all.txt")
}

/// The whole `p4t1` matrix through `GsonProvider.GSON`, as `scripts/differential/run.sh p4t1
/// <matrix> all 1` writes it. The golden is the **Java** side of that run (its two header lines
/// stripped), committed so the parity survives without a JVM; `run.sh` re-proves it against the
/// live jar.
///
/// The golden carries all 84 rows of `scripts/differential/matrix/p4t1-cases.tsv`; this test
/// replays the 64 that are the cross product `tests/matrix/mod.rs` builds and looks each one up
/// by case id. The remaining 20 `x-*` rows exist only in the TSV, so only `run.sh` covers them —
/// and it covers all 84.
///
/// The resolution below is `tests/precedence.rs`'s, not `p4t1.rs`'s: the driver parses the
/// scheduler's `.rules` against the *board's* layer structure where this test uses the
/// file-discovered one, and the two differ only in per-layer fields — `layers` and both
/// `scoring` cost arrays — every one of which Gson drops. The golden is therefore the same text
/// either way, which this test is also the proof of.
#[test]
fn p4t1_mode_1_parity() {
    if !parity::require_java_dir() {
        return;
    }
    let golden = golden_path();
    let text = std::fs::read_to_string(&golden)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", golden.display()));
    let expected = split_cases(&text);

    let host = HostEnvironment::with_processors(4);
    let mut checked = 0;
    for case in &matrix::cases() {
        let dsn = matrix::dsn_source(case.dsn);
        let cli_rules = matrix::rules_source(case.rules.cli_rules);
        let scheduler_rules = matrix::rules_source(case.rules.scheduler_rules);
        let env = matrix::env_source(case.env);
        let cli = matrix::cli_source(case.cli);
        let scheduler_rules = scheduler_rules
            .as_ref()
            .and_then(SettingsSource::get_settings);
        let inputs = SettingsInputs {
            dsn: dsn.as_ref().and_then(SettingsSource::get_settings),
            cli_rules: cli_rules.as_ref().and_then(SettingsSource::get_settings),
            scheduler_rules,
            env: env.get_settings(),
            cli: cli.get_settings(),
        };
        let board = matrix::board(case.dsn);
        let actual = resolve_headless(&inputs, Some(&board), &host)
            .to_json_string_pretty()
            .expect("serialises");
        let reference = expected
            .iter()
            .find(|(id, _)| *id == case.id)
            .unwrap_or_else(|| panic!("case {} is missing from {}", case.id, golden.display()))
            .1
            .clone();
        assert_eq!(
            parity::normalize_whitespace(&actual),
            parity::normalize_whitespace(&reference),
            "case {}",
            case.id
        );
        checked += 1;
    }
    assert_eq!(checked, 64, "the cross product is 4 x 4 x 2 x 2");
    assert_eq!(expected.len(), 84, "the TSV adds 20 hand-written rows");
}

/// Splits a `CASE <id>` / JSON transcript into `(id, json)` pairs.
fn split_cases(text: &str) -> Vec<(String, String)> {
    let mut cases: Vec<(String, String)> = Vec::new();
    for line in text.lines() {
        if let Some(id) = line.strip_prefix("CASE ") {
            cases.push((id.to_string(), String::new()));
        } else if let Some((_, json)) = cases.last_mut() {
            json.push_str(line);
            json.push('\n');
        }
    }
    for (_, json) in &mut cases {
        while json.ends_with('\n') {
            json.pop();
        }
    }
    cases
}
