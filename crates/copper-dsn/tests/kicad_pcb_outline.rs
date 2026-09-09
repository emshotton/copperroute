use copper_dsn::kicad::pcb::outline::{OUTLINE_TOLERANCE, assemble_outline, outline_paths};
use copper_dsn::kicad::sexpr::parse;

fn board(body: &str) -> String {
    format!("(kicad_pcb (version 20241229) {body})")
}

#[test]
fn it_assembles_a_rectangular_outline() {
    let text = board(r#"(gr_rect (start 0 0) (end 40 30) (layer "Edge.Cuts"))"#);
    let root = parse(&text).expect("it parses");
    let paths = outline_paths(&root).expect("outline paths");
    assert!(!paths.curved);
    let outline = assemble_outline(&paths).expect("an outline");
    assert!(outline.cutouts.is_empty());
    let xs: Vec<f64> = outline.boundary.iter().map(|p| p.0).collect();
    assert_eq!(xs.iter().cloned().fold(f64::MIN, f64::max), 40.0);
}

#[test]
fn it_assembles_four_lines_into_one_loop() {
    let text = board(concat!(
        r#"(gr_line (start 0 0) (end 10 0) (layer "Edge.Cuts"))"#,
        r#"(gr_line (start 10 0) (end 10 10) (layer "Edge.Cuts"))"#,
        r#"(gr_line (start 10 10) (end 0 10) (layer "Edge.Cuts"))"#,
        r#"(gr_line (start 0 10) (end 0 0) (layer "Edge.Cuts"))"#,
    ));
    let root = parse(&text).expect("it parses");
    let outline = assemble_outline(&outline_paths(&root).expect("paths")).expect("an outline");
    assert_eq!(outline.boundary.len(), 4);
    assert!(outline.cutouts.is_empty());
}

#[test]
fn it_treats_the_smaller_loop_as_a_cutout() {
    let text = board(concat!(
        r#"(gr_rect (start 10 10) (end 20 20) (layer "Edge.Cuts"))"#,
        r#"(gr_rect (start 0 0) (end 40 30) (layer "Edge.Cuts"))"#,
    ));
    let root = parse(&text).expect("it parses");
    let outline = assemble_outline(&outline_paths(&root).expect("paths")).expect("an outline");
    assert_eq!(outline.cutouts.len(), 1);
    let cutout_xs: Vec<f64> = outline.cutouts[0].iter().map(|p| p.0).collect();
    assert_eq!(cutout_xs.iter().cloned().fold(f64::MIN, f64::max), 20.0);
}

#[test]
fn it_marks_an_arc_outline_curved() {
    let text = board(r#"(gr_arc (start 0 0) (mid 5 5) (end 10 0) (layer "Edge.Cuts"))"#);
    let root = parse(&text).expect("it parses");
    let paths = outline_paths(&root).expect("paths");
    assert!(paths.curved);
    let sampled = &paths.paths[0];
    let radius = 5.0_f64;
    for point in sampled {
        let error = ((point.0 - 5.0).powi(2) + point.1.powi(2)).sqrt() - radius;
        assert!(
            error.abs() <= OUTLINE_TOLERANCE * 2.0,
            "{error} off the arc"
        );
    }
}

#[test]
fn it_rejects_an_open_outline() {
    let text = board(r#"(gr_line (start 0 0) (end 10 0) (layer "Edge.Cuts"))"#);
    let root = parse(&text).expect("it parses");
    let error = assemble_outline(&outline_paths(&root).expect("paths")).expect_err("it fails");
    assert_eq!(error.message, "The Edge.Cuts outline is not closed.");
}

#[test]
fn it_rejects_a_board_with_no_outline() {
    let root = parse(&board("")).expect("it parses");
    let error = assemble_outline(&outline_paths(&root).expect("paths")).expect_err("it fails");
    assert_eq!(error.message, "A closed Edge.Cuts outline is required.");
}

#[test]
fn it_rejects_separate_outlines() {
    let text = board(concat!(
        r#"(gr_rect (start 0 0) (end 10 10) (layer "Edge.Cuts"))"#,
        r#"(gr_rect (start 50 50) (end 60 60) (layer "Edge.Cuts"))"#,
    ));
    let root = parse(&text).expect("it parses");
    let error = assemble_outline(&outline_paths(&root).expect("paths")).expect_err("it fails");
    assert_eq!(
        error.message,
        "Separate board outlines are not supported yet."
    );
}
