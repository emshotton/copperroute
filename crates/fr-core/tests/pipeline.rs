//! Plan 8 Task 0: [`RoutingPipeline::run`] — the wrapper's two properties.
//!
//! * [`a_recording_sink_changes_no_board_byte`] — plan-7 ruling 11, re-run through the **whole**
//!   pipeline rather than through the router alone. Plan 7 pinned it for `run_pipeline`
//!   (`crates/fr-router/tests/stop_and_progress.rs`); this crate adds a DRC pass and an
//!   unrouted-report pass after it, both of which take `&mut Board`, so the claim has to be
//!   re-made at this level or the two new passes are unpinned.
//! * [`routing_result_carries_the_drc_violations_and_the_incompletes`] — the wrapper composes and
//!   does not decide: the three additions are present, the statistics are the pipeline's own, and
//!   nothing is recomputed.

use std::sync::{Arc, Mutex};

use fr_core::{Ctx, RoutingEvent, RoutingPipeline, RoutingResult, SyncProgressSink};
use fr_dsn::{BoardReadResult, CoordinateTransform, DsnReadOptions};
use fr_settings::sources::{CliSettings, DsnFileSettings, EnvironmentVariablesSource};
use fr_settings::{HostEnvironment, SettingsInputs, SettingsSource, resolve_headless};

/// `batch_parity`'s first CI stem, with the `-mp` its `tests/reference/router-fixtures.txt` row
/// records.
const DSN: &str = "fixtures/Issue143-rpi_splitter.dsn";
const MAX_PASSES: i32 = 8;

/// Plan-7 ruling 11 at the `fr-core` level: **no port decision reads the sink**.
///
/// Java's headless path runs with all three of `NamedAlgorithm`'s listener lists empty
/// (`autoroute/pipeline/NamedAlgorithm.java:26-31`), so a board routed with a recording sink must
/// be byte-for-byte the board routed with a no-op one. The comparison is the **SES bytes**, not a
/// hash: that is the artefact the CLI writes and `batch_parity`'s rung (c) compares, so a
/// divergence this test cannot see is one no user can see either.
#[test]
fn a_recording_sink_changes_no_board_byte() {
    if !parity::require_java_dir() {
        return;
    }

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

/// The wrapper's three additions over `run_pipeline`, and the one thing it must **not** do.
#[test]
fn routing_result_carries_the_drc_violations_and_the_incompletes() {
    if !parity::require_java_dir() {
        return;
    }
    let run = route(&SyncProgressSink::noop());
    let result = &run.result;

    // Addition 1: the unrouted report — Plan 7's `build_unrouted_report`, verbatim.
    assert!(
        !result.unrouted_report.is_empty(),
        "build_unrouted_report always answers a header, even for a fully routed board"
    );

    // Addition 2: the DRC violations of the FINAL board, from `fr_drc`.
    assert_eq!(result.violation_count(), result.drc_violations.len());
    // …and they agree with the statistics the pipeline itself computed, which is the cross-check
    // that the checker ran on the routed board rather than on a stale one.
    assert_eq!(
        result.stats.clearance_violations.total_count,
        Some(result.violation_count() as i32),
        "the DRC pass and BoardStatistics disagree about the same board"
    );

    // Addition 3: the CancelToken adaptation — an uncancelled run is not timed out.
    assert!(!result.timed_out);

    // The NON-addition: `stats` IS `pipeline.final_statistics`, not a second computation.
    // `BoardStatistics`' constructor runs `DesignRulesChecker` twice
    // (`core/scoring/BoardStatistics.java:265-268`, `:338-341`), so recomputing would be
    // observable through Plan 5's memoisation, not merely wasteful.
    assert_eq!(result.stats, result.pipeline.final_statistics);

    // Scan ruling R4: the incomplete COUNT comes from the statistics, not from a parallel
    // port-only structure over the report text.
    assert_eq!(
        result.incomplete_count(),
        result
            .pipeline
            .final_statistics
            .connections
            .incomplete_count
    );

    // And the router actually ran, so the assertions above are about a routed board.
    assert!(result.pipeline.passes_run > 0);
}

/// What one run answers: the report, and the SES bytes a `-de/-do` run would have written.
struct Run {
    result: RoutingResult,
    ses: Vec<u8>,
}

/// `batch_parity::route_stem`'s load-and-resolve sequence (controller ruling AW), through
/// [`RoutingPipeline::run`] rather than through `run_pipeline` directly.
fn route(sink: &SyncProgressSink) -> Run {
    let dsn = parity::java_dir().join(DSN);
    let bytes =
        std::fs::read(&dsn).unwrap_or_else(|e| panic!("cannot read {}: {e}", dsn.display()));
    let file_name = dsn
        .file_name()
        .expect("a file name")
        .to_string_lossy()
        .into_owned();
    // `RoutingJob.setInputFromFile:432`/`:457` — the SES header carries the base name.
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
        dsn: dsn_source.get_settings(),
        cli_rules: None,
        scheduler_rules: None,
        env: env_source.get_settings(),
        cli: cli_source.get_settings(),
    };
    let settings = resolve_headless(&inputs, Some(&board), &HostEnvironment::detect());
    fr_core::prepare_board(&mut board, &settings);

    let ctx = Ctx::with_disabled_budget(&settings, sink);
    let result = RoutingPipeline::run(&mut board, &ctx).expect("the stem has a routable layer");

    let mut ses = Vec::new();
    fr_dsn::ses_writer::write(&board, &transform, &mut ses, &design_name)
        .expect("the SES writer never fails on a board it just routed");

    Run { result, ses }
}

fn read_board(dsn: &std::path::Path, design_name: &str) -> (fr_board::Board, CoordinateTransform) {
    let file = std::fs::File::open(dsn).unwrap_or_else(|e| panic!("cannot open {dsn:?}: {e}"));
    match fr_dsn::read_board(file, None, Some(design_name), &DsnReadOptions::default()) {
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
