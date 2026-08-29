//! `ReflectionUtil.copyFields` (`util/ReflectionUtil.java:215-344`) rule-by-rule, plus
//! `RouterSettings.applyNewValuesFrom` (`settings/RouterSettings.java:907-929`) and its inverse
//! `fill_absent_from` (plan ruling 1).
//!
//! Ported from `settings/RouterSettingsMergeTest.java`, `settings/SettingsMergerTest.java` and
//! `util/ReflectionUtilArrayTest.java`. Every test is named after the `copyFields` rule it pins;
//! the eight rules are enumerated in plan ruling 4.

use fr_settings::copy_fields::{CopyFields, JavaEnum, enum_copy_by_name};
use fr_settings::prelude::*;

/// Rule 6 (object arrays, `ReflectionUtil.java:291-327`), port of
/// `RouterSettingsMergeTest.java:13-38` (`mergeLayersArray`).
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

    // Java's `assertNotSame(source.layers[0], target.layers[0])` — the merge deep-copies, so
    // writing through the target must not reach the source.
    target.layers.as_mut().unwrap()[0].routable = Some(true);
    assert_eq!(source.layers.as_ref().unwrap()[0].routable, Some(false));
}

/// Rule 6's never-shrink half (`ReflectionUtil.java:302-312`), port of
/// `SettingsMergerTest.java:259-277` (`layersArrayNotShrunkOnMerge`).
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

/// Rule 6's count (`ReflectionUtil.java:311`, `:325`): the object-array arm adds
/// `sourceArray.length` whether or not any element actually moved, and discards the inner
/// `copyFields` return values.
#[test]
fn object_array_count_is_source_length() {
    let mut target = RouterSettings::new();
    target.set_layer_count(6);

    let mut source = RouterSettings::new();
    source.set_layer_count(2);
    // Make the two source layers identical to the target's, so nothing actually changes.
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

/// Rule 5 (primitive/`String` arrays, `ReflectionUtil.java:269-290`): first writer wins.
#[test]
fn primitive_arrays_are_first_writer_wins() {
    // Populated target: never overwritten.
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

    // Empty target + non-empty source: copied (`targetArrayLength == 0 && sourceArrayLength > 0`).
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

    // Null target: copied (`targetValue == null`).
    let mut target = ScoringSettings::default();
    source.copy_fields_into(&mut target, &mut report);
    assert_eq!(target.preferred_direction_trace_cost, Some(vec![2.5]));

    // Empty source onto a populated target: the `sourceArrayLength > 0` half of :285.
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

/// Rule 5 again, for the one `String[]` field of `RouterSettings`.
#[test]
fn ignore_net_classes_follows_the_same_rule() {
    let mut target = RouterSettings::new();
    target.ignore_net_classes = Some(vec!["GND".to_string()]);
    let mut source = RouterSettings::new();
    source.ignore_net_classes = Some(vec!["VCC".to_string()]);

    target.apply_new_values_from(&source);
    assert_eq!(target.ignore_net_classes, Some(vec!["GND".to_string()]));

    // ... and the null-target half.
    let mut target = RouterSettings::new();
    target.apply_new_values_from(&source);
    assert_eq!(target.ignore_net_classes, Some(vec!["VCC".to_string()]));
}

// Rule 1's non-`public` half (`ReflectionUtil.java:226-228`) — `private transient Boolean
// boardSpecificTraceCostsApplied` (`RouterSettings.java:111`) is never copied — is pinned by the
// `board_specific_flag_is_never_copied` unit test in `src/copy_fields.rs`: the Rust field is
// `pub(crate)` for exactly the reason the Java one is `private`, so only an in-crate test can set
// it to `Some(true)` and observe that the merge leaves the target's `None` alone.

/// Rule 1's `static` half (`ReflectionUtil.java:221-223`): `RouterSettings`'s four
/// `public static final` constants (`RouterSettings.java:15-18`) are not fields of an instance
/// and never appear in the merge engine's field table.
#[test]
fn static_constants_are_not_in_the_field_table() {
    for constant in [
        "ALGORITHM_CURRENT",
        "ALGORITHM_V19",
        "MIN_BEND_COST",
        "MAX_BEND_COST",
    ] {
        assert!(
            !RouterSettings::FIELD_NAMES.contains(&constant),
            "{constant} is static and must not be copied"
        );
    }
}

/// Rule 2's default-suppression half (`ReflectionUtil.java:235-238` with
/// `getDefaultValue` :350-372): a Java `boolean` field's default is `false`, so a `false` source
/// value can never be merged.
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

    // The `true` direction does copy.
    let source = DesignRulesCheckerSettings {
        enabled: true,
        ..Default::default()
    };
    let mut target = DesignRulesCheckerSettings::default();
    source.copy_fields_into(&mut target, &mut report);
    assert!(target.enabled);
}

/// Rule 2 on `DebugSettings`'s `int` and `String[]` fields: `getDefaultValue` answers `0` for an
/// `int` (`ReflectionUtil.java:355-356`), and `operationFilters` goes down rule 5's path.
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

/// Rule 2's null half (`ReflectionUtil.java:234`): a `None` source field copies nothing.
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

/// Rule 3 (primitives, wrappers and `String`, `ReflectionUtil.java:242-259`).
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
    // `transient` is NOT skipped by copyFields (plan ruling 4).
    assert_eq!(target.max_items, Some(5000));
    assert_eq!(target.save_intermediate_stages, Some(true));
    assert_eq!(target.result_json_path.as_deref(), Some("/tmp/out.json"));
    assert_eq!(report.fields_changed, 7);
}

/// Rule 4 (enums by name, `ReflectionUtil.java:260-266`): `Enum.valueOf(type, source.toString())`
/// — exact, case-sensitive matching, unlike `setFieldValue`'s case-insensitive one (:158-164).
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
    assert_eq!(BoardUpdateStrategy::Hybrid.java_name(), "HYBRID");
    assert_eq!(
        BoardUpdateStrategy::from_java_name("HYBRID"),
        Some(BoardUpdateStrategy::Hybrid)
    );

    // Case-sensitive: `Enum.valueOf` throws IllegalArgumentException for "hybrid".
    assert_eq!(BoardUpdateStrategy::from_java_name("hybrid"), None);
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

/// Rule 7 (any other object, `ReflectionUtil.java:328-336`): recurse, instantiating a `null`
/// target field with its no-arg constructor.
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
    // A target whose nested objects are all absent — Java's `new RouterSettings()` allocates them,
    // but `RouterSettings::default()` (the deserialised shape) does not.
    let mut target = RouterSettings::default();
    let report = target.apply_new_values_from(&source);

    assert_eq!(target.optimizer.as_ref().unwrap().max_passes, Some(9));
    assert_eq!(target.fanout.as_ref().unwrap().enabled, Some(true));
    // The nested field itself is not counted — only the leaves it wrote (:335).
    assert_eq!(report.fields_changed, 2);
    assert!(
        target.scoring.is_none(),
        "an absent source object copies nothing"
    );
}

/// Rule 8 (`ReflectionUtil.java:338-340`): every exception is swallowed per field and the merge
/// carries on with the next one.
#[test]
fn errors_never_abort() {
    let mut report = MergeReport::default();
    // An error already recorded by an earlier field (Task 3's string-keyed sources are where a
    // real one comes from) must not stop anything that follows.
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

/// `fill_absent_from` (plan ruling 1): `copy_fields` with the scalar roles inverted. Only fields
/// still `None` — and arrays still null-or-empty, which rule 5 already does in both directions —
/// are filled.
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

    // ... and `apply_new_values_from` is still the other direction.
    let mut target = RouterSettings::new();
    target.max_passes = Some(3);
    target.apply_new_values_from(&source);
    assert_eq!(target.max_passes, Some(99));
}
