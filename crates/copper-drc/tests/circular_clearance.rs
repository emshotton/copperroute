use copper_drc::DesignRulesChecker;

#[test]
fn circular_via_clearance_uses_rounded_pad_boundary() {
    for (name, input, oracle, expected) in [
        (
            "clear",
            include_str!("data/circular-clearance/clear-native.json"),
            include_str!("data/circular-clearance/clear-kicad.json"),
            0,
        ),
        (
            "violating",
            include_str!("data/circular-clearance/violating-native.json"),
            include_str!("data/circular-clearance/violating-kicad.json"),
            1,
        ),
    ] {
        let oracle: serde_json::Value = serde_json::from_str(oracle).unwrap();
        assert_eq!(
            oracle["violations"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|v| v["type"] == "clearance")
                .count(),
            expected,
            "{name}: KiCad oracle"
        );
        let copper_dsn::error::BoardReadResult::Success {
            board: Some(mut board),
            ..
        } = copper_dsn::kicad::read_board(input, None)
        else {
            panic!("fixture must import");
        };
        let violations = DesignRulesChecker::new(&mut board).get_all_violations();
        assert_eq!(
            violations
                .iter()
                .filter(|v| v.kind.kicad_type() == "clearance")
                .count(),
            expected,
            "{name}: {violations:?}"
        );
    }
}

fn copper_count(input: serde_json::Value) -> usize {
    let copper_dsn::error::BoardReadResult::Success {
        board: Some(mut board),
        ..
    } = copper_dsn::kicad::read_board(&input.to_string(), None)
    else {
        panic!("fixture must import");
    };
    DesignRulesChecker::new(&mut board)
        .get_all_violations()
        .iter()
        .filter(|v| matches!(v.kind.kicad_type(), "clearance" | "shorting_items"))
        .count()
}

#[test]
fn circular_pads_keep_true_clearance_and_short_checks() {
    for (offset, expected) in [(0.86, 0), (0.84, 1), (0.1, 1)] {
        for same_net in [false, true] {
            let input = serde_json::json!({
                "layers":[{"name":"F.Cu"},{"name":"B.Cu"}],
                "nets":[{"name":"A"},{"name":"B"}],
                "components":[
                    {"reference":"P1","position":{"x":10,"y":10},"pads":[
                        {"name":"1","netName":"A","shape":"circle",
                         "size":{"x":1,"y":1},"drill":0,"layers":["F.Cu"]}
                    ]},
                    {"reference":"P2","position":{"x":10.0+offset,"y":10.0+offset},"pads":[
                        {"name":"1","netName":if same_net { "A" } else { "B" },
                         "shape":"circle","size":{"x":1,"y":1},"drill":0,"layers":["F.Cu"]}
                    ]}
                ]
            });
            assert_eq!(
                copper_count(input),
                if same_net { 0 } else { expected },
                "offset={offset}, same_net={same_net}"
            );
        }
    }
}

#[test]
fn circular_pad_clearance_uses_trace_endcap() {
    for (offset, expected) in [(0.5, 0), (0.49, 1), (0.1, 1)] {
        let input = serde_json::json!({
            "layers":[{"name":"F.Cu"},{"name":"B.Cu"}],
            "nets":[{"name":"A"},{"name":"B"}],
            "components":[
                {"reference":"P1","position":{"x":11.0+offset,"y":10.0+offset},"pads":[
                    {"name":"1","netName":"A","shape":"circle",
                     "size":{"x":0.5,"y":0.5},"drill":0,"layers":["F.Cu"]}
                ]}
            ],
            "traces":[{"netName":"B","width":0.5,"layerIndex":0,
                "points":[{"x":10,"y":10},{"x":11,"y":10}]}]
        });
        assert_eq!(copper_count(input), expected, "offset={offset}");
    }
}
