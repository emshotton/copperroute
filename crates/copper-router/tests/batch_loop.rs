use std::path::PathBuf;

use copper_board::prelude::*;
use copper_router::RouterError;
use copper_router::pipeline::batch_loop::{
    BOARD_RANK_LIMIT, STAGNATION_PASS_LIMIT, STOP_AT_PASS_MINIMUM, STOP_AT_PASS_MODULO,
    final_best_board_swap, rank_limit_exceeded, restore_gate, stagnation_guard, stagnation_step,
};
use copper_router::pipeline::{
    AutorouteBatchLoop, BatchLoopExit, BoardHistory, NamedAlgorithmType, NoopProgressSink,
    ProgressSink, RouterBudget, RouterStop, RoutingEvent, StagnationStep, TaskState,
};
use copper_router::score::BoardStatistics;
use copper_settings::sources::DefaultSettings;
use copper_settings::{HostEnvironment, RouterSettings, ScoringSettings, SettingsSource};

const RPI: &str = "fixtures/Issue143-rpi_splitter.dsn";

const ECC83: &str = "fixtures/Issue649-kicad_ecc83-pp_input_board_v1.dsn";

const J2: &str = "fixtures/Issue026-J2_reference.dsn";

fn load_board(rel_path: &str) -> Board {
    let path: PathBuf = parity::reference_dir().join(rel_path);
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

fn build_settings(board: &Board, max_passes: i32) -> RouterSettings {
    let mut settings = DefaultSettings::new(&HostEnvironment::detect())
        .get_settings()
        .expect("DefaultSettings always answers a table")
        .clone();
    settings.set_layer_count(board.get_layer_count());
    settings.apply_board_specific_optimizations(board);
    settings.max_passes = Some(max_passes);
    settings.fanout.get_or_insert_with(Default::default).enabled = Some(false);
    settings.set_run_router(true);
    settings.set_run_optimizer(false);
    settings.save_intermediate_stages = Some(false);
    settings
}

#[derive(Default)]
struct Recorder {
    events: Vec<RoutingEvent>,
}

impl ProgressSink for Recorder {
    fn on_event(&mut self, event: &RoutingEvent) {
        self.events.push(event.clone());
    }
}

impl Recorder {
    fn states(&self) -> Vec<TaskState> {
        self.events
            .iter()
            .filter_map(|e| match e {
                RoutingEvent::TaskStateChanged { algorithm, state } => {
                    assert_eq!(
                        *algorithm,
                        NamedAlgorithmType::Router,
                        "AutorouteBatchLoop fires as the ROUTER, never the OPTIMIZER"
                    );
                    Some(*state)
                }
                _ => None,
            })
            .collect()
    }
}

fn scoring_of(settings: &RouterSettings) -> ScoringSettings {
    settings
        .scoring
        .clone()
        .expect("DefaultSettings always writes a scoring block")
}

#[test]
fn a_board_with_no_signal_layer_errors_and_reports_cancelled() {
    if !parity::require_reference_dir() {
        return;
    }
    let mut board = load_board(RPI);
    let mut settings = build_settings(&board, 1);
    for layer in 0..settings.get_layer_count() {
        settings.set_layer_active(layer, false);
    }

    let stop = RouterStop::new();
    let mut sink = Recorder::default();
    let result = AutorouteBatchLoop::run(
        &mut board,
        &settings,
        &stop,
        RouterBudget::disabled(),
        &mut sink,
    );

    assert!(
        matches!(result, Err(RouterError::NoRoutableLayer)),
        "AutorouteBatchLoop.java:55 throws IllegalArgumentException; ruling 7 propagates it, \
         got {result:?}"
    );
    assert_eq!(
        sink.states(),
        vec![TaskState::Cancelled],
        "AutorouteBatchLoop.java:53-54 fires CANCELLED before the throw, and nothing else runs"
    );
}

#[test]
#[cfg_attr(debug_assertions, ignore)]
fn an_active_non_signal_layer_is_not_routable() {
    if !parity::require_reference_dir() {
        return;
    }
    const CANIOT: &str = "fixtures/Issue269-caniot-tiny-arm.dsn";
    let mut board = load_board(CANIOT);
    let signal: Vec<bool> = board
        .layer_structure()
        .layers
        .iter()
        .map(|l| l.is_signal)
        .collect();
    assert_eq!(
        signal,
        vec![true, false],
        "the fixture must keep one signal layer and one plane, or this test proves nothing"
    );

    let ok = build_settings(&board, 1);
    assert!(
        AutorouteBatchLoop::run(
            &mut board.deep_copy(),
            &ok,
            &RouterStop::new(),
            RouterBudget::disabled(),
            &mut NoopProgressSink,
        )
        .is_ok(),
        "layer 0 is active and is a signal layer"
    );

    let mut plane_only = build_settings(&board, 1);
    plane_only.set_layer_active(0, false);
    let result = AutorouteBatchLoop::run(
        &mut board,
        &plane_only,
        &RouterStop::new(),
        RouterBudget::disabled(),
        &mut NoopProgressSink,
    );
    assert!(
        matches!(result, Err(RouterError::NoRoutableLayer)),
        "AutorouteBatchLoop.java:46 wants layerActive(i) AND layers[i].isSignal, got {result:?}"
    );
}

#[test]
#[cfg_attr(debug_assertions, ignore)]
fn a_normal_finish_reports_finished() {
    assert_eq!(
        BatchLoopExit::Completed.task_state(false),
        TaskState::Finished
    );
    assert_eq!(
        BatchLoopExit::MaxPasses.task_state(false),
        TaskState::Finished
    );
    assert_eq!(
        BatchLoopExit::NoImprovement.task_state(false),
        TaskState::Finished
    );
    assert_eq!(
        BatchLoopExit::Stagnation.task_state(false),
        TaskState::Finished
    );
    assert_eq!(
        BatchLoopExit::Cancelled.task_state(false),
        TaskState::Cancelled
    );
    assert_eq!(
        BatchLoopExit::Cancelled.task_state(true),
        TaskState::TimedOut
    );
    for door in [
        BatchLoopExit::Completed,
        BatchLoopExit::MaxPasses,
        BatchLoopExit::NoImprovement,
        BatchLoopExit::Stagnation,
    ] {
        assert_eq!(
            door.task_state(true),
            TaskState::Finished,
            "{door:?} is a finish whatever the job clock says — only a cancellation is timed out"
        );
    }

    if !parity::require_reference_dir() {
        return;
    }
    let mut board = load_board(RPI);
    let settings = build_settings(&board, 1);
    let stop = RouterStop::new();
    let mut sink = Recorder::default();

    let result = AutorouteBatchLoop::run(
        &mut board,
        &settings,
        &stop,
        RouterBudget::disabled(),
        &mut sink,
    )
    .expect("rpi_splitter has two signal layers");

    assert_eq!(
        result.exit(),
        BatchLoopExit::MaxPasses,
        "`:268-273` is the door a `--max-passes 1` run leaves by"
    );
    assert_eq!(
        result.state,
        TaskState::Finished,
        "fixed: T9 (#214) — a run that did exactly what it was asked has FINISHED"
    );
    assert!(
        !result.continue_routing,
        ":587 is `!isStopAutoRouterRequested()`, and :271 requested it — the flag is still raised, \
         which is what four other readers in the loop depend on"
    );
    assert_eq!(
        sink.states(),
        vec![TaskState::Started, TaskState::Running, TaskState::Finished]
    );
    assert_eq!(result.per_pass.len(), 1);
    assert_eq!(result.per_pass[0].incomplete_count, 2);
    assert!(
        result.per_pass[0].score > 0.0,
        "the FINISHED run routed a board: {:?}",
        result.per_pass[0]
    );
}

#[test]
#[cfg_attr(debug_assertions, ignore)]
fn only_a_pass_that_routes_nothing_reaches_finished() {
    if !parity::require_reference_dir() {
        return;
    }
    let mut board = load_board(ECC83);
    let settings = build_settings(&board, 0);
    let stop = RouterStop::new();
    let mut sink = Recorder::default();

    let result = AutorouteBatchLoop::run(
        &mut board,
        &settings,
        &stop,
        RouterBudget::disabled(),
        &mut sink,
    )
    .expect("the ecc83 board has two signal layers");

    assert_eq!(result.state, TaskState::Finished);
    assert!(
        result.continue_routing,
        ":587 answers true when nothing ever raised the flag"
    );
    assert_eq!(
        *sink.states().last().expect("a final state"),
        TaskState::Finished
    );
}

#[test]
#[cfg_attr(debug_assertions, ignore)]
fn max_passes_zero_is_unlimited() {
    if !parity::require_reference_dir() {
        return;
    }
    let run_with = |max_passes: i32| {
        let mut board = load_board(RPI);
        let settings = build_settings(&board, max_passes);
        let stop = RouterStop::new();
        let mut sink = NoopProgressSink;
        AutorouteBatchLoop::run(
            &mut board,
            &settings,
            &stop,
            RouterBudget::disabled(),
            &mut sink,
        )
        .expect("rpi_splitter has two signal layers")
    };

    let one = run_with(1);
    let unlimited = run_with(0);

    assert_eq!(one.per_pass.len(), 1, "maxPasses = 1 is one pass");
    assert!(
        unlimited.per_pass.len() > one.per_pass.len(),
        "maxPasses = 0 must not break at :271; it ran {} passes against {}",
        unlimited.per_pass.len(),
        one.per_pass.len()
    );
    assert_eq!(unlimited.exit(), BatchLoopExit::Stagnation);
    assert_eq!(unlimited.state, TaskState::Finished);
    assert_eq!(one.exit(), BatchLoopExit::MaxPasses);
    assert_eq!(one.state, TaskState::Finished);

    let disabled = run_with(-1);
    assert!(
        disabled.per_pass.is_empty(),
        ":221-223's `maxPasses >= 0` is false for -1, so isRouterEnabled is false"
    );
    assert_eq!(disabled.state, TaskState::Finished);
    assert_eq!(
        disabled.exit(),
        BatchLoopExit::Completed,
        "a loop that was never entered left by the `while` head's own first conjunct"
    );
}

#[test]
fn the_restore_gate_needs_eight_entries_and_a_pass_multiple_of_four() {
    assert_eq!(STOP_AT_PASS_MINIMUM, 8, "BatchAutorouter.java:46");
    assert_eq!(STOP_AT_PASS_MODULO, 4, "BatchAutorouter.java:49");

    for pass in [4, 8, 12, 16] {
        assert!(
            !restore_gate(7, pass, false),
            "bh.size() = 7 is below STOP_AT_PASS_MINIMUM at pass {pass}"
        );
    }

    assert!(
        !restore_gate(8, 4, false),
        ":299 also demands currentPass >= 8"
    );
    for pass in [5, 6, 7, 9, 10, 11] {
        assert!(
            !restore_gate(8, pass, false),
            "pass {pass} is not a multiple of 4"
        );
    }
    for pass in [8, 12, 16, 40] {
        assert!(restore_gate(8, pass, false), "pass {pass} opens both gates");
    }

    assert!(restore_gate(0, 1, true), ":298 and :300's second disjunct");
    assert!(
        !restore_gate(0, 1, false),
        "and only with the flag raised — an empty history at pass 1 opens neither gate"
    );
}

#[test]
fn the_rank_limit_can_never_fire() {
    assert_eq!(
        BOARD_RANK_LIMIT,
        BoardHistory::MAX_HISTORY_SIZE,
        "BatchAutorouter.java:40 — BOARD_RANK_LIMIT *is* the cap"
    );

    assert!(!rank_limit_exceeded(-1));

    for rank in 1..=BOARD_RANK_LIMIT {
        assert!(
            !rank_limit_exceeded(rank as i32),
            "rank {rank} is inside a history capped at {BOARD_RANK_LIMIT}"
        );
    }

    assert!(rank_limit_exceeded(BOARD_RANK_LIMIT as i32 + 1));

    let source = include_str!("../src/pipeline/batch_loop.rs");
    let flat = source.split_whitespace().collect::<Vec<_>>().join(" ");
    assert_eq!(
        flat.matches("if rank_limit_exceeded(").count(),
        0,
        "the rank break is deleted from `run`; re-enabling it is a product decision with its own \
         A/B, not a cleanup (quirk #217)"
    );
    assert!(
        flat.contains("pub fn rank_limit_exceeded(rank: i32) -> bool"),
        "…and the predicate survives its call site, so the row's arithmetic stays testable"
    );
}

#[test]
#[cfg_attr(debug_assertions, ignore)]
fn a_full_history_never_ranks_a_board_past_its_cap() {
    if !parity::require_reference_dir() {
        return;
    }
    let mut board = load_board(J2);
    let settings = build_settings(&board, 1);
    let scoring = scoring_of(&settings);
    let cap = 2usize;
    let mut bh = BoardHistory::with_capacity(&scoring, cap);

    bh.add(&mut board);
    let stop = RouterStop::new();
    let mut sink = NoopProgressSink;
    let mut router = copper_router::pipeline::BatchAutorouter::for_routing_job(
        &board,
        &settings,
        RouterBudget::disabled(),
    );
    let mut log = copper_router::pipeline::RoutingFailureLog::new();
    for pass in 1..=3 {
        router
            .autoroute_pass(&mut board, &mut log, pass, &stop, &mut sink)
            .expect("the pass runner's boundary answers Ok");
        bh.add(&mut board);
    }

    assert_eq!(bh.size(), cap, "add's eviction holds the list at the cap");
    for entry in bh.entries() {
        let rank = bh
            .entries()
            .iter()
            .position(|e| e.hash == entry.hash)
            .expect("an entry finds itself")
            + 1;
        assert!(
            rank <= cap,
            "getRank is a 1-indexed position in a list capped at {cap}, got {rank}"
        );
    }
}

#[test]
#[cfg_attr(debug_assertions, ignore)]
fn the_rank_the_loop_tests_is_read_after_restore_boards_reorder() {
    if !parity::require_reference_dir() {
        return;
    }
    let mut board = load_board(J2);
    let settings = build_settings(&board, 1);
    let scoring = scoring_of(&settings);

    let unrouted = board.deep_copy();
    let stop = RouterStop::new();
    let mut sink = NoopProgressSink;
    let mut router = copper_router::pipeline::BatchAutorouter::for_routing_job(
        &board,
        &settings,
        RouterBudget::disabled(),
    );
    let mut log = copper_router::pipeline::RoutingFailureLog::new();
    router
        .autoroute_pass(&mut board, &mut log, 1, &stop, &mut sink)
        .expect("the pass runner's boundary answers Ok");
    let after_one = board.deep_copy();
    router
        .autoroute_pass(&mut board, &mut log, 2, &stop, &mut sink)
        .expect("the pass runner's boundary answers Ok");
    let after_two = board.deep_copy();

    let score = |b: &Board| BoardStatistics::new(&mut b.deep_copy()).normalized_score(&scoring);
    assert!(
        score(&unrouted) < score(&after_one) && score(&after_one) < score(&after_two),
        "the fixture needs three strictly increasing scores, got {} {} {}",
        score(&unrouted),
        score(&after_one),
        score(&after_two)
    );

    let mut bh = BoardHistory::new(&scoring);
    for b in [&unrouted, &after_one, &after_two] {
        bh.add(&mut b.deep_copy());
    }
    assert_eq!(bh.size(), 3);

    assert_eq!(bh.rank(&unrouted), 1, "inserted first");
    assert_eq!(bh.rank(&after_two), 3, "inserted last");

    let restored = bh
        .restore_board(copper_router::pipeline::batch_loop::MAXIMUM_TRIES_ON_THE_SAME_BOARD)
        .expect("three fresh entries are all inside the restore budget");
    assert_eq!(
        score(&restored),
        score(&after_two),
        "BoardHistory.java:143-149 hands back the head of the descending sort"
    );

    assert_eq!(bh.rank(&after_two), 1, "best board, now first");
    assert_eq!(bh.rank(&unrouted), 3, "worst board, now last");
    assert_eq!(
        bh.rank(&restored),
        1,
        ":315 reads the rank of the board :307 just handed back"
    );

    assert!(!rank_limit_exceeded(bh.rank(&restored)));
}

#[test]
fn the_stagnation_counter_resets_only_from_pass_eight() {
    for pass in 1..STOP_AT_PASS_MINIMUM {
        assert!(
            !stagnation_guard(pass, true),
            "pass {pass} reaches no arm of the stagnation block, reset included"
        );
    }
    for pass in [STOP_AT_PASS_MINIMUM, 9, 12, 40] {
        assert!(
            stagnation_guard(pass, true),
            "pass {pass} takes the stagnation arm, and :509's reset is now inside it"
        );
    }
    assert!(!stagnation_guard(40, false));

    assert_eq!(
        stagnation_step(10.0, 5.0, 3),
        StagnationStep::ScoreImproved,
        ":425 — boardScoreAfter > lastBestScore + 0.5"
    );
    assert_eq!(
        stagnation_step(10.0, 5.0, 0),
        StagnationStep::ScoreImproved,
        ":425 is tested first, so an improving *and* completed pass reports the improvement"
    );
    assert_eq!(
        stagnation_step(5.0, 5.0, 0),
        StagnationStep::BoardRouted,
        ":510 — incompleteCount == 0 && boardScoreAfter > 0.5"
    );
    assert_eq!(
        stagnation_step(0.0, 5.0, 0),
        StagnationStep::Accumulate,
        "a fully-routed board with score == 0 keeps accumulating until the global tracker fires"
    );
    assert_eq!(stagnation_step(0.5, 5.0, 0), StagnationStep::Accumulate);
    assert_eq!(stagnation_step(5.0, 5.0, 3), StagnationStep::Accumulate);

    assert_eq!(STAGNATION_PASS_LIMIT, 10, "BatchAutorouter.java:52");
}

#[test]
#[cfg_attr(debug_assertions, ignore)]
fn the_final_swap_takes_the_best_board_only_when_it_is_strictly_better() {
    if !parity::require_reference_dir() {
        return;
    }
    let mut board = load_board(RPI);
    let settings = build_settings(&board, 1);
    let scoring = scoring_of(&settings);

    let stop = RouterStop::new();
    let mut sink = NoopProgressSink;
    let mut router = copper_router::pipeline::BatchAutorouter::for_routing_job(
        &board,
        &settings,
        RouterBudget::disabled(),
    );
    let mut log = copper_router::pipeline::RoutingFailureLog::new();
    for pass in 1..=3 {
        router
            .autoroute_pass(&mut board, &mut log, pass, &stop, &mut sink)
            .expect("the pass runner's boundary answers Ok");
    }
    let routed = board.deep_copy();
    let routed_score = BoardStatistics::new(&mut board).normalized_score(&scoring);
    let mut unrouted = load_board(RPI);
    let unrouted_score = BoardStatistics::new(&mut unrouted).normalized_score(&scoring);
    assert!(
        routed_score > unrouted_score,
        "the fixture only means anything if routing improved the score: \
         {routed_score} vs {unrouted_score}"
    );

    {
        let mut bh = BoardHistory::new(&scoring);
        bh.add(&mut unrouted);
        let mut live = routed.deep_copy();
        let before = live.structural_hash();
        assert!(
            !final_best_board_swap(&mut live, &mut bh, &scoring),
            ":531 is `bestHistoryScore > currentFinalScore`, and the history is worse"
        );
        assert_eq!(live.structural_hash(), before, "the board must not move");
    }

    {
        let mut bh = BoardHistory::new(&scoring);
        let mut better = routed.deep_copy();
        bh.add(&mut better);
        let mut live = load_board(RPI);
        assert!(
            final_best_board_swap(&mut live, &mut bh, &scoring),
            "the history's best board scores {routed_score} against the live {unrouted_score}"
        );
        assert_eq!(
            BoardStatistics::new(&mut live).normalized_score(&scoring),
            routed_score,
            ":535 replaces the board with the history's best"
        );
    }

    {
        let mut bh = BoardHistory::new(&scoring);
        let mut same = routed.deep_copy();
        bh.add(&mut same);
        let mut live = routed.deep_copy();
        assert!(
            !final_best_board_swap(&mut live, &mut bh, &scoring),
            ":531's `>` is strict — an equally good history entry does not displace the board"
        );
    }
}

#[test]
#[cfg_attr(debug_assertions, ignore)]
fn an_empty_history_never_swaps() {
    if !parity::require_reference_dir() {
        return;
    }
    let mut board = load_board(RPI);
    let settings = build_settings(&board, 1);
    let scoring = scoring_of(&settings);
    let mut bh = BoardHistory::new(&scoring);
    let before = board.structural_hash();
    assert_eq!(bh.best_penalty(), f64::INFINITY);
    assert!(!final_best_board_swap(&mut board, &mut bh, &scoring));
    assert_eq!(board.structural_hash(), before);
}

#[test]
#[cfg_attr(debug_assertions, ignore)]
fn routing_with_fanout_enabled_runs_the_pre_pass() {
    if !parity::require_reference_dir() {
        return;
    }
    let mut board = load_board(RPI);
    let mut settings = build_settings(&board, 1);
    settings.fanout.get_or_insert_with(Default::default).enabled = Some(true);
    settings
        .fanout
        .get_or_insert_with(Default::default)
        .max_milliseconds_per_pin = Some(i64::from(i32::MAX));
    let stop = RouterStop::new();
    let mut sink = NoopProgressSink;
    let result = AutorouteBatchLoop::run(
        &mut board,
        &settings,
        &stop,
        RouterBudget::disabled(),
        &mut sink,
    )
    .expect("the fixture has a routable signal layer");
    let summary = result
        .fanout
        .expect("`:123-172` ran, so `:173` had a summary to read");
    assert!(!summary.is_timed_out);
    assert!(
        summary.completed_pass_count > 0,
        "the stage ran at least one pass"
    );
    assert!(
        board.get_vias().len() >= 9,
        "the escape vias `p7t5 board` counts on this stem are on the board the loop answered"
    );
}

#[test]
fn the_stagnation_paths_build_the_unrouted_report() {
    let source = include_str!("../src/pipeline/batch_loop.rs");
    let flat = source.split_whitespace().collect::<Vec<_>>().join(" ");
    assert_eq!(
        flat.matches(
            "let _report = build_unrouted_report(board); stop.request_stop_auto_router(); \
             exit = Some(BatchLoopExit::Stagnation); break;"
        )
        .count(),
        2,
        "both stagnation exits build the report before stopping the router"
    );
}

#[test]
#[cfg_attr(debug_assertions, ignore)]
fn one_pass_record_per_completed_pass_in_order() {
    if !parity::require_reference_dir() {
        return;
    }
    let mut board = load_board(RPI);
    let settings = build_settings(&board, 0);
    let stop = RouterStop::new();
    let mut sink = NoopProgressSink;
    let result = AutorouteBatchLoop::run(
        &mut board,
        &settings,
        &stop,
        RouterBudget::disabled(),
        &mut sink,
    )
    .expect("rpi_splitter has two signal layers");

    assert!(!result.per_pass.is_empty());
    for (i, record) in result.per_pass.iter().enumerate() {
        assert_eq!(
            record.pass,
            i as i32 + 1,
            "PassRecord::pass is `currentPass`, 1-based and contiguous"
        );
    }
    let last = result.per_pass.last().expect("at least one pass");
    assert_eq!(
        result.passes_run, last.pass,
        "the loop left `currentPass` where the last completed pass put it"
    );
    let stats = BoardStatistics::new(&mut board);
    assert_eq!(
        stats.connections.incomplete_count.expect("filled"),
        last.incomplete_count as i32
    );
}
