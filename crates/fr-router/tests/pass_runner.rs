use std::collections::BTreeSet;

use fr_board::items::Item;
use fr_board::prelude::*;
use fr_geometry::{FloatPoint, IntBox, IntOctagon, IntPoint, Point, Polyline, Shape, TileShape};
use fr_router::pipeline::{
    AutoroutePassRunner, BatchAutorouter, NoopProgressSink, RouterBudget, RouterStop,
    RoutingFailureLog, StopRequestState,
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

fn add_net(board: &mut Board, name: &str, contains_plane: bool) {
    let default_class = board.rules.get_default_net_class();
    board.rules.nets.add(name, 0, contains_plane, default_class);
}

fn trace(
    board: &mut Board,
    corners: &[Point],
    layer: usize,
    nets: Vec<i32>,
    fixed: FixedState,
) -> ItemId {
    board
        .insert_trace_without_cleaning(Polyline::from_points(corners), layer, 30, nets, 1, fixed)
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

fn run_one_pass(
    board: &mut Board,
    router: &mut BatchAutorouter<'_>,
    stop: &RouterStop,
) -> Result<bool, fr_router::RouterError> {
    let mut failure_log = RoutingFailureLog::new();
    let mut sink = NoopProgressSink;
    AutoroutePassRunner::run_single_thread(board, router, &mut failure_log, 1, stop, &mut sink)
}


#[test]
fn a_two_net_item_is_routed_once_per_qualifying_net() {
    let mut board = empty_board();
    add_net(&mut board, "N1", false);
    add_net(&mut board, "N2", false);

    let a = trace(
        &mut board,
        &[p(-3000, -3000), p(-3000, -1000)],
        0,
        vec![1, 2],
        FixedState::UserFixed,
    );
    trace(
        &mut board,
        &[p(3000, 3000), p(3000, 1000)],
        0,
        vec![1],
        FixedState::Unfixed,
    );
    trace(
        &mut board,
        &[p(5000, 3000), p(5000, 1000)],
        0,
        vec![2],
        FixedState::Unfixed,
    );

    let settings = settings_for(&board);
    let mut router = BatchAutorouter::for_routing_job(&board, &settings, RouterBudget::disabled());

    let work_list = router.autoroute_items(&board);
    assert_eq!(
        work_list,
        vec![(a, 1), (a, 2)],
        "BatchAutorouter.java:390 appends once per qualifying net; fixed: T9 (#213) — the entry \
         carries the net that qualified it"
    );

    let stop = RouterStop::new();
    run_one_pass(&mut board, &mut router, &stop).expect("the pass answers Ok");
    assert_eq!(
        router.total_items_routed, 2,
        "fixed: T9 (#213) — AutoroutePassRunner.java:222 counts one visit per (item, qualifying \
         net) pair. Java's :207 multiplied that by the item's whole netCount and this literal was \
         4; a smaller number than 2 means the pass ended early — most likely at :203's stop \
         check, or on a panic the :331 boundary swallowed"
    );
}

#[test]
fn the_inner_index_is_the_qualifying_net() {
    let mut board = empty_board();
    add_net(&mut board, "N1", false);
    add_net(&mut board, "N2", false);

    let a = trace(
        &mut board,
        &[p(-3000, -3000), p(-3000, -1000)],
        0,
        vec![1, 2],
        FixedState::UserFixed,
    );
    trace(
        &mut board,
        &[p(5000, 3000), p(5000, 1000)],
        0,
        vec![2],
        FixedState::Unfixed,
    );

    let settings = settings_for(&board);
    let mut router = BatchAutorouter::for_routing_job(&board, &settings, RouterBudget::disabled());

    assert_eq!(
        router.autoroute_items(&board),
        vec![(a, 2)],
        "only net 2 satisfies :375, so :390 runs once — and the entry says which net it was"
    );

    let stop = RouterStop::new();
    run_one_pass(&mut board, &mut router, &stop).expect("the pass answers Ok");
    assert_eq!(
        router.total_items_routed, 1,
        "fixed: T9 (#213) — one visit, on net 2. Java's :207 looped over both net indices of the \
         single appearance and attempted net 1, which nothing had enqueued; the literal was 2"
    );
}

#[test]
fn a_plane_net_with_a_conduction_area_is_skipped() {
    let mut board = empty_board();
    add_net(&mut board, "PLANE", true);

    let pour = board.insert_conduction_area(
        Shape::Tile(TileShape::Box(IntBox::new(
            IntPoint { x: -4000, y: -4000 },
            IntPoint { x: 0, y: 0 },
        )))
        .into(),
        0,
        vec![1],
        1,
        false,
        FixedState::UserFixed,
    );

    let a = trace(
        &mut board,
        &[p(-3000, -3000), p(-3000, -1000)],
        0,
        vec![1],
        FixedState::UserFixed,
    );
    let b = trace(
        &mut board,
        &[p(6000, 6000), p(6000, 8000)],
        1,
        vec![1],
        FixedState::UserFixed,
    );

    assert!(
        board.connected_set(a, 1, false).contains(&pour),
        "the fixture must actually connect A to the pour, or :385's stream tests nothing"
    );
    assert!(
        !board.connected_set(b, 1, false).contains(&pour),
        "and B must not"
    );

    let settings = settings_for(&board);
    let router = BatchAutorouter::for_routing_job(&board, &settings, RouterBudget::disabled());
    let work_list = router.autoroute_items(&board);

    assert!(
        !work_list.iter().any(|(id, _)| *id == a),
        "BatchAutorouter.java:383-389 skips an item already connected to the pour"
    );
    assert!(
        work_list.iter().any(|(id, _)| *id == b),
        "…and enqueues one that is not, so it can be routed to the pour this pass"
    );
}

#[test]
fn an_item_with_ignored_nets_is_skipped() {
    let build = |ignored: bool| {
        let mut board = empty_board();
        let class = board
            .rules
            .net_classes
            .append("ignored", &layers(), ignored);
        board.rules.nets.add("N1", 0, false, class);

        let a = trace(
            &mut board,
            &[p(-3000, -3000), p(-3000, -1000)],
            0,
            vec![1],
            FixedState::UserFixed,
        );
        trace(
            &mut board,
            &[p(3000, 3000), p(3000, 1000)],
            0,
            vec![1],
            FixedState::Unfixed,
        );
        (board, a)
    };

    let (board, a) = build(false);
    let settings = settings_for(&board);
    let router = BatchAutorouter::for_routing_job(&board, &settings, RouterBudget::disabled());
    assert_eq!(
        router.autoroute_items(&board),
        vec![(a, 1)],
        "with the flag clear the item is a normal candidate"
    );

    let (board, a) = build(true);
    let settings = settings_for(&board);
    let router = BatchAutorouter::for_routing_job(&board, &settings, RouterBudget::disabled());
    assert!(
        !router
            .autoroute_items(&board)
            .iter()
            .any(|(id, _)| *id == a),
        "BatchAutorouter.java:375 — hasIgnoredNets() drops the item"
    );
}


#[test]
fn an_empty_item_list_returns_false_without_touching_the_board() {
    let mut board = empty_board();
    add_net(&mut board, "N1", false);
    trace(
        &mut board,
        &[p(-3000, -3000), p(-3000, -1000)],
        0,
        vec![1],
        FixedState::Unfixed,
    );

    let settings = settings_for(&board);
    let mut router = BatchAutorouter::for_routing_job(&board, &settings, RouterBudget::disabled());
    assert!(router.autoroute_items(&board).is_empty());

    let before = board.structural_hash();
    let stop = RouterStop::new();
    let answer = run_one_pass(&mut board, &mut router, &stop).expect("the pass answers Ok");

    assert!(!answer, "AutoroutePassRunner.java:165 returns false");
    assert_eq!(
        board.structural_hash(),
        before,
        ":163-166 returns before startMarkingChangedArea, removeTails and everything else"
    );
    assert_eq!(
        router.total_items_routed, 0,
        ":222 is never reached, so the counter does not move"
    );
    assert!(
        router.progress_statistics.is_none(),
        ":170's BoardStatistics is below the early return"
    );
}

#[test]
fn max_items_requests_stop_auto_router() {
    let mut board = empty_board();
    add_net(&mut board, "N1", false);
    add_net(&mut board, "N2", false);
    trace(
        &mut board,
        &[p(-3000, -3000), p(-3000, -1000)],
        0,
        vec![1, 2],
        FixedState::UserFixed,
    );
    trace(
        &mut board,
        &[p(3000, 3000), p(3000, 1000)],
        0,
        vec![1],
        FixedState::Unfixed,
    );
    trace(
        &mut board,
        &[p(5000, 3000), p(5000, 1000)],
        0,
        vec![2],
        FixedState::Unfixed,
    );

    let mut settings = settings_for(&board);
    settings.max_items = Some(1);
    let mut router = BatchAutorouter::for_routing_job(&board, &settings, RouterBudget::disabled());

    let stop = RouterStop::new();
    assert_eq!(stop.state(), StopRequestState::None, "before the pass");
    run_one_pass(&mut board, &mut router, &stop).expect("the pass answers Ok");

    assert_eq!(
        router.total_items_routed, 1,
        ":213-215 tests `>= maxItems` before :222 increments, so exactly one item is routed"
    );
    assert_eq!(
        stop.state(),
        StopRequestState::AutoRouterOnly,
        "fixed: T9 (#202) — the site calls requestStopAutoRouter(), where Java's `:219` calls \
         requestStop()"
    );
    assert!(
        stop.is_stop_auto_router_requested(),
        "the pass loop and the item loop both still stop: they read `!= NONE`"
    );
    assert!(
        !stop.is_stop_requested(),
        "…and RoutingPipeline.java:117 reads `ALL`, so the optimizer stage is no longer skipped"
    );
}

#[test]
fn an_auto_router_only_stop_ends_the_item_loop() {
    let mut board = empty_board();
    add_net(&mut board, "N1", false);
    trace(
        &mut board,
        &[p(-3000, -3000), p(-3000, -1000)],
        0,
        vec![1],
        FixedState::UserFixed,
    );
    trace(
        &mut board,
        &[p(3000, 3000), p(3000, 1000)],
        0,
        vec![1],
        FixedState::Unfixed,
    );

    let settings = settings_for(&board);
    let mut router = BatchAutorouter::for_routing_job(&board, &settings, RouterBudget::disabled());
    assert_eq!(
        router.autoroute_items(&board).len(),
        1,
        "there is work to do"
    );

    let stop = RouterStop::new();
    stop.request_stop_auto_router();
    assert_eq!(stop.state(), StopRequestState::AutoRouterOnly);

    let answer = run_one_pass(&mut board, &mut router, &stop).expect("the pass answers Ok");
    assert!(!answer, ":330 — routed and notRouted are both zero");
    assert_eq!(router.total_items_routed, 0, ":203-205 breaks before :222");
}

#[test]
fn a_panicking_item_ends_the_pass_and_returns_false() {
    let mut board = empty_board();
    add_net(&mut board, "N1", false);
    let undeclared = trace(
        &mut board,
        &[p(-3000, -3000), p(-3000, -1000)],
        0,
        vec![2],
        FixedState::UserFixed,
    );
    trace(
        &mut board,
        &[p(3000, 3000), p(3000, 1000)],
        0,
        vec![2],
        FixedState::UserFixed,
    );
    assert!(
        board.rules.nets.get(2).is_none(),
        "the fixture rests on net 2 being undeclared — Nets.get answers null there"
    );

    let settings = settings_for(&board);
    let mut router = BatchAutorouter::for_routing_job(&board, &settings, RouterBudget::disabled());
    let stop = RouterStop::new();

    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let answer = run_one_pass(&mut board, &mut router, &stop);
    std::panic::set_hook(hook);

    assert!(
        !answer.expect("the boundary degrades rather than propagating"),
        "AutoroutePassRunner.java:331-335 catches and returns false"
    );

    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let direct = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        board.has_ignored_nets(undeclared)
    }));
    std::panic::set_hook(hook);
    assert!(
        direct.is_err(),
        "Item.java:1244 has no null guard, so the undeclared net is what panics"
    );
}

#[test]
fn the_pass_ends_with_remove_tails() {
    let mut board = empty_board();
    add_net(&mut board, "N1", false);
    let candidate = trace(
        &mut board,
        &[p(-3000, -3000), p(-3000, -1000)],
        0,
        vec![1],
        FixedState::UserFixed,
    );
    let tail = trace(
        &mut board,
        &[p(3000, 3000), p(3000, 1000)],
        0,
        vec![1],
        FixedState::Unfixed,
    );

    let settings = settings_for(&board);
    let mut router = BatchAutorouter::for_routing_job(&board, &settings, RouterBudget::disabled());
    assert_eq!(router.autoroute_items(&board), vec![(candidate, 1)]);
    assert!(board.get_item(tail).is_some(), "before the pass");

    let stop = RouterStop::new();
    run_one_pass(&mut board, &mut router, &stop).expect("the pass answers Ok");

    assert!(
        board.get_item(tail).is_none(),
        "AutoroutePassRunner.java:298-302 runs removeTails, and an unanchored trace is all tail \
         (RoutingBoard.java:1204)"
    );
}


#[test]
fn the_failure_log_records_what_java_records() {
    let mut board = empty_board();
    add_net(&mut board, "N1", false);
    add_net(&mut board, "N2", false);
    let item = trace(
        &mut board,
        &[p(-3000, -3000), p(-3000, -1000)],
        0,
        vec![2, 1],
        FixedState::UserFixed,
    );

    let mut log = RoutingFailureLog::new();
    assert!(log.is_empty(), "RoutingFailureLog.java:22-24");
    assert_eq!(log.failure_count(item), 0, ":96-100 — an absent key is 0");

    log.record_failure(
        &board,
        item,
        7,
        fr_router::AutorouteAttemptState::Failed,
        Some("no connection was found"),
    );
    let info = log
        .entry(item)
        .expect(":39-47 inserts on the first failure");
    assert_eq!(info.item, item);
    assert_eq!(
        info.net_number, 2,
        ":120 — getNetNumber(0), the item's first net"
    );
    assert_eq!(info.failure_count, 1, ":136");
    assert_eq!(info.last_attempt_pass, 7, ":137, widened to a long");
    assert_eq!(
        info.last_failure_state,
        Some(fr_router::AutorouteAttemptState::Failed),
        ":138"
    );
    assert_eq!(info.last_failure_reason, "no connection was found", ":139");
    assert_eq!(log.failure_count(item), 1, ":99-100");

    log.record_failure(
        &board,
        item,
        9,
        fr_router::AutorouteAttemptState::InsertError,
        None,
    );
    let info = log.entry(item).expect("still there");
    assert_eq!(info.failure_count, 2, ":136 increments");
    assert_eq!(info.last_attempt_pass, 9);
    assert_eq!(
        info.last_failure_reason, "",
        ":139 — `reason != null ? reason : \"\"`, so a null becomes the empty string"
    );
    assert_eq!(log.len(), 1, "one key, two failures");
    assert_eq!(
        RoutingFailureLog::FAILURE_THRESHOLD,
        50,
        "RoutingFailureLog.java:16"
    );
}

#[test]
fn the_pass_writes_the_failure_log() {
    let mut board = empty_board();
    add_net(&mut board, "N1", false);
    let candidate = trace(
        &mut board,
        &[p(-3000, -3000), p(-3000, -1000)],
        0,
        vec![1],
        FixedState::UserFixed,
    );
    trace(
        &mut board,
        &[p(3000, 3000), p(3000, 1000)],
        1,
        vec![1],
        FixedState::Unfixed,
    );

    let settings = settings_for(&board);
    let mut router = BatchAutorouter::for_routing_job(&board, &settings, RouterBudget::disabled());
    let stop = RouterStop::new();
    let mut log = RoutingFailureLog::new();
    let mut sink = NoopProgressSink;
    let answer = AutoroutePassRunner::run_single_thread(
        &mut board,
        &mut router,
        &mut log,
        3,
        &stop,
        &mut sink,
    )
    .expect("the pass answers Ok");

    assert!(
        answer,
        ":330 — a failed item still counts as progress (`notRouted > 0`)"
    );
    let info = log
        .entry(candidate)
        .expect("AutoroutePassRunner.java:269-271 records the failure");
    assert_eq!(info.last_attempt_pass, 3, ":270 passes passNo through");
    assert!(info.failure_count >= 1);
}

#[test]
fn only_a_non_routable_connectable_item_is_a_candidate() {
    let mut board = empty_board();
    add_net(&mut board, "N1", false);
    let fixed = trace(
        &mut board,
        &[p(-3000, -3000), p(-3000, -1000)],
        0,
        vec![1],
        FixedState::UserFixed,
    );
    let unfixed = trace(
        &mut board,
        &[p(3000, 3000), p(3000, 1000)],
        0,
        vec![1],
        FixedState::Unfixed,
    );

    let get = |id: ItemId| board.get_item(id).expect("on the board");
    assert!(!get(fixed).is_routable(), "Trace.java:205-209");
    assert!(get(unfixed).is_routable());
    assert!(get(fixed).as_connectable().is_some(), "Trace.java:28");
    assert!(matches!(get(fixed), Item::Trace(_)));
}


#[test]
fn calculate_airline_takes_the_closest_drill_item_pair() {
    let mut board = via_board();
    add_net(&mut board, "N1", false);

    let near_via = board
        .insert_via(
            PadstackId(1),
            Point::new(0, 0),
            vec![1],
            1,
            FixedState::Unfixed,
            true,
        )
        .expect("the padstack spans both layers");
    let far_via = board
        .insert_via(
            PadstackId(1),
            Point::new(4000, 0),
            vec![1],
            1,
            FixedState::Unfixed,
            true,
        )
        .expect("the padstack spans both layers");
    let farther_via = board
        .insert_via(
            PadstackId(1),
            Point::new(9000, 0),
            vec![1],
            1,
            FixedState::Unfixed,
            true,
        )
        .expect("the padstack spans both layers");
    let start_trace = trace(
        &mut board,
        &[p(1000, 5000), p(1100, 5000)],
        0,
        vec![1],
        FixedState::Unfixed,
    );
    let dest_trace = trace(
        &mut board,
        &[p(1150, 5000), p(1200, 5000)],
        0,
        vec![1],
        FixedState::Unfixed,
    );

    let start: BTreeSet<ItemId> = [near_via, start_trace].into_iter().collect();
    let dest: BTreeSet<ItemId> = [far_via, farther_via, dest_trace].into_iter().collect();

    let airline = fr_router::pipeline::calculate_airline(&board, &start, &dest)
        .expect(":39 — both sets hold a drill item, so neither endpoint is null");
    assert_eq!(
        airline.a,
        FloatPoint { x: 0.0, y: 0.0 },
        ":34 — the start via"
    );
    assert_eq!(
        airline.b,
        FloatPoint { x: 4000.0, y: 0.0 },
        ":35 — the nearer of the two destination vias, not the farther one"
    );

    let reversed = fr_router::pipeline::calculate_airline(&board, &dest, &start)
        .expect("still a drill item on each side");
    assert_eq!(reversed.a, airline.b);
    assert_eq!(reversed.b, airline.a);
}

#[test]
fn calculate_airline_answers_none_when_either_side_has_no_drill_item() {
    let mut board = via_board();
    add_net(&mut board, "N1", false);
    let via = board
        .insert_via(
            PadstackId(1),
            Point::new(0, 0),
            vec![1],
            1,
            FixedState::Unfixed,
            true,
        )
        .expect("the padstack spans both layers");
    let only_trace = trace(
        &mut board,
        &[p(1000, 5000), p(1100, 5000)],
        0,
        vec![1],
        FixedState::Unfixed,
    );

    let vias: BTreeSet<ItemId> = [via].into_iter().collect();
    let traces: BTreeSet<ItemId> = [only_trace].into_iter().collect();
    let empty: BTreeSet<ItemId> = BTreeSet::new();

    assert!(
        fr_router::pipeline::calculate_airline(&board, &vias, &traces).is_none(),
        ":26-29 skips every destination, so `toCorner` stays null"
    );
    assert!(
        fr_router::pipeline::calculate_airline(&board, &traces, &vias).is_none(),
        ":20-23 skips every source, so `fromCorner` stays null"
    );
    assert!(fr_router::pipeline::calculate_airline(&board, &empty, &vias).is_none());
    assert!(fr_router::pipeline::calculate_airline(&board, &vias, &empty).is_none());
    assert!(fr_router::pipeline::calculate_airline(&board, &empty, &empty).is_none());
}
