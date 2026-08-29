//! Plan 5 Task 2: `Item.clearanceViolations`, `clearanceViolationCount`, the private
//! `calculateClearanceBetweenTwoShapes` bisection, `Via`'s escape-via override, and the two
//! headless statics of `drc.ClearanceViolation`.
//!
//! # Provenance
//!
//! The bisection goldens in [`the_bisection_matches_the_jvm`] and
//! [`bisection_returns_low_after_sixteen_halvings`] are block `D` of
//! `crates/fr-board/tests/data/DrcProbe.java`, which invokes the **real** private
//! `Item.calculateClearanceBetweenTwoShapes` by reflection on the clone's HEAD jar (JDK 25); see
//! that file's header for the recorded command. Everything else is transcribed from the Java
//! source lines named in each test.

mod board_builder;

use board_builder::{BOUNDING_BOX, layers};
use fr_board::prelude::*;
use fr_geometry::{IntBox, IntVector, Point, Polyline, Shape, TileShape};

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

/// `ClearanceMatrix.getDefaultInstance(ls, 200)` plus the `"wide"` class (index 2) and the two
/// **asymmetric** entries `board_builder` uses: `setValue(2, 1, 600)` writes
/// `row[1].column[2]`, so `getValue(2, 1, …) == 600` while `getValue(1, 2, …) == 200`
/// (ClearanceMatrix.java:100-101, quirk #83).
fn asymmetric_matrix() -> ClearanceMatrix {
    let ls = layers();
    let mut matrix = ClearanceMatrix::get_default_instance(&ls, 200);
    assert!(matrix.append_class("wide"));
    matrix.set_value_on_all_layers(2, 1, 600);
    matrix.set_value_on_all_layers(2, 2, 800);
    matrix
}

/// A two-layer board with no outline polygon, `n` nets, and the caller's library/components.
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

/// An SMD pad on layer 0 only, `size` half-width square, at `offset` in its package.
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

/// Two overlapping SMD pads of **different nets and different clearance classes**: item 2 is net
/// 1 / class 1, item 3 is net 2 / class 2 (`"wide"`). Their raw shapes overlap, so the bisection
/// takes its early return (Item.java:472-474).
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

/// `(second_item, layer, expected, actual)` for each violation — everything but the shape.
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

// ---------------------------------------------------------------------------------------------
// Item.clearanceViolations (Item.java:363-469)
// ---------------------------------------------------------------------------------------------

#[test]
fn two_overlapping_pins_of_different_nets_violate() {
    // Item.java:424-425 for the expected clearance, :472-474 for the actual: two shapes that
    // already intersect in dimension 2 have zero clearance.
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
    // Item.java:445: the stored shape is the *intersection* of the two enlarged shapes.
    assert_eq!(violations[0].shape.dimension(), 2);
}

#[test]
fn the_violation_count_is_the_length_of_the_violation_list() {
    // Item.java:358-361.
    let mut board = two_overlapping_pins();
    assert_eq!(board.clearance_violation_count(ItemId(2)), 1);
    assert_eq!(board.clearance_violation_count(ItemId(3)), 1);
    assert_eq!(board.clearance_violation_count(ItemId(1)), 0);
}

#[test]
fn clearance_matrix_argument_order_is_other_then_this() {
    // Item.java:424-425: `getValue(currentItem.clearanceClassIndex, this.clearanceClassIndex,
    // shapeLayer(i), false)` — the *other* item's class first. The fixture's matrix is
    // asymmetric, so transposing the two is observable.
    let mut board = two_overlapping_pins();
    let matrix = board.rules.clearance_matrix.clone();
    assert_ne!(
        matrix.get_value(2, 1, 0, false),
        matrix.get_value(1, 2, 0, false),
        "the fixture matrix must be asymmetric for this test to mean anything"
    );
    // Querying item 2 (class 1) about item 3 (class 2): `getValue(2, 1)`.
    let from_2 = board.clearance_violations(ItemId(2));
    assert_eq!(
        from_2[0].expected_clearance,
        f64::from(matrix.get_value(2, 1, 0, false))
    );
    // …and the other way round: `getValue(1, 2)`.
    let from_3 = board.clearance_violations(ItemId(3));
    assert_eq!(
        from_3[0].expected_clearance,
        f64::from(matrix.get_value(1, 2, 0, false))
    );
    assert_ne!(from_2[0].expected_clearance, from_3[0].expected_clearance);
}

#[test]
fn same_net_items_are_not_obstacles() {
    // Item.java:379: `currentItem.isObstacle(this)` — Trace.java:101 answers false for a trace of
    // the same net. The tree query itself passes an **empty** ignore-net array (Item.java:377),
    // so the filtering is `isObstacle`'s, not the query's.
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
    // The same two traces on different nets do violate.
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

/// Two traces of different nets meeting at `(0, 0)`, with or without a pin there that carries
/// both nets.
fn tie_pin_board(with_pin: bool) -> Board {
    let (library, components) = smd_library(&[("tie", 60, IntVector::new(0, 0))]);
    let mut board = board_with(asymmetric_matrix(), library, components, 2);
    if with_pin {
        // The tie pin: on both nets, so it "shares net" with each trace (Item.java:405).
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
    // Item.java:383-411: two traces connected to the same pin may overlap without sharing a net.
    let mut board = tie_pin_board(true);
    // ids: 1 outline, 2 pin, 3 and 4 the traces.
    assert!(matches!(board.get_item(ItemId(2)), Some(Item::Pin(_))));
    assert!(board.clearance_violations(ItemId(3)).is_empty());
    assert!(board.clearance_violations(ItemId(4)).is_empty());
    // Without the pin the same pair violates: ids 1 outline, 2 and 3 the traces.
    let mut board = tie_pin_board(false);
    let violations = board.clearance_violations(ItemId(2));
    assert_eq!(rows(&violations), vec![(3, 0, 200.0, 0.0)]);
}

// ---------------------------------------------------------------------------------------------
// calculateClearanceBetweenTwoShapes (Item.java:471-493)
// ---------------------------------------------------------------------------------------------

fn bx(llx: i32, lly: i32, urx: i32, ury: i32) -> TileShape {
    TileShape::Box(IntBox::from_coords(llx, lly, urx, ury))
}

#[test]
fn bisection_returns_low_after_sixteen_halvings() {
    // `DrcProbe.java` block D, row `D1 gap=200 min=1000 comp=500/500 -> 200.98876953125`.
    //
    // The 16 halvings of `[0, 1000]` have a resolution of `1000 / 2^16 = 0.0152587890625`, and
    // the answer is the last `low` — never `mid` (Item.java:492). It sits *above* the 200-unit
    // gap because `IntBox.enlarge` truncates to integer coordinates, so the two shapes first
    // overlap in dimension 2 at a total enlargement of 201, not 200: `low` converges to that
    // integer threshold from below, and `200.98876953125 < 201`.
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
    // The step below the answer is one bisection step wide, which pins "16 iterations": a 15-step
    // or 17-step loop lands on a different multiple of the resolution.
    assert_eq!(
        f64::from((answer / (1000.0f64 / 65536.0)) as i32),
        answer / (1000.0 / 65536.0)
    );
}

#[test]
fn the_bisection_matches_the_jvm() {
    // Every row of `DrcProbe.java` block D, verbatim.
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
    // D3: the raw shapes already intersect in dimension 2 (Item.java:472-474).
    assert_eq!(
        Board::calculate_clearance_between_two_shapes(&a, &bx(50, 0, 150, 100), 1000.0, 500, 500),
        0.0,
        "D3"
    );
    // D4: they never meet inside `minimumClearance`, so `low` climbs to one step below `high`.
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
    // D5: touching boxes intersect in dimension *1*, so the early return does not fire.
    assert_eq!(
        Board::calculate_clearance_between_two_shapes(&a, &bx(100, 0, 200, 100), 1000.0, 500, 500),
        0.9918212890625,
        "D5"
    );
    // D6: `sumComp == 0` makes both factors 0.5 (Item.java:481-482), which for equal components
    // is the same split as D1.
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

// ---------------------------------------------------------------------------------------------
// smallestClearance (Item.java:47, :451-453)
// ---------------------------------------------------------------------------------------------

/// Item 2 (an SMD pad at the origin, net 1) violates item 3 (a trace of net 2 straight through
/// it, so the bisection returns `0.0`) and item 4 (a trace of net 3 well clear of it, so the
/// bisection returns a positive clearance). Item 3 is a trace rather than a second pin because
/// `BoardItemRepository.removeItem` refuses to delete a component pin
/// (`Item.isDeletionForbidden`), and the test needs to drop it between the two calls.
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
    // Item.java:451-453 lowers the field and never raises it; nothing in Java ever resets it to
    // the `-1.0` of Item.java:47 (quirk #153).
    let mut board = two_partner_board();
    // A fresh item carries the sentinel.
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
    // Drop the overlapping partner: the second call finds only the *larger* clearance…
    assert!(board.remove_item(ItemId(3)));
    let second = board.clearance_violations(ItemId(2));
    assert_eq!(second.len(), 1);
    assert!(second[0].actual_clearance > 0.0);
    // …and the field stays at the first call's minimum.
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
    // ClearanceViolation.java:86-94: `Double.MAX_VALUE` when no item has a non-negative value.
    let mut board = two_partner_board();
    assert_eq!(board.smallest_clearance(), f64::MAX);
    board.clearance_violations(ItemId(2));
    assert_eq!(board.smallest_clearance(), 0.0);
}

// ---------------------------------------------------------------------------------------------
// aggregateSortedBySeverity (ClearanceViolation.java:64-75)
// ---------------------------------------------------------------------------------------------

#[test]
fn aggregate_is_sorted_by_shortfall_descending_and_double_counts() {
    // ClearanceViolation.java:70-74, and the class doc's "reported once on each item" note; the
    // shape of `RatsnestClearanceHeadlessTest.java:96-110`.
    let mut board = two_overlapping_pins();
    let aggregated = board.aggregate_violations_sorted_by_severity();
    // One violating pair, reported on both items.
    assert_eq!(aggregated.len(), 2);
    let mut pairs: Vec<(u32, u32)> = aggregated
        .iter()
        .map(|v| (v.first_item.0, v.second_item.0))
        .collect();
    pairs.sort_unstable();
    assert_eq!(pairs, vec![(2, 3), (3, 2)]);
    // Descending shortfall: item 2 sees `getValue(2, 1) = 600`, item 3 sees `getValue(1, 2) = 200`.
    let shortfall = |v: &ClearanceViolation| v.expected_clearance - v.actual_clearance;
    assert_eq!(
        aggregated.iter().map(shortfall).collect::<Vec<_>>(),
        vec![600.0, 200.0]
    );
    assert_eq!(board.smallest_clearance(), 0.0);
}

#[test]
fn aggregate_over_a_clean_board_is_empty() {
    // `RatsnestClearanceHeadlessTest.emptyBoardHasNoIncompletesAndNoViolations` (:116-129).
    let (library, components) = smd_library(&[("a", 50, IntVector::new(-5000, -5000))]);
    let mut board = board_with(asymmetric_matrix(), library, components, 1);
    board.insert_pin(1, 0, vec![1], 1, FixedState::Unfixed);
    assert!(board.aggregate_violations_sorted_by_severity().is_empty());
    assert_eq!(board.smallest_clearance(), f64::MAX);
}

// ---------------------------------------------------------------------------------------------
// The Via escape-via override (Via.java:88-112)
// ---------------------------------------------------------------------------------------------

/// An escape via at the origin, sitting inside a **through** pad of its own net (item 2, net 1,
/// both layers) and overlapping a foreign-net SMD pad (item 3, net 2, layer 0).
///
/// The same-net partner has to span both layers: `Pin.isObstacle` answers false for a same-net
/// via unless `drillAllowed()` is false (Pin.java:365), and `DrillItem.drillAllowed` is
/// `firstLayer() == lastLayer()` — so a single-layer SMD pad of the via's own net is not an
/// obstacle at all and would produce nothing for `Via`'s override to filter. A two-layer pad
/// produces one violation per layer, which is exactly the pair the override separates.
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
    // Via.java:93-110: a violation is dropped only when *both* halves hold — the layer is the
    // escape via's SMD layer, and the other item shares a net with the via.
    let (mut board, via) = escape_via_board(0);
    let violations = board.clearance_violations(via);
    let mut seen: Vec<(u32, usize)> = violations
        .iter()
        .map(|v| (v.second_item.0, v.layer))
        .collect();
    seen.sort_unstable();
    // The same-net pad's layer-0 violation is gone; its layer-1 one and the foreign net's are not.
    assert_eq!(seen, vec![(2, 1), (3, 0)]);
    assert!(violations.iter().all(|v| v.first_item == via));
}

#[test]
fn escape_via_keeps_same_net_violations_on_other_layers() {
    // Via.java:94: the layer test comes first, so the same pair on any other layer survives.
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
    // Via.java:90-92: the override is a no-op unless `isEscapeVia && escapeViaSmdLayer >= 0`.
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
    // …and the same is true of an escape via whose SMD layer was never set (Via.java:44).
    let (mut board, via) = escape_via_board(0);
    let Some(Item::Via(v)) = board.get_item_mut(via) else {
        panic!("the escape via")
    };
    v.escape_via_smd_layer = -1;
    assert_eq!(board.clearance_violations(via).len(), 3);
}
