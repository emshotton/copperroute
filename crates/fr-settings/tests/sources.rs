//! `settings/{SettingsSource,SettingsMerger}.java` and the five in-scope
//! `settings/sources/**` classes — ports of `settings/SettingsMergerTest.java`,
//! `settings/sources/{DsnFileSettingsTest,RulesFileSettingsTest}.java` and the seeded half of
//! `settings/BendCostSettingsTest.defaultBendCost` (Task 4's hand-off), plus the JVM-pinned
//! default table and the `DsnRouterSettings` round trip (Plan 3 ruling 5).
//!
//! # JVM goldens — the command
//!
//! Every expected value below came out of `crates/fr-settings/tests/data/SProbe.java` run
//! against the clone-HEAD jar (plan ruling 7), with `availableProcessors` pinned to 4 so the
//! two `Math.max(1, availableProcessors - 1)` defaults are reproducible:
//!
//! ```sh
//! JAR=/Users/em/Development/freerouting/freerouting/build/libs/freerouting-current-executable.jar
//! ls -la "$JAR"   # 63 288 650 bytes, mtime 2026-08-27 20:03
//! /opt/homebrew/opt/openjdk@25/bin/javac -cp "$JAR" -d . SProbe.java
//! /opt/homebrew/opt/openjdk@25/bin/java -XX:ActiveProcessorCount=4 -Djava.awt.headless=true \
//!     -cp "$JAR:." SProbe /Users/em/Development/freerouting/freerouting/fixtures
//! ```
//!
//! The transcript is in `.superpowers/sdd/2026-08-28-plan-4-settings/task-6-report.md`; probe
//! rows are cited as `SProbe A.*` … `SProbe G.*` throughout.
//!
//! # The ten `SettingsMergerTest` cases that are *not* here
//!
//! `multipleSourcesMerging`, `priorityOrdering`, `partialOverrides`, `environmentVariablesPriority`,
//! `complexMerging`, `cliCanDisableRouterAndOptimizer`, `legacyBatchModeEnablesRouterWhenJsonDisablesIt`,
//! `explicitRouterEnabledFalseOverridesLegacyBatchMode` and `cliRoutableLayersDoesNotDisableRouter`
//! all need `EnvironmentVariablesSource` or `CliSettings`, which Task 7 owns; they are ported
//! there. `layersArrayNotShrunkOnMerge` (:259-277) is `copy_fields`'s object-array rule and is
//! already pinned by `tests/copy_fields.rs` (Task 2).

use fr_dsn::parser::DsnRouterSettings;
use fr_settings::sources::{
    ApiSettings, DefaultSettings, DsnFileSettings, RulesFileSettings, SesFileSettings,
};
use fr_settings::{
    BoardUpdateStrategy, HostEnvironment, ItemSelectionStrategy, RouterSettings, SettingsMerger,
    SettingsSource, SourceKind, priority,
};

/// The processor count every expectation in this file was taken at (`SProbe` header line
/// `# availableProcessors = 4`).
fn host() -> HostEnvironment {
    HostEnvironment::with_processors(4)
}

fn boxed<S: SettingsSource + 'static>(source: S) -> Box<dyn SettingsSource> {
    Box::new(source)
}

/// `SettingsMergerTest.nullValueHandling`'s anonymous source (:98-114): priority 100, no
/// settings at all.
struct NullSource;

impl SettingsSource for NullSource {
    fn get_settings(&self) -> Option<&RouterSettings> {
        None
    }

    fn get_source_name(&self) -> String {
        "Null Source".to_string()
    }

    fn get_priority(&self) -> i32 {
        100
    }

    fn kind(&self) -> SourceKind {
        SourceKind::Custom("Null Source")
    }
}

/// A source that hands back a caller-supplied `RouterSettings` at a caller-chosen priority — the
/// shape of the anonymous `SettingsSource` at `:196-212` inside
/// `SettingsMergerTest.legacyBatchModeEnablesRouterWhenJsonDisablesIt` (:191-224).
struct FixedSource {
    settings: RouterSettings,
    priority: i32,
    name: &'static str,
}

impl SettingsSource for FixedSource {
    fn get_settings(&self) -> Option<&RouterSettings> {
        Some(&self.settings)
    }

    fn get_source_name(&self) -> String {
        self.name.to_string()
    }

    fn get_priority(&self) -> i32 {
        self.priority
    }

    fn kind(&self) -> SourceKind {
        SourceKind::Custom("Fixed Source")
    }
}

// ---------------------------------------------------------------------------------------------
// SettingsMergerTest.java
// ---------------------------------------------------------------------------------------------

/// `SettingsMergerTest.defaultSettingsOnly` (:20-31), plus the two values `SProbe B` adds:
/// `maxThreads` survives `validate()` unchanged at 3 of 4 processors, and the merged layer count
/// is still 0 because `DefaultSettings` deliberately leaves `layers` null.
#[test]
fn default_settings_only() {
    let host = host();
    let merged = SettingsMerger::new(vec![boxed(DefaultSettings::new(&host))]).merge(&host);

    assert_eq!(merged.max_passes, Some(9999));
    assert!(merged.get_run_router());
    assert!(merged.get_vias_allowed());
    assert!(merged.get_run_optimizer());
    // SProbe B.defaultSettingsOnly.maxThreads = 3, .layerCount = 0
    assert_eq!(merged.max_threads, Some(3));
    assert_eq!(merged.get_layer_count(), 0);
}

/// `SettingsMergerTest.emptySourcesList` (:33-41).
///
/// The point of the test is the **early return** at `SettingsMerger.java:134-137`: it returns
/// `new RouterSettings()` *without* calling `validate()`, which is the only reason this does not
/// hit quirk Q5's `NullPointerException` (`docs/java-quirks.md` #125) on the null
/// `maxPasses`. `SProbe B.emptySourcesList`
/// confirms the returned object is a real `new RouterSettings()` — null `maxPasses`, non-null
/// `fanout`/`optimizer`/`scoring`.
#[test]
fn empty_sources_list() {
    let merged = SettingsMerger::new(Vec::new()).merge(&host());

    assert_eq!(merged.max_passes, None);
    assert!(merged.fanout.is_some());
    assert!(merged.optimizer.is_some());
    assert!(merged.scoring.is_some());
}

/// The other half of the early return: a merger that has sources but whose every source returns
/// `None` falls through to `new RouterSettings()` **and then calls `validate()`**
/// (`SettingsMerger.java:184-189`), which dereferences the null `maxPasses`.
///
/// `SProbe B.onlyNullSource = java.lang.NullPointerException`. Reproduced as the documented panic
/// of [`RouterSettings::validate`] (quirk #125).
#[test]
#[should_panic(expected = "maxPasses is dereferenced unboxed")]
fn only_null_sources_reproduces_javas_npe() {
    let _merged = SettingsMerger::new(vec![boxed(NullSource)]).merge(&host());
}

/// `SettingsMergerTest.sourcePriorities` (:43-51), widened to the whole ladder
/// (`SettingsSource.java:35-40`) and to every source name this task ports (`SProbe A.*`, `D.*`,
/// `E.*`, `F.*`).
#[test]
fn source_priorities_and_names() {
    let host = host();
    let defaults = DefaultSettings::new(&host);
    assert_eq!(defaults.get_priority(), 0);
    assert_eq!(defaults.get_source_name(), "Default Settings");

    assert_eq!(priority::DEFAULT, 0);
    assert_eq!(priority::JSON_FILE, 10);
    assert_eq!(priority::DSN_FILE, 20);
    assert_eq!(priority::SES_FILE, 30);
    assert_eq!(priority::RULES_FILE, 40);
    // 65, not the 50 `SettingsSource.java:36-39`'s javadoc claims — `GuiSettingsSource.PRIORITY`
    // is 65 (`:36`) and `SettingsMerger`'s class javadoc agrees. Quirk #138.
    assert_eq!(priority::GUI, 65);
    assert_eq!(priority::ENVIRONMENT, 55);
    assert_eq!(priority::CLI, 60);
    assert_eq!(priority::API, 70);

    let ses = SesFileSettings::new("board.ses");
    assert_eq!(ses.get_priority(), 30);
    assert_eq!(ses.get_source_name(), "SES file: board.ses");

    let rules = RulesFileSettings::new(&b""[..], "dummy.rules");
    assert_eq!(rules.get_priority(), 40);
    assert_eq!(rules.get_source_name(), "RULES file: dummy.rules");

    let api = ApiSettings::new(None);
    assert_eq!(api.get_priority(), 70);
    assert_eq!(api.get_source_name(), "API Settings");
}

/// `SettingsMergerTest.nullValueHandling` (:92-121): a source that returns no settings at all is
/// skipped (`SettingsMerger.java:150-158`) and does not disturb the base.
#[test]
fn null_value_handling() {
    let host = host();
    let merged = SettingsMerger::new(vec![boxed(DefaultSettings::new(&host)), boxed(NullSource)])
        .merge(&host);

    assert_eq!(merged.max_passes, Some(9999));
}

/// `SettingsMerger.merge`'s sort is `List.sort(Comparator.comparingInt(...))` (:143), which is
/// **stable** — so a source registered after `DefaultSettings` at the same priority 0 is applied
/// *on top of* it rather than becoming the base.
///
/// `SProbe B.stableSortTie.maxPasses = 42`.
#[test]
fn merge_sorts_stably_so_a_priority_tie_keeps_registration_order() {
    let host = host();
    let mut tie = RouterSettings::new();
    tie.max_passes = Some(42);
    let merged = SettingsMerger::new(vec![
        boxed(DefaultSettings::new(&host)),
        boxed(FixedSource {
            settings: tie,
            priority: 0,
            name: "Tie Source",
        }),
    ])
    .merge(&host);

    assert_eq!(merged.max_passes, Some(42));
}

/// `SettingsMerger.addOrReplaceSources` (:107-126): a new source of a kind already registered
/// **replaces** it in place; anything else is appended.
///
/// `SProbe C`: 1 source after the constructor, still 1 after a second `DefaultSettings`, 2 after
/// a `SesFileSettings`, still 2 after a second `SesFileSettings`.
#[test]
fn add_or_replace_sources_replaces_on_the_same_kind() {
    let host = host();
    let mut merger = SettingsMerger::new(vec![boxed(DefaultSettings::new(&host))]);
    assert_eq!(merger.sources().len(), 1);

    merger.add_or_replace_sources(vec![boxed(DefaultSettings::new(&host))]);
    assert_eq!(merger.sources().len(), 1);

    merger.add_or_replace_sources(vec![boxed(SesFileSettings::new("a.ses"))]);
    assert_eq!(merger.sources().len(), 2);

    merger.add_or_replace_sources(vec![boxed(SesFileSettings::new("b.ses"))]);
    assert_eq!(merger.sources().len(), 2);
    // The replacement is in place, at the *replaced* source's index (`sources.set(i, …)`, :121).
    assert_eq!(merger.sources()[1].get_source_name(), "SES file: b.ses");
}

// ---------------------------------------------------------------------------------------------
// DefaultSettings.java — the whole table, JVM-pinned
// ---------------------------------------------------------------------------------------------

/// Every value `DefaultSettings.getSettings()` assigns (`DefaultSettings.java:96-155`), against
/// the `SProbe A` transcript.
#[test]
#[allow(clippy::too_many_lines)]
fn default_settings_pins_every_java_value() {
    let host = host();
    let source = DefaultSettings::new(&host);
    let s = source
        .get_settings()
        .expect("DefaultSettings is never null");

    assert_eq!(s.enabled, Some(true));
    assert_eq!(s.algorithm.as_deref(), Some("freerouting-router"));
    assert_eq!(s.copper_to_edge_clearance_um, Some(500.0));
    assert_eq!(s.hole_clearance_um, Some(0.0));
    assert_eq!(s.neck_width_um, Some(0.0));
    assert_eq!(s.strict_drc, Some(false));
    assert_eq!(s.job_timeout_string.as_deref(), Some("12:00:00"));
    assert_eq!(s.max_passes, Some(9999));
    assert_eq!(s.max_items, Some(i32::MAX));
    assert_eq!(s.save_intermediate_stages, Some(false));
    assert_eq!(s.ignore_net_classes.as_deref(), Some(&[][..]));
    assert_eq!(s.trace_pull_tight_accuracy, Some(500));
    assert_eq!(s.vias_allowed, Some(true));
    assert_eq!(s.automatic_neckdown, Some(true));
    assert_eq!(s.max_threads, Some(3)); // Math.max(1, 4 - 1)
    assert_eq!(s.result_json_path, None);
    // `layers` is left null on purpose (DefaultSettings.java:88-93, :112-114).
    assert_eq!(s.layers, None);
    assert_eq!(s.get_layer_count(), 0);

    let fanout = s
        .fanout
        .as_ref()
        .expect("new RouterSettings() allocates it");
    assert_eq!(fanout.enabled, Some(true));
    assert_eq!(fanout.max_passes, Some(20));
    assert_eq!(fanout.max_items, Some(i32::MAX));
    assert_eq!(fanout.max_milliseconds_per_pin, Some(10_000));
    assert_eq!(fanout.ripup_allowed, Some(true));
    assert_eq!(fanout.min_escape_length_mm, Some(2.5));
    assert_eq!(fanout.max_escape_length_mm, Some(4.5));
    assert_eq!(fanout.start_via_diameter_mm, Some(0.250));
    assert_eq!(fanout.end_via_diameter_mm, Some(0.250));
    assert_eq!(fanout.pin_sorting_order.as_deref(), Some("outer_first"));
    assert_eq!(fanout.fallback_to_board_vias, Some(true));
    assert_eq!(fanout.timeout_string, None);

    let optimizer = s
        .optimizer
        .as_ref()
        .expect("new RouterSettings() allocates it");
    assert_eq!(optimizer.enabled, Some(true));
    assert_eq!(
        optimizer.algorithm.as_deref(),
        Some("freerouting-optimizer")
    );
    assert_eq!(optimizer.max_passes, Some(100));
    assert_eq!(optimizer.max_items, Some(i32::MAX));
    assert_eq!(optimizer.max_threads, Some(3)); // Math.max(1, 4 - 1)
    assert_eq!(optimizer.optimization_improvement_threshold, Some(0.01));
    assert_eq!(optimizer.max_consecutive_failures, Some(50));
    assert_eq!(optimizer.additional_ripup_cost_factor_at_start, Some(10));
    assert_eq!(optimizer.trace_ripup_cost_factor, Some(0.6));
    assert_eq!(optimizer.max_autoroute_passes, Some(6));
    assert_eq!(
        optimizer.board_update_strategy,
        Some(BoardUpdateStrategy::Greedy)
    );
    assert_eq!(optimizer.hybrid_ratio.as_deref(), Some("1:1"));
    assert_eq!(
        optimizer.item_selection_strategy,
        Some(ItemSelectionStrategy::Prioritized)
    );
    assert_eq!(optimizer.timeout_string, None);

    let scoring = s
        .scoring
        .as_ref()
        .expect("new RouterSettings() allocates it");
    // Both per-layer arrays stay null for the same reason `layers` does.
    assert_eq!(scoring.preferred_direction_trace_cost, None);
    assert_eq!(scoring.undesired_direction_trace_cost, None);
    assert_eq!(scoring.default_preferred_direction_trace_cost, Some(1.0));
    assert_eq!(scoring.default_undesired_direction_trace_cost, Some(1.0));
    assert_eq!(scoring.via_costs, Some(50));
    assert_eq!(scoring.plane_via_costs, Some(5));
    assert_eq!(scoring.start_ripup_costs, Some(100));
    assert_eq!(scoring.unrouted_net_penalty, Some(5_000_000.0));
    assert_eq!(scoring.clearance_violation_penalty, Some(1_000_000.0));
    assert_eq!(scoring.bend_penalty, Some(10.0));
    // BendCostSettingsTest.defaultBendCost (:12-18) — Task 4's hand-off.
    assert_eq!(scoring.default_bend_cost, Some(0.0));
}

/// The `DEFAULT_*` constants are the single source of truth (`DefaultSettings.java:12-14`), so
/// the table above must be expressible through them and nothing may repeat a magic number.
#[test]
fn default_settings_constants_match_the_table() {
    let host = host();
    let source = DefaultSettings::new(&host);
    let s = source.get_settings().expect("never null");
    let scoring = s.scoring.as_ref().expect("allocated");

    assert_eq!(
        scoring.unrouted_net_penalty,
        Some(DefaultSettings::DEFAULT_UNROUTED_NET_PENALTY)
    );
    assert_eq!(
        scoring.clearance_violation_penalty,
        Some(DefaultSettings::DEFAULT_CLEARANCE_VIOLATION_PENALTY)
    );
    assert_eq!(
        scoring.bend_penalty,
        Some(DefaultSettings::DEFAULT_BEND_PENALTY)
    );
    assert_eq!(scoring.via_costs, Some(DefaultSettings::DEFAULT_VIA_COSTS));
    assert_eq!(
        scoring.plane_via_costs,
        Some(DefaultSettings::DEFAULT_PLANE_VIA_COSTS)
    );
    assert_eq!(
        scoring.start_ripup_costs,
        Some(DefaultSettings::DEFAULT_START_RIPUP_COSTS)
    );
    assert_eq!(
        scoring.default_preferred_direction_trace_cost,
        Some(DefaultSettings::DEFAULT_PREFERRED_DIRECTION_TRACE_COST)
    );
    assert_eq!(
        scoring.default_undesired_direction_trace_cost,
        Some(DefaultSettings::DEFAULT_UNDESIRED_DIRECTION_TRACE_COST)
    );
    assert_eq!(
        s.copper_to_edge_clearance_um,
        Some(DefaultSettings::DEFAULT_COPPER_TO_EDGE_CLEARANCE_UM)
    );
    assert_eq!(
        s.hole_clearance_um,
        Some(DefaultSettings::DEFAULT_HOLE_CLEARANCE_UM)
    );
}

/// `DefaultSettings.getSettings()` builds a **fresh** object on every call (`SProbe
/// A.sameInstance = false`); this port computes it once in `new` because the trait hands back a
/// reference. The observable consequence — that a merge never mutates a source's settings —
/// is what this pins: merging twice from the same instance gives the same answer.
#[test]
fn default_settings_are_not_mutated_by_a_merge() {
    let host = host();
    let source = DefaultSettings::new(&host);
    let before = source.get_settings().expect("never null").max_passes;

    let mut merger = SettingsMerger::new(vec![boxed(DefaultSettings::new(&host))]);
    let mut override_max_passes = RouterSettings::new();
    override_max_passes.max_passes = Some(11);
    merger.add_or_replace_sources(vec![boxed(FixedSource {
        settings: override_max_passes,
        priority: 70,
        name: "API-ish",
    })]);
    assert_eq!(merger.merge(&host).max_passes, Some(11));
    assert_eq!(merger.merge(&host).max_passes, Some(11));
    assert_eq!(
        source.get_settings().expect("never null").max_passes,
        before
    );
}

// ---------------------------------------------------------------------------------------------
// SesFileSettings.java and ApiSettings.java
// ---------------------------------------------------------------------------------------------

/// `SesFileSettings.getSettings` returns an unconditional `new RouterSettings()`
/// (`SesFileSettings.java:27-36`) — plan ruling 1's "the SES tier has no behaviour to port".
///
/// `SProbe D.ses`: not null, `maxPasses = null`, `layerCount = 0`.
#[test]
fn ses_file_settings_is_a_structural_no_op() {
    let ses = SesFileSettings::new("board.ses");
    let s = ses.get_settings().expect("never null");
    assert_eq!(s.max_passes, None);
    assert_eq!(s.get_layer_count(), 0);
    assert!(s.fanout.is_some());
    assert!(s.optimizer.is_some());
    assert!(s.scoring.is_some());
}

/// `ApiSettings.ApiSettings` (:20-22): `null` becomes a blank `RouterSettings`; anything else is
/// carried through verbatim. `SProbe D.apiNull.maxPasses = null`, `D.api.maxPasses = 7`.
#[test]
fn api_settings_wraps_or_blanks() {
    assert_eq!(
        ApiSettings::new(None)
            .get_settings()
            .expect("never null")
            .max_passes,
        None
    );

    let mut payload = RouterSettings::new();
    payload.max_passes = Some(7);
    assert_eq!(
        ApiSettings::new(Some(payload))
            .get_settings()
            .expect("never null")
            .max_passes,
        Some(7)
    );
}

// ---------------------------------------------------------------------------------------------
// RulesFileSettingsTest.java — this plan's golden inputs
// ---------------------------------------------------------------------------------------------

/// `RulesFileSettingsTest.priorityIs40` (:16-21), plus the "file does not exist" arm of
/// `RulesFileSettings(String)` (:57-69) that test drives. `SProbe D.rulesMissing`.
#[test]
fn rules_file_settings_priority_is_40() {
    let source = RulesFileSettings::from_path(std::path::Path::new("dummy.rules"));
    assert_eq!(source.get_priority(), 40);
    assert_eq!(source.get_source_name(), "RULES file: dummy.rules");
    let s = source.get_settings().expect("never null");
    assert_eq!(s.max_passes, None);
    assert_eq!(s.get_layer_count(), 0);
}

/// `RulesFileSettingsTest.parsesAutorouteSettingsFromProcessorRules` (:23-54), plus the raw
/// field view `SProbe E.processorRaw` prints — which is what the merge engine actually sees.
#[test]
fn rules_file_settings_parses_processor_z80_rules() {
    if !parity::require_java_dir() {
        return;
    }
    let path = parity::fixture("Issue191-processor.Z80/processor.rules");
    let bytes = std::fs::read(&path).expect("golden fixture");
    let source = RulesFileSettings::new(&bytes[..], "processor.rules");
    assert_eq!(source.get_source_name(), "RULES file: processor.rules");
    assert_eq!(source.get_priority(), 40);
    let s = source.get_settings().expect("never null");

    assert!(s.get_vias_allowed());
    assert_eq!(s.get_via_costs(), 50);
    assert_eq!(s.get_plane_via_costs(), 5);
    assert_eq!(s.get_start_ripup_costs(), 100);
    assert!(s.get_run_router());
    assert!(s.get_run_optimizer());
    assert_eq!(s.get_layer_count(), 2);

    assert!(s.get_layer_active(0));
    assert!(s.get_layer_active(1));

    // Layer 0: F.Cu horizontal, cost 1.0 / 2.5
    assert!(s.get_preferred_direction_is_horizontal(0));
    assert_eq!(s.get_preferred_direction_trace_costs(0), 1.0);
    assert_eq!(s.get_against_preferred_direction_trace_costs(0), 2.5);

    // Layer 1: B.Cu vertical, cost 1.0 / 1.7
    assert!(!s.get_preferred_direction_is_horizontal(1));
    assert_eq!(s.get_preferred_direction_trace_costs(1), 1.0);
    assert_eq!(s.get_against_preferred_direction_trace_costs(1), 1.7);

    // The raw view (SProbe E.processorRaw): only the fields the `(autoroute_settings)` scope
    // names are non-null, and both `preferredDirectionHorizontal`s are explicit here.
    assert_eq!(s.enabled, Some(true));
    assert_eq!(s.vias_allowed, Some(true));
    assert_eq!(s.algorithm, None);
    assert_eq!(s.max_passes, None);
    let layers = s.layers.as_ref().expect("2 layers");
    assert_eq!(layers[0].routable, Some(true));
    assert_eq!(layers[0].preferred_direction_horizontal, Some(true));
    assert_eq!(layers[0].bend_cost, None);
    assert_eq!(layers[1].preferred_direction_horizontal, Some(false));
    let scoring = s.scoring.as_ref().expect("allocated");
    assert_eq!(
        scoring.preferred_direction_trace_cost.as_deref(),
        Some(&[1.0, 1.0][..])
    );
    assert_eq!(
        scoring.undesired_direction_trace_cost.as_deref(),
        Some(&[2.5, 1.7][..])
    );
    assert_eq!(scoring.default_bend_cost, None);
    // `(fanout off)` is read and discarded by AutorouteSettings.readScope (:45-46).
    assert_eq!(s.fanout.as_ref().expect("allocated").enabled, None);
    assert_eq!(s.optimizer.as_ref().expect("allocated").enabled, Some(true));
}

/// `RulesFileSettingsTest.parsesAutorouteSettingsFromHw48naRules` (:56-80). Note the source name:
/// the `File` constructor uses `file.getName()` (:39), i.e. the **basename**, not the path —
/// `SProbe E.hw48na.getSourceName = RULES file: Issue029-hw48na_valid.rules`.
#[test]
fn rules_file_settings_parses_hw48na_rules() {
    if !parity::require_java_dir() {
        return;
    }
    let source = RulesFileSettings::from_path(&parity::fixture("Issue029-hw48na_valid.rules"));
    assert_eq!(
        source.get_source_name(),
        "RULES file: Issue029-hw48na_valid.rules"
    );
    let s = source.get_settings().expect("never null");

    assert!(s.get_vias_allowed());
    assert_eq!(s.get_via_costs(), 50);
    assert_eq!(s.get_plane_via_costs(), 5);
    assert_eq!(s.get_start_ripup_costs(), 100);
    assert_eq!(s.get_layer_count(), 2);

    // Layer 0: F.Cu vertical, cost 1.0 / 2.0
    assert!(!s.get_preferred_direction_is_horizontal(0));
    assert_eq!(s.get_preferred_direction_trace_costs(0), 1.0);
    assert_eq!(s.get_against_preferred_direction_trace_costs(0), 2.0);

    // Layer 1: B.Cu horizontal, cost 1.0 / 2.0
    assert!(s.get_preferred_direction_is_horizontal(1));
    assert_eq!(s.get_preferred_direction_trace_costs(1), 1.0);
    assert_eq!(s.get_against_preferred_direction_trace_costs(1), 2.0);

    let scoring = s.scoring.as_ref().expect("allocated");
    assert_eq!(
        scoring.undesired_direction_trace_cost.as_deref(),
        Some(&[2.0, 2.0][..])
    );
}

// ---------------------------------------------------------------------------------------------
// DsnFileSettingsTest.java — quirk Q18 (`docs/java-quirks.md` #128)
// ---------------------------------------------------------------------------------------------

/// `DsnFileSettingsTest` (:36-98), all five of its assertions, plus the Q18 evidence the brief
/// asks for: the seeded source carries a full `layers[]` of `routable = Some(true)` **and** both
/// cost arrays of `1.0` at priority 20, for a file that has no `(autoroute_settings)` block at
/// all.
///
/// `SProbe F.Issue413-test.dsn`, `F.Issue066-Project_GP8B.dsn`, `F.Issue026-J2_reference.dsn`.
///
/// **No fixture substitution.** Java's three single-assertion tests
/// (`dsnFileSettingsReturnsNonNullSettings`, `…PriorityIs20`, `…SourceNameContainsFilename`,
/// `:76-98`) read `Issue143-rpi_splitter.dsn` through `DsnTestFixtures.openResource`, which is
/// `new FileInputStream(TestFixtures.resolveFile(filename))` (`DsnTestFixtures.java:27-32`,
/// `:59-61`) — the **`fixtures/` directory**, not a classpath resource. An earlier revision of
/// this comment claimed otherwise and substituted a different fixture on that basis; the file is
/// in `../freerouting/fixtures` like every other, so it is read here directly.
/// `Issue066-Project_GP8B.dsn` (4 layers) and `Issue026-J2_reference.dsn` stay in the loop as
/// extra layer counts. JVM rows: `SProbe F.<name>.*`.
#[test]
fn dsn_file_settings_seeds_the_layer_count() {
    if !parity::require_java_dir() {
        return;
    }
    for (name, layer_count) in [
        ("Issue413-test.dsn", 2),
        ("Issue066-Project_GP8B.dsn", 4),
        ("Issue026-J2_reference.dsn", 2),
        // The fixture Java's own three single-assertion tests use (`:76-98`).
        ("Issue143-rpi_splitter.dsn", 2),
    ] {
        let bytes = std::fs::read(parity::fixture(name)).expect("fixture");
        let source = DsnFileSettings::new(&bytes[..], name);
        assert_eq!(source.get_priority(), 20);
        assert_eq!(source.get_source_name(), format!("DSN file: {name}"));

        let s = source.get_settings().expect("never null");
        assert_eq!(s.get_layer_count(), layer_count, "{name}");

        let layers = s.layers.as_ref().expect("seeded");
        for (i, layer) in layers.iter().enumerate() {
            assert_eq!(layer.routable, Some(true), "{name} layer {i}");
            assert_eq!(
                layer.preferred_direction_horizontal, None,
                "{name} layer {i}"
            );
            assert_eq!(layer.bend_cost, None, "{name} layer {i}");
        }

        let scoring = s.scoring.as_ref().expect("allocated");
        assert_eq!(
            scoring.preferred_direction_trace_cost.as_deref(),
            Some(&vec![1.0; layer_count][..]),
            "{name}"
        );
        assert_eq!(
            scoring.undesired_direction_trace_cost.as_deref(),
            Some(&vec![1.0; layer_count][..]),
            "{name}"
        );
        // Nothing else is contributed: no `(autoroute_settings)` block means every scalar is null.
        assert_eq!(s.enabled, None, "{name}");
        assert_eq!(s.vias_allowed, None, "{name}");
        assert_eq!(scoring.via_costs, None, "{name}");
    }
}

/// Quirk Q18's (`docs/java-quirks.md` #128) consequence, merged: with `DefaultSettings` at 0
/// and `DsnFileSettings` at 20, the
/// merged settings already carry a full-length pair of all-`1.0` cost arrays, which
/// `copy_fields` rule 5 (first writer wins) then makes untouchable by every later source.
///
/// `SProbe G.merged`.
#[test]
fn dsn_source_seeds_the_arrays_that_block_later_sources() {
    if !parity::require_java_dir() {
        return;
    }
    let host = host();
    let bytes = std::fs::read(parity::fixture("Issue066-Project_GP8B.dsn")).expect("fixture");
    let merged = SettingsMerger::new(vec![
        boxed(DefaultSettings::new(&host)),
        boxed(DsnFileSettings::new(
            &bytes[..],
            "Issue066-Project_GP8B.dsn",
        )),
    ])
    .merge(&host);

    assert_eq!(merged.get_layer_count(), 4);
    let scoring = merged.scoring.as_ref().expect("allocated");
    assert_eq!(
        scoring.preferred_direction_trace_cost.as_deref(),
        Some(&[1.0, 1.0, 1.0, 1.0][..])
    );
    assert_eq!(
        scoring.undesired_direction_trace_cost.as_deref(),
        Some(&[1.0, 1.0, 1.0, 1.0][..])
    );
    // The defaults survive underneath.
    assert_eq!(merged.max_passes, Some(9999));
    assert_eq!(scoring.via_costs, Some(50));
    // Every per-layer direction is still unset — the `.rules` tier is the only thing that fills
    // them (plan ruling 1's Q1 channel).
    for layer in merged.layers.as_ref().expect("seeded") {
        assert_eq!(layer.preferred_direction_horizontal, None);
    }
}

/// A file under `crates/fr-settings/tests/data/`.
fn probe_data(name: &str) -> std::path::PathBuf {
    std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/")).join(name)
}

/// **Task 6 fix round 1, controller ruling L.** `Issue029-hw48na_reduced.rules` is the golden
/// with eight lines deleted — `(vias on)`, `(via_costs 50)`, `(plane_via_costs 5)`,
/// `(start_ripup_costs 100)` and the four per-layer trace-cost lines. Java's `readScope` calls no
/// setter for any of them, so the four scalars stay `null` and `boardSpecificTraceCostsApplied`
/// stays at the `false` that `setLayerCount` left; the merge therefore keeps `DefaultSettings`'
/// 50 / 5 / 100.
///
/// The port used to store the *coalesced defaults* in `DsnRouterSettings` (plain `bool`/`i32`
/// with no way to say "absent"), so this merged to `viaCosts = 1` and a `true` flag. Both halves
/// are JVM-measured — `SProbe H.reduced.merged.getViaCosts = 50`,
/// `H.reduced.merged.areBoardSpecificTraceCostsApplied = false`, against
/// `H.full.raw.areBoardSpecificTraceCostsApplied = true` for the unmodified file.
#[test]
fn an_unnamed_rules_field_does_not_overwrite_a_lower_priority_source() {
    let host = host();
    let reduced = std::fs::read(probe_data("Issue029-hw48na_reduced.rules")).expect("committed");

    // The source's own view: absent means absent (SProbe H.reduced.raw.*).
    let source = RulesFileSettings::new(&reduced[..], "Issue029-hw48na_reduced.rules");
    let raw = source.get_settings().expect("never null");
    assert_eq!(raw.vias_allowed, None);
    let raw_scoring = raw.scoring.as_ref().expect("allocated");
    assert_eq!(raw_scoring.via_costs, None);
    assert_eq!(raw_scoring.plane_via_costs, None);
    assert_eq!(raw_scoring.start_ripup_costs, None);
    assert!(!raw.are_board_specific_trace_costs_applied());
    // What the file *does* name still lands.
    assert_eq!(raw.enabled, Some(true));
    assert_eq!(
        raw.optimizer.as_ref().expect("allocated").enabled,
        Some(true)
    );
    assert_eq!(raw.get_layer_count(), 2);
    let raw_layers = raw.layers.as_ref().expect("seeded");
    assert_eq!(raw_layers[0].preferred_direction_horizontal, Some(false));
    assert_eq!(raw_layers[1].preferred_direction_horizontal, Some(true));
    // The arrays are present and seeded — `setLayerCount` filled them (SProbe
    // H.reduced.raw.scoring.preferredDirectionTraceCost = [1.0, 1.0]).
    assert_eq!(
        raw_scoring.preferred_direction_trace_cost.as_deref(),
        Some(&[1.0, 1.0][..])
    );
    assert_eq!(
        raw_scoring.undesired_direction_trace_cost.as_deref(),
        Some(&[1.0, 1.0][..])
    );

    // Merged under DefaultSettings: the defaults survive (SProbe H.reduced.merged.*).
    let merged = SettingsMerger::new(vec![
        boxed(DefaultSettings::new(&host)),
        boxed(RulesFileSettings::new(
            &reduced[..],
            "Issue029-hw48na_reduced.rules",
        )),
    ])
    .merge(&host);
    assert_eq!(merged.get_via_costs(), 50);
    assert_eq!(merged.get_plane_via_costs(), 5);
    assert_eq!(merged.get_start_ripup_costs(), 100);
    assert!(merged.get_vias_allowed());
    assert_eq!(merged.get_layer_count(), 2);
    assert!(!merged.are_board_specific_trace_costs_applied());

    // The unmodified golden names all four, and both trace costs, so all four land and the flag
    // is set (SProbe H.full.raw.*).
    if !parity::require_java_dir() {
        return;
    }
    let full = std::fs::read(parity::fixture("Issue029-hw48na_valid.rules")).expect("golden");
    let full = RulesFileSettings::new(&full[..], "Issue029-hw48na_valid.rules");
    let full = full.get_settings().expect("never null");
    assert_eq!(
        full.scoring.as_ref().expect("allocated").via_costs,
        Some(50)
    );
    assert_eq!(full.vias_allowed, Some(true));
    assert!(full.are_board_specific_trace_costs_applied());
}

/// `RouterSettings.isFanoutEnabled` (`:578-580`) — ported in fix round 1 because
/// `autoroute/pipeline/**` (Plan 6) gates the fanout pre-pass on it.
///
/// Absent is `false`, unlike its identically-documented neighbour `getRunFanout` (`:573-575`),
/// whose absent-default is `true` — quirk #139.
#[test]
fn is_fanout_enabled_defaults_to_false_when_absent() {
    // `new RouterSettings()` allocates `fanout` but leaves `enabled` null.
    assert!(!RouterSettings::new().is_fanout_enabled());
    // …and `Self::default()` leaves `fanout` itself absent.
    assert!(!RouterSettings::default().is_fanout_enabled());

    let mut settings = RouterSettings::new();
    settings.fanout.as_mut().expect("allocated").enabled = Some(true);
    assert!(settings.is_fanout_enabled());
    settings.fanout.as_mut().expect("allocated").enabled = Some(false);
    assert!(!settings.is_fanout_enabled());

    // DefaultSettings turns it on (DefaultSettings.java:117).
    let host = host();
    assert!(
        DefaultSettings::new(&host)
            .get_settings()
            .expect("never null")
            .is_fanout_enabled()
    );
}

// ---------------------------------------------------------------------------------------------
// Plan 3 ruling 5: DsnRouterSettings <-> RouterSettings
// ---------------------------------------------------------------------------------------------

/// The forward conversion reproduces `AutorouteSettings.readScope`'s own construction sequence
/// (`new RouterSettings()`, `setLayerCount(n)`, then the setters), so a `.rules` file read
/// through `fr-dsn` and converted here is byte-for-byte the object Java's `RulesReader` hands
/// `RulesFileSettings` — `SProbe E.processorRaw`.
#[test]
fn dsn_router_settings_converts_into_router_settings() {
    if !parity::require_java_dir() {
        return;
    }
    let bytes = std::fs::read(parity::fixture("Issue191-processor.Z80/processor.rules"))
        .expect("golden fixture");
    let dsn = fr_dsn::rules_reader::read_router_settings(&bytes[..])
        .expect("scanner error")
        .expect("processor.rules has an (autoroute_settings) scope");

    let s = RouterSettings::from(dsn);

    assert_eq!(s.enabled, Some(true));
    assert_eq!(s.vias_allowed, Some(true));
    assert_eq!(s.optimizer.as_ref().expect("allocated").enabled, Some(true));
    let scoring = s.scoring.as_ref().expect("allocated");
    assert_eq!(scoring.via_costs, Some(50));
    assert_eq!(scoring.plane_via_costs, Some(5));
    assert_eq!(scoring.start_ripup_costs, Some(100));
    assert_eq!(
        scoring.preferred_direction_trace_cost.as_deref(),
        Some(&[1.0, 1.0][..])
    );
    assert_eq!(
        scoring.undesired_direction_trace_cost.as_deref(),
        Some(&[2.5, 1.7][..])
    );
    let layers = s.layers.as_ref().expect("2 layers");
    assert_eq!(layers[0].routable, Some(true));
    assert_eq!(layers[0].preferred_direction_horizontal, Some(true));
    assert_eq!(layers[1].preferred_direction_horizontal, Some(false));
    // Everything the scope does not name stays absent.
    assert_eq!(s.algorithm, None);
    assert_eq!(s.max_threads, None);
    assert_eq!(s.result_json_path, None);
    assert_eq!(s.fanout.as_ref().expect("allocated").enabled, None);
}

/// The round trip, compared field by field (`DsnRouterSettings` holds `f64`, so it has no `Eq`).
/// Both `.rules` goldens name a direction for every layer, so nothing is lost either way.
#[test]
fn dsn_router_settings_round_trips_through_router_settings() {
    if !parity::require_java_dir() {
        return;
    }
    for name in [
        "Issue191-processor.Z80/processor.rules",
        "Issue029-hw48na_valid.rules",
    ] {
        let bytes = std::fs::read(parity::fixture(name)).expect("golden fixture");
        let before = fr_dsn::rules_reader::read_router_settings(&bytes[..])
            .expect("scanner error")
            .expect("has an (autoroute_settings) scope");

        let after = DsnRouterSettings::from(&RouterSettings::from(before.clone()));

        assert_eq!(after.run_router(), before.run_router(), "{name} run_router");
        assert_eq!(
            after.run_optimizer(),
            before.run_optimizer(),
            "{name} run_optimizer"
        );
        assert_eq!(
            after.vias_allowed(),
            before.vias_allowed(),
            "{name} vias_allowed"
        );
        assert_eq!(after.via_costs(), before.via_costs(), "{name} via_costs");
        assert_eq!(
            after.plane_via_costs(),
            before.plane_via_costs(),
            "{name} plane_via_costs"
        );
        assert_eq!(
            after.start_ripup_costs(),
            before.start_ripup_costs(),
            "{name} start_ripup_costs"
        );
        assert_eq!(
            after.get_layer_count(),
            before.get_layer_count(),
            "{name} layer_count"
        );
        for i in 0..before.get_layer_count() {
            assert_eq!(
                after.get_layer_active(i),
                before.get_layer_active(i),
                "{name} layer {i} active"
            );
            assert_eq!(
                after.get_preferred_direction_is_horizontal(i),
                before.get_preferred_direction_is_horizontal(i),
                "{name} layer {i} direction"
            );
            assert_eq!(
                after.get_preferred_direction_trace_costs(i),
                before.get_preferred_direction_trace_costs(i),
                "{name} layer {i} preferred cost"
            );
            assert_eq!(
                after.get_against_preferred_direction_trace_costs(i),
                before.get_against_preferred_direction_trace_costs(i),
                "{name} layer {i} against cost"
            );
        }
    }
}

/// Where the reverse conversion is lossy. It goes through `RouterSettings`' null-coalescing
/// getters, so **every** absent field comes back as its default: an unset
/// `preferredDirectionHorizontal` as the alternating `layer % 2 == 1` (`RouterSettings.java:743,
/// 747`), an unset `viasAllowed` as `true` (`:596`), and the three cost scalars as `1`
/// (`:538,601,614`). The forward conversion is **not** lossy in that direction any more — since
/// Task 6 fix round 1, `DsnRouterSettings` carries those four as `Option` and forwards absence as
/// absence (controller ruling L). The reverse cannot: it is what the DSN/`.rules` *writers* see,
/// and a writer must emit a value for every token.
///
/// The reverse also always ends up with `are_board_specific_trace_costs_applied() == true`,
/// because it replays the two trace-cost setters for every layer unconditionally — it has to, or
/// a `RouterSettings` that got its costs through a merge (quirk 127 clears the flag there) would
/// be written out with `1.0`s. Nothing reads that flag on a reverse-converted object; the writers
/// read only the values.
#[test]
fn reverse_conversion_applies_the_getters_defaults() {
    let mut blank = RouterSettings::new();
    blank.set_layer_count(3);
    // set_layer_count leaves every direction unset — exactly what a DSN with no `layer_rule`s
    // produces (SProbe F.*.layers[i] prefHoriz=null).
    for layer in blank.layers.as_ref().expect("seeded") {
        assert_eq!(layer.preferred_direction_horizontal, None);
    }

    let dsn = DsnRouterSettings::from(&blank);
    assert_eq!(dsn.get_layer_count(), 3);
    assert!(!dsn.get_preferred_direction_is_horizontal(0));
    assert!(dsn.get_preferred_direction_is_horizontal(1));
    assert!(!dsn.get_preferred_direction_is_horizontal(2));
    // Round-tripping *back* now yields `Some`, where the original had `None`.
    let back = RouterSettings::from(dsn);
    let layers = back.layers.as_ref().expect("3 layers");
    assert_eq!(layers[0].preferred_direction_horizontal, Some(false));
    assert_eq!(layers[1].preferred_direction_horizontal, Some(true));
    assert_eq!(layers[2].preferred_direction_horizontal, Some(false));

    // The scalar getters' defaults are applied too: a blank RouterSettings reports
    // enabled=true, optimizer disabled, vias allowed, and 1 for all three cost scalars
    // (RouterSettings.java:553, 562, 596, 538, 601, 614).
    let blank_dsn = DsnRouterSettings::from(&RouterSettings::new());
    assert!(blank_dsn.run_router());
    assert!(!blank_dsn.run_optimizer());
    assert!(blank_dsn.vias_allowed());
    assert_eq!(blank_dsn.via_costs(), 1);
    assert_eq!(blank_dsn.plane_via_costs(), 1);
    assert_eq!(blank_dsn.start_ripup_costs(), 1);
    assert_eq!(blank_dsn.get_layer_count(), 0);
}
