use std::cmp::Ordering;

use copper_board::{Board, TreeId, TreeObject};
use copper_geometry::{CRIT_INT, IntOctagon, TileShape};

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
pub struct Sorted45DegreeRoomNeighbours {
    pub completed_room: RoomRef,
    pub sorted_neighbours: BTreeSet<SortedRoomNeighbour>,
    pub from_room: RoomRef,
    pub room_shape: IntOctagon,
    pub edge_interior_touches_obstacle: [bool; 8],
}

impl Sorted45DegreeRoomNeighbours {
    fn new(
        from_room: RoomRef,
        completed_room: RoomRef,
        room_shape: IntOctagon,
    ) -> Sorted45DegreeRoomNeighbours {
        Sorted45DegreeRoomNeighbours {
            completed_room,
            sorted_neighbours: BTreeSet::new(),
            from_room,
            room_shape,
            edge_interior_touches_obstacle: [false; 8],
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
            let room_neighbours = Sorted45DegreeRoomNeighbours::calculate_neighbours(
                room, net_number, board, rooms, tree_id, room_id_no,
            )?;

            let edge_removed =
                room_neighbours.try_remove_edge_line(net_number, board, rooms, tree_id);
            let result = room_neighbours.completed_room;
            if edge_removed {
                rooms.remove_all_doors(result);
                continue;
            }

            if room_neighbours.sorted_neighbours.is_empty() {
                if let RoomRef::Obstacle(_) = result {
                    room_neighbours.calculate_edge_incomplete_rooms_of_obstacle_expansion_room(
                        0, 7, board, rooms,
                    );
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
    ) -> Option<Sorted45DegreeRoomNeighbours> {
        let room_shape = rooms
            .room_shape(room)
            .unwrap_or_else(|| {
                panic!(
                    "Sorted45DegreeRoomNeighbours.calculateNeighbours: room {room:?} has no shape \
                     (Sorted45DegreeRoomNeighbours.java:87) — Java NPEs here too"
                )
            })
            .clone();
        let layer = rooms.room_layer(board, room).unwrap_or_else(|| {
            panic!(
                "Sorted45DegreeRoomNeighbours.calculateNeighbours: room {room:?} has no layer \
                 (Sorted45DegreeRoomNeighbours.java:90) — Java NPEs here too"
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

        let room_oct = bounding_octagon_of(&room_shape, 98);
        let completed_shape = rooms
            .room_shape(completed_room)
            .expect("the completed room was just built from a shape")
            .clone();
        let mut result = Sorted45DegreeRoomNeighbours::new(
            room,
            completed_room,
            bounding_octagon_of(&completed_shape, 35),
        );

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
            let current_oct = bounding_octagon_of(&current_shape, 129);
            let intersection = room_oct.intersection(&current_oct);
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
                            "Sorted45DegreeRoomNeighbours.calculateNeighbours: item {item_id} has \
                             no expansion room for shape {} \
                             (Sorted45DegreeRoomNeighbours.java:139) — Java NPEs in \
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

            result.add_sorted_neighbour(current_object, rooms, current_oct, intersection);

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
                        &TileShape::Octagon(intersection),
                        board,
                        rooms,
                    )
                {
                    let new_door = rooms
                        .new_door_from_shapes(completed_room, neighbour_room)
                        .unwrap_or_else(|| {
                            panic!(
                                "Sorted45DegreeRoomNeighbours.calculateNeighbours: a door room \
                                 has no shape (Sorted45DegreeRoomNeighbours.java:164) — Java \
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
        neighbour_shape: IntOctagon,
        intersection: IntOctagon,
    ) {
        let new_neighbour = SortedRoomNeighbour::new(
            search_tree_object,
            object_id(search_tree_object, rooms),
            neighbour_shape,
            intersection,
            &self.room_shape,
            &mut self.edge_interior_touches_obstacle,
        );
        if new_neighbour.last_touching_side >= 0 {
            self.sorted_neighbours.insert(new_neighbour);
        }
    }

    fn calculate_edge_incomplete_rooms_of_obstacle_expansion_room(
        &self,
        from_side_index: usize,
        to_side_index: usize,
        board: &mut Board,
        rooms: &mut ExpansionRoomStore,
    ) {
        if !matches!(self.from_room, RoomRef::Obstacle(_)) {
            return;
        }
        let board_bounding_oct = board.get_bounding_box().bounding_octagon();
        let mut current_corner = self.room_shape.corner(from_side_index);
        let mut current_side_index = from_side_index;
        loop {
            let next_side_no = (current_side_index + 1) % 8;
            let next_corner = self.room_shape.corner(next_side_no);
            if current_corner != next_corner {
                let mut left_x = board_bounding_oct.left_x;
                let mut bottom_y = board_bounding_oct.bottom_y;
                let mut right_x = board_bounding_oct.right_x;
                let mut top_y = board_bounding_oct.top_y;
                let mut upper_left_diagonal_x = board_bounding_oct.upper_left_diagonal_x;
                let mut lower_right_diagonal_x = board_bounding_oct.lower_right_diagonal_x;
                let mut lower_left_diagonal_x = board_bounding_oct.lower_left_diagonal_x;
                let mut upper_right_diagonal_x = board_bounding_oct.upper_right_diagonal_x;
                match current_side_index {
                    0 => top_y = self.room_shape.bottom_y,
                    1 => upper_left_diagonal_x = self.room_shape.lower_right_diagonal_x,
                    2 => left_x = self.room_shape.right_x,
                    3 => lower_left_diagonal_x = self.room_shape.upper_right_diagonal_x,
                    4 => bottom_y = self.room_shape.top_y,
                    5 => lower_right_diagonal_x = self.room_shape.upper_left_diagonal_x,
                    6 => right_x = self.room_shape.left_x,
                    _ => upper_right_diagonal_x = self.room_shape.lower_left_diagonal_x,
                }
                self.insert_incomplete_room(
                    board,
                    rooms,
                    left_x,
                    bottom_y,
                    right_x,
                    top_y,
                    upper_left_diagonal_x,
                    lower_right_diagonal_x,
                    lower_left_diagonal_x,
                    upper_right_diagonal_x,
                );
            }
            if current_side_index == to_side_index {
                break;
            }
            current_side_index = next_side_no;
            current_corner = next_corner;
        }
    }

    fn try_remove_edge_line(
        &self,
        net_number: i32,
        board: &mut Board,
        rooms: &mut ExpansionRoomStore,
        tree_id: TreeId,
    ) -> bool {
        let RoomRef::Incomplete(incomplete_id) = self.from_room else {
            return false;
        };
        let Some(TileShape::Octagon(room_oct)) = rooms.room_shape(self.from_room).cloned() else {
            return false;
        };
        let room_area = room_oct.area();

        let mut try_remove_edge_lines = false;
        for i in 0..8 {
            if !self.edge_interior_touches_obstacle[i] {
                let prev_corner = self.room_shape.corner(i).to_float();
                let next_corner = self.room_shape.corner((i + 1) % 8).to_float();
                if prev_corner.distance_square(&next_corner) > 1.0 {
                    try_remove_edge_lines = true;
                    break;
                }
            }
        }

        if !try_remove_edge_lines {
            return false;
        }
        let enlarged_oct =
            remove_not_touching_border_lines(&room_oct, &self.edge_interior_touches_obstacle);

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
            Some(TileShape::Octagon(enlarged_oct)),
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
                "Sorted45DegreeRoomNeighbours.tryRemoveEdgeLine: completeShape answered a room \
                 with no shape (Sorted45DegreeRoomNeighbours.java:413) — Java NPEs here too"
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

    #[allow(clippy::too_many_arguments)]
    fn insert_incomplete_room(
        &self,
        board: &mut Board,
        rooms: &mut ExpansionRoomStore,
        left_x: i32,
        bottom_y: i32,
        right_x: i32,
        top_y: i32,
        upper_left_diagonal_x: i32,
        lower_right_diagonal_x: i32,
        lower_left_diagonal_x: i32,
        upper_right_diagonal_x: i32,
    ) {
        let new_incomplete_room_shape = IntOctagon::new(
            left_x,
            bottom_y,
            right_x,
            top_y,
            upper_left_diagonal_x,
            lower_right_diagonal_x,
            lower_left_diagonal_x,
            upper_right_diagonal_x,
        )
        .normalize();
        if new_incomplete_room_shape.dimension() != 2 {
            return;
        }
        let new_contained_shape = self.room_shape.intersection(&new_incomplete_room_shape);
        if new_contained_shape.is_empty() {
            return;
        }
        let door_dimension = new_contained_shape.dimension();
        if door_dimension <= 0 {
            return;
        }
        let layer = rooms
            .room_layer(board, self.from_room)
            .expect("the from room is in the arena");
        let new_room = RoomRef::Incomplete(rooms.new_incomplete_room(
            Some(TileShape::Octagon(new_incomplete_room_shape)),
            layer,
            Some(TileShape::Octagon(new_contained_shape)),
        ));
        let new_door = rooms.new_door(self.completed_room, new_room, door_dimension);
        rooms.add_door(self.completed_room, new_door);
        rooms.add_door(new_room, new_door);
    }

    fn calculate_new_incomplete_rooms_for_obstacle_expansion_room(
        &self,
        prev_neighbour: &SortedRoomNeighbour,
        next_neighbour: &SortedRoomNeighbour,
        prev_is_next: bool,
        board: &mut Board,
        rooms: &mut ExpansionRoomStore,
    ) {
        let from_side_index = prev_neighbour.last_touching_side;
        let to_side_index = next_neighbour.first_touching_side;
        if from_side_index == to_side_index && !prev_is_next {
            return;
        }
        let board_bounding_oct = board.bounding_box.bounding_octagon();

        let mut left_x = board_bounding_oct.left_x;
        let mut bottom_y = board_bounding_oct.bottom_y;
        let mut right_x = board_bounding_oct.right_x;
        let mut top_y = board_bounding_oct.top_y;
        let mut upper_left_diagonal_x = board_bounding_oct.upper_left_diagonal_x;
        let mut lower_right_diagonal_x = board_bounding_oct.lower_right_diagonal_x;
        let mut lower_left_diagonal_x = board_bounding_oct.lower_left_diagonal_x;
        let mut upper_right_diagonal_x = board_bounding_oct.upper_right_diagonal_x;
        let prev = &prev_neighbour.intersection;
        match from_side_index {
            0 => {
                top_y = self.room_shape.bottom_y;
                upper_left_diagonal_x = prev.lower_right_diagonal_x;
            }
            1 => {
                upper_left_diagonal_x = self.room_shape.lower_right_diagonal_x;
                left_x = prev.right_x;
            }
            2 => {
                left_x = self.room_shape.right_x;
                lower_left_diagonal_x = prev.upper_right_diagonal_x;
            }
            3 => {
                lower_left_diagonal_x = self.room_shape.upper_right_diagonal_x;
                bottom_y = prev.top_y;
            }
            4 => {
                bottom_y = self.room_shape.top_y;
                lower_right_diagonal_x = prev.upper_left_diagonal_x;
            }
            5 => {
                lower_right_diagonal_x = self.room_shape.upper_left_diagonal_x;
                right_x = prev.left_x;
            }
            6 => {
                right_x = self.room_shape.left_x;
                upper_right_diagonal_x = prev.lower_left_diagonal_x;
            }
            7 => {
                upper_right_diagonal_x = self.room_shape.lower_left_diagonal_x;
                top_y = prev.bottom_y;
            }
            _ => {}
        }
        self.insert_incomplete_room(
            board,
            rooms,
            left_x,
            bottom_y,
            right_x,
            top_y,
            upper_left_diagonal_x,
            lower_right_diagonal_x,
            lower_left_diagonal_x,
            upper_right_diagonal_x,
        );

        let mut left_x = board_bounding_oct.left_x;
        let mut bottom_y = board_bounding_oct.bottom_y;
        let mut right_x = board_bounding_oct.right_x;
        let mut top_y = board_bounding_oct.top_y;
        let mut upper_left_diagonal_x = board_bounding_oct.upper_left_diagonal_x;
        let mut lower_right_diagonal_x = board_bounding_oct.lower_right_diagonal_x;
        let mut lower_left_diagonal_x = board_bounding_oct.lower_left_diagonal_x;
        let mut upper_right_diagonal_x = board_bounding_oct.upper_right_diagonal_x;
        let next = &next_neighbour.intersection;
        match to_side_index {
            0 => {
                top_y = self.room_shape.bottom_y;
                upper_right_diagonal_x = next.lower_left_diagonal_x;
            }
            1 => {
                upper_left_diagonal_x = self.room_shape.lower_right_diagonal_x;
                top_y = next.bottom_y;
            }
            2 => {
                left_x = self.room_shape.right_x;
                upper_left_diagonal_x = next.lower_right_diagonal_x;
            }
            3 => {
                lower_left_diagonal_x = self.room_shape.upper_right_diagonal_x;
                left_x = next.right_x;
            }
            4 => {
                bottom_y = self.room_shape.top_y;
                lower_left_diagonal_x = next.upper_right_diagonal_x;
            }
            5 => {
                lower_right_diagonal_x = self.room_shape.upper_left_diagonal_x;
                bottom_y = next.top_y;
            }
            6 => {
                right_x = self.room_shape.left_x;
                lower_right_diagonal_x = next.upper_left_diagonal_x;
            }
            7 => {
                upper_right_diagonal_x = self.room_shape.lower_left_diagonal_x;
                right_x = next.left_x;
            }
            _ => {}
        }
        self.insert_incomplete_room(
            board,
            rooms,
            left_x,
            bottom_y,
            right_x,
            top_y,
            upper_left_diagonal_x,
            lower_right_diagonal_x,
            lower_left_diagonal_x,
            upper_right_diagonal_x,
        );

        let current_from_side_no = (from_side_index + 1).rem_euclid(8);
        if current_from_side_no == to_side_index {
            return;
        }
        let current_to_side_no = (to_side_index + 7).rem_euclid(8);
        self.calculate_edge_incomplete_rooms_of_obstacle_expansion_room(
            side_index(current_from_side_no, 607),
            side_index(current_to_side_no, 608),
            board,
            rooms,
        );
    }

    pub fn calculate_new_incomplete_rooms(
        &self,
        board: &mut Board,
        rooms: &mut ExpansionRoomStore,
    ) {
        let board_bounding_oct = board.bounding_box.bounding_octagon();
        let neighbours: Vec<&SortedRoomNeighbour> = self.sorted_neighbours.iter().collect();
        let Some(&last) = neighbours.last() else {
            panic!(
                "Sorted45DegreeRoomNeighbours.calculateNewIncompleteRooms: the neighbour set is \
                 empty (Sorted45DegreeRoomNeighbours.java:613) — Java throws \
                 NoSuchElementException here too"
            )
        };
        let mut prev_neighbour = last;

        if matches!(self.from_room, RoomRef::Obstacle(_)) && neighbours.len() == 1 {
            self.calculate_new_incomplete_rooms_for_obstacle_expansion_room(
                prev_neighbour,
                prev_neighbour,
                true,
                board,
                rooms,
            );
            return;
        }

        for next_neighbour in neighbours.iter().copied() {
            let insert_incomplete_room;

            if matches!(self.completed_room, RoomRef::Obstacle(_)) && neighbours.len() == 2 {
                let intersection = next_neighbour
                    .intersection
                    .intersection(&prev_neighbour.intersection);
                if intersection.is_empty() {
                    insert_incomplete_room = true;
                } else if intersection.dimension() >= 1 {
                    insert_incomplete_room = false;
                } else {
                    if prev_neighbour.last_touching_side == next_neighbour.first_touching_side {
                        insert_incomplete_room = false;
                    } else {
                        insert_incomplete_room = prev_neighbour.last_touching_side
                            != (next_neighbour.first_touching_side + 1).rem_euclid(8);
                    }
                }
            } else {
                insert_incomplete_room = !next_neighbour
                    .intersection
                    .intersects_octagon(&prev_neighbour.intersection);
            }

            if insert_incomplete_room {
                if matches!(self.from_room, RoomRef::Obstacle(_))
                    && next_neighbour.first_touching_side != prev_neighbour.last_touching_side
                {
                    self.calculate_new_incomplete_rooms_for_obstacle_expansion_room(
                        prev_neighbour,
                        next_neighbour,
                        false,
                        board,
                        rooms,
                    );
                } else {
                    let mut lx = board_bounding_oct.left_x;
                    let mut ly = board_bounding_oct.bottom_y;
                    let mut rx = board_bounding_oct.right_x;
                    let mut uy = board_bounding_oct.top_y;
                    let mut ulx = board_bounding_oct.upper_left_diagonal_x;
                    let mut lrx = board_bounding_oct.lower_right_diagonal_x;
                    let mut llx = board_bounding_oct.lower_left_diagonal_x;
                    let mut urx = board_bounding_oct.upper_right_diagonal_x;
                    let prev = &prev_neighbour.intersection;
                    let next = &next_neighbour.intersection;

                    match next_neighbour.first_touching_side {
                        0 => {
                            if prev.lower_left_diagonal_x < next.lower_left_diagonal_x {
                                urx = next.lower_left_diagonal_x;
                                uy = prev.bottom_y;
                                if prev_neighbour.last_touching_side == 0 {
                                    ulx = prev.lower_right_diagonal_x;
                                }
                            } else if prev.lower_left_diagonal_x > next.lower_left_diagonal_x {
                                rx = next.left_x;
                                urx = prev.lower_left_diagonal_x;
                            } else {
                                urx = next.lower_left_diagonal_x;
                            }
                        }
                        1 => {
                            if prev.bottom_y < next.bottom_y {
                                uy = next.bottom_y;
                                ulx = prev.lower_right_diagonal_x;
                                if prev_neighbour.last_touching_side == 1 {
                                    lx = prev.right_x;
                                }
                            } else if prev.bottom_y > next.bottom_y {
                                uy = prev.bottom_y;
                                urx = next.lower_left_diagonal_x;
                            } else {
                                uy = next.bottom_y;
                            }
                        }
                        2 => {
                            if prev.lower_right_diagonal_x > next.lower_right_diagonal_x {
                                ulx = next.lower_right_diagonal_x;
                                lx = prev.right_x;
                                if prev_neighbour.last_touching_side == 2 {
                                    llx = prev.upper_right_diagonal_x;
                                }
                            } else if prev.lower_right_diagonal_x < next.lower_right_diagonal_x {
                                uy = next.bottom_y;
                                ulx = prev.lower_right_diagonal_x;
                            } else {
                                ulx = next.lower_right_diagonal_x;
                            }
                        }
                        3 => {
                            if prev.right_x > next.right_x {
                                lx = next.right_x;
                                llx = prev.upper_right_diagonal_x;
                                if prev_neighbour.last_touching_side == 3 {
                                    ly = prev.top_y;
                                }
                            } else if prev.right_x < next.right_x {
                                lx = prev.right_x;
                                ulx = next.lower_right_diagonal_x;
                            } else {
                                lx = next.right_x;
                            }
                        }
                        4 => {
                            if prev.upper_right_diagonal_x > next.upper_right_diagonal_x {
                                llx = next.upper_right_diagonal_x;
                                ly = prev.top_y;
                                if prev_neighbour.last_touching_side == 4 {
                                    lrx = prev.upper_left_diagonal_x;
                                }
                            } else if prev.upper_right_diagonal_x < next.upper_right_diagonal_x {
                                lx = next.right_x;
                                llx = prev.upper_right_diagonal_x;
                            } else {
                                llx = next.upper_right_diagonal_x;
                            }
                        }
                        5 => {
                            if prev.top_y > next.top_y {
                                ly = next.top_y;
                                lrx = prev.upper_left_diagonal_x;
                                if prev_neighbour.last_touching_side == 5 {
                                    rx = prev.left_x;
                                }
                            } else if prev.top_y < next.top_y {
                                ly = prev.top_y;
                                llx = next.upper_right_diagonal_x;
                            } else {
                                ly = next.top_y;
                            }
                        }
                        6 => {
                            if prev.upper_left_diagonal_x < next.upper_left_diagonal_x {
                                lrx = next.upper_left_diagonal_x;
                                rx = prev.left_x;
                                if prev_neighbour.last_touching_side == 6 {
                                    urx = prev.lower_left_diagonal_x;
                                }
                            } else if prev.upper_left_diagonal_x > next.upper_left_diagonal_x {
                                ly = next.top_y;
                                lrx = prev.upper_left_diagonal_x;
                            } else {
                                lrx = next.upper_left_diagonal_x;
                            }
                        }
                        7 => {
                            if prev.left_x < next.left_x {
                                rx = next.left_x;
                                urx = prev.lower_left_diagonal_x;
                                if prev_neighbour.last_touching_side == 7 {
                                    uy = prev.bottom_y;
                                }
                            } else if prev.left_x > next.left_x {
                                rx = prev.left_x;
                                lrx = next.upper_left_diagonal_x;
                            } else {
                                rx = next.left_x;
                            }
                        }
                        _ => {}
                    }
                    self.insert_incomplete_room(board, rooms, lx, ly, rx, uy, ulx, lrx, llx, urx);
                }
            }
            prev_neighbour = next_neighbour;
        }
    }
}

fn remove_not_touching_border_lines(
    room_oct: &IntOctagon,
    edge_interior_touches_obstacle: &[bool; 8],
) -> IntOctagon {
    let pick = |touched: bool, value: i32, fallback: i32| if touched { value } else { fallback };
    let left_x = pick(
        edge_interior_touches_obstacle[6],
        room_oct.left_x,
        -CRIT_INT,
    );
    let bottom_y = pick(
        edge_interior_touches_obstacle[0],
        room_oct.bottom_y,
        -CRIT_INT,
    );
    let right_x = pick(
        edge_interior_touches_obstacle[2],
        room_oct.right_x,
        CRIT_INT,
    );
    let top_y = pick(edge_interior_touches_obstacle[4], room_oct.top_y, CRIT_INT);
    let upper_left_diagonal_x = pick(
        edge_interior_touches_obstacle[5],
        room_oct.upper_left_diagonal_x,
        -CRIT_INT,
    );
    let lower_right_diagonal_x = pick(
        edge_interior_touches_obstacle[1],
        room_oct.lower_right_diagonal_x,
        CRIT_INT,
    );
    let lower_left_diagonal_x = pick(
        edge_interior_touches_obstacle[7],
        room_oct.lower_left_diagonal_x,
        -CRIT_INT,
    );
    let upper_right_diagonal_x = pick(
        edge_interior_touches_obstacle[3],
        room_oct.upper_right_diagonal_x,
        CRIT_INT,
    );
    IntOctagon::new(
        left_x,
        bottom_y,
        right_x,
        top_y,
        upper_left_diagonal_x,
        lower_right_diagonal_x,
        lower_left_diagonal_x,
        upper_right_diagonal_x,
    )
    .normalize()
}

fn bounding_octagon_of(shape: &TileShape, source_line: u32) -> IntOctagon {
    shape.bounding_octagon().unwrap_or_else(|| {
        panic!(
            "Sorted45DegreeRoomNeighbours:{source_line}: boundingOctagon answered null for an \
             unbounded shape — Java NPEs here too"
        )
    })
}

fn side_index(no: i32, source_line: u32) -> usize {
    usize::try_from(no).unwrap_or_else(|_| {
        panic!(
            "Sorted45DegreeRoomNeighbours:{source_line}: the touching side is {no} — Java \
             throws ArrayIndexOutOfBoundsException here"
        )
    })
}

#[derive(Debug, Clone)]
pub struct SortedRoomNeighbour {
    pub search_tree_object: TreeObject,
    pub object_id: i32,
    pub shape: IntOctagon,
    pub intersection: IntOctagon,
    pub first_touching_side: i32,
    pub last_touching_side: i32,
}

impl SortedRoomNeighbour {
    pub fn new(
        search_tree_object: TreeObject,
        object_id: i32,
        neighbour_shape: IntOctagon,
        intersection: IntOctagon,
        room_shape: &IntOctagon,
        edge_interior_touches_obstacle: &mut [bool; 8],
    ) -> SortedRoomNeighbour {
        let first_touching_side = if intersection.bottom_y == room_shape.bottom_y
            && intersection.lower_left_diagonal_x > room_shape.lower_left_diagonal_x
        {
            0
        } else if intersection.lower_right_diagonal_x == room_shape.lower_right_diagonal_x
            && intersection.bottom_y > room_shape.bottom_y
        {
            1
        } else if intersection.right_x == room_shape.right_x
            && intersection.lower_right_diagonal_x < room_shape.lower_right_diagonal_x
        {
            2
        } else if intersection.upper_right_diagonal_x == room_shape.upper_right_diagonal_x
            && intersection.right_x < room_shape.right_x
        {
            3
        } else if intersection.top_y == room_shape.top_y
            && intersection.upper_right_diagonal_x < room_shape.upper_right_diagonal_x
        {
            4
        } else if intersection.upper_left_diagonal_x == room_shape.upper_left_diagonal_x
            && intersection.top_y < room_shape.top_y
        {
            5
        } else if intersection.left_x == room_shape.left_x
            && intersection.upper_left_diagonal_x > room_shape.upper_left_diagonal_x
        {
            6
        } else if intersection.lower_left_diagonal_x == room_shape.lower_left_diagonal_x
            && intersection.left_x > room_shape.left_x
        {
            7
        } else {
            return SortedRoomNeighbour {
                search_tree_object,
                object_id,
                shape: neighbour_shape,
                intersection,
                first_touching_side: -1,
                last_touching_side: -1,
            };
        };

        let last_touching_side = if intersection.lower_left_diagonal_x
            == room_shape.lower_left_diagonal_x
            && intersection.bottom_y > room_shape.bottom_y
        {
            7
        } else if intersection.left_x == room_shape.left_x
            && intersection.lower_left_diagonal_x > room_shape.lower_left_diagonal_x
        {
            6
        } else if intersection.upper_left_diagonal_x == room_shape.upper_left_diagonal_x
            && intersection.left_x > room_shape.left_x
        {
            5
        } else if intersection.top_y == room_shape.top_y
            && intersection.upper_left_diagonal_x > room_shape.upper_left_diagonal_x
        {
            4
        } else if intersection.upper_right_diagonal_x == room_shape.upper_right_diagonal_x
            && intersection.top_y < room_shape.top_y
        {
            3
        } else if intersection.right_x == room_shape.right_x
            && intersection.upper_right_diagonal_x < room_shape.upper_right_diagonal_x
        {
            2
        } else if intersection.lower_right_diagonal_x == room_shape.lower_right_diagonal_x
            && intersection.right_x < room_shape.right_x
        {
            1
        } else if intersection.bottom_y == room_shape.bottom_y
            && intersection.lower_right_diagonal_x < room_shape.lower_right_diagonal_x
        {
            0
        } else {
            return SortedRoomNeighbour {
                search_tree_object,
                object_id,
                shape: neighbour_shape,
                intersection,
                first_touching_side,
                last_touching_side: -1,
            };
        };

        let mut next_side_no = first_touching_side as usize;
        loop {
            let current_side_index = next_side_no;
            next_side_no = (next_side_no + 1) % 8;
            if !edge_interior_touches_obstacle[current_side_index] {
                let mut touch_only_at_corner = false;
                if current_side_index as i32 == first_touching_side
                    && intersection.corner(current_side_index) == room_shape.corner(next_side_no)
                {
                    touch_only_at_corner = true;
                }
                if current_side_index as i32 == last_touching_side
                    && intersection.corner(next_side_no) == room_shape.corner(current_side_index)
                {
                    touch_only_at_corner = true;
                }
                if !touch_only_at_corner {
                    edge_interior_touches_obstacle[current_side_index] = true;
                }
            }
            if current_side_index as i32 == last_touching_side {
                break;
            }
        }

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
        let corner_x = |oct: &IntOctagon, no: usize| oct.corner(no).x;
        let corner_y = |oct: &IntOctagon, no: usize| oct.corner(no).y;
        let mut cmp_value: i32 = match self.first_touching_side {
            0 => corner_x(is1, 0).wrapping_sub(corner_x(is2, 0)),
            1 => corner_x(is1, 1).wrapping_sub(corner_x(is2, 1)),
            2 => corner_y(is1, 2).wrapping_sub(corner_y(is2, 2)),
            3 => corner_y(is1, 3).wrapping_sub(corner_y(is2, 3)),
            4 => corner_x(is2, 4).wrapping_sub(corner_x(is1, 4)),
            5 => corner_x(is2, 5).wrapping_sub(corner_x(is1, 5)),
            6 => corner_y(is2, 6).wrapping_sub(corner_y(is1, 6)),
            7 => corner_y(is2, 7).wrapping_sub(corner_y(is1, 7)),
            _ => return Ordering::Equal,
        };

        if cmp_value == 0 {
            let this_touching_side_diff =
                (self.last_touching_side - self.first_touching_side + 8).rem_euclid(8);
            let other_touching_side_diff =
                (other.last_touching_side - other.first_touching_side + 8).rem_euclid(8);
            if this_touching_side_diff > other_touching_side_diff {
                return Ordering::Greater;
            }
            if this_touching_side_diff < other_touching_side_diff {
                return Ordering::Less;
            }
            cmp_value = match self.last_touching_side {
                0 => corner_x(is1, 1).wrapping_sub(corner_x(is2, 1)),
                1 => corner_x(is1, 2).wrapping_sub(corner_x(is2, 2)),
                2 => corner_y(is1, 3).wrapping_sub(corner_y(is2, 3)),
                3 => corner_y(is1, 4).wrapping_sub(corner_y(is2, 4)),
                4 => corner_x(is2, 5).wrapping_sub(corner_x(is1, 5)),
                5 => corner_x(is2, 6).wrapping_sub(corner_x(is1, 6)),
                6 => corner_y(is2, 7).wrapping_sub(corner_y(is1, 7)),
                7 => corner_y(is2, 0).wrapping_sub(corner_y(is1, 0)),
                _ => 0,
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
