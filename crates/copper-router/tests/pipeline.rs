//! Every test that routes a real board is release-only (`#[cfg_attr(debug_assertions, ignore)]`),
use std::path::PathBuf;

use copper_board::prelude::*;
use copper_router::RouterError;
use copper_router::pipeline::{
    NamedAlgorithmType, NoopProgressSink, PipelineResult, ProgressSink, RouterBudget, RouterStop,
    RoutingEvent, StopRequestState, TaskState, build_unrouted_report, run_pipeline,
};
use copper_settings::sources::DefaultSettings;
use copper_settings::{HostEnvironment, RouterSettings, SettingsSource};

const RPI: &str = "fixtures/Issue143-rpi_splitter.dsn";
const EMPTY_BOARD: &str = "fixtures/empty_board.dsn";

#[test]
fn fanout_frames_identify_their_stage_and_pass() {
    let mut board = load_test_board("tests/data/p9t13-multi-net-smd-pin.dsn");
    let mut settings = build_settings(&board, 1);
    settings.set_run_router(false);
    settings.fanout.as_mut().unwrap().enabled = Some(true);
    let directory = std::env::temp_dir().join(format!(
        "fr-fanout-frames-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut options = copper_router::RoutingVisualizationOptions::new(directory.clone());
    options.max_frames = 1;
    options.width = 160;
    options.height = 120;
    let guard = copper_router::start_routing_visualization(options).unwrap();
    let result = run_pipeline(
        &mut board,
        &settings,
        &RouterStop::new(),
        RouterBudget::disabled(),
        &mut NoopProgressSink,
    )
    .unwrap();
    let summary = guard.finish();
    let frames = std::fs::read_to_string(directory.join("frames.jsonl")).unwrap();
    std::fs::remove_dir_all(&directory).unwrap();
    assert_eq!(summary.frames_written, 1);
    assert!(frames.contains("\"phase\":\"fanout\""), "{frames}");
    assert!(frames.contains("\"pass\":1,"), "{frames}");
    assert_eq!(result.router_passes_completed, 0);
}

#[test]
fn expired_fanout_does_not_count_an_autorouter_pass() {
    let mut board = load_test_board("tests/data/p9t13-multi-net-smd-pin.dsn");
    let mut settings = build_settings(&board, 10);
    settings.fanout.as_mut().unwrap().enabled = Some(true);
    let result = run_pipeline(
        &mut board,
        &settings,
        &RouterStop::with_deadline(-1),
        RouterBudget::disabled(),
        &mut NoopProgressSink,
    )
    .unwrap();
    assert_eq!(result.router_state, TaskState::TimedOut);
    assert_eq!(result.router_passes_completed, 0);
}

#[test]
fn pass_panic_is_a_pipeline_error() {
    struct PanicSink;
    impl ProgressSink for PanicSink {
        fn on_event(&mut self, event: &RoutingEvent) {
            if matches!(event, RoutingEvent::BoardUpdated { .. }) {
                panic!("injected pass failure");
            }
        }
    }
    let mut board = load_test_board("tests/data/p9t13-multi-net-smd-pin.dsn");
    let settings = build_settings(&board, 1);
    let result = run_pipeline(
        &mut board,
        &settings,
        &RouterStop::new(),
        RouterBudget::disabled(),
        &mut PanicSink,
    );
    assert!(
        matches!(result, Err(RouterError::Panicked(message)) if message == "injected pass failure")
    );
}

#[test]
fn cancellation_during_a_pass_is_not_reported_as_finished() {
    struct CancelSink<'a>(&'a RouterStop);
    impl ProgressSink for CancelSink<'_> {
        fn on_event(&mut self, event: &RoutingEvent) {
            if matches!(event, RoutingEvent::BoardUpdated { .. }) {
                self.0.request_stop();
            }
        }
    }
    let mut board = load_test_board("tests/data/p9t13-multi-net-smd-pin.dsn");
    let settings = build_settings(&board, 10);
    let stop = RouterStop::new();
    let result = run_pipeline(
        &mut board,
        &settings,
        &stop,
        RouterBudget::disabled(),
        &mut CancelSink(&stop),
    )
    .unwrap();
    assert_eq!(result.router_state, TaskState::Cancelled);
    assert!(!result.timed_out);
    assert_eq!(result.router_passes_completed, 1);
}

#[test]
fn a_single_pass_limit_reports_one_pass() {
    let mut board = load_test_board("tests/data/p9t13-multi-net-smd-pin.dsn");
    let settings = build_settings(&board, 1);
    let result = run_pipeline(
        &mut board,
        &settings,
        &RouterStop::new(),
        RouterBudget::disabled(),
        &mut NoopProgressSink,
    )
    .unwrap();
    assert_eq!(result.router_state, TaskState::Finished);
    assert_eq!(result.router_passes_completed, 1);
}

fn load_board(rel_path: &str) -> Board {
    let path: PathBuf = testkit::corpus_dir().join(rel_path);
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

fn load_test_board(rel_path: &str) -> Board {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(rel_path);
    let file = std::fs::File::open(&path)
        .unwrap_or_else(|e| panic!("cannot open {}: {e}", path.display()));
    match copper_dsn::read_board(
        file,
        None,
        path.file_name().and_then(std::ffi::OsStr::to_str),
        &copper_dsn::DsnReadOptions::default(),
    ) {
        copper_dsn::BoardReadResult::Success { board, .. }
        | copper_dsn::BoardReadResult::OutlineMissing { board, .. } => *board.expect("a board"),
        other => panic!("{} did not read: {other:?}", path.display()),
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
        copper_drc::DesignRulesChecker::new(&mut board_a)
            .get_all_violations()
            .len(),
        copper_drc::DesignRulesChecker::new(&mut board_b)
            .get_all_violations()
            .len()
    );
}

#[test]
#[cfg_attr(debug_assertions, ignore)]
fn the_fanout_only_mode_sets_max_passes_to_zero_and_leaves_the_callers_settings_untouched() {
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
fn a_job_deadline_during_fanout_returns_a_timed_out_board() {
    let mut board = load_test_board("tests/data/p9t13-multi-net-smd-pin.dsn");
    let mut settings = build_settings(&board, 10);
    settings.fanout.get_or_insert_with(Default::default).enabled = Some(true);
    settings.set_run_optimizer(true);
    let stop = RouterStop::with_deadline(-1);
    let mut sink = NoopProgressSink;

    let result = run_pipeline(
        &mut board,
        &settings,
        &stop,
        RouterBudget::disabled(),
        &mut sink,
    )
    .expect("a job deadline is a successful partial routing result");

    assert_eq!(result.router_state, TaskState::TimedOut);
    assert_eq!(result.optimizer_state, Some(TaskState::Idle));
    assert!(result.timed_out);
    assert!(result.fanout.expect("fanout ran").is_timed_out);
    assert!(board.changed_area.is_none());
}

#[test]
#[cfg_attr(debug_assertions, ignore)]
fn a_recording_sink_sees_the_stage_events_in_javas_order() {
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
    let mut board = load_board(RPI);
    let report = build_unrouted_report(&mut board);
    assert!(
        !report.is_empty() && !report.starts_with("  (no unrouted"),
        "rpi_splitter is unrouted on load; the report must list its incompletes, got: {report}"
    );

    let airline_count = copper_drc::DesignRulesChecker::new(&mut board)
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

#[test]
fn routing_honors_project_hole_clearance_attached_after_load() {
    let json = r#"{"layers":[{"name":"F.Cu"},{"name":"B.Cu"}],
      "netClasses":[{"name":"Default","traceWidth":0.2,"clearance":0.15,"viaDiameter":0.5,"viaDrill":0.3}],
      "nets":[{"id":1,"name":"N","className":"Default"}],
      "outline":{"corners":[{"x":0,"y":0},{"x":30,"y":0},{"x":30,"y":20},{"x":0,"y":20}]},
      "components":[
      {"reference":"A","position":{"x":10,"y":10},"pads":[{"name":"1","netName":"N","shape":"rect","size":{"x":1,"y":1},"layers":["F.Cu"]}]},
      {"reference":"B","position":{"x":20,"y":10},"pads":[{"name":"1","netName":"N","shape":"rect","size":{"x":1,"y":1},"layers":["F.Cu"]}]},
      {"reference":"H","position":{"x":15,"y":9.2},"pads":[{"name":"1","shape":"circle","size":{"x":1,"y":1},"drill":1,"nonPlated":true,"layers":["F.Cu","B.Cu"]}]}]}"#;
    let copper_dsn::BoardReadResult::Success {
        board: Some(mut board),
        ..
    } = copper_dsn::kicad::read_board(json, None)
    else {
        panic!("fixture import failed")
    };
    let mut settings = build_settings(&board, 1);
    settings.hole_clearance_um = Some(0.0);
    settings.set_layer_active(1, false);
    copper_router::pipeline::prepare_board(&mut board, &settings);
    board.rules.drc_constraints = Some(copper_board::DrcConstraints {
        hole_clearance: Some(5000),
        ..copper_board::DrcConstraints::default()
    });
    let result = run_pipeline(
        &mut board,
        &settings,
        &RouterStop::new(),
        RouterBudget::disabled(),
        &mut NoopProgressSink,
    )
    .unwrap();
    assert_eq!(result.router_passes_completed, 1);
    assert_eq!(board.rules.get_hole_clearance(), 5000);
    assert!(!board.get_traces().is_empty());
    let mut drc = copper_drc::DesignRulesChecker::new(&mut board);
    let violations = drc.get_all_violations();
    assert!(
        !violations
            .iter()
            .any(|v| v.kind == copper_drc::DrcViolationKind::HoleClearance),
        "{violations:?}"
    );
    assert!(drc.get_all_unconnected_items().is_empty());
}

#[test]
#[cfg_attr(debug_assertions, ignore)]
fn a_rules_file_board_edge_clearance_survives_the_project_floor_at_pipeline_entry() {
    let json = r#"{"layers":[{"name":"F.Cu"},{"name":"B.Cu"}],
      "netClasses":[{"name":"Default","traceWidth":0.2,"clearance":0.15,"viaDiameter":0.5,"viaDrill":0.3}],
      "nets":[{"id":1,"name":"N","className":"Default"}],
      "outline":{"corners":[{"x":0,"y":0},{"x":30,"y":0},{"x":30,"y":20},{"x":0,"y":20}]},
      "components":[
      {"reference":"A","position":{"x":10,"y":10},"pads":[{"name":"1","netName":"N","shape":"rect","size":{"x":1,"y":1},"layers":["F.Cu"]}]},
      {"reference":"B","position":{"x":20,"y":10},"pads":[{"name":"1","netName":"N","shape":"rect","size":{"x":1,"y":1},"layers":["F.Cu"]}]}]}"#;
    let copper_dsn::BoardReadResult::Success {
        board: Some(mut board),
        ..
    } = copper_dsn::kicad::read_board(json, None)
    else {
        panic!("fixture import failed")
    };
    let mut settings = build_settings(&board, 1);
    settings.copper_to_edge_clearance_um = Some(500.0);
    settings.set_layer_active(1, false);
    copper_router::pipeline::prepare_board(&mut board, &settings);
    let edge = board
        .rules
        .clearance_matrix
        .get_no(copper_board::BOARD_EDGE_CLEARANCE_CLASS_NAME)
        .unwrap();
    for class in 1..board.rules.clearance_matrix.get_class_count() {
        board
            .rules
            .clearance_matrix
            .set_value_on_all_layers(edge, class, 8000);
        board
            .rules
            .clearance_matrix
            .set_value_on_all_layers(class, edge, 8000);
    }
    board.rules.drc_constraints = Some(copper_board::DrcConstraints {
        copper_edge_clearance: Some(5000),
        ..copper_board::DrcConstraints::default()
    });
    run_pipeline(
        &mut board,
        &settings,
        &RouterStop::new(),
        RouterBudget::disabled(),
        &mut NoopProgressSink,
    )
    .unwrap();
    assert_eq!(
        board.rules.clearance_matrix.get_value(edge, 1, 0, false),
        8000,
        "a board-edge clearance above the project minimum is kept, not rewritten"
    );
}
