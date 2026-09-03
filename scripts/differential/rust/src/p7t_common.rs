//! The helpers `p7t1` and `p7t2` share — the Rust twin of `P7T2.java`'s four shared statics plus
//! its rendering block.
//!
//! It sits in `src/` rather than in `src/bin/` and is `#[path = "../p7t_common.rs"]`-included by
//! both binaries — the `token_dump.rs` / `drc_common.rs` precedent. A file under `src/bin/` is a
//! *binary target* of its own, so it would have to declare a `main`; `cargo build --bin p7t1`
//! does not notice, `cargo clippy --all-targets` does. The duplication this costs is one `mod`
//! line per driver; the duplication it avoids is the whole board/settings/router ladder, which is
//! exactly the thing that must not drift between the two.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::time::UNIX_EPOCH;

use fr_board::prelude::*;
use fr_board::StopConnectionOption;
use fr_drc::DesignRulesChecker;
use fr_dsn::java_double_to_string;
use fr_dsn::parser::scope_parameter::DsnReadOptions;
use fr_dsn::BoardReadResult;
use fr_geometry::{Point, Polyline};
use fr_router::pipeline::{
    BatchAutorouter, NoopProgressSink, RouterBudget, RouterStop, RoutingFailureLog,
};
use fr_router::AutorouteAttemptState;
use fr_settings::sources::DefaultSettings;
use fr_settings::{HostEnvironment, RouterSettings, SettingsSource};

// ------------------------------------------------------------------------------------------------
// Header
// ------------------------------------------------------------------------------------------------

/// The three header fields Java derives from its own code source. `run.sh` exports the path, so a
/// mismatch is a real finding rather than a shared assumption (the `p4t1`/`p5t1` convention).
pub fn jar_identity() -> (String, u64, u128) {
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
    (jar.display().to_string(), meta.len(), mtime)
}

// ------------------------------------------------------------------------------------------------
// Board, settings, router — `P7T2.loadBoard` / `buildSettings` / `newRouter` / `runPass`
// ------------------------------------------------------------------------------------------------

/// `P7T2.loadBoard`: the DSN, with no `.rules` file — neither driver takes one.
pub fn load_board(dsn: &std::path::Path) -> Board {
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

/// [`load_board`] plus the DSN coordinate transform, which `fr_dsn::ses_writer::write` needs and
/// Java reads off `board.communication.coordinateTransform` (Plan 3 ruling A keeps it in
/// `fr-dsn`, so the port hands it back on the read result instead). Plan 7 Task 16's `p7t9
/// batch`.
pub fn load_board_with_transform(dsn: &std::path::Path) -> (Board, fr_dsn::CoordinateTransform) {
    let file = std::fs::File::open(dsn).unwrap_or_else(|e| panic!("cannot open {dsn:?}: {e}"));
    let design_name = dsn
        .file_name()
        .expect("a file name")
        .to_string_lossy()
        .into_owned();
    let result = fr_dsn::read_board(file, None, Some(&design_name), &DsnReadOptions::default());
    match result {
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
            coordinate_transform
                .unwrap_or_else(|| panic!("{design_name} produced no coordinate transform")),
        ),
        other => panic!("{design_name} did not read: {other:?}"),
    }
}

/// `P7T2.buildSettings` — the priority-0 source of the headless ladder, sized and tuned for this
/// board exactly as `RouterSettings(RoutingBoard)` (`RouterSettings.java:127-131`) does it.
pub fn build_settings(board: &Board) -> RouterSettings {
    let host = HostEnvironment::detect();
    let mut settings = DefaultSettings::new(&host)
        .get_settings()
        .expect("DefaultSettings always answers a table")
        .clone();
    settings.set_layer_count(board.get_layer_count());
    settings.apply_board_specific_optimizations(board);
    settings
}

/// `P7T2.newRouter` — `new BatchAutorouter(RoutingJob)` (`BatchAutorouter.java:110-122`).
///
/// Ruling AI's budget is **disabled** on this side against Java's live 1000 ms limit; see
/// `P7T8Probe`'s class comment for why the two are deliberately not configured the same, and why
/// a MATCH under that asymmetry is the stronger evidence.
pub fn new_router<'a>(board: &Board, settings: &'a RouterSettings) -> BatchAutorouter<'a> {
    BatchAutorouter::for_routing_job(board, settings, RouterBudget::disabled())
}

/// The two pieces of pass-to-pass state Java hangs off objects the port does not have: the
/// failure log (a `RoutingBoard` field there, a caller-owned value here — see
/// `RoutingFailureLog`'s ownership note) and the stop flag (`router.thread`).
pub struct PassState {
    pub stop: RouterStop,
    pub failure_log: RoutingFailureLog,
    pub sink: NoopProgressSink,
}

impl Default for PassState {
    fn default() -> PassState {
        PassState {
            stop: RouterStop::new(),
            failure_log: RoutingFailureLog::new(),
            sink: NoopProgressSink,
        }
    }
}

/// `P7T2.runPass` — `BatchAutorouter.autoroutePass` (`:415-421`), i.e. the real
/// `runSingleThread`. Used for the `passNo - 1` warm-up passes and for `p7t2`'s `[real]` half.
pub fn run_pass(board: &mut Board, router: &mut BatchAutorouter<'_>, pass_no: i32) -> bool {
    let mut state = PassState::default();
    run_pass_with(board, router, &mut state, pass_no)
}

/// [`run_pass`] against a caller-held [`PassState`], so a driver that needs several passes keeps
/// one stop flag and one failure log across them, as Java's `router.thread` and
/// `board.failureLog` are kept.
pub fn run_pass_with(
    board: &mut Board,
    router: &mut BatchAutorouter<'_>,
    state: &mut PassState,
    pass_no: i32,
) -> bool {
    router
        .autoroute_pass(
            board,
            &mut state.failure_log,
            pass_no,
            &state.stop,
            &mut state.sink,
        )
        .expect("run_single_thread's boundary answers Ok on every path")
}

// ------------------------------------------------------------------------------------------------
// getAutorouteItems — the `p7t1` payload
// ------------------------------------------------------------------------------------------------

/// `P7T2.dumpAutorouteItems`: `COUNT`, one `ITEM` line per entry of the work list **in list
/// order**, the per-step `HANDLED` trace, and `HANDLED-FINAL`.
///
/// The `HANDLED` lines are a transcription of `:363-370` run beside the real method, exactly as
/// the Java driver does it; `HANDLED-FINAL`'s `transcriptionAgrees` checks the transcription
/// against what the real call built — through
/// [`BatchAutorouter::autoroute_items_with_handled`] here, and through
/// `Field.setAccessible(true)` on `reusableHandledItems` there.
pub fn dump_autoroute_items<W: Write>(out: &mut W, router: &BatchAutorouter<'_>, board: &Board) {
    let (items, real_handled) = router.autoroute_items_with_handled(board);
    writeln!(out, "COUNT {}", items.len()).expect("write");
    for (ordinal, id) in items.iter().enumerate() {
        let item = board.get_item(*id).expect("the list holds live items");
        let nets: Vec<String> = (0..item.net_count())
            .map(|i| item.get_net_number(i).to_string())
            .collect();
        writeln!(
            out,
            "ITEM {ordinal} {} {} nets={} [{}]",
            id.0,
            java_class_name(item),
            item.net_count(),
            nets.join(",")
        )
        .expect("write");
    }

    // The transcription of `:351-370`, printed step by step.
    let mut handled: BTreeSet<ItemId> = BTreeSet::new();
    let mut step = 0usize;
    for current in board.items_in_board_order() {
        let Some(item) = board.get_item(current) else {
            continue;
        };
        if item.as_connectable().is_none() || item.is_routable() || handled.contains(&current) {
            continue;
        }
        for i in 0..item.net_count() {
            let connected_set = board.connected_set(current, item.get_net_number(i), false);
            for connected in &connected_set {
                if board
                    .get_item(*connected)
                    .is_some_and(|c| c.net_count() <= 1)
                {
                    handled.insert(*connected);
                }
            }
            writeln!(
                out,
                "HANDLED {step} {} {}",
                handled.len(),
                id_list(handled.iter().copied())
            )
            .expect("write");
            step += 1;
        }
    }

    writeln!(
        out,
        "HANDLED-FINAL {} {} transcriptionAgrees={}",
        real_handled.len(),
        id_list(real_handled.iter().copied()),
        real_handled == handled
    )
    .expect("write");
}

fn id_list(ids: impl Iterator<Item = ItemId>) -> String {
    let rendered: Vec<String> = ids.map(|id| id.0.to_string()).collect();
    format!("[{}]", rendered.join(","))
}

/// Java's `item.getClass().getSimpleName()`, as `p6t1`'s `append_board_state` renders it.
pub fn java_class_name(item: &Item) -> &'static str {
    match item {
        Item::Trace(_) => "PolylineTrace",
        Item::Via(_) => "Via",
        Item::Pin(_) => "Pin",
        Item::ObstacleArea(_) => "ObstacleArea",
        Item::ConductionArea(_) => "ConductionArea",
        Item::ViaObstacleArea(_) => "ViaObstacleArea",
        Item::ComponentObstacleArea(_) => "ComponentObstacleArea",
        Item::BoardOutline(_) => "BoardOutline",
        Item::ComponentOutline(_) => "ComponentOutline",
    }
}

// ------------------------------------------------------------------------------------------------
// runSingleThread, transcribed — the `p7t2` payload
// ------------------------------------------------------------------------------------------------

/// `P7T2.transcribeRunSingleThread`: `runSingleThread`'s loop (`:158-330`) written out here, with
/// the real [`BatchAutorouter::autoroute_items`], [`BatchAutorouter::autoroute_item`] and
/// [`BatchAutorouter::remove_tails`] called from it and one JSON line printed per
/// `(item, net index)`.
///
/// See `P7T2.java`'s class comment for why the driver has both a transcription and a `[real]`
/// half, and what the second one proves about the first.
pub fn transcribe_run_single_thread<W: Write>(
    out: &mut W,
    board: &mut Board,
    router: &mut BatchAutorouter<'_>,
    state: &mut PassState,
    pass_no: i32,
) -> bool {
    // :158.
    let autoroute_item_list = router.autoroute_items(board);
    // :163-166.
    if autoroute_item_list.is_empty() {
        writeln!(out, "EMPTY").expect("write");
        return false;
    }

    let mut not_routed = 0i32;
    let mut routed = 0i32;
    let mut skipped = 0i32;
    let mut ripped_item_count = 0i32;
    let mut items_to_go_count = autoroute_item_list.len() as i32;
    let mut k = 0usize;

    // :202.
    for current_item in autoroute_item_list {
        // :203-205.
        if state.stop.is_stop_auto_router_requested() {
            break;
        }
        // :207.
        let net_count = board.get_item(current_item).map_or(0, |i| i.net_count());
        for i in 0..net_count {
            // :208-210.
            if state.stop.is_stop_auto_router_requested() {
                break;
            }
            // :212-221.
            let max_items = router.settings().max_items;
            if max_items.is_some_and(|max| max > 0 && router.total_items_routed >= max) {
                writeln!(
                    out,
                    "MAXITEMS k={k} totalItemsRouted={}",
                    router.total_items_routed
                )
                .expect("write");
                // fixed: T9 (#202) — the real `AutoroutePassRunner` calls
                // `request_stop_auto_router()` here where Java's `:219` calls `requestStop()`, so
                // the transcription follows it. A `p7t2` MISMATCH confined to what the stop flag
                // reads after a `maxItems` run is that fix, not the port drifting.
                state.stop.request_stop_auto_router();
                break;
            }
            // :222-223.
            router.total_items_routed += 1;
            board.start_marking_changed_area();

            // :225-226.
            let mut ripped_item_list: BTreeSet<ItemId> = BTreeSet::new();
            let mut ripped_item_costs: BTreeMap<ItemId, i32> = BTreeMap::new();

            let route_net_no = board
                .get_item(current_item)
                .map_or(-1, |item| item.get_net_number(i));
            let mut sb = format!(
                "{{\"k\":{k},\"item\":{},\"netIndex\":{i},\"net\":{route_net_no}",
                current_item.0
            );
            k += 1;
            let max_id_before = board.communication.id_gen.max_generated_id();

            // :238-245. `isStopRequested()` (`ALL`), not `isStopAutoRouterRequested()`: Java
            // hands `router.thread` to `initAutoroute` and `optChangedArea`, both of which poll
            // `Stoppable.isStopRequested` (`AutorouteEngine.java:294-303`).
            let mut engine = None;
            let stop_check = &|| state.stop.is_stop_requested();
            let result = router.autoroute_item(
                board,
                &mut engine,
                current_item,
                route_net_no,
                &mut ripped_item_list,
                &mut ripped_item_costs,
                pass_no,
                stop_check,
            );

            sb.push_str(&format!(",\"state\":\"{}\"", result.state.name()));
            // `unwrap_or("")` rather than a `null`: see `p6t1.rs`'s comment — Java's `details` is
            // `""` on every construction site, and the port's `None` is the model of that.
            sb.push_str(&format!(
                ",\"details\":{}",
                quote(result.details.as_deref().unwrap_or(""))
            ));
            // Java's `TreeSet<Item>` is descending (quirk #44); the rendering reverses so the two
            // lists read the same way, the `p6t1` convention.
            sb.push_str(",\"ripped\":[");
            for (n, id) in ripped_item_list.iter().rev().enumerate() {
                if n > 0 {
                    sb.push(',');
                }
                sb.push_str(&id.0.to_string());
            }
            sb.push(']');
            sb.push_str(",\"ripupCosts\":[");
            for (n, (id, cost)) in ripped_item_costs.iter().enumerate() {
                if n > 0 {
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

            // :260-289.
            match result.state {
                AutorouteAttemptState::Routed => routed += 1,
                AutorouteAttemptState::AlreadyConnected
                | AutorouteAttemptState::NoUnconnectedNets
                | AutorouteAttemptState::ConnectedToPlane => skipped += 1,
                _ => {
                    state.failure_log.record_failure(
                        board,
                        current_item,
                        pass_no,
                        result.state,
                        result.details.as_deref(),
                    );
                    sb.push_str(&format!(
                        ",\"failures\":{}",
                        state.failure_log.failure_count(current_item)
                    ));
                    not_routed += 1;
                }
            }
            // :290-291.
            items_to_go_count -= 1;
            ripped_item_count += ripped_item_list.len() as i32;
            sb.push('}');
            writeln!(out, "{sb}").expect("write");
            out.flush().expect("flush");
        }
    }

    // :296-303.
    writeln!(out, "TAILS-BEFORE {}", board_shape(board)).expect("write");
    let tail_stop = &|| state.stop.is_stop_requested();
    if router.is_remove_unconnected_vias() {
        router
            .remove_tails(board, None, StopConnectionOption::None, tail_stop)
            .expect("removeTails");
    } else {
        router
            .remove_tails(board, None, StopConnectionOption::FanoutVia, tail_stop)
            .expect("removeTails");
    }
    writeln!(out, "TAILS-AFTER {}", board_shape(board)).expect("write");

    // :313-320.
    writeln!(
        out,
        "COUNTERS pass={pass_no} queued={items_to_go_count} skipped={skipped} \
         ripped={ripped_item_count} failed={not_routed} routed={routed} incomplete={}",
        BatchAutorouter::calculate_incomplete_count(board)
    )
    .expect("write");

    // :330.
    routed > 0 || not_routed > 0
}

// ------------------------------------------------------------------------------------------------
// Rendering — the `p6t1` conventions
// ------------------------------------------------------------------------------------------------

/// `P7T2.boardShape` — the closing tail-removal / `optChangedArea` delta line.
pub fn board_shape(board: &mut Board) -> String {
    let mut items = 0usize;
    let mut traces = 0usize;
    let mut vias = 0usize;
    for id in board.items_in_board_order() {
        let Some(item) = board.get_item(id) else {
            continue;
        };
        items += 1;
        match item {
            Item::Trace(_) => traces += 1,
            Item::Via(_) => vias += 1,
            _ => {}
        }
    }
    let mut drc = DesignRulesChecker::new(board);
    let incompletes = drc.get_incomplete_count();
    format!(
        "items={items} traces={traces} vias={vias} incompletes={incompletes} traceLength=\"{}\"",
        java_double_to_string(board.cumulative_trace_length())
    )
}

/// `P7T2.appendInsertedGeometry` (the `P6T1` original).
pub fn append_inserted_geometry(sb: &mut String, board: &Board, max_id_before: ItemId) {
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

/// `P6T1.pt`.
pub fn pt(p: &Point) -> String {
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
pub fn corners(p: &Polyline) -> String {
    let mut rendered = Vec::with_capacity(p.corner_count());
    for i in 0..p.corner_count() {
        match p.corner(i) {
            Some(Point::Int(ip)) => rendered.push(format!("\"({},{})\"", ip.x, ip.y)),
            _ => {
                let f = p.corner_approx(i).expect("a corner of a valid polyline");
                rendered.push(format!(
                    "\"~({},{})\"",
                    java_double_to_string(f.x),
                    java_double_to_string(f.y)
                ));
            }
        }
    }
    format!("[{}]", rendered.join(","))
}

/// `P6T1.quote`.
pub fn quote(value: &str) -> String {
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

// ------------------------------------------------------------------------------------------------
// The final board — `P6T15aProbe`'s polyline format
// ------------------------------------------------------------------------------------------------

/// `P7T9.dumpBoard` — one line per item in `getItems()` order (descending id, quirk #63).
pub fn dump_board<W: Write>(out: &mut W, board: &Board) {
    let ctx = board.ctx();
    writeln!(
        out,
        "maxId={}",
        board.communication.id_gen.max_generated_id().0
    )
    .expect("write");
    // `board.getItems()` order, i.e. **descending** item id (quirk #63), which is what
    // `items_in_board_order` already answers.
    for id in board.items_in_board_order() {
        let Some(item) = board.get_item(id) else {
            continue;
        };
        let nets: Vec<String> = (0..item.net_count())
            .map(|i| item.get_net_number(i).to_string())
            .collect();
        let mut sb = format!(
            "item id={} type={} nets=[{}] cl={}",
            id.0,
            java_class_name(item),
            nets.join(","),
            item.clearance_class()
        );
        match item {
            Item::Trace(trace) => {
                sb.push_str(&format!(
                    " layer={} hw={} {}",
                    trace.get_layer(),
                    trace.get_half_width(),
                    poly(trace.polyline())
                ));
            }
            Item::Via(via) => {
                let padstack = via
                    .get_padstack(&ctx)
                    .expect("a via always resolves its padstack");
                sb.push_str(&format!(
                    " center={} padstack={} firstLayer={} lastLayer={}",
                    point_of(&via.get_center()),
                    padstack.name,
                    via.first_layer(&ctx),
                    via.last_layer(&ctx)
                ));
            }
            Item::Pin(pin) => {
                sb.push_str(&format!(" center={}", point_of(&pin.get_center(&ctx))));
            }
            _ => {}
        }
        writeln!(out, "{sb}").expect("write");
    }
}

/// `P6T15aProbe.ln`.
pub fn ln(l: &fr_geometry::Line) -> String {
    format!("({},{})->({},{})", l.a.x, l.a.y, l.b.x, l.b.y)
}

/// `P6T15aProbe.pt` — one corner of a polyline. Named apart from [`pt`], which is `P6T1`'s
/// point renderer and takes a `Point`.
pub fn poly_corner(p: &fr_geometry::Polyline, no: usize) -> String {
    match p.corner(no) {
        Some(fr_geometry::Point::Int(ip)) => format!("({},{})", ip.x, ip.y),
        _ => {
            let f = p.corner_approx(no).expect("a corner of a valid polyline");
            format!(
                "~({},{})",
                java_double_to_string(f.x),
                java_double_to_string(f.y)
            )
        }
    }
}

/// `P6T15aProbe.poly` — the line array and the corners.
pub fn poly(p: &fr_geometry::Polyline) -> String {
    let lines: Vec<String> = p.lines().iter().map(ln).collect();
    let corners: Vec<String> = (0..p.corner_count()).map(|i| poly_corner(p, i)).collect();
    format!(
        "n={} lines=[{}] corners=[{}]",
        lines.len(),
        lines.join(","),
        corners.join(",")
    )
}

/// `P6T15aProbe.pointOf` — `p.toFloat().round().toFloat()`, then an `(int)` truncation.
pub fn point_of(p: &fr_geometry::Point) -> String {
    let f = p.to_float().round().to_float();
    format!("({},{})", f.x as i64, f.y as i64)
}
