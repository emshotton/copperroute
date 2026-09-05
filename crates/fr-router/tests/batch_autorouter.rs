use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

use fr_board::StopConnectionOption;
use fr_board::items::Item;
use fr_board::prelude::*;
use fr_dsn::{BoardReadResult, DsnReadOptions};
use fr_geometry::{IntBox, IntOctagon, IntPoint, Point, Polyline, Shape, TileShape};
use fr_router::autoroute::maze::ViaPricing;
use fr_router::board_ext::RoutingBoardExt;
use fr_router::pipeline::{BatchAutorouter, NamedAlgorithmType, RouterBudget};
use fr_router::{AutorouteAttemptState, AutorouteEngine, route_connection, route_connection_full};
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

fn never() -> bool {
    false
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
            ViaPricing::ByPadstackRadius,
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
    assert_eq!(
        fr_router::BoardHistory::MAX_HISTORY_SIZE,
        BatchAutorouter::BOARD_RANK_LIMIT,
        "BOARD_RANK_LIMIT = BoardHistory.MAX_HISTORY_SIZE (:40)"
    );
    assert_eq!("freerouting-router", BatchAutorouter::ID);
    assert_eq!("Freerouting Auto-router", BatchAutorouter::NAME);
    assert_eq!("1.0", BatchAutorouter::VERSION);
    assert_eq!("Freerouting Auto-router v1.0", BatchAutorouter::DESCRIPTION);
    assert_eq!(NamedAlgorithmType::Router, BatchAutorouter::TYPE);
}

#[test]
fn retain_autoroute_database_has_no_setter() {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut offenders = Vec::new();
    for entry in walk(&src) {
        let text = std::fs::read_to_string(&entry).expect("readable source");
        for (no, line) in text.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") {
                continue;
            }
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

#[test]
fn should_fire_board_update_uses_the_budget() {
    let board = empty_board(200, AngleRestriction::None);
    let settings = RouterSettings::new();

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

    let budget = RouterBudget {
        board_update_throttle_ms: 1000,
        ..RouterBudget::default()
    };
    let slow = BatchAutorouter::new(&board, &settings, false, true, 100, 500, budget);
    assert!(slow.should_fire_board_update_at(t0));
    assert!(!slow.should_fire_board_update_at(t0 + Duration::from_millis(999)));
    assert!(slow.should_fire_board_update_at(t0 + Duration::from_millis(1001)));
}

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

#[test]
fn remove_tails_pulls_the_marked_area_tight() {
    if !parity::require_java_dir() {
        return;
    }
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
    assert_eq!(500, router.get_trace_pull_tight_accuracy(), ":118-120");
    assert_eq!(
        board.get_layer_count(),
        router.get_trace_costs().len(),
        ":139 — one ExpansionCostFactor per layer"
    );
}

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

#[test]
fn remove_items_and_pull_tight_combines_traces_in_ascending_net_order() {
    let mut board = empty_board(200, AngleRestriction::None);
    add_net(&mut board, "N7", 7);
    add_net(&mut board, "N3", 3);

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

    let mut probe = board.clone();
    let (_removed_all, changed_nets) = probe.remove_items_marking_changed_area(vec![victim]);
    assert_eq!(
        vec![3, 7],
        changed_nets.into_iter().collect::<Vec<_>>(),
        ":93's TreeSet<Integer> is ascending — net 3 is combined before net 7"
    );

    let ok = board
        .remove_items_and_pull_tight(None, &[victim], 0, 500)
        .expect("cannot fail here");
    assert!(ok, "nothing on this board is fixed, so the answer is true");
    assert!(board.get_item(victim).is_none());
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

#[test]
fn a_non_positive_tidy_width_skips_the_tightener_entirely() {
    let build = || {
        let mut board = empty_board(200, AngleRestriction::None);
        add_net(&mut board, "N1", 1);
        insert_trace(
            &mut board,
            &[p(-3000, -3000), p(-3000, -1000), p(-1000, -1000)],
            0,
            30,
            1,
        );
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

#[test]
fn remove_items_and_pull_tight_hands_the_tightener_a_live_clip_octagon() {
    let build = || {
        let mut board = empty_board(200, AngleRestriction::FortyFiveDegree);
        add_net(&mut board, "N1", 1);
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
        let victim = insert_trace(&mut board, &[p(-4000, -3000), p(-3800, -3000)], 0, 30, 1);
        (board, staircase, victim)
    };

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

const QUIRK_184_STAIRCASE_AS_INSERTED: &[(i32, i32)] = &[
    (-6000, -6000),
    (-6000, -4000),
    (-4000, -4000),
    (-4000, -2000),
    (-2000, -2000),
    (-2000, 0),
    (0, 0),
];

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

const QUIRK_184_UNCLIPPED_STAIRCASE: &[(i32, i32)] = &[(-6000, -6000), (0, 0)];

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
            ViaPricing::ByPadstackRadius,
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

    let first = route(&mut board, &mut engine, 0);
    assert_eq!(AutorouteAttemptState::Routed, first.state);
    assert!(
        board.changed_area.is_none(),
        ":103-109's optChangedArea must have run and cleared the area"
    );

    let second = route(&mut board, &mut engine, 1);
    assert_eq!(AutorouteAttemptState::Failed, second.state);
    assert!(
        board.changed_area.is_some(),
        "a FAILED connection must leave its marked area for the next one — the `if state == \
         ROUTED` guard at :95 is the only thing that can"
    );
}

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

#[test]
fn the_necked_retry_gets_a_fresh_time_limit() {
    if !parity::require_java_dir() {
        return;
    }
    let (_board, engine, ids, call_start, call_duration) = route_prefix(2, 100.0);
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
    let offset = deadline.duration_since(call_start);
    let slack = offset - Duration::from_millis(100_000);
    assert!(
        slack > call_duration / 2,
        "#208: the retry's TimeLimit must be minted at `:209`, not inherited from `route:74` — \
         its deadline is only {slack:?} past `call_start + 100 s`, and the failed first attempt \
         alone accounts for most of this connection's {call_duration:?}"
    );
}

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
    let (routed, _, ids, _, _) = route_prefix(1, 0.0);
    assert_eq!(vec![63], ids);
    assert!(
        routed.communication.id_gen.max_generated_id().0 == 63,
        "nothing was rolled back"
    );
}

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
        vec![(c1, 1), (c2, 2), (c3, 3)],
        "R1 (#293): the work list is ascending by calculateItemDistance. Before the fix this \
         answered [c3, c2, c1] — the descending-id walk (quirk #63), which is exactly the \
         reverse here."
    );
}

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
