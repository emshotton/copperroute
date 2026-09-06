use std::path::Path;

mod common;

use fr_board::{Board, Item};
use fr_dsn::parser::scope_parameter::DsnReadOptions;
use fr_dsn::{BoardReadResult, CoordinateTransform, format_placement_rotation, ses_writer};

const FIXTURES: [(&str, &str); 4] = [
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
];

const WIRE_BEARING_STEM: &str = "Issue413-test";

const RULING_G_FIXTURES: [(&str, &str); 3] = [
    ("Issue413-test", "fixtures/Issue413-test.dsn"),
    ("Issue110-RelayModule", "fixtures/Issue110-RelayModule.dsn"),
    ("Issue753-CPU-85_r104", "fixtures/Issue753-CPU-85_r104.dsn"),
];

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

fn design_name(path: &Path) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .strip_suffix(".dsn")
        .unwrap_or_default()
        .to_string()
}

fn write_ses(relative_fixture: &str, design: &str) -> String {
    let fixture = parity::reference_dir().join(relative_fixture);
    let (board, ct) = read_fixture(&fixture);
    let mut actual: Vec<u8> = Vec::new();
    ses_writer::write(&board, &ct, &mut actual, design).expect("write must succeed into a Vec");
    String::from_utf8(actual).expect("SES output must be valid UTF-8")
}

fn assert_ses_parity(stem: &str, relative_fixture: &str) {
    if !parity::require_reference_dir() {
        return;
    }
    let reference = parity::reference(stem, "unrouted.ses");
    if !parity::require_reference(&reference) {
        return;
    }
    let actual = write_ses(relative_fixture, stem);
    parity::assert_text_parity(&actual, &reference);
}

#[test]
fn tutorial_board_ses_matches_java() {
    assert_ses_parity(FIXTURES[0].0, FIXTURES[0].1);
}

#[test]
fn issue026_j2_reference_ses_matches_java() {
    assert_ses_parity(FIXTURES[1].0, FIXTURES[1].1);
}

#[test]
fn issue103_board_unrouted_ses_matches_java() {
    assert_ses_parity(FIXTURES[2].0, FIXTURES[2].1);
}

#[test]
fn issue143_rpi_splitter_ses_matches_java() {
    assert_ses_parity(FIXTURES[3].0, FIXTURES[3].1);
}

#[test]
fn every_reference_is_byte_for_byte_identical_to_java() {
    if !parity::require_reference_dir() {
        return;
    }
    for (stem, relative_fixture) in FIXTURES.iter().chain(RULING_G_FIXTURES.iter()).copied() {
        let reference_path = parity::reference(stem, "unrouted.ses");
        if !parity::require_reference(&reference_path) {
            continue;
        }
        let actual = write_ses(relative_fixture, stem).into_bytes();
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
fn no_reference_contains_a_wire() {
    for (stem, _) in FIXTURES
        .iter()
        .chain(RULING_G_FIXTURES.iter())
        .copied()
        .filter(|(stem, _)| *stem != WIRE_BEARING_STEM)
    {
        let reference_path = parity::reference(stem, "unrouted.ses");
        if !parity::require_reference(&reference_path) {
            continue;
        }
        let reference = std::fs::read_to_string(&reference_path).expect("reference readable");
        assert!(
            !reference.contains("(wire"),
            "{stem}: reference unexpectedly has a wire — the parity claim in this file's docs \
             needs revisiting"
        );
    }
}

#[test]
fn valid_header() {
    if !parity::require_reference_dir() {
        return;
    }
    let content = write_ses(
        "fixtures/Issue026-J2_reference.dsn",
        "Issue026-J2_reference.dsn",
    );
    assert!(
        content.starts_with("(session "),
        "SES output must start with '(session '; got: {}",
        &content[..content.len().min(50)]
    );
    assert!(
        content.contains("(routes"),
        "SES output must contain '(routes' scope"
    );
}

#[test]
fn output_is_non_empty() {
    if !parity::require_reference_dir() {
        return;
    }
    let content = write_ses("fixtures/Issue143-rpi_splitter.dsn", "test.dsn");
    assert!(
        !content.is_empty(),
        "SesWriter must write data to the stream"
    );
}

#[test]
fn placement_rotation_formatting_matches_kicad_style() {
    assert_eq!(format_placement_rotation(0.0), "0");
    assert_eq!(format_placement_rotation(339.0), "339");
    assert_eq!(format_placement_rotation(338.5), "338.5");
    assert_eq!(format_placement_rotation(-45.25), "-45.25");
}

#[test]
fn issue742_placement_and_library_out_are_well_formed() {
    if !parity::require_reference_dir() {
        return;
    }
    let fixture = parity::reference_dir().join("fixtures/Issue742-tastexx-pcb.dsn");
    if !fixture.exists() {
        eprintln!("SKIP: {} missing", fixture.display());
        return;
    }
    let content = write_ses(
        "fixtures/Issue742-tastexx-pcb.dsn",
        "Issue742-tastexx-pcb.dsn",
    );
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
}

#[test]
fn ruling_g_references_do_carry_wires() {
    let reference_path = parity::reference(WIRE_BEARING_STEM, "unrouted.ses");
    if !parity::require_reference(&reference_path) {
        return;
    }
    let reference = std::fs::read_to_string(&reference_path).expect("reference readable");
    assert_eq!(
        reference.matches("(wire").count(),
        11,
        "Issue413-test's SES reference is the only committed one that exercises the trace path; \
         if this count changes the reference was regenerated from a different fixture"
    );
}

#[test]
fn every_fixture_is_balanced_with_unique_library_padstacks() {
    if !parity::require_reference_dir() {
        return;
    }
    for (stem, relative_fixture) in FIXTURES.iter().chain(RULING_G_FIXTURES.iter()).copied() {
        let content = write_ses(relative_fixture, stem);
        common::assert_balanced_scopes(&content);
        common::assert_unique_library_padstacks(&content);
    }
}

#[test]
fn ses_writer_writes_a_pins_line_for_every_swapped_pin() {
    let (mut board, ct) = common::read_directed("was-is");

    let pins = board.get_pins();
    let order: Vec<String> = pins.iter().map(|id| id.0.to_string()).collect();
    common::assert_rows_match(
        &[format!("[pinorder] getPins() {}", order.join(" "))],
        &common::directed_rows("was-is", "[pinorder]"),
        "was-is getPins() order",
    );

    let (expected_before, before_bytes) = common::directed_ses("was-is", "before");
    let actual_before = common::write_directed_ses(&board, &ct, "was-is");
    assert_eq!(actual_before, expected_before, "SES before the swap");
    assert_eq!(actual_before.len(), before_bytes, "SES byte count before");

    let (first, last) = (pins[0], pins[pins.len() - 1]);
    let Some(Item::Pin(a)) = board.get_item(first) else {
        panic!("board.get_pins() must answer pins");
    };
    let Some(Item::Pin(b)) = board.get_item(last) else {
        panic!("board.get_pins() must answer pins");
    };
    let (mut a, mut b) = (a.clone(), b.clone());
    assert!(
        a.swap(&mut b, &board.rules.nets),
        "both pins are on one net each, so `Pin.swap` returns true"
    );
    *board.get_item_mut(first).expect("pin id is live") = Item::Pin(a);
    *board.get_item_mut(last).expect("pin id is live") = Item::Pin(b);

    let (expected_after, after_bytes) = common::directed_ses("was-is", "after");
    let actual_after = common::write_directed_ses(&board, &ct, "was-is");
    assert_eq!(actual_after, expected_after, "SES after the swap");
    assert_eq!(actual_after.len(), after_bytes, "SES byte count after");
    assert_eq!(
        actual_after.matches("(pins ").count(),
        2,
        "one `(pins …)` line per swapped pin — the swap body ran twice"
    );
}

#[test]
fn ses_writer_mixes_integer_boundary_and_double_hole_coordinates() {
    let (board, ct) = common::read_directed("conduction-area");
    common::assert_rows_match(
        &common::directed_items(&board),
        &common::directed_rows("conduction-area", "[item"),
        "conduction-area item graph",
    );

    let (expected, bytes) = common::directed_ses("conduction-area", "ses");
    let actual = common::write_directed_ses(&board, &ct, "conduction-area");
    assert_eq!(actual, expected, "conduction area SES");
    assert_eq!(actual.len(), bytes, "conduction area SES byte count");

    assert!(
        actual.contains("            1000000 1000000\n"),
        "the boundary is written through `writeScopeInt`: plain integers"
    );
    assert!(
        actual.contains("              1300005 1300005\n"),
        "each hole is written through `writeHoleScope` -> `writeScope`, dropping the trailing .0"
    );
}
