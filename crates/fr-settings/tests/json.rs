use std::path::{Path, PathBuf};

use fr_settings::prelude::*;

#[path = "matrix/mod.rs"]
mod matrix;

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
    sc.preferred_direction_trace_cost = Some(vec![7.0, 8.0]);
    sc.undesired_direction_trace_cost = Some(vec![9.0, 10.0]);

    s
}

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

fn keys(json: &str) -> Vec<String> {
    keys_at(json, 1)
}

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

#[test]
fn emitted_key_set_matches_gson() {
    let json = fully_populated()
        .to_json_string_pretty()
        .expect("serialises");
    let top = keys(&json);
    assert_eq!(
        top,
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

#[test]
fn fully_populated_output_is_byte_identical_to_gson() {
    assert_eq!(
        fully_populated()
            .to_json_string_pretty()
            .expect("serialises"),
        FULLY_POPULATED_JSON
    );
}

#[test]
fn number_formatting_matches_java_number_to_string() {
    assert_eq!(
        number_format_fixture()
            .to_json_string_pretty()
            .expect("serialises"),
        NUMBER_FORMAT_JSON
    );
}

#[test]
fn an_empty_object_round_trips_to_three_empty_nested_objects() {
    let s = RouterSettings::from_json_str("{}").expect("parses");
    assert_eq!(
        s.to_json_string_pretty().expect("serialises"),
        "{\n  \"fanout\": {},\n  \"optimizer\": {},\n  \"scoring\": {}\n}"
    );
}

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

#[test]
fn explicit_nulls_clear_the_nested_objects() {
    let s = RouterSettings::from_json_str(r#"{"fanout":null,"optimizer":null,"scoring":null}"#)
        .expect("parses");
    assert_eq!(s.fanout, None);
    assert_eq!(s.optimizer, None);
    assert_eq!(s.scoring, None);
}

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

    let json = s.to_json_string_pretty().expect("serialises");
    assert!(json.contains("\"via_costs\": 7"), "{json}");
    assert!(json.contains("\"trace_pull_tight_accuracy\": 7"), "{json}");
    assert!(!json.contains("viaCosts"), "{json}");
}

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

#[test]
fn unknown_keys_are_ignored() {
    let s = RouterSettings::from_json_str(
        r#"{"max_passes":42,"no_such_field":1,"scoring":{"nope":2}}"#,
    )
    .expect("parses");
    assert_eq!(s.max_passes, Some(42));
    assert_eq!(s.scoring, Some(ScoringSettings::default()));
}

#[test]
fn the_written_form_round_trips_to_itself() {
    let json = fully_populated()
        .to_json_string_pretty()
        .expect("serialises");
    let back = RouterSettings::from_json_str(&json).expect("parses");
    assert_eq!(back.to_json_string_pretty().expect("serialises"), json);
    assert_eq!(back.layers, None);
}

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
    assert!(RouterSettings::from_json_str(r#"{"max_passes": 42,}"#).is_err());
}

#[test]
fn non_finite_floats_are_refused_on_the_read_side_too() {
    for refused_by_gson in [
        r#"{"hole_clearance_um": NaN}"#,
        r#"{"hole_clearance_um": Infinity}"#,
        r#"{"hole_clearance_um": -Infinity}"#,
        r#"{"scoring": {"bend_penalty": NaN}}"#,
        r#"{"hole_clearance_um": "NaN"}"#,
    ] {
        assert!(
            RouterSettings::from_json_str(refused_by_gson).is_err(),
            "{refused_by_gson:?} is refused by Gson and must be refused here"
        );
    }
}

#[test]
fn the_lenient_reader_coercions_are_not_ported() {
    assert!(RouterSettings::from_json_str(r#"{"max_passes": 1, "max_passes": 2}"#).is_err());
    assert!(RouterSettings::from_json_str(r#"{"max_passes": 1.9}"#).is_err());
    for null_document_for_gson in ["null", "", "   "] {
        assert!(RouterSettings::from_json_str(null_document_for_gson).is_err());
    }
    assert!(RouterSettings::from_json_str(r#""null""#).is_err());
}

#[test]
fn the_unicode_line_separators_are_escaped_and_the_html_set_is_not() {
    let mut s = RouterSettings::new();
    s.algorithm = Some("a\u{2028}b\u{2029}c".to_string());
    s.result_json_path = Some("<&>'\"".to_string());
    assert_eq!(
        s.to_json_string_pretty().expect("serialises"),
        concat!(
            "{\n",
            "  \"algorithm\": \"a\\u2028b\\u2029c\",\n",
            "  \"fanout\": {},\n",
            "  \"optimizer\": {},\n",
            "  \"scoring\": {},\n",
            "  \"result_json\": \"<&>'\\\"\"\n",
            "}"
        )
    );
}

fn golden_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/p4t1-mode1/all.txt")
}

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
        let cli_rules = matrix::rules_bytes(case.rules.cli_rules);
        let scheduler_rules = matrix::rules_bytes(case.rules.scheduler_rules);
        let env = matrix::env_source(case.env);
        let cli = matrix::cli_source(case.cli);
        let inputs = SettingsInputs {
            json_file: None,
            dsn: dsn.as_ref().and_then(SettingsSource::get_settings),
            cli_rules: cli_rules.as_deref(),
            scheduler_rules: scheduler_rules.as_deref(),
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

fn json_file_source(name: &str, body: &str) -> JsonFileSettings {
    let dir = std::env::temp_dir()
        .join("fr-settings-json-tier")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let path = dir.join("freerouting.json");
    std::fs::write(&path, body).expect("write freerouting.json");
    JsonFileSettings::new(&path)
}

#[test]
fn a_json_file_tier_beats_the_defaults_and_loses_to_the_dsn() {
    if !parity::require_java_dir() {
        return;
    }
    let host = HostEnvironment::with_processors(4);
    let dsn_case = &matrix::DSN_CASES[2];
    assert_eq!(dsn_case.id, "dsn2-autoroute");
    let board = matrix::board(dsn_case);
    let dsn = matrix::dsn_source(dsn_case).expect("the fixture case has a source");

    let json = json_file_source(
        "beats-defaults",
        r#"{"router": {"max_passes": 11, "scoring": {"via_costs": 77}}}"#,
    );
    let json_settings = json.get_settings().expect("the document parsed");

    let a = resolve_headless(
        &SettingsInputs {
            json_file: Some(json_settings),
            ..SettingsInputs::default()
        },
        Some(&board),
        &host,
    );
    assert_eq!(
        a.scoring.as_ref().and_then(|s| s.via_costs),
        Some(77),
        "priority 10 must beat DefaultSettings' 50"
    );
    assert_eq!(a.max_passes, Some(11));

    let b = resolve_headless(
        &SettingsInputs {
            json_file: Some(json_settings),
            dsn: dsn.get_settings(),
            ..SettingsInputs::default()
        },
        Some(&board),
        &host,
    );
    assert_eq!(
        b.scoring.as_ref().and_then(|s| s.via_costs),
        Some(50),
        "priority 20 must beat priority 10 — and run A shows 77 is what the json alone gives"
    );
    assert_eq!(
        b.max_passes,
        Some(11),
        "the json tier is still live in run B, which is what makes its via_costs 50 the DSN's"
    );

    let c = resolve_headless(
        &SettingsInputs {
            dsn: dsn.get_settings(),
            ..SettingsInputs::default()
        },
        Some(&board),
        &host,
    );
    assert_eq!(c.scoring.as_ref().and_then(|s| s.via_costs), Some(50));
    assert_eq!(c.max_passes, Some(9999));
}
