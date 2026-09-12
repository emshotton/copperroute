use copper_dsn::kicad::sexpr::parse;

#[test]
fn it_reads_nested_nodes_and_atoms() {
    let node = parse("(kicad_pcb (version 20241229) (generator \"pcbnew\"))").expect("it parses");
    assert_eq!(node.name(), "kicad_pcb");
    assert_eq!(node.value("version"), Some("20241229"));
    assert_eq!(node.value("generator"), Some("pcbnew"));
}

#[test]
fn it_unescapes_quoted_strings() {
    let node = parse(r#"(net 1 "Net-(J1\\CC1)")"#).expect("it parses");
    assert_eq!(node.atom(2), Some(r"Net-(J1\CC1)"));
}

#[test]
fn it_collects_repeated_children() {
    let node = parse("(pcb (net 1 A) (net 2 B) (net 3 C))").expect("it parses");
    let names: Vec<&str> = node.children("net").filter_map(|n| n.atom(2)).collect();
    assert_eq!(names, ["A", "B", "C"]);
}

#[test]
fn it_reads_numbers_and_points() {
    let node = parse("(pad (at 1.5 -2.25) (size 0.8 0.95))").expect("it parses");
    assert_eq!(node.number("at"), Some(1.5));
    assert_eq!(node.point("size"), Some((0.8, 0.95)));
    assert_eq!(node.point("at"), Some((1.5, -2.25)));
}

#[test]
fn it_rejects_an_unclosed_expression() {
    let error = parse("(kicad_pcb (version 3)").expect_err("it fails");
    assert_eq!(error.to_string(), "Unclosed expression.");
}

#[test]
fn it_rejects_a_non_expression() {
    let error = parse("kicad_pcb").expect_err("it fails");
    assert_eq!(error.to_string(), "Expected a KiCad S-expression.");
}

#[test]
fn it_rejects_excessive_nesting() {
    let deep = format!("{}{}", "(a ".repeat(120), ")".repeat(120));
    let error = parse(&deep).expect_err("it fails");
    assert_eq!(error.to_string(), "File nesting is too deep.");
}

#[test]
fn it_keeps_source_spans() {
    let text = "(pcb (net 1 A))";
    let node = parse(text).expect("it parses");
    let net = node.child("net").expect("a net");
    assert_eq!(&text[net.start..net.end], "(net 1 A)");
}

#[test]
fn it_rejects_an_unclosed_string() {
    let error = parse("(a \"abc").expect_err("it fails");
    assert_eq!(error.to_string(), "Unclosed string.");
}

#[test]
fn it_rejects_a_trailing_escape() {
    let error = parse("(a \"abc\\").expect_err("it fails");
    assert_eq!(error.to_string(), "Unclosed string.");
}
