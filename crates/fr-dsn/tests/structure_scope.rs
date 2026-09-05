//! Java authority: `io/specctra/parser/{Structure,Plane,PlaceControl,AutorouteSettings}.java`.
use fr_board::{AngleRestriction, Item, Layer, LayerStructure};
use fr_dsn::format::{DSN_RESERVED, IdentifierType, IndentFileWriter};
use fr_dsn::keyword::{Keyword, ScopeKeyword};
use fr_dsn::lexer::{DsnScanner, Token};
use fr_dsn::parser::autoroute_settings::{DsnRouterSettings, write_autoroute_settings_scope};
use fr_dsn::parser::scope_parameter::{DsnReadOptions, ReadScopeParameter, read_scope};

fn read_pcb<T>(text: &str, f: impl FnOnce(bool, &mut ReadScopeParameter<'_>) -> T) -> T {
    let options = DsnReadOptions::default();
    let scanner = DsnScanner::new(text);
    let mut p = ReadScopeParameter::new(scanner, &options);
    assert_eq!(p.scanner.next_token().expect("scan"), Some(Token::Open));
    assert_eq!(
        p.scanner.next_token().expect("scan"),
        Some(Token::Kw(Keyword::PcbScope))
    );
    let ok = read_scope(ScopeKeyword::Pcb, &mut p).expect("no scan error");
    f(ok, &mut p)
}

fn fixture(name: &str) -> String {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../freerouting/fixtures/"
    );
    std::fs::read_to_string(format!("{path}{name}"))
        .unwrap_or_else(|e| panic!("fixture {name}: {e}"))
}

#[test]
fn empty_board_dsn_builds_a_two_layer_board_with_javas_bounding_box() {
    let text = fixture("empty_board.dsn");
    read_pcb(&text, |ok, p| {
        assert!(ok);
        assert!(p.board_outline_ok);
        let board = p.board.as_ref().expect("board built");
        assert_eq!(board.layer_structure().layers.len(), 2);
        assert_eq!(board.layer_structure().layers[0].name, "F.Cu");
        assert_eq!(board.layer_structure().layers[1].name, "B.Cu");
        assert!(board.layer_structure().layers.iter().all(|l| l.is_signal));
        assert_eq!(board.bounding_box.ll.x, 1_294_400);
        assert_eq!(board.bounding_box.ll.y, -1_080_500);
        assert_eq!(board.bounding_box.ur.x, 1_867_900);
        assert_eq!(board.bounding_box.ur.y, -570_500);
        let ct = p.coordinate_transform.expect("coordinate transform set");
        assert!((ct.board_to_dsn(1.0) - 0.1).abs() < 1e-12);
        assert_eq!(board.items.len(), 1);
        assert!(matches!(
            board.items.values().next(),
            Some(Item::BoardOutline(_))
        ));
    });
}

#[test]
fn issue756_minimal_ok_builds_javas_bounding_box() {
    let text = fixture("Issue756-minimal-ok.dsn");
    read_pcb(&text, |ok, p| {
        assert!(ok);
        let board = p.board.as_ref().expect("board built");
        assert_eq!(board.layer_structure().layers.len(), 2);
        assert_eq!(board.bounding_box.ll.x, 899_000);
        assert_eq!(board.bounding_box.ll.y, -451_000);
        assert_eq!(board.bounding_box.ur.x, 1_101_000);
        assert_eq!(board.bounding_box.ur.y, -349_000);
        assert_eq!(p.host_cad.as_deref(), Some("KiCad's Pcbnew"));
    });
}

fn synthetic(resolution: &str, structure_body: &str) -> String {
    format!(
        "(pcb synthetic.dsn\n  (parser\n    (string_quote \")\n  )\n  (resolution um \
         {resolution})\n  (unit um)\n  (structure\n    (layer F.Cu\n      (type signal)\n    )\n \
         (layer B.Cu\n      (type signal)\n    )\n{structure_body}\n  )\n)\n"
    )
}

#[test]
fn a_degenerate_bounding_box_clears_board_outline_ok_and_builds_no_board() {
    let text = synthetic("10", "    (boundary\n      (rect pcb 0 0 0 0)\n    )");
    read_pcb(&text, |ok, p| {
        assert!(!ok, "Structure.readScope propagates createBoard's false");
        assert!(!p.board_outline_ok);
        assert!(p.board.is_none());
    });
}

#[test]
fn a_six_point_seven_million_unit_outline_keeps_its_scale() {
    let text = synthetic(
        "10",
        "    (boundary\n      (rect pcb 0 0 10000000 10000000)\n    )",
    );
    read_pcb(&text, |ok, p| {
        assert!(ok);
        let ct = p.coordinate_transform.expect("coordinate transform set");
        assert!(
            (ct.scale_factor() - 0.1).abs() < 1e-15,
            "two divisions by ten from 10, in f64: {}",
            ct.scale_factor()
        );
        assert_eq!(ct.board_to_dsn(1.0), 10.0);
        let board = p.board.as_ref().expect("board built");
        assert_eq!(board.bounding_box.ll.x, -1000);
        assert_eq!(board.bounding_box.ll.y, -1000);
        assert_eq!(board.bounding_box.ur.x, 1_001_000);
        assert_eq!(board.bounding_box.ur.y, 1_001_000);
    });
}

#[test]
fn the_plan_datum_for_the_overflow_loop_stops_at_a_scale_factor_of_100() {
    let text = synthetic(
        "10000000",
        "    (boundary\n      (rect pcb 0 0 10000 10000)\n    )",
    );
    read_pcb(&text, |ok, p| {
        assert!(ok);
        let ct = p.coordinate_transform.expect("coordinate transform set");
        assert!((ct.board_to_dsn(1.0) - 0.01).abs() < 1e-15);
    });
}

#[test]
fn a_clearance_rule_writes_both_index_orders() {
    let text = synthetic(
        "10",
        "    (boundary\n      (rect pcb 0 0 100000 50000)\n    )\n    (rule\n      (width \
         152.4)\n      (clearance 152.4)\n      (clearance 200 (type via_smd))\n    )",
    );
    read_pcb(&text, |ok, p| {
        assert!(ok);
        let board = p.board.as_mut().expect("board built");
        let matrix = &board.rules.clearance_matrix;
        assert_eq!(matrix.get_class_count(), 4);
        assert_eq!(matrix.get_name(0), Some("null"));
        assert_eq!(matrix.get_name(1), Some("default"));
        assert_eq!(matrix.get_name(2), Some("via"));
        assert_eq!(matrix.get_name(3), Some("smd"));
        assert_eq!(matrix.get_value(1, 1, 0, false), 1524);
        assert_eq!(matrix.get_value(2, 3, 0, false), 2000);
        assert_eq!(matrix.get_value(3, 2, 0, false), 2000);

        let default_class = board.rules.get_default_net_class();
        assert_eq!(
            board
                .rules
                .net_classes
                .get(default_class)
                .get_trace_half_width(0),
            762
        );
        assert!((board.rules.get_pin_edge_to_turn_dist() - 762.0).abs() < 1e-12);
    });
}

#[test]
fn a_snap_angle_scope_sets_the_boards_trace_angle_restriction() {
    let text = synthetic(
        "10",
        "    (boundary\n      (rect pcb 0 0 100000 50000)\n    )\n    (snap_angle\n      \
         ninety_degree\n    )",
    );
    read_pcb(&text, |ok, p| {
        assert!(ok);
        assert_eq!(p.snap_angle, AngleRestriction::NinetyDegree);
        let board = p.board.as_ref().expect("board built");
        assert_eq!(
            board.rules.trace_angle_restriction,
            AngleRestriction::NinetyDegree
        );
    });
}

#[test]
fn a_flip_style_scope_inside_structure_sets_flip_style_rotate_first() {
    let text = synthetic(
        "10",
        "    (boundary\n      (rect pcb 0 0 100000 50000)\n    )\n    (flip_style \
         rotate_first)",
    );
    read_pcb(&text, |ok, p| {
        assert!(ok);
        let board = p.board.as_ref().expect("board built");
        assert!(board.components.get_flip_style_rotate_first());
    });
}

#[test]
fn a_place_control_scope_sets_flip_style_rotate_first() {
    let text = "(pcb synthetic.dsn\n  (parser\n    (string_quote \")\n  )\n  (resolution um 10)\n  (structure\n    (layer F.Cu\n      (type signal)\n    )\n    (layer B.Cu\n      (type signal)\n    )\n    (boundary\n      (rect pcb 0 0 100000 50000)\n    )\n  )\n  (place_control\n    (flip_style rotate_first)\n  )\n)\n";
    read_pcb(text, |ok, p| {
        assert!(ok);
        let board = p.board.as_ref().expect("board built");
        assert!(board.components.get_flip_style_rotate_first());
    });
}

#[test]
fn a_signal_layer_keepout_is_inserted_on_every_signal_layer() {
    let text = synthetic(
        "10",
        "    (boundary\n      (rect pcb 0 0 100000 50000)\n    )\n    (keepout \"\"\n      (rect \
         signal 1000 1000 2000 2000)\n    )",
    );
    read_pcb(&text, |ok, p| {
        assert!(ok);
        let board = p.board.as_ref().expect("board built");
        let obstacles: Vec<_> = board
            .items
            .values()
            .filter(|i| matches!(i, Item::ObstacleArea(_)))
            .collect();
        assert_eq!(obstacles.len(), 2, "one per signal layer");
    });
}

#[test]
fn a_plane_scope_inserts_a_conduction_area_and_creates_its_net() {
    let text = synthetic(
        "10",
        "    (boundary\n      (rect pcb 0 0 100000 50000)\n    )\n    (plane GND\n      (rect \
         F.Cu 1000 1000 2000 2000)\n    )",
    );
    read_pcb(&text, |ok, p| {
        assert!(ok);
        assert_eq!(p.plane_list.len(), 1);
        assert_eq!(p.plane_list[0].net_name, "GND");
        let board = p.board.as_ref().expect("board built");
        let conduction: Vec<_> = board.get_conduction_areas();
        assert_eq!(conduction.len(), 1);
        assert_eq!(
            board
                .rules
                .nets
                .get_by_name_and_subnet("GND", 1)
                .map(|n| n.net_number),
            Some(1)
        );
    });
}

#[test]
fn write_autoroute_settings_scope_matches_javas_bytes_on_a_two_layer_board() {
    let layer_structure =
        LayerStructure::new(vec![Layer::new("F.Cu", true), Layer::new("B.Cu", true)]);
    let mut settings = DsnRouterSettings::new();
    settings.set_layer_count(2);

    let mut out: Vec<u8> = Vec::new();
    {
        let mut file = IndentFileWriter::new(&mut out);
        let identifier = IdentifierType::new(
            DSN_RESERVED.iter().map(|s| (*s).to_string()).collect(),
            "\"".to_string(),
        );
        write_autoroute_settings_scope(&mut file, &settings, &layer_structure, &identifier);
        file.flush().expect("write");
    }
    let text = String::from_utf8(out).expect("utf-8");
    let expected = [
        "",
        "(autoroute_settings",
        "  (autoroute on)",
        "  (postroute off)",
        "  (vias on)",
        "  (via_costs 1)",
        "  (plane_via_costs 1)",
        "  (start_ripup_costs 1)",
        "  ",
        "  (layer_rule F.Cu",
        "    (active on)",
        "    (preferred_direction vertical)",
        "    (preferred_direction_trace_costs 1.0)",
        "    (against_preferred_direction_trace_costs 1.0)",
        "  )",
        "  (layer_rule B.Cu",
        "    (active on)",
        "    (preferred_direction horizontal)",
        "    (preferred_direction_trace_costs 1.0)",
        "    (against_preferred_direction_trace_costs 1.0)",
        "  )",
        ")",
    ]
    .join("\n");
    assert_eq!(text, expected);
    assert!(text.contains("(preferred_direction_trace_costs 1.0)"));
}

#[test]
fn read_autoroute_settings_scope_reads_costs_and_layer_rules() {
    let text = synthetic(
        "10",
        "    (autoroute_settings\n      (fanout off)\n      (autoroute on)\n      (postroute \
         off)\n      (vias off)\n      (via_costs 42)\n      (plane_via_costs 7)\n      \
         (start_ripup_costs 13)\n      (layer_rule F.Cu\n        (active off)\n        \
         (preferred_direction horizontal)\n        (preferred_direction_trace_costs 2.5)\n       \
         (against_preferred_direction_trace_costs 3.5)\n      )\n    )\n    (boundary\n      \
         (rect pcb 0 0 100000 50000)\n    )",
    );
    read_pcb(&text, |ok, p| {
        assert!(ok);
        let settings = p
            .autoroute_settings
            .as_ref()
            .expect("autoroute_settings read");
        assert!(settings.run_router());
        assert!(!settings.run_optimizer());
        assert!(!settings.vias_allowed());
        assert_eq!(settings.via_costs(), 42);
        assert_eq!(settings.plane_via_costs(), 7);
        assert_eq!(settings.start_ripup_costs(), 13);
        assert!(!settings.get_layer_active(0));
        assert!(settings.get_layer_active(1));
        assert!(settings.get_preferred_direction_is_horizontal(0));
        assert!((settings.get_preferred_direction_trace_costs(0) - 2.5).abs() < 1e-12);
        assert!((settings.get_against_preferred_direction_trace_costs(0) - 3.5).abs() < 1e-12);
    });
}

#[test]
fn an_autoroute_settings_scope_after_a_keepout_is_read() {
    let text = synthetic(
        "10",
        "    (boundary\n      (rect pcb 0 0 100000 50000)\n    )\n    (keepout \"\"\n      (rect \
         F.Cu 1000 1000 2000 2000)\n    )\n    (autoroute_settings\n      (via_costs 42)\n    )",
    );
    read_pcb(&text, |ok, p| {
        assert!(ok, "the structure scope still closes on its own bracket");
        let settings = p
            .autoroute_settings
            .as_ref()
            .expect("the settings after a keepout are read now");
        assert_eq!(settings.via_costs(), 42);
    });
}

#[test]
fn the_corpus_sweep_counts_boards_whose_settings_were_read() {
    if !parity::require_java_dir() {
        return;
    }
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(parity::java_dir().join("fixtures"))
        .expect("the fixtures directory")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "dsn"))
        .collect();
    files.push(parity::example("tutorial_board/tutorial_board.dsn"));
    files.sort();
    assert_eq!(files.len(), 106, "sweep-p3t15.sh's corpus");

    let mut read: Vec<String> = Vec::new();
    for path in &files {
        let bytes = std::fs::read(path).expect("readable fixture");
        if let fr_dsn::BoardReadResult::Success {
            metadata: Some(metadata),
            ..
        } = fr_dsn::read_metadata(bytes.as_slice())
            && metadata.router_settings.is_some()
        {
            read.push(
                path.file_name()
                    .expect("a file name")
                    .to_string_lossy()
                    .into_owned(),
            );
        }
    }
    assert_eq!(
        read,
        vec![
            "Issue103-Board-Routed.dsn".to_string(),
            "Issue187-processor.Z80.dsn".to_string(),
        ],
        "the corpus boards whose (autoroute_settings …) scope reaches the reader"
    );
}

#[test]
fn insertion_order_matches_java_item_for_item() {
    let text = fixture_text_order_dsn();
    read_pcb(&text, |ok, p| {
        assert!(ok);
        let board = p.board.as_ref().expect("board built");
        assert_eq!(board.bounding_box.ll.x, -1000);
        assert_eq!(board.bounding_box.ll.y, -1000);
        assert_eq!(board.bounding_box.ur.x, 2_001_000);
        assert_eq!(board.bounding_box.ur.y, 1_001_000);

        let actual: Vec<String> = board
            .items
            .iter()
            .map(|(id, item)| {
                let kind = match item {
                    Item::BoardOutline(_) => "BoardOutline",
                    Item::ObstacleArea(_) => "ObstacleArea",
                    Item::ViaObstacleArea(_) => "ViaObstacleArea",
                    Item::ComponentObstacleArea(_) => "ComponentObstacleArea",
                    Item::ConductionArea(_) => "ConductionArea",
                    _ => "other",
                };
                let layer = match item {
                    Item::ObstacleArea(a) => format!(" layer={}", a.area.get_layer()),
                    Item::ViaObstacleArea(a) => format!(" layer={}", a.area.get_layer()),
                    Item::ComponentObstacleArea(a) => format!(" layer={}", a.area.get_layer()),
                    Item::ConductionArea(a) => format!(" layer={}", a.area.get_layer()),
                    _ => String::new(),
                };
                let nets = if item.header().net_count() > 0 {
                    format!(" net={}", item.header().get_net_number(0))
                } else {
                    String::new()
                };
                format!(
                    "item {} {kind}{layer}{nets} cl={}",
                    id.0,
                    item.header().clearance_class()
                )
            })
            .collect();
        let expected = [
            "item 1 BoardOutline cl=1",
            "item 2 ObstacleArea layer=0 cl=0",
            "item 3 ObstacleArea layer=1 cl=0",
            "item 4 ObstacleArea layer=2 cl=0",
            "item 5 ObstacleArea layer=0 cl=0",
            "item 6 ObstacleArea layer=1 cl=0",
            "item 7 ObstacleArea layer=2 cl=0",
            "item 8 ObstacleArea layer=0 cl=1",
            "item 9 ObstacleArea layer=0 cl=1",
            "item 10 ObstacleArea layer=2 cl=1",
            "item 11 ViaObstacleArea layer=2 cl=1",
            "item 12 ComponentObstacleArea layer=0 cl=1",
            "item 13 ConductionArea layer=2 net=1 cl=1",
            "item 14 ConductionArea layer=1 net=2 cl=0",
        ];
        assert_eq!(actual, expected);

        let vcc = board.rules.nets.get(1).expect("net 1");
        assert_eq!(vcc.name, "VCC");
        assert!(vcc.contains_plane());
        let gnd = board.rules.nets.get(2).expect("net 2");
        assert_eq!(gnd.name, "GND");
        assert!(gnd.contains_plane());
        assert_eq!(board.rules.nets.count(), 2);
    });
}

fn fixture_text_order_dsn() -> String {
    [
        "(pcb order.dsn",
        "  (parser",
        "    (string_quote \")",
        "  )",
        "  (resolution um 10)",
        "  (unit um)",
        "  (structure",
        "    (layer F.Cu",
        "      (type signal)",
        "    )",
        "    (layer In1.Cu",
        "      (type power)",
        "      (use_net GND)",
        "    )",
        "    (layer B.Cu",
        "      (type signal)",
        "    )",
        "    (boundary",
        "      (path pcb 0  0 0  200000 0  200000 100000  0 100000  0 0)",
        "    )",
        "    (boundary",
        "      (path pcb 0  50000 30000  60000 30000  60000 40000  50000 40000  50000 30000)",
        "    )",
        "    (boundary",
        "      (path pcb 0  90000 30000  100000 30000  100000 40000  90000 40000  90000 30000)",
        "    )",
        "    (keepout \"ko1\"",
        "      (rect F.Cu 1000 1000 2000 2000)",
        "    )",
        "    (keepout \"ko2\"",
        "      (rect signal 3000 1000 4000 2000)",
        "    )",
        "    (via_keepout \"vko1\"",
        "      (rect B.Cu 5000 1000 6000 2000)",
        "    )",
        "    (place_keepout \"pko1\"",
        "      (rect F.Cu 7000 1000 8000 2000)",
        "    )",
        "    (plane VCC",
        "      (rect B.Cu 10000 10000 20000 20000)",
        "    )",
        "  )",
        ")",
        "",
    ]
    .join("\n")
}
