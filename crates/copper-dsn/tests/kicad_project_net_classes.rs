use copper_dsn::kicad::{KiCadBoardJson, NetClassJson, NetJson, apply_net_classes};

fn board() -> KiCadBoardJson {
    KiCadBoardJson {
        netClasses: Some(vec![NetClassJson {
            viaInPadAllowed: None,
            name: Some("Default".to_string()),
            clearance: 0.2,
            traceWidth: 0.25,
            viaDiameter: 0.6,
            viaDrill: 0.3,
            netNames: Some(Vec::new()),
        }]),
        nets: Some(vec![
            NetJson {
                id: 1,
                name: Some("VCC".to_string()),
                className: None,
                containsPlane: false,
            },
            NetJson {
                id: 2,
                name: Some("USB_D+".to_string()),
                className: None,
                containsPlane: false,
            },
        ]),
        ..Default::default()
    }
}

fn net_class<'a>(board: &'a KiCadBoardJson, name: &str) -> &'a NetClassJson {
    board
        .netClasses
        .as_ref()
        .unwrap()
        .iter()
        .find(|class| class.name.as_deref() == Some(name))
        .unwrap_or_else(|| panic!("no net class named {name}"))
}

fn net<'a>(board: &'a KiCadBoardJson, name: &str) -> &'a NetJson {
    board
        .nets
        .as_ref()
        .unwrap()
        .iter()
        .find(|net| net.name.as_deref() == Some(name))
        .unwrap_or_else(|| panic!("no net named {name}"))
}

#[test]
fn project_class_overrides_board_defaults() {
    let mut b = board();
    let project = serde_json::json!({
        "board": {"design_settings": {"rules": {}}},
        "net_settings": {
            "classes": [
                {"name": "Default", "clearance": 0.2, "track_width": 0.2, "via_diameter": 0.6, "via_drill": 0.3},
            ],
        },
    })
    .to_string();
    apply_net_classes(&mut b, &project).expect("applies");
    assert_eq!(net_class(&b, "Default").traceWidth, 0.2);
}

#[test]
fn per_field_fallback_uses_board_default_when_project_omits_a_field() {
    let mut b = board();
    let project = serde_json::json!({
        "board": {"design_settings": {"rules": {}}},
        "net_settings": {
            "classes": [
                {"name": "Default", "clearance": 0.3},
            ],
        },
    })
    .to_string();
    apply_net_classes(&mut b, &project).expect("applies");
    let class = net_class(&b, "Default");
    assert_eq!(class.clearance, 0.3);
    assert_eq!(class.traceWidth, 0.25);
    assert_eq!(class.viaDiameter, 0.6);
    assert_eq!(class.viaDrill, 0.3);
}

#[test]
fn clearance_is_floored_at_the_board_minimum() {
    let mut b = board();
    let project = serde_json::json!({
        "board": {"design_settings": {"rules": {"min_clearance": 0.5}}},
        "net_settings": {"classes": [{"name": "Default", "clearance": 0.2}]},
    })
    .to_string();
    apply_net_classes(&mut b, &project).expect("applies");
    assert_eq!(net_class(&b, "Default").clearance, 0.5);
}

#[test]
fn track_width_is_floored_at_the_board_minimum() {
    let mut b = board();
    let project = serde_json::json!({
        "board": {"design_settings": {"rules": {"min_track_width": 0.4}}},
        "net_settings": {"classes": [{"name": "Default", "track_width": 0.2}]},
    })
    .to_string();
    apply_net_classes(&mut b, &project).expect("applies");
    assert_eq!(net_class(&b, "Default").traceWidth, 0.4);
}

#[test]
fn via_diameter_is_floored_at_the_board_minimum() {
    let mut b = board();
    let project = serde_json::json!({
        "board": {"design_settings": {"rules": {"min_via_diameter": 0.8}}},
        "net_settings": {"classes": [{"name": "Default", "via_diameter": 0.6}]},
    })
    .to_string();
    apply_net_classes(&mut b, &project).expect("applies");
    assert_eq!(net_class(&b, "Default").viaDiameter, 0.8);
}

#[test]
fn via_drill_is_floored_at_the_board_minimum_through_hole_diameter() {
    let mut b = board();
    let project = serde_json::json!({
        "board": {"design_settings": {"rules": {"min_through_hole_diameter": 0.35}}},
        "net_settings": {"classes": [{"name": "Default", "via_drill": 0.25, "via_diameter": 0.6}]},
    })
    .to_string();
    apply_net_classes(&mut b, &project).expect("applies");
    assert_eq!(net_class(&b, "Default").viaDrill, 0.35);
}

#[test]
fn a_project_with_no_default_class_gets_one_synthesised_from_the_board() {
    let mut b = board();
    let project = serde_json::json!({
        "board": {"design_settings": {"rules": {}}},
        "net_settings": {
            "classes": [
                {"name": "Power", "clearance": 0.4, "track_width": 0.8, "via_diameter": 0.9, "via_drill": 0.4},
            ],
        },
    })
    .to_string();
    apply_net_classes(&mut b, &project).expect("applies");
    let classes = b.netClasses.as_ref().unwrap();
    assert_eq!(classes[0].name.as_deref(), Some("Default"));
    assert_eq!(classes[0].traceWidth, 0.25);
    assert!(classes.iter().any(|c| c.name.as_deref() == Some("Power")));
}

#[test]
fn nonfinite_or_nonpositive_dimensions_are_rejected_with_the_reference_message() {
    let mut b = board();
    let project = serde_json::json!({
        "board": {"design_settings": {"rules": {}}},
        "net_settings": {"classes": [{"name": "Default", "clearance": 0.0}]},
    })
    .to_string();
    let error = apply_net_classes(&mut b, &project).unwrap_err();
    assert_eq!(
        error.to_string(),
        "Invalid dimensions for project net class Default"
    );
}

#[test]
fn via_drill_not_smaller_than_diameter_is_rejected() {
    let mut b = board();
    let project = serde_json::json!({
        "board": {"design_settings": {"rules": {}}},
        "net_settings": {"classes": [{"name": "Default", "via_diameter": 0.3, "via_drill": 0.3}]},
    })
    .to_string();
    let error = apply_net_classes(&mut b, &project).unwrap_err();
    assert_eq!(
        error.to_string(),
        "Invalid dimensions for project net class Default"
    );
}

#[test]
fn missing_design_settings_is_rejected_with_the_reference_message() {
    let mut b = board();
    let error = apply_net_classes(&mut b, "{}").unwrap_err();
    assert_eq!(error.to_string(), "Project has no board.design_settings.");
}

#[test]
fn composite_assignments_are_rejected_with_the_reference_message() {
    let mut b = board();
    let project = serde_json::json!({
        "board": {"design_settings": {"rules": {}}},
        "net_settings": {
            "classes": [
                {"name": "Default", "clearance": 0.2, "track_width": 0.2, "via_diameter": 0.6, "via_drill": 0.3},
                {"name": "Power", "clearance": 0.4, "track_width": 0.8, "via_diameter": 0.9, "via_drill": 0.4},
            ],
            "netclass_assignments": {"VCC": ["Default", "Power"]},
        },
    })
    .to_string();
    let error = apply_net_classes(&mut b, &project).unwrap_err();
    assert_eq!(
        error.to_string(),
        "Composite net classes for VCC are not supported yet."
    );
}

#[test]
fn an_assignment_naming_an_undeclared_class_is_rejected() {
    let mut b = board();
    let project = serde_json::json!({
        "board": {"design_settings": {"rules": {}}},
        "net_settings": {
            "classes": [{"name": "Default", "clearance": 0.2, "track_width": 0.2, "via_diameter": 0.6, "via_drill": 0.3}],
            "netclass_assignments": {"VCC": ["Ghost"]},
        },
    })
    .to_string();
    let error = apply_net_classes(&mut b, &project).unwrap_err();
    assert_eq!(error.to_string(), "Unknown project net class: Ghost");
}

#[test]
fn richer_pattern_syntax_is_rejected_with_the_reference_message() {
    let mut b = board();
    let project = serde_json::json!({
        "board": {"design_settings": {"rules": {}}},
        "net_settings": {
            "classes": [{"name": "Default", "clearance": 0.2, "track_width": 0.2, "via_diameter": 0.6, "via_drill": 0.3}],
            "netclass_patterns": [{"pattern": "D[0-9]", "netclass": "Default"}],
        },
    })
    .to_string();
    let error = apply_net_classes(&mut b, &project).unwrap_err();
    assert_eq!(error.to_string(), "Unsupported project net pattern: D[0-9]");
}

#[test]
fn nets_are_assigned_by_explicit_name() {
    let mut b = board();
    let project = serde_json::json!({
        "board": {"design_settings": {"rules": {}}},
        "net_settings": {
            "classes": [
                {"name": "Default", "clearance": 0.2, "track_width": 0.2, "via_diameter": 0.6, "via_drill": 0.3},
                {"name": "Power", "clearance": 0.4, "track_width": 0.8, "via_diameter": 0.9, "via_drill": 0.4},
            ],
            "netclass_assignments": {"VCC": ["Power"]},
        },
    })
    .to_string();
    apply_net_classes(&mut b, &project).expect("applies");
    assert_eq!(net(&b, "VCC").className.as_deref(), Some("Power"));
    assert_eq!(net(&b, "USB_D+").className.as_deref(), Some("Default"));
    assert_eq!(
        net_class(&b, "Power").netNames.as_deref(),
        Some(["VCC".to_string()].as_slice())
    );
}

#[test]
fn nets_are_assigned_by_wildcard_pattern() {
    let mut b = board();
    let project = serde_json::json!({
        "board": {"design_settings": {"rules": {}}},
        "net_settings": {
            "classes": [
                {"name": "Default", "clearance": 0.2, "track_width": 0.2, "via_diameter": 0.6, "via_drill": 0.3},
                {"name": "USB", "clearance": 0.2, "track_width": 0.2, "via_diameter": 0.6, "via_drill": 0.3},
            ],
            "netclass_patterns": [{"pattern": "USB_*", "netclass": "USB"}],
        },
    })
    .to_string();
    apply_net_classes(&mut b, &project).expect("applies");
    assert_eq!(net(&b, "USB_D+").className.as_deref(), Some("USB"));
    assert_eq!(net(&b, "VCC").className.as_deref(), Some("Default"));
}

#[test]
fn an_empty_project_document_is_a_no_op() {
    let mut b = board();
    let before = b.clone();
    apply_net_classes(&mut b, "").expect("applies");
    assert_eq!(b, before);
}
