//! Port of `board/optimize/TraceTightener45.java` (674 lines): the 45-degree regime.

use fr_board::prelude::*;
use fr_geometry::limits::SQRT2;
use fr_geometry::{
    Direction, FloatPoint, IntDirection, IntPoint, Line, Point, Polyline, Side, Signum, Vector,
    java_max, java_min,
};

use super::base::{TightenerBase, new_polyline, new_polyline_in_place};

/// `class TraceTightener45 extends TraceTightener` (TraceTightener45.java:21).
///
/// Like [`TraceTightener90`](super::TraceTightener90) it adds no state — the constructor
/// (`:24-32`) is a bare `super(...)` call.
pub struct TraceTightener45<'a> {
    pub(crate) base: TightenerBase<'a>,
}

impl<'a> TraceTightener45<'a> {
    /// Port of the constructor `TraceTightener45(RoutingBoard, int[], Stoppable, int, Point, int)`
    /// (TraceTightener45.java:24-32).
    pub(crate) fn new(base: TightenerBase<'a>) -> TraceTightener45<'a> {
        TraceTightener45 { base }
    }

    /// Port of `getAngleRestriction()` (TraceTightener45.java:47-49) — a package-private accessor
    /// with no caller anywhere in the Java tree, kept because the class declares it.
    // pub seam: none, in Java or here — deliberate, and the doc above says why.
    pub fn get_angle_restriction(&self) -> AngleRestriction {
        AngleRestriction::FortyFiveDegree
    }

    /// Port of `pullTight(Polyline)` (TraceTightener45.java:35-45). See
    /// [`TraceTightener90::pull_tight`](super::TraceTightener90::pull_tight) for why `changed`
    /// is Java's reference comparison.
    pub(crate) fn pull_tight(
        &mut self,
        board: &mut Board,
        polyline: &Polyline,
    ) -> Option<Polyline> {
        // :36. `ever_changed` is seeded from this arm, not from the loop — see
        // [`TraceTightener90::pull_tight`](super::TraceTightener90::pull_tight) for why the
        // `avoidAcidTraps` result counts as a change in its own right. Dead today (quirk #182).
        let (mut new_result, mut ever_changed) = match self.base.avoid_acid_traps(polyline) {
            Some(replacement) => (replacement, true),
            None => (polyline.clone(), false),
        };
        // :37-38.
        let mut changed = true;
        while changed && !self.base.is_stop_requested() {
            let mut current = new_result;
            let mut any = false;
            // :40.
            if let Some(tmp1) = self.reduce_corners(board, &current) {
                current = tmp1;
                any = true;
            }
            // :41.
            if let Some(tmp2) = self.smoothen_corners(board, &current) {
                current = tmp2;
                any = true;
            }
            // :42.
            if let Some(tmp3) = self.base.reposition_lines(board, &current) {
                current = tmp3;
                any = true;
            }
            new_result = current;
            changed = any;
            ever_changed |= any;
        }
        // :44.
        if ever_changed { Some(new_result) } else { None }
    }

    /// Port of the private `reduceCorners(Polyline)` (TraceTightener45.java:52-221): "tries to
    /// reduce the amount of corners of polyline. Return polyline, if nothing was changed."
    ///
    /// Java's `newCorners` array is sized `polyline.lines.length - 3` (`:75`) and written at
    /// `:202`, and it cannot overflow: `cornerIndex` runs over `[3, L - 2]` and every pass
    /// advances it by 1 or 2, so there are at most `L - 4` passes, each writing at most one slot
    /// from index 1 — the largest index written is `L - 4`, the last valid one.
    fn reduce_corners(&mut self, board: &mut Board, polyline: &Polyline) -> Option<Polyline> {
        let line_count = polyline.lines().len();
        // :53-55.
        if line_count <= 4 {
            return None;
        }
        // :56-62: every corner the algorithm moves must be an `IntPoint`.
        let mut current_corner: Vec<Point> = Vec::with_capacity(4);
        for i in 0..4 {
            let corner = polyline.corner(i).expect("i is below cornerCount");
            if !matches!(corner, Point::Int(_)) {
                return None;
            }
            current_corner.push(corner);
        }
        // :63-71.
        let mut current_corner_in_clip_shape = [false; 4];
        for i in 0..4 {
            current_corner_in_clip_shape[i] = !self.base.clip_is_outside(&current_corner[i]);
        }

        // :73-79.
        let mut polyline_changed = false;
        let mut new_corner_count: usize = 1;
        let mut new_corners: Vec<Point> = vec![current_corner[0].clone(); line_count - 3];
        let mut new_corner: Option<Point> = None;
        let mut corner_index: usize = 3;
        // :80.
        while corner_index < line_count - 1 {
            // :81-84.
            current_corner[3] = polyline
                .corner(corner_index)
                .expect("cornerIndex is below cornerCount");
            if !matches!(current_corner[3], Point::Int(_)) {
                return None;
            }
            // :85-99: corners in the middle of a line can be skipped.
            if current_corner[1] == current_corner[2]
                || (corner_index < line_count - 2
                    && current_corner[3].side_of(&current_corner[1], &current_corner[2])
                        == Side::Collinear)
            {
                corner_index += 1;
                current_corner[2] = current_corner[3].clone();
                // Java bug: `TraceTightener45.reduceCorners` copies `currentCornerInClipShape[3]` onto slot 2 at `:91` while `currentCorner[3]` was replaced at `:81` and the flag is only recomputed at `:100-101`, so the flag belongs to the *previous* corner. See docs/java-quirks.md #184.
                // obligation: quirk #184's own condition is still unexercised. Plan 7 Task 8
                // made the *clip-shape path* reachable —
                // `RoutingBoardExt::remove_items_and_pull_tight` with
                // `0 < tidyWidth < i32::MAX` is the only caller in either language that
                // hands `TraceTightener` a real octagon, and
                // `batch_autorouter.rs`'s
                // `remove_items_and_pull_tight_hands_the_tightener_a_live_clip_octagon`
                // pins it — but instrumenting this statement showed it reached **only**
                // with `current_clip_shape == None`, where stale and fresh are trivially
                // equal in Java too. Closing it needs a board whose clip octagon cuts
                // through a corner the `:85-99` skip block actually skips (a duplicate
                // corner, or a collinear middle corner) so that the stale flag and the
                // fresh one differ; then the two translate attempts at `:103-105` and
                // `:148-151` can be shown to take different branches.
                current_corner_in_clip_shape[2] = current_corner_in_clip_shape[3];
                if corner_index < line_count - 1 {
                    current_corner[3] = polyline
                        .corner(corner_index)
                        .expect("cornerIndex is below cornerCount");
                    if !matches!(current_corner[3], Point::Int(_)) {
                        return None;
                    }
                }
                polyline_changed = true;
            }
            // :100-101.
            current_corner_in_clip_shape[3] = !self.base.clip_is_outside(&current_corner[3]);
            let mut corner_removed = false;
            // :103-147: translate the line from corner 2 to corner 1 onto corner 3.
            if current_corner_in_clip_shape[1]
                && current_corner_in_clip_shape[2]
                && current_corner_in_clip_shape[3]
            {
                let delta = current_corner[3].difference_by(&current_corner[2]);
                let candidate = current_corner[1].translate_by(&delta);
                new_corner = Some(candidate.clone());
                if current_corner[3] == current_corner[2] {
                    // :109-111: just remove the multiple corner.
                    corner_removed = true;
                } else if candidate.side_of(&current_corner[0], &current_corner[1])
                    == Side::Collinear
                {
                    corner_removed = self.two_step_check(
                        board,
                        &candidate,
                        &current_corner[1],
                        &current_corner[3],
                    );
                }
            }
            // :148-190: the first try failed — translate the line from corner 2 to corner 1 onto
            // corner 0 instead.
            if !corner_removed
                && current_corner_in_clip_shape[0]
                && current_corner_in_clip_shape[1]
                && current_corner_in_clip_shape[2]
            {
                let delta = current_corner[0].difference_by(&current_corner[1]);
                let candidate = current_corner[2].translate_by(&delta);
                new_corner = Some(candidate.clone());
                if current_corner[0] == current_corner[1] {
                    // :156-158.
                    corner_removed = true;
                } else if candidate.side_of(&current_corner[2], &current_corner[3])
                    == Side::Collinear
                {
                    // :160-188. Unlike the first arm this one has **no**
                    // `currentCheckPoints[0].equals(currentCheckPoints[1])` shortcut at `:171`,
                    // so a zero-length second check polyline falls through to the
                    // `lines.length == 3` test and its `else` at `:182-184`.
                    corner_removed = self.two_step_check_without_shortcut(
                        board,
                        &candidate,
                        &current_corner[0],
                        &current_corner[2],
                    );
                }
            }
            // :191-208.
            if corner_removed {
                polyline_changed = true;
                let moved = new_corner.clone().expect(
                    "cornerRemoved is only set after newCorner was written (TraceTightener45.java:108,155)",
                );
                current_corner[1] = moved.clone();
                current_corner_in_clip_shape[1] = !self.base.clip_is_outside(&current_corner[1]);
                if board.changed_area.is_some() {
                    let layer = self.base.current_layer;
                    // :197-199: Java joins `newCorner` and `currentCorner[1]`, which `:193` has
                    // just made the same point, and then `currentCorner[2]`.
                    board.join_changed_area(&moved.to_float(), layer);
                    let corner1 = current_corner[1].to_float();
                    board.join_changed_area(&corner1, layer);
                    let corner2 = current_corner[2].to_float();
                    board.join_changed_area(&corner2, layer);
                }
            } else {
                new_corners[new_corner_count] = current_corner[1].clone();
                new_corner_count += 1;
                current_corner[0] = current_corner[1].clone();
                current_corner[1] = current_corner[2].clone();
                current_corner_in_clip_shape[0] = current_corner_in_clip_shape[1];
                current_corner_in_clip_shape[1] = current_corner_in_clip_shape[2];
            }
            // :209-211.
            current_corner[2] = current_corner[3].clone();
            current_corner_in_clip_shape[2] = current_corner_in_clip_shape[3];
            corner_index += 1;
        }
        // :213-215.
        if !polyline_changed {
            return None;
        }
        // :216-220.
        let mut adjusted_corners: Vec<Point> = new_corners[..new_corner_count].to_vec();
        adjusted_corners.push(current_corner[1].clone());
        adjusted_corners.push(current_corner[2].clone());
        Some(Polyline::from_points(&adjusted_corners))
    }

    /// `reduceCorners`' first arm, `:113-145`: check the segment `newCorner -> currentCorner[1]`,
    /// then the segment `newCorner -> currentCorner[3]`.
    fn two_step_check(
        &mut self,
        board: &mut Board,
        new_corner: &Point,
        first: &Point,
        second: &Point,
    ) -> bool {
        // :113-116.
        let check_polyline = Polyline::from_points(&[new_corner.clone(), first.clone()]);
        if check_polyline.lines().len() != 3 {
            // :143-145.
            return true;
        }
        // :117-123.
        let shape_to_check = check_polyline
            .offset_shape(self.base.current_half_width, 0)
            .expect("a three-line polyline has one offset shape");
        if !self.base.check(board, &shape_to_check) {
            return false;
        }
        // :124-126.
        if new_corner == second {
            return true;
        }
        // :128-141.
        let check_polyline = Polyline::from_points(&[new_corner.clone(), second.clone()]);
        if check_polyline.lines().len() != 3 {
            return true;
        }
        let shape_to_check = check_polyline
            .offset_shape(self.base.current_half_width, 0)
            .expect("a three-line polyline has one offset shape");
        self.base.check(board, &shape_to_check)
    }

    /// `reduceCorners`' second arm, `:160-188` — the same two checks without the
    /// `currentCheckPoints[0].equals(currentCheckPoints[1])` shortcut the first arm has.
    fn two_step_check_without_shortcut(
        &mut self,
        board: &mut Board,
        new_corner: &Point,
        first: &Point,
        second: &Point,
    ) -> bool {
        // :162-165.
        let check_polyline = Polyline::from_points(&[new_corner.clone(), first.clone()]);
        if check_polyline.lines().len() != 3 {
            // :186-188.
            return true;
        }
        // :164-170.
        let shape_to_check = check_polyline
            .offset_shape(self.base.current_half_width, 0)
            .expect("a three-line polyline has one offset shape");
        if !self.base.check(board, &shape_to_check) {
            return false;
        }
        // :171-184.
        let check_polyline = Polyline::from_points(&[new_corner.clone(), second.clone()]);
        if check_polyline.lines().len() != 3 {
            return true;
        }
        let shape_to_check = check_polyline
            .offset_shape(self.base.current_half_width, 0)
            .expect("a three-line polyline has one offset shape");
        self.base.check(board, &shape_to_check)
    }

    /// Port of the private `smoothenCorners(Polyline)` (TraceTightener45.java:227-267):
    /// "smoothens the 90 degree corners of polyline to 45 degree by cutting off the 90 degree
    /// corner. The cutting off is so small that no check is needed."
    fn smoothen_corners(&mut self, board: &mut Board, polyline: &Polyline) -> Option<Polyline> {
        // :228-229.
        let mut result: Option<Polyline> = None;
        let mut polyline_changed = true;
        while polyline_changed {
            {
                let current = result.as_ref().unwrap_or(polyline);
                // :231-233.
                if current.lines().len() < 4 {
                    return result;
                }
            }
            polyline_changed = false;
            // :235-236.
            let mut lines: Vec<Line> = result.as_ref().unwrap_or(polyline).lines().to_vec();
            // :238.
            let mut i: usize = 1;
            while i + 2 < lines.len() {
                let d1 = lines[i].direction();
                let d2 = lines[i + 1].direction();
                // :241-243: a 90 degree or sharper angle.
                if d1.is_multiple_of_45_degree()
                    && d2.is_multiple_of_45_degree()
                    && d1.projection(&d2) != Signum::Positive
                {
                    // :245-249.
                    let new_line = match self.smoothen_corner(board, &lines, i) {
                        Some(line) => Some(line),
                        // The greedy smoothening could not change the polyline.
                        None => self.smoothen_sharp_corner(board, &lines, i),
                    };
                    if let Some(new_line) = new_line {
                        // :250-259.
                        polyline_changed = true;
                        lines.insert(i + 1, new_line);
                        i += 1;
                    }
                }
                i += 1;
            }
            // :262-264.
            if polyline_changed {
                result = Some(new_polyline(lines));
            }
        }
        // :266.
        result
    }

    /// Port of the private `smoothenSharpCorner(Line[], int)` (TraceTightener45.java:275-310):
    /// "adds a line at `no` to smoothen a 90 degree corner between line1 and line2 to 45 degree.
    /// The distance of the new line to the corner will be so small that no clearance check is
    /// necessary."
    fn smoothen_sharp_corner(
        &mut self,
        board: &mut Board,
        lines: &[Line],
        no: usize,
    ) -> Option<Line> {
        // :276-285.
        let current_corner = lines[no].intersection_approx(&lines[no + 1]);
        if current_corner.x != f64::from(current_corner.x as i32) {
            // The intersection of two diagonal lines is not integer.
            if let Some(result) = self.smoothen_non_integer_corner(lines, no) {
                return Some(result);
            }
        }
        // :286-292.
        let prev_corner = lines[no].intersection_approx(&lines[no - 1]);
        let next_corner = lines[no + 1].intersection_approx(&lines[no + 2]);
        let prev_dir = lines[no].direction();
        let next_dir = lines[no + 1].direction();
        let new_line_dir = Direction::from_vector(
            &Vector::from(prev_dir.get_vector()).add(&Vector::from(next_dir.get_vector())),
        );
        let translate_line = TightenerBase::line_through(&current_corner, &new_line_dir);
        // :293-297.
        let mut translate_dist = (SQRT2 - 1.0) * f64::from(self.base.current_half_width);
        let prev_dist = translate_line.signed_distance(&prev_corner).abs();
        let next_dist = translate_line.signed_distance(&next_corner).abs();
        translate_dist = java_min(translate_dist, prev_dist);
        translate_dist = java_min(translate_dist, next_dist);
        // :298-300.
        if translate_dist < 0.99 {
            return None;
        }
        // :301-304.
        translate_dist = java_max(translate_dist - 1.0, 1.0);
        if translate_line.side_of_float_exact(&next_corner) == Side::OnTheLeft {
            translate_dist = -translate_dist;
        }
        // :305-309.
        let result = translate_line.translate(translate_dist);
        if board.changed_area.is_some() {
            let layer = self.base.current_layer;
            board.join_changed_area(&current_corner, layer);
        }
        Some(result)
    }

    /// Port of the private `smoothenNonIntegerCorner(Line[], int)` (TraceTightener45.java:
    /// 316-368): "smoothens with a short axis parallel line to remove a non integer corner of two
    /// intersecting diagonal lines. Returns null, if that is not possible."
    fn smoothen_non_integer_corner(&mut self, lines: &[Line], no: usize) -> Option<Line> {
        // :317-324.
        let prev_line = lines[no];
        let next_line = lines[no + 1];
        if prev_line.is_equal_or_opposite(&next_line) {
            return None;
        }
        if !(prev_line.is_diagonal() && next_line.is_diagonal()) {
            return None;
        }
        // :325-327.
        let current_corner = prev_line.intersection_approx(&next_line);
        let prev_corner = prev_line.intersection_approx(&lines[no - 1]);
        let next_corner = next_line.intersection_approx(&lines[no + 2]);
        // :328-348.
        let mut new_x = 0_i32;
        let mut new_y = 0_i32;
        let mut new_line_is_vertical = false;
        let mut new_line_is_horizontal = false;
        if prev_corner.x > current_corner.x && next_corner.x > current_corner.x {
            new_x = current_corner.x.ceil() as i32;
            new_y = current_corner.y.ceil() as i32;
            new_line_is_vertical = true;
        } else if prev_corner.x < current_corner.x && next_corner.x < current_corner.x {
            new_x = current_corner.x.floor() as i32;
            new_y = current_corner.y.floor() as i32;
            new_line_is_vertical = true;
        } else if prev_corner.y > current_corner.y && next_corner.y > current_corner.y {
            new_x = current_corner.x.ceil() as i32;
            new_y = current_corner.y.ceil() as i32;
            new_line_is_horizontal = true;
        } else if prev_corner.y < current_corner.y && next_corner.y < current_corner.y {
            new_x = current_corner.x.floor() as i32;
            new_y = current_corner.y.floor() as i32;
            new_line_is_horizontal = true;
        }
        // :349-364.
        let new_line_dir = if new_line_is_vertical {
            if prev_corner.y < next_corner.y {
                Direction::Int(IntDirection::UP)
            } else {
                Direction::Int(IntDirection::DOWN)
            }
        } else if new_line_is_horizontal {
            if prev_corner.x < next_corner.x {
                Direction::Int(IntDirection::RIGHT)
            } else {
                Direction::Int(IntDirection::LEFT)
            }
        } else {
            return None;
        };
        // :366-367.
        Some(
            Line::from_direction_any(IntPoint::new(new_x, new_y), &new_line_dir)
                .expect("UP/DOWN/LEFT/RIGHT are IntDirections"),
        )
    }

    /// Port of the private `smoothenCorner(Line[], int)` (TraceTightener45.java:376-459): "adds a
    /// line at `no` to smoothen a 90 degree corner between line1 and line2 to 45 degree. The
    /// distance of the new line to the corner will be so big that a clearance check is
    /// necessary."
    fn smoothen_corner(&mut self, board: &mut Board, lines: &[Line], no: usize) -> Option<Line> {
        // :377-383.
        let prev_corner = lines[no].intersection_approx(&lines[no - 1]);
        let current_corner = lines[no].intersection_approx(&lines[no + 1]);
        let next_corner = lines[no + 1].intersection_approx(&lines[no + 2]);
        let prev_dir = lines[no].direction();
        let next_dir = lines[no + 1].direction();
        let new_line_dir = Direction::from_vector(
            &Vector::from(prev_dir.get_vector()).add(&Vector::from(next_dir.get_vector())),
        );
        // :384-389.
        let translate_line = TightenerBase::line_through(&current_corner, &new_line_dir);
        let prev_dist = translate_line.signed_distance(&prev_corner).abs();
        let next_dist = translate_line.signed_distance(&next_corner).abs();
        if prev_dist == 0.0 || next_dist == 0.0 {
            return None;
        }
        // :390-398.
        let (mut max_translate_dist, nearest_corner) = if prev_dist <= next_dist {
            (prev_dist, prev_corner)
        } else {
            (next_dist, next_corner)
        };
        // :399-405.
        if max_translate_dist < 1.0 {
            return None;
        }
        max_translate_dist = java_max(max_translate_dist - 1.0, 1.0);
        if translate_line.side_of_float_exact(&next_corner) == Side::OnTheLeft {
            max_translate_dist = -max_translate_dist;
        }
        // :406-413.
        let check_line_0 = lines[no];
        let check_line_2 = lines[no + 1];
        let mut translate_dist = max_translate_dist;
        let mut delta_dist = max_translate_dist;
        let side_of_nearest_corner = translate_line.side_of_float_exact(&nearest_corner);
        let sign = Signum::as_int_f64(max_translate_dist);
        let mut result: Option<Line> = None;
        // :414.
        while delta_dist.abs() > f64::from(self.base.min_translate_dist) {
            let mut check_ok = false;
            // :416-419.
            let new_line = translate_line.translate(translate_dist);
            let new_line_side_of_nearest_corner = new_line.side_of_float_exact(&nearest_corner);
            if new_line_side_of_nearest_corner == side_of_nearest_corner
                || new_line_side_of_nearest_corner == Side::Collinear
            {
                // :420-432. `new Polyline(checkLines)` normalises **checkLines itself**, and
                // `:435` reads element 1 back out of it — see `new_polyline_in_place`.
                let mut check_lines = vec![check_line_0, new_line, check_line_2];
                let tmp = new_polyline_in_place(&mut check_lines);
                let new_line = check_lines[1];
                if tmp.lines().len() == 3 {
                    let shape_to_check = tmp
                        .offset_shape(self.base.current_half_width, 0)
                        .expect("a three-line polyline has one offset shape");
                    check_ok = self.base.check(board, &shape_to_check);
                }
                // :433-443.
                delta_dist /= 2.0;
                if check_ok {
                    result = Some(new_line);
                    if translate_dist == max_translate_dist {
                        // Biggest possible change.
                        break;
                    }
                    translate_dist += delta_dist;
                } else {
                    translate_dist -= delta_dist;
                }
            } else {
                // :444-449: moved a little bit too far at the first time because of numerical
                // inaccuracy.
                let shorten_value = f64::from(sign) * 0.5;
                max_translate_dist -= shorten_value;
                translate_dist -= shorten_value;
                delta_dist -= shorten_value;
            }
        }
        // :451-457.
        if let Some(result) = result
            && board.changed_area.is_some()
        {
            let layer = self.base.current_layer;
            let new_prev_corner = check_line_0.intersection_approx(&result);
            let new_next_corner = check_line_2.intersection_approx(&result);
            board.join_changed_area(&new_prev_corner, layer);
            board.join_changed_area(&new_next_corner, layer);
            board.join_changed_area(&current_corner, layer);
        }
        // :458.
        result
    }

    /// Port of `smoothenStartCornerAtTrace(PolylineTrace)` (TraceTightener45.java:462-565).
    pub(crate) fn smoothen_start_corner_at_trace(
        &mut self,
        board: &mut Board,
        trace: ItemId,
    ) -> Option<Polyline> {
        let trace_polyline = trace_polyline_of(board, trace)?;
        // :469-473.
        let current_end_corner = trace_polyline.corner(0)?;
        if self.base.clip_is_outside(&current_end_corner) {
            return None;
        }
        // :475-478.
        let current_prev_end_corner = trace_polyline.corner(1)?;
        let line_direction = trace_polyline.lines()[1].direction();
        let prev_line_direction = trace_polyline.lines()[2].direction();

        // :480-520.
        let contacts = board.trace_start_contacts(trace);
        let found = super::scan_contacts(
            board,
            &contacts,
            &trace_polyline,
            &current_end_corner,
            &current_prev_end_corner,
            &line_direction,
            &prev_line_direction,
            true,
            false,
        )?;

        if found.acute_angle {
            // :522-549.
            let other_trace_line = found.other_trace_line.expect("acuteAngle implies a match");
            let new_line_dir = if found.prev_corner_side == Some(Side::OnTheLeft) {
                other_trace_line.direction().turn_45_degree(2)
            } else {
                other_trace_line.direction().turn_45_degree(6)
            };
            let add_line = self.acute_add_line(
                &current_end_corner,
                &current_prev_end_corner,
                &new_line_dir,
                &found
                    .other_trace_corner_approx
                    .expect("acuteAngle implies a match"),
            )?;
            // :543-548.
            let mut new_lines: Vec<Line> = Vec::with_capacity(trace_polyline.lines().len() + 1);
            new_lines.push(other_trace_line);
            new_lines.push(add_line);
            new_lines.extend_from_slice(&trace_polyline.lines()[1..]);
            return Some(new_polyline(new_lines));
        } else if found.bend {
            // :550-563.
            let other_trace_line = found.other_trace_line.expect("bend implies a match");
            let other_prev_trace_line = found.other_prev_trace_line.expect("bend implies a match");
            let mut check_line_arr: Vec<Line> =
                Vec::with_capacity(trace_polyline.lines().len() + 1);
            check_line_arr.push(other_prev_trace_line);
            check_line_arr.push(other_trace_line);
            check_line_arr.extend_from_slice(&trace_polyline.lines()[1..]);
            let new_line = self.base.reposition_line(board, &check_line_arr, 2)?;
            let mut new_lines: Vec<Line> = Vec::with_capacity(trace_polyline.lines().len());
            new_lines.push(other_trace_line);
            new_lines.push(new_line);
            new_lines.extend_from_slice(&trace_polyline.lines()[2..]);
            return Some(new_polyline(new_lines));
        }
        // :564.
        None
    }

    /// Port of `smoothenEndCornerAtTrace(PolylineTrace)` (TraceTightener45.java:568-673).
    pub(crate) fn smoothen_end_corner_at_trace(
        &mut self,
        board: &mut Board,
        trace: ItemId,
    ) -> Option<Polyline> {
        let trace_polyline = trace_polyline_of(board, trace)?;
        // :575-579.
        let current_end_corner = trace_polyline.last_corner()?;
        if self.base.clip_is_outside(&current_end_corner) {
            return None;
        }
        // :581-586.
        let line_count = trace_polyline.lines().len();
        let current_prev_end_corner = trace_polyline.corner(trace_polyline.corner_count() - 2)?;
        let line_direction = trace_polyline.lines()[line_count - 2]
            .direction()
            .opposite();
        let prev_line_direction = trace_polyline.lines()[line_count - 3]
            .direction()
            .opposite();

        // :588-628.
        let contacts = board.trace_end_contacts(trace);
        let found = super::scan_contacts(
            board,
            &contacts,
            &trace_polyline,
            &current_end_corner,
            &current_prev_end_corner,
            &line_direction,
            &prev_line_direction,
            true,
            false,
        )?;

        if found.acute_angle {
            // :630-657.
            let other_trace_line = found.other_trace_line.expect("acuteAngle implies a match");
            let new_line_dir = if found.prev_corner_side == Some(Side::OnTheLeft) {
                other_trace_line.direction().turn_45_degree(6)
            } else {
                other_trace_line.direction().turn_45_degree(2)
            };
            let add_line = self.acute_add_line(
                &current_end_corner,
                &current_prev_end_corner,
                &new_line_dir,
                &found
                    .other_trace_corner_approx
                    .expect("acuteAngle implies a match"),
            )?;
            // :651-656.
            let mut new_lines: Vec<Line> = Vec::with_capacity(line_count + 1);
            new_lines.extend_from_slice(&trace_polyline.lines()[..line_count - 1]);
            new_lines.push(add_line);
            new_lines.push(other_trace_line);
            return Some(new_polyline(new_lines));
        } else if found.bend {
            // :658-671.
            let other_trace_line = found.other_trace_line.expect("bend implies a match");
            let other_prev_trace_line = found.other_prev_trace_line.expect("bend implies a match");
            let mut check_line_arr: Vec<Line> = Vec::with_capacity(line_count + 1);
            check_line_arr.extend_from_slice(&trace_polyline.lines()[..line_count - 1]);
            check_line_arr.push(other_trace_line);
            check_line_arr.push(other_prev_trace_line);
            let new_line = self
                .base
                .reposition_line(board, &check_line_arr, line_count - 2)?;
            let mut new_lines: Vec<Line> = Vec::with_capacity(line_count);
            new_lines.extend_from_slice(&trace_polyline.lines()[..line_count - 2]);
            new_lines.push(new_line);
            new_lines.push(other_trace_line);
            return Some(new_polyline(new_lines));
        }
        // :672.
        None
    }

    /// The shared body of `smoothenStartCornerAtTrace:522-549` and
    /// `smoothenEndCornerAtTrace:630-657` — identical apart from the `turn45Degree` factors the
    /// caller has already applied. `TraceTightenerAnyAngle:834-861` / `:960-987` repeat it
    /// verbatim, so both regimes call this one body.
    pub(crate) fn acute_add_line(
        &self,
        current_end_corner: &Point,
        current_prev_end_corner: &Point,
        new_line_dir: &IntDirection,
        other_trace_corner_approx: &FloatPoint,
    ) -> Option<Line> {
        acute_add_line(
            self.base.current_half_width,
            current_end_corner,
            current_prev_end_corner,
            new_line_dir,
            other_trace_corner_approx,
        )
    }
}

/// `TraceTightener45.smoothenStartCornerAtTrace:529-542` (and its three siblings): build the
/// 45-degree line that cuts the acute angle off, or answer `None` when `:536`'s
/// `translateDist >= 0.99` guard refuses.
pub(crate) fn acute_add_line(
    current_half_width: i32,
    current_end_corner: &Point,
    current_prev_end_corner: &Point,
    new_line_dir: &IntDirection,
    other_trace_corner_approx: &FloatPoint,
) -> Option<Line> {
    let translate_line = TightenerBase::line_through(
        &current_end_corner.to_float(),
        &Direction::Int(*new_line_dir),
    );
    let mut translate_dist = (SQRT2 - 1.0) * f64::from(current_half_width);
    let prev_corner_dist = translate_line
        .signed_distance(&current_prev_end_corner.to_float())
        .abs();
    let other_dist = translate_line
        .signed_distance(other_trace_corner_approx)
        .abs();
    translate_dist = java_min(translate_dist, prev_corner_dist);
    translate_dist = java_min(translate_dist, other_dist);
    if translate_dist < 0.99 {
        return None;
    }
    translate_dist = java_max(translate_dist - 1.0, 1.0);
    if translate_line.side_of(current_prev_end_corner) == Side::OnTheLeft {
        translate_dist = -translate_dist;
    }
    Some(translate_line.translate(translate_dist))
}

/// `trace.polyline()`, plus the two guards `smoothenEndCornersAtTrace2:495-497` applies before
/// either override is called.
pub(crate) fn trace_polyline_of(board: &Board, trace: ItemId) -> Option<Polyline> {
    match board.items.get(&trace) {
        Some(Item::Trace(t)) => Some(t.polyline().clone()),
        _ => None,
    }
}
