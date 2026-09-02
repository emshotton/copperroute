//! Plan 8 Task 0: [`CancelToken`] and its [`RouterStop`] mapping.
//!
//! # What is pinned here, and what is not, and why
//!
//! The brief asks for `a_cancel_from_a_second_thread_stops_a_run_mid_pass`: cancel after the
//! first `RoutingEvent::BoardSnapshot` and assert `passes_run` fell. **That test needs the
//! `fr-router` poll seam, which controller ruling BB assigns to Task 11** — the seam's first
//! consumer, whose own `a_cancelled_tool_stops_mid_flight` is this same assertion one layer up
//! (Task 12's `cancelling_route_board_mid_run_…` is the second consumer). (Scan ruling R3:
//! the committed tree has exactly one `poll_deadline` site, `RouterStop` is `Cell`-based with
//! private fields, and `run_pipeline` offers no closure hook, so ruling AP's "add a poll, never a
//! lock" escape applies and the *addition* is an additive-and-wrapped `fr-router` change).
//! Until **Task 11** lands it, a cancel that arrives **after** `run_pipeline` is entered is not
//! observed by that run.
//!
//! So this file pins everything that does not depend on the seam, and pins it against a **real
//! routed board** rather than against a constructed `RouterStop`:
//!
//! * [`a_cancel_from_a_second_thread_stops_a_run`] — the token really is shared across a thread
//!   boundary, and a run that starts with it cancelled really does route nothing. This is the
//!   brief's test minus the word "mid-pass"; **the mid-pass half is Task 11's** (ruling BB).
//! * [`cancel_all_and_cancel_auto_router_are_distinct`] — plan ruling 2's whole point.
//! * [`an_uncancelled_token_is_a_no_op`] — what keeps `batch_parity` byte-identical.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use fr_core::{CancelToken, Ctx, Deadline, RoutingPipeline, StopRequestState, SyncProgressSink};
use fr_dsn::{BoardReadResult, DsnReadOptions};
use fr_settings::sources::{CliSettings, DsnFileSettings, EnvironmentVariablesSource};
use fr_settings::{HostEnvironment, SettingsInputs, SettingsSource, resolve_headless};

/// The CI-lane batch stem, the smallest board that routes more than one pass.
///
/// The brief names `router-dac2020-bm01`; that stem is `batch_parity`'s **slow** lane
/// (`FR_SLOW_PARITY=1` + `#[cfg_attr(debug_assertions, ignore)]`, plan-7 ruling AM as amended by
/// its scan ruling 13), and a cancellation property does not need a big board to be true. This
/// file uses `router-rpi-splitter`, which is `batch_parity`'s first CI stem, with the same `-mp 8`
/// cap `tests/reference/router-fixtures.txt` records for it.
const DSN: &str = "fixtures/Issue143-rpi_splitter.dsn";
/// `router-rpi-splitter`'s `-mp`, from `tests/reference/router-fixtures.txt`.
const MAX_PASSES: i32 = 8;

/// Plan ruling 2: the token preserves Java's **three**-state stop.
///
/// `StoppableThread.requestStop()` sets `ALL` unconditionally (`core/StoppableThread.java:23-25`);
/// `requestStopAutoRouter()` upgrades `NONE -> AUTO_ROUTER_ONLY` and **only** that (`:33-37`).
/// The difference is quirk #200/#202: `RoutingPipeline.java:117` gates the optimizer stage on
/// `isStopRequested()`, i.e. on `ALL` alone, so an `AUTO_ROUTER_ONLY` stop leaves the optimizer
/// running and an `ALL` stop does not. A single `AtomicBool` would collapse the two.
#[test]
fn cancel_all_and_cancel_auto_router_are_distinct() {
    let all = CancelToken::new();
    all.cancel();
    assert_eq!(all.as_router_stop().state(), StopRequestState::All);

    let auto = CancelToken::new();
    auto.cancel_auto_router();
    assert_eq!(
        auto.as_router_stop().state(),
        StopRequestState::AutoRouterOnly
    );

    // …and the two readers disagree exactly where Java's do.
    let auto_stop = auto.as_router_stop();
    assert!(
        auto_stop.is_stop_auto_router_requested(),
        "AutorouteBatchLoop.java:250 stops the router"
    );
    assert!(
        !auto_stop.is_stop_requested(),
        "RoutingPipeline.java:117 still runs the optimizer — quirk #202"
    );

    // `requestStopAutoRouter` from `ALL` is a no-op (`:33-37`), which is what makes
    // `AutorouteBatchLoop.java:251-253` dead code (quirk #203). Order must not change the answer.
    let both = CancelToken::new();
    both.cancel();
    both.cancel_auto_router();
    assert_eq!(both.as_router_stop().state(), StopRequestState::All);
}

/// The property every existing parity driver depends on: a token nobody cancelled produces a
/// `RouterStop` indistinguishable from `RouterStop::new()`, and applying it to a live stop writes
/// nothing.
///
/// This is what makes Task 11's added polls invisible to `batch_parity`, `p6t1` and
/// `sweep-p7t9.sh`.
#[test]
fn an_uncancelled_token_is_a_no_op() {
    let token = CancelToken::new();
    let stop = token.as_router_stop();
    assert_eq!(stop.state(), StopRequestState::None);
    assert!(!stop.is_timed_out());
    assert!(
        !stop.poll_deadline(),
        "no deadline means poll_deadline is a constant false"
    );

    // The seam's own call, on a stop that is already mid-run: it must not move the state.
    for _ in 0..1000 {
        token.apply_to(&stop);
    }
    assert_eq!(stop.state(), StopRequestState::None);
    assert!(!stop.is_timed_out());

    // And once cancelled, the same call does move it — so the assertion above is about the
    // token's state, not about `apply_to` being inert.
    token.cancel();
    token.apply_to(&stop);
    assert_eq!(stop.state(), StopRequestState::All);
}

/// The token crosses a thread boundary and a run that sees it cancelled routes nothing.
///
/// The cancel is raised on a second thread and read by the routing thread through the shared
/// `Arc<AtomicBool>`; the routed board is compared against an identical uncancelled run.
///
/// **`passes_run == 0`, not "fewer passes"**: the cancel lands before `run_pipeline`'s first
/// stage guard (`RoutingPipeline.java:97`'s `isStopAutoRouterRequested`), so the routing stage
/// never starts — which is precisely Java's answer for a job cancelled before it began. The
/// mid-pass case is **Task 11's** (controller ruling BB); see the module doc.
#[test]
fn a_cancel_from_a_second_thread_stops_a_run() {
    if !parity::require_java_dir() {
        return;
    }

    let uncancelled = route(&CancelToken::new());
    assert!(
        uncancelled.passes_run > 0,
        "the uncancelled control routed no passes at all, so it proves nothing"
    );

    let token = CancelToken::new();
    let raiser = token.clone();
    std::thread::spawn(move || raiser.cancel())
        .join()
        .expect("the cancelling thread panicked");
    assert!(
        token.is_cancelled(),
        "the flag must be visible on this thread — it is one Arc<AtomicBool>, not two"
    );

    let cancelled = route(&token);
    assert_eq!(
        cancelled.passes_run, 0,
        "a run entered with the stop already at ALL never starts the routing stage \
         (RoutingPipeline.java:97)"
    );
    assert!(
        cancelled.passes_run < uncancelled.passes_run,
        "the cancel must reduce the work done"
    );
    assert!(
        cancelled.incompletes >= uncancelled.incompletes,
        "a cancelled run cannot have routed more connections than the full one"
    );
}

/// The deadline reaches the `RouterStop` as a span, and an already-expired one is exceeded on the
/// first poll — `fr_board::TimeLimit`'s `elapsed > limit` with a negative limit
/// (`TimeLimit.java:18-21`).
#[test]
fn an_expired_deadline_reaches_the_router_stop() {
    let expired = CancelToken::with_deadline(Deadline::in_seconds(-1));
    let stop = expired.as_router_stop();
    assert!(
        stop.poll_deadline(),
        "an expired job deadline must trip on the first poll"
    );
    // `poll_deadline` performs the monitor thread's two writes at once
    // (`RoutingJobSchedulerActionThread.java:75` and `:84`).
    assert_eq!(stop.state(), StopRequestState::All);
    assert!(stop.is_timed_out());

    // An unexpired one is invisible — ruling AI's determinism claim.
    let live = CancelToken::with_timeout(std::time::Duration::from_secs(3600));
    let live_stop = live.as_router_stop();
    assert!(!live_stop.poll_deadline());
    assert_eq!(live_stop.state(), StopRequestState::None);
}

/// The `SyncProgressSink` really is called during a run — the precondition of Task 11's
/// mid-pass test, asserted here so that task inherits a working sink rather than debugging two
/// things at once.
#[test]
fn a_sync_sink_sees_the_runs_events() {
    if !parity::require_java_dir() {
        return;
    }
    let seen = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&seen);
    let sink = SyncProgressSink::new(move |_| {
        counter.fetch_add(1, Ordering::SeqCst);
    });
    route_with(&CancelToken::new(), &sink);
    assert!(
        seen.load(Ordering::SeqCst) > 0,
        "the pipeline fired no events through a SyncProgressSink"
    );
}

/// What one run answers.
struct Run {
    passes_run: i32,
    incompletes: Option<i32>,
}

fn route(token: &CancelToken) -> Run {
    route_with(token, &SyncProgressSink::noop())
}

/// `batch_parity::route_stem`'s load-and-resolve sequence (controller ruling AW), reduced to the
/// two numbers this file compares: `resolve_headless`'s ladder, then
/// `fr_router::pipeline::prepare_board`, then [`RoutingPipeline::run`].
fn route_with(token: &CancelToken, sink: &SyncProgressSink) -> Run {
    let dsn = parity::java_dir().join(DSN);
    let bytes =
        std::fs::read(&dsn).unwrap_or_else(|e| panic!("cannot read {}: {e}", dsn.display()));
    let file_name = dsn
        .file_name()
        .expect("a file name")
        .to_string_lossy()
        .into_owned();

    let file = std::fs::File::open(&dsn).unwrap_or_else(|e| panic!("cannot open {dsn:?}: {e}"));
    let mut board =
        match fr_dsn::read_board(file, None, Some(&file_name), &DsnReadOptions::default()) {
            BoardReadResult::Success { board, .. }
            | BoardReadResult::OutlineMissing { board, .. } => {
                *board.expect("the stem produces a board")
            }
            other => panic!("{file_name} did not read: {other:?}"),
        };

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
    // `HeadlessBoardManager.java:746-747`, ruling AW.
    fr_core::prepare_board(&mut board, &settings);

    let ctx = Ctx {
        settings: &settings,
        cancel: token.clone(),
        progress: sink,
        // Ruling AI's wall clock off, on this side, exactly as every parity driver runs.
        budget: fr_core::RouterBudget::disabled(),
    };
    let result = RoutingPipeline::run(&mut board, &ctx).expect("the stem has a routable layer");
    Run {
        passes_run: result.pipeline.passes_run,
        incompletes: result.incomplete_count(),
    }
}
