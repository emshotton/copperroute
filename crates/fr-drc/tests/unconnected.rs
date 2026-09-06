use fr_board::prelude::*;
use fr_drc::{DesignRulesChecker, UnconnectedItems, UnconnectedKind};
use fr_dsn::{BoardReadResult, DsnReadOptions};
use fr_geometry::{IntBox, IntPoint, IntVector, Point, Polyline, Shape, TileShape};

fn fixture_board(name: &str) -> Board {
    let path = parity::fixture(name);
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("cannot read fixture {}: {e}", path.display()));
    match fr_dsn::read_board(&bytes[..], None, Some(name), &DsnReadOptions::default()) {
        BoardReadResult::Success { board, .. } | BoardReadResult::OutlineMissing { board, .. } => {
            *board.unwrap_or_else(|| panic!("{name} produced no board"))
        }
        other => panic!("{name} did not read: {other:?}"),
    }
}

const DEV_BOARD: &str = "Issue575-drc_dev-board_4_hole_clearance_violations.dsn";
const BBD_MARS_64: &str = "Issue575-drc_BBD_Mars-64_6_track_1_hole_clearance_violations.dsn";
const NATURAL_TONE_PREAMP: &str = "Issue575-drc_Natural_Tone_Preamp_7_unconnected_items.dsn";

fn phase_counts(entries: &[UnconnectedItems]) -> (usize, usize, usize) {
    let count = |kind| entries.iter().filter(|e| e.kind == kind).count();
    (
        count(UnconnectedKind::UnconnectedItems),
        count(UnconnectedKind::TrackDangling),
        count(UnconnectedKind::ViaDangling),
    )
}

fn fixture_entries(fixture: &str) -> (Board, Vec<UnconnectedItems>) {
    let mut board = fixture_board(fixture);
    let entries = DesignRulesChecker::new(&mut board).get_all_unconnected_items();
    (board, entries)
}

#[test]
fn dev_board_phase_counts() {
    if !parity::require_reference_dir() {
        return;
    }
    let (_, entries) = fixture_entries(DEV_BOARD);
    assert_eq!(phase_counts(&entries), (4, 8, 0));
}

#[test]
fn bbd_mars_64_phase_counts() {
    if !parity::require_reference_dir() {
        return;
    }
    let (_, entries) = fixture_entries(BBD_MARS_64);
    assert_eq!(phase_counts(&entries), (3, 2, 18));
}

#[test]
fn natural_tone_preamp_phase_counts() {
    if !parity::require_reference_dir() {
        return;
    }
    let (_, entries) = fixture_entries(NATURAL_TONE_PREAMP);
    assert_eq!(phase_counts(&entries), (44, 108, 4));
}

#[test]
fn natural_tone_preamp_matches_the_java_test_lower_bounds() {
    if !parity::require_reference_dir() {
        return;
    }
    let (_, entries) = fixture_entries(NATURAL_TONE_PREAMP);
    let (unconnected, track_dangling, via_dangling) = phase_counts(&entries);

    assert!(track_dangling >= 24, "EXPECTED_DANGLING_TRACKS");
    assert!(via_dangling >= 4, "EXPECTED_DANGLING_VIAS");
    assert!(unconnected >= 9, "EXPECTED_UNCONNECTED_NET_GROUPS");

    assert_eq!((unconnected, track_dangling, via_dangling), (44, 108, 4));
}

#[test]
fn spot_checked_dangling_track_ids() {
    if !parity::require_reference_dir() {
        return;
    }
    let (board, entries) = fixture_entries(NATURAL_TONE_PREAMP);
    let dangling: Vec<ItemId> = entries
        .iter()
        .filter(|e| e.kind == UnconnectedKind::TrackDangling)
        .map(|e| e.first_item)
        .collect();

    for id in [2340, 1869, 2372, 1802].map(ItemId) {
        let item = board
            .get_item(id)
            .unwrap_or_else(|| panic!("track with id {id} should exist in the board"));
        assert!(item.is_trace(), "item {id} should be a Trace");
        assert!(board.is_tail(id), "track {id} should be dangling (is_tail)");
        assert!(
            dangling.contains(&id),
            "DRC should detect track {id} as a dangling track"
        );
    }
}

#[test]
fn the_three_fixtures_match_the_jvm() {
    if !parity::require_reference_dir() {
        return;
    }
    for fixture in [DEV_BOARD, BBD_MARS_64, NATURAL_TONE_PREAMP] {
        let golden = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/data")
                .join(format!(
                    "{}.unconnected.txt",
                    fixture.trim_end_matches(".dsn")
                )),
        )
        .unwrap_or_else(|e| panic!("cannot read the {fixture} transcript: {e}"));
        let (board, entries) = fixture_entries(fixture);
        assert_eq!(render(&board, &entries), golden, "{fixture}");
    }
}

fn render(board: &Board, entries: &[UnconnectedItems]) -> String {
    let kind_class = |id: ItemId| match board.get_item(id).map(Item::kind) {
        Some(ItemKind::Pin) => "Pin",
        Some(ItemKind::Trace) => "Trace",
        _ => "other",
    };
    let net_of = |id: ItemId| {
        board
            .get_item(id)
            .filter(|item| item.net_count() > 0)
            .map_or(0, |item| item.get_net_number(0))
    };

    let mut nets: Vec<&UnconnectedItems> = entries
        .iter()
        .filter(|e| e.kind == UnconnectedKind::UnconnectedItems)
        .collect();
    nets.sort_by_key(|e| net_of(e.first_item));

    let mut out = format!("unconnectedItems {}\n", nets.len());
    for entry in &nets {
        let mut ids: Vec<u32> = entry.all_items.iter().map(|id| id.0).collect();
        ids.sort_unstable();
        let ids: Vec<String> = ids.iter().map(u32::to_string).collect();
        out.push_str(&format!(
            "net={} first={} second={} items={}\n",
            net_of(entry.first_item),
            kind_class(entry.first_item),
            entry.second_item.map_or("none", kind_class),
            ids.join(","),
        ));
    }

    let mut candidates: Vec<u32> = entries
        .iter()
        .filter(|e| e.kind == UnconnectedKind::TrackDangling)
        .map(|e| e.first_item.0)
        .chain(
            entries
                .iter()
                .filter(|e| e.kind == UnconnectedKind::UnconnectedItems)
                .map(|e| e.first_item)
                .filter(|&id| board.get_item(id).is_some_and(Item::is_trace) && board.is_tail(id))
                .map(|id| id.0),
        )
        .collect();
    candidates.sort_unstable_by(|a, b| b.cmp(a));
    out.push_str(&format!("track_dangling_candidates {}\n", candidates.len()));
    for id in candidates {
        out.push_str(&format!("first={id}\n"));
    }

    let vias: Vec<&UnconnectedItems> = entries
        .iter()
        .filter(|e| e.kind == UnconnectedKind::ViaDangling)
        .collect();
    out.push_str(&format!("via_dangling {}\n", vias.len()));
    for entry in vias {
        out.push_str(&format!("first={}\n", entry.first_item.0));
    }
    out
}

#[test]
fn entries_are_ordered_by_ascending_net_number() {
    if !parity::require_reference_dir() {
        return;
    }
    let (board, entries) = fixture_entries(DEV_BOARD);
    let nets: Vec<i32> = entries
        .iter()
        .filter(|e| e.kind == UnconnectedKind::UnconnectedItems)
        .map(|e| board.get_item(e.first_item).unwrap().get_net_number(0))
        .collect();
    assert_eq!(nets, vec![1, 41, 46, 47]);
    assert!(nets.windows(2).all(|w| w[0] < w[1]));
}

#[test]
fn items_within_an_entry_are_ascending_by_id() {
    let mut board = two_groups_board();
    let entries = DesignRulesChecker::new(&mut board).get_all_unconnected_items();
    let entry = &entries[0];
    assert_eq!(entry.kind, UnconnectedKind::UnconnectedItems);
    assert_eq!(entry.all_items, [3, 5, 2, 4].map(ItemId));
    assert_eq!(entry.first_item, ItemId(3));
    assert_eq!(entry.second_item, Some(ItemId(2)));
}

#[test]
fn the_representative_is_the_lowest_id_pin_then_trace_then_item() {
    let mut board = dedup_board();
    let entries = DesignRulesChecker::new(&mut board).get_all_unconnected_items();
    let entry = &entries[0];
    assert_eq!(entry.all_items, [3, 4, 2].map(ItemId));
    assert_eq!(entry.first_item, ItemId(4));
    assert_eq!(entry.second_item, Some(ItemId(2)));
}

#[test]
fn the_dangling_dedup_only_checks_first_item() {
    let mut board = dedup_board();
    let entries = DesignRulesChecker::new(&mut board).get_all_unconnected_items();

    let net_entry = &entries[0];
    assert_eq!(net_entry.kind, UnconnectedKind::UnconnectedItems);
    assert_eq!(net_entry.second_item, Some(ItemId(2)));
    assert!(net_entry.all_items.contains(&ItemId(2)));

    let dangling: Vec<ItemId> = entries
        .iter()
        .filter(|e| e.kind == UnconnectedKind::TrackDangling)
        .map(|e| e.first_item)
        .collect();
    assert_eq!(dangling, vec![ItemId(3), ItemId(2)]);
    assert!(net_entry.all_items.contains(&ItemId(3)));
    assert!(entries[1].second_item.is_none());
    assert_eq!(entries[1].all_items, vec![ItemId(3)]);
}

#[test]
fn a_first_item_trace_is_the_one_case_the_dedup_catches() {
    let mut board = trace_representative_board();
    let entries = DesignRulesChecker::new(&mut board).get_all_unconnected_items();
    assert_eq!(entries[0].first_item, ItemId(4));
    assert_eq!(
        entries
            .iter()
            .filter(|e| e.kind == UnconnectedKind::TrackDangling)
            .map(|e| e.first_item)
            .collect::<Vec<_>>(),
        vec![ItemId(2)],
    );
}

#[test]
fn the_via_phase_has_no_dedup_at_all() {
    if !parity::require_reference_dir() {
        return;
    }
    let (board, entries) = fixture_entries(BBD_MARS_64);
    let vias: Vec<ItemId> = entries
        .iter()
        .filter(|e| e.kind == UnconnectedKind::ViaDangling)
        .map(|e| e.first_item)
        .collect();
    assert_eq!(vias.len(), 18);
    assert!(vias.windows(2).all(|w| w[0] > w[1]));
    assert!(vias.iter().all(|&id| board.is_tail(id)));
}

#[test]
fn a_via_that_represents_its_net_is_still_reported_dangling() {
    let mut board = vias_and_a_dangling_trace_board();
    let entries = DesignRulesChecker::new(&mut board).get_all_unconnected_items();

    let net_entry = &entries[0];
    assert_eq!(net_entry.kind, UnconnectedKind::UnconnectedItems);
    assert_eq!(net_entry.first_item, ItemId(4));
    assert_eq!(net_entry.second_item, Some(ItemId(3)));
    assert_eq!(net_entry.all_items, [4, 3].map(ItemId));
    assert_eq!(
        board.get_item(ItemId(4)).map(Item::kind),
        Some(ItemKind::Via)
    );
    assert!(board.is_tail(ItemId(4)));

    let vias: Vec<ItemId> = entries
        .iter()
        .filter(|e| e.kind == UnconnectedKind::ViaDangling)
        .map(|e| e.first_item)
        .collect();
    assert_eq!(vias, vec![ItemId(4), ItemId(3)]);
}

#[test]
fn every_dangling_trace_precedes_every_dangling_via() {
    let mut board = vias_and_a_dangling_trace_board();
    let entries = DesignRulesChecker::new(&mut board).get_all_unconnected_items();
    assert_eq!(
        entries.iter().map(|e| e.kind).collect::<Vec<_>>(),
        vec![
            UnconnectedKind::UnconnectedItems,
            UnconnectedKind::TrackDangling,
            UnconnectedKind::ViaDangling,
            UnconnectedKind::ViaDangling,
        ],
    );

    if !parity::require_reference_dir() {
        return;
    }
    let (_, entries) = fixture_entries(BBD_MARS_64);
    let index_of = |kind| {
        let idx: Vec<usize> = entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.kind == kind)
            .map(|(i, _)| i)
            .collect();
        (idx[0], idx[idx.len() - 1])
    };
    let (first_net, last_net) = index_of(UnconnectedKind::UnconnectedItems);
    let (first_track, last_track) = index_of(UnconnectedKind::TrackDangling);
    let (first_via, last_via) = index_of(UnconnectedKind::ViaDangling);
    assert_eq!(first_net, 0);
    assert!(last_net < first_track, "phase 1 precedes phase 2");
    assert!(last_track < first_via, "phase 2 precedes phase 3");
    assert_eq!(last_via, entries.len() - 1);
}

#[test]
fn a_net_with_one_item_is_never_unconnected() {
    let mut board = single_item_net_board();
    let entries = DesignRulesChecker::new(&mut board).get_all_unconnected_items();
    assert!(
        entries
            .iter()
            .all(|e| e.kind != UnconnectedKind::UnconnectedItems)
    );
}

#[test]
fn empty_board_has_nothing_unconnected() {
    if !parity::require_reference_dir() {
        return;
    }
    let (_, entries) = fixture_entries("empty_board.dsn");
    assert_eq!(phase_counts(&entries), (0, 0, 0));
}

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
    LayerStructure::new(vec![Layer::new("front", true)])
}

fn bare_board(pin_offsets: &[i32]) -> Board {
    let mut padstacks = Padstacks::new(layers());
    let mut pins = Vec::new();
    for (i, offset) in pin_offsets.iter().enumerate() {
        let shape = Shape::Tile(TileShape::Box(IntBox::from_coords(-50, -50, 50, 50)));
        let name = format!("p{i}");
        let padstack = padstacks.add(&name, vec![Some(shape)], false, false);
        pins.push(PackagePin::new(
            &name,
            padstack,
            IntVector::new(*offset, 0).into(),
            0.0,
        ));
    }
    let mut packages = Packages::new();
    let package = packages.add(
        "pkg",
        pins,
        None,
        None,
        None,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        true,
    );
    let mut components = Components::new();
    components.add_with_generated_name(Some(Point::new(0, 0)), 0.0, true, package);

    let ls = layers();
    let matrix = ClearanceMatrix::get_default_instance(&ls, 200);
    let mut rules = BoardRules::new(ls, matrix);
    rules.create_default_net_class();
    let default_class = rules.get_default_net_class();
    let mut board = Board::new(
        Vec::new(),
        1,
        BOUNDING_BOX,
        rules,
        BoardLibrary::new(padstacks, packages),
        components,
        Communication::default(),
    );
    board.rules.nets.add("N1", 1, false, default_class);
    board
}

fn insert_trace(board: &mut Board, from: (i32, i32), to: (i32, i32)) -> ItemId {
    board
        .insert_trace_without_cleaning(
            Polyline::from_points(&[Point::new(from.0, from.1), Point::new(to.0, to.1)]),
            0,
            30,
            vec![1],
            1,
            FixedState::Unfixed,
        )
        .expect("the synthetic trace is neither degenerate nor closed")
}

fn two_groups_board() -> Board {
    let mut board = bare_board(&[0, 5000]);
    board.insert_pin(1, 0, vec![1], 1, FixedState::Unfixed);
    board.insert_pin(1, 1, vec![1], 1, FixedState::Unfixed);
    insert_trace(&mut board, (0, 0), (1000, 0));
    insert_trace(&mut board, (5000, 0), (6000, 0));
    board
}

fn dedup_board() -> Board {
    let mut board = bare_board(&[0]);
    insert_trace(&mut board, (2000, 2000), (3000, 2000));
    insert_trace(&mut board, (0, 0), (1000, 0));
    board.insert_pin(1, 0, vec![1], 1, FixedState::Unfixed);
    board
}

fn trace_representative_board() -> Board {
    let mut board = bare_board(&[0]);
    insert_trace(&mut board, (2000, 2000), (3000, 2000));
    board.insert_pin(1, 0, vec![1], 1, FixedState::Unfixed);
    insert_trace(&mut board, (4000, 4000), (5000, 4000));
    board
}

fn vias_and_a_dangling_trace_board() -> Board {
    let ls = LayerStructure::new(vec![Layer::new("front", true), Layer::new("back", true)]);
    let pad = Shape::Tile(TileShape::Box(IntBox::from_coords(-70, -70, 70, 70)));
    let mut padstacks = Padstacks::new(ls.clone());
    let thru = padstacks.add("thru", vec![Some(pad.clone()), Some(pad)], true, false);

    let matrix = ClearanceMatrix::get_default_instance(&ls, 200);
    let mut rules = BoardRules::new(ls, matrix);
    rules.create_default_net_class();
    let default_class = rules.get_default_net_class();
    let mut board = Board::new(
        Vec::new(),
        1,
        BOUNDING_BOX,
        rules,
        BoardLibrary::new(padstacks, Packages::new()),
        Components::new(),
        Communication::default(),
    );
    board.rules.nets.add("N1", 1, false, default_class);

    insert_trace(&mut board, (0, 3000), (1000, 3000));
    for x in [0, 5000] {
        board
            .insert_via(
                thru,
                Point::new(x, 0),
                vec![1],
                1,
                FixedState::Unfixed,
                true,
            )
            .expect("the synthetic via inserts cleanly");
    }
    board
}

fn single_item_net_board() -> Board {
    let mut board = bare_board(&[0]);
    board.insert_pin(1, 0, vec![1], 1, FixedState::Unfixed);
    board
}
