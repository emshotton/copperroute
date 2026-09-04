//! Plan 7 Task 10 — `AutorouteBatchLoop::run`, the pass loop, the best-board policy and the two
//! stagnation detectors (`autoroute/pipeline/AutorouteBatchLoop.java:37-588`).
//!
//! # What is pinned here, and what is pinned by `p7t9`
//!
//! The whole-board evidence is the differential driver, not this file:
//! `scripts/differential/run.sh p7t9 <dsn> <maxPasses> router-only` compares a line-for-line
//! transcription of `run`'s body — the per-pass `PassRecord` tuple and every decision arm — plus
//! the real `BatchAutorouter.runBatchLoop()` and the final board in `P6T15aProbe`'s polyline
//! format, against this port, on five corpus DSNs at `maxPasses ∈ {1, 2, 8}`. That is where "the
//! loop routes the same board" is established.
//!
//! This file pins the four things a corpus run cannot show:
//!
//! * the **error** boundary (`:44-56`) — no corpus stem has its layers switched off;
//! * the two **counter-intuitive reports** — quirk #214's normal finish, which said `CANCELLED`
//!   until Plan 9 Task 9 gave the loop an exit reason, and `maxPasses = 0` meaning *unlimited*
//!   (quirk #140);
//! * the arms that are **unreachable on the corpus**: the rank break (quirk #217 — unreachable
//!   *everywhere*, see below) and the final best-board swap, which prints `swapped=false` on every
//!   `p7t9` run because no corpus history ever holds a strictly better board than the one the run
//!   ended on;
//! * the two **gates** that need eight real passes to reach.
//!
//! # The routing tests are release-only
//!
//! **Ten of the sixteen** tests route a real board through the real pass runner or the real loop.
//! Unoptimised that is minutes; in release the whole file is about 40 s. They carry
//! `#[cfg_attr(debug_assertions, ignore)]`, Plan 3's convention — run them with
//! `cargo test --release -p fr-router --test batch_loop`, **without** `--ignored` (the attribute
//! does not apply in a release build, so `--ignored` would filter them out instead).

use std::path::PathBuf;

use fr_board::prelude::*;
use fr_router::RouterError;
use fr_router::pipeline::batch_loop::{
    BOARD_RANK_LIMIT, STAGNATION_PASS_LIMIT, STOP_AT_PASS_MINIMUM, STOP_AT_PASS_MODULO,
    final_best_board_swap, rank_limit_exceeded, restore_gate, stagnation_guard, stagnation_step,
};
use fr_router::pipeline::{
    AutorouteBatchLoop, BatchLoopExit, BoardHistory, NamedAlgorithmType, NoopProgressSink,
    ProgressSink, RouterBudget, RouterStop, RoutingEvent, StagnationStep, TaskState,
};
use fr_router::score::BoardStatistics;
use fr_settings::sources::DefaultSettings;
use fr_settings::{HostEnvironment, RouterSettings, ScoringSettings, SettingsSource};

// =================================================================================================
// The harness
// =================================================================================================

/// `Issue143-rpi_splitter.dsn` — the smallest corpus board that actually routes (eight
/// connections), and the one every `p7t*` driver defaults to.
const RPI: &str = "fixtures/Issue143-rpi_splitter.dsn";

/// `Issue649-kicad_ecc83-pp_input_board_v1.dsn` — a corpus board the port still routes to
/// **completion**.
///
/// It is here because of the M1 accept wave (ruling BV). Plan 9 Task 2's R1 (#293) and R2 (#294)
/// leave `rpi_splitter` with two connections that no pass can close, so its loop now ends at the
/// stagnation detector — `CANCELLED` — and it can no longer witness the `FINISHED` arm. This board
/// can: `maxPasses = 0` runs two passes, routes every connection and leaves the stop flag down.
const ECC83: &str = "fixtures/Issue649-kicad_ecc83-pp_input_board_v1.dsn";

/// `Issue026-J2_reference.dsn` — a corpus board whose score still **rises on two consecutive
/// passes** (0 -> 848.476 7 -> 909.082 46), giving the two `BoardHistory` tests below **three**
/// distinct boards out of the unrouted board plus two real passes.
///
/// Here because of the M1 accept wave. Those two tests need *distinct* boards out of consecutive
/// passes, and after R1 (#293) / R2 (#294) `rpi_splitter` converges after pass 1: every later pass
/// hands back a structurally identical board, which `BoardHistory::add` deduplicates — so the
/// fixture, not the assertion, is what stopped working.
///
/// **Why this board and not a bigger one.** The wave first re-pointed both tests at
/// `Issue103-Board-Unrouted.dsn`, which also works and costs **196 s** and **141 s** in the
/// release lane against `rpi_splitter`'s 0.125 s and 0.078 s — 337 s that every later task in the
/// plan would pay, and enough to trip nextest's `SLOW` threshold twice. Both test bodies were then
/// re-run verbatim against this board: **0.20 s** and **0.18 s**, every assertion holding. Pick
/// the smallest fixture that still makes the two orders disagree.
const J2: &str = "fixtures/Issue026-J2_reference.dsn";

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

/// `P7T9.buildSettings` — the headless ladder's priority-0 source plus the driver's three knobs.
///
/// `fanout.enabled = false` is not optional: `DefaultSettings` turns fanout **on**, and
/// [`AutorouteBatchLoop::run`] asserts it is off until Plan 7 Task 12 lands the pre-pass
/// (ruling B1).
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

/// A [`ProgressSink`] that keeps every event, so the three `fireTaskStateChangedEvent` calls
/// (`:53-54`, `:58-59`, `:279-280`, `:572-584`) can be read back.
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
    fn states(&self) -> Vec<TaskState> {
        self.events
            .iter()
            .filter_map(|e| match e {
                RoutingEvent::TaskStateChanged { algorithm, state } => {
                    assert_eq!(
                        *algorithm,
                        NamedAlgorithmType::Router,
                        "AutorouteBatchLoop fires as the ROUTER, never the OPTIMIZER"
                    );
                    Some(*state)
                }
                _ => None,
            })
            .collect()
    }
}

fn scoring_of(settings: &RouterSettings) -> ScoringSettings {
    settings
        .scoring
        .clone()
        .expect("DefaultSettings always writes a scoring block")
}

// =================================================================================================
// :44-56 — ruling 7's sole new recovery boundary
// =================================================================================================

/// `:44-56`. With no layer that is both **active in the settings** and a **signal layer**, Java
/// fires a `TaskState.CANCELLED` event (`:53-54`) and then throws `IllegalArgumentException`
/// (`:55`). `RoutingPipeline.run` does not catch it, so plan-7 ruling 7 makes this the port's one
/// *propagating* boundary: [`RouterError::NoRoutableLayer`].
///
/// The event comes **first**, and the test asserts both halves — a port that returned the error
/// without firing would leave an API consumer with no state transition at all.
#[test]
fn a_board_with_no_signal_layer_errors_and_reports_cancelled() {
    if !parity::require_java_dir() {
        return;
    }
    let mut board = load_board(RPI);
    let mut settings = build_settings(&board, 1);
    // `:46`'s `settings.getLayerActive(i)` — switch every layer off, which is what the CLI's
    // `-dl` does. `rpi_splitter` is a two-layer board and both of its layers *are* signal layers,
    // so this is the only way to reach `:51` on a real fixture.
    for layer in 0..settings.get_layer_count() {
        settings.set_layer_active(layer, false);
    }

    let stop = RouterStop::new();
    let mut sink = Recorder::default();
    let result = AutorouteBatchLoop::run(
        &mut board,
        &settings,
        &stop,
        RouterBudget::disabled(),
        &mut sink,
    );

    assert!(
        matches!(result, Err(RouterError::NoRoutableLayer)),
        "AutorouteBatchLoop.java:55 throws IllegalArgumentException; ruling 7 propagates it, \
         got {result:?}"
    );
    assert_eq!(
        sink.states(),
        vec![TaskState::Cancelled],
        "AutorouteBatchLoop.java:53-54 fires CANCELLED before the throw, and nothing else runs"
    );
}

/// The other half of `:46`: a layer that is **active** but not a **signal** layer does not make
/// the board routable. `anyRoutable` needs both conjuncts, and the loop over
/// `settings.getLayerCount()` indexes `board.layerStructure.layers`, so the two must also agree on
/// layer order — deactivating layer 0 of a board whose layer 1 is a plane is the case that tells
/// the two conjuncts apart.
///
/// `Issue269-caniot-tiny-arm.dsn` is the fixture: `F.Cu` is `(type signal)` and `B.Cu` is
/// `(type power)`, the only shape in the corpus where switching **one** layer off is enough.
///
/// Release-only, like the other routing tests: the positive half runs a real pass over a real
/// board, which is what makes it evidence that `:46`'s **first** conjunct is satisfied there.
#[test]
#[cfg_attr(debug_assertions, ignore)]
fn an_active_non_signal_layer_is_not_routable() {
    if !parity::require_java_dir() {
        return;
    }
    const CANIOT: &str = "fixtures/Issue269-caniot-tiny-arm.dsn";
    let mut board = load_board(CANIOT);
    let signal: Vec<bool> = board
        .layer_structure()
        .layers
        .iter()
        .map(|l| l.is_signal)
        .collect();
    assert_eq!(
        signal,
        vec![true, false],
        "the fixture must keep one signal layer and one plane, or this test proves nothing"
    );

    // Both layers active: `:46` finds layer 0 and the loop runs.
    let ok = build_settings(&board, 1);
    assert!(
        AutorouteBatchLoop::run(
            &mut board.deep_copy(),
            &ok,
            &RouterStop::new(),
            RouterBudget::disabled(),
            &mut NoopProgressSink,
        )
        .is_ok(),
        "layer 0 is active and is a signal layer"
    );

    // Only the plane left active: the first conjunct now holds for layer 1 and the second does
    // not, so `anyRoutable` stays false.
    let mut plane_only = build_settings(&board, 1);
    plane_only.set_layer_active(0, false);
    let result = AutorouteBatchLoop::run(
        &mut board,
        &plane_only,
        &RouterStop::new(),
        RouterBudget::disabled(),
        &mut NoopProgressSink,
    );
    assert!(
        matches!(result, Err(RouterError::NoRoutableLayer)),
        "AutorouteBatchLoop.java:46 wants layerActive(i) AND layers[i].isSignal, got {result:?}"
    );
}

// =================================================================================================
// :571-585 — quirk #214, the headline row
// =================================================================================================

/// **Quirk #214, fixed in Plan 9 Task 9.** A run that stops because it reached its pass budget
/// reports [`TaskState::Finished`].
///
/// Before the fix it reported [`TaskState::Cancelled`], because `:271`'s
/// `requestStopAutoRouter()` has already raised the flag `:571` tests — and four sibling arms do
/// the same, so the *only* reachable `FINISHED` was the `while` head's own
/// `continueAutorouting == false`. The run did exactly what it was asked to do and nothing went
/// wrong — one pass of `rpi_splitter` takes it from five incomplete connections to two — and an
/// API consumer watching `TaskStateChangedEvent` could not tell that from a user cancellation.
///
/// The fix carries the **reason** out of the loop ([`BatchLoopExit`]) and takes `:571-585`'s
/// decision on it. This test measures both halves: the reason on a real capped run, and the
/// state each of the five doors maps to on a constructed exit — including that
/// [`BatchLoopExit::Cancelled`] still reports `CANCELLED` and, with the job clock expired,
/// `TIMED_OUT`.
#[test]
#[cfg_attr(debug_assertions, ignore)]
fn a_normal_finish_reports_finished() {
    // The five doors, each on a constructed exit. `Cancelled` is the only one that is not a
    // finish, and it is the only one a `timed_out` job clock can turn into `TIMED_OUT`.
    assert_eq!(
        BatchLoopExit::Completed.task_state(false),
        TaskState::Finished
    );
    assert_eq!(
        BatchLoopExit::MaxPasses.task_state(false),
        TaskState::Finished
    );
    assert_eq!(
        BatchLoopExit::NoImprovement.task_state(false),
        TaskState::Finished
    );
    assert_eq!(
        BatchLoopExit::Stagnation.task_state(false),
        TaskState::Finished
    );
    assert_eq!(
        BatchLoopExit::Cancelled.task_state(false),
        TaskState::Cancelled
    );
    // `:578-584` — the job clock, and only on the one door a clock can produce.
    assert_eq!(
        BatchLoopExit::Cancelled.task_state(true),
        TaskState::TimedOut
    );
    for door in [
        BatchLoopExit::Completed,
        BatchLoopExit::MaxPasses,
        BatchLoopExit::NoImprovement,
        BatchLoopExit::Stagnation,
    ] {
        assert_eq!(
            door.task_state(true),
            TaskState::Finished,
            "{door:?} is a finish whatever the job clock says — only a cancellation is timed out"
        );
    }

    if !parity::require_java_dir() {
        return;
    }
    let mut board = load_board(RPI);
    let settings = build_settings(&board, 1);
    let stop = RouterStop::new();
    let mut sink = Recorder::default();

    let result = AutorouteBatchLoop::run(
        &mut board,
        &settings,
        &stop,
        RouterBudget::disabled(),
        &mut sink,
    )
    .expect("rpi_splitter has two signal layers");

    assert_eq!(
        result.exit(),
        BatchLoopExit::MaxPasses,
        "`:268-273` is the door a `--max-passes 1` run leaves by"
    );
    assert_eq!(
        result.state,
        TaskState::Finished,
        "fixed: T9 (#214) — a run that did exactly what it was asked has FINISHED"
    );
    assert!(
        !result.continue_routing,
        ":587 is `!isStopAutoRouterRequested()`, and :271 requested it — the flag is still raised, \
         which is what four other readers in the loop depend on"
    );
    // The full event tape: STARTED, one RUNNING per pass, then the final state. The `maxPasses`
    // break happens at the *top* of pass 2, before `:279`, so there is exactly one RUNNING.
    assert_eq!(
        sink.states(),
        vec![TaskState::Started, TaskState::Running, TaskState::Finished]
    );
    // And the pass really did route: one pass of rpi_splitter takes the board from five
    // incomplete connections to two. The point of the quirk is that this is a *successful* run.
    //
    // PORT-REGRESSION PIN — re-cut at the M1 accept wave (ruling BV). The jar-parity value was **1** — the JVM's, via `p7t9`. The port answers **2** since Plan 9 Task 2's
    // R1 (#293, the airline-first work list) and R2 (#294, the micro-neckdown floor): on this
    // board the pair trades one connection for legality. Accepted at M1 (ruling BV), so this
    // literal is now the **port's** own regression pin, not a jar-parity pin.
    assert_eq!(result.per_pass.len(), 1);
    assert_eq!(result.per_pass[0].incomplete_count, 2);
    assert!(
        result.per_pass[0].score > 0.0,
        "the FINISHED run routed a board: {:?}",
        result.per_pass[0]
    );
}

/// The complement of quirk #214: `FINISHED` is reachable, and only through the `while` head's own
/// `continueAutorouting == false` — a pass in which `runSingleThread` answered `false`, i.e. one
/// that routed and failed nothing.
///
/// `maxPasses = 0` is what lets the loop get there, which is the next test's subject.
///
/// PORT-REGRESSION PIN — the **board** was re-cut at the M1 accept wave (ruling BV), not a
/// number. The jar-parity fixture was `rpi_splitter`, which
/// reached `FINISHED` in two passes on the pre-R1/R2 port. It no longer can: R1 (#293) and R2
/// (#294) leave two connections that every pass fails, so the loop runs to the stagnation
/// detector and reports `CANCELLED` (measured: 18 passes, `incomplete_count` 2 throughout). The
/// **claim** — that `FINISHED` is reachable, and only that way — is unchanged and is measured on
/// [`ECC83`], which still routes to completion. Accepted at M1 (ruling BV).
#[test]
#[cfg_attr(debug_assertions, ignore)]
fn only_a_pass_that_routes_nothing_reaches_finished() {
    if !parity::require_java_dir() {
        return;
    }
    let mut board = load_board(ECC83);
    let settings = build_settings(&board, 0);
    let stop = RouterStop::new();
    let mut sink = Recorder::default();

    let result = AutorouteBatchLoop::run(
        &mut board,
        &settings,
        &stop,
        RouterBudget::disabled(),
        &mut sink,
    )
    .expect("the ecc83 board has two signal layers");

    assert_eq!(result.state, TaskState::Finished);
    assert!(
        result.continue_routing,
        ":587 answers true when nothing ever raised the flag"
    );
    assert_eq!(
        *sink.states().last().expect("a final state"),
        TaskState::Finished
    );
}

// =================================================================================================
// :221-223 and :268-270 — quirk #140
// =================================================================================================

/// **Quirk #140.** `maxPasses = 0` is **unlimited**, not "no passes".
///
/// Two lines make it so, and they disagree about what `0` means: `:221-223`'s `maxPasses >= 0`
/// lets `0` enable the router, and `:268-270`'s `maxPasses > 0` then refuses to compare it against
/// `currentPass` — so the `:271` break never fires and the loop runs until the board is done or a
/// stagnation detector stops it. A *negative* `maxPasses` disables the router outright.
///
/// Measured rather than argued: `0` runs strictly more passes than `1` on the same board.
#[test]
#[cfg_attr(debug_assertions, ignore)]
fn max_passes_zero_is_unlimited() {
    if !parity::require_java_dir() {
        return;
    }
    let run_with = |max_passes: i32| {
        let mut board = load_board(RPI);
        let settings = build_settings(&board, max_passes);
        let stop = RouterStop::new();
        let mut sink = NoopProgressSink;
        AutorouteBatchLoop::run(
            &mut board,
            &settings,
            &stop,
            RouterBudget::disabled(),
            &mut sink,
        )
        .expect("rpi_splitter has two signal layers")
    };

    let one = run_with(1);
    let unlimited = run_with(0);

    assert_eq!(one.per_pass.len(), 1, "maxPasses = 1 is one pass");
    assert!(
        unlimited.per_pass.len() > one.per_pass.len(),
        "maxPasses = 0 must not break at :271; it ran {} passes against {}",
        unlimited.per_pass.len(),
        one.per_pass.len()
    );
    // …and here it is the stagnation detector that stops it, not the board being done.
    //
    // PORT-REGRESSION PIN — re-cut at the M1 accept wave (ruling BV) and again by Plan 9 Task 9's
    // #214. On the pre-R1/R2 port `rpi_splitter` closed every connection and the `while` head fell
    // out on its own; R1 (#293) and R2 (#294) leave two connections no pass can route, so the run
    // walks 18 passes at a flat `incomplete_count` of 2 and the **stagnation** door ends it. Under
    // the jar and under the pre-T9 port that door reported `CANCELLED` (quirk #214); it is a door
    // the *router* chose, so it now reports `FINISHED` and names itself.
    assert_eq!(unlimited.exit(), BatchLoopExit::Stagnation);
    assert_eq!(unlimited.state, TaskState::Finished);
    // The two doors really are told apart — `maxPasses` is the caller's, stagnation is the
    // router's, and neither is a cancellation.
    assert_eq!(one.exit(), BatchLoopExit::MaxPasses);
    assert_eq!(one.state, TaskState::Finished);

    // The negative case, which is the other side of `:221-223`'s `>= 0`: the router is disabled,
    // so `continueAutorouting` starts false and the `while` head never runs a pass.
    let disabled = run_with(-1);
    assert!(
        disabled.per_pass.is_empty(),
        ":221-223's `maxPasses >= 0` is false for -1, so isRouterEnabled is false"
    );
    assert_eq!(disabled.state, TaskState::Finished);
    assert_eq!(
        disabled.exit(),
        BatchLoopExit::Completed,
        "a loop that was never entered left by the `while` head's own first conjunct"
    );
}

// =================================================================================================
// :298-300 — the restore gate
// =================================================================================================

/// `:298-300`. The best-board restore is behind **two** gates, and the port's [`restore_gate`] is
/// their conjunction.
///
/// Reaching it through `run` would take eight real passes of a board that also fails to improve,
/// which no corpus stem does; the predicate is pinned directly instead, over the whole
/// neighbourhood of both constants.
#[test]
fn the_restore_gate_needs_eight_entries_and_a_pass_multiple_of_four() {
    assert_eq!(STOP_AT_PASS_MINIMUM, 8, "BatchAutorouter.java:46");
    assert_eq!(STOP_AT_PASS_MODULO, 4, "BatchAutorouter.java:49");

    // Not yet eight entries: the outer gate (`:298`) is false whatever the pass number is.
    for pass in [4, 8, 12, 16] {
        assert!(
            !restore_gate(7, pass, false),
            "bh.size() = 7 is below STOP_AT_PASS_MINIMUM at pass {pass}"
        );
    }

    // Eight entries, but the inner gate (`:299-300`) wants `pass % 4 == 0` **and** `pass >= 8`.
    // Pass 4 satisfies the modulo and fails the minimum — the `&&` is what makes 4 the one
    // multiple of four that does not fire.
    assert!(
        !restore_gate(8, 4, false),
        ":299 also demands currentPass >= 8"
    );
    for pass in [5, 6, 7, 9, 10, 11] {
        assert!(
            !restore_gate(8, pass, false),
            "pass {pass} is not a multiple of 4"
        );
    }
    for pass in [8, 12, 16, 40] {
        assert!(restore_gate(8, pass, false), "pass {pass} opens both gates");
    }

    // A raised stop flag satisfies **both** gates on its own (`:298` and `:300` each carry their
    // own `|| isStopAutoRouterRequested()`), which is what makes the restore run one last time on
    // the way out of the loop.
    assert!(restore_gate(0, 1, true), ":298 and :300's second disjunct");
    assert!(
        !restore_gate(0, 1, false),
        "and only with the flag raised — an empty history at pass 1 opens neither gate"
    );
}

// =================================================================================================
// :317-320 — quirk #217, the break that cannot fire
// =================================================================================================

/// **Quirk #217: `:317-320` is dead code, and the comment on its constant has the reason
/// backwards.**
///
/// `BOARD_RANK_LIMIT` is *defined as* `BoardHistory.MAX_HISTORY_SIZE` (`BatchAutorouter.java:40`),
/// and `BoardHistory.getRank` answers a 1-indexed position in a list `add` never lets grow past
/// that same cap. So `getRank`'s range is `{-1} ∪ 1..=MAX_HISTORY_SIZE` and
/// `rank > MAX_HISTORY_SIZE` has no solution.
///
/// `BatchAutorouter.java:38-39` says the limit "Must not exceed `BoardHistory.MAX_HISTORY_SIZE`
/// **so the check can actually fire**". For it to fire the limit must be *strictly less than* the
/// cap; equal is the one value that guarantees it never does.
///
/// The test does four things: pins the identity, shows a *full* history's ranks are all inside
/// the limit, exercises the predicate one above it so the arithmetic is checked rather than
/// merely reported dead, and asserts that **the loop no longer calls it**.
///
/// fixed: T9 (#217) — the decision recorded for this row is **delete the branch**. The register
/// offers two options and the other one — setting the limit strictly below the cap — would make a
/// documented stop reason reachable and stop runs *earlier* than any freerouting ever has. That
/// is a product decision with its own A/B, not a cleanup, and it is deliberately not taken here.
/// The predicate survives its call site so that the row's arithmetic and its proof stay in one
/// place; the last assertion is what keeps the deletion from silently coming back.
#[test]
fn the_rank_limit_can_never_fire() {
    // The identity that makes it dead (`BatchAutorouter.java:40`).
    assert_eq!(
        BOARD_RANK_LIMIT,
        BoardHistory::MAX_HISTORY_SIZE,
        "BatchAutorouter.java:40 — BOARD_RANK_LIMIT *is* the cap"
    );

    // `getRank`'s "not found" sentinel. `-1 > 30` is false, so a board the history cannot see
    // does not break the loop either.
    assert!(!rank_limit_exceeded(-1));

    // The whole in-range band.
    for rank in 1..=BOARD_RANK_LIMIT {
        assert!(
            !rank_limit_exceeded(rank as i32),
            "rank {rank} is inside a history capped at {BOARD_RANK_LIMIT}"
        );
    }

    // And the arithmetic itself is right — it just has no input that reaches it.
    assert!(rank_limit_exceeded(BOARD_RANK_LIMIT as i32 + 1));

    // fixed: T9 (#217) — and the loop does not call it any more. Counted on a whitespace-
    // normalised copy so rustfmt cannot make this pass or fail for the wrong reason; the only
    // occurrences left in the file are this function's own declaration and its doc.
    let source = include_str!("../src/pipeline/batch_loop.rs");
    let flat = source.split_whitespace().collect::<Vec<_>>().join(" ");
    assert_eq!(
        flat.matches("if rank_limit_exceeded(").count(),
        0,
        "the rank break is deleted from `run`; re-enabling it is a product decision with its own \
         A/B, not a cleanup (quirk #217)"
    );
    assert!(
        flat.contains("pub fn rank_limit_exceeded(rank: i32) -> bool"),
        "…and the predicate survives its call site, so the row's arithmetic stays testable"
    );
}

/// The other half of quirk #217's argument, measured rather than reasoned: a history filled to its
/// cap answers ranks in `1..=cap`, never `cap + 1`.
///
/// Demonstrated at `cap = 3` through `BoardHistory::with_capacity` — Java's package-private
/// test constructor (`BoardHistory.java:42-45`) — because the argument is about the *cap*, not
/// about the number 30, and building thirty distinct real boards would test the router instead.
/// `add`'s eviction is what enforces it (`BoardHistory.java:53-76`) and it does not depend on the
/// cap's value.
///
/// PORT-REGRESSION PIN — the **board** was re-cut at the M1 accept wave (ruling BV), not a
/// number. The jar-parity fixture was `rpi_splitter`, whose unrouted board plus three
/// passes were four distinct boards. After R1 (#293) and R2 (#294) that board converges after
/// pass 1, so passes 2 and 3 hand back a structurally identical board, `add` deduplicates them
/// and the list never reaches its cap — `bh.size()` answered **2** against the cap of 3. The
/// eviction claim is unchanged; the fixture that can still exercise it is [`J2`], whose unrouted
/// board and first two passes are **three** distinct boards. Accepted at M1 (ruling BV).
///
/// **The cap is 2 here, not 3, and the difference is the whole assertion.** Three distinct boards
/// against a cap of 3 would leave `size() == cap` for the wrong reason — `add` deduplicating the
/// identical later board rather than evicting a distinct one — and this test's message says
/// *eviction*. At `cap = 2` the third distinct board must push one out, so the equality below is
/// the eviction it claims to be. The cap's *value* was never the subject: the doc above says so,
/// and `BoardHistory.java:53-76` does not branch on it.
#[test]
#[cfg_attr(debug_assertions, ignore)]
fn a_full_history_never_ranks_a_board_past_its_cap() {
    if !parity::require_java_dir() {
        return;
    }
    let mut board = load_board(J2);
    let settings = build_settings(&board, 1);
    let scoring = scoring_of(&settings);
    let cap = 2usize;
    let mut bh = BoardHistory::with_capacity(&scoring, cap);

    // Three distinct boards out of the first two passes — the unrouted board, then the board after
    // each pass; each pass changes the trace set, so each is a distinct `structural_hash`. The
    // third pass is run too and is *expected* to be a duplicate: `add` drops it, and the list is
    // held at the cap by the eviction that already happened.
    bh.add(&mut board);
    let stop = RouterStop::new();
    let mut sink = NoopProgressSink;
    let mut router = fr_router::pipeline::BatchAutorouter::for_routing_job(
        &board,
        &settings,
        RouterBudget::disabled(),
    );
    let mut log = fr_router::pipeline::RoutingFailureLog::new();
    for pass in 1..=3 {
        router
            .autoroute_pass(&mut board, &mut log, pass, &stop, &mut sink)
            .expect("the pass runner's boundary answers Ok");
        bh.add(&mut board);
    }

    assert_eq!(bh.size(), cap, "add's eviction holds the list at the cap");
    for entry in bh.entries() {
        let rank = bh
            .entries()
            .iter()
            .position(|e| e.hash == entry.hash)
            .expect("an entry finds itself")
            + 1;
        assert!(
            rank <= cap,
            "getRank is a 1-indexed position in a list capped at {cap}, got {rank}"
        );
    }
}

/// **Discharges Task 3's `obligation:` on `BoardHistory::rank`** (`board_history.rs`, the marker
/// on `fn rank`): the pass loop's `:307` -> `:315` pair is the *composition* that method's own
/// tests could not pin, because `p7t10` covers `getRank` in **insertion** order only — it takes
/// exactly `HISTORY_CAP` adds and never calls `restore_board`, deliberately, so that a `p7t10`
/// diff has exactly one possible cause.
///
/// `AutorouteBatchLoop.java:315` is `getRank`'s **only** caller, and it always runs one line after
/// `restoreBoard`, which sorts the list in place (`BoardHistory.java:143`) and bumps one entry's
/// `restoreCount` (`:147`) under a *read* lock — quirk #198. So the rank the loop tests against
/// [`BOARD_RANK_LIMIT`] is a position in the **score-sorted** list, never the insertion order.
///
/// The fixture makes the two orders disagree: three boards are inserted in *ascending* score, so
/// the descending sort reverses them and every rank moves.
///
/// PORT-REGRESSION PIN — the **board** was re-cut at the M1 accept wave (ruling BV), not a
/// number. The jar-parity fixture was `rpi_splitter`, whose unrouted / after-pass-1 /
/// after-pass-2 scores were strictly increasing. After R1 (#293) and R2 (#294) they are
/// `0 / 599.985 4 / 599.985 4` — the board is at its ceiling after one pass — so the fixture, not
/// the assertion, stopped working. [`J2`] still gives three strictly increasing scores
/// (`0 / 848.476 7 / 909.082 46`). Accepted at M1 (ruling BV).
#[test]
#[cfg_attr(debug_assertions, ignore)]
fn the_rank_the_loop_tests_is_read_after_restore_boards_reorder() {
    if !parity::require_java_dir() {
        return;
    }
    let mut board = load_board(J2);
    let settings = build_settings(&board, 1);
    let scoring = scoring_of(&settings);

    // Three boards in ascending score: unrouted, after pass 1, after pass 2.
    let unrouted = board.deep_copy();
    let stop = RouterStop::new();
    let mut sink = NoopProgressSink;
    let mut router = fr_router::pipeline::BatchAutorouter::for_routing_job(
        &board,
        &settings,
        RouterBudget::disabled(),
    );
    let mut log = fr_router::pipeline::RoutingFailureLog::new();
    router
        .autoroute_pass(&mut board, &mut log, 1, &stop, &mut sink)
        .expect("the pass runner's boundary answers Ok");
    let after_one = board.deep_copy();
    router
        .autoroute_pass(&mut board, &mut log, 2, &stop, &mut sink)
        .expect("the pass runner's boundary answers Ok");
    let after_two = board.deep_copy();

    let score = |b: &Board| BoardStatistics::new(&mut b.deep_copy()).normalized_score(&scoring);
    assert!(
        score(&unrouted) < score(&after_one) && score(&after_one) < score(&after_two),
        "the fixture needs three strictly increasing scores, got {} {} {}",
        score(&unrouted),
        score(&after_one),
        score(&after_two)
    );

    let mut bh = BoardHistory::new(&scoring);
    for b in [&unrouted, &after_one, &after_two] {
        bh.add(&mut b.deep_copy());
    }
    assert_eq!(bh.size(), 3);

    // Insertion order — what `getRank` answers *before* any restore.
    assert_eq!(bh.rank(&unrouted), 1, "inserted first");
    assert_eq!(bh.rank(&after_two), 3, "inserted last");

    // `:307` — `restoreBoard(MAXIMUM_TRIES_ON_THE_SAME_BOARD)`. It sorts descending by score and
    // hands back the head, i.e. the *best* board.
    let restored = bh
        .restore_board(fr_router::pipeline::batch_loop::MAXIMUM_TRIES_ON_THE_SAME_BOARD)
        .expect("three fresh entries are all inside the restore budget");
    assert_eq!(
        score(&restored),
        score(&after_two),
        "BoardHistory.java:143-149 hands back the head of the descending sort"
    );

    // `:315` — and now every rank has moved, which is the whole point of the obligation.
    assert_eq!(bh.rank(&after_two), 1, "best board, now first");
    assert_eq!(bh.rank(&unrouted), 3, "worst board, now last");
    assert_eq!(
        bh.rank(&restored),
        1,
        ":315 reads the rank of the board :307 just handed back"
    );

    // …and that is the number `:317` tests. It is inside the limit here, as quirk #217 says it
    // always is.
    assert!(!rank_limit_exceeded(bh.rank(&restored)));
}

// =================================================================================================
// :422 with :509-517 — quirk #215
// =================================================================================================

/// **Quirk #215, fixed in Plan 9 Task 9.** The fully-routed counter reset at `:509-517` was the
/// `else` of `:422`'s `currentPass >= STOP_AT_PASS_MINIMUM && continueAutorouting`, so it fired on
/// passes **1-7** and never on pass 8 or later — the opposite of the rule its own comment
/// (`:511-514`) describes. The consequence was that the pass-local counter was reset exactly while
/// it was still incapable of firing (it needs [`STAGNATION_PASS_LIMIT`] increments, and the
/// increments themselves only happen inside the `>= 8` arm), and was not reset once it could.
///
/// The fix moves it inside, as [`StagnationStep::BoardRouted`] — the third branch of `:425`'s
/// score test. This test asserts the new placement from both sides: the reset is unreachable
/// below pass 8 because the guard is, and from pass 8 on it is the branch a fully-routed board
/// above the score threshold takes.
#[test]
fn the_stagnation_counter_resets_only_from_pass_eight() {
    // Passes 1-7: the guard is false, so **no** arm of the stagnation block runs at all — neither
    // the increment nor the reset. That is the half Java got backwards.
    for pass in 1..STOP_AT_PASS_MINIMUM {
        assert!(
            !stagnation_guard(pass, true),
            "pass {pass} reaches no arm of the stagnation block, reset included"
        );
    }
    // Pass 8 and later: the guard is true, and `stagnation_step` is the three-way inside it.
    for pass in [STOP_AT_PASS_MINIMUM, 9, 12, 40] {
        assert!(
            stagnation_guard(pass, true),
            "pass {pass} takes the stagnation arm, and :509's reset is now inside it"
        );
    }
    // The second conjunct: once `autoroutePass` answers false the guard is false again at *any*
    // pass number — on a pass the loop is about to leave anyway.
    assert!(!stagnation_guard(40, false));

    // The three-way itself, in Java's own order.
    //
    // `:425` wins when the score rose past the threshold, whatever the incomplete count.
    assert_eq!(
        stagnation_step(10.0, 5.0, 3),
        StagnationStep::ScoreImproved,
        ":425 — boardScoreAfter > lastBestScore + 0.5"
    );
    assert_eq!(
        stagnation_step(10.0, 5.0, 0),
        StagnationStep::ScoreImproved,
        ":425 is tested first, so an improving *and* completed pass reports the improvement"
    );
    // `:510`, now reachable: a fully-routed board above the score threshold that did **not**
    // improve resets the counter instead of incrementing it.
    assert_eq!(
        stagnation_step(5.0, 5.0, 0),
        StagnationStep::BoardRouted,
        ":510 — incompleteCount == 0 && boardScoreAfter > 0.5"
    );
    // …and the case `:511-514`'s comment is explicitly about: a fully-routed board scoring **0**
    // must NOT reset. Before the fix this rule was stated in a comment attached to an arm that
    // could never run at a pass where the counter existed.
    assert_eq!(
        stagnation_step(0.0, 5.0, 0),
        StagnationStep::Accumulate,
        "a fully-routed board with score == 0 keeps accumulating until the global tracker fires"
    );
    // The score threshold is the same 0.5 the improvement test adds, and it is **strict**.
    assert_eq!(stagnation_step(0.5, 5.0, 0), StagnationStep::Accumulate);
    // An incomplete board that did not improve: the ordinary `:429` path.
    assert_eq!(stagnation_step(5.0, 5.0, 3), StagnationStep::Accumulate);

    // The window is ten passes wide (`:456`, `:486`) and `:429`'s increment is inside the `>= 8`
    // arm, so the counter first reaches 10 at **pass 17** and the global tracker at **pass 18** —
    // the band in which the reset used to be out of reach and now is not.
    assert_eq!(STAGNATION_PASS_LIMIT, 10, "BatchAutorouter.java:52");
}

// =================================================================================================
// :525-550 — the final best-board swap
// =================================================================================================

/// `:528-535`. The swap takes the history's best board when it is **strictly** better than the one
/// the loop ended on, and leaves the board alone otherwise.
///
/// This is the assignment that decides which board reaches SES, and it never fires on the corpus —
/// every `p7t9` run prints `swapped=false` — so it is pinned here directly. The fixture is built
/// the way the loop builds one: an unrouted board goes into the history, three real passes improve
/// the live board, and the swap is then asked about a history whose best entry is *worse*. Then
/// the two boards are exchanged and the same question is asked again.
#[test]
#[cfg_attr(debug_assertions, ignore)]
fn the_final_swap_takes_the_best_board_only_when_it_is_strictly_better() {
    if !parity::require_java_dir() {
        return;
    }
    let mut board = load_board(RPI);
    let settings = build_settings(&board, 1);
    let scoring = scoring_of(&settings);

    // A routed board — three real passes of the real pass runner.
    let stop = RouterStop::new();
    let mut sink = NoopProgressSink;
    let mut router = fr_router::pipeline::BatchAutorouter::for_routing_job(
        &board,
        &settings,
        RouterBudget::disabled(),
    );
    let mut log = fr_router::pipeline::RoutingFailureLog::new();
    for pass in 1..=3 {
        router
            .autoroute_pass(&mut board, &mut log, pass, &stop, &mut sink)
            .expect("the pass runner's boundary answers Ok");
    }
    let routed = board.deep_copy();
    let routed_score = BoardStatistics::new(&mut board).normalized_score(&scoring);
    let mut unrouted = load_board(RPI);
    let unrouted_score = BoardStatistics::new(&mut unrouted).normalized_score(&scoring);
    assert!(
        routed_score > unrouted_score,
        "the fixture only means anything if routing improved the score: \
         {routed_score} vs {unrouted_score}"
    );

    // (a) The history holds only the *worse* board: `:531`'s `>` is false and nothing moves.
    {
        let mut bh = BoardHistory::new(&scoring);
        bh.add(&mut unrouted);
        let mut live = routed.deep_copy();
        let before = live.structural_hash();
        assert!(
            !final_best_board_swap(&mut live, &mut bh, &scoring),
            ":531 is `bestHistoryScore > currentFinalScore`, and the history is worse"
        );
        assert_eq!(live.structural_hash(), before, "the board must not move");
    }

    // (b) The history holds the *better* board: the swap fires and the live board is replaced.
    {
        let mut bh = BoardHistory::new(&scoring);
        let mut better = routed.deep_copy();
        bh.add(&mut better);
        let mut live = load_board(RPI);
        assert!(
            final_best_board_swap(&mut live, &mut bh, &scoring),
            "the history's best board scores {routed_score} against the live {unrouted_score}"
        );
        assert_eq!(
            BoardStatistics::new(&mut live).normalized_score(&scoring),
            routed_score,
            ":535 replaces the board with the history's best"
        );
    }

    // (c) The **strictness** of `:531`: the same board in the history and on the table ties, and a
    // tie does not swap. Written as its own case because `>=` would pass (a) and (b) unchanged.
    {
        let mut bh = BoardHistory::new(&scoring);
        let mut same = routed.deep_copy();
        bh.add(&mut same);
        let mut live = routed.deep_copy();
        // `BoardHistory::add` measures the board it is handed, so both sides carry the same score.
        assert!(
            !final_best_board_swap(&mut live, &mut bh, &scoring),
            ":531's `>` is strict — an equally good history entry does not displace the board"
        );
    }
}

/// `getMaxScore()` seeds `0`, not `-inf` (quirk #197), so an **empty** history cannot swap — which
/// is what makes `:298`'s `bh.size() >= 8` gate belt-and-braces rather than load-bearing.
#[test]
#[cfg_attr(debug_assertions, ignore)]
fn an_empty_history_never_swaps() {
    if !parity::require_java_dir() {
        return;
    }
    let mut board = load_board(RPI);
    let settings = build_settings(&board, 1);
    let scoring = scoring_of(&settings);
    let mut bh = BoardHistory::new(&scoring);
    let before = board.structural_hash();
    assert_eq!(bh.max_score(), 0.0, "BoardHistory.java:118 seeds 0");
    assert!(!final_best_board_swap(&mut board, &mut bh, &scoring));
    assert_eq!(board.structural_hash(), before);
}

// =================================================================================================
// :249 — quirk #216's roster assertion
// =================================================================================================

/// **A roster assertion, and that is its whole job.**
///
/// `:249` allocates `Set<String> alreadyRoutedBoardHashes = new HashSet<>()`. Both of its readers
/// are commented out — the `:257-266` block Java disabled — and what survives is the allocation
/// plus two `.clear()` calls (`:334`, `:446`). There is **nothing behavioural to assert**: the set
/// has no live reader in Java either, which is exactly why the quirk row exists and why this test
/// checks the marker rather than a behaviour.
///
/// It exists so the `// not ported:` line cannot be deleted as noise by someone who greps for
/// unused markers, and so the audit's map row keeps a home for the member.
#[test]
fn the_dead_hash_set_is_javas_only_allocation() {
    let source = include_str!("../src/pipeline/batch_loop.rs");
    assert!(
        source.contains("not ported: `alreadyRoutedBoardHashes` (`:249`)"),
        "batch_loop.rs must keep the `not ported:` line for AutorouteBatchLoop.java:249 — \
         quirk #216 is a row about a dead allocation, and the marker is the only thing that \
         records it in the port"
    );
    assert!(
        source.contains("`:259` and `:266`"),
        "the marker must name both commented-out readers, which is what makes the set dead"
    );
}

// =================================================================================================
// The two stubs, and the obligations they carry
// =================================================================================================

/// Ruling B1's tripwire, discharged. Until Plan 7 Task 12 the fanout pre-pass (`:89-173`) was a
/// **loud** stub — a `run` with `fanout.enabled = true` panicked, because silently skipping the
/// stage would have answered a different board — and this test asserted the panic. Task 12 landed
/// [`BatchFanout::fanout_board`], so the same call now routes and reports a summary, and the test
/// asserts *that* instead: the stub is gone and the arm it stood in for is live.
///
/// The whole-board version is `scripts/differential/run.sh p7t9 <dsn> 1 router+fanout`;
/// `crates/fr-router/tests/fanout.rs` owns the stage's own unit tests.
#[test]
#[cfg_attr(debug_assertions, ignore)]
fn routing_with_fanout_enabled_runs_the_pre_pass() {
    if !parity::require_java_dir() {
        return;
    }
    let mut board = load_board(RPI);
    let mut settings = build_settings(&board, 1);
    settings.fanout.get_or_insert_with(Default::default).enabled = Some(true);
    // Ruling AI: `fanoutPass:231-232`'s per-pin clock, off on this side as the drivers set it.
    settings
        .fanout
        .get_or_insert_with(Default::default)
        .max_milliseconds_per_pin = Some(i64::from(i32::MAX));
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
    let summary = result
        .fanout
        .expect("`:123-172` ran, so `:173` had a summary to read");
    assert!(!summary.is_timed_out);
    assert!(
        summary.completed_pass_count > 0,
        "the stage ran at least one pass"
    );
    assert!(
        board.get_vias().len() >= 9,
        "the escape vias `p7t5 board` counts on this stem are on the board the loop answered"
    );
}

/// The stagnation report (`:456-476`, `:486-507`) is Task 15's, and its stub is the *other* shape
/// of ruling B1: **inert**, because the report is a log payload and both arms are live on a long
/// run — being loud there would break a legitimate corpus run over a string nobody reads.
///
/// This test pins the distinction rather than the string: it asserts the `obligation:` marker
/// names Task 15, and that the stub sits beside the two `break`s it must not disturb.
#[test]
fn the_stagnation_report_is_discharged_and_names_task_15() {
    let source = include_str!("../src/pipeline/batch_loop.rs");
    assert!(
        source.contains(
            "discharged: `AutorouteBatchLoop`'s stagnation report (`:456-476`, `:486-507`) called"
        ),
        "the stagnation report site must carry its `discharged:` marker naming Task 15's landing"
    );
    // The call is the *report*, never the break. Java's loop leaves through five
    // `requestStopAutoRouter(); break;` pairs — `:271-272` (maxPasses), `:311-312` ("not able to
    // improve"), `:318-319` (the rank limit), `:474-475` (the pass-local stagnation window) and
    // `:505-506` (the global one). Since Plan 9 Task 9 each of them also **names its door**
    // (`exit = Some(BatchLoopExit::…)`, quirk #214) between the request and the `break`, and the
    // rank-limit arm is **gone** with quirk #217's decision (delete the branch; re-enabling it is
    // a product decision with its own A/B) — so what is left is **four** requests each followed
    // by a `break`, plus `:252`'s dead arm, and the two the stub sits between are among them.
    // Counted on a whitespace-normalised copy so that rustfmt's indentation cannot make the
    // assertion pass or fail for the wrong reason.
    let flat = source.split_whitespace().collect::<Vec<_>>().join(" ");
    assert_eq!(
        flat.matches("stop.request_stop_auto_router();").count(),
        // `:252`'s dead arm (quirk #203) is the fifth call and has no `break`.
        5,
        "the four `requestStopAutoRouter()` breaks (:271-272, :311-312, :474-475, :505-506) plus \
         `:252`'s dead arm must all be present"
    );
    assert_eq!(
        flat.matches("exit = Some(BatchLoopExit::").count(),
        4,
        "every one of the four breaks names the door it left by — quirk #214's fix"
    );
    // …and the report really is called at both stagnation arms, so Task 15 has two sites to
    // discharge rather than one.
    assert_eq!(
        flat.matches("let _report = build_unrouted_report(board);")
            .count(),
        2,
        ":457 and :487 both call buildUnroutedConnectionsReport"
    );
}

// =================================================================================================
// The per-pass record — ruling 1(a)
// =================================================================================================

/// `PassRecord` is filled once per **completed** pass, in pass order, from the statistics Java
/// reports at `:353-363`. The acceptance ladder compares this tuple pass by pass, so its
/// *cardinality* matters as much as its contents: a pass that broke out before completing must not
/// leave a record.
#[test]
#[cfg_attr(debug_assertions, ignore)]
fn one_pass_record_per_completed_pass_in_order() {
    if !parity::require_java_dir() {
        return;
    }
    let mut board = load_board(RPI);
    let settings = build_settings(&board, 0);
    let stop = RouterStop::new();
    let mut sink = NoopProgressSink;
    let result = AutorouteBatchLoop::run(
        &mut board,
        &settings,
        &stop,
        RouterBudget::disabled(),
        &mut sink,
    )
    .expect("rpi_splitter has two signal layers");

    assert!(!result.per_pass.is_empty());
    for (i, record) in result.per_pass.iter().enumerate() {
        assert_eq!(
            record.pass,
            i as i32 + 1,
            "PassRecord::pass is `currentPass`, 1-based and contiguous"
        );
    }
    // `:520-522` increments only when the loop is going round again, so the last record's pass is
    // one behind `passes_run` on a run that ended by exhausting its work.
    let last = result.per_pass.last().expect("at least one pass");
    assert_eq!(
        result.passes_run, last.pass,
        "the loop left `currentPass` where the last completed pass put it"
    );
    // And the final board really is the one the last record describes.
    let stats = BoardStatistics::new(&mut board);
    assert_eq!(
        stats.connections.incomplete_count.expect("filled"),
        last.incomplete_count as i32
    );
}
