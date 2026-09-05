//! The routing cost: the score's cost term, in `f64`, as the optimizer's and the board history's
//! decision input. The `f32` `normalized_score` stays what the manifest reports.

use fr_board::ItemId;
use fr_board::prelude::*;
use fr_router::pipeline::{
    AutorouteBatchLoop, BatchOptimizer, BoardHistory, ItemRouteResult, NoopProgressSink,
    RouterBudget, RouterStop, optimizer_nothing_to_improve,
};
use fr_router::score::BoardStatistics;
use fr_settings::sources::DefaultSettings;
use fr_settings::{HostEnvironment, RouterSettings, ScoringSettings, SettingsSource};

const RPI: &str = "fixtures/Issue143-rpi_splitter.dsn";

fn default_settings() -> RouterSettings {
    DefaultSettings::new(&HostEnvironment::detect())
        .get_settings()
        .expect("DefaultSettings always answers a table")
        .clone()
}

fn scoring() -> ScoringSettings {
    default_settings()
        .scoring
        .expect("DefaultSettings fills the scoring block")
}

fn stats(
    connections: i32,
    incomplete: i32,
    violations: i32,
    length_mm: f32,
    vias: i32,
    bends: i32,
) -> BoardStatistics {
    let mut stats = BoardStatistics::default();
    stats.connections.maximum_count = Some(connections);
    stats.connections.incomplete_count = Some(incomplete);
    stats.clearance_violations.total_count = Some(violations);
    stats.traces.total_length_mm = Some(length_mm);
    stats.traces.total_length = Some(length_mm);
    stats.vias.total_count = Some(vias);
    stats.bends.total_count = Some(bends);
    stats
}

#[test]
fn the_routing_cost_is_the_scores_cost_term() {
    let scoring = scoring();
    let cost = stats(10, 0, 0, 123.5, 3, 7).routing_cost(&scoring);
    assert_eq!(cost, 123.5 + 3.0 * 50.0 + 7.0 * 10.0);
}

#[test]
fn the_routing_penalty_adds_incompletes_and_violations_at_the_scores_weights() {
    let scoring = scoring();
    let penalty = stats(10, 2, 1, 100.0, 1, 1).routing_penalty(&scoring);
    assert_eq!(
        penalty,
        2.0 * 5_000_000.0 + 1_000_000.0 + 100.0 + 50.0 + 10.0
    );
}

#[test]
fn a_via_the_f32_score_cannot_see_moves_the_penalty() {
    let scoring = scoring();
    let with = stats(1000, 0, 0, 500.0, 40, 0);
    let without = stats(1000, 0, 0, 500.0, 39, 0);
    assert_eq!(
        with.normalized_score(&scoring),
        without.normalized_score(&scoring),
        "one via in 5·10⁹ is below the f32 ulp near 1000"
    );
    assert!(without.routing_penalty(&scoring) < with.routing_penalty(&scoring));
}

#[test]
fn an_item_result_improves_when_its_penalty_falls() {
    let one_via_for_thirty_mm =
        ItemRouteResult::new(ItemId(1), 4, 3, 100.0, 130.0, 0, 0, 330.0, 310.0);
    assert!(one_via_for_thirty_mm.improved());

    let shorter_but_bendier =
        ItemRouteResult::new(ItemId(1), 3, 3, 100.0, 90.0, 0, 0, 480.0, 520.0);
    assert!(!shorter_but_bendier.improved());

    let tied = ItemRouteResult::new(ItemId(1), 3, 3, 100.0, 100.0, 0, 0, 480.0, 480.0);
    assert!(!tied.improved());

    let one_more_incomplete =
        ItemRouteResult::new(ItemId(1), 3, 2, 100.0, 50.0, 0, 1, 480.0, 5_000_380.0);
    assert!(!one_more_incomplete.improved());
}

#[test]
fn only_a_zero_cost_board_has_nothing_to_improve() {
    assert!(optimizer_nothing_to_improve(0.0));
    assert!(!optimizer_nothing_to_improve(0.03));
    assert!(!optimizer_nothing_to_improve(672.4));
}

#[test]
fn the_pass_improvement_is_the_relative_cost_reduction() {
    let settings = default_settings();
    let mut optimizer = BatchOptimizer::new(&settings);
    optimizer.use_increased_ripup_costs = false;

    let (improvement, force) = optimizer.apply_pass_improvement(0, 100.0, 0, 90.0);
    assert!((improvement - 0.1).abs() < 1e-12);
    assert!(!force);

    let (improvement, _) = optimizer.apply_pass_improvement(0, 0.0, 0, 0.0);
    assert_eq!(
        improvement, 0.0,
        "a zero cost before divides to 0, not to NaN"
    );

    let (improvement, _) = optimizer.apply_pass_improvement(2, 100.0, 1, 150.0);
    assert!(
        improvement >= 1.0,
        "a completed connection outranks any cost"
    );
}

#[test]
fn a_non_improving_pass_still_drops_the_increased_ripup_costs_once() {
    let settings = default_settings();
    let mut optimizer = BatchOptimizer::new(&settings);
    optimizer.use_increased_ripup_costs = true;

    let (improvement, force) = optimizer.apply_pass_improvement(0, 100.0, 0, 100.0);
    assert_eq!(improvement, 0.0);
    assert!(force);
    assert!(!optimizer.use_increased_ripup_costs);

    let (_, force) = optimizer.apply_pass_improvement(0, 100.0, 0, 100.0);
    assert!(!force);
}

fn load_rpi() -> Board {
    let path = parity::java_dir().join(RPI);
    let file = std::fs::File::open(&path)
        .unwrap_or_else(|e| panic!("cannot open {}: {e}", path.display()));
    match fr_dsn::read_board(
        file,
        None,
        Some("Issue143-rpi_splitter.dsn"),
        &fr_dsn::parser::scope_parameter::DsnReadOptions::default(),
    ) {
        fr_dsn::BoardReadResult::Success { board, .. }
        | fr_dsn::BoardReadResult::OutlineMissing { board, .. } => {
            *board.expect("rpi_splitter produces a board")
        }
        other => panic!("rpi_splitter did not read: {other:?}"),
    }
}

fn rpi_settings(board: &Board) -> RouterSettings {
    let mut settings = default_settings();
    settings.set_layer_count(board.get_layer_count());
    settings.apply_board_specific_optimizations(board);
    let fanout = settings.fanout.get_or_insert_with(Default::default);
    fanout.enabled = Some(false);
    settings.set_run_router(true);
    settings.set_run_optimizer(false);
    settings.max_passes = Some(1);
    settings
}

#[test]
#[cfg_attr(debug_assertions, ignore)]
fn the_history_ranks_boards_by_penalty_ascending() {
    if !parity::require_java_dir() {
        return;
    }
    let mut unrouted = load_rpi();
    let settings = rpi_settings(&unrouted);
    let scoring = settings.scoring.clone().expect("scoring");
    let mut routed = unrouted.clone();
    AutorouteBatchLoop::run(
        &mut routed,
        &settings,
        &RouterStop::new(),
        RouterBudget::disabled(),
        &mut NoopProgressSink,
    )
    .expect("rpi_splitter routes");

    let mut history = BoardHistory::new(&scoring);
    assert_eq!(history.best_penalty(), f64::INFINITY);
    history.add(&mut unrouted);
    history.add(&mut routed);

    let routed_penalty = BoardStatistics::new(&mut routed).routing_penalty(&scoring);
    let unrouted_penalty = BoardStatistics::new(&mut unrouted).routing_penalty(&scoring);
    assert!(routed_penalty < unrouted_penalty);
    assert_eq!(history.entries()[0].penalty, routed_penalty);
    assert_eq!(history.entries()[1].penalty, unrouted_penalty);
    assert_eq!(history.best_penalty(), routed_penalty);
}

#[test]
#[cfg_attr(debug_assertions, ignore)]
fn a_fully_routed_board_still_gets_an_optimizer_pass() {
    if !parity::require_java_dir() {
        return;
    }
    let mut board = load_rpi();
    let mut settings = rpi_settings(&board);
    settings.max_passes = Some(8);
    settings.fanout.get_or_insert_with(Default::default).enabled = Some(true);
    AutorouteBatchLoop::run(
        &mut board,
        &settings,
        &RouterStop::new(),
        RouterBudget::disabled(),
        &mut NoopProgressSink,
    )
    .expect("rpi_splitter routes");
    let scoring = settings.scoring.clone().expect("scoring");
    let before = BoardStatistics::new(&mut board);
    assert_eq!(before.connections.incomplete_count, Some(0));
    assert!(
        before.normalized_score(&scoring) > 990.1,
        "the old exit's territory"
    );

    settings.set_run_optimizer(true);
    let mut optimizer = BatchOptimizer::new(&settings);
    let result = optimizer
        .run_batch_loop(
            &mut board,
            &RouterStop::new(),
            RouterBudget::disabled(),
            &mut NoopProgressSink,
        )
        .expect("the stage runs");

    assert!(!result.per_pass.is_empty(), "at least one pass completed");
    let after = BoardStatistics::new(&mut board);
    assert!(after.routing_cost(&scoring) < before.routing_cost(&scoring));
}

#[test]
#[cfg_attr(debug_assertions, ignore)]
fn the_job_deadline_ends_the_optimizer_stage_on_a_routed_board() {
    if !parity::require_java_dir() {
        return;
    }
    let mut board = load_rpi();
    let mut settings = rpi_settings(&board);
    settings.max_passes = Some(8);
    settings.fanout.get_or_insert_with(Default::default).enabled = Some(true);
    AutorouteBatchLoop::run(
        &mut board,
        &settings,
        &RouterStop::new(),
        RouterBudget::disabled(),
        &mut NoopProgressSink,
    )
    .expect("rpi_splitter routes");
    assert_eq!(
        BoardStatistics::new(&mut board)
            .connections
            .incomplete_count,
        Some(0),
        "a complete board: the per-item re-router has nothing to route and never polls"
    );

    settings.set_run_optimizer(true);
    let stop = RouterStop::with_deadline(1);
    std::thread::sleep(std::time::Duration::from_millis(5));
    let mut optimizer = BatchOptimizer::new(&settings);
    let result = optimizer
        .run_batch_loop(
            &mut board,
            &stop,
            RouterBudget::disabled(),
            &mut NoopProgressSink,
        )
        .expect("the stage runs");

    assert!(stop.is_timed_out(), "the stage polled the job deadline");
    assert!(
        stop.is_stop_requested(),
        "…and the deadline stops everything"
    );
    assert_eq!(result.state, fr_router::pipeline::TaskState::Cancelled);
    assert!(
        result.items_optimized <= 1,
        "at most the item in flight when the deadline was seen: {}",
        result.items_optimized
    );
}

#[test]
#[cfg_attr(debug_assertions, ignore)]
fn the_statistics_incomplete_count_is_the_drc_incomplete_count() {
    if !parity::require_java_dir() {
        return;
    }
    let mut board = load_rpi();
    let settings = rpi_settings(&board);
    AutorouteBatchLoop::run(
        &mut board,
        &settings,
        &RouterStop::new(),
        RouterBudget::disabled(),
        &mut NoopProgressSink,
    )
    .expect("rpi_splitter routes");
    let from_statistics = BoardStatistics::with_options(&mut board, None, false)
        .connections
        .incomplete_count
        .expect("the statistics count connections");
    let from_drc = BatchOptimizer::calculate_incomplete_count(&mut board);
    assert!(
        from_statistics > 0,
        "a one-pass board still has open connections"
    );
    assert_eq!(
        usize::try_from(from_statistics).expect("non-negative"),
        from_drc
    );
}

#[test]
fn the_search_stop_check_sees_an_expired_job_deadline_without_a_poll() {
    let stop = RouterStop::with_deadline(-1);
    assert!(
        !stop.is_stop_requested(),
        "nothing has polled the deadline yet"
    );
    assert!(
        stop.is_stopped_or_expired(),
        "the check the maze search runs must notice the deadline itself: a connection that \
         starts just before it can otherwise search for its whole per-connection time limit"
    );
    assert!(stop.is_timed_out());
    assert!(stop.is_stop_requested());

    let fresh = RouterStop::new();
    assert!(!fresh.is_stopped_or_expired());
    assert!(!fresh.is_timed_out());
}

#[test]
fn the_routing_decision_statistics_skip_the_fanout_census() {
    if !parity::require_java_dir() {
        return;
    }
    let mut board = load_rpi();
    let full = BoardStatistics::with_options(&mut board, None, false);
    let slim = BoardStatistics::for_routing_decisions(&mut board);

    assert_eq!(slim.items, full.items);
    assert_eq!(slim.traces, full.traces);
    assert_eq!(slim.connections, full.connections);
    assert_eq!(slim.vias, full.vias);
    assert_eq!(slim.bends, full.bends);
    assert_eq!(slim.clearance_violations, full.clearance_violations);
    assert!(
        full.fanout.total_smd_pins > 0,
        "rpi_splitter has SMD pins, so the full census counts them"
    );
    assert_eq!(
        slim.fanout,
        fr_router::score::BoardStatisticsFanout::default()
    );
}

#[test]
#[cfg_attr(debug_assertions, ignore = "routes rpi_splitter; run with --release")]
fn the_optimizers_carried_incomplete_counts_match_a_full_count() {
    if !parity::require_java_dir() {
        return;
    }
    let mut board = load_rpi();
    let settings = rpi_settings(&board);
    let stop = RouterStop::new();
    AutorouteBatchLoop::run(
        &mut board,
        &settings,
        &stop,
        RouterBudget::disabled(),
        &mut NoopProgressSink,
    )
    .expect("rpi_splitter routes");

    let mut optimizer = BatchOptimizer::new(&settings);
    optimizer.begin_pass_bookkeeping(&mut board);
    let mut cursor = fr_router::pipeline::ReadSortedRouteItems::new();
    let mut accepted = 0;
    let mut undone = 0;
    while let Some(item) = cursor.next(&board) {
        if accepted + undone == 12 {
            break;
        }
        let full_before = BatchOptimizer::calculate_incomplete_count(&mut board);
        let result = optimizer
            .opt_route_item(
                &mut board,
                item,
                true,
                false,
                &stop,
                RouterBudget::disabled(),
                &mut NoopProgressSink,
            )
            .expect("the item routes");
        assert_eq!(
            usize::try_from(result.incomplete_count_before()).expect("non-negative"),
            full_before,
            "the carried before-count is the board's count"
        );
        let expected_after = if result.improved() {
            accepted += 1;
            result.incomplete_count()
        } else {
            undone += 1;
            result.incomplete_count_before()
        };
        assert_eq!(
            BatchOptimizer::calculate_incomplete_count(&mut board),
            usize::try_from(expected_after).expect("non-negative"),
            "after item {item:?} (improved = {})",
            result.improved()
        );
    }
    assert!(
        accepted + undone > 0,
        "the routed board offers vias to optimise"
    );
}
