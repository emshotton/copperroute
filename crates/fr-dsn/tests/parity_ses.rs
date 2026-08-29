//! Bit-parity of the SES write path against the committed Java reference outputs
//! (`tests/reference/<stem>/unrouted.ses`, produced by `scripts/gen-reference.sh` from the pinned
//! freerouting 2.3.0 jar). Plan 3 ruling 9 puts these here rather than in `tests/parity`, which
//! must not depend on `fr-dsn`.
//!
//! Each case is exactly `scripts/gen-reference/RefWriter.java`'s first and third steps —
//! `DsnReader.readBoard` then `SesWriter.write` — with `designName` the input file's stem with
//! `.dsn` stripped, which is what puts `(session "tutorial_board"` on the reference's first line.
//!
//! Every reference has **0 `(wire` entries** (`tests/reference/README.md`): the boards are read
//! and written back without routing, and each fixture's `(wiring …)` scope is empty. A port that
//! emits a wire here has a `get_connectable_items`/`FixedState::SystemFixed` bug, not a
//! formatting one — [`no_reference_contains_a_wire`] pins that invariant on the reference side so
//! the claim cannot rot.

use std::path::Path;

mod common;

use fr_board::Board;
use fr_dsn::parser::scope_parameter::DsnReadOptions;
use fr_dsn::{BoardReadResult, CoordinateTransform, format_placement_rotation, ses_writer};

/// The four stems the plan's bit-parity tests cover, with the fixture each was generated from.
/// All four have an empty `(wiring …)` scope, which is what [`no_reference_contains_a_wire`]
/// relies on; the three ruling-G stems below deliberately do not.
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

/// Plan 3 controller ruling G: three further stems added in Task 15 so the trace, wiring-via,
/// fixed-state and `(plane …)` writers have a committed byte-exact gate. `Issue413-test` is the
/// only reference in the tree whose SES carries `(wire` entries — 11 of them — so it is the one
/// that pins `SesWriter.writeRoutesScope`'s trace path against Java.
const RULING_G_FIXTURES: [(&str, &str); 3] = [
    ("Issue413-test", "fixtures/Issue413-test.dsn"),
    ("Issue110-RelayModule", "fixtures/Issue110-RelayModule.dsn"),
    ("Issue753-CPU-85_r104", "fixtures/Issue753-CPU-85_r104.dsn"),
];

/// `RefWriter.main`'s read half: the board and the transform it was built with.
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
        BoardReadResult::ParseError { location, detail } => {
            panic!("parse error at {location}: {detail}")
        }
        BoardReadResult::IoError(e) => panic!("io error: {e}"),
    }
}

/// `Path.of(args[0]).getFileName().toString().replaceAll("\\.dsn$", "")` (RefWriter.java:26).
fn design_name(path: &Path) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .strip_suffix(".dsn")
        .unwrap_or_default()
        .to_string()
}

/// Reads a fixture and returns the SES text this port writes for it.
fn write_ses(relative_fixture: &str, design: &str) -> String {
    let fixture = parity::java_dir().join(relative_fixture);
    let (board, ct) = read_fixture(&fixture);
    let mut actual: Vec<u8> = Vec::new();
    ses_writer::write(&board, &ct, &mut actual, design).expect("write must succeed into a Vec");
    String::from_utf8(actual).expect("SES output must be valid UTF-8")
}

fn assert_ses_parity(stem: &str, relative_fixture: &str) {
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

// ---------------------------------------------------------------------------------------------
// Byte-exact parity
// ---------------------------------------------------------------------------------------------

/// The four cases above go through [`parity::assert_text_parity`], which normalises CRLF,
/// trailing spaces/tabs and runs of blank lines before comparing (plan ruling 9). This one
/// asserts the stronger property the plan actually promises — the output is byte-for-byte what
/// the pinned 2.3.0 jar wrote — so a regression that hides inside that normalisation (the
/// trailing space on `(routes `, `(library_out ` or `(network_out `) still fails.
#[test]
fn every_reference_is_byte_for_byte_identical_to_java() {
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

/// `tests/reference/README.md`'s "0 `(wire` entries" claim, asserted against the four original
/// references themselves — the premise of the brief's "a wire in the output means a
/// `get_connectable_items`/fixed-state bug, not a formatting one".
///
/// [`RULING_G_FIXTURES`] is deliberately excluded: those were added precisely because they *do*
/// have routed wiring, and [`ruling_g_references_do_carry_wires`] pins the opposite property for
/// them.
#[test]
fn no_reference_contains_a_wire() {
    for (stem, _) in FIXTURES {
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

// ---------------------------------------------------------------------------------------------
// Ports of `io/specctra/SesRoundTripTest.java` (the half that does not need `SesReader`)
// ---------------------------------------------------------------------------------------------
//
// `SesRoundTripTest`'s `@BeforeEach` (`Freerouting.globalSettings = new GlobalSettings()`) has no
// counterpart here — there is no global state to reset. `DsnTestFixtures.loadBoard` is
// `read_fixture` above.
//
// not ported: sesRoundTripPreservesWireCount, invalidSesThrowsOnRead,
// writerOutputCanBeReadBackBySesReader, nullInputStreamThrowsIoException,
// endpointSnappingIsStableAndRoundTrips, issue742SesRoundTripsWithoutErrors — all six need
// `SesReader`, which lands in Task 13. The three assertions of `issue742SesRoundTripsWithoutErrors`
// that do *not* need the reader are ported below as `issue742_placement_and_library_out_are_well_formed`.
//
// not ported: nullBoardThrowsIoException — it asserts `SesReader.read(in, null)` throws
// `IOException` rather than NPE, i.e. it is a `SesReader` test, and its sibling guard in
// `SesWriter.write` (`if (out == null) throw new IOException(...)`, SesWriter.java:57-59) has no
// Rust counterpart at all: `&Board` and `&mut impl Write` cannot be null. The brief calls this
// one `null_board_is_an_error`; there is no behaviour left to assert once the type system has
// taken both null cases away.

/// `SesRoundTripTest.sesWriterProducesValidHeader` (SesRoundTripTest.java:54-71).
// renamed: sesWriterProducesValidHeader -> valid_header (the task brief's name).
#[test]
fn valid_header() {
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

/// `SesRoundTripTest.sesWriterOutputIsNonEmpty` (SesRoundTripTest.java:171-178).
// renamed: sesWriterOutputIsNonEmpty -> output_is_non_empty (the task brief's name).
#[test]
fn output_is_non_empty() {
    let content = write_ses("fixtures/Issue143-rpi_splitter.dsn", "test.dsn");
    assert!(
        !content.is_empty(),
        "SesWriter must write data to the stream"
    );
}

/// `SesRoundTripTest.placementRotationFormattingMatchesKicadStyle` (SesRoundTripTest.java:271-278).
#[test]
fn placement_rotation_formatting_matches_kicad_style() {
    assert_eq!(format_placement_rotation(0.0), "0");
    assert_eq!(format_placement_rotation(339.0), "339");
    assert_eq!(format_placement_rotation(338.5), "338.5");
    assert_eq!(format_placement_rotation(-45.25), "-45.25");
}

/// The three assertions of `SesRoundTripTest.issue742SesRoundTripsWithoutErrors`
/// (SesRoundTripTest.java:234-269) that do not need `SesReader`: balanced scopes, unique
/// `library_out` padstacks, and KiCad-style rotation formatting in the placement records.
///
/// Java imports `Issue742-tastexx-pcb.ses` into the board before writing; the wire data that
/// import adds changes nothing these three assertions look at (they read the `placement` and
/// `library_out` scopes, and bracket balance), so this runs them on the un-imported board.
/// `tests/ses_round_trip.rs::issue742_ses_round_trips_without_errors` (Task 13) runs the same
/// three helpers on the *imported* board, i.e. on Java's own input — this one keeps them
/// covering the writer alone.
#[test]
fn issue742_placement_and_library_out_are_well_formed() {
    let fixture = parity::java_dir().join("fixtures/Issue742-tastexx-pcb.dsn");
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

/// Not a Java test: the two shared helpers, run over all four parity fixtures.
/// `assert_balanced_scopes` and `assert_unique_library_padstacks` are the brief's named test
/// ports, and `Issue742-tastexx-pcb.dsn` lives outside `tests/reference`, so this keeps them
/// covering the committed corpus too.
#[test]
fn ruling_g_references_do_carry_wires() {
    let reference_path = parity::reference("Issue413-test", "unrouted.ses");
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
    for (stem, relative_fixture) in FIXTURES.iter().chain(RULING_G_FIXTURES.iter()).copied() {
        let content = write_ses(relative_fixture, stem);
        common::assert_balanced_scopes(&content);
        common::assert_unique_library_padstacks(&content);
    }
}
