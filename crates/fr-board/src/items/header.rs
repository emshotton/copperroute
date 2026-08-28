//! [`ItemHeader`] — the state every board item carries, and the two small types it owns.
//!
//! Java: the field block and the field-only methods of `board/model/items/Item.java:41-67`,
//! plus `board/actions/ItemSearchTreesInfo.java` (folded into [`ItemHeader::tree_entries`])
//! and the `ItemAutorouteInfo` slot (an opaque placeholder here; Plan 6 fills it).
//!
//! Java's `Item` is an abstract base class, so its fields live on every subclass instance. The
//! port has no inheritance: [`crate::items::Item`] is an enum whose nine variant structs each
//! embed one `ItemHeader`, and [`crate::items::Item::header`] is the dispatch that Java gets
//! from `super`.

use std::collections::HashMap;

use fr_geometry::TileShape;

use crate::datastructures::LeafId;
use crate::ids::{ItemId, TreeId};
use crate::rules::{BoardRules, Nets};
use crate::structure::FixedState;

/// Placeholder for `autoroute.ItemAutorouteInfo`, the per-run scratch data `Item.autorouteInfo`
/// (Item.java:67) points at.
///
/// Plan 6 replaces this with the real type. It is deliberately empty and deliberately reached
/// only through [`ItemHeader::autoroute_info`], so that `Board::deep_copy` can drop it wholesale
/// (`global-constraints.md`: `clone()` must not copy per-run autoroute scratch).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AutorouteInfo;

/// One item's per-search-tree data: the leaves it owns in that tree, and the tile shapes that
/// were inserted for it.
///
/// Java: `ItemSearchTreesInfo.SearchTreeInfo` (ItemSearchTreesInfo.java:81-92), a
/// `{tree, entryArr, precalculatedTreeShapes}` triple held in a `LinkedList` and looked up by
/// scanning for `currentTreeInfo.tree == tree`. Here the scan becomes a `HashMap` keyed by
/// [`TreeId`], which is the same lookup by a different index.
///
/// Both fields are `Option` because both of Java's are independently nullable and the
/// difference is observable: `clearPrecalculatedTreeShapes` (ItemSearchTreesInfo.java:74-79)
/// nulls `precalculatedTreeShapes` for every tree while leaving `entryArr` alone, and
/// `Item.getTreeShape` (Item.java:212-226) treats a `null` shape array as "recompute" but an
/// *empty* one as "this item has no shapes" (`treeShapeCount` then answers 0). An empty `Vec`
/// therefore cannot stand in for `None`.
///
/// `leaves` is `Vec<Option<LeafId>>`, not `Vec<LeafId>`: Java's `Leaf[]` is holed on purpose —
/// `ShapeSearchTree.reuseEntriesAfterCutout` writes `null` into the middle of a trace's array
/// (ShapeSearchTree.java:327,344) after moving those leaves elsewhere, and the holed array is
/// later handed straight to `ShapeTree.remove(Leaf[])`. This is the shape
/// [`crate::datastructures::ShapeTree::insert_tiles`] returns and
/// [`crate::datastructures::ShapeTree::remove_opt`] consumes (their `obligation:` markers).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TreeEntries {
    /// Java `SearchTreeInfo.entryArr` (ItemSearchTreesInfo.java:84).
    pub leaves: Option<Vec<Option<LeafId>>>,
    /// Java `SearchTreeInfo.precalculatedTreeShapes` (ItemSearchTreesInfo.java:85).
    ///
    /// The inner `Option` is the *element* `null` Java allows and relies on:
    /// `ShapeSearchTree.calculateTreeShapes(DrillItem)` writes `result[i] = null`
    /// (ShapeSearchTree.java:882, and the same line in both subclass overrides,
    /// ShapeSearchTree45Degree.java:500 / ShapeSearchTree90Degree.java:446) for a layer where the
    /// padstack has no pad and the hole-clearance rule synthesises no obstacle either — a
    /// through-via with unused inner layers, which is the common case. `ShapeTree.insert` then
    /// leaves that index leafless (ShapeTree.java:46-49). The slot must stay, because
    /// `DrillItem.shapeLayer(index)` is `firstLayer() + index` (DrillItem.java:147-154): dropping
    /// it would renumber every later layer.
    pub shapes: Option<Vec<Option<TileShape>>>,
}

/// The fields `Item.java` declares on its abstract base class (Item.java:41-67), and every
/// method of `Item` whose body touches nothing but those fields.
///
/// not ported: `Item.board` (Item.java:45), the back-pointer to the owning board —
/// `global-constraints.md` forbids board back-pointers. Every Java method that reads it becomes
/// either a method taking the piece of the board it actually needed (see
/// [`Self::assign_net_no`], which takes `&Nets`) or a `Board` method in Task 11.
///
/// not ported: `Item.serialVersionUID` / `Serializable` — no serialization in this port.
#[derive(Debug, Clone, PartialEq)]
pub struct ItemHeader {
    /// Java `private final int id` (Item.java:42). Final in Java; private here for the same
    /// reason, read through [`Self::id`].
    id: ItemId,

    /// Java `public int[] netNumbers` (Item.java:53) — public there too, and mutated in place by
    /// `assignNetNo`/`removeFromNet`.
    pub net_nos: Vec<i32>,

    /// Java `private int clearanceClassIndex` (Item.java:56): the row/column of this item in the
    /// clearance matrix.
    clearance_class: usize,

    /// Java `private FixedState fixedState` (Item.java:61).
    fixed_state: FixedState,

    /// Java `protected int componentId` (Item.java:50); 0 means "does not belong to a
    /// component" (Item.java:49, and the `componentId <= 0` test at Item.java:351).
    component_id: i32,

    /// Java `private boolean onTheBoard` (Item.java:64): false while the item is deleted or not
    /// yet inserted.
    on_the_board: bool,

    /// Java `private transient ItemSearchTreesInfo searchTreesInfo` (Item.java:59), flattened
    /// into the map its `LinkedList` scan implements.
    ///
    /// Java's field is `null` until the first `setSearchTreeEntries` call and again after
    /// `clearSearchTreeEntries`; the port uses an empty map, which every accessor treats
    /// identically. The one Java method that distinguishes them is
    /// `Item.setPrecalculatedTreeShapes` (Item.java:1022-1031), which warns and *drops the
    /// shapes* when the info is still `null`. That branch is unreachable: all four callers
    /// (`ShapeSearchTree.java:158,230,304,866`) run after a `getSearchTreeEntries` on the same
    /// item whose result they have already dereferenced, so the info always exists by then.
    pub tree_entries: HashMap<TreeId, TreeEntries>,

    /// Java `private transient ItemAutorouteInfo autorouteInfo` (Item.java:67).
    ///
    /// `Box` keeps `ItemHeader` small once Plan 6 gives [`AutorouteInfo`] a body, and `Option`
    /// is Java's `null`: `clearAutorouteInfo` (Item.java:1052-1054) sets it back to `None`, and
    /// so must `Board::deep_copy` (`global-constraints.md`).
    pub autoroute_info: Option<Box<AutorouteInfo>>,

    /// Java `public double smallestClearance = -1.0` (Item.java:47) — public there too. The
    /// `-1` sentinel means "no clearance measured yet"; `Item.clearanceViolations`
    /// (Item.java:451-453) only lowers it, guarded by `smallestClearance < 0`.
    pub smallest_clearance: f64,
}

impl ItemHeader {
    /// Port of the `Item(int[], int, int, int, FixedState, BasicBoard)` constructor
    /// (Item.java:69-91).
    ///
    /// Java's `netNumbers == null` guard (Item.java:76-77) is an empty `Vec` here; its
    /// `System.arraycopy` (Item.java:80) is the by-value `Vec`.
    ///
    /// `id` is already resolved: Java's `if (id <= 0) this.id = board.communication.idGenerator
    /// .newId()` (Item.java:86-90) needs the board, and [`crate::ids::ItemIdGenerator`] is the
    /// port's generator.
    // `Board`'s typed inserters run Java's `id <= 0` branch (Item.java:86-90) by calling
    // `Board::new_item_id` — the board's `ItemIdGenerator` — before building the header.
    pub fn new(
        id: ItemId,
        net_nos: Vec<i32>,
        clearance_class: usize,
        component_id: i32,
        fixed_state: FixedState,
    ) -> ItemHeader {
        ItemHeader {
            id,
            net_nos,
            clearance_class,
            fixed_state,
            component_id,
            // Java leaves `onTheBoard` at its `boolean` default, i.e. false (Item.java:63-64:
            // "False, if the item is deleted or not inserted into the board").
            on_the_board: false,
            tree_entries: HashMap::new(),
            autoroute_info: None,
            smallest_clearance: -1.0,
        }
    }

    /// Port of `Item.getId` (Item.java:106-109).
    // renamed: Item.getId -> ItemHeader::id / Item::id — `get_` prefixes are not idiomatic Rust
    // and the crate drops them wherever the Java name carries no other information.
    pub fn id(&self) -> ItemId {
        self.id
    }

    /// Port of `Item.netCount` (Item.java:874-876).
    pub fn net_count(&self) -> usize {
        self.net_nos.len()
    }

    /// Port of `Item.getNetNumber(int)` (Item.java:879-881). Java indexes the array directly and
    /// throws `ArrayIndexOutOfBoundsException` out of range; slice indexing panics identically.
    pub fn get_net_number(&self, no: usize) -> i32 {
        self.net_nos[no]
    }

    /// Port of `Item.containsNet` (Item.java:148-159), including its `netNumber <= 0` guard
    /// (Item.java:150-152), which makes every non-normal net number a non-member even if the
    /// array literally holds it.
    pub fn contains_net(&self, net_number: i32) -> bool {
        if net_number <= 0 {
            return false;
        }
        self.net_nos.contains(&net_number)
    }

    /// Port of `Item.sharesNetNo(int[])` (Item.java:179-189).
    ///
    /// Note this is the raw array intersection, *not* [`Self::contains_net`]: a shared net
    /// number of 0 or less does count here, because the nested loop compares elements directly
    /// (Item.java:183). Two net-less items still share nothing, both arrays being empty.
    pub fn shares_net_no(&self, net_nos: &[i32]) -> bool {
        self.net_nos.iter().any(|a| net_nos.contains(a))
    }

    /// Port of `Item.netsEqual(int[])` (Item.java:1189-1200): same length, and every number in
    /// `net_nos` is contained in this item.
    ///
    /// Because the element test is `containsNet` (Item.java:1195), which rejects `<= 0`
    /// (Item.java:150), two items that both carry the single net number 0 are **not**
    /// `netsEqual`. Reproduced.
    pub fn nets_equal(&self, net_nos: &[i32]) -> bool {
        if self.net_nos.len() != net_nos.len() {
            return false;
        }
        net_nos.iter().all(|n| self.contains_net(*n))
    }

    /// Port of `Item.netsNormal` (Item.java:1174-1182).
    pub fn nets_normal(&self) -> bool {
        self.net_nos.iter().all(|n| Nets::is_normal_net_number(*n))
    }

    /// Port of `Item.assignNetNo` (Item.java:957-980): makes this item connectable and assigns
    /// it to `net_number`.
    ///
    /// `nets` replaces Java's `board.rules.nets` (Item.java:965). Java's two `FRLogger.warn`
    /// calls are dropped (`global-constraints.md`); neither changes control flow, and the second
    /// one in particular does **not** stop the assignment.
    //
    // Java bug: the doc comment promises "If netNumber < 0, the net items net number will be
    // removed and the item will no longer be connectable" (Item.java:958-959) and the body has
    // the branch for it (`if (netNumber <= 0) netNumbers = new int[0];`, Item.java:970-971) —
    // but the `Nets.isNormalNetNumber` guard three lines above (Item.java:962-964) already
    // returned for every `netNumber <= 0`, so that branch is dead code and a negative argument
    // is silently a no-op. Reproduced; see docs/java-quirks.md.
    //
    // Java bug: for an item that already has more than one net number, Java warns
    // ("unexpected netCount > 1", Item.java:975-977) and then *overwrites only element 0*
    // (Item.java:978), leaving the other net numbers in place — so the item ends up on
    // `net_number` **plus** whatever it was on before. Reproduced; see docs/java-quirks.md.
    // added in Task 12: `board.itemList.saveForUndo(this)` (Item.java:969) — Plan 2 replaces
    // Java's `UndoableObjects` snapshot stack with `Board::clone` in Task 12, so there is no
    // per-item undo record to take here yet.
    pub fn assign_net_no(&mut self, net_number: i32, nets: &Nets) {
        if !Nets::is_normal_net_number(net_number) {
            return;
        }
        if net_number > nets.max_net_number() {
            return;
        }
        if self.net_nos.is_empty() {
            self.net_nos.push(net_number);
        } else {
            self.net_nos[0] = net_number;
        }
    }

    /// Port of `Item.removeFromNet` (Item.java:888-915): removes `net_number` from the net
    /// number array, returning false (Java: `false`) if it was not there.
    ///
    /// Java's search loop has no `break` (Item.java:894-898), so with a duplicated net number it
    /// removes the **last** occurrence, not the first. Reproduced with `rposition`.
    pub fn remove_from_net(&mut self, net_number: i32) -> bool {
        match self.net_nos.iter().rposition(|n| *n == net_number) {
            Some(index) => {
                self.net_nos.remove(index);
                true
            }
            None => false,
        }
    }

    /// Port of `Item.getFixedState` (Item.java:841-844).
    pub fn get_fixed_state(&self) -> FixedState {
        self.fixed_state
    }

    /// Port of `Item.setFixedState` (Item.java:846-849).
    pub fn set_fixed_state(&mut self, fixed_state: FixedState) {
        self.fixed_state = fixed_state;
    }

    /// Port of `Item.unfix` (Item.java:856-861): unfixes the item unless it is system-fixed.
    pub fn unfix(&mut self) {
        if self.fixed_state != FixedState::SystemFixed {
            self.fixed_state = FixedState::Unfixed;
        }
    }

    /// Port of `Item.isUserFixed` (Item.java:816-819): `fixedState.ordinal() >=
    /// USER_FIXED.ordinal()`, which is the derived `Ord` on [`FixedState`] (declared in Java's
    /// ordinal order).
    pub fn is_user_fixed(&self) -> bool {
        self.fixed_state >= FixedState::UserFixed
    }

    /// Port of `Item.isShoveFixed` (Item.java:834-839), the **base-class** body only.
    ///
    /// `Trace` overrides it (Trace.java:236-254) with an extra net-class test; that override
    /// lives on [`crate::items::Item::is_shove_fixed`], which takes the rules it needs.
    pub fn is_shove_fixed(&self) -> bool {
        self.fixed_state >= FixedState::ShoveFixed
    }

    /// Port of `Item.clearanceClassIndex` (Item.java:917-923).
    // renamed: Item.clearanceClassIndex -> ItemHeader::clearance_class (and
    // Item.setClearanceClassIndex -> set_clearance_class), matching the Task 5 brief and the
    // `clearance_class` naming the rest of the crate already uses.
    pub fn clearance_class(&self) -> usize {
        self.clearance_class
    }

    /// Port of `Item.setClearanceClassIndex` (Item.java:925-935): sets the clearance class,
    /// **ignoring** an index outside the clearance matrix (Java warns and returns).
    ///
    /// `rules` replaces Java's `this.board.rules` (Item.java:930). Java's index test is signed
    /// (`index < 0`); `usize` covers that half.
    pub fn set_clearance_class(&mut self, index: usize, rules: &BoardRules) {
        if index >= rules.clearance_matrix.get_class_count() {
            return;
        }
        self.clearance_class = index;
    }

    /// Port of `Item.getComponentId` (Item.java:883-886); 0 means "no component".
    pub fn get_component_id(&self) -> i32 {
        self.component_id
    }

    /// Port of `Item.assignComponentId` (Item.java:952-955).
    pub fn assign_component_id(&mut self, id: i32) {
        self.component_id = id;
    }

    /// Port of `Item.isOnTheBoard` (Item.java:243-246).
    pub fn is_on_the_board(&self) -> bool {
        self.on_the_board
    }

    /// Port of `Item.setOnTheBoard` (Item.java:248-250).
    pub fn set_on_the_board(&mut self, value: bool) {
        self.on_the_board = value;
    }

    /// Port of `Item.getSearchTreeEntries` (Item.java:1008-1017) by way of
    /// `ItemSearchTreesInfo.getTreeEntries` (ItemSearchTreesInfo.java:25-32): the leaves this
    /// item owns in `tree`, or `None` (Java: `null`) if it has none there.
    pub fn get_tree_entries(&self, tree: TreeId) -> Option<&[Option<LeafId>]> {
        self.tree_entries
            .get(&tree)
            .and_then(|e| e.leaves.as_deref())
    }

    /// Port of `Item.setSearchTreeEntries` (Item.java:996-1006) by way of
    /// `ItemSearchTreesInfo.setTreeEntries` (ItemSearchTreesInfo.java:35-45).
    ///
    /// not ported: Java's `if (this.board == null) return;` (Item.java:999-1001) — there is no
    /// board back-pointer, and an item that is not on a board cannot be in a search tree.
    pub fn set_tree_entries(&mut self, tree: TreeId, leaves: Vec<Option<LeafId>>) {
        self.tree_entries.entry(tree).or_default().leaves = Some(leaves);
    }

    /// Port of `ItemSearchTreesInfo.getPrecalculatedTreeShapes`
    /// (ItemSearchTreesInfo.java:51-58), reached in Java through the private
    /// `Item.getPrecalculatedTreeShapes` (Item.java:228-238).
    ///
    /// This is the *cache read only*. Java's private wrapper fills the cache on a miss by
    /// calling the item's `calculateTreeShapes(searchTree)` (Item.java:234), which is a
    /// `ShapeSearchTree` operation.
    /// The lazy fill at Item.java:233-236 (`Item.getPrecalculatedTreeShapes`) — which calls
    /// `ShapeSearchTree.calculateTreeShapes` and stores the result through
    /// [`Self::set_precalculated_tree_shapes`] below — is [`crate::Board::item_tree_shape`],
    /// because it needs the tree and the board context at once.
    pub fn get_precalculated_tree_shapes(&self, tree: TreeId) -> Option<&[Option<TileShape>]> {
        self.tree_entries
            .get(&tree)
            .and_then(|e| e.shapes.as_deref())
    }

    /// Port of `Item.setPrecalculatedTreeShapes` (Item.java:1019-1031) by way of
    /// `ItemSearchTreesInfo.setPrecalculatedTreeShapes` (ItemSearchTreesInfo.java:61-71).
    ///
    /// not ported: Java's `board == null` and `searchTreesInfo == null` guards
    /// (Item.java:1023-1029) — see the note on [`ItemHeader::tree_entries`] for why the second
    /// one is unreachable.
    pub fn set_precalculated_tree_shapes(&mut self, tree: TreeId, shapes: Vec<Option<TileShape>>) {
        self.tree_entries.entry(tree).or_default().shapes = Some(shapes);
    }

    /// Port of `ItemSearchTreesInfo.clearPrecalculatedTreeShapes`
    /// (ItemSearchTreesInfo.java:74-79): drops the cached shapes for **every** tree while
    /// keeping the leaves.
    pub fn clear_precalculated_tree_shapes(&mut self) {
        for entry in self.tree_entries.values_mut() {
            entry.shapes = None;
        }
    }

    /// Port of `Item.clearSearchTreeEntries` (Item.java:1033-1036), which sets the whole
    /// `searchTreesInfo` to `null` — leaves *and* cached shapes, for every tree.
    pub fn clear_search_tree_entries(&mut self) {
        self.tree_entries.clear();
    }

    /// Port of `Item.getAutorouteInfo` (Item.java:1038-1044): the autoroute scratch data,
    /// created on first use.
    pub fn get_autoroute_info(&mut self) -> &mut AutorouteInfo {
        self.autoroute_info
            .get_or_insert_with(|| Box::new(AutorouteInfo))
    }

    /// Port of `Item.getAutorouteInfoPur` (Item.java:1046-1049): the same slot without creating
    /// it.
    pub fn get_autoroute_info_pur(&self) -> Option<&AutorouteInfo> {
        self.autoroute_info.as_deref()
    }

    /// Port of `Item.clearAutorouteInfo` (Item.java:1051-1054).
    pub fn clear_autoroute_info(&mut self) {
        self.autoroute_info = None;
    }

    /// Port of `Item.clearDerivedData` (Item.java:1056-1065): drops the cached tree shapes and
    /// the autoroute scratch, keeping the tree leaves.
    ///
    /// `Via`, `Pin`, `ObstacleArea`, `ConductionArea`, `ComponentOutline` and `DrillItem` all
    /// override it to clear their own caches first and then call `super`; those overrides land
    /// with their geometry in Tasks 6-8, and each ends here.
    pub fn clear_derived_data(&mut self) {
        self.clear_precalculated_tree_shapes();
        self.autoroute_info = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::NetClassId;
    use crate::rules::ClearanceMatrix;
    use crate::structure::{Layer, LayerStructure};

    fn header(net_nos: Vec<i32>) -> ItemHeader {
        ItemHeader::new(ItemId(7), net_nos, 3, 0, FixedState::Unfixed)
    }

    fn nets_up_to(max: i32) -> Nets {
        let mut nets = Nets::new();
        for i in 1..=max {
            nets.add(format!("net{i}"), 1, false, NetClassId(0));
        }
        nets
    }

    fn layer_structure() -> LayerStructure {
        LayerStructure::new(vec![Layer::new("F.Cu", true), Layer::new("B.Cu", true)])
    }

    fn rules_with_classes(count: usize) -> BoardRules {
        let names: Vec<String> = (0..count).map(|i| format!("class{i}")).collect();
        let matrix = ClearanceMatrix::new(count, &layer_structure(), &names);
        BoardRules::new(layer_structure(), matrix)
    }

    // ---- net operations (Item.assignNetNo / removeFromNet / containsNet / netsEqual) --------

    #[test]
    fn assign_net_no_on_an_item_with_no_nets_gives_it_one() {
        // Item.java:973-974: `if (netNumbers.length == 0) netNumbers = new int[1];`, then
        // `netNumbers[0] = netNumber` (Item.java:978).
        let mut h = header(vec![]);
        h.assign_net_no(2, &nets_up_to(3));
        assert_eq!(h.net_nos, vec![2]);
    }

    #[test]
    fn assign_net_no_on_an_item_with_one_net_replaces_it() {
        // Item.java:978 with `netNumbers.length == 1`: element 0 is overwritten in place.
        let mut h = header(vec![1]);
        h.assign_net_no(3, &nets_up_to(3));
        assert_eq!(h.net_nos, vec![3]);
    }

    #[test]
    fn assign_net_no_on_an_item_with_two_nets_replaces_only_the_first() {
        // Java bug (Item.java:975-978): the `netCount > 1` branch warns and falls straight
        // through to `netNumbers[0] = netNumber`, so net 2 survives.
        let mut h = header(vec![1, 2]);
        h.assign_net_no(3, &nets_up_to(3));
        assert_eq!(h.net_nos, vec![3, 2]);
    }

    #[test]
    fn assign_net_no_ignores_non_normal_net_numbers() {
        // Item.java:962-964 (`Nets.isNormalNetNumber`, Nets.java:32-34: `> 0 && <= 9_999_999`).
        // The `netNumber <= 0` clearing branch at Item.java:970-971 is therefore dead code.
        let nets = nets_up_to(3);
        for bad in [0, -1, i32::MIN, Nets::MAX_LEGAL_NET_NUMBER + 1] {
            let mut h = header(vec![1]);
            h.assign_net_no(bad, &nets);
            assert_eq!(h.net_nos, vec![1], "net number {bad} must be ignored");
        }
    }

    #[test]
    fn assign_net_no_ignores_a_net_number_above_the_boards_maximum() {
        // Item.java:965-968: `if (netNumber > board.rules.nets.maxNetNumber())` warn and return.
        let mut h = header(vec![1]);
        h.assign_net_no(4, &nets_up_to(3));
        assert_eq!(h.net_nos, vec![1]);
    }

    #[test]
    fn remove_from_net_removes_and_reports() {
        // Item.java:892-915.
        let mut h = header(vec![1, 2, 3]);
        assert!(h.remove_from_net(2));
        assert_eq!(h.net_nos, vec![1, 3]);
        assert!(!h.remove_from_net(2));
        assert_eq!(h.net_nos, vec![1, 3]);
    }

    #[test]
    fn remove_from_net_removes_the_last_duplicate() {
        // Item.java:894-898: the search loop has no `break`, so `foundIndex` ends on the last
        // match.
        let mut h = header(vec![5, 7, 5]);
        assert!(h.remove_from_net(5));
        assert_eq!(h.net_nos, vec![5, 7]);
    }

    #[test]
    fn remove_from_net_on_a_single_net_item_empties_the_array() {
        // Item.java:902-912 with `netNumbers.length == 1`: `newNetNoArr` is `new int[0]` and the
        // `foundIndex < newNetNoArr.length` guard skips the second arraycopy.
        let mut h = header(vec![9]);
        assert!(h.remove_from_net(9));
        assert!(h.net_nos.is_empty());
    }

    #[test]
    fn contains_net_rejects_non_positive_numbers_even_when_present() {
        // Item.java:150-152.
        let h = header(vec![0, -3, 4]);
        assert!(!h.contains_net(0));
        assert!(!h.contains_net(-3));
        assert!(h.contains_net(4));
    }

    #[test]
    fn shares_net_no_compares_raw_array_elements() {
        // Item.java:180-189 compares elements directly, without `containsNet`'s `<= 0` guard.
        assert!(header(vec![0]).shares_net_no(&[0]));
        assert!(header(vec![1, 2]).shares_net_no(&[9, 2]));
        assert!(!header(vec![1, 2]).shares_net_no(&[9]));
        assert!(!header(vec![]).shares_net_no(&[]));
    }

    #[test]
    fn nets_equal_needs_equal_length_and_membership() {
        // Item.java:1190-1200.
        assert!(header(vec![1, 2]).nets_equal(&[2, 1]));
        assert!(!header(vec![1, 2]).nets_equal(&[1]));
        assert!(header(vec![]).nets_equal(&[]));
        // The membership test is `containsNet`, which rejects 0 (Item.java:1195, 150-152).
        assert!(!header(vec![0]).nets_equal(&[0]));
    }

    #[test]
    fn nets_normal_checks_every_number() {
        // Item.java:1175-1182.
        assert!(header(vec![1, 2]).nets_normal());
        assert!(header(vec![]).nets_normal());
        assert!(!header(vec![1, 0]).nets_normal());
        assert!(!header(vec![Nets::HIDDEN_NET_NUMBER]).nets_normal());
    }

    #[test]
    fn net_count_and_get_net_number() {
        // Item.java:874-881.
        let h = header(vec![4, 6]);
        assert_eq!(h.net_count(), 2);
        assert_eq!(h.get_net_number(0), 4);
        assert_eq!(h.get_net_number(1), 6);
    }

    #[test]
    #[should_panic]
    fn get_net_number_panics_out_of_range_like_java() {
        // Item.java:880 indexes the array directly: ArrayIndexOutOfBoundsException.
        header(vec![4]).get_net_number(1);
    }

    // ---- FixedState predicates (Item.isUserFixed / isShoveFixed / unfix) --------------------

    #[test]
    fn is_user_fixed_is_true_from_user_fixed_upwards() {
        // Item.java:817-818: `fixedState.ordinal() >= USER_FIXED.ordinal()`.
        let cases = [
            (FixedState::Unfixed, false),
            (FixedState::ShoveFixed, false),
            (FixedState::UserFixed, true),
            (FixedState::SystemFixed, true),
        ];
        for (state, expected) in cases {
            let h = ItemHeader::new(ItemId(1), vec![], 0, 0, state);
            assert_eq!(h.is_user_fixed(), expected, "{state:?}");
        }
    }

    #[test]
    fn is_shove_fixed_is_true_from_shove_fixed_upwards() {
        // Item.java:837-838.
        let cases = [
            (FixedState::Unfixed, false),
            (FixedState::ShoveFixed, true),
            (FixedState::UserFixed, true),
            (FixedState::SystemFixed, true),
        ];
        for (state, expected) in cases {
            let h = ItemHeader::new(ItemId(1), vec![], 0, 0, state);
            assert_eq!(h.is_shove_fixed(), expected, "{state:?}");
        }
    }

    #[test]
    fn unfix_spares_system_fixed_items() {
        // Item.java:857-860.
        for state in [
            FixedState::Unfixed,
            FixedState::ShoveFixed,
            FixedState::UserFixed,
        ] {
            let mut h = ItemHeader::new(ItemId(1), vec![], 0, 0, state);
            h.unfix();
            assert_eq!(h.get_fixed_state(), FixedState::Unfixed);
        }
        let mut h = ItemHeader::new(ItemId(1), vec![], 0, 0, FixedState::SystemFixed);
        h.unfix();
        assert_eq!(h.get_fixed_state(), FixedState::SystemFixed);
    }

    #[test]
    fn set_fixed_state_round_trips() {
        let mut h = header(vec![]);
        h.set_fixed_state(FixedState::UserFixed);
        assert_eq!(h.get_fixed_state(), FixedState::UserFixed);
    }

    // ---- clearance class, component id, on-the-board ----------------------------------------

    #[test]
    fn set_clearance_class_ignores_an_out_of_range_index() {
        // Item.java:930-933: warn and return, leaving the old index in place.
        let rules = rules_with_classes(4);
        let mut h = header(vec![]);
        h.set_clearance_class(2, &rules);
        assert_eq!(h.clearance_class(), 2);
        h.set_clearance_class(4, &rules);
        assert_eq!(h.clearance_class(), 2);
    }

    #[test]
    fn component_id_defaults_and_assigns() {
        // Item.java:883-886, 952-955.
        let mut h = header(vec![]);
        assert_eq!(h.get_component_id(), 0);
        h.assign_component_id(12);
        assert_eq!(h.get_component_id(), 12);
    }

    #[test]
    fn on_the_board_starts_false() {
        // Item.java:63-64: the field's Java default.
        let mut h = header(vec![]);
        assert!(!h.is_on_the_board());
        h.set_on_the_board(true);
        assert!(h.is_on_the_board());
    }

    #[test]
    fn smallest_clearance_starts_at_the_minus_one_sentinel() {
        // Item.java:47.
        assert_eq!(header(vec![]).smallest_clearance, -1.0);
    }

    #[test]
    fn id_is_what_the_constructor_was_given() {
        assert_eq!(header(vec![]).id(), ItemId(7));
    }

    // ---- search-tree bookkeeping (ItemSearchTreesInfo) ---------------------------------------

    #[test]
    fn tree_entries_are_absent_until_set() {
        // ItemSearchTreesInfo.java:25-32 returns null when no SearchTreeInfo matches.
        let h = header(vec![]);
        assert_eq!(h.get_tree_entries(TreeId(0)), None);
        assert_eq!(h.get_precalculated_tree_shapes(TreeId(0)), None);
    }

    #[test]
    fn tree_entries_round_trip_per_tree() {
        // ItemSearchTreesInfo.java:35-45: one SearchTreeInfo per tree.
        let mut h = header(vec![]);
        h.set_tree_entries(TreeId(0), vec![None]);
        h.set_tree_entries(TreeId(1), vec![None, None]);
        assert_eq!(h.get_tree_entries(TreeId(0)).map(<[_]>::len), Some(1));
        assert_eq!(h.get_tree_entries(TreeId(1)).map(<[_]>::len), Some(2));
    }

    #[test]
    fn clear_precalculated_tree_shapes_keeps_the_leaves() {
        // ItemSearchTreesInfo.java:74-79 nulls only `precalculatedTreeShapes`.
        let mut h = header(vec![]);
        h.set_tree_entries(TreeId(0), vec![None]);
        h.set_precalculated_tree_shapes(TreeId(0), vec![]);
        assert_eq!(h.get_precalculated_tree_shapes(TreeId(0)), Some(&[][..]));
        h.clear_precalculated_tree_shapes();
        assert_eq!(h.get_precalculated_tree_shapes(TreeId(0)), None);
        assert_eq!(h.get_tree_entries(TreeId(0)), Some(&[None][..]));
    }

    #[test]
    fn an_empty_shape_list_is_not_the_same_as_no_shape_list() {
        // Item.java:212-226 recomputes for a null array but not for an empty one; the port's
        // `Option` keeps the two apart.
        let mut h = header(vec![]);
        h.set_precalculated_tree_shapes(TreeId(0), vec![]);
        assert_eq!(h.get_precalculated_tree_shapes(TreeId(0)), Some(&[][..]));
        assert_ne!(h.get_precalculated_tree_shapes(TreeId(0)), None);
    }

    #[test]
    fn clear_search_tree_entries_drops_everything() {
        // Item.java:1033-1036 sets `searchTreesInfo = null`.
        let mut h = header(vec![]);
        h.set_tree_entries(TreeId(0), vec![None]);
        h.set_precalculated_tree_shapes(TreeId(0), vec![]);
        h.clear_search_tree_entries();
        assert_eq!(h.get_tree_entries(TreeId(0)), None);
        assert_eq!(h.get_precalculated_tree_shapes(TreeId(0)), None);
    }

    // ---- autoroute scratch -------------------------------------------------------------------

    #[test]
    fn autoroute_info_is_created_on_demand_and_cleared() {
        // Item.java:1038-1054.
        let mut h = header(vec![]);
        assert_eq!(h.get_autoroute_info_pur(), None);
        h.get_autoroute_info();
        assert_eq!(h.get_autoroute_info_pur(), Some(&AutorouteInfo));
        h.clear_autoroute_info();
        assert_eq!(h.get_autoroute_info_pur(), None);
    }

    #[test]
    fn clear_derived_data_drops_shapes_and_autoroute_info_but_not_leaves() {
        // Item.java:1060-1065.
        let mut h = header(vec![]);
        h.set_tree_entries(TreeId(0), vec![None]);
        h.set_precalculated_tree_shapes(TreeId(0), vec![]);
        h.get_autoroute_info();
        h.clear_derived_data();
        assert_eq!(h.get_precalculated_tree_shapes(TreeId(0)), None);
        assert_eq!(h.get_autoroute_info_pur(), None);
        assert_eq!(h.get_tree_entries(TreeId(0)), Some(&[None][..]));
    }
}
