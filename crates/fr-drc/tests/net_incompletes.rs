//! Plan 5 Task 5: `drc.NetIncompletes` (`drc/NetIncompletes.java:57-226`) and `drc.AirLine`.
//!
//! # What is compared to the JVM, and what cannot be
//!
//! Plan-5 ruling 4 splits this class in two. The per-net `count()`,
//! `get_connected_group_count()`, `get_length_violation()` and `get_marker_radius()` are graph
//! (and rules) invariants — **hash-independent** — and [`the_three_fixtures_match_the_jvm`]
//! compares them byte for byte against `tests/data/*.netincompletes.txt`, which was re-verified
//! for this task under `-XX:hashCode=0..4` (one digest per fixture across all five modes).
//!
//! The **airlines themselves** are not comparable. `NetIncompletes.calculateNetItems` seeds its
//! outer loop from a `HashSet<Item>` (`:295`, `:299`) over a class with no `hashCode` override,
//! so the Delaunay corner insertion order — hence which of several equal-length edges the
//! spanning tree accepts — is identity-hash ordered. The same sweep gives **five** different
//! airline lists on the dev board, **two** on BBD Mars-64 and **five** on Natural Tone Preamp,
//! at identical counts. [`the_airline_endpoints_are_a_hash_dependent_choice`] prints the
//! comparison against the one committed run and asserts only the counts; `tests/data/README.md`
//! carries the classification.
//!
//! The port's own two order choices are unit-tested next to the private functions they belong to
//! (`src/net_incompletes.rs`): `net_items_are_ordered_descending_within_a_component` (ruling 15),
//! `seeds_are_taken_in_ascending_id_order` (ruling 3) and the three `Edge` comparator tests
//! including quirk #147's dropped edge. `calculate_net_items`, `NetItem` and `Edge` are private
//! in Java too, so there is nothing here to reach them through.

use std::collections::BTreeMap;

use fr_board::prelude::*;
use fr_drc::{AirLine, DesignRulesChecker, NetIncompletes, UnconnectedKind};
use fr_dsn::{BoardReadResult, DsnReadOptions};
use fr_geometry::{FloatPoint, IntBox, IntPoint, IntVector, Point, Polyline, Shape, TileShape};

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

/// A real board, read the way `RoutingFixtureTest` reads one; see `tests/unconnected.rs`.
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

const FIXTURES: [&str; 3] = [
    "Issue575-drc_dev-board_4_hole_clearance_violations",
    "Issue575-drc_BBD_Mars-64_6_track_1_hole_clearance_violations",
    "Issue575-drc_Natural_Tone_Preamp_7_unconnected_items",
];

/// The per-net item lists `DesignRulesChecker.calculateAllIncompletes` builds
/// (DesignRulesChecker.java:544-563) — every `Connectable` item, once per net it carries,
/// indexed by `netNumber - 1`.
///
/// Task 6 owns `calculateAllIncompletes`; this is the two-loop part of it the constructor needs,
/// reproduced here so Task 5 can be compared to the JVM on its own. The **order** of each list is
/// not observable: `NetIncompletes::new` filters it and hands the result to `calculate_net_items`,
/// which drops it into a set (`:295`).
fn net_item_lists(board: &Board) -> Vec<Vec<ItemId>> {
    let max_net_no = board.rules.nets.max_net_number();
    let mut lists: Vec<Vec<ItemId>> = vec![Vec::new(); max_net_no.max(0) as usize];
    for item in board.get_items() {
        if !item.is_connectable() {
            continue;
        }
        for i in 0..item.net_count() {
            let net_number = item.get_net_number(i);
            if net_number >= 1 && net_number <= max_net_no {
                lists[net_number as usize - 1].push(item.id());
            }
        }
    }
    lists
}

/// Every net's [`NetIncompletes`], indexed by `netNumber - 1` — Java's `netIncompletes` array
/// (DesignRulesChecker.java:617-621).
fn all_net_incompletes(board: &Board) -> Vec<NetIncompletes> {
    net_item_lists(board)
        .iter()
        .enumerate()
        .map(|(i, items)| NetIncompletes::new(i as i32 + 1, items, board))
        .collect()
}

// ---------------------------------------------------------------------------------------------
// The JVM golden
// ---------------------------------------------------------------------------------------------

/// Renders the port's per-net results in `NetIncompletesProbe`'s transcript format.
fn transcript(board: &Board) -> String {
    let lists = net_item_lists(board);
    let mut out = format!("nets {}\n", lists.len());
    let mut total = 0usize;
    for (i, items) in lists.iter().enumerate() {
        let net_number = i as i32 + 1;
        let net_incompletes = NetIncompletes::new(net_number, items, board);
        total += net_incompletes.count();
        out.push_str(&format!(
            "net={} items={} count={} groups={} lengthViolation={} markerRadius={}\n",
            net_number,
            items.len(),
            net_incompletes.count(),
            net_incompletes.get_connected_group_count(),
            fr_dsn::java_double_to_string(net_incompletes.get_length_violation()),
            fr_dsn::java_double_to_string(net_incompletes.get_marker_radius()),
        ));
    }
    // `getAllAirlines().length` (DesignRulesChecker.java:780-798) and `getIncompleteCount()`
    // (`:663-706`) are both the sum of the per-net counts; Task 6 ports them.
    out.push_str(&format!("airlines {total}\n"));
    out.push_str(&format!("incompleteCount {total}\n"));
    out
}

#[test]
fn the_three_fixtures_match_the_jvm() {
    // `crates/fr-drc/tests/data/NetIncompletesProbe.java`, run on the clone's HEAD jar; see
    // `tests/data/README.md` for the recorded command and the hash sweep that shows this
    // projection is stable across `-XX:hashCode=0..4`.
    if !parity::require_java_dir() {
        return;
    }
    for stem in FIXTURES {
        let board = fixture_board(&format!("{stem}.dsn"));
        let expected = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/data")
                .join(format!("{stem}.netincompletes.txt")),
        )
        .expect("the JVM transcript is committed next to the probe");
        assert_transcripts_eq(&transcript(&board), &expected, stem);
    }
}

/// `assert_eq!` on two 100-line transcripts is unreadable; this names the first differing line.
fn assert_transcripts_eq(actual: &str, expected: &str, stem: &str) {
    for (i, (a, e)) in actual.lines().zip(expected.lines()).enumerate() {
        assert_eq!(a, e, "{stem}: transcript line {}", i + 1);
    }
    assert_eq!(
        actual.lines().count(),
        expected.lines().count(),
        "{stem}: transcript line count",
    );
}

#[test]
fn the_airline_endpoints_are_a_hash_dependent_choice() {
    // Plan-5 ruling 4: the endpoints are **not** a parity surface. This compares the port's list
    // against the one committed JVM run, asserts only what the hash sweep showed to be stable —
    // the per-net counts — and *prints* the classification of every difference. Run with
    // `--nocapture` to see it; `tests/data/README.md` records the measured numbers.
    //
    // Measured while writing this task, over the six JVM runs (`-XX:hashCode=0..4` and the
    // default): the port differs from the committed run on **1 of 9** airlines on the dev board,
    // **0 of 3** on BBD Mars-64 and **6 of 145** on Natural Tone Preamp — and every one of those
    // 157 airlines is also chosen by at least one of the six JVM runs. The JVM gives 6, 1 and 6
    // distinct lists over those runs respectively, so the port's list sits *inside* the space
    // Java itself spans rather than beside it. Note the differences are not confined to
    // equal-length swaps: on Natural Tone Preamp nets 16 and 23 the port's spanning tree has a
    // different total weight than the committed run's — and so do the JVM's own runs (net 16
    // ranges over 73.7e9..87.7e9 across the six), because a different corner insertion order
    // changes which *edges the triangulation has*, not only which of them Kruskal takes.
    if !parity::require_java_dir() {
        return;
    }
    for stem in FIXTURES {
        let board = fixture_board(&format!("{stem}.dsn"));
        let jvm = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/data")
                .join(format!("{stem}.airlines.txt")),
        )
        .expect("the JVM airline dump is committed next to the probe");

        // `NetIncompletesProbe`'s second output: one line per airline, in `getAllAirlines` order.
        let mut jvm_by_net: BTreeMap<i32, Vec<Line>> = BTreeMap::new();
        for text in jvm.lines().filter(|l| !l.is_empty()) {
            let field = |key: &str| {
                text.split_whitespace()
                    .find_map(|f| f.strip_prefix(key))
                    .unwrap_or_else(|| panic!("no {key} in {text:?}"))
            };
            let corner = |key: &str| {
                let (x, y) = field(key).split_once(',').expect("x,y");
                FloatPoint::new(x.parse().unwrap(), y.parse().unwrap())
            };
            jvm_by_net
                .entry(field("net=").parse().unwrap())
                .or_default()
                .push(Line {
                    ends: (
                        field("from=").parse().unwrap(),
                        field("to=").parse().unwrap(),
                    ),
                    length_square: corner("fromCorner=").distance_square(&corner("toCorner=")),
                });
        }

        let mut port_by_net: BTreeMap<i32, Vec<Line>> = BTreeMap::new();
        for net_incompletes in all_net_incompletes(&board) {
            for airline in &net_incompletes.incompletes {
                port_by_net
                    .entry(airline.net_number)
                    .or_default()
                    .push(Line {
                        ends: (airline.from_item.0, airline.to_item.0),
                        length_square: airline.to_corner.distance_square(&airline.from_corner),
                    });
            }
        }

        // The parity half: the same nets carry airlines, and the same number of them.
        let counts = |by_net: &BTreeMap<i32, Vec<Line>>| -> Vec<(i32, usize)> {
            by_net.iter().map(|(&n, v)| (n, v.len())).collect()
        };
        assert_eq!(
            counts(&jvm_by_net),
            counts(&port_by_net),
            "{stem}: per-net airline counts",
        );

        // The informational half. A difference is "expected" under ruling 4 when the two nets'
        // spanning trees have the **same multiset of edge lengths** — i.e. the port took an
        // equally short edge, which is exactly what a different corner insertion order does.
        let mut differing = 0usize;
        let mut same_weight = 0usize;
        for (net, jvm_lines) in &jvm_by_net {
            let port_lines = &port_by_net[net];
            let unmatched = port_lines
                .iter()
                .filter(|line| !jvm_lines.iter().any(|other| line.same_ends(other)))
                .count();
            if unmatched == 0 {
                continue;
            }
            differing += unmatched;
            let weights = |lines: &[Line]| {
                let mut w: Vec<String> = lines
                    .iter()
                    .map(|l| fr_dsn::java_double_to_string(l.length_square))
                    .collect();
                w.sort();
                w
            };
            let (port_weights, jvm_weights) = (weights(port_lines), weights(jvm_lines));
            if port_weights == jvm_weights {
                same_weight += unmatched;
            }
            println!(
                "  net={net}: {unmatched} of {} differ; equal-weight tree: {}",
                port_lines.len(),
                port_weights == jvm_weights,
            );
        }
        println!(
            "{stem}: {differing} of {} airlines have endpoints the committed JVM run did not \
             pick, {same_weight} of them inside an equal-weight spanning tree",
            counts(&port_by_net).iter().map(|(_, c)| c).sum::<usize>(),
        );
    }
}

/// One airline of the comparison above: its two item ids and its squared length.
struct Line {
    ends: (u32, u32),
    length_square: f64,
}

impl Line {
    /// Java's `AirLine` has no direction — `from`/`to` is whichever way the Delaunay edge came
    /// out — so an airline that appears reversed is the same airline.
    fn same_ends(&self, other: &Line) -> bool {
        self.ends == other.ends || self.ends == (other.ends.1, other.ends.0)
    }
}

// ---------------------------------------------------------------------------------------------
// The filter (NetIncompletes.java:80-116)
// ---------------------------------------------------------------------------------------------

#[test]
fn an_isolated_pin_still_reaches_the_ratsnest() {
    // `:99-105`: a `DrillItem` — a `Pin` or a `Via` — is exempt from the contact-free drop,
    // because an unrouted pin legitimately has no contacts and *should* be in the ratsnest. A
    // free-floating `Trace` is not exempt; it never reaches that test, because a trace with a
    // contact-free end is already `isTail()` (`:93`, Trace.java:213-218).
    let mut board = isolated_pin_board();
    assert!(!board.is_tail(ItemId(2)), "the pin is not a tail");
    assert!(board.is_tail(ItemId(3)), "the free-floating trace is");
    assert!(board.normal_contacts(ItemId(2)).is_empty());

    let net_incompletes = NetIncompletes::new(1, &[ItemId(2), ItemId(3)], &board);
    // Only the pin survives, so `groupedNetItems.length <= 1` and the early return at `:154-163`
    // fires: one group, no airline. Had the trace survived there would be two groups and one.
    assert_eq!(net_incompletes.get_connected_group_count(), 1);
    assert_eq!(net_incompletes.count(), 0);
    assert!(net_incompletes.incompletes.is_empty());

    // And with the pin removed there is nothing left at all — `:155` writes the array length,
    // which is 0, not the `uniqueConnectedSets` count.
    let empty = NetIncompletes::new(1, &[ItemId(3)], &board);
    assert_eq!(empty.get_connected_group_count(), 0);
    assert_eq!(empty.count(), 0);

    // Nothing above mutates the board; `DesignRulesChecker` is the thing that borrows it mutably.
    let _ = DesignRulesChecker::new(&mut board);
}

#[test]
fn dangling_items_are_filtered_before_triangulation() {
    // `:93-96` drops every `isTail()` item, so a dangling via is **not** an incomplete
    // connection. The two lists are complementary by construction: the same via is Task 4's
    // `ViaDangling` entry.
    let mut board = dangling_via_board();
    assert!(board.is_tail(ItemId(4)), "the free-standing via is a tail");

    let with_via = NetIncompletes::new(1, &[ItemId(2), ItemId(3), ItemId(4)], &board);
    // Two pins, two groups, one airline — the via contributed no third group.
    assert_eq!(with_via.get_connected_group_count(), 2);
    assert_eq!(with_via.count(), 1);
    let airline = &with_via.incompletes[0];
    assert_eq!(airline.net_number, 1);
    assert_eq!(
        [airline.from_item, airline.to_item],
        [ItemId(2), ItemId(3)],
        "each pin is its own single-item component, so the ascending seed order (ruling 3) puts \
         pin 2 into the `NetItem` array first and ruling 15's within-component reversal has \
         nothing to reverse",
    );

    // The complement: Task 4's list reports exactly that via, and nothing the ratsnest kept.
    let dangling: Vec<ItemId> = DesignRulesChecker::new(&mut board)
        .get_all_unconnected_items()
        .into_iter()
        .filter(|e| e.kind == UnconnectedKind::ViaDangling)
        .map(|e| e.first_item)
        .collect();
    assert_eq!(dangling, vec![ItemId(4)]);
}

// ---------------------------------------------------------------------------------------------
// calcLengthViolation (NetIncompletes.java:257-275)
// ---------------------------------------------------------------------------------------------

#[test]
fn length_violation_is_zero_without_a_net_class_restriction() {
    // `:260-263`: both limits `<= 0` — the default `NetClass` — clears the field and answers
    // "unchanged" without ever reading the trace length.
    let board = complete_net_board();
    let mut net_incompletes = NetIncompletes::new(1, &[ItemId(2), ItemId(3), ItemId(4)], &board);
    assert_eq!(net_incompletes.get_length_violation(), 0.0);
    assert!(!net_incompletes.calc_length_violation(&board));
    assert_eq!(net_incompletes.get_length_violation(), 0.0);
}

#[test]
fn length_violation_is_positive_when_too_long() {
    // `:266-268`: over the maximum, by exactly the excess. The maximum arm has **no**
    // `incompletes.isEmpty()` guard, unlike the minimum one.
    let mut board = complete_net_board();
    assert_eq!(board.net_trace_length(1), 5000.0);
    set_trace_length_limits(&mut board, 0.0, 1000.0);

    let mut net_incompletes = NetIncompletes::new(1, &[ItemId(2), ItemId(3), ItemId(4)], &board);
    // The constructor calls `calcLengthViolation` on its way out (`:225`).
    assert_eq!(net_incompletes.get_length_violation(), 4000.0);
    // Idempotent: the second call recomputes the same number and answers "unchanged".
    assert!(!net_incompletes.calc_length_violation(&board));
    assert_eq!(net_incompletes.get_length_violation(), 4000.0);
}

#[test]
fn length_violation_is_negative_only_when_the_net_is_complete() {
    // `:269-271`: the minimum arm fires only when `incompletes.isEmpty()`. A net that is still
    // unrouted is short for a reason, and Java declines to report that as a length violation.
    let mut complete = complete_net_board();
    set_trace_length_limits(&mut complete, 12_000.0, 0.0);
    let net_incompletes = NetIncompletes::new(1, &[ItemId(2), ItemId(3), ItemId(4)], &complete);
    assert_eq!(net_incompletes.count(), 0, "one connected group");
    assert_eq!(net_incompletes.get_length_violation(), -7000.0);

    // The same limits on a net that still has an airline: no violation at all.
    let mut incomplete = dangling_via_board();
    assert_eq!(incomplete.net_trace_length(1), 0.0);
    set_trace_length_limits(&mut incomplete, 12_000.0, 0.0);
    let net_incompletes = NetIncompletes::new(1, &[ItemId(2), ItemId(3)], &incomplete);
    assert_eq!(net_incompletes.count(), 1);
    assert_eq!(net_incompletes.get_length_violation(), 0.0);
}

#[test]
fn length_violation_change_threshold_is_one_tenth() {
    // `:274`: `Math.abs(newViolation - oldViolation) > 0.1`, **strictly** — and the field is
    // written either way, so a caller that trusts the `false` sees a stale number.
    //
    // The steps below are exact in binary: `0.2_f64 - 0.1_f64 == 0.1_f64` (the two have the same
    // significand and adjacent exponents), so the boundary is hit exactly rather than
    // approached. A single pin on an otherwise trace-free net gives `traceLength == 0` and an
    // empty `incompletes`, which is what the minimum-length arm (`:269`) needs.
    let mut board = lone_pin_board();
    assert_eq!(board.net_trace_length(1), 0.0);
    let mut net_incompletes = NetIncompletes::new(1, &[ItemId(2)], &board);
    assert!(net_incompletes.incompletes.is_empty());
    assert_eq!(net_incompletes.get_length_violation(), 0.0);

    // 0.0 -> -0.1: a move of exactly 0.1, which is not `> 0.1`.
    set_trace_length_limits(&mut board, 0.1, 0.0);
    assert!(!net_incompletes.calc_length_violation(&board));
    assert_eq!(net_incompletes.get_length_violation(), -0.1);

    // -0.1 -> -0.2: exactly 0.1 again.
    set_trace_length_limits(&mut board, 0.2, 0.0);
    assert!(!net_incompletes.calc_length_violation(&board));
    assert_eq!(net_incompletes.get_length_violation(), -0.2);

    // -0.2 -> -0.5: 0.3, over the threshold.
    set_trace_length_limits(&mut board, 0.5, 0.0);
    assert!(net_incompletes.calc_length_violation(&board));
    assert_eq!(net_incompletes.get_length_violation(), -0.5);

    // Clearing both limits writes 0 and still answers "unchanged" (`:260-263`), even though the
    // violation moved by 0.5.
    set_trace_length_limits(&mut board, 0.0, 0.0);
    assert!(!net_incompletes.calc_length_violation(&board));
    assert_eq!(net_incompletes.get_length_violation(), 0.0);
}

#[test]
fn the_marker_radius_is_twice_the_minimum_trace_half_width() {
    // `:58`, the one field that has nothing to do with the ratsnest.
    let board = complete_net_board();
    let net_incompletes = NetIncompletes::new(1, &[ItemId(2)], &board);
    assert_eq!(
        net_incompletes.get_marker_radius(),
        f64::from(board.rules.get_min_trace_half_width()) * 2.0,
    );
}

#[test]
fn a_net_number_with_no_net_has_no_length_restriction() {
    // totalized: Java throws a `NullPointerException` at `:258` when `board.rules.nets.get`
    // returned null at `:60`. `calculateAllIncompletes` only builds net numbers that exist, so
    // the arm is unreachable from the Java producer.
    let board = complete_net_board();
    let mut net_incompletes = NetIncompletes::new(99, &[], &board);
    assert_eq!(net_incompletes.get_net_number(), 99);
    assert!(!net_incompletes.calc_length_violation(&board));
    assert_eq!(net_incompletes.get_length_violation(), 0.0);
}

// ---------------------------------------------------------------------------------------------
// AirLine
// ---------------------------------------------------------------------------------------------

#[test]
fn airline_compare_by_net_name_is_not_a_total_order() {
    // Quirk #148: `AirLine.compareTo` (AirLine.java:47-49) compares `net.name` and nothing else,
    // so two airlines of one net are "equal" however far apart their endpoints are. A
    // `TreeSet<AirLine>` would keep one of them.
    let board = complete_net_board();
    let corner = |x: f64, y: f64| FloatPoint::new(x, y);
    let first = AirLine::new(1, ItemId(2), corner(0.0, 0.0), ItemId(3), corner(1.0, 0.0));
    let second = AirLine::new(1, ItemId(4), corner(9.0, 9.0), ItemId(5), corner(8.0, 8.0));
    assert_ne!(first, second);
    assert_eq!(
        first.compare_by_net_name(&second, &board.rules.nets),
        std::cmp::Ordering::Equal,
    );

    // Between nets it does order, by name — not by number.
    let other_net = AirLine::new(2, ItemId(2), corner(0.0, 0.0), ItemId(3), corner(1.0, 0.0));
    assert_eq!(board.rules.nets.get(1).unwrap().name, "N1");
    assert_eq!(board.rules.nets.get(2).unwrap().name, "A0");
    assert_eq!(
        first.compare_by_net_name(&other_net, &board.rules.nets),
        std::cmp::Ordering::Greater,
    );
    assert_eq!(
        other_net.compare_by_net_name(&first, &board.rules.nets),
        std::cmp::Ordering::Less,
    );
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
/// two nets — `N1` (number 1) and `A0` (number 2, named so that it sorts *before* `N1`) — and a
/// spare padstack for a via. Nothing is inserted; the caller places the items itself, in the
/// order that fixes the ids.
///
/// **Item 1 is the board outline**: `Board::new` calls `insert_outline` even for an empty shape
/// list, so the caller's first insertion is item 2.
fn bare_board(pin_offsets: &[i32]) -> (Board, PadstackId) {
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
    let via_shape = Shape::Tile(TileShape::Box(IntBox::from_coords(-40, -40, 40, 40)));
    let via_padstack = padstacks.add("via", vec![Some(via_shape)], false, false);

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
    board.rules.nets.add("A0", 1, false, default_class);
    (board, via_padstack)
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

/// `NetClass.setMinimumTraceLength`/`setMaximumTraceLength` on the default class, which is the
/// class every net of these boards belongs to. `0` means "no restriction" (`<= 0`, `:260`).
fn set_trace_length_limits(board: &mut Board, minimum: f64, maximum: f64) {
    let class = board.rules.net_classes.get_mut(NetClassId(0));
    class.set_minimum_trace_length(minimum);
    class.set_maximum_trace_length(maximum);
}

/// Net 1 as a pin with no contacts (item 2) and a free-floating trace far from it (item 3).
fn isolated_pin_board() -> Board {
    let (mut board, _) = bare_board(&[0]);
    board.insert_pin(1, 0, vec![1], 1, FixedState::Unfixed);
    insert_trace(&mut board, (5000, 5000), (6000, 5000));
    board
}

/// Net 1 as two unconnected pins (items 2 and 3, at `x = 0` and `x = 5000`) plus a via with no
/// contacts (item 4), which `isTail()` and which the ratsnest therefore drops.
fn dangling_via_board() -> Board {
    let (mut board, via_padstack) = bare_board(&[0, 5000]);
    board.insert_pin(1, 0, vec![1], 1, FixedState::Unfixed);
    board.insert_pin(1, 1, vec![1], 1, FixedState::Unfixed);
    board
        .insert_via(
            via_padstack,
            Point::new(8000, 8000),
            vec![1],
            1,
            FixedState::Unfixed,
            false,
        )
        .expect("the synthetic via is inside the bounding box");
    board
}

/// Net 1 as a single pin (item 2) and nothing else: no traces, so `net_trace_length` is 0, and
/// one `NetItem`, so the constructor takes the early return at `:154-163` and `incompletes` is
/// empty — the state the minimum-length arm of `calcLengthViolation` needs.
fn lone_pin_board() -> Board {
    let (mut board, _) = bare_board(&[0]);
    board.insert_pin(1, 0, vec![1], 1, FixedState::Unfixed);
    board
}

/// Net 1 fully routed: two pins (items 2 and 3) joined by a 5000-long trace (item 4), so one
/// connected group and no airline.
fn complete_net_board() -> Board {
    let (mut board, _) = bare_board(&[0, 5000]);
    board.insert_pin(1, 0, vec![1], 1, FixedState::Unfixed);
    board.insert_pin(1, 1, vec![1], 1, FixedState::Unfixed);
    insert_trace(&mut board, (0, 0), (5000, 0));
    board
}
