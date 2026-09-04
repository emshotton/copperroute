use fr_board::prelude::*;
use fr_geometry::{Direction, IntPoint, Line, Point, Polyline, Side, Signum};

use super::base::{C_MAX_COS_ANGLE, TightenerBase, new_polyline, new_polyline_in_place};
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
        let (mut new_result, mut ever_changed) = match self.base.avoid_acid_traps(polyline) {
            Some(replacement) => (replacement, true),
            None => (polyline.clone(), false),
        };
        let mut changed = true;
        while changed && !self.base.is_stop_requested() {
            let mut current = new_result;
            let mut any = false;
            if let Some(tmp) = self.base.skip_segments_of_length_0(board, &current) {
                current = tmp;
                any = true;
            }
            if let Some(tmp0) = self.reduce_lines(board, &current) {
                current = tmp0;
                any = true;
            }
            if let Some(tmp1) = self.skip_lines(board, &current) {
                current = tmp1;
                any = true;
            }
            if let Some(tmp2) = self.reduce_corners(board, &current) {
                current = tmp2;
                any = true;
            }
            if let Some(tmp3) = self.reposition_lines(board, &current) {
                current = tmp3;
                any = true;
            }
            if let Some(result) = self.smoothen_corners(board, &current) {
                current = result;
                any = true;
            }
            new_result = current;
            changed = any;
            ever_changed |= any;
        }
        if ever_changed { Some(new_result) } else { None }
    }

                    fn reduce_corners(&mut self, board: &mut Board, polyline: &Polyline) -> Option<Polyline> {
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

        for i in 0..=last_index {
            let mut skip_line = false;
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
                current_lines[1] = Some(Line::new(new_a.round(), new_b.round()));
                let mut ok = true;
                let first_corner = polyline
                    .first_corner()
                    .expect("a polyline has a first corner");
                let last_corner = polyline
                    .last_corner()
                    .expect("a polyline has a last corner");
                if new_line_index == 1 {
                    if !matches!(first_corner, Point::Int(_)) {
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
                if i == last_index {
                    if !matches!(last_corner, Point::Int(_)) {
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
                let mut current_polyline: Option<Polyline> = None;
                if ok {
                    let skip_corner =
                        new_lines[new_line_index].intersection_approx(&polyline.lines()[i + 2]);
                    let mut check_lines = vec![
                        current_lines[0].expect("set above when ok"),
                        current_lines[1].expect("just set"),
                        current_lines[2].expect("set above when ok"),
                    ];
                    let built = new_polyline_in_place(&mut check_lines);
                    for (slot, line) in current_lines.iter_mut().zip(check_lines.iter()) {
                        *slot = Some(*line);
                    }
                    if built.lines().len() != 3 {
                        ok = false;
                    }
                    let length_before = skip_corner.distance(&new_a) + skip_corner.distance(&new_b);
                    let length_after = built.length_approx() + 1.5;
                    if length_after >= length_before {
                        ok = false;
                    }
                    current_polyline = Some(built);
                }
                if ok {
                    let shape_to_check = current_polyline
                        .as_ref()
                        .expect("set when ok")
                        .offset_shape(self.base.current_half_width, 0)
                        .expect("a three-line polyline has one offset shape");
                    skip_line = self.base.check(board, &shape_to_check);
                }
            }
            if skip_line {
                polyline_changed = true;
                new_lines[new_line_index] = current_lines[1].expect("set when skipLine");
                if new_line_index == 1 {
                    new_lines[0] = current_lines[0].expect("set when skipLine");
                }
                if i == last_index {
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
            if new_lines[new_line_index].is_parallel(&new_lines[new_line_index - 1]) {
                new_line_index -= 1;
            }
        }
        if !polyline_changed {
            return None;
        }
        new_lines.truncate(new_line_index + 1);
        Some(new_polyline(new_lines))
    }

            fn smoothen_corners(&mut self, board: &mut Board, polyline: &Polyline) -> Option<Polyline> {
        if polyline.lines().len() < 4 {
            return None;
        }
        let mut polyline_changed = false;
        let mut lines: Vec<Line> = polyline.lines().to_vec();
        let mut i: usize = 0;
        while i + 3 < lines.len() {
            if let Some(new_line) = self.smoothen_corner(board, &lines, i) {
                polyline_changed = true;
                lines.insert(i + 2, new_line);
                i += 1;
            }
            i += 1;
        }
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
        if polyline.lines().len() < 5 {
            return None;
        }
        let mut polyline_changed = false;
        let mut lines: Vec<Line> = polyline.lines().to_vec();
        for i in 0..lines.len() - 4 {
            if let Some(new_line) = self.reposition_line(board, &lines, i) {
                polyline_changed = true;
                lines[i + 2] = new_line;
                if lines[i + 2].is_parallel(&lines[i + 1])
                    || lines[i + 2].is_parallel(&lines[i + 3])
                {
                    break;
                }
            }
        }
        if !polyline_changed {
            return None;
        }
        Some(new_polyline(lines))
    }

                fn reduce_lines(&mut self, board: &mut Board, polyline: &Polyline) -> Option<Polyline> {
        if polyline.lines().len() < 6 {
            return None;
        }
        let mut polyline_changed = false;
        let mut lines: Vec<Line> = polyline.lines().to_vec();
        let mut i: usize = 2;
        while i + 2 < lines.len() {
            let prev_corner = lines[i - 2].intersection_approx(&lines[i - 1]);
            let next_corner = lines[i + 1].intersection_approx(&lines[i + 2]);
            let in_clip_shape = self.base.current_clip_shape.is_none()
                || (self.base.clip_contains(&prev_corner) && self.base.clip_contains(&next_corner));
            if !in_clip_shape {
                i += 1;
                continue;
            }
            let translate_line = lines[i];
            let prev_dist = translate_line.signed_distance(&prev_corner);
            let next_dist = translate_line.signed_distance(&next_corner);
            if Signum::of_f64(prev_dist) != Signum::of_f64(next_dist) {
                i += 1;
                continue;
            }
            let mut translate_dist = if prev_dist.abs() < next_dist.abs() {
                prev_dist
            } else {
                next_dist
            };
            if translate_dist == 0.0 {
                i += 1;
                continue;
            }
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
            let keep_before_ind = i - crossed_corners_before_count;
            let (tmp, current_lines) = splice_and_normalise(
                &lines,
                keep_before_ind,
                new_line,
                i + 1 + crossed_corners_after_count,
            );
            let mut check_ok = false;
            if tmp.lines().len() == current_lines.len() {
                let shape_to_check = tmp
                    .offset_shape(self.base.current_half_width, keep_before_ind - 1)
                    .expect("keepBeforeInd - 1 is below tileShapeCount");
                check_ok = self.base.check(board, &shape_to_check);
            }
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
        if lines.len() - start_no < 4 {
            return None;
        }
        let current_corner = lines[start_no + 1].intersection_approx(&lines[start_no + 2]);
        if self.base.current_clip_shape.is_some() && !self.base.clip_contains(&current_corner) {
            return None;
        }
        let cosinus_angle = lines[start_no + 1].cos_angle(&lines[start_no + 2]);
        if cosinus_angle > C_MAX_COS_ANGLE {
            return None;
        }
        let prev_corner = lines[start_no].intersection_approx(&lines[start_no + 1]);
        let next_corner = lines[start_no + 2].intersection_approx(&lines[start_no + 3]);
        let prev_dir = lines[start_no + 1].direction();
        let next_dir = lines[start_no + 2].direction();
        let middle_dir = prev_dir.middle_approx(&next_dir);
        let translate_line =
            TightenerBase::line_through(&current_corner, &Direction::Int(middle_dir));
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
        let mut current_lines: Vec<Line> = Vec::with_capacity(lines.len() + 1);
        current_lines.extend_from_slice(&lines[..start_no + 2]);
        current_lines.push(lines[start_no + 2]); 
        current_lines.extend_from_slice(&lines[start_no + 2..]);
        let mut translate_dist = max_translate_dist;
        let mut delta_dist = max_translate_dist;
        let side_of_nearest_point = translate_line.side_of_float_exact(&nearest_point);
        let sign = Signum::as_int_f64(max_translate_dist);
        let mut result: Option<Line> = None;
        while delta_dist.abs() > f64::from(self.base.min_translate_dist) {
            let mut check_ok = false;
            let new_line = translate_line.translate(-translate_dist);
            let new_line_side_of_nearest_point = new_line.side_of_float_exact(&nearest_point);
            if new_line_side_of_nearest_point == side_of_nearest_point
                || new_line_side_of_nearest_point == Side::Collinear
            {
                current_lines[start_no + 2] = new_line;
                let tmp = new_polyline_in_place(&mut current_lines);
                if tmp.lines().len() == current_lines.len() {
                    let shape_to_check = tmp
                        .offset_shape(self.base.current_half_width, start_no + 1)
                        .expect("startNo + 1 is below tileShapeCount");
                    check_ok = self.base.check(board, &shape_to_check);
                }
                delta_dist /= 2.0;
                if check_ok {
                    result = Some(current_lines[start_no + 2]);
                    if translate_dist == max_translate_dist {
                        break;
                    }
                    translate_dist += delta_dist;
                } else {
                    translate_dist -= delta_dist;
                }
            } else {
                let shorten_value = f64::from(sign) * 0.5;
                max_translate_dist -= shorten_value;
                translate_dist -= shorten_value;
                delta_dist -= shorten_value;
            }
        }
        result?;
        if board.changed_area.is_some() {
            let layer = self.base.current_layer;
            let new_prev_corner =
                current_lines[start_no].intersection_approx(&current_lines[start_no + 1]);
            let new_next_corner =
                current_lines[start_no + 3].intersection_approx(&current_lines[start_no + 4]);
            board.join_changed_area(&new_prev_corner, layer);
            board.join_changed_area(&new_next_corner, layer);
        }
        result
    }

                                        pub(crate) fn reposition_line(
        &mut self,
        board: &mut Board,
        lines: &[Line],
        start_no: usize,
    ) -> Option<Line> {
        if lines.len() - start_no < 5 {
            return None;
        }
        if self.base.current_clip_shape.is_some() {
            for i in 1..3 {
                let current_corner =
                    lines[start_no + i].intersection_approx(&lines[start_no + i + 1]);
                if !self.base.clip_contains(&current_corner) {
                    return None;
                }
            }
        }
        let translate_line = lines[start_no + 2];
        let mut prev_corner = lines[start_no].intersection_approx(&lines[start_no + 1]);
        let mut next_corner = lines[start_no + 3].intersection_approx(&lines[start_no + 4]);
        let mut prev_dist = translate_line.signed_distance(&prev_corner);
        let mut corners_skipped_before: usize = 0;
        let mut corners_skipped_after: usize = 0;
        const EPSILON: f64 = 0.001;
        while prev_dist.abs() < EPSILON {
            corners_skipped_before += 1;
            let Some(current_no) = start_no.checked_sub(corners_skipped_before) else {
                return None;
            };
            prev_corner = lines[current_no].intersection_approx(&lines[current_no + 1]);
            prev_dist = translate_line.signed_distance(&prev_corner);
        }
        let mut next_dist = translate_line.signed_distance(&next_corner);
        while next_dist.abs() < EPSILON {
            corners_skipped_after += 1;
            let current_no = start_no + 3 + corners_skipped_after;
            if current_no + 2 >= lines.len() {
                return None;
            }
            next_corner = lines[current_no].intersection_approx(&lines[current_no + 1]);
            next_dist = translate_line.signed_distance(&next_corner);
        }
        if Signum::of_f64(prev_dist) != Signum::of_f64(next_dist) {
            return None;
        }
        let (nearest_point, mut max_translate_dist) = if prev_dist.abs() < next_dist.abs() {
            (prev_corner, prev_dist)
        } else {
            (next_corner, next_dist)
        };
        let mut current_lines: Vec<Line> = lines.to_vec();
        let mut translate_dist = max_translate_dist;
        let mut delta_dist = max_translate_dist;
        let side_of_nearest_point = translate_line.side_of_float_exact(&nearest_point);
        let sign = Signum::as_int_f64(max_translate_dist);
        let mut result: Option<Line> = None;
        let mut first_time = true;
        while first_time || delta_dist.abs() > f64::from(self.base.min_translate_dist) {
            let mut check_ok = false;
            let mut new_line = translate_line.translate(-translate_dist);
            if first_time && translate_dist.abs() < 1.0 {
                if new_line.equals_geometric(&translate_line) {
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
            let new_line_side_of_nearest_point = new_line.side_of_float_exact(&nearest_point);
            if new_line_side_of_nearest_point == side_of_nearest_point
                || new_line_side_of_nearest_point == Side::Collinear
            {
                first_time = false;
                current_lines[start_no + 2] = new_line;
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
                let tmp = new_polyline_in_place(&mut current_lines);
                if tmp.lines().len() == current_lines.len() {
                    let shape_to_check = tmp
                        .offset_shape(self.base.current_half_width, start_no + 1)
                        .expect("startNo + 1 is below tileShapeCount");
                    check_ok = self.base.check(board, &shape_to_check);
                }
                delta_dist /= 2.0;
                if check_ok {
                    result = Some(current_lines[start_no + 2]);
                    if translate_dist == max_translate_dist {
                        break;
                    }
                    translate_dist += delta_dist;
                } else {
                    translate_dist -= delta_dist;
                }
            } else {
                let shorten_value = f64::from(sign) * 0.5;
                max_translate_dist -= shorten_value;
                translate_dist -= shorten_value;
                delta_dist -= shorten_value;
            }
        }
        result?;
        if board.changed_area.is_some() {
            let layer = self.base.current_layer;
            let new_prev_corner =
                current_lines[start_no].intersection_approx(&current_lines[start_no + 1]);
            let new_next_corner =
                current_lines[start_no + 3].intersection_approx(&current_lines[start_no + 4]);
            board.join_changed_area(&new_prev_corner, layer);
            board.join_changed_area(&new_next_corner, layer);
        }
        result
    }

        fn skip_lines(&mut self, board: &mut Board, polyline: &Polyline) -> Option<Polyline> {
        let mut i: usize = 1;
        while i + 3 < polyline.lines().len() {
            for j in 0..=1 {
                let (mut current_line, corner1, corner2) = if j == 0 {
                    (
                        polyline.lines()[i + 2],
                        polyline.corner_approx(i).expect("i is below cornerCount"),
                        polyline
                            .corner_approx(i - 1)
                            .expect("i - 1 is below cornerCount"),
                    )
                } else {
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
                let in_clip_shape = self.base.current_clip_shape.is_none()
                    || (self.base.clip_contains(&corner1) && self.base.clip_contains(&corner2));
                if !in_clip_shape {
                    continue;
                }
                let mut side1 = current_line.side_of_float_exact(&corner1);
                let mut side2 = current_line.side_of_float_exact(&corner2);
                if side1 != side2 {
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
                if i + 4 >= polyline.lines().len() {
                    break;
                }
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
                if j == 0 {
                    current_line = polyline.lines()[i + 3];
                    side1 = current_line.side_of_float_exact(&corner1);
                    side2 = current_line.side_of_float_exact(&corner2);
                } else {
                    side1 = current_line.side_of_float_exact(&corner3);
                }
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
        None
    }

            pub(crate) fn smoothen_start_corner_at_trace(
        &mut self,
        board: &mut Board,
        trace: ItemId,
    ) -> Option<Polyline> {
        let trace_polyline = trace_polyline_of(board, trace)?;
        let current_end_corner = trace_polyline.corner(0)?;
        if self.base.clip_is_outside(&current_end_corner) {
            return None;
        }
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
        let line_direction = trace_polyline.lines()[start_line_no].direction();
        let prev_line_direction = trace_polyline.lines()[start_line_no + 1].direction();

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

        let mut new_line_count = trace_polyline.lines().len() + 1;
        let mut diff: usize = 1;
        if skip_short_segment {
            new_line_count -= 1;
            diff -= 1;
        }
        if found.acute_angle {
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
            let mut new_lines: Vec<Line> = Vec::with_capacity(new_line_count);
            new_lines.push(other_trace_line);
            new_lines.push(add_line);
            new_lines.extend_from_slice(&trace_polyline.lines()[2 - diff..]);
            new_lines.truncate(new_line_count);
            return Some(new_polyline(new_lines));
        } else if found.bend {
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
        None
    }

                        pub(crate) fn smoothen_end_corner_at_trace(
        &mut self,
        board: &mut Board,
        trace: ItemId,
    ) -> Option<Polyline> {
        let trace_polyline = trace_polyline_of(board, trace)?;
        let current_end_corner = trace_polyline.last_corner()?;
        if self.base.clip_is_outside(&current_end_corner) {
            return None;
        }
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
        let line_direction = trace_polyline.lines()[end_line_no].direction().opposite();
        let prev_line_direction = trace_polyline.lines()[end_line_no].direction().opposite();

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

        let mut new_line_count = line_count + 1;
        let mut diff: usize = 0;
        if skip_short_segment {
            new_line_count -= 1;
            diff += 1;
        }
        if found.acute_angle {
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
    let tmp = new_polyline_in_place(&mut current_lines);
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
        assert!(
            !current_lines[1].is_same_object(&reversed),
            "`:368` must normalise the caller's array in place (quirk #188)"
        );
        assert_eq!(current_lines[1], lines[1]);
        assert_eq!(current_lines.as_slice(), tmp.lines());
        for i in (0..current_lines.len()).filter(|i| *i != 1) {
            assert!(current_lines[i].is_same_object(&lines[i]), "index {i}");
        }
    }
}
