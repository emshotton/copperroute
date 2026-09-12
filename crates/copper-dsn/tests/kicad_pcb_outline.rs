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
    let outline = assemble_outline(&paths, &mut Vec::new()).expect("an outline");
    assert!(outline.cutouts.is_empty());
    let xs: Vec<f64> = outline.boundary.iter().map(|p| p.0).collect();
    assert_eq!(xs.iter().cloned().fold(f64::MIN, f64::max), 40.0);
}

#[test]
fn it_ignores_an_alignment_target_on_edge_cuts() {
    let text = board(concat!(
        r#"(gr_rect (start 0 0) (end 40 30) (layer "Edge.Cuts"))"#,
        r#"(target plus (at 20 15) (size 5) (width 0.1) (layer "Edge.Cuts"))"#,
    ));
    let root = parse(&text).expect("it parses");
    let paths = outline_paths(&root).expect("outline paths");
    let outline = assemble_outline(&paths, &mut Vec::new()).expect("an outline");
    assert_eq!(outline.boundary.len(), 4);
    assert!(outline.cutouts.is_empty());
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
    let outline = assemble_outline(&outline_paths(&root).expect("paths"), &mut Vec::new())
        .expect("an outline");
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
    let outline = assemble_outline(&outline_paths(&root).expect("paths"), &mut Vec::new())
        .expect("an outline");
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
    let error = assemble_outline(&outline_paths(&root).expect("paths"), &mut Vec::new())
        .expect_err("it fails");
    assert_eq!(error.message, "A closed Edge.Cuts outline is required.");
}

#[test]
fn it_rejects_a_board_with_no_outline() {
    let root = parse(&board("")).expect("it parses");
    let error = assemble_outline(&outline_paths(&root).expect("paths"), &mut Vec::new())
        .expect_err("it fails");
    assert_eq!(error.message, "A closed Edge.Cuts outline is required.");
}

#[test]
fn it_rejects_an_outline_with_no_closed_loop_at_all() {
    let text = board(concat!(
        r#"(gr_line (start 0 0) (end 10 0) (layer "Edge.Cuts"))"#,
        r#"(gr_line (start 50 50) (end 60 60) (layer "Edge.Cuts"))"#,
    ));
    let root = parse(&text).expect("it parses");
    let error = assemble_outline(&outline_paths(&root).expect("paths"), &mut Vec::new())
        .expect_err("it fails");
    assert_eq!(error.message, "A closed Edge.Cuts outline is required.");
}

#[test]
fn it_assembles_a_loop_and_warns_about_an_orphan_fragment() {
    let text = board(concat!(
        r#"(gr_rect (start 0 0) (end 40 30) (layer "Edge.Cuts"))"#,
        r#"(gr_line (start 100 100) (end 100.07 100) (layer "Edge.Cuts"))"#,
    ));
    let root = parse(&text).expect("it parses");
    let mut warnings = Vec::new();
    let outline =
        assemble_outline(&outline_paths(&root).expect("paths"), &mut warnings).expect("an outline");
    assert_eq!(outline.boundary.len(), 4);
    assert!(outline.cutouts.is_empty());
    assert_eq!(
        warnings,
        vec!["1 Edge.Cuts edges could not be closed into a loop and were ignored.".to_string()]
    );
}

#[test]
fn it_rejects_a_footprint_with_an_invalid_angle() {
    let text = board(concat!(
        r#"(footprint "test" (layer "F.Cu") (at 0 0 garbage)"#,
        r#"(fp_line (start 0 0) (end 10 0) (layer "Edge.Cuts")))"#,
    ));
    let root = parse(&text).expect("it parses");
    let error = outline_paths(&root).expect_err("it fails");
    assert_eq!(error.message, "Invalid outline coordinate.");
}

#[test]
fn it_accepts_a_footprint_with_no_angle() {
    let text = board(concat!(
        r#"(footprint "test" (layer "F.Cu") (at 0 0)"#,
        r#"(fp_line (start 0 0) (end 10 0) (layer "Edge.Cuts")))"#,
    ));
    let root = parse(&text).expect("it parses");
    let paths = outline_paths(&root).expect("outline paths");
    assert_eq!(paths.paths.len(), 1);
}

#[test]
fn equal_area_cutouts_sort_by_geometry_not_by_declaration_order() {
    let declared_a_then_b = board(concat!(
        r#"(gr_rect (start 10 10) (end 20 20) (layer "Edge.Cuts"))"#,
        r#"(gr_rect (start 25 10) (end 35 20) (layer "Edge.Cuts"))"#,
        r#"(gr_rect (start 0 0) (end 100 100) (layer "Edge.Cuts"))"#,
    ));
    let declared_b_then_a = board(concat!(
        r#"(gr_rect (start 25 10) (end 35 20) (layer "Edge.Cuts"))"#,
        r#"(gr_rect (start 10 10) (end 20 20) (layer "Edge.Cuts"))"#,
        r#"(gr_rect (start 0 0) (end 100 100) (layer "Edge.Cuts"))"#,
    ));
    let outline_of = |text: &str| {
        let root = parse(text).expect("it parses");
        assemble_outline(&outline_paths(&root).expect("paths"), &mut Vec::new())
            .expect("an outline")
    };
    let first = outline_of(&declared_a_then_b);
    let second = outline_of(&declared_b_then_a);
    assert_eq!(first.cutouts.len(), 2);
    assert_eq!(
        first.cutouts, second.cutouts,
        "two equal-area cutouts must land in the same order regardless of which one the file \
         declares first"
    );
    let leftmost_x: Vec<f64> = first.cutouts[0].iter().map(|p| p.0).collect();
    assert_eq!(leftmost_x.iter().cloned().fold(f64::MAX, f64::min), 10.0);
}

fn rounded_square_with_gap(gap: f64) -> String {
    board(&format!(
        concat!(
            r#"(gr_line (start 0 0) (end 10 0) (layer "Edge.Cuts"))"#,
            r#"(gr_line (start 10 0) (end 10 10) (layer "Edge.Cuts"))"#,
            r#"(gr_line (start 10 10) (end 0 10) (layer "Edge.Cuts"))"#,
            r#"(gr_line (start 0 {gap}) (end 0 0) (layer "Edge.Cuts"))"#,
        ),
        gap = 10.0 + gap
    ))
}

#[test]
fn it_chains_endpoints_just_under_kicads_chaining_tolerance() {
    let text = rounded_square_with_gap(0.0099);
    let root = parse(&text).expect("it parses");
    let outline = assemble_outline(&outline_paths(&root).expect("paths"), &mut Vec::new())
        .expect("an outline");
    assert_eq!(outline.boundary.len(), 4);
}

#[test]
fn it_bridges_endpoints_just_over_kicads_chaining_tolerance() {
    let text = rounded_square_with_gap(0.0101);
    let root = parse(&text).expect("it parses");
    let mut warnings = Vec::new();
    let outline =
        assemble_outline(&outline_paths(&root).expect("paths"), &mut warnings).expect("an outline");
    assert_eq!(outline.boundary.len(), 4);
    assert!(
        warnings.iter().any(|warning| warning.contains("bridged")),
        "the bridge has to be reported: {warnings:?}"
    );
}

#[test]
fn it_rejects_endpoints_past_the_healing_tolerance() {
    let text = rounded_square_with_gap(1.5);
    let root = parse(&text).expect("it parses");
    let error = assemble_outline(&outline_paths(&root).expect("paths"), &mut Vec::new())
        .expect_err("it fails");
    assert_eq!(error.message, "A closed Edge.Cuts outline is required.");
}

#[test]
fn it_rejects_separate_outlines() {
    let text = board(concat!(
        r#"(gr_rect (start 0 0) (end 10 10) (layer "Edge.Cuts"))"#,
        r#"(gr_rect (start 50 50) (end 60 60) (layer "Edge.Cuts"))"#,
    ));
    let root = parse(&text).expect("it parses");
    let error = assemble_outline(&outline_paths(&root).expect("paths"), &mut Vec::new())
        .expect_err("it fails");
    assert_eq!(
        error.message,
        "Separate board outlines are not supported yet."
    );
}
