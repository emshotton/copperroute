use copper_drc::DesignRulesChecker;

#[test]
fn trace_clearance_uses_rounded_pad_boundary() {
    for (name, input, oracle, expected) in [
        (
            "clear",
            include_str!("data/rounded-trace-clearance/clear-native.json"),
            include_str!("data/rounded-trace-clearance/clear-kicad.json"),
            0,
        ),
        (
            "violating",
            include_str!("data/rounded-trace-clearance/violating-native.json"),
            include_str!("data/rounded-trace-clearance/violating-kicad.json"),
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

#[test]
fn rounded_pad_track_checks_keep_endcaps_sides_and_shorts() {
    for (points, expected) in [
        ([[11.2, 11.2], [12.0, 12.0]], 0),
        ([[11.17, 11.17], [12.0, 12.0]], 1),
        ([[11.46, 9.0], [11.46, 11.0]], 0),
        ([[11.44, 9.0], [11.44, 11.0]], 1),
        ([[8.0, 10.0], [12.0, 10.0]], 1),
    ] {
        for angle in [0.0_f64, 45.0, 90.0] {
            for same_net in [false, true] {
                let radians = angle.to_radians();
                let points: Vec<_> = points
                    .iter()
                    .map(|p| {
                        let (x, y) = (p[0] - 10.0, p[1] - 10.0);
                        serde_json::json!({
                            "x":10.0+x*radians.cos()-y*radians.sin(),
                            "y":10.0+x*radians.sin()+y*radians.cos()
                        })
                    })
                    .collect();
                let input = serde_json::json!({
                    "layers":[{"name":"F.Cu"},{"name":"B.Cu"}],
                    "nets":[{"name":"A"},{"name":"B"}],
                    "components":[{"reference":"P1","rotation":angle,
                        "position":{"x":10,"y":10},"pads":[
                            {"name":"1","netName":"A","shape":"roundrect",
                             "roundRectRatio":0.25,"size":{"x":2,"y":2},
                             "drill":0,"layers":["F.Cu"]}
                        ]}],
                    "traces":[{"netName":if same_net {"A"} else {"B"},
                        "width":0.5,"layerIndex":0,"points":points}]
                });
                let copper_dsn::error::BoardReadResult::Success {
                    board: Some(mut board),
                    ..
                } = copper_dsn::kicad::read_board(&input.to_string(), None)
                else {
                    panic!("fixture must import")
                };
                let violations = DesignRulesChecker::new(&mut board).get_all_violations();
                assert_eq!(
                    violations
                        .iter()
                        .filter(|v| matches!(v.kind.kicad_type(), "clearance" | "shorting_items"))
                        .count(),
                    if same_net { 0 } else { expected },
                    "angle={angle}, same_net={same_net}, points={points:?}: {violations:?}"
                );
            }
        }
    }
}
