//! The four tests that need a *routed* board carry `#[cfg_attr(debug_assertions, ignore)]` and
use copper_board::prelude::*;
use copper_board::structure::FixedState;
use copper_geometry::{IntBox, IntOctagon, IntPoint, Shape, TileShape};
use copper_router::pipeline::{
    AutorouteBatchLoop, BatchOptimizer, ItemRouteResult, NamedAlgorithmType, NoopProgressSink,
    PORT_OPTIMIZER_ROUTE_WORK_BUDGET, ProgressSink, RouterBudget, RouterStop, RoutingEvent,
    StopRequestState, TaskState, optimizer_route_improved,
};
use copper_settings::sources::DefaultSettings;
use copper_settings::{HostEnvironment, RouterSettings, SettingsSource};

const BOUNDING_BOX: IntBox = IntBox {
    ll: IntPoint {
        x: -10_000,
        y: -10_000,
    },
    ur: IntPoint {
        x: 10_000,
        y: 10_000,
    },
};

const RPI: &str = "fixtures/Issue143-rpi_splitter.dsn";

fn layers() -> LayerStructure {
    LayerStructure::new(vec![Layer::new("front", true), Layer::new("back", true)])
}

fn empty_board() -> Board {
    let mut padstacks = Padstacks::new(layers());
    let through_shape = Shape::Tile(TileShape::Octagon(IntOctagon::new(
        -70, -70, 70, 70, -140, 140, -140, 140,
    )));
    padstacks.add(
        "thru",
        vec![Some(through_shape.clone()), Some(through_shape)],
        true,
        false,
    );
    let clearance_matrix = ClearanceMatrix::get_default_instance(&layers(), 200);
    let rules = BoardRules::new(layers(), clearance_matrix);
    let mut board = Board::new(
        Vec::new(),
        0,
        BOUNDING_BOX,
        rules,
        BoardLibrary::new(padstacks, Packages::new()),
        Components::new(),
        Communication::default(),
    );
    let default_class = board.rules.get_default_net_class();
    for name in ["N1", "N2", "N3", "N4"] {
        board.rules.nets.add(name, 1, false, default_class);
    }
    board
}

fn load_board(rel_path: &str) -> Board {
    let path = testkit::corpus_dir().join(rel_path);
    let file = std::fs::File::open(&path)
        .unwrap_or_else(|e| panic!("cannot open {}: {e}", path.display()));
    let design_name = path
        .file_name()
        .expect("a file name")
        .to_string_lossy()
        .into_owned();
    match copper_dsn::read_board(
        file,
        None,
        Some(&design_name),
        &copper_dsn::parser::scope_parameter::DsnReadOptions::default(),
    ) {
        copper_dsn::BoardReadResult::Success { board, .. }
        | copper_dsn::BoardReadResult::OutlineMissing { board, .. } => {
            *board.unwrap_or_else(|| panic!("{design_name} produced no board"))
        }
        other => panic!("{design_name} did not read: {other:?}"),
    }
}

fn build_settings(board: &Board) -> RouterSettings {
    let mut settings = DefaultSettings::new(&HostEnvironment::detect())
        .get_settings()
        .expect("DefaultSettings always answers a table")
        .clone();
    settings.set_layer_count(board.get_layer_count());
    settings.apply_board_specific_optimizations(board);
    let fanout = settings.fanout.get_or_insert_with(Default::default);
    fanout.enabled = Some(false);
    fanout.max_milliseconds_per_pin = Some(i64::from(i32::MAX));
    settings.set_run_router(true);
    settings.set_run_optimizer(false);
    settings.save_intermediate_stages = Some(false);
    settings
}

fn routed_rpi() -> (Board, RouterSettings) {
    let mut board = load_board(RPI);
    let mut settings = build_settings(&board);
    settings.fanout.get_or_insert_with(Default::default).enabled = Some(true);
    settings.max_passes = Some(8);
    let stop = RouterStop::new();
    let mut sink = NoopProgressSink;
    AutorouteBatchLoop::run(
        &mut board,
        &settings,
        &stop,
        RouterBudget::disabled(),
        &mut sink,
    )
    .expect("rpi_splitter has a routable signal layer");
    (board, settings)
}

fn optimizer_settings(settings: &mut RouterSettings) -> &mut copper_settings::OptimizerSettings {
    settings
        .optimizer
        .as_mut()
        .expect("DefaultSettings always fills the optimizer block")
}

struct RecordingSink {
    events: Vec<RoutingEvent>,
}

impl ProgressSink for RecordingSink {
    fn on_event(&mut self, event: &RoutingEvent) {
        self.events.push(event.clone());
    }
}

#[test]
fn the_improvement_recomputation_disagrees_with_the_scorecard_field() {
    let result = ItemRouteResult::new(ItemId(1), 10, 3, 1000.0, 900.0, 0, 0, 0.0, 0.0);

    assert!(
        (result.improvement_percentage() - 0.55).abs() < 1e-6,
        "the scorecard field truncates the via term to 0, giving 0.55, not {}",
        result.improvement_percentage()
    );

    let route_improved = optimizer_route_improved(&result, 10, 1000.0);
    assert!(
        (route_improved - 0.4).abs() < 1e-6,
        "the pass's recomputation divides for real, giving 0.4, not {route_improved}"
    );

    assert!(
        (result.improvement_percentage() - route_improved).abs() > 0.1,
        "the two numbers are not each other, and the port must not quietly reconcile them"
    );
}

#[test]
fn the_recomputation_guard_answers_zero_and_so_reads_as_no_improvement() {
    let result = ItemRouteResult::new(ItemId(1), 10, 3, 1000.0, 900.0, 0, 0, 0.0, 0.0);
    assert_eq!(optimizer_route_improved(&result, 0, 1000.0), 0.0);
    assert_eq!(optimizer_route_improved(&result, 10, 0.0), 0.0);
}

#[test]
fn the_increased_ripup_costs_are_dropped_after_one_non_improving_pass() {
    let settings = DefaultSettings::new(&HostEnvironment::detect())
        .get_settings()
        .expect("DefaultSettings always answers a table")
        .clone();
    let mut optimizer = BatchOptimizer::new(&settings);

    optimizer.use_increased_ripup_costs = true;

    let (pass_improvement, force_another_pass) =
        optimizer.apply_pass_improvement(0, 800.0, 0, 800.0);
    assert_eq!(pass_improvement, 0.0, "(800 - 800) / 800 is 0");
    assert!(force_another_pass, ":215's sentinel, as its own bool");
    assert!(!optimizer.use_increased_ripup_costs, ":213 clears the flag");

    let (pass_improvement, force_another_pass) =
        optimizer.apply_pass_improvement(0, 800.0, 0, 800.0);
    assert_eq!(pass_improvement, 0.0);
    assert!(
        !force_another_pass,
        ":217 — the arm cannot fire twice, so the loop's threshold exit is now reachable"
    );

    let mut optimizer = BatchOptimizer::new(&settings);
    optimizer.use_increased_ripup_costs = true;
    let (pass_improvement, force_another_pass) =
        optimizer.apply_pass_improvement(0, 800.0, 0, 792.0);
    assert!((pass_improvement - 0.01).abs() < 1e-6, "8 / 800 is 1 %");
    assert!(!force_another_pass);
    assert!(optimizer.use_increased_ripup_costs, "still up");
}

#[test]
fn the_improvement_flag_is_a_bool() {
    let settings = DefaultSettings::new(&HostEnvironment::detect())
        .get_settings()
        .expect("DefaultSettings always answers a table")
        .clone();

    let mut optimizer = BatchOptimizer::new(&settings);
    optimizer.use_increased_ripup_costs = false;
    let (pass_improvement, force_another_pass) =
        optimizer.apply_pass_improvement(0, 800.0, 0, 1600.0);
    assert_eq!(
        pass_improvement, -1.0,
        ":209-210 — (800 - 1600) / 800 is exactly Java's sentinel value"
    );
    assert!(
        !force_another_pass,
        "fixed: T9 (#228) — the decision is its own bool, so a real -1.0 is not mistaken for \
         `:215`'s `keep going`"
    );
    let threshold = 0.01_f64;
    assert!(
        !force_another_pass && pass_improvement < threshold,
        "the threshold exit fires, which is what `:214`'s own comment intends"
    );

    let mut optimizer = BatchOptimizer::new(&settings);
    optimizer.use_increased_ripup_costs = true;
    assert!(optimizer.apply_pass_improvement(0, 800.0, 0, 800.0).1);
    assert!(!optimizer.apply_pass_improvement(0, 800.0, 0, 800.0).1);
}

#[test]
fn the_pass_improvement_is_zero_when_the_cost_before_is_not_positive() {
    let settings = DefaultSettings::new(&HostEnvironment::detect())
        .get_settings()
        .expect("DefaultSettings always answers a table")
        .clone();
    let mut optimizer = BatchOptimizer::new(&settings);
    let (pass_improvement, _) = optimizer.apply_pass_improvement(0, 0.0, 0, 500.0);
    assert_eq!(
        pass_improvement, 0.0,
        "a division by zero would have been +inf; Java's ternary answers 0"
    );
}

#[test]
fn passes_alternate_preferred_directions() {
    let mut board = empty_board();
    let padstack = copper_board::ids::PadstackId(
        board
            .library
            .padstacks
            .get_by_name("thru")
            .expect("the thru padstack")
            .no,
    );
    board
        .insert_via(
            padstack,
            copper_geometry::Point::new(1_000, 1_000),
            vec![1],
            1,
            FixedState::UserFixed,
            false,
        )
        .expect("a user-fixed via costs 50 and is not an optimizable item");
    let mut settings = build_settings(&board);
    {
        let optimizer = optimizer_settings(&mut settings);
        optimizer.max_passes = Some(3);
        optimizer.optimization_improvement_threshold = Some(-1.0);
    }
    let mut optimizer = BatchOptimizer::new(&settings);
    let stop = RouterStop::new();
    let mut sink = NoopProgressSink;
    let result = optimizer
        .run_batch_loop(&mut board, &stop, RouterBudget::disabled(), &mut sink)
        .expect("an item-less pass cannot fail");

    assert_eq!(result.passes_run, 3, ":167-168 — `currentPass < maxPasses`");
    let flags: Vec<bool> = result
        .per_pass
        .iter()
        .map(|record| record.with_preferred_directions)
        .collect();
    assert_eq!(
        flags,
        vec![true, false, true],
        "pass 1 and pass 3 are odd, pass 2 is not"
    );
    assert_eq!(result.state, TaskState::Finished);
    assert_eq!(result.items_optimized, 0, "the board has no route items");
}

#[test]
fn a_board_with_nothing_to_route_exits_before_the_first_pass() {
    let mut board = empty_board();
    let settings = build_settings(&board);
    let mut optimizer = BatchOptimizer::new(&settings);
    let stop = RouterStop::new();
    let mut sink = NoopProgressSink;
    let before = board.structural_hash();
    let result = optimizer
        .run_batch_loop(&mut board, &stop, RouterBudget::disabled(), &mut sink)
        .expect("an item-less pass cannot fail");

    assert_eq!(result.passes_run, 1, "the pass is counted, then abandoned");
    assert!(
        result.per_pass.is_empty(),
        "a zero routing cost has nothing to improve"
    );
    assert_eq!(result.items_optimized, 0);
    assert_eq!(board.structural_hash(), before);
}

#[test]
fn an_all_stop_disables_this_stage() {
    let mut board = empty_board();
    let settings = build_settings(&board);
    let mut optimizer = BatchOptimizer::new(&settings);
    let stop = RouterStop::new();
    stop.request_stop();
    let mut sink = RecordingSink { events: Vec::new() };
    let result = optimizer
        .run_batch_loop(&mut board, &stop, RouterBudget::disabled(), &mut sink)
        .expect("the loop is never entered");

    assert_eq!(result.passes_run, 0);
    assert!(result.per_pass.is_empty());
    assert_eq!(result.items_optimized, 0);
    assert_eq!(result.state, TaskState::Cancelled, ":253-255's ternary");

    assert_eq!(
        sink.events,
        vec![
            RoutingEvent::TaskStateChanged {
                algorithm: NamedAlgorithmType::Optimizer,
                state: TaskState::Started,
            },
            RoutingEvent::TaskStateChanged {
                algorithm: NamedAlgorithmType::Optimizer,
                state: TaskState::Finished,
            },
        ],
        "`:162-163` then `:233-234`, with nothing between them"
    );
}

#[test]
fn an_auto_router_only_stop_does_not_disable_this_stage() {
    let mut board = empty_board();
    let settings = build_settings(&board);
    let mut optimizer = BatchOptimizer::new(&settings);
    let stop = RouterStop::new();
    stop.request_stop_auto_router();
    let mut sink = NoopProgressSink;
    let result = optimizer
        .run_batch_loop(&mut board, &stop, RouterBudget::disabled(), &mut sink)
        .expect("an item-less pass cannot fail");

    assert_eq!(result.passes_run, 1, "the loop head let it through");
    assert_eq!(
        result.state,
        TaskState::Finished,
        "not `Cancelled`: `:254` reads `isStopRequested()` too"
    );
}

#[test]
fn the_stage_deadline_times_out_without_touching_the_stop_flag() {
    let mut board = empty_board();
    let mut settings = build_settings(&board);
    optimizer_settings(&mut settings).timeout_string = Some("0".to_string());
    let mut optimizer = BatchOptimizer::new(&settings);
    let stop = RouterStop::new();
    let mut sink = NoopProgressSink;
    let result = optimizer
        .run_batch_loop(&mut board, &stop, RouterBudget::disabled(), &mut sink)
        .expect("the loop breaks at the deadline");

    assert!(result.timed_out, ":173");
    assert!(optimizer.is_timed_out(), ":81-83 reads the same field");
    assert_eq!(
        result.passes_run, 0,
        ":172-176 breaks before `++currentPass`"
    );
    assert_eq!(result.state, TaskState::TimedOut, ":252-255");

    assert!(
        !stop.is_stop_requested(),
        "the stage deadline is not a job stop"
    );
    assert!(!stop.is_stop_auto_router_requested());
    assert!(
        !stop.is_timed_out(),
        "`RouterStop::poll_deadline` was never called"
    );
}

#[test]
fn the_stage_clock_never_writes_the_jobs_state() {
    let mut board = empty_board();
    let mut settings = build_settings(&board);
    optimizer_settings(&mut settings).timeout_string = Some("0".to_string());
    let mut optimizer = BatchOptimizer::new(&settings);
    let stop = RouterStop::new();
    let mut sink = NoopProgressSink;
    let result = optimizer
        .run_batch_loop(&mut board, &stop, RouterBudget::disabled(), &mut sink)
        .expect("the loop breaks at the stage deadline");

    assert!(result.timed_out, "the **stage**'s clock fired");
    assert_eq!(result.state, TaskState::TimedOut);
    assert!(
        !stop.is_stop_requested(),
        "a stage timeout is not a job timeout: the flag is still NONE"
    );
    assert!(
        !stop.is_timed_out(),
        "…and `job.state` was never written to TIMED_OUT"
    );
}

#[test]
fn the_jobs_deadline_ends_the_optimizer_stage() {
    let mut board = empty_board();
    let mut settings = build_settings(&board);
    optimizer_settings(&mut settings).timeout_string = Some("0".to_string());
    let mut optimizer = BatchOptimizer::new(&settings);
    let stop = RouterStop::with_deadline(-1);
    let mut sink = NoopProgressSink;
    let result = optimizer
        .run_batch_loop(&mut board, &stop, RouterBudget::disabled(), &mut sink)
        .expect("the loop breaks at the job deadline");

    assert!(
        stop.is_timed_out(),
        "the job's deadline was polled before the first pass"
    );
    assert!(stop.is_stop_requested(), "…and it stops everything");
    assert_eq!(result.state, TaskState::Cancelled);
    assert!(!result.timed_out, "the stage's own clock never got to fire");
}

#[test]
fn no_timeout_string_means_no_deadline() {
    let settings = DefaultSettings::new(&HostEnvironment::detect())
        .get_settings()
        .expect("DefaultSettings always answers a table")
        .clone();
    assert_eq!(
        settings
            .optimizer
            .as_ref()
            .expect("the optimizer block")
            .timeout_string,
        None,
        "DefaultSettings.java:130-142 sets no timeout, so no `p7t*` run has a deadline"
    );

    let mut board = empty_board();
    let build = build_settings(&board);
    let mut optimizer = BatchOptimizer::new(&build);
    assert!(
        !optimizer.is_deadline_reached(),
        "no deadline before the run"
    );
    let stop = RouterStop::new();
    let mut sink = NoopProgressSink;
    optimizer
        .run_batch_loop(&mut board, &stop, RouterBudget::disabled(), &mut sink)
        .expect("an item-less pass cannot fail");
    assert!(!optimizer.is_deadline_reached());
    assert!(!optimizer.is_timed_out());
}

#[test]
fn normalize_algorithm_always_answers_the_optimizers_id() {
    assert_eq!(
        BatchOptimizer::normalize_algorithm("copperroute-optimizer"),
        "copperroute-optimizer",
        ":69's guard is false, so `:76` never runs"
    );
    assert_eq!(
        BatchOptimizer::normalize_algorithm("something-else"),
        "copperroute-optimizer",
        ":70-76 — the warning, then the overwrite"
    );
    assert_eq!(
        BatchOptimizer::normalize_algorithm(""),
        "copperroute-optimizer",
        "Java's `equals` against a null or empty name is false, so this is the same arm"
    );
    let settings = DefaultSettings::new(&HostEnvironment::detect())
        .get_settings()
        .expect("DefaultSettings always answers a table")
        .clone();
    assert_eq!(
        settings
            .optimizer
            .as_ref()
            .expect("the optimizer block")
            .algorithm
            .as_deref(),
        Some(BatchOptimizer::ID)
    );
}

#[test]
fn the_five_named_algorithm_members_are_javas_literals() {
    assert_eq!(BatchOptimizer::ID, "copperroute-optimizer");
    assert_eq!(BatchOptimizer::NAME, "Copperroute Optimizer");
    assert_eq!(BatchOptimizer::VERSION, "1.0");
    assert_eq!(BatchOptimizer::DESCRIPTION, "Copperroute Optimizer v1.0");
    assert_eq!(BatchOptimizer::TYPE, NamedAlgorithmType::Optimizer);
}

#[test]
#[cfg_attr(debug_assertions, ignore)]
fn consecutive_failures_break_the_pass() {
    let (mut board, mut settings) = routed_rpi();
    {
        let optimizer = optimizer_settings(&mut settings);
        optimizer.max_consecutive_failures = Some(1);
        optimizer.max_passes = Some(1);
    }
    let mut optimizer = BatchOptimizer::new(&settings);
    optimizer.use_increased_ripup_costs = true;
    let stop = RouterStop::new();
    let mut sink = NoopProgressSink;
    optimizer
        .opt_route_pass(
            &mut board,
            1,
            true,
            &stop,
            RouterBudget::disabled(),
            &mut sink,
        )
        .expect("the pass runs");

    assert_eq!(
        optimizer.total_items_optimized, 3,
        "items 0 and 1 improve and item 2 fails; one failure is the limit"
    );

    let (mut board, settings) = routed_rpi();
    let mut optimizer = BatchOptimizer::new(&settings);
    optimizer.use_increased_ripup_costs = true;
    let mut sink = NoopProgressSink;
    optimizer
        .opt_route_pass(
            &mut board,
            1,
            true,
            &stop,
            RouterBudget::disabled(),
            &mut sink,
        )
        .expect("the pass runs");
    assert_eq!(optimizer.total_items_optimized, 9);
}

#[test]
#[cfg_attr(debug_assertions, ignore)]
fn an_auto_router_only_stop_still_runs_the_optimizer() {
    let (mut board, mut settings) = routed_rpi();
    optimizer_settings(&mut settings).max_passes = Some(1);
    let before = board.structural_hash();

    let mut optimizer = BatchOptimizer::new(&settings);
    let stop = RouterStop::new();
    stop.request_stop_auto_router();
    let mut sink = NoopProgressSink;
    let result = optimizer
        .run_batch_loop(&mut board, &stop, RouterBudget::disabled(), &mut sink)
        .expect("the stage runs");

    assert_eq!(
        result.passes_run, 1,
        "the stage is not disabled — `:171` reads `ALL`"
    );
    assert_eq!(
        result.items_optimized, 9,
        "it visits every item the reader offers, all of them `improved=false`"
    );
    assert_eq!(
        board.structural_hash(),
        before,
        "…and changes not one byte of the board, because every item routed zero passes and was \
         restored from its snapshot"
    );
    assert!(
        result
            .per_pass
            .iter()
            .all(|pass| pass.route_improved <= 0.0),
        "not one item improved: `:306`'s `routeImproved` never left 0 and `:365-368` drove it to -1"
    );

    let (mut board, mut settings) = routed_rpi();
    optimizer_settings(&mut settings).max_passes = Some(1);
    let mut optimizer = BatchOptimizer::new(&settings);
    let stop = RouterStop::new();
    stop.request_stop_auto_router();
    stop.begin_optimizer_stage();
    assert_eq!(
        stop.state(),
        StopRequestState::None,
        "the routing stage's own ending does not end the optimizer's"
    );
    let mut sink = NoopProgressSink;
    let result = optimizer
        .run_batch_loop(&mut board, &stop, RouterBudget::disabled(), &mut sink)
        .expect("the stage runs");

    assert_eq!(result.items_optimized, 9, "the same nine items are visited");
    assert!(
        result
            .per_pass
            .iter()
            .any(|pass| pass.route_improved != -1.0),
        "at least one item improved: `:365-368` drives `routeImproved` to -1 only when nothing \
         did — got {:?}",
        result
            .per_pass
            .iter()
            .map(|p| p.route_improved)
            .collect::<Vec<_>>()
    );
    assert_ne!(
        board.structural_hash(),
        before,
        "…and the board shape really changes — the whole point of the stage"
    );
}

#[test]
fn the_stage_scoped_stop_does_not_leak_into_the_router() {
    let cancelled = RouterStop::new();
    cancelled.request_stop();
    cancelled.begin_optimizer_stage();
    assert_eq!(cancelled.state(), StopRequestState::All);
    assert!(
        cancelled.is_stop_requested(),
        "`:117` still skips the stage"
    );

    let clean = RouterStop::new();
    clean.begin_optimizer_stage();
    assert_eq!(clean.state(), StopRequestState::None);

    let routed = RouterStop::new();
    routed.request_stop_auto_router();
    assert!(routed.is_stop_auto_router_requested());
    routed.begin_optimizer_stage();
    assert_eq!(routed.state(), StopRequestState::None);
    assert!(!routed.is_stop_auto_router_requested());
    assert!(!routed.is_stop_requested());

    let expired = RouterStop::with_deadline(-1);
    expired.request_stop_auto_router();
    expired.begin_optimizer_stage();
    assert_eq!(expired.state(), StopRequestState::None);
    assert!(expired.poll_deadline());
    assert!(
        expired.is_stop_requested(),
        "the job clock still ends the job"
    );
    expired.begin_optimizer_stage();
    assert_eq!(expired.state(), StopRequestState::All);
}

#[test]
#[cfg_attr(debug_assertions, ignore)]
fn the_optimizer_stage_is_pinned_on_the_routed_rpi() {
    let (mut board, settings) = routed_rpi();
    let mut optimizer = BatchOptimizer::new(&settings);
    let stop = RouterStop::new();
    let mut sink = NoopProgressSink;
    let result = optimizer
        .run_batch_loop(&mut board, &stop, RouterBudget::disabled(), &mut sink)
        .expect("the stage runs");

    assert_eq!(result.state, TaskState::Finished);
    assert_eq!(
        result.passes_run, 2,
        "pass 1 strips the fanout vias the router never needed; pass 2 finds nothing and ends \
         the loop"
    );
    assert_eq!(
        result.items_optimized, 15,
        "nine items in pass 1, six in pass 2"
    );
    assert!(!result.timed_out);
    assert!(
        !optimizer.use_increased_ripup_costs,
        "pass 2 improved nothing, so the increased ripup costs were dropped"
    );

    assert_eq!(result.per_pass.len(), 2);
    let first = result.per_pass[0];
    assert_eq!(first.pass, 1);
    assert!(first.with_preferred_directions, ":200 — pass 1 is odd");
    assert_eq!(first.record.incomplete_count, 0);
    assert_eq!(first.record.clearance_violations, 0);
    assert_eq!(first.record.via_count, 2, "nine fanout vias down to two");
    assert_eq!(first.record.trace_count, 14);
    assert!(
        first.pass_improvement > 0.5,
        "seven vias at 50 mm each are most of the board's cost: {}",
        first.pass_improvement
    );
    assert!(!first.force_another_pass);
    assert!(first.use_increased_ripup_costs);
    assert!(first.route_improved > 0.0);

    let second = result.per_pass[1];
    assert_eq!(second.pass, 2);
    assert!(!second.with_preferred_directions);
    assert_eq!(second.pass_improvement, 0.0);
    assert_eq!(second.route_improved, -1.0, ":365-368's sentinel");
    assert!(!second.use_increased_ripup_costs);
    assert_eq!(second.record.incomplete_count, 0);
    assert_eq!(second.record.via_count, 2);
    assert_eq!(second.record.trace_count, 14);
}

/// `sum of incompleteCount * passesRun` per item -- at [`PORT_OPTIMIZER_ROUTE_WORK_BUDGET`], and
#[test]
fn the_route_work_budget_bounds_only_incomplete_board_routing() {
    assert_eq!(PORT_OPTIMIZER_ROUTE_WORK_BUDGET, 1800);

    let board = empty_board();
    let settings = build_settings(&board);
    let mut optimizer = BatchOptimizer::new(&settings);

    assert_eq!(optimizer.total_route_work, 0);
    assert!(!optimizer.route_work_budget_spent());

    optimizer.total_route_work = PORT_OPTIMIZER_ROUTE_WORK_BUDGET - 1;
    assert!(!optimizer.route_work_budget_spent());
    optimizer.total_route_work = PORT_OPTIMIZER_ROUTE_WORK_BUDGET;
    assert!(optimizer.route_work_budget_spent());

    let work = |incomplete: i32, passes: i32| -> i64 {
        i64::from(incomplete.max(0)) * i64::from(passes.max(0))
    };
    assert_eq!(work(0, 6), 0, "a complete-board item is never charged");
    assert_eq!(
        work(30, 6),
        180,
        "an item on a 30-connection backlog is charged its attempts"
    );
    assert!(work(30, 6) * 9 < PORT_OPTIMIZER_ROUTE_WORK_BUDGET);
    assert!(work(30, 6) * 10 >= PORT_OPTIMIZER_ROUTE_WORK_BUDGET);
}

#[test]
fn every_optimizer_item_polls_the_job_deadline() {
    let mut board = empty_board();
    let padstack = copper_board::ids::PadstackId(
        board
            .library
            .padstacks
            .get_by_name("thru")
            .expect("the thru padstack")
            .no,
    );
    board
        .insert_via(
            padstack,
            copper_geometry::Point::new(1_000, 1_000),
            vec![1],
            1,
            FixedState::Unfixed,
            false,
        )
        .expect("a lone via on a net with nothing else to connect");
    let settings = build_settings(&board);
    let mut optimizer = BatchOptimizer::new(&settings);

    let stop = RouterStop::with_deadline(1);
    std::thread::sleep(std::time::Duration::from_millis(5));
    let mut sink = NoopProgressSink;
    optimizer
        .opt_route_pass(
            &mut board,
            1,
            true,
            &stop,
            RouterBudget::disabled(),
            &mut sink,
        )
        .expect("the pass runs");

    assert!(
        stop.is_timed_out(),
        "ripping the via leaves nothing to route, so no routing pass polls the deadline; the \
         item loop must poll it itself"
    );
    assert!(stop.is_stop_requested());
}
