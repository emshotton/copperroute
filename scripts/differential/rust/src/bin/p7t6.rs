//! Rust twin of `scripts/differential/java/P7T6.java` (Plan 7 Task 5): `PolylineTrace`'s
//! `ConnectionToPin` trio — `checkConnectionToPin` (`:1013-1076`), `correctConnectionToPin`
//! (`:1082-1245`) and `swapConnectionToPin` (`:1252-1313`) — plus the four call sites of
//! `pullTight:841-861` that drive them.
//!
//! Usage: `p7t6 <check|correct|swap|rand|edge>`. See the Java twin's class comment for what each
//! mode covers and why the fixture's pad is a 400 x 100 rectangle on a four-pin package.
//!
//! The board builder is transcribed rather than shared: the Java twin is self-contained too, so
//! any drift between the two fixtures shows up as a diff rather than as a silently-agreeing pair.

use fr_board::items::Item;
use fr_board::prelude::*;
use fr_dsn::format_double;
use fr_geometry::{IntBox, IntPoint, IntVector, Line, Point, Polyline, Shape, TileShape};
use fr_router::board_ext::{PolylineTraceExt, TraceTightener};
use std::io::{BufWriter, Write};

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

const RANDOM_COUNT: i32 = 128;
const RANDOM_SEED: i64 = 70605;

struct JavaRandom {
    seed: i64,
}

impl JavaRandom {
    const MULTIPLIER: i64 = 0x5DEECE66D_i64;
    const ADDEND: i64 = 0xB;
    const MASK: i64 = (1 << 48) - 1;

    fn new(seed: i64) -> JavaRandom {
        JavaRandom {
            seed: (seed ^ JavaRandom::MULTIPLIER) & JavaRandom::MASK,
        }
    }

    fn next(&mut self, bits: u32) -> i32 {
        self.seed = self
            .seed
            .wrapping_mul(JavaRandom::MULTIPLIER)
            .wrapping_add(JavaRandom::ADDEND)
            & JavaRandom::MASK;
        (self.seed >> (48 - bits)) as i32
    }

    fn next_int(&mut self, bound: i32) -> i32 {
        let mut r = self.next(31);
        let m = bound - 1;
        if bound & m == 0 {
            r = ((bound as i64 * r as i64) >> 31) as i32;
        } else {
            let mut u = r;
            loop {
                r = u % bound;
                if u.wrapping_sub(r).wrapping_add(m) >= 0 {
                    break;
                }
                u = self.next(31);
            }
        }
        r
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mode = args.first().map(String::as_str).unwrap_or("check");
    let stdout = std::io::stdout();
    let mut out = BufWriter::new(stdout.lock());
    match mode {
        "check" => check_mode(&mut out),
        "correct" => correct_mode(&mut out),
        "swap" => swap_mode(&mut out),
        "rand" => rand_mode(&mut out),
        "edge" => edge_mode(&mut out),
        _ => {
            eprintln!("usage: p7t6 <check|correct|swap|rand|edge>");
            std::process::exit(2);
        }
    }
    out.flush().expect("stdout");
}

// -- the board ------------------------------------------------------------------------------------

fn layers() -> LayerStructure {
    LayerStructure::new(vec![Layer::new("front", true), Layer::new("back", true)])
}

/// `P7T6.build` — a four-pin component with a 400 x 100 SMD pad per pin.
fn build(angle: AngleRestriction, pin_edge_to_turn_dist: f64) -> Board {
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

// -- dumps ----------------------------------------------------------------------------------------

fn dump_line(line: &Line) -> String {
    format!("({},{})->({},{})", line.a.x, line.a.y, line.b.x, line.b.y)
}

fn dump_corner(polyline: &Polyline, no: usize) -> String {
    match polyline.corner(no).expect("no is below cornerCount") {
        Point::Int(p) => format!("({},{})", p.x, p.y),
        Point::Rational(_) => {
            let f = polyline.corner_approx(no).expect("no is below cornerCount");
            format!(
                "~({},{})",
                format_double(f.x),
                format_double(f.y)
            )
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

/// Java's `FixedState.toString()` — the enum constant name.
fn dump_fixed_state(state: FixedState) -> &'static str {
    match state {
        FixedState::Unfixed => "UNFIXED",
        FixedState::ShoveFixed => "SHOVE_FIXED",
        FixedState::UserFixed => "USER_FIXED",
        FixedState::SystemFixed => "SYSTEM_FIXED",
    }
}

fn dump_board(out: &mut impl Write, board: &Board) {
    writeln!(
        out,
        "    maxId={}",
        board.communication.id_gen.max_generated_id()
    )
    .expect("stdout");
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
        writeln!(out, "{line}").expect("stdout");
    }
}

/// Java's `AngleRestriction.toString()`.
fn regime_name(angle: AngleRestriction) -> &'static str {
    match angle {
        AngleRestriction::NinetyDegree => "NINETY_DEGREE",
        AngleRestriction::FortyFiveDegree => "FORTYFIVE_DEGREE",
        AngleRestriction::None => "NONE",
    }
}

// -- the tables -------------------------------------------------------------------------------------

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

/// `P7T6.lastTrace` — `get_items()` is descending id, so the newest trace comes first.
fn last_trace(board: &Board) -> Option<ItemId> {
    board
        .get_items()
        .find(|item| matches!(item, Item::Trace(_)))
        .map(Item::id)
}

fn insert(board: &mut Board, case: &Case) -> Option<ItemId> {
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

// -- mode `check` -----------------------------------------------------------------------------------

fn check_mode(out: &mut impl Write) {
    for angle in REGIMES {
        for edge in EDGE_DISTS {
            for case in table() {
                let mut board = build(angle, edge);
                let trace = insert(&mut board, &case).expect("the case inserts a trace");
                writeln!(
                    out,
                    "regime={} edge={} case={} atStart={} atEnd={}",
                    regime_name(angle),
                    format_double(edge),
                    case.name,
                    <Board as PolylineTraceExt>::check_connection_to_pin(&board, trace, true),
                    <Board as PolylineTraceExt>::check_connection_to_pin(&board, trace, false),
                )
                .expect("stdout");
            }
        }
    }
}

// -- mode `correct` ---------------------------------------------------------------------------------

fn correct_mode(out: &mut impl Write) {
    for angle in REGIMES {
        for edge in EDGE_DISTS {
            for case in table() {
                for at_start in [true, false] {
                    let mut board = build(angle, edge);
                    let trace = insert(&mut board, &case).expect("the case inserts a trace");
                    let changed = <Board as PolylineTraceExt>::correct_connection_to_pin(
                        &mut board, None, trace, at_start, angle,
                    )
                    .expect("correctConnectionToPin cannot fail in Java");
                    writeln!(
                        out,
                        "regime={} edge={} case={} atStart={at_start} changed={changed}",
                        regime_name(angle),
                        format_double(edge),
                        case.name,
                    )
                    .expect("stdout");
                    dump_board(out, &board);
                }
            }
        }
    }
}

// -- mode `swap` ------------------------------------------------------------------------------------

struct SwapCase {
    name: &'static str,
    stub_end: Point,
    main_corners: Vec<Point>,
    half_width: i32,
    /// The stub's own half width, normally the main trace's. `wide-stub` makes them differ:
    /// `swapConnectionToPin` never compares widths, but `combineAtStart` does
    /// (`PolylineTrace.java:239-244`), so that row is a `swap` that succeeds and whose
    /// `combine()` then merges **nothing**.
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

fn swap_mode(out: &mut impl Write) {
    for angle in REGIMES {
        for edge in EDGE_DISTS {
            for case in swap_table() {
                for at_start in [true, false] {
                    let mut board = build(angle, edge);
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
                    let main = last_trace(&board).expect("the case inserts a trace");
                    let changed = <Board as PolylineTraceExt>::swap_connection_to_pin(
                        &mut board, None, main, at_start,
                    )
                    .expect("swapConnectionToPin cannot fail in Java");
                    writeln!(
                        out,
                        "regime={} edge={} case={} atStart={at_start} changed={changed}",
                        regime_name(angle),
                        format_double(edge),
                        case.name,
                    )
                    .expect("stdout");
                    dump_board(out, &board);
                }
            }
        }
    }
}

// -- mode `rand` ------------------------------------------------------------------------------------

fn rand_mode(out: &mut impl Write) {
    for angle in REGIMES {
        writeln!(
            out,
            "regime={} n={RANDOM_COUNT} seed={RANDOM_SEED}",
            regime_name(angle)
        )
        .expect("stdout");
        let mut rnd = JavaRandom::new(RANDOM_SEED);
        for i in 0..RANDOM_COUNT {
            let corner_count = 2 + rnd.next_int(4);
            let mut corners = vec![p(-500, 0)];
            for _ in 1..corner_count {
                corners.push(p(rnd.next_int(2401) - 1200, rnd.next_int(1601) - 400));
            }
            let half_width = 10 + rnd.next_int(60);
            let edge = EDGE_DISTS[rnd.next_int(EDGE_DISTS.len() as i32) as usize];
            let at_start = rnd.next_int(2) == 0;

            let mut board = build(angle, edge);
            let polyline = Polyline::from_points(&corners);
            if polyline.lines().len() < 3 {
                writeln!(out, "row {i} degenerate").expect("stdout");
                continue;
            }
            board.insert_trace_without_cleaning(
                polyline.clone(),
                0,
                half_width,
                vec![1],
                1,
                FixedState::Unfixed,
            );
            let checked = match last_trace(&board) {
                Some(trace) => {
                    <Board as PolylineTraceExt>::check_connection_to_pin(&board, trace, at_start)
                }
                None => true,
            };
            writeln!(
                out,
                "row {i} hw={half_width} edge={} atStart={at_start} check={checked}",
                format_double(edge),
            )
            .expect("stdout");
            writeln!(out, "  in  {}", dump_polyline(&polyline)).expect("stdout");

            let mut board = build(angle, edge);
            board.insert_trace_without_cleaning(
                Polyline::from_points(&corners),
                0,
                half_width,
                vec![1],
                1,
                FixedState::Unfixed,
            );
            let corrected = match last_trace(&board) {
                Some(trace) => <Board as PolylineTraceExt>::correct_connection_to_pin(
                    &mut board, None, trace, at_start, angle,
                )
                .expect("correctConnectionToPin cannot fail in Java"),
                None => false,
            };
            writeln!(out, "  correct={corrected}").expect("stdout");
            dump_board(out, &board);

            let mut board = build(angle, edge);
            let stub = Polyline::from_points(&[p(-500, 0), corners[1].clone()]);
            if stub.lines().len() >= 3 {
                board.insert_trace_without_cleaning(
                    stub,
                    0,
                    half_width,
                    vec![1],
                    1,
                    FixedState::ShoveFixed,
                );
                let tail_polyline = Polyline::from_points(&corners[1..]);
                if tail_polyline.lines().len() >= 3 {
                    board.insert_trace_without_cleaning(
                        tail_polyline,
                        0,
                        half_width,
                        vec![1],
                        1,
                        FixedState::Unfixed,
                    );
                    let swapped = match last_trace(&board) {
                        Some(trace) => <Board as PolylineTraceExt>::swap_connection_to_pin(
                            &mut board, None, trace, at_start,
                        )
                        .expect("swapConnectionToPin cannot fail in Java"),
                        None => false,
                    };
                    writeln!(out, "  swap={swapped}").expect("stdout");
                    dump_board(out, &board);
                } else {
                    writeln!(out, "  swap=degenerate-tail").expect("stdout");
                }
            } else {
                writeln!(out, "  swap=degenerate-stub").expect("stdout");
            }
        }
    }
}

// -- mode `edge` ------------------------------------------------------------------------------------

fn edge_mode(out: &mut impl Write) {
    for angle in REGIMES {
        for edge in EDGE_DISTS {
            for case in table() {
                let mut board = build(angle, edge);
                let trace = insert(&mut board, &case).expect("the case inserts a trace");
                let mut algo = TraceTightener::get_instance(
                    &mut board,
                    Vec::new(),
                    None,
                    500,
                    None,
                    -1,
                    None,
                    -1,
                );
                let changed =
                    <Board as PolylineTraceExt>::pull_tight_with(&mut board, trace, &mut algo);
                writeln!(
                    out,
                    "regime={} edge={} case={} pullTight={changed}",
                    regime_name(angle),
                    format_double(edge),
                    case.name,
                )
                .expect("stdout");
                dump_board(out, &board);
            }
        }
    }
}
