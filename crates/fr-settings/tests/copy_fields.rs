use fr_settings::copy_fields::{CopyFields, NamedEnum, enum_copy_by_name};
use fr_settings::prelude::*;

#[test]
fn merge_layers_array() {
    let mut source = RouterSettings::new();
    source.set_layer_count(2);
    let layers = source.layers.as_mut().unwrap();
    layers[0].routable = Some(false);
    layers[1].routable = Some(true);
    layers[0].preferred_direction_horizontal = Some(true);
    layers[1].preferred_direction_horizontal = Some(false);

    let mut target = RouterSettings::new();
    assert!(target.layers.is_none());

    target.apply_new_values_from(&source);

    let target_layers = target
        .layers
        .as_ref()
        .expect("layers instantiated by the merge");
    assert_eq!(target_layers.len(), 2);
    assert_eq!(target_layers[0].routable, Some(false));
    assert_eq!(target_layers[1].routable, Some(true));
    assert_eq!(target_layers[0].preferred_direction_horizontal, Some(true));
    assert_eq!(target_layers[1].preferred_direction_horizontal, Some(false));

    target.layers.as_mut().unwrap()[0].routable = Some(true);
    assert_eq!(source.layers.as_ref().unwrap()[0].routable, Some(false));
}

#[test]
fn layers_array_not_shrunk_on_merge() {
    let mut target = RouterSettings::new();
    target.set_layer_count(6);
    {
        let layers = target.layers.as_mut().unwrap();
        layers[0].routable = Some(true);
        layers[1].routable = Some(true);
        layers[2].routable = Some(true);
    }

    let mut source = RouterSettings::new();
    source.set_layer_count(2);
    {
        let layers = source.layers.as_mut().unwrap();
        layers[0].routable = Some(false);
        layers[1].routable = Some(true);
    }

    let mut report = MergeReport::default();
    source.copy_fields_into(&mut target, &mut report);

    assert_eq!(target.get_layer_count(), 6);
    let layers = target.layers.as_ref().unwrap();
    assert_eq!(layers[0].routable, Some(false));
    assert_eq!(layers[1].routable, Some(true));
    assert_eq!(layers[2].routable, Some(true));
}

#[test]
fn object_array_count_is_source_length() {
    let mut target = RouterSettings::new();
    target.set_layer_count(6);

    let mut source = RouterSettings::new();
    source.set_layer_count(2);
    for i in 0..2 {
        source.layers.as_mut().unwrap()[i] = target.layers.as_ref().unwrap()[i];
    }

    let mut report = MergeReport::default();
    source.copy_fields_into(&mut target, &mut report);

    assert!(
        report.fields_changed >= 2,
        "the object-array arm must add source.len() unconditionally, got {}",
        report.fields_changed
    );
}

#[test]
fn primitive_arrays_are_first_writer_wins() {
    let mut target = ScoringSettings {
        preferred_direction_trace_cost: Some(vec![1.0, 1.0]),
        ..Default::default()
    };
    let source = ScoringSettings {
        preferred_direction_trace_cost: Some(vec![2.5, 3.5]),
        ..Default::default()
    };
    let mut report = MergeReport::default();
    source.copy_fields_into(&mut target, &mut report);
    assert_eq!(target.preferred_direction_trace_cost, Some(vec![1.0, 1.0]));

    let mut target = ScoringSettings {
        preferred_direction_trace_cost: Some(vec![]),
        ..Default::default()
    };
    let source = ScoringSettings {
        preferred_direction_trace_cost: Some(vec![2.5]),
        ..Default::default()
    };
    source.copy_fields_into(&mut target, &mut report);
    assert_eq!(target.preferred_direction_trace_cost, Some(vec![2.5]));

    let mut target = ScoringSettings::default();
    source.copy_fields_into(&mut target, &mut report);
    assert_eq!(target.preferred_direction_trace_cost, Some(vec![2.5]));

    let mut target = ScoringSettings {
        preferred_direction_trace_cost: Some(vec![1.0]),
        ..Default::default()
    };
    let source = ScoringSettings {
        preferred_direction_trace_cost: Some(vec![]),
        ..Default::default()
    };
    source.copy_fields_into(&mut target, &mut report);
    assert_eq!(target.preferred_direction_trace_cost, Some(vec![1.0]));
}

#[test]
fn ignore_net_classes_follows_the_same_rule() {
    let mut target = RouterSettings::new();
    target.ignore_net_classes = Some(vec!["GND".to_string()]);
    let mut source = RouterSettings::new();
    source.ignore_net_classes = Some(vec!["VCC".to_string()]);

    target.apply_new_values_from(&source);
    assert_eq!(target.ignore_net_classes, Some(vec!["GND".to_string()]));

    let mut target = RouterSettings::new();
    target.apply_new_values_from(&source);
    assert_eq!(target.ignore_net_classes, Some(vec!["VCC".to_string()]));
}

#[test]
fn primitive_false_does_not_copy() {
    let source = DesignRulesCheckerSettings {
        include_warnings: false,
        ..Default::default()
    };
    let mut target = DesignRulesCheckerSettings {
        include_warnings: true,
        ..Default::default()
    };

    let mut report = MergeReport::default();
    source.copy_fields_into(&mut target, &mut report);

    assert!(
        target.include_warnings,
        "Java bug: `false` is indistinguishable from unset for a primitive field"
    );

    let source = DesignRulesCheckerSettings {
        enabled: true,
        ..Default::default()
    };
    let mut target = DesignRulesCheckerSettings::default();
    source.copy_fields_into(&mut target, &mut report);
    assert!(target.enabled);
}

#[test]
fn debug_settings_primitive_defaults_are_suppressed() {
    let source = DebugSettings {
        trace_insertion_delay: 0,
        operation_filters: vec!["only_this".to_string()],
        ..Default::default()
    };
    let mut target = DebugSettings {
        trace_insertion_delay: 7,
        ..Default::default()
    };
    let mut report = MergeReport::default();
    source.copy_fields_into(&mut target, &mut report);

    assert_eq!(target.trace_insertion_delay, 7, "0 is the `int` default");
    assert_eq!(
        target.operation_filters,
        DebugSettings::default().operation_filters,
        "a non-empty target String[] is never overwritten"
    );

    let source = DebugSettings {
        trace_insertion_delay: 3,
        ..Default::default()
    };
    source.copy_fields_into(&mut target, &mut report);
    assert_eq!(target.trace_insertion_delay, 3);
}

#[test]
fn absent_source_fields_are_not_copied() {
    let source = RouterSettings::default();
    let mut target = RouterSettings::new();
    target.max_passes = Some(42);
    target.algorithm = Some("freerouting-router".to_string());

    let report = target.apply_new_values_from(&source);

    assert_eq!(target.max_passes, Some(42));
    assert_eq!(target.algorithm.as_deref(), Some("freerouting-router"));
    assert_eq!(report.fields_changed, 0);
    assert!(report.errors.is_empty());
}

#[test]
fn scalars_wrappers_and_strings_copy() {
    let mut source = RouterSettings::new();
    source.enabled = Some(true);
    source.algorithm = Some("freerouting-router-v19".to_string());
    source.copper_to_edge_clearance_um = Some(300.0);
    source.max_passes = Some(11);
    source.max_items = Some(5000);
    source.save_intermediate_stages = Some(true);
    source.result_json_path = Some("/tmp/out.json".to_string());

    let mut target = RouterSettings::new();
    target.max_passes = Some(3);
    let report = target.apply_new_values_from(&source);

    assert_eq!(target.enabled, Some(true));
    assert_eq!(target.algorithm.as_deref(), Some("freerouting-router-v19"));
    assert_eq!(target.copper_to_edge_clearance_um, Some(300.0));
    assert_eq!(target.max_passes, Some(11));
    assert_eq!(target.max_items, Some(5000));
    assert_eq!(target.save_intermediate_stages, Some(true));
    assert_eq!(target.result_json_path.as_deref(), Some("/tmp/out.json"));
    assert_eq!(report.fields_changed, 7);
}

#[test]
fn enums_are_copied_by_name() {
    let source = OptimizerSettings {
        board_update_strategy: Some(BoardUpdateStrategy::Hybrid),
        item_selection_strategy: Some(ItemSelectionStrategy::Prioritized),
        ..Default::default()
    };
    let mut target = OptimizerSettings::default();
    let mut report = MergeReport::default();
    source.copy_fields_into(&mut target, &mut report);

    assert_eq!(
        target.board_update_strategy,
        Some(BoardUpdateStrategy::Hybrid)
    );
    assert_eq!(
        target.item_selection_strategy,
        Some(ItemSelectionStrategy::Prioritized)
    );
    assert_eq!(BoardUpdateStrategy::Hybrid.name(), "HYBRID");
    assert_eq!(
        BoardUpdateStrategy::from_name("HYBRID"),
        Some(BoardUpdateStrategy::Hybrid)
    );

    assert_eq!(BoardUpdateStrategy::from_name("hybrid"), None);
    let mut dst: Option<BoardUpdateStrategy> = None;
    let mut report = MergeReport::default();
    enum_copy_by_name(
        "optimizer.board_update_strategy",
        "hybrid",
        &mut dst,
        &mut report,
    );
    assert_eq!(dst, None);
    assert_eq!(
        report.errors,
        vec![MergeError::EnumName {
            path: "optimizer.board_update_strategy".to_string(),
            value: "hybrid".to_string(),
        }]
    );
    assert_eq!(report.fields_changed, 0);
}

#[test]
fn nested_objects_recurse_and_instantiate() {
    let mut source = RouterSettings::default();
    source.optimizer = Some(OptimizerSettings {
        max_passes: Some(9),
        ..Default::default()
    });
    source.fanout = Some(FanoutSettings {
        enabled: Some(true),
        ..Default::default()
    });
    let mut target = RouterSettings::default();
    let report = target.apply_new_values_from(&source);

    assert_eq!(target.optimizer.as_ref().unwrap().max_passes, Some(9));
    assert_eq!(target.fanout.as_ref().unwrap().enabled, Some(true));
    assert_eq!(report.fields_changed, 2);
    assert!(
        target.scoring.is_none(),
        "an absent source object copies nothing"
    );
}

#[test]
fn errors_never_abort() {
    let mut report = MergeReport::default();
    enum_copy_by_name(
        "optimizer.item_selection_strategy",
        "not-a-constant",
        &mut None::<ItemSelectionStrategy>,
        &mut report,
    );
    assert_eq!(report.errors.len(), 1);

    let mut source = RouterSettings::new();
    source.max_passes = Some(11);
    source.set_layer_count(2);
    source.optimizer.as_mut().unwrap().max_passes = Some(3);
    source.result_json_path = Some("/tmp/x.json".to_string());

    let mut target = RouterSettings::new();
    source.copy_fields_into(&mut target, &mut report);

    assert_eq!(target.max_passes, Some(11));
    assert_eq!(target.get_layer_count(), 2);
    assert_eq!(target.optimizer.as_ref().unwrap().max_passes, Some(3));
    assert_eq!(target.result_json_path.as_deref(), Some("/tmp/x.json"));
    assert_eq!(
        report.errors.len(),
        1,
        "the earlier error is kept, not cleared"
    );
}

#[test]
fn fill_absent_from_only_fills_absent_fields() {
    let mut target = RouterSettings::new();
    target.max_passes = Some(3);
    target.algorithm = Some("freerouting-router".to_string());
    target.set_layer_count(2);
    target.layers.as_mut().unwrap()[0].routable = Some(true);

    let mut source = RouterSettings::new();
    source.max_passes = Some(99);
    source.result_json_path = Some("/tmp/only-here.json".to_string());
    source.set_layer_count(2);
    source.layers.as_mut().unwrap()[0].routable = Some(false);
    source.layers.as_mut().unwrap()[0].bend_cost = Some(1.5);
    source.optimizer.as_mut().unwrap().timeout_string = Some("5m".to_string());

    target.fill_absent_from(&source);

    assert_eq!(target.max_passes, Some(3), "a present field is kept");
    assert_eq!(target.algorithm.as_deref(), Some("freerouting-router"));
    assert_eq!(
        target.result_json_path.as_deref(),
        Some("/tmp/only-here.json"),
        "an absent field is filled"
    );
    let layers = target.layers.as_ref().unwrap();
    assert_eq!(
        layers[0].routable,
        Some(true),
        "present per-layer field kept"
    );
    assert_eq!(
        layers[0].bend_cost,
        Some(1.5),
        "absent per-layer field filled"
    );
    assert_eq!(
        target.optimizer.as_ref().unwrap().timeout_string.as_deref(),
        Some("5m")
    );

    let mut target = RouterSettings::new();
    target.max_passes = Some(3);
    target.apply_new_values_from(&source);
    assert_eq!(target.max_passes, Some(99));
}

fn populated_layer() -> LayerSettings {
    LayerSettings {
        routable: Some(false),
        preferred_direction_horizontal: Some(true),
        bend_cost: Some(2.5),
    }
}

fn populated_scoring() -> ScoringSettings {
    ScoringSettings {
        preferred_direction_trace_cost: Some(vec![1.25, 2.25]),
        undesired_direction_trace_cost: Some(vec![3.25, 4.25]),
        default_preferred_direction_trace_cost: Some(5.5),
        default_undesired_direction_trace_cost: Some(6.5),
        via_costs: Some(7),
        plane_via_costs: Some(8),
        start_ripup_costs: Some(9),
        unrouted_net_penalty: Some(10.5),
        clearance_violation_penalty: Some(11.5),
        bend_penalty: Some(12.5),
        default_bend_cost: Some(13.5),
        smd_via_cost_factor: Some(14.5),
    }
}

fn populated_optimizer() -> OptimizerSettings {
    OptimizerSettings {
        enabled: Some(true),
        algorithm: Some("freerouting-optimizer".to_string()),
        max_passes: Some(21),
        max_items: Some(22),
        max_threads: Some(23),
        optimization_improvement_threshold: Some(0.25),
        max_consecutive_failures: Some(24),
        additional_ripup_cost_factor_at_start: Some(25),
        trace_ripup_cost_factor: Some(0.75),
        max_autoroute_passes: Some(26),
        max_search_steps: Some(27),
        board_update_strategy: Some(BoardUpdateStrategy::Hybrid),
        hybrid_ratio: Some("1:2".to_string()),
        item_selection_strategy: Some(ItemSelectionStrategy::Prioritized),
        timeout_string: Some("7m".to_string()),
    }
}

fn populated_fanout() -> FanoutSettings {
    FanoutSettings {
        enabled: Some(true),
        max_passes: Some(31),
        max_items: Some(32),
        max_milliseconds_per_pin: Some(33),
        ripup_allowed: Some(true),
        min_escape_length_mm: Some(0.35),
        max_escape_length_mm: Some(0.36),
        start_via_diameter_mm: Some(0.37),
        end_via_diameter_mm: Some(0.38),
        pin_sorting_order: Some("outer_first".to_string()),
        fallback_to_board_vias: Some(true),
        timeout_string: Some("9m".to_string()),
    }
}

fn populated_router() -> RouterSettings {
    let mut s = RouterSettings::default();
    s.enabled = Some(true);
    s.algorithm = Some("freerouting-router-v19".to_string());
    s.fanout = Some(populated_fanout());
    s.copper_to_edge_clearance_um = Some(41.5);
    s.hole_clearance_um = Some(42.5);
    s.neck_width_um = Some(43.5);
    s.strict_drc = Some(true);
    s.job_timeout_string = Some("11m".to_string());
    s.max_passes = Some(44);
    s.max_items = Some(45);
    s.layers = Some(vec![populated_layer()]);
    s.save_intermediate_stages = Some(true);
    s.ignore_net_classes = Some(vec!["GND".to_string(), "VCC".to_string()]);
    s.trace_pull_tight_accuracy = Some(46);
    s.vias_allowed = Some(true);
    s.automatic_neckdown = Some(true);
    s.optimizer = Some(populated_optimizer());
    s.scoring = Some(populated_scoring());
    s.max_threads = Some(47);
    s.result_json_path = Some("/tmp/result.json".to_string());
    s.opt_changed_area_ms = Some(48);
    s.smd_via_relaxation = Some(false);
    s.failure_give_up_threshold = Some(50);
    s.connection_search_steps = Some(51);
    s
}

#[test]
fn layer_settings_table_covers_every_field() {
    let source = populated_layer();

    let mut target = LayerSettings::default();
    let mut report = MergeReport::default();
    source.copy_fields_into(&mut target, &mut report);
    assert_eq!(
        target, source,
        "a field missing from the table would differ"
    );
    assert_eq!(report.fields_changed, LayerSettings::FIELD_NAMES.len());

    let mut target = LayerSettings::default();
    let mut report = MergeReport::default();
    source.merge_fields_into(&mut target, MergeMode::FillAbsent, &mut report);
    assert_eq!(target, source, "every field of an empty target is absent");
    assert_eq!(report.fields_changed, LayerSettings::FIELD_NAMES.len());
}

#[test]
fn scoring_settings_table_covers_every_field() {
    let source = populated_scoring();

    let mut target = ScoringSettings::default();
    let mut report = MergeReport::default();
    source.copy_fields_into(&mut target, &mut report);
    assert_eq!(
        target, source,
        "a field missing from the table would differ"
    );
    assert_eq!(report.fields_changed, ScoringSettings::FIELD_NAMES.len());

    let mut target = ScoringSettings::default();
    let mut report = MergeReport::default();
    source.merge_fields_into(&mut target, MergeMode::FillAbsent, &mut report);
    assert_eq!(target, source);
    assert_eq!(report.fields_changed, ScoringSettings::FIELD_NAMES.len());
}

#[test]
fn optimizer_settings_table_covers_every_field() {
    let source = populated_optimizer();

    let mut target = OptimizerSettings::default();
    let mut report = MergeReport::default();
    source.copy_fields_into(&mut target, &mut report);
    assert_eq!(
        target, source,
        "a field missing from the table would differ"
    );
    assert_eq!(report.fields_changed, OptimizerSettings::FIELD_NAMES.len());

    let mut target = OptimizerSettings::default();
    let mut report = MergeReport::default();
    source.merge_fields_into(&mut target, MergeMode::FillAbsent, &mut report);
    assert_eq!(target, source);
    assert_eq!(report.fields_changed, OptimizerSettings::FIELD_NAMES.len());
}

#[test]
fn fanout_settings_table_covers_every_field() {
    let source = populated_fanout();

    let mut target = FanoutSettings::default();
    let mut report = MergeReport::default();
    source.copy_fields_into(&mut target, &mut report);
    assert_eq!(
        target, source,
        "a field missing from the table would differ"
    );
    assert_eq!(report.fields_changed, FanoutSettings::FIELD_NAMES.len());

    let mut target = FanoutSettings::default();
    let mut report = MergeReport::default();
    source.merge_fields_into(&mut target, MergeMode::FillAbsent, &mut report);
    assert_eq!(target, source);
    assert_eq!(report.fields_changed, FanoutSettings::FIELD_NAMES.len());
}

#[test]
fn router_settings_table_covers_every_field() {
    let source = populated_router();

    let mut target = RouterSettings::default();
    let report = target.apply_new_values_from(&source);
    assert_eq!(
        target, source,
        "a field missing from the table would differ"
    );

    let expected = (RouterSettings::FIELD_NAMES.len() - 6)
        + 1
        + 1
        + ScoringSettings::FIELD_NAMES.len()
        + OptimizerSettings::FIELD_NAMES.len()
        + FanoutSettings::FIELD_NAMES.len();
    assert_eq!(report.fields_changed, expected);

    let mut target = RouterSettings::default();
    let report = target.fill_absent_from(&source);
    assert_eq!(target, source, "every field of an empty target is absent");
    assert_eq!(report.fields_changed, expected);
}

#[test]
fn design_rules_checker_settings_table_covers_every_field() {
    let source = DesignRulesCheckerSettings {
        enabled: true,
        include_warnings: true,
        include_errors: true,
    };

    let mut target = DesignRulesCheckerSettings::default();
    let mut report = MergeReport::default();
    source.copy_fields_into(&mut target, &mut report);
    assert_eq!(
        target, source,
        "a field missing from the table would differ"
    );
    assert_eq!(
        report.fields_changed,
        DesignRulesCheckerSettings::FIELD_NAMES.len()
    );

    let mut fill_target = DesignRulesCheckerSettings::default();
    let mut report = MergeReport::default();
    source.merge_fields_into(&mut fill_target, MergeMode::FillAbsent, &mut report);
    assert_eq!(fill_target, target);
}

#[test]
fn debug_settings_table_covers_every_field() {
    let mut filter = std::collections::HashSet::new();
    filter.insert("gnd".to_string());
    let source = DebugSettings {
        enable_detailed_logging: true,
        single_step_execution: true,
        trace_insertion_delay: 7,
        filter_by_net: filter,
        operation_filters: vec!["only_this".to_string()],
    };

    let mut target = DebugSettings::default();
    let mut report = MergeReport::default();
    source.copy_fields_into(&mut target, &mut report);
    assert!(target.enable_detailed_logging);
    assert!(target.single_step_execution);
    assert_eq!(target.trace_insertion_delay, 7);
    assert!(
        target.filter_by_net.is_empty(),
        "rule 7 on a Set is a no-op"
    );
    assert_eq!(
        target.operation_filters,
        DebugSettings::default().operation_filters
    );
    assert_eq!(report.fields_changed, 3);

    let mut target = DebugSettings {
        operation_filters: Vec::new(),
        ..Default::default()
    };
    let mut report = MergeReport::default();
    source.copy_fields_into(&mut target, &mut report);
    assert_eq!(target.operation_filters, vec!["only_this".to_string()]);
    assert_eq!(report.fields_changed, 4);

    let mut fill_target = DebugSettings::default();
    let mut report = MergeReport::default();
    source.merge_fields_into(&mut fill_target, MergeMode::FillAbsent, &mut report);
    assert!(fill_target.enable_detailed_logging);
    assert_eq!(fill_target.trace_insertion_delay, 7);
}

#[test]
fn empty_object_array_leaves_a_null_target_null() {
    let mut source = RouterSettings::new();
    source.set_layer_count(0);
    assert_eq!(source.layers.as_ref().unwrap().len(), 0);

    let mut target = RouterSettings::new();
    assert!(target.layers.is_none());
    let report = target.apply_new_values_from(&source);

    assert!(
        target.layers.is_none(),
        "Java leaves the field null; it must not become Some(vec![])"
    );
    assert_eq!(
        report.fields_changed, 2,
        "the two empty scoring cost arrays"
    );

    let mut target = RouterSettings::new();
    target.set_layer_count(2);
    target.apply_new_values_from(&source);
    assert_eq!(target.get_layer_count(), 2);
}

#[test]
fn fill_absent_grows_a_short_object_array_without_discarding_it() {
    let mut target = RouterSettings::new();
    target.set_layer_count(1);
    target.layers.as_mut().unwrap()[0].routable = Some(false);

    let mut source = RouterSettings::new();
    source.set_layer_count(3);
    {
        let layers = source.layers.as_mut().unwrap();
        layers[0].bend_cost = Some(1.5);
        layers[1].bend_cost = Some(2.5);
        layers[2].bend_cost = Some(3.5);
    }

    let report = target.fill_absent_from(&source);

    assert_eq!(target.get_layer_count(), 3, "grown to the source's length");
    let layers = target.layers.as_ref().unwrap();
    assert_eq!(
        layers[0].routable,
        Some(false),
        "the target's own value survives"
    );
    assert_eq!(layers[0].bend_cost, Some(1.5), "absent field filled");
    assert_eq!(
        layers[1].routable,
        Some(true),
        "new slot filled from source"
    );
    assert_eq!(layers[2].bend_cost, Some(3.5));
    assert_eq!(report.fields_changed, 3, "rule 6 counts source.len()");
}
