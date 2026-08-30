//! Port of `autoroute.path.FoundConnectionLocatorAnyAngle`
//! (FoundConnectionLocatorAnyAngle.java:24-454): "calculates from the backtrack list the location
//! of the traces and vias, which realize a connection found by the maze search algorithm."
//!
//! It is the override `FoundConnectionLocator.getInstance` (`:201-205`) picks for every angle
//! restriction other than 90° and 45°. Where `super::locator_45` walks door by door and bends
//! at each one, this one advances the door index as far as it can still *see* through, and lays
//! a single straight line across the whole visible run.
//!
//! Java's class, like the 45° one, has no state of its own — the constructor is a bare
//! `super(...)` (`:29-37`) — so the port is free functions over
//! `LocatorWalk`.
//!
//! # Nulls, and where a null is a crash rather than a value
//!
//! `doorLeftCorner`/`doorRightCorner` become `null` **mid-method** (`:92`, `:96`) and every later
//! use is guarded, so both are `Option<FloatPoint>` here and the four tangential helpers take an
//! `Option` centre — `FloatPoint.leftTangentialPoint(null, d)` answers `null`
//! (FloatPoint.java:408-410, `:429-431`), which is what the port's `?` does.
//!
//! Everywhere **else** a null in this file is a Java `NullPointerException`, not a value, and the
//! port panics rather than degrading. `FloatPoint.sideOf` (FloatPoint.java:264-271) dereferences
//! `p1` on its first line; `FloatLine.segmentDistance` (FloatLine.java:113-122) dereferences the
//! line's own `b` through `perpendicularProjection`; and `calcDoorLeftCorner`/`calcDoorRightCorner`
//! (`:43-61`) dereference `fromRoom` at `:45`/`:57` with no guard. Java's degraded value at each
//! of those is *not* a different route but the whole connection failing —
//! `AutorouteEngine.autorouteConnection:189-195` catches the throw into
//! `AutorouteAttemptState.FAILED` — so a port that quietly answered `None` and carried on would
//! route a wire Java does not. Plan-6 ruling 7's `catch_unwind` around
//! [`FoundConnectionLocator::get_instance`] is what turns these panics back into that `FAILED`.
//! None of them is a totalization site (the crate's `totalized` marker), because no degraded
//! value here matches Java: the only faithful outcome is the throw.
//!
//! [`FoundConnectionLocator::get_instance`]:
//!     crate::autoroute::path::FoundConnectionLocator::get_instance

use fr_geometry::{FloatLine, FloatPoint, PolylineShapeOps, Side, TileShape};

use crate::autoroute::maze::TRACE_WIDTH_TOLERANCE;
use crate::autoroute::path::locator::{BacktrackElement, LocatedCorner, LocatorWalk};

/// `private static final double cTolerance = 1.0` (`:26`).
const C_TOLERANCE: f64 = 1.0;

/// The `fromRoom` prologue both corner helpers share (`:44-45` = `:56-57`), plus the door shape
/// they read (`:46` = `:58`).
///
/// # Panics
///
/// Java has **no** guard on any of the three steps. `toInfo.nextRoom` may be null — the probe
/// prints one (`p6t14-locator.txt`, the last backtrack element, because
/// `TargetItemExpansionDoor.otherRoom` answers null unconditionally,
/// TargetItemExpansionDoor.java:50-53) — and `ExpansionDoor.otherRoom(null)` (`:62-71`) matches
/// neither room and answers null again, so `fromRoom.getShape()` at `:45` throws. The port panics
/// at the same point, and ruling 7's `catch_unwind` turns that into the `FAILED` Java's
/// `AutorouteEngine.autorouteConnection:189-195` produces.
///
/// The invariant that keeps it unreachable is an **index** one, not a shape one: this file's two
/// callers index `backtrackArray` at `currentToDoorIndex` and at `i < newDoorInd`, and both are
/// strictly below `currentTargetDoorIndex`, which the constructor pins at
/// `backtrackArray.length - 1` (`:163`). The one element with a null `nextRoom` is exactly that
/// last one — the start door — so it is never passed here. Nothing else asserts it, which is why
/// this panics rather than answering a value Java never produces.
fn door_pole_and_shape(
    walk: &LocatorWalk<'_>,
    to_info: &BacktrackElement,
) -> (FloatPoint, TileShape) {
    // :44-45 = :56-57.
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
    // :46 = :58.
    let shape = walk.engine.expandable_shape(to_info.door).expect(
        "FoundConnectionLocatorAnyAngle.calcDoorLeftCorner: the door has no shape — Java throws a \
         NullPointerException at FoundConnectionLocatorAnyAngle.java:47",
    );
    (pole, shape)
}

/// Port of the private static `calcDoorLeftCorner(BacktrackElement)` (`:43-49`): "calculates the
/// left most corner of the shape of toInfo.door seen from the center of the common room with the
/// previous door."
///
/// # Panics
///
/// See [`door_pole_and_shape`]. `TileShape.cornerApprox` also answers `null` for a line-less
/// simplex (Simplex.java:182-184), and Java lets that null out of the method — but every one of
/// its four uses dereferences it immediately (`sideOf` at `:87` and `:165`, `segmentDistance` at
/// `:293` and `:308`), so the throw is only deferred by a line and the port takes it here.
fn calc_door_left_corner(walk: &LocatorWalk<'_>, to_info: &BacktrackElement) -> FloatPoint {
    let (pole, shape) = door_pole_and_shape(walk, to_info);
    // :47-48.
    let left_most_corner_no = shape.index_of_left_most_corner(&pole);
    shape.corner_approx(left_most_corner_no).expect(
        "FoundConnectionLocatorAnyAngle.calcDoorLeftCorner: cornerApprox is null for a line-less \
         simplex (Simplex.java:182-184), and every caller dereferences it at once",
    )
}

/// Port of the private static `calcDoorRightCorner(BacktrackElement)` (`:55-61`).
///
/// # Panics
///
/// See [`calc_door_left_corner`].
fn calc_door_right_corner(walk: &LocatorWalk<'_>, to_info: &BacktrackElement) -> FloatPoint {
    let (pole, shape) = door_pole_and_shape(walk, to_info);
    // :59-60.
    let right_most_corner_no = shape.index_of_right_most_corner(&pole);
    shape.corner_approx(right_most_corner_no).expect(
        "FoundConnectionLocatorAnyAngle.calcDoorRightCorner: cornerApprox is null for a line-less \
         simplex (Simplex.java:182-184), and every caller dereferences it at once",
    )
}

/// `FloatPoint.leftTangentialPoint(FloatPoint, double)` with Java's null-argument arm
/// (FloatPoint.java:408-410).
fn left_tangential(from: FloatPoint, to: Option<FloatPoint>, distance: f64) -> Option<FloatPoint> {
    from.left_tangential_point(&to?, distance)
}

/// `FloatPoint.rightTangentialPoint(FloatPoint, double)`, likewise (FloatPoint.java:429-431).
fn right_tangential(from: FloatPoint, to: Option<FloatPoint>, distance: f64) -> Option<FloatPoint> {
    from.right_tangential_point(&to?, distance)
}

/// Port of the private `rightTurnNextCorner(FloatPoint, double, FloatPoint, FloatPoint)`
/// (`:365-383`): "calculates as first line the left side tangent from fromCorner to the circle
/// with center toCorner and radius dist. As second line the right side tangent from toCorner to
/// the circle with center nextCorner and radius 2 * dist is constructed. The second line is than
/// translated by the distance dist to the left. Returned is the intersection of the first and the
/// second line."
///
/// The two `null` arms (`:368-372`, `:375-379`) answer **`fromCorner` itself** — the object
/// identity `FoundConnectionLocator:432` filters on — so the port hands back a
/// [`LocatedCorner`] rather than a bare point.
fn right_turn_next_corner(
    walk: &mut LocatorWalk<'_>,
    from_corner: FloatPoint,
    dist: f64,
    to_corner: Option<FloatPoint>,
    next_corner: Option<FloatPoint>,
) -> Option<LocatedCorner> {
    // :367-372.
    let Some(current_tangential_point) = left_tangential(from_corner, to_corner, dist) else {
        return Some(walk.same_as_current_from_point());
    };
    // :373.
    let first_line = FloatLine::new(from_corner, current_tangential_point);
    // :374-379.
    let to_corner = to_corner?;
    let Some(current_tangential_point) =
        right_tangential(to_corner, next_corner, 2.0 * dist + C_TOLERANCE)
    else {
        return Some(walk.same_as_current_from_point());
    };
    // :380-382.
    let second_line = FloatLine::new(to_corner, current_tangential_point).translate(dist);
    first_line
        .intersection(&second_line)
        .map(|point| walk.fresh(point))
}

/// Port of the private `leftTurnNextCorner(FloatPoint, double, FloatPoint, FloatPoint)`
/// (`:391-408`) — the mirror of [`right_turn_next_corner`], translating the second line by
/// `-dist`.
fn left_turn_next_corner(
    walk: &mut LocatorWalk<'_>,
    from_corner: FloatPoint,
    dist: f64,
    to_corner: Option<FloatPoint>,
    next_corner: Option<FloatPoint>,
) -> Option<LocatedCorner> {
    // :393-398.
    let Some(current_tangential_point) = right_tangential(from_corner, to_corner, dist) else {
        return Some(walk.same_as_current_from_point());
    };
    // :399.
    let first_line = FloatLine::new(from_corner, current_tangential_point);
    // :400-404.
    let to_corner = to_corner?;
    let Some(current_tangential_point) =
        left_tangential(to_corner, next_corner, 2.0 * dist + C_TOLERANCE)
    else {
        return Some(walk.same_as_current_from_point());
    };
    // :405-407.
    let second_line = FloatLine::new(to_corner, current_tangential_point).translate(-dist);
    first_line
        .intersection(&second_line)
        .map(|point| walk.fresh(point))
}

/// Port of the private `rightLeftTangentialPoint(FloatPoint, FloatPoint, FloatPoint, double)`
/// (`:414-431`): "calculates the right tangential line from fromPoint and the left tangential
/// line from toPoint to the circle with center center and radius dist. Returns the intersection
/// of the 2 lines."
fn right_left_tangential_point(
    from_point: FloatPoint,
    to_point: FloatPoint,
    center: Option<FloatPoint>,
    dist: f64,
) -> Option<FloatPoint> {
    // :416-422.
    let first = right_tangential(from_point, center, dist)?;
    let first_line = FloatLine::new(from_point, first);
    // :423-430.
    let second = left_tangential(to_point, center, dist)?;
    let second_line = FloatLine::new(to_point, second);
    first_line.intersection(&second_line)
}

/// Port of the private `leftRightTangentialPoint(FloatPoint, FloatPoint, FloatPoint, double)`
/// (`:437-454`).
fn left_right_tangential_point(
    from_point: FloatPoint,
    to_point: FloatPoint,
    center: Option<FloatPoint>,
    dist: f64,
) -> Option<FloatPoint> {
    // :439-445.
    let first = left_tangential(from_point, center, dist)?;
    let first_line = FloatLine::new(from_point, first);
    // :446-453.
    let second = right_tangential(to_point, center, dist)?;
    let second_line = FloatLine::new(to_point, second);
    first_line.intersection(&second_line)
}

/// Port of `calculateNextTraceCorners()` (`:67-356`) — the free-angle override of
/// `FoundConnectionLocator.calculateNextTraceCorners` (FoundConnectionLocator.java:499):
/// "calculates a list with the next point of the trace under construction. If the trace is
/// completed, the result list will be empty."
///
/// `// not ported:` `calculateNextTraceCorners` — the net-33/66/67 `FRLogger.trace` of
/// `:332-354` and the four diagnostic `FRLogger.trace` calls of `:125-135`, `:181-182`
/// (plan-6 ruling 14 and the global no-logger constraint).
///
/// # Panics
///
/// Java bug: `FoundConnectionLocatorAnyAngle.calculateNextTraceCorners` — `:287` builds
/// `new FloatLine(this.currentFromPoint, resultCorner)` with **no null check**, although the
/// only two producers of `resultCorner` before that line
/// ([`right_turn_next_corner`]/[`left_turn_next_corner`], through
/// `FloatLine.intersection`, FloatLine.java:53-55) return `null` for two parallel lines and
/// `:329` guards for exactly that. The `FloatLine` constructor stores the null happily; the very
/// next `checkLine.segmentDistance(...)` at `:293` throws a `NullPointerException`, which
/// `AutorouteEngine.autorouteConnection:189-194` catches into a `FAILED` attempt. The port
/// panics at the same point (and only when the loop at `:291` would actually iterate, exactly as
/// Java does), which plan-6 ruling 7's `catch_unwind` around `getInstance` turns back into that
/// same `FAILED`. See `docs/java-quirks.md` #181.
#[allow(clippy::too_many_lines)]
pub(crate) fn calculate_next_trace_corners(
    walk: &mut LocatorWalk<'_>,
    backtrack_array: &[BacktrackElement],
) -> Vec<LocatedCorner> {
    let mut result: Vec<LocatedCorner> = Vec::new();
    // :70-78.
    if walk.current_to_door_index >= walk.current_target_door_index {
        if walk.current_to_door_index == walk.current_target_door_index {
            // :72-73.
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

    // :80-82.
    let trace_halfwidth_exact =
        f64::from(walk.ctrl.compensated_trace_half_width[walk.current_trace_layer]);
    let trace_halfwidth_max = trace_halfwidth_exact + f64::from(TRACE_WIDTH_TOLERANCE);
    let trace_halfwidth_middle = trace_halfwidth_exact + C_TOLERANCE;

    // :84-86.
    let to_index =
        usize::try_from(walk.current_to_door_index).expect("the door index is non-negative");
    let current_to_info = backtrack_array[to_index];
    let first_left_corner = calc_door_left_corner(walk, &current_to_info);
    let first_right_corner = calc_door_right_corner(walk, &current_to_info);
    let mut door_left_corner = Some(first_left_corner);
    let mut door_right_corner = Some(first_right_corner);
    // :87-104. Both corners are still the fresh, non-null ones `:85-86` produced, so Java's
    // `sideOf` here cannot see a null; the two `Option`s exist for `:92`/`:96` below.
    if walk
        .current_from_point
        .side_of(&first_left_corner, &first_right_corner)
        != Side::OnTheRight
    {
        // "the door is already crossed at this.fromPoint"
        // :89-93: "also the left corner of the door is passed. That may not be the case if the
        // door line is crossed almost parallel."
        if let Some(left) = door_left_corner
            && walk
                .current_from_point
                .scalar_product(&walk.previous_from_point, &left)
                >= 0.0
        {
            door_left_corner = None;
        }
        // :94-97.
        if let Some(right) = door_right_corner
            && walk
                .current_from_point
                .scalar_product(&walk.previous_from_point, &right)
                >= 0.0
        {
            door_right_corner = None;
        }
        // :98-103: "the door is completely passed."
        if door_left_corner.is_none() && door_right_corner.is_none() {
            walk.current_to_door_index += 1;
            let corner = walk.same_as_current_from_point();
            result.push(corner);
            return result;
        }
    }

    // :106-118. "Calculate the visibility range for a trace line from currentFromPoint through
    // the interval from left_most_visible_point to right_most_visible_point, by advancing the
    // door index as far as possible, so that still something is visible."
    let mut end_of_trace = false;
    let mut left_tangent_point: Option<FloatPoint>;
    let mut right_tangent_point: Option<FloatPoint>;
    let mut new_door_ind = walk.current_to_door_index;
    let mut left_ind = new_door_ind;
    let mut right_ind = new_door_ind;
    let mut current_door_ind = walk.current_to_door_index + 1;
    let mut result_corner: Option<LocatedCorner> = None;

    // :121-244: "construct a maximum length straight line through the doors".
    loop {
        // :122-128.
        left_tangent_point = right_tangential(
            walk.current_from_point,
            door_left_corner,
            trace_halfwidth_max,
        );
        if door_left_corner.is_some() && left_tangent_point.is_none() {
            left_tangent_point = door_left_corner;
        }
        // :129-135.
        right_tangent_point = left_tangential(
            walk.current_from_point,
            door_right_corner,
            trace_halfwidth_max,
        );
        if door_right_corner.is_some() && right_tangent_point.is_none() {
            right_tangent_point = door_right_corner;
        }
        // :136-157.
        if let (Some(left), Some(right)) = (left_tangent_point, right_tangent_point)
            && right.side_of(&walk.current_from_point, &left) != Side::OnTheRight
        {
            // "the gap between left_most_visible_point and right_most_visible_point is too small
            // for a trace with the current half width."
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
        // :158-161.
        if current_door_ind >= walk.current_target_door_index {
            end_of_trace = true;
            break;
        }
        // :162-164.
        let next_index = usize::try_from(current_door_ind).expect("the door index is non-negative");
        let next_to_info = backtrack_array[next_index];
        let first_next_left = calc_door_left_corner(walk, &next_to_info);
        let first_next_right = calc_door_right_corner(walk, &next_to_info);
        let mut next_left_corner = Some(first_next_left);
        let mut next_right_corner = Some(first_next_right);
        // :165-187. Fresh and non-null again, for the same reason as `:87`.
        if walk
            .current_from_point
            .side_of(&first_next_left, &first_next_right)
            != Side::OnTheRight
        {
            // "the door may be already crossed at this.fromPoint"
            // :167-172.
            if door_left_corner.is_none()
                && let Some(next_left) = next_left_corner
                && walk
                    .current_from_point
                    .scalar_product(&walk.previous_from_point, &next_left)
                    >= 0.0
            {
                next_left_corner = None;
            }
            // :173-177.
            if door_right_corner.is_none()
                && let Some(next_right) = next_right_corner
                && walk
                    .current_from_point
                    .scalar_product(&walk.previous_from_point, &next_right)
                    >= 0.0
            {
                next_right_corner = None;
            }
            // :178-186: "should not happen because the previous door was not passed completely."
            if next_left_corner.is_none() && next_right_corner.is_none() {
                walk.current_to_door_index += 1;
                let corner = walk.same_as_current_from_point();
                result.push(corner);
                return result;
            }
        }
        // :188-208: "otherwise the following sideOf conditions may not be correct even if all
        // parameter points are defined".
        if let (Some(left), Some(right)) = (door_left_corner, door_right_corner) {
            let next_left = next_left_corner
                .expect("nextLeftCorner is nulled only when doorLeftCorner is null");
            let next_right = next_right_corner
                .expect("nextRightCorner is nulled only when doorRightCorner is null");
            // :191-198: "bend to the right".
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
            // :200-207: "bend to the left".
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
        // :209-225.
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
            // :221-225: "the visibility range gets smaller on the right side."
            door_right_corner = next_right_corner;
            right_ind = current_door_ind;
        }
        // :226-242.
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
            // :238-242: "the visibility range gets smaller on the left side."
            door_left_corner = next_left_corner;
            left_ind = current_door_ind;
        }
        // :243.
        current_door_ind += 1;
    }

    // :246-279.
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
            // :250-263: "the nearest target point is to the left of the visible range, add
            // another corner".
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
            // :264-278: "the nearest target point is to the right of the visible range, add
            // another corner".
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
    // :280-282.
    if end_of_trace {
        new_door_ind = walk.current_target_door_index;
    }

    // :284-326: "check clearance violation with the previous door shapes and correct them in this
    // case."
    let check_from_door_index = walk
        .current_to_door_index
        .saturating_sub(5)
        .max(walk.current_from_door_index + 1);
    let mut corrected_result: Option<FloatPoint> = None;
    let mut corrected_door_ind: i32 = 0;
    if check_from_door_index < new_door_ind {
        // :287. See this function's `# Panics`: Java has no guard here and NPEs at `:293`.
        let result_point = result_corner
            .expect(
                "FoundConnectionLocatorAnyAngle.calculateNextTraceCorners: resultCorner is null \
                 at :287 — Java throws a NullPointerException at :293 (docs/java-quirks.md #181)",
            )
            .point;
        let check_line = FloatLine::new(walk.current_from_point, result_point);
        for i in check_from_door_index..new_door_ind {
            let index = usize::try_from(i).expect("the door index is non-negative");
            let element = backtrack_array[index];
            // :292-306. Java dereferences the corner on the next line, with no guard.
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
            // :307-321.
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
    // :323-326.
    if let Some(corrected) = corrected_result {
        result_corner = Some(walk.fresh(corrected));
        new_door_ind = corrected_door_ind.max(walk.current_to_door_index);
    }

    // :328-331.
    walk.current_to_door_index = new_door_ind;
    if let Some(corner) = result_corner
        && corner.id != walk.current_from_id
    {
        result.push(corner);
    }
    result
}
