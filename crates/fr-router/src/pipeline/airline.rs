//! Port of `autoroute/pipeline/AutorouteAirlineCalculator.java`.
//!
//! The file is **214** lines and declares six members behind a private constructor (`:14`).
//! **Four** of them are ported here and **two** are rostered `// not ported:` below with their
//! grep — a split Plan 9 Task 2 moved, and the reason is the whole of register row **#293**.
//!
//! * [`calculate_airline`] (`:16-40`) is the one `AutorouteConnectionRouter.java:70` calls on
//!   every routed connection.
//! * [`calculate_item_distance`] (`:162-177`), [`calculate_min_distance`] (`:179-202`) and
//!   [`get_item_reference_point`] (`:204-213`) had **no caller in Java** when the port was
//!   written, and Plan 7 rostered all three on that evidence. The evidence was correct and the
//!   conclusion was Java's bug, not the port's: commit `933d2980` deleted
//!   `autorouteItemList.sort(Comparator.comparingDouble(this::calculateItemDistance))` and left
//!   the three methods stranded. Task 2 restores the caller (see
//!   [`BatchAutorouter::autoroute_items_with_handled`](crate::pipeline::BatchAutorouter::autoroute_items_with_handled)),
//!   so the three are ported and their roster lines are **re-stated** below as ported rows.

use std::collections::{BTreeMap, BTreeSet};

use fr_board::items::Item;
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
//
// The three greps re-stated at Plan 9 Task 2 (register row #293). Each line was
// `// not ported: … no caller` and is now a **ported** row: the grep answer is unchanged in
// Java — `grep -rn "calculateItemDistance" src/main src/test` still finds only the declaration
// at `AutorouteAirlineCalculator.java:162` — and that is the defect, not the justification.
// renamed: `AutorouteAirlineCalculator.calculateItemDistance` (`:162-177`) -> `fr_router::pipeline::calculate_item_distance`; caller-less in Java since `933d2980` deleted the sort, and given its caller back here.
// renamed: `AutorouteAirlineCalculator.calculateMinDistance` (`:179-202`) -> `fr_router::pipeline::airline::calculate_min_distance`; private in Java and private here, reached only from `calculate_item_distance` (`:175`).
// renamed: `AutorouteAirlineCalculator.getItemReferencePoint` (`:204-213`) -> `fr_router::pipeline::airline::get_item_reference_point`; private in Java and private here, reached only from `calculate_min_distance` (`:183`, `:189`).

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

// =================================================================================================
// calculateItemDistance / calculateMinDistance / getItemReferencePoint — `:162-213`
//
// Ported at Plan 9 Task 2 (R1, register row #293). See the module doc for why they were rostered
// `// not ported:` before it and why that roster line was Java's bug rather than the port's.
// =================================================================================================

/// The memo [`calculate_item_distance`] runs behind, and the one the work-list sort shares across
/// its whole answer.
///
/// **Not a Java type.** Java re-extracts the key on *every comparison*
/// (`Comparator.comparingDouble` holds a function, not a table) and re-walks the board inside it,
/// so its sort evaluates `calculateItemDistance` `O(n log n)` times and re-derives every
/// reference point `|connected| × |unconnected|` times inside each of those. Every number below
/// is a pure function of a board the sort does not mutate, so caching them cannot change the
/// order — it only stops the port paying Java's bill.
///
/// Three things are memoised, and each is measured rather than guessed:
///
/// * **the key itself**, per item — quirk #213 puts a multi-net item in the work list once per
///   qualifying net, and every appearance has the same key;
/// * **`getConnectableItems(net)`**, per net — [`Board::get_connectable_items`] is a full scan of
///   the item table, and without this the sort is a second quadratic term beside the one
///   `getAutorouteItems` already pays at `:372`;
/// * **`getItemReferencePoint(item)`**, per item — `:191`'s double loop asks for the same points
///   `|from| × |to|` times, and a `Pin`'s centre is a padstack lookup and a transform, not a
///   field read.
#[derive(Debug, Default)]
pub(crate) struct ItemDistanceCache {
    /// `getConnectableItems(netNumber)` (BoardConnectivityQueries.java:25-35), per net.
    connectable: BTreeMap<i32, BTreeSet<ItemId>>,
    /// `calculateItemDistance(item)` (`:162-177`), per item.
    keys: BTreeMap<ItemId, f64>,
    /// `getItemReferencePoint(item)` (`:204-213`), per item — `None` is a memoised answer too.
    reference: BTreeMap<ItemId, Option<FloatPoint>>,
}

/// Port of `AutorouteAirlineCalculator.calculateItemDistance(Item)` (`:162-177`): "calculates the
/// shortest reference-point distance for an item and its incomplete connections", or
/// `Double.MAX_VALUE` when the item is on no net.
///
/// This is the sort key of the work list —
/// [`BatchAutorouter::autoroute_items_with_handled`](crate::pipeline::BatchAutorouter::autoroute_items_with_handled)
/// orders its answer ascending by it, so the pass routes the shortest airline first.
///
/// # `Double.MAX_VALUE` and `0` are both real answers
///
/// `:163-165` — an item with **no net** sorts last, because there is nothing to route it to.
/// `:172-174` — an item whose unconnected set is **empty** sorts first at exactly `0`: it is
/// already whole, and Java gives it the cheapest key rather than skipping it. Neither is a
/// sentinel the caller has to unpick; both are ordinary `f64`s that sort where Java sorts them,
/// and [`f64::total_cmp`] reproduces `Double.compare` on every one of them.
///
/// # `Set.of(item)` at `:176` is unreachable, and is ported anyway
///
/// `:176`'s ternary falls back to a singleton set of the item itself when `connectedSet` is
/// empty. It cannot be: `netNumber` comes from the item's own `getNetNumber(0)` (`:167`), so
/// `Item.getConnectedSet`'s `containsNet` guard (Item.java:602-604) passes and `:605` adds the
/// item itself before the recursion starts. The port reproduces the branch rather than asserting
/// it away, because the cost is three lines and the alternative is a panic on a board shape
/// nobody has enumerated.
#[must_use]
pub fn calculate_item_distance(board: &Board, item: ItemId) -> f64 {
    calculate_item_distance_cached(board, item, &mut ItemDistanceCache::default())
}

/// [`calculate_item_distance`] over a shared [`ItemDistanceCache`] — the form the work-list sort
/// calls, and the only reason the cache type exists. The answer is identical.
pub(crate) fn calculate_item_distance_cached(
    board: &Board,
    item: ItemId,
    cache: &mut ItemDistanceCache,
) -> f64 {
    if let Some(known) = cache.keys.get(&item) {
        return *known;
    }
    let distance = item_distance(board, item, cache);
    cache.keys.insert(item, distance);
    distance
}

fn item_distance(board: &Board, item: ItemId, cache: &mut ItemDistanceCache) -> f64 {
    let Some(current) = board.get_item(item) else {
        // Not a Java branch: Java holds the `Item` itself and cannot be handed a dangling id.
        // An id the board does not carry has no connections, which is `:164`'s answer.
        return f64::MAX;
    };
    // :163-165.
    if current.net_count() == 0 {
        return f64::MAX;
    }
    // :167.
    let net_number = current.get_net_number(0);
    // :168-169. The one-argument `getConnectedSet`, i.e. `stopAtPlane = false`
    // (Item.java:596-598), exactly as `getAutorouteItems:365` reads it.
    //
    // Java walks the connected set **twice** — once inside `getUnconnectedSet` (Item.java:684)
    // and once at `:169` — and the two walks answer the same set, because neither mutates the
    // board. The port walks it once and hands the answer to both.
    let connected_set = board.connected_set(item, net_number, false);
    let unconnected_set = unconnected_set_of(board, item, net_number, &connected_set, cache);

    // :171-174.
    if unconnected_set.is_empty() {
        return 0.0;
    }

    // :176-177.
    if connected_set.is_empty() {
        let singleton: BTreeSet<ItemId> = [item].into_iter().collect();
        return calculate_min_distance(board, &singleton, &unconnected_set, cache);
    }
    calculate_min_distance(board, &connected_set, &unconnected_set, cache)
}

/// Port of `Item.getUnconnectedSet(int)` (Item.java:671-690), with the two board scans it makes
/// lifted into [`ItemDistanceCache`].
///
/// [`Board::unconnected_set`] is the general form and is what every other caller uses; this one
/// exists because it is called once per work-list entry and both of its ingredients are shared
/// across the whole sort. `connected` is the caller's already-computed
/// `getConnectedSet(netNumber)` — Item.java:684 recomputes it, and the two are the same set.
fn unconnected_set_of(
    board: &Board,
    item: ItemId,
    net_number: i32,
    connected: &BTreeSet<ItemId>,
    cache: &mut ItemDistanceCache,
) -> BTreeSet<ItemId> {
    let mut result = BTreeSet::new();
    let Some(current) = board.get_item(item) else {
        return result;
    };
    // Item.java:673-675.
    if net_number > 0 && !current.contains_net(net_number) {
        return result;
    }
    if net_number > 0 {
        result.extend(connectable_items(board, net_number, cache).iter().copied());
    } else {
        // Item.java:679-682: every net the item is on.
        let nets: Vec<i32> = current.net_nos().to_vec();
        for current_net_number in nets {
            result.extend(
                connectable_items(board, current_net_number, cache)
                    .iter()
                    .copied(),
            );
        }
    }
    // Item.java:684.
    for id in connected {
        result.remove(id);
    }
    result
}

/// [`Board::get_connectable_items`], memoised per net for the life of one sort.
fn connectable_items<'a>(
    board: &Board,
    net_number: i32,
    cache: &'a mut ItemDistanceCache,
) -> &'a BTreeSet<ItemId> {
    cache.connectable.entry(net_number).or_insert_with(|| {
        board
            .get_connectable_items(net_number)
            .into_iter()
            .collect()
    })
}

/// Port of the private `AutorouteAirlineCalculator.calculateMinDistance(Collection<Item>,
/// Collection<Item>)` (`:179-202`): the smallest **Euclidean** reference-point distance across
/// the cross product, or `Double.MAX_VALUE` when no pair has two reference points.
///
/// Note the difference from [`calculate_airline`], which measures the **squared** distance
/// (`:31`) between **drill centres** only. This one measures the true distance (`:191`) between
/// [`get_item_reference_point`]s, which also has an answer for a trace. The two are different
/// functions of the same board and neither is a cheaper spelling of the other.
///
/// # Iteration order is not observable
///
/// `:192`'s `<` is strict, so the first pair at the minimum wins in either walk direction and the
/// answer is a `double` rather than the pair that produced it. The port's `BTreeSet<ItemId>`
/// walks ascending where Java's `TreeSet` walks descending (quirk #44), and the returned value is
/// identical.
///
/// The port resolves each side's reference points **once** into a `Vec` before the double loop,
/// where Java asks for them again on every pair (`:183`, `:189`). Same points, same minimum,
/// `|from| + |to|` lookups instead of `|from| × |to|`.
fn calculate_min_distance(
    board: &Board,
    from_items: &BTreeSet<ItemId>,
    to_items: &BTreeSet<ItemId>,
    cache: &mut ItemDistanceCache,
) -> f64 {
    // :183-186 and :189-192 — the `null` reference points are the `continue`s, so an item that
    // has none simply does not appear here.
    let from_points: Vec<FloatPoint> = from_items
        .iter()
        .filter_map(|id| reference_point(board, *id, cache))
        .collect();
    let to_points: Vec<FloatPoint> = to_items
        .iter()
        .filter_map(|id| reference_point(board, *id, cache))
        .collect();

    // :180.
    let mut min_distance = f64::MAX;
    // :182, :188.
    for from_point in &from_points {
        for to_point in &to_points {
            // :191-195.
            let distance = from_point.distance(to_point);
            if distance < min_distance {
                min_distance = distance;
            }
        }
    }

    // :201.
    min_distance
}

/// [`get_item_reference_point`], memoised per item for the life of one sort.
fn reference_point(
    board: &Board,
    item: ItemId,
    cache: &mut ItemDistanceCache,
) -> Option<FloatPoint> {
    if let Some(known) = cache.reference.get(&item) {
        return *known;
    }
    let point = get_item_reference_point(board, item);
    cache.reference.insert(item, point);
    point
}

/// Port of the private `AutorouteAirlineCalculator.getItemReferencePoint(Item)` (`:204-213`): a
/// drill item's centre, a trace's **end-to-end midpoint**, and `null` for everything else.
///
/// The midpoint at `:209-211` is the mean of `firstCorner` and `lastCorner` — **not** the
/// polyline's centroid and not a point on the trace: a hairpin's reference point sits off the
/// copper entirely. That is what Java measures and what the sort is therefore ordered by.
///
/// Java's `null` is [`None`]. `DrillItem` is the port's [`Item::Via`](fr_board::items::Item::Via)
/// and [`Item::Pin`](fr_board::items::Item::Pin), which is exactly what
/// [`Board::drill_center`](fr_board::Board::drill_center) dispatches over, so the two
/// `instanceof` arms are one call and one match here.
fn get_item_reference_point(board: &Board, item: ItemId) -> Option<FloatPoint> {
    // :205-207 — `item instanceof DrillItem`.
    if let Some(center) = board.drill_center(item) {
        return Some(center.to_float());
    }
    // :207-212 — `item instanceof PolylineTrace`.
    match board.get_item(item)? {
        Item::Trace(trace) => {
            let first = trace.first_corner()?.to_float();
            let last = trace.last_corner()?.to_float();
            // :211. The mean of the two corners, component-wise.
            Some(FloatPoint::new(
                (first.x + last.x) / 2.0,
                (first.y + last.y) / 2.0,
            ))
        }
        // :212 — every other subclass answers `null`.
        _ => None,
    }
}
