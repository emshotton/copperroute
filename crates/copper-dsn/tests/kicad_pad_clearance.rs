use copper_dsn::error::BoardReadResult;
use serde_json::json;

fn input(clearance: Option<f64>) -> serde_json::Value {
    let mut pad = json!({"name":"1","netName":"A","shape":"rect","size":{"x":1.0,"y":1.0},"layers":["F.Cu","B.Cu"]});
    if let Some(value) = clearance {
        pad["copperClearance"] = json!(value);
    }
    json!({"resolution":10000,"layers":[{"name":"F.Cu"},{"name":"B.Cu"}],
        "netClasses":[{"name":"Default","clearance":0.1,"traceWidth":0.2}],
        "nets":[{"id":1,"name":"A","className":"Default"},{"id":2,"name":"B","className":"Default"}],
        "components":[{"reference":"J1","position":{"x":0,"y":0},"layer":"F.Cu","pads":[pad]},
        {"reference":"J2","position":{"x":1.2,"y":0},"layer":"F.Cu","pads":[{"name":"1","netName":"B","shape":"rect","size":{"x":1.0,"y":1.0},"layers":["F.Cu","B.Cu"]}]}]})
}

fn read(value: serde_json::Value) -> copper_board::Board {
    match copper_dsn::kicad::read_board(&value.to_string(), None) {
        BoardReadResult::Success {
            board: Some(board), ..
        } => *board,
        result => panic!("{result:?}"),
    }
}

#[test]
fn copper_clearance_is_not_capped_to_an_existing_pad_gap() {
    let before = read(input(None));
    let board = read(input(Some(0.3)));
    let pin = board
        .get_pins()
        .into_iter()
        .find(|id| {
            let item = board.get_item(*id).unwrap();
            board.components.get(item.component_id()).name == "J1"
        })
        .unwrap();
    let class = board.get_item(pin).unwrap().clearance_class();
    let old = before.get_item(pin).unwrap().clearance_class();
    assert_eq!(board.solder_mask_clearance_limit(pin, 0, 3000), 2000);
    for layer in 0..2 {
        assert!(
            before
                .rules
                .clearance_matrix
                .get_value(old, 1, layer, false)
                < 3000
        );
        assert!(
            board
                .rules
                .clearance_matrix
                .get_value(class, 1, layer, false)
                >= 3000
        );
        assert!(
            board
                .rules
                .clearance_matrix
                .get_value(1, class, layer, false)
                >= 3000
        );
    }
}

#[test]
fn copper_floor_rounds_up_and_preserves_larger_rules() {
    for clearance in [0.30001, 0.01, 0.0, -0.1] {
        let board = read(input(Some(clearance)));
        let pin = board
            .get_pins()
            .into_iter()
            .find(|id| {
                board
                    .components
                    .get(board.get_item(*id).unwrap().component_id())
                    .name
                    == "J1"
            })
            .unwrap();
        let class = board.get_item(pin).unwrap().clearance_class();
        let baseline = read(input(None));
        let old_class = baseline.get_item(pin).unwrap().clearance_class();
        for layer in 0..2 {
            let actual = board
                .rules
                .clearance_matrix
                .get_value(class, 1, layer, false);
            assert!(actual >= (clearance * 10000.0).ceil() as i32);
            assert!(
                actual
                    >= baseline
                        .rules
                        .clearance_matrix
                        .get_value(old_class, 1, layer, false)
            );
        }
    }
}

#[test]
fn excessive_local_copper_clearance_is_rejected() {
    assert!(!matches!(
        copper_dsn::kicad::read_board(&input(Some(1e12)).to_string(), None),
        BoardReadResult::Success { .. }
    ));
}
