//! Plan 6 Task 16: `AutorouteEngine.autorouteConnection` (AutorouteEngine.java:130-280),
//! `describeConnection` (`:282-287`) and steps 1-5 of `AutorouteConnectionRouter.route`
//! (AutorouteConnectionRouter.java:30-100) — the top of Plan 6's scope and the surface Plan 7
//! consumes.
//!
//! # Where the numbers come from
//!
//! Every expectation below is **read off the HEAD jar**, not off this port. The probe is
//! `scripts/differential/java/probes/P6T16Probe.java`, committed with the exact `javac`/`java`
//! invocation in its header, and its whole stdout is committed as
//! `tests/data/p6t16-autoroute-connection.txt`. Each test regenerates its mode's rows and
//! compares them line by line, so a board that differs by one item, one id, one corner, one
//! ripped id or one word of a `FAILED` message fails.
//!
//! The brief asks for `routing_the_first_connection_of_rpi_splitter_matches_java` with literals
//! from `p6t1`. `p6t1` is **Task 17's** driver and does not exist yet, so ruling 1(a)+(b) is
//! pinned here against `P6T16Probe` instead: the same state, the same ripped-item id set and the
//! same inserted geometry, on four hand-built boards and in all three angle regimes. Task 17
//! adds the fixture-scale version on `Issue143-rpi_splitter.dsn`.
//!
//! # The fixtures
//!
//! `P6T13Probe.buildSimple`'s two-pin board and `P6T13Probe.build`'s 8000-unit board, both
//! through `P6T14Probe`; `P6T14Probe.buildBlocked` (the blocker 100 units short of the outline,
//! which the search rips); `P6T16Probe.buildSealed` (the blocker taken to the outline, which
//! makes `MazeSearchEngine.getInstance` answer null); and `P6T16Probe`'s power-plane variant of
//! `buildSimple`, the only shape that reaches `:221-228`.

#![allow(clippy::too_many_lines, clippy::too_many_arguments)]

use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};

use fr_board::ids::{ItemId, PadstackId, ViaInfoId, ViaRuleId};
use fr_board::prelude::*;
use fr_board::rules::{ViaInfo, ViaRule};
use fr_geometry::{
    Area, IntBox, IntOctagon, IntPoint, IntVector, Point, Polyline, Shape, TileShape,
};
use fr_router::autoroute::maze::AutorouteControl;
use fr_router::autoroute::maze::engine::AutorouteEngine;
use fr_router::autoroute::maze::engine::describe_connection;
use fr_router::board_ext::RoutingBoardExt;
use fr_router::{AutorouteAttemptResult, AutorouteAttemptState, route_connection};
use fr_settings::RouterSettings;

// =================================================================================================
// The probe's boards, rebuilt from scratch (`P6T13Probe`'s, through `P6T14Probe`)
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
///
/// `front_is_signal` is `P6T16Probe`'s `inactive` mode: Java replaces
/// `board.layerStructure.layers[0]` with a dedicated power plane straight after `buildSimple`,
/// and the port builds the same structure up front, because `fr-board` has no mutable accessor
/// for it. The two are equivalent: the only value computed from `isSignal` in between is
/// `NetClass.activeRoutingLayer[0]`, and `AutorouteControl.java:151-158` forces `layerActive[0]`
/// off for a non-signal layer before `:225-227` ever consults it.
fn base_board(bounds: IntBox, front_is_signal: bool) -> Board {
    let layers = || {
        LayerStructure::new(vec![
            Layer::new("front", front_is_signal),
            Layer::new("back", true),
        ])
    };
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
    via_rule.append_via(ViaInfoId(0));
    rules.via_rules.push(via_rule);
    let default_class = rules.get_default_net_class();
    rules
        .net_classes
        .get_mut(default_class)
        .set_via_rule(Some(ViaRuleId(0)));

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
fn simple_board_with(front_is_signal: bool) -> Board {
    let mut board = base_board(SIMPLE_BOUNDING_BOX, front_is_signal);
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

fn simple_board() -> Board {
    simple_board_with(true)
}

/// `P6T14Probe.buildBlocked` — `simple_board` plus a net-2 blocker that stops 100 units short of
/// the outline at each end, so the search can rip it rather than fail to seed.
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

/// `P6T16Probe.buildSealed` — the same blocker taken all the way to the outline, which makes
/// `MazeSearchEngine.getInstance` answer null (`init` cannot seed the destination).
fn sealed_board() -> Board {
    let mut board = simple_board();
    let default_class = board.rules.get_default_net_class();
    board.rules.nets.add("N2", 1, false, default_class);
    board.insert_trace_without_cleaning(
        Polyline::from_points(&[Point::new(0, -1000), Point::new(0, 1000)]),
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
    let mut board = base_board(BOUNDING_BOX, true);
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

// =================================================================================================
// The probe's harness
// =================================================================================================

fn probe_settings(board: &Board) -> RouterSettings {
    let mut settings = RouterSettings::new();
    settings.set_layer_count(board.get_layer_count());
    settings.apply_board_specific_optimizations(board);
    settings
}

/// `P6T16Probe.control`.
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

/// `P6T16Probe.Stop` — a stop check whose answer is fixed, plus its call counter.
struct Stop {
    calls: Cell<u32>,
    stopped: bool,
    /// `P6T16Probe.StopAfter`: answer false `limit` times and true from then on. `u32::MAX`
    /// disables it.
    limit: u32,
}

impl Stop {
    fn never() -> Stop {
        Stop {
            calls: Cell::new(0),
            stopped: false,
            limit: u32::MAX,
        }
    }

    fn always() -> Stop {
        Stop {
            calls: Cell::new(0),
            stopped: true,
            limit: u32::MAX,
        }
    }

    fn after(limit: u32) -> Stop {
        Stop {
            calls: Cell::new(0),
            stopped: false,
            limit,
        }
    }

    fn check(&self) -> bool {
        let calls = self.calls.get();
        self.calls.set(calls + 1);
        self.stopped || calls >= self.limit
    }

    fn calls(&self) -> u32 {
        self.calls.get()
    }
}

fn set_of(ids: &[u32]) -> BTreeSet<ItemId> {
    ids.iter().map(|id| ItemId(*id)).collect()
}

/// `P6T16Probe.rippedOf` — Java's `TreeSet<Item>` prints **descending** by id.
fn ripped_of(ripped: &BTreeSet<ItemId>) -> String {
    let ids: Vec<String> = ripped.iter().rev().map(|id| id.0.to_string()).collect();
    format!("ripped n={} [{}]", ripped.len(), ids.join(","))
}

/// `P6T16Probe.ripupCostsOf` — likewise a `TreeMap<Item,Integer>`, descending by id.
fn ripup_costs_of(costs: &BTreeMap<ItemId, i32>) -> String {
    let entries: Vec<String> = costs
        .iter()
        .rev()
        .map(|(id, cost)| format!("{}={cost}", id.0))
        .collect();
    format!("ripupCosts n={} [{}]", costs.len(), entries.join(","))
}

/// `P6T15Probe.ln`, reused by `P6T16Probe.boardDump`.
fn ln(line: &fr_geometry::Line) -> String {
    format!("({},{})->({},{})", line.a.x, line.a.y, line.b.x, line.b.y)
}

/// `P6T15Probe.pt`.
fn pt(polyline: &Polyline, no: usize) -> String {
    match polyline.corner(no) {
        Some(Point::Int(p)) => format!("({},{})", p.x, p.y),
        _ => {
            let f = polyline.corner_approx(no).expect("no is below cornerCount");
            format!(
                "~({},{})",
                fr_dsn::format::double::java_double_to_string(f.x),
                fr_dsn::format::double::java_double_to_string(f.y)
            )
        }
    }
}

/// `P6T15Probe.poly`.
fn poly(polyline: &Polyline) -> String {
    let lines: Vec<String> = polyline.lines().iter().map(ln).collect();
    let corners: Vec<String> = (0..polyline.corner_count())
        .map(|i| pt(polyline, i))
        .collect();
    format!(
        "n={} lines=[{}] corners=[{}]",
        polyline.lines().len(),
        lines.join(","),
        corners.join(",")
    )
}

/// `P6T15Probe.pointOf`.
fn point_of(point: &Point) -> String {
    let rounded = point.to_float().round();
    format!("({},{})", rounded.x, rounded.y)
}

/// `P6T15Probe.nets`.
fn nets(net_nos: &[i32]) -> String {
    let inner: Vec<String> = net_nos.iter().map(i32::to_string).collect();
    format!("[{}]", inner.join(","))
}

fn type_name(item: &Item) -> &'static str {
    match item {
        Item::Trace(_) => "PolylineTrace",
        Item::Via(_) => "Via",
        Item::Pin(_) => "Pin",
        Item::ObstacleArea(_) => "ObstacleArea",
        Item::ConductionArea(_) => "ConductionArea",
        Item::ViaObstacleArea(_) => "ViaObstacleArea",
        Item::ComponentObstacleArea(_) => "ComponentObstacleArea",
        Item::ComponentOutline(_) => "ComponentOutline",
        Item::BoardOutline(_) => "BoardOutline",
    }
}

/// `P6T15Probe.boardDump` — `maxId=` plus one line per item in `getItems()` order (descending id,
/// quirk #63).
fn board_dump(board: &Board) -> Vec<String> {
    let ctx = board.ctx();
    let mut out = vec![format!(
        "    maxId={}",
        board.communication.id_gen.max_generated_id()
    )];
    for item in board.get_items() {
        let mut line = format!(
            "    item id={} type={} nets={} cl={}",
            item.id().0,
            type_name(item),
            nets(item.net_nos()),
            item.clearance_class()
        );
        match item {
            Item::Trace(trace) => line.push_str(&format!(
                " layer={} hw={} {}",
                trace.get_layer(),
                trace.get_half_width(),
                poly(trace.polyline())
            )),
            Item::Via(via) => {
                let padstack = board
                    .library
                    .padstacks
                    .get(via.get_padstack_id())
                    .expect("the via's padstack is in the library");
                line.push_str(&format!(
                    " center={} padstack={} layers={}..{}",
                    point_of(&via.get_center()),
                    padstack.name,
                    via.first_layer(&ctx),
                    via.last_layer(&ctx)
                ));
            }
            Item::Pin(pin) => {
                line.push_str(&format!(" center={}", point_of(&pin.get_center(&ctx))));
            }
            _ => {}
        }
        out.push(line);
    }
    out
}

// =================================================================================================
// The transcript
// =================================================================================================

const T16: &str = include_str!("data/p6t16-autoroute-connection.txt");

/// The rows of one `=== mode <mode> ===` section of the Task 16 transcript.
fn t16_section(mode: &str) -> Vec<&'static str> {
    let header = format!("=== mode {mode} ===");
    let mut rows = Vec::new();
    let mut inside = false;
    for line in T16.lines() {
        if line.starts_with("=== mode ") {
            inside = line == header;
            continue;
        }
        if inside {
            rows.push(line.trim_end());
        }
    }
    assert!(!rows.is_empty(), "transcript section `{mode}` is empty");
    rows
}

/// Compares the rows this port produces with the JVM's, collecting **every** difference rather
/// than stopping at the first.
fn assert_rows_match(mode: &str, actual: &[String]) {
    let expected = t16_section(mode);
    let mut diffs = Vec::new();
    for i in 0..expected.len().max(actual.len()) {
        let want = expected.get(i).copied().unwrap_or("<missing>");
        let got = actual
            .get(i)
            .map(|row| row.trim_end())
            .unwrap_or("<missing>");
        if want != got {
            diffs.push(format!("row {i}\n  jvm:  {want}\n  rust: {got}"));
        }
    }
    assert!(
        diffs.is_empty(),
        "mode `{mode}`: {} of {} rows differ\n{}",
        diffs.len(),
        expected.len().max(actual.len()),
        diffs
            .iter()
            .take(12)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// The three regimes, in the probe's order.
const REGIMES: [AngleRestriction; 3] = [
    AngleRestriction::NinetyDegree,
    AngleRestriction::FortyFiveDegree,
    AngleRestriction::None,
];

fn regime_name(angle: AngleRestriction) -> &'static str {
    match angle {
        AngleRestriction::NinetyDegree => "NINETY_DEGREE",
        AngleRestriction::FortyFiveDegree => "FORTYFIVE_DEGREE",
        AngleRestriction::None => "NONE",
    }
}

/// `P6T16Probe.run` — one `autorouteConnection`, printed the way every mode prints it.
fn run(
    board: &mut Board,
    engine: &mut AutorouteEngine,
    ctrl: &AutorouteControl,
    start: &[u32],
    dest: &[u32],
    ripped: &mut BTreeSet<ItemId>,
    ripup_costs: &mut BTreeMap<ItemId, i32>,
    stop: &Stop,
) -> Vec<String> {
    let before = board.communication.id_gen.max_generated_id();
    let result = engine.autoroute_connection(
        board,
        &set_of(start),
        &set_of(dest),
        ctrl,
        ripped,
        Some(ripup_costs),
        &|| stop.check(),
    );
    let mut out = vec![
        format!("  result={result}"),
        format!("  state={}", result.state),
        format!("  {}", ripped_of(ripped)),
        format!("  {}", ripup_costs_of(ripup_costs)),
        format!("  maxIdBefore={}", before.0),
    ];
    out.extend(board_dump(board));
    out
}

/// `P6T16Probe.oneRegime`.
fn one_regime(
    board: &mut Board,
    regime: AngleRestriction,
    no_vias: bool,
    ripup_allowed: bool,
    maintain: bool,
    stop: &Stop,
) -> (Vec<String>, AutorouteEngine) {
    board.rules.trace_angle_restriction = regime;
    let mut engine = board.init_autoroute(None, 1, 1, None, maintain);
    let mut ctrl = probe_control(board, 1);
    if no_vias {
        ctrl.vias_allowed = false;
    }
    if ripup_allowed {
        ctrl.ripup_allowed = true;
        ctrl.ripup_costs = 1000;
    }
    let mut ripped = BTreeSet::new();
    let mut ripup_costs = BTreeMap::new();
    let rows = run(
        board,
        &mut engine,
        &ctrl,
        &[2],
        &[3],
        &mut ripped,
        &mut ripup_costs,
        stop,
    );
    (rows, engine)
}

/// Every regime of one mode, over a fresh board each time.
fn all_regimes(mode: &str, build: fn() -> Board, no_vias: bool, ripup_allowed: bool) {
    let mut rows = Vec::new();
    for regime in REGIMES {
        let mut board = build();
        rows.push(format!("=== {}", regime_name(regime)));
        let stop = Stop::never();
        let (mode_rows, _engine) =
            one_regime(&mut board, regime, no_vias, ripup_allowed, false, &stop);
        rows.extend(mode_rows);
    }
    assert_rows_match(mode, &rows);
}

// =================================================================================================
// Ruling 1(a)+(b): the state, the ripped set and the inserted geometry
// =================================================================================================

/// Probe mode `plain`: `ROUTED`, an empty ripped set and one inserted trace, in all three
/// regimes. The 45-degree run burns ids 4..7 inside `insertForcedTracePolyline` before the
/// combined trace lands on 8, and that id burn is part of the comparison.
#[test]
fn a_plain_connection_routes_and_inserts_javas_trace() {
    all_regimes("plain", simple_board, false, false);
}

/// Probe mode `via`: the connection crosses two `ExpansionDrill`s, so the insert lays down a via
/// at the layer change (`FoundConnectionInserter.java:66`) — ruling 1(b)'s via case.
#[test]
fn a_layer_changing_connection_routes_through_javas_via() {
    all_regimes("via", probe_board, false, false);
}

/// Probe mode `ripup`: with vias forbidden and ripup on, the search rips the net-2 blocker, so
/// `:238-263` runs with a non-empty `rippedItemList` — the ripped ids, the per-item ripup costs
/// and the item the board loses are all compared. The 90-degree regime is the control: it finds
/// no connection at all and rips nothing.
#[test]
fn a_ripping_connection_deletes_javas_items_and_reports_javas_ripped_set() {
    all_regimes("ripup", blocked_board, true, true);
}

// =================================================================================================
// One test per early return, asserting both the state and the message
// =================================================================================================

/// Probe mode `nomaze` (`:145-151`): the blocker taken to the outline makes
/// `MazeSearchEngine.getInstance` answer null in the free-angle regime, and leaves the other two
/// with no path at all.
///
/// The room and tree-leaf counts afterwards are the point of the extra rows. `:145-151` returns
/// **before** the cleanup of `:198-205`, where every later early return runs it — an asymmetry
/// that would leak complete expansion rooms and their leaves in the compensated autoroute tree
/// into the next connection. The JVM says it is **latent**: `getInstance` answers null only when
/// `MazeSearchEngine.init` fails, which is before any room has been completed, so
/// `completeExpansionRooms` is empty either way and the tree holds only the four board items.
/// That is why it earns no quirk row — but the port transcribes the order anyway, and
/// `cleanup_runs_before_every_early_return` is what holds it there.
#[test]
fn a_maze_that_cannot_be_built_fails_with_javas_message() {
    let mut rows = Vec::new();
    for regime in REGIMES {
        let mut board = sealed_board();
        rows.push(format!("=== {}", regime_name(regime)));
        let stop = Stop::never();
        let (mode_rows, engine) = one_regime(&mut board, regime, true, false, false, &stop);
        rows.extend(mode_rows);
        rows.extend(dump_rooms(&engine, &board, "  after-connection"));
        rows.push(format!("  treeSize={}", tree_size(&engine, &board)));
    }
    assert_rows_match("nomaze", &rows);
}

/// `P6T16Probe.treeSize` — the leaf count of the engine's compensated autoroute tree.
fn tree_size(engine: &AutorouteEngine, board: &Board) -> usize {
    board
        .trees
        .trees()
        .find(|tree| tree.id() == engine.tree)
        .expect("the engine's tree is in the board")
        .size()
}

/// Probe mode `nopath` (`:207-213`): the maze builds but `findConnection` answers null.
#[test]
fn no_connection_found_fails_with_javas_message() {
    all_regimes("nopath", blocked_board, true, false);
}

/// Probe mode `inactive` (`:221-228`). A plain `layerActive[0] = false` cannot reach it: with a
/// **signal** layer `MazeSearchEngine.expandToRoomDoors:396-399` returns before expanding
/// anything and `:207` fires instead. The lever is a dedicated **power plane**, whose
/// `layerActive` `AutorouteControl.java:151-158` forces off while `:397-399`'s guard does not
/// fire — so the located `startLayer` is a disabled one.
#[test]
fn an_inactive_layer_fails_with_javas_message() {
    let mut rows = Vec::new();
    for regime in REGIMES {
        let mut board = simple_board_with(false);
        rows.push(format!("=== {}", regime_name(regime)));
        board.rules.trace_angle_restriction = regime;
        let mut engine = board.init_autoroute(None, 1, 1, None, false);
        let ctrl = probe_control(&board, 1);
        rows.push(format!(
            "  layerActive=[{},{}]",
            ctrl.layer_active[0], ctrl.layer_active[1]
        ));
        let stop = Stop::never();
        let mut ripped = BTreeSet::new();
        let mut ripup_costs = BTreeMap::new();
        rows.extend(run(
            &mut board,
            &mut engine,
            &ctrl,
            &[2],
            &[3],
            &mut ripped,
            &mut ripup_costs,
            &stop,
        ));
    }
    assert_rows_match("inactive", &rows);
}

/// `:230-235`'s `SKIPPED` is **dead code** — quirk #180. `FoundConnectionLocator.connectionItems`
/// is a `final` field assigned at `FoundConnectionLocator.java:101`, before both of the
/// constructor's early returns, so the `== null` test cannot hold; the port's type is a `Vec`
/// and cannot express the null. Nothing in `fr-router` may produce `SKIPPED`.
#[test]
fn skipped_has_no_producer_because_connection_items_is_never_null() {
    let source = include_str!("../src/autoroute/maze/engine.rs");
    assert!(
        !source.contains("AutorouteAttemptState::Skipped"),
        "AutorouteEngine.autorouteConnection:230-235 is unreachable (quirk #180); a `Skipped` \
         here would be a state Java cannot produce"
    );
    // And the state itself still exists, because Task 1 ported all nine constants.
    assert_eq!(AutorouteAttemptState::Skipped.name(), "SKIPPED");
}

// =================================================================================================
// The cleanup order (`:198-205`)
// =================================================================================================

/// `:201-205` runs **before** every early return from `:207` on, and **after** the `:145-151`
/// one. Getting that wrong leaks rooms into the next connection and is invisible until a later
/// fixture routes differently, so this measures both halves directly:
///
/// * `maintain_database = false` — `clear()` empties the complete-room list;
/// * `maintain_database = true` — `resetAllDoors()` keeps it, which is what probe mode
///   `maintain` then shows carrying into the next `initConnection`;
/// * the `:145-151` return — the rooms `initConnection` left are untouched, because the cleanup
///   is three statements further down than the return.
#[test]
fn cleanup_runs_before_every_early_return() {
    // `maintain_database = false`: the list is empty afterwards.
    let mut board = blocked_board();
    board.rules.trace_angle_restriction = AngleRestriction::None;
    let stop = Stop::never();
    let (_, engine) = one_regime(
        &mut board,
        AngleRestriction::None,
        true,
        false,
        false,
        &stop,
    );
    assert!(
        engine.complete_expansion_rooms().is_empty(),
        "clear() empties completeExpansionRooms before the `:207-213` return"
    );

    // `maintain_database = true`: the list survives, with every door reset.
    let mut board = blocked_board();
    let stop = Stop::never();
    let (_, engine) = one_regime(&mut board, AngleRestriction::None, true, false, true, &stop);
    assert!(
        !engine.complete_expansion_rooms().is_empty(),
        "resetAllDoors() keeps completeExpansionRooms"
    );
    for room in engine.complete_expansion_rooms() {
        let room = engine
            .rooms
            .complete_room(*room)
            .expect("a listed room is in the arena");
        for door in room.get_doors() {
            let door = engine.rooms.door(*door).expect("a listed door is live");
            let mut section_no = 0;
            while let Some(section) = door.get_maze_search_element(section_no) {
                assert!(
                    !section.is_occupied,
                    "ExpansionDoor.reset (ExpansionDoor.java:174-182) leaves every section \
                     unoccupied"
                );
                assert!(section.backtrack_door.is_none());
                section_no += 1;
            }
        }
    }

    // The `:145-151` return happens *before* the cleanup, so the rooms `initConnection` built
    // are still there — with `maintain_database = false`, where `clear()` would have dropped
    // them.
    let mut board = sealed_board();
    board.rules.trace_angle_restriction = AngleRestriction::None;
    let mut engine = board.init_autoroute(None, 1, 1, None, false);
    let ctrl = probe_control(&board, 1);
    let stop = Stop::never();
    let mut ripped = BTreeSet::new();
    let result = engine.autoroute_connection(
        &mut board,
        &set_of(&[2]),
        &set_of(&[3]),
        &ctrl,
        &mut ripped,
        None,
        &|| stop.check(),
    );
    assert_eq!(result.state, AutorouteAttemptState::Failed);
    assert!(
        result
            .details()
            .ends_with("because the maze search algorithm could not be created."),
        "got {result}"
    );
}

/// Probe mode `maintain`: with `maintainDatabase = true` the complete-room list, the target-door
/// index and every reset door carry into the next `initConnection` on the **same** net, and are
/// dropped on a different one. The `roomsWithTargetItems` row is
/// `getRoomsWithTargetItems`'s `SortedSet<CompleteFreeSpaceExpansionRoom>`, whose comparator is
/// `other.id - this.id` — so the port's `BTreeSet<RoomId>` is walked `.rev()` (Task 11's
/// carry-over, only observable here).
///
/// The connection deliberately FAILs. A successful one would insert items, and every
/// `BoardItemRepository`/`PolylineTrace` mutation runs `RoutingBoard.additionalUpdateAfterChange`
/// on the JVM while `fr-board` cannot (see the crate README's Task 16 section), so with
/// `maintainDatabase = true` the two room databases would part company for a reason that has
/// nothing to do with this method.
#[test]
fn the_room_database_carries_over_when_maintain_database_is_set() {
    let mut rows = Vec::new();
    for regime in REGIMES {
        let mut board = blocked_board();
        rows.push(format!("=== {}", regime_name(regime)));
        let stop = Stop::never();
        let (mode_rows, mut engine) = one_regime(&mut board, regime, true, false, true, &stop);
        rows.extend(mode_rows);
        rows.extend(dump_rooms(&engine, &board, "  after-connection"));
        engine.init_connection(&mut board, 1, None);
        rows.extend(dump_rooms(&engine, &board, "  after-same-net"));
        engine.init_connection(&mut board, 2, None);
        rows.extend(dump_rooms(&engine, &board, "  after-other-net"));
    }
    assert_rows_match("maintain", &rows);
}

/// `P6T16Probe.dumpRooms`.
fn dump_rooms(engine: &AutorouteEngine, _board: &Board, label: &str) -> Vec<String> {
    let listed = engine.complete_expansion_rooms();
    let ids: Vec<String> = listed
        .iter()
        .map(|room| {
            engine
                .rooms
                .complete_room(*room)
                .expect("a listed room is in the arena")
                .get_id()
                .to_string()
        })
        .collect();
    let with_targets = engine.rooms_with_target_items(&set_of(&[2, 3]));
    // `getRoomsWithTargetItems` answers a `TreeSet<CompleteFreeSpaceExpansionRoom>` whose
    // comparator is `other.id - this.id` — descending (plan-2 ruling 14's third rule).
    let target_ids: Vec<String> = with_targets
        .iter()
        .rev()
        .map(|room| {
            engine
                .rooms
                .complete_room(*room)
                .expect("a listed room is in the arena")
                .get_id()
                .to_string()
        })
        .collect();
    vec![
        format!(
            "{label} completeRooms n={} [{}]",
            listed.len(),
            ids.join(",")
        ),
        format!(
            "{label} roomsWithTargetItems n={} [{}]",
            with_targets.len(),
            target_ids.join(",")
        ),
    ]
}

// =================================================================================================
// Ruling 7's recovery boundaries
// =================================================================================================

/// Probe mode `stop`: a stop check that is already tripped bails inside `MazeSearchEngine.init`,
/// so `getInstance` answers null and `:145-151` fires. This is ruling 6's cancellation reaching
/// `autorouteConnection`'s first early return.
#[test]
fn an_already_stopped_run_fails_before_the_maze_is_built() {
    let mut rows = Vec::new();
    for regime in REGIMES {
        let mut board = simple_board();
        rows.push(format!("=== {}", regime_name(regime)));
        let stop = Stop::always();
        let (mode_rows, _engine) = one_regime(&mut board, regime, false, false, false, &stop);
        rows.extend(mode_rows);
    }
    assert_rows_match("stop", &rows);
}

/// Probe mode `stopafter` — **ruling 7's boundary #3 (`:157`), reached naturally**.
///
/// A stop check that answers false a fixed number of times first lets `getInstance` finish and
/// then trips inside the pop loop. On the JVM two of the nine rows do more than that:
/// `PolylineArea.splitToConvex` answers `null` when the flag trips inside it, and
/// `DrillPage.getDrills:108` then dereferences it — a `NullPointerException` that only
/// `AutorouteEngine.java:157` catches, degrading to `:207-213`'s FAILED. The port panics at the
/// same place (`DrillPage::get_drills`) and boundary #3 turns it back into the same value.
///
/// The `stopCalls` row is the second assertion: ruling 6 fixes the six sites that may consult
/// the stop flag, so a port that checks a seventh — or skips one — lands on a different count
/// and on a different result.
///
/// Every limit is chosen so that the connection fails **before** the insert. A limit that lets
/// the search finish (14 checks on this board) routes on the JVM but not here: plan-3 ruling F
/// and plan-6 ruling 6 deliberately put a stop check inside `BasicBoard.splitTraces` and
/// `normalizeTraces`, which Java has not got, so an already-tripped flag aborts the insert with
/// `BoardError::Stopped` — a value Java has no counterpart for. Measured with `limit = 20`,
/// where the JVM answers `ROUTED` after 14 checks and the port answers
/// `AutorouteConnectionRouter.route:155-158`'s bare `FAILED`; recorded in the crate README.
#[test]
fn a_stop_inside_the_pop_loop_degrades_through_boundary_three() {
    let mut rows = Vec::new();
    for regime in REGIMES {
        for limit in [8u32, 12, 13] {
            let mut board = simple_board();
            rows.push(format!("=== {} limit={limit}", regime_name(regime)));
            let stop = Stop::after(limit);
            let (mode_rows, _engine) = one_regime(&mut board, regime, false, false, false, &stop);
            rows.extend(mode_rows);
            rows.push(format!("  stopCalls={}", stop.calls()));
        }
    }
    assert_rows_match("stopafter", &rows);
}

/// **Ruling 7's boundary #4 (`:190`)**, and the only way to reach `:215-219`'s message-less
/// FAILED: `FoundConnectionLocator::get_instance` answers `None` for exactly one input, a null
/// maze result, which `:180` has already excluded.
///
/// The JVM lever is probe mode `locatorfail`: an **unmodifiable** `rippedItemList`, which makes
/// `backtrack:318`'s `add` throw. A `BTreeSet` cannot refuse an insert, so the port injects the
/// panic instead and asserts the probe's value — the exact message, for the two regimes whose
/// search actually rips (the 90-degree one finds no connection at all and stops at `:207`).
#[test]
fn a_panicking_locator_degrades_to_javas_message_less_failure() {
    let expected: Vec<&str> = t16_section("locatorfail")
        .into_iter()
        .filter(|row| row.starts_with("  result="))
        .collect();
    assert_eq!(expected.len(), 3, "one result row per regime");

    let mut actual = Vec::new();
    for (i, regime) in REGIMES.into_iter().enumerate() {
        let mut board = blocked_board();
        board.rules.trace_angle_restriction = regime;
        let mut engine = board.init_autoroute(None, 1, 1, None, false);
        let mut ctrl = probe_control(&board, 1);
        ctrl.vias_allowed = false;
        ctrl.ripup_allowed = true;
        ctrl.ripup_costs = 1000;
        let stop = Stop::never();
        let mut ripped = BTreeSet::new();
        // The 90-degree row never reaches the locator, so it must not be injected: its FAILED is
        // `:207-213`'s, and the transcript says so.
        let inject = i != 0;
        let result = engine.autoroute_connection_with_forced_locator_failure(
            &mut board,
            &set_of(&[2]),
            &set_of(&[3]),
            &ctrl,
            &mut ripped,
            None,
            &|| stop.check(),
            inject,
        );
        actual.push(format!("  result={result}"));
    }
    assert_eq!(actual, expected);
}

// =================================================================================================
// `AutorouteConnectionRouter.route` steps 1-5
// =================================================================================================

/// `P6T16Probe.routeSteps1to5`, i.e. `route_connection` under the probe's arguments.
fn route_once(
    board: &mut Board,
    engine: &mut Option<AutorouteEngine>,
    item: u32,
    net_no: i32,
    ripped: &mut BTreeSet<ItemId>,
    ripup_costs: &mut BTreeMap<ItemId, i32>,
    stop: &Stop,
) -> AutorouteAttemptResult {
    let settings = probe_settings(board);
    let trace_costs = settings.get_trace_costs();
    route_connection(
        board,
        engine,
        ItemId(item),
        net_no,
        &settings,
        &trace_costs,
        ripped,
        ripup_costs,
        1,
        false,
        &|| stop.check(),
    )
}

/// Probe mode `route`: the first connection routes, the same item routed again answers
/// `NO_UNCONNECTED_NETS` at `:49-52` before any engine is built, and a positive net the board
/// does not have makes `AutorouteControl::new` panic — **ruling 7's fifth boundary
/// (`:154-158`)** — which degrades to a **bare** `FAILED` with no details at all. That empty
/// `details` is the whole difference between the fifth boundary and every message-carrying
/// `FAILED` `autoroute_connection` produces.
#[test]
fn route_steps_one_to_five_match_java_including_the_fifth_boundary() {
    let mut rows = Vec::new();
    for regime in REGIMES {
        let mut board = simple_board();
        rows.push(format!("=== {}", regime_name(regime)));
        board.rules.trace_angle_restriction = regime;
        let stop = Stop::never();
        let mut engine = None;
        let mut ripped = BTreeSet::new();
        let mut ripup_costs = BTreeMap::new();

        let first = route_once(
            &mut board,
            &mut engine,
            2,
            1,
            &mut ripped,
            &mut ripup_costs,
            &stop,
        );
        rows.push(format!("  first={first}"));
        rows.push(format!("  {}", ripped_of(&ripped)));
        rows.extend(board_dump(&board));

        let second = route_once(
            &mut board,
            &mut engine,
            2,
            1,
            &mut ripped,
            &mut ripup_costs,
            &stop,
        );
        rows.push(format!("  second={second}"));

        let boundary = route_once(
            &mut board,
            &mut engine,
            2,
            99,
            &mut ripped,
            &mut ripup_costs,
            &stop,
        );
        rows.push(format!("  boundary5={boundary}"));
        rows.push(format!("  boundary5state={}", boundary.state));
        rows.push(format!("  boundary5details=[{}]", boundary.details()));
    }
    assert_rows_match("route", &rows);
}

/// Probe mode `routeripup`: `route_connection` on the blocker board over three `ripupPassNo`
/// values, which is the only fixture here where `:44-46` — `ripupAllowed = true`,
/// `ripupCosts = startRipupCosts * ripupPassNo` and
/// `removeUnconnectedVias = !settings.isFanoutEnabled()` — reaches the ripup cost model and the
/// `:241-245` `StopConnectionOption` choice through the full pipeline entry point.
#[test]
fn route_connection_rips_through_javas_cost_model() {
    let mut rows = Vec::new();
    for regime in REGIMES {
        rows.push(format!("=== {}", regime_name(regime)));
        for pass in [1, 2, 4] {
            let mut board = blocked_board();
            board.rules.trace_angle_restriction = regime;
            let stop = Stop::never();
            let mut engine = None;
            let mut ripped = BTreeSet::new();
            let mut ripup_costs = BTreeMap::new();
            let settings = probe_settings(&board);
            let trace_costs = settings.get_trace_costs();
            let result = route_connection(
                &mut board,
                &mut engine,
                ItemId(2),
                1,
                &settings,
                &trace_costs,
                &mut ripped,
                &mut ripup_costs,
                pass,
                false,
                &|| stop.check(),
            );
            rows.push(format!("  pass={pass} {result}"));
            rows.push(format!("  {}", ripped_of(&ripped)));
            rows.push(format!("  {}", ripup_costs_of(&ripup_costs)));
            rows.extend(board_dump(&board));
        }
    }
    assert_rows_match("routeripup", &rows);
}

/// `no_unconnected_items_answers_no_unconnected_nets` — the brief's name for the second half of
/// the mode above, asserted on its own so that a failure names the arm.
#[test]
fn no_unconnected_items_answers_no_unconnected_nets() {
    let mut board = simple_board();
    let stop = Stop::never();
    let mut engine = None;
    let mut ripped = BTreeSet::new();
    let mut ripup_costs = BTreeMap::new();
    let first = route_once(
        &mut board,
        &mut engine,
        2,
        1,
        &mut ripped,
        &mut ripup_costs,
        &stop,
    );
    assert_eq!(first.state, AutorouteAttemptState::Routed);
    let second = route_once(
        &mut board,
        &mut engine,
        2,
        1,
        &mut ripped,
        &mut ripup_costs,
        &stop,
    );
    assert_eq!(second.state, AutorouteAttemptState::NoUnconnectedNets);
    assert_eq!(second.details(), "");
}

/// Probe mode `plane`: `:54-68`. `describeConnection` prints the **start** set first, so the
/// FAILED message is where the swap is observable — `plain` names the unconnected set first,
/// `plane` names the connected one. A `ConductionArea` of the same net in the connected set
/// short-circuits at `:58-60` with `CONNECTED_TO_PLANE`, before the swap.
#[test]
fn the_plane_swap_reverses_start_and_dest() {
    let mut rows = Vec::new();
    for regime in REGIMES {
        rows.push(format!("=== {}", regime_name(regime)));
        for shape in ["plain", "plane", "conduction"] {
            let mut board = sealed_board();
            board.rules.trace_angle_restriction = regime;
            if shape != "plain" {
                board
                    .rules
                    .nets
                    .get_mut(1)
                    .expect("net 1 exists")
                    .set_contains_plane(true);
            }
            if shape == "conduction" {
                board.insert_conduction_area(
                    Area::from(Shape::Tile(TileShape::Box(IntBox::from_coords(
                        -600, -200, -200, 200,
                    )))),
                    0,
                    vec![1],
                    1,
                    false,
                    FixedState::Unfixed,
                ); // id 5
            }
            let stop = Stop::never();
            let mut engine = None;
            let mut ripped = BTreeSet::new();
            let mut ripup_costs = BTreeMap::new();
            let result = route_once(
                &mut board,
                &mut engine,
                2,
                1,
                &mut ripped,
                &mut ripup_costs,
                &stop,
            );
            rows.push(format!("  {shape}={result}"));
        }
    }
    assert_rows_match("plane", &rows);
}

// =================================================================================================
// `describeConnection`
// =================================================================================================

/// Probe mode `describe`: the `", "` join, the `" and "` separator, the empty-set shape and —
/// the load-bearing part — the **descending** id order of Java's `TreeSet<Item>`
/// (`Item.compareTo:95-102` is `other.id - this.id`, quirk #44).
#[test]
fn describe_connection_joins_javas_item_names_in_descending_id_order() {
    let board = probe_board();
    let mut rows = Vec::new();
    let empty = BTreeSet::new();
    rows.push(format!(
        "  empty=[{}]",
        describe_via_message(&board, &empty, &empty)
    ));
    rows.push(format!(
        "  one=[{}]",
        describe_via_message(&board, &set_of(&[2]), &set_of(&[3]))
    ));
    rows.push(format!(
        "  many=[{}]",
        describe_via_message(&board, &set_of(&[2, 4, 6]), &set_of(&[3, 5]))
    ));
    rows.push(format!(
        "  vias=[{}]",
        describe_via_message(&board, &set_of(&[7, 9]), &set_of(&[8, 10, 11]))
    ));
    for item in board.get_items() {
        rows.push(format!("  item id={} toString=[{item}]", item.id().0));
    }
    assert_rows_match("describe", &rows);
}

/// `describeConnection`, called directly — Java's probe reaches the `private static` method by
/// reflection and the port's is `pub` for the same reason.
fn describe_via_message(
    board: &Board,
    start: &BTreeSet<ItemId>,
    dest: &BTreeSet<ItemId>,
) -> String {
    describe_connection(board, start, dest)
}
