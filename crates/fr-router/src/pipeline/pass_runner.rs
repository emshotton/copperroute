//! Port of `autoroute/pipeline/AutoroutePassRunner.java` (526 lines) — one autoroute pass.
//!
//! The file holds two pass drivers and five `log*` helpers. Only `runSingleThread` (`:151-336`)
//! is ported; `runMultiThread` (`:40-149`) and the helpers are rostered below with their grep.

use std::collections::{BTreeMap, BTreeSet};
use std::panic::AssertUnwindSafe;

use fr_board::{Board, ItemId, StopConnectionOption};

use crate::autoroute::attempt::AutorouteAttemptState;
use crate::error::RouterError;
use crate::pipeline::batch_autorouter::BatchAutorouter;
use crate::pipeline::failure_log::RoutingFailureLog;
use crate::pipeline::{ProgressSink, RouterCounters, RouterStop, RoutingEvent};
use crate::score::BoardStatistics;

/// Port of `autoroute.pipeline.AutoroutePassRunner` (AutoroutePassRunner.java:20-526) — the
/// object `BatchAutorouter` delegates one autoroute pass to.
///
/// # A unit struct, because Java's only field is a back-pointer
///
/// `AutoroutePassRunner` holds exactly one field, `private final BatchAutorouter router`
/// (`:22`), assigned by its only constructor (`:36-38`) from `BatchAutorouter.java:156`'s
/// `new AutoroutePassRunner(this)`. The port hands the router in as a parameter instead — the
/// same shape [`BatchAutorouter`] itself uses for the board — so there is no state to hold and
/// the type exists only to give the two methods a home an audit map can point at.
///
// not ported: `AutoroutePassRunner.runMultiThread` (`:40-149`) — its only caller is `BatchAutorouter.autoroutePassMultiThread` (`:411-413`), which has **zero** callers in `src/main` or `src/test` (survey §3.4). No rayon, no threads (ruling AM).
// not ported: `AutoroutePassRunner.onBoardUpdatedEvent` — **not a method of the class**: it is the single method of an *anonymous* `BoardUpdatedEventListener` declared at `:78-85`, inside the dead `runMultiThread`, and `audit-port.sh`'s line-based extraction attributes it to the enclosing file. Its body forwards the multithreaded workers' board updates to `NamedAlgorithm.fireBoardUpdatedEvent`, which controller ruling AK replaces with [`ProgressSink`] wholesale.
// not ported: `AutoroutePassRunner.logIncompleteDetails` (`:338-359`) — an `FRLogger`/`job.logDebug` payload builder; every `FRLogger` call in `autoroute/**` is dropped (plan-6 ruling 14).
// not ported: `AutoroutePassRunner.logRippedItems` (`:361-395`) — likewise; it is also the reason `rippedItemCosts` is a `LinkedHashMap` rather than a `TreeMap` (see [`AutoroutePassRunner::run_single_thread`]).
// not ported: `AutoroutePassRunner.logTraceRouteComparison` (`:397-437`) — likewise, and guarded by `FRLogger.isTraceEnabled()` at its call site (`:252`).
// not ported: `AutoroutePassRunner.logNet94Items` (`:439-487`) — likewise, and gated by the hard-coded net number at `:256` (quirk #190, which already lists this site).
// not ported: `AutoroutePassRunner.logTailRemoval` (`:518-525`) — likewise; called at `:297` and `:303` around `removeTails`.
#[derive(Debug, Clone, Copy, Default)]
pub struct AutoroutePassRunner;

impl AutoroutePassRunner {
    /// Port of `runSingleThread(int passNo)` (`:151-336`) — "auto-routes one ripup pass of all
    /// items of the board".
    ///
    /// Answers Java's `boolean` at `:330`: `routed > 0 || notRouted > 0`, i.e. "something
    /// happened, so there may still be work to do". An empty work list is `false` at `:165`, and
    /// so is the caught-exception path at `:334`.
    ///
    /// # The recovery boundary this method **does** have — a correction to scan ruling 9
    ///
    /// The plan's scan ruling 9 struck plan-7 ruling 7's second recovery boundary with the
    /// finding that `AutoroutePassRunner.java:144` is the catch of the *dead* `runMultiThread`
    /// and that "`runSingleThread` (`:151-336`) has no try/catch". **Half of that is right and
    /// half is wrong, and Java wins** (plan §Global Constraints):
    ///
    /// ```text
    /// $ sed -n '140,160p' AutoroutePassRunner.java
    /// 140
    /// 141       boolean anyProgress = bestThread.getRoutedCount() > 0 || …
    /// 142       router.airLine = null;
    /// 143       return anyProgress;
    /// 144     } catch (Exception e) {          <-- closes runMultiThread. Ruling 9 is right here.
    /// 145       router.job.logError("Something went wrong during the auto-routing", e);
    /// 146       router.airLine = null;
    /// 147       return false;
    /// 148     }
    /// 149   }
    /// 150
    /// 151   boolean runSingleThread(int passNo) {
    /// 152     long passStartTime = System.currentTimeMillis();
    /// 153     if (BatchAutorouter.isBenchmarkProfileEnabled()) {
    /// 154       router.resetPassProfile();
    /// 155     }
    /// 156     try {                            <-- runSingleThread's OWN try. Ruling 9 missed it.
    /// ```
    ///
    /// The window ruling 9 quotes (`140,155`) stops **one line short** of the `try` that
    /// contradicts it. `runSingleThread`'s body is wrapped from `:156` to `:335`, and its
    /// `catch (Exception e)` at `:331-335` degrades to `router.airLine = null; return false;` —
    /// the same shape as each of plan-6 ruling 7's boundaries: *catch, produce a specific
    /// degraded value, do not propagate.*
    ///
    /// So the port **does** carry a boundary, and it is the one Java has: a
    /// [`std::panic::catch_unwind`] around the whole body (not around the per-item step, which is
    /// what ruling 9 was right to forbid — a Java exception inside the item loop ends the
    /// *pass*, it does not skip one item and carry on), plus the same degradation for a
    /// [`RouterError`] travelling out of the body by value. A panic on item 3 of 40 therefore
    /// leaves the board exactly as Java leaves it: mutated by the first two items, with the
    /// remaining 37 unrouted and `false` returned.
    ///
    /// # `Result`, and why it never carries an `Err` today
    ///
    /// Nothing inside can escape the `catch (Exception)` Java writes at `:331`, so every path of
    /// this method answers `Ok`. The `Result` is kept because Task 10's `AutorouteBatchLoop` —
    /// which *does* have a propagating boundary (plan-7 ruling 7's `NoRoutableLayer`) — threads
    /// its callee's error channel, and because a caller reading the signature should not have to
    /// discover the boundary by reading the body.
    ///
    /// # The two parameters the brief listed that are not here
    ///
    /// * **`budget: RouterBudget`** — ruling AI's budget is already a field of
    ///   [`BatchAutorouter`] ([`BatchAutorouter::budget`]), and it is from there that
    ///   `autoroute_item` and `remove_tails` read it. A second copy on this signature could
    ///   disagree with the router's, and Java has no field for it to model: its literals live in
    ///   `AutorouteConnectionRouter.java:22` and `BatchAutorouter.java:337`, both reached through
    ///   the router. Callers configure the budget where they build the router.
    /// * `passNo`'s companion `maxItems` is read off `router.settings` (`:213`), not passed.
    ///
    /// # Transcription notes
    ///
    /// The two `fireBoardUpdatedEvent` calls at `:196` and `:321` are **ungated** — only
    /// `updateProgress`'s (`:507`) goes through `shouldFireBoardUpdate`. All three become
    /// [`RoutingEvent::BoardUpdated`].
    ///
    /// `:227-228`'s `final int netItemsBefore = board.getConnectableItems(…).size()` is computed
    /// unconditionally but read only by `logTraceRouteComparison` (`:255`), which is not ported.
    /// It is a pure query, so dropping it changes nothing; the line is recorded here rather than
    /// silently omitted.
    ///
    /// `:225-226`'s two containers: Java's `TreeSet<Item>` is a [`BTreeSet<ItemId>`] — **quirk
    /// #44**, ascending here and descending there, which is unobservable because
    /// `AutorouteConnectionRouter` only tests membership and counts it; and Java's
    /// `LinkedHashMap<Item, Integer>` is a [`BTreeMap<ItemId, i32>`], because its only
    /// order-sensitive reader is `logRippedItems` (`:361-395`), which is not ported, and
    /// Plan 6 Task 17 established that both sides may sort the map by item id.
    #[allow(clippy::too_many_arguments)]
    pub fn run_single_thread(
        board: &mut Board,
        router: &mut BatchAutorouter<'_>,
        failure_log: &mut RoutingFailureLog,
        pass_no: i32,
        stop: &RouterStop,
        progress: &mut dyn ProgressSink,
    ) -> Result<bool, RouterError> {
        // :156 … :335 — the method-level `try`/`catch (Exception e)`. The `catch` produces
        // `false` (`:334`) after clearing `router.airLine` (`:333`), which the port does not
        // hold (`BatchAutorouter`'s field block rosters it `not ported:`).
        let outcome = std::panic::catch_unwind(AssertUnwindSafe(|| {
            AutoroutePassRunner::run_single_thread_body(
                board,
                router,
                failure_log,
                pass_no,
                stop,
                progress,
            )
        }));
        match outcome {
            // :330 — the body's own answer.
            Ok(Ok(any_progress)) => Ok(any_progress),
            // :331-335 — an exception thrown by value, and a panic, are the same event in Java.
            Ok(Err(_)) | Err(_) => Ok(false),
        }
    }

    /// `runSingleThread`'s body, `:157-330` — everything inside Java's `try`.
    ///
    /// Split out so the boundary above is one expression; see
    /// [`AutoroutePassRunner::run_single_thread`] for why the boundary exists at all.
    #[allow(clippy::too_many_arguments)]
    fn run_single_thread_body(
        board: &mut Board,
        router: &mut BatchAutorouter<'_>,
        failure_log: &mut RoutingFailureLog,
        pass_no: i32,
        stop: &RouterStop,
        progress: &mut dyn ProgressSink,
    ) -> Result<bool, RouterError> {
        // :158 — the Java line. Since Plan 9 Task 2 (R1, register row #293) this list arrives
        // **sorted ascending by `calculateItemDistance`** — shortest airline first, ties in the
        // descending-id walk order Java's `board.itemList` produces. The sort lives in
        // [`BatchAutorouter::autoroute_items_with_handled`], where Java's deleted
        // `autorouteItemList.sort(...)` stood; nothing changes here, and the `for` at `:202`
        // below simply walks a better order. `run.sh p7t1` therefore MISMATCHes on the `ITEM`
        // order by design — that seam's doc says what must still agree.
        let autoroute_item_list = router.autoroute_items(board);

        // :163-166. `router.airLine = null` is the not-ported field.
        if autoroute_item_list.is_empty() {
            return Ok(false);
        }

        // :170-171. `new BoardStatistics(board, null, false)`: no preferred unit, no clearance
        // violations, and the four-argument constructor's `includeConnections` defaulting to
        // `true` — so this is a full incomplete-connection pass, once per autoroute pass.
        BoardStatistics::compute_side_effects(board, false);
        router.progress_items_since_statistics = 0;

        // :176.
        let mut items_to_go_count = i32::try_from(autoroute_item_list.len()).unwrap_or(i32::MAX);
        // :177-185.
        let mut counters = RouterCounters {
            phase: Some("autoroute".to_string()),
            pass_count: Some(pass_no),
            queued_to_be_routed_count: Some(items_to_go_count),
            skipped_count: Some(0),
            ripped_count: Some(0),
            failed_to_be_routed_count: Some(0),
            routed_count: Some(0),
            ..RouterCounters::default()
        };

        // :188-190. Java builds its own throw-away `DesignRulesChecker`, runs
        // `calculateAllIncompletes()` and reads `getIncompleteCount()`; it keeps the checker only
        // to hand to `logIncompleteDetails` (`:195`), which is not ported. Those three lines are
        // `BatchAutorouter.calculateIncompleteCount` (`:556-564`) verbatim, so the port calls it.
        counters.incomplete_count =
            i32::try_from(BatchAutorouter::calculate_incomplete_count(board)).ok();

        // :196 — an **ungated** board-update fire, before the loop.
        progress.on_event(&RoutingEvent::BoardUpdated {
            counters: counters.clone(),
        });

        // :198-201.
        let mut ripped_item_count: i32 = 0;
        let mut not_routed: i32 = 0;
        let mut routed: i32 = 0;
        let mut skipped: i32 = 0;

        // :202.
        for current_item in autoroute_item_list {
            // **Controller ruling AI's fourth poll site, added by Plan 8 Task 12 after a
            // measurement.** Ruling BB's three sites are the *pass* loop heads
            // (`batch_loop`, `fanout`, `optimizer`), and Task 11 recorded the residual latency as
            // "one pass". Task 12 measured what one pass costs on a real board:
            // `fixtures/Issue508-DAC2020_bm01.dsn`, release build, `--max-passes 1` with fanout
            // and optimizer off, is **135 seconds**. An operator's `notifications/cancelled`
            // taking over two minutes to be observed is not a cancellation, so ruling AI's
            // sanctioned fourth site is taken: this loop, which is Java's own
            // `AutoroutePassRunner.java:202-205` guard, one turn per item.
            //
            // Task 12 took `poll_cancel` here and only `poll_cancel`, because ruling AI's
            // prohibition is about *per-stage* clocks: `poll_deadline` requests `ALL`, and on a
            // stage clock that would suppress a stage Java leaves running, while `poll_cancel`
            // carries no clock at all and copies in what an operator asked for, which is
            // `requestStop()` by definition. The **job** deadline is not a stage clock and this
            // is one of its two sanctioned sites, so `poll_deadline` joins it below — see the
            // comment there.
            //
            // Byte-invisible by construction: both `RouterStop` constructors leave
            // `cancel_poll: None`, and a `None` poll is a load and nothing else. Every parity
            // driver builds `RouterStop::new()`.
            stop.poll_cancel();
            // **The job deadline's second permitted read site**, the one
            // `RouterStop::poll_deadline`'s own doc names: "`AutorouteBatchLoop:251` (Task 10) and
            // the top of `AutoroutePassRunner`'s item loop at `:203` (Task 9)". Only the first had
            // a caller until the post-merge outlier investigation measured the cost of the gap:
            // `zx-sizif-512-ext` at `--router.job_timeout=00:05:00` ran **341 s**, +41 s, because
            // the deadline was observed at pass boundaries only and the pass that was running when
            // it expired ran to completion. Java's gap is a **sleep interval**, not a pass: its
            // monitor thread (`RoutingJobSchedulerActionThread.java:55-90`) wakes once a second,
            // calls `job.thread.requestStop()` at `:75`, and `AutoroutePassRunner.java:203` reads
            // the flag once per item — so Java breaks out mid-pass within ~1000 ms of expiry. The
            // port polls the clock directly at this same site and therefore lands marginally
            // *tighter* than Java, not merely level with it; the residual is one item, which
            // neither side preempts.
            //
            // This is a **job-level** site, so ruling AI's prohibition does not reach it: the four
            // forbidden sites (`BatchFanout:111`/`:396`, `BatchOptimizer:172`/`:308`) read a
            // *per-stage* clock whose Java action never touches the stop flag, and requesting `ALL`
            // there would suppress a stage Java leaves running. The job deadline is not a stage
            // clock. See `pipeline::stop`'s module doc table.
            //
            // Byte-invisible on an untimed run for the same reason as `poll_cancel` above: both
            // `RouterStop` constructors leave `deadline: None`, and `poll_deadline` on a `None`
            // deadline is a constant `false` that touches nothing. The `:203-205` guard below is
            // the reader, exactly as in Java — no extra branch is added here, because
            // `poll_deadline` performs Java's monitor action (`requestStop()`, i.e. `ALL`) and
            // `is_stop_auto_router_requested()` is `!= NONE`.
            stop.poll_deadline();
            // :203-205.
            if stop.is_stop_auto_router_requested() {
                break;
            }

            // :207 — `currentItem.netCount()`, re-read from the **live board** because
            // `RoutingBoard.reduceNetsOfRouteItems` can change an item's net list between
            // connections (quirk #211). Java reads it off the `Item` *object* the list holds,
            // which would survive removal from `itemList`; the port has only the id, so an item
            // that had been removed would answer `net_count == 0` here and be skipped where Java
            // would still attempt it.
            //
            // **That case cannot arise.** `getAutorouteItems:358-359` keeps only items where
            // `!isRoutable()`, and `MazeRipupResolver.checkRipup` (`:72-76`) refuses to rip any
            // item where `!isRoutable()` — `return -1` before anything else — as does its
            // `:205-212` twin. The two sets are **disjoint**, so no connection of this pass can
            // remove an item later in this pass's work list. `removeTails`
            // (`RoutingBoard.java:1197`) applies the same test, and runs after the loop in any
            // case.
            let net_count = board.get_item(current_item).map_or(0, |i| i.net_count());
            for i in 0..net_count {
                // :208-210.
                if stop.is_stop_auto_router_requested() {
                    break;
                }

                // :212-221. `maxItems` reached: log, request an **ALL** stop, and break the net
                // loop; `:203-205` then breaks the item loop on the next turn.
                //
                // Java bug: `AutoroutePassRunner.runSingleThread` (`:219`) — `thread.requestStop()` sets `ALL`, not `AUTO_ROUTER_ONLY`, so reaching `--max-items` also silences the optimizer stage at `RoutingPipeline.java:117` (quirk #202).
                let max_items = router.settings().max_items;
                if max_items.is_some_and(|max| max > 0 && router.total_items_routed >= max) {
                    // :219 — `requestStop`, not `requestStopAutoRouter`.
                    stop.request_stop();
                    break;
                }
                // :222.
                router.total_items_routed += 1;
                // :223.
                board.start_marking_changed_area();

                // :225-226 — see the method doc for both container choices.
                let mut ripped_item_list: BTreeSet<ItemId> = BTreeSet::new();
                let mut ripped_item_costs: BTreeMap<ItemId, i32> = BTreeMap::new();

                // :227-228's `netItemsBefore` is `logTraceRouteComparison`'s only input and is
                // not computed here; see the method doc.

                // :232, :239 — `currentItem.getNetNumber(i)`, read off the live board for the
                // reason `net_count` is, and safe for the same disjointness argument.
                let route_net_no = board
                    .get_item(current_item)
                    .map_or(-1, |item| item.get_net_number(i));

                // :238-245 -> `BatchAutorouter.autorouteItem` (`:507-514`) -> the whole of
                // `AutorouteConnectionRouter.route`. A fresh engine per connection, because
                // `retainAutorouteDatabase` is permanently `false` (ruling AJ).
                let mut engine = None;
                // Java hands `router.thread` — a `Stoppable` — to `initAutoroute` (`route:80`)
                // and to `optChangedArea` (`:108`, `:228`), and both poll
                // `Stoppable.isStopRequested()` (`AutorouteEngine.java:294-303`,
                // `TraceTightener.java:195-196`), i.e. **`ALL`**. That is a different predicate
                // from the item loop's `isStopAutoRouterRequested()` at `:203`/`:208`, which is
                // `!= NONE`: an `AUTO_ROUTER_ONLY` stop ends the pass between connections but
                // does **not** abort a connection already in flight.
                let stop_check = &|| stop.is_stop_requested();
                let autorouter_result = router.autoroute_item(
                    board,
                    &mut engine,
                    current_item,
                    route_net_no,
                    &mut ripped_item_list,
                    &mut ripped_item_costs,
                    pass_no,
                    stop_check,
                );

                // :251-258 are the three log payloads; see the `not ported:` roster above.

                // :260-289 — the result switch.
                match autorouter_result.state {
                    // :260-261.
                    AutorouteAttemptState::Routed => routed += 1,
                    // :262-266.
                    AutorouteAttemptState::AlreadyConnected
                    | AutorouteAttemptState::NoUnconnectedNets
                    | AutorouteAttemptState::ConnectedToPlane => skipped += 1,
                    // :267-289 — everything else is a failure.
                    _ => {
                        // :269-271. `:267-268`'s `net`/`netName` locals feed `:277`'s log line
                        // only.
                        failure_log.record_failure(
                            board,
                            current_item,
                            pass_no,
                            autorouter_result.state,
                            autorouter_result.details.as_deref(),
                        );
                        // :272-273's `getFailureCount` gates `:274-287`'s `job.logDebug` and
                        // nothing else. The call is kept because it is the log's **only** live
                        // reader and dropping it would leave `RoutingFailureLog::failure_count`
                        // with no caller at all; its answer is deliberately discarded.
                        let _failure_count = failure_log.failure_count(current_item);
                        // :288.
                        not_routed += 1;
                    }
                }

                // :290.
                items_to_go_count -= 1;
                // :291.
                ripped_item_count += i32::try_from(ripped_item_list.len()).unwrap_or(i32::MAX);
                // :292-293.
                AutoroutePassRunner::update_progress(
                    board,
                    router,
                    progress,
                    &mut counters,
                    items_to_go_count,
                    ripped_item_count,
                    not_routed,
                    routed,
                    skipped,
                );
            }
        }

        // :297 and :303 are `logTailRemoval`; not ported.
        // :298-302.
        // `removeTails` forwards `this.thread` to `optChangedArea` (`BatchAutorouter.java:497`),
        // so the check here is `isStopRequested()` (`ALL`) for the same reason the item's is.
        let tail_stop = &|| stop.is_stop_requested();
        if router.is_remove_unconnected_vias() {
            router.remove_tails(board, None, StopConnectionOption::None, tail_stop)?;
        } else {
            router.remove_tails(board, None, StopConnectionOption::FanoutVia, tail_stop)?;
        }

        // :309 — `board.getStatistics()` is `new BoardStatistics(this)`
        // (`RoutingBoard.java:1410-1412`), i.e. the full three-argument default. Its only reader
        // is the event fired at `:321`, whose port payload is the counters; its DRC pass is run
        // anyway because it is a **full DRC pass** that warms the board's caches, and skipping it
        // would be a different program.
        BoardStatistics::compute_side_effects(board, true);

        // :313-320.
        counters.pass_count = Some(pass_no);
        counters.queued_to_be_routed_count = Some(items_to_go_count);
        counters.skipped_count = Some(skipped);
        counters.ripped_count = Some(ripped_item_count);
        counters.failed_to_be_routed_count = Some(not_routed);
        counters.routed_count = Some(routed);
        counters.incomplete_count =
            i32::try_from(BatchAutorouter::calculate_incomplete_count(board)).ok();
        // :321 — the second **ungated** fire.
        progress.on_event(&RoutingEvent::BoardUpdated {
            counters: counters.clone(),
        });

        // :323-327 are the pass duration, `PerformanceProfiler.recordPass` and
        // `logBenchmarkProfile`; all three are rostered `not ported:`.

        // :329-330.
        Ok(routed > 0 || not_routed > 0)
    }

    /// Port of the private `updateProgress(RouterCounters, int, int, int, int, int)`
    /// (`:489-516`).
    ///
    /// Two independent things, in Java's order: the periodic
    /// [`BatchAutorouter::progress_statistics`] rebuild every
    /// [`BatchAutorouter::PROGRESS_STATISTICS_ITEM_INTERVAL`] items (`:496-505`), and the
    /// throttled board-update event (`:507-515`), gated by
    /// [`BatchAutorouter::should_fire_board_update`] — the only one of the pass's three fires
    /// that is gated.
    ///
    /// `router` and `board` are Java's `router.` and `router.board.` prefixes; `progress` is the
    /// listener list ruling AK replaced.
    #[allow(clippy::too_many_arguments)]
    fn update_progress(
        board: &mut Board,
        router: &mut BatchAutorouter<'_>,
        progress: &mut dyn ProgressSink,
        counters: &mut RouterCounters,
        items_to_go_count: i32,
        ripped_item_count: i32,
        not_routed: i32,
        routed: i32,
        skipped: i32,
    ) {
        // :496.
        router.progress_items_since_statistics += 1;
        // :497-505.
        if router.progress_items_since_statistics
            >= BatchAutorouter::PROGRESS_STATISTICS_ITEM_INTERVAL
        {
            // :500-501.
            BoardStatistics::compute_side_effects(board, false);
            router.progress_items_since_statistics = 0;
        }

        // :507.
        if router.should_fire_board_update() {
            // :508-513.
            counters.queued_to_be_routed_count = Some(items_to_go_count);
            counters.skipped_count = Some(skipped);
            counters.ripped_count = Some(ripped_item_count);
            counters.failed_to_be_routed_count = Some(not_routed);
            counters.routed_count = Some(routed);
            counters.incomplete_count =
                i32::try_from(BatchAutorouter::calculate_incomplete_count(board)).ok();
            // :514.
            progress.on_event(&RoutingEvent::BoardUpdated {
                counters: counters.clone(),
            });
        }
    }
}
