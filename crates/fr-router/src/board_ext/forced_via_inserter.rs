//! Port of the check half of `board.actions.ForcedViaInserter`
//! (`board/actions/ForcedViaInserter.java`).

use fr_board::PadstackId;
use fr_board::prelude::*;
use fr_board::rules::ViaInfo;
use fr_geometry::{Circle, FloatPoint, Point, Shape, ShapeOps, Simplex, TileShape, limits};

use crate::board_ext::forced_pad_router::{CheckDrillResult, ForcedPadRouter};

/// Port of `board.actions.ForcedViaInserter` (ForcedViaInserter.java:22-462) — the check half.
///
/// Java's class is `final` with a private constructor and nothing but static methods; the port is
/// a unit struct with associated functions and Java's `RoutingBoard board` parameter promoted to
/// the leading `&mut Board`, matching [`crate::board_ext::DrillItemMover`].
///
/// # This is the check half
///
/// `checkLayer` (`:30-129`) and `check` (`:131-247`) are here; `insert` (`:249-361`) is
/// `// added in Plan 7:` in the roster at the bottom of this file, because its per-layer body is
/// three calls to `ForcedPadRouter.forcedPad`, which reaches `TraceShover.insert` and
/// `DrillItemMover.shoveVias` — plan-6 ruling 2's "`board/optimize/**`'s mutating half", i.e.
/// Plan 7's. See `task-10-report.md` §2.1: this is a **contradiction in the task brief**, which
/// asked for `insert` here while also declaring `forcedPad` Plan 7's, and Java decides it.
pub struct ForcedViaInserter;

impl ForcedViaInserter {
    /// Port of `ForcedViaInserter.checkLayer(double, int, boolean, TileShape, Point, int, int[],
    /// int, int, RoutingBoard, int, int)` (ForcedViaInserter.java:30-129): "checks, if a Via is
    /// possible at the input layer after evtl. shoving aside obstacle traces. `roomShape` is used
    /// for calculating the `fromSide`."
    ///
    /// A two-phase gate: `checkForcedPad` for the **via** shape (`:74-90`), then a second for the
    /// **trace** shape (`:96-119`), with `NOT_DRILLABLE` short-circuiting either phase and
    /// `DRILLABLE_WITH_ATTACH_SMD` from **either** winning (`:120-124`). Note the second phase
    /// hard-codes `copperSharingAllowed = true` (`:111`) where the first passes the caller's
    /// `attachSmdAllowed`, so the promotion can come from the trace phase even when the via phase
    /// was not allowed to share copper.
    ///
    /// Reached from `MazeExpansionEngine.java:391`, which is Task 13's.
    #[allow(clippy::too_many_arguments)]
    pub fn check_layer(
        board: &mut Board,
        via_radius: f64,
        clearance_class_index: usize,
        attach_smd_allowed: bool,
        room_shape: &TileShape,
        location: &Point,
        layer: usize,
        net_numbers: &[i32],
        max_recursion_depth: i32,
        max_via_recursion_depth: i32,
        trace_half_width: i32,
        trace_clearance_class: usize,
    ) -> CheckDrillResult {
        // :43-45.
        if via_radius <= 0.0 {
            return CheckDrillResult::Drillable;
        }
        // :46-49.
        let Point::Int(int_location) = location else {
            return CheckDrillResult::NotDrillable;
        };
        // :50.
        let via_shape = Circle::new(*int_location, ceil_to_i32(via_radius));

        // :52-55.
        let check_radius = via_radius
            + 0.5
                * f64::from(board.clearance_value(
                    clearance_class_index,
                    clearance_class_index,
                    layer,
                ))
            + f64::from(board.get_min_trace_half_width());

        // :57-65.
        let is_90_degree = board.rules.trace_angle_restriction == AngleRestriction::NinetyDegree;
        let tile_shape = if is_90_degree {
            TileShape::Box(via_shape.bounding_box())
        } else {
            TileShape::Octagon(via_shape.bounding_octagon())
        };

        // :67-72.
        let Some(from_side) = Self::calculate_from_side(
            &int_location.to_float(),
            &tile_shape,
            &room_shape.to_simplex(),
            check_radius,
            is_90_degree,
        ) else {
            return CheckDrillResult::NotDrillable;
        };

        // :74-90.
        let via_result = ForcedPadRouter::check_forced_pad(
            board,
            &tile_shape,
            &from_side,
            layer,
            net_numbers,
            clearance_class_index,
            attach_smd_allowed,
            None,
            max_recursion_depth,
            max_via_recursion_depth,
            false,
            None,
        );
        if via_result == CheckDrillResult::NotDrillable {
            return via_result;
        }

        // :92-94.
        if trace_half_width <= 0 {
            return via_result;
        }

        // :96-102.
        let start_trace_circle = Circle::new(*int_location, trace_half_width);
        let start_trace_shape = if is_90_degree {
            TileShape::Box(start_trace_circle.bounding_box())
        } else {
            TileShape::Octagon(start_trace_circle.bounding_octagon())
        };

        // :104-119. Note `:111`'s hard-coded `true` where the via phase passed
        // `attachSmdAllowed`.
        let trace_result = ForcedPadRouter::check_forced_pad(
            board,
            &start_trace_shape,
            &from_side,
            layer,
            net_numbers,
            trace_clearance_class,
            true,
            None,
            max_recursion_depth,
            max_via_recursion_depth,
            false,
            None,
        );
        if trace_result == CheckDrillResult::NotDrillable {
            return trace_result;
        }
        // :120-124.
        if via_result == CheckDrillResult::DrillableWithAttachSmd
            || trace_result == CheckDrillResult::DrillableWithAttachSmd
        {
            return CheckDrillResult::DrillableWithAttachSmd;
        }
        CheckDrillResult::Drillable
    }

    /// Port of `ForcedViaInserter.check(ViaInfo, Point, int[], int, int, RoutingBoard, int[],
    /// int)` (ForcedViaInserter.java:131-247): "checks, if a Via is possible with the input
    /// parameter after evtl. shoving aside obstacle traces."
    ///
    /// Up to three `checkForcedPad` calls per padstack layer: the pad itself (`:167-182`), the
    /// drill hole where the layer's pad exists but keeps a smaller copper clearance (`:183-208`,
    /// a HEAD-only addition) and the start-trace circle where `tracePenHalfwidthArr` asks for one
    /// (`:210-237`). Each refusal records the layer in `board.shoveFailingLayer` and returns
    /// `false`.
    ///
    /// Reached from `FoundConnectionInserter.java:708`, which is Task 15's.
    #[allow(clippy::too_many_arguments)]
    pub fn check(
        board: &mut Board,
        via_info: &ViaInfo,
        location: &Point,
        net_numbers: &[i32],
        max_recursion_depth: i32,
        max_via_recursion_depth: i32,
        trace_pen_halfwidth_arr: Option<&[i32]>,
        trace_clearance_class_index: usize,
    ) -> bool {
        // :140-144.
        let translate_vector = location.difference_by(&Point::ZERO);
        let calc_from_side_offset = board.get_min_trace_half_width();
        let via_padstack = via_info.get_padstack();
        let hole_shape = Self::hole_check_shape(board, via_padstack, location);
        let attach_smd_allowed = via_info.attach_smd_allowed();
        let via_clearance_class = via_info.get_clearance_class_index();
        let is_90_degree = board.rules.trace_angle_restriction == AngleRestriction::NinetyDegree;
        let Some((from_layer, to_layer)) = padstack_shape_layer_range(board, via_padstack) else {
            return true;
        };

        // :145-238.
        for i in from_layer..=to_layer {
            // :146-157.
            let padstack_shape = board
                .library
                .padstacks
                .get(via_padstack)
                .and_then(|padstack| padstack.get_shape(i))
                .cloned();
            let (current_pad_shape, current_clearance_class_index) = match padstack_shape {
                None => match &hole_shape {
                    // :149-151.
                    None => continue,
                    // :152-154. "The drill hole itself must keep hole clearance from copper on
                    // this layer."
                    Some(hole) => (hole.clone(), 0usize),
                },
                // :155-157.
                // `Shape::translate_by` answers a `Shape`, so Java's `(Shape)` cast at `:156`
                // — which would throw for a multi-piece area — has no counterpart here.
                Some(shape) => (shape.translate_by(&translate_vector), via_clearance_class),
            };
            let layer = i as usize;
            // :158-163.
            let Some(tile_shape) = bounding_tile(&current_pad_shape, is_90_degree) else {
                continue;
            };
            // :164-166.
            let from_side = ForcedPadRouter::calc_from_side(
                board,
                &tile_shape,
                location,
                layer,
                calc_from_side_offset,
                current_clearance_class_index,
            );
            // :167-182.
            if ForcedPadRouter::check_forced_pad(
                board,
                &tile_shape,
                &from_side,
                layer,
                net_numbers,
                current_clearance_class_index,
                attach_smd_allowed,
                None,
                max_recursion_depth,
                max_via_recursion_depth,
                false,
                None,
            ) == CheckDrillResult::NotDrillable
            {
                board.set_shove_failing_layer(i);
                return false;
            }
            // :183-208. "The drill hole must ALSO keep hole clearance from other-net copper on
            // layers where the pad exists — the pad check above only enforces the (smaller)
            // copper clearance."
            if current_clearance_class_index != 0
                && let Some(hole) = &hole_shape
                && let Some(hole_tile) = bounding_tile(hole, is_90_degree)
                && ForcedPadRouter::check_forced_pad(
                    board,
                    &hole_tile,
                    &from_side,
                    layer,
                    net_numbers,
                    0,
                    attach_smd_allowed,
                    None,
                    max_recursion_depth,
                    max_via_recursion_depth,
                    false,
                    None,
                ) == CheckDrillResult::NotDrillable
            {
                board.set_shove_failing_layer(i);
                return false;
            }

            // :210-237.
            let pen_half_width = trace_pen_halfwidth_arr
                .and_then(|arr| {
                    usize::try_from(i)
                        .ok()
                        .and_then(|idx| arr.get(idx))
                        .copied()
                })
                .unwrap_or(0);
            if pen_half_width > 0
                && let Point::Int(trace_point) = location
            {
                let start_trace_circle = Circle::new(*trace_point, pen_half_width);
                let start_trace_shape = if is_90_degree {
                    TileShape::Box(start_trace_circle.bounding_box())
                } else {
                    TileShape::Octagon(start_trace_circle.bounding_octagon())
                };
                if ForcedPadRouter::check_forced_pad(
                    board,
                    &start_trace_shape,
                    &from_side,
                    layer,
                    net_numbers,
                    trace_clearance_class_index,
                    true,
                    None,
                    max_recursion_depth,
                    max_via_recursion_depth,
                    false,
                    None,
                ) == CheckDrillResult::NotDrillable
                {
                    board.set_shove_failing_layer(i);
                    return false;
                }
            }
        }
        // :239.
        true
    }

    /// Port of the private `ForcedViaInserter.holeCheckShape(Padstack, Point, RoutingBoard)`
    /// (ForcedViaInserter.java:363-375): "hole-clearance substitute shape for a copper-less layer
    /// of a via padstack: the drill still passes through, so other copper must stay
    /// `holeClearance` away from it. Returns null when the rule is off or no drill radius is
    /// known."
    ///
    /// The `+ 10` at `:374` is Java's, verbatim; the comment there explains the inflation but not
    /// the constant.
    ///
    /// `pub` where Java is private, so `tests/forced_via.rs` — an integration test, i.e. a
    /// separate crate — can pin it against the JVM.
    pub fn hole_check_shape(
        board: &Board,
        padstack: PadstackId,
        location: &Point,
    ) -> Option<Shape> {
        // :364-367.
        let hole_clearance = board.rules.get_hole_clearance();
        let Point::Int(center) = location else {
            return None;
        };
        if hole_clearance <= 0 {
            return None;
        }
        // :368-371.
        let drill_radius = board.library.padstacks.get(padstack)?.drill_radius();
        if drill_radius <= 0.0 {
            return None;
        }
        // :372-374.
        Some(Shape::Circle(Circle::new(
            *center,
            ceil_to_i32(drill_radius + f64::from(hole_clearance) + 10.0),
        )))
    }

    /// Port of the private `ForcedViaInserter.calculateFromSide(FloatPoint, TileShape, Simplex,
    /// double, boolean)` (ForcedViaInserter.java:377-461): the first of the four orthogonal
    /// directions (`:384-420`) whose probe point at distance `dist` is still inside `roomShape`,
    /// then — in the any-angle regime only — the four diagonals (`:424-459`).
    ///
    /// `None` where Java returns `null`; the two `null` arms are `:421-423` (90-degree mode, no
    /// orthogonal side worked) and `:460` (no side at all). Unlike
    /// [`ForcedPadRouter::calc_from_side`] this one always fills the border intersection in.
    ///
    /// `pub` where Java is private, for the same reason as [`Self::hole_check_shape`].
    pub fn calculate_from_side(
        via_location: &FloatPoint,
        via_shape: &TileShape,
        room_shape: &Simplex,
        dist: f64,
        is_90_degree: bool,
    ) -> Option<ShapeEntrySide> {
        // :383.
        let via_box = via_shape.bounding_box();
        // :384-420.
        for i in 0..4 {
            let (check_point, border_x, border_y) = match i {
                0 => (
                    FloatPoint::new(via_location.x, via_location.y - dist),
                    via_location.x,
                    f64::from(via_box.ll.y),
                ),
                1 => (
                    FloatPoint::new(via_location.x + dist, via_location.y),
                    f64::from(via_box.ur.x),
                    via_location.y,
                ),
                2 => (
                    FloatPoint::new(via_location.x, via_location.y + dist),
                    via_location.x,
                    f64::from(via_box.ur.y),
                ),
                _ => (
                    FloatPoint::new(via_location.x - dist, via_location.y),
                    f64::from(via_box.ll.x),
                    via_location.y,
                ),
            };
            if TileShape::Simplex(room_shape.clone()).contains_float(&check_point) {
                // :411-418.
                let from_side_index = if is_90_degree { i } else { 2 * i };
                return Some(ShapeEntrySide::new(
                    from_side_index,
                    Some(FloatPoint::new(border_x, border_y)),
                ));
            }
        }
        // :421-423.
        if is_90_degree {
            return None;
        }
        // :424-426. "try the diagonal directions"
        let dist = dist / limits::SQRT2;
        let border_dist = via_box.max_width() / (2.0 * limits::SQRT2);
        // :427-459.
        for i in 0..4 {
            let (check_point, border_x, border_y) = match i {
                0 => (
                    FloatPoint::new(via_location.x + dist, via_location.y - dist),
                    via_location.x + border_dist,
                    via_location.y - border_dist,
                ),
                1 => (
                    FloatPoint::new(via_location.x + dist, via_location.y + dist),
                    via_location.x + border_dist,
                    via_location.y + border_dist,
                ),
                2 => (
                    FloatPoint::new(via_location.x - dist, via_location.y + dist),
                    via_location.x - border_dist,
                    via_location.y + border_dist,
                ),
                _ => (
                    FloatPoint::new(via_location.x - dist, via_location.y - dist),
                    via_location.x - border_dist,
                    via_location.y - border_dist,
                ),
            };
            if TileShape::Simplex(room_shape.clone()).contains_float(&check_point) {
                // :455-457.
                return Some(ShapeEntrySide::new(
                    2 * i + 1,
                    Some(FloatPoint::new(border_x, border_y)),
                ));
            }
        }
        // :460.
        None
    }
}

/// `(int) Math.ceil(x)` — Java's narrowing cast, which saturates rather than wrapping and maps
/// NaN to 0 (JLS 5.1.3), exactly as Rust's `as` does since 1.45.
fn ceil_to_i32(x: f64) -> i32 {
    x.ceil() as i32
}

/// `Shape.boundingBox()` / `Shape.boundingOctagon()` chosen by the angle regime, as
/// `ForcedViaInserter` does at `:158-163`, `:186-191` and `:216-220`.
///
/// totalized: `ForcedViaInserter.check`'s `currentPadShape.boundingOctagon()` (`:162`) -> a
/// skipped layer. Java's `boundingOctagon` never answers null for a non-empty shape, and a
/// padstack shape translated by a vector cannot be empty. Unreachable — no register row.
fn bounding_tile(shape: &Shape, is_90_degree: bool) -> Option<TileShape> {
    if is_90_degree {
        Some(TileShape::Box(shape.bounding_box()))
    } else {
        shape.bounding_octagon().map(TileShape::Octagon)
    }
}

/// `viaPadstack.fromLayer() ..= viaPadstack.toLayer()` (`:145`), or `None` for a padstack with no
/// shape at all — where Java's loop runs zero times because `fromLayer() > toLayer()`.
fn padstack_shape_layer_range(board: &Board, padstack: PadstackId) -> Option<(i32, i32)> {
    let padstack = board.library.padstacks.get(padstack)?;
    let (from, to) = (padstack.from_layer(), padstack.to_layer());
    if from > to { None } else { Some((from, to)) }
}

// =================================================================================================
// The deferral roster for `board/actions/ForcedViaInserter.java`
// =================================================================================================
//
// added in Plan 7: `ForcedViaInserter.insert` (ForcedViaInserter.java:249-356) — three calls to `ForcedPadRouter.forcedPad` per padstack layer (`:297`, `:317`, `:333`) and then `BasicBoard.insertVia` (`:348`). `forcedPad` reaches `TraceShover.insert` and `DrillItemMover.shoveVias`, which plan-6 ruling 2 assigns to Plan 7, so `insert` cannot land before them. Its Plan 6 caller is `FoundConnectionInserter.java:754` (Task 15); see `task-10-report.md` §2.1 for the brief contradiction this resolves and the ruling-6 `StopCheck` that travels with it.
