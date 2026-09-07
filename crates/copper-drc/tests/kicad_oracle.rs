use std::collections::BTreeMap;
use std::path::PathBuf;

use copper_board::prelude::*;
use copper_drc::{DesignRulesChecker, DrcViolationKind, UnconnectedKind};
use copper_dsn::{BoardReadResult, CoordinateTransform, DsnReadOptions};

struct Row {
    stem: String,
    dsn: PathBuf,
    ses: PathBuf,
    pro: PathBuf,
    ignore: Vec<String>,
    needs_reference: bool,
}

fn resolve(field: &str) -> (PathBuf, bool) {
    match field.strip_prefix("java:") {
        Some(rest) => (parity::reference_dir().join(rest), true),
        None => (parity::workspace_root().join(field), false),
    }
}

fn rows() -> Vec<Row> {
    let path = parity::workspace_root().join("tests/reference/kicad-drc-fixtures.txt");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let fields: Vec<&str> = line.split('|').map(str::trim).collect();
            let (dsn, dsn_is_reference) = resolve(fields[1]);
            let (ses, ses_is_reference) = resolve(fields[2]);
            let (pro, pro_is_reference) = resolve(fields[4]);
            Row {
                stem: fields[0].to_string(),
                dsn,
                ses,
                pro,
                ignore: fields
                    .get(5)
                    .map(|s| {
                        s.split(',')
                            .filter(|t| !t.is_empty())
                            .map(str::to_string)
                            .collect()
                    })
                    .unwrap_or_default(),
                needs_reference: dsn_is_reference || ses_is_reference || pro_is_reference,
            }
        })
        .collect()
}

fn strip_existing_routing(board: &mut Board) {
    let routing: Vec<ItemId> = board
        .items_in_board_order()
        .into_iter()
        .filter(|&id| {
            matches!(
                board.get_item(id).map(Item::kind),
                Some(ItemKind::Trace | ItemKind::Via)
            )
        })
        .collect();
    for id in routing {
        board.remove_item(id);
    }
}

fn load(row: &Row) -> (Board, CoordinateTransform) {
    let bytes = std::fs::read(&row.dsn)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", row.dsn.display()));
    let (mut board, transform) =
        match copper_dsn::read_board(&bytes[..], None, None, &DsnReadOptions::default()) {
            BoardReadResult::Success {
                board,
                coordinate_transform,
                ..
            }
            | BoardReadResult::OutlineMissing {
                board,
                coordinate_transform,
                ..
            } => (
                *board.expect("a board"),
                coordinate_transform.expect("a transform"),
            ),
            other => panic!("{} did not read: {other:?}", row.stem),
        };
    strip_existing_routing(&mut board);
    let ses = std::fs::File::open(&row.ses)
        .unwrap_or_else(|e| panic!("cannot open {}: {e}", row.ses.display()));
    copper_dsn::ses_reader::read(ses, &mut board, &transform).expect("the session imports");
    let project = std::fs::read_to_string(&row.pro).expect("the project reads");
    copper_drc::apply_kicad_project(&project, &mut board, &transform).expect("the project applies");
    (board, transform)
}

fn port_counts(board: &mut Board) -> (BTreeMap<String, usize>, usize) {
    let mut checker = DesignRulesChecker::new(board);
    let mut counts = BTreeMap::new();
    for violation in checker.get_all_violations() {
        if violation.severity == DrcSeverity::Error {
            *counts
                .entry(violation.kind.kicad_type().to_string())
                .or_insert(0) += 1;
        }
    }
    let unconnected = checker
        .get_all_unconnected_items()
        .iter()
        .filter(|entry| entry.kind == UnconnectedKind::UnconnectedItems)
        .count();
    (counts, unconnected)
}

fn oracle_counts(stem: &str) -> Option<(BTreeMap<String, usize>, usize)> {
    let path = parity::reference(stem, "kicad-drc.json");
    if !parity::require_reference(&path) {
        return None;
    }
    let text = std::fs::read_to_string(&path).expect("the reference reads");
    let doc: serde_json::Value = serde_json::from_str(&text).expect("the reference is JSON");
    let mut counts = BTreeMap::new();
    for violation in doc["violations"].as_array().into_iter().flatten() {
        if violation["severity"].as_str() != Some("error") {
            continue;
        }
        let kind = violation["type"].as_str().unwrap_or_default();
        if DrcViolationKind::from_kicad_type(kind).is_some() {
            *counts.entry(kind.to_string()).or_insert(0) += 1;
        }
    }
    let unconnected = doc["unconnected_items"].as_array().map_or(0, Vec::len);
    Some((counts, unconnected))
}

#[test]
fn the_port_matches_kicad_cli_per_violation_type() {
    let mut failures = Vec::new();
    for row in rows() {
        if row.needs_reference && !parity::require_reference_dir() {
            continue;
        }
        let Some((expected, expected_unconnected)) = oracle_counts(&row.stem) else {
            continue;
        };
        let (mut board, _) = load(&row);
        let (actual, actual_unconnected) = port_counts(&mut board);
        for kind in DrcViolationKind::ALL {
            let name = kind.kicad_type();
            if row.ignore.iter().any(|t| t == name) {
                continue;
            }
            let want = expected.get(name).copied().unwrap_or(0);
            let got = actual.get(name).copied().unwrap_or(0);
            if want != got {
                failures.push(format!(
                    "{}: {name}: kicad-cli {want}, port {got}",
                    row.stem
                ));
            }
        }
        if expected_unconnected != actual_unconnected
            && !row.ignore.iter().any(|t| t == "unconnected_items")
        {
            failures.push(format!(
                "{}: unconnected_items: kicad-cli {expected_unconnected}, port {actual_unconnected}",
                row.stem
            ));
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}
