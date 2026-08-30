//! Port of `board.actions.ForcedPadRouter` (`board/actions/ForcedPadRouter.java`).

use fr_board::board::ShapeTraceEntries;
use fr_board::datastructures::StopCheck;
use fr_board::free_trace_tree_shapes;
use fr_board::items::Item;
use fr_board::prelude::*;
use fr_board::{BoardError, ItemId, StopConnectionOption, TimeLimit};
use fr_geometry::{Direction, Line, Point, Polyline, PolylineShapeOps, TileShape};

use crate::board_ext::drill_item_mover::{DrillItemMover, drill_item_center};
use crate::board_ext::swallow_normalize_error;
use crate::board_ext::trace_shover::TraceShover;

/// Port of `board.actions.ForcedPadRouter.CheckDrillResult` (ForcedPadRouter.java:494-500).
///
/// Java's constants are declared `DRILLABLE, DRILLABLE_WITH_ATTACH_SMD, NOT_DRILLABLE` and
/// nothing reads `ordinal()` or `values()`, so the order here is Java's for readability only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CheckDrillResult {
    /// `DRILLABLE` — the pad fits once the obstacle traces are shoved aside.
    Drillable,
    /// `DRILLABLE_WITH_ATTACH_SMD` — as `Drillable`, but the pad overlaps an SMD pin it is
    /// allowed to attach to (`:281-288`, `ForcedViaInserter.java:120-123`).
    DrillableWithAttachSmd,
    /// `NOT_DRILLABLE` — the check failed.
    NotDrillable,
}

/// Port of `board.actions.ForcedPadRouter` (ForcedPadRouter.java:33-500).
///
/// Java's class holds a `private final RoutingBoard board`; the port passes the board per call
/// instead (plan-2 ruling 11: `Board` is `Send + Sync` and holds no back-pointers), so every
/// method is an associated function on a unit struct.
///
/// # The check half and the shove half
///
/// [`Self::check_forced_pad`] only *asks* whether a pad would fit — Task 10 landed it, and
/// `check_forced_pad_does_not_mutate_the_board` in `tests/forced_via.rs` pins that it changes
/// nothing. [`Self::forced_pad`] is the mutating twin **controller ruling AA** moved into Task
/// 10b: it shoves for real and, per its own javadoc, "if the shove failed, the database may be
/// damaged, so that an undo becomes necessary".
pub struct ForcedPadRouter;

impl ForcedPadRouter {
    /// Port of `ForcedPadRouter.checkForcedPad(TileShape, ShapeEntrySide, int, int[], int,
    /// boolean, Collection<Item>, int, int, boolean, TimeLimit)` (ForcedPadRouter.java:221-340):
    /// "checks, if possible obstacle traces can be shoved aside, so that a pad with the input
    /// parameters can be inserted without clearance violations. [...] If `ignoreItems != null`,
    /// items in this list are not checked. If `checkOnlyFront`, only trace obstacles in the
    /// direction from `fromSide` are checked for performance reasons. This is the case when
    /// moving drill items."
    ///
    /// This is the method that closes Task 9's cycle: `DrillItemMover.check:86` calls it and it
    /// calls `DrillItemMover.check` back at `:269-278`, plus `TraceShover.check` at `:322-334`.
    ///
    /// `ignore_items` is `Option<&[ItemId]>` where Java takes a `Collection<Item>`; unlike
    /// `DrillItemMover::check`'s (quirk #175) this one is **read only** — `:244`'s `removeAll`
    /// mutates the local obstacle list, never the caller's — so a shared slice is exact.
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
                // Java's `shoveViaList` is a `List<Via>`, so `getCenter()` always answers.
                return CheckDrillResult::NotDrillable;
            };
            let delta = Point::Int(new_via_center).difference_by(&via_center);
            // :268-278. Java allocates a **fresh** `LinkedList` per via, so quirk #175's
            // side effect never reaches this method's caller.
            let mut check_ignore_items = Vec::new();
            if !DrillItemMover::check(
                board,
                current_shove_via,
                &delta,
                max_recursion_depth,
                max_via_recursion_depth - 1,
                Some(&mut check_ignore_items),
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
            // `ShapeAndEntrySide`'s `:28` is `trace.getTreeShape(searchTree, index)`, which Java
            // memoises on the item (Item.java:228-238) — computed once per piece, looked up per
            // index. The port's pieces carry no cache, so the vector is computed once here,
            // outside the `for i` loop, exactly as Task 9's `TraceShover::check` does.
            let substitute_tree_shapes = free_trace_tree_shapes(board, &current_substitute_trace);
            for i in 0..current_substitute_trace.tile_shape_count() {
                // totalized: `ForcedPadRouter.checkForcedPad`'s `polyline().lines[i + 1]` (`:309`)
                // -> a skipped index. `i` is bounded by `tileShapeCount()` = `lines.length - 2`,
                // so `i + 1` is always in range and Java's array access cannot throw.
                // Unreachable — no register row.
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
                // :319-335.
                //
                // totalized: `ForcedPadRouter.checkForcedPad`'s `new ShapeAndEntrySide(…, i, …)`
                // (`:320-321`) -> a skipped index. Java's constructor cannot fail: `:28`'s
                // `getTreeShape(searchTree, i)` answers null only for an index the piece does not
                // have, and the same bound as above makes that unreachable. No register row.
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

    /// Port of `ForcedPadRouter.forcedPad(TileShape, ShapeEntrySide, int, int[], int, boolean,
    /// Collection<Item>, int, int)` (ForcedPadRouter.java:346-465): "shoves aside traces, so that
    /// a pad with the input parameters can be inserted without clearance violations. Returns
    /// false, if the shove failed. In this case the database may be damaged, so that an undo
    /// becomes necessary."
    ///
    /// The mutating twin of [`Self::check_forced_pad`]. Three shapes of difference are worth
    /// naming, because a reader who knows `checkForcedPad` will look for them:
    ///
    /// * it has **no `checkOnlyFront` and no `TimeLimit`** — the `inFrontOfPad` short-circuit is a
    ///   check-time optimisation and there is no cancellation once the database is changing;
    /// * the via arm is not here at all: `:363-374` hands the whole problem to
    ///   [`DrillItemMover::shove_vias`], and `:389` then insists the entries' via list came back
    ///   **empty** (`obstaclesShovable && shoveViaList.isEmpty()`);
    /// * there is **no `stackDepth() > 1` gate**. `checkForcedPad:297-300` has one; this method
    ///   does not, so a pad whose entries stack answers `false` there and shoves here.
    ///
    /// `ignore_items` is `Option<&[ItemId]>`: Java's `:386`'s `removeAll` mutates the local
    /// obstacle list, never the caller's collection, and `:420` passes the same reference straight
    /// down to `TraceShover.insert`, which is equally read-only. `check_forced_pad` takes the
    /// slice for the same reason.
    ///
    /// The [`StopCheck`] is plan-6 ruling 6's, threaded into the `fr-board` walks below this one
    /// that Java cannot leave on a ladder board (quirk #76): `PolylineTrace.split` under
    /// `normalize`, and `Item.getConnectionItems` under the tail cleanup at `:445-457`.
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
        // :355-358. FRLogger.warn("ShoveTraceAux.forced_pad: padShape is empty")
        if pad_shape.is_empty() {
            return Ok(true);
        }
        // :359-362.
        if !PolylineShapeOps::is_contained_in(pad_shape, &board.get_bounding_box()) {
            let outline = board.get_outline();
            board.set_shove_failing_obstacle(outline);
            return Ok(false);
        }
        // :363-374. Note `copperSharingAllowed = false` here, where `TraceShover.insert:444`
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

        // :375-387. The **default** tree, not the engine's compensated one.
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
        // :388-395. The third argument is `isPadCheck`: `true` here, `false` in
        // `TraceShover.insert:455` — the difference `ShapeTraceEntries.storeItems:180-183` reads.
        let obstacles_shovable =
            shape_entries.store_items(board, &obstacles, true, copper_sharing_allowed)
                && shape_entries.shove_via_list.is_empty();
        if !obstacles_shovable {
            let found = shape_entries.get_found_obstacle();
            board.set_shove_failing_obstacle(found);
            return Ok(false);
        }
        // :396-403.
        let trace_piece_count = shape_entries.substitute_trace_count();
        if trace_piece_count == 0 {
            return Ok(true);
        }
        if max_recursion_depth <= 0 {
            let found = shape_entries.get_found_obstacle();
            board.set_shove_failing_obstacle(found);
            return Ok(false);
        }
        // :404-406.
        let tails_exist_before = board.contains_trace_tails(obstacles.iter().copied(), net_numbers);
        shape_entries.cutout_traces(board, &obstacles);
        let is_orthogonal_mode = matches!(pad_shape, TileShape::Box(_));

        // :407-464.
        loop {
            let Some(current_substitute_trace) = shape_entries.next_substitute_trace_piece(board)
            else {
                break;
            };
            // :412-420.
            // totalized: `ForcedPadRouter.forcedPad`'s `firstCorner().equals(lastCorner())` (`:412`)
            // -> a skipped piece for a polyline with no corners at all, where Java throws a
            // `NullPointerException` on the receiver. `nextSubstituteTracePiece` builds every
            // piece from at least three lines, so it cannot arise. `Option<Point>`'s `PartialEq`
            // is Java's `Point.equals` exactly, `getClass()` test included. No register row.
            if current_substitute_trace.first_corner() == current_substitute_trace.last_corner() {
                continue;
            }
            let substitute_net_nos = current_substitute_trace.hdr.net_nos.clone();
            let substitute_clearance_class = current_substitute_trace.hdr.clearance_class();
            // Java's `ShapeAndEntrySide:28` calls `trace.getTreeShape(searchTree, index)`, which
            // memoises the whole vector on the item (Item.java:228-238) the first time — so Java
            // computes it once per piece and looks it up per index, even though `insert` below
            // mutates the board in between. The port has no per-item cache, so the vector is
            // computed once here, outside the `for i` loop, which is that memoisation.
            let substitute_tree_shapes = free_trace_tree_shapes(board, &current_substitute_trace);
            for i in 0..current_substitute_trace.tile_shape_count() {
                // totalized: `ForcedPadRouter.forcedPad`'s `new ShapeAndEntrySide(…, i, …)`
                // (`:417-418`) -> a skipped index. `i` is bounded by `tileShapeCount()`, so `:28`'s
                // `getTreeShape(searchTree, i)` is always in range and Java's constructor cannot
                // fail. Unreachable — no register row.
                let Some(Some(current_tree_shape)) = substitute_tree_shapes.get(i).cloned() else {
                    continue;
                };
                // `inShoveCheck = false` (`:418`), where both `check` paths pass `true`: with it
                // false, `ShapeAndEntrySide:70-73` fills a `fromSide` in from the piece's own
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
            // :446-450. Java wraps `normalize` in `catch (Exception e)` and only logs — see the
            // "the two `normalize` catches" section on the module.
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

    /// Port of `ForcedPadRouter.calcFromSide(TileShape, Point, int, int, int)`
    /// (ForcedPadRouter.java:471-492): "looks for a side of shape, so that a trace line from the
    /// shape center to the nearest point on this side does not conflict with any obstacles."
    ///
    /// Two sweeps: the first with the caller's clearance class, the second (`:483-490`) with
    /// class 0. Every answer is built as `new ShapeEntrySide(i, null)`, so the border
    /// intersection is **never** filled in — unlike `ForcedViaInserter::calculate_from_side`,
    /// which always fills it. `ShapeEntrySide::NOT_CALCULATED` (`no = -1`) when neither sweep
    /// finds a side, which is what `ForcedViaInserter.check` then hands to `checkForcedPad`.
    ///
    /// `pub` where Java is package-private, so `tests/forced_via.rs` — an integration test, i.e.
    /// a separate crate — can pin it against the JVM.
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

    /// Port of the private `ForcedPadRouter.inFrontOfPad(Line, TileShape, int, int, boolean)`
    /// (ForcedPadRouter.java:57-212): "checks, if `line` is in front of `padShape` when shoving
    /// from `fromSide`."
    ///
    /// An eight-case switch over `fromSide`, each case a three-way disjunction plus an optional
    /// `withSides` widening. Only implemented for octagons (`:59-62`) and only for lines with
    /// `IntPoint` end points (`:64-67`); both fall through to `true`.
    ///
    /// # Java bug: `ForcedPadRouter.inFrontOfPad`'s `case 0` reads `lineB.x` twice (quirk #176)
    ///
    /// `:78` is `Math.min(lineA.x + lineA.y, lineB.x + lineB.x)` where all seven sibling cases
    /// and both neighbouring disjuncts of this one read `x + y`. Reproduced verbatim. The
    /// observable consequence is that at `fromSide = 0` the answer depends on which end point of
    /// the line is `a`, even though the two `Line`s describe the same geometry; the probe pins
    /// `((0,900),(900,0)) -> true` against `((900,0),(0,900)) -> false`.
    ///
    /// `pub` where Java is private, for the same reason as [`Self::calc_from_side`].
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
        // :63.
        //
        // totalized: `ForcedPadRouter.inFrontOfPad`'s `padShape.boundingOctagon()` (`:63`) -> the
        // `true` of the arm above. The port's `bounding_octagon` answers `None` only for an
        // unbounded simplex, and `Simplex.isIntOctagon` (Simplex.java:365-379) already required
        // every corner to be bounded, so the guard above makes this unreachable. No register row.
        let Some(pad) = pad_shape.bounding_octagon() else {
            return true;
        };
        // totalized: `ForcedPadRouter.inFrontOfPad`'s `line.a instanceof IntPoint` guard (`:64-67`)
        // -> always taken. This port's `Line` holds two `IntPoint`s by construction (a Plan 1
        // decision; Java's `Line(Point, Point)` merely warns for anything else), so the "not
        // implemented" arm is unreachable here. No register row.
        let (a, b) = (line.a, line.b);

        // :69. `width * Math.sqrt(2)`, an `int * double` product — **not** `Limits.sqrt2`, which
        // Java uses elsewhere; the two are the same `double`.
        let diag_width = f64::from(width) * f64::sqrt(2.0);
        // The three families of coordinate the switch compares. Every one is Java `int`
        // arithmetic, which wraps on overflow.
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
        // Java bug: `ForcedPadRouter.inFrontOfPad`'s `case 0` — `:78`'s second argument is
        // `lineB.x + lineB.x`, twice `x`. Quirk #176; see the doc comment above.
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

/// Port of the private `ForcedPadRouter.calcCheckShapeForFromSide(TileShape, Point, Line)`
/// (ForcedPadRouter.java:42-54): the one-unit-wide sliver from the shape centre out to its
/// projection on `borderLine`, which `calcFromSide` then hands to `checkTraceShape`.
///
/// The `shape` parameter Java declares is unused in its body — the shape only reaches the method
/// through `shapeCenter` and `borderLine` — so the port does not take it.
///
/// # The degraded value for all three failure arms is "skip this border line"
///
/// Java can leave this method three ways that are not a shape, and **all three end in a crash**
/// rather than in a value: two of them here and one in the caller. The port answers `None` at
/// each, and `calcFromSide:475-490` treats that exactly as it treats a border line whose check
/// shape was built but rejected — it moves to the next `i`, and falls through to
/// `ShapeEntrySide::NOT_CALCULATED` if no line works.
///
/// **Why that degraded value, and not a `Result`.** Plan-6 ruling 7 fixes the recovery boundaries
/// at five named `catch` sites, and neither this method nor `calcFromSide` is one of them; adding
/// a sixth would push a `Result` through `ForcedViaInserter::check` and `::insert`, whose Java
/// forms answer `boolean`. More to the point, `NOT_CALCULATED` is **Java's own vocabulary for
/// this method**: `calcFromSide:491` already returns it when both sweeps find no usable side. So
/// skipping a line the port cannot build a check shape for lands on a value Java itself produces
/// for "no side works", instead of inventing one. Every caller already handles it —
/// `ForcedViaInserter.check:164-166` hands `NOT_CALCULATED` straight to `checkForcedPad`, which
/// takes `fromSide.no = -1` without complaint.
///
/// **Reachability.** All three arms look unreachable in production, which is the argument for "no
/// register row", not for "no marker". `offset` is `board.getMinTraceHalfWidth()` at both call
/// sites (`ForcedViaInserter.java:166`, `:296`) and is therefore `>= 0`, so `offsetShape` only
/// grows the pad and its centre stays strictly interior; `lines[0]` and `lines[1]` are
/// perpendicular by construction, and `lines[2]` is parallel to `lines[0]` but through the
/// centre's *projection*, which coincides with the centre only if the centre sits within half a
/// unit of the offset border line.
fn calc_check_shape_for_from_side(shape_center: &Point, border_line: &Line) -> Option<TileShape> {
    // totalized: `ForcedPadRouter.calcCheckShapeForFromSide`'s `new Line(shapeCenter, dir)`
    // (`:49-51`) -> skip this border line. Java only logs a warning for a non-`IntPoint` centre
    // and then builds a `Line` whose arithmetic is broken (Line.java:36-42). See the
    // "degraded value" section above; no register row, every production caller passes an
    // `IntPoint`.
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
    // totalized: `ForcedPadRouter.calcCheckShapeForFromSide`'s `new Polyline(lines)` (`:52`) -> skip
    // this border line. `Polyline::from_lines` is `Err` on exactly one input class — the
    // `removeOverlaps` underflow where Java reads `tmpArr[-1]` and throws
    // `ArrayIndexOutOfBoundsException: Index -1` (Polyline.java:148), quirk #22. Java's **other**
    // sub-three-line exit is not an error at all: it stores `lines = new Line[0]` (`:78-81`) and
    // returns normally, which the port reproduces as `Ok` with an empty polyline — that path
    // therefore falls to the `offset_shape` arm below, not to this one. See the "degraded value"
    // section above.
    let check_line = Polyline::from_lines(lines).ok()?;
    // :53.
    //
    // totalized: `ForcedPadRouter.calcCheckShapeForFromSide`'s `checkLine.offsetShape(1, 0)`
    // (`:53`) -> skip this border line. This is the arm the empty polyline above reaches: with
    // `lines.length == 0`, `no > lines.length - 3` holds, so Java warns and returns **`null`**
    // (Polyline.java:518-521) — it does not throw here. The crash is one call later:
    // `calcFromSide:479` hands that `null` to `BasicBoard.checkTraceShape`, whose first statement
    // is `shape.isContainedIn(boundingBox)` (BasicBoard.java:990), a `NullPointerException`. See
    // the "degraded value" section above.
    check_line.offset_shape(1, 0)
}

// The deferral roster for `board/actions/ForcedPadRouter.java` is empty: every method of the class
// is ported. `checkForcedPad`, `inFrontOfPad`, `calcFromSide` and `calcCheckShapeForFromSide`
// landed in Task 10; `forcedPad` above is controller ruling AA's Task 10b.
