//! [`RoutingBoardExt`]: the `RoutingBoard` methods `fr-board` deliberately left out.

use fr_board::datastructures::StopCheck;
use fr_board::items::Item;
use fr_board::prelude::*;
use fr_board::{BoardError, ItemId, RoomId, TimeLimit};
use fr_geometry::{IntOctagon, Point, Polyline, TileShape};

use crate::autoroute::maze::engine::AutorouteEngine;
use crate::board_ext::drill_item_mover::tree_by_id;
use crate::board_ext::tightener::{PolylineTraceExt, TraceTightener};
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
    /// An offset shape that overlaps a **shovable foreign-net via** sends `TraceShover::check`
    /// into [`crate::board_ext::DrillItemMover::check`] and
    /// [`crate::board_ext::ForcedPadRouter::check_forced_pad`], the mutual recursion plan-6
    /// Task 10 closed. This method's first production caller is `MazeSearchEngine.java:681`,
    /// which is Task 13's.
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

    /// Port of `insertForcedTracePolyline(Polyline, int, int, int[], int, int, int, int, int, int,
    /// boolean, TimeLimit)` (RoutingBoard.java:456-876): "tries to insert a trace polyline with
    /// the input parameters while shoving aside obstacle traces and vias. Returns the last corner
    /// on the polyline, to which the shove succeeded. Returns null, if the check was inaccurate
    /// and an error occurred while inserting, so that the database may be damaged and an undo
    /// necessary."
    ///
    /// `None` is Java's `null` — "the board may be damaged" — and is answered at exactly three
    /// places: the degenerate polyline of `:472-480`, and the two failed `TraceShover::insert`s
    /// of `:616-618` and `:742-745`. Every other refusal answers `Some(fromCorner)`, which the
    /// caller reads as "nothing was inserted past the start".
    ///
    /// # Two parameters where Java has one, and one Java does not have
    ///
    /// Java's single `TimeLimit timeLimit` is passed straight to `TraceShover.check` (`:584`,
    /// `:703`) and nowhere else, so it stays a `TimeLimit` here. The `StopCheck` beside it is
    /// plan-6 ruling 6's and plan-3 ruling F's: `TraceShover::insert` needs one, because it
    /// reaches `Board::split_traces_checked` and `Board::connection_items_checked`, the two
    /// `fr-board` walks that do not terminate on quirk #76's ladder board. Java has no
    /// cancellation on this path at all.
    ///
    /// `engine` is Java's `RoutingBoard.autorouteEngine` field (`:70`), threaded so the
    /// `PolylineTrace.change` inside the `:860-862` pull-tight tail can run
    /// `additionalUpdateAfterChange` — see [`crate::board_ext::tightener`]'s module docs.
    ///
    /// # `optChangedArea` is **not** reached from here
    ///
    /// `RoutingBoard`'s two `optChangedArea` overloads (`:151-190`) stay `// added in Plan 7:`.
    /// The method that pull-tightens a whole changed area after a shove is `forcedVia`
    /// (`:312-352`, its tail at `:348`) and `insertTrace`/`autoroute`/`fanout` (`:293`, `:962`,
    /// `:1101`) — **not** this one, whose tail (`:773-875`) builds its own `TraceTightener` and
    /// calls `splitTracesAtKeepPoint` plus a single `PolylineTrace.pullTight`. So controller
    /// ruling AB's conditional ("if the insertion tail calls `optChangedArea`, port only the
    /// branch reached") does not arise: there is no call to port or to mark.
    #[allow(clippy::too_many_arguments)]
    fn insert_forced_trace_polyline(
        &mut self,
        engine: Option<&mut AutorouteEngine>,
        polyline: &Polyline,
        half_width: i32,
        layer: usize,
        net_numbers: &[i32],
        clearance_class_index: usize,
        max_recursion_depth: i32,
        max_via_recursion_depth: i32,
        max_spring_over_recursion_depth: i32,
        tidy_width: i32,
        pull_tight_accuracy: i32,
        with_check: bool,
        time_limit: Option<&TimeLimit>,
        stop: StopCheck<'_>,
    ) -> Result<Option<Point>, BoardError>;

    /// Port of `insertForcedTraceSegment(Point, Point, int, int, int[], int, int, int, int, int,
    /// int, boolean, TimeLimit)` (RoutingBoard.java:361-402): "tries to insert a trace line with
    /// the input parameters from `fromCorner` to `toCorner` while shoving aside obstacle traces
    /// and vias. Returns the last point between `fromCorner` and `toCorner`, to which the shove
    /// succeeded. Returns null, if the check was inaccurate and an error occurred while
    /// inserting, so that the database may be damaged and an undo necessary."
    ///
    /// # Java's `==` on the returned corner, and why value equality is the same test
    ///
    /// `:394-400` compares the polyline's answer against `insertPolyline.firstCorner()` and
    /// `lastCorner()` by **reference**. That works in Java because `Polyline.corner` memoises
    /// (`Polyline.java:309-317`), so `insertForcedTracePolyline`'s own `fromCorner`/`toCorner`
    /// (`:470-471`) are the very objects those two accessors answer. The port's `Point` is a
    /// value type, so the test becomes `==` — and the two agree here, provably:
    ///
    /// * every `Some(fromCorner)` / `Some(toCorner)` return of `insertForcedTracePolyline` hands
    ///   back one of those two memoised objects, so identity and value both say yes;
    /// * the only other non-`None` answer is `newCorner`, which starts *as* `toCorner` (`:620`,
    ///   identity again) and is reassigned only at `:675`, from
    ///   `newPolyline.shorten(…, sampleWidth)`'s last corner under `lastSegmentLength >
    ///   sampleWidth` — a corner strictly nearer `fromCorner` than `toCorner` is, so it can
    ///   equal neither by value;
    /// * `None` fails both tests in both languages (`null == firstCorner()` is false).
    ///
    /// The probe prints Java's reference answer beside the value one for all 452 rows of modes
    /// `poly`, `seg` and `rand`, and they agree on every row.
    ///
    /// This matters to Task 15: `FoundConnectionInserter.tryNeckDown:492` and
    /// `insertFanoutMicroNeckdown:611,:629,:655` all read the answer as
    /// `candidateOkPoint == targetPoint`.
    #[allow(clippy::too_many_arguments)]
    fn insert_forced_trace_segment(
        &mut self,
        engine: Option<&mut AutorouteEngine>,
        from_corner: &Point,
        to_corner: &Point,
        half_width: i32,
        layer: usize,
        net_numbers: &[i32],
        clearance_class_index: usize,
        max_recursion_depth: i32,
        max_via_recursion_depth: i32,
        max_spring_over_recursion_depth: i32,
        tidy_width: i32,
        pull_tight_accuracy: i32,
        with_check: bool,
        time_limit: Option<&TimeLimit>,
        stop: StopCheck<'_>,
    ) -> Result<Option<Point>, BoardError>;
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

    fn insert_forced_trace_polyline(
        &mut self,
        engine: Option<&mut AutorouteEngine>,
        polyline: &Polyline,
        half_width: i32,
        layer: usize,
        net_numbers: &[i32],
        clearance_class_index: usize,
        max_recursion_depth: i32,
        max_via_recursion_depth: i32,
        max_spring_over_recursion_depth: i32,
        tidy_width: i32,
        pull_tight_accuracy: i32,
        with_check: bool,
        time_limit: Option<&TimeLimit>,
        stop: StopCheck<'_>,
    ) -> Result<Option<Point>, BoardError> {
        // RoutingBoard.java:469.
        self.clear_shove_failing_obstacle();
        // :470-471.
        let from_corner = polyline.first_corner();
        let to_corner = polyline.last_corner();
        // :472-480. "A degenerate polyline (parallel/near-parallel lines produced while shoving
        // against fixed multi-layer copper) has no well-defined first/last corner -- corner(i)
        // returns null. The trace cannot be inserted; return null so the caller treats this
        // segment as not inserted and reroutes, instead of dereferencing a null corner."
        let (Some(from_corner), Some(to_corner)) = (from_corner, to_corner) else {
            return Ok(None);
        };
        // :481-483.
        if from_corner == to_corner {
            return Ok(Some(to_corner));
        }
        // :484-487. FRLogger.warn("only implemented for IntPoints")
        if !matches!(from_corner, Point::Int(_)) || !matches!(to_corner, Point::Int(_)) {
            return Ok(Some(from_corner));
        }
        // :488.
        self.start_marking_changed_area();

        // :489-517. "Check, if there ends an item of the same net at fromCorner. If so, its
        // geometry will be used to cut off dog ears of the check shape." `pick_traces` is Java's
        // `pickItems(location, layer, new ItemSelectionFilter(TRACES))` (`:492-494`).
        //
        // not ported: the `compare_trace_insert_forced_sub` `FRLogger.trace` blocks (`:495-507`,
        // `:603-615`, `:621-633`, `:758-771`, `:793-805`, `:811-821`, `:846-859`, `:863-874`) and
        // the five `netNumbers[0] == 94` `compare_trace_insert_forced_fail` blocks (`:526-533`,
        // `:544-551`, `:647-654`, `:665-672`, `:705-728`) — diagnostics; no decision reads them.
        //
        // `pickedTrace` is a **live `PolylineTrace` reference** in Java, and the only thing read
        // through it is `combineTrace.polyline()` at `:541` and `:680`, both of them *after* the
        // shove loop has run. The port keeps the polyline itself rather than the id, which is the
        // fix Task 10b made for `ShapeTraceEntries.EntryPoint.trace` and for the same reason: a
        // shove that cuts this trace out of the board leaves Java's reference alive, holding the
        // very lines snapshotted here, where an id lookup would answer `None`. The two agree in
        // the other direction too — nothing between here and `:680` mutates a *board* trace's
        // polyline in place (`TraceShover::insert` cuts and re-inserts).
        let picked_items = self.pick_traces(&from_corner, Some(layer));
        let mut picked_trace: Option<Polyline> = None;
        if picked_items.len() == 1 {
            let current = *picked_items.iter().next().expect("size is 1");
            // :510-516. Java's fourth test is `instanceof PolylineTrace`; `pick_traces` already
            // filtered to `Item::is_trace`, and `Item::Trace` *is* `PolylineTrace` in this port
            // (`fr-board` has no other trace kind).
            if let Some(Item::Trace(trace)) = self.items.get(&current)
                && trace.hdr.nets_equal(net_numbers)
                && trace.get_half_width() == half_width
                && trace.hdr.clearance_class() == clearance_class_index
            {
                picked_trace = Some(trace.polyline().clone());
            }
        }

        // :518-520. The **default** tree, as in `check_forced_trace_polyline`.
        let compensated_half_width = half_width
            + self.trees.get_default_tree().clearance_compensation_value(
                clearance_class_index,
                layer,
                &self.rules,
            );
        // :521-535. `contactPins` is `null` here, so every own-net pin stays passable.
        let Some(mut new_polyline) = TraceShover::spring_over_obstacles(
            self,
            polyline,
            compensated_half_width,
            layer,
            net_numbers,
            clearance_class_index,
            None,
        ) else {
            return Ok(Some(from_corner));
        };

        // :536-542.
        let mut combined_polyline = combine_with_picked(&new_polyline, picked_trace.as_ref());
        // :543-553.
        if combined_polyline.lines().len() < 3 {
            return Ok(Some(from_corner));
        }
        // :554-559. Java's `startShapeNo` is an `int` and **can be negative**: `Polyline.combine`
        // ends in `new Polyline(Line[])`, whose overlap removal can leave the combined polyline
        // *shorter* than the one it was built from. Java then hands the negative value to
        // `offsetShapes`, which clamps it with `Math.max(requestedFromNo, 0)` (Polyline.java:359);
        // a `usize` cannot carry the negative value, so the clamp happens here instead and
        // `offset_shapes_between` sees exactly what Java's `fromNo` would be.
        let start_shape_no = (combined_polyline.lines().len() as i64
            - new_polyline.lines().len() as i64)
            .max(0) as usize;
        let trace_shapes = combined_polyline.offset_shapes_between(
            compensated_half_width,
            start_shape_no,
            combined_polyline.lines().len() - 1,
        );
        let mut last_shape_no = trace_shapes.len();
        // :560.
        let orthogonal_mode = self.rules.trace_angle_restriction == AngleRestriction::NinetyDegree;

        // :562-619. The shove loop.
        for i in 0..trace_shapes.len() {
            // :563-566.
            let current_trace_shape = if orthogonal_mode {
                TileShape::Box(trace_shapes[i].bounding_box())
            } else {
                trace_shapes[i].clone()
            };
            // :567-571.
            let from_side = entry_side(
                &combined_polyline,
                trace_shapes.len(),
                i,
                &current_trace_shape,
            );
            // :572-589.
            if with_check {
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
                    time_limit,
                );
                if !check_shove_ok {
                    last_shape_no = i;
                    break;
                }
            }
            // :590-618.
            let insert_ok = TraceShover::insert(
                self,
                &current_trace_shape,
                Some(&from_side),
                layer,
                net_numbers,
                clearance_class_index,
                None,
                max_recursion_depth,
                max_via_recursion_depth,
                max_spring_over_recursion_depth,
                stop,
            )?;
            if !insert_ok {
                return Ok(None);
            }
        }

        // :620.
        let mut new_corner = to_corner.clone();
        // :634-746. "the shove with index lastShapeNo failed. Sample the shove line to a shorter
        // shove distance and try again."
        if last_shape_no < trace_shapes.len() {
            // :637-640.
            let mut last_trace_shape = if orthogonal_mode {
                TileShape::Box(trace_shapes[last_shape_no].bounding_box())
            } else {
                trace_shapes[last_shape_no].clone()
            };
            // :641-644.
            let sample_width = 2 * self.get_min_trace_half_width();
            // totalized: `RoutingBoard.insertForcedTracePolyline`'s two `newPolyline.cornerApprox`
            // reads (`:642-643`) -> a refused segment. `lastShapeNo` is bounded by
            // `traceShapes.length`, which is `newPolyline.lines.length - 2`, so `lastShapeNo + 1`
            // is always a valid corner index and Java's `corner(i)` cannot answer `null` here.
            // Unreachable — no register row.
            let (Some(last_corner), Some(prev_last_corner)) = (
                new_polyline.corner_approx(last_shape_no + 1),
                new_polyline.corner_approx(last_shape_no),
            ) else {
                return Ok(Some(from_corner));
            };
            let last_segment_length = last_corner.distance(&prev_last_corner);
            // :645-656. "to many cycles to sample". Java multiplies two `int`s and *then* widens,
            // so a `sampleWidth` above 21 474 836 wraps; `wrapping_mul` is that, not `100.0 * x`.
            if last_segment_length > f64::from(100_i32.wrapping_mul(sample_width)) {
                return Ok(Some(from_corner));
            }
            // :657.
            let mut shape_index =
                shape_entry_index(&combined_polyline, trace_shapes.len(), last_shape_no);
            // :658-690.
            if last_segment_length > f64::from(sample_width) {
                // :659-661. Java's `newLineCount` is an `int`, and `traceShapes.length` is derived
                // from `combinedPolyline` while the length subtracted from is `newPolyline`'s, so
                // a shorter combined polyline (see `startShapeNo` above) can make it negative;
                // `Polyline.shorten` then throws out of its `System.arraycopy`. The port panics
                // instead, at the same boundary and with a message that names the site.
                // totalized: a negative `newLineCount` -> a panic, where Java throws
                // `ArrayIndexOutOfBoundsException` inside `Polyline.shorten`.
                let new_line_count = i64::try_from(new_polyline.lines().len()).unwrap_or(i64::MAX)
                    - (trace_shapes.len() as i64 - last_shape_no as i64 - 1);
                let new_line_count = usize::try_from(new_line_count).unwrap_or_else(|_| {
                    panic!(
                        "RoutingBoard.insertForcedTracePolyline:659-661: newLineCount is \
                         {new_line_count}, and Polyline.shorten throws on a negative one"
                    )
                });
                new_polyline = new_polyline
                    .shorten(new_line_count, f64::from(sample_width))
                    .unwrap_or_else(|e| {
                        panic!("Polyline.shorten threw (Polyline.java:148, quirk #22): {e}")
                    });
                // :662-674. FRLogger.trace("IntPoint expected")
                let current_last_corner = new_polyline.last_corner();
                let Some(current_last_corner @ Point::Int(_)) = current_last_corner else {
                    return Ok(Some(from_corner));
                };
                // :675.
                new_corner = current_last_corner;
                // :676-681.
                combined_polyline = combine_with_picked(&new_polyline, picked_trace.as_ref());
                // :682-684.
                if combined_polyline.lines().len() < 3 {
                    return Ok(Some(new_corner));
                }
                // :685-689.
                shape_index = combined_polyline.lines().len() - 3;
                // totalized: `combinedPolyline.offsetShape(compensatedHalfWidth, shapeIndex)`
                // (`:686`) -> a refused segment. `shapeIndex` is `lines.length - 3` and the three
                // lines above guarantee `lines.length >= 3`, so Java's
                // `FRLogger.warn("offsetShape: no out of range")` + `null` cannot happen.
                // Unreachable — no register row.
                let Some(shape) =
                    combined_polyline.offset_shape(compensated_half_width, shape_index)
                else {
                    return Ok(Some(from_corner));
                };
                last_trace_shape = if orthogonal_mode {
                    TileShape::Box(shape.bounding_box())
                } else {
                    shape
                };
            }
            // :691.
            let from_side =
                ShapeEntrySide::from_polyline(&combined_polyline, shape_index, &last_trace_shape);
            // :692-730.
            let check_shove_ok = TraceShover::check(
                self,
                &last_trace_shape,
                Some(&from_side),
                None,
                layer,
                net_numbers,
                clearance_class_index,
                max_recursion_depth,
                max_via_recursion_depth,
                max_spring_over_recursion_depth,
                time_limit,
            );
            if !check_shove_ok {
                return Ok(Some(from_corner));
            }
            // :731-745. FRLogger.trace("shove trace failed")
            let insert_ok = TraceShover::insert(
                self,
                &last_trace_shape,
                Some(&from_side),
                layer,
                net_numbers,
                clearance_class_index,
                None,
                max_recursion_depth,
                max_via_recursion_depth,
                max_spring_over_recursion_depth,
                stop,
            )?;
            if !insert_ok {
                return Ok(None);
            }
        }
        // :747-750. "insert the new trace segment"
        for i in 0..new_polyline.corner_count() {
            let Some(corner) = new_polyline.corner_approx(i) else {
                continue;
            };
            self.join_changed_area(&corner, layer);
        }
        // :751-754.
        let mut new_trace = self.insert_trace_without_cleaning(
            new_polyline.clone(),
            layer,
            half_width,
            net_numbers.to_vec(),
            clearance_class_index,
            FixedState::Unfixed,
        );
        // :756. `newTrace.combine()`.
        //
        // Java bug: `RoutingBoard.insertForcedTracePolyline:756` dereferences `newTrace` with no
        // null guard, while `:791` — 35 lines later, on the same variable — guards it with
        // `newTrace != null &&`. `insertTraceWithoutCleaning` answers `null` for a polyline with
        // fewer than two corners (BasicBoard.java:185-187) and for a closed trace below
        // `USER_FIXED` (`:191-195`), both of which `newPolyline` can be after the `:659-661`
        // shorten, so `:756` is a latent `NullPointerException` (quirk #185). No `catch` covers
        // it — this method's only `try` opens at `:787` — so the nearest handler is
        // `AutorouteConnectionRouter.route:155-158`'s bare `FAILED`, and the port reproduces that
        // with a panic the caller's `catch_unwind` boundary turns into the same `FAILED`.
        let combine_target = new_trace.expect(
            "RoutingBoard.insertForcedTracePolyline:756: newTrace.combine() on a null trace — \
             Java throws a NullPointerException here (quirk #185)",
        );
        let _combine_result = self.combine_trace(combine_target)?;

        // :773-776.
        let tidy_region: Option<IntOctagon> = if tidy_width < i32::MAX {
            Some(
                new_corner
                    .surrounding_octagon()
                    .enlarge(f64::from(tidy_width)),
            )
        } else {
            None
        };
        // :777-782. `optNetNoArr` is `TraceTightener`'s `onlyNetNoArr`, and its **only** reader
        // is `PolylineTrace.pullTight:821-823`'s "this trace is not on one of those nets" refusal.
        //
        // obligation: the `maxRecursionDepth <= 0` arm is unobservable here for the same reason
        // the `:829` pick is (see that marker): the trace `:860-862` pull-tightens is the one
        // just inserted, whose nets *are* `netNumbers`, so the filter passes whichever array this
        // produces. It bites only when `:826-833` re-picks a **foreign** trace. **Task 17**
        // covers both with one fixture connection.
        let opt_net_no_arr = if max_recursion_depth <= 0 {
            net_numbers.to_vec()
        } else {
            Vec::new()
        };
        // :783-785. Java passes `null` for the `Stoppable` and `-1` for the time limit.
        let mut pull_tight_algo = TraceTightener::get_instance(
            self,
            opt_net_no_arr,
            tidy_region,
            pull_tight_accuracy,
            None,
            -1,
            Some(new_corner.clone()),
            layer as i32,
        );

        // :787-842. Java's `try { … } catch (Exception e) { FRLogger.trace(…) }`: "max
        // normalization depth is hit for geometrically complex or degenerate trace segments. The
        // router skips the segment and continues; affected connections may remain unrouted." The
        // degraded value (plan-6 ruling 7) is "leave `newTrace` as it is and fall through to the
        // pull-tight tail", which is what dropping the error does here — `BoardError::Stopped`
        // excepted, because it is the port's cancellation and not one of Java's exceptions.
        //
        // not ported: the `FRLogger.trace` of the catch block itself (`:838-841`).
        {
            // :790-791. `changedArea` is non-null: `:488` created it.
            let clip_shape = self
                .changed_area
                .as_ref()
                .expect("RoutingBoard.insertForcedTracePolyline:488 called startMarkingChangedArea")
                .get_area(layer);
            // The `_checked` overload, for plan-3 ruling F's reason: `normalize` reaches
            // `Board::split_trace_checked`, which is quirk #76's non-terminating walk on a ladder
            // board. Java has no cancellation here; the check adds no decision of its own.
            let normalize_result = match new_trace {
                Some(trace) => self.normalize_trace_checked(trace, Some(&clip_shape), stop),
                None => Ok(false),
            };
            match normalize_result {
                Err(BoardError::Stopped) => return Err(BoardError::Stopped),
                Err(_) => {}
                Ok(false) => {}
                // :806-834.
                Ok(true) => {
                    // :808-810.
                    match pull_tight_algo.split_traces_at_keep_point(self) {
                        Err(BoardError::Stopped) => return Err(BoardError::Stopped),
                        Err(_) => {}
                        Ok(_) => {
                            // :822-833. "otherwise the new corner may no more be contained in the
                            // new trace after optimizing". Java's `pickItems` answers a
                            // `TreeSet<Item>` ordered by `Item.compareTo` (Item.java:95-103,
                            // `other.id - this.id`), so `iterator().next()` is the item with the
                            // **highest** id; `pick_traces`' `BTreeSet<ItemId>` is ascending, so
                            // the same element is `next_back`.
                            //
                            // obligation: this choice has **no discriminating row** in
                            // `p6t15b-insert-forced.txt`. Instrumented, the branch is reached
                            // 118 times across every mode and only **one** of those (a `rand`
                            // row at `(-322, 879)`, 45-degree, candidates 5 and 10) has more than
                            // one trace to choose from — and there `:860-862` pull-tightens
                            // neither of them, so `next()` and `next_back()` leave the same
                            // board (verified by mutation). Reaching it needs
                            // `normalize` at `:791` to answer `true` **and** two traces at
                            // `newCorner`; `ownNetCross` gives the first and `ownNetTee` the
                            // second, and no case on this board gives both. **Task 17** must
                            // record a fixture connection that does.
                            let picked = self.pick_traces(&new_corner, Some(layer));
                            new_trace =
                                picked.iter().next_back().copied().filter(|id| {
                                    matches!(self.items.get(id), Some(Item::Trace(_)))
                                });
                        }
                    }
                }
            }
        }

        // :860-862. "To avoid, that a separate handling for moving backwards in the own trace
        // line becomes necessary, pull tight is called here." `FoundConnectionInserter:185`
        // passes `tidyWidth = Integer.MAX_VALUE`, so this branch runs on every autoroute
        // insertion — which is why controller ruling AB moved the tightener family into Plan 6.
        if tidy_width > 0
            && let Some(trace) = new_trace
        {
            <Board as PolylineTraceExt>::pull_tight_with_engine(
                self,
                trace,
                &mut pull_tight_algo,
                engine,
            );
        }
        // :875.
        Ok(Some(new_corner))
    }

    fn insert_forced_trace_segment(
        &mut self,
        engine: Option<&mut AutorouteEngine>,
        from_corner: &Point,
        to_corner: &Point,
        half_width: i32,
        layer: usize,
        net_numbers: &[i32],
        clearance_class_index: usize,
        max_recursion_depth: i32,
        max_via_recursion_depth: i32,
        max_spring_over_recursion_depth: i32,
        tidy_width: i32,
        pull_tight_accuracy: i32,
        with_check: bool,
        time_limit: Option<&TimeLimit>,
        stop: StopCheck<'_>,
    ) -> Result<Option<Point>, BoardError> {
        // RoutingBoard.java:375-377.
        if from_corner == to_corner {
            return Ok(Some(to_corner.clone()));
        }
        // :378.
        let insert_polyline = Polyline::from_two_points(from_corner, to_corner);
        // :379-392.
        let ok_point = self.insert_forced_trace_polyline(
            engine,
            &insert_polyline,
            half_width,
            layer,
            net_numbers,
            clearance_class_index,
            max_recursion_depth,
            max_via_recursion_depth,
            max_spring_over_recursion_depth,
            tidy_width,
            pull_tight_accuracy,
            with_check,
            time_limit,
            stop,
        )?;
        // :393-401. Java's three-way **reference** test; see the trait doc for why value
        // equality is the same test here. `ok_point` is `None` exactly where Java's `okPoint` is
        // `null`, and `null` matches neither corner.
        let result = if ok_point.is_some() && ok_point == insert_polyline.first_corner() {
            Some(from_corner.clone())
        } else if ok_point.is_some() && ok_point == insert_polyline.last_corner() {
            Some(to_corner.clone())
        } else {
            ok_point
        };
        Ok(result)
    }
}

/// `RoutingBoard.insertForcedTracePolyline:536-542` and `:676-681`, which are the same four
/// lines twice: with a picked trace to combine with, the new polyline is combined with that
/// trace's own polyline; without one it is used as it is.
fn combine_with_picked(new_polyline: &Polyline, picked: Option<&Polyline>) -> Polyline {
    let Some(combine_polyline) = picked else {
        return new_polyline.clone();
    };
    // `Polyline.combine` (Polyline.java:693-749) ends in `new Polyline(newLines)`, so its `Err`
    // is Java's `ArrayIndexOutOfBoundsException` out of `removeOverlaps` (quirk #22) and panics
    // here for the same reason `java_reverse` does: no `catch` stands between this line and
    // `AutorouteConnectionRouter.route:155-158`.
    // totalized: Java's ArrayIndexOutOfBoundsException out of `Polyline.combine` becomes a panic.
    new_polyline
        .combine(combine_polyline)
        .unwrap_or_else(|e| panic!("Polyline.combine threw (Polyline.java:148, quirk #22): {e}"))
}

/// `RoutingBoard.insertForcedTracePolyline`'s two `ShapeEntrySide` indices — `:570` inside the
/// shove loop and `:657` in the resample branch — which are the same expression,
/// `combinedPolyline.cornerCount() - traceShapes.length - 1 + i`.
///
/// # It is the shove line's index **minus one**, where `checkForcedTracePolyline` uses the index
///
/// `traceShapes` is `combinedPolyline.offsetShapes(hw, startShapeNo, lines.length - 1)`, whose
/// shape `i` is built from `lines[startShapeNo + 1 + i]` (Polyline.java:358-368), and
/// `cornerCount()` is `lines.length - 1` (`:178-181`), so this expression reduces to
/// `startShapeNo + i` — one **less** than the shape's own line index.
/// `RoutingBoard.checkForcedTracePolyline:429`, which builds the side for the same shape, passes
/// `i + 1`, i.e. the line index itself, and `ShapeEntrySide`'s own doc says "no is expected
/// between 1 and polyline.lineCount - 2".
///
/// **Both answer the same entry side, and that is measured rather than assumed.**
/// `ShapeEntrySide`'s constructor walks *down* from `no` (ShapeEntrySide.java:31-40) for the
/// first segment that crosses the shape's border and takes `borderIntersections[0]`, which is the
/// crossing nearest the segment's start — so `no = lineIndex` finds where the shove line *enters*
/// the shape it is centred on, and `no = lineIndex - 1` finds where its predecessor crosses the
/// same border, which is the same side; at `startShapeNo == 0 && i == 0` the index is `0`, the
/// loop runs zero times and `:41-58`'s "the first corner of polyline is inside shape" fallback
/// computes that entry side directly. Probe mode `side` prints both indices and both
/// `ShapeEntrySide.no` values for 45 shape rows — 14 cases x 3 regimes, minus the degenerate
/// case, which has no shape — and every row says
/// `agree=true`. The expression is transcribed exactly all the same, because nothing in
/// `ShapeEntrySide` *guarantees* the two agree.
///
/// obligation: `RoutingBoard.insertForcedTracePolyline`'s `ShapeEntrySide` index has **no board
/// row that discriminates it**: because the two indices agree on every fixture here, changing
/// this expression to `checkForcedTracePolyline`'s leaves all 1 621 rows of modes `poly`, `tail`,
/// `seg`, `neck` and `rand` byte-identical (verified by mutation). Probe mode `side` pins the
/// index values themselves; **Task 17** should record, for one fixture connection, a shove whose
/// entry side differs between the two — a polyline whose segment before the shove line re-enters
/// that line's offset shape through a different border side.
fn shape_entry_index(combined_polyline: &Polyline, trace_shape_count: usize, i: usize) -> usize {
    combined_polyline.corner_count() - trace_shape_count - 1 + i
}

/// `shape_entry_index` plus the `new ShapeEntrySide(...)` it feeds (`:567-571`).
fn entry_side(
    combined_polyline: &Polyline,
    trace_shape_count: usize,
    i: usize,
    shape: &TileShape,
) -> ShapeEntrySide {
    ShapeEntrySide::from_polyline(
        combined_polyline,
        shape_entry_index(combined_polyline, trace_shape_count, i),
        shape,
    )
}
