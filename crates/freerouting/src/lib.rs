#![forbid(unsafe_code)]

//! `freerouting` — the command-line program, and the library the differential drivers and the
//! integration tests drive it through.
//!
//! The binary (`src/main.rs`) is three lines: read `argv`, call [`run`], exit with the code it
//! answers. Everything else lives here so that `scripts/differential/rust/src/bin/p8t5.rs` can
//! compare the **same** [`legacy::rewrite`] the program runs against the jar, rather than a
//! second copy of the rule written for the driver.
//!
//! # The two command lines
//!
//! * The **legacy** form is Java's: `freerouting -de board.dsn -do board.ses -mp 100`. It is
//!   rewritten, bug for bug, by [`legacy::rewrite`] — see that module for the rules and for what
//!   deliberately does *not* happen there.
//! * The **native** form is the port's: `freerouting route board.dsn -o board.ses`. It is clap's,
//!   and it is the only form that can produce [`legacy::ExitCode::UsageError`] (ruling AR).
//!
//! [`legacy::is_legacy_form`] decides which: the native form starts with a subcommand name, and
//! every other argv — including an empty one — is legacy.

pub mod cli;
pub mod commands;
pub mod legacy;
pub mod logging;
pub mod mcp;

use clap::Parser;

use legacy::{ExitCode, Level};

/// Runs the program over a raw argv (**without** the program name) and answers its exit code.
///
/// The order below is Java's, and each step's position is load-bearing:
///
/// 1. **The log level first.** Java resolves it in a pre-bootstrap pass
///    (`Freerouting.java:1032-1065`) before log4j is configured, precisely so the flag-table
///    warnings come out at the level the command line asked for. [`logging::level_from_argv`]
///    reads the **raw** argv for the same reason.
/// 2. **The rewrite**, which produces the native argv and every `FRLogger` line Java would have
///    emitted (`GlobalSettings.applyCommandLineArguments`, `:521-838`).
/// 3. **The diagnostics**, at their Java levels.
/// 4. **The exit ladder** (`Freerouting.java:1469-1495`): an empty rewrite is a refusal Java
///    answers with `System.exit(1)`; clap owns the native form and its usage errors; a subcommand
///    that is not wired up yet answers [`ExitCode::NotImplemented`], which Task 12 must make
///    unreachable.
///
/// # The raw argv is the settings argv
///
/// `raw` — not the rewritten argv — is what the settings ladder must be built from, because
/// `fr_settings::CliSettings` is Java's **second** parser and matches `-mp`/`-mt` with an exact
/// `switch` where the rewrite matches by prefix (scan ruling R19). Feeding it the rewritten argv
/// would make `-mpx 5` set `max_passes`, which no Java parser does. Each command runner therefore
/// takes it as its second argument.
//
// not ported: Freerouting.main's startup version line (`Freerouting.java:1120`) —
//   `FRLogger.info("Freerouting " + VERSION_NUMBER_STRING)`, i.e.
//   `Freerouting v2.3.1-SNAPSHOT (build-date: 2026-09-01)`, printed before anything else on every
//   run. The port does not print it, and the reason is controller ruling AT: `PARITY_VERSION` is
//   for FILE-FORMAT fields, and a *banner* that claims to be the jar would be the port lying
//   about what it is. Printing the crate's own version instead would be a different string, which
//   is a divergence either way — so the divergence is taken in the honest direction and recorded
//   here and in `parity::normalize_log`'s `SUPPRESSED_SITES`, which is the one place the `p8t1`
//   comparison skips a `MESSAGE_MAP` site. `crate::logging::MESSAGE_MAP` still carries the entry,
//   because its job is to enumerate the CLI's `FRLogger` calls, not to promise the port makes
//   them.
// not ported: Freerouting.main's `Couldn't initialize the GUI` arm (`:1450`) — spec §2 has no GUI,
//   and `legacy::rewrite` never produces a GUI mode.
#[must_use]
pub fn run(raw: &[String]) -> ExitCode {
    logging::init(logging::level_from_argv(raw));

    let (argv, diagnostics) = legacy::rewrite(raw);
    for diagnostic in &diagnostics {
        match diagnostic.level {
            Level::Warn => tracing::warn!("{}", diagnostic.message),
            Level::Error => tracing::error!("{}", diagnostic.message),
        }
    }

    // Port only, at DEBUG: the native command line the legacy form was rewritten into. Java has
    // no counterpart — it never builds a second command line — and this is how plan ruling 12's
    // port of `GlobalSettingsCommandLineTest` observes the four filename slots **through the
    // binary** (`crates/freerouting/tests/legacy_cli.rs`). `{argv:?}` rather than a join, so a
    // filename with a space in it (`GlobalSettingsCommandLineTest.filenameWithSpaces`) is still
    // unambiguous.
    tracing::debug!("rewritten command line: {argv:?}");

    // `Freerouting.java:80-86` / `:247-250` — the two refusals, both `System.exit(1)` by way of
    // `cliResult == false` at `:1469-1474`. `rewrite` has already logged the `FRLogger.error`.
    if argv.is_empty() {
        return ExitCode::Failure;
    }

    let cli = match cli::Cli::try_parse_from(
        std::iter::once("freerouting".to_string()).chain(argv.iter().cloned()),
    ) {
        Ok(cli) => cli,
        Err(error) => {
            // clap's own exit codes: 0 for `--help`/`--version`, 2 for a usage error. The second
            // is [`ExitCode::UsageError`], and ruling AR confines it to the native form — which
            // is exactly what `rewrite` guarantees, because every argv it *builds* parses.
            let _ = error.print();
            return if error.use_stderr() {
                ExitCode::UsageError
            } else {
                ExitCode::Ok
            };
        }
    };

    match &cli.command {
        cli::Command::Route(args) => commands::route::run(args, raw),
        cli::Command::Drc(args) => commands::drc::run(args, raw),
        cli::Command::Info(args) => commands::info::run(args, raw),
        // The MCP transport predates the exit ladder and answers a raw code; it can only ever be
        // 0 today (`mcp::stdio::run_with` returns 0 on EOF and on a broken pipe alike).
        cli::Command::Mcp => match mcp::stdio::run() {
            0 => ExitCode::Ok,
            _ => ExitCode::Failure,
        },
    }
}
