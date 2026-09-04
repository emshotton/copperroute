#![allow(clippy::too_many_lines)]

use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};

use fr_board::ids::{ItemId, PadstackId, ViaInfoId};
use fr_board::prelude::*;
use fr_board::rules::{ViaInfo, ViaRule};
use fr_dsn::format::double::java_double_to_string;
use fr_geometry::{
    FloatPoint, IntBox, IntOctagon, IntPoint, IntVector, Point, Polyline, Shape, TileShape,
};
use fr_router::autoroute::expansion::{ExpandableRef, RoomRef};
use fr_router::autoroute::maze::AutorouteControl;
use fr_router::autoroute::maze::engine::AutorouteEngine;
use fr_router::autoroute::maze::search::MazeResult;
use fr_router::autoroute::maze::search::MazeSearchEngine;
use fr_router::autoroute::path::{
    FoundConnectionLocator, LocatorKind, calculate_additional_corner,
};
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

fn simple_board_fortyfive() -> Board {
    let mut board = simple_board();
    board.rules.trace_angle_restriction = AngleRestriction::FortyFiveDegree;
    board
}

fn blocked_board() -> Board {
    let mut board = simple_board();
    let default_class = board.rules.get_default_net_class();
    board.rules.nets.add("N2", 1, false, default_class);
    board.insert_trace_without_cleaning(
        Polyline::from_points(&[Point::new(0, -900), Point::new(0, 900)]),
        0,
        30,
        vec![2],
        1,
        FixedState::Unfixed,
    );
    board
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

struct Located {
    locator: FoundConnectionLocator,
    ripped: BTreeSet<ItemId>,
    ripup_costs: BTreeMap<ItemId, i32>,
}

fn locate(board: &mut Board, angle: AngleRestriction, ripup_no_vias: bool) -> Located {
    locate_between(board, angle, ripup_no_vias, &[2], &[3])
}

fn locate_between(
    board: &mut Board,
    angle: AngleRestriction,
    ripup_no_vias: bool,
    start: &[u32],
    dest: &[u32],
) -> Located {
    let mut engine = probe_engine(board, 1);
    let mut ctrl = probe_control(board, 1);
    if ripup_no_vias {
        ctrl.vias_allowed = false;
        ctrl.ripup_allowed = true;
        ctrl.ripup_costs = 1000;
    }
    let counter = Counter::new();
    let result = {
        let mut maze = MazeSearchEngine::get_instance(
            &set_of(start),
            &set_of(dest),
            &mut engine,
            board,
            &ctrl,
            &|| counter.check(),
        )
        .expect("MazeSearchEngine.getInstance answers a search engine");
        maze.find_connection(board, &|| counter.check())
            .expect("findConnection answers a result")
    };
    let mut ripped = BTreeSet::new();
    let mut ripup_costs = BTreeMap::new();
    let locator = FoundConnectionLocator::get_instance(
        Some(&result),
        &ctrl,
        &mut engine,
        board,
        angle,
        &mut ripped,
        Some(&mut ripup_costs),
    )
    .expect("getInstance answers a locator for a non-null result");
    Located {
        locator,
        ripped,
        ripup_costs,
    }
}

const REGIMES: [AngleRestriction; 3] = [
    AngleRestriction::NinetyDegree,
    AngleRestriction::FortyFiveDegree,
    AngleRestriction::None,
];

type Item = (usize, Vec<(i32, i32)>);

fn items(located: &Located) -> Vec<Item> {
    located
        .locator
        .connection_items
        .iter()
        .map(|item| {
            (
                item.layer,
                item.corners.iter().map(|c| (c.x, c.y)).collect(),
            )
        })
        .collect()
}

#[test]
fn ninety_and_fortyfive_share_one_implementation() {
    assert_eq!(
        LocatorKind::of(AngleRestriction::NinetyDegree),
        LocatorKind::FortyFiveDegree
    );
    assert_eq!(
        LocatorKind::of(AngleRestriction::FortyFiveDegree),
        LocatorKind::FortyFiveDegree
    );
}

#[test]
fn any_angle_does_not() {
    assert_eq!(
        LocatorKind::of(AngleRestriction::None),
        LocatorKind::AnyAngle
    );
}

#[test]
fn get_instance_answers_none_only_for_a_null_maze_result() {
    let mut board = simple_board();
    let mut engine = probe_engine(&mut board, 1);
    let ctrl = probe_control(&board, 1);
    let mut ripped = BTreeSet::new();
    assert!(
        FoundConnectionLocator::get_instance(
            None,
            &ctrl,
            &mut engine,
            &mut board,
            AngleRestriction::None,
            &mut ripped,
            None,
        )
        .is_none()
    );
}

#[test]
fn the_additional_corner_of_every_regime_matches_the_jvm() {
    let expected: [(&str, &str, bool, &str, &str, &str); 24] = [
        (
            "(0.0,0.0)",
            "(100.0,300.0)",
            true,
            "(100.0,0.0)",
            "(100.0,100.0)",
            "(100.0,300.0)",
        ),
        (
            "(0.0,0.0)",
            "(100.0,300.0)",
            false,
            "(0.0,300.0)",
            "(0.0,200.0)",
            "(100.0,300.0)",
        ),
        (
            "(0.0,0.0)",
            "(100.0,-300.0)",
            true,
            "(100.0,0.0)",
            "(100.0,-100.0)",
            "(100.0,-300.0)",
        ),
        (
            "(0.0,0.0)",
            "(100.0,-300.0)",
            false,
            "(0.0,-300.0)",
            "(0.0,-200.0)",
            "(100.0,-300.0)",
        ),
        (
            "(0.0,0.0)",
            "(-100.0,300.0)",
            true,
            "(-100.0,0.0)",
            "(-100.0,100.0)",
            "(-100.0,300.0)",
        ),
        (
            "(0.0,0.0)",
            "(-100.0,300.0)",
            false,
            "(0.0,300.0)",
            "(0.0,200.0)",
            "(-100.0,300.0)",
        ),
        (
            "(0.0,0.0)",
            "(-100.0,-300.0)",
            true,
            "(-100.0,0.0)",
            "(-100.0,-100.0)",
            "(-100.0,-300.0)",
        ),
        (
            "(0.0,0.0)",
            "(-100.0,-300.0)",
            false,
            "(0.0,-300.0)",
            "(0.0,-200.0)",
            "(-100.0,-300.0)",
        ),
        (
            "(0.0,0.0)",
            "(300.0,100.0)",
            true,
            "(300.0,0.0)",
            "(200.0,0.0)",
            "(300.0,100.0)",
        ),
        (
            "(0.0,0.0)",
            "(300.0,100.0)",
            false,
            "(0.0,100.0)",
            "(100.0,100.0)",
            "(300.0,100.0)",
        ),
        (
            "(0.0,0.0)",
            "(-300.0,100.0)",
            true,
            "(-300.0,0.0)",
            "(-200.0,0.0)",
            "(-300.0,100.0)",
        ),
        (
            "(0.0,0.0)",
            "(-300.0,100.0)",
            false,
            "(0.0,100.0)",
            "(-100.0,100.0)",
            "(-300.0,100.0)",
        ),
        (
            "(0.0,0.0)",
            "(300.0,-100.0)",
            true,
            "(300.0,0.0)",
            "(200.0,0.0)",
            "(300.0,-100.0)",
        ),
        (
            "(0.0,0.0)",
            "(300.0,-100.0)",
            false,
            "(0.0,-100.0)",
            "(100.0,-100.0)",
            "(300.0,-100.0)",
        ),
        (
            "(0.0,0.0)",
            "(200.0,200.0)",
            true,
            "(200.0,0.0)",
            "(200.0,200.0)",
            "(200.0,200.0)",
        ),
        (
            "(0.0,0.0)",
            "(200.0,200.0)",
            false,
            "(0.0,200.0)",
            "(0.0,0.0)",
            "(200.0,200.0)",
        ),
        (
            "(0.0,0.0)",
            "(100.0,0.0)",
            true,
            "(100.0,0.0)",
            "(100.0,0.0)",
            "(100.0,0.0)",
        ),
        (
            "(0.0,0.0)",
            "(100.0,0.0)",
            false,
            "(0.0,0.0)",
            "(0.0,0.0)",
            "(100.0,0.0)",
        ),
        (
            "(17.5,-3.25)",
            "(-9.75,-3.25)",
            true,
            "(-9.75,-3.25)",
            "(-9.75,-3.25)",
            "(-9.75,-3.25)",
        ),
        (
            "(17.5,-3.25)",
            "(-9.75,-3.25)",
            false,
            "(17.5,-3.25)",
            "(17.5,-3.25)",
            "(-9.75,-3.25)",
        ),
        (
            "(17.5,-3.25)",
            "(17.5,42.5)",
            true,
            "(17.5,-3.25)",
            "(17.5,-3.25)",
            "(17.5,42.5)",
        ),
        (
            "(17.5,-3.25)",
            "(17.5,42.5)",
            false,
            "(17.5,42.5)",
            "(17.5,42.5)",
            "(17.5,42.5)",
        ),
        (
            "(-1.5,2.5)",
            "(-1.5,2.5)",
            true,
            "(-1.5,2.5)",
            "(-1.5,2.5)",
            "(-1.5,2.5)",
        ),
        (
            "(-1.5,2.5)",
            "(-1.5,2.5)",
            false,
            "(-1.5,2.5)",
            "(-1.5,2.5)",
            "(-1.5,2.5)",
        ),
    ];

    for (from_text, to_text, horizontal_first, ninety, fortyfive, none) in expected {
        let from = parse_point(from_text);
        let to = parse_point(to_text);
        let got: Vec<String> = REGIMES
            .into_iter()
            .map(|regime| {
                format_point(calculate_additional_corner(
                    from,
                    to,
                    horizontal_first,
                    regime,
                ))
            })
            .collect();
        assert_eq!(
            got,
            vec![ninety.to_string(), fortyfive.to_string(), none.to_string()],
            "from={from_text} to={to_text} hf={horizontal_first}"
        );
    }
}

fn format_point(point: FloatPoint) -> String {
    format!(
        "({},{})",
        java_double_to_string(point.x),
        java_double_to_string(point.y)
    )
}

fn parse_point(text: &str) -> FloatPoint {
    let inner = text.trim_start_matches('(').trim_end_matches(')');
    let (x, y) = inner.split_once(',').expect("a comma-separated pair");
    FloatPoint::new(x.parse().expect("a double"), y.parse().expect("a double"))
}

#[test]
fn the_backtrack_walk_reproduces_the_jvms_door_chain() {
    let mut board = simple_board_fortyfive();
    let located = locate(&mut board, AngleRestriction::None, false);
    let mut engine = probe_engine(&mut board, 1);
    let _ = &mut engine;

    let arr = &located.locator.backtrack_array;
    assert_eq!(arr.len(), 3);
    assert!(matches!(arr[0].door, ExpandableRef::TargetDoor(_)));
    assert_eq!(arr[0].section_no_of_door, 0);
    assert!(matches!(arr[0].next_room, Some(RoomRef::Complete(_))));
    assert!(matches!(arr[1].door, ExpandableRef::Door(_)));
    assert_eq!(arr[1].section_no_of_door, 0);
    assert!(matches!(arr[1].next_room, Some(RoomRef::Complete(_))));
    assert!(matches!(arr[2].door, ExpandableRef::TargetDoor(_)));
    assert_eq!(arr[2].section_no_of_door, 0);
    assert_eq!(arr[2].next_room, None, "the start door has no other room");
}

#[test]
fn a_single_room_connection_yields_one_trace_with_the_java_corners() {
    let mut board = simple_board_fortyfive();
    let located = locate(&mut board, AngleRestriction::NinetyDegree, false);
    assert_eq!(located.locator.start_item, Some(ItemId(2)));
    assert_eq!(located.locator.start_layer, 0);
    assert_eq!(located.locator.target_item, Some(ItemId(3)));
    assert_eq!(located.locator.target_layer, 0);
    assert_eq!(
        items(&located),
        vec![(
            0,
            vec![
                (400, 0),
                (400, -132),
                (0, -132),
                (-132, -132),
                (-400, -132),
                (-400, 0)
            ]
        )]
    );
    assert!(located.ripped.is_empty());
}

#[test]
fn the_three_regimes_locate_three_different_corner_lists() {
    let expected: [Vec<(i32, i32)>; 3] = [
        vec![
            (400, 0),
            (400, -132),
            (0, -132),
            (-132, -132),
            (-400, -132),
            (-400, 0),
        ],
        vec![
            (400, 0),
            (268, -132),
            (0, -132),
            (-132, -132),
            (-268, -132),
            (-400, 0),
        ],
        vec![(400, 0), (0, -140), (-400, 0)],
    ];
    assert_eq!(
        expected
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        3,
        "the three regimes must locate three DIFFERENT corner lists"
    );
    for (regime, want) in REGIMES.into_iter().zip(expected) {
        let mut board = simple_board_fortyfive();
        let located = locate(&mut board, regime, false);
        assert_eq!(items(&located), vec![(0, want)], "regime {regime:?}");
    }
}

#[test]
fn the_free_angle_simple_board_search_finds_nothing() {
    let mut board = simple_board();
    assert_eq!(
        board.rules.trace_angle_restriction,
        AngleRestriction::None,
        "the retired arm is the FREE-ANGLE one"
    );
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
    .expect("`getInstance` still builds the search: it is `findConnection` that answers None");
    assert!(
        maze.find_connection(&mut board, &|| counter.check())
            .is_none(),
        "#165's second half detached the abandoned room's doors and this board's free-angle path \
         ran through it — if this ever answers again, #165 has been undone"
    );
}

#[test]
fn a_layer_change_yields_two_traces_and_no_via_entry() {
    let mut board = probe_board();
    let located = locate(&mut board, AngleRestriction::NinetyDegree, false);

    let arr = &located.locator.backtrack_array;
    assert_eq!(arr.len(), 8);
    let kinds: Vec<&str> = arr
        .iter()
        .map(|e| match e.door {
            ExpandableRef::TargetDoor(_) => "target",
            ExpandableRef::Door(_) => "door",
            ExpandableRef::Drill(_) => "drill",
            ExpandableRef::Page(_) => "page",
        })
        .collect();
    assert_eq!(
        kinds,
        vec![
            "target", "drill", "drill", "door", "drill", "drill", "door", "target"
        ]
    );
    assert_eq!(
        arr.iter().map(|e| e.section_no_of_door).collect::<Vec<_>>(),
        vec![0, 0, 1, 1, 1, 0, 1, 0]
    );

    assert_eq!(
        items(&located),
        vec![
            (0, vec![(2000, 0), (1180, 0)]),
            (
                1,
                vec![
                    (1180, 0),
                    (1180, -974),
                    (451, -974),
                    (372, -974),
                    (372, -1080),
                    (-130, -1080),
                    (-130, -974)
                ]
            ),
            (
                0,
                vec![(-130, -974), (-130, 0), (-130, 132), (-130, 0), (-2000, 0)]
            ),
        ]
    );
    assert_eq!(located.locator.start_layer, 0);
    assert_eq!(located.locator.target_layer, 0);
}

#[test]
fn the_layer_change_splits_the_same_way_in_all_three_regimes() {
    let expected: [Vec<Item>; 3] = [
        vec![
            (0, vec![(2000, 0), (1180, 0)]),
            (
                1,
                vec![
                    (1180, 0),
                    (1180, -974),
                    (451, -974),
                    (372, -974),
                    (372, -1080),
                    (-130, -1080),
                    (-130, -974),
                ],
            ),
            (
                0,
                vec![(-130, -974), (-130, 0), (-130, 132), (-130, 0), (-2000, 0)],
            ),
        ],
        vec![
            (0, vec![(2000, 0), (1180, 0)]),
            (
                1,
                vec![
                    (1180, 0),
                    (1180, -245),
                    (451, -974),
                    (372, -1053),
                    (372, -1080),
                    (-24, -1080),
                    (-130, -974),
                ],
            ),
            (
                0,
                vec![(-130, -974), (-130, 0), (-130, 132), (-262, 0), (-2000, 0)],
            ),
        ],
        vec![
            (0, vec![(2000, 0), (1180, 0)]),
            (1, vec![(1180, 0), (-130, -974)]),
            (0, vec![(-130, -974), (-1962, 220), (-2000, 0)]),
        ],
    ];
    for (regime, want) in REGIMES.into_iter().zip(expected) {
        let mut board = probe_board();
        let located = locate(&mut board, regime, false);
        assert_eq!(items(&located), want, "regime {regime:?}");
    }
}

#[test]
fn the_long_way_round_matches_the_jvm_in_all_three_regimes() {
    let expected: [Vec<(i32, i32)>; 3] = [
        vec![
            (2000, 0),
            (874, 0),
            (874, -501),
            (874, -1981),
            (1701, -1981),
            (1701, -1784),
            (1450, -1784),
            (1450, -1845),
            (1333, -1845),
            (1333, -2560),
            (862, -2560),
            (789, -2560),
            (789, -2670),
            (-516, -2670),
            (-516, -2516),
            (-516, -2468),
            (-563, -2468),
            (-563, -2395),
            (-636, -2395),
            (-636, -2301),
            (-729, -2301),
            (-729, -1252),
            (-1866, -1252),
            (-1998, -1252),
            (-2000, -1252),
            (-2000, 0),
        ],
        vec![
            (2000, 0),
            (1375, 0),
            (874, -501),
            (874, -1154),
            (1701, -1981),
            (1504, -1784),
            (1450, -1784),
            (1389, -1845),
            (1333, -1845),
            (1333, -2089),
            (862, -2560),
            (789, -2633),
            (789, -2670),
            (-362, -2670),
            (-516, -2516),
            (-516, -2515),
            (-563, -2468),
            (-636, -2395),
            (-636, -2394),
            (-729, -2301),
            (-1778, -1252),
            (-1866, -1252),
            (-1998, -1252),
            (-2000, -1250),
            (-2000, 0),
        ],
        vec![
            (2000, 0),
            (331, -2249),
            (278, -2282),
            (-154, -2282),
            (-250, -2177),
            (-1990, -57),
            (-2000, 0),
        ],
    ];
    for (regime, want) in REGIMES.into_iter().zip(expected) {
        let mut board = probe_board();
        let located = locate(&mut board, regime, true);
        assert_eq!(items(&located), vec![(0, want)], "regime {regime:?}");
        assert!(
            located.ripped.is_empty(),
            "regime {regime:?}: nothing is ripped"
        );
    }
}

#[test]
fn the_long_way_round_has_eight_expansion_doors_and_no_drill() {
    let mut board = probe_board();
    let located = locate(&mut board, AngleRestriction::NinetyDegree, true);
    let arr = &located.locator.backtrack_array;
    assert_eq!(arr.len(), 9);
    assert!(matches!(arr[0].door, ExpandableRef::TargetDoor(_)));
    assert!(matches!(arr[8].door, ExpandableRef::TargetDoor(_)));
    assert!(
        arr[1..8]
            .iter()
            .all(|e| matches!(e.door, ExpandableRef::Door(_))),
        "no drill on the path, so `layerChanged` never fires"
    );
    assert_eq!(
        arr.iter().map(|e| e.section_no_of_door).collect::<Vec<_>>(),
        vec![0, 2, 2, 1, 2, 2, 1, 2, 0]
    );
    assert_eq!(located.locator.connection_items.len(), 1);
}

#[test]
fn the_reversed_search_reaches_the_left_turn_corner() {
    let expected: [Vec<(i32, i32)>; 3] = [
        vec![
            (-2000, 0),
            (-2000, 78),
            (-1866, 78),
            (-1734, 78),
            (-1734, 1203),
            (-1751, 1203),
            (-1816, 1203),
            (-1816, 1318),
            (-1772, 1318),
            (-1772, 1960),
            (-1772, 2092),
            (-1772, 2272),
            (-24, 2272),
            (-24, 2428),
            (396, 2428),
            (396, 2402),
            (617, 2402),
            (1196, 2402),
            (1196, 2396),
            (1196, 2474),
            (2157, 2474),
            (1225, 2474),
            (1225, 2225),
            (1225, 2094),
            (1241, 2094),
            (1241, 1504),
            (1722, 1504),
            (1722, 1422),
            (1825, 1422),
            (2000, 1422),
            (2000, 0),
        ],
        vec![
            (-2000, 0),
            (-1922, 78),
            (-1866, 78),
            (-1734, 78),
            (-1734, 1186),
            (-1751, 1203),
            (-1816, 1268),
            (-1816, 1318),
            (-1772, 1362),
            (-1772, 1960),
            (-1772, 2092),
            (-1592, 2272),
            (-24, 2272),
            (132, 2428),
            (396, 2428),
            (422, 2402),
            (617, 2402),
            (1190, 2402),
            (1196, 2396),
            (1274, 2474),
            (2157, 2474),
            (1474, 2474),
            (1225, 2225),
            (1225, 2110),
            (1241, 2094),
            (1241, 1985),
            (1722, 1504),
            (1804, 1422),
            (1825, 1422),
            (2000, 1247),
            (2000, 0),
        ],
        vec![
            (-2000, 0),
            (-682, 2064),
            (-646, 2081),
            (-629, 2088),
            (-121, 2280),
            (-96, 2282),
            (93, 2282),
            (96, 2282),
            (157, 2279),
            (250, 2177),
            (2000, 0),
        ],
    ];
    for (regime, want) in REGIMES.into_iter().zip(expected) {
        let mut board = probe_board();
        let located = locate_between(&mut board, regime, true, &[3], &[2]);
        assert_eq!(located.locator.start_item, Some(ItemId(3)));
        assert_eq!(located.locator.target_item, Some(ItemId(2)));
        assert_eq!(items(&located), vec![(0, want)], "regime {regime:?}");
    }
}

#[test]
fn the_reversed_search_with_vias_splits_into_three_traces() {
    let expected: [Vec<Item>; 3] = [
        vec![
            (
                0,
                vec![
                    (-2000, 0),
                    (-2000, -74),
                    (-2000, -76),
                    (-1734, -76),
                    (-1734, 56),
                    (-1734, 188),
                    (-1734, 815),
                    (-764, 815),
                ],
            ),
            (
                1,
                vec![
                    (-764, 815),
                    (446, 815),
                    (446, 435),
                    (446, 409),
                    (469, 409),
                    (556, 409),
                    (556, 309),
                    (1180, 309),
                    (1180, 0),
                ],
            ),
            (0, vec![(1180, 0), (2000, 0)]),
        ],
        vec![
            (
                0,
                vec![
                    (-2000, 0),
                    (-2000, -74),
                    (-2000, -76),
                    (-1866, -76),
                    (-1734, 56),
                    (-1734, 188),
                    (-1107, 815),
                    (-764, 815),
                ],
            ),
            (
                1,
                vec![
                    (-764, 815),
                    (66, 815),
                    (446, 435),
                    (446, 432),
                    (469, 409),
                    (556, 322),
                    (556, 309),
                    (871, 309),
                    (1180, 0),
                ],
            ),
            (0, vec![(1180, 0), (2000, 0)]),
        ],
        vec![
            (
                0,
                vec![
                    (-2000, 0),
                    (-2000, -74),
                    (-1846, -76),
                    (-1801, -62),
                    (-764, 815),
                ],
            ),
            (1, vec![(-764, 815), (1180, 0)]),
            (0, vec![(1180, 0), (2000, 0)]),
        ],
    ];
    for (regime, want) in REGIMES.into_iter().zip(expected) {
        let mut board = probe_board();
        let located = locate_between(&mut board, regime, false, &[3], &[2]);
        assert_eq!(items(&located), want, "regime {regime:?}");
    }
}

#[test]
fn a_ripped_obstacle_room_reaches_the_ripped_item_list_with_its_cost() {
    let mut board = blocked_board();
    let located = locate(&mut board, AngleRestriction::NinetyDegree, true);

    let arr = &located.locator.backtrack_array;
    assert_eq!(arr.len(), 4);
    assert!(
        matches!(arr[1].next_room, Some(RoomRef::Obstacle(_))),
        "the ripped trace's obstacle room is the second element's next room"
    );

    assert_eq!(located.ripped, set_of(&[4]));
    assert_eq!(
        located.ripup_costs,
        BTreeMap::from([(ItemId(4), 1)]),
        "MazeSearchEngine.ALREADY_RIPPED_COSTS is 1"
    );
    assert_eq!(
        items(&located),
        vec![(
            0,
            vec![(400, 0), (130, 0), (0, 0), (-130, 0), (-262, 0), (-400, 0)]
        )]
    );
}

#[test]
fn the_ripped_item_list_does_not_depend_on_the_angle_regime() {
    let expected: [Vec<(i32, i32)>; 3] = [
        vec![(400, 0), (130, 0), (0, 0), (-130, 0), (-262, 0), (-400, 0)],
        vec![(400, 0), (130, 0), (0, 0), (-130, 0), (-262, 0), (-400, 0)],
        vec![(400, 0), (-400, 0)],
    ];
    for (regime, want) in REGIMES.into_iter().zip(expected) {
        let mut board = blocked_board();
        let located = locate(&mut board, regime, true);
        assert_eq!(located.ripped, set_of(&[4]), "regime {regime:?}");
        assert_eq!(items(&located), vec![(0, want)], "regime {regime:?}");
    }
}

#[test]
fn a_null_ripup_cost_map_still_fills_the_ripped_item_set() {
    let mut board = blocked_board();
    let mut engine = probe_engine(&mut board, 1);
    let mut ctrl = probe_control(&board, 1);
    ctrl.vias_allowed = false;
    ctrl.ripup_allowed = true;
    ctrl.ripup_costs = 1000;
    let counter = Counter::new();
    let result = {
        let mut maze = MazeSearchEngine::get_instance(
            &set_of(&[2]),
            &set_of(&[3]),
            &mut engine,
            &mut board,
            &ctrl,
            &|| counter.check(),
        )
        .expect("a search engine");
        maze.find_connection(&mut board, &|| counter.check())
            .expect("a result")
    };
    let mut ripped = BTreeSet::new();
    let locator = FoundConnectionLocator::get_instance(
        Some(&result),
        &ctrl,
        &mut engine,
        &mut board,
        AngleRestriction::NinetyDegree,
        &mut ripped,
        None,
    )
    .expect("a locator");
    assert_eq!(ripped, set_of(&[4]));
    assert_eq!(locator.connection_items.len(), 1);
}

#[test]
fn an_unexpected_destination_door_yields_an_empty_connection_with_the_start_fields_set() {
    let mut board = simple_board_fortyfive();
    let mut engine = probe_engine(&mut board, 1);
    let ctrl = probe_control(&board, 1);
    let counter = Counter::new();
    let result = {
        let mut maze = MazeSearchEngine::get_instance(
            &set_of(&[2]),
            &set_of(&[3]),
            &mut engine,
            &mut board,
            &ctrl,
            &|| counter.check(),
        )
        .expect("a search engine");
        maze.find_connection(&mut board, &|| counter.check())
            .expect("a result")
    };
    let mut ripped = BTreeSet::new();
    let real = FoundConnectionLocator::get_instance(
        Some(&result),
        &ctrl,
        &mut engine,
        &mut board,
        AngleRestriction::None,
        &mut ripped,
        None,
    )
    .expect("a locator");
    let middle = real.backtrack_array[1];
    assert!(matches!(middle.door, ExpandableRef::Door(_)));
    assert_eq!(middle.section_no_of_door, 0);

    let forged = MazeResult {
        destination_door: middle.door,
        section_no_of_door: middle.section_no_of_door,
    };
    let mut ripped = BTreeSet::new();
    let located = FoundConnectionLocator::get_instance(
        Some(&forged),
        &ctrl,
        &mut engine,
        &mut board,
        AngleRestriction::None,
        &mut ripped,
        None,
    )
    .expect("getInstance still answers a locator: only a null result is None");
    assert_eq!(located.start_item, Some(ItemId(2)));
    assert_eq!(located.start_layer, 0);
    assert_eq!(located.target_item, None);
    assert_eq!(located.target_layer, 0);
    assert_eq!(located.backtrack_array.len(), 2);
    assert_eq!(
        located.connection_items,
        Vec::new(),
        "`:101` assigned an empty list before the `:130-135` return, and it is never null"
    );
}

#[test]
fn a_start_door_that_is_not_a_target_door_yields_an_all_default_locator() {
    let mut board = blocked_board();
    let mut engine = probe_engine(&mut board, 1);
    let ctrl = probe_control(&board, 1);
    let counter = Counter::new();
    let result = {
        let mut maze = MazeSearchEngine::get_instance(
            &set_of(&[2]),
            &set_of(&[3]),
            &mut engine,
            &mut board,
            &ctrl,
            &|| counter.check(),
        )
        .expect("a search engine");
        maze.find_connection(&mut board, &|| counter.check())
            .expect("a result")
    };
    assert!(matches!(
        result.destination_door,
        ExpandableRef::TargetDoor(_)
    ));

    let mut orphan: Option<(ExpandableRef, i32)> = None;
    'scan: for room in engine.complete_expansion_rooms().to_vec() {
        for door in engine.rooms.room_doors(RoomRef::Complete(room)).to_vec() {
            let object = ExpandableRef::Door(door);
            let Some(count) = engine.maze_search_element_count(object) else {
                continue;
            };
            for section in 0..i32::try_from(count).expect("a small section count") {
                if engine
                    .maze_search_element(object, section)
                    .is_some_and(|element| element.backtrack_door.is_none())
                {
                    orphan = Some((object, section));
                    break 'scan;
                }
            }
        }
    }
    let (orphan_door, orphan_section) =
        orphan.expect("the search leaves at least one door section unreached");
    assert_eq!(orphan_section, 0);

    let forged = MazeResult {
        destination_door: orphan_door,
        section_no_of_door: orphan_section,
    };
    let mut ripped = BTreeSet::new();
    let located = FoundConnectionLocator::get_instance(
        Some(&forged),
        &ctrl,
        &mut engine,
        &mut board,
        AngleRestriction::None,
        &mut ripped,
        None,
    )
    .expect("getInstance still answers a locator: only a null result is None");
    assert_eq!(located.start_item, None);
    assert_eq!(located.start_layer, 0);
    assert_eq!(located.target_item, None);
    assert_eq!(located.target_layer, 0);
    assert_eq!(located.backtrack_array.len(), 1);
    assert_eq!(located.connection_items, Vec::new());
    assert!(ripped.is_empty());
}

#[test]
fn the_fanout_arm_is_reachable_and_ends_on_a_drill() {
    let mut board = probe_board();
    let mut engine = probe_engine(&mut board, 1);
    let mut ctrl = probe_control(&board, 1);
    ctrl.is_fanout = true;

    let counter = Counter::new();
    let result = {
        let mut maze = MazeSearchEngine::get_instance(
            &set_of(&[2]),
            &set_of(&[3]),
            &mut engine,
            &mut board,
            &ctrl,
            &|| counter.check(),
        )
        .expect("MazeSearchEngine.getInstance answers a search engine");
        maze.find_connection(&mut board, &|| counter.check())
            .expect("findConnection answers a result")
    };

    assert!(
        matches!(result.destination_door, ExpandableRef::Drill(_)),
        "MazeSearchEngine.java:361-368 must end a fanout search on a drill; got {:?}",
        result.destination_door
    );

    let mut ripped = BTreeSet::new();
    let mut ripup_costs = BTreeMap::new();
    let locator = FoundConnectionLocator::get_instance(
        Some(&result),
        &ctrl,
        &mut engine,
        &mut board,
        AngleRestriction::NinetyDegree,
        &mut ripped,
        Some(&mut ripup_costs),
    )
    .expect("getInstance answers a locator for a non-null result");

    assert!(
        locator.target_item.is_none(),
        "the fanout arm sets targetItem = null (:126)"
    );
    assert!(
        !locator.connection_items.is_empty(),
        "the two warn branches also leave targetItem null, but they return an EMPTY \
         connectionItems; a non-empty list is what separates the fanout arm from them"
    );
}
