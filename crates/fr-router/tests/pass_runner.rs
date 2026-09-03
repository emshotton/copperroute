//! Plan 7 Task 9 — `BatchAutorouter::autoroute_items` (`BatchAutorouter.java:345-409`) and
//! `AutoroutePassRunner::run_single_thread` (`AutoroutePassRunner.java:151-336`).
//!
//! # Where the numbers come from
//!
//! The end-to-end evidence is `scripts/differential/run.sh p7t1 <dsn> <passNo>` and
//! `run.sh p7t2 <dsn> <passNo> <maxItems|all>`, both against the HEAD jar, over the six corpus
//! stems at passes 1-3. The tests below isolate the branches a whole board exercises all at once
//! and could not attribute — above all plan-7 ruling 10's multi-net duplication, which on a real
//! board is indistinguishable from "the item was simply enqueued twice".
//!
//! # `BatchAutorouterDebugTest` (ruling 14) is **not** ported, and the name is why
//!
//! The plan budgets 521 lines for porting
//! `src/test/java/app/freerouting/autoroute/BatchAutorouterDebugTest.java` into this file. Read
//! out of HEAD, that file tests **nothing in this task's scope, and nothing in `BatchAutorouter`
//! at all**:
//!
//! * `grep -n BatchAutorouter` over its 521 lines answers exactly **one** hit — `:16`, its own
//!   class declaration. There is no `getAutorouteItems`, no `runSingleThread`, no `RoutingBoard`
//!   and no board of any kind in it.
//! * Its 21 `@Test` methods are all about `app.freerouting.debug.DebugControl` — a singleton
//!   pause / resume / single-step / fast-forward debugger with `AtomicBoolean`s, listener lists
//!   and `Thread.sleep`, driven through the global `Freerouting.globalSettings.debugSettings`
//!   (83 references to those two names). It is the interactive debugger's controller, which the
//!   port drops under "No GUI, no observers" and "no threads" (plan §Tech Stack).
//! * The one piece of it that has a headless counterpart — `DebugSettings.isNetPermitted` — is
//!   already ported and tested in `fr-settings`
//!   (`crates/fr-settings/tests/struct_shape.rs`'s `debug_settings_is_net_permitted_matches_java`
//!   and `debug_settings_default_matches_javas_field_initializers`).
//!
//! So there is no "port the assertions that bear on `getAutorouteItems`/`runSingleThread` here"
//! subset to take: the intersection is empty. The file is rostered rather than ported, and the
//! roster line lives in `crates/fr-router/src/lib.rs` beside the other `autoroute/**` entries.

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

/// [`empty_board`] plus one through-via padstack, so `insert_via` has something to place — the
/// `batch_autorouter.rs` fixture, which the airline tests need and the loop tests do not.
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

/// A trace with the given nets and fixed state.
///
/// **`FixedState::UserFixed` is what makes an item enter the work list.**
/// `getAutorouteItems:358` keeps only items that are *not* `isRoutable()`, and
/// `Trace.isRoutable` (Trace.java:205-209) is `!isUserFixed() && netCount() > 0` — so an ordinary
/// trace is skipped and a user-fixed one is a candidate. It is also the only way to build a
/// **multi-net** candidate without a component library: `Pin` needs a package and a placement,
/// while `insert_trace_without_cleaning` takes the net list directly.
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

/// One pass through `AutoroutePassRunner::run_single_thread`, with the two pieces of state Java
/// hangs off `router.thread` and `board.failureLog`.
fn run_one_pass(
    board: &mut Board,
    router: &mut BatchAutorouter<'_>,
    stop: &RouterStop,
) -> Result<bool, fr_router::RouterError> {
    let mut failure_log = RoutingFailureLog::new();
    let mut sink = NoopProgressSink;
    AutoroutePassRunner::run_single_thread(board, router, &mut failure_log, 1, stop, &mut sink)
}

// =================================================================================================
// getAutorouteItems — the work list
// =================================================================================================

/// Plan-7 ruling 10's pin: `:390`'s `autorouteItemList.add(currentItem)` is **inside** the per-net
/// loop, so a two-net item that qualifies on both nets is appended **twice** — and
/// `AutoroutePassRunner:202, :207` then walks *every* net index of *each* appearance, so the item
/// is routed **four** times in one pass.
///
/// The board is the smallest one that isolates it. `A` is a user-fixed two-net trace, so it is
/// `Connectable` and not `isRoutable()`; `B` and `C` are ordinary traces on nets 1 and 2, so they
/// are `isRoutable()` and `:358` skips them as *candidates* while `connectableItemCount` still
/// counts them — which is what makes `:375`'s `connectedSet.size() < netItemCount` true on both
/// of `A`'s nets.
///
/// `p7t1` shows the same shape on the corpus: `router-dac2020-bm01` at pass 1 prints 591 lines
/// for a board whose `getAutorouteItems` returns far fewer distinct items than entries.
#[test]
fn a_two_net_item_is_routed_four_times() {
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

    // `:390` — two entries for one item.
    let work_list = router.autoroute_items(&board);
    assert_eq!(
        work_list,
        vec![a, a],
        "BatchAutorouter.java:390 appends once per qualifying net index"
    );

    // `AutoroutePassRunner:202, :207` — two appearances x two net indices.
    let stop = RouterStop::new();
    run_one_pass(&mut board, &mut router, &stop).expect("the pass answers Ok");
    assert_eq!(
        router.total_items_routed, 4,
        "AutoroutePassRunner.java:222 counts one visit per (appearance, net index); a smaller \
         number here means the pass ended early — most likely at :203/:208's stop check, or on a \
         panic the :331 boundary swallowed"
    );
}

/// The second half of the same bug: `:207`'s `i` is a **fresh** `0..netCount()` walk, not the net
/// index that qualified at `:375`. Here `A` qualifies on net 2 only — net 1 has no second
/// connectable item, so `connectedSet.size() < netItemCount` is `1 < 1`, false — and the work
/// list therefore holds **one** entry. The pass still routes `A` **twice**, once for each of its
/// net indices, i.e. it attempts net 1 although nothing enqueued net 1.
#[test]
fn the_inner_index_is_a_net_index_not_the_qualifying_one() {
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
    // Only net 2 gets a second item, so only net 2 qualifies.
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
        vec![a],
        "only net 2 satisfies :375, so :390 runs once"
    );

    let stop = RouterStop::new();
    run_one_pass(&mut board, &mut router, &stop).expect("the pass answers Ok");
    assert_eq!(
        router.total_items_routed, 2,
        "AutoroutePassRunner.java:207 loops over both net indices of the single appearance, so \
         net 1 is attempted although net 2 is what qualified"
    );
}

/// `:383-389` — "for plane nets: skip items whose connected set already contains a
/// `ConductionArea` (copper pour)". The two halves are asserted on one board: `A` shares its
/// layer-0 corner with the pour and is dropped, `B` does not touch it and is kept, so the same
/// plane net produces both answers and a port that ignored the net's `containsPlane` flag would
/// fail on `A` while a port that dropped every plane-net item would fail on `B`.
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

    // `A` ends inside the pour, so `getConnectedSet` reaches it through `normalContacts`.
    let a = trace(
        &mut board,
        &[p(-3000, -3000), p(-3000, -1000)],
        0,
        vec![1],
        FixedState::UserFixed,
    );
    // `B` is on the same net but far away and on the other layer, so its connected set is itself.
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
        !work_list.contains(&a),
        "BatchAutorouter.java:383-389 skips an item already connected to the pour"
    );
    assert!(
        work_list.contains(&b),
        "…and enqueues one that is not, so it can be routed to the pour this pass"
    );
}

/// `:375`'s second conjunct — `!currentItem.hasIgnoredNets()`. The net's class carries
/// `is_ignored_by_autorouter`, and the item drops out of the work list entirely.
///
/// The same board with the flag cleared is asserted first, so the test cannot pass because the
/// item was missing for some other reason.
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
        vec![a],
        "with the flag clear the item is a normal candidate"
    );

    let (board, a) = build(true);
    let settings = settings_for(&board);
    let router = BatchAutorouter::for_routing_job(&board, &settings, RouterBudget::disabled());
    assert!(
        !router.autoroute_items(&board).contains(&a),
        "BatchAutorouter.java:375 — hasIgnoredNets() drops the item"
    );
}

// =================================================================================================
// runSingleThread
// =================================================================================================

/// `:163-166` — an empty work list returns `false` immediately, before the `BoardStatistics` at
/// `:170` and before the `DesignRulesChecker` at `:188`. The board is untouched, which
/// `structural_hash` equality asserts.
///
/// The board has one ordinary trace, so it is not empty; it is the *work list* that is, because
/// `:358` skips a routable item.
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

/// `:212-221` — `maxItems` reached. Java calls `thread.requestStop()`, which is
/// `StoppableThread.requestStop` (`StoppableThread.java:20-23`) and sets **`ALL`**, not
/// `AUTO_ROUTER_ONLY`; **quirk #202**, because `ALL` is what `RoutingPipeline.java:117` gates the
/// optimizer stage on, so a run that reached `--max-items` silently lost the optimizer as well as
/// the router while the sibling `--max-passes` limit did not.
///
/// **fixed: T9 (#202)** — the site calls `request_stop_auto_router()`, so both limits mean "stop
/// routing" and the optimizer stage still runs. This test is the pin for the fixed state: the
/// flag reaches `AUTO_ROUTER_ONLY` and **not** `ALL`, which is the whole of the difference.
/// `crates/fr-router/tests/stop_and_progress.rs`'s `max_items_optimises_like_max_passes` measures
/// the consequence end to end, and `max_items_stops_all_and_max_passes_stops_the_router_only` in
/// the same file keeps the **jar's** answer on record.
///
/// The guard is `totalItemsRouted >= maxItems` **before** `:222`'s increment, so `maxItems = 1`
/// routes exactly one item and stops on the second — which is the off-by-one the assertion
/// records rather than smooths over.
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

    // Four visits without the limit — `a_two_net_item_is_routed_four_times` is that board.
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

/// `:203-205` and `:208-210` — a stop requested **before** the pass ends the item loop on its
/// first turn, so nothing is routed and `:330` answers `false` (both counters are still zero).
///
/// The distinction that matters is which predicate: `:203` and `:208` read
/// `isStopAutoRouterRequested()`, which is `!= NONE`, so an `AUTO_ROUTER_ONLY` stop is enough.
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

/// The recovery boundary `runSingleThread` really has — `:156`'s `try` and `:331-335`'s
/// `catch (Exception e)`, which the plan's scan ruling 9 struck on the strength of a `sed` window
/// that stops one line short of the `try`. See [`AutoroutePassRunner::run_single_thread`]'s doc
/// for the quoted evidence.
///
/// The panic is a real one and not a contrivance: `Item.hasIgnoredNets` dereferences
/// `nets.get(netNumber)` with no null check (Item.java:1244), so an item carrying a net number the
/// net list does not know throws a `NullPointerException` there — and `getAutorouteItems:375`
/// reaches it inside `runSingleThread`'s `try`. The port panics at the same line (`Board::
/// has_ignored_nets`), and the boundary turns it into Java's `return false`.
///
/// `:375` is guarded by `connectedSet.size() < netItemCount`, which is why the board needs
/// **two** items on the undeclared net.
#[test]
fn a_panicking_item_ends_the_pass_and_returns_false() {
    let mut board = empty_board();
    // Net **1** is declared; net 2 is not, and `Nets::get(2)` answers `None`.
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

    // The panic message goes to stderr through the default hook; silence it so a passing run is
    // quiet, and restore the hook afterwards (the `tightener.rs` precedent).
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let answer = run_one_pass(&mut board, &mut router, &stop);
    std::panic::set_hook(hook);

    assert!(
        !answer.expect("the boundary degrades rather than propagating"),
        "AutoroutePassRunner.java:331-335 catches and returns false"
    );

    // And the panic really was where the test claims: the same call outside the boundary throws.
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

/// `:298-302` — the pass ends with `removeTails`, and which `StopConnectionOption` it passes is
/// decided by `router.removeUnconnectedVias`, i.e. `!settings.isFanoutEnabled()` at
/// `BatchAutorouter.java:115`.
///
/// The board has one user-fixed candidate and one ordinary trace with a free end, and the free
/// end is a tail. Asserting that the trace is gone after the pass is what proves `:298-302` ran
/// at all — nothing else in `runSingleThread` removes an item the pass did not route.
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
    assert_eq!(router.autoroute_items(&board), vec![candidate]);
    assert!(board.get_item(tail).is_some(), "before the pass");

    let stop = RouterStop::new();
    run_one_pass(&mut board, &mut router, &stop).expect("the pass answers Ok");

    assert!(
        board.get_item(tail).is_none(),
        "AutoroutePassRunner.java:298-302 runs removeTails, and an unanchored trace is all tail \
         (RoutingBoard.java:1204)"
    );
}

// =================================================================================================
// The failure log — RoutingFailureLog.java
// =================================================================================================

/// `recordFailure` (`:34-48`) and `getFailureCount` (`:95-101`), plus every field of the nested
/// `ItemFailureInfo` (`:109-160`) that a live caller can write.
///
/// The two calls Java makes are `recordFailure(item, passNo, state, details)` — **not** the
/// `(item, netNo, reason, passNo)` the plan's draft sketched — and the nested constructor derives
/// `netNumber` from `item.getNetNumber(0)` on the **first** failure only (`:120`), which the
/// second half asserts by changing nothing and recording again.
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

    // A second failure updates in place; `:43` only constructs when the key is absent.
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

/// `runSingleThread:267-289` writes the log on every state that is neither `ROUTED` nor one of the
/// three "skipped" ones. On the board below every attempt fails — there is no padstack, so no via
/// can be placed and no connection completes — and the log ends up holding the candidate.
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

/// A guard on the fixture the two loop tests rest on: `Trace.isRoutable` (Trace.java:205-209) is
/// `!isUserFixed() && netCount() > 0`, so the fixed state really is what decides whether an item
/// is a candidate at `:358`. If this ever stopped being true, the boards above would test nothing
/// and would still pass.
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

// =================================================================================================
// calculateAirline — AutorouteAirlineCalculator.java:16-40
// =================================================================================================

/// The brief's airline pin. `calculateAirline` picks the closest **drill-item** pair between the
/// two sets, by squared distance (`:31-36`), and answers a line between their centres (`:39`).
///
/// The board carries a trace in each set as well as a via, so the two `instanceof DrillItem`
/// guards (`:21-23`, `:27-29`) have something to skip: a port that measured trace corners would
/// pick a different pair here, because the traces are *closer* to each other than the vias are.
///
/// The values are the vias' own centres, so the expected line is exact and needs no epsilon.
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
    // Two traces that are much closer to each other than any via pair. `:21-29` must skip them.
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

    // `:32`'s `<` is strict, so the *farther* via never displaces the nearer one however the
    // sets are walked. Swapping the two sets swaps the endpoints and nothing else.
    let reversed = fr_router::pipeline::calculate_airline(&board, &dest, &start)
        .expect("still a drill item on each side");
    assert_eq!(reversed.a, airline.b);
    assert_eq!(reversed.b, airline.a);
}

/// `:39` is an unconditional `new FloatLine(fromCorner, toCorner)`, and `FloatLine`'s constructor
/// (FloatLine.java:19-24) takes `null`s with nothing but an `FRLogger.debug` — so on sets holding
/// no drill item Java answers a line whose **both** endpoints are `null`. The port's `None` is
/// the model of that, and the three empty-ish shapes are asserted together because Java's two
/// locals are assigned as a pair at `:33-35` and can never be half-null.
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
