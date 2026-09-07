#![allow(clippy::too_many_lines)]

use std::cell::Cell;
use std::collections::BTreeSet;

use copper_board::ids::{ItemId, PadstackId, ViaInfoId};
use copper_board::prelude::*;
use copper_board::rules::{ViaInfo, ViaRule};
use copper_geometry::{IntBox, IntOctagon, IntPoint, IntVector, Point, Polyline, Shape, TileShape};
use copper_router::autoroute::expansion::RoomRef;
use copper_router::autoroute::maze::engine::AutorouteEngine;
use copper_router::autoroute::maze::ripup_resolver::MazeRipupResolver;
use copper_router::autoroute::maze::search::MazeSearchEngine;
use copper_router::autoroute::maze::{AutorouteControl, MazeAdjustment, MazeListElement};
use copper_router::autoroute::path::Connection;
use copper_settings::RouterSettings;

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

#[test]
fn the_fanout_via_cost_factor_answers_javas_table() {
    let mut board = probe_board();
    for (id, want) in [(6u32, 1.081_730_769), (8, 1.0), (10, 1.0), (11, 1.0)] {
        let factor = MazeRipupResolver::calc_fanout_via_ripup_cost_factor(&board, ItemId(id));
        assert!(
            (factor - want).abs() < 5e-10,
            "trace {id}: {factor} != {want}"
        );
    }

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

fn seeded_element(maze: &MazeSearchEngine<'_>) -> MazeListElement {
    maze.queue.iter().next().expect("a seeded element").clone()
}

#[test]
fn an_unroutable_obstacle_and_a_small_door_are_both_refused() {
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
    assert_eq!(
        MazeRipupResolver::check_ripup(&mut maze, &mut f.board, &from, ItemId(2), false),
        -1
    );
    assert_eq!(
        MazeRipupResolver::check_ripup(&mut maze, &mut f.board, &from, ItemId(6), true),
        -1
    );
}

#[test]
fn a_trace_ripup_costs_the_ripup_cost_times_the_half_width_over_the_detour() {
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

    maze.ctrl = &fanout_ctrl;
    assert_eq!(
        MazeRipupResolver::check_ripup(&mut maze, &mut f.board, &from, ItemId(6), false),
        30000
    );
}

#[test]
fn pass_four_randomises_and_pass_six_does_not() {
    let mut f = fixture();
    let counter = Counter::new();
    let passes = [(3, 29431), (4, 53014), (5, 25302), (6, 29431), (7, 26800)];
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
fn the_detour_is_memoised_through_the_item_autoroute_info() {
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
    assert_eq!(f.engine.connections.len(), 1);
    assert_eq!(
        Connection::get(&mut f.board, &mut f.engine.connections, ItemId(6)),
        Some(blocker)
    );
    assert_eq!(f.engine.connections.len(), 1);
    assert_eq!(
        copper_router::autoroute::item_info::get_precalculated_connection(&mut f.board, ItemId(6)),
        Some(blocker)
    );

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

#[test]
fn the_small_door_check_answers_javas_table_over_every_completed_door() {
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
                door: copper_router::autoroute::expansion::ExpandableRef::Door(door),
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
