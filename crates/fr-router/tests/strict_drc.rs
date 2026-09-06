//! |---|---|
use std::collections::{BTreeMap, BTreeSet};

use fr_board::items::Item;
use fr_board::prelude::*;
use fr_board::structure::FixedState;
use fr_dsn::{BoardReadResult, DsnReadOptions};
use fr_router::autoroute::maze::ViaPricing;
use fr_router::pipeline::{BatchAutorouter, RouterBudget};
use fr_router::{AutorouteAttemptState, AutorouteEngine, BoardStatistics, route_connection_full};
use fr_settings::sources::DefaultSettings;
use fr_settings::{HostEnvironment, SettingsSource};

const FIXTURE: &str = "fixtures/Issue575-drc_BBD_Mars-64_6_track_1_hole_clearance_violations.dsn";

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

fn violating_net(board: &mut Board) -> i32 {
    for id in board.items_in_board_order() {
        let Some(item) = board.get_item(id) else {
            continue;
        };
        if !matches!(item, Item::Trace(_) | Item::Via(_)) || item.net_count() == 0 {
            continue;
        }
        if !board.clearance_violations(id).is_empty() {
            return board
                .get_item(id)
                .expect("still on the board")
                .get_net_number(0);
        }
    }
    -1
}

#[test]
fn rips_new_items_when_they_carry_violations() {
    if !parity::require_java_dir() {
        return;
    }
    let mut board = load_board();
    let net_number = violating_net(&mut board);
    if net_number <= 0 {
        eprintln!("skipped: fixture must contain a violating routed net");
        return;
    }
    unfix_all_wiring(&mut board);
    let traces_before = count_traces_on_net(&board, net_number);
    if traces_before == 0 {
        eprintln!("skipped: the violating net carries no traces");
        return;
    }

    let result = BatchAutorouter::enforce_strict_drc(&mut board, net_number, ItemId(0));

    let result = result.expect("violating connection must be rejected");
    assert_eq!(AutorouteAttemptState::Failed, result.state);
    assert!(
        result
            .details
            .as_deref()
            .is_some_and(|d| d.starts_with("strict_drc: connection ripped because ")),
        "unexpected details: {:?}",
        result.details
    );
    let traces_after = count_traces_on_net(&board, net_number);
    assert!(
        traces_after < traces_before,
        "violating wiring must have been removed ({traces_before} -> {traces_after})"
    );
}

#[test]
fn keeps_connections_whose_new_items_are_clean() {
    if !parity::require_java_dir() {
        return;
    }
    let mut board = load_board();
    let net_number = violating_net(&mut board);
    if net_number <= 0 {
        eprintln!("skipped: fixture must contain a violating routed net");
        return;
    }
    let max_id = board.communication.id_gen.max_generated_id();
    let before = BoardStatistics::new(&mut board);

    assert!(
        BatchAutorouter::enforce_strict_drc(&mut board, net_number, max_id).is_none(),
        "a connection that inserted nothing must be kept"
    );
    let after = BoardStatistics::new(&mut board);
    assert_eq!(
        before.clearance_violations.total_count, after.clearance_violations.total_count,
        "a kept connection must leave the violation count alone"
    );
}

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
    settings.strict_drc = Some(true);
    assert!(settings.is_strict_drc());

    let trace_costs = settings.get_trace_costs();
    let mut engine: Option<AutorouteEngine> = None;
    let mut rejected = 0;

    for (k, (item_id, net_no)) in pick_connections(&board, 6).into_iter().enumerate() {
        if board.get_item(item_id).is_none() {
            continue;
        }
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
            ViaPricing::ByPadstackRadius,
            &mut BTreeSet::new(),
            &mut BTreeMap::new(),
            1,
            settings.get_start_ripup_costs(),
            !settings.is_fanout_enabled(),
            settings.trace_pull_tight_accuracy.unwrap_or(500),
            RouterBudget::disabled(),
            &|| false,
            None,
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
        assert_eq!(AutorouteAttemptState::Failed, result.state);
        assert_eq!(
            "strict_drc: connection ripped because 4 new item(s) included clearance violations",
            details
        );
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
