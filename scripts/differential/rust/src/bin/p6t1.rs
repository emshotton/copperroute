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
//! Usage: `p6t1 <dsn> [maxItems] [ripupPassNo] [rules|-] [1-5|1-8] [neckWidthUm]`. The fourth
//! slot exists for plan-6 ruling 9 (the via-info / via-rule re-pointing register row): the
//! deciding comparison is Java-with-rules against the port-with-rules on
//! `Issue593-BBD_Mars-64.dsn` plus `crates/fr-router/tests/data/ruling-h-redeclare.rules` — see
//! `P6T1.java`'s class comment and Plan 6 Task 8's report §4.
//!
//! # `steps` — Plan 7 Task 8
//!
//! `1-5` is Plan 6's slice, [`fr_router::route_connection`], and is what
//! `tests/reference/<stem>/router.jsonl` was generated with; that path and its output are
//! untouched. `1-8` calls [`fr_router::route_connection_full`], i.e.
//! `AutorouteConnectionRouter.route` in full — step 6's `optChangedArea` on `ROUTED`, step 7's
//! necked retry and step 8's strict-DRC rollback. `neckWidthUm` seeds `settings.neckWidthUm`,
//! which `DefaultSettings.java:109` leaves at `0.0` and `route:125` gates the whole necked retry
//! on, so without it step 7 is unreachable.
//!
//! # The budget, and why the two sides are *not* configured the same
//!
//! Controller ruling AI asks a `p7t*` parity run to disable the 1000 ms
//! `TIME_LIMIT_TO_PREVENT_ENDLESS_LOOP` on both sides, and the task brief suggested reflecting
//! the Java constant to `0`. **That cannot be done**: `AutorouteConnectionRouter.java:22` is a
//! compile-time constant and `javac` inlines it — `javap -c` on the shipping jar shows
//! `sipush 1000` immediately before each `invokevirtual RoutingBoard.optChangedArea`, so no
//! reflective write reaches the call site.
//!
//! This side therefore runs with [`RouterBudget::disabled`], i.e. **no** limit, against the jar's
//! live 1000 ms one — and that asymmetry is *stronger* evidence than a matched constant. The two
//! sweeps can only agree if the Java limit never trips: had it tripped anywhere on the corpus,
//! the jar's tightener would have stopped mid-sweep and left a board this side kept optimising.
//! A MATCH is therefore a proof that `--steps=1-8` parity is time-independent, which is what
//! ruling AI is for. `scripts/differential/java/probes/P7T8Probe.java` carries the same note.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufWriter, Write};
use std::time::UNIX_EPOCH;

use fr_board::prelude::*;
use fr_drc::DesignRulesChecker;
use fr_dsn::format_double;
use fr_dsn::parser::scope_parameter::DsnReadOptions;
use fr_dsn::BoardReadResult;
use fr_geometry::Point;
use fr_router::autoroute::maze::ViaPricing;
use fr_router::pipeline::RouterBudget;
use fr_router::{route_connection, route_connection_full};
use fr_settings::sources::DefaultSettings;
use fr_settings::{HostEnvironment, RouterSettings, SettingsSource};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: p6t1 <dsn> [maxItems] [ripupPassNo] [rules|-] [1-5|1-8] [neckWidthUm]");
        std::process::exit(2);
    }
    let dsn = std::fs::canonicalize(&args[0])
        .unwrap_or_else(|e| panic!("cannot resolve {}: {e}", args[0]));
    let max_items: usize = args.get(1).map_or(8, |a| a.parse().expect("maxItems"));
    let ripup_pass_no: i32 = args.get(2).map_or(1, |a| a.parse().expect("ripupPassNo"));
    let rules = args
        .get(3)
        .filter(|a| !a.is_empty() && *a != "-")
        .map(|a| std::fs::canonicalize(a).unwrap_or_else(|e| panic!("cannot resolve {a}: {e}")));
    let steps: &str = args
        .get(4)
        .filter(|a| !a.is_empty())
        .map_or("1-5", String::as_str);
    assert!(
        steps == "1-5" || steps == "1-8",
        "steps must be 1-5 or 1-8, not {steps}"
    );
    let neck_width_um: f64 = args.get(5).map_or(0.0, |a| a.parse().expect("neckWidthUm"));

    let stdout = std::io::stdout();
    let mut out = BufWriter::new(stdout.lock());

    print_header(
        &mut out,
        &dsn,
        max_items,
        ripup_pass_no,
        rules.as_deref(),
        steps,
        neck_width_um,
    );

    let mut board = load_board(&dsn, rules.as_deref());
    let mut settings = build_settings(&board);
    // `P6T1.main`: after the two board-dependent steps, so neither can overwrite it.
    settings.neck_width_um = Some(neck_width_um);

    // Plan 7 Task 15b. Off unless `P7T15B_PREPARE` is set, so a run without the variable
    // executes exactly the code it executed before this switch existed and every committed
    // `tests/reference/*/router.meta.txt` stays byte-identical. With it, both sides apply
    // `HeadlessBoardManager`'s two board-mutating clearance overrides to the loaded board —
    // `prepare_board` here, `applyCopperToEdgeClearanceOverride`/`applyHoleClearanceOverride`
    // through a `HeadlessBoardManager` on the Java side — before a single connection is routed.
    // That is what a real `-de <dsn> -do <ses>` run does and what Plan 7 Task 16's references
    // encode; `run.sh p6t1 fixtures/Issue026-J2_reference.dsn 45` under the variable is the
    // whole-board evidence that the port's `board_edge` class routes the same as the jar's.
    if std::env::var_os("P7T15B_PREPARE").is_some() {
        let changed = fr_router::pipeline::prepare_board(&mut board, &settings);
        eprintln!("p7t15b-prepare changed={changed}");
    }

    for connection in pick_connections(&board, max_items) {
        let line = route_one(&mut board, &settings, &connection, ripup_pass_no, steps);
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
#[allow(clippy::too_many_arguments)]
fn print_header<W: Write>(
    out: &mut W,
    dsn: &std::path::Path,
    max_items: usize,
    pass: i32,
    rules: Option<&std::path::Path>,
    steps: &str,
    neck_width_um: f64,
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
    // `P6T1.main`: the `1-5` header is byte-identical to the one every committed
    // `router.meta.txt` records; the two extra fields appear only under `1-8`.
    let steps_suffix = if steps == "1-8" {
        format!(
            " steps={steps} neckWidthUm={}",
            format_double(neck_width_um)
        )
    } else {
        String::new()
    };
    writeln!(
        out,
        "HEADER jar={} bytes={} mtime={mtime} fixture={} maxItems={max_items} ripupPassNo={pass} \
         rules={}{steps_suffix}",
        jar.display(),
        meta.len(),
        dsn.file_name().expect("a file name").to_string_lossy(),
        rules.map_or_else(
            || "-".to_string(),
            |p| p
                .file_name()
                .expect("a file name")
                .to_string_lossy()
                .into_owned()
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
        let read =
            fr_dsn::rules_reader::read(file, rules_design_name, &mut board, &transform, None)
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
            result.push(Connection { k, item_id, net_no });
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
    steps: &str,
) -> String {
    // Plan 7 Task 8b: the per-connection separator for the level-7 bisect ledgers, the twin of
    // `P6T1.routeOne`'s. The ledgers themselves (`CHG` in `fr_board`'s `change_trace`, `OCA*` in
    // `fr_router`'s `opt_changed_area`) print to **stderr** without saying which connection they
    // belong to; this line is what splits the stream. Gated on the same two variables, so a run
    // without them writes nothing.
    if std::env::var_os("P7T8B_CHANGE").is_some() || std::env::var_os("P7T8B_OCA").is_some() {
        eprintln!(
            "CONN k={} item={} net={}",
            connection.k, connection.item_id.0, connection.net_no
        );
    }
    let mut sb = String::new();
    sb.push_str(&format!(
        "{{\"k\":{},\"item\":{},\"net\":{}",
        connection.k, connection.item_id.0, connection.net_no
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
    let result = if steps == "1-8" {
        // `P7T8Probe.routeFull` -> `BatchAutorouter.autorouteItem` -> the whole of
        // `AutorouteConnectionRouter.route`. The two extra arguments are what steps 6-8 read:
        // `getTracePullTightAccuracy()` — the `RoutingJob` constructor's value
        // (`BatchAutorouter.java:118-120`) — and ruling AI's budget, **disabled** here; see the
        // module comment for why the two sides are deliberately not configured the same.
        route_connection_full(
            board,
            &mut engine,
            connection.item_id,
            connection.net_no,
            settings,
            &trace_costs,
            ViaPricing::ByPadstackRadius,
            &mut ripped,
            &mut ripup_costs,
            ripup_pass_no,
            settings.get_start_ripup_costs(),
            !settings.is_fanout_enabled(),
            settings.trace_pull_tight_accuracy.unwrap_or(500),
            RouterBudget::disabled(),
            &|| false,
        )
    } else {
        route_connection(
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
        )
    };

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
    // `P6T1_DUMP_BOARD=1` — see `P6T1.routeOne`'s comment. A bisection tool, off by default.
    if std::env::var_os("P6T1_DUMP_BOARD").is_some() {
        // `P6T1.java` passes `-1` where this passes `ItemId(0)`: the port's ids are unsigned and
        // `ItemIdGenerator` hands out `1` first (`ItemIdGenerator.java:37-55`), so `> 0` and
        // `> -1` select the same items on every board either side can build.
        append_inserted_geometry(&mut sb, board, ItemId(0));
        append_board_state(&mut sb, board);
    }
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
    let (incompletes, all_violations) = {
        let mut drc = DesignRulesChecker::new(board);
        (drc.get_incomplete_count(), drc.get_all_violations())
    };
    let violations = all_violations
        .iter()
        .filter(|violation| violation.involves_routing(board))
        .count();
    sb.push_str(&format!(
        ",\"metrics\":{{\"incompletes\":{incompletes},\"vias\":{vias},\"traceLength\":\"{}\",\"violations\":{violations}}}",
        format_double(trace_length)
    ));
}

// ------------------------------------------------------------------------------------------------
// Rendering
// ------------------------------------------------------------------------------------------------

/// `P6T1.appendBoardState` — the non-geometric half of the `P6T1_DUMP_BOARD` bisection dump.
fn append_board_state(sb: &mut String, board: &Board) {
    sb.push_str(",\"state2\":[");
    // A `first` flag rather than the loop index: an id the board does not resolve is skipped, and
    // an index-based separator would emit a leading comma if that happened at index 0.
    let mut first = true;
    for id in board.items_in_board_order() {
        let Some(item) = board.get_item(id) else {
            continue;
        };
        if !first {
            sb.push(',');
        }
        first = false;
        let kind = match item {
            Item::Trace(_) => "PolylineTrace",
            Item::Via(_) => "Via",
            Item::Pin(_) => "Pin",
            Item::ObstacleArea(_) => "ObstacleArea",
            Item::ConductionArea(_) => "ConductionArea",
            Item::ViaObstacleArea(_) => "ViaObstacleArea",
            Item::ComponentObstacleArea(_) => "ComponentObstacleArea",
            Item::BoardOutline(_) => "BoardOutline",
            Item::ComponentOutline(_) => "ComponentOutline",
        };
        let nets: Vec<String> = item.net_nos().iter().map(i32::to_string).collect();
        sb.push_str(&format!(
            "\"{}|{}|{}|{}|{}\"",
            id.0,
            kind,
            nets.join(","),
            item.clearance_class(),
            item.get_fixed_state() as i32
        ));
    }
    sb.push(']');
    sb.push_str(",\"changedArea\":");
    let Some(changed_area) = board.changed_area.as_ref() else {
        sb.push_str("null");
        return;
    };
    sb.push('[');
    for layer in 0..board.get_layer_count() {
        if layer > 0 {
            sb.push(',');
        }
        sb.push_str(&format!("\"{}\"", changed_area.get_area(layer)));
    }
    sb.push(']');
}

/// `P6T1.pt`.
fn pt(p: &Point) -> String {
    match p {
        Point::Int(ip) => format!("({},{})", ip.x, ip.y),
        Point::Rational(_) => {
            let f = p.to_float();
            format!(
                "~({},{})",
                format_double(f.x),
                format_double(f.y)
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
                    format_double(f.x),
                    format_double(f.y)
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
