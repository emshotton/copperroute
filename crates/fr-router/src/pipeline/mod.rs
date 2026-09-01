//! `fr_router::pipeline` — the batch pipeline above the maze search: the fanout pre-pass, the
//! pass loop, the pass runner, the batch autorouter and the optimizer (Plan 7).
//!
//! Plan 6 stopped at `AutorouteConnectionRouter.route`'s step 5, i.e. at one connection
//! ([`crate::route_connection`]). Everything between that and a `.ses` file lives here.
//!
//! # State
//!
//! **Task 11 of 18.** The module holds [`BoardHistory`] — `autoroute/BoardHistory.java`, the pass
//! loop's best-board memory (controller ruling AF) — and, from Task 4, the plumbing every later
//! task threads through: the three-state stop [`RouterStop`] with ruling AI's deadline,
//! [`RouterBudget`], [`ProgressThrottler`], [`RouterCounters`], [`PassRecord`],
//! [`NamedAlgorithmType`], [`TaskState`] and the [`ProgressSink`] seam.
//!
//! Task 8 added [`BatchAutorouter`] — the object the pass runner and the optimizer both drive —
//! and **Task 9 the pass itself**: [`BatchAutorouter::autoroute_items`] and
//! [`BatchAutorouter::autoroute_pass`], [`AutoroutePassRunner`] (`runSingleThread`),
//! [`RoutingFailureLog`] with its [`ItemFailureInfo`], [`ItemRouteResult`] and
//! [`calculate_airline`].
//!
//! **Task 10 added [`AutorouteBatchLoop`]** — `AutorouteBatchLoop.run`, the pass loop above the
//! pass runner, with its best-board policy and its two stagnation detectors, answering a
//! [`BatchLoopResult`]. That is the whole `-dr`-equivalent routing stage with fanout and the
//! optimizer off, and `scripts/differential/run.sh p7t9` is its whole-board evidence. Two arms of
//! it are still stubbed behind `obligation:` markers in `batch_loop.rs`: the fanout pre-pass
//! (Task 12, and the stub is **loud** — `run` asserts fanout is disabled) and the stagnation
//! report (Task 15, inert).
//!
//! **Task 11 added [`fanout`]** — `BatchFanout`'s type and constructor, the
//! [`FanoutComponent`] / [`FanoutPin`] ordering it builds, and the three records
//! [`EscapeStatistics`], [`FanoutPassStatus`] and [`FanoutRunSummary`]. The per-pin escape
//! router those loops call is
//! [`RoutingBoardExt::fanout`](crate::board_ext::RoutingBoardExt::fanout), landed in the same
//! task; `scripts/differential/run.sh p7t5` is the evidence for both.
//!
//! **Task 12 added the fanout stage itself** — [`BatchFanout::fanout_board`] and its pass loop,
//! with the oscillation detector and the hash stop of [`FanoutLoopState`] — and **discharged
//! Task 10's loud stub**: `AutorouteBatchLoop::run`'s pre-pass now calls it, so
//! [`BatchLoopResult::fanout`] carries a real summary and `p7t9 router+fanout` routes a board
//! end to end.
//!
//! **Task 13 added the optimizer's item half** — [`BatchOptimizer`] with its field block and
//! `createForHeadless` constructor, [`ReadSortedRouteItems`] (the visit order the whole optimizer
//! stage hangs off), `BatchOptimizer::opt_route_item` with plan-7 ruling 8's clone-based snapshot,
//! and [`BatchAutorouter::autoroute_passes_for_optimizing_item`], the optimizer's own autorouter.
//! `scripts/differential/run.sh p7t8` is the evidence. Tasks 14-15 add the pass loop above it.
//! `crates/fr-router/src/lib.rs`'s roster names every class still deferred and the task that
//! owns it.

pub mod airline;
pub mod batch_autorouter;
pub mod batch_loop;
pub mod board_history;
pub mod counters;
pub mod failure_log;
pub mod fanout;
pub mod item_route_result;
pub mod optimizer;
pub mod pass_runner;
pub mod stop;

pub use airline::calculate_airline;
pub use batch_autorouter::BatchAutorouter;
pub use batch_loop::{AutorouteBatchLoop, BatchLoopResult};
pub use board_history::{BoardHistory, BoardHistoryEntry, java_float_compare};
pub use counters::RouterCounters;
pub use failure_log::{ItemFailureInfo, RoutingFailureLog};
pub use fanout::{
    BatchFanout, EscapeStatistics, FanoutComponent, FanoutLoopState, FanoutPassStatus, FanoutPin,
    FanoutRunSummary, FanoutStop, fanout_pin_can_use_vias, fanout_ripup_costs,
    parse_timespan_seconds,
};
pub use item_route_result::ItemRouteResult;
pub use optimizer::{
    BatchOptimizer, OptimizerPassRecord, OptimizerResult, ReadSortedRouteItems,
    optimizer_near_perfect_exit, optimizer_ripup_costs, optimizer_route_improved,
};
pub use pass_runner::AutoroutePassRunner;
pub use stop::{PassRecord, ProgressThrottler, RouterBudget, RouterStop, StopRequestState};

use fr_board::ItemId;

// =================================================================================================
// The two small pipeline enums
// =================================================================================================

/// Port of `autoroute/pipeline/NamedAlgorithmType.java` (7 lines) — which of the two named
/// algorithms an event came from.
///
/// Variant order is Java's declaration order, pinned by
/// `crates/fr-router/tests/data/p7t4-stop-and-counters.txt`'s `[enums]` block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NamedAlgorithmType {
    /// `ROUTER` (`NamedAlgorithmType.java:5`) — `BatchAutorouter`.
    Router,
    /// `OPTIMIZER` (`:6`) — `BatchOptimizer`.
    Optimizer,
}

impl NamedAlgorithmType {
    /// Java's `Enum.ordinal()` — the declaration index.
    pub fn ordinal(self) -> i32 {
        match self {
            NamedAlgorithmType::Router => 0,
            NamedAlgorithmType::Optimizer => 1,
        }
    }
}

/// Port of `autoroute/pipeline/TaskState.java` (11 lines) — the execution state of an
/// autorouting task, transcribed constant for constant.
///
/// Six variants; the order is Java's declaration order and therefore its `ordinal()` order,
/// pinned by the probe transcript's `[enums]` block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum TaskState {
    /// `IDLE` (`TaskState.java:5`).
    #[default]
    Idle,
    /// `STARTED` (`:6`).
    Started,
    /// `RUNNING` (`:7`).
    Running,
    /// `FINISHED` (`:8`).
    Finished,
    /// `CANCELLED` (`:9`) — note the two `l`s, which is Java's spelling.
    Cancelled,
    /// `TIMED_OUT` (`:10`) — what `AutorouteBatchLoop.java:578-584` reports off
    /// [`RouterStop::is_timed_out`].
    TimedOut,
}

impl TaskState {
    /// Java's `Enum.ordinal()` — the declaration index.
    pub fn ordinal(self) -> i32 {
        match self {
            TaskState::Idle => 0,
            TaskState::Started => 1,
            TaskState::Running => 2,
            TaskState::Finished => 3,
            TaskState::Cancelled => 4,
            TaskState::TimedOut => 5,
        }
    }
}

// =================================================================================================
// `ProgressSink` — spec §10's listener seam (ruling AK)
// =================================================================================================

/// Spec §10's `ProgressSink`, which replaces `NamedAlgorithm`'s three listener lists
/// (`autoroute/pipeline/NamedAlgorithm.java:26-31`, with their `add`/`fire` pairs at `:82-93`,
/// `:95-110` and `:112-125`) and the six files of `autoroute/events/**` (controller ruling AK).
///
/// # Ruling 11: no port decision reads it
///
/// The sink is an observer and never an input. Java's own headless path runs with all three
/// listener lists empty — nothing calls `addBoardUpdatedEventListener` on the CLI route — so a
/// board routed with a recording sink must be byte-for-byte the board routed with
/// [`NoopProgressSink`]. `crates/fr-router/tests/stop_and_progress.rs`'s
/// `a_recording_sink_changes_no_board_byte` is the pin.
///
/// The trait takes `&mut self` because a sink that records has to write, and the pipeline
/// threads it as `&mut dyn ProgressSink` (ruling AK) — one sink, one owner, no listener list.
pub trait ProgressSink {
    /// Handed every event the pipeline fires. The default implementation drops it, which is
    /// Java with an empty listener list.
    fn on_event(&mut self, event: &RoutingEvent) {
        let _ = event;
    }
}

/// The default sink: Java with every listener list empty, which is the headless CLI's state.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoopProgressSink;

impl ProgressSink for NoopProgressSink {}

/// What the pipeline fires. One variant per Java `fire*` call the port keeps, each naming the
/// Java site it stands for.
///
/// renamed: Java has one `EventObject` subclass per kind (`autoroute/events/**`), each carrying a
/// `source` back-reference the port has no use for; the enum is the port's shape and its payloads
/// are the fields the Java events' getters expose — a *subset*, chosen for what a headless caller
/// can act on. `FanoutProgress`, for instance, keeps three of `FanoutPassStatus`'s thirteen
/// constructor arguments (`autoroute/pipeline/BatchFanout.java:546-576`).
///
/// **A later task that needs more must widen a variant here, not open a parallel channel.** Ruling
/// 11 holds only while this enum is the single progress path: a second channel (a callback
/// argument, a return-value side band, a field on a pipeline struct) would put progress data
/// somewhere `a_recording_sink_changes_no_board_byte` cannot see, and the "no port decision reads
/// the sink" guarantee would stop being checkable. Tasks 12 and 14 own the two variants most
/// likely to be short.
#[derive(Debug, Clone, PartialEq)]
pub enum RoutingEvent {
    /// `NamedAlgorithm.fireTaskStateChangedEvent` (`NamedAlgorithm.java:121-125`).
    TaskStateChanged {
        /// Which algorithm changed state — `NamedAlgorithm.getType` (`:80`).
        algorithm: NamedAlgorithmType,
        /// The new state.
        state: TaskState,
    },
    /// `NamedAlgorithm.fireBoardUpdatedEvent` (`NamedAlgorithm.java:104-110`), gated by
    /// `BatchAutorouter.shouldFireBoardUpdate` (`BatchAutorouter.java:335-343`).
    BoardUpdated {
        /// The per-pass counters the event carries (`BoardUpdatedEvent.getRouterCounters`).
        counters: RouterCounters,
    },
    /// `NamedAlgorithm.fireBoardSnapshotEvent` (`:88-93`), reached from
    /// `AutorouteBatchLoop.java:606-608`.
    BoardSnapshot {
        /// The pass the snapshot was taken after.
        pass: i32,
    },
    /// `BatchFanout.publishProgress` -> `FanoutProgressListener.onProgress`
    /// (`autoroute/pipeline/BatchFanout.java:544-576`), reduced to the three numbers a headless
    /// caller can act on.
    FanoutProgress {
        /// `FanoutPassStatus.passNo`, 1-based (`BatchFanout.java:563`, the `passNo + 1`
        /// argument).
        pass: i32,
        /// Pins routed in this pass.
        routed: i32,
        /// Pins still to go.
        pins_to_go: i32,
    },
    /// The optimizer's per-item improvement, fired where `BatchOptimizer` fires its throttled
    /// board update on an improved route (`autoroute/pipeline/BatchOptimizer.java:333-337`).
    OptimizerImproved {
        /// The item that improved.
        item: ItemId,
        /// The board's normalized score before the item was re-routed.
        score_before: f32,
        /// …and after.
        score_after: f32,
    },
}
