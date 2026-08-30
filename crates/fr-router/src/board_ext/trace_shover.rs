//! Port of the check half of `board.optimize.TraceShover` (`board/optimize/TraceShover.java`).

use std::collections::BTreeSet;

use fr_board::board::ShapeTraceEntries;
use fr_board::free_trace_tree_shapes;
use fr_board::items::Item;
use fr_board::prelude::*;
use fr_board::{ItemId, TimeLimit};
use fr_geometry::{
    Direction, IntBox, Line, LineSegment, Point, Polyline, PolylineShapeOps, ShapeOps, TileShape,
    java_min,
};

use crate::board_ext::drill_item_mover::{DrillItemMover, drill_item_center, tree_shape_on_layer};

/// What the private `springOver` (TraceShover.java:611-818) answered, with Java's
/// **reference identity** made explicit.
///
/// Java's caller distinguishes three outcomes by pointer: `null` (no way round the obstacle), the
/// **same** `Polyline` object it passed in (`:692`, `:757` — no obstacle at all, nothing to do)
/// and a different one (a detour was built). `TraceShover.check:382` and
/// `springOverObstacles:845` both branch on `!=` against the argument, so the distinction is
/// load-bearing and a plain `Option<Polyline>` would lose it: the detour can be *equal* to the
/// input without being *identical* to it.
#[derive(Debug, Clone, PartialEq)]
pub enum SpringOverOutcome {
    /// Java returned the polyline it was handed, by identity — "there were no obstacles".
    Unchanged,
    /// Java returned a freshly built polyline that wraps around the obstacles.
    Changed(Polyline),
}

/// Port of `board.optimize.TraceShover` (TraceShover.java:42-875) — the check half.
///
/// Java's class holds a `private final RoutingBoard board` and its two `check` methods differ
/// only in whether that field is used; the port passes the board per call instead (plan-2 ruling
/// 11: `Board` is `Send + Sync` and holds no back-pointers), so both become associated functions
/// on a unit struct. `check_segment` is Java's `static check(RoutingBoard, LineSegment, …)` and
/// `check` is the instance `check(TileShape, ShapeEntrySide, …)`; Rust has no overloading.
// renamed: the static `TraceShover.check(RoutingBoard, LineSegment, ...)` -> `check_segment`.
pub struct TraceShover;

impl TraceShover {
    /// Port of the **static** `TraceShover.check(RoutingBoard, LineSegment, boolean, int, int[],
    /// int, int, int, int)` (TraceShover.java:57-225): "checks if a shove with the input
    /// parameters is possible without clearance violations. The result is the maximum length of a
    /// trace from the start of the line segment to the end of the line segment, for which the
    /// algorithm succeeds. If the algorithm succeeds completely, the result will be equal to
    /// `Integer.MAX_VALUE`."
    ///
    /// `Integer.MAX_VALUE` is `f64::from(i32::MAX)`, not `f64::INFINITY`: `:190` compares the
    /// recursive answer against it and `:218` takes a `Math.min` with it, so the exact value is
    /// observable.
    ///
    /// A shape that overlaps a **shovable foreign-net via** reaches [`DrillItemMover::check`] at
    /// `:119-135`, and through it `ForcedPadRouter::check_forced_pad`, which reaches this method
    /// back: the three form one mutual recursion, closed by plan-6 Task 10.
    #[allow(clippy::too_many_arguments)]
    pub fn check_segment(
        board: &mut Board,
        line_segment: &LineSegment,
        shove_to_the_left: bool,
        layer: usize,
        net_numbers: &[i32],
        trace_half_width: i32,
        clearance_class_index: usize,
        max_recursion_depth: i32,
        max_via_recursion_depth: i32,
    ) -> f64 {
        // :67-70.
        let tree = board.trees.get_default_tree().id();
        let compensation_used = board
            .trees
            .get_default_tree()
            .is_clearance_compensation_used();
        let mut trace_half_width = trace_half_width;
        if compensation_used {
            trace_half_width += board.trees.get_default_tree().clearance_compensation_value(
                clearance_class_index,
                layer,
                &board.rules,
            );
        }

        // :71-75. `lineSegment.toPolyline()` throws on a degenerate segment where the port
        // answers `Err`; either way the shove cannot be checked, which is the `return 0` below.
        let Ok(segment_polyline) = line_segment.to_polyline() else {
            return 0.0;
        };
        let trace_shapes = segment_polyline.offset_shapes(trace_half_width);
        if trace_shapes.len() != 1 {
            // FRLogger.warn("TraceShover.check: traceShape count 1 expected")
            return 0.0;
        }

        // :77-84.
        let trace_shape = trace_shapes[0].clone();
        if trace_shape.is_empty() {
            // FRLogger.warn("TraceShover.check: traceShape is empty")
            return 0.0;
        }
        if !PolylineShapeOps::is_contained_in(&trace_shape, &board.get_bounding_box()) {
            return 0.0;
        }

        // :85-94.
        let from_side =
            ShapeEntrySide::from_line_segment(line_segment, &trace_shape, shove_to_the_left);
        let mut shape_entries = ShapeTraceEntries::new(
            trace_shape.clone(),
            layer,
            net_numbers.to_vec(),
            clearance_class_index,
            Some(from_side),
        );
        let obstacles = board.overlapping_items_with_clearance(
            &trace_shape,
            Some(layer),
            &[],
            clearance_class_index,
        );
        let obstacles_shovable = shape_entries.store_items(board, &obstacles, false, true);
        if !obstacles_shovable || shape_entries.trace_tails_in_shape() {
            return 0.0;
        }
        // :95-99.
        let trace_piece_count = shape_entries.substitute_trace_count();
        if shape_entries.stack_depth() > 1 {
            return 0.0;
        }

        // :101-107.
        let start_corner_approx = line_segment.start_point_approx();
        let end_corner_approx = line_segment.end_point_approx();
        let segment_length = end_corner_approx.distance(&start_corner_approx);
        let mut result = f64::from(i32::MAX);

        // :109-155. "check, if the obstacle vias can be shoved"
        for current_shove_via in shape_entries.shove_via_list.clone() {
            // :112-114.
            if board
                .get_item(current_shove_via)
                .is_none_or(|item| item.shares_net_no(net_numbers))
            {
                continue;
            }
            let mut shove_via_ok = false;
            // :116-136.
            if max_via_recursion_depth > 0 {
                let new_via_center = DrillItemMover::try_shove_via_points(
                    board,
                    &trace_shape,
                    layer,
                    current_shove_via,
                    clearance_class_index,
                    false,
                );
                // :122-124.
                if new_via_center.is_empty() {
                    return 0.0;
                }
                let Some(via_center) = drill_item_center(board, current_shove_via) else {
                    return 0.0;
                };
                let delta = Point::Int(new_via_center[0]).difference_by(&via_center);
                let mut ignore_items = Vec::new();
                shove_via_ok = DrillItemMover::check(
                    board,
                    current_shove_via,
                    &delta,
                    max_recursion_depth,
                    max_via_recursion_depth - 1,
                    Some(&mut ignore_items),
                    None,
                );
            }

            // :138-154.
            if !shove_via_ok {
                let Some(via_center) = drill_item_center(board, current_shove_via) else {
                    return 0.0;
                };
                let via_center_approx = via_center.to_float();
                let mut projection =
                    start_corner_approx.scalar_product(&end_corner_approx, &via_center_approx);
                projection /= segment_length;
                let Some(via_shape) = tree_shape_on_layer(board, current_shove_via, tree, layer)
                else {
                    return 0.0;
                };
                let via_box = via_shape.bounding_box();
                let via_radius = 0.5 * via_box.max_width();
                let mut current_ok_length = projection - via_radius - f64::from(trace_half_width);
                if !compensation_used {
                    let via_clearance_class = board
                        .get_item(current_shove_via)
                        .map_or(0, Item::clearance_class);
                    current_ok_length -= f64::from(board.rules.clearance_matrix.get_value(
                        clearance_class_index,
                        via_clearance_class,
                        layer,
                        true,
                    ));
                }
                if current_ok_length <= 0.0 {
                    return 0.0;
                }
                result = java_min(result, current_ok_length);
            }
        }

        // :156-161.
        if trace_piece_count == 0 {
            return result;
        }
        if max_recursion_depth <= 0 {
            return 0.0;
        }

        // :163-223.
        let line_direction = Direction::Int(line_segment.get_line().direction());
        loop {
            let Some(current_substitute_trace) = shape_entries.next_substitute_trace_piece(board)
            else {
                break;
            };
            for i in 0..current_substitute_trace.tile_shape_count() {
                // totalized: `TraceShover.check`'s `new LineSegment(polyline, i + 1)` (`:170`) ->
                // a skipped index. Java's constructor only warns for an out-of-range `no`; `i` is
                // bounded by `tileShapeCount()` = `lines.length - 2`, so it is always in range.
                // Unreachable — no register row.
                let Some(mut current_line_segment) =
                    LineSegment::from_polyline(current_substitute_trace.polyline(), i + 1)
                else {
                    continue;
                };
                if shove_to_the_left {
                    // ":171-175. swap the line segment to get the correct shove length in case it
                    // is smaller than the length of the whole line segment."
                    current_line_segment = current_line_segment.opposite();
                }

                // :177.
                let is_in_front =
                    Direction::Int(current_line_segment.get_line().direction()) == line_direction;
                if !is_in_front {
                    continue;
                }
                // :179-189.
                let substitute_net_nos = current_substitute_trace.hdr.net_nos.clone();
                let substitute_half_width = current_substitute_trace.get_half_width();
                let substitute_clearance_class = current_substitute_trace.hdr.clearance_class();
                let shove_ok_length = Self::check_segment(
                    board,
                    &current_line_segment,
                    shove_to_the_left,
                    layer,
                    &substitute_net_nos,
                    substitute_half_width,
                    substitute_clearance_class,
                    max_recursion_depth - 1,
                    max_via_recursion_depth,
                );
                // :190-219.
                if shove_ok_length < f64::from(i32::MAX) {
                    if shove_ok_length <= 0.0 {
                        return 0.0;
                    }
                    let mut projection = java_min(
                        start_corner_approx.scalar_product(
                            &end_corner_approx,
                            &current_line_segment.start_point_approx(),
                        ),
                        start_corner_approx.scalar_product(
                            &end_corner_approx,
                            &current_line_segment.end_point_approx(),
                        ),
                    );
                    projection /= segment_length;
                    let mut current_ok_length = shove_ok_length + projection
                        - f64::from(trace_half_width)
                        - f64::from(substitute_half_width);
                    if compensation_used {
                        current_ok_length -=
                            f64::from(board.trees.get_default_tree().clearance_compensation_value(
                                substitute_clearance_class,
                                layer,
                                &board.rules,
                            ));
                    } else {
                        current_ok_length -= f64::from(board.rules.clearance_matrix.get_value(
                            clearance_class_index,
                            substitute_clearance_class,
                            layer,
                            true,
                        ));
                    }
                    if current_ok_length <= 0.0 {
                        return 0.0;
                    }
                    result = java_min(current_ok_length, result);
                }
                // :220 — the `break` is inside the `isInFront` arm, so the loop stops at the
                // first segment pointing the same way whether or not the check bit.
                break;
            }
        }
        result
    }

    /// Port of the **instance** `TraceShover.check(TileShape, ShapeEntrySide, Direction, int,
    /// int[], int, int, int, int, TimeLimit)` (TraceShover.java:231-411): "checks if a shove with
    /// the input parameters is possible without clearance violations. `dir` is used internally to
    /// prevent the check from bouncing back. Returns false, if the shove failed."
    ///
    /// `from_side` and `dir` are `Option` because Java passes `null` for both — `dir` from
    /// `checkForcedTracePolyline` (RoutingBoard.java:434) and `from_side` from every
    /// `ShapeAndEntrySide` whose cut lines produced no side (`:392-396`).
    ///
    /// The recursion has three arms and each spends a different budget: `DrillItemMover::check`
    /// (`:335-342`) spends `maxViaRecursionDepth`, `spring_over` (`:368-377`) spends
    /// `maxSpringOverRecursionDepth` — but **only when it actually changed something** (`:382-386`)
    /// — and the self-call (`:394-404`) spends `maxRecursionDepth`. The maze passes
    /// `ctrl.maxShoveTraceRecursionDepth = 20`, a hard-coded constant, so the accounting is
    /// transcribed exactly.
    ///
    /// A `trace_shape` that overlaps a **shovable foreign-net via** reaches
    /// [`DrillItemMover::check`] at `:335-342`, and through it
    /// `ForcedPadRouter::check_forced_pad`, which reaches this method back at
    /// `ForcedPadRouter.java:322-334`: the three form one mutual recursion, closed by plan-6
    /// Task 10.
    #[allow(clippy::too_many_arguments)]
    pub fn check(
        board: &mut Board,
        trace_shape: &TileShape,
        from_side: Option<&ShapeEntrySide>,
        dir: Option<Direction>,
        layer: usize,
        net_numbers: &[i32],
        clearance_class_index: usize,
        max_recursion_depth: i32,
        max_via_recursion_depth: i32,
        max_spring_over_recursion_depth: i32,
        time_limit: Option<&TimeLimit>,
    ) -> bool {
        // :242-244.
        if time_limit.is_some_and(TimeLimit::is_exceeded) {
            return false;
        }
        // :246-249.
        if trace_shape.is_empty() {
            // FRLogger.warn("ShoveTraceAux.check: traceShape is empty")
            return true;
        }
        // :250-253.
        if !PolylineShapeOps::is_contained_in(trace_shape, &board.get_bounding_box()) {
            let outline = board.get_outline();
            board.set_shove_failing_obstacle(outline);
            return false;
        }

        // :254-265.
        let mut shape_entries = ShapeTraceEntries::new(
            trace_shape.clone(),
            layer,
            net_numbers.to_vec(),
            clearance_class_index,
            from_side.copied(),
        );
        let mut obstacles = board.overlapping_items_with_clearance(
            trace_shape,
            Some(layer),
            &[],
            clearance_class_index,
        );
        let ignored = Self::ignore_items_at_tie_pins(board, trace_shape, layer, net_numbers);
        obstacles.retain(|id| !ignored.contains(id));
        let obstacles_shovable = shape_entries.store_items(board, &obstacles, false, true);
        if !obstacles_shovable {
            let found = shape_entries.get_found_obstacle();
            board.set_shove_failing_obstacle(found);
            return false;
        }
        // :266.
        let trace_piece_count = shape_entries.substitute_trace_count();

        // not ported: `TraceShover.check`'s `[shove_check_obstacles]` `FRLogger.trace` block
        // (:268-303) — a diagnostic string built from the obstacle list; no decision reads it.

        // :305-308.
        if shape_entries.stack_depth() > 1 {
            let found = shape_entries.get_found_obstacle();
            board.set_shove_failing_obstacle(found);
            return false;
        }
        // :309.
        let shape_radius = 0.5 * trace_shape.bounding_box().min_width();

        // :311-351. "check, if the obstacle vias can be shoved"
        for current_shove_via in shape_entries.shove_via_list.clone() {
            // :314-316.
            if board
                .get_item(current_shove_via)
                .is_none_or(|item| item.shares_net_no(net_numbers))
            {
                continue;
            }
            // :317-320.
            if max_via_recursion_depth <= 0 {
                board.set_shove_failing_obstacle(Some(current_shove_via));
                return false;
            }
            // :321-328.
            let Some(via_center) = drill_item_center(board, current_shove_via) else {
                return false;
            };
            let current_shove_via_center = via_center.to_float();
            let try_via_centers = DrillItemMover::try_shove_via_points(
                board,
                trace_shape,
                layer,
                current_shove_via,
                clearance_class_index,
                true,
            );
            let via_max_width = {
                let ctx = board.ctx();
                match board.get_item(current_shove_via) {
                    Some(Item::Via(via)) => via
                        .get_shape_on_layer(layer, &ctx)
                        .map_or(0.0, |shape| shape.bounding_box().max_width()),
                    Some(Item::Pin(pin)) => pin
                        .get_shape_on_layer(layer, &ctx)
                        .map_or(0.0, |shape| shape.bounding_box().max_width()),
                    _ => 0.0,
                }
            };
            let max_dist = 0.5 * via_max_width + shape_radius;
            let max_dist_square = max_dist * max_dist;

            // :329-347.
            let mut shove_via_ok = false;
            for (i, try_via_center) in try_via_centers.iter().enumerate() {
                if i == 0
                    || current_shove_via_center.distance_square(&try_via_center.to_float())
                        <= max_dist_square
                {
                    let delta = Point::Int(*try_via_center).difference_by(&via_center);
                    let mut ignore_items = Vec::new();
                    if DrillItemMover::check(
                        board,
                        current_shove_via,
                        &delta,
                        max_recursion_depth,
                        max_via_recursion_depth - 1,
                        Some(&mut ignore_items),
                        time_limit,
                    ) {
                        shove_via_ok = true;
                        break;
                    }
                }
            }
            // :348-350. Java does **not** set `shoveFailingObstacle` here, unlike every other
            // refusal in this method.
            if !shove_via_ok {
                return false;
            }
        }

        // :353-359.
        if trace_piece_count == 0 {
            return true;
        }
        if max_recursion_depth <= 0 {
            let found = shape_entries.get_found_obstacle();
            board.set_shove_failing_obstacle(found);
            return false;
        }

        // :361-409.
        let is_orthogonal_mode = matches!(trace_shape, TileShape::Box(_));
        let mut max_spring_over_recursion_depth = max_spring_over_recursion_depth;
        loop {
            let Some(mut current_substitute_trace) =
                shape_entries.next_substitute_trace_piece(board)
            else {
                break;
            };
            // :367-387.
            if max_spring_over_recursion_depth > 0 {
                let compensated_half_width = board
                    .trees
                    .get_default_tree()
                    .compensated_half_width(&current_substitute_trace, &board.rules);
                let substitute_net_nos = current_substitute_trace.hdr.net_nos.clone();
                let substitute_clearance_class = current_substitute_trace.hdr.clearance_class();
                let outcome = Self::spring_over(
                    board,
                    current_substitute_trace.polyline().clone(),
                    compensated_half_width,
                    layer,
                    &substitute_net_nos,
                    substitute_clearance_class,
                    false,
                    max_spring_over_recursion_depth,
                    None,
                );
                match outcome {
                    // :378-381. "spring_over did not work"
                    None => return false,
                    // :382 — the identity test: nothing changed, so nothing is spent.
                    Some(SpringOverOutcome::Unchanged) => {}
                    // :383-386. "spring_over changed something"
                    Some(SpringOverOutcome::Changed(new_polyline)) => {
                        max_spring_over_recursion_depth -= 1;
                        // `PolylineTrace.change` (PolylineTrace.java:937-941) on a trace that is
                        // not on the board just replaces its polyline — and a substitute piece
                        // never is.
                        current_substitute_trace.set_polyline(new_polyline);
                    }
                }
            }
            // :388-408.
            let substitute_net_nos = current_substitute_trace.hdr.net_nos.clone();
            let substitute_clearance_class = current_substitute_trace.hdr.clearance_class();
            // `ShapeAndEntrySide`'s `:28` is `trace.getTreeShape(searchTree, index)`, and Java
            // computes the piece's shape vector **once** and memoises it on the item
            // (Item.java:228-238) — every later index is a lookup. The port's pieces carry no
            // cache, so the vector is computed once here, outside the `for i` loop, and indexed
            // below. Nothing in the loop mutates the piece (`set_polyline` ran above it) and
            // `calculate_tree_shapes` reads only the polyline, the tree's compensation class and
            // the rules, none of which the recursion touches — so this is Java's memoisation, not
            // a behavioural change.
            let substitute_tree_shapes = free_trace_tree_shapes(board, &current_substitute_trace);
            for i in 0..current_substitute_trace.tile_shape_count() {
                // totalized: `TraceShover.check`'s `polyline().lines[i + 1]` (`:389`) -> a skipped
                // index. `i` is bounded by `tileShapeCount()` = `lines.length - 2`, so `i + 1` is
                // always in range and Java's array access cannot throw. Unreachable — no row.
                let Some(current_line) = current_substitute_trace
                    .polyline()
                    .lines()
                    .get(i + 1)
                    .copied()
                else {
                    continue;
                };
                let current_direction = Direction::Int(current_line.direction());
                // :390.
                let is_in_front = dir.as_ref().is_none_or(|d| *d == current_direction);
                if !is_in_front {
                    continue;
                }
                // :392-406.
                //
                // totalized: `TraceShover.check`'s `new ShapeAndEntrySide(…, i, …)` (`:392-393`)
                // -> a skipped index. Java's constructor cannot fail at all: `:28`'s
                // `getTreeShape(searchTree, i)` answers null only for an index the piece does not
                // have, and `:29-33` would then throw. The port refuses that index instead, and
                // the same bound as above makes it unreachable. No register row.
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
                if !Self::check(
                    board,
                    &current.shape,
                    current.from_side.as_ref(),
                    Some(current_direction),
                    layer,
                    &substitute_net_nos,
                    substitute_clearance_class,
                    max_recursion_depth - 1,
                    max_via_recursion_depth,
                    max_spring_over_recursion_depth,
                    time_limit,
                ) {
                    return false;
                }
            }
        }
        // :410.
        true
    }

    /// Port of `TraceShover.getIgnoreItemsAtTiePins` (TraceShover.java:592-603): the contacts of
    /// every own-net pin the shape overlaps, which `check` then removes from its obstacle list.
    ///
    /// Java's `Set<Item>` is a `TreeSet`, so it iterates in `Item.compareTo` order — **descending
    /// id** (quirk #63). The result is only ever used as the argument of `Collection.removeAll`
    /// (`:260`), so the order is unobservable there; it is kept anyway so the test can pin the
    /// probe's list verbatim.
    ///
    /// `pub` where Java is package-private: `tests/board_ext.rs` is an integration test, so a
    /// `pub(crate)` method would be untestable against the JVM.
    pub fn ignore_items_at_tie_pins(
        board: &Board,
        trace_shape: &TileShape,
        layer: usize,
        net_numbers: &[i32],
    ) -> Vec<ItemId> {
        // :593-594.
        let overlaps = board.overlapping_objects(trace_shape, Some(layer));
        let mut result: BTreeSet<ItemId> = BTreeSet::new();
        // :595-601.
        for current_object in overlaps {
            let TreeObject::Item(id) = current_object else {
                continue;
            };
            let is_own_net_pin = board.get_item(id).is_some_and(|item| {
                matches!(item, Item::Pin(_)) && item.shares_net_no(net_numbers)
            });
            if is_own_net_pin {
                result.extend(board.all_contacts_on_layer(id, layer));
            }
        }
        // Java's `TreeSet<Item>` order.
        result.into_iter().rev().collect()
    }

    /// Port of the private `TraceShover.springOver` (TraceShover.java:611-818): "checks if there
    /// are obstacles in the way of `polyline` and tries to wrap the polyline trace around these
    /// obstacles in counterclock sense. Returns null, if that is not possible. Returns polyline,
    /// if there were no obstacles. If `contactPins != null`, all pins not contained in
    /// `contactPins` are regarded as obstacles, even if they are of the own net."
    ///
    /// Reached from Plan 6 only through the instance [`check`](Self::check) (`:369`), which passes
    /// `overConnectedPins = false` and `contactPins = null`; `springOverObstacles` — the caller
    /// that passes the other combination — is `// added in Plan 7:` (see `board_ext/mod.rs`).
    /// The whole method is ported anyway, because `check`'s call reaches every branch of it.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn spring_over(
        board: &mut Board,
        polyline: Polyline,
        half_width: i32,
        layer: usize,
        net_numbers: &[i32],
        clearance_class_index: usize,
        over_connected_pins: bool,
        recursion_depth: i32,
        contact_pins: Option<&BTreeSet<ItemId>>,
    ) -> Option<SpringOverOutcome> {
        // :620-628.
        let mut found_obstacle: Option<ItemId> = None;
        let mut found_obstacle_bounding_box: Option<IntBox> = None;
        let check_net_no_arr: Vec<i32> = if contact_pins.is_none() {
            net_numbers.to_vec()
        } else {
            Vec::new()
        };

        // :629-689.
        let line_count = polyline.lines().len();
        for i in 0..line_count.saturating_sub(2) {
            let Some(current_shape) = polyline.offset_shape(half_width, i) else {
                continue;
            };
            let obstacles = board.overlapping_items_with_clearance(
                &current_shape,
                Some(layer),
                &check_net_no_arr,
                clearance_class_index,
            );
            for current_item in obstacles {
                let Some(item) = board.get_item(current_item) else {
                    continue;
                };
                // :635-664.
                let is_obstacle = if item.shares_net_no(net_numbers) {
                    // "to avoid acid traps"
                    matches!(item, Item::Pin(_))
                        && contact_pins.is_some_and(|pins| !pins.contains(&current_item))
                } else if let Item::ConductionArea(area) = item {
                    area.get_is_obstacle()
                } else if matches!(
                    item,
                    Item::ViaObstacleArea(_) | Item::ComponentObstacleArea(_)
                ) {
                    false
                } else if item.is_trace() {
                    if item.is_shove_fixed(&board.rules) {
                        // ":650. check for a shove fixed trace exit stub, which has to be ignored
                        // at a tie pin."
                        !board.normal_contacts(current_item).into_iter().any(|c| {
                            board
                                .get_item(c)
                                .is_some_and(|contact| contact.shares_net_no(net_numbers))
                        })
                    } else {
                        // "an unfixed trace can be pushed aside eventually"
                        false
                    }
                } else {
                    // "an unfixed via can be pushed aside eventually"
                    !item.is_routable()
                };

                // :666-684.
                if is_obstacle {
                    let ctx = board.ctx();
                    let current_item_bounding_box =
                        board.get_item(current_item).map(|it| it.bounding_box(&ctx));
                    match found_obstacle {
                        None => {
                            found_obstacle = Some(current_item);
                            found_obstacle_bounding_box = current_item_bounding_box;
                        }
                        Some(found) if found != current_item => {
                            // ":671-673. check, if 1 obstacle is contained in the other obstacle
                            // and take the bigger obstacle in this case. That may happen in case
                            // of fixed vias inside of pins."
                            //
                            // totalized: `TraceShover.springOver`'s `foundObstacleBoundingBox` /
                            // `currentItem.boundingBox()` (`:674-675`) -> a skipped candidate.
                            // Neither is null in Java: `:669` set the first alongside
                            // `foundObstacle`, and `Item.boundingBox` always answers a box. The
                            // port's `Option`s are `None` only for an id the board has dropped,
                            // which the query one line up cannot return. Unreachable — no row.
                            let (Some(found_box), Some(current_box)) =
                                (found_obstacle_bounding_box, current_item_bounding_box)
                            else {
                                continue;
                            };
                            if found_box.intersects(&current_box) {
                                if current_box.contains(&found_box) {
                                    found_obstacle = Some(current_item);
                                    found_obstacle_bounding_box = Some(current_box);
                                } else if !found_box.contains(&current_box) {
                                    return None;
                                }
                            }
                        }
                        Some(_) => {}
                    }
                }
            }
            // :686-688.
            if found_obstacle.is_some() {
                break;
            }
        }
        // :690-693. "no obstacle in the way, nothing to do"
        let Some(found_obstacle) = found_obstacle else {
            return Some(SpringOverOutcome::Unchanged);
        };

        // :695-700.
        let obstacle_is_outline_or_unfixed_trace = {
            let is_trace = board.get_item(found_obstacle).is_some_and(Item::is_trace);
            let is_shove_fixed = board
                .get_item(found_obstacle)
                .is_some_and(|it| it.is_shove_fixed(&board.rules));
            // `foundObstacle instanceof BoardOutline` — the port's outline is an item id the
            // board knows by name.
            let is_outline = board.get_outline() == Some(found_obstacle);
            is_outline || (is_trace && !is_shove_fixed)
        };
        if recursion_depth <= 0 || obstacle_is_outline_or_unfixed_trace {
            board.set_shove_failing_obstacle(Some(found_obstacle));
            return None;
        }

        // :701-711.
        let mut try_spring_over = true;
        if !over_connected_pins {
            // "Check if the obstacle has a trace contact on layer"
            for current_contact in board.all_contacts_on_layer(found_obstacle, layer) {
                if board.get_item(current_contact).is_some_and(Item::is_trace) {
                    try_spring_over = false;
                    break;
                }
            }
        }
        // :712-723.
        let tree = board.trees.get_default_tree().id();
        let mut obstacle_shape: Option<TileShape> = None;
        if try_spring_over {
            let kind = board
                .get_item(found_obstacle)
                .map(|it| (it.is_obstacle_area() || it.is_trace(), it.is_drill_item()));
            match kind {
                Some((true, _)) => {
                    if board.item_tree_shape_count(found_obstacle, tree) == 1 {
                        obstacle_shape = board.item_tree_shape(found_obstacle, tree, 0);
                    } else {
                        try_spring_over = false;
                    }
                }
                Some((false, true)) => {
                    obstacle_shape = tree_shape_on_layer(board, found_obstacle, tree, layer);
                }
                _ => {}
            }
        }
        // :724-727.
        if !try_spring_over {
            board.set_shove_failing_obstacle(Some(found_obstacle));
            return None;
        }
        // totalized: `TraceShover.springOver`'s `:731`/`:739` dereference of a null
        // `obstacleShape` -> a refusal. Java leaves `obstacleShape` null when `foundObstacle` is
        // neither an `ObstacleArea`/`Trace` nor a `DrillItem` (`:713-723` has no `else`), or when
        // a `DrillItem`'s `getTreeShapeOnLayer` answers null, and then throws a
        // `NullPointerException` one line later. Unreachable in practice: `:632` found the
        // obstacle through `overlappingItemsWithClearance(currentShape, layer, …)`, so it *has* a
        // tree shape on `layer`, and `:642-663` admit nothing outside those two families except a
        // `BoardOutline`, which `:696` has already rejected. No register row, per
        // `docs/java-quirks.md` §Process notes.
        let Some(obstacle_shape) = obstacle_shape else {
            board.set_shove_failing_obstacle(Some(found_obstacle));
            return None;
        };

        // :728-741.
        let compensation_used = board
            .trees
            .get_default_tree()
            .is_clearance_compensation_used();
        let offset = f64::from(half_width + 1);
        let mut offset_shape = if compensation_used {
            obstacle_shape.enlarge(offset)
        } else {
            // "enlarge the shape in 2 steps for symmetry reasons"
            let obstacle_clearance_class = board
                .get_item(found_obstacle)
                .map_or(0, Item::clearance_class);
            let half_cl_offset = 0.5
                * f64::from(board.clearance_value(
                    obstacle_clearance_class,
                    clearance_class_index,
                    layer,
                ));
            obstacle_shape
                .enlarge(offset + half_cl_offset)
                .enlarge(half_cl_offset)
        };
        // :742-746.
        match board.rules.trace_angle_restriction {
            AngleRestriction::NinetyDegree => {
                offset_shape = TileShape::Box(offset_shape.bounding_box());
            }
            AngleRestriction::FortyFiveDegree => {
                // totalized: `TileShape.boundingOctagon` (`:745`) is non-null in Java for every
                // non-empty shape; the port's `Option` is `None` only for an empty one, which
                // `enlarge` above cannot produce from a live obstacle. `?` refuses rather than
                // panicking. Unreachable — no register row.
                offset_shape = TileShape::Octagon(offset_shape.bounding_octagon()?);
            }
            AngleRestriction::None => {}
        }

        // :748-754. "can happen with clearance compensation off because of asymmetry in
        // calculations with the offset shapes"
        let first_corner = polyline.first_corner();
        let last_corner = polyline.last_corner();
        let touches_an_end = first_corner.is_some_and(|c| offset_shape.contains_inside(&c))
            || last_corner.is_some_and(|c| offset_shape.contains_inside(&c));
        if touches_an_end {
            board.set_shove_failing_obstacle(Some(found_obstacle));
            return None;
        }

        // :755-763.
        let entries = offset_shape.entrance_points(&polyline);
        if entries.is_empty() {
            // "no obstacle"
            return Some(SpringOverOutcome::Unchanged);
        }
        if entries.len() < 2 {
            board.set_shove_failing_obstacle(Some(found_obstacle));
            return None;
        }

        // :764-782.
        let first_intersection_side_no = entries[0][1];
        let last_intersection_side_no = entries[entries.len() - 1][1];
        let first_intersection_line_no = entries[0][0];
        let last_intersection_line_no = entries[entries.len() - 1][0];
        let border_line_count = offset_shape.border_line_count();
        let mut side_diff = last_intersection_side_no as i64 - first_intersection_side_no as i64;
        if side_diff < 0 {
            side_diff += border_line_count as i64;
        } else if side_diff == 0 {
            let compare_corner = offset_shape.corner_approx(first_intersection_side_no)?;
            let (Some(first_border), Some(last_border)) = (
                offset_shape.border_line(first_intersection_side_no),
                offset_shape.border_line(last_intersection_side_no),
            ) else {
                return None;
            };
            let lines = polyline.lines();
            let first_intersection =
                lines[first_intersection_line_no].intersection_approx(&first_border);
            let second_intersection =
                lines[last_intersection_line_no].intersection_approx(&last_border);
            if compare_corner.distance(&second_intersection)
                < compare_corner.distance(&first_intersection)
            {
                side_diff += border_line_count as i64;
            }
        }

        // :783-796.
        let side_diff = side_diff as usize;
        let mut substitute_lines: Vec<Line> = Vec::with_capacity(side_diff + 3);
        substitute_lines.push(polyline.lines()[first_intersection_line_no]);
        let mut current_edge_line_no = first_intersection_side_no;
        for _ in 1..=side_diff + 1 {
            substitute_lines.push(offset_shape.border_line(current_edge_line_no)?);
            if current_edge_line_no == border_line_count - 1 {
                current_edge_line_no = 0;
            } else {
                current_edge_line_no += 1;
            }
        }
        substitute_lines.push(polyline.lines()[last_intersection_line_no]);
        // totalized: `TraceShover.springOver`'s `new Polyline(substituteLines)` (`:796`) -> a
        // refusal. Java's constructor throws only on fewer than three lines or a null entry;
        // `sideDiff >= 0` makes the array at least three long and every entry was just written.
        // Unreachable — no register row.
        let Ok(substitute_polyline) = Polyline::from_lines(substitute_lines) else {
            return None;
        };

        // :797-808. "build a circuit around the offsetShape in counter clock sense from the first
        // intersection point to the second intersection point"
        let pieces = offset_shape.cutout_polyline(&polyline).unwrap_or_default();
        let mut result = substitute_polyline;
        if let Some(first_piece) = pieces.first() {
            result = first_piece.combine(&result).ok()?;
        }
        if let Some(second_piece) = pieces.get(1) {
            result = result.combine(second_piece).ok()?;
        }

        // :809-817. The recursion returns the *new* polyline when it finds nothing more to do, so
        // an inner `Unchanged` is this frame's `Changed(result)`.
        match Self::spring_over(
            board,
            result.clone(),
            half_width,
            layer,
            net_numbers,
            clearance_class_index,
            over_connected_pins,
            recursion_depth - 1,
            contact_pins,
        ) {
            None => None,
            Some(SpringOverOutcome::Unchanged) => Some(SpringOverOutcome::Changed(result)),
            Some(changed) => Some(changed),
        }
    }
}

// =================================================================================================
// The deferral roster for `board/optimize/TraceShover.java`
// =================================================================================================
//
// The mutating half. `insert` shoves for real and, per its own javadoc, "if the shove does not
// work, the database may be damaged" — which is exactly why Plan 6 needs only `check`.
// added in Plan 7: `TraceShover.insert` (TraceShover.java:417-591) — the mutating twin of the instance `check`.
// added in Plan 7: `TraceShover.springOverObstacles` (TraceShover.java:827-874) — the public wrapper that runs the private `springOver` in both senses and keeps the shorter result. Plan 6 reaches `springOver` only through the instance `check` (`:367-387`), which is why `spring_over` above is `pub(crate)` and this wrapper is not ported yet.
