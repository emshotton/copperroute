//! Rust half of the `p7t5` differential pair — `scripts/differential/java/P7T5.java`.
//!
//! Plan 7 Task 11: `BatchFanout`'s component/pin ordering (`BatchFanout.java:35-78`, `:631-693`,
//! `:695-778`) and `RoutingBoard.fanout` (`RoutingBoard.java:978-1110`).
//!
//! Plan 7 Task 12 adds modes `pass` (one whole `BatchFanout.fanoutPass`) and `board` (the whole
//! `BatchFanout.fanoutBoard`), each in a transcribed half and a **real** half.
//!
//! Usage: `p7t5 <dsn> [passNo|maxPasses] [sortingOrder] [order|pin|pass|board]`. See
//! `P7T5.java`'s class comment for what each mode prints, for why the removed-id set stands in
//! for `rippedItemList`, for `P7T5_HASH_MODE` and for the budget note. The shared board/settings
//! ladder is [`p7t_common`], the same one `p7t1`, `p7t2` and `p7t9` use.
//!
//! # Where the two sides differ in *shape*, and why the bytes still match
//!
//! Java reaches `sortedComponents`, `Component` and `Component.Pin` through
//! `Field.setAccessible(true)`, because all three are private. The port's are `pub`: scan ruling 7
//! makes Task 11 the declaring task, and Task 12 needs them from `pipeline/fanout.rs`'s own
//! `impl` blocks. Everything printed is read out of the objects the real constructor built on
//! both sides.

use std::io::{BufWriter, Write};

use fr_board::items::Item;
use fr_board::prelude::*;
use fr_dsn::format_double;
use fr_router::board_ext::RoutingBoardExt;
use fr_router::pipeline::{
    fanout_pin_can_use_vias, fanout_ripup_costs, BatchFanout, EscapeStatistics, FanoutLoopState,
    FanoutStop, NoopProgressSink, RouterBudget, RouterStop,
};
use fr_router::score::BoardStatistics;
use fr_router::AutorouteAttemptState;
use fr_settings::RouterSettings;

#[path = "../p7t_common.rs"]
mod p7t_common;

/// `P7T5.SORTING_ORDERS` — the four strings `Pin.compareTo:744-771` tests, plus one it does not.
const SORTING_ORDERS: [&str; 5] = [
    "inner_first",
    "outer_first",
    "distanceToClosestOnNet",
    "surroundingsDensity",
    "not_a_sorting_order",
];

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: p7t5 <dsn> [passNo|maxPasses] [sortingOrder] [order|pin|pass|board]");
        std::process::exit(2);
    }
    let dsn = std::fs::canonicalize(&args[0])
        .unwrap_or_else(|e| panic!("cannot resolve {}: {e}", args[0]));
    let pass_no: i32 = args
        .get(1)
        .filter(|a| !a.is_empty())
        .map_or(0, |a| a.parse().expect("passNo"));
    let sorting_order: &str = args
        .get(2)
        .filter(|a| !a.is_empty())
        .map_or("outer_first", String::as_str);
    let mode: &str = args
        .get(3)
        .filter(|a| !a.is_empty())
        .map_or("order", String::as_str);
    if !matches!(mode, "order" | "pin" | "pass" | "board") {
        eprintln!("p7t5: mode must be one of `order`, `pin`, `pass`, `board`, not '{mode}'");
        std::process::exit(2);
    }
    // `P7T5_HASH_MODE` moves the **Java** side only (see `P7T5.java`'s class comment): the port
    // has one hash and it is warm by construction. Printed here so a mismatched pair is a diff.
    let hash_mode = std::env::var("P7T5_HASH_MODE").unwrap_or_default();
    let hash_mode = if hash_mode.is_empty() {
        "warm".to_string()
    } else {
        hash_mode
    };
    if hash_mode != "warm" && hash_mode != "raw" {
        eprintln!("p7t5: P7T5_HASH_MODE must be `warm` or `raw`, not '{hash_mode}'");
        std::process::exit(2);
    }

    let stdout = std::io::stdout();
    let mut out = BufWriter::new(stdout.lock());

    let (jar, bytes, mtime) = p7t_common::jar_identity();
    writeln!(
        out,
        "HEADER jar={jar} bytes={bytes} mtime={mtime} fixture={} passNo={pass_no} \
         sortingOrder={sorting_order} mode={mode} hashMode={hash_mode}",
        dsn.file_name().expect("a file name").to_string_lossy(),
    )
    .expect("write");
    if let Ok(exe) = std::env::current_exe() {
        eprintln!("rust-binary {}", exe.display());
    }

    match mode {
        "order" => {
            for order in SORTING_ORDERS {
                let board = p7t_common::load_board(&dsn);
                let mut settings = p7t_common::build_settings(&board);
                set_sorting_order(&mut settings, order);
                dump_order(&mut out, &board, &settings, order);
            }
        }
        "pin" => {
            let mut board = p7t_common::load_board(&dsn);
            let mut settings = p7t_common::build_settings(&board);
            set_sorting_order(&mut settings, sorting_order);
            dump_fanout_run(&mut out, &mut board, &settings, pass_no);
        }
        "pass" => dump_pass(&mut out, &dsn, sorting_order, pass_no),
        _ => dump_board_run(&mut out, &dsn, sorting_order, pass_no),
    }
    out.flush().expect("flush");
}

/// `settings.fanout.pinSortingOrder = order` (`P7T5.java:129`).
fn set_sorting_order(settings: &mut RouterSettings, order: &str) {
    settings
        .fanout
        .as_mut()
        .expect("DefaultSettings builds a fanout block")
        .pin_sorting_order = Some(order.to_string());
}

// =================================================================================================
// Mode `order`
// =================================================================================================

/// `P7T5.dumpOrder`.
fn dump_order<W: Write>(out: &mut W, board: &Board, settings: &RouterSettings, order: &str) {
    let ctx = board.ctx();
    let fanout = BatchFanout::new(board, settings);
    writeln!(
        out,
        "[order] sortingOrder={} components={} totalSmdPinCount={} alreadyConnectedPinCount={}",
        p7t_common::quote(order),
        fanout.sorted_components.len(),
        fanout.total_smd_pin_count,
        fanout.already_connected_pin_count,
    )
    .expect("write");
    for (component_index, component) in fanout.sorted_components.iter().enumerate() {
        writeln!(
            out,
            "COMPONENT {component_index} id={} name={} smdPinCount={} sortedPins={} \
             gravity=({},{})",
            component.component,
            p7t_common::quote(&component.component_name),
            component.smd_pin_count,
            component.smd_pins.len(),
            format_double(component.gravity_center_of_smd_pins.x),
            format_double(component.gravity_center_of_smd_pins.y),
        )
        .expect("write");
        for (pin_index, pin) in component.smd_pins.iter().enumerate() {
            let name = match board.get_item(pin.inner.pin) {
                Some(Item::Pin(p)) => p.name(&ctx).map(str::to_owned),
                _ => None,
            };
            writeln!(
                out,
                "  PIN {pin_index} id={} pinIndex={} name={} distToCentre={} \
                 distToClosestOnNet={} surroundingsDensity={}",
                pin.inner.pin.0,
                pin.inner.pin_index,
                name.as_deref()
                    .map_or("null".to_string(), p7t_common::quote),
                format_double(pin.inner.distance_to_component_center),
                // `None` is the port's model of Java's `Double.MAX_VALUE` sentinel (quirk #219,
                // `FanoutPin::distance_to_closest_on_net`'s doc) — `unwrap_or(f64::MAX)` is not a
                // fallback, it is that sentinel.
                format_double(pin.inner.distance_to_closest_on_net.unwrap_or(f64::MAX)),
                pin.inner.surroundings_density,
            )
            .expect("write");
        }
    }
}

// =================================================================================================
// Mode `pin`
// =================================================================================================

/// `P7T5.dumpFanoutRun`.
fn dump_fanout_run<W: Write>(
    out: &mut W,
    board: &mut Board,
    settings: &RouterSettings,
    pass_no: i32,
) {
    // The walk order is fixed before the first `fanout` call, exactly as Java's is: the
    // constructor runs once (`BatchFanout.java:92`) and `fanoutPass` iterates the set it built.
    let order: Vec<(String, Vec<ItemId>)> = {
        let fanout = BatchFanout::new(board, settings);
        fanout
            .sorted_components
            .iter()
            .map(|component| {
                (
                    component.component_name.clone(),
                    component.smd_pins.iter().map(|pin| pin.inner.pin).collect(),
                )
            })
            .collect()
    };

    // `:173` and `:179-183`.
    let ripup_costs = settings.get_start_ripup_costs() * (pass_no + 1);
    let ripup_allowed = settings
        .fanout
        .as_ref()
        .and_then(|f| f.ripup_allowed)
        .unwrap_or(true);
    let effective_ripup_costs = if ripup_allowed { ripup_costs } else { -1 };
    let fallback = settings
        .fanout
        .as_ref()
        .and_then(|f| f.fallback_to_board_vias);
    writeln!(
        out,
        "[pin] ripupCosts={effective_ripup_costs} fallbackToBoardVias={} {}",
        fallback.map_or("null".to_string(), |b| b.to_string()),
        p7t_common::board_shape(board),
    )
    .expect("write");

    // Ruling AI: the wall clock is off on both sides. Java passes `new TimeLimit(MAX_VALUE)`;
    // `RouterBudget::disabled()` additionally takes the `optChangedArea` limit out of the port's
    // half, where Java's is a `javac`-inlined local it cannot reach.
    let time_limit = TimeLimit::new(i32::MAX);
    let budget = RouterBudget::disabled();
    let mut engine: Option<fr_router::AutorouteEngine> = None;

    let mut index = 0_i32;
    for (component_name, pins) in &order {
        for pin in pins {
            let (net_number, full_pin_name) = {
                let ctx = board.ctx();
                let item = board.get_item(*pin).expect("an SMD pin");
                let name = match item {
                    Item::Pin(p) => p.name(&ctx).map(str::to_owned),
                    _ => None,
                };
                (
                    item.get_net_number(0),
                    format!("{component_name}-{}", name.unwrap_or_else(|| "null".into())),
                )
            };

            // `:238-259` — the "no vias and no fallback" skip.
            if let Some(net) = board.rules.nets.get(net_number) {
                let net_class = net.get_net_class();
                let via_count = board
                    .rules
                    .net_classes
                    .get(net_class)
                    .get_via_rule()
                    .map_or(0, fr_board::ViaRule::via_count);
                let has_board_vias =
                    !board.rules.via_rules.is_empty() && board.rules.via_rules[0].via_count() > 0;
                let fallback_allowed = fallback == Some(true) && has_board_vias;
                if !(via_count > 0 || fallback_allowed) {
                    writeln!(
                        out,
                        "SKIP {index} pin={} net={net_number}",
                        p7t_common::quote(&full_pin_name)
                    )
                    .expect("write");
                    index += 1;
                    continue;
                }
            }

            let before: Vec<ItemId> = board.get_items().map(Item::id).collect();
            let max_id_before = board.communication.id_gen.max_generated_id();
            board.start_marking_changed_area(); // `:279`
            let result = board.fanout(
                &mut engine,
                *pin,
                settings,
                effective_ripup_costs,
                &|| false,
                Some(time_limit),
                budget,
            )
            .expect("fanout completes");
            let after: Vec<ItemId> = board.get_items().map(Item::id).collect();

            let mut sb = String::new();
            sb.push_str(&format!("{{\"k\":{index}"));
            sb.push_str(&format!(",\"pin\":{}", pin.0));
            sb.push_str(&format!(",\"name\":{}", p7t_common::quote(&full_pin_name)));
            sb.push_str(&format!(",\"net\":{net_number}"));
            sb.push_str(&format!(",\"state\":\"{}\"", result.state.name()));
            sb.push_str(&format!(
                ",\"details\":{}",
                p7t_common::quote(result.details.as_deref().unwrap_or(""))
            ));
            sb.push_str(&format!(",\"removed\":{}", removed(&before, &after)));
            sb.push_str(&format!(",\"maxIdBefore\":{}", max_id_before.0));
            sb.push_str(&format!(
                ",\"maxIdAfter\":{}",
                board.communication.id_gen.max_generated_id().0
            ));
            p7t_common::append_inserted_geometry(&mut sb, board, max_id_before);
            sb.push('}');
            writeln!(out, "{sb}").expect("write");
            index += 1;
        }
    }
    writeln!(out, "[final] {}", p7t_common::board_shape(board)).expect("write");
}

/// `P7T5.removed` — the ids in `before` that `after` no longer has, ascending.
fn removed(before: &[ItemId], after: &[ItemId]) -> String {
    let live: std::collections::BTreeSet<ItemId> = after.iter().copied().collect();
    let mut gone: Vec<u32> = before
        .iter()
        .filter(|id| !live.contains(id))
        .map(|id| id.0)
        .collect();
    gone.sort_unstable();
    let rendered: Vec<String> = gone.iter().map(u32::to_string).collect();
    format!("[{}]", rendered.join(","))
}

// =================================================================================================
// Modes `pass` and `board` — Plan 7 Task 12
// =================================================================================================

/// `P7T5.buildSettings(board, sortingOrder, maxPasses)` — the shared ladder plus the two knobs
/// modes `pass`/`board` need. Ruling AI's per-pin clock goes off through
/// `settings.fanout.maxMillisecondsPerPin`, which is where `fanoutPass:231-232` reads its base,
/// so both sides disable the same knob.
fn build_pass_settings(board: &Board, sorting_order: &str, max_passes: i32) -> RouterSettings {
    let mut settings = p7t_common::build_settings(board);
    set_sorting_order(&mut settings, sorting_order);
    let fanout = settings
        .fanout
        .as_mut()
        .expect("DefaultSettings builds a fanout block");
    fanout.max_milliseconds_per_pin = Some(i64::from(i32::MAX));
    if max_passes > 0 {
        fanout.max_passes = Some(max_passes);
    }
    settings
}

/// `P7T5.escapeLine`.
fn escape_line(board: &mut Board) -> String {
    let stats = BoardStatistics::with_options(board, None, false);
    let escape = EscapeStatistics::from_board_statistics(&stats);
    format!(
        "totalSmdPins={} escapedCount={} escapedPercentage={} pinsToEscape={}",
        escape.total_smd_pins,
        escape.escaped_count,
        format_double(escape.escaped_percentage),
        stats.fanout.pins_to_escape,
    )
}

/// `P7T5.dumpPass`.
fn dump_pass<W: Write>(out: &mut W, dsn: &std::path::Path, sorting_order: &str, pass_no: i32) {
    let mut board = p7t_common::load_board(dsn);
    let settings = build_pass_settings(&board, sorting_order, 0);
    let mut instance = BatchFanout::new(&board, &settings);
    writeln!(
        out,
        "[pass] passNo={pass_no} ripupCosts={} maxMillisecondsPerPin={} totalSmdPinCount={} \
         alreadyConnectedPinCount={} {}",
        fanout_ripup_costs(&settings, pass_no),
        settings
            .fanout
            .as_ref()
            .and_then(|f| f.max_milliseconds_per_pin)
            .map_or("null".to_string(), |v| v.to_string()),
        instance.total_smd_pin_count,
        instance.already_connected_pin_count,
        p7t_common::board_shape(&mut board),
    )
    .expect("write");

    writeln!(out, "[transcript]").expect("write");
    let routed_count = transcribe_pass(out, &mut instance, &mut board, &settings, pass_no);
    writeln!(
        out,
        "TRANSCRIPT routed={routed_count} {}",
        p7t_common::board_shape(&mut board)
    )
    .expect("write");
    writeln!(out, "ESCAPE {}", escape_line(&mut board)).expect("write");
    let transcript_hash = board.structural_hash();

    let mut real_board = p7t_common::load_board(dsn);
    let real_settings = build_pass_settings(&real_board, sorting_order, 0);
    let mut real_instance = BatchFanout::new(&real_board, &real_settings);
    writeln!(out, "[real]").expect("write");
    let stop = RouterStop::new();
    let mut sink = NoopProgressSink;
    let real_routed = real_instance
        .fanout_pass(
            &mut real_board,
            pass_no,
            &stop,
            RouterBudget::disabled(),
            &mut sink,
        )
        .expect("fanout_pass answers Ok on every path");
    writeln!(
        out,
        "REAL routed={real_routed} totalItemsFanouted={} extraViasTotal={} \
         lastNotRoutedCount={} isTimedOut={} {}",
        real_instance.total_items_fanouted,
        real_instance.extra_vias_total,
        real_instance.last_not_routed_count,
        real_instance.is_timed_out,
        p7t_common::board_shape(&mut real_board),
    )
    .expect("write");
    writeln!(out, "ESCAPE {}", escape_line(&mut real_board)).expect("write");
    writeln!(
        out,
        "EQUALS-TRANSCRIPT {}",
        real_board.structural_hash() == transcript_hash
    )
    .expect("write");
}

/// `P7T5.transcribePass` — `fanoutPass`' body (`BatchFanout.java:166-506`), with the real
/// `RoutingBoardExt::fanout`.
fn transcribe_pass<W: Write>(
    out: &mut W,
    instance: &mut BatchFanout<'_>,
    board: &mut Board,
    settings: &RouterSettings,
    pass_no: i32,
) -> i32 {
    // `:168-172`.
    let mut pins_to_go = instance.total_smd_pin_count;
    let mut routed_count = 0_i32;
    let mut not_routed_count = 0_i32;
    let mut insert_error_count = 0_i32;
    let mut already_connected_count = 0_i32;
    let vias_before_pass = board.get_vias().len();
    // `:173`, `:175-183`.
    let ripup_costs = settings.get_start_ripup_costs() * (pass_no + 1);
    let base_millis_per_pin = settings
        .fanout
        .as_ref()
        .and_then(|f| f.max_milliseconds_per_pin)
        .unwrap_or(10_000);
    let effective_ripup_costs = fanout_ripup_costs(settings, pass_no);
    let max_items = settings.fanout.as_ref().and_then(|f| f.max_items);

    // `:220-221`, snapshotted for the same reason `BatchFanout::fanout_pass` snapshots it.
    let walk: Vec<(String, Vec<ItemId>)> = instance
        .sorted_components
        .iter()
        .map(|component| {
            (
                component.component_name.clone(),
                component.smd_pins.iter().map(|pin| pin.inner.pin).collect(),
            )
        })
        .collect();

    let mut max_limit_reached = false;
    let mut index = 0_i32;
    for (component_name, pins) in &walk {
        for current_pin in pins {
            // `:222-230`.
            if max_items.is_some_and(|max| max > 0 && instance.total_items_fanouted >= max) {
                writeln!(
                    out,
                    "MAXITEMS k={index} totalItemsFanouted={}",
                    instance.total_items_fanouted
                )
                .expect("write");
                max_limit_reached = true;
                break;
            }
            // `:231-232` — `(int)` of a `double`, i.e. a saturating narrowing.
            let time_limit =
                TimeLimit::new((base_millis_per_pin as f64 * f64::from(pass_no + 1)) as i32);
            // `:233-236`.
            let (net_number, full_pin_name, target_count) = {
                let ctx = board.ctx();
                let item = board.get_item(*current_pin).expect("an SMD pin");
                let name = match item {
                    Item::Pin(p) => p.name(&ctx).map(str::to_owned),
                    _ => None,
                };
                let net_number = item.get_net_number(0);
                (
                    net_number,
                    format!("{component_name}-{}", name.unwrap_or_else(|| "null".into())),
                    board.unconnected_set(*current_pin, net_number).len(),
                )
            };

            // `:238-259`.
            if !fanout_pin_can_use_vias(board, settings, net_number) {
                pins_to_go -= 1;
                writeln!(
                    out,
                    "SKIP {index} pin={} net={net_number} pinsToGo={pins_to_go}",
                    p7t_common::quote(&full_pin_name)
                )
                .expect("write");
                index += 1;
                continue;
            }

            let before: Vec<ItemId> = board.get_items().map(Item::id).collect();
            let max_id_before = board.communication.id_gen.max_generated_id();
            board.start_marking_changed_area(); // `:279`
            let mut engine = None;
            let result = board.fanout(
                &mut engine,
                *current_pin,
                settings,
                effective_ripup_costs,
                &|| false,
                Some(time_limit),
                RouterBudget::disabled(),
            )
            .expect("fanout completes");
            let after: Vec<ItemId> = board.get_items().map(Item::id).collect();

            // `:286-381`.
            match result.state {
                AutorouteAttemptState::Routed => {
                    routed_count += 1;
                    instance.total_items_fanouted += 1;
                }
                AutorouteAttemptState::AlreadyConnected => already_connected_count += 1,
                AutorouteAttemptState::Failed => {
                    not_routed_count += 1;
                    instance.total_items_fanouted += 1;
                }
                AutorouteAttemptState::InsertError => {
                    insert_error_count += 1;
                    instance.total_items_fanouted += 1;
                }
                _ => {}
            }
            pins_to_go -= 1; // `:382`
            let extra_vias_this_pass =
                (board.get_vias().len().saturating_sub(vias_before_pass)) as i32; // `:383`

            let mut sb = String::new();
            sb.push_str(&format!("{{\"k\":{index}"));
            sb.push_str(&format!(",\"pin\":{}", current_pin.0));
            sb.push_str(&format!(",\"name\":{}", p7t_common::quote(&full_pin_name)));
            sb.push_str(&format!(",\"net\":{net_number}"));
            sb.push_str(&format!(",\"targets\":{target_count}"));
            sb.push_str(&format!(",\"state\":\"{}\"", result.state.name()));
            sb.push_str(&format!(
                ",\"details\":{}",
                p7t_common::quote(result.details.as_deref().unwrap_or(""))
            ));
            sb.push_str(&format!(",\"removed\":{}", removed(&before, &after)));
            sb.push_str(&format!(",\"maxIdBefore\":{}", max_id_before.0));
            sb.push_str(&format!(
                ",\"maxIdAfter\":{}",
                board.communication.id_gen.max_generated_id().0
            ));
            sb.push_str(&format!(",\"routed\":{routed_count}"));
            sb.push_str(&format!(",\"notRouted\":{not_routed_count}"));
            sb.push_str(&format!(",\"insertErrors\":{insert_error_count}"));
            sb.push_str(&format!(",\"alreadyConnected\":{already_connected_count}"));
            sb.push_str(&format!(",\"pinsToGo\":{pins_to_go}"));
            sb.push_str(&format!(
                ",\"totalItemsFanouted\":{}",
                instance.total_items_fanouted
            ));
            sb.push_str(&format!(",\"extraVias\":{extra_vias_this_pass}"));
            p7t_common::append_inserted_geometry(&mut sb, board, max_id_before);
            sb.push('}');
            writeln!(out, "{sb}").expect("write");
            index += 1;
        }
        if max_limit_reached {
            break;
        }
    }
    // `:439`.
    let extra_vias_this_pass = (board.get_vias().len().saturating_sub(vias_before_pass)) as i32;
    writeln!(
        out,
        "PASS-END routed={routed_count} notRouted={not_routed_count} \
         insertErrors={insert_error_count} alreadyConnected={already_connected_count} \
         pinsToGo={pins_to_go} totalItemsFanouted={} extraViasThisPass={extra_vias_this_pass} \
         ripupCosts={ripup_costs}",
        instance.total_items_fanouted
    )
    .expect("write");
    routed_count
}

/// `P7T5.dumpBoardRun`.
fn dump_board_run<W: Write>(
    out: &mut W,
    dsn: &std::path::Path,
    sorting_order: &str,
    max_passes_arg: i32,
) {
    let mut board = p7t_common::load_board(dsn);
    let settings = build_pass_settings(&board, sorting_order, max_passes_arg);
    writeln!(
        out,
        "[board-run] maxPasses={} maxItems={} timeout={} {}",
        settings
            .fanout
            .as_ref()
            .and_then(|f| f.max_passes)
            .map_or("null".to_string(), |v| v.to_string()),
        settings
            .fanout
            .as_ref()
            .and_then(|f| f.max_items)
            .map_or("null".to_string(), |v| v.to_string()),
        settings
            .fanout
            .as_ref()
            .and_then(|f| f.timeout_string.as_deref())
            .unwrap_or("null"),
        p7t_common::board_shape(&mut board),
    )
    .expect("write");

    writeln!(out, "[transcript]").expect("write");
    let mut instance = BatchFanout::new(&board, &settings);
    let stop = RouterStop::new();
    let mut sink = NoopProgressSink;
    // `:101-109`.
    let max_passes = settings
        .fanout
        .as_ref()
        .and_then(|f| f.max_passes)
        .unwrap_or(20);
    let max_items = settings.fanout.as_ref().and_then(|f| f.max_items);
    let mut completed_passes = 0_i32;
    let mut loop_state = FanoutLoopState::new(board.structural_hash());
    // `FanoutLoopState::board_state` and its `identical_passes` stagnation counter were retired
    // from the crate (59bbdd9 #222, then 1929fc8 folded stagnation into an immediate
    // `UnchangedHash` stop); the packed value and the counter live only here now, kept verbatim
    // so this transcript still lines up with `P7T5.java`'s `:133-147`, which still computes both.
    let mut previous_board_state = i64::MIN;
    let mut identical_passes = 0_i32;
    let mut stop_reason = "MAXPASSES".to_string();
    // `:110-157`.
    for i in 0..max_passes {
        // `:111-116` — unreachable here, `timeout` is null on both sides.
        if instance.is_deadline_reached() {
            instance.is_timed_out = true;
            stop_reason = "DEADLINE".to_string();
            break;
        }
        // `:117-122`.
        if max_items.is_some_and(|max| max > 0 && instance.total_items_fanouted >= max) {
            stop_reason = "MAXITEMS".to_string();
            break;
        }
        // `:123-124`.
        let routed_count = instance
            .fanout_pass(&mut board, i, &stop, RouterBudget::disabled(), &mut sink)
            .expect("fanout_pass answers Ok on every path");
        completed_passes += 1;
        let via_count = board.get_vias().len();
        let board_state = (i64::from(routed_count) << 32) ^ i64::from(via_count as i32);
        let timed_out = instance.is_timed_out;
        writeln!(
            out,
            "PASS {i} routed={routed_count} vias={via_count} boardState={board_state} \
             isTimedOut={timed_out} totalItemsFanouted={} extraViasTotal={} lastNotRouted={}",
            instance.total_items_fanouted,
            instance.extra_vias_total,
            instance.last_not_routed_count
        )
        .expect("write");
        // `:125-156`, through the port's own decision block, so the driver checks the shipped
        // code rather than a second copy of it.
        if routed_count != 0 {
            if board_state == previous_board_state {
                identical_passes += 1;
                writeln!(out, "STAGNATION pass={i} identicalPasses={identical_passes}")
                    .expect("write");
            } else {
                identical_passes = 0;
                previous_board_state = board_state;
            }
        }
        let mut hash_taken = false;
        let decision = loop_state.after_pass(routed_count, timed_out, || {
            hash_taken = true;
            board.structural_hash()
        });
        if hash_taken {
            writeln!(
                out,
                "HASHEQ pass={i} equal={}",
                decision == Some(FanoutStop::UnchangedHash)
            )
            .expect("write");
        }
        if let Some(reason) = decision {
            stop_reason = match reason {
                FanoutStop::NothingRouted => "NOTHING-ROUTED",
                FanoutStop::TimedOut => "TIMED-OUT",
                FanoutStop::UnchangedHash => "UNCHANGED-HASH",
            }
            .to_string();
            break;
        }
    }
    writeln!(
        out,
        "STOP {stop_reason} completedPasses={completed_passes} isTimedOut={} \
         totalItemsFanouted={}",
        instance.is_timed_out, instance.total_items_fanouted
    )
    .expect("write");
    writeln!(out, "TRANSCRIPT {}", p7t_common::board_shape(&mut board)).expect("write");
    writeln!(out, "ESCAPE {}", escape_line(&mut board)).expect("write");
    let transcript_hash = board.structural_hash();

    // `:81-83` — the real thing, on a freshly loaded board.
    let mut real_board = p7t_common::load_board(dsn);
    let real_settings = build_pass_settings(&real_board, sorting_order, max_passes_arg);
    writeln!(out, "[real]").expect("write");
    let real_stop = RouterStop::new();
    let mut real_sink = NoopProgressSink;
    let summary = BatchFanout::fanout_board(
        &mut real_board,
        &real_settings,
        &real_stop,
        RouterBudget::disabled(),
        &mut real_sink,
    )
    .expect("fanout_board answers Ok on every path");
    let escape = summary.escape_statistics;
    writeln!(
        out,
        "REAL completedPassCount={} isTimedOut={} escapeTotalSmdPins={} escapedCount={} \
         escapedPercentage={} {}",
        summary.completed_pass_count,
        summary.is_timed_out,
        escape.total_smd_pins,
        escape.escaped_count,
        format_double(escape.escaped_percentage),
        p7t_common::board_shape(&mut real_board),
    )
    .expect("write");
    writeln!(out, "ESCAPE {}", escape_line(&mut real_board)).expect("write");
    writeln!(
        out,
        "EQUALS-TRANSCRIPT {}",
        real_board.structural_hash() == transcript_hash
    )
    .expect("write");

    writeln!(out, "[board]").expect("write");
    p7t_common::dump_board(out, &board);
}
