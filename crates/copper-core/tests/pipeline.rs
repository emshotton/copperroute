use std::sync::{Arc, Mutex};

use copper_core::{Ctx, RoutingEvent, RoutingPipeline, RoutingResult, SyncProgressSink};
use copper_dsn::{BoardReadResult, CoordinateTransform, DsnReadOptions};
use copper_settings::sources::{CliSettings, DsnFileSettings, EnvironmentVariablesSource};
use copper_settings::{HostEnvironment, SettingsInputs, SettingsSource, resolve_headless};

const DSN: &str = "fixtures/Issue143-rpi_splitter.dsn";
const MAX_PASSES: i32 = 8;

#[test]
fn a_recording_sink_changes_no_board_byte() {
    let quiet = route(&SyncProgressSink::noop());

    let events: Arc<Mutex<Vec<RoutingEvent>>> = Arc::new(Mutex::new(Vec::new()));
    let recorder = Arc::clone(&events);
    let loud = route(&SyncProgressSink::new(move |event| {
        recorder
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push(event.clone());
    }));

    assert_eq!(
        quiet.ses, loud.ses,
        "a recording SyncProgressSink moved a board byte (plan-7 ruling 11)"
    );
    assert_eq!(quiet.result.unrouted_report, loud.result.unrouted_report);
    assert_eq!(
        quiet.result.violation_count(),
        loud.result.violation_count()
    );
    assert_eq!(
        quiet.result.incomplete_count(),
        loud.result.incomplete_count()
    );

    let events = events.lock().unwrap_or_else(|p| p.into_inner());
    assert!(
        !events.is_empty(),
        "the recording sink saw no events at all, so it proves nothing"
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, RoutingEvent::BoardUpdated { .. })),
        "a whole-board run fires at least one BoardUpdated"
    );
}

#[test]
fn the_wrapper_is_transparent_to_the_ses_bytes() {
    // Nominal clearance deliberately changes the JVM route; wrapper transparency
    // must compare the same routing algorithm with and without result collection.
    route_impl(&SyncProgressSink::noop(), true);
}

#[test]
fn routing_result_carries_the_drc_violations_and_the_incompletes() {
    let run = route(&SyncProgressSink::noop());
    let result = &run.result;

    assert!(
        !result.unrouted_report.is_empty(),
        "build_unrouted_report always answers a header, even for a fully routed board"
    );

    assert_eq!(result.violation_count(), result.drc_violations.len());
    let routing_involved = result
        .drc_violations
        .iter()
        .filter(|violation| violation.involves_routing(&run.board))
        .count();
    assert_eq!(
        result.stats.clearance_violations.total_count,
        Some(routing_involved as i32),
        "BoardStatistics counts the routing-involved subset of the DRC pass"
    );

    assert!(!result.timed_out);

    assert_eq!(result.stats, result.pipeline.final_statistics);

    assert_eq!(
        result.incomplete_count(),
        result
            .pipeline
            .final_statistics
            .connections
            .incomplete_count
    );

    assert!(result.pipeline.router_passes_completed > 0);
}

struct Run {
    result: RoutingResult,
    board: copper_board::Board,
    ses: String,
}

fn route(sink: &SyncProgressSink) -> Run {
    route_impl(sink, false)
}

fn route_impl(sink: &SyncProgressSink, check_wrapper: bool) -> Run {
    let dsn = testkit::corpus_dir().join(DSN);
    let bytes =
        std::fs::read(&dsn).unwrap_or_else(|e| panic!("cannot read {}: {e}", dsn.display()));
    let file_name = dsn
        .file_name()
        .expect("a file name")
        .to_string_lossy()
        .into_owned();
    let design_name = file_name
        .rsplit_once('.')
        .map_or_else(|| file_name.clone(), |(base, _)| base.to_string());

    let (mut board, transform) = read_board(&dsn, &file_name);

    let argv = vec![
        "-de".to_string(),
        dsn.display().to_string(),
        "-do".to_string(),
        dsn.display().to_string().replace(".dsn", ".ses"),
        "-mp".to_string(),
        MAX_PASSES.to_string(),
        "--router.fanout.enabled=true".to_string(),
        "--router.optimizer.enabled=true".to_string(),
    ];
    let dsn_source = DsnFileSettings::new(&bytes[..], &file_name);
    let env_map: std::collections::BTreeMap<String, String> = std::env::vars().collect();
    let env_source = EnvironmentVariablesSource::new(&env_map);
    let cli_source = CliSettings::new(&argv);
    let inputs = SettingsInputs {
        json_file: None,
        dsn: dsn_source.get_settings(),
        cli_rules: None,
        scheduler_rules: None,
        env: env_source.get_settings(),
        cli: cli_source.get_settings(),
    };
    let settings = resolve_headless(&inputs, Some(&board), &HostEnvironment::detect());
    copper_core::prepare_board(&mut board, &settings);

    let direct_ses = if check_wrapper {
        let mut direct = board.clone();
        let stop = copper_router::pipeline::RouterStop::new();
        let quiet = SyncProgressSink::noop();
        copper_router::pipeline::run_pipeline(
            &mut direct,
            &settings,
            &stop,
            copper_router::pipeline::RouterBudget::disabled(),
            &mut quiet.as_pipeline_sink(),
        )
        .expect("direct pipeline has a routable layer");
        let mut bytes = Vec::new();
        copper_dsn::ses_writer::write(&direct, &transform, &mut bytes, &design_name)
            .expect("direct pipeline session is writable");
        Some(bytes)
    } else {
        None
    };

    let ctx = Ctx::with_disabled_budget(&settings, sink);
    let result = RoutingPipeline::run(&mut board, &ctx).expect("the stem has a routable layer");

    let mut ses = Vec::new();
    copper_dsn::ses_writer::write(&board, &transform, &mut ses, &design_name)
        .expect("the SES writer never fails on a board it just routed");
    if let Some(expected) = direct_ses {
        assert_eq!(
            ses, expected,
            "the core wrapper changes the direct router's session"
        );
    }
    let ses = String::from_utf8(ses).expect("the SES writer emits UTF-8");

    Run { result, board, ses }
}

fn read_board(
    dsn: &std::path::Path,
    design_name: &str,
) -> (copper_board::Board, CoordinateTransform) {
    let file = std::fs::File::open(dsn).unwrap_or_else(|e| panic!("cannot open {dsn:?}: {e}"));
    match copper_dsn::read_board(file, None, Some(design_name), &DsnReadOptions::default()) {
        BoardReadResult::Success {
            board,
            coordinate_transform,
            ..
        }
        | BoardReadResult::OutlineMissing {
            board,
            coordinate_transform,
            ..
        } => (
            *board.expect("the stem produces a board"),
            coordinate_transform.expect("the stem produces a coordinate transform"),
        ),
        other => panic!("{design_name} did not read: {other:?}"),
    }
}
