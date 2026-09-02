//! Port of `autoroute/pipeline/AutorouteBatchLoop.java` (609 lines) — the pass loop, its
//! best-board policy and its two stagnation detectors.
//!
//! This is the whole `-dr`-equivalent routing stage: everything between "a board has just been
//! read" and "a board is ready for the SES writer". `RoutingPipeline.runRoutingStage`
//! (`RoutingPipeline.java:85-110`) reduces to one call into [`AutorouteBatchLoop::run`] plus
//! `finishAutoroute()`, which touches only a transient field.
//!
//! # `job.board = router.board` is the whole point of the file
//!
//! Java's `run` writes its answer into two fields — `router.board` (`:322`, `:535`) and then
//! `job.board` (`:552`) — and **`:552` is the assignment that decides which board is written to
//! SES.** Two of this method's arms replace the board wholesale with an *older* one taken out of
//! [`BoardHistory`]: the mid-loop restore at `:322` and the final swap at `:535`. The port has one
//! [`Board`], threaded as `&mut`, so both arms are `*board = …` and the caller's board is the
//! answer.

use std::time::Instant;

use fr_board::StopConnectionOption;
use fr_board::prelude::*;
use fr_settings::{RouterSettings, ScoringSettings};

use crate::error::RouterError;
use crate::pipeline::batch_autorouter::BatchAutorouter;
use crate::pipeline::board_history::BoardHistory;
use crate::pipeline::failure_log::RoutingFailureLog;
use crate::pipeline::fanout::{BatchFanout, FanoutRunSummary};
use crate::pipeline::stop::{PassRecord, RouterBudget, RouterStop};
use crate::pipeline::unrouted_report::build_unrouted_report;
use crate::pipeline::{NamedAlgorithmType, ProgressSink, RoutingEvent, TaskState};
use crate::score::BoardStatistics;

// =================================================================================================
// The seven constants — AutorouteBatchLoop.java:3-9's static imports
// =================================================================================================

// Java reaches all seven by `import static app.freerouting.autoroute.pipeline.BatchAutorouter.*`
// (`:3-9`), so they have exactly one definition and it is `BatchAutorouter`'s. The port does the
// same: these are aliases, not copies, and Plan 7 Task 8 transcribed each literal beside its Java
// line. Re-declaring them here with their own literals would let the two drift, and the whole
// point of the constants is that `BatchAutorouter.autoroutePassesForOptimizingItem` and this loop
// read the same numbers.

/// `STOP_AT_PASS_MINIMUM` (`BatchAutorouter.java:46`) — the minimum pass number, and the minimum
/// [`BoardHistory`] size, for the restore gate at `:298-299` and the stagnation guard at `:422`.
///
/// `i32`, not the plan sketch's `usize`: Java's is an `int` and it is compared against
/// `currentPass` twice for every `bh.size()` comparison. `:298` casts the size instead.
pub const STOP_AT_PASS_MINIMUM: i32 = BatchAutorouter::STOP_AT_PASS_MINIMUM;

/// `STOP_AT_PASS_MODULO` (`:49`) — the restore gate fires on every fourth pass (`:299`).
pub const STOP_AT_PASS_MODULO: i32 = BatchAutorouter::STOP_AT_PASS_MODULO;

/// `MAXIMUM_TRIES_ON_THE_SAME_BOARD` (`:42`) — [`BoardHistory::restore_board`]'s budget (`:307`).
pub const MAXIMUM_TRIES_ON_THE_SAME_BOARD: i32 = BatchAutorouter::MAXIMUM_TRIES_ON_THE_SAME_BOARD;

/// `BOARD_RANK_LIMIT` (`:40`) — `BoardHistory::MAX_HISTORY_SIZE`, i.e. **30**; the rank break at
/// `:317-320`.
pub const BOARD_RANK_LIMIT: usize = BatchAutorouter::BOARD_RANK_LIMIT;

/// `STAGNATION_PASS_LIMIT` (`:52`) — both stagnation windows (`:456`, `:486`).
pub const STAGNATION_PASS_LIMIT: i32 = BatchAutorouter::STAGNATION_PASS_LIMIT;

/// `FANOUT_RECOVERY_STAGNATION_PASSES` (`:54`) — the one-shot fanout recovery's trigger (`:438`).
pub const FANOUT_RECOVERY_STAGNATION_PASSES: i32 =
    BatchAutorouter::FANOUT_RECOVERY_STAGNATION_PASSES;

/// `STAGNATION_SCORE_THRESHOLD` (`:60`) — `0.5f`, the margin every "did the score improve?" test
/// in this file adds (`:425`, `:482`, `:510`).
pub const STAGNATION_SCORE_THRESHOLD: f32 = BatchAutorouter::STAGNATION_SCORE_THRESHOLD;

// =================================================================================================
// `BatchLoopResult` — what Java writes into fields
// =================================================================================================

/// What [`AutorouteBatchLoop::run`] answers.
///
/// renamed: **not a Java type.** Java's `run` returns one `boolean` and writes everything else into
/// fields of objects the port does not have. Each field names the Java site it is read from:
///
/// | field | Java |
/// |---|---|
/// | [`Self::state`] | `:571-585` — the `TaskState` the last `fireTaskStateChangedEvent` carries (quirk #214) |
/// | [`Self::continue_routing`] | `:587` — `return !thread.isStopAutoRouterRequested()` |
/// | [`Self::passes_run`] | `:520-522` — `currentPass`, as `:574`/`:583` report it |
/// | [`Self::last_reported_pass`] | `:276` — the value the loop last wrote into `job.currentPass` |
/// | [`Self::per_pass`] | ruling 1(a) — the acceptance ladder's per-pass tuple |
///
/// The board itself is **not** in here: Java's `:552` writes `job.board`, and the port's `board`
/// argument is that field.
#[derive(Debug, Clone, PartialEq)]
pub struct BatchLoopResult {
    /// The state `:571-585` reports. **A normal end of routing is `Cancelled`** — see quirk #214
    /// and [`AutorouteBatchLoop::run`]'s doc.
    pub state: TaskState,
    /// `:587` — Java's return value: `!thread.isStopAutoRouterRequested()`.
    pub continue_routing: bool,
    /// `currentPass` as the loop left it (`:520-522`). Because `:521` increments only when the
    /// loop is going round again, a run that stopped at `maxPasses = n` leaves this at `n + 1`.
    pub passes_run: i32,
    /// The value `:276`'s `job.setCurrentPass(currentPass)` last wrote — `0` when the loop never
    /// reached it.
    ///
    /// **Not** [`Self::passes_run`], and the difference is quirk #230: `:270-274`'s cap check
    /// runs *before* `:276`, so a `maxPasses`-capped exit leaves the job's field one behind the
    /// local. Plan 8's CLI writes this into [`fr_core::RoutingJob::set_current_pass`], because
    /// `RoutingResultManifest.fromJob:124-126` reports the **job's** field and not the loop's.
    /// This crate has no `RoutingJob` to write, so it hands the value back instead.
    pub last_reported_pass: i32,
    /// What `BatchFanout.fanoutBoard` answered (`:123-172`), or `None` when the fanout stage did
    /// not run — `settings.fanout.enabled` off (`:89`) or a board with no SMD pins at all
    /// (`:90-91`). Java keeps no such field: the summary is a local, read twice, at `:173`
    /// (`router.fanoutTimedOut`) and by the `:198-216` `job.logInfo` payload. The port hands it back
    /// because it is the only evidence that the pre-pass ran, and `p7t9 router+fanout` prints it.
    ///
    /// Its `total_duration_millis` is wall clock and must never be compared.
    pub fanout: Option<FanoutRunSummary>,
    /// Ruling 1(a)'s per-pass tuple, one entry per **completed** pass, in pass order.
    ///
    /// Filled where Java writes its `"Auto-routing pass #%d … completed … with score %s"` line
    /// (`:353-363`): after the best-board restore has had its say (`:298-344`) and before the
    /// stagnation block (`:422-517`). That is the point at which `boardStatisticsAfter` and
    /// `boardScoreAfter` are the numbers Java *reports* for the pass. The one-shot fanout recovery
    /// at `:441-442` recomputes both **after** this point, so a `router+fanout` run records the
    /// pre-recovery numbers — which is exactly what Java's log line says.
    pub per_pass: Vec<PassRecord>,
}

// =================================================================================================
// `AutorouteBatchLoop`
// =================================================================================================

/// Port of `autoroute.pipeline.AutorouteBatchLoop` (`AutorouteBatchLoop.java:29-609`).
///
/// Java's class is a one-field wrapper around its [`BatchAutorouter`] (`:31-35`) whose only
/// public-ish member is `run()`; the port has no field to hold, because `run` builds the router it
/// needs. It is a unit struct rather than a free function so that the five private delegates
/// (`:590-608`) have a place to be rostered beside the code.
pub struct AutorouteBatchLoop;

impl AutorouteBatchLoop {
    /// Port of `run()` (`AutorouteBatchLoop.java:37-588`) — the pass loop.
    ///
    /// Answers Java's `!thread.isStopAutoRouterRequested()` (`:587`) as
    /// [`BatchLoopResult::continue_routing`], with the four other things Java writes into fields
    /// beside it. **`board` is Java's `job.board` (`:552`)**: on return it is the board the run
    /// chose, which may be an *older* one restored out of [`BoardHistory`].
    ///
    /// # Java bug (quirk #214): a normal end of routing reports `CANCELLED`, not `FINISHED`
    ///
    /// `:571` reports [`TaskState::Finished`] only when the stop flag is still `NONE`. **Every
    /// ordinary exit from the loop raises it first** — `maxPasses` (`:271`), "not able to improve"
    /// (`:311`), the rank limit (`:318`) and both stagnation windows (`:474`, `:505`). The only
    /// path that leaves the flag `NONE` is the `while` head's own `continueAutorouting == false`,
    /// i.e. a pass that routed nothing at all. So a run that stops because it hit its pass budget
    /// — the CLI's normal case — reports `CANCELLED`, and an API consumer watching
    /// `TaskStateChangedEvent` cannot tell it from a user cancellation.
    ///
    /// # Ruling 7's sole new recovery boundary
    ///
    /// `:51-56` fires a `TaskState.CANCELLED` event **and then throws**
    /// `IllegalArgumentException`, which `RoutingPipeline.run` does not catch. The port answers
    /// [`RouterError::NoRoutableLayer`] after firing the same event, and Task 15's `run_pipeline`
    /// propagates it. `empty_board.dsn` reaches it.
    ///
    /// # What is not here
    ///
    /// The `PerformanceProfiler.recordConfiguration` block (`:68-81`) and its two closing calls
    /// (`:568-569`), every `job.log*` payload, the two `FRLogger.traceEntry`/`traceExit` wrappers
    /// (`:286-291`, `:345-351`) and the per-net incomplete breakdown trace (`:378-406`) are all
    /// `FRLogger`/profiler, which `global-constraints.md` drops; see the roster at the foot of
    /// this file.
    pub fn run(
        board: &mut Board,
        settings: &RouterSettings,
        stop: &RouterStop,
        budget: RouterBudget,
        progress: &mut dyn ProgressSink,
    ) -> Result<BatchLoopResult, RouterError> {
        // :38-42. Java's five locals are fields of the `BatchAutorouter` its constructor was
        // handed; the port builds that router here, exactly as `RoutingPipeline`'s constructor
        // does (`RoutingPipeline.java:34`, `new BatchAutorouter(job)`), because `run` is reached
        // only through `BatchAutorouter.runBatchLoop` (`:479-481`).
        //
        // `board` is Java's `RoutingBoard board = router.board` (`:38`) **and** `router.board`
        // itself — the port has one board, which is why quirk #209's staleness cannot happen here.
        let mut router = BatchAutorouter::for_routing_job(board, settings, budget);
        // `:42`'s `isOptimizerAutorouter` stays `false`: its only reader is `:374`'s log gate, and
        // the optimizer's own loop is `BatchOptimizer.runBatchLoop`, not this one.

        // `job.routerSettings.scoring`, read at `:283`, `:296`, `:328`, `:442`, `:529` and `:545`.
        // Java dereferences it with no null check, so an absent block is Java's NPE and the port
        // says so rather than substituting a default.
        let scoring: &ScoringSettings = settings
            .scoring
            .as_ref()
            .expect("RouterSettings.scoring — AutorouteBatchLoop.java:283 dereferences it");

        // :44-50.
        let mut any_routable = false;
        for i in 0..settings.get_layer_count() {
            if settings.get_layer_active(i) && board.layer_structure().layers[i].is_signal {
                any_routable = true;
                break;
            }
        }
        // :51-56 — the event, and then the throw at `:55`. Ruling 7's sole new boundary: it
        // **propagates**, it does not degrade.
        if !any_routable {
            // :52 is `FRLogger.warn`; :53-54.
            progress.on_event(&RoutingEvent::TaskStateChanged {
                algorithm: NamedAlgorithmType::Router,
                state: TaskState::Cancelled,
            });
            // :55.
            return Err(RouterError::NoRoutableLayer);
        }

        // :58-59.
        progress.on_event(&RoutingEvent::TaskStateChanged {
            algorithm: NamedAlgorithmType::Router,
            state: TaskState::Started,
        });

        // :62-63.
        router.session_start_time = Some(Instant::now());
        router.initial_unrouted_count =
            i32::try_from(BatchAutorouter::calculate_incomplete_count(board)).unwrap_or(i32::MAX);

        // :65.
        let mut bh = BoardHistory::new(scoring);

        // :68-81 — `PerformanceProfiler.recordConfiguration`; see the roster.

        // :83-218 — the SMD fanout pre-pass. **Task 12 discharged Task 10's loud `assert!`**;
        // `:83-87` is the `job.logDebug` payload that opens the block.
        //
        // `:93-96` and `:175-216` are `AutorouteRuntimeMetrics` — the CPU/heap report and the two
        // `job.logInfo` summaries built from it, rostered at the foot of this file. `:100-110`'s
        // two counters (`netConnectedSmdPins`, `alreadyConnectedAtStart`) and `:111` feed only
        // the `:111-122` `job.logInfo` line, and `BatchFanout`'s own constructor recomputes both as
        // [`BatchFanout::total_smd_pin_count`] and
        // [`BatchFanout::already_connected_pin_count`], so the port reads them off the summary's
        // escape statistics instead of counting the same pins twice.
        let mut fanout_summary: Option<FanoutRunSummary> = None;
        // :89.
        if settings.is_fanout_enabled() {
            // :90-92 — "the fanout stage is enabled but skipped because the board has no SMD
            // pins". Note the test is `getSmdPins()`, the **unfiltered** list, while
            // `BatchFanout`'s constructor filters to the net-carrying ones (`BatchFanout.java:43-51`):
            // a board whose every SMD pin is netless still enters the stage and runs
            // `maxPasses` empty passes.
            if !board.get_smd_pins().is_empty() {
                // :123-172. The listener Java passes (`:126-171`) fires a
                // `fireBoardUpdatedEvent` per tick and a `job.logInfo` per completed pass; ruling
                // AK makes both one `RoutingEvent::FanoutProgress`, which
                // [`BatchFanout::fanout_board`] fires directly.
                let summary = BatchFanout::fanout_board(board, settings, stop, budget, progress)?;
                // :173.
                router.fanout_timed_out = summary.is_timed_out;
                fanout_summary = Some(summary);
            }
        }

        // :220.
        let _current_unrouted = BatchAutorouter::calculate_incomplete_count(board);
        // :221-223. `maxPasses == 0` means **unlimited**, not "no passes" — quirk #140; the
        // `>= 0` is what lets `0` through, and `:268-270`'s `maxPasses > 0` is what makes it
        // unbounded.
        let is_router_enabled =
            settings.get_run_router() && settings.max_passes.is_none_or(|max| max >= 0);
        // :225-232 is the log payload.
        // :234.
        let mut continue_autorouting = is_router_enabled;

        // :236-242.
        let mut current_pass: i32 = 1;
        // The value `:276` last published into `job.currentPass`; see
        // [`BatchLoopResult::last_reported_pass`].
        let mut last_reported_pass: i32 = 0;
        let mut consecutive_no_improvement_passes: i32 = 0;
        let mut fanout_recovery_applied = false;
        let mut last_best_score = f32::NEG_INFINITY;
        let mut global_best_score = f32::NEG_INFINITY;
        let mut pass_of_best_score: i32 = 0;
        let mut _incomplete_count_at_best_score: usize = 0;

        // not ported: `alreadyRoutedBoardHashes` (`:249`) — a `HashSet<String>` whose **only two
        // readers are commented out** (`:259` and `:266`, inside the `:257-266` block Java
        // disabled). What survives is the allocation and two `.clear()` calls (`:334`, `:446`),
        // neither of which anything can observe. Quirk #216 is the row; there is nothing
        // behavioural to reproduce, and `the_dead_hash_set_is_javas_only_allocation` asserts this
        // very line so it cannot be deleted as noise.

        // The two pieces of pass-to-pass state Java hangs off objects the port does not have:
        // `board.failureLog` (a `RoutingBoard` field there — see `RoutingFailureLog`'s ownership
        // note) and `router.thread`, which is the caller's `stop`.
        let mut failure_log = RoutingFailureLog::new();
        let mut per_pass: Vec<PassRecord> = Vec::new();

        // :250.
        while continue_autorouting && !stop.is_stop_auto_router_requested() {
            // Controller ruling BB's poll seam (Plan 8 Task 11) — the **job-level** site. No Java
            // counterpart: Java's MCP cannot cancel a run at all (a documented delta,
            // `crates/freerouting/README.md`). `RouterStop::poll_cancel` is a `None` test on every
            // stop this crate or any parity driver builds, which is why this line moves no byte of
            // `batch_parity`, `p6t1`, `p8t1` or `sweep-p7t9.sh`. It sits **above** the
            // `poll_deadline` call because an external `requestStop()` and the monitor thread's
            // are the same `ALL`, and quirk #203's dead arm below must stay dead either way.
            stop.poll_cancel();

            // :251-253. Java reads `job.state == RoutingJobState.TIMED_OUT`; ruling AI's model of
            // the writer is `RouterStop::poll_deadline`, and this is one of its two permitted
            // sites (the other is `AutoroutePassRunner`'s item loop).
            //
            // Java bug: `AutorouteBatchLoop.java:251-253` — this is a **dead read** (quirk #203).
            // `requestStopAutoRouter` upgrades `NONE -> AUTO_ROUTER_ONLY` and nothing else
            // (`StoppableThread.java:33-37`), and the only writer of `TIMED_OUT` is the job
            // monitor, which called `requestStop()` — `ALL` — thirty seconds earlier
            // (`RoutingJobSchedulerActionThread.java:75, 84`). So by the time the guard can be
            // true the flag is already `ALL` and the call cannot change it. Reproduced rather than
            // dropped, because the *sequence* is what makes it dead: `poll_deadline` performs the
            // `request_stop()` first.
            if stop.poll_deadline() {
                // :252.
                stop.request_stop_auto_router();
            }

            // not ported: `String currentBoardHash = router.board.getHash()` (`:255`). Its only
            // live consumers are log payloads (`:280`, `:290`, `:350`, `:358`) — the two reads at
            // `:259` and `:266` that gave it a decision are commented out, and `:330` rewrites it
            // for the same log. Computing an MD5 over the whole item graph once per pass to
            // throw it away is not something the port reproduces.

            // :268-273. **`requestStopAutoRouter`**, i.e. `AUTO_ROUTER_ONLY` — not
            // `requestStop()`, which is what the `maxItems` stop raises. Quirk #202 is the
            // difference: hitting `--max-items` silently disables the optimizer stage and hitting
            // `--max-passes` does not.
            if settings
                .max_passes
                .is_some_and(|max| max > 0 && current_pass > max)
            {
                stop.request_stop_auto_router();
                break;
            }

            // :275-277 — `job.setCurrentPass(currentPass)`. The port has no `RoutingJob`
            // (Plan 8's, spec §13), so the value is recorded and handed back as
            // [`BatchLoopResult::last_reported_pass`]; Plan 8 Task 6's `commands::route` writes
            // it into the job, which is what the result manifest reads.
            last_reported_pass = current_pass;
            // :279-280.
            progress.on_event(&RoutingEvent::TaskStateChanged {
                algorithm: NamedAlgorithmType::Router,
                state: TaskState::Running,
            });

            // :282-283. Read for the `:335-341` log only — but computed **before** `bh.add`, and
            // `BoardStatistics`' constructor is what fills `DrillItem.center`'s lazy cache
            // (quirk #200), so its position is transcribed rather than optimised away.
            let _board_score_before = BoardStatistics::new(board).normalized_score(scoring);
            // :284.
            bh.add(board);

            // :293.
            continue_autorouting =
                router.autoroute_pass(board, &mut failure_log, current_pass, stop, progress)?;

            // :295-296.
            let mut board_statistics_after = BoardStatistics::new(board);
            let mut board_score_after = board_statistics_after.normalized_score(scoring);

            // :298-344 — the best-board restore.
            //
            // `:298`'s `bh.size() >= 8` is belt-and-braces: `getMaxScore()` seeds `0`, not
            // `-inf` (quirk #197), so `:306`'s strict `>` cannot fire on an empty history anyway.
            if restore_gate(
                bh.size(),
                current_pass,
                stop.is_stop_auto_router_requested(),
            ) {
                // :306 — a **strict** `>`, and Java's comment at `:301-305` says why: with
                // `>=` a board whose every entry scores the same would restore on every check
                // cycle and the history would grow without ever stopping.
                if bh.max_score() > board_score_after {
                    // :307.
                    let Some(board_to_restore) = bh.restore_board(MAXIMUM_TRIES_ON_THE_SAME_BOARD)
                    else {
                        // :308-313 — "The router was not able to improve the board".
                        stop.request_stop_auto_router();
                        break;
                    };

                    // :315. Order-dependent by construction: `restoreBoard` has just sorted
                    // the list in place and bumped one entry's `restoreCount` (quirk #198),
                    // so this rank is read off the *post-sort* order.
                    let board_to_restore_rank = bh.rank(&board_to_restore);

                    // :317-320.
                    if rank_limit_exceeded(board_to_restore_rank) {
                        stop.request_stop_auto_router();
                        break;
                    }

                    // :322-334 — **a fall-through, not an `else`**: the three arms above all
                    // `break`, so reaching here means the restore succeeded.
                    //
                    // `:322-323` is `router.board = boardToRestore; board = router.board`, the
                    // two-field assignment the port collapses into one move.
                    *board = board_to_restore;
                    // :324 — `router.board.getStatistics()`, which is
                    // `new BoardStatistics(this)` (RoutingBoard.java:1410-1412).
                    let board_statistics = BoardStatistics::new(board);
                    // :326.
                    consecutive_no_improvement_passes = 0;
                    // :327-328.
                    board_statistics_after = board_statistics;
                    board_score_after = board_statistics_after.normalized_score(scoring);
                    // :329.
                    last_best_score = board_score_after;
                    // :330 rewrites `currentBoardHash` for the log; `:334` clears the dead
                    // hash set. Neither is ported.
                }
            }

            // :345-376 — the pass-completed report. Ruling 1(a)'s tuple is filled here, where
            // Java's numbers for the pass are final; see `BatchLoopResult::per_pass`.
            per_pass.push(PassRecord {
                pass: current_pass,
                score: board_score_after,
                incomplete_count: stat(board_statistics_after.connections.incomplete_count),
                clearance_violations: stat(board_statistics_after.clearance_violations.total_count),
                via_count: stat(board_statistics_after.items.via_count),
                trace_count: stat(board_statistics_after.items.trace_count),
            });

            // :378-406 — the per-net incomplete breakdown; `FRLogger.trace` only, see the roster.

            // :408-410.
            if settings.save_intermediate_stages == Some(true) {
                progress.on_event(&RoutingEvent::BoardSnapshot { pass: current_pass });
            }

            // :422 — the stagnation detector's guard, and the reason `:509`'s `else if` is a bug.
            if stagnation_guard(current_pass, continue_autorouting) {
                // :425-427 — the pass-local counter, which a board restore resets.
                if board_score_after > last_best_score + STAGNATION_SCORE_THRESHOLD {
                    consecutive_no_improvement_passes = 0;
                    last_best_score = board_score_after;
                } else {
                    // :429.
                    consecutive_no_improvement_passes += 1;

                    // :435-454 — the one-shot fanout recovery. Task 10 could not reach it at
                    // all (`is_fanout_enabled()` was asserted `false`); Task 12 removed that
                    // stub, so `p7t9 router+fanout` is now on a path that can fire it.
                    if fanout_recovery_fires(
                        settings,
                        fanout_recovery_applied,
                        stat(board_statistics_after.connections.incomplete_count),
                        consecutive_no_improvement_passes,
                    ) {
                        // :440 — `removeTails(NONE)`, i.e. fanout vias included.
                        router.remove_tails(board, None, StopConnectionOption::None, &|| {
                            stop.is_stop_requested()
                        })?;
                        // :441-443.
                        board_statistics_after = BoardStatistics::new(board);
                        board_score_after = board_statistics_after.normalized_score(scoring);
                        last_best_score = board_score_after;
                        // :444-445.
                        consecutive_no_improvement_passes = 0;
                        fanout_recovery_applied = true;
                        // :446 clears the dead hash set; not ported.
                    }

                    // :456-476.
                    if consecutive_no_improvement_passes >= STAGNATION_PASS_LIMIT {
                        // :457.
                        let _report = build_unrouted_report(board);
                        // :474-475.
                        stop.request_stop_auto_router();
                        break;
                    }
                }

                // :482-485 — the global tracker, which a board restore does **not** reset.
                if board_score_after > global_best_score + STAGNATION_SCORE_THRESHOLD {
                    global_best_score = board_score_after;
                    pass_of_best_score = current_pass;
                    _incomplete_count_at_best_score =
                        stat(board_statistics_after.connections.incomplete_count);
                } else if (current_pass - pass_of_best_score) >= STAGNATION_PASS_LIMIT {
                    // :486-507.
                    let _report = build_unrouted_report(board);
                    // :505-506.
                    stop.request_stop_auto_router();
                    break;
                }

            // Java bug: `AutorouteBatchLoop.java:509-517` — this `else if` hangs off `:422`'s
            // `currentPass >= STOP_AT_PASS_MINIMUM && continueAutorouting` guard, so it runs on
            // passes **1-7** (and on any pass where the router has stopped making progress) and
            // never on pass 8 or later. Its own comment at `:511-514` says the opposite: it
            // describes a rule for "a fully-routed board with score == 0" that "must NOT reset the
            // stagnation counter … it should keep accumulating until the global tracker fires",
            // which only makes sense inside the `>= 8` arm. As written, a fully routed board
            // resets the counter exactly when the counter cannot yet fire, and stops resetting it
            // exactly when it can. Quirk #215.
            } else if stat(board_statistics_after.connections.incomplete_count) == 0
                && board_score_after > STAGNATION_SCORE_THRESHOLD
            {
                // :515-516.
                consecutive_no_improvement_passes = 0;
                last_best_score = board_score_after;
            }

            // :520-522.
            if continue_autorouting && !stop.is_stop_auto_router_requested() {
                current_pass += 1;
            }
        }

        // :525-550 — the final best-board swap.
        final_best_board_swap(board, &mut bh, scoring);

        // :552 — `job.board = router.board`. The port's `board` argument **is** `job.board`, and
        // the two `*board = …` assignments above are `:322` and `:535`.

        // :554-556. The same expression as `:221-223`, recomputed rather than reused — Java does,
        // and `settings.maxPasses` is a mutable field that `RoutingPipeline.java:101-104` really
        // does write between the two reads on the fanout-only path.
        let was_router_run =
            settings.get_run_router() && settings.max_passes.is_none_or(|max| max >= 0);
        // :557-563 — "clean up the route if the board is completed and if fanout is used". The
        // three-way `!(a || b || c)` means the tails come off only when the router ran, vias are
        // being kept (`removeUnconnectedVias == false`, i.e. fanout is on), the last pass reported
        // nothing left to do, and nothing raised the stop flag.
        if was_router_run
            && !(router.is_remove_unconnected_vias()
                || continue_autorouting
                || stop.is_stop_auto_router_requested())
        {
            // :562.
            router.remove_tails(board, None, StopConnectionOption::None, &|| {
                stop.is_stop_requested()
            })?;
        }

        // :565.
        bh.clear();

        // :567-569 — `PerformanceProfiler.printResults()` / `reset()`; see the roster.

        // :571-585. Quirk #214: `FINISHED` needs the flag still `NONE`, and every ordinary exit
        // raised it.
        let state = if !stop.is_stop_auto_router_requested() {
            // :572-574.
            TaskState::Finished
        } else if stop.is_timed_out() {
            // :578-584 — `job.state == RoutingJobState.TIMED_OUT`, which is what
            // `RouterStop::is_timed_out` models.
            TaskState::TimedOut
        } else {
            TaskState::Cancelled
        };
        progress.on_event(&RoutingEvent::TaskStateChanged {
            algorithm: NamedAlgorithmType::Router,
            state,
        });

        // :587.
        Ok(BatchLoopResult {
            state,
            continue_routing: !stop.is_stop_auto_router_requested(),
            passes_run: current_pass,
            last_reported_pass,
            fanout: fanout_summary,
            per_pass,
        })
    }
}

// =================================================================================================
// The four arms of `run`, lifted out so they can be pinned
// =================================================================================================
//
// Java's `run` is one 552-line method and these four expressions are inside it. They are lifted
// here — each still called from exactly one place, each carrying its Java range — because they are
// the four decisions the loop *makes*, and every one of them is either unreachable on the corpus
// (`final_best_board_swap`, `rank_limit_exceeded`) or reachable only after eight real passes
// (`restore_gate`, `stagnation_guard`). A test that had to route eight passes of a real board to
// reach a boolean would be a slow test of the router, not a test of the loop. Nothing else moved:
// `run` reads exactly as Java does with these four names substituted for the expressions.

/// `:298-300` — the two nested gates of the best-board restore, as the one predicate they are.
///
/// The outer gate (`:298`) wants a history of at least [`STOP_AT_PASS_MINIMUM`] entries; the inner
/// (`:299-300`) wants a pass number that is both a multiple of [`STOP_AT_PASS_MODULO`] **and** at
/// least [`STOP_AT_PASS_MINIMUM`]. **Either is satisfied outright by a raised stop flag**, which is
/// what makes the restore run one last time on the way out.
///
/// The outer gate is belt-and-braces: `BoardHistory::max_score` seeds `0` rather than `-inf`
/// (quirk #197), so `:306`'s strict `>` cannot fire on an empty history in any case.
pub fn restore_gate(
    history_size: usize,
    current_pass: i32,
    stop_auto_router_requested: bool,
) -> bool {
    // :298.
    let size_gate = i32::try_from(history_size).unwrap_or(i32::MAX) >= STOP_AT_PASS_MINIMUM
        || stop_auto_router_requested;
    // :299-300.
    let modulo_gate = ((current_pass % STOP_AT_PASS_MODULO == 0)
        && (current_pass >= STOP_AT_PASS_MINIMUM))
        || stop_auto_router_requested;
    size_gate && modulo_gate
}

/// `:317-320` — the rank break.
///
/// # Java bug (quirk #217): this can never fire
///
/// [`BOARD_RANK_LIMIT`] **is** `BoardHistory::MAX_HISTORY_SIZE` (`BatchAutorouter.java:40`), and
/// `BoardHistory.getRank` answers a **1-indexed position in a list that `add` caps at
/// `MAX_HISTORY_SIZE`** (`BoardHistory.java:53-76`, `:173-186`) — so its range is `-1` for "not
/// found" and `1..=30` otherwise, and `rank > 30` has no solution. The `-1` case is false too.
///
/// `BatchAutorouter.java:38-39`'s comment on the constant says "Must not exceed
/// `BoardHistory.MAX_HISTORY_SIZE` **so the check can actually fire**" — which has the direction
/// backwards. For the check to fire the limit must be *strictly less than* the cap; setting it
/// equal is precisely the value that makes it dead. `the_rank_limit_can_never_fire` is the pin,
/// and it also shows the arm is transcribed correctly by exercising it one above the limit.
// Java bug: `AutorouteBatchLoop.java:317-320` — `boardToRestoreRank > BOARD_RANK_LIMIT` is
// unreachable, because `BOARD_RANK_LIMIT == BoardHistory.MAX_HISTORY_SIZE` and `getRank` is
// bounded by the history's own cap (quirk #217).
pub fn rank_limit_exceeded(rank: i32) -> bool {
    rank > i32::try_from(BOARD_RANK_LIMIT).unwrap_or(i32::MAX)
}

/// `:422` — the stagnation detector's guard, and therefore also the guard on `:509`'s `else if`.
///
/// # Java bug (quirk #215): the `else if` is attached to the wrong arm
///
/// `:509-517` resets the pass-local stagnation counter when the board is fully routed and scoring
/// above the threshold. It is the `else` of **this** predicate, so it runs on passes **1-7** — and
/// on any pass where `autoroutePass` has already answered `false` — and never on pass 8 or later.
/// Its own comment (`:511-514`) describes the opposite rule: "A fully-routed board with score == 0
/// … must NOT reset the stagnation counter; it should keep accumulating until the global tracker
/// fires", which is a statement about the arm the code cannot reach. As written the reset happens
/// exactly while the counter is still incapable of firing, and stops happening exactly when it
/// becomes capable.
///
/// Measured on the corpus: `scripts/differential/run.sh p7t9 <ecc83> 8 router-only` prints
/// `ROUTED-RESET pass=1` and `ROUTED-RESET pass=2` and nothing after that.
// Java bug: `AutorouteBatchLoop.java:422` with `:509-517` — the fully-routed counter reset hangs
// off the `currentPass >= STOP_AT_PASS_MINIMUM` guard, so it fires on passes 1-7 only, the
// opposite of the comment at `:511-514` (quirk #215).
pub fn stagnation_guard(current_pass: i32, continue_autorouting: bool) -> bool {
    current_pass >= STOP_AT_PASS_MINIMUM && continue_autorouting
}

/// `:435-439` — the one-shot fanout recovery's four-term guard.
///
/// ```java
/// if (settings.isFanoutEnabled()
///     && !fanoutRecoveryApplied
///     && boardStatisticsAfter.connections.incompleteCount > 0
///     && consecutiveNoImprovementPasses >= FANOUT_RECOVERY_STAGNATION_PASSES) {
/// ```
///
/// Lifted out because the arm needs **eight real passes of a stagnating board with fanout on**
/// to fire, which no unit test can afford and which none of Plan 7's fixtures produces: the
/// guard's four terms are the decision, and this is where they can be checked one at a time.
/// The body it guards — `removeTails(NONE)`, the two recomputed statistics and the two counter
/// resets — stays inline, because it mutates five of `run`'s locals.
///
/// `FANOUT_RECOVERY_STAGNATION_PASSES` is **3** and `STAGNATION_PASS_LIMIT` is **10**
/// (`BatchAutorouter.java:52-55`), so the recovery fires on the third pass without improvement
/// and the loop still has seven more before `:456`'s local stagnation break ends it — which is
/// what makes "one shot" meaningful rather than academic.
pub fn fanout_recovery_fires(
    settings: &RouterSettings,
    fanout_recovery_applied: bool,
    incomplete_count: usize,
    consecutive_no_improvement_passes: i32,
) -> bool {
    settings.is_fanout_enabled()
        && !fanout_recovery_applied
        && incomplete_count > 0
        && consecutive_no_improvement_passes >= FANOUT_RECOVERY_STAGNATION_PASSES
}

/// `:525-550` — the final best-board swap: **the last thing that can change which board reaches
/// SES.**
///
/// Every ordinary exit from the pass loop leaves the board the *last* pass produced, which may be
/// worse than one the history is still holding — a restore at `:322` puts an older board back and
/// the passes after it can make it worse again. This block asks the history one more time, with no
/// restore-count budget at all (`restoreBestBoard()` is `restoreBoard(0)`,
/// `BoardHistory.java:158-160`).
///
/// The comparison at `:531` is a **strict** `>`: a history entry that merely ties the current
/// board does not displace it. Combined with `getMaxScore`'s `0` seed (quirk #197) that means an
/// empty history never swaps, and a run whose every board scored `0` never swaps either.
///
/// Answers whether the board was replaced — `:533`'s `bestBoard != null`, which is the guard Java
/// puts on its own log line. Nothing in Java reads the answer; the port returns it so
/// `the_final_swap_takes_the_best_board_only_when_it_is_strictly_better` can see it.
pub fn final_best_board_swap(
    board: &mut Board,
    bh: &mut BoardHistory,
    scoring: &ScoringSettings,
) -> bool {
    // :528-529.
    let current_final_score = BoardStatistics::new(board).normalized_score(scoring);
    // :530.
    let best_history_score = bh.max_score();
    // :531.
    if best_history_score > current_final_score {
        // :532-535.
        if let Some(best_board) = bh.restore_best_board() {
            *board = best_board;
            // :537-548 build the log payload from two more `BoardStatistics`; not ported.
            return true;
        }
    }
    false
}

/// `BoardStatistics`' `Option<i32>` counters as the `usize` [`PassRecord`] carries.
///
/// Java's fields are boxed `Integer`s that the computing constructor always fills; a `None` here
/// is the `NullPointerException` Java would raise on `stats.connections.incompleteCount`, and it
/// says so rather than reading as a zero.
pub(crate) fn stat(value: Option<i32>) -> usize {
    usize::try_from(
        value.expect("BoardStatistics' computing constructor fills every count Java unboxes"),
    )
    .unwrap_or(0)
}

// =================================================================================================
// The stagnation report — Task 15's `AutorouteUnroutedReport::build`
// =================================================================================================

// discharged: `AutorouteBatchLoop`'s stagnation report (`:456-476`, `:486-507`) called
// `buildUnroutedConnectionsReport()` (`:602-604` -> `BatchAutorouter.java:483-485` ->
// `AutorouteUnroutedReport.build`), stubbed here as an empty-string placeholder until Task 15
// landed the real port at `pipeline::unrouted_report::build_unrouted_report`, used below.
//
// The report is a **log payload** either way — Java concatenates it into the `job.logInfo` string
// at `:473` and `:504` and does nothing else with it, so building the real string here still
// cannot change control flow — but it does mean this loop pays `AutorouteUnroutedReport.build`'s
// full cost (a fresh `DesignRulesChecker`, `calculateAllIncompletes`, `getAllAirlines`) on every
// stagnation exit, exactly as Java's `job.logInfo` call site does.

// =================================================================================================
// The deferral roster for `autoroute/pipeline/AutorouteBatchLoop.java`
// =================================================================================================

// The five private one-line delegates `:590-608` are **ported by inlining**, which is what the
// port's shape makes of them: each forwards to a `BatchAutorouter` member that `run` could have
// called directly, and the port's `run` does.
//
// | Java | port |
// |---|---|
// | `calculateIncompleteCount` (`:590-592`) | [`BatchAutorouter::calculate_incomplete_count`] at `:63` and `:220` |
// | `removeTails` (`:594-596`) | [`BatchAutorouter::remove_tails`] at `:440` and `:562` |
// | `autoroutePass` (`:598-600`) | [`BatchAutorouter::autoroute_pass`] at `:293` |
// | `buildUnroutedConnectionsReport` (`:602-604`) | `build_unrouted_report`, the Task 15 stub above |
// | `fireBoardSnapshotEvent` (`:606-608`) | `RoutingEvent::BoardSnapshot` at `:408-410` |
//
// not ported: the `PerformanceProfiler.recordConfiguration` block (`:67-81`) and the
// `PerformanceProfiler.printResults()` / `reset()` pair (`:567-569`) — the profiler is rostered
// `// not ported:` for the whole workspace in `crates/fr-router/src/lib.rs`; it accumulates
// per-configuration timing into static maps and prints them through `FRLogger`, and no routing
// decision reads it. `grep -n PerformanceProfiler autoroute/pipeline/AutorouteBatchLoop.java`
// answers exactly these three call sites.
// not ported: `AutorouteRuntimeMetrics.currentThreadCpuSeconds` / `currentThreadAllocatedMb` /
// `currentHeapUsageMb` / `cpuSecondsSnapshot` / `allocatedMemoryMbSnapshot` / `peakHeapMbSnapshot`
// (`:93-96`, `:129-132`, `:175-195`) — the fanout stage's CPU and heap report, a `job.logInfo`
// payload. The class itself is rostered in `crates/fr-router/src/lib.rs`.
// not ported: the per-net incomplete breakdown at `:378-406` — a throw-away `DesignRulesChecker`
// (`:378-379`) walked net by net to build two `FRLogger.trace` payloads (`:384-389`, `:396-406`);
// the checker is discarded at the end of the iteration.
//
// Dropping it is **measured, not assumed**, because `DesignRulesChecker::new` takes `&mut Board`
// in this port and `BoardStatistics`' constructor really does mutate the board it measures
// (Plan 7 Task 1's finding 2). The block was temporarily added back — the checker, the
// `calculate_all_incompletes()` and the whole `1..=maxNetNumber` walk — and `p7t9` still MATCHes
// on `rpi` at `maxPasses = 8`, `j2` at `8` and `ecc83` at `2` (88 / 227 / 402 lines, byte
// identical to the runs without it). The Java side is unchanged across that experiment, so a
// MATCH in both directions is what says the walk leaves no trace a later pass can see.
