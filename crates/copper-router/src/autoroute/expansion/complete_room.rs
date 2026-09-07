use std::cmp::Ordering;

use copper_board::datastructures::{LeafId, TreeEntry};
use copper_board::searchtree::ShapeSearchTree;
use copper_board::{Board, ItemLookup, RoomId, TreeId, TreeObject};
use copper_geometry::TileShape;

use crate::Arena;
use crate::arena::{DoorId, TargetDoorId};
use crate::autoroute::expansion::{
    ExpandableRef, ExpansionDoor, ExpansionRoomStore, FreeSpaceExpansionRoom, RoomRef,
};

#[derive(Debug, Clone, PartialEq)]
pub struct CompleteFreeSpaceExpansionRoom {
    pub base: FreeSpaceExpansionRoom,
    id: i32,
    room_id: RoomId,
    tree_leaf: Option<LeafId>,
    target_doors: Vec<TargetDoorId>,
    room_is_net_dependent: bool,
}

impl CompleteFreeSpaceExpansionRoom {
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

    pub fn set_search_tree_entries(&mut self, leaf: Option<LeafId>) {
        self.tree_leaf = leaf;
    }

    pub fn tree_leaf(&self) -> Option<LeafId> {
        self.tree_leaf
    }

    pub fn room_id(&self) -> RoomId {
        self.room_id
    }

    pub fn compare_to(&self, other: &CompleteFreeSpaceExpansionRoom) -> Ordering {
        other.id.cmp(&self.id)
    }

    pub fn remove_from_tree(&mut self, tree: &mut ShapeSearchTree) {
        tree.remove_room(self.tree_leaf.take());
    }

    pub fn tree_shape_count(&self) -> usize {
        1
    }

    pub fn get_tree_shape(&self, _index: usize) -> Option<&TileShape> {
        self.base.get_shape()
    }

    pub fn shape_layer(&self, _index: usize) -> usize {
        self.base.get_layer()
    }

    pub fn is_obstacle(&self, _net_number: i32) -> bool {
        true
    }

    pub fn is_trace_obstacle(&self, _net_number: i32) -> bool {
        true
    }

    pub fn set_net_dependent(&mut self) {
        self.room_is_net_dependent = true;
    }

    pub fn is_net_dependent(&self) -> bool {
        self.room_is_net_dependent
    }

    pub fn get_id(&self) -> i32 {
        self.id
    }

    pub fn get_target_doors(&self) -> &[TargetDoorId] {
        &self.target_doors
    }

    pub fn add_target_door(&mut self, door: TargetDoorId) {
        self.target_doors.push(door);
    }

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

    pub fn get_object(&self) -> TreeObject {
        TreeObject::Room(self.room_id)
    }

    pub fn clear_doors(&mut self) {
        self.base.clear_doors();
        self.target_doors = Vec::new();
    }

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

    pub fn get_shape(&self) -> Option<&TileShape> {
        self.base.get_shape()
    }

    pub fn set_shape(&mut self, shape: Option<TileShape>) {
        self.base.set_shape(shape);
    }

    pub fn get_layer(&self) -> usize {
        self.base.get_layer()
    }

    pub fn add_door(&mut self, door: DoorId) {
        self.base.add_door(door);
    }

    pub fn get_doors(&self) -> &[DoorId] {
        self.base.get_doors()
    }

    pub fn door_exists(&self, doors: &Arena<ExpansionDoor>, other: RoomRef) -> bool {
        self.base.door_exists(doors, other)
    }

    /// `FRLogger.warn("ExpansionRoom overlap conflict")` is dropped; the `false` it accompanies is
    pub fn validate(
        &self,
        engine: &crate::autoroute::maze::engine::AutorouteEngine,
        board: &Board,
        room_id: RoomId,
    ) -> bool {
        use crate::autoroute::expansion::sorted_neighbours::{
            object_is_trace_obstacle, object_tree_shape,
        };

        let mut result = true;
        let net_numbers = [engine.get_net_number()];
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
        for current_entry in overlapping_objects {
            if current_entry.object == TreeObject::Room(room_id) {
                continue;
            }
            if !object_is_trace_obstacle(
                current_entry.object,
                engine.get_net_number(),
                &board.items,
            ) {
                continue;
            }
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
            let current_shape = object_tree_shape(
                tree,
                current_entry.object,
                current_entry.shape_index,
                &board.items,
                &engine.rooms,
                &ctx,
            );
            if room_shape.intersection(&current_shape).dimension() > 1 {
                result = false;
            }
        }
        result
    }
}

pub fn calculate_target_doors(
    room: RoomId,
    own_net_object: &TreeEntry<TreeObject>,
    net_number: i32,
    board: &mut Board,
    rooms: &mut ExpansionRoomStore,
    tree_id: TreeId,
) {
    if let Some(r) = rooms.complete_room_mut(room) {
        r.set_net_dependent();
    }
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
        if !connectable.as_dyn().contains_net(net_number) {
            return;
        }
        connectable
            .as_dyn()
            .get_trace_connection_shape(tree_id, own_net_object.shape_index, &ctx)
    };
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

#[cfg(test)]
mod tests {
    use super::*;
    use copper_geometry::IntBox;

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
