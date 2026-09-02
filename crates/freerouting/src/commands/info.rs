//! `freerouting info` — spec §12's third subcommand, and **the only CLI surface with no Java
//! counterpart at all**.
//!
//! The jar has no `info` mode: `Freerouting.main`'s mode ladder (`Freerouting.java:1455-1467`)
//! is GUI, DRC and CLI, and `legacy::rewrite` can never produce this subcommand, so no legacy
//! argv reaches it. `crates/freerouting/README.md`'s "Port-only spellings" table says so.
//!
//! # The load is `drc`'s, not `route`'s
//!
//! The obligation this runner discharges named `fr_settings::resolve_headless` over
//! `settings_argv`. **What landed does not call it, deliberately**, and the reason is the same
//! one `commands::drc`'s step 6 records: the summary reads the *board*, never the settings, so a
//! merge would be work whose result nothing could observe. What the summary needs is a loaded
//! board, and [`fr_core::load_board_if_needed`] is the whole loader — the same call `drc` makes,
//! with `job.router_settings` still `new RouterSettings()`, so
//! `applyRouterSettingsForLoadedBoard`'s two board mutations read the two `null` clearance
//! overrides and no-op exactly as they do there. Splitting the loader (which is what `route` has
//! to do, because merge #1 needs the parsed board) would be the same board today and a
//! divergence the moment either default changed.
//!
//! `settings_argv` is therefore unread on this path, and the parameter stays because
//! [`crate::run`] hands every runner the same pair (scan ruling R19) and because a future
//! `--set` on `info` would need it.

use crate::cli::InfoArgs;
use crate::legacy::ExitCode;
use fr_core::{RoutingJob, SessionId};

/// Loads the board and writes [`fr_core::BoardSummary`] to stdout as JSON, then exits 0.
///
/// Spec §12: *"`drc` and `info` write JSON to stdout when no `-o`"* — `info` has no `-o` at all,
/// so stdout is its only surface.
///
/// The two failures are `drc`'s, with `drc`'s own messages, because they are the same two calls:
/// an input that cannot be read (`Freerouting.java:266`) and a board that will not load
/// (`BoardLoader.java:32-37`, `:52`). Both answer [`ExitCode::Failure`].
pub fn run(args: &InfoArgs, settings_argv: &[String]) -> ExitCode {
    // See the module docs: the ladder is not consulted, and the argument is kept because every
    // runner takes the same pair.
    let _ = settings_argv;

    let mut job = RoutingJob::new(SessionId::NIL);
    if let Err(error) = job.set_input(&args.input) {
        tracing::error!(
            "Couldn't load the input file '{}': {error}",
            args.input.display()
        );
        return ExitCode::Failure;
    }

    let loaded = match fr_core::load_board_if_needed(&mut job) {
        Ok(loaded) => loaded,
        Err(error) => {
            // Quirk #274 (label S)'s "Only DSN and JSON formats are supported" and
            // `BoardLoader.java:52`'s "Failed to load board" both arrive as this message.
            tracing::error!("{error}");
            return ExitCode::Failure;
        }
    };
    let mut board = loaded.board;
    for warning in &loaded.warnings {
        tracing::warn!("{warning}");
    }

    // `loaded.metadata` is `None` on every load path (`LoadedBoard::metadata`'s doc); it is
    // passed anyway rather than hard-coding the `None`, so that a reader who fills it later gets
    // it here without editing this line.
    let summary = fr_core::summarise(&mut board, loaded.metadata.as_ref());
    println!("{}", summary.to_json_pretty());
    ExitCode::Ok
}
