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
#[allow(dead_code)] // not every suite that includes this module uses every constant
pub const JAR_VERSION: &str = "2.3.1-SNAPSHOT";
