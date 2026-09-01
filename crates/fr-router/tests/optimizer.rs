//! `BatchOptimizer`'s **stage** half — Plan 7 Task 14.
//!
//! `crates/fr-router/tests/optimizer_items.rs` (Task 13) pins the item half: the reader's
//! ordering, `optRouteItem` and the clone-based snapshot. This file pins what sits above it —
//! `runBatchLoop` (`BatchOptimizer.java:125-272`), `optRoutePass` (`:279-385`), the four arms
//! that decide whether the optimizer goes round again, and the roster of the two multithreaded
//! classes.
//!
//! # Why so many of these are synthetic
//!
//! Every termination arm is a decision over two `float`s and two `Integer`s, and a real board
//! reaches at most one of them per run. The three lifted decisions — [`optimizer_near_perfect_exit`],
//! [`optimizer_route_improved`] and `BatchOptimizer::apply_pass_improvement` — are therefore
//! tested as functions, on the values Java's arithmetic makes interesting rather than on the
//! values a corpus board happens to produce, and the loop itself is driven over the same
//! synthetic two-layer board `optimizer_items.rs` builds.
//!
//! The four tests that need a *routed* board carry `#[cfg_attr(debug_assertions, ignore)]` and
//! run in release, the convention `optimizer_items.rs` established: they route
//! `Issue143-rpi_splitter.dsn` with the real [`AutorouteBatchLoop`] first, which is the board
//! `p7t9` and `p7t8` both pin.

use fr_board::prelude::*;
use fr_geometry::{IntBox, IntOctagon, IntPoint, Shape, TileShape};
use fr_router::pipeline::{
    AutorouteBatchLoop, BatchOptimizer, ItemRouteResult, NamedAlgorithmType, NoopProgressSink,
    ProgressSink, RouterBudget, RouterStop, RoutingEvent, TaskState, optimizer_near_perfect_exit,
    optimizer_route_improved,
};
use fr_settings::sources::DefaultSettings;
use fr_settings::{HostEnvironment, RouterSettings, SettingsSource};

// =================================================================================================
// Fixtures — the `optimizer_items.rs` canvas
// =================================================================================================

const BOUNDING_BOX: IntBox = IntBox {
    ll: IntPoint {
        x: -10_000,
        y: -10_000,
    },
    ur: IntPoint {
        x: 10_000,
        y: 10_000,
    },
};

/// `p7t8`'s and `p7t9`'s default stem.
const RPI: &str = "fixtures/Issue143-rpi_splitter.dsn";

fn layers() -> LayerStructure {
    LayerStructure::new(vec![Layer::new("front", true), Layer::new("back", true)])
}

/// The two-layer, four-net, item-less board `optimizer_items.rs` paints on. A pass over it visits
/// **no** items — `ReadSortedRouteItems::next` answers `None` at once — which is exactly what the
/// loop-shape tests want: the pass runs, costs nothing, and improves nothing.
fn empty_board() -> Board {
    let mut padstacks = Padstacks::new(layers());
    let through_shape = Shape::Tile(TileShape::Octagon(IntOctagon::new(
        -70, -70, 70, 70, -140, 140, -140, 140,
    )));
    padstacks.add(
        "thru",
        vec![Some(through_shape.clone()), Some(through_shape)],
        true,
        false,
    );
    let clearance_matrix = ClearanceMatrix::get_default_instance(&layers(), 200);
    let rules = BoardRules::new(layers(), clearance_matrix);
    let mut board = Board::new(
        Vec::new(),
        0,
        BOUNDING_BOX,
        rules,
        BoardLibrary::new(padstacks, Packages::new()),
        Components::new(),
        Communication::default(),
    );
    let default_class = board.rules.get_default_net_class();
    for name in ["N1", "N2", "N3", "N4"] {
        board.rules.nets.add(name, 1, false, default_class);
    }
    board
}

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

/// `P7T9.buildSettings` in its `router-only` mode, which is what the routed-board tests want.
fn build_settings(board: &Board) -> RouterSettings {
    let mut settings = DefaultSettings::new(&HostEnvironment::detect())
        .get_settings()
        .expect("DefaultSettings always answers a table")
        .clone();
    settings.set_layer_count(board.get_layer_count());
    settings.apply_board_specific_optimizations(board);
    let fanout = settings.fanout.get_or_insert_with(Default::default);
    fanout.enabled = Some(false);
    fanout.max_milliseconds_per_pin = Some(i64::from(i32::MAX));
    settings.set_run_router(true);
    settings.set_run_optimizer(false);
    settings.save_intermediate_stages = Some(false);
    settings
}

/// `P7T8`'s routing prologue: the real `BatchAutorouter.runBatchLoop()` at `maxPasses = 1`, i.e.
/// the call `p7t9 router-only` pins byte for byte on both sides.
fn routed_rpi() -> (Board, RouterSettings) {
    let mut board = load_board(RPI);
    let mut settings = build_settings(&board);
    settings.max_passes = Some(1);
    let stop = RouterStop::new();
    let mut sink = NoopProgressSink;
    AutorouteBatchLoop::run(
        &mut board,
        &settings,
        &stop,
        RouterBudget::disabled(),
        &mut sink,
    )
    .expect("rpi_splitter has a routable signal layer");
    (board, settings)
}

fn optimizer_settings(settings: &mut RouterSettings) -> &mut fr_settings::OptimizerSettings {
    settings
        .optimizer
        .as_mut()
        .expect("DefaultSettings always fills the optimizer block")
}

struct RecordingSink {
    events: Vec<RoutingEvent>,
}

impl ProgressSink for RecordingSink {
    fn on_event(&mut self, event: &RoutingEvent) {
        self.events.push(event.clone());
    }
}

// =================================================================================================
// `:182-193` — the "already near-perfect" exit
// =================================================================================================

/// `:182-183` is `scoreBeforePass * (1 + optimizationImprovementThreshold) >= 1000.0f`, and
/// **every step of it is a `float`**: the threshold is a `Float` (`OptimizerSettings.java:37-38`),
/// so `1 + threshold` is a `float` add and the product a `float` multiply.
///
/// This test is the discriminator. `999.99994f` is the largest `float` below the score ceiling and
/// `6.0e-8` sits between half an `f32` ulp of 1 (`5.96e-8`) and a whole one (`1.19e-7`) — so
/// `1 + threshold` **rounds up** to `1.00000011920929` in `f32` and the product clears 1000, while
/// the same arithmetic in `f64` adds only `6e-5` to `999.99993896` and does not. The port must
/// answer `true`; an `f64` transcription answers `false`.
#[test]
fn the_near_perfect_exit_is_computed_in_f32() {
    let score = 999.999_94_f32;
    let threshold = 6.0e-8_f32;

    // What the port does — `:182-183`, in `float`.
    assert!(
        optimizer_near_perfect_exit(score, threshold),
        "the f32 product clears 1000 because `1 + 6e-8` rounds up to the next f32"
    );

    // What an `f64` transcription would have done, computed here so the disagreement is measured
    // rather than asserted.
    let in_f64 = f64::from(score) * (1.0 + f64::from(threshold));
    assert!(
        in_f64 < 1000.0,
        "the f64 product is {in_f64}, which does not clear 1000 — the two really do disagree"
    );
}

/// The arm's two ordinary answers, on the numbers a corpus board produces: `p7t9 router-only`
/// leaves `Issue143-rpi_splitter` at score `799.98267` after one pass, and `DefaultSettings`'
/// threshold is `0.01` (`DefaultSettings.java:135`).
#[test]
fn the_near_perfect_exit_is_false_at_the_default_threshold() {
    // 799.98267 * 1.01 = 807.98 — nowhere near the ceiling, so the optimizer runs.
    assert!(!optimizer_near_perfect_exit(799.982_67, 0.01));
    // A threshold of 0.26 puts the same score over 1000, which is the arm firing.
    assert!(optimizer_near_perfect_exit(799.982_67, 0.26));
    // A zero score can never fire it, whatever the threshold — which is why an *unrouted* board
    // (score 0, because `calculateScore` charges every incomplete connection) always takes a pass.
    assert!(!optimizer_near_perfect_exit(0.0, 1.0e6));
}

// =================================================================================================
// `:340-348` — the improvement recomputation, and the bug it is the correct twin of
// =================================================================================================

/// **Quirk #212's twin.** `ItemRouteResult`'s constructor (`ItemRouteResult.java:59-65`) computes
/// this formula with `viaCountAfter / viaCountBefore` as an **`int`** division, so the via term
/// truncates; `optRoutePass:345` writes `(float) result.viaCount() / …`, where the cast binds to
/// the numerator and the division is real. Both are ported, and on the same item they disagree.
///
/// 3 vias out of 10 and 900 units of trace out of 1000:
///
/// * the scorecard field: `1 - ((3/10 == 0) + 0.9) / 2` = **0.55**;
/// * the pass's own number: `1 - (0.3 + 0.9) / 2` = **0.4**.
#[test]
fn the_improvement_recomputation_disagrees_with_the_scorecard_field() {
    let result = ItemRouteResult::new(ItemId(1), 10, 3, 1000.0, 900.0, 0, 0);

    // `ItemRouteResult.java:59-65` — the integer-truncating one (quirk #212).
    assert!(
        (result.improvement_percentage() - 0.55).abs() < 1e-6,
        "the scorecard field truncates the via term to 0, giving 0.55, not {}",
        result.improvement_percentage()
    );

    // `BatchOptimizer.java:340-348` — the `(float)`-cast twin, on the same numbers.
    let route_improved = optimizer_route_improved(&result, 10, 1000.0);
    assert!(
        (route_improved - 0.4).abs() < 1e-6,
        "the pass's recomputation divides for real, giving 0.4, not {route_improved}"
    );

    assert!(
        (result.improvement_percentage() - route_improved).abs() > 0.1,
        "the two numbers are not each other, and the port must not quietly reconcile them"
    );
}

/// `:342-343`'s guard is on the **pass**'s statistics, and it answers a flat `0` — which is also
/// the value `:365`'s `routeImproved == 0` test reads as "this pass improved nothing". So a pass
/// on a board with no vias drops the increased ripup costs however well its items did.
#[test]
fn the_recomputation_guard_answers_zero_and_so_reads_as_no_improvement() {
    let result = ItemRouteResult::new(ItemId(1), 10, 3, 1000.0, 900.0, 0, 0);
    assert_eq!(optimizer_route_improved(&result, 0, 1000.0), 0.0);
    assert_eq!(optimizer_route_improved(&result, 10, 0.0), 0.0);
}

// =================================================================================================
// `:209-218` — the pass improvement and the increased ripup costs
// =================================================================================================

/// `:212-215`: the **first** pass that fails to raise the score spends the increased ripup costs
/// instead of the optimizer's budget, and answers the `-1` sentinel so that `:220`'s threshold
/// test is skipped and the loop goes round again. It can fire only once, because `:212`'s first
/// conjunct is false forever afterwards.
#[test]
fn the_increased_ripup_costs_are_dropped_after_one_non_improving_pass() {
    let settings = DefaultSettings::new(&HostEnvironment::detect())
        .get_settings()
        .expect("DefaultSettings always answers a table")
        .clone();
    let mut optimizer = BatchOptimizer::new(&settings);

    // `runBatchLoop:132`.
    optimizer.use_increased_ripup_costs = true;

    // A pass that did not raise the score: `scoreAfter <= scoreBefore`.
    let (pass_improvement, score_improvement) = optimizer.apply_pass_improvement(800.0, 800.0);
    assert_eq!(pass_improvement, 0.0, "(800 - 800) / 800 is 0");
    assert_eq!(score_improvement, -1.0, ":215's sentinel");
    assert!(!optimizer.use_increased_ripup_costs, ":213 clears the flag");

    // The very same pass again: the flag is down, so `:217` answers the real number and `:220`
    // now has something to compare against the threshold.
    let (pass_improvement, score_improvement) = optimizer.apply_pass_improvement(800.0, 800.0);
    assert_eq!(pass_improvement, 0.0);
    assert_eq!(
        score_improvement, 0.0,
        ":217 — the arm cannot fire twice, so the loop's threshold exit is now reachable"
    );

    // And a pass that *did* improve never reaches the arm at all.
    let mut optimizer = BatchOptimizer::new(&settings);
    optimizer.use_increased_ripup_costs = true;
    let (pass_improvement, score_improvement) = optimizer.apply_pass_improvement(800.0, 808.0);
    assert!((pass_improvement - 0.01).abs() < 1e-6, "8 / 800 is 1 %");
    assert_eq!(score_improvement, pass_improvement);
    assert!(optimizer.use_increased_ripup_costs, "still up");
}

/// `:209-210`'s ternary: a non-positive `scoreBeforePass` answers `0` rather than dividing. That
/// is the arm every *unrouted* board takes, because `getNormalizedScore` floors at zero.
#[test]
fn the_pass_improvement_is_zero_when_the_score_before_is_not_positive() {
    let settings = DefaultSettings::new(&HostEnvironment::detect())
        .get_settings()
        .expect("DefaultSettings always answers a table")
        .clone();
    let mut optimizer = BatchOptimizer::new(&settings);
    let (pass_improvement, _) = optimizer.apply_pass_improvement(0.0, 500.0);
    assert_eq!(
        pass_improvement, 0.0,
        "a division by zero would have been +inf; Java's ternary answers 0"
    );
}

// =================================================================================================
// `:167-171`, `:200` — the loop head and the alternation
// =================================================================================================

/// `:200` — `withPreferredDirections = currentPass % 2 != 0`, "to create more variations". Odd
/// passes route with the preferred directions and even passes without.
///
/// Three passes are forced by a **negative** improvement threshold: `:220`'s exit needs
/// `scoreImprovement < threshold`, and a pass over an item-less board improves by exactly `0`.
#[test]
fn passes_alternate_preferred_directions() {
    let mut board = empty_board();
    let mut settings = build_settings(&board);
    {
        let optimizer = optimizer_settings(&mut settings);
        optimizer.max_passes = Some(3);
        optimizer.optimization_improvement_threshold = Some(-1.0);
    }
    let mut optimizer = BatchOptimizer::new(&settings);
    let stop = RouterStop::new();
    let mut sink = NoopProgressSink;
    let result = optimizer
        .run_batch_loop(&mut board, &stop, RouterBudget::disabled(), &mut sink)
        .expect("an item-less pass cannot fail");

    assert_eq!(result.passes_run, 3, ":167-168 — `currentPass < maxPasses`");
    let flags: Vec<bool> = result
        .per_pass
        .iter()
        .map(|record| record.with_preferred_directions)
        .collect();
    assert_eq!(
        flags,
        vec![true, false, true],
        "pass 1 and pass 3 are odd, pass 2 is not"
    );
    assert_eq!(result.state, TaskState::Finished);
    assert_eq!(result.items_optimized, 0, "the board has no route items");
}

/// `:365-368` is the **other** writer of `useIncreasedRipupCosts`, on a different condition from
/// `:212-215`: "no item improved in this pass". It fires first, which is why a pass that improves
/// nothing ends the whole stage one pass earlier than `:214`'s comment suggests — by the time
/// `:212` looks, its first conjunct is already false, so `:217` answers `0` and `:220` breaks.
#[test]
fn a_pass_that_improves_nothing_clears_the_increased_ripup_costs_and_ends_the_stage() {
    let mut board = empty_board();
    let settings = build_settings(&board);
    let mut optimizer = BatchOptimizer::new(&settings);
    let stop = RouterStop::new();
    let mut sink = NoopProgressSink;
    let result = optimizer
        .run_batch_loop(&mut board, &stop, RouterBudget::disabled(), &mut sink)
        .expect("an item-less pass cannot fail");

    assert_eq!(
        result.passes_run, 1,
        "one pass, then `:220`'s threshold exit — `maxPasses` is 100"
    );
    let pass = result.per_pass[0];
    assert_eq!(
        pass.route_improved, -1.0,
        ":367's sentinel, which `:201` discards"
    );
    assert!(
        !pass.use_increased_ripup_costs,
        ":366 cleared the flag inside the pass"
    );
    assert_eq!(
        pass.score_improvement, 0.0,
        ":217, not `:215` — `:212`'s first conjunct was already false"
    );
    assert!(!optimizer.use_increased_ripup_costs);
}

/// Quirk #202's second half, end to end at this stage: `AutoroutePassRunner:219` answers a
/// `--max-items` limit with `requestStop()`, i.e. **`ALL`**, and `:171` is `isStopRequested()`.
/// So a `maxItems` router stop disables the optimizer stage outright — zero passes, zero items.
#[test]
fn a_max_items_router_stop_disables_this_stage() {
    let mut board = empty_board();
    let settings = build_settings(&board);
    let mut optimizer = BatchOptimizer::new(&settings);
    let stop = RouterStop::new();
    // `AutoroutePassRunner.java:219`.
    stop.request_stop();
    let mut sink = RecordingSink { events: Vec::new() };
    let result = optimizer
        .run_batch_loop(&mut board, &stop, RouterBudget::disabled(), &mut sink)
        .expect("the loop is never entered");

    assert_eq!(result.passes_run, 0);
    assert!(result.per_pass.is_empty());
    assert_eq!(result.items_optimized, 0);
    assert_eq!(result.state, TaskState::Cancelled, ":253-255's ternary");

    // `:233-234` fires `FINISHED` **anyway**, which is the asymmetry this test also pins: the
    // event and the reported state disagree, and only the state is honest.
    assert_eq!(
        sink.events,
        vec![
            RoutingEvent::TaskStateChanged {
                algorithm: NamedAlgorithmType::Optimizer,
                state: TaskState::Started,
            },
            RoutingEvent::TaskStateChanged {
                algorithm: NamedAlgorithmType::Optimizer,
                state: TaskState::Finished,
            },
        ],
        "`:162-163` then `:233-234`, with nothing between them"
    );
}

/// **Quirk #227, half one.** `AutorouteBatchLoop:271` (and four sibling arms) answer the ordinary
/// end of routing with `requestStopAutoRouter()`, i.e. `AUTO_ROUTER_ONLY` — and `:171` reads
/// `isStopRequested()`, which is `ALL`. So the optimizer stage is **not** disabled by it and the
/// loop runs normally. Half two, on a routed board, is
/// `an_auto_router_only_stop_leaves_every_item_rejected`.
#[test]
fn an_auto_router_only_stop_does_not_disable_this_stage() {
    let mut board = empty_board();
    let settings = build_settings(&board);
    let mut optimizer = BatchOptimizer::new(&settings);
    let stop = RouterStop::new();
    // `AutorouteBatchLoop.java:271`.
    stop.request_stop_auto_router();
    let mut sink = NoopProgressSink;
    let result = optimizer
        .run_batch_loop(&mut board, &stop, RouterBudget::disabled(), &mut sink)
        .expect("an item-less pass cannot fail");

    assert_eq!(result.passes_run, 1, "the loop head let it through");
    assert_eq!(
        result.state,
        TaskState::Finished,
        "not `Cancelled`: `:254` reads `isStopRequested()` too"
    );
}

// =================================================================================================
// `:153-176`, `:308-313` — the per-stage deadline
// =================================================================================================

/// Ruling AI, and the two read sites `pipeline::stop`'s `obligation:` named: the deadline is
/// derived from `settings.optimizer.timeoutString` (`:153-160`), it raises
/// `BatchOptimizer::is_timed_out` (`:173`), and it **never touches the stop flag** — because
/// `RoutingPipeline.java:117` gates this very stage on that flag, so requesting `ALL` here would
/// suppress a stage Java leaves running.
///
/// A `"0"` timeout is `PT0S`, so the deadline is the session start and `:172`'s non-strict
/// `>=` fires on the first look — no sleep, no flake.
#[test]
fn the_stage_deadline_times_out_without_touching_the_stop_flag() {
    let mut board = empty_board();
    let mut settings = build_settings(&board);
    optimizer_settings(&mut settings).timeout_string = Some("0".to_string());
    let mut optimizer = BatchOptimizer::new(&settings);
    let stop = RouterStop::new();
    let mut sink = NoopProgressSink;
    let result = optimizer
        .run_batch_loop(&mut board, &stop, RouterBudget::disabled(), &mut sink)
        .expect("the loop breaks at the deadline");

    assert!(result.timed_out, ":173");
    assert!(optimizer.is_timed_out(), ":81-83 reads the same field");
    assert_eq!(
        result.passes_run, 0,
        ":172-176 breaks before `++currentPass`"
    );
    assert_eq!(result.state, TaskState::TimedOut, ":252-255");

    // The whole point of the obligation.
    assert!(
        !stop.is_stop_requested(),
        "the stage deadline is not a job stop"
    );
    assert!(!stop.is_stop_auto_router_requested());
    assert!(
        !stop.is_timed_out(),
        "`RouterStop::poll_deadline` was never called"
    );
}

/// The same obligation from the other side, and the mutation that proves this file can see it:
/// the **job**'s deadline is expired here, and `run_batch_loop` must still not look at it.
///
/// `RouterStop::poll_deadline` is what reproduces Java's monitor thread — it requests `ALL` and
/// raises `RouterStop::is_timed_out` — and Java's optimizer never calls anything of the kind:
/// `grep -n requestStop BatchOptimizer.java` is empty. Routing `:172`'s read through it would
/// therefore end not just this stage but, through `RoutingPipeline.java:117`, any stage after it.
/// A negative limit is exceeded on the first look (`TimeLimit.java:18-21` compares strictly
/// greater), so the assertion needs no sleep.
#[test]
fn the_stage_deadline_never_polls_the_jobs_own_deadline() {
    let mut board = empty_board();
    let mut settings = build_settings(&board);
    optimizer_settings(&mut settings).timeout_string = Some("0".to_string());
    let mut optimizer = BatchOptimizer::new(&settings);
    // Ruling AI's job-level deadline, already expired.
    let stop = RouterStop::with_deadline(-1);
    let mut sink = NoopProgressSink;
    let result = optimizer
        .run_batch_loop(&mut board, &stop, RouterBudget::disabled(), &mut sink)
        .expect("the loop breaks at the stage deadline");

    assert!(result.timed_out, "the **stage**'s clock fired");
    assert_eq!(result.state, TaskState::TimedOut);
    assert!(
        !stop.is_stop_requested(),
        "nothing in the optimizer polled the job's deadline, so the flag is still NONE"
    );
    assert!(
        !stop.is_timed_out(),
        "…and `job.state` was never written to TIMED_OUT either"
    );
}

/// A board with no `timeoutString` has no deadline at all (`:153`'s guard), which is every corpus
/// run: `DefaultSettings` never sets the field.
#[test]
fn no_timeout_string_means_no_deadline() {
    let settings = DefaultSettings::new(&HostEnvironment::detect())
        .get_settings()
        .expect("DefaultSettings always answers a table")
        .clone();
    assert_eq!(
        settings
            .optimizer
            .as_ref()
            .expect("the optimizer block")
            .timeout_string,
        None,
        "DefaultSettings.java:130-142 sets no timeout, so no `p7t*` run has a deadline"
    );

    let mut board = empty_board();
    let build = build_settings(&board);
    let mut optimizer = BatchOptimizer::new(&build);
    assert!(
        !optimizer.is_deadline_reached(),
        "no deadline before the run"
    );
    let stop = RouterStop::new();
    let mut sink = NoopProgressSink;
    optimizer
        .run_batch_loop(&mut board, &stop, RouterBudget::disabled(), &mut sink)
        .expect("an item-less pass cannot fail");
    assert!(!optimizer.is_deadline_reached());
    assert!(!optimizer.is_timed_out());
}

// =================================================================================================
// `:68-78`, `:527-550` — `normalizeAlgorithm` and the five identity members
// =================================================================================================

/// `:69` compares `getId()` against the configured name and `:76` overwrites it, so the answer is
/// always the id — a mismatched name is replaced, and a matching one already is it.
#[test]
fn normalize_algorithm_always_answers_the_optimizers_id() {
    assert_eq!(
        BatchOptimizer::normalize_algorithm("freerouting-optimizer"),
        "freerouting-optimizer",
        ":69's guard is false, so `:76` never runs"
    );
    assert_eq!(
        BatchOptimizer::normalize_algorithm("something-else"),
        "freerouting-optimizer",
        ":70-76 — the warning, then the overwrite"
    );
    assert_eq!(
        BatchOptimizer::normalize_algorithm(""),
        "freerouting-optimizer",
        "Java's `equals` against a null or empty name is false, so this is the same arm"
    );
    // `DefaultSettings.java:131` sets exactly this, which is why no headless run ever warns.
    let settings = DefaultSettings::new(&HostEnvironment::detect())
        .get_settings()
        .expect("DefaultSettings always answers a table")
        .clone();
    assert_eq!(
        settings
            .optimizer
            .as_ref()
            .expect("the optimizer block")
            .algorithm
            .as_deref(),
        Some(BatchOptimizer::ID)
    );
}

/// `:527-550` — five one-line `return "literal";` overrides, as five associated consts.
#[test]
fn the_five_named_algorithm_members_are_javas_literals() {
    assert_eq!(BatchOptimizer::ID, "freerouting-optimizer");
    assert_eq!(BatchOptimizer::NAME, "Freerouting Optimizer");
    assert_eq!(BatchOptimizer::VERSION, "1.0");
    assert_eq!(BatchOptimizer::DESCRIPTION, "Freerouting Optimizer v1.0");
    assert_eq!(BatchOptimizer::TYPE, NamedAlgorithmType::Optimizer);
}

// =================================================================================================
// The roster — `BatchOptimizerMultiThreaded`, `OptimizeRouteTask` and `createForGui`
// =================================================================================================

/// **A roster assertion, and that is its whole job** (the `batch_loop.rs:798` precedent).
///
/// Controller ruling AM/General rosters all five multithread classes `// not ported:` with caller
/// evidence. Two of them are the optimizer's, and they are reachable through exactly one door —
/// `BatchOptimizer.createForGui:59` — which is itself GUI-only. There is nothing behavioural to
/// assert: the port has no multithreaded optimizer to compare against. What can rot is the
/// evidence, so the test pins the markers and the two line numbers that make the argument.
#[test]
fn the_multithreaded_optimizer_is_rostered_not_ported() {
    let roster = include_str!("../src/lib.rs");
    assert!(
        roster.contains("not ported: `BatchOptimizerMultiThreaded.getNumTasks`"),
        "lib.rs must roster `BatchOptimizerMultiThreaded` — the map row points the class here"
    );
    assert!(
        roster.contains("not ported: `OptimizeRouteTask.run`"),
        "…and `OptimizeRouteTask`, which only it constructs"
    );
    assert!(
        roster.contains("`BatchOptimizer.java:59`"),
        "the single construction site is the whole of the evidence for the first class"
    );
    assert!(
        roster.contains("`BatchOptimizerMultiThreaded.java:259`"),
        "…and `:259` is the single construction site of the second"
    );

    let optimizer = include_str!("../src/pipeline/optimizer.rs");
    assert!(
        optimizer.contains("not ported: `BatchOptimizer.createForGui` (`:56-66`)"),
        "the door itself is rostered beside the code, where the class's second map row points"
    );
    assert!(
        optimizer.contains("GuiRoutingJobWorker.java:212"),
        "with the GUI caller that is the only reason it exists"
    );
    assert!(
        optimizer.contains("settings/FeatureFlagsSettings.java:11"),
        "and with `featureFlags.multiThreading`, the one static mutable global in Plan 7's scope — \
         read at `:58` inside the factory the port does not have"
    );
}

// =================================================================================================
// The routed-board half — release only (`optimizer_items.rs`' convention)
// =================================================================================================

/// `:182-193` on a real board. `p7t9 router-only` leaves `Issue143-rpi_splitter` at `799.98267`
/// after one pass, so a threshold of `0.5` puts `score * 1.5` over the 1000 ceiling and the
/// optimizer exits **before** the first pass runs — but **after** `:177` has counted it, which is
/// why `passes_run` is 1 while `per_pass` is empty and no item was touched.
#[test]
#[cfg_attr(debug_assertions, ignore)]
fn a_near_perfect_board_exits_before_the_first_pass() {
    if !parity::require_java_dir() {
        return;
    }
    let (mut board, mut settings) = routed_rpi();
    optimizer_settings(&mut settings).optimization_improvement_threshold = Some(0.5);
    let before = board.structural_hash();

    let mut optimizer = BatchOptimizer::new(&settings);
    let stop = RouterStop::new();
    let mut sink = NoopProgressSink;
    let result = optimizer
        .run_batch_loop(&mut board, &stop, RouterBudget::disabled(), &mut sink)
        .expect("the loop breaks at `:192`");

    assert_eq!(
        result.passes_run, 1,
        ":177 counts the pass `:192` then abandons"
    );
    assert!(result.per_pass.is_empty(), "no pass completed");
    assert_eq!(result.items_optimized, 0);
    assert_eq!(
        board.structural_hash(),
        before,
        "the board was never touched"
    );
}

/// `:349-361` — `maxConsecutiveFailures` consecutive unimproved items end the pass early.
///
/// The literals are `p7t8 <rpi> item 1 all`'s, which is the JVM transcript this file's sibling
/// pins at 10/10 MATCH: item 0 (`id=43`, a via) improves and item 1 (`id=49`) does not. With the
/// limit set to **1**, that single failure ends the pass — so exactly two items are visited,
/// where the unbounded walk visits seven.
#[test]
#[cfg_attr(debug_assertions, ignore)]
fn consecutive_failures_break_the_pass() {
    if !parity::require_java_dir() {
        return;
    }
    let (mut board, mut settings) = routed_rpi();
    {
        let optimizer = optimizer_settings(&mut settings);
        optimizer.max_consecutive_failures = Some(1);
        optimizer.max_passes = Some(1);
    }
    let mut optimizer = BatchOptimizer::new(&settings);
    // `runBatchLoop:132`, which `opt_route_pass` alone does not set.
    optimizer.use_increased_ripup_costs = true;
    let stop = RouterStop::new();
    let mut sink = NoopProgressSink;
    optimizer
        .opt_route_pass(
            &mut board,
            1,
            true,
            &stop,
            RouterBudget::disabled(),
            &mut sink,
        )
        .expect("the pass runs");

    assert_eq!(
        optimizer.total_items_optimized, 2,
        "`p7t8 item` improves item 0 and fails item 1; one failure is the limit"
    );

    // The same pass with Java's default limit walks the board to exhaustion — seven items, which
    // is `p7t8`'s `ITEMS-END count=7`.
    let (mut board, settings) = routed_rpi();
    let mut optimizer = BatchOptimizer::new(&settings);
    optimizer.use_increased_ripup_costs = true;
    let mut sink = NoopProgressSink;
    optimizer
        .opt_route_pass(
            &mut board,
            1,
            true,
            &stop,
            RouterBudget::disabled(),
            &mut sink,
        )
        .expect("the pass runs");
    assert_eq!(optimizer.total_items_optimized, 7);
}

/// **Quirk #227, half two — the seam Task 15 must not flatten.**
///
/// Java shares one `StoppableThread` between the two stages (`RoutingPipeline.java:81-85`) and
/// **never lowers the flag**: `grep -rn requestStop src/main` finds no writer that does. So after
/// any ordinary router run the flag is `AUTO_ROUTER_ONLY` (quirk #214), the optimizer stage still
/// runs (`:171` reads `ALL`), and every `optRouteItem` inside it calls
/// `autoroutePassesForOptimizingItem`, whose loop head is `!isStopAutoRouterRequested()`
/// (`BatchAutorouter.java:268`) — **zero** autoroute passes. Each item rips its connections,
/// measures a strictly worse board and restores the snapshot.
///
/// The measurable consequence: the stage visits every item and changes **nothing**.
#[test]
#[cfg_attr(debug_assertions, ignore)]
fn an_auto_router_only_stop_leaves_every_item_rejected() {
    if !parity::require_java_dir() {
        return;
    }
    let (mut board, mut settings) = routed_rpi();
    optimizer_settings(&mut settings).max_passes = Some(1);
    let before = board.structural_hash();

    let mut optimizer = BatchOptimizer::new(&settings);
    let stop = RouterStop::new();
    // What `AutorouteBatchLoop.java:271` left behind, and what `RoutingPipeline` hands on.
    stop.request_stop_auto_router();
    let mut sink = NoopProgressSink;
    let result = optimizer
        .run_batch_loop(&mut board, &stop, RouterBudget::disabled(), &mut sink)
        .expect("the stage runs");

    assert_eq!(
        result.passes_run, 1,
        "the stage is not disabled — `:171` reads `ALL`"
    );
    assert_eq!(
        result.items_optimized, 6,
        "and it visits every item the reader offers — `p7t9 <rpi> 1 optimizer-shared 2 all`'s \
         `OPT-RESULT items=6`, all six `improved=false`"
    );
    assert_eq!(
        board.structural_hash(),
        before,
        "…and changes not one byte of the board, because every item routed zero passes and was \
         restored from its snapshot"
    );

    // The same board with a clean flag does change, which is what makes the assertion above a
    // measurement of the seam rather than of an inert optimizer.
    let (mut board, settings) = routed_rpi();
    let mut optimizer = BatchOptimizer::new(&settings);
    let clean = RouterStop::new();
    let mut sink = NoopProgressSink;
    optimizer
        .run_batch_loop(&mut board, &clean, RouterBudget::disabled(), &mut sink)
        .expect("the stage runs");
    assert_ne!(
        board.structural_hash(),
        before,
        "with the flag down the optimizer really does move the board"
    );
}

/// The whole stage against the JVM: the literals are `p7t9 <rpi> 1 optimizer`'s `OPT-PASS` and
/// `OPT-RESULT` lines, i.e. the transcript the differential harness compares byte for byte.
///
/// It is here as well as in the harness because a driver can be edited and a test cannot be
/// forgotten: if `run_batch_loop`'s termination condition drifts, this fails without a JVM.
#[test]
#[cfg_attr(debug_assertions, ignore)]
fn the_optimizer_stage_matches_the_jvm() {
    if !parity::require_java_dir() {
        return;
    }
    let (mut board, settings) = routed_rpi();
    let mut optimizer = BatchOptimizer::new(&settings);
    let stop = RouterStop::new();
    let mut sink = NoopProgressSink;
    let result = optimizer
        .run_batch_loop(&mut board, &stop, RouterBudget::disabled(), &mut sink)
        .expect("the stage runs");

    // `p7t9 <rpi> 1 optimizer 2 all`, the `OPT-RESULT` line:
    //   `state=FINISHED passesRun=2 items=7 timedOut=false useIncreasedRipupCosts=true
    //    finalScore=999.986`.
    assert_eq!(result.state, TaskState::Finished);
    assert_eq!(
        result.passes_run, 2,
        "`:177` counted pass 2, `:182-193` abandoned it"
    );
    assert_eq!(result.items_optimized, 7);
    assert!(!result.timed_out);
    assert!(
        optimizer.use_increased_ripup_costs,
        "pass 1 improved, so neither `:365-368` nor `:212-215` fired"
    );

    // …and its single `OPT-PASS` line — pass 2 never completed, because `:182-193` fired on a
    // board that pass 1 had taken to 999.986 (`999.986 * 1.01 >= 1000`).
    assert_eq!(result.per_pass.len(), 1);
    let pass = result.per_pass[0];
    assert_eq!(pass.pass, 1);
    assert!(pass.with_preferred_directions, ":200 — pass 1 is odd");
    assert_eq!(pass.score_before, 799.982_67);
    assert_eq!(pass.score_after, 999.986);
    assert_eq!(pass.total_items_optimized, 7);
    assert_eq!(pass.record.incomplete_count, 0);
    assert_eq!(pass.record.clearance_violations, 0);
    assert_eq!(pass.record.via_count, 2);
    assert_eq!(pass.record.trace_count, 14);
    // `OPT-PASS … passImprovement=0.25000961324540416 scoreImprovement=0.25000961324540416
    //  routeImproved=0.063065656`.
    assert_eq!(pass.pass_improvement, 0.250_009_613_245_404_16);
    assert_eq!(pass.score_improvement, pass.pass_improvement);
    assert_eq!(pass.route_improved, 0.063_065_656);
}
