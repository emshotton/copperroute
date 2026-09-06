#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

const PORT: &str = env!("CARGO_BIN_EXE_freerouting");

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("fr-cli-e2e").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir
}

fn stage_dsn(dir: &Path, source: &Path, name: &str) -> PathBuf {
    let target = dir.join(name);
    std::fs::copy(source, &target)
        .unwrap_or_else(|e| panic!("cannot stage {}: {e}", source.display()));
    target
}

fn small_dsn() -> PathBuf {
    parity::fixture("Issue143-rpi_splitter.dsn")
}

fn run(argv: &[&str]) -> (String, String, i32) {
    let (stdout, stderr, code) = parity::run_port_binary(Path::new(PORT), argv);
    (
        String::from_utf8_lossy(&stdout).into_owned(),
        String::from_utf8_lossy(&stderr).into_owned(),
        code,
    )
}

fn final_state(manifest: &Path) -> String {
    let text = std::fs::read_to_string(manifest)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", manifest.display()));
    let value: serde_json::Value =
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("manifest is not JSON: {e}"));
    value["final_state"]
        .as_str()
        .unwrap_or_else(|| panic!("manifest has no final_state: {text}"))
        .to_string()
}

fn settings_snapshot(manifest: &Path) -> serde_json::Value {
    let text = std::fs::read_to_string(manifest)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", manifest.display()));
    let value: serde_json::Value =
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("manifest is not JSON: {e}"));
    value
        .get("settings_snapshot")
        .cloned()
        .unwrap_or_else(|| panic!("manifest has no settings_snapshot: {text}"))
}

#[test]
fn a_failed_run_leaves_the_previous_result_on_disk() {
    let dir = scratch("failed-run-keeps-previous");
    let input = dir.join("board.dsn");
    std::fs::write(&input, b"(session previous)\n").unwrap();
    let output = dir.join("out.ses");
    const SENTINEL: &[u8] = b"PREVIOUS RESULT";
    std::fs::write(&output, SENTINEL).unwrap();

    let (_, _, code) = run(&[
        "route",
        &input.to_string_lossy(),
        "-o",
        &output.to_string_lossy(),
    ]);

    assert_eq!(code, 1, "a session under a .dsn name is not a board");
    assert_eq!(
        std::fs::read(&output).expect("the previous result is still there"),
        SENTINEL,
        "a failed run must leave the previous result byte for byte where it was"
    );
}

#[test]
fn an_empty_output_directory_is_not_unlinked() {
    let dir = scratch("empty-output-dir");
    let input = dir.join("board.dsn");
    std::fs::write(&input, b"(session previous)\n").unwrap();
    let output = dir.join("out.ses");
    std::fs::create_dir(&output).unwrap();

    let (_, _, code) = run(&[
        "route",
        &input.to_string_lossy(),
        "-o",
        &output.to_string_lossy(),
    ]);

    assert_eq!(code, 1);
    assert!(
        output.is_dir(),
        "the empty directory must still be there: nothing deletes it now"
    );
    assert_eq!(
        std::fs::read_dir(&output).unwrap().count(),
        0,
        "and nothing was written into it either"
    );
}

#[test]
fn an_unsupported_output_extension_is_refused_at_the_argument() {
    if !parity::require_reference_dir() {
        return;
    }
    let dir = scratch("unsupported-output-extension");
    let dsn = small_dsn().to_string_lossy().into_owned();

    for name in [
        "out.dsn",
        "out.scr",
        "out.frb",
        "out.txt",
        "out.rules",
        "out",
    ] {
        let output = dir.join(name);
        let (_, stderr, code) = run(&["route", &dsn, "-o", &output.to_string_lossy()]);
        assert_eq!(code, 2, "-o {name} is a usage error");
        assert!(
            stderr.contains(".ses") && stderr.contains(".json"),
            "-o {name}: the refusal must name the accepted formats, got:\n{stderr}"
        );
        assert!(
            !output.exists(),
            "-o {name}: no file may be created — not even a 0-byte one"
        );
    }

    for name in ["out.ses", "out.json"] {
        let output = dir.join(name);
        let (_, stderr, code) = run(&[
            "route",
            &dsn,
            "-o",
            &output.to_string_lossy(),
            "--max-passes",
            "1",
        ]);
        assert_eq!(code, 0, "-o {name} must still work:\n{stderr}");
        assert!(
            std::fs::metadata(&output).is_ok_and(|meta| meta.len() > 0),
            "-o {name} must hold a document"
        );
    }
}

#[test]
fn do_out_json_writes_the_routed_board() {
    if !parity::require_reference_dir() {
        return;
    }
    let dir = scratch("do-out-json");
    let json = dir.join("out.json");
    let ses = dir.join("out.ses");
    let dsn = small_dsn().to_string_lossy().into_owned();
    let run_to = |out: &Path| {
        run(&[
            "route",
            &dsn,
            "-o",
            &out.to_string_lossy(),
            "--max-passes",
            "8",
            "--set",
            "router.fanout.enabled=true",
            "--set",
            "router.optimizer.enabled=true",
        ])
    };

    let (_, _, code) = run_to(&json);
    assert_eq!(code, 0, "`-o out.json` is a writable output format");
    let written = std::fs::read_to_string(&json).expect("out.json was written");

    let (_, _, code) = run_to(&ses);
    assert_eq!(code, 0);
    let session = std::fs::read_to_string(&ses).expect("out.ses was written");

    let wires = session.matches("(wire").count();
    let vias = session.matches("(via ").count();
    assert_eq!(wires, 13, "the routed board the CLI reference records");
    assert_eq!(vias, 2);

    let traces = json_array_len(&written, "traces");
    assert_eq!(
        traces, wires,
        "the JSON must hold the same routed board the SES does:\n{written}"
    );
    assert_eq!(
        json_array_len(&written, "vias"),
        vias,
        "and the same vias:\n{written}"
    );
    assert!(
        !written.contains("\"traces\": []"),
        "the pre-routing board is exactly what this must not be"
    );
}

fn json_array_len(text: &str, key: &str) -> usize {
    let at = text
        .find(&format!("\"{key}\": "))
        .unwrap_or_else(|| panic!("the document has no `{key}` key:\n{text}"));
    let rest = &text[at..];
    if rest.starts_with(&format!("\"{key}\": []")) {
        return 0;
    }
    let open = rest.find('[').expect("an array");
    let mut depth = 0usize;
    let mut end = open;
    for (i, c) in rest[open..].char_indices() {
        match c {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    end = open + i;
                    break;
                }
            }
            _ => {}
        }
    }
    rest[open..end].matches("\"id\": ").count()
}

#[test]
fn a_non_ascii_session_file_is_read_as_utf8() {
    let dir = scratch("non-ascii-session");
    let board = dir.join("board.json");
    let session = dir.join("session.json");
    std::fs::write(
        &board,
        "{\"unit\":\"MM\",\"resolution\":1000.0,\
         \"layers\":[{\"index\":0,\"name\":\"F.Cu\",\"type\":\"signal\"},\
         {\"index\":1,\"name\":\"B.Cu\",\"type\":\"signal\"}],\
         \"nets\":[{\"id\":1,\"name\":\"GND_é中\",\"className\":\"default\"},\
         {\"id\":2,\"name\":\"VCC\",\"className\":\"default\"}],\
         \"outline\":{\"corners\":[{\"x\":0.0,\"y\":0.0},{\"x\":50.0,\"y\":0.0},\
         {\"x\":50.0,\"y\":40.0},{\"x\":0.0,\"y\":40.0}]},\
         \"components\":[{\"reference\":\"Ré1\",\"value\":\"1k\",\"footprint\":\"R_0603_é\",\
         \"position\":{\"x\":10.0,\"y\":10.0},\"rotation\":0.0,\"layer\":\"F.Cu\",\
         \"pads\":[{\"name\":\"1\",\"netName\":\"GND_é中\",\"shape\":\"rect\",\
         \"size\":{\"x\":1.0,\"y\":1.0},\"offset\":{\"x\":0.0,\"y\":0.0},\"drill\":0.0,\
         \"layers\":[\"F.Cu\"]},{\"name\":\"2\",\"netName\":\"VCC\",\"shape\":\"rect\",\
         \"size\":{\"x\":1.0,\"y\":1.0},\"offset\":{\"x\":2.0,\"y\":0.0},\"drill\":0.0,\
         \"layers\":[\"F.Cu\"]}]}]}",
    )
    .unwrap();
    std::fs::write(
        &session,
        "{\"unit\":\"MM\",\"resolution\":1000.0,\
         \"traces\":[{\"id\":1,\"netName\":\"GND_é中\",\"width\":0.25,\"layerIndex\":0,\
         \"points\":[{\"x\":1.0,\"y\":1.0},{\"x\":9.0,\"y\":1.0}]}],\
         \"vias\":[{\"id\":1,\"netName\":\"GND_é中\",\"position\":{\"x\":5.0,\"y\":5.0},\
         \"diameter\":0.8,\"drill\":0.4,\"startLayerIndex\":0,\"endLayerIndex\":1}]}",
    )
    .unwrap();

    let out = dir.join("out.ses");
    let (_, stderr, code) = run(&[
        "route",
        &board.to_string_lossy(),
        "-o",
        &out.to_string_lossy(),
        "--ses",
        &session.to_string_lossy(),
        "--max-passes",
        "1",
    ]);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(
        stderr.contains("KiCad JSON session loaded"),
        "the session import is logged: {stderr}"
    );
    let session_out = std::fs::read_to_string(&out).expect("out.ses was written");
    assert!(
        session_out.contains("(net \"GND_é中\""),
        "the non-ASCII net survived the decode: {session_out}"
    );
    assert!(
        session_out.contains("(via \"Via[0-1]_800:400_um\" 5000 -5000"),
        "the session's via is on the net"
    );
    assert!(
        session_out.contains("(path F.Cu 250"),
        "and so is the session's wire"
    );
}

#[test]
fn a_session_under_a_dsn_name_exits_1() {
    let dir = scratch("session-as-dsn");
    let input = dir.join("board.dsn");
    std::fs::write(&input, b"(session previous)\n").unwrap();
    let (stdout, stderr, code) = run(&[
        "route",
        &input.to_string_lossy(),
        "-o",
        &dir.join("out.ses").to_string_lossy(),
    ]);
    assert_eq!(code, 1);
    assert!(stdout.is_empty(), "nothing reaches stdout: {stdout}");
    assert!(stderr.contains("not a board"), "{stderr}");
}

#[test]
fn a_missing_rules_path_disables_rules_discovery() {
    if !parity::require_reference_dir() {
        return;
    }
    let dir = scratch("missing-rules");
    let dsn = stage_dsn(&dir, &small_dsn(), "board.dsn");
    std::fs::write(
        dir.join("board.rules"),
        b"(rules PCB board\n  (autoroute_settings\n    (via_costs 99)\n  )\n)\n",
    )
    .unwrap();
    let manifest = dir.join("m.json");

    let (_, _, code) = run(&[
        "route",
        &dsn.to_string_lossy(),
        "-o",
        &dir.join("a.ses").to_string_lossy(),
        "--max-passes",
        "1",
        "--result-json",
        &manifest.to_string_lossy(),
    ]);
    assert_eq!(code, 0);
    assert_eq!(
        settings_snapshot(&manifest)["scoring"]["via_costs"],
        serde_json::json!(99),
        "the adjacent board.rules must reach the answer when nothing else does"
    );

    let (_, _, code) = run(&[
        "route",
        &dsn.to_string_lossy(),
        "-o",
        &dir.join("b.ses").to_string_lossy(),
        "--rules",
        &dir.join("nosuch.rules").to_string_lossy(),
        "--max-passes",
        "1",
        "--result-json",
        &manifest.to_string_lossy(),
    ]);
    assert_eq!(code, 0);
    assert_eq!(
        settings_snapshot(&manifest)["scoring"]["via_costs"],
        serde_json::json!(50),
        "a named rules file that is missing switches discovery off"
    );
}

#[test]
fn no_banner_on_stdout() {
    if !parity::require_reference_dir() {
        return;
    }
    let dir = scratch("no-banner");
    let (stdout, _, code) = run(&[
        "route",
        &small_dsn().to_string_lossy(),
        "-o",
        &dir.join("out.ses").to_string_lossy(),
        "--max-passes",
        "1",
    ]);
    assert_eq!(code, 0);
    assert!(stdout.is_empty(), "stdout must be empty, got {stdout:?}");
}

#[test]
fn max_passes_zero_is_unlimited() {
    if !parity::require_reference_dir() {
        return;
    }
    let dir = scratch("max-passes-zero");
    let manifest = dir.join("m.json");
    let (_, _, code) = run(&[
        "route",
        &small_dsn().to_string_lossy(),
        "-o",
        &dir.join("out.ses").to_string_lossy(),
        "--max-passes",
        "0",
        "--result-json",
        &manifest.to_string_lossy(),
    ]);
    assert_eq!(code, 0);
    assert_eq!(
        settings_snapshot(&manifest)["max_passes"],
        serde_json::json!(9999),
        "0 means unlimited, which the settings clamp spells 9999"
    );
}

#[test]
fn max_passes_and_timeout_reach_the_settings() {
    if !parity::require_reference_dir() {
        return;
    }
    let dir = scratch("max-passes-timeout");
    let manifest = dir.join("m.json");
    let (_, stderr, code) = run(&[
        "route",
        &small_dsn().to_string_lossy(),
        "-o",
        &dir.join("out.ses").to_string_lossy(),
        "--max-passes",
        "3",
        "--timeout",
        "1:00:00",
        "--result-json",
        &manifest.to_string_lossy(),
    ]);
    assert_eq!(code, 0, "{stderr}");
    let snapshot = settings_snapshot(&manifest);
    assert_eq!(snapshot["max_passes"], serde_json::json!(3));
    assert_eq!(snapshot["job_timeout"], serde_json::json!("1:00:00"));
}

#[test]
fn the_rules_file_is_read_as_bytes_twice() {
    if !parity::require_reference_dir() {
        return;
    }
    let dir = scratch("rules-twice");
    let dsn = stage_dsn(&dir, &small_dsn(), "board.dsn");
    std::fs::write(
        dir.join("board.rules"),
        b"(rules PCB board\n  (autoroute_settings\n    (via_costs 99)\n    (plane_via_costs 7)\n  )\n)\n",
    )
    .unwrap();
    let manifest = dir.join("m.json");
    let (_, _, code) = run(&[
        "route",
        &dsn.to_string_lossy(),
        "-o",
        &dir.join("out.ses").to_string_lossy(),
        "--max-passes",
        "1",
        "--result-json",
        &manifest.to_string_lossy(),
    ]);
    assert_eq!(code, 0);
    let scoring = settings_snapshot(&manifest);
    assert_eq!(scoring["scoring"]["via_costs"], serde_json::json!(99));
    assert_eq!(scoring["scoring"]["plane_via_costs"], serde_json::json!(7));
}

#[test]
fn set_reaches_the_run_and_the_dotted_spelling_is_a_usage_error() {
    if !parity::require_reference_dir() {
        return;
    }
    let dir = scratch("set");
    let dsn = small_dsn();
    let dsn = dsn.to_string_lossy();
    let via_costs = |manifest: &Path| settings_snapshot(manifest)["scoring"]["via_costs"].clone();

    for (name, spelling) in [
        ("space", vec!["--set", "router.scoring.via_costs=77"]),
        ("equals", vec!["--set=router.scoring.via_costs=77"]),
    ] {
        let manifest = dir.join(format!("{name}.json"));
        let manifest_text = manifest.to_string_lossy().into_owned();
        let ses = dir
            .join(format!("{name}.ses"))
            .to_string_lossy()
            .into_owned();
        let mut argv = vec![
            "route",
            &dsn,
            "-o",
            &ses,
            "--max-passes",
            "1",
            "--result-json",
            &manifest_text,
        ];
        argv.extend(spelling);
        let (_, stderr, code) = run(&argv);
        assert_eq!(code, 0, "{name}: {stderr}");
        assert_eq!(via_costs(&manifest), serde_json::json!(77), "{name}");
    }

    let (_, stderr, code) = run(&[
        "route",
        &dsn,
        "-o",
        &dir.join("d.ses").to_string_lossy(),
        "--router.scoring.via_costs=77",
    ]);
    assert_eq!(code, 2, "{stderr}");
    assert!(stderr.contains("unexpected argument"), "{stderr}");

    let (_, stderr, code) = run(&[
        "route",
        &dsn,
        "-o",
        &dir.join("e.ses").to_string_lossy(),
        "--set",
        "scoring.via_costs=77",
    ]);
    assert_eq!(
        code, 1,
        "a --set without the router. prefix is refused: {stderr}"
    );
    assert!(stderr.contains("router."), "{stderr}");
}

#[test]
fn a_settings_file_reaches_the_run_and_the_working_directory_is_not_read() {
    if !parity::require_reference_dir() {
        return;
    }
    let dir = scratch("settings-file");
    let json = dir.join("s.json");
    std::fs::write(&json, r#"{"router": {"scoring": {"via_costs": 77}}}"#).unwrap();
    let cwd = dir.join("cwd");
    std::fs::create_dir_all(&cwd).unwrap();
    std::fs::write(
        cwd.join("freerouting.json"),
        r#"{"router": {"scoring": {"via_costs": 77}}}"#,
    )
    .unwrap();
    let dsn = small_dsn();
    let dsn = dsn.to_string_lossy();
    let via_costs = |manifest: &Path| settings_snapshot(manifest)["scoring"]["via_costs"].clone();

    let b = dir.join("b.json");
    let (_, stderr, code) = run(&[
        "--settings",
        &json.to_string_lossy(),
        "route",
        &dsn,
        "-o",
        &dir.join("b.ses").to_string_lossy(),
        "--max-passes",
        "1",
        "--result-json",
        &b.to_string_lossy(),
    ]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(via_costs(&b), serde_json::json!(77));

    let e = dir.join("e.json");
    let output = std::process::Command::new(PORT)
        .current_dir(&cwd)
        .args([
            "route",
            &dsn,
            "-o",
            &dir.join("e.ses").to_string_lossy(),
            "--max-passes",
            "1",
            "--result-json",
            &e.to_string_lossy(),
        ])
        .stdin(std::process::Stdio::null())
        .output()
        .expect("the port runs");
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        via_costs(&e),
        serde_json::json!(50),
        "no default settings file is read"
    );

    let (_, stderr, code) = run(&[
        "--settings",
        "/nonexistent/s.json",
        "route",
        &dsn,
        "-o",
        &dir.join("f.ses").to_string_lossy(),
    ]);
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains("/nonexistent/s.json"), "{stderr}");
}

#[test]
fn final_state_distinguishes_stage_limits_from_job_deadlines() {
    if !parity::require_reference_dir() {
        return;
    }
    let dir = scratch("stage-timeout");
    let dsn = small_dsn();
    let dsn = dsn.to_string_lossy();

    let manifest = dir.join("stage.json");
    let (_, stderr, code) = run(&[
        "route",
        &dsn,
        "-o",
        &dir.join("stage.ses").to_string_lossy(),
        "--max-passes",
        "1",
        "--set",
        "router.optimizer.timeout=0:00:00",
        "--result-json",
        &manifest.to_string_lossy(),
    ]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(final_state(&manifest), "COMPLETED");

    let manifest = dir.join("job.json");
    let (_, stderr, code) = run(&[
        "route",
        &dsn,
        "-o",
        &dir.join("job.ses").to_string_lossy(),
        "--max-passes",
        "1",
        "--timeout",
        "0:00:00",
        "--result-json",
        &manifest.to_string_lossy(),
    ]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(final_state(&manifest), "TIMED_OUT");
}

#[test]
fn an_invalid_input_writes_no_manifest() {
    let dir = scratch("final-state-invalid");
    let input = dir.join("board.dsn");
    std::fs::write(&input, b"(session previous)\n").unwrap();
    let manifest = dir.join("m.json");
    let (_, stderr, code) = run(&[
        "route",
        &input.to_string_lossy(),
        "-o",
        &dir.join("out.ses").to_string_lossy(),
        "--result-json",
        &manifest.to_string_lossy(),
    ]);
    assert_eq!(code, 1, "{stderr}");
    assert!(!manifest.exists());
}

#[test]
fn the_via_net_number_fixture_routes_instead_of_hanging() {
    let dir = scratch("via-net-numbers");
    let fixture =
        parity::workspace_root().join("crates/fr-dsn/tests/data/p8t13-via-net-numbers.dsn");
    let ses = dir.join("out.ses");
    let (_, stderr, code) = run(&[
        "route",
        &fixture.to_string_lossy(),
        "-o",
        &ses.to_string_lossy(),
        "--set",
        "router.optimizer.enabled=false",
    ]);
    assert_eq!(code, 0, "the fixture must route: {stderr}");

    let bytes = std::fs::read(&ses).expect("the run must write a .ses");
    assert_eq!(bytes.len(), 1_843, "the routed SES with the optimizer off");

    let text = String::from_utf8(bytes).expect("the SES is UTF-8");
    assert!(text.starts_with("(session \"p8t13-via-net-numbers\""));
    assert!(
        text.contains("(net NORDERED"),
        "the routed net must be named in the session"
    );
    assert!(
        text.contains("(via VIA1 "),
        "the fixture's one via must reach the output"
    );

    let dsn = std::fs::read(&fixture).expect("the fixture");
    let options = fr_dsn::DsnReadOptions::default();
    let fr_dsn::BoardReadResult::Success {
        mut board,
        coordinate_transform,
        ..
    } = fr_dsn::read_board(
        dsn.as_slice(),
        None,
        Some("p8t13-via-net-numbers.dsn"),
        &options,
    )
    else {
        panic!("the fixture must read")
    };
    let mut board = *board.take().expect("a board");
    let coordinate_transform = coordinate_transform.expect("a coordinate transform");
    let summary = fr_dsn::ses_reader::read(text.as_bytes(), &mut board, &coordinate_transform)
        .expect("the port must read back the session it just wrote");
    assert_eq!(
        summary.errors_encountered, 0,
        "the re-read must reach no error"
    );
    assert!(
        summary.wires_imported > 0 && summary.vias_imported > 0,
        "the session must carry the routed wires and the fixture's via: {summary:?}"
    );
}

fn drc_dsn() -> PathBuf {
    parity::fixture("Issue575-drc_dev-board_4_hole_clearance_violations.dsn")
}

fn report(path: &Path) -> serde_json::Value {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("the report is not JSON: {e}"))
}

fn autoroute_settings_rules(dir: &Path, via_costs: i32) -> PathBuf {
    let path = dir.join("scoring.rules");
    std::fs::write(
        &path,
        format!("(rules PCB scoring\n  (autoroute_settings\n    (via_costs {via_costs})\n  )\n)\n"),
    )
    .expect("the scratch file is writable");
    path
}

#[test]
fn drc_exits_1_on_violations_and_0_on_a_clean_board() {
    if !parity::require_reference_dir() {
        return;
    }
    let dir = scratch("drc-exit");
    let out = dir.join("dirty.json");
    let (_, stderr, code) = run(&[
        "drc",
        &drc_dsn().to_string_lossy(),
        "-o",
        &out.to_string_lossy(),
    ]);
    assert_eq!(code, 1, "{stderr}");
    assert_eq!(report(&out)["violations"].as_array().unwrap().len(), 8);

    let clean = dir.join("clean.json");
    let tutorial = parity::reference_dir().join("examples/tutorial_board/tutorial_board.dsn");
    let (_, stderr, code) = run(&[
        "drc",
        &tutorial.to_string_lossy(),
        "-o",
        &clean.to_string_lossy(),
    ]);
    assert_eq!(code, 0, "{stderr}");
    assert!(report(&clean)["violations"].as_array().unwrap().is_empty());
}

#[test]
fn drc_only_warns_about_a_missing_rules_or_session_file() {
    if !parity::require_reference_dir() {
        return;
    }
    let dir = scratch("drc-missing-aux");
    let dsn = drc_dsn();
    for (flag, value) in [("--rules", "nosuch.rules"), ("--ses", "nosuch.ses")] {
        let out = dir.join(format!("{value}.json"));
        let (_, stderr, code) = run(&[
            "drc",
            &dsn.to_string_lossy(),
            flag,
            &dir.join(value).to_string_lossy(),
            "-o",
            &out.to_string_lossy(),
        ]);
        assert_eq!(
            code, 1,
            "the report is written and the board's violations set the code:\n{stderr}"
        );
        assert!(
            stderr.contains("not read") || stderr.contains("not found"),
            "{stderr}"
        );
        assert!(out.is_file());
    }
}

#[test]
fn drc_exits_1_when_the_input_is_unreadable_or_not_a_board_or_unwritable() {
    if !parity::require_reference_dir() {
        return;
    }
    let dir = scratch("drc-refusals");
    let (_, stderr, code) = run(&[
        "drc",
        "/nonexistent/board.dsn",
        "-o",
        &dir.join("a.json").to_string_lossy(),
    ]);
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains("Couldn't load the input file"), "{stderr}");

    let session = dir.join("session.dsn");
    std::fs::write(&session, b"(session previous)\n").unwrap();
    let (_, stderr, code) = run(&[
        "drc",
        &session.to_string_lossy(),
        "-o",
        &dir.join("b.json").to_string_lossy(),
    ]);
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains("not a board"), "{stderr}");

    let out = dir.join("nodir").join("r.json");
    let (_, stderr, code) = run(&[
        "drc",
        &drc_dsn().to_string_lossy(),
        "-o",
        &out.to_string_lossy(),
    ]);
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains("Couldn't save the DRC report"), "{stderr}");
    assert!(!out.exists());
}

#[test]
fn the_session_is_imported_after_the_rules() {
    if !parity::require_reference_dir() {
        return;
    }
    let dir = scratch("drc-load-order");
    let dsn = parity::fixture("Issue593-BBD_Mars-64.dsn");
    let ses = parity::fixture("Issue593-BBD_Mars-64.ses");

    let typed = dir.join("smd_via.rules");
    std::fs::write(
        &typed,
        "(rules PCB Issue593-BBD_Mars-64\n  (rule\n    (clearance 400 (type smd_via))\n  )\n)\n",
    )
    .expect("the scratch file is writable");

    let mut request = freerouting::ops::load::LoadRequest::for_board(
        freerouting::ops::load::BoardSource::Path(dsn.clone()),
    );
    request.rules = Some(typed.clone());
    request.session = Some(ses.clone());
    let mut loaded = freerouting::ops::load::load(&request).expect("the fixture loads");
    let expected = fr_drc::DesignRulesChecker::new(&mut loaded.board)
        .get_all_violations()
        .len();

    let out = dir.join("r.json");
    let (_, stderr, code) = run(&[
        "drc",
        &dsn.to_string_lossy(),
        "--rules",
        &typed.to_string_lossy(),
        "--ses",
        &ses.to_string_lossy(),
        "-o",
        &out.to_string_lossy(),
    ]);
    assert!(code == 0 || code == 1, "{stderr}");
    let clearance = report(&out)["violations"]
        .as_array()
        .expect("violations is an array")
        .iter()
        .filter(|violation| {
            !matches!(
                violation["type"].as_str(),
                Some("track_dangling") | Some("via_dangling")
            )
        })
        .count();
    assert_eq!(
        clearance, expected,
        "the CLI builds the same board the loader builds: rules first, then the session"
    );

    let rules_at = stderr
        .find("Rules loaded from")
        .unwrap_or_else(|| panic!("the rules file is not mentioned:\n{stderr}"));
    let session_at = stderr
        .find("Session loaded from")
        .unwrap_or_else(|| panic!("the session import is not logged:\n{stderr}"));
    assert!(rules_at < session_at, "rules before session:\n{stderr}");
}

#[test]
fn the_quality_score_uses_a_dsn_only_merge() {
    if !parity::require_reference_dir() {
        return;
    }
    let dir = scratch("drc-separate-merge");
    let rules = autoroute_settings_rules(&dir, 999);
    let dsn = drc_dsn();

    let manifest = dir.join("route.json");
    let (_, stderr, code) = run(&[
        "route",
        &small_dsn().to_string_lossy(),
        "-o",
        &dir.join("out.ses").to_string_lossy(),
        "--max-passes",
        "1",
        "--rules",
        &rules.to_string_lossy(),
        "--result-json",
        &manifest.to_string_lossy(),
    ]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(
        settings_snapshot(&manifest)["scoring"]["via_costs"].as_i64(),
        Some(999),
        "the rules file reaches the router path"
    );

    let with_rules = dir.join("with-rules.json");
    let (_, stderr, code) = run(&[
        "drc",
        &dsn.to_string_lossy(),
        "--rules",
        &rules.to_string_lossy(),
        "-o",
        &with_rules.to_string_lossy(),
    ]);
    assert_eq!(code, 1, "{stderr}");
    let without_rules = dir.join("without-rules.json");
    let (_, stderr, code) = run(&[
        "drc",
        &dsn.to_string_lossy(),
        "-o",
        &without_rules.to_string_lossy(),
    ]);
    assert_eq!(code, 1, "{stderr}");

    let mut with = report(&with_rules);
    let mut without = report(&without_rules);
    assert_eq!(
        with["quality_score"], without["quality_score"],
        "a rules file does not reach the quality score's merge"
    );
    with["date"] = serde_json::Value::Null;
    without["date"] = serde_json::Value::Null;
    assert_eq!(
        with, without,
        "an `(autoroute_settings …)`-only rules file changes nothing on the DRC path at all"
    );
}

#[test]
fn the_quality_score_is_an_f32_widened_to_f64() {
    if !parity::require_reference_dir() {
        return;
    }
    let dir = scratch("drc-score-width");
    let out = dir.join("r.json");
    let (_, stderr, code) = run(&[
        "drc",
        &drc_dsn().to_string_lossy(),
        "-o",
        &out.to_string_lossy(),
    ]);
    assert_eq!(code, 1, "{stderr}");

    let text = std::fs::read_to_string(&out).expect("the report is readable");
    assert!(
        text.contains("\"quality_score\": 906.2450561523438"),
        "the score is the widened float, verbatim:\n{text}"
    );

    let score = report(&out)["quality_score"]
        .as_f64()
        .expect("quality_score is a number");
    #[allow(clippy::cast_possible_truncation)]
    let narrowed = score as f32;
    assert_eq!(f64::from(narrowed), score);
}

#[test]
fn every_committed_reference_score_is_recomputed() {
    if !parity::require_reference_dir() {
        return;
    }
    let dir = scratch("drc-reference-scores");
    let table = parity::workspace_root().join("tests/reference/drc-fixtures.txt");
    let text = std::fs::read_to_string(&table)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", table.display()));

    let mut checked = 0usize;
    for line in text.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut fields = line.split('|');
        let mut next = || fields.next().unwrap_or_default().trim().to_string();
        let (stem, dsn, rules, ses) = (next(), next(), next(), next());

        let reference_path = parity::reference(&stem, "drc.json");
        if !parity::require_reference(&reference_path) {
            continue;
        }
        let reference_text =
            std::fs::read_to_string(&reference_path).expect("the reference is readable");
        let expected = parity::parse_drc_json(&reference_text)
            .unwrap_or_else(|e| panic!("{stem}: the reference does not parse: {e}"))
            .quality_score
            .unwrap_or_else(|| panic!("{stem}: the reference carries no qualityScore"));

        let out = dir.join(format!("{stem}.json"));
        let mut argv = vec![
            "drc".to_string(),
            parity::reference_dir().join(&dsn).to_string_lossy().into_owned(),
        ];
        if !rules.is_empty() {
            argv.push("--rules".to_string());
            argv.push(
                parity::reference_dir()
                    .join(&rules)
                    .to_string_lossy()
                    .into_owned(),
            );
        }
        if !ses.is_empty() {
            argv.push("--ses".to_string());
            argv.push(parity::reference_dir().join(&ses).to_string_lossy().into_owned());
        }
        argv.push("-o".to_string());
        argv.push(out.to_string_lossy().into_owned());
        let argv_refs: Vec<&str> = argv.iter().map(String::as_str).collect();
        let (_, stderr, code) = run(&argv_refs);
        assert!(code == 0 || code == 1, "{stem}: {stderr}");

        let actual = report(&out)["quality_score"]
            .as_f64()
            .unwrap_or_else(|| panic!("{stem}: the report carries no quality_score"));
        assert_eq!(
            actual, expected,
            "{stem}: the computed score must equal the recorded one, bit for bit"
        );
        checked += 1;
    }
    assert_eq!(
        checked, 8,
        "all eight `drc-*` stems must be checked; `tests/reference/drc-fixtures.txt` has eight rows"
    );
}

#[test]
fn drc_with_no_output_prints_to_stdout() {
    if !parity::require_reference_dir() {
        return;
    }
    let (stdout, stderr, code) = run(&["drc", &drc_dsn().to_string_lossy()]);
    assert_eq!(code, 1, "{stderr}");
    let document: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout is not one JSON document: {e}\n{stdout}"));
    assert_eq!(
        document["$schema"].as_str(),
        Some("https://schemas.kicad.org/drc.v1.json")
    );
    assert!(document["violations"].is_array());
    assert!(
        stdout.trim_start().starts_with('{'),
        "nothing may precede the document on stdout"
    );
    assert!(
        stderr.contains("violation"),
        "the log stays on stderr\n{stderr}"
    );
}

#[test]
fn info_writes_the_board_summary_to_stdout() {
    if !parity::require_reference_dir() {
        return;
    }
    let dsn = small_dsn();
    let (stdout, stderr, code) = run(&["info", &dsn.to_string_lossy()]);
    assert_eq!(code, 0, "{stderr}");
    let document: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout is not one JSON document: {e}\n{stdout}"));
    assert!(
        stdout.trim_start().starts_with('{'),
        "nothing may precede the document on stdout"
    );

    let order: Vec<usize> = ["layers", "nets", "components", "statistics", "metadata"]
        .iter()
        .map(|key| {
            stdout
                .find(&format!("\n  \"{key}\""))
                .unwrap_or_else(|| panic!("no top-level `{key}` in\n{stdout}"))
        })
        .collect();
    assert!(order.windows(2).all(|w| w[0] < w[1]), "{order:?}");

    assert_eq!(
        document["statistics"]["nets"]["total_count"].as_u64(),
        Some(document["nets"].as_array().expect("nets").len() as u64)
    );
    assert_eq!(document["metadata"]["unit"], "mil");
    assert_eq!(
        document["layers"]
            .as_array()
            .expect("layers")
            .iter()
            .map(|l| l["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["1#Top", "16#Bottom"]
    );
}

#[test]
fn info_exits_1_on_an_unreadable_input_and_on_an_unloadable_board() {
    if !parity::require_reference_dir() {
        return;
    }
    let (stdout, stderr, code) = run(&["info", "/nonexistent/board.dsn"]);
    assert_eq!(code, 1, "{stderr}");
    assert!(stdout.is_empty(), "{stdout}");
    assert!(stderr.contains("Couldn't load the input file"), "{stderr}");

    let ses = parity::fixture("Issue593-BBD_Mars-64.ses");
    let (stdout, stderr, code) = run(&["info", &ses.to_string_lossy()]);
    assert_eq!(code, 1, "{stderr}");
    assert!(stdout.is_empty(), "{stdout}");
    assert!(stderr.contains("not a board"), "{stderr}");
}

#[test]
fn the_cli_passes_kicad_flavor_explicitly() {
    if !parity::require_reference_dir() {
        return;
    }
    let dir = scratch("drc-flavor");
    let dsn = drc_dsn();

    let kicad = dir.join("kicad.json");
    let (_, stderr, code) = run(&[
        "drc",
        &dsn.to_string_lossy(),
        "-o",
        &kicad.to_string_lossy(),
    ]);
    assert_eq!(code, 1, "{stderr}");
    let kicad_text = std::fs::read_to_string(&kicad).expect("readable");
    for key in [
        "\"coordinate_units\"",
        "\"kicad_version\"",
        "\"freerouting_version\"",
        "\"unconnected_items\"",
        "\"schematic_parity\"",
        "\"quality_score\"",
        "\"type\": \"track_dangling\"",
    ] {
        assert!(
            kicad_text.contains(key),
            "the default must be KiCad's spelling: {key} missing"
        );
    }
    for key in [
        "\"coordinateUnits\"",
        "\"unconnectedItems\"",
        "\"qualityScore\"",
    ] {
        assert!(
            !kicad_text.contains(key),
            "the camelCase spelling must not appear: {key}"
        );
    }

    let head = dir.join("head.json");
    let (_, stderr, code) = run(&[
        "drc",
        &dsn.to_string_lossy(),
        "-o",
        &head.to_string_lossy(),
        "--schema",
        "freerouting",
    ]);
    assert_eq!(code, 1, "{stderr}");
    let head_text = std::fs::read_to_string(&head).expect("readable");
    assert!(head_text.contains("\"coordinateUnits\""));
    assert!(head_text.contains("\"qualityScore\""));
    assert!(!head_text.contains("\"quality_score\""));
    assert_ne!(
        kicad_text, head_text,
        "the two flavors must be genuinely different documents"
    );
    parity::parse_drc_json(&head_text).expect("the camelCase flavour parses");
    parity::parse_drc_json(&kicad_text).expect("the KiCad flavour parses");
}

fn climb_one(stem: &parity::CliStem) -> Result<(), String> {
    let dir = scratch(&format!("ref-{}", stem.name));
    let argv = parity::cli_argv(&stem.name, &dir);
    let argv_refs: Vec<&str> = argv.iter().map(String::as_str).collect();
    let (stdout, stderr, code) = parity::run_port_binary(Path::new(PORT), &argv_refs);

    if parity::regolden_label().is_some() {
        return regolden_one(stem, &dir, &argv, &stdout, &stderr, code);
    }

    let expected_code: i32 =
        std::fs::read_to_string(parity::cli_reference(&stem.name, "route.exit"))
            .map_err(|e| format!("route.exit: {e}"))?
            .trim()
            .parse()
            .map_err(|e| format!("route.exit is not a number: {e}"))?;
    if code != expected_code {
        return Err(format!(
            "exit code {code} != the reference's {expected_code}\n--- stderr ---\n{}",
            String::from_utf8_lossy(&stderr)
        ));
    }

    let expected_ses = std::fs::read_to_string(parity::cli_reference(&stem.name, "route.ses"))
        .map_err(|e| format!("route.ses: {e}"))?;
    let expected_ses = parity::normalize_ses_head_tokens(&expected_ses);
    let actual_ses = std::fs::read_to_string(dir.join("route.ses"))
        .map_err(|e| format!("the run wrote no route.ses: {e}"))?;
    if actual_ses != expected_ses {
        let at = actual_ses
            .bytes()
            .zip(expected_ses.bytes())
            .position(|(a, b)| a != b)
            .unwrap_or_else(|| actual_ses.len().min(expected_ses.len()));
        return Err(format!(
            "SES differs at byte {at} (run {} B, reference {} B)\n  run: {:?}\n  ref: {:?}",
            actual_ses.len(),
            expected_ses.len(),
            actual_ses.get(at.saturating_sub(40)..(at + 40).min(actual_ses.len())),
            expected_ses.get(at.saturating_sub(40)..(at + 40).min(expected_ses.len())),
        ));
    }

    let manifest = dir.join("manifest.json");
    let mut manifest_argv = argv.clone();
    manifest_argv.push("--result-json".to_string());
    manifest_argv.push(manifest.display().to_string());
    let manifest_refs: Vec<&str> = manifest_argv.iter().map(String::as_str).collect();
    let (_, manifest_stderr, manifest_code) =
        parity::run_port_binary(Path::new(PORT), &manifest_refs);
    if manifest_code != expected_code {
        return Err(format!(
            "exit code {manifest_code} != the reference's {expected_code} on the manifest run\n{}",
            String::from_utf8_lossy(&manifest_stderr)
        ));
    }
    let expected_manifest =
        std::fs::read_to_string(parity::cli_reference(&stem.name, "manifest.json"))
            .map_err(|e| format!("manifest.json: {e}"))?;
    let actual_manifest = std::fs::read_to_string(&manifest)
        .map_err(|e| format!("the run wrote no manifest: {e}"))?;
    let expected_manifest = parity::normalize_manifest(&expected_manifest);
    let actual_manifest = parity::normalize_manifest(&actual_manifest);
    if actual_manifest != expected_manifest {
        return Err(format!(
            "manifest differs\n--- run ---\n{}\n--- reference ---\n{}",
            actual_manifest.to_pretty(),
            expected_manifest.to_pretty()
        ));
    }
    Ok(())
}

fn regolden_one(
    stem: &parity::CliStem,
    dir: &Path,
    argv: &[String],
    _stdout: &[u8],
    _stderr: &[u8],
    code: i32,
) -> Result<(), String> {
    let reference = |file: &str| parity::cli_reference(&stem.name, file);
    std::fs::write(reference("route.exit"), format!("{code}\n")).map_err(|e| e.to_string())?;
    let ses = std::fs::read(dir.join("route.ses")).map_err(|e| format!("route.ses: {e}"))?;
    std::fs::write(reference("route.ses"), ses).map_err(|e| e.to_string())?;

    let manifest = dir.join("manifest.json");
    let mut manifest_argv = argv.to_vec();
    manifest_argv.push("--result-json".to_string());
    manifest_argv.push(manifest.display().to_string());
    let manifest_refs: Vec<&str> = manifest_argv.iter().map(String::as_str).collect();
    let (_, _, manifest_code) = parity::run_port_binary(Path::new(PORT), &manifest_refs);
    if manifest_code != code {
        return Err(format!(
            "exit code {manifest_code} != {code} on the manifest run"
        ));
    }
    let written = std::fs::read(&manifest).map_err(|e| format!("manifest.json: {e}"))?;
    std::fs::write(reference("manifest.json"), written).map_err(|e| e.to_string())?;
    Ok(())
}

fn climb(ci_only: bool) {
    if !parity::require_reference_dir() {
        return;
    }
    let mut failures = Vec::new();
    let mut checked = 0;
    for stem in parity::cli_stems() {
        if ci_only && !stem.ci {
            continue;
        }
        if !parity::cli_reference(&stem.name, "route.ses").exists() {
            eprintln!(
                "SKIP: cli-{} has no reference — cut one with FR_REGOLDEN=<label>",
                stem.name
            );
            continue;
        }
        checked += 1;
        if let Err(why) = climb_one(&stem) {
            failures.push(format!("cli-{}: {why}", stem.name));
        }
    }
    assert!(checked > 0, "no stem was checked");
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

#[test]
#[cfg_attr(debug_assertions, ignore)]
fn the_slow_stems_match_the_reference() {
    if std::env::var_os("FR_SLOW_PARITY").is_none() {
        eprintln!("SKIP: set FR_SLOW_PARITY=1 to run the slow CLI reference lane");
        return;
    }
    climb(false);
}

#[test]
fn two_runs_of_every_ci_stem_are_byte_identical() {
    if !parity::require_reference_dir() {
        return;
    }
    let mut failures = Vec::new();
    let mut checked = 0;

    for stem in parity::cli_stems() {
        if !stem.ci {
            continue;
        }
        checked += 1;

        let mut runs = Vec::new();
        for pass in 0..2 {
            let dir = scratch(&format!("identity-{}-{pass}", stem.name));
            let manifest = dir.join("manifest.json");
            let mut argv = parity::cli_argv(&stem.name, &dir);
            argv.push("--result-json".to_string());
            argv.push(manifest.display().to_string());
            let argv_refs: Vec<&str> = argv.iter().map(String::as_str).collect();
            let (_stdout, stderr, code) = parity::run_port_binary(Path::new(PORT), &argv_refs);
            let ses = std::fs::read(dir.join("route.ses")).unwrap_or_else(|e| {
                panic!(
                    "cli-{} pass {pass} wrote no route.ses: {e}\n{}",
                    stem.name,
                    String::from_utf8_lossy(&stderr)
                )
            });
            let manifest = std::fs::read_to_string(&manifest)
                .unwrap_or_else(|e| panic!("cli-{} pass {pass} wrote no manifest: {e}", stem.name));
            runs.push((code, ses, stable_manifest(&manifest)));
        }

        let (code_a, ses_a, manifest_a) = &runs[0];
        let (code_b, ses_b, manifest_b) = &runs[1];

        if code_a != code_b {
            failures.push(format!(
                "cli-{}: exit code {code_a} then {code_b} — the same argv on the same binary",
                stem.name
            ));
        }
        if ses_a != ses_b {
            let at = ses_a
                .iter()
                .zip(ses_b.iter())
                .position(|(a, b)| a != b)
                .unwrap_or_else(|| ses_a.len().min(ses_b.len()));
            failures.push(format!(
                "cli-{}: the SES is NOT reproducible — two runs differ at byte {at} ({} B then \
                 {} B): something in the run depends on the machine rather than on the board.",
                stem.name,
                ses_a.len(),
                ses_b.len()
            ));
        }
        if manifest_a != manifest_b {
            failures.push(format!(
                "cli-{}: the manifest is not reproducible once timestamps and durations are \
                 removed:\n--- run 1 ---\n{manifest_a}\n--- run 2 ---\n{manifest_b}",
                stem.name
            ));
        }
    }

    assert!(checked > 0, "no CI stem was checked");
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

fn stable_manifest(text: &str) -> String {
    let mut value: serde_json::Value = serde_json::from_str(text).expect("the manifest is JSON");
    if let Some(object) = value.as_object_mut() {
        for volatile in ["generated_at", "phases", "resource_usage"] {
            object.remove(volatile);
        }
        if let Some(settings) = object
            .get_mut("settings_snapshot")
            .and_then(serde_json::Value::as_object_mut)
        {
            settings.remove("result_json");
        }
    }
    serde_json::to_string_pretty(&value).expect("re-serialises")
}

#[test]
fn every_stem_has_a_reference_and_every_reference_has_a_stem() {
    let stems = parity::cli_stems();
    assert!(!stems.is_empty(), "cli-fixtures.txt has rows");
    let root = parity::workspace_root().join("tests").join("reference");
    for stem in &stems {
        for file in [
            "argv.txt",
            "route.ses",
            "route.exit",
            "manifest.json",
            "meta.txt",
        ] {
            let path = parity::cli_reference(&stem.name, file);
            assert!(path.exists(), "missing {}", path.display());
        }
    }
    let mut orphans = Vec::new();
    for entry in std::fs::read_dir(&root).expect("tests/reference is readable") {
        let entry = entry.expect("a directory entry");
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(stem) = name.strip_prefix("cli-") else {
            continue;
        };
        if !entry.path().is_dir() {
            continue;
        }
        if !stems.iter().any(|s| s.name == stem) {
            orphans.push(name);
        }
    }
    assert!(
        orphans.is_empty(),
        "reference directories with no cli-fixtures.txt row: {orphans:?}"
    );
}

#[test]
fn the_cli_reference_agrees_with_the_batch_reference() {
    for stem in parity::cli_stems() {
        let batch = parity::reference(&stem.name, "batch.ses");
        if !batch.exists() {
            continue;
        }
        let cli = parity::cli_reference(&stem.name, "route.ses");
        if !cli.exists() {
            continue;
        }
        assert_eq!(
            std::fs::read(&cli).unwrap(),
            std::fs::read(&batch).unwrap(),
            "cli-{}/route.ses != {}/batch.ses",
            stem.name,
            stem.name
        );
    }
}
