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
    let path = parity::reference_dir().join(relative);
    let mut job = RoutingJob::new(SessionId::default());
    job.set_input(&path)
        .unwrap_or_else(|e| panic!("cannot set {} as input: {e}", path.display()));
    job
}

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
        "Cannot load board: only DSN and JSON formats are supported, got SES",
        "the message is Java's, verbatim"
    );

    let mut job = job_for("fixtures/Issue593-BBD_Mars-64.rules");
    let error = load_board_if_needed(&mut job).expect_err("a RULES input is refused");
    assert_eq!(
        error.to_string(),
        "Cannot load board: only DSN and JSON formats are supported, got RULES"
    );

    let mut job = RoutingJob::new(SessionId::default());
    let error = load_board_if_needed(&mut job).expect_err("an inputless job is refused");
    assert_eq!(error.to_string(), "Cannot load board: job has no input");

    let mut job = job_for("examples/tutorial_board/tutorial_board.dsn");
    let loaded = load_board_if_needed(&mut job).expect("a DSN input loads");
    assert!(loaded.board.get_items().count() > 0);
    assert!(
        loaded.metadata.is_none(),
        "`DsnReader.readBoard` answers a Success with a null metadata (DsnReader.java:146)"
    );
}

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

    let mut settings = default_settings();
    let mut job = RoutingJob::new(SessionId::default());
    let loaded = load_from_kicad_json("{}", &mut job, &mut settings)
        .expect("an empty JSON object is a Success, as it is in the jar");
    assert_eq!(loaded.board.get_items().count(), 1, "the generated outline");
    assert_eq!(loaded.warnings.len(), 1, "the missing-outline warning");
}

#[test]
fn the_loader_runs_read_then_the_settings_pass_then_the_post_load_pass() {
    let path = parity::reference_dir()
        .join("fixtures/Issue575-drc_dev-board_4_hole_clearance_violations.dsn");
    let bytes = std::fs::read(&path).expect("the fixture is readable");

    let mut job = job_for("fixtures/Issue575-drc_dev-board_4_hole_clearance_violations.dsn");
    let mut settings = default_settings();
    let loaded =
        load_from_specctra_dsn(&bytes, &mut job, &mut settings).expect("the fixture loads");

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

    assert_eq!(
        before_reduce,
        digest(&manual),
        "the pass is a no-op on this board — every item carries at most one net number"
    );
    assert!(
        manual.get_items().all(|item| item.net_count() <= 1),
        "RoutingBoard.java:1296's `netNumbers.length <= 1` guard skips every item on the corpus"
    );
    assert!(!apply_immediate_post_load_processing(&mut manual));
    assert_eq!(before_reduce, digest(&manual));
}

#[test]
fn the_loaded_board_carries_its_coordinate_transform() {
    let relative = "examples/tutorial_board/tutorial_board.dsn";
    let path = parity::reference_dir().join(relative);
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

#[test]
fn the_settings_pass_is_the_same_two_steps_resolve_headless_runs() {
    let relative = "fixtures/Issue753-CPU-85_r104.dsn";
    let path = parity::reference_dir().join(relative);
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
