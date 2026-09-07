#![forbid(unsafe_code)]

//! Test-suite helpers: locate the board corpus and the committed reference outputs, compare
//! text outputs modulo whitespace, and read and write the reference document formats.

use similar::{ChangeTag, TextDiff};
use std::path::{Path, PathBuf};

pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .to_path_buf()
}

/// `tests/corpus`: the boards the tests read.
pub fn corpus_dir() -> PathBuf {
    workspace_root().join("tests").join("corpus")
}

pub fn fixture(name: &str) -> PathBuf {
    corpus_dir().join("fixtures").join(name)
}

pub fn example(name: &str) -> PathBuf {
    corpus_dir().join("examples").join(name)
}

/// The `designName` the DSN/SES writers stamp into `(pcb "…")` and `(session "…")`: the file
/// name with a trailing `.dsn` removed, and the empty string for a file that is not a `.dsn`.
pub fn dsn_design_name(path: &Path) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .strip_suffix(".dsn")
        .unwrap_or_default()
        .to_string()
}

pub fn reference(stem: &str, file: &str) -> PathBuf {
    workspace_root()
        .join("tests")
        .join("reference")
        .join(stem)
        .join(file)
}

/// Canonicalises a text output for comparison: CRLF → LF, trailing **spaces and tabs**
/// stripped from every line, runs of blank lines collapsed to one, and exactly one trailing
/// newline.
///
/// Deliberately *not* `trim_end()`: that strips every Unicode whitespace character, including
/// U+00A0, U+2028 and the vertical tab. Those can legitimately appear inside a Specctra DSN
/// string literal (component names, comments), so treating them as insignificant trailing
/// noise could hide a real difference. Only the two characters an editor or a line-ending
/// conversion actually introduces are removed.
pub fn normalize_whitespace(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut blank_run = 0usize;
    for line in s.replace("\r\n", "\n").split('\n') {
        let t = line.trim_end_matches([' ', '\t']);
        if t.is_empty() {
            blank_run += 1;
            if blank_run > 1 {
                continue;
            }
        } else {
            blank_run = 0;
        }
        out.push_str(t);
        out.push('\n');
    }
    while out.ends_with("\n\n") {
        out.pop();
    }
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// Returns false and prints a skip message when a reference file is absent.
pub fn require_reference(path: &Path) -> bool {
    if path.exists() {
        true
    } else {
        eprintln!(
            "SKIP: reference {} missing — cut it with COPPERROUTE_REGOLDEN=<label>",
            path.display()
        );
        false
    }
}

pub fn assert_text_parity(actual: &str, reference_path: &Path) {
    let expected = std::fs::read_to_string(reference_path)
        .unwrap_or_else(|e| panic!("cannot read reference {}: {e}", reference_path.display()));
    let a = normalize_whitespace(actual);
    let e = normalize_whitespace(&expected);
    if a == e {
        return;
    }
    if let Ok(rel) = reference_path.strip_prefix(workspace_root().join("tests").join("reference")) {
        let scratch = workspace_root()
            .join("tests")
            .join("reference")
            .join("_scratch")
            .join(rel);
        let _ = std::fs::create_dir_all(scratch.parent().unwrap());
        let _ = std::fs::write(&scratch, &a);
    }
    let diff = TextDiff::from_lines(&e, &a);
    let mut msg = format!("parity mismatch vs {}\n", reference_path.display());
    let mut shown = 0;
    for change in diff.iter_all_changes() {
        let sign = match change.tag() {
            ChangeTag::Delete => "-",
            ChangeTag::Insert => "+",
            ChangeTag::Equal => continue,
        };
        msg.push_str(sign);
        msg.push_str(change.value());
        shown += 1;
        if shown > 200 {
            msg.push_str("… (diff truncated)\n");
            break;
        }
    }
    panic!("{msg}");
}

// =============================================================================================
// The KiCad DRC report
// =============================================================================================

/// The DRC report reduced to what a comparison can assert on: `date` is dropped, and every
/// `unconnected_items` entry's `items` array is sorted by numeric uuid. `violations` is left
/// untouched, array order included, because its order is part of what the references pin.
///
/// The document is re-emitted from [`DrcReportDoc`], so the key order and number spelling of
/// the input do not survive; `deny_unknown_fields` makes an unexpected key a loud failure.
///
/// # Errors
///
/// Any `serde_json` parse failure, including an unknown or missing key.
pub fn normalize_drc_json(s: &str) -> Result<String, serde_json::Error> {
    normalize_drc_doc(&mut parse_drc_json(s)?)
}

/// Parses a DRC report. Split out of [`normalize_drc_json`] so a caller that has to edit the
/// document before comparing can do it on typed data and still compare the normalised bytes.
///
/// # Errors
///
/// Any `serde_json` parse failure, including an unknown or missing key.
pub fn parse_drc_json(s: &str) -> Result<DrcReportDoc, serde_json::Error> {
    serde_json::from_str(s)
}

/// Applies the rules above to an already-parsed report and renders it.
///
/// # Errors
///
/// Any `serde_json` serialisation failure.
pub fn normalize_drc_doc(doc: &mut DrcReportDoc) -> Result<String, serde_json::Error> {
    // Rule 1: `date` is never serialised (see [`DrcReportDoc::date`]).
    // Rule 2.
    for entry in &mut doc.unconnected_items {
        entry.items.sort_by_key(|item| {
            item.uuid
                .parse::<i64>()
                .unwrap_or_else(|_| panic!("uuid {} is not a number", item.uuid))
        });
    }
    // Rule 3: `violations` is untouched.
    serde_json::to_string_pretty(doc)
}

/// The report's fields in the order the writer emits them. Deliberately not
/// `copper_drc::KiCadDrcReport`: this crate sits below every crate under test.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DrcReportDoc {
    #[serde(rename = "$schema")]
    pub schema: String,
    pub coordinate_units: String,
    /// Read so `deny_unknown_fields` accepts a real report, then never serialised.
    #[serde(default, skip_serializing)]
    pub date: Option<String>,
    pub kicad_version: String,
    pub copperroute_version: String,
    pub source: String,
    pub unconnected_items: Vec<DrcViolationDoc>,
    pub violations: Vec<DrcViolationDoc>,
    pub schematic_parity: Vec<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality_score: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DrcViolationDoc {
    pub description: String,
    pub items: Vec<DrcViolationItemDoc>,
    pub severity: String,
    #[serde(rename = "type")]
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DrcViolationItemDoc {
    pub description: String,
    pub pos: DrcPositionDoc,
    /// The board-unique item id rendered as text, which is why it is sorted numerically.
    pub uuid: String,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DrcPositionDoc {
    pub x: f64,
    pub y: f64,
}

// =================================================================================================
// The router reference
// =================================================================================================
//
// `tests/reference/<stem>/router.jsonl` is one JSON line per connection. The types below are
// that line, in field order, so that `crates/copper-router/tests/reference_parity.rs` can compare
// a reference rung by rung against what the router produces rather than diffing two strings.
//
// Coordinates are `String`s: an integer corner is rendered `"(x,y)"` and a rational one
// `"~(x,y)"`, and `traceLength` likewise, so no re-parse can round a value into agreement.

/// One connection of a `router.jsonl` reference.
///
/// Every field after `state` is `#[serde(default)]` because a `"GONE"` line — a connection whose
/// item an earlier connection ripped up — carries only the first four.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouterConnectionDoc {
    /// 1-based index into the driver's connection list.
    pub k: usize,
    /// The board item id the connection starts from.
    pub item: i64,
    /// The net number routed.
    pub net: i32,
    /// The `AutorouteAttemptState` name, or `"GONE"` for a connection an earlier one ripped up.
    pub state: String,
    /// `AutorouteAttemptResult.details`, `""` when there are none.
    #[serde(default)]
    pub details: String,
    /// The ripped-item id set, in descending id order.
    #[serde(default)]
    pub ripped: Vec<i64>,
    /// `(item id, cost)` pairs, sorted by item id.
    #[serde(rename = "ripupCosts", default)]
    pub ripup_costs: Vec<(i64, i32)>,
    #[serde(rename = "maxIdBefore", default)]
    pub max_id_before: i64,
    #[serde(rename = "maxIdAfter", default)]
    pub max_id_after: i64,
    /// Every trace the connection inserted, in insertion (ascending id) order.
    #[serde(default)]
    pub traces: Vec<RouterTraceDoc>,
    /// Every via the connection inserted, in insertion order.
    #[serde(default)]
    pub vias: Vec<RouterViaDoc>,
    /// Inserted items that are neither a trace nor a via — always 0 so far, printed rather than
    /// dropped so that a new item kind cannot arrive unnoticed.
    #[serde(rename = "otherInserted", default)]
    pub other_inserted: usize,
    /// Absent on a `"GONE"` line.
    #[serde(default)]
    pub metrics: Option<RouterMetrics>,
}

/// One inserted trace of a `router.jsonl` line.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouterTraceDoc {
    pub id: i64,
    pub layer: usize,
    #[serde(rename = "halfWidth")]
    pub half_width: i32,
    /// `"(x,y)"` per `IntPoint` corner, `"~(x,y)"` for a rational one.
    pub corners: Vec<String>,
}

/// One inserted via of a `router.jsonl` line.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouterViaDoc {
    pub id: i64,
    pub center: String,
    pub padstack: String,
    #[serde(rename = "firstLayer")]
    pub first_layer: usize,
    #[serde(rename = "lastLayer")]
    pub last_layer: usize,
}

/// Four metrics measured on the live board after one connection.
///
/// `trace_length` is kept as the rendered decimal so that two runs that agree exactly agree byte
/// for byte; [`RouterMetrics::trace_length_value`] parses it for the ±10 % band the comparison
/// allows.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouterMetrics {
    /// `DesignRulesChecker.getIncompleteCount()`.
    pub incompletes: i64,
    /// `Net.getViaCount()` for the routed net.
    pub vias: i64,
    /// `BasicBoard.cumulativeTraceLength()`, as `Double.toString` rendered it.
    #[serde(rename = "traceLength")]
    pub trace_length: String,
    /// `DesignRulesChecker.getAllClearanceViolations().size()`.
    pub violations: i64,
}

/// The per-connection *change* in the two counted metrics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RouterMetricDelta {
    pub incompletes: i64,
    pub vias: i64,
}

impl RouterMetrics {
    /// `trace_length` as a number.
    ///
    /// # Panics
    ///
    /// If the field is not a finite decimal, which means one side emitted something the other
    /// cannot have produced.
    #[must_use]
    pub fn trace_length_value(&self) -> f64 {
        self.trace_length
            .parse::<f64>()
            .unwrap_or_else(|e| panic!("traceLength {:?} is not a double: {e}", self.trace_length))
    }

    /// The change this connection made, against the previous connection of the same stem. `None`
    /// for the first connection, where the absolute values are the delta from the loaded board.
    #[must_use]
    pub fn delta(&self, previous: Option<&RouterMetrics>) -> RouterMetricDelta {
        let (base_incompletes, base_vias) = previous.map_or((0, 0), |p| (p.incompletes, p.vias));
        RouterMetricDelta {
            incompletes: self.incompletes - base_incompletes,
            vias: self.vias - base_vias,
        }
    }

    /// One connection's comparison: the two deltas equal, the violation count equal to the
    /// reference's, and the cumulative trace length within ±10 %.
    ///
    /// # Errors
    ///
    /// One line naming the rung that failed and both sides' values.
    pub fn check_spec9(
        &self,
        previous: Option<&RouterMetrics>,
        expected: &RouterMetrics,
        expected_previous: Option<&RouterMetrics>,
    ) -> Result<(), String> {
        let mine = self.delta(previous);
        let theirs = expected.delta(expected_previous);
        if mine.incompletes != theirs.incompletes {
            return Err(format!(
                "incompletes delta {} != the reference's {} (absolute {} vs {})",
                mine.incompletes, theirs.incompletes, self.incompletes, expected.incompletes
            ));
        }
        if mine.vias != theirs.vias {
            return Err(format!(
                "via delta {} != the reference's {} (absolute {} vs {})",
                mine.vias, theirs.vias, self.vias, expected.vias
            ));
        }
        if self.violations != expected.violations {
            return Err(format!(
                "clearance violations {} != the reference's {}",
                self.violations, expected.violations
            ));
        }
        let (mine, theirs) = (self.trace_length_value(), expected.trace_length_value());
        // A zero reference length is only reachable before anything is routed, where the port's
        // must be zero too; the relative band would be a division by zero.
        let within = if theirs == 0.0 {
            mine == 0.0
        } else {
            ((mine - theirs) / theirs).abs() <= 0.10
        };
        if !within {
            return Err(format!(
                "trace length {mine} is outside ±10 % of the reference's {theirs}"
            ));
        }
        Ok(())
    }
}

/// Parses a whole `router.jsonl` reference.
///
/// # Errors
///
/// Any `serde_json` parse failure, with the 1-based line number prefixed.
pub fn parse_router_jsonl(s: &str) -> Result<Vec<RouterConnectionDoc>, String> {
    s.lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(i, line)| {
            serde_json::from_str::<RouterConnectionDoc>(line)
                .map_err(|e| format!("router.jsonl line {}: {e}", i + 1))
        })
        .collect()
}

// =================================================================================================
// The whole-board batch reference
// =================================================================================================
//
// `tests/reference/<stem>/batch.ses` is the SES of a whole-board run and `batch.passes.jsonl`
// its per-pass records. `crates/copper-router/tests/batch_parity.rs` reads and re-cuts them.

/// One completed routing pass of a `batch.passes.jsonl` reference: the six fields of
/// `copper_router::pipeline::PassRecord`.
///
/// `score` is `f32` because the reference carries the rendered decimal and `serde_json` parses
/// it back to the same bit pattern.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BatchPassDoc {
    /// The 1-based pass number.
    pub pass: i32,
    /// `BoardStatistics.getNormalizedScore` after the pass.
    pub score: f32,
    /// Connections still in the ratsnest after the pass.
    pub incompletes: usize,
    /// Clearance violations on the board after the pass.
    pub violations: usize,
    /// Vias on the board after the pass.
    pub vias: usize,
    /// Traces on the board after the pass.
    pub traces: usize,
}

/// Parses a whole `batch.passes.jsonl` reference.
///
/// # Errors
///
/// Any `serde_json` parse failure, with the 1-based line number prefixed.
pub fn parse_batch_passes(s: &str) -> Result<Vec<BatchPassDoc>, String> {
    s.lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(i, line)| {
            serde_json::from_str::<BatchPassDoc>(line)
                .map_err(|e| format!("batch.passes.jsonl line {}: {e}", i + 1))
        })
        .collect()
}

// =================================================================================================
// The CLI end-to-end harness
// =================================================================================================
//
// `tests/reference/cli-<stem>/` holds one whole command line's answer — `argv.txt`, `route.ses`,
// `route.exit`, `manifest.json`, `meta.txt` — cut from the binary itself under `COPPERROUTE_REGOLDEN`.
// [`run_port`] starts the binary and [`normalize_manifest`] reduces the one non-deterministic
// output to what a comparison can assert on. The runner normalises nothing.

/// The binary: `$COPPERROUTE_BIN`, else `target/release/copperroute`, else
/// `target/debug/copperroute`.
///
/// A test inside `crates/copperroute` should pass `env!("CARGO_BIN_EXE_copperroute")` instead —
/// Cargo builds and names the binary for it. This search exists for callers outside that package.
#[must_use]
pub fn port_binary() -> PathBuf {
    if let Some(path) = std::env::var_os("COPPERROUTE_BIN") {
        return PathBuf::from(path);
    }
    let target = workspace_root().join("target");
    let release = target.join("release").join("copperroute");
    if release.is_file() {
        return release;
    }
    target.join("debug").join("copperroute")
}

///
/// If the binary cannot be started — see [`port_binary`].
#[must_use]
pub fn run_port(argv: &[&str]) -> (Vec<u8>, Vec<u8>, i32) {
    run_port_binary(&port_binary(), argv)
}

/// [`run_port`] with the binary named explicitly, for a caller that has
/// `env!("CARGO_BIN_EXE_copperroute")`.
///
/// # Panics
///
/// If the binary cannot be started.
#[must_use]
pub fn run_port_binary(binary: &Path, argv: &[&str]) -> (Vec<u8>, Vec<u8>, i32) {
    let output = std::process::Command::new(binary)
        .args(argv)
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap_or_else(|e| panic!("cannot run {}: {e}", binary.display()));
    (
        output.stdout,
        output.stderr,
        output.status.code().unwrap_or(-1),
    )
}

/// One stem of `tests/reference/cli-fixtures.txt`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliStem {
    /// The reference directory is `tests/reference/cli-<name>/`.
    pub name: String,
    /// The DSN, relative to the board corpus.
    pub dsn: String,
    /// Everything after `route <dsn> -o <out>`, already split; empty for a bare run.
    pub extra: Vec<String>,
    /// `true` for the `ci` lane, `false` for the `slow` one.
    pub ci: bool,
}

/// Parses `tests/reference/cli-fixtures.txt`.
///
/// # Panics
///
/// If the file is missing or a row has fewer than four `|`-separated columns — either is a broken
/// checkout rather than a parity difference.
#[must_use]
pub fn cli_stems() -> Vec<CliStem> {
    let path = workspace_root()
        .join("tests")
        .join("reference")
        .join("cli-fixtures.txt");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    text.lines()
        .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
        .map(|line| {
            let columns: Vec<&str> = line.split('|').collect();
            assert!(
                columns.len() >= 4,
                "cli-fixtures.txt row needs four columns: {line:?}"
            );
            CliStem {
                name: columns[0].to_string(),
                dsn: columns[1].to_string(),
                extra: if columns[2] == "-" {
                    Vec::new()
                } else {
                    columns[2].split_whitespace().map(str::to_string).collect()
                },
                ci: columns[3].trim() == "ci",
            }
        })
        .collect()
}

/// `tests/reference/cli-<stem>/<file>`.
#[must_use]
pub fn cli_reference(stem: &str, file: &str) -> PathBuf {
    reference(&format!("cli-{stem}"), file)
}

/// The stem's committed `argv.txt` with its two placeholders resolved: `<CORPUS>` to the board
/// corpus and `<OUT>` to `out_dir`.
///
/// Reading the argv back rather than rebuilding it from `cli-fixtures.txt` is deliberate: the
/// committed file is what the reference was generated with, so a table edit that did not go
/// through the generator shows up as a failing run instead of as a silently different comparison.
///
/// # Panics
///
/// If the file is missing.
#[must_use]
pub fn cli_argv(stem: &str, out_dir: &Path) -> Vec<String> {
    let path = cli_reference(stem, "argv.txt");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    text.lines()
        .filter(|line| !line.is_empty())
        .map(|token| {
            token
                .replace("<CORPUS>", &corpus_dir().to_string_lossy())
                .replace("<OUT>", &out_dir.to_string_lossy())
        })
        .collect()
}

// -------------------------------------------------------------------------------------------------
// normalize_manifest
// -------------------------------------------------------------------------------------------------

/// A result manifest with everything a second run cannot reproduce removed.
///
/// Kept as a [`serde_json::Value`] rather than a typed mirror so that a new manifest field shows
/// up as a diff, not as a parse failure.
#[derive(Debug, Clone, PartialEq)]
pub struct ManifestDoc(pub serde_json::Value);

impl ManifestDoc {
    /// The document rendered with sorted keys and stable indentation, for a diff.
    ///
    /// # Panics
    ///
    /// Never in practice: the value came from `serde_json` and re-serialising it cannot fail.
    #[must_use]
    pub fn to_pretty(&self) -> String {
        serde_json::to_string_pretty(&self.0).expect("a parsed Value re-serialises")
    }
}

/// Removes `generated_at`, `git_sha`, `app_version`, `resource_usage`, every phase's
/// `duration_seconds`, and the `result_json` and `max_threads` settings: the clock, the
/// environment and the machine. Everything else is compared.
///
/// # Panics
///
/// If `json` is not a JSON object.
#[must_use]
pub fn normalize_manifest(json: &str) -> ManifestDoc {
    let mut value: serde_json::Value =
        serde_json::from_str(json).unwrap_or_else(|e| panic!("manifest is not JSON: {e}"));
    {
        let object = value.as_object_mut().expect("a manifest is a JSON object");
        object.remove("generated_at");
        object.remove("git_sha");
        object.remove("app_version");
        object.remove("resource_usage");
        if let Some(phases) = object.get_mut("phases").and_then(|p| p.as_object_mut()) {
            for (_, phase) in phases.iter_mut() {
                if let Some(phase) = phase.as_object_mut() {
                    phase.remove("duration_seconds");
                }
            }
        }
        if let Some(settings) = object
            .get_mut("settings_snapshot")
            .and_then(|s| s.as_object_mut())
        {
            settings.remove("result_json");
            settings.remove("max_threads");
            if let Some(optimizer) = settings
                .get_mut("optimizer")
                .and_then(|o| o.as_object_mut())
            {
                optimizer.remove("max_threads");
            }
        }
    }
    ManifestDoc(value)
}

/// The label `COPPERROUTE_REGOLDEN` carries when a test run is asked to rewrite the port goldens it
/// would otherwise compare against.
#[must_use]
pub fn regolden_label() -> Option<String> {
    std::env::var("COPPERROUTE_REGOLDEN")
        .ok()
        .map(|label| label.trim().to_string())
        .filter(|label| !label.is_empty())
}

/// Rewrites the named sections of a transcript file, keeping every other section in place and
/// appending the ones the file does not have yet. A section header is `prefix + name + suffix`.
///
/// # Panics
///
/// If the file cannot be written.
pub fn regolden_sections(path: &Path, prefix: &str, suffix: &str, sections: &[(&str, &[String])]) {
    let existing = std::fs::read_to_string(path).unwrap_or_default();
    let mut order: Vec<String> = Vec::new();
    let mut bodies: std::collections::BTreeMap<String, Vec<String>> = Default::default();
    let mut current: Option<String> = None;
    for line in existing.lines() {
        if let Some(name) = line
            .strip_prefix(prefix)
            .and_then(|rest| rest.strip_suffix(suffix))
        {
            current = Some(name.to_string());
            order.push(name.to_string());
            bodies.entry(name.to_string()).or_default();
            continue;
        }
        if let Some(name) = &current {
            bodies
                .get_mut(name)
                .expect("the header inserted it")
                .push(line.trim_end().to_string());
        }
    }
    for (name, rows) in sections {
        if !bodies.contains_key(*name) {
            order.push((*name).to_string());
        }
        bodies.insert(
            (*name).to_string(),
            rows.iter().map(|row| row.trim_end().to_string()).collect(),
        );
    }
    let mut out = String::new();
    for name in order {
        out.push_str(prefix);
        out.push_str(&name);
        out.push_str(suffix);
        out.push('\n');
        for row in &bodies[&name] {
            out.push_str(row);
            out.push('\n');
        }
    }
    std::fs::write(path, out).unwrap_or_else(|e| panic!("cannot write {}: {e}", path.display()));
}

/// Writes a `batch.passes.jsonl` reference, one document per line.
///
/// # Panics
///
/// If the file cannot be written.
pub fn write_batch_passes(path: &Path, passes: &[BatchPassDoc]) {
    let text: String = passes
        .iter()
        .map(|pass| serde_json::to_string(pass).expect("a pass document serialises") + "\n")
        .collect();
    std::fs::write(path, text).unwrap_or_else(|e| panic!("cannot write {}: {e}", path.display()));
}

/// Writes a `router.jsonl` reference, one connection document per line.
///
/// # Panics
///
/// If the file cannot be written.
pub fn write_router_jsonl(path: &Path, docs: &[RouterConnectionDoc]) {
    let text: String = docs
        .iter()
        .map(|doc| serde_json::to_string(doc).expect("a connection document serialises") + "\n")
        .collect();
    std::fs::write(path, text).unwrap_or_else(|e| panic!("cannot write {}: {e}", path.display()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_drops_the_date_and_sorts_unconnected_items_numerically() {
        let report = r#"{
            "$schema": "https://schemas.kicad.org/drc.v1.json",
            "coordinate_units": "mm",
            "date": "2026-09-06T00:00:00Z",
            "kicad_version": "N/A",
            "copperroute_version": "Copperroute 0.1.0",
            "source": "board.dsn",
            "unconnected_items": [{"description": "d", "severity": "error", "type": "unconnected_items",
                "items": [{"description": "a", "pos": {"x": 0, "y": 0}, "uuid": "1000"},
                          {"description": "b", "pos": {"x": 0, "y": 0}, "uuid": "99"}]}],
            "violations": [],
            "schematic_parity": []
        }"#;
        let normalized = normalize_drc_json(report).expect("parses");
        assert!(!normalized.contains("date"));
        let doc: DrcReportDoc = serde_json::from_str(&normalized).expect("re-parses");
        let uuids: Vec<&str> = doc.unconnected_items[0]
            .items
            .iter()
            .map(|item| item.uuid.as_str())
            .collect();
        assert_eq!(uuids, ["99", "1000"]);
    }
}
