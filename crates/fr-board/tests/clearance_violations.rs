mod board_builder;

use board_builder::{BOUNDING_BOX, layers};
use fr_board::prelude::*;
use fr_geometry::{IntBox, IntVector, Point, Polyline, Shape, TileShape};


fn asymmetric_matrix() -> ClearanceMatrix {
    let ls = layers();
    let mut matrix = ClearanceMatrix::get_default_instance(&ls, 200);
    assert!(matrix.append_class("wide"));
    matrix.set_value_on_all_layers(2, 1, 600);
    matrix.set_value_on_all_layers(2, 2, 800);
    matrix
}

fn board_with(
    matrix: ClearanceMatrix,
    library: BoardLibrary,
    components: Components,
    net_count: usize,
) -> Board {
    let mut rules = BoardRules::new(layers(), matrix);
    rules.create_default_net_class();
    let default_class = rules.get_default_net_class();
    let mut board = Board::new(
        Vec::new(),
        1,
        BOUNDING_BOX,
        rules,
        library,
        components,
        Communication::default(),
    );
    for i in 0..net_count {
        board
            .rules
            .nets
            .add(format!("N{}", i + 1), 1, false, default_class);
    }
    board
}

fn smd_library(pads: &[(&str, i32, IntVector)]) -> (BoardLibrary, Components) {
    let mut padstacks = Padstacks::new(layers());
    let mut pins = Vec::new();
    for (name, half, offset) in pads {
        let padstack = padstacks.add(
            *name,
            vec![
                Some(Shape::Tile(TileShape::Box(IntBox::from_coords(
                    -*half, -*half, *half, *half,
                )))),
                None,
            ],
            false,
            false,
        );
        pins.push(PackagePin::new(*name, padstack, (*offset).into(), 0.0));
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
    (BoardLibrary::new(padstacks, packages), components)
}

fn two_overlapping_pins() -> Board {
    let (library, components) = smd_library(&[
        ("a", 50, IntVector::new(0, 0)),
        ("b", 50, IntVector::new(30, 0)),
    ]);
    let mut board = board_with(asymmetric_matrix(), library, components, 2);
    board.insert_pin(1, 0, vec![1], 1, FixedState::Unfixed);
    board.insert_pin(1, 1, vec![2], 2, FixedState::Unfixed);
    board
}

fn rows(violations: &[ClearanceViolation]) -> Vec<(u32, usize, f64, f64)> {
    violations
        .iter()
        .map(|v| {
            (
                v.second_item.0,
                v.layer,
                v.expected_clearance,
                v.actual_clearance,
            )
        })
        .collect()
}


#[test]
fn two_overlapping_pins_of_different_nets_violate() {
    let mut board = two_overlapping_pins();
    let violations = board.clearance_violations(ItemId(2));
    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0].first_item, ItemId(2));
    let expected = board
        .rules
        .clearance_matrix
        .get_value(2, 1, 0, false)
        .into();
    assert_eq!(rows(&violations), vec![(3, 0, expected, 0.0)]);
    assert_eq!(violations[0].shape.dimension(), 2);
}

#[test]
fn the_violation_count_is_the_length_of_the_violation_list() {
    let mut board = two_overlapping_pins();
    assert_eq!(board.clearance_violation_count(ItemId(2)), 1);
    assert_eq!(board.clearance_violation_count(ItemId(3)), 1);
    assert_eq!(board.clearance_violation_count(ItemId(1)), 0);
}

#[test]
fn clearance_matrix_argument_order_is_other_then_this() {
    let mut board = two_overlapping_pins();
    let matrix = board.rules.clearance_matrix.clone();
    assert_ne!(
        matrix.get_value(2, 1, 0, false),
        matrix.get_value(1, 2, 0, false),
        "the fixture matrix must be asymmetric for this test to mean anything"
    );
    let from_2 = board.clearance_violations(ItemId(2));
    assert_eq!(
        from_2[0].expected_clearance,
        f64::from(matrix.get_value(2, 1, 0, false))
    );
    let from_3 = board.clearance_violations(ItemId(3));
    assert_eq!(
        from_3[0].expected_clearance,
        f64::from(matrix.get_value(1, 2, 0, false))
    );
    assert_ne!(from_2[0].expected_clearance, from_3[0].expected_clearance);
}

#[test]
fn same_net_items_are_not_obstacles() {
    let (library, components) = smd_library(&[("a", 50, IntVector::new(-5000, -5000))]);
    let mut board = board_with(asymmetric_matrix(), library, components, 2);
    for _ in 0..2 {
        board.insert_trace_without_cleaning(
            Polyline::from_points(&[Point::new(-1000, 0), Point::new(1000, 0)]),
            0,
            30,
            vec![1],
            1,
            FixedState::Unfixed,
        );
    }
    assert!(board.clearance_violations(ItemId(3)).is_empty());
    let (library, components) = smd_library(&[("a", 50, IntVector::new(-5000, -5000))]);
    let mut board = board_with(asymmetric_matrix(), library, components, 2);
    for net in [1, 2] {
        board.insert_trace_without_cleaning(
            Polyline::from_points(&[Point::new(-1000, 0), Point::new(1000, 0)]),
            0,
            30,
            vec![net],
            1,
            FixedState::Unfixed,
        );
    }
    assert_eq!(board.clearance_violations(ItemId(3)).len(), 1);
}

fn tie_pin_board(with_pin: bool) -> Board {
    let (library, components) = smd_library(&[("tie", 60, IntVector::new(0, 0))]);
    let mut board = board_with(asymmetric_matrix(), library, components, 2);
    if with_pin {
        board.insert_pin(1, 0, vec![1, 2], 1, FixedState::Unfixed);
    }
    board.insert_trace_without_cleaning(
        Polyline::from_points(&[Point::new(0, 0), Point::new(1000, 0)]),
        0,
        30,
        vec![1],
        1,
        FixedState::Unfixed,
    );
    board.insert_trace_without_cleaning(
        Polyline::from_points(&[Point::new(0, 0), Point::new(0, 1000)]),
        0,
        30,
        vec![2],
        1,
        FixedState::Unfixed,
    );
    board
}

#[test]
fn tie_pin_exemption_suppresses_a_trace_pair() {
    let mut board = tie_pin_board(true);
    assert!(matches!(board.get_item(ItemId(2)), Some(Item::Pin(_))));
    assert!(board.clearance_violations(ItemId(3)).is_empty());
    assert!(board.clearance_violations(ItemId(4)).is_empty());
    let mut board = tie_pin_board(false);
    let violations = board.clearance_violations(ItemId(2));
    assert_eq!(rows(&violations), vec![(3, 0, 200.0, 0.0)]);
}

fn tie_pin_board_at_last_corner(with_pin: bool) -> Board {
    let (library, components) = smd_library(&[("tie", 60, IntVector::new(1000, 0))]);
    let mut board = board_with(asymmetric_matrix(), library, components, 2);
    if with_pin {
        board.insert_pin(1, 0, vec![1, 2], 1, FixedState::Unfixed);
    }
    board.insert_trace_without_cleaning(
        Polyline::from_points(&[Point::new(0, 0), Point::new(1000, 0)]),
        0,
        30,
        vec![1],
        1,
        FixedState::Unfixed,
    );
    board.insert_trace_without_cleaning(
        Polyline::from_points(&[Point::new(1000, 0), Point::new(1000, 1000)]),
        0,
        30,
        vec![2],
        1,
        FixedState::Unfixed,
    );
    board
}

#[test]
fn tie_pin_exemption_also_fires_at_the_last_corner() {
    let mut board = tie_pin_board_at_last_corner(true);
    let (pin, trace_a, trace_b) = (ItemId(2), ItemId(3), ItemId(4));
    assert!(matches!(board.get_item(pin), Some(Item::Pin(_))));
    assert!(
        board
            .trace_normal_contacts_at(trace_a, &Point::new(0, 0), true)
            .is_empty()
    );
    let at_last = board.trace_normal_contacts_at(trace_a, &Point::new(1000, 0), true);
    assert!(at_last.contains(&trace_b) && at_last.contains(&pin));

    assert!(board.clearance_violations(trace_a).is_empty());
    assert!(board.clearance_violations(trace_b).is_empty());
    let mut board = tie_pin_board_at_last_corner(false);
    assert_eq!(
        rows(&board.clearance_violations(ItemId(2))),
        vec![(3, 0, 200.0, 0.0)]
    );
}


fn bx(llx: i32, lly: i32, urx: i32, ury: i32) -> TileShape {
    TileShape::Box(IntBox::from_coords(llx, lly, urx, ury))
}

#[test]
fn bisection_returns_low_after_sixteen_halvings() {
    let answer = Board::calculate_clearance_between_two_shapes(
        &bx(0, 0, 100, 100),
        &bx(300, 0, 400, 100),
        1000.0,
        500,
        500,
    );
    assert_eq!(answer, 200.98876953125);
    assert!(
        answer < 201.0,
        "the returned `low` never reaches the threshold"
    );

    let never_meets = Board::calculate_clearance_between_two_shapes(
        &bx(0, 0, 100, 100),
        &bx(2100, 0, 2200, 100),
        1000.0,
        500,
        500,
    );
    assert_eq!(never_meets, 999.9847412109375);
    assert_eq!(never_meets, 1000.0 * (1.0 - f64::powi(2.0, -16)));
    assert_ne!(never_meets, 1000.0 * (1.0 - f64::powi(2.0, -15)));
    assert_ne!(never_meets, 1000.0 * (1.0 - f64::powi(2.0, -17)));
}

#[test]
fn the_bisection_matches_the_jvm() {
    let a = bx(0, 0, 100, 100);
    let far = bx(300, 0, 400, 100);
    assert_eq!(
        Board::calculate_clearance_between_two_shapes(&a, &far, 1000.0, 500, 500),
        200.98876953125,
        "D1"
    );
    assert_eq!(
        Board::calculate_clearance_between_two_shapes(&a, &far, 1000.0, 250, 750),
        200.653076171875,
        "D2"
    );
    assert_eq!(
        Board::calculate_clearance_between_two_shapes(&a, &bx(50, 0, 150, 100), 1000.0, 500, 500),
        0.0,
        "D3"
    );
    assert_eq!(
        Board::calculate_clearance_between_two_shapes(
            &a,
            &bx(2100, 0, 2200, 100),
            1000.0,
            500,
            500
        ),
        999.9847412109375,
        "D4"
    );
    assert_eq!(
        Board::calculate_clearance_between_two_shapes(&a, &bx(100, 0, 200, 100), 1000.0, 500, 500),
        0.9918212890625,
        "D5"
    );
    assert_eq!(
        Board::calculate_clearance_between_two_shapes(&a, &far, 1000.0, 0, 0),
        200.98876953125,
        "D6"
    );
    assert_eq!(
        Board::calculate_clearance_between_two_shapes(&a, &bx(220, 0, 320, 100), 200.0, 100, 100),
        120.9991455078125,
        "D7"
    );
}


fn two_partner_board() -> Board {
    let (library, components) = smd_library(&[("a", 50, IntVector::new(0, 0))]);
    let mut board = board_with(asymmetric_matrix(), library, components, 3);
    board.insert_pin(1, 0, vec![1], 1, FixedState::Unfixed);
    board.insert_trace_without_cleaning(
        Polyline::from_points(&[Point::new(-1000, 0), Point::new(1000, 0)]),
        0,
        30,
        vec![2],
        1,
        FixedState::Unfixed,
    );
    board.insert_trace_without_cleaning(
        Polyline::from_points(&[Point::new(250, -500), Point::new(250, 500)]),
        0,
        30,
        vec![3],
        1,
        FixedState::Unfixed,
    );
    board
}

#[test]
fn smallest_clearance_only_ever_falls_and_is_never_reset() {
    let mut board = two_partner_board();
    assert_eq!(
        board
            .get_item(ItemId(2))
            .unwrap()
            .header()
            .smallest_clearance,
        -1.0
    );
    let first = board.clearance_violations(ItemId(2));
    assert_eq!(first.len(), 2);
    assert_eq!(
        board
            .get_item(ItemId(2))
            .unwrap()
            .header()
            .smallest_clearance,
        0.0
    );
    assert!(board.remove_item(ItemId(3)));
    let second = board.clearance_violations(ItemId(2));
    assert_eq!(second.len(), 1);
    assert!(second[0].actual_clearance > 0.0);
    assert_eq!(
        board
            .get_item(ItemId(2))
            .unwrap()
            .header()
            .smallest_clearance,
        0.0
    );
}

#[test]
fn smallest_clearance_over_the_board_is_max_value_until_something_is_computed() {
    let mut board = two_partner_board();
    assert_eq!(board.smallest_clearance(), f64::MAX);
    board.clearance_violations(ItemId(2));
    assert_eq!(board.smallest_clearance(), 0.0);
}


#[test]
fn aggregate_is_sorted_by_shortfall_descending_and_double_counts() {
    let mut board = two_overlapping_pins();
    let aggregated = board.aggregate_violations_sorted_by_severity();
    assert_eq!(aggregated.len(), 2);
    let mut pairs: Vec<(u32, u32)> = aggregated
        .iter()
        .map(|v| (v.first_item.0, v.second_item.0))
        .collect();
    pairs.sort_unstable();
    assert_eq!(pairs, vec![(2, 3), (3, 2)]);
    let shortfall = |v: &ClearanceViolation| v.expected_clearance - v.actual_clearance;
    assert_eq!(
        aggregated.iter().map(shortfall).collect::<Vec<_>>(),
        vec![600.0, 200.0]
    );
    assert_eq!(board.smallest_clearance(), 0.0);
}

#[test]
fn aggregate_over_a_clean_board_is_empty() {
    let (library, components) = smd_library(&[("a", 50, IntVector::new(-5000, -5000))]);
    let mut board = board_with(asymmetric_matrix(), library, components, 1);
    board.insert_pin(1, 0, vec![1], 1, FixedState::Unfixed);
    assert!(board.aggregate_violations_sorted_by_severity().is_empty());
    assert_eq!(board.smallest_clearance(), f64::MAX);
}


fn escape_via_board(smd_layer: usize) -> (Board, ItemId) {
    let mut padstacks = Padstacks::new(layers());
    let pad_shape = Shape::Tile(TileShape::Box(IntBox::from_coords(-50, -50, 50, 50)));
    let smd_a = padstacks.add(
        "a",
        vec![Some(pad_shape.clone()), Some(pad_shape)],
        false,
        false,
    );
    let smd_b = padstacks.add(
        "b",
        vec![
            Some(Shape::Tile(TileShape::Box(IntBox::from_coords(
                -50, -50, 50, 50,
            )))),
            None,
        ],
        false,
        false,
    );
    let thru_shape = Shape::Tile(TileShape::Box(IntBox::from_coords(-70, -70, 70, 70)));
    let thru = padstacks.add(
        "thru",
        vec![Some(thru_shape.clone()), Some(thru_shape)],
        true,
        false,
    );
    let mut packages = Packages::new();
    let package = packages.add(
        "pkg",
        vec![
            PackagePin::new("P1", smd_a, IntVector::new(0, 0).into(), 0.0),
            PackagePin::new("P2", smd_b, IntVector::new(100, 0).into(), 0.0),
        ],
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
    let mut board = board_with(
        asymmetric_matrix(),
        BoardLibrary::new(padstacks, packages),
        components,
        2,
    );
    board.insert_pin(1, 0, vec![1], 1, FixedState::Unfixed);
    board.insert_pin(1, 1, vec![2], 1, FixedState::Unfixed);
    let via = board
        .insert_escape_via(
            thru,
            Point::new(0, 0),
            vec![1],
            1,
            FixedState::Unfixed,
            smd_layer,
        )
        .expect("no normalisation failure");
    (board, via)
}

#[test]
fn escape_via_drops_same_net_violations_on_its_smd_layer() {
    let (mut board, via) = escape_via_board(0);
    let violations = board.clearance_violations(via);
    let mut seen: Vec<(u32, usize)> = violations
        .iter()
        .map(|v| (v.second_item.0, v.layer))
        .collect();
    seen.sort_unstable();
    assert_eq!(seen, vec![(2, 1), (3, 0)]);
    assert!(violations.iter().all(|v| v.first_item == via));
}

#[test]
fn escape_via_keeps_same_net_violations_on_other_layers() {
    let (mut board, via) = escape_via_board(1);
    let mut seen: Vec<(u32, usize)> = board
        .clearance_violations(via)
        .iter()
        .map(|v| (v.second_item.0, v.layer))
        .collect();
    seen.sort_unstable();
    assert_eq!(seen, vec![(2, 0), (3, 0)]);
}

#[test]
fn a_plain_via_keeps_every_violation() {
    let (mut board, via) = escape_via_board(0);
    let Some(Item::Via(v)) = board.get_item_mut(via) else {
        panic!("the escape via")
    };
    v.is_escape_via = false;
    let mut seen: Vec<(u32, usize)> = board
        .clearance_violations(via)
        .iter()
        .map(|v| (v.second_item.0, v.layer))
        .collect();
    seen.sort_unstable();
    assert_eq!(seen, vec![(2, 0), (2, 1), (3, 0)]);
    let (mut board, via) = escape_via_board(0);
    let Some(Item::Via(v)) = board.get_item_mut(via) else {
        panic!("the escape via")
    };
    v.escape_via_smd_layer = -1;
    assert_eq!(board.clearance_violations(via).len(), 3);
}
