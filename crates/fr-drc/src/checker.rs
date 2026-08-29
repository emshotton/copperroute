//! `drc.DesignRulesChecker`: the headless design-rule checker.
//!
//! Java: `drc/DesignRulesChecker.java`. Task 3 ports the fields, the constructor and
//! `getAllClearanceViolations`; the markers below name the task that owns each of the remaining
//! members, so `scripts/audit-port.sh` records the obligation instead of waiving it.

use std::collections::BTreeSet;

use fr_board::{Board, ClearanceViolation};

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
// added in Task 4: DesignRulesChecker.getAllUnconnectedItems (DesignRulesChecker.java:91-176) — the per-net connected-set grouping.
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
}
