use std::collections::BTreeMap;

use fr_dsn::{BoardReadResult, CoordinateTransform, DsnReadOptions};
use fr_router::autoroute::instrument::{self, Guard, Snapshot};
use fr_router::pipeline::{
    NoopProgressSink, RouterBudget, RouterStop, prepare_board, run_pipeline,
};
use fr_settings::sources::{CliSettings, DsnFileSettings, EnvironmentVariablesSource};
use fr_settings::{HostEnvironment, SettingsInputs, SettingsSource, resolve_headless};

struct Stem {
    name: &'static str,
    dsn: &'static str,
    max_passes: i32,
    fanout: bool,
    optimizer: bool,
    ci: bool,
}

const STEMS: &[Stem] = &[
    Stem {
        name: "router-rpi-splitter",
        dsn: "fixtures/Issue143-rpi_splitter.dsn",
        max_passes: 8,
        fanout: true,
        optimizer: true,
        ci: true,
    },
    Stem {
        name: "router-dac2020-bm01",
        dsn: "fixtures/Issue508-DAC2020_bm01.dsn",
        max_passes: 2,
        fanout: true,
        optimizer: true,
        ci: false,
    },
    Stem {
        name: "router-j2-reference",
        dsn: "fixtures/Issue026-J2_reference.dsn",
        max_passes: 99,
        fanout: true,
        optimizer: true,
        ci: true,
    },
    Stem {
        name: "router-tutorial-board",
        dsn: "examples/tutorial_board/tutorial_board.dsn",
        max_passes: 8,
        fanout: true,
        optimizer: true,
        ci: false,
    },
    Stem {
        name: "router-ecc83-input",
        dsn: "fixtures/Issue649-kicad_ecc83-pp_input_board_v1.dsn",
        max_passes: 8,
        fanout: true,
        optimizer: true,
        ci: true,
    },
    Stem {
        name: "router-fanout-bm11",
        dsn: "fixtures/Issue730-DAC2020_bm11.dsn",
        max_passes: 2,
        fanout: true,
        optimizer: false,
        ci: false,
    },
    Stem {
        name: "router-strict-drc-cnh",
        dsn: "fixtures/Issue555-CNH_Functional_Tester_1.dsn",
        max_passes: 2,
        fanout: true,
        optimizer: true,
        ci: false,
    },
    Stem {
        name: "router-empty-board",
        dsn: "fixtures/empty_board.dsn",
        max_passes: 1,
        fanout: false,
        optimizer: false,
        ci: true,
    },
];

struct Expected {
    stem: &'static str,
    fires: [u64; 5],
    visits: [u64; 5],
}

const MEASURED: &[Expected] = &[
    Expected {
        stem: "router-rpi-splitter",
        fires: [0, 0, 0, 0, 0],
        visits: [76, 76, 1042, 1042, 111],
    },
    Expected {
        stem: "router-dac2020-bm01",
        fires: [0, 0, 0, 0, 0],
        visits: [144_444, 144_444, 13_355_651, 13_355_651, 2_339_316],
    },
    Expected {
        stem: "router-j2-reference",
        fires: [0, 0, 0, 0, 0],
        visits: [35630, 35630, 296_273, 296_273, 21548],
    },
    Expected {
        stem: "router-tutorial-board",
        fires: [0, 0, 0, 0, 0],
        visits: [0, 0, 0, 0, 0],
    },
    Expected {
        stem: "router-ecc83-input",
        fires: [0, 0, 0, 0, 0],
        visits: [26, 26, 89, 89, 4],
    },
    Expected {
        stem: "router-fanout-bm11",
        fires: [0, 0, 0, 0, 0],
        visits: [17445, 17445, 80853, 80853, 6142],
    },
    Expected {
        stem: "router-strict-drc-cnh",
        fires: [0, 0, 0, 0, 0],
        visits: [90961, 90961, 939_226, 939_226, 61424],
    },
    Expected {
        stem: "router-empty-board",
        fires: [0, 0, 0, 0, 0],
        visits: [0, 0, 0, 0, 0],
    },
];

fn expected_for(stem: &str) -> &'static Expected {
    MEASURED
        .iter()
        .find(|row| row.stem == stem)
        .unwrap_or_else(|| panic!("no measured row for {stem} — add one and cite the report"))
}

struct Run {
    ses: String,
    snapshot: Snapshot,
    board: fr_board::Board,
}

fn route_stem(stem: &Stem, instrumented: bool) -> Run {
    let dsn = parity::java_dir().join(stem.dsn);
    let bytes =
        std::fs::read(&dsn).unwrap_or_else(|e| panic!("cannot read {}: {e}", dsn.display()));
    let file_name = dsn
        .file_name()
        .expect("a file name")
        .to_string_lossy()
        .into_owned();
    let design_name = file_name
        .rsplit_once('.')
        .map_or_else(|| file_name.clone(), |(base, _)| base.to_string());

    let (mut board, transform) = read_board(&dsn, &file_name);

    let argv = vec![
        "-de".to_string(),
        dsn.display().to_string(),
        "-do".to_string(),
        dsn.display().to_string().replace(".dsn", ".ses"),
        "-mp".to_string(),
        stem.max_passes.to_string(),
        format!("--router.fanout.enabled={}", stem.fanout),
        format!("--router.optimizer.enabled={}", stem.optimizer),
    ];
    let dsn_source = DsnFileSettings::new(&bytes[..], &file_name);
    let env_map: BTreeMap<String, String> = std::env::vars().collect();
    let env_source = EnvironmentVariablesSource::new(&env_map);
    let cli_source = CliSettings::new(&argv);
    let inputs = SettingsInputs {
        json_file: None,
        dsn: dsn_source.get_settings(),
        cli_rules: None,
        scheduler_rules: None,
        env: env_source.get_settings(),
        cli: cli_source.get_settings(),
    };
    let settings = resolve_headless(&inputs, Some(&board), &HostEnvironment::detect());
    prepare_board(&mut board, &settings);

    instrument::set_on(instrumented);
    instrument::reset();

    let stop = RouterStop::new();
    let mut sink = NoopProgressSink;
    run_pipeline(
        &mut board,
        &settings,
        &stop,
        RouterBudget::disabled(),
        &mut sink,
    )
    .expect("every batch stem has a routable signal layer");

    let snapshot = instrument::snapshot();
    instrument::set_on(false);

    let mut ses = Vec::new();
    fr_dsn::ses_writer::write(&board, &transform, &mut ses, &design_name)
        .expect("the SES writer cannot fail on a Vec");
    Run {
        ses: String::from_utf8(ses).expect("the SES writer emits UTF-8"),
        snapshot,
        board,
    }
}

fn read_board(dsn: &std::path::Path, design_name: &str) -> (fr_board::Board, CoordinateTransform) {
    let file = std::fs::File::open(dsn).unwrap_or_else(|e| panic!("cannot open {dsn:?}: {e}"));
    match fr_dsn::read_board(file, None, Some(design_name), &DsnReadOptions::default()) {
        BoardReadResult::Success {
            board,
            coordinate_transform,
            ..
        }
        | BoardReadResult::OutlineMissing {
            board,
            coordinate_transform,
            ..
        } => (
            *board.expect("the batch stems all produce a board"),
            coordinate_transform.expect("the batch stems all produce a coordinate transform"),
        ),
        other => panic!("{design_name} did not read: {other:?}"),
    }
}

fn stems(ci_only: bool) -> impl Iterator<Item = &'static Stem> {
    STEMS.iter().filter(move |s| !ci_only || s.ci)
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "slow in debug; run with FR_SLOW_PARITY=1 --release for all eight stems"
)]
fn the_three_guards_are_counted_on_every_router_stem() {
    if !parity::require_java_dir() {
        return;
    }
    let ci_only = std::env::var_os("FR_SLOW_PARITY").is_none();
    let mut report = String::new();
    let mut moved: Vec<String> = Vec::new();
    for stem in stems(ci_only) {
        let run = route_stem(stem, true);
        report.push_str(&instrument::render(stem.name, &run.snapshot));
        let expected = expected_for(stem.name);
        let fires: [u64; 5] = [
            run.snapshot.fires(Guard::G1aTreeEntryOutOfRange),
            run.snapshot.fires(Guard::G1bNullConnectionShape),
            run.snapshot.fires(Guard::G2RoomArrayResized),
            run.snapshot.fires(Guard::G2RoomIndexOutOfRange),
            run.snapshot.fires(Guard::G3TraceCornerOutOfRange),
        ];
        let visits: [u64; 5] = [
            run.snapshot.visits(Guard::G1aTreeEntryOutOfRange),
            run.snapshot.visits(Guard::G1bNullConnectionShape),
            run.snapshot.visits(Guard::G2RoomArrayResized),
            run.snapshot.visits(Guard::G2RoomIndexOutOfRange),
            run.snapshot.visits(Guard::G3TraceCornerOutOfRange),
        ];
        report.push_str(&format!(
            "    MEASURED row: stem {:?} fires {fires:?} visits {visits:?}\n",
            stem.name
        ));
        if fires != expected.fires {
            moved.push(format!(
                "{}: FIRE counts moved {:?} -> {fires:?}",
                stem.name, expected.fires
            ));
        }
        if visits != expected.visits {
            moved.push(format!(
                "{}: VISIT counts moved {:?} -> {visits:?}",
                stem.name, expected.visits
            ));
        }
    }
    println!("{report}");
    assert!(
        moved.is_empty(),
        "the #193 guard counts moved on {} row(s). A **fire** count moving is #193's finding \
         changing; a **visit** count moving is a routing change, not an instrumentation one. \
         Either way: update MEASURED and say which fix did it. The order is [G1a, G1b, \
         G2-resized, G2-out-of-range, G3].\n{}",
        moved.len(),
        moved.join("\n")
    );
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "slow in debug; run with FR_SLOW_PARITY=1 --release for all eight stems"
)]
fn instrumentation_changes_no_board_byte() {
    if !parity::require_java_dir() {
        return;
    }
    let ci_only = std::env::var_os("FR_SLOW_PARITY").is_none();
    for stem in stems(ci_only) {
        let off = route_stem(stem, false);
        let on = route_stem(stem, true);
        assert_eq!(
            off.snapshot.total_fires(),
            0,
            "{}: the counters must be inert while the instrumentation is off",
            stem.name
        );
        assert_eq!(
            off.ses.len(),
            on.ses.len(),
            "{}: the SES length moved with the instrumentation on",
            stem.name
        );
        assert!(
            off.ses == on.ses,
            "{}: the SES is not byte-identical with the instrumentation on",
            stem.name
        );
    }
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "needs a routed board; run with FR_SLOW_PARITY=1 --release"
)]
fn a_shortened_trace_makes_a_recorded_index_stale_and_clears_the_array_that_held_it() {
    if !parity::require_java_dir() {
        return;
    }
    let stem = STEMS
        .iter()
        .find(|s| s.name == "router-rpi-splitter")
        .expect("the smallest board that actually routes");
    let mut board = route_stem(stem, false).board;
    let tree = board.default_tree_id();

    let (trace_id, corners) = board
        .items
        .iter()
        .find_map(|(id, item)| match item {
            fr_board::Item::Trace(trace) if trace.polyline().corner_count() >= 4 => {
                let corners: Vec<_> = (0..trace.polyline().corner_count())
                    .filter_map(|i| trace.polyline().corner(i))
                    .collect();
                Some((*id, corners))
            }
            _ => None,
        })
        .expect("a routed board has a trace with four corners");

    let before = board.item_tree_shape_count(trace_id, tree);
    assert!(before > 0, "the trace has tree shapes to begin with");
    let recorded_index = before - 1;

    let seeded = fr_router::autoroute::item_info::get_expansion_room(
        &mut board,
        trace_id,
        recorded_index,
        tree,
        |_board, _item, _index, _tree| fr_board::ObstacleRoomId(0),
    );
    assert_eq!(
        seeded,
        Some(fr_board::ObstacleRoomId(0)),
        "the index was valid when it was recorded — that is what makes it stale later"
    );
    assert_eq!(
        board
            .get_item(trace_id)
            .and_then(fr_board::Item::get_autoroute_info_pur)
            .map(|info| info.expansion_rooms.len()),
        Some(before),
        "the array is as long as the item's tree-shape count"
    );

    let shorter = fr_geometry::Polyline::from_points(&corners[..corners.len() - 2]);
    assert!(
        board.replace_trace_geometry(trace_id, shorter),
        "replaceGeometry accepts a shorter polyline"
    );

    let after = board.item_tree_shape_count(trace_id, tree);
    assert!(
        after < before,
        "the directed case must actually shorten the shape array: {before} -> {after}"
    );
    assert!(
        recorded_index >= after,
        "and the index recorded before the change is now out of range: {recorded_index} >= {after}"
    );

    assert!(
        board
            .get_item(trace_id)
            .and_then(fr_board::Item::get_autoroute_info_pur)
            .is_none_or(|info| info.expansion_rooms.is_empty()),
        "replaceGeometry's clearDerivedData drops the autoroute scratch, so #193's G2 cannot \
         observe a live prefix and the stale index has nothing left to index"
    );
}

#[test]
fn the_stem_table_matches_the_fixture_file() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("the workspace root");
    let table = std::fs::read_to_string(root.join("tests/reference/router-fixtures.txt"))
        .expect("router-fixtures.txt");
    for stem in STEMS {
        let row = table
            .lines()
            .find(|line| line.starts_with(&format!("{}|", stem.name)))
            .unwrap_or_else(|| panic!("{} has no row in router-fixtures.txt", stem.name));
        let fields: Vec<&str> = row.split('|').collect();
        assert_eq!(fields[1], stem.dsn, "{}: dsn", stem.name);
        assert_eq!(
            fields[4],
            stem.max_passes.to_string(),
            "{}: max_passes",
            stem.name
        );
        assert_eq!(fields[5] == "on", stem.fanout, "{}: fanout", stem.name);
        assert_eq!(
            fields[6] == "on",
            stem.optimizer,
            "{}: optimizer",
            stem.name
        );
    }
}
