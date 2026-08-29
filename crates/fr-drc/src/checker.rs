//! `drc.DesignRulesChecker`: the headless design-rule checker.
//!
//! Java: `drc/DesignRulesChecker.java`. Task 3 ports the fields, the constructor and
//! `getAllClearanceViolations`; the markers below name the task that owns each of the remaining
//! members, so `scripts/audit-port.sh` records the obligation instead of waiving it.

use std::collections::{BTreeMap, BTreeSet};

use fr_board::{Board, ClearanceViolation, Item, ItemId, ItemKind};

use crate::airline::AirLine;
use crate::net_incompletes::NetIncompletes;
use crate::unconnected::{UnconnectedItems, UnconnectedKind};

/// Port of `drc.DesignRulesChecker` (DesignRulesChecker.java:29-821).
///
/// Borrows the board **mutably** (plan-5 ruling 8): every clearance query lowers the queried
/// item's `smallestClearance` (Item.java:451-453) and advances the search tree's entry counter
/// (`ShapeSearchTree.lastGeneratedEntryId`, ShapeSearchTree.java:55), exactly as Java's does.
/// Java hides both behind a `final BasicBoard` field; the port says so in its type.
///
// not ported: DesignRulesChecker.drcSettings (DesignRulesChecker.java:32, :44) — the field is
// stored and never read; 10 of Java's 15 constructions pass `null`, including both
// `BoardStatistics` call sites (`:268`, `:339`) and every `autoroute/pipeline/*`, and
// `includeWarnings`/`includeErrors` filter nothing. Taking it would force `fr-drc -> fr-settings`
// for a dead field (plan-5 ruling 12). Plan 4's quirk #115 — a primitive `boolean` `false` is
// unmergeable, so those two flags cannot be turned off through the settings ladder anyway — is
// the cross-reference.
//
// The remaining members of the Java class, each with the task that owns it:
//
// added in Task 7: DesignRulesChecker.generateReport (DesignRulesChecker.java:210-540) — the KiCad DRC report DTO.
// added in Task 8: DesignRulesChecker.generateReportJson (DesignRulesChecker.java:817-820) — `generateReport` through the Gson-compatible writer.
#[derive(Debug)]
pub struct DesignRulesChecker<'a> {
    /// Java `board` (DesignRulesChecker.java:31).
    board: &'a mut Board,
    /// Java `maxConnections` (DesignRulesChecker.java:33): the number of connections a fully
    /// routed board would have, i.e. the denominator of the quality score
    /// (`BoardStatistics.java:270`, `connections.maximumCount`).
    ///
    /// Java's field is `public` and is written by exactly one place,
    /// [`Self::calculate_all_incompletes`] (`:567-577`); the port keeps it private behind
    /// [`Self::max_connections`] so that "meaningless until the initialiser has run" — the
    /// invariant Java's own readers honour by calling `calculateAllIncompletes()` on the line
    /// before (`BoardStatistics.java:269-270`) — has somewhere to be written down. Unlike the
    /// eight accessors below it is **not** lazy, exactly as Java's bare field read is not.
    max_connections: i32,
    /// Java `netIncompletes` (DesignRulesChecker.java:35), one [`NetIncompletes`] per net number,
    /// indexed by `netNumber - 1` (`:617-621`).
    ///
    /// `None` is Java's `null`: the field stays null until `calculateAllIncompletes()` runs,
    /// which is why every one of its eight readers begins `if (netIncompletes == null)
    /// calculateAllIncompletes();`. Java's array is never *partly* filled — `:618-621` writes
    /// every slot in one loop — so the nullability belongs to the whole `Vec`, not to its
    /// elements.
    net_incompletes: Option<Vec<NetIncompletes>>,
}

impl<'a> DesignRulesChecker<'a> {
    /// Port of `DesignRulesChecker(BasicBoard, DesignRulesCheckerSettings)`
    /// (DesignRulesChecker.java:43-46), minus the settings parameter (see the `not ported:`
    /// marker on the struct).
    pub fn new(board: &'a mut Board) -> Self {
        DesignRulesChecker {
            board,
            // Java leaves `maxConnections` at the `int` default until `calculateAllIncompletes`
            // writes it (DesignRulesChecker.java:620).
            max_connections: 0,
            // Java leaves `netIncompletes` null until the same call (`:617`).
            net_incompletes: None,
        }
    }

    /// Port of `getAllClearanceViolations` (DesignRulesChecker.java:52-81): every clearance
    /// violation on the board, each unordered pair reported once per layer.
    ///
    /// The walk is `board.getItems()` order — **descending id** (BasicBoard.java:603-605,
    /// quirks #44/#63) — so when both items of a pair report the same violation, the one that
    /// survives is the **higher**-id item's, i.e. `first_item > second_item`.
    ///
    /// The result is in insertion order and is *not* sorted by severity:
    /// `ClearanceViolation.aggregateSortedBySeverity` is the GUI path (ported as
    /// [`Board::aggregate_violations_sorted_by_severity`]), and `generateReport` consumes this
    /// list as it comes.
    ///
    // renamed: the dedup key. Java builds a `String` `"<lo>-<hi>-<layer>"` in a
    // `HashSet<String>` (DesignRulesChecker.java:54, :69-72); the port uses the tuple
    // `(lo, hi, layer)` in a `BTreeSet`. Only *membership* is observable — the set is never
    // iterated (`:74-77`) and the output order comes from the item walk — and the string
    // encoding is injective over the same triple, so the two agree on every board.
    pub fn get_all_clearance_violations(&mut self) -> Vec<ClearanceViolation> {
        // DesignRulesChecker.java:54-55.
        let mut all_violations: Vec<ClearanceViolation> = Vec::new();
        let mut seen_violations: BTreeSet<(u32, u32, usize)> = BTreeSet::new();

        // DesignRulesChecker.java:58-59. The ids are collected up front because the body needs
        // `&mut self.board` per item; `items_in_board_order()` is `board.getItems()`' order.
        //
        // Java's `if (item != null)` (`:60`) has no counterpart: the port's item map cannot hold
        // a null value, and an id it yields is an id it has.
        for id in self.board.items_in_board_order() {
            // DesignRulesChecker.java:62.
            for violation in self.board.clearance_violations(id) {
                // DesignRulesChecker.java:66-72: the key is the *sorted* id pair plus the layer,
                // so a pair overlapping on two layers is two entries while several tile-shape
                // overlaps on one layer collapse to one.
                let id1 = violation.first_item.0;
                let id2 = violation.second_item.0;
                let key = if id1 < id2 {
                    (id1, id2, violation.layer)
                } else {
                    (id2, id1, violation.layer)
                };

                // DesignRulesChecker.java:74-77: `contains` then `add` then `add`; `BTreeSet::insert`
                // answers "was new" and does both.
                if seen_violations.insert(key) {
                    all_violations.push(violation);
                }
            }
        }

        // DesignRulesChecker.java:80.
        all_violations
    }

    /// Port of `getAllUnconnectedItems` (DesignRulesChecker.java:91-178): three phases, in
    /// Java's order, which **is** the output order.
    ///
    /// 1. **Per net** (`:95-149`): group the connectable items by their first net number, drop
    ///    the one-item nets, split each remaining net into connected groups, and emit **one**
    ///    entry per net that has two or more — with representatives from groups 0 and 1 and
    ///    `all_items` the two groups concatenated.
    /// 2. **Dangling traces** (`:152-165`): every trace with a contact-free end, minus the ones
    ///    the `first_item`-only dedup happens to catch (quirk #146).
    /// 3. **Dangling vias** (`:168-175`): every via that `is_tail()`, with **no** dedup at all.
    ///
    /// Both dangling walks are `board.getItems()` order — descending id (BasicBoard.java:603-605,
    /// quirk #63).
    ///
    /// Nothing here mutates the board; the receiver is `&mut self` because
    /// [`DesignRulesChecker`] holds the board mutably (plan-5 ruling 8) and because
    /// `generateReport`, which calls this, is `&mut self` for
    /// [`Self::get_all_clearance_violations`]' sake.
    ///
    // Java bug: DesignRulesChecker.getAllUnconnectedItems (DesignRulesChecker.java:160) — the
    // trace phase's dedup compares only `firstItem`, so a dangling trace that is a net entry's
    // `secondItem`, or merely a member of its `allItems`, is reported twice: once inside the net
    // entry and once as its own `track_dangling` entry. The comparison is also `==`, over a list
    // that grows as the walk goes, which makes the phase O(n^2). Both reproduced; quirks row
    // #146.
    //
    // renamed: Java's `unconnectedItems` local (`:92`) keeps its name; `itemsByNet`'s `HashMap`
    // (`:95`) becomes a `BTreeMap` and `connectedSets`' `HashSet`s (`:123`) become `Vec`s in the
    // ascending order `Board::connected_set` already produces — plan-5 ruling 3, quirks row #144.
    pub fn get_all_unconnected_items(&mut self) -> Vec<UnconnectedItems> {
        // DesignRulesChecker.java:92.
        let mut unconnected_items: Vec<UnconnectedItems> = Vec::new();

        // DesignRulesChecker.java:95-101. Java's `item instanceof Connectable && netCount() > 0`
        // is exactly `Item.isConnectable` (Item.java:868-871), which is what the port calls.
        //
        // The lists come out in `board.getItems()` order — descending id — and that order is
        // observable: it decides which connected group is seeded first below, hence which one is
        // group 0 and which representative is `firstItem`.
        let mut items_by_net: BTreeMap<i32, Vec<ItemId>> = BTreeMap::new();
        for item in self.board.get_items() {
            if item.is_connectable() {
                items_by_net
                    .entry(item.get_net_number(0))
                    .or_default()
                    .push(item.id());
            }
        }

        // DesignRulesChecker.java:104-149.
        for (&net_number, net_items) in &items_by_net {
            // DesignRulesChecker.java:108-110.
            if net_items.len() <= 1 {
                continue;
            }

            // DesignRulesChecker.java:113-114.
            let mut connected_sets: Vec<Vec<ItemId>> = Vec::new();
            let mut processed_items: BTreeSet<ItemId> = BTreeSet::new();
            let net_item_set: BTreeSet<ItemId> = net_items.iter().copied().collect();

            // DesignRulesChecker.java:116-132.
            for &item in net_items {
                if processed_items.contains(&item) {
                    continue;
                }
                // DesignRulesChecker.java:122-126: the connected set, intersected with this
                // net's items (Java's `retainAll`) — `getConnectedSet` already filters by net,
                // but an item whose *first* net is a different one still reaches it.
                let set_items: Vec<ItemId> = self
                    .board
                    .connected_set(item, net_number, false)
                    .into_iter()
                    .filter(|id| net_item_set.contains(id))
                    .collect();

                // DesignRulesChecker.java:128-131. The set always holds at least `item` itself,
                // which is in `net_items`, so Java's emptiness guard never fires; it is kept
                // because Java keeps it.
                if !set_items.is_empty() {
                    processed_items.extend(set_items.iter().copied());
                    connected_sets.push(set_items);
                }
            }

            // DesignRulesChecker.java:136-148: at most **one** entry per net, however many
            // groups there are.
            if connected_sets.len() >= 2 {
                // DesignRulesChecker.java:138-139.
                let item1 = self.find_representative_item(&connected_sets[0]);
                let item2 = self.find_representative_item(&connected_sets[1]);

                // DesignRulesChecker.java:141. Neither group is empty, so neither is `None`.
                if let (Some(item1), Some(item2)) = (item1, item2) {
                    // DesignRulesChecker.java:143-146.
                    let mut group_items = connected_sets[0].clone();
                    group_items.extend_from_slice(&connected_sets[1]);
                    unconnected_items.push(UnconnectedItems::new_with_all_items(
                        item1,
                        item2,
                        group_items,
                    ));
                }
            }
        }

        // DesignRulesChecker.java:152-165: dangling traces.
        for id in self.board.items_in_board_order() {
            let Some(item) = self.board.get_item(id) else {
                continue;
            };
            // DesignRulesChecker.java:153.
            if !item.is_trace() {
                continue;
            }
            // DesignRulesChecker.java:154-155, :158.
            let start_is_empty = self.board.trace_start_contacts(id).is_empty();
            let end_is_empty = self.board.trace_end_contacts(id).is_empty();
            if !(start_is_empty || end_is_empty) {
                continue;
            }
            // DesignRulesChecker.java:160 — the `firstItem`-only dedup; see the `Java bug:`
            // marker above.
            if unconnected_items.iter().any(|ui| ui.first_item == id) {
                continue;
            }
            // DesignRulesChecker.java:161.
            unconnected_items.push(UnconnectedItems::new_typed(
                id,
                None,
                UnconnectedKind::TrackDangling,
            ));
        }

        // DesignRulesChecker.java:168-175: dangling vias. No guard here at all, unlike the trace
        // phase's `anyMatch` (`:160`). A via reaches this phase whether or not it already appears
        // in a net entry, so one that *is* a net entry's `firstItem` — which needs its connected
        // group to hold no Pin and no Trace, `:188-198` — is reported a second time whenever it
        // is `isTail`. Pinned by `a_via_that_represents_its_net_is_still_reported_dangling`.
        for id in self.board.items_in_board_order() {
            // DesignRulesChecker.java:169.
            if self.board.get_item(id).map(|item| item.kind()) != Some(ItemKind::Via) {
                continue;
            }
            // DesignRulesChecker.java:171-172: `Via.isTail` — no contacts, or contacts on at
            // most one layer (Via.java:170-187).
            if self.board.is_tail(id) {
                unconnected_items.push(UnconnectedItems::new_typed(
                    id,
                    None,
                    UnconnectedKind::ViaDangling,
                ));
            }
        }

        // DesignRulesChecker.java:177.
        unconnected_items
    }

    /// Port of the private `findRepresentativeItem` (DesignRulesChecker.java:186-201): a `Pin` if
    /// the group has one, else a `Trace`, else any item; `None` only for an empty group
    /// (`:200`).
    ///
    // Java bug: DesignRulesChecker.findRepresentativeItem scans a `HashSet<Item>` over a class
    // that overrides neither `hashCode` nor `equals`, so *which* Pin (or Trace, or item) it
    // returns varies from JVM run to JVM run — and the choice is observable, because the
    // representative's description is interpolated into the report's `description`
    // (DesignRulesChecker.java:411-418) and because quirk #146's dedup keys on it. **Not
    // reproduced**, deliberately: the port takes the **lowest-id** candidate of each class,
    // which is what its ascending `connected_set` order yields for free. Plan-5 ruling 3,
    // quirks row #144.
    fn find_representative_item(&self, connected_set: &[ItemId]) -> Option<ItemId> {
        let of_kind = |kind: ItemKind| {
            connected_set
                .iter()
                .copied()
                .find(|&id| self.board.get_item(id).map(|item| item.kind()) == Some(kind))
        };
        // DesignRulesChecker.java:188-192, :194-198, :200.
        of_kind(ItemKind::Pin)
            .or_else(|| of_kind(ItemKind::Trace))
            .or_else(|| connected_set.first().copied())
    }

    // ------------------------------------------------------------------------------------------
    // The ratsnest: `calculateAllIncompletes` and the eight accessors that lazily call it
    // ------------------------------------------------------------------------------------------

    /// Port of `calculateAllIncompletes` (DesignRulesChecker.java:542-623): builds one
    /// [`NetIncompletes`] per net number and, on the way, [`Self::max_connections`].
    ///
    /// Three steps, in Java's order:
    ///
    /// 1. **The per-net item lists** (`:544-563`). One list per net number `1..=maxNetNumber`,
    ///    filled by walking `board.itemList` — descending id (quirk #63) — and appending each
    ///    connectable item to the list of **every** net it carries (`:556-561`), so a two-net pin
    ///    lands in two lists. The order of a list is not observable: [`NetIncompletes::new`]
    ///    filters it and drops the result into a set (NetIncompletes.java:295).
    /// 2. **`maxConnections`** (`:567-577`): over the **non-empty** lists only, the number of
    ///    `Pin`/`ConductionArea` items minus one, clamped at zero, summed. The comment at
    ///    `:564-567` explains both halves — empty nets used to be counted in the denominator, and
    ///    `Math.max` is what stops a net of nothing but traces from contributing `-1`.
    /// 3. **The array** (`:617-622`), one entry per index, `netNumber = i + 1`.
    ///
    /// Java's `instanceof Connectable` (`:556`) is one test weaker than `Item.isConnectable`
    /// (Item.java:868-871, which the port calls): it does not require `netCount() > 0`. The two
    /// agree here, because the body is a loop over `0..netCount()`.
    ///
    // totalized: DesignRulesChecker.calculateAllIncompletes — Java indexes `netItemLists` with `getNetNumber(i) - 1` unguarded (`:558`), so an item carrying a net number outside `1..=maxNetNumber` throws `ArrayIndexOutOfBoundsException`. The port drops such an item instead. Unreachable from `fr-dsn`, whose reader registers every net it assigns.
    //
    // not ported: the `focusNets = {98, 99}` block (DesignRulesChecker.java:598-615) — hard-coded
    // debug logging over two net numbers that mean nothing on any other board, wrapped around a
    // `validateAndLogPolylineIntegrity()` call that is itself commented out (`:613`), so the loop
    // computes nothing. Quirks row #149. The `totalItems` sum (`:579`) and the `FRLogger.trace`
    // it feeds (`:580-590`) go with it — this crate has no logger.
    pub fn calculate_all_incompletes(&mut self) {
        let board = &*self.board;

        // DesignRulesChecker.java:543-548.
        let max_net_no = board.rules.nets.max_net_number();
        let mut net_item_lists: Vec<Vec<ItemId>> = vec![Vec::new(); max_net_no.max(0) as usize];

        // DesignRulesChecker.java:549-563.
        for id in board.items_in_board_order() {
            let Some(item) = board.get_item(id) else {
                continue;
            };
            if !item.is_connectable() {
                continue;
            }
            for i in 0..item.net_count() {
                let index = item.get_net_number(i) - 1;
                if let Some(list) = usize::try_from(index)
                    .ok()
                    .and_then(|index| net_item_lists.get_mut(index))
                {
                    list.push(id);
                }
            }
        }

        // DesignRulesChecker.java:567-577.
        self.max_connections = net_item_lists
            .iter()
            .filter(|list| !list.is_empty())
            .map(|list| {
                let endpoint_count = list
                    .iter()
                    .filter(|&&id| {
                        matches!(
                            board.get_item(id).map(Item::kind),
                            Some(ItemKind::Pin | ItemKind::ConductionArea),
                        )
                    })
                    .count() as i64;
                i32::try_from(endpoint_count - 1).unwrap_or(0).max(0)
            })
            .sum();

        // DesignRulesChecker.java:617-622.
        self.net_incompletes = Some(
            net_item_lists
                .iter()
                .enumerate()
                .map(|(i, items)| NetIncompletes::new(i as i32 + 1, items, board))
                .collect(),
        );
    }

    /// Port of `recalculateNetIncompletes(int)` (DesignRulesChecker.java:630-643): rebuilds one
    /// net's [`NetIncompletes`] from the board's current items.
    ///
    /// Java **returns** after the lazy initialisation (`:633`) rather than falling through, so on
    /// a fresh checker the named net keeps [`Self::calculate_all_incompletes`]' answer. The
    /// two-argument overload has no such `return`; the asymmetry is transcribed, not smoothed.
    pub fn recalculate_net_incompletes(&mut self, net_number: i32) {
        // DesignRulesChecker.java:631-634.
        if self.net_incompletes.is_none() {
            self.calculate_all_incompletes();
            return;
        }
        // DesignRulesChecker.java:635-638.
        let board = &*self.board;
        let list = self.net_incompletes.as_mut().expect("just initialised");
        if let Some(index) = slot(net_number, list.len()) {
            let item_list = board.get_connectable_items(net_number);
            list[index] = NetIncompletes::new(net_number, &item_list, board);
        }
    }

    /// Port of `recalculateNetIncompletes(int, Collection<Item>)` (DesignRulesChecker.java:647-660):
    /// the same, from a caller-supplied item list.
    ///
    /// Java copies the collection first (`:656-657`) because "it will be changed inside the
    /// constructor of `NetIncompletes`" — it is not, `NetIncompletes` builds its own filtered
    /// list (NetIncompletes.java:80-116) — and the port takes a slice, so the copy has nothing
    /// to protect and is dropped.
    pub fn recalculate_net_incompletes_with(&mut self, net_number: i32, item_list: &[ItemId]) {
        // DesignRulesChecker.java:648-652. No `return` here, unlike the overload above.
        if self.net_incompletes.is_none() {
            self.calculate_all_incompletes();
        }
        // DesignRulesChecker.java:654-659.
        let board = &*self.board;
        let list = self.net_incompletes.as_mut().expect("just initialised");
        if let Some(index) = slot(net_number, list.len()) {
            list[index] = NetIncompletes::new(net_number, item_list, board);
        }
    }

    /// Java `maxConnections` (DesignRulesChecker.java:33), read.
    ///
    /// Zero until [`Self::calculate_all_incompletes`] has run — Java's `int` default, which is
    /// why `BoardStatistics.java:269-270` calls the initialiser on the line before reading it.
    /// None of the lazy accessors below leave it at zero, because each of them runs that
    /// initialiser.
    pub fn max_connections(&self) -> i32 {
        self.max_connections
    }

    /// Port of `getIncompleteCount()` (DesignRulesChecker.java:663-706): the number of airlines
    /// on the whole board, i.e. Σ over the nets of `groups - 1`.
    ///
    /// This is `BoardStatistics`' `connections.incompleteCount` (`BoardStatistics.java:271`) and
    /// **not** the length of the report's `unconnectedItems` array, which is one entry per net
    /// with two or more groups — plan-5 ruling 11 tabulates both families for the three fixtures.
    ///
    // not ported: `getIncompleteCount`'s `detailsBuilder` (DesignRulesChecker.java:670-690) — the
    // per-net log line, which is `FRLogger`-only and which this crate drops with every other
    // trace call. It carries a Java bug worth recording even so: `:686` appends
    // `netIncompletes` — the whole **array** — where every other `append` in the chain adds a
    // scalar, so the line reads `Net #7 (GND): [Lapp/freerouting/drc/NetIncompletes;@1b6d3586
    // incomplete(s);` instead of the count sitting in `count`. Quirks row #150.
    pub fn get_incomplete_count(&mut self) -> usize {
        // DesignRulesChecker.java:664-666.
        self.net_incompletes_mut()
            .iter()
            // DesignRulesChecker.java:672-676. The `count > 0` guard only gates the logging;
            // adding a zero changes nothing.
            .map(NetIncompletes::count)
            .sum()
    }

    /// Port of `getIncompleteCount(int)` (DesignRulesChecker.java:708-734): one net's airline
    /// count, `0` for a net number outside `1..=maxNetNumber` (`:712-714`).
    ///
    // renamed: DesignRulesChecker.getIncompleteCount(int) -> `get_incomplete_count_for_net`, because Rust has no overloading.
    pub fn get_incomplete_count_for_net(&mut self, net_number: i32) -> usize {
        // DesignRulesChecker.java:709-716.
        let list = self.net_incompletes_mut();
        match slot(net_number, list.len()) {
            Some(index) => list[index].count(),
            None => 0,
        }
    }

    /// Port of `getLengthViolationCount` (DesignRulesChecker.java:736-748): how many nets have a
    /// non-zero length violation — the **stored** one, not a recomputed one.
    pub fn get_length_violation_count(&mut self) -> usize {
        // DesignRulesChecker.java:737-745.
        self.net_incompletes_mut()
            .iter()
            .filter(|net_incompletes| net_incompletes.get_length_violation() != 0.0)
            .count()
    }

    /// Port of `getLengthViolation(int)` (DesignRulesChecker.java:750-763): one net's length
    /// violation — positive too long, negative too short — and `0` out of range (`:754-756`).
    pub fn get_length_violation(&mut self, net_number: i32) -> f64 {
        // DesignRulesChecker.java:751-757.
        let list = self.net_incompletes_mut();
        match slot(net_number, list.len()) {
            Some(index) => list[index].get_length_violation(),
            None => 0.0,
        }
    }

    /// Port of `recalculateLengthViolations` (DesignRulesChecker.java:765-778): recomputes every
    /// net's length violation and answers whether any of them changed.
    ///
    /// On a fresh checker it answers `true` — Java's comment is "technically changed from nothing
    /// to something" (`:768`) — without recomputing anything, because
    /// [`Self::calculate_all_incompletes`] has just done it (NetIncompletes.java:225).
    pub fn recalculate_length_violations(&mut self) -> bool {
        // DesignRulesChecker.java:766-769.
        if self.net_incompletes.is_none() {
            self.calculate_all_incompletes();
            return true;
        }
        // DesignRulesChecker.java:770-776. `fold` rather than `any`, because Java's loop has no
        // early exit: every net is recalculated, whatever the ones before it answered.
        let board = &*self.board;
        self.net_incompletes
            .as_mut()
            .expect("just checked")
            .iter_mut()
            .fold(false, |result, net_incompletes| {
                net_incompletes.calc_length_violation(board) || result
            })
    }

    /// Port of `getAllAirlines` (DesignRulesChecker.java:780-798): every net's airlines,
    /// flattened in **ascending net number** and, within a net, in the order Kruskal accepted
    /// them.
    ///
    /// That flattening is deterministic even though the per-net edge choice is not (plan-5
    /// ruling 4): `NetIncompletes.calculateNetItems` seeds off a `HashSet<Item>`, so *which*
    /// airlines a net has varies from JVM run to JVM run while *how many* does not.
    ///
    /// Java sizes the result array from `getIncompleteCount()` (`:784-785`) and then fills it
    /// from the same per-net lists that count summed, so the array is exactly filled; the port
    /// collects instead and cannot disagree with itself.
    pub fn get_all_airlines(&mut self) -> Vec<AirLine> {
        // DesignRulesChecker.java:781-796.
        self.net_incompletes_mut()
            .iter()
            .flat_map(|net_incompletes| net_incompletes.incompletes.iter().cloned())
            .collect()
    }

    /// Port of `getNetIncompletes(int)` (DesignRulesChecker.java:800-815): one net's
    /// [`NetIncompletes`], `None` for Java's `null` out of range (`:804-806`).
    pub fn get_net_incompletes(&mut self, net_number: i32) -> Option<&NetIncompletes> {
        // DesignRulesChecker.java:801-807.
        let list = self.net_incompletes_mut();
        let index = slot(net_number, list.len())?;
        Some(&list[index])
    }

    /// `if (netIncompletes == null) calculateAllIncompletes();` — the two lines that open every
    /// one of the eight accessors above (DesignRulesChecker.java:664-666, `:709-711`, `:737-739`,
    /// `:751-753`, `:766-768`, `:781-783`, `:801-803`) — followed by the dereference Java then
    /// does unguarded.
    fn net_incompletes_mut(&mut self) -> &mut Vec<NetIncompletes> {
        if self.net_incompletes.is_none() {
            self.calculate_all_incompletes();
        }
        self.net_incompletes
            .as_mut()
            .expect("calculate_all_incompletes always assigns")
    }
}

/// Java's `netNumber <= 0 || netNumber > netIncompletes.length` guard
/// (DesignRulesChecker.java:712, `:754`, `:804`) and the `netNumber >= 1 && netNumber <= length`
/// that says the same thing the other way round (`:635`, `:654`), as the array index they gate.
fn slot(net_number: i32, len: usize) -> Option<usize> {
    if net_number <= 0 || net_number as usize > len {
        return None;
    }
    Some(net_number as usize - 1)
}
