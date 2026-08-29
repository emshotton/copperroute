//! Port of `autoroute.expansion.TargetItemExpansionDoor` (TargetItemExpansionDoor.java:11-74) —
//! "an expansion door leading to a start or destination item of the autoroute algorithm".

use fr_board::{Board, ItemId, TreeId};
use fr_geometry::{Simplex, TileShape};

use crate::autoroute::expansion::RoomRef;
use crate::autoroute::item_info;
use crate::autoroute::maze::MazeSearchElement;

/// Port of `TargetItemExpansionDoor` (TargetItemExpansionDoor.java:11-74).
///
/// # Java-vs-brief
///
/// The brief names the second field `tree_shape_index`; Java's is `public final int treeEntryNo`
/// (`:14`), and it is the *tree entry* index, i.e. `ShapeTree.TreeEntry.shapeIndexInObject`.
/// The Java name is kept. The brief also types the room as a bare `RoomRef`; Java's is
/// nullable (`:25-27` and `:73` both branch on `room == null`), so it is an `Option` here.
#[derive(Debug, Clone, PartialEq)]
pub struct TargetItemExpansionDoor {
    /// `public final Item item` (:13), as its id.
    pub item: ItemId,
    /// `public final int treeEntryNo` (:14).
    pub tree_entry_no: usize,
    /// `public final CompleteExpansionRoom room` (:15) — nullable, see the type doc.
    pub room: Option<RoomRef>,
    /// `private final TileShape shape` (:16), computed once in the constructor (`:25-30`):
    /// `Simplex.EMPTY` for a null room, else the item's tree shape intersected with the room's.
    shape: TileShape,
    /// `private final MazeSearchElement mazeSearchInfo` (:17) — exactly one, because
    /// `mazeSearchElementCount()` is 1 (`:60-63`).
    maze_search_info: MazeSearchElement,
}

impl TargetItemExpansionDoor {
    /// Port of the constructor (TargetItemExpansionDoor.java:20-32).
    ///
    /// `room_shape` is the room's `getShape()`, which the caller resolves out of the arena.
    /// Java would NPE on a null room shape at `:29`; the port's caller answers `None` for the
    /// room instead — which is the same `Simplex.EMPTY` branch (`:26`) that a null *room* takes,
    /// and the only difference is that Java reaches it by throwing.
    ///
    /// `item.getTreeShape(searchTree, treeEntryNo)` is [`Board::item_tree_shape`], the `&mut`
    /// variant (plan-6 ruling 10). Its `None` — a tree entry with no shape — takes the empty
    /// branch too, where Java's `itemShape.intersection(...)` would throw.
    pub fn new(
        board: &mut Board,
        item: ItemId,
        tree_entry_no: usize,
        room: Option<RoomRef>,
        room_shape: Option<&TileShape>,
        tree: TreeId,
    ) -> TargetItemExpansionDoor {
        // TargetItemExpansionDoor.java:25-30.
        let shape = match (room, room_shape) {
            (Some(_), Some(room_shape)) => match board.item_tree_shape(item, tree, tree_entry_no) {
                Some(item_shape) => item_shape.intersection(room_shape),
                // totalized: Java's `itemShape.intersection(...)` NPEs on a null tree shape;
                // the port answers the same empty shape the null-room branch (:26) produces.
                None => TileShape::Simplex(Simplex::EMPTY),
            },
            // TargetItemExpansionDoor.java:25-27.
            _ => TileShape::Simplex(Simplex::EMPTY),
        };
        TargetItemExpansionDoor {
            item,
            tree_entry_no,
            room,
            shape,
            maze_search_info: MazeSearchElement::new(),
        }
    }

    /// Port of `getShape` (TargetItemExpansionDoor.java:34-37) — the precomputed intersection,
    /// unlike [`super::ExpansionDoor::get_shape`], which recomputes.
    pub fn get_shape(&self) -> &TileShape {
        &self.shape
    }

    /// Port of `getDimension` (TargetItemExpansionDoor.java:39-42): always 2.
    pub fn get_dimension(&self) -> i32 {
        2
    }

    /// Port of `isDestinationDoor` (TargetItemExpansionDoor.java:44-48): "returns true if this
    /// door leads to a destination item rather than a start item" — `!itemInfo.isStartInfo()`.
    ///
    /// Java reaches the info through `item.getAutorouteInfo()` (`:46`), which **creates it on
    /// demand**, so this is one of the twelve creating call sites (see
    /// [`crate::autoroute::item_info`]) and the board is taken mutably for that reason alone.
    pub fn is_destination_door(&self, board: &mut Board) -> bool {
        !item_info::is_start_info(board, self.item)
    }

    /// Port of `otherRoom(CompleteExpansionRoom)` (TargetItemExpansionDoor.java:50-53) — the
    /// `ExpandableObject` interface method (ExpandableObject.java:22): always `null`, because a
    /// target door leads *out* of the room graph rather than across it.
    ///
    /// Named `other_room` like Java, unlike [`super::ExpansionDoor`], which has two overloads
    /// and needs two names.
    pub fn other_room(&self, _room: RoomRef) -> Option<RoomRef> {
        None
    }

    /// Port of `getMazeSearchElement(int)` (TargetItemExpansionDoor.java:55-58): the one
    /// element, **whatever the index** — Java ignores the argument.
    pub fn get_maze_search_element(&self, _index: usize) -> &MazeSearchElement {
        &self.maze_search_info
    }

    /// [`get_maze_search_element`](Self::get_maze_search_element), mutably.
    pub fn get_maze_search_element_mut(&mut self, _index: usize) -> &mut MazeSearchElement {
        &mut self.maze_search_info
    }

    /// Port of `mazeSearchElementCount` (TargetItemExpansionDoor.java:60-63): always 1.
    pub fn maze_search_element_count(&self) -> usize {
        1
    }

    /// Port of `reset` (TargetItemExpansionDoor.java:65-68).
    pub fn reset(&mut self) {
        self.maze_search_info.reset();
    }

    /// Port of `getId` (TargetItemExpansionDoor.java:70-74):
    /// `31 * item.getId() + (room != null ? room.getId() : 0)`.
    ///
    /// A hash, not an identity, and the `int` arithmetic wraps. The room's id is resolved by
    /// the caller, because it comes from whichever of the three room `getId()`s applies; `0` is
    /// Java's null-room branch.
    pub fn get_id(&self, room_id: i32) -> i32 {
        target_door_id(self.item, room_id)
    }
}

impl TargetItemExpansionDoor {
    /// A target door with a given shape, bypassing the constructor's board lookup.
    ///
    /// Test-only: the real constructor needs a `Board` to read `item.getTreeShape`, and the
    /// store's dispatch tests have no reason to build one.
    #[cfg(test)]
    pub(crate) fn with_shape(
        item: ItemId,
        tree_entry_no: usize,
        room: Option<RoomRef>,
        shape: TileShape,
    ) -> TargetItemExpansionDoor {
        TargetItemExpansionDoor {
            item,
            tree_entry_no,
            room,
            shape,
            maze_search_info: MazeSearchElement::new(),
        }
    }
}

/// [`TargetItemExpansionDoor::get_id`]'s arithmetic (TargetItemExpansionDoor.java:71-74), as a
/// free function so it can be pinned without a board. `room_id` is `0` for Java's null room.
pub fn target_door_id(item: ItemId, room_id: i32) -> i32 {
    (item.0 as i32).wrapping_mul(31).wrapping_add(room_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_id_folds_the_item_and_the_room_and_wraps() {
        assert_eq!(target_door_id(ItemId(3), 4), 3 * 31 + 4);
        assert_eq!(target_door_id(ItemId(3), 0), 3 * 31);
        assert_eq!(
            target_door_id(ItemId(u32::MAX), 1),
            (u32::MAX as i32).wrapping_mul(31).wrapping_add(1)
        );
    }
}
