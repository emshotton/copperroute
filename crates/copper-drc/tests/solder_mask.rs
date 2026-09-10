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
    let board_allowed: serde_json::Value = serde_json::from_str(include_str!(
        "data/solder-mask-board-allowed-track/drc.json"
    ))
    .unwrap();
    assert_eq!(
        board_allowed["violations"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|v| v["type"] == "solder_mask_bridge")
            .count(),
        1
    );
    for allowed in [false, true] {
        let input = serde_json::json!({
            "layers": [{"name":"F.Cu"},{"name":"B.Cu"}],
            "nets": [{"name":"N1"},{"name":"N2"}],
            "allowSolderMaskBridgesInFootprints":true,
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

#[test]
fn an_unnetted_pad_aperture_checks_both_foreign_front_tracks() {
    let oracle: serde_json::Value =
        serde_json::from_str(include_str!("data/solder-mask-unnetted/drc.json")).unwrap();
    assert_eq!(
        oracle["violations"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|v| v["type"] == "solder_mask_bridge")
            .count(),
        2
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
    pad.hdr.net_nos.clear();
    pad.solder_mask_expansion.insert(0, 2000);
    fixture.trace(&[(7900, -3000), (7900, 3000)], 0, 1000, 2);
    fixture.trace(&[(-7900, -3000), (-7900, 3000)], 0, 1000, 1);
    fixture.trace(&[(7900, -3000), (7900, 3000)], 1, 1000, 2);
    fixture.board.rules.drc_constraints = Some(DrcConstraints {
        min_clearance: Some(1800),
        ..Default::default()
    });
    let violations = DesignRulesChecker::new(&mut fixture.board).get_all_violations();
    let mask: Vec<_> = violations
        .iter()
        .filter(|v| v.kind.kicad_type() == "solder_mask_bridge")
        .collect();
    assert_eq!(mask.len(), 2, "{violations:?}");
    assert!(
        mask.iter()
            .all(|v| v.first_item == pin && v.layer == Some(0))
    );
}

#[test]
fn a_pad_mask_checks_a_track_without_a_net() {
    check_unnetted_track(
        false,
        include_str!("data/solder-mask-unnetted-track/drc.json"),
    );
}

#[test]
fn two_unnetted_copper_items_are_not_exempt_from_mask_checks() {
    check_unnetted_track(
        true,
        include_str!("data/solder-mask-both-unnetted/drc.json"),
    );
}

fn check_unnetted_track(unnetted_pad: bool, reference: &str) {
    let oracle: serde_json::Value = serde_json::from_str(reference).unwrap();
    let expected = oracle["violations"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|v| v["type"] == "solder_mask_bridge")
        .count();
    assert_eq!(expected, if unnetted_pad { 2 } else { 1 });
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
    if unnetted_pad {
        pad.hdr.net_nos.clear();
    }
    pad.solder_mask_expansion.insert(0, 2000);
    fixture.trace(&[(-7900, -3000), (-7900, 3000)], 0, 1000, 1);
    for layer in [0, 1] {
        let track = fixture.trace(&[(7900, -3000), (7900, 3000)], layer, 1000, 2);
        let Item::Trace(trace) = fixture.board.items.get_mut(&track).unwrap() else {
            unreachable!()
        };
        trace.hdr.net_nos.clear();
    }
    fixture.board.rules.drc_constraints = Some(DrcConstraints {
        min_clearance: Some(1800),
        ..Default::default()
    });
    let violations = DesignRulesChecker::new(&mut fixture.board).get_all_violations();
    assert_eq!(
        violations
            .iter()
            .filter(|v| v.kind.kicad_type() == "solder_mask_bridge")
            .count(),
        expected,
        "{violations:?}"
    );
}

#[test]
fn expanded_pad_apertures_are_checked_against_each_other() {
    let oracle: serde_json::Value =
        serde_json::from_str(include_str!("data/solder-mask-pad-pad/drc.json")).unwrap();
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
        &[
            PadSpec {
                name: "1",
                half: 5000,
                offset: IntVector::new(0, 0),
                through_hole: true,
            },
            PadSpec {
                name: "2",
                half: 5000,
                offset: IntVector::new(13000, 0),
                through_hole: true,
            },
        ],
        2,
        1800,
    );
    let a = fixture.pin(0, 1);
    let b = fixture.pin(1, 2);
    for id in [a, b] {
        let Item::Pin(pad) = fixture.board.items.get_mut(&id).unwrap() else {
            unreachable!()
        };
        pad.solder_mask_expansion.insert(0, 2000);
    }
    fixture.board.rules.drc_constraints = Some(DrcConstraints {
        min_clearance: Some(1800),
        ..Default::default()
    });
    let violations = DesignRulesChecker::new(&mut fixture.board).get_all_violations();
    assert_eq!(violations.len(), 1, "{violations:?}");
    assert_eq!(violations[0].kind.kicad_type(), "solder_mask_bridge");
    assert_eq!(violations[0].layer, Some(0));
    assert_eq!(violations[0].expected, 4000.0);
    assert_eq!(violations[0].actual, 3000.0);
}

#[test]
fn separate_apertures_enforce_the_board_minimum_mask_web() {
    let oracle: serde_json::Value =
        serde_json::from_str(include_str!("data/solder-mask-web-width/drc.json")).unwrap();
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
        &[
            PadSpec {
                name: "1",
                half: 5000,
                offset: IntVector::new(0, 0),
                through_hole: true,
            },
            PadSpec {
                name: "2",
                half: 5000,
                offset: IntVector::new(15000, 0),
                through_hole: true,
            },
        ],
        2,
        1800,
    );
    let a = fixture.pin(0, 1);
    let b = fixture.pin(1, 2);
    for id in [a, b] {
        let Item::Pin(pad) = fixture.board.items.get_mut(&id).unwrap() else {
            unreachable!()
        };
        pad.solder_mask_expansion.insert(0, 2000);
    }
    fixture.board.rules.drc_constraints = Some(DrcConstraints {
        min_clearance: Some(1800),
        ..Default::default()
    });
    fixture
        .board
        .rules
        .drc_constraints
        .as_mut()
        .unwrap()
        .solder_mask_min_width = Some(2000);
    let violations = DesignRulesChecker::new(&mut fixture.board).get_all_violations();
    assert_eq!(violations.len(), 1, "{violations:?}");
    assert_eq!(violations[0].kind.kicad_type(), "solder_mask_bridge");
    assert_eq!(violations[0].layer, Some(0));
    assert_eq!(violations[0].expected, 6000.0);
    assert_eq!(violations[0].actual, 5000.0);
}

#[test]
fn board_mask_web_setting_survives_json_import_and_export() {
    let input = r#"{"unit":"MM","resolution":10000,"layers":[{"name":"F.Cu"},{"name":"B.Cu"}],"solderMaskMinWidth":0.2}"#;
    let copper_dsn::BoardReadResult::Success { board, .. } =
        copper_dsn::kicad::read_board(input, None)
    else {
        panic!("invalid input")
    };
    let board = board.unwrap();
    assert_eq!(
        copper_drc::constraints::resolve(&board).solder_mask_min_width,
        Some(2000)
    );
    let output: serde_json::Value =
        serde_json::from_str(&copper_dsn::kicad::write(&board, "mask-web")).unwrap();
    assert_eq!(output["solderMaskMinWidth"], 0.2);
}

#[test]
fn pad_pair_controls_preserve_kicad_exceptions() {
    for (same_net, dy, allowed, reference) in [
        (
            true,
            0.0,
            false,
            include_str!("data/solder-mask-pad-pair-same-net/drc.json"),
        ),
        (
            false,
            1.3,
            false,
            include_str!("data/solder-mask-pad-pair-corner/drc.json"),
        ),
        (
            false,
            0.0,
            true,
            include_str!("data/solder-mask-pad-pair-board-allowed/drc.json"),
        ),
    ] {
        let oracle: serde_json::Value = serde_json::from_str(reference).unwrap();
        assert_eq!(
            oracle["violations"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|v| v["type"] == "solder_mask_bridge")
                .count(),
            0
        );
        let input = serde_json::json!({
            "layers":[{"name":"F.Cu"},{"name":"B.Cu"}],
            "nets":[{"name":"N1"},{"name":"N2"}],
            "allowSolderMaskBridgesInFootprints":allowed,
            "components":[{"reference":"U1","position":{"x":10,"y":10},"pads":[
                {"name":"1","shape":"rect","size":{"x":1,"y":1},"drill":0.3,"netName":"N1","layers":["F.Cu","B.Cu"],"solderMaskExpansion":{"F.Mask":0.2}},
                {"name":"2","offset":{"x":1.3,"y":dy},"shape":"rect","size":{"x":1,"y":1},"drill":0.3,"netName":if same_net {"N1"}else{"N2"},"layers":["F.Cu","B.Cu"],"solderMaskExpansion":{"F.Mask":0.2}}
            ]}]
        });
        let copper_dsn::BoardReadResult::Success {
            board: Some(mut board),
            ..
        } = copper_dsn::kicad::read_board(&input.to_string(), None)
        else {
            panic!("invalid input")
        };
        let violations = DesignRulesChecker::new(&mut board).get_all_violations();
        assert_eq!(
            violations
                .iter()
                .filter(|v| v.kind.kicad_type() == "solder_mask_bridge")
                .count(),
            0,
            "same_net={same_net} dy={dy} allowed={allowed}: {violations:?}"
        );
    }
}

#[test]
fn non_plated_holes_only_exempt_pads_without_remaining_copper() {
    let oracle: serde_json::Value =
        serde_json::from_str(include_str!("data/solder-mask-hole-only/drc.json")).unwrap();
    assert!(
        oracle["violations"]
            .as_array()
            .unwrap()
            .iter()
            .all(|v| v["type"] != "solder_mask_bridge")
    );
    let corners: serde_json::Value = serde_json::from_str(include_str!(
        "data/solder-mask-npth-copper-corners/drc.json"
    ))
    .unwrap();
    assert_eq!(
        corners["violations"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|v| v["type"] == "solder_mask_bridge")
            .count(),
        2
    );
    for (shape, expected) in [("circle", 0), ("rect", 2)] {
        let input = serde_json::json!({
            "layers": [{"name":"F.Cu"},{"name":"B.Cu"}],
            "nets": [{"name":"N1"},{"name":"N2"}],
            "components": [{"reference":"U1", "position":{"x":10,"y":10},
                "pads":[{"name":"1","shape":shape,"size":{"x":1,"y":1},
                    "drill":1,"nonPlated":true,"layers":["F.Cu","B.Cu"],
                    "solderMaskExpansion":{"F.Mask":0.2}}]}],
            "traces": [
                {"netName":"N1","width":0.2,"layerIndex":0,
                    "points":[{"x":9.21,"y":9.7},{"x":9.21,"y":10.3}]},
                {"netName":"N2","width":0.2,"layerIndex":0,
                    "points":[{"x":10.79,"y":9.7},{"x":10.79,"y":10.3}]},
                {"netName":"N2","width":0.2,"layerIndex":1,
                    "points":[{"x":10.79,"y":9.7},{"x":10.79,"y":10.3}]}]
        });
        let copper_dsn::error::BoardReadResult::Success {
            board: Some(mut board),
            ..
        } = copper_dsn::kicad::read_board(&input.to_string(), None)
        else {
            panic!("fixture must import");
        };
        let violations = DesignRulesChecker::new(&mut board).get_all_violations();
        let mask_count = violations
            .iter()
            .filter(|v| v.kind.kicad_type() == "solder_mask_bridge")
            .count();
        assert_eq!(mask_count, expected, "shape={shape}: {violations:?}");
    }
}

#[test]
fn flattened_pads_retain_their_logical_identity_for_mask_checks() {
    for (oracle, expected) in [
        (
            include_str!("data/solder-mask-same-logical-pad/drc.json"),
            0,
        ),
        (include_str!("data/solder-mask-unnumbered-pads/drc.json"), 1),
    ] {
        let oracle: serde_json::Value = serde_json::from_str(oracle).unwrap();
        assert_eq!(
            oracle["violations"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|v| v["type"] == "solder_mask_bridge")
                .count(),
            expected
        );
    }
    for (footprint, number, allow, expected) in [
        ("U1", "1", false, 0),
        ("U1", "", false, 1),
        ("U1", "2", false, 1),
        ("U2", "1", false, 1),
        ("U1", "2", true, 0),
        ("U2", "1", true, 1),
    ] {
        let components: Vec<_> = [
            ("U1", if number.is_empty() { "" } else { "1" }, 10.0),
            (footprint, number, 11.3),
        ]
        .into_iter()
        .enumerate()
        .map(|(i, (source, number, x))| {
            serde_json::json!({
                "reference":format!("flattened-{i}"),"position":{"x":x,"y":10},
                "pads":[{"name":i.to_string(), "sourceFootprint":source, "sourcePadNumber":number,
                    "shape":"rect","size":{"x":1,"y":1},"drill":0.3,
                    "layers":["F.Cu","B.Cu"],"solderMaskExpansion":{"F.Mask":0.2}}]
            })
        })
        .collect();
        let input = serde_json::json!({"layers":[{"name":"F.Cu"},{"name":"B.Cu"}],
            "nets":[],"components":components,"allowSolderMaskBridgesInFootprints":allow});
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
        assert_eq!(
            count, expected,
            "footprint={footprint}, number={number}, allow={allow}"
        );
    }
}
