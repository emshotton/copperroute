//! **Net numbering** on the KiCad board-JSON path — `KiCadJsonReader.java:454-496`.
//!
//! A KiCad board need not declare its nets: `readBoard` collects every net name a pad, a
//! conduction area, a trace or a via mentions into a `java.util.HashSet<String>` and then numbers
//! them in the order the set's iterator hands them back. That order is
//! `String.hashCode`'s — deterministic, and otherwise arbitrary — and the numbering it produces is
//! what every `netNumbers[]`, every DSN `(net …)` scope and every SES wire carries.
//!
//! fixed: T7 (#280) — first-reference order, which is Java's own one-word fix (`LinkedHashSet`).

use fr_board::Board;
use fr_dsn::error::BoardReadResult;
use fr_dsn::kicad::read_board;

/// The Java checkout's `fixtures/` directory, honouring `FREEROUTING_JAVA_DIR` as the rest of the
/// suite does.
fn fixture(relative: &str) -> String {
    let root = std::env::var("FREEROUTING_JAVA_DIR").unwrap_or_else(|_| {
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../../freerouting").to_string()
    });
    std::fs::read_to_string(format!("{root}/{relative}"))
        .unwrap_or_else(|e| panic!("fixture {relative}: {e}"))
}

fn read(json: &str) -> Board {
    match read_board(json, None) {
        BoardReadResult::Success { board: Some(b), .. } => *b,
        other => panic!("expected a loaded board, got {other:?}"),
    }
}

/// The board's nets as `(number, name)`, in net-number order and without net 0.
fn numbered_nets(board: &Board) -> Vec<(i32, String)> {
    board
        .rules
        .nets
        .iter()
        .map(|net| (net.net_number, net.name.clone()))
        .collect()
}

/// `Issue649-kicad_ecc83-pp_input_board_v1.json` declares **no** nets at all: all thirteen come
/// from the pads that reference them, so every one of them is numbered by the auto-registration
/// loop. This is the fixture the fix list names, and its numbering is the whole of the golden
/// movement T7 causes on `tests/reference/cli-kicad-ecc83-json/`.
///
/// The expectation is the order the file first mentions each name, walking components in order
/// and each component's pads in order — the order a reader of the JSON would predict, which is
/// the point.
#[test]
fn net_numbers_follow_declaration_order() {
    let board = read(&fixture(
        "fixtures/Issue649-kicad_ecc83-pp_input_board_v1.json",
    ));
    assert_eq!(
        numbered_nets(&board),
        [
            (1, "Net-(P3-P1)".to_string()),
            (2, "GND".to_string()),
            (3, "Net-(P2-P1)".to_string()),
            (4, "Net-(U1A-K)".to_string()),
            (5, "unconnected-(P5-Pad1)".to_string()),
            (6, "unconnected-(P6-Pad1)".to_string()),
            (7, "unconnected-(P7-Pad1)".to_string()),
            (8, "Net-(U1A-G)".to_string()),
            (9, "Net-(U1B-K)".to_string()),
            (10, "Net-(P1-PM)".to_string()),
            (11, "Net-(P4-P1)".to_string()),
            (12, "Net-(P4-PM)".to_string()),
            (13, "unconnected-(P8-Pad1)".to_string()),
        ],
        "the order the file first mentions each name — derived from the JSON independently of \
         the reader, walking components then conduction areas then traces then vias"
    );
}

/// The same rule on a payload small enough to read: four nets referenced in an order no hash
/// would reproduce, numbered 1..4 in the order they appear.
#[test]
fn an_auto_registered_net_takes_the_next_number_at_first_reference() {
    let board = read(
        r#"{"layers":[{"name":"F.Cu"},{"name":"B.Cu"}],
            "components":[{"reference":"U1","footprint":"F","position":{"x":0,"y":0},
              "pads":[
                {"name":"1","netName":"zeta","size":{"x":1.0,"y":1.0},"layers":["F.Cu"]},
                {"name":"2","netName":"alpha","size":{"x":1.0,"y":1.0},"layers":["F.Cu"]},
                {"name":"3","netName":"zeta","size":{"x":1.0,"y":1.0},"layers":["F.Cu"]},
                {"name":"4","netName":"mu","size":{"x":1.0,"y":1.0},"layers":["F.Cu"]}]}],
            "traces":[{"netName":"beta","width":0.2,"layerIndex":0,
                       "points":[{"x":0,"y":0},{"x":1,"y":1}]}]}"#,
    );
    assert_eq!(
        numbered_nets(&board),
        [
            (1, "zeta".to_string()),
            (2, "alpha".to_string()),
            (3, "mu".to_string()),
            (4, "beta".to_string()),
        ],
        "a repeat reference does not renumber, and the traces are walked after the pads"
    );
}

/// **#280's second consumer.** `resolveNetClassIndex:972-976` falls back to an
/// `equalsIgnoreCase` walk of `netClassIndexMap.entrySet()` when no class matches the net's
/// `className` exactly — so with two classes differing only in case, which one a net lands on was
/// `java.util.HashMap`'s bucket order, exactly as the net numbering was.
///
/// The tie-break is declaration order now: the net asking for `"power"` matches neither `"Power"`
/// nor `"POWER"` exactly, hits the fallback, and gets the **first** one its own file declares.
/// Two candidates are the whole point — with one, every order agrees.
///
/// Clearance class 0 is the reserved `"null"` row and 1 is `"default"`, so the two declared
/// classes are clearance classes 2 and 3 and `NetClass` ids 1 and 2 (`readBoard:450`'s deliberate
/// `clNo - 1`). `"Power"` is declared first, so the answer is `NetClassId(1)`.
///
/// **What the jar answers, computed from its own model**: `HashMap`'s bucket order at capacity 16
/// puts `"POWER"` first for **both** declaration orders, so the jar picks `"POWER"` either way —
/// which is the arbitrariness stated as plainly as it can be. The first half of this test
/// therefore fails against the un-fixed reader (it answered `NetClassId(2)`) and the second half
/// passes by coincidence; together they pin the order rather than one board's luck.
#[test]
fn a_net_class_named_only_by_case_takes_the_first_declared() {
    let board = read(
        r#"{"layers":[{"name":"F.Cu"},{"name":"B.Cu"}],
            "netClasses":[{"name":"Power","clearance":0.3},{"name":"POWER","clearance":0.4}],
            "nets":[{"name":"VCC","className":"power"}]}"#,
    );
    let net = board.rules.nets.iter().next().expect("the one net");
    assert_eq!(net.name, "VCC");
    assert_eq!(
        net.get_net_class(),
        fr_board::NetClassId(1),
        "`Power` is declared first, so the case-insensitive fallback answers it"
    );

    // Swap the declaration order and the answer swaps with it — which is what makes this a test
    // of the *order* rather than of one board's happenstance.
    let board = read(
        r#"{"layers":[{"name":"F.Cu"},{"name":"B.Cu"}],
            "netClasses":[{"name":"POWER","clearance":0.4},{"name":"Power","clearance":0.3}],
            "nets":[{"name":"VCC","className":"power"}]}"#,
    );
    assert_eq!(
        board
            .rules
            .nets
            .iter()
            .next()
            .expect("the one net")
            .get_net_class(),
        fr_board::NetClassId(1),
        "still the first declared — now `POWER`"
    );
    assert_eq!(
        board
            .rules
            .net_classes
            .get(fr_board::NetClassId(1))
            .get_name(),
        "POWER"
    );
}

/// An **exact** match still wins over the case-insensitive fallback, so the tie-break above only
/// ever decides a case that has no exact answer.
#[test]
fn an_exact_net_class_name_beats_the_case_insensitive_fallback() {
    let board = read(
        r#"{"layers":[{"name":"F.Cu"},{"name":"B.Cu"}],
            "netClasses":[{"name":"Power","clearance":0.3},{"name":"POWER","clearance":0.4}],
            "nets":[{"name":"VCC","className":"POWER"}]}"#,
    );
    assert_eq!(
        board
            .rules
            .nets
            .iter()
            .next()
            .expect("the one net")
            .get_net_class(),
        fr_board::NetClassId(2),
        "`POWER` matches exactly, and the exact `HashMap.get` runs first (`:968-971`)"
    );
}

/// A **declared** net keeps its declared number, and the auto-registered ones continue from
/// there. That half was never hash-ordered and must not become so.
#[test]
fn a_declared_net_keeps_its_position_and_the_rest_follow() {
    let board = read(
        r#"{"layers":[{"name":"F.Cu"},{"name":"B.Cu"}],
            "nets":[{"name":"GND"},{"name":"VCC"}],
            "components":[{"reference":"U1","footprint":"F","position":{"x":0,"y":0},
              "pads":[
                {"name":"1","netName":"SIG","size":{"x":1.0,"y":1.0},"layers":["F.Cu"]},
                {"name":"2","netName":"GND","size":{"x":1.0,"y":1.0},"layers":["F.Cu"]}]}]}"#,
    );
    assert_eq!(
        numbered_nets(&board),
        [
            (1, "GND".to_string()),
            (2, "VCC".to_string()),
            (3, "SIG".to_string()),
        ]
    );
}
