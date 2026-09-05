use fr_board::prelude::*;
use fr_router::RouterError;
use fr_router::pipeline::batch_loop::fanout_recovery_fires;
use fr_router::pipeline::{
    AutorouteBatchLoop, BatchFanout, FanoutLoopState, FanoutStop, NoopProgressSink, ProgressSink,
    RouterBudget, RouterStop, RoutingEvent, fanout_pin_can_use_vias, fanout_ripup_costs,
    parse_timespan_seconds, parse_timespan_seconds_java,
};
use fr_settings::sources::DefaultSettings;
use fr_settings::{HostEnvironment, RouterSettings, SettingsSource};

const RPI: &str = "fixtures/Issue143-rpi_splitter.dsn";

const ECC83: &str = "fixtures/Issue649-kicad_ecc83-pp_input_board_v1.dsn";

fn load_board(rel_path: &str) -> Board {
    let path = parity::java_dir().join(rel_path);
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

fn build_settings(board: &Board) -> RouterSettings {
    let mut settings = DefaultSettings::new(&HostEnvironment::detect())
        .get_settings()
        .expect("DefaultSettings always answers a table")
        .clone();
    settings.set_layer_count(board.get_layer_count());
    settings.apply_board_specific_optimizations(board);
    let fanout = settings.fanout.get_or_insert_with(Default::default);
    fanout.max_milliseconds_per_pin = Some(i64::from(i32::MAX));
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

#[test]
#[cfg_attr(debug_assertions, ignore)]
fn an_empty_smd_pin_set_skips_the_pre_pass() {
    if !parity::require_java_dir() {
        return;
    }
    let mut board = load_board(ECC83);
    assert!(
        board.get_smd_pins().is_empty(),
        "the fixture is chosen for having no SMD pins at all"
    );
    let mut settings = build_settings(&board);
    settings.max_passes = Some(1);
    settings.set_run_router(true);
    settings.set_run_optimizer(false);
    settings.fanout.get_or_insert_with(Default::default).enabled = Some(true);

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
    assert!(
        result.fanout.is_none(),
        "`:90-91` skips the whole stage, so there is no summary to report"
    );
}

#[test]
#[cfg_attr(debug_assertions, ignore)]
fn a_board_with_smd_pins_runs_the_pre_pass_and_reports_it() {
    if !parity::require_java_dir() {
        return;
    }
    let mut board = load_board(RPI);
    let mut settings = build_settings(&board);
    settings.max_passes = Some(1);
    settings.set_run_router(true);
    settings.set_run_optimizer(false);
    settings.fanout.get_or_insert_with(Default::default).enabled = Some(true);

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
    let summary = result.fanout.expect("the pre-pass ran");
    assert!(!summary.is_timed_out, "no timeout is configured");
    assert_eq!(
        summary.escape_statistics.total_smd_pins, 10,
        "the JVM's number for this stem (`p7t5 board`'s ESCAPE line)"
    );
    assert_eq!(summary.escape_statistics.escaped_count, 9);
}

#[test]
#[cfg_attr(debug_assertions, ignore)]
fn zero_routed_pins_ends_the_loop() {
    if !parity::require_java_dir() {
        return;
    }
    let mut board = load_board(RPI);
    let settings = build_settings(&board);
    let stop = RouterStop::new();
    let mut sink = NoopProgressSink;
    let summary = BatchFanout::fanout_board(
        &mut board,
        &settings,
        &stop,
        RouterBudget::disabled(),
        &mut sink,
    )
    .expect("fanout_board answers Ok on every path");
    assert_eq!(
        summary.completed_pass_count, 2,
        "pass 1 escapes nine pins, pass 2 escapes none and ends the loop — and is counted"
    );
    assert!(!summary.is_timed_out);
    assert_eq!(summary.escape_statistics.total_smd_pins, 10);
    assert_eq!(summary.escape_statistics.escaped_count, 9);
    assert!((summary.escape_statistics.escaped_percentage - 90.0).abs() < f64::EPSILON);
}

#[test]
fn two_passes_with_different_hashes_do_not_stop_the_loop() {
    let mut state = FanoutLoopState::new(1);
    assert_eq!(state.after_pass(1, false, || 2), None);
    assert_eq!(state.after_pass(1, false, || 3), None);
}

#[test]
fn an_unchanged_hash_stops_the_loop_on_the_first_repeat() {
    let mut state = FanoutLoopState::new(1);
    assert_eq!(
        state.after_pass(2, false, || 1),
        Some(FanoutStop::UnchangedHash)
    );
}

#[test]
fn the_stops_are_tested_in_order() {
    let mut state = FanoutLoopState::new(1);
    assert_eq!(
        state.after_pass(0, true, || 1),
        Some(FanoutStop::NothingRouted)
    );

    let mut state = FanoutLoopState::new(7);
    let mut hash_taken = false;
    let stop = state.after_pass(1, true, || {
        hash_taken = true;
        7
    });
    assert_eq!(stop, Some(FanoutStop::TimedOut));
    assert!(!hash_taken, ":153 is never evaluated");
}

#[test]
fn ripup_costs_scale_with_the_pass_number() {
    if !parity::require_java_dir() {
        return;
    }
    let board = load_board(RPI);
    let mut settings = build_settings(&board);
    let start = settings.get_start_ripup_costs();
    assert_eq!(start, 100, "the JVM's number for this stem (`p7t5 pass`)");

    for pass_no in 0..5 {
        assert_eq!(
            fanout_ripup_costs(&settings, pass_no),
            start * (pass_no + 1),
            "`:173` is a plain multiply by the 1-based pass number"
        );
    }

    settings
        .fanout
        .get_or_insert_with(Default::default)
        .ripup_allowed = Some(false);
    for pass_no in 0..5 {
        assert_eq!(
            fanout_ripup_costs(&settings, pass_no),
            -1,
            "the pass number does not scale the sentinel"
        );
    }

    settings
        .fanout
        .get_or_insert_with(Default::default)
        .ripup_allowed = None;
    assert_eq!(fanout_ripup_costs(&settings, 0), start);
    settings.fanout = None;
    assert_eq!(fanout_ripup_costs(&settings, 0), start);
}

#[test]
fn a_pin_whose_net_does_not_resolve_cannot_use_vias() {
    if !parity::require_java_dir() {
        return;
    }
    let mut board = load_board(RPI);
    let mut settings = build_settings(&board);

    assert!(board.rules.nets.get(0).is_none());
    assert!(board.rules.nets.get(9_999).is_none());
    assert!(!fanout_pin_can_use_vias(&board, &settings, 0));
    assert!(!fanout_pin_can_use_vias(&board, &settings, 9_999));

    assert!(board.rules.nets.get(1).is_some());
    assert!(fanout_pin_can_use_vias(&board, &settings, 1));

    let net_class = board.rules.nets.get(1).expect("net 1").get_net_class();
    board
        .rules
        .net_classes
        .get_mut(net_class)
        .set_via_rule(None);
    settings
        .fanout
        .get_or_insert_with(Default::default)
        .fallback_to_board_vias = Some(false);
    assert!(
        !fanout_pin_can_use_vias(&board, &settings, 1),
        "`:251` — no rule vias and no fallback"
    );
    assert!(!fanout_pin_can_use_vias(&board, &settings, 9_999));

    settings
        .fanout
        .get_or_insert_with(Default::default)
        .fallback_to_board_vias = Some(true);
    assert!(board.rules.via_rules[0].via_count() > 0);
    assert!(fanout_pin_can_use_vias(&board, &settings, 1));
}

#[test]
fn the_max_items_gate_treats_a_non_positive_limit_as_no_limit() {
    if !parity::require_java_dir() {
        return;
    }
    let board = load_board(RPI);
    let mut settings = build_settings(&board);
    let fanout_settings = settings.fanout.get_or_insert_with(Default::default);
    fanout_settings.max_items = Some(2);
    let mut fanout = BatchFanout::new(&board, &settings);
    assert!(!fanout.max_items_reached());
    fanout.total_items_fanouted = 1;
    assert!(!fanout.max_items_reached());
    fanout.total_items_fanouted = 2;
    assert!(fanout.max_items_reached(), "the test is `>=`, not `>`");

    settings
        .fanout
        .get_or_insert_with(Default::default)
        .max_items = Some(0);
    let mut fanout = BatchFanout::new(&board, &settings);
    fanout.total_items_fanouted = 1_000;
    assert!(!fanout.max_items_reached(), "`maxItems > 0` is the guard");

    settings
        .fanout
        .get_or_insert_with(Default::default)
        .max_items = None;
    let mut fanout = BatchFanout::new(&board, &settings);
    fanout.total_items_fanouted = 1_000;
    assert!(!fanout.max_items_reached());
}

#[test]
#[cfg_attr(debug_assertions, ignore)]
fn the_stage_deadline_never_touches_the_stop_flag() {
    if !parity::require_java_dir() {
        return;
    }
    let mut board = load_board(RPI);
    let mut settings = build_settings(&board);
    settings
        .fanout
        .get_or_insert_with(Default::default)
        .timeout_string = Some("-1".to_string());

    let stop = RouterStop::new();
    let mut sink = NoopProgressSink;
    let summary = BatchFanout::fanout_board(
        &mut board,
        &settings,
        &stop,
        RouterBudget::disabled(),
        &mut sink,
    )
    .expect("fanout_board answers Ok on every path");
    assert!(summary.is_timed_out, "`:113` fires before the first pass");
    assert_eq!(summary.completed_pass_count, 0);
    assert!(
        !stop.is_stop_auto_router_requested() && !stop.is_stop_requested(),
        "a fanout timeout leaves the router and the optimizer running — Task 4's split"
    );
    assert!(
        !stop.is_timed_out(),
        "and it is not the job-level `TIMED_OUT` either"
    );
    assert_eq!(board.get_vias().len(), 0, "no pass ran, so nothing escaped");
}

#[test]
#[cfg_attr(debug_assertions, ignore)]
fn the_progress_sink_sees_the_1_based_pass_number() {
    if !parity::require_java_dir() {
        return;
    }
    let mut board = load_board(RPI);
    let settings = build_settings(&board);
    let stop = RouterStop::new();
    let mut sink = Recorder::default();
    let summary = BatchFanout::fanout_board(
        &mut board,
        &settings,
        &stop,
        RouterBudget::disabled(),
        &mut sink,
    )
    .expect("fanout_board answers Ok on every path");

    let progress: Vec<(i32, i32, i32)> = sink
        .events
        .iter()
        .filter_map(|event| match event {
            RoutingEvent::FanoutProgress {
                pass,
                routed,
                pins_to_go,
            } => Some((*pass, *routed, *pins_to_go)),
            _ => None,
        })
        .collect();
    assert!(!progress.is_empty(), "the throttle is disabled in this run");
    assert_eq!(
        progress[0],
        (1, 0, 10),
        "`:205-217` — the pass-start tick, 1-based, with every pin still to go"
    );
    assert!(
        progress.iter().all(|(pass, _, _)| *pass >= 1),
        "`passNo + 1` is what the record carries"
    );
    let last_of_first_pass = progress
        .iter()
        .rfind(|(pass, _, _)| *pass == 1)
        .expect("pass 1 published");
    assert_eq!(
        (last_of_first_pass.1, last_of_first_pass.2),
        (9, 0),
        "the pass-completed tick carries the pass's own counters"
    );
    assert_eq!(summary.completed_pass_count, 2);
}

#[test]
#[cfg_attr(debug_assertions, ignore)]
fn a_recording_sink_changes_no_fanout_byte() {
    if !parity::require_java_dir() {
        return;
    }
    let mut quiet = load_board(RPI);
    let settings = build_settings(&quiet);
    let stop = RouterStop::new();
    let mut noop = NoopProgressSink;
    let quiet_summary = BatchFanout::fanout_board(
        &mut quiet,
        &settings,
        &stop,
        RouterBudget::disabled(),
        &mut noop,
    )
    .expect("fanout_board answers Ok on every path");

    let mut loud = load_board(RPI);
    let loud_settings = build_settings(&loud);
    let loud_stop = RouterStop::new();
    let mut recorder = Recorder::default();
    let loud_summary = BatchFanout::fanout_board(
        &mut loud,
        &loud_settings,
        &loud_stop,
        RouterBudget::disabled(),
        &mut recorder,
    )
    .expect("fanout_board answers Ok on every path");

    assert_eq!(quiet.structural_hash(), loud.structural_hash());
    assert_eq!(
        quiet_summary.completed_pass_count,
        loud_summary.completed_pass_count
    );
    assert_eq!(
        quiet_summary.escape_statistics,
        loud_summary.escape_statistics
    );
}

#[test]
fn the_documented_fanout_timeout_examples_both_parse_to_nothing_in_java() {
    assert_eq!(parse_timespan_seconds_java("5m"), None);
    assert_eq!(parse_timespan_seconds_java("300s"), None);
    assert_eq!(parse_timespan_seconds_java("300"), Some(300));
    assert_eq!(parse_timespan_seconds_java("1:00"), Some(60));
    assert_eq!(parse_timespan_seconds_java("01:02:03"), Some(3723));
    assert_eq!(parse_timespan_seconds_java("00:00:30"), Some(30));
}

#[test]
fn the_timespan_grammar_matches_the_jvms() {
    assert_eq!(parse_timespan_seconds_java(""), None);
    assert_eq!(parse_timespan_seconds_java("   "), None);
    assert_eq!(parse_timespan_seconds_java("-5"), Some(-5));
    assert_eq!(parse_timespan_seconds_java("+5"), Some(5));
    assert_eq!(parse_timespan_seconds_java("007"), Some(7));
    assert_eq!(parse_timespan_seconds_java("1.5"), Some(1));
    assert_eq!(parse_timespan_seconds_java("1.0"), Some(1));
    assert_eq!(parse_timespan_seconds_java("1."), Some(1));
    assert_eq!(parse_timespan_seconds_java("1,5"), Some(1));
    assert_eq!(parse_timespan_seconds_java("0.9"), Some(0));
    assert_eq!(parse_timespan_seconds_java("-1.5"), Some(-2));
    assert_eq!(parse_timespan_seconds_java("-0.9"), Some(-1));
    assert_eq!(parse_timespan_seconds_java("1:0.5"), Some(60));
    assert_eq!(parse_timespan_seconds_java("1:-0.5"), Some(59));
    assert_eq!(parse_timespan_seconds_java("-1:2:3"), Some(-3477));
    assert_eq!(parse_timespan_seconds_java("1:"), Some(1));
    assert_eq!(parse_timespan_seconds_java("2:3:"), Some(123));
    assert_eq!(parse_timespan_seconds_java("1:2:3:4"), None);
    assert_eq!(parse_timespan_seconds_java("::"), None);
    assert_eq!(parse_timespan_seconds_java("1::2"), None);
    assert_eq!(parse_timespan_seconds_java("1.0000000001"), None);
    assert_eq!(parse_timespan_seconds_java("5 "), None);
    assert_eq!(parse_timespan_seconds_java("1.S"), None);
    assert_eq!(
        parse_timespan_seconds_java("9223372036854775807"),
        Some(i64::MAX)
    );
    assert_eq!(parse_timespan_seconds_java("9223372036854775807:0:0"), None);
}

#[test]
fn the_documented_timeout_spellings_parse() {
    assert_eq!(parse_timespan_seconds("5m"), Ok(Some(300)));
    assert_eq!(parse_timespan_seconds("300s"), Ok(Some(300)));
    assert_eq!(parse_timespan_seconds("1h30m"), Ok(Some(5400)));

    assert_eq!(parse_timespan_seconds("2h"), Ok(Some(7200)));
    assert_eq!(parse_timespan_seconds("1h2m3s"), Ok(Some(3723)));
    assert_eq!(parse_timespan_seconds("0m"), Ok(Some(0)));
    assert_eq!(parse_timespan_seconds("-5m"), Ok(Some(-300)));

    assert!(parse_timespan_seconds("30m1h").is_err());
    assert!(
        parse_timespan_seconds("5M").is_err(),
        "M is months in ISO-8601"
    );
    assert!(
        parse_timespan_seconds("5d").is_err(),
        "no day unit is offered"
    );
    assert!(parse_timespan_seconds("m").is_err());
    assert!(parse_timespan_seconds("1h2h").is_err());

    for input in [
        "300",
        "1:00",
        "01:02:03",
        "00:00:30",
        "-5",
        "+5",
        "007",
        "1.5",
        "1,5",
        "0.9",
        "-1.5",
        "-0.9",
        "1:0.5",
        "1:-0.5",
        "-1:2:3",
        "1:",
        "2:3:",
        "9223372036854775807",
    ] {
        assert_eq!(
            parse_timespan_seconds(input),
            Ok(parse_timespan_seconds_java(input)),
            "#224 must not change an answer the jar already gave for {input:?}"
        );
    }

    assert_eq!(parse_timespan_seconds(""), Ok(None));
    assert_eq!(parse_timespan_seconds("   "), Ok(None));

    let error = parse_timespan_seconds("banana").expect_err("`banana` is not a timespan");
    assert_eq!(error.input, "banana");
    assert!(
        error.to_string().contains("banana"),
        "the refusal must name the string the operator wrote: {error}"
    );

    for input in [
        "1:2:3:4",
        "::",
        "1::2",
        "1.0000000001",
        "5 ",
        "1.S",
        "x",
        "PT1H",
    ] {
        assert!(
            parse_timespan_seconds(input).is_err(),
            "{input:?} answered null in Java and must be a refusal here, not an unbounded run"
        );
        assert_eq!(
            parse_timespan_seconds_java(input),
            None,
            "…and it did answer null"
        );
    }
}

#[test]
fn an_unparseable_fanout_timeout_is_refused() {
    if !parity::require_java_dir() {
        return;
    }
    let mut board = load_board(RPI);
    let mut settings = build_settings(&board);
    {
        let fanout = settings.fanout.get_or_insert_with(Default::default);
        fanout.enabled = Some(true);
        fanout.timeout_string = Some("banana".to_string());
    }

    let stop = RouterStop::new();
    let mut progress = fr_router::pipeline::NoopProgressSink;
    let error = BatchFanout::fanout_board(
        &mut board,
        &settings,
        &stop,
        RouterBudget::disabled(),
        &mut progress,
    )
    .expect_err("an unreadable fanout timeout must stop the run");

    let message = error.to_string();
    assert!(
        message.contains("banana"),
        "the failure must name the string the operator wrote: {message}"
    );
    assert!(
        matches!(error, RouterError::Timespan(_)),
        "…and it must be the timespan refusal, not some later failure: {error:?}"
    );

    settings
        .fanout
        .get_or_insert_with(Default::default)
        .timeout_string = Some("5m".to_string());
    let mut board = load_board(RPI);
    BatchFanout::fanout_board(
        &mut board,
        &settings,
        &stop,
        RouterBudget::disabled(),
        &mut progress,
    )
    .expect("`5m` is 300 seconds and the stage runs");
}

#[test]
fn the_fanout_recovery_in_the_batch_loop_fires_once() {
    if !parity::require_java_dir() {
        return;
    }
    let board = load_board(RPI);
    let mut settings = build_settings(&board);
    settings.fanout.get_or_insert_with(Default::default).enabled = Some(true);
    let limit = fr_router::pipeline::batch_loop::FANOUT_RECOVERY_STAGNATION_PASSES;

    assert!(
        fanout_recovery_fires(&settings, false, 1, limit),
        "fanout on, not yet applied, an incomplete connection and the counter at the limit"
    );
    assert!(
        fanout_recovery_fires(&settings, false, 1, limit + 1),
        "the counter test is `>=`"
    );
    assert!(
        !fanout_recovery_fires(&settings, true, 1, limit),
        "one shot: `fanoutRecoveryApplied` is what makes it so"
    );
    assert!(
        !fanout_recovery_fires(&settings, false, 0, limit),
        "a fully routed board has nothing to recover"
    );
    assert!(
        !fanout_recovery_fires(&settings, false, 1, limit - 1),
        "the counter has not reached the limit"
    );
    settings.fanout.get_or_insert_with(Default::default).enabled = Some(false);
    assert!(
        !fanout_recovery_fires(&settings, false, 1, limit),
        "with fanout off there are no fanout vias to strip"
    );
}

#[test]
fn the_fanout_recovery_body_strips_tails_and_sets_the_one_shot_flag() {
    let source = include_str!("../src/pipeline/batch_loop.rs");
    let start = source
        .find("if fanout_recovery_fires(")
        .expect("the recovery guard");
    let body = &source[start..start + 1_400];
    assert!(
        body.contains("StopConnectionOption::None"),
        ":440 — `removeTails(NONE)`, i.e. fanout vias are stripped too"
    );
    assert!(
        body.contains("fanout_recovery_applied = true"),
        ":445 — one shot"
    );
    assert!(
        body.contains("consecutive_no_improvement_passes = 0"),
        ":444 — the counter restarts, so the loop gets another window"
    );
    assert!(
        !source.contains("is not ported yet"),
        "Task 10's loud fanout stub is gone, which is what makes the arm reachable"
    );
}
