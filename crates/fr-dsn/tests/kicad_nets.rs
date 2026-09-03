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

fn board(json: &str) -> Board {
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
    let board = board(&fixture(
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
    let board = board(
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

/// A **declared** net keeps its declared number, and the auto-registered ones continue from
/// there. That half was never hash-ordered and must not become so.
#[test]
fn a_declared_net_keeps_its_position_and_the_rest_follow() {
    let board = board(
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
