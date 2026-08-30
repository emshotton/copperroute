//! Plan 6 Task 18: the single-pass fixture harness — this crate's stand-in for
//! `app.freerouting.fixtures.RoutingFixtureTest`'s assertion family (spec §14.3).
//!
//! # What the Java suite asserts, and what this file can assert
//!
//! `RoutingFixtureTest.RoutingResultAssertions` (`fixtures/RoutingFixtureTest.java:261-429`, its
//! `check()` at `:365-429`)
//! checks five things after a job runs: a wall-clock bound, a minimum and a maximum **pass**
//! count, `BoardStatistics.connections.incompleteCount` and
//! `BoardStatistics.clearanceViolations.totalCount`. Passes are `AutoroutePassRunner`'s, i.e.
//! Plan 7's (plan-6 ruling 2), and so is the wall clock (controller ruling AC). What Plan 6 owns
//! is **one pass**: `route_connection` over the first *k* connections of a board, which is exactly
//! what `AutoroutePassRunner.runPass` does once. So this harness routes one pass and asserts the
//! two board-state numbers, both computed the way `BoardStatistics.java:271` computes them —
//! `DesignRulesChecker.getIncompleteCount()` and the clearance-violation list.
//!
//! # Quirk #189 is why the bounds mean what they mean
//!
//! `TestingSettings.setMaxPasses` is **first-writer-wins**
//! (`src/test/java/app/freerouting/settings/sources/TestingSettings.java:52-55`: the body is
//! `if (this.settings.maxPasses == null) { … }`). `RoutingFixtureTest.getRoutingJob` (`:53-92`) calls
//! `testingSettings.setMaxPasses(100)` at `:76` on every job — *after* the test has called
//! `setMaxPasses(1)` on the same object. The 100 therefore never lands, and
//! `Dac2020Bm01RoutingTest`'s bound of 194 is a **one-pass** bound. Read the other way round it
//! would be a hundred-pass bound and this file could not reproduce it at all.
//!
//! # The bounds, and where each comes from
//!
//! * `dac2020_bm01_one_pass_two_items_leaves_at_most_194_incompletes` is the in-CI smoke test
//!   spec §14.3 names. `k = 2` and `194` are `Dac2020Bm01RoutingTest.java:24-36` verbatim
//!   (`setMaxItems(2)`, `setMaxPasses(1)`, `maxIncompleteConnections(194)`), and its comment says
//!   why: "There are 195 connections in total on the board … If only net 99 is routed, there
//!   should be 194 incomplete connections left."
//! * The other four have no Java bound at one pass — every other `fixtures/*RoutingTest` runs the
//!   whole batch loop, fanout and optimizer included. Their bounds are the **HEAD jar's own**
//!   numbers, read off the last row of `tests/reference/<stem>/router.jsonl`'s metric block, which
//!   `scripts/gen-router-reference.sh` wrote from `P6T1.java` on the parity jar. The literal is
//!   spelled out here rather than read back from the file, so that a regenerated reference cannot
//!   silently move the bar; `tests/reference_parity.rs` is what checks the port against the
//!   reference row for row.
//!
//! Every fixture also asserts **zero** clearance violations, which is `exactClearanceViolations(0)`
//! in the Java vocabulary and is not a bound but an invariant: a pass that shoves copper into a
//! violation has mis-routed regardless of how many connections it closed.
//!
//! # What runs in CI
//!
//! Only the smoke test. The rest carry `#[cfg_attr(debug_assertions, ignore)]` (Plan 3's
//! convention) and run under `cargo test --release -- --ignored`, because a full board unoptimised
//! is minutes of work.

use std::collections::{BTreeMap, BTreeSet};

use fr_board::prelude::*;
use fr_drc::DesignRulesChecker;
use fr_dsn::{BoardReadResult, DsnReadOptions};
use fr_router::route_connection;
use fr_settings::sources::DefaultSettings;
use fr_settings::{HostEnvironment, RouterSettings, SettingsSource};

// ---------------------------------------------------------------------------------------------
// The harness
// ---------------------------------------------------------------------------------------------

/// What one pass leaves behind: `BoardStatistics.connections.incompleteCount` and
/// `BoardStatistics.clearanceViolations.totalCount` (`BoardStatistics.java:271`, `:277`).
struct PassResult {
    routed: usize,
    incomplete_connections: usize,
    clearance_violations: usize,
}

/// `HeadlessBoardManager.loadFromSpecctraDsn` on a fixture under the Java checkout.
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

/// The headless settings ladder's priority-0 source, sized and tuned for the board — the same
/// `DefaultSettings` `RoutingFixtureTest.getRoutingJob` puts at the bottom of its merger (`:78-82`),
/// and the same one `scripts/differential/java/P6T1.java` uses.
fn build_settings(board: &Board) -> RouterSettings {
    let mut settings = DefaultSettings::new(&HostEnvironment::detect())
        .get_settings()
        .expect("DefaultSettings always answers a table")
        .clone();
    settings.set_layer_count(board.get_layer_count());
    settings.apply_board_specific_optimizations(board);
    settings
}

/// `AutoroutePassRunner.runPass`'s item walk, truncated at `settings.maxItems`: `getItems()`
/// order (descending id, quirk #63) × each item's own net index order, keeping the pairs with a
/// non-empty unconnected set.
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

/// One pass — `maxPasses = 1`, `maxItems = k` — and the two numbers the Java assertion family
/// reads off the board afterwards.
fn route_one_pass(dsn: &str, k: usize) -> PassResult {
    let mut board = load_board(dsn);
    let settings = build_settings(&board);
    let trace_costs = settings.get_trace_costs();
    let connections = pick_connections(&board, k);
    let routed = connections.len();

    for (item_id, net_no) in connections {
        if board.get_item(item_id).is_none() {
            // The item was ripped up by an earlier connection of the same pass.
            // `AutoroutePassRunner.java:202` walks `autorouteItemList`, a snapshot taken once
            // per pass, and does **not** re-check that the item still exists — a real pass
            // would have rebuilt the list before the next pass. `P6T1.java:254-259` reports
            // this case as `"GONE"` for the same reason; here it is simply skipped, and no
            // corpus board reaches it inside one pass.
            continue;
        }
        // `AutoroutePassRunner.java:224` — quirk #177 makes the presence of `changed_area`
        // observable inside `TraceShover::insert`, so leaving this out would route another board.
        board.start_marking_changed_area();
        let mut ripped: BTreeSet<ItemId> = BTreeSet::new();
        let mut ripup_costs: BTreeMap<ItemId, i32> = BTreeMap::new();
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
            // `AutorouteConnectionRouter.route:45`'s `ripupPassNo`, which is the pass index —
            // 1 for the single pass this harness runs.
            1,
            settings.get_start_ripup_costs(),
            !settings.is_fanout_enabled(),
            false,
            &|| false,
        );
    }

    let mut drc = DesignRulesChecker::new(&mut board);
    PassResult {
        routed,
        incomplete_connections: drc.get_incomplete_count(),
        clearance_violations: drc.get_all_clearance_violations().len(),
    }
}

/// The two assertions, together, with the Java vocabulary in the messages.
fn check(name: &str, result: &PassResult, max_incomplete_connections: usize) {
    assert!(
        result.incomplete_connections <= max_incomplete_connections,
        "'{name}' should have at most {max_incomplete_connections} unrouted connection(s), \
         but had {}.",
        result.incomplete_connections
    );
    assert_eq!(
        result.clearance_violations, 0,
        "'{name}' should have exactly 0 clearance violation(s)."
    );
}

// ---------------------------------------------------------------------------------------------
// The in-CI smoke test (spec §14.3)
// ---------------------------------------------------------------------------------------------

/// `Dac2020Bm01RoutingTest.issue508Bm01First2NetsOnly`
/// (`src/test/java/app/freerouting/fixtures/Dac2020Bm01RoutingTest.java:13-36`): `setMaxItems(2)`,
/// `setMaxPasses(1)` and `maxIncompleteConnections(194)` on `Issue508-DAC2020_bm01.dsn`.
///
/// Spec §14.3 names this one as the smoke test that runs in normal CI, so it carries no
/// `ignore`. Two connections on this board is about a second in a debug build.
#[test]
fn dac2020_bm01_one_pass_two_items_leaves_at_most_194_incompletes() {
    if !parity::require_java_dir() {
        return;
    }
    let result = route_one_pass("fixtures/Issue508-DAC2020_bm01.dsn", 2);
    assert_eq!(result.routed, 2, "maxItems(2) must pick exactly 2");
    check("Issue508-DAC2020_bm01.dsn", &result, 194);
}

// ---------------------------------------------------------------------------------------------
// The rest of the corpus, at one pass over the whole board
// ---------------------------------------------------------------------------------------------

/// `Issue143-rpi_splitter.dsn`, the eight connections `tests/reference/router-rpi-splitter` pins.
/// Bound: the jar's own `incompletes` after the eighth connection — **2**.
#[cfg_attr(debug_assertions, ignore)]
#[test]
fn rpi_splitter_one_pass_leaves_at_most_two_incompletes() {
    if !parity::require_java_dir() {
        return;
    }
    let result = route_one_pass("fixtures/Issue143-rpi_splitter.dsn", 8);
    check("Issue143-rpi_splitter.dsn", &result, 2);
}

/// `Issue026-J2_reference.dsn`, all 45 connections
/// (`tests/reference/router-j2-reference`; the board `J2ReferenceRoutingTest.java:29` uses).
#[cfg_attr(debug_assertions, ignore)]
#[test]
fn j2_reference_one_pass_leaves_at_most_seven_incompletes() {
    if !parity::require_java_dir() {
        return;
    }
    let result = route_one_pass("fixtures/Issue026-J2_reference.dsn", 100_000);
    check("Issue026-J2_reference.dsn", &result, 7);
}

/// `Issue649-kicad_ecc83-pp_input_board_v1.dsn`, all 22 connections — the corpus's board with a
/// `(plane …)` net and a copper pour (`tests/reference/router-ecc83-input`).
#[cfg_attr(debug_assertions, ignore)]
#[test]
fn ecc83_input_one_pass_leaves_no_incompletes() {
    if !parity::require_java_dir() {
        return;
    }
    let result = route_one_pass(
        "fixtures/Issue649-kicad_ecc83-pp_input_board_v1.dsn",
        100_000,
    );
    check("Issue649-kicad_ecc83-pp_input_board_v1.dsn", &result, 0);
}

/// `Issue508-DAC2020_bm01.dsn`, all 294 connections — the whole board at one pass, which is the
/// bound `dac2020_bm01_one_pass_two_items_leaves_at_most_194_incompletes` only samples.
#[cfg_attr(debug_assertions, ignore)]
#[test]
fn dac2020_bm01_one_pass_whole_board_leaves_at_most_57_incompletes() {
    if !parity::require_java_dir() {
        return;
    }
    let result = route_one_pass("fixtures/Issue508-DAC2020_bm01.dsn", 100_000);
    check("Issue508-DAC2020_bm01.dsn", &result, 57);
}

/// `examples/tutorial_board/tutorial_board.dsn` routes **nothing**: its `(network …)` scope is
/// 438 empty `@:no_net_N` nets, so no item has an unconnected set. One pass is therefore a no-op,
/// and the board must come out of it exactly as clean as it went in.
#[cfg_attr(debug_assertions, ignore)]
#[test]
fn tutorial_board_has_no_connections_to_route() {
    if !parity::require_java_dir() {
        return;
    }
    let result = route_one_pass("examples/tutorial_board/tutorial_board.dsn", 100_000);
    assert_eq!(result.routed, 0, "the board has no candidate connections");
    check("tutorial_board.dsn", &result, 0);
}
