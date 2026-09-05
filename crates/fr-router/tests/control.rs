use std::path::Path;

use fr_board::ids::{NetClassId, ViaInfoId};
use fr_board::prelude::*;
use fr_board::rules::{ViaInfo, ViaRule};
use fr_dsn::{BoardReadResult, DsnReadOptions};
use fr_geometry::{IntBox, Shape, TileShape};
use fr_router::ExpansionCostFactor;
use fr_router::autoroute::maze::AutorouteControl;
use fr_settings::RouterSettings;

fn fixture_board(name: &str) -> Board {
    let path = parity::fixture(name);
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("cannot read fixture {}: {e}", path.display()));
    match fr_dsn::read_board(&bytes[..], None, Some(name), &DsnReadOptions::default()) {
        BoardReadResult::Success { board, .. } | BoardReadResult::OutlineMissing { board, .. } => {
            *board.unwrap_or_else(|| panic!("{name} produced no board"))
        }
        other => panic!("{name} did not read: {other:?}"),
    }
}

fn board_settings(board: &Board) -> RouterSettings {
    let mut settings = RouterSettings::new();
    settings.set_layer_count(board.get_layer_count());
    settings.apply_board_specific_optimizations(board);
    settings
}

fn control(board: &Board, net_no: i32, settings: &RouterSettings) -> AutorouteControl {
    let trace_costs = settings.get_trace_costs();
    AutorouteControl::new(
        board,
        net_no,
        settings,
        settings.get_via_costs(),
        &trace_costs,
    )
}

fn f(v: f64) -> String {
    format!("{v:.6}")
}

fn dump_control(board: &Board, net_no: i32, settings: &RouterSettings) -> String {
    let c = control(board, net_no, settings);
    let mut s = format!("ctrl net={net_no}");
    s += &format!(
        " pureSmd={}",
        AutorouteControl::is_pure_smd_net(board, net_no)
    );
    s += &format!(" layerCount={}", c.layer_count);
    s += " layerActive=";
    for b in &c.layer_active {
        s.push(if *b { '1' } else { '0' });
    }
    s += " traceHalfWidth=";
    for v in &c.trace_half_width {
        s += &format!("{v},");
    }
    s += " compensatedTraceHalfWidth=";
    for v in &c.compensated_trace_half_width {
        s += &format!("{v},");
    }
    s += " viaRadii=";
    for v in &c.via_radii {
        s += &format!("{},", f(*v));
    }
    s += " bendCosts=";
    for v in &c.bend_costs {
        s += &format!("{},", f(*v));
    }
    s += &format!(
        " traceClearanceClassIndex={}",
        c.trace_clearance_class_index
    );
    s += &format!(" viaClearanceClass={}", c.via_clearance_class);
    s += &format!(
        " viaRule={}",
        match &c.via_rule {
            Some(rule) => rule.name.clone(),
            None => "null".to_string(),
        }
    );
    s += " viaInfos=";
    for m in &c.via_infos {
        s += &format!(
            "[{}..{} attach={}]",
            m.from_layer, m.to_layer, m.attach_smd_allowed
        );
    }
    s += &format!(" attachSmdAllowed={}", c.attach_smd_allowed);
    s += &format!(" maxViaRadius={}", f(c.max_via_radius));
    s += &format!(" minNormalViaCost={}", f(c.min_normal_via_cost));
    s += &format!(" minCheapViaCost={}", f(c.min_cheap_via_cost));
    s += &format!(" viasAllowed={}", c.vias_allowed);
    s += &format!(" withNeckdown={}", c.with_neckdown);
    s += &format!(" viaLowerBound={}", c.via_lower_bound);
    s += &format!(" viaUpperBound={}", c.via_upper_bound);
    s += &format!(" tidyRegionWidth={}", c.tidy_region_width);
    s += &format!(" pullTightAccuracy={}", c.pull_tight_accuracy);
    s += &format!(
        " maxShoveTraceRecursionDepth={}",
        c.max_shove_trace_recursion_depth
    );
    s += &format!(
        " maxShoveViaRecursionDepth={}",
        c.max_shove_via_recursion_depth
    );
    s += &format!(
        " maxSpringOverRecursionDepth={}",
        c.max_spring_over_recursion_depth
    );
    s += &format!(" ripupAllowed={}", c.ripup_allowed);
    s += &format!(" ripupCosts={}", c.ripup_costs);
    s += &format!(" ripupPassNo={}", c.ripup_pass_no);
    s += &format!(" isFanout={}", c.is_fanout);
    s += &format!(" fanoutStartPinLayer={}", c.fanout_start_pin_layer);
    s += &format!(" removeUnconnectedVias={}", c.remove_unconnected_vias);
    s += &format!(
        " addViaCostsAllZero={}",
        c.add_via_costs
            .iter()
            .all(|row| row.iter().all(|v| *v == 0))
    );
    s
}

fn transcript() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/p6t8-autoroute-control.txt");
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

#[test]
fn every_field_of_the_jvm_transcript_is_reproduced() {
    let text = transcript();
    let mut board: Option<Board> = None;
    let mut settings: Option<RouterSettings> = None;
    let mut ctrl_rows = 0usize;

    for line in text.lines() {
        let line = line.trim();
        if let Some(stem) = line.strip_prefix("######## ") {
            let b = fixture_board(&format!("{stem}.dsn"));
            settings = Some(board_settings(&b));
            board = Some(b);
            continue;
        }
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let b = board.as_ref().expect("a fixture header first");
        let s = settings.as_ref().expect("a fixture header first");

        if let Some(rest) = line.strip_prefix("board ") {
            assert_eq!(
                format!(
                    "layerCount={} maxNetNo={} viaRules={} viaInfos={}",
                    b.get_layer_count(),
                    b.rules.nets.max_net_number(),
                    b.rules.via_rules.len(),
                    b.rules.via_infos.count()
                ),
                rest
            );
        } else if let Some(rest) = line.strip_prefix("layer ") {
            let index: usize = rest.split(' ').next().unwrap().parse().unwrap();
            let layer = &b.layer_structure().layers[index];
            assert_eq!(
                format!(
                    "{index} name={} isSignal={} settingActive={}",
                    layer.name,
                    layer.is_signal,
                    s.get_layer_active(index)
                ),
                rest
            );
        } else if let Some(rest) = line.strip_prefix("settings ") {
            assert_eq!(
                format!(
                    "viaCosts={} viasAllowed={}",
                    s.get_via_costs(),
                    s.get_vias_allowed()
                ),
                rest
            );
        } else if let Some(rest) = line.strip_prefix("firstPureSmdNet=") {
            let (pure, mixed) = first_pure_and_mixed(b);
            assert_eq!(format!("{pure} firstMixedNet={mixed}"), rest);
        } else if line.contains(" threw ") {
            let net: i32 = line
                .strip_prefix("ctrl net=")
                .and_then(|r| r.split(' ').next())
                .and_then(|n| n.parse().ok())
                .expect("a net number");
            assert!(
                b.rules.nets.get(net).is_none(),
                "the jar threw here because net {net} does not exist"
            );
            let ctrl =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| control(b, net, s)))
                    .unwrap_or_else(|_| {
                        panic!("net {net} must no longer throw — quirk #173 is fixed (fixed: T6)")
                    });
            let default_class = b.rules.net_classes.get(NetClassId(0));
            for layer in 0..b.get_layer_count() {
                assert_eq!(
                    ctrl.trace_half_width[layer],
                    default_class.get_trace_half_width(layer),
                    "net {net}, layer {layer}: the null-net arm's own half-width fallback"
                );
            }
            assert_eq!(ctrl.trace_clearance_class_index, 1, "`:213`, unchanged");
        } else if line.starts_with("ctrl net=") {
            let net: i32 = line
                .strip_prefix("ctrl net=")
                .and_then(|r| r.split(' ').next())
                .and_then(|n| n.parse().ok())
                .expect("a net number");
            assert_eq!(dump_control(b, net, s), line);
            ctrl_rows += 1;
        } else {
            panic!("unparsed transcript row: {line}");
        }
    }

    assert_eq!(
        ctrl_rows, 5,
        "five successful `ctrl` rows in the transcript"
    );
}

fn first_pure_and_mixed(board: &Board) -> (i32, i32) {
    let mut pure = -1;
    let mut mixed = -1;
    for n in 1..=board.rules.nets.max_net_number() {
        if board.get_connectable_items(n).is_empty() {
            continue;
        }
        if AutorouteControl::is_pure_smd_net(board, n) {
            if pure < 0 {
                pure = n;
            }
        } else if mixed < 0 {
            mixed = n;
        }
    }
    (pure, mixed)
}

#[test]
fn pure_smd_relaxes_attach_and_scales_the_via_cost() {
    for (name, pure_net, mixed_net, pure_cost, normal_cost) in [
        ("Issue593-BBD_Mars-64.dsn", 1, 0, 400.0, 4000.0),
        ("Issue508-DAC2020_bm01.dsn", 8, 1, 300.0, 3000.0),
    ] {
        let board = fixture_board(name);
        let settings = board_settings(&board);

        assert!(AutorouteControl::is_pure_smd_net(&board, pure_net));
        let pure = control(&board, pure_net, &settings);
        assert!(
            !pure.via_infos[0].attach_smd_allowed,
            "{name}: the padstack itself still says attach=false"
        );
        assert!(
            pure.attach_smd_allowed,
            "{name}: :263-269 relaxes the routing gate anyway"
        );
        assert_eq!(pure.min_normal_via_cost, pure_cost, "{name}: :277-281");
        assert_eq!(pure.min_cheap_via_cost, 0.8 * pure_cost, "{name}: :283");

        assert!(!AutorouteControl::is_pure_smd_net(&board, mixed_net));
        let normal = control(&board, mixed_net, &settings);
        assert!(!normal.attach_smd_allowed, "{name}: no relaxation");
        assert_eq!(normal.min_normal_via_cost, normal_cost, "{name}");
        assert_eq!(
            normal.min_normal_via_cost,
            10.0 * pure.min_normal_via_cost,
            "{name}: the two differ by exactly the 0.1 factor"
        );
    }
}

#[test]
fn the_smd_relaxation_is_a_setting() {
    let board = fixture_board("Issue593-BBD_Mars-64.dsn");
    let pure_net = 1;
    assert!(AutorouteControl::is_pure_smd_net(&board, pure_net));

    let mut settings = board_settings(&board);
    settings.set_smd_via_relaxation(Some(false));
    let off = control(&board, pure_net, &settings);
    assert_eq!(off.min_normal_via_cost, 4000.0, "no 0.1 discount when off");
    assert_eq!(
        off.attach_smd_allowed,
        off.via_infos.iter().any(|via| via.attach_smd_allowed),
        "off: attachSmdAllowed agrees with the via masks, no override"
    );

    settings.set_smd_via_relaxation(Some(true));
    let on = control(&board, pure_net, &settings);
    assert_eq!(on.min_normal_via_cost, 400.0, "the 0.1 discount when on");
    assert!(
        on.attach_smd_allowed,
        "on: forced true even though every via mask says false"
    );
    assert!(
        !on.via_infos.iter().any(|via| via.attach_smd_allowed),
        "on: the padstacks themselves still say attach=false"
    );
}

#[test]
fn an_empty_net_is_not_pure_smd() {
    let board = fixture_board("Issue593-BBD_Mars-64.dsn");
    assert!(board.get_connectable_items(0).is_empty());
    assert!(!AutorouteControl::is_pure_smd_net(&board, 0));
    let unused = board.rules.nets.max_net_number() + 1000;
    assert!(board.get_connectable_items(unused).is_empty());
    assert!(!AutorouteControl::is_pure_smd_net(&board, unused));
}

#[test]
fn net_zero_falls_back_to_net_one_half_widths() {
    let board = fixture_board("Issue508-DAC2020_bm01.dsn");
    let settings = board_settings(&board);

    assert_eq!(
        board
            .rules
            .nets
            .get(1)
            .expect("net 1 exists on this board")
            .get_net_class(),
        NetClassId(0),
        "the old fallback (net 1) and the new one (the default class) agree here only because \
         net 1 is on the default class"
    );

    let zero = control(&board, 0, &settings);
    let one = control(&board, 1, &settings);
    for layer in 0..board.get_layer_count() {
        assert_eq!(
            zero.trace_half_width[layer],
            board.rules.get_trace_half_width(1, layer)
        );
        assert_eq!(zero.trace_half_width[layer], one.trace_half_width[layer]);
    }
    assert_eq!(
        zero.compensated_trace_half_width,
        one.compensated_trace_half_width
    );
}

#[test]
fn a_null_net_uses_clearance_class_one_and_the_first_via_rule() {
    let board = fixture_board("Issue508-DAC2020_bm01.dsn");
    let settings = board_settings(&board);
    assert!(
        board.rules.nets.get(0).is_none(),
        "net 0 is Java's null net"
    );

    let zero = control(&board, 0, &settings);
    assert_eq!(zero.trace_clearance_class_index, 1);
    assert_eq!(zero.via_rule.as_ref(), Some(&board.rules.via_rules[0]));
    assert!(board.rules.via_rules.len() > 1, "there is a second rule");
}

#[test]
fn a_positive_unknown_net_gets_the_half_width_fallback() {
    let board = fixture_board("Issue593-BBD_Mars-64.dsn");
    let settings = board_settings(&board);
    let unknown = board.rules.nets.max_net_number() + 1000;
    assert!(
        board.rules.nets.get(unknown).is_none(),
        "the net the control is built for must be one the board does not have"
    );

    let ctrl = control(&board, unknown, &settings);

    assert_eq!(ctrl.trace_clearance_class_index, 1);
    assert_eq!(ctrl.via_rule.as_ref(), Some(&board.rules.via_rules[0]));
    let default_class = board.rules.net_classes.get(NetClassId(0));
    for layer in 0..board.get_layer_count() {
        assert_eq!(
            ctrl.trace_half_width[layer],
            default_class.get_trace_half_width(layer),
            "layer {layer} takes the default net class's width, not net 1's and not a throw"
        );
    }
    assert!(
        ctrl.compensated_trace_half_width
            .iter()
            .zip(&ctrl.trace_half_width)
            .all(|(compensated, half)| compensated >= half),
        "`:223-225` adds a non-negative clearance compensation to each width"
    );
}

fn plane_board() -> Board {
    let layers = || {
        LayerStructure::new(vec![
            Layer::new("front", true),
            Layer::new("power", false),
            Layer::new("back", true),
        ])
    };
    let mut padstacks = Padstacks::new(layers());
    let via_padstack = padstacks.add_layer_range(
        Shape::Tile(TileShape::Box(IntBox::from_coords(-300, -300, 300, 300))),
        0,
        2,
    );
    let clearance_matrix = ClearanceMatrix::get_default_instance(&layers(), 200);
    let mut rules = BoardRules::new(layers(), clearance_matrix);
    rules
        .via_infos
        .add(ViaInfo::new("via", via_padstack, 1, false));
    let mut via_rule = ViaRule::new("default");
    via_rule.append_via(rules.via_infos.get(ViaInfoId(0)).clone());
    rules.via_rules.push(via_rule);
    let class = rules.net_classes.append("default", &layers(), false);
    rules
        .net_classes
        .get_mut(class)
        .set_via_rule(Some(rules.via_rules[0].clone()));
    rules.nets.add("n1", 1, false, class);
    Board::new(
        Vec::new(),
        0,
        IntBox::from_coords(-10_000, -10_000, 10_000, 10_000),
        rules,
        BoardLibrary::new(padstacks, Packages::new()),
        Components::new(),
        Communication::default(),
    )
}

#[test]
fn a_power_plane_is_forced_inactive_however_the_settings_ask() {
    let board = plane_board();
    let mut settings = board_settings(&board);
    for layer in 0..3 {
        settings.set_layer_active(layer, true);
    }
    let ctrl = control(&board, 0, &settings);
    assert_eq!(ctrl.layer_active, vec![true, false, true]);

    settings.set_layer_active(0, false);
    let ctrl = control(&board, 0, &settings);
    assert_eq!(ctrl.layer_active, vec![false, false, true]);
}

#[test]
fn with_every_layer_disabled_no_layer_is_routable() {
    let board = plane_board();
    let mut settings = board_settings(&board);
    for layer in 0..3 {
        settings.set_layer_active(layer, false);
    }
    let ctrl = control(&board, 0, &settings);
    assert_eq!(ctrl.layer_active, vec![false, false, false]);
    assert!(!ctrl.layer_active.iter().any(|active| *active));
}

#[test]
fn every_hard_coded_constant_matches_java() {
    let board = plane_board();
    let settings = board_settings(&board);
    let c = control(&board, 0, &settings);

    assert_eq!(c.tidy_region_width, i32::MAX, ":169");
    assert_eq!(c.pull_tight_accuracy, 500, ":170");
    assert_eq!(c.max_shove_trace_recursion_depth, 20, ":171");
    assert_eq!(c.max_shove_via_recursion_depth, 5, ":172");
    assert_eq!(c.max_spring_over_recursion_depth, 5, ":173");

    assert_eq!(c.via_lower_bound, 0, ":181");
    assert_eq!(c.via_upper_bound, c.layer_count, ":182");
    assert!(!c.ripup_allowed, ":184");
    assert_eq!(c.ripup_costs, 1000, ":185");
    assert_eq!(c.ripup_pass_no, 1, ":186");

    assert!(!c.is_fanout, ":163");
    assert_eq!(c.fanout_start_pin_name, None, ":164");
    assert_eq!(c.fanout_start_pin_center, None, ":165");
    assert_eq!(c.fanout_start_pin_layer, -1, ":166");
    assert!(c.remove_unconnected_vias, ":167");

    assert_eq!(c.add_via_costs.len(), c.layer_count);
    assert!(
        c.add_via_costs
            .iter()
            .all(|row| row.len() == c.layer_count && row.iter().all(|v| *v == 0))
    );
}

#[test]
fn the_two_fanout_escape_lengths_are_copied_with_javas_defaults() {
    let board = plane_board();

    let mut settings = RouterSettings::new();
    settings.set_layer_count(board.get_layer_count());
    settings.fanout = None;
    let c = control(&board, 0, &settings);
    assert_eq!(c.fanout_max_escape_length, 3000.0);
    assert_eq!(c.fanout_min_escape_length, 500.0);

    let mut settings = RouterSettings::new();
    settings.set_layer_count(board.get_layer_count());
    if let Some(fanout) = settings.fanout.as_mut() {
        fanout.max_escape_length_mm = None;
        fanout.min_escape_length_mm = None;
    }
    let c = control(&board, 0, &settings);
    assert_eq!(c.fanout_max_escape_length, 3000.0);
    assert_eq!(c.fanout_min_escape_length, 500.0);

    let mut settings = RouterSettings::new();
    settings.set_layer_count(board.get_layer_count());
    if let Some(fanout) = settings.fanout.as_mut() {
        fanout.max_escape_length_mm = Some(4.5);
        fanout.min_escape_length_mm = Some(2.5);
    }
    let c = control(&board, 0, &settings);
    assert_eq!(c.fanout_max_escape_length, 4500.0);
    assert_eq!(c.fanout_min_escape_length, 2500.0);
}

#[test]
fn the_per_layer_and_flag_copies_track_the_settings() {
    let board = plane_board();
    let mut settings = board_settings(&board);
    settings.set_bend_cost(0, 7.5);
    settings.set_bend_cost(2, 0.25);
    let c = control(&board, 0, &settings);

    assert_eq!(c.bend_costs, vec![7.5, settings.get_bend_cost(1), 0.25]);
    assert_eq!(c.vias_allowed, settings.get_vias_allowed());
    assert_eq!(c.with_neckdown, settings.get_automatic_neckdown());
    assert_eq!(c.layer_count, board.get_layer_count());
    assert_eq!(
        c.trace_costs,
        settings.get_trace_costs(),
        ":179 stores the array it was handed"
    );
}

#[test]
fn rebuild_via_info_accumulates_the_radii_it_is_run_over() {
    let board = fixture_board("Issue593-BBD_Mars-64.dsn");
    let settings = board_settings(&board);
    let mut c = control(&board, 1, &settings);
    let radii = c.via_radii.clone();
    let max = c.max_via_radius;

    c.rebuild_via_info(&board, settings.get_via_costs() * 3, 1);

    assert_eq!(c.via_radii, radii, "the max is already at its fixed point");
    assert_eq!(c.max_via_radius, max);
    assert_eq!(
        c.min_normal_via_cost,
        3.0 * 400.0,
        "but the cost tracks the new viaCosts"
    );
    assert_eq!(c.min_cheap_via_cost, 0.8 * (3.0 * 400.0));
}

#[test]
fn the_trace_costs_are_the_re_exported_settings_type() {
    let board = plane_board();
    let settings = board_settings(&board);
    let costs: Vec<ExpansionCostFactor> = settings.get_trace_costs();
    let c = AutorouteControl::new(&board, 0, &settings, 1, &costs);
    assert_eq!(c.trace_costs, costs);
}
