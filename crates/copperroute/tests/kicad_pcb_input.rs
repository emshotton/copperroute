#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

const PORT: &str = env!("CARGO_BIN_EXE_copperroute");

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("fr-kicad-pcb-input").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir
}

fn run(argv: &[&str]) -> (String, String, i32) {
    let (stdout, stderr, code) = testkit::run_port_binary(Path::new(PORT), argv);
    (
        String::from_utf8_lossy(&stdout).into_owned(),
        String::from_utf8_lossy(&stderr).into_owned(),
        code,
    )
}

#[test]
fn it_reports_a_board_summary_for_a_kicad_board() {
    let board = testkit::workspace_root().join("web/example.kicad_pcb");
    let (stdout, stderr, code) = run(&["info", &board.to_string_lossy()]);
    assert_eq!(code, 0, "info failed: {stderr}");

    let summary: serde_json::Value =
        serde_json::from_str(&stdout).unwrap_or_else(|e| panic!("info emits JSON: {e}: {stdout}"));

    let layers = summary
        .get("layers")
        .unwrap_or_else(|| panic!("no layers in summary: {summary}"));
    assert_eq!(
        layers.as_array().map(Vec::len),
        Some(2),
        "expected the two copper layers from web/example.kicad_pcb, got {layers}"
    );

    let nets = summary
        .get("nets")
        .unwrap_or_else(|| panic!("no nets in summary: {summary}"));
    let net_names: Vec<&str> = nets
        .as_array()
        .unwrap_or_else(|| panic!("nets is not an array: {nets}"))
        .iter()
        .filter_map(|net| net.get("name").and_then(|n| n.as_str()))
        .collect();
    assert!(
        net_names.contains(&"Signal") && net_names.contains(&"Return"),
        "expected Signal and Return nets from web/example.kicad_pcb, got {net_names:?}"
    );
}

#[test]
fn it_prints_the_loaders_warnings_when_routing_a_kicad_board() {
    let board = testkit::workspace_root().join("web/examples/easyduino/nano.kicad_pcb");
    let dir = scratch("prints-loader-warnings");
    let output = dir.join("out.ses");

    let (_, stderr, code) = run(&[
        "route",
        &board.to_string_lossy(),
        "-o",
        &output.to_string_lossy(),
        "--max-passes",
        "1",
        "--timeout",
        "120",
    ]);
    assert_eq!(code, 0, "route failed: {stderr}");
    assert!(
        stderr.contains("copper zones will be preserved with their fill cache removed"),
        "expected read_pcb's zone warning on stderr, got:\n{stderr}"
    );
    assert!(
        stderr.contains("Curved board edges are approximated"),
        "expected read_pcb's outline warning on stderr, got:\n{stderr}"
    );
}

#[test]
fn it_routes_a_kicad_board_to_a_session_that_actually_connects_the_nets() {
    let board = testkit::workspace_root().join("web/example.kicad_pcb");
    let dir = scratch("routes-to-session");
    let output = dir.join("out.ses");

    let (_, stderr, code) = run(&[
        "route",
        &board.to_string_lossy(),
        "-o",
        &output.to_string_lossy(),
        "--max-passes",
        "1",
        "--timeout",
        "120",
    ]);
    assert_eq!(code, 0, "route failed: {stderr}");

    let session = std::fs::read_to_string(&output).expect("a session was written");
    assert!(
        session.starts_with("(session"),
        "got {}",
        &session[..40.min(session.len())]
    );

    let wire_count = session.matches("(wire").count();
    assert_eq!(
        wire_count, 2,
        "expected one routed wire per net (Signal, Return), got {wire_count} in:\n{session}"
    );
    assert!(
        session.contains("Signal") && session.contains("Return"),
        "the session should name both nets from web/example.kicad_pcb, got:\n{session}"
    );
}
