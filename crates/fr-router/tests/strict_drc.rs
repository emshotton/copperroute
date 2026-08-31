//! The port of `src/test/java/app/freerouting/autoroute/StrictDrcEnforcementTest.java` (81 loc),
//! method for method — plan-7 ruling 14's second named suite, Task 8's.
//!
//! | Java method | ported below |
//! |---|---|
//! | `violatingNet(RoutingBoard)` (`:28-36`) | [`violating_net`] |
//! | `ripsNewItemsWhenTheyCarryViolations` (`:38-66`) | [`rips_new_items_when_they_carry_violations`] |
//! | `keepsConnectionsWhoseNewItemsAreClean` (`:68-79`) | [`keeps_connections_whose_new_items_are_clean`] |
//!
//! Plus the one assertion the task brief adds on top of the Java suite:
//! [`a_rejected_connection_restores_the_pre_route_board_exactly`], which is about
//! `AutorouteConnectionRouter.applyStrictDrcAfterRoute` (`:243-254`) rather than about
//! `enforceStrictDrc` — Java has no test for the rollback at all, and plan-7 ruling 8 makes the
//! rollback a restore from a `Board` clone, so `structural_hash` equality against that clone is
//! what says the restore is complete.
//!
//! # `assumeTrue` becomes an early `return`
//!
//! JUnit's `Assumptions.assumeTrue` marks a test *skipped* rather than failed. The three
//! `assumeTrue`s here all guard "the fixture really does carry violating routed wiring", which is
//! a property of the committed DSN and not of the port, so the port keeps them as early returns
//! with the same message — a fixture that stopped carrying violations would silently skip in Java
//! too, and turning it into a failure would be a different test.
//!
//! The fixture is `Issue575-drc_BBD_Mars-64_6_track_1_hole_clearance_violations.dsn`, the same
//! "Mars-64 snapshot with 76 violations" the Java suite names, loaded through
//! `parity::java_dir()` because it lives in the Java checkout.

use std::collections::{BTreeMap, BTreeSet};

use fr_board::items::Item;
use fr_board::prelude::*;
use fr_board::structure::FixedState;
use fr_dsn::{BoardReadResult, DsnReadOptions};
use fr_router::pipeline::{BatchAutorouter, RouterBudget};
use fr_router::{AutorouteAttemptState, AutorouteEngine, BoardStatistics, route_connection_full};
use fr_settings::sources::DefaultSettings;
use fr_settings::{HostEnvironment, SettingsSource};

/// `StrictDrcEnforcementTest.FIXTURE` (`:23-24`).
const FIXTURE: &str = "fixtures/Issue575-drc_BBD_Mars-64_6_track_1_hole_clearance_violations.dsn";

/// `DsnTestFixtures.loadBoard(String)` — `DsnReader.readBoard(stream, null, null)`, i.e. no
/// design name, which is what the Java helper passes.
fn load_board() -> Board {
    let path = parity::java_dir().join(FIXTURE);
    let file = std::fs::File::open(&path)
        .unwrap_or_else(|e| panic!("cannot open {}: {e}", path.display()));
    match fr_dsn::read_board(file, None, None, &DsnReadOptions::default()) {
        BoardReadResult::Success { board, .. } | BoardReadResult::OutlineMissing { board, .. } => {
            *board.unwrap_or_else(|| panic!("{FIXTURE} produced no board"))
        }
        other => panic!("{FIXTURE} did not read: {other:?}"),
    }
}

/// `StrictDrcEnforcementTest.violatingNet` (`:27-36`): "finds a net whose traces/vias include at
/// least one clearance violation."
///
/// `board.getItems()` is descending item id (quirk #63), which
/// [`Board::items_in_board_order`] answers, so the *first* violating item — and therefore the net
/// this returns — is the same one Java picks.
fn violating_net(board: &mut Board) -> i32 {
    // :29.
    for id in board.items_in_board_order() {
        // :30-32.
        let Some(item) = board.get_item(id) else {
            continue;
        };
        if !matches!(item, Item::Trace(_) | Item::Via(_)) || item.net_count() == 0 {
            continue;
        }
        if !board.clearance_violations(id).is_empty() {
            // :33.
            return board
                .get_item(id)
                .expect("still on the board")
                .get_net_number(0);
        }
    }
    // :35.
    -1
}

/// `StrictDrcEnforcementTest.ripsNewItemsWhenTheyCarryViolations` (`:37-66`).
#[test]
fn rips_new_items_when_they_carry_violations() {
    if !parity::require_java_dir() {
        return;
    }
    // :40-41.
    let mut board = load_board();
    let net_number = violating_net(&mut board);
    // :42 — `assumeTrue`.
    if net_number <= 0 {
        eprintln!("skipped: fixture must contain a violating routed net");
        return;
    }
    // :43-49. "DSN-imported wiring is fixed; freshly routed items never are. Unfix so the rip
    // behaves as it does for router-inserted items." — `removeItemsAndPullTight`'s refusal is
    // `isUserFixed()`, and `BasicBoard.removeItems` re-tests `isDeletionForbidden()`.
    unfix_all_wiring(&mut board);
    // :50-55.
    let traces_before = count_traces_on_net(&board, net_number);
    // :55 — `assumeTrue`.
    if traces_before == 0 {
        eprintln!("skipped: the violating net carries no traces");
        return;
    }

    // :57-58. "Treat the whole net's wiring as 'newly inserted' (max id 0): the rip must fire."
    // `ItemId(0)` is the port's `maxItemIdBefore = 0`: ids start at 1
    // (`ItemIdGenerator.java:37-55`), so every item is strictly greater.
    let result = BatchAutorouter::enforce_strict_drc(&mut board, net_number, ItemId(0));

    // :60-61.
    let result = result.expect("violating connection must be rejected");
    assert_eq!(AutorouteAttemptState::Failed, result.state);
    // The details string is `enforceStrictDrc:326-328`'s, which Java's own test does not check
    // and Task 9's failure log will write out verbatim.
    assert!(
        result
            .details
            .as_deref()
            .is_some_and(|d| d.starts_with("strict_drc: connection ripped because ")),
        "unexpected details: {:?}",
        result.details
    );
    // :62-65.
    let traces_after = count_traces_on_net(&board, net_number);
    assert!(
        traces_after < traces_before,
        "violating wiring must have been removed ({traces_before} -> {traces_after})"
    );
}

/// `StrictDrcEnforcementTest.keepsConnectionsWhoseNewItemsAreClean` (`:67-79`).
#[test]
fn keeps_connections_whose_new_items_are_clean() {
    if !parity::require_java_dir() {
        return;
    }
    // :70-71.
    let mut board = load_board();
    let net_number = violating_net(&mut board);
    // :72 — `assumeTrue`.
    if net_number <= 0 {
        eprintln!("skipped: fixture must contain a violating routed net");
        return;
    }
    // :73-74.
    let max_id = board.communication.id_gen.max_generated_id();
    let before = BoardStatistics::new(&mut board);

    // :76-77. "Nothing is newer than maxId, so nothing may be ripped regardless of violations."
    assert!(
        BatchAutorouter::enforce_strict_drc(&mut board, net_number, max_id).is_none(),
        "a connection that inserted nothing must be kept"
    );
    // :78-79.
    let after = BoardStatistics::new(&mut board);
    assert_eq!(
        before.clearance_violations.total_count, after.clearance_violations.total_count,
        "a kept connection must leave the violation count alone"
    );
}

/// The brief's extra assertion, which the Java suite has no counterpart for: after
/// `applyStrictDrcAfterRoute` rejects, the board must be **exactly** the board that was cloned
/// before the route, not merely one with the new items gone.
///
/// This runs the real path. `route_connection_full` takes the `:84-85` clone (because
/// `settings.strict_drc` is on), routes, calls `enforceStrictDrc` at `:249`, and — this is the
/// statement under test — executes `:250`'s two-part guard and `:251`'s whole-board replacement,
/// which plan-7 ruling 8 makes `*board = snapshot`.
///
/// # Why the assertion needs two halves
///
/// `enforceStrictDrc` alone removes the *new* trace/via items, so a board that had only those
/// added would look restored after the rip. Two things separate the rip from the restore:
///
/// * **`structural_hash`** — the rip does not undo what step 6's `optChangedArea` did to
///   *pre-existing* traces, and on this fixture it did plenty;
/// * **`maxGeneratedId`** — Java's `BasicBoard.deserialize` restores the whole object graph,
///   `communication.idGenerator` included, and so does `Board: Clone`. The rip cannot rewind a
///   counter.
///
/// Both are asserted. **Measured RED/GREEN**: disabling the `:250-252` arm leaves the hash
/// different and the id generator advanced 1398 -> 1427, so each half fails on its own.
///
/// # The fixture and the connection
///
/// `Issue575-…_clearance_violations.dsn` with every trace and via unfixed — DSN-imported wiring
/// is `USER_FIXED`, and freshly routed items never are, so unfixing is what makes the board
/// behave the way it does mid-pass. Connection **6** of the driver's list (net 1) is the one
/// whose newly inserted items carry violations; the four before it are routed first because the
/// board state they leave is what makes connection 6 violate.
#[test]
fn a_rejected_connection_restores_the_pre_route_board_exactly() {
    if !parity::require_java_dir() {
        return;
    }
    let mut board = load_board();
    unfix_all_wiring(&mut board);

    let mut settings = DefaultSettings::new(&HostEnvironment::detect())
        .get_settings()
        .expect("DefaultSettings always answers a table")
        .clone();
    settings.set_layer_count(board.get_layer_count());
    settings.apply_board_specific_optimizations(&board);
    // `RouterSettings.isStrictDrc` (`:531-534`) — `DefaultSettings.java:110` seeds `false`, so a
    // production run never reaches `applyStrictDrcAfterRoute`'s body at all.
    settings.strict_drc = Some(true);
    assert!(settings.is_strict_drc());

    let trace_costs = settings.get_trace_costs();
    let mut engine: Option<AutorouteEngine> = None;
    let mut rejected = 0;

    for (k, (item_id, net_no)) in pick_connections(&board, 6).into_iter().enumerate() {
        if board.get_item(item_id).is_none() {
            continue;
        }
        // The precondition `docs/plan-6-handoff.md` §3 states.
        board.start_marking_changed_area();
        let hash_before = board.structural_hash();
        let id_before = board.communication.id_gen.max_generated_id();

        let result = route_connection_full(
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
            settings.trace_pull_tight_accuracy.unwrap_or(500),
            RouterBudget::disabled(),
            &|| false,
        );

        let Some(details) = result.details.as_deref() else {
            continue;
        };
        if !details.starts_with("strict_drc: ") {
            continue;
        }
        rejected += 1;
        assert_eq!(
            6,
            k + 1,
            "connection 6 is the one that violates on this fixture"
        );
        // `enforceStrictDrc:324-328` — the state and the message Task 9's failure log writes out.
        assert_eq!(AutorouteAttemptState::Failed, result.state);
        assert_eq!(
            "strict_drc: connection ripped because 4 new item(s) included clearance violations",
            details
        );
        // `applyStrictDrcAfterRoute:250-252` — the restore, both halves.
        assert_eq!(
            hash_before,
            board.structural_hash(),
            "the rollback must put the whole board back, not merely rip the new items"
        );
        assert_eq!(
            id_before,
            board.communication.id_gen.max_generated_id(),
            "…including `communication.idGenerator`, which the rip cannot rewind"
        );
    }
    assert_eq!(
        1, rejected,
        "exactly one of the first six connections must be rejected, or the test proves nothing"
    );
}

/// The Java suite's `:43-49` unfix loop, hoisted: "DSN-imported wiring is fixed; freshly routed
/// items never are."
fn unfix_all_wiring(board: &mut Board) {
    for id in board.items_in_board_order() {
        let is_wiring = board
            .get_item(id)
            .is_some_and(|item| matches!(item, Item::Trace(_) | Item::Via(_)));
        if is_wiring && let Some(item) = board.items.get_mut(&id) {
            item.set_fixed_state(FixedState::Unfixed);
        }
    }
}

/// `P6T1.pickConnections`, the driver's own connection list.
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

/// `board.getItems().stream().filter(it -> it instanceof Trace && it.containsNet(netNumber))
/// .count()` (`:51-54`, `:62-65`).
fn count_traces_on_net(board: &Board, net_number: i32) -> usize {
    board
        .items_in_board_order()
        .into_iter()
        .filter(|id| {
            board
                .get_item(*id)
                .is_some_and(|item| matches!(item, Item::Trace(_)) && item.contains_net(net_number))
        })
        .count()
}
