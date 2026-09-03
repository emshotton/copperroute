//! Port of `autoroute/pipeline/RoutingPipeline.java` (143 lines) — the thin sequencer above
//! [`AutorouteBatchLoop::run`] (Task 10, itself the fanout pre-pass plus the routing pass loop)
//! and [`BatchOptimizer::run_batch_loop`] (Task 14) that Plan 8's `fr-core` wraps (controller
//! ruling AK). Plan 6 stopped at one connection; Plan 7 Tasks 8-14 built the two stages; this is
//! where they become one function call.
//!
//! # What collapses, and why
//!
//! Java's `RoutingPipeline` is a stateful object: a private constructor builds a
//! [`BatchAutorouter`] and, when `settings.getRunOptimizer()` says so
//! (`RoutingPipeline.java:36`), a [`BatchOptimizer`] — both held for the object's lifetime so the
//! GUI can poll them between passes. The headless path (`createForHeadless`,
//! `RoutingPipeline.java:45-47`) never polls anything mid-run; both stage constructors are Tasks
//! 8/13's own `for_routing_job`/`new`, callable inline, so [`run_pipeline`] builds each stage
//! where Java's constructor would have and drops it the moment the stage returns. `getAutorouter`/
//! `getOptimizer` (`:50-57`) — the GUI's only reason to keep the objects around — have no
//! counterpart; [`PipelineResult`] carries what their only headless reader,
//! `RoutingJobSchedulerActionThread.java:170-172`'s `fanoutTimedOut`/`optimizerTimedOut` pair,
//! actually needs.
//!
//! `StageListener` (`:15-25`) and the three `add*Listener` methods (`:59-78`) are controller
//! ruling AK's whole point: replaced by [`ProgressSink`], landed in Plan 7 Task 4. Nothing this
//! task's own decisions read fires a *new* event for them (ruling 11) — see the per-call notes
//! below for why each of the three callback points has nothing to fire into.
//!
//! `createForGui` (`:39-42`) and the object-holding constructor's GUI half are
//! `// not ported:` — no headless caller, same roster line `BatchOptimizer.createForGui` already
//! carries (Task 14's report §9).

use fr_settings::RouterSettings;

use crate::error::RouterError;
use crate::pipeline::batch_loop::{AutorouteBatchLoop, BatchLoopResult};
use crate::pipeline::fanout::FanoutRunSummary;
use crate::pipeline::optimizer::BatchOptimizer;
use crate::pipeline::stop::{PassRecord, RouterBudget, RouterStop};
use crate::pipeline::{ProgressSink, TaskState};
use crate::score::BoardStatistics;
use fr_board::Board;

// not ported: `RoutingPipeline.createForGui` (`:39-42`) — the GUI factory: `BatchOptimizer::
// createForGui`, `GuiRoutingJobWorker.java:212` and `FeatureFlagsSettings.java:11` are its only
// other threads, all GUI. `BatchOptimizer.createForGui` itself is rostered in `pipeline/
// optimizer.rs` (Task 14's report §9); this line covers the pipeline-level factory the same way.
//
// renamed: `RoutingPipeline.createForHeadless` (`:45-47`) -> `run_pipeline`'s own setup — the
// headless path never polls the stage objects mid-run, so there is nothing the factory built that
// this function's own local variables do not already build inline (see the module doc's "What
// collapses" section).
//
// not ported: `RoutingPipeline.getAutorouter` (`:50-52`) and `RoutingPipeline.getOptimizer`
// (`:54-57`) — the GUI's only reason to hold the stage objects between passes is polling them
// (`GuiRoutingJobWorker.java:213-214`); the headless job scheduler's own two reads
// (`RoutingJobSchedulerActionThread.java:170-172`, `fanoutTimedOut`/`optimizerTimedOut`) are
// exactly what `PipelineResult::fanout`'s `is_timed_out` and the per-stage half of
// `PipelineResult::timed_out` (below) carry instead of a live object to call `.isTimedOut()` on.
//
// not ported: `RoutingPipeline.addStageListener` (`:59-61`).
// not ported: `RoutingPipeline.addBoardUpdatedEventListener` (`:64-69`).
// not ported: `RoutingPipeline.addTaskStateChangedEventListener` (`:72-78`).
//
// All three, and `StageListener` (`:15-25`) with them — controller ruling AK replaces
// `NamedAlgorithm`'s three listener lists (and this class's own forwarding pair) with
// `pipeline::ProgressSink`, landed in Plan 7 Task 4. `StageListener::afterRouting`/
// `beforeOptimization`/`afterOptimization` have no `RoutingEvent` variant because nothing
// downstream of this task's own decisions reads one (ruling 11: the sink is an observer, never an
// input) — a later task that needs one must widen the enum, not open a parallel channel.

/// Port of `RoutingPipeline.run()` (`RoutingPipeline.java:81-129`, plus the constructor's
/// `normalizeRouterAlgorithm` call and the object construction the module doc explains) — the
/// full routing job: fanout, autoroute passes and optimizer passes, in that order.
///
/// # The two stages, in Java's own order
///
/// **`runRoutingStage`** (`:87-114`): `routerEnabled = getRunRouter() && (maxPasses == null ||
/// maxPasses >= 0)` (`:88-91`). If `routerEnabled` and the stop flag is not even
/// `AUTO_ROUTER_ONLY`, [`AutorouteBatchLoop::run`] runs (`:97-98`) — which is Task 12's fanout
/// pre-pass **and** the pass loop, both in one call, because `BatchAutorouter.runBatchLoop()` is
/// `return batchLoop.run();` and nothing more. Otherwise, if fanout alone is enabled and the same
/// stop check passes, the **fanout-only mode** runs the identical call on a settings clone whose
/// `maxPasses` is forced to `0` (`:99-108`) — quirk #140's "unlimited", not "no passes"; see the
/// note on [`run_pipeline`]'s fanout-only branch below for why the port cannot mutate the
/// caller's settings in place the way Java's `finally` restores its shared object. Either way,
/// `:110`'s `job.board.finishAutoroute()` is next — see the note at its call site for why the
/// port has nothing to call.
///
/// **`runOptimizationStage`** (`:116-129`): skipped when there is no optimizer (`runOptimizer`
/// was false) **or** the stop flag is `ALL` (`:117-119` — not `AUTO_ROUTER_ONLY`: an ordinary
/// router run leaves the flag at exactly that value and this stage still starts). **Java never
/// resets the flag between stages**, and that is quirk #227: the stage started, visited every
/// item and changed nothing, because `BatchAutorouter.autoroutePassesForOptimizingItem`'s loop
/// head reads `isStopAutoRouterRequested()` — `!= NONE` — and ran zero passes per item.
///
// fixed: T9 (#227) — the stage-scoped stop. `run_pipeline` calls
// [`RouterStop::begin_optimizer_stage`] immediately after `:117`'s `ALL` gate, which lowers
// `AUTO_ROUTER_ONLY` to `NONE` and leaves `ALL` alone. It is the **only** call site in the tree.
///
/// # Ruling 7's sole new recovery boundary
///
/// [`AutorouteBatchLoop::run`]'s `IllegalArgumentException` (`:51-56`, thrown at `:55` when no
/// layer is both active and a signal layer) is **not** caught anywhere in `RoutingPipeline.run`,
/// so it escapes to the job scheduler; `run_pipeline` answers [`RouterError::NoRoutableLayer`]
/// for it, via the `?` on both stage calls, exactly as it would for any other `RouterError` a
/// deeper call raised (Java catches nothing in this method either way).
pub fn run_pipeline(
    board: &mut Board,
    settings: &RouterSettings,
    stop: &RouterStop,
    budget: RouterBudget,
    progress: &mut dyn ProgressSink,
) -> Result<PipelineResult, RouterError> {
    // ---- runRoutingStage (`:87-114`) -----------------------------------------------------------

    // `:88-91`.
    let router_enabled =
        settings.get_run_router() && settings.max_passes.is_none_or(|max| max >= 0);

    // `:93-95` — `job.stage = RoutingStage.ROUTING`. `core.RoutingJob` is Plan 8's `fr-core`
    // type; nothing in this crate reads a job stage, so the write has no port-side counterpart.
    // not ported: `RoutingPipeline`'s `job.stage = RoutingStage.ROUTING`/`OPTIMIZATION`/`IDLE`
    // writes (`:94`, `:121`, `:84`) — `core.RoutingJob.stage` belongs to Plan 8.

    let router_loop: Option<BatchLoopResult> =
        if router_enabled && !stop.is_stop_auto_router_requested() {
            // `:97-98`.
            Some(AutorouteBatchLoop::run(
                board, settings, stop, budget, progress,
            )?)
        } else if settings.is_fanout_enabled() && !stop.is_stop_auto_router_requested() {
            // `:99-108` — the fanout-only mode. Java mutates the shared `job.routerSettings.maxPasses`
            // in place and restores the original value in a `finally` once `runBatchLoop()` returns.
            // `run_pipeline`'s signature keeps `settings: &RouterSettings` — Plan 8's contract, per
            // the task brief — so there is no field to mutate and restore; the port clones the
            // settings, sets `max_passes = Some(0)` on the clone, runs the stage against it and drops
            // the clone when this branch ends. The caller's `settings` is never touched, which
            // `the_fanout_only_mode_sets_max_passes_to_zero_and_leaves_the_callers_settings_untouched`
            // asserts directly.
            let mut fanout_only_settings = settings.clone();
            fanout_only_settings.set_max_passes(Some(0));
            Some(AutorouteBatchLoop::run(
                board,
                &fanout_only_settings,
                stop,
                budget,
                progress,
            )?)
        } else {
            None
        };

    // `:110` — `this.job.board.finishAutoroute()`, the **only** caller of
    // `RoutingBoard.finishAutoroute` in the whole Java tree. Its whole body is
    // `if (autorouteEngine != null) autorouteEngine.clear(); autorouteEngine = null;`
    // (`RoutingBoard.java:899-905`) on a field this port never populates: controller ruling AJ
    // makes `retainAutorouteDatabase` permanently `false` (`BatchAutorouter::
    // BENCHMARK_RETAIN_AUTOROUTE_DATABASE`), so `RoutingBoardExt::init_autoroute` never hands
    // back an engine a caller retains, and no `AutorouteEngine` ever survives a lower-level call
    // into this scope for `RoutingBoardExt::finish_autoroute` — the trait method built to consume
    // exactly that value — to be given one. The Java call is therefore a no-op on every path this
    // port can reach, the same fact ruling AJ already recorded as the five `// not reachable:`
    // markers on `RoutingBoard.additionalUpdateAfterChange`'s call sites in `fr-board`.
    // not reachable: RoutingBoard.finishAutoroute (RoutingPipeline.java:110) — ruling AJ.

    // `:111-113` — `listener.afterRouting(autorouter)`. See the module doc's `StageListener`
    // paragraph: ruling AK replaces the mechanism and ruling 11 says nothing here fires a new
    // event, because no port decision downstream reads one.

    // ---- runOptimizationStage (`:116-129`) -----------------------------------------------------

    let mut optimizer_timed_out = false;
    // `BatchOptimizer.java:196`'s `job.setCurrentPass(currentPass)`, when the loop reached it.
    // `0` means it did not — see [`PipelineResult::optimizer_passes_completed`].
    let mut optimizer_last_reported_pass = 0;
    let optimizer_state = if settings.get_run_optimizer() {
        if stop.is_stop_requested() {
            // `:117-119` — `this.job.thread.isStopRequested()` is `ALL`, not `AUTO_ROUTER_ONLY`
            // (quirk #202/#227's other half). The stage never starts: no `job.stage` write, no
            // listener call, no `TaskStateChanged` event. `TaskState::Idle` — the type's own
            // "never left the start state" value — records "configured to run, but the stage
            // never entered", which [`PipelineResult::optimizer_state`] must tell apart from
            // `None` ("not configured at all", `RoutingPipeline.java:36`).
            Some(TaskState::Idle)
        } else {
            // fixed: T9 (#227) — **the stage-scoped stop.** `:117` has just decided the optimizer
            // may run, which is exactly where the register row asks for the reset. Every ordinary
            // exit from `AutorouteBatchLoop::run` left `AUTO_ROUTER_ONLY` behind (quirk #214's
            // five arms), and `BatchAutorouter::autoroute_passes_for_optimizing_item`'s loop head
            // is `!is_stop_auto_router_requested()` — `!= NONE` — so until this line every
            // `opt_route_item` in the stage below ripped its connections, ran **zero** autoroute
            // passes, measured a worse board and was restored from a whole-board deep copy. The
            // stage visited every item and produced the board it was given.
            //
            // [`RouterStop::begin_optimizer_stage`] lowers `AUTO_ROUTER_ONLY` to `NONE` and
            // nothing else: an `ALL` stop — an operator's cancel, or ruling AI's job deadline —
            // never reaches this line at all, because the `if` above returns `TaskState::Idle` on
            // it, and an `ALL` raised *during* the stage still ends it at `:171`. The three-state
            // stop is therefore preserved rather than collapsed (survey §9.1); see that method's
            // doc for the writer table that makes the downgrade safe.
            //
            // This is the single largest quality change in the catalogue and it **costs time on
            // every board**: an entire optimization stage begins working, at one whole-board deep
            // copy per item. Ruling BP4 pre-authorises the `cpu_s` rise and the task's A/B
            // measures it.
            stop.begin_optimizer_stage();
            // `:121-128` — `job.stage = OPTIMIZATION`; `beforeOptimization`/`afterOptimization`
            // (module doc); `optimizer.runBatchLoop()`.
            let mut optimizer = BatchOptimizer::new(settings);
            let result = optimizer.run_batch_loop(board, stop, budget, progress)?;
            optimizer_timed_out = result.timed_out;
            optimizer_last_reported_pass = result.last_reported_pass;
            Some(result.state)
        }
    } else {
        None
    };

    // `:84` — `this.job.stage = RoutingStage.IDLE`, reached only when neither stage threw. Same
    // "no `RoutingJob` here" reasoning as the two stage-entry writes above.

    // The report `PipelineResult` is built from: not a literal `RoutingPipeline.java` statement,
    // but the same computation `RoutingJobSchedulerActionThread.java:115`'s
    // `job.board.getStatistics()` performs, moved one level down so Plan 8's wrapper does not
    // have to repeat it.
    let final_statistics = BoardStatistics::new(board);

    let (router_state, router_passes_completed, fanout, per_pass) = match router_loop {
        Some(result) => (
            result.state,
            result.passes_run,
            result.fanout,
            result.per_pass,
        ),
        // Neither `if` nor `else if` at `:97-108` ran — routing was disabled, fanout was
        // disabled, or the stop flag was already raised at entry. `TaskState::Idle` again means
        // "the stage never started", matching `router_state`'s type (`TaskState`, not
        // `Option<TaskState>` — the routing stage is not optional the way the optimizer is).
        None => (TaskState::Idle, 0, None, Vec::new()),
    };
    // fixed: T9 (#267) — the two stages keep their own numbers. Java has one `job.currentPass`
    // field and both loops write it, so `RoutingResultManifest.fromJob:124-126` reports whichever
    // wrote last under a key that names the **autorouter**: one completed optimizer pass
    // overwrote a three-pass routing stage with `1`. There is no arbitration here any more,
    // because there is nothing to arbitrate.

    // Not a Java field: `RoutingJobSchedulerActionThread.java:170-172` composes exactly this pair
    // (`fanoutTimedOut`, `optimizerTimedOut`) itself, from a live `BatchAutorouter`/
    // `BatchOptimizer` this port never keeps around (the module doc's "what collapses" section).
    // `PipelineResult::timed_out` folds that pair into the one bool a Plan 8 caller needs,
    // widened by the router stage's own overall-deadline report (`router_state ==
    // TaskState::TimedOut`, `RouterStop::is_timed_out` — ruling AI's *job* deadline, never a
    // per-stage one) so the field answers "did anything not finish because of a clock" without
    // the caller inspecting three places.
    let timed_out = router_state == TaskState::TimedOut
        || fanout.as_ref().is_some_and(|f| f.is_timed_out)
        || optimizer_timed_out;

    Ok(PipelineResult {
        router_state,
        optimizer_state,
        router_passes_completed,
        optimizer_passes_completed: optimizer_last_reported_pass,
        fanout,
        per_pass,
        final_statistics,
        timed_out,
    })
}

/// What Plan 8's `RoutingResult` is built from — [`run_pipeline`]'s answer.
///
/// `board` is left in the caller's `board`; this is the report.
#[derive(Debug, Clone, PartialEq)]
pub struct PipelineResult {
    /// The routing stage's final [`TaskState`] (`AutorouteBatchLoop.java:571-585`, quirk
    /// candidate A) — [`TaskState::Idle`] when the stage never ran at all (`:97-108`'s two
    /// guards both false).
    pub router_state: TaskState,
    /// The optimizer stage's final [`TaskState`], or `None` when `runOptimizer` was false
    /// (`RoutingPipeline.java:36`) — the stage was never configured, not merely skipped. See
    /// [`run_pipeline`]'s doc for the `TaskState::Idle` case, which **is** configured but never
    /// entered (`:117-119`).
    pub optimizer_state: Option<TaskState>,
    /// **The routing stage's own completed pass count** (`AutorouteBatchLoop.java:275-277`,
    /// `:521`) — `0` when the stage never ran. Plan 8's CLI writes it into
    /// `fr_core::RoutingJob::set_current_pass`, which is what
    /// `RoutingResultManifest.fromJob:124-126` reports under `phases.autorouter.passes_completed`.
    ///
    // fixed: T9 (#267) — its own field. Java has **one** `job.currentPass` and both loops write
    // it, the router's counter starting at 1 and the optimizer's at 0, so `fromJob:124-126`
    // reported whichever wrote last under a key naming the autorouter: `router-dac2020-bm01` at
    // `-mp 2` logged two routing passes and one optimizer pass and its manifest said `1`.
    // fixed: T9 (#230) — and it is the honest number at every exit. Java's loop-local and
    // `job.currentPass` disagree by one on a `maxPasses`-capped exit, because `:270-274` breaks
    // before `:276` runs again for the pass the cap refused; the port does not count that pass.
    pub router_passes_completed: i32,
    /// **The optimizer stage's own completed pass count** — `BatchOptimizer.java:196`'s
    /// `job.setCurrentPass(currentPass)`, or `0` when the loop broke before reaching it (the
    /// per-stage deadline at `:172-176`, or the near-perfect exit at `:182-193`).
    ///
    /// Nothing in this crate reads it. Plan 8's CLI carries it on the job for
    /// `phases.optimizer.passes_completed`, **which Task 20 writes** — the key exists and is
    /// always `{}` today (quirk #254), and filling it is that task's, not this one's. What
    /// landed here is the field split.
    pub optimizer_passes_completed: i32,
    /// What the fanout pre-pass answered (`BatchFanout.FanoutRunSummary`,
    /// `AutorouteBatchLoop.java:625-629`), or `None` when it did not run.
    pub fanout: Option<FanoutRunSummary>,
    /// Ruling 1(a)'s per-pass diagnostic ladder for the routing stage, one entry per completed
    /// pass — empty when the stage never ran.
    pub per_pass: Vec<PassRecord>,
    /// The board's statistics once both stages have run — see `run_pipeline`'s doc for why this
    /// is a Plan-7 convenience rather than a literal `RoutingPipeline.java` read.
    pub final_statistics: BoardStatistics,
    /// Whether the router's overall deadline or either stage's own per-stage deadline tripped —
    /// see `run_pipeline`'s doc for the derivation and why it is not a single Java field.
    pub timed_out: bool,
}

/// Port of `RoutingPipeline.normalizeRouterAlgorithm(RoutingJob)` (`:131-142`): a CLI-supplied
/// `--router.algorithm` value that does not name the one algorithm this port (and HEAD's Java)
/// implements is replaced by [`RouterSettings::ALGORITHM_CURRENT`].
///
// renamed: takes `&str` and answers the normalized `String`, the same move Task 14's
// `BatchOptimizer::normalize_algorithm` makes for the identical reason: Java's parameter is the
// `RoutingJob` (`:132`'s read, `:134`'s `job.logWarning` — dropped, `global-constraints.md` —
// and `:140`'s write back through the job's settings object). `run_pipeline` borrows its settings
// immutably (the contract Plan 8 wraps, per the task brief), so there is no field to write
// through; the normalized value is handed back instead, for Plan 8 to write into the mutable
// settings it owns before constructing the job's `stop`/`board` and calling `run_pipeline` — the
// same point in the object's lifetime Java's own constructor calls it, before `run()` exists to
// be invoked at all.
pub fn normalize_router_algorithm(algorithm: &str) -> String {
    // `:133-140`. Java's own equality is `String.equals` — the algorithm names have no accented
    // or multi-codepoint content, so `==` on `&str` (a UTF-8 byte comparison) agrees with it.
    if algorithm == RouterSettings::ALGORITHM_CURRENT {
        algorithm.to_string()
    } else {
        RouterSettings::ALGORITHM_CURRENT.to_string()
    }
}
