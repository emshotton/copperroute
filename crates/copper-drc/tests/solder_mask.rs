mod common;

use common::synthetic::{PadSpec, SyntheticBoard};
use copper_board::prelude::*;
use copper_drc::DesignRulesChecker;
use copper_geometry::IntVector;

#[test]
fn a_track_outside_the_rounded_mask_corner_is_not_a_bridge() {
    let oracle: serde_json::Value =
        serde_json::from_str(include_str!("data/solder-mask-corner/drc.json")).unwrap();
    assert!(
        oracle["violations"]
            .as_array()
            .unwrap()
            .iter()
            .all(|v| v["type"] != "solder_mask_bridge")
    );
    let mut fixture = SyntheticBoard::new(
        &[PadSpec {
            name: "1",
            half: 5000,
            offset: IntVector::new(0, 0),
            through_hole: false,
        }],
        2,
        1000,
    );
    let id = fixture.pin(0, 1);
    let Item::Pin(pin) = fixture.board.items.get_mut(&id).unwrap() else {
        unreachable!()
    };
    pin.solder_mask_expansion.insert(0, 2000);
    fixture.trace(&[(7400, 6000), (9000, 6000)], 0, 500, 2);
    fixture.via(9600, 7000, 2);
    let violations = DesignRulesChecker::new(&mut fixture.board).get_all_violations();
    assert!(
        violations
            .iter()
            .all(|v| v.kind.kicad_type() != "solder_mask_bridge"),
        "{violations:?}"
    );
}

#[test]
fn exposed_pad_detects_foreign_track_with_legal_copper_clearance() {
    let oracle: serde_json::Value =
        serde_json::from_str(include_str!("data/solder-mask-pad-track/drc.json")).unwrap();
    let mask_errors: Vec<_> = oracle["violations"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|v| v["type"] == "solder_mask_bridge")
        .collect();
    assert_eq!(mask_errors.len(), 1);
    assert!(
        oracle["violations"]
            .as_array()
            .unwrap()
            .iter()
            .all(|v| v["type"] != "clearance")
    );
    let mut fixture = SyntheticBoard::new(
        &[PadSpec {
            name: "1",
            half: 5000,
            offset: IntVector::new(0, 0),
            through_hole: true,
        }],
        2,
        1800,
    );
    let pin = fixture.pin(0, 1);
    let Item::Pin(pad) = fixture.board.items.get_mut(&pin).unwrap() else {
        unreachable!()
    };
    pad.solder_mask_expansion.insert(0, 2000);
    let foreign = fixture.trace(&[(7900, -3000), (7900, 3000)], 0, 1000, 2);
    fixture.trace(&[(7900, -3000), (7900, 3000)], 1, 1000, 2);
    fixture.trace(&[(-7900, -3000), (-7900, 3000)], 0, 1000, 1);
    fixture.board.rules.drc_constraints = Some(DrcConstraints {
        min_clearance: Some(1800),
        ..Default::default()
    });
    let violations = DesignRulesChecker::new(&mut fixture.board).get_all_violations();
    assert_eq!(violations.len(), 1, "{violations:?}");
    let violation = &violations[0];
    assert_eq!(violation.kind.kicad_type(), "solder_mask_bridge");
    assert_eq!(violation.first_item, pin);
    assert_eq!(violation.second_item, Some(foreign));
    assert_eq!(violation.layer, Some(0));
    assert!(violation.shortfall() > 0.0);
}

#[test]
fn mask_to_copper_rule_finds_tracks_outside_the_aperture() {
    let oracle: serde_json::Value =
        serde_json::from_str(include_str!("data/solder-mask-rule/drc.json")).unwrap();
    assert_eq!(
        oracle["violations"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|v| v["type"] == "solder_mask_bridge")
            .count(),
        1
    );
    let mut fixture = SyntheticBoard::new(
        &[PadSpec {
            name: "1",
            half: 5000,
            offset: IntVector::new(0, 0),
            through_hole: false,
        }],
        2,
        1800,
    );
    let pin = fixture.pin(0, 1);
    let Item::Pin(pad) = fixture.board.items.get_mut(&pin).unwrap() else {
        unreachable!()
    };
    pad.solder_mask_expansion.insert(0, 2000);
    let track = fixture.trace(&[(8300, -3000), (8300, 3000)], 0, 1000, 2);
    fixture.board.communication.unit = Unit::Um;
    fixture.board.communication.resolution = 10;
    copper_drc::apply_kicad_project(
        include_str!("data/solder-mask-rule/mask.kicad_pro"),
        &mut fixture.board,
        &copper_dsn::CoordinateTransform::new(10.0, 0.0, 0.0).unwrap(),
    )
    .unwrap();
    assert_eq!(
        fixture
            .board
            .rules
            .drc_constraints
            .as_ref()
            .unwrap()
            .solder_mask_to_copper_clearance,
        Some(500)
    );
    let violations = DesignRulesChecker::new(&mut fixture.board).get_all_violations();
    assert_eq!(violations.len(), 1, "{violations:?}");
    assert_eq!(violations[0].kind.kicad_type(), "solder_mask_bridge");
    assert_eq!(violations[0].second_item, Some(track));
    assert!((violations[0].shortfall() - 200.0).abs() <= 1.0);
}

#[test]
fn explicit_footprint_permission_suppresses_only_mask_reports() {
    let oracle: serde_json::Value =
        serde_json::from_str(include_str!("data/solder-mask-allowed/drc.json")).unwrap();
    assert!(
        oracle["violations"]
            .as_array()
            .unwrap()
            .iter()
            .all(|v| v["type"] != "solder_mask_bridge")
    );
    for allowed in [false, true] {
        let input = serde_json::json!({
            "layers": [{"name":"F.Cu"},{"name":"B.Cu"}],
            "nets": [{"name":"N1"},{"name":"N2"}],
            "components": [{"reference":"U1", "position":{"x":10,"y":10},
                "pads":[{"name":"1","shape":"rect","size":{"x":1,"y":1},
                    "netName":"N1","layers":["F.Cu"],
                    "solderMaskExpansion":{"F.Mask":0.2}, "allowSolderMaskBridges":allowed}]}],
            "traces": [{"netName":"N2","width":0.2,"layerIndex":0,
                "points":[{"x":10.79,"y":9.7},{"x":10.79,"y":10.3}]}]
        });
        let copper_dsn::error::BoardReadResult::Success {
            board: Some(mut board),
            ..
        } = copper_dsn::kicad::read_board(&input.to_string(), None)
        else {
            panic!("fixture must import");
        };
        let count = DesignRulesChecker::new(&mut board)
            .get_all_violations()
            .iter()
            .filter(|v| v.kind.kicad_type() == "solder_mask_bridge")
            .count();
        assert_eq!(count, usize::from(!allowed), "permission={allowed}");
        board.rules.drc_constraints = Some(DrcConstraints {
            min_clearance: Some(3000),
            ..Default::default()
        });
        assert!(
            DesignRulesChecker::new(&mut board)
                .get_all_violations()
                .iter()
                .any(|v| v.kind.kicad_type() == "clearance"),
            "permission must not exempt copper clearance"
        );
    }
}
