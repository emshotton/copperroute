use std::collections::{BTreeMap, BTreeSet};

use copper_board::prelude::*;
use copper_board::{ItemId, TreeId};
use copper_dsn::{BoardReadResult, CoordinateTransform, DsnReadOptions};
use copper_geometry::{
    IntBox, IntVector, Point, PolygonShape, Polyline, PolylineShapeRef, Shape, TileShape,
};
use copper_router::autoroute::instrument::{self, Guard, Snapshot};
use copper_router::autoroute::maze::search::MazeSearchEngine;
use copper_router::pipeline::{
    NoopProgressSink, RouterBudget, RouterStop, prepare_board, run_pipeline,
};
use copper_settings::sources::{CliSettings, DsnFileSettings, EnvironmentVariablesSource};
use copper_settings::{HostEnvironment, SettingsInputs, SettingsSource, resolve_headless};

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
    /// Whether the stem runs under a plain `cargo nextest run -p copper-router`.
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

const MEASURED: &[Expected] = &[
    Expected {
        stem: "router-rpi-splitter",
        fires: [0, 0, 0, 0, 0],
        visits: [75, 75, 1059, 1059, 114],
    },
    Expected {
        stem: "router-dac2020-bm01",
        fires: [0, 0, 0, 0, 0],
        // T8: was [13210, 13210, 336_796, 336_796, 39896].
        // #163 (cc6c210) -> [13161, 13161, 343_469, 343_469, 41377];
        // #171+#170 (e860a26) -> [13631, 13631, 353_561, 353_561, 43337];
        // #156+#167+#158 (c39d844) -> [13630, 13630, 353_444, 353_444, 43337].
        // T9: #227 — the optimizer stage began doing work, so the router walks every guard site
        // tens of times more often. This is the same board the A/B measures at 48.4x cpu.
        visits: [144_444, 144_444, 13_355_651, 13_355_651, 2_339_316],
    },
    Expected {
        stem: "router-j2-reference",
        fires: [0, 0, 0, 0, 0],
        // T8: was [2587, 2587, 17843, 17843, 1053].
        // #171+#170 (e860a26) moved G1a/G1b 2587 -> 3177;
        // #156+#167+#158 (c39d844) moved G2 17843 -> 17854. #163 moved nothing here.
        // T9: #227, as on `dac2020`. The A/B measures this board at ~12x cpu.
        visits: [35630, 35630, 296_273, 296_273, 21548],
    },
    Expected {
        // 438 empty `@:no_net_N` nets, so no item has an unconnected set and nothing routes —
        // `router-fixtures.txt`'s own note. The zero visits are that board, not a missing probe.
        stem: "router-tutorial-board",
        fires: [0, 0, 0, 0, 0],
        // T8, T9: unchanged — this board routes nothing.
        visits: [0, 0, 0, 0, 0],
    },
    Expected {
        stem: "router-ecc83-input",
        fires: [0, 0, 0, 0, 0],
        // T8: unchanged by all ten commits. T9: unchanged, for the same near-perfect reason as
        // `router-rpi-splitter`.
        visits: [26, 26, 89, 89, 4],
    },
    Expected {
        stem: "router-fanout-bm11",
        fires: [0, 0, 0, 0, 0],
        // T8: was [14952, 14952, 73088, 73088, 5461].
        // #163 (cc6c210) -> [14952, 14952, 73080, 73080, 5455];
        // #171+#170 (e860a26) -> [17445, 17445, 80849, 80849, 6142];
        // #156+#167+#158 (c39d844) -> below.
        // T9: **unchanged, and that is the control.** This is the one stem that runs with
        // `optimizer=off`, so #227 cannot reach it — and none of T9's other six fixes moves a
        // guard count either.
        visits: [17445, 17445, 80853, 80853, 6142],
    },
    Expected {
        stem: "router-strict-drc-cnh",
        fires: [0, 0, 0, 0, 0],
        // T8: was [3665, 3665, 60219, 60219, 7353].
        // #163 (cc6c210) -> [3667, 3667, 60206, 60206, 7333];
        // #171+#170 (e860a26) -> [3669, 3669, 60206, 60206, 7333];
        // #156+#167+#158 (c39d844) -> [3669, 3669, 60180, 60180, 7356].
        // T9: #227, as on `dac2020`. The A/B measures this board at ~26x cpu, and its incompletes
        // fall 14 -> 2.
        visits: [90961, 90961, 939_226, 939_226, 61424],
    },
    Expected {
        stem: "router-empty-board",
        fires: [0, 0, 0, 0, 0],
        // T8, T9: unchanged — this board has no routable signal layer.
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
    board: copper_board::Board,
}

/// The jar's `-de <dsn> -do <ses> -mp <n>` flow, port side — `batch_parity.rs`'s `route_stem`
/// with the counters reset around it. Ruling AW's `resolve_headless` + `prepare_board` ladder is
/// reproduced exactly, because a run that loaded the board differently would route a different
/// board and the guard counts would be of something else.
fn route_stem(stem: &Stem, instrumented: bool) -> Run {
    let dsn = testkit::corpus_dir().join(stem.dsn);
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
    copper_dsn::ses_writer::write(&board, &transform, &mut ses, &design_name)
        .expect("the SES writer cannot fail on a Vec");
    Run {
        ses: String::from_utf8(ses).expect("the SES writer emits UTF-8"),
        snapshot,
        board,
    }
}

/// `batch_parity.rs`'s reader, verbatim.
fn read_board(
    dsn: &std::path::Path,
    design_name: &str,
) -> (copper_board::Board, CoordinateTransform) {
    let file = std::fs::File::open(dsn).unwrap_or_else(|e| panic!("cannot open {dsn:?}: {e}"));
    match copper_dsn::read_board(file, None, Some(design_name), &DsnReadOptions::default()) {
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
    ignore = "slow in debug; run with COPPERROUTE_SLOW=1 --release for all eight stems"
)]
fn the_three_guards_are_counted_on_every_router_stem() {
    let ci_only = std::env::var_os("COPPERROUTE_SLOW").is_none();
    let mut report = String::new();
    // **Collected, not asserted per stem.** A `assert_eq!` inside the loop stops at the first
    // moved row, and on this suite a stem costs minutes — so a task that moves four rows would
    // need four full sweeps to learn what to write into `MEASURED`. The rows are gathered, the
    // whole table is printed, and the assertion is taken once at the end over every stem.
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
        // The row in `MEASURED`'s own syntax, so a task that has to update the table can paste it.
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
    // The measurement is the deliverable, so it is printed even on success (`--nocapture`).
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
    ignore = "slow in debug; run with COPPERROUTE_SLOW=1 --release for all eight stems"
)]
fn instrumentation_changes_no_board_byte() {
    let ci_only = std::env::var_os("COPPERROUTE_SLOW").is_none();
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

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "needs a routed board; run with COPPERROUTE_SLOW=1 --release"
)]
fn a_shortened_trace_makes_a_recorded_index_stale_and_clears_the_array_that_held_it() {
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
            copper_board::Item::Trace(trace) if trace.polyline().corner_count() >= 4 => {
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

    let seeded = copper_router::autoroute::item_info::get_expansion_room(
        &mut board,
        trace_id,
        recorded_index,
        tree,
        |_board, _item, _index, _tree| copper_board::ObstacleRoomId(0),
    );
    assert_eq!(
        seeded,
        Some(copper_board::ObstacleRoomId(0)),
        "the index was valid when it was recorded — that is what makes it stale later"
    );
    assert_eq!(
        board
            .get_item(trace_id)
            .and_then(copper_board::Item::get_autoroute_info_pur)
            .map(|info| info.expansion_rooms.len()),
        Some(before),
        "the array is as long as the item's tree-shape count"
    );

    // The mutation: two corners fewer, through the trace-replacement path the router uses.
    let shorter = copper_geometry::Polyline::from_points(&corners[..corners.len() - 2]);
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
            .and_then(copper_board::Item::get_autoroute_info_pur)
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

fn tie_pin_board() -> (Board, ItemId, ItemId, ItemId) {
    let ls = LayerStructure::new(vec![
        Layer::new("front".to_string(), true),
        Layer::new("back".to_string(), true),
    ]);
    let cm = ClearanceMatrix::get_default_instance(&ls, 200);
    let mut rules = BoardRules::new(ls.clone(), cm);
    rules.create_default_net_class();
    let default_class = rules.get_default_net_class();

    // A through pad, so the pin has a shape on both layers and a centre to contact.
    let mut padstacks = Padstacks::new(ls);
    let pad_shape = Shape::Tile(TileShape::Box(IntBox::from_coords(-200, -200, 200, 200)));
    let pad = padstacks.add(
        "tie",
        vec![Some(pad_shape.clone()), Some(pad_shape)],
        true,
        false,
    );
    let mut packages = Packages::new();
    let pkg = packages.add(
        "tiepkg",
        vec![PackagePin::new("P1", pad, IntVector::new(0, 0).into(), 0.0)],
        None,
        None,
        None,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        true,
    );
    let mut components = Components::new();
    components.add_with_generated_name(Some(Point::new(0, 0)), 0.0, true, pkg);

    let outline = vec![PolylineShapeRef::Polygon(PolygonShape::from_points(&[
        Point::new(-10_000, -10_000),
        Point::new(10_000, -10_000),
        Point::new(10_000, 10_000),
        Point::new(-10_000, 10_000),
    ]))];
    let mut board = Board::new(
        outline,
        0,
        IntBox::from_coords(-20_000, -20_000, 20_000, 20_000),
        rules,
        BoardLibrary::new(padstacks, packages),
        components,
        Communication::default(),
    );
    board.rules.nets.add("GND", 1, false, default_class);
    board.rules.nets.add("GNDA", 1, false, default_class);

    // **The tie pin**: one pin, two nets. This is the input `:157`'s `netCount() > 1` asks for,
    // and the thing no corpus board turned out to have in this configuration.
    let tie_pin = board.insert_pin(1, 0, vec![1, 2], 1, FixedState::Unfixed);
    assert_eq!(
        board
            .get_item(tie_pin)
            .expect("the pin inserts")
            .net_count(),
        2,
        "the fixture must actually be a tie pin, or `:157` is not reached at all"
    );

    let foreign = board
        .insert_trace_without_cleaning(
            Polyline::from_points(&[Point::new(0, 0), Point::new(5000, 0)]),
            0,
            100,
            vec![2],
            1,
            FixedState::Unfixed,
        )
        .expect("the foreign-net trace inserts");
    let own = board
        .insert_trace_without_cleaning(
            Polyline::from_points(&[Point::new(0, 0), Point::new(-5000, 0)]),
            0,
            100,
            vec![1],
            1,
            FixedState::Unfixed,
        )
        .expect("the own-net trace inserts");

    let contacts = board.normal_contacts(tie_pin);
    assert!(
        contacts.contains(&foreign) && contacts.contains(&own),
        "both traces must be normal contacts of the tie pin, or `:158`'s loop never sees them: \
         {contacts:?}"
    );
    (board, tie_pin, foreign, own)
}

/// The tree shapes of one item in the default tree, as a comparable snapshot.
fn tree_shapes(board: &mut Board, id: ItemId, tree: TreeId) -> Vec<Option<TileShape>> {
    (0..board.item_tree_shape_count(id, tree))
        .map(|i| board.item_tree_shape(id, tree, i))
        .collect()
}

#[test]
fn the_tie_pin_reduction_fires_on_a_genuine_tie_pin() {
    let (mut board, tie_pin, foreign, own) = tie_pin_board();
    let tree = board.trees.get_default_tree().id();

    // The shape arrays before, so "it fired" is a board fact and not only a counter reading.
    let foreign_before = tree_shapes(&mut board, foreign, tree);
    let own_before = tree_shapes(&mut board, own, tree);

    let item_list: BTreeSet<ItemId> = [tie_pin].into_iter().collect();
    MazeSearchEngine::reduce_trace_shapes_at_tie_pins(&mut board, &item_list, 1, tree);

    assert_ne!(
        tree_shapes(&mut board, foreign, tree),
        foreign_before,
        "the foreign-net trace's tree shape must actually be reduced, so the pin centre stops \
         being blocked — the whole point of the method"
    );
    assert_eq!(
        tree_shapes(&mut board, own, tree),
        own_before,
        "and the own-net trace must be left alone: `:160`'s `containsNet(ownNetNo)` excuses it"
    );
}

/// The other half of #297's answer: two negative controls, so
/// [`the_tie_pin_reduction_fires_on_a_genuine_tie_pin`] cannot be passing for a reason unrelated
/// to the predicate.
///
/// * Searching for net **2** on the same board makes the net-1 trace the foreign one, so the
///   *other* trace is reduced — the predicate keys on the net argument, not on which trace the
///   contact set happens to list first.
/// * A pin on a **single** net fails `:157`'s `netCount() > 1` and nothing fires at all. That is
///   the configuration every pin on all eight corpus stems is in, and it is why the census reads
///   zero.
#[test]
fn the_tie_pin_reduction_keys_on_the_net_and_on_the_pin_being_a_tie() {
    // Control 1: the same board, the other net.
    let (mut board, tie_pin, foreign, own) = tie_pin_board();
    let tree = board.trees.get_default_tree().id();
    let own_before = tree_shapes(&mut board, own, tree);
    let foreign_before = tree_shapes(&mut board, foreign, tree);
    let item_list: BTreeSet<ItemId> = [tie_pin].into_iter().collect();
    MazeSearchEngine::reduce_trace_shapes_at_tie_pins(&mut board, &item_list, 2, tree);
    assert_ne!(
        tree_shapes(&mut board, own, tree),
        own_before,
        "net 1's trace is the foreign one now"
    );
    assert_eq!(
        tree_shapes(&mut board, foreign, tree),
        foreign_before,
        "and net 2's is excused"
    );

    // Control 2: a pin on one net. `:157` refuses, so neither trace moves.
    let (mut board, _tie_pin, foreign, own) = tie_pin_board();
    let tree = board.trees.get_default_tree().id();
    let single_net_pin = board.insert_pin(1, 0, vec![1], 1, FixedState::Unfixed);
    assert_eq!(
        board
            .get_item(single_net_pin)
            .expect("inserted")
            .net_count(),
        1
    );
    let foreign_before = tree_shapes(&mut board, foreign, tree);
    let own_before = tree_shapes(&mut board, own, tree);
    let item_list: BTreeSet<ItemId> = [single_net_pin].into_iter().collect();
    MazeSearchEngine::reduce_trace_shapes_at_tie_pins(&mut board, &item_list, 1, tree);
    assert_eq!(
        tree_shapes(&mut board, foreign, tree),
        foreign_before,
        "a pin on one net is not a tie pin — `:157` refuses"
    );
    assert_eq!(tree_shapes(&mut board, own, tree), own_before);
}
