use std::cmp::Ordering;

use copper_board::{Board, TreeId, TreeObject};
use copper_geometry::{CRIT_INT, IntBox, TileShape};

use crate::autoroute::expansion::complete_room::calculate_target_doors;
use crate::autoroute::expansion::sorted_neighbours::{
    create_overlap_door, insert_door_ok, object_id, object_is_trace_obstacle, object_tree_shape,
    tree_of,
};
use crate::autoroute::expansion::{ExpansionRoomStore, IncompleteFreeSpaceExpansionRoom, RoomRef};
use crate::autoroute::item_info;
use crate::autoroute::tree_ext::AutorouteSearchTreeExt;
use std::collections::BTreeSet;

#[derive(Debug, Clone)]
pub struct SortedOrthogonalRoomNeighbours {
    pub completed_room: RoomRef,
    pub sorted_neighbours: BTreeSet<SortedRoomNeighbour>,
    pub from_room: RoomRef,
    pub is_obstacle_expansion_room: bool,
    pub room_shape: IntBox,
    pub edge_interior_touches_obstacle: [bool; 4],
}

impl SortedOrthogonalRoomNeighbours {
    fn new(
        from_room: RoomRef,
        completed_room: RoomRef,
        room_shape: IntBox,
    ) -> SortedOrthogonalRoomNeighbours {
        SortedOrthogonalRoomNeighbours {
            completed_room,
            sorted_neighbours: BTreeSet::new(),
            from_room,
            is_obstacle_expansion_room: matches!(from_room, RoomRef::Obstacle(_)),
            room_shape,
            edge_interior_touches_obstacle: [false; 4],
        }
    }

    pub fn calculate(
        room: RoomRef,
        net_number: i32,
        board: &mut Board,
        rooms: &mut ExpansionRoomStore,
        tree_id: TreeId,
    ) -> Option<RoomRef> {
        loop {
            let room_id_no = rooms.next_room_id_no();
            let room_neighbours = SortedOrthogonalRoomNeighbours::calculate_neighbours(
                room, net_number, board, rooms, tree_id, room_id_no,
            )?;

            let edge_removed = room_neighbours.try_remove_edge(net_number, board, rooms, tree_id);
            let result = room_neighbours.completed_room;
            if edge_removed {
                rooms.remove_all_doors(result);
                continue;
            }

            if room_neighbours.sorted_neighbours.is_empty() {
                if let RoomRef::Obstacle(_) = result {
                    calculate_incomplete_rooms_with_empty_neighbours(result, board, rooms);
                }
            } else {
                room_neighbours.calculate_new_incomplete_rooms(board, rooms);
            }
            return Some(result);
        }
    }

    pub fn calculate_neighbours(
        room: RoomRef,
        net_number: i32,
        board: &mut Board,
        rooms: &mut ExpansionRoomStore,
        tree_id: TreeId,
        room_id_no: i32,
    ) -> Option<SortedOrthogonalRoomNeighbours> {
        let room_shape = rooms
            .room_shape(room)
            .unwrap_or_else(|| {
                panic!(
                    "SortedOrthogonalRoomNeighbours.calculateNeighbours: room {room:?} has no \
                     shape (SortedOrthogonalRoomNeighbours.java:116) — Java NPEs here too"
                )
            })
            .clone();
        let TileShape::Box(room_box) = room_shape else {
            return None;
        };
        let layer = rooms.room_layer(board, room).unwrap_or_else(|| {
            panic!(
                "SortedOrthogonalRoomNeighbours.calculateNeighbours: room {room:?} has no layer \
                 (SortedOrthogonalRoomNeighbours.java:123) — Java NPEs here too"
            )
        });

        let completed_room = match room {
            RoomRef::Incomplete(_) => RoomRef::Complete(rooms.new_complete_room(
                Some(room_shape.clone()),
                layer,
                room_id_no,
            )),
            RoomRef::Obstacle(id) => RoomRef::Obstacle(id),
            RoomRef::Complete(_) => return None,
        };

        let mut result = SortedOrthogonalRoomNeighbours::new(room, completed_room, room_box);

        let mut overlapping_objects = {
            let ctx = board.ctx();
            tree_of(board, tree_id).overlapping_tree_entries_with_rooms(
                &room_shape,
                Some(layer),
                &[],
                &board.items,
                &*rooms,
                &ctx,
            )
        };

        overlapping_objects.sort_by(|e1, e2| {
            let id_diff = object_id(e1.object, rooms).wrapping_sub(object_id(e2.object, rooms));
            if id_diff != 0 {
                return id_diff.cmp(&0);
            }
            e1.shape_index.cmp(&e2.shape_index)
        });

        for current_entry in overlapping_objects {
            let current_object = current_entry.object;
            if rooms
                .get_object(room)
                .is_some_and(|object| room.is_free_space() && object == current_object)
            {
                continue;
            }

            if let RoomRef::Complete(free_room) = completed_room
                && !object_is_trace_obstacle(current_object, net_number, &board.items)
            {
                calculate_target_doors(
                    free_room,
                    &current_entry,
                    net_number,
                    board,
                    rooms,
                    tree_id,
                );
                continue;
            }

            let current_shape = {
                let ctx = board.ctx();
                object_tree_shape(
                    tree_of(board, tree_id),
                    current_object,
                    current_entry.shape_index,
                    &board.items,
                    &*rooms,
                    &ctx,
                )
            };
            let TileShape::Box(current_box) = current_shape else {
                return None;
            };
            let intersection = room_box.intersection(&current_box);
            let dimension = intersection.dimension();

            if dimension > 1
                && let RoomRef::Obstacle(obstacle_room) = completed_room
            {
                if let TreeObject::Item(item_id) = current_object
                    && board
                        .get_item(item_id)
                        .is_some_and(|item| item.is_routable())
                {
                    let overlap_room = {
                        let (b, r) = (&mut *board, &mut *rooms);
                        item_info::get_expansion_room(
                            b,
                            item_id,
                            current_entry.shape_index,
                            tree_id,
                            |b, item, index, tree| r.new_obstacle_room(b, item, index, tree),
                        )
                    };
                    let overlap_room = overlap_room.unwrap_or_else(|| {
                        panic!(
                            "SortedOrthogonalRoomNeighbours.calculateNeighbours: item {item_id} \
                             has no expansion room for shape {} \
                             (SortedOrthogonalRoomNeighbours.java:175) — Java NPEs in \
                             createOverlapDoor here too",
                            current_entry.shape_index
                        )
                    });
                    create_overlap_door(obstacle_room, overlap_room, board, rooms);
                }
                continue;
            }
            if dimension < 0 {
                continue;
            }

            result.add_sorted_neighbour(current_object, rooms, current_box, intersection);

            if dimension > 0 {
                let neighbour_room: Option<RoomRef> = match current_object {
                    TreeObject::Room(id) => Some(RoomRef::Complete(id)),
                    TreeObject::Item(item_id) => {
                        if board
                            .get_item(item_id)
                            .is_some_and(|item| item.is_routable())
                        {
                            let (b, r) = (&mut *board, &mut *rooms);
                            item_info::get_expansion_room(
                                b,
                                item_id,
                                current_entry.shape_index,
                                tree_id,
                                |b, item, index, tree| r.new_obstacle_room(b, item, index, tree),
                            )
                            .map(RoomRef::Obstacle)
                        } else {
                            None
                        }
                    }
                };
                if let Some(neighbour_room) = neighbour_room
                    && insert_door_ok(
                        completed_room,
                        neighbour_room,
                        &TileShape::Box(intersection),
                        board,
                        rooms,
                    )
                {
                    let new_door = rooms
                        .new_door_from_shapes(completed_room, neighbour_room)
                        .unwrap_or_else(|| {
                            panic!(
                                "SortedOrthogonalRoomNeighbours.calculateNeighbours: a door room \
                                 has no shape (SortedOrthogonalRoomNeighbours.java:201) — Java \
                                 NPEs in the ExpansionDoor constructor here too"
                            )
                        });
                    rooms.add_door(neighbour_room, new_door);
                    rooms.add_door(completed_room, new_door);
                }
            }
        }
        Some(result)
    }

    fn add_sorted_neighbour(
        &mut self,
        search_tree_object: TreeObject,
        rooms: &ExpansionRoomStore,
        neighbour_shape: IntBox,
        intersection: IntBox,
    ) {
        let new_neighbour = SortedRoomNeighbour::new(
            search_tree_object,
            object_id(search_tree_object, rooms),
            neighbour_shape,
            intersection,
            &self.room_shape,
            &mut self.edge_interior_touches_obstacle,
        );
        self.sorted_neighbours.insert(new_neighbour);
    }

    fn try_remove_edge(
        &self,
        net_number: i32,
        board: &mut Board,
        rooms: &mut ExpansionRoomStore,
        tree_id: TreeId,
    ) -> bool {
        let RoomRef::Incomplete(incomplete_id) = self.from_room else {
            return false;
        };
        let Some(TileShape::Box(room_box)) = rooms.room_shape(self.from_room).cloned() else {
            return false;
        };
        let room_area = room_box.area();

        let mut remove_edge_no: i32 = -1;
        for i in 0..4 {
            if !self.edge_interior_touches_obstacle[i] {
                remove_edge_no = i as i32;
                break;
            }
        }
        if remove_edge_no < 0 {
            return false;
        }

        let enlarged_box = remove_border_line(&room_box, remove_edge_no);

        let mut ignore_shape: Option<TileShape> = None;
        let mut ignore_object: Option<TreeObject> = None;
        let mut max_door_area = 0.0f64;
        for door_id in rooms.room_doors(self.completed_room).to_vec() {
            let Some(door) = rooms.door(door_id) else {
                continue;
            };
            if door.dimension != 2 {
                continue;
            }
            let Some(RoomRef::Complete(other_room)) = door.other_room(self.completed_room) else {
                continue;
            };
            let Some(current_door_shape) = rooms.door_shape(door_id) else {
                continue;
            };
            let current_door_area = current_door_shape.area();
            if current_door_area > max_door_area {
                max_door_area = current_door_area;
                ignore_shape = Some(current_door_shape);
                ignore_object = Some(TreeObject::Room(other_room));
            }
        }

        let (layer, contained_shape) = {
            let r = rooms
                .incomplete_room(incomplete_id)
                .expect("the from room is in the arena");
            (r.get_layer(), r.get_contained_shape().cloned())
        };
        let enlarged_room = IncompleteFreeSpaceExpansionRoom::new(
            enlarged_box.map(TileShape::Box),
            layer,
            contained_shape,
        );
        let new_rooms = {
            let ctx = board.ctx();
            tree_of(board, tree_id).complete_shape(
                &enlarged_room,
                net_number,
                ignore_object,
                ignore_shape.as_ref(),
                &board.items,
                &*rooms,
                &ctx,
            )
        };
        if new_rooms.len() != 1 {
            return false;
        }
        let new_shape = new_rooms[0].get_shape().unwrap_or_else(|| {
            panic!(
                "SortedOrthogonalRoomNeighbours.tryRemoveEdge: completeShape answered a room with \
                 no shape (SortedOrthogonalRoomNeighbours.java:565) — Java NPEs here too"
            )
        });
        if new_shape.area() <= room_area {
            return false;
        }
        let (new_shape, new_contained) = {
            let new_room = &new_rooms[0];
            (
                new_room.get_shape().cloned(),
                new_room.get_contained_shape().cloned(),
            )
        };
        if let Some(r) = rooms.incomplete_room_mut(incomplete_id) {
            r.set_shape(new_shape);
            r.set_contained_shape(new_contained);
        }
        true
    }

    fn insert_incomplete_room(
        &self,
        board: &mut Board,
        rooms: &mut ExpansionRoomStore,
        ll_x: i32,
        ll_y: i32,
        ur_x: i32,
        ur_y: i32,
    ) {
        let new_incomplete_room_shape = IntBox::from_coords(ll_x, ll_y, ur_x, ur_y);
        if new_incomplete_room_shape.dimension() != 2 {
            return;
        }
        let new_contained_shape = self.room_shape.intersection(&new_incomplete_room_shape);
        if new_contained_shape.is_empty() {
            return;
        }
        let door_dimension = new_incomplete_room_shape
            .intersection(&self.room_shape)
            .dimension();
        if door_dimension <= 0 {
            return;
        }
        let layer = rooms
            .room_layer(board, self.from_room)
            .expect("the from room is in the arena");
        let new_room = RoomRef::Incomplete(rooms.new_incomplete_room(
            Some(TileShape::Box(new_incomplete_room_shape)),
            layer,
            Some(TileShape::Box(new_contained_shape)),
        ));
        let new_door = rooms.new_door(self.completed_room, new_room, door_dimension);
        rooms.add_door(self.completed_room, new_door);
        rooms.add_door(new_room, new_door);
    }

    pub fn calculate_new_incomplete_rooms(
        &self,
        board: &mut Board,
        rooms: &mut ExpansionRoomStore,
    ) {
        let board_bounds = board.bounding_box;
        let neighbours: Vec<&SortedRoomNeighbour> = self.sorted_neighbours.iter().collect();
        let Some(&last) = neighbours.last() else {
            panic!(
                "SortedOrthogonalRoomNeighbours.calculateNewIncompleteRooms: the neighbour set is \
                 empty (SortedOrthogonalRoomNeighbours.java:226) — Java throws \
                 NoSuchElementException here too"
            )
        };
        let mut prev_neighbour = last;

        for next_neighbour in neighbours.iter().copied() {
            if !next_neighbour
                .intersection
                .intersects(&prev_neighbour.intersection)
            {
                let prev = &prev_neighbour.intersection;
                let next = &next_neighbour.intersection;
                match next_neighbour.first_touching_side {
                    0 => {
                        if prev_neighbour.last_touching_side == 0 {
                            if prev.ur.x < next.ll.x {
                                self.insert_incomplete_room(
                                    board,
                                    rooms,
                                    prev.ur.x,
                                    board_bounds.ll.y,
                                    next.ll.x,
                                    self.room_shape.ll.y,
                                );
                            }
                        } else if prev.ll.y > self.room_shape.ll.y
                            || next.ll.x > self.room_shape.ll.x
                        {
                            if self.is_obstacle_expansion_room {
                                if prev_neighbour.last_touching_side == 3 {
                                    self.insert_incomplete_room(
                                        board,
                                        rooms,
                                        board_bounds.ll.x,
                                        self.room_shape.ll.y,
                                        self.room_shape.ll.x,
                                        prev.ll.y,
                                    );
                                }
                                self.insert_incomplete_room(
                                    board,
                                    rooms,
                                    self.room_shape.ll.x,
                                    board_bounds.ll.y,
                                    next.ll.x,
                                    self.room_shape.ll.y,
                                );
                            } else {
                                self.insert_incomplete_room(
                                    board,
                                    rooms,
                                    board_bounds.ll.x,
                                    board_bounds.ll.y,
                                    next.ll.x,
                                    prev.ll.y,
                                );
                            }
                        }
                    }
                    1 => {
                        if prev_neighbour.last_touching_side == 1 {
                            if prev.ur.y < next.ll.y {
                                self.insert_incomplete_room(
                                    board,
                                    rooms,
                                    self.room_shape.ur.x,
                                    prev.ur.y,
                                    board_bounds.ur.x,
                                    next.ll.y,
                                );
                            }
                        } else if prev.ur.x < self.room_shape.ur.x
                            || next.ll.y > self.room_shape.ll.y
                        {
                            if self.is_obstacle_expansion_room {
                                if prev_neighbour.last_touching_side == 0 {
                                    self.insert_incomplete_room(
                                        board,
                                        rooms,
                                        prev.ur.x,
                                        board_bounds.ll.y,
                                        self.room_shape.ur.x,
                                        self.room_shape.ll.y,
                                    );
                                }
                                self.insert_incomplete_room(
                                    board,
                                    rooms,
                                    self.room_shape.ur.x,
                                    self.room_shape.ll.y,
                                    self.room_shape.ur.x,
                                    next.ll.y,
                                );
                            } else {
                                self.insert_incomplete_room(
                                    board,
                                    rooms,
                                    prev.ur.x,
                                    board_bounds.ll.y,
                                    board_bounds.ur.x,
                                    next.ll.y,
                                );
                            }
                        }
                    }
                    2 => {
                        if prev_neighbour.last_touching_side == 2 {
                            if prev.ll.x > next.ur.x {
                                self.insert_incomplete_room(
                                    board,
                                    rooms,
                                    next.ur.x,
                                    self.room_shape.ur.y,
                                    prev.ll.x,
                                    board_bounds.ur.y,
                                );
                            }
                        } else if prev.ur.y < self.room_shape.ur.y
                            || next.ur.x < self.room_shape.ur.x
                        {
                            if self.is_obstacle_expansion_room {
                                if prev_neighbour.last_touching_side == 1 {
                                    self.insert_incomplete_room(
                                        board,
                                        rooms,
                                        self.room_shape.ur.x,
                                        prev.ur.y,
                                        board_bounds.ur.x,
                                        self.room_shape.ur.y,
                                    );
                                }
                                self.insert_incomplete_room(
                                    board,
                                    rooms,
                                    next.ur.x,
                                    self.room_shape.ur.y,
                                    self.room_shape.ur.x,
                                    board_bounds.ur.y,
                                );
                            } else {
                                self.insert_incomplete_room(
                                    board,
                                    rooms,
                                    next.ur.x,
                                    prev.ur.y,
                                    board_bounds.ur.x,
                                    board_bounds.ur.y,
                                );
                            }
                        }
                    }
                    3 => {
                        if prev_neighbour.last_touching_side == 3 {
                            if prev.ll.y > next.ur.y {
                                self.insert_incomplete_room(
                                    board,
                                    rooms,
                                    board_bounds.ll.x,
                                    next.ur.y,
                                    self.room_shape.ll.x,
                                    prev.ll.y,
                                );
                            }
                        } else if next.ur.y < self.room_shape.ur.y
                            || prev.ll.x > self.room_shape.ll.x
                        {
                            if self.is_obstacle_expansion_room {
                                if prev_neighbour.last_touching_side == 2 {
                                    self.insert_incomplete_room(
                                        board,
                                        rooms,
                                        self.room_shape.ll.x,
                                        self.room_shape.ur.y,
                                        prev.ll.x,
                                        board_bounds.ur.y,
                                    );
                                }
                                self.insert_incomplete_room(
                                    board,
                                    rooms,
                                    board_bounds.ll.x,
                                    next.ur.y,
                                    self.room_shape.ll.x,
                                    self.room_shape.ur.y,
                                );
                            } else {
                                self.insert_incomplete_room(
                                    board,
                                    rooms,
                                    board_bounds.ll.x,
                                    next.ur.y,
                                    prev.ll.x,
                                    board_bounds.ur.y,
                                );
                            }
                        }
                    }
                    _ => {}
                }
            }
            prev_neighbour = next_neighbour;
        }
    }
}

fn calculate_incomplete_rooms_with_empty_neighbours(
    room: RoomRef,
    board: &mut Board,
    rooms: &mut ExpansionRoomStore,
) {
    let Some(TileShape::Box(room_box)) = rooms.room_shape(room).cloned() else {
        return;
    };
    let bounding_box = board.get_bounding_box();
    let layer = rooms
        .room_layer(board, room)
        .expect("the obstacle room is in the arena");
    for i in 0..4 {
        let new_room_box = match i {
            0 => IntBox::from_coords(
                bounding_box.ll.x,
                bounding_box.ll.y,
                bounding_box.ur.x,
                room_box.ll.y,
            ),
            1 => IntBox::from_coords(
                room_box.ur.x,
                bounding_box.ll.y,
                bounding_box.ur.x,
                bounding_box.ur.y,
            ),
            2 => IntBox::from_coords(
                bounding_box.ll.x,
                room_box.ur.y,
                bounding_box.ur.x,
                bounding_box.ur.y,
            ),
            _ => IntBox::from_coords(
                bounding_box.ll.x,
                bounding_box.ll.y,
                room_box.ll.x,
                bounding_box.ur.y,
            ),
        };
        let new_contained_box = room_box.intersection(&new_room_box);
        let new_room = RoomRef::Incomplete(rooms.new_incomplete_room(
            Some(TileShape::Box(new_room_box)),
            layer,
            Some(TileShape::Box(new_contained_box)),
        ));
        let new_door = rooms.new_door(room, new_room, 1);
        rooms.add_door(room, new_door);
        rooms.add_door(new_room, new_door);
    }
}

fn remove_border_line(room_box: &IntBox, remove_edge_no: i32) -> Option<IntBox> {
    match remove_edge_no {
        0 => Some(IntBox::from_coords(
            room_box.ll.x,
            -CRIT_INT,
            room_box.ur.x,
            room_box.ur.y,
        )),
        1 => Some(IntBox::from_coords(
            room_box.ll.x,
            room_box.ll.y,
            CRIT_INT,
            room_box.ur.y,
        )),
        2 => Some(IntBox::from_coords(
            room_box.ll.x,
            room_box.ll.y,
            room_box.ur.x,
            CRIT_INT,
        )),
        3 => Some(IntBox::from_coords(
            -CRIT_INT,
            room_box.ll.y,
            room_box.ur.x,
            room_box.ur.y,
        )),
        _ => None,
    }
}

#[derive(Debug, Clone)]
pub struct SortedRoomNeighbour {
    pub search_tree_object: TreeObject,
    pub object_id: i32,
    pub shape: IntBox,
    pub intersection: IntBox,
    pub first_touching_side: i32,
    pub last_touching_side: i32,
}

impl SortedRoomNeighbour {
    pub fn new(
        search_tree_object: TreeObject,
        object_id: i32,
        neighbour_shape: IntBox,
        intersection: IntBox,
        room_shape: &IntBox,
        edge_interior_touches_obstacle: &mut [bool; 4],
    ) -> SortedRoomNeighbour {
        if intersection.ll.y == room_shape.ll.y
            && intersection.ur.x > room_shape.ll.x
            && intersection.ll.x < room_shape.ur.x
        {
            edge_interior_touches_obstacle[0] = true;
        }
        if intersection.ur.x == room_shape.ur.x
            && intersection.ur.y > room_shape.ll.y
            && intersection.ll.y < room_shape.ur.y
        {
            edge_interior_touches_obstacle[1] = true;
        }
        if intersection.ur.y == room_shape.ur.y
            && intersection.ur.x > room_shape.ll.x
            && intersection.ll.x < room_shape.ur.x
        {
            edge_interior_touches_obstacle[2] = true;
        }
        if intersection.ll.x == room_shape.ll.x
            && intersection.ur.y > room_shape.ll.y
            && intersection.ll.y < room_shape.ur.y
        {
            edge_interior_touches_obstacle[3] = true;
        }

        let first_touching_side =
            if intersection.ll.y == room_shape.ll.y && intersection.ll.x > room_shape.ll.x {
                0
            } else if intersection.ur.x == room_shape.ur.x && intersection.ll.y > room_shape.ll.y {
                1
            } else if intersection.ur.y == room_shape.ur.y {
                2
            } else if intersection.ll.x == room_shape.ll.x {
                3
            } else {
                -1
            };

        let last_touching_side =
            if intersection.ll.x == room_shape.ll.x && intersection.ll.y > room_shape.ll.y {
                3
            } else if intersection.ur.y == room_shape.ur.y && intersection.ll.x > room_shape.ll.x {
                2
            } else if intersection.ur.x == room_shape.ur.x {
                1
            } else if intersection.ll.y == room_shape.ll.y {
                0
            } else {
                -1
            };

        SortedRoomNeighbour {
            search_tree_object,
            object_id,
            shape: neighbour_shape,
            intersection,
            first_touching_side,
            last_touching_side,
        }
    }

    pub fn compare_to(&self, other: &SortedRoomNeighbour) -> Ordering {
        if self.first_touching_side > other.first_touching_side {
            return Ordering::Greater;
        }
        if self.first_touching_side < other.first_touching_side {
            return Ordering::Less;
        }

        let is1 = &self.intersection;
        let is2 = &other.intersection;
        let mut cmp_value: i32 = match self.first_touching_side {
            0 => is1.ll.x.wrapping_sub(is2.ll.x),
            1 => is1.ll.y.wrapping_sub(is2.ll.y),
            2 => is2.ur.x.wrapping_sub(is1.ur.x),
            3 => is2.ur.y.wrapping_sub(is1.ur.y),
            _ => return Ordering::Equal,
        };

        if cmp_value == 0 {
            let this_touching_side_diff =
                (self.last_touching_side - self.first_touching_side + 4).rem_euclid(4);
            let other_touching_side_diff =
                (other.last_touching_side - other.first_touching_side + 4).rem_euclid(4);
            if this_touching_side_diff > other_touching_side_diff {
                return Ordering::Greater;
            }
            if this_touching_side_diff < other_touching_side_diff {
                return Ordering::Less;
            }

            cmp_value = match self.last_touching_side {
                0 => is1.ur.x.wrapping_sub(is2.ur.x),
                1 => is1.ur.y.wrapping_sub(is2.ur.y),
                2 => is2.ll.x.wrapping_sub(is1.ll.x),
                3 => is2.ll.y.wrapping_sub(is1.ll.y),
                _ => return Ordering::Equal,
            };
        }
        if cmp_value == 0 {
            cmp_value = self.object_id.wrapping_sub(other.object_id);
        }
        cmp_value.cmp(&0)
    }
}

impl PartialEq for SortedRoomNeighbour {
    fn eq(&self, other: &SortedRoomNeighbour) -> bool {
        self.compare_to(other) == Ordering::Equal
    }
}

impl Eq for SortedRoomNeighbour {}

impl PartialOrd for SortedRoomNeighbour {
    fn partial_cmp(&self, other: &SortedRoomNeighbour) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SortedRoomNeighbour {
    fn cmp(&self, other: &SortedRoomNeighbour) -> Ordering {
        self.compare_to(other)
    }
}
