//! Rust twin of `scripts/differential/java/P7T3.java` (Plan 7 Task 5): `RoutingBoard.optChangedArea`
//! (RoutingBoard.java:151-161 -> :171-190 -> RoutingBoardOperations.java:52-79) and the
//! `TraceTightener.optChangedArea(ExpansionCostFactor[])` sweep it drives (TraceTightener.java:
//! 121-169), over a real DSN board whose changed area was marked by real routing.
//!
//! Usage: `p7t3 <dsn> [mode] [accuracy] [routeK]`. See the Java twin's class comment for the five
//! modes and for why mode 4 is expected to diff until Plan 7 Task 6 lands `ViaOptimizer`.
//!
//! **The budget is disabled on both sides.** `RouterBudget::disabled()`'s `opt_changed_area_ms` is
//! `0`, which is what the Java half passes for `timeLimit`, and `TraceTightener`'s constructor
//! builds a `TimeLimit` only when `timeLimit > 0` (TraceTightener.java:73-77). So neither side
//! reads a clock and a wall-clock difference between the two languages cannot move the result.
//!
//! `P6T1.java`'s four routing choices are transcribed here for the reason `p7t7.rs`'s module
//! comment gives: Rust binaries cannot share a private module, and the Java side is the
//! un-duplicated one, so drift shows up as a diff.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufWriter, Write};
use std::time::UNIX_EPOCH;

use fr_board::items::Item;
use fr_board::prelude::*;
use fr_dsn::parser::scope_parameter::DsnReadOptions;
use fr_dsn::{java_double_to_string, BoardReadResult};
use fr_geometry::{Line, Point, Polyline};
use fr_router::board_ext::RoutingBoardExt;
use fr_router::pipeline::RouterBudget;
use fr_router::route_connection;
use fr_settings::sources::DefaultSettings;
use fr_settings::{ExpansionCostFactor, HostEnvironment, RouterSettings, SettingsSource};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: p7t3 <dsn> [mode] [accuracy] [routeK]");
        std::process::exit(2);
    }
    let dsn = std::fs::canonicalize(&args[0])
        .unwrap_or_else(|e| panic!("cannot resolve {}: {e}", args[0]));
    let mode: i32 = args.get(1).map_or(0, |a| a.parse().expect("mode"));
    let accuracy: i32 = args.get(2).map_or(500, |a| a.parse().expect("accuracy"));
    let route_k: usize = args.get(3).map_or(6, |a| a.parse().expect("routeK"));

    let stdout = std::io::stdout();
    let mut out = BufWriter::new(stdout.lock());
    print_header(&mut out, &dsn, mode, accuracy, route_k);

    let mut board = load_board(&dsn);
    let settings = build_settings(&board);

    for connection in pick_connections(&board, route_k) {
        route_one(&mut out, &mut board, &settings, &connection);
    }

    let regime = match mode {
        0 => AngleRestriction::NinetyDegree,
        1 => AngleRestriction::FortyFiveDegree,
        _ => AngleRestriction::None,
    };
    board.rules.trace_angle_restriction = regime;
    if mode == 3 {
        board.rules.set_pin_edge_to_turn_dist(100_000.0);
    }
    let trace_costs: Option<Vec<ExpansionCostFactor>> = if mode == 4 {
        Some(vec![
            ExpansionCostFactor {
                horizontal: 1.0,
                vertical: 1.0,
            };
            board.get_layer_count()
        ])
    } else {
        None
    };

    writeln!(
        out,
        "sweep regime={} pinEdgeToTurnDist={} traceCosts={}",
        regime_name(regime),
        java_double_to_string(board.rules.get_pin_edge_to_turn_dist()),
        match &trace_costs {
            Some(costs) => costs.len().to_string(),
            None => "null".to_string(),
        }
    )
    .expect("write");
    dump_changed_area(&mut out, &board, "before");

    // `clip_shape = None` runs `RoutingBoardOperations.java:64`'s branch — the reference
    // comparison ruling 9 is about. The stop check never trips and the budget is disabled.
    let budget = RouterBudget::disabled();
    board
        .opt_changed_area(
            None,
            &[],
            None,
            accuracy,
            trace_costs.as_deref(),
            &|| false,
            budget.opt_changed_area_ms,
        )
        .expect("optChangedArea cannot fail in Java");

    dump_changed_area(&mut out, &board, "after");
    dump_board(&mut out, &board);
    out.flush().expect("flush");
}

// ------------------------------------------------------------------------------------------------
// Header, board, settings, routing — `P6T1.java`'s choices, transcribed
// ------------------------------------------------------------------------------------------------

fn print_header<W: Write>(
    out: &mut W,
    dsn: &std::path::Path,
    mode: i32,
    accuracy: i32,
    route_k: usize,
) {
    let jar =
        std::env::var("FREEROUTING_JAR").expect("environment variable FREEROUTING_JAR is not set");
    let jar = std::fs::canonicalize(jar).expect("jar exists");
    let meta = std::fs::metadata(&jar).expect("jar metadata");
    let mtime = meta
        .modified()
        .expect("mtime")
        .duration_since(UNIX_EPOCH)
        .expect("after epoch")
        .as_millis();
    writeln!(
        out,
        "HEADER jar={} bytes={} mtime={mtime} fixture={} mode={mode} accuracy={accuracy} routeK={route_k}",
        jar.display(),
        meta.len(),
        dsn.file_name().expect("a file name").to_string_lossy(),
    )
    .expect("write");
    if let Ok(exe) = std::env::current_exe() {
        eprintln!("rust-binary {}", exe.display());
    }
}

fn load_board(dsn: &std::path::Path) -> Board {
    let file = std::fs::File::open(dsn).unwrap_or_else(|e| panic!("cannot open {dsn:?}: {e}"));
    let design_name = dsn
        .file_name()
        .expect("a file name")
        .to_string_lossy()
        .into_owned();
    let result = fr_dsn::read_board(file, None, Some(&design_name), &DsnReadOptions::default());
    match result {
        BoardReadResult::Success { board, .. } | BoardReadResult::OutlineMissing { board, .. } => {
            *board.unwrap_or_else(|| panic!("{design_name} produced no board"))
        }
        other => panic!("{design_name} did not read: {other:?}"),
    }
}

fn build_settings(board: &Board) -> RouterSettings {
    let host = HostEnvironment::detect();
    let mut settings = DefaultSettings::new(&host)
        .get_settings()
        .expect("DefaultSettings always answers a table")
        .clone();
    settings.set_layer_count(board.get_layer_count());
    settings.apply_board_specific_optimizations(board);
    settings
}

struct Connection {
    k: usize,
    item_id: ItemId,
    net_no: i32,
}

fn pick_connections(board: &Board, max_items: usize) -> Vec<Connection> {
    let mut result = Vec::new();
    if max_items == 0 {
        return result;
    }
    let mut k = 0;
    for item_id in board.items_in_board_order() {
        let Some(item) = board.get_item(item_id) else {
            continue;
        };
        if item.as_connectable().is_none() {
            continue;
        }
        let net_nos: Vec<i32> = item.net_nos().to_vec();
        for net_no in net_nos {
            if board.unconnected_set(item_id, net_no).is_empty() {
                continue;
            }
            k += 1;
            result.push(Connection {
                k,
                item_id,
                net_no,
            });
            if result.len() >= max_items {
                return result;
            }
        }
    }
    result
}

fn route_one<W: Write>(
    out: &mut W,
    board: &mut Board,
    settings: &RouterSettings,
    connection: &Connection,
) {
    if board.get_item(connection.item_id).is_none() {
        writeln!(
            out,
            "route k={} item={} state=GONE",
            connection.k, connection.item_id.0
        )
        .expect("write");
        return;
    }
    board.start_marking_changed_area();
    let mut ripped: BTreeSet<ItemId> = BTreeSet::new();
    let mut ripup_costs: BTreeMap<ItemId, i32> = BTreeMap::new();
    let trace_costs = settings.get_trace_costs();
    let mut engine = None;
    let result = route_connection(
        board,
        &mut engine,
        connection.item_id,
        connection.net_no,
        settings,
        &trace_costs,
        &mut ripped,
        &mut ripup_costs,
        1,
        settings.get_start_ripup_costs(),
        !settings.is_fanout_enabled(),
        false,
        &|| false,
    );
    writeln!(
        out,
        "route k={} item={} net={} state={} ripped={}",
        connection.k,
        connection.item_id.0,
        connection.net_no,
        result.state.name(),
        ripped.len()
    )
    .expect("write");
}

// ------------------------------------------------------------------------------------------------
// Dumps — `P7T3.java`'s
// ------------------------------------------------------------------------------------------------

fn regime_name(angle: AngleRestriction) -> &'static str {
    match angle {
        AngleRestriction::NinetyDegree => "NINETY_DEGREE",
        AngleRestriction::FortyFiveDegree => "FORTYFIVE_DEGREE",
        AngleRestriction::None => "NONE",
    }
}

fn dump_changed_area<W: Write>(out: &mut W, board: &Board, tag: &str) {
    let Some(changed_area) = &board.changed_area else {
        writeln!(out, "changedArea {tag}=null").expect("write");
        return;
    };
    let mut parts = Vec::new();
    for i in 0..board.get_layer_count() {
        let area = changed_area.get_area(i);
        parts.push(if area.is_empty() {
            "empty".to_string()
        } else {
            format!(
                "({},{},{},{},{},{},{},{})",
                area.left_x,
                area.bottom_y,
                area.right_x,
                area.top_y,
                area.upper_left_diagonal_x,
                area.lower_right_diagonal_x,
                area.lower_left_diagonal_x,
                area.upper_right_diagonal_x
            )
        });
    }
    writeln!(out, "changedArea {tag}=[{}]", parts.join(",")).expect("write");
}

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
                java_double_to_string(f.x),
                java_double_to_string(f.y)
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

fn dump_fixed_state(state: FixedState) -> &'static str {
    match state {
        FixedState::Unfixed => "UNFIXED",
        FixedState::ShoveFixed => "SHOVE_FIXED",
        FixedState::UserFixed => "USER_FIXED",
        FixedState::SystemFixed => "SYSTEM_FIXED",
    }
}

fn dump_board<W: Write>(out: &mut W, board: &Board) {
    writeln!(
        out,
        "maxId={}",
        board.communication.id_gen.max_generated_id()
    )
    .expect("write");
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
            "item id={} type={} nets={} cl={} fix={}",
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
            Item::Via(_) | Item::Pin(_) => {
                let center = board
                    .drill_center(item.id())
                    .expect("a drill item has a centre");
                line.push_str(&format!(" center={}", dump_point(&center)));
            }
            _ => {}
        }
        writeln!(out, "{line}").expect("write");
    }
}
