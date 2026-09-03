//! Plan 7 Task 12 — `BatchFanout`'s pass loop (`BatchFanout.java:81-163`), `fanoutPass`
//! (`:166-506`), the two progress publishers (`:508-576`) and `AutorouteBatchLoop`'s fanout
//! pre-pass (`AutorouteBatchLoop.java:83-218`).
//!
//! # What is pinned here, and what is pinned by the drivers
//!
//! The whole-board evidence is `scripts/differential/run.sh p7t5 <dsn> 0 outer_first pass|board`
//! — one whole `fanoutPass` and then the whole `fanoutBoard`, each transcribed *and* called for
//! real, against the HEAD jar on eight DSNs — and `run.sh p7t9 <dsn> 1 router+fanout`, which puts
//! the pre-pass in front of the routing stage and compares the finished board. That is where "the
//! loop agrees with Java" is established.
//!
//! This file pins the five things a corpus run cannot show, all of them arms of the loop that no
//! fixture reaches:
//!
//! * the **oscillation detector** and the **hash stop** ([`FanoutLoopState`]), which need four
//!   passes with an identical `(routedCount, viaCount)` pair and a pass that routes something
//!   without moving the board's hash;
//! * the **`canUseVias` gate**'s `net == null` fall-through (**quirk #223**), which needs an SMD
//!   pin whose net number names no net;
//! * `ripupAllowed = false`, which no `DefaultSettings` run produces;
//! * `TextManager.parseTimespanString`'s grammar (**quirk #224**), whose two documented example
//!   values both answer `null`;
//! * the batch loop's one-shot **fanout recovery** guard, which needs eight passes of a
//!   stagnating board with fanout on.

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

// =================================================================================================
// Fixtures
// =================================================================================================

/// `p7t5`'s default stem: ten net-carrying SMD pins over three components, nine of which escape.
const RPI: &str = "fixtures/Issue143-rpi_splitter.dsn";

/// One of the two corpus DSNs with **no SMD pins at all** (Task 11 report §9), which is what
/// `AutorouteBatchLoop.java:90-91`'s skip needs.
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

/// `P7T5.buildSettings` — the headless ladder's priority-0 source, plus ruling AI's knob: the
/// per-pin fanout budget goes off through `settings.fanout.maxMillisecondsPerPin`, which is where
/// `fanoutPass:231-232` builds its `TimeLimit` from.
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

/// A sink that keeps every event, so `publishProgress`' reduction can be read back.
#[derive(Default)]
struct Recorder {
    events: Vec<RoutingEvent>,
}

impl ProgressSink for Recorder {
    fn on_event(&mut self, event: &RoutingEvent) {
        self.events.push(event.clone());
    }
}

// =================================================================================================
// `fanoutBoard`'s pass loop — BatchFanout.java:110-157
// =================================================================================================

/// `AutorouteBatchLoop.java:90-92` — "the fanout stage is enabled but skipped because the board
/// has no SMD pins". The guard reads the **unfiltered** `getSmdPins()`, so it is a property of the
/// board and not of `BatchFanout`'s net filter.
///
/// This is also the test that would have failed on Task 10's loud `assert!`: a `run` with fanout
/// enabled used to panic, and now it routes.
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

/// The batch loop's pre-pass on a board that **does** have SMD pins: `:123-173` runs and `:173`
/// wires `router.fanoutTimedOut`. `p7t9 router+fanout` is the byte-level version of this.
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

/// `:125-127` — a pass that routes nothing ends the loop, and the pass is still counted
/// (`completedPasses++` at `:124` runs first).
///
/// The JVM's answer for this stem is in `p7t5 board`'s transcript: pass 0 routes 9, pass 1 routes
/// 0, `completedPassCount = 2`.
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

/// `:128-148` — the oscillation detector. Four passes with the same `(routedCount, viaCount)`
/// pair end the loop; the first only seeds `previousBoardState`.
#[test]
fn three_identical_board_states_end_the_loop() {
    let mut state = FanoutLoopState::new(1);
    // A hash that moves on every pass, so only the oscillation arm can fire.
    let mut hash = 1_u64;
    let mut next_hash = || {
        hash += 1;
        hash
    };

    // Pass 1 seeds `previousBoardState` (`:146-147`).
    assert_eq!(state.after_pass(5, 3, false, &mut next_hash), None);
    assert_eq!(state.identical_passes, 0);
    // Passes 2, 3 and 4 repeat it.
    assert_eq!(state.after_pass(5, 3, false, &mut next_hash), None);
    assert_eq!(state.identical_passes, 1);
    assert_eq!(state.after_pass(5, 3, false, &mut next_hash), None);
    assert_eq!(state.identical_passes, 2);
    assert_eq!(
        state.after_pass(5, 3, false, &mut next_hash),
        Some(FanoutStop::Stagnated),
        ":135-145 — `identicalPasses >= stagnationPassLimit`"
    );
}

/// Quirk **#222**, the counting half: `stagnationPassLimit` is a **repeat** count, so the message
/// at `:141-146` ("no progress for 3 consecutive passes") is off by one — it takes four.
#[test]
fn the_stagnation_limit_is_a_repeat_count_so_it_takes_four_identical_passes() {
    let mut state = FanoutLoopState::new(1);
    let mut hash = 1_u64;
    let mut next_hash = || {
        hash += 1;
        hash
    };
    let mut identical_passes_seen = 0;
    loop {
        identical_passes_seen += 1;
        if state.after_pass(2, 7, false, &mut next_hash).is_some() {
            break;
        }
        assert!(identical_passes_seen < 10, "the detector must fire");
    }
    assert_eq!(
        identical_passes_seen, 4,
        "three repeats after the seeding pass, not three passes in total"
    );
}

/// Quirk **#222**, the packing half: `((long) routedCount << 32) ^ board.getVias().size()` is a
/// summary of two numbers, so two passes that escaped *different* pins in *different* places
/// collide whenever they escaped the same count and left the same via count.
///
/// The last case pins the **narrowing**, not a reachable Java defect: `via_count` is a `usize`
/// here and an `int` there, so the port narrows to `i32` before the widening XOR and lands where
/// Java's would. No board reaches two billion vias.
#[test]
fn the_board_state_packs_the_routed_count_above_the_via_count() {
    // `:133`, evaluated by hand.
    assert_eq!(FanoutLoopState::board_state(0, 0), 0);
    assert_eq!(FanoutLoopState::board_state(1, 0), 1 << 32);
    assert_eq!(FanoutLoopState::board_state(9, 9), (9_i64 << 32) ^ 9);
    // The collision the quirk names: nothing about *which* pins escaped survives the packing.
    assert_eq!(
        FanoutLoopState::board_state(4, 12),
        FanoutLoopState::board_state(4, 12)
    );
    // And the sign extension `int` -> `long` the XOR performs, which a via count above `2^31`
    // would reach on a board nobody builds.
    assert_eq!(
        FanoutLoopState::board_state(0, 0x8000_0000),
        i64::from(i32::MIN)
    );
}

/// `:152-156` — controller ruling AH's first decision site: an unchanged board hash ends the loop.
///
/// The port compares two [`Board::structural_hash`] values where Java compares two MD5 strings;
/// the **decision** is what `p7t5 board`'s `HASHEQ` line compares, and the warm-mode caveat is on
/// [`BatchFanout::fanout_board`].
#[test]
fn an_unchanged_hash_ends_the_loop() {
    let mut state = FanoutLoopState::new(0xDEAD_BEEF);
    assert_eq!(
        state.after_pass(3, 1, false, || 0xDEAD_BEEF),
        Some(FanoutStop::UnchangedHash)
    );

    // …and a hash that moved does not, and is remembered for the next pass.
    let mut state = FanoutLoopState::new(0xDEAD_BEEF);
    assert_eq!(state.after_pass(3, 1, false, || 0x0BAD_F00D), None);
    assert_eq!(state.last_board_hash, 0x0BAD_F00D);
    assert_eq!(
        state.after_pass(4, 2, false, || 0x0BAD_F00D),
        Some(FanoutStop::UnchangedHash),
        "the second pass compares against the first pass's hash, not the initial one"
    );
}

/// The four stops are tested in Java's order, and the order is load-bearing: a pass that routed
/// nothing never reaches the oscillation detector, and a timed-out pass never reaches the hash.
#[test]
fn the_four_stops_are_tested_in_javas_order() {
    // `:125` before `:133`: `routedCount == 0` wins even when the state repeats.
    let mut state = FanoutLoopState::new(1);
    assert_eq!(
        state.after_pass(0, 0, true, || 1),
        Some(FanoutStop::NothingRouted)
    );
    assert_eq!(
        state.identical_passes, 0,
        "the detector is not even consulted"
    );

    // `:149` before `:152`: a timed-out pass stops before the hash is taken.
    let mut state = FanoutLoopState::new(7);
    let mut hash_taken = false;
    let stop = state.after_pass(1, 0, true, || {
        hash_taken = true;
        7
    });
    assert_eq!(stop, Some(FanoutStop::TimedOut));
    assert!(!hash_taken, ":153 is never evaluated");
}

// =================================================================================================
// `fanoutPass`' two gates — BatchFanout.java:173-183, :238-259
// =================================================================================================

/// `:173` — `settings.getStartRipupCosts() * (passNo + 1)`, and `:179-183`'s `-1` for
/// "no ripup".
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

    // `:179-183` — "negative ripup costs signal 'no ripup' to RoutingBoard.fanout()".
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

    // An absent flag and an absent block both mean **allowed** (`:179-181`) — the opposite
    // default from `isFanoutEnabled`.
    settings
        .fanout
        .get_or_insert_with(Default::default)
        .ripup_allowed = None;
    assert_eq!(fanout_ripup_costs(&settings, 0), start);
    settings.fanout = None;
    assert_eq!(fanout_ripup_costs(&settings, 0), start);
}

/// Quirk **#223** — `:239`'s `if (net != null)` wraps the **whole** `canUseVias` gate, so a pin
/// whose net number names no net is never checked and is fanned out regardless.
#[test]
fn a_null_net_pin_skips_the_via_gate() {
    if !parity::require_java_dir() {
        return;
    }
    let mut board = load_board(RPI);
    let mut settings = build_settings(&board);

    // Net 0 is `Nets.get`'s own "no net" answer, and a number above the net count is the other
    // way a pin can name nothing.
    assert!(board.rules.nets.get(0).is_none());
    assert!(board.rules.nets.get(9_999).is_none());
    assert!(
        fanout_pin_can_use_vias(&board, &settings, 0),
        "the gate is skipped, so the pin is fanned out"
    );
    assert!(fanout_pin_can_use_vias(&board, &settings, 9_999));

    // A net that *does* exist is checked, and this stem's class has vias.
    assert!(board.rules.nets.get(1).is_some());
    assert!(fanout_pin_can_use_vias(&board, &settings, 1));

    // Empty its class's via rule and turn the fallback off: now the same pin is skipped —
    // which is the asymmetry the quirk records, because the *unknown* net still is not.
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
    assert!(
        fanout_pin_can_use_vias(&board, &settings, 9_999),
        "and the pin whose net does not exist is still not checked"
    );

    // With the fallback back on, `:242-245`'s `viaRules.firstElement()` carries this board's vias.
    settings
        .fanout
        .get_or_insert_with(Default::default)
        .fallback_to_board_vias = Some(true);
    assert!(board.rules.via_rules[0].via_count() > 0);
    assert!(fanout_pin_can_use_vias(&board, &settings, 1));
}

// =================================================================================================
// `maxItems`, the deadline and the progress reduction
// =================================================================================================

/// `:117-122` and `:222-230` — the same four-term gate, and `maxItems <= 0` is "no limit".
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

/// `:111-112`/`:396` — the per-stage deadline, and Task 4's split: it must **not** touch the job
/// stop flag. `DefaultSettings` ships no `fanout.timeout`, so the flag is never set on a
/// corpus run.
#[test]
#[cfg_attr(debug_assertions, ignore)]
fn the_stage_deadline_never_touches_the_stop_flag() {
    if !parity::require_java_dir() {
        return;
    }
    let mut board = load_board(RPI);
    let mut settings = build_settings(&board);
    // "-1" seconds: a deadline already in the past when `fanoutBoard` computes it (`:98`).
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

/// `:544-576` reduced to [`RoutingEvent::FanoutProgress`] (ruling AK): the pass number is 1-based
/// (`:561`), and the pass-start tick reports every pin still to go.
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

/// Ruling 11: the sink is an observer and never an input — a run with a recording sink must
/// answer the same board as one with [`NoopProgressSink`].
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

// =================================================================================================
// `TextManager.parseTimespanString` — quirk #224
// =================================================================================================

/// Quirk **#224**, as the **jar** behaves: `FanoutSettings.timeout`'s own javadoc gives `"5m"` and
/// `"300s"` as its examples and `parseTimespanString` parses **neither**, so the fanout stage
/// silently runs with no timeout. Only the colon forms work.
///
/// Every expectation here was measured against a JDK 25 `Duration.parse` rather than read off the
/// regex; see the method's doc for the shape of the grammar. Plan 9 Task 1 fixed the port's own
/// parser and this test moved to `parse_timespan_seconds_java`, the function that still answers
/// exactly what the jar answers — the defect is still a fact about the jar, and the fix's whole
/// evidence is the gap between this test and
/// [`the_documented_timeout_spellings_parse`].
#[test]
fn the_documented_fanout_timeout_examples_both_parse_to_nothing_in_java() {
    // The javadoc's two examples.
    assert_eq!(parse_timespan_seconds_java("5m"), None);
    assert_eq!(parse_timespan_seconds_java("300s"), None);
    // …and the forms that do work.
    assert_eq!(parse_timespan_seconds_java("300"), Some(300));
    assert_eq!(parse_timespan_seconds_java("1:00"), Some(60));
    assert_eq!(parse_timespan_seconds_java("01:02:03"), Some(3723));
    assert_eq!(parse_timespan_seconds_java("00:00:30"), Some(30));
}

/// The rest of Java's grammar, one measured case per branch of
/// `convertFromTimespanToDurationFormat` and of `Duration.parse`.
#[test]
fn the_timespan_grammar_matches_the_jvms() {
    // `:84-86` — blank.
    assert_eq!(parse_timespan_seconds_java(""), None);
    assert_eq!(parse_timespan_seconds_java("   "), None);
    // Signs, and the negative deadline that puts the stage past its budget before it starts.
    assert_eq!(parse_timespan_seconds_java("-5"), Some(-5));
    assert_eq!(parse_timespan_seconds_java("+5"), Some(5));
    assert_eq!(parse_timespan_seconds_java("007"), Some(7));
    // `Duration.getSeconds()` is the whole-second field of a normalised pair, so a **negative**
    // fraction carries one second down.
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
    // `String.split` drops trailing empty parts, so these change arity rather than failing.
    assert_eq!(parse_timespan_seconds_java("1:"), Some(1));
    assert_eq!(parse_timespan_seconds_java("2:3:"), Some(123));
    // …and these do fail: four parts, an empty interior part, ten fraction digits, whitespace.
    assert_eq!(parse_timespan_seconds_java("1:2:3:4"), None);
    assert_eq!(parse_timespan_seconds_java("::"), None);
    assert_eq!(parse_timespan_seconds_java("1::2"), None);
    assert_eq!(parse_timespan_seconds_java("1.0000000001"), None);
    assert_eq!(parse_timespan_seconds_java("5 "), None);
    assert_eq!(parse_timespan_seconds_java("1.S"), None);
    // Overflow is a `DateTimeParseException` on the JVM too, i.e. `null` rather than a throw.
    assert_eq!(
        parse_timespan_seconds_java("9223372036854775807"),
        Some(i64::MAX)
    );
    assert_eq!(parse_timespan_seconds_java("9223372036854775807:0:0"), None);
}

// =================================================================================================
// #224's fix — the documented spellings parse, and an unreadable one is refused
// =================================================================================================

/// The brief's first named test: the two spellings `FanoutSettings.timeout`'s javadoc promises
/// actually work, and an unparseable string is an **error** rather than `None`-then-unbounded.
///
// fixed: T1 (#224) — the acceptance half. `5m` and `300s` are the javadoc's own examples
// (`settings/FanoutSettings.java:98-101`); `1h30m` is the composite form the brief names.
#[test]
fn the_documented_timeout_spellings_parse() {
    assert_eq!(parse_timespan_seconds("5m"), Ok(Some(300)));
    assert_eq!(parse_timespan_seconds("300s"), Ok(Some(300)));
    assert_eq!(parse_timespan_seconds("1h30m"), Ok(Some(5400)));

    // The rest of the suffix grammar: each unit alone, all three together, and a sign.
    assert_eq!(parse_timespan_seconds("2h"), Ok(Some(7200)));
    assert_eq!(parse_timespan_seconds("1h2m3s"), Ok(Some(3723)));
    assert_eq!(parse_timespan_seconds("0m"), Ok(Some(0)));
    assert_eq!(parse_timespan_seconds("-5m"), Ok(Some(-300)));

    // Order matters, so a typo is a refusal rather than a silently different number.
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

    // **Every string the jar accepted still parses to the same number.** #224 only adds
    // acceptances; it never changes an answer.
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

    // A blank string is still "no timeout" — the one silent arm, and Java's own `:84-86`. It has
    // to stay distinguishable from an unreadable one, or the refusal below would fire on every
    // run that simply did not set a timeout.
    assert_eq!(parse_timespan_seconds(""), Ok(None));
    assert_eq!(parse_timespan_seconds("   "), Ok(None));

    // The brief's own example of the refusal.
    let error = parse_timespan_seconds("banana").expect_err("`banana` is not a timespan");
    assert_eq!(error.input, "banana");
    assert!(
        error.to_string().contains("banana"),
        "the refusal must name the string the operator wrote: {error}"
    );

    // And every string the jar swallowed into an unbounded run is now an error.
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

/// The brief's second named test: the run fails **at the setting**, with the string named, rather
/// than running with no timeout at all.
///
/// `BatchFanout.fanoutBoard:91-100` reads `settings.fanout.timeout`, and on a
/// `DateTimeParseException` Java's `:91-93` swallows it, answers `null`, and leaves `deadlineMs`
/// unset so `:111-116`/`:396` never fire. That is the defect: the operator asked for a bounded
/// stage and got an unbounded one, silently.
///
// fixed: T1 (#224) — the refusal half, on the stage the register names.
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

    // The documented spelling on the same setting runs, so the refusal is about the string and
    // not about the stage.
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

// =================================================================================================
// `AutorouteBatchLoop`'s one-shot fanout recovery — AutorouteBatchLoop.java:435-454
// =================================================================================================

/// `:435-439`'s four-term guard, one term at a time. The arm is one-shot because
/// `fanoutRecoveryApplied` is the second term and the body sets it (`:445`).
///
/// **This is the guard, not the run.** Firing it for real needs eight passes of a board that
/// stagnates with fanout on, which no Plan 7 fixture produces; the body is checked by the source
/// shape below and by `p7t9 router+fanout`, which puts the arm on a live path for the first time.
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
    // Each term alone blocks it.
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

/// The body of `:440-445`, by source shape: `removeTails(NONE)` — fanout vias included — then the
/// two recomputed statistics, then the two counter resets, then the one-shot flag.
///
/// Task 10's precedent for an arm the corpus cannot reach. What changed in Task 12 is that the
/// arm is now *reachable*: the loud `assert!` that made `is_fanout_enabled()` false on every path
/// is gone.
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
