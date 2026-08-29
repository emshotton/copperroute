//! Port of `autoroute.expansion.ObstacleExpansionRoom` (ObstacleExpansionRoom.java:14-158) —
//! "expansion room used for pushing and ripping obstacles in the autoroute algorithm".
//!
//! Unlike the free-space rooms this one does **not** extend `FreeSpaceExpansionRoom`: it
//! implements `CompleteExpansionRoom` directly (`:14`) and keeps its own door list, so there is
//! no shared base to compose here.

use fr_board::{Board, ItemId, TreeId, TreeObject};
use fr_geometry::TileShape;

use crate::Arena;
use crate::arena::{DoorId, TargetDoorId};
use crate::autoroute::expansion::{ExpandableRef, ExpansionDoor, RoomRef};

/// Port of `ObstacleExpansionRoom` (ObstacleExpansionRoom.java:14-158).
#[derive(Debug, Clone, PartialEq)]
pub struct ObstacleExpansionRoom {
    /// `private final Item item` (:16). Java holds the object; the port holds its id.
    item: ItemId,
    /// `private final int indexInItem` (:17) — the index of this room's shape within the item.
    index_in_item: usize,
    /// `private final TileShape shape` (:18), computed **once** at construction from
    /// `item.getTreeShape(shapeTree, indexInItem)` (`:29`). `None` is Java's null tree shape —
    /// a drill layer with no pad and no synthesised hole obstacle
    /// (`ShapeSearchTree.calculateTreeShapes(DrillItem)`).
    shape: Option<TileShape>,
    /// `private List<ExpansionDoor> doors` (:21), an `ArrayList` (`:30`).
    doors: Vec<DoorId>,
    /// `private boolean doorsCalculated` (:23).
    doors_calculated: bool,
}

impl ObstacleExpansionRoom {
    /// Port of the constructor (ObstacleExpansionRoom.java:26-31).
    ///
    /// Java's `item.getTreeShape(shapeTree, indexInItem)` is [`Board::item_tree_shape`], the
    /// `&mut` variant — **never** `item_tree_shape_ref` (plan-6 ruling 10): the `&self` twin
    /// cannot perform `clearDerivedData()`'s cold-cache recompute, and that drop is
    /// router-observable.
    pub fn new(
        board: &mut Board,
        item: ItemId,
        index_in_item: usize,
        tree: TreeId,
    ) -> ObstacleExpansionRoom {
        ObstacleExpansionRoom {
            item,
            index_in_item,
            shape: board.item_tree_shape(item, tree, index_in_item),
            doors: Vec::new(),
            doors_calculated: false,
        }
    }

    /// Port of `getIndexInItem` (ObstacleExpansionRoom.java:33-36).
    pub fn get_index_in_item(&self) -> usize {
        self.index_in_item
    }

    /// Port of `getItem` (ObstacleExpansionRoom.java:126-129), as the item's id.
    pub fn get_item(&self) -> ItemId {
        self.item
    }

    /// Port of `getLayer` (ObstacleExpansionRoom.java:38-41):
    /// `this.item.shapeLayer(this.indexInItem)`.
    ///
    /// Java recomputes this on every call rather than caching it beside the shape, so the port
    /// takes the board rather than storing a layer. `None` is an item that has left the board.
    pub fn get_layer(&self, board: &Board) -> Option<usize> {
        board.item_shape_layer(self.item, self.index_in_item)
    }

    /// Port of `getShape` (ObstacleExpansionRoom.java:43-46).
    pub fn get_shape(&self) -> Option<&TileShape> {
        self.shape.as_ref()
    }

    /// Port of `getId` (ObstacleExpansionRoom.java:48-51):
    /// `(this.item.getId() << 10) | this.indexInItem`.
    ///
    /// A hash, not an identity, and it **aliases** two ways (quirk #156, hazard D):
    ///
    /// * `|` never carries, so any `indexInItem >= 1024` spills into the item's bits — an item
    ///   with 1024 or more tree shapes gives two of its own rooms the same id, and can collide
    ///   with another item's rooms outright.
    /// * `<< 10` overflows a Java `int` at `itemId >= 2^21`, so ids go negative and then wrap.
    ///
    /// Both are reproduced with `wrapping_shl`, because this id reaches
    /// [`super::ExpansionDoor::get_id`], which is a sort key of `MazeListElement` (Task 8).
    pub fn get_id(&self) -> i32 {
        ObstacleExpansionRoom::id(self.item, self.index_in_item)
    }

    /// [`get_id`](Self::get_id)'s arithmetic, as a free function over the two inputs, so the
    /// aliasing can be pinned without building a board.
    pub fn id(item: ItemId, index_in_item: usize) -> i32 {
        // Java's `int` cast of both operands, then `<<` and `|`.
        (item.0 as i32).wrapping_shl(10) | (index_in_item as i32)
    }

    /// Port of `doorExists` (ObstacleExpansionRoom.java:53-64): "checks if this room already has
    /// a 1-dimensional door to other" — the javadoc says 1-dimensional, the body tests every
    /// door.
    ///
    /// Java's `doors != null` guard (`:56`) cannot happen here.
    pub fn door_exists(&self, doors: &Arena<ExpansionDoor>, other: RoomRef) -> bool {
        self.doors.iter().any(|door| {
            doors
                .get(door.0)
                .is_some_and(|d| d.first_room == other || d.second_room == other)
        })
    }

    /// Port of `addDoor` (ObstacleExpansionRoom.java:66-70).
    pub fn add_door(&mut self, door: DoorId) {
        self.doors.push(door);
    }

    /// Port of `getDoors` (ObstacleExpansionRoom.java:102-106).
    pub fn get_doors(&self) -> &[DoorId] {
        &self.doors
    }

    /// Port of `clearDoors` (ObstacleExpansionRoom.java:108-112).
    pub fn clear_doors(&mut self) {
        self.doors = Vec::new();
    }

    /// Port of `resetDoors` (ObstacleExpansionRoom.java:114-119).
    pub fn reset_doors(&self, doors: &mut Arena<ExpansionDoor>) {
        for door in &self.doors {
            if let Some(door) = doors.get_mut(door.0) {
                door.reset();
            }
        }
    }

    /// Port of `removeDoor` (ObstacleExpansionRoom.java:136-139): `List.remove(Object)`.
    ///
    /// An obstacle room has no target-door list, so a `TargetItemExpansionDoor` — or a drill or
    /// a page — simply matches nothing, which is what Java's `List.remove` answers.
    pub fn remove_door(&mut self, door: ExpandableRef) -> bool {
        let ExpandableRef::Door(id) = door else {
            return false;
        };
        match self.doors.iter().position(|d| *d == id) {
            Some(index) => {
                self.doors.remove(index);
                true
            }
            None => false,
        }
    }

    /// Port of `getTargetDoors` (ObstacleExpansionRoom.java:121-124): always empty. Java
    /// allocates a fresh `ArrayList` each call.
    pub fn get_target_doors(&self) -> &[TargetDoorId] {
        &[]
    }

    /// Port of `getObject` (ObstacleExpansionRoom.java:131-134): `this.item` — the obstacle
    /// room's `SearchTreeObject` is the **item**, not the room, so an obstacle room is never in
    /// the tree under its own key.
    pub fn get_object(&self) -> TreeObject {
        TreeObject::Item(self.item)
    }

    /// Port of `allDoorsCalculated` (ObstacleExpansionRoom.java:141-144).
    pub fn all_doors_calculated(&self) -> bool {
        self.doors_calculated
    }

    /// Port of `setDoorsCalculated` (ObstacleExpansionRoom.java:146-148).
    pub fn set_doors_calculated(&mut self, value: bool) {
        self.doors_calculated = value;
    }
}

// renamed: `ObstacleExpansionRoom.createOverlapDoor` (ObstacleExpansionRoom.java:72-100) is
// `crate::autoroute::expansion::sorted_neighbours::create_overlap_door`, a free function rather
// than a method: three of its five guards read the **board** (`Item.isRoutable`,
// `Item.sharesNet`, `instanceof PolylineTrace`) and the door it builds has to go into the
// store's arena, neither of which a `&mut self` on this struct can reach. Its only caller is
// `SortedRoomNeighbours.calculateNeighbours` (`SortedRoomNeighbours.java:240` — the 2-dimensional
// overlap branch; the marker this replaces named `calculateNewIncompleteRooms`, which is wrong).
//
// not ported: `ObstacleExpansionRoom.emitDiagnostic` (ObstacleExpansionRoom.java:150-158) — an
// `AutorouteDiagnostic.Sink`, i.e. a GUI overlay (`global-constraints.md`).

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_id_is_an_or_not_a_sum_so_a_wide_index_aliases() {
        // quirk #156. `1 << 10 | 1024` is 1024, which is `1 << 10 | 0`.
        assert_eq!(
            ObstacleExpansionRoom::id(ItemId(1), 1024),
            ObstacleExpansionRoom::id(ItemId(1), 0)
        );
        // `1 << 10 | 2048` is 3072, which is `3 << 10`.
        assert_eq!(
            ObstacleExpansionRoom::id(ItemId(1), 2048),
            ObstacleExpansionRoom::id(ItemId(3), 0)
        );
        // Within the 10 bits the encoding is injective, which is why the bug is invisible on
        // every real board.
        assert_ne!(
            ObstacleExpansionRoom::id(ItemId(1), 1023),
            ObstacleExpansionRoom::id(ItemId(1), 1022)
        );
        assert_ne!(
            ObstacleExpansionRoom::id(ItemId(1), 0),
            ObstacleExpansionRoom::id(ItemId(2), 0)
        );
    }

    #[test]
    fn the_shift_overflows_a_java_int_at_two_to_the_twenty_first() {
        assert!(ObstacleExpansionRoom::id(ItemId(1 << 21), 0) < 0);
        assert_eq!(
            ObstacleExpansionRoom::id(ItemId(1 << 22), 0),
            ObstacleExpansionRoom::id(ItemId(0), 0)
        );
    }
}
