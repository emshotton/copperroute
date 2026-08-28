//! Port of `board/searchtree/SearchTreeManager.java`: the set of search trees a board keeps in
//! step with its item list.

use crate::ids::TreeId;
use crate::items::{Item, ItemCtx, PolylineTrace};
use crate::rules::BoardRules;
use crate::structure::AngleRestriction;

use super::ShapeSearchTree;

/// Port of `SearchTreeManager` (`board/searchtree/SearchTreeManager.java`).
///
/// Java keeps one `Collection<ShapeSearchTree> compensatedSearchTrees` whose first element is
/// also held in `defaultTree`. The port splits that into [`Self::default_tree`] plus
/// [`Self::compensated`] — the autoroute trees only — because Rust cannot alias one tree from
/// two owners; every Java loop over `compensatedSearchTrees` becomes a loop over
/// [`Self::trees_mut`], which yields the default tree first, exactly as Java's `LinkedList`
/// does (SearchTreeManager.java:34 adds it first, and every later tree is appended at :161).
///
/// not ported: `SearchTreeManager.board` (SearchTreeManager.java:25) — see the module docs. Every
/// method that walked `board.itemList` takes the items it needs instead, as
/// `items: &mut [&mut Item]`.
///
/// # The order those items must arrive in
///
/// `board.itemList` is an `UndoableObjects`, which stores its objects in a
/// `ConcurrentSkipListMap<Storable, …>` (UndoableObjects.java:21,37) — a *sorted* map, keyed by
/// `Item.compareTo`, whose subtraction is reversed (Item.java:98, quirk #44). So every Java walk
/// of the item list runs in **descending item id**, deterministically. Verified on the JVM: the
/// two-pin/two-trace board of `scripts/differential/java/P2T10.java` yields `5 4 3 2 1` from both
/// `itemList` and `getItems()`, on every run.
///
/// That order is load-bearing, not cosmetic: it is the order items are inserted into a freshly
/// built tree, and `MinAreaTree`'s insertion heuristic makes the tree a function of it. Feeding
/// `getAutorouteTree` ascending ids builds a differently shaped tree with the same contents.
///
// added in Task 11: `Board` stores items in a `BTreeMap<ItemId, Item>` (plan-rulings.md #1),
// which iterates *ascending*, so every call into this type that stands in for a `board.itemList`
// walk must pass `items.values_mut().rev()`.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchTreeManager {
    /// Java `SearchTreeManager.defaultTree` (SearchTreeManager.java:26), the tree interactive
    /// routing uses.
    default_tree: ShapeSearchTree,
    /// Java's `compensatedSearchTrees` (SearchTreeManager.java:24) minus its first element.
    compensated: Vec<ShapeSearchTree>,
    /// Java `SearchTreeManager.clearanceCompensationUsed` (SearchTreeManager.java:27).
    clearance_compensation_used: bool,
    /// The port's replacement for the **static** `ShapeSearchTree.lastGeneratedEntryId`
    /// (ShapeSearchTree.java:55), the tie-break of
    /// [`ShapeSearchTree::overlapping_tree_entries_with_clearance`].
    ///
    /// Deliberate divergence, recorded in `docs/java-quirks.md`: Java's counter is shared by
    /// every board and every tree in the JVM and wraps at `Integer.MAX_VALUE`
    /// (ShapeSearchTree.java:1144-1148), so the tie-break depends on how much routing happened
    /// earlier in the process — and a wrap mid-query inverts it. A per-manager `u64` gives the
    /// same order within any one query and makes the result reproducible across runs.
    next_entry_id: u64,
    /// The id the next tree created by this manager gets. No Java counterpart: Java identifies a
    /// tree by object reference.
    next_tree_id: u32,
}

impl SearchTreeManager {
    /// Port of `SearchTreeManager(BasicBoard)` (SearchTreeManager.java:30-36).
    ///
    /// The default tree is always the **base** `ShapeSearchTree` with 45-degree bounding
    /// directions and no compensation (SearchTreeManager.java:33), whatever the board's angle
    /// restriction is.
    pub fn new() -> SearchTreeManager {
        SearchTreeManager {
            default_tree: ShapeSearchTree::new(TreeId(0), AngleRestriction::None, 0),
            compensated: Vec::new(),
            clearance_compensation_used: false,
            next_entry_id: 0,
            next_tree_id: 1,
        }
    }

    /// Port of `SearchTreeManager.getDefaultTree` (SearchTreeManager.java:65-67).
    pub fn get_default_tree(&self) -> &ShapeSearchTree {
        &self.default_tree
    }

    /// [`Self::get_default_tree`] for the callers that mutate it.
    pub fn get_default_tree_mut(&mut self) -> &mut ShapeSearchTree {
        &mut self.default_tree
    }

    /// The port's stand-in for Java's `for (ShapeSearchTree currentTree : compensatedSearchTrees)`
    /// (e.g. SearchTreeManager.java:40): the default tree first, then the autoroute trees in
    /// creation order.
    pub fn trees_mut(&mut self) -> impl Iterator<Item = &mut ShapeSearchTree> {
        std::iter::once(&mut self.default_tree).chain(self.compensated.iter_mut())
    }

    /// [`Self::trees_mut`] without the mutable borrow.
    pub fn trees(&self) -> impl Iterator<Item = &ShapeSearchTree> {
        std::iter::once(&self.default_tree).chain(self.compensated.iter())
    }

    /// The counter behind [`ShapeSearchTree::overlapping_tree_entries_with_clearance`]'s
    /// tie-break; see [`Self::next_entry_id`].
    pub fn entry_counter_mut(&mut self) -> &mut u64 {
        &mut self.next_entry_id
    }

    /// The number of entry ids handed out so far. No Java counterpart (the Java field is a
    /// private static); exposed so tests can pin that the counter only ever grows.
    pub fn entry_counter(&self) -> u64 {
        self.next_entry_id
    }

    /// Port of `SearchTreeManager.insert(Item)` (SearchTreeManager.java:39-44): insert the
    /// item's tree shapes into every active tree, then mark it as on the board.
    pub fn insert(&mut self, item: &mut Item, ctx: &ItemCtx<'_>) {
        for tree in std::iter::once(&mut self.default_tree).chain(self.compensated.iter_mut()) {
            tree.insert_item(item, ctx);
        }
        item.set_on_the_board(true);
    }

    /// Port of `SearchTreeManager.remove(Item)` (SearchTreeManager.java:47-62).
    ///
    /// Java's `if (!item.isOnTheBoard()) return;` guard is the first line, and
    /// `clearSearchTreeEntries()` runs once after every tree has been visited — it drops the
    /// leaves *and* the cached shapes of every tree at once (Item.java:1033-1036).
    pub fn remove(&mut self, item: &mut Item) {
        if !item.is_on_the_board() {
            return;
        }
        for tree in std::iter::once(&mut self.default_tree).chain(self.compensated.iter_mut()) {
            tree.remove_item(item);
        }
        item.clear_tree_entries();
        item.set_on_the_board(false);
    }

    /// Port of `SearchTreeManager.validateEntries` (SearchTreeManager.java:69-78): true only if
    /// every tree agrees.
    ///
    /// Java deliberately does **not** short-circuit — it visits every tree so that each logs its
    /// own warning — and the port keeps the full walk even though the warnings are dropped.
    pub fn validate_entries(&self, item: &Item) -> bool {
        let mut result = true;
        for tree in self.trees() {
            if !tree.validate_entries(item) {
                result = false;
            }
        }
        result
    }

    /// Port of `SearchTreeManager.isClearanceCompensationUsed` (SearchTreeManager.java:84-86).
    pub fn is_clearance_compensation_used(&self) -> bool {
        self.clearance_compensation_used
    }

    /// Port of `SearchTreeManager.setClearanceCompensationUsed` (SearchTreeManager.java:89-108):
    /// rebuild the default tree, compensated for clearance class 1 or for nothing, and re-insert
    /// every board item into it.
    ///
    /// Java clears the *whole* `compensatedSearchTrees` list (SearchTreeManager.java:96), so the
    /// autoroute trees are discarded too. It also removes every item from the trees before the
    /// list is cleared and re-inserts them afterwards, which is what drops their cached shapes
    /// (`clearSearchTreeEntries`) and forces the new tree to recompute them.
    ///
    /// `items` is Java's `board.itemList` walk (`removeAllBoardItems` / `insertAllBoardItems`,
    /// SearchTreeManager.java:202-231).
    pub fn set_clearance_compensation_used(
        &mut self,
        value: bool,
        items: &mut [&mut Item],
        ctx: &ItemCtx<'_>,
    ) {
        // SearchTreeManager.java:90-92.
        if self.clearance_compensation_used == value {
            return;
        }
        self.clearance_compensation_used = value;
        self.remove_all_board_items(items);
        // SearchTreeManager.java:96.
        self.compensated.clear();
        // SearchTreeManager.java:97-106.
        let compensated_clearance_class = usize::from(value);
        self.default_tree = ShapeSearchTree::new(
            TreeId(self.next_tree_id),
            AngleRestriction::None,
            compensated_clearance_class,
        );
        self.next_tree_id += 1;
        self.insert_all_board_items(items, ctx);
    }

    /// Port of `SearchTreeManager.clearanceValueChanged` (SearchTreeManager.java:111-119): drop
    /// every tree whose compensated class differs from the default tree's, and rebuild the
    /// remaining ones if compensation is in use.
    pub fn clearance_value_changed(&mut self, items: &mut [&mut Item], ctx: &ItemCtx<'_>) {
        let default_class = self.default_tree.compensated_clearance_class();
        self.compensated
            .retain(|tree| tree.compensated_clearance_class() == default_class);
        if self.clearance_compensation_used {
            self.remove_all_board_items(items);
            self.insert_all_board_items(items, ctx);
        }
    }

    /// Port of `SearchTreeManager.clearanceClassRemoved` (SearchTreeManager.java:122-134):
    /// discard the autoroute tree compensated for the removed class.
    ///
    /// Java refuses (with a warning) to remove the default tree's class and returns without
    /// touching anything; the warning is dropped, the refusal is not.
    pub fn clearance_class_removed(&mut self, no: usize) {
        if no == self.default_tree.compensated_clearance_class() {
            return;
        }
        self.compensated
            .retain(|tree| tree.compensated_clearance_class() != no);
    }

    /// Port of `SearchTreeManager.resetCompensatedTrees` (SearchTreeManager.java:178-180):
    /// clears every tree except the default one.
    pub fn reset_compensated_trees(&mut self) {
        self.compensated.clear();
    }

    /// Port of `SearchTreeManager.getAutorouteTree(int)` (SearchTreeManager.java:140-172): the
    /// tree compensated for `clearance_class_index`, created and filled on first use.
    ///
    /// The new tree's class comes from the board's angle restriction
    /// (SearchTreeManager.java:149-160): `ShapeSearchTree90Degree` for a 90-degree board,
    /// `ShapeSearchTree45Degree` for a 45-degree one, and the base class otherwise — which the
    /// port expresses by handing `rules.trace_angle_restriction` straight to
    /// [`ShapeSearchTree::new`], since the three cases line up one-for-one.
    ///
    /// Java searches `compensatedSearchTrees`, which includes the default tree, so a request for
    /// the default tree's own class answers *it* rather than building a second one.
    pub fn get_autoroute_tree(
        &mut self,
        clearance_class_index: usize,
        items: &mut [&mut Item],
        ctx: &ItemCtx<'_>,
    ) -> &mut ShapeSearchTree {
        // SearchTreeManager.java:141-145.
        let existing = if self.default_tree.compensated_clearance_class() == clearance_class_index {
            Some(usize::MAX)
        } else {
            self.compensated
                .iter()
                .position(|tree| tree.compensated_clearance_class() == clearance_class_index)
        };
        if let Some(index) = existing {
            return if index == usize::MAX {
                &mut self.default_tree
            } else {
                &mut self.compensated[index]
            };
        }

        // SearchTreeManager.java:147-161.
        let mut tree = ShapeSearchTree::new(
            TreeId(self.next_tree_id),
            ctx.rules.trace_angle_restriction,
            clearance_class_index,
        );
        self.next_tree_id += 1;
        // SearchTreeManager.java:163-170: fill the new tree from the board's item list.
        for item in items.iter_mut() {
            tree.insert_item(item, ctx);
        }
        self.compensated.push(tree);
        self.compensated.last_mut().expect("just pushed")
    }

    /// Port of `SearchTreeManager.reinsertTreeItems` (SearchTreeManager.java:186-200): remove
    /// every item from every tree, drop the cached shapes, and insert them again.
    ///
    /// The `clearDerivedData()` pass between the two halves is load-bearing and Java says so:
    /// removal clears the tree entries but not the precalculated shapes, so without it a rule
    /// change would never reach the trees.
    // renamed: SearchTreeManager.reinsertTreeItems -> reinsert_tree_shapes (task brief naming).
    pub fn reinsert_tree_shapes(&mut self, items: &mut [&mut Item], ctx: &ItemCtx<'_>) {
        self.remove_all_board_items(items);
        // SearchTreeManager.java:191-198.
        self.insert_all_board_items(items, ctx);
    }

    /// Port of the private `SearchTreeManager.removeAllBoardItems`
    /// (SearchTreeManager.java:202-215).
    fn remove_all_board_items(&mut self, items: &mut [&mut Item]) {
        for item in items.iter_mut() {
            self.remove(item);
        }
    }

    /// Port of the private `SearchTreeManager.insertAllBoardItems`
    /// (SearchTreeManager.java:217-231), including its `clearDerivedData()` per item.
    fn insert_all_board_items(&mut self, items: &mut [&mut Item], ctx: &ItemCtx<'_>) {
        for item in items.iter_mut() {
            item.clear_derived_data();
            self.insert(item, ctx);
        }
    }

    /// Port of `SearchTreeManager.mergeEntriesInFront` (SearchTreeManager.java:237-246).
    pub fn merge_entries_in_front(
        &mut self,
        from_trace: &mut PolylineTrace,
        to_trace: &mut PolylineTrace,
        joined_polyline: &fr_geometry::Polyline,
        from_entry_no: usize,
        to_entry_no: usize,
        rules: &BoardRules,
    ) {
        for tree in std::iter::once(&mut self.default_tree).chain(self.compensated.iter_mut()) {
            tree.merge_entries_in_front(
                from_trace,
                to_trace,
                joined_polyline,
                from_entry_no,
                to_entry_no,
                rules,
            );
        }
    }

    /// Port of `SearchTreeManager.mergeEntriesAtEnd` (SearchTreeManager.java:252-261).
    pub fn merge_entries_at_end(
        &mut self,
        from_trace: &mut PolylineTrace,
        to_trace: &mut PolylineTrace,
        joined_polyline: &fr_geometry::Polyline,
        from_entry_no: usize,
        to_entry_no: usize,
        rules: &BoardRules,
    ) {
        for tree in std::iter::once(&mut self.default_tree).chain(self.compensated.iter_mut()) {
            tree.merge_entries_at_end(
                from_trace,
                to_trace,
                joined_polyline,
                from_entry_no,
                to_entry_no,
                rules,
            );
        }
    }

    /// Port of `SearchTreeManager.changeEntries` (SearchTreeManager.java:267-272).
    pub fn change_entries(
        &mut self,
        trace: &mut PolylineTrace,
        new_polyline: &fr_geometry::Polyline,
        keep_at_start_count: usize,
        keep_at_end_count: usize,
        rules: &BoardRules,
    ) {
        for tree in std::iter::once(&mut self.default_tree).chain(self.compensated.iter_mut()) {
            tree.change_entries(
                trace,
                new_polyline,
                keep_at_start_count,
                keep_at_end_count,
                rules,
            );
        }
    }

    /// Port of the package-private `SearchTreeManager.reuseEntriesAfterCutout`
    /// (SearchTreeManager.java:278-284).
    pub fn reuse_entries_after_cutout(
        &mut self,
        from_trace: &mut PolylineTrace,
        start_piece: &mut PolylineTrace,
        end_piece: &mut PolylineTrace,
        ctx: &ItemCtx<'_>,
    ) {
        for tree in std::iter::once(&mut self.default_tree).chain(self.compensated.iter_mut()) {
            tree.reuse_entries_after_cutout(from_trace, start_piece, end_piece, ctx);
        }
    }
}

impl Default for SearchTreeManager {
    fn default() -> Self {
        Self::new()
    }
}
