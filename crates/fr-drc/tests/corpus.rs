//! The whole `-drc` path over every `.dsn` in the fixture corpus.
//!
//! Plan 5 Task 11. This suite asserts one cheap invariant per board and nothing about the
//! contents, which is what makes it affordable over 100+ files:
//!
//! 1. `generate_report` does not panic. That is worth an assertion because Task 11 replaced the
//!    report layer's three silent fallbacks (`item_description`, `detailed_trace_description`,
//!    `item_position` — `src/report/build.rs`) with panics: Java is handed the `Item` itself and
//!    would throw a `NullPointerException`, and returning `""` or `(0.0, 0.0)` would have put a
//!    plausible-looking wrong value into a parity document instead. The claim "unreachable" is
//!    only as good as the corpus that tests it, and this is that corpus.
//! 2. `report_to_json(FreeroutingHead)` produces parseable JSON with the nine HEAD keys
//!    `generateReportJson` can emit — and **without** `qualityScore`, which is `None` here and
//!    which Gson omits for a null (`serializeNulls = false`).
//! 3. **`violations.len() == clearance + track_dangling + via_dangling`.** `generateReport` fills
//!    `violations` from *two* sources — the clearance list (`DesignRulesChecker.java:231-233`)
//!    and the two dangling kinds it routes out of `getAllUnconnectedItems` (`:271-276`) — while
//!    everything else goes to `unconnectedItems` (`:275`). A mis-routed entry (a `holeClearance`
//!    landing in `unconnectedItems`, an `unconnectedItems` entry landing in `violations`) is
//!    invisible to a count of either list alone and is exactly what this sum catches.
//! 4. The clearance half of that sum equals `get_all_clearance_violations().len()` computed
//!    independently, so the two-source loop cannot compensate for itself.
//!
//! It is **not** a parity suite: nothing here compares to Java. The corpus-wide Java comparison is
//! `scripts/differential/sweep-p5t1.sh` (the whole document, 112 rows) and `sweep-p5t2.sh` (the
//! three raw lists, 5 modes). This runs without a JVM.
//!
//! `#[cfg_attr(debug_assertions, ignore)]`: the DRC compute is quadratic-ish in item count and the
//! corpus has boards with thousands of items, so a debug run is minutes. Run it with
//!
//! ```sh
//! cargo test -p fr-drc --release --test corpus
//! ```
//!
//! or `cargo test -p fr-drc --test corpus -- --ignored` if you want the debug assertions too.
//!
//! Measured while writing Task 11: **147 boards checked, 1 skipped** (`Issue006-LPC18XX_43XX_SCH.dsn`,
//! which neither reader accepts — the same row `sweep-p5t1.sh` reports as its one SKIP), 51 s in
//! release. That is a wider net than the differential's 112 rows, because the sweeps run only the
//! fixtures the *jar* also reads.
//!
//! Boards the reader rejects are **skipped, not failed**: `fr-dsn`'s coverage is Plan 3's, and a
//! DRC suite that fails on an unreadable DSN would be reporting someone else's regression. The
//! count of skips is printed and bounded.

use std::path::{Path, PathBuf};

use fr_board::Board;
use fr_drc::report::{DrcCoordinates, DrcReportOptions};
use fr_drc::{DesignRulesChecker, DrcJsonFlavor};
use fr_dsn::{BoardReadResult, CoordinateTransform, DsnReadOptions};

/// Every `.dsn` under `../freerouting/fixtures`, sorted, so a failure names the same file on
/// every machine.
fn corpus() -> Vec<PathBuf> {
    let mut found = Vec::new();
    collect(&parity::java_dir().join("fixtures"), &mut found);
    found.sort();
    found
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, out);
        } else if path.extension().is_some_and(|e| e == "dsn") {
            out.push(path);
        }
    }
}

/// `Success` and `OutlineMissing` both carry a board, exactly as
/// `RatsnestClearanceHeadlessTest.java:41-45` accepts both. Anything else is a reader-side skip.
fn read(bytes: &[u8], name: &str) -> Option<(Board, CoordinateTransform)> {
    match fr_dsn::read_board(bytes, None, Some(name), &DsnReadOptions::default()) {
        BoardReadResult::Success {
            board,
            coordinate_transform,
            ..
        }
        | BoardReadResult::OutlineMissing {
            board,
            coordinate_transform,
            ..
        } => Some((*board?, coordinate_transform?)),
        _ => None,
    }
}

#[test]
#[cfg_attr(debug_assertions, ignore)]
fn every_fixture_reports_without_panicking_and_the_two_sources_add_up() {
    if !parity::require_java_dir() {
        return;
    }
    let files = corpus();
    assert!(
        files.len() >= 105,
        "corpus shrank: {} .dsn files",
        files.len()
    );

    let mut checked = 0usize;
    let mut skipped: Vec<String> = Vec::new();

    for path in &files {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .expect("a UTF-8 file name")
            .to_string();
        let bytes = std::fs::read(path).expect("fixture readable");
        let Some((mut board, transform)) = read(&bytes, &name) else {
            skipped.push(name);
            continue;
        };

        let coords = DrcCoordinates {
            board_unit: board.communication.unit,
            transform,
        };
        let options = DrcReportOptions {
            source: name.clone(),
            // `Freerouting.initializeDrc` hard-codes `"mm"` (Freerouting.java:335-336, quirk #151).
            coordinate_unit: "mm".to_string(),
            date: "2026-08-29T00:00:00Z".to_string(),
            freerouting_version: "corpus".to_string(),
            // `generateReportJson`'s own shape: the CLI assigns the score afterwards
            // (Freerouting.java:349), so the key is absent here.
            quality_score: None,
        };

        // (4) the clearance list on its own, before the report re-derives it.
        let clearance = DesignRulesChecker::new(&mut board)
            .get_all_clearance_violations()
            .len();

        // (1) no panic.
        let mut drc = DesignRulesChecker::new(&mut board);
        let report = drc.generate_report(&coords, &options);

        // (3) the two-source sum.
        let of = |kind: &str| report.violations.iter().filter(|v| v.kind == kind).count();
        let hole_clearance = of("holeClearance");
        let plain_clearance = of("clearance");
        let track_dangling = of("track_dangling");
        let via_dangling = of("via_dangling");
        assert_eq!(
            hole_clearance + plain_clearance + track_dangling + via_dangling,
            report.violations.len(),
            "{name}: violations carries a `type` that is neither a clearance nor a dangling kind \
             — generateReport's two-source loop mis-routed an entry"
        );
        assert_eq!(
            hole_clearance + plain_clearance,
            clearance,
            "{name}: the report's clearance entries do not match getAllClearanceViolations()"
        );
        // `unconnectedItems` is the *other* branch of the same `if`, so nothing dangling may be
        // in it (DesignRulesChecker.java:271-276).
        assert!(
            report
                .unconnected_items
                .iter()
                .all(|entry| entry.kind == "unconnectedItems"),
            "{name}: a dangling entry reached unconnectedItems"
        );

        // (2) valid JSON with the HEAD spelling.
        let text = drc
            .report_to_json(&coords, &options, DrcJsonFlavor::FreeroutingHead)
            .unwrap_or_else(|e| panic!("{name}: report_to_json failed: {e}"));
        let json: serde_json::Value = serde_json::from_str(&text)
            .unwrap_or_else(|e| panic!("{name}: report_to_json wrote invalid JSON: {e}"));
        let object = json.as_object().expect("a JSON object");
        for key in [
            "$schema",
            "coordinateUnits",
            "date",
            "kicadVersion",
            "freeroutingVersion",
            "source",
            "unconnectedItems",
            "violations",
            "schematicParity",
        ] {
            assert!(object.contains_key(key), "{name}: JSON has no {key}");
        }
        // `quality_score: None` is Java's null, which Gson omits.
        assert!(!object.contains_key("qualityScore"), "{name}");

        checked += 1;
    }

    println!(
        "fr-drc corpus: {checked} boards checked, {} skipped {skipped:?}",
        skipped.len()
    );
    assert!(checked >= 100, "only {checked} boards reached the DRC");
    assert!(
        skipped.len() <= 5,
        "the reader rejected {} fixtures, which is more than Plan 3 left open: {skipped:?}",
        skipped.len()
    );
}
