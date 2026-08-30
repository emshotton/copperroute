//! Plan 7 Task 1: `fr_router::score` — `BoardStatistics`' score subset.
//!
//! Every literal in this file is the **HEAD jar's own answer**, read off
//! `scripts/differential/run.sh p7t7`'s Java side (JDK 25, `-XX:hashCode=2`,
//! `-Duser.language=en -Duser.country=US -Djava.awt.headless=true`) and rendered by
//! `Float.toString` / `Double.toString` / `Integer.toString`. The driver is the evidence; this
//! file is the regression pin, so a later task cannot move a number without noticing.
//!
//! `p7t7` is byte-identical on eleven fixture/`routeK` pairs and on all five `-XX:hashCode=0..4`
//! modes for `Issue143-rpi_splitter` at `k = 8` — see the task report.

use fr_board::prelude::*;
use fr_board::structure::{FixedState, Unit};
use fr_drc::DesignRulesChecker;
use fr_dsn::{BoardReadResult, DsnReadOptions};
use fr_router::route_connection;
use fr_router::score::BoardStatistics;
use fr_settings::sources::DefaultSettings;
use fr_settings::{HostEnvironment, RouterSettings, ScoringSettings, SettingsSource};

use std::collections::{BTreeMap, BTreeSet};

// ---------------------------------------------------------------------------------------------
// The harness — `P7T7.java`'s, which is `P6T1.java`'s
// ---------------------------------------------------------------------------------------------

fn load_board(rel_path: &str) -> Board {
    let path = parity::java_dir().join(rel_path);
    let file = std::fs::File::open(&path)
        .unwrap_or_else(|e| panic!("cannot open {}: {e}", path.display()));
    let design_name = path
        .file_name()
        .expect("a file name")
        .to_string_lossy()
        .into_owned();
    match fr_dsn::read_board(file, None, Some(&design_name), &DsnReadOptions::default()) {
        BoardReadResult::Success { board, .. } | BoardReadResult::OutlineMissing { board, .. } => {
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
    settings
}

/// `P6T1.pickConnections`, then `route_connection` for each — the same four choices `p6t1` makes.
fn route_first(board: &mut Board, settings: &RouterSettings, max_items: usize) {
    if max_items == 0 {
        return;
    }
    let mut connections: Vec<(ItemId, i32)> = Vec::new();
    'outer: for item_id in board.items_in_board_order() {
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
            connections.push((item_id, net_no));
            if connections.len() >= max_items {
                break 'outer;
            }
        }
    }
    let trace_costs = settings.get_trace_costs();
    for (item_id, net_no) in connections {
        if board.get_item(item_id).is_none() {
            continue;
        }
        board.start_marking_changed_area();
        let mut ripped: BTreeSet<ItemId> = BTreeSet::new();
        let mut ripup_costs: BTreeMap<ItemId, i32> = BTreeMap::new();
        let mut engine = None;
        route_connection(
            board,
            &mut engine,
            item_id,
            net_no,
            settings,
            &trace_costs,
            &mut ripped,
            &mut ripup_costs,
            1,
            settings.get_start_ripup_costs(),
            !settings.is_fanout_enabled(),
            false,
            &|| false,
        );
    }
}

fn board_of(rel_path: &str, route_k: usize) -> Board {
    let mut board = load_board(rel_path);
    let settings = build_settings(&board);
    route_first(&mut board, &settings, route_k);
    board
}

/// One row of the JVM table: everything `p7t7` prints for variant `A` that a reader would want
/// pinned, plus `getNormalizedScore` under the default weights.
struct Row {
    dsn: &'static str,
    route_k: usize,
    items_total: i32,
    maximum_count: i32,
    incomplete_count: i32,
    trace_count: i32,
    segment_count: i32,
    total_length: f32,
    total_length_mm: f32,
    total_weighted_length: f32,
    bend_count: i32,
    via_count: i32,
    violation_count: i32,
    fanout: (i32, i32, i32),
    normalized_score: f32,
}

fn check(row: &Row) {
    let mut board = board_of(row.dsn, row.route_k);
    let settings = build_settings(&board);
    let scoring = settings
        .scoring
        .clone()
        .expect("DefaultSettings always writes a scoring block");
    let stats = BoardStatistics::new(&mut board);
    let what = format!("{} (routeK = {})", row.dsn, row.route_k);

    assert_eq!(
        stats.items.total_count,
        Some(row.items_total),
        "{what}: items.totalCount"
    );
    assert_eq!(
        stats.connections.maximum_count,
        Some(row.maximum_count),
        "{what}: connections.maximumCount"
    );
    assert_eq!(
        stats.connections.incomplete_count,
        Some(row.incomplete_count),
        "{what}: connections.incompleteCount"
    );
    assert_eq!(
        stats.traces.total_count,
        Some(row.trace_count),
        "{what}: traces.totalCount"
    );
    assert_eq!(
        stats.traces.total_segment_count,
        Some(row.segment_count),
        "{what}: traces.totalSegmentCount"
    );
    // Bit-for-bit, not approximately: these are the JVM's `float`s.
    assert_eq!(
        stats.traces.total_length,
        Some(row.total_length),
        "{what}: traces.totalLength"
    );
    assert_eq!(
        stats.traces.total_length_mm,
        Some(row.total_length_mm),
        "{what}: traces.totalLengthMm"
    );
    assert_eq!(
        stats.traces.total_weighted_length,
        Some(row.total_weighted_length),
        "{what}: traces.totalWeightedLength"
    );
    assert_eq!(
        stats.bends.total_count,
        Some(row.bend_count),
        "{what}: bends.totalCount"
    );
    assert_eq!(
        stats.vias.total_count,
        Some(row.via_count),
        "{what}: vias.totalCount"
    );
    assert_eq!(
        stats.clearance_violations.total_count,
        Some(row.violation_count),
        "{what}: clearanceViolations.totalCount"
    );
    assert_eq!(
        (
            stats.fanout.total_smd_pins,
            stats.fanout.pins_to_escape,
            stats.fanout.escaped_count
        ),
        row.fanout,
        "{what}: fanout"
    );
    assert_eq!(
        stats.normalized_score(&scoring),
        row.normalized_score,
        "{what}: getNormalizedScore"
    );
}

// ---------------------------------------------------------------------------------------------
// The JVM table
// ---------------------------------------------------------------------------------------------

/// The five stems that need no routing — the DSN reader's board, scored.
///
/// `tutorial_board` is the `maximumCount == 0` case; `Issue413-test` arrives from KiCad with
/// eleven traces and four vias already on it, so the trace/bend/via blocks are live without a
/// router; `Issue103-Board-Unrouted` is the 1 849-item board with 702 connections and nothing
/// routed.
#[test]
fn normalized_score_matches_the_jvm_on_the_unrouted_stems() {
    let rows = [
        Row {
            dsn: "examples/tutorial_board/tutorial_board.dsn",
            route_k: 0,
            items_total: 439,
            maximum_count: 0,
            incomplete_count: 0,
            trace_count: 0,
            segment_count: 0,
            total_length: 0.0,
            total_length_mm: 0.0,
            total_weighted_length: 0.0,
            bend_count: 0,
            via_count: 0,
            violation_count: 0,
            fanout: (0, 0, 0),
            normalized_score: 0.0,
        },
        Row {
            dsn: "fixtures/Issue143-rpi_splitter.dsn",
            route_k: 0,
            items_total: 33,
            maximum_count: 5,
            incomplete_count: 5,
            trace_count: 0,
            segment_count: 0,
            total_length: 0.0,
            total_length_mm: 0.0,
            total_weighted_length: 0.0,
            bend_count: 0,
            via_count: 0,
            violation_count: 0,
            fanout: (10, 9, 0),
            normalized_score: 0.0,
        },
        Row {
            dsn: "fixtures/Issue413-test.dsn",
            route_k: 0,
            items_total: 37,
            maximum_count: 5,
            incomplete_count: 1,
            trace_count: 11,
            segment_count: 20,
            total_length: 17604.82,
            total_length_mm: 1760.482,
            total_weighted_length: 4.6386614E8,
            bend_count: 9,
            via_count: 4,
            violation_count: 0,
            fanout: (10, 2, 8),
            normalized_score: 799.91797,
        },
        Row {
            dsn: "fixtures/Issue508-DAC2020_bm01.dsn",
            route_k: 0,
            items_total: 320,
            maximum_count: 195,
            incomplete_count: 195,
            trace_count: 0,
            segment_count: 0,
            total_length: 0.0,
            total_length_mm: 0.0,
            total_weighted_length: 0.0,
            bend_count: 0,
            via_count: 0,
            violation_count: 0,
            fanout: (187, 187, 0),
            normalized_score: 0.0,
        },
        Row {
            dsn: "fixtures/Issue103-Board-Unrouted.dsn",
            route_k: 0,
            items_total: 1849,
            maximum_count: 702,
            incomplete_count: 702,
            trace_count: 0,
            segment_count: 0,
            total_length: 0.0,
            total_length_mm: 0.0,
            total_weighted_length: 0.0,
            bend_count: 0,
            via_count: 0,
            violation_count: 0,
            fanout: (60, 60, 0),
            normalized_score: 0.0,
        },
    ];
    for row in &rows {
        check(row);
    }
}

/// The five stems that are routed first, by exactly `p6t1`'s machinery — so the trace, via, bend
/// and weighted-length blocks are exercised on a board that really is routed.
///
/// `#[cfg_attr(debug_assertions, ignore)]` is Plan 3's convention: run them with
/// `cargo test --release -p fr-router --test score` (**not** `-- --ignored`, which in a release
/// build filters them out again).
#[test]
#[cfg_attr(debug_assertions, ignore)]
fn normalized_score_matches_the_jvm_on_the_routed_stems() {
    let rows = [
        Row {
            dsn: "fixtures/Issue143-rpi_splitter.dsn",
            route_k: 8,
            items_total: 49,
            maximum_count: 5,
            incomplete_count: 2,
            trace_count: 10,
            segment_count: 24,
            total_length: 130610.65,
            total_length_mm: 51.421516,
            total_weighted_length: 6.637111E9,
            bend_count: 14,
            via_count: 6,
            violation_count: 0,
            fanout: (10, 4, 7),
            normalized_score: 599.98035,
        },
        Row {
            dsn: "fixtures/Issue508-DAC2020_bm01.dsn",
            route_k: 40,
            items_total: 359,
            maximum_count: 195,
            incomplete_count: 165,
            trace_count: 36,
            segment_count: 291,
            total_length: 11023.574,
            total_length_mm: 1102.3574,
            total_weighted_length: 3.3247096E7,
            bend_count: 255,
            via_count: 3,
            violation_count: 0,
            fanout: (187, 164, 27),
            normalized_score: 153.84225,
        },
        Row {
            dsn: "fixtures/Issue026-J2_reference.dsn",
            route_k: 45,
            items_total: 131,
            maximum_count: 33,
            incomplete_count: 7,
            trace_count: 42,
            segment_count: 161,
            total_length: 3098.9265,
            total_length_mm: 309.89264,
            total_weighted_length: 1.0127292E7,
            bend_count: 119,
            via_count: 8,
            violation_count: 0,
            fanout: (51, 20, 31),
            normalized_score: 787.86725,
        },
        Row {
            // A board with a `(plane …)` net and a copper pour, i.e. `ConductionArea` items —
            // `isPinEscaped`'s third arm and the via arm's inner walk.
            dsn: "fixtures/Issue649-kicad_ecc83-pp_input_board_v1.dsn",
            route_k: 22,
            items_total: 368,
            maximum_count: 21,
            incomplete_count: 0,
            trace_count: 14,
            segment_count: 28,
            total_length: 1615.3136,
            total_length_mm: 161.53136,
            total_weighted_length: 1.2948354E7,
            bend_count: 14,
            via_count: 0,
            violation_count: 0,
            fanout: (0, 0, 0),
            normalized_score: 999.9971,
        },
        Row {
            // The one corpus board whose traces are all fixed harder than `SHOVE_FIXED`, so
            // `:250-262`'s filter drops every one of them and `totalWeightedLength` is 0 even
            // though `totalLength` is not. Also the only stem with clearance violations.
            dsn: "fixtures/Issue753-CPU-85_r104.dsn",
            route_k: 20,
            items_total: 2415,
            maximum_count: 569,
            incomplete_count: 365,
            trace_count: 65,
            segment_count: 66,
            total_length: 1754.4889,
            total_length_mm: 175.44888,
            total_weighted_length: 0.0,
            bend_count: 1,
            via_count: 19,
            violation_count: 78,
            fanout: (102, 79, 0),
            normalized_score: 331.1068,
        },
    ];
    for row in &rows {
        check(row);
    }
}

// ---------------------------------------------------------------------------------------------
// The transcription notes, one test each
// ---------------------------------------------------------------------------------------------

/// `BoardStatistics.java:250-262`: a trace contributes `length * (halfWidth + clearance)`, and a
/// `SHOVE_FIXED` one contributes **half** of that ("to produce less violations with pin exit
/// directions", `:257-259`). A trace fixed harder than `SHOVE_FIXED` contributes nothing at all —
/// the filter at `:252-253` admits only `UNFIXED` and `SHOVE_FIXED`.
///
/// The three states are driven over the same board, so the only thing that moves is the state.
/// The halving is asserted with a relative tolerance rather than bit for bit: Java halves each
/// term **before** adding it to a `float` accumulator, so `Σ(w/2)` and `Σ(w)/2` differ in the
/// last bits by construction — the point of the test is the factor, and `Issue753-CPU-85_r104`'s
/// exact `0.0` row in the JVM table above is what pins the filter bit for bit.
#[test]
fn a_shove_fixed_trace_is_weighted_at_half() {
    let mut board = load_board("fixtures/Issue413-test.dsn");
    let trace_ids = board.get_traces();
    assert!(
        !trace_ids.is_empty(),
        "the fixture must have traces to weigh"
    );

    let set_all = |board: &mut Board, state: FixedState| {
        for id in board.get_traces() {
            board
                .get_item_mut(id)
                .expect("the trace was just listed")
                .set_fixed_state(state);
        }
    };

    set_all(&mut board, FixedState::Unfixed);
    let unfixed = BoardStatistics::new(&mut board)
        .traces
        .total_weighted_length
        .expect("the ctor always sets it");
    assert!(unfixed > 0.0, "an unfixed board must weigh something");

    set_all(&mut board, FixedState::ShoveFixed);
    let shove_fixed = BoardStatistics::new(&mut board)
        .traces
        .total_weighted_length
        .expect("the ctor always sets it");
    assert!(
        (shove_fixed * 2.0 - unfixed).abs() <= unfixed * 1e-6,
        "SHOVE_FIXED should weigh half: {shove_fixed} vs {unfixed}"
    );

    for state in [FixedState::UserFixed, FixedState::SystemFixed] {
        set_all(&mut board, state);
        assert_eq!(
            BoardStatistics::new(&mut board)
                .traces
                .total_weighted_length,
            Some(0.0),
            "{state:?} is outside `:252-253`'s filter"
        );
    }
}

/// `BoardStatistics.java:190-202`: `boardUnitToMmFactor` divides by
/// `resolution > 0 ? resolution : 1`, so a DSN with no `(resolution …)` scope — or a nonsense one
/// — normalises by 1 rather than dividing by zero.
///
/// `Issue413-test.dsn` carries `(unit um) (resolution um 10)`. Passing the board's **own** unit
/// skips the conversion block at `:377-405` (`unit != board.communication.unit` is false), so
/// `totalLength` stays the raw board figure and `totalLengthMm` is that figure times the factor —
/// which is the only thing the guard moves.
#[test]
fn the_unit_normalisation_uses_the_resolution_guard() {
    let mut board = load_board("fixtures/Issue413-test.dsn");
    assert_eq!(board.communication.unit, Unit::Um);
    assert_eq!(board.communication.resolution, 10);

    // Java-pinned first: `p7t7 Issue413-test 0`'s variant `A`, where `unit = null` means
    // millimetres and the µm board therefore *does* go through the conversion block.
    let converted = BoardStatistics::new(&mut board);
    assert_eq!(converted.traces.total_length, Some(17604.82));
    assert_eq!(converted.traces.total_length_mm, Some(1760.482));

    let mm_per_board_unit = Unit::scale(1.0, Unit::Um, Unit::Mm);
    let native = BoardStatistics::compute(&mut board, Some(Unit::Um), true, true);
    let raw = native.traces.total_length.expect("the ctor always sets it");
    assert_eq!(
        native.traces.total_length_mm,
        Some((f64::from(raw) * (mm_per_board_unit / 10.0)) as f32),
        "resolution 10 divides the factor by 10"
    );

    for resolution in [0, -1] {
        board.communication.resolution = resolution;
        let guarded = BoardStatistics::compute(&mut board, Some(Unit::Um), true, true);
        assert_eq!(
            guarded.traces.total_length,
            Some(raw),
            "resolution does not touch the raw length"
        );
        assert_eq!(
            guarded.traces.total_length_mm,
            Some((f64::from(raw) * mm_per_board_unit) as f32),
            "resolution {resolution} must divide by 1, not by {resolution}"
        );
    }
}

/// `BoardStatistics.java:618-635`, and the correction this task made to the plan.
///
/// The plan (`docs/superpowers/plans/2026-08-30-plan-7-router-batch.md`, quirk label #196) and
/// this task's brief both said a board with no connections scores **NaN**, because
/// `getMaximumScore` returns 0 and `Math.max(0, 0f/0f)` is NaN. **HEAD does not do that.**
/// `getNormalizedScore` opens with `if (maximumScore <= 0f) return 0f;` (`:626-633`) and a
/// six-line comment saying the guard exists to stop exactly that NaN from breaking stagnation
/// detection. Java wins: the quirk row was struck, and this test is its inverse.
///
/// `tutorial_board.dsn` is the case — its `(network …)` scope is 438 empty `@:no_net_N` nets, so
/// `maxConnections` is 0.
#[test]
fn an_empty_board_scores_zero_not_nan() {
    let mut board = load_board("examples/tutorial_board/tutorial_board.dsn");
    let settings = build_settings(&board);
    let scoring = settings
        .scoring
        .clone()
        .expect("DefaultSettings always writes a scoring block");
    let stats = BoardStatistics::new(&mut board);

    assert_eq!(stats.connections.maximum_count, Some(0));
    assert_eq!(stats.maximum_score(&scoring), 0.0);
    // The division the guard skips: `0.0 / 0.0` really is NaN, so the guard is load-bearing and
    // not decoration.
    assert!((stats.calculate_score(&scoring) / stats.maximum_score(&scoring)).is_nan());
    let normalized = stats.normalized_score(&scoring);
    assert!(!normalized.is_nan(), "HEAD's guard returns 0f, not NaN");
    assert_eq!(normalized, 0.0);
}

/// `BoardStatistics.java:407-426`: the fanout block reads **net index 0 only** (`:415`), so an
/// SMD pin that is already connected on its first net is counted as connected no matter what its
/// later nets look like. Quirk row.
///
/// The pin is given a second net the same way Java's own code gives one — `Item.netNumbers` is a
/// public, mutated-in-place `int[]` (Item.java:53), and `ItemHeader::net_nos` is the port's field
/// of the same name.
#[test]
fn a_multi_net_smd_pin_is_counted_on_net_index_zero_only() {
    let mut board = board_of("fixtures/Issue143-rpi_splitter.dsn", 8);
    let before = BoardStatistics::new(&mut board).fanout;
    assert_eq!(before.total_smd_pins, 10);
    assert_eq!(before.pins_to_escape, 4);

    // An SMD pin whose net-index-0 unconnected set is empty, i.e. one of the six the block counts
    // as `alreadyConnected`.
    let connected_pin = board
        .get_smd_pins()
        .into_iter()
        .find(|pin| {
            let item = board.get_item(*pin).expect("a listed pin");
            item.net_count() > 0
                && board
                    .unconnected_set(*pin, item.get_net_number(0))
                    .is_empty()
        })
        .expect("the routed board has an already-connected SMD pin");
    let first_net = board
        .get_item(connected_pin)
        .expect("the pin")
        .get_net_number(0);

    // Give it a second net it is *not* connected on. `Item.netNumbers` is a public, mutated-in-
    // place `int[]` in Java (Item.java:53) and `ItemHeader::net_nos` is the port's field of the
    // same name, so this is the same edit `assignNetNo` would make if it appended rather than
    // overwrote. The unconnected set has to be read **after** the push: an item that does not
    // carry a net has no unconnected set on it.
    let mut second_net = None;
    for candidate in 1..=board.rules.nets.max_net_number() {
        if candidate == first_net {
            continue;
        }
        board
            .get_item_mut(connected_pin)
            .expect("the pin")
            .header_mut()
            .net_nos
            .push(candidate);
        if board.unconnected_set(connected_pin, candidate).is_empty() {
            board
                .get_item_mut(connected_pin)
                .expect("the pin")
                .header_mut()
                .net_nos
                .pop();
            continue;
        }
        second_net = Some(candidate);
        break;
    }
    let second_net = second_net.expect("a second net the pin is unconnected on");

    let pin = board.get_item(connected_pin).expect("the pin");
    assert_eq!(pin.net_count(), 2);
    assert_eq!(pin.get_net_number(0), first_net);
    assert_eq!(pin.get_net_number(1), second_net);
    // The counterfactual: had `:415` looped over the pin's nets, this pin would have been found
    // unconnected on index 1 and would have raised `pinsToEscape`.
    assert!(!board.unconnected_set(connected_pin, second_net).is_empty());

    let after = BoardStatistics::new(&mut board).fanout;
    assert_eq!(
        after.pins_to_escape, before.pins_to_escape,
        "`:415` reads net index 0, so the second net cannot move the count"
    );
    assert_eq!(after.total_smd_pins, before.total_smd_pins);
}

/// The four synthetic cases `p7t7` prints as `S0`-`S3`, with the JVM's own `Float.toString`
/// values. Each pins one width decision a plausible mis-transcription would lose; the driver's
/// Javadoc explains them one by one.
///
/// `S0`'s `maximumCount` is `2^24 + 1`, the first integer a `float` cannot represent: Java's
/// `int * Float` promotes to `float` and rounds it down to `16777216`, where an `f64`
/// intermediate would keep the odd value. `S3`'s `viaCount * viaCosts` is `3e9`, an
/// `Integer * Integer` product that **overflows `int` and wraps negative** before `:613` widens
/// it — so the "cost" term raises the score by 1.29e9.
#[test]
fn float_narrowing_matches_java() {
    struct Synth {
        tag: &'static str,
        maximum_count: i32,
        incomplete_count: i32,
        violation_count: i32,
        bend_count: i32,
        total_length_mm: f32,
        via_count: i32,
        unrouted_net_penalty: f32,
        clearance_violation_penalty: f32,
        bend_penalty: f32,
        trace_cost: f64,
        via_costs: i32,
        expected: (f32, f32, f32),
    }

    let cases = [
        Synth {
            tag: "S0",
            maximum_count: 16_777_217,
            incomplete_count: 1,
            violation_count: 3,
            bend_count: 7,
            total_length_mm: 1.1,
            via_count: 5,
            unrouted_net_penalty: 1.0,
            clearance_violation_penalty: 0.1,
            bend_penalty: 0.1,
            trace_cost: 0.1,
            via_costs: 3,
            expected: (1.6777199E7, 1.6777216E7, 999.99896),
        },
        Synth {
            tag: "S1",
            maximum_count: 3,
            incomplete_count: 3,
            violation_count: 11,
            bend_count: 129,
            total_length_mm: 12345.678,
            via_count: 17,
            unrouted_net_penalty: 1.0E7,
            clearance_violation_penalty: 1.5,
            bend_penalty: 0.75,
            trace_cost: 3.3,
            via_costs: 42,
            expected: (-41566.74, 3.0E7, 0.0),
        },
        Synth {
            tag: "S2",
            maximum_count: 0,
            incomplete_count: 4,
            violation_count: 0,
            bend_count: 0,
            total_length_mm: 2.5,
            via_count: 1,
            unrouted_net_penalty: 5.0E6,
            clearance_violation_penalty: 1.0,
            bend_penalty: 1.0,
            trace_cost: 1.0,
            via_costs: 50,
            expected: (-2.0000052E7, 0.0, 0.0),
        },
        Synth {
            tag: "S3",
            maximum_count: 7,
            incomplete_count: 0,
            violation_count: 0,
            bend_count: 0,
            total_length_mm: 0.0,
            via_count: 3000,
            unrouted_net_penalty: 1.0,
            clearance_violation_penalty: 1.0,
            bend_penalty: 1.0,
            trace_cost: 1.0,
            via_costs: 1_000_000,
            expected: (1.2949673E9, 7.0, 1.8499533E11),
        },
        Synth {
            // A tiny negative score over a huge maximum: the quotient at `:634` **underflows to
            // `-0.0f`**, and `Math.max`'s signed-zero clause turns it back into `+0.0f`. Rust's
            // `f32::max` documents the equal-inputs case as non-deterministic, which is why
            // `java_max_f32` is transcribed statement for statement — `Float.toString` renders
            // `0.0` and `-0.0` differently, and the pass loop's `> lastBestScore + 0.5` would
            // not notice the difference until it did.
            tag: "S4",
            maximum_count: 1,
            incomplete_count: 1,
            violation_count: 0,
            bend_count: 0,
            total_length_mm: 1.0E-40,
            via_count: 0,
            unrouted_net_penalty: 3.4E38,
            clearance_violation_penalty: 1.0,
            bend_penalty: 1.0,
            trace_cost: 1.0,
            via_costs: 1,
            expected: (-1.0E-40, 3.4E38, 0.0),
        },
        Synth {
            // `maximumScore` and `penalties` both overflow `float` to `Infinity`, so
            // `calculateScore` is `Inf - Inf = NaN`. The `maximumScore <= 0f` guard does **not**
            // fire (`Infinity <= 0f` is false), so this is the one input on which
            // `getNormalizedScore` really does answer NaN — `Math.max(0, NaN)` is NaN, where
            // Rust's `f32::max` would return the non-NaN `0.0`. Asserted separately below,
            // because `NaN != NaN`.
            tag: "S5",
            maximum_count: 2,
            incomplete_count: 2,
            violation_count: 0,
            bend_count: 0,
            total_length_mm: 0.0,
            via_count: 0,
            unrouted_net_penalty: 3.4E38,
            clearance_violation_penalty: 1.0,
            bend_penalty: 1.0,
            trace_cost: 1.0,
            via_costs: 1,
            expected: (f32::NAN, f32::INFINITY, f32::NAN),
        },
    ];

    for case in &cases {
        let mut stats = BoardStatistics::default();
        stats.connections.maximum_count = Some(case.maximum_count);
        stats.connections.incomplete_count = Some(case.incomplete_count);
        stats.clearance_violations.total_count = Some(case.violation_count);
        stats.bends.total_count = Some(case.bend_count);
        stats.traces.total_length_mm = Some(case.total_length_mm);
        stats.vias.total_count = Some(case.via_count);

        let scoring = ScoringSettings {
            unrouted_net_penalty: Some(case.unrouted_net_penalty),
            clearance_violation_penalty: Some(case.clearance_violation_penalty),
            bend_penalty: Some(case.bend_penalty),
            default_preferred_direction_trace_cost: Some(case.trace_cost),
            via_costs: Some(case.via_costs),
            ..ScoringSettings::default()
        };

        let actual = (
            stats.calculate_score(&scoring),
            stats.maximum_score(&scoring),
            stats.normalized_score(&scoring),
        );
        // Compared by bit pattern, not by `==`: `NaN != NaN` would wave the `S5` row through,
        // and `-0.0 == 0.0` would wave `S4`'s signed zero through. A NaN of any payload counts
        // as a NaN — the payload is the JVM's and the platform's, the NaN-ness is Java's
        // semantics.
        let same = |a: f32, b: f32| (a.is_nan() && b.is_nan()) || a.to_bits() == b.to_bits();
        assert!(
            same(actual.0, case.expected.0)
                && same(actual.1, case.expected.1)
                && same(actual.2, case.expected.2),
            "{}: got {actual:?}, want {:?}",
            case.tag,
            case.expected
        );
    }
}

/// `BoardStatistics.java:265-268` and `:338-341` build **two** `DesignRulesChecker`s, each with
/// its own full calculation. The port does too, and the numbers they produce are the same ones a
/// caller gets from a checker of its own — which is what makes the `fr-drc` dependency edge
/// (ruling 3) a delegation rather than a re-port.
#[test]
fn the_connection_counters_are_fr_drcs_own() {
    let mut board = load_board("fixtures/Issue413-test.dsn");
    let stats = BoardStatistics::new(&mut board);

    let mut drc = DesignRulesChecker::new(&mut board);
    drc.calculate_all_incompletes();
    assert_eq!(stats.connections.maximum_count, Some(drc.max_connections()));
    assert_eq!(
        stats.connections.incomplete_count,
        Some(drc.get_incomplete_count() as i32)
    );
    assert_eq!(
        stats.clearance_violations.total_count,
        Some(drc.get_all_clearance_violations().len() as i32)
    );
}
