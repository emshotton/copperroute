//! Plan 7 Task 13 — `BatchOptimizer`'s item half: `ReadSortedRouteItems`
//! (`BatchOptimizer.java:563-659`), `optRouteItem` (`:395-514`), `containsOnlyUnfixedTraces`
//! (`:85-92`), `getCurrentPosition` (`:520-525`) and
//! `BatchAutorouter.autoroutePassesForOptimizingItem` (`BatchAutorouter.java:245-281`).
//!
//! # What is pinned here, and what is pinned by the driver
//!
//! The whole-board evidence is `scripts/differential/run.sh p7t8 <dsn> sequence|item` — a real
//! routed board, then the reader walked to exhaustion (mode `sequence`) and then `optRouteItem`
//! driven item by item with the two ripped sets and the ripup costs transcribed beside each call
//! (mode `item`), against the HEAD jar on the corpus stems. That is where "the visit order and
//! the re-route agree with Java" is established, and
//! [`the_item_sequence_is_the_ports_own`](fn.the_item_sequence_is_the_ports_own.html) copies the
//! sequence in as literals so a regression is a unit-test failure rather than a driver run. Those
//! literals were the jar's until the M1 accept wave (ruling BV) re-cut them from the port; see
//! the pin on [`RPI_SEQUENCE`].
//!
//! This file pins the arms a corpus run cannot show, each on a board built for it:
//!
//! * the **via/trace tie** at `:605` and the item the tie *loses* forever;
//! * the **via-contact veto** at `:633-645`, and its asymmetry — a *user-fixed* via does not veto;
//! * the two different "fixed" predicates, `isUserFixed` for a via (`:584`) and `isShoveFixed`
//!   for a trace (`:613`);
//! * **plan-7 ruling 12**: the reader is a rescan, so a board mutated between two `next()` calls
//!   changes the answer;
//! * the terminal cursor — `:651-652` is unconditional, so an exhausted reader parks at
//!   `Integer.MAX_VALUE` (the brief says otherwise; Java wins);
//! * `optRouteItem`'s **user-fixed early exit** (`:436-440`) — **quirk #226**: it cannot fire,
//!   because `getConnectionItems` only ever collects `isRoutable()` items and a user-fixed trace
//!   or via is not one;
//! * `Math.round`'s half-up rounding and the `Float`-widening of `traceRipupCostFactor`
//!   (`:460-463`), neither of which any settings file reaches.

use std::collections::BTreeSet;

use fr_board::ids::PadstackId;
use fr_board::items::Item;
use fr_board::prelude::*;
use fr_geometry::{IntBox, IntOctagon, IntPoint, Point, Polyline, Shape, TileShape};
use fr_router::pipeline::{
    AutorouteBatchLoop, BatchAutorouter, BatchOptimizer, NoopProgressSink,
    PORT_OPTIMIZER_ROUTE_WORK_BUDGET, ReadSortedRouteItems, RouterBudget, RouterStop,
    optimizer_ripup_costs,
};
use fr_settings::sources::DefaultSettings;
use fr_settings::{HostEnvironment, RouterSettings, SettingsSource};

// =================================================================================================
// Fixtures
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

/// `p7t8`'s default stem.
const RPI: &str = "fixtures/Issue143-rpi_splitter.dsn";

fn layers() -> LayerStructure {
    LayerStructure::new(vec![Layer::new("front", true), Layer::new("back", true)])
}

/// A two-layer board with one through padstack, four nets and no items — the canvas every
/// ordering test paints its own vias and traces on. Nothing here is a Java fixture: the reader's
/// arms are decided by coordinates and fixed states, and a real DSN board offers neither on
/// demand.
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

fn thru(board: &Board) -> PadstackId {
    PadstackId(
        board
            .library
            .padstacks
            .get_by_name("thru")
            .expect("the thru padstack")
            .no,
    )
}

fn add_via(board: &mut Board, x: i32, y: i32, net: i32, fixed: FixedState) -> ItemId {
    let padstack = thru(board);
    board
        .insert_via(padstack, Point::new(x, y), vec![net], 1, fixed, false)
        .expect("the via")
}

fn add_trace(
    board: &mut Board,
    from: (i32, i32),
    to: (i32, i32),
    layer: usize,
    net: i32,
    fixed: FixedState,
) -> ItemId {
    board
        .insert_trace_without_cleaning(
            Polyline::from_points(&[Point::new(from.0, from.1), Point::new(to.0, to.1)]),
            layer,
            30,
            vec![net],
            1,
            fixed,
        )
        .expect("the trace")
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

/// `P7T8.buildSettings` — the headless ladder's priority-0 source, sized for this board.
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

/// Walk a fresh reader to exhaustion — `optRoutePass`' loop (`:327-330`) with nothing in it.
fn full_sequence(board: &Board) -> Vec<ItemId> {
    let mut reader = ReadSortedRouteItems::new();
    let mut result = Vec::new();
    while let Some(id) = reader.next(board) {
        result.push(id);
        assert!(
            result.len() <= board.items_in_board_order().len(),
            "the reader is strictly monotone and cannot outrun the board"
        );
    }
    result
}

// =================================================================================================
// The cursor — BatchOptimizer.java:568-571, :651-652, :656-658
// =================================================================================================

#[test]
fn the_fresh_cursor_is_the_int_extremes() {
    // :569-570 — `new FloatPoint(Integer.MIN_VALUE, Integer.MIN_VALUE)` and layer `-1`, i.e. the
    // **`int`** extremes widened to `double`, not an infinity.
    let reader = ReadSortedRouteItems::new();
    assert_eq!(reader.min_item_coor.x, f64::from(i32::MIN));
    assert_eq!(reader.min_item_coor.y, f64::from(i32::MIN));
    assert_eq!(reader.min_item_layer, -1);
    assert_eq!(reader.get_current_position(), reader.min_item_coor);
}

#[test]
fn an_exhausted_reader_parks_the_cursor_at_the_maximum() {
    // `:651-652` is **unconditional**, so a scan that found nothing still writes the two locals
    // — which are still `Integer.MAX_VALUE` — into the cursor. The brief says "the cursor
    // advances only when an item is returned"; Java wins.
    let board = empty_board();
    let mut reader = ReadSortedRouteItems::new();
    assert_eq!(reader.next(&board), None);
    assert_eq!(reader.min_item_coor.x, f64::from(i32::MAX));
    assert_eq!(reader.min_item_coor.y, f64::from(i32::MAX));
    assert_eq!(reader.min_item_layer, i32::MAX);
    // And it stays exhausted: nothing is strictly greater than the maximum.
    assert_eq!(reader.next(&board), None);
}

#[test]
fn get_current_position_is_none_until_a_pass_starts() {
    // `:520-525` — `sortedRouteItems == null` answers `null`.
    let board = empty_board();
    let settings = build_settings(&board);
    let mut optimizer = BatchOptimizer::new(&settings);
    assert_eq!(optimizer.get_current_position(), None);
    optimizer.sorted_route_items = Some(ReadSortedRouteItems::new());
    assert_eq!(
        optimizer
            .get_current_position()
            .expect("a reader is installed")
            .x,
        f64::from(i32::MIN)
    );
}

// =================================================================================================
// The two scans — BatchOptimizer.java:577-650
// =================================================================================================

#[test]
fn a_via_wins_a_tie_with_a_trace_at_the_same_coordinate() {
    // `:605`: "read traces last to prefer vias to traces at the same location". The trace scan's
    // test against `currentMinCoor` is a strict `<`, so an exact tie leaves the via in place.
    let mut board = empty_board();
    let via = add_via(&mut board, 1_000, 1_000, 1, FixedState::Unfixed);
    // `compareCorner` is the **larger** endpoint (`:617-622`), so this trace keys on
    // `(1000, 1000)` — the via's centre — and it is on layer 0, the via's `firstLayer`. A
    // different net keeps it out of the via's contact set, so the veto at `:633-645` is not what
    // is being measured here.
    let trace = add_trace(
        &mut board,
        (500, 500),
        (1_000, 1_000),
        0,
        2,
        FixedState::Unfixed,
    );

    let mut reader = ReadSortedRouteItems::new();
    assert_eq!(reader.next(&board), Some(via), "the via wins the tie");
    // And the trace is now lost for good: the cursor sits on `(1000, 1000, 0)` and `:624-627`
    // demands *strictly* after, so the trace can never be strictly greater than itself.
    assert_eq!(reader.next(&board), None);
    assert!(board.get_item(trace).is_some(), "the trace is still there");
}

#[test]
fn a_trace_one_unit_below_the_via_wins() {
    // The contrast that makes the tie test mean something: move the trace's key one unit down and
    // `:628-632`'s strict `<` fires, so the trace comes first and the via second.
    let mut board = empty_board();
    let via = add_via(&mut board, 1_000, 1_000, 1, FixedState::Unfixed);
    let trace = add_trace(
        &mut board,
        (500, 500),
        (1_000, 999),
        0,
        2,
        FixedState::Unfixed,
    );
    assert_eq!(full_sequence(&board), vec![trace, via]);
}

#[test]
fn a_trace_touching_an_unfixed_via_is_skipped_and_a_user_fixed_one_does_not_veto() {
    // `:633-645`. The veto asks `currentContact instanceof Via && !currentContact.isUserFixed()`,
    // so the two halves of this test differ only in the contact via's fixed state.
    for (contact_state, trace_is_returned) in
        [(FixedState::Unfixed, false), (FixedState::UserFixed, true)]
    {
        let mut board = empty_board();
        let contact_via = add_via(&mut board, 1_000, 1_000, 1, contact_state);
        // Keyed on `(3000, 3000)` — its larger endpoint — and touching `contact_via` at its
        // smaller one, so it is a candidate that the veto can reject.
        let trace = add_trace(
            &mut board,
            (1_000, 1_000),
            (3_000, 3_000),
            0,
            1,
            FixedState::Unfixed,
        );
        assert!(
            board.normal_contacts(trace).contains(&contact_via),
            "the trace must actually touch the via for the veto to be under test"
        );
        let last_via = add_via(&mut board, 5_000, 5_000, 3, FixedState::Unfixed);

        let expected: Vec<ItemId> = if trace_is_returned {
            // A user-fixed via is skipped by the via scan (`:584`) *and* does not veto (`:636`).
            vec![trace, last_via]
        } else {
            vec![contact_via, last_via]
        };
        assert_eq!(full_sequence(&board), expected, "contact {contact_state:?}");
    }
}

#[test]
fn a_user_fixed_via_and_a_shove_fixed_trace_are_skipped() {
    // The two predicates are different predicates: `:584` asks a via `isUserFixed()` and `:613`
    // asks a trace `isShoveFixed()`.
    let mut board = empty_board();
    add_via(&mut board, 1_000, 1_000, 1, FixedState::UserFixed);
    add_trace(
        &mut board,
        (1_500, 1_500),
        (2_000, 2_000),
        0,
        2,
        FixedState::ShoveFixed,
    );
    let survivor = add_via(&mut board, 5_000, 5_000, 3, FixedState::Unfixed);
    assert_eq!(full_sequence(&board), vec![survivor]);
}

#[test]
fn a_shove_fixed_via_is_still_returned() {
    // The other side of the asymmetry above: `:584` is `isUserFixed`, **not** `isShoveFixed`, so
    // a shove-fixed via is a perfectly good optimizer candidate.
    let mut board = empty_board();
    let via = add_via(&mut board, 1_000, 1_000, 1, FixedState::ShoveFixed);
    assert_eq!(full_sequence(&board), vec![via]);
}

#[test]
fn a_trace_is_keyed_on_its_larger_endpoint() {
    // `:617-622` picks `compareCorner` as the endpoint that is **not** the lexicographic minimum
    // of the two, so a long trace is keyed where it ends rather than where it starts. The two
    // traces here are chosen so that the answer flips if the choice does: keyed on the larger
    // endpoint the short one comes first, keyed on the smaller one the long one does.
    let mut board = empty_board();
    // Key `(5000, 5000)` on the larger endpoint, `(100, 100)` on the smaller.
    let long = add_trace(
        &mut board,
        (100, 100),
        (5_000, 5_000),
        0,
        1,
        FixedState::Unfixed,
    );
    // Key `(3000, 3000)` on the larger endpoint, `(2000, 2000)` on the smaller.
    let short = add_trace(
        &mut board,
        (2_000, 2_000),
        (3_000, 3_000),
        0,
        2,
        FixedState::Unfixed,
    );
    // Different nets, so neither is a contact of the other and the `:633-645` veto is not in play.
    assert!(board.normal_contacts(long).is_empty());
    assert_eq!(full_sequence(&board), vec![short, long]);
}

#[test]
fn the_layer_is_the_third_comparison_key() {
    // `:590-591` and `:594-596`: two vias at the same centre are ordered by `firstLayer()`, and
    // two traces at the same corner by `getLayer()`. A one-layer via is the only way to move a
    // via's `firstLayer`, so this uses two traces on the same corner instead.
    let mut board = empty_board();
    let back = add_trace(
        &mut board,
        (500, 500),
        (1_000, 1_000),
        1,
        2,
        FixedState::Unfixed,
    );
    let front = add_trace(
        &mut board,
        (400, 400),
        (1_000, 1_000),
        0,
        3,
        FixedState::Unfixed,
    );
    // Both key on `(1000, 1000)`; layer 0 sorts before layer 1.
    assert_eq!(full_sequence(&board), vec![front, back]);
}

#[test]
fn the_rescan_sees_items_the_previous_call_moved() {
    // **Plan-7 ruling 12.** `next()` is a full rescan of `board.itemList`, twice, so a board that
    // changed between two calls changes the answer. A memoised sequence would answer `far` here.
    let mut board = empty_board();
    let near = add_via(&mut board, 1_000, 1_000, 1, FixedState::Unfixed);
    let far = add_via(&mut board, 5_000, 5_000, 3, FixedState::Unfixed);

    let mut reader = ReadSortedRouteItems::new();
    assert_eq!(reader.next(&board), Some(near));
    // The mutation an `optRouteItem` would have made: a via that did not exist when the first
    // `next()` ran, between the cursor and what used to come next.
    let inserted = add_via(&mut board, 3_000, 3_000, 4, FixedState::Unfixed);
    assert_eq!(
        reader.next(&board),
        Some(inserted),
        "the reader must rescan, not replay"
    );
    assert_eq!(reader.next(&board), Some(far));
    assert_eq!(reader.next(&board), None);
}

#[test]
fn a_removed_item_is_simply_not_seen_again() {
    // The other half of ruling 12: the board `optRouteItem` leaves behind has fewer items, and
    // the reader must not hand out an id the board no longer has.
    let mut board = empty_board();
    let near = add_via(&mut board, 1_000, 1_000, 1, FixedState::Unfixed);
    let far = add_via(&mut board, 5_000, 5_000, 3, FixedState::Unfixed);
    let mut reader = ReadSortedRouteItems::new();
    assert_eq!(reader.next(&board), Some(near));
    board.remove_items([far]);
    assert_eq!(reader.next(&board), None);
}

// =================================================================================================
// containsOnlyUnfixedTraces — BatchOptimizer.java:85-92
// =================================================================================================

#[test]
fn contains_only_unfixed_traces_answers_javas_three_cases() {
    let mut board = empty_board();
    let unfixed_trace = add_trace(&mut board, (0, 0), (1_000, 0), 0, 1, FixedState::Unfixed);
    let user_fixed_trace = add_trace(
        &mut board,
        (0, 500),
        (1_000, 500),
        0,
        2,
        FixedState::UserFixed,
    );
    let via = add_via(&mut board, 3_000, 3_000, 3, FixedState::Unfixed);

    // `:91` — vacuously true on the empty collection, which is the answer that matters: a trace
    // with no start contacts takes the `addAll` branch with nothing to add (`:421-422`).
    assert!(BatchOptimizer::contains_only_unfixed_traces(
        &board,
        &BTreeSet::new()
    ));
    assert!(BatchOptimizer::contains_only_unfixed_traces(
        &board,
        &BTreeSet::from([unfixed_trace])
    ));
    // `:87` — `isUserFixed()` first.
    assert!(!BatchOptimizer::contains_only_unfixed_traces(
        &board,
        &BTreeSet::from([unfixed_trace, user_fixed_trace])
    ));
    // `:87` — `!(currentItem instanceof Trace)` second.
    assert!(!BatchOptimizer::contains_only_unfixed_traces(
        &board,
        &BTreeSet::from([unfixed_trace, via])
    ));
    // A shove-fixed trace is *not* excluded: `:87` asks `isUserFixed`, nothing else.
    let shove_fixed = add_trace(
        &mut board,
        (0, 900),
        (1_000, 900),
        0,
        4,
        FixedState::ShoveFixed,
    );
    assert!(BatchOptimizer::contains_only_unfixed_traces(
        &board,
        &BTreeSet::from([shove_fixed])
    ));
}

// =================================================================================================
// The ripup-cost ladder — BatchOptimizer.java:453-463
// =================================================================================================

#[test]
fn the_trace_ripup_cost_factor_is_rounded_java_style() {
    let board = empty_board();
    let mut settings = build_settings(&board);
    let base = settings.get_start_ripup_costs();
    let optimizer = settings
        .optimizer
        .as_mut()
        .expect("DefaultSettings builds an optimizer block");
    // `DefaultSettings.java:139-140` — the only two values a corpus run can show.
    assert_eq!(optimizer.additional_ripup_cost_factor_at_start, Some(10));
    assert_eq!(optimizer.trace_ripup_cost_factor, Some(0.6));

    // The four corpus-reachable answers.
    assert_eq!(optimizer_ripup_costs(&settings, false, false), base);
    assert_eq!(optimizer_ripup_costs(&settings, true, false), base * 10);
    assert_eq!(
        optimizer_ripup_costs(&settings, true, true),
        fr_geometry::java_round(f64::from(0.6_f32) * f64::from(base * 10)) as i32
    );
    assert_eq!(optimizer_ripup_costs(&settings, true, false), 1_000);
    assert_eq!(optimizer_ripup_costs(&settings, true, true), 600);

    // Half-up, not banker's: `Math.round(2.5) == 3`.
    settings
        .optimizer
        .as_mut()
        .expect("an optimizer block")
        .additional_ripup_cost_factor_at_start = Some(1);
    settings
        .optimizer
        .as_mut()
        .expect("an optimizer block")
        .trace_ripup_cost_factor = Some(0.025);
    // 0.025f as a double is 0.02500000037252903; 100 * that rounds to 3 either way, so use the
    // exact half instead.
    settings
        .optimizer
        .as_mut()
        .expect("an optimizer block")
        .trace_ripup_cost_factor = Some(0.5);
    settings.set_start_ripup_costs(5);
    assert_eq!(settings.get_start_ripup_costs(), 5);
    assert_eq!(optimizer_ripup_costs(&settings, true, true), 3, "2.5 -> 3");

    // …and **negative** half-up, which is where `Math.round` and Rust's `f64::round` disagree:
    // Java answers `-2` (it is `floor(x + 0.5)`), `f64::round` answers `-3`.
    settings
        .optimizer
        .as_mut()
        .expect("an optimizer block")
        .trace_ripup_cost_factor = Some(-0.5);
    assert_eq!(
        optimizer_ripup_costs(&settings, true, true),
        -2,
        "-2.5 -> -2"
    );
    assert_eq!((-2.5_f64).round() as i32, -3, "the trap this avoids");

    // The `Float` widening: `0.1f` is not `0.1`, and at this scale the difference is a whole
    // count. `0.1f == 0.100000001490116119384765625`, so `0.1f * 45` is `4.500000067…` and rounds
    // to 5, while `0.1 * 45` is exactly `4.5` — which also rounds to 5 — but `0.1f * 5` is
    // `0.500000007…` and `0.1 * 5` is `0.5000000000000001`; the pair that separates them is
    // below.
    settings
        .optimizer
        .as_mut()
        .expect("an optimizer block")
        .trace_ripup_cost_factor = Some(0.7);
    settings.set_start_ripup_costs(5);
    // 0.7f == 0.699999988079071; 0.7f * 5 == 3.499999940395355 -> 3, while 0.7 * 5 == 3.5 -> 4.
    assert_eq!(
        optimizer_ripup_costs(&settings, true, true),
        3,
        "the float factor is widened after its own rounding, not before"
    );
    assert_eq!(fr_geometry::java_round(0.7_f64 * 5.0) as i32, 4, "the trap");
}

// =================================================================================================
// autoroutePassesForOptimizingItem — BatchAutorouter.java:245-281
// =================================================================================================

#[test]
fn the_optimizer_autorouter_always_removes_unconnected_vias() {
    // `:258` is a literal `true`, not `!settings.isFanoutEnabled()`. The contrast is the router
    // the pass loop builds: with fanout **on**, `:115` makes it `false` there and `:258` still
    // makes it `true` here.
    let board = empty_board();
    let mut settings = build_settings(&board);
    settings.fanout.get_or_insert_with(Default::default).enabled = Some(true);
    assert!(settings.is_fanout_enabled());

    let pass_loop_router =
        BatchAutorouter::for_routing_job(&board, &settings, RouterBudget::disabled());
    assert!(
        !pass_loop_router.is_remove_unconnected_vias(),
        "BatchAutorouter.java:115 — the pass loop's router follows the fanout setting"
    );

    // The constructor call `:253-261` makes, with `:258`'s literal in place.
    let optimizer_router = BatchAutorouter::new(
        &board,
        &settings,
        true,
        true,
        1_000,
        500,
        RouterBudget::disabled(),
    );
    assert!(optimizer_router.is_remove_unconnected_vias());

    // And the source shape, so the literal cannot quietly become the setting again: the audited
    // marker and the `:258` comment sit on the call.
    let source = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/pipeline/batch_autorouter.rs"
    ))
    .expect("the port's own source");
    let body = source
        .split("pub fn autoroute_passes_for_optimizing_item(")
        .nth(1)
        .expect("the method is in this file");
    let ctor = body
        .split("BatchAutorouter::new(")
        .nth(1)
        .expect("the method builds a second router");
    assert!(
        ctor.contains("// :258 — unconditional.\n            true,"),
        "`:258` must stay a literal `true`"
    );
    assert!(
        body.contains("StopConnectionOption::None"),
        "`:276` is `removeTails(NONE)`"
    );
}

// =================================================================================================
// optRouteItem — BatchOptimizer.java:395-514
// =================================================================================================

#[test]
fn a_user_fixed_contact_never_reaches_the_ripped_connections() {
    // **Quirk #226.** `:434-440` is described in Java's own comment as "check if the connections
    // contain user fixed items, which should not be re-routed", and it cannot fire.
    // `rippedConnections` is filled from **nothing but** `getConnectionItems`
    // (`:428-432`), and that method adds an item only when `isRoutable()` — `:701-703` for the
    // start item and `:723-726` for every step of the walk. `Trace.isRoutable` (Trace.java:206-208)
    // and `Via.isRoutable` (Via.java:147-149) are both `!isUserFixed() && netCount() > 0`, and the
    // base `Item.isRoutable` (`:864-866`) is `false`. So every member of `rippedConnections` is
    // routable, therefore not user-fixed, and `:437` is dead on every path.
    //
    // `p7t8 item` prints `anyUserFixed` on every `ITEM` line and it reads `false` on every item of
    // every corpus stem, which is the same statement measured rather than argued.
    let mut board = empty_board();
    let fixed_via = add_via(&mut board, 1_000, 1_000, 1, FixedState::UserFixed);
    let trace = add_trace(
        &mut board,
        (1_000, 1_000),
        (3_000, 3_000),
        0,
        1,
        FixedState::Unfixed,
    );

    // The via really is a contact — this is not a fixture that failed to touch.
    assert!(board.normal_contacts(trace).contains(&fixed_via));
    // …and `Via.isRoutable` is what keeps it out of the connection set.
    assert!(
        !board.get_item(fixed_via).expect("the via").is_routable(),
        "Via.java:147-149"
    );
    let connections = board.connection_items(trace, fr_board::StopConnectionOption::None);
    assert_eq!(connections, BTreeSet::from([trace]));
    assert!(
        !connections.contains(&fixed_via),
        "`:437` can never see a user-fixed item"
    );

    // The same argument in the other direction: make the via unfixed and it *is* collected, so
    // the exclusion is `isUserFixed`'s and not a quirk of this fixture's geometry.
    let mut unfixed_board = empty_board();
    let free_via = add_via(&mut unfixed_board, 1_000, 1_000, 1, FixedState::Unfixed);
    let free_trace = add_trace(
        &mut unfixed_board,
        (1_000, 1_000),
        (3_000, 3_000),
        0,
        1,
        FixedState::Unfixed,
    );
    assert!(
        unfixed_board
            .connection_items(free_trace, fr_board::StopConnectionOption::None)
            .contains(&free_via)
    );
}

// =================================================================================================
// The routed-board sequence — the shape of `run.sh p7t8 <rpi> sequence|item 1 all`
// =================================================================================================

/// The reader's walk over the routed `Issue143-rpi_splitter.dsn` — `(id, class, key x, key y,
/// layer)` in order.
/// PORT-REGRESSION PIN — re-cut at the M1 accept wave (ruling BV), and the test that reads it
/// renamed with the literals (`the_item_sequence_matches_the_jvm`). The jar-parity sequence was
/// **six** entries, ids `43, 49, 99, 104, 238, 161`, the fifth of them a `PolylineTrace` at
/// `(1 116 000, 802 200)`. Plan 9 Task 2's R1 (#293) re-orders the work list and R2 (#294)
/// withholds the sub-minimum fanout trace, so the routed board offers **five** items, every id
/// has moved and the trace is gone — while the four surviving key coordinates are the jar's to
/// the digit, which is what says the *reader's* order is unchanged and only the board moved.
/// Accepted at M1 (ruling BV).
const RPI_SEQUENCE: [(u32, &str, f64, f64, usize); 5] = [
    (86, "Via", 531_001.0, 2_346_089.0, 0),
    (92, "Via", 546_813.0, 2_253_860.0, 0),
    (41, "Via", 552_083.0, 1_806_420.0, 0),
    (46, "Via", 666_000.0, 1_432_720.0, 0),
    (127, "Via", 1_366_000.0, 1_007_139.0, 0),
];

#[test]
#[cfg_attr(debug_assertions, ignore)]
fn the_item_sequence_is_the_ports_own() {
    let mut board = load_board(RPI);
    let settings = build_settings(&board);
    let stop = RouterStop::new();
    let mut sink = NoopProgressSink;
    let mut routed_settings = settings.clone();
    routed_settings.max_passes = Some(1);
    AutorouteBatchLoop::run(
        &mut board,
        &routed_settings,
        &stop,
        RouterBudget::disabled(),
        &mut sink,
    )
    .expect("rpi_splitter has a routable signal layer");

    let ctx = board.ctx();
    let actual: Vec<(u32, &str, f64, f64, usize)> = full_sequence(&board)
        .into_iter()
        .map(|id| match board.get_item(id).expect("a live item") {
            Item::Via(via) => {
                let centre = via.get_center().to_float();
                (id.0, "Via", centre.x, centre.y, via.first_layer(&ctx))
            }
            Item::Trace(trace) => {
                let first = trace.first_corner().expect("a corner").to_float();
                let last = trace.last_corner().expect("a corner").to_float();
                let key = if first.x < last.x || first.x == last.x && first.y < last.y {
                    last
                } else {
                    first
                };
                (id.0, "PolylineTrace", key.x, key.y, trace.get_layer())
            }
            other => panic!("the reader returned a {other:?}"),
        })
        .collect();
    assert_eq!(actual.as_slice(), RPI_SEQUENCE.as_slice());
}

#[test]
#[cfg_attr(debug_assertions, ignore)]
fn an_unimproved_item_restores_the_clone_byte_for_byte() {
    // Plan-7 ruling 8, measured. Item 0 (`id = 86`) is `improved=true` and item 1 (`id = 92`) is
    // `improved=false`, and the board after item 1 is item 0's board again — while the id
    // generator's maximum has moved on, because Java's `undo` does not roll it back and neither
    // does this port.
    //
    // PORT-REGRESSION PIN — re-cut at the M1 accept wave (ruling BV). The jar-parity ids were
    // `p7t8 <rpi> item 1 all`'s **43** and **49**; Plan 9 Task 2's R1 (#293)/R2 (#294) route the
    // board differently and the reader's first two items are now **86** and **92**. The two
    // *claims* — the first item improves, the second does not and is restored byte for byte while
    // its burned ids stay burned — are unchanged and are what this test is for. Accepted at M1
    // (ruling BV).
    let mut board = load_board(RPI);
    let settings = build_settings(&board);
    let stop = RouterStop::new();
    let mut sink = NoopProgressSink;
    let mut routed_settings = settings.clone();
    routed_settings.max_passes = Some(1);
    AutorouteBatchLoop::run(
        &mut board,
        &routed_settings,
        &stop,
        RouterBudget::disabled(),
        &mut sink,
    )
    .expect("rpi_splitter has a routable signal layer");

    // `optRoutePass:283, :288` and `runBatchLoop:132`, which is how `p7t8 item` seeds it.
    let mut optimizer = BatchOptimizer::new(&settings);
    optimizer.use_increased_ripup_costs = true;
    optimizer.min_cumulative_trace_length = f64::from(
        fr_router::score::BoardStatistics::new(&mut board)
            .traces
            .total_weighted_length
            .expect("a routed board has traces"),
    );
    // A **fresh** stop: `AutorouteBatchLoop:271` left the routing one at `AUTO_ROUTER_ONLY`, and
    // `autoroutePassesForOptimizingItem:268` would run zero passes on it (see `p7t8.rs`).
    let optimizer_stop = RouterStop::new();
    let mut reader = ReadSortedRouteItems::new();

    let first = reader.next(&board).expect("item 0");
    assert_eq!(first.0, 86);
    let improved = optimizer
        .opt_route_item(
            &mut board,
            first,
            true,
            false,
            &optimizer_stop,
            RouterBudget::disabled(),
            &mut sink,
        )
        .expect("optRouteItem answers Ok");
    assert!(improved.improved(), "item 0 is improved=true");
    let hash_after_first = board.structural_hash();
    let id_after_first = board.communication.id_gen.max_generated_id();

    let second = reader.next(&board).expect("item 1");
    assert_eq!(second.0, 92);
    let unimproved = optimizer
        .opt_route_item(
            &mut board,
            second,
            true,
            false,
            &optimizer_stop,
            RouterBudget::disabled(),
            &mut sink,
        )
        .expect("optRouteItem answers Ok");
    assert!(!unimproved.improved(), "item 1 is improved=false");
    assert_eq!(
        board.structural_hash(),
        hash_after_first,
        "the clone restores the board the failed item started from"
    );
    assert!(
        board.communication.id_gen.max_generated_id() > id_after_first,
        "…but the ids the failed attempt burned stay burned (BasicBoard.undo:1233-1240)"
    );
}

// =================================================================================================
// ruling CI — the real routing-work accumulator (SF1)
// =================================================================================================

/// **fixed: T9 (ruling CI), SF1.** The invariant the whole bound rests on is that a **complete**-
/// board item charges the routing-work budget **zero** (so via-/length-optimization is never
/// bounded) while an **incomplete**-board item charges `incompleteCount × passesRun`. The
/// synthetic pin `optimizer.rs::the_route_work_budget_bounds_only_incomplete_board_routing`
/// checks the *arithmetic* against a re-implemented closure; **this** test drives the **production**
/// [`BatchOptimizer::total_route_work`] accumulator through real `opt_route_item` calls, so a
/// regression that dropped the `incomplete_count_before` factor is caught in the fast lane rather
/// than only in the slow-lane dac2020/cnh A/B.
///
/// Release-gated like every routed-board test in this file: fast in release (the default lane
/// runs release), minutes in an unoptimised debug build.
#[test]
#[cfg_attr(debug_assertions, ignore)]
fn the_real_route_work_accumulator_charges_zero_on_a_complete_board() {
    // ---- complete board: a single via on a net has nothing to connect, so incomplete == 0 ------
    let mut board = empty_board();
    let lone_via = add_via(&mut board, 0, 0, 1, FixedState::Unfixed);
    let settings = build_settings(&board);
    let mut sink = NoopProgressSink;

    // Route once at maxPasses=1 to initialise the board's trees exactly as the routed-board tests
    // do; there is nothing to route, so the board stays complete.
    let stop = RouterStop::new();
    AutorouteBatchLoop::run(
        &mut board,
        &settings,
        &stop,
        RouterBudget::disabled(),
        &mut sink,
    )
    .expect("the synthetic board has a routable signal layer");
    assert_eq!(
        BatchAutorouter::calculate_incomplete_count(&mut board),
        0,
        "a single-via net is complete — incomplete_count_before will be 0"
    );

    let mut optimizer = BatchOptimizer::new(&settings);
    optimizer.use_increased_ripup_costs = true;
    assert_eq!(
        optimizer.total_route_work, 0,
        "a fresh stage has spent nothing"
    );
    let stop = RouterStop::new();
    optimizer
        .opt_route_item(
            &mut board,
            lone_via,
            true,
            false,
            &stop,
            RouterBudget::disabled(),
            &mut sink,
        )
        .expect("optRouteItem answers Ok on a complete board");
    // The production accumulator, not a closure: the item ran real passes, and the board was
    // complete, so its charge is `0 × passesRun == 0`. A regression to `+= passesRun` would make
    // this non-zero and the whole bound would start strangling complete-board optimization.
    assert_eq!(
        optimizer.total_route_work, 0,
        "invariant-2b: a complete-board item charges the budget nothing"
    );
    assert!(!optimizer.route_work_budget_spent());
}

/// The other half, on the production accumulator: an **incomplete**-board item charges
/// `incompleteCount × passesRun` — so the `incomplete_count_before` factor is present. Uses the
/// routed `rpi_splitter`, which R2 (#294) leaves with a stubborn incomplete count the router
/// cannot close, so the board handed to the optimizer is genuinely incomplete.
#[test]
#[cfg_attr(debug_assertions, ignore)]
fn the_real_route_work_accumulator_charges_incomplete_count_times_passes() {
    let mut board = load_board(RPI);
    let settings = build_settings(&board);
    let stop = RouterStop::new();
    let mut sink = NoopProgressSink;
    let mut routed_settings = settings.clone();
    routed_settings.max_passes = Some(1);
    AutorouteBatchLoop::run(
        &mut board,
        &routed_settings,
        &stop,
        RouterBudget::disabled(),
        &mut sink,
    )
    .expect("rpi_splitter has a routable signal layer");

    let incomplete_before = BatchAutorouter::calculate_incomplete_count(&mut board);
    assert!(
        incomplete_before >= 2,
        "the routed rpi is incomplete (R2's stubborn connections); got {incomplete_before}"
    );

    let mut optimizer = BatchOptimizer::new(&settings);
    optimizer.use_increased_ripup_costs = true;
    optimizer.min_cumulative_trace_length = f64::from(
        fr_router::score::BoardStatistics::new(&mut board)
            .traces
            .total_weighted_length
            .expect("a routed board has traces"),
    );
    let mut reader = ReadSortedRouteItems::new();
    let item = reader
        .next(&board)
        .expect("the routed board offers an item");

    let stop = RouterStop::new();
    optimizer
        .opt_route_item(
            &mut board,
            item,
            true,
            false,
            &stop,
            RouterBudget::disabled(),
            &mut sink,
        )
        .expect("optRouteItem answers Ok");

    // The charge is `incomplete_count_before × passesRun`. `passesRun` is
    // `autoroutePassesForOptimizingItem`'s return, in `1..=maxAutoroutePasses+1` (`= 7`). So the
    // charge is a positive multiple of `incomplete_count_before` — which a regression that dropped
    // the factor (`+= passesRun`) would break, because `passesRun` is not a multiple of
    // `incomplete_count_before >= 2` for every pass count.
    let work = optimizer.total_route_work;
    let inc = i64::try_from(incomplete_before).expect("a non-negative count");
    assert!(work > 0, "an incomplete-board item is charged, got {work}");
    assert_eq!(
        work % inc,
        0,
        "the charge {work} must be a multiple of incomplete_count_before {inc} — the factor is \
         present"
    );
    let passes = work / inc;
    assert!(
        (1..=7).contains(&passes),
        "the quotient {passes} is passesRun, which lives in 1..=maxAutoroutePasses+1"
    );
    // And the value is well under the budget on one item, so the budget only bites in aggregate.
    assert!(work < PORT_OPTIMIZER_ROUTE_WORK_BUDGET);
}
