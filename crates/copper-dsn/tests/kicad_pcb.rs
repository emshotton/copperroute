use copper_dsn::error::BoardReadResult;
use copper_dsn::kicad::pcb::{default_net_class, read_pcb};
use copper_dsn::kicad::read_board_json;

fn example() -> String {
    let path = testkit::workspace_root().join("web/example.kicad_pcb");
    std::fs::read_to_string(&path).expect("the example board reads")
}

const LAYERS: &str = r#"(layers (0 "F.Cu" signal) (2 "B.Cu" signal))"#;

#[test]
fn it_imports_the_example_board() {
    let imported =
        read_pcb(&example(), "example", &default_net_class()).expect("the board imports");
    assert!(
        !imported
            .board
            .components
            .as_ref()
            .expect("components")
            .is_empty()
    );
    assert!(imported.board.outline.is_some());
    assert!(!imported.board.nets.as_ref().expect("nets").is_empty());
}

#[test]
fn the_imported_board_loads_through_the_reader() {
    let imported =
        read_pcb(&example(), "example", &default_net_class()).expect("the board imports");
    match read_board_json(imported.board, None) {
        BoardReadResult::Success {
            board: Some(board), ..
        } => {
            assert!(board.get_layer_count() >= 1);
        }
        other => panic!("expected a loaded board, got {other:?}"),
    }
}

#[test]
fn it_sets_the_design_name_from_the_name_parameter() {
    let imported =
        read_pcb(&example(), "my-board", &default_net_class()).expect("the board imports");
    assert_eq!(imported.board.designName.as_deref(), Some("my-board"));
}

#[test]
fn it_rejects_a_file_that_is_not_a_board() {
    let error =
        read_pcb("(kicad_sch (version 1))", "t", &default_net_class()).expect_err("it fails");
    assert_eq!(error.message, "Expected one kicad_pcb board.");
}

#[test]
fn it_rejects_a_board_with_no_pads() {
    let text = r#"(kicad_pcb (version 20241229) (layers (0 "F.Cu" signal))
        (gr_rect (start 0 0) (end 10 10) (layer "Edge.Cuts")))"#;
    let error = read_pcb(text, "t", &default_net_class()).expect_err("it fails");
    assert_eq!(error.message, "No pads were found.");
}

#[test]
fn it_rejects_curved_tracks() {
    let text = r#"(kicad_pcb (version 20241229) (layers (0 "F.Cu" signal)) (net 1 "GND")
        (gr_rect (start 0 0) (end 10 10) (layer "Edge.Cuts"))
        (footprint "R" (layer "F.Cu") (at 1 1)
          (pad "1" smd rect (at 0 0) (size 1 1) (layers "F.Cu") (net 1 "GND")))
        (arc (start 1 1) (mid 2 2) (end 3 3) (width 0.25) (layer "F.Cu") (net 1)))"#;
    let error = read_pcb(text, "t", &default_net_class()).expect_err("it fails");
    assert_eq!(error.message, "Curved tracks are not supported yet.");
}

#[test]
fn it_rejects_a_top_level_copper_object_on_a_cu_suffixed_layer_absent_from_the_table() {
    let text = r#"(kicad_pcb (version 20241229) (layers (0 "F.Cu" signal) (2 "B.Cu" signal))
        (gr_rect (start 0 0) (end 10 10) (layer "Edge.Cuts"))
        (gr_curve (layer "In1.Cu")))"#;
    let error = read_pcb(text, "t", &default_net_class()).expect_err("it fails");
    assert_eq!(error.message, "Unsupported copper object: gr_curve");
}

/// Two footprint copper rectangles each push the identical "Footprint copper rectangles..."
/// warning from `read_components`; `read_pcb` deduplicates warnings, so the imported board must
/// carry that text once, not twice.
#[test]
fn duplicate_warnings_are_collapsed_to_one() {
    let text = format!(
        r#"(kicad_pcb (version 20241229) {LAYERS} (net 1 "GND")
        (gr_rect (start 0 0) (end 10 10) (layer "Edge.Cuts"))
        (footprint "R1" (layer "F.Cu") (at 1 1)
          (fp_rect (start 0 0) (end 1 1) (layer "F.Cu"))
          (pad "1" smd rect (at 0 0) (size 1 1) (layers "F.Cu") (net 1 "GND")))
        (footprint "R2" (layer "F.Cu") (at 3 3)
          (fp_rect (start 0 0) (end 1 1) (layer "F.Cu"))
          (pad "1" smd rect (at 0 0) (size 1 1) (layers "F.Cu") (net 1 "GND"))))"#
    );
    let imported = read_pcb(&text, "t", &default_net_class()).expect("the board imports");
    let message = "Footprint copper rectangles are reserved as solid routing obstacles and \
                    preserved in downloads.";
    let occurrences = imported
        .warnings
        .iter()
        .filter(|warning| warning.as_str() == message)
        .count();
    assert_eq!(
        occurrences, 1,
        "expected the duplicate warning collapsed once, got {:?}",
        imported.warnings
    );
    assert_eq!(
        imported
            .board
            .conductionAreas
            .as_ref()
            .expect("areas")
            .len(),
        2
    );
}

#[test]
fn default_net_class_matches_what_read_pcb_falls_back_to() {
    let defaults = default_net_class();
    let text = format!(
        r#"(kicad_pcb (version 20241229) {LAYERS} (net 1 "GND")
        (gr_rect (start 0 0) (end 10 10) (layer "Edge.Cuts"))
        (footprint "R" (layer "F.Cu") (at 1 1)
          (pad "1" smd rect (at 0 0) (size 1 1) (layers "F.Cu") (net 1 "GND"))))"#
    );
    let imported = read_pcb(&text, "t", &defaults).expect("the board imports");
    let classes = imported.board.netClasses.expect("net classes");
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].name.as_deref(), Some("Default"));
    assert_eq!(classes[0].clearance, defaults.clearance);
}

/// `read_pcb` validates `defaults` only when the board carries no embedded net class named
/// "Default"; a minimal board with no `net_class` node at all reaches that branch.
#[test]
fn it_rejects_invalid_routing_rules() {
    let mut bad = default_net_class();
    bad.clearance = 0.0;
    let error = read_pcb("(kicad_pcb (version 20241229))", "t", &bad).expect_err("it fails");
    assert_eq!(error.message, "Invalid routing rules.");
}

#[test]
fn it_rejects_a_via_drill_that_is_not_smaller_than_the_diameter() {
    let mut bad = default_net_class();
    bad.viaDrill = 0.6;
    bad.viaDiameter = 0.6;
    let error = read_pcb("(kicad_pcb (version 20241229))", "t", &bad).expect_err("it fails");
    assert_eq!(error.message, "Via drill must be smaller than diameter.");
}

/// The same invalid `defaults` that trips `it_rejects_invalid_routing_rules` must be left
/// unchecked once the board supplies its own "Default" net class; the board below fails later,
/// for lack of pads, proving the rules were never inspected.
#[test]
fn defaults_are_not_validated_when_the_board_has_its_own_default_class() {
    let mut bad = default_net_class();
    bad.clearance = 0.0;
    let text = r#"(kicad_pcb (version 20241229) (layers (0 "F.Cu" signal))
        (net_class "Default" (clearance 0.2) (trace_width 0.25) (via_dia 0.6) (via_drill 0.3))
        (gr_rect (start 0 0) (end 10 10) (layer "Edge.Cuts")))"#;
    let error = read_pcb(text, "t", &bad).expect_err("it fails");
    assert_eq!(error.message, "No pads were found.");
}

fn one_pad(reference: &str, x: f64, y: f64) -> String {
    format!(
        r#"(footprint "{reference}" (layer "F.Cu") (at {x} {y})
          (pad "1" smd rect (at 0 0) (size 1 1) (layers "F.Cu") (net 1 "GND")))"#
    )
}

#[test]
fn pads_carry_their_footprint_index_and_their_own_pad_number() {
    let text = format!(
        r#"(kicad_pcb (version 20241229) {LAYERS} (net 1 "GND")
        (gr_rect (start 0 0) (end 10 10) (layer "Edge.Cuts"))
        (footprint "R1" (layer "F.Cu") (at 1 1)
          (pad "1" smd rect (at 0 0) (size 1 1) (layers "F.Cu") (net 1 "GND")))
        (footprint "R2" (layer "F.Cu") (at 3 3)
          (pad "A2" smd rect (at 0 0) (size 1 1) (layers "F.Cu") (net 1 "GND"))))"#
    );
    let imported = read_pcb(&text, "t", &default_net_class()).expect("the board imports");
    let components = imported.board.components.expect("components");
    let first = components[0].pads.as_ref().expect("pads")[0].clone();
    assert_eq!(first.sourceFootprint.as_deref(), Some("0"));
    assert_eq!(first.sourcePadNumber.as_deref(), Some("1"));
    let second = components[1].pads.as_ref().expect("pads")[0].clone();
    assert_eq!(second.sourceFootprint.as_deref(), Some("1"));
    assert_eq!(second.sourcePadNumber.as_deref(), Some("A2"));
}

#[test]
fn an_explicit_zero_pad_clearance_inherits_the_footprint_default_on_an_old_format_board() {
    let text = format!(
        r#"(kicad_pcb (version 20240201) {LAYERS} (net 1 "GND")
        (gr_rect (start 0 0) (end 10 10) (layer "Edge.Cuts"))
        (footprint "R" (layer "F.Cu") (at 1 1)
          (pad "1" smd rect (at 0 0) (size 1 1) (layers "F.Cu") (net 1 "GND") (clearance 0))))"#
    );
    let imported = read_pcb(&text, "t", &default_net_class()).expect("the board imports");
    let components = imported.board.components.expect("components");
    let pad = &components[0].pads.as_ref().expect("pads")[0];
    assert_eq!(pad.copperClearance, None);
}

#[test]
fn an_explicit_zero_pad_clearance_overrides_the_footprint_default_on_a_new_format_board() {
    let text = format!(
        r#"(kicad_pcb (version 20240202) {LAYERS} (net 1 "GND")
        (gr_rect (start 0 0) (end 10 10) (layer "Edge.Cuts"))
        (footprint "R" (layer "F.Cu") (at 1 1)
          (pad "1" smd rect (at 0 0) (size 1 1) (layers "F.Cu") (net 1 "GND") (clearance 0))))"#
    );
    let imported = read_pcb(&text, "t", &default_net_class()).expect("the board imports");
    let components = imported.board.components.expect("components");
    let pad = &components[0].pads.as_ref().expect("pads")[0];
    assert_eq!(pad.copperClearance, Some(0.0));
}

#[test]
fn a_pad_clearance_takes_priority_over_the_footprint_clearance() {
    let text = format!(
        r#"(kicad_pcb (version 20241229) {LAYERS} (net 1 "GND")
        (gr_rect (start 0 0) (end 10 10) (layer "Edge.Cuts"))
        (footprint "R" (layer "F.Cu") (at 1 1) (clearance 0.3)
          (pad "1" smd rect (at 0 0) (size 1 1) (layers "F.Cu") (net 1 "GND") (clearance 0.15))))"#
    );
    let imported = read_pcb(&text, "t", &default_net_class()).expect("the board imports");
    let components = imported.board.components.expect("components");
    let pad = &components[0].pads.as_ref().expect("pads")[0];
    assert_eq!(pad.copperClearance, Some(0.15));
}

#[test]
fn a_pad_without_its_own_clearance_falls_back_to_the_footprint_clearance() {
    let text = format!(
        r#"(kicad_pcb (version 20241229) {LAYERS} (net 1 "GND")
        (gr_rect (start 0 0) (end 10 10) (layer "Edge.Cuts"))
        (footprint "R" (layer "F.Cu") (at 1 1) (clearance 0.3)
          (pad "1" smd rect (at 0 0) (size 1 1) (layers "F.Cu") (net 1 "GND"))))"#
    );
    let imported = read_pcb(&text, "t", &default_net_class()).expect("the board imports");
    let components = imported.board.components.expect("components");
    let pad = &components[0].pads.as_ref().expect("pads")[0];
    assert_eq!(pad.copperClearance, Some(0.3));
}

#[test]
fn a_pad_and_footprint_with_no_clearance_node_carries_none() {
    let text = format!(
        r#"(kicad_pcb (version 20241229) {LAYERS} (net 1 "GND")
        (gr_rect (start 0 0) (end 10 10) (layer "Edge.Cuts"))
        {})"#,
        one_pad("R", 1.0, 1.0)
    );
    let imported = read_pcb(&text, "t", &default_net_class()).expect("the board imports");
    let components = imported.board.components.expect("components");
    let pad = &components[0].pads.as_ref().expect("pads")[0];
    assert_eq!(pad.copperClearance, None);
}

#[test]
fn allow_soldermask_bridges_in_footprints_reads_the_board_setup() {
    let text = format!(
        r#"(kicad_pcb (version 20241229) {LAYERS} (net 1 "GND")
        (setup (allow_soldermask_bridges_in_footprints yes))
        (gr_rect (start 0 0) (end 10 10) (layer "Edge.Cuts"))
        {})"#,
        one_pad("R", 1.0, 1.0)
    );
    let imported = read_pcb(&text, "t", &default_net_class()).expect("the board imports");
    assert!(imported.board.allowSolderMaskBridgesInFootprints);
}

#[test]
fn allow_soldermask_bridges_in_footprints_is_false_when_setup_omits_it() {
    let text = format!(
        r#"(kicad_pcb (version 20241229) {LAYERS} (net 1 "GND")
        (setup (pad_to_mask_clearance 0.05))
        (gr_rect (start 0 0) (end 10 10) (layer "Edge.Cuts"))
        {})"#,
        one_pad("R", 1.0, 1.0)
    );
    let imported = read_pcb(&text, "t", &default_net_class()).expect("the board imports");
    assert!(!imported.board.allowSolderMaskBridgesInFootprints);
}

#[test]
fn allow_soldermask_bridges_in_footprints_defaults_to_false_without_setup() {
    let text = format!(
        r#"(kicad_pcb (version 20241229) {LAYERS} (net 1 "GND")
        (gr_rect (start 0 0) (end 10 10) (layer "Edge.Cuts"))
        {})"#,
        one_pad("R", 1.0, 1.0)
    );
    let imported = read_pcb(&text, "t", &default_net_class()).expect("the board imports");
    assert!(!imported.board.allowSolderMaskBridgesInFootprints);
}

#[test]
fn solder_mask_min_width_is_read_when_the_setup_node_is_present() {
    let text = format!(
        r#"(kicad_pcb (version 20241229) {LAYERS} (net 1 "GND")
        (setup (solder_mask_min_width 0.1))
        (gr_rect (start 0 0) (end 10 10) (layer "Edge.Cuts"))
        {})"#,
        one_pad("R", 1.0, 1.0)
    );
    let imported = read_pcb(&text, "t", &default_net_class()).expect("the board imports");
    assert_eq!(imported.board.solderMaskMinWidth, Some(0.1));
}

/// An explicit `0` and an absent `solder_mask_min_width` node are different values, so a present
/// node carrying `0` must still round-trip as `Some(0.0)`, not fall through to `None`.
#[test]
fn solder_mask_min_width_distinguishes_an_explicit_zero_from_an_absent_node() {
    let text = format!(
        r#"(kicad_pcb (version 20241229) {LAYERS} (net 1 "GND")
        (setup (solder_mask_min_width 0))
        (gr_rect (start 0 0) (end 10 10) (layer "Edge.Cuts"))
        {})"#,
        one_pad("R", 1.0, 1.0)
    );
    let imported = read_pcb(&text, "t", &default_net_class()).expect("the board imports");
    assert_eq!(imported.board.solderMaskMinWidth, Some(0.0));
}

#[test]
fn solder_mask_min_width_is_absent_when_setup_omits_it() {
    let text = format!(
        r#"(kicad_pcb (version 20241229) {LAYERS} (net 1 "GND")
        (setup (pad_to_mask_clearance 0.05))
        (gr_rect (start 0 0) (end 10 10) (layer "Edge.Cuts"))
        {})"#,
        one_pad("R", 1.0, 1.0)
    );
    let imported = read_pcb(&text, "t", &default_net_class()).expect("the board imports");
    assert_eq!(imported.board.solderMaskMinWidth, None);
}

#[test]
fn solder_mask_min_width_is_absent_without_a_setup_node() {
    let text = format!(
        r#"(kicad_pcb (version 20241229) {LAYERS} (net 1 "GND")
        (gr_rect (start 0 0) (end 10 10) (layer "Edge.Cuts"))
        {})"#,
        one_pad("R", 1.0, 1.0)
    );
    let imported = read_pcb(&text, "t", &default_net_class()).expect("the board imports");
    assert_eq!(imported.board.solderMaskMinWidth, None);
}

#[test]
fn it_reports_the_count_of_imported_embedded_net_classes() {
    let text = format!(
        r#"(kicad_pcb (version 20241229) {LAYERS} (net 1 "GND")
        (net_class "Custom" (clearance 0.2) (trace_width 0.3) (via_dia 0.6) (via_drill 0.3))
        (gr_rect (start 0 0) (end 10 10) (layer "Edge.Cuts"))
        (footprint "R" (layer "F.Cu") (at 1 1)
          (pad "1" smd rect (at 0 0) (size 1 1) (layers "F.Cu") (net 1 "GND"))))"#
    );
    let imported = read_pcb(&text, "t", &default_net_class()).expect("the board imports");
    let expected = "Imported 1 embedded KiCad net classes, including trace widths, clearances \
                     and via dimensions.";
    assert!(
        imported.warnings.iter().any(|warning| warning == expected),
        "got {:?}",
        imported.warnings
    );
}

#[test]
fn a_length_tuning_generator_on_copper_contributes_no_obstacle() {
    let text = format!(
        r#"(kicad_pcb (version 20241229) {LAYERS} (net 1 "GND")
        (gr_rect (start 0 0) (end 10 10) (layer "Edge.Cuts"))
        (footprint "R" (layer "F.Cu") (at 1 1)
          (pad "1" smd rect (at 0 0) (size 1 1) (layers "F.Cu") (net 1 "GND")))
        (generated (uuid "u") (type tuning_pattern) (layer "F.Cu")
          (base_line (pts (xy 2 2) (xy 8 2)))
          (members "a" "b")))"#
    );
    let imported = read_pcb(&text, "t", &default_net_class()).expect("the board imports");
    assert_eq!(
        imported
            .board
            .conductionAreas
            .as_deref()
            .unwrap_or(&[])
            .len(),
        0,
        "the generator's copper is its member segments, which are read on their own"
    );
}

#[test]
fn a_non_plated_slot_is_imported_with_the_narrow_slot_dimension_as_its_drill() {
    let text = format!(
        r#"(kicad_pcb (version 20241229) {LAYERS}
        (gr_rect (start 0 0) (end 10 10) (layer "Edge.Cuts"))
        (footprint "H" (layer "F.Cu") (at 1 1)
          (pad "" np_thru_hole oval (at 0 0) (size 1.6 1.9) (drill oval 0.8 1.5)
            (layers "F.Cu" "B.Cu"))))"#
    );
    let imported = read_pcb(&text, "t", &default_net_class()).expect("the board imports");
    let components = imported.board.components.expect("components");
    let pad = components
        .iter()
        .flat_map(|component| component.pads.as_deref().unwrap_or(&[]))
        .find(|pad| pad.nonPlated)
        .expect("the slot pad is imported");
    assert_eq!(pad.drill, 0.8);
}

#[test]
fn a_slot_with_a_zero_dimension_is_still_rejected() {
    let text = format!(
        r#"(kicad_pcb (version 20241229) {LAYERS}
        (gr_rect (start 0 0) (end 10 10) (layer "Edge.Cuts"))
        (footprint "H" (layer "F.Cu") (at 1 1)
          (pad "" np_thru_hole oval (at 0 0) (size 1.6 1.9) (drill oval 0 1.5)
            (layers "F.Cu" "B.Cu"))))"#
    );
    let error = read_pcb(&text, "t", &default_net_class()).expect_err("it fails");
    assert_eq!(
        error.message,
        "Only slots with positive dimensions are supported."
    );
}

/// Corners cross-checked against `PAD::GetEffectivePolygon` in KiCad 10.0.3 for
/// size (2, 1) with rect_delta (0.3, -0.4), an asymmetric case that pins both signs.
#[test]
fn a_trapezoid_pad_matches_the_corners_kicad_builds() {
    let text = format!(
        r#"(kicad_pcb (version 20241229) {LAYERS} (net 1 "GND")
        (gr_rect (start 0 0) (end 10 10) (layer "Edge.Cuts"))
        (footprint "R" (layer "F.Cu") (at 1 1)
          (pad "1" smd trapezoid (at 0 0) (size 2 1) (rect_delta 0.3 -0.4)
            (layers "F.Cu") (net 1 "GND"))))"#
    );
    let imported = read_pcb(&text, "t", &default_net_class()).expect("the board imports");
    let components = imported.board.components.expect("components");
    let pad = &components[0].pads.as_ref().expect("pads")[0];
    let polygon = pad.copperPolygon.as_ref().expect("a trapezoid carries its polygon");
    let corners: Vec<(f64, f64)> = polygon.iter().map(|p| (p.x, p.y)).collect();
    assert_eq!(
        corners,
        vec![(-0.8, 0.65), (-1.2, -0.65), (1.2, -0.35), (0.8, 0.35)]
    );
}

#[test]
fn a_trapezoid_without_a_delta_is_the_plain_rectangle() {
    let text = format!(
        r#"(kicad_pcb (version 20241229) {LAYERS} (net 1 "GND")
        (gr_rect (start 0 0) (end 10 10) (layer "Edge.Cuts"))
        (footprint "R" (layer "F.Cu") (at 1 1)
          (pad "1" smd trapezoid (at 0 0) (size 2 1) (layers "F.Cu") (net 1 "GND"))))"#
    );
    let imported = read_pcb(&text, "t", &default_net_class()).expect("the board imports");
    let components = imported.board.components.expect("components");
    let pad = &components[0].pads.as_ref().expect("pads")[0];
    let polygon = pad.copperPolygon.as_ref().expect("a trapezoid carries its polygon");
    let corners: Vec<(f64, f64)> = polygon.iter().map(|p| (p.x, p.y)).collect();
    assert_eq!(
        corners,
        vec![(-1.0, 0.5), (-1.0, -0.5), (1.0, -0.5), (1.0, 0.5)]
    );
}

#[test]
fn a_trapezoid_delta_wider_than_the_pad_is_rejected() {
    let text = format!(
        r#"(kicad_pcb (version 20241229) {LAYERS} (net 1 "GND")
        (gr_rect (start 0 0) (end 10 10) (layer "Edge.Cuts"))
        (footprint "R" (layer "F.Cu") (at 1 1)
          (pad "1" smd trapezoid (at 0 0) (size 2 1) (rect_delta 1.4 0)
            (layers "F.Cu") (net 1 "GND"))))"#
    );
    let error = read_pcb(&text, "t", &default_net_class()).expect_err("it fails");
    assert_eq!(
        error.message,
        "Trapezoid pads must keep a positive width and height."
    );
}

fn only_pad_polygon(text: &str) -> Vec<(f64, f64)> {
    let imported = read_pcb(text, "t", &default_net_class()).expect("the board imports");
    let components = imported.board.components.expect("components");
    components[0].pads.as_ref().expect("pads")[0]
        .copperPolygon
        .as_ref()
        .expect("a custom pad carries its polygon")
        .iter()
        .map(|p| (p.x, p.y))
        .collect()
}

fn contains(polygon: &[(f64, f64)], p: (f64, f64)) -> bool {
    let side = |a: (f64, f64), b: (f64, f64)| (b.0 - a.0) * (p.1 - a.1) - (b.1 - a.1) * (p.0 - a.0);
    let sides: Vec<f64> = polygon
        .iter()
        .enumerate()
        .map(|(i, &a)| side(a, polygon[(i + 1) % polygon.len()]))
        .filter(|value| *value != 0.0)
        .collect();
    sides.windows(2).all(|pair| pair[0] * pair[1] > 0.0)
}

fn bounds(polygon: &[(f64, f64)]) -> (f64, f64, f64, f64) {
    polygon.iter().fold(
        (f64::MAX, f64::MAX, f64::MIN, f64::MIN),
        |(x0, y0, x1, y1), &(x, y)| (x0.min(x), y0.min(y), x1.max(x), y1.max(y)),
    )
}

/// The shape that dominates the corpus: a rectangular anchor with a polygon and two circles
/// forming a stadium. The union spans x [-0.5, 0.55] and y [-0.75, 0.75]; circles are sampled
/// circumscribed, so the hull may exceed that by up to the 0.005 mm outline tolerance.
#[test]
fn a_stadium_custom_pad_covers_its_polygon_and_both_circles() {
    let text = format!(
        r#"(kicad_pcb (version 20241229) {LAYERS} (net 1 "GND")
        (gr_rect (start 0 0) (end 10 10) (layer "Edge.Cuts"))
        (footprint "R" (layer "F.Cu") (at 1 1)
          (pad "1" smd custom (at 0 0) (size 1 0.5) (layers "F.Cu") (net 1 "GND")
            (options (clearance outline) (anchor rect))
            (primitives
              (gr_poly (pts (xy 0.55 0.75) (xy 0 0.75) (xy 0 -0.75) (xy 0.55 -0.75)) (width 0))
              (gr_circle (center 0 -0.25) (end 0.5 -0.25) (width 0))
              (gr_circle (center 0 0.25) (end 0.5 0.25) (width 0))))))"#
    );
    let polygon = only_pad_polygon(&text);
    assert!(
        contains(&polygon, (-0.45, 0.45)),
        "only the circles reach the upper left; without them the hull runs straight from the \
         anchor corner (-0.5, 0.25) to the polygon corner (0, 0.75), which cuts this point off"
    );
    let (x0, y0, x1, y1) = bounds(&polygon);
    assert!((-0.505..=-0.5).contains(&x0), "left edge was {x0}");
    assert_eq!(x1, 0.55, "the polygon's right edge is exact");
    assert!((-0.755..=-0.75).contains(&y0), "bottom edge was {y0}");
    assert!((0.75..=0.755).contains(&y1), "top edge was {y1}");
}

/// With no primitive reaching it, the circular anchor alone sets the pad copper, and KiCad
/// takes its diameter from size.x while ignoring size.y.
#[test]
fn a_circular_anchor_takes_its_diameter_from_the_x_size_alone() {
    let text = format!(
        r#"(kicad_pcb (version 20241229) {LAYERS} (net 1 "GND")
        (gr_rect (start 0 0) (end 10 10) (layer "Edge.Cuts"))
        (footprint "R" (layer "F.Cu") (at 1 1)
          (pad "1" smd custom (at 0 0) (size 1.2 0.4) (layers "F.Cu") (net 1 "GND")
            (options (clearance outline) (anchor circle))
            (primitives (gr_line (start -0.1 0) (end 0.1 0) (width 0.1))))))"#
    );
    let (x0, y0, x1, y1) = bounds(&only_pad_polygon(&text));
    for edge in [x0.abs(), y0.abs(), x1, y1] {
        assert!(
            (0.6..=0.605).contains(&edge),
            "every edge should sit at the 0.6 anchor radius, got {edge}"
        );
    }
}

#[test]
fn a_multi_primitive_pad_warns_that_its_hull_can_overstate_the_copper() {
    let hull_warning = "Custom pads that are not a single convex outline are reserved as their \
                        convex hull, which can overstate their copper; original pad definitions \
                        are preserved.";
    let board = |primitives: &str| {
        format!(
            r#"(kicad_pcb (version 20241229) {LAYERS} (net 1 "GND")
            (gr_rect (start 0 0) (end 10 10) (layer "Edge.Cuts"))
            (footprint "R" (layer "F.Cu") (at 1 1)
              (pad "1" smd custom (at 0 0) (size 0.4 0.4) (layers "F.Cu") (net 1 "GND")
                (options (clearance outline) (anchor circle))
                (primitives {primitives}))))"#
        )
    };
    let convex = board(
        r#"(gr_poly (pts (xy -0.5 -0.5) (xy 0.5 -0.5) (xy 0.5 0.5) (xy -0.5 0.5)) (width 0) (fill yes))"#,
    );
    let imported = read_pcb(&convex, "t", &default_net_class()).expect("the board imports");
    assert!(
        !imported.warnings.iter().any(|w| w == hull_warning),
        "a single convex filled outline is reserved exactly"
    );

    let pair = board(
        r#"(gr_line (start -1 0) (end 1 0) (width 0.3)) (gr_line (start 0 -1) (end 0 1) (width 0.3))"#,
    );
    let imported = read_pcb(&pair, "t", &default_net_class()).expect("the board imports");
    assert!(
        imported.warnings.iter().any(|w| w == hull_warning),
        "a cross of two strokes is not convex, so its hull overstates the copper"
    );
}

#[test]
fn an_unsupported_custom_pad_anchor_and_primitive_are_named_in_the_error() {
    let board = |anchor: &str, primitive: &str| {
        format!(
            r#"(kicad_pcb (version 20241229) {LAYERS} (net 1 "GND")
            (gr_rect (start 0 0) (end 10 10) (layer "Edge.Cuts"))
            (footprint "R" (layer "F.Cu") (at 1 1)
              (pad "1" smd custom (at 0 0) (size 0.4 0.4) (layers "F.Cu") (net 1 "GND")
                (options (clearance outline) (anchor {anchor}))
                (primitives {primitive}))))"#
        )
    };
    let error = read_pcb(
        &board("oval", r#"(gr_line (start 0 0) (end 1 0) (width 0.1))"#),
        "t",
        &default_net_class(),
    )
    .expect_err("it fails");
    assert_eq!(error.message, "Unsupported custom pad anchor: oval");

    let error = read_pcb(
        &board("circle", r#"(gr_curve (width 0.1))"#),
        "t",
        &default_net_class(),
    )
    .expect_err("it fails");
    assert_eq!(error.message, "Unsupported custom pad primitive: gr_curve");
}

/// A gap of exactly the chaining epsilon closes in KiCad, whose coordinates are integer
/// nanometres. In millimetre floats `92.79 - 92.78` is `0.010000000000005116`, so comparing
/// there would reject this outline however the limit is written.
#[test]
fn an_outline_gap_of_exactly_the_chaining_epsilon_closes() {
    let text = format!(
        r#"(kicad_pcb (version 20241229) {LAYERS} (net 1 "GND")
        (gr_line (start 100 92.79) (end 110 92.79) (layer "Edge.Cuts"))
        (gr_line (start 110 92.79) (end 110 102.79) (layer "Edge.Cuts"))
        (gr_line (start 110 102.79) (end 100 102.79) (layer "Edge.Cuts"))
        (gr_line (start 100 102.79) (end 100 92.78) (layer "Edge.Cuts"))
        (footprint "R" (layer "F.Cu") (at 105 97)
          (pad "1" smd rect (at 0 0) (size 1 1) (layers "F.Cu") (net 1 "GND"))))"#
    );
    let imported = read_pcb(&text, "t", &default_net_class()).expect("the board imports");
    assert!(imported.board.outline.is_some());
}

fn square_with_gap(gap: f64) -> String {
    let bottom = 92.79 - gap;
    format!(
        r#"(kicad_pcb (version 20241229) {LAYERS} (net 1 "GND")
        (gr_line (start 100 92.79) (end 110 92.79) (layer "Edge.Cuts"))
        (gr_line (start 110 92.79) (end 110 102.79) (layer "Edge.Cuts"))
        (gr_line (start 110 102.79) (end 100 102.79) (layer "Edge.Cuts"))
        (gr_line (start 100 102.79) (end 100 {bottom}) (layer "Edge.Cuts"))
        (footprint "R" (layer "F.Cu") (at 105 97)
          (pad "1" smd rect (at 0 0) (size 1 1) (layers "F.Cu") (net 1 "GND"))))"#
    )
}

/// KiCad calls a board with a gap this wide malformed. Bridging it only supplies the router
/// with a boundary, so the warning has to say where the gap was and that nothing was altered.
#[test]
fn a_gap_past_the_chaining_epsilon_is_bridged_and_reported() {
    let imported =
        read_pcb(&square_with_gap(0.02), "t", &default_net_class()).expect("the board imports");
    assert!(imported.board.outline.is_some());
    let bridged = imported
        .warnings
        .iter()
        .find(|warning| warning.contains("bridged"))
        .expect("the bridge is reported");
    assert!(
        bridged.contains("1 Edge.Cuts gap was bridged")
            && bridged.contains("0.0200 mm at (100.0000, 92.7700)")
            && bridged.contains("downloads is unchanged"),
        "got {bridged}"
    );
}

#[test]
fn a_gap_wider_than_the_healing_tolerance_is_still_refused() {
    let error =
        read_pcb(&square_with_gap(1.5), "t", &default_net_class()).expect_err("it fails");
    assert_eq!(error.message, "A closed Edge.Cuts outline is required.");
}
