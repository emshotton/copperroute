//! `p8t7` — Plan 8 Task 12's **KiCad end-to-end acceptance of spec §1**.
//!
//! Spec §1 is the port's reason to exist: *a KiCad-exported board goes in, a session KiCad can
//! import comes out.* Every other driver in this directory proves a piece of that; this one
//! proves the sentence, on three rungs.
//!
//! | rung | what |
//! |---|---|
//! | (a) | a **KiCad-exported DSN** routed to a SES that is byte-identical to `tests/reference/cli-router-ecc83-input/route.ses` **and that `fr_dsn::ses_reader::read` reads back without error** |
//! | (b) | Task 9's `-de board.json -do out.ses`: the same physical board as a KiCad *design* JSON, through the port's own JSON reader, against `tests/reference/cli-kicad-ecc83-json/route.ses` |
//! | (c) | Task 10's quirk **T**: `-do out.json` writes the board **as loaded**, so the document does not depend on how many passes ran — measured on **both** programs |
//!
//! # Rungs (a) and (b) compare against the committed golden (Plan 9 M1 accept wave, ruling BV)
//!
//! They compared against a **live jar** until Plan 9 Task 2 fixed two measured Java regressions —
//! R1 (#293), the deleted shortest-airline-first work-list ordering, and R2 (#294), the
//! micro-neckdown fanout fallback that ignored the board's minimum track width. The port now routes
//! this board deliberately differently, so both rungs went `DIFF` (@1755 and @2335) and stayed
//! there. Survey §7.3's transition applies: the **jar arm is retired from the default lane, not
//! deleted** — `run.sh --against-jar` exports `AGAINST_JAR=1` and puts it back — and the default
//! right-hand side is `tests/reference/cli-<stem>/{route.ses,route.exit}`, which Task 2 regenerated
//! from the port at `bd296d7`.
//!
//! **Rung (c) keeps its live jar in both lanes, deliberately.** What it measures is a *jar* quirk
//! (label T, register #289) on **both** programs at two pass counts; a committed golden cannot
//! express "the jar and the port answer the same document", so retiring the jar there would retire
//! the measurement rather than move it. It is also not affected by the conversion's cause: it was
//! `MATCH` before R1/R2 and it is `MATCH` after. Plan 9 Task 3 rewrites this rung for #289, at
//! which point it becomes an `XDIFF` with the divergence named.
//!
//! # Why rung (a) reads its own output back
//!
//! Every byte comparison in this suite asks "does the port write what the jar writes". None of
//! them asks "is what the port writes a document the port can read", and the two are not the same
//! question: a writer and a reader that are wrong in the same direction agree with each other. The
//! SES the port produced is therefore fed to [`fr_dsn::ses_reader::read`] against a freshly loaded
//! copy of the same board, and the import is required to place the wires and vias the SES claims
//! with **zero** `errors_encountered` — `SesReader` counts a scope it could not use rather than
//! failing (see its own docs), so a non-zero count is a silently half-read session.
//!
//! # Why there is no `P8T7.java`
//!
//! `p8t1`'s reason, unchanged: what is under test is a **whole program**, run on the argv of a
//! `tests/reference/cli-*` stem, and a Java class could only re-implement `normalize_log` and
//! `normalize_ses_head_tokens` a second time in a second language. `rust_only=1` in `run.sh`.
//!
//! ```text
//! scripts/differential/run.sh p8t7                 # rungs (a)/(b) port vs golden, (c) both programs
//! scripts/differential/run.sh --against-jar p8t7   # the retired arm: (a)/(b) against a live jar
//! ```

use std::path::{Path, PathBuf};

/// One rung's verdict.
struct Row {
    rung: &'static str,
    verdict: &'static str,
    detail: String,
}

/// `run.sh` exports `AGAINST_JAR=1` for `--against-jar`; anything else is the default lane, where
/// rungs (a) and (b) compare against the committed golden. Rung (c) does not read it — see the
/// header for why it keeps its live jar in both lanes.
fn against_jar() -> bool {
    std::env::var("AGAINST_JAR").is_ok_and(|value| value == "1")
}

fn main() {
    let scratch = std::env::temp_dir().join("p8t7");
    let _ = std::fs::remove_dir_all(&scratch);

    let against_jar = against_jar();
    let mut rows = Vec::new();
    rows.push(rung_a(&scratch, against_jar));
    rows.push(rung_b(&scratch, against_jar));
    rows.push(rung_c(&scratch));

    println!("== p8t7: spec §1 end to end — a KiCad board in, a session KiCad can import out");
    if against_jar {
        println!(
            "   --against-jar (the retired arm): rungs (a)/(b) run a live jar. R1 (#293) and R2 \
             (#294) make the port route this board differently, so both are EXPECTED to DIFF."
        );
    }
    println!("{:<28} {:<7} {}", "rung", "verdict", "detail");
    for row in &rows {
        println!("{:<28} {:<7} {}", row.rung, row.verdict, row.detail);
    }
    let matched = rows.iter().filter(|r| r.verdict == "MATCH").count();
    println!("rungs: {}  MATCH: {}  DIFF: {}", rows.len(), matched, rows.len() - matched);
    if matched != rows.len() {
        std::process::exit(1);
    }
}

/// A scratch directory per rung, so two rungs cannot see each other's `route.ses`.
fn dir(scratch: &Path, name: &str) -> PathBuf {
    let dir = scratch.join(name);
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir
}

/// One `cli-fixtures.txt` stem through the port: the SES bytes and the exit code, against the
/// committed golden — or, behind `--against-jar`, against a live jar run on the same argv.
///
/// The reference side is normalised by [`parity::normalize_ses_head_tokens`] — quirk #92's four
/// `(parser …)` keyword literals, the same rewrite `p8t1` and `cli_e2e.rs` apply, and the **only**
/// one either side gets. It is a no-op on the committed golden, which Task 2 cut from the port at
/// `bd296d7`; it is applied unconditionally so the two lanes run one comparison rather than two.
fn route_both(stem: &str, scratch: &Path, against_jar: bool) -> Result<(String, PathBuf), String> {
    let port_dir = dir(scratch, &format!("{stem}-port"));
    let port_argv = parity::cli_argv(stem, &port_dir);
    let port_refs: Vec<&str> = port_argv.iter().map(String::as_str).collect();
    let (_, port_err, port_code) = parity::run_port(&port_refs);

    let (want_code, want_ses, side) = if against_jar {
        let jar_dir = dir(scratch, &format!("{stem}-jar"));
        let jar_argv = parity::cli_argv(stem, &jar_dir);
        let jar_refs: Vec<&str> = jar_argv.iter().map(String::as_str).collect();
        let (_, _, jar_code) = parity::run_jar(&jar_refs);
        let jar_ses = std::fs::read_to_string(jar_dir.join("route.ses"))
            .map_err(|e| format!("the jar wrote no route.ses: {e}"))?;
        (jar_code, jar_ses, "jar")
    } else {
        let code: i32 = std::fs::read_to_string(parity::cli_reference(stem, "route.exit"))
            .map_err(|e| format!("route.exit: {e}"))?
            .trim()
            .parse()
            .map_err(|e| format!("route.exit is not a number: {e}"))?;
        let ses = std::fs::read_to_string(parity::cli_reference(stem, "route.ses"))
            .map_err(|e| format!("route.ses: {e}"))?;
        (code, ses, "golden")
    };

    if want_code != port_code {
        return Err(format!(
            "exit {port_code} != the {side}'s {want_code}: {}",
            String::from_utf8_lossy(&port_err).lines().last().unwrap_or_default()
        ));
    }

    let want_ses = parity::normalize_ses_head_tokens(&want_ses);
    let port_path = port_dir.join("route.ses");
    let port_ses = std::fs::read_to_string(&port_path)
        .map_err(|e| format!("the port wrote no route.ses: {e}"))?;
    if port_ses != want_ses {
        let at = port_ses
            .bytes()
            .zip(want_ses.bytes())
            .position(|(a, b)| a != b)
            .unwrap_or_else(|| port_ses.len().min(want_ses.len()));
        return Err(format!(
            "SES differs at byte {at} (port {} B, {side} {} B)",
            port_ses.len(),
            want_ses.len()
        ));
    }
    Ok((port_ses, port_path))
}

/// Reads a SES back against a fresh copy of the board it was routed from.
///
/// The board comes through [`fr_core::load_board_if_needed`], which is the loader the CLI and the
/// MCP both use, so the [`fr_dsn::CoordinateTransform`] the reader is given is the one
/// `Structure.createBoard` built — the only transform that round-trips a file's coordinates
/// unchanged (Plan 3 ruling A).
fn read_back(board_source: &Path, ses: &str) -> Result<String, String> {
    let mut job = fr_core::RoutingJob::new(fr_core::SessionId::NIL);
    job.set_input(board_source)
        .map_err(|e| format!("cannot re-read {}: {e}", board_source.display()))?;
    let loaded = fr_core::load_board_if_needed(&mut job)
        .map_err(|e| format!("cannot re-load {}: {e}", board_source.display()))?;
    let mut board = loaded.board;
    let summary = fr_dsn::ses_reader::read(ses.as_bytes(), &mut board, &loaded.transform)
        .map_err(|e| format!("the port cannot read its own SES: {e}"))?;
    if summary.errors_encountered != 0 {
        return Err(format!(
            "the port's own SES imported with {} error(s) — a silently half-read session",
            summary.errors_encountered
        ));
    }
    Ok(format!(
        "{} wires, {} vias imported, 0 errors",
        summary.wires_imported, summary.vias_imported
    ))
}

/// Rung (a): the KiCad-exported **DSN**.
///
/// `router-ecc83-input` is `fixtures/Issue649-kicad_ecc83-pp_input_board_v1.dsn`, the smallest
/// KiCad export in the corpus, with a `(plane …)` net and a copper pour on a signal layer.
fn rung_a(scratch: &Path, against_jar: bool) -> Row {
    let stem = "router-ecc83-input";
    match route_both(stem, scratch, against_jar) {
        Err(detail) => Row {
            rung: "a: KiCad DSN -> SES",
            verdict: "DIFF",
            detail,
        },
        Ok((ses, _)) => {
            let source = parity::java_dir().join("fixtures/Issue649-kicad_ecc83-pp_input_board_v1.dsn");
            match read_back(&source, &ses) {
                Err(detail) => Row {
                    rung: "a: KiCad DSN -> SES",
                    verdict: "DIFF",
                    detail,
                },
                Ok(summary) => Row {
                    rung: "a: KiCad DSN -> SES",
                    verdict: "MATCH",
                    detail: format!(
                        "{} B, byte-identical to the {}; read back: {summary}",
                        ses.len(),
                        if against_jar { "jar" } else { "golden" }
                    ),
                },
            }
        }
    }
}

/// Rung (b): Task 9's `-de board.json -do out.ses` — the same physical board as a KiCad *design*
/// JSON, through `fr_dsn::kicad`'s reader.
fn rung_b(scratch: &Path, against_jar: bool) -> Row {
    let stem = "kicad-ecc83-json";
    match route_both(stem, scratch, against_jar) {
        Err(detail) => Row {
            rung: "b: KiCad JSON -> SES",
            verdict: "DIFF",
            detail,
        },
        Ok((ses, _)) => {
            let source = parity::java_dir().join("fixtures/Issue649-kicad_ecc83-pp_input_board_v1.json");
            match read_back(&source, &ses) {
                Err(detail) => Row {
                    rung: "b: KiCad JSON -> SES",
                    verdict: "DIFF",
                    detail,
                },
                Ok(summary) => Row {
                    rung: "b: KiCad JSON -> SES",
                    verdict: "MATCH",
                    detail: format!(
                        "{} B, byte-identical to the {}; read back: {summary}",
                        ses.len(),
                        if against_jar { "jar" } else { "golden" }
                    ),
                },
            }
        }
    }
}

/// Rung (c): **quirk T** (register #289), measured on both programs.
///
/// `-do out.json` on a board input takes the KiCad-session-JSON path, where `setJobOutput` is both
/// a board-updated listener (`RoutingJobSchedulerActionThread.java:100`) and a once-only call
/// after `pipeline.run()` (`:168`) — and only the **first** of those ever writes, because
/// `output.setData` re-sniffs the bytes and a document starting `{` re-detects as
/// `KICAD_DESIGN_JSON`, after which neither `:275` nor `:282` matches again. So the file holds the
/// board **as loaded**, before any routing, and does not depend on how many passes ran.
///
/// Task 10 measured that on the jar. This rung measures it on **both**, at two pass counts, and
/// requires all four documents to be the same bytes — which is the strongest form of the claim: if
/// the port ever started writing the routed board, or the jar stopped, one of the four moves.
///
/// **This rung keeps its live jar in both lanes** (Plan 9 M1 accept wave, ruling BV). The claim is
/// about the two programs agreeing on a *jar* quirk; a committed golden cannot express it, so
/// retiring the jar here would retire the measurement rather than move it. R1/R2 do not reach it —
/// it was `MATCH` before them and is `MATCH` after. Plan 9 Task 3 rewrites it for #289.
fn rung_c(scratch: &Path) -> Row {
    let dsn = parity::java_dir().join("fixtures/Issue649-kicad_ecc83-pp_input_board_v1.dsn");
    let dir = dir(scratch, "quirk-t");
    let mut documents = Vec::new();
    for passes in ["1", "8"] {
        for (side, jar) in [("jar", true), ("port", false)] {
            let out = dir.join(format!("{side}-{passes}.json"));
            let argv: Vec<String> = vec![
                "-de".into(),
                dsn.display().to_string(),
                "-do".into(),
                out.display().to_string(),
                "-mp".into(),
                passes.into(),
                "--router.fanout.enabled=true".into(),
                "--router.optimizer.enabled=true".into(),
            ];
            let refs: Vec<&str> = argv.iter().map(String::as_str).collect();
            let (_, err, code) = if jar {
                parity::run_jar(&refs)
            } else {
                parity::run_port(&refs)
            };
            if code != 0 {
                return Row {
                    rung: "c: quirk T (-do out.json)",
                    verdict: "DIFF",
                    detail: format!(
                        "{side} -mp {passes} exited {code}: {}",
                        String::from_utf8_lossy(&err).lines().last().unwrap_or_default()
                    ),
                };
            }
            match std::fs::read_to_string(&out) {
                Ok(text) => documents.push((format!("{side} -mp {passes}"), text)),
                Err(e) => {
                    return Row {
                        rung: "c: quirk T (-do out.json)",
                        verdict: "DIFF",
                        detail: format!("{side} -mp {passes} wrote nothing: {e}"),
                    };
                }
            }
        }
    }
    let (first_label, first) = &documents[0];
    for (label, text) in &documents[1..] {
        if text != first {
            return Row {
                rung: "c: quirk T (-do out.json)",
                verdict: "DIFF",
                detail: format!(
                    "{label} ({} B) differs from {first_label} ({} B)",
                    text.len(),
                    first.len()
                ),
            };
        }
    }
    // The board as **loaded** carries no trace the router produced. The same argv with `-do
    // out.ses` answers a session with wires, which is what makes this a quirk rather than an
    // empty board.
    let traces = first.matches("\"traces\"").count();
    Row {
        rung: "c: quirk T (-do out.json)",
        verdict: "MATCH",
        detail: format!(
            "4 documents (jar/port x -mp 1/8) byte-identical, {} B, {traces} traces key",
            first.len()
        ),
    }
}
