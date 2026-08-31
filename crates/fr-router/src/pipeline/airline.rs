//! Port of the one method of `autoroute/pipeline/AutorouteAirlineCalculator.java` that has a live
//! caller — `calculateAirline` (`:16-40`).
//!
//! The file is **214** lines and declares six members behind a private constructor (`:14`). Five
//! of them are rostered `// not ported:` below with their grep; the sixth is
//! [`calculate_airline`], which `AutorouteConnectionRouter.java:70` calls on every routed
//! connection.

use std::collections::BTreeSet;

use fr_board::{Board, ItemId};
use fr_geometry::{FloatLine, FloatPoint};

// The class is package-private with a private constructor (`AutorouteAirlineCalculator.java:12,
// 14`), so `audit-port.sh` sees only the five package-private statics it declares beside
// `calculateAirline`. Every one is reached from `nearestPointOnTrace`/`findClosestPointsBetweenTraces`
// or from nothing at all, and both of those are themselves unreached: a tree-wide
// `grep -rn "AutorouteAirlineCalculator\." src/main src/test` answers exactly one line,
// `AutorouteConnectionRouter.java:70`.
// renamed: `AutorouteAirlineCalculator.calculateAirline` (`:16-40`) -> `fr_router::pipeline::calculate_airline`; Java's package-private final class with a private constructor is a free function here, because it holds no state and Rust needs no class to hang a static on.
// not ported: `AutorouteAirlineCalculator.nearestPointOnTrace` (`:42-83`) — no caller; `grep -rn nearestPointOnTrace src/main src/test` finds only the declaration.
// not ported: `AutorouteAirlineCalculator.findClosestPointsBetweenTraces` (`:85-160`) — no caller; same grep.
// not ported: `AutorouteAirlineCalculator.calculateItemDistance` (`:162-177`) — no caller; same grep.
// not ported: `AutorouteAirlineCalculator.calculateMinDistance` (`:179-202`) — private, and its only caller is the unported `calculateItemDistance` (`:175`).
// not ported: `AutorouteAirlineCalculator.getItemReferencePoint` (`:204-213`) — private, and its only callers are the unported `calculateMinDistance` (`:183`, `:189`).

/// Port of `AutorouteAirlineCalculator.calculateAirline(Collection<Item>, Collection<Item>)`
/// (`AutorouteAirlineCalculator.java:16-40`): the shortest segment between a **drill item** of the
/// start set and a drill item of the destination set, measured by squared distance.
///
/// # It has no reader, and that is the whole story
///
/// Its one caller is `AutorouteConnectionRouter.java:70`,
/// `router.setAirLine(calculateAirline(routeStartSet, routeDestSet))`, which writes
/// `BatchAutorouter.airLine` (`:86`, via `setAirLine` at `:192-194`). That field's only reader is
/// the public `getAirLine()` accessor (`:519-527`), which the GUI draws and nothing headless
/// calls — Plan 6 already rostered the field `// not ported:` for exactly that reason (plan-7
/// ruling 6). So this function is ported for the audit and for the value's shape, and the pass
/// runner does not call it: `runSingleThread` sets `router.airLine = null` on all three of its
/// exits (`:164`, `:329`, `:333`) and never reads it.
///
/// # Java returns a `FloatLine` with `null` endpoints; the port returns `None`
///
/// `:39` is an unconditional `new FloatLine(fromCorner, toCorner)`, and `FloatLine`'s constructor
/// (`FloatLine.java:19-24`) accepts `null`s with nothing but an `FRLogger.debug` — so on a pair of
/// sets that hold no drill item at all, Java hands back a line whose `a` and `b` are both `null`.
/// The two are assigned together at `:33-35`, so one can never be null without the other.
/// `Option<FloatLine>` is therefore an exact model of the two reachable shapes, and the plan's
/// `-> Option<FloatLine>` is right for a reason the plan did not give.
///
/// # Iteration order
///
/// Java takes two `Set<Item>`s and the caller passes `getConnectedSet`/`getUnconnectedSet`
/// results, which are `TreeSet`s; the port's `BTreeSet<ItemId>` walks ascending where Java's walks
/// descending (quirk #44). **The order is not observable here**: `:32`'s `<` is strict, so the
/// first pair at the minimum distance wins in either direction, and ties keep whichever pair the
/// walk reached first. A tie between two *distinct* pairs at exactly the same squared distance
/// would resolve differently — and cannot be reached without two drill items sharing a centre,
/// which the board's own geometry forbids. The port walks ascending rather than reversing,
/// because reversing would claim a fidelity the sets' element type cannot deliver.
pub fn calculate_airline(
    board: &Board,
    from_items: &BTreeSet<ItemId>,
    to_items: &BTreeSet<ItemId>,
) -> Option<FloatLine> {
    // :17-19.
    let mut from_corner: Option<FloatPoint> = None;
    let mut to_corner: Option<FloatPoint> = None;
    let mut min_distance = f64::MAX;

    // :20.
    for from_id in from_items {
        // :21-23 — `!(currentFromItem instanceof DrillItem) → continue`.
        let Some(from_center) = board.drill_center(*from_id) else {
            continue;
        };
        // :24.
        let current_from_corner = from_center.to_float();

        // :26.
        for to_id in to_items {
            // :27-29.
            let Some(to_center) = board.drill_center(*to_id) else {
                continue;
            };
            // :30.
            let current_to_corner = to_center.to_float();
            // :31.
            let current_distance = current_from_corner.distance_square(&current_to_corner);
            // :32-36.
            if current_distance < min_distance {
                min_distance = current_distance;
                from_corner = Some(current_from_corner);
                to_corner = Some(current_to_corner);
            }
        }
    }

    // :39. Both endpoints are assigned together, so this is `Some` exactly when Java's line has
    // non-`null` endpoints.
    Some(FloatLine::new(from_corner?, to_corner?))
}
