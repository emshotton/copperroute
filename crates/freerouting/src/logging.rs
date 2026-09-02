//! The CLI's log surface: the level ladder Java's `-ll` drives, the sink, and the Java message
//! set the port must emit.
//!
//! **This is not a port of `logger/**`.** `logger/FRLogger.java` (480) and
//! `logger/Log4j2ConfigurationFactory.java` (137) are out of scope — spec §2 drops log files, the
//! in-memory `LogEntries` ring exists for a GUI window this port does not have, and
//! `FRLogger.traceEntry`/`traceExit`'s performance timers are an observer surface Plan 8 replaced
//! with `ProgressSink`. What *is* in scope, and what this module carries, is three facts:
//!
//! 1. **The level ladder** — Java's console level defaults to `INFO`
//!    (`Log4j2ConfigurationFactory.java:102`) and `-ll <level>` is the only flag that moves it
//!    (`GlobalSettings.java:825-830`, `Freerouting.java:1056-1065`). `parseLevel`
//!    (`Log4j2ConfigurationFactory.java:130-135`) upper-cases and falls back to `INFO` for
//!    anything `Level.valueOf` refuses — it never fails.
//! 2. **Everything goes to stderr**, which is a deliberate divergence — see below.
//! 3. **[`MESSAGE_MAP`]** — the Java message set the CLI must emit, keyed by the `FRLogger` call
//!    site so `normalize_log` can map one onto the other. A table, not a translation layer.
//!
//! # Quirk label AI: Java's log stream layout, and why the port has one stream
//!
//! `Log4j2ConfigurationFactory.getConfiguration` builds three appenders (`:54-113`):
//!
//! | appender | target | level |
//! |---|---|---|
//! | `Console` | `ConsoleAppender.Target.SYSTEM_OUT` (`:58`) | the console level (`:102-104`) |
//! | `File` | the resolved log file (`:76-84`) | the file level, default `DEBUG` (`:107-109`) |
//! | `stderr` | `ConsoleAppender.Target.SYSTEM_ERR` (`:88-96`) | `Level.ERROR`, **always** (`:112-113`) |
//!
//! So an `ERROR` is written **three** times (stdout, the file, stderr), every `INFO` and `WARN`
//! pollutes **stdout** — which is what a caller piping a board through the CLI reads — and the
//! `stderr` appender reuses `filePattern` rather than `PATTERN` (`:95`), so
//! `--logging.file.pattern=` silently reformats stderr.
//!
//! **The port sends everything to stderr** (spec §12) and writes no log file. That is a
//! divergence, recorded as `docs/java-quirks.md` #261, and it is the reason `p8t1`'s log
//! normaliser merges Java's two console streams before comparing: after the merge the two
//! programs emit the same message set, in the same order, on one stream each.
//!
//! # The pattern, and why this module does not reproduce it
//!
//! Java's layout is `"%d{yyyy-MM-dd HH:mm:ss.SSS} %-6level %msg%n"`
//! (`Log4j2ConfigurationFactory.java:29`). The timestamp is wall-clock and can never match across
//! two processes, so `normalize_log` strips it on the Java side; the port therefore emits no
//! timestamp at all — `<LEVEL> <message>` — which keeps the port's own output byte-reproducible
//! for `tests/reference/cli-*` instead of merely normalisable.

use std::sync::atomic::{AtomicBool, Ordering};

/// The console level, in Log4j2's own order. `Off` is `Level.OFF`; `All` collapses into
/// [`LogLevel::Trace`] because the port has no level below `TRACE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum LogLevel {
    /// `Level.OFF` — no output at all.
    Off,
    /// `Level.ERROR` and `Level.FATAL`.
    Error,
    /// `Level.WARN`.
    Warn,
    /// `Level.INFO` — Java's console default (`Log4j2ConfigurationFactory.java:102`).
    #[default]
    Info,
    /// `Level.DEBUG`.
    Debug,
    /// `Level.TRACE` and `Level.ALL`.
    Trace,
}

impl LogLevel {
    /// `Log4j2ConfigurationFactory.parseLevel` (`:130-135`): `Level.valueOf(level.toUpperCase())`
    /// with **`INFO` on any failure**. It cannot error, and it never consults the caller.
    ///
    /// `Level.valueOf` knows eight names — `OFF`, `FATAL`, `ERROR`, `WARN`, `INFO`, `DEBUG`,
    /// `TRACE`, `ALL` (log4j-api `Level`); `FATAL` collapses onto [`LogLevel::Error`] and `ALL`
    /// onto [`LogLevel::Trace`], because those are the port's extreme rungs.
    ///
    /// renamed: parseLevel -> LogLevel::parse_java (a free function on the factory in Java; the
    /// port has no factory to hang it on).
    #[must_use]
    pub fn parse_java(level: &str) -> Self {
        match level.to_uppercase().as_str() {
            "OFF" => Self::Off,
            "FATAL" | "ERROR" => Self::Error,
            "WARN" => Self::Warn,
            "DEBUG" => Self::Debug,
            "TRACE" | "ALL" => Self::Trace,
            // `Level.valueOf` throws for anything else, and `:133-134` answers INFO — as it also
            // does for the literal "INFO".
            _ => Self::Info,
        }
    }

    /// The `tracing` filter directive this level means.
    #[must_use]
    pub fn as_filter(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Error => "error",
            Self::Warn => "warn",
            Self::Info => "info",
            Self::Debug => "debug",
            Self::Trace => "trace",
        }
    }
}

/// `GlobalSettings.logging.console.level` after `applyCommandLineArguments` — the **string**, not
/// the level, because that is what Java stores and it does not validate it.
///
/// * The default is `"INFO"` (`LoggingSettings.java:25`).
/// * `-ll <value>` stores `value.toUpperCase()` (`GlobalSettings.java:825-830`), matched by
///   **prefix** (so `-lll` is `-ll`) with the blanket value rule, and the **last** occurrence
///   wins because the loop assigns on every one.
/// * `--logging.console.level=<value>` reaches the same field through
///   `GlobalSettings.setValue` (`:560`, `:498-509`) and stores the value **verbatim** — no
///   upper-casing. The two spellings therefore disagree on case, which nothing downstream
///   notices: `Log4j2ConfigurationFactory.parseLevel` upper-cases again.
///
/// Java's pre-bootstrap pass (`Freerouting.java:1056-1065`) is the one that actually configures
/// log4j, and it takes the **first** `-ll` and matches it with `equals`. Plan ruling 10 keeps one
/// parse and takes `GlobalSettings`' rule; the divergence is quirk `docs/java-quirks.md` #262 and
/// is unobservable in any output file.
#[must_use]
pub fn console_level_string(argv: &[String]) -> String {
    let mut level = "INFO".to_string();

    let mut i = 0;
    while i < argv.len() {
        let arg = argv[i].as_str();
        if let Some(value) = arg.strip_prefix("--logging.console.level=") {
            level = value.to_string();
        } else if !arg.starts_with("--") && arg.starts_with("-ll") {
            match argv.get(i + 1) {
                Some(next) if !next.starts_with('-') => {
                    level = next.to_uppercase();
                    i += 1;
                }
                // No value: Java leaves the field alone and does not advance `i`.
                _ => {}
            }
        }
        i += 1;
    }

    level
}

/// Reads the console level out of a raw command line, **before** anything is logged.
///
/// [`console_level_string`] is Java's half; on top of it this adds two spellings Java does not
/// have, for the native subcommand form: `--log-level <level>` / `--log-level=<level>`, and
/// `-v`/`-vv`. **Java has no `-v`** — quirk #260 — so both are the port's own, and the CLI's help
/// text says so.
///
/// The port-only spellings are applied last, so `-v` raises a level `-ll` chose and never lowers
/// it.
#[must_use]
pub fn level_from_argv(argv: &[String]) -> LogLevel {
    let mut level = LogLevel::parse_java(&console_level_string(argv));
    let mut verbosity = 0u8;

    let mut i = 0;
    while i < argv.len() {
        let arg = argv[i].as_str();
        if arg == "--log-level" {
            if let Some(value) = argv.get(i + 1) {
                level = LogLevel::parse_java(value);
                i += 1;
            }
        } else if let Some(value) = arg.strip_prefix("--log-level=") {
            level = LogLevel::parse_java(value);
        } else if arg == "-v" || arg == "--verbose" {
            verbosity = verbosity.saturating_add(1);
        } else if arg == "-vv" {
            verbosity = verbosity.saturating_add(2);
        }
        i += 1;
    }

    match verbosity {
        0 => level,
        1 => level.max(LogLevel::Debug),
        _ => LogLevel::Trace,
    }
}

/// Guards against a second `init` in the same process — `tracing`'s global subscriber can only be
/// installed once, and the integration tests drive [`init`] through the binary rather than in
/// process, so a double call would be a bug rather than a race.
static INITIALISED: AtomicBool = AtomicBool::new(false);

/// Installs the process-wide log sink: **stderr**, no timestamp, no ANSI, no target — see the
/// module docs for both divergences.
///
/// Idempotent: a second call is a no-op, so a caller that re-reads its command line cannot
/// panic the process on `SubscriberBuilder::init`'s "a global default trace dispatcher has
/// already been set".
pub fn init(level: LogLevel) {
    if INITIALISED.swap(true, Ordering::SeqCst) {
        return;
    }
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new(level.as_filter()))
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .with_target(false)
        .without_time()
        .init();
}

/// The Java message set the CLI must emit, keyed by the `FRLogger` call site.
///
/// **A table, not a translation layer.** Nothing dispatches on it at run time: it exists so that
/// `normalize_log` (Task 6) can map a Java line onto the port's, so that a reviewer can check a
/// port message against its Java original without opening the jar, and so that a message that
/// drifts is a diff in one place rather than an unnoticed divergence in twelve.
///
/// The key is `<file>:<line>` at the clone's HEAD (plan ruling 7). The value is the message with
/// every interpolated value replaced by `{}`, in the order Java concatenates them — Java builds
/// these with `+`, so the placeholder marks where the runtime value lands, not a format spec.
///
/// Only the **CLI** path is listed. The API server, the MCP HTTP transport, the analytics upload,
/// the board comparator and the GUI are out of scope (spec §2), and their `FRLogger` lines with
/// them.
pub const MESSAGE_MAP: &[(&str, &str)] = &[
    // ── settings/GlobalSettings.applyCommandLineArguments (:521-838) ────────────────────────────
    (
        "GlobalSettings.java:562",
        "Unknown command line argument: {}",
    ),
    (
        "GlobalSettings.java:602",
        "Multiple DSN files provided in -de argument. Only the last one will be used.",
    ),
    (
        "GlobalSettings.java:615",
        "Multiple session files (SES/JSON) provided in -de argument. Only the last one will be used.",
    ),
    (
        "GlobalSettings.java:623",
        "Multiple SES files provided in -de argument. Only the last one will be used.",
    ),
    (
        "GlobalSettings.java:631",
        "Multiple RULES files provided in -de argument. Only the last one will be used.",
    ),
    (
        "GlobalSettings.java:638",
        "Unknown file type in -de argument: {}. Expected .dsn, .json, .ses, or .rules",
    ),
    (
        "GlobalSettings.java:833",
        "Unknown command line argument: {}",
    ),
    (
        "GlobalSettings.java:836",
        "There was a problem parsing the '{}' parameter",
    ),
    // `GlobalSettings.setValue` (:498-509), reached from the `--name=value` arm at :560.
    ("GlobalSettings.java:503", "Unknown settings property: {}"),
    (
        "GlobalSettings.java:506",
        "Failed to set property value for: {}",
    ),
    // ── Freerouting.initializeCli (:79-186) ─────────────────────────────────────────────────────
    (
        "Freerouting.java:81",
        "Both an input file and an output file must be specified with command line arguments if you are running in CLI mode.",
    ),
    ("Freerouting.java:105", "Couldn't load the input file '{}'"),
    (
        "Freerouting.java:109",
        "Couldn't read the input file '{}', aborting.",
    ),
    ("Freerouting.java:119", "Couldn't delete the file '{}'"),
    ("Freerouting.java:138", "Couldn't load rules file '{}': {}"),
    ("Freerouting.java:210", "Couldn't save the output file '{}'"),
    (
        "Freerouting.java:238",
        "Couldn't write routing result manifest to '{}'",
    ),
    // ── Freerouting.initializeDrc (:246-375) — Task 7 emits these ───────────────────────────────
    (
        "Freerouting.java:248",
        "An input file must be specified with -de argument in DRC mode.",
    ),
    ("Freerouting.java:263", "Loading DSN file for DRC: {}"),
    // `:266`'s `"Couldn't load the input file '" + … + "'"` is **byte-identical** to
    // `initializeCli`'s `:105` and is deliberately not listed twice: `normalize_log` resolves a
    // line to the *first* template that matches, so a second row would be unreachable and a
    // reader would be told the two messages are distinguishable when they are not. `p8t3 e2e`'s
    // `missing-input` row therefore reports `ERROR Freerouting.java:105` for a `-drc` run — on
    // **both** sides, which is what makes it a comparison rather than a coincidence.
    ("Freerouting.java:272", "Failed to load board for DRC check"),
    ("Freerouting.java:281", "Loading RULES file for DRC: {}"),
    (
        "Freerouting.java:286",
        "RULES file loaded for DRC successfully",
    ),
    ("Freerouting.java:289", "RULES file for DRC not found: {}"),
    ("Freerouting.java:292", "Failed to load RULES file for DRC"),
    // Task 7 added the three `:303`-`:312` rows, so `initializeDrc`'s message set is complete
    // rather than sampled: without them `p8t3`'s log rung compared 3 lines on
    // `drc-issue593-ses` where the jar emitted 4, and a message the port stopped emitting would
    // have been invisible. `:306` was listed then even though the port could not yet emit it;
    // **Plan 8 Task 10 discharged that stub**, so `commands::drc::load_session_file` now takes
    // the `.json` arm through `fr_dsn::kicad::import_session` and emits `:306` on success and
    // `:327` on failure, exactly as the jar does.
    (
        "Freerouting.java:303",
        "Loading KiCad JSON session file for DRC: {}",
    ),
    (
        "Freerouting.java:306",
        "KiCad JSON session file loaded for DRC successfully",
    ),
    ("Freerouting.java:309", "Loading SES file for DRC: {}"),
    (
        "Freerouting.java:312",
        "SES file loaded for DRC: {} wires, {} vias imported{}",
    ),
    ("Freerouting.java:324", "Session file for DRC not found: {}"),
    (
        "Freerouting.java:327",
        "Failed to load session file for DRC",
    ),
    (
        "Freerouting.java:351",
        "Failed to calculate quality score for DRC report: {}",
    ),
    ("Freerouting.java:363", "DRC report written to: {}"),
    (
        "Freerouting.java:365",
        "Couldn't save the DRC report to '{}'",
    ),
    // ── Freerouting.main (:899-1495) ────────────────────────────────────────────────────────────
    ("Freerouting.java:1120", "Freerouting {}"),
    ("Freerouting.java:1450", "Couldn't initialize the GUI"),
];

// =================================================================================================
// The `logger/**` roster (plan §The audit; `scripts/audit-map/freerouting.map` sends all seven
// classes here)
// =================================================================================================
//
// Spec §2 drops `FRLogger` and the log files entirely, and there is no GUI to show the in-memory
// `LogEntries` ring to. Every public method of the package's seven classes is listed below with
// the reason it is absent, so that `scripts/audit-port.sh logger crates/freerouting/src '*.java'
// scripts/audit-map/freerouting.map` checks the deferral instead of skipping it. Line numbers are
// the clone's HEAD (plan ruling 7). The audit matches **per method**, so an overload set gets one
// line.
//
// What survives of this package is three things, all above: the level ladder, the single stderr
// sink, and [`MESSAGE_MAP`] — the message *set* the CLI must emit, which is a parity surface even
// though the logger is not.
//
// ── FRLogger.java (480) ─────────────────────────────────────────────────────────────────────────
//
// The static facade over log4j2 plus an in-memory ring and a trace-event bus. `tracing` is the
// port's facade; the ring and the bus have no reader here.
// not ported: FRLogger.setEnabled (:43-45) — a global mute. `tracing`'s subscriber is the
//   equivalent, and [`init`] installs exactly one.
// not ported: FRLogger.info (:233-255) — `tracing::info!` at the call site.
// not ported: FRLogger.warn (:263-285) — `tracing::warn!`.
// not ported: FRLogger.debug (:293-314) — `tracing::debug!`. Both overloads. Note that the
//   two-argument body has **no `logEntries.add` call at all** and simply `return null` (:303),
//   where `info`/`warn`/`error` end in `logEntries.add(...)` (:243, :273, :338) — which is why
//   `p8t5`'s Java half sees only info/warn/error.
// not ported: FRLogger.error (:324-350) — `tracing::error!`.
// not ported: FRLogger.trace (:373-430) — `tracing::trace!`, plus the two `trace(TraceEvent…)`
//   overloads that feed the listener bus below.
// not ported: FRLogger.isTraceEnabled (:357-359) — `tracing`'s own level filter answers this.
// not ported: FRLogger.traceEntry (:159-166) — a named performance timer keyed by a string. Spec
//   §10's `ProgressSink` replaces the observer surface; the timings reach the manifest through
//   `fr_core::manifest`, not through a logger.
// not ported: FRLogger.traceExit (:176-215) — the matching stop, both overloads.
// not ported: FRLogger.buildTracePayload (:100-118) — formats a trace event's JSON payload for
//   the listener bus (no bus).
// not ported: FRLogger.formatDuration (:53-73) — a human-readable duration for log lines. The
//   manifest's durations are numbers (`fr_core::manifest`), not formatted strings.
// not ported: FRLogger.formatScore (:78-95) — a human-readable score for log lines.
// not ported: FRLogger.formatNetLabel (:123-154) — both overloads; a `BasicBoard` net name for a
//   log line.
// not ported: FRLogger.disableLogging (:439-443) — sets `enabled = false`, which makes every
//   method above return **before** the ring is written. There is nothing to disable here.
// not ported: FRLogger.getLogEntries (:448-450) — the in-memory ring; see `LogEntries` below.
// not ported: FRLogger.getLogger (:457-462) — hands out the raw log4j2 `Logger`.
// not ported: FRLogger.addTraceEventListener (:466-468) — the trace-event bus (no observers,
//   plan Global Constraints).
// not ported: FRLogger.removeTraceEventListener (:471-473) — the same.
//
// ── Log4j2ConfigurationFactory.java (137) ───────────────────────────────────────────────────────
//
// not ported: Log4j2ConfigurationFactory.getConfiguration (:37-118) — builds the three appenders
//   programmatically from system properties. Both overloads. What it *decides* is reproduced
//   above — the `INFO` console default (`:102`) and `parseLevel`'s fallback (`:130-135`) — and
//   what it *builds* is quirk #261's stdout/stderr layout, which this port deliberately replaces
//   with one stderr stream.
//
// ── LogEntries.java, LogEntry.java, LogEntryType.java ───────────────────────────────────────────
//
// The in-memory ring `FRLogger` writes alongside log4j. Its readers are the Swing log window and
// the REST API's `GET /logs`, both out of scope (spec §2). It has one other reader worth naming:
// `scripts/differential/java/P8T5.java` uses `getEntries(null, null)` to read back the messages
// the parse emitted, because the console stream is redirected away — so the class this port does
// not have is what makes the port's messages comparable to Java's.
// not ported: LogEntries.add (:63-79) — both overloads; appends and fires the listeners.
// not ported: LogEntries.clear (:30-34) — empties the ring.
// not ported: LogEntries.get (:44-48) — every entry as `LogEntry.toString()`.
// not ported: LogEntries.getAsString (:37-41) — the same, newline-joined.
// not ported: LogEntries.getEntries (:51-60) — filtered by timestamp and topic (the REST API's
//   long-poll).
// not ported: LogEntries.getWarningCount (:16-20) — a counter no output file reads.
// not ported: LogEntries.getErrorCount (:23-27) — the same.
// not ported: LogEntries.addLogEntryAddedListener (:84-86) — the GUI's live tail.
// not ported: LogEntries.removeLogEntryAddedListener (:89-91) — the same.
// not ported: LogEntry.getType (:40-42) — the ring's element; no ring, no element.
// not ported: LogEntry.getMessage (:44-46) — the same.
// not ported: LogEntry.getTopic (:48-50) — the per-job/session UUID the REST API filters on.
// not ported: LogEntry.getException (:52-54) — the same.
// not ported: LogEntry.toString (:57-59) — `"%-7s".formatted(TYPE) + " " + message`, the shape
//   `p8t5` re-derives on both sides rather than importing.
//
// ── TraceEvent.java, TraceEventListener.java ────────────────────────────────────────────────────
//
// The structured trace bus `FRLogger.trace(TraceEvent)` feeds. No observers in this port (plan
// Global Constraints); `TraceEventListener` is an interface with no public methods of its own.
// not ported: TraceEvent.getMethod (:41-43) — the event record's accessors, all six.
// not ported: TraceEvent.getOperation (:45-47) — the same.
// not ported: TraceEvent.getMessage (:49-51) — the same.
// not ported: TraceEvent.getImpactedItems (:53-55) — the same.
// not ported: TraceEvent.getImpactedPoints (:57-59) — the same.
// not ported: TraceEvent.getTimestamp (:61-63) — the same.

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(args: &[&str]) -> Vec<String> {
        args.iter().map(|a| (*a).to_string()).collect()
    }

    #[test]
    fn the_default_console_level_is_java_s_info() {
        assert_eq!(level_from_argv(&argv(&[])), LogLevel::Info);
        assert_eq!(level_from_argv(&argv(&["route", "a.dsn"])), LogLevel::Info);
    }

    #[test]
    fn parse_java_falls_back_to_info_and_never_fails() {
        // Log4j2ConfigurationFactory.java:130-135.
        assert_eq!(LogLevel::parse_java("debug"), LogLevel::Debug);
        assert_eq!(LogLevel::parse_java("DeBuG"), LogLevel::Debug);
        assert_eq!(LogLevel::parse_java("fatal"), LogLevel::Error);
        assert_eq!(LogLevel::parse_java("all"), LogLevel::Trace);
        assert_eq!(LogLevel::parse_java("off"), LogLevel::Off);
        assert_eq!(LogLevel::parse_java("verbose"), LogLevel::Info);
        assert_eq!(LogLevel::parse_java(""), LogLevel::Info);
    }

    #[test]
    fn ll_is_prefix_matched_and_last_wins() {
        // `startsWith("-ll")` (GlobalSettings.java:825) — `-lll` is `-ll`.
        assert_eq!(level_from_argv(&argv(&["-lll", "debug"])), LogLevel::Debug);
        // Plan ruling 10: `GlobalSettings`' loop assigns on every occurrence, so the LAST wins.
        // Java's pre-bootstrap pass (`main:1060-1065`) takes the FIRST — quirk #262.
        assert_eq!(
            level_from_argv(&argv(&["-ll", "trace", "-ll", "error"])),
            LogLevel::Error
        );
    }

    #[test]
    fn ll_without_a_value_is_a_silent_no_op() {
        // The blanket rule: a value starting with `-` is never consumed.
        assert_eq!(level_from_argv(&argv(&["-ll"])), LogLevel::Info);
        assert_eq!(level_from_argv(&argv(&["-ll", "-de"])), LogLevel::Info);
    }

    #[test]
    fn the_console_level_string_is_javas_field_verbatim() {
        // LoggingSettings.java:25.
        assert_eq!(console_level_string(&argv(&[])), "INFO");
        // `-ll` upper-cases (GlobalSettings.java:828); `--logging.console.level=` does not
        // (`setValue` stores the raw string).
        assert_eq!(console_level_string(&argv(&["-ll", "debug"])), "DEBUG");
        assert_eq!(
            console_level_string(&argv(&["--logging.console.level=debug"])),
            "debug"
        );
        // Java does not validate it.
        assert_eq!(console_level_string(&argv(&["-ll", "garbage"])), "GARBAGE");
        assert_eq!(LogLevel::parse_java("GARBAGE"), LogLevel::Info);
    }

    #[test]
    fn the_port_only_spellings_are_accepted_too() {
        assert_eq!(
            level_from_argv(&argv(&["--logging.console.level=warn"])),
            LogLevel::Warn
        );
        assert_eq!(
            level_from_argv(&argv(&["--log-level", "trace"])),
            LogLevel::Trace
        );
        assert_eq!(level_from_argv(&argv(&["-v"])), LogLevel::Debug);
        assert_eq!(level_from_argv(&argv(&["-vv"])), LogLevel::Trace);
    }

    #[test]
    fn the_message_map_is_unique_by_call_site_and_never_empty() {
        let mut seen = std::collections::BTreeSet::new();
        for (site, message) in MESSAGE_MAP {
            assert!(
                seen.insert(*site),
                "duplicate FRLogger call site in MESSAGE_MAP: {site}"
            );
            assert!(!message.is_empty(), "empty message for {site}");
            assert!(
                site.contains(".java:"),
                "MESSAGE_MAP keys are <file>:<line>, got {site}"
            );
        }
    }
}
