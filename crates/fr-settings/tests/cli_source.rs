//! `settings/sources/CliSettings.java`, the dead `LegacyBridge`
//! (`settings/GlobalSettings.java:521-838`, plan ruling 8) and `classify_de_arguments`
//! (`GlobalSettings.java:564-648`, plan ruling 10).
//!
//! Ports the five CLI cases of `settings/SettingsMergerTest.java` Task 6 deferred (:177-256),
//! `settings/NeckWidthSettingsTest.cliFlagReachesRouterSettings` (:11-15) and the whole `-de`
//! matrix of `settings/GlobalSettingsCommandLineTest.java` (:25-171).
//!
//! # JVM goldens — the command
//!
//! Every expected value below came out of `crates/fr-settings/tests/data/CProbe.java`, blocks
//! `B`–`E`, run against the clone-HEAD jar (plan ruling 7):
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
//! cited as `CProbe B.*` … `CProbe E.*`.

use fr_settings::sources::cli::{
    DeSlots, LegacyBridge, apply_command_line_arguments, classify_de_arguments,
};
use fr_settings::sources::{CliSettings, DefaultSettings};
use fr_settings::{
    BoardUpdateStrategy, HostEnvironment, ItemSelectionStrategy, RouterSettings, SettingsMerger,
    SettingsSource, SourceKind, priority,
};

/// The processor count every expectation in this file was taken at (`-XX:ActiveProcessorCount=4`).
fn host() -> HostEnvironment {
    HostEnvironment::with_processors(4)
}

fn argv(args: &[&str]) -> Vec<String> {
    args.iter().map(|a| (*a).to_string()).collect()
}

fn cli(args: &[&str]) -> CliSettings {
    CliSettings::new(&argv(args))
}

fn bridge(args: &[&str]) -> LegacyBridge {
    apply_command_line_arguments(&argv(args))
}

fn de(args: &[&str]) -> DeSlots {
    classify_de_arguments(&argv(args))
}

/// A priority-10 stand-in for `SettingsMergerTest.legacyBatchModeEnablesRouterWhenJsonDisablesIt`'s
/// anonymous JSON source (:196-212).
struct JsonTestSource {
    settings: RouterSettings,
}

impl SettingsSource for JsonTestSource {
    fn get_settings(&self) -> Option<&RouterSettings> {
        Some(&self.settings)
    }

    fn get_source_name(&self) -> String {
        "JSON test source".to_string()
    }

    fn get_priority(&self) -> i32 {
        priority::JSON_FILE
    }

    fn kind(&self) -> SourceKind {
        SourceKind::JsonFile
    }
}

fn json_source_disabling_the_router() -> JsonTestSource {
    let mut settings = RouterSettings::new();
    settings.enabled = Some(false);
    JsonTestSource { settings }
}

// ---------------------------------------------------------------------------------------------
// CliSettings
// ---------------------------------------------------------------------------------------------

/// `CliSettings.getPriority`/`getSourceName` (:117-125). `CProbe B.priority = 60`,
/// `B.sourceName = CLI Arguments`.
#[test]
fn priority_and_source_name() {
    let empty = cli(&[]);
    assert_eq!(empty.get_priority(), 60);
    assert_eq!(empty.get_priority(), priority::CLI);
    assert_eq!(empty.get_source_name(), "CLI Arguments");
    assert_eq!(empty.kind(), SourceKind::Cli);
    assert!(empty.get_settings().is_some());
}

/// `NeckWidthSettingsTest.cliFlagReachesRouterSettings` (:11-15). `CProbe B7.neckWidthUm = 250.0`.
#[test]
fn a_long_option_reaches_the_router_settings() {
    let cli = cli(&["--router.neck_width_um=250"]);
    assert_eq!(cli.get_settings().unwrap().get_neck_width_um(), 250.0);
    assert_eq!(
        cli.get_parsed_arguments().get("router.neck_width_um"),
        Some(&"250".to_string())
    );
}

/// `CliSettings.java:54-56` — a `--name=value` whose name does **not** start with `router.` is
/// dropped without even trying. `CProbe B9.nonRouterIgnored.parsedArguments =
/// {router.max_passes=7}`.
#[test]
fn a_long_option_outside_the_router_namespace_is_ignored() {
    let cli = cli(&["--gui.foo=1", "--router.max_passes=7"]);
    assert_eq!(cli.get_settings().unwrap().max_passes, Some(7));
    assert_eq!(cli.get_parsed_arguments().len(), 1);
    assert!(cli.errors().is_empty());
}

/// `CliSettings.java:45` — a `--name` with no `=` at all is skipped entirely.
/// `CProbe B13.noEquals.maxPasses = null`.
#[test]
fn a_long_option_without_an_equals_sign_is_skipped() {
    let cli = cli(&["--router.max_passes"]);
    assert_eq!(cli.get_settings().unwrap().max_passes, None);
    assert!(cli.get_parsed_arguments().is_empty());
}

/// `CliSettings.java:91-92` — the `router.` prefix is stripped before `setFieldValue`, so a
/// nested path keeps working. `CProbe B14.nested.optimizer.maxThreads = 3`.
#[test]
fn a_nested_long_option_is_applied_under_the_stripped_prefix() {
    let cli = cli(&["--router.optimizer.max_threads=3"]);
    assert_eq!(
        cli.get_settings()
            .unwrap()
            .optimizer
            .as_ref()
            .unwrap()
            .max_threads,
        Some(3)
    );
}

/// `CliSettings.mapFlagToProperty` (:102-109) maps **only** `mp` and `mt`, by exact match.
/// `CProbe B1.mp5.maxPasses = 5`, `B4.mpx.parsedArguments = {}` — `-mpx` is *not* `-mp` here,
/// unlike `GlobalSettings`' `startsWith` table (see `legacy_bridge_matches_flags_by_prefix`).
#[test]
fn only_mp_and_mt_map_to_a_property() {
    assert_eq!(
        cli(&["-mp", "5"]).get_settings().unwrap().max_passes,
        Some(5)
    );
    assert_eq!(
        cli(&["-mt", "5"]).get_settings().unwrap().max_threads,
        Some(5)
    );
    assert_eq!(cli(&["-mpx", "5"]).get_settings().unwrap().max_passes, None);
    assert!(cli(&["-mpx", "5"]).get_parsed_arguments().is_empty());
    assert!(cli(&["-oit", "5"]).get_parsed_arguments().is_empty());
}

/// `CliSettings.java:61` — the value is consumed only when the next argument exists **and does
/// not start with `-`**; otherwise it is the empty string, which `setFieldValue` then fails to
/// parse. `CProbe B2.mpAtEnd.maxPasses = null`, `B3.mpMinus5.maxPasses = null`, and both leave
/// `parsedArguments` empty because the map insert happens *after* the assignment (:94-95).
#[test]
fn a_short_flag_consumes_a_value_only_when_it_does_not_start_with_a_dash() {
    let at_end = cli(&["-mp"]);
    assert_eq!(at_end.get_settings().unwrap().max_passes, None);
    assert!(at_end.get_parsed_arguments().is_empty());
    assert_eq!(at_end.errors().len(), 1);

    let negative = cli(&["-mp", "-5"]);
    assert_eq!(negative.get_settings().unwrap().max_passes, None);
    assert!(negative.get_parsed_arguments().is_empty());
    assert_eq!(negative.errors().len(), 1);
}

/// `SettingsMergerTest.cliCanDisableRouterAndOptimizer` (:177-189). `CProbe C1.runRouter = false`,
/// `C1.runOptimizer = false`.
#[test]
fn cli_can_disable_the_router_and_the_optimizer() {
    let merger = SettingsMerger::new(vec![
        Box::new(DefaultSettings::new(&host())),
        Box::new(cli(&[
            "--router.enabled=false",
            "--router.optimizer.enabled=false",
        ])),
    ]);
    let merged = merger.merge(&host());
    assert!(!merged.get_run_router());
    assert!(!merged.get_run_optimizer());
}

/// `SettingsMergerTest.legacyBatchModeEnablesRouterWhenJsonDisablesIt` (:191-224): `-de` **and**
/// `-do` with no explicit `--router.enabled` force `enabled = true` at priority 60, which beats
/// the priority-10 source's `false`. `CProbe C2.runRouter = true`.
#[test]
fn de_plus_do_forces_the_router_on_over_a_lower_priority_false() {
    let merger = SettingsMerger::new(vec![
        Box::new(DefaultSettings::new(&host())),
        Box::new(json_source_disabling_the_router()),
        Box::new(cli(&["-de", "a.dsn", "-do", "a.ses"])),
    ]);
    assert!(merger.merge(&host()).get_run_router());
    assert_eq!(
        cli(&["-de", "a.dsn", "-do", "a.ses"])
            .get_settings()
            .unwrap()
            .enabled,
        Some(true)
    );
}

/// `CliSettings.java:79` needs **both** flags. `CProbe B11.deOnly.enabled = null`.
#[test]
fn de_alone_does_not_force_the_router_on() {
    assert_eq!(cli(&["-de", "a.dsn"]).get_settings().unwrap().enabled, None);
    assert_eq!(cli(&["-do", "a.ses"]).get_settings().unwrap().enabled, None);
}

/// `SettingsMergerTest.explicitRouterEnabledFalseOverridesLegacyBatchMode` (:226-241).
/// `CProbe C3.runRouter = false`.
#[test]
fn an_explicit_router_enabled_argument_beats_the_batch_mode_forcing() {
    let merger = SettingsMerger::new(vec![
        Box::new(DefaultSettings::new(&host())),
        Box::new(cli(&[
            "-de",
            "a.dsn",
            "-do",
            "a.ses",
            "--router.enabled=false",
        ])),
    ]);
    assert!(!merger.merge(&host()).get_run_router());
}

/// `CliSettings.java:50-52` sets `hasExplicitRouterEnabledArgument` from the *property name*
/// alone, so even a valueless `--router.enabled=` disarms the batch-mode forcing — and then
/// `Boolean.parseBoolean("")` makes it `false` (quirks row 120). `CProbe B8.emptyEnabled.enabled
/// = false`.
#[test]
fn an_empty_router_enabled_argument_still_counts_as_explicit() {
    let cli = cli(&["--router.enabled=", "-de", "a.dsn", "-do", "a.ses"]);
    assert_eq!(cli.get_settings().unwrap().enabled, Some(false));
}

/// `SettingsMergerTest.cliRoutableLayersDoesNotDisableRouter` (:243-256). `CProbe C4.runRouter =
/// true`, `C4.layerCount = 2` — quirk 119 sizes the layer array by the value's token count.
#[test]
fn routable_layers_do_not_disable_the_router() {
    let merger = SettingsMerger::new(vec![
        Box::new(DefaultSettings::new(&host())),
        Box::new(cli(&[
            "-de",
            "a.dsn",
            "--router.layers.routable=false,true",
        ])),
    ]);
    let merged = merger.merge(&host());
    assert!(merged.get_run_router());
    assert_eq!(merged.get_layer_count(), 2);
}

// ---------------------------------------------------------------------------------------------
// The dead LegacyBridge (plan ruling 8)
// ---------------------------------------------------------------------------------------------

/// Plan ruling 8's test rather than comment: the five legacy flags are parsed and normalised
/// exactly as `GlobalSettings.applyCommandLineArguments` does — **and** the `CliSettings` built
/// from the same argv carries none of them, because `mapFlagToProperty` maps only `mp`/`mt`.
///
/// `CProbe D1.oit = 0.05`, `D1.us = GLOBAL_OPTIMAL`, `D1.is = SEQUENTIAL`, `D1.hr = 2:3`,
/// `D1.inc = [GND,  VCC]`.
#[test]
fn legacy_bridge_is_dead() {
    let args = [
        "-oit", "5", "-us", "global", "-is", "seq", "-hr", "2:3", "-inc", "GND, VCC",
    ];
    let dead = bridge(&args);
    assert_eq!(dead.optimization_improvement_threshold, Some(0.05f32));
    assert_eq!(
        dead.board_update_strategy,
        Some(BoardUpdateStrategy::GlobalOptimal)
    );
    assert_eq!(
        dead.item_selection_strategy,
        Some(ItemSelectionStrategy::Sequential)
    );
    assert_eq!(dead.hybrid_ratio.as_deref(), Some("2:3"));
    assert_eq!(
        dead.ignore_net_classes.as_deref(),
        // `-inc` does **not** trim its entries (`docs/cli-legacy-flags.md` quirk 5).
        Some(&["GND".to_string(), " VCC".to_string()][..])
    );

    // The half that makes ruling 8 a test: none of it reaches `RouterSettings`.
    let cli = cli(&args);
    let settings = cli.get_settings().unwrap();
    let optimizer = settings.optimizer.as_ref().unwrap();
    assert_eq!(optimizer.optimization_improvement_threshold, None);
    assert_eq!(optimizer.board_update_strategy, None);
    assert_eq!(optimizer.item_selection_strategy, None);
    assert_eq!(optimizer.hybrid_ratio, None);
    assert_eq!(settings.ignore_net_classes, None);
    assert!(cli.get_parsed_arguments().is_empty());
}

/// `--router.<path>=<value>` reaches the **bridge** as well as `CliSettings`.
///
/// `GlobalSettings.setValue` (`:498-509`) is `ReflectionUtil.setFieldValue(this, name, value)`,
/// and `@SerializedName("router")` on the deprecated bridge (`:52`) is what makes a `router.`
/// path resolve onto it — the field's own javadoc says as much. An earlier revision of
/// `sources/cli.rs` claimed the `--name=value` arm wrote *"never the router bridge"*; the
/// `p8t5` differential's `router-enabled-empty` row measured otherwise against the HEAD jar
/// (`routerSettings.enabled = false`, not `null`, because `Boolean.parseBoolean("") == false`).
///
/// The two parsers still disagree about the same token, which is the point of ruling 8: the
/// bridge's copy is dead and `CliSettings`' is live.
#[test]
fn a_router_long_option_writes_the_dead_bridge_too() {
    // The measured row: an EMPTY value is `false`, not "absent".
    let dead = bridge(&["--router.enabled="]);
    assert_eq!(dead.router_enabled, Some(false));
    assert_eq!(
        cli(&["--router.enabled="]).get_settings().unwrap().enabled,
        Some(false)
    );

    // A path that navigates into `optimizer` lands on the bridge's flattened field.
    let dead = bridge(&["--router.optimizer.max_threads=7"]);
    assert_eq!(dead.optimizer_max_threads, Some(7));
    // …with **no** clamp, unlike `-mt` (`GlobalSettings.java:692-697`).
    let dead = bridge(&["--router.optimizer.max_threads=99999"]);
    assert_eq!(dead.optimizer_max_threads, Some(99999));
    assert_eq!(bridge(&["-mt", "99999"]).optimizer_max_threads, Some(1024));

    // `user_data_path` is excluded from the setter (`:558`), and a non-`router.` name never
    // reaches the bridge at all.
    assert_eq!(bridge(&["--user_data_path=/tmp"]), LegacyBridge::default());
    assert_eq!(
        bridge(&["--api_server.endpoints=http://x"]),
        LegacyBridge::default()
    );
    // A path with no such field is `setValue`'s `NoSuchFieldException` arm: warn and carry on.
    assert_eq!(
        bridge(&["--router.no_such_field=1"]),
        LegacyBridge::default()
    );
    // A `--name` with no `=` is skipped before the split (`:539`).
    assert_eq!(bridge(&["--router.enabled"]), LegacyBridge::default());
}

/// One argv token, two fields, two clamps (plan ruling 8, `docs/cli-legacy-flags.md` quirk 2 /
/// `docs/java-quirks.md` #132).
/// `CProbe D11.mtClampHigh = 1024` (the dead bridge, `GlobalSettings.java:695-697`),
/// `B6.mt2000.maxThreads = 2000` (`RouterSettings.maxThreads`, unclamped at parse time),
/// `C5.mt2000.maxThreads(validated) = 4` (`validate()` caps at the processor count),
/// `B6.mt2000.optimizer.maxThreads = null` and `D11.mtClampHigh.routerMaxThreads = null` — the
/// two paths never write the same field.
#[test]
fn mt_feeds_two_fields_with_different_clamps() {
    let dead = bridge(&["-mt", "2000"]);
    assert_eq!(dead.optimizer_max_threads, Some(1024));

    let live = cli(&["-mt", "2000"]);
    let settings = live.get_settings().unwrap();
    assert_eq!(settings.max_threads, Some(2000));
    assert_eq!(settings.optimizer.as_ref().unwrap().max_threads, None);

    // `validate()` is only reachable through a merge that has a base — `CProbe C5`:
    // `maxThreads` is capped at the processor count, and `optimizer.maxThreads` keeps
    // `DefaultSettings`' `max(1, 4 - 1)` because `-mt` never wrote it.
    let merged = SettingsMerger::new(vec![
        Box::new(DefaultSettings::new(&host())),
        Box::new(cli(&["-mt", "2000"])),
    ])
    .merge(&host());
    assert_eq!(merged.max_threads, Some(4));
    assert_eq!(merged.optimizer.as_ref().unwrap().max_threads, Some(3));
}

/// `GlobalSettings.java:692-697`: `< 0 → 0`, `> 1024 → 1024`. The negative arm is **unreachable
/// from a command line** — a value starting with `-` is never consumed (`:689`) — so it is
/// reached only through `Integer.decode`'s other negative spellings. `CProbe
/// D12.mtNegative(notConsumed) = null`.
#[test]
fn mt_clamps_are_asymmetrically_reachable() {
    assert_eq!(bridge(&["-mt", "-3"]).optimizer_max_threads, None);
    assert_eq!(bridge(&["-mt", "0"]).optimizer_max_threads, Some(0));
    assert_eq!(bridge(&["-mt", "1024"]).optimizer_max_threads, Some(1024));
}

/// `GlobalSettings.java:700-708`: `Float.parseFloat(v) / 100`, **then** `<= 0 → 0.0f`.
/// `CProbe D2b.oitZero = 0.0`, `D2c.oit1 = 0.01` (`floatToIntBits = 1008981770`),
/// `D2d.oit33 = 0.33` (`floatToIntBits = 1051260355`).
///
/// The division is `float`, not `double` — `33.0f32 / 100.0f32` and `(33.0f64 / 100.0) as f32`
/// happen to agree here, but the port keeps Java's arithmetic rather than relying on that.
///
/// **Correction to the brief and to `docs/cli-legacy-flags.md`:** `-oit -5` does *not* give
/// `0.0f`; `-5` starts with `-`, so the value is never consumed and the field stays absent
/// (`CProbe D2.oitNegative = null`). The `<= 0` clamp's negative arm is therefore unreachable
/// from argv; only a literal zero reaches it.
#[test]
fn oit_divides_before_it_clamps() {
    assert_eq!(
        bridge(&["-oit", "0"]).optimization_improvement_threshold,
        Some(0.0f32)
    );
    assert_eq!(
        bridge(&["-oit", "-5"]).optimization_improvement_threshold,
        None
    );

    let one = bridge(&["-oit", "1"])
        .optimization_improvement_threshold
        .unwrap();
    assert_eq!(one.to_bits(), 1_008_981_770);
    let thirty_three = bridge(&["-oit", "33"])
        .optimization_improvement_threshold
        .unwrap();
    assert_eq!(thirty_three.to_bits(), 1_051_260_355);
}

/// `GlobalSettings.java:710-731` — both strategy flags are **total** functions with a fixed
/// fallback, and `-is` matches by prefix. `CProbe D3.*`, `D4.*`.
#[test]
fn us_and_is_never_reject_a_value() {
    assert_eq!(
        bridge(&["-us", "HYBRID"]).board_update_strategy,
        Some(BoardUpdateStrategy::Hybrid)
    );
    assert_eq!(
        bridge(&["-us", " global "]).board_update_strategy,
        Some(BoardUpdateStrategy::GlobalOptimal)
    );
    assert_eq!(
        bridge(&["-us", "nonsense"]).board_update_strategy,
        Some(BoardUpdateStrategy::Greedy)
    );

    assert_eq!(
        bridge(&["-is", "sequestered"]).item_selection_strategy,
        Some(ItemSelectionStrategy::Sequential)
    );
    assert_eq!(
        bridge(&["-is", "RANDOMIZE"]).item_selection_strategy,
        Some(ItemSelectionStrategy::Random)
    );
    assert_eq!(
        bridge(&["-is", "nonsense"]).item_selection_strategy,
        Some(ItemSelectionStrategy::Prioritized)
    );
    assert_eq!(
        bridge(&["-hr", " 2:3 "]).hybrid_ratio.as_deref(),
        Some("2:3")
    );
}

/// `GlobalSettings.java:675-686` uses `Integer.decode`, so `0x10` is 16 and `010` is 8; the
/// `CliSettings` path uses `Integer.parseInt` (`ReflectionUtil.java:136-138`) and rejects both.
/// `CProbe D7.mpDecodeHex = 16`, `D8.mpDecodeOctal = 8`, `B5.mpHex.maxPasses = null`.
#[test]
fn mp_decodes_on_the_bridge_and_parses_on_the_cli_path() {
    assert_eq!(bridge(&["-mp", "0x10"]).max_passes, Some(16));
    assert_eq!(bridge(&["-mp", "010"]).max_passes, Some(8));
    assert_eq!(
        cli(&["-mp", "0x10"]).get_settings().unwrap().max_passes,
        None
    );

    // `< 0 → 1`, `> 9999 → 9999`, `0` kept (`:679-685`).
    assert_eq!(bridge(&["-mp", "100000"]).max_passes, Some(9999));
    assert_eq!(bridge(&["-mp", "0"]).max_passes, Some(0));
    assert_eq!(bridge(&["-mp", "9999"]).max_passes, Some(9999));
}

/// A malformed number throws out of `Integer.decode`, is caught by the loop's own
/// `try`/`catch` (`GlobalSettings.java:835-837`), and — because `i++` never ran (`:686`) — the
/// value token is then processed as an argument of its own. Later flags still apply.
/// `CProbe D18.mpBad.maxPasses = null`, `D18.mpBad.hybridRatio = 9:9`.
#[test]
fn a_malformed_number_skips_the_flag_without_aborting_the_parse() {
    let bad = bridge(&["-mp", "notanumber", "-hr", "9:9"]);
    assert_eq!(bad.max_passes, None);
    assert_eq!(bad.hybrid_ratio.as_deref(), Some("9:9"));
    assert_eq!(bridge(&["-mp"]).max_passes, None);
}

/// `GlobalSettings.java:660-669`: `-drc` sets both flags **before** looking for a value, and is
/// matched before `-dr`. `CProbe D13.drc.routerEnabled = false`, `D13.drc.drcEnabled = true`,
/// `D14.dr.routerEnabled = null`.
#[test]
fn drc_switches_to_drc_only_mode_before_it_looks_for_a_value() {
    let drc = bridge(&["-drc"]);
    assert_eq!(drc.router_enabled, Some(false));
    assert_eq!(drc.drc_enabled, Some(true));

    let with_report = bridge(&["-drc", "report.json"]);
    assert_eq!(with_report.router_enabled, Some(false));
    assert_eq!(with_report.drc_enabled, Some(true));

    // `-dr` must not be swallowed by the `-drc` arm.
    let dr = bridge(&["-dr", "x.rules"]);
    assert_eq!(dr.router_enabled, None);
    assert_eq!(dr.drc_enabled, None);
}

/// Every `GlobalSettings` branch tests `args[i].startsWith("-mp")`, not equality, so `-mpx 5`
/// sets `maxPasses`. `CProbe D10.mpxPrefixMatches = 5` — and `CliSettings`, which matches
/// exactly, gets nothing from the same token (`B4.mpx.parsedArguments = {}`).
#[test]
fn legacy_bridge_matches_flags_by_prefix() {
    assert_eq!(bridge(&["-mpx", "5"]).max_passes, Some(5));
    assert_eq!(
        bridge(&["-incoming", "GND"]).ignore_net_classes.as_deref(),
        Some(&["GND".to_string()][..])
    );
    assert_eq!(cli(&["-mpx", "5"]).get_settings().unwrap().max_passes, None);
}

/// `GlobalSettings.java:810-815`: `split(",")` with no trim, and Java's default limit drops
/// trailing empty pieces. `CProbe D15.incSingle = [GND]`, `D16.incTrailingComma = [GND]`.
#[test]
fn inc_splits_without_trimming() {
    assert_eq!(
        bridge(&["-inc", "GND"]).ignore_net_classes.as_deref(),
        Some(&["GND".to_string()][..])
    );
    assert_eq!(
        bridge(&["-inc", "GND,"]).ignore_net_classes.as_deref(),
        Some(&["GND".to_string()][..])
    );
    assert_eq!(
        bridge(&["-inc", "GND, VCC"]).ignore_net_classes.as_deref(),
        Some(&["GND".to_string(), " VCC".to_string()][..])
    );
}

// ---------------------------------------------------------------------------------------------
// classify_de_arguments (plan ruling 10)
// ---------------------------------------------------------------------------------------------

fn slots(input: Option<&str>, session: Option<&str>, rules: Option<&str>) -> DeSlots {
    DeSlots {
        initial_input_file: input.map(str::to_string),
        design_session_filename: session.map(str::to_string),
        initial_rules_file: rules.map(str::to_string),
    }
}

/// The whole `-de` matrix of `GlobalSettingsCommandLineTest.java` (:25-171), verbatim, plus the
/// `.json`, unknown-extension and prefix rows `CProbe E` adds.
#[test]
fn de_classification_matrix() {
    // singleDsnFile (:25-33)
    assert_eq!(
        de(&["-de", "myboard.dsn"]),
        slots(Some("myboard.dsn"), None, None)
    );
    // dsnAndSesWithPlusSeparator (:35-43)
    assert_eq!(
        de(&["-de", "myboard.dsn+myboard.ses"]),
        slots(Some("myboard.dsn"), Some("myboard.ses"), None)
    );
    // dsnAndRulesWithPlusSeparator (:45-53)
    assert_eq!(
        de(&["-de", "myboard.dsn+myboard.rules"]),
        slots(Some("myboard.dsn"), None, Some("myboard.rules"))
    );
    // allThreeFilesWithPlusSeparator (:55-63)
    assert_eq!(
        de(&["-de", "myboard.dsn+myboard.ses+myboard.rules"]),
        slots(
            Some("myboard.dsn"),
            Some("myboard.ses"),
            Some("myboard.rules")
        )
    );
    // filesInDifferentOrder (:65-73)
    assert_eq!(
        de(&["-de", "myboard.rules+myboard.dsn+myboard.ses"]),
        slots(
            Some("myboard.dsn"),
            Some("myboard.ses"),
            Some("myboard.rules")
        )
    );
    // spaceSeparatedFiles (:75-83) — every following non-`-` argument is consumed
    assert_eq!(
        de(&["-de", "myboard.dsn", "myboard.ses", "myboard.rules"]),
        slots(
            Some("myboard.dsn"),
            Some("myboard.ses"),
            Some("myboard.rules")
        )
    );
    // filenameWithSpaces (:85-93)
    assert_eq!(
        de(&["-de", "sonde xilinx.dsn"]),
        slots(Some("sonde xilinx.dsn"), None, None)
    );
    // mixedSeparators (:95-103)
    assert_eq!(
        de(&["-de", "myboard.dsn+myboard.ses", "myboard.rules"]),
        slots(
            Some("myboard.dsn"),
            Some("myboard.ses"),
            Some("myboard.rules")
        )
    );
    // filesWithPaths (:105-112)
    assert_eq!(
        de(&["-de", "/path/to/myboard.dsn+/path/to/myboard.ses"]),
        slots(
            Some("/path/to/myboard.dsn"),
            Some("/path/to/myboard.ses"),
            None
        )
    );
    // caseInsensitiveExtensions (:114-122) — classified by the lower-cased name, stored verbatim
    assert_eq!(
        de(&["-de", "myboard.DSN+myboard.SES+myboard.RULES"]),
        slots(
            Some("myboard.DSN"),
            Some("myboard.SES"),
            Some("myboard.RULES")
        )
    );
    assert_eq!(
        de(&["-de", "myboard.Dsn+myboard.Ses"]),
        slots(Some("myboard.Dsn"), Some("myboard.Ses"), None)
    );
    // multipleDsnFilesUsesLast (:124-131)
    assert_eq!(
        de(&["-de", "board1.dsn+board2.dsn"]),
        slots(Some("board2.dsn"), None, None)
    );
    assert_eq!(
        de(&["-de", "a.ses+b.ses"]),
        slots(None, Some("b.ses"), None)
    );
    assert_eq!(
        de(&["-de", "a.rules+b.rules"]),
        slots(None, None, Some("b.rules"))
    );
    // onlySesFile (:133-140) / onlyRulesFile (:142-150)
    assert_eq!(
        de(&["-de", "myboard.ses"]),
        slots(None, Some("myboard.ses"), None)
    );
    assert_eq!(
        de(&["-de", "myboard.rules"]),
        slots(None, None, Some("myboard.rules"))
    );
    // emptyArgument (:152-160)
    assert_eq!(de(&["-de"]), slots(None, None, None));
    // withOtherArguments (:162-171) — the run stops at the next `-` argument
    assert_eq!(
        de(&[
            "-de",
            "myboard.dsn+myboard.ses",
            "-do",
            "output.ses",
            "-mp",
            "10"
        ]),
        slots(Some("myboard.dsn"), Some("myboard.ses"), None)
    );
}

/// `GlobalSettings.java:570,579,595` trims each entry, so spaces around a `+`-separated list
/// disappear. `CProbe E [-de,  a.dsn + b.ses ] -> in=a.dsn ses=b.ses`.
#[test]
fn de_entries_are_trimmed() {
    assert_eq!(
        de(&["-de", " a.dsn + b.ses "]),
        slots(Some("a.dsn"), Some("b.ses"), None)
    );
    assert_eq!(de(&["-de", "+"]), slots(None, None, None));
    assert_eq!(de(&["-de", "a.dsn+"]), slots(Some("a.dsn"), None, None));
}

/// `GlobalSettings.java:638-644` — an unrecognised extension is warned about and **dropped**; it
/// does not fall back to the design-input slot. `CProbe E [-de, a.txt] -> in=null`,
/// `E [-de, a.txt+b.dsn] -> in=b.dsn`.
#[test]
fn de_drops_an_unknown_extension_rather_than_guessing() {
    assert_eq!(de(&["-de", "a.txt"]), slots(None, None, None));
    assert_eq!(
        de(&["-de", "a.txt+b.dsn"]),
        slots(Some("b.dsn"), None, None)
    );
}

/// `GlobalSettings.java:609-621`: `.json` fills the **design-input** slot while no `.dsn` has
/// been seen, and the session slot afterwards. `CProbe E [-de, a.json] -> in=a.json`,
/// `E [-de, a.dsn+b.json] -> ses=b.json`, `E [-de, a.json+b.dsn] -> in=b.dsn` (the `.dsn` arm
/// overwrites the slot the `.json` claimed, and the session slot stays empty).
#[test]
fn de_json_is_design_input_until_a_dsn_appears() {
    assert_eq!(de(&["-de", "a.json"]), slots(Some("a.json"), None, None));
    assert_eq!(
        de(&["-de", "a.dsn+b.json"]),
        slots(Some("a.dsn"), Some("b.json"), None)
    );
    assert_eq!(
        de(&["-de", "a.json+b.json"]),
        slots(Some("a.json"), Some("b.json"), None)
    );
    assert_eq!(
        de(&["-de", "a.json+b.dsn"]),
        slots(Some("b.dsn"), None, None)
    );
}

/// `GlobalSettings.java:571-572`: an argument that names an **existing** path is taken verbatim,
/// so a real file whose name contains `+` survives; the same name with nothing on disk is split.
/// `CProbe E.realFileWithPlus (equalsPath=true)`, `E.missingFileWithPlus -> in=null`.
#[test]
fn de_takes_an_existing_path_verbatim_even_with_a_plus_in_it() {
    // A `Drop` guard, not a pair of calls at the end: an assertion below panics on failure, and
    // an early return would leave the directory behind in `$TMPDIR` for good.
    struct TempDir(std::path::PathBuf);
    impl Drop for TempDir {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).ok();
        }
    }

    let dir = TempDir(std::env::temp_dir().join(format!(
        "fr-settings-de-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    )));
    std::fs::create_dir_all(&dir.0).expect("temp dir");
    let path = dir.0.join("board+rev2.dsn");
    std::fs::write(&path, b"").expect("temp file");
    let name = path.to_str().expect("utf-8 temp path").to_string();

    assert_eq!(de(&["-de", &name]), slots(Some(&name), None, None));

    // The same string with nothing on disk behind it takes the `equalsPath` branch's `else`
    // (`:574-644`) and is split on `+` into `<tmp>/board` and `rev2.dsn.ghost`. **Neither** half
    // ends in `.dsn`, `.ses`, `.rules` or `.json`, so every slot stays empty — `:638-644` drops
    // an unknown extension rather than guessing (quirk #137).
    let ghost = format!("{name}.ghost");
    assert_eq!(de(&["-de", &ghost]), slots(None, None, None));
}

/// The `-de` branch is reached only for arguments starting with a single `-` (`:538` takes `--`
/// first), and it matches by **prefix**. Slots persist across occurrences, so the last wins.
/// `CProbe E [-dexyz, a.dsn] -> in=a.dsn`, `E [--de, a.dsn] -> in=null`,
/// `E [-de, a.dsn, -de, b.dsn] -> in=b.dsn`.
#[test]
fn de_is_prefix_matched_and_never_double_dashed() {
    assert_eq!(de(&["-dexyz", "a.dsn"]), slots(Some("a.dsn"), None, None));
    assert_eq!(de(&["--de", "a.dsn"]), slots(None, None, None));
    assert_eq!(
        de(&["-de", "a.dsn", "-de", "b.dsn"]),
        slots(Some("b.dsn"), None, None)
    );
    assert_eq!(de(&[]), slots(None, None, None));
}
