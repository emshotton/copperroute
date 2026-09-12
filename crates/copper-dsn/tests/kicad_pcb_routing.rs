use copper_dsn::kicad::pcb::routing::{check_zones, read_copper_graphics, read_traces, read_vias};
use copper_dsn::kicad::pcb::structure::{Layers, NetTable};
use copper_dsn::kicad::sexpr::parse;

const LAYERS: &str = r#"(layers (0 "F.Cu" signal) (2 "B.Cu" signal))"#;

fn root_of(body: &str) -> copper_dsn::kicad::sexpr::Node {
    let text = format!("(kicad_pcb (version 20241229) {LAYERS} (net 1 \"GND\") {body})");
    parse(&text).expect("it parses")
}

#[test]
fn it_reads_a_track() {
    let root = root_of(r#"(segment (start 1 2) (end 3 4) (width 0.25) (layer "F.Cu") (net 1))"#);
    let layers = Layers::read(&root).expect("layers");
    let nets = NetTable::read(&root).expect("nets");
    let traces = read_traces(&root, &layers, &nets).expect("traces");
    assert_eq!(traces.len(), 1);
    assert_eq!(traces[0].netName.as_deref(), Some("GND"));
    assert_eq!(traces[0].layerIndex, 0);
    assert_eq!(traces[0].width, 0.25);
    let points = traces[0].points.as_ref().expect("points");
    assert_eq!(points.len(), 2);
    assert_eq!((points[0].x, points[0].y), (1.0, 2.0));
    assert_eq!((points[1].x, points[1].y), (3.0, 4.0));
}

#[test]
fn it_rejects_a_track_without_a_net() {
    let root = root_of(r#"(segment (start 1 2) (end 3 4) (width 0.25) (layer "F.Cu"))"#);
    let layers = Layers::read(&root).expect("layers");
    let nets = NetTable::read(&root).expect("nets");
    let error = read_traces(&root, &layers, &nets).expect_err("it fails");
    assert_eq!(error.message, "Tracks without a net are not supported yet.");
}

#[test]
fn it_rejects_a_locked_track() {
    let root = root_of(
        r#"(segment (start 1 2) (end 3 4) (width 0.25) (layer "F.Cu") (net 1) (locked yes))"#,
    );
    let layers = Layers::read(&root).expect("layers");
    let nets = NetTable::read(&root).expect("nets");
    let error = read_traces(&root, &layers, &nets).expect_err("it fails");
    assert_eq!(error.message, "Locked tracks are not supported yet.");
}

#[test]
fn it_reads_a_through_via() {
    let root = root_of(r#"(via (at 5 6) (size 0.6) (drill 0.3) (layers "F.Cu" "B.Cu") (net 1))"#);
    let layers = Layers::read(&root).expect("layers");
    let nets = NetTable::read(&root).expect("nets");
    let vias = read_vias(&root, &layers, &nets).expect("vias");
    assert_eq!(vias.len(), 1);
    assert_eq!(vias[0].startLayerIndex, 0);
    assert_eq!(vias[0].endLayerIndex, 1);
}

#[test]
fn it_rejects_a_blind_via() {
    let root =
        root_of(r#"(via blind (at 5 6) (size 0.6) (drill 0.3) (layers "F.Cu" "B.Cu") (net 1))"#);
    let layers = Layers::read(&root).expect("layers");
    let nets = NetTable::read(&root).expect("nets");
    let error = read_vias(&root, &layers, &nets).expect_err("it fails");
    assert_eq!(
        error.message,
        "Locked, blind, and micro vias are not supported yet."
    );
}

#[test]
fn it_warns_about_copper_zones_without_routing_against_them() {
    let root = root_of(r#"(zone (net 1) (layer "F.Cu"))"#);
    let nets = NetTable::read(&root).expect("nets");
    let mut warnings = Vec::new();
    check_zones(&root, &nets, &mut warnings).expect("zones are accepted");
    assert_eq!(
        warnings,
        vec![
            "1 copper zones will be preserved with their fill cache removed. Routing uses \
             tracks only; refill zones in KiCad (B), then run DRC."
        ]
    );
}

#[test]
fn it_warns_about_preserved_keepouts_that_allow_tracks_and_vias() {
    let root = root_of(r#"(zone (layer "F.Cu") (keepout (tracks allowed) (vias allowed)))"#);
    let nets = NetTable::read(&root).expect("nets");
    let mut warnings = Vec::new();
    check_zones(&root, &nets, &mut warnings).expect("the keepout is accepted");
    assert_eq!(
        warnings,
        vec!["1 keepout areas allow tracks and vias and are preserved for KiCad zone refill."]
    );
}

#[test]
fn it_rejects_a_netless_copper_zone() {
    let root = root_of(r#"(zone (layer "F.Cu"))"#);
    let nets = NetTable::read(&root).expect("nets");
    let mut warnings = Vec::new();
    let error = check_zones(&root, &nets, &mut warnings).expect_err("it fails");
    assert_eq!(
        error.message,
        "Netless copper zones are not supported for routing yet."
    );
}

#[test]
fn it_rejects_a_keepout_that_restricts_tracks() {
    let root = root_of(r#"(zone (layer "F.Cu") (keepout (tracks not_allowed) (vias allowed)))"#);
    let nets = NetTable::read(&root).expect("nets");
    let mut warnings = Vec::new();
    let error = check_zones(&root, &nets, &mut warnings).expect_err("it fails");
    assert_eq!(
        error.message,
        "Zone keepouts that restrict tracks or vias are not supported for routing yet."
    );
}

#[test]
fn it_reserves_copper_text_as_an_obstacle() {
    let root = root_of(
        r#"(gr_text "HI" (at 5 5 0) (layer "F.Cu") (effects (font (size 1 1) (thickness 0.15))))"#,
    );
    let layers = Layers::read(&root).expect("layers");
    let mut warnings = Vec::new();
    let areas = read_copper_graphics(&root, &layers, &mut warnings).expect("areas");
    assert_eq!(areas.len(), 1);
    assert!(areas[0].isObstacle);
    let polygon = areas[0].polygon.as_ref().expect("a polygon");
    assert_eq!(polygon.len(), 4);
    let corners: Vec<(f64, f64)> = polygon.iter().map(|p| (p.x, p.y)).collect();
    assert_eq!(
        corners,
        vec![(3.0, 4.0), (7.0, 4.0), (7.0, 6.0), (3.0, 6.0)]
    );
    assert!(warnings.iter().any(|w| w.contains("Copper text")));
}

#[test]
fn it_rejects_copper_text_on_a_cu_suffixed_layer_absent_from_the_table() {
    let root = root_of(
        r#"(gr_text "HI" (at 5 5 0) (layer "In1.Cu") (effects (font (size 1 1) (thickness 0.15))))"#,
    );
    let layers = Layers::read(&root).expect("layers");
    let mut warnings = Vec::new();
    let error = read_copper_graphics(&root, &layers, &mut warnings).expect_err("it fails");
    assert_eq!(error.message, "Unknown copper layer: In1.Cu");
}

#[test]
fn it_rejects_a_via_whose_layer_span_skips_an_inner_layer() {
    let root = root_of(r#"(via (at 5 6) (size 0.6) (drill 0.3) (layers "F.Cu" "In1.Cu") (net 1))"#);
    let layers = Layers::read(&root).expect("layers");
    let nets = NetTable::read(&root).expect("nets");
    let error = read_vias(&root, &layers, &nets).expect_err("it fails");
    assert_eq!(
        error.message,
        "Only through vias assigned to a net are supported."
    );
}

fn corners_of(area: &copper_dsn::kicad::ConductionAreaJson) -> Vec<(f64, f64)> {
    area.polygon
        .as_ref()
        .expect("a polygon")
        .iter()
        .map(|p| (p.x, p.y))
        .collect()
}

fn assert_close(got: &[(f64, f64)], want: &[(f64, f64)]) {
    assert_eq!(got.len(), want.len(), "got {got:?}, want {want:?}");
    for (g, w) in got.iter().zip(want) {
        assert!(
            (g.0 - w.0).abs() < 1e-9 && (g.1 - w.1).abs() < 1e-9,
            "got {got:?}, want {want:?}"
        );
    }
}

#[test]
fn it_reserves_a_copper_line_as_an_obstacle() {
    let root = root_of(
        r#"(gr_line (start 0 0) (end 4 0) (layer "F.Cu") (stroke (width 0.2) (type solid)))"#,
    );
    let layers = Layers::read(&root).expect("layers");
    let mut warnings = Vec::new();
    let areas = read_copper_graphics(&root, &layers, &mut warnings).expect("areas");
    assert_eq!(areas.len(), 1);
    assert!(areas[0].isObstacle);
    assert_close(
        &corners_of(&areas[0]),
        &[(-0.1, -0.1), (4.1, -0.1), (4.1, 0.1), (-0.1, 0.1)],
    );
    assert!(warnings.iter().any(|w| w.contains("Copper lines")));
}

#[test]
fn it_reserves_a_copper_rectangle_as_an_obstacle() {
    let root = root_of(
        r#"(gr_rect (start 0 0) (end 4 2) (layer "F.Cu") (stroke (width 0.2) (type solid)) (fill none))"#,
    );
    let layers = Layers::read(&root).expect("layers");
    let mut warnings = Vec::new();
    let areas = read_copper_graphics(&root, &layers, &mut warnings).expect("areas");
    assert_eq!(areas.len(), 1);
    assert_close(
        &corners_of(&areas[0]),
        &[(-0.1, -0.1), (4.1, -0.1), (4.1, 2.1), (-0.1, 2.1)],
    );
    assert!(warnings.iter().any(|w| w.contains("Copper rectangles")));
}

#[test]
fn it_reserves_a_copper_circle_as_an_obstacle() {
    let root = root_of(
        r#"(gr_circle (center 2 0) (end 2.5 0) (layer "F.Cu") (stroke (width 0.1) (type solid)) (fill none))"#,
    );
    let layers = Layers::read(&root).expect("layers");
    let mut warnings = Vec::new();
    let areas = read_copper_graphics(&root, &layers, &mut warnings).expect("areas");
    assert_eq!(areas.len(), 1);
    assert_close(
        &corners_of(&areas[0]),
        &[(1.45, -0.55), (2.55, -0.55), (2.55, 0.55), (1.45, 0.55)],
    );
    assert!(warnings.iter().any(|w| w.contains("Copper circles")));
}

#[test]
fn it_reserves_a_copper_arc_as_an_obstacle() {
    let root = root_of(
        r#"(gr_arc (start 0 0) (mid 1 1) (end 2 0) (layer "F.Cu") (stroke (width 0.2) (type solid)))"#,
    );
    let layers = Layers::read(&root).expect("layers");
    let mut warnings = Vec::new();
    let areas = read_copper_graphics(&root, &layers, &mut warnings).expect("areas");
    assert_eq!(areas.len(), 1);
    assert_close(
        &corners_of(&areas[0]),
        &[(-0.1, -0.1), (2.1, -0.1), (2.1, 1.1), (-0.1, 1.1)],
    );
    assert!(warnings.iter().any(|w| w.contains("Copper arcs")));
}

#[test]
fn it_reserves_a_copper_polygon_as_an_obstacle() {
    let root = root_of(
        r#"(gr_poly (pts (xy 0 0) (xy 2 0) (xy 1 2)) (layer "F.Cu") (stroke (width 0.2) (type solid)) (fill yes))"#,
    );
    let layers = Layers::read(&root).expect("layers");
    let mut warnings = Vec::new();
    let areas = read_copper_graphics(&root, &layers, &mut warnings).expect("areas");
    assert_eq!(areas.len(), 1);
    assert_close(
        &corners_of(&areas[0]),
        &[(-0.1, -0.1), (2.1, -0.1), (2.1, 2.1), (-0.1, 2.1)],
    );
    assert!(warnings.iter().any(|w| w.contains("Copper polygons")));
}

#[test]
fn it_still_rejects_a_top_level_copper_curve() {
    let root = root_of(r#"(gr_curve (layer "F.Cu"))"#);
    let layers = Layers::read(&root).expect("layers");
    let mut warnings = Vec::new();
    let error = read_copper_graphics(&root, &layers, &mut warnings).expect_err("it fails");
    assert_eq!(error.message, "Unsupported copper object: gr_curve");
}
