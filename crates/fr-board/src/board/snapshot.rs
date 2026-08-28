//! Snapshots and the board hash — Task 12's replacement for Java's
//! `board/facade/{BoardSnapshotManager,RoutingBoardUndoFacade}.java`.
//!
//! # What Java's `clone`/`deepCopy` actually do
//!
//! `BasicBoard.clone` (BasicBoard.java:158-161) is `BoardSnapshotManager.deserialize(
//! getSnapshotManager().serialize(false))`: a full Java-serialization round trip of the whole
//! object graph. `RoutingBoard.deepCopy` (RoutingBoard.java:1414-1420) delegates to
//! `RoutingBoardUndoFacade.deepCopy`, which does the *same* round trip and then calls
//! `clearAllItemTemporaryAutorouteData()` and `finishAutoroute()` on the result. So
//! `BasicBoard.clone()`'s port is [`Board::deep_copy`] **minus** those last two steps — not the
//! derived [`Clone`] impl below, which does none of the resets the next section describes.
//!
//! `global-constraints.md` forbids `Serializable` in this port, so [`Board`] derives [`Clone`]
//! instead (`board/mod.rs`), but for a different job: a plain, field-for-field in-memory copy —
//! `ShapeTree`'s arena clones by value, so every [`crate::LeafId`] stays valid — that this task
//! substitutes for Java's `generateSnapshot`/`popSnapshot`/`undo`/`redo` (`board/mod.rs`'s
//! `not ported:` notes on those four; a `board.clone()` is what a Plan-7 caller takes before a
//! trial mutation it might have to revert). A derived `clone()` copies *every* field, including
//! the ones `readObject` resets, exactly as they stood; it is not a port of `BasicBoard.clone()`
//! at all.
//!
//! # The transient fields
//!
//! Java's `readObject` (BasicBoard.java:1388-1400, the hook every deserialization — hence every
//! `clone()`/`deepCopy()` — runs through) resets every field either class marks `transient`
//! rather than restoring it, because a `transient` field is never written to the stream to begin
//! with. Besides the search tree and `normalizeSuppressedNetNos` (`Board`'s struct doc; both
//! covered in the next section), that is:
//!
//! - `BasicBoard.revision` (BasicBoard.java:97) — comes back `0`, the same value a freshly
//!   constructed board starts at (Java's plain `int` default).
//! - `RoutingBoard.changedArea` (RoutingBoard.java:67) — comes back `null`.
//! - `RoutingBoard.shoveFailingObstacle` (RoutingBoard.java:72) — comes back `null`.
//! - `RoutingBoard.shoveFailingLayer` (RoutingBoard.java:73) — **Java bug:** its `= -1` field
//!   initializer is a declaration-site initializer, compiled into every constructor
//!   `RoutingBoard` has; deserialization calls none of them (the object is allocated directly and
//!   only `readObject` runs), so this `transient int` comes back at the language default, `0` —
//!   not the `-1` sentinel a freshly-built board starts with, so a cloned/deep-copied board
//!   disagrees with a fresh one about what "no failing layer yet" looks like. Reproduced
//!   (`docs/java-quirks.md`); [`Board::deep_copy`] sets it to `0` to match Java, not `-1`.
//!
//! [`Board::deep_copy`] resets all four explicitly: the port's `clone()` is an ordinary
//! `#[derive(Clone)]` with no `transient` concept, so left alone it would copy every one of them
//! as they stood on `self`. `changed_area` in particular is Plan 7's: `optChangedArea` early-
//! returns on a `null` one, `PolylineTrace.change` (:994-996) and `TraceShover` (:572) both
//! dereference it, and `deep_copy` runs once per autoroute pass/optimizer task — a stale
//! non-`None` `changed_area` surviving a copy would corrupt the next pass's bookkeeping.
//!
//! # The search-tree question
//!
//! Java's `readObject` (BasicBoard.java:1388-1400, the hook every deserialization — hence every
//! `clone`/`deepCopy` — runs through) does **not** restore the search trees from the serialized
//! stream: `searchTreeManager` is `transient` (BasicBoard.java:94), so `readObject` rebuilds it
//! from scratch and **reinserts every item**:
//!
//! ```text
//! searchTreeManager = new SearchTreeManager(this);
//! normalizeSuppressedNetNos = new HashSet<>();
//! ...
//! for (Item currentItem : this.getItems()) {
//!   currentItem.board = this;
//!   searchTreeManager.insert(currentItem);
//! }
//! ```
//!
//! `getItems()` walks `itemList`, a `ConcurrentSkipListMap` keyed by `Item.compareTo`, whose
//! subtraction is reversed (Item.java:98) — quirk #63, documented on this module's parent. So
//! Java's clone rebuilds the tree by reinserting items in **descending id order**, which is not
//! generally the order the *original* board's tree was built in (items are inserted into the
//! live tree as `insertItem` runs, i.e. roughly in creation order). The clone's tree can
//! therefore have a different physical shape (different `ShapeTree.toArray()` sequence) than the
//! board it was cloned from — verified in the JVM: `scripts/differential/java/P2T11.java` mode
//! 11 dumps `toArray()` of the default tree before and after `board.deepCopy()`, on a board built
//! with removes and reinserts along the way, and the two sequences differ.
//!
//! That would matter here only if the port's `#[derive(Clone)]` — which preserves the *original*
//! tree's exact shape and leaf ids, unlike Java's rebuild — were observably different from
//! rebuilding by reinsertion. It is not, for two reasons:
//!
//! 1. `ShapeTree.toArray()` (the only thing that exposes physical tree shape) has exactly one
//!    caller in the whole Java source: `ShapeTree.statistics`, a diagnostic log method this crate
//!    does not port (`global-constraints.md`: `FRLogger` calls are dropped). No routing or query
//!    logic ever reads raw tree order.
//! 2. Every real query — `overlappingObjects`, `pick_items`, the connectivity family — collects
//!    its result into a `TreeSet`/`BTreeSet` ordered by `(object id, shape index)`
//!    (`docs/superpowers/plans/2026-08-28-plan-2-board-model.md`'s global constraints), which
//!    canonicalizes the result independently of the tree's internal shape. Two trees holding the
//!    same set of leaves answer every such query identically no matter how they were built.
//!
//! So cloning the arena is behaviorally equivalent to Java's rebuild-by-reinsertion for every
//! caller this crate has, and is strictly *more* faithful to the pre-clone board than Java's own
//! clone is (Java's clone can itself diverge in shape from the board it copied). Rebuilding the
//! tree by reinserting items in descending id order — the alternative this module's authors
//! considered — would throw that fidelity away for a property (`toArray()` order) nothing
//! observes, so [`Board::deep_copy`] does not do it.
//!
//! # The hash
//!
//! `BasicBoard.getHash` (BasicBoard.java:163-166) delegates to `BoardSnapshotManager.getHash`,
//! which MD5-hashes `serialize(true)` — the Java-serialized bytes of `board.getTraces()`,
//! `board.getVias()` and `board.itemList` (i.e. every item, not just traces and vias) in that
//! order. `autoroute.BoardHistory` (read for the hash *contract* only, per this task's brief)
//! uses `getHash()` purely as a same-board membership test across autorouting passes
//! (`BoardHistory.contains`/`getRank`/`remove`, all just string equality on the hash) — it never
//! inspects the hash's value, only whether two boards produce the same one.
//!
//! [`Board::structural_hash`] is **not** a hash of the same bytes and is not comparable across
//! languages — Java hashes a serialized object graph including every item kind, this hashes a
//! trace's `(id, layer, half_width, polyline corners, net numbers, clearance class, fixed
//! state)` and a via's `(id, layer range, center, padstack, net numbers, clearance class, fixed
//! state)` and nothing else. The two are not a strict "differ iff" pair — Java's `itemList`
//! serialization also covers pins, the outline, obstacle/conduction areas, keepouts and the
//! rules/library, none of which this hashes — but for every field it does cover on a trace or a
//! via, a change there changes both hashes, and on any board this crate can build the items
//! outside that coverage (pins, outlines, obstacle areas, keepouts, the rules and library) are
//! fixed for the duration of one autorouting run — only traces and vias change from pass to pass
//! — so within one run, this hash's equality answers agree with Java's. See `docs/java-quirks.md`
//! for the "hash values are not byte-comparable with Java; only same-board equality semantics
//! are" quirk row.

use std::collections::BTreeSet;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::ids::ItemId;
use crate::items::Item;

use super::Board;

impl Board {
    /// Port of `BasicBoard.clone` (BasicBoard.java:158-161), `RoutingBoard.deepCopy`
    /// (RoutingBoard.java:1414-1420) and `RoutingBoardUndoFacade.deepCopy`
    /// (RoutingBoardUndoFacade.java:45-63) in one method — the module doc explains why the two
    /// Java methods share almost this whole body (the same `readObject`) and differ only in the
    /// last two steps.
    ///
    /// `self.clone()` (the derived `impl Clone for Board`) copies every field as-is, including
    /// the ones Java's `readObject` resets; this method is `clone()` plus those resets — the
    /// search tree is handled by the clone itself (module doc, "The search-tree question"),
    /// `normalize_suppressed_net_nos`/`revision`/`changed_area`/`shove_failing_obstacle`/
    /// `shove_failing_layer` are reset explicitly below (module doc, "The transient fields") —
    /// plus the two things `RoutingBoardUndoFacade.deepCopy` adds on top of the plain
    /// `BasicBoard.clone()` round trip: `clearAllItemTemporaryAutorouteData()`
    /// ([`Self::clear_autoroute_scratch`]) and `finishAutoroute()` ([`Self::finish_autoroute`]).
    pub fn deep_copy(&self) -> Board {
        let mut copy = self.clone();

        // Every field Java's `readObject` (BasicBoard.java:1388-1400) resets rather than
        // restores from the stream, because none of them survive Java serialization
        // (`transient`) — module doc, "The transient fields".
        copy.normalize_suppressed_net_nos.clear();
        copy.revision = 0; // BasicBoard.java:97.
        copy.changed_area = None; // RoutingBoard.java:67.
        copy.shove_failing_obstacle = None; // RoutingBoard.java:72.
        // Java bug: the language default, not the `-1` sentinel — module doc's
        // `shoveFailingLayer` entry has the full argument.
        copy.shove_failing_layer = 0; // RoutingBoard.java:73.

        copy.clear_autoroute_scratch();
        copy.finish_autoroute();
        copy
    }

    /// Port of `RoutingBoard.clearAllItemTemporaryAutorouteData` (RoutingBoard.java:1240-1249):
    /// clears every item's `autorouteInfo`.
    ///
    /// Java walks `itemList` through `startReadObject`/`readObject`, its undo-stack-aware
    /// iterator; the port's items live in a plain `BTreeMap`, so a direct `values_mut` walk reads
    /// (and mutates) every one, which is the same set Java's iterator produces.
    // renamed: RoutingBoard.clearAllItemTemporaryAutorouteData -> Board::clear_autoroute_scratch
    // (this task's brief names it).
    fn clear_autoroute_scratch(&mut self) {
        for item in self.items.values_mut() {
            item.clear_autoroute_info();
        }
    }

    /// Port of `RoutingBoard.finishAutoroute` (RoutingBoard.java:899-905): "clears the auto-route
    /// database in case it was retained" by clearing `autorouteEngine`.
    ///
    /// Empty until Plan 6 gives [`Board`] an `autoroute_engine` field: this port has nothing
    /// `autorouteEngine.clear(); autorouteEngine = null;` could act on yet.
    /// [`Board::deep_copy`] already calls this hook, so Plan 6 only has to fill the body in, not
    /// find every call site that needs it.
    // added in Plan 6: `self.autoroute_engine = None` (RoutingBoard.java:901-904 clears the
    // engine first, if one was retained).
    fn finish_autoroute(&mut self) {}

    /// Port of `BasicBoard.getHash` (BasicBoard.java:163-166) / `BoardSnapshotManager.getHash`
    /// (:57-71): a deterministic hash over every trace's `(id, layer, half_width, polyline
    /// corners, net numbers, clearance class, fixed state)` and every via's `(id, layer range,
    /// center, padstack, net numbers, clearance class, fixed state)`, in ascending item-id order.
    /// The last three fields of each tuple are not in the task brief's list; they are added so
    /// that every `Item` field Java's `itemList` serialization would observe *for these two
    /// kinds* is covered too (module doc, "The hash") — net numbers, clearance class and fixed
    /// state can all change on a live trace/via (`assign_net_no`, `change_clearance_class_index`,
    /// a `FixedState` upgrade) without moving geometry, and each is a real distinguishing fact
    /// about routing state.
    ///
    /// Not a port of Java's *algorithm* (MD5 over a serialized byte stream) — see the module doc
    /// for why a same-board equality test over this narrower, crate-native input is the faithful
    /// reading of the contract `autoroute.BoardHistory` actually uses. `totalized:` in spirit: a
    /// `u64` from [`std::collections::hash_map::DefaultHasher`] (fixed keys, so it is
    /// deterministic within one build) stands in for Java's hex MD5 string.
    ///
    /// Ascending, not [`Board::items_in_board_order`]'s descending: this function has no Java
    /// original to match the iteration order of (Java hashes the whole serialized `itemList`,
    /// order-independent for a hash), so it walks `self.items` in its own natural order.
    pub fn structural_hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        let ctx = self.ctx();
        for item in self.items.values() {
            let id = item.id();
            match item {
                Item::Trace(trace) => {
                    0u8.hash(&mut hasher);
                    id.hash(&mut hasher);
                    trace.get_layer().hash(&mut hasher);
                    trace.get_half_width().hash(&mut hasher);
                    trace.polyline().corners().hash(&mut hasher);
                    item.net_nos().hash(&mut hasher);
                    item.clearance_class().hash(&mut hasher);
                    item.get_fixed_state().hash(&mut hasher);
                }
                Item::Via(via) => {
                    1u8.hash(&mut hasher);
                    id.hash(&mut hasher);
                    via.first_layer(&ctx).hash(&mut hasher);
                    via.last_layer(&ctx).hash(&mut hasher);
                    via.get_center().hash(&mut hasher);
                    via.get_padstack_id().hash(&mut hasher);
                    item.net_nos().hash(&mut hasher);
                    item.clearance_class().hash(&mut hasher);
                    item.get_fixed_state().hash(&mut hasher);
                }
                _ => {}
            }
        }
        hasher.finish()
    }

    /// Port of `BasicBoard.diffTraces` (BasicBoard.java:168-171) / `BoardSnapshotManager.diffTraces`
    /// (:81-95): the number of trace ids that appear in exactly one of the two boards.
    pub fn diff_traces(&self, compare_to: &Board) -> usize {
        let mut trace_ids: BTreeSet<ItemId> = self.get_traces().into_iter().collect();
        let mut result = 0usize;
        for id in compare_to.get_traces() {
            if !trace_ids.remove(&id) {
                result += 1;
            }
        }
        result + trace_ids.len()
    }
}

#[cfg(test)]
mod tests {
    use fr_geometry::{IntBox, Point, Polyline, PolylineShapeRef, TileShape};

    use crate::ids::ItemId;
    use crate::library::{BoardLibrary, Packages, Padstacks};
    use crate::rules::{BoardRules, ClearanceMatrix};
    use crate::structure::{Components, FixedState, Layer, LayerStructure};
    use crate::{Board, Communication};

    fn board() -> Board {
        let layers = LayerStructure::new(vec![Layer::new("Top", true)]);
        let clearance_matrix = ClearanceMatrix::get_default_instance(&layers, 10);
        let mut rules = BoardRules::new(layers.clone(), clearance_matrix);
        rules.create_default_net_class();
        let outline = vec![PolylineShapeRef::Tile(TileShape::Box(IntBox::from_coords(
            0, 0, 1000, 1000,
        )))];
        Board::new(
            outline,
            0,
            IntBox::from_coords(0, 0, 1000, 1000),
            rules,
            BoardLibrary::new(Padstacks::new(layers), Packages::new()),
            Components::new(),
            Communication::default(),
        )
    }

    fn insert_trace(board: &mut Board, net_number: i32, x1: i32, x2: i32) -> ItemId {
        board
            .insert_trace_without_cleaning(
                Polyline::from_points(&[Point::new(x1, 100), Point::new(x2, 100)]),
                0,
                10,
                vec![net_number],
                0,
                FixedState::Unfixed,
            )
            .expect("a straight two-corner trace")
    }

    #[test]
    fn deep_copy_is_independent() {
        let mut board = board();
        insert_trace(&mut board, 1, 100, 500);
        let mut copy = board.deep_copy();

        // A full `PartialEq` would fail here: `deep_copy` deliberately resets the transient
        // bookkeeping the module doc's "The transient fields" section describes (`revision` in
        // particular has already advanced past the two inserts above), so the items and the
        // structural hash are what should agree.
        assert_eq!(copy.items, board.items);
        assert_eq!(copy.structural_hash(), board.structural_hash());

        let trace = board.get_traces()[0];
        copy.remove_item(trace);

        assert_ne!(copy, board);
        assert!(board.get_item(trace).is_some());
        assert!(copy.get_item(trace).is_none());

        // The copy's tree is its own: a query the original still answers with the trace, the
        // copy no longer does (the outline's own tile shape may still overlap the probe, so the
        // assertion checks for the trace specifically rather than emptiness).
        let ctx = board.ctx();
        let shape = board
            .get_item(trace)
            .expect("the trace")
            .get_tile_shape(board.default_tree_id(), 0, &ctx)
            .expect("its tile shape");
        let object = crate::ids::TreeObject::Item(trace);
        assert!(board.overlapping_objects(&shape, Some(0)).contains(&object));
        assert!(!copy.overlapping_objects(&shape, Some(0)).contains(&object));
    }

    #[test]
    fn deep_copy_clears_autoroute_scratch() {
        let mut board = board();
        let trace = insert_trace(&mut board, 1, 100, 500);
        board
            .get_item_mut(trace)
            .expect("the trace")
            .get_autoroute_info();
        assert!(
            board
                .get_item(trace)
                .expect("the trace")
                .get_autoroute_info_pur()
                .is_some()
        );

        let copy = board.deep_copy();
        assert!(
            copy.get_item(trace)
                .expect("the trace")
                .get_autoroute_info_pur()
                .is_none()
        );
        // `clone()` alone does not clear it — only `deep_copy` does.
        assert!(
            board
                .get_item(trace)
                .expect("the trace")
                .get_autoroute_info_pur()
                .is_some()
        );
    }

    #[test]
    fn deep_copy_clears_normalize_suppressed_net_nos() {
        let mut board = board();
        board.normalize_suppressed_net_nos.insert(3);
        let copy = board.deep_copy();
        assert!(copy.normalize_suppressed_net_nos.is_empty());
        assert!(board.normalize_suppressed_net_nos.contains(&3));
    }

    #[test]
    fn deep_copy_resets_transient_bookkeeping() {
        // Module doc, "The transient fields": `revision`, `changed_area`,
        // `shove_failing_obstacle` and `shove_failing_layer` are all Java `transient` fields
        // `readObject` resets rather than restores, so `deep_copy` must reset them explicitly —
        // `self.clone()` alone would carry every one of these over unchanged.
        let mut board = board();
        let trace = insert_trace(&mut board, 1, 100, 500);
        assert_ne!(
            board.revision(),
            0,
            "the two inserts above must have advanced it"
        );

        board.start_marking_changed_area();
        assert!(board.changed_area.is_some());
        board.shove_failing_obstacle = Some(trace);
        board.shove_failing_layer = 3;

        let copy = board.deep_copy();

        assert_eq!(copy.revision(), 0);
        assert!(copy.changed_area.is_none());
        assert!(copy.shove_failing_obstacle.is_none());
        // Java bug (module doc): the reset value is `0`, the `int` default — not the `-1`
        // sentinel a freshly constructed board starts with — because deserialization never runs
        // `shoveFailingLayer`'s `= -1` field initializer.
        assert_eq!(copy.shove_failing_layer, 0);

        // `self` is untouched: `deep_copy` must not mutate the board it is called on.
        assert_ne!(board.revision(), 0);
        assert!(board.changed_area.is_some());
        assert_eq!(board.shove_failing_obstacle, Some(trace));
        assert_eq!(board.shove_failing_layer, 3);
    }

    #[test]
    fn hash_stable_across_clone() {
        let mut board = board();
        insert_trace(&mut board, 1, 100, 500);
        let clone = board.clone();
        assert_eq!(board.structural_hash(), clone.structural_hash());
        let copy = board.deep_copy();
        assert_eq!(board.structural_hash(), copy.structural_hash());
    }

    #[test]
    fn hash_equal_for_equal_boards_and_differs_after_trace_change() {
        let mut board_a = board();
        insert_trace(&mut board_a, 1, 100, 500);
        let mut board_b = board();
        insert_trace(&mut board_b, 1, 100, 500);
        assert_eq!(board_a.structural_hash(), board_b.structural_hash());

        insert_trace(&mut board_b, 2, 600, 900);
        assert_ne!(board_a.structural_hash(), board_b.structural_hash());
    }

    #[test]
    fn diff_traces_counts_ids_present_in_exactly_one_board() {
        let mut board_a = board();
        insert_trace(&mut board_a, 1, 100, 500);
        let trace_b = insert_trace(&mut board_a, 2, 600, 900);

        let mut board_b = board_a.clone();
        assert_eq!(board_a.diff_traces(&board_b), 0);

        board_b.remove_item(trace_b);
        assert_eq!(board_a.diff_traces(&board_b), 1);
        assert_eq!(board_b.diff_traces(&board_a), 1);

        insert_trace(&mut board_b, 3, 200, 300);
        assert_eq!(board_a.diff_traces(&board_b), 2);
    }
}
