//! `#[cfg_attr(debug_assertions, ignore)]`: the DRC compute is quadratic-ish in item count and the
use std::path::{Path, PathBuf};

use copper_board::Board;
use copper_drc::report::{DrcCoordinates, DrcReportOptions};
use copper_drc::{DesignRulesChecker, DrcViolationKind};
use copper_dsn::{BoardReadResult, CoordinateTransform, DsnReadOptions};

fn corpus() -> Vec<PathBuf> {
    let mut found = Vec::new();
    collect(&testkit::corpus_dir().join("fixtures"), &mut found);
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

fn read(bytes: &[u8], name: &str) -> Option<(Board, CoordinateTransform)> {
    match copper_dsn::read_board(bytes, None, Some(name), &DsnReadOptions::default()) {
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
            coordinate_unit: "mm".to_string(),
            date: "2026-08-29T00:00:00Z".to_string(),
            router_version: "corpus".to_string(),
            quality_score: None,
        };

        let clearance = DesignRulesChecker::new(&mut board)
            .get_all_violations()
            .len();

        let mut drc = DesignRulesChecker::new(&mut board);
        let report = drc.generate_report(&coords, &options);

        let of = |kind: &str| report.violations.iter().filter(|v| v.kind == kind).count();
        let checked_kinds: usize = DrcViolationKind::ALL
            .iter()
            .map(|kind| of(kind.kicad_type()))
            .sum();
        let track_dangling = of("track_dangling");
        let via_dangling = of("via_dangling");
        assert_eq!(
            checked_kinds + track_dangling + via_dangling,
            report.violations.len(),
            "{name}: violations carries a type that is neither a checked kind nor a dangling kind"
        );
        assert_eq!(
            checked_kinds, clearance,
            "{name}: the report's checked entries do not match get_all_violations()"
        );
        assert!(
            report
                .unconnected_items
                .iter()
                .all(|entry| entry.kind == "unconnected_items"),
            "{name}: a dangling entry reached unconnected_items"
        );

        let text = drc
            .report_to_json(&coords, &options)
            .unwrap_or_else(|e| panic!("{name}: report_to_json failed: {e}"));
        let json: serde_json::Value = serde_json::from_str(&text)
            .unwrap_or_else(|e| panic!("{name}: report_to_json wrote invalid JSON: {e}"));
        let object = json.as_object().expect("a JSON object");
        for key in [
            "$schema",
            "coordinate_units",
            "date",
            "kicad_version",
            "copperroute_version",
            "source",
            "unconnected_items",
            "violations",
            "schematic_parity",
        ] {
            assert!(object.contains_key(key), "{name}: JSON has no {key}");
        }
        assert!(!object.contains_key("quality_score"), "{name}");

        checked += 1;
    }

    println!(
        "copper-drc corpus: {checked} boards checked, {} skipped {skipped:?}",
        skipped.len()
    );
    assert!(checked >= 100, "only {checked} boards reached the DRC");
    assert!(
        skipped.len() <= 5,
        "the reader rejected {} fixtures, which is more than Plan 3 left open: {skipped:?}",
        skipped.len()
    );
}
