use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use std::time::Instant;

use fr_board::StopConnectionOption;
use fr_board::datastructures::StopCheck;
use fr_board::items::Item;
use fr_board::{Board, BoardError, ItemId};
use fr_drc::DesignRulesChecker;
use fr_geometry::Point;
use fr_settings::{ExpansionCostFactor, RouterSettings};

use crate::autoroute::attempt::{AutorouteAttemptResult, AutorouteAttemptState};
use crate::autoroute::maze::ViaPricing;
use crate::autoroute::maze::engine::{AutorouteEngine, route_connection_full};
use crate::board_ext::RoutingBoardExt;
use crate::error::RouterError;
use crate::pipeline::airline::{ItemDistanceCache, calculate_item_distance_cached};
use crate::pipeline::board_history::BoardHistory;
use crate::pipeline::failure_log::RoutingFailureLog;
use crate::pipeline::pass_runner::AutoroutePassRunner;
use crate::pipeline::stop::{DeterministicWorkBudget, ProgressThrottler, RouterBudget};
use crate::pipeline::{NamedAlgorithmType, ProgressSink, RouterStop};
use crate::score::BoardStatistics;

#[derive(Debug)]
pub struct BatchAutorouter<'a> {
    settings: &'a RouterSettings,

    remove_unconnected_vias: bool,
    trace_costs: Vec<ExpansionCostFactor>,
    retain_autoroute_database: bool,
    start_ripup_costs: i32,
    trace_pull_tight_accuracy: i32,

    pub total_items_routed: i32,
    pub fanout_timed_out: bool,
    pub initial_unrouted_count: i32,
    pub session_start_time: Option<Instant>,
    pub is_optimizer_autorouter: bool,

    board_update_gate: ProgressThrottler,
    pub progress_statistics: Option<BoardStatistics>,
    pub progress_items_since_statistics: i32,

    budget: RouterBudget,
    optimizer_work_budget: Option<Rc<DeterministicWorkBudget>>,
}

impl<'a> BatchAutorouter<'a> {
    pub const BOARD_RANK_LIMIT: usize = BoardHistory::MAX_HISTORY_SIZE;
    pub const MAXIMUM_TRIES_ON_THE_SAME_BOARD: i32 = 3;
    pub const TIME_LIMIT_TO_PREVENT_ENDLESS_LOOP: i32 = 1000;
    pub const STOP_AT_PASS_MINIMUM: i32 = 8;
    pub const STOP_AT_PASS_MODULO: i32 = 4;
    pub const STAGNATION_PASS_LIMIT: i32 = 10;
    pub const FANOUT_RECOVERY_STAGNATION_PASSES: i32 = 3;
    pub const PROGRESS_STATISTICS_ITEM_INTERVAL: i32 = 10;
    pub const STAGNATION_SCORE_THRESHOLD: f32 = 0.5;

    pub const BENCHMARK_PROFILE_ENABLED: bool = false;

    pub const BENCHMARK_RETAIN_AUTOROUTE_DATABASE: bool = false;

    pub const ID: &'static str = "freerouting-router";
    pub const NAME: &'static str = "Freerouting Auto-router";
    pub const VERSION: &'static str = "1.0";
    pub const DESCRIPTION: &'static str = "Freerouting Auto-router v1.0";
    pub const TYPE: NamedAlgorithmType = NamedAlgorithmType::Router;

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        board: &Board,
        settings: &'a RouterSettings,
        remove_unconnected_vias: bool,
        with_preferred_directions: bool,
        start_ripup_costs: i32,
        pull_tight_accuracy: i32,
        budget: RouterBudget,
    ) -> BatchAutorouter<'a> {
        let trace_costs = if with_preferred_directions {
            settings.get_trace_costs()
        } else {
            (0..board.get_layer_count())
                .map(|i| {
                    let current_min_cost = settings.get_preferred_direction_trace_costs(i);
                    ExpansionCostFactor {
                        horizontal: current_min_cost,
                        vertical: current_min_cost,
                    }
                })
                .collect()
        };

        BatchAutorouter {
            settings,
            remove_unconnected_vias,
            trace_costs,
            retain_autoroute_database: BatchAutorouter::BENCHMARK_RETAIN_AUTOROUTE_DATABASE,
            start_ripup_costs,
            trace_pull_tight_accuracy: pull_tight_accuracy,
            total_items_routed: 0,
            fanout_timed_out: false,
            initial_unrouted_count: 0,
            session_start_time: None,
            is_optimizer_autorouter: false,
            board_update_gate: ProgressThrottler::board_update_gate(
                budget.board_update_throttle_ms,
            ),
            progress_statistics: None,
            progress_items_since_statistics: 0,
            budget,
            optimizer_work_budget: None,
        }
    }

    pub fn for_routing_job(
        board: &Board,
        settings: &'a RouterSettings,
        budget: RouterBudget,
    ) -> BatchAutorouter<'a> {
        BatchAutorouter::new(
            board,
            settings,
            !settings.is_fanout_enabled(),
            true,
            settings.get_start_ripup_costs(),
            settings.trace_pull_tight_accuracy.unwrap_or(500),
            budget,
        )
    }

    pub fn is_benchmark_profile_enabled() -> bool {
        BatchAutorouter::BENCHMARK_PROFILE_ENABLED
    }

    pub fn is_remove_unconnected_vias(&self) -> bool {
        self.remove_unconnected_vias
    }

    pub fn get_trace_costs(&self) -> &[ExpansionCostFactor] {
        &self.trace_costs
    }

    pub fn is_retain_autoroute_database(&self) -> bool {
        self.retain_autoroute_database
    }

    pub fn get_start_ripup_costs(&self) -> i32 {
        self.start_ripup_costs
    }

    pub fn get_trace_pull_tight_accuracy(&self) -> i32 {
        self.trace_pull_tight_accuracy
    }

    pub fn is_fanout_timed_out(&self) -> bool {
        self.fanout_timed_out
    }

    pub fn get_initial_unrouted_count(&self) -> i32 {
        self.initial_unrouted_count
    }

    pub fn get_session_start_time(&self) -> Option<Instant> {
        self.session_start_time
    }

    pub fn settings(&self) -> &'a RouterSettings {
        self.settings
    }

    pub fn budget(&self) -> RouterBudget {
        self.budget
    }

    pub fn optimizer_work_budget(&self) -> Option<Rc<DeterministicWorkBudget>> {
        self.optimizer_work_budget.clone()
    }

    pub fn impacted_points(board: &Board, item: ItemId) -> Vec<Point> {
        let ctx = board.ctx();
        match board.get_item(item) {
            Some(Item::Trace(trace)) => {
                let mut result = Vec::new();
                if let Some(first) = trace.first_corner() {
                    result.push(first);
                }
                if let Some(last) = trace.last_corner() {
                    result.push(last);
                }
                result
            }
            Some(Item::Via(via)) => vec![via.get_center()],
            Some(Item::Pin(pin)) => vec![pin.get_center(&ctx)],
            _ => Vec::new(),
        }
    }

    pub fn enforce_strict_drc(
        board: &mut Board,
        route_net_no: i32,
        max_item_id_before: ItemId,
    ) -> Option<AutorouteAttemptResult> {
        let mut new_items: Vec<ItemId> = Vec::new();
        let mut has_violation = false;
        for current_item in board.get_connectable_items(route_net_no) {
            if current_item <= max_item_id_before
                || !matches!(
                    board.get_item(current_item),
                    Some(Item::Trace(_) | Item::Via(_))
                )
            {
                continue;
            }
            new_items.push(current_item);
            if !has_violation && !board.clearance_violations(current_item).is_empty() {
                has_violation = true;
            }
        }
        if !has_violation {
            return None;
        }
        let removed = new_items.len();
        board.remove_items(new_items);
        Some(AutorouteAttemptResult::with_details(
            AutorouteAttemptState::Failed,
            format!(
                "strict_drc: connection ripped because {removed} new item(s) included clearance \
                 violations"
            ),
        ))
    }

    pub fn should_fire_board_update(&self) -> bool {
        self.board_update_gate.should_update()
    }

    pub fn should_fire_board_update_at(&self, now: Instant) -> bool {
        self.board_update_gate.should_update_at(now)
    }

    pub fn remove_tails(
        &self,
        board: &mut Board,
        engine: Option<&mut AutorouteEngine>,
        stop_connection_option: StopConnectionOption,
        stop: StopCheck<'_>,
    ) -> Result<(), BoardError> {
        board.start_marking_changed_area();
        board.remove_trace_tails(-1, stop_connection_option)?;
        board.opt_changed_area(
            engine,
            &[],
            None,
            self.trace_pull_tight_accuracy,
            Some(&self.trace_costs),
            stop,
            self.budget.opt_changed_area_ms,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn autoroute_item(
        &self,
        board: &mut Board,
        engine: &mut Option<AutorouteEngine>,
        item: ItemId,
        route_net_no: i32,
        ripped_item_list: &mut BTreeSet<ItemId>,
        ripup_costs: &mut BTreeMap<ItemId, i32>,
        ripup_pass_no: i32,
        stop: StopCheck<'_>,
    ) -> AutorouteAttemptResult {
        let via_pricing = if self.is_optimizer_autorouter {
            ViaPricing::PerMillimetre
        } else {
            ViaPricing::ByPadstackRadius
        };
        route_connection_full(
            board,
            engine,
            item,
            route_net_no,
            self.settings,
            &self.trace_costs,
            via_pricing,
            ripped_item_list,
            ripup_costs,
            ripup_pass_no,
            self.start_ripup_costs,
            self.remove_unconnected_vias,
            self.trace_pull_tight_accuracy,
            self.budget,
            stop,
        )
    }

    pub fn autoroute_items(&self, board: &Board) -> Vec<(ItemId, i32)> {
        self.autoroute_items_with_handled(board).0
    }

    pub fn autoroute_items_with_handled(
        &self,
        board: &Board,
    ) -> (Vec<(ItemId, i32)>, BTreeSet<ItemId>) {
        let mut autoroute_item_list: Vec<(ItemId, i32)> = Vec::new();
        let mut handled_items: BTreeSet<ItemId> = BTreeSet::new();

        for current_item in board.items_in_board_order() {
            let Some(item) = board.get_item(current_item) else {
                continue;
            };
            if item.as_connectable().is_none() {
                continue;
            }
            if item.is_routable() || handled_items.contains(&current_item) {
                continue;
            }

            for i in 0..item.net_count() {
                let current_net_number = item.get_net_number(i);
                let connected_set = board.connected_set(current_item, current_net_number, false);
                for connected in &connected_set {
                    if board
                        .get_item(*connected)
                        .is_some_and(|c| c.net_count() <= 1)
                    {
                        handled_items.insert(*connected);
                    }
                }
                let net_item_count = board.connectable_item_count(current_net_number);

                if connected_set.len() >= net_item_count || board.has_ignored_nets(current_item) {
                    continue;
                }

                let net = board.rules.nets.get(current_net_number);
                if net.is_some_and(|net| net.contains_plane()) {
                    let already_connected_to_plane = connected_set
                        .iter()
                        .any(|id| matches!(board.get_item(*id), Some(Item::ConductionArea(_))));
                    if already_connected_to_plane {
                        continue;
                    }
                }

                autoroute_item_list.push((current_item, current_net_number));
            }
        }

        let mut cache = ItemDistanceCache::default();
        let mut keyed: Vec<(f64, (ItemId, i32))> = autoroute_item_list
            .iter()
            .map(|entry| {
                (
                    calculate_item_distance_cached(board, entry.0, &mut cache),
                    *entry,
                )
            })
            .collect();
        keyed.sort_by(|a, b| a.0.total_cmp(&b.0));
        let autoroute_item_list: Vec<(ItemId, i32)> =
            keyed.into_iter().map(|(_, entry)| entry).collect();

        (autoroute_item_list, handled_items)
    }

    pub fn autoroute_pass(
        &mut self,
        board: &mut Board,
        failure_log: &mut RoutingFailureLog,
        pass_no: i32,
        stop: &RouterStop,
        progress: &mut dyn ProgressSink,
    ) -> Result<bool, RouterError> {
        AutoroutePassRunner::run_single_thread(board, self, failure_log, pass_no, stop, progress)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn autoroute_passes_for_optimizing_item(
        board: &mut Board,
        settings: &RouterSettings,
        max_pass_count: i32,
        ripup_costs: i32,
        trace_pull_tight_accuracy: i32,
        with_preferred_directions: bool,
        stop: &RouterStop,
        budget: RouterBudget,
        progress: &mut dyn ProgressSink,
        optimizer_work_budget: Option<Rc<DeterministicWorkBudget>>,
    ) -> Result<i32, RouterError> {
        let mut router_instance = BatchAutorouter::new(
            board,
            settings,
            true,
            with_preferred_directions,
            ripup_costs,
            trace_pull_tight_accuracy,
            budget,
        );
        router_instance.is_optimizer_autorouter = true;
        router_instance.optimizer_work_budget = optimizer_work_budget.clone();

        let mut still_unrouted_items = true;
        let mut current_pass_no: i32 = 1;
        let mut failure_log = RoutingFailureLog::new();

        while still_unrouted_items
            && !stop.is_stop_auto_router_requested()
            && current_pass_no <= max_pass_count
            && !optimizer_work_budget
                .as_ref()
                .is_some_and(|work| work.exhausted())
        {
            still_unrouted_items = router_instance.autoroute_pass(
                board,
                &mut failure_log,
                current_pass_no,
                stop,
                progress,
            )?;
            current_pass_no += 1;
        }

        router_instance.remove_tails(board, None, StopConnectionOption::None, &|| {
            stop.is_stop_requested()
        })?;
        if !still_unrouted_items {
            current_pass_no -= 1;
        }
        Ok(current_pass_no)
    }

    pub fn calculate_incomplete_count(board: &mut Board) -> usize {
        let mut temp_drc = DesignRulesChecker::new(board);
        temp_drc.calculate_all_incompletes();
        temp_drc.get_incomplete_count()
    }
}

pub type BatchAutorouterStop = RouterStop;
