use fr_board::{Board, ItemId, TreeId, TreeObject};
use fr_geometry::TileShape;

use crate::Arena;
use crate::arena::{DoorId, TargetDoorId};
use crate::autoroute::expansion::{ExpandableRef, ExpansionDoor, RoomRef};

#[derive(Debug, Clone, PartialEq)]
pub struct ObstacleExpansionRoom {
    item: ItemId,
    index_in_item: usize,
    id_no: i32,
    shape: Option<TileShape>,
    doors: Vec<DoorId>,
    doors_calculated: bool,
}

impl ObstacleExpansionRoom {
    pub fn new(
        board: &mut Board,
        item: ItemId,
        index_in_item: usize,
        tree: TreeId,
        id_no: i32,
    ) -> ObstacleExpansionRoom {
        ObstacleExpansionRoom {
            item,
            index_in_item,
            id_no,
            shape: board.item_tree_shape(item, tree, index_in_item),
            doors: Vec::new(),
            doors_calculated: false,
        }
    }

    pub fn get_index_in_item(&self) -> usize {
        self.index_in_item
    }

    pub fn get_item(&self) -> ItemId {
        self.item
    }

    pub fn get_layer(&self, board: &Board) -> Option<usize> {
        board.item_shape_layer(self.item, self.index_in_item)
    }

    pub fn get_shape(&self) -> Option<&TileShape> {
        self.shape.as_ref()
    }

    pub fn get_id(&self) -> i32 {
        self.id_no
    }

    pub fn id(item: ItemId, index_in_item: usize) -> i32 {
        (item.0 as i32).wrapping_shl(10) | (index_in_item as i32)
    }

    pub fn door_exists(&self, doors: &Arena<ExpansionDoor>, other: RoomRef) -> bool {
        self.doors.iter().any(|door| {
            doors
                .get(door.0)
                .is_some_and(|d| d.first_room == other || d.second_room == other)
        })
    }

    pub fn add_door(&mut self, door: DoorId) {
        self.doors.push(door);
    }

    pub fn get_doors(&self) -> &[DoorId] {
        &self.doors
    }

    pub fn clear_doors(&mut self) {
        self.doors = Vec::new();
    }

    pub fn reset_doors(&self, doors: &mut Arena<ExpansionDoor>) {
        for door in &self.doors {
            if let Some(door) = doors.get_mut(door.0) {
                door.reset();
            }
        }
    }

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

    pub fn get_target_doors(&self) -> &[TargetDoorId] {
        &[]
    }

    pub fn get_object(&self) -> TreeObject {
        TreeObject::Item(self.item)
    }

    pub fn all_doors_calculated(&self) -> bool {
        self.doors_calculated
    }

    pub fn set_doors_calculated(&mut self, value: bool) {
        self.doors_calculated = value;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_id_is_an_or_not_a_sum_so_a_wide_index_aliases() {
        assert_eq!(
            ObstacleExpansionRoom::id(ItemId(1), 1024),
            ObstacleExpansionRoom::id(ItemId(1), 0)
        );
        assert_eq!(
            ObstacleExpansionRoom::id(ItemId(1), 2048),
            ObstacleExpansionRoom::id(ItemId(3), 0)
        );
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
