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
    #[serde(rename = "coordinateUnits")]
    pub coordinate_units: String,
    /// Read so `deny_unknown_fields` accepts a real report, then dropped by rule 1 — the
    /// `skip_serializing` is the rule. A caller that wants the JVM run's timestamp (to inject it
    /// into the port, ruling 5) reads it off the parsed document.
    #[serde(default, skip_serializing)]
    pub date: Option<String>,
    #[serde(rename = "kicadVersion")]
    pub kicad_version: String,
    #[serde(rename = "freeroutingVersion")]
    pub freerouting_version: String,
    pub source: String,
    #[serde(rename = "unconnectedItems")]
    pub unconnected_items: Vec<DrcViolationDoc>,
    pub violations: Vec<DrcViolationDoc>,
    #[serde(rename = "schematicParity")]
    pub schematic_parity: Vec<serde_json::Value>,
    /// Absent when Gson dropped a `null` (`serializeNulls` is off): `generateReportJson` never
    /// sets it, only the CLI does (`Freerouting.java:349`).
    #[serde(
        rename = "qualityScore",
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

    /// Ruling 1(c) for one connection: the two deltas equal, `violations == 0` on **both** sides,
    /// and the cumulative trace length within ±10 %.
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
        if self.violations != 0 || expected.violations != 0 {
            return Err(format!(
                "clearance violations must be 0: port {}, Java {}",
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
