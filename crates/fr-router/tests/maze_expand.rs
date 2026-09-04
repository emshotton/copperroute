#![allow(clippy::too_many_lines)]

use std::cell::Cell;
use std::collections::BTreeSet;

use fr_board::ids::{ItemId, ObstacleRoomId};
use fr_board::prelude::*;
use fr_board::rules::ViaRule;
use fr_geometry::{
    Area, FloatLine, FloatPoint, IntBox, IntOctagon, IntPoint, IntVector, Point, Polyline, Shape,
    TileShape,
};
use fr_router::arena::DoorId;
use fr_router::autoroute::expansion::{ExpandableRef, RoomRef};
use fr_router::autoroute::maze::engine::AutorouteEngine;
use fr_router::autoroute::maze::search::MazeSearchEngine;
use fr_router::autoroute::maze::trace_shover::{DoorSection, MazeTraceShover};
use fr_router::autoroute::maze::{AutorouteControl, MazeAdjustment, MazeListElement};
use fr_settings::RouterSettings;


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

fn probe_board() -> Board {
    let layers = || LayerStructure::new(vec![Layer::new("front", true), Layer::new("back", true)]);
    let mut clearance_matrix = ClearanceMatrix::get_default_instance(&layers(), 200);
    assert!(clearance_matrix.append_class("wide"));
    clearance_matrix.set_value_on_all_layers(2, 1, 600);
    clearance_matrix.set_value_on_all_layers(2, 2, 800);
    let mut rules = BoardRules::new(layers(), clearance_matrix);
    rules.trace_angle_restriction = AngleRestriction::None;
    rules.via_rules.push(ViaRule::new("empty"));
    let default_class = rules.get_default_net_class();
    rules
        .net_classes
        .get_mut(default_class)
        .set_via_rule(Some(rules.via_rules[0].clone()));
    rules.nets.add("N1", 1, false, default_class);
    rules.nets.add("N2", 1, false, default_class);

    let mut padstacks = Padstacks::new(layers());
    let smd = padstacks.add(
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
    let through = padstacks.add(
        "thru",
        vec![Some(through_shape.clone()), Some(through_shape)],
        true,
        false,
    );
    let mut packages = Packages::new();
    let package = packages.add(
        "pkg",
        vec![
            PackagePin::new("P1", smd, IntVector::new(-500, 0).into(), 0.0),
            PackagePin::new("P2", through, IntVector::new(500, 0).into(), 0.0),
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

    let mut board = Board::new(
        Vec::new(),
        0,
        BOUNDING_BOX,
        rules,
        BoardLibrary::new(padstacks, packages),
        components,
        Communication::default(),
    );
    board.insert_pin(1, 0, vec![1], 1, FixedState::Unfixed);
    board.insert_pin(1, 1, vec![1], 1, FixedState::Unfixed);
    board.insert_obstacle(
        Area::Shape(Shape::Tile(TileShape::Box(IntBox::from_coords(
            700, -1000, 900, 1000,
        )))),
        0,
        1,
        FixedState::Unfixed,
    );
    board
}

fn probe_settings(board: &Board) -> RouterSettings {
    let mut settings = RouterSettings::new();
    settings.set_layer_count(board.get_layer_count());
    settings.apply_board_specific_optimizations(board);
    settings
}

fn probe_control(board: &Board, net_no: i32) -> AutorouteControl {
    let settings = probe_settings(board);
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

type QueueRow = (
    i32,
    i32,
    f64,
    f64,
    Option<i32>,
    (f64, f64),
    (f64, f64),
    bool,
    MazeAdjustment,
    bool,
    i32,
);

fn r9(x: f64) -> f64 {
    (x * 1e9).round() / 1e9
}

fn r6(p: &FloatPoint) -> (f64, f64) {
    ((p.x * 1e6).round() / 1e6, (p.y * 1e6).round() / 1e6)
}

fn queue_rows(maze: &MazeSearchEngine<'_>) -> Vec<QueueRow> {
    maze.queue
        .iter()
        .map(|e| {
            (
                maze.engine.expandable_id_no(e.door),
                e.section_no_of_door,
                r9(e.expansion_value),
                r9(e.sorting_value),
                e.next_room
                    .and_then(|room| maze.engine.rooms.room_id_no(room)),
                r6(&e.shape_entry.a),
                r6(&e.shape_entry.b),
                e.room_ripped,
                e.adjustment,
                e.already_checked,
                e.ripup_cost,
            )
        })
        .collect()
}

fn expand_control(board: &Board, net_no: i32) -> AutorouteControl {
    let mut ctrl = probe_control(board, net_no);
    ctrl.vias_allowed = false;
    ctrl
}

fn drain(maze: &mut MazeSearchEngine<'_>) {
    while maze.queue.pop_first().is_some() {}
}

fn door_at(maze: &mut MazeSearchEngine<'_>, x: i32, y: i32, id1: i32, id2: i32) -> DoorId {
    let a = maze.engine.rooms.new_complete_room(
        Some(TileShape::Box(IntBox::from_coords(
            x - 500,
            y - 100,
            x,
            y + 100,
        ))),
        0,
        id1,
    );
    let b = maze.engine.rooms.new_complete_room(
        Some(TileShape::Box(IntBox::from_coords(
            x,
            y - 100,
            x + 500,
            y + 100,
        ))),
        0,
        id2,
    );
    maze.engine
        .rooms
        .new_door(RoomRef::Complete(a), RoomRef::Complete(b), 1)
}


#[test]
fn the_first_pop_expands_the_start_room_through_every_door_of_it() {
    let mut board = probe_board();
    let mut engine = probe_engine(&mut board, 1);
    let ctrl = expand_control(&board, 1);
    let counter = Counter::new();
    let mut maze = MazeSearchEngine::get_instance(
        &set_of(&[2]),
        &set_of(&[3]),
        &mut engine,
        &mut board,
        &ctrl,
        &|| counter.check(),
    )
    .expect("init succeeds on this board");

    assert_eq!(maze.queue.len(), 2, "init seeded two elements");
    assert_eq!(maze.engine.complete_expansion_rooms().len(), 3);

    assert!(maze.occupy_next_element(&mut board, &|| counter.check()));
    assert_eq!(
        maze.engine.complete_expansion_rooms().len(),
        4,
        "completeNeighbourRooms added room 5"
    );

    assert_eq!(
        queue_rows(&maze),
        vec![
            (
                75,
                0,
                0.0,
                830.0,
                Some(13),
                (-500.0, 0.0),
                (-500.0, 0.0),
                false,
                MazeAdjustment::None,
                false,
                0
            ),
            (
                100,
                0,
                1000.0,
                1000.0,
                None,
                (500.0, 0.0),
                (500.0, 0.0),
                false,
                MazeAdjustment::None,
                false,
                0
            ),
            (
                230,
                0,
                1550.0,
                3930.0,
                Some(13),
                (-3098.0, 0.0),
                (-1002.0, 0.0),
                false,
                MazeAdjustment::None,
                false,
                0
            ),
            (
                232,
                0,
                11_149.268_182_262,
                20_379.268_182_262,
                Some(15),
                (-3_884.326_137, -8_621.203_369),
                (-215.673_863, -2_419.796_631),
                false,
                MazeAdjustment::None,
                false,
                0
            ),
            (
                193,
                0,
                10_846.197_490_365,
                21_737.317_726_594,
                Some(6),
                (-4700.0, -8398.0),
                (-4700.0, -1602.0),
                false,
                MazeAdjustment::None,
                false,
                0
            ),
        ]
    );
    assert_eq!(maze.destination_door(), None);
}

#[test]
fn the_second_pop_into_a_thin_room_expands_nothing() {
    let mut board = probe_board();
    let mut engine = probe_engine(&mut board, 1);
    let ctrl = expand_control(&board, 1);
    let counter = Counter::new();
    let mut maze = MazeSearchEngine::get_instance(
        &set_of(&[2]),
        &set_of(&[3]),
        &mut engine,
        &mut board,
        &ctrl,
        &|| counter.check(),
    )
    .expect("init succeeds on this board");

    assert!(maze.occupy_next_element(&mut board, &|| counter.check()));
    let after_first = queue_rows(&maze);
    assert!(maze.occupy_next_element(&mut board, &|| counter.check()));
    assert_eq!(
        maze.engine.complete_expansion_rooms().len(),
        5,
        "room 4's neighbours were completed in their turn"
    );
    assert_eq!(
        queue_rows(&maze),
        after_first[1..].to_vec(),
        "nothing was added, only the head removed"
    );
}

#[test]
fn an_inactive_layer_is_never_expanded_into() {
    let mut board = probe_board();
    let mut engine = probe_engine(&mut board, 1);
    let mut ctrl = expand_control(&board, 1);
    let counter = Counter::new();
    assert!(
        board.layer_structure().layers[0].is_signal,
        "layer0isSignal=true"
    );
    ctrl.layer_active[0] = false;
    let mut maze = MazeSearchEngine::get_instance(
        &set_of(&[2]),
        &set_of(&[3]),
        &mut engine,
        &mut board,
        &ctrl,
        &|| counter.check(),
    )
    .expect("init succeeds on this board");
    let before = queue_rows(&maze);
    let first = maze.queue.iter().next().expect("two seeded").clone();
    assert!(maze.expand_to_room_doors(&mut board, &first));
    assert_eq!(queue_rows(&maze), before);
    assert_eq!(maze.engine.complete_expansion_rooms().len(), 3);
}


fn bend_fixture(
    maze: &mut MazeSearchEngine<'_>,
    dy: i32,
    room_ripped_parent: bool,
) -> (MazeListElement, DoorId) {
    let from_room = maze.engine.rooms.new_complete_room(
        Some(TileShape::Box(IntBox::from_coords(
            -5000, -5000, 5000, 5000,
        ))),
        0,
        900,
    );
    let backtrack_door = door_at(maze, 0, 0, 901, 902);
    let to_door = door_at(maze, 2000, dy, 903, 904);
    assert_eq!(
        maze.engine
            .rooms
            .door_section_segments(to_door, 1600.0)
            .len(),
        1,
        "sections=1"
    );
    let from = MazeListElement {
        door: ExpandableRef::Door(backtrack_door),
        section_no_of_door: 0,
        backtrack_door: Some(ExpandableRef::Door(backtrack_door)),
        section_no_of_backtrack_door: 0,
        expansion_value: 0.0,
        sorting_value: 0.0,
        next_room: Some(RoomRef::Complete(from_room)),
        shape_entry: FloatLine::new(FloatPoint::new(1000.0, 0.0), FloatPoint::new(1000.0, 0.0)),
        room_ripped: room_ripped_parent,
        adjustment: MazeAdjustment::None,
        already_checked: room_ripped_parent,
        ripup_cost: 0,
    };
    (from, to_door)
}

#[test]
fn the_bend_penalty_fires_exactly_above_sin_squared_one_percent() {
    for (dy, expected_expansion, expected_sorting, bend_charged) in [
        (0, 1000.0, 2330.0, false),
        (100, 1_019.803_902_719, 2_349.803_902_719, false),
        (101, 1_120.198_019_994, 2_450.198_019_994, true),
        (1000, 2_336.067_977_5, 4_463.155_187_748, true),
    ] {
        let mut board = probe_board();
        let mut engine = probe_engine(&mut board, 1);
        let mut ctrl = probe_control(&board, 1);
        ctrl.bend_costs[0] = 100.0;
        let counter = Counter::new();
        let mut maze = MazeSearchEngine::get_instance(
            &set_of(&[2]),
            &set_of(&[3]),
            &mut engine,
            &mut board,
            &ctrl,
            &|| counter.check(),
        )
        .expect("init succeeds on this board");
        drain(&mut maze);

        let (from, to_door) = bend_fixture(&mut maze, dy, false);
        let shape_entry = FloatLine::new(
            FloatPoint::new(2000.0, f64::from(dy)),
            FloatPoint::new(2000.0, f64::from(dy)),
        );
        assert!(maze.expand_to_door_section(
            &mut board,
            ExpandableRef::Door(to_door),
            0,
            Some(&shape_entry),
            &from,
            0,
            MazeAdjustment::None,
        ));
        assert_eq!(
            queue_rows(&maze),
            vec![(
                28_897,
                0,
                expected_expansion,
                expected_sorting,
                None,
                (2000.0, f64::from(dy)),
                (2000.0, f64::from(dy)),
                false,
                MazeAdjustment::None,
                false,
                0
            )],
            "dy={dy}"
        );
        let straight = (1000.0f64 * 1000.0 + 4.0 * f64::from(dy) * f64::from(dy)).sqrt();
        let charged = r9(expected_expansion) - r9((straight * 1e9).round() / 1e9);
        assert!(
            (charged - if bend_charged { 100.0 } else { 0.0 }).abs() < 1e-6,
            "dy={dy}: bend penalty {charged}"
        );
    }
}

#[test]
fn room_ripped_and_the_ripup_cost_follow_add_costs_and_the_adjustment() {
    let cases: [(i32, MazeAdjustment, f64, bool, i32); 4] = [
        (0, MazeAdjustment::None, 1000.0, false, 0),
        (7, MazeAdjustment::None, 1007.0, true, 7),
        (7, MazeAdjustment::Right, 1007.0, false, 0),
        (7, MazeAdjustment::Left, 1007.0, false, 0),
    ];
    for (add_costs, adjustment, expansion, room_ripped, ripup_cost) in cases {
        let mut board = probe_board();
        let mut engine = probe_engine(&mut board, 1);
        let mut ctrl = probe_control(&board, 1);
        ctrl.bend_costs[0] = 100.0;
        let counter = Counter::new();
        let mut maze = MazeSearchEngine::get_instance(
            &set_of(&[2]),
            &set_of(&[3]),
            &mut engine,
            &mut board,
            &ctrl,
            &|| counter.check(),
        )
        .expect("init succeeds on this board");
        drain(&mut maze);
        let (from, to_door) = bend_fixture(&mut maze, 0, false);
        let shape_entry =
            FloatLine::new(FloatPoint::new(2000.0, 0.0), FloatPoint::new(2000.0, 0.0));
        assert!(maze.expand_to_door_section(
            &mut board,
            ExpandableRef::Door(to_door),
            0,
            Some(&shape_entry),
            &from,
            add_costs,
            adjustment,
        ));
        assert_eq!(
            queue_rows(&maze),
            vec![(
                28_897,
                0,
                expansion,
                expansion + 1330.0,
                None,
                (2000.0, 0.0),
                (2000.0, 0.0),
                room_ripped,
                adjustment,
                false,
                ripup_cost
            )],
            "addCosts={add_costs} adjustment={adjustment:?}"
        );
    }
}

#[test]
fn an_already_checked_ripped_parent_propagates_room_ripped_but_not_the_cost() {
    let mut board = probe_board();
    let mut engine = probe_engine(&mut board, 1);
    let mut ctrl = probe_control(&board, 1);
    ctrl.bend_costs[0] = 100.0;
    let counter = Counter::new();
    let mut maze = MazeSearchEngine::get_instance(
        &set_of(&[2]),
        &set_of(&[3]),
        &mut engine,
        &mut board,
        &ctrl,
        &|| counter.check(),
    )
    .expect("init succeeds on this board");
    drain(&mut maze);
    let (from, to_door) = bend_fixture(&mut maze, 0, true);
    let shape_entry = FloatLine::new(FloatPoint::new(2000.0, 0.0), FloatPoint::new(2000.0, 0.0));
    assert!(maze.expand_to_door_section(
        &mut board,
        ExpandableRef::Door(to_door),
        0,
        Some(&shape_entry),
        &from,
        0,
        MazeAdjustment::None,
    ));
    assert_eq!(
        queue_rows(&maze),
        vec![(
            28_897,
            0,
            1000.0,
            2330.0,
            None,
            (2000.0, 0.0),
            (2000.0, 0.0),
            true,
            MazeAdjustment::None,
            false,
            0
        )]
    );
}

#[test]
fn an_occupied_section_and_a_null_shape_entry_are_both_refused() {
    let mut board = probe_board();
    let mut engine = probe_engine(&mut board, 1);
    let ctrl = probe_control(&board, 1);
    let counter = Counter::new();
    let mut maze = MazeSearchEngine::get_instance(
        &set_of(&[2]),
        &set_of(&[3]),
        &mut engine,
        &mut board,
        &ctrl,
        &|| counter.check(),
    )
    .expect("init succeeds on this board");
    drain(&mut maze);
    let (from, to_door) = bend_fixture(&mut maze, 0, true);
    let shape_entry = FloatLine::new(FloatPoint::new(2000.0, 0.0), FloatPoint::new(2000.0, 0.0));

    maze.engine
        .rooms
        .door_mut(to_door)
        .expect("live door")
        .get_maze_search_element_mut(0)
        .expect("one section")
        .is_occupied = true;
    assert!(!maze.expand_to_door_section(
        &mut board,
        ExpandableRef::Door(to_door),
        0,
        Some(&shape_entry),
        &from,
        0,
        MazeAdjustment::None,
    ));
    maze.engine
        .rooms
        .door_mut(to_door)
        .expect("live door")
        .get_maze_search_element_mut(0)
        .expect("one section")
        .is_occupied = false;
    assert!(!maze.expand_to_door_section(
        &mut board,
        ExpandableRef::Door(to_door),
        0,
        None,
        &from,
        0,
        MazeAdjustment::None,
    ));
    assert_eq!(maze.queue.len(), 0);
}


#[test]
fn room_shape_is_thick_at_the_compensated_half_width_boundary() {
    let mut board = probe_board();
    {
        let mut engine = probe_engine(&mut board, 1);
        let ctrl = probe_control(&board, 1);
        let maze = MazeSearchEngine::new(&mut engine, &ctrl);
        assert_eq!(ctrl.compensated_trace_half_width[0], 1600);
        let tree = maze.search_tree;
        drop(maze);
        for item in [ItemId(4), ItemId(3)] {
            let room = engine.rooms.new_obstacle_room(&mut board, item, 0, tree);
            let maze = MazeSearchEngine::new(&mut engine, &ctrl);
            assert!(
                !maze.room_shape_is_thick(&board, room),
                "item {item:?} takes the unexpected-obstacle arm"
            );
            drop(maze);
        }
    }

    for half_width in [100, 1500, 1600, 2000] {
        board.insert_trace_without_cleaning(
            Polyline::from_points(&[
                Point::new(-9000, 5000 + half_width),
                Point::new(-3000, 5000 + half_width),
            ]),
            0,
            half_width,
            vec![2],
            1,
            FixedState::Unfixed,
        );
    }
    let mut engine = probe_engine(&mut board, 1);
    let ctrl = probe_control(&board, 1);
    let tree = engine.tree;
    let mut rows = Vec::new();
    for id in board.items_in_board_order() {
        if !matches!(board.items.get(&id), Some(Item::Trace(_))) {
            continue;
        }
        let room = engine.rooms.new_obstacle_room(&mut board, id, 0, tree);
        let maze = MazeSearchEngine::new(&mut engine, &ctrl);
        let half_width = match board.items.get(&id) {
            Some(Item::Trace(trace)) => trace.get_half_width(),
            _ => unreachable!(),
        };
        rows.push((id.0, half_width, maze.room_shape_is_thick(&board, room)));
        drop(maze);
    }
    assert_eq!(
        rows,
        vec![
            (8, 2000, true),
            (7, 1600, true),
            (6, 1500, true),
            (5, 100, false),
        ]
    );
    let _ = tree;
}

#[test]
fn check_neck_down_at_dest_pin_answers_the_first_pin_target_door_whichever_it_is() {
    let mut board = probe_board();
    let mut engine = probe_engine(&mut board, 1);
    let ctrl = probe_control(&board, 1);
    let counter = Counter::new();
    let maze = MazeSearchEngine::get_instance(
        &set_of(&[2]),
        &set_of(&[3]),
        &mut engine,
        &mut board,
        &ctrl,
        &|| counter.check(),
    )
    .expect("init succeeds on this board");

    let rooms: Vec<RoomRef> = maze
        .queue
        .iter()
        .map(|e| e.next_room.expect("a seeded element has a room"))
        .collect();
    let answers: Vec<(i32, f64)> = rooms
        .iter()
        .map(|room| {
            (
                maze.engine.rooms.room_id_no(*room).expect("a live room"),
                maze.check_neck_down_at_dest_pin(&board, *room),
            )
        })
        .collect();
    assert_eq!(answers, vec![(7, 49.0), (13, 49.0)]);
    let first_target_item = {
        let door = maze.engine.rooms.room_target_doors(rooms[0])[0];
        maze.engine.rooms.target_door(door).expect("live").item
    };
    assert_eq!(first_target_item, ItemId(2));

    let bare = maze.engine.rooms.new_complete_room(
        Some(TileShape::Box(IntBox::from_coords(0, 0, 10, 10))),
        0,
        950,
    );
    assert_eq!(
        maze.check_neck_down_at_dest_pin(&board, RoomRef::Complete(bare)),
        0.0
    );
}


#[test]
fn a_small_door_refuses_the_whole_round() {
    for (half_width, door_is_small, expanded) in [(1600, false, true), (100_000, true, false)] {
        let mut board = probe_board();
        let mut engine = probe_engine(&mut board, 1);
        let mut ctrl = expand_control(&board, 1);
        ctrl.compensated_trace_half_width[0] = half_width;
        let counter = Counter::new();
        let mut maze = MazeSearchEngine::get_instance(
            &set_of(&[2]),
            &set_of(&[3]),
            &mut engine,
            &mut board,
            &ctrl,
            &|| counter.check(),
        )
        .expect("init succeeds on this board");
        assert_eq!(maze.queue.len(), 2, "init is unaffected by the half width");

        let seed = maze.queue.iter().next().expect("two seeded").clone();
        let room = seed.next_room.expect("a seeded element has a room");
        let door = maze.engine.rooms.room_doors(room)[0];
        assert_eq!(maze.engine.rooms.door_id_no(door), Some(193));
        assert_eq!(
            maze.engine.rooms.door(door).expect("live").dimension,
            1,
            "dimension=1"
        );
        let door_shape = maze.engine.rooms.door_shape(door).expect("live");
        let b = door_shape.bounding_box();
        assert_eq!((b.ll.x, b.ll.y, b.ur.x, b.ur.y), (-4700, -10_000, -4700, 0));
        let centre = door_shape.centre_of_gravity();
        assert_eq!(r6(&centre), (-4700.0, -5000.0));

        assert_eq!(
            maze.door_is_small(&board, door, 2.0 * (f64::from(half_width) + 2.0)),
            door_is_small
        );

        let element = MazeListElement {
            door: ExpandableRef::Door(door),
            section_no_of_door: 0,
            backtrack_door: None,
            section_no_of_backtrack_door: 0,
            expansion_value: 0.0,
            sorting_value: 0.0,
            next_room: Some(room),
            shape_entry: FloatLine::new(centre, centre),
            room_ripped: false,
            adjustment: MazeAdjustment::None,
            already_checked: false,
            ripup_cost: 0,
        };
        drain(&mut maze);
        assert_eq!(maze.expand_to_room_doors(&mut board, &element), expanded);
        let ids: Vec<i32> = maze
            .queue
            .iter()
            .map(|e| maze.engine.expandable_id_no(e.door))
            .collect();
        if expanded {
            assert_eq!(ids, vec![100, 69, 232, 230]);
        } else {
            assert!(ids.is_empty(), "a small door expands nothing");
        }
    }
}

#[test]
fn the_door_list_is_snapshotted_after_completing_neighbours() {
    let mut board = probe_board();
    let mut engine = probe_engine(&mut board, 1);
    let ctrl = expand_control(&board, 1);
    let counter = Counter::new();
    let maze = MazeSearchEngine::get_instance(
        &set_of(&[2]),
        &set_of(&[3]),
        &mut engine,
        &mut board,
        &ctrl,
        &|| counter.check(),
    )
    .expect("init succeeds on this board");
    let room = maze
        .queue
        .iter()
        .next()
        .expect("two seeded")
        .next_room
        .expect("a room");
    let before: Vec<DoorId> = maze.engine.rooms.room_doors(room).to_vec();
    assert_eq!(before.len(), 4, "doorsBefore=4");

    maze.engine.complete_neighbour_rooms(&mut board, room);
    let after: Vec<DoorId> = maze.engine.rooms.room_doors(room).to_vec();
    assert_eq!(after.len(), 3, "doorsAfterCompletion=3");
        type DoorRow = (i32, i32, bool, (i32, i32, i32, i32));
    let rows: Vec<DoorRow> = after
        .iter()
        .map(|door| {
            let d = maze.engine.rooms.door(*door).expect("live");
            let b = maze
                .engine
                .rooms
                .door_shape(*door)
                .expect("live")
                .bounding_box();
            (
                maze.engine.rooms.door_id_no(*door).expect("live"),
                d.dimension,
                before.contains(door),
                (b.ll.x, b.ll.y, b.ur.x, b.ur.y),
            )
        })
        .collect();
    assert_eq!(
        rows,
        vec![
            (193, 1, true, (-4700, -10_000, -4700, 0)),
            (230, 1, true, (-4700, 0, 600, 0)),
            (232, 1, false, (-4700, -10_000, 600, -1041)),
        ]
    );
}

#[test]
fn a_stale_tree_entry_is_skipped_silently() {
    let mut board = probe_board();
    let mut engine = probe_engine(&mut board, 1);
    let ctrl = expand_control(&board, 1);
    let counter = Counter::new();
    let mut maze = MazeSearchEngine::get_instance(
        &set_of(&[2]),
        &set_of(&[3]),
        &mut engine,
        &mut board,
        &ctrl,
        &|| counter.check(),
    )
    .expect("init succeeds on this board");
    let seed = maze.queue.iter().next().expect("two seeded").clone();
    let room = seed.next_room.expect("a room");
    let mid = seed.shape_entry.a.middle_point(&seed.shape_entry.b);

    drain(&mut maze);
    assert!(maze.expand_to_target_doors(&mut board, &seed, true, false, &mid));
    assert_eq!(
        queue_rows(&maze),
        vec![(
            100,
            0,
            1000.0,
            1000.0,
            None,
            (500.0, 0.0),
            (500.0, 0.0),
            false,
            MazeAdjustment::None,
            false,
            0
        )]
    );

    drain(&mut maze);
    for door in maze.engine.rooms.room_target_doors(room).to_vec() {
        maze.engine
            .rooms
            .target_door_mut(door)
            .expect("live")
            .tree_entry_no = 99;
    }
    assert!(!maze.expand_to_target_doors(&mut board, &seed, true, false, &mid));
    assert_eq!(maze.queue.len(), 0);
}


#[test]
fn shove_trace_room_does_not_mutate_the_board() {
    let mut board = probe_board();
    let mut engine = probe_engine(&mut board, 1);
    let ctrl = expand_control(&board, 1);
    let counter = Counter::new();
    let mut maze = MazeSearchEngine::get_instance(
        &set_of(&[2]),
        &set_of(&[3]),
        &mut engine,
        &mut board,
        &ctrl,
        &|| counter.check(),
    )
    .expect("init succeeds on this board");
    let tree = maze.search_tree;
    let area_room = maze
        .engine
        .rooms
        .new_obstacle_room(&mut board, ItemId(4), 0, tree);
    let seed = maze.queue.iter().next().expect("two seeded").clone();

    let before = board.items.len();
    assert_eq!(before, 4, "itemCountBefore=4");
    assert!(maze.shove_trace_room(&mut board, &seed, area_room));
    assert_eq!(board.items.len(), 4, "itemCountAfter=4");

    let mut out: Vec<DoorSection> = Vec::new();
    assert!(MazeTraceShover::check_shove_trace_line(
        &seed,
        area_room,
        &mut maze.engine.rooms,
        &mut board,
        &ctrl,
        false,
        &mut out,
    ));
    assert!(out.is_empty(), "sections=0");
}

fn shove_board() -> Board {
    let mut board = probe_board();
    board.insert_trace_without_cleaning(
        Polyline::from_points(&[
            Point::new(-7000, -8000),
            Point::new(0, -8000),
            Point::new(0, -4500),
            Point::new(6000, -4500),
        ]),
        0,
        1500,
        vec![2],
        1,
        FixedState::Unfixed,
    );
    board
}

fn shove_doors(
    maze: &mut MazeSearchEngine<'_>,
    board: &mut Board,
    obstacle_room: ObstacleRoomId,
    corner_no: i32,
) -> DoorId {
    let room_ref = RoomRef::Obstacle(obstacle_room);
    let b = maze
        .engine
        .rooms
        .room_shape(room_ref)
        .expect("the room has a shape")
        .bounding_box();
    let mid_x = (b.ll.x + b.ur.x) / 2;
    let mid_y = (b.ll.y + b.ur.y) / 2;
    let mut make = |llx, lly, urx, ury, id| {
        let free = maze.engine.rooms.new_complete_room(
            Some(TileShape::Box(IntBox::from_coords(llx, lly, urx, ury))),
            0,
            id,
        );
        let door = maze
            .engine
            .rooms
            .new_door(RoomRef::Complete(free), room_ref, 1);
        maze.engine.rooms.add_door(room_ref, door);
        door
    };
    let from_door = make(b.ll.x, b.ur.y, mid_x, b.ur.y + 6000, 8000 + corner_no);
    make(mid_x, b.ur.y, b.ur.x, b.ur.y + 6000, 8100 + corner_no);
    make(mid_x, b.ll.y - 6000, b.ur.x, b.ll.y, 8200 + corner_no);
    make(b.ll.x - 6000, mid_y, b.ll.x, b.ur.y, 8300 + corner_no);
    let _ = board;
    from_door
}

#[test]
fn the_shover_answers_javas_table_over_the_three_trace_segments() {
    let expected: [[bool; 4]; 3] = [
        [false, true, true, false],
        [true, true, true, true],
        [false, true, true, false],
    ];
    let boxes = [
        (-8600, -9600, 1600, -6400),
        (-1600, -9600, 1600, -2900),
        (-1600, -6100, 7600, -2900),
    ];
    for corner_no in 0..3usize {
        let mut board = shove_board();
        let mut engine = probe_engine(&mut board, 1);
        let ctrl = probe_control(&board, 1);
        let tree = engine.tree;
        let mut maze = MazeSearchEngine::new(&mut engine, &ctrl);
        let obstacle_room =
            maze.engine
                .rooms
                .new_obstacle_room(&mut board, ItemId(5), corner_no, tree);
        let b = maze
            .engine
            .rooms
            .room_shape(RoomRef::Obstacle(obstacle_room))
            .expect("a shape")
            .bounding_box();
        assert_eq!(
            (b.ll.x, b.ll.y, b.ur.x, b.ur.y),
            boxes[corner_no],
            "treeShape[{corner_no}]"
        );
        let from_door = shove_doors(
            &mut maze,
            &mut board,
            obstacle_room,
            i32::try_from(corner_no).expect("0..3"),
        );
        assert_eq!(
            maze.engine
                .rooms
                .door_section_segments(from_door, 1600.0)
                .len(),
            1,
            "doorSections=1"
        );
        let door_segment = maze
            .engine
            .rooms
            .door_shape(from_door)
            .expect("live")
            .diagonal_corner_segment()
            .expect("a non-empty door");

        let mut answers = Vec::new();
        for (left, at_a) in [(false, true), (false, false), (true, true), (true, false)] {
            let anchor = if at_a { door_segment.a } else { door_segment.b };
            let element = MazeListElement {
                door: ExpandableRef::Door(from_door),
                section_no_of_door: 0,
                backtrack_door: None,
                section_no_of_backtrack_door: 0,
                expansion_value: 0.0,
                sorting_value: 0.0,
                next_room: Some(RoomRef::Obstacle(obstacle_room)),
                shape_entry: FloatLine::new(anchor, anchor),
                room_ripped: false,
                adjustment: MazeAdjustment::None,
                already_checked: false,
                ripup_cost: 0,
            };
            let mut out: Vec<DoorSection> = Vec::new();
            let items_before = board.items.len();
            let ok = MazeTraceShover::check_shove_trace_line(
                &element,
                obstacle_room,
                &mut maze.engine.rooms,
                &mut board,
                &ctrl,
                left,
                &mut out,
            );
            assert!(out.is_empty(), "sections=0 at cornerNo={corner_no}");
            assert_eq!(items_before, board.items.len(), "the board is not written");
            answers.push(ok);
        }
        assert_eq!(
            answers,
            expected[corner_no].to_vec(),
            "cornerNo={corner_no}"
        );
    }
}

#[test]
fn a_two_dimensional_link_door_takes_javas_other_shove_branch() {
    let mut board = shove_board();
    let mut engine = probe_engine(&mut board, 1);
    let ctrl = probe_control(&board, 1);
    let tree = engine.tree;
    let mut maze = MazeSearchEngine::new(&mut engine, &ctrl);
    let seg1 = maze
        .engine
        .rooms
        .new_obstacle_room(&mut board, ItemId(5), 1, tree);
    let seg2 = maze
        .engine
        .rooms
        .new_obstacle_room(&mut board, ItemId(5), 2, tree);
    let link_door = maze
        .engine
        .rooms
        .new_door(RoomRef::Obstacle(seg1), RoomRef::Obstacle(seg2), 2);
    maze.engine
        .rooms
        .add_door(RoomRef::Obstacle(seg2), link_door);
    for (llx, lly, urx, ury, id) in [
        (3000, -2900, 7600, 3000, 8500),
        (3000, -9000, 7600, -6100, 8600),
    ] {
        let free = maze.engine.rooms.new_complete_room(
            Some(TileShape::Box(IntBox::from_coords(llx, lly, urx, ury))),
            0,
            id,
        );
        let door = maze
            .engine
            .rooms
            .new_door(RoomRef::Complete(free), RoomRef::Obstacle(seg2), 1);
        maze.engine.rooms.add_door(RoomRef::Obstacle(seg2), door);
    }
    assert_eq!(maze.engine.rooms.door_id_no(link_door), Some(161));
    let link_shape = maze.engine.rooms.door_shape(link_door).expect("live");
    let b = link_shape.bounding_box();
    assert_eq!(
        (b.ll.x, b.ll.y, b.ur.x, b.ur.y),
        (-1600, -6100, 1600, -2900)
    );
    assert!((link_shape.max_width() - 3_862.924_345_622_109).abs() < 1e-9);
    assert_eq!(
        maze.engine
            .rooms
            .door_section_segments(link_door, 1600.0)
            .len(),
        1,
        "sections=1"
    );
    let centre = link_shape.centre_of_gravity();

    for left in [false, true] {
        let element = MazeListElement {
            door: ExpandableRef::Door(link_door),
            section_no_of_door: 0,
            backtrack_door: None,
            section_no_of_backtrack_door: 0,
            expansion_value: 0.0,
            sorting_value: 0.0,
            next_room: Some(RoomRef::Obstacle(seg2)),
            shape_entry: FloatLine::new(centre, centre),
            room_ripped: false,
            adjustment: MazeAdjustment::None,
            already_checked: false,
            ripup_cost: 0,
        };
        let mut out: Vec<DoorSection> = Vec::new();
        assert!(MazeTraceShover::check_shove_trace_line(
            &element,
            seg2,
            &mut maze.engine.rooms,
            &mut board,
            &ctrl,
            left,
            &mut out,
        ));
        assert!(out.is_empty(), "sections=0");
        drain(&mut maze);
        assert!(maze.shove_trace_room(&mut board, &element, seg2));
        assert_eq!(maze.queue.len(), 0, "queue n=0");
    }
}

#[test]
fn a_stale_trace_index_is_refused_silently_by_the_shover() {
    let mut board = shove_board();
    let mut engine = probe_engine(&mut board, 1);
    let ctrl = probe_control(&board, 1);
    let tree = engine.tree;
    let maze = MazeSearchEngine::new(&mut engine, &ctrl);
    let stale_room = maze
        .engine
        .rooms
        .new_obstacle_room(&mut board, ItemId(5), 2, tree);
    let b = maze
        .engine
        .rooms
        .room_shape(RoomRef::Obstacle(stale_room))
        .expect("a shape")
        .bounding_box();
    let free = maze
        .engine
        .rooms
        .new_complete_room(Some(TileShape::Box(b.offset(1000.0))), 0, 8900);
    let stale_door =
        maze.engine
            .rooms
            .new_door(RoomRef::Complete(free), RoomRef::Obstacle(stale_room), 1);
    maze.engine
        .rooms
        .add_door(RoomRef::Obstacle(stale_room), stale_door);
    maze.engine.rooms.door_section_segments(stale_door, 1600.0);
    let anchor = maze
        .engine
        .rooms
        .door_shape(stale_door)
        .expect("live")
        .centre_of_gravity();
    let element = MazeListElement {
        door: ExpandableRef::Door(stale_door),
        section_no_of_door: 0,
        backtrack_door: None,
        section_no_of_backtrack_door: 0,
        expansion_value: 0.0,
        sorting_value: 0.0,
        next_room: Some(RoomRef::Obstacle(stale_room)),
        shape_entry: FloatLine::new(anchor, anchor),
        room_ripped: false,
        adjustment: MazeAdjustment::None,
        already_checked: false,
        ripup_cost: 0,
    };

    match board.items.get_mut(&ItemId(5)) {
        Some(Item::Trace(trace)) => trace.set_polyline(Polyline::from_points(&[
            Point::new(-7000, -8000),
            Point::new(0, -8000),
        ])),
        _ => panic!("item 5 is the obstacle trace"),
    }
    assert_eq!(
        board.items.get(&ItemId(5)).map(|item| match item {
            Item::Trace(trace) => trace.polyline().lines().len(),
            _ => 0,
        }),
        Some(3),
        "lines=3"
    );
    assert_eq!(
        maze.engine
            .rooms
            .obstacle_room(stale_room)
            .expect("live")
            .get_index_in_item(),
        2,
        "indexInItem=2"
    );

    let mut out: Vec<DoorSection> = Vec::new();
    assert!(
        !MazeTraceShover::check_shove_trace_line(
            &element,
            stale_room,
            &mut maze.engine.rooms,
            &mut board,
            &ctrl,
            false,
            &mut out,
        ),
        "checkShoveTraceLine=false"
    );
    assert!(out.is_empty(), "sections=0");
}

#[test]
fn the_neckdown_call_sites_narrow_the_half_width_for_the_whole_round() {
    type Expected = (bool, Vec<(i32, f64, f64)>);
    let cases: [(&str, bool, Expected); 4] = [
        ("targetDoor", false, (false, vec![])),
        ("targetDoor", true, (true, vec![(106, 1000.0, 1000.0)])),
        (
            "expansionDoor",
            false,
            (true, vec![(418, 1_007.153_180_628, 3_572.056_297_706)]),
        ),
        (
            "expansionDoor",
            true,
            (true, vec![(75, 1550.0, 2380.0), (106, 2550.0, 2550.0)]),
        ),
    ];
    for (site, neckdown, (expanded, rows)) in cases {
        let mut board = probe_board();
        let mut engine = probe_engine(&mut board, 1);
        let mut settings = probe_settings(&board);
        settings.set_automatic_neckdown(neckdown);
        let trace_costs = settings.get_trace_costs();
        let mut ctrl =
            AutorouteControl::new(&board, 1, &settings, settings.get_via_costs(), &trace_costs);
        ctrl.vias_allowed = false;
        assert_eq!(ctrl.with_neckdown, neckdown);
        let counter = Counter::new();
        let mut maze = MazeSearchEngine::get_instance(
            &set_of(&[2]),
            &set_of(&[3]),
            &mut engine,
            &mut board,
            &ctrl,
            &|| counter.check(),
        )
        .expect("init succeeds on this board");

        let seed = maze.queue.iter().last().expect("two seeded").clone();
        let room = seed.next_room.expect("a room");
        assert_eq!(maze.engine.rooms.room_id_no(room), Some(13));
        let room_min_width = maze
            .engine
            .rooms
            .room_shape(room)
            .expect("a shape")
            .min_width();
        assert!((room_min_width - 687.494_208_590_330_1).abs() < 1e-9);
        assert_eq!(ctrl.compensated_trace_half_width[0], 1600);
        assert_eq!(maze.check_neck_down_at_dest_pin(&board, room), 49.0);

        let element = if site == "targetDoor" {
            assert_eq!(maze.engine.expandable_id_no(seed.door), 75);
            seed
        } else {
            let door = maze.engine.rooms.room_doors(room)[0];
            assert_eq!(maze.engine.rooms.door_id_no(door), Some(230));
            assert_eq!(maze.engine.rooms.door(door).expect("live").dimension, 1);
            let centre = maze
                .engine
                .rooms
                .door_shape(door)
                .expect("live")
                .centre_of_gravity();
            MazeListElement {
                door: ExpandableRef::Door(door),
                section_no_of_door: 0,
                backtrack_door: None,
                section_no_of_backtrack_door: 0,
                expansion_value: 0.0,
                sorting_value: 0.0,
                next_room: Some(room),
                shape_entry: FloatLine::new(centre, centre),
                room_ripped: false,
                adjustment: MazeAdjustment::None,
                already_checked: false,
                ripup_cost: 0,
            }
        };

        drain(&mut maze);
        assert_eq!(
            maze.expand_to_room_doors(&mut board, &element),
            expanded,
            "site={site} withNeckdown={neckdown}"
        );
        let got: Vec<(i32, f64, f64)> = maze
            .queue
            .iter()
            .map(|e| {
                (
                    maze.engine.expandable_id_no(e.door),
                    r9(e.expansion_value),
                    r9(e.sorting_value),
                )
            })
            .collect();
        assert_eq!(got, rows, "site={site} withNeckdown={neckdown}");
    }
}
