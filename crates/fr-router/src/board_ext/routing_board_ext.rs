//! [`RoutingBoardExt`]: the `RoutingBoard` methods `fr-board` deliberately left out.

use std::collections::BTreeSet;

use fr_board::datastructures::StopCheck;
use fr_board::items::Item;
use fr_board::prelude::*;
use fr_board::{BoardError, ItemId, RoomId, TimeLimit};
use fr_geometry::{IntOctagon, Point, Polyline, TileShape};
use fr_settings::{ExpansionCostFactor, RouterSettings};

use crate::autoroute::attempt::{AutorouteAttemptResult, AutorouteAttemptState};
use crate::autoroute::maze::control::AutorouteControl;
use crate::autoroute::maze::engine::AutorouteEngine;
use crate::board_ext::drill_item_mover::tree_by_id;
use crate::board_ext::tightener::{PolylineTraceExt, TraceTightener};
use crate::board_ext::trace_shover::TraceShover;
use crate::pipeline::RouterBudget;

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
    /// `RoutingBoard`'s two `optChangedArea` overloads (`:151-190`) are **Plan 7 Task 5's**, and
    /// landed there as [`RoutingBoardExt::opt_changed_area`] /
    /// [`RoutingBoardExt::opt_changed_area_with_keep_point`]; they are not reached from here.
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
    ///   equal neither by value. That step needs `sampleWidth = 2 * getMinTraceHalfWidth() > 0`:
    ///   at a zero minimum half width the `:658` guard degenerates to `> 0` and `shorten(_, 0.0)`
    ///   could leave `newCorner` value-equal to `toCorner` where Java's reference test says no.
    ///   The conclusion survives anyway — both sides still answer the same *value*, which is all
    ///   `tryNeckDown:492` compares — and no shipped board has a zero minimum trace half width;
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

    /// Port of `RoutingBoard.optChangedArea(int[], IntOctagon, int, ExpansionCostFactor[],
    /// Stoppable, int)` (RoutingBoard.java:151-161): "optimizes the route in the internally marked
    /// area. If netNumber > 0, only traces with net number netNumber are optimized. If clipShape
    /// != null the optimizing is restricted to clipShape. traceCosts is used for optimizing vias
    /// and may be null. If stoppableThread != null, the algorithm can be requested to be stopped.
    /// If timeLimit > 0; the algorithm will be stopped after timeLimit Milliseconds."
    ///
    /// The batch pull-tight + via-optimise sweep every routed connection, every tail removal and
    /// every fanout pin runs. Java's overload passes `keepPoint = null` and `keepPointLayer = 0`
    /// (`:159-160`); this is that call, and
    /// [`Self::opt_changed_area_with_keep_point`] is the eight-argument one below it.
    ///
    /// `time_limit_ms` is controller ruling AI's knob: Java's callers all pass the literal
    /// `TIME_LIMIT_TO_PREVENT_ENDLESS_LOOP = 1000`;
    /// [`RouterBudget::opt_changed_area_ms`](crate::pipeline::RouterBudget::opt_changed_area_ms)
    /// supplies it, and `0` is Java's own "no limit" (`TraceTightener.java:73-77` only builds a
    /// `TimeLimit` when `timeLimit > 0`).
    #[allow(clippy::too_many_arguments)]
    fn opt_changed_area(
        &mut self,
        engine: Option<&mut AutorouteEngine>,
        only_net_no_arr: &[i32],
        clip_shape: Option<IntOctagon>,
        accuracy: i32,
        trace_costs: Option<&[ExpansionCostFactor]>,
        stop: StopCheck<'_>,
        time_limit_ms: i32,
    ) -> Result<(), BoardError>;

    /// Port of the `keepPoint` overload `RoutingBoard.optChangedArea(int[], IntOctagon, int,
    /// ExpansionCostFactor[], Stoppable, int, Point, int)` (RoutingBoard.java:171-190) together
    /// with the body it delegates to, `RoutingBoardOperations.optChangedArea` (`:52-79`): "if
    /// keepPoint != null, traces on layer keepPointLayer containing keepPoint will also contain
    /// this point after optimizing."
    ///
    /// `keep_point`/`keep_point_layer` are `null`/`0` from every router caller; only `RouteState`
    /// (GUI) passes them.
    #[allow(clippy::too_many_arguments)]
    fn opt_changed_area_with_keep_point(
        &mut self,
        engine: Option<&mut AutorouteEngine>,
        only_net_no_arr: &[i32],
        clip_shape: Option<IntOctagon>,
        accuracy: i32,
        trace_costs: Option<&[ExpansionCostFactor]>,
        stop: StopCheck<'_>,
        time_limit_ms: i32,
        keep_point: Option<Point>,
        keep_point_layer: i32,
    ) -> Result<(), BoardError>;

    /// Port of `RoutingBoard.removeItemsAndPullTight(Collection<Item>, int, int)`
    /// (RoutingBoard.java:124-127, whose body is `RoutingBoardOperations.java:81-120`): "removes
    /// the items in itemList and pulls the nearby rubber traces tight. Returns false, if some
    /// items could not be removed, because they were fixed."
    ///
    /// The order is Java's: mark + remove (`:90-109`, [`Board::remove_items_marking_changed_area`]),
    /// then `combineTraces(netNo)` over the changed nets in **ascending** order (`:111-113`), then
    /// one `optChangedArea` (`:115-116`).
    ///
    /// # The signature is Java's, not the plan's
    ///
    /// The plan's draft gave this method `accuracy`, `with_preferred_directions`, `stop` and
    /// `time_limit_ms` parameters. Java has none of them: it takes `(itemList, tidyWidth,
    /// pullTightAccuracy)` and hard-codes the rest of the `optChangedArea` call — `new int[0]`
    /// (all nets), `traceCosts = null`, `stoppableThread = null` and
    /// `timeLimit = PULL_TIGHT_TIME_LIMIT`, which is **2000**
    /// (`RoutingBoardOperations.java:18`, `:115-116`) and *not* the router's
    /// `TIME_LIMIT_TO_PREVENT_ENDLESS_LOOP = 1000`. Java wins (plan conventions §1), so the port
    /// takes the three Java arguments plus the `engine` every `RoutingBoardExt` method needs.
    ///
    /// # `tidyWidth` decides whether the tightener runs at all — and reaches quirk #184
    ///
    /// `:83-89` is a three-way switch on `tidyWidth`, and the port reproduces all three arms:
    ///
    /// | `tidyWidth` | `tidyRegion` handed to `optChangedArea` | effect |
    /// |---|---|---|
    /// | `< 0` or `== 0` | the `IntOctagon.EMPTY` **singleton**, never enlarged | quirk #204's guard is `false`, so the whole `TraceTightener` sweep is skipped |
    /// | `> 0`, `< i32::MAX` | the union of every removed shape's bounding octagon, `enlarge`d by `tidyWidth` | a **non-null clip shape**, the only one on any port path |
    /// | `== i32::MAX` | `null` | no restriction (the `None` arm of quirk #204) |
    ///
    /// The middle row is why this method is ported at all: it is the **only** caller in either
    /// language that hands `TraceTightener` a real clip octagon, and therefore the only way to
    /// reach **quirk #184** (`TraceTightener45.reduceCorners`' stale `currentCornerInClipShape[3]`
    /// copy), which the plan left as Task 8's reachability obligation. `crates/fr-router/tests/
    /// batch_autorouter.rs`'s `remove_items_and_pull_tight_reaches_quirk_184s_stale_clip_flag` is
    /// the directed case.
    ///
    /// # Headless-dead
    ///
    /// The only Java caller is `gui/interactive/RouteState.java:340` (`RouteState.cancel()` with
    /// push enabled), so **no Plan 7 code calls this method**: it is ported because the audit
    /// demands it, because Plan 8's interactive surface may reach it, and because quirk #184's
    /// reachability is decided here. A grep for callers outside `crates/fr-router/tests` finds
    /// none, and that is the intended state.
    fn remove_items_and_pull_tight(
        &mut self,
        engine: Option<&mut AutorouteEngine>,
        item_list: &[ItemId],
        tidy_width: i32,
        pull_tight_accuracy: i32,
    ) -> Result<bool, BoardError>;

    /// Port of `RoutingBoard.fanout(Pin, RouterSettings, int, Stoppable, TimeLimit)`
    /// (RoutingBoard.java:978-1110): "autoroutes from the input pin until the first via, in case
    /// the pin and its connected set has only 1 layer. Ripup is allowed if `ripupCosts >= 0`."
    ///
    /// The per-pin escape router
    /// [`BatchFanout`](crate::pipeline::BatchFanout) calls once per SMD pin per pass
    /// (`BatchFanout.java:281-283`); Task 12 owns that loop.
    ///
    /// # The three parameters Java does not have
    ///
    /// `engine` is `RoutingBoard.autorouteEngine` (`:70`), threaded rather than held — see this
    /// trait's "The engine is a value, not a field". `stop` is plan-6 ruling 6's per-call
    /// [`StopCheck`] where Java passes the `Stoppable` into `initAutoroute` and reads it from the
    /// engine. `budget` is controller ruling AI's knob for `:1099`'s literal
    /// `timeLimitToPreventEndlessLoop = 1000`, which is a **local** here rather than one of the
    /// four class constants (`RoutingBoard.java:1100`).
    ///
    /// # Two attempts share one ripped set (quirk #221)
    ///
    /// For four targets or fewer (`:1064-1085`) Java routes to the *closest* target alone and,
    /// if that neither routed nor was already connected, retries against the **whole**
    /// unconnected set — passing the **same** `rippedItemList`, declared once at `:1058`. So items
    /// the abandoned first attempt ripped are still in the set when the retry runs, and
    /// `AutorouteEngine.autorouteConnection:237-245` deletes each of their whole connections from
    /// the board: a successful retry destroys connections only the *failed* attempt asked to rip.
    /// The set is a local and is never returned, so the deletion is the only observable — it is
    /// what `p7t5 pin`'s `"removed"` column prints. Java bug, reproduced.
    ///
    /// The `> 4` arm (`:1086-1092`) makes one attempt against the whole set: "for large nets
    /// (e.g. power/ground/buses), route to the entire unconnected set at once to avoid CPU
    /// thrashing".
    #[allow(clippy::too_many_arguments)]
    fn fanout(
        &mut self,
        engine: &mut Option<AutorouteEngine>,
        pin: ItemId,
        router_settings: &RouterSettings,
        ripup_costs: i32,
        stop: StopCheck<'_>,
        time_limit: Option<TimeLimit>,
        budget: RouterBudget,
    ) -> AutorouteAttemptResult;
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
        // the other direction too, and the reason is `:510`'s `netsEqual(netNumbers)`: it makes
        // `pickedTrace` **own-net**, and `ShapeTraceEntries` turns only *foreign*-net obstacles
        // into substitute pieces — so `pickedTrace` is never one of the pieces
        // `TraceShover.insert:540` (or `check:385`) calls `PolylineTrace.change` on, which are the
        // only in-place polyline mutations on this path. Splits and cut-outs produce **new**
        // items and leave the old lines intact, which is exactly what the snapshot holds.
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
                // totalized: `RoutingBoard.insertForcedTracePolyline:659-661`'s negative `newLineCount` -> a panic, where Java throws `ArrayIndexOutOfBoundsException` inside `Polyline.shorten`.
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
        //
        // totalized: `RoutingBoard.insertForcedTracePolyline`'s `newPolyline.cornerApprox(i)` (`:749`) -> a skipped corner.
        // Java's `Polyline.cornerApprox` clamps an out-of-range index and warns rather than
        // answering `null` (Polyline.java:271-281), and `i` is bounded by `cornerCount()`, so the
        // arm cannot fire. Unreachable — no register row; the same argument as the two
        // `cornerApprox` reads at `:642-643` above.
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
        // obligation: `RoutingBoard.insertForcedTracePolyline:777-782`'s `maxRecursionDepth <= 0` arm is still unobservable — **re-marked in Task 17**.
        // It is unobservable for the same reason the `:826-833` pick was (see that marker, which
        // Task 17 *did* discharge): the trace `:860-862` pull-tightens is the one just inserted,
        // whose nets *are* `netNumbers`, so the filter passes whichever array this produces.
        // Measured over Task 17's whole acceptance corpus — 369 connections on five boards,
        // `tests/reference/router-fixtures.txt` — with a counter on this arm: **it is entered
        // zero times**. Every corpus call reaches `insertForcedTracePolyline` with
        // `maxRecursionDepth > 0`, so `optNetNoArr` is always the empty array. Reaching it needs
        // a shove chain deep enough to exhaust the recursion budget, which no board here has.
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
                            // obligation: `RoutingBoard.insertForcedTracePolyline:826-833`'s `pickItems(...).iterator().next()` — **discharged in Task 17**.
                            // In `p6t15b-insert-forced.txt`, instrumented, the branch is reached
                            // 118 times across every mode and only **one** of those (a `rand`
                            // row at `(-322, 879)`, 45-degree, candidates 5 and 10) has more than
                            // one trace to choose from — and there `:860-862` pull-tightens
                            // neither of them, so `next()` and `next_back()` left the same board.
                            // Task 17's corpus does discriminate it: instrumented, the branch
                            // sees **two or more** candidate traces on `router-rpi-splitter`
                            // (2 evaluations), `router-j2-reference` (4) and
                            // `router-dac2020-bm01` (6, up to 4 candidates at once). Mutating
                            // `next_back()` — Java's `iterator().next()` over the **descending**
                            // `TreeSet<Item>` (quirk #44) — to `next()` makes
                            // `router-j2-reference` DIFF at connection k = 44 while
                            // `router-rpi-splitter` still matches, so the order is load-bearing
                            // and pinned by `tests/reference_parity.rs`.
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

    fn opt_changed_area(
        &mut self,
        engine: Option<&mut AutorouteEngine>,
        only_net_no_arr: &[i32],
        clip_shape: Option<IntOctagon>,
        accuracy: i32,
        trace_costs: Option<&[ExpansionCostFactor]>,
        stop: StopCheck<'_>,
        time_limit_ms: i32,
    ) -> Result<(), BoardError> {
        // RoutingBoard.java:158-160 — `null`, `0`.
        self.opt_changed_area_with_keep_point(
            engine,
            only_net_no_arr,
            clip_shape,
            accuracy,
            trace_costs,
            stop,
            time_limit_ms,
            None,
            0,
        )
    }

    fn opt_changed_area_with_keep_point(
        &mut self,
        engine: Option<&mut AutorouteEngine>,
        only_net_no_arr: &[i32],
        clip_shape: Option<IntOctagon>,
        accuracy: i32,
        trace_costs: Option<&[ExpansionCostFactor]>,
        stop: StopCheck<'_>,
        time_limit_ms: i32,
        keep_point: Option<Point>,
        keep_point_layer: i32,
    ) -> Result<(), BoardError> {
        // RoutingBoardOperations.java:61-63.
        if self.changed_area.is_none() {
            return Ok(());
        }
        // :64 — ruling 9. Java compares the **reference** `clipShape != IntOctagon.EMPTY`, so
        // `null` **runs** the branch and `Option::None` therefore does too: a `null` clip shape is
        // "no restriction", not "no work". Only the shared `IntOctagon.EMPTY` singleton skips it,
        // and `Route.java:225` (`traceTidyWidth == 0`) and `RoutingBoardOperations:86` (the same
        // width through `removeItemsAndPullTight`) are the two sites that pass it.
        //
        // Java bug (quirk #204): the guard reads as "restrict the optimizing to clipShape, and
        // skip it when there is nothing to restrict it to", and the reference test makes it
        // something else — a *hand-built* octagon with `EMPTY`'s eight coordinates is a different
        // object, so Java optimizes the **whole** changed area against a clip shape that excludes
        // every point, and `TightenerBase::{clip_is_outside, clip_contains}` then reject every
        // candidate. Reproduced rather than fixed; the port folds it into the same value comparison
        // [`IntOctagon::is_empty`] already uses for `IntOctagon.isEmpty()`'s own `this == EMPTY`
        // (int_octagon.rs:92-99), which differs from Java only for that unreachable hand-built
        // copy.
        if !clip_shape.is_some_and(|shape| shape.is_empty()) {
            // :65-75.
            let mut pull_tight_algo = TraceTightener::get_instance(
                self,
                only_net_no_arr.to_vec(),
                clip_shape,
                accuracy,
                Some(stop),
                time_limit_ms,
                keep_point,
                keep_point_layer,
            );
            // :76.
            pull_tight_algo.opt_changed_area(self, engine, trace_costs)?;
        }
        // not ported: `board.joinGraphicsUpdateBox(board.changedArea.surroundingBox())` (`:77`) —
        // the GUI repaint box; `Board` has no `joinGraphicsUpdateBox`.
        //
        // :78. Load-bearing: the next `startMarkingChangedArea` re-creates the store, and leaving
        // it in place would make the next sweep see this one's stale region.
        self.changed_area = None;
        Ok(())
    }

    fn remove_items_and_pull_tight(
        &mut self,
        engine: Option<&mut AutorouteEngine>,
        item_list: &[ItemId],
        tidy_width: i32,
        pull_tight_accuracy: i32,
    ) -> Result<bool, BoardError> {
        // RoutingBoardOperations.java:83-89. `tidyRegion` is *the singleton* on the `< MAX_VALUE`
        // arm — which is what makes quirk #204's reference guard skip the sweep for
        // `tidyWidth <= 0` — and `null` on the other.
        let (mut tidy_region, calculate_tidy_region) = if tidy_width < i32::MAX {
            (Some(IntOctagon::EMPTY), tidy_width > 0)
        } else {
            (None, false)
        };

        // :90-109. The mark + remove + `changedNets` half is `fr-board`'s, because every step of
        // it is a `Board` mutation; `tidyRegion` is accumulated here because it is `fr-router`'s
        // `optChangedArea` argument and `fr-board` has no reason to know it exists.
        //
        // Java interleaves the two: one pass over `itemList` that, per removable item, joins each
        // tile shape into the changed area *and* unions its bounding octagon into `tidyRegion`.
        // The port walks the shapes first and removes second. That is the same answer: the
        // predicate `isDeletionForbidden() || isUserFixed()` is a property of the item and of the
        // rules, neither of which any other item's removal changes, and one item's removal does
        // not move another item's tile shapes. The pre-walk therefore sees exactly the shapes
        // Java's interleaved loop sees, in the same order.
        //
        // **The cost, stated rather than hidden.** Java makes one pass and the port makes two,
        // so each removable item's tile shapes are computed twice. Three things bound it: the
        // pre-walk is inside `if calculate_tidy_region`, so the two arms this port actually
        // reaches — `tidyWidth <= 0` (the `IntOctagon.EMPTY` singleton) and `tidyWidth ==
        // Integer.MAX_VALUE` (a `null` clip) — do **no** second walk at all; the one arm that
        // does is the GUI's (`RouteState.cancel`, the method's only Java caller); and the item
        // list is a single trace tail there, not a board. **No Plan 7 code calls this method**,
        // so the second walk costs nothing on any production or parity path. Fusing it would mean
        // either duplicating `Board::remove_items_marking_changed_area`'s body here or widening
        // its `fr-board` signature with a tidy-region out-parameter that `fr-board` has no reason
        // to know about; neither is worth it for a headless-dead path.
        if calculate_tidy_region {
            for id in item_list {
                let Some(item) = self.items.get(id) else {
                    continue;
                };
                // :91-92 — the same skip, evaluated on the same item.
                if item.is_deletion_forbidden(&self.rules) || item.is_user_fixed() {
                    continue;
                }
                let shape_count = {
                    let ctx = self.ctx();
                    item.tile_shape_count(&ctx)
                };
                for i in 0..shape_count {
                    // :95-99. A shape the port cannot build is skipped rather than unioned;
                    // Java's `getTileShape` answers a real shape for every index below the count.
                    if let Some(shape) = self.item_tile_shape(*id, i)
                        && let Some(octagon) = shape.bounding_octagon()
                    {
                        tidy_region =
                            Some(tidy_region.unwrap_or(IntOctagon::EMPTY).union(&octagon));
                    }
                }
            }
        }

        // :90, :93, :100-108 — including `startMarkingChangedArea` at `:90`.
        let (result, changed_nets) = self.remove_items_marking_changed_area(item_list.to_vec());

        // :111-113. `changedNets` is a `TreeSet<Integer>`, i.e. **ascending** net number (plan-7
        // ruling 5's per-container decision: the key is a boxed `int`, the comparator is total and
        // the key cannot mutate, so `BTreeSet<i32>` is exact and no `JavaTreeSet` is needed).
        // `Board::remove_items_marking_changed_area` already answers one.
        for net_number in changed_nets {
            self.combine_traces(net_number)?;
        }

        // :114. `enlarge` is `offset`, which takes a `double` — Java widens `tidyWidth` here.
        if calculate_tidy_region {
            tidy_region = tidy_region.map(|region| region.enlarge(f64::from(tidy_width)));
        }

        // :115-116. Every argument but `tidyRegion` and `pullTightAccuracy` is a literal:
        // `new int[0]` (all nets), `traceCosts = null`, `stoppableThread = null` — which is the
        // never-stopping `StopCheck` — and `PULL_TIGHT_TIME_LIMIT`, declared at `:18` as **2000**.
        // Not `TIME_LIMIT_TO_PREVENT_ENDLESS_LOOP`, and therefore not
        // `RouterBudget::opt_changed_area_ms`: this constant has no other reader and no Java caller
        // can vary it, so it stays a literal here rather than becoming a port-only knob.
        self.opt_changed_area(
            engine,
            &[],
            tidy_region,
            pull_tight_accuracy,
            None,
            &|| false,
            PULL_TIGHT_TIME_LIMIT,
        )?;
        // :117.
        Ok(result)
    }

    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    fn fanout(
        &mut self,
        engine: &mut Option<AutorouteEngine>,
        pin: ItemId,
        router_settings: &RouterSettings,
        ripup_costs: i32,
        stop: StopCheck<'_>,
        time_limit: Option<TimeLimit>,
        budget: RouterBudget,
    ) -> AutorouteAttemptResult {
        let ctx = self.ctx();
        let pin_item = self
            .get_item(pin)
            .expect("RoutingBoard.fanout takes a Pin off the board");
        // `:984-987`. Java's message interpolates `pin.toString()` (Pin.java:675-692), which is
        // `Display for Item`.
        let already_connected = || {
            AutorouteAttemptResult::with_details(
                AutorouteAttemptState::AlreadyConnected,
                format!("The pin '{pin_item}' is already connected."),
            )
        };
        if pin_item.first_layer(&ctx) != pin_item.last_layer(&ctx) || pin_item.net_count() != 1 {
            return already_connected();
        }
        // :988-990.
        let pin_net_no = pin_item.get_net_number(0);
        let pin_layer = pin_item.first_layer(&ctx);
        // `pin.getConnectedSet(pinNetNo)` is the `stopAtPlane = false` overload
        // (Item.java:596-598), the same one `AutorouteConnectionRouter.route:55` takes.
        let pin_connected_set = self.connected_set(pin, pin_net_no, false);
        // :991-996 — quirk #44's descending id order, which cannot change the answer here (the
        // loop returns on the first non-conforming item and the predicate is per item) but is the
        // convention every `TreeSet<Item>` walk in this port keeps. The target sort at `:1002-1021`
        // below keeps the *same* rule and there it **does** change the answer, because the sort is
        // stable and the seed decides every distance tie — see [`sorted_unconnected_targets`].
        for current_item in pin_connected_set.iter().rev() {
            let Some(item) = self.get_item(*current_item) else {
                continue;
            };
            if item.first_layer(&ctx) != pin_layer || item.last_layer(&ctx) != pin_layer {
                return already_connected();
            }
        }
        // :997-1001.
        let unconnected_set = self.unconnected_set(pin, pin_net_no);
        if unconnected_set.is_empty() {
            return AutorouteAttemptResult::with_details(
                AutorouteAttemptState::NoUnconnectedNets,
                format!("The pin '{pin_item}' is already connected."),
            );
        }

        // :1002-1021 — the targets, sorted by the **squared** distance from the pin centre to the
        // midpoint of each item's bounding box.
        //
        // `sortedUnconnectedList` is `new ArrayList<>(unconnectedSet)`, i.e. the `TreeSet<Item>`'s
        // iteration order — and that is **descending** id, quirk #44: `Item.compareTo` is
        // `item.id - id` (Item.java:95-103) and `Item.getUnconnectedSet` returns a plain
        // `new TreeSet<>()` (Item.java:676-690), the same rule the `pin_connected_set` walk above
        // keeps with `.iter().rev()`. `List.sort` is TimSort, which is **stable**, so on an exact
        // distance tie the **higher** id wins. `sort_by` is Rust's stable sort, and the key is
        // built the way Java builds it: `int` midpoint sums divided by `2.0`, then a `double`
        // difference.
        let pin_center = pin_center_of(self, pin).to_float();
        let sorted_unconnected_list =
            sorted_unconnected_targets(self, &pin_center, &unconnected_set);

        // :1023-1024 — the three-argument constructor (AutorouteControl.java:117-120), which is
        // `settings.getTraceCosts()` / `settings.getViaCosts()`.
        //
        // pub seam discharged: `AutorouteControl::from_settings`' marker names "Plan 7's fanout
        // pre-pass", and this is that call site.
        let mut ctrl_settings = AutorouteControl::from_settings(self, pin_net_no, router_settings);
        ctrl_settings.is_fanout = true;

        // :1025-1044 — the `fallbackToBoardVias` combined rule.
        //
        // This is the run-time-synthesised `ViaRule` that made Plan 7 Task 0 a prerequisite: it is
        // in no list, so `AutorouteControl::via_rule` has to **own** its rule (see that field).
        let fallback_to_board_vias = router_settings
            .fanout
            .as_ref()
            .and_then(|f| f.fallback_to_board_vias)
            .unwrap_or(false);
        if fallback_to_board_vias && ctrl_settings.via_rule.is_some() {
            let net_class_rule = ctrl_settings
                .via_rule
                .clone()
                .expect("guarded by the `is_some` above");
            // :1026-1041.
            let combined_via_rule =
                combined_fallback_via_rule(&net_class_rule, &self.rules.via_rules);
            // :1042-1043.
            ctrl_settings.via_rule = Some(combined_via_rule);
            ctrl_settings.rebuild_via_info(self, router_settings.get_via_costs(), pin_net_no);
        }

        // :1045-1050.
        //
        // `this.components.get(pin.getComponentId())` runs **unconditionally** at `:1045`, before
        // `:1046`'s two-part test, and `Components.get` is `elementAt(componentId - 1)` with no
        // bounds check (Components.java:84-94) — so Java's `pinComponent != null` is dead: the
        // lookup either answers an object or throws. The port keeps the shape and lets
        // `Components::get` panic where Java throws; `getSmdPins()` only ever answers `Pin`s, and
        // a `Pin` is created with a 1-based component number, so neither happens.
        let component_name = self.components.get(pin_item.component_id()).name.clone();
        let pin_name = match self.get_item(pin) {
            Some(Item::Pin(p)) => p.name(&ctx).map(str::to_owned),
            _ => None,
        };
        ctrl_settings.fanout_start_pin_name = match pin_name {
            Some(name) => Some(format!("{component_name}-{name}")), // :1047
            None => Some(format!("{pin_item}")),                    // :1049
        };
        ctrl_settings.fanout_start_pin_center = Some(pin_center_of(self, pin)); // :1051
        ctrl_settings.fanout_start_pin_layer =
            i32::try_from(pin_layer).expect("a board layer index"); // :1052
        ctrl_settings.remove_unconnected_vias = false; // :1053
        // :1054-1057. "Ripup is allowed if ripupCosts >= 0" — `BatchFanout.java:183` passes `-1`
        // to switch it off.
        if ripup_costs >= 0 {
            ctrl_settings.ripup_allowed = true;
            ctrl_settings.ripup_costs = ripup_costs;
        }

        // :1058-1061. `retainAutorouteDatabase = false` (ruling AJ's value anyway).
        let mut ripped_item_list: BTreeSet<ItemId> = BTreeSet::new();
        *engine = Some(self.init_autoroute(
            engine.take(),
            pin_net_no,
            ctrl_settings.trace_clearance_class_index,
            time_limit,
            false,
        ));
        let autoroute_engine = engine
            .as_mut()
            .expect("initAutoroute always answers an engine");

        // :1063-1092.
        let mut result: Option<AutorouteAttemptResult> = None; // :1063
        if sorted_unconnected_list.len() <= 4 {
            // :1064-1085.
            if let Some(closest_target) = sorted_unconnected_list.first().copied() {
                // :1066-1074 — "try to route to the closest target first".
                let mut single_target = BTreeSet::new();
                single_target.insert(closest_target);
                let first = autoroute_engine.autoroute_connection(
                    self,
                    &pin_connected_set,
                    &single_target,
                    &ctrl_settings,
                    &mut ripped_item_list,
                    // `null` — "costs not needed here" (`:1073`).
                    None,
                    stop,
                );
                // :1076-1085 — "if that fails and we have other targets, fall back to searching
                // the entire unconnected set at once". Quirk #221: the **same** `rippedItemList`.
                let retry = first.state != AutorouteAttemptState::Routed
                    && first.state != AutorouteAttemptState::AlreadyConnected
                    && sorted_unconnected_list.len() > 1;
                result = Some(if retry {
                    autoroute_engine.autoroute_connection(
                        self,
                        &pin_connected_set,
                        &unconnected_set,
                        &ctrl_settings,
                        &mut ripped_item_list,
                        None,
                        stop,
                    )
                } else {
                    first
                });
            }
        } else {
            // :1086-1092.
            result = Some(autoroute_engine.autoroute_connection(
                self,
                &pin_connected_set,
                &unconnected_set,
                &ctrl_settings,
                &mut ripped_item_list,
                None,
                stop,
            ));
        }

        // :1093-1097. Reachable only through the `<= 4` arm's empty list, which
        // `:997-1001` has already excluded — so this is Java's belt-and-braces, kept because the
        // state it names has no other producer.
        let result = result.unwrap_or_else(|| {
            AutorouteAttemptResult::with_details(
                AutorouteAttemptState::Failed,
                "No target items to route connection.".to_string(),
            )
        });

        // :1099-1108. Note the **net-filtered** `new int[]{pinNetNo}`, unlike
        // `AutorouteConnectionRouter.route`'s step 6, which passes `new int[0]`.
        if result.state == AutorouteAttemptState::Routed {
            let trace_costs = ctrl_settings.trace_costs.clone();
            self.opt_changed_area(
                engine.as_mut(),
                &[pin_net_no],
                None,
                router_settings
                    .trace_pull_tight_accuracy
                    .expect("RoutingBoard.fanout:1103 unboxes tracePullTightAccuracy; null NPEs"),
                Some(&trace_costs),
                stop,
                budget.opt_changed_area_ms,
            )
            // The same reasoning as `route_connection_full`'s step 6: an `Err` here is Java
            // throwing out of the tightener family, and `BatchFanout` has no `catch` of its own —
            // `AutorouteBatchLoop.java:89-173` does not wrap the fanout in one either, so the
            // throw escapes the stage exactly as this panic does.
            .expect("optChangedArea throws out of RoutingBoard.fanout in Java too");
        }
        result // :1109
    }
}

/// `RoutingBoardOperations.java:18` — the pull-tight budget `removeItemsAndPullTight` hands
/// `optChangedArea`. **2000**, not the router's `TIME_LIMIT_TO_PREVENT_ENDLESS_LOOP = 1000`.
const PULL_TIGHT_TIME_LIMIT: i32 = 2000;

/// `RoutingBoard.insertForcedTracePolyline:536-542` and `:676-681`, which are the same four
/// lines twice: with a picked trace to combine with, the new polyline is combined with that
/// trace's own polyline; without one it is used as it is.
/// `RoutingBoard.fanout:1002-1021` — the unconnected set as a list, sorted by the **squared**
/// distance from `pin_center` to the midpoint of each item's bounding box.
///
/// Lifted out of [`RoutingBoardExt::fanout`] so that `crates/fr-router/tests/fanout_order.rs` can
/// reach the sort without routing a board; `fanout` is its only production caller, and the body is
/// Java's `Comparator` verbatim.
///
/// # Stability is load-bearing
///
/// `sortedUnconnectedList` starts as `new ArrayList<>(unconnectedSet)` (`:1003`), i.e. the
/// `TreeSet<Item>`'s iteration order, and `List.sort` is TimSort, which is **stable** — so two
/// targets at the same squared distance keep that order. The order is **descending id**, not
/// ascending: quirk **#44**, `Item.compareTo` is `item.id - id` (Item.java:95-103) and
/// `Item.getUnconnectedSet` (Item.java:676-690) collects into a plain `new TreeSet<>()`. A
/// [`BTreeSet`] iterates **ascending**, so the seed is `.iter().rev()` — the same rule
/// [`RoutingBoardExt::fanout`]'s `pin_connected_set` walk keeps a few lines earlier. On a tie the
/// **higher** id wins.
///
/// `sort_by` is Rust's stable sort, and `sort_unstable_by` here would be a silent divergence on
/// every symmetric board — as would seeding ascending, which is the shape this port shipped with
/// until the tie-break fix (see `crates/fr-router/tests/fanout_tie_break.rs`).
pub fn sorted_unconnected_targets(
    board: &Board,
    pin_center: &fr_geometry::FloatPoint,
    unconnected_set: &BTreeSet<ItemId>,
) -> Vec<ItemId> {
    let ctx = board.ctx();
    let dist_sq = |id: ItemId| -> f64 {
        // :1005-1013 / :1015-1017.
        let bx = board
            .get_item(id)
            .expect("an unconnected-set item")
            .bounding_box(&ctx);
        // `(box.ll.x + box.ur.x) / 2.0` — Java adds two `int`s and *then* divides, so the sum is
        // `int` arithmetic; board coordinates cannot overflow it.
        let cx = f64::from(bx.ll.x + bx.ur.x) / 2.0;
        let cy = f64::from(bx.ll.y + bx.ur.y) / 2.0;
        let dx = cx - pin_center.x;
        let dy = cy - pin_center.y;
        dx * dx + dy * dy
    };
    // Quirk #44: descending id, so the stable sort's tie-break matches Java's TreeSet seed.
    let mut list: Vec<ItemId> = unconnected_set.iter().rev().copied().collect();
    // `Double.compare` (`:1019`). Both operands are finite and non-negative here, so `total_cmp`
    // is that function; the two differ only for a **negative** NaN, which a sum of two squares
    // cannot be.
    list.sort_by(|item1, item2| dist_sq(*item1).total_cmp(&dist_sq(*item2)));
    list
}

/// `RoutingBoard.fanout:1026-1041` — the `fallbackToBoardVias` rule: a fresh `ViaRule` named
/// `<netClassRule.name>_fallback` holding the net class's vias, plus every via of
/// `rules.viaRules.firstElement()` the merge does not already hold.
///
/// Lifted out of [`RoutingBoardExt::fanout`] for the same reason as
/// [`sorted_unconnected_targets`], and because this is the site controller ruling AN names: `:1037`
/// is [`ViaRule::contains`], which is Java's `==` — **object identity** — so a
/// `.rules` file that re-declares a `(via …)` with identical values and then re-declares the
/// `(via_rule …)` naming it makes Java append a duplicate here. Quirk **#218**.
///
/// `board_via_rules` is `this.rules.viaRules`; `:1034`'s `isEmpty()` guard is what keeps
/// `Vector.firstElement()` from throwing, and the port's `first()` answers `None` in the same
/// case.
pub fn combined_fallback_via_rule(
    net_class_rule: &ViaRule,
    board_via_rules: &[ViaRule],
) -> ViaRule {
    // :1028-1032.
    let mut combined_via_rule = ViaRule::new(format!("{}_fallback", net_class_rule.name));
    for i in 0..net_class_rule.via_count() {
        combined_via_rule.append_via(net_class_rule.get_via(i).clone());
    }
    // :1033-1040.
    if let Some(default_via_rule) = board_via_rules.first() {
        for i in 0..default_via_rule.via_count() {
            let default_via = default_via_rule.get_via(i);
            // :1037 — object identity, not value equality: a value comparison would drop a via
            // the JVM appends.
            if !combined_via_rule.contains(default_via) {
                combined_via_rule.append_via(default_via.clone()); // :1038
            }
        }
    }
    combined_via_rule
}

/// `pin.getCenter()` (board/model/items/Pin.java:871-...) for a board item known to be a `Pin`.
/// Not a Java helper — Java calls the method on the `Pin` the signature already gives it, while
/// this port reaches the item through the board.
fn pin_center_of(board: &Board, pin: ItemId) -> Point {
    let ctx = board.ctx();
    match board.get_item(pin) {
        Some(Item::Pin(p)) => p.get_center(&ctx),
        _ => panic!("RoutingBoard.fanout takes a Pin"),
    }
}

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
/// obligation: `RoutingBoard.insertForcedTracePolyline`'s `ShapeEntrySide` index — **discharged
/// in Task 17**. Task 15b could not discriminate it: the two indices agreed on every unit
/// fixture, so changing this expression to `checkForcedTracePolyline`'s `i + 1` left all 1 621
/// rows of modes `poly`, `tail`, `seg`, `neck` and `rand` byte-identical. Task 17's corpus
/// separates them twice over. Instrumented, this expression **differs** from `i + 1` on 50 of 63
/// evaluations in `router-rpi-splitter`, 508 of 553 in `router-j2-reference`, 5 160 of 6 084 in
/// `router-dac2020-bm01` and 39 of 71 in `router-ecc83-input`; and mutating it to `i + 1` makes
/// `router-j2-reference` and `router-dac2020-bm01` DIFF against the HEAD jar while
/// `router-rpi-splitter` and `router-ecc83-input` still match. Probe mode `side` pins the index
/// values themselves.
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
