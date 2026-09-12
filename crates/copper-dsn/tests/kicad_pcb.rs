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
