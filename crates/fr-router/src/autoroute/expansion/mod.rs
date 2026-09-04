pub mod complete_room;
pub mod door;
pub mod free_space_room;
pub mod incomplete_room;
pub mod obstacle_room;
pub mod room;
pub mod sorted_neighbours;
pub mod sorted_neighbours_45;
pub mod sorted_neighbours_orthogonal;
pub mod target_door;

pub use complete_room::CompleteFreeSpaceExpansionRoom;
pub use door::ExpansionDoor;
pub use free_space_room::FreeSpaceExpansionRoom;
pub use incomplete_room::IncompleteFreeSpaceExpansionRoom;
pub use obstacle_room::ObstacleExpansionRoom;
pub use room::{ExpandableRef, RoomRef};
pub use sorted_neighbours::{
    CalculationMode, SortedRoomNeighbour, SortedRoomNeighbours, select_calculation_mode,
};
pub use sorted_neighbours_45::Sorted45DegreeRoomNeighbours;
pub use sorted_neighbours_orthogonal::SortedOrthogonalRoomNeighbours;
pub use target_door::{TargetItemExpansionDoor, target_door_id};

use fr_board::searchtree::ShapeSearchTree;
use fr_board::{Board, ItemId, ObstacleRoomId, RoomId, TreeId, TreeObject};
use fr_geometry::{FloatLine, TileShape};

use crate::Arena;
use crate::arena::{DoorId, IncompleteRoomId, TargetDoorId};
use crate::autoroute::drill::ExpansionDrill;

#[derive(Debug, Clone, Default)]
pub struct ExpansionRoomStore {
                                            pub complete_rooms: Arena<CompleteFreeSpaceExpansionRoom>,
        pub incomplete_rooms: Arena<IncompleteFreeSpaceExpansionRoom>,
                pub obstacle_rooms: Arena<ObstacleExpansionRoom>,
            pub doors: Arena<ExpansionDoor>,
            pub target_doors: Arena<TargetItemExpansionDoor>,
                                                                    pub drills: Arena<ExpansionDrill>,
        room_instance_count: i32,
                                    incomplete_list_created: bool,
}

impl fr_board::RoomLookup for ExpansionRoomStore {
    fn room_tree_shape(&self, id: RoomId) -> Option<&TileShape> {
        self.complete_rooms.get(id.0)?.get_tree_shape(0)
    }

    fn room_shape_layer(&self, id: RoomId) -> Option<usize> {
        Some(self.complete_rooms.get(id.0)?.shape_layer(0))
    }
}

impl ExpansionRoomStore {
            pub fn new() -> ExpansionRoomStore {
        ExpansionRoomStore::default()
    }

                                                                                            pub fn next_room_id_no(&mut self) -> i32 {
        self.room_instance_count = self.room_instance_count.wrapping_add(1);
        self.room_instance_count
    }

                                                                                                        pub fn release_room_id_no(&mut self, id_no: i32) {
        if self.room_instance_count == id_no {
            self.room_instance_count = self.room_instance_count.wrapping_sub(1);
        }
    }

                                                                                                                pub fn clear(&mut self, tree: &mut ShapeSearchTree) {
        for (_, room) in self.complete_rooms.iter_mut() {
            room.remove_from_tree(tree);
        }
        self.complete_rooms.clear();
        self.incomplete_rooms.clear();
        self.obstacle_rooms.clear();
        self.doors.clear();
        self.target_doors.clear();
        self.room_instance_count = 0;
        self.incomplete_list_created = false;
    }


                                    pub fn new_complete_room(&mut self, shape: Option<TileShape>, layer: usize, id: i32) -> RoomId {
        let room_id = RoomId(
            u32::try_from(self.complete_rooms.slot_count())
                .expect("expansion-room arena index overflowed u32"),
        );
        let index = self
            .complete_rooms
            .insert(CompleteFreeSpaceExpansionRoom::new(
                shape, layer, id, room_id,
            ));
        debug_assert_eq!(index, room_id.0);
        room_id
    }

                                                    pub fn new_incomplete_room(
        &mut self,
        shape: Option<TileShape>,
        layer: usize,
        contained_shape: Option<TileShape>,
    ) -> IncompleteRoomId {
        self.incomplete_list_created = true;
        self.new_unlisted_incomplete_room(shape, layer, contained_shape)
    }

                                                                            pub fn new_unlisted_incomplete_room(
        &mut self,
        shape: Option<TileShape>,
        layer: usize,
        contained_shape: Option<TileShape>,
    ) -> IncompleteRoomId {
        let mut room = IncompleteFreeSpaceExpansionRoom::new(shape, layer, contained_shape);
        room.set_id_no(self.next_room_id_no());
        IncompleteRoomId(self.incomplete_rooms.insert(room))
    }

        pub fn incomplete_list_created(&self) -> bool {
        self.incomplete_list_created
    }

                pub fn new_obstacle_room(
        &mut self,
        board: &mut Board,
        item: ItemId,
        index_in_item: usize,
        tree: TreeId,
    ) -> ObstacleRoomId {
        let id_no = self.next_room_id_no();
        let room = ObstacleExpansionRoom::new(board, item, index_in_item, tree, id_no);
        ObstacleRoomId(self.obstacle_rooms.insert(room))
    }

                        pub fn new_door(
        &mut self,
        first_room: RoomRef,
        second_room: RoomRef,
        dimension: i32,
    ) -> DoorId {
        DoorId(
            self.doors
                .insert(ExpansionDoor::new(first_room, second_room, dimension)),
        )
    }

                            pub fn new_door_from_shapes(
        &mut self,
        first_room: RoomRef,
        second_room: RoomRef,
    ) -> Option<DoorId> {
        let first_shape = self.room_shape(first_room)?.clone();
        let second_shape = self.room_shape(second_room)?.clone();
        let door = ExpansionDoor::new_with_computed_dimension(
            first_room,
            second_room,
            &first_shape,
            &second_shape,
        );
        Some(DoorId(self.doors.insert(door)))
    }

            pub fn new_target_door(
        &mut self,
        board: &mut Board,
        item: ItemId,
        tree_entry_no: usize,
        room: Option<RoomRef>,
        tree: TreeId,
    ) -> TargetDoorId {
        let room_shape = room.and_then(|r| self.room_shape(r)).cloned();
        let door = TargetItemExpansionDoor::new(
            board,
            item,
            tree_entry_no,
            room,
            room_shape.as_ref(),
            tree,
        );
        TargetDoorId(self.target_doors.insert(door))
    }


                                        pub fn insert_complete_room(&mut self, tree: &mut ShapeSearchTree, room: RoomId) {
        let Some(shape) = self
            .complete_rooms
            .get(room.0)
            .and_then(|r| r.get_shape())
            .cloned()
        else {
            return;
        };
        let leaf = tree.insert_room(room, &shape);
        if let Some(r) = self.complete_rooms.get_mut(room.0) {
            r.set_search_tree_entries(leaf);
        }
    }

                                                            pub fn remove_complete_room(&mut self, tree: &mut ShapeSearchTree, room: RoomId) -> bool {
        match self.complete_rooms.remove(room.0) {
            Some(mut r) => {
                r.remove_from_tree(tree);
                true
            }
            None => false,
        }
    }


        pub fn complete_room(&self, room: RoomId) -> Option<&CompleteFreeSpaceExpansionRoom> {
        self.complete_rooms.get(room.0)
    }

        pub fn complete_room_mut(
        &mut self,
        room: RoomId,
    ) -> Option<&mut CompleteFreeSpaceExpansionRoom> {
        self.complete_rooms.get_mut(room.0)
    }

        pub fn incomplete_room(
        &self,
        room: IncompleteRoomId,
    ) -> Option<&IncompleteFreeSpaceExpansionRoom> {
        self.incomplete_rooms.get(room.0)
    }

        pub fn incomplete_room_mut(
        &mut self,
        room: IncompleteRoomId,
    ) -> Option<&mut IncompleteFreeSpaceExpansionRoom> {
        self.incomplete_rooms.get_mut(room.0)
    }

        pub fn obstacle_room(&self, room: ObstacleRoomId) -> Option<&ObstacleExpansionRoom> {
        self.obstacle_rooms.get(room.0)
    }

        pub fn obstacle_room_mut(
        &mut self,
        room: ObstacleRoomId,
    ) -> Option<&mut ObstacleExpansionRoom> {
        self.obstacle_rooms.get_mut(room.0)
    }

        pub fn door(&self, door: DoorId) -> Option<&ExpansionDoor> {
        self.doors.get(door.0)
    }

        pub fn door_mut(&mut self, door: DoorId) -> Option<&mut ExpansionDoor> {
        self.doors.get_mut(door.0)
    }

        pub fn target_door(&self, door: TargetDoorId) -> Option<&TargetItemExpansionDoor> {
        self.target_doors.get(door.0)
    }

        pub fn target_door_mut(&mut self, door: TargetDoorId) -> Option<&mut TargetItemExpansionDoor> {
        self.target_doors.get_mut(door.0)
    }


            pub fn room_shape(&self, room: RoomRef) -> Option<&TileShape> {
        match room {
            RoomRef::Complete(id) => self.complete_rooms.get(id.0)?.get_shape(),
            RoomRef::Obstacle(id) => self.obstacle_rooms.get(id.0)?.get_shape(),
            RoomRef::Incomplete(id) => self.incomplete_rooms.get(id.0)?.get_shape(),
        }
    }

                pub fn room_layer(&self, board: &Board, room: RoomRef) -> Option<usize> {
        match room {
            RoomRef::Complete(id) => Some(self.complete_rooms.get(id.0)?.get_layer()),
            RoomRef::Obstacle(id) => self.obstacle_rooms.get(id.0)?.get_layer(board),
            RoomRef::Incomplete(id) => Some(self.incomplete_rooms.get(id.0)?.get_layer()),
        }
    }

                                pub fn room_id_no(&self, room: RoomRef) -> Option<i32> {
        match room {
            RoomRef::Complete(id) => Some(self.complete_rooms.get(id.0)?.get_id()),
            RoomRef::Obstacle(id) => Some(self.obstacle_rooms.get(id.0)?.get_id()),
            RoomRef::Incomplete(id) => Some(self.incomplete_rooms.get(id.0)?.get_id()),
        }
    }

                        pub fn get_object(&self, room: RoomRef) -> Option<TreeObject> {
        match room {
            RoomRef::Complete(id) => Some(self.complete_rooms.get(id.0)?.get_object()),
            RoomRef::Obstacle(id) => Some(self.obstacle_rooms.get(id.0)?.get_object()),
            RoomRef::Incomplete(_) => None,
        }
    }

        pub fn add_door(&mut self, room: RoomRef, door: DoorId) {
        match room {
            RoomRef::Complete(id) => {
                if let Some(r) = self.complete_rooms.get_mut(id.0) {
                    r.add_door(door);
                }
            }
            RoomRef::Obstacle(id) => {
                if let Some(r) = self.obstacle_rooms.get_mut(id.0) {
                    r.add_door(door);
                }
            }
            RoomRef::Incomplete(id) => {
                if let Some(r) = self.incomplete_rooms.get_mut(id.0) {
                    r.add_door(door);
                }
            }
        }
    }

            pub fn room_doors(&self, room: RoomRef) -> &[DoorId] {
        match room {
            RoomRef::Complete(id) => self
                .complete_rooms
                .get(id.0)
                .map_or(&[][..], |r| r.get_doors()),
            RoomRef::Obstacle(id) => self
                .obstacle_rooms
                .get(id.0)
                .map_or(&[][..], |r| r.get_doors()),
            RoomRef::Incomplete(id) => self
                .incomplete_rooms
                .get(id.0)
                .map_or(&[][..], |r| r.get_doors()),
        }
    }

            pub fn room_target_doors(&self, room: RoomRef) -> &[TargetDoorId] {
        match room {
            RoomRef::Complete(id) => self
                .complete_rooms
                .get(id.0)
                .map_or(&[][..], |r| r.get_target_doors()),
            RoomRef::Obstacle(_) | RoomRef::Incomplete(_) => &[],
        }
    }

            pub fn clear_doors(&mut self, room: RoomRef) {
        match room {
            RoomRef::Complete(id) => {
                if let Some(r) = self.complete_rooms.get_mut(id.0) {
                    r.clear_doors();
                }
            }
            RoomRef::Obstacle(id) => {
                if let Some(r) = self.obstacle_rooms.get_mut(id.0) {
                    r.clear_doors();
                }
            }
            RoomRef::Incomplete(id) => {
                if let Some(r) = self.incomplete_rooms.get_mut(id.0) {
                    r.clear_doors();
                }
            }
        }
    }

                        pub fn reset_doors(&mut self, room: RoomRef) {
        let doors: Vec<DoorId> = self.room_doors(room).to_vec();
        for door in doors {
            if let Some(d) = self.doors.get_mut(door.0) {
                d.reset();
            }
        }
        if let RoomRef::Complete(id) = room {
            let target_doors: Vec<TargetDoorId> = self
                .complete_rooms
                .get(id.0)
                .map_or(Vec::new(), |r| r.get_target_doors().to_vec());
            for door in target_doors {
                if let Some(d) = self.target_doors.get_mut(door.0) {
                    d.reset();
                }
            }
        }
    }

            pub fn door_exists(&self, room: RoomRef, other: RoomRef) -> bool {
        match room {
            RoomRef::Complete(id) => self
                .complete_rooms
                .get(id.0)
                .is_some_and(|r| r.door_exists(&self.doors, other)),
            RoomRef::Obstacle(id) => self
                .obstacle_rooms
                .get(id.0)
                .is_some_and(|r| r.door_exists(&self.doors, other)),
            RoomRef::Incomplete(id) => self
                .incomplete_rooms
                .get(id.0)
                .is_some_and(|r| r.door_exists(&self.doors, other)),
        }
    }

            pub fn remove_door(&mut self, room: RoomRef, door: ExpandableRef) -> bool {
        match room {
            RoomRef::Complete(id) => self
                .complete_rooms
                .get_mut(id.0)
                .is_some_and(|r| r.remove_door(door)),
            RoomRef::Obstacle(id) => self
                .obstacle_rooms
                .get_mut(id.0)
                .is_some_and(|r| r.remove_door(door)),
            RoomRef::Incomplete(id) => match door {
                ExpandableRef::Door(d) => self
                    .incomplete_rooms
                    .get_mut(id.0)
                    .is_some_and(|r| r.remove_door(d)),
                _ => false,
            },
        }
    }


                pub fn door_shape(&self, door: DoorId) -> Option<TileShape> {
        let d = self.doors.get(door.0)?;
        let first = self.room_shape(d.first_room)?;
        let second = self.room_shape(d.second_room)?;
        Some(d.get_shape(first, second))
    }

                            pub fn door_section_segments(&mut self, door: DoorId, offset: f64) -> Vec<FloatLine> {
        let Some(d) = self.doors.get(door.0) else {
            return Vec::new();
        };
        let (Some(first), Some(second)) = (
            self.room_shape(d.first_room).cloned(),
            self.room_shape(d.second_room).cloned(),
        ) else {
            return Vec::new();
        };
        match self.doors.get_mut(door.0) {
            Some(d) => d.get_section_segments(&first, &second, offset),
            None => Vec::new(),
        }
    }

            pub fn door_id_no(&self, door: DoorId) -> Option<i32> {
        let d = self.doors.get(door.0)?;
        Some(ExpansionDoor::id(
            self.room_id_no(d.first_room)?,
            self.room_id_no(d.second_room)?,
        ))
    }

                                                pub fn remove_all_doors(&mut self, room: RoomRef) {
        let doors: Vec<DoorId> = self.room_doors(room).to_vec();
        for door in doors {
            let Some(other) = self.doors.get(door.0).and_then(|d| d.other_room(room)) else {
                continue;
            };
            self.remove_door(other, ExpandableRef::Door(door));
            if let RoomRef::Incomplete(id) = other {
                self.remove_incomplete_expansion_room(id);
            }
        }
        self.clear_doors(room);
    }

                                                                            pub fn detach_all_doors(&mut self, room: RoomRef) {
        let doors: Vec<DoorId> = self.room_doors(room).to_vec();
        for door in doors {
            let Some(other) = self.doors.get(door.0).and_then(|d| d.other_room(room)) else {
                continue;
            };
            self.remove_door(other, ExpandableRef::Door(door));
        }
        self.clear_doors(room);
    }

                                                                                                                                                        pub fn remove_incomplete_expansion_room(&mut self, room: IncompleteRoomId) {
        self.remove_all_doors(RoomRef::Incomplete(room));
        self.incomplete_rooms.remove(room.0);
    }

            pub fn target_door_id_no(&self, door: TargetDoorId) -> Option<i32> {
        let d = self.target_doors.get(door.0)?;
        let room_id = match d.room {
            Some(room) => self.room_id_no(room)?,
            None => 0,
        };
        Some(target_door_id(d.item, room_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fr_geometry::IntBox;

    fn boxed(llx: i32, lly: i32, urx: i32, ury: i32) -> TileShape {
        TileShape::Box(IntBox::from_coords(llx, lly, urx, ury))
    }

    fn two_rooms(store: &mut ExpansionRoomStore) -> (RoomRef, RoomRef) {
        let a = store.next_room_id_no();
        let a = store.new_complete_room(Some(boxed(0, 0, 10, 10)), 0, a);
        let b = store.next_room_id_no();
        let b = store.new_complete_room(Some(boxed(10, 0, 20, 10)), 0, b);
        (RoomRef::Complete(a), RoomRef::Complete(b))
    }

    #[test]
    fn the_counter_pre_increments_so_the_first_room_id_is_one() {
        let mut store = ExpansionRoomStore::new();
        assert_eq!(store.next_room_id_no(), 1);
        assert_eq!(store.next_room_id_no(), 2);
        assert_eq!(store.next_room_id_no(), 3);
    }

        fn bare_tree() -> ShapeSearchTree {
        ShapeSearchTree::new(
            fr_board::TreeId(0),
            fr_board::structure::AngleRestriction::NinetyDegree,
            0,
        )
    }

    #[test]
    fn clear_removes_the_tree_leaves_before_it_drains_the_arenas() {
        let mut tree = bare_tree();
        let mut store = ExpansionRoomStore::new();
        let (a, b) = two_rooms(&mut store);
        let (RoomRef::Complete(a_id), RoomRef::Complete(b_id)) = (a, b) else {
            unreachable!()
        };
        store.insert_complete_room(&mut tree, a_id);
        store.insert_complete_room(&mut tree, b_id);
        assert_eq!(tree.tree().leaf_count(), 2);

        let door = store.new_door(a, b, 1);
        store.add_door(a, door);
        store.clear(&mut tree);

        assert_eq!(tree.tree().leaf_count(), 0, "the room leaves are gone");
        assert!(tree.tree().is_empty());
        assert!(store.complete_rooms.is_empty());
        assert!(store.doors.is_empty());
        assert_eq!(store.next_room_id_no(), 1, "the counter went back to 0");
        let fresh = store.next_room_id_no();
        assert_eq!(
            store.new_complete_room(Some(boxed(0, 0, 1, 1)), 0, fresh),
            RoomId(0)
        );
    }

    #[test]
    fn clear_is_safe_for_a_room_that_never_entered_the_tree() {
        let mut tree = bare_tree();
        let mut store = ExpansionRoomStore::new();
        let id = store.next_room_id_no();
        store.new_complete_room(None, 0, id);
        store.clear(&mut tree);
        assert!(tree.tree().is_empty());
        assert!(store.complete_rooms.is_empty());
    }

    #[test]
    fn reset_doors_reaches_both_the_doors_and_the_target_doors() {
        let mut store = ExpansionRoomStore::new();
        let (a, b) = two_rooms(&mut store);
        let door = store.new_door(a, b, 1);
        store.add_door(a, door);
        store.door_mut(door).unwrap().allocate_sections(2);
        store
            .door_mut(door)
            .unwrap()
            .get_maze_search_element_mut(0)
            .unwrap()
            .is_occupied = true;

        let target = TargetDoorId(
            store
                .target_doors
                .insert(TargetItemExpansionDoor::with_shape(
                    ItemId(1),
                    0,
                    Some(a),
                    boxed(0, 0, 1, 1),
                )),
        );
        let RoomRef::Complete(a_id) = a else {
            unreachable!()
        };
        store
            .complete_room_mut(a_id)
            .unwrap()
            .add_target_door(target);
        store
            .target_door_mut(target)
            .unwrap()
            .get_maze_search_element_mut(0)
            .room_ripped = true;

        store.reset_doors(a);
        assert!(
            !store
                .door(door)
                .unwrap()
                .get_maze_search_element(0)
                .unwrap()
                .is_occupied
        );
        assert!(
            !store
                .target_door(target)
                .unwrap()
                .get_maze_search_element(0)
                .room_ripped
        );
    }

    #[test]
    fn only_a_complete_free_space_room_has_target_doors() {
        let mut store = ExpansionRoomStore::new();
        let (a, _) = two_rooms(&mut store);
        let incomplete =
            RoomRef::Incomplete(store.new_incomplete_room(Some(boxed(0, 0, 1, 1)), 0, None));
        assert!(store.room_target_doors(a).is_empty());
        assert!(store.room_target_doors(incomplete).is_empty());
        assert_eq!(store.get_object(incomplete), None);
    }

    #[test]
    fn a_stale_id_reads_a_hole_rather_than_someone_elses_room() {
        let mut store = ExpansionRoomStore::new();
        let (a, _) = two_rooms(&mut store);
        let RoomRef::Complete(id) = a else {
            unreachable!()
        };
        store.complete_rooms.remove(id.0);
        assert_eq!(store.room_shape(a), None);
        assert_eq!(store.room_id_no(a), None);
        assert!(store.room_doors(a).is_empty());
        assert!(!store.door_exists(a, a));
        assert!(!store.remove_door(a, ExpandableRef::Door(DoorId(0))));
    }

    #[test]
    fn the_three_get_ids_dispatch_to_three_different_formulas() {
        let mut store = ExpansionRoomStore::new();
        let counter = store.next_room_id_no();
        let complete =
            RoomRef::Complete(store.new_complete_room(Some(boxed(0, 0, 4, 4)), 2, counter));
        assert_eq!(store.room_id_no(complete), Some(counter));

        let shape = boxed(1, 1, 5, 5);
        let incomplete =
            RoomRef::Incomplete(store.new_incomplete_room(Some(shape.clone()), 3, None));
        assert_eq!(store.room_id_no(incomplete), Some(counter + 1));
        let RoomRef::Incomplete(id) = incomplete else {
            unreachable!()
        };
        assert_eq!(
            store.incomplete_room(id).unwrap().java_id(),
            shape.get_id().wrapping_mul(31).wrapping_add(3),
            "Java's formula, kept pinned"
        );
    }
}
