//! Java authority: `io/kicad/KiCadJsonWriter.java:27-226` and
use std::fmt::Write as _;

use fr_board::{Board, Item};
use fr_dsn::error::{BoardReadResult, DsnError};
use fr_dsn::format::java_double_to_string;
use fr_dsn::kicad::{DEFAULT_DESIGN_NAME, import_session, read_board, write};

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

const KNOWN_DIVERGENCES: &[(&str, &str, &str)] = &[(
    "ecc83-v1",
    "#280",
    "the **one** writer stem of the nine whose board carries auto-registered nets: the \
         fixture declares no `nets` at all, so all thirteen come from pad `netName`s and their \
         numbers were `String.hashCode`'s. `KiCadJsonWriter:106-120` writes `nets` in net-number \
         order, so 24 of its 145 rows move — the thirteen names, their `id`s, and `[w]bytes=` \
         with them, because the names are of different lengths. The other eight stems declare \
         their nets (or have none) and are byte-identical to the jar.",
)];

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

#[test]
fn the_writer_port_golden_differs_from_the_jar_only_where_a_fix_says_so() {
    let jar = writer_golden(TRANSCRIPT);
    let port = writer_golden(PORT_TRANSCRIPT);
    assert_eq!(jar.len(), port.len(), "the two files list the same inputs");
    let mut unexplained: Vec<String> = Vec::new();
    let mut diverged: Vec<&str> = Vec::new();
    for ((stem, jar_rows), (port_stem, port_rows)) in jar.iter().zip(&port) {
        assert_eq!(stem, port_stem, "the stems are in one order");
        if jar_rows == port_rows {
            continue;
        }
        diverged.push(stem);
        if !KNOWN_DIVERGENCES.iter().any(|(name, _, _)| name == stem) {
            let first = jar_rows
                .iter()
                .zip(port_rows)
                .position(|(a, b)| a != b)
                .unwrap_or(0);
            unexplained.push(format!(
                "{stem}[{first}]\n  jar:  {}\n  port: {}",
                jar_rows.get(first).map_or("<none>", String::as_str),
                port_rows.get(first).map_or("<none>", String::as_str),
            ));
        }
    }
    assert!(
        unexplained.is_empty(),
        "{} writer stem(s) differ from the jar with no KNOWN_DIVERGENCES entry:\n{}",
        unexplained.len(),
        unexplained.join("\n")
    );
    for (stem, row, reason) in KNOWN_DIVERGENCES {
        assert!(
            diverged.contains(stem),
            "`{stem}` now MATCHES the jar — delete its KNOWN_DIVERGENCES entry ({row}: {reason})"
        );
    }
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

struct SessionCase {
    stem: String,
    base: String,
    session: String,
    expected: Vec<String>,
}

const XDIFF: &[(&str, &str)] = &[
    ("json-truncated", "[is] throw="),
    ("via-start-gt-end", "[is] items count="),
    ("via-start-gt-end", "[is] item 2 Via"),
    ("via-start-gt-end", "[is] throw="),
    ("via-start-gt-end", "[is] padstacks count="),
    ("via-start-gt-end", "[is] padstack 2 name=Via"),
];

fn session_cases() -> Vec<SessionCase> {
    let mut cases: Vec<SessionCase> = Vec::new();
    for line in TRANSCRIPT.lines() {
        if let Some(rest) = line.strip_prefix("[icase] stem=") {
            let (stem, rest) = rest.split_once(" base=").expect("stem then base");
            let (base, session) = rest.split_once(" session=").expect("base then session");
            cases.push(SessionCase {
                stem: stem.to_string(),
                base: unescape(base),
                session: unescape(session),
                expected: Vec::new(),
            });
        } else if line.starts_with("[is] ") {
            cases
                .last_mut()
                .expect("an [is] row follows an [icase] line")
                .expected
                .push(line.to_string());
        }
    }
    assert_eq!(cases.len(), 24, "the transcript measures 24 session inputs");
    cases
}

fn emit_session(case: &SessionCase) -> Vec<String> {
    let mut rows: Vec<String> = Vec::new();
    let Some(mut board) = board_of(&case.base) else {
        rows.push("[is] base=<no board>".to_string());
        return rows;
    };
    match import_session(&case.session, &mut board) {
        Ok(()) => rows.push("[is] result=ok".to_string()),
        Err(DsnError::KicadSession(message)) => rows.push(format!("[is] throw={message}")),
        Err(other) => rows.push(format!("[is] throw=<unexpected> {other}")),
    }

    let items = board.items_in_board_order();
    rows.push(format!("[is] items count={}", items.len()));
    rows.push(format!(
        "[is] padstacks count={}",
        board.library.padstacks.count()
    ));
    for i in 1..=board.library.padstacks.count() {
        let padstack = board
            .library
            .padstacks
            .get(fr_board::PadstackId(i))
            .expect("in range");
        rows.push(format!(
            "[is] padstack {i} name={} fromLayer={} toLayer={}",
            padstack.name,
            padstack.from_layer(),
            padstack.to_layer()
        ));
    }
    let ctx = board.ctx();
    for id in items {
        let item = board.get_item(id).expect("listed above");
        let mut nets = String::new();
        for n in 0..item.net_count() {
            if n > 0 {
                nets.push(',');
            }
            let _ = write!(nets, "{}", item.get_net_number(n));
        }
        let head = format!(
            "[is] item {} {} nets=[{nets}] cl={} fixed={}",
            id.0,
            java_class_name(item),
            item.clearance_class(),
            java_fixed_state(item.get_fixed_state())
        );
        match item {
            Item::Trace(trace) => {
                let mut corners = String::new();
                for (c, corner) in trace.polyline().corners().iter().enumerate() {
                    if c > 0 {
                        corners.push(';');
                    }
                    let point = corner.to_float();
                    let _ = write!(
                        corners,
                        "{},{}",
                        java_double_to_string(point.x),
                        java_double_to_string(point.y)
                    );
                }
                rows.push(format!(
                    "{head} layer={} halfWidth={} corners=[{corners}]",
                    trace.get_layer(),
                    trace.get_half_width()
                ));
            }
            Item::Via(via) => {
                let center = via.get_center().to_float();
                rows.push(format!(
                    "{head} padstack={} center={},{}",
                    via.get_padstack(&ctx)
                        .map_or_else(|| "<null>".to_string(), |p| p.name.clone()),
                    java_double_to_string(center.x),
                    java_double_to_string(center.y)
                ));
            }
            Item::ConductionArea(zone) => {
                let mut area = String::new();
                for (i, corner) in zone
                    .get_relative_area()
                    .corner_approx_arr()
                    .iter()
                    .enumerate()
                {
                    if i > 0 {
                        area.push(';');
                    }
                    let _ = write!(
                        area,
                        "{},{}",
                        java_double_to_string(corner.x),
                        java_double_to_string(corner.y)
                    );
                }
                rows.push(format!(
                    "{head} layer={} isObstacle={} area=({area})",
                    zone.get_layer(),
                    zone.get_is_obstacle()
                ));
            }
            _ => rows.push(head),
        }
    }
    rows
}

fn java_class_name(item: &Item) -> &'static str {
    match item {
        Item::Trace(_) => "PolylineTrace",
        Item::Via(_) => "Via",
        Item::Pin(_) => "Pin",
        Item::ConductionArea(_) => "ConductionArea",
        Item::ObstacleArea(_) => "ObstacleArea",
        Item::ViaObstacleArea(_) => "ViaObstacleArea",
        Item::ComponentObstacleArea(_) => "ComponentObstacleArea",
        Item::ComponentOutline(_) => "ComponentOutline",
        Item::BoardOutline(_) => "BoardOutline",
    }
}

fn java_fixed_state(state: fr_board::FixedState) -> &'static str {
    match state {
        fr_board::FixedState::Unfixed => "NOT_FIXED",
        fr_board::FixedState::ShoveFixed => "SHOVE_FIXED",
        fr_board::FixedState::UserFixed => "USER_FIXED",
        fr_board::FixedState::SystemFixed => "SYSTEM_FIXED",
    }
}

#[test]
fn the_import_session_item_graph_matches_the_jvm() {
    let cases = session_cases();
    let mut compared = 0usize;
    let mut xdiff_seen = 0usize;
    for case in &cases {
        let actual = emit_session(case);
        let allowed: Vec<&str> = XDIFF
            .iter()
            .filter(|(stem, _)| *stem == case.stem)
            .map(|(_, prefix)| *prefix)
            .collect();
        let is_xdiff = |row: &String| allowed.iter().any(|prefix| row.starts_with(prefix));

        let expected_kept: Vec<&String> = case.expected.iter().filter(|r| !is_xdiff(r)).collect();
        let actual_kept: Vec<&String> = actual.iter().filter(|r| !is_xdiff(r)).collect();
        assert_eq!(
            actual_kept, expected_kept,
            "stem {}: the item graph diverged from the jar",
            case.stem
        );
        compared += actual_kept.len();

        let expected_diff: Vec<&String> = case.expected.iter().filter(|r| is_xdiff(r)).collect();
        let actual_diff: Vec<&String> = actual.iter().filter(|r| is_xdiff(r)).collect();
        for prefix in &allowed {
            let expected_rows: Vec<&&String> = expected_diff
                .iter()
                .filter(|r| r.starts_with(prefix))
                .collect();
            let actual_rows: Vec<&&String> = actual_diff
                .iter()
                .filter(|r| r.starts_with(prefix))
                .collect();
            assert_ne!(
                actual_rows, expected_rows,
                "stem {} prefix {prefix:?} is listed in XDIFF but matches — remove the entry",
                case.stem
            );
            xdiff_seen += 1;
        }
    }
    assert_eq!(compared, 142, "the replay compares 142 rows over 24 inputs");
    assert_eq!(
        xdiff_seen,
        XDIFF.len(),
        "every XDIFF entry was reached exactly once"
    );
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
            2.0,
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
