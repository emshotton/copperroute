use copper_dsn::kicad::pcb::structure::{Layers, NetTable, read_net_classes};
use copper_dsn::kicad::sexpr::parse;

const LAYERS: &str = r#"(layers (0 "F.Cu" signal) (2 "B.Cu" signal) (1 "F.Mask" user))"#;

fn board(body: &str) -> String {
    format!("(kicad_pcb (version 20241229) {LAYERS} {body})")
}

#[test]
fn it_keeps_only_copper_layers_in_order() {
    let root = parse(&board("")).expect("it parses");
    let layers = Layers::read(&root).expect("layers");
    assert_eq!(layers.entries.len(), 2);
    assert_eq!(layers.entries[0].name.as_deref(), Some("F.Cu"));
    assert_eq!(layers.index_of("B.Cu").expect("an index"), 1);
}

#[test]
fn it_maps_a_power_layer_to_a_plane() {
    let text = r#"(kicad_pcb (version 20241229) (layers (0 "F.Cu" signal) (2 "B.Cu" power)))"#;
    let root = parse(text).expect("it parses");
    let layers = Layers::read(&root).expect("layers");
    assert_eq!(layers.entries[1].r#type.as_deref(), Some("plane"));
}

#[test]
fn it_accepts_a_renamed_copper_layer_by_its_declared_type() {
    let text = r#"(kicad_pcb (version 20241229) (layers (0 TOP mixed) (31 BOTTOM mixed)))"#;
    let root = parse(text).expect("it parses");
    let layers = Layers::read(&root).expect("layers");
    assert_eq!(layers.entries.len(), 2);
    assert_eq!(layers.entries[0].name.as_deref(), Some("TOP"));
    assert_eq!(layers.entries[1].name.as_deref(), Some("BOTTOM"));
}

#[test]
fn it_rejects_an_unknown_copper_layer_type() {
    let text = r#"(kicad_pcb (version 20241229) (layers (0 "F.Cu" jumper)))"#;
    let root = parse(text).expect("it parses");
    let error = Layers::read(&root).expect_err("it fails");
    assert_eq!(error.message, "Unsupported copper layer type: jumper");
}

#[test]
fn it_resolves_a_track_net_by_number() {
    let root = parse(&board(r#"(net 1 "GND") (segment (net 1))"#)).expect("it parses");
    let nets = NetTable::read(&root).expect("nets");
    let segment = root.child("segment").expect("a segment");
    assert_eq!(nets.name_of(segment).expect("a name"), "GND");
}

#[test]
fn it_rejects_an_unknown_net_number() {
    let root = parse(&board(r#"(net 1 "GND") (segment (net 7))"#)).expect("it parses");
    let nets = NetTable::read(&root).expect("nets");
    let segment = root.child("segment").expect("a segment");
    assert_eq!(
        nets.name_of(segment).expect_err("it fails").message,
        "Unknown net 7"
    );
}

#[test]
fn it_treats_net_zero_as_no_net() {
    let root = parse(&board(r#"(net 1 "GND") (segment (net 0))"#)).expect("it parses");
    let nets = NetTable::read(&root).expect("nets");
    let segment = root.child("segment").expect("a segment");
    assert_eq!(nets.name_of(segment).expect("a name"), "");
}

#[test]
fn it_rejects_non_numeric_net_text() {
    let root = parse(&board(r#"(net 1 "GND") (segment (net abc))"#)).expect("it parses");
    let nets = NetTable::read(&root).expect("nets");
    let segment = root.child("segment").expect("a segment");
    assert_eq!(
        nets.name_of(segment).expect_err("it fails").message,
        "Invalid or excessive board coordinate."
    );
}

#[test]
fn it_rejects_an_out_of_range_net_id() {
    let root = parse(&board(r#"(net 1 "GND") (segment (net 999999999))"#)).expect("it parses");
    let nets = NetTable::read(&root).expect("nets");
    let segment = root.child("segment").expect("a segment");
    assert_eq!(
        nets.name_of(segment).expect_err("it fails").message,
        "Invalid or excessive board coordinate."
    );
}

#[test]
fn it_resolves_a_track_net_by_name_on_new_boards() {
    let text =
        format!(r#"(kicad_pcb (version 20260101) {LAYERS} (net 1 "GND") (segment (net "VCC")))"#);
    let root = parse(&text).expect("it parses");
    let nets = NetTable::read(&root).expect("nets");
    let segment = root.child("segment").expect("a segment");
    assert_eq!(nets.name_of(segment).expect("a name"), "VCC");
}

#[test]
fn it_synthesises_a_default_class_when_the_board_has_none() {
    let root = parse(&board("")).expect("it parses");
    let defaults = copper_dsn::kicad::NetClassJson {
        name: Some("Default".to_string()),
        clearance: 0.2,
        traceWidth: 0.25,
        viaDiameter: 0.6,
        viaDrill: 0.3,
        ..Default::default()
    };
    let classes = read_net_classes(&root, &defaults).expect("classes");
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].name.as_deref(), Some("Default"));
    assert!((classes[0].clearance - 0.2).abs() < f64::EPSILON);
}

#[test]
fn it_rejects_a_net_in_two_classes() {
    let body = concat!(
        r#"(net_class "A" "" (clearance 0.2) (trace_width 0.25) (via_dia 0.6) (via_drill 0.3) (add_net "GND"))"#,
        r#"(net_class "B" "" (clearance 0.3) (trace_width 0.25) (via_dia 0.6) (via_drill 0.3) (add_net "GND"))"#,
    );
    let root = parse(&board(body)).expect("it parses");
    let defaults = copper_dsn::kicad::NetClassJson::default();
    let error = read_net_classes(&root, &defaults).expect_err("it fails");
    assert_eq!(
        error.message,
        "Net GND belongs to multiple embedded net classes."
    );
}

#[test]
fn it_assigns_board_nets_to_their_declared_class_in_finish() {
    let body = concat!(
        r#"(net 1 "GND") (net 2 "VCC") "#,
        r#"(net_class "Power" "" (clearance 0.3) (trace_width 0.3) (via_dia 0.6) (via_drill 0.3) (add_net "VCC") (add_net "PHANTOM"))"#,
    );
    let root = parse(&board(body)).expect("it parses");
    let nets = NetTable::read(&root).expect("nets");
    let defaults = copper_dsn::kicad::pcb::default_net_class();
    let classes = read_net_classes(&root, &defaults).expect("classes");
    let (nets, classes) = nets.finish(classes);

    let gnd = nets
        .iter()
        .find(|net| net.name.as_deref() == Some("GND"))
        .expect("a GND net");
    assert_eq!(gnd.className.as_deref(), Some("Default"));

    let vcc = nets
        .iter()
        .find(|net| net.name.as_deref() == Some("VCC"))
        .expect("a VCC net");
    assert_eq!(vcc.className.as_deref(), Some("Power"));

    assert_eq!(classes[0].name.as_deref(), Some("Default"));
    let power = classes
        .iter()
        .find(|class| class.name.as_deref() == Some("Power"))
        .expect("a Power class");
    assert_eq!(power.netNames.as_deref(), Some(&["VCC".to_string()][..]));
}

#[test]
fn it_registers_a_newly_seen_named_net_so_finish_can_assign_it() {
    let text =
        format!(r#"(kicad_pcb (version 20260101) {LAYERS} (net 1 "GND") (segment (net "VCC")))"#);
    let root = parse(&text).expect("it parses");
    let nets = NetTable::read(&root).expect("nets");
    let segment = root.child("segment").expect("a segment");
    nets.name_of(segment).expect("a name");

    let defaults = copper_dsn::kicad::pcb::default_net_class();
    let classes = read_net_classes(&root, &defaults).expect("classes");
    let (nets, _) = nets.finish(classes);
    assert!(nets.iter().any(|net| net.name.as_deref() == Some("VCC")));
}
