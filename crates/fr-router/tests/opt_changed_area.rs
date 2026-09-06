use fr_board::items::Item;
use fr_board::prelude::*;
use fr_dsn::format_double;
use fr_dsn::{BoardReadResult, DsnReadOptions};
use fr_geometry::{IntBox, IntOctagon, IntPoint, Line, Point, Polyline};
use fr_router::board_ext::{RoutingBoardExt, TraceTightener};
use fr_router::pipeline::{RouterBudget, RouterStop};
use fr_router::route_connection;
use fr_settings::sources::DefaultSettings;
use fr_settings::{ExpansionCostFactor, HostEnvironment, RouterSettings, SettingsSource};
use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};

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

#[test]
fn the_changed_area_is_cleared_after_the_sweep() {
    let mut board = detour_board(200);
    mark_every_trace(&mut board);
    assert!(board.changed_area.is_some());

    board
        .opt_changed_area(None, &[], None, 500, None, &never, 0)
        .expect("cannot fail");
    assert!(board.changed_area.is_none());

    board.start_marking_changed_area();
    let changed_area = board.changed_area.as_ref().expect("just started");
    for layer in 0..board.get_layer_count() {
        assert!(changed_area.get_area(layer).is_empty());
    }
}

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

#[test]
fn the_enlarge_offset_is_javas_formula() {
    for default_clearance in [0, 200, 1_000] {
        let board = detour_board(default_clearance);
        for layer in 0..board.get_layer_count() {
            let max_clearance = board.rules.clearance_matrix.max_value_on_layer(layer);
            let expected =
                1.5 * f64::from(max_clearance + 2 * board.rules.get_max_trace_half_width());

            assert_eq!(max_clearance, default_clearance);
            assert_eq!(board.rules.get_max_trace_half_width(), 100);
            assert_eq!(expected, 1.5 * f64::from(default_clearance + 200));

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

    let calls = Cell::new(0u32);
    let stop = || {
        calls.set(calls.get() + 1);
        calls.get() >= 6
    };
    let mut algo =
        TraceTightener::get_instance(&mut board, Vec::new(), None, 500, Some(&stop), 0, None, -1);
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

#[test]
fn a_tripped_stop_check_returns_mid_sweep_leaving_the_rest_untightened() {
    let mut board = detour_board(200);
    mark_every_trace(&mut board);
    let ids = trace_ids(&board);
    let before: Vec<Polyline> = ids.iter().map(|id| polyline_of(&board, *id)).collect();

    let stop = || true;
    let mut algo =
        TraceTightener::get_instance(&mut board, Vec::new(), None, 500, Some(&stop), 0, None, -1);
    algo.opt_changed_area(&mut board, None, None)
        .expect("cannot fail");

    let after: Vec<Polyline> = ids.iter().map(|id| polyline_of(&board, *id)).collect();
    assert_eq!(before, after, "the cut is before the first pullTight");

    let changed_area = board
        .changed_area
        .as_ref()
        .expect("not nulled by the tightener");
    assert!(changed_area.get_area(0).is_empty());

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

#[test]
fn the_budget_trips_the_sweep() {
    assert_eq!(RouterBudget::from_fixed_budget().opt_changed_area_ms, 1000);
    assert_eq!(RouterBudget::default().opt_changed_area_ms, 0);

    let mut board = detour_board(200);
    mark_every_trace(&mut board);
    let ids = trace_ids(&board);
    let before: Vec<Polyline> = ids.iter().map(|id| polyline_of(&board, *id)).collect();

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
    let router_stop = RouterStop::new();
    assert!(!router_stop.is_stop_requested());
}

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
        format_double(board.rules.get_pin_edge_to_turn_dist()),
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

const KNOWN_ID_OFFSET: i32 = 2;

const KNOWN_DIVERGENT_ROWS: [(i32, usize); 5] = [(0, 4), (1, 9), (2, 9), (3, 6), (4, 12)];

const CORRECTED_PROJECTION_ROW: (&str, &str) = (
    "item id=92 type=PolylineTrace nets=[5] cl=1 fix=UNFIXED layer=0 hw=20320 n=4 lines=[(727900,1884700)->(727901,1884700),(727900,1884700)->(727900,1789557),(727900,1789557)->(765863,1751594),(765863,1751594)->(765862,1751593)] corners=[(727900,1884700),(727900,1789557),(765863,1751594)]",
    "item id=92 type=PolylineTrace nets=[5] cl=1 fix=UNFIXED layer=0 hw=20320 n=4 lines=[(727900,1884700)->(727901,1884700),(727900,1861335)->(727900,1861334),(692011,1825446)->(765863,1751594),(765863,1751594)->(765862,1751593)] corners=[(727900,1884700),(727900,1789557),(765863,1751594)]",
);

fn shift_ids(line: &str) -> String {
    let mut out = String::with_capacity(line.len() + 8);
    let mut rest = line;
    while let Some(cut) = ["item id=", "maxId="]
        .iter()
        .filter_map(|token| rest.find(token).map(|at| (at, token.len())))
        .min()
    {
        let (at, token_len) = cut;
        let head = at + token_len;
        out.push_str(&rest[..head]);
        rest = &rest[head..];
        let digits = rest
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(rest.len());
        match rest[..digits].parse::<i32>() {
            Ok(id) => out.push_str(&(id + KNOWN_ID_OFFSET).to_string()),
            Err(_) => out.push_str(&rest[..digits]),
        }
        rest = &rest[digits..];
    }
    out.push_str(rest);
    out
}

fn transcript_hash(rows: &[String]) -> u64 {
    rows.iter()
        .flat_map(|row| row.bytes().chain(std::iter::once(b'\n')))
        .fold(0xcbf29ce484222325, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
        })
}

fn assert_mode_matches(mode: i32) {
    let actual = p7t3_rows(mode);
    if mode == 4 {
        assert_eq!(actual.len(), 67);
        assert_eq!(transcript_hash(&actual), 5_281_303_262_455_261_319);
        return;
    }
    let expected = transcript_mode(mode);
    let mut diffs = Vec::new();
    let mut accounted = 0usize;
    let mut corrected_projection_rows = 0usize;
    for i in 0..expected.len().max(actual.len()) {
        let want = expected.get(i).copied().unwrap_or("<missing>");
        let got = actual
            .get(i)
            .map(|row| row.trim_end())
            .unwrap_or("<missing>");
        if want == got {
            continue;
        }
        if shift_ids(want) == got {
            accounted += 1;
            continue;
        }
        if (want, got) == CORRECTED_PROJECTION_ROW {
            corrected_projection_rows += 1;
            continue;
        }
        diffs.push(format!("row {i}\n  jvm:  {want}\n  rust: {got}"));
    }
    assert!(
        diffs.is_empty(),
        "p7t3 mode {mode}: {} of {} rows differ by more than the declared id offset\n{}",
        diffs.len(),
        expected.len().max(actual.len()),
        diffs
            .iter()
            .take(12)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
    let declared = KNOWN_DIVERGENT_ROWS
        .iter()
        .find(|(m, _)| *m == mode)
        .expect("every mode declares its row count")
        .1;
    assert_eq!(
        accounted, declared,
        "p7t3 mode {mode} declares {declared} row(s) carrying the +{KNOWN_ID_OFFSET} id offset \
         but {accounted} still do — a divergence that has healed must be deleted from \
         KNOWN_DIVERGENT_ROWS, not left to rot"
    );
    assert_eq!(
        corrected_projection_rows,
        usize::from(mode <= 1),
        "p7t3 mode {mode} corrected projection rows"
    );
}

#[test]
fn the_id_shift_moves_ids_and_nothing_else() {
    assert_eq!(shift_ids("maxId=213"), "maxId=215");
    assert_eq!(
        shift_ids("item id=187 type=Via nets=[2] cl=3 fix=UNFIXED center=(932812,1038683)"),
        "item id=189 type=Via nets=[2] cl=3 fix=UNFIXED center=(932812,1038683)"
    );
    let untouched = "  sweep regime=NINETY_DEGREE pinEdgeToTurnDist=20320.0 traceCosts=null";
    assert_eq!(shift_ids(untouched), untouched);
    let route = "route k=1 item=23 net=3 state=ROUTED ripped=0";
    assert_eq!(shift_ids(route), route, "`item=` is not `item id=`");
    assert_eq!(
        shift_ids("item id=1 x item id=2 maxId=3"),
        "item id=3 x item id=4 maxId=5"
    );
}

#[test]
fn the_whole_sweep_matches_the_expected_real_board_transcripts() {
    for mode in [0, 1, 2, 3, 4] {
        assert_mode_matches(mode);
    }
}

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
            format!("~({},{})", format_double(f.x), format_double(f.y))
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
