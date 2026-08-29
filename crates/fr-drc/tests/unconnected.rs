//! Plan 5 Task 4: `DesignRulesChecker::get_all_unconnected_items`
//! (`drc/DesignRulesChecker.java:91-178`) and `drc.UnconnectedItems`.
//!
//! # Provenance, and what can be compared at all
//!
//! Java's list is **not reproducible run to run**. `connectedSets` is a `HashSet<Item>`
//! (`:123`) over a class with no `hashCode` override, so `allItems`' order and
//! `findRepresentativeItem`'s choice among equal-kind candidates (`:186-201`) are identity-hash
//! ordered; `itemsByNet` is a `HashMap` (`:95`) whose iteration order is table-size dependent.
//! Plan-5 ruling 3 measured that on the JVM and the port answers it with **ascending item id**
//! and **ascending net number** — a deliberate divergence, quirks row #144.
//!
//! So the JVM golden is the *hash-independent projection* of the result:
//! `crates/fr-drc/tests/data/UnconnectedProbe.java` sorts each entry's items, sorts the entries
//! by net number, reduces each representative to its **kind class** (a set holding a `Pin`
//! always yields a `Pin`; one holding no `Pin` but a `Trace` always yields a `Trace`, `:188-198`),
//! and prints the trace phase as its pre-dedup **candidate** set. The emitted `track_dangling`
//! *count* is hash-dependent too — Natural Tone Preamp gives 109, 110 or 111 out of 111
//! candidates on the same jar depending on `-XX:hashCode`, and 110 vs 111 across two runs of the
//! same mode — because the dedup at `:160` drops whichever dangling trace a net entry's
//! `firstItem` happens to be (quirk #146). The port's ascending-id representatives make **108**
//! of the 111, a different point in the same space; that number is asserted below as this port's
//! own regression guard, **not** as Java parity, and `tests/data/README.md` tabulates the
//! measurements. [`the_three_fixtures_match_the_jvm`] compares the projection byte for byte, and
//! reconstructs the candidate set from the port's output so that the quirk's two halves are both
//! pinned.
//!
//! The Java-side lower bounds in [`natural_tone_preamp_matches_the_java_test_lower_bounds`] come
//! from `UnconnectedItemsReproductionTest.java:110-145`, and the four spot-checked ids in
//! [`spot_checked_dangling_track_ids`] from `:147-168`.

use fr_board::prelude::*;
use fr_drc::{DesignRulesChecker, UnconnectedItems, UnconnectedKind};
use fr_dsn::{BoardReadResult, DsnReadOptions};
use fr_geometry::{IntBox, IntPoint, IntVector, Point, Polyline, Shape, TileShape};

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

/// A real board, read the way `RoutingFixtureTest` reads one; see `tests/clearance_list.rs`.
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

/// `(unconnected_items, track_dangling, via_dangling)` — the three phases, counted.
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

// ---------------------------------------------------------------------------------------------
// The three fixtures' phase counts
// ---------------------------------------------------------------------------------------------

#[test]
fn dev_board_phase_counts() {
    if !parity::require_java_dir() {
        return;
    }
    let (_, entries) = fixture_entries(DEV_BOARD);
    assert_eq!(phase_counts(&entries), (4, 8, 0));
}

#[test]
fn bbd_mars_64_phase_counts() {
    if !parity::require_java_dir() {
        return;
    }
    let (_, entries) = fixture_entries(BBD_MARS_64);
    assert_eq!(phase_counts(&entries), (3, 2, 18));
}

#[test]
fn natural_tone_preamp_phase_counts() {
    if !parity::require_java_dir() {
        return;
    }
    let (_, entries) = fixture_entries(NATURAL_TONE_PREAMP);
    assert_eq!(phase_counts(&entries), (44, 108, 4));
}

#[test]
fn natural_tone_preamp_matches_the_java_test_lower_bounds() {
    // Port of `UnconnectedItemsReproductionTest.java:110-145`. Its three bounds are
    // **historical**: the "reference JSON" they were read from is the stale 2.1.2-era
    // `../freerouting/fixtures/*-freerouting_drc.json` (plan-5 ruling 10), which the current jar
    // exceeds on every one of them. They are ported because the Java test ports them; the exact
    // numbers asserted alongside are this port's regression guard.
    if !parity::require_java_dir() {
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
    // Port of `UnconnectedItemsReproductionTest.java:147-168`: the four ids the reference JSON
    // names — GND/Top, +5V/Top, GND/Bottom, +5V/Bottom.
    if !parity::require_java_dir() {
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

// ---------------------------------------------------------------------------------------------
// The JVM golden
// ---------------------------------------------------------------------------------------------

#[test]
fn the_three_fixtures_match_the_jvm() {
    // `UnconnectedProbe.java`'s three transcripts, byte for byte. See the module docs for what
    // the projection drops and why: everything it keeps is hash-independent, verified on the jar
    // under `-XX:hashCode=0..4` plus the default.
    if !parity::require_java_dir() {
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

/// `UnconnectedProbe.java`'s output format, exactly.
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

    // The probe's `track_dangling_candidates` block is the trace phase *before* its dedup: the
    // emitted entries plus whichever dangling trace the dedup dropped for being some net entry's
    // `first_item` (`:160`, quirk #146). Reconstructing it here is what makes the comparison
    // hash-independent — and it is a strictly stronger check than comparing the emitted list,
    // because it pins both halves of the quirk.
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
    // `board.getItems()` order: descending id.
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

// ---------------------------------------------------------------------------------------------
// Ruling 3's two order choices, each named so the divergence is greppable
// ---------------------------------------------------------------------------------------------

#[test]
fn entries_are_ordered_by_ascending_net_number() {
    // Ruling 3: Java iterates `itemsByNet`, a `HashMap<Integer, List<Item>>` (`:95`, `:104`),
    // whose order is table-size dependent; the port uses a `BTreeMap`, i.e. ascending net
    // number. Deliberate divergence, quirks row #144.
    if !parity::require_java_dir() {
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
    // Ruling 3: Java's `setItems` is a `HashSet<Item>` (`:123`) and `allItems` is
    // `connectedSets.get(0)` followed by `connectedSets.get(1)` (`:143-146`), so each half is
    // identity-hash ordered; the port keeps `Board::connected_set`'s `BTreeSet` order, i.e.
    // ascending id within each half. Deliberate divergence, quirks row #144.
    let mut board = two_groups_board();
    let entries = DesignRulesChecker::new(&mut board).get_all_unconnected_items();
    let entry = &entries[0];
    assert_eq!(entry.kind, UnconnectedKind::UnconnectedItems);
    // Set 0 is seeded from the highest-id item of the net (`netItems` is `board.getItems()`
    // order, descending), so it is the group at `x = 5000`: pin 3 and trace 5.
    assert_eq!(entry.all_items, [3, 5, 2, 4].map(ItemId));
    assert_eq!(entry.first_item, ItemId(3));
    assert_eq!(entry.second_item, Some(ItemId(2)));
}

#[test]
fn the_representative_is_the_lowest_id_pin_then_trace_then_item() {
    // `findRepresentativeItem` (`:186-201`) — ruling 3's fourth choice. Group 0 here holds a Pin
    // and a Trace, group 1 only a Trace.
    let mut board = dedup_board();
    let entries = DesignRulesChecker::new(&mut board).get_all_unconnected_items();
    let entry = &entries[0];
    // Set 0 = {trace 3, pin 4}: `:188-192` scans the *whole* set for a Pin before `:194-198`
    // looks at Traces, so the Pin wins even though the Trace has the lower id.
    assert_eq!(entry.all_items, [3, 4, 2].map(ItemId));
    assert_eq!(entry.first_item, ItemId(4));
    // Set 1 = {trace 2}: no Pin, so the lowest-id Trace.
    assert_eq!(entry.second_item, Some(ItemId(2)));
}

// ---------------------------------------------------------------------------------------------
// Quirk #146: the dangling dedup only checks `firstItem`
// ---------------------------------------------------------------------------------------------

#[test]
fn the_dangling_dedup_only_checks_first_item() {
    // `:160`: `unconnectedItems.stream().anyMatch(ui -> ui.firstItem == trace)`. A trace that is
    // already a net entry's `secondItem` — or a member of its `allItems` — is emitted a second
    // time as a `track_dangling` entry. Quirk #146.
    let mut board = dedup_board();
    let entries = DesignRulesChecker::new(&mut board).get_all_unconnected_items();

    let net_entry = &entries[0];
    assert_eq!(net_entry.kind, UnconnectedKind::UnconnectedItems);
    assert_eq!(net_entry.second_item, Some(ItemId(2)));
    assert!(net_entry.all_items.contains(&ItemId(2)));

    // Trace 2 is the entry's `second_item` and one of its `all_items`, and still comes out as a
    // `TrackDangling` entry of its own. Trace 3, a member of `all_items` too, likewise.
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
    // The complement of the quirk: when the net entry's representative *is* a dangling trace,
    // the dedup does fire and that trace gets no `track_dangling` entry of its own.
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

// ---------------------------------------------------------------------------------------------
// Vias
// ---------------------------------------------------------------------------------------------

#[test]
fn the_via_phase_has_no_dedup_at_all() {
    // `:168-175` — no `anyMatch` guard, unlike the trace phase. A via that is already a net
    // entry's representative is emitted again.
    if !parity::require_java_dir() {
        return;
    }
    let (_, entries) = fixture_entries(BBD_MARS_64);
    let vias: Vec<ItemId> = entries
        .iter()
        .filter(|e| e.kind == UnconnectedKind::ViaDangling)
        .map(|e| e.first_item)
        .collect();
    assert_eq!(vias.len(), 18);
    // `board.getItems()` order: descending id.
    assert!(vias.windows(2).all(|w| w[0] > w[1]));
}

// ---------------------------------------------------------------------------------------------
// Single-item nets and empty boards
// ---------------------------------------------------------------------------------------------

#[test]
fn a_net_with_one_item_is_never_unconnected() {
    // `:108-110`.
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
    if !parity::require_java_dir() {
        return;
    }
    let (_, entries) = fixture_entries("empty_board.dsn");
    assert_eq!(phase_counts(&entries), (0, 0, 0));
}

// ---------------------------------------------------------------------------------------------
// The synthetic boards
// ---------------------------------------------------------------------------------------------

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

/// A one-layer board carrying one component whose pins sit at the given offsets from the origin,
/// and one net `N1`. Nothing is inserted; the caller places the pins and traces itself, in the
/// order that fixes the item ids.
///
/// **Item 1 is the board outline**: `Board::new` calls `insert_outline` even for an empty shape
/// list, so the caller's first insertion is item 2. The outline is not connectable, so it never
/// reaches the per-net phase.
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

/// Net 1 in two groups of two: pin 2 + trace 4 at the origin, pin 3 + trace 5 at `x = 5000`.
/// Item ids are the insertion order after the outline's 1, so `netItems` (descending) is
/// `[5, 4, 3, 2]` and the two connected sets come out as `[3, 5]` then `[2, 4]`.
fn two_groups_board() -> Board {
    let mut board = bare_board(&[0, 5000]);
    board.insert_pin(1, 0, vec![1], 1, FixedState::Unfixed);
    board.insert_pin(1, 1, vec![1], 1, FixedState::Unfixed);
    insert_trace(&mut board, (0, 0), (1000, 0));
    insert_trace(&mut board, (5000, 0), (6000, 0));
    board
}

/// Net 1 in two groups: a free-floating trace 2, and trace 3 + pin 4 at the origin. The
/// free-floating trace is the *second* representative, which is what quirk #146 needs; the pin
/// is inserted last so that group 0's Pin has a **higher** id than its Trace, which is what
/// `the_representative_is_the_lowest_id_pin_then_trace_then_item` needs.
fn dedup_board() -> Board {
    let mut board = bare_board(&[0]);
    insert_trace(&mut board, (2000, 2000), (3000, 2000));
    insert_trace(&mut board, (0, 0), (1000, 0));
    board.insert_pin(1, 0, vec![1], 1, FixedState::Unfixed);
    board
}

/// Net 1 in three groups: two free-floating traces 2 and 4 and a lone pin 3 — with a trace the
/// highest id, so group 0 is `{4}` alone and its representative is a dangling trace.
fn trace_representative_board() -> Board {
    let mut board = bare_board(&[0]);
    insert_trace(&mut board, (2000, 2000), (3000, 2000));
    board.insert_pin(1, 0, vec![1], 1, FixedState::Unfixed);
    insert_trace(&mut board, (4000, 4000), (5000, 4000));
    board
}

/// One pin on net 1 and nothing else — `netItems.size() <= 1` (`:108-110`).
fn single_item_net_board() -> Board {
    let mut board = bare_board(&[0]);
    board.insert_pin(1, 0, vec![1], 1, FixedState::Unfixed);
    board
}
