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

pub const STOP_AT_PASS_MINIMUM: i32 = BatchAutorouter::STOP_AT_PASS_MINIMUM;

pub const STOP_AT_PASS_MODULO: i32 = BatchAutorouter::STOP_AT_PASS_MODULO;

pub const MAXIMUM_TRIES_ON_THE_SAME_BOARD: i32 = BatchAutorouter::MAXIMUM_TRIES_ON_THE_SAME_BOARD;

pub const BOARD_RANK_LIMIT: usize = BatchAutorouter::BOARD_RANK_LIMIT;

pub const STAGNATION_PASS_LIMIT: i32 = BatchAutorouter::STAGNATION_PASS_LIMIT;

pub const FANOUT_RECOVERY_STAGNATION_PASSES: i32 =
    BatchAutorouter::FANOUT_RECOVERY_STAGNATION_PASSES;

pub const STAGNATION_SCORE_THRESHOLD: f32 = BatchAutorouter::STAGNATION_SCORE_THRESHOLD;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BatchLoopExit {
    Completed,
    MaxPasses,
    NoImprovement,
    Stagnation,
    Cancelled,
}

impl BatchLoopExit {
    pub fn task_state(self, timed_out: bool) -> TaskState {
        match self {
            BatchLoopExit::Cancelled if timed_out => TaskState::TimedOut,
            BatchLoopExit::Cancelled => TaskState::Cancelled,
            BatchLoopExit::Completed
            | BatchLoopExit::MaxPasses
            | BatchLoopExit::NoImprovement
            | BatchLoopExit::Stagnation => TaskState::Finished,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BatchLoopResult {
    pub state: TaskState,
    pub exit: BatchLoopExit,
    pub continue_routing: bool,
    pub passes_run: i32,
    pub fanout: Option<FanoutRunSummary>,
    pub per_pass: Vec<PassRecord>,
}

impl BatchLoopResult {
    pub fn exit(&self) -> BatchLoopExit {
        self.exit
    }
}

pub struct AutorouteBatchLoop;

impl AutorouteBatchLoop {
    pub fn run(
        board: &mut Board,
        settings: &RouterSettings,
        stop: &RouterStop,
        budget: RouterBudget,
        progress: &mut dyn ProgressSink,
    ) -> Result<BatchLoopResult, RouterError> {
        let mut router = BatchAutorouter::for_routing_job(board, settings, budget);

        let scoring: &ScoringSettings = settings
            .scoring
            .as_ref()
            .expect("RouterSettings.scoring — AutorouteBatchLoop.java:283 dereferences it");

        let mut any_routable = false;
        for i in 0..settings.get_layer_count() {
            if settings.get_layer_active(i) && board.layer_structure().layers[i].is_signal {
                any_routable = true;
                break;
            }
        }
        if !any_routable {
            progress.on_event(&RoutingEvent::TaskStateChanged {
                algorithm: NamedAlgorithmType::Router,
                state: TaskState::Cancelled,
            });
            return Err(RouterError::NoRoutableLayer);
        }

        progress.on_event(&RoutingEvent::TaskStateChanged {
            algorithm: NamedAlgorithmType::Router,
            state: TaskState::Started,
        });

        router.session_start_time = Some(Instant::now());
        router.initial_unrouted_count =
            i32::try_from(BatchAutorouter::calculate_incomplete_count(board)).unwrap_or(i32::MAX);

        let mut bh = BoardHistory::new(scoring);

        let mut fanout_summary: Option<FanoutRunSummary> = None;
        if settings.is_fanout_enabled() {
            if !board.get_smd_pins().is_empty() {
                let summary = BatchFanout::fanout_board(board, settings, stop, budget, progress)?;
                router.fanout_timed_out = summary.is_timed_out;
                fanout_summary = Some(summary);
            }
        }

        let _current_unrouted = BatchAutorouter::calculate_incomplete_count(board);
        let is_router_enabled =
            settings.get_run_router() && settings.max_passes.is_none_or(|max| max >= 0);
        let mut continue_autorouting = is_router_enabled;

        let mut current_pass: i32 = 1;
        let mut passes_run = 0;
        let mut consecutive_no_improvement_passes: i32 = 0;
        let mut fanout_recovery_applied = false;
        let mut last_best_score = f32::NEG_INFINITY;
        let mut global_best_score = f32::NEG_INFINITY;
        let mut pass_of_best_score: i32 = 0;
        let mut _incomplete_count_at_best_score: usize = 0;

        let mut failure_log = RoutingFailureLog::new();
        let mut per_pass: Vec<PassRecord> = Vec::new();

        let mut exit: Option<BatchLoopExit> = None;

        while continue_autorouting && !stop.is_stop_auto_router_requested() {
            stop.poll_cancel();

            stop.poll_deadline();
            if stop.is_stop_auto_router_requested() {
                break;
            }

            if settings
                .max_passes
                .is_some_and(|max| max > 0 && current_pass > max)
            {
                stop.request_stop_auto_router();
                exit = Some(BatchLoopExit::MaxPasses);
                break;
            }

            progress.on_event(&RoutingEvent::TaskStateChanged {
                algorithm: NamedAlgorithmType::Router,
                state: TaskState::Running,
            });

            let _board_score_before = BoardStatistics::new(board).normalized_score(scoring);
            bh.add(board);

            continue_autorouting =
                router.autoroute_pass(board, &mut failure_log, current_pass, stop, progress)?;
            passes_run += 1;

            let mut board_statistics_after = BoardStatistics::new(board);
            let mut board_score_after = board_statistics_after.normalized_score(scoring);

            if restore_gate(
                bh.size(),
                current_pass,
                stop.is_stop_auto_router_requested(),
            ) {
                if bh.best_penalty() < board_statistics_after.routing_penalty(scoring) {
                    let Some(board_to_restore) = bh.restore_board(MAXIMUM_TRIES_ON_THE_SAME_BOARD)
                    else {
                        stop.request_stop_auto_router();
                        exit = Some(BatchLoopExit::NoImprovement);
                        break;
                    };

                    *board = board_to_restore;
                    let board_statistics = BoardStatistics::new(board);
                    consecutive_no_improvement_passes = 0;
                    board_statistics_after = board_statistics;
                    board_score_after = board_statistics_after.normalized_score(scoring);
                    last_best_score = board_score_after;
                }
            }

            per_pass.push(PassRecord {
                pass: current_pass,
                score: board_score_after,
                incomplete_count: stat(board_statistics_after.connections.incomplete_count),
                clearance_violations: stat(board_statistics_after.clearance_violations.total_count),
                via_count: stat(board_statistics_after.items.via_count),
                trace_count: stat(board_statistics_after.items.trace_count),
            });

            if settings.save_intermediate_stages == Some(true) {
                progress.on_event(&RoutingEvent::BoardSnapshot { pass: current_pass });
            }

            if stagnation_guard(current_pass, continue_autorouting) {
                match stagnation_step(
                    board_score_after,
                    last_best_score,
                    stat(board_statistics_after.connections.incomplete_count),
                ) {
                    StagnationStep::ScoreImproved => {
                        consecutive_no_improvement_passes = 0;
                        last_best_score = board_score_after;
                    }
                    StagnationStep::BoardRouted => {
                        consecutive_no_improvement_passes = 0;
                        last_best_score = board_score_after;
                    }
                    StagnationStep::Accumulate => {
                        consecutive_no_improvement_passes += 1;

                        if fanout_recovery_fires(
                            settings,
                            fanout_recovery_applied,
                            stat(board_statistics_after.connections.incomplete_count),
                            consecutive_no_improvement_passes,
                        ) {
                            router.remove_tails(
                                board,
                                None,
                                StopConnectionOption::None,
                                &|| stop.is_stopped_or_expired(),
                            )?;
                            board_statistics_after = BoardStatistics::new(board);
                            board_score_after = board_statistics_after.normalized_score(scoring);
                            last_best_score = board_score_after;
                            consecutive_no_improvement_passes = 0;
                            fanout_recovery_applied = true;
                        }

                        if consecutive_no_improvement_passes >= STAGNATION_PASS_LIMIT {
                            let _report = build_unrouted_report(board);
                            stop.request_stop_auto_router();
                            exit = Some(BatchLoopExit::Stagnation);
                            break;
                        }
                    }
                }

                if board_score_after > global_best_score + STAGNATION_SCORE_THRESHOLD {
                    global_best_score = board_score_after;
                    pass_of_best_score = current_pass;
                    _incomplete_count_at_best_score =
                        stat(board_statistics_after.connections.incomplete_count);
                } else if (current_pass - pass_of_best_score) >= STAGNATION_PASS_LIMIT {
                    let _report = build_unrouted_report(board);
                    stop.request_stop_auto_router();
                    exit = Some(BatchLoopExit::Stagnation);
                    break;
                }
            }

            if continue_autorouting && !stop.is_stop_auto_router_requested() {
                current_pass += 1;
            }
        }

        final_best_board_swap(board, &mut bh, scoring);

        let was_router_run =
            settings.get_run_router() && settings.max_passes.is_none_or(|max| max >= 0);
        if was_router_run
            && !(router.is_remove_unconnected_vias()
                || continue_autorouting
                || stop.is_stop_auto_router_requested())
        {
            router.remove_tails(board, None, StopConnectionOption::None, &|| {
                stop.is_stopped_or_expired()
            })?;
        }

        bh.clear();

        let exit = exit.unwrap_or({
            if continue_autorouting || stop.is_stop_auto_router_requested() {
                BatchLoopExit::Cancelled
            } else {
                BatchLoopExit::Completed
            }
        });
        let state = exit.task_state(stop.is_timed_out());
        progress.on_event(&RoutingEvent::TaskStateChanged {
            algorithm: NamedAlgorithmType::Router,
            state,
        });

        Ok(BatchLoopResult {
            state,
            exit,
            continue_routing: !stop.is_stop_auto_router_requested(),
            passes_run,
            fanout: fanout_summary,
            per_pass,
        })
    }
}

pub fn restore_gate(
    history_size: usize,
    current_pass: i32,
    stop_auto_router_requested: bool,
) -> bool {
    let size_gate = i32::try_from(history_size).unwrap_or(i32::MAX) >= STOP_AT_PASS_MINIMUM
        || stop_auto_router_requested;
    let modulo_gate = ((current_pass % STOP_AT_PASS_MODULO == 0)
        && (current_pass >= STOP_AT_PASS_MINIMUM))
        || stop_auto_router_requested;
    size_gate && modulo_gate
}

pub fn rank_limit_exceeded(rank: i32) -> bool {
    rank > i32::try_from(BOARD_RANK_LIMIT).unwrap_or(i32::MAX)
}

pub fn stagnation_guard(current_pass: i32, continue_autorouting: bool) -> bool {
    current_pass >= STOP_AT_PASS_MINIMUM && continue_autorouting
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StagnationStep {
    ScoreImproved,
    BoardRouted,
    Accumulate,
}

pub fn stagnation_step(
    board_score_after: f32,
    last_best_score: f32,
    incomplete_count: usize,
) -> StagnationStep {
    if board_score_after > last_best_score + STAGNATION_SCORE_THRESHOLD {
        StagnationStep::ScoreImproved
    } else if incomplete_count == 0 && board_score_after > STAGNATION_SCORE_THRESHOLD {
        StagnationStep::BoardRouted
    } else {
        StagnationStep::Accumulate
    }
}

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

pub fn final_best_board_swap(
    board: &mut Board,
    bh: &mut BoardHistory,
    scoring: &ScoringSettings,
) -> bool {
    let current_final_penalty = BoardStatistics::new(board).routing_penalty(scoring);
    if bh.best_penalty() < current_final_penalty {
        if let Some(best_board) = bh.restore_best_board() {
            *board = best_board;
            return true;
        }
    }
    false
}

pub(crate) fn stat(value: Option<i32>) -> usize {
    usize::try_from(
        value.expect("BoardStatistics' computing constructor fills every count Java unboxes"),
    )
    .unwrap_or(0)
}
