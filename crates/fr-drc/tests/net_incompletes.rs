use std::collections::{BTreeMap, BTreeSet};

use fr_board::prelude::*;
use fr_drc::{DesignRulesChecker, NetIncompletes, UnconnectedKind};
use fr_dsn::{BoardReadResult, DsnReadOptions};
use fr_geometry::{FloatPoint, IntBox, IntPoint, IntVector, Point, Polyline, Shape, TileShape};

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

const FIXTURES: [&str; 3] = [
    "Issue575-drc_dev-board_4_hole_clearance_violations",
    "Issue575-drc_BBD_Mars-64_6_track_1_hole_clearance_violations",
    "Issue575-drc_Natural_Tone_Preamp_7_unconnected_items",
];

fn net_item_lists(board: &Board) -> Vec<Vec<ItemId>> {
    let max_net_no = board.rules.nets.max_net_number();
    let mut lists: Vec<Vec<ItemId>> = vec![Vec::new(); max_net_no.max(0) as usize];
    for item in board.get_items() {
        if !item.is_connectable() {
            continue;
        }
        for i in 0..item.net_count() {
            let net_number = item.get_net_number(i);
            if net_number >= 1 && net_number <= max_net_no {
                lists[net_number as usize - 1].push(item.id());
            }
        }
    }
    lists
}

fn all_net_incompletes(board: &Board) -> Vec<NetIncompletes> {
    net_item_lists(board)
        .iter()
        .enumerate()
        .map(|(i, items)| NetIncompletes::new(i as i32 + 1, items, board))
        .collect()
}

#[test]
fn the_airline_endpoints_are_a_hash_dependent_choice() {
    if !parity::require_reference_dir() {
        return;
    }
    for stem in FIXTURES {
        let board = fixture_board(&format!("{stem}.dsn"));
        let jvm = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/data")
                .join(format!("{stem}.airlines.txt")),
        )
        .expect("the JVM airline dump is committed next to the probe");

        let mut jvm_by_net: BTreeMap<i32, Vec<Line>> = BTreeMap::new();
        for text in jvm.lines().filter(|l| !l.is_empty()) {
            let field = |key: &str| {
                text.split_whitespace()
                    .find_map(|f| f.strip_prefix(key))
                    .unwrap_or_else(|| panic!("no {key} in {text:?}"))
            };
            let corner = |key: &str| {
                let (x, y) = field(key).split_once(',').expect("x,y");
                FloatPoint::new(x.parse().unwrap(), y.parse().unwrap())
            };
            jvm_by_net
                .entry(field("net=").parse().unwrap())
                .or_default()
                .push(Line {
                    ends: (
                        field("from=").parse().unwrap(),
                        field("to=").parse().unwrap(),
                    ),
                    length_square: corner("fromCorner=").distance_square(&corner("toCorner=")),
                });
        }

        let mut port_by_net: BTreeMap<i32, Vec<Line>> = BTreeMap::new();
        for net_incompletes in all_net_incompletes(&board) {
            for airline in &net_incompletes.incompletes {
                port_by_net
                    .entry(airline.net_number)
                    .or_default()
                    .push(Line {
                        ends: (airline.from_item.0, airline.to_item.0),
                        length_square: airline.to_corner.distance_square(&airline.from_corner),
                    });
            }
        }

        let counts = |by_net: &BTreeMap<i32, Vec<Line>>| -> Vec<(i32, usize)> {
            by_net.iter().map(|(&n, v)| (n, v.len())).collect()
        };
        assert_eq!(
            counts(&jvm_by_net),
            counts(&port_by_net),
            "{stem}: per-net airline counts",
        );

        let mut differing = 0usize;
        let mut same_weight = 0usize;
        for (net, jvm_lines) in &jvm_by_net {
            let port_lines = &port_by_net[net];
            let unmatched = port_lines
                .iter()
                .filter(|line| !jvm_lines.iter().any(|other| line.same_ends(other)))
                .count();
            if unmatched == 0 {
                continue;
            }
            differing += unmatched;
            let weights = |lines: &[Line]| {
                let mut w: Vec<String> = lines
                    .iter()
                    .map(|l| fr_dsn::format_double(l.length_square))
                    .collect();
                w.sort();
                w
            };
            let (port_weights, jvm_weights) = (weights(port_lines), weights(jvm_lines));
            if port_weights == jvm_weights {
                same_weight += unmatched;
            }
            println!(
                "  net={net}: {unmatched} of {} differ; equal-weight tree: {}",
                port_lines.len(),
                port_weights == jvm_weights,
            );
        }
        println!(
            "{stem}: {differing} of {} airlines have endpoints the committed JVM run did not \
             pick, {same_weight} of them inside an equal-weight spanning tree",
            counts(&port_by_net).iter().map(|(_, c)| c).sum::<usize>(),
        );
    }
}

struct Line {
    ends: (u32, u32),
    length_square: f64,
}

impl Line {
    fn same_ends(&self, other: &Line) -> bool {
        self.ends == other.ends || self.ends == (other.ends.1, other.ends.0)
    }
}

#[test]
fn every_port_airline_is_one_some_jvm_run_picks() {
    if !parity::require_reference_dir() {
        return;
    }
    for stem in FIXTURES {
        let board = fixture_board(&format!("{stem}.dsn"));
        let union_text = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/data")
                .join(format!("{stem}.airlines-union.txt")),
        )
        .expect("the union of the six JVM runs is committed next to the probe");

        let mut union: BTreeSet<(i32, u32, u32)> = BTreeSet::new();
        for text in union_text.lines().filter(|l| !l.is_empty()) {
            let field = |key: &str| -> u32 {
                text.split_whitespace()
                    .find_map(|f| f.strip_prefix(key))
                    .unwrap_or_else(|| panic!("no {key} in {text:?}"))
                    .parse()
                    .expect("an integer")
            };
            union.insert((field("net=") as i32, field("a="), field("b=")));
        }

        let mut checked = 0usize;
        for net_incompletes in all_net_incompletes(&board) {
            for airline in &net_incompletes.incompletes {
                let (a, b) = (airline.from_item.0, airline.to_item.0);
                let key = (airline.net_number, a.min(b), a.max(b));
                assert!(
                    union.contains(&key),
                    "{stem}: airline {key:?} was picked by none of the six JVM runs",
                );
                checked += 1;
            }
        }
        assert!(checked > 0, "{stem}: no airlines to check");
        println!("{stem}: all {checked} port airlines are in the six-run JVM union");
    }
}

#[test]
fn an_isolated_pin_still_reaches_the_ratsnest() {
    let mut board = isolated_pin_board();
    assert!(!board.is_tail(ItemId(2)), "the pin is not a tail");
    assert!(board.is_tail(ItemId(3)), "the free-floating trace is");
    assert!(board.normal_contacts(ItemId(2)).is_empty());

    let net_incompletes = NetIncompletes::new(1, &[ItemId(2), ItemId(3)], &board);
    assert_eq!(net_incompletes.get_connected_group_count(), 1);
    assert_eq!(net_incompletes.count(), 0);
    assert!(net_incompletes.incompletes.is_empty());

    let empty = NetIncompletes::new(1, &[ItemId(3)], &board);
    assert_eq!(empty.get_connected_group_count(), 0);
    assert_eq!(empty.count(), 0);

    let _ = DesignRulesChecker::new(&mut board);
}

#[test]
fn dangling_items_are_filtered_before_triangulation() {
    let mut board = dangling_via_board();
    assert!(board.is_tail(ItemId(4)), "the free-standing via is a tail");

    let with_via = NetIncompletes::new(1, &[ItemId(2), ItemId(3), ItemId(4)], &board);
    assert_eq!(with_via.get_connected_group_count(), 2);
    assert_eq!(with_via.count(), 1);
    let airline = &with_via.incompletes[0];
    assert_eq!(airline.net_number, 1);
    assert_eq!(
        [airline.from_item, airline.to_item],
        [ItemId(3), ItemId(2)],
        "the airline connects the two single-item pin components; its endpoint order follows the \
         seeded Delaunay triangulation edge",
    );

    let dangling: Vec<ItemId> = DesignRulesChecker::new(&mut board)
        .get_all_unconnected_items()
        .into_iter()
        .filter(|e| e.kind == UnconnectedKind::ViaDangling)
        .map(|e| e.first_item)
        .collect();
    assert_eq!(dangling, vec![ItemId(4)]);
}

#[test]
fn length_violation_is_zero_without_a_net_class_restriction() {
    let board = complete_net_board();
    let mut net_incompletes = NetIncompletes::new(1, &[ItemId(2), ItemId(3), ItemId(4)], &board);
    assert_eq!(net_incompletes.get_length_violation(), 0.0);
    assert!(!net_incompletes.calc_length_violation(&board));
    assert_eq!(net_incompletes.get_length_violation(), 0.0);
}

#[test]
fn length_violation_is_positive_when_too_long() {
    let mut board = complete_net_board();
    assert_eq!(board.net_trace_length(1), 5000.0);
    set_trace_length_limits(&mut board, 0.0, 1000.0);

    let mut net_incompletes = NetIncompletes::new(1, &[ItemId(2), ItemId(3), ItemId(4)], &board);
    assert_eq!(net_incompletes.get_length_violation(), 4000.0);
    assert!(!net_incompletes.calc_length_violation(&board));
    assert_eq!(net_incompletes.get_length_violation(), 4000.0);
}

#[test]
fn length_violation_is_negative_only_when_the_net_is_complete() {
    let mut complete = complete_net_board();
    set_trace_length_limits(&mut complete, 12_000.0, 0.0);
    let net_incompletes = NetIncompletes::new(1, &[ItemId(2), ItemId(3), ItemId(4)], &complete);
    assert_eq!(net_incompletes.count(), 0, "one connected group");
    assert_eq!(net_incompletes.get_length_violation(), -7000.0);

    let mut incomplete = dangling_via_board();
    assert_eq!(incomplete.net_trace_length(1), 0.0);
    set_trace_length_limits(&mut incomplete, 12_000.0, 0.0);
    let net_incompletes = NetIncompletes::new(1, &[ItemId(2), ItemId(3)], &incomplete);
    assert_eq!(net_incompletes.count(), 1);
    assert_eq!(net_incompletes.get_length_violation(), 0.0);
}

#[test]
fn length_violation_change_threshold_is_one_tenth() {
    let mut board = lone_pin_board();
    assert_eq!(board.net_trace_length(1), 0.0);
    let mut net_incompletes = NetIncompletes::new(1, &[ItemId(2)], &board);
    assert!(net_incompletes.incompletes.is_empty());
    assert_eq!(net_incompletes.get_length_violation(), 0.0);

    set_trace_length_limits(&mut board, 0.1, 0.0);
    assert!(!net_incompletes.calc_length_violation(&board));
    assert_eq!(net_incompletes.get_length_violation(), -0.1);

    set_trace_length_limits(&mut board, 0.2, 0.0);
    assert!(!net_incompletes.calc_length_violation(&board));
    assert_eq!(net_incompletes.get_length_violation(), -0.2);

    set_trace_length_limits(&mut board, 0.5, 0.0);
    assert!(net_incompletes.calc_length_violation(&board));
    assert_eq!(net_incompletes.get_length_violation(), -0.5);

    set_trace_length_limits(&mut board, 0.0, 0.0);
    assert!(!net_incompletes.calc_length_violation(&board));
    assert_eq!(net_incompletes.get_length_violation(), 0.0);
}

#[test]
fn the_marker_radius_is_twice_the_minimum_trace_half_width() {
    let board = complete_net_board();
    let net_incompletes = NetIncompletes::new(1, &[ItemId(2)], &board);
    assert_eq!(
        net_incompletes.get_marker_radius(),
        f64::from(board.rules.get_min_trace_half_width()) * 2.0,
    );
}

#[test]
fn a_net_number_with_no_net_has_no_length_restriction() {
    let board = complete_net_board();
    let mut net_incompletes = NetIncompletes::new(99, &[], &board);
    assert_eq!(net_incompletes.get_net_number(), 99);
    assert!(!net_incompletes.calc_length_violation(&board));
    assert_eq!(net_incompletes.get_length_violation(), 0.0);
}

const BOUNDING_BOX: IntBox = IntBox {
    ll: IntPoint {
        x: -10_000,
        y: -10_000,
    },
    ur: IntPoint {
        x: 10_000,
        y: 10_000,
    },
};

fn layers() -> LayerStructure {
    LayerStructure::new(vec![Layer::new("front", true)])
}

fn bare_board(pin_offsets: &[i32]) -> (Board, PadstackId) {
    let mut padstacks = Padstacks::new(layers());
    let mut pins = Vec::new();
    for (i, offset) in pin_offsets.iter().enumerate() {
        let shape = Shape::Tile(TileShape::Box(IntBox::from_coords(-50, -50, 50, 50)));
        let name = format!("p{i}");
        let padstack = padstacks.add(&name, vec![Some(shape)], false, false);
        pins.push(PackagePin::new(
            &name,
            padstack,
            IntVector::new(*offset, 0).into(),
            0.0,
        ));
    }
    let via_shape = Shape::Tile(TileShape::Box(IntBox::from_coords(-40, -40, 40, 40)));
    let via_padstack = padstacks.add("via", vec![Some(via_shape)], false, false);

    let mut packages = Packages::new();
    let package = packages.add(
        "pkg",
        pins,
        None,
        None,
        None,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        true,
    );
    let mut components = Components::new();
    components.add_with_generated_name(Some(Point::new(0, 0)), 0.0, true, package);

    let ls = layers();
    let matrix = ClearanceMatrix::get_default_instance(&ls, 200);
    let mut rules = BoardRules::new(ls, matrix);
    rules.create_default_net_class();
    let default_class = rules.get_default_net_class();
    let mut board = Board::new(
        Vec::new(),
        1,
        BOUNDING_BOX,
        rules,
        BoardLibrary::new(padstacks, packages),
        components,
        Communication::default(),
    );
    board.rules.nets.add("N1", 1, false, default_class);
    board.rules.nets.add("A0", 1, false, default_class);
    (board, via_padstack)
}

fn insert_trace(board: &mut Board, from: (i32, i32), to: (i32, i32)) -> ItemId {
    board
        .insert_trace_without_cleaning(
            Polyline::from_points(&[Point::new(from.0, from.1), Point::new(to.0, to.1)]),
            0,
            30,
            vec![1],
            1,
            FixedState::Unfixed,
        )
        .expect("the synthetic trace is neither degenerate nor closed")
}

fn set_trace_length_limits(board: &mut Board, minimum: f64, maximum: f64) {
    let class = board.rules.net_classes.get_mut(NetClassId(0));
    class.set_minimum_trace_length(minimum);
    class.set_maximum_trace_length(maximum);
}

fn isolated_pin_board() -> Board {
    let (mut board, _) = bare_board(&[0]);
    board.insert_pin(1, 0, vec![1], 1, FixedState::Unfixed);
    insert_trace(&mut board, (5000, 5000), (6000, 5000));
    board
}

fn dangling_via_board() -> Board {
    let (mut board, via_padstack) = bare_board(&[0, 5000]);
    board.insert_pin(1, 0, vec![1], 1, FixedState::Unfixed);
    board.insert_pin(1, 1, vec![1], 1, FixedState::Unfixed);
    board
        .insert_via(
            via_padstack,
            Point::new(8000, 8000),
            vec![1],
            1,
            FixedState::Unfixed,
            false,
        )
        .expect("the synthetic via is inside the bounding box");
    board
}

fn lone_pin_board() -> Board {
    let (mut board, _) = bare_board(&[0]);
    board.insert_pin(1, 0, vec![1], 1, FixedState::Unfixed);
    board
}

fn complete_net_board() -> Board {
    let (mut board, _) = bare_board(&[0, 5000]);
    board.insert_pin(1, 0, vec![1], 1, FixedState::Unfixed);
    board.insert_pin(1, 1, vec![1], 1, FixedState::Unfixed);
    insert_trace(&mut board, (0, 0), (5000, 0));
    board
}
