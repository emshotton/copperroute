//! `RoutingPipeline` — the composition layer over Plan 7's
//! [`fr_router::pipeline::run_pipeline`].
//!
//! # What Java this stands for
//!
//! `autoroute/pipeline/RoutingPipeline.java`:
//!
//! * `createForHeadless(RoutingJob)` (`:45-47`) — `new RoutingPipeline(job,
//!   BatchOptimizer::createForHeadless)`. Its GUI twin `createForGui` (`:40-42`) is the only
//!   difference between the two factories, and it is the one that reaches
//!   `BatchOptimizerMultiThreaded` (quirk #143's dead branch). The port has one factory because
//!   it has one policy: [`RoutingPipeline::run`].
//! * `run()` (`:80-85`) — `runRoutingStage(); runOptimizationStage(); job.stage = IDLE;`.
//!
//! **Plan 7 ported the body.** `run_pipeline` is `runRoutingStage` (`:87-114`) plus
//! `runOptimizationStage` (`:116-129`), including quirk #227's no-reset (the identical `stop` is
//! handed to both stages) and the `:99-108` fanout-only clone. Controller ruling AK already
//! accounted for `createForGui` and the six `NamedAlgorithm` listener methods, and plan-7's
//! hand-off §4 records that *"`RoutingPipeline` is now wholly accounted for"*. What is left for
//! this crate is the **wrapper's shape**: what a headless caller needs around the routing that
//! `RoutingJobSchedulerActionThread.threadAction` does around `pipeline.run()`.
//!
//! # It composes; it does not decide
//!
//! Three additions, each with the Java line that asks for it, and **no fourth**:
//!
//! | addition | Java |
//! |---|---|
//! | the [`CancelToken`] adaptation | ruling AP; `RoutingJobSchedulerActionThread.java:75` |
//! | the DRC violations | `Freerouting.java:277-294`'s checker, run on the final board |
//! | the unrouted report | `AutorouteBatchLoop`'s ratsnest diagnostic, Plan 7's `build_unrouted_report` |
//!
//! And one deliberate **non**-addition: the final [`fr_router::score::BoardStatistics`] is
//! **taken from** [`fr_router::pipeline::PipelineResult::final_statistics`], never recomputed.
//! `BoardStatistics`' constructor runs `DesignRulesChecker` twice
//! (`core/scoring/BoardStatistics.java:265-268`, `:338-341`) and Plan 5's memoisation makes the
//! call count observable, so a second `BoardStatistics::compute` here would be a behaviour
//! change wearing the costume of a convenience.

// ── `RoutingJobSchedulerActionThread`'s three `StageListener` methods ───────────────────────────
//
// `threadAction` registers the action thread itself as the pipeline's `StageListener`
// (`RoutingJobSchedulerActionThread.java:101`), and the three callbacks only log and re-serialise:
// controller ruling AK replaces the whole listener mechanism with
// `fr_router::pipeline::ProgressSink`, and ruling 11 says nothing downstream reads it.
//
// not ported: RoutingJobSchedulerActionThread.afterRouting — a `FRLogger` line plus `setJobOutput`
// (quirk label K: the SES is re-serialised on every board update; plan ruling 9 writes once).
// not ported: RoutingJobSchedulerActionThread.beforeOptimization — likewise.
// not ported: RoutingJobSchedulerActionThread.afterOptimization — likewise.

use fr_board::Board;

use crate::Error;
use crate::ctx::{Ctx, RoutingResult};

/// `autoroute/pipeline/RoutingPipeline` — the headless factory (`:45-47`) and `run()` (`:80-85`),
/// as a namespace rather than an object.
///
/// renamed: Java's class holds four fields (`job`, `autorouter`, `optimizer`, `stageListeners`)
/// that the port has no use for. `job` is Task 1's type and is passed in around this call, not
/// held; `autorouter` and `optimizer` are constructed inside `run_pipeline` where Java constructs
/// them in the constructor (the difference is unobservable — neither is read between construction
/// and `run()`); and `stageListeners` is ruling AK's replaced mechanism. What is left is one
/// function, so the type is a unit struct that names the Java class for the audit rather than a
/// state machine with nothing in it.
#[derive(Debug, Clone, Copy, Default)]
pub struct RoutingPipeline;

impl RoutingPipeline {
    /// `RoutingPipeline.createForHeadless(job).run()`
    /// (`autoroute/pipeline/RoutingPipeline.java:45-47`, `:80-85`), wrapping Plan 7's
    /// [`fr_router::pipeline::run_pipeline`].
    ///
    /// The board is left routed in `board`; the answer is the report.
    ///
    /// # Order, and why it is this order
    ///
    /// 1. **The stop is built before the run** — [`crate::CancelToken::as_router_stop`]. A
    ///    cancel already requested is therefore visible to the very first stage guard
    ///    (`RoutingPipeline.java:97`'s `isStopAutoRouterRequested`), which is where Java's own
    ///    already-stopped job lands.
    /// 2. **`run_pipeline`**, with the caller's budget and the sink's `&mut` view.
    /// 3. **The unrouted report before the DRC.** Both take `&mut Board`, so the order is a
    ///    choice; `build_unrouted_report` is Plan 7's own and touches the ratsnest, and
    ///    `DesignRulesChecker::new` is Plan 5's and memoises. Running the report first keeps the
    ///    checker's memo state the same one a bare `-drc` run would see.
    ///
    /// # Errors
    ///
    /// Only what `run_pipeline` raises. Plan-7 ruling 7's boundary
    /// ([`fr_router::RouterError::NoRoutableLayer`], `AutorouteBatchLoop.java:51-56`) propagates
    /// here exactly as it escapes `RoutingPipeline.run` to the job scheduler in Java — this
    /// method adds **no** recovery boundary. Plan-8 ruling 4's single new `catch_unwind` is the
    /// MCP tool call, and the CLI deliberately gets none.
    pub fn run(board: &mut Board, ctx: &Ctx<'_>) -> Result<RoutingResult, Error> {
        // 1. Ruling AP's adapter as a constructor (scan ruling R3).
        let stop = ctx.cancel.as_router_stop();

        // 2. `RoutingPipeline.run()` — Plan 7's body, unchanged.
        let mut sink = ctx.progress.as_pipeline_sink();
        let pipeline =
            fr_router::pipeline::run_pipeline(board, ctx.settings, &stop, ctx.budget, &mut sink)?;

        // 3a. The ratsnest diagnostic.
        let unrouted_report = fr_router::pipeline::build_unrouted_report(board);

        // 3b. The final board's clearance violations.
        let drc_violations = fr_drc::DesignRulesChecker::new(board).get_all_clearance_violations();

        // `AutorouteBatchLoop.java:251-253` reads `job.state == TIMED_OUT`, which the monitor
        // thread writes (`RoutingJobSchedulerActionThread.java:84`). The port has no monitor
        // thread, so the two sources are the stop's own deadline poll and the token's — see
        // `RouterStop::poll_deadline` and `CancelToken::is_timed_out`.
        let timed_out = pipeline.timed_out || stop.is_timed_out() || ctx.cancel.is_timed_out();

        Ok(RoutingResult {
            // NOT a second `BoardStatistics::compute` — see the module doc.
            stats: pipeline.final_statistics.clone(),
            unrouted_report,
            drc_violations,
            timed_out,
            pipeline,
        })
    }
}
