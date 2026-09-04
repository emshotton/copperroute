use fr_board::prelude::*;
use fr_drc::DesignRulesChecker;
use fr_drc::report::{
    DrcCoordinates, DrcJsonFlavor, DrcReportOptions, KiCadDrcPosition, KiCadDrcReport,
};
use fr_dsn::{BoardReadResult, CoordinateTransform, DsnReadOptions};


mod common;
use common::JAR_VERSION;

const DEV_BOARD: &str = "Issue575-drc_dev-board_4_hole_clearance_violations.dsn";
const BBD_MARS_64: &str = "Issue575-drc_BBD_Mars-64_6_track_1_hole_clearance_violations.dsn";
const EMPTY_BOARD: &str = "empty_board.dsn";

const GOLDEN_FIXTURES: [&str; 3] = [DEV_BOARD, BBD_MARS_64, EMPTY_BOARD];

#[allow(clippy::excessive_precision)]
const DEV_BOARD_SCORE: f32 = 902.078369140625;

fn data_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data")
}

fn golden(fixture: &str) -> String {
    let stem = fixture.strip_suffix(".dsn").expect("a .dsn fixture name");
    let path = data_dir().join(format!("{stem}.head.json"));
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read golden {}: {e}", path.display()))
}

fn date_of(golden: &str) -> String {
    for line in golden.lines() {
        if let Some(rest) = line.trim().strip_prefix("\"date\": \"") {
            return rest.trim_end_matches(',').trim_matches('"').to_string();
        }
    }
    panic!("no date in the golden");
}

fn fixture_board(name: &str) -> (Board, CoordinateTransform) {
    let path = parity::fixture(name);
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("cannot read fixture {}: {e}", path.display()));
    match fr_dsn::read_board(&bytes[..], None, Some(name), &DsnReadOptions::default()) {
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
        freerouting_version: JAR_VERSION.to_string(),
        quality_score,
    }
}

fn json_for(
    fixture: &str,
    date: &str,
    quality_score: Option<f32>,
    flavor: DrcJsonFlavor,
) -> String {
    let (mut board, transform) = fixture_board(fixture);
    let board_unit = board.communication.unit;
    let coords = DrcCoordinates {
        transform,
        board_unit,
    };
    DesignRulesChecker::new(&mut board)
        .report_to_json(&coords, &options(fixture, date, quality_score), flavor)
        .expect("the report serialises")
}

fn normalised_json_for(fixture: &str, date: &str, flavor: DrcJsonFlavor) -> String {
    let (mut board, transform) = fixture_board(fixture);
    let board_unit = board.communication.unit;
    let coords = DrcCoordinates {
        transform,
        board_unit,
    };
    let mut report =
        DesignRulesChecker::new(&mut board).generate_report(&coords, &options(fixture, date, None));
    for entry in &mut report.unconnected_items {
        entry
            .items
            .sort_by_key(|item| item.uuid.parse::<i64>().expect("a numeric uuid"));
    }
    report.to_json(flavor).expect("the report serialises")
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
    KiCadDrcReport::new("mm", source, "Freerouting probe", "2026-08-29T00:00:00Z")
}


#[test]
fn head_flavor_key_order() {
    if !parity::require_java_dir() {
        return;
    }
    let date = date_of(&golden(DEV_BOARD));

    let with_score = json_for(
        DEV_BOARD,
        &date,
        Some(DEV_BOARD_SCORE),
        DrcJsonFlavor::default(),
    );
    assert_eq!(
        top_level_keys(&with_score),
        [
            "$schema",
            "coordinateUnits",
            "date",
            "kicadVersion",
            "freeroutingVersion",
            "source",
            "unconnectedItems",
            "violations",
            "schematicParity",
            "qualityScore",
        ]
    );

    let without = json_for(DEV_BOARD, &date, None, DrcJsonFlavor::FreeroutingHead);
    assert_eq!(top_level_keys(&without), top_level_keys(&with_score)[..9]);
    assert!(!without.contains("qualityScore"));
}

#[test]
fn kicad_flavor_key_order() {
    if !parity::require_java_dir() {
        return;
    }
    let date = date_of(&golden(DEV_BOARD));
    let json = json_for(
        DEV_BOARD,
        &date,
        Some(DEV_BOARD_SCORE),
        DrcJsonFlavor::KiCad,
    );
    assert_eq!(
        top_level_keys(&json),
        [
            "$schema",
            "coordinate_units",
            "date",
            "kicad_version",
            "freerouting_version",
            "source",
            "unconnected_items",
            "violations",
            "schematic_parity",
            "quality_score",
        ]
    );
}

#[test]
fn flavors_differ_only_in_the_key_tables_eight_strings() {
    if !parity::require_java_dir() {
        return;
    }
    const REWRITES: [(&str, &str); 7] = [
        ("coordinateUnits", "coordinate_units"),
        ("kicadVersion", "kicad_version"),
        ("freeroutingVersion", "freerouting_version"),
        ("unconnectedItems", "unconnected_items"),
        ("schematicParity", "schematic_parity"),
        ("qualityScore", "quality_score"),
        ("holeClearance", "hole_clearance"),
    ];

    for fixture in GOLDEN_FIXTURES {
        let date = date_of(&golden(fixture));
        let head = json_for(
            fixture,
            &date,
            Some(DEV_BOARD_SCORE),
            DrcJsonFlavor::FreeroutingHead,
        );
        let kicad = json_for(fixture, &date, Some(DEV_BOARD_SCORE), DrcJsonFlavor::KiCad);

        let mut renamed = head.clone();
        for (from, to) in REWRITES {
            renamed = renamed.replace(&format!("\"{from}\""), &format!("\"{to}\""));
        }
        assert_eq!(renamed, kicad, "{fixture}");

        assert_ne!(head, kicad, "{fixture}");
    }
}


#[test]
fn head_flavor_is_the_jvms_gson_bytes() {
    if !parity::require_java_dir() {
        return;
    }
    for fixture in GOLDEN_FIXTURES {
        let expected = golden(fixture);
        let actual =
            normalised_json_for(fixture, &date_of(&expected), DrcJsonFlavor::FreeroutingHead);
        assert_eq!(actual, expected, "{fixture}");
    }
}

#[test]
fn quality_score_is_java_double_text() {
    let expected = std::fs::read_to_string(data_dir().join("gson-escapes.txt"))
        .expect("cannot read tests/data/gson-escapes.txt");

    let mut score = bare_report("probe");
    score.quality_score = Some(f64::from(DEV_BOARD_SCORE));
    let score_902 = line_with(
        &score.to_json(DrcJsonFlavor::FreeroutingHead).unwrap(),
        "qualityScore",
    );

    score.quality_score = Some(1.0e7);
    let score_1e7 = line_with(
        &score.to_json(DrcJsonFlavor::FreeroutingHead).unwrap(),
        "qualityScore",
    );

    let html = bare_report("<'&=>\"")
        .to_json(DrcJsonFlavor::FreeroutingHead)
        .unwrap();
    let separators = bare_report("a\u{2028}b\u{2029}c")
        .to_json(DrcJsonFlavor::FreeroutingHead)
        .unwrap();

    let actual = format!(
        "qualityScore-902\t{score_902}\n\
         qualityScore-1e7\t{score_1e7}\n\
         source-html\t{}\n\
         source-separators\t{}\n",
        line_with(&html, "\"source\""),
        line_with(&separators, "\"source\""),
    );
    assert_eq!(actual, expected);

    assert!(score_1e7.ends_with("1.0E7"), "{score_1e7}");
    assert!(score_902.ends_with("902.078369140625"), "{score_902}");
}

#[test]
fn schema_is_verbatim_and_schematic_parity_is_empty() {
    let json = bare_report("probe")
        .to_json(DrcJsonFlavor::FreeroutingHead)
        .unwrap();
    assert!(
        json.contains("  \"$schema\": \"https://schemas.kicad.org/drc.v1.json\",\n"),
        "{json}"
    );
    assert!(json.contains("\n  \"schematicParity\": []\n}"), "{json}");
    for escape in ["\\u003c", "\\u003e", "\\u0026", "\\u003d", "\\u0027"] {
        assert!(!json.contains(escape), "{escape} in {json}");
    }
}

#[test]
fn dates_are_iso_offset() {
    const SAMPLED: &str = "2026-08-29T01:38:28.155317-07:00";
    let mut report = bare_report("probe");
    report.date = SAMPLED.to_string();
    let json = report.to_json(DrcJsonFlavor::FreeroutingHead).unwrap();
    assert!(
        json.contains(&format!("  \"date\": \"{SAMPLED}\",\n")),
        "{json}"
    );

    if parity::require_java_dir() {
        let date = date_of(&golden(DEV_BOARD));
        assert_eq!(date.len(), SAMPLED.len(), "{date}");
        assert!(
            date.contains('T') && (date.contains('+') || date[10..].contains('-')),
            "{date}"
        );
    }
}


#[test]
fn kicad_flavor_matches_the_real_kicad_schema() {
    if !parity::require_java_dir() {
        return;
    }
    const KICAD_FIXTURE: &str =
        "Issue575-drc_Natural_Tone_Preamp_7_unconnected_items-kicad_drc.json";
    let kicad: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(parity::fixture(KICAD_FIXTURE)).unwrap())
            .expect("the KiCad fixture parses");

    let date = date_of(&golden(DEV_BOARD));
    let ours: serde_json::Value = serde_json::from_str(&json_for(
        DEV_BOARD,
        &date,
        Some(DEV_BOARD_SCORE),
        DrcJsonFlavor::KiCad,
    ))
    .expect("our document parses");

    let keys = |v: &serde_json::Value| -> Vec<String> {
        v.as_object()
            .expect("an object")
            .keys()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect()
    };

    let theirs = keys(&kicad);
    let mine = keys(&ours);
    let extra: Vec<&String> = mine.iter().filter(|k| !theirs.contains(k)).collect();
    assert_eq!(extra, ["freerouting_version", "quality_score"]);
    let missing: Vec<&String> = theirs.iter().filter(|k| !mine.contains(k)).collect();
    assert!(missing.is_empty(), "{missing:?}");

    let sample = |v: &serde_json::Value| -> (Vec<String>, Vec<String>) {
        for list in ["violations", "unconnected_items"] {
            if let Some(first) = v[list].as_array().and_then(|a| a.first()) {
                let item = first["items"].as_array().and_then(|a| a.first()).unwrap();
                return (keys(first), keys(item));
            }
        }
        panic!("no violation to sample");
    };
    assert_eq!(sample(&kicad), sample(&ours));

    let mut kicad_types = std::collections::BTreeSet::new();
    for name in [
        KICAD_FIXTURE,
        "Issue575-drc_dev-board_4_hole_clearance_violations-kicad_drc.json",
        "Issue575-drc_BBD_Mars-64_6_track_1_hole_clearance_violations-kicad_drc.json",
    ] {
        let doc: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(parity::fixture(name)).unwrap()).unwrap();
        for list in ["violations", "unconnected_items"] {
            for entry in doc[list].as_array().into_iter().flatten() {
                kicad_types.insert(entry["type"].as_str().unwrap().to_string());
            }
        }
    }
    for ours in [
        "clearance",
        "hole_clearance",
        "unconnected_items",
        "track_dangling",
        "via_dangling",
    ] {
        assert!(kicad_types.contains(ours), "KiCad never writes {ours}");
    }
    for head_only in ["holeClearance", "unconnectedItems"] {
        assert!(!kicad_types.contains(head_only), "{head_only}");
    }
}


#[test]
fn report_to_json_is_generate_report_then_to_json() {
    if !parity::require_java_dir() {
        return;
    }
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
        .to_json(DrcJsonFlavor::FreeroutingHead)
        .unwrap();
    let through = checker
        .report_to_json(&coords, &options, DrcJsonFlavor::FreeroutingHead)
        .unwrap();
    assert_eq!(direct, through);
}

#[test]
fn a_non_finite_coordinate_is_refused_where_gson_throws() {
    let mut report = bare_report("probe");
    report.quality_score = Some(f64::NAN);
    assert!(report.to_json(DrcJsonFlavor::FreeroutingHead).is_err());

    let mut report = bare_report("probe");
    report.add_violation(fr_drc::report::KiCadDrcViolation::new(
        "clearance",
        "d",
        "error",
        vec![fr_drc::report::KiCadDrcViolationItem::new(
            "i",
            KiCadDrcPosition::new(0.0, f64::INFINITY),
            "1",
        )],
    ));
    assert!(report.to_json(DrcJsonFlavor::KiCad).is_err());
}

#[test]
fn a_hand_built_position_renders_through_the_java_formatter() {
    let mut report = bare_report("probe");
    report.add_violation(fr_drc::report::KiCadDrcViolation::new(
        "clearance",
        "d",
        "error",
        vec![fr_drc::report::KiCadDrcViolationItem::new(
            "i",
            KiCadDrcPosition::new(1.0e7, -72.18960000000001),
            "1",
        )],
    ));
    let json = report.to_json(DrcJsonFlavor::FreeroutingHead).unwrap();
    assert!(json.contains("\"x\": 1.0E7"), "{json}");
    assert!(json.contains("\"y\": -72.18960000000001"), "{json}");
}
