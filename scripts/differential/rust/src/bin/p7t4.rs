//! Rust twin of `scripts/differential/java/P7T4.java` (Plan 7 Task 6): `board.optimize.ViaOptimizer`'s
//! `optViaLocation` (ViaOptimizer.java:33-158), `optPlaneOrFanoutVia` (:161-296) and
//! `isWithinTolerance` (:719-732), over a real DSN board whose vias were placed by real routing.
//!
//! Usage: `p7t4 <dsn> [mode] [accuracy] [routeK]`. See the Java twin's class comment for the four
//! modes, for why the driver declares `package app.freerouting.autoroute.maze` rather than
//! `board.optimize`, and for the scripted `isWithinTolerance` stream mode 2 replays.
//!
//! **The budget is disabled on both sides.** `ViaOptimizer` reads no clock of its own; the routing
//! prologue is `p7t3`'s and the `pull_tight` calls inside the two methods carry a `StopCheck` that
//! never trips, which is Java's `null` `Stoppable`.
//!
//! **Modes 0, 1 and 3 diff until Plan 7 Task 7.** Both methods reach `repositionVia`, whose three
//! overloads are Task 7's and answer `None` here; the Java side moves vias the port leaves alone.
//! Mode 2 is the one this task pins to 0 diffs. `crates/fr-router/README.md` records the measured
//! counts.
//!
//! `P6T1.java`'s four routing choices are transcribed here for the reason `p7t3.rs`'s module
//! comment gives: Rust binaries cannot share a private module, and the Java side is the
//! un-duplicated one, so drift shows up as a diff.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufWriter, Write};
use std::time::UNIX_EPOCH;

use fr_board::items::Item;
use fr_board::prelude::*;
use fr_dsn::parser::scope_parameter::DsnReadOptions;
use fr_dsn::{java_double_to_string, BoardReadResult};
use fr_geometry::{IntPoint, Line, Point, Polyline};
use fr_router::board_ext::ViaOptimizer;
use fr_router::route_connection;
use fr_settings::sources::DefaultSettings;
use fr_settings::{ExpansionCostFactor, HostEnvironment, RouterSettings, SettingsSource};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: p7t4 <dsn> [mode] [accuracy] [routeK]");
        std::process::exit(2);
    }
    let dsn = std::fs::canonicalize(&args[0])
        .unwrap_or_else(|e| panic!("cannot resolve {}: {e}", args[0]));
    let mode: i32 = args.get(1).map_or(0, |a| a.parse().expect("mode"));
    let accuracy: i32 = args.get(2).map_or(500, |a| a.parse().expect("accuracy"));
    let route_k: usize = args.get(3).map_or(12, |a| a.parse().expect("routeK"));

    let stdout = std::io::stdout();
    let mut out = BufWriter::new(stdout.lock());
    print_header(&mut out, &dsn, mode, accuracy, route_k);

    if mode == 2 {
        tolerance_triples(&mut out);
        out.flush().expect("flush");
        return;
    }

    let mut board = load_board(&dsn);
    let settings = build_settings(&board);

    for connection in pick_connections(&board, route_k) {
        route_one(&mut out, &mut board, &settings, &connection);
    }

    let trace_costs: Option<Vec<ExpansionCostFactor>> = if mode == 3 {
        None
    } else {
        Some(vec![
            ExpansionCostFactor {
                horizontal: 1.0,
                vertical: 1.0,
            };
            board.get_layer_count()
        ])
    };
    writeln!(
        out,
        "sweep regime={} traceCosts={}",
        regime_name(board.rules.trace_angle_restriction),
        match &trace_costs {
            Some(costs) => costs.len().to_string(),
            None => "null".to_string(),
        }
    )
    .expect("write");

    // Snapshotted first, for the reason the Java twin gives: the calls insert and remove items.
    let via_ids: Vec<ItemId> = board
        .get_items()
        .filter(|item| matches!(item, Item::Via(_)))
        .map(Item::id)
        .collect();
    writeln!(out, "vias n={}", via_ids.len()).expect("write");

    for via_id in via_ids {
        if !matches!(board.get_item(via_id), Some(Item::Via(_))) {
            writeln!(out, "via id={} state=GONE", via_id.0).expect("write");
            continue;
        }
        let center = board.drill_center(via_id).expect("a via has a centre");
        let min_width = match board.get_item(via_id) {
            Some(Item::Via(v)) => v.min_width(&board.ctx()),
            _ => unreachable!("just matched"),
        };
        let contacts = contact_ids(&board, via_id);
        let class = classify(&board, via_id);
        let result = if mode == 1 {
            ViaOptimizer::opt_plane_or_fanout_via(&mut board, via_id, accuracy, 10)
        } else {
            ViaOptimizer::opt_via_location(&mut board, via_id, trace_costs.as_deref(), accuracy, 10)
        }
        .expect("ViaOptimizer cannot fail in Java");
        let after = match board.drill_center(via_id) {
            Some(point) if matches!(board.get_item(via_id), Some(Item::Via(_))) => {
                dump_point(&point)
            }
            _ => "gone".to_string(),
        };
        writeln!(
            out,
            "via id={} center={} minWidth={} contacts={contacts} class={class} result={result} after={after}",
            via_id.0,
            dump_point(&center),
            java_double_to_string(min_width),
        )
        .expect("write");
    }

    dump_board(&mut out, &board);
    out.flush().expect("flush");
}

// ------------------------------------------------------------------------------------------------
// Mode 2 — `isWithinTolerance` over the scripted stream
// ------------------------------------------------------------------------------------------------

/// The Java twin's 64-bit LCG, with `wrapping_mul` / `wrapping_add` for Java's silent `long`
/// overflow and `>> 33` on the **unsigned** value for Java's `>>>`. The `% 4001 - 2000` and
/// `% 121 - 20` reductions are on a non-negative `i32`, so Rust's truncating `%` and Java's agree.
fn tolerance_triples<W: Write>(out: &mut W) {
    let mut seed: u64 = 0x0005_DEEC_E66D;
    let next = |seed: &mut u64| -> i32 {
        *seed = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        // Java's `(int) (seed >>> 33)` keeps the low 32 bits of a 31-bit value, so it is
        // non-negative and the cast is lossless.
        (*seed >> 33) as u32 as i32
    };
    for i in 0..10_000 {
        let x1 = next(&mut seed) % 4001 - 2000;
        let y1 = next(&mut seed) % 4001 - 2000;
        let x2 = next(&mut seed) % 4001 - 2000;
        let y2 = next(&mut seed) % 4001 - 2000;
        let tolerance = next(&mut seed) % 121 - 20;
        let p1 = Point::Int(IntPoint::new(x1, y1));
        let p2 = Point::Int(IntPoint::new(x2, y2));
        let answer = ViaOptimizer::is_within_tolerance(Some(&p1), &p2, tolerance);
        writeln!(
            out,
            "tol i={i} p1=({x1},{y1}) p2=({x2},{y2}) t={tolerance} -> {answer}"
        )
        .expect("write");
    }
    for i in 0..256 {
        let x1 = next(&mut seed) % 4001 - 2000;
        let y1 = next(&mut seed) % 4001 - 2000;
        let dx = i % 16;
        let tolerance = i / 16 + dx;
        for delta in -1..=1 {
            let dy = tolerance - dx + delta;
            let p1 = Point::Int(IntPoint::new(x1, y1));
            let p2 = Point::Int(IntPoint::new(x1 + dx, y1 + dy));
            let answer = ViaOptimizer::is_within_tolerance(Some(&p1), &p2, tolerance);
            writeln!(
                out,
                "bnd i={i} d={delta} p1=({x1},{y1}) p2=({},{}) t={tolerance} -> {answer}",
                x1 + dx,
                y1 + dy
            )
            .expect("write");
        }
    }
}

// ------------------------------------------------------------------------------------------------
// The read-only replica of `optViaLocation:39-106`'s dispatch — `P7T4.java`'s `classify`
// ------------------------------------------------------------------------------------------------

fn contact_ids(board: &Board, via: ItemId) -> String {
    let inner: Vec<String> = board
        .normal_contacts(via)
        .into_iter()
        .rev()
        .map(|id| format!("{}:{}", id.0, type_name(board.get_item(id).expect("a contact"))))
        .collect();
    format!("[{}]", inner.join(","))
}

fn classify(board: &Board, via: ItemId) -> &'static str {
    let item = board.get_item(via).expect("a via");
    if item.is_shove_fixed(&board.rules) {
        return "SHOVE_FIXED";
    }
    let contacts: Vec<ItemId> = board.normal_contacts(via).into_iter().rev().collect();
    if contacts.len() == 1 {
        return "PLANE_OR_FANOUT_ONE_CONTACT";
    }
    if contacts.len() != 2 {
        return "WRONG_CONTACT_COUNT";
    }
    let mut traces: Vec<ItemId> = Vec::new();
    let mut is_plane_or_fanout_via = false;
    for contact in contacts {
        let Some(contact_item) = board.get_item(contact) else {
            return "UNUSABLE_CONTACT";
        };
        if contact_item.is_shove_fixed(&board.rules) || !contact_item.is_trace() {
            if matches!(contact_item, Item::ConductionArea(_)) {
                is_plane_or_fanout_via = true;
            } else {
                return "UNUSABLE_CONTACT";
            }
        } else {
            traces.push(contact);
        }
    }
    if is_plane_or_fanout_via {
        return "PLANE_OR_FANOUT_CONDUCTION";
    }
    let via_center = board.drill_center(via).expect("a via has a centre");
    let min_width = match board.get_item(via) {
        Some(Item::Via(v)) => v.min_width(&board.ctx()),
        _ => unreachable!("a via"),
    };
    let tolerance = (min_width / 2.0) as i32 + 1;
    for trace in traces {
        let Some(Item::Trace(t)) = board.get_item(trace) else {
            return "UNUSABLE_CONTACT";
        };
        let first = t.first_corner();
        let last = t.last_corner();
        if !within(first.as_ref(), &via_center, tolerance)
            && !within(last.as_ref(), &via_center, tolerance)
        {
            return "NOT_AT_ENDPOINT";
        }
    }
    "TWO_TRACES"
}

/// `isWithinTolerance:719-732`, re-transcribed here so the replica needs no `pub(crate)` reach —
/// the Java twin's `within` helper does exactly the same.
fn within(p1: Option<&Point>, p2: &Point, tolerance: i32) -> bool {
    let Some(p1) = p1 else {
        return false;
    };
    let fp1 = p1.to_float();
    let fp2 = p2.to_float();
    (fp1.x - fp2.x).abs() + (fp1.y - fp2.y).abs() <= f64::from(tolerance)
}

// ------------------------------------------------------------------------------------------------
// Header, board, settings, routing — `P6T1.java`'s choices, transcribed (`p7t3.rs`'s copy)
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
// Dumps — `P7T3.java`'s, verbatim
// ------------------------------------------------------------------------------------------------

fn regime_name(angle: AngleRestriction) -> &'static str {
    match angle {
        AngleRestriction::NinetyDegree => "NINETY_DEGREE",
        AngleRestriction::FortyFiveDegree => "FORTYFIVE_DEGREE",
        AngleRestriction::None => "NONE",
    }
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
        let mut line = format!(
            "item id={} type={} nets={} cl={} fix={}",
            item.id().0,
            type_name(item),
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
