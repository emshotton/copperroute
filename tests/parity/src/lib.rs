#![forbid(unsafe_code)]

//! Parity-test helpers: locate the Java clone, fixtures and reference outputs,
//! and compare text outputs modulo whitespace.

use similar::{ChangeTag, TextDiff};
use std::path::{Path, PathBuf};

pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .to_path_buf()
}

pub fn java_dir() -> PathBuf {
    std::env::var_os("FREEROUTING_JAVA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace_root().join("..").join("freerouting"))
}

pub fn fixture(name: &str) -> PathBuf {
    java_dir().join("fixtures").join(name)
}

pub fn example(name: &str) -> PathBuf {
    java_dir().join("examples").join(name)
}

/// `Path.of(args[0]).getFileName().toString().replaceAll("\\.dsn$", "")`
/// (`scripts/gen-reference/RefWriter.java:26`) — the `designName` the DSN/SES writers stamp into
/// `(pcb "…")` and `(session "…")`.
///
/// The Java is a **regex replace over the whole file name**, anchored with `$`, so `a.dsn` becomes
/// `a` and `a.dsn.b` is left alone; `strip_suffix` is exactly that. A name that is *not* a `.dsn`
/// yields the empty string, which is Java's behaviour too and is why the `unwrap_or_default` is
/// not a shortcut.
///
/// **One copy, deliberately.** It lived twice — in `crates/fr-dsn/tests/parity_dsn.rs` and in
/// `scripts/differential/rust/src/bin/refwriter.rs`, the binary that cuts the committed
/// `roundtrip.dsn` / `unrouted.ses` from the port — and a drift between the two would have moved
/// every family-G golden with nothing failing (Plan 9 Task 0 review, S5). Both now call this.
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

/// Canonicalises a text output for parity comparison: CRLF → LF, trailing **spaces and tabs**
/// stripped from every line, runs of blank lines collapsed to one, and exactly one trailing
/// newline.
///
/// Deliberately *not* `trim_end()`: that strips every Unicode whitespace character, including
/// U+00A0, U+2028 and the vertical tab. Those can legitimately appear inside a Specctra DSN
/// string literal (component names, comments), so treating them as insignificant trailing
/// noise could hide a real parity difference. Only the two characters an editor or a
/// line-ending conversion actually introduces are removed.
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
    // trim trailing blank lines to exactly one '\n'
    while out.ends_with("\n\n") {
        out.pop();
    }
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// Returns false and prints a skip message when the sibling Java checkout is absent.
///
/// The committed `tests/reference/` outputs travel with this repository, but the *inputs* they
/// were generated from live in `../freerouting/fixtures` (or `$FREEROUTING_JAVA_DIR`), which is
/// not vendored. A suite that reads a fixture must call this first and return early, so
/// `cargo test` on a bare checkout skips loudly instead of panicking on a missing file.
pub fn require_java_dir() -> bool {
    let dir = java_dir();
    if dir.join("fixtures").is_dir() {
        true
    } else {
        eprintln!(
            "SKIP: Java fixture corpus not found at {} — check out the freerouting repo as a \
             sibling directory, or set FREEROUTING_JAVA_DIR",
            dir.display()
        );
        false
    }
}

/// Returns false and prints a skip message when a reference file is absent.
pub fn require_reference(path: &Path) -> bool {
    if path.exists() {
        true
    } else {
        eprintln!(
            "SKIP: reference {} missing — run scripts/gen-reference.sh",
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
    // Save actual for inspection.
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
// The KiCad DRC report (plan-5 Task 9)
// =============================================================================================

/// The DRC report, reduced to what is reproducible across JVMs (plan-5 ruling 3).
///
/// 1. **`date` is dropped.** Java fills it from `ZonedDateTime.now()`
///    (`io/kicad/KiCadDrcReport.java:70`); the port takes it as an injected string (ruling 5), so
///    there is nothing to compare.
/// 2. **Every `unconnectedItems` entry's `items` array is sorted by *numeric* uuid.** On the JVM
///    that array is `connectedSets.get(0)` followed by `connectedSets.get(1)`
///    (`drc/DesignRulesChecker.java:143-145`), two `HashSet<Item>`s over a class with no
///    `hashCode` override — identity-hash order, i.e. nothing to port (quirk #144). The port emits
///    ascending item id. The sort is numeric because the uuids are `String.valueOf(item.getId())`
///    (`:320-321`), so `"1000"` must sort *after* `"99"`.
/// 3. **Nothing else is touched.** In particular `violations` is left completely alone — neither
///    the array nor any entry's `items`. The array order is hash-independent (ruling 3's probe)
///    and is this plan's bit-parity surface; a clearance entry's `items` is
///    `[firstItem, secondItem]` in `ClearanceViolation`'s own deterministic order (`:319-324`), so
///    sorting it would hide a real ordering regression. A dangling entry has exactly one item,
///    where sorting is a no-op anyway. The `unconnectedItems` *entry* order is left alone too: it
///    is ascending net number on both sides.
///
/// The document is re-emitted from [`DrcReportDoc`], whose fields are in Java's declaration order
/// (`KiCadDrcReport.java:20-57`), so the key order of the input does not survive — which is why
/// key order is pinned separately and byte-exactly by `crates/fr-drc/tests/report_json.rs`
/// against Gson's own output. `#[serde(deny_unknown_fields)]` makes a key the port does not know
/// about a loud parse failure rather than a silent drop.
///
/// The input must be plan-5 ruling 2's `FreeroutingHead` flavor — the parity default and the
/// spelling the HEAD jar writes.
///
/// # Errors
///
/// Any `serde_json` parse failure, including an unknown or missing key.
pub fn normalize_drc_json(s: &str) -> Result<String, serde_json::Error> {
    normalize_drc_doc(&mut parse_drc_json(s)?)
}

/// Parses a `FreeroutingHead`-flavor DRC report. Split out of [`normalize_drc_json`] so a caller
/// that has to *edit* the document before comparing — the Natural Tone Preamp fixture, where the
/// port legitimately emits three `track_dangling` entries fewer than the JVM (quirk #146) — can do
/// it on typed data and still compare the normalised bytes.
///
/// # Errors
///
/// Any `serde_json` parse failure, including an unknown or missing key.
pub fn parse_drc_json(s: &str) -> Result<DrcReportDoc, serde_json::Error> {
    serde_json::from_str(s)
}

/// Applies the three rules above to an already-parsed report and renders it.
///
/// The result is a **re-rendered** document, not the input's bytes: keys come out in
/// [`DrcReportDoc`]'s field order and every number is re-emitted by `serde_json`. Two normalised
/// documents being byte-equal therefore says the two reports carry the same values in the same
/// order — it says nothing about how either side spelled a `double` or ordered its keys. Those are
/// pinned separately and byte-exactly against Gson's own output by
/// `crates/fr-drc/tests/report_json.rs::head_flavor_is_the_jvms_gson_bytes`.
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

/// Port of `io.kicad.KiCadDrcReport` as a *parity projection*: the fields in Java's declaration
/// order (`KiCadDrcReport.java:20-57`) under plan-5 ruling 1's HEAD key spellings.
///
/// This is deliberately not `fr_drc::KiCadDrcReport`: `tests/parity` sits below every crate under
/// test and must be able to read a reference the port cannot yet produce.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DrcReportDoc {
    #[serde(rename = "$schema")]
    pub schema: String,
    #[serde(rename = "coordinateUnits", alias = "coordinate_units")]
    pub coordinate_units: String,
    /// Read so `deny_unknown_fields` accepts a real report, then dropped by rule 1 — the
    /// `skip_serializing` is the rule. A caller that wants the JVM run's timestamp (to inject it
    /// into the port, ruling 5) reads it off the parsed document.
    #[serde(default, skip_serializing)]
    pub date: Option<String>,
    #[serde(rename = "kicadVersion", alias = "kicad_version")]
    pub kicad_version: String,
    #[serde(rename = "freeroutingVersion", alias = "freerouting_version")]
    pub freerouting_version: String,
    pub source: String,
    #[serde(rename = "unconnectedItems", alias = "unconnected_items")]
    pub unconnected_items: Vec<DrcViolationDoc>,
    pub violations: Vec<DrcViolationDoc>,
    #[serde(rename = "schematicParity", alias = "schematic_parity")]
    pub schematic_parity: Vec<serde_json::Value>,
    /// Absent when Gson dropped a `null` (`serializeNulls` is off): `generateReportJson` never
    /// sets it, only the CLI does (`Freerouting.java:349`).
    #[serde(
        rename = "qualityScore",
        alias = "quality_score",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub quality_score: Option<f64>,
}

/// Port of `io.kicad.KiCadDrcViolation` (`KiCadDrcViolation.java:7-50`), declaration order.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DrcViolationDoc {
    pub description: String,
    pub items: Vec<DrcViolationItemDoc>,
    pub severity: String,
    /// Java's field is `type`, which is a Rust keyword.
    #[serde(rename = "type")]
    pub kind: String,
}

/// Port of `io.kicad.KiCadDrcViolationItem` (`KiCadDrcViolationItem.java:6-31`).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DrcViolationItemDoc {
    pub description: String,
    pub pos: DrcPositionDoc,
    /// `String.valueOf(item.getId())` (`DesignRulesChecker.java:320-321`) — the board-unique item
    /// id, not a UUID, which is why rule 2 sorts it numerically.
    pub uuid: String,
}

/// Port of `io.kicad.KiCadDrcPosition` (`KiCadDrcPosition.java:6-25`), `@SerializedName`d to
/// `x`/`y`.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DrcPositionDoc {
    pub x: f64,
    pub y: f64,
}

// =================================================================================================
// The router reference (Plan 6 Task 17, ruling 1)
// =================================================================================================
//
// `tests/reference/<stem>/router.jsonl` is one JSON line per connection, written verbatim by
// `scripts/differential/java/P6T1.java` through `scripts/gen-router-reference.sh`. The types below
// are a *parity projection* of that line, in the driver's own field order, so that
// `crates/fr-router/tests/reference_parity.rs` can read a reference and compare it rung by rung
// against what the port produces — rather than diffing two strings and reporting "line 214
// differs".
//
// Why the coordinates are `String`s: the driver renders an `IntPoint` corner as `"(x,y)"` and a
// rational one as `"~(x,y)"` through `Double.toString`, and `traceLength` likewise. Keeping the
// rendering as the comparison surface means no re-parse can round a value into agreement, and it
// is the same choice `DrcViolationItemDoc::uuid` makes for `String.valueOf(item.getId())`.

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
    /// `AutorouteAttemptState`'s Java name, or the driver's own `"GONE"`.
    pub state: String,
    /// `AutorouteAttemptResult.details`; `""` where Java's one-argument constructor stored the
    /// empty string.
    #[serde(default)]
    pub details: String,
    /// The ripped-item id set, rendered in Java's descending `TreeSet<Item>` order (quirk #44).
    #[serde(default)]
    pub ripped: Vec<i64>,
    /// `(item id, cost)` pairs, sorted by item id on both sides — see `P6T1.routeOne`.
    #[serde(rename = "ripupCosts", default)]
    pub ripup_costs: Vec<(i64, i32)>,
    #[serde(rename = "maxIdBefore", default)]
    pub max_id_before: i64,
    #[serde(rename = "maxIdAfter", default)]
    pub max_id_after: i64,
    /// Ruling 1(b): every trace the connection inserted, in insertion (ascending id) order.
    #[serde(default)]
    pub traces: Vec<RouterTraceDoc>,
    /// Ruling 1(b): every via the connection inserted, in insertion order.
    #[serde(default)]
    pub vias: Vec<RouterViaDoc>,
    /// Inserted items that are neither a trace nor a via — always 0 so far, printed rather than
    /// dropped so that a new item kind cannot arrive unnoticed.
    #[serde(rename = "otherInserted", default)]
    pub other_inserted: usize,
    /// Ruling 1(c). Absent on a `"GONE"` line.
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

/// Spec §9's four metrics, measured on the live board after one connection.
///
/// `trace_length` is the string `Double.toString(board.cumulativeTraceLength())` produced, so that
/// two runs that agree exactly agree byte for byte; [`RouterMetrics::trace_length_value`] parses it
/// for the ±10 % band ruling 1(c) allows.
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

/// The per-connection *change* in the two counted metrics — ruling 1(c)'s "incompletes delta
/// equal, via delta equal".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RouterMetricDelta {
    pub incompletes: i64,
    pub vias: i64,
}

impl RouterMetrics {
    /// `trace_length` as a number. Java's `Double.toString` and Rust's `f64::from_str` agree on
    /// every finite value's *value* (they disagree only on rendering, which is why the field is
    /// kept as a string), so this is exact for both sides' output.
    ///
    /// # Panics
    ///
    /// If the field is not a finite `Double.toString` rendering — which would mean the reference
    /// or the port emitted something the other side cannot have produced, and is a failure worth
    /// stopping on rather than tolerating.
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

    /// Ruling 1(c) for one connection: the two deltas equal, the violation count equal to the
    /// reference's, and the cumulative trace length within ±10 %.
    ///
    /// `Ok(())` or the first failing rung, named — the caller turns it into the panic, so that a
    /// harness that only *reports* rung (c) (the README's table) can use the same check.
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
                "incompletes delta {} != Java's {} (absolute {} vs {})",
                mine.incompletes, theirs.incompletes, self.incompletes, expected.incompletes
            ));
        }
        if mine.vias != theirs.vias {
            return Err(format!(
                "via delta {} != Java's {} (absolute {} vs {})",
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
                "trace length {mine} is outside ±10 % of Java's {theirs}"
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
// The whole-board batch reference (plan-7 Task 16)
// =================================================================================================
//
// `tests/reference/<stem>/batch.ses` is the HEAD jar's **verbatim** SES for a whole-board
// `-de <dsn> -do <ses>` run, and `batch.passes.jsonl` its per-pass `PassRecord` tuples. Both are
// written by `scripts/gen-batch-reference.sh`. The types and the one normaliser below are what
// `crates/fr-router/tests/batch_parity.rs` reads them with.

/// One completed routing pass of a `batch.passes.jsonl` reference — the six fields of
/// `fr_router::pipeline::PassRecord`, in `P7T9.passRecord`'s order.
///
/// `score` is `f32` because both sides render it through Java's `Float.toString`; the reference
/// carries the rendered decimal, and `serde_json` parses it back to the same `f32` bit pattern
/// (the rendering is round-trip exact by construction — that is what `Float.toString` guarantees).
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

/// The four `(parser …)` scope keywords the clone's HEAD camelCased, rewritten to the Specctra
/// spelling this port emits — **quirk #92**, and the *only* normalisation `batch.ses` needs.
///
/// # Why this exists and why it is not a tolerance
///
/// Plan 3 ruling 1 pins the port's DSN/SES **writer** literals to the 2.3.0 jar, because HEAD
/// camelCased fifteen keyword literals in `Keyword.java` and the writers with them while leaving
/// its own lexer recognising only the snake_case tokens — so HEAD writes Specctra it cannot read
/// back (re-reading its own `tutorial_board` output drops `host_cad`, every via rule, every
/// clearance rule and 26 wires). `tests/reference/README.md` §"Why the 2.3.0 jar" records the
/// ruling and forbids regenerating those references from HEAD.
///
/// Plan 6 ruling and plan 7 pin the **router** to HEAD, because 2.3.0's `autoroute/**` is a
/// different algorithm. `batch.ses` is therefore the one file in the tree written by HEAD's
/// `SesWriter`, and it carries HEAD's spelling of the two `(parser …)` keywords an SES contains.
///
/// So the difference is not a routing difference, not a rounding difference and not a tolerance:
/// it is a **closed, enumerated set of four keyword literals** — every camelCase string literal in
/// `io/specctra/SesWriter.java` and `io/specctra/parser/Parser.java` combined, found with
/// `grep -ohE '"\(?[a-z]+[A-Z][A-Za-z_]*'` over the two files — rewritten on the *reference* side
/// only, line by line, and only where the line's first non-blank characters are the keyword's own
/// `(name ` opening. Anything else that differs is a real diff and stays one. The DRC reference
/// family has the same shape of problem and the same shape of answer
/// (`normalize_drc_json`/`scripts/normalize-drc.py`, plan-5 ruling 3).
///
/// Only `hostCad` and `hostVersion` actually occur in an SES: `SesWriter.write` reaches
/// `Parser.writeScope` with `reduced = true`, which skips `stringQuote`, and `writeResolution` is
/// written only when the board carries one. The other two are listed anyway, because the set is
/// the writers' and not this corpus's.
#[must_use]
pub fn normalize_ses_head_tokens(s: &str) -> String {
    /// `(head, specctra)` — `io/specctra/parser/Parser.java:102-135` at the clone's HEAD against
    /// the same method in `tools/freerouting-2.3.0.jar`.
    const TOKENS: [(&str, &str); 4] = [
        ("(hostCad ", "(host_cad "),
        ("(hostVersion ", "(host_version "),
        ("(stringQuote ", "(string_quote "),
        ("(writeResolution ", "(write_resolution "),
    ];
    let mut out = String::with_capacity(s.len());
    for line in s.split_inclusive('\n') {
        let indent = line.len() - line.trim_start().len();
        let (lead, rest) = line.split_at(indent);
        let mut written = false;
        for (head, specctra) in TOKENS {
            if let Some(tail) = rest.strip_prefix(head) {
                out.push_str(lead);
                out.push_str(specctra);
                out.push_str(tail);
                written = true;
                break;
            }
        }
        if !written {
            out.push_str(line);
        }
    }
    out
}

// =================================================================================================
// The CLI end-to-end harness (Plan 8 Task 6, ruling 13)
// =================================================================================================
//
// `tests/reference/cli-<stem>/` is the **bare HEAD jar**'s answer to one whole command line —
// `argv.txt`, `route.ses`, `route.exit`, `route.log`, `manifest.json`, `meta.txt` — written by
// `scripts/gen-cli-reference.sh`. The four helpers below are what compare a run against it:
// [`run_jar`] and [`run_port`] start the two programs, and [`normalize_log`] and
// [`normalize_manifest`] reduce their two non-deterministic outputs to what a comparison can
// legitimately assert on. **Neither runner normalises anything**, so a caller that wants the raw
// bytes — `p8t1`'s SES rung does — has them.

/// The HEAD jar `run_jar` starts: `$FREEROUTING_JAR`, else the clone's own build output.
#[must_use]
pub fn jar_path() -> PathBuf {
    std::env::var_os("FREEROUTING_JAR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            java_dir()
                .join("build")
                .join("libs")
                .join("freerouting-current-executable.jar")
        })
}

/// The `java` binary. JDK 25 is required — the jar's class files are version 69 — and the pinned
/// Homebrew `opt` symlink is the harness-wide default (`scripts/differential/run.sh`'s
/// `JAVA25_HOME`, `scripts/gen-batch-reference.sh`'s `JAVA_BIN`).
#[must_use]
pub fn java_binary() -> PathBuf {
    std::env::var_os("JAVA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/opt/homebrew/opt/openjdk@25/bin/java"))
}

/// The port's own binary: `$FREEROUTING_BIN`, else `target/release/freerouting`, else
/// `target/debug/freerouting`.
///
/// A test inside `crates/freerouting` should pass `env!("CARGO_BIN_EXE_freerouting")` instead —
/// Cargo builds and names the binary for it. This search exists for callers outside that package
/// (the `p8t1`/`p8t2` drivers, which live in their own workspace).
#[must_use]
pub fn port_binary() -> PathBuf {
    if let Some(path) = std::env::var_os("FREEROUTING_BIN") {
        return PathBuf::from(path);
    }
    let target = workspace_root().join("target");
    let release = target.join("release").join("freerouting");
    if release.is_file() {
        return release;
    }
    target.join("debug").join("freerouting")
}

/// The JVM flags every reference and every parity run uses — `scripts/gen-cli-reference.sh`'s
/// `LOCALE_FLAGS` plus the constant-hash mode.
///
/// The locale pair is load-bearing rather than hygiene: every `%.4f` in a DRC violation
/// description goes through `String.formatted`, which uses the default FORMAT locale, so a German
/// JVM writes `0,0500 mm` (plan-5 ruling 6). `-XX:hashCode=2` is plan-5 ruling 3/4's constant
/// mode, and `--verify-hash-modes` is what proves the SES does not depend on it.
const JVM_FLAGS: [&str; 5] = [
    "-Djava.awt.headless=true",
    "-Duser.language=en",
    "-Duser.country=US",
    "-XX:+UnlockExperimentalVMOptions",
    "-XX:hashCode=2",
];

/// Ruling 13's jar runner: `java <flags> -jar <jar> <argv>`, answering
/// `(stdout, stderr, exit code)` **raw**.
///
/// An exit code of `-1` means the process was killed by a signal, which `ExitStatus::code`
/// reports as `None`; no reference run has ever produced one, and a caller that sees it should
/// stop rather than compare.
///
/// # Panics
///
/// If the JVM cannot be started at all — a missing jar or a missing `java` is a broken harness,
/// not a parity difference.
#[must_use]
pub fn run_jar(argv: &[&str]) -> (Vec<u8>, Vec<u8>, i32) {
    let jar = jar_path();
    let mut command = std::process::Command::new(java_binary());
    command
        .args(JVM_FLAGS)
        .arg("-jar")
        .arg(&jar)
        .args(argv)
        .stdin(std::process::Stdio::null());
    let output = command.output().unwrap_or_else(|e| {
        panic!(
            "cannot run {} -jar {}: {e}",
            java_binary().display(),
            jar.display()
        )
    });
    (
        output.stdout,
        output.stderr,
        output.status.code().unwrap_or(-1),
    )
}

/// Ruling 13's port runner: `freerouting <argv>`, answering `(stdout, stderr, exit code)` **raw**.
///
/// # Panics
///
/// If the binary cannot be started — see [`port_binary`].
#[must_use]
pub fn run_port(argv: &[&str]) -> (Vec<u8>, Vec<u8>, i32) {
    run_port_binary(&port_binary(), argv)
}

/// [`run_port`] with the binary named explicitly, for a caller that has
/// `env!("CARGO_BIN_EXE_freerouting")`.
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
    /// The DSN, relative to the Java checkout.
    pub dsn: String,
    /// Everything after `-de <dsn> -do <out>`, already split; empty for a bare run.
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

/// The stem's committed `argv.txt` with its two placeholders resolved: `<JAVA_DIR>` to the Java
/// checkout and `<OUT>` to `out_dir`.
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
                .replace("<JAVA_DIR>", &java_dir().to_string_lossy())
                .replace("<OUT>", &out_dir.to_string_lossy())
        })
        .collect()
}

// -------------------------------------------------------------------------------------------------
// normalize_log
// -------------------------------------------------------------------------------------------------

/// The log projection both sides are compared through: **one `<LEVEL> <Java call site>` line per
/// `FRLogger` message `freerouting::logging::MESSAGE_MAP` names, in order.**
///
/// # What is removed, and why each removal is not a tolerance
///
/// 1. **log4j2's timestamp.** The layout is
///    `"%d{yyyy-MM-dd HH:mm:ss.SSS} %-6level %msg%n"` (`Log4j2ConfigurationFactory.java:29`).
///    A wall-clock stamp can never match across two processes. The port emits none at all
///    (`crate::logging`'s `.without_time()`), which is why this parser treats the prefix as
///    optional rather than requiring it on both sides.
/// 2. **Java's two console streams are merged** — quirk label AI / `docs/java-quirks.md` #261.
///    `Log4j2ConfigurationFactory` builds a `Console` appender on `SYSTEM_OUT` (`:58`) *and* a
///    second one on `SYSTEM_ERR` at `Level.ERROR` (`:88-96`), so every `ERROR` is written twice
///    to the console (three times counting the file appender). The port writes everything once,
///    to stderr. The merge is therefore: concatenate, then drop a repeated `ERROR` entry that has
///    already been seen. Only `ERROR` is deduped, because only `ERROR` is duplicated.
/// 3. **Everything the map does not name is dropped.** That is most of a successful run: the
///    version banner, the headless screen-resolution warning, the update check, and every
///    `job.logInfo` progress line the pipeline emits — board hashes, per-pass scores, CPU
///    seconds, allocated gigabytes, elapsed durations and two ISO instants. Controller ruling AK
///    replaced Java's listener mechanism with `ProgressSink` and ruling 11 records that nothing
///    downstream reads it, so the port emits none of them; comparing them would be comparing a
///    thing the port deliberately does not have. What is left is the CLI's own message set, which
///    **is** a parity surface.
/// 4. **The donation banner** (`Freerouting.java:164-183`, `docs/java-quirks.md` #266,
///    quirk label H) — six box-drawing lines on **stdout**. It is un-suppressible in Java and the port
///    does not print it (see `commands/route.rs`'s roster). Dropped here so a machine whose
///    persisted `statistics.jobsCompleted` has passed 5 does not fail the comparison.
/// 5. **The startup version line**, `Freerouting.java:1120`'s `FRLogger.info("Freerouting " +
///    VERSION_NUMBER_STRING)` — `[`SUPPRESSED_SITES`]. It is in `MESSAGE_MAP` because it is a
///    real `FRLogger` call the map's job is to enumerate, but it is **not** a message the port
///    can emit: controller ruling AT forbids the port claiming to be the jar, so it would have to
///    print its own version, which is a different string that no normaliser could match without
///    erasing the comparison. Recorded as a divergence at `freerouting::run`'s
///    `// not ported:` marker rather than smoothed away silently.
/// 6. **The message arguments.** A mapped line renders as `<LEVEL> <site>` and its interpolated
///    values are discarded, because they are absolute paths (which differ by scratch directory)
///    and exception texts (`java.io.FileNotFoundException: … (No such file or directory)` against
///    `No such file or directory (os error 2)`) that the two runtimes cannot spell the same way.
///    The *identity* of the message is the site key, and that is what is compared. Java's
///    following stack-trace lines are dropped with them.
///
/// Matching is by the map's literal segments in order: the line must begin with the template's
/// first literal and contain the rest, in sequence, after it. `{}` marks where a runtime value
/// lands (Java builds these with `+`), so the segments are the only fixed part.
#[must_use]
pub fn normalize_log(stdout: &[u8], stderr: &[u8]) -> String {
    let mut combined = String::from_utf8_lossy(stdout).into_owned();
    combined.push_str(&String::from_utf8_lossy(stderr));

    let mut seen_errors: Vec<String> = Vec::new();
    let mut out = String::new();
    for line in combined.lines() {
        let Some((level, message)) = split_log_line(line) else {
            continue;
        };
        let Some(site) = message_site(message) else {
            continue;
        };
        if SUPPRESSED_SITES.contains(&site) {
            continue;
        }
        let entry = format!("{level} {site}");
        if level == "ERROR" {
            if seen_errors.contains(&entry) {
                continue;
            }
            seen_errors.push(entry.clone());
        }
        out.push_str(&entry);
        out.push('\n');
    }
    out
}

/// The `MESSAGE_MAP` sites `normalize_log` drops rather than compares, each with the reason it
/// cannot be a parity surface.
///
/// | site | Java | why |
/// |---|---|---|
/// | `Freerouting.java:1120` | `FRLogger.info("Freerouting " + VERSION_NUMBER_STRING)` at startup | the port must not print the jar's version (controller ruling AT) and printing its own would be a different string |
///
/// Kept as a named constant so the list is one line to read and one line to grow, and so a site
/// that is dropped can never be mistaken for a site the map never had.
const SUPPRESSED_SITES: [&str; 1] = ["Freerouting.java:1120"];

/// `(level, message)` for a console line of either program, or `None` for a line that is not one
/// — a stack-trace frame, an exception header, a banner row, a blank line.
fn split_log_line(line: &str) -> Option<(&str, &str)> {
    // The optional log4j2 timestamp: `yyyy-MM-dd HH:mm:ss.SSS`, 23 characters, then whitespace.
    let rest = match line.as_bytes() {
        [y0, y1, y2, y3, b'-', m0, m1, b'-', d0, d1, b' ', ..]
            if y0.is_ascii_digit()
                && y1.is_ascii_digit()
                && y2.is_ascii_digit()
                && y3.is_ascii_digit()
                && m0.is_ascii_digit()
                && m1.is_ascii_digit()
                && d0.is_ascii_digit()
                && d1.is_ascii_digit() =>
        {
            line.get(23..)?
        }
        _ => line,
    };
    let rest = rest.trim_start();
    // `%-6level` on the Java side, `tracing`'s right-aligned five on the port's; either way the
    // level is the first whitespace-delimited token.
    let (level, message) = rest.split_once(char::is_whitespace)?;
    let level = match level {
        "OFF" | "FATAL" | "ERROR" | "WARN" | "INFO" | "DEBUG" | "TRACE" => level,
        _ => return None,
    };
    Some((level, message.trim_start()))
}

/// The `MESSAGE_MAP` key whose template `message` matches, or `None`.
fn message_site(message: &str) -> Option<&'static str> {
    freerouting::logging::MESSAGE_MAP
        .iter()
        .find(|(_, template)| template_matches(template, message))
        .map(|(site, _)| *site)
}

/// Whether `template`'s literal segments occur in `message`, in order, with the first anchored at
/// the start. `{}` is a runtime value of any length, including empty.
fn template_matches(template: &str, message: &str) -> bool {
    let mut rest = message;
    let mut first = true;
    for segment in template.split("{}") {
        if segment.is_empty() {
            first = false;
            continue;
        }
        if first {
            let Some(tail) = rest.strip_prefix(segment) else {
                return false;
            };
            rest = tail;
            first = false;
        } else {
            let Some(at) = rest.find(segment) else {
                return false;
            };
            rest = &rest[at + segment.len()..];
        }
    }
    true
}

// -------------------------------------------------------------------------------------------------
// normalize_manifest
// -------------------------------------------------------------------------------------------------

/// A `--router.result_json` manifest with everything a second run cannot reproduce removed —
/// [`normalize_manifest`]'s answer, and what `p8t2` compares field for field.
///
/// The document is kept as a [`serde_json::Value`] rather than as a typed mirror of
/// `fr_core::RoutingResultManifest`, deliberately: this crate sits below every crate under test
/// and must be able to read a jar-written manifest whose `settings_snapshot` carries fields the
/// port has not modelled. A typed reader would turn "the jar grew a setting" into a parse failure
/// instead of into a diff.
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

/// The six removals plan ruling 8 and this task's measurements call for.
///
/// | removed | why |
/// |---|---|
/// | `generated_at` | `Instant.now().toString()` (`RoutingResultManifest.java:101`) |
/// | `git_sha` | `resolveGitSha` reads the environment (`:147-161`) |
/// | `resource_usage` | the monitor thread's CPU/memory samples, which the port has no monitor thread to fill (quirk #237) |
/// | `phases.*.duration_seconds` | wall clock (`:128-132`) |
/// | `settings_snapshot.result_json` | the caller's own `--router.result_json` path |
/// | `settings_snapshot.max_threads`, `settings_snapshot.optimizer.max_threads` | `Runtime.getRuntime().availableProcessors() - 1` (`DefaultSettings.java:134`) — the machine, not the port |
///
/// Everything else is compared, including every `board_statistics` number, `normalized_score`,
/// `final_state`, `exit_code`, `output_written`, `fixture.sha256` and the whole rest of
/// `settings_snapshot`.
///
/// # Panics
///
/// If `json` is not a JSON object — a manifest that is not one is a failure worth stopping on.
#[must_use]
pub fn normalize_manifest(json: &str) -> ManifestDoc {
    let mut value: serde_json::Value =
        serde_json::from_str(json).unwrap_or_else(|e| panic!("manifest is not JSON: {e}"));
    {
        let object = value
            .as_object_mut()
            .expect("a manifest is a JSON object (RoutingResultManifest.java:21-171)");
        object.remove("generated_at");
        object.remove("git_sha");
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

/// The **lane** a reference meta file declares (ruling BT, Plan 9).
///
/// A reference family may sit in the `port` lane from Plan 9 on: the first fix that *moves* a
/// family regenerates it with `--from-port`, and from then on its bytes are the port's rather
/// than the jar's. The generators write that on the meta file's own `lane` line. A file with no
/// such line **is a jar-lane file** — which is what every pre-Plan-9 meta is, and why the default
/// is not an error.
///
/// Lives here rather than in each `references_are_from_the_head_jar` because all three of them
/// ask the same question of three different meta files, and a fourth family will ask it too.
#[must_use]
pub fn declared_lane(meta: &str) -> &str {
    meta.lines()
        .find_map(|line| line.strip_prefix("lane "))
        .map(str::trim)
        .unwrap_or("jar")
}

/// The port lane's own provenance, asserted: the sha of the build that wrote the reference and
/// the Plan 9 task it was cut at, both written by the generator.
///
/// They are what makes a port-cut reference traceable to a commit, exactly as `jar revision` does
/// in the other lane — and asserting the *jar's* build identity over a port-cut file would only
/// be asserting that nobody had switched lanes, which is not a property anything wants.
///
/// `what` names the stem in the panic message.
///
/// # Panics
///
/// If either line is absent or malformed.
pub fn assert_port_lane_provenance(meta: &str, what: &str) {
    let sha = meta
        .lines()
        .find_map(|line| line.strip_prefix("port sha "))
        .map(str::trim)
        .unwrap_or_else(|| panic!("{what}: a port-lane meta with no `port sha` line:\n{meta}"));
    assert!(
        sha.len() >= 12 && sha.chars().all(|c| c.is_ascii_hexdigit()),
        "{what}: `port sha` is not a git sha: {sha}"
    );
    let task = meta
        .lines()
        .find_map(|line| line.strip_prefix("plan 9 task "))
        .map(str::trim)
        .unwrap_or_else(|| panic!("{what}: a port-lane meta with no `plan 9 task` line:\n{meta}"));
    assert!(
        !task.is_empty() && task.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'),
        "{what}: `plan 9 task` is not a task id or branch label: {task}"
    );
}

/// The label `FR_REGOLDEN` carries when a test run is asked to rewrite the port goldens it
/// would otherwise compare against.
#[must_use]
pub fn regolden_label() -> Option<String> {
    std::env::var("FR_REGOLDEN")
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
    fn kicad_and_head_flavors_parse_to_the_same_document() {
        let head = r#"{
            "$schema": "https://schemas.kicad.org/drc.v1.json",
            "coordinateUnits": "mm",
            "kicadVersion": "N/A",
            "freeroutingVersion": "Freerouting 2.3.1-SNAPSHOT",
            "source": "board.dsn",
            "unconnectedItems": [],
            "violations": [],
            "schematicParity": [],
            "qualityScore": 100.0
        }"#;
        let kicad = r#"{
            "$schema": "https://schemas.kicad.org/drc.v1.json",
            "coordinate_units": "mm",
            "kicad_version": "N/A",
            "freerouting_version": "Freerouting 2.3.1-SNAPSHOT",
            "source": "board.dsn",
            "unconnected_items": [],
            "violations": [],
            "schematic_parity": [],
            "quality_score": 100.0
        }"#;
        let head_doc = parse_drc_json(head).expect("the head flavour parses");
        let kicad_doc = parse_drc_json(kicad).expect("the KiCad flavour parses");
        assert_eq!(
            head_doc, kicad_doc,
            "the two flavors must alias to the same document"
        );
    }
}
