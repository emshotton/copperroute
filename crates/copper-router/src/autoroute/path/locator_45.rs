use copper_geometry::{FloatPoint, Signum, TileShape};

use crate::autoroute::expansion::{ExpandableRef, RoomRef};
use crate::autoroute::maze::TRACE_WIDTH_TOLERANCE;
use crate::autoroute::path::locator::{
    BacktrackElement, LocatedCorner, LocatorWalk, calculate_additional_corner,
};

fn round_to_integer(point: FloatPoint) -> FloatPoint {
    point.round().to_float()
}

fn calc_horizontal_first_from_door(
    door_shape: &TileShape,
    door_dimension: i32,
    from_point: FloatPoint,
    to_point: FloatPoint,
) -> bool {
    let from_door_box = door_shape.bounding_box();
    if door_dimension != 1 {
        return from_door_box.height() >= from_door_box.width();
    }
    let (left_corner, right_corner) = door_corners(door_shape, "calcHorizontalFirstFromDoor", 59);
    let door_half_max_width = door_half_max_width(left_corner, right_corner);
    if f64::from(from_door_box.width()) <= door_half_max_width {
        return true;
    }
    if f64::from(from_door_box.height()) <= door_half_max_width {
        return false;
    }
    let dx = to_point.x - from_point.x;
    let dy = to_point.y - from_point.y;
    let same_sign = Signum::of_f64(dx) == Signum::of_f64(dy);
    if left_corner.y < right_corner.y {
        if same_sign {
            dx.abs() > dy.abs()
        } else {
            dx.abs() < dy.abs()
        }
    } else {
        if same_sign {
            dx.abs() < dy.abs()
        } else {
            dx.abs() > dy.abs()
        }
    }
}

fn calc_horizontal_first_to_door(
    door_shape: &TileShape,
    door_dimension: i32,
    from_point: FloatPoint,
    to_point: FloatPoint,
) -> bool {
    let from_door_box = door_shape.bounding_box();
    if door_dimension != 1 {
        return from_door_box.height() <= from_door_box.width();
    }
    let (left_corner, right_corner) = door_corners(door_shape, "calcHorizontalFirstToDoor", 314);
    let door_half_max_width = door_half_max_width(left_corner, right_corner);
    if f64::from(from_door_box.width()) <= door_half_max_width {
        return false;
    }
    if f64::from(from_door_box.height()) <= door_half_max_width {
        return true;
    }
    let dx = to_point.x - from_point.x;
    let dy = to_point.y - from_point.y;
    let same_sign = Signum::of_f64(dx) == Signum::of_f64(dy);
    if left_corner.y < right_corner.y {
        if same_sign {
            dx.abs() < dy.abs()
        } else {
            dx.abs() > dy.abs()
        }
    } else {
        if same_sign {
            dx.abs() > dy.abs()
        } else {
            dx.abs() < dy.abs()
        }
    }
}

fn door_corners(
    door_shape: &TileShape,
    caller: &str,
    source_line: u32,
) -> (FloatPoint, FloatPoint) {
    let door_line_segment = door_shape.diagonal_corner_segment().unwrap_or_else(|| {
        panic!(
            "FoundConnectionLocator45Degree.{caller}: diagonalCornerSegment is null for an \
             empty door shape — Java throws a NullPointerException at \
             FoundConnectionLocator45Degree:{source_line}"
        )
    });
    if door_line_segment.a.x < door_line_segment.b.x
        || (door_line_segment.a.x == door_line_segment.b.x
            && door_line_segment.a.y <= door_line_segment.b.y)
    {
        (door_line_segment.a, door_line_segment.b)
    } else {
        (door_line_segment.b, door_line_segment.a)
    }
}

fn door_half_max_width(left_corner: FloatPoint, right_corner: FloatPoint) -> f64 {
    let door_dx = right_corner.x - left_corner.x;
    let door_dy = right_corner.y - left_corner.y;
    let abs_door_dy = door_dy.abs();
    let door_max_width = (door_dx).max(abs_door_dy);
    0.5 * door_max_width
}

#[allow(clippy::too_many_lines)]
pub(crate) fn calculate_next_trace_corners(
    walk: &mut LocatorWalk<'_>,
    backtrack_array: &[BacktrackElement],
) -> Vec<LocatedCorner> {
    let mut result: Vec<LocatedCorner> = Vec::new();

    if walk.current_to_door_index > walk.current_target_door_index {
        return result;
    }

    let from_index = usize::try_from(walk.current_to_door_index - 1)
        .expect("currentToDoorIndex is currentFromDoorIndex + 1, i.e. at least 1");
    let current_from_info = backtrack_array[from_index];

    let Some(next_room) = current_from_info.next_room else {
        // FRLogger.warn("... nextRoom is null")
        return result;
    };

    let room_shape = walk
        .engine
        .rooms
        .room_shape(next_room)
        .expect("the backtrack room has a shape")
        .clone();

    let trace_halfwidth = walk.ctrl.compensated_trace_half_width[walk.current_trace_layer];
    let trace_halfwidth_add = trace_halfwidth + TRACE_WIDTH_TOLERANCE;
    let shrink_offset = if matches!(next_room, RoomRef::Obstacle(_)) {
        trace_halfwidth
    } else {
        trace_halfwidth_add
    };

    let mut shrinked_room_shape = room_shape.offset(-f64::from(shrink_offset));
    if shrinked_room_shape.is_empty() {
        shrinked_room_shape = room_shape;
    } else {
        let nearest_room_point = shrinked_room_shape
            .nearest_point_approx(&walk.current_from_point)
            .expect("a non-empty shape has a nearest point");
        let door_shape = walk
            .engine
            .expandable_shape(current_from_info.door)
            .expect("the backtrack door has a shape");
        let door_dimension = walk.engine.expandable_dimension(current_from_info.door);
        let horizontal_first = calc_horizontal_first_from_door(
            &door_shape,
            door_dimension,
            walk.current_from_point,
            nearest_room_point,
        );
        let nearest_room_point = round_to_integer(nearest_room_point);
        let add_corner = calculate_additional_corner(
            walk.current_from_point,
            nearest_room_point,
            horizontal_first,
            walk.angle_restriction,
        );
        let add_corner = walk.fresh(add_corner);
        result.push(add_corner);
        let nearest = walk.fresh(nearest_room_point);
        result.push(nearest);
        walk.set_current_from_point(nearest);
    }

    if walk.current_to_door_index == walk.current_target_door_index {
        let nearest_point = walk
            .current_target_shape
            .nearest_point_approx(&walk.current_from_point)
            .expect("the target shape is non-empty");
        let nearest_point = round_to_integer(nearest_point);
        let mut add_corner = calculate_additional_corner(
            walk.current_from_point,
            nearest_point,
            true,
            walk.angle_restriction,
        );
        if !shrinked_room_shape.contains_float(&add_corner) {
            add_corner = calculate_additional_corner(
                walk.current_from_point,
                nearest_point,
                false,
                walk.angle_restriction,
            );
        }
        let add_corner = walk.fresh(add_corner);
        result.push(add_corner);
        let nearest = walk.fresh(nearest_point);
        result.push(nearest);
        walk.current_to_door_index += 1;
        return result;
    }

    let to_index =
        usize::try_from(walk.current_to_door_index).expect("the door index is non-negative");
    let current_to_info = backtrack_array[to_index];
    let ExpandableRef::Door(current_to_door) = current_to_info.door else {
        // FRLogger.warn("... ExpansionDoor expected")
        return result;
    };

    let mut nearest_to_door_point;
    let to_door_dimension = walk
        .engine
        .rooms
        .door(current_to_door)
        .expect("the backtrack door is live")
        .dimension;
    if to_door_dimension == 2 {
        let to_door_shape = walk
            .engine
            .rooms
            .door_shape(current_to_door)
            .expect("the backtrack door has a shape");
        let shrinked_to_door_shape = to_door_shape.shrink(f64::from(shrink_offset));
        nearest_to_door_point = shrinked_to_door_shape
            .nearest_point_approx(&walk.current_from_point)
            .expect("a shrunk door shape is non-empty");
        nearest_to_door_point = round_to_integer(nearest_to_door_point);
    } else {
        let line_sections = walk
            .engine
            .rooms
            .door_section_segments(current_to_door, f64::from(trace_halfwidth));
        let section_no = usize::try_from(current_to_info.section_no_of_door)
            .expect("a section number is non-negative");
        if section_no >= line_sections.len() {
            // FRLogger.warn("... lineSections inconsistent")
            return result;
        }
        let current_line_section = line_sections[section_no];
        nearest_to_door_point =
            current_line_section.nearest_segment_point(&walk.current_from_point);

        let mut nearest_to_door_point_ok = true;
        if let Some(to_next_room) = current_to_info.next_room {
            let next_room_shape = walk
                .engine
                .rooms
                .room_shape(to_next_room)
                .expect("the backtrack room has a shape")
                .to_simplex();
            let nearest_points = TileShape::Simplex(next_room_shape)
                .nearest_border_points_approx(&nearest_to_door_point, 2);
            if nearest_points.len() >= 2 {
                nearest_to_door_point_ok = nearest_points[1].distance(&nearest_to_door_point)
                    >= f64::from(trace_halfwidth_add);
            }
        }
        if !nearest_to_door_point_ok {
            nearest_to_door_point = current_line_section.a.middle_point(&current_line_section.b);
        }
    }
    let nearest_to_door_point = round_to_integer(nearest_to_door_point);
    let to_door_shape = walk
        .engine
        .expandable_shape(current_to_info.door)
        .expect("the backtrack door has a shape");
    let to_door_dim = walk.engine.expandable_dimension(current_to_info.door);
    let horizontal_first = calc_horizontal_first_to_door(
        &to_door_shape,
        to_door_dim,
        walk.current_from_point,
        nearest_to_door_point,
    );
    let add_corner = calculate_additional_corner(
        walk.current_from_point,
        nearest_to_door_point,
        horizontal_first,
        walk.angle_restriction,
    );
    let add_corner = walk.fresh(add_corner);
    result.push(add_corner);
    let nearest = walk.fresh(nearest_to_door_point);
    result.push(nearest);
    walk.current_to_door_index += 1;
    result
}
