use copper_board::{Board, Item};
use copper_dsn::keyword::{Keyword, ScopeKeyword};
use copper_dsn::lexer::{DsnScanner, Token};
use copper_dsn::parser::scope_parameter::{DsnReadOptions, ReadScopeParameter, read_scope};

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
    let path = testkit::fixture(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("fixture {}: {e}", path.display()))
}

fn test_data(name: &str) -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/");
    std::fs::read_to_string(format!("{path}{name}"))
        .unwrap_or_else(|e| panic!("test data {name}: {e}"))
}

fn golden(name: &str) -> Vec<String> {
    test_data(name)
        .lines()
        .filter(|l| !l.starts_with('#'))
        .map(ToString::to_string)
        .collect()
}

fn dump(board: &Board, warnings: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    out.push(format!("layers {}", board.get_layer_count()));
    let ctx = board.ctx();
    for (id, item) in board.items.iter() {
        let kind = match item {
            Item::BoardOutline(_) => "BoardOutline",
            Item::ObstacleArea(_) => "ObstacleArea",
            Item::ViaObstacleArea(_) => "ViaObstacleArea",
            Item::ComponentObstacleArea(_) => "ComponentObstacleArea",
            Item::ComponentOutline(_) => "ComponentOutline",
            Item::ConductionArea(_) => "ConductionArea",
            Item::Pin(_) => "Pin",
            Item::Via(_) => "Via",
            Item::Trace(_) => "PolylineTrace",
        };
        let detail = match item {
            Item::ObstacleArea(a) => format!(
                " layer={} name={}",
                a.area.get_layer(),
                a.area.name().unwrap_or("null")
            ),
            Item::ViaObstacleArea(a) => format!(
                " layer={} name={}",
                a.area.get_layer(),
                a.area.name().unwrap_or("null")
            ),
            Item::ComponentObstacleArea(a) => format!(
                " layer={} name={}",
                a.area.get_layer(),
                a.area.name().unwrap_or("null")
            ),
            Item::ComponentOutline(o) => {
                format!(
                    " layer={} courtyard={}",
                    o.get_layer(&ctx),
                    o.is_courtyard()
                )
            }
            Item::Pin(p) => format!(" pin={}", p.name(&ctx).unwrap_or("null")),
            Item::ConductionArea(a) => format!(
                " layer={} name={}",
                a.area.get_layer(),
                a.area.name().unwrap_or("null")
            ),
            _ => String::new(),
        };
        let header = item.header();
        let mut nets = String::new();
        for i in 0..header.net_count() {
            nets.push_str(&format!("{},", header.get_net_number(i)));
        }
        out.push(format!(
            "item {} {kind}{detail} cmp={} cl={} nets=[{nets}]",
            id.0,
            header.get_component_id(),
            header.clearance_class(),
        ));
    }
    out.push(format!("itemcount {}", board.items.len()));

    for i in 1..=board.rules.nets.max_net_number() {
        let net = board.rules.nets.get(i).expect("net in range");
        out.push(format!(
            "net {i} {} subnet={} class={} plane={}",
            net.name,
            net.subnet_number,
            board.rules.net_classes.get(net.get_net_class()).get_name(),
            net.contains_plane(),
        ));
    }

    let via_padstacks = board.library.get_via_padstacks();
    let names: Vec<&str> = via_padstacks
        .iter()
        .map(|id| {
            board
                .library
                .get_padstack(*id)
                .map_or("null", |p| p.name.as_str())
        })
        .collect();
    out.push(format!(
        "viapadstacks[{}] {}",
        via_padstacks.len(),
        names.join(" ")
    ));

    for (i, via) in board.rules.via_infos.iter().enumerate() {
        out.push(format!(
            "viainfo {i} {} padstack={} cl={} attach={}",
            via.get_name(),
            board
                .library
                .get_padstack(via.get_padstack())
                .map_or("null", |p| p.name.as_str()),
            via.get_clearance_class_index(),
            via.attach_smd_allowed(),
        ));
    }

    for (i, rule) in board.rules.via_rules.iter().enumerate() {
        let vias: Vec<&str> = rule.iter().map(copper_board::ViaInfo::get_name).collect();
        out.push(format!("viarule {i} {} [{}]", rule.name, vias.join(" ")));
    }

    for i in 0..board.rules.net_classes.count() {
        let net_class = board.rules.net_classes.get(copper_board::NetClassId(i));
        let via_rule = net_class
            .get_via_rule()
            .map_or("null", |rule| rule.name.as_str());
        let half_widths: Vec<String> = (0..net_class.layer_count())
            .map(|layer| net_class.get_trace_half_width(layer).to_string())
            .collect();
        out.push(format!(
            "netclass {i} {} traceCl={} viaRule={via_rule} hw=[{}] pullTight={} shoveFixed={} \
             minLen={:?} maxLen={:?}",
            net_class.get_name(),
            net_class.get_trace_clearance_class(),
            half_widths.join(","),
            net_class.get_pull_tight(),
            net_class.is_shove_fixed(),
            net_class.get_minimum_trace_length(),
            net_class.get_maximum_trace_length(),
        ));
    }

    let cm = &board.rules.clearance_matrix;
    let mut class_names = String::new();
    for i in 0..cm.get_class_count() {
        class_names.push_str(&format!("{i}:{} ", cm.get_name(i).unwrap_or("null")));
    }
    out.push(format!(
        "clclasses[{}] {}",
        cm.get_class_count(),
        class_names.trim_end()
    ));
    for i in 0..cm.get_class_count() {
        let mut row = String::new();
        for j in 0..cm.get_class_count() {
            row.push_str(&format!("{} ", cm.get_value(i, j, 0, false)));
        }
        out.push(format!("clrow {i} {}", row.trim_end()));
    }

    for i in 1..=board.components.count() {
        let component = board.components.get(i as i32);
        out.push(format!(
            "component {i} {} pkg={} placed={} front={} lp={}",
            component.name,
            board.library.packages.get(component.get_package()).name,
            component.is_placed(),
            component.placed_on_front(),
            component.get_logical_part().map_or("null", |no| board
                .library
                .logical_parts
                .get(no)
                .name
                .as_str()),
        ));
    }

    for i in 0..board.library.logical_parts.count() {
        out.push(format!(
            "logicalpart {i} {}",
            board.library.logical_parts.get(i).name
        ));
    }

    for warning in warnings {
        out.push(format!("warning {warning}"));
    }
    out
}

fn assert_matches_golden(dsn: &str, golden_name: &str) {
    read_pcb(dsn, |ok, p| {
        assert!(ok, "{golden_name}: the read must succeed");
        let board = p.board.as_ref().expect("board built");
        let actual = dump(board, &p.warnings);
        let expected = golden(golden_name);
        for (i, (a, e)) in actual.iter().zip(expected.iter()).enumerate() {
            assert_eq!(a, e, "{golden_name}: line {} differs", i + 1);
        }
        assert_eq!(
            actual.len(),
            expected.len(),
            "{golden_name}: line count differs"
        );
    });
}

#[test]
fn issue026_matches_javas_item_ids_nets_and_rules() {
    assert_matches_golden(
        &fixture("Issue026-J2_reference.dsn"),
        "Issue026-J2_reference-items.txt",
    );
}

#[test]
fn issue034_matches_javas_item_ids_nets_and_rules() {
    assert_matches_golden(
        &fixture("Issue034-Green14SegLED.dsn"),
        "Issue034-Green14SegLED-items.txt",
    );
}

#[test]
fn empty_board_has_no_network_scope_and_no_via_rule() {
    assert_matches_golden(&fixture("empty_board.dsn"), "empty_board-items.txt");
}

#[test]
fn via_padstack_names_are_merged_normalised_and_compacted() {
    assert_matches_golden(&test_data("via_order.dsn"), "via_order-items.txt");
}

#[test]
fn a_network_only_via_padstack_list_survives_because_set_via_padstacks_is_skipped() {
    assert_matches_golden(&test_data("network_via.dsn"), "network_via-items.txt");

    read_pcb(&test_data("network_via.dsn"), |_, p| {
        let board = p.board.as_ref().expect("board built");
        let names: Vec<&str> = board
            .library
            .get_via_padstacks()
            .iter()
            .map(|id| {
                board
                    .library
                    .get_padstack(*id)
                    .expect("resolves")
                    .name
                    .as_str()
            })
            .collect();
        assert_eq!(names, ["VA", "VB"]);
    });
}

#[test]
fn class_pairs_write_both_halves_of_the_clearance_matrix() {
    assert_matches_golden(&test_data("class_pair.dsn"), "class_pair-items.txt");

    read_pcb(&test_data("class_pair.dsn"), |_, p| {
        let board = p.board.as_ref().expect("board built");
        let cm = &board.rules.clearance_matrix;
        let alpha = cm.get_no("Alpha").expect("Alpha clearance class");
        let beta = cm.get_no("Beta").expect("Beta clearance class");
        let gamma = cm.get_no("Gamma").expect("Gamma clearance class");
        assert_eq!(cm.get_value(alpha, beta, 0, false), 7770);
        assert_eq!(cm.get_value(beta, alpha, 0, false), 7770);
        assert_eq!(cm.get_value(alpha, gamma, 0, false), 7770);
        assert_eq!(cm.get_value(gamma, alpha, 0, false), 7770);
        assert_eq!(cm.get_value(beta, gamma, 0, false), 5000);
        assert_eq!(cm.get_value(gamma, beta, 0, false), 5000);
    });
}

#[test]
fn a_net_classs_use_via_list_is_aliased_into_the_merged_via_padstack_names() {
    assert_matches_golden(&test_data("alias.dsn"), "alias-items.txt");

    read_pcb(&test_data("alias.dsn"), |_, p| {
        let board = p.board.as_ref().expect("board built");
        let alpha = board.rules.get_via_rule("Alpha").expect("Alpha via rule");
        let beta = board.rules.get_via_rule("Beta").expect("Beta via rule");
        assert_eq!(board.rules.via_rules[alpha.0].via_count(), 2);
        assert_eq!(board.rules.via_rules[beta.0].via_count(), 1);
    });
}
