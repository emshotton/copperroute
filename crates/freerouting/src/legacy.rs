//! Rewrites a Java-freerouting command line (`-de in.dsn -do out.ses -mp 100 …`) into the port's
//! subcommand form (`route in.dsn -o out.ses`), **bug for bug** (plan ruling AR).
//!
//! # What this module is, after Plan 8 Task 5
//!
//! It is a **mode and slot resolver**, and nothing else. It answers three questions from an argv:
//! which subcommand the line names, which files fill Java's four filename slots, and which
//! `FRLogger` lines Java would have emitted on the way. It does **not** normalise a single value:
//! Plan 4 ported every per-flag normalisation into
//! [`fr_settings::prelude::apply_command_line_arguments`] (the dead bridge, plan-4 ruling 8) and
//! [`fr_settings::prelude::classify_de_arguments_reporting`] (the `-de` rule, plan-4 ruling 10),
//! and the settings that actually reach the router come from `CliSettings` at priority 60. An
//! earlier revision of this file reproduced the `-de` rule itself — the `Path::exists` probe, the
//! `+` splitting, the extension table — and forwarded `-mp`/`-oit`/`-us`/… as native flags. Both
//! are gone: re-deriving either here would have applied the normalisation twice, or applied it on
//! the legacy path and not on the native one.
//!
//! # Two parsers over one command line (plan ruling 10, as amended by scan ruling R19)
//!
//! Java runs **five** parsers over its argv; this port runs **two**, and they disagree on purpose
//! because Java's do:
//!
//! | | this module + `apply_command_line_arguments` | `fr_settings::CliSettings` |
//! |---|---|---|
//! | Java | `GlobalSettings.applyCommandLineArguments` `:521-838` (P3) | `sources/CliSettings.java:39-79` (P4) |
//! | flag matching | **prefix** — `startsWith("-mp")`, so `-mpx 5` is `-mp` | exact `switch` on `arg.substring(1)`, so `-mpx` is nothing |
//! | what it reaches | the filename slots, the mode, and the **dead** bridge | `RouterSettings` — the router |
//!
//! `-mpx 5` therefore sets `max_passes` on **neither**: the parser that matches it writes only the
//! dead bridge, and the parser that reaches the router does not match it. `-decoy a.dsn`, by
//! contrast, **does** set the design input, because the slot resolver is the prefix-matching one.
//! Both are in the table in `tests/legacy_cli.rs`.
//!
//! Collapsing the two into one parser — which plan ruling 10 originally called for — would have
//! made `-mpx 5` set `max_passes`, which no Java parser does. That is why the raw argv, not the
//! rewritten one, is what the settings ladder must be built from.
//!
//! # The rules, in one place
//!
//! * **Every short flag except `-l` is matched with `startsWith`** (`GlobalSettings.java:521-838`;
//!   `-l` is the single `equals` at `:737`). So `-decoy`/`-diff`/`-drcx` are accepted as
//!   `-de`/`-di`/`-drc`, and `-dlx` is `-dl`.
//! * **A value is consumed only when the next argument exists and does not start with `-`**
//!   (`args.length > i + 1 && !args[i + 1].startsWith("-")`). Otherwise the flag is a **silent
//!   no-op** and `i` is not advanced — so `-mp -5` is not "minus five", it is a dead `-mp`
//!   followed by an unknown argument `-5`, and `-mp -5` is *inexpressible*.
//! * **Nothing here fails.** An unknown flag warns and continues (`:561`, `:833`); a parse
//!   exception is caught and the loop continues (`:835-837`). Every failure on this path maps to
//!   **exit 1**, which is why [`rewrite`] answers a `Vec<String>` and a diagnostics list rather
//!   than a `Result`. `LegacyError` and its four variants are gone.
//! * **`-di` is unsupported** — it names a GUI input *directory*, and this port has no GUI. It
//!   still **consumes its value**, exactly as `:649-654` does, so the rest of the line parses the
//!   way Java parses it; the port then warns that the directory is ignored.
//! * **There is no `-v`** on this path. Java's log-level flag is `-ll` (quirk #260); `-v` is the
//!   port's own and lives on the native form only.
//! * **Ruling 14: `.json` follows Java's slot rule.** A `.json` is the *design input* while no
//!   `.dsn` has been seen and the *session* afterwards (`GlobalSettings.java:609-621`). The port
//!   used to divert it to a `--kicad-json` slot of its own; that slot survives on the **native**
//!   form only. See `docs/cli-legacy-flags.md`.
//! * **A bare `-drc` is not DRC mode** (quirk #263). `:660-663` sets two booleans *before* it
//!   looks for a report path, but `main:1462` enters DRC mode on `drcReportFile != null` alone —
//!   so `-drc` with no path falls through to the CLI branch and dies there.

use fr_settings::prelude::{
    UNKNOWN_COMMAND_LINE_ARGUMENT_PREFIX, classify_de_arguments_reporting,
    legacy_flag_value_is_consumed,
};

// ---------------------------------------------------------------------------------------------
// The exit ladder (plan ruling AR)
// ---------------------------------------------------------------------------------------------

/// The process exit codes, Java's two plus the port's two.
///
/// | code | who produces it | Java |
/// |---|---|---|
/// | `Ok` | a completed run, `--help`, and `-drc` unconditionally (quirk label B) | `Freerouting.java:1495` `System.exit(0)`, `:1397` for help |
/// | `Failure` | `computeCliExitCode` (`:194-201`), **and every failure on the legacy path** | `Freerouting.java:1474` `System.exit(1)` |
/// | `UsageError` | **port only** — clap's own usage error, so the *native* subcommand form only | — |
/// | `NotImplemented` | **port only**, and **reserved**: by the end of Plan 8 Task 12 it must be unreachable | — |
///
/// Ruling AR is why 2 and 3 are native-form-only: a Java command line that Java accepts must
/// never come back with a code Java cannot produce. `rewrite` therefore never fails — it warns —
/// and `main` maps its refusals to `Failure`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ExitCode {
    /// The run succeeded.
    Ok = 0,
    /// The run failed — Java's only failure code.
    Failure = 1,
    /// The **native** subcommand form was misused; clap printed the usage. Port only.
    UsageError = 2,
    /// A subcommand that is not wired up yet. Port only, and **reserved**: by the end of Task 12
    /// no command runner may answer it.
    ///
    /// # The gate, spelled out — because the obvious one is now vacuous
    ///
    /// The plan's checklist reads `grep -rn "EXIT_NOT_IMPLEMENTED" crates/` returns nothing.
    /// That identifier no longer exists: it was `commands::EXIT_NOT_IMPLEMENTED`, an `i32`
    /// constant, and Task 5 replaced the whole `i32` exit surface with this enum (scan ruling R13
    /// asked for exactly that rename, so that "the grep is empty" and "code 3 stays reserved and
    /// documented" stop contradicting each other). So the grep passes today for the wrong
    /// reason, and the check a reviewer must actually run is the one that names the **producers**:
    ///
    /// ```sh
    /// grep -rn "ExitCode::NotImplemented" crates/*/src/commands crates/*/src/mcp
    /// ```
    ///
    /// Three sites answer it today — `commands/{route,drc,info}.rs` — and Tasks 6, 7 and 12 are
    /// what remove them. The variant itself stays, unused and documented, per controller answer 3.
    NotImplemented = 3,
}

impl ExitCode {
    /// The number `std::process::exit` wants.
    #[must_use]
    pub fn code(self) -> i32 {
        self as i32
    }
}

// ---------------------------------------------------------------------------------------------
// Diagnostics
// ---------------------------------------------------------------------------------------------

/// The severity of a [`Diagnostic`] — the two `FRLogger` levels this path can reach.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    /// `FRLogger.warn` — the run continues.
    Warn,
    /// `FRLogger.error` — the caller exits 1 straight afterwards.
    Error,
}

/// One `FRLogger` line [`rewrite`] would have emitted, with the Java call site that produced it.
///
/// `rewrite` does not log: it is a pure function, and the caller owns the sink (`crate::logging`).
/// `java_site` keys into [`crate::logging::MESSAGE_MAP`], so a reviewer can check the string
/// against its Java original without opening the jar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// `FRLogger.warn` or `FRLogger.error`.
    pub level: Level,
    /// The message, fully interpolated, exactly as Java builds it.
    pub message: String,
    /// `<file>:<line>` at the clone's HEAD — the Java code this diagnostic corresponds to. A
    /// site that [`crate::logging::MESSAGE_MAP`] does **not** hold means the *message* is the
    /// port's own (there are two: the `-di` and `--compare-boards=` refusals, which Java accepts
    /// silently, and nothing else).
    pub java_site: &'static str,
}

impl Diagnostic {
    fn warn(java_site: &'static str, message: impl Into<String>) -> Self {
        Self {
            level: Level::Warn,
            message: message.into(),
            java_site,
        }
    }

    fn error(java_site: &'static str, message: impl Into<String>) -> Self {
        Self {
            level: Level::Error,
            message: message.into(),
            java_site,
        }
    }
}

/// `Freerouting.java:81-85` — `initializeCli`'s refusal, verbatim.
pub const BOTH_FILES_REQUIRED: &str = "Both an input file and an output file must be specified \
with command line arguments if you are running in CLI mode.";

/// `Freerouting.java:248` — `initializeDrc`'s refusal, verbatim.
pub const DRC_INPUT_REQUIRED: &str =
    "An input file must be specified with -de argument in DRC mode.";

/// `GlobalSettings.java:836`, before the flag name — the per-iteration `catch`'s message.
pub const PARSE_PROBLEM_PREFIX: &str = "There was a problem parsing the '";
/// `GlobalSettings.java:836`, after it.
pub const PARSE_PROBLEM_SUFFIX: &str = "' parameter";

/// Port-only: `-di` names a GUI input directory (`GlobalSettings.java:649-654`) and there is no
/// GUI here. Java has no counterpart line — it accepts the flag silently.
pub const DESIGN_DIRECTORY_IGNORED: &str =
    "The -di option selects a GUI input directory and is ignored: this port is headless.";

/// Port-only: `--compare-boards=` drives `BoardComparator` (`Freerouting.java:820-862`), which is
/// out of scope (spec §2). Java would run it; the port says so rather than pretending.
pub const COMPARE_BOARDS_IGNORED: &str =
    "The --compare-boards= option is not supported by this port and is ignored.";

// ---------------------------------------------------------------------------------------------
// The rewrite
// ---------------------------------------------------------------------------------------------

/// The `GlobalSettings` state `applyCommandLineArguments` leaves behind that is **not** a
/// `RouterSettings` field — the four filename slots, the two mode flags, and one port-only flag.
///
/// `fr_settings::apply_command_line_arguments` is the other half of the same Java method: it
/// carries everything that lands on the (dead) `routerSettings` bridge. Together the two cover
/// `GlobalSettings.java:521-838`, and `scripts/differential/rust/src/bin/p8t5.rs` prints both
/// beside the jar's own answer.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LegacySlots {
    /// `GlobalSettings.initialInputFile` — the design to route (`-de`).
    pub initial_input_file: Option<String>,
    /// `GlobalSettings.initialOutputFile` — where the session is written (`-do`, `:655-659`).
    pub initial_output_file: Option<String>,
    /// `GlobalSettings.initialRulesFile` — `-de …*.rules`, then `-dr` on top (`:670-674`).
    pub initial_rules_file: Option<String>,
    /// `GlobalSettings.designSessionFilename` — a previous session (`-de …*.ses`, or a `.json`
    /// after a `.dsn`).
    pub design_session_filename: Option<String>,
    /// `GlobalSettings.drcReportFile.getFilename()` — set **only** by `-drc <path>` (`:664-668`),
    /// which is what `main:1462` tests for. A bare `-drc` leaves it `None` (quirk #263).
    pub drc_report_file: Option<String>,
    /// `GlobalSettings.showHelpOption` (`:526`, `:809`).
    pub show_help_option: bool,
    /// `Freerouting.java:903-909`'s stdio pre-scan, which is not a `GlobalSettings` field at all —
    /// it is a local in `main`. Kept here because it selects a mode exactly as the others do.
    pub stdio_mode: bool,
    /// Port only: `--version`/`-V`, which Java has no flag for (it prints the banner at `:1120`).
    pub show_version: bool,
}

/// The four subcommands the native form exposes. `rewrite` never emits anything else.
const SUBCOMMANDS: &[&str] = &["route", "drc", "info", "mcp", "help"];

/// Is this the **legacy** command line, or the port's native subcommand form?
///
/// Java has no such switch — it has one command line. The port has two, so it needs a rule that
/// can never be ambiguous, and this is it: **the native form starts with a subcommand name; every
/// other argv is legacy.** An empty argv is legacy, because that is what a Java user who typed
/// `freerouting` and nothing else produced, and Java answers it with `initializeCli`'s refusal
/// and exit 1 rather than a usage screen.
#[must_use]
pub fn is_legacy_form(argv: &[String]) -> bool {
    match argv.first() {
        None => true,
        Some(first) => !SUBCOMMANDS.contains(&first.as_str()),
    }
}

/// Walks `argv` the way `GlobalSettings.applyCommandLineArguments` walks it (`:521-838`) and
/// answers the state it leaves behind, plus every `FRLogger` line it emitted, in Java's order.
///
/// A **native**-form argv is walked too; the caller decides whether to use the answer. `rewrite`
/// does not.
///
/// The order of the arms below is **Java's own**, and it is load-bearing: `-drc` must be tested
/// before `-dr` (`:660` before `:670`), `-de`/`-di`/`-do` before both, and `-l` is the one
/// `equals` in the chain, which is what lets `-ll` reach its own arm at the end.
#[must_use]
pub fn resolve_slots(argv: &[String]) -> (LegacySlots, Vec<Diagnostic>) {
    let mut slots = LegacySlots::default();
    let mut diagnostics: Vec<Diagnostic> = Vec::new();

    // `args.length > i + 1 && !args[i + 1].startsWith("-")` — the blanket rule every valued flag
    // repeats. Returns the value without consuming it; the caller advances `i`.
    let value_of = |i: usize| -> Option<&str> {
        match argv.get(i + 1) {
            Some(next) if !next.starts_with('-') => Some(next.as_str()),
            _ => None,
        }
    };

    let mut i = 0;
    while i < argv.len() {
        let arg = argv[i].as_str();

        // :524-529 — `-help`/`--help`/`-h`, case-insensitively, before everything else.
        if arg.eq_ignore_ascii_case("-help")
            || arg.eq_ignore_ascii_case("--help")
            || arg.eq_ignore_ascii_case("-h")
        {
            slots.show_help_option = true;
            i += 1;
            continue;
        }

        // Port only — Java has no `--version`.
        if arg == "--version" || arg == "-V" {
            slots.show_version = true;
            i += 1;
            continue;
        }

        if let Some(body) = arg.strip_prefix("--") {
            // :530-537 — `--compare-boards=a,b`.
            if body.starts_with("compare-boards=") {
                diagnostics.push(Diagnostic::warn(
                    "GlobalSettings.java:530",
                    COMPARE_BOARDS_IGNORED,
                ));
                i += 1;
                continue;
            }

            // `Freerouting.java:903-909` — the stdio pre-scan. Java only redirects stdout here;
            // starting the MCP server additionally needs `mcp_server.enabled=true`, which
            // `--mcp_server.stdio=true` does **not** set (`McpServerSettings.java:10`). The port
            // treats the flag as the whole request and rewrites it to the `mcp` subcommand —
            // quirk #262, and the reason the `mcp` subcommand is the supported spelling.
            //
            // The flag *also* falls through to the `--name=value` setter below in Java
            // (`:538-563` sets `mcpServerSettings.isStdioMode`), which is silent, so taking it
            // here changes no log line.
            if let Some(value) = body.strip_prefix("mcp_server.stdio=")
                && (value.eq_ignore_ascii_case("true") || value == "1")
            {
                slots.stdio_mode = true;
                i += 1;
                continue;
            }

            // :538-563 — the `--name=value` setter. `--user_data_path` is excluded from both the
            // setter and the warning (`:558`, `:561`), so it is silent either way.
            //
            // not ported: the pre-bootstrap `--user_data_path=` pass (`Freerouting.java:944-956`,
            // first-wins) and the pre-bootstrap logging pass (`:1032-1065`, `-dl` by `equals` and
            // `-ll` first-wins) — spec §2 has no user-data directory and no log file, and plan
            // ruling 10 keeps one parse. Quirk #262.
            match body.split_once('=') {
                Some(("user_data_path", _)) => {}
                // A `--name=value` the port does not model reaches `GlobalSettings.setValue`
                // (`:560`) in Java, which either sets a field this crate has no equivalent of or
                // warns at `:503`. The port cannot tell those apart without a `GlobalSettings`
                // field table, so it stays silent and lets `fr_settings::CliSettings` — which
                // reads the **raw** argv — decide what `--router.*=` means. `p8t5`'s argv table
                // avoids the shapes where the two would differ, and says so.
                Some(_) => {}
                None if body == "user_data_path" => {}
                // :561 — a `--name` with no `=` at all.
                None => diagnostics.push(Diagnostic::warn(
                    "GlobalSettings.java:561",
                    format!("{UNKNOWN_COMMAND_LINE_ARGUMENT_PREFIX}{arg}"),
                )),
            }
            i += 1;
            continue;
        }

        if arg.starts_with("-de") {
            // :564-648. The run of following non-`-` arguments belongs to `-de`; the rule that
            // classifies them is `fr_settings`', called here once per occurrence so that the
            // per-occurrence `hasDsn`/`hasSes`/`hasRules` reset (`:589-591`) and the warnings'
            // position in the log both come out the way Java produces them.
            if value_of(i).is_some() {
                let mut j = i + 1;
                while j < argv.len() && !argv[j].starts_with('-') {
                    j += 1;
                }
                let (de, warnings) = classify_de_arguments_reporting(&argv[i..j]);
                for warning in warnings {
                    diagnostics.push(Diagnostic::warn(de_warning_site(&warning), warning));
                }
                // The slots persist across occurrences (they are `GlobalSettings` fields); only
                // the flags reset. A slot this occurrence did not fill keeps its previous value.
                if de.initial_input_file.is_some() {
                    slots.initial_input_file = de.initial_input_file;
                }
                if de.design_session_filename.is_some() {
                    slots.design_session_filename = de.design_session_filename;
                }
                if de.initial_rules_file.is_some() {
                    slots.initial_rules_file = de.initial_rules_file;
                }
                i = j;
                continue;
            }
        } else if arg.starts_with("-di") {
            // :649-654 — `guiSettings.inputDirectory`. Consumed like Java, then dropped.
            if value_of(i).is_some() {
                diagnostics.push(Diagnostic::warn(
                    "GlobalSettings.java:651",
                    DESIGN_DIRECTORY_IGNORED,
                ));
                i += 1;
            }
        } else if arg.starts_with("-do") {
            // :655-659
            if let Some(value) = value_of(i) {
                slots.initial_output_file = Some(value.to_string());
                i += 1;
            }
        } else if arg.starts_with("-drc") {
            // :660-669 — `routerSettings.enabled = false` and `drcSettings.enabled = true` run
            // *before* the report path is looked for, and both are dead on this path (quirk #131,
            // ruling AQ); `apply_command_line_arguments` is where they are recorded. What decides
            // DRC mode is `drcReportFile`, and only `:664-668` sets it (quirk #263).
            if let Some(value) = value_of(i) {
                slots.drc_report_file = Some(value.to_string());
                i += 1;
            }
        } else if arg.starts_with("-dr") {
            // :670-674 — `initialRulesFile`, overwriting whatever `-de` put there.
            if let Some(value) = value_of(i) {
                slots.initial_rules_file = Some(value.to_string());
                i += 1;
            }
        } else if arg.starts_with("-mp")
            || arg.starts_with("-mt")
            || arg.starts_with("-oit")
            || arg.starts_with("-us")
            || arg.starts_with("-is")
            || arg.starts_with("-hr")
        {
            // :675-736 — the numeric/strategy block. Every one of these writes the **dead**
            // bridge (`fr_settings::apply_command_line_arguments`, plan-4 ruling 8, quirks #131
            // and #143), and `-mp`/`-mt` additionally reach the router through `CliSettings`'
            // exact-`switch` parser over the *raw* argv. Nothing is recorded here: this arm
            // exists only to move the cursor the way Java moves it, because the rest of the line
            // parses differently if it does not.
            //
            // Java bug: applyCommandLineArguments (GlobalSettings.java:675-709) — the `i++` at
            // `:686`, `:698` and `:708` runs *after* an assignment that can throw, so a
            // malformed number logs at `:836` and leaves its own value token to be re-read as an
            // argument and warned about again at `:833`. `legacy_flag_value_is_consumed` is that
            // decision, and it lives in `fr-settings` so this file does not re-derive
            // `Integer.decode`/`Float.parseFloat` (plan-4 ruling 10).
            if let Some(value) = value_of(i) {
                if legacy_flag_value_is_consumed(arg, value) {
                    i += 1;
                } else {
                    diagnostics.push(Diagnostic::error(
                        "GlobalSettings.java:836",
                        format!("{PARSE_PROBLEM_PREFIX}{arg}{PARSE_PROBLEM_SUFFIX}"),
                    ));
                }
            }
        } else if arg == "-l" {
            // :737-798 — `currentLocale`. **Dropped and that is correct**: it drives the GUI's
            // `TextManager` and the localised help text (`main:1394-1396`) and nothing else —
            // there is no `Locale.setDefault` anywhere in the Java tree, so the number formatting
            // in every output file is the JVM's default locale, not this. The port formats as
            // `en_US` unconditionally, and every parity driver runs with
            // `-Duser.language=en -Duser.country=US`.
            //
            // not ported: the locale prefix chain (`GlobalSettings.java:743-797`) — 27 arms onto
            // `java.util.Locale`, a GUI concern (spec §2).
            //
            // This is the **one** `equals` in the whole table, which is why `-ll` below is
            // reachable at all.
            if value_of(i).is_some() {
                i += 1;
            }
        } else if arg.starts_with("-dl") || arg.starts_with("-da") {
            // :799-802 — `logging.file.enabled = false` and
            // `usageAndDiagnosticData.disableAnalytics = true`. Both are valueless switches and
            // both are out of scope (spec §2 writes no log file and uploads no analytics).
            // Matched by **prefix**, so `-dlx` is `-dl`.
            //
            // not ported: the pre-bootstrap `-dl` pass (`Freerouting.java:1055-1056`), which
            // matches by `equals` where this matches by prefix — quirk #262.
        } else if arg.starts_with("-host") {
            // :803-807 — `runtimeEnvironment.host`, a telemetry field (spec §2).
            if value_of(i).is_some() {
                i += 1;
            }
        } else if arg.starts_with("-help") {
            // :808-809 — the second `-help` arm, which only ever fires for a *prefixed* spelling
            // like `-helpme`: the exact three are taken at :524-529 above.
            slots.show_help_option = true;
        } else if arg.starts_with("-inc") || arg.starts_with("-dct") || arg.starts_with("-ll") {
            // :810-815 `routerSettings.ignoreNetClasses` (dead — quirk #131), :816-824
            // `guiSettings.dialogConfirmationTimeout` (GUI, and the one `Integer.parseInt` on
            // this path — so `-dct 0x10` throws where `-mp 0x10` decodes), :825-830
            // `logging.console.level` — which `crate::logging::console_level_string` reads off
            // the same raw argv, with the same prefix match and the same last-wins rule.
            if let Some(value) = value_of(i) {
                if legacy_flag_value_is_consumed(arg, value) {
                    i += 1;
                } else {
                    diagnostics.push(Diagnostic::error(
                        "GlobalSettings.java:836",
                        format!("{PARSE_PROBLEM_PREFIX}{arg}{PARSE_PROBLEM_SUFFIX}"),
                    ));
                }
            }
        } else {
            // :832-834 — anything else, including a bare filename and a value token a failed
            // flag left behind.
            diagnostics.push(Diagnostic::warn(
                "GlobalSettings.java:833",
                format!("{UNKNOWN_COMMAND_LINE_ARGUMENT_PREFIX}{arg}"),
            ));
        }

        i += 1;
    }

    (slots, diagnostics)
}

/// Rewrites a Java-freerouting command line into the port's subcommand form.
///
/// Returns the native argv (**without** the program name) and every `FRLogger` line Java would
/// have emitted while parsing it, in Java's own order. An **empty** argv means the line named no
/// runnable job; the last diagnostic is then the `Error` Java printed before `System.exit(1)`,
/// and the caller exits [`ExitCode::Failure`].
///
/// A native-form argv is returned unchanged, with no diagnostics: clap owns it.
///
/// See the module docs for the parse rules and [`resolve_slots`] for the walk. This function is
/// the **mode ladder** on top of it — `Freerouting.java:1394-1467`, in Java's own order:
/// help exits 0 (`:1394-1398`); `drcReportFile != null` disables every server (`:1401-1405`) and
/// `:1462` picks the DRC branch; the MCP server runs next (`:1435-1445`); and the CLI branch is
/// what is left (`:1456-1465`), refusing without both files (`:80-86`).
#[must_use]
pub fn rewrite(argv: &[String]) -> (Vec<String>, Vec<Diagnostic>) {
    if !is_legacy_form(argv) {
        return (argv.to_vec(), Vec::new());
    }

    let (slots, mut diagnostics) = resolve_slots(argv);

    // :1394-1398 — help is checked first and exits 0.
    if slots.show_help_option {
        return (vec!["--help".to_string()], diagnostics);
    }
    if slots.show_version {
        return (vec!["--version".to_string()], diagnostics);
    }

    // :1401-1405 / :1462 — DRC mode wins over every server.
    if let Some(report) = slots.drc_report_file {
        let Some(input) = slots.initial_input_file else {
            // :247-250
            diagnostics.push(Diagnostic::error(
                "Freerouting.java:248",
                DRC_INPUT_REQUIRED,
            ));
            return (Vec::new(), diagnostics);
        };
        let mut out = vec!["drc".to_string(), input];
        push_option(&mut out, "--ses", slots.design_session_filename);
        push_option(&mut out, "--rules", slots.initial_rules_file);
        out.push("-o".to_string());
        out.push(report);
        return (out, diagnostics);
    }

    // :1435-1445 — the MCP server, once the DRC branch has not claimed the run.
    if slots.stdio_mode {
        return (vec!["mcp".to_string()], diagnostics);
    }

    // :1456-1465 — otherwise the CLI branch, which refuses without both files (`:80-86`).
    let (Some(input), Some(output)) = (slots.initial_input_file, slots.initial_output_file) else {
        diagnostics.push(Diagnostic::error(
            "Freerouting.java:81",
            BOTH_FILES_REQUIRED,
        ));
        return (Vec::new(), diagnostics);
    };
    let mut out = vec!["route".to_string(), input];
    push_option(&mut out, "--ses", slots.design_session_filename);
    push_option(&mut out, "--rules", slots.initial_rules_file);
    out.push("-o".to_string());
    out.push(output);
    (out, diagnostics)
}

fn push_option(out: &mut Vec<String>, flag: &str, value: Option<String>) {
    if let Some(value) = value {
        out.push(flag.to_string());
        out.push(value);
    }
}

/// Which `FRLogger.warn` line inside the `-de` arm a message came from — the five call sites are
/// distinguishable by their text, and the mapping is what keys them into
/// [`crate::logging::MESSAGE_MAP`].
fn de_warning_site(warning: &str) -> &'static str {
    use fr_settings::prelude::{
        MULTIPLE_DSN_FILES, MULTIPLE_RULES_FILES, MULTIPLE_SES_FILES, MULTIPLE_SESSION_FILES,
    };
    if warning == MULTIPLE_DSN_FILES {
        "GlobalSettings.java:602"
    } else if warning == MULTIPLE_SESSION_FILES {
        "GlobalSettings.java:615"
    } else if warning == MULTIPLE_SES_FILES {
        "GlobalSettings.java:623"
    } else if warning == MULTIPLE_RULES_FILES {
        "GlobalSettings.java:631"
    } else {
        "GlobalSettings.java:638"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| (*x).to_string()).collect()
    }

    fn argv_of(v: &[&str]) -> Vec<String> {
        rewrite(&s(v)).0
    }

    fn warnings_of(v: &[&str]) -> Vec<String> {
        rewrite(&s(v)).1.into_iter().map(|d| d.message).collect()
    }

    #[test]
    fn a_native_command_line_passes_through_untouched() {
        for native in [
            s(&["route", "a.dsn", "-o", "b.ses"]),
            s(&["drc", "a.dsn"]),
            s(&["info", "a.dsn"]),
            s(&["mcp"]),
        ] {
            let (out, diagnostics) = rewrite(&native);
            assert_eq!(out, native);
            assert!(diagnostics.is_empty());
        }
    }

    #[test]
    fn de_and_do_become_route() {
        assert_eq!(
            argv_of(&["-de", "a.dsn", "-do", "b.ses"]),
            s(&["route", "a.dsn", "-o", "b.ses"])
        );
    }

    #[test]
    fn every_short_flag_but_l_is_prefix_matched() {
        // `-decoy` is `-de`, so the design input IS set (GlobalSettings.java:564).
        assert_eq!(
            argv_of(&["-decoy", "a.dsn", "-do", "b.ses"]),
            s(&["route", "a.dsn", "-o", "b.ses"])
        );
        // `-drcx` is `-drc`.
        assert_eq!(
            argv_of(&["-de", "a.dsn", "-drcx", "r.json"]),
            s(&["drc", "a.dsn", "-o", "r.json"])
        );
        // `-dox` is `-do`.
        assert_eq!(
            argv_of(&["-de", "a.dsn", "-dox", "b.ses"]),
            s(&["route", "a.dsn", "-o", "b.ses"])
        );
    }

    #[test]
    fn a_missing_value_is_a_silent_no_op_and_i_is_not_advanced() {
        // `-mp -5`: `-5` starts with `-`, so it is never consumed; it then reaches the
        // unknown-argument arm on its own (:833).
        let (out, diagnostics) = rewrite(&s(&["-de", "a.dsn", "-do", "b.ses", "-mp", "-5"]));
        assert_eq!(out, s(&["route", "a.dsn", "-o", "b.ses"]));
        assert_eq!(
            diagnostics
                .iter()
                .map(|d| d.message.as_str())
                .collect::<Vec<_>>(),
            vec!["Unknown command line argument: -5"]
        );
    }

    #[test]
    fn an_unknown_flag_warns_and_continues() {
        let (out, diagnostics) = rewrite(&s(&["-de", "a.dsn", "-zz", "-do", "b.ses"]));
        assert_eq!(out, s(&["route", "a.dsn", "-o", "b.ses"]));
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].level, Level::Warn);
        assert_eq!(diagnostics[0].message, "Unknown command line argument: -zz");
    }

    #[test]
    fn ruling_14_json_takes_javas_slot() {
        // No `.dsn` seen yet -> the design input (GlobalSettings.java:610-612).
        assert_eq!(
            argv_of(&["-de", "board.json", "-do", "b.ses"]),
            s(&["route", "board.json", "-o", "b.ses"])
        );
        // After a `.dsn` -> the session slot (:613-620).
        assert_eq!(
            argv_of(&["-de", "a.dsn", "prev.json", "-do", "b.ses"]),
            s(&["route", "a.dsn", "--ses", "prev.json", "-o", "b.ses"])
        );
    }

    #[test]
    fn a_bare_drc_is_not_drc_mode() {
        // Quirk #263: `:660-663` sets two dead booleans, `main:1462` needs `drcReportFile`.
        let (out, diagnostics) = rewrite(&s(&["-de", "a.dsn", "-drc"]));
        assert!(out.is_empty());
        assert_eq!(
            diagnostics.last().expect("one error").message,
            BOTH_FILES_REQUIRED
        );
    }

    #[test]
    fn drc_without_an_input_is_javas_other_refusal() {
        let (out, diagnostics) = rewrite(&s(&["-drc", "r.json"]));
        assert!(out.is_empty());
        assert_eq!(
            diagnostics.last().expect("one error").message,
            DRC_INPUT_REQUIRED
        );
    }

    #[test]
    fn no_arguments_at_all_is_javas_cli_refusal() {
        let (out, diagnostics) = rewrite(&[]);
        assert!(out.is_empty());
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].level, Level::Error);
        assert_eq!(diagnostics[0].message, BOTH_FILES_REQUIRED);
    }

    #[test]
    fn dr_overwrites_the_rules_slot_de_filled() {
        assert_eq!(
            argv_of(&["-de", "a.dsn", "x.rules", "-do", "b.ses", "-dr", "y.rules"]),
            s(&["route", "a.dsn", "--rules", "y.rules", "-o", "b.ses"])
        );
    }

    #[test]
    fn the_de_warnings_come_out_in_javas_order() {
        assert_eq!(
            warnings_of(&["-zz", "-de", "a.dsn", "b.dsn", "junk.bin", "-do", "o.ses"]),
            vec![
                "Unknown command line argument: -zz".to_string(),
                "Multiple DSN files provided in -de argument. Only the last one will be used."
                    .to_string(),
                "Unknown file type in -de argument: junk.bin. Expected .dsn, .json, .ses, or .rules"
                    .to_string(),
            ]
        );
    }

    #[test]
    fn help_wins_over_everything() {
        assert_eq!(
            argv_of(&["-de", "a.dsn", "-do", "b.ses", "-h"]),
            s(&["--help"])
        );
        assert_eq!(argv_of(&["--help"]), s(&["--help"]));
        assert_eq!(argv_of(&["-HELP"]), s(&["--help"]));
    }

    #[test]
    fn the_mcp_stdio_long_form_becomes_the_mcp_subcommand() {
        assert_eq!(argv_of(&["--mcp_server.stdio=true"]), s(&["mcp"]));
        assert_eq!(argv_of(&["--mcp_server.stdio=1"]), s(&["mcp"]));
        // Anything else is a plain `--name=value` and the run falls through to the CLI branch.
        let (out, _) = rewrite(&s(&["--mcp_server.stdio=no"]));
        assert!(out.is_empty());
    }

    #[test]
    fn di_consumes_its_value_and_warns() {
        let (out, diagnostics) = rewrite(&s(&["-di", "dir", "-de", "a.dsn", "-do", "b.ses"]));
        assert_eq!(out, s(&["route", "a.dsn", "-o", "b.ses"]));
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].message, DESIGN_DIRECTORY_IGNORED);
    }

    #[test]
    fn the_valued_flags_consume_their_values_and_forward_nothing() {
        // Every one of these is `CliSettings`' or the dead bridge's business; none of them may
        // appear in the rewritten argv (plan ruling 10 / scan ruling R19).
        assert_eq!(
            argv_of(&[
                "-de", "a.dsn", "-do", "b.ses", "-mp", "5", "-mt", "2", "-oit", "50", "-us",
                "hybrid", "-is", "random", "-hr", "1:1", "-inc", "GND,VCC", "-l", "de", "-host",
                "x", "-dct", "5", "-ll", "debug", "-dl", "-da",
            ]),
            s(&["route", "a.dsn", "-o", "b.ses"])
        );
    }

    #[test]
    fn a_malformed_number_logs_and_leaves_its_value_to_be_re_read() {
        // GlobalSettings.java:677 throws, :686's `i++` never runs, :836 logs, and `abc` then
        // reaches :833 on its own.
        let (out, diagnostics) = rewrite(&s(&["-de", "a.dsn", "-do", "b.ses", "-mp", "abc"]));
        assert_eq!(out, s(&["route", "a.dsn", "-o", "b.ses"]));
        assert_eq!(
            diagnostics
                .iter()
                .map(|d| (d.level, d.message.as_str()))
                .collect::<Vec<_>>(),
            vec![
                (
                    Level::Error,
                    "There was a problem parsing the '-mp' parameter"
                ),
                (Level::Warn, "Unknown command line argument: abc"),
            ]
        );
        // `-dct` is the one `Integer.parseInt` on this path, so a hex literal throws there and
        // decodes for `-mp`.
        let (_, diagnostics) = rewrite(&s(&["-de", "a.dsn", "-do", "b.ses", "-dct", "0x10"]));
        assert_eq!(diagnostics.len(), 2, "{diagnostics:?}");
        let (_, diagnostics) = rewrite(&s(&["-de", "a.dsn", "-do", "b.ses", "-mp", "0x10"]));
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn exit_codes_are_the_ruling_ar_ladder() {
        assert_eq!(ExitCode::Ok.code(), 0);
        assert_eq!(ExitCode::Failure.code(), 1);
        assert_eq!(ExitCode::UsageError.code(), 2);
        assert_eq!(ExitCode::NotImplemented.code(), 3);
    }
}
