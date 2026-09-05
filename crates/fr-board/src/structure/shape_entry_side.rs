use fr_geometry::{FloatLine, FloatPoint, Line, LineSegment, Point, Polyline, Side, TileShape};

use crate::board::Board;
use crate::ids::ItemId;
use crate::items::{Item, PolylineTrace};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShapeEntrySide {
    pub no: i32,
    pub border_intersection: Option<FloatPoint>,
}

impl ShapeEntrySide {
    pub const NOT_CALCULATED: ShapeEntrySide = ShapeEntrySide {
        no: -1,
        border_intersection: None,
    };

    pub fn new(no: i32, border_intersection: Option<FloatPoint>) -> ShapeEntrySide {
        ShapeEntrySide {
            no,
            border_intersection,
        }
    }

    pub fn from_polyline(polyline: &Polyline, no: usize, shape: &TileShape) -> ShapeEntrySide {
        let mut fromside_no: i32 = -1;
        let mut intersection: Option<FloatPoint> = None;
        let mut border_intersection_found = false;
        for current_no in (1..=no).rev() {
            let Some(current_seg) = LineSegment::from_polyline(polyline, current_no) else {
                continue;
            };
            let intersections = current_seg.border_intersections(shape);
            if let Some(first) = intersections.first() {
                fromside_no = *first as i32;
                if let Some(border_line) = shape.border_line(*first) {
                    intersection = Some(current_seg.get_line().intersection_approx(&border_line));
                }
                border_intersection_found = true;
                break;
            }
        }
        if !border_intersection_found {
            let Some(from_point) = polyline.corner_approx(0) else {
                return ShapeEntrySide::new(fromside_no, intersection);
            };
            let Some(check_line) = polyline.lines().get(1).copied() else {
                return ShapeEntrySide::new(fromside_no, intersection);
            };
            let mut min_dist = f64::MAX;
            for i in 0..shape.border_line_count() {
                let Some(current_line) = shape.border_line(i) else {
                    continue;
                };
                let current_intersection = check_line.intersection_approx(&current_line);
                let current_distance = current_intersection.distance(&from_point).abs();
                if current_distance < min_dist {
                    fromside_no = i as i32;
                    intersection = Some(current_intersection);
                    min_dist = current_distance;
                }
            }
        }
        ShapeEntrySide::new(fromside_no, intersection)
    }

    pub fn from_point(from_point: &Point, shape: &TileShape) -> ShapeEntrySide {
        let Some(border_projection) = shape.nearest_border_point(from_point) else {
            return ShapeEntrySide::NOT_CALCULATED;
        };
        let no = shape
            .contains_on_border_line_no(&border_projection)
            .map_or(-1, |no| no as i32);
        ShapeEntrySide::new(no, Some(border_projection.to_float()))
    }

    pub fn from_line_segment(
        line_segment: &LineSegment,
        shape: &TileShape,
        shove_to_the_left: bool,
    ) -> ShapeEntrySide {
        let start_corner = line_segment.start_point_approx();
        let end_corner = line_segment.end_point_approx();
        let border_line_count = shape.border_line_count();
        if border_line_count == 0 {
            return ShapeEntrySide::NOT_CALCULATED;
        }
        let check_line = line_segment.get_line();
        let Some(first_corner) = shape.corner_approx(0) else {
            return ShapeEntrySide::NOT_CALCULATED;
        };
        let mut prev_side = check_line.side_of_float_exact(&first_corner);
        let mut front_side_no: i32 = -1;

        for i in 1..=border_line_count {
            let next_corner = if i == border_line_count {
                first_corner
            } else {
                match shape.corner_approx(i) {
                    Some(corner) => corner,
                    None => continue,
                }
            };
            let next_side = check_line.side_of_float_exact(&next_corner);
            if prev_side != next_side {
                let Some(border_line) = shape.border_line(i - 1) else {
                    continue;
                };
                let current_intersection = border_line.intersection_approx(&check_line);
                if current_intersection.distance_square(&start_corner)
                    < current_intersection.distance_square(&end_corner)
                {
                    front_side_no = (i - 1) as i32;
                    break;
                }
            }
            prev_side = next_side;
        }

        if front_side_no < 0 {
            let mut min_distance = f64::MAX;
            let mut nearest_side = 0usize;
            for i in 0..border_line_count {
                let Some(border_line) = shape.border_line(i) else {
                    continue;
                };
                let border_float =
                    FloatLine::new(border_line.a.to_float(), border_line.b.to_float());
                let projection = border_float.perpendicular_projection(&start_corner);
                let (Some(side_start), Some(side_end)) = (
                    shape.corner_approx(i),
                    shape.corner_approx((i + 1) % border_line_count),
                ) else {
                    continue;
                };
                if projection.is_contained_in_box(&side_start, &side_end, 0.01) {
                    let distance = start_corner.distance(&projection);
                    if distance < min_distance {
                        min_distance = distance;
                        nearest_side = i;
                    }
                }
            }
            let no = rotate_side(nearest_side, border_line_count, shove_to_the_left);
            return ShapeEntrySide::new(no as i32, middle_of_side(shape, no, border_line_count));
        }
        let no = rotate_side(front_side_no as usize, border_line_count, shove_to_the_left);
        ShapeEntrySide::new(no as i32, middle_of_side(shape, no, border_line_count))
    }
}

fn rotate_side(side: usize, border_line_count: usize, shove_to_the_left: bool) -> usize {
    if shove_to_the_left {
        (side + 2) % border_line_count
    } else {
        (side + border_line_count - 2) % border_line_count
    }
}

fn middle_of_side(shape: &TileShape, no: usize, border_line_count: usize) -> Option<FloatPoint> {
    let prev_corner = shape.corner_approx(no)?;
    let next_corner = shape.corner_approx((no + 1) % border_line_count)?;
    Some(prev_corner.middle_point(&next_corner))
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShapeAndEntrySide {
    pub shape: TileShape,
    pub from_side: Option<ShapeEntrySide>,
}

impl ShapeAndEntrySide {
    pub fn new(
        board: &Board,
        trace_id: ItemId,
        index: usize,
        orthogonal: bool,
        in_shove_check: bool,
    ) -> Option<ShapeAndEntrySide> {
        let Some(Item::Trace(trace)) = board.get_item(trace_id) else {
            return None;
        };
        let search_tree = board.trees.get_default_tree();
        let current_shape = board
            .item_tree_shape_ref(trace_id, search_tree.id(), index)?
            .into_owned();
        Some(Self::build(
            board,
            trace,
            current_shape,
            index,
            orthogonal,
            in_shove_check,
        ))
    }

    pub fn from_free_trace(
        board: &Board,
        trace: &PolylineTrace,
        tree_shape: TileShape,
        index: usize,
        orthogonal: bool,
        in_shove_check: bool,
    ) -> ShapeAndEntrySide {
        Self::build(board, trace, tree_shape, index, orthogonal, in_shove_check)
    }

    fn build(
        board: &Board,
        trace: &PolylineTrace,
        current_shape: TileShape,
        index: usize,
        orthogonal: bool,
        in_shove_check: bool,
    ) -> ShapeAndEntrySide {
        let search_tree = board.trees.get_default_tree();
        let mut current_shape = current_shape;
        let mut current_from_side: Option<ShapeEntrySide> = None;
        if orthogonal {
            current_shape = TileShape::Box(current_shape.bounding_box());
        } else {
            let mut cut_off_at_start = false;
            let mut cut_off_at_end = false;
            current_shape = TileShape::Simplex(current_shape.to_simplex());
            let compensated_half_width = search_tree.compensated_half_width(trace, &board.rules);
            let end_cutline = calc_cutline_at_end(index, trace.polyline(), compensated_half_width);
            if let Some(end_cutline) = end_cutline {
                let cut_plane = TileShape::get_instance_from_line(end_cutline);
                let tmp_shape = current_shape.intersection(&cut_plane);
                if !tmp_shape.is_empty() && !tmp_shape.contains_tile(&current_shape) {
                    current_shape = TileShape::Simplex(tmp_shape.to_simplex());
                    cut_off_at_end = true;
                }
            }
            let start_cutline =
                calc_cutline_at_start(index, trace.polyline(), compensated_half_width);
            if let Some(start_cutline) = start_cutline {
                let cut_plane = TileShape::get_instance_from_line(start_cutline);
                let tmp_shape = current_shape.intersection(&cut_plane);
                if !tmp_shape.is_empty() && !tmp_shape.contains_tile(&current_shape) {
                    current_shape = TileShape::Simplex(tmp_shape.to_simplex());
                    cut_off_at_start = true;
                }
            }
            let mut from_side_index: Option<usize> = None;
            let mut current_cut_line: Option<Line> = None;
            if cut_off_at_start {
                current_cut_line = start_cutline;
                from_side_index =
                    current_cut_line.and_then(|line| current_shape.border_line_index(&line));
            }
            if from_side_index.is_none() && cut_off_at_end {
                current_cut_line = end_cutline;
                from_side_index =
                    current_cut_line.and_then(|line| current_shape.border_line_index(&line));
            }
            if let (Some(from_side_index), Some(current_cut_line)) =
                (from_side_index, current_cut_line)
                && let Some(border_line) = current_shape.border_line(from_side_index)
            {
                let border_intersection = current_cut_line.intersection_approx(&border_line);
                current_from_side = Some(ShapeEntrySide::new(
                    from_side_index as i32,
                    Some(border_intersection),
                ));
            }
        }
        if current_from_side.is_none() && !in_shove_check {
            current_from_side = Some(ShapeEntrySide::from_polyline(
                trace.polyline(),
                index,
                &current_shape,
            ));
        }
        ShapeAndEntrySide {
            shape: current_shape,
            from_side: current_from_side,
        }
    }
}

pub fn free_trace_tree_shapes(board: &Board, trace: &PolylineTrace) -> Vec<Option<TileShape>> {
    let ctx = board.ctx();
    board
        .trees
        .get_default_tree()
        .calculate_tree_shapes(&Item::Trace(trace.clone()), &ctx)
}

fn calc_cutline_at_end(
    index: usize,
    trace_lines: &Polyline,
    compensated_half_width: i32,
) -> Option<Line> {
    let line_count = trace_lines.lines().len();
    if line_count < 3 {
        return None;
    }
    let near_the_end = trace_lines
        .corner_approx(line_count - 2)
        .zip(trace_lines.corner_approx(index + 1))
        .is_some_and(|(a, b)| a.distance(&b) < f64::from(compensated_half_width));
    if index + 3 != line_count && !near_the_end {
        return None;
    }
    let current_line = trace_lines.lines()[line_count - 1];
    let is = trace_lines.corner_approx(line_count - 3)?;
    Some(
        if current_line.side_of_float_exact(&is) == Side::OnTheLeft {
            current_line.opposite()
        } else {
            current_line
        },
    )
}

fn calc_cutline_at_start(
    index: usize,
    trace_lines: &Polyline,
    compensated_half_width: i32,
) -> Option<Line> {
    if trace_lines.lines().len() < 2 {
        return None;
    }
    let near_the_start = trace_lines
        .corner_approx(0)
        .zip(trace_lines.corner_approx(index))
        .is_some_and(|(a, b)| a.distance(&b) < f64::from(compensated_half_width));
    if index != 0 && !near_the_start {
        return None;
    }
    let current_line = trace_lines.lines()[0];
    let is = trace_lines.corner_approx(1)?;
    Some(
        if current_line.side_of_float_exact(&is) == Side::OnTheLeft {
            current_line.opposite()
        } else {
            current_line
        },
    )
}
