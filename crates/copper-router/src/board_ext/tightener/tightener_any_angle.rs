use copper_board::prelude::*;
use copper_geometry::{Direction, IntPoint, Line, Point, Polyline, Side, Signum};

use super::base::{C_MAX_COS_ANGLE, TightenerBase, new_polyline, new_polyline_normalised};
use super::tightener_45::{acute_add_line, trace_polyline_of};

const SKIP_LENGTH: f64 = 10.0;

pub struct TraceTightenerAnyAngle<'a> {
    pub(crate) base: TightenerBase<'a>,
}

impl<'a> TraceTightenerAnyAngle<'a> {
    pub(crate) fn new(base: TightenerBase<'a>) -> TraceTightenerAnyAngle<'a> {
        TraceTightenerAnyAngle { base }
    }

    pub(crate) fn pull_tight(
        &mut self,
        board: &mut Board,
        polyline: &Polyline,
    ) -> Option<Polyline> {
        let mut new_result = polyline.clone();
        let mut ever_changed = false;
        // :37-38.
        let mut changed = true;
        while changed && !self.base.is_stop_requested() {
            let mut current = new_result;
            let mut any = false;
            // :40.
            if let Some(tmp) = self.base.skip_segments_of_length_0(board, &current) {
                current = tmp;
                any = true;
            }
            // :41.
            if let Some(tmp0) = self.reduce_lines(board, &current) {
                current = tmp0;
                any = true;
            }
            // :42.
            if let Some(tmp1) = self.skip_lines(board, &current) {
                current = tmp1;
                any = true;
            }
            // :51.
            if let Some(tmp2) = self.reduce_corners(board, &current) {
                current = tmp2;
                any = true;
            }
            // :52.
            if let Some(tmp3) = self.reposition_lines(board, &current) {
                current = tmp3;
                any = true;
            }
            // :53.
            if let Some(result) = self.smoothen_corners(board, &current) {
                current = result;
                any = true;
            }
            new_result = current;
            changed = any;
            ever_changed |= any;
        }
        // :55.
        if ever_changed { Some(new_result) } else { None }
    }

    fn reduce_corners(&mut self, board: &mut Board, polyline: &Polyline) -> Option<Polyline> {
        // :62-65.
        if polyline.lines().len() < 4 {
            return None;
        }
        let last_index = polyline.lines().len() - 4;
        let mut new_lines: Vec<Line> = vec![polyline.lines()[0]; polyline.lines().len()];
        new_lines[0] = polyline.lines()[0];
        new_lines[1] = polyline.lines()[1];
        let mut new_line_index: usize = 1;
        let mut polyline_changed = false;
        let mut current_lines: [Option<Line>; 3] = [None, None, None];

        // :77.
        for i in 0..=last_index {
            let mut skip_line = false;
            // :79-85.
            let new_a =
                new_lines[new_line_index - 1].intersection_approx(&new_lines[new_line_index]);
            let new_b = polyline
                .corner_approx(i + 2)
                .expect("i + 2 is below cornerCount");
            let in_clip_shape = self.base.current_clip_shape.is_none()
                || (self.base.clip_contains(&new_a)
                    && self.base.clip_contains(&new_b)
                    && self.base.clip_contains(
                        &polyline
                            .corner_approx(new_line_index)
                            .expect("newLineIndex is below cornerCount"),
                    ));

            if in_clip_shape {
                // :88.
                current_lines[1] = Some(Line::new(new_a.round(), new_b.round()));
                let mut ok = true;
                let first_corner = polyline
                    .first_corner()
                    .expect("a polyline has a first corner");
                let last_corner = polyline
                    .last_corner()
                    .expect("a polyline has a last corner");
                // :90-100.
                if new_line_index == 1 {
                    if !matches!(first_corner, Point::Int(_)) {
                        // The first corner must not be changed.
                        ok = false;
                    } else {
                        let dir = current_lines[1].expect("just set").direction();
                        current_lines[0] = Some(
                            Line::from_direction_any(
                                int_point_of(&first_corner),
                                &Direction::Int(dir.turn_45_degree(2)),
                            )
                            .expect("turn45Degree of an IntDirection is an IntDirection"),
                        );
                    }
                } else {
                    current_lines[0] = Some(new_lines[new_line_index - 1]);
                }
                // :101-111.
                if i == last_index {
                    if !matches!(last_corner, Point::Int(_)) {
                        // The last corner must not be changed.
                        ok = false;
                    } else {
                        let dir = current_lines[1].expect("just set").direction();
                        current_lines[2] = Some(
                            Line::from_direction_any(
                                int_point_of(&last_corner),
                                &Direction::Int(dir.turn_45_degree(2)),
                            )
                            .expect("turn45Degree of an IntDirection is an IntDirection"),
                        );
                    }
                } else {
                    current_lines[2] = Some(polyline.lines()[i + 3]);
                }

                // :113-134: the intersections must land near `newA` and `newB` — near-parallel
                // lines are numerically unstable.
                const CHECK_DIST: f64 = 100.0;
                if ok {
                    let check_is = current_lines[0]
                        .expect("set above when ok")
                        .intersection_approx(&current_lines[1].expect("just set"));
                    if check_is.distance_square(&new_a) > CHECK_DIST {
                        ok = false;
                    }
                }
                if ok {
                    let check_is = current_lines[1]
                        .expect("just set")
                        .intersection_approx(&current_lines[2].expect("set above when ok"));
                    if check_is.distance_square(&new_b) > CHECK_DIST {
                        ok = false;
                    }
                }
                // :135-143.
                if ok && i == 1 && !matches!(first_corner, Point::Int(_)) {
                    let new_corner = current_lines[0]
                        .expect("set above when ok")
                        .intersection(&current_lines[1].expect("just set"));
                    if new_corner.side_of_line(&new_lines[0])
                        != polyline
                            .corner(1)
                            .expect("a polyline has a second corner")
                            .side_of_line(&new_lines[0])
                    {
                        ok = false;
                    }
                }
                // :144-154.
                if ok
                    && last_index >= 1
                    && i == last_index - 1
                    && !matches!(last_corner, Point::Int(_))
                {
                    let new_corner = current_lines[1]
                        .expect("just set")
                        .intersection(&current_lines[2].expect("set above when ok"));
                    if new_corner.side_of_line(&new_lines[0])
                        != polyline
                            .corner(polyline.corner_count() - 2)
                            .expect("cornerCount - 2 is a corner index")
                            .side_of_line(&new_lines[0])
                    {
                        ok = false;
                    }
                }
                // :155-171.
                let mut current_polyline: Option<Polyline> = None;
                if ok {
                    let skip_corner =
                        new_lines[new_line_index].intersection_approx(&polyline.lines()[i + 2]);
                    // `new Polyline(currentLines)` normalises **currentLines itself**, and
                    // `:186`/`:188`/`:192` read its elements back out — see
                    // `new_polyline_normalised`.
                    let check_lines = vec![
                        current_lines[0].expect("set above when ok"),
                        current_lines[1].expect("just set"),
                        current_lines[2].expect("set above when ok"),
                    ];
                    let (built, check_lines) = new_polyline_normalised(&check_lines);
                    for (slot, line) in current_lines.iter_mut().zip(check_lines.iter()) {
                        *slot = Some(*line);
                    }
                    if built.lines().len() != 3 {
                        ok = false;
                    }
                    let length_before = skip_corner.distance(&new_a) + skip_corner.distance(&new_b);
                    // 1.5 added because of possible inaccuracy SQRT_2 by twice rounding.
                    let length_after = built.length_approx() + 1.5;
                    if length_after >= length_before {
                        // May happen from rounding to integer; prevents an infinite loop.
                        ok = false;
                    }
                    current_polyline = Some(built);
                }
                // :173-182.
                if ok {
                    let shape_to_check = current_polyline
                        .as_ref()
                        .expect("set when ok")
                        .offset_shape(self.base.current_half_width, 0)
                        .expect("a three-line polyline has one offset shape");
                    skip_line = self.base.check(board, &shape_to_check);
                }
            }
            // :184-207.
            if skip_line {
                polyline_changed = true;
                new_lines[new_line_index] = current_lines[1].expect("set when skipLine");
                if new_line_index == 1 {
                    // Make the first line perpendicular to the current line.
                    new_lines[0] = current_lines[0].expect("set when skipLine");
                }
                if i == last_index {
                    // Make the last line perpendicular to the current line.
                    new_line_index += 1;
                    new_lines[new_line_index] = current_lines[2].expect("set when skipLine");
                }
                if board.changed_area.is_some() {
                    let layer = self.base.current_layer;
                    board.join_changed_area(&new_a, layer);
                    board.join_changed_area(&new_b, layer);
                }
            } else {
                new_line_index += 1;
                new_lines[new_line_index] = polyline.lines()[i + 2];
                if i == last_index {
                    new_line_index += 1;
                    new_lines[new_line_index] = polyline.lines()[i + 3];
                }
            }
            // :208-211: skip the line if it is parallel to the previous one.
            if new_lines[new_line_index].is_parallel(&new_lines[new_line_index - 1]) {
                new_line_index -= 1;
            }
        }
        // :213-218.
        if !polyline_changed {
            return None;
        }
        new_lines.truncate(new_line_index + 1);
        Some(new_polyline(new_lines))
    }

    fn smoothen_corners(&mut self, board: &mut Board, polyline: &Polyline) -> Option<Polyline> {
        // :223-225.
        if polyline.lines().len() < 4 {
            return None;
        }
        // :226-228.
        let mut polyline_changed = false;
        let mut lines: Vec<Line> = polyline.lines().to_vec();
        // :230.
        let mut i: usize = 0;
        while i + 3 < lines.len() {
            if let Some(new_line) = self.smoothen_corner(board, &lines, i) {
                // :232-241.
                polyline_changed = true;
                lines.insert(i + 2, new_line);
                i += 1;
            }
            i += 1;
        }
        // :243-246.
        if !polyline_changed {
            return None;
        }
        Some(new_polyline(lines))
    }

    pub(crate) fn reposition_lines(
        &mut self,
        board: &mut Board,
        polyline: &Polyline,
    ) -> Option<Polyline> {
        // :252-254.
        if polyline.lines().len() < 5 {
            return None;
        }
        // :255-257.
        let mut polyline_changed = false;
        let mut lines: Vec<Line> = polyline.lines().to_vec();
        // :258-269.
        for i in 0..lines.len() - 4 {
            if let Some(new_line) = self.reposition_line(board, &lines, i) {
                polyline_changed = true;
                lines[i + 2] = new_line;
                if lines[i + 2].is_parallel(&lines[i + 1])
                    || lines[i + 2].is_parallel(&lines[i + 3])
                {
                    // Calculation of corners is not possible before skipping parallel lines.
                    break;
                }
            }
        }
        // :270-273.
        if !polyline_changed {
            return None;
        }
        Some(new_polyline(lines))
    }

    fn reduce_lines(&mut self, board: &mut Board, polyline: &Polyline) -> Option<Polyline> {
        // :281-283.
        if polyline.lines().len() < 6 {
            return None;
        }
        // :284-285.
        let mut polyline_changed = false;
        let mut lines: Vec<Line> = polyline.lines().to_vec();
        // :286 — `lines` is reassigned inside the loop, so the bound is re-read every round and
        // `--i` at `:387` re-examines the index the removal moved into.
        let mut i: usize = 2;
        while i + 2 < lines.len() {
            // :287-294.
            let prev_corner = lines[i - 2].intersection_approx(&lines[i - 1]);
            let next_corner = lines[i + 1].intersection_approx(&lines[i + 2]);
            let in_clip_shape = self.base.current_clip_shape.is_none()
                || (self.base.clip_contains(&prev_corner) && self.base.clip_contains(&next_corner));
            if !in_clip_shape {
                i += 1;
                continue;
            }
            // :295-301.
            let translate_line = lines[i];
            let prev_dist = translate_line.signed_distance(&prev_corner);
            let next_dist = translate_line.signed_distance(&next_corner);
            if Signum::of_f64(prev_dist) != Signum::of_f64(next_dist) {
                // The 2 corners are on different sides of the translateLine.
                i += 1;
                continue;
            }
            // :302-312.
            let mut translate_dist = if prev_dist.abs() < next_dist.abs() {
                prev_dist
            } else {
                next_dist
            };
            if translate_dist == 0.0 {
                // The line segment may have length 0.
                i += 1;
                continue;
            }
            // :313-324: make sure we have crossed the nearest corner.
            let line_side = translate_line.side_of_float_exact(&prev_corner);
            let mut new_line = translate_line.translate(-translate_dist);
            let sign = Signum::as_int_f64(translate_dist);
            let mut new_line_side_of_prev_corner = new_line.side_of_float_exact(&prev_corner);
            let mut new_line_side_of_next_corner = new_line.side_of_float_exact(&next_corner);
            while new_line_side_of_prev_corner == line_side
                && new_line_side_of_next_corner == line_side
            {
                translate_dist += f64::from(sign) * 0.5;
                new_line = translate_line.translate(-translate_dist);
                new_line_side_of_prev_corner = new_line.side_of_float_exact(&prev_corner);
                new_line_side_of_next_corner = new_line.side_of_float_exact(&next_corner);
            }
            // :325-336.
            let mut crossed_corners_before_count = 0_usize;
            let mut crossed_corners_after_count = 0_usize;
            if new_line_side_of_prev_corner != line_side {
                crossed_corners_before_count += 1;
            }
            if new_line_side_of_next_corner != line_side {
                crossed_corners_after_count += 1;
            }
            if crossed_corners_before_count > 1 || crossed_corners_after_count > 1 {
                i += 1;
                continue;
            }
            // :337-356: the next-nearest corner and the nearest corner must be on different
            // sides of newLine.
            if crossed_corners_before_count > 0 {
                if i < 3 {
                    i += 1;
                    continue;
                }
                let prev_prev_corner = lines[i - 3].intersection_approx(&lines[i - 2]);
                if new_line.side_of_float_exact(&prev_prev_corner) != line_side {
                    i += 1;
                    continue;
                }
            }
            if crossed_corners_after_count > 0 {
                if i + 3 >= lines.len() {
                    i += 1;
                    continue;
                }
                let next_next_corner = lines[i + 2].intersection_approx(&lines[i + 3]);
                if new_line.side_of_float_exact(&next_next_corner) != line_side {
                    i += 1;
                    continue;
                }
            }
            // :357-368.
            let keep_before_ind = i - crossed_corners_before_count;
            let (tmp, current_lines) = splice_and_normalise(
                &lines,
                keep_before_ind,
                new_line,
                i + 1 + crossed_corners_after_count,
            );
            // :369-379.
            let mut check_ok = false;
            if tmp.lines().len() == current_lines.len() {
                let shape_to_check = tmp
                    .offset_shape(self.base.current_half_width, keep_before_ind - 1)
                    .expect("keepBeforeInd - 1 is below tileShapeCount");
                check_ok = self.base.check(board, &shape_to_check);
            }
            // :380-388.
            if check_ok {
                if board.changed_area.is_some() {
                    let layer = self.base.current_layer;
                    board.join_changed_area(&prev_corner, layer);
                    board.join_changed_area(&next_corner, layer);
                }
                polyline_changed = true;
                lines = current_lines;
                i -= 1;
            }
            i += 1;
        }
        // :390-393.
        if !polyline_changed {
            return None;
        }
        Some(new_polyline(lines))
    }

    fn smoothen_corner(
        &mut self,
        board: &mut Board,
        lines: &[Line],
        start_no: usize,
    ) -> Option<Line> {
        // :397-399.
        if lines.len() - start_no < 4 {
            return None;
        }
        // :400-403.
        let current_corner = lines[start_no + 1].intersection_approx(&lines[start_no + 2]);
        if self.base.current_clip_shape.is_some() && !self.base.clip_contains(&current_corner) {
            return None;
        }
        // :404-409: lines that are already nearly parallel are not divided any further.
        let cosinus_angle = lines[start_no + 1].cos_angle(&lines[start_no + 2]);
        if cosinus_angle > C_MAX_COS_ANGLE {
            return None;
        }
        // :410-420.
        let prev_corner = lines[start_no].intersection_approx(&lines[start_no + 1]);
        let next_corner = lines[start_no + 2].intersection_approx(&lines[start_no + 3]);
        let prev_dir = lines[start_no + 1].direction();
        let next_dir = lines[start_no + 2].direction();
        let middle_dir = prev_dir.middle_approx(&next_dir);
        let translate_line =
            TightenerBase::line_through(&current_corner, &Direction::Int(middle_dir));
        // :421-434.
        let prev_dist = translate_line.signed_distance(&prev_corner);
        let next_dist = translate_line.signed_distance(&next_corner);
        let (nearest_point, mut max_translate_dist) = if prev_dist.abs() < next_dist.abs() {
            (prev_corner, prev_dist)
        } else {
            (next_corner, next_dist)
        };
        if max_translate_dist.abs() < 1.0 {
            return None;
        }
        // :435-443.
        let mut current_lines: Vec<Line> = Vec::with_capacity(lines.len() + 1);
        current_lines.extend_from_slice(&lines[..start_no + 2]);
        current_lines.push(lines[start_no + 2]); // placeholder, overwritten at :450 every round
        current_lines.extend_from_slice(&lines[start_no + 2..]);
        let mut translate_dist = max_translate_dist;
        let mut delta_dist = max_translate_dist;
        let side_of_nearest_point = translate_line.side_of_float_exact(&nearest_point);
        let sign = Signum::as_int_f64(max_translate_dist);
        let mut result: Option<Line> = None;
        // :444.
        while delta_dist.abs() > f64::from(self.base.min_translate_dist) {
            let mut check_ok = false;
            // :446-449.
            let new_line = translate_line.translate(-translate_dist);
            let new_line_side_of_nearest_point = new_line.side_of_float_exact(&nearest_point);
            if new_line_side_of_nearest_point == side_of_nearest_point
                || new_line_side_of_nearest_point == Side::Collinear
            {
                // :450-462.
                current_lines[start_no + 2] = new_line;
                // `new Polyline(currentLines)` normalises **currentLines itself**, and `:465`
                // reads `currentLines[startNo + 2]` back out — see `new_polyline_normalised`.
                let (tmp, normalised) = new_polyline_normalised(&current_lines);
                current_lines = normalised;
                if tmp.lines().len() == current_lines.len() {
                    let shape_to_check = tmp
                        .offset_shape(self.base.current_half_width, start_no + 1)
                        .expect("startNo + 1 is below tileShapeCount");
                    check_ok = self.base.check(board, &shape_to_check);
                }
                // :463-473.
                delta_dist /= 2.0;
                if check_ok {
                    result = Some(current_lines[start_no + 2]);
                    if translate_dist == max_translate_dist {
                        // Biggest possible change.
                        break;
                    }
                    translate_dist += delta_dist;
                } else {
                    translate_dist -= delta_dist;
                }
            } else {
                // :474-479: moved a little bit too far at the first time because of numerical
                // inaccuracy.
                let shorten_value = f64::from(sign) * 0.5;
                max_translate_dist -= shorten_value;
                translate_dist -= shorten_value;
                delta_dist -= shorten_value;
            }
        }
        // :481-483.
        result?;
        // :485-492.
        if board.changed_area.is_some() {
            let layer = self.base.current_layer;
            let new_prev_corner =
                current_lines[start_no].intersection_approx(&current_lines[start_no + 1]);
            let new_next_corner =
                current_lines[start_no + 3].intersection_approx(&current_lines[start_no + 4]);
            board.join_changed_area(&new_prev_corner, layer);
            board.join_changed_area(&new_next_corner, layer);
        }
        // :493.
        result
    }

    pub(crate) fn reposition_line(
        &mut self,
        board: &mut Board,
        lines: &[Line],
        start_no: usize,
    ) -> Option<Line> {
        // :498-500.
        if lines.len() - start_no < 5 {
            return None;
        }
        // :501-510: the corners of the line to translate must be inside the clip shape.
        if self.base.current_clip_shape.is_some() {
            for i in 1..3 {
                let current_corner =
                    lines[start_no + i].intersection_approx(&lines[start_no + i + 1]);
                if !self.base.clip_contains(&current_corner) {
                    return None;
                }
            }
        }
        // :511-514.
        let translate_line = lines[start_no + 2];
        let mut prev_corner = lines[start_no].intersection_approx(&lines[start_no + 1]);
        let mut next_corner = lines[start_no + 3].intersection_approx(&lines[start_no + 4]);
        let mut prev_dist = translate_line.signed_distance(&prev_corner);
        // :515-528: move also all lines through the start corner of the line to translate.
        let mut corners_skipped_before: usize = 0;
        let mut corners_skipped_after: usize = 0;
        const EPSILON: f64 = 0.001;
        while prev_dist.abs() < EPSILON {
            corners_skipped_before += 1;
            let Some(current_no) = start_no.checked_sub(corners_skipped_before) else {
                // The first corner is on the line to translate.
                return None;
            };
            prev_corner = lines[current_no].intersection_approx(&lines[current_no + 1]);
            prev_dist = translate_line.signed_distance(&prev_corner);
        }
        // :529-540.
        let mut next_dist = translate_line.signed_distance(&next_corner);
        while next_dist.abs() < EPSILON {
            corners_skipped_after += 1;
            let current_no = start_no + 3 + corners_skipped_after;
            if current_no + 2 >= lines.len() {
                // The last corner is on the line to translate.
                return None;
            }
            next_corner = lines[current_no].intersection_approx(&lines[current_no + 1]);
            next_dist = translate_line.signed_distance(&next_corner);
        }
        // :541-544.
        if Signum::of_f64(prev_dist) != Signum::of_f64(next_dist) {
            return None;
        }
        // :545-553.
        let (nearest_point, mut max_translate_dist) = if prev_dist.abs() < next_dist.abs() {
            (prev_corner, prev_dist)
        } else {
            (next_corner, next_dist)
        };
        let mut current_lines: Vec<Line> = lines.to_vec();
        // :558-563.
        let mut translate_dist = max_translate_dist;
        let mut delta_dist = max_translate_dist;
        let side_of_nearest_point = translate_line.side_of_float_exact(&nearest_point);
        let sign = Signum::as_int_f64(max_translate_dist);
        let mut result: Option<Line> = None;
        let mut first_time = true;
        // :564.
        while first_time || delta_dist.abs() > f64::from(self.base.min_translate_dist) {
            let mut check_ok = false;
            // :566.
            let mut new_line = translate_line.translate(-translate_dist);
            if first_time && translate_dist.abs() < 1.0 {
                if new_line.equals_geometric(&translate_line) {
                    // Try the parallel line through the nearestPoint.
                    let rounded_nearest_point: IntPoint = nearest_point.round();
                    if nearest_point.distance(&rounded_nearest_point.to_float())
                        < translate_dist.abs()
                    {
                        new_line = Line::from_direction(
                            rounded_nearest_point,
                            &translate_line.direction(),
                        );
                    }
                    first_time = false;
                }
                if new_line.equals_geometric(&translate_line) {
                    return None;
                }
            }
            // :580-582.
            let new_line_side_of_nearest_point = new_line.side_of_float_exact(&nearest_point);
            if new_line_side_of_nearest_point == side_of_nearest_point
                || new_line_side_of_nearest_point == Side::Collinear
            {
                first_time = false;
                current_lines[start_no + 2] = new_line;
                // :585-613: `cornersSkippedBefore > 0` or `cornersSkippedAfter > 0` happens very
                // rarely, but the handling is important — for example when 3 or more consecutive
                // corners are equal.
                let mut prev_translated_line = new_line;
                for k in 0..corners_skipped_before {
                    let prev_line_no = start_no + 1 - corners_skipped_before;
                    let current_prev_corner =
                        prev_translated_line.intersection_approx(&current_lines[prev_line_no]);
                    let current_translate_line = lines[start_no + 1 - k];
                    let current_translate_dist =
                        current_translate_line.signed_distance(&current_prev_corner);
                    prev_translated_line =
                        current_translate_line.translate(-current_translate_dist);
                    current_lines[start_no + 1 - k] = prev_translated_line;
                }
                let mut prev_translated_line = new_line;
                for k in 0..corners_skipped_after {
                    let next_line_no = start_no + 3 + corners_skipped_after;
                    let current_next_corner =
                        prev_translated_line.intersection_approx(&current_lines[next_line_no]);
                    let current_translate_line = lines[start_no + 3 + k];
                    let current_translate_dist =
                        current_translate_line.signed_distance(&current_next_corner);
                    prev_translated_line =
                        current_translate_line.translate(-current_translate_dist);
                    current_lines[start_no + 3 + k] = prev_translated_line;
                }
                // :614-625. `new Polyline(currentLines)` normalises **currentLines itself**,
                // and `:625` reads `currentLines[startNo + 2]` back out — see
                // `new_polyline_normalised`.
                let (tmp, normalised) = new_polyline_normalised(&current_lines);
                current_lines = normalised;
                if tmp.lines().len() == current_lines.len() {
                    let shape_to_check = tmp
                        .offset_shape(self.base.current_half_width, start_no + 1)
                        .expect("startNo + 1 is below tileShapeCount");
                    check_ok = self.base.check(board, &shape_to_check);
                }
                // :626-636.
                delta_dist /= 2.0;
                if check_ok {
                    result = Some(current_lines[start_no + 2]);
                    if translate_dist == max_translate_dist {
                        // Biggest possible change.
                        break;
                    }
                    translate_dist += delta_dist;
                } else {
                    translate_dist -= delta_dist;
                }
            } else {
                // :637-642.
                let shorten_value = f64::from(sign) * 0.5;
                max_translate_dist -= shorten_value;
                translate_dist -= shorten_value;
                delta_dist -= shorten_value;
            }
        }
        // :644-646.
        result?;
        // :648-655.
        if board.changed_area.is_some() {
            let layer = self.base.current_layer;
            let new_prev_corner =
                current_lines[start_no].intersection_approx(&current_lines[start_no + 1]);
            let new_next_corner =
                current_lines[start_no + 3].intersection_approx(&current_lines[start_no + 4]);
            board.join_changed_area(&new_prev_corner, layer);
            board.join_changed_area(&new_next_corner, layer);
        }
        // :656.
        result
    }

    fn skip_lines(&mut self, board: &mut Board, polyline: &Polyline) -> Option<Polyline> {
        // :660.
        let mut i: usize = 1;
        while i + 3 < polyline.lines().len() {
            // :661.
            for j in 0..=1 {
                // :662-673.
                let (mut current_line, corner1, corner2) = if j == 0 {
                    // Try to skip the line before the i+2-th line.
                    (
                        polyline.lines()[i + 2],
                        polyline.corner_approx(i).expect("i is below cornerCount"),
                        polyline
                            .corner_approx(i - 1)
                            .expect("i - 1 is below cornerCount"),
                    )
                } else {
                    // Try to skip the line after the i-th line.
                    (
                        polyline.lines()[i],
                        polyline
                            .corner_approx(i + 1)
                            .expect("i + 1 is below cornerCount"),
                        polyline
                            .corner_approx(i + 2)
                            .expect("i + 2 is below cornerCount"),
                    )
                };
                // :674-679.
                let in_clip_shape = self.base.current_clip_shape.is_none()
                    || (self.base.clip_contains(&corner1) && self.base.clip_contains(&corner2));
                if !in_clip_shape {
                    continue;
                }
                // :681-705.
                let mut side1 = current_line.side_of_float_exact(&corner1);
                let mut side2 = current_line.side_of_float_exact(&corner2);
                if side1 != side2 {
                    // The two corners are on different sides of the line.
                    let reduced_polyline = skip_lines_of(polyline, i + 1, i + 1);
                    if reduced_polyline.lines().len() == polyline.lines().len() - 1 {
                        let shape_index = if j == 0 { i } else { i - 1 };
                        let shape_to_check = reduced_polyline
                            .offset_shape(self.base.current_half_width, shape_index)
                            .expect("shapeIndex is below tileShapeCount");
                        if self.base.check(board, &shape_to_check) {
                            if board.changed_area.is_some() {
                                let layer = self.base.current_layer;
                                board.join_changed_area(&corner1, layer);
                                board.join_changed_area(&corner2, layer);
                            }
                            return Some(reduced_polyline);
                        }
                    }
                }
                // :706-709: now try skipping 2 lines.
                if i + 4 >= polyline.lines().len() {
                    break;
                }
                // :710-718.
                let corner3 = if j == 1 {
                    polyline
                        .corner_approx(i + 3)
                        .expect("i + 3 is below cornerCount")
                } else {
                    polyline
                        .corner_approx(i + 1)
                        .expect("i + 1 is below cornerCount")
                };
                if self.base.current_clip_shape.is_some() && !self.base.clip_contains(&corner3) {
                    continue;
                }
                // :719-727.
                if j == 0 {
                    // `currentLine` is one line later than in the case of skipping 1 line when
                    // coming from behind.
                    current_line = polyline.lines()[i + 3];
                    side1 = current_line.side_of_float_exact(&corner1);
                    side2 = current_line.side_of_float_exact(&corner2);
                } else {
                    side1 = current_line.side_of_float_exact(&corner3);
                }
                // :728-751.
                if side1 != side2 {
                    let reduced_polyline = skip_lines_of(polyline, i + 1, i + 2);
                    if reduced_polyline.lines().len() == polyline.lines().len() - 2 {
                        let shape_index = if j == 0 { i } else { i - 1 };
                        let shape_to_check = reduced_polyline
                            .offset_shape(self.base.current_half_width, shape_index)
                            .expect("shapeIndex is below tileShapeCount");
                        if self.base.check(board, &shape_to_check) {
                            if board.changed_area.is_some() {
                                let layer = self.base.current_layer;
                                board.join_changed_area(&corner1, layer);
                                board.join_changed_area(&corner2, layer);
                                board.join_changed_area(&corner3, layer);
                            }
                            return Some(reduced_polyline);
                        }
                    }
                }
            }
            i += 1;
        }
        // :754.
        None
    }

    pub(crate) fn smoothen_start_corner_at_trace(
        &mut self,
        board: &mut Board,
        trace: ItemId,
    ) -> Option<Polyline> {
        let trace_polyline = trace_polyline_of(board, trace)?;
        // :765-769.
        let current_end_corner = trace_polyline.corner(0)?;
        if self.base.clip_is_outside(&current_end_corner) {
            return None;
        }
        // :771-783.
        let mut current_prev_end_corner = trace_polyline.corner(1)?;
        let skip_short_segment = !matches!(current_end_corner, Point::Int(_))
            && current_end_corner
                .to_float()
                .distance_square(&current_prev_end_corner.to_float())
                < SKIP_LENGTH;
        let mut start_line_no: usize = 1;
        if skip_short_segment {
            if trace_polyline.corner_count() < 3 {
                return None;
            }
            current_prev_end_corner = trace_polyline.corner(2)?;
            start_line_no += 1;
        }
        // :784-786.
        let line_direction = trace_polyline.lines()[start_line_no].direction();
        let prev_line_direction = trace_polyline.lines()[start_line_no + 1].direction();

        // :788-827.
        let contacts = board.trace_start_contacts(trace);
        let found = super::scan_contacts(
            board,
            &contacts,
            &trace_polyline,
            &current_end_corner,
            &current_prev_end_corner,
            &line_direction,
            &prev_line_direction,
            false,
            false,
        )?;

        // :828-833.
        let mut new_line_count = trace_polyline.lines().len() + 1;
        let mut diff: usize = 1;
        if skip_short_segment {
            new_line_count -= 1;
            diff -= 1;
        }
        if found.acute_angle {
            // :834-861.
            let other_trace_line = found.other_trace_line.expect("acuteAngle implies a match");
            let new_line_dir = if found.prev_corner_side == Some(Side::OnTheLeft) {
                other_trace_line.direction().turn_45_degree(2)
            } else {
                other_trace_line.direction().turn_45_degree(6)
            };
            let add_line = acute_add_line(
                self.base.current_half_width,
                &current_end_corner,
                &current_prev_end_corner,
                &new_line_dir,
                &found
                    .other_trace_corner_approx
                    .expect("acuteAngle implies a match"),
            )?;
            // :855-860.
            let mut new_lines: Vec<Line> = Vec::with_capacity(new_line_count);
            new_lines.push(other_trace_line);
            new_lines.push(add_line);
            new_lines.extend_from_slice(&trace_polyline.lines()[2 - diff..]);
            new_lines.truncate(new_line_count);
            return Some(new_polyline(new_lines));
        } else if found.bend {
            // :862-875.
            let other_trace_line = found.other_trace_line.expect("bend implies a match");
            let other_prev_trace_line = found.other_prev_trace_line.expect("bend implies a match");
            let mut check_line_arr: Vec<Line> = Vec::with_capacity(new_line_count);
            check_line_arr.push(other_prev_trace_line);
            check_line_arr.push(other_trace_line);
            check_line_arr.extend_from_slice(&trace_polyline.lines()[2 - diff..]);
            check_line_arr.truncate(new_line_count);
            let new_line = self.reposition_line(board, &check_line_arr, 0)?;
            let mut new_lines: Vec<Line> = Vec::with_capacity(trace_polyline.lines().len());
            new_lines.push(other_trace_line);
            new_lines.push(new_line);
            new_lines.extend_from_slice(&trace_polyline.lines()[2..]);
            return Some(new_polyline(new_lines));
        }
        // :876.
        None
    }

    pub(crate) fn smoothen_end_corner_at_trace(
        &mut self,
        board: &mut Board,
        trace: ItemId,
    ) -> Option<Polyline> {
        let trace_polyline = trace_polyline_of(board, trace)?;
        // :887-891.
        let current_end_corner = trace_polyline.last_corner()?;
        if self.base.clip_is_outside(&current_end_corner) {
            return None;
        }
        // :893-905.
        let line_count = trace_polyline.lines().len();
        let mut current_prev_end_corner =
            trace_polyline.corner(trace_polyline.corner_count() - 2)?;
        let skip_short_segment = !matches!(current_end_corner, Point::Int(_))
            && current_end_corner
                .to_float()
                .distance_square(&current_prev_end_corner.to_float())
                < SKIP_LENGTH;
        let mut end_line_no = line_count - 2;
        if skip_short_segment {
            if trace_polyline.corner_count() < 3 {
                return None;
            }
            current_prev_end_corner = trace_polyline.corner(trace_polyline.corner_count() - 3)?;
            end_line_no -= 1;
        }
        if end_line_no == 0 {
            return None;
        }
        let line_direction = trace_polyline.lines()[end_line_no].direction().opposite();
        let prev_line_direction = trace_polyline.lines()[end_line_no - 1]
            .direction()
            .opposite();

        // :910-951.
        let contacts = board.trace_end_contacts(trace);
        let found = super::scan_contacts(
            board,
            &contacts,
            &trace_polyline,
            &current_end_corner,
            &current_prev_end_corner,
            &line_direction,
            &prev_line_direction,
            false,
            true,
        )?;

        // :953-958.
        let mut new_line_count = line_count + 1;
        let mut diff: usize = 0;
        if skip_short_segment {
            new_line_count -= 1;
            diff += 1;
        }
        if found.acute_angle {
            // :960-987.
            let other_trace_line = found.other_trace_line.expect("acuteAngle implies a match");
            let new_line_dir = if found.prev_corner_side == Some(Side::OnTheLeft) {
                other_trace_line.direction().turn_45_degree(6)
            } else {
                other_trace_line.direction().turn_45_degree(2)
            };
            let add_line = acute_add_line(
                self.base.current_half_width,
                &current_end_corner,
                &current_prev_end_corner,
                &new_line_dir,
                &found
                    .other_trace_corner_approx
                    .expect("acuteAngle implies a match"),
            )?;
            // :981-986.
            let mut new_lines: Vec<Line> = vec![other_trace_line; new_line_count];
            new_lines[..line_count - 1].copy_from_slice(&trace_polyline.lines()[..line_count - 1]);
            new_lines[new_line_count - 2] = add_line;
            new_lines[new_line_count - 1] = other_trace_line;
            return Some(new_polyline(new_lines));
        } else if found.bend {
            let other_trace_line = found.other_trace_line.expect("bend implies a match");
            let other_prev_trace_line = found.other_prev_trace_line.expect("bend implies a match");
            let mut check_line_arr: Vec<Line> = vec![other_trace_line; new_line_count];
            check_line_arr[..new_line_count - 2]
                .copy_from_slice(&trace_polyline.lines()[diff..diff + new_line_count - 2]);
            check_line_arr[new_line_count - 2] = other_trace_line;
            check_line_arr[new_line_count - 1] = other_prev_trace_line;
            let new_line = self.reposition_line(board, &check_line_arr, new_line_count - 5)?;
            let mut new_lines: Vec<Line> = vec![other_trace_line; line_count];
            new_lines[..line_count - 2].copy_from_slice(&trace_polyline.lines()[..line_count - 2]);
            new_lines[line_count - 2] = new_line;
            new_lines[line_count - 1] = other_trace_line;
            return Some(new_polyline(new_lines));
        }
        // :1002.
        None
    }
}

fn skip_lines_of(polyline: &Polyline, from_no: usize, to_no: usize) -> Polyline {
    polyline
        .skip_lines(from_no, to_no)
        .unwrap_or_else(|e| panic!("Polyline.skipLines threw (quirk #22): {e}"))
}

fn int_point_of(point: &Point) -> IntPoint {
    match point {
        Point::Int(p) => *p,
        Point::Rational(_) => unreachable!("guarded by an `instanceof IntPoint` test"),
    }
}

fn splice_and_normalise(
    lines: &[Line],
    keep_before_ind: usize,
    new_line: Line,
    suffix_start: usize,
) -> (Polyline, Vec<Line>) {
    let mut current_lines: Vec<Line> =
        Vec::with_capacity(keep_before_ind + 1 + (lines.len() - suffix_start));
    current_lines.extend_from_slice(&lines[..keep_before_ind]);
    current_lines.push(new_line);
    current_lines.extend_from_slice(&lines[suffix_start..]);
    let (tmp, current_lines) = new_polyline_normalised(&current_lines);
    (tmp, current_lines)
}

#[cfg(test)]
mod reduce_lines_write_back_tests {
    use super::*;

    #[test]
    fn splice_and_normalise_writes_the_flipped_line_back_into_the_loops_array() {
        let corners = [
            Point::new(0, 0),
            Point::new(1000, 0),
            Point::new(1000, 1000),
            Point::new(0, 1000),
        ];
        let straight = Polyline::from_points(&corners);
        let lines = straight.lines().to_vec();
        assert!(lines.len() >= 4);

        let reversed = lines[1].opposite();
        assert!(!reversed.is_same_object(&lines[1]));

        let (tmp, current_lines) = splice_and_normalise(&lines, 1, reversed, 2);

        assert_eq!(current_lines.len(), lines.len());
        assert_eq!(tmp.lines().len(), current_lines.len());
        // ... and the caller's array carries the normalised line, not the one it pushed.
        assert!(
            !current_lines[1].is_same_object(&reversed),
            "`:368` must normalise the caller's array in place (quirk #188)"
        );
        assert_eq!(current_lines[1], lines[1]);
        assert_eq!(current_lines.as_slice(), tmp.lines());
        // Every other element is untouched, object for object.
        for i in (0..current_lines.len()).filter(|i| *i != 1) {
            assert!(current_lines[i].is_same_object(&lines[i]), "index {i}");
        }
    }
}
