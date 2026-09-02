//! Spec §10's `Ctx` and `RoutingResult` — everything a routing run needs that is not the board,
//! and everything it answers that is not the board.

use crate::cancel::CancelToken;
use crate::progress::SyncProgressSink;

/// Spec §10's `Ctx`: the run's settings, its cancellation, its progress sink and ruling AI's
/// budget.
///
/// # What Java has here that the port does not, and why
///
/// Java's counterpart is `core/RoutingJob` plus the `HeadlessBoardManager` that holds the board
/// — an object graph with a mutable `RoutingBoard` field (`getRoutingBoard :255-257`,
/// `replaceRoutingBoard :276-278`) and a `getCurrentRoutingJob :570-572`. The port has no manager
/// object: the board is the `&mut Board` parameter [`crate::RoutingPipeline::run`] takes, and
/// `RoutingJob` is its own type (Task 1). Those three `HeadlessBoardManager` accessors are
/// `// renamed:` to exactly that, at their marker site in `fr-board`.
///
/// # No rng seed, and no `max_threads`
///
/// Spec §10 mentions both. The port has **neither**, and this comment is the record rather than
/// two fields nothing sets:
///
/// * **No seed.** Plan-6 ruling 5 forbids `rand` and there is no randomness anywhere in the
///   ported router. Java's one random knob, `ItemSelectionStrategy.RANDOM`, is GUI-only.
/// * **No `max_threads`.** Quirk #143, extended by Plan 7 Task 17: `RouterSettings.maxThreads`
///   and `optimizer.maxThreads` have **no live reader anywhere** in the Java tree, headless or
///   not, and controller ruling AQ keeps `-mt` parsed-and-dead. The port's threads are the MCP
///   transport's two, and they carry no routing policy (plan ruling 3).
#[derive(Debug)]
pub struct Ctx<'a> {
    /// The resolved settings for this run — `job.routerSettings`, after
    /// [`fr_settings::resolve_headless`]'s ladder.
    pub settings: &'a fr_settings::RouterSettings,
    /// Ruling AP's cancellation. Cloneable and shareable; see [`CancelToken`].
    pub cancel: CancelToken,
    /// Spec §10's progress sink. Borrowed so the caller can clone it into another thread.
    pub progress: &'a SyncProgressSink,
    /// Ruling AI's wall-clock knob. Every parity run passes
    /// [`fr_router::pipeline::RouterBudget::disabled`]; the CLI passes
    /// [`fr_router::pipeline::RouterBudget::default`], which is Java's four literals.
    pub budget: fr_router::pipeline::RouterBudget,
}

impl<'a> Ctx<'a> {
    /// A context with no cancellation and Java's own budget literals — what a plain
    /// `-de/-do` run is.
    pub fn new(
        settings: &'a fr_settings::RouterSettings,
        progress: &'a SyncProgressSink,
    ) -> Ctx<'a> {
        Ctx {
            settings,
            cancel: CancelToken::new(),
            progress,
            budget: fr_router::pipeline::RouterBudget::default(),
        }
    }

    /// [`Ctx::new`] with ruling AI's wall clock switched off — what every parity driver passes,
    /// on both sides.
    pub fn with_disabled_budget(
        settings: &'a fr_settings::RouterSettings,
        progress: &'a SyncProgressSink,
    ) -> Ctx<'a> {
        Ctx {
            budget: fr_router::pipeline::RouterBudget::disabled(),
            ..Ctx::new(settings, progress)
        }
    }
}

/// Spec §10's `RoutingResult` — what [`crate::RoutingPipeline::run`] answers.
///
/// The board itself is left in the caller's `&mut Board`, exactly as
/// [`fr_router::pipeline::run_pipeline`] leaves it; this is the report.
#[derive(Debug, Clone)]
pub struct RoutingResult {
    /// The board's statistics once both stages have run.
    ///
    /// **This is `pipeline.final_statistics`, not a second computation.**
    /// [`fr_router::pipeline::run_pipeline`] already computes it (`run.rs`, the
    /// `BoardStatistics::new(board)` at the foot), standing for
    /// `RoutingJobSchedulerActionThread.java:115`'s `job.board.getStatistics()`. Recomputing it
    /// would be observable, not merely wasteful: `BoardStatistics`' constructor runs
    /// `DesignRulesChecker` twice (`core/scoring/BoardStatistics.java:265-268`, `:338-341`), and
    /// Plan 5's memoisation makes the call count visible.
    pub stats: fr_router::score::BoardStatistics,
    /// `fr_router::pipeline::build_unrouted_report`'s answer — the jar's own diagnostic **text**
    /// for the connections still in the ratsnest.
    ///
    /// Scan ruling R4: the plan drafted `incompletes: Vec<(String, Vec<String>)>`, but
    /// `build_unrouted_report(board: &mut Board) -> String` is what Plan 7 pinned
    /// (`pipeline/unrouted_report.rs:23`). Structuring it here would be a second, port-only
    /// implementation of a method that is already byte-pinned. The **count** comes from
    /// [`RoutingResult::incomplete_count`].
    pub unrouted_report: String,
    /// `fr_drc::DesignRulesChecker::get_all_clearance_violations`' answer for the final board.
    pub drc_violations: Vec<fr_board::ClearanceViolation>,
    /// Whether the router's overall deadline or either stage's own deadline tripped —
    /// [`fr_router::pipeline::PipelineResult::timed_out`], widened by the [`CancelToken`]'s own
    /// `timed_out_at` (Java's `job.state == TIMED_OUT`, which the port has no monitor thread to
    /// write).
    pub timed_out: bool,
    /// Plan 7's whole answer, unmodified — pass records, task states, the fanout summary.
    pub pipeline: fr_router::pipeline::PipelineResult,
}

impl RoutingResult {
    /// The connections still in the ratsnest, from
    /// `BoardStatistics.connections.incompleteCount` (`core/scoring/BoardStatistics.java:271`)
    /// — scan ruling R4's replacement for the drafted `incompletes` vector's length.
    ///
    /// `None` is Java's null `Integer`, which the computing constructor leaves when
    /// `includeConnections = false` (`:265-271`). [`crate::RoutingPipeline::run`] always goes
    /// through `BoardStatistics::new`, which includes them, so the `None` arm is unreachable on
    /// this path; it is answered rather than unwrapped because the field is `Option` and Java's
    /// own unboxing there is an NPE, not a value.
    pub fn incomplete_count(&self) -> Option<i32> {
        self.stats.connections.incomplete_count
    }

    /// The number of clearance violations on the final board.
    pub fn violation_count(&self) -> usize {
        self.drc_violations.len()
    }
}
