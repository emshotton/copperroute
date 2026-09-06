//! Rust half of the `p7t8` differential pair — `scripts/differential/java/P7T8.java`.
//!
//! Plan 7 Task 13: `BatchOptimizer`'s item half — [`ReadSortedRouteItems`]
//! (`BatchOptimizer.java:563-659`), `optRouteItem` (`:395-514`), `containsOnlyUnfixedTraces`
//! (`:85-92`) and `BatchAutorouter::autoroute_passes_for_optimizing_item`
//! (`BatchAutorouter.java:245-281`) — over a board routed by the real routing stage.
//!
//! Usage: `p7t8 <dsn> [mode] [routePasses] [items|all]`. See `P7T8.java`'s class comment for the
//! prologue, for what each of the two modes proves, and for the budget note. The shared board and
//! settings ladder is [`p7t_common`].
//!
//! # The optimizer gets its own `StoppableThread`
//!
//! `P7T8.main` hands `BatchOptimizer.createForHeadless` a `RoutingJob` with a **new**
//! `NeverStarted`, and this side builds a second [`RouterStop`] to match. That is not a
//! convenience: `AutorouteBatchLoop.java:271` calls `requestStopAutoRouter()` when the pass loop
//! hits `maxPasses`, so the prologue's flag comes back `AUTO_ROUTER_ONLY`, and
//! `autoroutePassesForOptimizingItem`'s loop head (`:268`) is
//! `!job.thread.isStopAutoRouterRequested()` — reusing the routing flag would run **zero**
//! autoroute passes per item and the driver would compare two boards nothing had touched.
//! (That is a real production consequence of sharing one job, not a driver artefact:
//! `RoutingPipeline.java:117` gates the optimizer stage on `isStopRequested()`, which is `ALL`,
//! so a `-mp`-bounded run *does* enter the optimizer with the auto-router already stopped. Task
//! 14/15 owns that seam; this driver is about `optRouteItem` doing work.)
//!
//! # The two places this side cannot be a literal transcription
//!
//! * Java's `optimizer.sortedRouteItems = reader` is an **alias**: `getCurrentPosition()` and
//!   `reader.next()` see one object. [`ReadSortedRouteItems`] is [`Copy`] here and `opt_route_item`
//!   takes `&mut BatchOptimizer`, so the driver cannot hold a `&mut` into the field across the
//!   call; [`reader_next`] copies the cursor out, advances it and writes it back, which is the
//!   same state machine with the aliasing spelled out.
//! * Java's `TreeSet<Item>` iterates **descending** (quirk #44) and `P7T8.ids` reverses it before
//!   printing; a [`BTreeSet<ItemId>`] is already ascending, so this side prints it as it stands.
//!   The two renderings are the same list, which is the `p6t1` convention.

use std::collections::BTreeSet;
use std::io::{BufWriter, Write};

use fr_board::prelude::*;
use fr_board::StopConnectionOption;
use fr_dsn::{format_double, format_float};
use fr_geometry::FloatPoint;
use fr_router::pipeline::{
    AutorouteBatchLoop, BatchOptimizer, NoopProgressSink, ReadSortedRouteItems, RouterBudget,
    RouterStop,
};
use fr_router::score::BoardStatistics;
use fr_settings::RouterSettings;

#[path = "../p7t_common.rs"]
mod p7t_common;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: p7t8 <dsn> [mode] [routePasses] [items|all]");
        std::process::exit(2);
    }
    let dsn = std::fs::canonicalize(&args[0])
        .unwrap_or_else(|e| panic!("cannot resolve {}: {e}", args[0]));
    let mode: &str = args
        .get(1)
        .filter(|a| !a.is_empty())
        .map_or("sequence", String::as_str);
    if mode != "sequence" && mode != "item" {
        eprintln!("p7t8: mode must be 'sequence' or 'item', not: {mode}");
        std::process::exit(2);
    }
    let route_passes: i32 = args
        .get(2)
        .filter(|a| !a.is_empty())
        .map_or(1, |a| a.parse().expect("routePasses"));
    let max_items: i32 = args.get(3).filter(|a| !a.is_empty()).map_or(5, |a| {
        if a == "all" {
            i32::MAX
        } else {
            a.parse().expect("items")
        }
    });

    let stdout = std::io::stdout();
    let mut out = BufWriter::new(stdout.lock());

    let (jar, bytes, mtime) = p7t_common::jar_identity();
    writeln!(
        out,
        "HEADER jar={jar} bytes={bytes} mtime={mtime} fixture={} mode={mode} \
         routePasses={route_passes} items={}",
        dsn.file_name().expect("a file name").to_string_lossy(),
        if max_items == i32::MAX {
            "all".to_string()
        } else {
            max_items.to_string()
        },
    )
    .expect("write");
    if let Ok(exe) = std::env::current_exe() {
        eprintln!("rust-binary {}", exe.display());
    }

    // ---- the prologue: a real routed board -----------------------------------------------------
    let mut board = p7t_common::load_board(&dsn);
    let settings = build_settings(&board, route_passes);
    let stop = RouterStop::new();
    let mut sink = NoopProgressSink;
    writeln!(out, "[route]").expect("write");
    let router_result = AutorouteBatchLoop::run(
        &mut board,
        &settings,
        &stop,
        RouterBudget::disabled(),
        &mut sink,
    )
    .expect("the corpus stems all have a routable signal layer");
    writeln!(
        out,
        "ROUTED returned={} {}",
        router_result.continue_routing,
        p7t_common::board_shape(&mut board)
    )
    .expect("write");

    // `BatchOptimizer.createForHeadless` (`:51-53`), on a **fresh** stop flag — see the module
    // doc's "the optimizer gets its own `StoppableThread`".
    let mut optimizer = BatchOptimizer::new(&settings);
    let optimizer_stop = RouterStop::new();

    if mode == "sequence" {
        dump_sequence(&mut out, &mut optimizer, &board);
    } else {
        dump_items(
            &mut out,
            &mut optimizer,
            &mut board,
            max_items,
            &optimizer_stop,
            &mut sink,
        );
    }

    writeln!(out, "[board]").expect("write");
    p7t_common::dump_board(&mut out, &board);
    out.flush().expect("flush");
}

/// `P7T8.buildSettings` — `p7t_common::build_settings` plus the routing prologue's knobs, i.e.
/// `p7t9`'s `router-only` mode, so the board this driver optimizes is the board `p7t9` pins.
fn build_settings(board: &Board, route_passes: i32) -> RouterSettings {
    let mut settings = p7t_common::build_settings(board);
    settings.max_passes = Some(route_passes);
    let fanout = settings.fanout.get_or_insert_with(Default::default);
    fanout.enabled = Some(false);
    fanout.max_milliseconds_per_pin = Some(i64::from(i32::MAX));
    settings.set_run_router(true);
    settings.set_run_optimizer(false);
    settings.save_intermediate_stages = Some(false);
    settings
}

/// `optimizer.sortedRouteItems`' aliasing, spelled out — see the module doc.
fn reader_next(optimizer: &mut BatchOptimizer<'_>, board: &Board) -> Option<ItemId> {
    let mut reader = optimizer
        .sorted_route_items
        .expect("the driver assigns the reader before walking it");
    let result = reader.next(board);
    optimizer.sorted_route_items = Some(reader);
    result
}

// ------------------------------------------------------------------------------------------------
// Mode `sequence` — ReadSortedRouteItems.next(), BatchOptimizer.java:573-654
// ------------------------------------------------------------------------------------------------

fn dump_sequence<W: Write>(out: &mut W, optimizer: &mut BatchOptimizer<'_>, board: &Board) {
    writeln!(out, "[sequence]").expect("write");
    // `:520-525` — `getCurrentPosition()` before `sortedRouteItems` exists.
    writeln!(
        out,
        "OPTPOS-BEFORE {}",
        pos(optimizer.get_current_position())
    )
    .expect("write");
    optimizer.sorted_route_items = Some(ReadSortedRouteItems::new());
    writeln!(
        out,
        "OPTPOS-FRESH {}",
        pos(optimizer.get_current_position())
    )
    .expect("write");

    let mut n = 0i32;
    // The same safety bound the Java side uses: the reader is strictly monotone, so it cannot
    // return more items than the board holds.
    let bound = i32::try_from(board.items_in_board_order().len()).unwrap_or(i32::MAX) + 1;
    while n < bound {
        let Some(current) = reader_next(optimizer, board) else {
            break;
        };
        let item = board
            .get_item(current)
            .expect("the reader returns live items");
        writeln!(
            out,
            "SEQ n={n} id={} kind={} key={} layer={} cursor={} cursorLayer={}",
            current.0,
            p7t_common::java_class_name(item),
            pos(Some(key_of(board, current))),
            layer_of(board, current),
            pos(optimizer.get_current_position()),
            optimizer
                .sorted_route_items
                .expect("assigned above")
                .min_item_layer,
        )
        .expect("write");
        n += 1;
    }
    writeln!(
        out,
        "SEQ-END count={n} cursor={} cursorLayer={}",
        pos(optimizer.get_current_position()),
        optimizer
            .sorted_route_items
            .expect("assigned above")
            .min_item_layer,
    )
    .expect("write");
}

/// `P7T8.keyOf` — the `(x, y)` the comparison chain keys on: `:585` for a via, `:617-622` for a
/// trace.
fn key_of(board: &Board, id: ItemId) -> FloatPoint {
    match board.get_item(id).expect("a live item") {
        Item::Via(via) => via.get_center().to_float(),
        Item::Trace(trace) => {
            let first_corner = trace.first_corner().expect("a board trace").to_float();
            let last_corner = trace.last_corner().expect("a board trace").to_float();
            if first_corner.x < last_corner.x
                || first_corner.x == last_corner.x && first_corner.y < last_corner.y
            {
                last_corner
            } else {
                first_corner
            }
        }
        other => panic!("ReadSortedRouteItems returned a {other:?}"),
    }
}

/// `P7T8.layerOf` — `:586` for a via, `:623` for a trace.
fn layer_of(board: &Board, id: ItemId) -> usize {
    let ctx = board.ctx();
    match board.get_item(id).expect("a live item") {
        Item::Via(via) => via.first_layer(&ctx),
        Item::Trace(trace) => trace.get_layer(),
        other => panic!("ReadSortedRouteItems returned a {other:?}"),
    }
}

/// `P7T8.pos` — `Double.toString` on both coordinates, or Java's `null`.
fn pos(p: Option<FloatPoint>) -> String {
    match p {
        None => "null".to_string(),
        Some(p) => format!(
            "({},{})",
            format_double(p.x),
            format_double(p.y)
        ),
    }
}

// ------------------------------------------------------------------------------------------------
// Mode `item` — optRouteItem, BatchOptimizer.java:395-514
// ------------------------------------------------------------------------------------------------

fn dump_items<W: Write>(
    out: &mut W,
    optimizer: &mut BatchOptimizer<'_>,
    board: &mut Board,
    max_items: i32,
    stop: &RouterStop,
    sink: &mut NoopProgressSink,
) {
    writeln!(out, "[items]").expect("write");
    // `runBatchLoop:132`.
    optimizer.use_increased_ripup_costs = true;
    // `optRoutePass:283, :288`.
    let statistics_before = BoardStatistics::new(board);
    optimizer.min_cumulative_trace_length = f64::from(
        statistics_before
            .traces
            .total_weighted_length
            .unwrap_or(0.0),
    );
    writeln!(
        out,
        "SEED useIncreasedRipupCosts={} minCumulativeTraceLength={}",
        optimizer.use_increased_ripup_costs,
        format_double(optimizer.min_cumulative_trace_length)
    )
    .expect("write");

    optimizer.sorted_route_items = Some(ReadSortedRouteItems::new());
    // `runBatchLoop:200` — pass 1, so `1 % 2 != 0`.
    let with_preferred_directions = true;

    let mut n = 0i32;
    while n < max_items {
        let Some(current_item) = reader_next(optimizer, board) else {
            writeln!(out, "NEXT-NULL n={n}").expect("write");
            break;
        };

        // The transcription of `:412-432`, printed beside the real call.
        let mut ripped_items: BTreeSet<ItemId> = BTreeSet::new();
        ripped_items.insert(current_item);
        let item_is_trace = matches!(board.get_item(current_item), Some(Item::Trace(_)));
        if item_is_trace {
            let mut current_contact_list = board.trace_start_contacts(current_item);
            for _ in 0..2 {
                if BatchOptimizer::contains_only_unfixed_traces(board, &current_contact_list) {
                    ripped_items.extend(current_contact_list.iter().copied());
                }
                current_contact_list = board.trace_end_contacts(current_item);
            }
        }
        let mut ripped_connections: BTreeSet<ItemId> = BTreeSet::new();
        for item in ripped_items.iter().rev() {
            ripped_connections.extend(board.connection_items(*item, StopConnectionOption::None));
        }
        let any_user_fixed = ripped_connections
            .iter()
            .rev()
            .any(|id| board.get_item(*id).is_some_and(Item::is_user_fixed));

        // The transcription of `:453-463`.
        let optimizer_settings = optimizer
            .settings
            .optimizer
            .as_ref()
            .expect("DefaultSettings always builds an optimizer block");
        let mut ripup_costs = optimizer.settings.get_start_ripup_costs();
        if optimizer.use_increased_ripup_costs {
            ripup_costs = ripup_costs.wrapping_mul(
                optimizer_settings
                    .additional_ripup_cost_factor_at_start
                    .expect("a default value"),
            );
        }
        if item_is_trace {
            ripup_costs = (f64::from(
                optimizer_settings
                    .trace_ripup_cost_factor
                    .expect("a default value"),
            ) * f64::from(ripup_costs))
            .round() as i32;
        }
        let max_autoroute_passes = optimizer_settings.max_autoroute_passes;

        let item = board.get_item(current_item).expect("a live item");
        let nets: Vec<String> = (0..item.net_count())
            .map(|i| item.get_net_number(i).to_string())
            .collect();
        writeln!(
            out,
            "ITEM n={n} id={} kind={} nets=[{}] rippedItems={} rippedConnections={} \
             anyUserFixed={any_user_fixed} ripupCosts={ripup_costs} \
             maxAutoroutePasses={} tracePullTightAccuracy={}",
            current_item.0,
            p7t_common::java_class_name(item),
            nets.join(","),
            ids(&ripped_items),
            ids(&ripped_connections),
            integer(max_autoroute_passes),
            integer(optimizer.settings.trace_pull_tight_accuracy),
        )
        .expect("write");

        let max_id_before = board.communication.id_gen.max_generated_id();
        // Plan 7 Task 14b: the `ID` ledger (`Board::new_item_id`, `P7T8B_IDS`) says nothing about
        // which item it belongs to, so both sides print the same separator to stderr under the
        // same variable. The Java half is `P7T8.dumpItems`' `ITEMSEP`.
        if std::env::var_os("P7T8B_IDS").is_some() {
            eprintln!("ITEMSEP n={n} id={}", current_item.0);
        }
        let result = optimizer
            .opt_route_item(
                board,
                current_item,
                with_preferred_directions,
                false,
                stop,
                RouterBudget::disabled(),
                sink,
            )
            .expect("optRouteItem answers Ok on every corpus path");

        writeln!(
            out,
            "RESULT n={n} itemId={} improved={} viaCount={} traceLength={} incompleteBefore={} \
             incompleteAfter={} viaCountReduced={} lengthReduced={} improvementPercentage={} \
             minCumulativeTraceLength={}",
            result.item_id().0,
            result.improved(),
            result.via_count(),
            format_double(result.trace_length()),
            result.incomplete_count_before(),
            result.incomplete_count(),
            result.via_count_reduced(),
            format_double(result.length_reduced()),
            format_float(result.improvement_percentage()),
            format_double(optimizer.min_cumulative_trace_length),
        )
        .expect("write");
        writeln!(
            out,
            "BOARD n={n} maxIdBefore={} maxIdAfter={} {}",
            max_id_before.0,
            board.communication.id_gen.max_generated_id().0,
            p7t_common::board_shape(board)
        )
        .expect("write");
        out.flush().expect("flush");
        n += 1;
    }
    writeln!(
        out,
        "ITEMS-END count={n} cursor={}",
        pos(optimizer.get_current_position())
    )
    .expect("write");
}

/// `P7T8.ids` — see the module doc for why this side does not reverse.
fn ids(items: &BTreeSet<ItemId>) -> String {
    let rendered: Vec<String> = items.iter().map(|id| id.0.to_string()).collect();
    format!("[{}]", rendered.join(","))
}

/// Java's `"" + Integer` — the boxed value or the literal `null`.
fn integer(value: Option<i32>) -> String {
    value.map_or_else(|| "null".to_string(), |v| v.to_string())
}
