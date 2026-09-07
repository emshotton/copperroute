use copper_board::items::Item;
use copper_board::prelude::*;
use copper_dsn::{BoardReadResult, DsnReadOptions};
use copper_geometry::{IntPoint, Point, Polyline};
use copper_router::board_ext::ViaOptimizer;
use copper_router::route_connection;
use copper_settings::sources::DefaultSettings;
use copper_settings::{ExpansionCostFactor, HostEnvironment, RouterSettings, SettingsSource};
use std::collections::{BTreeMap, BTreeSet};

fn never() -> bool {
    false
}

const TRANSCRIPT: &str = include_str!("data/p7t4-via-optimizer.txt");
const TASK_16_GOLDEN: &str = include_str!("data/p9t16-via-optimizer.txt");

fn section<'a>(transcript: &'a str, name: &str) -> Vec<&'a str> {
    let header = format!("######## {name}");
    let mut rows = Vec::new();
    let mut inside = false;
    for line in transcript.lines() {
        if line.starts_with("######## ") {
            inside = line == header;
            continue;
        }
        if inside {
            rows.push(line.trim_end());
        }
    }
    assert!(!rows.is_empty(), "transcript section {name} is empty");
    rows
}

fn fixture_of(tag: &str) -> &'static str {
    match tag {
        "rpi" => "Issue143-rpi_splitter.dsn",
        "j2" => "Issue026-J2_reference.dsn",
        other => panic!("unknown transcript tag {other}"),
    }
}

fn overload_rows(tag: &str, mode: i32) -> Vec<String> {
    let mut board = routed(fixture_of(tag));
    let via_ids: Vec<ItemId> = board
        .get_items()
        .filter(|item| matches!(item, Item::Via(_)))
        .map(Item::id)
        .collect();
    let mut out = Vec::new();
    for via_id in via_ids {
        match mode {
            3 => rows_a(&mut out, &mut board, via_id),
            4 => rows_b(&mut out, &mut board, via_id),
            5 => rows_c(&mut out, &mut board, via_id),
            other => panic!("mode {other} is not an overload mode"),
        }
    }
    out
}

fn rows_a(out: &mut Vec<String>, board: &mut Board, via_id: ItemId) {
    let traces = trace_contacts(board, via_id);
    let Some(trace) = traces.first().copied() else {
        out.push(format!("repA id={} state=NO_TRACE_CONTACT", via_id.0));
        return;
    };
    let center = center_of(board, via_id).to_float().round();
    let (hw, cl, layer) = trace_params(board, trace);
    let targets = targets_a(center, &polyline_of(board, trace));
    for (k, to) in targets.iter().enumerate() {
        let answer = ViaOptimizer::reposition_via_toward_location(board, via_id, to, hw, layer, cl);
        out.push(format!(
            "repA id={} k={k} to=({},{}) hw={hw} layer={layer} cl={cl} -> {}",
            via_id.0,
            to.x,
            to.y,
            answer.map_or_else(|| "null".to_string(), |p| dump_point(&p))
        ));
    }
}

fn rows_b(out: &mut Vec<String>, board: &mut Board, via_id: ItemId) {
    let traces = trace_contacts(board, via_id);
    let Some(t1) = traces.first().copied() else {
        out.push(format!("repB id={} state=NO_TRACE_CONTACT", via_id.0));
        return;
    };
    let t2 = traces.get(1).copied().unwrap_or(t1);
    let via_center = center_of(board, via_id);
    let center = via_center.to_float().round();
    let c1 = from_corner_of(board, t1, &via_center).to_float().round();
    let c2 = from_corner_of(board, t2, &via_center).to_float().round();
    let params1 = trace_params(board, t1);
    let params2 = trace_params(board, t2);
    for (k, to) in targets_b(center, c1, c2).iter().enumerate() {
        for r in 0..2 {
            let (moved_hw, moved_cl, moved_layer) = if r == 0 { params2 } else { params1 };
            let (conn_hw, conn_cl, conn_layer) = if r == 0 { params1 } else { params2 };
            let connect = if r == 0 { c1 } else { c2 };
            let answer = ViaOptimizer::reposition_via_check_candidate(
                board,
                via_id,
                to,
                moved_hw,
                moved_layer,
                moved_cl,
                &connect,
                conn_hw,
                conn_layer,
                conn_cl,
            );
            out.push(format!(
                "repB id={} k={k} r={r} to=({},{}) connect=({},{}) -> {answer}",
                via_id.0, to.x, to.y, connect.x, connect.y
            ));
        }
    }
}

fn rows_c(out: &mut Vec<String>, board: &mut Board, via_id: ItemId) {
    let class = classify(board, via_id);
    if class != "TWO_TRACES" {
        out.push(format!("repC id={} class={class} state=SKIP", via_id.0));
        return;
    }
    let (t1, t2, c1, c2) = overload_c_arguments(board, via_id);
    let (hw1, cl1, layer1) = trace_params(board, t1);
    let (hw2, cl2, layer2) = trace_params(board, t2);
    for (k, pair) in COST_PAIRS.iter().enumerate() {
        let answer = ViaOptimizer::reposition_via_general(
            board,
            via_id,
            hw1,
            cl1,
            layer1,
            ExpansionCostFactor {
                horizontal: pair[0],
                vertical: pair[1],
            },
            &c1,
            hw2,
            cl2,
            layer2,
            ExpansionCostFactor {
                horizontal: pair[2],
                vertical: pair[3],
            },
            &c2,
        );
        out.push(format!(
            "repC id={} k={k} costs1=({},{}) costs2=({},{}) from1={} from2={} -> {}",
            via_id.0,
            copper_dsn::format_double(pair[0]),
            copper_dsn::format_double(pair[1]),
            copper_dsn::format_double(pair[2]),
            copper_dsn::format_double(pair[3]),
            dump_point(&c1),
            dump_point(&c2),
            answer.map_or_else(|| "null".to_string(), |p| dump_point(&p))
        ));
    }
}

fn transcript_overload_rows(tag: &str, mode: i32, prefix: &str) -> Vec<String> {
    let transcript = if mode == 3 {
        TRANSCRIPT
    } else {
        TASK_16_GOLDEN
    };
    section(transcript, &format!("{tag} mode {mode}"))
        .into_iter()
        .filter(|row| row.starts_with(prefix))
        .map(str::to_string)
        .collect()
}

const ID_KEYS: [&str; 7] = [
    "via id=",
    "item id=",
    "maxId=",
    "repA id=",
    "repB id=",
    "repC id=",
    "contacts=[",
];

fn split_ids(line: &str) -> (Vec<i64>, String) {
    let mut ids = Vec::new();
    let mut blanked = String::with_capacity(line.len());
    let mut rest = line;
    loop {
        let Some((at, key_len)) = ID_KEYS
            .iter()
            .filter_map(|key| rest.find(key).map(|at| (at, key.len())))
            .min_by_key(|(at, _)| *at)
        else {
            break;
        };
        let head = at + key_len;
        blanked.push_str(&rest[..head]);
        rest = &rest[head..];
        loop {
            let digits = rest
                .find(|c: char| !c.is_ascii_digit())
                .unwrap_or(rest.len());
            let Ok(id) = rest[..digits].parse::<i64>() else {
                break;
            };
            ids.push(id);
            blanked.push('#');
            rest = &rest[digits..];
            let Some(comma) = rest.find(',') else { break };
            if rest[..comma].contains(']') || rest[..comma].contains(' ') {
                break;
            }
            blanked.push_str(&rest[..=comma]);
            rest = &rest[comma + 1..];
        }
    }
    blanked.push_str(rest);
    (ids, blanked)
}

fn assert_rows_match_up_to_one_renaming(label: &str, ours: &[String], theirs: &[String]) {
    let blank = |rows: &[String]| -> (Vec<Vec<i64>>, Vec<String>) {
        rows.iter().map(|row| split_ids(row)).unzip()
    };
    let (our_ids, our_blanked) = blank(ours);
    let (their_ids, their_blanked) = blank(theirs);
    assert_eq!(
        our_blanked, their_blanked,
        "{label}: a difference outside the board item ids"
    );

    let mut forward: BTreeMap<i64, i64> = BTreeMap::new();
    let mut backward: BTreeMap<i64, i64> = BTreeMap::new();
    for (row, (theirs_row, ours_row)) in their_ids.iter().zip(&our_ids).enumerate() {
        assert_eq!(theirs_row.len(), ours_row.len(), "{label} row {row}");
        for (&jar, &port) in theirs_row.iter().zip(ours_row) {
            if let Some(&seen) = forward.get(&jar) {
                assert_eq!(
                    seen, port,
                    "{label} row {row}: the jar's id {jar} is the port's {seen} elsewhere and \
                     {port} here — not one renaming"
                );
            }
            if let Some(&seen) = backward.get(&port) {
                assert_eq!(
                    seen, jar,
                    "{label} row {row}: the port's id {port} stands for the jar's {seen} \
                     elsewhere and {jar} here — not one renaming"
                );
            }
            forward.insert(jar, port);
            backward.insert(port, jar);
        }
    }
    assert!(
        !forward.is_empty(),
        "{label}: no id was compared, so the renaming check did nothing"
    );
}

fn assert_overload_rows(tag: &str, mode: i32, prefix: &str) {
    assert_rows_match_up_to_one_renaming(
        &format!("p7t4 {tag} mode {mode}"),
        &overload_rows(tag, mode),
        &transcript_overload_rows(tag, mode, prefix),
    );
}

#[test]
fn overload_a_matches_the_jvm_on_every_scripted_target() {
    assert_overload_rows("rpi", 3, "repA ");
}

#[test]
fn overload_b_matches_the_task_16_golden_on_every_scripted_candidate() {
    assert_overload_rows("rpi", 4, "repB ");
}

#[test]
fn overload_c_matches_the_task_16_golden_on_every_cost_pair() {
    assert_overload_rows("rpi", 5, "repC ");
}

#[test]
fn a_one_contact_via_takes_overload_a() {
    let board = routed("Issue143-rpi_splitter.dsn");
    for (via_id, overload_a_answer, final_center) in [
        (
            ItemId(189),
            IntPoint::new(932_812, 1_011_224),
            IntPoint::new(932_812, 1_011_224),
        ),
        (
            ItemId(84),
            IntPoint::new(1_016_000, 3_007_058),
            IntPoint::new(1_016_000, 3_119_161),
        ),
    ] {
        assert_eq!(
            board.normal_contacts(via_id).len(),
            1,
            "via {} is the one-contact shape",
            via_id.0
        );

        let mut scratch = board.clone();
        let trace = trace_contacts(&scratch, via_id)[0];
        let via_center = center_of(&scratch, via_id);
        let check_corner = from_corner_of(&scratch, trace, &via_center);
        let (hw, cl, layer) = trace_params(&scratch, trace);
        let before = scratch.structural_hash();
        let direct = ViaOptimizer::reposition_via_toward_location(
            &mut scratch,
            via_id,
            &check_corner.to_float().round(),
            hw,
            layer,
            cl,
        );
        assert_eq!(
            direct,
            Some(Point::Int(overload_a_answer)),
            "overload A on via {}",
            via_id.0
        );
        assert_eq!(
            before,
            scratch.structural_hash(),
            "overload A mutates nothing"
        );

        for label in ["opt_via_location", "opt_plane_or_fanout_via"] {
            let mut scratch = board.clone();
            let moved = if label == "opt_via_location" {
                ViaOptimizer::opt_via_location(&mut scratch, via_id, None, 500, 10)
            } else {
                ViaOptimizer::opt_plane_or_fanout_via(&mut scratch, via_id, 500, 10)
            }
            .expect("cannot fail");
            assert!(moved, "{label} on via {}", via_id.0);
            assert_eq!(
                scratch.drill_center(via_id).expect("still a via"),
                Point::Int(final_center),
                "{label} on via {}",
                via_id.0
            );
        }
    }
}

#[test]
fn overload_b_is_reached_only_from_c() {
    const SOURCE: &str = include_str!("../src/board_ext/via_optimizer.rs");
    let calls = SOURCE
        .matches("Self::reposition_via_check_candidate(")
        .count();
    assert_eq!(
        calls, 4,
        "overload B is called exactly four times, all from overload C's decomposition arms"
    );
    let a_calls = SOURCE
        .matches("Self::reposition_via_toward_location(")
        .count();
    assert_eq!(
        a_calls, 9,
        "overload A is called once from the plane/fanout arm and eight times from overload C"
    );
}

#[test]
fn the_general_case_leaves_the_board_untouched_when_no_candidate_improves() {
    let mut board = routed("Issue026-J2_reference.dsn");
    let via_ids: Vec<ItemId> = board
        .get_items()
        .filter(|item| matches!(item, Item::Via(_)))
        .map(Item::id)
        .collect();
    let mut refusals = 0;
    let mut successes = 0;
    for via_id in via_ids {
        if classify(&board, via_id) != "TWO_TRACES" {
            continue;
        }
        let (t1, t2, c1, c2) = overload_c_arguments(&board, via_id);
        let (hw1, cl1, layer1) = trace_params(&board, t1);
        let (hw2, cl2, layer2) = trace_params(&board, t2);
        for pair in &COST_PAIRS {
            let before = board.structural_hash();
            let answer = ViaOptimizer::reposition_via_general(
                &mut board,
                via_id,
                hw1,
                cl1,
                layer1,
                ExpansionCostFactor {
                    horizontal: pair[0],
                    vertical: pair[1],
                },
                &c1,
                hw2,
                cl2,
                layer2,
                ExpansionCostFactor {
                    horizontal: pair[2],
                    vertical: pair[3],
                },
                &c2,
            );
            assert_eq!(
                before,
                board.structural_hash(),
                "overload C mutated the board at via {}",
                via_id.0
            );
            if answer.is_none() {
                refusals += 1;
            } else {
                successes += 1;
            }
            if via_id == ItemId(124) {
                assert_eq!(answer, None, "via 124 refuses every cost pair");
            }
        }
    }
    assert!(refusals > 0, "the fall-through at :712 is exercised");
    assert!(successes > 0, "and so is the success path");
}

#[test]
fn the_general_case_never_returns_a_diagonal_move_under_ninety_degree_restriction() {
    let mut board = routed("Issue026-J2_reference.dsn");
    board.rules.trace_angle_restriction = AngleRestriction::NinetyDegree;
    let via_ids: Vec<ItemId> = board
        .get_items()
        .filter(|item| matches!(item, Item::Via(_)))
        .map(Item::id)
        .collect();
    let mut checked = 0;
    for via_id in via_ids {
        if classify(&board, via_id) != "TWO_TRACES" {
            continue;
        }
        let (t1, t2, c1, c2) = overload_c_arguments(&board, via_id);
        let (hw1, cl1, layer1) = trace_params(&board, t1);
        let (hw2, cl2, layer2) = trace_params(&board, t2);
        let Some(via_center) = board.drill_center(via_id) else {
            continue;
        };
        for pair in &COST_PAIRS {
            let answer = ViaOptimizer::reposition_via_general(
                &mut board,
                via_id,
                hw1,
                cl1,
                layer1,
                ExpansionCostFactor {
                    horizontal: pair[0],
                    vertical: pair[1],
                },
                &c1,
                hw2,
                cl2,
                layer2,
                ExpansionCostFactor {
                    horizontal: pair[2],
                    vertical: pair[3],
                },
                &c2,
            );
            checked += 1;
            if let Some(new_location) = answer {
                let delta = new_location.difference_by(&via_center);
                assert!(
                    delta.is_orthogonal(),
                    "via {} moved to {new_location:?} (delta {delta:?}), which is not \
                     orthogonal, under a NinetyDegree restriction",
                    via_id.0
                );
            }
        }
    }
    assert!(
        checked > 0,
        "the fixture has no two-trace via to exercise overload C"
    );
}

const COST_PAIRS: [[f64; 4]; 5] = [
    [1.0, 1.0, 1.0, 1.0],
    [1.0, 2.0, 2.0, 1.0],
    [2.0, 1.0, 1.0, 2.0],
    [1.0, 1.0, 2.0, 2.0],
    [3.0, 1.0, 1.0, 3.0],
];

fn targets_a(center: IntPoint, polyline: &Polyline) -> Vec<IntPoint> {
    let mut result = vec![
        polyline.corner(1).expect("two corners").to_float().round(),
        polyline
            .corner(polyline.corner_count() - 2)
            .expect("two corners")
            .to_float()
            .round(),
        center,
    ];
    for d in [1, 1000, 10_000, 100_000] {
        result.push(IntPoint::new(center.x + d, center.y));
        result.push(IntPoint::new(center.x, center.y + d));
        result.push(IntPoint::new(center.x + d, center.y + d));
        result.push(IntPoint::new(center.x - d, center.y));
        result.push(IntPoint::new(center.x, center.y - d));
        result.push(IntPoint::new(center.x - d, center.y - d));
    }
    result
}

fn targets_b(center: IntPoint, c1: IntPoint, c2: IntPoint) -> Vec<IntPoint> {
    vec![
        IntPoint::new(center.x, c1.y),
        IntPoint::new(c1.x, center.y),
        IntPoint::new(center.x, c2.y),
        IntPoint::new(c2.x, center.y),
        c1,
        c2,
        center,
        IntPoint::new(center.x + 1, center.y),
        IntPoint::new(center.x + 1, center.y + 1),
        IntPoint::new(center.x + 1000, center.y + 1000),
        IntPoint::new(center.x - 1000, center.y),
    ]
}

fn overload_c_arguments(board: &Board, via: ItemId) -> (ItemId, ItemId, Point, Point) {
    let traces = trace_contacts(board, via);
    let via_center = center_of(board, via);
    let c1 = from_corner_of(board, traces[0], &via_center);
    let c2 = from_corner_of(board, traces[1], &via_center);
    (traces[0], traces[1], c1, c2)
}

fn trace_params(board: &Board, trace: ItemId) -> (i32, usize, usize) {
    (
        half_width_of(board, trace),
        clearance_of(board, trace),
        layer_of(board, trace),
    )
}

fn trace_contacts(board: &Board, via: ItemId) -> Vec<ItemId> {
    board
        .normal_contacts(via)
        .into_iter()
        .rev()
        .filter(|id| board.get_item(*id).is_some_and(Item::is_trace))
        .collect()
}

fn polyline_of(board: &Board, trace: ItemId) -> Polyline {
    match board.get_item(trace) {
        Some(Item::Trace(t)) => t.polyline().clone(),
        _ => panic!("a trace contact"),
    }
}

fn half_width_of(board: &Board, trace: ItemId) -> i32 {
    match board.get_item(trace) {
        Some(Item::Trace(t)) => t.get_half_width(),
        _ => panic!("a trace contact"),
    }
}

fn layer_of(board: &Board, trace: ItemId) -> usize {
    match board.get_item(trace) {
        Some(Item::Trace(t)) => t.get_layer(),
        _ => panic!("a trace contact"),
    }
}

fn clearance_of(board: &Board, trace: ItemId) -> usize {
    board
        .get_item(trace)
        .expect("a trace contact")
        .clearance_class()
}

fn center_of(board: &Board, via: ItemId) -> Point {
    board.drill_center(via).expect("a via has a centre")
}

fn from_corner_of(board: &Board, trace: ItemId, via_center: &Point) -> Point {
    let polyline = polyline_of(board, trace);
    let (first, last) = match board.get_item(trace) {
        Some(Item::Trace(t)) => (t.first_corner(), t.last_corner()),
        _ => panic!("a trace contact"),
    };
    if first.as_ref() == Some(via_center) {
        return polyline.corner(1).expect("two corners");
    }
    if last.as_ref() == Some(via_center) {
        return polyline
            .corner(polyline.corner_count() - 2)
            .expect("two corners");
    }
    polyline.corner(1).expect("two corners")
}

fn classify(board: &Board, via: ItemId) -> &'static str {
    let item = board.get_item(via).expect("a via");
    if item.is_shove_fixed(&board.rules) {
        return "SHOVE_FIXED";
    }
    let contacts: Vec<ItemId> = board.normal_contacts(via).into_iter().rev().collect();
    if contacts.len() == 1 {
        return "PLANE_OR_FANOUT_ONE_CONTACT";
    }
    if contacts.len() != 2 {
        return "WRONG_CONTACT_COUNT";
    }
    let mut traces: Vec<ItemId> = Vec::new();
    let mut is_plane_or_fanout_via = false;
    for contact in contacts {
        let Some(contact_item) = board.get_item(contact) else {
            return "UNUSABLE_CONTACT";
        };
        if contact_item.is_shove_fixed(&board.rules) || !contact_item.is_trace() {
            if matches!(contact_item, Item::ConductionArea(_)) {
                is_plane_or_fanout_via = true;
            } else {
                return "UNUSABLE_CONTACT";
            }
        } else {
            traces.push(contact);
        }
    }
    if is_plane_or_fanout_via {
        return "PLANE_OR_FANOUT_CONDUCTION";
    }
    let via_center = center_of(board, via);
    for trace in traces {
        let Some(Item::Trace(t)) = board.get_item(trace) else {
            return "UNUSABLE_CONTACT";
        };
        let (first, last) = (t.first_corner(), t.last_corner());
        if first.as_ref() != Some(&via_center) && last.as_ref() != Some(&via_center) {
            return "NOT_AT_ENDPOINT";
        }
    }
    "TWO_TRACES"
}

fn dump_point(point: &Point) -> String {
    let rounded = point.to_float().round();
    format!("({},{})", rounded.x, rounded.y)
}

fn routed(design_name: &str) -> Board {
    let path = parity::fixture(design_name);
    let file = std::fs::File::open(&path)
        .unwrap_or_else(|e| panic!("cannot open {}: {e}", path.display()));
    let mut board =
        match copper_dsn::read_board(file, None, Some(design_name), &DsnReadOptions::default()) {
            BoardReadResult::Success { board, .. }
            | BoardReadResult::OutlineMissing { board, .. } => {
                *board.expect("the fixture produces a board")
            }
            other => panic!("{design_name} did not read: {other:?}"),
        };
    let settings = build_settings(&board);
    for (item_id, net_no) in pick_connections(&board, 12) {
        if board.get_item(item_id).is_none() {
            continue;
        }
        board.start_marking_changed_area();
        let mut ripped: BTreeSet<ItemId> = BTreeSet::new();
        let mut ripup_costs: BTreeMap<ItemId, i32> = BTreeMap::new();
        let trace_costs = settings.get_trace_costs();
        let mut engine = None;
        route_connection(
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
    }
    board
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
