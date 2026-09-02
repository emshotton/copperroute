//! The four subcommand runners.
//!
//! Each takes its parsed arguments **and the raw argv**, because the settings ladder is built
//! from the raw argv rather than the rewritten one (scan ruling R19 — see `crate::run`).

pub mod drc;
pub mod info;
pub mod route;

use fr_settings::sources::CliSettings;
use std::path::PathBuf;

/// The priority-60 source, built with ruling **BJ**'s `--set` alias **on the native form only**.
///
/// # The two command lines have two different generic overrides, and neither has both
///
/// | form | generic override | why |
/// |---|---|---|
/// | **legacy** | `--<section>.<field>=<value>` — Java's own | `crate::legacy::rewrite` passes an unknown `--name=value` straight through, and `CliSettings` reads it exactly as the jar does |
/// | **native** | `--set <section>.<field>=<value>` | `clap` owns this command line and has **no arm** for a free-form `--<section>.<field>=<value>`: it answers `error: unexpected argument '--router.optimizer.max_threads' found` and exit 2, before any runner is reached |
///
/// So the two spellings are not interchangeable, and saying otherwise — which the first draft of
/// Task 14's documentation did, in `--help` — sends a reader to a usage error. `--set` is declared
/// on `route` only (`cli::RouteArgs::set`); on `drc`, `info` and `mcp` `clap` refuses it, so the
/// alias constructor below can never fire for those runners even though they call this function.
/// That is deliberate: one predicate, one place, no runner able to disagree about which command
/// line the user typed — the shape [`json_settings_path`] already uses for ruling BG.
///
/// **Why the legacy form must not honour `--set`.** Ruling AR makes that path bug-for-bug, and
/// the jar ignores `--set` twice over — `--set` has no `=` so `CliSettings.java:45` skips it, and
/// `router.x=7` does not start with `-` so neither branch sees it. Honouring it here would make
/// `freerouting -de a.dsn -do b.ses --set router.max_passes=7` route differently from the jar on
/// the same argv. `p8t5`'s `set-on-legacy` row is that comparison, run against the live jar.
pub(crate) fn cli_settings(settings_argv: &[String]) -> CliSettings {
    if crate::legacy::is_legacy_form(settings_argv) {
        CliSettings::new(settings_argv)
    } else {
        CliSettings::new_with_set_alias(settings_argv)
    }
}

/// `--settings <file>`'s path, read back off the **raw** argv — **on the native form only**.
///
/// The flag is `clap`'s and global (`cli::Cli::settings`), but the command runners are handed the
/// raw argv rather than the parsed `Cli` (see [`crate::run`]), so the value is recovered here
/// from the same slice `CliSettings` reads — which also keeps the whole runner testable in
/// process, without a `std::env::args()` read no test could control.
///
/// # Controller ruling BG: the legacy form warns and applies nothing
///
/// Scan ruling R7 scoped this flag to the **native** form, and ruling AR makes the legacy path
/// bug-for-bug. Java has no flag for this tier at all, so
/// `freerouting -de a.dsn -do b.ses --settings s.json` is, to the jar, two unknown arguments:
/// it warns `Unknown command line argument: --settings` and then
/// `Unknown command line argument: s.json` (`GlobalSettings.java:833` twice — the flag is not a
/// value-consuming arm, so its argument warns on its own) and **ignores the file**. Measured on
/// `Issue143-rpi_splitter.dsn` with `{"router": {"scoring": {"via_costs": 77}}}`: the jar's
/// manifest reports `via_costs 50`.
///
/// Round 1 of the Task 6 review left the port applying the file on that argv — same warnings,
/// different settings — and recorded it as accepted. **Ruling BG ruled it out**: the legacy path
/// reproduces the jar, so it must warn identically *and* apply nothing. The
/// [`crate::legacy::is_legacy_form`] guard below is that ruling.
///
/// # Two callers, one rule
///
/// [`route`] fills merge #1's priority-10 slot with it, and [`drc`] fills the **prototype**
/// merger's (`Freerouting.java:1411`'s `new JsonFileSettings()`) for the quality-score sub-merge —
/// see `drc::quality_score`. It lives here rather than in either runner so the two cannot disagree
/// about which command line carries a `--settings`.
///
/// The warnings themselves are `legacy::rewrite`'s, unchanged and already jar-compared — `p8t1`'s
/// `settings-on-legacy` row runs the two programs on exactly this argv, and
/// `crates/freerouting/tests/cli_e2e.rs::a_settings_file_reaches_the_run` asserts the settings
/// answer on both forms.
pub(crate) fn json_settings_path(settings_argv: &[String]) -> Option<PathBuf> {
    // Ruling BG. `is_legacy_form` is the same predicate `crate::run` dispatches on, so the two
    // can never disagree about which command line the user typed.
    if crate::legacy::is_legacy_form(settings_argv) {
        return None;
    }
    let mut args = settings_argv.iter();
    while let Some(arg) = args.next() {
        if let Some(value) = arg.strip_prefix("--settings=") {
            return Some(PathBuf::from(value));
        }
        if arg == "--settings" {
            return args.next().map(PathBuf::from);
        }
    }
    None
}
