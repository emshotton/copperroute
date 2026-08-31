//! Plan 7 Task 5: `RoutingBoard.optChangedArea` (RoutingBoard.java:151-161 -> :171-190 ->
//! `RoutingBoardOperations.java:52-79`) and the `TraceTightener.optChangedArea(ExpansionCostFactor[])`
//! sweep it drives (TraceTightener.java:121-169) — the batch pull-tight every routed connection,
//! every tail removal and every fanout pin runs.
//!
//! # Where the numbers come from
//!
//! The end-to-end evidence is `scripts/differential/java/P7T3.java`, whose stdout on
//! `Issue143-rpi_splitter.dsn` is committed as `tests/data/p7t3-opt-changed-area.txt` (its
//! `HEADER` line, which names the jar by absolute path, is stripped) and diffed live by
//! `scripts/differential/run.sh p7t3`. [`the_whole_sweep_matches_the_jvm_on_a_real_board`] replays
//! modes 0-3 of it here.
//!
//! **Mode 4 is deliberately not replayed.** It offers `traceCosts` to the sweep, which opens
//! `TraceTightener.java:160-165`'s `ViaOptimizer.optViaLocation` arm. Plan 7 Task 6 landed that
//! arm — it is a live call now, not a stub — but `repositionVia` overload A is an
//! `unimplemented!` until Task 7 (controller ruling B1: a `None` there would push
//! `optPlaneOrFanoutVia` into a branch that *inserts*, and move a via somewhere Java never puts
//! it). Two of this fixture's six vias reach it, so mode 4 **panics**;
//! [`mode_four_is_task_sevens_obligation`] is the `#[should_panic]` that says so, and mode 4's
//! section is in the committed transcript so Task 7 has the ground truth to match.
//!
//! The rest of the file is hand-built: each test isolates one branch of the two methods, because
//! a real board exercises them all at once and could not say which one moved.

use fr_board::items::Item;
use fr_board::prelude::*;
use fr_dsn::java_double_to_string;
use fr_dsn::{BoardReadResult, DsnReadOptions};
use fr_geometry::{IntBox, IntOctagon, IntPoint, Line, Point, Polyline};
use fr_router::board_ext::{RoutingBoardExt, TraceTightener};
use fr_router::pipeline::{RouterBudget, RouterStop};
use fr_router::route_connection;
use fr_settings::sources::DefaultSettings;
use fr_settings::{ExpansionCostFactor, HostEnvironment, RouterSettings, SettingsSource};
use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};

// =================================================================================================
// A hand-built board with two loose traces in free space
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

fn layers() -> LayerStructure {
    LayerStructure::new(vec![Layer::new("front", true), Layer::new("back", true)])
}

fn rules(default_clearance: i32) -> BoardRules {
    let clearance_matrix = ClearanceMatrix::get_default_instance(&layers(), default_clearance);
    let mut rules = BoardRules::new(layers(), clearance_matrix);
    rules.trace_angle_restriction = AngleRestriction::None;
    rules
}

fn p(x: i32, y: i32) -> Point {
    Point::new(x, y)
}

/// Two detours on layer 0 and one on layer 1, all in free space, all shortenable — so every
/// `pullTight` in the sweep answers `true` and `somethingChanged` keeps the outer `while` going.
fn detour_board(default_clearance: i32) -> Board {
    let mut board = Board::new(
        Vec::new(),
        0,
        BOUNDING_BOX,
        rules(default_clearance),
        BoardLibrary::new(Padstacks::new(layers()), Packages::new()),
        Components::new(),
        Communication::default(),
    );
    let default_class = board.rules.get_default_net_class();
    board.rules.nets.add("N1", 1, false, default_class);
    board.rules.nets.add("N2", 1, false, default_class);
    board.rules.nets.add("N3", 1, false, default_class);

    for (corners, layer, net) in [
        (
            vec![p(-3000, -3000), p(-3000, -1000), p(-1000, -1000)],
            0usize,
            1i32,
        ),
        (vec![p(1000, -3000), p(1000, -1000), p(3000, -1000)], 0, 2),
        (vec![p(-3000, 2000), p(-3000, 4000), p(-1000, 4000)], 1, 3),
    ] {
        board.insert_trace_without_cleaning(
            Polyline::from_points(&corners),
            layer,
            30,
            vec![net],
            1,
            FixedState::Unfixed,
        );
    }
    board
}

/// Every trace id on the board, ascending.
fn trace_ids(board: &Board) -> Vec<ItemId> {
    let mut ids: Vec<ItemId> = board
        .get_items()
        .filter(|item| matches!(item, Item::Trace(_)))
        .map(Item::id)
        .collect();
    ids.reverse();
    ids
}

fn polyline_of(board: &Board, id: ItemId) -> Polyline {
    let Some(Item::Trace(trace)) = board.items.get(&id) else {
        panic!("{id:?} is not a trace")
    };
    trace.polyline().clone()
}

/// `startMarkingChangedArea` followed by `markChangedArea` over every trace's own tile shapes —
/// what `TraceShover.insert:571-575` and `PolylineTrace.change` do for real, condensed.
fn mark_every_trace(board: &mut Board) {
    board.start_marking_changed_area();
    for id in trace_ids(board) {
        let Some(Item::Trace(trace)) = board.items.get(&id) else {
            continue;
        };
        let layer = trace.get_layer();
        let corners: Vec<Point> = (0..trace.polyline().corner_count())
            .filter_map(|i| trace.polyline().corner(i))
            .collect();
        for corner in corners {
            board.join_changed_area(&corner.to_float(), layer);
        }
    }
}

fn never() -> bool {
    false
}

// =================================================================================================
// `RoutingBoardOperations.optChangedArea:61-63` and `:78`
// =================================================================================================

/// `:61-63`: a `null` `changedArea` returns before anything else, so no tightener is built and
/// no trace moves.
#[test]
fn a_null_changed_area_returns_immediately() {
    let mut board = detour_board(200);
    assert!(board.changed_area.is_none());
    let before: Vec<Polyline> = trace_ids(&board)
        .into_iter()
        .map(|id| polyline_of(&board, id))
        .collect();

    board
        .opt_changed_area(None, &[], None, 500, None, &never, 0)
        .expect("cannot fail");

    let after: Vec<Polyline> = trace_ids(&board)
        .into_iter()
        .map(|id| polyline_of(&board, id))
        .collect();
    assert_eq!(before, after);
    assert!(board.changed_area.is_none());
}

/// `:78`, `board.changedArea = null`. Load-bearing: the next `startMarkingChangedArea` re-creates
/// the store, and leaving this one in place would make the following sweep see a stale region.
#[test]
fn the_changed_area_is_cleared_after_the_sweep() {
    let mut board = detour_board(200);
    mark_every_trace(&mut board);
    assert!(board.changed_area.is_some());

    board
        .opt_changed_area(None, &[], None, 500, None, &never, 0)
        .expect("cannot fail");
    assert!(board.changed_area.is_none());

    // And the next marking starts from empty rather than from this sweep's leftovers.
    board.start_marking_changed_area();
    let changed_area = board.changed_area.as_ref().expect("just started");
    for layer in 0..board.get_layer_count() {
        assert!(changed_area.get_area(layer).is_empty());
    }
}

/// Ruling 9, `RoutingBoardOperations.java:64`. Java's guard is the **reference** comparison
/// `clipShape != IntOctagon.EMPTY`, so a `null` clip shape — "no restriction" — *runs* the sweep,
/// and `Option::None` therefore does too. Only the `EMPTY` singleton skips it.
#[test]
fn a_none_clip_shape_runs_the_branch() {
    let mut board = detour_board(200);
    mark_every_trace(&mut board);
    let before: Vec<Polyline> = trace_ids(&board)
        .into_iter()
        .map(|id| polyline_of(&board, id))
        .collect();

    board
        .opt_changed_area(None, &[], None, 500, None, &never, 0)
        .expect("cannot fail");

    let after: Vec<Polyline> = trace_ids(&board)
        .into_iter()
        .map(|id| polyline_of(&board, id))
        .collect();
    assert_ne!(before, after, "a None clip shape must run the sweep");
}

/// The other half of ruling 9: the `EMPTY` octagon — which `Route.java:225` and
/// `RoutingBoardOperations:86` do pass — skips the tightener entirely, while still clearing
/// `changedArea` at `:78`.
#[test]
fn an_empty_clip_shape_skips_the_tightener_but_still_clears_the_area() {
    let mut board = detour_board(200);
    mark_every_trace(&mut board);
    let before: Vec<Polyline> = trace_ids(&board)
        .into_iter()
        .map(|id| polyline_of(&board, id))
        .collect();

    board
        .opt_changed_area(None, &[], Some(IntOctagon::EMPTY), 500, None, &never, 0)
        .expect("cannot fail");

    let after: Vec<Polyline> = trace_ids(&board)
        .into_iter()
        .map(|id| polyline_of(&board, id))
        .collect();
    assert_eq!(before, after, "the EMPTY singleton skips the sweep");
    assert!(board.changed_area.is_none(), ":78 runs either way");
}

// =================================================================================================
// `TraceTightener.optChangedArea:132-145` — the per-layer region
// =================================================================================================

/// `:136`, `board.changedArea.setEmpty(i)`, which runs **before** the work rather than after it.
/// The tightener that follows re-marks whatever it moves (through `PolylineTrace.change`), so the
/// outer `while (somethingChanged)` sees the *new* region and not the old one. A port that emptied
/// the layer afterwards would wipe those marks and stop a pass early.
///
/// Driven through `TraceTightener::opt_changed_area` directly, because the wrapper nulls the whole
/// store at `:78` and the per-layer state would not be observable through it.
#[test]
fn the_layer_region_is_emptied_before_the_work() {
    let mut board = detour_board(200);
    mark_every_trace(&mut board);
    let before_layer_1 = board.changed_area.as_ref().expect("marked").get_area(1);
    assert!(!before_layer_1.is_empty());

    let mut algo =
        TraceTightener::get_instance(&mut board, Vec::new(), None, 500, None, 0, None, -1);
    algo.opt_changed_area(&mut board, None, None)
        .expect("cannot fail");

    // The sweep ran to completion, so `somethingChanged` was false on the last iteration and
    // every layer it emptied at `:136` stayed empty.
    let changed_area = board
        .changed_area
        .as_ref()
        .expect("the tightener never nulls it");
    for layer in 0..board.get_layer_count() {
        assert!(
            changed_area.get_area(layer).is_empty(),
            "layer {layer} was left marked"
        );
    }
}

/// `:138-142`, `1.5 * (clearanceMatrix.maxValue(i) + 2 * rules.getMaxTraceHalfWidth())`, over
/// three clearance matrices. The offset is what decides which items the sweep can see at all, so
/// it is asserted against the formula rather than against a behaviour that happens to agree.
#[test]
fn the_enlarge_offset_is_javas_formula() {
    for default_clearance in [0, 200, 1_000] {
        let board = detour_board(default_clearance);
        for layer in 0..board.get_layer_count() {
            let max_clearance = board.rules.clearance_matrix.max_value_on_layer(layer);
            let expected =
                1.5 * f64::from(max_clearance + 2 * board.rules.get_max_trace_half_width());

            // The formula's two inputs, pinned: `maxValue(int)` is the per-layer maximum over the
            // **whole** matrix (`ClearanceMatrix.java:216-220`), not `maxValue(classI, layer)`;
            // and `getMaxTraceHalfWidth` is `BoardRules.maxTraceHalfWidth`, which `BoardRules`'
            // constructor seeds to 100 (BoardRules.java:59) and only `setTraceHalfWidth` raises —
            // `insertTrace` does not, so the three 30-half-width traces leave it at the seed.
            assert_eq!(max_clearance, default_clearance);
            assert_eq!(board.rules.get_max_trace_half_width(), 100);
            assert_eq!(expected, 1.5 * f64::from(default_clearance + 200));

            // And the enlarged region is the marked one grown by exactly that.
            let mut marked = detour_board(default_clearance);
            mark_every_trace(&mut marked);
            let region = marked
                .changed_area
                .as_ref()
                .expect("marked")
                .get_area(layer);
            if !region.is_empty() {
                let enlarged = region.enlarge(expected);
                let grown = f64::from(region.left_x) - expected;
                assert_eq!(
                    f64::from(enlarged.left_x),
                    grown.floor().max(f64::from(i32::MIN))
                );
                assert_eq!(
                    f64::from(enlarged.right_x),
                    (f64::from(region.right_x) + expected).ceil()
                );
            }
        }
    }
}

// =================================================================================================
// `TraceTightener.optChangedArea:146-166` — the item loop
// =================================================================================================

/// `:150-159`. **Both trace arms `break`, but not on the same condition**: the `pullTight` arm
/// breaks only when `splitTracesAtKeepPoint()` answers `true` (`:153-155`), while the
/// `smoothenEndCornersAtTrace` arm breaks unconditionally (`:156-158`, "because items may be
/// removed"). The plan's prose folded the two into one unconditional `break`; Java wins.
///
/// With no keep point — which is every router caller — `splitTracesAtKeepPoint` is a no-op
/// answering `false`, so a layer whose objects all tighten is walked to the end in one pass. The
/// test asserts that: all three traces of the board move in the first outer iteration.
#[test]
fn the_item_loop_does_not_break_after_a_plain_pull_tight() {
    let mut board = detour_board(200);
    mark_every_trace(&mut board);
    let ids = trace_ids(&board);
    assert_eq!(
        ids.len(),
        3,
        "two shortenable traces on layer 0, one on layer 1"
    );
    let before: Vec<Polyline> = ids.iter().map(|id| polyline_of(&board, *id)).collect();

    // The instrument is **which traces have moved when the sweep is cut**, not how many times the
    // stop check was read: `is_stop_requested` is also read inside the tighteners' own loops
    // (Plan 6), so the read count is not a clean per-object counter and a first draft of this test
    // that asserted `calls > 3` was satisfied by both behaviours.
    //
    // Cutting on the sixth read separates them. Measured, by building the same board twice — once
    // against this port and once against a copy whose `:153-155` break was made unconditional:
    //
    // | behaviour | layer-0 trace 0 | layer-0 trace 1 | layer-1 trace |
    // |---|---|---|---|
    // | Java's, conditional on `splitTracesAtKeepPoint()` (this port) | moved | moved | untouched |
    // | the brief's reading, unconditional `break` | **untouched** | moved | **moved** |
    //
    // With no keep point — which is every router caller, `RouteState` being the only one that
    // passes one — `splitTracesAtKeepPoint` is a no-op answering `false`, so layer 0 is walked to
    // the end and the cut lands on layer 1's first object. An unconditional break leaves layer 0
    // after one object and reaches layer 1 a whole outer iteration earlier.
    let calls = Cell::new(0u32);
    let stop = || {
        calls.set(calls.get() + 1);
        calls.get() >= 6
    };
    let mut algo =
        TraceTightener::get_instance(&mut board, Vec::new(), None, 500, Some(&stop), 0, None, -1);
    // The tightener has no keep point, so this is the arm's condition and it is `false`.
    assert!(
        !algo
            .split_traces_at_keep_point(&mut board)
            .expect("cannot fail"),
        "splitTracesAtKeepPoint is a no-op without a keep point (TraceTightener.java:476-491)"
    );
    algo.opt_changed_area(&mut board, None, None)
        .expect("cannot fail");

    let after: Vec<Polyline> = ids.iter().map(|id| polyline_of(&board, *id)).collect();
    assert_ne!(
        before[0], after[0],
        "the layer-0 walk stopped after one object — the `pullTight` arm broke unconditionally"
    );
    assert_ne!(
        before[1], after[1],
        "the first layer-0 object was not tightened"
    );
    assert_eq!(
        before[2], after[2],
        "layer 1 must still be untouched at the cut"
    );
}

/// `:147-149`, the sweep's only cut. A stop check that trips part-way returns with the rest of the
/// board **untightened** and with `board.changedArea` already emptied for every layer walked so
/// far — so those regions are lost. That is a fact about the port, not a bug to be fixed: it is
/// Java's own control flow, and it is why every parity run uses [`RouterBudget::disabled`].
#[test]
fn a_tripped_stop_check_returns_mid_sweep_leaving_the_rest_untightened() {
    let mut board = detour_board(200);
    mark_every_trace(&mut board);
    let ids = trace_ids(&board);
    let before: Vec<Polyline> = ids.iter().map(|id| polyline_of(&board, *id)).collect();

    // Trip on the very first read, so nothing is tightened at all.
    let stop = || true;
    let mut algo =
        TraceTightener::get_instance(&mut board, Vec::new(), None, 500, Some(&stop), 0, None, -1);
    algo.opt_changed_area(&mut board, None, None)
        .expect("cannot fail");

    let after: Vec<Polyline> = ids.iter().map(|id| polyline_of(&board, *id)).collect();
    assert_eq!(before, after, "the cut is before the first pullTight");

    // And layer 0 — the one the sweep reached — was emptied at `:136` before the cut, so its
    // marks are gone even though nothing was optimised there.
    let changed_area = board
        .changed_area
        .as_ref()
        .expect("not nulled by the tightener");
    assert!(changed_area.get_area(0).is_empty());

    // The parity configuration: no stop check and no budget, so the sweep runs to the end.
    let mut board = detour_board(200);
    mark_every_trace(&mut board);
    let budget = RouterBudget::disabled();
    assert_eq!(budget.opt_changed_area_ms, 0);
    board
        .opt_changed_area(
            None,
            &[],
            None,
            500,
            None,
            &never,
            budget.opt_changed_area_ms,
        )
        .expect("cannot fail");
    let after: Vec<Polyline> = ids.iter().map(|id| polyline_of(&board, *id)).collect();
    for (i, (b, a)) in before.iter().zip(after.iter()).enumerate() {
        assert_ne!(b, a, "trace {i} was not tightened with the budget disabled");
    }
}

/// The Rust-only half of controller ruling AI's knob: a **budget** — not a stop check — trips the
/// same `:147` read, through the `TimeLimit` `TraceTightener`'s constructor builds when
/// `timeLimit > 0` (TraceTightener.java:73-77). A 1 ms budget on an already-expired clock cuts the
/// sweep; the default is Java's own literal 1000.
#[test]
fn the_budget_trips_the_sweep() {
    assert_eq!(RouterBudget::default().opt_changed_area_ms, 1000);

    let mut board = detour_board(200);
    mark_every_trace(&mut board);
    let ids = trace_ids(&board);
    let before: Vec<Polyline> = ids.iter().map(|id| polyline_of(&board, *id)).collect();

    // `TimeLimit::new(1)` starts the clock now; the sleep puts it behind before the first read.
    let mut algo =
        TraceTightener::get_instance(&mut board, Vec::new(), None, 500, None, 1, None, -1);
    std::thread::sleep(std::time::Duration::from_millis(5));
    algo.opt_changed_area(&mut board, None, None)
        .expect("cannot fail");

    let after: Vec<Polyline> = ids.iter().map(|id| polyline_of(&board, *id)).collect();
    assert_eq!(
        before, after,
        "an expired budget cuts before the first pullTight"
    );

    // A zero budget is Java's "no limit" and builds no `TimeLimit` at all.
    let mut board = detour_board(200);
    mark_every_trace(&mut board);
    let mut algo =
        TraceTightener::get_instance(&mut board, Vec::new(), None, 500, None, 0, None, -1);
    std::thread::sleep(std::time::Duration::from_millis(5));
    algo.opt_changed_area(&mut board, None, None)
        .expect("cannot fail");
    let after: Vec<Polyline> = ids.iter().map(|id| polyline_of(&board, *id)).collect();
    for (i, (b, a)) in before.iter().zip(after.iter()).enumerate() {
        assert_ne!(b, a, "trace {i} was not tightened at timeLimit = 0");
    }
    // The three-state stop is a separate mechanism and is not read here: `TraceTightener` takes a
    // `Stoppable`, which the port models as a `StopCheck`, and `RouterStop` reaches it only
    // through a closure a caller writes.
    let router_stop = RouterStop::new();
    assert!(!router_stop.is_stop_requested());
}

// =================================================================================================
// The `ViaOptimizer` arm — Task 6 landed it, Task 7 closes it
// =================================================================================================

/// `:160-165`. Offering `traceCosts` must not change the **trace** side of the sweep: the arm is
/// reached only for a `Via`, so on a board with no via at all it cannot fire.
///
/// This is a guard against the via arm leaking into the trace arms. It is **not** a statement
/// about `optViaLocation`, which Task 6 landed and this board never reaches; the statement about
/// the arm's remaining gap is [`mode_four_is_task_sevens_obligation`], which runs on a real board
/// that does have vias in its changed area.
#[test]
fn offering_trace_costs_does_not_change_the_trace_arms() {
    let mut board = detour_board(200);
    mark_every_trace(&mut board);
    let ids = trace_ids(&board);
    let with_costs: Vec<ExpansionCostFactor> = vec![
        ExpansionCostFactor {
            horizontal: 1.0,
            vertical: 1.0,
        };
        board.get_layer_count()
    ];
    board
        .opt_changed_area(None, &[], None, 500, Some(&with_costs), &never, 0)
        .expect("cannot fail");
    let with = ids
        .iter()
        .map(|id| polyline_of(&board, *id))
        .collect::<Vec<_>>();

    let mut board = detour_board(200);
    mark_every_trace(&mut board);
    board
        .opt_changed_area(None, &[], None, 500, None, &never, 0)
        .expect("cannot fail");
    let without = ids
        .iter()
        .map(|id| polyline_of(&board, *id))
        .collect::<Vec<_>>();

    assert_eq!(
        with, without,
        "the ViaOptimizer arm must not reach the trace arms"
    );
}

// =================================================================================================
// The `p7t3` transcript — the whole sweep over a real routed board
// =================================================================================================

const TRANSCRIPT: &str = include_str!("data/p7t3-opt-changed-area.txt");

fn transcript_mode(mode: i32) -> Vec<&'static str> {
    let header = format!("######## mode {mode}");
    let mut rows = Vec::new();
    let mut inside = false;
    for line in TRANSCRIPT.lines() {
        if line.starts_with("######## ") {
            inside = line == header;
            continue;
        }
        if inside {
            rows.push(line.trim_end());
        }
    }
    assert!(!rows.is_empty(), "transcript mode {mode} is empty");
    rows
}

/// `P7T3`'s run on `Issue143-rpi_splitter.dsn` with `accuracy = 500` and `routeK = 12`, replayed.
fn p7t3_rows(mode: i32) -> Vec<String> {
    let mut out = Vec::new();
    let path = parity::fixture("Issue143-rpi_splitter.dsn");
    let file = std::fs::File::open(&path)
        .unwrap_or_else(|e| panic!("cannot open {}: {e}", path.display()));
    let design_name = "Issue143-rpi_splitter.dsn";
    let mut board =
        match fr_dsn::read_board(file, None, Some(design_name), &DsnReadOptions::default()) {
            BoardReadResult::Success { board, .. }
            | BoardReadResult::OutlineMissing { board, .. } => {
                *board.expect("the fixture produces a board")
            }
            other => panic!("{design_name} did not read: {other:?}"),
        };
    let settings = build_settings(&board);

    for (k, (item_id, net_no)) in pick_connections(&board, 12).into_iter().enumerate() {
        let k = k + 1;
        if board.get_item(item_id).is_none() {
            out.push(format!("route k={k} item={} state=GONE", item_id.0));
            continue;
        }
        board.start_marking_changed_area();
        let mut ripped: BTreeSet<ItemId> = BTreeSet::new();
        let mut ripup_costs: BTreeMap<ItemId, i32> = BTreeMap::new();
        let trace_costs = settings.get_trace_costs();
        let mut engine = None;
        let result = route_connection(
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
            &never,
        );
        out.push(format!(
            "route k={k} item={} net={net_no} state={} ripped={}",
            item_id.0,
            result.state.name(),
            ripped.len()
        ));
    }

    let regime = match mode {
        0 => AngleRestriction::NinetyDegree,
        1 => AngleRestriction::FortyFiveDegree,
        _ => AngleRestriction::None,
    };
    board.rules.trace_angle_restriction = regime;
    if mode == 3 {
        board.rules.set_pin_edge_to_turn_dist(100_000.0);
    }
    let via_costs: Option<Vec<ExpansionCostFactor>> = if mode == 4 {
        Some(vec![
            ExpansionCostFactor {
                horizontal: 1.0,
                vertical: 1.0,
            };
            board.get_layer_count()
        ])
    } else {
        None
    };

    out.push(format!(
        "sweep regime={} pinEdgeToTurnDist={} traceCosts={}",
        regime_name(regime),
        java_double_to_string(board.rules.get_pin_edge_to_turn_dist()),
        match &via_costs {
            Some(costs) => costs.len().to_string(),
            None => "null".to_string(),
        }
    ));
    out.push(dump_changed_area(&board, "before"));
    board
        .opt_changed_area(
            None,
            &[],
            None,
            500,
            via_costs.as_deref(),
            &never,
            RouterBudget::disabled().opt_changed_area_ms,
        )
        .expect("cannot fail");
    out.push(dump_changed_area(&board, "after"));
    out.extend(dump_board(&board));
    out
}

#[test]
fn the_whole_sweep_matches_the_jvm_on_a_real_board() {
    for mode in [0, 1, 2, 3] {
        assert_eq!(p7t3_rows(mode), transcript_mode(mode), "p7t3 mode {mode}");
    }
}

/// Mode 4 offers `traceCosts`, which opens the `ViaOptimizer` arm. Plan 7 Task 6 landed
/// `optViaLocation` and wired the arm, but its `repositionVia` overloads are **Task 7's** — and
/// overload A is an **`unimplemented!`**, not a `None`, by controller ruling B1: answering `None`
/// there would send `optPlaneOrFanoutVia` into its `:218-260` projection branch, which *inserts*,
/// and which Java reaches only when its own overload A answered `null`. A stubbed arm must be
/// inert or loud, never a silent port-only mutation, so mode 4 **panics** rather than diverging
/// quietly.
///
/// This fixture at `routeK = 12` puts two `PLANE_OR_FANOUT_ONE_CONTACT` vias in the changed area
/// (ids 187 and 84 — `crates/fr-router/tests/via_optimizer.rs`'s
/// `a_plane_via_reaches_task_sevens_guard` names them), so the sweep reaches the guard.
///
/// **Task 7 must delete this test and add mode 4 to the loop above.**
#[test]
#[should_panic(expected = "repositionVia overload A")]
fn mode_four_is_task_sevens_obligation() {
    // added in Task 7: `ViaOptimizer.repositionVia` overload A (ViaOptimizer.java:302-365) — when
    // it lands, this becomes `assert_eq!(p7t3_rows(4), transcript_mode(4))` inside the loop above.
    let _ = p7t3_rows(4);
}

// -- `P6T1.java`'s choices, as `reference_parity.rs` transcribes them ------------------------------

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

// -- `P7T3.java`'s dumps ---------------------------------------------------------------------------

fn regime_name(angle: AngleRestriction) -> &'static str {
    match angle {
        AngleRestriction::NinetyDegree => "NINETY_DEGREE",
        AngleRestriction::FortyFiveDegree => "FORTYFIVE_DEGREE",
        AngleRestriction::None => "NONE",
    }
}

fn dump_changed_area(board: &Board, tag: &str) -> String {
    let Some(changed_area) = &board.changed_area else {
        return format!("changedArea {tag}=null");
    };
    let mut parts = Vec::new();
    for i in 0..board.get_layer_count() {
        let area = changed_area.get_area(i);
        parts.push(if area.is_empty() {
            "empty".to_string()
        } else {
            format!(
                "({},{},{},{},{},{},{},{})",
                area.left_x,
                area.bottom_y,
                area.right_x,
                area.top_y,
                area.upper_left_diagonal_x,
                area.lower_right_diagonal_x,
                area.lower_left_diagonal_x,
                area.upper_right_diagonal_x
            )
        });
    }
    format!("changedArea {tag}=[{}]", parts.join(","))
}

fn dump_line(line: &Line) -> String {
    format!("({},{})->({},{})", line.a.x, line.a.y, line.b.x, line.b.y)
}

fn dump_corner(polyline: &Polyline, no: usize) -> String {
    match polyline.corner(no).expect("no is below cornerCount") {
        Point::Int(point) => format!("({},{})", point.x, point.y),
        Point::Rational(_) => {
            let f = polyline.corner_approx(no).expect("no is below cornerCount");
            format!(
                "~({},{})",
                java_double_to_string(f.x),
                java_double_to_string(f.y)
            )
        }
    }
}

fn dump_polyline(polyline: &Polyline) -> String {
    let lines: Vec<String> = polyline.lines().iter().map(dump_line).collect();
    let corners: Vec<String> = (0..polyline.corner_count())
        .map(|i| dump_corner(polyline, i))
        .collect();
    format!(
        "n={} lines=[{}] corners=[{}]",
        polyline.lines().len(),
        lines.join(","),
        corners.join(",")
    )
}

fn dump_point(point: &Point) -> String {
    let rounded = point.to_float().round();
    format!("({},{})", rounded.x, rounded.y)
}

fn dump_nets(net_nos: &[i32]) -> String {
    let inner: Vec<String> = net_nos.iter().map(i32::to_string).collect();
    format!("[{}]", inner.join(","))
}

fn dump_fixed_state(state: FixedState) -> &'static str {
    match state {
        FixedState::Unfixed => "UNFIXED",
        FixedState::ShoveFixed => "SHOVE_FIXED",
        FixedState::UserFixed => "USER_FIXED",
        FixedState::SystemFixed => "SYSTEM_FIXED",
    }
}

fn dump_board(board: &Board) -> Vec<String> {
    let mut out = vec![format!(
        "maxId={}",
        board.communication.id_gen.max_generated_id()
    )];
    for item in board.get_items() {
        let type_name = match item {
            Item::Trace(_) => "PolylineTrace",
            Item::Via(_) => "Via",
            Item::Pin(_) => "Pin",
            Item::ObstacleArea(_) => "ObstacleArea",
            Item::ConductionArea(_) => "ConductionArea",
            Item::ViaObstacleArea(_) => "ViaObstacleArea",
            Item::ComponentObstacleArea(_) => "ComponentObstacleArea",
            Item::ComponentOutline(_) => "ComponentOutline",
            Item::BoardOutline(_) => "BoardOutline",
        };
        let mut line = format!(
            "item id={} type={} nets={} cl={} fix={}",
            item.id().0,
            type_name,
            dump_nets(item.net_nos()),
            item.clearance_class(),
            dump_fixed_state(item.get_fixed_state())
        );
        match item {
            Item::Trace(trace) => {
                line.push_str(&format!(
                    " layer={} hw={} {}",
                    trace.get_layer(),
                    trace.get_half_width(),
                    dump_polyline(trace.polyline())
                ));
            }
            Item::Via(_) | Item::Pin(_) => {
                let center = board
                    .drill_center(item.id())
                    .expect("a drill item has a centre");
                line.push_str(&format!(" center={}", dump_point(&center)));
            }
            _ => {}
        }
        out.push(line);
    }
    out
}
