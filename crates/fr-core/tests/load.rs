//! Plan 8 Task 3: the load sequence's own pins — the `BoardLoader` guard, the post-load pass and
//! the coordinate transform the loaded board carries.
//!
//! The board-state parity is `tests/overrides.rs`' job (the `P8T3Probe` transcript replay); this
//! file pins the parts of the sequence that are not board state.

use fr_board::Board;
use fr_core::{
    Error, FileFormat, RoutingJob, SessionId, apply_immediate_post_load_processing,
    apply_router_settings_for_loaded_board, calculate_crc32_for_board, load_board_if_needed,
    load_from_kicad_json, load_from_specctra_dsn, save_as_specctra_session_ses,
};
use fr_dsn::{BoardReadResult, DsnReadOptions};
use fr_settings::sources::DefaultSettings;
use fr_settings::{HostEnvironment, RouterSettings, SettingsInputs, SettingsSource};

fn default_settings() -> RouterSettings {
    DefaultSettings::new(&HostEnvironment::detect())
        .get_settings()
        .expect("DefaultSettings always answers a table")
        .clone()
}

fn job_for(relative: &str) -> RoutingJob {
    let path = parity::java_dir().join(relative);
    let mut job = RoutingJob::new(SessionId::default());
    job.set_input(&path)
        .unwrap_or_else(|e| panic!("cannot set {} as input: {e}", path.display()));
    job
}

/// Every item, in `board.itemList` order, with its net numbers and clearance class — enough to
/// tell two boards apart without printing a whole clearance matrix.
fn digest(board: &Board) -> String {
    let mut out = String::new();
    for item in board.get_items() {
        out.push_str(&format!(
            "{}:{:?}:{};",
            item.id().0,
            item.net_nos(),
            item.clearance_class()
        ));
    }
    out
}

/// Quirk label S: `-de prev.ses -drc r.json` fails **in the loader**, not at the argument.
///
/// `BoardLoader.loadBoardIfNeeded:31-37` is the only place the format is checked, so the CLI
/// accepts the path, [`RoutingJob::set_input`] sniffs the bytes as [`FileFormat::Ses`], and the
/// run stops here with Java's own message.
#[test]
fn only_dsn_and_json_are_accepted() {
    let mut job = job_for("fixtures/Issue593-BBD_Mars-64.ses");
    assert_eq!(
        job.get_input().expect("an input").format,
        FileFormat::Ses,
        "the sniffed format is what the guard reads"
    );
    let error = load_board_if_needed(&mut job).expect_err("a SES input is refused");
    assert!(matches!(error, Error::Load(_)));
    assert_eq!(
        error.to_string(),
        // BoardLoader.java:33-34, with Java's `FileFormat.toString()` — the enum constant name.
        "Cannot load board: only DSN and JSON formats are supported, got SES",
        "the message is Java's, verbatim"
    );

    // A `.rules` input is refused by the same guard, with its own format name.
    let mut job = job_for("fixtures/Issue593-BBD_Mars-64.rules");
    let error = load_board_if_needed(&mut job).expect_err("a RULES input is refused");
    assert_eq!(
        error.to_string(),
        "Cannot load board: only DSN and JSON formats are supported, got RULES"
    );

    // And a job with no input at all takes the first exit (:26-29).
    let mut job = RoutingJob::new(SessionId::default());
    let error = load_board_if_needed(&mut job).expect_err("an inputless job is refused");
    assert_eq!(error.to_string(), "Cannot load board: job has no input");

    // A DSN passes the guard and loads.
    let mut job = job_for("examples/tutorial_board/tutorial_board.dsn");
    let loaded = load_board_if_needed(&mut job).expect("a DSN input loads");
    assert!(loaded.board.get_items().count() > 0);
    assert!(
        loaded.metadata.is_none(),
        "`DsnReader.readBoard` answers a Success with a null metadata (DsnReader.java:146)"
    );
}

/// A KiCad-design-JSON input passes `BoardLoader`'s format guard and **loads a real board** —
/// the discharge of Task 3's stub obligation, landed by Plan 8 Task 9.
///
/// This test read `the_kicad_json_reader_is_a_stub` until Task 9. It asserted the inert
/// `ParseError("(kicad_json", "the KiCad JSON reader is not ported yet (Plan 8 Task 9)")` that
/// [`fr_core::load::kicad_read_board`] used to answer; that body is now
/// `fr_dsn::kicad::read_board(text, None)`, so the assertion is inverted rather than deleted —
/// the obligation is discharged where it was pinned.
///
/// The counts come from the JVM: `crates/fr-dsn/tests/data/p8t8-kicad-read-b.txt`'s `ecc83-v1`
/// case reports `components count=15`, `packages count=9`, `padstacks count=6` and
/// `items count=34` for this exact fixture, and the whole item graph is compared row by row by
/// `crates/fr-dsn/tests/kicad_reader.rs::the_whole_section_9_to_11_item_graph_matches_the_jvm`.
/// What this test adds is that the **loader** reaches that reader.
#[test]
fn a_kicad_json_input_loads_a_real_board() {
    let mut job = job_for("fixtures/Issue649-kicad_ecc83-pp_input_board_v1.json");
    assert_eq!(
        job.get_input().expect("an input").format,
        FileFormat::KicadDesignJson,
        "the guard lets this format through"
    );
    let loaded = load_board_if_needed(&mut job).expect("a KiCad JSON input loads");
    assert_eq!(loaded.board.components.count(), 15);
    assert_eq!(loaded.board.library.packages.count(), 9);
    assert_eq!(loaded.board.library.padstacks.count(), 6);
    assert_eq!(loaded.board.get_items().count(), 34);
    assert!(
        loaded.metadata.is_some(),
        "`KiCadJsonReader.readBoard:729-736` builds a BoardMetadata, unlike DsnReader"
    );
    assert_eq!(
        loaded.transform.scale_factor(),
        10000.0,
        "`readBoard:322` builds the CoordinateTransform the SES writer needs; this fixture's \
         `\"resolution\": 1.0` in MM takes `:98-100`'s 0.1-micrometre default"
    );

    // Straight through the loader, without the `BoardLoader` wrapper. `{}` is a **Success** on
    // both sides — Gson and `serde_json` both give every field its initializer, so section 3's
    // empty-layer default builds `F.Cu`/`B.Cu` and section 5's missing outline generates the
    // padded box. Measured: stem `empty-object` in both committed transcripts.
    let mut settings = default_settings();
    let mut job = RoutingJob::new(SessionId::default());
    let loaded = load_from_kicad_json("{}", &mut job, &mut settings)
        .expect("an empty JSON object is a Success, as it is in the jar");
    assert_eq!(loaded.board.get_items().count(), 1, "the generated outline");
    assert_eq!(loaded.warnings.len(), 1, "the missing-outline warning");
}

/// `applyImmediatePostLoadProcessing:751-757` calls `board.reduceNetsOfRouteItems()`, and this
/// task is the **first caller** `fr_board::Board::reduce_nets_of_route_items` has ever had
/// (ported in Plan 2 at `crates/fr-board/src/board/query.rs:1096`, unreached by Plans 2-7).
///
/// # What this pins, and the count it deliberately does not
///
/// The task brief named this test `reduce_nets_of_route_items_is_called_exactly_once`. **It was
/// renamed in Task 3's review round 1, because no assertion here can establish a call count.**
/// The pass is a **measured no-op on every corpus board**: `RoutingBoard.java:1296`'s
/// `netNumbers.length <= 1` guard skips every item, because nothing `fr_dsn::read_board` produces
/// carries more than one net number. So the board digest is identical after zero, one or two
/// calls, and a name promising "exactly once" would be claiming evidence that does not exist.
/// Building a multi-net route item to make the count observable would mean widening `fr-board`'s
/// public surface for a test — `Item::assign_net_no` overwrites element 0 rather than appending
/// (quirk-faithfully, `Item.java:975-978`), so there is no existing API for it.
///
/// What *is* pinned:
///
/// 1. the loader runs **exactly** `read → the settings pass → the post-load pass`, by rebuilding
///    that sequence by hand and comparing the boards;
/// 2. `apply_immediate_post_load_processing` answers Java's always-`false`
///    (`RoutingBoard.java:1285,1355` computes `result` and never assigns it);
/// 3. the no-op premise itself, asserted rather than assumed — the day a reader hands back a
///    multi-net route item, assertion (c) below fails and this pin has to be rewritten around a
///    real observable instead of quietly passing.
#[test]
fn the_loader_runs_read_then_the_settings_pass_then_the_post_load_pass() {
    let path =
        parity::java_dir().join("fixtures/Issue575-drc_dev-board_4_hole_clearance_violations.dsn");
    let bytes = std::fs::read(&path).expect("the fixture is readable");

    let mut job = job_for("fixtures/Issue575-drc_dev-board_4_hole_clearance_violations.dsn");
    let mut settings = default_settings();
    let loaded =
        load_from_specctra_dsn(&bytes, &mut job, &mut settings).expect("the fixture loads");

    // The same sequence by hand.
    let mut manual = match fr_dsn::read_board(
        &bytes[..],
        None,
        Some("Issue575-drc_dev-board_4_hole_clearance_violations.dsn"),
        &DsnReadOptions::default(),
    ) {
        BoardReadResult::Success { board, .. } => *board.expect("a board"),
        other => panic!("the fixture did not read: {other:?}"),
    };
    let mut manual_settings = default_settings();
    apply_router_settings_for_loaded_board(&mut manual, &mut manual_settings);
    let before_reduce = digest(&manual);
    let reduced = apply_immediate_post_load_processing(&mut manual);
    assert!(
        !reduced,
        "`reduceNetsOfRouteItems` computes `result` and never assigns it \
         (RoutingBoard.java:1285,1355), so Java always answers false"
    );
    assert_eq!(
        digest(&loaded.board),
        digest(&manual),
        "the loader runs exactly `read` -> the settings pass -> the post-load pass"
    );

    // The corpus premise, asserted rather than assumed.
    assert_eq!(
        before_reduce,
        digest(&manual),
        "the pass is a no-op on this board — every item carries at most one net number"
    );
    assert!(
        manual.get_items().all(|item| item.net_count() <= 1),
        "RoutingBoard.java:1296's `netNumbers.length <= 1` guard skips every item on the corpus"
    );
    // And a second run cannot move it either, which is why the call *count* is not observable.
    assert!(!apply_immediate_post_load_processing(&mut manual));
    assert_eq!(before_reduce, digest(&manual));
}

/// Plan 3 ruling A's obligation — *"whatever holds a `Board` between a read and a write must also
/// hold the `CoordinateTransform`"* — is discharged by [`fr_core::LoadedBoard`].
///
/// The pin is that `LoadedBoard::transform` **is** the transform the read produced: writing the
/// SES through it is byte-identical to writing it through the transform taken straight out of
/// `fr_dsn::read_board`'s result.
#[test]
fn the_loaded_board_carries_its_coordinate_transform() {
    let relative = "examples/tutorial_board/tutorial_board.dsn";
    let path = parity::java_dir().join(relative);
    let bytes = std::fs::read(&path).expect("the fixture is readable");

    let mut job = job_for(relative);
    let mut settings = default_settings();
    let loaded = load_from_specctra_dsn(&bytes, &mut job, &mut settings).expect("it loads");

    let (mut reference_board, reference_transform) = match fr_dsn::read_board(
        &bytes[..],
        None,
        Some("tutorial_board.dsn"),
        &DsnReadOptions::default(),
    ) {
        BoardReadResult::Success {
            board,
            coordinate_transform,
            ..
        } => (
            *board.expect("a board"),
            coordinate_transform.expect("a transform"),
        ),
        other => panic!("the fixture did not read: {other:?}"),
    };
    let mut reference_settings = default_settings();
    apply_router_settings_for_loaded_board(&mut reference_board, &mut reference_settings);
    apply_immediate_post_load_processing(&mut reference_board);

    let mut through_loaded: Vec<u8> = Vec::new();
    save_as_specctra_session_ses(
        &loaded.board,
        &loaded.transform,
        "tutorial_board.dsn",
        &mut through_loaded,
    )
    .expect("the SES writes");
    let mut through_reader: Vec<u8> = Vec::new();
    save_as_specctra_session_ses(
        &reference_board,
        &reference_transform,
        "tutorial_board.dsn",
        &mut through_reader,
    )
    .expect("the SES writes");

    let header: String =
        String::from_utf8_lossy(&through_loaded[..64.min(through_loaded.len())]).into_owned();
    assert!(
        header.contains("tutorial_board.ses"),
        "SesWriter.java:61 replaces `.dsn` with `.ses` in the session name, got {header:?}"
    );
    assert_eq!(
        through_loaded, through_reader,
        "`LoadedBoard::transform` is the transform `Structure.createBoard` built"
    );

    // `calculateCrc32ForBoard` uses the same pair, and is a function of the board: the same board
    // and transform give the same checksum, a mutated board a different one.
    let crc = calculate_crc32_for_board(&loaded.board, &loaded.transform);
    assert_eq!(
        crc,
        calculate_crc32_for_board(&reference_board, &reference_transform)
    );
    let mut mutated = loaded.board.clone();
    assert!(mutated.apply_copper_to_edge_clearance_override(1234.0));
    assert_ne!(
        crc,
        calculate_crc32_for_board(&mutated, &loaded.transform),
        "the CRC32 is taken over the DSN serialisation, so a rules change moves it"
    );
}

/// `applyRouterSettingsForLoadedBoard`'s first two steps (`:741-744`, `:745`) are the same two
/// `fr_settings::resolve_headless` runs at `crates/fr-settings/src/resolve.rs:274-284`. They are
/// not shared code — the loader mutates the merge-#1 object in place, as Java does, while
/// `resolve_headless` runs the whole ladder — so this asserts they agree on both observables.
#[test]
fn the_settings_pass_is_the_same_two_steps_resolve_headless_runs() {
    let relative = "fixtures/Issue753-CPU-85_r104.dsn";
    let path = parity::java_dir().join(relative);
    let bytes = std::fs::read(&path).expect("the fixture is readable");
    let mut board = match fr_dsn::read_board(
        &bytes[..],
        None,
        Some("Issue753-CPU-85_r104.dsn"),
        &DsnReadOptions::default(),
    ) {
        BoardReadResult::Success { board, .. } => *board.expect("a board"),
        other => panic!("the fixture did not read: {other:?}"),
    };
    assert_eq!(board.get_layer_count(), 4, "a four-layer board");

    let mut through_loader = default_settings();
    assert_ne!(
        through_loader.get_layer_count(),
        board.get_layer_count(),
        "the defaults disagree with the board, so `:741-744` has work to do"
    );
    apply_router_settings_for_loaded_board(&mut board, &mut through_loader);
    assert_eq!(through_loader.get_layer_count(), board.get_layer_count());
    assert!(
        through_loader.are_board_specific_trace_costs_applied(),
        "`:745`'s `applyBoardSpecificOptimizations` ran"
    );

    let through_ladder = fr_settings::resolve_headless(
        &SettingsInputs {
            json_file: None,
            dsn: None,
            cli_rules: None,
            scheduler_rules: None,
            env: None,
            cli: None,
        },
        Some(&board),
        &HostEnvironment::detect(),
    );
    assert_eq!(
        through_ladder.get_layer_count(),
        through_loader.get_layer_count(),
        "the two reach the same layer count from the same board"
    );
    assert!(through_ladder.are_board_specific_trace_costs_applied());
}
