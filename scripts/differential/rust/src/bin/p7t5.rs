//! Rust half of the `p7t5` differential pair — `scripts/differential/java/P7T5.java`.
//!
//! Plan 7 Task 11: `BatchFanout`'s component/pin ordering (`BatchFanout.java:35-78`, `:631-693`,
//! `:695-778`) and `RoutingBoard.fanout` (`RoutingBoard.java:978-1110`).
//!
//! Usage: `p7t5 <dsn> [passNo] [sortingOrder] [order|pin]`. See `P7T5.java`'s class comment for
//! what each mode prints, for why the removed-id set stands in for `rippedItemList`, and for the
//! budget note. The shared board/settings ladder is [`p7t_common`], the same one `p7t1`, `p7t2`
//! and `p7t9` use.
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
use fr_dsn::java_double_to_string;
use fr_router::board_ext::RoutingBoardExt;
use fr_router::pipeline::{BatchFanout, RouterBudget};
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
        eprintln!("usage: p7t5 <dsn> [passNo] [sortingOrder] [order|pin]");
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
    if mode != "order" && mode != "pin" {
        eprintln!("p7t5: mode must be `order` or `pin`, not '{mode}'");
        std::process::exit(2);
    }

    let stdout = std::io::stdout();
    let mut out = BufWriter::new(stdout.lock());

    let (jar, bytes, mtime) = p7t_common::jar_identity();
    writeln!(
        out,
        "HEADER jar={jar} bytes={bytes} mtime={mtime} fixture={} passNo={pass_no} \
         sortingOrder={sorting_order} mode={mode}",
        dsn.file_name().expect("a file name").to_string_lossy(),
    )
    .expect("write");
    if let Ok(exe) = std::env::current_exe() {
        eprintln!("rust-binary {}", exe.display());
    }

    if mode == "order" {
        for order in SORTING_ORDERS {
            let board = p7t_common::load_board(&dsn);
            let mut settings = p7t_common::build_settings(&board);
            set_sorting_order(&mut settings, order);
            dump_order(&mut out, &board, &settings, order);
        }
    } else {
        let mut board = p7t_common::load_board(&dsn);
        let mut settings = p7t_common::build_settings(&board);
        set_sorting_order(&mut settings, sorting_order);
        dump_fanout_run(&mut out, &mut board, &settings, pass_no);
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
            java_double_to_string(component.gravity_center_of_smd_pins.x),
            java_double_to_string(component.gravity_center_of_smd_pins.y),
        )
        .expect("write");
        for (pin_index, pin) in component.smd_pins.iter().enumerate() {
            let name = match board.get_item(pin.pin) {
                Some(Item::Pin(p)) => p.name(&ctx).map(str::to_owned),
                _ => None,
            };
            writeln!(
                out,
                "  PIN {pin_index} id={} pinIndex={} name={} distToCentre={} \
                 distToClosestOnNet={} surroundingsDensity={}",
                pin.pin.0,
                pin.pin_index,
                name.as_deref()
                    .map_or("null".to_string(), p7t_common::quote),
                java_double_to_string(pin.distance_to_component_center),
                java_double_to_string(pin.distance_to_closest_on_net),
                pin.surroundings_density,
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
                    component.smd_pins.iter().map(|pin| pin.pin).collect(),
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
            );
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
