//! Every test that routes a real board is release-only (`#[cfg_attr(debug_assertions, ignore)]`),
use std::path::PathBuf;

use fr_board::prelude::*;
use fr_router::RouterError;
use fr_router::pipeline::{
    NamedAlgorithmType, NoopProgressSink, PipelineResult, ProgressSink, RouterBudget, RouterStop,
    RoutingEvent, StopRequestState, TaskState, build_unrouted_report, run_pipeline,
};
use fr_settings::sources::DefaultSettings;
use fr_settings::{HostEnvironment, RouterSettings, SettingsSource};

const RPI: &str = "fixtures/Issue143-rpi_splitter.dsn";
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
    assert_eq!(
        result_a.final_statistics.connections.incomplete_count,
        result_b.final_statistics.connections.incomplete_count
    );
    assert_eq!(
        result_a.final_statistics.items.via_count,
        result_b.final_statistics.items.via_count
    );
    let mut board_a = board_a;
    let mut board_b = board_b;
    assert_eq!(
        fr_drc::DesignRulesChecker::new(&mut board_a)
            .get_all_violations()
            .len(),
        fr_drc::DesignRulesChecker::new(&mut board_b)
            .get_all_violations()
            .len()
    );
}

#[test]
#[cfg_attr(debug_assertions, ignore)]
fn the_fanout_only_mode_sets_max_passes_to_zero_and_leaves_the_callers_settings_untouched() {
    if !parity::require_java_dir() {
        return;
    }
    let mut board = load_board(RPI);
    let mut settings = build_settings(&board, 1);
    settings.set_run_router(false);
    settings.fanout.get_or_insert_with(Default::default).enabled = Some(true);
    settings
        .fanout
        .get_or_insert_with(Default::default)
        .max_milliseconds_per_pin = Some(i64::from(i32::MAX));
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

#[test]
#[cfg_attr(debug_assertions, ignore)]
fn neither_routing_limit_skips_the_optimizer_stage() {
    if !parity::require_java_dir() {
        return;
    }

    {
        let mut board = load_board(RPI);
        let mut settings = build_settings(&board, 8);
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
            !stop.is_stop_requested(),
            "fixed: T9 (#202) — reaching --max-items must not raise the ALL stop"
        );
        assert_eq!(stop.state(), StopRequestState::AutoRouterOnly);
        assert_ne!(
            result.optimizer_state,
            Some(TaskState::Idle),
            "fixed: T9 (#202) — a --max-items run optimises, exactly as a --max-passes run does"
        );
    }

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
            "an ordinary maxPasses stop must never raise ALL (quirk #214's five arms all call \
             requestStopAutoRouter)"
        );
        assert_eq!(
            stop.state(),
            StopRequestState::None,
            "the routing stage's own ending must not survive into — or past — the optimizer stage"
        );
        assert_ne!(
            result.optimizer_state,
            Some(TaskState::Idle),
            "an AUTO_ROUTER_ONLY stop does not skip the optimizer stage, so it must report a real \
             TaskState, not the never-entered one"
        );
    }
}

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

#[test]
#[cfg_attr(debug_assertions, ignore)]
fn the_unrouted_report_lists_airlines_in_getallairlines_order() {
    if !parity::require_java_dir() {
        return;
    }
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
