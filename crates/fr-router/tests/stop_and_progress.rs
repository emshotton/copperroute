//! Plan 7 Task 4 (controller ruling AI): the three-state stop, the deadline, the `RouterBudget`
//! knob, `ProgressSink` and the small pipeline DTOs.
//!
//! # Where the ground truth comes from
//!
//! Everything a JVM can be asked is asked, and the answer is committed: `tests/data/
//! p7t4-stop-and-counters.txt` is the byte-stable stdout of
//! `scripts/differential/java/probes/P7T4Probe.java` on the HEAD jar, and the transition table,
//! the two queries, the three enums' variant lists and `RouterCounters`' nine-field declaration
//! order are all read back out of it rather than restated here. What a JVM cannot be asked —
//! three of the four `RouterBudget` literals, because they are inline in method bodies or are
//! constructor arguments to an instance-field initialiser — carries its Java file:line in the
//! assertion message instead (the controller's "a small probe or direct source citation per
//! field").
//!
//! # What is parity and what is not
//!
//! The transition table, the queries, the enum variant lists, the counter field list and the four
//! budget defaults are **parity**: Java decides them. `the_opt_changed_area_budget_trips_...` is
//! explicitly **not** — no Java run reaches a `TIME_LIMIT_TO_PREVENT_ENDLESS_LOOP`, which is
//! ruling AI's whole premise, so the test proves the port's own knob works rather than that Java
//! agrees. The two fixture tests are determinism claims, also not parity: they assert that
//! neither an unexpired deadline nor a recording `ProgressSink` moves a single board byte.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

use fr_board::prelude::*;
use fr_dsn::{BoardReadResult, DsnReadOptions};
use fr_router::pipeline::{
    NamedAlgorithmType, NoopProgressSink, ProgressSink, ProgressThrottler, RouterBudget,
    RouterCounters, RouterStop, RoutingEvent, StopRequestState, TaskState,
};
use fr_router::route_connection;
use fr_settings::sources::DefaultSettings;
use fr_settings::{HostEnvironment, RouterSettings, SettingsSource};

// =================================================================================================
// The probe transcript
// =================================================================================================

const TRANSCRIPT: &str = include_str!("data/p7t4-stop-and-counters.txt");

/// Every line of the transcript under the `[section]` header, header excluded.
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

/// `key=value key=value …` -> the value of `key`.
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

/// A `RouterStop` forced into `state`, the way `P7T4Probe.withState` reflects the private field.
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

// =================================================================================================
// `StoppableThread`'s transition table (core/StoppableThread.java:8, 20-42)
// =================================================================================================

/// The `[queries-at-rest]` block: `isStopRequested()` is `== ALL` (`:28-30`) and
/// `isStopAutoRouterRequested()` is `!= NONE` (`:40-42`). The two are adjacent and easy to swap,
/// so both are asserted in all three states.
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

/// The full 3 x 2 table from `[transitions]` — every starting state against both requests, with
/// both queries read afterwards (`StoppableThread.java:23-25`, `:33-37`).
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

/// `requestStopAutoRouter` only ever upgrades `NONE -> AUTO_ROUTER_ONLY`
/// (`StoppableThread.java:33-37`), so once the state is `ALL` it is a **no-op** — which is why
/// `AutorouteBatchLoop.java:251-253`'s `if (job.state == TIMED_OUT) requestStopAutoRouter()` can
/// never do anything in production (quirk #203: the monitor thread called `requestStop()` 30 s
/// earlier). The `[sequences]` block's `seq=monitorTimeout` row is the JVM's word for it.
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

// =================================================================================================
// Quirk #202: `--max-items` silently disables the optimizer and `--max-passes` does not
// =================================================================================================

/// The port's miniature of the three Java sites the quirk is made of:
///
/// * `AutoroutePassRunner.java:213-220` — `maxItems` reached, `thread.requestStop()` (**ALL**);
/// * `AutorouteBatchLoop.java:267-271` — `maxPasses` reached, `thread.requestStopAutoRouter()`
///   (**AUTO_ROUTER_ONLY**);
/// * `RoutingPipeline.java:116-118` — the optimizer stage returns early on `isStopRequested()`.
///
/// Everything below is the *reading* of the stop state, not the pass loop: the loop is Tasks 9
/// and 10 and the optimizer stage is Task 15. What this asserts is that `RouterStop` answers the
/// three sites the way the JVM does, which is the whole of the quirk.
fn optimizer_stage_would_run(stop: &RouterStop) -> bool {
    // RoutingPipeline.java:117 — `if (this.optimizer == null || this.job.thread.isStopRequested())`
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

    // maxItems: AutoroutePassRunner.java:219.
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
        "quirk #202: hitting --max-items silently disables the optimizer stage \
         (RoutingPipeline.java:117 reads isStopRequested, which is ALL-only)"
    );
    assert_eq!(
        field(max_items_row, "optimizerStageWouldRun"),
        "false",
        "the JVM's word: `{max_items_row}`"
    );

    // maxPasses: AutorouteBatchLoop.java:271.
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
        "the JVM's word: `{max_passes_row}`"
    );
}

// =================================================================================================
// Ruling AI's deadline
// =================================================================================================

/// `RoutingJobSchedulerActionThread.java:73-84`: on expiry the monitor thread calls
/// `job.thread.requestStop()` — `ALL`, not `AUTO_ROUTER_ONLY` — and only writes
/// `RoutingJobState.TIMED_OUT` after a 30 s grace period. The port has no thread, so
/// [`RouterStop::poll_deadline`] performs both at the poll site.
#[test]
fn the_deadline_requests_stop_all_like_the_monitor_thread() {
    // A limit already in the past. `TimeLimit::new(0)`'s `is_exceeded` is `elapsed > 0`, so this
    // needs one tick of the monotonic clock and nothing more; the loop below is bounded.
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
    // Idempotent: a second poll neither un-sets nor changes anything.
    assert!(stop.poll_deadline());
    assert_eq!(stop.state(), StopRequestState::All);
}

/// A `RouterStop` with **no** deadline is what the CLI builds, and polling it is inert.
#[test]
fn a_stop_without_a_deadline_never_times_out() {
    let stop = RouterStop::new();
    for _ in 0..1000 {
        assert!(!stop.poll_deadline());
    }
    assert_eq!(stop.state(), StopRequestState::None);
    assert!(!stop.is_timed_out());
}

/// Ruling AI's determinism claim: a deadline far in the future must not be observable in the
/// board. Two runs of the same fixture, one polling an unexpired deadline at every item, and
/// [`Board::structural_hash`] equal afterwards.
#[test]
fn an_unexpired_deadline_is_invisible() {
    if !parity::require_java_dir() {
        return;
    }
    let without = route_mini_pass(&RouterStop::new(), &mut NoopProgressSink);
    // An hour: `RoutingFixtureTest.java:71-76`'s `jobTimeoutString` default is one minute, and
    // ruling AI says the port's twin must not inherit a wall clock at all — so the deadline is
    // set explicitly here and set beyond any possible run.
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

// =================================================================================================
// `ProgressSink` (spec §10, ruling AK) — ruling 11's "no port decision reads it"
// =================================================================================================

/// A sink that records every event it is handed, and nothing else.
#[derive(Default)]
struct RecordingSink {
    events: Vec<RoutingEvent>,
}

impl ProgressSink for RecordingSink {
    fn on_event(&mut self, event: &RoutingEvent) {
        self.events.push(event.clone());
    }
}

/// Ruling 11: the sink is an observer, never an input. Route the same fixture twice — once into
/// [`NoopProgressSink`], once into a sink that records every event — and assert the board is
/// byte-for-byte the same and that the recording sink actually saw something.
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

/// [`NoopProgressSink`] is Java with every listener list empty — the headless CLI's state
/// (`NamedAlgorithm.java:26-31`, three `new ArrayList<>()`s nothing ever adds to on that path).
#[test]
fn the_noop_sink_swallows_every_event() {
    let mut sink = NoopProgressSink;
    for event in every_event_shape() {
        sink.on_event(&event);
    }
}

/// One value of every [`RoutingEvent`] variant, so a new variant cannot be added without this
/// list noticing (the `match` below is exhaustive).
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
        // Exhaustive on purpose: adding a variant must break this file.
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

// =================================================================================================
// The two enums (autoroute/pipeline/{NamedAlgorithmType,TaskState}.java)
// =================================================================================================

/// The variant sets **and their order**, read off `[enums]`. Java's `ordinal()` is what a Gson
/// round trip and every `switch` table key on, so the order is part of the port's contract.
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
    // `PartialOrd`/`Ord` follow declaration order, which is Java's `ordinal()` order.
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

/// `Name = A(0) B(1) …` -> `["A", "B", …]`, with the ordinals checked to be dense and ascending.
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

// =================================================================================================
// `RouterCounters` (core/RouterCounters.java:1-48)
// =================================================================================================

/// The port's field list against the probe's **reflected declaration order** — not against a doc
/// comment. Nine fields; `phase` and `fanoutExtraViasCount` are the two an eyeball transcription
/// misses.
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

    // Every Java field is a boxed `Integer`/`String` and every default is `null`
    // (`[RouterCounters] … default=null` x 9), so the port's fields are `Option`s and its
    // `Default` is all-`None`. That is why `RouterCounters` is not `Copy`.
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

// =================================================================================================
// `RouterBudget` (ruling AI, amendment ruling 10)
// =================================================================================================

/// [`RouterBudget::java_literals`]'s four values are Java's literals, each with its Java line in
/// the message. Two of them the probe reflected; two are inline in a method body and one is a
/// constructor argument, so those three are source citations (see the module doc).
///
/// **This was `RouterBudget::default()` until Plan 9 Task 1.** #234 moved the port's default
/// `opt_changed_area_ms` to `0` — Java's own "off" value — because the 1000 abandons the
/// pull-tight on wall clock and makes the jar's own output depend on how fast the machine is.
/// The Java fact did not stop being a fact, so it kept a constructor and kept this test; the
/// port's departure from it is pinned separately by
/// [`the_ports_default_budget_departs_from_java_in_exactly_one_field`].
#[test]
fn the_java_literal_budget_carries_javas_four_literals() {
    let budget = RouterBudget::java_literals();

    // Reflected by the probe at all four declaration sites.
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

    // Amendment ruling 10: **four** fields, not three, and 250 and 1000 are two different knobs.
    assert_ne!(
        budget.board_update_throttle_ms, budget.progress_throttle_ms,
        "conflating the two throttles would turn a 250 ms gate into a 1000 ms gate"
    );
}

/// `TraceTightener`'s constructor only builds a `TimeLimit` when `timeLimit > 0`
/// (`board/optimize/TraceTightener.java:73-77`), so `opt_changed_area_ms = 0` is *exactly*
/// Java's "no limit" and needs no port-only branch. That is what makes
/// [`RouterBudget::disabled`] a faithful configuration rather than a port-only mode — and, since
/// Plan 9 Task 1, what makes the port's own default a legal Java configuration too (#234) rather
/// than a behaviour the jar has no way to express.
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
    // A negative budget takes the same `else` branch (`> 0`, not `!= 0`).
    let negative = RouterBudget {
        opt_changed_area_ms: -1,
        ..RouterBudget::default()
    };
    assert!(negative.opt_changed_area_limit().is_none());
}

/// #234's fix, stated as the delta rather than as an absolute: the port's default differs from
/// Java's literals in **exactly one** field, and it is the only one of the four that can change a
/// routed board.
///
// fixed: T1 (#234) — this is the test that says how much of a departure the fix is, so that a
// later reader can see it was one field and not a general retreat from parity.
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

    // The other three are untouched: a fanout per-pin budget that changes what a user gets on a
    // huge board, and two progress throttles that decide only whether an event fires.
    assert_eq!(port.fanout_ms_per_pin, java.fanout_ms_per_pin);
    assert_eq!(port.board_update_throttle_ms, java.board_update_throttle_ms);
    assert_eq!(port.progress_throttle_ms, java.progress_throttle_ms);

    // And `disabled` is still strictly stronger than `default`, which is why ruling AI's parity
    // configuration and `scripts/quality-ab.sh`'s quality lane are not made redundant by #234.
    assert_ne!(RouterBudget::disabled(), port);
}

/// **Not a parity test** (ruling AI: no parity run ever reaches this limit). It proves the port's
/// knob does what the name says — the budget trips — and that disabling it removes the trip.
#[test]
fn the_opt_changed_area_budget_trips_and_disabling_it_removes_the_trip() {
    // 0 ms: `TimeLimit::is_exceeded` is `elapsed > limit`, so this trips on the first tick of the
    // monotonic clock. The spin is bounded and asserts a *condition*, never a duration.
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

    // Disabling it removes the trip: there is no `TimeLimit` at all to exceed.
    assert!(RouterBudget::disabled().opt_changed_area_limit().is_none());
}

/// [`RouterBudget::disabled`] is what every `p7t*` driver sets on both sides. Each field's
/// "off" value is the one Java's own code makes inert, and they are **not** all zero: the fanout
/// per-pin limit has no `> 0` guard (`BatchFanout.java:231-232` builds the `TimeLimit`
/// unconditionally), so its off value is `i32::MAX`, not `0`.
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

    // A disabled throttle never suppresses, which is the deterministic choice.
    let throttler = disabled.progress_throttler();
    let t0 = Instant::now();
    for i in 0..5 {
        assert!(
            throttler.should_update_at(t0 + Duration::from_micros(i)),
            "a disabled throttle fires every time"
        );
    }
}

// =================================================================================================
// `ProgressThrottler` — the clock-injected gate
// =================================================================================================

/// `core/ProgressThrottler.java:15-26`: the first call arms and fires, and thereafter the gate is
/// `now - last >= interval` — **non-strict**. Driven off an injected instant, so no test sleeps.
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
    // …and that call moved `lastUpdateMs` to t0+1000.
    assert!(!throttler.should_update_at(t0 + Duration::from_millis(1999)));
    assert!(throttler.should_update_at(t0 + Duration::from_millis(2000)));
}

/// `BatchAutorouter.shouldFireBoardUpdate` (`:335-343`) is the *other* gate, and its comparison
/// is **strict** (`> 250`), one millisecond apart from `ProgressThrottler`'s. Amendment ruling 10
/// keeps the two as separate knobs for exactly this reason.
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

/// `ProgressThrottler.reset` (`:29-31`) puts `lastUpdateMs` back to the `0` sentinel, so the next
/// call fires whatever the clock says. Three live callers: `BatchAutorouterThread.java:309`,
/// `BatchOptimizer.java:284`, `BatchFanout.java:203`.
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

/// The budget builds both gates, so a driver that pins the knobs pins the gates.
#[test]
fn the_budget_builds_both_gates() {
    let budget = RouterBudget::default();
    assert_eq!(budget.board_update_throttler().interval_ms(), 250);
    assert_eq!(budget.progress_throttler().interval_ms(), 1000);
}

// =================================================================================================
// `PassRecord` (ruling 1(a)'s per-pass tuple; produced here per scan ruling 6)
// =================================================================================================

/// It is a plain value with six public fields — no Java counterpart, so the only thing to pin is
/// that it stays a value and keeps ruling 1(a)'s six members.
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

// =================================================================================================
// The mini pass loop the two fixture tests share
// =================================================================================================

/// A bounded spin on the monotonic clock: returns whether `condition` became true inside five
/// seconds. Used instead of `thread::sleep` so the tests assert a *condition* and never a
/// duration, and so a loaded machine cannot make them flaky in either direction.
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

/// `AutoroutePassRunner.runPass` in miniature: two connections of `Issue508-DAC2020_bm01.dsn`
/// (the fixture harness's in-CI smoke board — 0.08 s in a debug build), with ruling AI's deadline
/// polled at the top of the item loop and the `ProgressSink` fired around it.
///
/// The real pass loop is Task 9's and the real stage bracket is Task 15's; this is only enough of
/// both to make "does the plumbing move a board byte?" answerable now, which is what ruling AI's
/// and ruling 11's determinism claims need.
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
        // Ruling AI's site: the top of `AutoroutePassRunner`'s item loop (`:203`).
        if stop.poll_deadline() || stop.is_stop_auto_router_requested() {
            break;
        }
        if board.get_item(item_id).is_none() {
            continue;
        }
        // `AutoroutePassRunner.java:223` — quirk #177.
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

/// The headless settings ladder's priority-0 source, as `crates/fr-router/tests/fixtures.rs`
/// builds it.
fn build_settings(board: &Board) -> RouterSettings {
    let mut settings = DefaultSettings::new(&HostEnvironment::detect())
        .get_settings()
        .expect("DefaultSettings always answers a table")
        .clone();
    settings.set_layer_count(board.get_layer_count());
    settings.apply_board_specific_optimizations(board);
    settings
}

/// `AutoroutePassRunner.runPass`'s item walk, truncated at `maxItems`, as
/// `crates/fr-router/tests/fixtures.rs` computes it.
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

/// [`ProgressSink`] takes `&mut self`, so a caller that must *read* what its sink recorded while
/// the pipeline still holds it puts the sink behind a [`RefCell`] — the pattern Plan 8's CLI
/// needs, asserted here so the `&mut self` receiver is a deliberate choice rather than an
/// accident. Java's equivalent is a listener list the caller keeps a reference into
/// (`NamedAlgorithm.java:26-31`).
#[test]
fn a_sink_can_live_behind_a_refcell() {
    let sink = RefCell::new(RecordingSink::default());
    sink.borrow_mut()
        .on_event(&RoutingEvent::BoardSnapshot { pass: 1 });
    assert_eq!(sink.borrow().events.len(), 1);
}
