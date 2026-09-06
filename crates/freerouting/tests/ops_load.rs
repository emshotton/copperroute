#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

use freerouting::ops::load::{BoardSource, LoadRequest, load};
use freerouting::ops::{OpError, SettingsOverrides};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("fr-ops-load").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir
}

fn spike_dsn() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../benchmark/tests/data/spike/spike.dsn")
}

#[test]
fn a_dsn_path_loads_and_the_design_block_reaches_the_settings() {
    let loaded = load(&LoadRequest::for_board(BoardSource::Path(spike_dsn()))).unwrap();
    assert!(loaded.board.get_layer_count() >= 2);
    assert_eq!(
        loaded.settings.get_layer_count(),
        loaded.board.get_layer_count()
    );
    assert!(loaded.job.get_input().is_some());
}

#[test]
fn dsn_text_loads_under_the_given_name() {
    let text = std::fs::read_to_string(spike_dsn()).unwrap();
    let request = LoadRequest::for_board(BoardSource::Text {
        text,
        name: "board".to_string(),
    });
    let loaded = load(&request).unwrap();
    assert_eq!(loaded.job.name, "board");
    assert_eq!(loaded.job.get_input().unwrap().get_filename(), "board.dsn");
}

#[test]
fn a_missing_path_is_an_input_error() {
    let error = load(&LoadRequest::for_board(BoardSource::Path(PathBuf::from(
        "/nonexistent/board.dsn",
    ))))
    .unwrap_err();
    assert!(matches!(error, OpError::Input(_)), "{error}");
}

#[test]
fn a_session_file_is_not_a_board() {
    let dir = scratch("session-input");
    let input = dir.join("board.dsn");
    std::fs::write(&input, b"(session previous)\n").unwrap();
    let error = load(&LoadRequest::for_board(BoardSource::Path(input))).unwrap_err();
    assert!(matches!(error, OpError::Input(_)), "{error}");
    assert!(error.to_string().contains("Specctra DSN"), "{error}");
}

#[test]
fn an_explicit_rules_file_reaches_the_settings_and_an_adjacent_one_only_when_asked() {
    if !parity::require_reference_dir() {
        return;
    }
    let dir = scratch("rules");
    let dsn = dir.join("board.dsn");
    std::fs::copy(parity::fixture("Issue143-rpi_splitter.dsn"), &dsn).unwrap();
    std::fs::write(
        dir.join("board.rules"),
        b"(rules PCB board\n  (autoroute_settings\n    (via_costs 99)\n  )\n)\n",
    )
    .unwrap();

    let mut request = LoadRequest::for_board(BoardSource::Path(dsn.clone()));
    let quiet = load(&request).unwrap();
    assert_eq!(quiet.settings.scoring.as_ref().unwrap().via_costs, Some(50));

    request.discover_adjacent_rules = true;
    let discovered = load(&request).unwrap();
    assert_eq!(
        discovered.settings.scoring.as_ref().unwrap().via_costs,
        Some(99)
    );

    request.discover_adjacent_rules = false;
    request.rules = Some(dir.join("board.rules"));
    let explicit = load(&request).unwrap();
    assert_eq!(
        explicit.settings.scoring.as_ref().unwrap().via_costs,
        Some(99)
    );
}

#[test]
fn set_outranks_the_flags_below_it_and_the_sparse_payload_outranks_the_rules_file() {
    if !parity::require_reference_dir() {
        return;
    }
    let dir = scratch("precedence");
    let dsn = dir.join("board.dsn");
    std::fs::copy(parity::fixture("Issue143-rpi_splitter.dsn"), &dsn).unwrap();
    let rules = dir.join("r.rules");
    std::fs::write(
        &rules,
        b"(rules PCB board\n  (autoroute_settings\n    (via_costs 99)\n  )\n)\n",
    )
    .unwrap();

    let mut request = LoadRequest::for_board(BoardSource::Path(dsn));
    request.rules = Some(rules);
    request.settings = SettingsOverrides {
        settings_file: None,
        set: vec!["router.scoring.via_costs=77".to_string()],
        sparse: None,
    };
    let loaded = load(&request).unwrap();
    assert_eq!(
        loaded.settings.scoring.as_ref().unwrap().via_costs,
        Some(99),
        "the rules file is re-applied after the flags; that is the resolver's order and it stays"
    );

    let mut sparse = fr_settings::RouterSettings::new();
    fr_settings::set_field_value(&mut sparse, "scoring.via_costs", "5").unwrap();
    request.settings.sparse = Some(sparse);
    let loaded = load(&request).unwrap();
    assert_eq!(loaded.settings.scoring.as_ref().unwrap().via_costs, Some(5));
}

#[test]
fn a_session_is_imported_and_a_project_sets_constraints() {
    if !parity::require_reference_dir() {
        return;
    }
    let dsn = parity::fixture("Issue593-BBD_Mars-64.dsn");
    let ses = parity::fixture("Issue593-BBD_Mars-64.ses");

    let bare = load(&LoadRequest::for_board(BoardSource::Path(dsn.clone()))).unwrap();
    let mut request = LoadRequest::for_board(BoardSource::Path(dsn));
    request.session = Some(ses);
    let with_session = load(&request).unwrap();
    assert!(
        with_session.board.get_traces().len() > bare.board.get_traces().len(),
        "the session's wires are on the board"
    );

    let project = spike_dsn().with_file_name("stripped.kicad_pro");
    let mut request = LoadRequest::for_board(BoardSource::Path(spike_dsn()));
    request.kicad_project = Some(project);
    let loaded = load(&request).unwrap();
    assert_eq!(
        loaded
            .board
            .rules
            .drc_constraints
            .as_ref()
            .and_then(|c| c.hole_to_hole),
        Some(2500)
    );
}
