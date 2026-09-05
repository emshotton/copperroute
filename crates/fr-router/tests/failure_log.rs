use fr_board::prelude::*;
use fr_geometry::{IntBox, IntPoint, Point, Polyline};
use fr_router::pipeline::{
    AutoroutePassRunner, BatchAutorouter, NoopProgressSink, RouterBudget, RouterStop,
    RoutingFailureLog,
};
use fr_settings::sources::DefaultSettings;
use fr_settings::{HostEnvironment, RouterSettings, SettingsSource};

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

fn empty_board() -> Board {
    let clearance_matrix = ClearanceMatrix::get_default_instance(&layers(), 200);
    let mut rules = BoardRules::new(layers(), clearance_matrix);
    rules.trace_angle_restriction = AngleRestriction::None;
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

fn add_net(board: &mut Board, name: &str) {
    let default_class = board.rules.get_default_net_class();
    board.rules.nets.add(name, 0, false, default_class);
}

fn trace(board: &mut Board, corners: &[Point], layer: usize, fixed: FixedState) -> ItemId {
    board
        .insert_trace_without_cleaning(Polyline::from_points(corners), layer, 30, vec![1], 1, fixed)
        .expect("a two-corner polyline always inserts")
}

fn settings_for(board: &Board) -> RouterSettings {
    let mut settings = DefaultSettings::new(&HostEnvironment::detect())
        .get_settings()
        .expect("DefaultSettings always answers a table")
        .clone();
    settings.set_layer_count(board.get_layer_count());
    settings.apply_board_specific_optimizations(board);
    settings
}

/// A candidate that never routes: fixed on one layer, its only same-net partner unreachable on
/// another, so every pass records a fresh failure on the same item.
fn never_routes_board() -> (Board, ItemId) {
    let mut board = empty_board();
    add_net(&mut board, "N1");
    let candidate = trace(
        &mut board,
        &[p(-3000, -3000), p(-3000, -1000)],
        0,
        FixedState::UserFixed,
    );
    trace(
        &mut board,
        &[p(3000, 3000), p(3000, 1000)],
        1,
        FixedState::UserFixed,
    );
    (board, candidate)
}

fn run_pass(
    board: &mut Board,
    router: &mut BatchAutorouter<'_>,
    log: &mut RoutingFailureLog,
    pass_no: i32,
) {
    let stop = RouterStop::new();
    let mut sink = NoopProgressSink;
    AutoroutePassRunner::run_single_thread(board, router, log, pass_no, &stop, &mut sink)
        .expect("the pass answers Ok");
}

/// #235 — `RoutingFailureLog::FAILURE_THRESHOLD`'s give-up policy, wired into the item loop only
/// when `failure_give_up_threshold` is set: an item that has already failed that many times is
/// skipped rather than attempted again.
#[test]
fn an_item_that_fails_fifty_times_is_skipped_when_enabled() {
    let (mut board, candidate) = never_routes_board();
    let mut settings = settings_for(&board);
    settings.set_failure_give_up_threshold(Some(RoutingFailureLog::FAILURE_THRESHOLD));
    let mut router = BatchAutorouter::for_routing_job(&board, &settings, RouterBudget::disabled());
    let mut log = RoutingFailureLog::new();

    for pass in 1..=RoutingFailureLog::FAILURE_THRESHOLD {
        run_pass(&mut board, &mut router, &mut log, pass);
    }
    assert_eq!(
        log.failure_count(candidate),
        RoutingFailureLog::FAILURE_THRESHOLD,
        "fifty passes, fifty recorded failures"
    );

    run_pass(
        &mut board,
        &mut router,
        &mut log,
        RoutingFailureLog::FAILURE_THRESHOLD + 1,
    );
    assert_eq!(
        log.failure_count(candidate),
        RoutingFailureLog::FAILURE_THRESHOLD,
        "the 51st pass skips the item instead of attempting and failing it again"
    );
}

/// #235 — with no setting, the give-up policy never runs and an item is retried on the 51st pass
/// exactly as on the first, matching the jar's inert threshold.
#[test]
fn the_give_up_policy_is_disabled_by_default() {
    let (mut board, candidate) = never_routes_board();
    let settings = settings_for(&board);
    assert_eq!(
        settings.get_failure_give_up_threshold(),
        None,
        "disabled by default"
    );
    let mut router = BatchAutorouter::for_routing_job(&board, &settings, RouterBudget::disabled());
    let mut log = RoutingFailureLog::new();

    for pass in 1..=(RoutingFailureLog::FAILURE_THRESHOLD + 1) {
        run_pass(&mut board, &mut router, &mut log, pass);
    }
    assert_eq!(
        log.failure_count(candidate),
        RoutingFailureLog::FAILURE_THRESHOLD + 1,
        "the 51st pass still records a new failure"
    );
}
