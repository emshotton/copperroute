//! Plan 3 Task 10: `Wiring.readScope`, `DsnReader.readBoard` and `DsnReader.readMetadata`.
//!
//! Java authority: `io/specctra/parser/Wiring.java`, `io/specctra/DsnReader.java`. The tests
//! below are ports of `io/specctra/DsnReaderTest.java`, `io/specctra/DsnReadResultTest.java`,
//! `io/specctra/DsnReaderMetadataTest.java` and
//! `RulesRoundTripTest.loadingProducesWarningsForDegenerateWires`, plus the item-level goldens
//! the `wiring` scope needs (captured from the pinned 2.3.0 jar — see `tests/common/mod.rs`).
//!
//! # Two Java-wins corrections to the task brief
//!
//! 1. **`readMetadata` answers a whole `BoardReadResult`**, not a `BoardMetadata`
//!    (DsnReader.java:182, `:278-279`), and every `DsnReaderMetadataTest` case asserts on the
//!    `Success` wrapper. The port matches Java.
//! 2. **`Issue026-J2_reference.dsn`'s `(wiring)` scope is empty** — so are
//!    `Issue034-Green14SegLED.dsn`'s and `empty_board.dsn`'s — so it cannot be the wiring
//!    golden the brief asks for. Its Task 9 golden is still exercised (through the real
//!    `read_board` now), and the wiring goldens are `Issue413-test.dsn` (11 traces, 4 vias, two
//!    layers, `shove_fixed` types) and `Issue110-RelayModule.dsn` (22 vias).
//!
//! `DsnReaderTest`'s fixtures are Java's, not the brief's: `readBoardReturnsSuccess` and
//! `readBoardSetsHostCad` use `Issue143-rpi_splitter.dsn`, the metadata tests use
//! `Issue143-rpi_splitter.dsn` / `Issue508-DAC2020_bm01.dsn` / `Issue187-processor.Z80.dsn`.

mod common;

use std::time::Duration;

use fr_board::ItemIdGenerator;
use fr_dsn::error::BoardReadResult;
use fr_dsn::parser::scope_parameter::DsnReadOptions;
use fr_dsn::{read_board, read_metadata};

use common::{assert_matches_golden, fixture};

/// `DsnReader.readBoard(in, null, null)` — the three-argument overload (DsnReader.java:40-43).
fn read(text: &str) -> BoardReadResult {
    read_board(text.as_bytes(), None, None, &DsnReadOptions::default())
}

fn success(result: &BoardReadResult) -> (&fr_board::Board, &[String]) {
    match result {
        BoardReadResult::Success {
            board, warnings, ..
        } => (
            board.as_deref().expect("board must not be null on success"),
            warnings,
        ),
        other => panic!("expected Success, got {other:?}"),
    }
}

// ------------------------------------------------------------------- DsnReaderTest.java

/// `DsnReaderTest.readBoardReturnsSuccess` (:28-38).
#[test]
fn read_board_returns_success() {
    let result = read(&fixture("Issue143-rpi_splitter.dsn"));
    let (board, _) = success(&result);
    assert_eq!(
        board.get_layer_count(),
        2,
        "Issue143-rpi_splitter.dsn is a 2-layer board"
    );
}

/// `DsnReaderTest.readBoardSetsHostCad` (:40-49): Java asserts only that
/// `board.communication.specctraParserInfo` is populated. The port pins the actual values too;
/// both come from a `RProbe`-style `DsnReader.readMetadata` run against the pinned 2.3.0 jar.
#[test]
fn read_board_sets_host_cad() {
    let result = read(&fixture("Issue143-rpi_splitter.dsn"));
    let (board, _) = success(&result);
    // Java's `SpecctraParserInfo` nested record is flattened into `Communication` by Plan 2
    // (Task 5 fix round 1), so "specctraParserInfo must be populated" becomes "the fields it
    // held are populated".
    let info = &board.communication;
    assert_eq!(info.host_cad.as_deref(), Some("CadSoft"));
    assert_eq!(
        info.host_version.as_deref(),
        Some("Eagle V 9.5 - Using eagle2freerouting.ulp, version 2022-09-01, on 9/20/2022 9:56 PM")
    );
}

/// `DsnReaderTest.readBoardReturnsParseErrorForGarbage` (:53-60).
#[test]
fn read_board_parse_error_for_garbage() {
    let result = read("not a dsn file");
    let BoardReadResult::ParseError { location, detail } = &result else {
        panic!("garbage input must produce ParseError, got {result:?}");
    };
    // DsnReader.java:106-107. There is no line/column tracking anywhere in Java.
    assert_eq!(location, "(pcb");
    assert_eq!(
        detail,
        "Not a Specctra DSN file: expected '(pcb <name>' header"
    );
}

/// `DsnReaderTest.readBoardReturnsParseErrorForNullStream` (:62-66), as close as Rust gets: an
/// `impl Read` cannot be null, so the nearest reachable input is an empty one, which fails the
/// same header check.
#[test]
fn read_board_parse_error_for_empty_input() {
    let result = read("");
    assert!(
        matches!(result, BoardReadResult::ParseError { .. }),
        "got {result:?}"
    );
}

/// `DsnReaderTest.readBoardReturnsOutlineMissingWhenBoundaryAbsent` (:70-87), with Java's own
/// synthetic DSN string.
#[test]
fn read_board_outline_missing_when_boundary_absent() {
    const DSN_NO_BOUNDARY: &str = concat!(
        "(pcb test\n",
        "  (parser (stringQuote \"))\n",
        "  (resolution um 10)\n",
        "  (unit um)\n",
        "  (structure\n",
        "    (layer F.Cu (type signal) (property (index 0)))\n",
        "    (layer B.Cu (type signal) (property (index 1)))\n",
        "  )\n",
        ")\n",
    );
    let result = read(DSN_NO_BOUNDARY);
    assert!(
        !matches!(result, BoardReadResult::Success { .. }),
        "a DSN file with no (boundary ...) scope must not produce Success, got {result:?}"
    );
    // Java's own assertion stops there, and deliberately so: on **this** input the 2.3.0 jar
    // answers `ParseError`, not `OutlineMissing` — `(parser (stringQuote "))` opens a quoted
    // string at the `"` that never closes (`stringQuote` is not a Specctra keyword, plan ruling
    // 1, so `skipScope` swallows the rest of the file), the `structure` scope is therefore never
    // read, and `boardOutlineOk` is still true when `readScope` answers false.
    // JVM-verified against tools/freerouting-2.3.0.jar:
    //     readBoard -> ParseError[location=(pcb, detail=DSN structure parsing failed]
    assert!(
        matches!(
            &result,
            BoardReadResult::ParseError { location, detail }
                if location == "(pcb" && detail == "DSN structure parsing failed"
        ),
        "got {result:?}"
    );
}

/// The input that really does reach `BoardReadResult::OutlineMissing`: a `structure` scope whose
/// boundary has zero extent, so `Structure.createBoard` sets `boardOutlineOk = false` and builds
/// no board (Structure.java:1195-1198).
///
/// JVM-verified against tools/freerouting-2.3.0.jar:
///     readBoard -> OutlineMissing[board=null, metadata=null, warnings=[]]
#[test]
fn read_board_outline_missing_for_a_zero_extent_boundary() {
    const DSN_ZERO_BOUNDARY: &str = concat!(
        "(pcb test\n",
        "  (resolution um 10)\n",
        "  (structure\n",
        "    (layer F.Cu (type signal) (property (index 0)))\n",
        "    (layer B.Cu (type signal) (property (index 1)))\n",
        "    (boundary (rect pcb 0 0 0 0))\n",
        "  )\n",
        ")\n",
    );
    let result = read(DSN_ZERO_BOUNDARY);
    let BoardReadResult::OutlineMissing {
        board,
        metadata,
        warnings,
        ..
    } = &result
    else {
        panic!("expected OutlineMissing, got {result:?}");
    };
    assert!(board.is_none());
    assert!(metadata.is_none());
    assert!(warnings.is_empty());
}

/// `DsnReaderTest.readBoardSucceedsForEmptyBoard` (:91-99), plus the Task 9 golden, now read
/// through the real `read_board` rather than a hand-driven scope loop.
#[test]
fn read_board_succeeds_for_empty_board() {
    let result = read(&fixture("empty_board.dsn"));
    let (board, warnings) = success(&result);
    assert_matches_golden(board, warnings, "empty_board-items.txt");
}

/// `DsnReaderTest.readBoardMergesKicadDefaultIntoFreeroutingDefault` (:101-113).
#[test]
fn read_board_merges_kicad_default_into_freerouting_default() {
    let result = read(&fixture("Issue508-DAC2020_bm08.dsn"));
    let (board, _) = success(&result);
    assert!(
        board
            .rules
            .net_classes
            .get_by_name("kicad_default")
            .is_none()
    );
    assert!(board.rules.net_classes.get_by_name("default").is_some());
    let net = board
        .rules
        .nets
        .get_by_name_and_subnet("/SCL", 1)
        .expect("net /SCL subnet 1");
    assert_eq!(
        board.rules.net_classes.get(net.get_net_class()).get_name(),
        "default"
    );
}

/// `DsnReaderTest.readBoardLoadsIssue034WithMultipleBoundaryPaths` (:115-127), plus the Task 9
/// golden through the real reader.
#[test]
fn read_board_loads_issue034_with_multiple_boundary_paths() {
    let result = read_board(
        fixture("Issue034-Green14SegLED.dsn").as_bytes(),
        None,
        Some("Issue034-Green14SegLED.dsn"),
        &DsnReadOptions::default(),
    );
    let (board, warnings) = success(&result);
    assert!(board.get_outline().is_some());
    assert!(
        board.components.count() > 0,
        "board must contain placed components"
    );
    assert_matches_golden(board, warnings, "Issue034-Green14SegLED-items.txt");
}

/// `DsnReaderTest.readBoardF60Keyboard` (:145-158) — conditional on the benchmark corpus being
/// present, exactly as Java's is.
#[test]
fn read_board_f60_keyboard() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../freerouting/scripts/benchmark/fixtures/PCBench/f.60_keyboard/unrouted.dsn"
    );
    let Ok(text) = std::fs::read_to_string(path) else {
        return;
    };
    let result = read_board(
        text.as_bytes(),
        None,
        Some("unrouted.dsn"),
        &DsnReadOptions::default(),
    );
    assert!(
        matches!(result, BoardReadResult::Success { .. }),
        "f.60_keyboard DSN must parse successfully, got {result:?}"
    );
}

/// `DsnReaderTest.patternSwitchIsExhaustive` (:131-143) / `DsnReadResultTest.patternMatchExhaustive`
/// (:16-28). In Rust the exhaustiveness is a compile-time property of `match` on a non-`#[non_exhaustive]`
/// enum — this test exists so the four arms are written down somewhere.
#[test]
fn the_four_variant_match_is_exhaustive() {
    let result = read("not dsn");
    let label = match result {
        BoardReadResult::Success { .. } => "success",
        BoardReadResult::OutlineMissing { .. } => "outline",
        BoardReadResult::ParseError { .. } => "parse",
        BoardReadResult::IoError(_) => "io",
    };
    assert_eq!(label, "parse");
}

/// `DsnReadResultTest.parseErrorAccessors` (:31-36) and `ioErrorWrapsException` (:38-43).
#[test]
fn parse_error_and_io_error_carry_their_payloads() {
    let error = BoardReadResult::ParseError {
        location: "(structure".to_string(),
        detail: "missing layer".to_string(),
    };
    let BoardReadResult::ParseError { location, detail } = &error else {
        unreachable!()
    };
    assert_eq!(location, "(structure");
    assert_eq!(detail, "missing layer");

    let cause = std::io::Error::other("disk full");
    let BoardReadResult::IoError(wrapped) = BoardReadResult::IoError(cause) else {
        unreachable!()
    };
    assert_eq!(wrapped.to_string(), "disk full");
}

/// `DsnReadResultTest.successAndOutlineMissingHoldNullBoard` (:45-56): "Board is allowed to be
/// null at the data-model level (parser wires it later)". This is the case that forced
/// `Success.board` to be `Option` — Task 1 had it non-optional.
#[test]
fn success_and_outline_missing_hold_a_null_board() {
    let success = BoardReadResult::Success {
        board: None,
        metadata: None,
        warnings: Vec::new(),
        coordinate_transform: None,
    };
    let BoardReadResult::Success {
        board,
        metadata,
        warnings,
        coordinate_transform,
    } = &success
    else {
        unreachable!()
    };
    assert!(board.is_none());
    assert!(metadata.is_none());
    assert!(warnings.is_empty());
    // The port's own field (controller ruling A); Java's record has none.
    assert!(coordinate_transform.is_none());

    let missing = BoardReadResult::OutlineMissing {
        board: None,
        metadata: None,
        warnings: Vec::new(),
        coordinate_transform: None,
    };
    let BoardReadResult::OutlineMissing {
        board,
        metadata,
        warnings,
        ..
    } = &missing
    else {
        unreachable!()
    };
    assert!(board.is_none());
    assert!(metadata.is_none());
    assert!(warnings.is_empty());
}

/// `DsnReadResultTest.warningsAreExposed` (:58-66), with Java's own two strings.
#[test]
fn warnings_are_exposed() {
    let warnings = vec![
        "Wiring: degenerate wire skipped".to_string(),
        "Wiring: duplicate via skipped at (100, 200)".to_string(),
    ];
    let success = BoardReadResult::Success {
        board: None,
        metadata: None,
        warnings,
        coordinate_transform: None,
    };
    let BoardReadResult::Success { warnings, .. } = &success else {
        unreachable!()
    };
    assert_eq!(warnings.len(), 2);
    assert!(warnings[0].contains("degenerate wire"));
}

// not ported: DsnReadResultTest.recordEquality (:68-74) and instanceOfChecks (:76-87).
// `recordEquality` asserts that two `ParseError`s with the same fields are `equals` and share a
// `hashCode` — a property of Java `record`s that Rust does not synthesise. `BoardReadResult`
// deliberately does **not** derive `PartialEq`: it holds a `Box<Board>` (no `PartialEq`, and a
// structural comparison of two boards is not something any caller should be doing) and a
// `std::io::Error` (no `PartialEq` at all), so deriving it is not "trivial" — it would mean
// hand-writing an impl whose only honest answer for those two variants is "compare the payload
// you can". Nothing in the port compares two results. `instanceOfChecks` is `match`, covered by
// `the_four_variant_match_is_exhaustive`.

/// An unreadable stream is `BoardReadResult::IoError` (DsnReader.java:87-90). Java only reaches
/// this from one of the first three token reads, because its reader is lazy; the port reads the
/// stream up front, so it reaches it from the same call for any input.
#[test]
fn read_board_io_error_for_an_unreadable_stream() {
    struct Broken;
    impl std::io::Read for Broken {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("broken pipe"))
        }
    }
    let result = read_board(Broken, None, None, &DsnReadOptions::default());
    assert!(
        matches!(result, BoardReadResult::IoError(_)),
        "got {result:?}"
    );
}

// -------------------------------------------------------------- the `wiring` scope itself

/// The task's item-level parity test: `Issue413-test.dsn` is the smallest fixture in the corpus
/// with a real `(wiring …)` scope — 11 traces on two layers (three of them `shove_fixed`) and
/// four vias — so the golden pins every wire's layer, half width, corner count, end points, net
/// and fixed state, and every via's padstack, centre, layer span, `attach_allowed` and fixed
/// state, against the pinned 2.3.0 jar.
#[test]
fn issue413_wiring_matches_javas_traces_and_vias() {
    let result = read(&fixture("Issue413-test.dsn"));
    let (board, warnings) = success(&result);
    assert_matches_golden(board, warnings, "Issue413-test-items.txt");
}

/// The via-heavy control: `Issue110-RelayModule.dsn`'s `wiring` scope is 22 `(via …)` entries
/// and no wires, so it exercises `Wiring.readViaScope`'s padstack lookup, net resolution,
/// clearance-class default and `viaExists` duplicate check on their own.
#[test]
fn issue110_relay_module_wiring_matches_javas_vias() {
    let result = read(&fixture("Issue110-RelayModule.dsn"));
    let (board, warnings) = success(&result);
    assert_matches_golden(board, warnings, "Issue110-RelayModule-items.txt");
}

/// The Task 9 golden, now produced by the real `read_board` (empty `(wiring)` scope, so the only
/// thing the new code contributes is the closing `normalizeAllTraces`, which must be a no-op).
#[test]
fn issue026_still_matches_through_the_real_reader() {
    let result = read(&fixture("Issue026-J2_reference.dsn"));
    let (board, warnings) = success(&result);
    assert_matches_golden(board, warnings, "Issue026-J2_reference-items.txt");
}

/// Port of `RulesRoundTripTest.loadingProducesWarningsForDegenerateWires` (:114-135), and the
/// reason the eight `scopeParameter.warnings.add` strings are transcribed verbatim.
///
/// The four strings are what the 2.3.0 jar prints for this fixture, em dash included.
#[test]
fn loading_produces_warnings_for_degenerate_wires() {
    let result = read(&fixture("Issue029-hw48na.dsn"));
    let (_, warnings) = success(&result);
    assert_eq!(
        warnings,
        [
            "Wiring: degenerate wire trace skipped (all 1 corners are identical — zero-length \
             trace) on layer 'F.Cu'. This is likely a DSN export issue in your EDA tool.",
            "Wiring: degenerate wire trace skipped (all 1 corners are identical — zero-length \
             trace) on layer 'F.Cu'. This is likely a DSN export issue in your EDA tool.",
            "Wiring: degenerate wire trace skipped (all 1 corners are identical — zero-length \
             trace) on layer 'B.Cu'. This is likely a DSN export issue in your EDA tool.",
            "Wiring: degenerate wire trace skipped (all 1 corners are identical — zero-length \
             trace) on layer 'F.Cu'. This is likely a DSN export issue in your EDA tool.",
        ]
    );
}

/// Plan ruling 4: a normalisation that runs out of time lands in the branch Java already has
/// for a `normalizeAllTraces` that threw — `warnings.add("Wiring: normalization of traces
/// failed")` (Wiring.java:346-352) — and the read still answers `Success`.
///
/// `Duration::ZERO` is the documented "give up before the first check" value
/// (`DsnReadOptions::normalize_time_limit`), so the trip is deterministic: it does not depend
/// on a millisecond having elapsed inside the walk, which is what `TimeLimit.isExceeded`'s
/// strictly-greater comparison (TimeLimit.java:20) would otherwise require.
#[test]
fn a_normalisation_time_limit_trip_produces_javas_own_warning() {
    let options = DsnReadOptions {
        normalize_time_limit: Duration::ZERO,
    };
    let result = read_board(
        fixture("Issue029-hw48na.dsn").as_bytes(),
        None,
        None,
        &options,
    );
    let (_, warnings) = success(&result);
    assert_eq!(
        warnings.last().map(String::as_str),
        Some("Wiring: normalization of traces failed"),
        "got {warnings:?}"
    );
}

/// A caller-supplied `ItemIdGenerator` (Java's nullable `IdGenerator`, DsnReader.java:73-75)
/// reaches the board's `Communication` (Structure.java:1252). One that has already issued ids
/// keeps counting from where it left off, so the board's items start above 1.
#[test]
fn a_supplied_id_generator_reaches_the_boards_communication() {
    let mut id_generator = ItemIdGenerator::new();
    for _ in 0..100 {
        id_generator.new_id();
    }
    let result = read_board(
        fixture("Issue413-test.dsn").as_bytes(),
        Some(id_generator),
        None,
        &DsnReadOptions::default(),
    );
    let (board, _) = success(&result);
    let first = *board.items.keys().next().expect("at least one item");
    assert_eq!(first.0, 101, "the outline takes the generator's 101st id");
}

// -------------------------------------------------------- DsnReaderMetadataTest.java

/// `DsnReaderMetadataTest.readMetadataExtractsLayerCount` (:24-31).
#[test]
fn read_metadata_extracts_layer_count() {
    let result = read_metadata(fixture("Issue143-rpi_splitter.dsn").as_bytes());
    let BoardReadResult::Success { metadata, .. } = &result else {
        panic!("expected Success, got {result:?}");
    };
    assert_eq!(metadata.as_ref().expect("metadata").layer_count, 2);
}

/// `DsnReaderMetadataTest.readMetadataExtractsHostCad` (:33-41) and `readMetadataPopulatesUnit`
/// (:52-60). Java asserts only non-null; the values are the JVM's for these fixtures.
#[test]
fn read_metadata_extracts_host_cad_and_unit() {
    let result = read_metadata(fixture("Issue508-DAC2020_bm01.dsn").as_bytes());
    let BoardReadResult::Success { metadata, .. } = &result else {
        panic!("expected Success, got {result:?}");
    };
    let metadata = metadata.as_ref().expect("metadata");
    assert_eq!(metadata.host_cad.as_deref(), Some("KiCad's Pcbnew"));

    let result = read_metadata(fixture("Issue143-rpi_splitter.dsn").as_bytes());
    let BoardReadResult::Success { metadata, .. } = &result else {
        panic!("expected Success, got {result:?}");
    };
    let metadata = metadata.as_ref().expect("metadata");
    assert_eq!(metadata.unit, fr_board::Unit::Mil);
    assert_eq!(metadata.resolution, 2540);
    assert_eq!(metadata.host_cad.as_deref(), Some("CadSoft"));
    assert_eq!(metadata.layer_count, 2);
    assert_eq!(
        metadata.snap_angle,
        fr_board::AngleRestriction::FortyFiveDegree
    );
}

/// `DsnReaderMetadataTest.readMetadataReturnsParseErrorForNullStream` (:44-48) — see
/// `read_board_parse_error_for_empty_input` for why an empty stream stands in for `null`.
#[test]
fn read_metadata_parse_error_for_empty_input() {
    let result = read_metadata(&b""[..]);
    assert!(
        matches!(result, BoardReadResult::ParseError { .. }),
        "got {result:?}"
    );
}

/// `DsnReaderMetadataTest.readMetadataCompletesWithinReasonableTimeOnLargeDsn` (:43-48): the
/// assertion that the `break outer` after the `structure` scope is real, on the fixture Java
/// picks. `#[ignore]`d in a debug build, where the lexer is an order of magnitude slower than
/// the JVM's and the 5 s budget means nothing.
#[test]
#[cfg_attr(debug_assertions, ignore = "timing assertion is release-only")]
fn read_metadata_completes_within_reasonable_time_on_large_dsn() {
    let text = fixture("Issue187-processor.Z80.dsn");
    let start = std::time::Instant::now();
    let result = read_metadata(text.as_bytes());
    let elapsed = start.elapsed();
    assert!(matches!(result, BoardReadResult::Success { .. }));
    assert!(
        elapsed < Duration::from_secs(5),
        "read_metadata took {elapsed:?}"
    );
}

/// `read_metadata` must not touch `library`, `placement`, `network` or `wiring`: on a fixture
/// with a real `wiring` scope it inserts **no** traces or vias, while `read_board` on the same
/// input inserts 15.
#[test]
fn read_metadata_never_reads_the_heavy_scopes() {
    let text = fixture("Issue413-test.dsn");
    let BoardReadResult::Success { board, .. } = read_metadata(text.as_bytes()) else {
        panic!("expected Success");
    };
    let board = board.expect("Issue413-test.dsn has a boundary, so the board is built");
    assert_eq!(board.components.count(), 0, "no placement scope was read");
    assert_eq!(
        board.get_traces().len() + board.get_vias().len(),
        0,
        "no wiring scope was read"
    );

    let result = read(&text);
    let (board, _) = success(&result);
    assert_eq!(
        board.get_traces().len() + board.get_vias().len(),
        15,
        "read_board reads all 11 wires and 4 vias"
    );
}

/// Fix round 1, review item 1: `Wiring.tryCorrectNet` (Wiring.java:583-599) walks a
/// `TreeSet<Item>` — **descending** id (quirk #44) — and `break`s on the first contact with
/// exactly one net, so a netless wire takes the net of the **highest-id** item it touches. The
/// port's `BTreeSet` iterates ascending and must be `.rev()`ed.
///
/// `wiring_try_correct_net.dsn` is built so the two answers differ: a `(wire …)` with **no**
/// `(net …)` runs between two single-pin components on different nets, `PLOW` (pin id 2, net 1)
/// and `PHIGH` (pin id 3, net 2). JVM-verified against tools/freerouting-2.3.0.jar:
///
///     item 4 PolylineTrace layer=0 hw=1000 corners=2 … nets=[2,]
///
/// An ascending walk answers `nets=[1,]`, so this test fails on the wrong direction rather than
/// merely being order-insensitive.
#[test]
fn try_correct_net_takes_the_highest_id_contact() {
    let result = read(&common::test_data("wiring_try_correct_net.dsn"));
    let (board, warnings) = success(&result);

    // Spelled out as well as diffed, so a golden regeneration cannot quietly flip it.
    let trace = board
        .get_traces()
        .into_iter()
        .next()
        .expect("the one netless wire was inserted");
    let header = board.items[&trace].header();
    assert_eq!(header.net_count(), 1, "tryCorrectNet assigned a net");
    assert_eq!(
        header.get_net_number(0),
        2,
        "the highest-id contact's net (NHIGH), not the lowest-id one's (NLOW)"
    );

    assert_matches_golden(board, warnings, "wiring_try_correct_net-items.txt");
}

// -------------------------------------------------- all eight warning sites, one by one

/// Seven of `Wiring.java`'s eight `scopeParameter.warnings.add` sites, on three synthetic DSN
/// files built for them (`tests/data/wiring_*.dsn`). The corpus reaches only the eighth-listed
/// one — the degenerate wire — so without these the other six message strings would be
/// untested, and they are the ones `BoardReadResult.warnings` consumers match on.
///
/// Every expectation is the pinned 2.3.0 jar's output for the same file, captured with
/// `tests/data/CProbe.java`; the em dash in the second and fourth strings is Java's.
#[test]
fn warnings_for_every_reachable_wiring_site() {
    // Wiring.java:432, :468, :508, :544 — the four `readWireScope` sites.
    let result = read(&common::test_data("wiring_warnings.dsn"));
    let (_, warnings) = success(&result);
    assert_eq!(
        warnings,
        [
            "Wiring: wire has no shape at 'N1'",
            "Wiring: wire ignored — unknown layer 'pcb' at 'N1'",
            "Wiring: wire corner (9000000,9000000) is outside board bounds at 'N1'",
            "Wiring: degenerate wire trace skipped (all 1 corners are identical — zero-length \
             trace) on layer 'F.Cu'. This is likely a DSN export issue in your EDA tool.",
        ]
    );

    // Wiring.java:682, :703 — the two `readViaScope` sites that do not abort the read. The
    // duplicate's coordinates are the *board* ones (Wiring.java:701 prints `boardLocation`),
    // where the wire-corner message above prints the *DSN* ones.
    let result = read(&common::test_data("wiring_via_warnings.dsn"));
    let (_, warnings) = success(&result);
    assert_eq!(
        warnings,
        [
            "Wiring: via net 'NMISSING' not found at 'NMISSING'",
            "Wiring: duplicate via skipped at (600000, 600000)",
        ]
    );
}

/// Wiring.java:669, the eighth site, is the one whose warning **nobody ever sees**:
/// `readViaScope` pushes it and then answers `false` (:670), `Wiring.readScope` propagates that
/// (:340-342), and `DsnReader.readBoard` turns a `false` with `boardOutlineOk == true` into a
/// `ParseError` (:159) — a variant that carries no warnings at all. So a DSN naming a via
/// padstack the library does not have fails the whole read, and the message explaining why is
/// dropped on the floor.
///
/// JVM-verified against tools/freerouting-2.3.0.jar:
///     FIXTURE wiring_via_padstack_missing.dsn ParseError warnings=0
#[test]
fn a_missing_via_padstack_fails_the_read_and_loses_its_warning() {
    let result = read(&common::test_data("wiring_via_padstack_missing.dsn"));
    let BoardReadResult::ParseError { location, detail } = &result else {
        panic!("expected ParseError, got {result:?}");
    };
    assert_eq!(location, "(pcb");
    assert_eq!(detail, "DSN structure parsing failed");
}

// -------------------------------------------------------------------- the corpus smoke test

/// The `.dsn` corpus the golden was captured from. Honours `FREEROUTING_JAVA_DIR` through
/// `parity::java_dir`, same as `common::fixture` now does.
fn corpus_dir() -> std::path::PathBuf {
    parity::java_dir().join("fixtures")
}

/// The five fixtures the task brief names explicitly, so that a corpus reshuffle cannot quietly
/// drop them: the non-ASCII identifiers, and the four minimal reproductions of the two hangs
/// plan ruling 4 exists for. The golden pins the whole list, but only by content — these are
/// pinned by name, by both tests below.
const NAMED_CORPUS_FIXTURES: [&str; 5] = [
    "Issue110-Паяльная станция.dsn",
    "Issue756-minimal-hang.dsn",
    "Issue756-minimal-ok.dsn",
    "Issue757-minimal-soe.dsn",
    "Issue757-minimal-soe-ok.dsn",
];

/// Parses `corpus-read-results.txt` into `(fixture name, variant)` pairs.
///
/// The golden's shape is one `FIXTURE <name> <variant> warnings=<n>` line per file, each
/// optionally followed by `  W <message>` continuation lines. Anything else is a corrupt golden
/// and fails the parse.
///
/// Positions in the messages below are **entry numbers**, not file line numbers:
/// `common::golden` strips the `#` header before this sees the file.
fn corpus_golden_variants() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for (i, line) in common::golden("corpus-read-results.txt").iter().enumerate() {
        if line.starts_with("  W ") {
            assert!(
                !out.is_empty(),
                "corpus-read-results.txt entry {}: warning line before any FIXTURE line",
                i + 1
            );
            continue;
        }
        let rest = line.strip_prefix("FIXTURE ").unwrap_or_else(|| {
            panic!(
                "corpus-read-results.txt line {}: expected `FIXTURE `, got {line:?}",
                i + 1
            )
        });
        let (name, tail) = rest.rsplit_once(' ').unwrap_or_else(|| {
            panic!(
                "corpus-read-results.txt line {}: no `warnings=` field in {line:?}",
                i + 1
            )
        });
        assert!(
            tail.starts_with("warnings="),
            "corpus-read-results.txt entry {}: expected `warnings=<n>`, got {tail:?}",
            i + 1
        );
        let (name, variant) = name.rsplit_once(' ').unwrap_or_else(|| {
            panic!(
                "corpus-read-results.txt line {}: no variant in {line:?}",
                i + 1
            )
        });
        assert!(
            matches!(
                variant,
                "Success" | "OutlineMissing" | "ParseError" | "IoError"
            ),
            "corpus-read-results.txt entry {}: unknown variant {variant:?}",
            i + 1
        );
        out.push((name.to_string(), variant.to_string()));
    }
    out
}

/// The always-on half of the corpus check (controller ruling I).
///
/// [`every_fixture_in_the_corpus_matches_javas_result_and_warnings`] is `#[ignore]`d in debug
/// because it takes ~90 s, so on a default `cargo test` nothing would notice a corrupted golden
/// or a corpus that had lost one of the five fixtures the brief names. This one costs
/// milliseconds and covers exactly that: the golden parses, it names all 105 files including the
/// five, and each of those five still reads to the variant the golden records.
///
/// Skips with a printed message when the sibling Java checkout is absent, like every other
/// fixture-reading suite in this crate.
#[test]
fn the_corpus_golden_parses_and_the_named_fixtures_read_to_its_variant() {
    let golden = corpus_golden_variants();
    assert_eq!(
        golden.len(),
        105,
        "corpus-read-results.txt should name all 105 fixtures"
    );
    for required in NAMED_CORPUS_FIXTURES {
        assert!(
            golden.iter().any(|(name, _)| name == required),
            "{required} is missing from corpus-read-results.txt"
        );
    }

    if !parity::require_java_dir() {
        return;
    }
    let options = DsnReadOptions::default();
    for required in NAMED_CORPUS_FIXTURES {
        let expected = &golden
            .iter()
            .find(|(name, _)| name == required)
            .expect("checked above")
            .1;
        let path = corpus_dir().join(required);
        let bytes =
            std::fs::read(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        let result = read_board(&bytes[..], None, Some(required), &options);
        let variant = match &result {
            BoardReadResult::Success { .. } => "Success",
            BoardReadResult::OutlineMissing { .. } => "OutlineMissing",
            BoardReadResult::ParseError { .. } => "ParseError",
            BoardReadResult::IoError(_) => "IoError",
        };
        assert_eq!(variant, expected, "{required} read to the wrong variant");
    }
}

/// Every `.dsn` in the Java repo's fixture corpus reads without panicking, answers the same
/// `BoardReadResult` variant the pinned 2.3.0 jar answers, and produces **exactly** the same
/// warnings — message for message, in order.
///
/// The golden is `tests/data/corpus-read-results.txt`, captured by `tests/data/CProbe.java`
/// (the command is in the file's header). 105 files: 104 `Success`, one `ParseError` —
/// `Issue006-LPC18XX_43XX_SCH.dsn` is an OLE compound document (an Altium schematic), not a
/// DSN, and the jar rejects it with the identical detail string. 13 files produce warnings,
/// 213 in all, and every one of them is `Wiring.java:544`'s degenerate-wire message: the
/// corpus reaches exactly one of the eight `scopeParameter.warnings.add` sites, which is why
/// `warnings_for_every_wiring_site` below exists.
///
/// Plan ruling 4's safety net is the `"Wiring: normalization of traces failed"` assertion: **no**
/// corpus fixture may trip the normalisation time limit.
///
/// **Deviation from the task brief, controller ruling I.** The brief asked for a *non*-`#[ignore]`d
/// corpus test. This one runs ~90 s in a debug build — unreasonable for the default `cargo test`
/// — so it stays `#[ignore]`d there and runs unconditionally in release
/// (`cargo test -p fr-dsn --release`). **Consequence: the full 105-file corpus check only runs in
/// a release build or under `--ignored`, not in a bare debug `cargo test`.**
/// [`the_corpus_golden_parses_and_the_named_fixtures_read_to_its_variant`] is the always-on
/// sibling that keeps the golden and the five named fixtures from rotting unnoticed.
#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "~90 s in debug; run with --release or --ignored"
)]
fn every_fixture_in_the_corpus_matches_javas_result_and_warnings() {
    if !parity::require_java_dir() {
        return;
    }
    let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(corpus_dir())
        .expect("fixture directory (existence already checked by require_java_dir)")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "dsn"))
        .collect();
    paths.sort();

    for required in NAMED_CORPUS_FIXTURES {
        assert!(
            paths
                .iter()
                .any(|p| p.file_name().is_some_and(|n| n == required)),
            "{required} is missing from the corpus"
        );
    }

    // Well under the 60 s default, so a fixture that trips it is a bug in this test's budget,
    // not in the reader; none does.
    let options = DsnReadOptions {
        normalize_time_limit: Duration::from_secs(30),
    };
    let mut actual: Vec<String> = Vec::new();
    for path in &paths {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let bytes = std::fs::read(path).expect("fixture readable");
        let result = read_board(&bytes[..], None, Some(&name), &options);
        let (variant, warnings): (&str, &[String]) = match &result {
            BoardReadResult::Success { warnings, .. } => ("Success", warnings),
            BoardReadResult::OutlineMissing { warnings, .. } => ("OutlineMissing", warnings),
            BoardReadResult::ParseError { .. } => ("ParseError", &[]),
            BoardReadResult::IoError(_) => ("IoError", &[]),
        };
        assert!(
            !warnings
                .iter()
                .any(|w| w == "Wiring: normalization of traces failed"),
            "{name}: no corpus fixture may trip the normalisation time limit (plan ruling 4)"
        );
        actual.push(format!(
            "FIXTURE {name} {variant} warnings={}",
            warnings.len()
        ));
        for warning in warnings {
            actual.push(format!("  W {warning}"));
        }
    }

    let expected = common::golden("corpus-read-results.txt");
    for (i, (a, e)) in actual.iter().zip(expected.iter()).enumerate() {
        assert_eq!(a, e, "corpus-read-results.txt: line {} differs", i + 1);
    }
    assert_eq!(actual.len(), expected.len(), "corpus line count differs");
}

// ---------------------------------------------------------------------------------------------
// Plan 8 Task 13: one of the four zero-coverage Plan 3 paths (docs/plan-3-handoff.md's register
// row). The other three are `tests/parity_ses.rs`'s two and
// `tests/placement_scope.rs::the_lock_type_position_arm_survives_a_whole_file_read`;
// `tests/plan_3_zero_coverage.rs` is the list assertion over all four.
// ---------------------------------------------------------------------------------------------

/// **Zero-coverage path 4 of 4: quirk #105 — `Wiring.readViaScope`'s net-number loop that never
/// increments** (Wiring.java:684-687), against `tests/data/p8t13-via-net-numbers.dsn` and the JVM
/// transcript `tests/data/p8t13-directed-via-net-numbers.txt`.
///
/// Why no corpus fixture reaches it: the loop only pads when `getSubnets` answers **more than
/// one** net, which needs a `(via … (net NAME))` with no subnet number on a name that carries
/// several subnets — and several subnets only ever come from `Network.readNetScope`'s `(order …)`
/// or `(fromto …)` arms (Network.java:1374-1386, :1401-1406). No `.dsn` in the 105-file corpus
/// writes either, so every corpus via has exactly one found net and the missing `++currentIndex`
/// is invisible. This fixture's `(order U1-1 U2-1 U3-1)` makes `createOrderedSubnets` split
/// `NORDERED` into subnets 1 and 2, and the `(wiring …)` scope puts a via **and** a wire on the
/// bare name, so the two loops stand side by side in one transcript:
///
/// ```text
/// [item] 5 Via           … nets=[2,0]    <- readViaScope:684-687, no ++currentIndex
/// [item] 6 PolylineTrace … nets=[1,2]    <- readWireScope:441-445, with it
/// ```
///
/// **The jar does not survive its own file.** `[jar-cli] exit=<none: still running after 60s>`:
/// the padded `0` reaches `DesignRulesChecker.calculateAllIncompletes:558`, which does
/// `rules.nets.get(0)` — `Vector.get(-1)` — and throws
/// `ArrayIndexOutOfBoundsException: Index -1 out of bounds for length 2` on every autoroute pass,
/// for ever. That is a **jar**-side consequence of the quirk, not a port divergence: the port
/// reproduces the reader exactly, as the rows above show. See the task-13 report's §4.
///
/// # The control
///
/// `tests/data/p8t13-via-net-numbers-control.dsn` is the **same file**, byte for byte, except that
/// its via reads `(net NORDERED 1)` instead of `(net NORDERED)`. A subnet number `> 0` sends
/// `Wiring.getSubnets` (Wiring.java:226-230) down its single-net branch, so `foundNets.size() == 1`
/// and the loop pads nothing. Its transcript
/// (`tests/data/p8t13-directed-via-net-numbers-control.txt`) records the difference twice over:
/// the via reads `nets=[1]` rather than `nets=[2,0]`, and `[jar-cli] exit=0` — the jar routes the
/// file and writes a 1 995-byte `.ses`. That is what makes the hang attributable to the padded
/// zero and to nothing else about the fixture, and this test asserts both halves so the claim
/// cannot rot into prose.
#[test]
fn read_via_scope_pads_a_multi_subnet_vias_net_numbers_with_zeros() {
    let (board, _) = common::read_directed("via-net-numbers");
    common::assert_rows_match(
        &common::directed_nets(&board),
        &common::directed_rows("via-net-numbers", "[net]"),
        "via-net-numbers nets",
    );
    common::assert_rows_match(
        &common::directed_items(&board),
        &common::directed_rows("via-net-numbers", "[item"),
        "via-net-numbers item graph",
    );

    // The control: one changed token, no padding, and a jar run that terminates.
    let (control, _) = common::read_directed("via-net-numbers-control");
    common::assert_rows_match(
        &common::directed_nets(&control),
        &common::directed_rows("via-net-numbers-control", "[net]"),
        "via-net-numbers-control nets",
    );
    common::assert_rows_match(
        &common::directed_items(&control),
        &common::directed_rows("via-net-numbers-control", "[item"),
        "via-net-numbers-control item graph",
    );

    // Stated as an assertion rather than left to the byte comparison above: the two fixtures
    // differ in exactly one via, and only the padded one has a zero in its net array.
    let via_row = |path: &str| -> String {
        common::directed_rows(path, "[item] 5 Via")
            .first()
            .expect("both fixtures put the via at id 5")
            .clone()
    };
    assert!(
        via_row("via-net-numbers").contains("nets=[2,0]"),
        "the bare `(net NORDERED)` via is padded — quirk #105"
    );
    assert!(
        via_row("via-net-numbers-control").contains("nets=[1]"),
        "the `(net NORDERED 1)` via takes getSubnets' single-net branch and is not padded"
    );

    // And the jar's own verdict on each, which is the whole point of keeping the control.
    let hang = common::test_data("p8t13-directed-via-net-numbers.txt");
    let ok = common::test_data("p8t13-directed-via-net-numbers-control.txt");
    assert!(
        hang.contains("[jar-cli] exit=<none: still running after"),
        "the padded fixture must still hang the HEAD jar"
    );
    assert!(
        hang.contains(
            "[jar-cli] throwable java.lang.ArrayIndexOutOfBoundsException: Index -1 out of bounds"
        ),
        "and it must still hang for the `nets.get(0)` reason, not some other one"
    );
    assert!(
        ok.contains("[jar-cli] exit=0"),
        "the control must still route and exit 0"
    );
    assert!(
        !ok.contains("[jar-cli] throwable "),
        "the control must reach no throwable at all"
    );
}
