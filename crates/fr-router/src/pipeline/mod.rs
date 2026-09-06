pub mod airline;
pub mod batch_autorouter;
pub mod batch_loop;
pub mod board_history;
pub mod board_prep;
pub mod connection_budget;
pub mod counters;
pub mod failure_log;
pub mod fanout;
pub mod item_route_result;
pub mod optimizer;
pub mod pass_runner;
pub mod run;
pub mod stop;
pub mod unrouted_report;

pub use airline::{calculate_airline, calculate_item_distance};
pub use batch_autorouter::BatchAutorouter;
pub use batch_loop::{AutorouteBatchLoop, BatchLoopExit, BatchLoopResult, StagnationStep};
pub use board_history::{BoardHistory, BoardHistoryEntry};
pub use board_prep::prepare_board;
pub use connection_budget::{ConnectionBudget, DEFAULT_CONNECTION_SEARCH_STEPS};
pub use counters::RouterCounters;
pub use failure_log::{ItemFailureInfo, RoutingFailureLog};
pub use fanout::{
    BatchFanout, EscapeStatistics, FanoutComponent, FanoutLoopState, FanoutPassStatus, FanoutPin,
    FanoutRunSummary, FanoutStop, TimespanError, fanout_pin_can_use_vias, fanout_ripup_costs,
    parse_timespan_seconds, parse_timespan_seconds_java,
};
pub use item_route_result::ItemRouteResult;
pub use optimizer::{
    BatchOptimizer, DEFAULT_OPTIMIZER_SEARCH_STEPS, OptimizerPassRecord, OptimizerResult,
    PORT_OPTIMIZER_ROUTE_WORK_BUDGET, ReadSortedRouteItems, optimizer_nothing_to_improve,
    optimizer_ripup_costs, optimizer_route_improved,
};
pub use pass_runner::AutoroutePassRunner;
pub use run::{PipelineResult, normalize_router_algorithm, run_pipeline};
pub use stop::{
    CancelPoll, DeterministicWorkBudget, PassRecord, ProgressThrottler, RouterBudget, RouterStop,
    StopRequestState,
};
pub use unrouted_report::build_unrouted_report;

use fr_board::ItemId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NamedAlgorithmType {
    Router,
    /// `OPTIMIZER` (`:6`) — `BatchOptimizer`.
    Optimizer,
}

impl NamedAlgorithmType {
    pub fn ordinal(self) -> i32 {
        match self {
            NamedAlgorithmType::Router => 0,
            NamedAlgorithmType::Optimizer => 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum TaskState {
    #[default]
    Idle,
    Started,
    Running,
    Finished,
    Cancelled,
    TimedOut,
}

impl TaskState {
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

pub trait ProgressSink {
    fn on_event(&mut self, event: &RoutingEvent) {
        let _ = event;
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct NoopProgressSink;

impl ProgressSink for NoopProgressSink {}

#[derive(Debug, Clone, PartialEq)]
pub enum RoutingEvent {
    TaskStateChanged {
        algorithm: NamedAlgorithmType,
        state: TaskState,
    },
    BoardUpdated {
        counters: RouterCounters,
    },
    BoardSnapshot {
        pass: i32,
    },
    FanoutProgress {
        pass: i32,
        routed: i32,
        pins_to_go: i32,
    },
    OptimizerImproved {
        item: ItemId,
        score_before: f32,
        score_after: f32,
    },
}
