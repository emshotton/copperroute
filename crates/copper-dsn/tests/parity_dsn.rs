use std::path::Path;

use copper_board::Board;
use copper_dsn::dsn_writer;
use copper_dsn::parser::scope_parameter::DsnReadOptions;
use copper_dsn::{BoardReadResult, CoordinateTransform};

fn read_fixture(path: &Path) -> (Board, CoordinateTransform) {
    let file = std::fs::File::open(path)
        .unwrap_or_else(|e| panic!("cannot open fixture {}: {e}", path.display()));
    let options = DsnReadOptions::default();
    let stem = design_name(path);
    match copper_dsn::read_board(file, None, Some(&stem), &options) {
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

use testkit::dsn_design_name as design_name;

#[test]
fn valid_header() {
    let fixture = testkit::corpus_dir().join("fixtures/Issue143-rpi_splitter.dsn");
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
    let fixture = testkit::corpus_dir().join("fixtures/Issue143-rpi_splitter.dsn");
    let (original, ct) = read_fixture(&fixture);
    let original_layers = original.get_layer_count();
    let mut out: Vec<u8> = Vec::new();
    dsn_writer::write(&original, &ct, &mut out, "roundtrip", false).expect("write");
    let options = DsnReadOptions::default();
    let reloaded = match copper_dsn::read_board(out.as_slice(), None, Some("roundtrip"), &options) {
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
    let fixture = testkit::corpus_dir().join("fixtures/Issue143-rpi_splitter.dsn");
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
    let fixture = testkit::corpus_dir().join("fixtures/Issue143-rpi_splitter.dsn");
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
    let fixture = testkit::corpus_dir().join("fixtures/Issue413-test.dsn");
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
