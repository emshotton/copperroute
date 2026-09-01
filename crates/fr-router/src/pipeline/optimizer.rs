//! [`BatchOptimizer`] — the port of `autoroute/pipeline/BatchOptimizer.java` (660 lines), the
//! optimizer stage (Plan 7 Task 13, part A).
//!
//! Task 13 lands the **item half**: the type and its field block (`:29-38`), the
//! `createForHeadless` constructor (`:51-53`), `containsOnlyUnfixedTraces` (`:85-92`),
//! `optRouteItem` (`:395-514`), `getCurrentPosition` (`:520-525`), `calculateIncompleteCount`
//! (`:552-556`) and the protected inner class [`ReadSortedRouteItems`] (`:563-659`).
//! **Task 14** adds `runBatchLoop` (`:125-272`) and `optRoutePass` (`:279-385`) as a second
//! `impl` block and the five `NamedAlgorithm` identity constants; each is a marker at the foot of
//! this file.
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
//! | `searchTreeManager` | mutated item by item (`:1254`, `:1273`) | replaced wholesale | the *contents* agree, and the SHAPE difference is provably inert for every downstream consumer: both Java's `MinAreaTree.overlaps` and the port's mirror funnel traversal results through an identity-ordered set (`Leaf.compareTo` / `TreeEntry`'s `Ord`), so tree topology cannot leak into any query's order or membership — only traversal COST, which is parity-irrelevant with budgets disabled (ruling AI). (Task 13 review §4; supersedes the earlier "measured inert, not proved" framing. The k=175/quirk-#210 analogy does not apply: that was `TreeSet<Item>` ITERATION order, which has no such re-sorting insulation.) |
//! | `revision` | keeps counting | rolled back | inert — `Board::revision` has no reader outside tests |
//! | `changedArea` | untouched | rolled back to the snapshot's `None` | inert — every consumer calls `startMarkingChangedArea()` first (`AutoroutePassRunner.java:223`, `BatchAutorouter.java:489`) |
//! | `maxTraceHalfWidth` / `minTraceHalfWidth` | keeps the widened value of a trace that no longer exists | rolled back | private in the port, so "keep live" is not expressible; the rolled-back value is the one that describes the restored board |
//!
//! # What is deliberately not here
//!
//! * the three JMX samplers (`:94-122`) — `sampleCurrentThreadCpuSeconds`,
//!   `sampleCurrentThreadAllocatedMb`, `sampleHeapUsageMb`; every reader is a `job.logInfo`
//!   string;
//! * `createForGui` (`:56-66`) and `normalizeAlgorithm` (`:68-78`) — the GUI factory and its
//!   warning; Task 14 owns the roster line;
//! * `RoutingJob` (`:35`) — Plan 8's, together with the CLI that builds it.

use std::collections::BTreeSet;

use fr_board::items::Item;
use fr_board::{Board, ItemId, StopConnectionOption};
use fr_drc::DesignRulesChecker;
use fr_geometry::{FloatPoint, java_min, java_round};
use fr_settings::RouterSettings;

use crate::error::RouterError;
use crate::pipeline::batch_autorouter::BatchAutorouter;
use crate::pipeline::counters::RouterCounters;
use crate::pipeline::item_route_result::ItemRouteResult;
use crate::pipeline::stop::{ProgressThrottler, RouterBudget, RouterStop};
use crate::pipeline::{ProgressSink, RoutingEvent};
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
    /// sites, which requests `ALL`. This field is the declaration half of `pipeline::stop`'s
    /// `obligation:` marker for `BatchOptimizer`; **Task 14 owns the two reads** and discharges
    /// the rest. `BatchFanout`'s half was discharged in Task 12.
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
// The deferral roster for `autoroute/pipeline/BatchOptimizer.java`
// =================================================================================================

// added in Task 14: `BatchOptimizer.runBatchLoop` (`:125-272`) — the optimizer stage's pass loop and its whole termination condition, including the two per-stage deadline reads at `:172` and `:308` that discharge `pipeline::stop`'s `obligation:` marker.
// added in Task 14: `BatchOptimizer.optRoutePass` (`:279-385`) — the per-pass walk over [`ReadSortedRouteItems`], calling this file's [`BatchOptimizer::opt_route_item`].
// added in Task 14: `BatchOptimizer.createForGui` (`:56-66`), `BatchOptimizer.normalizeAlgorithm` (`:68-78`) — the GUI factory and the algorithm-name warning it fires; the only path to `BatchOptimizerMultiThreaded`.
// added in Task 14: `BatchOptimizer.getId` (`:527-530`), `BatchOptimizer.getName` (`:532-535`), `BatchOptimizer.getVersion` (`:537-540`), `BatchOptimizer.getDescription` (`:542-545`), `BatchOptimizer.getType` (`:547-550`) — the five `NamedAlgorithm` identity overrides, which become five associated consts as `BatchAutorouter`'s did.
