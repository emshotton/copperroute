//! ported verbatim. The rest pin the quirks and the conversion tolerances; every expected value
use fr_settings::field_path::{FieldKind, set_field_value};
use fr_settings::{
    BoardUpdateStrategy, FanoutSettings, ItemSelectionStrategy, LayerSettings, MergeError,
    OptimizerSettings, RouterSettings, ScoringSettings,
};

#[test]
fn set_simple_property() {
    let mut settings = RouterSettings::new();
    set_field_value(&mut settings, "enabled", "false").expect("resolves");
    assert_eq!(settings.enabled, Some(false));

    set_field_value(&mut settings, "enabled", "true").expect("resolves");
    assert_eq!(settings.enabled, Some(true));
}

#[test]
fn set_nested_array_properties_when_null() {
    let mut settings = RouterSettings::new();
    set_field_value(&mut settings, "layers.routable", "false,true").expect("resolves");

    let layers = settings.layers.as_ref().expect("allocated");
    assert_eq!(layers.len(), 2);
    assert_eq!(layers[0].routable, Some(false));
    assert_eq!(layers[1].routable, Some(true));
}

#[test]
fn set_nested_array_properties_when_initialized() {
    let mut settings = RouterSettings::new();
    settings.set_layer_count(2);

    set_field_value(&mut settings, "layers.routable", "false,true").expect("resolves");
    assert_eq!(settings.layers.as_ref().unwrap()[0].routable, Some(false));
    assert_eq!(settings.layers.as_ref().unwrap()[1].routable, Some(true));

    set_field_value(
        &mut settings,
        "layers.preferred_direction_horizontal",
        "true,false",
    )
    .expect("resolves");
    let layers = settings.layers.as_ref().expect("allocated");
    assert_eq!(layers[0].preferred_direction_horizontal, Some(true));
    assert_eq!(layers[1].preferred_direction_horizontal, Some(false));
}

#[test]
fn case_insensitive_and_serialized_name_matching() {
    let mut settings = RouterSettings::new();
    settings.set_layer_count(2);

    set_field_value(
        &mut settings,
        "layers.preferred_direction_horizontal",
        "true,false",
    )
    .expect("resolves");
    assert_eq!(pdh(&settings), [Some(true), Some(false)], "serialized name");

    set_field_value(
        &mut settings,
        "layers.preferredDirectionHorizontal",
        "false,true",
    )
    .expect("resolves");
    assert_eq!(pdh(&settings), [Some(false), Some(true)], "java field name");

    set_field_value(
        &mut settings,
        "LAYERS.PREFERRED_DIRECTION_HORIZONTAL",
        "true,false",
    )
    .expect("resolves");
    assert_eq!(pdh(&settings), [Some(true), Some(false)], "screaming snake");

    set_field_value(&mut settings, "layers.routable", "true,false").expect("resolves");
    let layers = settings.layers.as_ref().expect("allocated");
    assert_eq!(layers[0].routable, Some(true));
    assert_eq!(layers[1].routable, Some(false));
}

fn pdh(settings: &RouterSettings) -> Vec<Option<bool>> {
    settings
        .layers
        .as_ref()
        .expect("allocated")
        .iter()
        .map(|l| l.preferred_direction_horizontal)
        .collect()
}

#[test]
fn hyphen_and_colon_are_path_separators() {
    let mut settings = RouterSettings::new();
    set_field_value(&mut settings, "optimizer-max_passes", "7").expect("resolves");
    assert_eq!(settings.optimizer.as_ref().unwrap().max_passes, Some(7));

    set_field_value(&mut settings, "optimizer:max_passes", "4").expect("resolves");
    assert_eq!(settings.optimizer.as_ref().unwrap().max_passes, Some(4));
}

#[test]
fn separator_characters_in_the_value_survive() {
    let mut settings = RouterSettings::new();
    set_field_value(&mut settings, "optimizer.hybrid_ratio", "1:1").expect("resolves");
    assert_eq!(
        settings.optimizer.as_ref().unwrap().hybrid_ratio.as_deref(),
        Some("1:1")
    );

    set_field_value(&mut settings, "algorithm", "freerouting-router").expect("resolves");
    assert_eq!(settings.algorithm.as_deref(), Some("freerouting-router"));
}

#[test]
fn extra_array_tokens_are_dropped() {
    let mut settings = RouterSettings::new();
    settings.set_layer_count(2);
    set_field_value(&mut settings, "layers.routable", "a,b,c").expect("resolves");

    let layers = settings.layers.as_ref().expect("allocated");
    assert_eq!(layers.len(), 2);
    assert_eq!(layers[0].routable, Some(false));
    assert_eq!(layers[1].routable, Some(false));
}

#[test]
fn null_array_is_allocated_at_the_token_count() {
    let mut settings = RouterSettings::new();
    set_field_value(&mut settings, "layers.routable", "a,b,c").expect("resolves");
    assert_eq!(settings.layers.as_ref().expect("allocated").len(), 3);
}

#[test]
fn extra_array_elements_are_left_untouched() {
    let mut settings = RouterSettings::new();
    settings.set_layer_count(3);
    set_field_value(&mut settings, "layers.bend_cost", "1.5,2.5").expect("resolves");

    let layers = settings.layers.as_ref().expect("allocated");
    assert_eq!(layers.len(), 3);
    assert_eq!(layers[0].bend_cost, Some(1.5));
    assert_eq!(layers[1].bend_cost, Some(2.5));
    assert_eq!(layers[2].bend_cost, None);
}

#[test]
fn a_trailing_comma_does_not_add_an_array_element() {
    let mut settings = RouterSettings::new();
    set_field_value(&mut settings, "layers.routable", "false,true,").expect("resolves");
    assert_eq!(settings.layers.as_ref().expect("allocated").len(), 2);
}

#[test]
fn boolean_conversion_silently_falls_back_to_false() {
    let mut settings = RouterSettings::new();

    set_field_value(&mut settings, "enabled", "yes").expect("no error — that is the quirk");
    assert_eq!(settings.enabled, Some(false));

    set_field_value(&mut settings, "enabled", "TRUE").expect("resolves");
    assert_eq!(settings.enabled, Some(true));

    set_field_value(&mut settings, "enabled", "0").expect("resolves");
    assert_eq!(settings.enabled, Some(false));

    set_field_value(&mut settings, "enabled", "1").expect("resolves");
    assert_eq!(settings.enabled, Some(true));

    set_field_value(&mut settings, "enabled", " true ").expect("resolves");
    assert_eq!(settings.enabled, Some(false));
}

#[test]
fn array_tokens_are_trimmed_but_scalar_leaves_are_not() {
    let mut settings = RouterSettings::new();
    settings.set_layer_count(2);
    set_field_value(&mut settings, "layers.routable", " true , true ").expect("resolves");
    let layers = settings.layers.as_ref().expect("allocated");
    assert_eq!(layers[0].routable, Some(true));
    assert_eq!(layers[1].routable, Some(true));

    set_field_value(&mut settings, "enabled", " true ").expect("resolves");
    assert_eq!(settings.enabled, Some(false));
}

#[test]
fn enum_matching_is_case_insensitive_on_the_java_constant_name() {
    let mut settings = RouterSettings::new();

    set_field_value(
        &mut settings,
        "optimizer.board_update_strategy",
        "global_optimal",
    )
    .expect("resolves");
    assert_eq!(
        settings.optimizer.as_ref().unwrap().board_update_strategy,
        Some(BoardUpdateStrategy::GlobalOptimal)
    );

    settings.optimizer.as_mut().unwrap().board_update_strategy = None;
    set_field_value(
        &mut settings,
        "optimizer.board_update_strategy",
        "GLOBAL_OPTIMAL",
    )
    .expect("resolves");
    assert_eq!(
        settings.optimizer.as_ref().unwrap().board_update_strategy,
        Some(BoardUpdateStrategy::GlobalOptimal)
    );

    set_field_value(&mut settings, "optimizer.board_update_strategy", " hybrid ")
        .expect("resolves");
    assert_eq!(
        settings.optimizer.as_ref().unwrap().board_update_strategy,
        Some(BoardUpdateStrategy::Hybrid)
    );

    let err = set_field_value(
        &mut settings,
        "optimizer.board_update_strategy",
        "globalOptimal",
    )
    .expect_err("no constant matches");
    assert!(matches!(err, MergeError::EnumName { .. }), "got {err:?}");
    assert_eq!(
        settings.optimizer.as_ref().unwrap().board_update_strategy,
        Some(BoardUpdateStrategy::Hybrid),
        "the failed write leaves the old value in place"
    );

    set_field_value(
        &mut settings,
        "optimizer.item_selection_strategy",
        "prioritized",
    )
    .expect("resolves");
    assert_eq!(
        settings.optimizer.as_ref().unwrap().item_selection_strategy,
        Some(ItemSelectionStrategy::Prioritized)
    );
}

#[test]
fn int_rejects_whitespace_but_double_trims_it() {
    let mut settings = RouterSettings::new();

    let err = set_field_value(&mut settings, "max_passes", " 7 ").expect_err("parseInt is strict");
    assert!(
        matches!(err, MergeError::NumberFormat { .. }),
        "got {err:?}"
    );
    assert_eq!(settings.max_passes, None);

    set_field_value(&mut settings, "copper_to_edge_clearance_um", " 7 ")
        .expect("parseDouble trims");
    assert_eq!(settings.copper_to_edge_clearance_um, Some(7.0));
}

#[test]
fn int_conversion_follows_parse_int() {
    let mut settings = RouterSettings::new();

    set_field_value(&mut settings, "max_passes", "+7").expect("resolves");
    assert_eq!(settings.max_passes, Some(7));

    assert!(matches!(
        set_field_value(&mut settings, "max_passes", "7_0"),
        Err(MergeError::NumberFormat { .. })
    ));
    assert!(matches!(
        set_field_value(&mut settings, "max_passes", "99999999999"),
        Err(MergeError::NumberFormat { .. })
    ));
    assert_eq!(
        settings.max_passes,
        Some(7),
        "both failures leave 7 in place"
    );
}

#[test]
fn double_conversion_follows_parse_double() {
    let mut settings = RouterSettings::new();

    for (input, expected) in [
        ("1e5", 100_000.0),
        ("5d", 5.0),
        ("5F", 5.0),
        (".5", 0.5),
        ("5.", 5.0),
    ] {
        set_field_value(&mut settings, "hole_clearance_um", input).expect(input);
        assert_eq!(
            settings.hole_clearance_um,
            Some(expected),
            "input {input:?}"
        );
    }

    set_field_value(&mut settings, "hole_clearance_um", "Infinity").expect("resolves");
    assert_eq!(settings.hole_clearance_um, Some(f64::INFINITY));
    set_field_value(&mut settings, "hole_clearance_um", "-Infinity").expect("resolves");
    assert_eq!(settings.hole_clearance_um, Some(f64::NEG_INFINITY));
    set_field_value(&mut settings, "hole_clearance_um", "NaN").expect("resolves");
    assert!(settings.hole_clearance_um.expect("set").is_nan());

    for bad in [
        "inf",
        "infinity",
        "nan",
        "",
        " ",
        "1e",
        ".",
        "5.5.5",
        "--5",
        "1_0",
        "Infinityd",
    ] {
        assert!(
            matches!(
                set_field_value(&mut settings, "hole_clearance_um", bad),
                Err(MergeError::NumberFormat { .. })
            ),
            "{bad:?} should not parse"
        );
    }
}

#[test]
fn long_and_float_fields_use_their_own_parsers() {
    let mut settings = RouterSettings::new();

    set_field_value(
        &mut settings,
        "fanout.max_milliseconds_per_pin",
        "9000000000",
    )
    .expect("resolves");
    assert_eq!(
        settings.fanout.as_ref().unwrap().max_milliseconds_per_pin,
        Some(9_000_000_000)
    );

    set_field_value(&mut settings, "scoring.unrouted_net_penalty", "1e40").expect("resolves");
    assert_eq!(
        settings.scoring.as_ref().unwrap().unrouted_net_penalty,
        Some(f32::INFINITY)
    );
}

#[test]
fn double_array_leaf_splits_and_trims() {
    let mut settings = RouterSettings::new();
    set_field_value(
        &mut settings,
        "scoring.preferred_direction_trace_cost",
        "1.5, 2.0",
    )
    .expect("resolves");
    assert_eq!(
        settings
            .scoring
            .as_ref()
            .unwrap()
            .preferred_direction_trace_cost
            .as_deref(),
        Some([1.5, 2.0].as_slice())
    );

    assert!(matches!(
        set_field_value(&mut settings, "scoring.preferred_direction_trace_cost", "x"),
        Err(MergeError::NumberFormat { .. })
    ));
}

#[test]
fn string_array_leaf_follows_java_split() {
    let mut settings = RouterSettings::new();

    set_field_value(&mut settings, "ignore_net_classes", " a , b ,").expect("resolves");
    assert_eq!(
        settings.ignore_net_classes.as_deref(),
        Some(["a".to_string(), "b".to_string()].as_slice())
    );

    set_field_value(&mut settings, "ignore_net_classes", "").expect("resolves");
    assert_eq!(settings.ignore_net_classes.as_deref(), Some([].as_slice()));

    set_field_value(&mut settings, "ignore_net_classes", "a,,b").expect("resolves");
    assert_eq!(
        settings.ignore_net_classes.as_deref(),
        Some(["a".to_string(), String::new(), "b".to_string()].as_slice())
    );
}

#[test]
fn serialized_alternate_and_java_names_all_resolve() {
    let mut settings = RouterSettings::new();

    set_field_value(&mut settings, "trace_pull_tight_accuracy", "8").expect("serialized");
    assert_eq!(settings.trace_pull_tight_accuracy, Some(8));
    set_field_value(&mut settings, "tracePullTightAccuracy", "9").expect("alternate");
    assert_eq!(settings.trace_pull_tight_accuracy, Some(9));
    set_field_value(&mut settings, "TRACEPULLTIGHTACCURACY", "7").expect("java name, any case");
    assert_eq!(settings.trace_pull_tight_accuracy, Some(7));

    set_field_value(&mut settings, "allowed_via_types", "true").expect("serialized");
    assert_eq!(settings.vias_allowed, Some(true));
    set_field_value(&mut settings, "vias_allowed", "false").expect("snake of the java name");
    assert_eq!(settings.vias_allowed, Some(false));
    set_field_value(&mut settings, "viasAllowed", "true").expect("java name");
    assert_eq!(settings.vias_allowed, Some(true));

    set_field_value(&mut settings, "job_timeout", "5m").expect("serialized");
    assert_eq!(settings.job_timeout_string.as_deref(), Some("5m"));
    set_field_value(&mut settings, "jobTimeoutString", "6m").expect("java name");
    assert_eq!(settings.job_timeout_string.as_deref(), Some("6m"));
    set_field_value(&mut settings, "job_timeout_string", "7m").expect("snake of the java name");
    assert_eq!(settings.job_timeout_string.as_deref(), Some("7m"));

    set_field_value(&mut settings, "result_json", "/tmp/x").expect("serialized");
    assert_eq!(settings.result_json_path.as_deref(), Some("/tmp/x"));
    set_field_value(&mut settings, "resultJsonPath", "/tmp/y").expect("java name");
    assert_eq!(settings.result_json_path.as_deref(), Some("/tmp/y"));

    set_field_value(&mut settings, "scoring.viaCosts", "3").expect("alternate");
    assert_eq!(settings.scoring.as_ref().unwrap().via_costs, Some(3));
    set_field_value(&mut settings, "scoring.via_costs", "4").expect("serialized");
    assert_eq!(settings.scoring.as_ref().unwrap().via_costs, Some(4));
    set_field_value(&mut settings, "fanout.ripupAllowed", "true").expect("alternate");
    assert_eq!(settings.fanout.as_ref().unwrap().ripup_allowed, Some(true));
    set_field_value(&mut settings, "optimizer.improvement_threshold", "0.5").expect("serialized");
    assert_eq!(
        settings
            .optimizer
            .as_ref()
            .unwrap()
            .optimization_improvement_threshold,
        Some(0.5)
    );
    set_field_value(
        &mut settings,
        "optimizer.optimization_improvement_threshold",
        "0.25",
    )
    .expect("snake of the java name");
    assert_eq!(
        settings
            .optimizer
            .as_ref()
            .unwrap()
            .optimization_improvement_threshold,
        Some(0.25)
    );
    set_field_value(&mut settings, "optimizer.timeout", "9m").expect("serialized");
    assert_eq!(
        settings
            .optimizer
            .as_ref()
            .unwrap()
            .timeout_string
            .as_deref(),
        Some("9m")
    );
}

#[test]
fn an_unknown_name_is_no_such_field() {
    let mut settings = RouterSettings::new();
    let err = set_field_value(&mut settings, "nope", "1").expect_err("no such field");
    assert!(matches!(err, MergeError::NoSuchField { .. }), "got {err:?}");
}

#[test]
fn a_null_nested_object_is_instantiated() {
    let mut settings = RouterSettings::default();
    assert_eq!(settings.fanout, None);
    set_field_value(&mut settings, "fanout.max_passes", "3").expect("resolves");
    assert_eq!(settings.fanout.as_ref().unwrap().max_passes, Some(3));
}

#[test]
fn empty_path_segments_follow_java_split() {
    let mut settings = RouterSettings::new();

    set_field_value(&mut settings, "enabled.", "false").expect("trailing separator is dropped");
    assert_eq!(settings.enabled, Some(false));

    assert!(matches!(
        set_field_value(&mut settings, "optimizer..max_passes", "3"),
        Err(MergeError::NoSuchField { .. })
    ));
}

#[test]
fn java_static_constants_are_not_settable_fields() {
    let mut settings = RouterSettings::new();
    assert!(matches!(
        set_field_value(&mut settings, "min_bend_cost", "1"),
        Err(MergeError::NoSuchField { .. })
    ));
    assert!(matches!(
        set_field_value(&mut settings, "ALGORITHM_CURRENT", "x"),
        Err(MergeError::NoSuchField { .. })
    ));
}

#[test]
fn type_mismatches_are_errors_not_panics() {
    let mut settings = RouterSettings::new();
    for path in ["fanout", "layers", "enabled.foo", "max_passes.value"] {
        let err = set_field_value(&mut settings, path, "x").expect_err(path);
        assert!(
            matches!(err, MergeError::TypeMismatch { .. }),
            "{path}: {err:?}"
        );
    }
}

#[test]
fn a_bad_segment_after_an_array_field_writes_nothing() {
    let mut settings = RouterSettings::new();

    let err =
        set_field_value(&mut settings, "ignore_net_classes.foo", "a,b").expect_err("String[]");
    assert!(
        matches!(err, MergeError::TypeMismatch { .. }),
        "got {err:?}"
    );
    assert_eq!(
        settings.ignore_net_classes, None,
        "Java would leave [\"\", null] behind"
    );

    let err = set_field_value(
        &mut settings,
        "scoring.preferred_direction_trace_cost.foo",
        "1,2",
    )
    .expect_err("double[]");
    assert!(
        matches!(err, MergeError::TypeMismatch { .. }),
        "got {err:?}"
    );
    assert_eq!(
        settings
            .scoring
            .as_ref()
            .expect("nested")
            .preferred_direction_trace_cost,
        None,
        "Java would leave [0.0, 0.0] behind"
    );
}

#[test]
fn hexadecimal_float_literals_are_a_recorded_divergence() {
    let mut settings = RouterSettings::new();
    assert!(matches!(
        set_field_value(&mut settings, "hole_clearance_um", "0x1p3"),
        Err(MergeError::NumberFormat { .. })
    ));
}

#[test]
fn an_all_separator_path_is_an_error_not_a_panic() {
    let mut settings = RouterSettings::new();
    for path in [".", "-", ":", "..", "_"] {
        assert!(
            matches!(
                set_field_value(&mut settings, path, "1"),
                Err(MergeError::NoSuchField { .. })
            ),
            "{path:?}"
        );
    }
}

#[test]
fn field_tables_match_the_declaration_order_pins() {
    fn rust_names(fields: &[fr_settings::field_path::FieldSpec]) -> Vec<&'static str> {
        fields.iter().map(|f| f.rust_name).collect()
    }

    assert_eq!(
        rust_names(RouterSettings::FIELDS),
        RouterSettings::FIELD_NAMES
    );
    assert_eq!(
        rust_names(LayerSettings::FIELDS),
        LayerSettings::FIELD_NAMES
    );
    assert_eq!(
        rust_names(ScoringSettings::FIELDS),
        ScoringSettings::FIELD_NAMES
    );
    assert_eq!(
        rust_names(OptimizerSettings::FIELDS),
        OptimizerSettings::FIELD_NAMES
    );
    assert_eq!(
        rust_names(FanoutSettings::FIELDS),
        FanoutSettings::FIELD_NAMES
    );
}

#[test]
fn field_kinds_match_the_java_field_types() {
    use FieldKind::{Bool, Enum, F32, F64, F64Vec, I32, I64, Nested, ObjectArray, Str, StringVec};
    const BUS: &[&str] = &["GREEDY", "GLOBAL_OPTIMAL", "HYBRID"];
    const ISS: &[&str] = &["SEQUENTIAL", "RANDOM", "PRIORITIZED"];

    fn check(
        what: &str,
        fields: &[fr_settings::field_path::FieldSpec],
        want: &[(&str, FieldKind)],
    ) {
        let got: Vec<(&str, FieldKind)> = fields.iter().map(|f| (f.rust_name, f.kind)).collect();
        assert_eq!(got, want.to_vec(), "{what}");
    }

    check(
        "RouterSettings",
        RouterSettings::FIELDS,
        &[
            ("enabled", Bool),
            ("algorithm", Str),
            ("fanout", Nested),
            ("copper_to_edge_clearance_um", F64),
            ("hole_clearance_um", F64),
            ("neck_width_um", F64),
            ("strict_drc", Bool),
            ("job_timeout_string", Str),
            ("max_passes", I32),
            ("max_items", I32),
            ("layers", ObjectArray),
            ("save_intermediate_stages", Bool),
            ("ignore_net_classes", StringVec),
            ("trace_pull_tight_accuracy", I32),
            ("vias_allowed", Bool),
            ("automatic_neckdown", Bool),
            ("optimizer", Nested),
            ("scoring", Nested),
            ("max_threads", I32),
            ("result_json_path", Str),
            ("board_specific_trace_costs_applied", Bool),
            ("opt_changed_area_ms", I32),
        ],
    );
    check(
        "LayerSettings",
        LayerSettings::FIELDS,
        &[
            ("routable", Bool),
            ("preferred_direction_horizontal", Bool),
            ("bend_cost", F64),
        ],
    );
    check(
        "ScoringSettings",
        ScoringSettings::FIELDS,
        &[
            ("preferred_direction_trace_cost", F64Vec),
            ("undesired_direction_trace_cost", F64Vec),
            ("default_preferred_direction_trace_cost", F64),
            ("default_undesired_direction_trace_cost", F64),
            ("via_costs", I32),
            ("plane_via_costs", I32),
            ("start_ripup_costs", I32),
            ("unrouted_net_penalty", F32),
            ("clearance_violation_penalty", F32),
            ("bend_penalty", F32),
            ("default_bend_cost", F64),
        ],
    );
    check(
        "OptimizerSettings",
        OptimizerSettings::FIELDS,
        &[
            ("enabled", Bool),
            ("algorithm", Str),
            ("max_passes", I32),
            ("max_items", I32),
            ("max_threads", I32),
            ("optimization_improvement_threshold", F32),
            ("max_consecutive_failures", I32),
            ("additional_ripup_cost_factor_at_start", I32),
            ("trace_ripup_cost_factor", F32),
            ("max_autoroute_passes", I32),
            ("board_update_strategy", Enum(BUS)),
            ("hybrid_ratio", Str),
            ("item_selection_strategy", Enum(ISS)),
            ("timeout_string", Str),
        ],
    );
    check(
        "FanoutSettings",
        FanoutSettings::FIELDS,
        &[
            ("enabled", Bool),
            ("max_passes", I32),
            ("max_items", I32),
            ("max_milliseconds_per_pin", I64),
            ("ripup_allowed", Bool),
            ("min_escape_length_mm", F64),
            ("max_escape_length_mm", F64),
            ("start_via_diameter_mm", F64),
            ("end_via_diameter_mm", F64),
            ("pin_sorting_order", Str),
            ("fallback_to_board_vias", Bool),
            ("timeout_string", Str),
        ],
    );
}

#[test]
fn every_field_converts_according_to_its_kind() {
    fn after(path: &str, value: &str) -> RouterSettings {
        let mut settings = RouterSettings::new();
        set_field_value(&mut settings, path, value)
            .unwrap_or_else(|e| panic!("{path} = {value:?} should convert: {e}"));
        settings
    }
    fn err(path: &str, value: &str) -> MergeError {
        let mut settings = RouterSettings::new();
        set_field_value(&mut settings, path, value)
            .expect_err(&format!("{path} = {value:?} should not convert"))
    }
    fn is_number_format(path: &str, value: &str) {
        let e = err(path, value);
        assert!(
            matches!(e, MergeError::NumberFormat { .. }),
            "{path} = {value:?}: {e:?}"
        );
    }

    let tables: [(&str, &[fr_settings::field_path::FieldSpec]); 5] = [
        ("", RouterSettings::FIELDS),
        ("layers.", LayerSettings::FIELDS),
        ("scoring.", ScoringSettings::FIELDS),
        ("optimizer.", OptimizerSettings::FIELDS),
        ("fanout.", FanoutSettings::FIELDS),
    ];

    for (prefix, fields) in tables {
        for field in fields {
            let p = &format!("{prefix}{}", field.rust_name);
            match field.kind {
                FieldKind::Bool => {
                    assert_eq!(after(p, "1"), after(p, "true"), "{p}: Bool");
                    assert_ne!(after(p, "1"), after(p, "0"), "{p}: Bool");
                }
                FieldKind::Str => {
                    assert_ne!(after(p, "1"), after(p, "true"), "{p}: Str");
                    assert_ne!(after(p, " a , b ,"), after(p, "a,b"), "{p}: Str");
                }
                FieldKind::StringVec => {
                    assert_ne!(after(p, "1"), after(p, "true"), "{p}: StringVec");
                    assert_eq!(after(p, " a , b ,"), after(p, "a,b"), "{p}: StringVec");
                }
                FieldKind::I32 | FieldKind::I64 => {
                    after(p, "7");
                    is_number_format(p, "zz");
                    is_number_format(p, " 7 ");
                }
                FieldKind::F32 | FieldKind::F64 => {
                    after(p, "7.5");
                    after(p, " 7 ");
                    after(p, "7d");
                    is_number_format(p, "zz");
                }
                FieldKind::F64Vec | FieldKind::I32Vec => {
                    assert_eq!(after(p, "1"), after(p, " 1 , "), "{p}: numeric vec");
                    is_number_format(p, "zz");
                }
                FieldKind::Enum(constants) => {
                    let first = constants.first().expect("no constants");
                    assert_eq!(
                        after(p, first),
                        after(p, &first.to_lowercase()),
                        "{p}: Enum is case-insensitive"
                    );
                    let e = err(p, "zz");
                    assert!(matches!(e, MergeError::EnumName { .. }), "{p}: {e:?}");
                }
                FieldKind::Nested | FieldKind::ObjectArray => {
                    let e = err(p, "zz");
                    assert!(matches!(e, MergeError::TypeMismatch { .. }), "{p}: {e:?}");
                }
            }
        }
    }
}

#[test]
fn only_the_four_navigable_router_fields_are_navigable() {
    let navigable: Vec<&str> = RouterSettings::FIELDS
        .iter()
        .filter(|f| matches!(f.kind, FieldKind::Nested | FieldKind::ObjectArray))
        .map(|f| f.rust_name)
        .collect();
    assert_eq!(navigable, ["fanout", "layers", "optimizer", "scoring"]);
}
