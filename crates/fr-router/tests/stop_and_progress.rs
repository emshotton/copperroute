use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

use fr_board::prelude::*;
use fr_dsn::{BoardReadResult, DsnReadOptions};
use fr_router::pipeline::{
    NamedAlgorithmType, NoopProgressSink, ProgressSink, ProgressThrottler, RouterBudget,
    RouterCounters, RouterStop, RoutingEvent, StopRequestState, TaskState, run_pipeline,
};
use fr_router::route_connection;
use fr_settings::sources::DefaultSettings;
use fr_settings::{HostEnvironment, RouterSettings, SettingsSource};

const TRANSCRIPT: &str = include_str!("data/p7t4-stop-and-counters.txt");

fn section(name: &str) -> Vec<&'static str> {
    let mut out = Vec::new();
    let mut inside = false;
    for line in TRANSCRIPT.lines() {
        if line.starts_with('[') {
            inside = line == format!("[{name}]");
            continue;
        }
        if inside && !line.starts_with('#') && !line.is_empty() {
            out.push(line);
        }
    }
    assert!(!out.is_empty(), "section [{name}] is empty or missing");
    out
}

fn field<'a>(line: &'a str, key: &str) -> &'a str {
    for token in line.split_whitespace() {
        if let Some(value) = token.strip_prefix(&format!("{key}=")) {
            return value;
        }
    }
    panic!("no `{key}=` in `{line}`");
}

fn parse_state(name: &str) -> StopRequestState {
    match name {
        "NONE" => StopRequestState::None,
        "AUTO_ROUTER_ONLY" => StopRequestState::AutoRouterOnly,
        "ALL" => StopRequestState::All,
        other => panic!("unknown StopRequestState `{other}`"),
    }
}

fn stop_in(state: StopRequestState) -> RouterStop {
    let stop = RouterStop::new();
    match state {
        StopRequestState::None => {}
        StopRequestState::AutoRouterOnly => stop.request_stop_auto_router(),
        StopRequestState::All => stop.request_stop(),
    }
    assert_eq!(stop.state(), state, "cannot reach {state:?}");
    stop
}

#[test]
fn the_two_queries_match_the_jvm_in_every_state() {
    for line in section("queries-at-rest") {
        let state = parse_state(field(line, "state"));
        let stop = stop_in(state);
        assert_eq!(
            stop.is_stop_requested().to_string(),
            field(line, "isStopRequested"),
            "isStopRequested in {state:?} (StoppableThread.java:28-30): `{line}`"
        );
        assert_eq!(
            stop.is_stop_auto_router_requested().to_string(),
            field(line, "isStopAutoRouterRequested"),
            "isStopAutoRouterRequested in {state:?} (StoppableThread.java:40-42): `{line}`"
        );
    }
}

#[test]
fn the_transition_table_matches_the_jvm() {
    let lines = section("transitions");
    assert_eq!(lines.len(), 6, "3 states x 2 requests");
    for line in lines {
        let from = parse_state(field(line, "from"));
        let stop = stop_in(from);
        match field(line, "request") {
            "requestStop" => stop.request_stop(),
            "requestStopAutoRouter" => stop.request_stop_auto_router(),
            other => panic!("unknown request `{other}`"),
        }
        assert_eq!(
            stop.state(),
            parse_state(field(line, "state")),
            "state after: `{line}`"
        );
        assert_eq!(
            stop.is_stop_requested().to_string(),
            field(line, "isStopRequested"),
            "isStopRequested after: `{line}`"
        );
        assert_eq!(
            stop.is_stop_auto_router_requested().to_string(),
            field(line, "isStopAutoRouterRequested"),
            "isStopAutoRouterRequested after: `{line}`"
        );
    }
}

#[test]
fn request_stop_auto_router_does_not_downgrade_all() {
    let row = section("sequences")
        .into_iter()
        .find(|l| field(l, "seq") == "monitorTimeout")
        .expect("seq=monitorTimeout");
    assert_eq!(
        field(row, "secondWasNoOp"),
        "true",
        "the JVM's word: `{row}`"
    );

    let stop = RouterStop::new();
    stop.request_stop();
    assert_eq!(stop.state(), parse_state(field(row, "stateAfterFirst")));
    stop.request_stop_auto_router();
    assert_eq!(
        stop.state(),
        parse_state(field(row, "stateAfterSecond")),
        "requestStopAutoRouter must not downgrade ALL (StoppableThread.java:33-37)"
    );
    assert!(stop.is_stop_requested());
}

fn optimizer_stage_would_run(stop: &RouterStop) -> bool {
    !stop.is_stop_requested()
}

#[test]
fn max_items_stops_all_and_max_passes_stops_the_router_only() {
    let rows = section("sequences");
    let max_items_row = rows
        .iter()
        .find(|l| field(l, "seq") == "maxItems")
        .expect("seq=maxItems");
    let max_passes_row = rows
        .iter()
        .find(|l| field(l, "seq") == "maxPasses")
        .expect("seq=maxPasses");

    let by_items = RouterStop::new();
    by_items.request_stop();
    assert_eq!(by_items.state(), parse_state(field(max_items_row, "state")));
    assert_eq!(by_items.state(), StopRequestState::All);
    assert!(
        by_items.is_stop_auto_router_requested(),
        "the pass loop stops"
    );
    assert!(
        !optimizer_stage_would_run(&by_items),
        "quirk #202 on the JVM: hitting --max-items silently disables the optimizer stage \
         (RoutingPipeline.java:117 reads isStopRequested, which is ALL-only). The **port's** own \
         maxItems site no longer writes ALL — fixed: T9 (#202) — and \
         `max_items_optimises_like_max_passes` below is the consequence; this test is the jar's \
         answer, kept on record"
    );
    assert_eq!(
        field(max_items_row, "optimizerStageWouldRun"),
        "false",
        "the JVM's word: `{max_items_row}`"
    );

    let by_passes = RouterStop::new();
    by_passes.request_stop_auto_router();
    assert_eq!(
        by_passes.state(),
        parse_state(field(max_passes_row, "state"))
    );
    assert_eq!(by_passes.state(), StopRequestState::AutoRouterOnly);
    assert!(
        by_passes.is_stop_auto_router_requested(),
        "the pass loop stops"
    );
    assert!(
        optimizer_stage_would_run(&by_passes),
        "--max-passes leaves the optimizer stage enabled"
    );
    assert_eq!(
        field(max_passes_row, "optimizerStageWouldRun"),
        "true",
        "--max-passes leaves the optimizer stage enabled"
    );
}

#[test]
#[cfg_attr(debug_assertions, ignore)]
fn max_items_optimises_like_max_passes() {
    if !parity::require_java_dir() {
        return;
    }

    fn route(max_items: Option<i32>, max_passes: i32) -> (StopRequestState, TaskState, u64) {
        let path = parity::java_dir().join("fixtures/Issue143-rpi_splitter.dsn");
        let file = std::fs::File::open(&path)
            .unwrap_or_else(|e| panic!("cannot open {}: {e}", path.display()));
        let mut board = match fr_dsn::read_board(
            file,
            None,
            Some("Issue143-rpi_splitter.dsn"),
            &DsnReadOptions::default(),
        ) {
            BoardReadResult::Success { board, .. }
            | BoardReadResult::OutlineMissing { board, .. } => *board.expect("a board"),
            other => panic!("did not read: {other:?}"),
        };
        let mut settings = build_settings(&board);
        settings.max_passes = Some(max_passes);
        settings.max_items = max_items;
        settings.fanout.get_or_insert_with(Default::default).enabled = Some(false);
        settings.set_run_router(true);
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
        (
            stop.state(),
            result
                .optimizer_state
                .expect("run_optimizer is on, so the stage is configured"),
            board.structural_hash(),
        )
    }

    let (by_items_flag, by_items_state, by_items_hash) = route(Some(2), 8);
    let (by_passes_flag, by_passes_state, _) = route(None, 1);

    assert_ne!(by_items_flag, StopRequestState::All);
    assert_ne!(by_passes_flag, StopRequestState::All);
    assert_eq!(by_passes_flag, StopRequestState::None);

    assert_ne!(by_items_state, TaskState::Idle);
    assert_ne!(by_passes_state, TaskState::Idle);

    let unoptimised = {
        let path = parity::java_dir().join("fixtures/Issue143-rpi_splitter.dsn");
        let file = std::fs::File::open(&path).expect("cannot open the fixture");
        let mut board = match fr_dsn::read_board(
            file,
            None,
            Some("Issue143-rpi_splitter.dsn"),
            &DsnReadOptions::default(),
        ) {
            BoardReadResult::Success { board, .. }
            | BoardReadResult::OutlineMissing { board, .. } => *board.expect("a board"),
            other => panic!("did not read: {other:?}"),
        };
        let mut settings = build_settings(&board);
        settings.max_passes = Some(8);
        settings.max_items = Some(2);
        settings.fanout.get_or_insert_with(Default::default).enabled = Some(false);
        settings.set_run_router(true);
        settings.set_run_optimizer(false);
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
        board.structural_hash()
    };
    assert_ne!(
        by_items_hash, unoptimised,
        "a --max-items run must write an OPTIMISED board — that is the whole of quirk #202, and \
         it is only observable because #227 landed with it"
    );
}

#[test]
fn the_deadline_requests_stop_all_like_the_monitor_thread() {
    let stop = RouterStop::with_deadline(0);
    assert_eq!(stop.state(), StopRequestState::None, "not polled yet");
    assert!(!stop.is_timed_out(), "not polled yet");

    let spun = spin_until(|| stop.poll_deadline());
    assert!(spun, "a 0 ms deadline must expire within the spin bound");
    assert_eq!(
        stop.state(),
        StopRequestState::All,
        "RoutingJobSchedulerActionThread.java:75 calls requestStop(), which is ALL"
    );
    assert!(stop.is_stop_requested());
    assert!(
        stop.is_timed_out(),
        "AutorouteBatchLoop.java:578-584 reports TaskState::TimedOut off this flag"
    );
    assert!(stop.poll_deadline());
    assert_eq!(stop.state(), StopRequestState::All);
}

#[test]
fn a_stop_without_a_deadline_never_times_out() {
    let stop = RouterStop::new();
    for _ in 0..1000 {
        assert!(!stop.poll_deadline());
    }
    assert_eq!(stop.state(), StopRequestState::None);
    assert!(!stop.is_timed_out());
}

#[test]
fn an_unexpired_deadline_is_invisible() {
    if !parity::require_java_dir() {
        return;
    }
    let without = route_mini_pass(&RouterStop::new(), &mut NoopProgressSink);
    let far_future = RouterStop::with_deadline(3_600_000);
    let with = route_mini_pass(&far_future, &mut NoopProgressSink);

    assert!(!far_future.is_timed_out(), "an hour is not reached");
    assert_eq!(far_future.state(), StopRequestState::None);
    assert_eq!(
        without.hash, with.hash,
        "an unexpired deadline moved a board byte"
    );
    assert_eq!(without.routed, with.routed);
}

#[derive(Default)]
struct RecordingSink {
    events: Vec<RoutingEvent>,
}

impl ProgressSink for RecordingSink {
    fn on_event(&mut self, event: &RoutingEvent) {
        self.events.push(event.clone());
    }
}

#[test]
fn a_recording_sink_changes_no_board_byte() {
    if !parity::require_java_dir() {
        return;
    }
    let quiet = route_mini_pass(&RouterStop::new(), &mut NoopProgressSink);

    let mut recorder = RecordingSink::default();
    let loud = route_mini_pass(&RouterStop::new(), &mut recorder);

    assert_eq!(
        quiet.hash, loud.hash,
        "a recording ProgressSink moved a board byte (ruling 11)"
    );
    assert!(
        !recorder.events.is_empty(),
        "the recording sink saw no events at all, so it proves nothing"
    );
    assert!(
        recorder
            .events
            .iter()
            .any(|e| matches!(e, RoutingEvent::BoardUpdated { .. })),
        "the mini pass fires at least one BoardUpdated"
    );
    assert!(matches!(
        recorder.events.first(),
        Some(RoutingEvent::TaskStateChanged {
            algorithm: NamedAlgorithmType::Router,
            state: TaskState::Started
        })
    ));
    assert!(matches!(
        recorder.events.last(),
        Some(RoutingEvent::TaskStateChanged {
            algorithm: NamedAlgorithmType::Router,
            state: TaskState::Finished
        })
    ));
}

#[test]
fn the_noop_sink_swallows_every_event() {
    let mut sink = NoopProgressSink;
    for event in every_event_shape() {
        sink.on_event(&event);
    }
}

fn every_event_shape() -> Vec<RoutingEvent> {
    let shapes = vec![
        RoutingEvent::TaskStateChanged {
            algorithm: NamedAlgorithmType::Router,
            state: TaskState::Running,
        },
        RoutingEvent::BoardUpdated {
            counters: RouterCounters::default(),
        },
        RoutingEvent::BoardSnapshot { pass: 3 },
        RoutingEvent::FanoutProgress {
            pass: 1,
            routed: 2,
            pins_to_go: 3,
        },
        RoutingEvent::OptimizerImproved {
            item: ItemId(7),
            score_before: 1.0,
            score_after: 2.0,
        },
    ];
    for shape in &shapes {
        match shape {
            RoutingEvent::TaskStateChanged { .. }
            | RoutingEvent::BoardUpdated { .. }
            | RoutingEvent::BoardSnapshot { .. }
            | RoutingEvent::FanoutProgress { .. }
            | RoutingEvent::OptimizerImproved { .. } => {}
        }
    }
    shapes
}

#[test]
fn the_enum_variant_lists_match_the_jvm() {
    let lines = section("enums");

    let stop_states = variants(&lines, "StopRequestState");
    assert_eq!(stop_states, vec!["NONE", "AUTO_ROUTER_ONLY", "ALL"]);
    let ported = [
        StopRequestState::None,
        StopRequestState::AutoRouterOnly,
        StopRequestState::All,
    ];
    assert_eq!(ported.len(), stop_states.len());
    assert!(ported.windows(2).all(|w| w[0] < w[1]));

    let task_states = variants(&lines, "TaskState");
    assert_eq!(
        task_states,
        vec![
            "IDLE",
            "STARTED",
            "RUNNING",
            "FINISHED",
            "CANCELLED",
            "TIMED_OUT"
        ]
    );
    let ported = [
        TaskState::Idle,
        TaskState::Started,
        TaskState::Running,
        TaskState::Finished,
        TaskState::Cancelled,
        TaskState::TimedOut,
    ];
    assert_eq!(ported.len(), task_states.len(), "TaskState.java has 6");
    for (i, state) in ported.iter().enumerate() {
        assert_eq!(
            state.ordinal(),
            i as i32,
            "{state:?} is ordinal {i} in Java"
        );
    }

    let algorithm_types = variants(&lines, "NamedAlgorithmType");
    assert_eq!(algorithm_types, vec!["ROUTER", "OPTIMIZER"]);
    let ported = [NamedAlgorithmType::Router, NamedAlgorithmType::Optimizer];
    assert_eq!(ported.len(), algorithm_types.len());
    for (i, kind) in ported.iter().enumerate() {
        assert_eq!(kind.ordinal(), i as i32, "{kind:?} is ordinal {i} in Java");
    }
}

fn variants(lines: &[&str], name: &str) -> Vec<String> {
    let prefix = format!("{name} = ");
    let line = lines
        .iter()
        .find(|l| l.starts_with(&prefix))
        .unwrap_or_else(|| panic!("no `{name}` line in [enums]"));
    line[prefix.len()..]
        .split_whitespace()
        .enumerate()
        .map(|(i, token)| {
            let (variant, ordinal) = token
                .split_once('(')
                .unwrap_or_else(|| panic!("malformed `{token}`"));
            assert_eq!(
                ordinal.trim_end_matches(')'),
                i.to_string(),
                "ordinals must be dense and ascending"
            );
            variant.to_string()
        })
        .collect()
}

#[test]
fn router_counters_field_list_matches_java() {
    let lines = section("RouterCounters");
    let java: Vec<(String, String)> = lines
        .iter()
        .filter(|l| l.starts_with("field "))
        .map(|l| (field(l, "name").to_string(), field(l, "type").to_string()))
        .collect();
    assert_eq!(java.len(), 9, "RouterCounters.java declares nine fields");
    let count_line = lines
        .iter()
        .find(|l| l.starts_with("fieldCount"))
        .expect("fieldCount");
    assert_eq!(count_line, &"fieldCount = 9");

    assert_eq!(
        RouterCounters::JAVA_FIELD_NAMES.len(),
        java.len(),
        "the port declares {} fields, Java {}",
        RouterCounters::JAVA_FIELD_NAMES.len(),
        java.len()
    );
    for (i, (name, ty)) in java.iter().enumerate() {
        assert_eq!(
            RouterCounters::JAVA_FIELD_NAMES[i],
            name,
            "field {i} is `{name}` ({ty}) in declaration order"
        );
    }

    for line in lines.iter().filter(|l| l.starts_with("field ")) {
        assert_eq!(field(line, "default"), "null", "`{line}`");
    }
    let default = RouterCounters::default();
    assert_eq!(default.pass_count, None);
    assert_eq!(default.queued_to_be_routed_count, None);
    assert_eq!(default.routed_count, None);
    assert_eq!(default.skipped_count, None);
    assert_eq!(default.ripped_count, None);
    assert_eq!(default.failed_to_be_routed_count, None);
    assert_eq!(default.incomplete_count, None);
    assert_eq!(default.phase, None);
    assert_eq!(default.fanout_extra_vias_count, None);
}

#[test]
fn the_java_literal_budget_carries_javas_four_literals() {
    let budget = RouterBudget::java_literals();

    let reflected: Vec<i32> = section("constants")
        .iter()
        .map(|l| {
            l.rsplit_once(" = ")
                .expect("`… = <int>`")
                .1
                .parse()
                .expect("an int")
        })
        .collect();
    assert_eq!(reflected.len(), 4, "four TIME_LIMIT_… declarations");
    assert!(
        reflected.iter().all(|&v| v == budget.opt_changed_area_ms),
        "opt_changed_area_ms must be TIME_LIMIT_TO_PREVENT_ENDLESS_LOOP \
         (AutorouteConnectionRouter.java:22, BatchAutorouter.java:43, \
          BatchAutorouterThread.java:38, AutoroutePassRunner.java:32, and the two locals at \
          board/facade/RoutingBoard.java:961 and :1100); the probe reflected {reflected:?}"
    );
    assert_eq!(budget.opt_changed_area_ms, 1000);

    assert_eq!(
        budget.fanout_ms_per_pin, 10000,
        "settings.fanout.maxMillisecondsPerPin's fallback literal, \
         autoroute/pipeline/BatchFanout.java:175-178 (`: 10000L`)"
    );
    assert_eq!(
        budget.board_update_throttle_ms, 250,
        "BatchAutorouter.shouldFireBoardUpdate's gate, \
         autoroute/pipeline/BatchAutorouter.java:335-343 (`> 250`)"
    );
    assert_eq!(
        budget.progress_throttle_ms, 1000,
        "core.ProgressThrottler's constructor argument — Java has no constant, and all three \
         construction sites pass 1000: autoroute/pipeline/BatchOptimizer.java:29, \
         autoroute/pipeline/BatchAutorouterThread.java:43, autoroute/pipeline/BatchFanout.java:28"
    );

    assert_ne!(
        budget.board_update_throttle_ms, budget.progress_throttle_ms,
        "conflating the two throttles would turn a 250 ms gate into a 1000 ms gate"
    );
}

#[test]
fn a_zero_opt_changed_area_budget_is_javas_no_limit() {
    assert!(
        RouterBudget::java_literals()
            .opt_changed_area_limit()
            .is_some(),
        "TraceTightener.java:73 — `if (timeLimit > 0) this.timeLimit = new TimeLimit(timeLimit)`"
    );
    let disabled = RouterBudget::disabled();
    assert_eq!(disabled.opt_changed_area_ms, 0);
    assert!(
        disabled.opt_changed_area_limit().is_none(),
        "TraceTightener.java:75-77 — `else this.timeLimit = null`"
    );
    let negative = RouterBudget {
        opt_changed_area_ms: -1,
        ..RouterBudget::default()
    };
    assert!(negative.opt_changed_area_limit().is_none());
}

#[test]
fn the_ports_default_budget_departs_from_java_in_exactly_one_field() {
    let java = RouterBudget::java_literals();
    let port = RouterBudget::default();

    assert_eq!(java.opt_changed_area_ms, 1000, "the jar's inlined constant");
    assert_eq!(
        port.opt_changed_area_ms, 0,
        "Java's own `off` value, TraceTightener.java:73-77"
    );
    assert!(
        port.opt_changed_area_limit().is_none(),
        "so a default CLI run's pull-tight is never abandoned mid-way, and two runs agree"
    );

    assert_eq!(port.fanout_ms_per_pin, java.fanout_ms_per_pin);
    assert_eq!(port.board_update_throttle_ms, java.board_update_throttle_ms);
    assert_eq!(port.progress_throttle_ms, java.progress_throttle_ms);

    assert_ne!(RouterBudget::disabled(), port);
}

#[test]
fn the_opt_changed_area_budget_trips_and_disabling_it_removes_the_trip() {
    let tight = RouterBudget {
        opt_changed_area_ms: 0,
        ..RouterBudget::default()
    };
    assert!(
        tight.opt_changed_area_limit().is_none(),
        "0 is Java's `no limit`, not a tight one — that is the point of the `> 0` guard"
    );

    let trips = RouterBudget {
        opt_changed_area_ms: 1,
        ..RouterBudget::default()
    };
    let limit = trips
        .opt_changed_area_limit()
        .expect("1 > 0, so TraceTightener builds a TimeLimit");
    assert!(!limit.is_exceeded(), "not yet");
    assert!(
        spin_until(|| limit.is_exceeded()),
        "a 1 ms budget must trip within the spin bound"
    );

    assert!(RouterBudget::disabled().opt_changed_area_limit().is_none());
}

#[test]
fn the_disabled_budget_turns_every_wall_clock_off() {
    let disabled = RouterBudget::disabled();
    assert_eq!(disabled.opt_changed_area_ms, 0, "TraceTightener.java:73-77");
    assert_eq!(
        disabled.fanout_ms_per_pin,
        i32::MAX,
        "BatchFanout.java:231-232 has no `> 0` guard, so 0 would be a *tight* limit"
    );
    assert_eq!(disabled.board_update_throttle_ms, 0);
    assert_eq!(disabled.progress_throttle_ms, 0);

    let throttler = disabled.progress_throttler();
    let t0 = Instant::now();
    for i in 0..5 {
        assert!(
            throttler.should_update_at(t0 + Duration::from_micros(i)),
            "a disabled throttle fires every time"
        );
    }
}

#[test]
fn the_progress_throttler_gates_on_its_interval() {
    let throttler = ProgressThrottler::new(1000);
    assert_eq!(throttler.interval_ms(), 1000);

    let t0 = Instant::now();
    assert!(
        throttler.should_update_at(t0),
        "ProgressThrottler.java:17-20 — `lastUpdateMs == 0` arms and returns true"
    );
    assert!(!throttler.should_update_at(t0 + Duration::from_millis(1)));
    assert!(!throttler.should_update_at(t0 + Duration::from_millis(999)));
    assert!(
        throttler.should_update_at(t0 + Duration::from_millis(1000)),
        "ProgressThrottler.java:21 is `>=`, so exactly the interval fires"
    );
    assert!(!throttler.should_update_at(t0 + Duration::from_millis(1999)));
    assert!(throttler.should_update_at(t0 + Duration::from_millis(2000)));
}

#[test]
fn the_board_update_gate_is_strict_where_the_progress_gate_is_not() {
    let t0 = Instant::now();

    let board_update = ProgressThrottler::board_update_gate(250);
    assert!(board_update.should_update_at(t0), "lastTimestamp is 0");
    assert!(!board_update.should_update_at(t0 + Duration::from_millis(250)));
    assert!(
        board_update.should_update_at(t0 + Duration::from_millis(251)),
        "BatchAutorouter.java:337-338 — `currentTime - lastBoardUpdateTimestamp > 250`"
    );

    let progress = ProgressThrottler::new(250);
    assert!(progress.should_update_at(t0));
    assert!(
        progress.should_update_at(t0 + Duration::from_millis(250)),
        "ProgressThrottler.java:21 — `now - lastUpdateMs >= intervalMs`"
    );
}

#[test]
fn resetting_the_throttler_arms_the_next_call() {
    let throttler = ProgressThrottler::new(1000);
    let t0 = Instant::now();
    assert!(throttler.should_update_at(t0));
    assert!(!throttler.should_update_at(t0 + Duration::from_millis(1)));
    throttler.reset();
    assert!(
        throttler.should_update_at(t0 + Duration::from_millis(2)),
        "after reset the next call always fires (ProgressThrottler.java:17-20)"
    );
}

#[test]
fn the_budget_builds_both_gates() {
    let budget = RouterBudget::default();
    assert_eq!(budget.board_update_throttler().interval_ms(), 250);
    assert_eq!(budget.progress_throttler().interval_ms(), 1000);
}

#[test]
fn a_pass_record_is_a_plain_six_field_value() {
    use fr_router::pipeline::PassRecord;
    let record = PassRecord {
        pass: 3,
        score: 1.5,
        incomplete_count: 12,
        clearance_violations: 0,
        via_count: 7,
        trace_count: 40,
    };
    assert_eq!(record, record.clone());
    let PassRecord {
        pass,
        score,
        incomplete_count,
        clearance_violations,
        via_count,
        trace_count,
    } = record;
    assert_eq!(
        (
            pass,
            score,
            incomplete_count,
            clearance_violations,
            via_count,
            trace_count
        ),
        (3, 1.5, 12, 0, 7, 40)
    );
}

fn spin_until(condition: impl Fn() -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(5) {
        if condition() {
            return true;
        }
        std::hint::spin_loop();
    }
    false
}

struct MiniPass {
    hash: u64,
    routed: usize,
}

fn route_mini_pass(stop: &RouterStop, progress: &mut dyn ProgressSink) -> MiniPass {
    let path = parity::java_dir().join("fixtures/Issue508-DAC2020_bm01.dsn");
    let file = std::fs::File::open(&path)
        .unwrap_or_else(|e| panic!("cannot open {}: {e}", path.display()));
    let mut board = match fr_dsn::read_board(
        file,
        None,
        Some("Issue508-DAC2020_bm01.dsn"),
        &DsnReadOptions::default(),
    ) {
        BoardReadResult::Success { board, .. } | BoardReadResult::OutlineMissing { board, .. } => {
            *board.expect("a board")
        }
        other => panic!("did not read: {other:?}"),
    };
    let settings = build_settings(&board);
    let trace_costs = settings.get_trace_costs();

    progress.on_event(&RoutingEvent::TaskStateChanged {
        algorithm: NamedAlgorithmType::Router,
        state: TaskState::Started,
    });

    let connections = pick_connections(&board, 2);
    let mut routed = 0;
    for (item_id, net_no) in connections {
        if stop.poll_deadline() || stop.is_stop_auto_router_requested() {
            break;
        }
        if board.get_item(item_id).is_none() {
            continue;
        }
        board.start_marking_changed_area();
        let mut ripped: BTreeSet<ItemId> = BTreeSet::new();
        let mut ripup_costs: BTreeMap<ItemId, i32> = BTreeMap::new();
        let mut engine = None;
        route_connection(
            &mut board,
            &mut engine,
            item_id,
            net_no,
            &settings,
            &trace_costs,
            &mut ripped,
            &mut ripup_costs,
            1,
            settings.get_start_ripup_costs(),
            !settings.is_fanout_enabled(),
            false,
            &|| stop.is_stop_auto_router_requested(),
        );
        routed += 1;
        progress.on_event(&RoutingEvent::BoardUpdated {
            counters: RouterCounters {
                pass_count: Some(1),
                routed_count: Some(routed),
                ..RouterCounters::default()
            },
        });
    }

    progress.on_event(&RoutingEvent::TaskStateChanged {
        algorithm: NamedAlgorithmType::Router,
        state: TaskState::Finished,
    });

    MiniPass {
        hash: board.structural_hash(),
        routed: routed as usize,
    }
}

fn build_settings(board: &Board) -> RouterSettings {
    let mut settings = DefaultSettings::new(&HostEnvironment::detect())
        .get_settings()
        .expect("DefaultSettings always answers a table")
        .clone();
    settings.set_layer_count(board.get_layer_count());
    settings.apply_board_specific_optimizations(board);
    settings
}

fn pick_connections(board: &Board, max_items: usize) -> Vec<(ItemId, i32)> {
    let mut result = Vec::new();
    for item_id in board.items_in_board_order() {
        let Some(item) = board.get_item(item_id) else {
            continue;
        };
        if item.as_connectable().is_none() {
            continue;
        }
        for net_no in item.net_nos().to_vec() {
            if board.unconnected_set(item_id, net_no).is_empty() {
                continue;
            }
            result.push((item_id, net_no));
            if result.len() >= max_items {
                return result;
            }
        }
    }
    result
}

#[test]
fn a_sink_can_live_behind_a_refcell() {
    let sink = RefCell::new(RecordingSink::default());
    sink.borrow_mut()
        .on_event(&RoutingEvent::BoardSnapshot { pass: 1 });
    assert_eq!(sink.borrow().events.len(), 1);
}
