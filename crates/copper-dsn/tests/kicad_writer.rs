use copper_board::{Board, Item};
use copper_dsn::error::BoardReadResult;
use copper_dsn::kicad::{DEFAULT_DESIGN_NAME, import_session, read_board, write};

const TRANSCRIPT: &str = include_str!("data/p8t10-kicad-writer.txt");

fn fixture(relative: &str) -> String {
    let root = std::env::var("FREEROUTING_JAVA_DIR").unwrap_or_else(|_| {
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../../freerouting").to_string()
    });
    std::fs::read_to_string(format!("{root}/{relative}"))
        .unwrap_or_else(|e| panic!("fixture {relative}: {e}"))
}

fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('\\') => out.push('\\'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('n') => out.push('\n'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

fn board_of(json: &str) -> Option<Box<Board>> {
    match read_board(json, None) {
        BoardReadResult::Success { board, .. } | BoardReadResult::OutlineMissing { board, .. } => {
            board
        }
        _ => None,
    }
}

struct WriterCase {
    stem: String,
    design_name: String,
    json: String,
    expected: Vec<String>,
}

fn writer_cases() -> Vec<WriterCase> {
    let mut cases: Vec<WriterCase> = Vec::new();
    for line in TRANSCRIPT.lines() {
        if let Some(rest) = line.strip_prefix("[wcase] stem=") {
            let (stem, rest) = rest.split_once(' ').expect("stem then designName");
            let (design_name, rest) = rest
                .strip_prefix("designName=")
                .expect("designName=")
                .split_once(' ')
                .expect("designName then the source");
            let json = if let Some(path) = rest.strip_prefix("file=") {
                fixture(path.split(' ').next().expect("non-empty"))
            } else {
                unescape(rest.strip_prefix("json=").expect("file= or json="))
            };
            cases.push(WriterCase {
                stem: stem.to_string(),
                design_name: unescape(design_name),
                json,
                expected: Vec::new(),
            });
        } else if (line.starts_with("[w]") || line.starts_with("[rt] "))
            && let Some(case) = cases.last_mut()
        {
            case.expected.push(line.to_string());
        }
    }
    assert_eq!(cases.len(), 9, "the transcript measures 9 writer inputs");
    cases
}

fn emit_writer(case: &WriterCase) -> Vec<String> {
    let mut rows: Vec<String> = Vec::new();
    let Some(board) = board_of(&case.json) else {
        rows.push("[w] result=<no board>".to_string());
        return rows;
    };
    let design_name = match case.design_name.as_str() {
        "<one-arg>" | "<null>" => DEFAULT_DESIGN_NAME,
        other => other,
    };
    let written = write(&board, design_name);
    rows.push(format!("[w]bytes={}", written.len()));
    for line in written.split('\n') {
        rows.push(format!("[w]|{line}"));
    }

    match board_of(&written) {
        None => rows.push("[rt] reread=<no board>".to_string()),
        Some(reread) => {
            let again = write(&reread, "roundtrip");
            let first = write(&board, "roundtrip");
            rows.push(format!(
                "[rt] fixedPoint={} bytes={}",
                again == first,
                again.chars().count()
            ));
        }
    }
    rows
}

const PORT_TRANSCRIPT: &str = include_str!("data/p9t7-kicad-writer.txt");

#[test]
fn the_writer_output_matches_the_port_golden_byte_for_byte() {
    let golden = writer_golden(PORT_TRANSCRIPT);
    let cases = writer_cases();
    assert_eq!(cases.len(), golden.len());
    let mut compared = 0usize;
    for (case, (stem, expected)) in cases.iter().zip(&golden) {
        assert_eq!(
            &case.stem, stem,
            "the two files list the stems in one order"
        );
        let actual = emit_writer(case);
        assert_eq!(actual.len(), expected.len(), "row count for stem {stem}");
        for (row, (actual, expected)) in actual.iter().zip(expected).enumerate() {
            assert_eq!(
                actual, expected,
                "stem {stem} row {row}: the writer diverged from the port golden"
            );
            compared += 1;
        }
    }
    assert_eq!(
        compared, 5021,
        "the replay compares 5021 rows over 9 boards"
    );
}

fn writer_golden(text: &str) -> Vec<(String, Vec<String>)> {
    let mut cases: Vec<(String, Vec<String>)> = Vec::new();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("[wcase] stem=") {
            let stem = rest.split(' ').next().expect("a [wcase] line names a stem");
            cases.push((stem.to_string(), Vec::new()));
        } else if (line.starts_with("[w]") || line.starts_with("[rt] "))
            && let Some(case) = cases.last_mut()
        {
            case.1.push(line.to_string());
        }
    }
    cases
}

#[test]
#[ignore = "a generator, not a check — see PORT_TRANSCRIPT for the command"]
fn emit_the_port_writer_transcript() {
    println!("# the PORT's rows over the inputs of data/p8t10-kicad-writer.txt, which stays as");
    println!("# the jar's record. Plan 9 Task 7 moved this family to the port lane: see this");
    println!("# file's PORT_TRANSCRIPT and KNOWN_DIVERGENCES, and kicad_reader.rs for the");
    println!("# argument in full.");
    for case in writer_cases() {
        println!();
        println!("[wcase] stem={}", case.stem);
        for row in emit_writer(&case) {
            println!("{row}");
        }
    }
}

#[test]
fn write_then_read_is_a_fixed_point() {
    let json = fixture("fixtures/Issue649-kicad_ecc83-pp_input_board_v1.json");
    let board = board_of(&json).expect("the fixture reads");
    assert_eq!(board.components.count(), 15, "the document has components");

    let once = write(&board, "roundtrip");
    assert!(
        once.contains("\"components\": []"),
        "the writer emits no components"
    );
    let reread = board_of(&once).expect("the written document reads back");
    assert_eq!(reread.components.count(), 0, "they are gone after one pass");

    let twice = write(&reread, "roundtrip");
    assert_eq!(once, twice, "the second write is a fixed point");

    let two_traces = "{\"unit\":\"MM\",\"resolution\":1000.0,\
        \"layers\":[{\"index\":0,\"name\":\"F.Cu\",\"type\":\"signal\"}],\
        \"nets\":[{\"id\":1,\"name\":\"A\",\"className\":\"default\"},\
        {\"id\":2,\"name\":\"B\",\"className\":\"default\"}],\
        \"outline\":{\"corners\":[{\"x\":0.0,\"y\":0.0},{\"x\":50.0,\"y\":0.0},\
        {\"x\":50.0,\"y\":40.0},{\"x\":0.0,\"y\":40.0}]},\
        \"traces\":[{\"id\":1,\"netName\":\"A\",\"width\":0.25,\"layerIndex\":0,\
        \"points\":[{\"x\":1.0,\"y\":1.0},{\"x\":2.0,\"y\":1.0}]},\
        {\"id\":2,\"netName\":\"B\",\"width\":0.25,\"layerIndex\":0,\
        \"points\":[{\"x\":3.0,\"y\":3.0},{\"x\":4.0,\"y\":3.0}]}]}";
    let board = board_of(two_traces).expect("reads");
    let first = write(&board, "roundtrip");
    let second = write(&board_of(&first).expect("reads"), "roundtrip");
    let third = write(&board_of(&second).expect("reads"), "roundtrip");
    assert_ne!(first, second, "the trace list comes back reversed");
    assert_eq!(first, third, "and reversing it twice is the identity");
    let first_net = first
        .split("\"netName\": ")
        .nth(1)
        .expect("the first trace's netName");
    let second_net = second
        .split("\"netName\": ")
        .nth(1)
        .expect("the first trace's netName");
    assert_eq!(&first_net[..3], "\"B\"");
    assert_eq!(&second_net[..3], "\"A\"");
}

#[test]
fn import_session_inserts_the_wires_and_vias() {
    let base = "{\"unit\":\"MM\",\"resolution\":1000.0,\
        \"layers\":[{\"index\":0,\"name\":\"F.Cu\",\"type\":\"signal\"},\
        {\"index\":1,\"name\":\"B.Cu\",\"type\":\"signal\"}],\
        \"nets\":[{\"id\":1,\"name\":\"GND\",\"className\":\"default\"},\
        {\"id\":2,\"name\":\"VCC\",\"className\":\"default\"}],\
        \"outline\":{\"corners\":[{\"x\":0.0,\"y\":0.0},{\"x\":50.0,\"y\":0.0},\
        {\"x\":50.0,\"y\":40.0},{\"x\":0.0,\"y\":40.0}]},\
        \"traces\":[{\"id\":1,\"netName\":\"GND\",\"width\":0.25,\"layerIndex\":0,\
        \"points\":[{\"x\":1.0,\"y\":1.0},{\"x\":9.0,\"y\":1.0}]}]}";
    let session = "{\"unit\":\"MM\",\"resolution\":1000.0,\
        \"vias\":[{\"id\":1,\"netName\":\"GND\",\"position\":{\"x\":5.0,\"y\":1.0},\
        \"diameter\":0.8,\"drill\":0.4,\"startLayerIndex\":0,\"endLayerIndex\":1}]}";

    let mut board = board_of(base).expect("the base board reads");
    assert_eq!(board.get_traces().len(), 1);
    assert_eq!(board.get_vias().len(), 0);

    import_session(session, &mut board).expect("the session imports");

    assert_eq!(
        board.get_vias().len(),
        1,
        "the session's via is on the board"
    );
    let via = board.get_vias()[0];
    let ctx = board.ctx();
    let Some(Item::Via(via)) = board.get_item(via) else {
        panic!("a via")
    };
    assert_eq!(
        via.get_padstack(&ctx).expect("registered").name,
        "Via[0-1]_800:400_um"
    );
    assert!(matches!(
        via.get_shape_on_layer(0, &ctx),
        Some(copper_geometry::Shape::Circle(_))
    ));
    assert_eq!(via.get_padstack(&ctx).unwrap().drill_diameter, Some(400.0));
    let center = via.get_center().to_float();
    assert_eq!((center.x, center.y), (5000.0, -1000.0));
    assert_eq!(board.get_traces().len(), 1, "the trace is left whole");
}

#[test]
fn a_non_ascii_component_name_survives_utf8() {
    let base = "{\"unit\":\"MM\",\"resolution\":1000.0,\
        \"layers\":[{\"index\":0,\"name\":\"F.Cu\",\"type\":\"signal\"},\
        {\"index\":1,\"name\":\"B.Cu\",\"type\":\"signal\"}],\
        \"nets\":[{\"id\":1,\"name\":\"GND_é中\",\"className\":\"default\"}],\
        \"outline\":{\"corners\":[{\"x\":0.0,\"y\":0.0},{\"x\":50.0,\"y\":0.0},\
        {\"x\":50.0,\"y\":40.0},{\"x\":0.0,\"y\":40.0}]}}";
    let session = "{\"unit\":\"MM\",\"resolution\":1000.0,\
        \"traces\":[{\"id\":1,\"netName\":\"GND_é中\",\"width\":0.25,\"layerIndex\":0,\
        \"points\":[{\"x\":1.0,\"y\":1.0},{\"x\":2.0,\"y\":1.0}]}]}";

    let mut board = board_of(base).expect("the base board reads");
    assert_eq!(
        board.rules.nets.get(1).expect("net 1").name,
        "GND_é中",
        "the reader kept every code point"
    );
    import_session(session, &mut board).expect("the session imports");
    let trace = board.get_traces()[0];
    let Some(item) = board.get_item(trace) else {
        panic!("a trace")
    };
    assert_eq!(
        item.get_net_number(0),
        1,
        "the non-ASCII name matched net 1, so the trace is not netless"
    );
}

#[test]
fn the_three_lists_are_guarded_here_and_not_in_read_board() {
    let base = "{\"unit\":\"MM\",\"resolution\":1000.0,\
        \"layers\":[{\"index\":0,\"name\":\"F.Cu\",\"type\":\"signal\"}],\
        \"outline\":{\"corners\":[{\"x\":0.0,\"y\":0.0},{\"x\":50.0,\"y\":0.0},\
        {\"x\":50.0,\"y\":40.0}]}}";
    let mut board = board_of(base).expect("the base board reads");
    let before = board.items_in_board_order().len();

    import_session(
        "{\"traces\":null,\"vias\":null,\"conductionAreas\":null}",
        &mut board,
    )
    .expect("importSession guards all three");
    assert_eq!(board.items_in_board_order().len(), before);

    let with_null_traces = base.replace("\"outline\"", "\"traces\":null,\"outline\"");
    assert!(
        matches!(
            read_board(&with_null_traces, None),
            BoardReadResult::ParseError { .. }
        ),
        "readBoard does not guard them"
    );
}

#[test]
fn an_absent_resolution_in_mm_means_ten_thousand() {
    let base = "{\"unit\":\"MM\",\"resolution\":1000.0,\
        \"layers\":[{\"index\":0,\"name\":\"F.Cu\",\"type\":\"signal\"}],\
        \"nets\":[{\"id\":1,\"name\":\"GND\",\"className\":\"default\"}],\
        \"outline\":{\"corners\":[{\"x\":0.0,\"y\":0.0},{\"x\":50.0,\"y\":0.0},\
        {\"x\":50.0,\"y\":40.0},{\"x\":0.0,\"y\":40.0}]}}";
    let trace = "\"traces\":[{\"id\":1,\"netName\":\"GND\",\"width\":0.25,\"layerIndex\":0,\
        \"points\":[{\"x\":1.0,\"y\":1.0},{\"x\":2.0,\"y\":1.0}]}]";

    for (session, expected_x, expected_half_width) in [
        (format!("{{\"unit\":\"MM\",{trace}}}"), 10_000.0, 1250),
        (format!("{{\"unit\":\"MIL\",{trace}}}"), 1.0, 0),
        (
            format!("{{\"unit\":\"MM\",\"resolution\":0.25,{trace}}}"),
            1.0,
            0,
        ),
        (
            format!("{{\"unit\":\"UM\",\"resolution\":2.7,{trace}}}"),
            3.0,
            0,
        ),
    ] {
        let mut board = board_of(base).expect("the base board reads");
        import_session(&session, &mut board).expect("imports");
        let id = board.get_traces()[0];
        let Some(Item::Trace(trace)) = board.get_item(id) else {
            panic!("a trace")
        };
        assert_eq!(
            trace.polyline().corners()[0].to_float().x,
            expected_x,
            "for {session}"
        );
        assert_eq!(trace.get_half_width(), expected_half_width, "for {session}");
    }
}

#[test]
fn import_session_reuses_a_padstack_that_lacks_drill_metadata() {
    let base = "{\"unit\":\"MM\",\"resolution\":1000.0,\
        \"layers\":[{\"index\":0,\"name\":\"F.Cu\",\"type\":\"signal\"},\
        {\"index\":1,\"name\":\"B.Cu\",\"type\":\"signal\"}],\
        \"nets\":[{\"id\":1,\"name\":\"GND\",\"className\":\"default\"}],\
        \"outline\":{\"corners\":[{\"x\":0.0,\"y\":0.0},{\"x\":50.0,\"y\":0.0},\
        {\"x\":50.0,\"y\":40.0},{\"x\":0.0,\"y\":40.0}]}}";
    let session = "{\"unit\":\"MM\",\"resolution\":1000.0,\
        \"vias\":[{\"id\":1,\"netName\":\"GND\",\"position\":{\"x\":5.0,\"y\":1.0},\
        \"diameter\":0.8,\"drill\":0.4,\"startLayerIndex\":0,\"endLayerIndex\":1},\
        {\"id\":2,\"netName\":\"GND\",\"position\":{\"x\":9.0,\"y\":1.0},\
        \"diameter\":0.8,\"drill\":0.4,\"startLayerIndex\":0,\"endLayerIndex\":1}]}";

    let mut board = board_of(base).expect("the base board reads");
    let copper = copper_geometry::Shape::Circle(copper_geometry::Circle::new(
        copper_geometry::IntPoint::ZERO,
        400,
    ));
    let legacy = board.library.padstacks.add(
        "Via[0-1]_800:400_um",
        vec![Some(copper); board.get_layer_count()],
        true,
        false,
    );
    let before = board.library.padstacks.count();

    import_session(session, &mut board).expect("the session imports");

    assert_eq!(
        board.library.padstacks.count(),
        before,
        "a same-named padstack without drill metadata is reused, not duplicated"
    );
    let ctx = board.ctx();
    for id in board.get_vias() {
        let Some(Item::Via(via)) = board.get_item(id) else {
            panic!("a via")
        };
        let padstack = via.get_padstack(&ctx).expect("registered");
        assert_eq!(copper_board::PadstackId(padstack.no), legacy);
        assert_eq!(padstack.drill_diameter, Some(400.0));
    }
}
