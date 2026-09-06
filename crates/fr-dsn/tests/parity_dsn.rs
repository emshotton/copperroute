use std::path::Path;

use fr_board::Board;
use fr_dsn::dsn_writer;
use fr_dsn::parser::scope_parameter::DsnReadOptions;
use fr_dsn::{BoardReadResult, CoordinateTransform};

fn read_fixture(path: &Path) -> (Board, CoordinateTransform) {
    let file = std::fs::File::open(path)
        .unwrap_or_else(|e| panic!("cannot open fixture {}: {e}", path.display()));
    let options = DsnReadOptions::default();
    let stem = design_name(path);
    match fr_dsn::read_board(file, None, Some(&stem), &options) {
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
            *board.unwrap_or_else(|| panic!("{} produced no board", path.display())),
            coordinate_transform
                .unwrap_or_else(|| panic!("{} produced no coordinate transform", path.display())),
        ),
        BoardReadResult::Partial { diagnostic, .. } => panic!("truncated: {diagnostic}"),
        BoardReadResult::ParseError { location, detail } => {
            panic!("parse error at {location}: {detail}")
        }
        BoardReadResult::IoError(e) => panic!("io error: {e}"),
    }
}

use parity::dsn_design_name as design_name;

fn assert_roundtrip_parity(stem: &str, relative_fixture: &str) {
    if !parity::require_reference_dir() {
        return;
    }
    let reference = parity::reference(stem, "roundtrip.dsn");
    if !parity::require_reference(&reference) {
        return;
    }
    let fixture = parity::reference_dir().join(relative_fixture);
    let (board, coordinate_transform) = read_fixture(&fixture);
    let mut actual: Vec<u8> = Vec::new();
    dsn_writer::write(
        &board,
        &coordinate_transform,
        &mut actual,
        &design_name(&fixture),
        false,
    )
    .expect("write must succeed into a Vec");
    let actual = String::from_utf8(actual).expect("DSN output must be valid UTF-8");
    parity::assert_text_parity(&actual, &reference);
}

#[test]
fn tutorial_board_roundtrip_matches_java() {
    assert_roundtrip_parity(
        "tutorial_board",
        "examples/tutorial_board/tutorial_board.dsn",
    );
}

#[test]
fn issue026_j2_reference_roundtrip_matches_java() {
    assert_roundtrip_parity(
        "Issue026-J2_reference",
        "fixtures/Issue026-J2_reference.dsn",
    );
}

#[test]
fn issue103_board_unrouted_roundtrip_matches_java() {
    assert_roundtrip_parity(
        "Issue103-Board-Unrouted",
        "fixtures/Issue103-Board-Unrouted.dsn",
    );
}

#[test]
fn issue143_rpi_splitter_roundtrip_matches_java() {
    assert_roundtrip_parity(
        "Issue143-rpi_splitter",
        "fixtures/Issue143-rpi_splitter.dsn",
    );
}

#[test]
fn issue413_test_roundtrip_matches_java() {
    assert_roundtrip_parity("Issue413-test", "fixtures/Issue413-test.dsn");
}

#[test]
fn issue110_relay_module_roundtrip_matches_java() {
    assert_roundtrip_parity("Issue110-RelayModule", "fixtures/Issue110-RelayModule.dsn");
}

#[test]
fn issue753_cpu_85_r104_roundtrip_matches_java() {
    assert_roundtrip_parity("Issue753-CPU-85_r104", "fixtures/Issue753-CPU-85_r104.dsn");
}

#[test]
fn every_reference_is_byte_for_byte_identical_to_java() {
    if !parity::require_reference_dir() {
        return;
    }
    for (stem, relative_fixture) in [
        (
            "tutorial_board",
            "examples/tutorial_board/tutorial_board.dsn",
        ),
        (
            "Issue026-J2_reference",
            "fixtures/Issue026-J2_reference.dsn",
        ),
        (
            "Issue103-Board-Unrouted",
            "fixtures/Issue103-Board-Unrouted.dsn",
        ),
        (
            "Issue143-rpi_splitter",
            "fixtures/Issue143-rpi_splitter.dsn",
        ),
        ("Issue413-test", "fixtures/Issue413-test.dsn"),
        ("Issue110-RelayModule", "fixtures/Issue110-RelayModule.dsn"),
        ("Issue753-CPU-85_r104", "fixtures/Issue753-CPU-85_r104.dsn"),
    ] {
        let reference_path = parity::reference(stem, "roundtrip.dsn");
        if !parity::require_reference(&reference_path) {
            continue;
        }
        let fixture = parity::reference_dir().join(relative_fixture);
        let (board, coordinate_transform) = read_fixture(&fixture);
        let mut actual: Vec<u8> = Vec::new();
        dsn_writer::write(&board, &coordinate_transform, &mut actual, stem, false)
            .expect("write must succeed into a Vec");
        let expected = std::fs::read(&reference_path).expect("reference must be readable");
        if actual == expected {
            continue;
        }
        let a = String::from_utf8_lossy(&actual);
        let e = String::from_utf8_lossy(&expected);
        let first_difference = a
            .lines()
            .zip(e.lines())
            .enumerate()
            .find(|(_, (la, le))| la != le)
            .map_or_else(
                || format!("no differing line; lengths {} vs {}", a.len(), e.len()),
                |(i, (la, le))| format!("line {}:\n  actual   {la:?}\n  expected {le:?}", i + 1),
            );
        panic!(
            "{stem}: not byte-identical to {}\n{first_difference}",
            reference_path.display()
        );
    }
}

#[test]
fn valid_header() {
    if !parity::require_reference_dir() {
        return;
    }
    let fixture = parity::reference_dir().join("fixtures/Issue143-rpi_splitter.dsn");
    let (board, ct) = read_fixture(&fixture);
    let mut out: Vec<u8> = Vec::new();
    dsn_writer::write(&board, &ct, &mut out, "test", false).expect("write");
    let content = String::from_utf8(out).expect("UTF-8");
    assert!(
        content.starts_with("(pcb"),
        "DSN output must start with (pcb"
    );
    assert!(
        content.contains("(structure"),
        "DSN output must contain (structure scope"
    );
}

#[test]
fn roundtrip_preserves_layer_count() {
    if !parity::require_reference_dir() {
        return;
    }
    let fixture = parity::reference_dir().join("fixtures/Issue143-rpi_splitter.dsn");
    let (original, ct) = read_fixture(&fixture);
    let original_layers = original.get_layer_count();
    let mut out: Vec<u8> = Vec::new();
    dsn_writer::write(&original, &ct, &mut out, "roundtrip", false).expect("write");
    let options = DsnReadOptions::default();
    let reloaded = match fr_dsn::read_board(out.as_slice(), None, Some("roundtrip"), &options) {
        BoardReadResult::Success { board, .. } | BoardReadResult::OutlineMissing { board, .. } => {
            *board.expect("re-read must produce a board")
        }
        BoardReadResult::Partial { diagnostic, .. } => panic!("re-read truncated: {diagnostic}"),
        BoardReadResult::ParseError { location, detail } => {
            panic!("re-read failed at {location}: {detail}")
        }
        BoardReadResult::IoError(e) => panic!("re-read io error: {e}"),
    };
    assert_eq!(original_layers, reloaded.get_layer_count());
}

#[test]
fn compat_mode_produces_output() {
    if !parity::require_reference_dir() {
        return;
    }
    let fixture = parity::reference_dir().join("fixtures/Issue143-rpi_splitter.dsn");
    let (board, ct) = read_fixture(&fixture);
    let mut out: Vec<u8> = Vec::new();
    dsn_writer::write(&board, &ct, &mut out, "compat-test", true).expect("write");
    let content = String::from_utf8(out).expect("UTF-8");
    assert!(
        content.starts_with("(pcb"),
        "Compat-mode DSN output must start with (pcb"
    );
}

#[test]
fn output_is_non_empty() {
    if !parity::require_reference_dir() {
        return;
    }
    let fixture = parity::reference_dir().join("fixtures/Issue143-rpi_splitter.dsn");
    let (board, ct) = read_fixture(&fixture);
    let mut out: Vec<u8> = Vec::new();
    dsn_writer::write(&board, &ct, &mut out, "flush-test", false).expect("write");
    assert!(
        !out.is_empty(),
        "output must contain data after write (flush must have occurred)"
    );
}

#[test]
fn compat_mode_writes_paths_where_the_default_writes_polyline_paths() {
    if !parity::require_reference_dir() {
        return;
    }
    let fixture = parity::reference_dir().join("fixtures/Issue413-test.dsn");
    if !fixture.exists() {
        eprintln!("SKIP: {} missing", fixture.display());
        return;
    }
    let (board, ct) = read_fixture(&fixture);
    assert!(
        !board.get_traces().is_empty(),
        "fixture must have traces for this test to mean anything"
    );

    let mut default_mode: Vec<u8> = Vec::new();
    dsn_writer::write(&board, &ct, &mut default_mode, "Issue413-test", false).expect("write");
    let default_mode = String::from_utf8(default_mode).expect("UTF-8");

    let mut compat: Vec<u8> = Vec::new();
    dsn_writer::write(&board, &ct, &mut compat, "Issue413-test", true).expect("write");
    let compat = String::from_utf8(compat).expect("UTF-8");

    assert!(default_mode.contains("(polyline_path "));
    assert!(!default_mode.contains("(path "));
    assert!(compat.contains("(path "));
    assert!(!compat.contains("(polyline_path "));
    assert!(compat.contains("(string_quote "));
    assert!(compat.contains("(space_in_quoted_tokens on)"));
}
