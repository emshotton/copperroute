use copper_board::items::Item;
use copper_board::prelude::*;
use copper_dsn::format::double::format_double;
use copper_geometry::{IntBox, IntPoint, IntVector, Line, Point, Polyline, Shape, TileShape};
use copper_router::autoroute::maze::engine::AutorouteEngine;
use copper_router::board_ext::{PolylineTraceExt, TraceTightener};

const TRANSCRIPT: &str = include_str!("data/p7t6-connection-to-pin.txt");
const TASK_16_GOLDEN: &str = include_str!("data/p9t16-connection-to-pin.txt");

fn section(mode: &str) -> Vec<&'static str> {
    let transcript = if matches!(mode, "check" | "correct") {
        TASK_16_GOLDEN
    } else {
        TRANSCRIPT
    };
    let header = format!("######## {mode}");
    let mut rows = Vec::new();
    let mut inside = false;
    for line in transcript.lines() {
        if line.starts_with("######## ") {
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

const REGIMES: [AngleRestriction; 3] = [
    AngleRestriction::NinetyDegree,
    AngleRestriction::FortyFiveDegree,
    AngleRestriction::None,
];

const EDGE_DISTS: [f64; 4] = [-1.0, 0.0, 100.0, 500.0];

fn layers() -> LayerStructure {
    LayerStructure::new(vec![Layer::new("front", true), Layer::new("back", true)])
}

fn probe_board(angle: AngleRestriction, pin_edge_to_turn_dist: f64) -> Board {
    let mut clearance_matrix = ClearanceMatrix::get_default_instance(&layers(), 200);
    assert!(clearance_matrix.append_class("wide"));
    clearance_matrix.set_value_on_all_layers(2, 1, 600);
    clearance_matrix.set_value_on_all_layers(2, 2, 800);
    let mut rules = BoardRules::new(layers(), clearance_matrix);
    rules.trace_angle_restriction = angle;
    rules.set_pin_edge_to_turn_dist(pin_edge_to_turn_dist);

    let mut padstacks = Padstacks::new(layers());
    let long = padstacks.add(
        "long",
        vec![
            Some(Shape::Tile(TileShape::Box(IntBox::from_coords(
                -200, -50, 200, 50,
            )))),
            None,
        ],
        false,
        false,
    );
    let mut packages = Packages::new();
    let package = packages.add(
        "pkg",
        vec![
            PackagePin::new("P1", long, IntVector::new(-500, 0).into(), 0.0),
            PackagePin::new("P2", long, IntVector::new(500, 0).into(), 0.0),
            PackagePin::new("P3", long, IntVector::new(-500, 2000).into(), 0.0),
            PackagePin::new("P4", long, IntVector::new(500, 2000).into(), 0.0),
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
    let default_class = board.rules.get_default_net_class();
    board.rules.nets.add("N1", 1, false, default_class);
    board.rules.nets.add("N2", 1, false, default_class);

    board.insert_pin(1, 0, vec![1], 1, FixedState::Unfixed);
    board.insert_pin(1, 1, vec![1], 1, FixedState::Unfixed);
    board
}

fn p(x: i32, y: i32) -> Point {
    Point::new(x, y)
}

struct Case {
    name: &'static str,
    corners: Vec<Point>,
    half_width: i32,
    clearance_class: usize,
}

fn table() -> Vec<Case> {
    vec![
        Case {
            name: "short-right",
            corners: vec![p(-500, 0), p(-200, 0), p(-200, 800)],
            half_width: 30,
            clearance_class: 1,
        },
        Case {
            name: "mid-right",
            corners: vec![p(-500, 0), p(100, 0), p(100, 800)],
            half_width: 30,
            clearance_class: 1,
        },
        Case {
            name: "long-right",
            corners: vec![p(-500, 0), p(500, 0), p(500, 800)],
            half_width: 30,
            clearance_class: 1,
        },
        Case {
            name: "up-out",
            corners: vec![p(-500, 0), p(-500, 800), p(300, 800)],
            half_width: 30,
            clearance_class: 1,
        },
        Case {
            name: "diag-out",
            corners: vec![p(-500, 0), p(-100, 400), p(600, 400)],
            half_width: 30,
            clearance_class: 1,
        },
        Case {
            name: "pin-to-pin-short",
            corners: vec![p(-500, 0), p(-200, 300), p(200, 300), p(500, 0)],
            half_width: 30,
            clearance_class: 1,
        },
        Case {
            name: "pin-to-pin-long",
            corners: vec![p(-500, 0), p(-500, 900), p(500, 900), p(500, 0)],
            half_width: 30,
            clearance_class: 1,
        },
        Case {
            name: "wide-class",
            corners: vec![p(-500, 0), p(-200, 0), p(-200, 800)],
            half_width: 30,
            clearance_class: 2,
        },
        Case {
            name: "fat",
            corners: vec![p(-500, 0), p(100, 0), p(100, 800)],
            half_width: 90,
            clearance_class: 1,
        },
        Case {
            name: "two-corner",
            corners: vec![p(-500, 0), p(0, 0)],
            half_width: 30,
            clearance_class: 1,
        },
    ]
}

struct SwapCase {
    name: &'static str,
    stub_end: Point,
    main_corners: Vec<Point>,
    half_width: i32,
    stub_half_width: i32,
}

fn swap_table() -> Vec<SwapCase> {
    vec![
        SwapCase {
            name: "left-stub-sharp",
            stub_end: p(-800, 0),
            main_corners: vec![p(-800, 0), p(-200, 300)],
            half_width: 30,
            stub_half_width: 30,
        },
        SwapCase {
            name: "left-stub-blunt",
            stub_end: p(-800, 0),
            main_corners: vec![p(-800, 0), p(-1400, 300)],
            half_width: 30,
            stub_half_width: 30,
        },
        SwapCase {
            name: "right-stub-sharp",
            stub_end: p(-200, 0),
            main_corners: vec![p(-200, 0), p(-800, 300)],
            half_width: 30,
            stub_half_width: 30,
        },
        SwapCase {
            name: "left-stub-long",
            stub_end: p(-1200, 0),
            main_corners: vec![p(-1200, 0), p(-300, 700)],
            half_width: 30,
            stub_half_width: 30,
        },
        SwapCase {
            name: "left-stub-kink",
            stub_end: p(-800, 0),
            main_corners: vec![p(-800, 0), p(-780, 20), p(-200, 400)],
            half_width: 30,
            stub_half_width: 30,
        },
        SwapCase {
            name: "left-stub-fat",
            stub_end: p(-900, 0),
            main_corners: vec![p(-900, 0), p(-200, 500)],
            half_width: 90,
            stub_half_width: 90,
        },
        SwapCase {
            name: "wide-stub",
            stub_end: p(-800, 0),
            main_corners: vec![p(-800, 0), p(-200, 300)],
            half_width: 30,
            stub_half_width: 90,
        },
    ]
}

fn last_trace(board: &Board) -> ItemId {
    board
        .get_items()
        .find(|item| matches!(item, Item::Trace(_)))
        .map(Item::id)
        .expect("the fixture inserted a trace")
}

fn insert(board: &mut Board, case: &Case) -> ItemId {
    board.insert_trace_without_cleaning(
        Polyline::from_points(&case.corners),
        0,
        case.half_width,
        vec![1],
        case.clearance_class,
        FixedState::Unfixed,
    );
    last_trace(board)
}

fn regime_name(angle: AngleRestriction) -> &'static str {
    match angle {
        AngleRestriction::NinetyDegree => "NINETY_DEGREE",
        AngleRestriction::FortyFiveDegree => "FORTYFIVE_DEGREE",
        AngleRestriction::None => "NONE",
    }
}

fn dump_line(line: &Line) -> String {
    format!("({},{})->({},{})", line.a.x, line.a.y, line.b.x, line.b.y)
}

fn dump_corner(polyline: &Polyline, no: usize) -> String {
    match polyline.corner(no).expect("no is below cornerCount") {
        Point::Int(point) => format!("({},{})", point.x, point.y),
        Point::Rational(_) => {
            let f = polyline.corner_approx(no).expect("no is below cornerCount");
            format!("~({},{})", format_double(f.x), format_double(f.y))
        }
    }
}

fn dump_polyline(polyline: &Polyline) -> String {
    let lines: Vec<String> = polyline.lines().iter().map(dump_line).collect();
    let corners: Vec<String> = (0..polyline.corner_count())
        .map(|i| dump_corner(polyline, i))
        .collect();
    format!(
        "n={} lines=[{}] corners=[{}]",
        polyline.lines().len(),
        lines.join(","),
        corners.join(",")
    )
}

fn dump_point(point: &Point) -> String {
    let rounded = point.to_float().round();
    format!("({},{})", rounded.x, rounded.y)
}

fn dump_nets(net_nos: &[i32]) -> String {
    let inner: Vec<String> = net_nos.iter().map(i32::to_string).collect();
    format!("[{}]", inner.join(","))
}

fn dump_fixed_state(state: FixedState) -> &'static str {
    match state {
        FixedState::Unfixed => "UNFIXED",
        FixedState::ShoveFixed => "SHOVE_FIXED",
        FixedState::UserFixed => "USER_FIXED",
        FixedState::SystemFixed => "SYSTEM_FIXED",
    }
}

fn dump_board(board: &Board) -> Vec<String> {
    let mut out = vec![format!(
        "    maxId={}",
        board.communication.id_gen.max_generated_id()
    )];
    for item in board.get_items() {
        let type_name = match item {
            Item::Trace(_) => "PolylineTrace",
            Item::Via(_) => "Via",
            Item::Pin(_) => "Pin",
            Item::ObstacleArea(_) => "ObstacleArea",
            Item::ConductionArea(_) => "ConductionArea",
            Item::ViaObstacleArea(_) => "ViaObstacleArea",
            Item::ComponentObstacleArea(_) => "ComponentObstacleArea",
            Item::ComponentOutline(_) => "ComponentOutline",
            Item::BoardOutline(_) => "BoardOutline",
        };
        let mut line = format!(
            "    item id={} type={} nets={} cl={} fix={}",
            item.id().0,
            type_name,
            dump_nets(item.net_nos()),
            item.clearance_class(),
            dump_fixed_state(item.get_fixed_state())
        );
        match item {
            Item::Trace(trace) => {
                line.push_str(&format!(
                    " layer={} hw={} {}",
                    trace.get_layer(),
                    trace.get_half_width(),
                    dump_polyline(trace.polyline())
                ));
            }
            Item::Pin(_) => {
                let center = board.drill_center(item.id()).expect("a pin has a centre");
                line.push_str(&format!(" center={}", dump_point(&center)));
            }
            _ => {}
        }
        out.push(line);
    }
    out
}

#[test]
fn check_connection_to_pin_matches_the_task_16_golden_over_the_whole_table() {
    let rows = section("check");
    let mut i = 0;
    for angle in REGIMES {
        for edge in EDGE_DISTS {
            for case in table() {
                let mut board = probe_board(angle, edge);
                let trace = insert(&mut board, &case);
                let line = format!(
                    "regime={} edge={} case={} atStart={} atEnd={}",
                    regime_name(angle),
                    format_double(edge),
                    case.name,
                    <Board as PolylineTraceExt>::check_connection_to_pin(&board, trace, true),
                    <Board as PolylineTraceExt>::check_connection_to_pin(&board, trace, false),
                );
                assert_eq!(line, rows[i], "check row {i}");
                i += 1;
            }
        }
    }
    assert_eq!(
        i,
        rows.len(),
        "the transcript has rows the test did not walk"
    );
}

#[test]
fn check_connection_to_pin_answers_the_corrected_outcomes() {
    let short = &table()[0];
    for edge in [0.0, 500.0] {
        let mut board = probe_board(AngleRestriction::None, edge);
        let trace = insert(&mut board, short);
        assert!(!<Board as PolylineTraceExt>::check_connection_to_pin(
            &board, trace, true
        ));
    }
    let mid = &table()[1];
    let mut board = probe_board(AngleRestriction::None, 100.0);
    let trace = insert(&mut board, mid);
    assert!(<Board as PolylineTraceExt>::check_connection_to_pin(
        &board, trace, true
    ));
    let mut board = probe_board(AngleRestriction::None, 500.0);
    let trace = insert(&mut board, mid);
    assert!(!<Board as PolylineTraceExt>::check_connection_to_pin(
        &board, trace, true
    ));

    let up = &table()[3];
    let mut board = probe_board(AngleRestriction::None, 0.0);
    let trace = insert(&mut board, up);
    assert!(!<Board as PolylineTraceExt>::check_connection_to_pin(
        &board, trace, true
    ));

    let long = &table()[2];
    let mut board = probe_board(AngleRestriction::None, -1.0);
    let trace = insert(&mut board, long);
    assert!(!<Board as PolylineTraceExt>::check_connection_to_pin(
        &board, trace, true
    ));
    let mut board = probe_board(AngleRestriction::None, 0.0);
    let trace = insert(&mut board, long);
    assert!(!<Board as PolylineTraceExt>::check_connection_to_pin(
        &board, trace, true
    ));
}

fn correct_rows_for(angle: AngleRestriction) -> Vec<String> {
    let mut out = Vec::new();
    for edge in EDGE_DISTS {
        for case in table() {
            for at_start in [true, false] {
                let mut board = probe_board(angle, edge);
                let trace = insert(&mut board, &case);
                let changed = <Board as PolylineTraceExt>::correct_connection_to_pin(
                    &mut board, None, trace, at_start, angle,
                )
                .expect("correctConnectionToPin cannot fail in Java");
                out.push(format!(
                    "regime={} edge={} case={} atStart={at_start} changed={changed}",
                    regime_name(angle),
                    format_double(edge),
                    case.name,
                ));
                out.extend(dump_board(&board));
            }
        }
    }
    out
}

fn transcript_rows_for(mode: &str, angle: AngleRestriction) -> Vec<&'static str> {
    let marker = format!("regime={} ", regime_name(angle));
    let rows = section(mode);
    let mut out = Vec::new();
    let mut inside = false;
    for row in rows {
        if row.starts_with("regime=") {
            inside = row.starts_with(&marker);
        }
        if inside {
            out.push(row);
        }
    }
    assert!(
        !out.is_empty(),
        "no `{mode}` rows for {}",
        regime_name(angle)
    );
    out
}

#[test]
fn correct_connection_to_pin_matches_the_task_16_golden_at_ninety_degrees() {
    assert_eq!(
        correct_rows_for(AngleRestriction::NinetyDegree),
        transcript_rows_for("correct", AngleRestriction::NinetyDegree)
    );
}

#[test]
fn correct_connection_to_pin_matches_the_task_16_golden_at_fortyfive_degrees() {
    assert_eq!(
        correct_rows_for(AngleRestriction::FortyFiveDegree),
        transcript_rows_for("correct", AngleRestriction::FortyFiveDegree)
    );
}

#[test]
fn correct_connection_to_pin_matches_the_task_16_golden_at_any_angle() {
    assert_eq!(
        correct_rows_for(AngleRestriction::None),
        transcript_rows_for("correct", AngleRestriction::None)
    );
}

fn swap_rows_for(angle: AngleRestriction) -> Vec<String> {
    let mut out = Vec::new();
    for edge in EDGE_DISTS {
        for case in swap_table() {
            for at_start in [true, false] {
                let mut board = probe_board(angle, edge);
                board.insert_trace_without_cleaning(
                    Polyline::from_points(&[p(-500, 0), case.stub_end.clone()]),
                    0,
                    case.stub_half_width,
                    vec![1],
                    1,
                    FixedState::ShoveFixed,
                );
                let mut main_corners = case.main_corners.clone();
                if !at_start {
                    main_corners.reverse();
                }
                board.insert_trace_without_cleaning(
                    Polyline::from_points(&main_corners),
                    0,
                    case.half_width,
                    vec![1],
                    1,
                    FixedState::Unfixed,
                );
                let main = last_trace(&board);
                let changed = <Board as PolylineTraceExt>::swap_connection_to_pin(
                    &mut board, None, main, at_start,
                )
                .expect("swapConnectionToPin cannot fail in Java");
                out.push(format!(
                    "regime={} edge={} case={} atStart={at_start} changed={changed}",
                    regime_name(angle),
                    format_double(edge),
                    case.name,
                ));
                out.extend(dump_board(&board));
            }
        }
    }
    out
}

#[test]
fn swap_connection_to_pin_matches_the_jvm_at_ninety_degrees() {
    assert_eq!(
        swap_rows_for(AngleRestriction::NinetyDegree),
        transcript_rows_for("swap", AngleRestriction::NinetyDegree)
    );
}

#[test]
fn swap_connection_to_pin_matches_the_jvm_at_fortyfive_degrees() {
    assert_eq!(
        swap_rows_for(AngleRestriction::FortyFiveDegree),
        transcript_rows_for("swap", AngleRestriction::FortyFiveDegree)
    );
}

#[test]
fn swap_connection_to_pin_matches_the_jvm_at_any_angle() {
    assert_eq!(
        swap_rows_for(AngleRestriction::None),
        transcript_rows_for("swap", AngleRestriction::None)
    );
}

#[test]
fn a_successful_swap_reorders_the_stub_into_the_trace() {
    let case = &swap_table()[0];
    let mut board = probe_board(AngleRestriction::None, 100.0);
    board.insert_trace_without_cleaning(
        Polyline::from_points(&[p(-500, 0), case.stub_end.clone()]),
        0,
        case.half_width,
        vec![1],
        1,
        FixedState::ShoveFixed,
    );
    let stub = last_trace(&board);
    board.insert_trace_without_cleaning(
        Polyline::from_points(&case.main_corners),
        0,
        case.half_width,
        vec![1],
        1,
        FixedState::Unfixed,
    );
    let main = last_trace(&board);
    assert_ne!(stub, main);
    assert_eq!(
        board.items.get(&stub).map(Item::get_fixed_state),
        Some(FixedState::ShoveFixed)
    );

    assert!(
        <Board as PolylineTraceExt>::swap_connection_to_pin(&mut board, None, main, true)
            .expect("swapConnectionToPin cannot fail in Java")
    );

    let survivors: Vec<ItemId> = board
        .get_items()
        .filter(|item| matches!(item, Item::Trace(_)))
        .map(Item::id)
        .collect();
    assert_eq!(survivors.len(), 1, "the stub was swallowed");
    let Some(Item::Trace(trace)) = board.items.get(&survivors[0]) else {
        unreachable!("just filtered")
    };
    assert_eq!(trace.first_corner(), Some(p(-500, 0)));
    assert_eq!(trace.last_corner(), Some(p(-200, 300)));
}

fn edge_rows_for(angle: AngleRestriction) -> Vec<String> {
    let mut out = Vec::new();
    for edge in EDGE_DISTS {
        for case in table() {
            let mut board = probe_board(angle, edge);
            let trace = insert(&mut board, &case);
            let mut algo =
                TraceTightener::get_instance(&mut board, Vec::new(), None, 500, None, -1, None, -1);
            let changed =
                <Board as PolylineTraceExt>::pull_tight_with(&mut board, trace, &mut algo);
            out.push(format!(
                "regime={} edge={} case={} pullTight={changed}",
                regime_name(angle),
                format_double(edge),
                case.name,
            ));
            out.extend(dump_board(&board));
        }
    }
    out
}

#[test]
fn pull_tight_matches_the_jvm_in_every_regime_at_every_pin_edge_to_turn_dist() {
    for angle in REGIMES {
        assert_eq!(
            edge_rows_for(angle),
            transcript_rows_for("edge", angle),
            "edge rows for {}",
            regime_name(angle)
        );
    }
}

#[test]
fn the_pair_is_skipped_at_ninety_degrees() {
    let case = &table()[0];

    let mut board = probe_board(AngleRestriction::NinetyDegree, 500.0);
    let trace = insert(&mut board, case);
    assert!(
        <Board as PolylineTraceExt>::correct_connection_to_pin(
            &mut board,
            None,
            trace,
            true,
            AngleRestriction::NinetyDegree
        )
        .expect("cannot fail"),
        "the fixture is one the correction would change"
    );

    let mut board = probe_board(AngleRestriction::NinetyDegree, 500.0);
    let trace = insert(&mut board, case);
    let before = dump_board(&board);
    let mut algo =
        TraceTightener::get_instance(&mut board, Vec::new(), None, 500, None, -1, None, -1);
    assert!(!<Board as PolylineTraceExt>::pull_tight_with(
        &mut board, trace, &mut algo
    ));
    assert_eq!(dump_board(&board), before);
}

#[test]
fn a_zero_pin_edge_to_turn_distance_is_refused() {
    let case = &table()[4];

    fn shove_fixed_traces(board: &Board) -> usize {
        board
            .get_items()
            .filter(|item| {
                matches!(item, Item::Trace(_)) && item.get_fixed_state() == FixedState::ShoveFixed
            })
            .count()
    }

    let mut board = probe_board(AngleRestriction::None, 0.0);
    let trace = insert(&mut board, case);
    assert!(
        !<Board as PolylineTraceExt>::check_connection_to_pin(&board, trace, true),
        "the check refuses zero"
    );
    assert!(
        !<Board as PolylineTraceExt>::correct_connection_to_pin(
            &mut board,
            None,
            trace,
            true,
            AngleRestriction::None
        )
        .expect("cannot fail"),
        "the correction refuses zero"
    );
    assert_eq!(shove_fixed_traces(&board), 0);

    for edge in [-1.0, 0.0] {
        let mut board = probe_board(AngleRestriction::None, edge);
        let trace = insert(&mut board, case);
        let before = dump_board(&board);
        let mut algo =
            TraceTightener::get_instance(&mut board, Vec::new(), None, 500, None, -1, None, -1);
        assert!(!<Board as PolylineTraceExt>::pull_tight_with(
            &mut board, trace, &mut algo
        ));
        assert_eq!(shove_fixed_traces(&board), 0, "edge={edge}");
        assert_eq!(dump_board(&board), before, "edge={edge}");
    }

    let mut board = probe_board(AngleRestriction::None, 500.0);
    let trace = insert(&mut board, case);
    let mut algo =
        TraceTightener::get_instance(&mut board, Vec::new(), None, 500, None, -1, None, -1);
    assert!(<Board as PolylineTraceExt>::pull_tight_with(
        &mut board, trace, &mut algo
    ));
    assert_eq!(shove_fixed_traces(&board), 1);
}

#[test]
fn combine_calls_additional_update_after_change_once_per_merge_and_never_without_one() {
    for (case_name, expect_merge) in [("left-stub-sharp", true), ("wide-stub", false)] {
        let case = swap_table()
            .into_iter()
            .find(|c| c.name == case_name)
            .expect("the case is in the table");
        let mut board = probe_board(AngleRestriction::None, 100.0);
        board.insert_trace_without_cleaning(
            Polyline::from_points(&[p(-500, 0), case.stub_end.clone()]),
            0,
            case.stub_half_width,
            vec![1],
            1,
            FixedState::ShoveFixed,
        );
        board.insert_trace_without_cleaning(
            Polyline::from_points(&case.main_corners),
            0,
            case.half_width,
            vec![1],
            1,
            FixedState::Unfixed,
        );
        let main = last_trace(&board);

        let mut engine = AutorouteEngine::new(&mut board, 1, true);
        engine.init_connection(&mut board, 1, None);
        let seed = engine.add_incomplete_expansion_room(
            None,
            0,
            Some(TileShape::Box(IntBox::from_coords(-660, -40, -640, -20))),
        );
        let completed = engine
            .complete_expansion_room(&mut board, seed)
            .expect("no failure boundary is reached");
        assert_eq!(
            completed.len(),
            2,
            "{case_name}: the seed completes to two rooms"
        );
        let rooms_before = engine.complete_expansion_rooms().len();
        assert_eq!(rooms_before, 2, "{case_name}");

        assert!(
            <Board as PolylineTraceExt>::swap_connection_to_pin(
                &mut board,
                Some(&mut engine),
                main,
                true
            )
            .expect("swapConnectionToPin cannot fail in Java"),
            "{case_name}: the swap itself must succeed in both rows"
        );

        let rooms_after = engine.complete_expansion_rooms().len();
        let traces_left = board
            .get_items()
            .filter(|item| matches!(item, Item::Trace(_)))
            .count();
        if expect_merge {
            assert_eq!(traces_left, 1, "{case_name}: the stub was absorbed");
            assert_eq!(
                rooms_after, 1,
                "{case_name}: the merge invalidated the room it now overlaps"
            );
        } else {
            assert_eq!(
                traces_left, 2,
                "{case_name}: the widths differ, so combineAtStart refuses and nothing merges"
            );
            assert_eq!(
                rooms_after, rooms_before,
                "{case_name}: zero merges is zero additionalUpdateAfterChange calls \
                 (PolylineTrace.java:183-190) — this is the assertion the earlier draft failed"
            );
        }
    }
}
