use copper_board::Board;
use copper_dsn::{BoardReadResult, kicad};
use serde_json::json;

fn imported(text: &str) -> Box<Board> {
    let BoardReadResult::Success {
        board: Some(board), ..
    } = kicad::read_board(text, None)
    else {
        panic!("KiCad import failed");
    };
    board
}

fn permissions(board: &Board) -> Vec<bool> {
    board
        .rules
        .net_classes
        .iter()
        .map(|class| {
            class
                .get_via_rule()
                .unwrap()
                .get_via(0)
                .attach_smd_allowed()
        })
        .collect()
}

fn payload() -> serde_json::Value {
    json!({"layers":[{"name":"F.Cu"},{"name":"B.Cu"}],
        "outline":{"corners":[{"x":0,"y":0},{"x":20,"y":0},{"x":20,"y":20},{"x":0,"y":20}]},
        "netClasses":[{"name":"Default"},{"name":"Power"}]})
}

#[test]
fn global_permission_defaults_off_and_survives_typed_round_trip() {
    for setting in [None, Some(false), Some(true)] {
        let mut input = payload();
        if let Some(allowed) = setting {
            input["viaInPadAllowed"] = json!(allowed);
        }
        let board = imported(&input.to_string());
        assert!(board.rules.strict_smd_via_attachment);
        assert_eq!(permissions(&board), vec![setting.unwrap_or(false); 2]);
        let output = kicad::write(&board, "permission");
        let dto: kicad::KiCadBoardJson = serde_json::from_str(&output).unwrap();
        assert_eq!(dto.viaInPadAllowed, setting.unwrap_or(false));
        assert_eq!(permissions(&imported(&output)), permissions(&board));
    }
}

#[test]
fn class_permissions_override_the_global_and_survive_mixed_round_trips() {
    for allowed in [false, true] {
        let mut input = payload();
        input["viaInPadAllowed"] = json!(!allowed);
        input["netClasses"][0]["viaInPadAllowed"] = json!(allowed);
        let mut board = imported(&input.to_string());
        assert_eq!(permissions(&board), vec![allowed, !allowed]);
        // The catalogue can disagree with the net-class rule copies.
        for id in 0..board.rules.via_infos.count() {
            board
                .rules
                .via_infos
                .get_mut(copper_board::ViaInfoId(id))
                .set_attach_smd_allowed(!allowed);
        }
        let output = kicad::write(&board, "mixed");
        let dto: kicad::KiCadBoardJson = serde_json::from_str(&output).unwrap();
        assert_eq!(dto.viaInPadAllowed, allowed);
        assert_eq!(dto.netClasses.unwrap()[1].viaInPadAllowed, Some(!allowed));
        assert_eq!(permissions(&imported(&output)), vec![allowed, !allowed]);
    }
}

#[test]
fn dsn_structure_permission_gates_the_exported_class_rules() {
    for control in ["", "(control (via_at_smd on))"] {
        let text = include_str!("data/network_via.dsn")
            .replace("(via v_a VA default)", "(via v_a VA default attach)")
            .replace("(via v_b VB default)", "(via v_b VB default attach)")
            .replace(
                "(net N1)",
                "(net N1) (class kicad_default N1 (circuit (use_via VA VB)))",
            )
            .replace("(structure", &format!("(structure {control}"));
        let BoardReadResult::Success {
            board: Some(board), ..
        } = copper_dsn::read_board(
            text.as_bytes(),
            None,
            None,
            &copper_dsn::DsnReadOptions::default(),
        )
        else {
            panic!("DSN import failed");
        };
        assert!(!board.rules.strict_smd_via_attachment);
        assert!(
            board
                .rules
                .via_infos
                .iter()
                .all(|via| via.attach_smd_allowed())
        );
        let output = kicad::write(&board, "dsn");
        let dto: kicad::KiCadBoardJson = serde_json::from_str(&output).unwrap();
        assert_eq!(dto.viaInPadAllowed, !control.is_empty());
        assert_eq!(permissions(&imported(&output)), permissions(&board));
    }
}
