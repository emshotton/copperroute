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

const RPI: &str = "fixtures/Issue143-rpi_splitter.dsn";

fn layers() -> LayerStructure {
    LayerStructure::new(vec![Layer::new("front", true), Layer::new("back", true)])
}

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

#[test]
fn the_fresh_cursor_is_the_int_extremes() {
    let reader = ReadSortedRouteItems::new();
    assert_eq!(reader.min_item_coor.x, f64::from(i32::MIN));
    assert_eq!(reader.min_item_coor.y, f64::from(i32::MIN));
    assert_eq!(reader.min_item_layer, -1);
    assert_eq!(reader.get_current_position(), reader.min_item_coor);
}

#[test]
fn an_exhausted_reader_parks_the_cursor_at_the_maximum() {
    let board = empty_board();
    let mut reader = ReadSortedRouteItems::new();
    assert_eq!(reader.next(&board), None);
    assert_eq!(reader.min_item_coor.x, f64::from(i32::MAX));
    assert_eq!(reader.min_item_coor.y, f64::from(i32::MAX));
    assert_eq!(reader.min_item_layer, i32::MAX);
    assert_eq!(reader.next(&board), None);
}

#[test]
fn get_current_position_is_none_until_a_pass_starts() {
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

#[test]
fn a_via_wins_a_tie_with_a_trace_at_the_same_coordinate() {
    let mut board = empty_board();
    let via = add_via(&mut board, 1_000, 1_000, 1, FixedState::Unfixed);
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
    assert_eq!(reader.next(&board), None);
    assert!(board.get_item(trace).is_some(), "the trace is still there");
}

#[test]
fn a_trace_one_unit_below_the_via_wins() {
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
    for (contact_state, trace_is_returned) in
        [(FixedState::Unfixed, false), (FixedState::UserFixed, true)]
    {
        let mut board = empty_board();
        let contact_via = add_via(&mut board, 1_000, 1_000, 1, contact_state);
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
            vec![trace, last_via]
        } else {
            vec![contact_via, last_via]
        };
        assert_eq!(full_sequence(&board), expected, "contact {contact_state:?}");
    }
}

#[test]
fn a_user_fixed_via_and_a_shove_fixed_trace_are_skipped() {
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
    let mut board = empty_board();
    let via = add_via(&mut board, 1_000, 1_000, 1, FixedState::ShoveFixed);
    assert_eq!(full_sequence(&board), vec![via]);
}

#[test]
fn a_trace_is_keyed_on_its_larger_endpoint() {
    let mut board = empty_board();
    let long = add_trace(
        &mut board,
        (100, 100),
        (5_000, 5_000),
        0,
        1,
        FixedState::Unfixed,
    );
    let short = add_trace(
        &mut board,
        (2_000, 2_000),
        (3_000, 3_000),
        0,
        2,
        FixedState::Unfixed,
    );
    assert!(board.normal_contacts(long).is_empty());
    assert_eq!(full_sequence(&board), vec![short, long]);
}

#[test]
fn the_layer_is_the_third_comparison_key() {
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
    assert_eq!(full_sequence(&board), vec![front, back]);
}

#[test]
fn the_rescan_sees_items_the_previous_call_moved() {
    let mut board = empty_board();
    let near = add_via(&mut board, 1_000, 1_000, 1, FixedState::Unfixed);
    let far = add_via(&mut board, 5_000, 5_000, 3, FixedState::Unfixed);

    let mut reader = ReadSortedRouteItems::new();
    assert_eq!(reader.next(&board), Some(near));
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
    let mut board = empty_board();
    let near = add_via(&mut board, 1_000, 1_000, 1, FixedState::Unfixed);
    let far = add_via(&mut board, 5_000, 5_000, 3, FixedState::Unfixed);
    let mut reader = ReadSortedRouteItems::new();
    assert_eq!(reader.next(&board), Some(near));
    board.remove_items([far]);
    assert_eq!(reader.next(&board), None);
}

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

    assert!(BatchOptimizer::contains_only_unfixed_traces(
        &board,
        &BTreeSet::new()
    ));
    assert!(BatchOptimizer::contains_only_unfixed_traces(
        &board,
        &BTreeSet::from([unfixed_trace])
    ));
    assert!(!BatchOptimizer::contains_only_unfixed_traces(
        &board,
        &BTreeSet::from([unfixed_trace, user_fixed_trace])
    ));
    assert!(!BatchOptimizer::contains_only_unfixed_traces(
        &board,
        &BTreeSet::from([unfixed_trace, via])
    ));
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

#[test]
fn the_trace_ripup_cost_factor_is_rounded_java_style() {
    let board = empty_board();
    let mut settings = build_settings(&board);
    let base = settings.get_start_ripup_costs();
    let optimizer = settings
        .optimizer
        .as_mut()
        .expect("DefaultSettings builds an optimizer block");
    assert_eq!(optimizer.additional_ripup_cost_factor_at_start, Some(10));
    assert_eq!(optimizer.trace_ripup_cost_factor, Some(0.6));

    assert_eq!(optimizer_ripup_costs(&settings, false, false), base);
    assert_eq!(optimizer_ripup_costs(&settings, true, false), base * 10);
    assert_eq!(
        optimizer_ripup_costs(&settings, true, true),
        fr_geometry::java_round(f64::from(0.6_f32) * f64::from(base * 10)) as i32
    );
    assert_eq!(optimizer_ripup_costs(&settings, true, false), 1_000);
    assert_eq!(optimizer_ripup_costs(&settings, true, true), 600);

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
    settings
        .optimizer
        .as_mut()
        .expect("an optimizer block")
        .trace_ripup_cost_factor = Some(0.5);
    settings.set_start_ripup_costs(5);
    assert_eq!(settings.get_start_ripup_costs(), 5);
    assert_eq!(optimizer_ripup_costs(&settings, true, true), 3, "2.5 -> 3");

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

    settings
        .optimizer
        .as_mut()
        .expect("an optimizer block")
        .trace_ripup_cost_factor = Some(0.7);
    settings.set_start_ripup_costs(5);
    assert_eq!(
        optimizer_ripup_costs(&settings, true, true),
        3,
        "the float factor is widened after its own rounding, not before"
    );
    assert_eq!(fr_geometry::java_round(0.7_f64 * 5.0) as i32, 4, "the trap");
}

#[test]
fn the_optimizer_autorouter_always_removes_unconnected_vias() {
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
}

#[test]
fn a_user_fixed_contact_never_reaches_the_ripped_connections() {
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

    assert!(board.normal_contacts(trace).contains(&fixed_via));
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

    let mut optimizer = BatchOptimizer::new(&settings);
    optimizer.use_increased_ripup_costs = true;
    optimizer.min_cumulative_trace_length = f64::from(
        fr_router::score::BoardStatistics::new(&mut board)
            .traces
            .total_weighted_length
            .expect("a routed board has traces"),
    );
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

#[test]
#[cfg_attr(debug_assertions, ignore)]
fn an_exhausted_search_budget_restores_the_speculative_item() {
    let mut board = load_board(RPI);
    let mut settings = build_settings(&board);
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

    settings
        .optimizer
        .as_mut()
        .expect("DefaultSettings always fills the optimizer block")
        .max_search_steps = Some(1);
    let before = board.structural_hash();
    let mut optimizer = BatchOptimizer::new(&settings);
    optimizer.use_increased_ripup_costs = true;
    optimizer.min_cumulative_trace_length = f64::from(
        fr_router::score::BoardStatistics::new(&mut board)
            .traces
            .total_weighted_length
            .expect("a routed board has traces"),
    );
    let item = ReadSortedRouteItems::new()
        .next(&board)
        .expect("the routed board offers an item");

    let result = optimizer
        .opt_route_item(
            &mut board,
            item,
            true,
            false,
            &RouterStop::new(),
            RouterBudget::disabled(),
            &mut sink,
        )
        .expect("optRouteItem answers Ok");

    assert!(!result.improved());
    assert!(optimizer.search_work_budget_spent());
    assert_eq!(board.structural_hash(), before);
}

#[test]
#[cfg_attr(debug_assertions, ignore)]
fn the_real_route_work_accumulator_charges_zero_on_a_complete_board() {
    let mut board = empty_board();
    let lone_via = add_via(&mut board, 0, 0, 1, FixedState::Unfixed);
    let settings = build_settings(&board);
    let mut sink = NoopProgressSink;

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
    assert_eq!(
        optimizer.total_route_work, 0,
        "invariant-2b: a complete-board item charges the budget nothing"
    );
    assert!(!optimizer.route_work_budget_spent());
}

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
    assert!(work < PORT_OPTIMIZER_ROUTE_WORK_BUDGET);
}
