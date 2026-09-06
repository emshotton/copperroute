mod common;

use std::time::Duration;

use fr_board::ItemIdGenerator;
use fr_dsn::error::BoardReadResult;
use fr_dsn::parser::scope_parameter::DsnReadOptions;
use fr_dsn::{read_board, read_metadata};

use common::{assert_matches_golden, fixture};

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

#[test]
fn read_board_sets_host_cad() {
    let result = read(&fixture("Issue143-rpi_splitter.dsn"));
    let (board, _) = success(&result);
    let info = &board.communication;
    assert_eq!(info.host_cad.as_deref(), Some("CadSoft"));
    assert_eq!(
        info.host_version.as_deref(),
        Some("Eagle V 9.5 - Using eagle2freerouting.ulp, version 2022-09-01, on 9/20/2022 9:56 PM")
    );
}

#[test]
fn read_board_parse_error_for_garbage() {
    let result = read("not a dsn file");
    let BoardReadResult::ParseError { location, detail } = &result else {
        panic!("garbage input must produce ParseError, got {result:?}");
    };
    assert_eq!(location, "(pcb");
    assert_eq!(
        detail,
        "Not a Specctra DSN file: expected '(pcb <name>' header"
    );
}

#[test]
fn read_board_parse_error_for_empty_input() {
    let result = read("");
    assert!(
        matches!(result, BoardReadResult::ParseError { .. }),
        "got {result:?}"
    );
}

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
    assert!(
        matches!(
            &result,
            BoardReadResult::ParseError { location, detail }
                if location == "(pcb" && detail == "DSN structure parsing failed"
        ),
        "got {result:?}"
    );
}

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

#[test]
fn read_board_succeeds_for_empty_board() {
    let result = read(&fixture("empty_board.dsn"));
    let (board, warnings) = success(&result);
    assert_matches_golden(board, warnings, "empty_board-items.txt");
}

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

/// (:16-28). In Rust the exhaustiveness is a compile-time property of `match` on a non-`#[non_exhaustive]`
#[test]
fn the_five_variant_match_is_exhaustive() {
    let result = read("not dsn");
    let label = match result {
        BoardReadResult::Success { .. } => "success",
        BoardReadResult::Partial { .. } => "partial",
        BoardReadResult::OutlineMissing { .. } => "outline",
        BoardReadResult::ParseError { .. } => "parse",
        BoardReadResult::IoError(_) => "io",
    };
    assert_eq!(label, "parse");
}

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

#[test]
fn issue413_wiring_matches_javas_traces_and_vias() {
    let result = read(&fixture("Issue413-test.dsn"));
    let (board, warnings) = success(&result);
    assert_matches_golden(board, warnings, "Issue413-test-items.txt");
}

#[test]
fn issue110_relay_module_wiring_matches_javas_vias() {
    let result = read(&fixture("Issue110-RelayModule.dsn"));
    let (board, warnings) = success(&result);
    assert_matches_golden(board, warnings, "Issue110-RelayModule-items.txt");
}

#[test]
fn issue026_still_matches_through_the_real_reader() {
    let result = read(&fixture("Issue026-J2_reference.dsn"));
    let (board, warnings) = success(&result);
    assert_matches_golden(board, warnings, "Issue026-J2_reference-items.txt");
}

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

#[test]
fn read_metadata_extracts_layer_count() {
    let result = read_metadata(fixture("Issue143-rpi_splitter.dsn").as_bytes());
    let BoardReadResult::Success { metadata, .. } = &result else {
        panic!("expected Success, got {result:?}");
    };
    assert_eq!(metadata.as_ref().expect("metadata").layer_count, 2);
}

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

#[test]
fn read_metadata_parse_error_for_empty_input() {
    let result = read_metadata(&b""[..]);
    assert!(
        matches!(result, BoardReadResult::ParseError { .. }),
        "got {result:?}"
    );
}

/// picks. `#[ignore]`d in a debug build, where the lexer is an order of magnitude slower than
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

#[test]
fn try_correct_net_takes_the_highest_id_contact() {
    let result = read(&common::test_data("wiring_try_correct_net.dsn"));
    let (board, warnings) = success(&result);

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

#[test]
fn warnings_for_every_reachable_wiring_site() {
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

#[test]
fn a_missing_via_padstack_fails_the_read_and_loses_its_warning() {
    let result = read(&common::test_data("wiring_via_padstack_missing.dsn"));
    let BoardReadResult::ParseError { location, detail } = &result else {
        panic!("expected ParseError, got {result:?}");
    };
    assert_eq!(location, "(pcb");
    assert_eq!(detail, "DSN structure parsing failed");
}

fn corpus_dir() -> std::path::PathBuf {
    parity::reference_dir().join("fixtures")
}

const NAMED_CORPUS_FIXTURES: [&str; 5] = [
    "Issue110-Паяльная станция.dsn",
    "Issue756-minimal-hang.dsn",
    "Issue756-minimal-ok.dsn",
    "Issue757-minimal-soe.dsn",
    "Issue757-minimal-soe-ok.dsn",
];

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

/// [`every_fixture_in_the_corpus_matches_javas_result_and_warnings`] is `#[ignore]`d in debug
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

    if !parity::require_reference_dir() {
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
            BoardReadResult::Partial { .. } => "Partial",
            BoardReadResult::OutlineMissing { .. } => "OutlineMissing",
            BoardReadResult::ParseError { .. } => "ParseError",
            BoardReadResult::IoError(_) => "IoError",
        };
        assert_eq!(variant, expected, "{required} read to the wrong variant");
    }
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "~90 s in debug; run with --release or --ignored"
)]
fn every_fixture_in_the_corpus_matches_javas_result_and_warnings() {
    if !parity::require_reference_dir() {
        return;
    }
    let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(corpus_dir())
        .expect("fixture directory (existence already checked by require_reference_dir)")
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
            BoardReadResult::Partial { warnings, .. } => ("Partial", warnings),
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
