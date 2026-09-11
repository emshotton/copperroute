#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

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

#[derive(Clone, Default)]
struct CapturedLog(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for CapturedLog {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for CapturedLog {
    type Writer = CapturedLog;
    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

fn capture_logs<T>(f: impl FnOnce() -> T) -> (T, String) {
    let buf = CapturedLog::default();
    let subscriber = tracing_subscriber::fmt()
        .with_writer(buf.clone())
        .with_ansi(false)
        .with_target(false)
        .without_time()
        .with_max_level(tracing::Level::INFO)
        .finish();
    let result = tracing::subscriber::with_default(subscriber, f);
    let text = String::from_utf8_lossy(&buf.0.lock().unwrap()).into_owned();
    (result, text)
}

const KICAD_PCB_LAYERS: &str = r#"(layers (0 "F.Cu" signal) (31 "B.Cu" signal))"#;

fn kicad_pcb_text(net_class: &str) -> String {
    format!(
        r#"(kicad_pcb (version 20240108) {KICAD_PCB_LAYERS} (net 0 "") (net 1 "GND")
        {net_class}
        (gr_rect (start 0 0) (end 10 10) (layer "Edge.Cuts"))
        (footprint "R1" (layer "F.Cu") (at 1 1)
          (pad "1" smd rect (at 0 0) (size 1 1) (layers "F.Cu") (net 1 "GND"))))"#
    )
}

fn kicad_project_text(default_track_width_mm: f64) -> String {
    format!(
        r#"{{"board": {{"design_settings": {{"rules": {{}}}}}}, "net_settings": {{"classes": [
        {{"name": "Default", "clearance": 0.2, "track_width": {default_track_width_mm}, "via_diameter": 0.6, "via_drill": 0.3}}
        ]}}}}"#
    )
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

#[test]
fn a_project_net_class_failure_is_surfaced_as_a_warning_instead_of_silently_dropped() {
    let board = netclass_project_dir().join("board.kicad_pcb");
    let dir = scratch("broken-netclass-project");
    let project = dir.join("project.kicad_pro");
    std::fs::write(
        &project,
        r#"{"board": {"design_settings": {"rules": {}}}, "net_settings": {"classes": [{"name": "Default", "clearance": 0.2, "track_width": 0.2, "via_diameter": 0.6, "via_drill": 0.3}], "netclass_patterns": [{"pattern": "V[0-9]", "netclass": "Default"}]}}"#,
    )
    .expect("a broken project file");

    let mut request = LoadRequest::for_board(BoardSource::Path(board));
    request.kicad_project = Some(project);
    let loaded = load(&request).expect("the board still loads despite the project failure");

    assert!(
        loaded
            .warnings
            .iter()
            .any(|w| w.contains("Unsupported project net pattern: V[0-9]")),
        "the apply_net_classes error must reach Loaded.warnings, got {:?}",
        loaded.warnings
    );
    let default_class = loaded
        .board
        .rules
        .net_classes
        .get_by_name("default")
        .unwrap();
    assert_eq!(
        default_class.get_trace_half_width(0),
        1250,
        "net classes fall back to the router's own defaults when the project fails to apply"
    );
}

#[test]
fn an_adjacent_kicad_pro_is_discovered_and_used_when_the_flag_is_absent() {
    let dir = scratch("discovery-adjacent");
    let board = dir.join("board.kicad_pcb");
    std::fs::write(&board, kicad_pcb_text("")).unwrap();
    std::fs::write(dir.join("board.kicad_pro"), kicad_project_text(1.0)).unwrap();

    let request = LoadRequest::for_board(BoardSource::Path(board));
    let (loaded, log) = capture_logs(|| load(&request).unwrap());

    let default_class = loaded
        .board
        .rules
        .net_classes
        .get_by_name("default")
        .unwrap();
    assert_eq!(
        default_class.get_trace_half_width(0),
        5000,
        "the discovered project's 1.0 mm track width should have reached the router's rules"
    );
    assert!(
        log.contains("Found KiCad project file") && log.contains(&*"board.kicad_pro".to_string()),
        "expected the discovery message on the log, got:\n{log}"
    );
}

#[test]
fn an_explicit_kicad_project_outranks_an_adjacent_one() {
    let dir = scratch("discovery-explicit-precedence");
    let board = dir.join("board.kicad_pcb");
    std::fs::write(&board, kicad_pcb_text("")).unwrap();
    std::fs::write(dir.join("board.kicad_pro"), kicad_project_text(1.0)).unwrap();
    let explicit = dir.join("explicit.kicad_pro");
    std::fs::write(&explicit, kicad_project_text(2.0)).unwrap();

    let mut request = LoadRequest::for_board(BoardSource::Path(board));
    request.kicad_project = Some(explicit.clone());
    let (loaded, log) = capture_logs(|| load(&request).unwrap());

    let default_class = loaded
        .board
        .rules
        .net_classes
        .get_by_name("default")
        .unwrap();
    assert_eq!(
        default_class.get_trace_half_width(0),
        10000,
        "the explicit project's 2.0 mm track width must win over the adjacent 1.0 mm one"
    );
    assert!(
        log.contains("Using KiCad project file") && log.contains("explicit.kicad_pro"),
        "expected the explicit-project message on the log, got:\n{log}"
    );
    assert!(
        !log.contains("Found KiCad project file"),
        "the adjacent file must not be reported as used, got:\n{log}"
    );
}

#[test]
fn a_bad_explicit_project_path_does_not_fall_back_to_a_discovered_one() {
    let dir = scratch("discovery-bad-explicit");
    let board = dir.join("board.kicad_pcb");
    std::fs::write(&board, kicad_pcb_text("")).unwrap();
    std::fs::write(dir.join("board.kicad_pro"), kicad_project_text(1.0)).unwrap();

    let mut request = LoadRequest::for_board(BoardSource::Path(board));
    request.kicad_project = Some(dir.join("missing.kicad_pro"));
    let (loaded, log) = capture_logs(|| load(&request).unwrap());

    let default_class = loaded
        .board
        .rules
        .net_classes
        .get_by_name("default")
        .unwrap();
    assert_eq!(
        default_class.get_trace_half_width(0),
        1250,
        "a bad explicit path must not fall back to the adjacent project; built-in defaults apply"
    );
    assert!(
        log.contains("missing.kicad_pro") && log.contains("not read"),
        "expected a warning that the explicit project could not be read, got:\n{log}"
    );
    assert!(
        !log.contains("Found KiCad project file"),
        "the adjacent file must never be silently substituted, got:\n{log}"
    );
}

#[test]
fn no_project_file_at_all_warns_loudly_about_default_rules() {
    let dir = scratch("discovery-none");
    let board = dir.join("board.kicad_pcb");
    std::fs::write(&board, kicad_pcb_text("")).unwrap();

    let request = LoadRequest::for_board(BoardSource::Path(board));
    let loaded = load(&request).unwrap();

    let default_class = loaded
        .board
        .rules
        .net_classes
        .get_by_name("default")
        .unwrap();
    assert_eq!(
        default_class.get_trace_half_width(0),
        1250,
        "built-in defaults apply"
    );
    assert!(
        loaded
            .warnings
            .iter()
            .any(|w| w.contains("No KiCad project file found")
                && w.contains("built-in default design rules")),
        "expected the loud no-project warning in Loaded.warnings, got {:?}",
        loaded.warnings
    );
}

#[test]
fn a_board_with_embedded_net_classes_and_no_project_warns_of_nothing() {
    let dir = scratch("discovery-embedded-quiet");
    let board = dir.join("board.kicad_pcb");
    std::fs::write(
        &board,
        kicad_pcb_text(r#"(net_class "Default" (clearance 0.2) (trace_width 0.25) (via_dia 0.6) (via_drill 0.3))"#),
    )
    .unwrap();

    let request = LoadRequest::for_board(BoardSource::Path(board));
    let loaded = load(&request).unwrap();

    assert!(
        !loaded
            .warnings
            .iter()
            .any(|w| w.contains("No KiCad project file found")),
        "a board with its own embedded net classes needs no project file, got {:?}",
        loaded.warnings
    );
}
