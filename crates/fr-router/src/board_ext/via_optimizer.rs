use fr_board::items::Item;
use fr_board::prelude::*;
use fr_board::{BoardError, ItemId};
use fr_geometry::{FloatLine, FloatPoint, IntPoint, Point, Side, java_min};
use fr_settings::ExpansionCostFactor;

use crate::board_ext::drill_item_mover::DrillItemMover;
use crate::board_ext::tightener::PolylineTraceExt;

pub struct ViaOptimizer;

impl ViaOptimizer {
    pub fn opt_via_location(
        board: &mut Board,
        via: ItemId,
        trace_costs: Option<&[ExpansionCostFactor]>,
        trace_pull_tight_accuracy: i32,
        max_recursion_depth: i32,
    ) -> Result<bool, BoardError> {
        let Some(item) = board.get_item(via) else {
            return Ok(false);
        };
        if !matches!(item, Item::Via(_)) {
            return Ok(false);
        }
        if item.is_shove_fixed(&board.rules) {
            return Ok(false);
        }
        if max_recursion_depth <= 0 {
            return Ok(false);
        }
        let contacts: Vec<ItemId> = board.normal_contacts(via).into_iter().rev().collect();
        let mut is_plane_or_fanout_via = contacts.len() == 1;
        let mut first_trace: Option<ItemId> = None;
        let mut second_trace: Option<ItemId> = None;
        if !is_plane_or_fanout_via {
            if contacts.len() != 2 {
                return Ok(false);
            }
            for (slot, current_item) in [&mut first_trace, &mut second_trace]
                .into_iter()
                .zip(contacts.iter().copied())
            {
                match Self::contact_role(board, current_item) {
                    ContactRole::FreeTrace => *slot = Some(current_item),
                    ContactRole::Plane => is_plane_or_fanout_via = true,
                    ContactRole::Unusable => return Ok(false),
                }
            }
        }
        if is_plane_or_fanout_via {
            return Self::opt_plane_or_fanout_via(
                board,
                via,
                trace_pull_tight_accuracy,
                max_recursion_depth,
            );
        }
        let (Some(first_trace), Some(second_trace)) = (first_trace, second_trace) else {
            return Ok(false);
        };
        let Some(via_center) = board.drill_center(via) else {
            return Ok(false);
        };
        let first_layer = Self::trace_layer(board, first_trace);
        let second_layer = Self::trace_layer(board, second_trace);

        let tolerance = Self::via_tolerance(board, via);

        let Some(first_trace_from_corner) =
            Self::from_corner(board, first_trace, &via_center, tolerance)
        else {
            return Ok(false);
        };
        let Some(second_trace_from_corner) =
            Self::from_corner(board, second_trace, &via_center, tolerance)
        else {
            return Ok(false);
        };

        let (first_layer_trace_costs, second_layer_trace_costs) = match trace_costs {
            Some(costs) => (costs[first_layer], costs[second_layer]),
            None => {
                let unit = ExpansionCostFactor {
                    horizontal: 1.0,
                    vertical: 1.0,
                };
                (unit, unit)
            }
        };

        let new_location = Self::reposition_via_general(
            board,
            via,
            Self::trace_half_width(board, first_trace),
            Self::trace_clearance_class(board, first_trace),
            first_layer,
            first_layer_trace_costs,
            &first_trace_from_corner,
            Self::trace_half_width(board, second_trace),
            Self::trace_clearance_class(board, second_trace),
            second_layer,
            second_layer_trace_costs,
            &second_trace_from_corner,
        );
        let Some(new_location) = new_location else {
            return Ok(false);
        };
        if new_location == via_center {
            return Ok(false);
        }
        let delta = new_location.difference_by(&via_center);
        // that reason. `FRLogger.warn("OptViaAlgo.opt_via_location: move via failed")` is dropped.
        if !DrillItemMover::insert(board, via, &delta, 9, 9, None, &|| false)? {
            return Ok(false);
        }
        for layer in [first_layer, second_layer] {
            let picked: Vec<ItemId> = board
                .pick_traces(&new_location, Some(layer))
                .into_iter()
                .rev()
                .collect();
            for current_item in picked {
                <Board as PolylineTraceExt>::pull_tight(
                    board,
                    current_item,
                    true,
                    trace_pull_tight_accuracy,
                    &|| false,
                )?;
            }
        }
        let first_picked_via = board
            .pick_items(&new_location, Some(first_layer))
            .into_iter()
            .rev()
            .find(|id| matches!(board.get_item(*id), Some(Item::Via(_))));
        if let Some(current_item) = first_picked_via {
            Self::opt_via_location(
                board,
                current_item,
                trace_costs,
                trace_pull_tight_accuracy,
                max_recursion_depth - 1,
            )?;
        }
        Ok(true)
    }

    pub fn opt_plane_or_fanout_via(
        board: &mut Board,
        via: ItemId,
        trace_pull_tight_accuracy: i32,
        max_recursion_depth: i32,
    ) -> Result<bool, BoardError> {
        if max_recursion_depth <= 0 {
            return Ok(false);
        }
        let contact_list: Vec<ItemId> = board.normal_contacts(via).into_iter().rev().collect();
        if contact_list.is_empty() {
            return Ok(false);
        }
        let mut contact_plane: Option<ItemId> = None;
        let mut contact_trace: Option<ItemId> = None;
        for current_contact in contact_list {
            match board.get_item(current_contact) {
                Some(Item::ConductionArea(_)) => {
                    if contact_plane.is_some() {
                        return Ok(false);
                    }
                    contact_plane = Some(current_contact);
                }
                Some(item) if item.is_trace() => {
                    if item.is_shove_fixed(&board.rules) || contact_trace.is_some() {
                        return Ok(false);
                    }
                    contact_trace = Some(current_contact);
                }
                _ => return Ok(false),
            }
        }
        let Some(contact_trace) = contact_trace else {
            return Ok(false);
        };
        let Some(via_center) = board.drill_center(via) else {
            return Ok(false);
        };

        let tolerance = Self::via_tolerance(board, via);

        let at_first_corner = {
            let first = Self::trace_corner(board, contact_trace, TraceEnd::First);
            let last = Self::trace_corner(board, contact_trace, TraceEnd::Last);
            if Self::is_within_tolerance(first.as_ref(), &via_center, tolerance) {
                true
            } else if Self::is_within_tolerance(last.as_ref(), &via_center, tolerance) {
                false
            } else {
                return Ok(false);
            }
        };
        let Some(Item::Trace(trace)) = board.get_item(contact_trace) else {
            return Ok(false);
        };
        let trace_polyline = trace.polyline().clone();
        let corner_count = trace_polyline.corner_count();
        let check_corner = if at_first_corner {
            trace_polyline.corner(1)
        } else {
            trace_polyline.corner(corner_count.wrapping_sub(2))
        };
        let Some(check_corner) = check_corner else {
            return Ok(false);
        };
        let rounded_check_corner: IntPoint = check_corner.to_float().round();
        let trace_half_width = Self::trace_half_width(board, contact_trace);
        let trace_layer = Self::trace_layer(board, contact_trace);
        let trace_cl_class_no = Self::trace_clearance_class(board, contact_trace);
        let mut new_via_location = Self::reposition_via_toward_location(
            board,
            via,
            &rounded_check_corner,
            trace_half_width,
            trace_layer,
            trace_cl_class_no,
        );
        if new_via_location.is_none() && corner_count >= 3 {
            let prev_corner = if at_first_corner {
                trace_polyline.corner(2)
            } else {
                trace_polyline.corner(corner_count - 3)
            };
            if let Some(prev_corner) = prev_corner {
                let float_check_corner = check_corner.to_float();
                let float_via_center = via_center.to_float();
                let float_prev_corner = prev_corner.to_float();
                if float_check_corner.scalar_product(&float_via_center, &float_prev_corner) != 0.0 {
                    let current_line = FloatLine::new(float_check_corner, float_prev_corner);
                    let projection = Point::Int(
                        current_line
                            .perpendicular_projection(&float_via_center)
                            .round(),
                    );
                    let diff_vector = projection.difference_by(&via_center);
                    let mut projection_ok = true;
                    let angle_restriction = board.rules.trace_angle_restriction;
                    if projection == via_center
                        || angle_restriction == AngleRestriction::NinetyDegree
                            && !diff_vector.is_orthogonal()
                        || angle_restriction == AngleRestriction::FortyFiveDegree
                            && !diff_vector.is_multiple_of_45_degree()
                    {
                        projection_ok = false;
                    }
                    if projection_ok
                        && DrillItemMover::check(board, via, &diff_vector, 0, 0, None, None)
                    {
                        let net_numbers = Self::net_nos(board, via);
                        let ok_length = board.check_trace_segment(
                            &via_center,
                            &projection,
                            trace_layer,
                            &net_numbers,
                            trace_half_width,
                            trace_cl_class_no,
                            false,
                        );
                        if ok_length >= f64::from(i32::MAX) {
                            new_via_location = Some(projection);
                        }
                    }
                }
            }
        }
        let Some(new_via_location) = new_via_location else {
            return Ok(false);
        };
        if let Some(contact_plane) = contact_plane {
            let plane_layer = match board.get_item(contact_plane) {
                Some(Item::ConductionArea(area)) => area.get_layer(),
                _ => return Ok(false),
            };
            let picked: Vec<ItemId> = board
                .pick_items(&new_via_location, Some(plane_layer))
                .into_iter()
                .rev()
                .filter(|id| matches!(board.get_item(*id), Some(Item::ConductionArea(_))))
                .collect();
            if !picked.contains(&contact_plane) {
                return Ok(false);
            }
        }
        let diff_vector = new_via_location.difference_by(&via_center);
        if !DrillItemMover::insert(board, via, &diff_vector, 9, 9, None, &|| false)? {
            return Ok(false);
        }
        let picked: Vec<ItemId> = board
            .pick_traces(&new_via_location, Some(trace_layer))
            .into_iter()
            .rev()
            .collect();
        for current_item in picked {
            <Board as PolylineTraceExt>::pull_tight(
                board,
                current_item,
                true,
                trace_pull_tight_accuracy,
                &|| false,
            )?;
        }
        if new_via_location == check_corner {
            Self::opt_plane_or_fanout_via(
                board,
                via,
                trace_pull_tight_accuracy,
                max_recursion_depth - 1,
            )?;
        }
        Ok(true)
    }

    pub fn is_within_tolerance(p1: Option<&Point>, p2: &Point, tolerance: i32) -> bool {
        let Some(p1) = p1 else {
            return false;
        };
        let fp1 = p1.to_float();
        let fp2 = p2.to_float();
        let dx = (fp1.x - fp2.x).abs();
        let dy = (fp1.y - fp2.y).abs();
        (dx + dy) <= f64::from(tolerance)
    }

    pub fn reposition_via_toward_location(
        board: &mut Board,
        via: ItemId,
        to_location: &IntPoint,
        trace_half_width: i32,
        trace_layer: usize,
        trace_cl_class: usize,
    ) -> Option<Point> {
        let from_location = board.drill_center(via)?;
        let to_point = Point::Int(*to_location);
        if from_location == to_point {
            return None;
        }
        let net_numbers = Self::net_nos(board, via);
        let mut ok_length = board.check_trace_segment(
            &from_location,
            &to_point,
            trace_layer,
            &net_numbers,
            trace_half_width,
            trace_cl_class,
            false,
        );
        if ok_length <= 0.0 {
            return None;
        }
        let float_from_location = from_location.to_float();
        let float_to_location = to_point.to_float();
        let new_float_to_location = if ok_length >= f64::from(i32::MAX) {
            float_to_location
        } else {
            float_from_location.change_length(&float_to_location, ok_length)
        };
        let new_to_location = Point::Int(new_float_to_location.round());
        let delta = new_to_location.difference_by(&from_location);
        let check_ok = DrillItemMover::check(board, via, &delta, 0, 0, None, None);
        if check_ok {
            return Some(new_to_location);
        }
        let min_length = 0.3 * f64::from(trace_half_width) + 1.0;
        ok_length = java_min(ok_length, float_from_location.distance(&float_to_location));
        let mut current_length = ok_length / 2.0;
        ok_length = 0.0;
        let mut result: Option<Point> = None;
        while current_length >= min_length {
            let check_point = Point::Int(
                float_from_location
                    .change_length(&float_to_location, ok_length + current_length)
                    .round(),
            );
            let delta = check_point.difference_by(&from_location);
            if DrillItemMover::check(board, via, &delta, 0, 0, None, None) {
                ok_length += current_length;
                result = Some(check_point);
            }
            current_length /= 2.0;
        }
        result
    }

    #[allow(clippy::too_many_arguments)]
    pub fn reposition_via_check_candidate(
        board: &mut Board,
        via: ItemId,
        to_location: &IntPoint,
        trace_half_width_1: i32,
        trace_layer_1: usize,
        trace_cl_class_1: usize,
        connect_location: &IntPoint,
        trace_half_width_2: i32,
        trace_layer_2: usize,
        trace_cl_class_2: usize,
    ) -> bool {
        let Some(from_location) = board.drill_center(via) else {
            return false;
        };
        let to_point = Point::Int(*to_location);
        if from_location == to_point {
            return false;
        }
        let delta = to_point.difference_by(&from_location);
        if board.rules.trace_angle_restriction == AngleRestriction::None
            && delta.length_approx() <= 1.5
        {
            return false;
        }
        let net_numbers = Self::net_nos(board, via);
        let ok_length = board.check_trace_segment(
            &from_location,
            &to_point,
            trace_layer_1,
            &net_numbers,
            trace_half_width_1,
            trace_cl_class_1,
            false,
        );
        if ok_length < f64::from(i32::MAX) {
            return false;
        }
        let ok_length = board.check_trace_segment(
            &to_point,
            &Point::Int(*connect_location),
            trace_layer_2,
            &net_numbers,
            trace_half_width_2,
            trace_cl_class_2,
            false,
        );
        if ok_length < f64::from(i32::MAX) {
            return false;
        }
        DrillItemMover::check(board, via, &delta, 0, 0, None, None)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn reposition_via_general(
        board: &mut Board,
        via: ItemId,
        first_trace_half_width: i32,
        first_trace_cl_class: usize,
        first_trace_layer: usize,
        first_trace_costs: ExpansionCostFactor,
        first_trace_from_corner: &Point,
        second_trace_half_width: i32,
        second_trace_cl_class: usize,
        second_trace_layer: usize,
        second_trace_costs: ExpansionCostFactor,
        second_trace_from_corner: &Point,
    ) -> Option<Point> {
        let via_location = board.drill_center(via)?;
        let first_delta = first_trace_from_corner.difference_by(&via_location);
        let second_delta = second_trace_from_corner.difference_by(&via_location);
        let scalar_product = first_delta.scalar_product(&second_delta);
        let float_via_location = via_location.to_float();
        let float_first_trace_from_corner = first_trace_from_corner.to_float();
        let float_second_trace_from_corner = second_trace_from_corner.to_float();
        let first_trace_from_corner_distance =
            float_via_location.distance(&float_first_trace_from_corner);
        let second_trace_from_corner_distance =
            float_via_location.distance(&float_second_trace_from_corner);
        let rounded_first_trace_from_corner = float_first_trace_from_corner.round();
        let rounded_second_trace_from_corner = float_second_trace_from_corner.round();

        if via_location.side_of(first_trace_from_corner, second_trace_from_corner)
            == Side::Collinear
            && scalar_product > 0.0
        {
            if second_trace_from_corner_distance < first_trace_from_corner_distance {
                return Self::reposition_via_toward_location(
                    board,
                    via,
                    &rounded_second_trace_from_corner,
                    first_trace_half_width,
                    first_trace_layer,
                    first_trace_cl_class,
                );
            }
            return Self::reposition_via_toward_location(
                board,
                via,
                &rounded_first_trace_from_corner,
                second_trace_half_width,
                second_trace_layer,
                second_trace_cl_class,
            );
        }
        let mut result: Option<Point>;

        let mut current_weighted_distance_1 = float_via_location.weighted_distance(
            &float_first_trace_from_corner,
            first_trace_costs.horizontal,
            first_trace_costs.vertical,
        );
        let mut current_weighted_distance_2 = float_via_location.weighted_distance(
            &float_first_trace_from_corner,
            second_trace_costs.horizontal,
            second_trace_costs.vertical,
        );

        if current_weighted_distance_1 > current_weighted_distance_2 {
            result = Self::reposition_via_toward_location(
                board,
                via,
                &rounded_first_trace_from_corner,
                second_trace_half_width,
                second_trace_layer,
                second_trace_cl_class,
            );
            if result.is_some() {
                return result;
            }
        }

        current_weighted_distance_1 = float_via_location.weighted_distance(
            &float_second_trace_from_corner,
            second_trace_costs.horizontal,
            second_trace_costs.vertical,
        );
        current_weighted_distance_2 = float_via_location.weighted_distance(
            &float_second_trace_from_corner,
            first_trace_costs.horizontal,
            first_trace_costs.vertical,
        );

        if current_weighted_distance_1 > current_weighted_distance_2 {
            result = Self::reposition_via_toward_location(
                board,
                via,
                &rounded_second_trace_from_corner,
                first_trace_half_width,
                first_trace_layer,
                first_trace_cl_class,
            );
            if result.is_some() {
                return result;
            }
        }

        if scalar_product > 0.0
            && board.rules.trace_angle_restriction != AngleRestriction::NinetyDegree
        {
            let to_point_1: IntPoint;
            let to_point_2: IntPoint;
            let float_to_point_1: FloatPoint;
            let float_to_point_2: FloatPoint;
            if first_trace_from_corner_distance < second_trace_from_corner_distance {
                to_point_1 = rounded_first_trace_from_corner;
                float_to_point_1 = float_first_trace_from_corner;
                float_to_point_2 = float_via_location.change_length(
                    &float_second_trace_from_corner,
                    first_trace_from_corner_distance,
                );
                to_point_2 = float_to_point_2.round();
            } else {
                float_to_point_1 = float_via_location.change_length(
                    &float_first_trace_from_corner,
                    second_trace_from_corner_distance,
                );
                to_point_1 = float_to_point_1.round();
                to_point_2 = rounded_second_trace_from_corner;
                float_to_point_2 = float_second_trace_from_corner;
            }
            current_weighted_distance_1 = float_to_point_1.weighted_distance(
                &float_to_point_2,
                first_trace_costs.horizontal,
                first_trace_costs.vertical,
            );
            current_weighted_distance_2 = float_to_point_1.weighted_distance(
                &float_to_point_2,
                second_trace_costs.horizontal,
                second_trace_costs.vertical,
            );

            if current_weighted_distance_1 > current_weighted_distance_2 {
                result = Self::reposition_via_toward_location(
                    board,
                    via,
                    &to_point_1,
                    second_trace_half_width,
                    second_trace_layer,
                    second_trace_cl_class,
                );
                if result.is_none() {
                    result = Self::reposition_via_toward_location(
                        board,
                        via,
                        &to_point_2,
                        first_trace_half_width,
                        first_trace_layer,
                        first_trace_cl_class,
                    );
                }
            } else {
                result = Self::reposition_via_toward_location(
                    board,
                    via,
                    &to_point_2,
                    first_trace_half_width,
                    first_trace_layer,
                    first_trace_cl_class,
                );
                if result.is_none() {
                    result = Self::reposition_via_toward_location(
                        board,
                        via,
                        &to_point_1,
                        second_trace_half_width,
                        second_trace_layer,
                        second_trace_cl_class,
                    );
                }
            }
            if result.is_some() {
                return result;
            }
        }

        if !first_delta.is_orthogonal() {
            let mut float_check_location =
                FloatPoint::new(float_via_location.x, float_first_trace_from_corner.y);

            current_weighted_distance_1 = float_via_location.weighted_distance(
                &float_first_trace_from_corner,
                first_trace_costs.horizontal,
                first_trace_costs.vertical,
            );
            current_weighted_distance_2 = float_via_location.weighted_distance(
                &float_check_location,
                second_trace_costs.horizontal,
                second_trace_costs.vertical,
            );
            let mut current_weighted_distance_3 = float_check_location.weighted_distance(
                &float_first_trace_from_corner,
                first_trace_costs.horizontal,
                first_trace_costs.vertical,
            );

            if current_weighted_distance_1
                > current_weighted_distance_2 + current_weighted_distance_3
            {
                let check_location = float_check_location.round();
                let check_ok = Self::reposition_via_check_candidate(
                    board,
                    via,
                    &check_location,
                    second_trace_half_width,
                    second_trace_layer,
                    second_trace_cl_class,
                    &rounded_first_trace_from_corner,
                    first_trace_half_width,
                    first_trace_layer,
                    first_trace_cl_class,
                );
                if check_ok {
                    return Some(Point::Int(check_location));
                }
            }

            float_check_location =
                FloatPoint::new(float_first_trace_from_corner.x, float_via_location.y);

            current_weighted_distance_2 = float_via_location.weighted_distance(
                &float_check_location,
                second_trace_costs.horizontal,
                second_trace_costs.vertical,
            );
            current_weighted_distance_3 = float_check_location.weighted_distance(
                &float_first_trace_from_corner,
                first_trace_costs.horizontal,
                first_trace_costs.vertical,
            );

            if current_weighted_distance_1
                > current_weighted_distance_2 + current_weighted_distance_3
            {
                let check_location = float_check_location.round();
                let check_ok = Self::reposition_via_check_candidate(
                    board,
                    via,
                    &check_location,
                    second_trace_half_width,
                    second_trace_layer,
                    second_trace_cl_class,
                    &rounded_first_trace_from_corner,
                    first_trace_half_width,
                    first_trace_layer,
                    first_trace_cl_class,
                );
                if check_ok {
                    return Some(Point::Int(check_location));
                }
            }
        }

        if !second_delta.is_orthogonal() {
            let mut float_check_location =
                FloatPoint::new(float_via_location.x, float_second_trace_from_corner.y);

            current_weighted_distance_1 = float_via_location.weighted_distance(
                &float_second_trace_from_corner,
                second_trace_costs.horizontal,
                second_trace_costs.vertical,
            );
            current_weighted_distance_2 = float_via_location.weighted_distance(
                &float_check_location,
                first_trace_costs.horizontal,
                first_trace_costs.vertical,
            );
            let mut current_weighted_distance_3 = float_check_location.weighted_distance(
                &float_second_trace_from_corner,
                second_trace_costs.horizontal,
                second_trace_costs.vertical,
            );

            if current_weighted_distance_1
                > current_weighted_distance_2 + current_weighted_distance_3
            {
                let check_location = float_check_location.round();
                let check_ok = Self::reposition_via_check_candidate(
                    board,
                    via,
                    &check_location,
                    first_trace_half_width,
                    first_trace_layer,
                    first_trace_cl_class,
                    &rounded_second_trace_from_corner,
                    second_trace_half_width,
                    second_trace_layer,
                    second_trace_cl_class,
                );
                if check_ok {
                    return Some(Point::Int(check_location));
                }
            }

            float_check_location =
                FloatPoint::new(float_second_trace_from_corner.x, float_via_location.y);

            current_weighted_distance_2 = float_via_location.weighted_distance(
                &float_check_location,
                first_trace_costs.horizontal,
                first_trace_costs.vertical,
            );
            current_weighted_distance_3 = float_check_location.weighted_distance(
                &float_second_trace_from_corner,
                second_trace_costs.horizontal,
                second_trace_costs.vertical,
            );

            if current_weighted_distance_1
                > current_weighted_distance_2 + current_weighted_distance_3
            {
                let check_location = float_check_location.round();
                let check_ok = Self::reposition_via_check_candidate(
                    board,
                    via,
                    &check_location,
                    first_trace_half_width,
                    first_trace_layer,
                    first_trace_cl_class,
                    &rounded_second_trace_from_corner,
                    second_trace_half_width,
                    second_trace_layer,
                    second_trace_cl_class,
                );
                if check_ok {
                    return Some(Point::Int(check_location));
                }
            }
        }
        None
    }

    fn contact_role(board: &Board, contact: ItemId) -> ContactRole {
        let Some(item) = board.get_item(contact) else {
            return ContactRole::Unusable;
        };
        if item.is_shove_fixed(&board.rules) || !item.is_trace() {
            if matches!(item, Item::ConductionArea(_)) {
                ContactRole::Plane
            } else {
                ContactRole::Unusable
            }
        } else {
            ContactRole::FreeTrace
        }
    }

    fn via_tolerance(board: &Board, via: ItemId) -> i32 {
        let ctx = board.ctx();
        let min_width = match board.get_item(via) {
            Some(Item::Via(v)) => v.min_width(&ctx),
            Some(Item::Pin(p)) => p.min_width(&ctx),
            _ => return 1,
        };
        (min_width / 2.0) as i32 + 1
    }

    fn from_corner(
        board: &Board,
        trace: ItemId,
        via_center: &Point,
        tolerance: i32,
    ) -> Option<Point> {
        let first = Self::trace_corner(board, trace, TraceEnd::First);
        let last = Self::trace_corner(board, trace, TraceEnd::Last);
        let Some(Item::Trace(polyline_trace)) = board.get_item(trace) else {
            return None;
        };
        let polyline = polyline_trace.polyline();
        if Self::is_within_tolerance(first.as_ref(), via_center, tolerance) {
            polyline.corner(1)
        } else if Self::is_within_tolerance(last.as_ref(), via_center, tolerance) {
            polyline.corner(polyline.corner_count().wrapping_sub(2))
        } else {
            None
        }
    }

    fn trace_corner(board: &Board, trace: ItemId, end: TraceEnd) -> Option<Point> {
        match board.get_item(trace) {
            Some(Item::Trace(t)) => match end {
                TraceEnd::First => t.first_corner(),
                TraceEnd::Last => t.last_corner(),
            },
            _ => None,
        }
    }

    fn trace_layer(board: &Board, trace: ItemId) -> usize {
        match board.get_item(trace) {
            Some(Item::Trace(t)) => t.get_layer(),
            _ => 0,
        }
    }

    fn trace_half_width(board: &Board, trace: ItemId) -> i32 {
        match board.get_item(trace) {
            Some(Item::Trace(t)) => t.get_half_width(),
            _ => 0,
        }
    }

    fn trace_clearance_class(board: &Board, trace: ItemId) -> usize {
        match board.get_item(trace) {
            Some(item) => item.clearance_class(),
            None => 0,
        }
    }

    fn net_nos(board: &Board, item: ItemId) -> Vec<i32> {
        match board.get_item(item) {
            Some(item) => item.net_nos().to_vec(),
            None => Vec::new(),
        }
    }
}

enum ContactRole {
    FreeTrace,
    Plane,
    Unusable,
}

enum TraceEnd {
    First,
    Last,
}
