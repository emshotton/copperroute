use std::cell::Cell;
use std::collections::BTreeSet;

use fr_board::ids::ItemId;
use fr_board::prelude::*;
use fr_board::rules::ViaRule;
use fr_geometry::{
    Area, FloatLine, FloatPoint, IntBox, IntOctagon, IntPoint, IntVector, Point, Polyline, Shape,
    TileShape,
};
use fr_router::autoroute::expansion::{ExpandableRef, RoomRef};
use fr_router::autoroute::item_info;
use fr_router::autoroute::maze::engine::AutorouteEngine;
use fr_router::autoroute::maze::search::{
    MazeSearchEngine, segment_projection, to_impacted_points,
};
use fr_router::autoroute::maze::{AutorouteControl, MazeListElement};
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

fn tie_pin_board() -> Board {
    let layers = || LayerStructure::new(vec![Layer::new("front", true), Layer::new("back", true)]);
    let clearance_matrix = ClearanceMatrix::get_default_instance(&layers(), 200);
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
    let pad = Shape::Tile(TileShape::Box(IntBox::from_coords(-200, -200, 200, 200)));
    let through = padstacks.add("thru", vec![Some(pad.clone()), Some(pad)], true, false);
    let mut packages = Packages::new();
    let package = packages.add(
        "pkg",
        vec![PackagePin::new(
            "P1",
            through,
            IntVector::new(0, 0).into(),
            0.0,
        )],
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
    board.insert_pin(1, 0, vec![1, 2], 1, FixedState::Unfixed);
    board.insert_trace_without_cleaning(
        Polyline::from_points(&[Point::new(0, 0), Point::new(0, 2000)]),
        0,
        60,
        vec![2],
        1,
        FixedState::Unfixed,
    );
    board.insert_trace_without_cleaning(
        Polyline::from_points(&[Point::new(0, 0), Point::new(2000, 0)]),
        0,
        60,
        vec![1],
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
    trip: u32,
    calls: Cell<u32>,
}

impl Counter {
    fn new(trip: u32) -> Counter {
        Counter {
            trip,
            calls: Cell::new(0),
        }
    }

    fn check(&self) -> bool {
        self.calls.set(self.calls.get() + 1);
        self.trip > 0 && self.calls.get() >= self.trip
    }

    fn calls(&self) -> u32 {
        self.calls.get()
    }
}

fn set_of(ids: &[u32]) -> BTreeSet<ItemId> {
    ids.iter().map(|id| ItemId(*id)).collect()
}

type RoomRow = (i32, usize, (i32, i32, i32, i32), usize);

fn complete_rooms(maze: &MazeSearchEngine<'_>) -> Vec<RoomRow> {
    maze.engine
        .complete_expansion_rooms()
        .iter()
        .map(|id| {
            let room = maze.engine.rooms.complete_room(*id).expect("live room");
            let b = room.get_shape().expect("a shape").bounding_box();
            (
                room.get_id(),
                room.get_layer(),
                (b.ll.x, b.ll.y, b.ur.x, b.ur.y),
                room.get_target_doors().len(),
            )
        })
        .collect()
}

type QueueRow = (i32, i32, f64, f64, Option<i32>, (f64, f64));

fn queue_rows(maze: &MazeSearchEngine<'_>) -> Vec<QueueRow> {
    maze.queue
        .iter()
        .map(|e| row_of(maze, e))
        .collect::<Vec<_>>()
}

fn row_of(maze: &MazeSearchEngine<'_>, e: &MazeListElement) -> QueueRow {
    (
        maze.engine.expandable_id_no(e.door),
        e.section_no_of_door,
        e.expansion_value,
        e.sorting_value,
        e.next_room
            .and_then(|room| maze.engine.rooms.room_id_no(room)),
        (e.shape_entry.a.x, e.shape_entry.a.y),
    )
}


#[test]
fn the_boards_item_ids_are_the_probes() {
    let mut board = probe_board();
    let tree = board.default_tree_id();
    let ids: Vec<u32> = board
        .items_in_board_order()
        .into_iter()
        .map(|id| id.0)
        .collect();
    assert_eq!(ids, vec![4, 3, 2, 1], "getItems() is descending id");
    assert_eq!(board.item_tree_shape_count(ItemId(3), tree), 2);
    assert_eq!(board.item_tree_shape_count(ItemId(2), tree), 1);
}


#[test]
fn init_seeds_one_element_per_non_destination_target_door() {
    let mut board = probe_board();
    let mut engine = probe_engine(&mut board, 1);
    let ctrl = probe_control(&board, 1);
    let counter = Counter::new(0);
    let mut maze = MazeSearchEngine::new(&mut engine, &ctrl);
    assert!(maze.init(&mut board, &set_of(&[2]), &set_of(&[3]), &|| {
        counter.check()
    }));
    assert_eq!(counter.calls(), 7);

    assert!(item_info::is_start_info(&mut board, ItemId(2)));
    assert!(!item_info::is_start_info(&mut board, ItemId(3)));
    assert!(!item_info::is_start_info(&mut board, ItemId(4)));

    assert_eq!(
        complete_rooms(&maze),
        vec![
            (6, 0, (-10_000, -10_000, -4700, 0), 0),
            (7, 0, (-4700, -10_000, 600, 0), 2),
            (13, 0, (-4700, 0, 600, 1041), 2),
        ]
    );

    assert_eq!(
        queue_rows(&maze),
        vec![
            (69, 0, 0.0, 830.0, Some(7), (-500.0, 0.0)),
            (75, 0, 0.0, 830.0, Some(13), (-500.0, 0.0)),
        ]
    );
    assert_eq!(maze.destination_door(), None);
}

#[test]
fn init_walks_the_start_set_in_javas_descending_item_order() {
    let mut board = probe_board();
    let mut engine = probe_engine(&mut board, 1);
    let ctrl = probe_control(&board, 1);
    let counter = Counter::new(0);
    let mut maze = MazeSearchEngine::new(&mut engine, &ctrl);
    assert!(maze.init(&mut board, &set_of(&[2, 3]), &set_of(&[3]), &|| {
        counter.check()
    }));
    assert_eq!(counter.calls(), 11);

    assert_eq!(
        complete_rooms(&maze),
        vec![
            (8, 0, (-10_000, -10_000, -4700, 0), 0),
            (9, 0, (-4700, -10_000, 600, 0), 2),
            (15, 0, (-4700, 0, 600, 1041), 2),
            (17, 1, (-10_000, -10_000, 0, 0), 0),
            (25, 1, (0, 0, 10_000, 10_000), 1),
        ]
    );
    assert_eq!(
        queue_rows(&maze),
        vec![
            (102, 0, 0.0, 0.0, Some(9), (500.0, 0.0)),
            (108, 0, 0.0, 0.0, Some(15), (500.0, 0.0)),
            (118, 0, 0.0, 0.0, Some(25), (500.0, 0.0)),
            (71, 0, 0.0, 830.0, Some(9), (-500.0, 0.0)),
            (77, 0, 0.0, 830.0, Some(15), (-500.0, 0.0)),
        ]
    );
}

#[test]
fn init_creates_the_start_rooms_in_javas_descending_item_order() {
    let mut board = probe_board();
    let mut engine = probe_engine(&mut board, 1);
    let ctrl = probe_control(&board, 1);
    let counter = Counter::new(4);
    let mut maze = MazeSearchEngine::new(&mut engine, &ctrl);
    assert!(
        !maze.init(&mut board, &set_of(&[2, 3]), &set_of(&[3]), &|| counter
            .check())
    );
    assert_eq!(counter.calls(), 4);

    let rooms: Vec<(usize, (i32, i32, i32, i32))> = maze
        .engine
        .rooms
        .incomplete_rooms
        .iter()
        .map(|(_, room)| {
            let b = room
                .get_contained_shape()
                .expect("the start room's contained shape")
                .bounding_box();
            (room.get_layer(), (b.ll.x, b.ll.y, b.ur.x, b.ur.y))
        })
        .collect();
    assert_eq!(
        rooms,
        vec![
            (0, (500, 0, 500, 0)),
            (1, (500, 0, 500, 0)),
            (0, (-500, 0, -500, 0)),
        ]
    );
}

#[test]
fn init_returns_none_when_no_start_door_exists() {
    let mut board = probe_board();
    let mut engine = probe_engine(&mut board, 1);
    let ctrl = probe_control(&board, 1);
    let counter = Counter::new(0);
    assert!(
        MazeSearchEngine::get_instance(
            &BTreeSet::new(),
            &set_of(&[3]),
            &mut engine,
            &mut board,
            &ctrl,
            &|| counter.check(),
        )
        .is_none()
    );
    assert_eq!(counter.calls(), 1);

    let mut board = probe_board();
    let mut engine = probe_engine(&mut board, 1);
    let ctrl = probe_control(&board, 1);
    let counter = Counter::new(0);
    assert!(
        MazeSearchEngine::get_instance(
            &set_of(&[2]),
            &BTreeSet::new(),
            &mut engine,
            &mut board,
            &ctrl,
            &|| counter.check(),
        )
        .is_none()
    );
    assert_eq!(counter.calls(), 0);
}


fn init_with_trip(trip: u32) -> (bool, u32, bool, usize, usize) {
    let mut board = probe_board();
    let mut engine = probe_engine(&mut board, 1);
    let ctrl = probe_control(&board, 1);
    let counter = Counter::new(trip);
    let mut maze = MazeSearchEngine::new(&mut engine, &ctrl);
    let ok = maze.init(&mut board, &set_of(&[2]), &set_of(&[3]), &|| {
        counter.check()
    });
    let rooms = maze.engine.complete_expansion_rooms().len();
    let queued = maze.queue.len();
    let start_info = item_info::is_start_info(&mut board, ItemId(2));
    (ok, counter.calls(), start_info, rooms, queued)
}

#[test]
fn each_of_the_four_init_stop_sites_aborts_where_java_does() {
    assert_eq!(init_with_trip(1), (false, 1, false, 0, 0));
    assert_eq!(init_with_trip(2), (false, 2, false, 0, 0));
    assert_eq!(init_with_trip(3), (false, 3, true, 0, 0));
    assert_eq!(init_with_trip(4), (false, 4, true, 3, 0));
    assert_eq!(init_with_trip(5), (false, 5, true, 3, 1));
    assert_eq!(init_with_trip(6), (false, 6, true, 3, 1));
    assert_eq!(init_with_trip(7), (false, 7, true, 3, 2));
    assert_eq!(init_with_trip(8), (true, 7, true, 3, 2));
}

#[test]
fn the_pop_loops_stop_check_aborts_before_the_queue_is_touched() {
    let mut board = probe_board();
    let mut engine = probe_engine(&mut board, 1);
    let ctrl = probe_control(&board, 1);
    let never = Counter::new(0);
    let mut maze = MazeSearchEngine::new(&mut engine, &ctrl);
    assert!(maze.init(&mut board, &set_of(&[2]), &set_of(&[3]), &|| never.check()));
    assert_eq!(maze.queue.len(), 2);

    let always = Counter::new(1);
    assert!(!maze.occupy_next_element(&mut board, &|| always.check()));
    assert_eq!(always.calls(), 1);
    assert_eq!(maze.queue.len(), 2, "nothing was popped");
    assert_eq!(maze.destination_door(), None);
}


fn destination_door_of_room(
    maze: &MazeSearchEngine<'_>,
    board: &mut Board,
    room: RoomRef,
) -> ExpandableRef {
    let doors = maze.engine.rooms.room_target_doors(room).to_vec();
    for door in doors {
        let is_dest = maze
            .engine
            .rooms
            .target_door(door)
            .expect("live target door")
            .is_destination_door(board);
        if is_dest {
            return ExpandableRef::TargetDoor(door);
        }
    }
    panic!("the room has no destination door");
}

#[test]
fn the_queue_pops_the_lowest_sorting_value_and_removes_it() {
    let mut board = probe_board();
    let mut engine = probe_engine(&mut board, 1);
    let ctrl = probe_control(&board, 1);
    let never = Counter::new(0);
    let mut maze = MazeSearchEngine::new(&mut engine, &ctrl);
    assert!(maze.init(&mut board, &set_of(&[2]), &set_of(&[3]), &|| never.check()));

    let room = maze
        .queue
        .iter()
        .next()
        .expect("a seeded element")
        .next_room;
    let room = room.expect("the seeded element has a next room");
    let dest_door = destination_door_of_room(&maze, &mut board, room);
    assert_eq!(maze.engine.expandable_id_no(dest_door), 100);

    let centre = maze
        .engine
        .rooms
        .target_door(match dest_door {
            ExpandableRef::TargetDoor(d) => d,
            _ => unreachable!(),
        })
        .expect("live")
        .get_shape()
        .centre_of_gravity();
    maze.push(
        MazeListElement {
            door: dest_door,
            section_no_of_door: 0,
            backtrack_door: None,
            section_no_of_backtrack_door: 0,
            expansion_value: 0.0,
            sorting_value: -1.0,
            next_room: Some(room),
            shape_entry: FloatLine::new(centre, centre),
            room_ripped: false,
            adjustment: fr_router::autoroute::maze::MazeAdjustment::None,
            already_checked: false,
            ripup_cost: 0,
        },
        &board,
    );
    assert_eq!(maze.queue.len(), 3);

    assert!(!maze.occupy_next_element(&mut board, &|| never.check()));
    assert_eq!(maze.queue.len(), 2);
    assert_eq!(maze.destination_door(), Some(dest_door));
    assert_eq!(maze.section_no_of_destination_door(), 0);
    let occupied = match dest_door {
        ExpandableRef::TargetDoor(d) => {
            maze.engine
                .rooms
                .target_door(d)
                .expect("live")
                .get_maze_search_element(0)
                .is_occupied
        }
        _ => unreachable!(),
    };
    assert!(!occupied);

    let result = maze
        .find_connection(&mut board, &|| never.check())
        .expect("a result");
    assert_eq!(result.destination_door, dest_door);
    assert_eq!(result.section_no_of_door, 0);
}

#[test]
fn an_already_occupied_section_is_skipped_without_expanding() {
    let mut board = probe_board();
    let mut engine = probe_engine(&mut board, 1);
    let ctrl = probe_control(&board, 1);
    let never = Counter::new(0);
    let mut maze = MazeSearchEngine::new(&mut engine, &ctrl);
    assert!(maze.init(&mut board, &set_of(&[2]), &set_of(&[3]), &|| never.check()));

    let seeded: Vec<(ExpandableRef, i32, Option<RoomRef>)> = maze
        .queue
        .iter()
        .map(|e| (e.door, e.section_no_of_door, e.next_room))
        .collect();
    for (door, section, _) in &seeded {
        maze.engine
            .maze_search_element_mut(*door, *section)
            .expect("a live door section")
            .is_occupied = true;
    }

    let room = seeded[0].2.expect("a next room");
    let dest_door = destination_door_of_room(&maze, &mut board, room);
    let centre = maze
        .engine
        .rooms
        .target_door(match dest_door {
            ExpandableRef::TargetDoor(d) => d,
            _ => unreachable!(),
        })
        .expect("live")
        .get_shape()
        .centre_of_gravity();
    maze.push(
        MazeListElement {
            door: dest_door,
            section_no_of_door: 0,
            backtrack_door: None,
            section_no_of_backtrack_door: 0,
            expansion_value: 0.0,
            sorting_value: 1.0e9,
            next_room: Some(room),
            shape_entry: FloatLine::new(centre, centre),
            room_ripped: false,
            adjustment: fr_router::autoroute::maze::MazeAdjustment::None,
            already_checked: false,
            ripup_cost: 0,
        },
        &board,
    );
    assert_eq!(maze.queue.len(), 3);

    assert!(!maze.occupy_next_element(&mut board, &|| never.check()));
    assert_eq!(maze.queue.len(), 0, "all three were popped");
    assert_eq!(maze.destination_door(), Some(dest_door));
    for (door, section, _) in &seeded {
        assert_eq!(
            maze.engine
                .maze_search_element(*door, *section)
                .expect("a live door section")
                .backtrack_door,
            None
        );
    }
}

#[test]
fn find_connection_answers_none_when_the_queue_is_empty() {
    let mut board = probe_board();
    let mut engine = probe_engine(&mut board, 1);
    let ctrl = probe_control(&board, 1);
    let never = Counter::new(0);
    let mut maze = MazeSearchEngine::new(&mut engine, &ctrl);
    assert!(
        maze.find_connection(&mut board, &|| never.check())
            .is_none()
    );
}


#[test]
fn an_empty_queue_makes_get_instance_answer_none() {
    let mut board = probe_board();
    let mut engine = probe_engine(&mut board, 1);
    let mut ctrl = probe_control(&board, 1);
    ctrl.is_fanout = true;
    ctrl.fanout_start_pin_center = Some(Point::new(9000, 9000));
    ctrl.fanout_start_pin_layer = 0;
    assert_eq!(ctrl.fanout_max_escape_length, 3000.0);

    let never = Counter::new(0);
    let maze = MazeSearchEngine::get_instance(
        &set_of(&[2]),
        &set_of(&[3]),
        &mut engine,
        &mut board,
        &ctrl,
        &|| never.check(),
    );
    assert!(
        maze.is_none(),
        "every seeded element was refused by the escape window, so initialisation failed - \
         the jar answers an engine here whose queue is empty (`instance=ok`, `queue n=0`)"
    );
}


#[test]
fn door_is_small_matches_the_three_angle_restrictions() {
    const GEOMS: [(i32, i32, i32, i32); 3] = [(0, 0, 100, 40), (0, 0, 40, 100), (0, 0, 30, 30)];
    const WIDTHS: [f64; 7] = [10.0, 42.0, 45.0, 101.0, 105.0, 108.0, 200.0];
    const EXPECTED: [[bool; 7]; 9] = [
        [false, false, false, true, true, true, true],
        [false, false, false, true, true, true, true],
        [false, true, true, true, true, true, true],
        [false, false, false, true, true, true, true],
        [false, false, false, true, true, true, true],
        [false, false, true, true, true, true, true],
        [false, false, false, false, false, true, true],
        [false, false, false, false, false, true, true],
        [false, false, true, true, true, true, true],
    ];

    let mut row = 0;
    for restriction in [
        AngleRestriction::NinetyDegree,
        AngleRestriction::FortyFiveDegree,
        AngleRestriction::None,
    ] {
        for geom in GEOMS {
            let mut board = probe_board();
            board.rules.trace_angle_restriction = restriction;
            let mut engine = probe_engine(&mut board, 1);
            let ctrl = probe_control(&board, 1);
            let first = engine.rooms.new_complete_room(
                Some(TileShape::Box(IntBox::from_coords(
                    geom.0 - 500,
                    geom.1 - 500,
                    geom.2,
                    geom.3,
                ))),
                0,
                1,
            );
            let second = engine.rooms.new_complete_room(
                Some(TileShape::Box(IntBox::from_coords(
                    geom.0,
                    geom.1,
                    geom.2 + 500,
                    geom.3 + 500,
                ))),
                0,
                2,
            );
            let door =
                engine
                    .rooms
                    .new_door(RoomRef::Complete(first), RoomRef::Complete(second), 2);
            let maze = MazeSearchEngine::new(&mut engine, &ctrl);
            for (col, width) in WIDTHS.iter().enumerate() {
                assert_eq!(
                    maze.door_is_small(&board, door, *width),
                    EXPECTED[row][col],
                    "restriction={restriction:?} geom={geom:?} width={width}"
                );
            }
            row += 1;
        }
    }
}

#[test]
fn door_is_small_answers_false_for_a_two_dimensional_door_onto_an_obstacle_room() {
    for (dimension, expected) in [(1, true), (2, false)] {
        let mut board = probe_board();
        let mut engine = probe_engine(&mut board, 1);
        let ctrl = probe_control(&board, 1);
        let tree = engine.tree;
        let free = engine.rooms.new_complete_room(
            Some(TileShape::Box(IntBox::from_coords(0, 0, 1000, 1000))),
            0,
            7,
        );
        let obstacle = engine
            .rooms
            .new_obstacle_room(&mut board, ItemId(4), 0, tree);
        let door = engine.rooms.new_door(
            RoomRef::Complete(free),
            RoomRef::Obstacle(obstacle),
            dimension,
        );
        let maze = MazeSearchEngine::new(&mut engine, &ctrl);
        assert_eq!(maze.door_is_small(&board, door, 1.0e9), expected);
    }
}


#[test]
fn segment_projection_matches_the_jvm() {
    let line = |ax: f64, ay: f64, bx: f64, by: f64| {
        FloatLine::new(FloatPoint::new(ax, ay), FloatPoint::new(bx, by))
    };
    let cases: [(FloatLine, FloatLine, Option<FloatLine>); 6] = [
        (
            line(0.0, 0.0, 100.0, 0.0),
            line(0.0, 10.0, 100.0, 10.0),
            Some(line(0.0, 10.0, 100.0, 10.0)),
        ),
        (
            line(0.0, 0.0, 100.0, 0.0),
            line(50.0, 10.0, 200.0, 10.0),
            Some(line(50.0, 10.0, 100.0, 10.0)),
        ),
        (
            line(0.0, 0.0, 100.0, 0.0),
            line(200.0, 10.0, 300.0, 10.0),
            None,
        ),
        (
            line(100.0, 0.0, 0.0, 0.0),
            line(0.0, 10.0, 100.0, 10.0),
            Some(line(0.0, 10.0, 100.0, 10.0)),
        ),
        (
            line(20.0, 0.0, 80.0, 0.0),
            line(0.0, 10.0, 100.0, 10.0),
            Some(line(20.0, 10.0, 80.0, 10.0)),
        ),
        (
            line(0.0, 0.0, 100.0, 100.0),
            line(0.0, 10.0, 100.0, 110.0),
            Some(line(0.0, 10.0, 95.0, 105.0)),
        ),
    ];
    for (from, to, expected) in cases {
        let got = segment_projection(&from, &to);
        match (&got, &expected) {
            (None, None) => {}
            (Some(g), Some(e)) => {
                assert!(
                    (g.a.x - e.a.x).abs() < 1e-9
                        && (g.a.y - e.a.y).abs() < 1e-9
                        && (g.b.x - e.b.x).abs() < 1e-9
                        && (g.b.y - e.b.y).abs() < 1e-9,
                    "from={from:?} to={to:?} got={got:?} expected={expected:?}"
                );
            }
            _ => panic!("from={from:?} to={to:?} got={got:?} expected={expected:?}"),
        }
    }
}

#[test]
fn to_impacted_points_rounds_both_ends_and_passes_null_through() {
    assert_eq!(to_impacted_points(None), None);
    let line = FloatLine::new(
        FloatPoint::new(1.4, -1.4),
        FloatPoint::new(2.5, f64::from(-2.5f32)),
    );
    assert_eq!(
        to_impacted_points(Some(&line)),
        Some([Point::new(1, -1), Point::new(3, -2)])
    );
}


#[test]
fn reduce_trace_shapes_at_tie_pins_cuts_only_the_foreign_net_contact() {
    let mut board = tie_pin_board();
    let mut engine = probe_engine(&mut board, 1);
    let tree = engine.tree;

    let shape_of = |board: &mut Board, id: u32| {
        let b = board
            .item_tree_shape(ItemId(id), tree, 0)
            .expect("a tree shape")
            .bounding_box();
        (b.ll.x, b.ll.y, b.ur.x, b.ur.y)
    };
    assert_eq!(shape_of(&mut board, 4), (-160, -160, 2160, 160));
    assert_eq!(shape_of(&mut board, 3), (-160, -160, 160, 2160));

    MazeSearchEngine::reduce_trace_shapes_at_tie_pins(&mut board, &set_of(&[2]), 1, tree);

    assert_eq!(shape_of(&mut board, 4), (-160, -160, 2160, 160));
    assert_eq!(shape_of(&mut board, 3), (-160, 300, 160, 2160));
    let _ = &mut engine;
}
