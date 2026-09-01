//! Plan 7 Task 15 — `run_pipeline`, the port of `RoutingPipeline.run()`
//! (`autoroute/pipeline/RoutingPipeline.java:81-129`), and `build_unrouted_report`, the port of
//! `AutorouteUnroutedReport.build` (`autoroute/pipeline/AutorouteUnroutedReport.java:19-54`).
//!
//! The whole-board evidence is `scripts/differential/run.sh p7t9 <dsn> <maxPasses> full`, which
//! drives the real Java `RoutingPipeline.createForHeadless(job).run()` against this crate's
//! `run_pipeline`. This file pins what a differential run cannot show as directly: the
//! fanout-only settings-clone contract, the `finishAutoroute` no-op, the `--max-items` vs
//! `--max-passes` distinction in whether the optimizer stage is skipped, the `ProgressSink`
//! event order across both stages, and `build_unrouted_report`'s net ordering.
//!
//! Every test that routes a real board is release-only (`#[cfg_attr(debug_assertions, ignore)]`),
//! the convention `crates/fr-router/tests/batch_loop.rs` and `optimizer.rs` both established:
//! `cargo test --release -p fr-router --test pipeline`, **without** `--ignored`.

use std::path::PathBuf;

use fr_board::prelude::*;
use fr_router::RouterError;
use fr_router::pipeline::{
    NamedAlgorithmType, NoopProgressSink, PipelineResult, ProgressSink, RouterBudget, RouterStop,
    RoutingEvent, TaskState, build_unrouted_report, run_pipeline,
};
use fr_settings::sources::DefaultSettings;
use fr_settings::{HostEnvironment, RouterSettings, SettingsSource};

// =================================================================================================
// The harness — the same shape `batch_loop.rs`'s and `optimizer.rs`'s carry
// =================================================================================================

/// `p7t9`'s default stem, and the smallest corpus board that actually routes (eight connections).
const RPI: &str = "fixtures/Issue143-rpi_splitter.dsn";
/// The fixture ruling 7's boundary test pins — every layer is a signal layer, so disabling all of
/// them is the only way a real corpus board reaches `AutorouteBatchLoop.java:51-56`.
const EMPTY_BOARD: &str = "fixtures/empty_board.dsn";

fn load_board(rel_path: &str) -> Board {
    let path: PathBuf = parity::java_dir().join(rel_path);
    let file = std::fs::File::open(&path)
        .unwrap_or_else(|e| panic!("cannot open {}: {e}", path.display()));
    let design_name = path
        .file_name()
        .expect("a file name")
        .to_string_lossy()
        .into_owned();
    match fr_dsn::read_board(
        file,
        None,
        Some(&design_name),
        &fr_dsn::parser::scope_parameter::DsnReadOptions::default(),
    ) {
        fr_dsn::BoardReadResult::Success { board, .. }
        | fr_dsn::BoardReadResult::OutlineMissing { board, .. } => {
            *board.unwrap_or_else(|| panic!("{design_name} produced no board"))
        }
        other => panic!("{design_name} did not read: {other:?}"),
    }
}

/// The headless ladder's priority-0 source plus this file's own knobs. Fanout is off unless a
/// test turns it on, so `run_pipeline`'s `routerEnabled` calculation is the whole story.
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

/// Keeps every event, tagged by which algorithm fired it — the shape
/// `crates/fr-router/tests/batch_loop.rs`'s own `Recorder` carries, widened to two algorithms.
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
    fn task_states(&self) -> Vec<(NamedAlgorithmType, TaskState)> {
        self.events
            .iter()
            .filter_map(|e| match e {
                RoutingEvent::TaskStateChanged { algorithm, state } => Some((*algorithm, *state)),
                _ => None,
            })
            .collect()
    }
}

// =================================================================================================
// `two_runs_of_the_same_board_are_identical` — the port of
// `RoutingPipelineComparisonTest.guiAndHeadlessPipelinesMatchOnDac2020Fixture`
// (`src/test/java/app/freerouting/fixtures/RoutingPipelineComparisonTest.java:49-76`), ruling 14
// =================================================================================================

/// Java's test compares a GUI-driven run against a headless run of the **same** settings on
/// **independent** board copies, because those are the only two code paths Java has, and settles
/// for three counters (`incompleteCount`, `viaCount`, clearance-violation count) because a GUI
/// board and a headless board are not otherwise comparable. The port has one code path, so the
/// analogous test is two independent `run_pipeline` calls on two independently loaded copies of
/// the same board under identical settings — and the stronger claim ruling 14 asks for is that
/// they are not just equal by those three counters but **byte-identical**
/// ([`Board::structural_hash`]), which is the whole point of a deterministic single-threaded
/// router.
#[test]
#[cfg_attr(debug_assertions, ignore)]
fn two_runs_of_the_same_board_are_identical() {
    if !parity::require_java_dir() {
        return;
    }

    fn run_once() -> (Board, PipelineResult) {
        let mut board = load_board(RPI);
        let mut settings = build_settings(&board, 1);
        settings.max_items = Some(2);
        settings.set_run_optimizer(true);
        settings
            .optimizer
            .get_or_insert_with(Default::default)
            .max_passes = Some(1);
        settings
            .optimizer
            .get_or_insert_with(Default::default)
            .max_items = Some(2);
        let stop = RouterStop::new();
        let mut sink = NoopProgressSink;
        let result = run_pipeline(
            &mut board,
            &settings,
            &stop,
            RouterBudget::disabled(),
            &mut sink,
        )
        .expect("rpi_splitter has a routable signal layer");
        (board, result)
    }

    let (board_a, result_a) = run_once();
    let (board_b, result_b) = run_once();

    assert_eq!(
        board_a.structural_hash(),
        board_b.structural_hash(),
        "two runs of the same board under identical settings must be byte-identical"
    );
    // The three counters Java's own test settles for, restated as the stronger claim's
    // corollaries rather than the whole assertion.
    assert_eq!(
        result_a.final_statistics.connections.incomplete_count,
        result_b.final_statistics.connections.incomplete_count
    );
    assert_eq!(
        result_a.final_statistics.items.via_count,
        result_b.final_statistics.items.via_count
    );
    // Java's third counter: the clearance-violation count on each routed board.
    let mut board_a = board_a;
    let mut board_b = board_b;
    assert_eq!(
        fr_drc::DesignRulesChecker::new(&mut board_a)
            .get_all_clearance_violations()
            .len(),
        fr_drc::DesignRulesChecker::new(&mut board_b)
            .get_all_clearance_violations()
            .len()
    );
}

// =================================================================================================
// `finish_autoroute_is_called_exactly_once`
// =================================================================================================

/// `RoutingPipeline.java:110`'s `this.job.board.finishAutoroute()` is the **only** caller of
/// `RoutingBoard.finishAutoroute` in the whole Java tree (`grep -rn "\.finishAutoroute()"
/// src/main` finds two hits — `RoutingPipeline.java:110` and `RoutingBoardUndoFacade.java:55` (via `RoutingBoard.deepCopy()`, whose four callers are all dead-on-headless or GUI-only; Task 15 review traced each) — so exactly one is reachable from `run_pipeline`, at that line). Ruling AJ (`BatchAutorouter::
/// BENCHMARK_RETAIN_AUTOROUTE_DATABASE` permanently `false`) makes the call a no-op on every path
/// this port can reach — no `AutorouteEngine` ever survives a lower-level call into
/// `run_pipeline`'s scope for `RoutingBoardExt::finish_autoroute` (the trait method built to
/// consume exactly that value) to be given one — so there is no board state, no counting `Board`
/// wrapper and no engine to inspect: `run_pipeline`'s signature does not even have an
/// `AutorouteEngine` to hand one. What is pinned instead is that the port's own transcription
/// notes the call exactly once, covering both arms of `:97-108` (the ordinary router run and the
/// fanout-only mode) rather than once per arm or not at all — `pipeline/run.rs`'s doc explains
/// why in full.
#[test]
fn finish_autoroute_is_called_exactly_once() {
    let src = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/pipeline/run.rs"),
    )
    .expect("crates/fr-router/src/pipeline/run.rs must be readable");
    let hits = src
        .lines()
        .filter(|line| line.contains("not reachable: RoutingBoard.finishAutoroute"))
        .count();
    assert_eq!(
        hits, 1,
        "RoutingPipeline.java:110 is the only finishAutoroute call in the whole Java tree; \
         run_pipeline's transcription must note it exactly once, after both routing arms and \
         before the optimizer stage — not once per arm, and not silently dropped"
    );
}

// =================================================================================================
// `the_fanout_only_mode_sets_max_passes_to_zero_and_leaves_the_callers_settings_untouched`
// =================================================================================================

/// `:99-108`: Java mutates the shared `job.routerSettings.maxPasses` to `0` and restores the
/// original value in a `finally` once `runBatchLoop()` returns. `run_pipeline` keeps
/// `settings: &RouterSettings` (Plan 8's contract), so it cannot mutate the caller's object at
/// all — it clones, mutates the clone, and drops it. This test drives the fanout-only branch
/// (`getRunRouter() == false`, `isFanoutEnabled() == true`) and asserts the caller's `max_passes`
/// reads back exactly what it was passed in with.
#[test]
#[cfg_attr(debug_assertions, ignore)]
fn the_fanout_only_mode_sets_max_passes_to_zero_and_leaves_the_callers_settings_untouched() {
    if !parity::require_java_dir() {
        return;
    }
    let mut board = load_board(RPI);
    let mut settings = build_settings(&board, 1);
    // `routerEnabled = getRunRouter() && (maxPasses == null || maxPasses >= 0)`: disabling the
    // router while keeping fanout on is the only way `:99` is reached rather than `:97`.
    settings.set_run_router(false);
    settings.fanout.get_or_insert_with(Default::default).enabled = Some(true);
    settings
        .fanout
        .get_or_insert_with(Default::default)
        .max_milliseconds_per_pin = Some(i64::from(i32::MAX));
    // A sentinel that is not `0`, so a leaked mutation is visible rather than accidentally
    // matching what the fanout-only branch would have set.
    let original_max_passes = Some(7);
    settings.max_passes = original_max_passes;

    let stop = RouterStop::new();
    let mut sink = NoopProgressSink;
    run_pipeline(
        &mut board,
        &settings,
        &stop,
        RouterBudget::disabled(),
        &mut sink,
    )
    .expect("rpi_splitter has a routable signal layer");

    assert_eq!(
        settings.max_passes, original_max_passes,
        "run_pipeline must not mutate the caller's settings — the fanout-only branch's \
         max_passes = 0 belongs to an internal clone"
    );
}

// =================================================================================================
// `a_max_items_stop_skips_the_optimizer_stage_but_a_max_passes_stop_does_not`
// =================================================================================================

/// Quirk candidate C, made observable end to end: `AutoroutePassRunner`'s `maxItems` gate
/// requests an **`ALL`** stop (`request_stop()`), which `RoutingPipeline.java:117`'s
/// `isStopRequested()` reads — so a router run that stops because it hit `--max-items` skips the
/// optimizer stage entirely. `AutorouteBatchLoop`'s `maxPasses` cap, by contrast, requests only
/// `AUTO_ROUTER_ONLY` (quirk #214) — `isStopRequested()` reads `false` for that, so the optimizer
/// stage **runs** (quirk #227: it changes nothing, because the shared flag also gates its own
/// item loop, but it runs and reports a real `TaskState`, not the "never entered"
/// [`TaskState::Idle`] the `max_items` case reports).
#[test]
#[cfg_attr(debug_assertions, ignore)]
fn a_max_items_stop_skips_the_optimizer_stage_but_a_max_passes_stop_does_not() {
    if !parity::require_java_dir() {
        return;
    }

    // ---- the max_items arm: ALL, optimizer stage never enters --------------------------------
    {
        let mut board = load_board(RPI);
        let mut settings = build_settings(&board, 8);
        // rpi_splitter has eight connections; two is enough to trip the router's own `max_items`
        // gate well inside the pass budget, so the stop is `ALL` before `maxPasses` ever matters.
        settings.max_items = Some(2);
        settings.set_run_optimizer(true);
        settings
            .optimizer
            .get_or_insert_with(Default::default)
            .max_passes = Some(2);

        let stop = RouterStop::new();
        let mut sink = NoopProgressSink;
        let result = run_pipeline(
            &mut board,
            &settings,
            &stop,
            RouterBudget::disabled(),
            &mut sink,
        )
        .expect("rpi_splitter has a routable signal layer");

        assert!(
            stop.is_stop_requested(),
            "the max_items gate must raise the ALL stop (AutoroutePassRunner's request_stop())"
        );
        assert_eq!(
            result.optimizer_state,
            Some(TaskState::Idle),
            "RoutingPipeline.java:117's isStopRequested() must skip the optimizer stage when the \
             max_items gate tripped"
        );
    }

    // ---- the max_passes arm: AUTO_ROUTER_ONLY, optimizer stage still runs ---------------------
    {
        let mut board = load_board(RPI);
        let mut settings = build_settings(&board, 1);
        settings.set_run_optimizer(true);
        settings
            .optimizer
            .get_or_insert_with(Default::default)
            .max_passes = Some(1);

        let stop = RouterStop::new();
        let mut sink = NoopProgressSink;
        let result = run_pipeline(
            &mut board,
            &settings,
            &stop,
            RouterBudget::disabled(),
            &mut sink,
        )
        .expect("rpi_splitter has a routable signal layer");

        assert!(
            !stop.is_stop_requested(),
            "an ordinary maxPasses stop must leave the flag at AUTO_ROUTER_ONLY, never ALL \
             (quirk #214)"
        );
        assert!(
            stop.is_stop_auto_router_requested(),
            "maxPasses being reached must raise at least AUTO_ROUTER_ONLY"
        );
        assert_ne!(
            result.optimizer_state,
            Some(TaskState::Idle),
            "quirk #227: an AUTO_ROUTER_ONLY stop does not skip the optimizer stage — it runs \
             (and changes nothing), so it must report a real TaskState, not the never-entered one"
        );
    }
}

// =================================================================================================
// `an_empty_board_errors_with_no_routable_layer`
// =================================================================================================

/// Ruling 7's sole new propagating boundary: `AutorouteBatchLoop.run`'s `IllegalArgumentException`
/// (`:51-56`) is not caught anywhere in `RoutingPipeline.run`, so it escapes as
/// [`RouterError::NoRoutableLayer`]. `empty_board.dsn` is a real corpus fixture with two signal
/// layers and no items at all, so both of its layers have to be switched off in settings — the
/// same shape `crates/fr-router/tests/batch_loop.rs`'s
/// `a_board_with_no_signal_layer_errors_and_reports_cancelled` pins one level down, restated here
/// at the pipeline boundary with no `P7T15Probe` needed on the Java side: the jar's own
/// `RoutingPipeline.createForHeadless(job).run()` throws the identical exception for the
/// identical reason.
#[test]
fn an_empty_board_errors_with_no_routable_layer() {
    if !parity::require_java_dir() {
        return;
    }
    let mut board = load_board(EMPTY_BOARD);
    let mut settings = build_settings(&board, 1);
    for layer in 0..settings.get_layer_count() {
        settings.set_layer_active(layer, false);
    }

    let stop = RouterStop::new();
    let mut sink = Recorder::default();
    let result = run_pipeline(
        &mut board,
        &settings,
        &stop,
        RouterBudget::disabled(),
        &mut sink,
    );

    assert!(
        matches!(result, Err(RouterError::NoRoutableLayer)),
        "AutorouteBatchLoop.java:55 throws IllegalArgumentException; run_pipeline propagates it \
         rather than catching it, got {result:?}"
    );
    assert_eq!(
        sink.task_states(),
        vec![(NamedAlgorithmType::Router, TaskState::Cancelled)],
        "AutorouteBatchLoop.java:53-54 fires CANCELLED before the throw, and nothing else runs"
    );
}

// =================================================================================================
// `a_recording_sink_sees_the_stage_events_in_javas_order`
// =================================================================================================

/// Both stages fire through the same `ProgressSink`, in `RoutingPipeline.run`'s own order
/// (`runRoutingStage()` then `runOptimizationStage()`, `:82-83`): the router's `Started` then its
/// final state, followed by the optimizer's `Started` then its final state. This is the pipeline
/// half of `crates/fr-router/tests/optimizer.rs`'s and `batch_loop.rs`'s own per-stage event
/// tests — what is new here is that both algorithms share one sink and one call, so the *order*
/// across algorithms is this task's own decision to get right.
#[test]
#[cfg_attr(debug_assertions, ignore)]
fn a_recording_sink_sees_the_stage_events_in_javas_order() {
    if !parity::require_java_dir() {
        return;
    }
    let mut board = load_board(RPI);
    let mut settings = build_settings(&board, 1);
    settings.set_run_optimizer(true);
    settings
        .optimizer
        .get_or_insert_with(Default::default)
        .max_passes = Some(1);

    let stop = RouterStop::new();
    let mut sink = Recorder::default();
    run_pipeline(
        &mut board,
        &settings,
        &stop,
        RouterBudget::disabled(),
        &mut sink,
    )
    .expect("rpi_splitter has a routable signal layer");

    let states = sink.task_states();
    assert!(
        states.len() >= 4,
        "at least Started + a final state per algorithm (each pass also fires Running); got \
         {states:?}"
    );
    assert_eq!(
        states[0],
        (NamedAlgorithmType::Router, TaskState::Started),
        "the router fires Started first; got {states:?}"
    );
    // Every Router event must precede every Optimizer event — `RoutingPipeline.run` is
    // `runRoutingStage(); runOptimizationStage();` (`:82-83`), never interleaved, because both
    // stages run to completion before the next one starts.
    let last_router = states
        .iter()
        .rposition(|(algorithm, _)| *algorithm == NamedAlgorithmType::Router)
        .expect("the router fired at least one event");
    let first_optimizer = states
        .iter()
        .position(|(algorithm, _)| *algorithm == NamedAlgorithmType::Optimizer)
        .expect("the optimizer fired at least one event");
    assert!(
        last_router < first_optimizer,
        "every Router event must precede every Optimizer event; got {states:?}"
    );
    assert_eq!(
        states[first_optimizer],
        (NamedAlgorithmType::Optimizer, TaskState::Started),
        "the optimizer's first event must be Started; got {states:?}"
    );
    assert_eq!(
        states.last().expect("at least one event").0,
        NamedAlgorithmType::Optimizer,
        "the optimizer's own final state must be the last event; got {states:?}"
    );
}

// =================================================================================================
// `the_unrouted_report_lists_airlines_in_getAllAirlines_order`
// =================================================================================================

/// `AutorouteUnroutedReport.build`'s `LinkedHashMap` is insertion-ordered by `getAllAirlines()`'s
/// own order, which is documented (`DesignRulesChecker::get_all_airlines`) to be **ascending net
/// number**, each net's own airlines contiguous. *Which* edges Kruskal picks inside one net is
/// JVM-run-dependent (plan-5 ruling 4) — only the count is deterministic — so this test asserts
/// the structural invariants that hold regardless: every net block's declared count matches the
/// lines under it, the sum matches `get_all_airlines().len()`, and the blocks appear in strictly
/// ascending net-number order (never a re-sort by name, which ruling 5 forbids).
#[test]
#[cfg_attr(debug_assertions, ignore)]
fn the_unrouted_report_lists_airlines_in_getallairlines_order() {
    if !parity::require_java_dir() {
        return;
    }
    // A freshly loaded, unrouted board: every connection is an airline, so the report has
    // several net blocks to check the ordering of.
    let mut board = load_board(RPI);
    let report = build_unrouted_report(&mut board);
    assert!(
        !report.is_empty() && !report.starts_with("  (no unrouted"),
        "rpi_splitter is unrouted on load; the report must list its incompletes, got: {report}"
    );

    let airline_count = fr_drc::DesignRulesChecker::new(&mut board)
        .get_all_airlines()
        .len();

    let mut total_lines = 0usize;
    let mut last_net_number: Option<i32> = None;
    let mut current_net_number: Option<i32> = None;
    let mut current_declared_count = 0usize;
    let mut current_line_count = 0usize;

    for line in report.lines() {
        if let Some(rest) = line.strip_prefix("  Net '") {
            // Close out the previous block before opening this one.
            if let Some(declared) = current_net_number.map(|_| current_declared_count) {
                assert_eq!(
                    declared, current_line_count,
                    "a net's declared unrouted-connection count must match its line count"
                );
            }
            let close_quote = rest.find('\'').expect("closing quote");
            let net_name = &rest[..close_quote];
            let tail = &rest[close_quote + 1..];
            let count_start = tail.find('(').expect("opening paren") + 1;
            let count_end = count_start
                + tail[count_start..]
                    .find(' ')
                    .expect("space after the count");
            let declared_count: usize = tail[count_start..count_end]
                .parse()
                .expect("the declared count must be an integer");

            let net_number = board
                .rules
                .nets
                .iter()
                .find(|net| net.name == net_name)
                .map(|net| net.net_number)
                .unwrap_or_else(|| {
                    panic!("report names a net the board does not have: {net_name}")
                });
            if let Some(last) = last_net_number {
                assert!(
                    net_number > last,
                    "getAllAirlines flattens in ascending net number; '{net_name}' (#{net_number}) \
                     followed a net numbered {last} or higher"
                );
            }
            last_net_number = Some(net_number);
            current_net_number = Some(net_number);
            current_declared_count = declared_count;
            current_line_count = 0;
        } else if line.trim_start().starts_with("- ") {
            current_line_count += 1;
            total_lines += 1;
        }
    }
    if current_net_number.is_some() {
        assert_eq!(
            current_declared_count, current_line_count,
            "the last net's declared count must match its line count too"
        );
    }

    assert_eq!(
        total_lines, airline_count,
        "the report must list exactly getAllAirlines().len() airlines in total"
    );
}
