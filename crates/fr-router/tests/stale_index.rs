//! Plan 9 Task 17: the #193 stale tree-index discovery.
//!
//! Quirk #193 is three HEAD-only guards that each silently `continue` when an item's tree-shape
//! indices have gone stale under a running maze search. The register row's improvement column
//! says *"find out why the indices go stale and fix **that**"*, and the plan makes that a
//! **discovery workstream, not a fix**: nothing here changes a routing decision, and the guards
//! stay exactly as they were transcribed.
//!
//! This file is the measurement. It drives the port's own whole-board pipeline over the eight
//! batch stems with [`fr_router::autoroute::instrument`] on, and asserts two things:
//!
//! * [`the_three_guards_are_counted_on_every_router_stem`] — the survey's claim that *the corpus
//!   reaches all three* is **verified rather than assumed**. It asserts what was measured, per
//!   guard, so a later change that starts or stops tripping a guard breaks the test rather than
//!   silently rewriting the report.
//! * [`instrumentation_changes_no_board_byte`] — the Plan 7 ruling 11 shape: with the counters on
//!   and off the SES is byte-identical. **This is the gate that makes the task safe to land**,
//!   and it is the reason the instrumentation is a plain module behind an environment variable
//!   rather than anything the guards read.
//!
//! # T17: these are characterisation pins, not fix markers
//!
//! Task 17 fixes nothing. [`MEASURED`] records the current, *stale*, behaviour. A Task 8 row that
//! changes a guard's fire count is expected to update the table and say which row moved it; that
//! is the flip, and the `// T17:` breadcrumbs at the guard sites point back here.
//!
//! # Why the counts are asserted exactly, and per stem
//!
//! Four of the eight stems are `FR_SLOW_PARITY` stems, so the CI lane and the full lane run
//! different subsets and a whole-corpus total would mean two different things. The assertion is
//! therefore per stem and an exact equality against [`MEASURED`], which is affordable because the
//! pipeline is deterministic — `batch_parity.rs` gets a byte-identical SES out of these same runs.
//! A moved count is a real change in what the router does, not measurement noise.

use std::collections::BTreeMap;

use fr_dsn::{BoardReadResult, CoordinateTransform, DsnReadOptions};
use fr_router::autoroute::instrument::{self, Guard, Snapshot};
use fr_router::pipeline::{
    NoopProgressSink, RouterBudget, RouterStop, prepare_board, run_pipeline,
};
use fr_settings::sources::{CliSettings, DsnFileSettings, EnvironmentVariablesSource};
use fr_settings::{HostEnvironment, SettingsInputs, SettingsSource, resolve_headless};

// =================================================================================================
// The stems — `tests/reference/router-fixtures.txt`'s batch rows, i.e. `batch_parity.rs`'s eight
// =================================================================================================

/// One whole-board stem, with the same four switches `batch_parity.rs` gives it.
struct Stem {
    name: &'static str,
    dsn: &'static str,
    max_passes: i32,
    fanout: bool,
    optimizer: bool,
    /// Whether the stem runs under a plain `cargo nextest run -p fr-router`.
    ci: bool,
}

/// The eight batch stems, in `router-fixtures.txt` order. Deliberately a **copy** of
/// `batch_parity.rs`'s table rather than a shared helper: that file is a parity gate and this one
/// is a measurement, and coupling them would make a measurement change look like a parity change.
/// [`the_stem_table_matches_the_fixture_file`] is what keeps the copy honest.
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

// =================================================================================================
// The measurement — filled in from the run, and asserted from then on
// =================================================================================================

/// What one stem's instrumented run produced: the five guard fire counts, in [`Guard::ALL`] order.
///
/// **T17: this is the characterisation, not a specification.** The numbers are what the port does
/// today with #193's guards in place. Task 8 may move them; when it does, the row moves with the
/// fix that moved it and the report's paragraph is quoted in the commit message.
struct Expected {
    stem: &'static str,
    /// `[G1a, G1b, G2-resized, G2-out-of-range, G3]`.
    fires: [u64; 5],
    /// The same five guards' **visit** counts — how many times each test was evaluated.
    ///
    /// Without these a row of zeroes in `fires` would be unreadable: "the guard was reached and
    /// never tripped" and "the guard is on a path this board does not walk" are opposite
    /// findings, and only the denominator separates them. The numbers are exact rather than a
    /// floor because the pipeline is deterministic — `batch_parity.rs` gets a byte-identical SES
    /// out of the same runs — so a moved visit count is a real change in what the router does.
    visits: [u64; 5],
}

/// The measured table, from the release run recorded in
/// `.superpowers/sdd/2026-09-03-plan-9-post-parity/task-17-report.md`.
///
/// **Every fire count is zero, across 1 101 064 guard evaluations on eight boards.** That is the
/// finding, and the plan asked for it in those words: *"if a guard never fires, that is the
/// finding and the report says so."*
const MEASURED: &[Expected] = &[
    Expected {
        stem: "router-rpi-splitter",
        fires: [0, 0, 0, 0, 0],
        visits: [76, 76, 1042, 1042, 111],
    },
    Expected {
        stem: "router-dac2020-bm01",
        fires: [0, 0, 0, 0, 0],
        visits: [13210, 13210, 336_796, 336_796, 39896],
    },
    Expected {
        stem: "router-j2-reference",
        fires: [0, 0, 0, 0, 0],
        visits: [2587, 2587, 17843, 17843, 1053],
    },
    Expected {
        // 438 empty `@:no_net_N` nets, so no item has an unconnected set and nothing routes —
        // `router-fixtures.txt`'s own note. The zero visits are that board, not a missing probe.
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
        visits: [14952, 14952, 73088, 73088, 5461],
    },
    Expected {
        stem: "router-strict-drc-cnh",
        fires: [0, 0, 0, 0, 0],
        visits: [3665, 3665, 60219, 60219, 7353],
    },
    Expected {
        // Nothing to route (plan-7 ruling 7's `NoRoutableLayer` board).
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

// =================================================================================================
// The run
// =================================================================================================

/// What one instrumented whole-board run produces.
struct Run {
    ses: String,
    snapshot: Snapshot,
    /// The routed board, so the directed case has real traces to make stale.
    board: fr_board::Board,
}

/// The jar's `-de <dsn> -do <ses> -mp <n>` flow, port side — `batch_parity.rs`'s `route_stem`
/// with the counters reset around it. Ruling AW's `resolve_headless` + `prepare_board` ladder is
/// reproduced exactly, because a run that loaded the board differently would route a different
/// board and the guard counts would be of something else.
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

/// `batch_parity.rs`'s reader, verbatim.
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

// =================================================================================================
// The two binding tests
// =================================================================================================

/// The survey's claim that "the corpus reaches all three" guards, checked rather than assumed.
///
/// The plan's own wording: *"if a guard never fires, that is the finding and the report says
/// so."* It does; see the report's characterisation section.
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
    for stem in stems(ci_only) {
        let run = route_stem(stem, true);
        report.push_str(&instrument::render(stem.name, &run.snapshot));
        let expected = expected_for(stem.name);
        let actual: [u64; 5] = [
            run.snapshot.fires(Guard::G1aTreeEntryOutOfRange),
            run.snapshot.fires(Guard::G1bNullConnectionShape),
            run.snapshot.fires(Guard::G2RoomArrayResized),
            run.snapshot.fires(Guard::G2RoomIndexOutOfRange),
            run.snapshot.fires(Guard::G3TraceCornerOutOfRange),
        ];
        assert_eq!(
            actual, expected.fires,
            "{}: the #193 guard fire counts moved. If a Task 8 fix moved them, update MEASURED \
             and say which row did it; the order is [G1a, G1b, G2-resized, G2-out-of-range, G3].",
            stem.name
        );
        let visits: [u64; 5] = [
            run.snapshot.visits(Guard::G1aTreeEntryOutOfRange),
            run.snapshot.visits(Guard::G1bNullConnectionShape),
            run.snapshot.visits(Guard::G2RoomArrayResized),
            run.snapshot.visits(Guard::G2RoomIndexOutOfRange),
            run.snapshot.visits(Guard::G3TraceCornerOutOfRange),
        ];
        assert_eq!(
            visits, expected.visits,
            "{}: the #193 guard **visit** counts moved — the router walks the guard sites a \
             different number of times, which is a routing change, not an instrumentation one.",
            stem.name
        );
    }
    // The measurement is the deliverable, so it is printed even on success (`--nocapture`).
    println!("{report}");
}

/// Plan 7 ruling 11's shape: the instrumentation is invisible to the board.
///
/// **The gate that makes Task 17 safe to land.** Every recorder is a read behind
/// [`instrument::on`], so this can only fail if a recorder is given a side effect.
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

// =================================================================================================
// The directed case — the staleness itself, and the mechanism that stops it reaching the guards
// =================================================================================================

/// T17: #193's staleness, reproduced on purpose, and the reason it never reaches the guards.
///
/// The eight-stem run says the guards never fire. That alone would be consistent with the
/// staleness being impossible *or* with the measurement missing it, so this drives the mechanism
/// by hand on a routed board:
///
/// 1. take a real trace and the tree-shape count a room or door would have recorded;
/// 2. seed the item's `expansionRoomArr` at the last valid index, which is what
///    `getExpansionRoom` does on every neighbour walk;
/// 3. shorten the trace through [`fr_board::Board::replace_trace_geometry`] — the port of
///    `PolylineTraceSearchTreeAdapter.replaceGeometry`, i.e. **the** path every pull-tight, shove
///    and split takes;
/// 4. read the count back.
///
/// The recorded index **is** now out of range — the staleness is real and the guards are not
/// defending against nothing. But the same call cleared the item's autoroute scratch on the way
/// through (`clearDerivedData` at `PolylineTraceSearchTreeAdapter.java:38`, ported at
/// `board/mod.rs`'s `replace_trace_geometry`), so the array a stale index would have indexed no
/// longer exists. That is the whole answer to the register row's *"find out why the indices go
/// stale"*: **on this code path they do, and Java's own `clearDerivedData` throws away the thing
/// that would have noticed.**
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

    // A trace with room to lose a corner. `router-rpi-splitter` routes 141 of them.
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

    // What every `Sorted*RoomNeighbours` walk does: allocate the item's room array and take the
    // slot for one shape index. `ObstacleRoomId(0)` stands in for the room Task 2's arena builds;
    // the directed case is about the array, not its contents.
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

    // The mutation: two corners fewer, through the trace-replacement path the router uses.
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

    // …and this is why no guard ever sees it: the array that held the index is gone.
    assert!(
        board
            .get_item(trace_id)
            .and_then(fr_board::Item::get_autoroute_info_pur)
            .is_none_or(|info| info.expansion_rooms.is_empty()),
        "replaceGeometry's clearDerivedData drops the autoroute scratch, so #193's G2 cannot \
         observe a live prefix and the stale index has nothing left to index"
    );
}

/// The copy of `batch_parity.rs`'s stem table is checked against the fixture file both read, so
/// the two cannot drift into describing different runs.
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
