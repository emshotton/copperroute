//! Constants shared by more than one `fr-drc` integration suite.
//!
//! Integration tests are separate binaries, so a `const` in one is invisible to the next; a
//! `tests/common/` module is Cargo's way of sharing one (a subdirectory is not itself a test
//! target). Everything here is a property of the **jar the transcripts came from**, not of the
//! port, which is why it lives in one place: a jar rebuild that moves it must move it once.

/// `Constants.FREEROUTING_VERSION` of the jar every committed transcript was taken with
/// (`../freerouting/build/libs/freerouting-current-executable.jar`, plan-5 ruling 1).
///
/// The port never reads a version of its own: `generate_report` prefixes `"Freerouting "` to
/// whatever `DrcReportOptions::freerouting_version` carries
/// (DesignRulesChecker.java:212-213), so this is only the value the goldens were taken with —
/// and the Java probes inject the same literal.
///
/// A rebuilt jar with a new `Constants.FREEROUTING_VERSION` needs **this constant and every
/// `tests/data/*` transcript** regenerated together; `tests/data/README.md` carries the commands.
/// Used by `tests/report.rs`, `tests/report_json.rs` and `tests/java_ports.rs`.
///
/// **Plan 8 Task 14, controller sweep item N6: this is now an alias, not a second literal.**
/// It used to carry its own `"2.3.1-SNAPSHOT"`, written when `fr-core` did not exist. Controller
/// ruling AT/5 makes [`fr_core::PARITY_VERSION`] the single place the pinned jar's
/// `Constants.FREEROUTING_VERSION` is written down, and this now reads it rather than repeating
/// it. **It is not a fourth of ruling AT's three readers.** Those three are FILE-FORMAT fields in
/// shipping code — the DRC report's `freeroutingVersion` (`crates/freerouting/src/commands/drc.rs`),
/// the MCP `check_drc` tool's (`mcp/tools/check_drc.rs`) and the manifest's `app_version`
/// (`fr-core/src/manifest.rs`) — and this is a **test** constant that names the version the
/// committed transcripts were taken with. It has to equal the same string, which is why it is now
/// spelled as that string rather than as a copy of it. `fr-core` is a **dev**-dependency here and only a dev-dependency: the
/// shipping `fr-drc` still depends on nothing above `fr-dsn`, so spec §4's direction
/// (`fr-core -> fr-router -> fr-drc`) is unchanged and the edge exists solely for this test-only
/// binary. Cargo permits the cycle for exactly that reason.
#[allow(dead_code)] // not every suite that includes this module uses every constant
pub const JAR_VERSION: &str = fr_core::PARITY_VERSION;
