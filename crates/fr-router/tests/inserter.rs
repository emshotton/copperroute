#![allow(clippy::too_many_lines)]

use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};

use fr_board::ids::{ItemId, PadstackId, ViaInfoId};
use fr_board::prelude::*;
use fr_board::rules::{ViaInfo, ViaRule};
use fr_dsn::format::double::format_double;
use fr_geometry::{IntBox, IntOctagon, IntPoint, IntVector, Point, Polyline, Shape, TileShape};
use fr_router::autoroute::maze::AutorouteControl;
use fr_router::autoroute::maze::engine::AutorouteEngine;
use fr_router::autoroute::maze::search::MazeSearchEngine;
use fr_router::autoroute::path::{FoundConnectionInserter, FoundConnectionLocator};
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

/// `P6T15Probe.buildToTrace` — `simple_board` plus a net-1 trace to route to.
fn to_trace_board() -> Board {
    let mut board = simple_board();
    board.insert_trace_without_cleaning(
        Polyline::from_points(&[Point::new(-400, 600), Point::new(400, 600)]),
        0,
        30,
        vec![1],
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

// =================================================================================================
// Search, locate, insert — `P6T15Probe.locate` and `P6T15Probe.insertAndDump`
// =================================================================================================

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

struct Located {
    locator: FoundConnectionLocator,
    ctrl: AutorouteControl,
    ripped: BTreeSet<ItemId>,
}

/// `P6T15Probe.locate`: run the maze search on `board` and locate its result under `angle`, with
/// the control the insert then reuses.
fn locate(
    board: &mut Board,
    angle: AngleRestriction,
    start: &[u32],
    dest: &[u32],
    no_vias: bool,
) -> Located {
    let mut engine = AutorouteEngine::new(board, 1, false);
    engine.init_connection(board, 1, None);
    let mut ctrl = probe_control(board, 1);
    if no_vias {
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
        ctrl,
        ripped,
    }
}

// =================================================================================================
// The transcript
// =================================================================================================

const T15: &str = include_str!("data/p6t15-inserter.txt");

fn t15_section(mode: &str) -> Vec<&'static str> {
    let header = format!("=== mode {mode} ===");
    let mut rows = Vec::new();
    let mut inside = false;
    for line in T15.lines() {
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

const T11_DIAG: &str = include_str!("data/p9t11-inserter-diag.txt");

const PORT_LANE: &[(&str, &str, &str)] = &[(
    "diag",
    "#186",
    "`FoundConnectionInserter` handed `connectToTrace` a trace the insert had already split away, \
     so the stub was inserted against a polyline the board no longer held and the two tail \
     removals then deleted both halves of the split trace. The jar's rows show trace 4 gone and \
     its line surviving only inside the combined trace 17; the port keeps both halves (ids 6 and \
     7) and lands the connection as a third trace.",
)];

/// The port-lane golden's rows, with its `#` provenance header stripped.
fn port_lane_section(mode: &str) -> Vec<&'static str> {
    let text = match mode {
        "diag" => T11_DIAG,
        _ => panic!("no port-lane golden for mode `{mode}`"),
    };
    let rows: Vec<&str> = text
        .lines()
        .filter(|line| !line.starts_with('#'))
        .map(str::trim_end)
        .collect();
    assert!(!rows.is_empty(), "port-lane golden `{mode}` is empty");
    rows
}

/// Compares the rows this port produces with the JVM's — or, for a [`PORT_LANE`] mode, with the
/// port's own re-cut golden — collecting **every** difference rather than stopping at the first.
fn assert_rows_match(mode: &str, actual: &[String]) {
    let in_port_lane = PORT_LANE.iter().any(|(name, _, _)| *name == mode);
    let expected = if in_port_lane {
        port_lane_section(mode)
    } else {
        t15_section(mode)
    };
    let lane = if in_port_lane { "port" } else { "jvm" };
    let mut diffs = Vec::new();
    for i in 0..expected.len().max(actual.len()) {
        let want = expected.get(i).copied().unwrap_or("<missing>");
        let got = actual.get(i).map(String::as_str).unwrap_or("<missing>");
        if want != got {
            diffs.push(format!("row {i}\n  {lane}:  {want}\n  rust: {got}"));
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

/// Every [`PORT_LANE`] mode must **still** differ from the jar. Without this, a fix that was
/// quietly reverted would go green against its own re-cut golden and nothing would notice; with
/// it, the re-cut is a statement about a divergence that has to keep existing.
#[test]
fn the_port_lane_modes_still_differ_from_the_jar() {
    for (mode, row, reason) in PORT_LANE {
        assert_ne!(
            port_lane_section(mode),
            t15_section(mode),
            "port-lane mode `{mode}` now MATCHES the jar — delete its PORT_LANE entry \
             (register {row}: {reason})"
        );
    }
}

/// Re-cuts [`T11_DIAG`]. `#[ignore]`d because it is a generator, not a check.
///
/// ```text
/// cargo test -p fr-router --test inserter -- --ignored --nocapture emit_the_diag_port_golden
/// ```
#[test]
#[ignore = "generator: prints the port-lane golden for re-cutting"]
fn emit_the_diag_port_golden() {
    let mut rows = Vec::new();
    for regime in REGIMES {
        let mut board = diag_trace_board();
        rows.push(format!("=== {}", regime_name(regime)));
        let located = locate(&mut board, regime, &[2, 3], &[4], false);
        rows.extend(t15_insert_and_dump(&mut board, &located));
    }
    for line in T11_DIAG.lines().take_while(|l| l.starts_with('#')) {
        println!("{line}");
    }
    for row in rows {
        println!("{row}");
    }
}

// --- the probe's dump format ---------------------------------------------------------------------

/// `P6T15Probe.ln`.
fn t15_line(line: &fr_geometry::Line) -> String {
    format!("({},{})->({},{})", line.a.x, line.a.y, line.b.x, line.b.y)
}

/// `P6T15Probe.pt`.
fn t15_corner(polyline: &Polyline, no: usize) -> String {
    match polyline.corner(no) {
        Some(Point::Int(p)) => format!("({},{})", p.x, p.y),
        _ => {
            let f = polyline.corner_approx(no).expect("no is below cornerCount");
            format!(
                "~({},{})",
                format_double(f.x),
                format_double(f.y)
            )
        }
    }
}

/// `P6T15Probe.poly`.
fn t15_polyline(polyline: &Polyline) -> String {
    let lines: Vec<String> = polyline.lines().iter().map(t15_line).collect();
    let corners: Vec<String> = (0..polyline.corner_count())
        .map(|i| t15_corner(polyline, i))
        .collect();
    format!(
        "n={} lines=[{}] corners=[{}]",
        polyline.lines().len(),
        lines.join(","),
        corners.join(",")
    )
}

/// `P6T15Probe.pointOf`.
fn t15_point(point: &Point) -> String {
    let rounded = point.to_float().round();
    format!("({},{})", rounded.x, rounded.y)
}

/// `P6T15Probe.nets`.
fn t15_nets(net_nos: &[i32]) -> String {
    let inner: Vec<String> = net_nos.iter().map(i32::to_string).collect();
    format!("[{}]", inner.join(","))
}

fn t15_type_name(item: &Item) -> &'static str {
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

fn t15_board_dump(board: &Board) -> Vec<String> {
    let ctx = board.ctx();
    let mut out = vec![format!(
        "    maxId={}",
        board.communication.id_gen.max_generated_id()
    )];
    for item in board.get_items() {
        let mut line = format!(
            "    item id={} type={} nets={} cl={}",
            item.id().0,
            t15_type_name(item),
            t15_nets(item.net_nos()),
            item.clearance_class()
        );
        match item {
            Item::Trace(trace) => line.push_str(&format!(
                " layer={} hw={} {}",
                trace.get_layer(),
                trace.get_half_width(),
                t15_polyline(trace.polyline())
            )),
            Item::Via(via) => {
                let padstack = board
                    .library
                    .padstacks
                    .get(via.get_padstack_id())
                    .expect("the via's padstack is in the library");
                line.push_str(&format!(
                    " center={} padstack={} layers={}..{}",
                    t15_point(&via.get_center()),
                    padstack.name,
                    via.first_layer(&ctx),
                    via.last_layer(&ctx)
                ));
            }
            Item::Pin(pin) => {
                line.push_str(&format!(" center={}", t15_point(&pin.get_center(&ctx))));
            }
            _ => {}
        }
        out.push(line);
    }
    out
}

/// `P6T15Probe.dumpItems`.
fn t15_dump_items(locator: &FoundConnectionLocator) -> Vec<String> {
    let mut out = vec![
        format!(
            "  startItem={} startLayer={} targetItem={} targetLayer={}",
            locator
                .start_item
                .map_or_else(|| "null".to_string(), |id| id.0.to_string()),
            locator.start_layer,
            locator
                .target_item
                .map_or_else(|| "null".to_string(), |id| id.0.to_string()),
            locator.target_layer
        ),
        format!("  connectionItems n={}", locator.connection_items.len()),
    ];
    for (i, item) in locator.connection_items.iter().enumerate() {
        let mut line = format!(
            "    [{}] layer={} corners={}",
            i,
            item.layer,
            item.corners.len()
        );
        for corner in &item.corners {
            line.push_str(&format!(" ({},{})", corner.x, corner.y));
        }
        out.push(line);
    }
    out
}

/// `P6T15Probe.insertAndDump`.
fn t15_insert_and_dump(board: &mut Board, located: &Located) -> Vec<String> {
    let mut out = t15_dump_items(&located.locator);
    out.push(format!("  ripped n={}", located.ripped.len()));
    let before = board.communication.id_gen.max_generated_id();
    let counter = Counter::new();
    let inserter = FoundConnectionInserter::get_instance(
        Some(&located.locator),
        board,
        &located.ctrl,
        None,
        &|| counter.check(),
    )
    .expect("the insert does not fail with an error on any fixture in this file");
    out.push(format!(
        "  insert={} maxIdBefore={}",
        if inserter.is_none() { "null" } else { "ok" },
        before.0
    ));
    out.extend(t15_board_dump(board));
    out
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

// =================================================================================================
// The tests
// =================================================================================================

#[test]
fn a_two_pin_connection_produces_one_trace_with_corrected_geometry() {
    for regime in REGIMES {
        let mut board = simple_board();
        let located = locate(&mut board, regime, &[3], &[2], false);
        assert_eq!(located.locator.connection_items.len(), 1);
        assert!(located.ripped.is_empty());
        let counter = Counter::new();
        assert!(
            FoundConnectionInserter::get_instance(
                Some(&located.locator),
                &mut board,
                &located.ctrl,
                None,
                &|| counter.check(),
            )
            .expect("the insertion completes")
            .is_some()
        );
        let traces = board.get_traces();
        assert_eq!(traces.len(), 1);
        let Some(Item::Trace(trace)) = board.get_item(traces[0]) else {
            panic!("the listed item is a trace");
        };
        let first = trace
            .polyline()
            .first_corner()
            .expect("the trace has a first corner");
        let last = trace
            .polyline()
            .last_corner()
            .expect("the trace has a last corner");
        assert_eq!((first, last), (Point::new(-400, 0), Point::new(400, 0)));
    }
}

#[test]
fn a_layer_change_produces_a_via_at_javas_location_with_javas_padstack() {
    let mut rows = Vec::new();
    for regime in REGIMES {
        let mut board = probe_board();
        rows.push(format!("=== {}", regime_name(regime)));
        let located = locate(&mut board, regime, &[2], &[3], false);
        rows.extend(t15_insert_and_dump(&mut board, &located));
    }
    assert_rows_match("via", &rows);
}

/// Probe mode `around`: with vias forbidden the connection walks the long way round the net-2
/// blocker — 26 / 25 / 7 corners — which is the fixture that exercises the `:171-405` loop's
/// `fromCornerNo` bookkeeping at length.
#[test]
fn the_long_way_round_inserts_javas_multi_segment_trace() {
    let mut rows = Vec::new();
    for regime in REGIMES {
        let mut board = probe_board();
        rows.push(format!("=== {}", regime_name(regime)));
        let located = locate(&mut board, regime, &[2], &[3], true);
        rows.extend(t15_insert_and_dump(&mut board, &located));
    }
    assert_rows_match("around", &rows);
}

/// Probe mode `totrace`: the destination item is a `PolylineTrace`, so `:77-91`'s
/// `connectToTrace` runs and `:108`'s `normalizeTraces` then combines or splits what it left.
#[test]
fn a_trace_target_reaches_connect_to_trace_and_normalize() {
    let mut rows = Vec::new();
    for regime in REGIMES {
        let mut board = to_trace_board();
        rows.push(format!("=== {}", regime_name(regime)));
        let located = locate(&mut board, regime, &[2, 3], &[4], false);
        rows.extend(t15_insert_and_dump(&mut board, &located));
    }
    assert_rows_match("totrace", &rows);
}

/// `P6T15Probe.buildDiagTrace` — `to_trace_board` with a **slanted** target trace, so that the
/// located connection's first corner rounds to a point that is *not* on the target polyline. It
/// is the only fixture in this file where `:79`'s `connectToTrace` does more than
/// `RoutingBoard.connectToTrace:1123-1126`'s "the point is already on the trace" early return —
/// dropping the `:77-91` block leaves every other mode's board untouched, and this one's
/// different.
fn diag_trace_board() -> Board {
    let mut board = simple_board();
    board.insert_trace_without_cleaning(
        Polyline::from_points(&[Point::new(-400, 600), Point::new(400, 653)]),
        0,
        30,
        vec![1],
        1,
        FixedState::Unfixed,
    ); // id 4
    board
}

#[test]
fn a_target_trace_the_connection_misses_gets_javas_connect_to_trace_stub() {
    let mut rows = Vec::new();
    for regime in REGIMES {
        let mut board = diag_trace_board();
        rows.push(format!("=== {}", regime_name(regime)));
        let located = locate(&mut board, regime, &[2, 3], &[4], false);
        rows.extend(t15_insert_and_dump(&mut board, &located));
    }
    assert_rows_match("diag", &rows);
}

/// The location `:66`'s via number `which` goes to — `P6T15Probe.viaLocation`.
fn via_location(locator: &FoundConnectionLocator, which: usize) -> IntPoint {
    let mut current_layer = locator.target_layer;
    let mut seen = 0;
    for item in &locator.connection_items {
        if item.layer != current_layer {
            if seen == which {
                return item.corners[0];
            }
            seen += 1;
        }
        current_layer = item.layer;
    }
    panic!("no layer change #{which} in the located connection");
}

#[test]
fn a_refused_forced_via_check_answers_none_not_an_error() {
    let mut rows = Vec::new();

    // (a) `emptyRule`.
    let mut board = probe_board();
    rows.push("=== emptyRule".to_string());
    let mut located = locate(
        &mut board,
        AngleRestriction::NinetyDegree,
        &[2],
        &[3],
        false,
    );
    board.rules.via_rules.push(ViaRule::new("empty"));
    located.ctrl.via_rule = board.rules.via_rules.last().cloned();
    rows.extend(t15_insert_and_dump(&mut board, &located));

    // (b) `blockedTrace`.
    let mut board = probe_board();
    rows.push("=== blockedTrace".to_string());
    let located = locate(
        &mut board,
        AngleRestriction::NinetyDegree,
        &[2],
        &[3],
        false,
    );
    let drill = via_location(&located.locator, 1);
    rows.push(format!("  drill=({},{})", drill.x, drill.y));
    board
        .insert_via(
            PadstackId(3),
            Point::Int(drill),
            vec![2],
            1,
            FixedState::UserFixed,
            false,
        )
        .expect("the blocking via inserts");
    rows.extend(t15_insert_and_dump(&mut board, &located));

    // (c) `refusedCheck`.
    let mut board = probe_board();
    rows.push("=== refusedCheck".to_string());
    let mut located = locate(
        &mut board,
        AngleRestriction::NinetyDegree,
        &[2],
        &[3],
        false,
    );
    let huge_shape = Shape::Tile(TileShape::Octagon(IntOctagon::new(
        -3000, -3000, 3000, 3000, -6000, 6000, -6000, 6000,
    )));
    let huge_pad = board.library.padstacks.add(
        "huge",
        vec![Some(huge_shape.clone()), Some(huge_shape)],
        true,
        false,
    );
    assert!(
        board
            .rules
            .via_infos
            .add(ViaInfo::new("huge", huge_pad, 1, false)),
        "the name is free"
    );
    let huge_info = ViaInfoId(board.rules.via_infos.count() - 1);
    let mut huge_rule = ViaRule::new("huge");
    huge_rule.append_via(board.rules.via_infos.get(huge_info).clone());
    board.rules.via_rules.push(huge_rule);
    located.ctrl.via_rule = board.rules.via_rules.last().cloned();
    rows.extend(t15_insert_and_dump(&mut board, &located));

    assert_rows_match("viafail", &rows);
}

fn forged_empty_connection(start_layer: usize) -> FoundConnectionLocator {
    FoundConnectionLocator {
        connection_items: Vec::new(),
        start_item: None,
        start_layer,
        target_item: None,
        target_layer: 0,
        backtrack_array: Vec::new(),
    }
}

#[test]
fn a_null_last_corner_with_no_spanning_padstack_answers_none() {
    let mut board = simple_board();
    let mut ctrl = probe_control(&board, 1);
    board.rules.via_rules.push(ViaRule::new("empty"));
    ctrl.via_rule = board.rules.via_rules.last().cloned();
    let before = t15_board_dump(&board);
    let counter = Counter::new();
    let result = FoundConnectionInserter::get_instance(
        Some(&forged_empty_connection(1)),
        &mut board,
        &ctrl,
        None,
        &|| counter.check(),
    );
    assert!(
        matches!(result, Ok(None)),
        "no spanning padstack is Java's `return false` at `:751`, not a throw: {result:?}"
    );
    assert_eq!(before, t15_board_dump(&board), "nothing was inserted");
}

#[test]
#[should_panic(expected = "ForcedViaInserter.java:140")]
fn a_null_last_corner_panics_where_java_dereferences_it() {
    let mut board = simple_board();
    let ctrl = probe_control(&board, 1);
    assert_eq!(
        ctrl.via_rule.as_ref(),
        Some(&board.rules.via_rules[0]),
        "the fixture's spanning rule"
    );
    let counter = Counter::new();
    let _ = FoundConnectionInserter::get_instance(
        Some(&forged_empty_connection(1)),
        &mut board,
        &ctrl,
        None,
        &|| counter.check(),
    );
}

const PER_LAYER_WIDTH_STEM: &str = "p9t11-per-layer-width";

#[test]
fn the_per_layer_width_fixture_really_has_two_different_widths() {
    let board = read_per_layer_width_board();
    let net_no = 1;
    let front = board.rules.get_trace_half_width(net_no, 0);
    let back = board.rules.get_trace_half_width(net_no, 1);
    assert!(
        front > 0 && back > 0,
        "both layers carry a width: F.Cu {front}, B.Cu {back}"
    );
    assert_eq!(
        front,
        back * 8,
        "the 8:1 ratio is the whole point of the fixture: F.Cu {front}, B.Cu {back}"
    );
}

#[test]
fn the_stub_takes_the_width_of_the_layer_it_lands_on() {
    const PER_LAYER: [i32; 2] = [200, 25];

    for target_layer in [0usize, 1] {
        // `probe_board`'s +/-4000 bounding box, not `simple_board`'s +/-1000: at half width 200
        // the wide stub needs room to clear the board boundary, and a stub that fails
        // `checkPolylineTrace` would make this test pass by inserting nothing.
        let mut board = probe_board();
        // A net-1 trace on `target_layer`, clear of every pin, for the stub to land on.
        board.insert_trace_without_cleaning(
            Polyline::from_points(&[Point::new(-1000, 3000), Point::new(1000, 3000)]),
            target_layer,
            30,
            vec![1],
            1,
            FixedState::Unfixed,
        );
        let target = *board
            .items
            .iter()
            .filter(|(_, item)| matches!(item, Item::Trace(_)))
            .map(|(id, _)| id)
            .max()
            .expect("the trace was inserted");
        let before: BTreeSet<ItemId> = board.items.keys().copied().collect();

        assert!(
            board.connect_to_trace_sized_by_layer(&Point::new(0, 3400), target, &PER_LAYER, 1),
            "the stub is inserted"
        );

        let stub_half_widths: Vec<i32> = board
            .items
            .iter()
            .filter(|(id, _)| !before.contains(id))
            .filter_map(|(_, item)| match item {
                Item::Trace(trace) => Some(trace.get_half_width()),
                _ => None,
            })
            .collect();
        assert_eq!(
            stub_half_widths,
            vec![PER_LAYER[target_layer]],
            "a stub landing on layer {target_layer} takes layer {target_layer}'s half width \
             ({}), not the other layer's ({})",
            PER_LAYER[target_layer],
            PER_LAYER[1 - target_layer]
        );

        let mut wrong = probe_board();
        wrong.insert_trace_without_cleaning(
            Polyline::from_points(&[Point::new(-1000, 3000), Point::new(1000, 3000)]),
            target_layer,
            30,
            vec![1],
            1,
            FixedState::Unfixed,
        );
        let wrong_target = *wrong
            .items
            .iter()
            .filter(|(_, item)| matches!(item, Item::Trace(_)))
            .map(|(id, _)| id)
            .max()
            .expect("the trace was inserted");
        let wrong_before: BTreeSet<ItemId> = wrong.items.keys().copied().collect();
        assert!(wrong.connect_to_trace(
            &Point::new(0, 3400),
            wrong_target,
            PER_LAYER[1 - target_layer],
            1,
        ));
        let wrong_widths: Vec<i32> = wrong
            .items
            .iter()
            .filter(|(id, _)| !wrong_before.contains(id))
            .filter_map(|(_, item)| match item {
                Item::Trace(trace) => Some(trace.get_half_width()),
                _ => None,
            })
            .collect();
        assert_eq!(
            wrong_widths,
            vec![PER_LAYER[1 - target_layer]],
            "the control: the width handed in is the width that lands, so the two answers really \
             are different and the fix is doing work"
        );
    }
}

/// Reads [`PER_LAYER_WIDTH_STEM`] into a `Board`, the way `tree_ext.rs` reads its own 90-degree
/// fixture.
fn read_per_layer_width_board() -> Board {
    use fr_dsn::{BoardReadResult, DsnReadOptions};

    let root = parity::workspace_root();
    let dsn = root.join(format!(
        "crates/fr-router/tests/data/{PER_LAYER_WIDTH_STEM}.dsn"
    ));
    let bytes =
        std::fs::read(&dsn).unwrap_or_else(|e| panic!("cannot read {}: {e}", dsn.display()));
    match fr_dsn::read_board(
        std::io::Cursor::new(&bytes[..]),
        None,
        Some(&format!("{PER_LAYER_WIDTH_STEM}.dsn")),
        &DsnReadOptions::default(),
    ) {
        BoardReadResult::Success { board, .. } => *board.expect("the fixture produces a board"),
        other => panic!("{PER_LAYER_WIDTH_STEM} did not read: {other:?}"),
    }
}
