use copper_board::prelude::*;
use copper_drc::{
    BoardStatisticsClearanceViolations, DesignRulesChecker, DrcViolation, DrcViolationKind,
};
use copper_dsn::{BoardReadResult, DsnReadOptions};
use copper_geometry::{
    Area, FloatPoint, IntBox, IntPoint, IntVector, Point, Polyline, Shape, TileShape,
};

fn fixture_board(name: &str) -> Board {
    let path = testkit::fixture(name);
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("cannot read fixture {}: {e}", path.display()));
    match copper_dsn::read_board(&bytes[..], None, Some(name), &DsnReadOptions::default()) {
        BoardReadResult::Success { board, .. } | BoardReadResult::OutlineMissing { board, .. } => {
            *board.unwrap_or_else(|| panic!("{name} produced no board"))
        }
        other => panic!("{name} did not read: {other:?}"),
    }
}

fn counters(board: &mut Board) -> (i32, usize, usize, usize) {
    let max_net_number = board.rules.nets.max_net_number();
    let mut drc = DesignRulesChecker::new(board);
    drc.calculate_all_incompletes();
    let per_net = (1..=max_net_number)
        .map(|n| drc.get_incomplete_count_for_net(n))
        .sum();
    (
        drc.max_connections(),
        drc.get_incomplete_count(),
        drc.get_all_airlines().len(),
        per_net,
    )
}

#[test]
fn dev_board_counts() {
    let mut board = fixture_board("Issue575-drc_dev-board_4_hole_clearance_violations.dsn");
    assert_eq!(counters(&mut board), (96, 9, 9, 9));
}

#[test]
fn bbd_mars_64_counts() {
    let mut board =
        fixture_board("Issue575-drc_BBD_Mars-64_6_track_1_hole_clearance_violations.dsn");
    assert_eq!(counters(&mut board), (106, 3, 3, 3));
}

#[test]
fn natural_tone_preamp_counts() {
    let mut board = fixture_board("Issue575-drc_Natural_Tone_Preamp_7_unconnected_items.dsn");
    assert_eq!(counters(&mut board), (218, 145, 145, 145));
}

#[test]
fn empty_board_has_no_incompletes() {
    let mut board = fixture_board("empty_board.dsn");
    assert_eq!(counters(&mut board), (0, 0, 0, 0));
    let mut drc = DesignRulesChecker::new(&mut board);
    assert!(drc.get_all_violations().is_empty());
}

#[test]
fn a_multi_net_item_lands_in_every_net_list() {
    let mut board = two_net_pin_board();

    let max_connections = {
        let mut drc = DesignRulesChecker::new(&mut board);
        drc.calculate_all_incompletes();
        assert_eq!(drc.get_incomplete_count_for_net(1), 1);
        assert_eq!(drc.get_incomplete_count_for_net(2), 1);
        drc.max_connections()
    };
    assert_eq!(max_connections, 2);

    let mut drc = DesignRulesChecker::new(&mut board);
    let airlines = drc.get_all_airlines();
    assert_eq!(airlines.len(), 2);
    assert_eq!(
        airlines.iter().map(|a| a.net_number).collect::<Vec<_>>(),
        vec![1, 2],
    );
    for airline in &airlines {
        assert!(
            [airline.from_item, airline.to_item].contains(&ItemId(4)),
            "the two-net pin is an endpoint of both airlines",
        );
    }
}

#[test]
fn max_connections_counts_only_pins_and_conduction_areas() {
    let mut board = three_trace_net_board();
    let mut drc = DesignRulesChecker::new(&mut board);
    drc.calculate_all_incompletes();
    assert_eq!(drc.max_connections(), 0);

    let mut board = three_trace_net_board();
    insert_conduction_area(&mut board);
    let mut drc = DesignRulesChecker::new(&mut board);
    drc.calculate_all_incompletes();
    assert_eq!(drc.max_connections(), 0);

    let mut board = three_trace_net_board();
    insert_conduction_area(&mut board);
    board.insert_pin(1, 0, vec![1], 1, FixedState::Unfixed);
    let mut drc = DesignRulesChecker::new(&mut board);
    drc.calculate_all_incompletes();
    assert_eq!(drc.max_connections(), 1);
}

#[test]
fn an_empty_net_contributes_nothing() {
    let mut board = three_trace_net_board();
    board.insert_pin(1, 0, vec![1], 1, FixedState::Unfixed);
    board.insert_pin(1, 1, vec![1], 1, FixedState::Unfixed);
    assert_eq!(
        board.rules.nets.max_net_number(),
        2,
        "net 2 exists and is empty"
    );
    let mut drc = DesignRulesChecker::new(&mut board);
    drc.calculate_all_incompletes();
    assert_eq!(drc.max_connections(), 1);
}

#[test]
fn lazy_initialisation_matches_java() {
    let mut board = fixture_board("Issue575-drc_dev-board_4_hole_clearance_violations.dsn");
    let mut drc = DesignRulesChecker::new(&mut board);

    assert_eq!(drc.max_connections(), 0);
    assert_eq!(drc.get_incomplete_count(), 9);
    assert_eq!(
        drc.max_connections(),
        96,
        "the accessor ran the initialiser"
    );

    let mut drc = DesignRulesChecker::new(&mut board);
    assert_eq!(drc.get_incomplete_count_for_net(1), 6);
    let mut drc = DesignRulesChecker::new(&mut board);
    assert_eq!(drc.get_all_airlines().len(), 9);
    let mut drc = DesignRulesChecker::new(&mut board);
    assert_eq!(drc.get_length_violation_count(), 0);
    let mut drc = DesignRulesChecker::new(&mut board);
    assert_eq!(drc.get_length_violation(1), 0.0);
    let mut drc = DesignRulesChecker::new(&mut board);
    assert!(
        drc.recalculate_length_violations(),
        "`:768` returns true when it had to initialise — \"changed from nothing to something\"",
    );
    assert!(
        !drc.recalculate_length_violations(),
        "and false on the second call, where nothing changed",
    );
    let mut drc = DesignRulesChecker::new(&mut board);
    assert_eq!(drc.get_net_incompletes(1).map(|ni| ni.count()), Some(6));
}

#[test]
fn out_of_range_net_numbers_answer_the_java_defaults() {
    let mut board = fixture_board("Issue575-drc_dev-board_4_hole_clearance_violations.dsn");
    let max_net_number = board.rules.nets.max_net_number();
    let mut drc = DesignRulesChecker::new(&mut board);
    for net_number in [-1, 0, max_net_number + 1] {
        assert_eq!(drc.get_incomplete_count_for_net(net_number), 0);
        assert_eq!(drc.get_length_violation(net_number), 0.0);
        assert!(drc.get_net_incompletes(net_number).is_none());
    }
    assert!(drc.get_net_incompletes(max_net_number).is_some());
}

#[test]
fn recalculating_one_net_replaces_only_that_net() {
    let mut board = two_group_net_board();
    let mut drc = DesignRulesChecker::new(&mut board);
    assert_eq!(drc.get_incomplete_count(), 1);

    drc.recalculate_net_incompletes_with(1, &[ItemId(2)]);
    assert_eq!(drc.get_incomplete_count_for_net(1), 0);

    drc.recalculate_net_incompletes(1);
    assert_eq!(drc.get_incomplete_count_for_net(1), 1);

    drc.recalculate_net_incompletes(99);
    drc.recalculate_net_incompletes_with(99, &[]);
    assert_eq!(drc.get_incomplete_count(), 1);
}

#[test]
fn recalculate_net_incompletes_initialises_and_returns() {
    let mut board = two_group_net_board();
    let mut drc = DesignRulesChecker::new(&mut board);
    drc.recalculate_net_incompletes(1);
    assert_eq!(drc.get_incomplete_count(), 1);
    assert_eq!(drc.max_connections(), 1, "the initialiser ran");

    let mut drc = DesignRulesChecker::new(&mut board);
    drc.recalculate_net_incompletes_with(1, &[ItemId(2)]);
    assert_eq!(
        drc.get_incomplete_count_for_net(1),
        0,
        "no early return: the caller's list won",
    );
}

#[test]
fn statistics_block() {
    let mut board =
        fixture_board("Issue575-drc_BBD_Mars-64_6_track_1_hole_clearance_violations.dsn");
    let factor = Unit::scale(1.0, board.communication.unit, Unit::Um)
        / f64::from(board.communication.resolution);
    assert_eq!(factor, 0.1);

    // All 9 of this fixture's clearance violations are trace-vs-trace at 0.1998-0.1999mm
    // against a 0.2mm rule (KiCad's `DRC_EPSILON_MM`, subtracted from every gap test as
    // `sub_epsilon`, absorbs exactly this rounding gap; see `constraints::DRC_EPSILON_MM`).
    let violations = DesignRulesChecker::new(&mut board).get_all_violations();
    assert_eq!(violations.len(), 0);
    let stats = BoardStatisticsClearanceViolations::from_violations(&violations, factor);
    assert_eq!(stats.total_count, Some(0));
    assert_eq!(stats.min_violation_um, Some(0.0));
    assert_eq!(stats.max_violation_um, Some(0.0));
    assert_eq!(stats.avg_violation_um, Some(0.0));
}

#[test]
fn an_empty_violation_list_is_four_zeroes_not_four_nulls() {
    let stats = BoardStatisticsClearanceViolations::from_violations(&[], 0.1);
    assert_eq!(stats.total_count, Some(0));
    assert_eq!(stats.min_violation_um, Some(0.0));
    assert_eq!(stats.max_violation_um, Some(0.0));
    assert_eq!(stats.avg_violation_um, Some(0.0));
    assert_eq!(
        BoardStatisticsClearanceViolations::default().total_count,
        None
    );
}

#[test]
fn a_negative_shortfall_is_clamped_but_still_averaged_over_every_violation() {
    let violation = |expected: f64, actual: f64| DrcViolation {
        kind: DrcViolationKind::Clearance,
        severity: DrcSeverity::Error,
        first_item: ItemId(2),
        second_item: Some(ItemId(3)),
        layer: Some(0),
        position: FloatPoint::new(0.0, 0.0),
        expected,
        actual,
        estimated: false,
    };
    let synthetic = vec![violation(100.0, 0.0), violation(100.0, 400.0)];
    let stats = BoardStatisticsClearanceViolations::from_violations(&synthetic, 2.0);
    assert_eq!(stats.total_count, Some(2));
    assert_eq!(stats.min_violation_um, Some(0.0), "the clamped one");
    assert_eq!(stats.max_violation_um, Some(200.0));
    assert_eq!(stats.avg_violation_um, Some(100.0), "200 / 2, not 200 / 1");
}

#[test]
fn a_default_block_serialises_to_nothing() {
    assert_eq!(
        serde_json::to_string(&BoardStatisticsClearanceViolations::default()).unwrap(),
        "{}",
    );
}

#[test]
fn the_serialised_keys_are_gsons() {
    let stats = BoardStatisticsClearanceViolations::from_violations(&[], 1.0);
    assert_eq!(
        serde_json::to_string(&stats).unwrap(),
        r#"{"total_count":0,"min_violation_um":0.0,"max_violation_um":0.0,"avg_violation_um":0.0}"#,
    );
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

fn bare_board(pin_offsets: &[i32]) -> Board {
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
    board
}

fn insert_trace(board: &mut Board, from: (i32, i32), to: (i32, i32), net: i32) -> ItemId {
    board
        .insert_trace_without_cleaning(
            Polyline::from_points(&[Point::new(from.0, from.1), Point::new(to.0, to.1)]),
            0,
            30,
            vec![net],
            1,
            FixedState::Unfixed,
        )
        .expect("the synthetic trace is neither degenerate nor closed")
}

fn two_net_pin_board() -> Board {
    let mut board = bare_board(&[0, 4000, 8000]);
    board.insert_pin(1, 0, vec![1], 1, FixedState::Unfixed);
    board.insert_pin(1, 2, vec![2], 1, FixedState::Unfixed);
    board.insert_pin(1, 1, vec![1, 2], 1, FixedState::Unfixed);
    board
}

fn three_trace_net_board() -> Board {
    let mut board = bare_board(&[0, 5000]);
    insert_trace(&mut board, (0, 1000), (1000, 1000), 1);
    insert_trace(&mut board, (3000, 1000), (4000, 1000), 1);
    insert_trace(&mut board, (6000, 1000), (7000, 1000), 1);
    board
}

fn two_group_net_board() -> Board {
    let mut board = bare_board(&[0, 8000, 4000]);
    board.insert_pin(1, 0, vec![1], 1, FixedState::Unfixed);
    board.insert_pin(1, 1, vec![1], 1, FixedState::Unfixed);
    board.insert_pin(1, 2, vec![2], 1, FixedState::Unfixed);
    board
}

fn insert_conduction_area(board: &mut Board) {
    board.insert_conduction_area(
        Area::Shape(Shape::Tile(TileShape::Box(IntBox::from_coords(
            6000, 6000, 7000, 7000,
        )))),
        0,
        vec![1],
        1,
        false,
        FixedState::Unfixed,
    );
}

#[test]
fn a_net_subset_count_matches_the_full_pass() {
    let mut board = fixture_board("Issue575-drc_dev-board_4_hole_clearance_violations.dsn");
    let max_net_number = board.rules.nets.max_net_number();
    let (full, per_net) = {
        let mut drc = DesignRulesChecker::new(&mut board);
        drc.calculate_all_incompletes();
        let per_net: Vec<usize> = (1..=max_net_number)
            .map(|n| drc.get_incomplete_count_for_net(n))
            .collect();
        (drc.get_incomplete_count(), per_net)
    };
    assert_eq!(full, 9);

    let every_net: std::collections::BTreeSet<i32> = (1..=max_net_number).collect();
    assert_eq!(
        DesignRulesChecker::incomplete_count_for_nets(&board, &every_net),
        full
    );
    for net_number in 1..=max_net_number {
        let just_this: std::collections::BTreeSet<i32> = [net_number].into_iter().collect();
        assert_eq!(
            DesignRulesChecker::incomplete_count_for_nets(&board, &just_this),
            per_net[net_number as usize - 1],
            "net {net_number}"
        );
    }
    assert_eq!(
        DesignRulesChecker::incomplete_count_for_nets(&board, &std::collections::BTreeSet::new()),
        0
    );
}
