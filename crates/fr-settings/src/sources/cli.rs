//! `settings/sources/CliSettings.java` (135): command-line arguments as a settings source,
//! priority 60 — plus the two pieces of `settings/GlobalSettings.applyCommandLineArguments`
//! (`GlobalSettings.java:521-838`) that belong with it: the dead [`LegacyBridge`] (plan ruling 8)
//! and the `-de` file classifier [`classify_de_arguments`] (plan ruling 10).
//!
//! # Two parsers over one command line
//!
//! Headless Java runs **both** of these over the same argv, and they disagree on purpose:
//!
//! | | `CliSettings` | `GlobalSettings.applyCommandLineArguments` |
//! |---|---|---|
//! | flag matching | exact (`switch (flag)`, `:104-109`) | **prefix** (`args[i].startsWith("-mp")`) |
//! | flags understood | `mp`, `mt` only | the whole legacy table |
//! | `-mp` value | `Integer.parseInt` (via `setFieldValue`) | `Integer.decode` |
//! | target | `RouterSettings` — reaches the router | the `@Deprecated` bridge — reaches nothing |
//!
//! So `-mt 2000` writes **two** fields with **two** clamps from one token, `-mp 0x10` sets 16 on
//! the dead bridge and nothing at all on the live one, and `-mpx 5` is `-mp` to one parser and
//! gibberish to the other. All three are pinned in `tests/cli_source.rs`.

use std::collections::BTreeMap;

use crate::field_path::{java_parse_f32, java_split, java_trim};
use crate::{
    BoardUpdateStrategy, ItemSelectionStrategy, MergeError, RouterSettings, SettingsSource,
    SourceKind, merger::priority, set_field_value,
};

// ---------------------------------------------------------------------------------------------
// CliSettings
// ---------------------------------------------------------------------------------------------

/// `settings/sources/CliSettings.java`: the router settings a command line names, priority 60.
///
/// Only two argument shapes exist (`:40-75`):
///
/// - `--name=value` — split at the **first** `=` (`split("=", 2)`, `:46`), so a value may contain
///   more. The setting is applied only when `name` starts with `router.` (`:54-56`), and that
///   prefix is stripped before [`set_field_value`] (`:91-92`). A `--name` with no `=` is skipped
///   entirely (`:45`).
/// - `-flag [value]` — the value is consumed only when the next argument exists **and does not
///   start with `-`** (`:61`); otherwise it is the empty string, which usually then fails to
///   convert. `map_flag_to_property` maps **only** `mp` and `mt` (`:104-109`).
///
/// After the loop, `-de` **and** `-do` with no explicit `--router.enabled=…` force
/// `enabled = true` (`:79-83`). That is a *source-level* forcing at priority 60, so it beats a
/// priority-10 source's `enabled = false` — which is exactly what
/// `SettingsMergerTest.legacyBatchModeEnablesRouterWhenJsonDisablesIt` checks.
///
/// Every failure is warned about and skipped (`:97-99`); nothing here is fatal.
#[derive(Debug, Clone)]
pub struct CliSettings {
    settings: RouterSettings,
    parsed_arguments: BTreeMap<String, String>,
    errors: Vec<MergeError>,
}

impl CliSettings {
    /// `CliSettings.PRIORITY` (`:16`).
    const PRIORITY: i32 = priority::CLI;

    /// `CliSettings(String[])` (`:25-28`) plus `parseArguments` (`:30-86`).
    ///
    /// renamed: CliSettings -> CliSettings::new (Rust has no constructors).
    #[must_use]
    pub fn new(args: &[String]) -> Self {
        let mut this = Self {
            settings: RouterSettings::new(),
            parsed_arguments: BTreeMap::new(),
            errors: Vec::new(),
        };

        let mut has_design_input_argument = false;
        let mut has_design_output_argument = false;
        let mut has_explicit_router_enabled_argument = false;

        let mut i = 0;
        while i < args.len() {
            let arg = args[i].as_str();

            if let Some(body) = arg.strip_prefix("--") {
                // :43-57 — the `--property=value` form. `contains("=")` first (:45), so a bare
                // `--router.max_passes` is skipped rather than treated as an empty value.
                if body.contains('=') {
                    // `split("=", 2)` (:46): everything after the first `=` is the value.
                    let (property_name, value) = body.split_once('=').expect("contains checked");

                    // :50-52 — the *name* alone arms this, whatever the value is, so even
                    // `--router.enabled=` disarms the `-de`/`-do` forcing below.
                    if property_name == "router.enabled" {
                        has_explicit_router_enabled_argument = true;
                    }

                    if property_name.starts_with("router.") {
                        this.apply_router_setting(property_name, value);
                    }
                }
            } else if let Some(flag) = arg.strip_prefix('-') {
                // :58-74 — the `-flag value` form. `!args[i + 1].startsWith("-")` is what makes
                // `-mp -5` a no-op rather than "minus five".
                let value = match args.get(i + 1) {
                    Some(next) if !next.starts_with('-') => {
                        i += 1;
                        next.clone()
                    }
                    _ => String::new(),
                };

                // :63-67
                if flag == "de" {
                    has_design_input_argument = true;
                } else if flag == "do" {
                    has_design_output_argument = true;
                }

                // :70-73 — `mapFlagToProperty` is an exact `switch`, and every mapping it has
                // already starts with `router.`, so the second half of the guard is a tautology
                // Java keeps for the day someone adds a non-router mapping.
                if let Some(property_name) = map_flag_to_property(flag)
                    && property_name.starts_with("router.")
                {
                    this.apply_router_setting(property_name, &value);
                }
            }

            i += 1;
        }

        // :77-83 — legacy batch invocation routes immediately unless told otherwise.
        if has_design_input_argument
            && has_design_output_argument
            && !has_explicit_router_enabled_argument
        {
            this.settings.enabled = Some(true);
        }

        this
    }

    /// `CliSettings.applyRouterSetting` (`:88-100`): strip the `router.` prefix, assign, and
    /// record the argument **only on success** (`:95` runs after `:94`).
    fn apply_router_setting(&mut self, property_name: &str, value: &str) {
        // :91-92
        let field_path = property_name
            .strip_prefix("router.")
            .unwrap_or(property_name);

        match set_field_value(&mut self.settings, field_path, value) {
            Ok(()) => {
                self.parsed_arguments
                    .insert(property_name.to_string(), value.to_string());
            }
            // :97-99 — `FRLogger.warn`, never fatal (plan Global Constraints: no `FRLogger`).
            Err(error) => self.errors.push(error),
        }
    }

    /// `CliSettings.getParsedArguments` (`:132-134`): the property names that actually landed,
    /// with the `router.` prefix still on them.
    ///
    /// renamed: getParsedArguments — Java returns `new HashMap<>(parsedArguments)`, a defensive
    /// copy; a shared borrow gives the same protection in Rust without the allocation.
    #[must_use]
    pub fn get_parsed_arguments(&self) -> &BTreeMap<String, String> {
        &self.parsed_arguments
    }

    /// Every argument that did **not** land, in parse order — this port's stand-in for the
    /// `FRLogger.warn` at `:98`. Added by this port; Java has no accessor for it.
    // added in Plan 4: (no Java counterpart — the failures only reach FRLogger)
    #[must_use]
    pub fn errors(&self) -> &[MergeError] {
        &self.errors
    }
}

impl SettingsSource for CliSettings {
    fn get_settings(&self) -> Option<&RouterSettings> {
        Some(&self.settings)
    }

    fn get_source_name(&self) -> String {
        "CLI Arguments".to_string()
    }

    fn get_priority(&self) -> i32 {
        Self::PRIORITY
    }

    fn kind(&self) -> SourceKind {
        SourceKind::Cli
    }
}

/// `CliSettings.mapFlagToProperty` (`:102-110`) — exactly two mappings, matched exactly.
fn map_flag_to_property(flag: &str) -> Option<&'static str> {
    match flag {
        "mp" => Some("router.max_passes"),
        "mt" => Some("router.max_threads"),
        _ => None,
    }
}

// ---------------------------------------------------------------------------------------------
// The dead legacy bridge (plan ruling 8)
// ---------------------------------------------------------------------------------------------

/// What `GlobalSettings.applyCommandLineArguments` writes into the `@Deprecated public final
/// RouterSettings routerSettings` bridge (`GlobalSettings.java:51-53`) — **and nothing reads**.
///
/// # Why this struct is deliberately dead (plan ruling 8)
///
/// `CliSettings.mapFlagToProperty` maps only `mp` and `mt`, so those are the only two legacy
/// flags that reach a `RouterSettings` the merger ever sees. Everything else on this struct is
/// written by `applyCommandLineArguments` into an object no routing path consults: in headless
/// Java, `-oit`, `-us`, `-is`, `-hr`, `-inc` and `-drc`'s `routerSettings.enabled = false`
/// **have no effect at all**. The port reproduces the parsing and the normalisation exactly, and
/// then — like Java — throws the result away: no `resolve_headless` input reads this struct.
/// `tests/cli_source.rs::legacy_bridge_is_dead` asserts both halves.
///
/// Wiring any of it up would make the port *more capable* than Java, which is a decision for
/// Plan 8 (see `docs/plan-4-handoff.md`), not a bug fix during a parity port.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LegacyBridge {
    /// `-mp` → `routerSettings.maxPasses` (`GlobalSettings.java:675-686`).
    ///
    /// Java bug: applyCommandLineArguments (GlobalSettings.java:677-685) clamps into the dead
    /// bridge while the live value goes through `CliSettings` unclamped, and the two parse the
    /// same token differently — `Integer.decode` here (so `0x10` is 16 and `010` is 8),
    /// `Integer.parseInt` there (so both are rejected and `max_passes` stays absent).
    ///
    /// added in Plan 4: (the plan's Task 7 brief lists the `-mp` normalisation but omits the
    /// field it lands in; without it the clamp could not be pinned)
    pub max_passes: Option<i32>,

    /// `-mt` → `routerSettings.optimizer.maxThreads` (`:688-698`), clamped `< 0 → 0`,
    /// `> 1024 → 1024`.
    ///
    /// Java bug: applyCommandLineArguments (GlobalSettings.java:690) writes `optimizer.maxThreads`
    /// while `CliSettings` writes `RouterSettings.maxThreads` from the *same* argv token, so one
    /// `-mt 2000` produces `1024` here and `2000` there (which `validate()` then caps at the
    /// processor count). Neither value ever reaches the other field. `docs/cli-legacy-flags.md`
    /// quirk 2.
    pub optimizer_max_threads: Option<i32>,

    /// `-oit` → `routerSettings.optimizer.optimizationImprovementThreshold` (`:700-708`):
    /// `Float.parseFloat(v) / 100`, **then** `<= 0 → 0.0f`.
    ///
    /// Java bug: applyCommandLineArguments (GlobalSettings.java:702-707) is dead — the optimizer
    /// keeps `DefaultSettings`' 0.01 whatever `-oit` says. `docs/cli-legacy-flags.md` quirk 3.
    /// The clamp's negative arm is additionally unreachable from argv: a value beginning with `-`
    /// is never consumed (`:701`), so only a literal zero can trip it.
    pub optimization_improvement_threshold: Option<f32>,

    /// `-us` → `routerSettings.optimizer.boardUpdateStrategy` (`:710-720`): `"global"` and
    /// `"hybrid"` after `toLowerCase().trim()`, **anything else** `Greedy`.
    ///
    /// Java bug: applyCommandLineArguments (GlobalSettings.java:713-718) is dead, and is a total
    /// function with no error for an unrecognised word — a typo silently selects a different
    /// strategy. `docs/cli-legacy-flags.md` quirk 4.
    pub board_update_strategy: Option<BoardUpdateStrategy>,

    /// `-is` → `routerSettings.optimizer.itemSelectionStrategy` (`:721-731`): **prefix** match
    /// `seq`/`rand`, anything else `Prioritized`.
    ///
    /// Java bug: applyCommandLineArguments (GlobalSettings.java:725-729) is dead, and matches by
    /// prefix (`indexOf(…) == 0`), so `sequestered` selects `SEQUENTIAL`.
    /// `docs/cli-legacy-flags.md` quirk 4.
    pub item_selection_strategy: Option<ItemSelectionStrategy>,

    /// `-hr` → `routerSettings.optimizer.hybridRatio` (`:732-736`): `trim()` only, kept as the
    /// raw string Java parses much later.
    ///
    /// Java bug: applyCommandLineArguments (GlobalSettings.java:734) is dead — the hybrid ratio
    /// the optimizer uses comes from `DefaultSettings`' `"1:1"`.
    pub hybrid_ratio: Option<String>,

    /// `-inc` → `routerSettings.ignoreNetClasses` (`:810-815`): `split(",")` with **no trim**.
    ///
    /// Java bug: applyCommandLineArguments (GlobalSettings.java:813) is dead, and does not trim
    /// its entries — unlike the otherwise parallel `debug.filter_by_net` handling at `:552-557`
    /// — so `-inc "GND, VCC"` yields `["GND", " VCC"]` and the second can never match a net
    /// class. `docs/cli-legacy-flags.md` quirk 5.
    pub ignore_net_classes: Option<Vec<String>>,

    /// `-drc` → `routerSettings.enabled = false` (`:662`).
    ///
    /// Java bug: applyCommandLineArguments (GlobalSettings.java:662) is dead — DRC-only mode does
    /// not disable the router through this field; the headless driver decides that elsewhere.
    pub router_enabled: Option<bool>,

    /// `-drc` → `drcSettings.enabled = true` (`:663`), set **before** the report path is looked
    /// for, so a bare `-drc` still switches mode. Unlike the fields above this one is *not* dead
    /// in Java — `DesignRulesCheckerSettings` is read by the DRC driver — but Plan 5 owns
    /// `fr-drc`, so here it is only recorded.
    pub drc_enabled: Option<bool>,
}

/// `GlobalSettings.applyCommandLineArguments` (`GlobalSettings.java:521-838`), reduced to the
/// fields plan ruling 8 keeps: the legacy flag table's effect on the dead bridge.
///
/// renamed: applyCommandLineArguments -> apply_command_line_arguments — Java mutates a
/// `GlobalSettings` in place; the port returns the only part of that object this crate models.
/// The branches that write `guiSettings`, `logging`, `currentLocale`, `runtimeEnvironment`,
/// `debugSettings`, `apiServerSettings`, `showHelpOption`, `compareFile1/2`, `drcReportFile` and
/// the `-de`/`-do`/`-di`/`-dr` filename slots are out of scope for `fr-settings` (Task 11 writes
/// their roster; [`classify_de_arguments`] is the one exception, per plan ruling 10). They are
/// still walked here, in Java's own order, because the **order** is load-bearing: `-drc` must be
/// matched before `-dr` (`:660` before `:670`), and each of them consumes a following value that
/// would otherwise be re-read as an argument of its own.
///
/// Every branch tests `args[i].startsWith("-xx")`, not equality, so `-mpx 5` is `-mp`. The whole
/// loop body sits inside `try`/`catch` (`:523`, `:835-837`), so a malformed number logs and the
/// flag is skipped — and because `i++` runs *after* the assignment that threw, the value token is
/// then processed as an argument of its own.
#[must_use]
pub fn apply_command_line_arguments(args: &[String]) -> LegacyBridge {
    let mut bridge = LegacyBridge::default();

    // `args.length > i + 1 && !args[i + 1].startsWith("-")` — the blanket rule every valued flag
    // repeats (`docs/cli-legacy-flags.md`).
    let value_of = |i: usize| -> Option<&str> {
        match args.get(i + 1) {
            Some(next) if !next.starts_with('-') => Some(next.as_str()),
            _ => None,
        }
    };

    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();

        // :524-529 — `-help`/`--help`/`-h`, case-insensitively, before everything else.
        if arg.eq_ignore_ascii_case("-help")
            || arg.eq_ignore_ascii_case("--help")
            || arg.eq_ignore_ascii_case("-h")
        {
            i += 1;
            continue;
        }

        // :530-537 `--compare-boards=`, :538-563 the `--name=value` setter (both out of scope —
        // they write `GlobalSettings`' own non-router fields, never the router bridge).
        if arg.starts_with("--") {
            i += 1;
            continue;
        }

        if arg.starts_with("-de") {
            // :564-648 — see `classify_de_arguments`. Only the *consumption* matters here: the
            // run of following non-`-` arguments belongs to `-de`.
            if value_of(i).is_some() {
                let mut j = i + 1;
                while j < args.len() && !args[j].starts_with('-') {
                    j += 1;
                }
                i = j;
                continue;
            }
        } else if arg.starts_with("-di") || arg.starts_with("-do") {
            // :649-659 — `guiSettings.inputDirectory` / `initialOutputFile`, out of scope.
            if value_of(i).is_some() {
                i += 1;
            }
        } else if arg.starts_with("-drc") {
            // :660-669 — both flags are set *before* the report path is looked for, so a bare
            // `-drc` still switches mode. Matched before `-dr` on purpose.
            bridge.router_enabled = Some(false);
            bridge.drc_enabled = Some(true);
            if value_of(i).is_some() {
                i += 1;
            }
        } else if arg.starts_with("-dr") {
            // :670-674 — `initialRulesFile`, out of scope (Task 8 reads it from the CLI crate).
            if value_of(i).is_some() {
                i += 1;
            }
        } else if arg.starts_with("-mp") {
            // :675-687
            // A `NumberFormatException` skips the `i++` (:686), leaving the value token to be
            // re-read as an argument of its own.
            if let Some(value) = value_of(i)
                && let Some(decoded) = java_integer_decode(value)
            {
                // `< 0 → 1`, `> 9999 → 9999` (:679-684). `0` is deliberately allowed and means
                // "no limit" (:685) — it is neither `< 0` nor `> 9999`, so it survives.
                bridge.max_passes = Some(if decoded < 0 { 1 } else { decoded.min(9999) });
                i += 1;
            }
        } else if arg.starts_with("-mt") {
            // :688-699
            if let Some(value) = value_of(i)
                && let Some(decoded) = java_integer_decode(value)
            {
                // `< 0 → 0`, `> 1024 → 1024` (:692-697).
                bridge.optimizer_max_threads = Some(decoded.clamp(0, 1024));
                i += 1;
            }
        } else if arg.starts_with("-oit") {
            // :700-709 — the division is `float`, and it happens *before* the clamp.
            //
            // The path string handed to `java_parse_f32` never surfaces: the `Err` it decorates
            // is dropped by the `let Ok(…)` below, matching Java, where `Float.parseFloat`'s
            // `NumberFormatException` is caught by the per-iteration `try` at `:523`/`:835-837`
            // and only reaches `FRLogger`. It is spelled as the field the value would have
            // reached so that a future caller that *does* read the error gets a usable message.
            if let Some(value) = value_of(i)
                && let Ok(percent) =
                    java_parse_f32(value, "optimizer.optimization_improvement_threshold")
            {
                let threshold = percent / 100.0f32;
                bridge.optimization_improvement_threshold =
                    Some(if threshold <= 0.0 { 0.0f32 } else { threshold });
                i += 1;
            }
        } else if arg.starts_with("-us") {
            // :710-720
            if let Some(value) = value_of(i) {
                let op = java_trim(&value.to_lowercase()).to_string();
                bridge.board_update_strategy = Some(match op.as_str() {
                    "global" => BoardUpdateStrategy::GlobalOptimal,
                    "hybrid" => BoardUpdateStrategy::Hybrid,
                    _ => BoardUpdateStrategy::Greedy,
                });
                i += 1;
            }
        } else if arg.starts_with("-is") {
            // :721-731 — `indexOf(…) == 0` is a prefix test, not equality.
            if let Some(value) = value_of(i) {
                let op = java_trim(&value.to_lowercase()).to_string();
                bridge.item_selection_strategy = Some(if op.starts_with("seq") {
                    ItemSelectionStrategy::Sequential
                } else if op.starts_with("rand") {
                    ItemSelectionStrategy::Random
                } else {
                    ItemSelectionStrategy::Prioritized
                });
                i += 1;
            }
        } else if arg.starts_with("-hr") {
            // :732-736
            if let Some(value) = value_of(i) {
                bridge.hybrid_ratio = Some(java_trim(value).to_string());
                i += 1;
            }
        } else if arg == "-l" {
            // :737-798 — `currentLocale`, out of scope. Note this branch is the one equality test
            // in the table, which is why `-ll` reaches its own arm below.
            if value_of(i).is_some() {
                i += 1;
            }
        } else if arg.starts_with("-dl") || arg.starts_with("-da") || arg.starts_with("-help") {
            // :799-802, :808-809 — valueless switches, out of scope.
        } else if arg.starts_with("-host") {
            // :803-807 — `runtimeEnvironment.host`, out of scope.
            if value_of(i).is_some() {
                i += 1;
            }
        } else if arg.starts_with("-inc") {
            // :810-815 — `split(",")`, entries **not** trimmed.
            if let Some(value) = value_of(i) {
                bridge.ignore_net_classes = Some(
                    java_split(value, |c| c == ',')
                        .into_iter()
                        .map(str::to_string)
                        .collect(),
                );
                i += 1;
            }
        } else if arg.starts_with("-dct") || arg.starts_with("-ll") {
            // :816-831 — `guiSettings.dialogConfirmationTimeout` / `logging.console.level`, out
            // of scope.
            if value_of(i).is_some() {
                i += 1;
            }
        }
        // :832-834 — anything else is an unknown argument, warned about and dropped.

        i += 1;
    }

    bridge
}

/// `Integer.decode(String)`: an optional sign, then `0x`/`0X`/`#` for hex, a leading `0` for
/// octal, else decimal. **Not** `Integer.parseInt`, which is what the `CliSettings` path uses —
/// `0x10` is 16 here and a parse failure there.
///
/// `None` is Java's `NumberFormatException`, which `applyCommandLineArguments`' outer
/// `try`/`catch` (`GlobalSettings.java:835-837`) swallows.
///
/// Java's `Integer.parseInt(s, radix)` accepts any Unicode decimal digit (`Character.digit`)
/// where Rust's `from_str_radix` is ASCII-only — the same recorded divergence as quirks row 122,
/// and an error here where Java produced a number, never a different number.
fn java_integer_decode(nm: &str) -> Option<i32> {
    if nm.is_empty() {
        return None;
    }

    let bytes = nm.as_bytes();
    let mut index = 0usize;
    let mut negative = false;
    if bytes[0] == b'-' {
        negative = true;
        index = 1;
    } else if bytes[0] == b'+' {
        index = 1;
    }

    let rest = nm.get(index..)?;
    let radix = if rest.starts_with("0x") || rest.starts_with("0X") {
        index += 2;
        16
    } else if rest.starts_with('#') {
        index += 1;
        16
    } else if rest.starts_with('0') && nm.len() > index + 1 {
        index += 1;
        8
    } else {
        10
    };

    let digits = nm.get(index..)?;
    // "Sign character in wrong position".
    if digits.starts_with('-') || digits.starts_with('+') {
        return None;
    }

    match i32::from_str_radix(digits, radix) {
        Ok(value) => Some(if negative { -value } else { value }),
        // Java retries with the sign attached so that `Integer.MIN_VALUE` decodes (`decode`'s
        // own `catch (NumberFormatException)` arm).
        Err(_) if negative => i32::from_str_radix(&format!("-{digits}"), radix).ok(),
        Err(_) => None,
    }
}

// ---------------------------------------------------------------------------------------------
// -de file classification (plan ruling 10)
// ---------------------------------------------------------------------------------------------

/// The three filename slots `-de` fills (`GlobalSettings.java:607,619,628,636`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeSlots {
    /// `GlobalSettings.initialInputFile` — the design to route (`.dsn`, or a `.json` seen before
    /// any `.dsn`).
    pub initial_input_file: Option<String>,
    /// `GlobalSettings.designSessionFilename` — a previous session (`.ses`, or a `.json` seen
    /// after a `.dsn`).
    pub design_session_filename: Option<String>,
    /// `GlobalSettings.initialRulesFile` — a `.rules` file.
    pub initial_rules_file: Option<String>,
}

/// `GlobalSettings.applyCommandLineArguments`' `-de` arm (`GlobalSettings.java:564-648`) as a
/// pure function — plan ruling 10.
///
/// `crates/freerouting/src/legacy.rs` reproduces the *shape* of this rule today and Plan 8 will
/// call this function instead, so the rule is written down and tested exactly once. The plan does
/// not touch the binary, so there is no caller in this repository yet.
///
/// The rule, in Java's order:
///
/// 1. Only an argument that starts with `-de` and is not `--…` reaches the arm (`:538` takes
///    `--` first), and it matches by **prefix** — `-dexyz` is `-de`.
/// 2. It consumes **every** following argument that does not start with `-` (`:569`), not just
///    one, and does nothing at all when there is none (`:566`).
/// 3. Each argument is `trim()`ed. If it names an **existing** path it is taken verbatim
///    (`:571-572`), so a real file whose name contains `+` survives; otherwise, if it contains
///    `+`, it is split there with empty parts dropped (`:573-581`).
/// 4. Each file is classified by its **lower-cased** extension (`:600-644`) but stored verbatim:
///    `.dsn` → input, `.ses` → session, `.rules` → rules, `.json` → input while no `.dsn` has
///    been seen and session afterwards (`:609-621`). Refilling a slot keeps the **last**.
/// 5. Any other extension is warned about and **dropped** (`:638-644`) — it does *not* fall back
///    to the design-input slot.
///
/// The `hasDsn`/`hasSes`/`hasRules` flags are declared **inside** the arm (`:589-591`), so they
/// reset per `-de` occurrence while the slots themselves persist: `-de a.dsn -de b.dsn` keeps
/// `b.dsn` without the "only the last one" warning.
///
/// added in Plan 8: legacy.rs call site (`crates/freerouting/src/legacy.rs` still reproduces the
/// rule itself; plan ruling 10 keeps the binary untouched)
#[must_use]
pub fn classify_de_arguments(args: &[String]) -> DeSlots {
    let mut slots = DeSlots::default();

    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        // :538 — `--anything` never reaches the `-de` arm.
        if arg.starts_with("--") || !arg.starts_with("-de") {
            i += 1;
            continue;
        }

        // :566 — nothing to do without at least one non-`-` argument after the flag.
        if args.get(i + 1).is_none_or(|next| next.starts_with('-')) {
            i += 1;
            continue;
        }

        // :567-586 — collect the run.
        let mut files: Vec<String> = Vec::new();
        let mut j = i + 1;
        while j < args.len() && !args[j].starts_with('-') {
            let raw_arg = java_trim(&args[j]).to_string();
            if std::path::Path::new(&raw_arg).exists() {
                files.push(raw_arg);
            } else if raw_arg.contains('+') {
                for part in java_split(&raw_arg, |c| c == '+') {
                    let part = java_trim(part);
                    if !part.is_empty() {
                        files.push(part.to_string());
                    }
                }
            } else {
                files.push(raw_arg);
            }
            j += 1;
        }

        // :589-644 — classify.
        // Java's `hasSes`/`hasRules` gate only the "only the last one will be used" log lines
        // (:614, :623, :631) this port drops; `hasDsn` is the one flag with an effect, on the
        // `.json` arm.
        let mut has_dsn = false;
        for file in files {
            let file = java_trim(&file).to_string();
            if file.is_empty() {
                continue;
            }
            let lower = file.to_lowercase();
            if lower.ends_with(".dsn") {
                slots.initial_input_file = Some(file);
                has_dsn = true;
            } else if lower.ends_with(".json") {
                // :609-621 — a `.json` is the design when nothing has claimed that slot yet, and
                // a session afterwards. Note the `.dsn` arm above overwrites the slot a `.json`
                // claimed, and does *not* move the `.json` to the session slot.
                if has_dsn {
                    slots.design_session_filename = Some(file);
                } else {
                    slots.initial_input_file = Some(file);
                    has_dsn = true;
                }
            } else if lower.ends_with(".ses") {
                slots.design_session_filename = Some(file);
            } else if lower.ends_with(".rules") {
                slots.initial_rules_file = Some(file);
            }
            // :638-644 — any other extension is dropped, not guessed at.
        }

        // :647 — skip the arguments the run consumed.
        i = j;
    }

    slots
}

// -------------------------------------------------------------------------------------------
// The `GlobalSettings` roster (plan Task 11)
// -------------------------------------------------------------------------------------------
//
// `settings/GlobalSettings.java` (870 lines) is the whole application's configuration object.
// Exactly one of its nineteen public members is in scope for `fr-settings`:
// `applyCommandLineArguments` (`:521-838`), ported above as `apply_command_line_arguments` with
// a `// renamed:` marker. The other eighteen are listed here, one line each with its reason, so
// the audit (`scripts/audit-port.sh settings crates/fr-settings/src '… GlobalSettings.java'
// scripts/audit-map/fr-settings.map`) checks the deferral instead of skipping it, and so nobody
// has to re-derive the reasons. Line numbers are the clone's HEAD (plan ruling 7).
//
// No persistent configuration file and no user-data directory (spec §2). These five are also the
// crate's `HostEnvironment` boundary: they are `static` mutable path state, which the plan's
// Global Constraints forbid outright.
// not ported: GlobalSettings.load (:267-369) — reads `freerouting.json` off disk through Gson.
//   Spec §2 has no persistent config file; the tier it feeds (`JsonFileSettings`, priority 10)
//   is reserved and empty, and `p4t1` asserts that on the JVM rather than assuming it.
// not ported: GlobalSettings.saveAsJson (:383-454) — writes that same file back.
// not ported: GlobalSettings.getConfigurationFilePath (:178-180) — resolves `freerouting.json`
//   under the user-data directory; nothing to resolve without the file.
// not ported: GlobalSettings.getUserDataPath (:154-157) — `static` mutable path state.
// not ported: GlobalSettings.setUserDataPath (:164-169) — the setter for it.
// not ported: GlobalSettings.lockUserDataPath (:149-152) — the latch that freezes it.
//
// Version and locale, neither of which is a router setting.
// not ported: GlobalSettings.getReleaseSafeVersion (:207-211) — the update check's version
//   string (spec §2: no version check, no telemetry).
// not ported: GlobalSettings.getCurrentLocale (:512-514) — returns `currentLocale`, a UI concern.
//
// The non-router half of the property-path machinery. The *path handling* is not dropped: it is
// `ReflectionUtil.setFieldValue`, which Task 3 ports in full as [`crate::field_path`]; what is
// dropped is only the `GlobalSettings`-rooted entry point and its `FRLogger` reporting.
// not ported: GlobalSettings.setValue (:498-509) — `ReflectionUtil.setFieldValue(this, …)` plus
//   two `FRLogger` arms. The same code is `crate::field_path::set_field_value`, rooted at
//   `RouterSettings` instead of `GlobalSettings`; the `Boolean` return becomes `Result`.
// not ported: GlobalSettings.setDefaultValue (:461-471) — `load()` + `setValue` + `saveAsJson`,
//   i.e. the persistent-file path again.
// not ported: GlobalSettings.applyNonRouterEnvironmentVariables (:473-490) — walks
//   `FREEROUTING__*` and *skips* everything starting with `router.` (:483-485), so by
//   construction it sets nothing this crate models. The router half is
//   `EnvironmentVariablesSource` ([`super::env`]).
//
// The six readers of the dead legacy bridge (plan ruling 8). Each returns a field of the
// `@Deprecated public final RouterSettings routerSettings` / `guiSettings` that no routing path
// reads; [`LegacyBridge`] carries the writes, and nothing in `resolve_headless` reads it.
// not ported: GlobalSettings.getDesignDir (:842-844) — `guiSettings.inputDirectory` (GUI).
// not ported: GlobalSettings.getMaxPasses (:847-849) — `routerSettings.maxPasses` off the bridge.
// not ported: GlobalSettings.getNumThreads (:852-854) — `routerSettings.optimizer.maxThreads`.
// not ported: GlobalSettings.getHybridRatio (:857-859) — `routerSettings.optimizer.hybridRatio`.
// not ported: GlobalSettings.getBoardUpdateStrategy (:862-864) — the bridge's update strategy.
// not ported: GlobalSettings.getItemSelectionStrategy (:867-869) — the bridge's selection
//   strategy.
//
// not ported: the GlobalSettings constructor (:139-147) — validates `currentLocale` against
//   `supportedLanguages` (a UI concern) and builds `settingsMergerProtype` from a bare
//   `DefaultSettings`. The prototype merger it builds is `Freerouting.java:1408-1413`'s, which
//   [`crate::resolve::resolve_headless`] linearises; `audit-port.sh` skips constructors, so this
//   line is documentation rather than a gate.
