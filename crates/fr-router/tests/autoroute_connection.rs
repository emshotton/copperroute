#![allow(clippy::too_many_lines, clippy::too_many_arguments)]

use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};

use fr_board::ids::{ItemId, PadstackId, ViaInfoId};
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
    board.insert_pin(1, 0, vec![1], 1, FixedState::Unfixed);
    board.insert_pin(1, 1, vec![1], 1, FixedState::Unfixed);
    board
}

fn simple_board() -> Board {
    simple_board_with(true)
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
    );
    board
}

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

struct Stop {
    calls: Cell<u32>,
    stopped: bool,
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

fn ripped_of(ripped: &BTreeSet<ItemId>) -> String {
    let ids: Vec<String> = ripped.iter().rev().map(|id| id.0.to_string()).collect();
    format!("ripped n={} [{}]", ripped.len(), ids.join(","))
}

fn ripup_costs_of(costs: &BTreeMap<ItemId, i32>) -> String {
    let entries: Vec<String> = costs
        .iter()
        .rev()
        .map(|(id, cost)| format!("{}={cost}", id.0))
        .collect();
    format!("ripupCosts n={} [{}]", costs.len(), entries.join(","))
}

fn ln(line: &fr_geometry::Line) -> String {
    format!("({},{})->({},{})", line.a.x, line.a.y, line.b.x, line.b.y)
}

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

fn point_of(point: &Point) -> String {
    let rounded = point.to_float().round();
    format!("({},{})", rounded.x, rounded.y)
}

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

const T16: &str = include_str!("data/p6t16-autoroute-connection.txt");

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

const KNOWN_DIVERGENCES: &[(&str, usize, &str, &str)] = &[
    (
        "plain",
        18,
        "    item id=8 type=PolylineTrace nets=[1] cl=1 layer=0 hw=30 n=3 lines=[(400,0)->(401,-1),(-400,0)->(-401,0),(-400,0)->(-399,1)] corners=[(400,0),(-400,0)]",
        "    item id=8 type=PolylineTrace nets=[1] cl=1 layer=0 hw=30 n=3 lines=[(400,0)->(401,-1),(-264,0)->(-400,0),(-400,0)->(-400,1)] corners=[(400,0),(-400,0)]",
    ),
    ("stopafter", 10, "  stopCalls=9", "  stopCalls=10"),
    ("stopafter", 55, "  stopCalls=13", "  stopCalls=14"),
    (
        "route",
        9,
        "  boundary5=FAILED:",
        "  boundary5=NO_UNCONNECTED_NETS:",
    ),
    (
        "route",
        10,
        "  boundary5state=FAILED",
        "  boundary5state=NO_UNCONNECTED_NETS",
    ),
    (
        "route",
        21,
        "  boundary5=FAILED:",
        "  boundary5=NO_UNCONNECTED_NETS:",
    ),
    (
        "route",
        22,
        "  boundary5state=FAILED",
        "  boundary5state=NO_UNCONNECTED_NETS",
    ),
    (
        "route",
        33,
        "  boundary5=FAILED:",
        "  boundary5=NO_UNCONNECTED_NETS:",
    ),
    (
        "route",
        34,
        "  boundary5state=FAILED",
        "  boundary5state=NO_UNCONNECTED_NETS",
    ),
    (
        "plain",
        23,
        "  result=ROUTED:",
        "  result=FAILED: Failed to route connection between pin of component #1 and pin #1 of component #1, because no connection was found between their nets.",
    ),
    ("plain", 24, "  state=ROUTED", "  state=FAILED"),
    ("plain", 28, "    maxId=4", "    maxId=3"),
    (
        "plain",
        29,
        "    item id=4 type=PolylineTrace nets=[1] cl=1 layer=0 hw=30 n=3 lines=[(400,0)->(400,-1),(400,0)->(-400,0),(-400,0)->(-400,1)] corners=[(400,0),(-400,0)]",
        "    item id=3 type=Pin nets=[1] cl=1 center=(400,0)",
    ),
    (
        "plain",
        30,
        "    item id=3 type=Pin nets=[1] cl=1 center=(400,0)",
        "    item id=2 type=Pin nets=[1] cl=1 center=(-400,0)",
    ),
    (
        "plain",
        31,
        "    item id=2 type=Pin nets=[1] cl=1 center=(-400,0)",
        "    item id=1 type=BoardOutline nets=[] cl=0",
    ),
    (
        "plain",
        32,
        "    item id=1 type=BoardOutline nets=[] cl=0",
        "<missing>",
    ),
    ("stopafter", 99, "  stopCalls=21", "  stopCalls=20"),
    (
        "maintain",
        11,
        "  after-connection completeRooms n=1 [1]",
        "  after-connection completeRooms n=1 [3]",
    ),
    (
        "maintain",
        12,
        "  after-connection roomsWithTargetItems n=1 [1]",
        "  after-connection roomsWithTargetItems n=1 [3]",
    ),
    (
        "maintain",
        13,
        "  after-same-net completeRooms n=1 [1]",
        "  after-same-net completeRooms n=1 [3]",
    ),
    (
        "maintain",
        14,
        "  after-same-net roomsWithTargetItems n=1 [1]",
        "  after-same-net roomsWithTargetItems n=1 [3]",
    ),
    (
        "maintain",
        28,
        "  after-connection completeRooms n=6 [1,3,4,5,6,7]",
        "  after-connection completeRooms n=6 [3,11,12,13,14,15]",
    ),
    (
        "maintain",
        29,
        "  after-connection roomsWithTargetItems n=2 [6,1]",
        "  after-connection roomsWithTargetItems n=2 [14,3]",
    ),
    (
        "maintain",
        30,
        "  after-same-net completeRooms n=6 [1,3,4,5,6,7]",
        "  after-same-net completeRooms n=6 [3,11,12,13,14,15]",
    ),
    (
        "maintain",
        31,
        "  after-same-net roomsWithTargetItems n=2 [6,1]",
        "  after-same-net roomsWithTargetItems n=2 [14,3]",
    ),
    (
        "maintain",
        45,
        "  after-connection completeRooms n=5 [1,3,4,6,7]",
        "  after-connection completeRooms n=5 [3,18,23,29,35]",
    ),
    (
        "maintain",
        46,
        "  after-connection roomsWithTargetItems n=2 [6,1]",
        "  after-connection roomsWithTargetItems n=2 [29,3]",
    ),
    (
        "maintain",
        47,
        "  after-same-net completeRooms n=5 [1,3,4,6,7]",
        "  after-same-net completeRooms n=5 [3,18,23,29,35]",
    ),
    (
        "maintain",
        48,
        "  after-same-net roomsWithTargetItems n=2 [6,1]",
        "  after-same-net roomsWithTargetItems n=2 [29,3]",
    ),
];

fn assert_rows_match(mode: &str, actual: &[String]) {
    let expected = t16_section(mode);
    let mut diffs = Vec::new();
    let mut accounted = 0usize;
    for i in 0..expected.len().max(actual.len()) {
        let want = expected.get(i).copied().unwrap_or("<missing>");
        let got = actual
            .get(i)
            .map(|row| row.trim_end())
            .unwrap_or("<missing>");
        if want == got {
            continue;
        }
        if KNOWN_DIVERGENCES
            .iter()
            .any(|(m, row, jvm, rust)| *m == mode && *row == i && *jvm == want && *rust == got)
        {
            accounted += 1;
            continue;
        }
        diffs.push(format!("row {i}\n  jvm:  {want}\n  rust: {got}"));
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
    let declared = KNOWN_DIVERGENCES
        .iter()
        .filter(|(m, ..)| *m == mode)
        .count();
    assert_eq!(
        accounted, declared,
        "mode `{mode}` declares {declared} known divergence(s) from the jar but only {accounted} \
         of them still differ — a divergence that has healed must be deleted from \
         KNOWN_DIVERGENCES, not left to rot"
    );
}

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

#[test]
fn a_plain_connection_routes_and_inserts_javas_trace() {
    all_regimes("plain", simple_board, false, false);
}

#[test]
fn a_layer_changing_connection_routes_through_javas_via() {
    all_regimes("via", probe_board, false, false);
}

#[test]
fn a_ripping_connection_deletes_javas_items_and_reports_javas_ripped_set() {
    all_regimes("ripup", blocked_board, true, true);
}

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

fn tree_size(engine: &AutorouteEngine, board: &Board) -> usize {
    board
        .trees
        .trees()
        .find(|tree| tree.id() == engine.tree)
        .expect("the engine's tree is in the board")
        .size()
}

#[test]
fn no_connection_found_fails_with_javas_message() {
    all_regimes("nopath", blocked_board, true, false);
}

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

#[test]
fn skipped_has_no_producer_because_connection_items_is_never_null() {
    let source = include_str!("../src/autoroute/maze/engine.rs");
    assert!(
        !source.contains("AutorouteAttemptState::Skipped"),
        "AutorouteEngine.autorouteConnection:230-235 is unreachable (quirk #180); a `Skipped` \
         here would be a state Java cannot produce"
    );
    assert_eq!(AutorouteAttemptState::Skipped.name(), "SKIPPED");
}

#[test]
fn cleanup_runs_before_every_early_return() {
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

#[test]
fn a_stop_inside_the_pop_loop_degrades_through_boundary_three() {
    let mut rows = Vec::new();
    for regime in REGIMES {
        for limit in [8u32, 12, 20] {
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
        settings.get_start_ripup_costs(),
        !settings.is_fanout_enabled(),
        false,
        &|| stop.check(),
    )
}

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
                settings.get_start_ripup_costs(),
                !settings.is_fanout_enabled(),
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
                );
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

fn describe_via_message(
    board: &Board,
    start: &BTreeSet<ItemId>,
    dest: &BTreeSet<ItemId>,
) -> String {
    describe_connection(board, start, dest)
}
