use copper_board::{Board, Item};
use copper_dsn::error::BoardReadResult;
use copper_dsn::kicad::{KiCadBoardJson, read_board, write};

fn read(json: &str) -> Board {
    match read_board(json, None) {
        BoardReadResult::Success { board: Some(b), .. } => *b,
        other => panic!("expected a loaded board, got {other:?}"),
    }
}

fn outline_clearance_class(board: &Board) -> usize {
    match board.get_outline().and_then(|id| board.get_item(id)) {
        Some(Item::BoardOutline(outline)) => outline.hdr.clearance_class(),
        other => panic!("expected a board outline item, got {other:?}"),
    }
}

const BOARD_WITH_A_MAPPED_OUTLINE_CLEARANCE: &str = r#"{
    "layers":[{"name":"F.Cu"},{"name":"B.Cu"}],
    "netClasses":[{"name":"power","clearance":0.3}],
    "outline":{"corners":[{"x":0,"y":0},{"x":10,"y":0},{"x":10,"y":10},{"x":0,"y":10}],
               "clearance":0.3}
}"#;

#[test]
fn the_outline_clearance_round_trips() {
    let board = read(BOARD_WITH_A_MAPPED_OUTLINE_CLEARANCE);
    assert_eq!(
        outline_clearance_class(&board),
        2,
        "the outline takes the `power` class, not Java's hard-coded 1"
    );

    let written: KiCadBoardJson =
        serde_json::from_str(&write(&board, "round-trip")).expect("the writer emits valid JSON");
    let outline = written
        .outline
        .expect("the writer always writes an outline");
    assert!(
        (outline.clearance - 0.3).abs() < 1e-9,
        "the clearance comes back as it went in, got {}",
        outline.clearance
    );

    let reread = read(&write(&board, "round-trip"));
    assert_eq!(outline_clearance_class(&reread), 2);
}

#[test]
fn an_outline_clearance_matching_no_class_keeps_the_default() {
    let board = read(
        r#"{"layers":[{"name":"F.Cu"},{"name":"B.Cu"}],
            "netClasses":[{"name":"power","clearance":0.3}],
            "outline":{"corners":[{"x":0,"y":0},{"x":10,"y":0},{"x":10,"y":10}],
                       "clearance":0.75}}"#,
    );
    assert_eq!(outline_clearance_class(&board), 1);
}

#[test]
fn an_absent_outline_clearance_keeps_the_default() {
    let board = read(
        r#"{"layers":[{"name":"F.Cu"},{"name":"B.Cu"}],
            "netClasses":[{"name":"power","clearance":0.3}],
            "outline":{"corners":[{"x":0,"y":0},{"x":10,"y":0},{"x":10,"y":10}]}}"#,
    );
    assert_eq!(outline_clearance_class(&board), 1);
}

#[test]
fn an_outline_clearance_that_matches_the_default_takes_the_default() {
    let board = read(
        r#"{"layers":[{"name":"F.Cu"},{"name":"B.Cu"}],
            "outline":{"corners":[{"x":0,"y":0},{"x":10,"y":0},{"x":10,"y":10}],
                       "clearance":0.2}}"#,
    );
    assert_eq!(outline_clearance_class(&board), 1);

    let board = read(
        r#"{"layers":[{"name":"F.Cu"},{"name":"B.Cu"}],
            "netClasses":[{"name":"power","clearance":0.2}],
            "outline":{"corners":[{"x":0,"y":0},{"x":10,"y":0},{"x":10,"y":10}],
                       "clearance":0.2}}"#,
    );
    assert_eq!(
        outline_clearance_class(&board),
        1,
        "class 1 and class 2 are both 2000; the ascending search answers the default"
    );
}

#[test]
fn the_other_three_dto_fields_still_round_trip() {
    let source = r#"{
        "layers":[{"index":7,"name":"F.Cu"},{"index":9,"name":"B.Cu"}],
        "netClasses":[{"name":"power","clearance":0.3,"netNames":["GND","VCC"]}],
        "nets":[{"id":41,"name":"GND","className":"power"}]
    }"#;
    let parsed: KiCadBoardJson = serde_json::from_str(source).expect("parses");
    let layers = parsed.layers.as_ref().expect("layers");
    assert_eq!((layers[0].index, layers[1].index), (7, 9));
    assert_eq!(
        parsed.netClasses.as_ref().expect("netClasses")[0]
            .netNames
            .as_deref(),
        Some(["GND".to_string(), "VCC".to_string()].as_slice())
    );
    assert_eq!(parsed.nets.as_ref().expect("nets")[0].id, 41);

    let board = read(source);
    assert_eq!(board.rules.nets.iter().next().expect("a net").net_number, 1);
    let round_tripped: KiCadBoardJson =
        serde_json::from_str(&write(&board, "wire-contract")).expect("valid JSON");
    assert!(
        round_tripped.layers.is_some() && round_tripped.nets.is_some(),
        "the writer still emits the sections these fields live in"
    );
}
