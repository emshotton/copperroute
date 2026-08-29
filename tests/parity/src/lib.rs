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
