//! Port of `board/model/structure/ShapeEntrySide.java` and
//! `board/model/structure/ShapeAndEntrySide.java`: which border side of a tile shape a shove
//! enters from, and the trace shape with its dog ears cut off.
//!
//! Both types belong to the shove algorithm (Plan 7), which needs them ready-made; nothing in
//! Plan 2 calls them except the tests below.

use fr_geometry::{FloatLine, FloatPoint, Line, LineSegment, Point, Polyline, Side, TileShape};

use crate::board::Board;
use crate::ids::ItemId;
use crate::items::Item;

/// Port of `ShapeEntrySide` (`board/model/structure/ShapeEntrySide.java`, Java's
/// `CalcFromSide`): the index of the border line of a tile shape where something enters, plus
/// the intersection point on it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShapeEntrySide {
    /// Java `public final int no` (ShapeEntrySide.java:17). Java's `-1` (the `NOT_CALCULATED`
    /// sentinel and the "not found" result) is kept as a signed value, because
    /// `ShapeTraceEntries.searchFromSide` (ShapeTraceEntries.java:445) tests `fromSide.no >= 0`
    /// and `resort` (:465) tests it again against the border-line count.
    pub no: i32,
    /// Java `public FloatPoint borderIntersection` (ShapeEntrySide.java:18); `null` is `None`.
    pub border_intersection: Option<FloatPoint>,
}

impl ShapeEntrySide {
    /// Port of `ShapeEntrySide.NOT_CALCULATED` (ShapeEntrySide.java:16).
    pub const NOT_CALCULATED: ShapeEntrySide = ShapeEntrySide {
        no: -1,
        border_intersection: None,
    };

    /// Port of the `ShapeEntrySide(int, FloatPoint)` constructor (ShapeEntrySide.java:157-160):
    /// values already calculated.
    pub fn new(no: i32, border_intersection: Option<FloatPoint>) -> ShapeEntrySide {
        ShapeEntrySide {
            no,
            border_intersection,
        }
    }

    /// Port of the `ShapeEntrySide(Polyline, int, TileShape)` constructor
    /// (ShapeEntrySide.java:26-62): the edge of `shape` where `polyline` enters, searched
    /// backwards from segment `no`.
    ///
    /// When no segment crosses the border at all, the first corner of the polyline is inside the
    /// shape and Java falls back to the border line nearest that corner along `polyline.lines[1]`
    /// (:41-59).
    // renamed: the four Java constructors -> `new`, `from_polyline`, `from_point` and
    // `from_line_segment` (Rust has no overloading).
    pub fn from_polyline(polyline: &Polyline, no: usize, shape: &TileShape) -> ShapeEntrySide {
        let mut fromside_no: i32 = -1;
        let mut intersection: Option<FloatPoint> = None;
        let mut border_intersection_found = false;
        // ShapeEntrySide.java:31-40.
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
            // ShapeEntrySide.java:41-59.
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

    /// Port of the `ShapeEntrySide(Point, TileShape)` constructor (ShapeEntrySide.java:68-75):
    /// the border side of `shape` nearest `from_point`, used by the shove-drill-item algorithm.
    ///
    /// Java's `FRLogger.warn("CalcFromSide: this.no >= 0 expected")` (:72) is dropped; the `-1`
    /// it warns about is stored either way.
    pub fn from_point(from_point: &Point, shape: &TileShape) -> ShapeEntrySide {
        let Some(border_projection) = shape.nearest_border_point(from_point) else {
            return ShapeEntrySide::NOT_CALCULATED;
        };
        let no = shape
            .contains_on_border_line_no(&border_projection)
            .map_or(-1, |no| no as i32);
        ShapeEntrySide::new(no, Some(border_projection.to_float()))
    }

    /// Port of the `ShapeEntrySide(LineSegment, TileShape, boolean)` constructor
    /// (ShapeEntrySide.java:81-154): the side two edges round from where `line_segment` first
    /// crosses the shape, in the shove direction.
    ///
    /// The fallback branch (:108-145) runs when no border line separates the segment's two end
    /// points; it picks the border side whose perpendicular projection of the start point lands
    /// on it, then applies the same `± 2` rotation.
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

        // ShapeEntrySide.java:90-107.
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
            // ShapeEntrySide.java:108-145.
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
        // ShapeEntrySide.java:146-153.
        let no = rotate_side(front_side_no as usize, border_line_count, shove_to_the_left);
        ShapeEntrySide::new(no as i32, middle_of_side(shape, no, border_line_count))
    }
}

/// `(side + 2) % count` when shoving left, `(side + count - 2) % count` otherwise
/// (ShapeEntrySide.java:134-138,146-150).
fn rotate_side(side: usize, border_line_count: usize, shove_to_the_left: bool) -> usize {
    if shove_to_the_left {
        (side + 2) % border_line_count
    } else {
        (side + border_line_count - 2) % border_line_count
    }
}

/// The middle of border side `no` (ShapeEntrySide.java:141-143,151-153).
fn middle_of_side(shape: &TileShape, no: usize, border_line_count: usize) -> Option<FloatPoint> {
    let prev_corner = shape.corner_approx(no)?;
    let next_corner = shape.corner_approx((no + 1) % border_line_count)?;
    Some(prev_corner.middle_point(&next_corner))
}

/// Port of `ShapeAndEntrySide` (`board/model/structure/ShapeAndEntrySide.java`): a trace's tree
/// shape with its dog ears cut off, plus the side a shove should push it from.
///
/// # Quirk #7
///
/// The `fromSideIndex = currentShape.borderLineIndex(cutLine)` calls (ShapeAndEntrySide.java:59,63)
/// land on `IntBox.borderLineIndex` / `IntOctagon.borderLineIndex`, which in Java are **stubs**
/// that log a warning and return `-1` unconditionally. Only `Simplex.borderLineIndex` really
/// searches. The port's `TileShape::border_line_index` answers `None` for a box or an octagon for
/// the same reason, so this constructor takes the same branches Java does. That is deliberate;
/// see docs/java-quirks.md row 7. In practice the shape at this point has been through
/// `toSimplex()` (:36), so the live path is the one that works.
#[derive(Debug, Clone, PartialEq)]
pub struct ShapeAndEntrySide {
    /// Java `public final TileShape shape` (ShapeAndEntrySide.java:17).
    pub shape: TileShape,
    /// Java `public final ShapeEntrySide fromSide` (ShapeAndEntrySide.java:18); `null` is `None`.
    pub from_side: Option<ShapeEntrySide>,
}

impl ShapeAndEntrySide {
    /// Port of the `ShapeAndEntrySide(PolylineTrace, int, boolean, boolean)` constructor
    /// (ShapeAndEntrySide.java:25-78).
    ///
    /// `board` and `trace_id` replace Java's `trace.board`, which the constructor reads three
    /// times (`:27`, and once in each of the two private cut-line helpers). `None` if `trace_id`
    /// is not a trace, or if it has no tree shape at `index`.
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
        let item = board.get_item(trace_id)?;
        let mut current_shape = item.get_tree_shape(search_tree.id(), index)?.clone();
        let mut current_from_side: Option<ShapeEntrySide> = None;
        if orthogonal {
            // ShapeAndEntrySide.java:32-33.
            current_shape = TileShape::Box(current_shape.bounding_box());
        } else {
            // ShapeAndEntrySide.java:35-69.
            let mut cut_off_at_start = false;
            let mut cut_off_at_end = false;
            current_shape = TileShape::Simplex(current_shape.to_simplex());
            let compensated_half_width = search_tree.compensated_half_width(trace, &board.rules);
            let end_cutline = calc_cutline_at_end(index, trace.polyline(), compensated_half_width);
            if let Some(end_cutline) = end_cutline {
                let cut_plane = TileShape::get_instance_from_line(end_cutline);
                let tmp_shape = current_shape.intersection(&cut_plane);
                if tmp_shape != current_shape && !tmp_shape.is_empty() {
                    current_shape = TileShape::Simplex(tmp_shape.to_simplex());
                    cut_off_at_end = true;
                }
            }
            let start_cutline =
                calc_cutline_at_start(index, trace.polyline(), compensated_half_width);
            if let Some(start_cutline) = start_cutline {
                let cut_plane = TileShape::get_instance_from_line(start_cutline);
                let tmp_shape = current_shape.intersection(&cut_plane);
                if tmp_shape != current_shape && !tmp_shape.is_empty() {
                    current_shape = TileShape::Simplex(tmp_shape.to_simplex());
                    cut_off_at_start = true;
                }
            }
            // ShapeAndEntrySide.java:55-69.
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
        // ShapeAndEntrySide.java:71-75.
        if current_from_side.is_none() && !in_shove_check {
            current_from_side = Some(ShapeEntrySide::from_polyline(
                trace.polyline(),
                index,
                &current_shape,
            ));
        }
        Some(ShapeAndEntrySide {
            shape: current_shape,
            from_side: current_from_side,
        })
    }
}

/// Port of the private `ShapeAndEntrySide.calcCutlineAtEnd`
/// (ShapeAndEntrySide.java:80-100).
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

/// Port of the private `ShapeAndEntrySide.calcCutlineAtStart`
/// (ShapeAndEntrySide.java:102-119).
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
