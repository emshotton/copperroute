use std::collections::{BTreeMap, BTreeSet};
use std::panic::AssertUnwindSafe;

use fr_board::{Board, ItemId, StopConnectionOption};

use crate::autoroute::attempt::AutorouteAttemptState;
use crate::error::RouterError;
use crate::pipeline::batch_autorouter::BatchAutorouter;
use crate::pipeline::connection_budget::ConnectionBudget;
use crate::pipeline::failure_log::RoutingFailureLog;
use crate::pipeline::{ProgressSink, RouterCounters, RouterStop, RoutingEvent};

#[derive(Debug, Clone, Copy, Default)]
pub struct AutoroutePassRunner;

impl AutoroutePassRunner {
    #[allow(clippy::too_many_arguments)]
    pub fn run_single_thread(
        board: &mut Board,
        router: &mut BatchAutorouter<'_>,
        failure_log: &mut RoutingFailureLog,
        pass_no: i32,
        stop: &RouterStop,
        progress: &mut dyn ProgressSink,
    ) -> Result<bool, RouterError> {
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
            Ok(Ok(any_progress)) => Ok(any_progress),
            Ok(Err(_)) | Err(_) => Ok(false),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn run_single_thread_body(
        board: &mut Board,
        router: &mut BatchAutorouter<'_>,
        failure_log: &mut RoutingFailureLog,
        pass_no: i32,
        stop: &RouterStop,
        progress: &mut dyn ProgressSink,
    ) -> Result<bool, RouterError> {
        stop.poll_cancel();
        stop.poll_deadline();
        let autoroute_item_list = router.autoroute_items(board);

        if autoroute_item_list.is_empty() {
            return Ok(false);
        }

        let mut items_to_go_count = i32::try_from(autoroute_item_list.len()).unwrap_or(i32::MAX);
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

        counters.incomplete_count =
            AutoroutePassRunner::incomplete_count_for_progress(board, router);

        progress.on_event(&RoutingEvent::BoardUpdated {
            counters: counters.clone(),
        });

        let mut ripped_item_count: i32 = 0;
        let mut not_routed: i32 = 0;
        let mut routed: i32 = 0;
        let mut skipped: i32 = 0;

        for (current_item, route_net_no) in autoroute_item_list {
            stop.poll_cancel();
            stop.poll_deadline();
            if stop.is_stop_auto_router_requested() {
                break;
            }

            if board.get_item(current_item).is_some() {
                if stop.is_stop_auto_router_requested() {
                    break;
                }

                if let Some(threshold) = router.settings().get_failure_give_up_threshold()
                    && failure_log.should_skip(current_item, threshold)
                {
                    skipped += 1;
                    items_to_go_count -= 1;
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
                    continue;
                }

                let max_items = router.settings().max_items;
                if max_items.is_some_and(|max| max > 0 && router.total_items_routed >= max) {
                    stop.request_stop_auto_router();
                    break;
                }
                router.total_items_routed += 1;
                board.start_marking_changed_area();

                let mut ripped_item_list: BTreeSet<ItemId> = BTreeSet::new();
                let mut ripped_item_costs: BTreeMap<ItemId, i32> = BTreeMap::new();

                let mut engine = None;
                let optimizer_work_budget = router.optimizer_work_budget();
                let connection_budget = if router.is_optimizer_autorouter {
                    None
                } else {
                    Some(ConnectionBudget::start(router.settings()))
                };
                let stop_check = &|| {
                    stop.is_stopped_or_expired()
                        || optimizer_work_budget
                            .as_ref()
                            .is_some_and(|work| work.poll())
                        || connection_budget
                            .as_ref()
                            .is_some_and(ConnectionBudget::exceeded)
                };
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

                match autorouter_result.state {
                    AutorouteAttemptState::Routed => routed += 1,
                    AutorouteAttemptState::AlreadyConnected
                    | AutorouteAttemptState::NoUnconnectedNets
                    | AutorouteAttemptState::ConnectedToPlane => skipped += 1,
                    _ => {
                        failure_log.record_failure(
                            board,
                            current_item,
                            pass_no,
                            autorouter_result.state,
                            autorouter_result.details.as_deref(),
                        );
                        let _failure_count = failure_log.failure_count(current_item);
                        not_routed += 1;
                    }
                }

                items_to_go_count -= 1;
                ripped_item_count += i32::try_from(ripped_item_list.len()).unwrap_or(i32::MAX);
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

        if ripped_item_count > 0 {
            router.net_filter = None;
        }

        let tail_stop = &|| stop.is_stopped_or_expired();
        if router.is_remove_unconnected_vias() {
            router.remove_tails(board, None, StopConnectionOption::None, tail_stop)?;
        } else {
            router.remove_tails(board, None, StopConnectionOption::FanoutVia, tail_stop)?;
        }

        counters.pass_count = Some(pass_no);
        counters.queued_to_be_routed_count = Some(items_to_go_count);
        counters.skipped_count = Some(skipped);
        counters.ripped_count = Some(ripped_item_count);
        counters.failed_to_be_routed_count = Some(not_routed);
        counters.routed_count = Some(routed);
        counters.incomplete_count =
            AutoroutePassRunner::incomplete_count_for_progress(board, router);
        progress.on_event(&RoutingEvent::BoardUpdated {
            counters: counters.clone(),
        });

        Ok(routed > 0 || not_routed > 0)
    }

    fn incomplete_count_for_progress(
        board: &mut Board,
        router: &BatchAutorouter<'_>,
    ) -> Option<i32> {
        if router.is_optimizer_autorouter {
            return None;
        }
        i32::try_from(BatchAutorouter::calculate_incomplete_count(board)).ok()
    }

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
        if router.should_fire_board_update() {
            counters.queued_to_be_routed_count = Some(items_to_go_count);
            counters.skipped_count = Some(skipped);
            counters.ripped_count = Some(ripped_item_count);
            counters.failed_to_be_routed_count = Some(not_routed);
            counters.routed_count = Some(routed);
            counters.incomplete_count =
                AutoroutePassRunner::incomplete_count_for_progress(board, router);
            progress.on_event(&RoutingEvent::BoardUpdated {
                counters: counters.clone(),
            });
        }
    }
}
