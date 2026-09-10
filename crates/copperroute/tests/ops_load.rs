#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

use copperroute::ops::load::{BoardSource, LoadRequest, load};
use copperroute::ops::{OpError, SettingsOverrides};

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
    let dir = scratch("rules");
    let dsn = dir.join("board.dsn");
    std::fs::copy(testkit::fixture("Issue143-rpi_splitter.dsn"), &dsn).unwrap();
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
    let dir = scratch("precedence");
    let dsn = dir.join("board.dsn");
    std::fs::copy(testkit::fixture("Issue143-rpi_splitter.dsn"), &dsn).unwrap();
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

    let mut sparse = copper_settings::RouterSettings::new();
    copper_settings::set_field_value(&mut sparse, "scoring.via_costs", "5").unwrap();
    request.settings.sparse = Some(sparse);
    let loaded = load(&request).unwrap();
    assert_eq!(loaded.settings.scoring.as_ref().unwrap().via_costs, Some(5));
}

#[test]
fn a_session_is_imported_and_a_project_sets_constraints() {
    let dsn = testkit::fixture("Issue593-BBD_Mars-64.dsn");
    let ses = testkit::fixture("Issue593-BBD_Mars-64.ses");

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

fn netclass_project_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/netclass-project")
}

#[test]
fn a_kicad_project_s_net_classes_reach_the_router_s_rules() {
    let board = netclass_project_dir().join("board.kicad_pcb");

    let bare = load(&LoadRequest::for_board(BoardSource::Path(board.clone()))).unwrap();
    let bare_default = bare.board.rules.net_classes.get_by_name("default").unwrap();
    assert_eq!(
        bare_default.get_trace_half_width(0),
        1250,
        "0.25 mm default trace width"
    );

    let mut request = LoadRequest::for_board(BoardSource::Path(board));
    request.kicad_project = Some(netclass_project_dir().join("project.kicad_pro"));
    let with_project = load(&request).unwrap();

    let default_class = with_project
        .board
        .rules
        .net_classes
        .get_by_name("default")
        .unwrap();
    assert_eq!(
        default_class.get_trace_half_width(0),
        1000,
        "0.2 mm project default trace width"
    );

    let power_class = with_project
        .board
        .rules
        .net_classes
        .get_by_name("Power")
        .unwrap();
    assert_eq!(
        power_class.get_trace_half_width(0),
        2500,
        "0.5 mm project Power trace width"
    );

    let vcc = with_project.board.rules.nets.get_by_name("VCC");
    assert_eq!(vcc.len(), 1, "the VCC net should exist");
    assert_eq!(
        with_project
            .board
            .rules
            .net_classes
            .get(vcc[0].get_net_class())
            .get_name(),
        "Power",
        "VCC is assigned to Power by the project's V* wildcard pattern"
    );

    let sig = with_project.board.rules.nets.get_by_name("SIG");
    assert_eq!(
        with_project
            .board
            .rules
            .net_classes
            .get(sig[0].get_net_class())
            .get_name(),
        "default",
        "SIG keeps the Default class"
    );

    let padstack_id = power_class
        .get_via_rule()
        .expect("Power has a via rule")
        .get_via(0)
        .get_padstack();
    let drill_diameter = 2.0
        * with_project
            .board
            .library
            .padstacks
            .get(padstack_id)
            .unwrap()
            .drill_radius();
    assert!(
        (drill_diameter - 3500.0).abs() < 1.0,
        "Power's via_drill (0.2 mm) must be floored to min_through_hole_diameter (0.35 mm), got {drill_diameter}"
    );
}
