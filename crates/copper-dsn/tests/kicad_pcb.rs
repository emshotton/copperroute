use copper_dsn::error::BoardReadResult;
use copper_dsn::kicad::pcb::{default_net_class, read_pcb};
use copper_dsn::kicad::{read_board_json, NetClassJson};

fn defaults() -> NetClassJson {
    NetClassJson {
        name: Some("Default".to_string()),
        clearance: 0.2,
        traceWidth: 0.25,
        viaDiameter: 0.6,
        viaDrill: 0.3,
        ..Default::default()
    }
}

fn example() -> String {
    let path = testkit::workspace_root().join("web/example.kicad_pcb");
    std::fs::read_to_string(&path).expect("the example board reads")
}

const LAYERS: &str = r#"(layers (0 "F.Cu" signal) (2 "B.Cu" signal))"#;

#[test]
fn it_imports_the_example_board() {
    let imported = read_pcb(&example(), &defaults()).expect("the board imports");
    assert!(!imported.board.components.as_ref().expect("components").is_empty());
    assert!(imported.board.outline.is_some());
    assert!(!imported.board.nets.as_ref().expect("nets").is_empty());
}

#[test]
fn the_imported_board_loads_through_the_reader() {
    let imported = read_pcb(&example(), &defaults()).expect("the board imports");
    match read_board_json(imported.board, None) {
        BoardReadResult::Success { board: Some(board), .. } => {
            assert!(board.get_layer_count() >= 1);
        }
        other => panic!("expected a loaded board, got {other:?}"),
    }
}

#[test]
fn it_rejects_a_file_that_is_not_a_board() {
    let error = read_pcb("(kicad_sch (version 1))", &defaults()).expect_err("it fails");
    assert_eq!(error.message, "Expected one kicad_pcb board.");
}

#[test]
fn it_rejects_a_board_with_no_pads() {
    let text = r#"(kicad_pcb (version 20241229) (layers (0 "F.Cu" signal))
        (gr_rect (start 0 0) (end 10 10) (layer "Edge.Cuts")))"#;
    let error = read_pcb(text, &defaults()).expect_err("it fails");
    assert_eq!(error.message, "No pads were found.");
}

#[test]
fn it_rejects_curved_tracks() {
    let text = r#"(kicad_pcb (version 20241229) (layers (0 "F.Cu" signal)) (net 1 "GND")
        (gr_rect (start 0 0) (end 10 10) (layer "Edge.Cuts"))
        (footprint "R" (layer "F.Cu") (at 1 1)
          (pad "1" smd rect (at 0 0) (size 1 1) (layers "F.Cu") (net 1 "GND")))
        (arc (start 1 1) (mid 2 2) (end 3 3) (width 0.25) (layer "F.Cu") (net 1)))"#;
    let error = read_pcb(text, &defaults()).expect_err("it fails");
    assert_eq!(error.message, "Curved tracks are not supported yet.");
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
    let imported = read_pcb(&text, &defaults()).expect("the board imports");
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
    assert_eq!(imported.board.conductionAreas.as_ref().expect("areas").len(), 2);
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
    let imported = read_pcb(&text, &defaults).expect("the board imports");
    let classes = imported.board.netClasses.expect("net classes");
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].name.as_deref(), Some("Default"));
    assert_eq!(classes[0].clearance, defaults.clearance);
}
