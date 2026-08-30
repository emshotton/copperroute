//! Rust twin of `scripts/differential/java/P6T1.java` (Plan 6 Task 17), connection level.
//!
//! Reads one DSN, picks the first `maxItems` connections by the deterministic rule `P6T1.java`'s
//! class comment states — `getItems()` order (descending item id, quirk #63) × the item's own net
//! index order, keeping the pairs with a non-empty unconnected set, computed once before any
//! routing — and routes each of them through [`fr_router::route_connection`], i.e. steps 1-5 of
//! `AutorouteConnectionRouter.route` (plan-6 ruling 2's seam). One JSON line per connection,
//! byte-identical to the Java driver's.
//!
//! Everything that decides *what* is compared lives in `P6T1.java`; this file's job is to make the
//! same four choices:
//!
//! 1. **the settings** are `DefaultSettings` + `set_layer_count` + `apply_board_specific_
//!    optimizations`, not a bare `RouterSettings::new()` — `DefaultSettings.java:103` sets
//!    `automaticNeckdown = true` and a bare constructor would take `try_neck_down` out of the
//!    comparison;
//! 2. **`start_marking_changed_area` runs before every connection**, because
//!    `AutoroutePassRunner.java:224` runs it there and quirk #177 makes the presence of
//!    `board.changed_area` observable inside `TraceShover::insert`;
//! 3. **the four `route` parameters** are the `RoutingJob` constructor's
//!    (`BatchAutorouter.java:110-121`): `trace_costs = settings.get_trace_costs()`,
//!    `start_ripup_costs = settings.get_start_ripup_costs()`,
//!    `remove_unconnected_vias = !settings.is_fanout_enabled()` and
//!    `retain_autoroute_database = false`;
//! 4. **nothing above the seam runs** — no `opt_changed_area`, no necked retry, no strict-DRC
//!    rollback, no `finish_autoroute`.
//!
//! Usage: `p6t1 <dsn> [maxItems] [ripupPassNo] [rules|-]`. The fourth slot exists for plan-6
//! ruling 9 (the via-info / via-rule re-pointing register row): the deciding comparison is
//! Java-with-rules against the port-with-rules on `Issue593-BBD_Mars-64.dsn` plus
//! `crates/fr-router/tests/data/ruling-h-redeclare.rules` — see `P6T1.java`'s class comment and
//! Task 8's report §4.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufWriter, Write};
use std::time::UNIX_EPOCH;

use fr_board::prelude::*;
use fr_drc::DesignRulesChecker;
use fr_dsn::java_double_to_string;
use fr_dsn::parser::scope_parameter::DsnReadOptions;
use fr_dsn::BoardReadResult;
use fr_geometry::Point;
use fr_router::route_connection;
use fr_settings::sources::DefaultSettings;
use fr_settings::{HostEnvironment, RouterSettings, SettingsSource};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: p6t1 <dsn> [maxItems] [ripupPassNo] [rules|-]");
        std::process::exit(2);
    }
    let dsn = std::fs::canonicalize(&args[0])
        .unwrap_or_else(|e| panic!("cannot resolve {}: {e}", args[0]));
    let max_items: usize = args.get(1).map_or(8, |a| a.parse().expect("maxItems"));
    let ripup_pass_no: i32 = args.get(2).map_or(1, |a| a.parse().expect("ripupPassNo"));
    let rules = args.get(3).filter(|a| !a.is_empty() && *a != "-").map(|a| {
        std::fs::canonicalize(a).unwrap_or_else(|e| panic!("cannot resolve {a}: {e}"))
    });

    let stdout = std::io::stdout();
    let mut out = BufWriter::new(stdout.lock());

    print_header(&mut out, &dsn, max_items, ripup_pass_no, rules.as_deref());

    let mut board = load_board(&dsn, rules.as_deref());
    let settings = build_settings(&board);

    for connection in pick_connections(&board, max_items) {
        let line = route_one(&mut board, &settings, &connection, ripup_pass_no);
        writeln!(out, "{line}").expect("write");
        out.flush().expect("flush");
    }
}

// ------------------------------------------------------------------------------------------------
// Header, board, settings
// ------------------------------------------------------------------------------------------------

/// `P6T1.main`'s first line. The jar path comes from the environment `run.sh` exports, where Java
/// derives it from its own code source — so a mismatch is a real finding rather than a shared
/// assumption (the `p4t1`/`p5t1` convention).
fn print_header<W: Write>(
    out: &mut W,
    dsn: &std::path::Path,
    max_items: usize,
    pass: i32,
    rules: Option<&std::path::Path>,
) {
    let jar = std::env::var("FREEROUTING_JAR")
        .expect("environment variable FREEROUTING_JAR is not set");
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
        "HEADER jar={} bytes={} mtime={mtime} fixture={} maxItems={max_items} ripupPassNo={pass} \
         rules={}",
        jar.display(),
        meta.len(),
        dsn.file_name().expect("a file name").to_string_lossy(),
        rules.map_or_else(
            || "-".to_string(),
            |p| p.file_name().expect("a file name").to_string_lossy().into_owned()
        ),
    )
    .expect("write");
    if let Ok(exe) = std::env::current_exe() {
        eprintln!("rust-binary {}", exe.display());
    }
}

/// `P6T1.loadBoard`: the DSN, then the optional `.rules` file.
fn load_board(dsn: &std::path::Path, rules: Option<&std::path::Path>) -> Board {
    let file = std::fs::File::open(dsn).unwrap_or_else(|e| panic!("cannot open {dsn:?}: {e}"));
    let design_name = dsn
        .file_name()
        .expect("a file name")
        .to_string_lossy()
        .into_owned();
    let result = fr_dsn::read_board(file, None, Some(&design_name), &DsnReadOptions::default());
    let (mut board, transform) = match result {
        BoardReadResult::Success {
            board,
            coordinate_transform,
            ..
        }
        | BoardReadResult::OutlineMissing {
            board,
            coordinate_transform,
            ..
        } => (
            *board.unwrap_or_else(|| panic!("{design_name} produced no board")),
            coordinate_transform.unwrap_or_else(|| panic!("{design_name} produced no transform")),
        ),
        other => panic!("{design_name} did not read: {other:?}"),
    };
    if let Some(rules) = rules {
        let file =
            std::fs::File::open(rules).unwrap_or_else(|e| panic!("cannot open {rules:?}: {e}"));
        // The base name **without** `.dsn` (`RoutingJob.java:457`) — `RulesReader.java:100-110`
        // compares it against the `(rules PCB <name>` header, so both sides must pass the same
        // string. `None` for the settings: the file's `(autoroute_settings …)` never reaches the
        // board.
        let rules_design_name = design_name.strip_suffix(".dsn").unwrap_or(&design_name);
        let read = fr_dsn::rules_reader::read(file, rules_design_name, &mut board, &transform, None)
            .unwrap_or_else(|e| panic!("{rules:?} did not read: {e:?}"));
        assert!(read, "{rules:?} was rejected by the rules reader");
    }
    board
}

/// `P6T1.main`'s three settings lines: the priority-0 source of the headless ladder, sized and
/// tuned for this board exactly as `RouterSettings(RoutingBoard)` (`RouterSettings.java:127-131`)
/// does it.
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

// ------------------------------------------------------------------------------------------------
// Connection selection
// ------------------------------------------------------------------------------------------------

/// `P6T1.Connection`.
struct Connection {
    k: usize,
    item_id: ItemId,
    net_no: i32,
}

/// `P6T1.pickConnections`.
fn pick_connections(board: &Board, max_items: usize) -> Vec<Connection> {
    let mut result = Vec::new();
    let mut k = 0;
    for item_id in board.items_in_board_order() {
        let Some(item) = board.get_item(item_id) else {
            continue;
        };
        // `instanceof Connectable`, not `Item.isConnectable()`: Java's `getAutorouteItems` writes
        // the bare `instanceof` (`BatchAutorouter.java:357`). The extra `netCount() > 0` the
        // method adds would be invisible here anyway — the loop below is empty on such an item.
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

// ------------------------------------------------------------------------------------------------
// One connection
// ------------------------------------------------------------------------------------------------

/// `P6T1.routeOne`.
fn route_one(
    board: &mut Board,
    settings: &RouterSettings,
    connection: &Connection,
    ripup_pass_no: i32,
) -> String {
    let mut sb = String::new();
    sb.push_str(&format!(
        "{{\"k\":{},\"item\":{},\"net\":{}",
        connection.k,
        connection.item_id.0,
        connection.net_no
    ));

    if board.get_item(connection.item_id).is_none() {
        sb.push_str(",\"state\":\"GONE\"}");
        return sb;
    }

    board.start_marking_changed_area();
    let mut ripped: BTreeSet<ItemId> = BTreeSet::new();
    let mut ripup_costs: BTreeMap<ItemId, i32> = BTreeMap::new();
    let max_id_before = board.communication.id_gen.max_generated_id();

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
        ripup_pass_no,
        settings.get_start_ripup_costs(),
        !settings.is_fanout_enabled(),
        false,
        &|| false,
    );

    sb.push_str(&format!(",\"state\":\"{}\"", result.state.name()));
    // `unwrap_or("")`, not a `null`, and that is correct rather than a gap. Java's `quote`
    // (P6T1.java:502-505) renders a `null` as the bare token `null`, so a null `details` would
    // print differently on the two sides — but Java cannot produce one:
    // `AutorouteAttemptResult.java:10-19` assigns `this.details = ""` in the one-argument
    // constructor, and all 21 `new AutorouteAttemptResult(...)` sites in `src/main/java` pass
    // either no details or a string literal/concatenation. The port's `None` is the model of
    // Java's `""` (see `attempt.rs`'s field doc), so rendering `null` here would be the
    // divergence. Task 17 review N1, closed by Task 18.
    sb.push_str(&format!(
        ",\"details\":{}",
        quote(result.details.as_deref().unwrap_or(""))
    ));
    // Java's `rippedItemList` is a `TreeSet<Item>`, i.e. **descending** id (quirk #44); the port's
    // is a `BTreeSet<ItemId>`, i.e. ascending. The rendering reverses so the two lists read the
    // same way — the *set* is the comparison surface, and reversing here means a genuine
    // membership difference still shows.
    sb.push_str(",\"ripped\":[");
    for (i, id) in ripped.iter().rev().enumerate() {
        if i > 0 {
            sb.push(',');
        }
        sb.push_str(&id.0.to_string());
    }
    sb.push(']');
    // `ripupCosts` is a `LinkedHashMap` in Java — insertion order — and a `BTreeMap` here. Java's
    // insertion order on this map is the order `MazeRipupResolver` prices the candidates in, which
    // no consumer below the seam reads back, so the rendering sorts both sides by item id.
    sb.push_str(",\"ripupCosts\":[");
    for (i, (id, cost)) in ripup_costs.iter().enumerate() {
        if i > 0 {
            sb.push(',');
        }
        sb.push_str(&format!("[{},{}]", id.0, cost));
    }
    sb.push(']');
    sb.push_str(&format!(",\"maxIdBefore\":{}", max_id_before.0));
    sb.push_str(&format!(
        ",\"maxIdAfter\":{}",
        board.communication.id_gen.max_generated_id().0
    ));
    append_inserted_geometry(&mut sb, board, max_id_before);
    append_metrics(&mut sb, board, connection.net_no);
    sb.push('}');
    sb
}

/// `P6T1.appendInsertedGeometry` — ruling 1(b).
fn append_inserted_geometry(sb: &mut String, board: &Board, max_id_before: ItemId) {
    // `items_in_board_order()` is descending, so the reverse is Java's `Collections.reverse` of
    // the same walk: ascending id, i.e. insertion order.
    let inserted: Vec<ItemId> = board
        .items_in_board_order()
        .into_iter()
        .filter(|id| id.0 > max_id_before.0)
        .rev()
        .collect();

    sb.push_str(",\"traces\":[");
    let mut first = true;
    for id in &inserted {
        let Some(Item::Trace(trace)) = board.get_item(*id) else {
            continue;
        };
        if !first {
            sb.push(',');
        }
        first = false;
        sb.push_str(&format!(
            "{{\"id\":{},\"layer\":{},\"halfWidth\":{},\"corners\":{}}}",
            id.0,
            trace.get_layer(),
            trace.get_half_width(),
            corners(trace.polyline())
        ));
    }
    sb.push_str("],\"vias\":[");
    first = true;
    let ctx = board.ctx();
    for id in &inserted {
        let Some(Item::Via(via)) = board.get_item(*id) else {
            continue;
        };
        if !first {
            sb.push(',');
        }
        first = false;
        let padstack = via
            .get_padstack(&ctx)
            .expect("a via always resolves its padstack");
        sb.push_str(&format!(
            "{{\"id\":{},\"center\":\"{}\",\"padstack\":{},\"firstLayer\":{},\"lastLayer\":{}}}",
            id.0,
            pt(&via.get_center()),
            quote(&padstack.name),
            via.first_layer(&ctx),
            via.last_layer(&ctx)
        ));
    }
    sb.push(']');

    let other = inserted
        .iter()
        .filter(|id| !matches!(board.get_item(**id), Some(Item::Trace(_) | Item::Via(_))))
        .count();
    sb.push_str(&format!(",\"otherInserted\":{other}"));
}

/// `P6T1.appendMetrics` — ruling 1(c).
fn append_metrics(sb: &mut String, board: &mut Board, net_no: i32) {
    let vias = board.net_via_count(net_no);
    let trace_length = board.cumulative_trace_length();
    let mut drc = DesignRulesChecker::new(board);
    let incompletes = drc.get_incomplete_count();
    let violations = drc.get_all_clearance_violations().len();
    sb.push_str(&format!(
        ",\"metrics\":{{\"incompletes\":{incompletes},\"vias\":{vias},\"traceLength\":\"{}\",\"violations\":{violations}}}",
        java_double_to_string(trace_length)
    ));
}

// ------------------------------------------------------------------------------------------------
// Rendering
// ------------------------------------------------------------------------------------------------

/// `P6T1.pt`.
fn pt(p: &Point) -> String {
    match p {
        Point::Int(ip) => format!("({},{})", ip.x, ip.y),
        Point::Rational(_) => {
            let f = p.to_float();
            format!(
                "~({},{})",
                java_double_to_string(f.x),
                java_double_to_string(f.y)
            )
        }
    }
}

/// `P6T1.corners`.
fn corners(p: &fr_geometry::Polyline) -> String {
    let mut sb = String::from("[");
    for i in 0..p.corner_count() {
        if i > 0 {
            sb.push(',');
        }
        match p.corner(i) {
            Some(Point::Int(ip)) => sb.push_str(&format!("\"({},{})\"", ip.x, ip.y)),
            _ => {
                let f = p.corner_approx(i).expect("a corner of a valid polyline");
                sb.push_str(&format!(
                    "\"~({},{})\"",
                    java_double_to_string(f.x),
                    java_double_to_string(f.y)
                ));
            }
        }
    }
    sb.push(']');
    sb
}

/// `P6T1.quote`.
fn quote(value: &str) -> String {
    let mut sb = String::from("\"");
    for c in value.chars() {
        match c {
            '"' => sb.push_str("\\\""),
            '\\' => sb.push_str("\\\\"),
            '\n' => sb.push_str("\\n"),
            '\r' => sb.push_str("\\r"),
            '\t' => sb.push_str("\\t"),
            c if (c as u32) < 0x20 => sb.push_str(&format!("\\u{:04x}", c as u32)),
            c => sb.push(c),
        }
    }
    sb.push('"');
    sb
}
