//! Port of `autoroute/pipeline/AutorouteUnroutedReport.java` (80 lines) — the diagnostic report
//! `AutorouteBatchLoop`/`BatchAutorouter` emit when the routing stage stagnates
//! (`BatchAutorouter.buildUnroutedConnectionsReport`, `BatchAutorouter.java:483-485`, a
//! package-private one-line delegate whose two callers are `AutorouteBatchLoop.java:457` and
//! `:487`; it carries a `// renamed:` marker pointing here, in `pipeline/batch_autorouter.rs`).
//!
//! It is a **consumer** of `fr-drc` (`new DesignRulesChecker(board, null)`,
//! `calculateAllIncompletes()`, `getAllAirlines()` at `AutorouteUnroutedReport.java:20-22`), which
//! is why it lives here rather than in `fr-drc` — `fr-drc`'s own marker at `src/lib.rs:143`
//! records the same decision from the other side, as a `// renamed:` rather than a forward
//! marker, now that this task discharges it.

use fr_board::items::Item;
use fr_board::{Board, ItemId};
use fr_drc::DesignRulesChecker;

/// Port of `AutorouteUnroutedReport.build(RoutingBoard)` (`:19-54`): every net that still has an
/// unrouted connection, in `getAllAirlines`' order, one indented line per airline.
///
/// The `LinkedHashMap<String, List<String>>` at `:28` is insertion-ordered by `getAllAirlines()`
/// order — a `Vec<(String, Vec<String>)>` here, **not** a `BTreeMap`, per ruling 5's recorded
/// decision (re-sorting by net name would silently change which report a stagnating run prints).
pub fn build_unrouted_report(board: &mut Board) -> String {
    // `:20-22`. The checker borrows `board` mutably for exactly as long as it takes to build the
    // airline list, which is `Vec<AirLine>` — owned data, no board reference — so the immutable
    // borrows `describe_item` needs below start once this block ends.
    let airlines = {
        let mut drc = DesignRulesChecker::new(board);
        drc.calculate_all_incompletes();
        drc.get_all_airlines()
    };

    // `:24-26`.
    if airlines.is_empty() {
        return "  (no unrouted connections found)".to_string();
    }

    // `:28-37`. `computeIfAbsent` over a `LinkedHashMap` — first-seen net name keeps its
    // insertion slot, every later airline of the same net appends to it.
    let mut by_net: Vec<(String, Vec<String>)> = Vec::new();
    for airline in &airlines {
        // `:30`. `board.rules.nets.get` answers `None` for a net number the board does not carry
        // (unreachable from this producer, but the fallback string is Java's own `null` arm).
        let net_name = board
            .rules
            .nets
            .get(airline.net_number)
            .map(|net| net.name.clone())
            .unwrap_or_else(|| "(unknown net)".to_string());
        // `:31-32`.
        let from_desc = describe_item(board, airline.from_item);
        let to_desc = describe_item(board, airline.to_item);
        // `:33-35`.
        let line = format!("    - {from_desc}  ->  {to_desc}");
        match by_net.iter_mut().find(|(name, _)| *name == net_name) {
            Some((_, lines)) => lines.push(line),
            None => by_net.push((net_name, vec![line])),
        }
    }

    // `:39-51`.
    let mut result = String::new();
    for (net_name, lines) in &by_net {
        let count = lines.len();
        result.push_str("  Net '");
        result.push_str(net_name);
        result.push_str("' (");
        result.push_str(&count.to_string());
        result.push_str(" unrouted connection");
        if count != 1 {
            result.push('s');
        }
        result.push_str("):\n");
        for line in lines {
            result.push_str(line);
            result.push('\n');
        }
    }
    // `:53`. `String.stripTrailing()` trims trailing whitespace only (never leading); `trim_end`
    // agrees with it on every character this function's own output can produce.
    result.trim_end().to_string()
}

/// Port of the private `describeItem(RoutingBoard, Item)` (`:60-79`): `ComponentName-PinName` for
/// a pin whose package names its pin index, `ComponentName (pin #N)` for a pin whose package does
/// not, and [`Item`]'s own `Display` (Java's `Item.toString()`, `Item.java:1257-1269`) for
/// everything else.
///
// totalized: Java wraps the whole `Pin` branch in `try { … } catch (Exception e) { // Fall
// through }` (`:62-77`), guarding `board.components.get(pin.getComponentId())` against the
// `ArrayIndexOutOfBoundsException` a pin with no assigned component (`componentId <= 0`) would
// throw. The port has no exception to catch, so the same guard is the bounds check
// `Board::is_placed_on_front` already uses for the identical lookup (`crates/fr-board/src/board/
// mod.rs`); a component id that passes it is guaranteed to resolve, so `Components::get` and
// `Packages::get` are called unguarded past that point, exactly as every other caller in this
// port already does (e.g. `Board::item_component_name`).
pub(crate) fn describe_item(board: &Board, item: ItemId) -> String {
    // `:61`.
    if let Some(Item::Pin(pin)) = board.get_item(item) {
        let component_id = pin.hdr.get_component_id();
        if component_id >= 1 && (component_id as usize) <= board.components.count() {
            // `:63`.
            let component = board.components.get(component_id);
            // `:64-65`. Java's `getPackage()` never answers `null` (both of `Component`'s package
            // fields are set at construction), so the port takes it unguarded rather than
            // modelling a `null` branch Java itself cannot reach.
            let package = board.library.packages.get(component.get_package());
            // `:66-70`. This is `Package.getPin`'s **real** `null` case (an out-of-range pin
            // index), which the port already answers as `None`.
            if let Some(package_pin) = package.get_pin(pin.get_pin_index()) {
                // `:69`.
                return format!("{}-{}", component.name, package_pin.name);
            }
            // `:73`.
            return format!("{} (pin #{})", component.name, pin.get_pin_index());
        }
    }
    // `:78`. `item != null` is always true here (`ItemId` is not itself nullable), so the only
    // arm this port can reach is `item.toString()`; `board.get_item` returning `None` — an id the
    // board does not know — is the one case Java's `null` fallback covers and this port cannot
    // otherwise construct, kept for totality.
    board
        .get_item(item)
        .map(|i| i.to_string())
        .unwrap_or_else(|| "(unknown)".to_string())
}
