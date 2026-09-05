use fr_board::Board;
use fr_dsn::error::BoardReadResult;
use fr_dsn::kicad::read_board;

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

fn numbered_nets(board: &Board) -> Vec<(i32, String)> {
    board
        .rules
        .nets
        .iter()
        .map(|net| (net.net_number, net.name.clone()))
        .collect()
}

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
