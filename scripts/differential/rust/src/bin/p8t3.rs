//! `p8t3` — Plan 8 Task 7's two gates for `freerouting drc`.
//!
//! | mode | shape | what it pins |
//! |---|---|---|
//! | `merge` (default) | a genuine Java-vs-Rust pair; `run.sh` diffs the two transcripts | quirk #272 — the quality score's **separate** settings merge (`Freerouting.java:342-352`), field by field |
//! | `e2e` | Rust-only, the `p8t1` shape: two whole programs on the same argv | the whole `-drc` run — report bytes, exit code, log |
//!
//! # `merge`
//!
//! `P8T3.java` transcribes `initializeDrc`'s `:342-352` — the prototype merger
//! (`Freerouting.java:1408-1413`) plus **one** `DsnFileSettings` and nothing else, then
//! `board.getStatistics().getNormalizedScore(routerSettings.scoring)` — over the eight rows of
//! `tests/reference/drc-fixtures.txt`. This half calls
//! `freerouting::commands::drc::{quality_score_settings, quality_score}`, i.e. **the program's own
//! functions** (the `p8t5` convention), and prints the same three lines per stem: the seven
//! scoring weights, the six board counters, and the score as text and as raw IEEE bits.
//!
//! Printing the weights is the point. The score is one `float`; a divergence in it could come from
//! the merge, from `BoardStatistics`, or from the board the three loaders built, and a single
//! number cannot tell a reader which. Three lines can.
//!
//! # `e2e`
//!
//! Per stem: `java -jar <jar> -de <dsn> [<ses>] [-dr <rules>] -drc <report>` against the port on
//! the **same** argv, on three rungs —
//!
//! | rung | assertion |
//! |---|---|
//! | (a) | the two reports are byte-identical after [`parity::normalize_drc_json`] (which drops `date` and sorts each `unconnectedItems` entry's `items` by numeric uuid — quirk #144, plan-5 ruling 3) |
//! | (b) | the two exit codes are equal |
//! | (c) | [`parity::normalize_log`] of both sides is equal |
//!
//! Rung (a) needs the **jar's key spelling**, so the port is run a second time on the *native*
//! form with `--schema freerouting`: the shipped default is `DrcJsonFlavor::KiCad` (ruling W,
//! quirk #154) and `normalize_drc_json` is a `deny_unknown_fields` reader of the HEAD document.
//! Rungs (b) and (c) — and rung (d) below — use the **legacy** run, so the shim, the message set
//! and the exit ladder are all compared on the argv a user actually types.
//!
//! | rung | assertion |
//! |---|---|
//! | (d) | the legacy run's report is the **KiCad** spelling: it carries `quality_score` and not `qualityScore` — ruling W, observed through the binary |
//!
//! ## `drc-natural-tone-preamp` is an expected `XDIFF`, and always will be
//!
//! Quirk #146: `generateReport` folds `getAllUnconnectedItems`' `track_dangling` entries into
//! `violations` (`DesignRulesChecker.java:271-276`), and that phase's dedup (`:160`) drops
//! whichever dangling trace a net entry's **hash-ordered** `firstItem` happens to be. The jar's
//! own answer moves between `-XX:hashCode` modes (113-115 violations); `-XX:hashCode=2` pins it at
//! 115, which is the committed reference, and the port's ascending-id representatives (plan-5
//! ruling 3) give 112. **The jar does not match itself on this stem**, so no amount of porting
//! makes it a MATCH — the three extra entries are pinned by uuid, here and in
//! `crates/fr-drc/tests/reference_parity.rs`, and this driver reports the row as `XDIFF` with
//! those uuids rather than deleting them from the jar's side to manufacture agreement. Its
//! `quality_score` still matches exactly, and the driver says so.
//!
//! Like `p8t1`, the exit status is 0 iff every row is `MATCH` or `XDIFF`; a `DIFF` is a failure.
//!
//! # Usage
//!
//! ```text
//! scripts/differential/run.sh p8t3                 # merge, all eight stems (Java vs Rust)
//! scripts/differential/run.sh p8t3 e2e             # the whole-program gate, all eight stems
//! scripts/differential/run.sh p8t3 e2e <stem> …    # the named stems
//! ```

use std::path::{Path, PathBuf};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("merge") => merge_mode(&args[1..]),
        Some("e2e") => e2e_mode(&args[1..]),
        _ => {
            eprintln!("usage: p8t3 merge <drc-fixtures.txt> <java-dir>");
            eprintln!("       p8t3 e2e [stem...]");
            std::process::exit(2);
        }
    }
}

// =================================================================================================
// The fixture table
// =================================================================================================

/// One row of `tests/reference/drc-fixtures.txt`: `stem|dsn|rules|ses`, the last two optional and
/// every path relative to the Java checkout.
#[derive(Clone)]
struct Row {
    stem: String,
    dsn: String,
    rules: Option<String>,
    ses: Option<String>,
}

fn read_table(path: &Path) -> Vec<Row> {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let mut fields = line.split('|');
            let mut next = || fields.next().unwrap_or_default().trim().to_string();
            let (stem, dsn, rules, ses) = (next(), next(), next(), next());
            Row {
                stem,
                dsn,
                rules: (!rules.is_empty()).then_some(rules),
                ses: (!ses.is_empty()).then_some(ses),
            }
        })
        .collect()
}

// =================================================================================================
// `merge` — the Java-vs-Rust pair
// =================================================================================================

fn merge_mode(args: &[String]) {
    if args.len() < 2 {
        eprintln!("usage: p8t3 merge <drc-fixtures.txt> <java-dir>");
        std::process::exit(2);
    }
    let table = PathBuf::from(&args[0]);
    let java_dir = PathBuf::from(&args[1]);
    let rows = read_table(&table);

    println!("HEADER driver=p8t3 mode=merge stems={}", rows.len());
    for row in &rows {
        merge_row(&java_dir, row);
    }
}

fn merge_row(java_dir: &Path, row: &Row) {
    let dsn = java_dir.join(&row.dsn);

    // `Freerouting.java:260-264` — the job, and its input.
    let mut job = fr_core::RoutingJob::new(fr_core::SessionId::NIL);
    if job.set_input(&dsn).is_err() {
        println!("LOAD {} failed", row.stem);
        return;
    }
    // `:271` — the whole loader, with `job.router_settings` still `RouterSettings::new()`.
    let Ok(loaded) = fr_core::load_board_if_needed(&mut job) else {
        println!("LOAD {} failed", row.stem);
        return;
    };
    let mut board = loaded.board;
    let transform = loaded.transform;

    // `:277-329` — the `.rules` file, then the session, through the runner's own functions.
    let rules = row.rules.as_ref().map(|rules| java_dir.join(rules));
    freerouting::commands::drc::load_rules_file(rules.as_deref(), &job, &mut board, &transform);
    let ses = row.ses.as_ref().map(|ses| java_dir.join(ses));
    freerouting::commands::drc::load_session_file(ses.as_deref(), &mut board, &transform);

    // `:340` — the report runs **before** `:348`'s statistics and mutates the board (plan-5
    // ruling 8). The document itself is `e2e`'s business; here only its side effect matters.
    let coords = fr_drc::report::DrcCoordinates {
        board_unit: board.communication.unit,
        transform,
    };
    let options = fr_drc::report::DrcReportOptions {
        source: row.dsn.rsplit('/').next().unwrap_or(&row.dsn).to_string(),
        coordinate_unit: "mm".to_string(),
        date: "1970-01-01T00:00Z".to_string(),
        freerouting_version: fr_core::PARITY_VERSION.to_string(),
        quality_score: None,
    };
    let _ = fr_drc::DesignRulesChecker::new(&mut board).generate_report(&coords, &options);

    // `:344-347` — the sub-merge, and `:348-349`, both through the binary's own library.
    let input = job.get_input().expect("the input was assigned above");
    let settings = freerouting::commands::drc::quality_score_settings(input, &[]);
    let scoring = settings
        .scoring
        .as_ref()
        .expect("merge()'s validate() allocates scoring");
    println!(
        "MERGE {} viaCosts={} planeViaCosts={} startRipupCosts={} unroutedNetPenalty={}\
         \u{20}clearanceViolationPenalty={} bendPenalty={} defaultPreferredDirectionTraceCost={}\
         \u{20}defaultUndesiredDirectionTraceCost={} defaultBendCost={}",
        row.stem,
        boxed_i32(scoring.via_costs),
        boxed_i32(scoring.plane_via_costs),
        boxed_i32(scoring.start_ripup_costs),
        boxed_f32(scoring.unrouted_net_penalty),
        boxed_f32(scoring.clearance_violation_penalty),
        boxed_f32(scoring.bend_penalty),
        boxed_f64(scoring.default_preferred_direction_trace_cost),
        boxed_f64(scoring.default_undesired_direction_trace_cost),
        boxed_f64(scoring.default_bend_cost),
    );

    let stats = fr_core::BoardStatistics::new(&mut board);
    println!(
        "STATS {} maximumCount={} incompleteCount={} clearanceViolations={} bends={} vias={}\
         \u{20}traceLengthMm={}",
        row.stem,
        boxed_i32(stats.connections.maximum_count),
        boxed_i32(stats.connections.incomplete_count),
        boxed_i32(stats.clearance_violations.total_count),
        boxed_i32(stats.bends.total_count),
        boxed_i32(stats.vias.total_count),
        boxed_f32(stats.traces.total_length_mm),
    );

    let score = stats.normalized_score(scoring);
    println!(
        "SCORE {} float={} bits={} widened={}",
        row.stem,
        fr_dsn::format::double::java_float_to_string(score),
        score.to_bits() as i32,
        fr_dsn::format::double::java_double_to_string(f64::from(score)),
    );
}

/// A Java boxed `Integer`: `null` when absent, so the two transcripts agree on the null spelling.
fn boxed_i32(value: Option<i32>) -> String {
    value.map_or_else(|| "null".to_string(), |value| value.to_string())
}

/// A Java boxed `Float`, rendered by `Float.toString` (`fr_dsn`'s port, pinned by `p3t2`).
fn boxed_f32(value: Option<f32>) -> String {
    value.map_or_else(
        || "null".to_string(),
        fr_dsn::format::double::java_float_to_string,
    )
}

/// A Java boxed `Double`, rendered by `Double.toString`.
fn boxed_f64(value: Option<f64>) -> String {
    value.map_or_else(
        || "null".to_string(),
        fr_dsn::format::double::java_double_to_string,
    )
}

// =================================================================================================
// `e2e` — the two whole programs
// =================================================================================================

/// One stem's verdict.
struct Verdict {
    stem: String,
    verdict: &'static str,
    detail: String,
}

fn e2e_mode(args: &[String]) {
    let table = parity::workspace_root().join("tests/reference/drc-fixtures.txt");
    let all = read_table(&table);
    let rows: Vec<Row> = if args.is_empty() {
        all
    } else {
        args.iter()
            .map(|name| {
                all.iter()
                    .find(|row| &row.stem == name)
                    .unwrap_or_else(|| panic!("no such stem in drc-fixtures.txt: {name}"))
                    .clone()
            })
            .collect()
    };

    let scratch = std::env::temp_dir().join("p8t3");
    let _ = std::fs::remove_dir_all(&scratch);

    let mut verdicts: Vec<Verdict> = rows.iter().map(|row| compare(row, &scratch)).collect();
    // The exit ladder is only reachable from argv shapes, not from fixture stems — see
    // [`refusal_rows`]. Run them only for a whole-table run, so `p8t3 e2e <stem>` stays a
    // one-stem debugging tool.
    if args.is_empty() {
        verdicts.extend(refusal_rows(&scratch));
    }

    println!(
        "== p8t3 e2e: the jar and the port, two whole programs, {} stems",
        verdicts.len()
    );
    println!("{:<26} {:<7} {}", "stem", "verdict", "detail");
    for verdict in &verdicts {
        println!(
            "{:<26} {:<7} {}",
            verdict.stem, verdict.verdict, verdict.detail
        );
    }
    let matched = verdicts.iter().filter(|v| v.verdict == "MATCH").count();
    let xdiff = verdicts.iter().filter(|v| v.verdict == "XDIFF").count();
    println!(
        "rows: {}  MATCH: {}  XDIFF: {}  DIFF: {}",
        verdicts.len(),
        matched,
        xdiff,
        verdicts.len() - matched - xdiff
    );
    if matched + xdiff != verdicts.len() {
        std::process::exit(1);
    }
}

/// Quirk #146's three `track_dangling` entries on `drc-natural-tone-preamp`: each is the
/// `firstItem` of a net entry whose connected group holds no `Pin`, so the JVM's identity-hash
/// choice picks a different representative from the port's ascending-id one (plan-5 ruling 3) and
/// the trace phase's dedup (`DesignRulesChecker.java:160`) then drops it. Pinned by uuid, never by
/// count — the same trio `crates/fr-drc/tests/reference_parity.rs` names.
const NATURAL_TONE_PREAMP_EXTRA_DANGLING: [&str; 3] = ["1909", "1696", "1242"];

fn compare(row: &Row, scratch: &Path) -> Verdict {
    let dir = scratch.join(&row.stem);
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let java_dir = parity::java_dir();
    let dsn = java_dir.join(&row.dsn);
    let rules = row.rules.as_ref().map(|rules| java_dir.join(rules));
    let ses = row.ses.as_ref().map(|ses| java_dir.join(ses));

    let jar_report = dir.join("jar.json");
    let port_report = dir.join("port.json");
    let port_head_report = dir.join("port-head.json");

    // The legacy argv, byte for byte what `tests/reference/<stem>/drc.meta.txt` records: the
    // session travels in the `-de` slot list (`GlobalSettings.java:564-648`), because there is no
    // `-ds` flag anywhere in the parser.
    let jar_argv = legacy_argv(&dsn, ses.as_deref(), rules.as_deref(), &jar_report);
    let port_argv = legacy_argv(&dsn, ses.as_deref(), rules.as_deref(), &port_report);
    let jar_refs: Vec<&str> = jar_argv.iter().map(String::as_str).collect();
    let port_refs: Vec<&str> = port_argv.iter().map(String::as_str).collect();

    let (jar_out, jar_err, jar_code) = parity::run_jar(&jar_refs);
    let (port_out, port_err, port_code) = parity::run_port(&port_refs);

    // Rung (b) first: a wrong exit code explains a missing report.
    if jar_code != port_code {
        return Verdict {
            stem: row.stem.clone(),
            verdict: "DIFF",
            detail: format!(
                "exit: jar {jar_code}, port {port_code}; port stderr: {}",
                first_line(&port_err)
            ),
        };
    }

    // Rung (d): ruling W, observed through the binary. The legacy run took the shipped default.
    match std::fs::read_to_string(&port_report) {
        Ok(text) => {
            if !text.contains("\"quality_score\"") || text.contains("\"qualityScore\"") {
                return Verdict {
                    stem: row.stem.clone(),
                    verdict: "DIFF",
                    detail: "ruling W: the shipped default is not the KiCad spelling".to_string(),
                };
            }
        }
        Err(error) => {
            return Verdict {
                stem: row.stem.clone(),
                verdict: "DIFF",
                detail: format!("the port wrote no report: {error}"),
            };
        }
    }

    // The second port run, on the **native** form, for rung (a)'s byte comparison — see the
    // module docs for why the flavor has to be asked for.
    let head_argv = native_head_argv(&dsn, ses.as_deref(), rules.as_deref(), &port_head_report);
    let head_refs: Vec<&str> = head_argv.iter().map(String::as_str).collect();
    let (_, head_err, head_code) = parity::run_port(&head_refs);
    if head_code != 0 {
        return Verdict {
            stem: row.stem.clone(),
            verdict: "DIFF",
            detail: format!(
                "the port's --schema freerouting run exited {head_code}: {}",
                first_line(&head_err)
            ),
        };
    }

    // Rung (a).
    let jar_text = match std::fs::read_to_string(&jar_report) {
        Ok(text) => text,
        Err(error) => {
            return Verdict {
                stem: row.stem.clone(),
                verdict: "DIFF",
                detail: format!("the jar wrote no report: {error}"),
            };
        }
    };
    let port_text = std::fs::read_to_string(&port_head_report).expect("the second run wrote one");
    let mut jar_doc = match parity::parse_drc_json(&jar_text) {
        Ok(doc) => doc,
        Err(error) => {
            return Verdict {
                stem: row.stem.clone(),
                verdict: "DIFF",
                detail: format!("the jar's report is not a HEAD-flavor document: {error}"),
            };
        }
    };
    let jar_score = jar_doc.quality_score;
    let port_doc = match parity::parse_drc_json(&port_text) {
        Ok(doc) => doc,
        Err(error) => {
            return Verdict {
                stem: row.stem.clone(),
                verdict: "DIFF",
                detail: format!("the port's report is not a HEAD-flavor document: {error}"),
            };
        }
    };
    // **The score is the rung Task 7 newly computes**, so it is asserted on its own before the
    // document — a divergence there is a scoring bug, and a divergence in the rest is not.
    if jar_score != port_doc.quality_score {
        return Verdict {
            stem: row.stem.clone(),
            verdict: "DIFF",
            detail: format!(
                "quality_score: jar {jar_score:?}, port {:?}",
                port_doc.quality_score
            ),
        };
    }

    let jar_normalised = parity::normalize_drc_doc(&mut jar_doc).expect("the jar's doc re-renders");
    let port_normalised = parity::normalize_drc_json(&port_text).expect("the port's doc re-renders");
    if jar_normalised != port_normalised {
        // Quirk #146's known, un-portable divergence — see the module docs.
        if row.stem == "drc-natural-tone-preamp" {
            let extra = extra_dangling(&jar_text, &port_text);
            if extra == NATURAL_TONE_PREAMP_EXTRA_DANGLING {
                return Verdict {
                    stem: row.stem.clone(),
                    verdict: "XDIFF",
                    detail: format!(
                        "quirk #146: the jar's -XX:hashCode=2 run has 3 more `track_dangling` \
                         entries (uuids {}); quality_score {jar_score:?} MATCHES",
                        extra.join(", ")
                    ),
                };
            }
        }
        let at = jar_normalised
            .bytes()
            .zip(port_normalised.bytes())
            .position(|(a, b)| a != b)
            .unwrap_or_else(|| jar_normalised.len().min(port_normalised.len()));
        return Verdict {
            stem: row.stem.clone(),
            verdict: "DIFF",
            detail: format!(
                "report differs at byte {at} (jar {} B, port {} B)",
                jar_normalised.len(),
                port_normalised.len()
            ),
        };
    }

    // Rung (c).
    let jar_log = parity::normalize_log(&jar_out, &jar_err);
    let port_log = parity::normalize_log(&port_out, &port_err);
    if jar_log != port_log {
        return Verdict {
            stem: row.stem.clone(),
            verdict: "DIFF",
            detail: format!(
                "log: jar {:?} port {:?}",
                jar_log.replace('\n', " | "),
                port_log.replace('\n', " | ")
            ),
        };
    }

    Verdict {
        stem: row.stem.clone(),
        verdict: "MATCH",
        detail: format!(
            "exit {jar_code}, {} B report, quality_score {jar_score:?}, log {} lines",
            jar_normalised.len(),
            jar_log.lines().count()
        ),
    }
}

/// The eight stems' own command line: `-de <dsn> [<ses>] [-dr <rules>] -drc <report>`.
fn legacy_argv(dsn: &Path, ses: Option<&Path>, rules: Option<&Path>, report: &Path) -> Vec<String> {
    let mut argv = vec!["-de".to_string(), dsn.to_string_lossy().into_owned()];
    if let Some(ses) = ses {
        argv.push(ses.to_string_lossy().into_owned());
    }
    if let Some(rules) = rules {
        argv.push("-dr".to_string());
        argv.push(rules.to_string_lossy().into_owned());
    }
    argv.push("-drc".to_string());
    argv.push(report.to_string_lossy().into_owned());
    argv
}

/// The same three files on the **native** form, asking for HEAD's key spelling — the only way to
/// get the jar's own bytes out of the port (ruling W; see the module docs).
fn native_head_argv(
    dsn: &Path,
    ses: Option<&Path>,
    rules: Option<&Path>,
    report: &Path,
) -> Vec<String> {
    let mut argv = vec!["drc".to_string(), dsn.to_string_lossy().into_owned()];
    if let Some(ses) = ses {
        argv.push("--ses".to_string());
        argv.push(ses.to_string_lossy().into_owned());
    }
    if let Some(rules) = rules {
        argv.push("--rules".to_string());
        argv.push(rules.to_string_lossy().into_owned());
    }
    argv.push("-o".to_string());
    argv.push(report.to_string_lossy().into_owned());
    argv.push("--schema".to_string());
    argv.push("freerouting".to_string());
    argv
}

/// The uuids of the `track_dangling` entries the jar's document has and the port's does not, in
/// the jar's own order — the evidence for the quirk #146 row.
fn extra_dangling(jar_text: &str, port_text: &str) -> Vec<String> {
    let jar = parity::parse_drc_json(jar_text).expect("parsed above");
    let port = parity::parse_drc_json(port_text).expect("parsed above");
    let port_uuids: Vec<&str> = port
        .violations
        .iter()
        .filter(|violation| violation.kind == "track_dangling" && violation.items.len() == 1)
        .map(|violation| violation.items[0].uuid.as_str())
        .collect();
    jar.violations
        .iter()
        .filter(|violation| violation.kind == "track_dangling" && violation.items.len() == 1)
        .map(|violation| violation.items[0].uuid.clone())
        .filter(|uuid| !port_uuids.contains(&uuid.as_str()))
        .collect()
}

/// The five argv shapes that reach `initializeDrc`'s exit ladder — **quirk #271, measured rather
/// than argued**.
///
/// The eight fixture stems all succeed, so on their own they prove nothing about the three
/// `System.exit(1)` sites or about the two failures that deliberately do *not* stop the run. Each
/// row below reaches exactly one of those five arms, and each is compared on the exit code, on
/// `parity::normalize_log`, and on whether a report was written:
///
/// | row | Java | expected |
/// |---|---|---|
/// | `missing-rules` | `:289` `FRLogger.warn`, run continues | exit **0**, report written |
/// | `missing-session` | `:324` `FRLogger.warn`, run continues | exit **0**, report written |
/// | `missing-input` | `:266` + `:267` `System.exit(1)` | exit **1**, no report |
/// | `ses-input` | `:272` + `:273` `System.exit(1)` — quirk #274, label S | exit **1**, no report |
/// | `unwritable-report` | `:365` + `:366` `System.exit(1)` | exit **1**, no report |
///
/// `ses-input` is quirk #274's whole point: `-de session.dsn` is accepted by the argument parser
/// (the `-de` classifier goes by **extension**, `GlobalSettings.java:564-648`), sniffed as `SES`
/// by `RoutingJob.setInput` (`RoutingJob.java:431`, by **bytes**), and refused only inside
/// `BoardLoader` (`:31-37`). Both programs print `Cannot load board: only DSN and JSON formats
/// are supported, got SES` — measured, and identical — though `normalize_log` drops it, because
/// it is a `BoardLoader` message and `MESSAGE_MAP` carries only `Freerouting.java`'s.
fn refusal_rows(scratch: &Path) -> Vec<Verdict> {
    let dir = scratch.join("refusals");
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let dsn = parity::fixture("Issue575-drc_dev-board_4_hole_clearance_violations.dsn");
    // Session bytes under a `.dsn` name: the name decides the slot, the bytes decide the format.
    let session_named_dsn = dir.join("session.dsn");
    std::fs::write(&session_named_dsn, b"(session previous)\n").expect("write the fixture");

    let cases: Vec<(&str, bool, Vec<String>)> = vec![
        (
            "missing-rules",
            true,
            argv(&[
                "-de",
                &dsn.to_string_lossy(),
                "-dr",
                &dir.join("nosuch.rules").to_string_lossy(),
                "-drc",
                &dir.join("missing-rules.json").to_string_lossy(),
            ]),
        ),
        (
            "missing-session",
            true,
            argv(&[
                "-de",
                &dsn.to_string_lossy(),
                &dir.join("nosuch.ses").to_string_lossy(),
                "-drc",
                &dir.join("missing-session.json").to_string_lossy(),
            ]),
        ),
        (
            "missing-input",
            false,
            argv(&[
                "-de",
                &dir.join("nosuch.dsn").to_string_lossy(),
                "-drc",
                &dir.join("missing-input.json").to_string_lossy(),
            ]),
        ),
        (
            "ses-input",
            false,
            argv(&[
                "-de",
                &session_named_dsn.to_string_lossy(),
                "-drc",
                &dir.join("ses-input.json").to_string_lossy(),
            ]),
        ),
        // The three-file run: `-de <dsn> <ses> -dr <rules> -drc <report>`, the **only** argv shape
        // that exercises quirk #273's whole load order (DSN → `.rules` → session) in one run. No
        // fixture stem does — `drc-issue593-rules` and `drc-issue593-ses` each fill one optional
        // slot — and the order is Java's, so the assertion that matters is that the two programs
        // build the same board from the same three files. Compared on the report bytes as well as
        // on the exit code and the log, by the `report_matches` flag below.
        (
            "rules-and-session",
            true,
            argv(&[
                "-de",
                &parity::fixture("Issue593-BBD_Mars-64.dsn").to_string_lossy(),
                &parity::fixture("Issue593-BBD_Mars-64.ses").to_string_lossy(),
                "-dr",
                &parity::fixture("Issue593-BBD_Mars-64.rules").to_string_lossy(),
                "-drc",
                &dir.join("rules-and-session.json").to_string_lossy(),
            ]),
        ),
        (
            "unwritable-report",
            false,
            argv(&[
                "-de",
                &dsn.to_string_lossy(),
                "-drc",
                &dir.join("nodir").join("unwritable.json").to_string_lossy(),
            ]),
        ),
    ];

    cases
        .into_iter()
        .map(|(name, expect_report, args)| {
            // One report path per row, so the two programs cannot see each other's file: each is
            // run into its own subdirectory with the same relative name.
            let jar_dir = dir.join(format!("{name}-jar"));
            let port_dir = dir.join(format!("{name}-port"));
            for target in [&jar_dir, &port_dir] {
                std::fs::create_dir_all(target).expect("a scratch directory");
            }
            let jar_args = retarget(&args, &dir, &jar_dir);
            let port_args = retarget(&args, &dir, &port_dir);
            let jar_refs: Vec<&str> = jar_args.iter().map(String::as_str).collect();
            let port_refs: Vec<&str> = port_args.iter().map(String::as_str).collect();

            let (jar_out, jar_err, jar_code) = parity::run_jar(&jar_refs);
            let (port_out, port_err, port_code) = parity::run_port(&port_refs);
            if jar_code != port_code {
                return Verdict {
                    stem: name.to_string(),
                    verdict: "DIFF",
                    detail: format!("exit: jar {jar_code}, port {port_code}"),
                };
            }
            let report = jar_args.last().expect("the argv ends in the report path");
            let jar_wrote = Path::new(report).is_file();
            let port_wrote = Path::new(port_args.last().expect("ditto")).is_file();
            if jar_wrote != port_wrote || jar_wrote != expect_report {
                return Verdict {
                    stem: name.to_string(),
                    verdict: "DIFF",
                    detail: format!(
                        "report written: jar {jar_wrote}, port {port_wrote}, expected \
                         {expect_report}"
                    ),
                };
            }
            let jar_log = parity::normalize_log(&jar_out, &jar_err);
            let port_log = parity::normalize_log(&port_out, &port_err);
            if jar_log != port_log {
                return Verdict {
                    stem: name.to_string(),
                    verdict: "DIFF",
                    detail: format!(
                        "log: jar {:?} port {:?}",
                        jar_log.replace('\n', " | "),
                        port_log.replace('\n', " | ")
                    ),
                };
            }
            // The report bytes, for the rows that wrote one. The port's legacy run is in the
            // **KiCad** spelling (ruling W), so it is re-run on the native form with
            // `--schema freerouting` — the same two-run shape [`compare`] uses, and for the same
            // reason.
            let mut report_bytes = String::new();
            if jar_wrote {
                let head = port_dir.join("head.json");
                let mut native = vec!["drc".to_string()];
                native.extend(native_from_legacy(&port_args));
                native.push("-o".to_string());
                native.push(head.to_string_lossy().into_owned());
                native.push("--schema".to_string());
                native.push("freerouting".to_string());
                let native_refs: Vec<&str> = native.iter().map(String::as_str).collect();
                let (_, _, code) = parity::run_port(&native_refs);
                if code != 0 {
                    return Verdict {
                        stem: name.to_string(),
                        verdict: "DIFF",
                        detail: format!("the port's --schema freerouting run exited {code}"),
                    };
                }
                let jar_text = std::fs::read_to_string(report).expect("the jar wrote one");
                let port_text = std::fs::read_to_string(&head).expect("the port wrote one");
                let jar_normalised =
                    parity::normalize_drc_json(&jar_text).expect("the jar's doc parses");
                let port_normalised =
                    parity::normalize_drc_json(&port_text).expect("the port's doc parses");
                if jar_normalised != port_normalised {
                    return Verdict {
                        stem: name.to_string(),
                        verdict: "DIFF",
                        detail: format!(
                            "report differs (jar {} B, port {} B)",
                            jar_normalised.len(),
                            port_normalised.len()
                        ),
                    };
                }
                report_bytes = format!(", {} B report", jar_normalised.len());
            }
            Verdict {
                stem: name.to_string(),
                verdict: "MATCH",
                detail: format!(
                    "exit {jar_code}, report {}{report_bytes}, log [{}]",
                    if jar_wrote { "written" } else { "absent" },
                    jar_log.trim_end().replace('\n', " | ")
                ),
            }
        })
        .collect()
}

/// The `-de <dsn> [<ses>]` / `-dr <rules>` half of a legacy argv, re-spelled for the native
/// subcommand form — everything but the report path and the schema flag, which the caller adds.
///
/// `legacy::rewrite` performs the same mapping inside the binary; this is the driver's own copy
/// because it needs the *native* argv to reach a port-only flag the shim cannot carry (ruling AR).
fn native_from_legacy(args: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-de" => {
                out.push(args[i + 1].clone());
                // A second `-de` argument is the session slot (`GlobalSettings.java:609-621`).
                // The driver package is edition 2021 (see its `Cargo.toml`), so this is a nested
                // `if` rather than a let-chain.
                if let Some(next) = args.get(i + 2) {
                    if !next.starts_with('-') {
                        out.push("--ses".to_string());
                        out.push(next.clone());
                        i += 1;
                    }
                }
                i += 2;
            }
            "-dr" => {
                out.push("--rules".to_string());
                out.push(args[i + 1].clone());
                i += 2;
            }
            "-drc" => i += 2,
            _ => i += 1,
        }
    }
    out
}

fn argv(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|part| (*part).to_string()).collect()
}

/// The same argv with the report directory swapped, so the two programs write to their own trees.
fn retarget(args: &[String], from: &Path, to: &Path) -> Vec<String> {
    let from = from.to_string_lossy().into_owned();
    let to = to.to_string_lossy().into_owned();
    args.iter()
        .map(|arg| {
            if arg.ends_with(".json") {
                arg.replace(&from, &to)
            } else {
                arg.clone()
            }
        })
        .collect()
}

fn first_line(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .lines()
        .next()
        .unwrap_or("")
        .to_string()
}
