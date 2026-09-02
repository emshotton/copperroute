//! The four subcommand runners.
//!
//! Each takes its parsed arguments **and the raw argv**, because the settings ladder is built
//! from the raw argv rather than the rewritten one (scan ruling R19 — see `crate::run`).

pub mod drc;
pub mod info;
pub mod route;

use std::path::PathBuf;

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
