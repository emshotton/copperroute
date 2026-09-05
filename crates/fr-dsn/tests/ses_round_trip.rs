mod common;

use std::path::Path;

use fr_board::{Board, Item};
use fr_dsn::parser::scope_parameter::DsnReadOptions;
use fr_dsn::{BoardReadResult, CoordinateTransform, ses_reader, ses_writer};
use fr_geometry::FloatPoint;

fn load_board(name: &str) -> (Board, CoordinateTransform) {
    let path = Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../freerouting/fixtures/"
    ))
    .join(name);
    let file = std::fs::File::open(&path)
        .unwrap_or_else(|e| panic!("cannot open fixture {}: {e}", path.display()));
    let options = DsnReadOptions::default();
    let stem = name.strip_suffix(".dsn").unwrap_or(name);
    match fr_dsn::read_board(file, None, Some(stem), &options) {
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
            coordinate_transform
                .unwrap_or_else(|| panic!("{name} produced no coordinate transform")),
        ),
        BoardReadResult::Partial { diagnostic, .. } => {
            panic!("{name}: truncated: {diagnostic}")
        }
        BoardReadResult::ParseError { location, detail } => {
            panic!("{name}: parse error at {location}: {detail}")
        }
        BoardReadResult::IoError(e) => panic!("{name}: io error: {e}"),
    }
}

fn fixture_bytes(name: &str) -> Vec<u8> {
    let path = Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../freerouting/fixtures/"
    ))
    .join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("cannot read fixture {}: {e}", path.display()))
}

#[test]
fn ses_round_trip_preserves_wire_count() {
    let (mut board, ct) = load_board("Issue593-BBD_Mars-64.dsn");
    let ses = fixture_bytes("Issue593-BBD_Mars-64.ses");
    let imported = ses_reader::read(&ses[..], &mut board, &ct).expect("valid SES file");
    assert!(
        imported.wires_imported > 0,
        "At least one wire should be imported from the SES file; got: {}",
        imported.wires_imported
    );
    assert_eq!(
        0, imported.errors_encountered,
        "No errors should occur importing a valid SES file",
    );
}

#[test]
fn invalid_ses_is_an_error() {
    let (mut board, ct) = load_board("Issue143-rpi_splitter.dsn");
    let result = ses_reader::read(&b"garbage"[..], &mut board, &ct);
    assert!(
        result.is_err(),
        "SesReader::read must fail for non-SES input, got {result:?}"
    );
}

#[test]
fn writer_output_can_be_read_back_by_ses_reader() {
    let (mut source, ct) = load_board("Issue593-BBD_Mars-64.dsn");
    let ses = fixture_bytes("Issue593-BBD_Mars-64.ses");
    let original = ses_reader::read(&ses[..], &mut source, &ct).expect("valid SES file");
    assert!(
        original.wires_imported > 0,
        "Fixture SES must contain at least one wire"
    );

    let mut out: Vec<u8> = Vec::new();
    ses_writer::write(&source, &ct, &mut out, "round-trip.dsn").expect("write into a Vec");
    assert!(!out.is_empty(), "SesWriter must produce non-empty output");

    let (mut target, target_ct) = load_board("Issue593-BBD_Mars-64.dsn");
    let summary = ses_reader::read(&out[..], &mut target, &target_ct).expect("self-written SES");
    assert!(
        summary.wires_imported > 0,
        "Re-imported SES must contain at least one wire"
    );
    assert_eq!(
        0, summary.errors_encountered,
        "No errors expected on re-import of self-written SES",
    );
    assert_eq!(
        original.wires_imported, summary.wires_imported,
        "Round-trip wire count must match the original import count",
    );
}

#[test]
fn endpoint_snapping_is_stable_and_round_trips() {
    let (mut board, ct) = load_board("Issue593-BBD_Mars-64.dsn");
    let ses = fixture_bytes("Issue593-BBD_Mars-64.ses");
    let imported = ses_reader::read(&ses[..], &mut board, &ct).expect("valid SES file");
    assert!(imported.wires_imported > 0);

    let mut traces_with_drill_contacts = 0;
    let trace_ids: Vec<_> = board
        .items
        .iter()
        .filter(|(_, item)| matches!(item, Item::Trace(_)))
        .map(|(id, _)| *id)
        .collect();
    for trace_id in trace_ids {
        for start_side in [true, false] {
            let contacts = if start_side {
                board.trace_start_contacts(trace_id)
            } else {
                board.trace_end_contacts(trace_id)
            };
            let drill_contacts: Vec<_> = contacts
                .iter()
                .filter(|id| matches!(board.items.get(id), Some(Item::Via(_) | Item::Pin(_))))
                .collect();
            if drill_contacts.is_empty() {
                continue;
            }
            traces_with_drill_contacts += 1;
            let drill_centers: Vec<FloatPoint> = drill_contacts
                .into_iter()
                .filter_map(|id| board.drill_center(*id))
                .map(|center| center.to_float())
                .collect();
            let Some(Item::Trace(trace)) = board.items.get(&trace_id) else {
                unreachable!("filtered above")
            };
            let corner = if start_side {
                trace.first_corner()
            } else {
                trace.last_corner()
            };
            let corner = corner.expect("a trace has corners").to_float();
            if let Some(snapped) = ses_writer::snapped_endpoint(&board, trace_id, start_side) {
                assert!(
                    drill_centers
                        .iter()
                        .any(|center| center.distance(&snapped) < 0.5),
                    "snap target must be a contacted drill item center"
                );
                assert!(
                    corner.distance(&snapped) > 0.5,
                    "None is expected for already-centered endpoints"
                );
            }
        }
    }
    assert!(
        traces_with_drill_contacts > 0,
        "fixture must exercise drill-contacted endpoints"
    );

    let mut out: Vec<u8> = Vec::new();
    ses_writer::write(&board, &ct, &mut out, "Issue593-BBD_Mars-64.dsn").expect("write");
    let (mut fresh, fresh_ct) = load_board("Issue593-BBD_Mars-64.dsn");
    let reimported = ses_reader::read(&out[..], &mut fresh, &fresh_ct).expect("self-written SES");
    assert_eq!(
        imported.wires_imported, reimported.wires_imported,
        "snapping must not change the wire count on re-import",
    );
    assert_eq!(0, reimported.errors_encountered);
}

#[test]
fn issue742_ses_round_trips_without_errors() {
    let (mut board, ct) = load_board("Issue742-tastexx-pcb.dsn");
    let ses = fixture_bytes("Issue742-tastexx-pcb.ses");
    let imported = ses_reader::read(&ses[..], &mut board, &ct).expect("valid SES file");
    assert!(
        imported.wires_imported > 0,
        "fixture SES must contain routed wires"
    );
    assert_eq!(0, imported.errors_encountered);

    let mut out: Vec<u8> = Vec::new();
    ses_writer::write(&board, &ct, &mut out, "Issue742-tastexx-pcb.dsn").expect("write");
    let content = String::from_utf8(out.clone()).expect("SES output must be valid UTF-8");

    common::assert_balanced_scopes(&content);
    common::assert_unique_library_padstacks(&content);
    assert!(
        !content.contains("0.000"),
        "whole-degree rotations must not use trailing decimals"
    );
    assert!(
        content.contains("338.5") || content.contains(" front 339"),
        "fractional or rounded component rotations must be preserved in placement records"
    );

    let (mut fresh, fresh_ct) = load_board("Issue742-tastexx-pcb.dsn");
    let reimported = ses_reader::read(&out[..], &mut fresh, &fresh_ct).expect("self-written SES");
    assert_eq!(
        imported.wires_imported, reimported.wires_imported,
        "round-trip must preserve wire count",
    );
    assert_eq!(0, reimported.errors_encountered);
}

const JVM_SUMMARIES: [(&str, usize, usize, usize); 6] = [
    ("Issue026-J2_reference", 89, 10, 0),
    ("Issue313-FastTest", 129, 21, 0),
    ("Issue508-DAC2020_bm05", 198, 31, 0),
    ("Issue593-BBD_Mars-64", 215, 51, 0),
    ("Issue690-ecc83", 61, 0, 0),
    ("Issue742-tastexx-pcb", 37, 7, 0),
];

#[test]
fn ses_import_summaries_match_the_jvm() {
    for (stem, wires, vias, errors) in JVM_SUMMARIES {
        let (mut board, ct) = load_board(&format!("{stem}.dsn"));
        let ses = fixture_bytes(&format!("{stem}.ses"));
        let summary = ses_reader::read(&ses[..], &mut board, &ct)
            .unwrap_or_else(|e| panic!("{stem}.ses: {e}"));
        assert_eq!(
            (wires, vias, errors),
            (
                summary.wires_imported,
                summary.vias_imported,
                summary.errors_encountered
            ),
            "{stem}: (wires, vias, errors)"
        );
    }
}

#[test]
fn pcb_layer_path_lands_on_layer_0_and_counts_as_a_wire() {
    let ses = String::from_utf8(fixture_bytes("Issue026-J2_reference.ses")).expect("UTF-8");
    let mutated = ses.replacen("(path B.Cu 2500", "(path pcb 2500", 1);
    assert_ne!(ses, mutated, "the fixture must still contain a B.Cu path");

    let (mut board, ct) = load_board("Issue026-J2_reference.dsn");
    let summary = ses_reader::read(mutated.as_bytes(), &mut board, &ct).expect("valid SES file");
    assert_eq!(
        (89, 10, 0),
        (
            summary.wires_imported,
            summary.vias_imported,
            summary.errors_encountered
        ),
        "a pcb-layer path is imported, not counted as an error"
    );

    let mut actual = vec![format!(
        "summary wires={} vias={} errors={}",
        summary.wires_imported, summary.vias_imported, summary.errors_encountered
    )];
    actual.extend(common::dump(&board, &[]));
    let expected: Vec<String> = common::golden("Issue026-J2_reference-ses-items.txt")
        .into_iter()
        .map(|line| {
            if line.starts_with("item 84 PolylineTrace layer=1 ") {
                line.replacen("layer=1 ", "layer=0 ", 1)
            } else {
                line
            }
        })
        .collect();
    for (i, (a, e)) in actual.iter().zip(expected.iter()).enumerate() {
        assert_eq!(e, a, "line {} differs", i + 1);
    }
    assert_eq!(expected.len(), actual.len(), "line count differs");
}

#[test]
fn issue026_ses_import_matches_jvm_golden() {
    let (mut board, ct) = load_board("Issue026-J2_reference.dsn");
    let ses = fixture_bytes("Issue026-J2_reference.ses");
    let summary = ses_reader::read(&ses[..], &mut board, &ct).expect("valid SES file");

    let mut actual = vec![format!(
        "summary wires={} vias={} errors={}",
        summary.wires_imported, summary.vias_imported, summary.errors_encountered
    )];
    actual.extend(common::dump(&board, &[]));

    let expected = common::golden("Issue026-J2_reference-ses-items.txt");
    for (i, (a, e)) in actual.iter().zip(expected.iter()).enumerate() {
        assert_eq!(e, a, "line {} differs", i + 1);
    }
    assert_eq!(expected.len(), actual.len(), "line count differs");
}
