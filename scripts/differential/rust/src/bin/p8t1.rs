//! `p8t1` — Plan 8 Task 6's **headline gate** (controller ruling AV): the HEAD jar and the port,
//! run as two whole programs on the same command line, compared on the SES bytes, the exit code
//! and the log.
//!
//! # Why this driver has no `P8T1.java`
//!
//! Every other driver in `scripts/differential/` is a pair — a Java class and a Rust twin, each
//! printing a transcript, with `run.sh` diffing the two. That shape exists because the *thing*
//! under test is a Java method that has to be called from inside a JVM.
//!
//! Here the thing under test is **the jar**. `java -jar freerouting.jar -de … -do …` against
//! `freerouting -de … -do …` needs no Java class to drive it, and the brief's own interface list
//! puts both runners and both normalisers in `tests/parity` — `run_jar`, `run_port`,
//! `normalize_log`, `normalize_manifest`, all Rust. A `P8T1.java` could only re-implement
//! `normalize_log` a second time, in a second language, from the same rules; the two copies could
//! then drift, and the diff would report agreement between two wrong answers. So this driver owns
//! the comparison and prints its own verdict, and `run.sh`'s `rust_only` mode runs it. The
//! deviation is recorded in the Task 6 report.
//!
//! # What is compared, per stem
//!
//! | rung | assertion |
//! |---|---|
//! | (a) | the two SES files are byte-identical, after quirk #92's four `(parser …)` keyword literals are rewritten on the **jar** side (`parity::normalize_ses_head_tokens`) |
//! | (b) | the two exit codes are equal |
//! | (c) | `parity::normalize_log` of both sides is equal |
//!
//! **No tolerance, ever.** A divergence is an `XDIFF` row in `crates/freerouting/README.md` with
//! the first differing byte, the offending item and a one-line root cause — not a widened band
//! here.
//!
//! # Usage
//!
//! ```text
//! scripts/differential/run.sh p8t1              # the four `ci` stems
//! scripts/differential/run.sh p8t1 all          # every stem of cli-fixtures.txt
//! scripts/differential/run.sh p8t1 <stem> …     # the named stems
//! ```

use std::path::{Path, PathBuf};

/// One stem's verdict.
struct Row {
    stem: String,
    verdict: &'static str,
    detail: String,
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let stems = select_stems(&args);
    if stems.is_empty() {
        eprintln!("p8t1: no stem selected");
        std::process::exit(1);
    }

    let scratch = std::env::temp_dir().join("p8t1");
    let _ = std::fs::remove_dir_all(&scratch);

    let mut rows = Vec::new();
    for stem in &stems {
        rows.push(compare(&stem.name, &scratch));
    }
    rows.extend(refusal_rows(&scratch));

    println!("== p8t1: the jar and the port, two whole programs, {} stems", rows.len());
    println!("{:<26} {:<7} {}", "stem", "verdict", "detail");
    for row in &rows {
        println!("{:<26} {:<7} {}", row.stem, row.verdict, row.detail);
    }
    let matched = rows.iter().filter(|r| r.verdict == "MATCH").count();
    let xdiff = rows.iter().filter(|r| r.verdict == "XDIFF").count();
    println!(
        "rows: {}  MATCH: {}  XDIFF: {}  DIFF: {}",
        rows.len(),
        matched,
        xdiff,
        rows.len() - matched - xdiff
    );
    if matched + xdiff != rows.len() {
        std::process::exit(1);
    }
}

/// `all`, a list of stem names, or — with no argument — the `ci` lane.
fn select_stems(args: &[String]) -> Vec<parity::CliStem> {
    let all = parity::cli_stems();
    if args.is_empty() {
        return all.into_iter().filter(|s| s.ci).collect();
    }
    if args.len() == 1 && args[0] == "all" {
        return all;
    }
    args.iter()
        .map(|name| {
            all.iter()
                .find(|s| &s.name == name)
                .unwrap_or_else(|| panic!("no such stem in cli-fixtures.txt: {name}"))
                .clone()
        })
        .collect()
}

fn compare(stem: &str, scratch: &Path) -> Row {
    let jar_dir = scratch.join(format!("{stem}-jar"));
    let port_dir = scratch.join(format!("{stem}-port"));
    for dir in [&jar_dir, &port_dir] {
        std::fs::create_dir_all(dir).expect("a scratch directory");
    }

    let jar_argv = parity::cli_argv(stem, &jar_dir);
    let port_argv = parity::cli_argv(stem, &port_dir);
    let jar_refs: Vec<&str> = jar_argv.iter().map(String::as_str).collect();
    let port_refs: Vec<&str> = port_argv.iter().map(String::as_str).collect();

    let (jar_out, jar_err, jar_code) = parity::run_jar(&jar_refs);
    let (port_out, port_err, port_code) = parity::run_port(&port_refs);

    // Rung (b) first: a wrong exit code explains a missing SES.
    if jar_code != port_code {
        return Row {
            stem: stem.to_string(),
            verdict: "DIFF",
            detail: format!(
                "exit: jar {jar_code}, port {port_code}; port stderr: {}",
                first_line(&port_err)
            ),
        };
    }

    // Rung (a).
    let jar_ses = read(&jar_dir.join("route.ses"));
    let port_ses = read(&port_dir.join("route.ses"));
    match (jar_ses, port_ses) {
        (Some(jar), Some(port)) => {
            let jar = parity::normalize_ses_head_tokens(&jar);
            if jar != port {
                let at = jar
                    .bytes()
                    .zip(port.bytes())
                    .position(|(a, b)| a != b)
                    .unwrap_or_else(|| jar.len().min(port.len()));
                return Row {
                    stem: stem.to_string(),
                    verdict: "DIFF",
                    detail: format!(
                        "SES differs at byte {at} (jar {} B, port {} B); jar {:?} port {:?}",
                        jar.len(),
                        port.len(),
                        window(&jar, at),
                        window(&port, at)
                    ),
                };
            }
        }
        (jar, port) => {
            return Row {
                stem: stem.to_string(),
                verdict: "DIFF",
                detail: format!(
                    "SES presence: jar {}, port {}",
                    if jar.is_some() { "written" } else { "absent" },
                    if port.is_some() { "written" } else { "absent" }
                ),
            };
        }
    }

    // Rung (c).
    let jar_log = parity::normalize_log(&jar_out, &jar_err);
    let port_log = parity::normalize_log(&port_out, &port_err);
    if jar_log != port_log {
        return Row {
            stem: stem.to_string(),
            verdict: "DIFF",
            detail: format!(
                "log: jar {:?} port {:?}",
                jar_log.replace('\n', " | "),
                port_log.replace('\n', " | ")
            ),
        };
    }

    let bytes = std::fs::metadata(port_dir.join("route.ses")).map_or(0, |m| m.len());
    Row {
        stem: stem.to_string(),
        verdict: "MATCH",
        detail: format!("exit {jar_code}, {bytes} B SES, log {} lines", jar_log.lines().count()),
    }
}

fn read(path: &PathBuf) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

fn window(text: &str, at: usize) -> &str {
    let lo = at.saturating_sub(30);
    let hi = (at + 30).min(text.len());
    text.get(lo..hi).unwrap_or("")
}

fn first_line(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .lines()
        .next()
        .unwrap_or("")
        .to_string()
}

// =================================================================================================
// The refusal rows
// =================================================================================================

/// Five argv shapes chosen so that rungs (b) and (c) are not vacuous.
///
/// Every `cli-fixtures.txt` stem succeeds, and a successful run emits no message
/// `freerouting::logging::MESSAGE_MAP` names — the jar's whole transcript is the startup banner
/// and the pipeline's progress chatter, all of which `parity::normalize_log` drops for the
/// reasons its own doc comment gives. So on the stems the log rung reports `0 lines` on both
/// sides, which proves the streams agree but proves nothing about the messages the CLI is
/// supposed to emit. These rows do: each one reaches a `MESSAGE_MAP` site, and the first also
/// exercises quirk #261's duplicate — the jar writes its `ERROR` to stdout *and* stderr, and the
/// normaliser has to fold the two back into one.
///
/// They are argv shapes rather than fixture stems because what they pin is the *argv*, not a
/// board; four of them produce no SES at all, so nothing about them belongs in
/// `tests/reference/cli-*`. The fifth, `settings-on-legacy`, **succeeds** on both sides and is
/// here for controller ruling BG: `--settings` is scoped to the native form (scan ruling R7) and
/// the legacy path is bug-for-bug (ruling AR), so the jar's two `Unknown command line argument`
/// warnings are what the port must emit — and this row is what compares them against the jar
/// rather than against a transcribed expectation. That the port also *ignores* the file, as the
/// jar does, is asserted by
/// `crates/freerouting/tests/cli_e2e.rs::a_settings_file_reaches_the_run`, which can read the
/// resolved setting out of the manifest; this row cannot.
fn refusal_rows(scratch: &Path) -> Vec<Row> {
    let dir = scratch.join("refusals");
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let dsn = parity::fixture("Issue143-rpi_splitter.dsn");
    let missing = dir.join("nosuch.dsn");
    let ses_bytes_named_dsn = dir.join("session.dsn");
    std::fs::write(&ses_bytes_named_dsn, b"(session previous)\n").expect("write the fixture");
    // A settings document that would be visible in the answer if either side applied it —
    // `DefaultSettings.java:149` seeds `scoring.viaCosts` at 50.
    let settings_json = dir.join("s.json");
    std::fs::write(
        &settings_json,
        br#"{"router": {"scoring": {"via_costs": 77}}}"#,
    )
    .expect("write the fixture");

    let cases: Vec<(&str, Vec<String>)> = vec![
        // `Freerouting.java:105` (`FRLogger.error`, duplicated to stderr) + `:109`
        // (`FRLogger.warn`), then exit 1.
        (
            "missing-input",
            argv(&[
                "-de",
                &missing.to_string_lossy(),
                "-do",
                &dir.join("a.ses").to_string_lossy(),
            ]),
        ),
        // `Freerouting.java:81` — neither slot filled, `legacy::rewrite`'s refusal.
        ("no-files", argv(&["-mp", "1"])),
        // Quirk label L: `-do out.dsn` is accepted by `tryToSetOutputFile` and serialised by
        // nothing, so a 0-byte file is left behind and the run exits 1.
        (
            "do-out-dsn",
            argv(&[
                "-de",
                &dsn.to_string_lossy(),
                "-do",
                &dir.join("b.dsn").to_string_lossy(),
                "-mp",
                "1",
            ]),
        ),
        // Plan ruling 7 / quirk #244: session bytes under a `.dsn` name reach
        // `RoutingJobState.INVALID`, where **the jar hangs for ever** — so this row is expected
        // to be an `XDIFF`, and the driver says so rather than waiting.
        // Ruling BG: `--settings` on the **legacy** form is two unknown arguments to the jar
        // (`GlobalSettings.java:833`, once for the flag and once for its argument — it is not a
        // value-consuming arm), and the port must say the same two things. A successful run, so
        // unlike its neighbours it also proves the warnings do not disturb the exit code.
        (
            "settings-on-legacy",
            argv(&[
                "-de",
                &dsn.to_string_lossy(),
                "-do",
                &dir.join("d.ses").to_string_lossy(),
                "-mp",
                "1",
                "--settings",
                &settings_json.to_string_lossy(),
            ]),
        ),
        (
            "invalid-input-java-hangs",
            argv(&[
                "-de",
                &ses_bytes_named_dsn.to_string_lossy(),
                "-do",
                &dir.join("c.ses").to_string_lossy(),
            ]),
        ),
    ];

    cases
        .into_iter()
        .map(|(name, args)| {
            if name == "invalid-input-java-hangs" {
                // Not run: `Freerouting.isCliTerminalState` omits `INVALID`
                // (`Freerouting.java:189-194`), so the jar sits in `:151-158`'s
                // `while (…) Thread.sleep(500)` at 0 % CPU with no output and no message. Running
                // it would hang this driver. The port's answer is pinned by
                // `crates/freerouting/tests/cli_e2e.rs::de_a_ses_exits_1_instead_of_hanging`.
                return Row {
                    stem: name.to_string(),
                    verdict: "XDIFF",
                    detail: "the jar HANGS (quirk #244: INVALID is not in isCliTerminalState); \
                             the port exits 1 — plan ruling 7, not run here"
                        .to_string(),
                };
            }
            let refs: Vec<&str> = args.iter().map(String::as_str).collect();
            let (jar_out, jar_err, jar_code) = parity::run_jar(&refs);
            let (port_out, port_err, port_code) = parity::run_port(&refs);
            if jar_code != port_code {
                return Row {
                    stem: name.to_string(),
                    verdict: "DIFF",
                    detail: format!("exit: jar {jar_code}, port {port_code}"),
                };
            }
            let jar_log = parity::normalize_log(&jar_out, &jar_err);
            let port_log = parity::normalize_log(&port_out, &port_err);
            if jar_log != port_log {
                return Row {
                    stem: name.to_string(),
                    verdict: "DIFF",
                    detail: format!(
                        "log: jar {:?} port {:?}",
                        jar_log.replace('\n', " | "),
                        port_log.replace('\n', " | ")
                    ),
                };
            }
            Row {
                stem: name.to_string(),
                verdict: "MATCH",
                detail: format!(
                    "exit {jar_code}, log [{}]",
                    jar_log.trim_end().replace('\n', " | ")
                ),
            }
        })
        .collect()
}

fn argv(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|part| (*part).to_string()).collect()
}
