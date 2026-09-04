use std::collections::BTreeSet;
use std::path::PathBuf;

use fr_board::prelude::*;
use fr_geometry::{FloatPoint, IntBox, IntOctagon, IntPoint, IntVector, Point, Shape, TileShape};
use fr_router::JavaTreeSet;
use fr_router::board_ext::{combined_fallback_via_rule, sorted_unconnected_targets};
use fr_router::pipeline::{BatchFanout, EscapeStatistics, FanoutComponent, FanoutPin};
use fr_router::score::BoardStatisticsFanout;
use fr_settings::sources::DefaultSettings;
use fr_settings::{HostEnvironment, RouterSettings, SettingsSource};

const RPI: &str = "fixtures/Issue143-rpi_splitter.dsn";

fn load_board(rel_path: &str) -> Board {
    load_board_with_rules(rel_path, None).0
}

fn load_board_with_rules(rel_path: &str, rules: Option<&str>) -> (Board, bool) {
    let path: PathBuf = parity::java_dir().join(rel_path);
    let file = std::fs::File::open(&path)
        .unwrap_or_else(|e| panic!("cannot open {}: {e}", path.display()));
    let design_name = path
        .file_name()
        .expect("a file name")
        .to_string_lossy()
        .into_owned();
    let (mut board, transform) = match fr_dsn::read_board(
        file,
        None,
        Some(&design_name),
        &fr_dsn::parser::scope_parameter::DsnReadOptions::default(),
    ) {
        fr_dsn::BoardReadResult::Success {
            board,
            coordinate_transform,
            ..
        }
        | fr_dsn::BoardReadResult::OutlineMissing {
            board,
            coordinate_transform,
            ..
        } => (
            *board.unwrap_or_else(|| panic!("{design_name} produced no board")),
            coordinate_transform.expect("a coordinate transform"),
        ),
        other => panic!("{design_name} did not read: {other:?}"),
    };
    let mut applied = false;
    if let Some(rules) = rules {
        let rules_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/data")
            .join(rules);
        let rules_file = std::fs::File::open(&rules_path)
            .unwrap_or_else(|e| panic!("cannot open {}: {e}", rules_path.display()));
        let stem = design_name.strip_suffix(".dsn").unwrap_or(&design_name);
        applied = fr_dsn::rules_reader::read(rules_file, stem, &mut board, &transform, None)
            .unwrap_or_else(|e| panic!("{rules} did not read: {e:?}"));
    }
    (board, applied)
}

fn settings_for(board: &Board, sorting_order: &str) -> RouterSettings {
    let mut settings = DefaultSettings::new(&HostEnvironment::detect())
        .get_settings()
        .expect("DefaultSettings always answers a table")
        .clone();
    settings.set_layer_count(board.get_layer_count());
    settings.apply_board_specific_optimizations(board);
    settings
        .fanout
        .get_or_insert_with(Default::default)
        .pin_sorting_order = Some(sorting_order.to_string());
    settings
}

fn pin(id: u32, pin_index: i32, to_centre: f64, to_closest_on_net: f64, density: i32) -> FanoutPin {
    FanoutPin {
        pin: ItemId(id),
        pin_index,
        distance_to_component_center: to_centre,
        distance_to_closest_on_net: to_closest_on_net,
        surroundings_density: density,
    }
}

fn tree_of(pins: &[FanoutPin], order: Option<&str>) -> JavaTreeSet<FanoutPin> {
    let mut set = JavaTreeSet::new();
    for p in pins {
        set.add_by(p.clone(), |a, b| a.compare_to(b, order).cmp(&0));
    }
    set
}

fn pin_ids(set: &JavaTreeSet<FanoutPin>) -> Vec<u32> {
    set.iter().map(|p| p.pin.0).collect()
}

fn component(id: i32, smd_pin_count: i32) -> FanoutComponent {
    FanoutComponent {
        component: id,
        component_name: format!("C{id}"),
        smd_pins: JavaTreeSet::new(),
        gravity_center_of_smd_pins: FloatPoint::new(0.0, 0.0),
        smd_pin_count,
        pin_sorting_order: None,
    }
}

#[test]
fn components_sort_by_pin_count_descending_then_id() {
    let mut set: BTreeSet<FanoutComponent> = BTreeSet::new();
    for c in [
        component(7, 2),
        component(3, 5),
        component(1, 2),
        component(9, 5),
        component(4, 1),
    ] {
        assert!(set.insert(c), "distinct components never collapse");
    }
    let order: Vec<(i32, i32)> = set.iter().map(|c| (c.component, c.smd_pin_count)).collect();
    assert_eq!(order, [(3, 5), (9, 5), (1, 2), (7, 2), (4, 1)]);
}

#[test]
fn two_components_collapse_only_if_they_share_a_pin_count_and_an_id() {
    let mut set: BTreeSet<FanoutComponent> = BTreeSet::new();
    assert!(set.insert(component(4, 3)));
    assert!(!set.insert(component(4, 3)), "same count, same id");
    assert!(set.insert(component(4, 2)), "same id, different count");
    assert!(set.insert(component(5, 3)), "same count, different id");
    assert_eq!(set.len(), 3);
}

#[test]
fn pins_sort_by_the_selected_double_then_pin_index() {
    let a = pin(10, 3, 100.0, 900.0, 2);
    let b = pin(20, 1, 300.0, 100.0, 7);
    let c = pin(30, 2, 200.0, 500.0, 7);

    let pins = [a.clone(), b.clone(), c.clone()];
    assert_eq!(pin_ids(&tree_of(&pins, Some("inner_first"))), [10, 30, 20]);
    assert_eq!(pin_ids(&tree_of(&pins, Some("outer_first"))), [20, 30, 10]);
    assert_eq!(
        pin_ids(&tree_of(&pins, Some("distanceToClosestOnNet"))),
        [20, 30, 10]
    );
    assert_eq!(
        pin_ids(&tree_of(&pins, Some("surroundingsDensity"))),
        [20, 30, 10]
    );
}

#[test]
fn a_tie_on_the_selected_key_falls_through_to_pin_index_on_every_branch() {
    let low = pin(11, 1, 500.0, 500.0, 4);
    let high = pin(22, 0, 500.0, 500.0, 4);
    for order in [
        "inner_first",
        "outer_first",
        "distanceToClosestOnNet",
        "surroundingsDensity",
    ] {
        assert_eq!(
            pin_ids(&tree_of(&[low.clone(), high.clone()], Some(order))),
            [22, 11],
            "{order}: pinIndex 0 before pinIndex 1"
        );
    }
}

#[test]
fn an_unrecognised_sorting_order_falls_back_to_pin_index() {
    let pins = [
        pin(10, 2, 100.0, 900.0, 1),
        pin(20, 0, 300.0, 100.0, 9),
        pin(30, 1, 200.0, 500.0, 5),
    ];
    assert_eq!(
        pin_ids(&tree_of(&pins, Some("not_a_sorting_order"))),
        [20, 30, 10]
    );
    assert_eq!(pin_ids(&tree_of(&pins, None)), [20, 30, 10]);
}

#[test]
fn an_unrecognised_sorting_order_collapses_pins_with_equal_pin_index() {
    let first = pin(10, 4, 100.0, 100.0, 1);
    let second = pin(20, 4, 999.0, 999.0, 9);
    let set = tree_of(&[first, second], Some("not_a_sorting_order"));
    assert_eq!(set.len(), 1, "the second add answered false");
    assert_eq!(pin_ids(&set), [10], "the tree keeps the element it had");
}

#[test]
fn equal_pin_index_and_an_equal_key_collapse_under_every_sorting_order() {
    for order in [
        "inner_first",
        "outer_first",
        "distanceToClosestOnNet",
        "surroundingsDensity",
        "not_a_sorting_order",
    ] {
        let set = tree_of(
            &[pin(10, 4, 500.0, 500.0, 3), pin(20, 4, 500.0, 500.0, 3)],
            Some(order),
        );
        assert_eq!(set.len(), 1, "{order}");
        assert_eq!(pin_ids(&set), [10], "{order}");
    }
}

#[test]
fn two_pins_alone_on_their_nets_tie_and_fall_through_to_pin_index() {
    let pins = [pin(10, 1, 7.0, f64::MAX, 1), pin(20, 0, 3.0, f64::MAX, 1)];
    assert_eq!(
        pin_ids(&tree_of(&pins, Some("distanceToClosestOnNet"))),
        [20, 10]
    );
}

#[test]
fn the_constructor_orders_a_real_boards_components_and_pins_like_the_jvm() {
    let board = load_board(RPI);
    let settings = settings_for(&board, "outer_first");
    let fanout = BatchFanout::new(&board, &settings);

    assert_eq!(fanout.total_smd_pin_count, 10);
    assert_eq!(fanout.already_connected_pin_count, 1);

    let shape: Vec<(i32, i32, Vec<u32>)> = fanout
        .sorted_components
        .iter()
        .map(|c| {
            (
                c.component,
                c.smd_pin_count,
                c.smd_pins.iter().map(|p| p.pin.0).collect(),
            )
        })
        .collect();
    assert_eq!(
        shape,
        vec![
            (1, 4, vec![4, 7, 2, 3]),
            (3, 4, vec![23, 29, 22, 21]),
            (2, 2, vec![10, 16]),
        ]
    );
}

#[test]
fn a_net_with_one_pin_gets_double_max_value() {
    let board = load_board(RPI);
    let settings = settings_for(&board, "outer_first");
    let fanout = BatchFanout::new(&board, &settings);

    let lonely = fanout
        .sorted_components
        .iter()
        .flat_map(|c| c.smd_pins.iter())
        .find(|p| p.pin == ItemId(29))
        .expect("J3-VBUS is an SMD pin with a net");
    assert_eq!(lonely.distance_to_closest_on_net, f64::MAX);

    let maxed = fanout
        .sorted_components
        .iter()
        .flat_map(|c| c.smd_pins.iter())
        .filter(|p| p.distance_to_closest_on_net == f64::MAX)
        .count();
    assert_eq!(maxed, 1);
}

#[test]
fn a_component_with_no_net_carrying_smd_pin_is_not_in_the_set() {
    let board = load_board(RPI);
    let settings = settings_for(&board, "outer_first");
    let fanout = BatchFanout::new(&board, &settings);

    assert!(
        board.components.count() >= fanout.sorted_components.len(),
        "the set is a subset of the board's components"
    );
    assert!(
        fanout.sorted_components.iter().all(|c| c.smd_pin_count > 0),
        ":58 drops a component with no SMD pin"
    );
    let counted: i32 = fanout
        .sorted_components
        .iter()
        .map(|c| c.smd_pin_count)
        .sum();
    assert_eq!(
        counted, fanout.total_smd_pin_count,
        ":62-65 sums the set, not the board"
    );
}

#[test]
fn the_five_sorting_orders_do_not_all_agree_on_a_real_board() {
    let board = load_board(RPI);
    let orders = [
        "inner_first",
        "outer_first",
        "distanceToClosestOnNet",
        "surroundingsDensity",
        "not_a_sorting_order",
    ];
    let mut seen: Vec<Vec<u32>> = Vec::new();
    for order in orders {
        let settings = settings_for(&board, order);
        let fanout = BatchFanout::new(&board, &settings);
        seen.push(
            fanout
                .sorted_components
                .iter()
                .flat_map(|c| c.smd_pins.iter())
                .map(|p| p.pin.0)
                .collect(),
        );
    }
    assert_eq!(seen[0], vec![2, 3, 4, 7, 21, 22, 29, 23, 10, 16]);
    assert_eq!(seen[1], vec![4, 7, 2, 3, 23, 29, 22, 21, 10, 16]);
    assert_ne!(seen[0], seen[1], "inner_first is outer_first reversed");
    assert_ne!(seen[1], seen[2]);
    assert_ne!(seen[1], seen[3]);
    assert_eq!(seen[4], vec![2, 3, 4, 7, 21, 22, 23, 29, 10, 16]);
}

#[test]
fn targets_are_sorted_by_squared_midpoint_distance_stably() {
    let board = symmetric_board();
    let all: Vec<ItemId> = board.get_pins();
    assert_eq!(all.len(), 3, "the fixture's three pins");
    let targets: BTreeSet<ItemId> = all.iter().copied().collect();

    let sorted = sorted_unconnected_targets(&board, &FloatPoint::new(0.0, 0.0), &targets);
    let ids: Vec<u32> = sorted.iter().map(|id| id.0).collect();
    let mut ascending: Vec<u32> = targets.iter().map(|id| id.0).collect();
    ascending.sort_unstable();
    assert_eq!(
        ids[0..2],
        [ascending[1], ascending[0]],
        "the tie keeps the descending id order the ArrayList started in (quirk #44)"
    );
    assert_eq!(ids[2], ascending[2], "the far pin sorts last");

    let sorted = sorted_unconnected_targets(&board, &FloatPoint::new(900.0, 0.0), &targets);
    assert_eq!(sorted[0].0, ascending[1], "the +1000 pin is now closest");
}

fn symmetric_board() -> Board {
    let layers = || LayerStructure::new(vec![Layer::new("front", true), Layer::new("back", true)]);
    let clearance_matrix = ClearanceMatrix::get_default_instance(&layers(), 200);
    let mut rules = BoardRules::new(layers(), clearance_matrix);
    rules.set_default_trace_half_widths(30);
    let default_class = rules.get_default_net_class();
    rules.nets.add("N1", 1, false, default_class);
    rules.nets.add("N2", 1, false, default_class);
    rules.nets.add("N3", 1, false, default_class);

    let mut padstacks = Padstacks::new(layers());
    padstacks.add(
        "smd",
        vec![
            Some(Shape::Tile(TileShape::Box(IntBox::from_coords(
                -50, -50, 50, 50,
            )))),
            None,
        ],
        false,
        false,
    );
    let via_shape = Shape::Tile(TileShape::Octagon(IntOctagon::new(
        -100, -100, 100, 100, -200, 200, -200, 200,
    )));
    padstacks.add(
        "via",
        vec![Some(via_shape.clone()), Some(via_shape)],
        true,
        false,
    );

    let mut board = Board::new(
        Vec::new(),
        0,
        IntBox {
            ll: IntPoint {
                x: -8_000,
                y: -8_000,
            },
            ur: IntPoint { x: 8_000, y: 8_000 },
        },
        rules,
        BoardLibrary::new(padstacks, Packages::new()),
        Components::new(),
        Communication::default(),
    );
    let pkg = board.library.packages.add(
        "sym",
        vec![
            PackagePin::new("L", PadstackId(1), IntVector::new(-1000, 0).into(), 0.0),
            PackagePin::new("R", PadstackId(1), IntVector::new(1000, 0).into(), 0.0),
            PackagePin::new("T", PadstackId(1), IntVector::new(0, 3000).into(), 0.0),
        ],
        None,
        None,
        None,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        true,
    );
    board
        .components
        .add_with_generated_name(Some(Point::new(0, 0)), 0.0, true, pkg);
    board.insert_pin(1, 0, vec![1], 1, FixedState::Unfixed);
    board.insert_pin(1, 1, vec![1], 1, FixedState::Unfixed);
    board.insert_pin(1, 2, vec![2], 1, FixedState::Unfixed);
    board
}

#[test]
fn the_gravity_centre_is_the_mean_of_the_components_smd_pads() {
    let board = symmetric_board();
    let settings = settings_for(&board, "outer_first");
    let fanout = BatchFanout::new(&board, &settings);
    let component = fanout
        .sorted_components
        .iter()
        .next()
        .expect("one component");
    assert_eq!(component.smd_pin_count, 3);
    assert_eq!(
        component.gravity_center_of_smd_pins,
        FloatPoint::new(0.0, 1000.0)
    );
    let ordered: Vec<i32> = component.smd_pins.iter().map(|p| p.pin_index).collect();
    assert_eq!(ordered, [2, 0, 1], "outer_first: T, then L before R");
}

#[test]
fn the_combined_via_rule_holds_both_sources() {
    let mut infos = ViaInfos::new();
    infos.add(ViaInfo::new("shared", PadstackId(1), 1, false));
    infos.add(ViaInfo::new("board_only", PadstackId(2), 1, false));

    let mut net_class_rule = ViaRule::new("kicad_default");
    net_class_rule.append_via(infos.get(ViaInfoId(0)).clone());

    let mut board_rule = ViaRule::new("default");
    board_rule.append_via(infos.get(ViaInfoId(0)).clone());
    board_rule.append_via(infos.get(ViaInfoId(1)).clone());

    let combined = combined_fallback_via_rule(&net_class_rule, std::slice::from_ref(&board_rule));
    assert_eq!(combined.name, "kicad_default_fallback");
    let names: Vec<&str> = combined.iter().map(ViaInfo::get_name).collect();
    assert_eq!(
        names,
        ["shared", "board_only"],
        "`shared` is deduped, `board_only` is appended"
    );

    let combined = combined_fallback_via_rule(&net_class_rule, &[]);
    assert_eq!(combined.via_count(), 1);
}

#[test]
fn a_redeclared_via_rule_is_deduplicated_by_value() {
    let mut infos = ViaInfos::new();
    infos.add(ViaInfo::new("V", PadstackId(1), 1, false));
    let mut net_class_rule = ViaRule::new("kicad_default");
    net_class_rule.append_via(infos.get(ViaInfoId(0)).clone());

    infos.remove(ViaInfoId(0));
    infos.add(ViaInfo::new("V", PadstackId(1), 1, false));
    let mut board_rule = ViaRule::new("default");
    board_rule.append_via(infos.get(ViaInfoId(0)).clone());

    assert_eq!(
        net_class_rule.get_via(0),
        board_rule.get_via(0),
        "the two are value-equal, which is what makes this the interesting case"
    );
    let combined = combined_fallback_via_rule(&net_class_rule, std::slice::from_ref(&board_rule));
    assert_eq!(combined.via_count(), 1);
    let names: Vec<&str> = combined.iter().map(ViaInfo::get_name).collect();
    assert_eq!(names, ["V"]);
}

#[test]
fn the_combined_via_rule_dedups_a_via_the_two_rules_share() {
    let mut infos = ViaInfos::new();
    infos.add(ViaInfo::new("V", PadstackId(1), 1, false));
    let mut net_class_rule = ViaRule::new("default");
    net_class_rule.append_via(infos.get(ViaInfoId(0)).clone());
    let board_rule = net_class_rule.clone();
    let combined = combined_fallback_via_rule(&net_class_rule, std::slice::from_ref(&board_rule));
    assert_eq!(combined.via_count(), 1);
}

#[test]
fn a_net_class_keeps_its_detached_via_rule_when_a_rules_file_replaces_it() {
    let (board, applied) = load_board_with_rules(RPI, Some("ruling-h-viarule.rules"));
    assert!(applied, "the .rules file was accepted");

    assert_eq!(board.rules.via_rules.len(), 1);
    assert_eq!(board.rules.via_rules[0].name, "default");
    assert_eq!(board.rules.via_rules[0].via_count(), 0);

    let net_class = board.rules.nets.get(1).expect("net 1").get_net_class();
    let rule = board
        .rules
        .net_classes
        .get(net_class)
        .get_via_rule()
        .expect("the class had a rule before the .rules file");
    assert_eq!(rule.name, "default");
    assert_eq!(
        rule.via_count(),
        1,
        "the detached original still carries `Round1$13`"
    );
    assert_eq!(rule.get_via(0).get_name(), "Round1$13");

    let plain = load_board(RPI);
    assert_eq!(plain.rules.via_rules[0].via_count(), 1);
}

#[test]
fn the_four_target_strategy_carries_the_ripped_set_into_the_second_attempt() {
    let source = include_str!("../src/board_ext/routing_board_ext.rs");
    let body = source
        .rsplit_once("fn fanout(")
        .expect("the fanout implementation")
        .1;
    let body = body
        .split_once("\n    }\n")
        .expect("the end of the method")
        .0;
    assert_eq!(
        body.matches("let mut ripped_item_list").count(),
        1,
        ":1058 declares the set once"
    );
    assert_eq!(
        body.matches("&mut ripped_item_list").count(),
        3,
        "the closest-target attempt, the retry and the > 4 arm all take the same binding"
    );
    assert_eq!(
        body.matches("autoroute_connection(").count(),
        3,
        ":1069, :1078 and :1088"
    );
}

#[test]
#[cfg_attr(debug_assertions, ignore)]
fn fanout_escapes_every_smd_pin_of_the_corpus_board() {
    use fr_board::TimeLimit;
    use fr_board::datastructures::StopCheck;
    use fr_router::AutorouteAttemptState;
    use fr_router::board_ext::RoutingBoardExt;
    use fr_router::pipeline::RouterBudget;

    let mut board = load_board(RPI);
    let settings = settings_for(&board, "outer_first");
    let order: Vec<ItemId> = {
        let fanout = BatchFanout::new(&board, &settings);
        fanout
            .sorted_components
            .iter()
            .flat_map(|c| c.smd_pins.iter())
            .map(|p| p.pin)
            .collect()
    };
    assert_eq!(
        order.iter().map(|id| id.0).collect::<Vec<_>>(),
        vec![4, 7, 2, 3, 23, 29, 22, 21, 10, 16]
    );

    let stop: StopCheck<'_> = &|| false;
    let mut engine = None;
    let mut states = Vec::new();
    for pin in &order {
        board.start_marking_changed_area();
        let result = board.fanout(
            &mut engine,
            *pin,
            &settings,
            settings.get_start_ripup_costs(),
            stop,
            Some(TimeLimit::new(i32::MAX)),
            RouterBudget::disabled(),
        );
        states.push(result.state);
    }
    assert_eq!(
        states,
        vec![
            AutorouteAttemptState::Routed,
            AutorouteAttemptState::Routed,
            AutorouteAttemptState::Routed,
            AutorouteAttemptState::Routed,
            AutorouteAttemptState::Routed,
            AutorouteAttemptState::NoUnconnectedNets,
            AutorouteAttemptState::Routed,
            AutorouteAttemptState::Routed,
            AutorouteAttemptState::Routed,
            AutorouteAttemptState::Routed,
        ]
    );
    assert_eq!(board.get_vias().len(), 9);
    assert_eq!(board.get_traces().len(), 10);
}

#[test]
fn escape_statistics_come_from_the_board_statistics_fanout_block() {
    let stats = BoardStatisticsFanout {
        total_smd_pins: 8,
        pins_to_escape: 5,
        escaped_count: 3,
    };
    let escape = EscapeStatistics::from_fanout_statistics(&stats);
    assert_eq!(escape.total_smd_pins, 8);
    assert_eq!(escape.escaped_count, 3);
    assert!((escape.escaped_percentage - 37.5).abs() < f64::EPSILON);

    let empty = EscapeStatistics::from_fanout_statistics(&BoardStatisticsFanout::default());
    assert_eq!(empty.escaped_percentage, 0.0, ":596's guard, not NaN");
}
