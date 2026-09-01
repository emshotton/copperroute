//! [`BatchOptimizer`] — the port of `autoroute/pipeline/BatchOptimizer.java` (660 lines), the
//! optimizer stage (Plan 7 Task 13, part A).
//!
//! Task 13 lands the **item half**: the type and its field block (`:29-38`), the
//! `createForHeadless` constructor (`:51-53`), `containsOnlyUnfixedTraces` (`:85-92`),
//! `optRouteItem` (`:395-514`), `getCurrentPosition` (`:520-525`), `calculateIncompleteCount`
//! (`:552-556`) and the protected inner class [`ReadSortedRouteItems`] (`:563-659`).
//! **Task 14** added the stage half in a second `impl` block: `runBatchLoop` (`:125-272`),
//! `optRoutePass` (`:279-385`), `normalizeAlgorithm` (`:68-78`) and the five `NamedAlgorithm`
//! identity constants (`:527-550`), with [`OptimizerResult`] and [`OptimizerPassRecord`] as their
//! answer. What is left of the class is one GUI factory, rostered at the foot of this file.
//!
//! # The snapshot is a clone (plan-7 ruling 8), and the clone is not free
//!
//! `optRouteItem` makes the board restorable at `:442-445` with `routingBoard.generateSnapshot()`
//! and either drops the snapshot (`:503`) or restores from it (`:509`) **inside the same call** —
//! nothing else touches the stack. `fr-board` rosters `RoutingBoard.{generateSnapshot, popSnapshot,
//! undo}` `// not ported:` and spec §6 replaces them with `Board: Clone`, so the port takes a
//! [`Board::deep_copy`] before the removal and assigns it back on failure.
//!
//! That substitution is **not** free, and the difference is named here rather than hidden.
//! `BasicBoard.undo` (`BasicBoard.java:1233-1240`) restores exactly two things — `components` and
//! `itemList`, with `applyUndoRedoSideEffects` fixing the search tree up to match — and leaves
//! every other field of the live board alone. A whole-board assignment additionally rolls back:
//!
//! | field | Java's `undo` | the clone | consequence |
//! |---|---|---|---|
//! | `communication.idGenerator` | untouched: the ids the failed attempt burned stay burned | rolled back | **compensated** — [`BatchOptimizer::opt_route_item`] carries the *live* `communication` onto the restored board, so the next inserted item gets the id Java gives it |
//! | `searchTreeManager` | mutated item by item (`BasicBoard.java:1262`, `:1276`) | replaced wholesale | **NOT inert — quirk #229, open.** The contents agree and the *shape* does not, and the shape leaks. The Task 13 review's §4 insulation argument covers `MinAreaTree.overlaps` only, whose result really is funnelled through an identity-ordered set; it does **not** cover `ShapeSearchTree45Degree.completeShape` (ShapeSearchTree45Degree.java:152-274), which walks the tree with an `ArrayStack` and prunes each node against a `boundingShape` that shrinks as obstacles are consumed (`:157` against `:263-264`). There, topology decides which obstacles restrain a room **at all**, so an "undone" board whose trees Java re-paired and the port restored intact completes free-space rooms differently. Measured on `Issue558-dev-board` and `Issue026-J2_reference`: the first failed `optRouteItem` costs Java 54 (resp. 84) tree ops the port never performs, and a later item burns a different number of ids. See `docs/java-quirks.md` #229 and `.superpowers/sdd/2026-08-30-plan-7-router-batch/task-14b-report.md`. |
//! | `revision` | keeps counting | rolled back | inert — `Board::revision` has no reader outside tests |
//! | `changedArea` | untouched | rolled back to the snapshot's `None` | inert — every consumer calls `startMarkingChangedArea()` first (`AutoroutePassRunner.java:223`, `BatchAutorouter.java:489`) |
//! | `maxTraceHalfWidth` / `minTraceHalfWidth` | keeps the widened value of a trace that no longer exists | rolled back | private in the port, so "keep live" is not expressible; the rolled-back value is the one that describes the restored board |
//!
//! # What is deliberately not here
//!
//! * the three JMX samplers (`:94-122`) — `sampleCurrentThreadCpuSeconds`,
//!   `sampleCurrentThreadAllocatedMb`, `sampleHeapUsageMb`; every reader is a `job.logInfo`
//!   string;
//! * `createForGui` (`:56-66`) — the GUI factory and the only path to
//!   `BatchOptimizerMultiThreaded`; the roster line at the foot of this file carries its three
//!   greps. (`normalizeAlgorithm` (`:68-78`), which it calls, **is** ported — Task 14.)
//! * `RoutingJob` (`:35`) — Plan 8's, together with the CLI that builds it.

use std::collections::BTreeSet;

use fr_board::items::Item;
use fr_board::{Board, ItemId, StopConnectionOption};
use fr_drc::DesignRulesChecker;
use fr_geometry::{FloatPoint, java_min, java_round};
use fr_settings::RouterSettings;

use crate::error::RouterError;
use crate::pipeline::batch_autorouter::BatchAutorouter;
use crate::pipeline::batch_loop::stat;
use crate::pipeline::counters::RouterCounters;
use crate::pipeline::fanout::{instant_offset_ms, parse_timespan_seconds};
use crate::pipeline::item_route_result::ItemRouteResult;
use crate::pipeline::stop::{PassRecord, ProgressThrottler, RouterBudget, RouterStop};
use crate::pipeline::{NamedAlgorithmType, ProgressSink, RoutingEvent, TaskState};
use crate::score::BoardStatistics;

// =================================================================================================
// `ReadSortedRouteItems` — BatchOptimizer.java:563-659
// =================================================================================================

/// Port of the protected inner class `BatchOptimizer.ReadSortedRouteItems`
/// (BatchOptimizer.java:563-659): "reads the vias and traces on the board in ascending x order.
/// Because the vias and traces on the board change while optimizing the item list of the board is
/// read from scratch each time the next route item is returned."
///
/// # It must not be memoised (plan-7 ruling 12)
///
/// [`ReadSortedRouteItems::next`] is a **full O(n) rescan of `board.itemList`, twice** — the vias
/// at `:577-604` and then the traces at `:606-650` — and it runs against the board that the
/// previous `optRouteItem` mutated. Caching the sequence would answer for a board that no longer
/// exists. `crates/fr-router/tests/optimizer_items.rs`'s
/// `the_rescan_sees_items_the_previous_call_moved` is the pin.
///
/// # The field set is Java's, and it is two fields, not four
///
/// `:565-566` declares `minItemCoor` and `minItemLayer` and nothing else; `currentMinCoor` and
/// `currentMinLayer` are **locals** of `next()` (`:575-576`) that are copied into the two fields
/// at `:651-652`. So "strictly after" is measured from the same pair that
/// [`ReadSortedRouteItems::get_current_position`] reports, and the plan's sketch — which described
/// a third, separately-held "previously returned position" — is one field too many.
///
/// # The cursor advances even when nothing is returned
///
/// `:651-652` is **unconditional**. When the scan finds nothing, `currentMinCoor` is still
/// `(Integer.MAX_VALUE, Integer.MAX_VALUE)` and `currentMinLayer` is still `Integer.MAX_VALUE`,
/// and those are written into the cursor — so an exhausted reader stays exhausted, because no
/// coordinate is strictly greater than the maximum. The brief's "the cursor advances only when an
/// item is returned" is the one sentence Java contradicts; the behaviour is the same in the case
/// that matters and different in the terminal one, and
/// `an_exhausted_reader_parks_the_cursor_at_the_maximum` pins it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReadSortedRouteItems {
    /// `protected FloatPoint minItemCoor` (`:565`) — the cursor's coordinate.
    pub min_item_coor: FloatPoint,
    /// `protected int minItemLayer` (`:566`) — the cursor's layer, the third comparison key.
    pub min_item_layer: i32,
}

impl Default for ReadSortedRouteItems {
    fn default() -> ReadSortedRouteItems {
        ReadSortedRouteItems::new()
    }
}

impl ReadSortedRouteItems {
    /// Port of the constructor `ReadSortedRouteItems()` (`:568-571`).
    ///
    /// The seed is `new FloatPoint(Integer.MIN_VALUE, Integer.MIN_VALUE)` — the **`int`**
    /// extremes widened to `double`, not `Double.NEGATIVE_INFINITY` — and layer `-1`, which is
    /// one below the first layer index.
    #[must_use]
    pub fn new() -> ReadSortedRouteItems {
        ReadSortedRouteItems {
            // :569.
            min_item_coor: FloatPoint {
                x: f64::from(i32::MIN),
                y: f64::from(i32::MIN),
            },
            // :570.
            min_item_layer: -1,
        }
    }

    /// Port of `next()` (`:573-654`): the lexicographically-next `(x, y, layer)` strictly after
    /// the cursor, over the unfixed vias and the un-shove-fixed traces of the board.
    ///
    /// # Two scans, and the tie the second one loses
    ///
    /// The via scan (`:577-604`) runs first and seeds `currentMinCoor`/`currentMinLayer`; the
    /// trace scan (`:606-650`) then compares against the **same** locals, and its test is a strict
    /// `<`. So a trace at exactly a via's `(x, y, layer)` never wins, which is what the comment at
    /// `:605` — "read traces last to prefer vias to traces at the same location" — is describing.
    /// Both scans walk `board.itemList.startReadObject()`, i.e. **descending item id** (quirk #63).
    ///
    /// # The two "fixed" predicates are different predicates
    ///
    /// `:584` asks a via `isUserFixed()` and `:613` asks a trace `isShoveFixed()`. The second is
    /// `Trace`'s override (`Trace.java:236-254`), which additionally answers `true` for any trace
    /// on a net whose class is shove-fixed — so a *user*-fixed via is skipped while a trace is
    /// skipped for a strictly wider reason. Transcribed as Java writes it.
    ///
    /// # A trace's key is its **larger** endpoint
    ///
    /// `:617-622` picks `compareCorner` as the corner that is *not* the lexicographic minimum of
    /// the two, so the trace is keyed by where it ends rather than where it starts. The `<` chain
    /// is on `FloatPoint` `double`s and is transcribed literally.
    ///
    /// # The via-contact veto does not move the cursor
    ///
    /// `:633-645` computes `isConnectedToVia` **inside** the "is a new minimum" branch and, when
    /// it answers `true`, falls through without assigning `currentMinCoor`. So a trace touching an
    /// unfixed via is not merely skipped as a candidate — it also leaves the running minimum
    /// where it was, and a *later* trace between it and that minimum can still win. Reproduced by
    /// keeping the assignment inside the `if`, exactly as Java has it.
    #[must_use]
    pub fn next(&mut self, board: &Board) -> Option<ItemId> {
        // :574.
        let mut result: Option<ItemId> = None;
        // :575-576.
        let mut current_min_coor = FloatPoint {
            x: f64::from(i32::MAX),
            y: f64::from(i32::MAX),
        };
        let mut current_min_layer: i32 = i32::MAX;

        let ctx = board.ctx();

        // :577-604 — the via scan.
        for id in board.items_in_board_order() {
            let Some(item) = board.get_item(id) else {
                continue;
            };
            // :583.
            let Item::Via(current_via) = item else {
                continue;
            };
            // :584.
            if item.is_user_fixed() {
                continue;
            }
            // :585-586.
            let current_via_center = current_via.get_center().to_float();
            let current_via_min_layer = layer_index(current_via.first_layer(&ctx));
            // :587-591 — strictly after the cursor.
            if current_via_center.x > self.min_item_coor.x
                || current_via_center.x == self.min_item_coor.x
                    && (current_via_center.y > self.min_item_coor.y
                        || current_via_center.y == self.min_item_coor.y
                            && current_via_min_layer > self.min_item_layer)
            {
                // :592-596 — and a new minimum among those.
                if current_via_center.x < current_min_coor.x
                    || current_via_center.x == current_min_coor.x
                        && (current_via_center.y < current_min_coor.y
                            || current_via_center.y == current_min_coor.y
                                && current_via_min_layer < current_min_layer)
                {
                    // :597-599.
                    current_min_coor = current_via_center;
                    current_min_layer = current_via_min_layer;
                    result = Some(id);
                }
            }
        }

        // :605-650 — "read traces last to prefer vias to traces at the same location".
        for id in board.items_in_board_order() {
            let Some(item) = board.get_item(id) else {
                continue;
            };
            // :612.
            let Item::Trace(current_trace) = item else {
                continue;
            };
            // :613 — `Trace.isShoveFixed`, not `Item.isShoveFixed`; see the doc comment.
            if item.is_shove_fixed(&board.rules) {
                continue;
            }
            // :614-615. Neither corner can be absent on a board trace: `BasicBoard.java:185-187`
            // refuses to insert a polyline with fewer than two corners, which is the same
            // argument `BatchAutorouter::impacted_points` records.
            let first_corner = current_trace
                .first_corner()
                .expect("a board trace has at least two corners (BasicBoard.java:185-187)")
                .to_float();
            let last_corner = current_trace
                .last_corner()
                .expect("a board trace has at least two corners (BasicBoard.java:185-187)")
                .to_float();
            // :616-622.
            let compare_corner = if first_corner.x < last_corner.x
                || first_corner.x == last_corner.x && first_corner.y < last_corner.y
            {
                last_corner
            } else {
                first_corner
            };
            // :623.
            let current_trace_layer = layer_index(current_trace.get_layer());
            // :624-627.
            if compare_corner.x > self.min_item_coor.x
                || compare_corner.x == self.min_item_coor.x
                    && (compare_corner.y > self.min_item_coor.y
                        || compare_corner.y == self.min_item_coor.y
                            && current_trace_layer > self.min_item_layer)
            {
                // :628-632 — strict `<` against the locals the via scan left behind.
                if compare_corner.x < current_min_coor.x
                    || compare_corner.x == current_min_coor.x
                        && (compare_corner.y < current_min_coor.y
                            || compare_corner.y == current_min_coor.y
                                && current_trace_layer < current_min_layer)
                {
                    // :633-640. `getNormalContacts()` is a `TreeSet<Item>`, i.e. descending id
                    // (quirk #44); the walk `break`s on the first match, so the order decides
                    // which contact answers but not the answer.
                    let mut is_connected_to_via = false;
                    for current_contact in board.normal_contacts(id).into_iter().rev() {
                        let Some(contact) = board.get_item(current_contact) else {
                            continue;
                        };
                        if matches!(contact, Item::Via(_)) && !contact.is_user_fixed() {
                            is_connected_to_via = true;
                            break;
                        }
                    }
                    // :641-645.
                    if !is_connected_to_via {
                        current_min_coor = compare_corner;
                        current_min_layer = current_trace_layer;
                        result = Some(id);
                    }
                }
            }
        }

        // :651-652 — unconditional; see the doc comment.
        self.min_item_coor = current_min_coor;
        self.min_item_layer = current_min_layer;
        // :653.
        result
    }

    /// Port of `getCurrentPosition()` (`:656-658`) — the cursor, i.e. the coordinate of the item
    /// the previous [`ReadSortedRouteItems::next`] returned.
    #[must_use]
    pub fn get_current_position(&self) -> FloatPoint {
        // :657.
        self.min_item_coor
    }
}

/// Java's layer indices are `int`s throughout (`Via.firstLayer`, `Trace.getLayer`) and the port's
/// are `usize`; the comparison chain in [`ReadSortedRouteItems::next`] is signed, because the
/// cursor's initial layer is `-1` (`:570`).
fn layer_index(layer: usize) -> i32 {
    i32::try_from(layer).expect("a board layer index fits in an i32")
}

// =================================================================================================
// `BatchOptimizer` — BatchOptimizer.java:26-659
// =================================================================================================

/// Port of `autoroute.pipeline.BatchOptimizer` (BatchOptimizer.java:26-659, in a 660-line file) —
/// "optimizes routes using a single thread on a board that has completed auto-routing."
///
/// **Declared here** rather than in Task 14 (scan ruling 7): this task writes `&mut self` methods
/// on it. Task 14 adds `run_batch_loop`/`opt_route_pass` as an `impl` extension and declares no
/// type but its result.
///
/// # The board is a parameter, not a field
///
/// `NamedAlgorithm.board` (`NamedAlgorithm.java:38`) is a field; the port threads one `Board` as
/// `&mut Board` exactly as [`BatchAutorouter`] does, and for the same reason. The one place it
/// shows is [`ReadSortedRouteItems::next`], which reads `board` off the enclosing instance in Java
/// (the inner class is non-`static`) and takes it as an argument here.
#[derive(Debug)]
pub struct BatchOptimizer<'a> {
    // -- NamedAlgorithm's one surviving field (NamedAlgorithm.java:32) --------------------------
    /// `NamedAlgorithm.settings` (`NamedAlgorithm.java:32`).
    pub settings: &'a RouterSettings,

    // -- BatchOptimizer.java:29-38 -------------------------------------------------------------
    /// `protected final ProgressThrottler progressThrottler = new ProgressThrottler(1000)`
    /// (`:29`).
    pub progress_throttler: ProgressThrottler,
    /// `protected ReadSortedRouteItems sortedRouteItems` (`:30`) — `null` outside a pass, which
    /// is what [`BatchOptimizer::get_current_position`] tests at `:521`.
    pub sorted_route_items: Option<ReadSortedRouteItems>,
    /// `protected boolean useIncreasedRipupCosts` (`:32`) — "in the first passes the ripup costs
    /// are increased for better performance." Written by `runBatchLoop` (Task 14) and read by
    /// [`BatchOptimizer::opt_route_item`] at `:455`.
    pub use_increased_ripup_costs: bool,
    /// `protected double minCumulativeTraceLength = 0.0` (`:34`) — "the minimum cumulative trace
    /// length that was reached during the optimization". Seeded per pass by `optRoutePass:288`
    /// (Task 14) and lowered at `:498-499`.
    pub min_cumulative_trace_length: f64,
    /// `protected int totalItemsOptimized` (`:36`).
    pub total_items_optimized: i32,
    /// `protected Long deadlineMs` (`:37`), set by `runBatchLoop` (`:153-159`) from
    /// `settings.optimizer.timeoutString` and read at `:172` and `:308`.
    ///
    /// # This is **not** the stop flag (Task 4's documented split)
    ///
    /// `:174` and `:310` write [`BatchOptimizer::is_timed_out`] and `break`/`return`; neither
    /// calls `requestStop` nor `requestStopAutoRouter` — `grep -n requestStop
    /// BatchOptimizer.java` is empty. So an optimizer timeout ends the optimizer stage and
    /// nothing else, and the port must **not** call
    /// [`RouterStop::poll_deadline`](crate::pipeline::RouterStop::poll_deadline) at those two
    /// sites, which requests `ALL`. Task 13 declared this field, and **Task 14 landed the two
    /// reads** — [`BatchOptimizer::run_batch_loop`] at `:172-176` and
    /// [`BatchOptimizer::opt_route_pass`] at `:308-313`, both through
    /// [`BatchOptimizer::is_deadline_reached`] — which closes `pipeline::stop`'s `obligation:`
    /// for `BatchOptimizer`. `BatchFanout`'s half was discharged in Task 12.
    ///
    /// A monotonic [`std::time::Instant`] rather than Java's epoch milliseconds, for the reason
    /// [`BatchFanout::deadline`](crate::pipeline::BatchFanout::deadline) records.
    pub deadline: Option<std::time::Instant>,
    /// `protected boolean isTimedOut` (`:38`) — the per-stage flag of the field above.
    pub is_timed_out: bool,
    // added in Plan 8: `NamedAlgorithm.job` / `BatchOptimizer.job` (`BatchOptimizer.java:35`) —
    // `core/RoutingJob`, the CLI/MCP job record; spec §13 puts it in Plan 8's `fr-core`. Its only
    // readers on this task's path are `job.logWarning` (`:399`) and `job.logInfo`, which
    // `global-constraints.md` drops with the rest of `FRLogger`.
    // not ported: the three listener lists of `NamedAlgorithm` (`:26-31`) — controller ruling AK
    // replaces them with `crate::pipeline::ProgressSink`.
    // not ported: `BatchOptimizer.sampleCurrentThreadCpuSeconds` (`:94-101`), `BatchOptimizer.sampleCurrentThreadAllocatedMb` (`:103-111`), `BatchOptimizer.sampleHeapUsageMb` (`:113-122`) — the three JMX samplers; every reader is a `job.logInfo` payload (`:255-272`), which the Global Constraints drop.
}

impl<'a> BatchOptimizer<'a> {
    /// Port of `createForHeadless(RoutingJob)` (`:51-53`) and, through it, of the constructor
    /// `BatchOptimizer(RoutingJob)` (`:45-48`).
    ///
    // renamed: `BatchOptimizer.createForHeadless` (`:51-53`) -> `BatchOptimizer::new` — the
    // factory and the constructor collapse, because `RoutingJob` is Plan 8's and the three values
    // `:46` reads off it are the thread (Task 4's `RouterStop`, a per-call parameter here), the
    // board (a per-call parameter) and the settings (the one field).
    ///
    /// The headless factory **always** answers the single-threaded implementation; the only path
    /// to `BatchOptimizerMultiThreaded` is `createForGui` (`:56-66`), which Task 14 rosters.
    #[must_use]
    pub fn new(settings: &'a RouterSettings) -> BatchOptimizer<'a> {
        BatchOptimizer {
            settings,
            // :29.
            progress_throttler: ProgressThrottler::new(1000),
            // :30.
            sorted_route_items: None,
            // :32 — Java's `boolean` default; `runBatchLoop:132` sets it `true`.
            use_increased_ripup_costs: false,
            // :34.
            min_cumulative_trace_length: 0.0,
            // :36.
            total_items_optimized: 0,
            // :37.
            deadline: None,
            // :38.
            is_timed_out: false,
        }
    }

    /// Port of `isTimedOut()` (`:81-83`) — the per-stage flag, never the job stop flag.
    #[must_use]
    pub fn is_timed_out(&self) -> bool {
        // :82.
        self.is_timed_out
    }

    /// Port of `containsOnlyUnfixedTraces(Collection<Item>)` (`:85-92`): "true when every item of
    /// the collection is a `Trace` and none of them is user-fixed" — the guard `optRouteItem`
    /// puts in front of adding a trace's fork items to the ripped set (`:421`).
    ///
    /// Java's parameter is a `Collection<Item>` of live references and the port's is a set of
    /// ids; an id the board does not have is skipped rather than dereferenced, which is the
    /// `Board::remove_items` `// totalized:` precedent. No caller can produce one — both call
    /// sites hand it a contact set the board just built.
    ///
    /// Vacuously `true` on the empty set, which is Java's answer and the one that matters: a
    /// trace with no start contacts takes the `addAll` branch with nothing to add.
    #[must_use]
    pub fn contains_only_unfixed_traces(board: &Board, item_list: &BTreeSet<ItemId>) -> bool {
        // :86 — a `TreeSet<Item>`, i.e. descending id (quirk #44). The walk short-circuits, so
        // the order decides which item answers but not the answer.
        for current_item in item_list.iter().rev() {
            let Some(item) = board.get_item(*current_item) else {
                continue;
            };
            // :87-89.
            if item.is_user_fixed() || !matches!(item, Item::Trace(_)) {
                return false;
            }
        }
        // :91.
        true
    }

    /// Port of the private `calculateIncompleteCount(RoutingBoard)` (`:552-556`) — a throw-away
    /// [`DesignRulesChecker`] over the whole board, `calculateAllIncompletes()`, then
    /// `getIncompleteCount()`.
    ///
    /// Character for character `BatchAutorouter.calculateIncompleteCount` (`:556-564`), which the
    /// port already has as [`BatchAutorouter::calculate_incomplete_count`]; it is transcribed
    /// again here because `audit-port.sh` is per class and a reader looking for `:552` must find
    /// it in this file. Task 14's brief lists it; it lands here because `optRouteItem` (`:406`,
    /// `:479`) is its first caller.
    #[must_use]
    pub fn calculate_incomplete_count(board: &mut Board) -> usize {
        // :553.
        let mut temp_drc = DesignRulesChecker::new(board);
        // :554.
        temp_drc.calculate_all_incompletes();
        // :555.
        temp_drc.get_incomplete_count()
    }

    /// Port of `getCurrentPosition()` (`:520-525`): "the current position of the item, which will
    /// be rerouted or null, if the optimizer is not active."
    #[must_use]
    pub fn get_current_position(&self) -> Option<FloatPoint> {
        // :521-523, :524.
        self.sorted_route_items
            .as_ref()
            .map(ReadSortedRouteItems::get_current_position)
    }

    /// Port of `optRouteItem(Item, boolean, boolean)` (`:395-514`): "try to improve the route by
    /// re-routing the connections containing item" — rip one item's connections out, re-route
    /// them with the optimizer's own autorouter, and keep the result only when the
    /// [`ItemRouteResult`] says it improved.
    ///
    /// # `disable_snapshots` is `false` on every path this port has
    ///
    /// It is Java's parameter (`:396`), `false` from `optRoutePass:331` and `true` only from
    /// `OptimizeRouteTask.java:46`, which is constructed solely by `BatchOptimizerMultiThreaded`,
    /// i.e. by `createForGui`. The parameter is kept so that the audit sees the method with the
    /// signature Java gives it, and so that a Plan 8 GUI caller has the door.
    ///
    /// # Two values are read off the item **before** it is removed
    ///
    /// `:449-450` calls `item.netCount()`/`item.getNetNumber(i)` and `:460` asks
    /// `item instanceof Trace` — both **after** `:448` has removed the item from the board.
    /// Java's `Item` object outlives its removal from `itemList`, so those reads still answer; the
    /// port keys items by id and `Board::remove_items` really deletes, so the net numbers and the
    /// kind are captured before the removal and the reads are answered from the captures. Same
    /// program, different mechanism — and `:487`'s `item.getId()` is the parameter here.
    ///
    /// # `settings.tracePullTightAccuracy` is dereferenced without a guard
    ///
    /// `:470` passes the `Integer` straight into an `int` parameter. Unlike
    /// `BatchAutorouter`'s constructor (`:118-120`), there is **no** `null` fallback here, so a
    /// settings table without the field NPEs in Java. The `expect` reproduces that.
    ///
    /// # The user-fixed early exit is dead (quirk #226)
    ///
    /// `:436-440` is transcribed and can never fire; the marker at the site has the argument.
    /// `p7t8 item`'s `anyUserFixed` column reads `false` on every item of every corpus stem.
    ///
    /// # `progress`, which the brief's sketch omits
    ///
    /// Java fires two throttled `fireBoardUpdatedEvent`s from this method (`:407-409`,
    /// `:480-482`) and hands `job` down to `autoroutePassesForOptimizingItem`, whose pass runner
    /// fires more. Controller ruling AK routes all of them through
    /// [`ProgressSink`](crate::pipeline::ProgressSink), so the sink has to be a parameter; the
    /// brief's signature has no way to reach one. Task 14's own sketch for `optRoutePass` carries
    /// it, so the two agree once this is added.
    ///
    /// # `pub`, not the brief's `pub(crate)`
    ///
    /// The brief declares both this method and
    /// [`BatchOptimizer::contains_only_unfixed_traces`] `pub(crate)`, which no test and no driver
    /// can reach: `crates/fr-router/tests/optimizer_items.rs` and
    /// `scripts/differential/rust/src/bin/p7t8.rs` are separate crates. They are `pub` for the
    /// same reason [`BatchAutorouter::autoroute_items_with_handled`] is — an observation seam the
    /// JVM-pinned evidence needs — and Java's own modifiers are `protected` and package-private,
    /// i.e. reachable from `P7T8.java` in the same package.
    ///
    /// # The `improved` verdict is re-taken against the **`ALL`** stop
    ///
    /// `:494` is `!this.thread.isStopRequested() && result.improved()` —
    /// [`RouterStop::is_stop_requested`], not the auto-router one. A run stopped for the router
    /// only therefore still commits an improvement here, which is the asymmetry quirk #202
    /// records at its other end.
    #[allow(clippy::too_many_arguments)]
    pub fn opt_route_item(
        &mut self,
        board: &mut Board,
        item: ItemId,
        with_preferred_directions: bool,
        disable_snapshots: bool,
        stop: &RouterStop,
        budget: RouterBudget,
        progress: &mut dyn ProgressSink,
    ) -> Result<ItemRouteResult, RouterError> {
        // not reachable: `:398-401` — `if (!(item.board instanceof RoutingBoard routingBoard))`.
        // The port has one `Board` type and takes it as a parameter, so there is no cast to fail
        // and no `new ItemRouteResult(item.getId())` to answer with.

        // :404-406.
        let board_statistics_before = BoardStatistics::with_options(board, None, false);
        let incomplete_count_before =
            count_as_i32(BatchOptimizer::calculate_incomplete_count(board));
        // :407-409.
        if self.progress_throttler.should_update() {
            progress.on_event(&RoutingEvent::BoardUpdated {
                counters: RouterCounters {
                    incomplete_count: Some(incomplete_count_before),
                    ..RouterCounters::default()
                },
            });
        }

        // :412-413.
        let mut ripped_items: BTreeSet<ItemId> = BTreeSet::new();
        ripped_items.insert(item);

        // The two reads `:449-450` and `:460` make against the removed item; see the doc comment.
        let item_net_numbers: Vec<i32> = board.get_item(item).map_or_else(Vec::new, |it| {
            (0..it.net_count()).map(|i| it.get_net_number(i)).collect()
        });
        let item_is_trace = matches!(board.get_item(item), Some(Item::Trace(_)));

        // :416-425. "Add also the fork items, especially because not all fork items may be
        // returned by ReadSortedRouteItems because of matching end points."
        if item_is_trace {
            // :419.
            let mut current_contact_list = board.trace_start_contacts(item);
            // :420-425. Two turns of the loop: the start contacts, then the end contacts. The
            // trailing reassignment on the second turn is Java's and is dead.
            for _ in 0..2 {
                // :421-423.
                if BatchOptimizer::contains_only_unfixed_traces(board, &current_contact_list) {
                    ripped_items.extend(current_contact_list.iter().copied());
                }
                // :424.
                current_contact_list = board.trace_end_contacts(item);
            }
        }

        // :428-432 — a `TreeSet<Item>`, walked descending (quirk #44).
        let mut ripped_connections: BTreeSet<ItemId> = BTreeSet::new();
        for current_item in ripped_items.iter().rev() {
            ripped_connections
                .extend(board.connection_items(*current_item, StopConnectionOption::None));
        }

        // :434-440 — "check if the connections contain user fixed items, which should not be
        // re-routed". **It cannot fire — quirk #226.** `rippedConnections` is filled from nothing
        // but `getConnectionItems` (`:428-432`), which adds an item only when `isRoutable()`
        // (`Item.java:701-703` for the start item, `:723-726` for every step of the walk), and
        // `Trace.isRoutable` (Trace.java:206-208) / `Via.isRoutable` (Via.java:147-149) are both
        // `!isUserFixed() && netCount() > 0` over a base that answers `false`. So every member is
        // routable, therefore not user-fixed. Transcribed anyway, because the port must read like
        // the method and because a Java change to `isRoutable` would make it live.
        // Java bug: `BatchOptimizer.optRouteItem` (`:434-440`) — the user-fixed guard is dead: `getConnectionItems` only ever collects `isRoutable()` items and neither a user-fixed trace nor a user-fixed via is one, so the fixed geometry it advertises protecting is never in the set (quirk #226).
        for current_item in ripped_connections.iter().rev() {
            if board
                .get_item(*current_item)
                .is_some_and(Item::is_user_fixed)
            {
                // :438.
                return Ok(ItemRouteResult::unimproved(item));
            }
        }

        // :442-445 — `routingBoard.generateSnapshot()`. Plan-7 ruling 8: the snapshot is a clone.
        // See the module doc for what a clone rolls back that Java's `undo` does not.
        let snapshot = if disable_snapshots {
            None
        } else {
            Some(board.deep_copy())
        };

        // :448 — descending, which is the order the `TreeSet` hands `removeItems`.
        let removal_order: Vec<ItemId> = ripped_connections.iter().rev().copied().collect();
        board.remove_items(removal_order);
        // :449-451.
        for net_number in &item_net_numbers {
            board.combine_traces(*net_number)?;
        }

        // :453-463 — lifted out; see [`optimizer_ripup_costs`].
        let ripup_costs =
            optimizer_ripup_costs(self.settings, self.use_increased_ripup_costs, item_is_trace);

        // :465-473.
        let optimizer = self.settings.optimizer.as_ref().expect(
            "BatchOptimizer.optRouteItem: settings.optimizer is dereferenced at :456 without a \
             null check — Java throws a NullPointerException here too",
        );
        let max_autoroute_passes = optimizer
            .max_autoroute_passes
            .expect("optimizer.maxAutoroutePasses is unboxed at :468");
        let trace_pull_tight_accuracy = self.settings.trace_pull_tight_accuracy.expect(
            "BatchOptimizer.optRouteItem: settings.tracePullTightAccuracy is unboxed at :470 \
             with no null fallback — Java throws a NullPointerException here too",
        );
        BatchAutorouter::autoroute_passes_for_optimizing_item(
            board,
            self.settings,
            max_autoroute_passes,
            ripup_costs,
            trace_pull_tight_accuracy,
            with_preferred_directions,
            stop,
            budget,
            progress,
        )?;

        // :475-482.
        let board_statistics_after = BoardStatistics::with_options(board, None, false);
        let incomplete_count_after =
            count_as_i32(BatchOptimizer::calculate_incomplete_count(board));
        if self.progress_throttler.should_update() {
            progress.on_event(&RoutingEvent::BoardUpdated {
                counters: RouterCounters {
                    incomplete_count: Some(incomplete_count_after),
                    ..RouterCounters::default()
                },
            });
        }

        // :484-493. `traces.totalLength` is a `Float` in Java and the constructor's parameter is
        // a `double`, so the widening is Java's own.
        let mut result = ItemRouteResult::new(
            item,
            board_statistics_before.items.via_count.unwrap_or(0),
            board_statistics_after.items.via_count.unwrap_or(0),
            self.min_cumulative_trace_length,
            f64::from(board_statistics_after.traces.total_length.unwrap_or(0.0)),
            incomplete_count_before,
            incomplete_count_after,
        );
        // :494-495 — the `ALL` stop; see the doc comment.
        let route_improved = !stop.is_stop_requested() && result.improved();
        result.update_improved(route_improved);

        if route_improved {
            // :498-499.
            self.min_cumulative_trace_length = java_min(
                self.min_cumulative_trace_length,
                f64::from(
                    board_statistics_after
                        .traces
                        .total_weighted_length
                        .unwrap_or(0.0),
                ),
            );
            // :501-504 — `popSnapshot()`, i.e. drop the clone.
            drop(snapshot);
        } else if let Some(mut restored) = snapshot {
            // :506-510 — `routingBoard.undo(null)`. The clone carries the board back, but the id
            // generator does **not** go back: Java's `undo` touches `components` and `itemList`
            // only, so the ids the failed attempt burned stay burned and the next inserted item
            // gets the id Java gives it. See the module doc's table.
            restored.communication = board.communication.clone();
            // Java bug in the *port*, not in Java: quirk #229. This assignment also replaces
            // `board.trees`, so the search trees come back with the topology they had before the
            // attempt; Java's `undo` instead replays the attempt's item changes through the live
            // trees (`BasicBoard.java:1262`, `:1276`), which re-pairs the same leaves into a
            // different `MinAreaTree` shape. `ShapeSearchTree45Degree.completeShape:152-274`
            // reads that shape, so from the first failed item on, the two sides complete
            // free-space rooms differently and eventually burn different numbers of item ids.
            // Localised by Task 14b (ruling AZ); the fix is not a one-liner and re-opens every
            // optimizer-stage reference. See the module doc's table and `docs/java-quirks.md`.
            *board = restored;
        }

        // :513.
        Ok(result)
    }
}

/// `optRouteItem`'s ripup-cost ladder (`BatchOptimizer.java:453-463`), lifted out of the method.
///
/// # Why this is a function
///
/// Task 11's and Task 12's precedent (`fanout_ripup_costs`, `fanout_pin_can_use_vias`,
/// `FanoutLoopState`): a pure function of data a test can build, with cases **no corpus board
/// reaches**. Every `DefaultSettings` run has `additionalRipupCostFactorAtStart = 10` and
/// `traceRipupCostFactor = 0.6f` (`settings/sources/DefaultSettings.java:139-140`), so the only
/// values `p7t8` can show are 1 000 and 600. The rounding this reproduces is visible only at a
/// factor that lands the product on a half, and a **negative** one is where `Math.round` and
/// Rust's `f64::round` actually disagree — neither is reachable through a settings file.
///
/// # Three things the two lines decide
///
/// 1. `settings.getStartRipupCosts()` (`:454`) is the base, **not** the router's own
///    `startRipupCosts` field — the optimizer re-reads the setting.
/// 2. `useIncreasedRipupCosts` multiplies by an `Integer` (`:456`), unboxed; Java's `int *=`
///    wraps on overflow and [`i32::wrapping_mul`] says so rather than panicking in a debug build.
/// 3. `traceRipupCostFactor` is a **`Float`** (`OptimizerSettings.java:63`) and `:462` writes
///    `factor * (double) ripupCosts`, so the `float` is widened to `double` **after** its own
///    rounding: `0.6f` enters the product as `0.60000002384185791015625`, not as `0.6`. The port
///    goes through `f64::from(f32)` for exactly that reason. `Math.round` is half-up
///    ([`java_round`]) and its `long` is narrowed by an `(int)` cast, which is Rust's `as i32`.
#[must_use]
pub fn optimizer_ripup_costs(
    settings: &RouterSettings,
    use_increased_ripup_costs: bool,
    item_is_trace: bool,
) -> i32 {
    let optimizer = settings.optimizer.as_ref().expect(
        "BatchOptimizer.optRouteItem: settings.optimizer is dereferenced at :456 without a null \
         check — Java throws a NullPointerException here too",
    );
    // :454.
    let mut ripup_costs = settings.get_start_ripup_costs();
    // :455-457.
    if use_increased_ripup_costs {
        ripup_costs = ripup_costs.wrapping_mul(
            optimizer
                .additional_ripup_cost_factor_at_start
                .expect("optimizer.additionalRipupCostFactorAtStart is unboxed at :456"),
        );
    }
    // :459-463 — "reduce the ripup costs for traces".
    if item_is_trace {
        let trace_ripup_cost_factor = optimizer
            .trace_ripup_cost_factor
            .expect("optimizer.traceRipupCostFactor is unboxed at :462");
        ripup_costs =
            java_round(f64::from(trace_ripup_cost_factor) * f64::from(ripup_costs)) as i32;
    }
    ripup_costs
}

/// `BoardStatistics`/`DesignRulesChecker` answer `usize` where Java's fields are `int`.
fn count_as_i32(value: usize) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

// =================================================================================================
// `runBatchLoop` and `optRoutePass` — BatchOptimizer.java:125-272, :279-385
// =================================================================================================

/// What [`BatchOptimizer::run_batch_loop`] (`BatchOptimizer.java:125-272`) leaves behind.
///
/// renamed: Java's `runBatchLoop` is `void` and writes its answer into three places the port has
/// no equivalent for — the `TaskStateChangedEvent` at `:233-234`, the `job.logInfo` payload at
/// `:256-271`, and the two fields `totalItemsOptimized`/`isTimedOut`. The record is the port's
/// shape and every field names the Java site it is read from:
///
/// | field | Java source |
/// |---|---|
/// | [`Self::state`] | `:252-255`'s `completionStatus` ternary — see below |
/// | [`Self::passes_run`] | `currentPass` as the loop left it, which is what `:234` carries |
/// | [`Self::items_optimized`] | `totalItemsOptimized` (`:36`, incremented at `:332`) |
/// | [`Self::timed_out`] | `isTimedOut` (`:38`), i.e. [`BatchOptimizer::is_timed_out`] |
/// | [`Self::per_pass`] | ruling 1(a)'s per-pass ladder — see [`OptimizerPassRecord`] |
///
/// # `state` is the log line's three-way, **not** the event's
///
/// The `TaskStateChangedEvent` at `:233-234` is fired **unconditionally** with
/// [`TaskState::Finished`], whatever ended the loop — a timeout and a user cancel both report
/// `FINISHED` here, unlike `AutorouteBatchLoop`'s `:571-585`, which branches. The only place
/// Java distinguishes the three cases is the `completionStatus` string at `:252-255`
/// (`"completed with timeout:"` / `"interrupted:"` / `"completed:"`), which is a `job.logInfo`
/// payload. [`Self::state`] carries that three-way, because a caller that cannot tell a timeout
/// from a clean finish has lost the only thing `isTimedOut` exists to say; the *event* stays
/// Java's unconditional `FINISHED`, and `the_finished_event_fires_whatever_ended_the_loop` pins
/// both halves.
#[derive(Debug, Clone, PartialEq)]
pub struct OptimizerResult {
    /// `:252-255`'s `completionStatus`, as a [`TaskState`]: [`TaskState::TimedOut`] when
    /// `isTimedOut`, else [`TaskState::Cancelled`] when the stop flag is `ALL`, else
    /// [`TaskState::Finished`].
    pub state: TaskState,
    /// `currentPass` as the loop left it (`:166`, `:177`). **`++currentPass` happens before the
    /// near-perfect exit** (`:177` then `:182-193`), so a run that stops there reports one pass
    /// more than it ran and [`Self::per_pass`] is one entry shorter.
    pub passes_run: i32,
    /// `totalItemsOptimized` (`:36`), the count `optRoutePass:332` increments — **cumulative over
    /// the whole stage**, which is what the loop head at `:169-170` compares against
    /// `optimizer.maxItems`.
    pub items_optimized: i32,
    /// `isTimedOut` (`:38`) — the **per-stage** deadline of `:153-160`, never the job stop flag.
    pub timed_out: bool,
    /// Ruling 1(a)'s per-pass tuple, one entry per **completed** pass, in pass order.
    pub per_pass: Vec<OptimizerPassRecord>,
}

/// Ruling 1(a)'s per-pass diagnostic for the optimizer stage — the shape
/// [`BatchLoopResult::per_pass`](crate::pipeline::BatchLoopResult::per_pass) has for the routing
/// stage, widened by the four locals that decide whether the optimizer goes round again.
///
/// renamed: not a Java type. Java scatters these across two `String.format` payloads (`:184-191`,
/// `:222-228`) and `optRoutePass`' own (`:373-383`), all of which the Global Constraints drop.
/// The record is what makes the termination condition testable without a board and what
/// `p7t9 optimizer` prints per pass; **nothing in the loop reads it** (ruling 11's shape: a
/// diagnostic, never an input).
///
/// The plan's interface block says this task "declares no type other than `OptimizerResult`".
/// This is the deviation, and it is the same one Task 10 made for the same reason: ruling 1(a)
/// asks acceptance to compare "the same `(normalized score, incomplete count,
/// clearance-violation count, via count, trace count)` tuple" **per pass**, and this task's own
/// acceptance line asks for "the per-pass score/incomplete tuple identical". There is nowhere
/// else to put it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OptimizerPassRecord {
    /// `currentPass` (`:177`), 1-based.
    pub pass: i32,
    /// `withPreferredDirections = currentPass % 2 != 0` (`:200`) — "to create more variations".
    /// Odd passes route with the preferred directions, even passes without.
    pub with_preferred_directions: bool,
    /// `scoreBeforePass` (`:179`).
    pub score_before: f32,
    /// `scoreAfterPass` (`:208`).
    pub score_after: f32,
    /// `passImprovement` (`:209-210`) — `(after - before) / before`, or `0` when `before <= 0`.
    pub pass_improvement: f64,
    /// `scoreImprovement` (`:215`/`:217`) as the arm left it: `-1` when the increased ripup costs
    /// were dropped this pass, else [`Self::pass_improvement`].
    pub score_improvement: f64,
    /// `useIncreasedRipupCosts` (`:32`) **after** the pass — cleared either by `optRoutePass`
    /// (`:365-368`, no item improved) or by `:212-215` (the board score did not rise).
    pub use_increased_ripup_costs: bool,
    /// What `optRoutePass` returned (`:384`), which `:201` **discards**. `-1` is its
    /// "keep the optimizer going with lower ripup costs" sentinel (`:367`).
    pub route_improved: f32,
    /// `totalItemsOptimized` (`:36`) after the pass, i.e. cumulative.
    pub total_items_optimized: i32,
    /// Ruling 1(a)'s tuple, read off the **same** `BoardStatistics` object `:208` builds for
    /// `scoreAfterPass` — no extra construction, because the count of `BoardStatistics`
    /// constructions is itself observable (see [`BoardStatistics::compute`]).
    pub record: PassRecord,
}

/// `runBatchLoop`'s "already near-perfect" exit (`BatchOptimizer.java:182-193`), lifted out of
/// the loop.
///
/// # Why this is a function
///
/// The same reason [`optimizer_ripup_costs`] is: a pure decision a test can reach without a
/// board. Every arithmetic step is `float` — `optimizationImprovementThreshold` is a
/// `Float` (`OptimizerSettings.java:37-38`), `1 + threshold` is a `float` add, the product is a
/// `float` multiply and `1000.0f` is the literal it is compared against — so computing it in
/// `f64` would answer differently at the boundary, and
/// `the_near_perfect_exit_is_computed_in_f32` pins that.
///
/// `1000` is [`BoardStatistics::normalized_score`]'s ceiling (`BoardStatistics.java:634`, the
/// `* 1000` at the end), so the test reads "the remaining headroom is smaller than the threshold
/// asks for".
#[must_use]
pub fn optimizer_near_perfect_exit(score_before_pass: f32, improvement_threshold: f32) -> bool {
    // :182-183.
    score_before_pass * (1.0 + improvement_threshold) >= 1000.0
}

/// `optRoutePass`' per-item improvement recomputation (`BatchOptimizer.java:340-348`), lifted out
/// of the loop.
///
/// # It is the **correct** twin of a bug the port also reproduces
///
/// `ItemRouteResult`'s constructor computes the same formula at `ItemRouteResult.java:59-65` and
/// gets it wrong: `viaCountAfter / viaCountBefore` there is `int / int`, so the via term
/// truncates to 0 or 1 (**quirk #212**). Here `:345` writes
/// `(float) result.viaCount() / boardStatisticsBefore.items.viaCount`, and the cast binds to the
/// numerator — so this one is a real division. The two therefore **disagree on the same item**,
/// and `the_improvement_recomputation_disagrees_with_the_scorecard_field` asserts both values.
///
/// Neither number is read by any decision: `routeImproved` is compared against `0` at `:365` and
/// returned at `:384` into a call site that discards it (`:201`), and
/// `ItemRouteResult.improvementPercentage` has no reader at all on the headless path. They are
/// reported side by side because the port must not quietly "fix" either.
///
/// # The denominators are the **pass**'s, not the item's
///
/// `boardStatisticsBefore` at `:342-346` is `optRoutePass:281`'s — the board as the pass found
/// it — while `result` measures one item. So the term is "this item's via count against the
/// whole board's", which is what makes the answer a small negative number on any real board.
///
/// # The widening order
///
/// `(float) viaCount / int` is a **`float`** division; `result.traceLength() / totalLength` is a
/// `double` divided by a widened `float`; their sum promotes the first to `double`; `1.0 - …` is
/// `double`; and the whole thing is narrowed by the `(float)` cast at `:341`. Computing the via
/// term in `f64` would lose the `f32` rounding, so the port goes through [`f32`] first.
#[must_use]
pub fn optimizer_route_improved(
    result: &ItemRouteResult,
    before_via_count: i32,
    before_total_length: f32,
) -> f32 {
    // :342-343 — the guard, on the **pass**'s statistics.
    if before_via_count != 0 && before_total_length != 0.0 {
        // :345 — `(float) result.viaCount() / boardStatisticsBefore.items.viaCount`.
        let via_term = f64::from(result.via_count() as f32 / before_via_count as f32);
        // :346 — `result.traceLength() / boardStatisticsBefore.traces.totalLength`.
        let length_term = result.trace_length() / f64::from(before_total_length);
        // :344, :347 — `1.0 - ((via + length) / 2)`, then `:341`'s `(float)`.
        (1.0 - ((via_term + length_term) / 2.0)) as f32
    } else {
        // :348.
        0.0
    }
}

impl BatchOptimizer<'_> {
    // ---------------------------------------------------------------------------------------------
    // NamedAlgorithm's identity — BatchOptimizer.java:527-550
    // ---------------------------------------------------------------------------------------------
    //
    // Five one-line `return "literal";` overrides of `NamedAlgorithm`'s abstract members, which
    // become five associated constants for the reason [`BatchAutorouter`]'s did: the port has no
    // `NamedAlgorithm` to override, because controller ruling AK replaces the class's other half
    // — the three listener lists — with [`ProgressSink`].
    //
    // renamed: `BatchOptimizer.getId` (`:527-530`) -> `BatchOptimizer::ID`, an associated const.
    // renamed: `BatchOptimizer.getName` (`:532-535`) -> `BatchOptimizer::NAME`.
    // renamed: `BatchOptimizer.getVersion` (`:537-540`) -> `BatchOptimizer::VERSION`.
    // renamed: `BatchOptimizer.getDescription` (`:542-545`) -> `BatchOptimizer::DESCRIPTION`.
    // renamed: `BatchOptimizer.getType` (`:547-550`) -> `BatchOptimizer::TYPE`.

    /// `getId()` (`:527-530`) — also the value [`BatchOptimizer::normalize_algorithm`] forces
    /// into `settings.optimizer.algorithm`, and `DefaultSettings.java:131`'s default for it.
    pub const ID: &'static str = "freerouting-optimizer";
    /// `getName()` (`:532-535`).
    pub const NAME: &'static str = "Freerouting Optimizer";
    /// `getVersion()` (`:537-540`).
    pub const VERSION: &'static str = "1.0";
    /// `getDescription()` (`:542-545`).
    pub const DESCRIPTION: &'static str = "Freerouting Optimizer v1.0";
    /// `getType()` (`:547-550`).
    pub const TYPE: NamedAlgorithmType = NamedAlgorithmType::Optimizer;

    /// Port of the private `normalizeAlgorithm(RoutingJob, BatchOptimizer)` (`:68-78`): "the
    /// algorithm '…' is not supported by the batch autorouter; the default algorithm '…' will be
    /// used instead."
    ///
    // renamed: `BatchOptimizer.normalizeAlgorithm` (`:68-78`) — Java's two parameters are the job
    // (for `logWarning` and for the settings object it mutates in place) and the optimizer (for
    // `getId()`). The port has neither: `RoutingJob` is Plan 8's, `settings` is borrowed
    // immutably, and the id is a `const`. What is left is the decision and its answer, so the
    // method takes the configured name and returns the one to use.
    ///
    /// # It always answers [`BatchOptimizer::ID`]
    ///
    /// `:69` is `!optimizer.getId().equals(algorithm)`, and `:76` then assigns `getId()`. So the
    /// post-state is `ID` whatever went in — a mismatched name is replaced and a matching one is
    /// already it. The method is written as the decision rather than as `ID.to_string()` because
    /// the *warning* is the point (`:70-75`), and a `BatchOptimizerMultiThreaded` would answer a
    /// different `getId()` through the same call.
    ///
    /// Its only caller is `createForGui:64`, which the port rosters; it is ported because the
    /// plan asks for it and because Plan 8's CLI will want the same normalisation for
    /// `settings.optimizer.algorithm` that `RoutingPipeline.normalizeRouterAlgorithm` does for
    /// `settings.algorithm`.
    #[must_use]
    pub fn normalize_algorithm(algorithm: &str) -> String {
        // :69.
        if BatchOptimizer::ID != algorithm {
            // :70-75 — `job.logWarning`, dropped with the rest of the logging.
            // :76.
            return BatchOptimizer::ID.to_string();
        }
        algorithm.to_string()
    }

    /// `:172` and `:308` — `deadlineMs != null && System.currentTimeMillis() >= deadlineMs`,
    /// **non-strict**, on the port's monotonic clock.
    ///
    /// The twin of [`BatchFanout::is_deadline_reached`](crate::pipeline::BatchFanout::is_deadline_reached),
    /// and it is a **per-stage** clock: both readers write [`BatchOptimizer::is_timed_out`] and
    /// leave, and neither touches [`RouterStop`]. See [`BatchOptimizer::deadline`].
    #[must_use]
    pub fn is_deadline_reached(&self) -> bool {
        self.deadline
            .is_some_and(|deadline| std::time::Instant::now() >= deadline)
    }

    /// Port of `runBatchLoop()` (`BatchOptimizer.java:125-272`) — "optimize the route on the
    /// board", i.e. the optimizer stage's whole termination condition.
    ///
    /// # The four doors out of the loop, in the order they are tested
    ///
    /// | `:` | door |
    /// |---|---|
    /// | `:167-171` | the head: `currentPass < optimizer.maxPasses`, `totalItemsOptimized < optimizer.maxItems`, and `!thread.isStopRequested()` — **`ALL` only** |
    /// | `:172-176` | the per-stage deadline; sets [`BatchOptimizer::is_timed_out`] |
    /// | `:182-193` | the board is already near-perfect ([`optimizer_near_perfect_exit`]) |
    /// | `:220-230` | the pass improved the score by less than the threshold |
    /// | `:204-206` | plus one more: `optRoutePass` timed out mid-pass |
    ///
    /// # The stop flag the loop reads is `ALL`, and the one that matters is not
    ///
    /// `:171` is `isStopRequested()`, so an `AUTO_ROUTER_ONLY` stop does **not** end this loop —
    /// which is one half of quirk #202. The other half is the half that bites: every ordinary
    /// exit from `AutorouteBatchLoop.run` raises `AUTO_ROUTER_ONLY` (quirk #214), and
    /// `BatchAutorouter.autoroutePassesForOptimizingItem`'s loop head (`BatchAutorouter.java:268`)
    /// is `!isStopAutoRouterRequested()` — so on a shared flag every `optRouteItem` in this loop
    /// rips its connections, routes **zero** passes, measures a worse board and restores the
    /// snapshot. The stage runs and changes nothing. **Java has no reset**: `grep -rn requestStop`
    /// over `src/main` answers no writer that lowers the flag, and `RoutingPipeline.run`
    /// (`:81-85`) hands both stages the one `job.thread`. The port reproduces that — this method
    /// takes the caller's [`RouterStop`] and does not clear it — and **quirk #227** records the
    /// consequence with its measurement. `p7t9 optimizer-shared` is the pin; `p7t9 optimizer`
    /// hands the stage a fresh stop on both sides so that the rest of this method is exercised at
    /// all.
    ///
    /// # What is not here
    ///
    /// `:126-130`, `:140-145` and `:174`, `:184-191`, `:222-228`, `:256-271` are `job.log*`
    /// payloads; `:142`, `:163`, `:195`, `:198`, `:234` are `board.getHash()` reads that only
    /// those payloads and the event objects carry; `:148-151`, `:202`, `:237-248` are the wall
    /// clock and the three JMX samplers rostered at the head of this file. The two
    /// `BoardStatistics` constructions at `:135` and `:250` **are** reproduced although only the
    /// log reads them: each builds two [`DesignRulesChecker`]s, and the construction count is
    /// observable (see [`BoardStatistics::compute`]).
    pub fn run_batch_loop(
        &mut self,
        board: &mut Board,
        stop: &RouterStop,
        budget: RouterBudget,
        progress: &mut dyn ProgressSink,
    ) -> Result<OptimizerResult, RouterError> {
        // Copied out of the field so that the borrow lives for `'a` rather than for `&mut self`;
        // `self.settings` is a shared reference and reading it does not borrow `self`.
        //
        // The three unboxings below are taken **here** rather than at Java's own `:167`, `:179`
        // and `:182`, which moves the `NullPointerException` a few lines earlier — before `:135`'s
        // statistics rather than after them. Unobservable: neither side produces output on that
        // path, `:135`'s constructor mutates nothing a later run could read, and
        // `DefaultSettings.java:130-142` fills all three on every settings ladder the port has.
        let settings = self.settings;
        // `:167` dereferences `this.settings.optimizer` with no null check — `:153`'s guard
        // covers only the timeout read — so an absent block is Java's `NullPointerException`.
        let optimizer = settings.optimizer.as_ref().expect(
            "BatchOptimizer.runBatchLoop: settings.optimizer is dereferenced at :167 without a \
             null check — Java throws a NullPointerException here too",
        );
        // `job.routerSettings.scoring`, read at `:136`, `:179`, `:208` and `:251`; Java
        // dereferences it unguarded.
        let scoring = settings
            .scoring
            .as_ref()
            .expect("RouterSettings.scoring — BatchOptimizer.java:179 dereferences it");
        let improvement_threshold = optimizer.optimization_improvement_threshold.expect(
            "optimizer.optimizationImprovementThreshold is unboxed at :182 and :221 with no null \
             fallback — Java throws a NullPointerException here too",
        );

        // `:29`'s `new ProgressThrottler(1000)` as ruling AI's knob, the substitution
        // `BatchFanout::fanout_board` makes at its own `:28`. The field is set here rather than in
        // `new` because `new` has no budget, and every reader of it is below this line.
        self.progress_throttler = budget.progress_throttler();

        // :132.
        self.use_increased_ripup_costs = true;

        // :135-138. `initialScore`/`initialIncomplete`/`initialViolations` are read only by the
        // `:140-145` and `:256-271` log lines and are dropped with them; the statistics **object**
        // is built, because `getNormalizedScore` is pure but the constructor is not.
        let _initial_stats = BoardStatistics::new(board);

        // :148.
        let session_start = std::time::Instant::now();
        // :153-160 — the per-stage deadline, from `settings.optimizer.timeoutString`. Ruling AI:
        // this is **not** `RouterStop::poll_deadline`, which would request `ALL` and, through
        // `RoutingPipeline.java:117`, suppress a stage Java leaves running.
        if let Some(timeout_string) = optimizer.timeout_string.as_deref()
            && let Some(timeout_seconds) = parse_timespan_seconds(timeout_string)
        {
            // :158 — `sessionStartMs + timeoutSeconds * 1000`, on the port's monotonic clock;
            // `instant_offset_ms` is `BatchFanout.fanoutBoard:98`'s own conversion, shared rather
            // than transcribed twice.
            self.deadline = instant_offset_ms(session_start, timeout_seconds.saturating_mul(1000));
        }

        // :162-163. The event's pass number (`0`) and board hash are payload the port's
        // `RoutingEvent` does not carry.
        progress.on_event(&RoutingEvent::TaskStateChanged {
            algorithm: BatchOptimizer::TYPE,
            state: TaskState::Started,
        });

        // :165-166. `scoreImprovement`'s `-1` initialiser is a **dead store**: `:212-218`
        // assigns it on every iteration before `:220` reads it, and nothing outside the loop
        // reads it at all — so the port declares it inside the loop, where it belongs, and says
        // so rather than carrying a variable Java only appears to carry.
        let mut current_pass: i32 = 0;
        let mut per_pass: Vec<OptimizerPassRecord> = Vec::new();

        // :167-171 — `maxPasses`/`maxItems` are `Integer`s, and a `null` is "no limit".
        // Java bug: `BatchOptimizer.runBatchLoop` (`:171`) — `isStopRequested()` is `ALL`, so an `AUTO_ROUTER_ONLY` stop (which every ordinary end of `AutorouteBatchLoop.run` leaves behind, quirk #214) lets this loop run while `BatchAutorouter.autoroutePassesForOptimizingItem:268` reads `!= NONE` and routes zero passes per item — the stage visits every item, rejects every one and changes nothing, at a whole-board deep copy each (quirk #227). Nothing in `src/main` lowers the flag, so the port must not either.
        while optimizer
            .max_passes
            .is_none_or(|max_passes| current_pass < max_passes)
            && optimizer
                .max_items
                .is_none_or(|max_items| self.total_items_optimized < max_items)
            && !stop.is_stop_requested()
        {
            // :172-176 — the per-stage deadline. `:174` is the log line.
            if self.is_deadline_reached() {
                // :173.
                self.is_timed_out = true;
                break;
            }
            // :177.
            current_pass += 1;

            // :179.
            let score_before_pass = BoardStatistics::new(board).normalized_score(scoring);

            // :182-193 — "stop if potential improvement is less than threshold". Note that
            // `:177` has already counted this pass, so `passes_run` is one more than the number
            // of passes that ran.
            if optimizer_near_perfect_exit(score_before_pass, improvement_threshold) {
                break;
            }

            // :195-196 — `board.getHash()` for the event payload, and `job.setCurrentPass`.
            // :197-198.
            progress.on_event(&RoutingEvent::TaskStateChanged {
                algorithm: BatchOptimizer::TYPE,
                state: TaskState::Running,
            });

            // :200 — "to create more variations": odd passes route with the preferred directions.
            let with_preferred_directions = current_pass % 2 != 0;
            // :201 — the return value is **discarded** here; see [`OptimizerPassRecord::route_improved`].
            let route_improved = self.opt_route_pass(
                board,
                current_pass,
                with_preferred_directions,
                stop,
                budget,
                progress,
            )?;
            // :202 — `sampleHeapUsageMb`, rostered.

            // :204-206 — `optRoutePass` returned early on the deadline.
            if self.is_timed_out {
                break;
            }

            // :208.
            let statistics_after = BoardStatistics::new(board);
            let score_after_pass = statistics_after.normalized_score(scoring);
            // :209-218.
            let (pass_improvement, score_improvement) =
                self.apply_pass_improvement(score_before_pass, score_after_pass);

            per_pass.push(OptimizerPassRecord {
                pass: current_pass,
                with_preferred_directions,
                score_before: score_before_pass,
                score_after: score_after_pass,
                pass_improvement,
                score_improvement,
                use_increased_ripup_costs: self.use_increased_ripup_costs,
                route_improved,
                total_items_optimized: self.total_items_optimized,
                record: PassRecord {
                    pass: current_pass,
                    score: score_after_pass,
                    incomplete_count: stat(statistics_after.connections.incomplete_count),
                    clearance_violations: stat(statistics_after.clearance_violations.total_count),
                    via_count: stat(statistics_after.items.via_count),
                    trace_count: stat(statistics_after.items.trace_count),
                },
            });

            // :220-230 — a `double` against a widened `float`.
            // Java bug: `BatchOptimizer.runBatchLoop` (`:220`) — `!= -1` is a **sentinel** test against a value `:209-210` can also produce honestly: a pass that drives a positive score to exactly zero computes `passImprovement = -1.0`, `:217` assigns it, and the threshold exit is then skipped on the false reading "the increased ripup costs were just dropped" (quirk #228, latent — no corpus pass collapses a score, because every item restores its own snapshot on failure).
            if score_improvement != -1.0 && score_improvement < f64::from(improvement_threshold) {
                break;
            }
        }

        // :233-234 — **unconditional** `FINISHED`, whatever ended the loop.
        progress.on_event(&RoutingEvent::TaskStateChanged {
            algorithm: BatchOptimizer::TYPE,
            state: TaskState::Finished,
        });

        // :237-248 — the session summary's wall clock and its three JMX samplers, rostered.
        // :250-251 — the final statistics. Built for the same reason `:135`'s is, and its score
        // and two counts are dropped with the `:256-271` payload that reads them.
        let _final_stats = BoardStatistics::new(board);

        // :252-255 — `completionStatus`, the only place Java tells the three endings apart.
        let state = if self.is_timed_out {
            TaskState::TimedOut
        } else if stop.is_stop_requested() {
            TaskState::Cancelled
        } else {
            TaskState::Finished
        };

        Ok(OptimizerResult {
            state,
            passes_run: current_pass,
            items_optimized: self.total_items_optimized,
            timed_out: self.is_timed_out,
            per_pass,
        })
    }

    /// `runBatchLoop:209-218`, lifted out of the loop for the reason [`optimizer_ripup_costs`] is:
    /// it is the arm that decides whether the optimizer goes round again, and a test cannot reach
    /// it through a board.
    ///
    /// Answers `(passImprovement, scoreImprovement)` and updates
    /// [`BatchOptimizer::use_increased_ripup_costs`] exactly as `:213` does.
    ///
    /// # `-1` means "keep going"
    ///
    /// `:214`'s comment is "keep the optimizer going to try with normal ripup costs": the first
    /// pass that fails to raise the score spends the increased ripup costs rather than the
    /// optimizer's budget, and `:220`'s `scoreImprovement != -1` is what buys that pass. It can
    /// only ever fire **once**, because `:212`'s first conjunct is then false forever — and
    /// `optRoutePass:365-368` can clear the same flag one step earlier, on the different
    /// condition "no item improved", in which case this arm never fires at all.
    /// `pub` for the reason [`BatchOptimizer::opt_route_pass`] is: this is the arm
    /// `the_increased_ripup_costs_are_dropped_after_one_non_improving_pass` exercises, and
    /// `crates/fr-router/tests/optimizer.rs` is a separate crate.
    pub fn apply_pass_improvement(&mut self, score_before: f32, score_after: f32) -> (f64, f64) {
        // :209-210 — the subtraction is a `float`, the cast and the division are `double`.
        let pass_improvement = if score_before > 0.0 {
            f64::from(score_after - score_before) / f64::from(score_before)
        } else {
            0.0
        };
        // :212-218.
        let score_improvement = if self.use_increased_ripup_costs && score_after <= score_before {
            // :213.
            self.use_increased_ripup_costs = false;
            // :215.
            -1.0
        } else {
            // :217.
            pass_improvement
        };
        (pass_improvement, score_improvement)
    }

    /// Port of `optRoutePass(int, boolean)` (`BatchOptimizer.java:279-385`): "tries to reduce the
    /// number of vias and the trace length of a completely routed board. Returns the amount of
    /// improvements is made in percentage (expressed between 0.0 and 1.0). -1 if the routing must
    /// go on no matter how much it improved."
    ///
    /// # The five doors out of the item loop
    ///
    /// | `:` | door | tail at `:364-384`? |
    /// |---|---|---|
    /// | `:308-313` | the per-stage deadline | **no** — an early `return` |
    /// | `:314-317` | `thread.isStopRequested()` — `ALL` | **no** |
    /// | `:318-326` | `optimizer.maxItems` reached | yes |
    /// | `:327-330` | the reader is exhausted | yes |
    /// | `:349-361` | `maxConsecutiveFailures` consecutive unimproved items | yes |
    ///
    /// The first two skip `:364`'s `sortedRouteItems = null` as well, so
    /// [`BatchOptimizer::get_current_position`] keeps answering the cursor of a pass that ended —
    /// Java's behaviour, transcribed.
    ///
    /// # `pub`, not the brief's private
    ///
    /// Java's modifier is `protected`, i.e. reachable from `P7T8`/`P7T9` in the same package, and
    /// `crates/fr-router/tests/optimizer.rs` is a separate crate. Same argument as
    /// [`BatchOptimizer::opt_route_item`]'s.
    ///
    /// # The return value
    ///
    /// `Result<f32, _>` rather than the brief's `Result<(), _>`: Java returns `routeImproved` at
    /// `:384` and `:201` discards it, but the number is the pass's own answer and dropping a
    /// value the method computes is a gratuitous divergence (Task 13 §2.4's precedent for
    /// `autoroutePassesForOptimizingItem`). [`OptimizerPassRecord::route_improved`] carries it.
    ///
    /// # What is not here
    ///
    /// `:289-298` and `:370` are `FRLogger.traceEntry`/`traceExit` and the id string they key on;
    /// `:309`, `:321-324`, `:352-358` and `:373-383` are `job.logInfo` payloads, and `:378`'s
    /// `board.getHash()` is one of their arguments.
    #[allow(clippy::too_many_arguments)]
    pub fn opt_route_pass(
        &mut self,
        board: &mut Board,
        pass_no: i32,
        with_preferred_directions: bool,
        stop: &RouterStop,
        budget: RouterBudget,
        progress: &mut dyn ProgressSink,
    ) -> Result<f32, RouterError> {
        let settings = self.settings;
        let optimizer = settings.optimizer.as_ref().expect(
            "BatchOptimizer.optRoutePass: settings.optimizer is dereferenced at :301 without a \
             null check — Java throws a NullPointerException here too",
        );

        // :281.
        let board_statistics_before = BoardStatistics::new(board);
        // :282-283.
        let router_counters = RouterCounters {
            pass_count: Some(pass_no),
            ..RouterCounters::default()
        };
        // :284.
        self.progress_throttler.reset();
        // :285 — **unthrottled**, unlike the two gates below.
        progress.on_event(&RoutingEvent::BoardUpdated {
            counters: router_counters.clone(),
        });

        // :287 — a fresh reader every pass; plan-7 ruling 12 forbids memoising it.
        self.sorted_route_items = Some(ReadSortedRouteItems::new());
        // :288 — the pass's floor for `ItemRouteResult`'s length rung, lowered per improved item
        // at `optRouteItem:498-499`.
        self.min_cumulative_trace_length = f64::from(
            board_statistics_before
                .traces
                .total_weighted_length
                .unwrap_or(0.0),
        );

        // :300-304 — `maxConsecutiveFailures` defaults to **50**, and that literal is the
        // fallback for a `null` setting, not for a missing one: `DefaultSettings.java:142` sets
        // the same 50.
        let mut consecutive_failures: i32 = 0;
        let max_consecutive_failures = optimizer.max_consecutive_failures.unwrap_or(50);

        // :306.
        let mut route_improved: f32 = 0.0;
        // :307.
        loop {
            // :308-313 — the per-stage deadline, again. It returns **without** the `:364-384`
            // tail, so `useIncreasedRipupCosts` is not cleared and no closing event fires.
            if self.is_deadline_reached() {
                // :310.
                self.is_timed_out = true;
                // :311-312.
                return Ok(route_improved);
            }
            // :314-317 — the `ALL` stop, and the same early return.
            if stop.is_stop_requested() {
                return Ok(route_improved);
            }
            // :318-326 — the `maxItems` gate. Unlike the loop head at `:169-170` this one carries
            // a `> 0` guard, so `maxItems = 0` stops the stage at the head and is ignored here.
            if optimizer
                .max_items
                .is_some_and(|max_items| max_items > 0 && self.total_items_optimized >= max_items)
            {
                break;
            }
            // :327-330.
            let Some(current_item) = self
                .sorted_route_items
                .as_mut()
                .expect("optRoutePass:287 assigned it one line ago")
                .next(board)
            else {
                break;
            };
            // :331 — `disableSnapshots = false`; the `true` caller is GUI-only.
            let result = self.opt_route_item(
                board,
                current_item,
                with_preferred_directions,
                false,
                stop,
                budget,
                progress,
            )?;
            // :332.
            self.total_items_optimized += 1;
            // :333.
            if result.improved() {
                // :334.
                consecutive_failures = 0;
                // :335-338 — the **throttled** board update. Java computes the statistics *inside*
                // the gate, so how often `new BoardStatistics(board)` runs here depends on the
                // wall clock; the port keeps the computation inside the gate for the same reason
                // and lets [`RouterBudget::progress_throttle_ms`] decide. `p7t9 optimizer`'s
                // `equalsTranscript` line is what measures that the difference is inert.
                if self.progress_throttler.should_update() {
                    // :336.
                    let _board_statistics_after = BoardStatistics::new(board);
                    // :337.
                    progress.on_event(&RoutingEvent::BoardUpdated {
                        counters: router_counters.clone(),
                    });
                }
                // :340-348.
                route_improved = optimizer_route_improved(
                    &result,
                    board_statistics_before.items.via_count.unwrap_or(0),
                    board_statistics_before.traces.total_length.unwrap_or(0.0),
                );
            } else {
                // :350.
                consecutive_failures += 1;
                // :351-360.
                if consecutive_failures >= max_consecutive_failures {
                    break;
                }
            }
        }

        // :364.
        self.sorted_route_items = None;
        // :365-368 — the **second** writer of `useIncreasedRipupCosts`, on a different condition
        // from `runBatchLoop:212-215`: "no item improved in this pass", not "the board score did
        // not rise". It fires first, and when it does `:212`'s first conjunct is already false —
        // so a pass that improves nothing costs the optimizer its increased ripup costs *and*
        // takes `:220`'s threshold exit, ending the stage one pass earlier than `:214`'s comment
        // suggests.
        if self.use_increased_ripup_costs && route_improved == 0.0 {
            self.use_increased_ripup_costs = false;
            // :367 — returned to a call site that discards it (`:201`).
            route_improved = -1.0;
        }

        // :371 — built for the `:372` event and the `:373-383` log line; the port keeps the
        // construction because the count of `BoardStatistics` constructions is observable.
        let _board_statistics_after = BoardStatistics::new(board);
        // :372 — unthrottled.
        progress.on_event(&RoutingEvent::BoardUpdated {
            counters: router_counters,
        });

        // :384.
        Ok(route_improved)
    }
}

// =================================================================================================
// The deferral roster for `autoroute/pipeline/BatchOptimizer.java`
// =================================================================================================

// Task 14 closed this roster: `runBatchLoop` (`:125-272`) and `optRoutePass` (`:279-385`) are
// [`BatchOptimizer::run_batch_loop`] and [`BatchOptimizer::opt_route_pass`] above, the five
// `NamedAlgorithm` identity overrides (`:527-550`) are the five `renamed:` consts, and
// `normalizeAlgorithm` (`:68-78`) is [`BatchOptimizer::normalize_algorithm`]. One member is left,
// and it is the door to the two multithreaded classes:
//
// not ported: `BatchOptimizer.createForGui` (`:56-66`) — the GUI factory. It is the **only** construction site of `BatchOptimizerMultiThreaded` (`:59`) and the only reader of `Freerouting.globalSettings.featureFlags.multiThreading` (`:58`, `settings/FeatureFlagsSettings.java:11`) on this path; its one caller is `RoutingPipeline.createForGui` (`RoutingPipeline.java:40-42`), whose one caller is `gui/workspace/progress/GuiRoutingJobWorker.java:212`. The Global Constraints have no GUI, and `createForHeadless` (`:51-53`, this file's [`BatchOptimizer::new`]) **always** answers the single-threaded implementation, so the port has neither the factory nor the flag — and therefore no static mutable global, which is what makes `featureFlags.multiThreading` (the one such global in Plan 7's scope) a non-issue rather than an exception.
//
// The two classes behind that door are rostered where `scripts/audit-map/fr-router.map` points
// them, `crates/fr-router/src/lib.rs`, with the same three greps.
