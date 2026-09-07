use copper_board::board::ShapeTraceEntries;
use copper_board::datastructures::StopCheck;
use copper_board::free_trace_tree_shapes;
use copper_board::items::Item;
use copper_board::prelude::*;
use copper_board::{BoardError, ItemId, StopConnectionOption, TimeLimit};
use copper_geometry::{Direction, Line, Point, Polyline, PolylineShapeOps, TileShape};

use crate::board_ext::drill_item_mover::{DrillItemMover, drill_item_center};
use crate::board_ext::swallow_normalize_error;
use crate::board_ext::trace_shover::TraceShover;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CheckDrillResult {
    /// `DRILLABLE` — the pad fits once the obstacle traces are shoved aside.
    Drillable,
    DrillableWithAttachSmd,
    /// `NOT_DRILLABLE` — the check failed.
    NotDrillable,
}

pub struct ForcedPadRouter;

impl ForcedPadRouter {
    #[allow(clippy::too_many_arguments)]
    pub fn check_forced_pad(
        board: &mut Board,
        pad_shape: &TileShape,
        from_side: &ShapeEntrySide,
        layer: usize,
        net_numbers: &[i32],
        clearance_class_index: usize,
        copper_sharing_allowed: bool,
        ignore_items: Option<&[ItemId]>,
        max_recursion_depth: i32,
        max_via_recursion_depth: i32,
        check_only_front: bool,
        time_limit: Option<&TimeLimit>,
    ) -> CheckDrillResult {
        // :233-236.
        if !PolylineShapeOps::is_contained_in(pad_shape, &board.get_bounding_box()) {
            let outline = board.get_outline();
            board.set_shove_failing_obstacle(outline);
            return CheckDrillResult::NotDrillable;
        }
        // :237-245. The **default** tree, not the engine's compensated one.
        let mut shape_entries = ShapeTraceEntries::new(
            pad_shape.clone(),
            layer,
            net_numbers.to_vec(),
            clearance_class_index,
            Some(*from_side),
        );
        let mut obstacles = board.overlapping_items_with_clearance(
            pad_shape,
            Some(layer),
            &[],
            clearance_class_index,
        );
        if let Some(ignored) = ignore_items {
            obstacles.retain(|id| !ignored.contains(id));
        }
        // :246-250.
        let obstacles_shovable =
            shape_entries.store_items(board, &obstacles, true, copper_sharing_allowed);
        if !obstacles_shovable {
            let found = shape_entries.get_found_obstacle();
            board.set_shove_failing_obstacle(found);
            return CheckDrillResult::NotDrillable;
        }

        // :252-279. "check, if the obstacle vias can be shoved"
        for current_shove_via in shape_entries.shove_via_list.clone() {
            // :255-258.
            if max_via_recursion_depth <= 0 {
                board.set_shove_failing_obstacle(Some(current_shove_via));
                return CheckDrillResult::NotDrillable;
            }
            // :259-266.
            let new_via_centers = DrillItemMover::try_shove_via_points(
                board,
                pad_shape,
                layer,
                current_shove_via,
                clearance_class_index,
                false,
            );
            let Some(new_via_center) = new_via_centers.first().copied() else {
                board.set_shove_failing_obstacle(Some(current_shove_via));
                return CheckDrillResult::NotDrillable;
            };
            // :267.
            let Some(via_center) = drill_item_center(board, current_shove_via) else {
                return CheckDrillResult::NotDrillable;
            };
            let delta = Point::Int(new_via_center).difference_by(&via_center);
            let check_ignore_items = Vec::new();
            if !DrillItemMover::check(
                board,
                current_shove_via,
                &delta,
                max_recursion_depth,
                max_via_recursion_depth - 1,
                Some(&check_ignore_items),
                time_limit,
            ) {
                return CheckDrillResult::NotDrillable;
            }
        }

        // :280-288. The obstacle list is walked in `overlappingItemsWithClearance` order and the
        // **first** pin promotes the answer; since the promotion is the same whichever pin wins,
        // the order is unobservable here.
        let mut result = CheckDrillResult::Drillable;
        if copper_sharing_allowed
            && obstacles
                .iter()
                .any(|id| matches!(board.get_item(*id), Some(Item::Pin(_))))
        {
            result = CheckDrillResult::DrillableWithAttachSmd;
        }

        // :289-300.
        let trace_piece_count = shape_entries.substitute_trace_count();
        if trace_piece_count == 0 {
            return result;
        }
        if max_recursion_depth <= 0 {
            let found = shape_entries.get_found_obstacle();
            board.set_shove_failing_obstacle(found);
            return CheckDrillResult::NotDrillable;
        }
        if shape_entries.stack_depth() > 1 {
            let found = shape_entries.get_found_obstacle();
            board.set_shove_failing_obstacle(found);
            return CheckDrillResult::NotDrillable;
        }

        // :301-337.
        let is_orthogonal_mode = matches!(pad_shape, TileShape::Box(_));
        loop {
            let Some(current_substitute_trace) = shape_entries.next_substitute_trace_piece(board)
            else {
                break;
            };
            let substitute_net_nos = current_substitute_trace.hdr.net_nos.clone();
            let substitute_clearance_class = current_substitute_trace.hdr.clearance_class();
            let substitute_half_width = current_substitute_trace.get_half_width();
            let substitute_tree_shapes = free_trace_tree_shapes(board, &current_substitute_trace);
            for i in 0..current_substitute_trace.tile_shape_count() {
                let Some(current_line) = current_substitute_trace
                    .polyline()
                    .lines()
                    .get(i + 1)
                    .copied()
                else {
                    continue;
                };
                let current_direction = Direction::Int(current_line.direction());
                // :311-318.
                let is_in_front = if check_only_front {
                    Self::in_front_of_pad(
                        &current_line,
                        pad_shape,
                        from_side.no,
                        substitute_half_width,
                        true,
                    )
                } else {
                    true
                };
                if !is_in_front {
                    continue;
                }
                let Some(Some(current_tree_shape)) = substitute_tree_shapes.get(i).cloned() else {
                    continue;
                };
                let current = ShapeAndEntrySide::from_free_trace(
                    board,
                    &current_substitute_trace,
                    current_tree_shape,
                    i,
                    is_orthogonal_mode,
                    true,
                );
                if !TraceShover::check(
                    board,
                    &current.shape,
                    current.from_side.as_ref(),
                    Some(current_direction),
                    layer,
                    &substitute_net_nos,
                    substitute_clearance_class,
                    max_recursion_depth - 1,
                    max_via_recursion_depth,
                    0,
                    time_limit,
                ) {
                    return CheckDrillResult::NotDrillable;
                }
            }
        }
        // :338.
        result
    }

    #[allow(clippy::too_many_arguments)]
    pub fn forced_pad(
        board: &mut Board,
        pad_shape: &TileShape,
        from_side: &ShapeEntrySide,
        layer: usize,
        net_numbers: &[i32],
        clearance_class_index: usize,
        copper_sharing_allowed: bool,
        ignore_items: Option<&[ItemId]>,
        max_recursion_depth: i32,
        max_via_recursion_depth: i32,
        stop: StopCheck<'_>,
    ) -> Result<bool, BoardError> {
        // :356-359. FRLogger.warn("ShoveTraceAux.forced_pad: padShape is empty")
        if pad_shape.is_empty() {
            return Ok(true);
        }
        // :360-363.
        if !PolylineShapeOps::is_contained_in(pad_shape, &board.get_bounding_box()) {
            let outline = board.get_outline();
            board.set_shove_failing_obstacle(outline);
            return Ok(false);
        }
        // :364-375. Note `copperSharingAllowed = false` here, where `TraceShover.insert:444`
        // passes `true` to the same method.
        if !DrillItemMover::shove_vias(
            board,
            pad_shape,
            from_side,
            layer,
            net_numbers,
            clearance_class_index,
            ignore_items,
            max_recursion_depth,
            max_via_recursion_depth,
            false,
            stop,
        )? {
            return Ok(false);
        }

        // :377-384. The **default** tree, not the engine's compensated one.
        let mut shape_entries = ShapeTraceEntries::new(
            pad_shape.clone(),
            layer,
            net_numbers.to_vec(),
            clearance_class_index,
            Some(*from_side),
        );
        let mut obstacles = board.overlapping_items_with_clearance(
            pad_shape,
            Some(layer),
            &[],
            clearance_class_index,
        );
        if let Some(ignored) = ignore_items {
            obstacles.retain(|id| !ignored.contains(id));
        }
        let obstacles_shovable =
            shape_entries.store_items(board, &obstacles, true, copper_sharing_allowed)
                && shape_entries.shove_via_list.is_empty();
        if !obstacles_shovable {
            let found = shape_entries.get_found_obstacle();
            board.set_shove_failing_obstacle(found);
            return Ok(false);
        }
        // :392-399.
        let trace_piece_count = shape_entries.substitute_trace_count();
        if trace_piece_count == 0 {
            return Ok(true);
        }
        if max_recursion_depth <= 0 {
            let found = shape_entries.get_found_obstacle();
            board.set_shove_failing_obstacle(found);
            return Ok(false);
        }
        // :400-402.
        let tails_exist_before = board.contains_trace_tails(obstacles.iter().copied(), net_numbers);
        shape_entries.cutout_traces(board, &obstacles);
        let is_orthogonal_mode = matches!(pad_shape, TileShape::Box(_));

        // :404-463.
        loop {
            let Some(current_substitute_trace) = shape_entries.next_substitute_trace_piece(board)
            else {
                break;
            };
            if current_substitute_trace.first_corner() == current_substitute_trace.last_corner() {
                continue;
            }
            // :412 and `:421`.
            let substitute_net_nos = current_substitute_trace.hdr.net_nos.clone();
            let substitute_clearance_class = current_substitute_trace.hdr.clearance_class();
            let substitute_tree_shapes = free_trace_tree_shapes(board, &current_substitute_trace);
            for i in 0..current_substitute_trace.tile_shape_count() {
                let Some(Some(current_tree_shape)) = substitute_tree_shapes.get(i).cloned() else {
                    continue;
                };
                // `inShoveCheck = false` (`:415`), where both `check` paths pass `true`: with it
                // false, `ShapeAndEntrySide:71-75` fills a `fromSide` in from the piece's own
                // polyline whenever the cut lines produced none.
                let current = ShapeAndEntrySide::from_free_trace(
                    board,
                    &current_substitute_trace,
                    current_tree_shape,
                    i,
                    is_orthogonal_mode,
                    false,
                );
                if !TraceShover::insert(
                    board,
                    &current.shape,
                    current.from_side.as_ref(),
                    layer,
                    &substitute_net_nos,
                    substitute_clearance_class,
                    ignore_items,
                    max_recursion_depth - 1,
                    max_via_recursion_depth,
                    0,
                    stop,
                )? {
                    return Ok(false);
                }
            }
            // :429-431.
            for i in 0..current_substitute_trace.corner_count() {
                if let Some(corner) = current_substitute_trace.polyline().corner_approx(i) {
                    board.join_changed_area(&corner, layer);
                }
            }
            // :432-437.
            let end_corners = if tails_exist_before {
                None
            } else {
                Some([
                    current_substitute_trace.first_corner(),
                    current_substitute_trace.last_corner(),
                ])
            };
            // :438. The piece keeps the id `nextSubstituteTracePiece` burnt for it.
            let inserted = board.insert_item(Item::Trace(current_substitute_trace));
            // :439-444.
            let opt_area = board
                .changed_area
                .as_ref()
                .map(|changed_area| changed_area.get_area(layer));
            swallow_normalize_error(board.normalize_trace_checked(
                inserted,
                opt_area.as_ref(),
                stop,
            ))?;
            // :452-462.
            if let Some(end_corners) = end_corners {
                for corner in end_corners.into_iter().flatten() {
                    let Some(tail) =
                        board.get_trace_tail(&corner, Some(layer), &substitute_net_nos)
                    else {
                        continue;
                    };
                    let connection =
                        board.connection_items_checked(tail, StopConnectionOption::Via, stop)?;
                    board.remove_items(connection);
                    for net_number in &substitute_net_nos {
                        board.combine_traces(*net_number)?;
                    }
                }
            }
        }
        // :464.
        Ok(true)
    }

    pub fn calc_from_side(
        board: &mut Board,
        shape: &TileShape,
        shape_center: &Point,
        layer: usize,
        offset: i32,
        clearance_class_index: usize,
    ) -> ShapeEntrySide {
        // :473-474.
        let offset_shape = shape.offset(f64::from(offset));
        for clearance_class in [clearance_class_index, 0] {
            for i in 0..offset_shape.border_line_count() {
                let Some(border_line) = offset_shape.border_line(i) else {
                    continue;
                };
                let Some(check_shape) = calc_check_shape_for_from_side(shape_center, &border_line)
                else {
                    continue;
                };
                // :479-481 / :487-489.
                if board.check_trace_shape(&check_shape, layer, &[], clearance_class, None) {
                    return ShapeEntrySide::new(
                        i32::try_from(i).expect("a border line index fits in an i32"),
                        None,
                    );
                }
            }
        }
        // :491.
        ShapeEntrySide::NOT_CALCULATED
    }

    pub fn in_front_of_pad(
        line: &Line,
        pad_shape: &TileShape,
        from_side: i32,
        width: i32,
        with_sides: bool,
    ) -> bool {
        // :59-62. "only implemented for octagons"
        if !pad_shape.is_int_octagon() {
            return true;
        }
        let Some(pad) = pad_shape.bounding_octagon() else {
            return true;
        };
        let (a, b) = (line.a, line.b);

        let diag_width = f64::from(width) * f64::sqrt(2.0);
        let min_y = a.y.min(b.y);
        let max_y = a.y.max(b.y);
        let min_x = a.x.min(b.x);
        let max_x = a.x.max(b.x);
        let a_diff = a.x.wrapping_sub(a.y);
        let b_diff = b.x.wrapping_sub(b.y);
        let min_diff = a_diff.min(b_diff);
        let max_diff = a_diff.max(b_diff);
        let a_sum = a.x.wrapping_add(a.y);
        let b_sum = b.x.wrapping_add(b.y);
        let min_sum = a_sum.min(b_sum);
        let max_sum = a_sum.max(b_sum);
        let min_sum_case0 = a_sum.min(b.x.wrapping_add(b.x));

        let top = f64::from(pad.top_y.wrapping_add(width));
        let bottom = f64::from(pad.bottom_y.wrapping_sub(width));
        let left = f64::from(pad.left_x.wrapping_sub(width));
        let right = f64::from(pad.right_x.wrapping_add(width));
        let upper_left = f64::from(pad.upper_left_diagonal_x) - diag_width;
        let upper_right = f64::from(pad.upper_right_diagonal_x) + diag_width;
        let lower_left = f64::from(pad.lower_left_diagonal_x) - diag_width;
        let lower_right = f64::from(pad.lower_right_diagonal_x) + diag_width;

        match from_side {
            // :73-89.
            0 => {
                let mut result = f64::from(min_y) >= top
                    || f64::from(max_diff) <= upper_left
                    || f64::from(min_sum_case0) >= upper_right;
                if with_sides && !result {
                    result = f64::from(max_x) <= left && f64::from(min_diff) <= upper_left
                        || f64::from(min_x) >= right && f64::from(min_sum) >= upper_right;
                }
                result
            }
            // :90-105.
            1 => {
                let mut result = f64::from(min_y) >= top
                    || f64::from(max_diff) <= upper_left
                    || f64::from(max_x) <= left;
                if with_sides && !result {
                    result = f64::from(min_x) <= left && f64::from(max_sum) <= lower_left
                        || f64::from(max_y) >= top && f64::from(min_sum) >= upper_right;
                }
                result
            }
            // :106-122.
            2 => {
                let mut result = f64::from(max_x) <= left
                    || f64::from(max_diff) <= upper_left
                    || f64::from(max_sum) <= lower_left;
                if with_sides && !result {
                    result = f64::from(max_y) <= bottom && f64::from(min_sum) <= lower_left
                        || f64::from(min_y) >= top && f64::from(min_diff) <= upper_left;
                }
                result
            }
            // :123-138.
            3 => {
                let mut result = f64::from(max_x) <= left
                    || f64::from(max_y) <= bottom
                    || f64::from(max_sum) <= lower_left;
                if with_sides && !result {
                    result = f64::from(min_y) <= bottom && f64::from(min_diff) >= lower_right
                        || f64::from(min_x) <= left && f64::from(max_diff) <= upper_left;
                }
                result
            }
            // :139-155.
            4 => {
                let mut result = f64::from(max_y) <= bottom
                    || f64::from(max_sum) <= lower_left
                    || f64::from(min_diff) >= lower_right;
                if with_sides && !result {
                    result = f64::from(min_x) >= right && f64::from(max_diff) >= lower_right
                        || f64::from(max_x) <= left && f64::from(min_sum) <= lower_left;
                }
                result
            }
            // :156-171.
            5 => {
                let mut result = f64::from(max_y) <= bottom
                    || f64::from(min_x) >= right
                    || f64::from(min_diff) >= lower_right;
                if with_sides && !result {
                    result = f64::from(max_x) >= right && f64::from(min_sum) >= upper_right
                        || f64::from(min_y) <= bottom && f64::from(max_sum) <= lower_left;
                }
                result
            }
            // :172-188.
            6 => {
                let mut result = f64::from(min_x) >= right
                    || f64::from(min_sum) >= upper_right
                    || f64::from(min_diff) >= lower_right;
                if with_sides && !result {
                    result = f64::from(max_y) <= bottom && f64::from(max_diff) >= lower_right
                        || f64::from(min_y) >= top && f64::from(max_sum) >= upper_right;
                }
                result
            }
            // :189-204.
            7 => {
                let mut result = f64::from(min_y) >= top
                    || f64::from(min_sum) >= upper_right
                    || f64::from(min_x) >= right;
                if with_sides && !result {
                    result = f64::from(max_y) >= top && f64::from(max_diff) <= upper_left
                        || f64::from(max_x) >= right && f64::from(min_diff) >= lower_right;
                }
                result
            }
            // :205-208. FRLogger.warn("ForcedPadAlgo.in_front_of_pad: fromSide out of range")
            _ => true,
        }
    }
}

fn calc_check_shape_for_from_side(shape_center: &Point, border_line: &Line) -> Option<TileShape> {
    let Point::Int(centre) = shape_center else {
        return None;
    };
    // :44-45.
    let offset_projection = centre.to_float().projection_approx(border_line);
    // :46-52. "Make sure, that direction restrictions are retained."
    let current_direction = border_line.direction();
    let lines = vec![
        Line::from_direction(*centre, &current_direction),
        Line::from_direction(*centre, &current_direction.turn_45_degree(2)),
        Line::from_direction(offset_projection.round(), &current_direction),
    ];
    let check_line = Polyline::from_lines(lines).ok()?;
    check_line.offset_shape(1, 0)
}
