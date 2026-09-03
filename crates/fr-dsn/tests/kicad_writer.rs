//! `fr_dsn::kicad::writer::write` and `fr_dsn::kicad::reader::import_session` — the write half of
//! the KiCad board-JSON codec and the session importer beside it.
//!
//! Java authority: `io/kicad/KiCadJsonWriter.java:27-226` and
//! `io/kicad/KiCadJsonReader.java:757-855`.
//!
//! # The ground truth
//!
//! `data/p8t10-kicad-writer.txt` is the byte-exact stdout of
//! `scripts/differential/java/probes/P8T10Probe.java` against the pinned HEAD jar under JDK 25
//! (the probe's header comment carries the exact `javac`/`java` invocation). It measures
//!
//! * **nine writer inputs** — four real KiCad exports the checkout ships under `fixtures/`
//!   (including both boards in the corpus that carry wiring), a synthetic MIL board with two
//!   traces, two vias and a conduction area, a UM board, the empty document, a `null` design name
//!   and the one-argument `write(RoutingBoard)` overload — each with `write`'s **exact** output,
//!   line for line, plus its UTF-8 byte length and the `write → readBoard → write` round trip's
//!   verdict; and
//! * **twenty-four `importSession` inputs**, each a base board plus a session document, with the
//!   board's whole item graph afterwards (or the throwable, class and message).
//!
//! [`the_writer_output_matches_the_jvm_byte_for_byte`] and
//! [`the_import_session_item_graph_matches_the_jvm`] replay them. Neither tolerates a differing
//! line except through [`XDIFF`], which asserts that the row still **differs** so a divergence
//! that starts matching fails the test rather than rotting.
//!
//! The named tests after the two replays pin, as literals, what the task brief calls out by name.
//!
//! **One of the brief's tests is not here.** The quirk-#289 (label T) decision test is
//! `crates/freerouting/tests/cli_e2e.rs::do_out_json_writes_the_routed_board`: the quirk is a
//! CLI-path behaviour — in the jar `setJobOutput` fires as a board-updated listener before the
//! router runs, and only that first call ever writes — so the assertion has to be the
//! **binary**'s output file. `fr-dsn` has no binary and no pipeline, so it cannot host it. What
//! *this* file pins is the writer that produces the bytes either way.
//!
//! **Plan 9 Task 3 fixed the quirk and changed what that test asserts.** It used to be
//! `do_out_json_writes_the_pre_routing_board` and it pasted the jar's own 1 540 bytes in as a
//! literal; the port now serialises **once, after the pipeline**, so `-do out.json` carries the
//! routed board and the test cross-checks the document's `traces` count against the `(wire `
//! count of the SES from the identical argv instead of against a pasted document. Nothing in
//! *this* file moved with it: `write` is called with whichever board its caller hands it, and
//! Task 3 changed only which board that is.

use std::fmt::Write as _;

use fr_board::{Board, Item};
use fr_dsn::error::{BoardReadResult, DsnError};
use fr_dsn::format::java_double_to_string;
use fr_dsn::kicad::{DEFAULT_DESIGN_NAME, import_session, read_board, write};

const TRANSCRIPT: &str = include_str!("data/p8t10-kicad-writer.txt");

/// The Java checkout's `fixtures/` directory, honouring `FREEROUTING_JAVA_DIR` the way the rest
/// of the suite does.
fn fixture(relative: &str) -> String {
    let root = std::env::var("FREEROUTING_JAVA_DIR").unwrap_or_else(|_| {
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../../freerouting").to_string()
    });
    std::fs::read_to_string(format!("{root}/{relative}"))
        .unwrap_or_else(|e| panic!("fixture {relative}: {e}"))
}

/// The inverse of `P8T10Probe.escape`.
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

/// The board a document reads into, or `None` when the read produced none — `P8T10Probe.boardOf`.
fn board_of(json: &str) -> Option<Box<Board>> {
    match read_board(json, None) {
        BoardReadResult::Success { board, .. } | BoardReadResult::OutlineMissing { board, .. } => {
            board
        }
        _ => None,
    }
}

// =============================================================== the writer replay

/// One `[wcase]` of the transcript.
struct WriterCase {
    stem: String,
    design_name: String,
    json: String,
    /// The `[w]bytes=` row, then every `[w]|` row joined with `\n`, then the `[rt]` row.
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

/// Re-emits the probe's `[w]`/`[rt]` rows from the Rust writer.
fn emit_writer(case: &WriterCase) -> Vec<String> {
    let mut rows: Vec<String> = Vec::new();
    let Some(board) = board_of(&case.json) else {
        rows.push("[w] result=<no board>".to_string());
        return rows;
    };
    // `<one-arg>` is the probe's marker for `KiCadJsonWriter.write(RoutingBoard)` (`:27-31`), and
    // `<null>` for a `null` `designName`, which `:45` collapses to the same literal.
    let design_name = match case.design_name.as_str() {
        "<one-arg>" | "<null>" => DEFAULT_DESIGN_NAME,
        other => other,
    };
    let written = write(&board, design_name);
    rows.push(format!("[w]bytes={}", written.len()));
    for line in written.split('\n') {
        rows.push(format!("[w]|{line}"));
    }

    // The round trip, exactly as the probe performs it: read the written document back and write
    // it again, both times under the same design name so only the *board* can differ.
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

/// Every `[w]`/`[rt]` row of every writer case, byte for byte against the pinned jar.
///
/// `[w]bytes=` is Java's `getBytes(UTF_8).length` and Rust's `String::len()` — the same number,
/// because both count UTF-8 bytes. `[rt] bytes=` is Java's `String.length()`, i.e. **UTF-16 code
/// units**, which is `chars().count()` here for every document in the corpus (none carries an
/// astral-plane character; the one non-ASCII stem is on the `importSession` side).
#[test]
fn the_writer_output_matches_the_jvm_byte_for_byte() {
    let cases = writer_cases();
    let mut compared = 0usize;
    for case in &cases {
        let actual = emit_writer(case);
        assert_eq!(
            actual.len(),
            case.expected.len(),
            "row count for stem {}",
            case.stem
        );
        for (row, (actual, expected)) in actual.iter().zip(&case.expected).enumerate() {
            assert_eq!(
                actual, expected,
                "stem {} row {row}: the writer diverged from the jar",
                case.stem
            );
            compared += 1;
        }
    }
    assert_eq!(
        compared, 5021,
        "the replay compares 5021 rows over 9 boards"
    );
}

// =============================================================== the importSession replay

/// One `[icase]` of the transcript.
struct SessionCase {
    stem: String,
    base: String,
    session: String,
    expected: Vec<String>,
}

/// The rows the port is expected to answer **differently**, each with its root cause.
///
/// Every entry is asserted to still differ: an entry that starts matching fails the test.
const XDIFF: &[(&str, &str)] = &[
    // Quirk #277's parser prose. Gson names the offending token and byte position of its own
    // `JsonReader`; `serde_json`'s message is its own. Both refuse, and both refuse before
    // inserting anything, which is the behaviour the row after it pins.
    ("json-truncated", "[is] throw="),
    // Quirk #286's second half. `BasicBoard.insertVia` puts the via in the **item list**
    // (BasicBoard.java:286 -> BoardItemRepository.insertItem:160) and *then* throws inside the
    // search-tree update, so the jar's board keeps a via that is in no tree. `fr_board`'s
    // `SearchTrees::insert` computes the same negative `tileShapeCount` and panics rather than
    // throwing, so the port refuses the via **before** inserting it and the board is one item
    // short. The refusal point, the message and the padstack registration are identical; the
    // divergence is exactly the one unusable item. See docs/java-quirks.md #286.
    //
    // Reachability, stated accurately rather than as "KiCad never writes it": **freerouting's own
    // writer can spell `startLayerIndex > endLayerIndex`.** `KiCadJsonWriter.write:184-193` walks
    // `firstLayer` up to `layerCount` and `lastLayer` down to `-1` when no layer has a shape, and
    // emits both verbatim — so a board carrying an all-`null`-padstack via writes
    // `"startLayerIndex": <layerCount>, "endLayerIndex": -1`. The only board that can carry such
    // a via is the half-inserted one **this very quirk produces**, so the input is either that
    // round trip or a hand-edited document; no KiCad export reaches it.
    ("via-start-gt-end", "[is] items count="),
    ("via-start-gt-end", "[is] item 2 Via"),
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

/// Re-emits the probe's `[is]` rows from a Rust board that `import_session` has run over.
fn emit_session(case: &SessionCase) -> Vec<String> {
    let mut rows: Vec<String> = Vec::new();
    let Some(mut board) = board_of(&case.base) else {
        rows.push("[is] base=<no board>".to_string());
        return rows;
    };
    match import_session(&case.session, &mut board) {
        Ok(()) => rows.push("[is] result=ok".to_string()),
        // `DsnError::KicadSession`'s payload **is** `getClass().getName() + ": " + getMessage()`,
        // which is what the probe prints.
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

/// `Item.getClass().getSimpleName()`, for the four item kinds this corpus produces.
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

/// `FixedState.toString()` — the Java enum constant names.
fn java_fixed_state(state: fr_board::FixedState) -> &'static str {
    match state {
        fr_board::FixedState::Unfixed => "NOT_FIXED",
        fr_board::FixedState::ShoveFixed => "SHOVE_FIXED",
        fr_board::FixedState::UserFixed => "USER_FIXED",
        fr_board::FixedState::SystemFixed => "SYSTEM_FIXED",
    }
}

/// Every `[is]` row of every session case, against the pinned jar.
///
/// Rows whose prefix is listed in [`XDIFF`] for the stem are lifted out of **both** sides before
/// the comparison and then asserted to differ — lifting them out of both is what keeps the
/// remaining rows aligned when the divergence is a *missing* row rather than a changed one, which
/// is exactly `via-start-gt-end`'s shape.
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
    assert_eq!(compared, 145, "the replay compares 145 rows over 24 inputs");
    assert_eq!(
        xdiff_seen,
        XDIFF.len(),
        "every XDIFF entry was reached exactly once"
    );
}

// =============================================================== the named tests

/// `KiCadJsonWriter.write` emits **no components**, so a `read → write` round trip drops every
/// component, pad and padstack the document carried — and the *second* write is then a fixed
/// point because there is nothing left to drop.
///
/// **Measured on the HEAD jar** (the `[rt]` rows of the transcript): **seven** of the nine writer
/// stems are fixed points at the first re-write and **two** are not, and the two that are not are
/// the two that carry **more than one trace**. That is not a rounding problem — it is item
/// ordering. `board.getTraces()` walks the item list in **descending id** (quirk #63), the writer
/// numbers `traceJson.id` 1..N in that order, and `readBoard` then inserts them in document order
/// so the *new* ids ascend in the *old* walk order — which the next `getTraces()` walk reads back
/// **reversed**. `mil-full` (2 traces) and `complex-hierarchy-session` (172) therefore come back
/// with their trace list turned round; `ecc83-v1`, `corney-island-session` and the four synthetic
/// stems have at most one trace each and are fixed points. Both halves are asserted below, so a
/// change to either the walk order or the writer's numbering fails here.
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

    // The other half: two traces come back in the other order, and a *third* write undoes it.
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
    // Naming the mechanism, not just the symptom: the first document's first trace is the last
    // item inserted, i.e. net "B".
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

/// `importSession` inserts the session's wiring onto a board that already carries some
/// (`RoutingJobScheduler.java:194-207`'s `-di`, and `Freerouting.java:301-307`'s `-drc`;
/// `:208` opens the SES `else`, so the `.json` arm ends at `:207`).
///
/// The literals are the transcript's `onto-existing-wiring` stem: one pre-existing trace on the
/// base board, one via from the session, and the via's generated padstack registered beside the
/// board's own `defaultVia`.
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
    // `:844-847`'s generated name, with both `%.0f`s in micrometres.
    let via = board.get_vias()[0];
    let ctx = board.ctx();
    let Some(Item::Via(via)) = board.get_item(via) else {
        panic!("a via")
    };
    assert_eq!(
        via.get_padstack(&ctx).expect("registered").name,
        "Via[0-1]_800:400_um"
    );
    // The centre is `round(x * scaleFactor)`, with Y negated (`:824-827`).
    let center = via.get_center().to_float();
    assert_eq!((center.x, center.y), (5000.0, -1000.0));
    // The pre-existing trace is **not** split, and that is measured, not assumed: the transcript's
    // `onto-existing-wiring` stem answers `items count=3` — the outline, the one trace and the
    // via. `BasicBoard.insertVia:289` loops `for (i = fromLayer; i < toLayer; i++)`, an
    // **exclusive** upper bound, so a via spanning layers 0-1 calls `splitTraces` on layer 0
    // alone, and `splitTraces` there finds no trace to split at that point. The checked seam is
    // still the right one — `split_traces` *is* reached, and on a board that already carries
    // wiring, which is exactly the case `readBoard` never sees.
    assert_eq!(board.get_traces().len(), 1, "the trace is left whole");
}

/// **Quirk #290 (label U)**: `Freerouting.java:304` opens the session with
/// `new java.io.FileReader(sessionFile)` — the **platform default charset** — where every other
/// JSON path in the tree names UTF-8 explicitly. The port reads UTF-8.
///
/// This test pins the *port* side: a non-ASCII net name survives the decode and the net lookup,
/// so the trace lands on net 1 and not on net 0. The transcript's `non-ascii-net` stem is the
/// jar's own answer to the identical document, handed to `importSession` through a `StringReader`
/// so that only the matching is measured. The **charset** half — which the two agree on for JDK
/// 18+ and disagree on below it — is measured end to end by
/// `crates/freerouting/tests/cli_e2e.rs::a_non_ascii_session_file_is_read_as_utf8`; see
/// `docs/java-quirks.md` #290 for the jar transcript.
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

/// The three lists `importSession` guards and `readBoard` does not.
///
/// `readBoard:667`/`:684`/`:648` dereference `boardJson.traces`, `.vias` and `.conductionAreas`
/// with no null check — an explicit `"traces": null` is a `NullPointerException` there — while
/// `importSession:777`/`:797`/`:817` each test `!= null` first. Measured, stem `all-lists-null`:
/// the jar answers `result=ok` with the board untouched.
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

    // The same three keys through `readBoard` are the `NullPointerException` quirk #283 records.
    let with_null_traces = base.replace("\"outline\"", "\"traces\":null,\"outline\"");
    assert!(
        matches!(
            read_board(&with_null_traces, None),
            BoardReadResult::ParseError { .. }
        ),
        "readBoard does not guard them"
    );
}

/// `importSession:770-773`'s resolution ladder, which `readBoard` has no counterpart for.
///
/// `(int) Math.max(1.0, resolution)` truncates toward zero, and a document that leaves
/// `KiCadBoardJson.resolution` at its `= 1.0` initialiser **in millimetres** is then read at
/// `10000`, not at `1`. The same document in MIL keeps `1`. Measured, stems `resolution-one-mm`,
/// `resolution-one-mil`, `resolution-fractional` and `resolution-below-one`.
#[test]
fn an_absent_resolution_in_mm_means_ten_thousand() {
    let base = "{\"unit\":\"MM\",\"resolution\":1000.0,\
        \"layers\":[{\"index\":0,\"name\":\"F.Cu\",\"type\":\"signal\"}],\
        \"nets\":[{\"id\":1,\"name\":\"GND\",\"className\":\"default\"}],\
        \"outline\":{\"corners\":[{\"x\":0.0,\"y\":0.0},{\"x\":50.0,\"y\":0.0},\
        {\"x\":50.0,\"y\":40.0},{\"x\":0.0,\"y\":40.0}]}}";
    let trace = "\"traces\":[{\"id\":1,\"netName\":\"GND\",\"width\":0.25,\"layerIndex\":0,\
        \"points\":[{\"x\":1.0,\"y\":1.0},{\"x\":2.0,\"y\":1.0}]}]";

    // The measured first corner for each (unit, resolution) pair.
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
