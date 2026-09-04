use fr_board::prelude::*;
use fr_drc::DesignRulesChecker;
use fr_drc::report::{DrcCoordinates, DrcJsonFlavor, DrcReportOptions};
use fr_dsn::{BoardReadResult, CoordinateTransform, DsnReadOptions};


struct Row {
    stem: String,
    dsn: String,
    rules: Option<String>,
    ses: Option<String>,
}

fn rows() -> Vec<Row> {
    let path = parity::workspace_root().join("tests/reference/drc-fixtures.txt");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let mut fields = line.split('|');
            let mut next = || fields.next().unwrap_or_default().trim().to_string();
            let (stem, dsn, rules, ses) = (next(), next(), next(), next());
            Row {
                stem,
                dsn,
                rules: (!rules.is_empty()).then_some(rules),
                ses: (!ses.is_empty()).then_some(ses),
            }
        })
        .collect()
}

fn row(stem: &str) -> Row {
    rows()
        .into_iter()
        .find(|row| row.stem == stem)
        .unwrap_or_else(|| panic!("{stem} is not in tests/reference/drc-fixtures.txt"))
}

fn base_name(path: &str) -> &str {
    path.rsplit('/').next().expect("a non-empty path")
}


fn read_dsn(rel_path: &str) -> (Board, CoordinateTransform) {
    let path = parity::java_dir().join(rel_path);
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("cannot read fixture {}: {e}", path.display()));
    let name = base_name(rel_path);
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

fn load_board(row: &Row) -> (Board, CoordinateTransform) {
    let (mut board, transform) = read_dsn(&row.dsn);

    if let Some(rules) = &row.rules {
        let path = parity::java_dir().join(rules);
        let file = std::fs::File::open(&path)
            .unwrap_or_else(|e| panic!("cannot open {}: {e}", path.display()));
        let design_name = base_name(&row.dsn)
            .strip_suffix(".dsn")
            .unwrap_or_else(|| base_name(&row.dsn));
        let read = fr_dsn::rules_reader::read(file, design_name, &mut board, &transform, None)
            .unwrap_or_else(|e| panic!("{} did not read: {e:?}", path.display()));
        assert!(read, "{} was rejected by the rules reader", path.display());
    }

    if let Some(ses) = &row.ses {
        let path = parity::java_dir().join(ses);
        let file = std::fs::File::open(&path)
            .unwrap_or_else(|e| panic!("cannot open {}: {e}", path.display()));
        let summary = fr_dsn::ses_reader::read(file, &mut board, &transform)
            .unwrap_or_else(|e| panic!("{} did not read: {e:?}", path.display()));
        assert_eq!(
            summary.errors_encountered,
            0,
            "{} imported with errors",
            path.display()
        );
    }

    (board, transform)
}

fn port_json(row: &Row, reference: &parity::DrcReportDoc) -> String {
    let source = base_name(&row.dsn).to_string();
    assert_eq!(
        source, reference.source,
        "the CLI's `source` is the input file's base name (Freerouting.java:339)"
    );
    let version = reference
        .freerouting_version
        .strip_prefix("Freerouting ")
        .unwrap_or_else(|| {
            panic!(
                "`generateReport` prefixes \"Freerouting \" (DesignRulesChecker.java:212-213); \
                 got {}",
                reference.freerouting_version
            )
        })
        .to_string();
    let options = DrcReportOptions {
        source,
        coordinate_unit: "mm".to_string(),
        date: reference
            .date
            .clone()
            .expect("the reference carries a date"),
        freerouting_version: version,
        quality_score: reference.quality_score.map(|score| score as f32),
    };

    let (mut board, transform) = load_board(row);
    let coords = DrcCoordinates {
        board_unit: board.communication.unit,
        transform,
    };
    DesignRulesChecker::new(&mut board)
        .report_to_json(&coords, &options, DrcJsonFlavor::FreeroutingHead)
        .expect("the report serialises")
}


fn reference_path(stem: &str) -> std::path::PathBuf {
    parity::reference(stem, "drc.json")
}

fn read_reference(stem: &str) -> parity::DrcReportDoc {
    let path = reference_path(stem);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read reference {}: {e}", path.display()));
    parity::parse_drc_json(&text)
        .unwrap_or_else(|e| panic!("{} is not a HEAD-flavor DRC report: {e}", path.display()))
}

fn assert_normalised_parity(stem: &str, port: &str, expected: &mut parity::DrcReportDoc) {
    let expected_text = parity::normalize_drc_doc(expected).expect("the reference re-serialises");
    let scratch = parity::workspace_root()
        .join("tests/reference/_scratch")
        .join(stem);
    std::fs::create_dir_all(&scratch).expect("the scratch directory is writable");
    let expected_path = scratch.join("drc.expected.json");
    std::fs::write(&expected_path, &expected_text).expect("the scratch file is writable");

    let actual = parity::normalize_drc_json(port)
        .unwrap_or_else(|e| panic!("{stem}: the port's document does not parse: {e}"));
    let _ = std::fs::write(scratch.join("drc.actual.json"), &actual);
    parity::assert_text_parity(&actual, &expected_path);
}

fn check(stem: &str) {
    if !parity::require_java_dir() {
        return;
    }
    if !parity::require_reference(&reference_path(stem)) {
        return;
    }
    let row = row(stem);
    let mut reference = read_reference(stem);
    let port = port_json(&row, &reference);
    assert_normalised_parity(stem, &port, &mut reference);
}

// None of them is `#[cfg_attr(debug_assertions, ignore)]`, which the task brief suggested for the

#[test]
fn drc_dev_board() {
    check("drc-dev-board");
}

#[test]
fn drc_bbd_mars_64() {
    check("drc-bbd-mars-64");
}

#[test]
fn drc_issue593_rules() {
    check("drc-issue593-rules");
}

#[test]
fn drc_issue593_ses() {
    check("drc-issue593-ses");
}

#[test]
fn drc_issue753_cpu85() {
    check("drc-issue753-cpu85");
}

#[test]
fn drc_issue110_relay() {
    check("drc-issue110-relay");
}

#[test]
fn drc_tutorial_board() {
    check("drc-tutorial-board");
}

#[test]
fn natural_tone_preamp_is_the_reference_minus_three_dangling_tracks() {
    if !parity::require_java_dir() {
        return;
    }
    let stem = "drc-natural-tone-preamp";
    if !parity::require_reference(&reference_path(stem)) {
        return;
    }
                const EXTRA_DANGLING_UUIDS: [&str; 3] = ["1909", "1696", "1242"];

    let mut reference = read_reference(stem);
    assert_eq!(
        reference.violations.len(),
        115,
        "the committed reference must be the -XX:hashCode=2 run"
    );

    let mut removed: Vec<String> = Vec::new();
    reference.violations.retain(|violation| {
        let drop = violation.kind == "track_dangling"
            && violation.items.len() == 1
            && EXTRA_DANGLING_UUIDS.contains(&violation.items[0].uuid.as_str());
        if drop {
            removed.push(violation.items[0].uuid.clone());
        }
        !drop
    });
    assert_eq!(
        removed,
        EXTRA_DANGLING_UUIDS.map(String::from),
        "all three pinned entries must be present, in the reference's own order"
    );
    assert_eq!(reference.violations.len(), 112);

    let row = row(stem);
    let port = port_json(&row, &reference);
    assert_normalised_parity(stem, &port, &mut reference);
}


#[test]
fn references_are_from_the_head_jar() {
    for row in rows() {
        let meta_path = parity::reference(&row.stem, "drc.meta.txt");
        if !parity::require_reference(&meta_path) {
            continue;
        }
        let meta = std::fs::read_to_string(&meta_path).expect("the meta file is readable");
        if parity::declared_lane(&meta).starts_with("port") {
            parity::assert_port_lane_provenance(&meta, &row.stem);
            continue;
        }
        assert!(
            meta.contains("freerouting-current-executable.jar"),
            "{}: not generated from the clone's HEAD build:\n{meta}",
            row.stem
        );
        assert!(
            meta.contains("Freerouting 2.3.1-SNAPSHOT"),
            "{}: not generated from a 2.3.1-SNAPSHOT jar:\n{meta}",
            row.stem
        );
        assert!(
            meta.contains("-XX:hashCode=2"),
            "{}: not generated under the constant-hash mode:\n{meta}",
            row.stem
        );
    }
}

#[test]
fn every_reference_carries_a_quality_score_and_the_head_key_spelling() {
    for row in rows() {
        if !parity::require_reference(&reference_path(&row.stem)) {
            continue;
        }
        let reference = read_reference(&row.stem);
        assert!(
            reference.quality_score.is_some(),
            "{}: no qualityScore",
            row.stem
        );
        assert_eq!(reference.schema, "https://schemas.kicad.org/drc.v1.json");
        assert_eq!(reference.kicad_version, "N/A");
        assert_eq!(reference.coordinate_units, "mm");
    }
}

#[test]
fn no_description_carries_a_comma_decimal() {
    for row in rows() {
        if !parity::require_reference(&reference_path(&row.stem)) {
            continue;
        }
        let reference = read_reference(&row.stem);
        for violation in reference
            .violations
            .iter()
            .chain(&reference.unconnected_items)
        {
            let bytes = violation.description.as_bytes();
            let comma_decimal = bytes
                .windows(3)
                .any(|w| w[1] == b',' && w[0].is_ascii_digit() && w[2].is_ascii_digit());
            assert!(
                !comma_decimal,
                "{}: comma decimal in {:?} — regenerate with -Duser.language=en \
                 -Duser.country=US",
                row.stem, violation.description
            );
        }
    }
}
