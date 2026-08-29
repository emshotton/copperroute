//! Port of `autoroute.expansion.CompleteFreeSpaceExpansionRoom`
//! (CompleteFreeSpaceExpansionRoom.java:19-209) — "an expansion room, whose shape is completely
//! calculated, so that it can be stored in a shape tree".
//!
//! This is the class that discharges Plan 2's `TreeObject::Room` obligation: it
//! `implements SearchTreeObject` (`:19-20`), so rooms and board items share one compensated tree
//! and one ordered result set.

use std::cmp::Ordering;

use fr_board::datastructures::{LeafId, TreeEntry};
use fr_board::searchtree::ShapeSearchTree;
use fr_board::{Board, ItemLookup, RoomId, TreeId, TreeObject};
use fr_geometry::TileShape;

use crate::Arena;
use crate::arena::{DoorId, TargetDoorId};
use crate::autoroute::expansion::{
    ExpandableRef, ExpansionDoor, ExpansionRoomStore, FreeSpaceExpansionRoom, RoomRef,
};

/// Port of `CompleteFreeSpaceExpansionRoom` (CompleteFreeSpaceExpansionRoom.java:19-209).
#[derive(Debug, Clone, PartialEq)]
pub struct CompleteFreeSpaceExpansionRoom {
    /// The inherited `FreeSpaceExpansionRoom` state (`extends FreeSpaceExpansionRoom`, :19).
    pub base: FreeSpaceExpansionRoom,
    /// `private final int id` (:23), "identification number for implementing the Comparable
    /// interface" — `AutorouteEngine.generateRoomIdNo()` (AutorouteEngine.java:672-674), the
    /// engine's `++expansionRoomInstanceCount`. It is the **only** true identity among the five
    /// `getId()`s of `autoroute/expansion`; the other four are hashes.
    id: i32,
    /// This room's own arena index, which is also the key the shared search tree stores it under
    /// ([`TreeObject::Room`]). No Java counterpart — Java stores the object reference.
    ///
    /// It is **not** [`Self::id`]: the engine's counter ticks once per
    /// `SortedRoomNeighbours.calculate` call (SortedRoomNeighbours.java:104), including calls
    /// that build no complete room at all (an obstacle room, `:193`) and calls whose room is
    /// then discarded by the `edgeRemoved` retry (`:111-114`). So ids skip. Both numbers are
    /// still minted in creation order, which is all [`TreeObject`]'s ordering needs.
    room_id: RoomId,
    /// `private ShapeTree.Leaf[] treeEntries` (:26), "the array of entries in the SearchTree.
    /// Consists of just one element" — so one `Option<LeafId>` rather than a `Vec`.
    ///
    /// `None` before the room is inserted, and `None` again after
    /// [`remove_from_tree`](Self::remove_from_tree) — see that method for why the port clears it
    /// where Java does not.
    tree_leaf: Option<LeafId>,
    /// `private Collection<TargetItemExpansionDoor> targetDoors` (:29), a `LinkedList` (:36).
    target_doors: Vec<TargetDoorId>,
    /// `private boolean roomIsNetDependent` (:31).
    room_is_net_dependent: bool,
}

impl CompleteFreeSpaceExpansionRoom {
    /// Port of the constructor (CompleteFreeSpaceExpansionRoom.java:34-38).
    ///
    /// `room_id` is the arena index this room is about to occupy; the store supplies it (see
    /// [`Self::room_id`]).
    pub fn new(
        shape: Option<TileShape>,
        layer: usize,
        id: i32,
        room_id: RoomId,
    ) -> CompleteFreeSpaceExpansionRoom {
        CompleteFreeSpaceExpansionRoom {
            base: FreeSpaceExpansionRoom::new(shape, layer),
            id,
            room_id,
            tree_leaf: None,
            target_doors: Vec::new(),
            room_is_net_dependent: false,
        }
    }

    /// Port of `setSearchTreeEntries(ShapeTree.Leaf[], ShapeTree)`
    /// (CompleteFreeSpaceExpansionRoom.java:40-43). Java ignores the `tree` argument and stores
    /// the array; the port stores the single leaf, because `treeShapeCount` is 1 (`:62-64`).
    ///
    /// `fr-board` inverted this call direction for items (`ShapeTree`'s `not ported: Storable`
    /// note): the caller keeps the entries instead of the tree calling back. Rooms follow the
    /// same shape — [`super::ExpansionRoomStore::insert_complete_room`] inserts and then calls
    /// this.
    pub fn set_search_tree_entries(&mut self, leaf: Option<LeafId>) {
        self.tree_leaf = leaf;
    }

    /// This room's entry in the compensated tree, or `None` if it is not in one.
    pub fn tree_leaf(&self) -> Option<LeafId> {
        self.tree_leaf
    }

    /// This room's arena index, which is the key the shared search tree stores it under.
    pub fn room_id(&self) -> RoomId {
        self.room_id
    }

    /// Port of `compareTo(Object)` (CompleteFreeSpaceExpansionRoom.java:45-53) against another
    /// room: `other.id - this.id`, i.e. **descending** id.
    ///
    /// # Java bug (quirk #157), recorded rather than fixed
    ///
    /// Java's test is `other instanceof FreeSpaceExpansionRoom` but its cast is to
    /// `CompleteFreeSpaceExpansionRoom` (`:48-49`), so an **incomplete** free-space room on the
    /// other side throws `ClassCastException` instead of comparing. It is unreachable today —
    /// incomplete rooms never enter a search tree, and nothing else sorts rooms — which is why
    /// this signature takes the complete room the reachable path always supplies. Widening it to
    /// `RoomRef` would *hide* the bug rather than reproduce it.
    ///
    /// The other arm (`:50-51`, `-1` against a non-room) is [`TreeObject`]'s `Ord`, which is
    /// where the item-versus-room half of the comparator lives.
    pub fn compare_to(&self, other: &CompleteFreeSpaceExpansionRoom) -> Ordering {
        other.id.cmp(&self.id)
    }

    /// Port of `removeFromTree(ShapeTree)` (CompleteFreeSpaceExpansionRoom.java:56-59):
    /// `shapeTree.remove(this.treeEntries)`.
    ///
    /// # Deviation from Java, and why it is the right one (Plan 2 ruling 8)
    ///
    /// Java leaves `treeEntries` pointing at the leaf it just removed, so a **second** call
    /// reaches `MinAreaTree.removeLeaf` with an already-removed leaf — whose `parent` is now
    /// null, so Java takes the `parent == null` branch, decrements `leafCount` again and sets
    /// `root = null`, silently discarding the whole tree (quirk #39). `fr-board`'s
    /// `ShapeTree::remove_leaf` panics there instead, deliberately.
    ///
    /// So the port **takes** the leaf out of the field, which makes the second call a no-op.
    /// That is not a behaviour change on any reachable path: `AutorouteEngine.clear`
    /// (AutorouteEngine.java:307-317) walks `completeExpansionRooms` calling `removeFromTree`,
    /// and `removeCompleteExpansionRoom` (`:377-412`) removes the room from that very list at
    /// `:406` in the same method that calls `removeFromTree` at `:404` — so the list can never
    /// hold a room that has already been removed, and Java's corrupting branch is unreachable
    /// too. [`super::ExpansionRoomStore::remove_complete_room`] proves it by construction.
    pub fn remove_from_tree(&mut self, tree: &mut ShapeSearchTree) {
        tree.remove_room(self.tree_leaf.take());
    }

    /// Port of `treeShapeCount(ShapeTree)` (CompleteFreeSpaceExpansionRoom.java:61-64): always
    /// 1 — a room is one shape on one layer.
    pub fn tree_shape_count(&self) -> usize {
        1
    }

    /// Port of `getTreeShape(ShapeTree, int)` (CompleteFreeSpaceExpansionRoom.java:66-69): the
    /// room's own shape, whatever the index. `None` is Java's null shape.
    pub fn get_tree_shape(&self, _index: usize) -> Option<&TileShape> {
        self.base.get_shape()
    }

    /// Port of `shapeLayer(int)` (CompleteFreeSpaceExpansionRoom.java:71-74): the room's layer,
    /// whatever the index.
    pub fn shape_layer(&self, _index: usize) -> usize {
        self.base.get_layer()
    }

    /// Port of `isObstacle(int)` (CompleteFreeSpaceExpansionRoom.java:76-79): **always true**,
    /// for every net. A room obstructs everything, which is what makes the maze search stop at
    /// a room boundary.
    pub fn is_obstacle(&self, _net_number: i32) -> bool {
        true
    }

    /// Port of `isTraceObstacle(int)` (CompleteFreeSpaceExpansionRoom.java:81-84): always true,
    /// like [`is_obstacle`](Self::is_obstacle).
    pub fn is_trace_obstacle(&self, _net_number: i32) -> bool {
        true
    }

    /// Port of `setNetDependent` (CompleteFreeSpaceExpansionRoom.java:86-89): "will be called
    /// when the room overlaps with net dependent objects."
    pub fn set_net_dependent(&mut self) {
        self.room_is_net_dependent = true;
    }

    /// Port of `isNetDependent` (CompleteFreeSpaceExpansionRoom.java:91-97): a net-dependent
    /// room "cannot be retained when the net number changes in autorouting" —
    /// `AutorouteEngine.initConnection` (:99-110) drops exactly these.
    pub fn is_net_dependent(&self) -> bool {
        self.room_is_net_dependent
    }

    /// Port of `getId` (CompleteFreeSpaceExpansionRoom.java:99-102): the engine counter.
    pub fn get_id(&self) -> i32 {
        self.id
    }

    /// Port of `getTargetDoors` (CompleteFreeSpaceExpansionRoom.java:104-108).
    pub fn get_target_doors(&self) -> &[TargetDoorId] {
        &self.target_doors
    }

    /// Port of `addTargetDoor` (CompleteFreeSpaceExpansionRoom.java:110-113).
    pub fn add_target_door(&mut self, door: TargetDoorId) {
        self.target_doors.push(door);
    }

    /// Port of the `removeDoor(ExpandableObject)` override
    /// (CompleteFreeSpaceExpansionRoom.java:115-124): a `TargetItemExpansionDoor` comes out of
    /// the target-door list, anything else falls through to `super.removeDoor`.
    ///
    /// A drill or a page reaches neither list, which is Java's `super.removeDoor` answering
    /// `false` from `List.remove(Object)` — no `ExpansionDoor` equals them.
    pub fn remove_door(&mut self, door: ExpandableRef) -> bool {
        match door {
            ExpandableRef::TargetDoor(id) => {
                match self.target_doors.iter().position(|d| *d == id) {
                    Some(index) => {
                        self.target_doors.remove(index);
                        true
                    }
                    None => false,
                }
            }
            ExpandableRef::Door(id) => self.base.remove_door(id),
            ExpandableRef::Drill(_) | ExpandableRef::Page(_) => false,
        }
    }

    /// Port of `getObject` (CompleteFreeSpaceExpansionRoom.java:126-129): Java returns `this`,
    /// because the room *is* the `SearchTreeObject`. The port's equivalent is the tree key.
    pub fn get_object(&self) -> TreeObject {
        TreeObject::Room(self.room_id)
    }

    /// Port of the `clearDoors` override (CompleteFreeSpaceExpansionRoom.java:196-201):
    /// `super.clearDoors()` **and** a fresh target-door list.
    pub fn clear_doors(&mut self) {
        self.base.clear_doors();
        self.target_doors = Vec::new();
    }

    /// Port of the `resetDoors` override (CompleteFreeSpaceExpansionRoom.java:203-209):
    /// `super.resetDoors()` and then `reset()` on every target door.
    pub fn reset_doors(
        &self,
        doors: &mut Arena<ExpansionDoor>,
        target_doors: &mut Arena<super::TargetItemExpansionDoor>,
    ) {
        self.base.reset_doors(doors);
        for id in &self.target_doors {
            if let Some(door) = target_doors.get_mut(id.0) {
                door.reset();
            }
        }
    }

    // --- the delegating `super` calls ----------------------------------------------------------

    /// `super.getShape()` (FreeSpaceExpansionRoom.java:64-67).
    pub fn get_shape(&self) -> Option<&TileShape> {
        self.base.get_shape()
    }

    /// `super.setShape(TileShape)` (FreeSpaceExpansionRoom.java:70-72).
    pub fn set_shape(&mut self, shape: Option<TileShape>) {
        self.base.set_shape(shape);
    }

    /// `super.getLayer()` (FreeSpaceExpansionRoom.java:74-77).
    pub fn get_layer(&self) -> usize {
        self.base.get_layer()
    }

    /// `super.addDoor(ExpansionDoor)` (FreeSpaceExpansionRoom.java:34-37).
    pub fn add_door(&mut self, door: DoorId) {
        self.base.add_door(door);
    }

    /// `super.getDoors()` (FreeSpaceExpansionRoom.java:40-43).
    pub fn get_doors(&self) -> &[DoorId] {
        self.base.get_doors()
    }

    /// `super.doorExists(ExpansionRoom)` (FreeSpaceExpansionRoom.java:80-91).
    pub fn door_exists(&self, doors: &Arena<ExpansionDoor>, other: RoomRef) -> bool {
        self.base.door_exists(doors, other)
    }

    /// Port of `validate(AutorouteEngine)` (CompleteFreeSpaceExpansionRoom.java:165-194): "check
    /// if this `FreeSpaceExpansionRoom` is valid."
    ///
    /// A room is valid when nothing in the compensated tree overlaps it 2-dimensionally on its own
    /// layer, other than itself and objects sharing the routed net. `:189`'s
    /// `FRLogger.warn("ExpansionRoom overlap conflict")` is dropped; the `false` it accompanies is
    /// the whole observable result.
    ///
    /// `room_id` is this room's own arena index — Java compares `currentEntry.object == this` by
    /// reference (`:174`), and the tree stores the index in its place.
    ///
    /// The engine is the [`crate::autoroute::maze::AutorouteEngine`] Java's parameter is: it
    /// supplies the net number
    /// (`:170`), the search tree (`:171`) and — where Java would have followed a stored object
    /// reference — the room arena that resolves a [`TreeObject::Room`] leaf.
    pub fn validate(
        &self,
        engine: &crate::autoroute::maze::engine::AutorouteEngine,
        board: &Board,
        room_id: RoomId,
    ) -> bool {
        use crate::autoroute::expansion::sorted_neighbours::{
            object_is_trace_obstacle, object_tree_shape,
        };

        // :167.
        let mut result = true;
        // :169-170.
        let net_numbers = [engine.get_net_number()];
        // :171-172. Java NPEs on a room with no shape; the port's `None` is that dead reference.
        let Some(room_shape) = self.get_shape() else {
            return result;
        };
        let layer = self.get_layer();
        let tree = crate::autoroute::maze::engine::tree_of(board, engine.tree);
        let ctx = board.ctx();
        let overlapping_objects = tree.overlapping_tree_entries_with_rooms(
            room_shape,
            Some(layer),
            &net_numbers,
            &board.items,
            &engine.rooms,
            &ctx,
        );
        // :173-192.
        for current_entry in overlapping_objects {
            // :174-176.
            if current_entry.object == TreeObject::Room(room_id) {
                continue;
            }
            // :177-180.
            if !object_is_trace_obstacle(
                current_entry.object,
                engine.get_net_number(),
                &board.items,
            ) {
                continue;
            }
            // :181-183.
            let object_layer = match current_entry.object {
                TreeObject::Item(id) => board.item_shape_layer(id, current_entry.shape_index),
                TreeObject::Room(id) => engine
                    .rooms
                    .complete_room(id)
                    .map(|room| room.shape_layer(current_entry.shape_index)),
            };
            if object_layer != Some(layer) {
                continue;
            }
            // :184-186.
            let current_shape = object_tree_shape(
                tree,
                current_entry.object,
                current_entry.shape_index,
                &board.items,
                &engine.rooms,
                &ctx,
            );
            // :187-191.
            if room_shape.intersection(&current_shape).dimension() > 1 {
                result = false;
            }
        }
        result
    }
}

/// Port of `CompleteFreeSpaceExpansionRoom.calculateTargetDoors(ShapeTree.TreeEntry, int,
/// ShapeSearchTree)` (CompleteFreeSpaceExpansionRoom.java:131-150): "calculates the doors to the
/// start and destination items of the autoroute algorithm."
///
/// **Not** the same function as the base sorter's static
/// `SortedRoomNeighbours.calculateTargetDoors` (SortedRoomNeighbours.java:158-185), which Task 4
/// ported into `sorted_neighbours.rs`: that one takes the whole deferred own-net list and calls
/// `setNetDependent()` **once, only when the list is non-empty** (`:162-164`); this one takes one
/// tree entry and calls it **unconditionally** (`:134`), so a room with an own-net overlap that
/// yields no target door is still marked net-dependent here and is not there. Its only callers
/// are `Sorted45DegreeRoomNeighbours.java:124` and `SortedOrthogonalRoomNeighbours.java:155`
/// (Task 4's finding 4), i.e. the two angle-restricted sorters, which call it **inside** their
/// neighbour loop rather than after it.
///
/// A free function rather than a method because three of its four steps need the board (the
/// `Connectable` test and `getTraceConnectionShape`) and the store (the target door it allocates),
/// neither of which a `&mut CompleteFreeSpaceExpansionRoom` can reach.
pub fn calculate_target_doors(
    room: RoomId,
    own_net_object: &TreeEntry<TreeObject>,
    net_number: i32,
    board: &mut Board,
    rooms: &mut ExpansionRoomStore,
    tree_id: TreeId,
) {
    // :134. Unconditional, and before every other test.
    if let Some(r) = rooms.complete_room_mut(room) {
        r.set_net_dependent();
    }
    // :136: `ownNetObject.object instanceof Connectable` — a `TreeObject::Room` never is.
    let TreeObject::Item(item_id) = own_net_object.object else {
        return;
    };
    let connection_shape = {
        let ctx = board.ctx();
        let Some(item) = board.items.item(item_id) else {
            return;
        };
        let Some(connectable) = item.as_connectable() else {
            return;
        };
        // :137.
        if !connectable.as_dyn().contains_net(net_number) {
            return;
        }
        // :138-140.
        connectable
            .as_dyn()
            .get_trace_connection_shape(tree_id, own_net_object.shape_index, &ctx)
    };
    // :141.
    let Some(connection_shape) = connection_shape else {
        return;
    };
    let intersects = rooms
        .complete_room(room)
        .and_then(|r| r.get_shape())
        .is_some_and(|shape| shape.intersects(&connection_shape));
    if !intersects {
        return;
    }
    // :142-146.
    let new_target_door = rooms.new_target_door(
        board,
        item_id,
        own_net_object.shape_index,
        Some(RoomRef::Complete(room)),
        tree_id,
    );
    if let Some(r) = rooms.complete_room_mut(room) {
        r.add_target_door(new_target_door);
    }
}

// not ported: `CompleteFreeSpaceExpansionRoom.emitDiagnostic`
// (CompleteFreeSpaceExpansionRoom.java:152-163) — it drives `AutorouteDiagnostic.Sink`, a GUI
// overlay (`global-constraints.md`: no GUI, no observers). No routing decision reads it.

#[cfg(test)]
mod tests {
    use super::*;
    use fr_geometry::IntBox;

    fn boxed(llx: i32, lly: i32, urx: i32, ury: i32) -> TileShape {
        TileShape::Box(IntBox::from_coords(llx, lly, urx, ury))
    }

    fn room(id: i32, room_id: u32) -> CompleteFreeSpaceExpansionRoom {
        CompleteFreeSpaceExpansionRoom::new(Some(boxed(0, 0, 10, 10)), 0, id, RoomId(room_id))
    }

    #[test]
    fn compare_to_is_descending_by_id_like_javas_reversed_subtraction() {
        let low = room(1, 0);
        let high = room(2, 1);
        assert_eq!(low.compare_to(&high), Ordering::Greater);
        assert_eq!(high.compare_to(&low), Ordering::Less);
        assert_eq!(low.compare_to(&room(1, 5)), Ordering::Equal);
    }

    #[test]
    fn the_search_tree_facade_answers_one_shape_on_one_layer_and_obstructs_everything() {
        let r = CompleteFreeSpaceExpansionRoom::new(Some(boxed(1, 2, 3, 4)), 7, 1, RoomId(0));
        assert_eq!(r.tree_shape_count(), 1);
        assert_eq!(r.get_tree_shape(0), Some(&boxed(1, 2, 3, 4)));
        assert_eq!(r.get_tree_shape(99), Some(&boxed(1, 2, 3, 4)));
        assert_eq!(r.shape_layer(0), 7);
        assert_eq!(r.shape_layer(99), 7);
        assert!(r.is_obstacle(-1));
        assert!(r.is_obstacle(42));
        assert!(r.is_trace_obstacle(42));
        assert_eq!(r.get_object(), TreeObject::Room(RoomId(0)));
    }

    #[test]
    fn remove_door_routes_target_doors_to_the_other_list() {
        let mut r = room(1, 0);
        r.add_door(DoorId(1));
        r.add_target_door(TargetDoorId(5));
        assert!(!r.remove_door(ExpandableRef::TargetDoor(TargetDoorId(6))));
        assert!(r.remove_door(ExpandableRef::TargetDoor(TargetDoorId(5))));
        assert!(r.get_target_doors().is_empty());
        assert!(r.remove_door(ExpandableRef::Door(DoorId(1))));
        assert!(!r.remove_door(ExpandableRef::Door(DoorId(1))));
    }

    #[test]
    fn clear_doors_empties_both_lists() {
        let mut r = room(1, 0);
        r.add_door(DoorId(1));
        r.add_target_door(TargetDoorId(2));
        r.clear_doors();
        assert!(r.get_doors().is_empty());
        assert!(r.get_target_doors().is_empty());
    }

    #[test]
    fn net_dependence_is_a_one_way_latch() {
        let mut r = room(1, 0);
        assert!(!r.is_net_dependent());
        r.set_net_dependent();
        assert!(r.is_net_dependent());
    }
}
