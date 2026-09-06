//! The per-connection search cap: a bound on the maze steps one connection may spend, so a
//! hopeless connection cannot eat a job's whole deadline.

use fr_board::prelude::*;
use fr_dsn::{BoardReadResult, DsnReadOptions};
use fr_router::pipeline::{
    BatchAutorouter, ConnectionBudget, DEFAULT_CONNECTION_SEARCH_STEPS, NoopProgressSink,
    RouterBudget, RouterStop, RoutingFailureLog,
};
use fr_settings::RouterSettings;
use fr_settings::sources::DefaultSettings;
use fr_settings::{HostEnvironment, SettingsSource};

#[test]
fn the_default_step_cap_is_a_quarter_million_and_zero_lifts_it() {
    let budget = ConnectionBudget::start(&RouterSettings::new());
    assert_eq!(budget.step_cap(), Some(DEFAULT_CONNECTION_SEARCH_STEPS));
    assert_eq!(DEFAULT_CONNECTION_SEARCH_STEPS, 250_000);
    for _ in 0..1000 {
        assert!(!budget.exceeded());
    }

    let mut uncapped = RouterSettings::new();
    uncapped.connection_search_steps = Some(0);
    let budget = ConnectionBudget::start(&uncapped);
    assert!(budget.step_cap().is_none());
    for _ in 0..1000 {
        assert!(!budget.exceeded());
    }
}

#[test]
fn a_step_cap_trips_after_that_many_polls() {
    let mut settings = RouterSettings::new();
    settings.connection_search_steps = Some(3);
    let budget = ConnectionBudget::start(&settings);
    assert_eq!(budget.step_cap(), Some(3));
    assert!(!budget.exceeded());
    assert!(!budget.exceeded());
    assert!(!budget.exceeded());
    assert!(budget.exceeded(), "the fourth poll is over the cap");
    assert!(budget.exceeded(), "and it stays tripped");
}

#[test]
fn the_resolved_defaults_carry_the_cap() {
    let settings = DefaultSettings::new(&HostEnvironment::detect())
        .get_settings()
        .expect("DefaultSettings always answers a table")
        .clone();
    assert_eq!(settings.connection_search_steps, Some(250_000));
}

fn load_rpi() -> Board {
    let path = parity::java_dir().join("fixtures/Issue143-rpi_splitter.dsn");
    let bytes = std::fs::read(&path).expect("the rpi fixture is in the corpus");
    match fr_dsn::read_board(
        &bytes[..],
        None,
        Some("Issue143-rpi_splitter.dsn"),
        &DsnReadOptions::default(),
    ) {
        BoardReadResult::Success { board, .. } => *board.expect("a board"),
        other => panic!("rpi_splitter did not read: {other:?}"),
    }
}

fn rpi_settings(board: &Board) -> RouterSettings {
    let mut settings = DefaultSettings::new(&HostEnvironment::detect())
        .get_settings()
        .expect("DefaultSettings always answers a table")
        .clone();
    settings.set_layer_count(board.get_layer_count());
    settings.apply_board_specific_optimizations(board);
    settings
}

fn routed_in_one_pass(settings: &RouterSettings) -> i32 {
    let mut board = load_rpi();
    let mut router = BatchAutorouter::for_routing_job(&board, settings, RouterBudget::disabled());
    let stop = RouterStop::new();
    let mut failure_log = RoutingFailureLog::new();
    let before = router.autoroute_items(&board).len();
    router
        .autoroute_pass(
            &mut board,
            &mut failure_log,
            1,
            &stop,
            &mut NoopProgressSink,
        )
        .expect("the pass runs");
    assert!(
        !stop.is_stop_requested(),
        "a tripped connection cap fails that connection, it does not stop the pass"
    );
    i32::try_from(before - router.autoroute_items(&board).len()).expect("fits")
}

#[test]
#[cfg_attr(debug_assertions, ignore = "routes rpi_splitter; run with --release")]
fn a_tiny_step_cap_fails_connections_without_stopping_the_pass() {
    if !parity::require_java_dir() {
        return;
    }
    let board = load_rpi();
    let mut generous = rpi_settings(&board);
    generous.connection_search_steps = Some(50_000_000);
    let mut starved = rpi_settings(&board);
    starved.connection_search_steps = Some(1);

    let with_generous_cap = routed_in_one_pass(&generous);
    let with_starved_cap = routed_in_one_pass(&starved);
    assert!(
        with_generous_cap > 0,
        "the board routes under a generous cap"
    );
    assert!(
        with_starved_cap < with_generous_cap,
        "one step per connection routes less: {with_starved_cap} vs {with_generous_cap}"
    );
}
