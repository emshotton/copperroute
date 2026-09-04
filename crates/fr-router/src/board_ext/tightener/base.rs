use std::collections::BTreeSet;

use fr_board::datastructures::StopCheck;
use fr_board::prelude::*;
use fr_board::{BoardError, ItemId, TimeLimit};
use fr_geometry::{FloatPoint, IntOctagon, IntPoint, Line, Point, Polyline, Side, Signum};

pub(crate) const C_MAX_COS_ANGLE: f64 = 0.999;

pub(crate) const C_MIN_CORNER_DIST_SQUARE: f64 = 0.9;

pub(crate) fn new_polyline(lines: Vec<Line>) -> Polyline {
    Polyline::from_lines(lines).unwrap_or_else(|e| {
        panic!("new Polyline(Line[]) threw (Polyline.java:148, quirk #22): {e}")
    })
}

pub(crate) fn new_polyline_in_place(lines: &mut Vec<Line>) -> Polyline {
    Polyline::from_lines_in_place(lines).unwrap_or_else(|e| {
        panic!("new Polyline(Line[]) threw (Polyline.java:148, quirk #22): {e}")
    })
}

static NEVER_STOP: fn() -> bool = || false;

pub(crate) struct TightenerBase<'a> {
    pub(crate) only_net_no_arr: Vec<i32>,
    stoppable_thread: Option<StopCheck<'a>>,
    time_limit: Option<TimeLimit>,
    keep_point: Option<Point>,
    keep_point_layer: i32,
    pub(crate) current_layer: usize,
    pub(crate) current_half_width: i32,
    pub(crate) current_net_numbers: Vec<i32>,
    pub(crate) current_clearance_class_index: usize,
    pub(crate) current_clip_shape: Option<IntOctagon>,
    pub(crate) contact_pins: Option<BTreeSet<ItemId>>,
    pub(crate) min_translate_dist: i32,
}

impl<'a> TightenerBase<'a> {
    pub(crate) fn new(
        only_net_no_arr: Vec<i32>,
        stoppable_thread: Option<StopCheck<'a>>,
        time_limit: i32,
        keep_point: Option<Point>,
        keep_point_layer: i32,
    ) -> TightenerBase<'a> {
        TightenerBase {
            only_net_no_arr,
            stoppable_thread,
            time_limit: if time_limit > 0 {
                Some(TimeLimit::new(time_limit))
            } else {
                None
            },
            keep_point,
            keep_point_layer,
            current_layer: 0,
            current_half_width: 0,
            current_net_numbers: Vec::new(),
            current_clearance_class_index: 0,
            current_clip_shape: None,
            contact_pins: None,
            min_translate_dist: 0,
        }
    }

    pub(crate) fn is_stop_requested(&self) -> bool {
        if let Some(stop) = self.stoppable_thread
            && stop()
        {
            return true;
        }
        let Some(time_limit) = &self.time_limit else {
            return false;
        };
        time_limit.is_exceeded()
    }

    pub(crate) fn stop_check(&self) -> StopCheck<'a> {
        self.stoppable_thread.unwrap_or(&NEVER_STOP)
    }

    pub(crate) fn check(&self, board: &mut Board, shape: &fr_geometry::TileShape) -> bool {
        board.check_trace_shape(
            shape,
            self.current_layer,
            &self.current_net_numbers,
            self.current_clearance_class_index,
            self.contact_pins.as_ref(),
        )
    }

    pub(crate) fn clip_is_outside(&self, point: &Point) -> bool {
        match &self.current_clip_shape {
            Some(clip) => fr_geometry::TileShape::Octagon(*clip).is_outside(point),
            None => false,
        }
    }

    pub(crate) fn clip_contains(&self, point: &FloatPoint) -> bool {
        match &self.current_clip_shape {
            Some(clip) => clip.contains_float(point),
            None => true,
        }
    }

    pub(crate) fn avoid_acid_traps(&self, _polyline: &Polyline) -> Option<Polyline> {
        None
    }

    pub(crate) fn reposition_lines(
        &mut self,
        board: &mut Board,
        polyline: &Polyline,
    ) -> Option<Polyline> {
        if polyline.lines().len() < 5 {
            return None;
        }
        for i in 2..polyline.lines().len() - 2 {
            let Some(new_line) = self.reposition_line(board, polyline.lines(), i) else {
                continue;
            };
            let mut lines = polyline.lines().to_vec();
            lines[i] = new_line;
            let result = new_polyline(lines);
            return Some(match self.skip_segments_of_length_0(board, &result) {
                Some(skipped) => skipped,
                None => result,
            });
        }
        None
    }

    #[allow(unused_assignments)]
    pub(crate) fn reposition_line(
        &mut self,
        board: &mut Board,
        lines: &[Line],
        no: usize,
    ) -> Option<Line> {
        if (lines.len() as i64) - (no as i64) < 3 {
            return None;
        }
        if self.current_clip_shape.is_some() {
            for i in [-1_i64, 0] {
                let index = (no as i64 + i) as usize;
                let current_corner = lines[index].intersection(&lines[index + 1]);
                if self.clip_is_outside(&current_corner) {
                    return None;
                }
            }
        }
        let translate_line = lines[no];
        let prev_corner = lines[no - 2].intersection(&lines[no - 1]);
        let next_corner = lines[no + 1].intersection(&lines[no + 2]);
        let prev_dist = translate_line.signed_distance(&prev_corner.to_float());
        let next_dist = translate_line.signed_distance(&next_corner.to_float());
        if Signum::of_f64(prev_dist) != Signum::of_f64(next_dist) {
            return None;
        }
        let (nearest_point, mut max_translate_dist) = if prev_dist.abs() < next_dist.abs() {
            (prev_corner, prev_dist)
        } else {
            (next_corner, next_dist)
        };
        let mut translate_dist = max_translate_dist;
        let mut delta_dist = max_translate_dist;
        let side_of_nearest_point = translate_line.side_of(&nearest_point);
        let sign = Signum::as_int_f64(max_translate_dist);
        let mut new_line: Option<Line> = None;
        let check_line_0 = lines[no - 1];
        let check_line_2 = lines[no + 1];
        let mut first_time = true;
        while first_time || delta_dist.abs() > f64::from(self.min_translate_dist) {
            let check_line_1 = match (first_time, &nearest_point) {
                (true, Point::Int(p)) => Line::from_direction(*p, &translate_line.direction()),
                _ => translate_line.translate(-translate_dist),
            };
            if check_line_1.equals_geometric(&translate_line) {
                return None;
            }
            let new_line_side_of_nearest_point = check_line_1.side_of(&nearest_point);
            if new_line_side_of_nearest_point != side_of_nearest_point
                && new_line_side_of_nearest_point != Side::Collinear
            {
                let shorten_value = f64::from(sign) * 0.5;
                max_translate_dist -= shorten_value;
                translate_dist -= shorten_value;
                delta_dist -= shorten_value;
                continue;
            }
            let mut check_lines = vec![check_line_0, check_line_1, check_line_2];
            let tmp = new_polyline_in_place(&mut check_lines);
            let check_line_1 = check_lines[1];
            let mut check_ok = false;
            if tmp.lines().len() == 3 {
                let shape_to_check = tmp
                    .offset_shape(self.current_half_width, 0)
                    .expect("a three-line polyline has one offset shape");
                check_ok = self.check(board, &shape_to_check);
            }
            delta_dist /= 2.0;
            if check_ok {
                new_line = Some(check_line_1);
                if first_time {
                    break;
                }
                translate_dist += delta_dist;
            } else {
                translate_dist -= delta_dist;
            }
            first_time = false;
        }
        if let Some(new_line) = new_line
            && board.changed_area.is_some()
        {
            let layer = self.current_layer;
            board.join_changed_area(&check_line_0.intersection_approx(&new_line), layer);
            board.join_changed_area(&check_line_2.intersection_approx(&new_line), layer);
            board.join_changed_area(&lines[no - 1].intersection_approx(&lines[no]), layer);
            board.join_changed_area(&lines[no].intersection_approx(&lines[no + 1]), layer);
        }
        new_line
    }

    pub(crate) fn skip_segments_of_length_0(
        &mut self,
        board: &mut Board,
        polyline: &Polyline,
    ) -> Option<Polyline> {
        let mut polyline_changed = false;
        let mut current_polyline = polyline.clone();
        let mut i: usize = 1;
        while i + 1 < current_polyline.lines().len() {
            let try_skip = if i == 1 || i == current_polyline.lines().len() - 2 {
                let prev_corner = current_polyline
                    .corner(i - 1)
                    .expect("i - 1 is below cornerCount");
                let current_corner = current_polyline.corner(i).expect("i is below cornerCount");
                current_corner == prev_corner
            } else {
                let prev_corner = current_polyline
                    .corner_approx(i - 1)
                    .expect("i - 1 is below cornerCount");
                let current_corner = current_polyline
                    .corner_approx(i)
                    .expect("i is below cornerCount");
                current_corner.distance_square(&prev_corner) < C_MIN_CORNER_DIST_SQUARE
            };

            if try_skip {
                let mut current_lines: Vec<Line> =
                    Vec::with_capacity(current_polyline.lines().len() - 1);
                current_lines.extend_from_slice(&current_polyline.lines()[..i]);
                current_lines.extend_from_slice(&current_polyline.lines()[i + 1..]);
                let tmp = new_polyline(current_lines.clone());
                let mut check_ok = tmp.lines().len() == current_lines.len();
                if check_ok && !current_polyline.lines()[i].is_multiple_of_45_degree() {
                    if i > 1 {
                        let shape_to_check = tmp
                            .offset_shape(self.current_half_width, i - 2)
                            .expect("i - 2 is below tileShapeCount");
                        check_ok = self.check(board, &shape_to_check);
                    }
                    if check_ok && i < current_polyline.lines().len() - 2 {
                        let shape_to_check = tmp
                            .offset_shape(self.current_half_width, i - 1)
                            .expect("i - 1 is below tileShapeCount");
                        check_ok = self.check(board, &shape_to_check);
                    }
                }
                if check_ok {
                    polyline_changed = true;
                    current_polyline = tmp;
                    i -= 1;
                }
            }
            i += 1;
        }
        if !polyline_changed {
            return None;
        }
        Some(current_polyline)
    }

    pub(crate) fn split_traces_at_keep_point(
        &mut self,
        board: &mut Board,
    ) -> Result<bool, BoardError> {
        let Some(keep_point) = self.keep_point.clone() else {
            return Ok(false);
        };
        let layer = usize::try_from(self.keep_point_layer).ok();
        let picked_items = board.pick_items(&keep_point, layer);
        for current_item in picked_items {
            if !matches!(board.items.get(&current_item), Some(Item::Trace(_))) {
                continue;
            }
            let split_pieces = board.split_trace_at_point(current_item, &keep_point)?;
            if split_pieces.is_some() {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub(crate) fn line_through(corner: &FloatPoint, dir: &fr_geometry::Direction) -> Line {
        let rounded: IntPoint = corner.round();
        Line::from_direction_any(rounded, dir)
            .expect("a Direction built from IntPoint differences is an IntDirection")
    }
}
