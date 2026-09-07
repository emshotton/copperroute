use copper_board::{Board, ItemId, TreeId};
use copper_geometry::{Simplex, TileShape};

use crate::autoroute::expansion::RoomRef;
use crate::autoroute::item_info;
use crate::autoroute::maze::MazeSearchElement;

#[derive(Debug, Clone, PartialEq)]
pub struct TargetItemExpansionDoor {
    pub item: ItemId,
    pub tree_entry_no: usize,
    pub room: Option<RoomRef>,
    shape: TileShape,
    maze_search_info: MazeSearchElement,
}

impl TargetItemExpansionDoor {
    pub fn new(
        board: &mut Board,
        item: ItemId,
        tree_entry_no: usize,
        room: Option<RoomRef>,
        room_shape: Option<&TileShape>,
        tree: TreeId,
    ) -> TargetItemExpansionDoor {
        let shape = match (room, room_shape) {
            (Some(_), Some(room_shape)) => match board.item_tree_shape(item, tree, tree_entry_no) {
                Some(item_shape) => item_shape.intersection(room_shape),
                None => TileShape::Simplex(Simplex::EMPTY),
            },
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

    pub fn get_shape(&self) -> &TileShape {
        &self.shape
    }

    pub fn get_dimension(&self) -> i32 {
        2
    }

    pub fn is_destination_door(&self, board: &mut Board) -> bool {
        !item_info::is_start_info(board, self.item)
    }

    pub fn other_room(&self, _room: RoomRef) -> Option<RoomRef> {
        None
    }

    pub fn get_maze_search_element(&self, _index: usize) -> &MazeSearchElement {
        &self.maze_search_info
    }

    pub fn get_maze_search_element_mut(&mut self, _index: usize) -> &mut MazeSearchElement {
        &mut self.maze_search_info
    }

    pub fn maze_search_element_count(&self) -> usize {
        1
    }

    pub fn reset(&mut self) {
        self.maze_search_info.reset();
    }

    pub fn get_id(&self, room_id: i32) -> i32 {
        target_door_id(self.item, room_id)
    }
}

impl TargetItemExpansionDoor {
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
