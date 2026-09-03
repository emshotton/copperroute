//! `p8t1` — Plan 8 Task 6's **headline gate** (controller ruling AV), **converted to port-golden
//! comparison at Plan 9's M1 accept wave** (ruling BV): the port, run as a whole program on the
//! argv each `tests/reference/cli-<stem>/argv.txt` records, compared against that stem's committed
//! reference on the SES bytes, the exit code and the log.
//!
//! # The lane switch, and why (survey §7.3, ruling BV)
//!
//! Until Plan 9 this driver ran the **jar** and the **port** side by side and required them to
//! agree. Plan 9 Task 2 fixed two measured Java regressions — R1 (#293), the deleted
//! shortest-airline-first ordering of the work list, and R2 (#294), the micro-neckdown fanout
//! fallback that ignored the board's minimum track width — and the port is now *deliberately*
//! better than the jar on every routed board. The consequence showed up here immediately: four of
//! the five `ci` rows (`router-rpi-splitter` @889, `router-j2-reference` @742,
//! `router-ecc83-input` @1755, `kicad-ecc83-json` @2335) turned `DIFF` and stayed there, and a
//! harness that is red by design is a harness nobody reads.
//!
//! So the **jar arm is retired from the default lane, not deleted**: it lives behind
//! `run.sh --against-jar`, which exports `AGAINST_JAR=1` and is read at [`against_jar`]. The
//! default lane compares the port against `tests/reference/cli-<stem>/`, which Task 2 regenerated
//! from the port at `bd296d7`. Nothing about the driver's *shape* changes — same stems, same three
//! rungs, same verdict table — only what the right-hand side is.
//!
//! The escape hatch is a triage tool and never a gate: under `--against-jar` those four rows are
//! expected to `DIFF`, and the divergence is R1/R2 doing their job.
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
//! | (a) | the SES bytes equal `tests/reference/cli-<stem>/route.ses` (the jar's file, under `--against-jar`, after quirk #92's four `(parser …)` keyword literals are rewritten on the jar side by `parity::normalize_ses_head_tokens`) |
//! | (b) | the exit code equals `route.exit` (the jar's, under `--against-jar`) |
//! | (c) | `parity::normalize_log` of the run equals the same projection of `route.log` (of the jar's streams, under `--against-jar`) |
//!
//! **No tolerance, ever.** A divergence in the default lane is a change in the port and is an
//! `XDIFF` row in `crates/freerouting/README.md` with the first differing byte, the offending item
//! and a one-line root cause — not a widened band here.
//!
//! # The refusal rows have no committed golden
//!
//! Four of the five write no SES at all, so nothing about them belongs under
//! `tests/reference/cli-*` — see [`refusal_rows`]. In the default lane their exit code and log
//! projection are compared against literals held in this file, cut from the port and byte-identical
//! to the jar's answers at the last `--against-jar` run (they are refusal paths: R1 and R2 cannot
//! reach them, and the run above the conversion recorded all four `MATCH`). Under `--against-jar`
//! the jar answers them live, exactly as before.
//!
//! # Usage
//!
//! ```text
//! scripts/differential/run.sh p8t1                 # the five `ci` stems, port vs golden
//! scripts/differential/run.sh p8t1 all             # every stem of cli-fixtures.txt
//! scripts/differential/run.sh p8t1 <stem> …        # the named stems
//! scripts/differential/run.sh --against-jar p8t1   # the retired arm: live jar vs live port
//! ```

use std::path::{Path, PathBuf};

/// One stem's verdict.
struct Row {
    stem: String,
    verdict: &'static str,
    detail: String,
}

/// `run.sh` exports `AGAINST_JAR=1` for `--against-jar`; anything else is the default
/// port-golden lane. Read once, in `main`, and threaded down — so a row can never be measured
/// against one side while the header claims the other.
fn against_jar() -> bool {
    std::env::var("AGAINST_JAR").is_ok_and(|value| value == "1")
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

    let against_jar = against_jar();
    let mut rows = Vec::new();
    for stem in &stems {
        rows.push(compare(&stem.name, &scratch, against_jar));
    }
    rows.extend(refusal_rows(&scratch, against_jar));

    if against_jar {
        println!(
            "== p8t1: --against-jar (the retired arm) — the jar and the port, two whole programs, \
             {} rows",
            rows.len()
        );
        println!(
            "   R1 (#293) and R2 (#294) make the port route every board differently, so a routed \
             row is EXPECTED to DIFF here."
        );
    } else {
        println!(
            "== p8t1: the port against its committed golden in tests/reference/cli-*, {} rows",
            rows.len()
        );
    }
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

fn compare(stem: &str, scratch: &Path, against_jar: bool) -> Row {
    let jar_dir = scratch.join(format!("{stem}-jar"));
    let port_dir = scratch.join(format!("{stem}-port"));
    for dir in [&jar_dir, &port_dir] {
        std::fs::create_dir_all(dir).expect("a scratch directory");
    }

    let port_argv = parity::cli_argv(stem, &port_dir);
    let port_refs: Vec<&str> = port_argv.iter().map(String::as_str).collect();
    let (port_out, port_err, port_code) = parity::run_port(&port_refs);

    // The right-hand side: the committed golden, or — behind the escape hatch — a live jar run on
    // the same argv. `jar` is the label the detail lines use for it either way, because what a
    // reader wants to know first is *which side* moved, not which file it came from.
    let (jar_out, jar_err, jar_code) = if against_jar {
        let jar_argv = parity::cli_argv(stem, &jar_dir);
        let jar_refs: Vec<&str> = jar_argv.iter().map(String::as_str).collect();
        parity::run_jar(&jar_refs)
    } else {
        let log = std::fs::read(parity::cli_reference(stem, "route.log"))
            .unwrap_or_else(|e| panic!("{stem}: cannot read route.log: {e}"));
        let code: i32 = std::fs::read_to_string(parity::cli_reference(stem, "route.exit"))
            .unwrap_or_else(|e| panic!("{stem}: cannot read route.exit: {e}"))
            .trim()
            .parse()
            .unwrap_or_else(|e| panic!("{stem}: route.exit is not a number: {e}"));
        // `route.log` is stdout then stderr, already concatenated by the generator, so the
        // second stream is empty here — `normalize_log`'s ERROR dedup works on the join either
        // way. See `crates/freerouting/tests/cli_e2e.rs::climb_one`, which reads it the same way.
        (log, Vec::new(), code)
    };

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
    let jar_ses = if against_jar {
        read(&jar_dir.join("route.ses"))
    } else {
        read(&parity::cli_reference(stem, "route.ses"))
    };
    let port_ses = read(&port_dir.join("route.ses"));
    match (jar_ses, port_ses) {
        (Some(jar), Some(port)) => {
            // A no-op on the committed golden — Task 2 regenerated it from the port at
            // `bd296d7`, so quirk #92's four keyword literals already carry the port's spelling.
            // Kept unconditionally so the two lanes run one comparison, not two.
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
///
/// # The golden for these rows lives here, not under `tests/reference/`
///
/// Converted with the rest of the driver at Plan 9's M1 accept wave (ruling BV). Four of the five
/// write no SES, so there is no `tests/reference/cli-*` directory to point at and inventing one
/// would commit four empty stems. The expectation is therefore a literal beside each case:
/// **the exit code and the `parity::normalize_log` projection, cut from the port**, and identical
/// to the jar's at the run taken immediately before the conversion (all four `MATCH`). They are
/// safe to pin that way because they are refusal paths — no board is routed, so R1 (#293) and R2
/// (#294) cannot reach them, which is also why they were the rows that stayed green while the four
/// routed stems went red. `--against-jar` still answers them with a live jar.
fn refusal_rows(scratch: &Path, against_jar: bool) -> Vec<Row> {
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

    // `(name, argv, expected exit, expected normalize_log lines)` — the last two are the
    // port-golden expectation the default lane compares against; see the section above.
    let cases: Vec<(&str, Vec<String>, i32, &[&str])> = vec![
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
            1,
            &["ERROR Freerouting.java:105", "WARN Freerouting.java:109"],
        ),
        // `Freerouting.java:81` — neither slot filled, `legacy::rewrite`'s refusal.
        (
            "no-files",
            argv(&["-mp", "1"]),
            1,
            &["ERROR Freerouting.java:81"],
        ),
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
            1,
            &[],
        ),
        // Plan ruling 7 / quirk #244: session bytes under a `.dsn` name reach
        // `RoutingJobState.INVALID`, where **the jar hangs for ever** — so under `--against-jar`
        // this row is expected to be an `XDIFF`, and the driver says so rather than waiting.
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
            0,
            &[
                "WARN GlobalSettings.java:562",
                "WARN GlobalSettings.java:562",
            ],
        ),
        (
            "invalid-input-java-hangs",
            argv(&[
                "-de",
                &ses_bytes_named_dsn.to_string_lossy(),
                "-do",
                &dir.join("c.ses").to_string_lossy(),
            ]),
            1,
            &[],
        ),
    ];

    cases
        .into_iter()
        .map(|(name, args, want_code, want_log)| {
            if name == "invalid-input-java-hangs" && against_jar {
                // Not run **under `--against-jar`**: `Freerouting.isCliTerminalState` omits
                // `INVALID` (`Freerouting.java:189-194`), so the jar sits in `:151-158`'s
                // `while (…) Thread.sleep(500)` at 0 % CPU with no output and no message. Running
                // it would hang this driver. The default port-golden lane has no jar to hang, so
                // there the row is measured like any other and answers `MATCH` on exit 1 — which
                // is the one row the conversion made *stronger*. The port's answer is also pinned
                // by `crates/freerouting/tests/cli_e2e.rs::de_a_ses_exits_1_instead_of_hanging`.
                return Row {
                    stem: name.to_string(),
                    verdict: "XDIFF",
                    detail: "the jar HANGS (quirk #244: INVALID is not in isCliTerminalState); \
                             the port exits 1 — plan ruling 7, not run here"
                        .to_string(),
                };
            }
            let refs: Vec<&str> = args.iter().map(String::as_str).collect();
            let (port_out, port_err, port_code) = parity::run_port(&refs);
            let port_log = parity::normalize_log(&port_out, &port_err);

            // The right-hand side: the literals above, or a live jar behind the escape hatch.
            let (want_code, want_log) = if against_jar {
                let (jar_out, jar_err, jar_code) = parity::run_jar(&refs);
                (jar_code, parity::normalize_log(&jar_out, &jar_err))
            } else {
                (want_code, {
                    let mut text = want_log.join("\n");
                    if !text.is_empty() {
                        text.push('\n');
                    }
                    text
                })
            };

            if want_code != port_code {
                return Row {
                    stem: name.to_string(),
                    verdict: "DIFF",
                    detail: format!("exit: expected {want_code}, port {port_code}"),
                };
            }
            if want_log != port_log {
                return Row {
                    stem: name.to_string(),
                    verdict: "DIFF",
                    detail: format!(
                        "log: expected {:?} port {:?}",
                        want_log.replace('\n', " | "),
                        port_log.replace('\n', " | ")
                    ),
                };
            }
            Row {
                stem: name.to_string(),
                verdict: "MATCH",
                detail: format!(
                    "exit {port_code}, log [{}]",
                    port_log.trim_end().replace('\n', " | ")
                ),
            }
        })
        .collect()
}

fn argv(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|part| (*part).to_string()).collect()
}
