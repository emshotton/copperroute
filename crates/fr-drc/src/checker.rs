//! `drc.DesignRulesChecker`: the headless design-rule checker.
//!
//! Java: `drc/DesignRulesChecker.java`. Task 3 ports the fields, the constructor and
//! `getAllClearanceViolations`; the markers below name the task that owns each of the remaining
//! members, so `scripts/audit-port.sh` records the obligation instead of waiving it.

use std::collections::{BTreeMap, BTreeSet};

use fr_board::{Board, ClearanceViolation, ItemId, ItemKind};

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
// added in Task 6: DesignRulesChecker.calculateAllIncompletes (DesignRulesChecker.java:542-623) — builds `netIncompletes` and `maxConnections`.
// added in Task 6: DesignRulesChecker.recalculateNetIncompletes (DesignRulesChecker.java:630-660) — both overloads.
// added in Task 6: DesignRulesChecker.getIncompleteCount (DesignRulesChecker.java:663-706, :708-734) — both overloads.
// added in Task 6: DesignRulesChecker.getLengthViolationCount (DesignRulesChecker.java:736-748).
// added in Task 6: DesignRulesChecker.getLengthViolation (DesignRulesChecker.java:750-763).
// added in Task 6: DesignRulesChecker.recalculateLengthViolations (DesignRulesChecker.java:765-778).
// added in Task 6: DesignRulesChecker.getAllAirlines (DesignRulesChecker.java:780-798).
// added in Task 6: DesignRulesChecker.getNetIncompletes (DesignRulesChecker.java:800-815).
// added in Task 7: DesignRulesChecker.generateReport (DesignRulesChecker.java:210-540) — the KiCad DRC report DTO.
// added in Task 8: DesignRulesChecker.generateReportJson (DesignRulesChecker.java:817-820) — `generateReport` through the Gson-compatible writer.
#[derive(Debug)]
pub struct DesignRulesChecker<'a> {
    /// Java `board` (DesignRulesChecker.java:31).
    board: &'a mut Board,
    /// Java `maxConnections` (DesignRulesChecker.java:33) — a **public** field there, written by
    /// `calculateAllIncompletes` (`:620`) and read by `BoardStatistics`, so it is a public field
    /// here too rather than a getter Java does not have.
    pub max_connections: i32,
    //
    // Java's `private NetIncompletes[] netIncompletes` (DesignRulesChecker.java:35) — `null`
    // until `calculateAllIncompletes()` runs, which is why every reader calls that first — lands
    // with the type itself in Task 5 (`net_incompletes.rs`), as
    // `Option<Vec<NetIncompletes>>`: `None` is Java's `null`. It is left out here rather than
    // stubbed, because a field of a type that does not exist yet does not compile and a
    // placeholder type would have to be deleted again.
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

    /// Port of `getAllUnconnectedItems` (DesignRulesChecker.java:91-176): three phases, in
    /// Java's order, which **is** the output order.
    ///
    /// 1. **Per net** (`:93-149`): group the connectable items by their first net number, drop
    ///    the one-item nets, split each remaining net into connected groups, and emit **one**
    ///    entry per net that has two or more — with representatives from groups 0 and 1 and
    ///    `all_items` the two groups concatenated.
    /// 2. **Dangling traces** (`:152-165`): every trace with a contact-free end, minus the ones
    ///    the `first_item`-only dedup happens to catch (quirk #146).
    /// 3. **Dangling vias** (`:168-174`): every via that `is_tail()`, with **no** dedup at all.
    ///
    /// Both dangling walks are `board.getItems()` order — descending id (BasicBoard.java:603-605,
    /// quirk #63).
    ///
    /// Nothing here mutates the board; the receiver is `&mut self` because
    /// [`DesignRulesChecker`] holds the board mutably (plan-5 ruling 8) and because
    /// `generateReport`, which calls this, is `&mut self` for
    /// [`Self::get_all_clearance_violations`]' sake.
    ///
    // Java bug: the trace phase's dedup compares only `firstItem`
    // (`DesignRulesChecker.java:162`), so a dangling trace that is a net entry's `secondItem`, or
    // merely a member of its `allItems`, is reported twice — once inside the net entry and once
    // as its own `track_dangling` entry. The comparison is also `==`, over a list that grows as
    // the walk goes, which makes the phase O(n^2). Both reproduced; quirks row #146.
    //
    // renamed: Java's `unconnectedItems` local (`:92`) keeps its name; `itemsByNet`'s `HashMap`
    // (`:93`) becomes a `BTreeMap` and `connectedSets`' `HashSet`s (`:118`) become `Vec`s in the
    // ascending order `Board::connected_set` already produces — plan-5 ruling 3, quirks row #144.
    pub fn get_all_unconnected_items(&mut self) -> Vec<UnconnectedItems> {
        // DesignRulesChecker.java:92.
        let mut unconnected_items: Vec<UnconnectedItems> = Vec::new();

        // DesignRulesChecker.java:94-100. Java's `item instanceof Connectable && netCount() > 0`
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

        // DesignRulesChecker.java:103-149.
        for (&net_number, net_items) in &items_by_net {
            // DesignRulesChecker.java:105-107.
            if net_items.len() <= 1 {
                continue;
            }

            // DesignRulesChecker.java:110-111.
            let mut connected_sets: Vec<Vec<ItemId>> = Vec::new();
            let mut processed_items: BTreeSet<ItemId> = BTreeSet::new();
            let net_item_set: BTreeSet<ItemId> = net_items.iter().copied().collect();

            // DesignRulesChecker.java:113-130.
            for &item in net_items {
                if processed_items.contains(&item) {
                    continue;
                }
                // DesignRulesChecker.java:121-124: the connected set, intersected with this
                // net's items (Java's `retainAll`) — `getConnectedSet` already filters by net,
                // but an item whose *first* net is a different one still reaches it.
                let set_items: Vec<ItemId> = self
                    .board
                    .connected_set(item, net_number, false)
                    .into_iter()
                    .filter(|id| net_item_set.contains(id))
                    .collect();

                // DesignRulesChecker.java:126-129. The set always holds at least `item` itself,
                // which is in `net_items`, so Java's emptiness guard never fires; it is kept
                // because Java keeps it.
                if !set_items.is_empty() {
                    processed_items.extend(set_items.iter().copied());
                    connected_sets.push(set_items);
                }
            }

            // DesignRulesChecker.java:133-148: at most **one** entry per net, however many
            // groups there are.
            if connected_sets.len() >= 2 {
                // DesignRulesChecker.java:135-136.
                let item1 = self.find_representative_item(&connected_sets[0]);
                let item2 = self.find_representative_item(&connected_sets[1]);

                // DesignRulesChecker.java:138. Neither group is empty, so neither is `None`.
                if let (Some(item1), Some(item2)) = (item1, item2) {
                    // DesignRulesChecker.java:141-146.
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
            // DesignRulesChecker.java:162 — the `firstItem`-only dedup; see the `Java bug:`
            // marker above.
            if unconnected_items.iter().any(|ui| ui.first_item == id) {
                continue;
            }
            // DesignRulesChecker.java:163.
            unconnected_items.push(UnconnectedItems::new_typed(
                id,
                None,
                UnconnectedKind::TrackDangling,
            ));
        }

        // DesignRulesChecker.java:168-175: dangling vias. No dedup here at all, so a via that is
        // already a net entry's representative is reported twice.
        for id in self.board.items_in_board_order() {
            // DesignRulesChecker.java:169.
            if self.board.get_item(id).map(|item| item.kind()) != Some(ItemKind::Via) {
                continue;
            }
            // DesignRulesChecker.java:171-173: `Via.isTail` — no contacts, or contacts on at
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

    /// Port of the private `findRepresentativeItem` (DesignRulesChecker.java:186-198): a `Pin` if
    /// the group has one, else a `Trace`, else any item; `None` only for an empty group
    /// (`:197`).
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
        // DesignRulesChecker.java:188-192, :193-197, :197.
        of_kind(ItemKind::Pin)
            .or_else(|| of_kind(ItemKind::Trace))
            .or_else(|| connected_set.first().copied())
    }
}
