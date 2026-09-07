use copper_board::Board;
use copper_dsn::error::BoardReadResult;
use copper_dsn::kicad::read_board;

fn board(json: &str) -> Board {
    match read_board(json, None) {
        BoardReadResult::Success { board: Some(b), .. } => *b,
        other => panic!("expected a loaded board, got {other:?}"),
    }
}

#[test]
fn three_components_with_unnamed_pads_yield_one_package() {
    let board = board(
        r#"{"layers":[{"name":"F.Cu"},{"name":"B.Cu"}],
            "components":[
              {"reference":"U1","footprint":"SOT-23","position":{"x":0,"y":0},
               "pads":[{"name":"","shape":"rect","size":{"x":1.0,"y":1.0},"layers":["F.Cu"]}]},
              {"reference":"U2","footprint":"SOT-23","position":{"x":5,"y":0},
               "pads":[{"name":"","shape":"rect","size":{"x":1.0,"y":1.0},"layers":["F.Cu"]}]},
              {"reference":"U3","footprint":"SOT-23","position":{"x":10,"y":0},
               "pads":[{"name":"","shape":"rect","size":{"x":1.0,"y":1.0},"layers":["F.Cu"]}]}
            ]}"#,
    );
    assert_eq!(
        board.library.packages.count(),
        1,
        "three identical components share one package"
    );
    assert_eq!(board.library.packages.get(1).name, "SOT-23");
    assert_eq!(board.components.count(), 3, "and all three still load");
}

#[test]
fn three_components_with_different_pins_still_yield_three_packages() {
    let board = board(
        r#"{"layers":[{"name":"F.Cu"},{"name":"B.Cu"}],
            "components":[
              {"reference":"U1","footprint":"SOT-23","position":{"x":0,"y":0},
               "pads":[{"name":"1","shape":"rect","size":{"x":1.0,"y":1.0},"layers":["F.Cu"]}]},
              {"reference":"U2","footprint":"SOT-23","position":{"x":5,"y":0},
               "pads":[{"name":"2","shape":"rect","size":{"x":1.0,"y":1.0},"layers":["F.Cu"]}]},
              {"reference":"U3","footprint":"SOT-23","position":{"x":10,"y":0},
               "pads":[{"name":"3","shape":"rect","size":{"x":1.0,"y":1.0},"layers":["F.Cu"]}]}
            ]}"#,
    );
    assert_eq!(board.library.packages.count(), 3);
    assert_eq!(board.library.packages.get(1).name, "SOT-23");
    assert_eq!(board.library.packages.get(2).name, "SOT-23::1");
    assert_eq!(board.library.packages.get(3).name, "SOT-23::2");
}
