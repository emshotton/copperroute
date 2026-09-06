use std::collections::{BTreeMap, BTreeSet};

use fr_board::prelude::*;
use fr_drc::{DesignRulesChecker, DrcViolation};
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

struct PassResult {
    routed: usize,
    incomplete_connections: usize,
    clearance_violations: usize,
}

fn load_board(rel_path: &str) -> Board {
    let path = parity::reference_dir().join(rel_path);
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

fn route_one_pass(dsn: &str, k: usize) -> PassResult {
    let mut board = load_board(dsn);
    let settings = build_settings(&board);
    let trace_costs = settings.get_trace_costs();
    let connections = pick_connections(&board, k);
    let routed = connections.len();

    for (item_id, net_no) in connections {
        if board.get_item(item_id).is_none() {
            continue;
        }
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
    let incomplete_connections = drc.get_incomplete_count();
    let violations = drc.get_all_violations();
    let clearance_violations: Vec<DrcViolation> = violations
        .into_iter()
        .filter(|violation| violation.involves_routing(&board))
        .collect();
    PassResult {
        routed,
        incomplete_connections,
        clearance_violations: clearance_violations.len(),
    }
}

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

#[test]
fn dac2020_bm01_one_pass_two_items_leaves_at_most_194_incompletes() {
    if !parity::require_reference_dir() {
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
    if !parity::require_reference_dir() {
        return;
    }
    let result = route_one_pass("fixtures/Issue143-rpi_splitter.dsn", 8);
    check("Issue143-rpi_splitter.dsn", &result, 2);
}

#[cfg_attr(debug_assertions, ignore)]
#[test]
fn j2_reference_one_pass_leaves_at_most_seven_incompletes() {
    if !parity::require_reference_dir() {
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
    if !parity::require_reference_dir() {
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
    if !parity::require_reference_dir() {
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
    if !parity::require_reference_dir() {
        return;
    }
    let result = route_one_pass("examples/tutorial_board/tutorial_board.dsn", 100_000);
    assert_eq!(result.routed, 0, "the board has no candidate connections");
    check("tutorial_board.dsn", &result, 0);
}

struct JobResult {
    passes_run: i32,
    incomplete_connections: usize,
    clearance_violations: usize,
    drill_item_count: i32,
}

fn run_job(dsn: &str, max_passes: i32, max_items: Option<i32>, strict_drc: bool) -> JobResult {
    let path = parity::reference_dir().join(dsn);
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

#[cfg_attr(debug_assertions, ignore)]
#[test]
fn dac2020_bm01_pipeline_first_2_nets_leaves_at_most_194_incompletes() {
    if !parity::require_reference_dir() {
        return;
    }
    let result = run_job("fixtures/Issue508-DAC2020_bm01.dsn", 1, Some(2), false);
    check_job("Issue508-DAC2020_bm01.dsn", &result, 194, 0);
}

/// `issue508Bm01First43NetsOnly` (`:34-48`): `maxItems(43)`, `maxIncompleteConnections(161)`.
#[cfg_attr(debug_assertions, ignore)]
#[test]
fn dac2020_bm01_pipeline_first_43_nets_leaves_at_most_161_incompletes() {
    if !parity::require_reference_dir() {
        return;
    }
    let result = run_job("fixtures/Issue508-DAC2020_bm01.dsn", 1, Some(43), false);
    check_job("Issue508-DAC2020_bm01.dsn", &result, 161, 0);
}

/// `issue508Bm01First61NetsOnly` (`:50-64`): `maxItems(61)`, `maxIncompleteConnections(147)`.
#[cfg_attr(debug_assertions, ignore)]
#[test]
fn dac2020_bm01_pipeline_first_61_nets_leaves_at_most_147_incompletes() {
    if !parity::require_reference_dir() {
        return;
    }
    let result = run_job("fixtures/Issue508-DAC2020_bm01.dsn", 1, Some(61), false);
    check_job("Issue508-DAC2020_bm01.dsn", &result, 147, 0);
}

/// `issue508Bm01First111NetsOnly` (`:66-80`): `maxItems(111)`, `maxIncompleteConnections(134)`.
#[cfg_attr(debug_assertions, ignore)]
#[test]
fn dac2020_bm01_pipeline_first_111_nets_leaves_at_most_134_incompletes() {
    if !parity::require_reference_dir() {
        return;
    }
    let result = run_job("fixtures/Issue508-DAC2020_bm01.dsn", 1, Some(111), false);
    check_job("Issue508-DAC2020_bm01.dsn", &result, 134, 0);
}

/// `issue508Bm01First151NetsOnly` (`:82-96`): `maxItems(151)`, `maxIncompleteConnections(126)`.
#[cfg_attr(debug_assertions, ignore)]
#[test]
fn dac2020_bm01_pipeline_first_151_nets_leaves_at_most_126_incompletes() {
    if !parity::require_reference_dir() {
        return;
    }
    let result = run_job("fixtures/Issue508-DAC2020_bm01.dsn", 1, Some(151), false);
    check_job("Issue508-DAC2020_bm01.dsn", &result, 126, 0);
}

#[cfg_attr(debug_assertions, ignore)]
#[test]
fn dac2020_bm01_pipeline_one_pass_leaves_at_most_71_incompletes() {
    if std::env::var_os("FR_SLOW_PARITY").is_none() || !parity::require_reference_dir() {
        return;
    }
    let result = run_job("fixtures/Issue508-DAC2020_bm01.dsn", 1, None, false);
    check_job("Issue508-DAC2020_bm01.dsn", &result, 71, 0);
}

#[cfg_attr(debug_assertions, ignore)]
#[test]
fn dac2020_bm01_pipeline_two_passes_leave_at_most_37_incompletes() {
    if std::env::var_os("FR_SLOW_PARITY").is_none() || !parity::require_reference_dir() {
        return;
    }
    let result = run_job("fixtures/Issue508-DAC2020_bm01.dsn", 2, None, false);
    check_job("Issue508-DAC2020_bm01.dsn", &result, 37, 0);
}

#[cfg_attr(debug_assertions, ignore)]
#[test]
fn j2_reference_pipeline_leaves_at_most_three_incompletes_and_under_sixty_drills() {
    if !parity::require_reference_dir() {
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

#[cfg_attr(debug_assertions, ignore)]
#[test]
fn strict_drc_cnh_pipeline_adds_no_violations_beyond_the_sixteen_pre_existing() {
    if std::env::var_os("FR_SLOW_PARITY").is_none() || !parity::require_reference_dir() {
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

/// The `board_edge` clearance class index and the outline's class, after running the whole
/// [`prepare_board`] seam with `copper_to_edge_clearance_um` set to `configured`.
///
/// `None` for `configured` is "no source supplied the value", which is the shape
/// `RouterSettings` carries until one does — the provenance half of the fix.
fn copper_override_outcome(dsn: &str, configured: Option<f64>) -> (Option<usize>, usize) {
    let mut board = load_board(dsn);
    let mut settings = build_settings(&board);
    settings.copper_to_edge_clearance_um = configured;
    // The hole override is a separate knob and would append a second class; hold it out so the
    // class indices below say only what this test is about.
    settings.hole_clearance_um = None;
    prepare_board(&mut board, &settings);
    let board_edge = board.rules.clearance_matrix.get_no("board_edge");
    let outline = board.get_outline().expect("the fixture has an outline");
    let outline_class = board
        .get_item(outline)
        .expect("the outline is an item")
        .clearance_class();
    (board_edge, outline_class)
}

#[cfg_attr(debug_assertions, ignore)]
#[test]
fn the_copper_to_edge_override_is_continuous() {
    if !parity::require_reference_dir() {
        return;
    }
    let dsn = "fixtures/Issue143-rpi_splitter.dsn";

    // The provenance case, and the baseline for everything below: no source supplied the value,
    // so the outline keeps the class its own DSN gave it and no `board_edge` class exists.
    let (unsupplied_edge, unsupplied_outline) = copper_override_outcome(dsn, None);
    assert_eq!(
        unsupplied_edge, None,
        "with no source supplying the value the override must not run at all"
    );

    // The three supplied values. Every one of them fires, and all three land the outline on the
    // same appended `board_edge` class — which is what "continuous" means.
    let supplied = [
        ("the default, typed", 500.0_f64),
        ("a hair above the default", 500.000_001_f64),
        ("zero", 0.0_f64),
    ];
    let mut outcomes = Vec::new();
    for (name, value) in supplied {
        let (edge, outline_class) = copper_override_outcome(dsn, Some(value));
        let edge = edge.unwrap_or_else(|| {
            panic!("{name} ({value}): an explicitly supplied value must append board_edge")
        });
        assert_eq!(
            outline_class, edge,
            "{name} ({value}): the outline must be re-pointed at board_edge"
        );
        assert_ne!(
            outline_class, unsupplied_outline,
            "{name} ({value}): this board's outline carries an explicit DSN class, so the \
             re-pointing is observable"
        );
        outcomes.push((name, value, edge, outline_class));
    }
    let (first_name, first_value, first_edge, first_outline) = outcomes[0];
    for &(name, value, edge, outline_class) in &outcomes[1..] {
        assert_eq!(
            (edge, outline_class),
            (first_edge, first_outline),
            "{name} ({value}) and {first_name} ({first_value}) must be indistinguishable — \
             quirk #231's discontinuity at the default is what this test exists to forbid"
        );
    }
}
