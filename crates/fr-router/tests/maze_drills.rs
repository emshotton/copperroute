#![allow(clippy::too_many_lines)]

use std::cell::Cell;
use std::collections::BTreeSet;

use fr_board::ids::{ItemId, PadstackId, ViaInfoId};
use fr_board::prelude::*;
use fr_board::rules::{ViaInfo, ViaRule};
use fr_geometry::{
    FloatLine, FloatPoint, IntBox, IntOctagon, IntPoint, IntVector, Point, Polyline, Shape,
    TileShape,
};
use fr_router::arena::PageId;
use fr_router::autoroute::drill::ExpansionDrill;
use fr_router::autoroute::expansion::{ExpandableRef, RoomRef};
use fr_router::autoroute::maze::engine::AutorouteEngine;
use fr_router::autoroute::maze::expansion_engine::MazeExpansionEngine;
use fr_router::autoroute::maze::search::MazeSearchEngine;
use fr_router::autoroute::maze::{AutorouteControl, MazeAdjustment, MazeListElement, ViaMask};
use fr_router::board_ext::CheckDrillResult;
use fr_settings::RouterSettings;

const BOUNDING_BOX: IntBox = IntBox {
    ll: IntPoint {
        x: -4_000,
        y: -4_000,
    },
    ur: IntPoint { x: 4_000, y: 4_000 },
};

const SIMPLE_BOUNDING_BOX: IntBox = IntBox {
    ll: IntPoint {
        x: -1_000,
        y: -1_000,
    },
    ur: IntPoint { x: 1_000, y: 1_000 },
};

fn base_board(bounds: IntBox) -> Board {
    let layers = || LayerStructure::new(vec![Layer::new("front", true), Layer::new("back", true)]);
    let clearance_matrix = ClearanceMatrix::get_default_instance(&layers(), 200);
    let mut rules = BoardRules::new(layers(), clearance_matrix);
    rules.trace_angle_restriction = AngleRestriction::None;
    rules.set_default_trace_half_widths(30);

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
    let via_shape = Shape::Tile(TileShape::Octagon(IntOctagon::new(
        -100, -100, 100, 100, -200, 200, -200, 200,
    )));
    let via = padstacks.add(
        "via",
        vec![Some(via_shape.clone()), Some(via_shape)],
        true,
        false,
    );
    assert_eq!(
        (smd, through, via),
        (PadstackId(1), PadstackId(2), PadstackId(3)),
        "the port's padstack ids start at 1"
    );

    rules.via_infos.add(ViaInfo::new("v", via, 1, false));
    let mut via_rule = ViaRule::new("rule");
    via_rule.append_via(rules.via_infos.get(ViaInfoId(0)).clone());
    rules.via_rules.push(via_rule);
    let default_class = rules.get_default_net_class();
    rules
        .net_classes
        .get_mut(default_class)
        .set_via_rule(Some(rules.via_rules[0].clone()));

    Board::new(
        Vec::new(),
        0,
        bounds,
        rules,
        BoardLibrary::new(padstacks, Packages::new()),
        Components::new(),
        Communication::default(),
    )
}

fn add_package(board: &mut Board, name: &str, pins: Vec<PackagePin>) -> usize {
    board.library.packages.add(
        name,
        pins,
        None,
        None,
        None,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        true,
    )
}

fn probe_board() -> Board {
    let mut board = base_board(BOUNDING_BOX);
    let default_class = board.rules.get_default_net_class();
    board.rules.nets.add("N1", 1, false, default_class);
    board.rules.nets.add("N2", 1, false, default_class);
    board.rules.nets.add("N3", 1, false, default_class);

    let pkg1 = add_package(
        &mut board,
        "pkg1",
        vec![
            PackagePin::new("P1", PadstackId(1), IntVector::new(-2000, 0).into(), 0.0),
            PackagePin::new("P2", PadstackId(2), IntVector::new(2000, 0).into(), 0.0),
        ],
    );
    let pkg2 = add_package(
        &mut board,
        "pkg2",
        vec![
            PackagePin::new("P3", PadstackId(1), IntVector::new(0, -2000).into(), 0.0),
            PackagePin::new("P4", PadstackId(1), IntVector::new(0, 2000).into(), 0.0),
        ],
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

fn simple_board() -> Board {
    let mut board = base_board(SIMPLE_BOUNDING_BOX);
    let default_class = board.rules.get_default_net_class();
    board.rules.nets.add("N1", 1, false, default_class);
    let pkg = add_package(
        &mut board,
        "pkg",
        vec![
            PackagePin::new("P1", PadstackId(1), IntVector::new(-400, 0).into(), 0.0),
            PackagePin::new("P2", PadstackId(2), IntVector::new(400, 0).into(), 0.0),
        ],
    );
    board
        .components
        .add_with_generated_name(Some(Point::new(0, 0)), 0.0, true, pkg);
    board.insert_pin(1, 0, vec![1], 1, FixedState::Unfixed);
    board.insert_pin(1, 1, vec![1], 1, FixedState::Unfixed);
    board
}

fn simple_board_ninety() -> Board {
    let mut board = simple_board();
    board.rules.trace_angle_restriction = AngleRestriction::NinetyDegree;
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

fn r9(x: f64) -> f64 {
    (x * 1e9).round() / 1e9
}

fn r6(p: &FloatPoint) -> (f64, f64) {
    ((p.x * 1e6).round() / 1e6, (p.y * 1e6).round() / 1e6)
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

fn drain(maze: &mut MazeSearchEngine<'_>) {
    while maze.queue.pop_first().is_some() {}
}

#[test]
fn the_control_carries_a_real_via_rule_and_the_start_ripup_costs() {
    let board = probe_board();
    let ctrl = probe_control(&board, 1);
    assert_eq!(ctrl.trace_half_width, vec![30, 30]);
    assert_eq!(ctrl.compensated_trace_half_width, vec![130, 130]);
    assert!((ctrl.via_radii[0] - 141.421_356_237_309_48).abs() < 1e-9);
    assert!((ctrl.min_normal_via_cost - 141.421_356_237_309_48).abs() < 1e-9);
    assert!((ctrl.min_cheap_via_cost - 113.137_084_989_847_6).abs() < 1e-9);
    assert_eq!((ctrl.via_lower_bound, ctrl.via_upper_bound), (0, 2));
    assert_eq!(ctrl.via_clearance_class, 1);
    assert_eq!(ctrl.via_infos.len(), 1);
    assert_eq!(ctrl.via_infos[0].from_layer, 0);
    assert_eq!(ctrl.via_infos[0].to_layer, 1);
    assert!(!ctrl.via_infos[0].attach_smd_allowed);
    assert!(!ctrl.attach_smd_allowed());
    assert!(ctrl.vias_allowed);
    assert!(!ctrl.ripup_allowed);
    assert_eq!(ctrl.ripup_costs, 1000);
    assert_eq!(ctrl.ripup_pass_no, 1);
    assert!(ctrl.remove_unconnected_vias);
    assert_eq!(ctrl.start_ripup_costs, 1);
    assert_eq!(ctrl.add_via_costs, vec![vec![0, 0], vec![0, 0]]);
}

#[test]
fn a_drill_page_element_costs_one_normal_via_and_keeps_the_room() {
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
    .expect("init succeeds");

    let from = maze.queue.iter().next().expect("a seeded element").clone();
    assert_eq!(maze.engine.expandable_id_no(from.door), 65);
    let room_shape = maze
        .engine
        .rooms
        .room_shape(from.next_room.expect("a next room"))
        .expect("a shape")
        .clone();
    let pages: Vec<PageId> = maze.engine.drill_pages().overlapping_pages(&room_shape);
    assert_eq!(pages.len(), 1);
    assert_eq!(
        maze.engine.expandable_id_no(ExpandableRef::Page(pages[0])),
        1
    );

    drain(&mut maze);
    MazeExpansionEngine::expand_to_drill_page(&mut maze, &mut board, pages[0], &from);
    assert_eq!(
        queue_rows(&maze),
        vec![(
            1,
            0,
            141.421_356_237,
            3_971.421_356_237,
            Some(3),
            (-2_000.0, 0.0),
            (-2_000.0, 0.0),
            false,
            MazeAdjustment::None,
            false,
            0,
        )]
    );
}

#[test]
fn a_shape_ending_on_a_page_boundary_reaches_the_next_page() {
    let mut board = probe_board();
    board.bounding_box = IntBox::from_coords(-12_000, -12_000, 12_000, 12_000);
    let engine = probe_engine(&mut board, 1);
    let pages = engine.drill_pages();
    assert!(pages.column_count() > 1);
    let bounds = pages.bounds();
    let boundary_x = bounds.ll.x + pages.page_width();
    let shape = TileShape::Box(IntBox::from_coords(
        bounds.ll.x + 1,
        bounds.ll.y + 1,
        boundary_x,
        bounds.ll.y + pages.page_height() - 1,
    ));

    assert_eq!(
        pages.overlapping_pages(&shape),
        [pages.page_id(0, 0), pages.page_id(1, 0)]
    );
}

struct DrillFixture {
    board: Board,
    engine: AutorouteEngine,
    ctrl: AutorouteControl,
}

fn drill_fixture() -> DrillFixture {
    let mut board = probe_board();
    let engine = probe_engine(&mut board, 1);
    let ctrl = probe_control(&board, 1);
    DrillFixture {
        board,
        engine,
        ctrl,
    }
}

#[test]
fn every_drill_of_the_page_whose_room_matches_is_expanded_once() {
    let mut fixture = drill_fixture();
    let counter = Counter::new();
    let mut maze = MazeSearchEngine::get_instance(
        &set_of(&[2]),
        &set_of(&[3]),
        &mut fixture.engine,
        &mut fixture.board,
        &fixture.ctrl,
        &|| counter.check(),
    )
    .expect("init succeeds");
    let from = maze.queue.iter().next().expect("a seeded element").clone();
    let room_shape = maze
        .engine
        .rooms
        .room_shape(from.next_room.expect("a next room"))
        .expect("a shape")
        .clone();
    let page = maze.engine.drill_pages().overlapping_pages(&room_shape)[0];
    drain(&mut maze);
    MazeExpansionEngine::expand_to_drill_page(&mut maze, &mut fixture.board, page, &from);
    let page_element = maze.queue.iter().next().expect("the page element").clone();

    let drills = maze.engine.drill_page_drills(
        &mut fixture.board,
        page,
        fixture.ctrl.attach_smd_allowed(),
        &|| counter.check(),
    );
    assert_eq!(drills.len(), 53);

    drain(&mut maze);
    MazeExpansionEngine::expand_to_drills_of_page(
        &mut maze,
        &mut fixture.board,
        &page_element,
        &|| counter.check(),
    );
    assert_eq!(
        queue_rows(&maze),
        vec![
            (
                -80_452_036,
                0,
                433.228_227_301,
                4_403.114_875_557,
                None,
                (-2_130.5, -310.5),
                (-2_130.5, -310.5),
                false,
                MazeAdjustment::None,
                false,
                0,
            ),
            (
                -63_903_616,
                0,
                4_033.421_356_237,
                9_508.149_112_026,
                None,
                (-2_000.0, -2_126.0),
                (-2_000.0, -2_126.0),
                false,
                MazeAdjustment::None,
                false,
                0,
            ),
        ]
    );
}

#[test]
fn a_drill_from_a_page_door_skips_the_via_cost_and_uses_the_pins_trace_exit_corner() {
    let mut fixture = drill_fixture();
    let counter = Counter::new();
    let mut maze = MazeSearchEngine::get_instance(
        &set_of(&[2]),
        &set_of(&[3]),
        &mut fixture.engine,
        &mut fixture.board,
        &fixture.ctrl,
        &|| counter.check(),
    )
    .expect("init succeeds");
    let from = maze.queue.iter().next().expect("a seeded element").clone();
    let room = from.next_room.expect("a next room");
    let room_shape = maze.engine.rooms.room_shape(room).expect("a shape").clone();
    assert!((room_shape.min_width() - 2_134.0).abs() < 1e-9);
    let page = maze.engine.drill_pages().overlapping_pages(&room_shape)[0];
    drain(&mut maze);
    MazeExpansionEngine::expand_to_drill_page(&mut maze, &mut fixture.board, page, &from);
    let page_element = maze.queue.iter().next().expect("the page element").clone();
    let drills = maze.engine.drill_page_drills(
        &mut fixture.board,
        page,
        fixture.ctrl.attach_smd_allowed(),
        &|| counter.check(),
    );
    let first = drills[0];

    for (add_costs, from_page, from_door) in [
        (0, (8_040.526_115_639, 11_555.947_471_876)),
        (250, (8_290.526_115_639, 11_805.947_471_876)),
    ]
    .into_iter()
    .zip([
        (8_137.326_308_149, 11_652.747_664_386),
        (8_387.326_308_149, 11_902.747_664_386),
    ])
    .map(|((c, p), d)| (c, p, d))
    {
        drain(&mut maze);
        MazeExpansionEngine::expand_to_drill(
            &mut maze,
            &mut fixture.board,
            first,
            &page_element,
            add_costs,
        );
        assert_eq!(
            queue_rows(&maze),
            vec![(
                67_236_366,
                0,
                from_page.0,
                from_page.1,
                None,
                (2_364.0, -3_350.0),
                (2_364.0, -3_350.0),
                false,
                MazeAdjustment::None,
                false,
                0,
            )],
            "fromPage addCosts={add_costs}"
        );
        drain(&mut maze);
        MazeExpansionEngine::expand_to_drill(
            &mut maze,
            &mut fixture.board,
            first,
            &from,
            add_costs,
        );
        assert_eq!(
            queue_rows(&maze),
            vec![(
                67_236_366,
                0,
                from_door.0,
                from_door.1,
                None,
                (2_364.0, -3_350.0),
                (2_364.0, -3_350.0),
                false,
                MazeAdjustment::None,
                false,
                0,
            )],
            "fromDoor addCosts={add_costs}"
        );
    }
}

#[test]
fn a_thin_room_refuses_a_drill_unless_the_backtrack_door_intersects_it() {
    let mut fixture = drill_fixture();
    let counter = Counter::new();
    let mut ctrl = fixture.ctrl.clone();
    ctrl.compensated_trace_half_width[0] = 2000;
    let mut maze = MazeSearchEngine::get_instance(
        &set_of(&[2]),
        &set_of(&[3]),
        &mut fixture.engine,
        &mut fixture.board,
        &ctrl,
        &|| counter.check(),
    )
    .expect("init succeeds");
    let from = maze.queue.iter().next().expect("a seeded element").clone();
    let room_shape = maze
        .engine
        .rooms
        .room_shape(from.next_room.expect("a next room"))
        .expect("a shape")
        .clone();
    let page = maze.engine.drill_pages().overlapping_pages(&room_shape)[0];
    drain(&mut maze);
    MazeExpansionEngine::expand_to_drill_page(&mut maze, &mut fixture.board, page, &from);
    let page_element = maze.queue.iter().next().expect("the page element").clone();
    let drills =
        maze.engine
            .drill_page_drills(&mut fixture.board, page, ctrl.attach_smd_allowed(), &|| {
                counter.check()
            });
    let first = drills[0];

    assert!(from.backtrack_door.is_none());
    drain(&mut maze);
    MazeExpansionEngine::expand_to_drill(&mut maze, &mut fixture.board, first, &from, 0);
    assert_eq!(queue_rows(&maze), vec![]);

    let with_backtrack = MazeListElement {
        backtrack_door: Some(page_element.door),
        ..from.clone()
    };
    drain(&mut maze);
    MazeExpansionEngine::expand_to_drill(&mut maze, &mut fixture.board, first, &with_backtrack, 0);
    assert_eq!(
        queue_rows(&maze),
        vec![(
            67_236_366,
            0,
            8_137.326_308_149,
            11_652.747_664_386,
            None,
            (2_364.0, -3_350.0),
            (2_364.0, -3_350.0),
            false,
            MazeAdjustment::None,
            false,
            0,
        )]
    );
}

#[test]
fn a_free_space_drill_expands_to_the_other_layer_at_the_add_via_cost() {
    let mut fixture = drill_fixture();
    let counter = Counter::new();
    let plain_ctrl = fixture.ctrl.clone();
    let mut costed_ctrl = fixture.ctrl.clone();
    costed_ctrl.add_via_costs[0][1] = 700;
    costed_ctrl.add_via_costs[1][0] = 900;
    let mut maze = MazeSearchEngine::get_instance(
        &set_of(&[2]),
        &set_of(&[3]),
        &mut fixture.engine,
        &mut fixture.board,
        &plain_ctrl,
        &|| counter.check(),
    )
    .expect("init succeeds");
    let from = maze.queue.iter().next().expect("a seeded element").clone();
    let room_shape = maze
        .engine
        .rooms
        .room_shape(from.next_room.expect("a next room"))
        .expect("a shape")
        .clone();
    let page = maze.engine.drill_pages().overlapping_pages(&room_shape)[0];
    drain(&mut maze);
    MazeExpansionEngine::expand_to_drill_page(&mut maze, &mut fixture.board, page, &from);
    let page_element = maze.queue.iter().next().expect("the page element").clone();
    drain(&mut maze);
    MazeExpansionEngine::expand_to_drills_of_page(
        &mut maze,
        &mut fixture.board,
        &page_element,
        &|| counter.check(),
    );
    assert_eq!(maze.queue.len(), 2);
    let drill_element = maze.queue.iter().next().expect("a drill element").clone();

    drain(&mut maze);
    MazeExpansionEngine::expand_to_other_layers(&mut maze, &mut fixture.board, &drill_element);
    assert_eq!(
        queue_rows(&maze),
        vec![(
            -80_452_036,
            1,
            433.228_227_301,
            4_674.649_583_538,
            Some(25),
            (-2_130.5, -310.5),
            (-2_130.5, -310.5),
            false,
            MazeAdjustment::None,
            false,
            0,
        )]
    );

    maze.ctrl = &costed_ctrl;
    drain(&mut maze);
    MazeExpansionEngine::expand_to_other_layers(&mut maze, &mut fixture.board, &drill_element);
    assert_eq!(
        queue_rows(&maze),
        vec![(
            -80_452_036,
            1,
            1_133.228_227_301,
            5_374.649_583_538,
            Some(25),
            (-2_130.5, -310.5),
            (-2_130.5, -310.5),
            false,
            MazeAdjustment::None,
            false,
            0,
        )]
    );
}

#[test]
fn an_attach_smd_via_promotes_the_layer_and_the_via_mask_then_decides_the_span() {
    let mut board = probe_board();
    board
        .rules
        .via_infos
        .get_mut(ViaInfoId(0))
        .set_attach_smd_allowed(true);
    let mut via_rule = ViaRule::new("rule");
    via_rule.append_via(board.rules.via_infos.get(ViaInfoId(0)).clone());
    board.rules.via_rules[0] = via_rule.clone();
    let default_class = board.rules.get_default_net_class();
    board
        .rules
        .net_classes
        .get_mut(default_class)
        .set_via_rule(Some(via_rule));
    let mut engine = probe_engine(&mut board, 1);
    let base = probe_control(&board, 1);
    assert!(base.attach_smd_allowed());
    assert!(base.via_infos[0].attach_smd_allowed);
    let controls: Vec<AutorouteControl> = [false, true]
        .into_iter()
        .map(|mask_attach| {
            let mut ctrl = base.clone();
            ctrl.add_via_costs[0][1] = 700;
            ctrl.add_via_costs[1][0] = 900;
            ctrl.via_infos[0] = ViaMask {
                from_layer: 0,
                to_layer: 1,
                attach_smd_allowed: mask_attach,
            };
            ctrl
        })
        .collect();
    let counter = Counter::new();
    let mut maze = MazeSearchEngine::get_instance(
        &set_of(&[2]),
        &set_of(&[3]),
        &mut engine,
        &mut board,
        &base,
        &|| counter.check(),
    )
    .expect("init succeeds");

    let spots = [
        ("freeSpace", IntPoint::new(1000, 1000)),
        ("onBlocker", IntPoint::new(400, 0)),
        ("onSmdPin", IntPoint::new(-2000, 0)),
        ("onThruPin", IntPoint::new(2000, 0)),
        ("onFreeVia", IntPoint::new(2500, 2500)),
    ];
    let expected: [[CheckDrillResult; 2]; 5] = [
        [CheckDrillResult::Drillable, CheckDrillResult::Drillable],
        [CheckDrillResult::Drillable, CheckDrillResult::Drillable],
        [
            CheckDrillResult::DrillableWithAttachSmd,
            CheckDrillResult::Drillable,
        ],
        [
            CheckDrillResult::NotDrillable,
            CheckDrillResult::NotDrillable,
        ],
        [
            CheckDrillResult::NotDrillable,
            CheckDrillResult::NotDrillable,
        ],
    ];
    for ((name, location), want) in spots.iter().zip(expected) {
        let room_shape = TileShape::Box(IntBox::from_coords(
            location.x - 400,
            location.y - 400,
            location.x + 400,
            location.y + 400,
        ))
        .to_simplex();
        for (layer, want) in want.into_iter().enumerate() {
            assert_eq!(
                MazeExpansionEngine::check_layer_with_any_matching_via(
                    &mut maze,
                    &mut board,
                    &TileShape::Simplex(room_shape.clone()),
                    &Point::Int(*location),
                    layer,
                    &[1],
                ),
                want,
                "spot={name} layer={layer}"
            );
        }
    }

    let seed = maze.queue.iter().next().expect("a seeded element").clone();
    let room_shape = maze
        .engine
        .rooms
        .room_shape(seed.next_room.expect("a next room"))
        .expect("a shape")
        .clone();
    let page = maze.engine.drill_pages().overlapping_pages(&room_shape)[0];
    let page_drills =
        maze.engine
            .drill_page_drills(&mut board, page, base.attach_smd_allowed(), &|| {
                counter.check()
            });
    assert_eq!(page_drills.len(), 42);

    let location = Point::new(-2000, 0);
    let mut drill = ExpansionDrill::new(
        TileShape::Box(TileShape::get_instance_from_point(&location)),
        location,
        0,
        1,
    );
    assert!(drill.calculate_expansion_rooms(maze.engine, &mut board));
    let drill_rooms = drill.rooms.clone();
    let drill_id = fr_router::arena::DrillId(maze.engine.rooms.drills.insert(drill));
    assert_eq!(
        drill_rooms
            .iter()
            .map(|room| room.and_then(|r| maze.engine.rooms.room_id_no(r)))
            .collect::<Vec<_>>(),
        vec![Some(15), Some(249)]
    );
    assert_eq!(
        maze.engine.expandable_id_no(ExpandableRef::Drill(drill_id)),
        -59_581_999
    );

    let expected_rows: [(i32, f64, f64, Option<i32>); 2] = [
        (1, 1700.0, 5_671.421_356_237, Some(249)),
        (0, 1900.0, 5730.0, Some(15)),
    ];
    for section in 0..2i32 {
        let element = MazeListElement {
            door: ExpandableRef::Drill(drill_id),
            section_no_of_door: section,
            backtrack_door: None,
            section_no_of_backtrack_door: 0,
            expansion_value: 1000.0,
            sorting_value: 2000.0,
            next_room: None,
            shape_entry: FloatLine::new(
                FloatPoint::new(-2000.0, 0.0),
                FloatPoint::new(-2000.0, 0.0),
            ),
            room_ripped: false,
            adjustment: MazeAdjustment::None,
            already_checked: false,
            ripup_cost: 0,
        };
        for (index, ctrl) in controls.iter().enumerate() {
            maze.ctrl = ctrl;
            drain(&mut maze);
            MazeExpansionEngine::expand_to_other_layers(&mut maze, &mut board, &element);
            let rows = queue_rows(&maze);
            if index == 0 {
                assert_eq!(rows, vec![], "section={section} maskAttachSmdAllowed=false");
            } else {
                let want = expected_rows[usize::try_from(section).expect("0 or 1")];
                assert_eq!(rows.len(), 1, "section={section} maskAttachSmdAllowed=true");
                assert_eq!(
                    (rows[0].0, rows[0].1, rows[0].2, rows[0].3, rows[0].4),
                    (-59_581_999, want.0, want.1, want.2, want.3),
                    "section={section} maskAttachSmdAllowed=true"
                );
            }
        }
    }
}

#[test]
fn check_layer_with_any_matching_via_answers_javas_table() {
    let mut fixture = drill_fixture();
    let counter = Counter::new();
    let maze = MazeSearchEngine::get_instance(
        &set_of(&[2]),
        &set_of(&[3]),
        &mut fixture.engine,
        &mut fixture.board,
        &fixture.ctrl,
        &|| counter.check(),
    )
    .expect("init succeeds");
    let mut maze = maze;

    let spots = [
        ("freeSpace", IntPoint::new(1000, 1000)),
        ("onBlocker", IntPoint::new(400, 0)),
        ("onSmdPin", IntPoint::new(-2000, 0)),
        ("onThruPin", IntPoint::new(2000, 0)),
        ("onFreeVia", IntPoint::new(2500, 2500)),
    ];
    let expected: [[CheckDrillResult; 2]; 5] = [
        [CheckDrillResult::Drillable, CheckDrillResult::Drillable],
        [CheckDrillResult::Drillable, CheckDrillResult::Drillable],
        [CheckDrillResult::NotDrillable, CheckDrillResult::Drillable],
        [
            CheckDrillResult::NotDrillable,
            CheckDrillResult::NotDrillable,
        ],
        [
            CheckDrillResult::NotDrillable,
            CheckDrillResult::NotDrillable,
        ],
    ];
    for (half_size, table) in [(60, None), (400, Some(expected))] {
        for (index, (name, location)) in spots.iter().enumerate() {
            let room_shape = TileShape::Box(IntBox::from_coords(
                location.x - half_size,
                location.y - half_size,
                location.x + half_size,
                location.y + half_size,
            ))
            .to_simplex();
            for layer in 0..2usize {
                let result = MazeExpansionEngine::check_layer_with_any_matching_via(
                    &mut maze,
                    &mut fixture.board,
                    &TileShape::Simplex(room_shape.clone()),
                    &Point::Int(*location),
                    layer,
                    &[1],
                );
                let want = match table {
                    None => CheckDrillResult::NotDrillable,
                    Some(t) => t[index][layer],
                };
                assert_eq!(
                    result,
                    want,
                    "room={} spot={name} layer={layer}",
                    2 * half_size
                );
            }
        }
    }
}

#[test]
fn find_connection_reaches_the_destination_door_in_four_pops() {
    let mut board = simple_board_ninety();
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
    .expect("init succeeds");
    assert_eq!(maze.queue.len(), 1);

    let expected: [(i32, i32, f64, f64, bool, usize); 4] = [
        (65, 0, 0.0, 630.0, true, 2),
        (1, 0, 141.421_356_237, 771.421_356_237, true, 8),
        (-297_909, 0, 252.421_356_237, 591.421_356_237, true, 7),
        (96, 0, 800.0, 800.0, false, 6),
    ];
    for (index, want) in expected.into_iter().enumerate() {
        let head = maze.queue.iter().next().expect("a head").clone();
        let head_id = maze.engine.expandable_id_no(head.door);
        let more = maze.occupy_next_element(&mut board, &|| counter.check());
        assert_eq!(
            (
                head_id,
                head.section_no_of_door,
                r9(head.expansion_value),
                r9(head.sorting_value),
                more,
                maze.queue.len(),
            ),
            want,
            "pop[{index}]"
        );
        if !more {
            break;
        }
    }
    let destination = maze.destination_door().expect("the destination door");
    assert_eq!(maze.engine.expandable_id_no(destination), 96);
    assert_eq!(maze.section_no_of_destination_door(), 0);
    assert!(matches!(destination, ExpandableRef::TargetDoor(_)));
    assert_eq!(maze.queue.len(), 6);
}

#[test]
fn find_connection_answers_the_result_the_pop_loop_leaves_behind() {
    let mut board = simple_board_ninety();
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
    .expect("init succeeds");
    let result = maze
        .find_connection(&mut board, &|| counter.check())
        .expect("the destination is reached");
    assert_eq!(maze.engine.expandable_id_no(result.destination_door), 96);
    assert_eq!(result.section_no_of_door, 0);
    assert_eq!(
        queue_rows(&maze)
            .into_iter()
            .map(|row| (row.0, row.2, row.3))
            .collect::<Vec<_>>(),
        vec![
            (-12_022_109, 363.421_356_237, 1_038.302_086_441),
            (-10_916_959, 363.421_356_237, 1_038.302_086_441),
            (-23_088_024, 252.421_356_237, 1_173.421_356_237),
            (23_385_936, 1_072.421_356_237, 1_213.421_356_237),
            (17_759_281, 999.421_938_988, 1_281.421_938_988),
            (18_883_651, 999.421_938_988, 1_281.421_938_988),
        ]
    );
}

#[test]
fn an_obstacle_via_room_expands_only_when_ripup_is_allowed_and_marks_the_element_ripped() {
    let mut fixture = drill_fixture();
    let counter = Counter::new();
    let mut ctrl = fixture.ctrl.clone();
    ctrl.add_via_costs[0][1] = 700;
    ctrl.add_via_costs[1][0] = 900;
    let mut ripup_ctrl = ctrl.clone();
    ripup_ctrl.ripup_allowed = true;
    let mut maze = MazeSearchEngine::get_instance(
        &set_of(&[2]),
        &set_of(&[3]),
        &mut fixture.engine,
        &mut fixture.board,
        &ctrl,
        &|| counter.check(),
    )
    .expect("init succeeds");
    let via_drill = fr_router::autoroute::maze::expansion_engine::via_autoroute_drill_info(
        maze.engine,
        &mut fixture.board,
        ItemId(7),
    )
    .expect("the via has a drill");
    assert_eq!(
        maze.engine
            .expandable_id_no(ExpandableRef::Drill(via_drill)),
        76_880_001
    );
    let rooms = maze
        .engine
        .rooms
        .drills
        .get(via_drill.0)
        .expect("the drill")
        .rooms
        .clone();
    assert!(matches!(rooms[0], Some(RoomRef::Obstacle(_))));
    assert!(matches!(rooms[1], Some(RoomRef::Obstacle(_))));

    let element = MazeListElement {
        door: ExpandableRef::Drill(via_drill),
        section_no_of_door: 0,
        backtrack_door: None,
        section_no_of_backtrack_door: 0,
        expansion_value: 1_000.0,
        sorting_value: 2_000.0,
        next_room: rooms[0],
        shape_entry: FloatLine::new(
            FloatPoint::new(2_000.0, 2_000.0),
            FloatPoint::new(2_000.0, 2_000.0),
        ),
        room_ripped: false,
        adjustment: MazeAdjustment::None,
        already_checked: false,
        ripup_cost: 0,
    };
    drain(&mut maze);
    MazeExpansionEngine::expand_to_other_layers(&mut maze, &mut fixture.board, &element);
    assert_eq!(queue_rows(&maze), vec![], "ripupAllowed=false");

    maze.ctrl = &ripup_ctrl;
    MazeExpansionEngine::expand_to_other_layers(&mut maze, &mut fixture.board, &element);
    let rows = queue_rows(&maze);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].0, 76_880_001);
    assert_eq!(rows[0].1, 1);
    assert_eq!(rows[0].2, 1_700.0);
    assert_eq!(rows[0].3, 3_530.0);
    assert!(rows[0].7, "roomRipped");
}
