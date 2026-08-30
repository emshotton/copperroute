//! Port of `board/optimize/TraceTightener90.java` (169 lines): the 90-degree regime.

use fr_board::prelude::*;
use fr_geometry::{Line, Polyline};

use super::base::{TightenerBase, new_polyline};

/// `class TraceTightener90 extends TraceTightener` (TraceTightener90.java:12).
///
/// It adds no state of its own — Java's constructor (`:15-23`) forwards every argument to
/// `super` — so the Rust type is a newtype over `TightenerBase`, and
/// [`TraceTightener`](super::TraceTightener) is the enum that stands in for the dynamic
/// dispatch.
pub struct TraceTightener90<'a> {
    pub(crate) base: TightenerBase<'a>,
}

impl<'a> TraceTightener90<'a> {
    /// Port of the constructor `TraceTightener90(RoutingBoard, int[], Stoppable, int, Point, int)`
    /// (TraceTightener90.java:15-23), whose whole body is the `super(...)` call.
    pub(crate) fn new(base: TightenerBase<'a>) -> TraceTightener90<'a> {
        TraceTightener90 { base }
    }

    /// Port of `pullTight(Polyline)` (TraceTightener90.java:26-36).
    ///
    /// Java's loop condition is `newResult != prevResult` — **reference** identity, not value
    /// equality: it stops as soon as all three steps hand their argument object straight back.
    /// Each step here answers `None` for exactly that case, so `changed` below is Java's `!=`.
    pub(crate) fn pull_tight(
        &mut self,
        board: &mut Board,
        polyline: &Polyline,
    ) -> Option<Polyline> {
        // :27 — `avoidAcidTraps` always answers its argument (quirk #182). `ever_changed` is
        // seeded from this arm rather than from the loop, because Java returns `newResult` and
        // `PolylineTrace.pullTight:837` compares it against the **original** argument: a
        // replacement here would count as "changed" even if all three steps below then handed
        // their argument back. The arm is dead, the assignment is Java's.
        let (mut new_result, mut ever_changed) = match self.base.avoid_acid_traps(polyline) {
            Some(replacement) => (replacement, true),
            None => (polyline.clone(), false),
        };
        // :28-29: `prevResult` starts as `null`, so the first round always runs.
        let mut changed = true;
        while changed && !self.base.is_stop_requested() {
            let mut current = new_result;
            let mut any = false;
            // :31.
            if let Some(tmp1) = self.try_skip_second_corner(board, &current) {
                current = tmp1;
                any = true;
            }
            // :32.
            if let Some(tmp2) = self.try_skip_corners(board, &current) {
                current = tmp2;
                any = true;
            }
            // :33.
            if let Some(tmp3) = self.base.reposition_lines(board, &current) {
                current = tmp3;
                any = true;
            }
            new_result = current;
            changed = any;
            ever_changed |= any;
        }
        // :35.
        if ever_changed { Some(new_result) } else { None }
    }

    /// Port of the private `trySkipSecondCorner(Polyline)` (TraceTightener90.java:39-70): "tries
    /// to skip the second corner of polyline. Return polyline, if nothing was changed."
    ///
    /// The four check lines are deliberately out of order — `lines[1]`, `lines[0]`, `lines[3]`,
    /// `lines[4]` — because dropping the second corner turns the polyline's opening perpendicular
    /// line into its second line.
    fn try_skip_second_corner(
        &mut self,
        board: &mut Board,
        polyline: &Polyline,
    ) -> Option<Polyline> {
        // :40-42.
        if polyline.lines().len() < 5 {
            return None;
        }
        // :43-48.
        let lines = polyline.lines();
        let check_polyline = new_polyline(vec![lines[1], lines[0], lines[3], lines[4]]);
        // :49-52.
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
        // :53-63.
        for i in 0..2 {
            let shape_to_check = check_polyline
                .offset_shape(self.base.current_half_width, i)
                .expect("a four-line polyline has two offset shapes");
            if !self.base.check(board, &shape_to_check) {
                return None;
            }
        }
        // :64-69: now the second corner can be skipped.
        let mut new_lines: Vec<Line> = Vec::with_capacity(lines.len() - 1);
        new_lines.push(lines[1]);
        new_lines.push(lines[0]);
        new_lines.extend_from_slice(&lines[3..]);
        Some(new_polyline(new_lines))
    }

    /// Port of the private `trySkipCorners(Polyline)` (TraceTightener90.java:73-158): "tries to
    /// reduce the amount of corners of polyline. Return polyline, if nothing was changed."
    ///
    /// # Panics
    ///
    /// On a polyline with fewer than two lines, where Java's `newLines[1] = polyline.lines[1]`
    /// (`:76`) throws `ArrayIndexOutOfBoundsException`, and wherever the `newLines` array Java
    /// sizes at `polyline.lines.length` (`:74`) overflows — the tail at `:142-153` can write two
    /// or three more entries than the loop consumed. Both are the caller's exception, and no
    /// `catch` stands between here and `AutorouteConnectionRouter.route:155-158`.
    fn try_skip_corners(&mut self, board: &mut Board, polyline: &Polyline) -> Option<Polyline> {
        let lines = polyline.lines();
        // :74-80. Java's `new Line[polyline.lines.length]` is a null-filled array whose bounds
        // throw; a `Vec` of the
        // same length indexed with `[]` panics the same way **out of bounds** — but not in
        // bounds: an unwritten in-bounds slot silently carries `lines[0]`'s value *and its
        // identity token* (quirk #74), where Java carries `null` and NPEs inside
        // `new Polyline(...)`. Every slot that survives into the result is written first, so the
        // difference is unreachable; it is a silent-wrong-answer shape rather than a panic
        // shape, so a future edit to the write pattern has to re-check it.
        let mut new_lines: Vec<Line> = vec![lines[0]; lines.len()];
        new_lines[0] = lines[0];
        new_lines[1] = lines[1];
        let mut new_line_index: usize = 1;
        let mut polyline_changed = false;
        let mut second_last_corner_skipped = false;
        // :81.
        let mut i: usize = 5;
        while i <= lines.len() {
            let mut skip_lines = false;
            // :83-84.
            let in_clip_shape = self.base.clip_contains(
                &polyline
                    .corner_approx(i - 3)
                    .expect("i - 3 is below cornerCount"),
            );
            let mut check_line_1 = lines[0];
            let mut check_line_2 = lines[0];
            if in_clip_shape {
                // :86-94.
                let check_line_0 = new_lines[new_line_index - 1];
                check_line_1 = new_lines[new_line_index];
                check_line_2 = lines[i - 1];
                let check_line_3 = if i < lines.len() {
                    lines[i]
                } else {
                    // Use as concluding line the second last line.
                    lines[i - 2]
                };
                // :95-99.
                let check_polyline =
                    new_polyline(vec![check_line_0, check_line_1, check_line_2, check_line_3]);
                skip_lines = check_polyline.lines().len() == 4
                    && self.base.clip_contains(
                        &check_polyline
                            .corner_approx(1)
                            .expect("a four-line polyline has three corners"),
                    );
                // :100-119.
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
            // :121-137.
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
        // :139-141.
        if !polyline_changed {
            return None;
        }
        // :142-153.
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
        // :155-157.
        new_lines.truncate(new_line_index + 1);
        Some(new_polyline(new_lines))
    }

    /// Port of `smoothenStartCornerAtTrace(PolylineTrace)` (TraceTightener90.java:160-163):
    /// the 90-degree regime returns `null` unconditionally.
    pub(crate) fn smoothen_start_corner_at_trace(
        &mut self,
        _board: &mut Board,
        _trace: ItemId,
    ) -> Option<Polyline> {
        None
    }

    /// Port of `smoothenEndCornerAtTrace(PolylineTrace)` (TraceTightener90.java:165-168):
    /// the 90-degree regime returns `null` unconditionally.
    pub(crate) fn smoothen_end_corner_at_trace(
        &mut self,
        _board: &mut Board,
        _trace: ItemId,
    ) -> Option<Polyline> {
        None
    }
}
