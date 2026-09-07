use std::collections::BTreeMap;

use copper_settings::sources::{DefaultSettings, EnvironmentVariablesSource};
use copper_settings::{
    BoardUpdateStrategy, HostEnvironment, ItemSelectionStrategy, MergeError, SettingsMerger,
    SettingsSource, SourceKind, priority,
};

fn host() -> HostEnvironment {
    HostEnvironment::with_processors(4)
}

fn env(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect()
}

fn source(pairs: &[(&str, &str)]) -> EnvironmentVariablesSource {
    EnvironmentVariablesSource::new(&env(pairs))
}

#[test]
fn priority_and_source_name() {
    let empty = source(&[]);
    assert_eq!(empty.get_priority(), 55);
    assert_eq!(empty.get_priority(), priority::ENVIRONMENT);
    assert_eq!(empty.get_source_name(), "Environment Variables");
    assert_eq!(empty.kind(), SourceKind::Environment);
}

#[test]
fn empty_environment_yields_a_blank_but_present_settings_object() {
    let empty = source(&[]);
    assert_eq!(empty.get_parsed_count(), 0);
    let settings = empty.get_settings().expect("never null");
    assert_eq!(settings.max_passes, None);
    assert!(settings.optimizer.is_some());
    assert!(settings.scoring.is_some());
    assert!(settings.fanout.is_some());
}

#[test]
fn a_simple_router_setting_is_parsed() {
    let s = source(&[("COPPERROUTE__ROUTER__MAX_PASSES", "50")]);
    assert_eq!(s.get_settings().unwrap().max_passes, Some(50));
    assert_eq!(s.get_parsed_count(), 1);
}

#[test]
fn variable_names_are_case_insensitive() {
    let lower = source(&[
        ("copperroute__router__max_passes", "50"),
        ("copperroute__router__save_intermediate_stages", "true"),
    ]);
    assert_eq!(lower.get_settings().unwrap().max_passes, Some(50));
    assert_eq!(
        lower.get_settings().unwrap().save_intermediate_stages,
        Some(true)
    );
    assert_eq!(lower.get_parsed_count(), 2);
    assert_eq!(
        lower
            .get_parsed_variables()
            .get("copperroute__router__max_passes"),
        Some(&"50".to_string())
    );

    let mixed = source(&[("CopperRoute__Router__Max_Passes", "50")]);
    assert_eq!(mixed.get_settings().unwrap().max_passes, Some(50));
    assert_eq!(
        mixed
            .get_parsed_variables()
            .get("CopperRoute__Router__Max_Passes"),
        Some(&"50".to_string())
    );
}

#[test]
fn a_nested_router_setting_is_parsed() {
    let s = source(&[("COPPERROUTE__ROUTER__OPTIMIZER__MAX_THREADS", "8")]);
    let settings = s.get_settings().unwrap();
    assert_eq!(settings.optimizer.as_ref().unwrap().max_threads, Some(8));
    assert_eq!(s.get_parsed_count(), 1);
}

#[test]
fn a_boolean_setting_is_parsed() {
    let s = source(&[("COPPERROUTE__ROUTER__VIAS_ALLOWED", "false")]);
    assert_eq!(s.get_settings().unwrap().vias_allowed, Some(false));
    assert_eq!(s.get_parsed_count(), 1);
}

#[test]
fn several_settings_are_parsed_together() {
    let s = source(&[
        ("COPPERROUTE__ROUTER__MAX_PASSES", "50"),
        ("COPPERROUTE__ROUTER__OPTIMIZER__MAX_THREADS", "4"),
        ("COPPERROUTE__ROUTER__VIAS_ALLOWED", "true"),
    ]);
    let settings = s.get_settings().unwrap();
    assert_eq!(settings.max_passes, Some(50));
    assert_eq!(settings.optimizer.as_ref().unwrap().max_threads, Some(4));
    assert_eq!(settings.vias_allowed, Some(true));
    assert_eq!(s.get_parsed_count(), 3);
}

#[test]
fn optimizer_strategies_and_the_hybrid_ratio() {
    let s = source(&[
        (
            "COPPERROUTE__ROUTER__OPTIMIZER__BOARD_UPDATE_STRATEGY",
            "GREEDY",
        ),
        ("COPPERROUTE__ROUTER__OPTIMIZER__HYBRID_RATIO", "1:1"),
        (
            "COPPERROUTE__ROUTER__OPTIMIZER__ITEM_SELECTION_STRATEGY",
            "SEQUENTIAL",
        ),
    ]);
    let optimizer = s.get_settings().unwrap().optimizer.as_ref().unwrap();
    assert_eq!(
        optimizer.board_update_strategy,
        Some(BoardUpdateStrategy::Greedy)
    );
    assert_eq!(optimizer.hybrid_ratio.as_deref(), Some("1:1"));
    assert_eq!(
        optimizer.item_selection_strategy,
        Some(ItemSelectionStrategy::Sequential)
    );
    assert_eq!(s.get_parsed_count(), 3);

    let lower = source(&[(
        "COPPERROUTE__ROUTER__OPTIMIZER__BOARD_UPDATE_STRATEGY",
        "greedy",
    )]);
    assert_eq!(
        lower
            .get_settings()
            .unwrap()
            .optimizer
            .as_ref()
            .unwrap()
            .board_update_strategy,
        Some(BoardUpdateStrategy::Greedy)
    );
}

#[test]
fn a_double_array_leaf_is_comma_split() {
    let s = source(&[
        (
            "COPPERROUTE__ROUTER__SCORING__PREFERRED_DIRECTION_TRACE_COST",
            "1.5,2.0",
        ),
        (
            "COPPERROUTE__ROUTER__SCORING__UNDESIRED_DIRECTION_TRACE_COST",
            "2.5,3.0",
        ),
    ]);
    let scoring = s.get_settings().unwrap().scoring.as_ref().unwrap();
    assert_eq!(
        scoring.preferred_direction_trace_cost.as_deref(),
        Some(&[1.5, 2.0][..])
    );
    assert_eq!(
        scoring.undesired_direction_trace_cost.as_deref(),
        Some(&[2.5, 3.0][..])
    );
    assert_eq!(s.get_parsed_count(), 2);
}

#[test]
fn a_string_setting_keeps_its_hyphens() {
    let s = source(&[("COPPERROUTE__ROUTER__ALGORITHM", "copperroute-router-v19")]);
    assert_eq!(
        s.get_settings().unwrap().algorithm.as_deref(),
        Some("copperroute-router-v19")
    );
    assert_eq!(s.get_parsed_count(), 1);
}

#[test]
fn save_intermediate_stages_is_parsed() {
    let s = source(&[("COPPERROUTE__ROUTER__SAVE_INTERMEDIATE_STAGES", "true")]);
    assert_eq!(
        s.get_settings().unwrap().save_intermediate_stages,
        Some(true)
    );
    assert_eq!(s.get_parsed_count(), 1);
}

#[test]
fn an_unknown_property_is_skipped_not_fatal() {
    let s = source(&[
        ("COPPERROUTE__ROUTER__INVALID_PROPERTY", "value"),
        ("COPPERROUTE__ROUTER__MAX_PASSES", "50"),
    ]);
    assert_eq!(s.get_settings().unwrap().max_passes, Some(50));
    assert_eq!(s.get_parsed_count(), 1);
    assert_eq!(
        s.errors(),
        [MergeError::NoSuchField {
            path: "INVALID_PROPERTY".to_string()
        }]
    );
}

#[test]
fn a_bad_value_is_skipped_not_fatal() {
    let s = source(&[("COPPERROUTE__ROUTER__MAX_PASSES", "abc")]);
    assert_eq!(s.get_settings().unwrap().max_passes, None);
    assert_eq!(s.get_parsed_count(), 0);
    assert_eq!(
        s.errors(),
        [MergeError::NumberFormat {
            path: "MAX_PASSES".to_string(),
            value: "abc".to_string()
        }]
    );
}

#[test]
fn keys_outside_the_router_prefix_are_ignored() {
    let s = source(&[
        ("MAX_PASSES", "100"),
        ("ROUTER__MAX_PASSES", "200"),
        ("COPPERROUTE__GUI__INPUT_DIRECTORY", "/some/path"),
        ("PATH", "/usr/bin"),
        ("HOME", "/home/user"),
    ]);
    assert_eq!(s.get_settings().unwrap().max_passes, None);
    assert_eq!(s.get_parsed_count(), 0);
    assert!(s.errors().is_empty());

    let mixed = source(&[
        ("COPPERROUTE__GUI__INPUT_DIRECTORY", "/some/path"),
        ("COPPERROUTE__ROUTER__MAX_PASSES", "100"),
    ]);
    assert_eq!(mixed.get_settings().unwrap().max_passes, Some(100));
    assert_eq!(mixed.get_parsed_count(), 1);
}

#[test]
fn get_parsed_variables_returns_every_key_that_landed() {
    let s = source(&[
        ("COPPERROUTE__ROUTER__MAX_PASSES", "100"),
        ("COPPERROUTE__ROUTER__OPTIMIZER__MAX_THREADS", "4"),
    ]);
    let parsed = s.get_parsed_variables();
    assert_eq!(parsed.len(), 2);
    assert_eq!(
        parsed.get("COPPERROUTE__ROUTER__MAX_PASSES"),
        Some(&"100".to_string())
    );
    assert_eq!(
        parsed.get("COPPERROUTE__ROUTER__OPTIMIZER__MAX_THREADS"),
        Some(&"4".to_string())
    );
}

#[test]
fn environment_variables_merge_over_the_defaults() {
    let merger = SettingsMerger::new(vec![
        Box::new(DefaultSettings::new(&host())),
        Box::new(source(&[
            ("COPPERROUTE__ROUTER__MAX_PASSES", "150"),
            ("COPPERROUTE__ROUTER__OPTIMIZER__MAX_THREADS", "6"),
            ("COPPERROUTE__ROUTER__VIAS_ALLOWED", "false"),
            ("COPPERROUTE__ROUTER__ALGORITHM", "copperroute-router-v19"),
        ])),
    ]);
    let merged = merger.merge(&host());
    assert_eq!(merged.max_passes, Some(150));
    assert_eq!(merged.optimizer.as_ref().unwrap().max_threads, Some(6));
    assert_eq!(merged.algorithm.as_deref(), Some("copperroute-router-v19"));
    assert_eq!(merged.vias_allowed, Some(false));
}

#[test]
fn from_process_env_reads_the_real_environment_without_panicking() {
    let s = EnvironmentVariablesSource::from_process_env();
    assert!(s.get_settings().is_some());
    assert_eq!(s.get_parsed_count(), s.get_parsed_variables().len());
}
