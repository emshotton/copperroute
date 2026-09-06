use fr_board::{Board, Item, ItemId, ObstacleRoomId};
use fr_geometry::polyline_shape::PolylineShapeOps;
use fr_geometry::{FloatLine, Line, LineSegment, Side};

use crate::arena::DoorId;
use crate::autoroute::expansion::{ExpandableRef, ExpansionRoomStore, RoomRef};
use crate::autoroute::maze::queue::p7t14b_maze_ledger;
use crate::autoroute::maze::{AutorouteControl, MazeListElement};
use crate::board_ext::TraceShover;

#[derive(Debug, Clone, PartialEq)]
pub struct DoorSection {
    pub door: DoorId,
    pub section_index: i32,
    pub section_line: FloatLine,
}

impl DoorSection {
    pub fn new(door: DoorId, section_index: i32, section_line: FloatLine) -> DoorSection {
        DoorSection {
            door,
            section_index,
            section_line,
        }
    }
}

pub struct MazeTraceShover;

impl MazeTraceShover {
    #[allow(clippy::too_many_arguments)]
    pub fn check_shove_trace_line(
        list_element: &MazeListElement,
        obstacle_room: ObstacleRoomId,
        rooms: &mut ExpansionRoomStore,
        board: &mut Board,
        ctrl: &AutorouteControl,
        shove_to_the_left: bool,
        to_door_list: &mut Vec<DoorSection>,
    ) -> bool {
        if p7t14b_maze_ledger() {
            let (item, index) = rooms.obstacle_room(obstacle_room).map_or((0, -1), |room| {
                (
                    room.get_item().0,
                    i64::try_from(room.get_index_in_item()).unwrap_or(-1),
                )
            });
            eprintln!(
                "CSTL left={shove_to_the_left} item={item} idx={index} sec={} adj={:?}",
                list_element.section_no_of_door, list_element.adjustment
            );
        }
        let obstacle_room_ref = RoomRef::Obstacle(obstacle_room);
        let ExpandableRef::Door(from_door) = list_element.door else {
            return true;
        };
        let Some(room) = rooms.obstacle_room(obstacle_room) else {
            return true;
        };
        let obstacle_trace_id = room.get_item();
        let trace_layer = match room.get_layer(board) {
            Some(layer) => layer,
            None => return true,
        };
        let Some(Item::Trace(obstacle_trace)) = board.items.get(&obstacle_trace_id) else {
            return true;
        };
        if obstacle_trace.get_half_width() != ctrl.trace_half_width[trace_layer]
            || obstacle_trace.hdr.clearance_class() != ctrl.trace_clearance_class_index
        {
            return true;
        }
        let compensated_trace_half_width =
            f64::from(ctrl.compensated_trace_half_width[trace_layer]);
        let Some(from_door_shape) = rooms.door_shape(from_door) else {
            return true;
        };
        if from_door_shape.max_width() < 2.0 * compensated_trace_half_width {
            return true;
        }
        let trace_corner_no = rooms
            .obstacle_room(obstacle_room)
            .expect("just read")
            .get_index_in_item();

        let trace_polyline = match board.items.get(&obstacle_trace_id) {
            Some(Item::Trace(trace)) => trace.polyline().clone(),
            _ => return true,
        };

        let line_count = trace_polyline.lines().len();
        crate::autoroute::instrument::record_visit(
            crate::autoroute::instrument::Guard::G3TraceCornerOutOfRange,
        );
        if line_count < 2 || trace_corner_no >= line_count - 2 {
            crate::autoroute::instrument::record_guard(
                crate::autoroute::instrument::Guard::G3TraceCornerOutOfRange,
                u64::from(obstacle_trace_id.0),
                trace_corner_no,
                line_count.saturating_sub(2),
                line_count >= 2,
            );
            return false;
        }
        let room_doors = rooms.room_doors(obstacle_room_ref).to_vec();

        let mut shove_line_segment: LineSegment;
        let from_door_dimension = rooms
            .door(from_door)
            .expect("the from door of a live list element")
            .dimension;
        if from_door_dimension == 2 {
            let Some(RoomRef::Obstacle(other)) = rooms
                .door(from_door)
                .expect("just read")
                .other_complete_room(obstacle_room_ref)
            else {
                return false;
            };
            let other_item = rooms
                .obstacle_room(other)
                .map(super::super::expansion::ObstacleExpansionRoom::get_item);
            let Some(other_item) = other_item else {
                return false;
            };
            if !end_points_matching(board, obstacle_trace_id, other_item) {
                return false;
            }
            let door_center = from_door_shape.centre_of_gravity();
            let (Some(corner1), Some(corner2)) = (
                trace_polyline.corner_approx(trace_corner_no),
                trace_polyline.corner_approx(trace_corner_no + 1),
            ) else {
                return false;
            };
            if corner1.distance_square(&corner2) < 1.0 {
                return false;
            }
            let shove_into_direction_of_trace_start =
                door_center.distance_square(&corner2) < door_center.distance_square(&corner1);
            let Some(segment) = LineSegment::from_polyline(&trace_polyline, trace_corner_no + 1)
            else {
                return false;
            };
            shove_line_segment = segment;
            if shove_into_direction_of_trace_start {
                shove_line_segment = shove_line_segment.opposite();
            }
        } else {
            let Some(from_room) = rooms
                .door(from_door)
                .expect("just read")
                .other_complete_room(obstacle_room_ref)
            else {
                return false;
            };
            let Some(from_room_shape) = rooms.room_shape(from_room) else {
                return false;
            };
            let from_point = from_room_shape.centre_of_gravity();
            let shove_trace_line = trace_polyline.lines()[trace_corner_no + 1];
            let Some(door_line_segment) = from_door_shape.diagonal_corner_segment() else {
                return false;
            };
            let side_of_trace_line = shove_trace_line.side_of_float(&door_line_segment.a, 0.0);
            let Some(polar_line_segment) = from_door_shape.polar_line_segment(&from_point) else {
                return false;
            };
            let door_line_swapped = polar_line_segment.b.distance_square(&door_line_segment.a)
                < polar_line_segment.a.distance_square(&door_line_segment.a);

            let shape_entry_check_distance = compensated_trace_half_width + 5.0;
            let check_dist_square = shape_entry_check_distance * shape_entry_check_distance;

            let section_ok = if shove_to_the_left && !door_line_swapped
                || !shove_to_the_left && door_line_swapped
            {
                let last_section = i32::try_from(
                    rooms
                        .door(from_door)
                        .expect("just read")
                        .maze_search_element_count()
                        .unwrap_or_else(|| {
                            panic!(
                                "MazeTraceShover.checkShoveTraceLine: the from door's sectionArr \
                                 is still null — Java throws at MazeTraceShover.java:120"
                            )
                        }),
                )
                .unwrap_or(i32::MAX)
                    - 1;
                list_element.section_no_of_door == last_section
                    && (list_element
                        .shape_entry
                        .a
                        .distance_square(&door_line_segment.b)
                        <= check_dist_square
                        || list_element
                            .shape_entry
                            .b
                            .distance_square(&door_line_segment.b)
                            <= check_dist_square)
            } else {
                list_element.section_no_of_door == 0
                    && (list_element
                        .shape_entry
                        .a
                        .distance_square(&door_line_segment.a)
                        <= check_dist_square
                        || list_element
                            .shape_entry
                            .b
                            .distance_square(&door_line_segment.a)
                            <= check_dist_square)
            };
            if !section_ok {
                return false;
            }

            let shrinked_line_segment =
                polar_line_segment.shrink_segment(compensated_trace_half_width);
            let perpendicular_direction = shove_trace_line.direction().turn_45_degree(2);
            let (closing_point, middle, end) = if side_of_trace_line == Side::OnTheLeft {
                if shove_to_the_left {
                    (
                        shrinked_line_segment.b.round(),
                        trace_polyline.lines()[trace_corner_no + 1],
                        trace_polyline.lines()[trace_corner_no + 2],
                    )
                } else {
                    (
                        shrinked_line_segment.a.round(),
                        trace_polyline.lines()[trace_corner_no + 1].opposite(),
                        trace_polyline.lines()[trace_corner_no].opposite(),
                    )
                }
            } else if shove_to_the_left {
                (
                    shrinked_line_segment.b.round(),
                    trace_polyline.lines()[trace_corner_no + 1].opposite(),
                    trace_polyline.lines()[trace_corner_no].opposite(),
                )
            } else {
                (
                    shrinked_line_segment.a.round(),
                    trace_polyline.lines()[trace_corner_no + 1],
                    trace_polyline.lines()[trace_corner_no + 2],
                )
            };
            let start_closing_line = Line::from_direction(closing_point, &perpendicular_direction);
            shove_line_segment = LineSegment::new(start_closing_line, middle, end);
        }

        let trace_half_width = ctrl.trace_half_width[trace_layer];
        let net_numbers = [ctrl.net_number];

        let mut shove_width = board.check_trace_segment_of_line_segment(
            &shove_line_segment,
            trace_layer,
            &net_numbers,
            trace_half_width,
            ctrl.trace_clearance_class_index,
            true,
        );
        let mut segment_shortened = false;
        if shove_width < f64::from(i32::MAX) {
            shove_width -= 1.0;
            if shove_width <= 0.0 {
                return true;
            }
            shove_line_segment = shove_line_segment.change_length_approx(shove_width);
            segment_shortened = true;
        }

        let from_corner = shove_line_segment.start_point_approx();
        let to_corner = shove_line_segment.end_point_approx();
        let segment_ist_point = from_corner.distance_square(&to_corner) < 0.1;

        if !segment_ist_point {
            shove_width = TraceShover::check_segment(
                board,
                &shove_line_segment,
                shove_to_the_left,
                trace_layer,
                &net_numbers,
                trace_half_width,
                ctrl.trace_clearance_class_index,
                ctrl.max_shove_trace_recursion_depth,
                ctrl.max_shove_via_recursion_depth,
            );
            if shove_width <= 0.0 {
                return true;
            }
        }

        if segment_shortened {
            shove_width = (shove_width).min(to_corner.distance(&from_corner));
        }

        let shove_line = shove_line_segment.get_line();

        let from_door_compare_distance = if from_door_dimension == 2 || segment_ist_point {
            f64::MAX
        } else {
            match from_door_shape.corner_approx(0) {
                Some(corner) => to_corner.distance_square(&corner),
                None => f64::MAX,
            }
        };

        for current_door in room_doors {
            if current_door == from_door {
                continue;
            }
            let Some(door) = rooms.door(current_door) else {
                continue;
            };
            let (first_room, second_room, dimension) =
                (door.first_room, door.second_room, door.dimension);
            if let (RoomRef::Obstacle(first), RoomRef::Obstacle(second)) = (first_room, second_room)
            {
                let first_item = rooms.obstacle_room(first).map(|r| r.get_item());
                let second_item = rooms.obstacle_room(second).map(|r| r.get_item());
                if first_item != second_item {
                    continue;
                }
            }
            let Some(current_door_shape) = rooms.door_shape(current_door) else {
                continue;
            };
            if dimension == 2 && shove_width >= f64::from(i32::MAX) {
                if current_door_shape.contains_float(&to_corner) {
                    let line_sections =
                        rooms.door_section_segments(current_door, compensated_trace_half_width);
                    if let Some(first_section) = line_sections.first() {
                        to_door_list.push(DoorSection::new(current_door, 0, *first_section));
                    }
                }
            } else if !segment_ist_point {
                let Some(current_door_segment) = current_door_shape.diagonal_corner_segment()
                else {
                    continue;
                };
                let start_corner_side = shove_line.side_of_float(&current_door_segment.a, 0.0);
                let end_corner_side = shove_line.side_of_float(&current_door_segment.b, 0.0);
                if shove_to_the_left {
                    if start_corner_side != Side::OnTheLeft || end_corner_side != Side::OnTheLeft {
                        continue;
                    }
                } else if start_corner_side != Side::OnTheRight
                    || end_corner_side != Side::OnTheRight
                {
                    continue;
                }
                let Some(current_door_line) = current_door_shape.polar_line_segment(&from_corner)
                else {
                    continue;
                };
                let current_door_nearest_corner =
                    if current_door_line.a.distance_square(&from_corner)
                        <= current_door_line.b.distance_square(&from_corner)
                    {
                        current_door_line.a
                    } else {
                        current_door_line.b
                    };
                if to_corner.distance_square(&current_door_nearest_corner)
                    >= from_door_compare_distance
                {
                    continue;
                }
                let current_door_projection =
                    current_door_nearest_corner.projection_approx(&shove_line);

                if current_door_projection.distance(&from_corner) + compensated_trace_half_width
                    <= shove_width
                {
                    let line_sections =
                        rooms.door_section_segments(current_door, compensated_trace_half_width);
                    for (i, current_line_section) in line_sections.iter().enumerate() {
                        let current_section_nearest_corner =
                            if current_line_section.a.distance_square(&from_corner)
                                <= current_line_section.b.distance_square(&from_corner)
                            {
                                current_line_section.a
                            } else {
                                current_line_section.b
                            };
                        let current_section_projection =
                            current_section_nearest_corner.projection_approx(&shove_line);
                        if current_section_projection.distance(&from_corner) <= shove_width {
                            to_door_list.push(DoorSection::new(
                                current_door,
                                i32::try_from(i).unwrap_or(i32::MAX),
                                *current_line_section,
                            ));
                        }
                    }
                }
            }
        }
        true
    }
}

pub(crate) fn end_points_matching(board: &Board, trace: ItemId, from_item: ItemId) -> bool {
    if from_item == trace {
        return true;
    }
    let (Some(from), Some(trace_item)) = (board.items.get(&from_item), board.items.get(&trace))
    else {
        return false;
    };
    if !trace_item.shares_net(from) {
        return false;
    }
    let Some(Item::Trace(trace_item)) = board.items.get(&trace) else {
        return false;
    };
    let (first_corner, last_corner) = (trace_item.first_corner(), trace_item.last_corner());
    match board.items.get(&from_item) {
        Some(Item::Pin(_) | Item::Via(_)) => {
            let Some(from_center) = board.drill_center(from_item) else {
                return false;
            };
            first_corner.as_ref() == Some(&from_center)
                || last_corner.as_ref() == Some(&from_center)
        }
        Some(Item::Trace(from_trace)) => {
            let (from_first, from_last) = (from_trace.first_corner(), from_trace.last_corner());
            (first_corner.is_some() && first_corner == from_first)
                || (first_corner.is_some() && first_corner == from_last)
                || (last_corner.is_some() && last_corner == from_first)
                || (last_corner.is_some() && last_corner == from_last)
        }
        _ => false,
    }
}
