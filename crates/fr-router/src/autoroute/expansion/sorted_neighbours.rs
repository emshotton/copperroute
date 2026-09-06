use std::cell::OnceCell;
use std::cmp::Ordering;

use fr_board::datastructures::TreeEntry;
use fr_board::searchtree::ShapeSearchTree;
use fr_board::{
    AngleRestriction, Board, Item, ItemCtx, ItemLookup, ObstacleRoomId, RoomId, RoomLookup, TreeId,
    TreeObject,
};
use fr_geometry::polyline_shape::PolylineShapeOps;
use fr_geometry::{Line, Point, Side, Simplex, TileShape};

use crate::autoroute::expansion::sorted_neighbours_45::Sorted45DegreeRoomNeighbours;
use crate::autoroute::expansion::sorted_neighbours_orthogonal::SortedOrthogonalRoomNeighbours;
use crate::autoroute::expansion::{ExpansionRoomStore, IncompleteFreeSpaceExpansionRoom, RoomRef};
use crate::autoroute::item_info;
use crate::autoroute::tree_ext::AutorouteSearchTreeExt;
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CalculationMode {
    Orthogonal,
    FortyFiveDegree,
    AnyAngle,
}

pub fn select_calculation_mode(tree: &ShapeSearchTree) -> CalculationMode {
    if tree.angle() == AngleRestriction::NinetyDegree {
        return CalculationMode::Orthogonal;
    }
    if tree.angle() == AngleRestriction::FortyFiveDegree {
        return CalculationMode::FortyFiveDegree;
    }
    CalculationMode::AnyAngle
}

#[derive(Debug, Clone)]
pub struct SortedRoomNeighbours {
    pub from_room: RoomRef,
    pub completed_room: RoomRef,
    pub room_shape: TileShape,
    pub sorted_neighbours: BTreeSet<SortedRoomNeighbour>,
    pub own_net_objects: Vec<TreeEntry<TreeObject>>,
}

impl SortedRoomNeighbours {
    pub fn complete(
        room: RoomRef,
        net_number: i32,
        board: &mut Board,
        rooms: &mut ExpansionRoomStore,
        tree_id: TreeId,
    ) -> Option<RoomRef> {
        match select_calculation_mode(tree_of(board, tree_id)) {
            CalculationMode::Orthogonal => {
                SortedOrthogonalRoomNeighbours::calculate(room, net_number, board, rooms, tree_id)
            }
            CalculationMode::FortyFiveDegree => {
                Sorted45DegreeRoomNeighbours::calculate(room, net_number, board, rooms, tree_id)
            }
            CalculationMode::AnyAngle => {
                SortedRoomNeighbours::calculate(room, net_number, board, rooms, tree_id)
            }
        }
    }

    pub fn calculate(
        room: RoomRef,
        net_number: i32,
        board: &mut Board,
        rooms: &mut ExpansionRoomStore,
        tree_id: TreeId,
    ) -> Option<RoomRef> {
        let room_id_no = rooms.next_room_id_no();
        loop {
            let room_neighbours = SortedRoomNeighbours::calculate_neighbours(
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
                    calculate_incomplete_rooms_with_empty_neighbours(room, board, rooms);
                }
            } else {
                room_neighbours.calculate_new_incomplete_rooms(board, rooms);
            }

            if let RoomRef::Complete(free_room) = result {
                calculate_target_doors(
                    free_room,
                    &room_neighbours.own_net_objects,
                    net_number,
                    board,
                    rooms,
                    tree_id,
                );
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
    ) -> Option<SortedRoomNeighbours> {
        let room_shape = rooms
            .room_shape(room)
            .unwrap_or_else(|| {
                panic!(
                    "SortedRoomNeighbours.calculateNeighbours: room {room:?} has no shape \
                     (SortedRoomNeighbours.java:189) — Java NPEs here too"
                )
            })
            .clone();
        let layer = rooms.room_layer(board, room).unwrap_or_else(|| {
            panic!(
                "SortedRoomNeighbours.calculateNeighbours: room {room:?} has no layer \
                 (SortedRoomNeighbours.java:192) — Java NPEs here too"
            )
        });
        let room_simplex = TileShape::Simplex(room_shape.to_simplex());

        let completed_room = match room {
            RoomRef::Incomplete(_) => RoomRef::Complete(rooms.new_complete_room(
                Some(room_shape.clone()),
                layer,
                room_id_no,
            )),
            RoomRef::Obstacle(id) => RoomRef::Obstacle(id),
            RoomRef::Complete(_) => return None,
        };

        let mut result = SortedRoomNeighbours {
            from_room: room,
            completed_room,
            room_shape: room_simplex.clone(),
            sorted_neighbours: BTreeSet::new(),
            own_net_objects: Vec::new(),
        };

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

            if matches!(room, RoomRef::Incomplete(_))
                && !object_is_trace_obstacle(current_object, net_number, &board.items)
            {
                result.own_net_objects.push(current_entry);
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
            let intersection = room_shape.intersection(&current_shape);
            let dimension = intersection.dimension();

            if dimension > 1 {
                if let (RoomRef::Obstacle(obstacle_room), TreeObject::Item(item_id)) =
                    (completed_room, current_object)
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
                            "SortedRoomNeighbours.calculateNeighbours: item {item_id} has no \
                             expansion room for shape {} (SortedRoomNeighbours.java:239) — Java \
                             NPEs in createOverlapDoor here too",
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

            if dimension == 1 {
                let Some(touching_sides) = room_simplex.touching_sides(&current_shape) else {
                    continue;
                };
                result.add_sorted_neighbour(SortedRoomNeighbour::new(
                    current_object,
                    object_id(current_object, rooms),
                    current_shape.clone(),
                    intersection.clone(),
                    touching_sides[0] as i32,
                    touching_sides[1] as i32,
                    false,
                    false,
                    room_simplex.clone(),
                ));

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
                    && insert_door_ok(completed_room, neighbour_room, &intersection, board, rooms)
                {
                    let new_door = rooms.new_door(completed_room, neighbour_room, 1);
                    rooms.add_door(neighbour_room, new_door);
                    rooms.add_door(completed_room, new_door);
                }
            } else {
                let touching_point = intersection.corner(0);
                let room_corner_no = room_simplex.equals_corner(&touching_point);
                let (room_touch_is_corner, touching_side_no_of_room) = match room_corner_no {
                    Some(no) => (true, no as i32),
                    None => (
                        false,
                        room_simplex
                            .contains_on_border_line_no(&touching_point)
                            .map_or(-1, |no| no as i32),
                    ),
                };
                let neighbour_room_corner_no = current_shape.equals_corner(&touching_point);
                let (neighbour_room_touch_is_corner, touching_side_no_of_neighbour_room) =
                    match neighbour_room_corner_no {
                        Some(no) => (true, current_shape.prev_no(no) as i32),
                        None => (
                            false,
                            current_shape
                                .contains_on_border_line_no(&touching_point)
                                .map_or(-1, |no| no as i32),
                        ),
                    };
                result.add_sorted_neighbour(SortedRoomNeighbour::new(
                    current_object,
                    object_id(current_object, rooms),
                    current_shape.clone(),
                    intersection.clone(),
                    touching_side_no_of_room,
                    touching_side_no_of_neighbour_room,
                    room_touch_is_corner,
                    neighbour_room_touch_is_corner,
                    room_simplex.clone(),
                ));
            }
        }
        Some(result)
    }

    fn add_sorted_neighbour(&mut self, neighbour: SortedRoomNeighbour) {
        self.sorted_neighbours.insert(neighbour);
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
        let mut remove_edge_no: i32 = -1;
        let TileShape::Simplex(room_simplex) = &self.room_shape else {
            unreachable!("calculate_neighbours builds room_shape as a Simplex")
        };
        let room_shape_area = self.room_shape.area();

        let mut prev_edge_no: i32 = -1;
        let mut current_edge_no: i32 = 0;
        for next_neighbour in &self.sorted_neighbours {
            if next_neighbour.touching_side_no_of_room == prev_edge_no {
                continue;
            }
            if next_neighbour.touching_side_no_of_room == current_edge_no {
                prev_edge_no = current_edge_no;
                current_edge_no += 1;
            } else {
                remove_edge_no = current_edge_no;
                break;
            }
        }

        if remove_edge_no < 0 && current_edge_no < room_simplex.border_line_count() as i32 {
            remove_edge_no = current_edge_no;
        }

        if remove_edge_no < 0 {
            return false;
        }
        let enlarged_shape = room_simplex.remove_border_line(index_of(
            remove_edge_no,
            "tryRemoveEdge's removeEdgeNo",
            458,
        ));
        let (layer, contained_shape) = {
            let r = rooms
                .incomplete_room(incomplete_id)
                .expect("the from room is in the arena");
            (r.get_layer(), r.get_contained_shape().cloned())
        };
        let enlarged_room = IncompleteFreeSpaceExpansionRoom::new(
            Some(TileShape::Simplex(enlarged_shape)),
            layer,
            contained_shape.clone(),
        );
        let new_rooms = {
            let ctx = board.ctx();
            tree_of(board, tree_id).complete_shape(
                &enlarged_room,
                net_number,
                None,
                None,
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
                "SortedRoomNeighbours.tryRemoveEdge: completeShape answered a room with no shape \
                 (SortedRoomNeighbours.java:483) — Java NPEs here too"
            )
        });
        if new_shape.area() <= room_shape_area {
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

    pub fn calculate_new_incomplete_rooms(
        &self,
        board: &mut Board,
        rooms: &mut ExpansionRoomStore,
    ) {
        let neighbours: Vec<&SortedRoomNeighbour> = self.sorted_neighbours.iter().collect();
        let Some(&last) = neighbours.last() else {
            panic!(
                "SortedRoomNeighbours.calculateNewIncompleteRooms: the neighbour set is empty \
                 (SortedRoomNeighbours.java:511) — Java throws NoSuchElementException here too"
            )
        };
        let room_simplex = &self.room_shape;
        let from_room_layer = rooms
            .room_layer(board, self.from_room)
            .expect("the from room is in the arena");

        let mut prev_neighbour = last;
        for (index, next_neighbour) in neighbours.iter().copied().enumerate() {
            let prev_is_last = index == 0;

            let mut first_touching_side_no = prev_neighbour.touching_side_no_of_room;
            let mut last_touching_side_no = next_neighbour.touching_side_no_of_room;

            let current_next_no = room_simplex.next_no(index_of(
                first_touching_side_no,
                "calculateNewIncompleteRooms' firstTouchingSideNo",
                517,
            )) as i32;
            let intersection_with_prev_neighbour_ends_at_corner =
                (first_touching_side_no != last_touching_side_no || prev_is_last)
                    && *prev_neighbour.last_corner()
                        == room_simplex.corner(index_of(current_next_no, "currentNextNo", 521));
            let intersection_with_next_neighbour_starts_at_corner =
                (first_touching_side_no != last_touching_side_no || prev_is_last)
                    && *next_neighbour.first_corner()
                        == room_simplex.corner(index_of(
                            last_touching_side_no,
                            "lastTouchingSideNo",
                            525,
                        ));

            if intersection_with_prev_neighbour_ends_at_corner {
                first_touching_side_no = current_next_no;
            }
            if intersection_with_next_neighbour_starts_at_corner {
                last_touching_side_no = room_simplex.prev_no(index_of(
                    last_touching_side_no,
                    "lastTouchingSideNo",
                    532,
                )) as i32;
            }

            let neighbours_touch = neighbours.len() > 1
                && prev_neighbour.last_corner() == next_neighbour.first_corner();

            if !neighbours_touch {
                let mut last_bounding_line_no = prev_neighbour.touching_side_no_of_neighbour_room;
                if !(intersection_with_prev_neighbour_ends_at_corner
                    || prev_neighbour.room_touch_is_corner)
                {
                    last_bounding_line_no = prev_neighbour.neighbour_shape.prev_no(index_of(
                        last_bounding_line_no,
                        "lastBoundingLineNo",
                        546,
                    )) as i32;
                }

                let mut first_bounding_line_no = next_neighbour.touching_side_no_of_neighbour_room;
                if !(intersection_with_next_neighbour_starts_at_corner
                    || next_neighbour.neighbour_room_touch_is_corner)
                {
                    first_bounding_line_no = next_neighbour.neighbour_shape.next_no(index_of(
                        first_bounding_line_no,
                        "firstBoundingLineNo",
                        552,
                    )) as i32;
                }
                let mut start_edge_line: Option<Line> = Some(
                    border_line_of(
                        &next_neighbour.neighbour_shape,
                        first_bounding_line_no,
                        "firstBoundingLineNo",
                        555,
                    )
                    .opposite(),
                );
                let mut middle_edge_line: Option<Line> = None;
                let mut current_touching_side_no = last_touching_side_no;
                let mut first_time = true;
                loop {
                    let mut corner_cut_off = false;
                    if let RoomRef::Incomplete(incomplete_id) = self.from_room
                        && current_touching_side_no == last_touching_side_no
                        && first_touching_side_no != last_touching_side_no
                    {
                        let cut_line_start = prev_neighbour.last_corner().to_float().round();
                        let cut_line_end = next_neighbour.first_corner().to_float().round();
                        let cut_line = Line::new(cut_line_start, cut_line_end);
                        let cut_half_plane = TileShape::get_instance_from_line(cut_line);
                        let new_shape = rooms
                            .room_shape(self.completed_room)
                            .map(|shape| shape.intersection(&cut_half_plane));
                        if let RoomRef::Complete(id) = self.completed_room
                            && let Some(r) = rooms.complete_room_mut(id)
                        {
                            r.set_shape(new_shape);
                        }
                        corner_cut_off = rooms
                            .incomplete_room(incomplete_id)
                            .and_then(|r| r.get_contained_shape())
                            .unwrap_or_else(|| {
                                panic!(
                                    "SortedRoomNeighbours.calculateNewIncompleteRooms: the \
                                     incomplete room has no contained shape \
                                     (SortedRoomNeighbours.java:579) — Java NPEs here too"
                                )
                            })
                            .side_of_line(&cut_line)
                            == Side::OnTheLeft;
                        if corner_cut_off {
                            middle_edge_line = Some(cut_line.opposite());
                        }
                    }
                    let next_touching_side_no = room_simplex.prev_no(index_of(
                        current_touching_side_no,
                        "currentTouchingSideNo",
                        585,
                    )) as i32;

                    if !corner_cut_off {
                        middle_edge_line = Some(
                            border_line_of(
                                room_simplex,
                                current_touching_side_no,
                                "currentTouchingSideNo",
                                588,
                            )
                            .opposite(),
                        );
                    }
                    let middle_edge_line = middle_edge_line.expect("assigned on both paths");
                    let middle_line_dir = middle_edge_line.direction();

                    let last_time = (current_touching_side_no == first_touching_side_no
                        && !(prev_is_last && first_time))
                        || corner_cut_off;

                    let mut end_edge_line: Option<Line> = if last_time {
                        Some(
                            border_line_of(
                                &prev_neighbour.neighbour_shape,
                                last_bounding_line_no,
                                "lastBoundingLineNo",
                                602,
                            )
                            .opposite(),
                        )
                    } else {
                        None
                    };
                    if let Some(line) = end_edge_line
                        && line.direction().side_of(&middle_line_dir) != Side::OnTheLeft
                    {
                        end_edge_line = None;
                    }

                    if let Some(line) = start_edge_line
                        && middle_line_dir.side_of(&line.direction()) != Side::OnTheLeft
                    {
                        start_edge_line = None;
                    }

                    let mut new_edge_lines: Vec<Line> = Vec::with_capacity(3);
                    if let Some(line) = start_edge_line {
                        new_edge_lines.push(line);
                    }
                    new_edge_lines.push(middle_edge_line);
                    if let Some(line) = end_edge_line {
                        new_edge_lines.push(line);
                    }
                    let new_room_shape = Simplex::from_lines(new_edge_lines);
                    if !new_room_shape.is_empty() {
                        let new_room_tile = TileShape::Simplex(new_room_shape);
                        let new_contained_shape = rooms
                            .room_shape(self.completed_room)
                            .map(|shape| shape.intersection(&new_room_tile));
                        if let Some(new_contained_shape) = new_contained_shape
                            && !new_contained_shape.is_empty()
                        {
                            let new_room = RoomRef::Incomplete(rooms.new_incomplete_room(
                                Some(new_room_tile),
                                from_room_layer,
                                Some(new_contained_shape),
                            ));
                            let new_door = rooms.new_door(self.completed_room, new_room, 1);
                            rooms.add_door(self.completed_room, new_door);
                            rooms.add_door(new_room, new_door);
                        }
                    }
                    if last_time {
                        break;
                    }
                    current_touching_side_no = next_touching_side_no;
                    start_edge_line = None;
                    first_time = false;
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
    let room_shape = rooms
        .room_shape(room)
        .unwrap_or_else(|| {
            panic!(
                "SortedRoomNeighbours.calculateIncompleteRoomsWithEmptyNeighbours: room {room:?} \
                 has no shape (SortedRoomNeighbours.java:140) — Java NPEs here too"
            )
        })
        .clone();
    let layer = rooms
        .room_layer(board, room)
        .expect("the obstacle room is in the arena");
    for i in 0..room_shape.border_line_count() {
        let current_line = room_shape
            .border_line(i)
            .expect("i < borderLineCount, so the line exists");
        if insert_door_ok_for_obstacle_room(room, Some(&current_line), board, rooms) {
            let new_room_shape =
                TileShape::Simplex(Simplex::from_lines(vec![current_line.opposite()]));
            let new_contained_shape = room_shape.intersection(&new_room_shape);
            let new_room = RoomRef::Incomplete(rooms.new_incomplete_room(
                Some(new_room_shape),
                layer,
                Some(new_contained_shape),
            ));
            let new_door = rooms.new_door(room, new_room, 1);
            rooms.add_door(room, new_door);
            rooms.add_door(new_room, new_door);
        }
    }
}

fn calculate_target_doors(
    room: RoomId,
    own_net_objects: &[TreeEntry<TreeObject>],
    net_number: i32,
    board: &mut Board,
    rooms: &mut ExpansionRoomStore,
    tree_id: TreeId,
) {
    if !own_net_objects.is_empty()
        && let Some(r) = rooms.complete_room_mut(room)
    {
        r.set_net_dependent();
    }
    for current_entry in own_net_objects {
        let TreeObject::Item(item_id) = current_entry.object else {
            continue;
        };
        let connection_shape = {
            let ctx = board.ctx();
            let Some(item) = board.items.item(item_id) else {
                continue;
            };
            let Some(connectable) = item.as_connectable() else {
                continue;
            };
            if !connectable.as_dyn().contains_net(net_number) {
                continue;
            }
            connectable.as_dyn().get_trace_connection_shape(
                tree_id,
                current_entry.shape_index,
                &ctx,
            )
        };
        let Some(connection_shape) = connection_shape else {
            continue;
        };
        let intersects = rooms
            .complete_room(room)
            .and_then(|r| r.get_shape())
            .is_some_and(|shape| shape.intersects(&connection_shape));
        if !intersects {
            continue;
        }
        let new_target_door = rooms.new_target_door(
            board,
            item_id,
            current_entry.shape_index,
            Some(RoomRef::Complete(room)),
            tree_id,
        );
        if let Some(r) = rooms.complete_room_mut(room) {
            r.add_target_door(new_target_door);
        }
    }
}

pub fn insert_door_ok(
    room1: RoomRef,
    room2: RoomRef,
    door_shape: &TileShape,
    board: &Board,
    rooms: &ExpansionRoomStore,
) -> bool {
    if rooms.door_exists(room1, room2) {
        return false;
    }
    if let (RoomRef::Obstacle(id1), RoomRef::Obstacle(id2)) = (room1, room2) {
        let (Some(r1), Some(r2)) = (rooms.obstacle_room(id1), rooms.obstacle_room(id2)) else {
            return false;
        };
        let (Some(first), Some(second)) =
            (board.get_item(r1.get_item()), board.get_item(r2.get_item()))
        else {
            return false;
        };
        return first.shares_net(second);
    }
    if !matches!(room1, RoomRef::Obstacle(_)) && !matches!(room2, RoomRef::Obstacle(_)) {
        return true;
    }
    let mut door_line: Option<Line> = None;
    let mut prev_corner = door_shape.corner(0);
    let corner_count = door_shape.border_line_count();
    for i in 1..corner_count {
        let current_corner = door_shape.corner(i);
        if current_corner != prev_corner {
            door_line = door_shape.border_line(i - 1);
            break;
        }
        prev_corner = current_corner;
    }
    if matches!(room1, RoomRef::Obstacle(_))
        && !insert_door_ok_for_obstacle_room(room1, door_line.as_ref(), board, rooms)
    {
        return false;
    }
    if matches!(room2, RoomRef::Obstacle(_)) {
        return insert_door_ok_for_obstacle_room(room2, door_line.as_ref(), board, rooms);
    }
    true
}

fn insert_door_ok_for_obstacle_room(
    room: RoomRef,
    door_line: Option<&Line>,
    board: &Board,
    rooms: &ExpansionRoomStore,
) -> bool {
    let Some(door_line) = door_line else {
        return false;
    };
    let RoomRef::Obstacle(id) = room else {
        return true;
    };
    let Some(obstacle_room) = rooms.obstacle_room(id) else {
        return true;
    };
    let Some(Item::Trace(current_trace)) = board.get_item(obstacle_room.get_item()) else {
        return true;
    };
    let room_index = obstacle_room.get_index_in_item();
    if room_index == 0 || room_index + 1 == current_trace.tile_shape_count() {
        let lines = current_trace.polyline().lines();
        let Some(current_trace_line) = lines.get(room_index + 1) else {
            panic!(
                "SortedRoomNeighbours.insertDoorOk: trace line {} of {} \
                 (SortedRoomNeighbours.java:384) — Java throws ArrayIndexOutOfBoundsException here",
                room_index + 1,
                lines.len()
            )
        };
        return current_trace_line.is_parallel(door_line);
    }
    true
}

pub fn create_overlap_door(
    room: ObstacleRoomId,
    other: ObstacleRoomId,
    board: &Board,
    rooms: &mut ExpansionRoomStore,
) -> bool {
    let this_ref = RoomRef::Obstacle(room);
    let other_ref = RoomRef::Obstacle(other);
    if rooms.door_exists(this_ref, other_ref) {
        return false;
    }
    let (Some(this_room), Some(other_room)) =
        (rooms.obstacle_room(room), rooms.obstacle_room(other))
    else {
        return false;
    };
    let (this_item_id, other_item_id) = (this_room.get_item(), other_room.get_item());
    let (this_index, other_index) = (
        this_room.get_index_in_item(),
        other_room.get_index_in_item(),
    );
    let (Some(this_item), Some(other_item)) =
        (board.get_item(this_item_id), board.get_item(other_item_id))
    else {
        return false;
    };
    if !(this_item.is_routable() && other_item.is_routable()) {
        return false;
    }
    if !this_item.shares_net(other_item) {
        return false;
    }
    if this_item_id == other_item_id {
        if !matches!(this_item, Item::Trace(_)) {
            return false;
        }
        let (this_index, other_index) = (this_index as i64, other_index as i64);
        if this_index != other_index + 1 && this_index != other_index - 1 {
            return false;
        }
    }
    let new_door = rooms.new_door(this_ref, other_ref, 2);
    rooms.add_door(this_ref, new_door);
    rooms.add_door(other_ref, new_door);
    true
}

#[derive(Debug, Clone)]
pub struct SortedRoomNeighbour {
    pub search_tree_object: TreeObject,
    pub object_id: i32,
    pub neighbour_shape: TileShape,
    pub intersection: TileShape,
    pub touching_side_no_of_room: i32,
    pub touching_side_no_of_neighbour_room: i32,
    pub room_touch_is_corner: bool,
    pub neighbour_room_touch_is_corner: bool,
    pub room_shape: TileShape,
    first_corner: OnceCell<Point>,
    last_corner: OnceCell<Point>,
}

impl SortedRoomNeighbour {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        search_tree_object: TreeObject,
        object_id: i32,
        neighbour_shape: TileShape,
        intersection: TileShape,
        touching_side_no_of_room: i32,
        touching_side_no_of_neighbour_room: i32,
        room_touch_is_corner: bool,
        neighbour_room_touch_is_corner: bool,
        room_shape: TileShape,
    ) -> SortedRoomNeighbour {
        SortedRoomNeighbour {
            search_tree_object,
            object_id,
            neighbour_shape,
            intersection,
            touching_side_no_of_room,
            touching_side_no_of_neighbour_room,
            room_touch_is_corner,
            neighbour_room_touch_is_corner,
            room_shape,
            first_corner: OnceCell::new(),
            last_corner: OnceCell::new(),
        }
    }

    pub fn first_corner(&self) -> &Point {
        self.first_corner.get_or_init(|| {
            if self.room_touch_is_corner {
                self.room_shape.corner(index_of(
                    self.touching_side_no_of_room,
                    "touchingSideNoOfRoom",
                    768,
                ))
            } else if self.neighbour_room_touch_is_corner {
                self.neighbour_shape.corner(index_of(
                    self.touching_side_no_of_neighbour_room,
                    "touchingSideNoOfNeighbourRoom",
                    770,
                ))
            } else {
                let current_first_corner =
                    self.neighbour_shape
                        .corner(self.neighbour_shape.next_no(index_of(
                            self.touching_side_no_of_neighbour_room,
                            "touchingSideNoOfNeighbourRoom",
                            773,
                        )));
                let prev_line = border_line_of(
                    &self.room_shape,
                    self.room_shape.prev_no(index_of(
                        self.touching_side_no_of_room,
                        "touchingSideNoOfRoom",
                        774,
                    )) as i32,
                    "prevNo(touchingSideNoOfRoom)",
                    774,
                );
                if prev_line.side_of(&current_first_corner) == Side::OnTheRight {
                    current_first_corner
                } else {
                    self.room_shape.corner(index_of(
                        self.touching_side_no_of_room,
                        "touchingSideNoOfRoom",
                        779,
                    ))
                }
            }
        })
    }

    pub fn last_corner(&self) -> &Point {
        self.last_corner.get_or_init(|| {
            if self.room_touch_is_corner {
                self.room_shape.corner(index_of(
                    self.touching_side_no_of_room,
                    "touchingSideNoOfRoom",
                    790,
                ))
            } else if self.neighbour_room_touch_is_corner {
                self.neighbour_shape.corner(index_of(
                    self.touching_side_no_of_neighbour_room,
                    "touchingSideNoOfNeighbourRoom",
                    792,
                ))
            } else {
                let current_last_corner = self.neighbour_shape.corner(index_of(
                    self.touching_side_no_of_neighbour_room,
                    "touchingSideNoOfNeighbourRoom",
                    794,
                ));
                let next_line = border_line_of(
                    &self.room_shape,
                    self.room_shape.next_no(index_of(
                        self.touching_side_no_of_room,
                        "touchingSideNoOfRoom",
                        795,
                    )) as i32,
                    "nextNo(touchingSideNoOfRoom)",
                    795,
                );
                if next_line.side_of(&current_last_corner) == Side::OnTheRight {
                    current_last_corner
                } else {
                    self.room_shape.corner(self.room_shape.next_no(index_of(
                        self.touching_side_no_of_room,
                        "touchingSideNoOfRoom",
                        800,
                    )))
                }
            }
        })
    }

    pub fn compare_to(&self, other: &SortedRoomNeighbour) -> Ordering {
        match self
            .touching_side_no_of_room
            .cmp(&other.touching_side_no_of_room)
        {
            Ordering::Equal => {}
            ordering => return ordering,
        }
        let compare_corner = self
            .room_shape
            .corner_approx(index_of(
                self.touching_side_no_of_room,
                "touchingSideNoOfRoom",
                725,
            ))
            .unwrap_or_else(|| {
                panic!(
                    "SortedRoomNeighbour.compareTo: the room shape has no corner \
                     {} (SortedRoomNeighbours.java:725) — Java NPEs here too",
                    self.touching_side_no_of_room
                )
            });
        let this_distance = self.first_corner().to_float().distance(&compare_corner);
        let other_distance = other.first_corner().to_float().distance(&compare_corner);
        match this_distance.total_cmp(&other_distance) {
            Ordering::Equal => {}
            ordering => return ordering,
        }
        let this_distance2 = self.last_corner().to_float().distance(&compare_corner);
        let other_distance2 = other.last_corner().to_float().distance(&compare_corner);
        match this_distance2.total_cmp(&other_distance2) {
            Ordering::Equal => {}
            ordering => return ordering,
        }
        match self.room_touch_is_corner.cmp(&other.room_touch_is_corner) {
            Ordering::Equal => {}
            ordering => return ordering,
        }
        match self
            .neighbour_room_touch_is_corner
            .cmp(&other.neighbour_room_touch_is_corner)
        {
            Ordering::Equal => {}
            ordering => return ordering,
        }
        if self.neighbour_room_touch_is_corner {
            let mut compare_line_no = self.touching_side_no_of_room;
            if self.room_touch_is_corner {
                compare_line_no =
                    self.room_shape
                        .prev_no(index_of(compare_line_no, "touchingSideNoOfRoom", 743))
                        as i32;
            }
            let compare_dir =
                border_line_of(&self.room_shape, compare_line_no, "compareLineNo", 745)
                    .direction()
                    .opposite();
            let this_compare_line = border_line_of(
                &self.neighbour_shape,
                self.touching_side_no_of_neighbour_room,
                "touchingSideNoOfNeighbourRoom",
                747,
            );
            let other_compare_line = border_line_of(
                &other.neighbour_shape,
                other.touching_side_no_of_neighbour_room,
                "touchingSideNoOfNeighbourRoom",
                749,
            );
            match compare_dir.compare_from(
                &this_compare_line.direction(),
                &other_compare_line.direction(),
            ) {
                Ordering::Equal => {}
                ordering => return ordering,
            }
        }
        match object_kind_rank(self.search_tree_object)
            .cmp(&object_kind_rank(other.search_tree_object))
        {
            Ordering::Equal => {}
            ordering => return ordering,
        }
        match self.object_id.cmp(&other.object_id) {
            Ordering::Equal => {}
            ordering => return ordering,
        }
        match self
            .touching_side_no_of_neighbour_room
            .cmp(&other.touching_side_no_of_neighbour_room)
        {
            Ordering::Equal => {}
            ordering => return ordering,
        }
        corner_key(self.first_corner())
            .cmp(&corner_key(other.first_corner()))
            .then_with(|| corner_key(self.last_corner()).cmp(&corner_key(other.last_corner())))
            .then_with(|| shape_key(&self.neighbour_shape).cmp(&shape_key(&other.neighbour_shape)))
    }
}

fn object_kind_rank(object: TreeObject) -> u8 {
    match object {
        TreeObject::Item(_) => 0,
        TreeObject::Room(_) => 1,
    }
}

fn corner_key(point: &Point) -> (OrderedF64, OrderedF64) {
    let float = point.to_float();
    (OrderedF64(float.x), OrderedF64(float.y))
}

fn shape_key(shape: &TileShape) -> (i32, Vec<(OrderedF64, OrderedF64)>) {
    (
        shape.dimension(),
        shape
            .corner_approx_arr()
            .iter()
            .map(|corner| (OrderedF64(corner.x), OrderedF64(corner.y)))
            .collect(),
    )
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct OrderedF64(f64);

impl Eq for OrderedF64 {}

impl PartialOrd for OrderedF64 {
    fn partial_cmp(&self, other: &OrderedF64) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OrderedF64 {
    fn cmp(&self, other: &OrderedF64) -> Ordering {
        self.0.total_cmp(&other.0)
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

pub(crate) fn tree_of(board: &Board, tree_id: TreeId) -> &ShapeSearchTree {
    board
        .trees
        .trees()
        .find(|tree| tree.id() == tree_id)
        .unwrap_or_else(|| panic!("SortedRoomNeighbours: no search tree with id {tree_id:?}"))
}

pub(crate) fn object_id(object: TreeObject, rooms: &ExpansionRoomStore) -> i32 {
    match object {
        TreeObject::Item(id) => id.0 as i32,
        TreeObject::Room(id) => rooms.complete_room(id).map_or(0, |room| room.get_id()),
    }
}

pub(crate) fn object_is_trace_obstacle(
    object: TreeObject,
    net_number: i32,
    items: &impl ItemLookup,
) -> bool {
    match object {
        TreeObject::Item(id) => items
            .item(id)
            .is_some_and(|item| item.is_trace_obstacle(net_number)),
        TreeObject::Room(_) => true,
    }
}

pub(crate) fn object_tree_shape(
    tree: &ShapeSearchTree,
    object: TreeObject,
    shape_index: usize,
    items: &impl ItemLookup,
    rooms: &impl RoomLookup,
    ctx: &ItemCtx<'_>,
) -> TileShape {
    match object {
        TreeObject::Item(id) => {
            let item = items.item(id).unwrap_or_else(|| {
                panic!(
                    "SortedRoomNeighbours.calculateNeighbours: item {id} has a leaf but is not on \
                     the board"
                )
            });
            tree.get_tree_shape(item, shape_index, ctx)
                .map(std::borrow::Cow::into_owned)
                .unwrap_or_else(|| {
                    panic!(
                        "SortedRoomNeighbours.calculateNeighbours: item {id} has a leaf for shape \
                         {shape_index} but no shape — Java NPEs here too"
                    )
                })
        }
        TreeObject::Room(id) => rooms
            .room_tree_shape(id)
            .unwrap_or_else(|| {
                panic!(
                    "SortedRoomNeighbours.calculateNeighbours: expansion room {id:?} has a leaf \
                     but no shape — Java NPEs here too"
                )
            })
            .clone(),
    }
}

fn index_of(no: i32, what: &str, source_line: u32) -> usize {
    usize::try_from(no).unwrap_or_else(|_| {
        panic!(
            "SortedRoomNeighbours:{source_line}: {what} is {no} — Java throws \
             ArrayIndexOutOfBoundsException here (the -1 comes from :297-300 / :312-316, which \
             log it and use it anyway)"
        )
    })
}

fn border_line_of(shape: &TileShape, no: i32, what: &str, source_line: u32) -> Line {
    let index = index_of(no, what, source_line);
    shape.border_line(index).unwrap_or_else(|| {
        panic!(
            "SortedRoomNeighbours:{source_line}: {what} is {no}, past the shape's \
             {} border lines — Java throws ArrayIndexOutOfBoundsException here",
            shape.border_line_count()
        )
    })
}
