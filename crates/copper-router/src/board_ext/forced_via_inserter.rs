use copper_board::datastructures::StopCheck;
use copper_board::prelude::*;
use copper_board::rules::ViaInfo;
use copper_board::{BoardError, PadstackId};
use copper_geometry::{Circle, FloatPoint, Point, Shape, ShapeOps, Simplex, TileShape, limits};

use crate::board_ext::forced_pad_router::{CheckDrillResult, ForcedPadRouter};

pub struct ForcedViaInserter;

impl ForcedViaInserter {
    #[allow(clippy::too_many_arguments)]
    pub fn check_layer(
        board: &mut Board,
        via_radius: f64,
        clearance_class_index: usize,
        attach_smd_allowed: bool,
        room_shape: &TileShape,
        location: &Point,
        layer: usize,
        net_numbers: &[i32],
        max_recursion_depth: i32,
        max_via_recursion_depth: i32,
        trace_half_width: i32,
        trace_clearance_class: usize,
    ) -> CheckDrillResult {
        if via_radius <= 0.0 {
            return CheckDrillResult::Drillable;
        }
        let Point::Int(int_location) = location else {
            return CheckDrillResult::NotDrillable;
        };
        let via_shape = Circle::new(*int_location, ceil_to_i32(via_radius));

        let check_radius = via_radius
            + 0.5
                * f64::from(board.clearance_value(
                    clearance_class_index,
                    clearance_class_index,
                    layer,
                ))
            + f64::from(board.get_min_trace_half_width());

        let is_90_degree = board.rules.trace_angle_restriction == AngleRestriction::NinetyDegree;
        let tile_shape = if is_90_degree {
            TileShape::Box(via_shape.bounding_box())
        } else {
            TileShape::Octagon(via_shape.bounding_octagon())
        };

        let Some(from_side) = Self::calculate_from_side(
            &int_location.to_float(),
            &tile_shape,
            &room_shape.to_simplex(),
            check_radius,
            is_90_degree,
        ) else {
            return CheckDrillResult::NotDrillable;
        };

        let via_result = ForcedPadRouter::check_forced_pad(
            board,
            &tile_shape,
            &from_side,
            layer,
            net_numbers,
            clearance_class_index,
            attach_smd_allowed,
            None,
            max_recursion_depth,
            max_via_recursion_depth,
            false,
            None,
        );
        if via_result == CheckDrillResult::NotDrillable {
            return via_result;
        }

        if trace_half_width <= 0 {
            return via_result;
        }

        let start_trace_circle = Circle::new(*int_location, trace_half_width);
        let start_trace_shape = if is_90_degree {
            TileShape::Box(start_trace_circle.bounding_box())
        } else {
            TileShape::Octagon(start_trace_circle.bounding_octagon())
        };

        let trace_result = ForcedPadRouter::check_forced_pad(
            board,
            &start_trace_shape,
            &from_side,
            layer,
            net_numbers,
            trace_clearance_class,
            true,
            None,
            max_recursion_depth,
            max_via_recursion_depth,
            false,
            None,
        );
        if trace_result == CheckDrillResult::NotDrillable {
            return trace_result;
        }
        if via_result == CheckDrillResult::DrillableWithAttachSmd
            || trace_result == CheckDrillResult::DrillableWithAttachSmd
        {
            return CheckDrillResult::DrillableWithAttachSmd;
        }
        CheckDrillResult::Drillable
    }

    #[allow(clippy::too_many_arguments)]
    pub fn check(
        board: &mut Board,
        via_info: &ViaInfo,
        location: &Point,
        net_numbers: &[i32],
        max_recursion_depth: i32,
        max_via_recursion_depth: i32,
        trace_pen_halfwidth_arr: Option<&[i32]>,
        trace_clearance_class_index: usize,
    ) -> bool {
        let translate_vector = location.difference_by(&Point::ZERO);
        let calc_from_side_offset = board.get_min_trace_half_width();
        let via_padstack = via_info.get_padstack();
        let hole_shape = Self::hole_check_shape(board, via_padstack, location);
        let attach_smd_allowed = via_info.attach_smd_allowed();
        let via_clearance_class = via_info.get_clearance_class_index();
        let is_90_degree = board.rules.trace_angle_restriction == AngleRestriction::NinetyDegree;
        let Some((from_layer, to_layer)) = padstack_shape_layer_range(board, via_padstack) else {
            return true;
        };

        for i in from_layer..=to_layer {
            let padstack_shape = board
                .library
                .padstacks
                .get(via_padstack)
                .and_then(|padstack| padstack.get_shape(i))
                .cloned();
            let (current_pad_shape, current_clearance_class_index) = match padstack_shape {
                None => match &hole_shape {
                    None => continue,
                    Some(hole) => (hole.clone(), 0usize),
                },
                Some(shape) => (shape.translate_by(&translate_vector), via_clearance_class),
            };
            let layer = i as usize;
            let Some(tile_shape) = bounding_tile(&current_pad_shape, is_90_degree) else {
                continue;
            };
            let from_side = ForcedPadRouter::calc_from_side(
                board,
                &tile_shape,
                location,
                layer,
                calc_from_side_offset,
                current_clearance_class_index,
            );
            if ForcedPadRouter::check_forced_pad(
                board,
                &tile_shape,
                &from_side,
                layer,
                net_numbers,
                current_clearance_class_index,
                attach_smd_allowed,
                None,
                max_recursion_depth,
                max_via_recursion_depth,
                false,
                None,
            ) == CheckDrillResult::NotDrillable
            {
                board.set_shove_failing_layer(i);
                return false;
            }
            if current_clearance_class_index != 0
                && let Some(hole) = &hole_shape
                && let Some(hole_tile) = bounding_tile(hole, is_90_degree)
                && ForcedPadRouter::check_forced_pad(
                    board,
                    &hole_tile,
                    &from_side,
                    layer,
                    net_numbers,
                    0,
                    attach_smd_allowed,
                    None,
                    max_recursion_depth,
                    max_via_recursion_depth,
                    false,
                    None,
                ) == CheckDrillResult::NotDrillable
            {
                board.set_shove_failing_layer(i);
                return false;
            }

            let pen_half_width = trace_pen_halfwidth_arr
                .and_then(|arr| {
                    usize::try_from(i)
                        .ok()
                        .and_then(|idx| arr.get(idx))
                        .copied()
                })
                .unwrap_or(0);
            if pen_half_width > 0
                && let Point::Int(trace_point) = location
            {
                let start_trace_circle = Circle::new(*trace_point, pen_half_width);
                let start_trace_shape = if is_90_degree {
                    TileShape::Box(start_trace_circle.bounding_box())
                } else {
                    TileShape::Octagon(start_trace_circle.bounding_octagon())
                };
                if ForcedPadRouter::check_forced_pad(
                    board,
                    &start_trace_shape,
                    &from_side,
                    layer,
                    net_numbers,
                    trace_clearance_class_index,
                    true,
                    None,
                    max_recursion_depth,
                    max_via_recursion_depth,
                    false,
                    None,
                ) == CheckDrillResult::NotDrillable
                {
                    board.set_shove_failing_layer(i);
                    return false;
                }
            }
        }
        true
    }

    #[allow(clippy::too_many_arguments)]
    pub fn insert(
        board: &mut Board,
        via_info: &ViaInfo,
        location: &Point,
        net_numbers: &[i32],
        trace_clearance_class_index: usize,
        trace_pen_halfwidth_arr: &[i32],
        max_recursion_depth: i32,
        max_via_recursion_depth: i32,
        stop: StopCheck<'_>,
    ) -> Result<bool, BoardError> {
        let translate_vector = location.difference_by(&Point::ZERO);
        let calc_from_side_offset = board.get_min_trace_half_width();
        let via_padstack = via_info.get_padstack();
        let hole_shape = Self::hole_check_shape(board, via_padstack, location);
        let attach_smd_allowed = via_info.attach_smd_allowed();
        let via_clearance_class = via_info.get_clearance_class_index();
        let is_90_degree = board.rules.trace_angle_restriction == AngleRestriction::NinetyDegree;
        let layer_range: Vec<i32> = padstack_shape_layer_range(board, via_padstack)
            .map_or_else(Vec::new, |(from, to)| (from..=to).collect());

        for i in layer_range {
            let padstack_shape = board
                .library
                .padstacks
                .get(via_padstack)
                .and_then(|padstack| padstack.get_shape(i))
                .cloned();
            let (current_pad_shape, current_clearance_class_index) = match padstack_shape {
                None => match &hole_shape {
                    None => continue,
                    Some(hole) => (hole.clone(), 0usize),
                },
                Some(shape) => (shape.translate_by(&translate_vector), via_clearance_class),
            };
            let layer = i as usize;
            let pen_half_width = usize::try_from(i)
                .ok()
                .and_then(|idx| trace_pen_halfwidth_arr.get(idx))
                .copied()
                .unwrap_or(0);
            let start_trace_circle = match location {
                Point::Int(point) if pen_half_width > 0 => {
                    Some(Circle::new(*point, pen_half_width))
                }
                _ => None,
            };
            let Some(tile_shape) = bounding_tile(&current_pad_shape, is_90_degree) else {
                continue;
            };
            let start_trace_shape = start_trace_circle.map(|circle| {
                if is_90_degree {
                    TileShape::Box(circle.bounding_box())
                } else {
                    TileShape::Octagon(circle.bounding_octagon())
                }
            });
            let from_side = ForcedPadRouter::calc_from_side(
                board,
                &tile_shape,
                location,
                layer,
                calc_from_side_offset,
                current_clearance_class_index,
            );
            if !ForcedPadRouter::forced_pad(
                board,
                &tile_shape,
                &from_side,
                layer,
                net_numbers,
                current_clearance_class_index,
                attach_smd_allowed,
                None,
                max_recursion_depth,
                max_via_recursion_depth,
                stop,
            )? {
                board.set_shove_failing_layer(i);
                return Ok(false);
            }
            if current_clearance_class_index != 0
                && let Some(hole) = &hole_shape
                && let Some(hole_tile) = bounding_tile(hole, is_90_degree)
                && !ForcedPadRouter::forced_pad(
                    board,
                    &hole_tile,
                    &from_side,
                    layer,
                    net_numbers,
                    0,
                    attach_smd_allowed,
                    None,
                    max_recursion_depth,
                    max_via_recursion_depth,
                    stop,
                )?
            {
                board.set_shove_failing_layer(i);
                return Ok(false);
            }
            if let Some(start_trace_shape) = start_trace_shape
                && !ForcedPadRouter::forced_pad(
                    board,
                    &start_trace_shape,
                    &from_side,
                    layer,
                    net_numbers,
                    trace_clearance_class_index,
                    true,
                    None,
                    max_recursion_depth,
                    max_via_recursion_depth,
                    stop,
                )?
            {
                board.set_shove_failing_layer(i);
                return Ok(false);
            }
        }
        board.insert_via_checked(
            via_padstack,
            location.clone(),
            net_numbers.to_vec(),
            via_clearance_class,
            FixedState::Unfixed,
            attach_smd_allowed,
            stop,
        )?;
        Ok(true)
    }

    pub fn hole_check_shape(
        board: &Board,
        padstack: PadstackId,
        location: &Point,
    ) -> Option<Shape> {
        let hole_clearance = board.rules.get_hole_clearance();
        let Point::Int(center) = location else {
            return None;
        };
        if hole_clearance <= 0 {
            return None;
        }
        let drill_radius = board.library.padstacks.get(padstack)?.drill_radius();
        if drill_radius <= 0.0 {
            return None;
        }
        Some(Shape::Circle(Circle::new(
            *center,
            ceil_to_i32(drill_radius + f64::from(hole_clearance) + 10.0),
        )))
    }

    pub fn calculate_from_side(
        via_location: &FloatPoint,
        via_shape: &TileShape,
        room_shape: &Simplex,
        dist: f64,
        is_90_degree: bool,
    ) -> Option<ShapeEntrySide> {
        let via_box = via_shape.bounding_box();
        for i in 0..4 {
            let (check_point, border_x, border_y) = match i {
                0 => (
                    FloatPoint::new(via_location.x, via_location.y - dist),
                    via_location.x,
                    f64::from(via_box.ll.y),
                ),
                1 => (
                    FloatPoint::new(via_location.x + dist, via_location.y),
                    f64::from(via_box.ur.x),
                    via_location.y,
                ),
                2 => (
                    FloatPoint::new(via_location.x, via_location.y + dist),
                    via_location.x,
                    f64::from(via_box.ur.y),
                ),
                _ => (
                    FloatPoint::new(via_location.x - dist, via_location.y),
                    f64::from(via_box.ll.x),
                    via_location.y,
                ),
            };
            if TileShape::Simplex(room_shape.clone()).contains_float(&check_point) {
                let from_side_index = if is_90_degree { i } else { 2 * i };
                return Some(ShapeEntrySide::new(
                    from_side_index,
                    Some(FloatPoint::new(border_x, border_y)),
                ));
            }
        }
        if is_90_degree {
            return None;
        }
        let dist = dist / limits::SQRT2;
        let border_dist = via_box.max_width() / (2.0 * limits::SQRT2);
        for i in 0..4 {
            let (check_point, border_x, border_y) = match i {
                0 => (
                    FloatPoint::new(via_location.x + dist, via_location.y - dist),
                    via_location.x + border_dist,
                    via_location.y - border_dist,
                ),
                1 => (
                    FloatPoint::new(via_location.x + dist, via_location.y + dist),
                    via_location.x + border_dist,
                    via_location.y + border_dist,
                ),
                2 => (
                    FloatPoint::new(via_location.x - dist, via_location.y + dist),
                    via_location.x - border_dist,
                    via_location.y + border_dist,
                ),
                _ => (
                    FloatPoint::new(via_location.x - dist, via_location.y - dist),
                    via_location.x - border_dist,
                    via_location.y - border_dist,
                ),
            };
            if TileShape::Simplex(room_shape.clone()).contains_float(&check_point) {
                return Some(ShapeEntrySide::new(
                    2 * i + 1,
                    Some(FloatPoint::new(border_x, border_y)),
                ));
            }
        }
        None
    }
}

fn ceil_to_i32(x: f64) -> i32 {
    x.ceil() as i32
}

fn bounding_tile(shape: &Shape, is_90_degree: bool) -> Option<TileShape> {
    if is_90_degree {
        Some(TileShape::Box(shape.bounding_box()))
    } else {
        shape.bounding_octagon().map(TileShape::Octagon)
    }
}

fn padstack_shape_layer_range(board: &Board, padstack: PadstackId) -> Option<(i32, i32)> {
    let padstack = board.library.padstacks.get(padstack)?;
    let (from, to) = (padstack.from_layer(), padstack.to_layer());
    if from > to { None } else { Some((from, to)) }
}
