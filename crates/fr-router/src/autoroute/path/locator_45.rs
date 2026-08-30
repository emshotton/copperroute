//! Port of `autoroute.path.FoundConnectionLocator45Degree`
//! (FoundConnectionLocator45Degree.java:27-356): "locates and constructs 45-degree trace
//! connection geometries from maze search backtrack paths."
//!
//! It serves **both** the 90° and the 45° regime — `FoundConnectionLocator.getInstance`
//! (`:196-200`) builds it for either. The two differ only inside `calculateAdditionalCorner`
//! (FoundConnectionLocator.java:390-404), which switches on the `angleRestriction` value this
//! class carries as a field.
//!
//! Java's class has no state of its own: its constructor is a bare `super(...)` (`:30-38`) and
//! every member is `static` or reads `this` fields declared in the superclass. So the port is
//! three free functions over `LocatorWalk`, and there is no struct here.

use fr_geometry::{FloatPoint, Signum, TileShape};

use crate::autoroute::expansion::{ExpandableRef, RoomRef};
use crate::autoroute::maze::TRACE_WIDTH_TOLERANCE;
use crate::autoroute::path::locator::{
    BacktrackElement, LocatedCorner, LocatorWalk, calculate_additional_corner,
};

/// Port of the private static `roundToInteger(FloatPoint)` (`:40-42`).
fn round_to_integer(point: FloatPoint) -> FloatPoint {
    point.round().to_float()
}

/// Port of the private static `calcHorizontalFirstFromDoor(ExpandableObject, FloatPoint,
/// FloatPoint)` (`:48-101`): "calculates if the next 45-degree angle should be horizontal first
/// when coming from fromPoint on fromDoor."
fn calc_horizontal_first_from_door(
    door_shape: &TileShape,
    door_dimension: i32,
    from_point: FloatPoint,
    to_point: FloatPoint,
) -> bool {
    // :50-54.
    let from_door_box = door_shape.bounding_box();
    if door_dimension != 1 {
        return from_door_box.height() >= from_door_box.width();
    }
    let Some((left_corner, right_corner)) = door_corners(door_shape) else {
        // Java dereferences the null `diagonalCornerSegment` at `:59` and throws; an empty door
        // shape cannot reach here, because `getDimension() == 1` implies a real segment.
        return from_door_box.height() >= from_door_box.width();
    };
    // :68-79.
    let door_half_max_width = door_half_max_width(left_corner, right_corner);
    if f64::from(from_door_box.width()) <= door_half_max_width {
        // :74-76: "door is about vertical".
        return true;
    }
    if f64::from(from_door_box.height()) <= door_half_max_width {
        // :77-79: "door is about horizontal".
        return false;
    }
    // :80-99.
    let dx = to_point.x - from_point.x;
    let dy = to_point.y - from_point.y;
    let same_sign = Signum::of_f64(dx) == Signum::of_f64(dy);
    if left_corner.y < right_corner.y {
        // :83-89: "door is about right diagonal".
        if same_sign {
            dx.abs() > dy.abs()
        } else {
            dx.abs() < dy.abs()
        }
    } else {
        // :91-98: "door is about left diagonal".
        if same_sign {
            dx.abs() < dy.abs()
        } else {
            dx.abs() > dy.abs()
        }
    }
}

/// Port of the private `calcHorizontalFirstToDoor(ExpandableObject, FloatPoint, FloatPoint)`
/// (`:304-356`): "calculates, if the 45-degree angle to the next door shape should be horizontal
/// first when coming from fromPoint."
///
/// It is `calcHorizontalFirstFromDoor` with **every** answer flipped — `:309` vs `:53`, `:331`
/// vs `:75`, `:334` vs `:78` and all four arms of the diagonal tests. The two are written out
/// separately in Java and are kept separate here, because "the negation of the other" is a claim
/// about six independent lines rather than a shared body.
fn calc_horizontal_first_to_door(
    door_shape: &TileShape,
    door_dimension: i32,
    from_point: FloatPoint,
    to_point: FloatPoint,
) -> bool {
    // :306-310.
    let from_door_box = door_shape.bounding_box();
    if door_dimension != 1 {
        return from_door_box.height() <= from_door_box.width();
    }
    let Some((left_corner, right_corner)) = door_corners(door_shape) else {
        return from_door_box.height() <= from_door_box.width();
    };
    // :323-334.
    let door_half_max_width = door_half_max_width(left_corner, right_corner);
    if f64::from(from_door_box.width()) <= door_half_max_width {
        // :329-331: "door is about vertical".
        return false;
    }
    if f64::from(from_door_box.height()) <= door_half_max_width {
        // :332-334: "door is about horizontal".
        return true;
    }
    // :335-354.
    let dx = to_point.x - from_point.x;
    let dy = to_point.y - from_point.y;
    let same_sign = Signum::of_f64(dx) == Signum::of_f64(dy);
    if left_corner.y < right_corner.y {
        // :338-344: "door is about right diagonal".
        if same_sign {
            dx.abs() < dy.abs()
        } else {
            dx.abs() > dy.abs()
        }
    } else {
        // :346-353: "door is about left diagonal".
        if same_sign {
            dx.abs() > dy.abs()
        } else {
            dx.abs() < dy.abs()
        }
    }
}

/// The shared prologue of the two `calcHorizontalFirst*` methods (`:56-67` = `:311-322`): the
/// diagonal corner segment of the door shape, ordered left corner first.
fn door_corners(door_shape: &TileShape) -> Option<(FloatPoint, FloatPoint)> {
    let door_line_segment = door_shape.diagonal_corner_segment()?;
    // :59-67.
    if door_line_segment.a.x < door_line_segment.b.x
        || (door_line_segment.a.x == door_line_segment.b.x
            && door_line_segment.a.y <= door_line_segment.b.y)
    {
        Some((door_line_segment.a, door_line_segment.b))
    } else {
        Some((door_line_segment.b, door_line_segment.a))
    }
}

/// The shared `0.5 * max(doorDx, |doorDy|)` of `:68-73` = `:323-328`.
///
/// `doorDx` is **not** taken in absolute value (`:68`), so a door whose ordering put the larger
/// x first yields a negative `doorDx` and `Math.max` picks `|doorDy|`.
fn door_half_max_width(left_corner: FloatPoint, right_corner: FloatPoint) -> f64 {
    let door_dx = right_corner.x - left_corner.x;
    let door_dy = right_corner.y - left_corner.y;
    let abs_door_dy = door_dy.abs();
    let door_max_width = fr_geometry::java_max(door_dx, abs_door_dy);
    0.5 * door_max_width
}

/// Port of `calculateNextTraceCorners()` (`:103-298`) — the 90°/45° override of
/// `FoundConnectionLocator.calculateNextTraceCorners` (FoundConnectionLocator.java:499).
///
/// `// not ported:` `calculateNextTraceCorners` — the four net-33/66/67 `FRLogger.trace` blocks
/// of `:108-123`, `:151-174`, `:205-224` and `:277-296` (plan-6 ruling 14).
#[allow(clippy::too_many_lines)]
pub(crate) fn calculate_next_trace_corners(
    walk: &mut LocatorWalk<'_>,
    backtrack_array: &[BacktrackElement],
) -> Vec<LocatedCorner> {
    let mut result: Vec<LocatedCorner> = Vec::new();

    // :107-125.
    if walk.current_to_door_index > walk.current_target_door_index {
        return result;
    }

    // :127.
    let from_index = usize::try_from(walk.current_to_door_index - 1)
        .expect("currentToDoorIndex is currentFromDoorIndex + 1, i.e. at least 1");
    let current_from_info = backtrack_array[from_index];

    // :129-133.
    let Some(next_room) = current_from_info.next_room else {
        // FRLogger.warn("... nextRoom is null")
        return result;
    };

    // :135.
    let room_shape = walk
        .engine
        .rooms
        .room_shape(next_room)
        .expect("the backtrack room has a shape")
        .clone();

    // :137-148.
    let trace_halfwidth = walk.ctrl.compensated_trace_half_width[walk.current_trace_layer];
    // :138-141: "add some tolerance for free space expansion rooms."
    let trace_halfwidth_add = trace_halfwidth + TRACE_WIDTH_TOLERANCE;
    let shrink_offset = if matches!(next_room, RoomRef::Obstacle(_)) {
        trace_halfwidth
    } else {
        trace_halfwidth_add
    };

    // :150.
    let mut shrinked_room_shape = room_shape.offset(-f64::from(shrink_offset));
    if shrinked_room_shape.is_empty() {
        // :187-189.
        shrinked_room_shape = room_shape;
    } else {
        // :175-186: "enter the shrunk room shape by a 45-degree angle first".
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
        // :186. Note Java assigns `currentFromPoint` here and leaves `previousFromPoint` alone.
        walk.set_current_from_point(nearest);
    }

    // :191-226.
    if walk.current_to_door_index == walk.current_target_door_index {
        let nearest_point = walk
            .current_target_shape
            .nearest_point_approx(&walk.current_from_point)
            .expect("the target shape is non-empty");
        let nearest_point = round_to_integer(nearest_point);
        // :194-201.
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
        // :204.
        walk.current_to_door_index += 1;
        return result;
    }

    // :228-233.
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
        // :236-242: "may not happen in free angle routing mode because then corners are cut off."
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
        // :244-250.
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
        // :251-252.
        let current_line_section = line_sections[section_no];
        nearest_to_door_point =
            current_line_section.nearest_segment_point(&walk.current_from_point);

        // :254-267.
        let mut nearest_to_door_point_ok = true;
        if let Some(to_next_room) = current_to_info.next_room {
            // :256-258: "with IntBox or IntOctagon the next calculation will not work, because
            // they have border lines of length 0."
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
            // :264-267: "may be the room has an acute (45 degree) angle at a corner of the door".
            nearest_to_door_point = current_line_section.a.middle_point(&current_line_section.b);
        }
    }
    // :269.
    let nearest_to_door_point = round_to_integer(nearest_to_door_point);
    // :270-275.
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
    // :276.
    walk.current_to_door_index += 1;
    result
}
