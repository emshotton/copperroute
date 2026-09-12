use copper_board::{Board, PadstackId};
use copper_dsn::error::BoardReadResult;
use copper_dsn::kicad::read_board;
use copper_dsn::ses_writer;

fn board(json: &str) -> Board {
    match read_board(json, None) {
        BoardReadResult::Success { board: Some(b), .. } => *b,
        other => panic!("expected a loaded board, got {other:?}"),
    }
}

fn board_and_ct(json: &str) -> (Board, copper_dsn::CoordinateTransform) {
    match read_board(json, None) {
        BoardReadResult::Success {
            board: Some(b),
            coordinate_transform: Some(ct),
            ..
        } => (*b, ct),
        other => panic!("expected a loaded board with a coordinate transform, got {other:?}"),
    }
}

fn parse_error(json: &str) -> (String, String) {
    match read_board(json, None) {
        BoardReadResult::ParseError { location, detail } => (location, detail),
        other => panic!("expected a ParseError, got {other:?}"),
    }
}

fn first_pin_padstack(board: &Board, index: usize) -> PadstackId {
    let component_package = board
        .components
        .get(i32::try_from(index).expect("a small index") + 1)
        .get_package();
    board
        .library
        .packages
        .get(component_package)
        .get_pin(0)
        .expect("the package has a pin")
        .padstack_no
}

fn two_pads_on(first: &str, second: &str, drill_a: f64, drill_b: f64) -> String {
    format!(
        r#"{{
        "layers":[{{"name":"F.Cu"}},{{"name":"In1.Cu"}},{{"name":"In2.Cu"}},{{"name":"B.Cu"}}],
        "components":[
          {{"reference":"U1","footprint":"FA","position":{{"x":0,"y":0}},
            "pads":[{{"name":"1","shape":"rect","size":{{"x":1.0,"y":1.0}},
                     "drill":{drill_a},"layers":[{first}]}}]}},
          {{"reference":"U2","footprint":"FB","position":{{"x":10,"y":0}},
            "pads":[{{"name":"1","shape":"rect","size":{{"x":1.0,"y":1.0}},
                     "drill":{drill_b},"layers":[{second}]}}]}}
        ]}}"#
    )
}

#[test]
fn two_pads_of_the_same_name_and_different_layers_get_different_padstacks() {
    let board = board(&two_pads_on(
        r#""F.Cu","In1.Cu""#,
        r#""In2.Cu","B.Cu""#,
        0.0,
        0.0,
    ));
    let first = first_pin_padstack(&board, 0);
    let second = first_pin_padstack(&board, 1);
    assert_ne!(
        first, second,
        "the two pads span different layers and must not share a padstack"
    );

    let first = board.library.padstacks.get(first).expect("padstack 1");
    let second = board.library.padstacks.get(second).expect("padstack 2");
    assert_eq!(
        (first.from_layer(), first.to_layer()),
        (0, 1),
        "the F.Cu/In1.Cu pad spans layers 0..1"
    );
    assert_eq!(
        (second.from_layer(), second.to_layer()),
        (2, 3),
        "the In2.Cu/B.Cu pad spans layers 2..3, not the first pad's 0..1"
    );
}

#[test]
fn two_pads_of_the_same_shapes_and_drill_share_one_padstack() {
    let board = board(&two_pads_on(
        r#""F.Cu","In1.Cu""#,
        r#""F.Cu","In1.Cu""#,
        0.0,
        0.0,
    ));
    assert_eq!(
        first_pin_padstack(&board, 0),
        first_pin_padstack(&board, 1),
        "identical pads still share a padstack — the fix narrows the key, it does not abolish it"
    );
}

#[test]
fn mask_expansion_stays_per_pad_and_uses_absolute_outer_layers() {
    let mut input: serde_json::Value = serde_json::from_str(&two_pads_on(
        r#""F.Cu","B.Cu""#,
        r#""F.Cu","B.Cu""#,
        0.3,
        0.3,
    ))
    .unwrap();
    input["components"][0]["pads"][0]["solderMaskExpansion"] = serde_json::json!({"F.Mask": 0.2});
    input["components"][1]["pads"][0]["solderMaskExpansion"] = serde_json::json!({"B.Mask": -0.05});
    let board = board(&input.to_string());
    assert_eq!(first_pin_padstack(&board, 0), first_pin_padstack(&board, 1));
    for pin in board.get_pins() {
        let copper_board::Item::Pin(pin) = board.get_item(pin).unwrap() else {
            unreachable!()
        };
        let expected = if pin.hdr.get_component_id() == 1 {
            [(0, 2000)].into_iter().collect()
        } else {
            [(3, -500)].into_iter().collect()
        };
        assert_eq!(pin.solder_mask_expansion, expected);
    }
}

#[test]
fn a_drilled_and_an_undrilled_pad_do_not_share_attach_allowed() {
    let board = board(&two_pads_on(
        r#""F.Cu","In1.Cu""#,
        r#""F.Cu","In1.Cu""#,
        0.5,
        0.0,
    ));
    let drilled = board
        .library
        .padstacks
        .get(first_pin_padstack(&board, 0))
        .expect("padstack 1");
    let undrilled = board
        .library
        .padstacks
        .get(first_pin_padstack(&board, 1))
        .expect("padstack 2");
    assert!(drilled.attach_allowed, "the drilled pad allows attachment");
    assert!(
        !undrilled.attach_allowed,
        "the undrilled pad must not inherit the drilled one's attachAllowed"
    );
}

#[test]
fn a_round_pad_name_carries_size_y() {
    let board = board(
        r#"{"layers":[{"name":"F.Cu"},{"name":"B.Cu"}],
            "components":[{"reference":"U1","footprint":"F","position":{"x":0,"y":0},
              "pads":[{"name":"1","shape":"circle","size":{"x":1.0,"y":2.0},"layers":["F.Cu"]}]}]}"#,
    );
    let padstack = board
        .library
        .padstacks
        .get(first_pin_padstack(&board, 0))
        .expect("the pad's padstack");
    assert_eq!(padstack.name, "Round[T]Pad_1000x2000_um");
}

#[test]
fn an_all_null_shape_array_is_refused_naming_the_pad() {
    let (location, detail) = parse_error(
        r#"{"layers":[{"name":"F.Cu"},{"name":"B.Cu"}],
            "components":[{"reference":"U7","footprint":"F","position":{"x":0,"y":0},
              "pads":[{"name":"3","shape":"rect","size":{"x":1.0,"y":1.0},
                       "layers":["Nonexistent.Cu"]}]}]}"#,
    );
    assert_eq!(location, "components");
    assert!(
        detail.contains("KiCad board JSON file"),
        "the diagnostic names the file: {detail}"
    );
    assert!(
        detail.contains("pad `3` of component `U7`"),
        "the diagnostic names the pad: {detail}"
    );
    assert!(
        detail.contains("no shape on any layer"),
        "the diagnostic says what is wrong: {detail}"
    );
    assert!(
        !detail.contains("-2"),
        "the bare NegativeArraySizeException number is gone: {detail}"
    );
}

#[test]
fn a_via_with_an_empty_layer_span_is_refused_naming_the_via() {
    let (location, detail) = parse_error(
        r#"{"layers":[{"name":"F.Cu"},{"name":"B.Cu"}],
            "vias":[{"netName":"GND","position":{"x":1,"y":1},"diameter":0.6,"drill":0.3,
                     "startLayerIndex":1,"endLayerIndex":0}]}"#,
    );
    assert_eq!(location, "vias");
    assert!(
        detail.contains("via `Via[1-0]_600:300_um`") && detail.contains("no shape on any layer"),
        "the diagnostic names the via and what is wrong: {detail}"
    );
}

#[test]
fn equal_copper_with_different_drills_keeps_distinct_exact_padstacks() {
    let b = board(&two_pads_on(
        r#""F.Cu","B.Cu""#,
        r#""F.Cu","B.Cu""#,
        0.3,
        0.65,
    ));
    let a = first_pin_padstack(&b, 0);
    let c = first_pin_padstack(&b, 1);
    assert_ne!(a, c);
    for (id, diameter) in [(a, 3000.0), (c, 6500.0)] {
        let p = b.library.padstacks.get(id).unwrap();
        assert_eq!(p.drill_diameter, Some(diameter));
        assert_eq!(p.drill_radius(), diameter / 2.0);
        assert!(!p.drill_estimated);
    }
}

#[test]
fn via_class_templates_and_existing_vias_preserve_drills_on_export() {
    let b = board(
        r#"{
      "layers":[{"name":"F.Cu"},{"name":"B.Cu"}],
      "netClasses":[{"name":"Default","viaDiameter":0.5,"viaDrill":0.3},
                    {"name":"Power","viaDiameter":0.8,"viaDrill":0.4}],
      "nets":[{"id":1,"name":"GND","className":"Default"}],
      "vias":[{"netName":"GND","position":{"x":10,"y":10},"diameter":0.6,"drill":0.35,"startLayerIndex":0,"endLayerIndex":1}]
    }"#,
    );
    assert_eq!(
        b.library
            .padstacks
            .get_by_name("defaultVia")
            .unwrap()
            .drill_diameter,
        Some(3000.0)
    );
    assert_eq!(
        b.library
            .padstacks
            .get_by_name("via_Power")
            .unwrap()
            .drill_diameter,
        Some(4000.0)
    );
    let output: serde_json::Value =
        serde_json::from_str(&copper_dsn::kicad::write(&b, "test")).unwrap();
    assert_eq!(output["vias"][0]["drill"], 0.35);
    assert_eq!(output["netClasses"][0]["viaDrill"], 0.3);
    assert_eq!(output["netClasses"][1]["viaDrill"], 0.4);
}

#[test]
fn a_native_net_class_via_padstack_is_named_with_its_true_diameter_and_drill_in_the_session() {
    let (board, ct) = board_and_ct(
        r#"{
      "layers":[{"name":"F.Cu"},{"name":"B.Cu"}],
      "netClasses":[{"name":"Default","viaDiameter":0.6,"viaDrill":0.25}],
      "nets":[{"id":1,"name":"GND","className":"Default"}]
    }"#,
    );
    assert_eq!(
        board
            .library
            .padstacks
            .get_by_name("defaultVia")
            .unwrap()
            .drill_diameter,
        Some(2500.0),
        "0.25 mm at this board's 10000-unit-per-mm resolution"
    );

    let mut out: Vec<u8> = Vec::new();
    ses_writer::write(&board, &ct, &mut out, "test.dsn").expect("write into a Vec");
    let ses = String::from_utf8(out).expect("SES is UTF-8");
    assert!(
        ses.contains("Via[0-1]_600:250_um"),
        "the session must name the via by its true diameter and drill, not `defaultVia`: {ses}"
    );
    assert!(
        !ses.contains("defaultVia"),
        "the synthetic name must not reach the session: {ses}"
    );
}

fn via_padstack_names(board: &Board) -> Vec<String> {
    let ctx = board.ctx();
    let mut names: Vec<String> = board
        .get_vias()
        .into_iter()
        .map(|id| match board.get_item(id) {
            Some(copper_board::Item::Via(via)) => {
                via.get_padstack(&ctx).expect("registered").name.clone()
            }
            _ => panic!("a via"),
        })
        .collect();
    names.sort();
    names.dedup();
    names
}

#[test]
fn vias_whose_drills_differ_below_the_name_rounding_get_one_distinct_padstack_each() {
    let via = |x: i32, drill: &str| {
        format!(
            r#"{{"netName":"GND","position":{{"x":{x},"y":1}},"diameter":0.6,"drill":{drill},"startLayerIndex":0,"endLayerIndex":1}}"#
        )
    };
    let b = board(&format!(
        r#"{{"layers":[{{"name":"F.Cu"}},{{"name":"B.Cu"}}],"nets":[{{"id":1,"name":"GND"}}],
            "vias":[{},{},{}]}}"#,
        via(1, "0.3"),
        via(3, "0.30004"),
        via(5, "0.30004")
    ));
    assert_eq!(
        via_padstack_names(&b),
        vec![
            "Via[0-1]_600:300_um".to_string(),
            "Via[0-1]_600:300_um#2".to_string()
        ],
        "the second drill gets one uniquely named padstack that the third via shares"
    );
    assert_eq!(
        b.library.padstacks.count(),
        3,
        "defaultVia plus one padstack per distinct drill"
    );
}

#[test]
fn only_non_plated_holes_without_a_copper_ring_are_hole_only() {
    let b = board(
        r#"{"layers":[{"name":"F.Cu"},{"name":"B.Cu"}],
            "components":[
            {"reference":"H1","position":{"x":0,"y":0},"pads":[{"name":"1","shape":"circle","size":{"x":6,"y":6},"drill":3.2,"nonPlated":true,"layers":["F.Cu","B.Cu"]}]},
            {"reference":"H2","position":{"x":10,"y":0},"pads":[{"name":"1","shape":"circle","size":{"x":1,"y":1},"drill":1,"nonPlated":true,"layers":["F.Cu","B.Cu"]}]},
            {"reference":"H3","position":{"x":20,"y":0},"pads":[{"name":"1","shape":"circle","size":{"x":1,"y":1},"nonPlated":true,"layers":["F.Cu","B.Cu"]}]}]}"#,
    );
    let hole_only = |index| {
        b.library
            .padstacks
            .get(first_pin_padstack(&b, index))
            .expect("padstack")
            .hole_only
    };
    assert!(
        !hole_only(0),
        "a copper ring around a non-plated hole is still copper"
    );
    assert!(
        hole_only(1),
        "copper no wider than the drill is a bare hole"
    );
    assert!(!hole_only(2), "without a drill there is no hole to be bare");
}

#[test]
fn a_via_drill_the_net_class_does_not_declare_is_flagged_estimated() {
    let b = board(
        r#"{"layers":[{"name":"F.Cu"},{"name":"B.Cu"}],
            "netClasses":[{"name":"Default","viaDiameter":0.5},
                          {"name":"Power","viaDiameter":0.8,"viaDrill":0.4},
                          {"name":"Signal","viaDiameter":0.6}],
            "nets":[{"id":1,"name":"GND","className":"Default"}]}"#,
    );
    let padstack = |name| b.library.padstacks.get_by_name(name).expect(name);
    assert_eq!(padstack("defaultVia").drill_diameter, Some(4000.0));
    assert!(padstack("defaultVia").drill_estimated);
    assert!(!padstack("via_Power").drill_estimated);
    assert!(padstack("via_Signal").drill_estimated);
}

#[test]
fn a_through_hole_pad_without_a_drill_keeps_no_exact_drill() {
    let b = board(&two_pads_on(
        r#""F.Cu","B.Cu""#,
        r#""F.Cu","B.Cu""#,
        0.0,
        0.0,
    ));
    let p = b
        .library
        .padstacks
        .get(first_pin_padstack(&b, 0))
        .expect("padstack");
    assert_eq!(
        p.drill_diameter, None,
        "an absent drill is unknown, not a zero-diameter hole"
    );
}

#[test]
fn rounded_pad_radii_are_validated_and_part_of_padstack_identity() {
    let input = |ratio: f64| {
        serde_json::json!({"layers":[{"name":"F.Cu"},{"name":"B.Cu"}],
        "components":[{"reference":"J","pads":[
            {"name":"1","shape":"roundrect","roundRectRatio":ratio,"size":{"x":1.2,"y":0.6},"layers":["F.Cu"]},
            {"name":"2","shape":"roundrect","roundRectRatio":0.1,"size":{"x":1.2,"y":0.6},"layers":["F.Cu"]}
        ]}]}).to_string()
    };
    let b = board(&input(0.25));
    let ctx = b.ctx();
    let mut radii: Vec<_> = b
        .get_pins()
        .into_iter()
        .map(|id| {
            let Some(copper_board::Item::Pin(pin)) = b.get_item(id) else {
                panic!("pin")
            };
            pin.get_padstack(&ctx).unwrap().round_rect_radius.unwrap()
        })
        .collect();
    radii.sort_by(f64::total_cmp);
    assert_eq!(radii, vec![600.0, 1500.0]);
    for ratio in [-0.1, 0.6] {
        assert!(!matches!(
            copper_dsn::kicad::read_board(&input(ratio), None),
            copper_dsn::BoardReadResult::Success { .. }
        ));
    }
}

#[test]
fn rounded_pads_without_radius_metadata_keep_the_legacy_rectangle() {
    let input = r#"{"layers":[{"name":"F.Cu"},{"name":"B.Cu"}],
        "components":[{"reference":"J","pads":[
            {"name":"1","shape":"ROUNDRECT","size":{"x":1.2,"y":0.6},"layers":["F.Cu"]},
            {"name":"2","shape":"rect","size":{"x":1.2,"y":0.6},"layers":["F.Cu"]}
        ]}]} "#;
    let b = board(input);
    let first = first_pin_padstack(&b, 0);
    let package = b.library.packages.get(b.components.get(1).get_package());
    assert_eq!(first, package.get_pin(1).unwrap().padstack_no);
    assert_eq!(
        b.library.padstacks.get(first).unwrap().round_rect_radius,
        None
    );
}
