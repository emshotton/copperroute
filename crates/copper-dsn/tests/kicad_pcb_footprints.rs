use copper_dsn::kicad::pcb::footprints::read_components;
use copper_dsn::kicad::pcb::structure::{Layers, NetTable};
use copper_dsn::kicad::sexpr::parse;

const LAYERS: &str = r#"(layers (0 "F.Cu" signal) (2 "B.Cu" signal))"#;

fn read(body: &str) -> (Vec<copper_dsn::kicad::ComponentJson>, Vec<String>) {
    let text = format!("(kicad_pcb (version 20241229) {LAYERS} (net 1 \"GND\") {body})");
    let root = parse(&text).expect("it parses");
    let layers = Layers::read(&root).expect("layers");
    let nets = NetTable::read(&root).expect("nets");
    let mut warnings = Vec::new();
    let (components, _) =
        read_components(&root, &layers, &nets, &mut warnings).expect("components");
    (components, warnings)
}

fn read_err(body: &str) -> String {
    let text = format!("(kicad_pcb (version 20241229) {LAYERS} (net 1 \"GND\") {body})");
    let root = parse(&text).expect("it parses");
    let layers = Layers::read(&root).expect("layers");
    let nets = NetTable::read(&root).expect("nets");
    let mut warnings = Vec::new();
    read_components(&root, &layers, &nets, &mut warnings)
        .expect_err("it fails")
        .message
}

#[test]
fn it_gives_each_pad_its_own_component_at_absolute_position() {
    let (components, _) = read(concat!(
        r#"(footprint "R" (layer "F.Cu") (at 10 20)"#,
        r#" (property "Reference" "R1")"#,
        r#" (pad "1" smd rect (at 1 0) (size 0.8 0.9) (layers "F.Cu") (net 1 "GND")))"#,
    ));
    assert_eq!(components.len(), 1);
    let pad = &components[0].pads.as_ref().expect("pads")[0];
    assert_eq!(pad.netName.as_deref(), Some("GND"));
    let position = components[0].position.as_ref().expect("a position");
    assert!((position.x - 11.0).abs() < 1e-9, "got {}", position.x);
    assert!((position.y - 20.0).abs() < 1e-9, "got {}", position.y);
}

#[test]
fn it_rotates_a_pad_offset_about_the_footprint_origin() {
    let (components, _) = read(concat!(
        r#"(footprint "R" (layer "F.Cu") (at 10 20 90)"#,
        r#" (pad "1" smd rect (at 1 0) (size 0.8 0.9) (layers "F.Cu") (net 1 "GND")))"#,
    ));
    let position = components[0].position.as_ref().expect("a position");
    assert!((position.x - 10.0).abs() < 1e-9, "got {}", position.x);
    assert!((position.y - 19.0).abs() < 1e-9, "got {}", position.y);
}

#[test]
fn it_expands_a_wildcard_layer_span() {
    let (components, _) = read(concat!(
        r#"(footprint "J" (layer "F.Cu") (at 0 0)"#,
        r#" (pad "1" thru_hole circle (at 0 0) (size 1 1) (drill 0.5) (layers "*.Cu") (net 1 "GND")))"#,
    ));
    let pad = &components[0].pads.as_ref().expect("pads")[0];
    assert_eq!(pad.layers.as_ref().expect("layers").len(), 2);
}

#[test]
fn it_skips_a_paste_only_aperture() {
    let (components, _) = read(concat!(
        r#"(footprint "A" (layer "F.Cu") (at 0 0)"#,
        r#" (pad "1" smd rect (at 0 0) (size 1 1) (layers "F.Paste")))"#,
    ));
    assert!(components.is_empty());
}

#[test]
fn it_warns_that_rounded_pads_route_as_rectangles() {
    let (_, warnings) = read(concat!(
        r#"(footprint "R" (layer "F.Cu") (at 0 0)"#,
        r#" (pad "1" smd roundrect (at 0 0) (size 1 1) (roundrect_rratio 0.25)"#,
        r#" (layers "F.Cu") (net 1 "GND")))"#,
    ));
    assert!(
        warnings
            .iter()
            .any(|w| w.contains("corner radius for hole DRC"))
    );
}

#[test]
fn it_rejects_net_ties() {
    let error = read_err(concat!(
        r#"(footprint "U" (layer "F.Cu") (at 0 0) (net_tie_pad_groups "1,2")"#,
        r#" (pad "1" smd rect (at 0 0) (size 1 1) (layers "F.Cu") (net 1 "GND")))"#,
    ));
    assert_eq!(error, "Net ties are not supported yet.");
}

#[test]
fn it_rejects_a_footprint_zone() {
    let error = read_err(concat!(
        r#"(footprint "U" (layer "F.Cu") (at 0 0) (zone (net 1))"#,
        r#" (pad "1" smd rect (at 0 0) (size 1 1) (layers "F.Cu") (net 1 "GND")))"#,
    ));
    assert_eq!(error, "Footprint zones are not supported yet.");
}

#[test]
fn it_rejects_an_unsupported_pad_shape() {
    let error = read_err(concat!(
        r#"(footprint "U" (layer "F.Cu") (at 0 0)"#,
        r#" (pad "1" smd trapezoid (at 0 0) (size 1 1) (layers "F.Cu") (net 1 "GND")))"#,
    ));
    assert_eq!(error, "Unsupported pad FP0.1: smd/trapezoid");
}

#[test]
fn it_rejects_a_nonpositive_pad_size() {
    let error = read_err(concat!(
        r#"(footprint "U" (layer "F.Cu") (at 0 0)"#,
        r#" (pad "1" smd rect (at 0 0) (size 0 1) (layers "F.Cu") (net 1 "GND")))"#,
    ));
    assert_eq!(error, "Pad sizes must be positive.");
}

#[test]
fn it_rejects_an_unequal_circular_pad() {
    let error = read_err(concat!(
        r#"(footprint "U" (layer "F.Cu") (at 0 0)"#,
        r#" (pad "1" smd circle (at 0 0) (size 1 2) (layers "F.Cu") (net 1 "GND")))"#,
    ));
    assert_eq!(error, "Circular pads must have equal dimensions.");
}

#[test]
fn it_prefers_the_pads_own_solder_mask_margin_over_the_footprints() {
    let (components, _) = read(concat!(
        r#"(setup (pad_to_mask_clearance 0.3))"#,
        r#"(footprint "R" (layer "F.Cu") (at 0 0) (solder_mask_margin 0.2)"#,
        r#" (pad "1" smd rect (at 0 0) (size 1 1) (solder_mask_margin 0.05)"#,
        r#" (layers "F.Cu" "F.Mask") (net 1 "GND")))"#,
    ));
    let pad = &components[0].pads.as_ref().expect("pads")[0];
    let expansion = pad.solderMaskExpansion.as_ref().expect("an expansion map");
    assert!(
        (expansion["F.Mask"] - 0.05).abs() < 1e-9,
        "got {expansion:?}"
    );
}

#[test]
fn it_prefers_the_footprints_solder_mask_margin_over_the_boards() {
    let (components, _) = read(concat!(
        r#"(setup (pad_to_mask_clearance 0.3))"#,
        r#"(footprint "R" (layer "F.Cu") (at 0 0) (solder_mask_margin 0.15)"#,
        r#" (pad "1" smd rect (at 0 0) (size 1 1) (layers "F.Cu" "F.Mask") (net 1 "GND")))"#,
    ));
    let pad = &components[0].pads.as_ref().expect("pads")[0];
    let expansion = pad.solderMaskExpansion.as_ref().expect("an expansion map");
    assert!(
        (expansion["F.Mask"] - 0.15).abs() < 1e-9,
        "got {expansion:?}"
    );
}

#[test]
fn it_falls_back_to_the_boards_pad_to_mask_clearance() {
    let (components, _) = read(concat!(
        r#"(setup (pad_to_mask_clearance 0.1))"#,
        r#"(footprint "R" (layer "F.Cu") (at 0 0)"#,
        r#" (pad "1" smd rect (at 0 0) (size 1 1) (layers "F.Cu" "F.Mask") (net 1 "GND")))"#,
    ));
    let pad = &components[0].pads.as_ref().expect("pads")[0];
    let expansion = pad.solderMaskExpansion.as_ref().expect("an expansion map");
    assert!(
        (expansion["F.Mask"] - 0.1).abs() < 1e-9,
        "got {expansion:?}"
    );
}

#[test]
fn it_gives_a_pad_with_no_mask_layer_an_empty_expansion_map() {
    let (components, _) = read(concat!(
        r#"(setup (pad_to_mask_clearance 0.2))"#,
        r#"(footprint "R" (layer "F.Cu") (at 0 0)"#,
        r#" (pad "1" smd rect (at 0 0) (size 1 1) (layers "F.Cu") (net 1 "GND")))"#,
    ));
    let pad = &components[0].pads.as_ref().expect("pads")[0];
    let expansion = pad.solderMaskExpansion.as_ref().expect("an expansion map");
    assert!(expansion.is_empty(), "got {expansion:?}");
}

#[test]
fn it_expands_a_wildcard_mask_layer_to_both_sides() {
    let (components, _) = read(concat!(
        r#"(setup (pad_to_mask_clearance 0.2))"#,
        r#"(footprint "J" (layer "F.Cu") (at 0 0)"#,
        r#" (pad "1" thru_hole circle (at 0 0) (size 1 1) (drill 0.5)"#,
        r#" (layers "*.Cu" "*.Mask") (net 1 "GND")))"#,
    ));
    let pad = &components[0].pads.as_ref().expect("pads")[0];
    let expansion = pad.solderMaskExpansion.as_ref().expect("an expansion map");
    assert_eq!(expansion.len(), 2, "got {expansion:?}");
    assert!(
        (expansion["F.Mask"] - 0.2).abs() < 1e-9,
        "got {expansion:?}"
    );
    assert!(
        (expansion["B.Mask"] - 0.2).abs() < 1e-9,
        "got {expansion:?}"
    );
}

#[test]
fn it_takes_allow_solder_mask_bridges_from_the_footprints_attr_not_the_pad() {
    let (components, _) = read(concat!(
        r#"(footprint "R" (layer "F.Cu") (at 0 0) (attr smd allow_soldermask_bridges)"#,
        r#" (pad "1" smd rect (at 0 0) (size 1 1) (layers "F.Cu") (net 1 "GND")))"#,
    ));
    let pad = &components[0].pads.as_ref().expect("pads")[0];
    assert!(pad.allowSolderMaskBridges);
}
