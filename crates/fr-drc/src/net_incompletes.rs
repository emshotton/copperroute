//! `drc.NetIncompletes`: the ratsnest of **one** net — a Delaunay triangulation of the net's
//! items, reduced to a minimum spanning tree by Kruskal's algorithm over the net's already
//! connected groups, plus the net's length violation.
//!
//! Java: `drc/NetIncompletes.java`. `DesignRulesChecker.calculateAllIncompletes`
//! (DesignRulesChecker.java:542-623, Task 6) is its only producer, one instance per net number.
//!
//! # What is parity here and what is not
//!
//! Plan-5 ruling 4: the airline **list** is not a parity surface and the airline **counts** are.
//! Java feeds the triangulation from `calculateNetItems`, whose outer loop seeds off a
//! `HashSet<Item>` (`:295`, `:299`) over a class with no `hashCode` override, so the corner
//! insertion order — and therefore which of several equal-length Delaunay edges the spanning
//! tree accepts — is identity-hash ordered. JVM-verified with `-XX:hashCode=0..4`: five
//! different airline lists on the dev board, identical `incompleteCount`. The port seeds in
//! **ascending item id** (ruling 3, quirks row #144); the order *within* one connected group is
//! **not** free and is Java's **descending** id (ruling 15, quirk #44 — `Item.getConnectedSet`
//! returns a `TreeSet` keyed by the reversed `Item.compareTo`).
//!
//! Two consequences follow, and both are deliberate:
//!
//! * `count()`, `get_connected_group_count()` and `get_length_violation()` are compared to the
//!   JVM exactly (`tests/net_incompletes.rs`).
//! * `incompletes`' endpoints are compared informationally only.
//!
//! Quirk #82 — `PlanarDelaunayTriangulation`'s in-circle degeneracy on axis-aligned input, which
//! loses edges a real triangulation would have — **must not be fixed** here (`docs/plan-2-handoff.md`
//! ruling 17). It decides which airlines exist.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use fr_board::{Board, DelaunayCorner, ItemId, ItemKind, PlanarDelaunayTriangulation};
use fr_geometry::{FloatPoint, Signum};

use crate::airline::AirLine;

/// Port of `drc.NetIncompletes` (NetIncompletes.java:28-398).
#[derive(Debug, Clone, PartialEq)]
pub struct NetIncompletes {
    /// Java `incompletes` (NetIncompletes.java:31), a `LinkedList<AirLine>` in the order
    /// Kruskal accepted the edges.
    ///
    // renamed: NetIncompletes.getIncompletes (:229-231) -> this public field, which Java also
    // exposes directly (`:31` is package-private and `DesignRulesChecker.getAllAirlines` reads it
    // that way, DesignRulesChecker.java:786). A getter over a public field would be noise.
    pub incompletes: Vec<AirLine>,
    /// Java `net` (NetIncompletes.java:34), as a net number — see [`AirLine::net_number`].
    net_number: i32,
    /// Java `drawMarkerRadius` (NetIncompletes.java:37).
    draw_marker_radius: f64,
    /// Java `lengthViolation` (NetIncompletes.java:44): `> 0` too long, `< 0` too short, `0` ok
    /// or unrestricted.
    length_violation: f64,
    /// Java `connectedGroupCount` (NetIncompletes.java:47).
    connected_group_count: usize,
}

impl NetIncompletes {
    /// Port of `NetIncompletes(int, Collection<Item>, BasicBoard)` (NetIncompletes.java:57-226).
    ///
    /// `net_items` is every connectable item of the net, as
    /// `DesignRulesChecker.calculateAllIncompletes` collects it (`:550-563`). **Its order is not
    /// observable**: the only thing done with it is the filter below, and the filtered list goes
    /// straight into `calculate_net_items`, which drops the order into a set (`:295`).
    ///
    // not ported: the six `FRLogger.trace` calls (NetIncompletes.java:64-74, :118-132, :143-152,
    // :156-161, :207-223) and the four counters that exist only to feed them (`danglingCount`,
    // `unconnectedCount`, `conductionAreaCount`, `conductionAreaFilteredCount`, `:81-84`).
    pub fn new(net_number: i32, net_items: &[ItemId], board: &Board) -> NetIncompletes {
        // NetIncompletes.java:58. Java computes `int * 2` and widens; the product cannot overflow
        // an `i32` for any half width a board rules object holds.
        let draw_marker_radius = f64::from(board.rules.get_min_trace_half_width()) * 2.0;

        let mut this = NetIncompletes {
            // NetIncompletes.java:59.
            incompletes: Vec::new(),
            // NetIncompletes.java:60: `board.rules.nets.get(netNumber)`, which may be null. The
            // port keeps the number and resolves the net where Java would dereference it.
            net_number,
            draw_marker_radius,
            // NetIncompletes.java:44.
            length_violation: 0.0,
            connected_group_count: 0,
        };

        // NetIncompletes.java:80-116: drop the dangling items and the contact-free ones.
        //
        // The second test is the double negative Java wrote: an item with no normal contacts is
        // dropped **unless** it is a `ConductionArea` (a connection medium) or a `DrillItem`
        // (a `Pin` or a `Via` — an unrouted pin legitimately has no contacts and *should* reach
        // the ratsnest). Transcribed as written.
        let filtered: Vec<ItemId> = net_items
            .iter()
            .copied()
            .filter(|&id| {
                // NetIncompletes.java:93-96.
                if board.is_tail(id) {
                    return false;
                }
                // totalized: Java's loop variable *is* the `Item` (NetIncompletes.java:85), so
                // there is no lookup to fail. The port carries ids, and an id that is not on the
                // board has no kind and no contacts; dropping it is the only answer that keeps
                // the filter total. Unreachable from any producer — `calculateAllIncompletes`
                // collects the ids **from** the board.
                let Some(item) = board.get_item(id) else {
                    debug_assert!(false, "net item {id:?} is not on the board");
                    return false;
                };
                // NetIncompletes.java:103-108. `DrillItem` is Java's `Pin`/`Via` superclass.
                let exempt = matches!(
                    item.kind(),
                    ItemKind::ConductionArea | ItemKind::Pin | ItemKind::Via
                );
                exempt || !board.normal_contacts(id).is_empty()
            })
            .collect();

        // NetIncompletes.java:135.
        let (mut grouped_net_items, connected_sets) =
            calculate_net_items(net_number, &filtered, board);

        // NetIncompletes.java:137-141.
        //
        // Java bug: `uniqueConnectedSets` is a `HashSet<Collection<Item>>`, so its element
        // equality is `AbstractSet.equals` — **value**-based over the `TreeSet`s
        // `Item.getConnectedSet` returns, not the reference identity the very next loop
        // (`:192`) relies on. Two distinct-but-equal connected sets would therefore collapse
        // into one here and still produce an airline there. Unreachable in practice — two
        // distinct components of the same net are disjoint and non-empty, so they can never be
        // equal — but the port counts distinct set *contents* rather than distinct set ids so
        // that it agrees with Java's value semantics wherever they do differ.
        let unique_connected_sets: BTreeSet<&BTreeSet<ItemId>> = connected_sets.iter().collect();
        this.connected_group_count = unique_connected_sets.len();

        // NetIncompletes.java:154-163: nothing to connect, and `calcLengthViolation` is **not**
        // called, so a one-item net's `lengthViolation` stays 0 however short its traces are.
        if grouped_net_items.len() <= 1 {
            this.connected_group_count = grouped_net_items.len();
            return this;
        }

        // NetIncompletes.java:166-169. Java's `PlanarDelaunayTriangulation` constructor flattens
        // its `Storable`s by calling `getTriangulationCorners()` on each in list order
        // (PlanarDelaunayTriangulation.java:50-56) — for a `NetItem` that is
        // `item.getRatsnestCorners()` (`:394-396`). The port's constructor takes the flattened
        // list, so the flattening happens here, in the same two nested orders.
        let mut corners: Vec<DelaunayCorner> = Vec::new();
        for net_item in &grouped_net_items {
            for point in board.ratsnest_corners(net_item.item) {
                corners.push(DelaunayCorner::new(net_item.item, point));
            }
        }
        let triangulation = PlanarDelaunayTriangulation::new(&corners);

        // NetIncompletes.java:172-184: the candidate edges, sorted by length. `TreeSet` keeps the
        // first of any group its comparator calls equal and drops the rest, and on the finite
        // coordinates a real board produces `BTreeSet` drops exactly the same ones — see
        // [`Edge::cmp`] for the one input where that equivalence stops holding.
        //
        // An item appears at most once in `grouped_net_items` (`calculate_net_items` seeds from a
        // set), so an item id identifies its `NetItem` index, which is what the Kruskal step
        // below compares.
        let index_of: BTreeMap<ItemId, usize> = grouped_net_items
            .iter()
            .enumerate()
            .map(|(index, net_item)| (net_item.item, index))
            .collect();
        let mut sorted_edges: BTreeSet<Edge> = BTreeSet::new();
        for line in triangulation.get_edge_lines() {
            // totalized: Java casts `currentLine.startObject` to `NetItem` unconditionally
            // (NetIncompletes.java:179, :181) and throws a `NullPointerException` at `:192` when
            // it is null — a bounding-triangle corner, whose `Storable` is null
            // (PlanarDelaunayTriangulation.java:67-69). `getEdgeLines` filters those out of the
            // leaf edges (`:720-723`) but not out of the degenerate ones (`:160`), so the port
            // skips such an edge instead of panicking.
            let (Some(start), Some(end)) = (line.start_object, line.end_object) else {
                continue;
            };
            // totalized: Java casts the `Storable` straight back to the `NetItem` it handed in
            // (NetIncompletes.java:179, :181) — object identity, which cannot miss. The port's
            // `DelaunayCorner` carries an `ItemId` instead, and every id the triangulation was
            // given came from `grouped_net_items`, so this arm is unreachable; skipping keeps the
            // constructor total where Java would throw a `ClassCastException`.
            let (Some(&from_item), Some(&to_item)) = (index_of.get(&start), index_of.get(&end))
            else {
                debug_assert!(
                    false,
                    "a triangulation corner names an item the net does not have"
                );
                continue;
            };
            sorted_edges.insert(Edge {
                from_item,
                from_corner: line.start_point.to_float(),
                to_item,
                to_corner: line.end_point.to_float(),
                length_square: line.length_square(),
            });
        }

        // NetIncompletes.java:190-205: Kruskal. An edge whose two ends are already in one
        // connected set is skipped; every other edge becomes an airline and merges the two sets.
        for edge in sorted_edges {
            let from_set = grouped_net_items[edge.from_item].set_id;
            let to_set = grouped_net_items[edge.to_item].set_id;
            // NetIncompletes.java:192: Java compares the two `Collection` **references**, not
            // their contents; `set_id` is that identity.
            if from_set == to_set {
                continue;
            }
            this.incompletes.push(AirLine::new(
                net_number,
                grouped_net_items[edge.from_item].item,
                edge.from_corner,
                grouped_net_items[edge.to_item].item,
                edge.to_corner,
            ));
            join_connected_sets(&mut grouped_net_items, from_set, to_set);
        }

        // NetIncompletes.java:225.
        this.calc_length_violation(board);
        this
    }

    /// Port of `NetIncompletes.count` (NetIncompletes.java:247-249): the number of airlines.
    pub fn count(&self) -> usize {
        self.incompletes.len()
    }

    /// Port of `NetIncompletes.getConnectedGroupCount` (NetIncompletes.java:252-254): the number
    /// of connected groups this net had when the ratsnest was computed. `count()` is one less
    /// than this on a net whose triangulation reached every group.
    pub fn get_connected_group_count(&self) -> usize {
        self.connected_group_count
    }

    /// Port of `NetIncompletes.getMarkerRadius` (NetIncompletes.java:242-244).
    pub fn get_marker_radius(&self) -> f64 {
        self.draw_marker_radius
    }

    /// Port of `NetIncompletes.getLengthViolation` (NetIncompletes.java:282-284): `> 0` the net's
    /// cumulative trace length is over its class's maximum, `< 0` under its minimum, `0` neither.
    pub fn get_length_violation(&self) -> f64 {
        self.length_violation
    }

    /// The net this ratsnest belongs to.
    ///
    // renamed: NetIncompletes.getNet (NetIncompletes.java:234-236) -> get_net_number. Java hands
    // back the `Net` object; the port stores the number and leaves the lookup to the caller,
    // which already holds the `Board`.
    pub fn get_net_number(&self) -> i32 {
        self.net_number
    }

    /// Port of `NetIncompletes.calcLengthViolation` (NetIncompletes.java:257-275): recomputes
    /// [`Self::get_length_violation`] and answers **whether it moved by more than 0.1**.
    ///
    /// Java is a no-argument method reading `this.net.getTraceLength()`, which walks the board
    /// through the net's back-pointer (Net.java:130-140); the port takes the board instead.
    ///
    /// The minimum-length arm fires only when `incompletes` is **empty** (`:269`): a net that is
    /// still unrouted is short for a reason, and Java does not want to report that twice. The
    /// maximum-length arm has no such guard.
    ///
    // totalized: Java dereferences `this.net.getNetClass()` (`:258-259`) and throws a
    // `NullPointerException` when `board.rules.nets.get(netNumber)` returned null at `:60`. A net
    // number with no net has no length restriction, so the port clears the violation and reports
    // "unchanged", which is the same answer the `maxLength <= 0 && minLength <= 0` arm gives.
    pub fn calc_length_violation(&mut self, board: &Board) -> bool {
        let Some(net) = board.rules.nets.get(self.net_number) else {
            self.length_violation = 0.0;
            return false;
        };
        let net_class = board.rules.net_classes.get(net.get_net_class());
        // NetIncompletes.java:258-259.
        let max_length = net_class.get_maximum_trace_length();
        let min_length = net_class.get_minimum_trace_length();
        // NetIncompletes.java:260-263. Note this returns `false` while *also* writing the field,
        // so a net whose restriction is removed reports "unchanged" on the run that clears it.
        if max_length <= 0.0 && min_length <= 0.0 {
            self.length_violation = 0.0;
            return false;
        }
        // NetIncompletes.java:264-271.
        let mut new_violation = 0.0;
        let trace_length = board.net_trace_length(self.net_number);
        if max_length > 0.0 && trace_length > max_length {
            new_violation = trace_length - max_length;
        }
        if min_length > 0.0 && trace_length < min_length && self.incompletes.is_empty() {
            new_violation = trace_length - min_length;
        }
        // NetIncompletes.java:272-274.
        let old_violation = self.length_violation;
        self.length_violation = new_violation;
        (new_violation - old_violation).abs() > 0.1
    }
}

/// Port of the private `NetIncompletes.NetItem` (NetIncompletes.java:383-397): one item of the
/// net, tagged with the connected group it currently belongs to.
///
// renamed: Java's `Collection<Item> connectedSet` field -> `set_id`, an index that stands in for
// the `Collection` **reference** Java compares at `:192` and re-points at `:332-334`.
//
// not ported: NetItem.getTriangulationCorners (NetIncompletes.java:393-396) — the port's
// `PlanarDelaunayTriangulation` takes the flattened corner list rather than a `Storable`
// interface, so `NetIncompletes::new` calls `Board::ratsnest_corners` at the insertion site
// instead. See `crates/fr-board/src/datastructures/delaunay.rs`' module docs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NetItem {
    item: ItemId,
    set_id: usize,
}

/// Port of the private `NetIncompletes.calculateNetItems` (NetIncompletes.java:293-322).
///
/// Returns the `NetItem` array **and** the connected set behind each `set_id`, which the
/// constructor needs for its value-based group count (`:137-141`).
///
/// # The two orders, and which of them is Java's
///
/// Java's outer loop seeds from `uniqueItems.iterator().next()` (`:299`) over a `HashSet<Item>`,
/// which is identity-hash ordered; plan-5 ruling 3 makes that **ascending item id** here.
///
/// The order *within* one component is not free. `Item.getConnectedSet` returns a
/// `TreeSet<Item>` (Item.java:607) keyed by `Item.compareTo`, whose subtraction is **reversed**
/// (Item.java:95-103, quirk #44), so the walk at `:304-308` sees **descending** item id and the
/// `NetItem` array — hence the Delaunay corner insertion order — is descending inside every
/// component. `Board::connected_set` returns an ascending `BTreeSet`, so this iterates `.rev()`.
/// Plan-5 ruling 15; getting it backwards changes which equal-length edge the ratsnest picks.
///
// not ported: the two `FRLogger.warn` size checks (NetIncompletes.java:316-320). They compare
// `result.size()` against `uniqueItems.size()` and can only fire if `getConnectedSet` returned an
// item twice, which a `TreeSet` cannot.
fn calculate_net_items(
    net_number: i32,
    item_list: &[ItemId],
    board: &Board,
) -> (Vec<NetItem>, Vec<BTreeSet<ItemId>>) {
    // NetIncompletes.java:295 — a `HashSet`, i.e. deduplicating; the `BTreeSet` also fixes the
    // seed order (ruling 3).
    let mut unique_items: BTreeSet<ItemId> = item_list.iter().copied().collect();
    let mut result: Vec<NetItem> = Vec::new();
    let mut connected_sets: Vec<BTreeSet<ItemId>> = Vec::new();

    // NetIncompletes.java:298-314.
    while let Some(&start_item) = unique_items.iter().next() {
        // NetIncompletes.java:300. Java reads `this.net.netNumber`, which is `netNumber` whenever
        // the net exists and a `NullPointerException` when it does not.
        let current_connected_set = board.connected_set(start_item, net_number, false);

        // NetIncompletes.java:303-308, `.rev()` for ruling 15.
        let items_in_component: Vec<ItemId> = current_connected_set
            .iter()
            .rev()
            .copied()
            .filter(|id| unique_items.contains(id))
            .collect();

        // totalized: when `start_item` does not carry `net_number`, `Item.getConnectedSet`
        // returns an **empty** set (Item.java:607-609), `itemsInComponent` is empty,
        // `uniqueItems.removeAll` removes nothing and Java's `while` loop **never terminates** —
        // it re-seeds off the same item forever. The port drops the seed and carries on. Every
        // Java producer groups the items by a net they contain
        // (DesignRulesChecker.java:557-561), so the arm is unreachable from `calculateAllIncompletes`.
        if items_in_component.is_empty() {
            unique_items.remove(&start_item);
            continue;
        }

        // NetIncompletes.java:310-312: every item of the component shares one `connectedSet`
        // reference, which is what `set_id` stands in for.
        let set_id = connected_sets.len();
        for &id in &items_in_component {
            result.push(NetItem { item: id, set_id });
        }
        // NetIncompletes.java:313.
        for id in &items_in_component {
            unique_items.remove(id);
        }
        connected_sets.push(current_connected_set);
    }

    (result, connected_sets)
}

/// Port of the private `NetIncompletes.joinConnectedSets` (NetIncompletes.java:328-337): every
/// `NetItem` in the *from* set is re-pointed at the *to* set.
///
// renamed: Java's `toConnectedSet.add(currentItem.item)` (`:333`) has no counterpart. It mutates
// the `TreeSet` `getConnectedSet` returned so that the merged set's *contents* are the union;
// nothing ever reads those contents again — `:192` compares references and the constructor
// already took its group count at `:141` — so the `set_id` relabelling is the whole of the
// observable behaviour.
fn join_connected_sets(net_items: &mut [NetItem], from_set: usize, to_set: usize) {
    for net_item in net_items.iter_mut() {
        // NetIncompletes.java:332.
        if net_item.set_id == from_set {
            net_item.set_id = to_set;
        }
    }
}

/// Port of the private `NetIncompletes.Edge` (NetIncompletes.java:344-380): one candidate airline,
/// ordered by length and then by its four coordinates.
///
// renamed: Edge.compareTo (NetIncompletes.java:360-379) -> the `Ord` impl below. Rust's
// `BTreeSet` takes its ordering from the element type rather than from a `Comparable` method, so
// there is no separate method to name; the class is private in Java too.
///
/// `from_item`/`to_item` are **indices into the `NetItem` array**, not [`ItemId`]s: the Kruskal
/// step compares connected-set identity, which lives on the `NetItem`.
#[derive(Debug, Clone)]
struct Edge {
    from_item: usize,
    from_corner: FloatPoint,
    to_item: usize,
    to_corner: FloatPoint,
    /// `toCorner.distanceSquare(fromCorner)` (NetIncompletes.java:357), which
    /// `DelaunayEdge::length_square` computes from the same two points.
    length_square: f64,
}

/// `PartialEq` is **defined through [`Edge::cmp`]**, not derived.
///
/// The two are not the same relation: `cmp` calls two edges equal whenever all five of its keys
/// tie, which happens between edges with different `from_item`/`to_item` (quirk #147), and a
/// derived, structural `PartialEq` would call those pairs different. `Ord`'s contract is that
/// `a.cmp(b) == Equal` exactly when `a == b`, and `BTreeSet` is entitled to rely on it, so the
/// port makes the equality the comparator's rather than the fields'. Java has the same split and
/// simply never exposes it: `Edge` inherits `Object.equals` (identity) and `TreeSet` uses
/// `compareTo` alone, so the two disagree there too — harmlessly, because nothing calls `equals`.
impl PartialEq for Edge {
    fn eq(&self, other: &Edge) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

/// `Eq` is asserted, not proved: [`Edge::cmp`] is **not reflexive** on an edge whose coordinates
/// make a subtraction NaN, because `Signum.asInt(NaN)` is 0 only by falling off the end of its
/// ladder — that edge compares `Equal` to itself for the wrong reason, and `Equal` to everything
/// else as well. The impl is here because `BTreeSet` requires `Ord: Eq` and the port needs the
/// `TreeSet` behaviour; Java is in the identical position, its `Comparable` contract broken on
/// the same input, and neither side has a caller that reaches it (a `FloatPoint` here always
/// comes from an `IntPoint`). See the `Java bug:` marker on [`Edge::cmp`].
impl Eq for Edge {}

impl PartialOrd for Edge {
    fn partial_cmp(&self, other: &Edge) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Edge {
    /// Port of `Edge.compareTo` (NetIncompletes.java:360-379), verbatim — including the `f64`
    /// subtraction and `Signum.asInt`, **not** `f64::partial_cmp`.
    ///
    /// Java's comment at `:364-366` explains the four coordinate tie-breakers: "prevent result 0,
    /// so that edges with the same length as another edge are not skipped in the set". They do
    /// not prevent it.
    ///
    // Java bug: the comparator is not injective, so the `TreeSet` at `:174` silently **drops**
    // edges. Two distinct candidate airlines that agree on all five keys — same squared length,
    // same `fromCorner`, same `toCorner`, different items — compare 0, and the second `add`
    // returns false. That happens whenever two different items contribute the *same* ratsnest
    // corner, which coincident pads and a via stacked on a pad both do. A dropped edge is one
    // fewer airline candidate, so the spanning tree may join the two groups through a longer
    // edge, or — if the drop takes the only edge between them — leave them unjoined, understating
    // `incompleteCount`. On finite input the comparator is a consistent total preorder, so
    // `BTreeSet` and `TreeSet` agree exactly on *which* edge is dropped — the later one offered —
    // and the port reproduces Java edge for edge.
    //
    // The NaN half is weaker, in two ways worth stating precisely. `Signum.asInt` is a
    // `> 0 / < 0 / else` ladder (Signum.java:35-42), so it maps **NaN** to 0: an edge whose
    // coordinates make a subtraction NaN compares `Equal` to every other edge. What that costs
    // depends on **insertion order** — offered first, it swallows every later edge and the set
    // ends up with one element; offered later, it is simply the edge that is dropped. And because
    // the relation is then not transitive, Rust's B-tree and Java's red-black tree may
    // legitimately drop *different* edges: the port reproduces Java's **comparator**, not Java's
    // tree, and only the consistent case is a parity claim. Reproduced; quirks row #147, tests
    // `an_exact_five_way_tie_drops_the_second_edge` and
    // `a_nan_edge_swallows_or_is_swallowed_depending_on_insertion_order`.
    fn cmp(&self, other: &Edge) -> Ordering {
        // NetIncompletes.java:362.
        let mut result = self.length_square - other.length_square;
        if result == 0.0 {
            // NetIncompletes.java:367-376. Note the three inner tests are **not** nested past
            // the first: `fromCorner.y` is only consulted when `fromCorner.x` tied, but
            // `toCorner.x` is consulted whenever the value so far is 0, which after the `y` test
            // it may have become again.
            result = self.from_corner.x - other.from_corner.x;
            if result == 0.0 {
                result = self.from_corner.y - other.from_corner.y;
            }
            if result == 0.0 {
                result = self.to_corner.x - other.to_corner.x;
            }
            if result == 0.0 {
                result = self.to_corner.y - other.to_corner.y;
            }
        }
        // NetIncompletes.java:378.
        match Signum::as_int_f64(result) {
            -1 => Ordering::Less,
            1 => Ordering::Greater,
            _ => Ordering::Equal,
        }
    }
}

#[cfg(test)]
mod tests {
    //! The two order rulings and the `Edge` comparator are tested here rather than in
    //! `tests/net_incompletes.rs` because `calculate_net_items`, `NetItem` and `Edge` are all
    //! private — as they are in Java. Everything observable through the public surface is tested
    //! in the integration file.

    use super::*;
    use fr_board::prelude::*;
    use fr_geometry::{IntBox, IntPoint, Point, Polyline};

    const BOUNDING_BOX: IntBox = IntBox {
        ll: IntPoint {
            x: -100_000,
            y: -100_000,
        },
        ur: IntPoint {
            x: 100_000,
            y: 100_000,
        },
    };

    fn layers() -> LayerStructure {
        LayerStructure::new(vec![Layer::new("front", true)])
    }

    /// A bare one-layer board with one net `N1` and no components. `Board::new` inserts the board
    /// outline as item **1**, so the first trace below is item 2.
    fn bare_board() -> Board {
        let ls = layers();
        let matrix = ClearanceMatrix::get_default_instance(&ls, 200);
        let mut rules = BoardRules::new(ls, matrix);
        rules.create_default_net_class();
        let default_class = rules.get_default_net_class();
        let mut board = Board::new(
            Vec::new(),
            1,
            BOUNDING_BOX,
            rules,
            BoardLibrary::new(Padstacks::new(layers()), Packages::new()),
            Components::new(),
            Communication::default(),
        );
        board.rules.nets.add("N1", 1, false, default_class);
        board
    }

    fn insert_trace(board: &mut Board, from: (i32, i32), to: (i32, i32)) -> ItemId {
        board
            .insert_trace_without_cleaning(
                Polyline::from_points(&[Point::new(from.0, from.1), Point::new(to.0, to.1)]),
                0,
                30,
                vec![1],
                1,
                FixedState::Unfixed,
            )
            .expect("the synthetic trace is neither degenerate nor closed")
    }

    #[test]
    fn net_items_are_ordered_descending_within_a_component() {
        // Plan-5 ruling 15. A four-trace chain on net 1: items 2-3-4-5, each touching the next,
        // so `Board::connected_set` answers `{2, 3, 4, 5}` ascending and Java's `TreeSet` answers
        // it descending (quirk #44). The `NetItem` array — the Delaunay insertion order — must be
        // the descending one.
        let mut board = bare_board();
        let ids: Vec<ItemId> = (0..4)
            .map(|i| insert_trace(&mut board, (i * 1000, 0), ((i + 1) * 1000, 0)))
            .collect();
        assert_eq!(ids, [2, 3, 4, 5].map(ItemId));

        let (net_items, sets) = calculate_net_items(1, &ids, &board);
        assert_eq!(
            net_items.iter().map(|n| n.item).collect::<Vec<_>>(),
            [5, 4, 3, 2].map(ItemId),
        );
        // One component, so one `set_id` and one connected set.
        assert!(net_items.iter().all(|n| n.set_id == 0));
        assert_eq!(sets.len(), 1);
    }

    #[test]
    fn seeds_are_taken_in_ascending_id_order() {
        // Plan-5 ruling 3. Two disjoint components, `{2, 3}` at the origin and `{4, 5}` far away.
        // The seed is `uniqueItems.iterator().next()` over a `HashSet` in Java and the lowest id
        // here, so component `{2, 3}` comes first — and each component is still descending.
        let mut board = bare_board();
        let a1 = insert_trace(&mut board, (0, 0), (1000, 0));
        let a2 = insert_trace(&mut board, (1000, 0), (2000, 0));
        let b1 = insert_trace(&mut board, (50_000, 0), (51_000, 0));
        let b2 = insert_trace(&mut board, (51_000, 0), (52_000, 0));
        assert_eq!([a1, a2, b1, b2], [2, 3, 4, 5].map(ItemId));

        let (net_items, sets) = calculate_net_items(1, &[a1, a2, b1, b2], &board);
        assert_eq!(
            net_items.iter().map(|n| n.item).collect::<Vec<_>>(),
            [3, 2, 5, 4].map(ItemId),
        );
        assert_eq!(
            net_items.iter().map(|n| n.set_id).collect::<Vec<_>>(),
            [0, 0, 1, 1],
        );
        assert_eq!(sets.len(), 2);
    }

    #[test]
    fn joining_two_sets_repoints_every_member_of_the_from_set() {
        // `joinConnectedSets` (`:328-337`): *every* `NetItem` whose set is the from set, not just
        // the edge's own endpoint.
        let mut net_items = vec![
            NetItem {
                item: ItemId(2),
                set_id: 0,
            },
            NetItem {
                item: ItemId(3),
                set_id: 0,
            },
            NetItem {
                item: ItemId(4),
                set_id: 1,
            },
        ];
        join_connected_sets(&mut net_items, 0, 1);
        assert!(net_items.iter().all(|n| n.set_id == 1));
    }

    fn edge(from_item: usize, from: (f64, f64), to_item: usize, to: (f64, f64)) -> Edge {
        let from_corner = FloatPoint::new(from.0, from.1);
        let to_corner = FloatPoint::new(to.0, to.1);
        Edge {
            from_item,
            from_corner,
            to_item,
            to_corner,
            length_square: to_corner.distance_square(&from_corner),
        }
    }

    #[test]
    fn an_exact_five_way_tie_drops_the_second_edge() {
        // Quirk #147. Two candidate airlines between four *different* items, whose corners
        // coincide pairwise — a via stacked on a pad, or two pads at one location. All five
        // comparison keys agree, so `compareTo` answers 0 and the `TreeSet`/`BTreeSet` keeps only
        // the first. The airline `1 -> 3` is therefore never even a candidate: if items 1 and 3
        // are in different connected groups and this was their only edge, the two groups stay
        // unjoined and `incompleteCount` is one too low.
        let first = edge(0, (0.0, 0.0), 1, (10.0, 0.0));
        let second = edge(2, (0.0, 0.0), 3, (10.0, 0.0));
        assert_eq!(first.cmp(&second), Ordering::Equal);
        // `PartialEq` is `cmp`-derived, as `Ord`'s contract demands, so the two *are* equal as
        // far as the set is concerned even though they name different items.
        assert_eq!(first, second);
        assert_ne!(
            (first.from_item, first.to_item),
            (second.from_item, second.to_item),
        );

        let mut set = BTreeSet::new();
        assert!(set.insert(first.clone()));
        assert!(!set.insert(second));
        assert_eq!(set.len(), 1);
        // The survivor is the one offered first — checked on the fields, since `==` on `Edge`
        // cannot tell them apart.
        let kept = set.iter().next().expect("one element");
        assert_eq!(
            (kept.from_item, kept.to_item),
            (first.from_item, first.to_item)
        );
    }

    #[test]
    fn a_nan_edge_swallows_or_is_swallowed_depending_on_insertion_order() {
        // The other half of quirk #147, stated exactly. `Signum.asInt` is a `> 0 / < 0 / else`
        // ladder, so NaN falls through to 0: an edge whose coordinates make a subtraction NaN
        // compares `Equal` to every other edge, and the relation stops being transitive. What
        // that costs the set depends entirely on **when** the edge is offered.
        let short = edge(0, (0.0, 0.0), 1, (10.0, 0.0));
        let long = edge(2, (0.0, 0.0), 3, (20.0, 0.0));
        let nan = edge(4, (f64::INFINITY, 0.0), 5, (f64::INFINITY, 0.0));
        assert!(nan.length_square.is_nan());
        assert_eq!(nan.cmp(&short), Ordering::Equal);
        assert_eq!(short.cmp(&nan), Ordering::Equal);
        // ... while the two finite edges order perfectly well against each other, which is what
        // makes the relation non-transitive.
        assert_eq!(short.cmp(&long), Ordering::Less);

        // Offered **first**, the NaN edge swallows both finite ones.
        let mut nan_first = BTreeSet::new();
        assert!(nan_first.insert(nan.clone()));
        assert!(!nan_first.insert(short.clone()));
        assert!(!nan_first.insert(long.clone()));
        assert_eq!(nan_first.len(), 1);

        // Offered **last**, it is the one that is dropped, and the set is otherwise intact.
        let mut nan_last = BTreeSet::new();
        assert!(nan_last.insert(short));
        assert!(nan_last.insert(long));
        assert!(!nan_last.insert(nan));
        assert_eq!(nan_last.len(), 2);

        // Note the port claims parity with Java's `TreeSet` only where the comparator is
        // consistent. Here it is not, and a red-black tree may reach a different element to
        // compare against than a B-tree does, so which edge survives a *mixed* sequence is not a
        // parity surface. Unreachable on any real board: a `FloatPoint` in this code path always
        // comes from an `IntPoint`.
    }

    #[test]
    fn edges_are_ordered_by_length_then_by_the_four_coordinates() {
        // The ordinary path: shortest first, and equal lengths broken by `fromCorner.x`,
        // `fromCorner.y`, `toCorner.x`, `toCorner.y` in that order.
        let short = edge(0, (0.0, 0.0), 1, (1.0, 0.0));
        let long = edge(0, (0.0, 0.0), 1, (10.0, 0.0));
        assert_eq!(short.cmp(&long), Ordering::Less);

        let left = edge(0, (0.0, 0.0), 1, (0.0, 5.0));
        let right = edge(0, (1.0, 0.0), 1, (1.0, 5.0));
        assert_eq!(left.length_square, right.length_square);
        assert_eq!(left.cmp(&right), Ordering::Less);

        let lower = edge(0, (1.0, 0.0), 1, (1.0, 5.0));
        let upper = edge(0, (1.0, 1.0), 1, (1.0, 6.0));
        assert_eq!(lower.cmp(&upper), Ordering::Less);

        let to_left = edge(0, (0.0, 0.0), 1, (3.0, 4.0));
        let to_right = edge(0, (0.0, 0.0), 1, (4.0, 3.0));
        assert_eq!(to_left.length_square, to_right.length_square);
        assert_eq!(to_left.cmp(&to_right), Ordering::Less);
    }
}
