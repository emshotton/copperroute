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
//! # The multi-pass half (Plan 7 Task 16)
//!
//! Plan 6 could only run one pass, because the pass loop was Plan 7's. It exists now, so the
//! second half of this file runs [`fr_router::pipeline::run_pipeline`] — the whole `-de/-do`
//! flow, fanout and optimizer included, on a board prepared by
//! [`fr_router::pipeline::prepare_board`] and settings resolved by
//! [`fr_settings::resolve_headless`] (controller ruling AW) — and asserts the bounds the Java
//! fixture suite asserts on the *same* runs:
//!
//! * `Dac2020BenchmarkRoutingTest.java`'s seven `Issue508-DAC2020_bm01.dsn` rows (`:19`, `:34`,
//!   `:50`, `:66`, `:82`, `:98`, `:114`): `maxItems` 2/43/61/111/151 at one pass →
//!   ≤ 194/161/147/134/126 incomplete, then the whole board at `maxPasses` 1 → ≤ 56 and at 2 →
//!   ≤ 28;
//! * `J2ReferenceRoutingTest.java:11-31`: `maxPasses(99)`, ≤ 3 incomplete, **exactly** 0
//!   clearance violations, and `statsAfter.items.drillItemCount < 60`;
//! * `StrictDrcRoutingTest.java:17-30`: `strict_drc = true` on
//!   `Issue555-CNH_Functional_Tester_1.dsn`, **exactly 16** clearance violations — the fixture's
//!   pre-existing ones, none added — and ≤ 30 incomplete.
//!
//! **Quirk #189 governs every one of those bounds**, not just the one-pass ones: it is why
//! `maxPasses(1)` in a test body survives `RoutingFixtureTest.getRoutingJob:76`'s
//! `setMaxPasses(100)`, and equally why the two tests that set *no* `maxPasses` — `J2Reference`
//! and `StrictDrc` — really do run at **100**. Read the setter the other way round and every
//! bound in this file would be a hundred-pass bound. (The task brief cites #157 here; #157 is
//! `CompleteFreeSpaceExpansionRoom.compareTo`, which has nothing to do with the fixture suite.
//! #189 is the row the brief means and the one the whole file is written against.)
//!
//! `jobTimeoutString` is first-writer-wins by the same mechanism, so a test's own
//! `"00:00:30"`/`"00:05:00"` survives `:75`'s `"00:01:00"`. Nothing here has a wall clock —
//! ruling AI disables every budget on the port's side — so the timeouts are recorded in the doc
//! comments and not asserted; a bound this harness *cannot* express is better named than
//! silently approximated.
//!
//! # What runs in CI
//!
//! Only the smoke test. The rest carry `#[cfg_attr(debug_assertions, ignore)]` (Plan 3's
//! convention), because a full board unoptimised is minutes of work.
//!
//! **The command that runs them is a plain `cargo test --release -p fr-router --test fixtures`,
//! not `-- --ignored`.** `debug_assertions` is off in a release build, so the `cfg_attr` does not
//! apply and the five are not ignored there — adding `--ignored` filters all six *out* and runs
//! zero tests. All six take about 14 s together.

use std::collections::{BTreeMap, BTreeSet};

use fr_board::prelude::*;
use fr_drc::DesignRulesChecker;
use fr_dsn::{BoardReadResult, DsnReadOptions};
use fr_router::pipeline::{
    NoopProgressSink, RouterBudget, RouterStop, prepare_board, run_pipeline,
};
use fr_router::route_connection;
use fr_settings::sources::{CliSettings, DefaultSettings, DsnFileSettings};
use fr_settings::{
    HostEnvironment, RouterSettings, SettingsInputs, SettingsSource, resolve_headless,
};

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
/// `ignore`. Two connections on this board is **0.08 s** in a debug build — the 30 KB DSN read
/// is nearly all of it, because two connections build very little search tree.
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

// ---------------------------------------------------------------------------------------------
// The multi-pass harness (Plan 7 Task 16) — the whole pipeline, as the CLI runs it
// ---------------------------------------------------------------------------------------------

/// What the Java assertion family reads off a finished job: `BoardStatistics.connections
/// .incompleteCount`, `.clearanceViolations.totalCount` and `.items.drillItemCount`
/// (`BoardStatistics.java:271`, `:277`, `:21-22`), plus the pass count the loop stopped at.
struct JobResult {
    passes_run: i32,
    incomplete_connections: usize,
    clearance_violations: usize,
    drill_item_count: i32,
}

/// `RoutingFixtureTest.getRoutingJob` + `runRoutingJob`, port side.
///
/// The Java suite builds `DefaultSettings` + `DsnFileSettings` + its `TestingSettings`
/// (priority 80) and lets the scheduler re-merge with `ApiSettings(job.routerSettings)`
/// (priority 70). This side is [`resolve_headless`] — that whole ladder as one linear pass,
/// byte-pinned against the JVM by Plan 4's `p4t1` — with the test's three overrides carried by a
/// real [`CliSettings`] at priority 60. Sixty rather than eighty is the one deliberate
/// simplification, and it is observationally identical here: no source between 60 and 80 writes
/// `max_passes`, `max_items` or `strict_drc` on any of these boards, so the field the test sets is
/// the field that survives either way.
///
/// Then [`prepare_board`] (controller ruling AW — `HeadlessBoardManager.java:746-747` mutates
/// the board on load, and the Java fixture suite loads through the manager) and [`run_pipeline`]
/// with every budget disabled (ruling AI).
fn run_job(dsn: &str, max_passes: i32, max_items: Option<i32>, strict_drc: bool) -> JobResult {
    let path = parity::java_dir().join(dsn);
    let bytes =
        std::fs::read(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    let file_name = path
        .file_name()
        .expect("a file name")
        .to_string_lossy()
        .into_owned();
    let mut board = load_board(dsn);

    let mut argv = vec![
        "-de".to_string(),
        path.display().to_string(),
        "-do".to_string(),
        path.display().to_string().replace(".dsn", ".ses"),
        "-mp".to_string(),
        max_passes.to_string(),
        // `TestingSettings`' **constructor**
        // (`src/test/java/app/freerouting/settings/sources/TestingSettings.java:23-25`):
        //
        //     // Keep legacy fixture expectations stable unless a test explicitly opts in.
        //     this.settings.copperToEdgeClearanceUm = 0.0;
        //
        // Every job in the fixture suite carries it — a test that passes no `TestingSettings` gets
        // one built for it at `RoutingFixtureTest.java:71-73`. It is not a detail: `0.0` is not the
        // `DEFAULT_COPPER_TO_EDGE_CLEARANCE_UM` 500.0 that a real `-de/-do` run uses, so
        // `applyCopperToEdgeClearanceOverride` writes **zero** into the `board_edge` row instead of
        // 500 µm and the fixture suite routes a materially more permissive board than the CLI does
        // (quirk #231; Task 15b measured the same board's SES at 15 254 B against 14 644 B for
        // exactly this switch). Measured here: with the CLI's 500 µm,
        // `Issue508-DAC2020_bm01.dsn` at `maxPasses = 2` leaves **34** incomplete on the HEAD jar
        // itself, above `issue508Bm01First2PassesOnly`'s bound of 28; with the fixture suite's
        // `0.0` both sides come in under it — the port leaves 27. Leaving this line out would make every bound below a bound on
        // a board the Java suite never routes.
        "--router.copper_to_edge_clearance_um=0.0".to_string(),
    ];
    if let Some(items) = max_items {
        argv.push(format!("--router.max_items={items}"));
    }
    if strict_drc {
        argv.push("--router.strict_drc=true".to_string());
    }
    let dsn_source = DsnFileSettings::new(&bytes[..], &file_name);
    let cli_source = CliSettings::new(&argv);
    let inputs = SettingsInputs {
        json_file: None,
        dsn: dsn_source.get_settings(),
        cli_rules: None,
        scheduler_rules: None,
        env: None,
        cli: cli_source.get_settings(),
    };
    let settings = resolve_headless(&inputs, Some(&board), &HostEnvironment::detect());
    assert_eq!(
        settings.max_passes,
        Some(max_passes),
        "{dsn}: the CLI source did not reach settings.max_passes"
    );
    if let Some(items) = max_items {
        assert_eq!(
            settings.max_items,
            Some(items),
            "{dsn}: the CLI source did not reach settings.max_items"
        );
    }
    assert_eq!(
        settings.strict_drc,
        Some(strict_drc),
        "{dsn}: the CLI source did not reach settings.strict_drc"
    );
    assert_eq!(
        settings.copper_to_edge_clearance_um,
        Some(0.0),
        "{dsn}: TestingSettings' constructor value did not reach the settings"
    );
    prepare_board(&mut board, &settings);

    let stop = RouterStop::new();
    let mut sink = NoopProgressSink;
    let result = run_pipeline(
        &mut board,
        &settings,
        &stop,
        RouterBudget::disabled(),
        &mut sink,
    )
    .unwrap_or_else(|e| panic!("{dsn}: the pipeline failed: {e:?}"));

    let statistics = result.final_statistics;
    JobResult {
        passes_run: result.router_passes_completed,
        // `Option` because `BoardStatistics`' DTOs mirror Gson's nullable fields; the pipeline
        // always fills them, and a `None` here would be a port bug rather than a routing result.
        incomplete_connections: usize::try_from(
            statistics
                .connections
                .incomplete_count
                .expect("the pipeline always computes an incomplete count"),
        )
        .expect("a non-negative incomplete count"),
        clearance_violations: usize::try_from(
            statistics
                .clearance_violations
                .total_count
                .expect("the pipeline always computes a violation count"),
        )
        .expect("a non-negative violation count"),
        drill_item_count: statistics.items.drill_item_count.unwrap_or(0),
    }
}

/// `RoutingResultAssertions.check()`'s two counted arms (`RoutingFixtureTest.java:365-429`), in
/// the Java vocabulary. `exact_violations` is `exactClearanceViolations`, `None` is "not asserted".
fn check_job(name: &str, result: &JobResult, max_incomplete: usize, exact_violations: usize) {
    assert!(
        result.incomplete_connections <= max_incomplete,
        "'{name}' should have at most {max_incomplete} unrouted connection(s), but had {} \
         (after {} pass(es)).",
        result.incomplete_connections,
        result.passes_run
    );
    assert_eq!(
        result.clearance_violations, exact_violations,
        "'{name}' should have exactly {exact_violations} clearance violation(s), but had {}.",
        result.clearance_violations
    );
}

// --- Dac2020BenchmarkRoutingTest.java, all seven Issue508-DAC2020_bm01 rows -------------------

/// `Dac2020BenchmarkRoutingTest.issue508Bm01First2NetsOnly` (`:19-32`): `setMaxPasses(1)`,
/// `setMaxItems(2)`, `maxIncompleteConnections(194)`. Java's `"00:00:30"` job timeout is not
/// asserted (see the module doc).
#[cfg_attr(debug_assertions, ignore)]
#[test]
fn dac2020_bm01_pipeline_first_2_nets_leaves_at_most_194_incompletes() {
    if !parity::require_java_dir() {
        return;
    }
    let result = run_job("fixtures/Issue508-DAC2020_bm01.dsn", 1, Some(2), false);
    check_job("Issue508-DAC2020_bm01.dsn", &result, 194, 0);
}

/// `issue508Bm01First43NetsOnly` (`:34-48`): `maxItems(43)`, `maxIncompleteConnections(161)`.
#[cfg_attr(debug_assertions, ignore)]
#[test]
fn dac2020_bm01_pipeline_first_43_nets_leaves_at_most_161_incompletes() {
    if !parity::require_java_dir() {
        return;
    }
    let result = run_job("fixtures/Issue508-DAC2020_bm01.dsn", 1, Some(43), false);
    check_job("Issue508-DAC2020_bm01.dsn", &result, 161, 0);
}

/// `issue508Bm01First61NetsOnly` (`:50-64`): `maxItems(61)`, `maxIncompleteConnections(147)`.
#[cfg_attr(debug_assertions, ignore)]
#[test]
fn dac2020_bm01_pipeline_first_61_nets_leaves_at_most_147_incompletes() {
    if !parity::require_java_dir() {
        return;
    }
    let result = run_job("fixtures/Issue508-DAC2020_bm01.dsn", 1, Some(61), false);
    check_job("Issue508-DAC2020_bm01.dsn", &result, 147, 0);
}

/// `issue508Bm01First111NetsOnly` (`:66-80`): `maxItems(111)`, `maxIncompleteConnections(134)`.
#[cfg_attr(debug_assertions, ignore)]
#[test]
fn dac2020_bm01_pipeline_first_111_nets_leaves_at_most_134_incompletes() {
    if !parity::require_java_dir() {
        return;
    }
    let result = run_job("fixtures/Issue508-DAC2020_bm01.dsn", 1, Some(111), false);
    check_job("Issue508-DAC2020_bm01.dsn", &result, 134, 0);
}

/// `issue508Bm01First151NetsOnly` (`:82-96`): `maxItems(151)`, `maxIncompleteConnections(126)`.
#[cfg_attr(debug_assertions, ignore)]
#[test]
fn dac2020_bm01_pipeline_first_151_nets_leaves_at_most_126_incompletes() {
    if !parity::require_java_dir() {
        return;
    }
    let result = run_job("fixtures/Issue508-DAC2020_bm01.dsn", 1, Some(151), false);
    check_job("Issue508-DAC2020_bm01.dsn", &result, 126, 0);
}

/// `issue508Bm01FirstPassOnly` (`:98-112`): the whole board at `maxPasses(1)`, with Java's
/// `maxIncompleteConnections(56)` **replaced by the port's own bound**.
///
/// PORT-REGRESSION PIN — re-cut at the M1 accept wave (ruling BV). The jar-parity bound was
/// **56**, transcribed from `Dac2020BenchmarkRoutingTest`; the port leaves **69** since Plan 9
/// Task 2's R1 (#293) restored the airline-first work-list order. This is R1's cost on the
/// corpus's hardest board and it is the sharpest single number Task 2 produced — the same change
/// takes the 605-board corpus's clean-pass rate from 0.375 to 0.550 (M1, `plan9-m1` vs
/// `java-278fe14`), which is why ruling BV accepts it with the gap recorded rather than reverting.
/// The test was renamed with its literal (`…_at_most_56_…` -> `…_at_most_69_…` -> `…_71_…`).
/// PORT-REGRESSION PIN re-cut at plan9-t7t8 (ruling CE): T8's door fixes moved this ONE-PASS
/// intermediate 69 -> 71, while the multi-pass -mp 10 final IMPROVED (dac2020 38 -> 30 incompletes,
/// now 1 ahead of the reference; corpus clean-pass 0.550 -> 0.567 at plan9-t7t8). The intermediate
/// is a stale midpoint, not a regression; the final is a win.
#[cfg_attr(debug_assertions, ignore)]
#[test]
fn dac2020_bm01_pipeline_one_pass_leaves_at_most_71_incompletes() {
    if !parity::require_java_dir() {
        return;
    }
    let result = run_job("fixtures/Issue508-DAC2020_bm01.dsn", 1, None, false);
    check_job("Issue508-DAC2020_bm01.dsn", &result, 71, 0);
}

/// `issue508Bm01First2PassesOnly` (`:114-128`): the whole board at `maxPasses(2)`, with Java's
/// `maxIncompleteConnections(28)` **replaced by the port's own bound**.
///
/// PORT-REGRESSION PIN — re-cut at the M1 accept wave (ruling BV). The jar-parity bound was
/// **28**; the port leaves **37**, for the reason the one-pass row above states. Renamed with its
/// literal (`…_at_most_28_…` -> `…_at_most_37_…`).
///
/// This is `tests/reference/router-dac2020-bm01`'s configuration except for one setting, and the
/// difference is worth stating because it is the whole reason the two disagree: the reference
/// runs the **CLI's** 500 µm `copper_to_edge_clearance_um` and leaves 34 incomplete (measured on
/// the HEAD jar itself, `Auto-routing stage completed … final score: 825.63 (34 unrouted …)`),
/// while the fixture suite runs `TestingSettings`' `0.0` and gets under this bound. Same jar,
/// same board, same two passes — a 500 µm board-edge keep-out is the only thing between them.
#[cfg_attr(debug_assertions, ignore)]
#[test]
fn dac2020_bm01_pipeline_two_passes_leave_at_most_37_incompletes() {
    if !parity::require_java_dir() {
        return;
    }
    let result = run_job("fixtures/Issue508-DAC2020_bm01.dsn", 2, None, false);
    check_job("Issue508-DAC2020_bm01.dsn", &result, 37, 0);
}

// --- J2ReferenceRoutingTest.java --------------------------------------------------------------

/// `J2ReferenceRoutingTest.issue026AutorouterInterruptedAndConnectionsNotFound` (`:11-31`):
/// `maxPasses(99)`, `maxIncompleteConnections(3)`, `exactClearanceViolations(0)` and the
/// standalone `statsAfter.items.drillItemCount < 60` guard.
///
/// The test sets no `maxPasses` of its own, so quirk #189's `setMaxPasses(100)` at
/// `RoutingFixtureTest.java:76` is the value that lands; `maxPasses(99)` is the *assertion* on the
/// pass count the loop stopped at, which the comment there explains as stagnation detection
/// rather than a limit. Both halves are asserted here.
#[cfg_attr(debug_assertions, ignore)]
#[test]
fn j2_reference_pipeline_leaves_at_most_three_incompletes_and_under_sixty_drills() {
    if !parity::require_java_dir() {
        return;
    }
    let result = run_job("fixtures/Issue026-J2_reference.dsn", 100, None, false);
    assert!(
        result.passes_run <= 99,
        "'Issue026-J2_reference.dsn' should stop within 99 passes, but ran {}.",
        result.passes_run
    );
    assert!(
        result.drill_item_count < 60,
        "The drill item count should be less than 60, but was {}.",
        result.drill_item_count
    );
    check_job("Issue026-J2_reference.dsn", &result, 3, 0);
}

// --- StrictDrcRoutingTest.java ----------------------------------------------------------------

/// `StrictDrcRoutingTest.strictDrcDoesNotAddViolationsBeyondPreExisting` (`:17-30`):
/// `setStrictDrc(true)` on `Issue555-CNH_Functional_Tester_1.dsn`,
/// `exactClearanceViolations(16)` — the fixture's pre-existing violations, none added — and
/// `maxIncompleteConnections(30)`.
///
/// The test sets no `maxPasses`, so quirk #189 leaves `getRoutingJob:76`'s **100** in force. This
/// is the only fixture in the corpus whose board arrives with violations already on it, and so
/// the only one where `exactClearanceViolations` is a non-zero number: it is the assertion that
/// Task 8's `enforce_strict_drc` rollback works, because a strict-DRC pass that failed to roll
/// back would leave 17 or more.
#[cfg_attr(debug_assertions, ignore)]
#[test]
fn strict_drc_cnh_pipeline_adds_no_violations_beyond_the_sixteen_pre_existing() {
    if !parity::require_java_dir() {
        return;
    }
    let result = run_job(
        "fixtures/Issue555-CNH_Functional_Tester_1.dsn",
        100,
        None,
        true,
    );
    check_job("Issue555-CNH_Functional_Tester_1.dsn", &result, 30, 16);
}
