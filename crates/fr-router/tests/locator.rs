//! Plan 6 Task 14: `autoroute.path.FoundConnectionLocator` (FoundConnectionLocator.java:30-569)
//! and its two overrides, `FoundConnectionLocator45Degree` (`:27-356`) and
//! `FoundConnectionLocatorAnyAngle` (`:24-454`).
//!
//! # Where the numbers come from
//!
//! Every literal below is **read off the HEAD jar**. The probe is
//! `scripts/differential/java/probes/P6T14Probe.java`, committed with the exact `javac`/`java`
//! invocation in its header; its whole stdout is committed as
//! `tests/data/p6t14-locator.txt`. Each test names its probe mode and pastes the lines it
//! asserts against.
//!
//! # The fixtures
//!
//! Three boards, all `P6T13Probe`'s so that Task 13's pinned search is the input:
//!
//! * `simple_board` (`P6T13Probe.buildSimple`) — a 2000-unit square with two net-1 pins and
//!   nothing else. The found connection is a single trace on layer 0, and the three regimes give
//!   three different corner lists over the *same* maze result (modes `backtrack`, `locate`).
//! * `probe_board` (`P6T13Probe.build`) — the 8000-unit board whose found connection crosses two
//!   `ExpansionDrill`s, so the walk splits into three `ResultItem`s on layers 0, 1, 0 (mode
//!   `ripup`).
//! * `blocked_board` (`P6T14Probe.buildBlocked`) — `simple_board` plus a net-2 trace across the
//!   channel and `viasAllowed` off, so the search rips the trace up and `backtrack` fills
//!   `rippedItemList`/`ripupCosts` (mode `ripped`).
//!
//! `probe_board` is searched three ways: pin 2 to pin 3 with vias (mode `ripup`, the layer
//! change), pin 2 to pin 3 without them (mode `around`, eight doors the long way round) and pin
//! 3 to pin 2 both ways (mode `reverse`, which is the only one that bends far enough left to
//! reach `FoundConnectionLocatorAnyAngle.leftTurnNextCorner`).

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

// =================================================================================================
// The probe's boards, rebuilt from scratch
// =================================================================================================

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

/// The rules, library and via rule every fixture shares (`P6T13Probe.build`'s prologue).
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

/// `P6T13Probe.buildSimple` — two net-1 pins on a 2000-unit square.
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
    board.insert_pin(1, 0, vec![1], 1, FixedState::Unfixed); // id 2
    board.insert_pin(1, 1, vec![1], 1, FixedState::Unfixed); // id 3
    board
}

/// `simple_board` with the board's own angle restriction set to `FORTYFIVE_DEGREE`, which is the
/// regime the maze search runs in — the locator's `angle` argument is a separate axis and never
/// reaches the search.
///
/// **Why this exists: #165's second half, accepted at plan9-t7t8 (ruling CC).**
/// `addCompleteRoom`'s `null` path now detaches the doors of the room it abandons, and on
/// `buildSimple` **in the free-angle regime only** the sole path from pin 2 to pin 3 ran through
/// that room. Measured at the accept wave, over `{simple, blocked, probe} x {90, 45, free}`:
/// eight of the nine cells still answer a connection and `simple x free-angle` is the one that
/// does not. Measured a second way, by ablation: with the one `detach_all_doors` call commented
/// out, every test below passes again unchanged.
///
/// So the fixture, not the subject, is what moved. The search is re-pointed to a regime where the
/// port and the **jar** still agree byte for byte about this very board — `p6t16`'s mode `plain`
/// matches the jar on every `FORTYFIVE_DEGREE` row, routed state, the 4..7 id burn and the
/// inserted polyline included — and the literals each test reads off that search are re-cut to
/// the port's measured values, with the jar's old value beside each. They are **port-regression
/// pins** from here, not jar-parity pins, and every one says so at its site.
///
/// `FORTYFIVE_DEGREE` and not `NINETY_DEGREE`, and the reason is measured rather than aesthetic.
/// A 90-degree search collapses this board to a **two-door** backtrack chain and a straight
/// `(400,0) (-400,0)` connection *in all three locator regimes*, which would silently empty out
/// [`the_three_regimes_locate_three_different_corner_lists`] (one list, not three), the
/// three-element door chain [`the_backtrack_walk_reproduces_the_jvms_door_chain`] is named for,
/// and the unreached door section the `warn`-mode tests need. The 45-degree search keeps all
/// three: three doors, three genuinely different corner lists, and an orphan section.
///
/// What retires with a record, rather than moving: the free-angle `simple_board` `findConnection`
/// arm. Nothing below asserts it any more, and [`the_free_angle_simple_board_search_finds_nothing`]
/// is the pin that holds the retirement in place, so the day it starts answering again is a
/// failure and not a silence.
fn simple_board_fortyfive() -> Board {
    let mut board = simple_board();
    board.rules.trace_angle_restriction = AngleRestriction::FortyFiveDegree;
    board
}

/// `P6T14Probe.buildBlocked` — `simple_board` plus the net-2 trace across the channel.
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
    ); // id 4
    board
}

/// `P6T13Probe.build` — the 8000-unit board whose connection crosses two drills.
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

    board.insert_pin(1, 0, vec![1], 1, FixedState::Unfixed); // id 2, the start pin
    board.insert_pin(1, 1, vec![1], 1, FixedState::Unfixed); // id 3, the destination pin
    board.insert_pin(2, 0, vec![2], 1, FixedState::Unfixed); // id 4
    board.insert_pin(2, 1, vec![2], 1, FixedState::Unfixed); // id 5
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
    ); // id 6
    board
        .insert_via(
            PadstackId(3),
            Point::new(2500, 2500),
            vec![3],
            1,
            FixedState::Unfixed,
            false,
        )
        .expect("the free via inserts"); // id 7
    board.insert_trace_without_cleaning(
        Polyline::from_points(&[Point::new(2500, 2500), Point::new(2500, 3500)]),
        0,
        30,
        vec![3],
        1,
        FixedState::Unfixed,
    ); // id 8
    board
        .insert_via(
            PadstackId(3),
            Point::new(2500, -2500),
            vec![3],
            1,
            FixedState::Unfixed,
            false,
        )
        .expect("the two-contact via inserts"); // id 9
    board.insert_trace_without_cleaning(
        Polyline::from_points(&[Point::new(2500, -2500), Point::new(2500, -3500)]),
        0,
        30,
        vec![3],
        1,
        FixedState::Unfixed,
    ); // id 10
    board.insert_trace_without_cleaning(
        Polyline::from_points(&[Point::new(2500, -2500), Point::new(3500, -2500)]),
        0,
        30,
        vec![3],
        1,
        FixedState::Unfixed,
    ); // id 11
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

/// A `StopCheck` that never fires.
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

/// What `P6T14Probe.dump` prints, in the same shape.
struct Located {
    locator: FoundConnectionLocator,
    ripped: BTreeSet<ItemId>,
    ripup_costs: BTreeMap<ItemId, i32>,
}

/// `P6T14Probe.locate`: run the search on `board` and locate its result under `angle`.
///
/// The board is rebuilt per regime by the caller, exactly as the probe does — the 45° override
/// calls `ExpansionDoor.getSectionSegments`, which reallocates the door's section array
/// (ExpansionDoor.java:141, `:193-201`) whenever the section count differs from the one the
/// search left, and that would drop the `backtrackDoor`s a second locator run needs.
fn locate(board: &mut Board, angle: AngleRestriction, ripup_no_vias: bool) -> Located {
    locate_between(board, angle, ripup_no_vias, &[2], &[3])
}

/// The same, from an arbitrary start set to an arbitrary destination set — `P6T14Probe.locate`'s
/// two extra parameters, which mode `reverse` uses to search pin 3 to pin 2.
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

/// The three regimes, in the probe's order.
const REGIMES: [AngleRestriction; 3] = [
    AngleRestriction::NinetyDegree,
    AngleRestriction::FortyFiveDegree,
    AngleRestriction::None,
];

/// One `ResultItem` as `P6T14Probe.dump` prints it: its layer and its rounded corner list.
type Item = (usize, Vec<(i32, i32)>);

/// `(layer, corners)` per `ResultItem`, which is what `P6T14Probe.dump` prints.
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

// =================================================================================================
// getInstance — probe mode `share` (FoundConnectionLocator.java:185-207)
// =================================================================================================

/// ```text
/// === mode share ===
/// regime=NINETY_DEGREE class=FoundConnectionLocator45Degree
/// regime=FORTYFIVE_DEGREE class=FoundConnectionLocator45Degree
/// regime=NONE class=FoundConnectionLocatorAnyAngle
/// ```
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

/// The other half of `:196-205`.
#[test]
fn any_angle_does_not() {
    assert_eq!(
        LocatorKind::of(AngleRestriction::None),
        LocatorKind::AnyAngle
    );
}

/// `nullResult=null` — the only reason `getInstance` (`:192-194`) answers null.
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

// =================================================================================================
// calculateAdditionalCorner — probe mode `corner` (:329-404)
// =================================================================================================

/// The whole of `=== mode corner ===`, verbatim: twelve `(from, to)` pairs times both
/// `horizontalFirst` values times the three regimes, printed with `Double.toString`.
#[test]
fn the_additional_corner_of_every_regime_matches_the_jvm() {
    // from=(x,y) to=(x,y) hf=<bool> NINETY_DEGREE=(x,y) FORTYFIVE_DEGREE=(x,y) NONE=(x,y)
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

/// `P6T14Probe.fp`: `"(" + Double.toString(x) + "," + Double.toString(y) + ")"`.
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

// =================================================================================================
// backtrack — probe mode `backtrack` (:225-327)
// =================================================================================================

/// ```text
/// === mode backtrack ===
/// backtrack n=3
///   [0] door=TargetItemExpansionDoor id=97 section=0 nextRoom=CompleteFreeSpaceExpansionRoom nextRoomId=4 nextRoomLayer=0
///   [1] door=ExpansionDoor id=35 section=0 nextRoom=CompleteFreeSpaceExpansionRoom nextRoomId=1 nextRoomLayer=0
///   [2] door=TargetItemExpansionDoor id=63 section=0 nextRoom=null nextRoomId=- nextRoomLayer=-
/// ```
///
/// The last element's `nextRoom` is `null` because the start door is a
/// `TargetItemExpansionDoor`, whose `otherRoom` answers null unconditionally
/// (TargetItemExpansionDoor.java:50-53).
#[test]
fn the_backtrack_walk_reproduces_the_jvms_door_chain() {
    let mut board = simple_board_fortyfive();
    let located = locate(&mut board, AngleRestriction::None, false);
    // Rebuild an engine view for the ids: the locator kept the arena refs, and the ids are what
    // the probe prints.
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

// =================================================================================================
// The single-room connection — probe mode `locate`
// =================================================================================================

/// ```text
/// === mode locate ===
/// === NINETY_DEGREE
/// startItem=2 startLayer=0 targetItem=3 targetLayer=0
/// connectionItems n=1
///   [0] layer=0 corners=5 (400,0) (-132,0) (-132,-132) (-132,0) (-400,0)
/// ripped n=0
/// ```
///
/// The 90° list visits `(-132,0)` **twice**: `calculateNextTrace`'s rounding loop (`:450-459`)
/// drops only *consecutive* duplicates, so the spike back and forth survives.
///
/// **This corner list is a PORT-REGRESSION PIN, not a jar-parity pin, since the plan9-t7t8 accept
/// wave (ruling CC).** The search behind it is [`simple_board_fortyfive`]'s, because #165's second
/// half took the free-angle one away; the jar's transcript above was cut from the free-angle
/// search and no longer describes what this test runs.
///
/// * old (jar-parity, free-angle search): `(400,0) (-132,0) (-132,-132) (-132,0) (-400,0)`
/// * new (port, 45-degree search): `(400,0) (400,-132) (0,-132) (-132,-132) (-400,-132) (-400,0)`
/// * `accepted at plan9-t7t8 (ruling CC)`
///
/// **What the test is for is unchanged and still asserted**: one `ResultItem`, on layer 0, from
/// the pin-2 start to the pin-3 target, with nothing ripped. The `(-132,-132)` corner the jar's
/// rounding loop produces is still in the list, in the same place relative to its neighbours;
/// what the 45-degree search removes is the *doubled* `(-132,0)`, because its own door chain
/// enters the room from below rather than head-on. The doubled-corner claim in the paragraph
/// above therefore no longer has a producer here — [`the_reversed_search_reaches_the_left_turn_corner`]
/// and the `p6t14` `ripup`/`around`/`reverse` modes, all still on the jar's numbers, are where
/// `calculateNextTrace`'s rounding is measured now.
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

/// The same maze result under all three regimes — mode `locate`'s three blocks. This is the
/// fixed case the brief asks for: one search, three different corner lists.
///
/// **All three lists are PORT-REGRESSION PINS since the plan9-t7t8 accept wave (ruling CC)**, for
/// the reason [`simple_board_fortyfive`] gives: the free-angle search the jar's three blocks were
/// cut from is what #165's second half removed, and this is the 45-degree search that replaces
/// it. Row by row, jar-parity value then port value, all `accepted at plan9-t7t8 (ruling CC)`:
///
/// | regime | old (jar, free-angle search) | new (port, 45-degree search) |
/// |---|---|---|
/// | `NINETY_DEGREE` | `(400,0) (-132,0) (-132,-132) (-132,0) (-400,0)` | `(400,0) (400,-132) (0,-132) (-132,-132) (-400,-132) (-400,0)` |
/// | `FORTYFIVE_DEGREE` | `(400,0) (-132,0) (-132,-132) (-264,0) (-400,0)` | `(400,0) (268,-132) (0,-132) (-132,-132) (-268,-132) (-400,0)` |
/// | `NONE` | `(400,0) (-400,0)` | `(400,0) (0,-140) (-400,0)` |
///
/// **The claim the test is named for is what had to survive the re-point, and it does — measured,
/// not assumed.** The three lists are still three *different* lists off one search: 90 degrees
/// turns square, 45 degrees cuts both corners to `268`/`-268`, and the free-angle locator answers
/// the shortest three-corner path. A 90-degree *search* was rejected for this re-point precisely
/// because it collapses all three to the same straight `(400,0) (-400,0)` and would have left the
/// test passing while measuring nothing.
#[test]
fn the_three_regimes_locate_three_different_corner_lists() {
    let expected: [Vec<(i32, i32)>; 3] = [
        // === NINETY_DEGREE
        vec![
            (400, 0),
            (400, -132),
            (0, -132),
            (-132, -132),
            (-400, -132),
            (-400, 0),
        ],
        // === FORTYFIVE_DEGREE
        vec![
            (400, 0),
            (268, -132),
            (0, -132),
            (-132, -132),
            (-268, -132),
            (-400, 0),
        ],
        // === NONE
        vec![(400, 0), (0, -140), (-400, 0)],
    ];
    // Three lists off one search, and they must stay three: a re-point that collapsed them would
    // leave every assertion below green while the test measured nothing.
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

/// **The retirement record for the free-angle `simple_board` `findConnection` arm** (#165's second
/// half, `accepted at plan9-t7t8 (ruling CC)`).
///
/// Until Task 8 this board answered a connection in every regime, and five tests in this file plus
/// one in `inserter.rs` read their literals off the free-angle one. `addCompleteRoom`'s `null`
/// path now detaches the doors of the room it abandons, and on this board that room *was* the
/// free-angle path. The controller took the trade: the reachability #165 exists to remove is worth
/// six probe boards' connection, and on the six real router stems the call is a measured no-op —
/// identical incomplete and violation counts with and without it (Task 8 §6a).
///
/// This test is what stops the retirement from being a silence. It is not an aspiration that the
/// search *should* fail; it is a pin on the one cell of the fixture matrix that changed, so the
/// day it starts answering again — a #165 revert, or anything that hands the abandoned room back
/// its doors — a test goes red and names the reason.
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

// =================================================================================================
// The layer change — probe mode `ripup`
// =================================================================================================

/// ```text
/// === mode ripup ===
/// === NINETY_DEGREE
/// backtrack n=8
///   [0] TargetItemExpansionDoor  [1] ExpansionDrill s=0  [2] ExpansionDrill s=1
///   [3] ExpansionDoor s=1        [4] ExpansionDrill s=1  [5] ExpansionDrill s=0
///   [6] ExpansionDoor s=1        [7] TargetItemExpansionDoor
/// connectionItems n=3
///   [0] layer=0 corners=2 (2000,0) (1180,0)
///   [1] layer=1 corners=7 (1180,0) (1180,-974) (451,-974) (372,-974) (372,-1080) (-130,-1080) (-130,-974)
///   [2] layer=0 corners=5 (-130,-974) (-130,0) (-130,132) (-130,0) (-2000,0)
/// ```
///
/// Three `ResultItem`s and **no** via entry: `connectionItems` holds traces only, and the two
/// vias are `FoundConnectionInserter`'s to derive from the layer changes (Task 15).
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

/// The other two regimes of mode `ripup`: the same three-item split, different geometry.
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

// =================================================================================================
// The long way round — probe mode `around`
// =================================================================================================

/// `P6T13Probe.build` with `viasAllowed` off: the connection has to walk eight `ExpansionDoor`s
/// round the net-2 blocker, which is the only fixture here that reaches the any-angle turn
/// corners (`FoundConnectionLocatorAnyAngle.java:365-408`), its clearance-correction loop
/// (`:284-326`) and the 45-degree `calcHorizontalFirst*` diagonal arms.
///
/// ```text
/// === mode around ===
/// === NINETY_DEGREE
/// backtrack n=9   (target, 7x ExpansionDoor, target)  sections 0,2,2,1,2,2,1,2,0
/// connectionItems n=1
///   [0] layer=0 corners=26 …
/// ```
#[test]
fn the_long_way_round_matches_the_jvm_in_all_three_regimes() {
    let expected: [Vec<(i32, i32)>; 3] = [
        // === NINETY_DEGREE, 26 corners
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
        // === FORTYFIVE_DEGREE, 25 corners
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
        // === NONE, 7 corners
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

/// The backtrack chain of mode `around`'s 90-degree block: eight doors between the two target
/// doors, all `ExpansionDoor`s and none a drill, so the walk emits a single `ResultItem`.
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

// =================================================================================================
// The reversed search — probe mode `reverse`
// =================================================================================================

/// `P6T13Probe.build` searched from pin **3** to pin 2. It is the only fixture here that reaches
/// `FoundConnectionLocatorAnyAngle.leftTurnNextCorner` (`:391-408`); the forward searches bend
/// the other way and reach `rightTurnNextCorner` only.
///
/// `=== mode reverse ===`, the three `noVias=true` blocks. Note `startItem=3`/`targetItem=2`:
/// the locator's "start" is the maze search's *destination set*, because `backtrack` walks from
/// the found destination door back to the seed.
#[test]
fn the_reversed_search_reaches_the_left_turn_corner() {
    let expected: [Vec<(i32, i32)>; 3] = [
        // === noVias=true NINETY_DEGREE, 31 corners
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
        // === noVias=true FORTYFIVE_DEGREE, 31 corners
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
        // === noVias=true NONE, 11 corners
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

/// The `noVias=false` half of mode `reverse`: the same reversed search with vias allowed, so it
/// splits into three `ResultItem`s on layers 0, 1, 0 the other way round from mode `ripup`.
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

// =================================================================================================
// The ripped item list — probe mode `ripped` (:256-265, :316-323)
// =================================================================================================

/// ```text
/// === mode ripped ===
/// === NINETY_DEGREE
/// backtrack n=4
///   [0] door=TargetItemExpansionDoor id=99 section=0 nextRoom=CompleteFreeSpaceExpansionRoom …
///   [1] door=ExpansionDoor id=4282 section=0 nextRoom=ObstacleExpansionRoom nextRoomId=4096 …
///   [2] door=ExpansionDoor id=4127 section=0 nextRoom=CompleteFreeSpaceExpansionRoom …
///   [3] door=TargetItemExpansionDoor id=63 section=0 nextRoom=null
/// connectionItems n=1
///   [0] layer=0 corners=6 (400,0) (130,0) (0,0) (-130,0) (-262,0) (-400,0)
/// ripped n=1 4:1
/// ```
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

/// The other two regimes of mode `ripped` — the ripped set is regime-independent, because
/// `backtrack` (`:225-327`) runs before any corner is computed.
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

/// `backtrack`'s `ripupCosts` parameter is Java's nullable `Map`
/// (`:260`, `:319`): a `None` here must not stop the ripped-item set from filling.
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

// =================================================================================================
// The empty connection-item list (:101-111, :130-135)
// =================================================================================================

/// Probe mode `warn`, `=== unexpectedDestinationDoor`:
/// ```text
/// middle door=ExpansionDoor section=0
/// startItem=2 startLayer=0 targetItem=null targetLayer=0 backtrack n=2 connectionItems=n=0
/// ```
///
/// `:130-135` — the destination door is neither a `TargetItemExpansionDoor` nor an
/// `ExpansionDrill`, so Java warns and returns with `startItem`/`startLayer` **already set** at
/// `:112-114` and `targetItem`/`targetLayer` left at their defaults. `connectionItems` is the
/// empty list `:101` assigned, **not** null — `docs/java-quirks.md` #180.
///
/// The forged `MazeResult` is not synthetic geometry: it names the live `ExpansionDoor` the real
/// walk found at `backtrack_array[1]`, with its own section number, so `backtrack` walks the same
/// live `backtrackDoor` chain and `start_info.door` is still the start target door.
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

/// Probe mode `warn`, `=== orphanDoor`:
/// ```text
/// === orphanDoor type=ExpansionDoor section=0
/// startItem=null startLayer=0 targetItem=null targetLayer=0 backtrack n=1 connectionItems=n=0
/// ```
///
/// `:103-111` — a door section the search never reached has a null `backtrackDoor`, so
/// `backtrack` (`:271-276`) breaks after one element and `startInfo.door` is an `ExpansionDoor`.
/// Java warns and leaves **every** field at its default, `connectionItems` included — again the
/// empty list, not null (`docs/java-quirks.md` #180).
///
/// The scan walks `completeExpansionRooms` in list order and each room's doors in list order,
/// exactly as the probe does, so both sides pick the same door.
#[test]
fn a_start_door_that_is_not_a_target_door_yields_an_all_default_locator() {
    // `blocked_board`, not `simple_board`, since the plan9-t7t8 accept wave (ruling CC).
    // #165's second half took the free-angle `simple_board` search away, and the 45-degree search
    // [`simple_board_fortyfive`] replaces it with reaches **every** door section, so there is no
    // orphan left on that board to forge a result from. Measured at the wave over the three
    // fixtures: `simple` at 45 degrees has none, `blocked` and `probe` both leave one at section
    // 0. `blocked_board` is `P6T14Probe.buildBlocked`, the jar's own fixture, it still finds its
    // connection free-angle, and the section number the test asserts is unchanged — so what moves
    // here is the board the orphan is picked from and nothing the test claims.
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

// =================================================================================================
// plan-6 §10.3's first coverage obligation, as a standing assertion (Task 17 fix round, SF5)
// =================================================================================================

/// **The `FoundConnectionLocator` fanout arm (`:124-129`), asserted rather than counted.**
///
/// Plan 6 filed this arm as a coverage obligation it could not discharge: it fires only when the
/// maze search's *destination* door is an `ExpansionDrill`, which needs `ctrl.isFanout`, which
/// only `BatchFanout` sets — `autoroute/pipeline`'s, i.e. Plan 7's. Plan 7 Tasks 11 and 12 built
/// it, and Task 17 measured the arm entered **32 times** by `tests/batch_parity.rs`'s
/// `the_ci_stems_climb_the_whole_ladder` with a temporary counter, then removed the counter.
///
/// This is the same statement without an instrument, and **directed rather than corpus-derived**,
/// which is the more durable form: a corpus assertion goes cold when the corpus changes, and the
/// SES byte-parity ladder is already the guard that a corpus board still reaches the arm *and*
/// gets the jar's answer there. What this test adds is that the arm cannot be deleted or made
/// unreachable in the port without a red test.
///
/// **Why it is exact, not inferential.** Two discriminants, both of them the arm's own:
///
/// * `MazeSearchEngine`'s "algorithm completed after the first drill" exit
///   (`MazeSearchEngine.java:361-368`, `search.rs`) sets `destination_door` to an
///   `ExpandableRef::Drill` **only** under `ctrl.is_fanout`. The `match` in
///   `FoundConnectionLocator::get_instance` dispatches on exactly that value, so a `Drill`
///   destination *is* the arm being taken.
/// * The arm is the only path that leaves `target_item == None` with a **non-empty**
///   `connection_items`: the other two `None` producers are the `:103-111` and `:130-135` warn
///   branches, and both `return` early with an empty list (see the field's own doc).
///
/// The board is the same `probe_board()` the whole file uses, and the *only* change from
/// [`a_layer_change_yields_two_traces_and_no_via_entry`] — whose backtrack array is
/// `target, drill, drill, door, drill, drill, door, target`, i.e. two drill-after-drill pairs the
/// non-fanout search walks straight past — is `ctrl.is_fanout = true`.
#[test]
fn the_fanout_arm_is_reachable_and_ends_on_a_drill() {
    let mut board = probe_board();
    let mut engine = probe_engine(&mut board, 1);
    let mut ctrl = probe_control(&board, 1);
    // `RoutingBoardExt::fanout` (`RoutingBoard.java:1024`) is the only producer of this in the
    // port; setting it here is what makes the search a fanout search.
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

    // Discriminant 1: the search stopped on a drill, which only `is_fanout` allows.
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

    // Discriminant 2: `target_item == None` with a non-empty `connection_items` is the fanout
    // arm and nothing else.
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
