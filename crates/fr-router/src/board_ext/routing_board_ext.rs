//! [`RoutingBoardExt`]: the `RoutingBoard` methods `fr-board` deliberately left out.

use fr_board::prelude::*;
use fr_board::{ItemId, RoomId, TimeLimit};
use fr_geometry::{Polyline, TileShape};

use crate::autoroute::maze::engine::AutorouteEngine;
use crate::board_ext::drill_item_mover::tree_by_id;
use crate::board_ext::trace_shover::TraceShover;

/// The `RoutingBoard` methods `fr-board` deliberately left out (plan-2 ruling 4, because each one
/// needs an `AutorouteEngine` and `fr-board` cannot name one).
///
/// Created in Plan 6 and **shared with Plan 7** (plan-6 ruling 3), which adds `opt_changed_area`,
/// the pull-tight entry points and the tighteners.
///
/// # The engine is a value, not a field
///
/// Java's `RoutingBoard` owns `private transient AutorouteEngine autorouteEngine`
/// (RoutingBoard.java:70) and every method here reads it. The port cannot: `AutorouteEngine`
/// lives in this crate and `Board` in `fr-board`, and the back-pointer would be a cycle across
/// the crate boundary. So the engine is passed in and handed back —
/// [`init_autoroute`](RoutingBoardExt::init_autoroute) takes `Option<AutorouteEngine>` where Java
/// reads the field and returns the engine where Java stores it, and
/// [`finish_autoroute`](RoutingBoardExt::finish_autoroute) consumes it where Java nulls the
/// field.
///
/// One consequence is recorded rather than hidden: Java's `additionalUpdateAfterChange` returns
/// at once while `board.autorouteEngine == null` (`:100`), and the field is set **only** by
/// `initAutoroute` (`:892`). The port has no field to test, so
/// [`additional_update_after_change`](RoutingBoardExt::additional_update_after_change) always
/// runs — which equals Java on every production path, because `initAutoroute` is the only
/// non-GUI caller of the `AutorouteEngine` constructor.
pub trait RoutingBoardExt {
    /// Port of `initAutoroute(int, int, Stoppable, TimeLimit, boolean)`
    /// (RoutingBoard.java:882-897): "initialises the auto-route database for routing a
    /// connection. If `retainAutorouteDatabase`, the auto-route database is retained and
    /// maintained after the algorithm for performance reasons."
    ///
    /// Reuses `engine` only when it exists, `retain` is set **and** the compensated clearance
    /// class of its tree matches `trace_clearance_class` (`:888-891`); otherwise a fresh engine
    /// is built. Either way `AutorouteEngine::init_connection` runs (`:895`).
    ///
    /// The `Stoppable` argument has no counterpart: plan-6 ruling 6 makes cancellation a
    /// per-call `StopCheck` rather than engine state (see `autoroute::maze::engine`'s module
    /// docs).
    fn init_autoroute(
        &mut self,
        engine: Option<AutorouteEngine>,
        net_number: i32,
        trace_clearance_class_index: usize,
        time_limit: Option<TimeLimit>,
        retain_autoroute_database: bool,
    ) -> AutorouteEngine;

    /// Port of `finishAutoroute()` (RoutingBoard.java:899-905): "clears the auto-route database
    /// in case it was retained."
    ///
    /// Java's `clear()` followed by `autorouteEngine = null` is `clear` followed by dropping the
    /// value the caller hands over.
    fn finish_autoroute(&mut self, engine: AutorouteEngine);

    /// Port of `additionalUpdateAfterChange(Item)` (RoutingBoard.java:96-118): "maintains the
    /// auto-router database after item is inserted, changed, or deleted" — invalidate the drill
    /// pages of every tree shape of the item, remove every complete free-space expansion room
    /// those shapes overlap, and clear the item's autoroute scratch.
    fn additional_update_after_change(&mut self, engine: &mut AutorouteEngine, item: ItemId);

    /// Port of `clearAllItemTemporaryAutorouteData()` (RoutingBoard.java:1240-1249).
    ///
    /// `fr-board` already ports this one as the inherent
    /// [`Board::clear_all_item_temporary_autoroute_data`] (it needs no engine), so this is a
    /// delegating wrapper; it is on the trait because plan-6 ruling 3 names it as one of the five
    /// methods `RoutingBoardExt` presents, and a caller working through the trait should not have
    /// to know which of the five happened to be expressible in `fr-board`.
    fn clear_all_item_temporary_autoroute_data(&mut self);

    /// Port of `checkForcedTracePolyline(Polyline, int, int, int[], int, int, int, int)`
    /// (RoutingBoard.java:405-448): "checks, if a trace polyline with the input parameters can be
    /// inserted while shoving aside obstacle traces and vias." Reached from
    /// `MazeSearchEngine.java:681`.
    ///
    /// Note that it queries the **default** tree (`:418`), not the engine's compensated autoroute
    /// tree, and adds that tree's clearance compensation value to the half width unconditionally
    /// — `TraceShover::check_segment` adds it only when compensation is in use (`:68-70`), which
    /// is not a contradiction because `clearanceCompensationValue` answers 0 when it is not.
    ///
    /// # Panics
    ///
    /// Panics when any of the polyline's offset shapes overlaps a **shovable foreign-net via**:
    /// `TraceShover::check` then reaches [`crate::board_ext::DrillItemMover::check`], whose main
    /// arm is plan-6 Task 10's `ForcedPadRouter.checkForcedPad` (the `added in Task 10:` marker
    /// in `board_ext/drill_item_mover.rs`). Everything else — trace, pin and area obstacles, the
    /// whole substitute-trace recursion, the spring-over — answers normally. This method's first
    /// production caller is `MazeSearchEngine.java:681`, which is Task 13's, so Task 10 must land
    /// first.
    #[allow(clippy::too_many_arguments)]
    fn check_forced_trace_polyline(
        &mut self,
        polyline: &Polyline,
        half_width: i32,
        layer: usize,
        net_numbers: &[i32],
        clearance_class_index: usize,
        max_recursion_depth: i32,
        max_via_recursion_depth: i32,
        max_spring_over_recursion_depth: i32,
    ) -> bool;
}

impl RoutingBoardExt for Board {
    fn init_autoroute(
        &mut self,
        engine: Option<AutorouteEngine>,
        net_number: i32,
        trace_clearance_class_index: usize,
        time_limit: Option<TimeLimit>,
        retain_autoroute_database: bool,
    ) -> AutorouteEngine {
        // RoutingBoard.java:888-894.
        let reusable = engine.filter(|existing| {
            retain_autoroute_database
                && tree_by_id(self, existing.tree).compensated_clearance_class()
                    == trace_clearance_class_index
        });
        let mut engine = match reusable {
            Some(existing) => existing,
            None => {
                AutorouteEngine::new(self, trace_clearance_class_index, retain_autoroute_database)
            }
        };
        // :895.
        engine.init_connection(self, net_number, time_limit);
        // :896.
        engine
    }

    fn finish_autoroute(&mut self, mut engine: AutorouteEngine) {
        // RoutingBoard.java:901-903.
        engine.clear(self);
        // :904 — `this.autorouteEngine = null` is the drop.
        drop(engine);
    }

    fn additional_update_after_change(&mut self, engine: &mut AutorouteEngine, item: ItemId) {
        // RoutingBoard.java:97-99. Java's `item == null` guard is an id the board does not know.
        if self.get_item(item).is_none() {
            return;
        }
        // :100-102. The `autorouteEngine == null` half of Java's guard has no counterpart — see
        // the trait doc.
        if !engine.maintain_database {
            return;
        }
        let tree = engine.tree;
        // :104-105. "Invalidate the free space expansion rooms touching a shape of item."
        let shape_count = self.item_tree_shape_count(item, tree);
        for i in 0..shape_count {
            // :106.
            let Some(current_shape) = self.item_tree_shape(item, tree, i) else {
                continue;
            };
            // :107.
            engine.invalidate_drill_pages(&current_shape);
            // :108.
            let current_layer = {
                let ctx = self.ctx();
                let Some(current_item) = self.get_item(item) else {
                    continue;
                };
                current_item.shape_layer(i, &ctx)
            };
            // :109-111. The third live mixed room/item set: the autoroute tree holds complete
            // rooms as well as items, so this walks a `BTreeSet<TreeObject>` and keeps the rooms.
            // `TreeObject`'s `Ord` is Java's `SearchTreeObject` order (rooms first, then items by
            // descending id), so the removal order below is Java's.
            let rooms_to_remove: Vec<RoomId> = {
                let ctx = self.ctx();
                tree_by_id(self, tree)
                    .overlapping_objects_with_rooms(
                        &current_shape,
                        Some(current_layer),
                        &[],
                        &self.items,
                        &engine.rooms,
                        &ctx,
                    )
                    .into_iter()
                    .filter_map(|object| match object {
                        TreeObject::Room(room) => Some(room),
                        TreeObject::Item(_) => None,
                    })
                    .collect()
            };
            // :112-116.
            for room in rooms_to_remove {
                engine.remove_complete_expansion_room(self, room);
            }
        }
        // :117.
        if let Some(current_item) = self.items.get_mut(&item) {
            current_item.clear_autoroute_info();
        }
    }

    fn clear_all_item_temporary_autoroute_data(&mut self) {
        Board::clear_all_item_temporary_autoroute_data(self);
    }

    fn check_forced_trace_polyline(
        &mut self,
        polyline: &Polyline,
        half_width: i32,
        layer: usize,
        net_numbers: &[i32],
        clearance_class_index: usize,
        max_recursion_depth: i32,
        max_via_recursion_depth: i32,
        max_spring_over_recursion_depth: i32,
    ) -> bool {
        // RoutingBoard.java:418-420. The **default** tree.
        let compensated_half_width = half_width
            + self.trees.get_default_tree().clearance_compensation_value(
                clearance_class_index,
                layer,
                &self.rules,
            );
        // :421-422.
        let line_count = polyline.lines().len();
        // totalized: `RoutingBoard.checkForcedTracePolyline`'s `polyline.lines.length - 1`
        // (`:422`) underflows for an empty polyline, where Java throws a
        // `NegativeArraySizeException` inside `offsetShapes`. `Polyline`'s
        // constructor guarantees at least three lines, so this is unreachable; answering `true`
        // (no shape to check, nothing refused) rather than panicking. No register row.
        if line_count == 0 {
            return true;
        }
        let trace_shapes =
            polyline.offset_shapes_between(compensated_half_width, 0, line_count - 1);
        // :423.
        let orthogonal_mode = self.rules.trace_angle_restriction == AngleRestriction::NinetyDegree;
        // :425-445.
        for (i, trace_shape) in trace_shapes.iter().enumerate() {
            // :427-430.
            let current_trace_shape = if orthogonal_mode {
                TileShape::Box(trace_shape.bounding_box())
            } else {
                trace_shape.clone()
            };
            // :431.
            let from_side = ShapeEntrySide::from_polyline(polyline, i + 1, &current_trace_shape);
            // :433-443.
            let check_shove_ok = TraceShover::check(
                self,
                &current_trace_shape,
                Some(&from_side),
                None,
                layer,
                net_numbers,
                clearance_class_index,
                max_recursion_depth,
                max_via_recursion_depth,
                max_spring_over_recursion_depth,
                None,
            );
            if !check_shove_ok {
                return false;
            }
        }
        // :446.
        true
    }
}
