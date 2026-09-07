use copper_board::prelude::*;
use copper_drc::DesignRulesChecker;
use copper_drc::report::{DrcCoordinates, DrcReportOptions, KiCadDrcPosition, KiCadDrcReport};
use copper_dsn::{BoardReadResult, CoordinateTransform, DsnReadOptions};

mod common;
use common::JAR_VERSION;

const DEV_BOARD: &str = "Issue575-drc_dev-board_4_hole_clearance_violations.dsn";
const DATE: &str = "2026-08-29T01:38:28.155317-07:00";

#[allow(clippy::excessive_precision)]
const DEV_BOARD_SCORE: f32 = 902.078369140625;

fn fixture_board(name: &str) -> (Board, CoordinateTransform) {
    let path = testkit::fixture(name);
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("cannot read fixture {}: {e}", path.display()));
    match copper_dsn::read_board(&bytes[..], None, Some(name), &DsnReadOptions::default()) {
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
            *board.unwrap_or_else(|| panic!("{name} produced no board")),
            coordinate_transform.unwrap_or_else(|| panic!("{name} produced no transform")),
        ),
        other => panic!("{name} did not read: {other:?}"),
    }
}

fn options(source: &str, date: &str, quality_score: Option<f32>) -> DrcReportOptions {
    DrcReportOptions {
        source: source.to_string(),
        coordinate_unit: "mm".to_string(),
        date: date.to_string(),
        router_version: JAR_VERSION.to_string(),
        quality_score,
    }
}

fn json_for(fixture: &str, quality_score: Option<f32>) -> String {
    let (mut board, transform) = fixture_board(fixture);
    let board_unit = board.communication.unit;
    let coords = DrcCoordinates {
        transform,
        board_unit,
    };
    DesignRulesChecker::new(&mut board)
        .report_to_json(&coords, &options(fixture, DATE, quality_score))
        .expect("the report serialises")
}

fn top_level_keys(json: &str) -> Vec<String> {
    json.lines()
        .filter_map(|line| Some(line.strip_prefix("  \"")?.split_once("\": ")?.0.to_string()))
        .collect()
}

fn line_with(json: &str, key: &str) -> String {
    json.lines()
        .find(|line| line.contains(key))
        .unwrap_or_else(|| panic!("no {key} in {json}"))
        .trim()
        .to_string()
}

fn bare_report(source: &str) -> KiCadDrcReport {
    KiCadDrcReport::new("mm", source, "Copperroute probe", "2026-08-29T00:00:00Z")
}

#[test]
fn key_order_is_kicads() {
    let with_score = json_for(DEV_BOARD, Some(DEV_BOARD_SCORE));
    assert_eq!(
        top_level_keys(&with_score),
        [
            "$schema",
            "coordinate_units",
            "date",
            "kicad_version",
            "copperroute_version",
            "source",
            "unconnected_items",
            "violations",
            "schematic_parity",
            "quality_score",
        ]
    );

    let without = json_for(DEV_BOARD, None);
    assert_eq!(top_level_keys(&without), top_level_keys(&with_score)[..9]);
    assert!(!without.contains("quality_score"));
}

#[test]
fn quality_score_is_plain_decimal_text() {
    let mut score = bare_report("probe");
    score.quality_score = Some(f64::from(DEV_BOARD_SCORE));
    let score_902 = line_with(&score.to_json().unwrap(), "quality_score");
    assert_eq!(score_902, "\"quality_score\": 902.078369140625");

    score.quality_score = Some(1.0e7);
    let score_1e7 = line_with(&score.to_json().unwrap(), "quality_score");
    assert_eq!(score_1e7, "\"quality_score\": 10000000");
}

#[test]
fn strings_are_written_without_html_or_separator_escapes() {
    let html = bare_report("<'&=>\"").to_json().unwrap();
    assert_eq!(line_with(&html, "\"source\""), "\"source\": \"<'&=>\\\"\",");
    let separators = bare_report("a\u{2028}b\u{2029}c").to_json().unwrap();
    assert_eq!(
        line_with(&separators, "\"source\""),
        "\"source\": \"a\\u2028b\\u2029c\","
    );
}

#[test]
fn schema_is_verbatim_and_schematic_parity_is_empty() {
    let json = bare_report("probe").to_json().unwrap();
    assert!(
        json.contains("  \"$schema\": \"https://schemas.kicad.org/drc.v1.json\",\n"),
        "{json}"
    );
    assert!(json.contains("\n  \"schematic_parity\": []\n}"), "{json}");
    for escape in ["\\u003c", "\\u003e", "\\u0026", "\\u003d", "\\u0027"] {
        assert!(!json.contains(escape), "{escape} in {json}");
    }
}

#[test]
fn dates_are_written_verbatim() {
    let mut report = bare_report("probe");
    report.date = DATE.to_string();
    let json = report.to_json().unwrap();
    assert!(
        json.contains(&format!("  \"date\": \"{DATE}\",\n")),
        "{json}"
    );
}

#[test]
fn report_to_json_is_generate_report_then_to_json() {
    let (mut board, transform) = fixture_board(DEV_BOARD);
    let board_unit = board.communication.unit;
    let coords = DrcCoordinates {
        transform,
        board_unit,
    };
    let options = options(DEV_BOARD, "2026-08-29T00:00:00Z", None);

    let mut checker = DesignRulesChecker::new(&mut board);
    let direct = checker
        .generate_report(&coords, &options)
        .to_json()
        .unwrap();
    let through = checker.report_to_json(&coords, &options).unwrap();
    assert_eq!(direct, through);
}

#[test]
fn a_non_finite_coordinate_is_refused() {
    let mut report = bare_report("probe");
    report.quality_score = Some(f64::NAN);
    assert!(report.to_json().is_err());

    let mut report = bare_report("probe");
    report.add_violation(copper_drc::report::KiCadDrcViolation::new(
        "clearance",
        "d",
        "error",
        vec![copper_drc::report::KiCadDrcViolationItem::new(
            "i",
            KiCadDrcPosition::new(0.0, f64::INFINITY),
            "1",
        )],
    ));
    assert!(report.to_json().is_err());
}

#[test]
fn a_hand_built_position_renders_through_the_formatter() {
    let mut report = bare_report("probe");
    report.add_violation(copper_drc::report::KiCadDrcViolation::new(
        "clearance",
        "d",
        "error",
        vec![copper_drc::report::KiCadDrcViolationItem::new(
            "i",
            KiCadDrcPosition::new(1.0e7, -72.18960000000001),
            "1",
        )],
    ));
    let json = report.to_json().unwrap();
    assert!(json.contains("\"x\": 10000000"), "{json}");
    assert!(json.contains("\"y\": -72.18960000000001"), "{json}");
}
