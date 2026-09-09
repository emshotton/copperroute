use copper_board::items::Item;
use copper_board::prelude::*;
use copper_dsn::format_double;
use copper_dsn::{BoardReadResult, DsnReadOptions};
use copper_geometry::{IntBox, IntPoint, Line, Point, Polyline, Shape, TileShape};
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
        "ecc83" => "Issue649-kicad_ecc83-pp_input_board_v1.dsn",
        other => panic!("unknown transcript tag {other}"),
    }
}

fn p7t4_rows(tag: &str, mode: i32) -> Vec<String> {
    let mut out = Vec::new();
    let design_name = fixture_of(tag);
    let path = testkit::fixture(design_name);
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

    let trace_costs: Option<Vec<ExpansionCostFactor>> = if mode == 6 {
        None
    } else {
        Some(vec![
            ExpansionCostFactor {
                horizontal: 1.0,
                vertical: 1.0,
            };
            board.get_layer_count()
        ])
    };
    out.push(format!(
        "sweep regime={} traceCosts={}",
        regime_name(board.rules.trace_angle_restriction),
        match &trace_costs {
            Some(costs) => costs.len().to_string(),
            None => "null".to_string(),
        }
    ));

    let via_ids: Vec<ItemId> = board
        .get_items()
        .filter(|item| matches!(item, Item::Via(_)))
        .map(Item::id)
        .collect();
    out.push(format!("vias n={}", via_ids.len()));

    for via_id in via_ids {
        if !matches!(board.get_item(via_id), Some(Item::Via(_))) {
            out.push(format!("via id={} state=GONE", via_id.0));
            continue;
        }
        let center = board.drill_center(via_id).expect("a via has a centre");
        let min_width = match board.get_item(via_id) {
            Some(Item::Via(v)) => v.min_width(&board.ctx()),
            _ => unreachable!("just matched"),
        };
        let contacts = contact_ids(&board, via_id);
        let class = classify(&board, via_id);
        let result = if mode == 1 {
            ViaOptimizer::opt_plane_or_fanout_via(&mut board, via_id, 500, 10)
        } else {
            ViaOptimizer::opt_via_location(&mut board, via_id, trace_costs.as_deref(), 500, 10)
        }
        .expect("ViaOptimizer cannot fail in Java");
        let after = match board.drill_center(via_id) {
            Some(point) if matches!(board.get_item(via_id), Some(Item::Via(_))) => {
                dump_point(&point)
            }
            _ => "gone".to_string(),
        };
        out.push(format!(
            "via id={} center={} minWidth={} contacts={contacts} class={class} result={result} after={after}",
            via_id.0,
            dump_point(&center),
            format_double(min_width),
        ));
    }

    out.extend(dump_board(&board));
    out
}

const ID_KEYS: [&str; 4] = ["via id=", "item id=", "maxId=", "contacts=["];

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

fn assert_rows_match_up_to_one_renaming(label: &str, ours: &[String], theirs: &[&str]) {
    let (our_ids, our_blanked): (Vec<Vec<i64>>, Vec<String>) =
        ours.iter().map(|row| split_ids(row)).unzip();
    let (their_ids, their_blanked): (Vec<Vec<i64>>, Vec<String>) =
        theirs.iter().map(|row| split_ids(row)).unzip();
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

#[test]
fn the_id_split_finds_ids_and_nothing_else() {
    assert_eq!(split_ids("maxId=213"), (vec![213], "maxId=#".to_string()));
    assert_eq!(
        split_ids("via id=264 center=(1328495,-867516) contacts=[266:PolylineTrace,263:PolylineTrace] class=TWO_TRACES"),
        (
            vec![264, 266, 263],
            "via id=# center=(1328495,-867516) contacts=[#:PolylineTrace,#:PolylineTrace] class=TWO_TRACES".to_string()
        )
    );
    let untouched = "route k=1 item=23 net=3 state=ROUTED ripped=0";
    assert_eq!(
        split_ids(untouched),
        (Vec::new(), untouched.to_string()),
        "`item=` is not `item id=`"
    );
}

#[test]
fn the_corrected_runs_match_the_task_16_golden_row_for_row() {
    for (tag, mode) in [("ecc83", 0), ("ecc83", 1), ("ecc83", 6)] {
        assert_eq!(
            p7t4_rows(tag, mode),
            section(TASK_16_GOLDEN, &format!("{tag} mode {mode}")),
            "p9t16 {tag} mode {mode}: still byte for byte"
        );
    }
    for (tag, mode) in [("rpi", 0), ("rpi", 1), ("rpi", 6)] {
        assert_rows_match_up_to_one_renaming(
            &format!("p7t4 {tag} mode {mode}"),
            &p7t4_rows(tag, mode),
            &section(TASK_16_GOLDEN, &format!("{tag} mode {mode}")),
        );
    }
}

fn formerly_divergent_vias(tag: &str, mode: i32) -> &'static [u32] {
    match (tag, mode) {
        ("rpi", 0) | ("rpi", 6) => &[187, 84],
        _ => &[],
    }
}

#[test]
fn the_task_16_golden_records_the_jvm_divergence() {
    for (tag, mode) in [("rpi", 0), ("rpi", 6), ("ecc83", 0)] {
        let ours = p7t4_rows(tag, mode);
        let golden = section(TASK_16_GOLDEN, &format!("{tag} mode {mode}"));
        assert_rows_match_up_to_one_renaming(&format!("p9t16 {tag} mode {mode}"), &ours, &golden);
        let theirs = section(TRANSCRIPT, &format!("{tag} mode {mode}"));
        assert_ne!(
            ours.iter().map(|row| split_ids(row).1).collect::<Vec<_>>(),
            theirs
                .iter()
                .map(|row| split_ids(row).1)
                .collect::<Vec<_>>(),
            "{tag} mode {mode}: the corrected behavior must remain distinct from the JVM"
        );

        let our_vias: Vec<&String> = ours.iter().filter(|r| r.starts_with("via ")).collect();
        let their_vias: Vec<&&str> = theirs.iter().filter(|r| r.starts_with("via ")).collect();
        assert_eq!(
            our_vias.len(),
            their_vias.len(),
            "{tag} mode {mode}: via row count"
        );
        let mut checked = Vec::new();
        for (ours_row, theirs_row) in our_vias.iter().zip(&their_vias) {
            let id = via_id_of(theirs_row);
            if !formerly_divergent_vias(tag, mode).contains(&id) {
                continue;
            }
            checked.push(id);
            assert_eq!(
                split_ids(ours_row).1,
                drop_trailing_zero_decimals(&split_ids(theirs_row).1)
            );
        }
        assert_eq!(
            checked,
            formerly_divergent_vias(tag, mode),
            "{tag} mode {mode}: every formerly divergent via is still in the run"
        );
    }
}

/// Drops a bare `.0` after a run of digits, so a row carrying the jar's `Double.toString` style
/// (`minWidth=75640.0`) compares equal to the idiomatic formatter's (`minWidth=75640`).
fn drop_trailing_zero_decimals(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'.'
            && i + 1 < bytes.len()
            && bytes[i + 1] == b'0'
            && i > 0
            && bytes[i - 1].is_ascii_digit()
            && (i + 2 == bytes.len() || !bytes[i + 2].is_ascii_digit())
        {
            i += 2;
            continue;
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

fn via_id_of(row: &str) -> u32 {
    column(row, "id=")
        .parse()
        .unwrap_or_else(|_| panic!("an id in {row}"))
}

fn column<'a>(row: &'a str, key: &str) -> &'a str {
    let start = row.find(key).unwrap_or_else(|| panic!("{key} in {row}")) + key.len();
    let rest = &row[start..];
    match rest.find(' ') {
        Some(end) => &rest[..end],
        None => rest,
    }
}

const BOUNDING_BOX: IntBox = IntBox {
    ll: IntPoint {
        x: -100_000,
        y: -100_000,
    },
    ur: IntPoint {
        x: 100_000,
        y: 100_000,
    },
};

fn layers() -> LayerStructure {
    LayerStructure::new(vec![Layer::new("front", true), Layer::new("back", true)])
}

fn empty_board() -> Board {
    let clearance_matrix = ClearanceMatrix::get_default_instance(&layers(), 100);
    let mut rules = BoardRules::new(layers(), clearance_matrix);
    rules.trace_angle_restriction = AngleRestriction::None;
    let mut padstacks = Padstacks::new(layers());
    let shape = Shape::Tile(TileShape::Box(IntBox::from_coords(-20, -20, 20, 20)));
    padstacks.add("via", vec![Some(shape.clone()), Some(shape)], true, false);
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
    board.rules.nets.add("N1", 1, false, default_class);
    board
}

#[test]
fn reposition_via_respects_the_boards_angle_restriction() {
    let mut board = empty_board();
    board.rules.trace_angle_restriction = AngleRestriction::NinetyDegree;
    let via = board
        .insert_via(
            copper_board::PadstackId(1),
            Point::new(0, 0),
            vec![1],
            1,
            FixedState::Unfixed,
            false,
        )
        .expect("a via");

    assert_eq!(
        ViaOptimizer::reposition_via_toward_location(
            &mut board,
            via,
            &IntPoint::new(100, 50),
            1,
            0,
            1,
        ),
        None
    );
    assert!(!ViaOptimizer::reposition_via_check_candidate(
        &mut board,
        via,
        &IntPoint::new(100, 50),
        1,
        0,
        1,
        &IntPoint::new(100, 100),
        1,
        1,
        1,
    ));
}

#[test]
fn the_two_guards_refuse_before_touching_the_board() {
    let mut board = empty_board();
    let before = board.structural_hash();
    assert!(
        !ViaOptimizer::opt_via_location(&mut board, ItemId(9999), None, 500, 10)
            .expect("cannot fail"),
        "an id naming nothing is Java's impossible `Via` argument"
    );
    assert!(
        !ViaOptimizer::opt_via_location(&mut board, ItemId(9999), None, 500, 0)
            .expect("cannot fail"),
        ":42-45, maxRecursionDepth <= 0"
    );
    assert!(
        !ViaOptimizer::opt_plane_or_fanout_via(&mut board, ItemId(9999), 500, 0)
            .expect("cannot fail"),
        ":163-166, maxRecursionDepth <= 0"
    );
    assert_eq!(before, board.structural_hash(), "the board was not touched");
}

#[test]
fn a_refused_move_leaves_the_board_byte_identical() {
    let mut board = routed_j2();
    let via_ids: Vec<ItemId> = board
        .get_items()
        .filter(|item| matches!(item, Item::Via(_)))
        .map(Item::id)
        .collect();
    assert!(!via_ids.is_empty(), "the routed prefix places vias");
    let mut refused = 0;
    let mut moved_count = 0;
    for via_id in via_ids {
        let before = board.structural_hash();
        let moved =
            ViaOptimizer::opt_via_location(&mut board, via_id, None, 500, 10).expect("cannot fail");
        let after = board.structural_hash();
        if moved {
            moved_count += 1;
            assert_ne!(
                before, after,
                "via {} answered true and did not move",
                via_id.0
            );
        } else {
            refused += 1;
            assert_eq!(
                before, after,
                "a refused move changed the board at via {}",
                via_id.0
            );
        }
    }
    assert!(refused > 0, "the fixture still exercises the refusal path");
    assert!(
        moved_count > 0,
        "and the move path, which is what Task 7 opened"
    );
}

#[test]
fn the_overload_dispatch_matches_javas_contact_counts() {
    let mut seen = BTreeMap::new();
    for (tag, mode) in [("rpi", 0), ("ecc83", 0)] {
        let ours = p7t4_rows(tag, mode);
        let theirs = section(TRANSCRIPT, &format!("{tag} mode {mode}"));
        let ours_vias = ours.iter().filter(|r| r.starts_with("via "));
        let theirs_vias = theirs.iter().filter(|r| r.starts_with("via "));
        let mut rows = 0;
        for (ours_row, theirs_row) in ours_vias.zip(theirs_vias) {
            assert_eq!(
                column(ours_row, "center="),
                column(theirs_row, "center="),
                "{tag}: via order"
            );
            assert_eq!(
                column(ours_row, "class="),
                column(theirs_row, "class="),
                "{tag}: via {}'s dispatch class",
                column(ours_row, "id=")
            );
            let ours_contacts: Vec<i64> = split_ids(ours_row).0;
            let contacts = &ours_contacts[1..];
            assert!(
                contacts.windows(2).all(|w| w[0] > w[1]),
                "{tag}: via {}'s contacts are not descending: {contacts:?}",
                column(theirs_row, "id=")
            );
            *seen
                .entry(column(theirs_row, "class=").to_string())
                .or_insert(0) += 1;
            rows += 1;
        }
        assert_eq!(
            rows,
            theirs.iter().filter(|r| r.starts_with("via ")).count(),
            "{tag}: every via row was compared"
        );
    }
    assert_eq!(
        seen.get("PLANE_OR_FANOUT_ONE_CONTACT").copied(),
        Some(2),
        "`rpi` reaches the one-contact arm at :47 twice"
    );
    assert_eq!(
        seen.get("TWO_TRACES").copied(),
        Some(4),
        "`rpi` reaches the two-trace arm at :118 four times. It was TEN across `rpi` and `j2` \
         until the plan9-t7t8 accept wave retired the `j2` arm — see the retirement record. The \
         number is asserted rather than the retirement quietly absorbed."
    );
}

#[test]
fn a_plane_via_moves_through_overload_a() {
    let board = routed_rpi();
    let via_ids: Vec<ItemId> = board
        .get_items()
        .filter(|item| matches!(item, Item::Via(_)))
        .map(Item::id)
        .collect();
    let mut one_contact = Vec::new();
    let mut two_trace = Vec::new();
    for via_id in &via_ids {
        if board.normal_contacts(*via_id).len() == 1 {
            one_contact.push(*via_id);
        } else {
            two_trace.push(*via_id);
        }
    }
    assert_eq!(
        one_contact.iter().map(|id| id.0).collect::<Vec<_>>(),
        vec![189, 84],
        "the routed `rpi` prefix leaves exactly these two one-contact vias"
    );
    assert_eq!(two_trace.len(), 4);

    for (via_id, expected) in [
        (ItemId(189), IntPoint::new(932_812, 1_011_224)),
        (ItemId(84), IntPoint::new(1_016_000, 3_119_161)),
    ] {
        for label in ["opt_via_location", "opt_plane_or_fanout_via"] {
            let mut scratch = board.clone();
            let before = scratch.structural_hash();
            let moved = if label == "opt_via_location" {
                ViaOptimizer::opt_via_location(&mut scratch, via_id, None, 500, 10)
            } else {
                ViaOptimizer::opt_plane_or_fanout_via(&mut scratch, via_id, 500, 10)
            }
            .expect("cannot fail");
            assert!(moved, "{label} on via {} must move it", via_id.0);
            assert_ne!(before, scratch.structural_hash(), "{label} must mutate");
            assert_eq!(
                scratch.drill_center(via_id).expect("still a via"),
                Point::Int(expected),
                "{label} on via {}",
                via_id.0
            );
        }
    }

    for via_id in two_trace {
        let mut scratch = board.clone();
        let before = scratch.structural_hash();
        assert!(
            !ViaOptimizer::opt_plane_or_fanout_via(&mut scratch, via_id, 500, 10)
                .expect("cannot fail"),
            ":188-190 — a second trace contact is already recorded"
        );
        assert_eq!(
            before,
            scratch.structural_hash(),
            "and the board is untouched"
        );
    }
}

fn routed_rpi() -> Board {
    routed("Issue143-rpi_splitter.dsn")
}

fn routed_j2() -> Board {
    routed("Issue026-J2_reference.dsn")
}

fn routed(design_name: &str) -> Board {
    let path = testkit::fixture(design_name);
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

fn regime_name(angle: AngleRestriction) -> &'static str {
    match angle {
        AngleRestriction::NinetyDegree => "NINETY_DEGREE",
        AngleRestriction::FortyFiveDegree => "FORTYFIVE_DEGREE",
        AngleRestriction::None => "NONE",
    }
}

fn type_name(item: &Item) -> &'static str {
    match item {
        Item::Trace(_) => "PolylineTrace",
        Item::Via(_) => "Via",
        Item::Pin(_) => "Pin",
        Item::ObstacleArea(_) => "ObstacleArea",
        Item::ConductionArea(_) => "ConductionArea",
        Item::ViaObstacleArea(_) => "ViaObstacleArea",
        Item::ComponentObstacleArea(_) => "ComponentObstacleArea",
        Item::ComponentOutline(_) => "ComponentOutline",
        Item::BoardOutline(_) => "BoardOutline",
    }
}

fn contact_ids(board: &Board, via: ItemId) -> String {
    let inner: Vec<String> = board
        .normal_contacts(via)
        .into_iter()
        .rev()
        .map(|id| {
            format!(
                "{}:{}",
                id.0,
                type_name(board.get_item(id).expect("a contact"))
            )
        })
        .collect();
    format!("[{}]", inner.join(","))
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
    let via_center = board.drill_center(via).expect("a via has a centre");
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

fn dump_line(line: &Line) -> String {
    format!("({},{})->({},{})", line.a.x, line.a.y, line.b.x, line.b.y)
}

fn dump_corner(polyline: &Polyline, no: usize) -> String {
    match polyline.corner(no).expect("no is below cornerCount") {
        Point::Int(p) => format!("({},{})", p.x, p.y),
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
    let mut rows = vec![format!(
        "maxId={}",
        board.communication.id_gen.max_generated_id()
    )];
    for item in board.get_items() {
        let mut line = format!(
            "item id={} type={} nets={} cl={} fix={}",
            item.id().0,
            type_name(item),
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
        rows.push(line);
    }
    rows
}

#[test]
fn fanout_via_does_not_report_a_move_that_rounds_to_its_current_location() {
    let mut board = empty_board();
    let class = board.rules.get_default_net_class();
    board.rules.nets.add("obstacle", 1, false, class);
    let center = Point::new(0, 0);
    let via = board
        .insert_via(
            copper_board::PadstackId(1),
            center.clone(),
            vec![1],
            1,
            FixedState::Unfixed,
            false,
        )
        .unwrap();
    board
        .insert_via(
            copper_board::PadstackId(1),
            Point::new(125, 125),
            vec![2],
            1,
            FixedState::UserFixed,
            false,
        )
        .unwrap();
    board
        .insert_trace_without_cleaning(
            Polyline::from_points(&[center.clone(), Point::new(1, 1)]),
            0,
            31,
            vec![1],
            1,
            FixedState::Unfixed,
        )
        .unwrap();
    // The obstacle's nearest corner is (105,105). Including the existing
    // 16-unit clearance safety margin and 1-unit endpoint reserve leaves
    // 105*sqrt(2) - (31 + 100 + 16 + 1) = 0.4924 units of movement.
    let distance = board.check_trace_segment(&center, &Point::new(1, 1), 0, &[1], 31, 1, false);
    assert!(
        distance > 0.0 && distance < 0.7,
        "expected a positive sub-grid movement, got {distance}"
    );
    assert_eq!(
        ViaOptimizer::reposition_via_toward_location(
            &mut board,
            via,
            &IntPoint::new(1, 1),
            31,
            0,
            1
        ),
        Some(center.clone())
    );
    let changed = ViaOptimizer::opt_plane_or_fanout_via(&mut board, via, 500, 10).unwrap();
    assert_eq!(board.drill_center(via), Some(center));
    assert!(
        !changed,
        "rounding to the existing location must not restart changed-area optimization"
    );
}
