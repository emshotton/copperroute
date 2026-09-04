use fr_board::prelude::*;
use fr_geometry::{Line, Polyline};

use super::base::{TightenerBase, new_polyline};

pub struct TraceTightener90<'a> {
    pub(crate) base: TightenerBase<'a>,
}

impl<'a> TraceTightener90<'a> {
            pub(crate) fn new(base: TightenerBase<'a>) -> TraceTightener90<'a> {
        TraceTightener90 { base }
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
            if let Some(tmp1) = self.try_skip_second_corner(board, &current) {
                current = tmp1;
                any = true;
            }
            if let Some(tmp2) = self.try_skip_corners(board, &current) {
                current = tmp2;
                any = true;
            }
            if let Some(tmp3) = self.base.reposition_lines(board, &current) {
                current = tmp3;
                any = true;
            }
            new_result = current;
            changed = any;
            ever_changed |= any;
        }
        if ever_changed { Some(new_result) } else { None }
    }

                            fn try_skip_second_corner(
        &mut self,
        board: &mut Board,
        polyline: &Polyline,
    ) -> Option<Polyline> {
        if polyline.lines().len() < 5 {
            return None;
        }
        let lines = polyline.lines();
        let check_polyline = new_polyline(vec![lines[1], lines[0], lines[3], lines[4]]);
        if check_polyline.lines().len() != 4 {
            return None;
        }
        if self.base.current_clip_shape.is_some()
            && !self.base.clip_contains(
                &check_polyline
                    .corner_approx(1)
                    .expect("a four-line polyline has three corners"),
            )
        {
            return None;
        }
        for i in 0..2 {
            let shape_to_check = check_polyline
                .offset_shape(self.base.current_half_width, i)
                .expect("a four-line polyline has two offset shapes");
            if !self.base.check(board, &shape_to_check) {
                return None;
            }
        }
        let mut new_lines: Vec<Line> = Vec::with_capacity(lines.len() - 1);
        new_lines.push(lines[1]);
        new_lines.push(lines[0]);
        new_lines.extend_from_slice(&lines[3..]);
        Some(new_polyline(new_lines))
    }

                                            fn try_skip_corners(&mut self, board: &mut Board, polyline: &Polyline) -> Option<Polyline> {
        let lines = polyline.lines();
        let mut new_lines: Vec<Line> = vec![lines[0]; lines.len()];
        new_lines[0] = lines[0];
        new_lines[1] = lines[1];
        let mut new_line_index: usize = 1;
        let mut polyline_changed = false;
        let mut second_last_corner_skipped = false;
        let mut i: usize = 5;
        while i <= lines.len() {
            let mut skip_lines = false;
            let in_clip_shape = self.base.clip_contains(
                &polyline
                    .corner_approx(i - 3)
                    .expect("i - 3 is below cornerCount"),
            );
            let mut check_line_1 = lines[0];
            let mut check_line_2 = lines[0];
            if in_clip_shape {
                let check_line_0 = new_lines[new_line_index - 1];
                check_line_1 = new_lines[new_line_index];
                check_line_2 = lines[i - 1];
                let check_line_3 = if i < lines.len() {
                    lines[i]
                } else {
                    lines[i - 2]
                };
                let check_polyline =
                    new_polyline(vec![check_line_0, check_line_1, check_line_2, check_line_3]);
                skip_lines = check_polyline.lines().len() == 4
                    && self.base.clip_contains(
                        &check_polyline
                            .corner_approx(1)
                            .expect("a four-line polyline has three corners"),
                    );
                if skip_lines {
                    let shape_to_check = check_polyline
                        .offset_shape(self.base.current_half_width, 0)
                        .expect("a four-line polyline has two offset shapes");
                    skip_lines = self.base.check(board, &shape_to_check);
                }
                if skip_lines {
                    let shape_to_check = check_polyline
                        .offset_shape(self.base.current_half_width, 1)
                        .expect("a four-line polyline has two offset shapes");
                    skip_lines = self.base.check(board, &shape_to_check);
                }
            }
            if skip_lines {
                if i == lines.len() {
                    second_last_corner_skipped = true;
                }
                if board.changed_area.is_some() {
                    let layer = self.base.current_layer;
                    let new_corner = check_line_1.intersection_approx(&check_line_2);
                    board.join_changed_area(&new_corner, layer);
                    let skipped_corner = lines[i - 2].intersection_approx(&lines[i - 3]);
                    board.join_changed_area(&skipped_corner, layer);
                }
                polyline_changed = true;
                i += 1;
            } else {
                new_line_index += 1;
                new_lines[new_line_index] = lines[i - 3];
            }
            i += 1;
        }
        if !polyline_changed {
            return None;
        }
        if second_last_corner_skipped {
            new_line_index += 1;
            new_lines[new_line_index] = lines[lines.len() - 1];
            new_line_index += 1;
            new_lines[new_line_index] = lines[lines.len() - 2];
        } else {
            for k in (1..=3).rev() {
                new_line_index += 1;
                new_lines[new_line_index] = lines[lines.len() - k];
            }
        }
        new_lines.truncate(new_line_index + 1);
        Some(new_polyline(new_lines))
    }

            pub(crate) fn smoothen_start_corner_at_trace(
        &mut self,
        _board: &mut Board,
        _trace: ItemId,
    ) -> Option<Polyline> {
        None
    }

            pub(crate) fn smoothen_end_corner_at_trace(
        &mut self,
        _board: &mut Board,
        _trace: ItemId,
    ) -> Option<Polyline> {
        None
    }
}
