use copper_geometry::{FloatLine, FloatPoint, PolylineShapeOps, Side, TileShape};

use crate::autoroute::maze::TRACE_WIDTH_TOLERANCE;
use crate::autoroute::path::locator::{BacktrackElement, LocatedCorner, LocatorWalk};

const C_TOLERANCE: f64 = 1.0;

fn door_pole_and_shape(
    walk: &LocatorWalk<'_>,
    to_info: &BacktrackElement,
) -> (FloatPoint, TileShape) {
    let from_room = to_info
        .next_room
        .and_then(|room| walk.engine.expandable_other_room(to_info.door, room))
        .expect(
            "FoundConnectionLocatorAnyAngle.calcDoorLeftCorner: fromRoom is null — Java throws a \
             NullPointerException at FoundConnectionLocatorAnyAngle.java:45",
        );
    let pole = walk
        .engine
        .rooms
        .room_shape(from_room)
        .expect(
            "FoundConnectionLocatorAnyAngle.calcDoorLeftCorner: the common room has no shape — \
             Java throws a NullPointerException at FoundConnectionLocatorAnyAngle.java:45",
        )
        .centre_of_gravity();
    let shape = walk.engine.expandable_shape(to_info.door).expect(
        "FoundConnectionLocatorAnyAngle.calcDoorLeftCorner: the door has no shape — Java throws a \
         NullPointerException at FoundConnectionLocatorAnyAngle.java:47",
    );
    (pole, shape)
}

fn calc_door_left_corner(walk: &LocatorWalk<'_>, to_info: &BacktrackElement) -> FloatPoint {
    let (pole, shape) = door_pole_and_shape(walk, to_info);
    let left_most_corner_no = shape.index_of_left_most_corner(&pole);
    shape.corner_approx(left_most_corner_no).expect(
        "FoundConnectionLocatorAnyAngle.calcDoorLeftCorner: cornerApprox is null for a line-less \
         simplex (Simplex.java:182-184), and every caller dereferences it at once",
    )
}

fn calc_door_right_corner(walk: &LocatorWalk<'_>, to_info: &BacktrackElement) -> FloatPoint {
    let (pole, shape) = door_pole_and_shape(walk, to_info);
    let right_most_corner_no = shape.index_of_right_most_corner(&pole);
    shape.corner_approx(right_most_corner_no).expect(
        "FoundConnectionLocatorAnyAngle.calcDoorRightCorner: cornerApprox is null for a line-less \
         simplex (Simplex.java:182-184), and every caller dereferences it at once",
    )
}

fn left_tangential(from: FloatPoint, to: Option<FloatPoint>, distance: f64) -> Option<FloatPoint> {
    from.left_tangential_point(&to?, distance)
}

fn right_tangential(from: FloatPoint, to: Option<FloatPoint>, distance: f64) -> Option<FloatPoint> {
    from.right_tangential_point(&to?, distance)
}

fn right_turn_next_corner(
    walk: &mut LocatorWalk<'_>,
    from_corner: FloatPoint,
    dist: f64,
    to_corner: Option<FloatPoint>,
    next_corner: Option<FloatPoint>,
) -> Option<LocatedCorner> {
    let Some(current_tangential_point) = left_tangential(from_corner, to_corner, dist) else {
        return Some(walk.same_as_current_from_point());
    };
    let first_line = FloatLine::new(from_corner, current_tangential_point);
    let to_corner = to_corner?;
    let Some(current_tangential_point) =
        right_tangential(to_corner, next_corner, 2.0 * dist + C_TOLERANCE)
    else {
        return Some(walk.same_as_current_from_point());
    };
    let second_line = FloatLine::new(to_corner, current_tangential_point).translate(dist);
    first_line
        .intersection(&second_line)
        .map(|point| walk.fresh(point))
}

fn left_turn_next_corner(
    walk: &mut LocatorWalk<'_>,
    from_corner: FloatPoint,
    dist: f64,
    to_corner: Option<FloatPoint>,
    next_corner: Option<FloatPoint>,
) -> Option<LocatedCorner> {
    let Some(current_tangential_point) = right_tangential(from_corner, to_corner, dist) else {
        return Some(walk.same_as_current_from_point());
    };
    let first_line = FloatLine::new(from_corner, current_tangential_point);
    let to_corner = to_corner?;
    let Some(current_tangential_point) =
        left_tangential(to_corner, next_corner, 2.0 * dist + C_TOLERANCE)
    else {
        return Some(walk.same_as_current_from_point());
    };
    let second_line = FloatLine::new(to_corner, current_tangential_point).translate(-dist);
    first_line
        .intersection(&second_line)
        .map(|point| walk.fresh(point))
}

fn right_left_tangential_point(
    from_point: FloatPoint,
    to_point: FloatPoint,
    center: Option<FloatPoint>,
    dist: f64,
) -> Option<FloatPoint> {
    let first = right_tangential(from_point, center, dist)?;
    let first_line = FloatLine::new(from_point, first);
    let second = left_tangential(to_point, center, dist)?;
    let second_line = FloatLine::new(to_point, second);
    first_line.intersection(&second_line)
}

fn left_right_tangential_point(
    from_point: FloatPoint,
    to_point: FloatPoint,
    center: Option<FloatPoint>,
    dist: f64,
) -> Option<FloatPoint> {
    let first = left_tangential(from_point, center, dist)?;
    let first_line = FloatLine::new(from_point, first);
    let second = right_tangential(to_point, center, dist)?;
    let second_line = FloatLine::new(to_point, second);
    first_line.intersection(&second_line)
}

#[allow(clippy::too_many_lines)]
pub(crate) fn calculate_next_trace_corners(
    walk: &mut LocatorWalk<'_>,
    backtrack_array: &[BacktrackElement],
) -> Vec<LocatedCorner> {
    let mut result: Vec<LocatedCorner> = Vec::new();
    if walk.current_to_door_index >= walk.current_target_door_index {
        if walk.current_to_door_index == walk.current_target_door_index {
            let nearest_point = walk
                .current_target_shape
                .nearest_point(&walk.current_from_point.round().into())
                .expect("the target shape is non-empty")
                .to_float();
            walk.current_to_door_index += 1;
            let corner = walk.fresh(nearest_point);
            result.push(corner);
        }
        return result;
    }

    let trace_halfwidth_exact =
        f64::from(walk.ctrl.compensated_trace_half_width[walk.current_trace_layer]);
    let trace_halfwidth_max = trace_halfwidth_exact + f64::from(TRACE_WIDTH_TOLERANCE);
    let trace_halfwidth_middle = trace_halfwidth_exact + C_TOLERANCE;

    let to_index =
        usize::try_from(walk.current_to_door_index).expect("the door index is non-negative");
    let current_to_info = backtrack_array[to_index];
    let first_left_corner = calc_door_left_corner(walk, &current_to_info);
    let first_right_corner = calc_door_right_corner(walk, &current_to_info);
    let mut door_left_corner = Some(first_left_corner);
    let mut door_right_corner = Some(first_right_corner);
    if walk
        .current_from_point
        .side_of(&first_left_corner, &first_right_corner)
        != Side::OnTheRight
    {
        if let Some(left) = door_left_corner
            && walk
                .current_from_point
                .scalar_product(&walk.previous_from_point, &left)
                >= 0.0
        {
            door_left_corner = None;
        }
        if let Some(right) = door_right_corner
            && walk
                .current_from_point
                .scalar_product(&walk.previous_from_point, &right)
                >= 0.0
        {
            door_right_corner = None;
        }
        if door_left_corner.is_none() && door_right_corner.is_none() {
            walk.current_to_door_index += 1;
            let corner = walk.same_as_current_from_point();
            result.push(corner);
            return result;
        }
    }

    let mut end_of_trace = false;
    let mut left_tangent_point: Option<FloatPoint>;
    let mut right_tangent_point: Option<FloatPoint>;
    let mut new_door_ind = walk.current_to_door_index;
    let mut left_ind = new_door_ind;
    let mut right_ind = new_door_ind;
    let mut current_door_ind = walk.current_to_door_index + 1;
    let mut result_corner: Option<LocatedCorner> = None;

    loop {
        left_tangent_point = right_tangential(
            walk.current_from_point,
            door_left_corner,
            trace_halfwidth_max,
        );
        if door_left_corner.is_some() && left_tangent_point.is_none() {
            left_tangent_point = door_left_corner;
        }
        right_tangent_point = left_tangential(
            walk.current_from_point,
            door_right_corner,
            trace_halfwidth_max,
        );
        if door_right_corner.is_some() && right_tangent_point.is_none() {
            right_tangent_point = door_right_corner;
        }
        if let (Some(left), Some(right)) = (left_tangent_point, right_tangent_point)
            && right.side_of(&walk.current_from_point, &left) != Side::OnTheRight
        {
            let left_corner_distance = door_left_corner
                .expect("a non-null left tangent point implies a non-null left corner")
                .distance(&walk.current_from_point);
            let right_corner_distance = door_right_corner
                .expect("a non-null right tangent point implies a non-null right corner")
                .distance(&walk.current_from_point);
            if left_corner_distance <= right_corner_distance {
                new_door_ind = left_ind;
                result_corner = left_turn_next_corner(
                    walk,
                    walk.current_from_point,
                    trace_halfwidth_max,
                    door_left_corner,
                    door_right_corner,
                );
            } else {
                new_door_ind = right_ind;
                result_corner = right_turn_next_corner(
                    walk,
                    walk.current_from_point,
                    trace_halfwidth_max,
                    door_right_corner,
                    door_left_corner,
                );
            }
            break;
        }
        if current_door_ind >= walk.current_target_door_index {
            end_of_trace = true;
            break;
        }
        let next_index = usize::try_from(current_door_ind).expect("the door index is non-negative");
        let next_to_info = backtrack_array[next_index];
        let first_next_left = calc_door_left_corner(walk, &next_to_info);
        let first_next_right = calc_door_right_corner(walk, &next_to_info);
        let mut next_left_corner = Some(first_next_left);
        let mut next_right_corner = Some(first_next_right);
        if walk
            .current_from_point
            .side_of(&first_next_left, &first_next_right)
            != Side::OnTheRight
        {
            if door_left_corner.is_none()
                && let Some(next_left) = next_left_corner
                && walk
                    .current_from_point
                    .scalar_product(&walk.previous_from_point, &next_left)
                    >= 0.0
            {
                next_left_corner = None;
            }
            if door_right_corner.is_none()
                && let Some(next_right) = next_right_corner
                && walk
                    .current_from_point
                    .scalar_product(&walk.previous_from_point, &next_right)
                    >= 0.0
            {
                next_right_corner = None;
            }
            if next_left_corner.is_none() && next_right_corner.is_none() {
                walk.current_to_door_index += 1;
                let corner = walk.same_as_current_from_point();
                result.push(corner);
                return result;
            }
        }
        if let (Some(left), Some(right)) = (door_left_corner, door_right_corner) {
            let next_left = next_left_corner
                .expect("nextLeftCorner is nulled only when doorLeftCorner is null");
            let next_right = next_right_corner
                .expect("nextRightCorner is nulled only when doorRightCorner is null");
            if next_left.side_of(&walk.current_from_point, &right) == Side::OnTheRight {
                new_door_ind = right_ind + 1;
                result_corner = right_turn_next_corner(
                    walk,
                    walk.current_from_point,
                    trace_halfwidth_max,
                    door_right_corner,
                    next_left_corner,
                );
                break;
            }
            if next_right.side_of(&walk.current_from_point, &left) == Side::OnTheLeft {
                new_door_ind = left_ind + 1;
                result_corner = left_turn_next_corner(
                    walk,
                    walk.current_from_point,
                    trace_halfwidth_max,
                    door_left_corner,
                    next_right_corner,
                );
                break;
            }
        }
        let mut visability_range_gets_smaller_on_the_right_side = door_right_corner.is_none();
        if let Some(right) = door_right_corner
            && next_right_corner
                .expect("nextRightCorner is nulled only when doorRightCorner is null")
                .side_of(&walk.current_from_point, &right)
                != Side::OnTheRight
            && let Some(current_tangential_point) = left_tangential(
                walk.current_from_point,
                next_right_corner,
                trace_halfwidth_max,
            )
        {
            let check_line = FloatLine::new(walk.current_from_point, current_tangential_point);
            if check_line.segment_distance(&right) >= trace_halfwidth_max {
                visability_range_gets_smaller_on_the_right_side = true;
            }
        }
        if visability_range_gets_smaller_on_the_right_side {
            door_right_corner = next_right_corner;
            right_ind = current_door_ind;
        }
        let mut visability_range_gets_smaller_on_the_left_side = door_left_corner.is_none();
        if let Some(left) = door_left_corner
            && next_left_corner
                .expect("nextLeftCorner is nulled only when doorLeftCorner is null")
                .side_of(&walk.current_from_point, &left)
                != Side::OnTheLeft
            && let Some(current_tangential_point) = right_tangential(
                walk.current_from_point,
                next_left_corner,
                trace_halfwidth_max,
            )
        {
            let check_line = FloatLine::new(walk.current_from_point, current_tangential_point);
            if check_line.segment_distance(&left) >= trace_halfwidth_max {
                visability_range_gets_smaller_on_the_left_side = true;
            }
        }
        if visability_range_gets_smaller_on_the_left_side {
            door_left_corner = next_left_corner;
            left_ind = current_door_ind;
        }
        current_door_ind += 1;
    }

    if end_of_trace {
        let nearest_point = walk
            .current_target_shape
            .nearest_point(&walk.current_from_point.round().into())
            .expect("the target shape is non-empty")
            .to_float();
        result_corner = Some(walk.fresh(nearest_point));
        if left_tangent_point.is_some_and(|left| {
            nearest_point.side_of(&walk.current_from_point, &left) == Side::OnTheLeft
        }) {
            new_door_ind = left_ind + 1;
            let target_right_corner = walk
                .current_target_shape
                .corner_approx(
                    walk.current_target_shape
                        .index_of_right_most_corner(&walk.current_from_point),
                )
                .expect("the target shape has corners");
            let current_corner = right_left_tangential_point(
                walk.current_from_point,
                target_right_corner,
                door_left_corner,
                trace_halfwidth_max,
            );
            if let Some(corner) = current_corner {
                result_corner = Some(walk.fresh(corner));
                end_of_trace = false;
            }
        } else if right_tangent_point.is_some_and(|right| {
            nearest_point.side_of(&walk.current_from_point, &right) == Side::OnTheRight
        }) {
            let target_left_corner = walk
                .current_target_shape
                .corner_approx(
                    walk.current_target_shape
                        .index_of_left_most_corner(&walk.current_from_point),
                )
                .expect("the target shape has corners");
            new_door_ind = right_ind + 1;
            let current_corner = left_right_tangential_point(
                walk.current_from_point,
                target_left_corner,
                door_right_corner,
                trace_halfwidth_max,
            );
            if let Some(corner) = current_corner {
                result_corner = Some(walk.fresh(corner));
                end_of_trace = false;
            }
        }
    }
    if end_of_trace {
        new_door_ind = walk.current_target_door_index;
    }

    let check_from_door_index = walk
        .current_to_door_index
        .saturating_sub(5)
        .max(walk.current_from_door_index + 1);
    let mut corrected_result: Option<FloatPoint> = None;
    let mut corrected_door_ind: i32 = 0;
    if check_from_door_index < new_door_ind
        && let Some(result_corner_value) = result_corner
    {
        let check_line = FloatLine::new(walk.current_from_point, result_corner_value.point);
        for i in check_from_door_index..new_door_ind {
            let index = usize::try_from(i).expect("the door index is non-negative");
            let element = backtrack_array[index];
            let current_left_corner = calc_door_left_corner(walk, &element);
            let current_distance = check_line.segment_distance(&current_left_corner);
            if current_distance.abs() < trace_halfwidth_middle
                && let Some(current_corrected_result) = right_left_tangential_point(
                    check_line.a,
                    check_line.b,
                    Some(current_left_corner),
                    trace_halfwidth_max,
                )
                && (corrected_result.is_none()
                    || corrected_result.is_some_and(|previous| {
                        current_corrected_result.side_of(&walk.current_from_point, &previous)
                            == Side::OnTheRight
                    }))
            {
                corrected_door_ind = i;
                corrected_result = Some(current_corrected_result);
            }
            let current_right_corner = calc_door_right_corner(walk, &element);
            let current_distance = check_line.segment_distance(&current_right_corner);
            if current_distance.abs() < trace_halfwidth_middle
                && let Some(current_corrected_result) = left_right_tangential_point(
                    check_line.a,
                    check_line.b,
                    Some(current_right_corner),
                    trace_halfwidth_max,
                )
                && (corrected_result.is_none()
                    || corrected_result.is_some_and(|previous| {
                        current_corrected_result.side_of(&walk.current_from_point, &previous)
                            == Side::OnTheLeft
                    }))
            {
                corrected_door_ind = i;
                corrected_result = Some(current_corrected_result);
            }
        }
    }
    if let Some(corrected) = corrected_result {
        result_corner = Some(walk.fresh(corrected));
        new_door_ind = corrected_door_ind.max(walk.current_to_door_index);
    }

    walk.current_to_door_index = new_door_ind;
    if let Some(corner) = result_corner
        && corner.id != walk.current_from_id
    {
        result.push(corner);
    }
    result
}
