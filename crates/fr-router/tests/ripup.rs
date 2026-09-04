//! Plan 6 Task 13, the ripup half: `autoroute.maze.MazeRipupResolver`
//! (`MazeRipupResolver.java:35-268`) and `autoroute.path.Connection`
//! (`Connection.java:39-154`).
//!
//! # Where the numbers come from
//!
//! Every literal below is **read off the HEAD jar**. The probe is
//! `scripts/differential/java/probes/P6T13Probe.java`; its whole stdout is committed as
//! `tests/data/p6t13-drills-ripup.txt`. Each test names its probe mode and pastes the lines it
//! asserts against. The `JavaRandom` literals additionally carry the `jshell` one-liner that
//! reproduces them without the probe.
//!
//! # The fixture
//!
//! `P6T13Probe.build`, the same board `tests/maze_drills.rs` uses; see its module docs. The items
//! this file names are `2` (the net-1 start pin, not routable), `6` (the bent net-2 blocker
//! trace), `7` (a net-3 via with **one** trace contact) and `9` (a net-3 via with **two**).

#![allow(clippy::too_many_lines)]

use std::cell::Cell;
use std::collections::BTreeSet;

use fr_board::ids::{ItemId, PadstackId, ViaInfoId};
use fr_board::prelude::*;
use fr_board::rules::{ViaInfo, ViaRule};
use fr_geometry::{
    IntBox, IntOctagon, IntPoint, IntVector, JavaRandom, Point, Polyline, Shape, TileShape,
};
use fr_router::autoroute::expansion::RoomRef;
use fr_router::autoroute::maze::engine::AutorouteEngine;
use fr_router::autoroute::maze::ripup_resolver::MazeRipupResolver;
use fr_router::autoroute::maze::search::MazeSearchEngine;
use fr_router::autoroute::maze::{AutorouteControl, MazeAdjustment, MazeListElement};
use fr_router::autoroute::path::Connection;
use fr_settings::RouterSettings;

// =================================================================================================
// The probe's board (`P6T13Probe.build`)
// =================================================================================================

const BOUNDING_BOX: IntBox = IntBox {
    ll: IntPoint {
        x: -4_000,
        y: -4_000,
    },
    ur: IntPoint { x: 4_000, y: 4_000 },
};

fn probe_board() -> Board {
    let layers = || LayerStructure::new(vec![Layer::new("front", true), Layer::new("back", true)]);
    let clearance_matrix = ClearanceMatrix::get_default_instance(&layers(), 200);
    let mut rules = BoardRules::new(layers(), clearance_matrix);
    rules.trace_angle_restriction = AngleRestriction::None;
    rules.set_default_trace_half_widths(30);

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
    let through_shape = Shape::Tile(TileShape::Octagon(IntOctagon::new(
        -70, -70, 70, 70, -140, 140, -140, 140,
    )));
    padstacks.add(
        "thru",
        vec![Some(through_shape.clone()), Some(through_shape)],
        true,
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

    rules
        .via_infos
        .add(ViaInfo::new("v", PadstackId(3), 1, false));
    let mut via_rule = ViaRule::new("rule");
    via_rule.append_via(rules.via_infos.get(ViaInfoId(0)).clone());
    rules.via_rules.push(via_rule);
    let default_class = rules.get_default_net_class();
    rules
        .net_classes
        .get_mut(default_class)
        .set_via_rule(Some(rules.via_rules[0].clone()));
    rules.nets.add("N1", 1, false, default_class);
    rules.nets.add("N2", 1, false, default_class);
    rules.nets.add("N3", 1, false, default_class);

    let mut board = Board::new(
        Vec::new(),
        0,
        BOUNDING_BOX,
        rules,
        BoardLibrary::new(padstacks, Packages::new()),
        Components::new(),
        Communication::default(),
    );

    let pkg1 = board.library.packages.add(
        "pkg1",
        vec![
            PackagePin::new("P1", PadstackId(1), IntVector::new(-2000, 0).into(), 0.0),
            PackagePin::new("P2", PadstackId(2), IntVector::new(2000, 0).into(), 0.0),
        ],
        None,
        None,
        None,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        true,
    );
    let pkg2 = board.library.packages.add(
        "pkg2",
        vec![
            PackagePin::new("P3", PadstackId(1), IntVector::new(0, -2000).into(), 0.0),
            PackagePin::new("P4", PadstackId(1), IntVector::new(0, 2000).into(), 0.0),
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
        .add_with_generated_name(Some(Point::new(0, 0)), 0.0, true, pkg1);
    board
        .components
        .add_with_generated_name(Some(Point::new(0, 0)), 0.0, true, pkg2);

    board.insert_pin(1, 0, vec![1], 1, FixedState::Unfixed);
    board.insert_pin(1, 1, vec![1], 1, FixedState::Unfixed);
    board.insert_pin(2, 0, vec![2], 1, FixedState::Unfixed);
    board.insert_pin(2, 1, vec![2], 1, FixedState::Unfixed);
    board.insert_trace_without_cleaning(
        Polyline::from_points(&[
            Point::new(0, -2000),
            Point::new(400, 0),
            Point::new(0, 2000),
        ]),
        0,
        30,
        vec![2],
        1,
        FixedState::Unfixed,
    );
    board
        .insert_via(
            PadstackId(3),
            Point::new(2500, 2500),
            vec![3],
            1,
            FixedState::Unfixed,
            false,
        )
        .expect("the free via inserts");
    board.insert_trace_without_cleaning(
        Polyline::from_points(&[Point::new(2500, 2500), Point::new(2500, 3500)]),
        0,
        30,
        vec![3],
        1,
        FixedState::Unfixed,
    );
    board
        .insert_via(
            PadstackId(3),
            Point::new(2500, -2500),
            vec![3],
            1,
            FixedState::Unfixed,
            false,
        )
        .expect("the two-contact via inserts");
    board.insert_trace_without_cleaning(
        Polyline::from_points(&[Point::new(2500, -2500), Point::new(2500, -3500)]),
        0,
        30,
        vec![3],
        1,
        FixedState::Unfixed,
    );
    board.insert_trace_without_cleaning(
        Polyline::from_points(&[Point::new(2500, -2500), Point::new(3500, -2500)]),
        0,
        30,
        vec![3],
        1,
        FixedState::Unfixed,
    );
    board
}

fn probe_control(board: &Board, net_no: i32) -> AutorouteControl {
    let mut settings = RouterSettings::new();
    settings.set_layer_count(board.get_layer_count());
    settings.apply_board_specific_optimizations(board);
    let trace_costs = settings.get_trace_costs();
    AutorouteControl::new(
        board,
        net_no,
        &settings,
        settings.get_via_costs(),
        &trace_costs,
    )
}

fn probe_engine(board: &mut Board, net_no: i32) -> AutorouteEngine {
    let mut engine = AutorouteEngine::new(board, 1, false);
    engine.init_connection(board, net_no, None);
    engine
}

struct Counter {
    calls: Cell<u32>,
}

impl Counter {
    fn new() -> Counter {
        Counter {
            calls: Cell::new(0),
        }
    }

    fn check(&self) -> bool {
        self.calls.set(self.calls.get() + 1);
        false
    }
}

fn set_of(ids: &[u32]) -> BTreeSet<ItemId> {
    ids.iter().map(|id| ItemId(*id)).collect()
}

/// The board, the engine and the control the ripup tests share, held together so the maze can
/// borrow the last two while the board stays a parameter.
struct Fixture {
    board: Board,
    engine: AutorouteEngine,
    ctrl: AutorouteControl,
}

fn fixture() -> Fixture {
    let mut board = probe_board();
    let engine = probe_engine(&mut board, 1);
    let ctrl = probe_control(&board, 1);
    Fixture {
        board,
        engine,
        ctrl,
    }
}

// =================================================================================================
// calcFanoutViaRipupCostFactor (:35-66) — probe mode `fanoutfac`
// =================================================================================================

#[test]
fn the_fanout_via_cost_factor_answers_javas_table() {
    // === mode fanoutfac ===
    // trace id=6  halfWidth=30 length=4_079.215610874 factor=1.081730769
    // trace id=8  halfWidth=30 length=1_000.000000000 factor=1.000000000
    // trace id=10 halfWidth=30 length=1_000.000000000 factor=1.000000000
    // trace id=11 halfWidth=30 length=1_000.000000000 factor=1.000000000
    // attached    halfWidth=30 length=600.000000000  factor=50.000000000
    let mut board = probe_board();
    for (id, want) in [(6u32, 1.081_730_769), (8, 1.0), (10, 1.0), (11, 1.0)] {
        let factor = MazeRipupResolver::calc_fanout_via_ripup_cost_factor(&board, ItemId(id));
        assert!(
            (factor - want).abs() < 5e-10,
            "trace {id}: {factor} != {want}"
        );
    }

    // The `SHOVE_FIXED` two-corner arm of `:51-56`: a short trace whose only contact is a
    // shove-fixed two-corner stub off the free via.
    board.insert_trace_without_cleaning(
        Polyline::from_points(&[Point::new(2500, 2500), Point::new(2500, 2900)]),
        1,
        30,
        vec![3],
        1,
        FixedState::ShoveFixed,
    );
    let attached = board
        .insert_trace_without_cleaning(
            Polyline::from_points(&[Point::new(2500, 2900), Point::new(3100, 2900)]),
            1,
            30,
            vec![3],
            1,
            FixedState::Unfixed,
        )
        .expect("the attached trace inserts");
    let factor = MazeRipupResolver::calc_fanout_via_ripup_cost_factor(&board, attached);
    assert!((factor - 50.0).abs() < 5e-10, "{factor} != 50");
}

// =================================================================================================
// checkRipup (:72-197) — probe mode `ripup`
// =================================================================================================

/// The first seeded queue element, which every `checkRipup` case below is run against.
fn seeded_element(maze: &MazeSearchEngine<'_>) -> MazeListElement {
    maze.queue.iter().next().expect("a seeded element").clone()
}

#[test]
fn an_unroutable_obstacle_and_a_small_door_are_both_refused() {
    // === mode ripup ===
    // --- not routable      smdPin -> -1
    // --- doorIsSmall       blocker small -> -1
    let mut f = fixture();
    let counter = Counter::new();
    let mut maze = MazeSearchEngine::get_instance(
        &set_of(&[2]),
        &set_of(&[3]),
        &mut f.engine,
        &mut f.board,
        &f.ctrl,
        &|| counter.check(),
    )
    .expect("init succeeds");
    let from = seeded_element(&maze);
    // `:74-76` — a `Pin` is not routable.
    assert_eq!(
        MazeRipupResolver::check_ripup(&mut maze, &mut f.board, &from, ItemId(2), false),
        -1
    );
    // `:77-79` — the seeded door is 2-dimensional, so `enterThroughSmallDoor` refuses at `:220`.
    assert_eq!(
        MazeRipupResolver::check_ripup(&mut maze, &mut f.board, &from, ItemId(6), true),
        -1
    );
}

#[test]
fn a_trace_ripup_costs_the_ripup_cost_times_the_half_width_over_the_detour() {
    // === mode ripup ===
    // --- ripupCosts=1000    blocker -> 29431
    // --- ripupCosts=100000  blocker -> 2943136
    for (ripup_costs, want) in [(1000, 29431), (100_000, 2_943_136)] {
        let mut f = fixture();
        let counter = Counter::new();
        let mut ctrl = f.ctrl.clone();
        ctrl.ripup_costs = ripup_costs;
        let mut maze = MazeSearchEngine::get_instance(
            &set_of(&[2]),
            &set_of(&[3]),
            &mut f.engine,
            &mut f.board,
            &ctrl,
            &|| counter.check(),
        )
        .expect("init succeeds");
        let from = seeded_element(&maze);
        assert_eq!(
            MazeRipupResolver::check_ripup(&mut maze, &mut f.board, &from, ItemId(6), false),
            want,
            "ripupCosts={ripup_costs}"
        );
    }
}

#[test]
fn the_cost_is_clamped_to_max_int_over_one_hundred() {
    // === mode ripup ===
    // --- ripupCosts=2000000000  blocker -> 21474836
    // `Integer.MAX_VALUE / 100 == 21474836` (`:167-168`), and `(int) ripupCost` on a double past
    // `Integer.MAX_VALUE` saturates before the clamp even sees it (`:166`).
    let mut f = fixture();
    let counter = Counter::new();
    let mut ctrl = f.ctrl.clone();
    ctrl.ripup_costs = 2_000_000_000;
    let mut maze = MazeSearchEngine::get_instance(
        &set_of(&[2]),
        &set_of(&[3]),
        &mut f.engine,
        &mut f.board,
        &ctrl,
        &|| counter.check(),
    )
    .expect("init succeeds");
    let from = seeded_element(&maze);
    assert_eq!(
        MazeRipupResolver::check_ripup(&mut maze, &mut f.board, &from, ItemId(6), false),
        21_474_836
    );
    assert_eq!(i32::MAX / 100, 21_474_836);
}

#[test]
fn a_via_cost_factor_scales_with_its_contact_count() {
    // === mode ripup ===
    // --- ripupCosts=1000        freeVia -> 1        twoContactVia -> 1
    // --- ripupCosts=100000      freeVia -> 1        twoContactVia -> 1
    // --- ripupCosts=2000000000  freeVia -> 1        twoContactVia -> 13
    //
    // `:123-125`: `costFactor *= 0.5 * max(contactCount - 1, 0)`, which is **zero** for the
    // one-contact via — so its whole ripup cost collapses to the `max(…, 1)` of `:166`.
    for (ripup_costs, free_via, two_contact) in
        [(1000, 1, 1), (100_000, 1, 1), (2_000_000_000, 1, 13)]
    {
        let mut f = fixture();
        let counter = Counter::new();
        let mut ctrl = f.ctrl.clone();
        ctrl.ripup_costs = ripup_costs;
        let mut maze = MazeSearchEngine::get_instance(
            &set_of(&[2]),
            &set_of(&[3]),
            &mut f.engine,
            &mut f.board,
            &ctrl,
            &|| counter.check(),
        )
        .expect("init succeeds");
        let from = seeded_element(&maze);
        assert_eq!(
            MazeRipupResolver::check_ripup(&mut maze, &mut f.board, &from, ItemId(7), false),
            free_via,
            "freeVia ripupCosts={ripup_costs}"
        );
        assert_eq!(
            MazeRipupResolver::check_ripup(&mut maze, &mut f.board, &from, ItemId(9), false),
            two_contact,
            "twoContactVia ripupCosts={ripup_costs}"
        );
    }
}

#[test]
fn the_fanout_protection_and_the_fanout_control_change_the_price() {
    // === mode ripup ===
    // --- removeUnconnectedVias=false (fanout protection on)
    // blocker -> 29431   freeVia -> 1   twoContactVia -> 1
    // --- isFanout=true
    // blocker -> 30000
    let mut f = fixture();
    let counter = Counter::new();
    let mut ctrl = f.ctrl.clone();
    ctrl.remove_unconnected_vias = false;
    let mut fanout_ctrl = f.ctrl.clone();
    fanout_ctrl.is_fanout = true;
    let mut maze = MazeSearchEngine::get_instance(
        &set_of(&[2]),
        &set_of(&[3]),
        &mut f.engine,
        &mut f.board,
        &ctrl,
        &|| counter.check(),
    )
    .expect("init succeeds");
    let from = seeded_element(&maze);
    // The blocker's own fanout factor is 1.081…, which is `> 1`, so `:134` skips the connection
    // detour and `:165` multiplies by it instead — and the two happen to answer the same 29431.
    assert_eq!(
        MazeRipupResolver::check_ripup(&mut maze, &mut f.board, &from, ItemId(6), false),
        29431
    );
    assert_eq!(
        MazeRipupResolver::check_ripup(&mut maze, &mut f.board, &from, ItemId(7), false),
        1
    );
    assert_eq!(
        MazeRipupResolver::check_ripup(&mut maze, &mut f.board, &from, ItemId(9), false),
        1
    );

    // `:134`'s `!ctrl.isFanout`: the detour stays 1, so the price is the bare
    // `ripupCosts * halfWidth`.
    maze.ctrl = &fanout_ctrl;
    assert_eq!(
        MazeRipupResolver::check_ripup(&mut maze, &mut f.board, &from, ItemId(6), false),
        30000
    );
}

#[test]
fn pass_four_randomises_and_pass_six_does_not() {
    // === mode ripup ===  --- randomize
    // passNo=3 blocker -> 29431   (no draw: `ripupPassNo >= 4` is false)
    // passNo=4 blocker -> 29303   (draw 1)
    // passNo=5 blocker -> 35440   (draw 2)
    // passNo=6 blocker -> 29431   (no draw: `ripupPassNo % 3 != 0` is false)
    // passNo=7 blocker -> 21087   (draw 3)
    //
    // The draws come from the one `Random` of the whole search, seeded with `ctrl.ripupCosts`
    // (1000) at `MazeSearchEngine.java:79-80`, so the five calls have to run in this order on
    // one engine.
    let mut f = fixture();
    let counter = Counter::new();
    let passes = [(3, 29431), (4, 29303), (5, 35440), (6, 29431), (7, 21087)];
    // The controls have to outlive the maze, which borrows one of them for its whole life.
    let controls: Vec<AutorouteControl> = passes
        .iter()
        .map(|(pass_no, _)| {
            let mut ctrl = f.ctrl.clone();
            ctrl.ripup_pass_no = *pass_no;
            ctrl
        })
        .collect();
    let mut maze = MazeSearchEngine::get_instance(
        &set_of(&[2]),
        &set_of(&[3]),
        &mut f.engine,
        &mut f.board,
        &controls[0],
        &|| counter.check(),
    )
    .expect("init succeeds");
    let from = seeded_element(&maze);
    for (index, (pass_no, want)) in passes.into_iter().enumerate() {
        maze.ctrl = &controls[index];
        let got = MazeRipupResolver::check_ripup(&mut maze, &mut f.board, &from, ItemId(6), false);
        assert_eq!(got, want, "passNo={pass_no}");
    }
}

#[test]
fn the_random_draw_matches_the_jvm() {
    // ruling 5's evidence. `jshell`:
    //   for (long s : new long[]{1000L, 5000L, 17L}) {
    //     var r = new java.util.Random(s);
    //     System.out.printf("%d %.17f %.17f %.17f%n", s, r.nextDouble(), r.nextDouble(),
    //         r.nextDouble());
    //   }
    // and the same three lines are `=== mode random ===` of the committed transcript.
    for (seed, want) in [
        (
            1000i64,
            [
                0.710_184_905_632_070_7,
                0.574_836_350_385_667,
                0.946_419_209_479_207_3,
            ],
        ),
        (
            5000,
            [
                0.085_709_861_872_486_53,
                0.844_727_162_596_632_8,
                0.259_628_380_197_038_93,
            ],
        ),
        (
            17,
            [
                0.732_311_513_959_731_6,
                0.697_370_478_360_749_7,
                0.082_956_111_450_170_68,
            ],
        ),
    ] {
        let mut random = JavaRandom::new(0);
        random.set_seed(seed);
        for (index, expected) in want.into_iter().enumerate() {
            let got = random.next_double();
            assert!(
                (got - expected).abs() < 1e-17,
                "seed {seed} draw {index}: {got} != {expected}"
            );
        }
    }
}

// =================================================================================================
// Connection (Connection.java:39-154) — probe mode `conn`
// =================================================================================================

#[test]
fn the_detour_is_memoised_through_the_item_autoroute_info() {
    // === mode conn ===
    // item 2..5 -> null   (a Pin is not routable)
    // item 6  -> items=[6]      start=(0,2000)  end=(0,-2000)  traceLength=4_079.215610874
    //            detour=1.019320881   memoised=true
    // item 7  -> items=[8,7]    start=null end=null traceLength=1000  detour=2147483647
    // item 9  -> items=[11,10,9] start=null end=null traceLength=2000 detour=2147483647
    //
    // The two `items=[…]` lists print in Java's `TreeSet<Item>` order, i.e. **descending** id;
    // the port's `BTreeSet<ItemId>` is ascending, so the assertions below read `[7, 8]` and
    // `[9, 10, 11]` for the same membership. Nothing consumes the order — see
    // `autoroute/path/connection.rs`' type docs.
    let mut f = fixture();
    for id in [2u32, 3, 4, 5] {
        assert_eq!(
            Connection::get(&mut f.board, &mut f.engine.connections, ItemId(id)),
            None,
            "item {id}"
        );
    }

    let blocker = Connection::get(&mut f.board, &mut f.engine.connections, ItemId(6))
        .expect("the blocker has a connection");
    {
        let connection = f
            .engine
            .connections
            .get(blocker.0)
            .expect("the arena holds it");
        assert_eq!(
            connection.item_list.iter().copied().collect::<Vec<_>>(),
            vec![ItemId(6)]
        );
        assert_eq!(connection.start_point, Some(Point::new(0, 2000)));
        assert_eq!(connection.end_point, Some(Point::new(0, -2000)));
        assert_eq!(connection.start_layer, 0);
        assert_eq!(connection.end_layer, 0);
        let trace_length = connection.trace_length(&f.board);
        assert!(
            (trace_length - 4_079.215_610_874).abs() < 5e-10,
            "{trace_length}"
        );
        let detour = connection.get_detour(&f.board);
        assert!((detour - 1.019_320_881).abs() < 5e-10, "{detour}");
    }
    // `:43-46`, the memo: the second call answers the same arena slot without allocating.
    assert_eq!(f.engine.connections.len(), 1);
    assert_eq!(
        Connection::get(&mut f.board, &mut f.engine.connections, ItemId(6)),
        Some(blocker)
    );
    assert_eq!(f.engine.connections.len(), 1);
    // `:126-128` writes the memo through **every** item of the connection.
    assert_eq!(
        fr_router::autoroute::item_info::get_precalculated_connection(&mut f.board, ItemId(6)),
        Some(blocker)
    );

    // A via and its trace share one connection, and it ends in empty space at both ends, so
    // `getDetour` answers `Integer.MAX_VALUE` (`:148-150`).
    let via = Connection::get(&mut f.board, &mut f.engine.connections, ItemId(7))
        .expect("the free via has a connection");
    let connection = f.engine.connections.get(via.0).expect("the arena holds it");
    assert_eq!(
        connection.item_list.iter().copied().collect::<Vec<_>>(),
        vec![ItemId(7), ItemId(8)]
    );
    assert_eq!(connection.start_point, None);
    assert_eq!(connection.end_point, None);
    assert!((connection.trace_length(&f.board) - 1_000.0).abs() < 5e-10);
    assert!((connection.get_detour(&f.board) - 2_147_483_647.0).abs() < 1e-6);
    // The trace reaches the same memoised connection.
    assert_eq!(
        Connection::get(&mut f.board, &mut f.engine.connections, ItemId(8)),
        Some(via)
    );

    let two_contact = Connection::get(&mut f.board, &mut f.engine.connections, ItemId(9))
        .expect("the two-contact via has a connection");
    let connection = f
        .engine
        .connections
        .get(two_contact.0)
        .expect("the arena holds it");
    assert_eq!(
        connection.item_list.iter().copied().collect::<Vec<_>>(),
        vec![ItemId(9), ItemId(10), ItemId(11)]
    );
    assert!((connection.trace_length(&f.board) - 2_000.0).abs() < 5e-10);
}

// =================================================================================================
// enterThroughSmallDoor (:219-268) and checkLeavingRippedItem (:200-213) — mode `smalldoor`
// =================================================================================================

#[test]
fn the_small_door_check_answers_javas_table_over_every_completed_door() {
    // === mode smalldoor ===
    // from door=TargetItemExpansionDoor id=63 dimension=2 -> false
    // checkLeavingRippedItem(from) -> false
    // room 1: doors 33/35/37    blocker=true via=true  leaving=false
    // room 2: door 33           blocker=true via=true  leaving=false
    //         door 6206         blocker=true via=false leaving=true
    //         door 1537988033   blocker=true via=true  leaving=false
    //         door -1901917981  blocker=true via=true  leaving=false
    //         door 8181055      blocker=true via=true  leaving=false
    //         door 8209730      blocker=true via=false leaving=false
    //         door 1619462847   blocker=true via=true  leaving=false
    //         door -576926342   blocker=true via=true  leaving=false
    //         door 68           blocker=true via=true  leaving=false
    // room 6: door 37           blocker=true via=true  leaving=false
    //         door 68           blocker=true via=true  leaving=false
    //         door 6331         blocker=true via=false leaving=true
    let mut f = fixture();
    let counter = Counter::new();
    let mut maze = MazeSearchEngine::get_instance(
        &set_of(&[2]),
        &set_of(&[3]),
        &mut f.engine,
        &mut f.board,
        &f.ctrl,
        &|| counter.check(),
    )
    .expect("init succeeds");
    let from = seeded_element(&maze);
    // `:220`: a `TargetItemExpansionDoor`'s dimension is 2, so the check refuses at once, and
    // `checkLeavingRippedItem` refuses even earlier at `:201-203` (the door is not an
    // `ExpansionDoor`).
    assert!(!MazeRipupResolver::enter_through_small_door(
        &mut maze,
        &mut f.board,
        &from,
        ItemId(6)
    ));
    assert!(!MazeRipupResolver::check_leaving_ripped_item(
        &mut maze,
        &mut f.board,
        &from
    ));

    let seeded: Vec<MazeListElement> = maze.queue.iter().cloned().collect();
    for element in &seeded {
        let room = element.next_room.expect("a next room");
        maze.engine.complete_neighbour_rooms(&mut f.board, room);
    }

    let mut table: Vec<(i32, i32, bool, bool, bool)> = Vec::new();
    let rooms: Vec<_> = maze.engine.complete_expansion_rooms().to_vec();
    for room in rooms {
        let room_ref = RoomRef::Complete(room);
        let room_id = maze
            .engine
            .rooms
            .room_id_no(room_ref)
            .expect("a completed room has an id");
        for door in maze.engine.rooms.room_doors(room_ref).to_vec() {
            let element = MazeListElement {
                door: fr_router::autoroute::expansion::ExpandableRef::Door(door),
                section_no_of_door: 0,
                backtrack_door: Some(from.door),
                section_no_of_backtrack_door: 0,
                expansion_value: 0.0,
                sorting_value: 0.0,
                next_room: Some(room_ref),
                shape_entry: from.shape_entry,
                room_ripped: false,
                adjustment: MazeAdjustment::None,
                already_checked: false,
                ripup_cost: 0,
            };
            let blocker = MazeRipupResolver::enter_through_small_door(
                &mut maze,
                &mut f.board,
                &element,
                ItemId(6),
            );
            let via = MazeRipupResolver::enter_through_small_door(
                &mut maze,
                &mut f.board,
                &element,
                ItemId(7),
            );
            let leaving =
                MazeRipupResolver::check_leaving_ripped_item(&mut maze, &mut f.board, &element);
            let door_id = maze
                .engine
                .rooms
                .door_id_no(door)
                .expect("a live door has an id");
            table.push((room_id, door_id, blocker, via, leaving));
        }
    }
    // PORT-REGRESSION PIN, `accepted at plan9-t7t8 (ruling CC)`. Two things moved and both are
    // named in Task 8's register rows.
    //
    // **The ids.** Room ids come from ONE shared counter across the engine's room kinds
    // (#156/#167/#158) and door ids derive from them, so the jar's rooms `1, 2, 6` are the port's
    // `3, 4, 15`, and the jar's mixed small-and-hash door ids
    // (`33, 35, 37, 6206, 1537988033, -1901917981, 8181055, 8209730, 1619462847, -576926342, 68,
    // 6331`) are the port's consecutive `97, 108, 129..135, 139, 481`.
    //
    // **One row is gone: fifteen doors became fourteen.** The jar's first room carries three doors
    // and the port's carries two; the missing one is the jar's door `35`, the only door of that
    // room that no OTHER room shares — its `33` and `37` are shared with the second and third
    // rooms and both survive as the port's `97` and `108`. That is a door-set change, from the
    // Task 8 fixes to the neighbour walk (#163, #171, #165b), and the ablation run at this wave
    // rules #165's second half out: with `detach_all_doors` reverted this table is unchanged.
    // The wave did not separate #163 from #171 here, and this comment does not claim it did.
    //
    // **What the test is named for is the TABLE, and the table is the jar's row for row on all
    // fourteen that remain.** Lay the two side by side and every boolean triple matches in order:
    // the second room's nine rows are `TTF, TFT, TTF, TTF, TTF, TFF, TTF, TTF, TTF` on both
    // sides, the third room's three are `TTF, TTF, TFT` on both, and the first room's survivors
    // are `TTF, TTF`. Every interesting row — the two `via=false leaving=true` and the one
    // `via=false leaving=false` — is still produced. The assertion below is written to say that
    // rather than only to hold the numbers.
    let shape: Vec<(i32, bool, bool, bool)> = table.iter().map(|r| (r.0, r.2, r.3, r.4)).collect();
    assert_eq!(
        shape,
        vec![
            (3, true, true, false),
            (3, true, true, false),
            (4, true, true, false),
            (4, true, false, true),
            (4, true, true, false),
            (4, true, true, false),
            (4, true, true, false),
            (4, true, false, false),
            (4, true, true, false),
            (4, true, true, false),
            (4, true, true, false),
            (15, true, true, false),
            (15, true, true, false),
            (15, true, false, true),
        ],
        "the answers of the small-door check, per room, in order"
    );
    assert_eq!(
        table,
        vec![
            (3, 97, true, true, false),
            (3, 108, true, true, false),
            (4, 97, true, true, false),
            (4, 129, true, false, true),
            (4, 130, true, true, false),
            (4, 131, true, true, false),
            (4, 132, true, true, false),
            (4, 133, true, false, false),
            (4, 134, true, true, false),
            (4, 135, true, true, false),
            (4, 139, true, true, false),
            (15, 108, true, true, false),
            (15, 139, true, true, false),
            (15, 481, true, false, true),
        ]
    );
}
