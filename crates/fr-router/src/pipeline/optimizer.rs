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

pub const PORT_OPTIMIZER_ROUTE_WORK_BUDGET: i64 = 1800;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReadSortedRouteItems {
    pub min_item_coor: FloatPoint,
    pub min_item_layer: i32,
}

impl Default for ReadSortedRouteItems {
    fn default() -> ReadSortedRouteItems {
        ReadSortedRouteItems::new()
    }
}

impl ReadSortedRouteItems {
    #[must_use]
    pub fn new() -> ReadSortedRouteItems {
        ReadSortedRouteItems {
            min_item_coor: FloatPoint {
                x: f64::from(i32::MIN),
                y: f64::from(i32::MIN),
            },
            min_item_layer: -1,
        }
    }

    #[must_use]
    pub fn next(&mut self, board: &Board) -> Option<ItemId> {
        let mut result: Option<ItemId> = None;
        let mut current_min_coor = FloatPoint {
            x: f64::from(i32::MAX),
            y: f64::from(i32::MAX),
        };
        let mut current_min_layer: i32 = i32::MAX;

        let ctx = board.ctx();

        for id in board.items_in_board_order() {
            let Some(item) = board.get_item(id) else {
                continue;
            };
            let Item::Via(current_via) = item else {
                continue;
            };
            if item.is_user_fixed() {
                continue;
            }
            let current_via_center = current_via.get_center().to_float();
            let current_via_min_layer = layer_index(current_via.first_layer(&ctx));
            if current_via_center.x > self.min_item_coor.x
                || current_via_center.x == self.min_item_coor.x
                    && (current_via_center.y > self.min_item_coor.y
                        || current_via_center.y == self.min_item_coor.y
                            && current_via_min_layer > self.min_item_layer)
            {
                if current_via_center.x < current_min_coor.x
                    || current_via_center.x == current_min_coor.x
                        && (current_via_center.y < current_min_coor.y
                            || current_via_center.y == current_min_coor.y
                                && current_via_min_layer < current_min_layer)
                {
                    current_min_coor = current_via_center;
                    current_min_layer = current_via_min_layer;
                    result = Some(id);
                }
            }
        }

        for id in board.items_in_board_order() {
            let Some(item) = board.get_item(id) else {
                continue;
            };
            let Item::Trace(current_trace) = item else {
                continue;
            };
            if item.is_shove_fixed(&board.rules) {
                continue;
            }
            let first_corner = current_trace
                .first_corner()
                .expect("a board trace has at least two corners (BasicBoard.java:185-187)")
                .to_float();
            let last_corner = current_trace
                .last_corner()
                .expect("a board trace has at least two corners (BasicBoard.java:185-187)")
                .to_float();
            let compare_corner = if first_corner.x < last_corner.x
                || first_corner.x == last_corner.x && first_corner.y < last_corner.y
            {
                last_corner
            } else {
                first_corner
            };
            let current_trace_layer = layer_index(current_trace.get_layer());
            if compare_corner.x > self.min_item_coor.x
                || compare_corner.x == self.min_item_coor.x
                    && (compare_corner.y > self.min_item_coor.y
                        || compare_corner.y == self.min_item_coor.y
                            && current_trace_layer > self.min_item_layer)
            {
                if compare_corner.x < current_min_coor.x
                    || compare_corner.x == current_min_coor.x
                        && (compare_corner.y < current_min_coor.y
                            || compare_corner.y == current_min_coor.y
                                && current_trace_layer < current_min_layer)
                {
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
                    if !is_connected_to_via {
                        current_min_coor = compare_corner;
                        current_min_layer = current_trace_layer;
                        result = Some(id);
                    }
                }
            }
        }

        self.min_item_coor = current_min_coor;
        self.min_item_layer = current_min_layer;
        result
    }

    #[must_use]
    pub fn get_current_position(&self) -> FloatPoint {
        self.min_item_coor
    }
}

fn layer_index(layer: usize) -> i32 {
    i32::try_from(layer).expect("a board layer index fits in an i32")
}

#[derive(Debug)]
pub struct BatchOptimizer<'a> {
    pub settings: &'a RouterSettings,

    pub progress_throttler: ProgressThrottler,
    pub sorted_route_items: Option<ReadSortedRouteItems>,
    pub use_increased_ripup_costs: bool,
    pub min_cumulative_trace_length: f64,
    pub total_items_optimized: i32,
    /// quantity [`PORT_OPTIMIZER_ROUTE_WORK_BUDGET`] bounds. A complete-board item adds 0, so
    pub total_route_work: i64,
    pub deadline: Option<std::time::Instant>,
    pub is_timed_out: bool,
}

impl<'a> BatchOptimizer<'a> {
    #[must_use]
    pub fn new(settings: &'a RouterSettings) -> BatchOptimizer<'a> {
        BatchOptimizer {
            settings,
            progress_throttler: ProgressThrottler::new(1000),
            sorted_route_items: None,
            use_increased_ripup_costs: false,
            min_cumulative_trace_length: 0.0,
            total_items_optimized: 0,
            total_route_work: 0,
            deadline: None,
            is_timed_out: false,
        }
    }

    #[must_use]
    pub fn is_timed_out(&self) -> bool {
        self.is_timed_out
    }

    #[must_use]
    pub fn contains_only_unfixed_traces(board: &Board, item_list: &BTreeSet<ItemId>) -> bool {
        for current_item in item_list.iter().rev() {
            let Some(item) = board.get_item(*current_item) else {
                continue;
            };
            if item.is_user_fixed() || !matches!(item, Item::Trace(_)) {
                return false;
            }
        }
        true
    }

    #[must_use]
    pub fn calculate_incomplete_count(board: &mut Board) -> usize {
        let mut temp_drc = DesignRulesChecker::new(board);
        temp_drc.calculate_all_incompletes();
        temp_drc.get_incomplete_count()
    }

    #[must_use]
    pub fn get_current_position(&self) -> Option<FloatPoint> {
        self.sorted_route_items
            .as_ref()
            .map(ReadSortedRouteItems::get_current_position)
    }

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
        let board_statistics_before = BoardStatistics::with_options(board, None, false);
        let incomplete_count_before =
            count_as_i32(BatchOptimizer::calculate_incomplete_count(board));
        if self.progress_throttler.should_update() {
            progress.on_event(&RoutingEvent::BoardUpdated {
                counters: RouterCounters {
                    incomplete_count: Some(incomplete_count_before),
                    ..RouterCounters::default()
                },
            });
        }

        let mut ripped_items: BTreeSet<ItemId> = BTreeSet::new();
        ripped_items.insert(item);

        let item_net_numbers: Vec<i32> = board.get_item(item).map_or_else(Vec::new, |it| {
            (0..it.net_count()).map(|i| it.get_net_number(i)).collect()
        });
        let item_is_trace = matches!(board.get_item(item), Some(Item::Trace(_)));

        if item_is_trace {
            let mut current_contact_list = board.trace_start_contacts(item);
            for _ in 0..2 {
                if BatchOptimizer::contains_only_unfixed_traces(board, &current_contact_list) {
                    ripped_items.extend(current_contact_list.iter().copied());
                }
                current_contact_list = board.trace_end_contacts(item);
            }
        }

        let mut ripped_connections: BTreeSet<ItemId> = BTreeSet::new();
        for current_item in ripped_items.iter().rev() {
            ripped_connections
                .extend(board.connection_items(*current_item, StopConnectionOption::None));
        }

        let snapshot = if disable_snapshots {
            None
        } else {
            let snapshot = board.deep_copy();
            board.begin_undo_journal();
            Some(snapshot)
        };

        let removal_order: Vec<ItemId> = ripped_connections.iter().rev().copied().collect();
        board.remove_items(removal_order);
        for net_number in &item_net_numbers {
            board.combine_traces(*net_number)?;
        }

        let ripup_costs =
            optimizer_ripup_costs(self.settings, self.use_increased_ripup_costs, item_is_trace);

        let optimizer = self.settings.optimizer.as_ref().expect(
            "BatchOptimizer.optRouteItem: settings.optimizer is dereferenced at :456 without a \
             null check — Java throws a NullPointerException here too",
        );
        // spends — [`PORT_OPTIMIZER_ROUTE_WORK_BUDGET`], accumulated just below as
        let max_autoroute_passes = optimizer
            .max_autoroute_passes
            .expect("optimizer.maxAutoroutePasses is unboxed at :468");
        let trace_pull_tight_accuracy = self.settings.trace_pull_tight_accuracy.expect(
            "BatchOptimizer.optRouteItem: settings.tracePullTightAccuracy is unboxed at :470 \
             with no null fallback — Java throws a NullPointerException here too",
        );
        let passes_run = BatchAutorouter::autoroute_passes_for_optimizing_item(
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
        // [`PORT_OPTIMIZER_ROUTE_WORK_BUDGET`].
        self.total_route_work = self.total_route_work.saturating_add(
            i64::from(incomplete_count_before.max(0)) * i64::from(passes_run.max(0)),
        );

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

        let mut result = ItemRouteResult::new(
            item,
            board_statistics_before.items.via_count.unwrap_or(0),
            board_statistics_after.items.via_count.unwrap_or(0),
            self.min_cumulative_trace_length,
            f64::from(board_statistics_after.traces.total_length.unwrap_or(0.0)),
            incomplete_count_before,
            incomplete_count_after,
        );
        let route_improved = !stop.is_stop_requested() && result.improved();
        result.update_improved(route_improved);

        if route_improved {
            self.min_cumulative_trace_length = java_min(
                self.min_cumulative_trace_length,
                f64::from(
                    board_statistics_after
                        .traces
                        .total_weighted_length
                        .unwrap_or(0.0),
                ),
            );
            board.discard_undo_journal();
            drop(snapshot);
        } else if let Some(restored) = snapshot {
            board.undo_from_snapshot(restored);
        }

        Ok(result)
    }
}

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
    let mut ripup_costs = settings.get_start_ripup_costs();
    if use_increased_ripup_costs {
        ripup_costs = ripup_costs.wrapping_mul(
            optimizer
                .additional_ripup_cost_factor_at_start
                .expect("optimizer.additionalRipupCostFactorAtStart is unboxed at :456"),
        );
    }
    if item_is_trace {
        let trace_ripup_cost_factor = optimizer
            .trace_ripup_cost_factor
            .expect("optimizer.traceRipupCostFactor is unboxed at :462");
        ripup_costs =
            java_round(f64::from(trace_ripup_cost_factor) * f64::from(ripup_costs)) as i32;
    }
    ripup_costs
}

fn count_as_i32(value: usize) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

#[derive(Debug, Clone, PartialEq)]
pub struct OptimizerResult {
    pub state: TaskState,
    pub passes_run: i32,
    pub last_reported_pass: i32,
    pub items_optimized: i32,
    pub timed_out: bool,
    pub per_pass: Vec<OptimizerPassRecord>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OptimizerPassRecord {
    pub pass: i32,
    pub with_preferred_directions: bool,
    pub score_before: f32,
    pub score_after: f32,
    pub pass_improvement: f64,
    pub force_another_pass: bool,
    pub use_increased_ripup_costs: bool,
    pub route_improved: f32,
    pub total_items_optimized: i32,
    pub record: PassRecord,
}

#[must_use]
pub fn optimizer_near_perfect_exit(score_before_pass: f32, improvement_threshold: f32) -> bool {
    score_before_pass * (1.0 + improvement_threshold) >= 1000.0
}

#[must_use]
pub fn optimizer_route_improved(
    result: &ItemRouteResult,
    before_via_count: i32,
    before_total_length: f32,
) -> f32 {
    if before_via_count != 0 && before_total_length != 0.0 {
        let via_term = f64::from(result.via_count() as f32 / before_via_count as f32);
        let length_term = result.trace_length() / f64::from(before_total_length);
        (1.0 - ((via_term + length_term) / 2.0)) as f32
    } else {
        0.0
    }
}

impl BatchOptimizer<'_> {
    pub const ID: &'static str = "freerouting-optimizer";
    pub const NAME: &'static str = "Freerouting Optimizer";
    pub const VERSION: &'static str = "1.0";
    pub const DESCRIPTION: &'static str = "Freerouting Optimizer v1.0";
    pub const TYPE: NamedAlgorithmType = NamedAlgorithmType::Optimizer;

    #[must_use]
    pub fn normalize_algorithm(algorithm: &str) -> String {
        if BatchOptimizer::ID != algorithm {
            return BatchOptimizer::ID.to_string();
        }
        algorithm.to_string()
    }

    /// budget? See [`PORT_OPTIMIZER_ROUTE_WORK_BUDGET`] for the full argument: it bounds the #227
    #[must_use]
    pub fn route_work_budget_spent(&self) -> bool {
        self.total_route_work >= PORT_OPTIMIZER_ROUTE_WORK_BUDGET
    }

    #[must_use]
    pub fn is_deadline_reached(&self) -> bool {
        self.deadline
            .is_some_and(|deadline| std::time::Instant::now() >= deadline)
    }

    pub fn run_batch_loop(
        &mut self,
        board: &mut Board,
        stop: &RouterStop,
        budget: RouterBudget,
        progress: &mut dyn ProgressSink,
    ) -> Result<OptimizerResult, RouterError> {
        let settings = self.settings;
        let optimizer = settings.optimizer.as_ref().expect(
            "BatchOptimizer.runBatchLoop: settings.optimizer is dereferenced at :167 without a \
             null check — Java throws a NullPointerException here too",
        );
        let scoring = settings
            .scoring
            .as_ref()
            .expect("RouterSettings.scoring — BatchOptimizer.java:179 dereferences it");
        let improvement_threshold = optimizer.optimization_improvement_threshold.expect(
            "optimizer.optimizationImprovementThreshold is unboxed at :182 and :221 with no null \
             fallback — Java throws a NullPointerException here too",
        );

        self.progress_throttler = budget.progress_throttler();

        self.use_increased_ripup_costs = true;

        let _initial_stats = BoardStatistics::new(board);

        let session_start = std::time::Instant::now();
        if let Some(timeout_string) = optimizer.timeout_string.as_deref() {
            if let Some(timeout_seconds) =
                parse_timespan_seconds(timeout_string).map_err(RouterError::Timespan)?
            {
                self.deadline =
                    instant_offset_ms(session_start, timeout_seconds.saturating_mul(1000));
            }
        }

        progress.on_event(&RoutingEvent::TaskStateChanged {
            algorithm: BatchOptimizer::TYPE,
            state: TaskState::Started,
        });

        let mut current_pass: i32 = 0;
        let mut last_reported_pass: i32 = 0;
        let mut per_pass: Vec<OptimizerPassRecord> = Vec::new();

        while optimizer
            .max_passes
            .is_none_or(|max_passes| current_pass < max_passes)
            && optimizer
                .max_items
                .is_none_or(|max_items| self.total_items_optimized < max_items)
            && !self.route_work_budget_spent()
            && !stop.is_stop_requested()
        {
            stop.poll_cancel();
            if self.is_deadline_reached() {
                self.is_timed_out = true;
                break;
            }
            current_pass += 1;

            let score_before_pass = BoardStatistics::new(board).normalized_score(scoring);

            if optimizer_near_perfect_exit(score_before_pass, improvement_threshold) {
                break;
            }

            last_reported_pass = current_pass;
            progress.on_event(&RoutingEvent::TaskStateChanged {
                algorithm: BatchOptimizer::TYPE,
                state: TaskState::Running,
            });

            let with_preferred_directions = current_pass % 2 != 0;
            let route_improved = self.opt_route_pass(
                board,
                current_pass,
                with_preferred_directions,
                stop,
                budget,
                progress,
            )?;

            if self.is_timed_out {
                break;
            }

            let statistics_after = BoardStatistics::new(board);
            let score_after_pass = statistics_after.normalized_score(scoring);
            let (pass_improvement, force_another_pass) =
                self.apply_pass_improvement(score_before_pass, score_after_pass);

            per_pass.push(OptimizerPassRecord {
                pass: current_pass,
                with_preferred_directions,
                score_before: score_before_pass,
                score_after: score_after_pass,
                pass_improvement,
                force_another_pass,
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

            if !force_another_pass && pass_improvement < f64::from(improvement_threshold) {
                break;
            }
        }

        progress.on_event(&RoutingEvent::TaskStateChanged {
            algorithm: BatchOptimizer::TYPE,
            state: TaskState::Finished,
        });

        let _final_stats = BoardStatistics::new(board);

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
            last_reported_pass,
            items_optimized: self.total_items_optimized,
            timed_out: self.is_timed_out,
            per_pass,
        })
    }

    pub fn apply_pass_improvement(&mut self, score_before: f32, score_after: f32) -> (f64, bool) {
        let pass_improvement = if score_before > 0.0 {
            f64::from(score_after - score_before) / f64::from(score_before)
        } else {
            0.0
        };
        let force_another_pass = if self.use_increased_ripup_costs && score_after <= score_before {
            self.use_increased_ripup_costs = false;
            true
        } else {
            false
        };
        (pass_improvement, force_another_pass)
    }

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

        let board_statistics_before = BoardStatistics::new(board);
        let router_counters = RouterCounters {
            pass_count: Some(pass_no),
            ..RouterCounters::default()
        };
        self.progress_throttler.reset();
        progress.on_event(&RoutingEvent::BoardUpdated {
            counters: router_counters.clone(),
        });

        self.sorted_route_items = Some(ReadSortedRouteItems::new());
        self.min_cumulative_trace_length = f64::from(
            board_statistics_before
                .traces
                .total_weighted_length
                .unwrap_or(0.0),
        );

        let mut consecutive_failures: i32 = 0;
        let max_consecutive_failures = optimizer.max_consecutive_failures.unwrap_or(50);

        let mut route_improved: f32 = 0.0;
        loop {
            if self.is_deadline_reached() {
                self.is_timed_out = true;
                return Ok(route_improved);
            }
            if stop.is_stop_requested() {
                return Ok(route_improved);
            }
            if optimizer
                .max_items
                .is_some_and(|max_items| max_items > 0 && self.total_items_optimized >= max_items)
            {
                break;
            }
            if self.route_work_budget_spent() {
                break;
            }
            let Some(current_item) = self
                .sorted_route_items
                .as_mut()
                .expect("optRoutePass:287 assigned it one line ago")
                .next(board)
            else {
                break;
            };
            let result = self.opt_route_item(
                board,
                current_item,
                with_preferred_directions,
                false,
                stop,
                budget,
                progress,
            )?;
            self.total_items_optimized += 1;
            if result.improved() {
                consecutive_failures = 0;
                if self.progress_throttler.should_update() {
                    let _board_statistics_after = BoardStatistics::new(board);
                    progress.on_event(&RoutingEvent::BoardUpdated {
                        counters: router_counters.clone(),
                    });
                }
                route_improved = optimizer_route_improved(
                    &result,
                    board_statistics_before.items.via_count.unwrap_or(0),
                    board_statistics_before.traces.total_length.unwrap_or(0.0),
                );
            } else {
                consecutive_failures += 1;
                if consecutive_failures >= max_consecutive_failures {
                    break;
                }
            }
        }

        self.sorted_route_items = None;
        if self.use_increased_ripup_costs && route_improved == 0.0 {
            self.use_increased_ripup_costs = false;
            route_improved = -1.0;
        }

        let _board_statistics_after = BoardStatistics::new(board);
        progress.on_event(&RoutingEvent::BoardUpdated {
            counters: router_counters,
        });

        Ok(route_improved)
    }
}
