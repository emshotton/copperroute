//! Plan 7 Task 11 — `BatchFanout`'s component/pin ordering (`BatchFanout.java:35-78`, `:631-693`,
//! `:695-778`) and `RoutingBoard.fanout` (`RoutingBoard.java:978-1110`).
//!
//! # What is pinned here, and what is pinned by `p7t5`
//!
//! The whole-board evidence is the differential driver:
//! `scripts/differential/run.sh p7t5 <dsn> <passNo> <sortingOrder> <order|pin>` compares
//! `sortedComponents × smdPins` for **all five** `pinSortingOrder` strings, and then the real
//! `RoutingBoard.fanout` on every SMD pin of the board, against the HEAD jar — 8 DSNs × 2 modes,
//! 16/16 MATCH. That is where "the order and the escape router agree with Java" is established.
//!
//! This file pins the four things a corpus run cannot show:
//!
//! * the **comparator itself**, branch by branch, including inputs no board produces (two pins at
//!   the same `pinIndex`, which is the only way `Pin.compareTo` can answer `0`);
//! * the `Double.MAX_VALUE` seed for a net with one pin (**quirk #219**) — the corpus *does* reach
//!   it, and the test names the pin;
//! * the **tie** in the target sort, which needs a symmetric board;
//! * the `contains` **identity** dedup in the fallback via rule (**quirk #218**) and the net class
//!   keeping its **detached** via rule (ruling H's via-rule half) — both need a `.rules`
//!   re-declaration, which no acceptance fixture carries.

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

// =================================================================================================
// Fixtures
// =================================================================================================

/// `Issue143-rpi_splitter.dsn` — `p7t5`'s default stem, ten SMD pins over three components, and
/// the one corpus board whose net class holds the **first** via rule of its name (see
/// [`a_net_class_keeps_its_detached_via_rule_when_a_rules_file_replaces_it`]).
const RPI: &str = "fixtures/Issue143-rpi_splitter.dsn";

fn load_board(rel_path: &str) -> Board {
    load_board_with_rules(rel_path, None).0
}

/// The DSN, then the optional `.rules` file — `P6T1.loadBoard`'s two halves.
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

/// A [`FanoutPin`] built field by field, so the comparator can be exercised on inputs no board
/// produces. Java's constructor computes these four from the board; `compareTo` reads nothing
/// else.
fn pin(id: u32, pin_index: i32, to_centre: f64, to_closest_on_net: f64, density: i32) -> FanoutPin {
    FanoutPin {
        pin: ItemId(id),
        pin_index,
        distance_to_component_center: to_centre,
        distance_to_closest_on_net: to_closest_on_net,
        surroundings_density: density,
    }
}

/// The `JavaTreeSet` a `FanoutComponent` holds, filled the way
/// `BatchFanout.Component`'s constructor fills it (`:675-677`) — insertion order matters, because
/// a comparator that answers `Equal` keeps the element already in the tree.
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

/// A [`FanoutComponent`] with only the two keys `compareTo` reads (`:683`, `:690`).
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

// =================================================================================================
// `Component.compareTo` — BatchFanout.java:680-693
// =================================================================================================

/// "Sort the components, so that components with more pins come first" (`:680`) — pin count
/// **descending**, ties broken by `boardComponent.id` **ascending** (`:690`).
#[test]
fn components_sort_by_pin_count_descending_then_id() {
    let mut set: BTreeSet<FanoutComponent> = BTreeSet::new();
    // Inserted in an order that is neither the answer nor its reverse.
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

/// Java's `Component` declares no `equals`, so its `TreeSet` membership is `compareTo` alone; the
/// port's `Eq` is `Ord`-consistent for exactly that reason, and a `BTreeSet` is therefore Java's
/// container here (ruling 5's recorded decision, confirmed).
#[test]
fn two_components_collapse_only_if_they_share_a_pin_count_and_an_id() {
    let mut set: BTreeSet<FanoutComponent> = BTreeSet::new();
    assert!(set.insert(component(4, 3)));
    assert!(!set.insert(component(4, 3)), "same count, same id");
    assert!(set.insert(component(4, 2)), "same id, different count");
    assert!(set.insert(component(5, 3)), "same count, different id");
    assert_eq!(set.len(), 3);
}

// =================================================================================================
// `Component.Pin.compareTo` — BatchFanout.java:741-777
// =================================================================================================

/// The three `double` branches (`:744-764`) and the `int` one (`:765-771`), each with the
/// `pinIndex` tie-break behind it (`:773-775`).
#[test]
fn pins_sort_by_the_selected_double_then_pin_index() {
    // id, pinIndex, distToCentre, distToClosestOnNet, density
    let a = pin(10, 3, 100.0, 900.0, 2);
    let b = pin(20, 1, 300.0, 100.0, 7);
    let c = pin(30, 2, 200.0, 500.0, 7);

    let pins = [a.clone(), b.clone(), c.clone()];
    // `inner_first` (`:744-750`): ascending distance to the component centre.
    assert_eq!(pin_ids(&tree_of(&pins, Some("inner_first"))), [10, 30, 20]);
    // `outer_first` (`:751-757`): the same difference, the opposite sign.
    assert_eq!(pin_ids(&tree_of(&pins, Some("outer_first"))), [20, 30, 10]);
    // `distanceToClosestOnNet` (`:758-764`): ascending.
    assert_eq!(
        pin_ids(&tree_of(&pins, Some("distanceToClosestOnNet"))),
        [20, 30, 10]
    );
    // `surroundingsDensity` (`:765-771`): **densest first**, because the subtraction is the other
    // way round — and `b` and `c` tie at 7, so `pinIndex` decides (1 before 2).
    assert_eq!(
        pin_ids(&tree_of(&pins, Some("surroundingsDensity"))),
        [20, 30, 10]
    );
}

/// `:773-775`'s tie-break is **outside** the `if`/`else if` chain, so it runs on every branch.
/// Two pins at the same selected key are ordered by `pinIndex`, ascending, whatever the key was.
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

/// An **unrecognised** `pinSortingOrder` matches none of the four `String.equals` tests, so
/// `result` is still `0` at `:773` and the order becomes **pure `pinIndex`** — quirk **#220**.
///
/// A `null` `pinSortingOrder` behaves identically, because Java tests
/// `"inner_first".equals(pinSortingOrder)` with the *constant* as the receiver.
#[test]
fn an_unrecognised_sorting_order_falls_back_to_pin_index() {
    // Deliberately mutually inconsistent keys: whichever branch had run, the answer would differ.
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

/// The `TreeSet` drop, and the reason the container is a [`JavaTreeSet`].
///
/// `Pin.compareTo` answers `0` for two **distinct** pins only when the selected key ties *and*
/// `pinIndex` ties — and `java.util.TreeSet.add` then keeps the element already in the tree and
/// answers `false`. No board produces that input, because `pinIndex` is "the index of the pin in
/// its component" (`board/model/items/Pin.java:49`) and a component holds one pin per index; but
/// that is a property of the input, not of the comparator, so the container stays Java's.
#[test]
fn an_unrecognised_sorting_order_collapses_pins_with_equal_pin_index() {
    let first = pin(10, 4, 100.0, 100.0, 1);
    let second = pin(20, 4, 999.0, 999.0, 9);
    let set = tree_of(&[first, second], Some("not_a_sorting_order"));
    assert_eq!(set.len(), 1, "the second add answered false");
    assert_eq!(pin_ids(&set), [10], "the tree keeps the element it had");
}

/// The same collapse under a **recognised** order, which is the half the plan's ruling 8 did not
/// state: the drop is caused by the `pinIndex` tie at `:774`, not by the string not matching.
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

/// `:759`'s `this.distanceToClosestOnNet - other.distanceToClosestOnNet` is a **difference**, and
/// `Double.MAX_VALUE - Double.MAX_VALUE` is `0.0`, so two pins alone on their nets tie and fall
/// through to `pinIndex`. Transcribing the difference matters: `total_cmp` would agree here, but
/// the shape is Java's and the next reader does not have to re-derive it.
#[test]
fn two_pins_alone_on_their_nets_tie_and_fall_through_to_pin_index() {
    let pins = [pin(10, 1, 7.0, f64::MAX, 1), pin(20, 0, 3.0, f64::MAX, 1)];
    assert_eq!(
        pin_ids(&tree_of(&pins, Some("distanceToClosestOnNet"))),
        [20, 10]
    );
}

// =================================================================================================
// The constructor, on a real board — BatchFanout.java:35-78, :642-678, :702-739
// =================================================================================================

/// The literals are the HEAD jar's, read out of `scripts/differential/run.sh p7t5
/// <rpi> 0 outer_first order` — the same run the driver compares byte for byte.
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
            // J1 and J3 both hold four SMD pins, so the id decides; J2 holds two.
            (1, 4, vec![4, 7, 2, 3]),
            (3, 4, vec![23, 29, 22, 21]),
            (2, 2, vec![10, 16]),
        ]
    );
}

/// `:711`'s `double minDistance = Double.MAX_VALUE;` survives when the pin's net has no other
/// pin — and the corpus reaches it: `J3-VBUS` (item 29, net 4) is the only pin on its net, and the
/// JVM prints `1.7976931348623157E308` for it. Quirk **#219**.
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

    // Every other SMD pin on this board shares its net with at least one more pin, so `MAX` is a
    // sentinel here rather than the common answer.
    let maxed = fanout
        .sorted_components
        .iter()
        .flat_map(|c| c.smd_pins.iter())
        .filter(|p| p.distance_to_closest_on_net == f64::MAX)
        .count();
    assert_eq!(maxed, 1);
}

/// `:43-51` — "filter out SMD pins that belong to no net", and `:58`'s `smdPinCount > 0`, which
/// together decide which components are in the set at all.
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

/// The order changes with `pinSortingOrder`, so the `p7t5 order` mode is measuring something: a
/// port that ignored the setting would pass every other test in this file.
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
    // The unrecognised string is pure `pinIndex` within each component.
    assert_eq!(seen[4], vec![2, 3, 4, 7, 21, 22, 23, 29, 10, 16]);
}

// =================================================================================================
// `RoutingBoard.fanout:1002-1021` — the target sort
// =================================================================================================

/// The key is the **squared** distance from the pin centre to each item's bounding-box midpoint,
/// and `List.sort` is stable, so equidistant targets keep the `TreeSet<Item>`'s ascending id
/// order.
#[test]
fn targets_are_sorted_by_squared_midpoint_distance_stably() {
    let board = symmetric_board();
    // The three pins of the symmetric component sit at (-1000, 0), (+1000, 0) and (0, 3000).
    let all: Vec<ItemId> = board.get_pins();
    assert_eq!(all.len(), 3, "the fixture's three pins");
    let targets: BTreeSet<ItemId> = all.iter().copied().collect();

    // From the origin, the two flanking pins are equidistant and the third is further away.
    let sorted = sorted_unconnected_targets(&board, &FloatPoint::new(0.0, 0.0), &targets);
    let ids: Vec<u32> = sorted.iter().map(|id| id.0).collect();
    let mut ascending: Vec<u32> = targets.iter().map(|id| id.0).collect();
    ascending.sort_unstable();
    assert_eq!(
        ids[0..2],
        ascending[0..2],
        "the tie keeps the ascending id order the ArrayList started in"
    );
    assert_eq!(ids[2], ascending[2], "the far pin sorts last");

    // Moving the centre next to one of the tied pins breaks the tie in its favour.
    let sorted = sorted_unconnected_targets(&board, &FloatPoint::new(900.0, 0.0), &targets);
    assert_eq!(sorted[0].0, ascending[1], "the +1000 pin is now closest");
}

/// A two-layer board with one component whose three SMD pads are symmetric about the origin, and
/// three nets — two shared, one with a single pin. Not a Java fixture: it exists because no corpus
/// board produces a tie in either sort.
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

/// The symmetric fixture also gives `Component.Pin` a tie on `distanceToComponentCenter`: `L` and
/// `R` are both 1000 units from the gravity centre `(0, 1000)`… they are not, and that is the
/// point — the gravity centre is the mean of the three pads, so the test asserts what the
/// constructor actually computes rather than what a symmetric picture suggests.
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
    // (-1000 + 1000 + 0) / 3, (0 + 0 + 3000) / 3.
    assert_eq!(
        component.gravity_center_of_smd_pins,
        FloatPoint::new(0.0, 1000.0)
    );
    // `L` and `R` are equidistant from it, so `pinIndex` decides between them.
    let ordered: Vec<i32> = component.smd_pins.iter().map(|p| p.pin_index).collect();
    assert_eq!(ordered, [2, 0, 1], "outer_first: T, then L before R");
}

// =================================================================================================
// `RoutingBoard.fanout:1026-1041` — the fallback via rule, and `ViaRule.contains`
// =================================================================================================

/// `:1028-1041` — the combined rule holds the net class's vias **first**, then every via of
/// `rules.viaRules.firstElement()` it does not already hold.
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

    // `:1034`'s `isEmpty()` guard: with no board rule at all the combined rule is the net class's.
    let combined = combined_fallback_via_rule(&net_class_rule, &[]);
    assert_eq!(combined.via_count(), 1);
}

/// **Quirk #218.** `:1037`'s `contains` is Java's `==`, i.e. object identity, so two
/// *value-equal but separately registered* `ViaInfo`s are **both** appended. A value comparison
/// would keep one and hand `rebuildViaInfo` a shorter rule.
///
/// The repro is the one ruling H is about: a `.rules` file re-declares a `(via …)` with identical
/// values — `RulesReader.applyViaInfo` (`:340-350`) registers a **new** object — and then
/// re-declares the `(via_rule …)` naming it, while the net class keeps the detached original.
#[test]
fn the_combined_via_rule_appends_a_value_equal_via_from_a_second_rule() {
    let mut infos = ViaInfos::new();
    infos.add(ViaInfo::new("V", PadstackId(1), 1, false));
    // The net class's rule holds the **original** object.
    let mut net_class_rule = ViaRule::new("kicad_default");
    net_class_rule.append_via(infos.get(ViaInfoId(0)).clone());

    // `applyViaInfo` replaces it with a value-equal new object …
    infos.remove(ViaInfoId(0));
    infos.add(ViaInfo::new("V", PadstackId(1), 1, false));
    // … and the board's rule is rebuilt from the new list.
    let mut board_rule = ViaRule::new("default");
    board_rule.append_via(infos.get(ViaInfoId(0)).clone());

    assert_eq!(
        net_class_rule.get_via(0),
        board_rule.get_via(0),
        "the two are value-equal, which is what makes this the interesting case"
    );
    assert!(
        !net_class_rule
            .get_via(0)
            .is_same_object(board_rule.get_via(0)),
        "…and separately registered, so Java's `==` says no"
    );

    let combined = combined_fallback_via_rule(&net_class_rule, std::slice::from_ref(&board_rule));
    assert_eq!(
        combined.via_count(),
        2,
        "identity dedup appends the duplicate; a value dedup would answer 1"
    );
    let names: Vec<&str> = combined.iter().map(ViaInfo::get_name).collect();
    assert_eq!(names, ["V", "V"]);
}

/// The other half of the same guard: a via the two rules genuinely share — the ordinary case, and
/// every corpus board's — is deduped, so `contains` is not simply "always false".
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

// =================================================================================================
// Ruling H, via-rule half — `Network.addViaRule:413-417` vs `NetClass.viaRule`
// =================================================================================================

/// `Network.addViaRule` removes the replaced rule from `board.rules.viaRules` and appends the new
/// one; `NetClass.viaRule` (`NetClass.java:28`) is an object reference and is **not** rewritten,
/// so a class that pointed at the replaced rule keeps the **detached original** — which is what
/// `AutorouteControl.initNet:210` reads.
///
/// `Issue143-rpi_splitter.dsn` is the corpus board that shows it: it carries exactly one via rule,
/// named `default`, and its one net class holds that rule. Every other corpus stem carries **two**
/// rules named `default` and binds the class to the second, so `BoardRules::get_via_rule`'s
/// first-match lookup can only ever reach the orphan — which is why the Task 0 review's three
/// `Issue593` repros all MATCHed.
///
/// Measured, not argued: with this fixture and the port re-pointing (the state before this task),
/// `scripts/differential/run.sh p6t1 …/Issue143-rpi_splitter.dsn 8 1
/// crates/fr-router/tests/data/ruling-h-viarule.rules` DIFFed on **all eight** connections — the
/// jar placed nine vias, the port none.
#[test]
fn a_net_class_keeps_its_detached_via_rule_when_a_rules_file_replaces_it() {
    let (board, applied) = load_board_with_rules(RPI, Some("ruling-h-viarule.rules"));
    assert!(applied, "the .rules file was accepted");

    // The board's list holds only the replacement, which the `(via_rule default)` scope left empty.
    assert_eq!(board.rules.via_rules.len(), 1);
    assert_eq!(board.rules.via_rules[0].name, "default");
    assert_eq!(board.rules.via_rules[0].via_count(), 0);

    // The net class still holds the original, vias and all.
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

    // Without the `.rules` file the two agree — the divergence needs the re-declaration.
    let plain = load_board(RPI);
    assert_eq!(plain.rules.via_rules[0].via_count(), 1);
}

// =================================================================================================
// `RoutingBoard.fanout` — the two-attempt strategy, and the whole method on a real board
// =================================================================================================

/// **Quirk #221.** For four targets or fewer, `:1064-1085` routes to the closest target alone and
/// then, on anything but `ROUTED`/`ALREADY_CONNECTED`, retries against the **whole** unconnected
/// set — with the **same** `rippedItemList`. So items the abandoned first attempt ripped are
/// carried into the second attempt, where
/// `AutorouteEngine.autorouteConnection:237-245` deletes their whole connections from the board.
///
/// Java's `rippedItemList` is a local of `fanout` (`:1058`) and is never returned, so neither
/// language can observe the set itself — only the deletions it causes, which is what `p7t5 pin`'s
/// `"removed"` column prints. The retry does not fire on any of the eight `p7t5` boards, so this
/// test pins the **transcription**: one binding, handed to both calls.
#[test]
fn the_four_target_strategy_carries_the_ripped_set_into_the_second_attempt() {
    let source = include_str!("../src/board_ext/routing_board_ext.rs");
    // `rsplit_once`: the trait declaration comes first and has no body, the `impl` second.
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

/// `RoutingBoard.fanout` end to end on the corpus board, in `p7t5 pin`'s own walk order. The
/// literals are the HEAD jar's, from `run.sh p7t5 <rpi> 0 outer_first pin`.
///
/// Release-only: it routes ten escapes through the real maze.
#[test]
#[cfg_attr(debug_assertions, ignore)]
fn fanout_escapes_every_smd_pin_of_the_corpus_board_like_the_jvm() {
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
            // `BatchFanout.fanoutPass:173` at `passNo = 0`.
            settings.get_start_ripup_costs(),
            stop,
            Some(TimeLimit::new(i32::MAX)),
            RouterBudget::disabled(),
        );
        states.push(result.state);
    }
    // Nine escapes, and `J3-VBUS` (item 29) is alone on net 4 with nothing to reach.
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
    assert_eq!(board.get_traces().len(), 11);
}

// =================================================================================================
// `EscapeStatistics` — BatchFanout.java:591-606
// =================================================================================================

/// `fromBoardStatistics` (`:594-600`): the percentage is `escapedCount * 100.0 / totalSmdPins`,
/// and the `> 0` guard is what keeps an empty board from answering `NaN`.
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
