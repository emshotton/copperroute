//! `settings/sources/EnvironmentVariablesSource.java` — a port of all **18** `@Test` cases of
//! `settings/sources/EnvironmentVariablesSourceTest.java` (279), plus the
//! `SettingsMergerTest.complexMerging` case Task 6 deferred.
//!
//! Eighteen cases, seventeen tests: `caseInsensitivePropertyNames` (:180-192) and
//! `simpleRouterSetting` (:39-50) are the **same assertion**, character for character apart from
//! the comment — both put `FREEROUTING__ROUTER__MAX_PASSES = "100"` into the map and read
//! `maxPasses == 100` back. Its comment ("Environment variables are case-sensitive, but property
//! names are converted to lowercase") describes the upper-casing at
//! `EnvironmentVariablesSource.java:56`, which the key it uses cannot exercise; the test that
//! does is `variable_names_are_case_insensitive`. Both Java cases are named on
//! `a_simple_router_setting_is_parsed` rather than given a duplicate of their own.
//!
//! # JVM goldens — the command
//!
//! Every expected value below came out of `crates/fr-settings/tests/data/CProbe.java`, block `A`,
//! run against the clone-HEAD jar (plan ruling 7):
//!
//! ```sh
//! JAR=/Users/em/Development/freerouting/freerouting/build/libs/freerouting-current-executable.jar
//! ls -la "$JAR"   # 63 288 650 bytes, mtime 2026-08-27 20:03
//! /opt/homebrew/opt/openjdk@25/bin/javac -cp "$JAR" -d . CProbe.java
//! /opt/homebrew/opt/openjdk@25/bin/java -XX:ActiveProcessorCount=4 -Djava.awt.headless=true \
//!     -cp "$JAR:." CProbe
//! ```
//!
//! The transcript is in `.superpowers/sdd/2026-08-28-plan-4-settings/task-7-report.md`; rows are
//! cited as `CProbe A.*`.

use std::collections::BTreeMap;

use fr_settings::sources::{DefaultSettings, EnvironmentVariablesSource};
use fr_settings::{
    BoardUpdateStrategy, HostEnvironment, ItemSelectionStrategy, MergeError, SettingsMerger,
    SettingsSource, SourceKind, priority,
};

/// The processor count every expectation in this file was taken at (`-XX:ActiveProcessorCount=4`).
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

/// `EnvironmentVariablesSourceTest.priority` (:17-21) and `.sourceName` (:23-27).
/// `CProbe A.priority = 55`, `A.sourceName = Environment Variables`.
#[test]
fn priority_and_source_name() {
    let empty = source(&[]);
    assert_eq!(empty.get_priority(), 55);
    assert_eq!(empty.get_priority(), priority::ENVIRONMENT);
    assert_eq!(empty.get_source_name(), "Environment Variables");
    assert_eq!(empty.kind(), SourceKind::Environment);
}

/// `EnvironmentVariablesSourceTest.emptyEnvironment` (:29-37). `CProbe A.empty.parsedCount = 0`.
#[test]
fn empty_environment_yields_a_blank_but_present_settings_object() {
    let empty = source(&[]);
    assert_eq!(empty.get_parsed_count(), 0);
    let settings = empty.get_settings().expect("never null");
    assert_eq!(settings.max_passes, None);
    // `new RouterSettings()` allocates the three nested objects (RouterSettings.java:119-124).
    assert!(settings.optimizer.is_some());
    assert!(settings.scoring.is_some());
    assert!(settings.fanout.is_some());
}

/// `EnvironmentVariablesSourceTest.simpleRouterSetting` (:39-50) and
/// `.caseInsensitivePropertyNames` (:180-192) — the same assertion twice (see the module docs);
/// asserted here with the brief's `50` rather than Java's `100`.
/// `CProbe A1.maxPasses = 50`, `A1.parsedCount = 1`.
#[test]
fn a_simple_router_setting_is_parsed() {
    let s = source(&[("FREEROUTING__ROUTER__MAX_PASSES", "50")]);
    assert_eq!(s.get_settings().unwrap().max_passes, Some(50));
    assert_eq!(s.get_parsed_count(), 1);
}

/// `EnvironmentVariablesSourceTest.lowercaseEnvironmentVariableNames` (:244-257) — the key is
/// upper-cased *first* (`EnvironmentVariablesSource.java:56`), so variable names are
/// case-insensitive. `CProbe A2.lowercase.maxPasses = 50`, `A2b.mixedcase.maxPasses = 50`.
///
/// `getParsedVariables` is keyed by the **raw** key, not the upper-cased one (`:70`):
/// `CProbe A2.lowercase.parsedVariables = {freerouting__router__max_passes=50}`.
#[test]
fn variable_names_are_case_insensitive() {
    let lower = source(&[
        ("freerouting__router__max_passes", "50"),
        ("freerouting__router__save_intermediate_stages", "true"),
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
            .get("freerouting__router__max_passes"),
        Some(&"50".to_string())
    );

    let mixed = source(&[("FreeRouting__Router__Max_Passes", "50")]);
    assert_eq!(mixed.get_settings().unwrap().max_passes, Some(50));
    assert_eq!(
        mixed
            .get_parsed_variables()
            .get("FreeRouting__Router__Max_Passes"),
        Some(&"50".to_string())
    );
}

/// `EnvironmentVariablesSourceTest.nestedRouterSetting` (:52-63). `CProbe A3.optimizer.maxThreads
/// = 8` — `__` becomes `.`, so the path is `optimizer.max_threads`.
#[test]
fn a_nested_router_setting_is_parsed() {
    let s = source(&[("FREEROUTING__ROUTER__OPTIMIZER__MAX_THREADS", "8")]);
    let settings = s.get_settings().unwrap();
    assert_eq!(settings.optimizer.as_ref().unwrap().max_threads, Some(8));
    assert_eq!(s.get_parsed_count(), 1);
}

/// `EnvironmentVariablesSourceTest.booleanSetting` (:65-76). `CProbe A12.viasAllowed = false`.
#[test]
fn a_boolean_setting_is_parsed() {
    let s = source(&[("FREEROUTING__ROUTER__VIAS_ALLOWED", "false")]);
    assert_eq!(s.get_settings().unwrap().vias_allowed, Some(false));
    assert_eq!(s.get_parsed_count(), 1);
}

/// `EnvironmentVariablesSourceTest.multipleSettings` (:78-95).
#[test]
fn several_settings_are_parsed_together() {
    let s = source(&[
        ("FREEROUTING__ROUTER__MAX_PASSES", "50"),
        ("FREEROUTING__ROUTER__OPTIMIZER__MAX_THREADS", "4"),
        ("FREEROUTING__ROUTER__VIAS_ALLOWED", "true"),
    ]);
    let settings = s.get_settings().unwrap();
    assert_eq!(settings.max_passes, Some(50));
    assert_eq!(settings.optimizer.as_ref().unwrap().max_threads, Some(4));
    assert_eq!(settings.vias_allowed, Some(true));
    assert_eq!(s.get_parsed_count(), 3);
}

/// `EnvironmentVariablesSourceTest.optimizerStrategies` (:221-242), and the brief's lower-case
/// half: `set_field_value`'s enum matching is **case-insensitive** (`ReflectionUtil.java:158-164`,
/// unlike `copyFields`'). `CProbe A4.upper.boardUpdateStrategy = GREEDY`,
/// `A4b.lower.boardUpdateStrategy = GREEDY`.
///
/// `HYBRID_RATIO=1:1` is the case that proves the **value** is not path-split: `setFieldValue`
/// splits only the property path on `[.:-]` (quirks row 118).  `CProbe A5.hybridRatio = 1:1`.
#[test]
fn optimizer_strategies_and_the_hybrid_ratio() {
    let s = source(&[
        (
            "FREEROUTING__ROUTER__OPTIMIZER__BOARD_UPDATE_STRATEGY",
            "GREEDY",
        ),
        ("FREEROUTING__ROUTER__OPTIMIZER__HYBRID_RATIO", "1:1"),
        (
            "FREEROUTING__ROUTER__OPTIMIZER__ITEM_SELECTION_STRATEGY",
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
        "FREEROUTING__ROUTER__OPTIMIZER__BOARD_UPDATE_STRATEGY",
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

/// `EnvironmentVariablesSourceTest.traceCostSettings` (:259-278). `CProbe
/// A6.preferredDirectionTraceCost = 1.5,2.0`.
#[test]
fn a_double_array_leaf_is_comma_split() {
    let s = source(&[
        (
            "FREEROUTING__ROUTER__SCORING__PREFERRED_DIRECTION_TRACE_COST",
            "1.5,2.0",
        ),
        (
            "FREEROUTING__ROUTER__SCORING__UNDESIRED_DIRECTION_TRACE_COST",
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

/// `EnvironmentVariablesSourceTest.stringSettings` (:195-206). `CProbe A7.algorithm =
/// freerouting-router-v19` — the *value* keeps its hyphens (only the path is split).
#[test]
fn a_string_setting_keeps_its_hyphens() {
    let s = source(&[("FREEROUTING__ROUTER__ALGORITHM", "freerouting-router-v19")]);
    assert_eq!(
        s.get_settings().unwrap().algorithm.as_deref(),
        Some("freerouting-router-v19")
    );
    assert_eq!(s.get_parsed_count(), 1);
}

/// `EnvironmentVariablesSourceTest.saveIntermediateStages` (:208-219).
#[test]
fn save_intermediate_stages_is_parsed() {
    let s = source(&[("FREEROUTING__ROUTER__SAVE_INTERMEDIATE_STAGES", "true")]);
    assert_eq!(
        s.get_settings().unwrap().save_intermediate_stages,
        Some(true)
    );
    assert_eq!(s.get_parsed_count(), 1);
}

/// `EnvironmentVariablesSourceTest.invalidPropertyName` (:133-148). The unknown name is warned
/// about and **skipped** (`EnvironmentVariablesSource.java:74-80`), never fatal; the valid one in
/// the same map still lands. `CProbe A9.unknown.parsedCount = 0`, `A9.unknown.maxPasses = null`.
#[test]
fn an_unknown_property_is_skipped_not_fatal() {
    let s = source(&[
        ("FREEROUTING__ROUTER__INVALID_PROPERTY", "value"),
        ("FREEROUTING__ROUTER__MAX_PASSES", "50"),
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

/// `EnvironmentVariablesSourceTest.invalidValue` (:150-162). `CProbe A10.badValue.parsedCount =
/// 0`, `A10.badValue.maxPasses = null`.
#[test]
fn a_bad_value_is_skipped_not_fatal() {
    let s = source(&[("FREEROUTING__ROUTER__MAX_PASSES", "abc")]);
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

/// `EnvironmentVariablesSourceTest.ignoresNonRouterVariables` (:97-114) and
/// `.ignoresVariablesWithoutPrefix` (:116-131). `CProbe A11.nonRouter.parsedCount = 0`.
#[test]
fn keys_outside_the_router_prefix_are_ignored() {
    let s = source(&[
        ("MAX_PASSES", "100"),
        ("ROUTER__MAX_PASSES", "200"),
        ("FREEROUTING__GUI__INPUT_DIRECTORY", "/some/path"),
        ("PATH", "/usr/bin"),
        ("HOME", "/home/user"),
    ]);
    assert_eq!(s.get_settings().unwrap().max_passes, None);
    assert_eq!(s.get_parsed_count(), 0);
    assert!(s.errors().is_empty());

    let mixed = source(&[
        ("FREEROUTING__GUI__INPUT_DIRECTORY", "/some/path"),
        ("FREEROUTING__ROUTER__MAX_PASSES", "100"),
    ]);
    assert_eq!(mixed.get_settings().unwrap().max_passes, Some(100));
    assert_eq!(mixed.get_parsed_count(), 1);
}

/// `EnvironmentVariablesSourceTest.getParsedVariables` (:164-179).
#[test]
fn get_parsed_variables_returns_every_key_that_landed() {
    let s = source(&[
        ("FREEROUTING__ROUTER__MAX_PASSES", "100"),
        ("FREEROUTING__ROUTER__OPTIMIZER__MAX_THREADS", "4"),
    ]);
    let parsed = s.get_parsed_variables();
    assert_eq!(parsed.len(), 2);
    assert_eq!(
        parsed.get("FREEROUTING__ROUTER__MAX_PASSES"),
        Some(&"100".to_string())
    );
    assert_eq!(
        parsed.get("FREEROUTING__ROUTER__OPTIMIZER__MAX_THREADS"),
        Some(&"4".to_string())
    );
}

/// `SettingsMergerTest.complexMerging` (:152-175) — the case Task 6 deferred: environment
/// variables applied over `DefaultSettings`. Its neighbour `.environmentVariablesPriority`
/// (:141-150) asserts only `getPriority() == 55`, which
/// `priority_and_source_name` already covers.
#[test]
fn environment_variables_merge_over_the_defaults() {
    let merger = SettingsMerger::new(vec![
        Box::new(DefaultSettings::new(&host())),
        Box::new(source(&[
            ("FREEROUTING__ROUTER__MAX_PASSES", "150"),
            ("FREEROUTING__ROUTER__OPTIMIZER__MAX_THREADS", "6"),
            ("FREEROUTING__ROUTER__VIAS_ALLOWED", "false"),
            ("FREEROUTING__ROUTER__ALGORITHM", "freerouting-router-v19"),
        ])),
    ]);
    let merged = merger.merge(&host());
    assert_eq!(merged.max_passes, Some(150));
    assert_eq!(merged.optimizer.as_ref().unwrap().max_threads, Some(6));
    assert_eq!(merged.algorithm.as_deref(), Some("freerouting-router-v19"));
    // `viasAllowed` is commented out in the Java test (:171-173) because `copyFields`' rule 2
    // cannot carry a `false` **primitive**; on `RouterSettings` it is a boxed `Boolean`, so the
    // port — like Java — does carry it. Pinned so the difference is deliberate.
    assert_eq!(merged.vias_allowed, Some(false));
}

/// `EnvironmentVariablesSource()` — the `System.getenv()` overload
/// (`EnvironmentVariablesSource.java:33-35`). It must not panic and must ignore whatever the
/// developer's shell happens to export; the test asserts only that much, because the process
/// environment is not ours to control.
#[test]
fn from_process_env_reads_the_real_environment_without_panicking() {
    let s = EnvironmentVariablesSource::from_process_env();
    assert!(s.get_settings().is_some());
    assert_eq!(s.get_parsed_count(), s.get_parsed_variables().len());
}
