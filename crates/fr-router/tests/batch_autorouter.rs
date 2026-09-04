//! Plan 7 Task 8: [`BatchAutorouter`]'s scaffold, `removeTails`, `removeItemsAndPullTight` and
//! `AutorouteConnectionRouter.route`'s steps 6-8.
//!
//! # Where the numbers come from
//!
//! The end-to-end evidence is `scripts/differential/run.sh p6t1 <dsn> <maxItems> <pass> - 1-8
//! [neckWidthUm]`, whose Java half calls `AutorouteConnectionRouter.route` **in full** through
//! `scripts/differential/java/probes/P7T8Probe.java`. The transcripts are committed as
//! `tests/reference/<stem>/router-steps18.jsonl`. The tests below isolate the branches a whole
//! board exercises all at once and could not attribute.
//!
//! Four of them replay the `Issue143-rpi_splitter.dsn` prefix the driver runs, because the necked
//! retry (`route:123-145` -> `retryConnectionNecked:162-241`) is unreachable on a hand-built
//! board: it needs a connection that genuinely **fails** at full trace width and a layer whose
//! half width really is wider than the neck. On `rpi_splitter` the second connection is exactly
//! that — measured with the driver: at `neckWidthUm = 0` it ends `FAILED` with `maxIdAfter = 83`,
//! and at `neckWidthUm = 100` it ends `FAILED` with `maxIdAfter = 90` (connection 1 ends at 63
//! either way), i.e. the retry ran and
//! spent seven item ids on an attempt that did not route (Java does not roll the failed neck
//! attempt back either). Both numbers are the **jar's**, and `run.sh p6t1 … 1-8 100` MATCHes them.

use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

use fr_board::StopConnectionOption;
use fr_board::items::Item;
use fr_board::prelude::*;
use fr_dsn::{BoardReadResult, DsnReadOptions};
use fr_geometry::{IntBox, IntOctagon, IntPoint, Point, Polyline, Shape, TileShape};
use fr_router::board_ext::RoutingBoardExt;
use fr_router::pipeline::{BatchAutorouter, NamedAlgorithmType, RouterBudget};
use fr_router::{AutorouteAttemptState, AutorouteEngine, route_connection, route_connection_full};
use fr_settings::sources::DefaultSettings;
use fr_settings::{HostEnvironment, RouterSettings, SettingsSource};

// =================================================================================================
// Hand-built boards
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

fn p(x: i32, y: i32) -> Point {
    Point::new(x, y)
}

fn never() -> bool {
    false
}

/// [`empty_board`] plus one through-via padstack, so `insert_via` has something to place.
/// `PadstackId(1)` is the first id the port hands out (`Padstacks::add`).
fn via_board() -> Board {
    let clearance_matrix = ClearanceMatrix::get_default_instance(&layers(), 200);
    let mut rules = BoardRules::new(layers(), clearance_matrix);
    rules.trace_angle_restriction = AngleRestriction::None;
    let mut padstacks = Padstacks::new(layers());
    let shape = Shape::Tile(TileShape::Octagon(IntOctagon::new(
        -100, -100, 100, 100, -200, 200, -200, 200,
    )));
    let via = padstacks.add("via", vec![Some(shape.clone()), Some(shape)], true, false);
    assert_eq!(PadstackId(1), via, "the port's padstack ids start at 1");
    Board::new(
        Vec::new(),
        0,
        BOUNDING_BOX,
        rules,
        BoardLibrary::new(padstacks, Packages::new()),
        Components::new(),
        Communication::default(),
    )
}

/// A two-layer board with `default_clearance` and the given trace-angle regime.
fn empty_board(default_clearance: i32, regime: AngleRestriction) -> Board {
    let clearance_matrix = ClearanceMatrix::get_default_instance(&layers(), default_clearance);
    let mut rules = BoardRules::new(layers(), clearance_matrix);
    rules.trace_angle_restriction = regime;
    Board::new(
        Vec::new(),
        0,
        BOUNDING_BOX,
        rules,
        BoardLibrary::new(Padstacks::new(layers()), Packages::new()),
        Components::new(),
        Communication::default(),
    )
}

fn add_net(board: &mut Board, name: &str, no: i32) {
    let default_class = board.rules.get_default_net_class();
    board.rules.nets.add(name, no, false, default_class);
}

fn insert_trace(
    board: &mut Board,
    corners: &[Point],
    layer: usize,
    half_width: i32,
    net: i32,
) -> ItemId {
    board
        .insert_trace_without_cleaning(
            Polyline::from_points(corners),
            layer,
            half_width,
            vec![net],
            1,
            FixedState::Unfixed,
        )
        .expect("a two-corner polyline always inserts")
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

fn corners_of(board: &Board, id: ItemId) -> Vec<(i32, i32)> {
    let Some(Item::Trace(trace)) = board.items.get(&id) else {
        panic!("{id:?} is not a trace")
    };
    (0..trace.polyline().corner_count())
        .filter_map(|i| trace.polyline().corner(i))
        .map(|c| match c {
            Point::Int(ip) => (ip.x, ip.y),
            Point::Rational(_) => {
                let f = c.to_float();
                (f.x as i32, f.y as i32)
            }
        })
        .collect()
}

// =================================================================================================
// The corpus prefix the driver runs — `Issue143-rpi_splitter.dsn`
// =================================================================================================

/// `P6T1.loadBoard` without a `.rules` file.
fn load_rpi() -> Board {
    let path = parity::java_dir().join("fixtures/Issue143-rpi_splitter.dsn");
    let file = std::fs::File::open(&path)
        .unwrap_or_else(|e| panic!("cannot open {}: {e}", path.display()));
    let design_name = "Issue143-rpi_splitter.dsn";
    match fr_dsn::read_board(file, None, Some(design_name), &DsnReadOptions::default()) {
        BoardReadResult::Success { board, .. } | BoardReadResult::OutlineMissing { board, .. } => {
            *board.unwrap_or_else(|| panic!("{design_name} produced no board"))
        }
        other => panic!("{design_name} did not read: {other:?}"),
    }
}

/// `P6T1.main`'s three settings lines, plus Task 8's `neckWidthUm`.
fn rpi_settings(board: &Board, neck_width_um: f64) -> RouterSettings {
    let mut settings = DefaultSettings::new(&HostEnvironment::detect())
        .get_settings()
        .expect("DefaultSettings always answers a table")
        .clone();
    settings.set_layer_count(board.get_layer_count());
    settings.apply_board_specific_optimizations(board);
    settings.neck_width_um = Some(neck_width_um);
    settings
}

/// `P6T1.pickConnections`.
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

/// The driver's per-connection loop, `--steps=1-8`, over the first `max_items` connections.
///
/// Returns the board, the engine the last connection left behind (the retry's, when it ran),
/// `maxGeneratedId()` after each connection — the "item ids each connection burned" the
/// acceptance ladder compares — and the instant the **last** `route_connection_full` call started
/// together with how long it took, which is quirk #208's measuring stick.
fn route_prefix(
    max_items: usize,
    neck_width_um: f64,
) -> (Board, Option<AutorouteEngine>, Vec<u32>, Instant, Duration) {
    let mut board = load_rpi();
    let settings = rpi_settings(&board, neck_width_um);
    let trace_costs = settings.get_trace_costs();
    let mut engine: Option<AutorouteEngine> = None;
    let mut max_ids = Vec::new();
    let mut last_call_start = Instant::now();
    let mut last_call_duration = Duration::ZERO;

    for (item_id, net_no) in pick_connections(&board, max_items) {
        if board.get_item(item_id).is_none() {
            max_ids.push(board.communication.id_gen.max_generated_id().0);
            continue;
        }
        // The precondition `docs/plan-6-handoff.md` §3 states: every production-shaped caller
        // marks, because quirk #177 makes the presence of `changedArea` observable.
        board.start_marking_changed_area();
        let mut ripped: BTreeSet<ItemId> = BTreeSet::new();
        let mut ripup_costs: BTreeMap<ItemId, i32> = BTreeMap::new();
        last_call_start = Instant::now();
        route_connection_full(
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
            settings.trace_pull_tight_accuracy.unwrap_or(500),
            RouterBudget::disabled(),
            &never,
        );
        last_call_duration = last_call_start.elapsed();
        max_ids.push(board.communication.id_gen.max_generated_id().0);
    }
    (board, engine, max_ids, last_call_start, last_call_duration)
}

// =================================================================================================
// The constants — BatchAutorouter.java:38-64
// =================================================================================================

/// Every constant of `:38-64`, against the Java literals.
#[test]
fn the_constants_are_javas_literals() {
    assert_eq!(30, BatchAutorouter::BOARD_RANK_LIMIT, ":40");
    assert_eq!(3, BatchAutorouter::MAXIMUM_TRIES_ON_THE_SAME_BOARD, ":42");
    assert_eq!(
        1000,
        BatchAutorouter::TIME_LIMIT_TO_PREVENT_ENDLESS_LOOP,
        ":43"
    );
    assert_eq!(8, BatchAutorouter::STOP_AT_PASS_MINIMUM, ":46");
    assert_eq!(4, BatchAutorouter::STOP_AT_PASS_MODULO, ":49");
    assert_eq!(10, BatchAutorouter::STAGNATION_PASS_LIMIT, ":52");
    assert_eq!(3, BatchAutorouter::FANOUT_RECOVERY_STAGNATION_PASSES, ":54");
    assert_eq!(
        10,
        BatchAutorouter::PROGRESS_STATISTICS_ITEM_INTERVAL,
        ":57"
    );
    assert!(
        (BatchAutorouter::STAGNATION_SCORE_THRESHOLD - 0.5_f32).abs() < f32::EPSILON,
        ":60"
    );
    // `:40` is written as the reference Java writes, not as the literal.
    assert_eq!(
        fr_router::BoardHistory::MAX_HISTORY_SIZE,
        BatchAutorouter::BOARD_RANK_LIMIT,
        "BOARD_RANK_LIMIT = BoardHistory.MAX_HISTORY_SIZE (:40)"
    );
    // `NamedAlgorithm`'s identity, `:423-441` and `:460-463`.
    assert_eq!("freerouting-router", BatchAutorouter::ID);
    assert_eq!("Freerouting Auto-router", BatchAutorouter::NAME);
    assert_eq!("1.0", BatchAutorouter::VERSION);
    assert_eq!("Freerouting Auto-router v1.0", BatchAutorouter::DESCRIPTION);
    assert_eq!(NamedAlgorithmType::Router, BatchAutorouter::TYPE);
}

/// Controller ruling AJ's pattern, applied to **both** `Boolean.getBoolean` properties of
/// `:61-64`: the port hard-codes `false` and offers nothing that could flip either.
///
/// This is the roster half of the pair — a source assertion, not a behavioural one, in the shape
/// Plan 6 used for `item_tree_shape_ref`. The behavioural half is
/// [`retain_autoroute_database_is_false_on_every_path`] below.
#[test]
fn retain_autoroute_database_has_no_setter() {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut offenders = Vec::new();
    for entry in walk(&src) {
        let text = std::fs::read_to_string(&entry).expect("readable source");
        for (no, line) in text.lines().enumerate() {
            let trimmed = line.trim_start();
            // A doc comment, a `//` comment or a marker is prose, not a setter.
            if trimmed.starts_with("//") {
                continue;
            }
            // The only two places the flag may be *written* are its own `const` declaration and
            // the field initialiser that copies it.
            let writes_the_flag = (line.contains("retain_autoroute_database")
                && (line.contains('=') || line.contains("&mut")))
                && !line.contains("retain_autoroute_database: BatchAutorouter::")
                && !line.contains("retain_autoroute_database: bool")
                && !line.contains("let retain_autoroute_database");
            if writes_the_flag {
                offenders.push(format!("{}:{}: {}", entry.display(), no + 1, line.trim()));
            }
            if trimmed.starts_with("pub const BENCHMARK_RETAIN_AUTOROUTE_DATABASE")
                && !trimmed.contains("false")
            {
                offenders.push(format!("{}:{}: {}", entry.display(), no + 1, trimmed));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "ruling AJ: `retainAutorouteDatabase` must have no setter anywhere in `fr-router`, but \
         these lines write it:\n{}",
        offenders.join("\n")
    );
    // And the two constants really are `false`. Written through a runtime binding so the
    // assertions are not constant-folded away — the claim is about the values ruling AJ fixes,
    // and a `const { assert!(..) }` would move the check to compile time and out of the test's
    // own report.
    let retain: bool = std::hint::black_box(BatchAutorouter::BENCHMARK_RETAIN_AUTOROUTE_DATABASE);
    let profile: bool = std::hint::black_box(BatchAutorouter::BENCHMARK_PROFILE_ENABLED);
    assert!(
        !retain,
        "ruling AJ: retainAutorouteDatabase is permanently false"
    );
    assert!(
        !profile,
        "the same pattern for -Dfreerouting.benchmark.profile"
    );
    assert!(!BatchAutorouter::is_benchmark_profile_enabled());
}

fn walk(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(walk(&path));
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
    out
}

/// The behavioural half of ruling AJ's pair: `route_connection_full` behaved as
/// `retainAutorouteDatabase = false`.
///
/// The observable the flag controls is `AutorouteEngine.maintainDatabase`, which
/// `RoutingBoard.initAutoroute:892` sets from it and which is the **only** gate on
/// `RoutingBoard.additionalUpdateAfterChange` (`:100-102`). With it `false`, no complete free
/// space expansion room is ever invalidated by an insert, a change or a removal — which is why
/// the five `fr-board` sites carry `// not reachable:` markers rather than deferrals.
#[test]
fn retain_autoroute_database_is_false_on_every_path() {
    if !parity::require_java_dir() {
        return;
    }
    let (_board, engine, _ids, _, _) = route_prefix(1, 0.0);
    let engine = engine.expect("initAutoroute always answers an engine");
    assert!(
        !engine.maintain_database,
        "route_connection_full must build its engine with maintainDatabase = false, so \
         RoutingBoard.additionalUpdateAfterChange returns at :100-102 and never runs"
    );
}

// =================================================================================================
// shouldFireBoardUpdate — BatchAutorouter.java:335-343
// =================================================================================================

/// `shouldFireBoardUpdate()` (`:335-343`) is the strict `> 250` gate, and the 250 is
/// [`RouterBudget::board_update_throttle_ms`] so a driver can pin it (ruling AI).
#[test]
fn should_fire_board_update_uses_the_budget() {
    let board = empty_board(200, AngleRestriction::None);
    let settings = RouterSettings::new();

    // Java's own literal.
    let router = BatchAutorouter::new(
        &board,
        &settings,
        false,
        true,
        100,
        500,
        RouterBudget::default(),
    );
    let t0 = Instant::now();
    // `lastBoardUpdateTimestamp` starts at 0, which any clock is more than 250 past.
    assert!(
        router.should_fire_board_update_at(t0),
        "the first call fires"
    );
    assert!(
        !router.should_fire_board_update_at(t0 + Duration::from_millis(250)),
        "exactly 250 ms does not fire — Java's test is strict `>` (:337-338)"
    );
    assert!(
        router.should_fire_board_update_at(t0 + Duration::from_millis(251)),
        "251 ms fires"
    );

    // The knob: a different budget is a different gate, and the same instants answer differently.
    let budget = RouterBudget {
        board_update_throttle_ms: 1000,
        ..RouterBudget::default()
    };
    let slow = BatchAutorouter::new(&board, &settings, false, true, 100, 500, budget);
    assert!(slow.should_fire_board_update_at(t0));
    assert!(!slow.should_fire_board_update_at(t0 + Duration::from_millis(999)));
    assert!(slow.should_fire_board_update_at(t0 + Duration::from_millis(1001)));
}

// =================================================================================================
// removeTails — BatchAutorouter.java:487-503
// =================================================================================================

/// `removeTails(StopConnectionOption)` (`:487-503`), first half: `:489`'s
/// `startMarkingChangedArea` and `:490`'s `removeTraceTails(-1, option)`.
///
/// `-1` is "all nets" (`RoutingBoard.java:1200`'s `netNumber > 0` test), and the walk keeps only
/// items that are `isRoutable()` **and** carry exactly one net (`:1195-1219`). Both traces here
/// qualify and both are tails, so both go — and the changed area is cleared by the
/// `optChangedArea` that follows (`RoutingBoardOperations.java:78`), which is what tells this
/// method apart from a bare `removeTraceTails`.
#[test]
fn remove_tails_strips_every_tail_and_clears_the_changed_area() {
    let mut board = empty_board(200, AngleRestriction::None);
    add_net(&mut board, "N1", 1);
    let spine = insert_trace(
        &mut board,
        &[p(-3000, -3000), p(-3000, -1000), p(-1000, -1000)],
        0,
        30,
        1,
    );
    let stub = insert_trace(&mut board, &[p(-1000, -1000), p(-1000, 2000)], 0, 30, 1);
    assert!(
        board.changed_area.is_none(),
        "removeTails must do its own :489 startMarkingChangedArea"
    );

    let settings = RouterSettings::new();
    let router = BatchAutorouter::new(
        &board,
        &settings,
        false,
        true,
        100,
        500,
        RouterBudget::disabled(),
    );
    router
        .remove_tails(&mut board, None, StopConnectionOption::None, &never)
        .expect("removeTails cannot fail here");

    assert!(board.get_item(stub).is_none(), ":490 strips the stub");
    assert!(
        board.get_item(spine).is_none(),
        ":490 strips the spine too — with no pin to anchor it, it is a tail as well"
    );
    assert!(trace_ids(&board).is_empty());
    assert!(
        board.changed_area.is_none(),
        ":492-498's optChangedArea clears the area at RoutingBoardOperations.java:78"
    );
}

/// `removeTails`'s second half, `:492-498`: the `optChangedArea(new int[0], null,
/// tracePullTightAccuracy, traceCosts, thread, TIME_LIMIT_TO_PREVENT_ENDLESS_LOOP)` sweep.
///
/// **`removeTraceTails` marks nothing** — `RoutingBoard.java:1193-1238` has no `joinChangedArea`,
/// and `:489`'s `startMarkingChangedArea` only *creates* the store when it is `null`
/// (`RoutingBoardOperations.java:27-29`); it does not reset one. So the region this sweep
/// optimises is whatever the pass marked while it was routing, which is why
/// `AutoroutePassRunner.java:298-302` calls `removeTails` at the **end** of a pass and not at the
/// start.
///
/// The board is therefore a real one with a real connection routed into it and the changed area
/// left standing, which is the only shape in which the two halves are separable: on a hand-built
/// board with no pins every trace is a tail (`RoutingBoard.java:1204`), so `:490` empties the
/// board and leaves the sweep nothing to do.
///
/// The `removeTails` call itself has no `p6t1` counterpart — its Java caller is
/// `AutoroutePassRunner.runSingleThread`, which is **Task 9's** — so what is pinned here is that
/// both statements ran, not a jar literal.
#[test]
fn remove_tails_pulls_the_marked_area_tight() {
    if !parity::require_java_dir() {
        return;
    }
    // Steps **1-5** only, i.e. `fr_router::route_connection`: the connection is inserted and the
    // changed area is left standing, which is the state `AutoroutePassRunner` is in when it
    // reaches `:298-302`. Routing through `route_connection_full` instead would have run step 6's
    // own `optChangedArea` already, leaving this one a measured no-op.
    let mut board = load_rpi();
    let settings = rpi_settings(&board, 0.0);
    let trace_costs = settings.get_trace_costs();
    let mut engine: Option<AutorouteEngine> = None;
    let (item_id, net_no) = pick_connections(&board, 1)[0];
    board.start_marking_changed_area();
    route_connection(
        &mut board,
        &mut engine,
        item_id,
        net_no,
        &settings,
        &trace_costs,
        &mut BTreeSet::new(),
        &mut BTreeMap::new(),
        1,
        settings.get_start_ripup_costs(),
        !settings.is_fanout_enabled(),
        false,
        &never,
    );
    assert!(
        board.changed_area.is_some(),
        "steps 1-5 leave the marked area standing"
    );
    let traces_before: Vec<(ItemId, Vec<(i32, i32)>)> = trace_ids(&board)
        .into_iter()
        .map(|id| (id, corners_of(&board, id)))
        .collect();
    let max_id_before = board.communication.id_gen.max_generated_id();

    let router = BatchAutorouter::for_routing_job(&board, &settings, RouterBudget::disabled());
    router
        .remove_tails(&mut board, None, StopConnectionOption::None, &never)
        .expect("removeTails cannot fail here");

    let traces_after: Vec<(ItemId, Vec<(i32, i32)>)> = trace_ids(&board)
        .into_iter()
        .map(|id| (id, corners_of(&board, id)))
        .collect();
    assert_ne!(
        traces_before, traces_after,
        "removeTails must have changed the board"
    );
    assert!(
        board.communication.id_gen.max_generated_id().0 > max_id_before.0,
        ":492-498's optChangedArea replaced at least one trace, burning an item id ({} -> {})",
        max_id_before.0,
        board.communication.id_gen.max_generated_id().0
    );
    assert!(
        board.changed_area.is_none(),
        ":492-498's optChangedArea clears the area at RoutingBoardOperations.java:78"
    );
    // And the accuracy and cost array it swept with are this router's, not defaults.
    assert_eq!(500, router.get_trace_pull_tight_accuracy(), ":118-120");
    assert_eq!(
        board.get_layer_count(),
        router.get_trace_costs().len(),
        ":139 — one ExpansionCostFactor per layer"
    );
}

/// The `StopConnectionOption` argument reaches `removeTraceTails` unchanged, and the two values
/// **disagree** on a board that has a via stub — which is the only shape in which the forwarding
/// is observable.
///
/// `RoutingBoard.java:1207-1216`: a stub that is a `Via` is skipped when the option is `VIA`, and
/// skipped when the option is `FANOUT_VIA` and the via is a fanout via. So on a trace-plus-via
/// tail, `NONE` takes both and `VIA` leaves the via standing. `AutoroutePassRunner.java:298-302`
/// is the live chooser (`removeUnconnectedVias ? NONE : FANOUT_VIA`), so this is not decoration.
#[test]
fn remove_tails_forwards_the_stop_connection_option() {
    let build = || {
        let mut board = via_board();
        add_net(&mut board, "N1", 1);
        let trace = insert_trace(&mut board, &[p(-3000, 0), p(-1000, 0)], 0, 30, 1);
        let via = board
            .insert_via(
                PadstackId(1),
                p(-1000, 0),
                vec![1],
                1,
                FixedState::Unfixed,
                true,
            )
            .expect("the via inserts");
        (board, trace, via)
    };
    let settings = RouterSettings::new();

    let (mut none_board, none_trace, none_via) = build();
    let (mut via_board, via_trace, via_via) = build();
    let router = BatchAutorouter::new(
        &none_board,
        &settings,
        false,
        true,
        100,
        500,
        RouterBudget::disabled(),
    );
    router
        .remove_tails(&mut none_board, None, StopConnectionOption::None, &never)
        .expect("cannot fail");
    router
        .remove_tails(&mut via_board, None, StopConnectionOption::Via, &never)
        .expect("cannot fail");

    assert!(
        none_board.get_item(none_trace).is_none() && none_board.get_item(none_via).is_none(),
        "StopConnectionOption::NONE takes the trace and the via"
    );
    assert!(
        via_board.get_item(via_trace).is_none() && via_board.get_item(via_via).is_some(),
        "StopConnectionOption::VIA takes the trace and leaves the via (RoutingBoard.java:1207-1209)"
    );
    assert_ne!(
        none_board.structural_hash(),
        via_board.structural_hash(),
        "the two options must produce different boards, or the argument could be ignored"
    );
}

// =================================================================================================
// removeItemsAndPullTight — RoutingBoardOperations.java:81-120
// =================================================================================================

/// `:111-113` — `combineTraces(netNo)` over the changed nets in **ascending** net order, which is
/// what `new TreeSet<Integer>` at `:93` gives.
///
/// The board carries two nets whose traces can each be combined into one, and the removed item
/// belongs to both. The assertion is that both nets were combined *and* that the set the port
/// iterates is the ascending one — the order is asserted directly on
/// `Board::remove_items_marking_changed_area`'s answer, because `combineTraces` is idempotent
/// across nets and the result alone could not distinguish the two orders.
#[test]
fn remove_items_and_pull_tight_combines_traces_in_ascending_net_order() {
    let mut board = empty_board(200, AngleRestriction::None);
    add_net(&mut board, "N7", 7);
    add_net(&mut board, "N3", 3);

    // Two collinear halves per net, which `combineTraces` can merge once the item between them
    // is gone. The victim is a two-net item so both nets end up in `changedNets`.
    let a1 = insert_trace(&mut board, &[p(-4000, 0), p(-2000, 0)], 0, 30, 7);
    let a2 = insert_trace(&mut board, &[p(-2000, 0), p(0, 0)], 0, 30, 7);
    let victim = board
        .insert_trace_without_cleaning(
            Polyline::from_points(&[p(0, 0), p(0, 1000)]),
            0,
            30,
            vec![7, 3],
            1,
            FixedState::Unfixed,
        )
        .expect("inserts");
    let b1 = insert_trace(&mut board, &[p(2000, 3000), p(4000, 3000)], 1, 30, 3);
    let b2 = insert_trace(&mut board, &[p(4000, 3000), p(6000, 3000)], 1, 30, 3);

    // The order the port hands `combineTraces`, straight off `fr-board`'s answer.
    let mut probe = board.clone();
    let (_removed_all, changed_nets) = probe.remove_items_marking_changed_area(vec![victim]);
    assert_eq!(
        vec![3, 7],
        changed_nets.into_iter().collect::<Vec<_>>(),
        ":93's TreeSet<Integer> is ascending — net 3 is combined before net 7"
    );

    // And the real call does the combining.
    let ok = board
        .remove_items_and_pull_tight(None, &[victim], 0, 500)
        .expect("cannot fail here");
    assert!(ok, "nothing on this board is fixed, so the answer is true");
    assert!(board.get_item(victim).is_none());
    // `combineTraces` keeps one of each pair and absorbs the other, so the assertion is on the
    // survivor count rather than on which id survived.
    assert_eq!(2, trace_ids(&board).len(), "one merged trace per net");
    assert_eq!(
        1,
        [a1, a2]
            .iter()
            .filter(|id| board.get_item(**id).is_some())
            .count(),
        "net 7's two halves became one"
    );
    assert_eq!(
        1,
        [b1, b2]
            .iter()
            .filter(|id| board.get_item(**id).is_some())
            .count(),
        "net 3's two halves became one"
    );
}

/// `:91-92` — a fixed item is refused and the method answers `false`, and the *other* items are
/// still removed.
#[test]
fn remove_items_and_pull_tight_refuses_a_user_fixed_item() {
    let mut board = empty_board(200, AngleRestriction::None);
    add_net(&mut board, "N1", 1);
    let removable = insert_trace(&mut board, &[p(-4000, 0), p(-2000, 0)], 0, 30, 1);
    let fixed = board
        .insert_trace_without_cleaning(
            Polyline::from_points(&[p(2000, 0), p(4000, 0)]),
            0,
            30,
            vec![1],
            1,
            FixedState::UserFixed,
        )
        .expect("inserts");

    let ok = board
        .remove_items_and_pull_tight(None, &[removable, fixed], 0, 500)
        .expect("cannot fail here");
    assert!(!ok, ":92 sets result = false for a user-fixed item");
    assert!(board.get_item(removable).is_none());
    assert!(board.get_item(fixed).is_some());
}

/// `:83-89` and `:114-116` — the three `tidyWidth` arms, and quirk #204's guard.
///
/// `tidyWidth <= 0` hands `optChangedArea` the `IntOctagon.EMPTY` **singleton**, so quirk #204's
/// reference test is `false` and the whole tightener sweep is skipped: the removal happens, the
/// nets are combined, but nothing is pulled tight. `tidyWidth == Integer.MAX_VALUE` hands it
/// `null` — no restriction. Anything in between hands it a real octagon.
#[test]
fn a_non_positive_tidy_width_skips_the_tightener_entirely() {
    let build = || {
        let mut board = empty_board(200, AngleRestriction::None);
        add_net(&mut board, "N1", 1);
        // A detour the sweep would straighten if it ran…
        insert_trace(
            &mut board,
            &[p(-3000, -3000), p(-3000, -1000), p(-1000, -1000)],
            0,
            30,
            1,
        );
        // …and something to remove right beside it, so the changed area the removal marks —
        // enlarged by `1.5 * (clearance + 2 * maxTraceHalfWidth)` at `TraceTightener.java:138-142`
        // — actually reaches the detour.
        let victim = insert_trace(&mut board, &[p(-3000, -2000), p(-2800, -2000)], 0, 30, 1);
        (board, victim)
    };

    let (mut skipped, victim) = build();
    let detour = trace_ids(&skipped)[0];
    let before = corners_of(&skipped, detour);
    skipped
        .remove_items_and_pull_tight(None, &[victim], 0, 500)
        .expect("cannot fail");
    assert!(
        skipped.get_item(victim).is_none(),
        "the removal still happens"
    );
    assert_eq!(
        before,
        corners_of(&skipped, detour),
        "tidyWidth = 0 -> IntOctagon.EMPTY -> quirk #204's guard is false -> no sweep"
    );

    let (mut unrestricted, victim) = build();
    unrestricted
        .remove_items_and_pull_tight(None, &[victim], i32::MAX, 500)
        .expect("cannot fail");
    assert_ne!(
        before,
        corners_of(&unrestricted, trace_ids(&unrestricted)[0]),
        "tidyWidth = Integer.MAX_VALUE -> a null clip shape -> the sweep runs unrestricted"
    );
    assert_eq!(
        1,
        trace_ids(&unrestricted).len(),
        "the detour is the only trace left in either arm"
    );
}

/// **Quirk #184's reachability obligation** (plan lines 206/777/869/871) — discharged *in the half
/// that is measurable here*, and the other half named rather than claimed.
///
/// `TraceTightener45.reduceCorners` copies a *stale* `currentCornerInClipShape[3]` onto slot 2
/// (`TraceTightener45.java:81, :90-91, :100-101`), and the flag it copies gates the two translate
/// attempts at `:103-105` and `:148-151`. The quirk is **reachable only with a non-null
/// `currentClipShape`**. Task 5 re-checked every `autoroute/pipeline` caller and they all pass
/// `null` (`AutorouteConnectionRouter:103, :223`, `BatchAutorouter:492`,
/// `BatchAutorouterThread:527, :563`), which is why the plan handed the obligation here:
/// `removeItemsAndPullTight:115-116`, with `0 < tidyWidth < Integer.MAX_VALUE`, is the **only**
/// call in either language that builds a real clip octagon (out of the removed items' shapes,
/// enlarged by `tidyWidth`) and hands it to `TraceTightener`.
///
/// # What this test pins
///
/// **That the clip octagon really arrives, and really restricts.** A 45-degree board, a
/// staircase, and a small victim beside it: at `tidyWidth = 1` the clip is the victim's own box
/// and the staircase keeps eight of its ten corners; at `Integer.MAX_VALUE` the clip is `null`
/// and the same sweep, over the same changed area, collapses it to two. Both answers are
/// literals. A `TraceTightener` that never received the octagon could not produce the first.
///
/// # What this test does **not** pin, measured
///
/// It does not exercise quirk #184's own statement. Instrumenting
/// `tightener_45.rs`'s `current_corner_in_clip_shape[2] = current_corner_in_clip_shape[3];`
/// shows it is reached **exactly once** in the whole test and **with `current_clip_shape ==
/// None`** (`stale = true`, `fresh = true`, trivially equal in Java too); under the clip octagon
/// the `:85-99` skip block it lives in is **never entered at all**. Consistently, replacing the
/// copy with the maximally discriminating mutation `!current_corner_in_clip_shape[3]` leaves
/// [`QUIRK_184_CLIPPED_STAIRCASE`] **unchanged** and moves only
/// [`QUIRK_184_UNCLIPPED_STAIRCASE`].
///
/// So the register row says exactly that: the clipped `optChangedArea` path is reachable and
/// pinned, and the stale-copy statement is **not yet reached with a non-null clip shape**.
/// Closing that half needs a board whose clip octagon cuts through a corner the skip block
/// actually skips — a duplicate corner or a collinear middle corner inside the clip — and the
/// `obligation:` marker beside the `// Java bug:` line in `tightener_45.rs` records it.
#[test]
fn remove_items_and_pull_tight_hands_the_tightener_a_live_clip_octagon() {
    let build = || {
        let mut board = empty_board(200, AngleRestriction::FortyFiveDegree);
        add_net(&mut board, "N1", 1);
        // A staircase with enough corners for `reduceCorners` to make more than one pass.
        let staircase = insert_trace(
            &mut board,
            &[
                p(-6000, -6000),
                p(-6000, -4000),
                p(-4000, -4000),
                p(-4000, -2000),
                p(-2000, -2000),
                p(-2000, 0),
                p(0, 0),
            ],
            0,
            30,
            1,
        );
        // The item whose removed shape *is* the clip region.
        let victim = insert_trace(&mut board, &[p(-4000, -3000), p(-3800, -3000)], 0, 30, 1);
        (board, staircase, victim)
    };

    // The middle arm: a real, small clip octagon.
    let (mut clipped, staircase, victim) = build();
    let before = corners_of(&clipped, staircase);
    assert_eq!(
        QUIRK_184_STAIRCASE_AS_INSERTED.to_vec(),
        before,
        "the staircase as inserted"
    );
    clipped
        .remove_items_and_pull_tight(None, &[victim], 1, 500)
        .expect("cannot fail here");
    let clipped_after = corners_of(&clipped, trace_ids(&clipped)[0]);
    assert_eq!(
        QUIRK_184_CLIPPED_STAIRCASE.to_vec(),
        clipped_after,
        "a clip octagon the size of the removed item restricts the sweep to the corners inside \
         it — which is only possible if `TraceTightener` really received one"
    );

    // The `Integer.MAX_VALUE` arm: the same removal, the same changed area, **no** clip.
    let (mut unclipped, _staircase, victim) = build();
    unclipped
        .remove_items_and_pull_tight(None, &[victim], i32::MAX, 500)
        .expect("cannot fail here");
    let unclipped_after = corners_of(&unclipped, trace_ids(&unclipped)[0]);
    assert_ne!(
        before, unclipped_after,
        "with no clip the same sweep does move the staircase, so the clip above is what stopped it"
    );
    assert_eq!(
        QUIRK_184_UNCLIPPED_STAIRCASE.to_vec(),
        unclipped_after,
        "the unclipped answer, corner for corner"
    );
}

/// The staircase [`remove_items_and_pull_tight_hands_the_tightener_a_live_clip_octagon`] inserts.
const QUIRK_184_STAIRCASE_AS_INSERTED: &[(i32, i32)] = &[
    (-6000, -6000),
    (-6000, -4000),
    (-4000, -4000),
    (-4000, -2000),
    (-2000, -2000),
    (-2000, 0),
    (0, 0),
];

/// The answer the **clipped** sweep — `0 < tidyWidth < Integer.MAX_VALUE`, quirk #184's only
/// reachable caller — leaves the staircase at. Neither the original nor the unclipped answer.
const QUIRK_184_CLIPPED_STAIRCASE: &[(i32, i32)] = &[
    (-6000, -6000),
    (-6000, -5999),
    (-4001, -4000),
    (-4000, -4000),
    (-4000, -3999),
    (-2001, -2000),
    (-2000, -2000),
    (-2000, -1999),
    (-1, 0),
    (0, 0),
];

/// The answer the **unclipped** sweep leaves the same staircase at.
const QUIRK_184_UNCLIPPED_STAIRCASE: &[(i32, i32)] = &[(-6000, -6000), (0, 0)];

// =================================================================================================
// Steps 6-8 — AutorouteConnectionRouter.java:95-153
// =================================================================================================

/// Step 6 (`:95-121`) runs on `ROUTED` and nothing else, and the observable is the changed area:
/// `optChangedArea` is the only thing that clears it (`RoutingBoardOperations.java:78`), so a
/// connection that ends `FAILED` leaves it **standing** — which is the state quirk #177 makes
/// visible inside `TraceShover::insert` on the *next* connection. It is not bookkeeping.
///
/// Both halves are driven through [`route_connection_full`] on `Issue143-rpi_splitter.dsn`,
/// because a hand-built board has no `Connectable` start item and cannot enter `route` at all.
/// The fixture supplies one of each: connection 1 ends `ROUTED` and connection 2 ends `FAILED`
/// (the jar's own answers — `tests/reference/router-rpi-splitter/router-steps18.jsonl`).
#[test]
fn step_six_runs_only_on_routed() {
    if !parity::require_java_dir() {
        return;
    }
    let mut board = load_rpi();
    let settings = rpi_settings(&board, 0.0);
    let trace_costs = settings.get_trace_costs();
    let mut engine: Option<AutorouteEngine> = None;
    let connections = pick_connections(&board, 2);

    let route = |board: &mut Board, engine: &mut Option<AutorouteEngine>, i: usize| {
        let (item_id, net_no) = connections[i];
        board.start_marking_changed_area();
        route_connection_full(
            board,
            engine,
            item_id,
            net_no,
            &settings,
            &trace_costs,
            &mut BTreeSet::new(),
            &mut BTreeMap::new(),
            1,
            settings.get_start_ripup_costs(),
            !settings.is_fanout_enabled(),
            settings.trace_pull_tight_accuracy.unwrap_or(500),
            RouterBudget::disabled(),
            &never,
        )
    };

    // `ROUTED` -> step 6 ran -> the area is cleared.
    let first = route(&mut board, &mut engine, 0);
    assert_eq!(AutorouteAttemptState::Routed, first.state);
    assert!(
        board.changed_area.is_none(),
        ":103-109's optChangedArea must have run and cleared the area"
    );

    // Not `ROUTED` -> step 6 skipped -> the area the connection marked is still standing.
    let second = route(&mut board, &mut engine, 1);
    assert_eq!(AutorouteAttemptState::Failed, second.state);
    assert!(
        board.changed_area.is_some(),
        "a FAILED connection must leave its marked area for the next one — the `if state == \
         ROUTED` guard at :95 is the only thing that can"
    );
}

/// Step 7 is **skipped** when no active layer is wider than the neck (`:181-190`): the loop
/// answers `narrowerSomewhere = false` and `retryConnectionNecked` returns `null` before it
/// builds anything, so the board is byte-identical to the run with no neck width at all.
///
/// The neck here is 100 000 um = 100 mm, far wider than any trace on `rpi_splitter`. Both halves
/// of the pair are asserted: the wide neck changes nothing, and
/// [`the_necked_retry_fires_and_spends_item_ids`] shows a narrow one does.
#[test]
fn the_necked_retry_is_skipped_when_no_layer_is_wider_than_the_neck() {
    if !parity::require_java_dir() {
        return;
    }
    let (no_neck, _, ids_no_neck, _, _) = route_prefix(2, 0.0);
    let (wide_neck, _, ids_wide_neck, _, _) = route_prefix(2, 100_000.0);
    assert_eq!(
        ids_no_neck, ids_wide_neck,
        "a neck no layer is wider than must burn no item id"
    );
    assert_eq!(
        no_neck.structural_hash(),
        wide_neck.structural_hash(),
        "…and must leave the board byte-identical"
    );
}

/// Step 7 **fires** at a neck narrower than the board's traces, and the numbers are the jar's.
///
/// `run.sh p6t1 <rpi> 8 1 - 1-8 0` ends connection 2 at `maxIdAfter = 83`; `… 1-8 100` ends it at
/// `maxIdAfter = 90`. The seven ids in between are the failed neck attempt's: `retryConnection
/// Necked` returns `null` at `:218-220` because the second attempt did not route either, and
/// **Java does not roll that attempt back** — the ids stay spent and whatever the attempt
/// inserted stays inserted. Both halves of the driver MATCH on those numbers.
#[test]
fn the_necked_retry_fires_and_spends_item_ids() {
    if !parity::require_java_dir() {
        return;
    }
    let (_, _, ids_no_neck, _, _) = route_prefix(2, 0.0);
    let (_, _, ids_neck, _, _) = route_prefix(2, 100.0);
    assert_eq!(
        vec![63, 83],
        ids_no_neck,
        "the jar's maxGeneratedId after connections 1 and 2 with no neck width"
    );
    assert_eq!(
        vec![63, 90],
        ids_neck,
        "…and with neckWidthUm = 100, where the retry runs and fails"
    );
}

/// **Quirk #208's fix, pinned** (Plan 9 Task 1): the necked retry gets a **fresh** `TimeLimit`,
/// not the one the failed first attempt already spent.
///
/// Java's `retryConnectionNecked` receives `route`'s own `TimeLimit`
/// (`AutorouteConnectionRouter.java:171`) and hands it straight to the second `initAutoroute`
/// (`:209`). `TimeLimit` keeps its construction instant and its limit as two never-reset fields
/// (`datastructures/TimeLimit.java:8-15`), so the retry's budget is the original minus everything
/// the first attempt spent — and the connections the retry exists for are exactly the ones where
/// the first attempt ran long, so the remaining budget is smallest precisely where the retry is
/// wanted.
///
/// The observation point is the engine the call leaves behind — after a retry that fired, that is
/// the **retry's** engine, and [`AutorouteEngine::time_limit`] is the budget it was initialised
/// with. Java's limit for `ripupPassNo = 1` is `(int) min(100000 * 2^0, Integer.MAX_VALUE)` =
/// 100 000 ms (`route:71-74`), and the fix does **not** change it: what separates the two shapes
/// is only **when the clock started**.
///
/// * reused (the Java bug) -> the deadline is `t_call_start + 100 000 ms` plus the microseconds
///   it takes to build an `AutorouteControl`;
/// * fresh (the fix)       -> that **plus the whole duration of the failed first attempt**.
///
/// So the assertion is inverted from what it was before this task: the deadline must sit *at
/// least* most of the connection's own duration past `t_call_start + 100 000 ms`. Measured
/// RED/GREEN on `rpi_splitter`'s connection 2 at `neckWidthUm = 100`: **81 ms of a 96 ms call**.
/// The test only makes the discriminating assertion when the call took more than 20 ms —
/// otherwise the first attempt is too cheap to tell the two shapes apart, and it says so instead
/// of pretending.
///
/// The budget itself is asserted unconditionally, because the fix must not quietly hand the retry
/// a *different* limit: same `100000 * 2^(ripupPassNo - 1)`, new clock.
#[test]
fn the_necked_retry_gets_a_fresh_time_limit() {
    if !parity::require_java_dir() {
        return;
    }
    let (_board, engine, ids, call_start, call_duration) = route_prefix(2, 100.0);
    // The retry really did run — otherwise the engine below is the first attempt's and the
    // assertion would be vacuous.
    assert_eq!(vec![63, 90], ids, "the retry must have fired");

    let engine = engine.expect("initAutoroute always answers an engine");
    let time_limit = engine
        .time_limit()
        .expect("route:76-82 and :204-210 both pass a TimeLimit");
    assert_eq!(
        100_000,
        time_limit.limit_ms(),
        "route:71-74 — 100000 * 2^(ripupPassNo - 1) at pass 1. #208's fix changes the clock, \
         never the budget"
    );

    if call_duration < Duration::from_millis(20) {
        eprintln!(
            "skipped quirk #208's discriminating assertion: the retrying connection took \
             {call_duration:?}, which is too little for a freshly-minted TimeLimit to be \
             distinguishable"
        );
        return;
    }
    let deadline = time_limit
        .deadline()
        .expect("a positive limit has a deadline");
    // How far the deadline sits past the start of the *retrying* connection.
    let offset = deadline.duration_since(call_start);
    let slack = offset - Duration::from_millis(100_000);
    assert!(
        slack > call_duration / 2,
        "#208: the retry's TimeLimit must be minted at `:209`, not inherited from `route:74` — \
         its deadline is only {slack:?} past `call_start + 100 s`, and the failed first attempt \
         alone accounts for most of this connection's {call_duration:?}"
    );
}

/// Step 8 (`:147-153` -> `applyStrictDrcAfterRoute:243-254`) is a no-op with strict DRC off,
/// which is `DefaultSettings.java:110`'s value and therefore every production run's.
///
/// The rejection and the rollback themselves are `crates/fr-router/tests/strict_drc.rs`.
#[test]
fn step_eight_is_a_no_op_when_strict_drc_is_off() {
    if !parity::require_java_dir() {
        return;
    }
    let board = load_rpi();
    let settings = rpi_settings(&board, 0.0);
    assert!(
        !settings.is_strict_drc(),
        "DefaultSettings.java:110 seeds strictDrc = false"
    );
    // With it off, `:84-85` takes no clone and `:245-247` returns `null` before
    // `enforceStrictDrc` is even called — so a routed connection keeps whatever it inserted.
    let (routed, _, ids, _, _) = route_prefix(1, 0.0);
    assert_eq!(vec![63], ids);
    assert!(
        routed.communication.id_gen.max_generated_id().0 == 63,
        "nothing was rolled back"
    );
}

// =================================================================================================
// R1 (register row #293) — the airline-distance ordering of the work list
//
// `getAutorouteItems` (`:345-409`) builds the list in `board.itemList`'s **descending-id** walk
// order (quirk #63) and, at HEAD, returns it that way: commit `933d2980` deleted
// `autorouteItemList.sort(Comparator.comparingDouble(this::calculateItemDistance))`.
// `benchmark/reports/java-regressions-2026-09.md` §"Regression 1" measures what that cost, and
// Plan 9 Task 2 restores the sort inside `autoroute_items_with_handled`.
// =================================================================================================

/// A **user-fixed** trace, which is what makes an item a work-list candidate: `:358` keeps only
/// items that are not `isRoutable()`, and `Trace.isRoutable` (Trace.java:205-209) is
/// `!isUserFixed() && netCount() > 0`.
fn fixed_trace(board: &mut Board, corners: &[Point], net: i32) -> ItemId {
    board
        .insert_trace_without_cleaning(
            Polyline::from_points(corners),
            0,
            30,
            vec![net],
            1,
            FixedState::UserFixed,
        )
        .expect("a two-corner polyline always inserts")
}

/// Three nets, each a user-fixed candidate plus one ordinary trace it is not connected to. The
/// **item ids ascend with insertion**, so `board.itemList`'s descending walk reaches net 3's
/// candidate first and net 1's last — and the three airline distances are arranged to run the
/// **other** way, 2000 / 3000 / 8000 in id order.
///
/// | net | candidate | its midpoint  | target midpoint | `calculateItemDistance` |
/// |-----|-----------|---------------|-----------------|-------------------------|
/// | 1   | `c1`      | (-9000,-8500) | (-9000,-6500)   | 2000                    |
/// | 2   | `c2`      | (0,-8500)     | (0,-5500)       | 3000                    |
/// | 3   | `c3`      | (9000,-8500)  | (9000,-500)     | 8000                    |
///
/// So the pre-fix answer is `[c3, c2, c1]` and the fixed answer is `[c1, c2, c3]` — the two are
/// exact reverses, which is what makes this test fail before the fix rather than merely differ.
#[test]
fn the_work_list_is_sorted_by_airline_distance() {
    let mut board = empty_board(200, AngleRestriction::None);
    add_net(&mut board, "N1", 0);
    add_net(&mut board, "N2", 0);
    add_net(&mut board, "N3", 0);

    let c1 = fixed_trace(&mut board, &[p(-9000, -9000), p(-9000, -8000)], 1);
    insert_trace(&mut board, &[p(-9000, -7000), p(-9000, -6000)], 0, 30, 1);
    let c2 = fixed_trace(&mut board, &[p(0, -9000), p(0, -8000)], 2);
    insert_trace(&mut board, &[p(0, -6000), p(0, -5000)], 0, 30, 2);
    let c3 = fixed_trace(&mut board, &[p(9000, -9000), p(9000, -8000)], 3);
    insert_trace(&mut board, &[p(9000, -1000), p(9000, 0)], 0, 30, 3);

    assert!(c1 < c2 && c2 < c3, "ids ascend with insertion order");

    // The keys, hand-checked against `AutorouteAirlineCalculator.java:204-213`'s midpoint rule.
    for (item, expected) in [(c1, 2000.0), (c2, 3000.0), (c3, 8000.0)] {
        assert_eq!(
            fr_router::pipeline::calculate_item_distance(&board, item),
            expected
        );
    }

    let settings = RouterSettings::new();
    let router = BatchAutorouter::for_routing_job(&board, &settings, RouterBudget::disabled());
    assert_eq!(
        router.autoroute_items(&board),
        // fixed: T9 (#213) — the work list carries `(item, qualifying net)` pairs; every item
        // here is single-net, so the sort's subject is unchanged.
        vec![(c1, 1), (c2, 2), (c3, 3)],
        "R1 (#293): the work list is ascending by calculateItemDistance. Before the fix this \
         answered [c3, c2, c1] — the descending-id walk (quirk #63), which is exactly the \
         reverse here."
    );
}

/// The stability claim, asserted so a later sort change cannot silently reorder ties.
///
/// Java's `List.sort` is a TimSort and keeps equal keys in insertion order, so the deleted line
/// left ties in the descending-id walk order `getAutorouteItems` built. The port's `sort_by` is
/// stable for the same reason and must answer the same thing: the restored sort is a
/// **refinement** of today's order, not a second, independent reordering.
///
/// Three nets whose candidate-to-target distance is **exactly 3000** on each, laid out so the
/// arithmetic is identical rather than merely close — the assertion is on `==`, and a fixture
/// whose keys differed in the last bit would be asserting the opposite of what it claims.
#[test]
fn equal_airline_distances_keep_the_descending_id_tie_order() {
    let mut board = empty_board(200, AngleRestriction::None);
    add_net(&mut board, "N1", 0);
    add_net(&mut board, "N2", 0);
    add_net(&mut board, "N3", 0);

    let c1 = fixed_trace(&mut board, &[p(-9000, -9000), p(-9000, -8000)], 1);
    insert_trace(&mut board, &[p(-9000, -6000), p(-9000, -5000)], 0, 30, 1);
    let c2 = fixed_trace(&mut board, &[p(0, -9000), p(0, -8000)], 2);
    insert_trace(&mut board, &[p(0, -6000), p(0, -5000)], 0, 30, 2);
    let c3 = fixed_trace(&mut board, &[p(9000, -9000), p(9000, -8000)], 3);
    insert_trace(&mut board, &[p(9000, -6000), p(9000, -5000)], 0, 30, 3);

    let keys: Vec<f64> = [c1, c2, c3]
        .into_iter()
        .map(|id| fr_router::pipeline::calculate_item_distance(&board, id))
        .collect();
    assert_eq!(
        keys,
        vec![3000.0, 3000.0, 3000.0],
        "the three keys must be bit-identical for this to be a tie test"
    );

    let settings = RouterSettings::new();
    let router = BatchAutorouter::for_routing_job(&board, &settings, RouterBudget::disabled());
    assert_eq!(
        router.autoroute_items(&board),
        vec![(c3, 3), (c2, 2), (c1, 1)],
        "ties keep the descending-id walk order (quirk #63) — the order `getAutorouteItems` \
         built and the order Java's stable `List.sort` would have preserved"
    );
}
