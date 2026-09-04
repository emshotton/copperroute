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
        "-de",
        &input.to_string_lossy(),
        "-do",
        &output.to_string_lossy(),
    ]);

    assert_eq!(code, 1, "a SES input is `BoardLoader.java:33`'s refusal");
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
        "-de",
        &input.to_string_lossy(),
        "-do",
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
    if !parity::require_java_dir() {
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
        let (_, stderr, code) = run(&["-de", &dsn, "-do", &output.to_string_lossy(), "-mp", "1"]);
        assert_eq!(code, 1, "-do {name} must be refused");
        assert!(
            stderr.contains("is not an output file this program can write")
                && stderr.contains(".ses")
                && stderr.contains(".json"),
            "-do {name}: the refusal must name the accepted formats, got:\n{stderr}"
        );
        assert!(
            !output.exists(),
            "-do {name}: no file may be created — not even a 0-byte one"
        );
    }

    for name in ["out.ses", "out.json"] {
        let output = dir.join(name);
        let (_, stderr, code) = run(&["-de", &dsn, "-do", &output.to_string_lossy(), "-mp", "1"]);
        assert_eq!(code, 0, "-do {name} must still work:\n{stderr}");
        assert!(
            std::fs::metadata(&output).is_ok_and(|meta| meta.len() > 0),
            "-do {name} must hold a document"
        );
    }

    let broken = dir.join("broken.dsn");
    std::fs::write(&broken, b"hello").unwrap();
    let (_, refused, code) = run(&[
        "-de",
        &broken.to_string_lossy(),
        "-do",
        &dir.join("late.dsn").to_string_lossy(),
    ]);
    assert_eq!(code, 1);
    assert!(
        refused.contains("is not an output file this program can write"),
        "the output extension is refused before anything reads the board:\n{refused}"
    );
    let (_, loaded, code) = run(&[
        "-de",
        &broken.to_string_lossy(),
        "-do",
        &dir.join("late.ses").to_string_lossy(),
    ]);
    assert_eq!(code, 1);
    assert!(
        !loaded.contains("is not an output file this program can write"),
        "with a writable -do the same input reaches the loader instead:\n{loaded}"
    );
    assert_ne!(
        refused, loaded,
        "the two runs must fail in two different places, or this case proves nothing"
    );
}

#[test]
fn do_out_json_writes_the_routed_board() {
    if !parity::require_java_dir() {
        return;
    }
    let dir = scratch("do-out-json");
    let json = dir.join("out.json");
    let ses = dir.join("out.ses");
    let dsn = small_dsn().to_string_lossy().into_owned();
    let run_to = |out: &Path| {
        run(&[
            "-de",
            &dsn,
            "-do",
            &out.to_string_lossy(),
            "-mp",
            "8",
            "--router.fanout.enabled=true",
            "--router.optimizer.enabled=true",
        ])
    };

    let (_, _, code) = run_to(&json);
    assert_eq!(code, 0, "`-do out.json` is a writable output format");
    let written = std::fs::read_to_string(&json).expect("out.json was written");

    let (_, _, code) = run_to(&ses);
    assert_eq!(code, 0);
    let session = std::fs::read_to_string(&ses).expect("out.ses was written");

    let wires = session.matches("(wire").count();
    let vias = session.matches("(via ").count();
    assert_eq!(wires, 16, "the routed board the jar transcript records");
    assert_eq!(vias, 9);

    let traces = json_array_len(&written, "traces");
    assert_eq!(
        traces, wires,
        "the JSON must hold the same routed board the SES does — it held `\"traces\": []` before \
         quirk #289 was fixed, on the identical argv:\n{written}"
    );
    assert_eq!(
        json_array_len(&written, "vias"),
        vias,
        "and the same vias:\n{written}"
    );
    assert!(
        !written.contains("\"traces\": []"),
        "the pre-routing board is exactly what this must no longer be"
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
        "-de",
        &board.to_string_lossy(),
        &session.to_string_lossy(),
        "-do",
        &out.to_string_lossy(),
        "-mp",
        "1",
    ]);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(
        stderr.contains("KiCad JSON session file loaded successfully"),
        "`RoutingJobScheduler.java:205-206`'s line: {stderr}"
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
fn de_a_ses_exits_1_instead_of_hanging() {
    if !parity::require_java_dir() {
        return;
    }
    let dir = scratch("de-a-ses");
    let input = dir.join("board.dsn");
    std::fs::write(&input, b"(session previous)\n").unwrap();
    let output = dir.join("out.ses");
    let (stdout, stderr, code) = run(&[
        "-de",
        &input.to_string_lossy(),
        "-do",
        &output.to_string_lossy(),
    ]);
    assert_eq!(code, 1);
    assert!(stdout.is_empty(), "nothing reaches stdout: {stdout}");
    assert!(
        stderr.contains("only DSN and JSON formats are supported"),
        "`BoardLoader.java:33`'s message: {stderr}"
    );
}

#[test]
fn a_missing_dr_path_disables_rules_discovery() {
    if !parity::require_java_dir() {
        return;
    }
    let dir = scratch("missing-dr");
    let dsn = stage_dsn(&dir, &small_dsn(), "board.dsn");
    std::fs::write(
        dir.join("board.rules"),
        b"(rules PCB board\n  (autoroute_settings\n    (via_costs 99)\n  )\n)\n",
    )
    .unwrap();
    let manifest = dir.join("m.json");

    let (_, _, code) = run(&[
        "-de",
        &dsn.to_string_lossy(),
        "-do",
        &dir.join("a.ses").to_string_lossy(),
        "-mp",
        "1",
        &format!("--router.result_json={}", manifest.display()),
    ]);
    assert_eq!(code, 0);
    assert_eq!(
        settings_snapshot(&manifest)["scoring"]["via_costs"],
        serde_json::json!(99),
        "the adjacent board.rules must reach the answer when nothing else does"
    );

    let (_, _, code) = run(&[
        "-de",
        &dsn.to_string_lossy(),
        "-do",
        &dir.join("b.ses").to_string_lossy(),
        "-dr",
        &dir.join("nosuch.rules").to_string_lossy(),
        "-mp",
        "1",
        &format!("--router.result_json={}", manifest.display()),
    ]);
    assert_eq!(code, 0);
    assert_eq!(
        settings_snapshot(&manifest)["scoring"]["via_costs"],
        serde_json::json!(50),
        "quirk V: the missing -dr takes the branch and stops there"
    );
}

#[test]
fn no_donation_banner_on_stdout() {
    if !parity::require_java_dir() {
        return;
    }
    let dir = scratch("no-banner");
    let (stdout, _, code) = run(&[
        "-de",
        &small_dsn().to_string_lossy(),
        "-do",
        &dir.join("out.ses").to_string_lossy(),
        "-mp",
        "1",
    ]);
    assert_eq!(code, 0);
    assert!(stdout.is_empty(), "stdout must be empty, got {stdout:?}");
}

#[test]
fn max_passes_zero_is_unlimited() {
    if !parity::require_java_dir() {
        return;
    }
    let dir = scratch("max-passes-zero");
    let manifest = dir.join("m.json");
    let (_, _, code) = run(&[
        "-de",
        &small_dsn().to_string_lossy(),
        "-do",
        &dir.join("out.ses").to_string_lossy(),
        "-mp",
        "0",
        &format!("--router.result_json={}", manifest.display()),
    ]);
    assert_eq!(code, 0);
    assert_eq!(
        settings_snapshot(&manifest)["max_passes"],
        serde_json::json!(9999),
        "quirk #140: 0 -> Integer.MAX_VALUE (merge #1's validate) -> 9999 (merge #2's)"
    );
}

#[test]
fn the_rules_file_is_read_as_bytes_twice() {
    if !parity::require_java_dir() {
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
        "-de",
        &dsn.to_string_lossy(),
        "-do",
        &dir.join("out.ses").to_string_lossy(),
        "-mp",
        "1",
        &format!("--router.result_json={}", manifest.display()),
    ]);
    assert_eq!(code, 0);
    let scoring = settings_snapshot(&manifest);
    assert_eq!(
        scoring["scoring"]["via_costs"],
        serde_json::json!(99),
        "only `RoutingJobScheduler.java:173-184`'s second parse can write over DefaultSettings' 50"
    );
    assert_eq!(
        scoring["scoring"]["plane_via_costs"],
        serde_json::json!(7),
        "the same, for the second field of the same scope"
    );
}

#[test]
fn the_generic_override_is_set_on_native_and_dotted_on_legacy() {
    if !parity::require_java_dir() {
        return;
    }
    let dir = scratch("generic-override");
    let dsn = small_dsn();
    let dsn = dsn.to_string_lossy();

    let via_costs = |manifest: &Path| settings_snapshot(manifest)["scoring"]["via_costs"].clone();
    let fifty = serde_json::json!(50);
    let seventy_seven = serde_json::json!(77);

    let m1 = dir.join("1.json");
    let (_, stderr, code) = run(&[
        "route",
        &dsn,
        "-o",
        &dir.join("1.ses").to_string_lossy(),
        "--max-passes",
        "1",
        "--set",
        "router.scoring.via_costs=77",
        "--result-json",
        &m1.to_string_lossy(),
    ]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(
        via_costs(&m1),
        seventy_seven,
        "ruling BJ: `--set` is the native form's generic override and must reach priority 60"
    );

    let m2 = dir.join("2.json");
    let (_, stderr, code) = run(&[
        "route",
        &dsn,
        "-o",
        &dir.join("2.ses").to_string_lossy(),
        "--max-passes",
        "1",
        "--set=router.scoring.via_costs=77",
        "--result-json",
        &m2.to_string_lossy(),
    ]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(
        via_costs(&m2),
        seventy_seven,
        "`--set=<payload>` must split at the payload's own first `=`, not at the flag's"
    );

    let (_, stderr, code) = run(&[
        "route",
        &dsn,
        "-o",
        &dir.join("3.ses").to_string_lossy(),
        "--max-passes",
        "1",
        "--router.scoring.via_costs=77",
    ]);
    assert_eq!(
        code, 2,
        "the native form has NO arm for `--<section>.<field>=<value>`; stderr: {stderr}"
    );
    assert!(
        stderr.contains("unexpected argument"),
        "clap's own usage error is what row 3 pins: {stderr}"
    );

    let m4 = dir.join("4.json");
    let (_, stderr, code) = run(&[
        "-de",
        &dsn,
        "-do",
        &dir.join("4.ses").to_string_lossy(),
        "-mp",
        "1",
        "--set",
        "router.scoring.via_costs=77",
        &format!("--router.result_json={}", m4.display()),
    ]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(
        via_costs(&m4),
        fifty,
        "ruling AR: the legacy path is bug-for-bug and the jar ignores `--set` twice over"
    );
    assert!(
        stderr.contains("Unknown command line argument: --set"),
        "{stderr}"
    );
    assert!(
        stderr.contains("Unknown command line argument: router.scoring.via_costs=77"),
        "{stderr}"
    );

    let m5 = dir.join("5.json");
    let (_, stderr, code) = run(&[
        "-de",
        &dsn,
        "-do",
        &dir.join("5.ses").to_string_lossy(),
        "-mp",
        "1",
        "--router.scoring.via_costs=77",
        &format!("--router.result_json={}", m5.display()),
    ]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(
        via_costs(&m5),
        seventy_seven,
        "the legacy form's generic override is Java's own dotted spelling, and it is live"
    );
}

#[test]
fn a_settings_file_reaches_the_run() {
    if !parity::require_java_dir() {
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
    let fifty = serde_json::json!(50);
    let seventy_seven = serde_json::json!(77);

    let a = dir.join("a.json");
    let (_, stderr, code) = run(&[
        "-de",
        &dsn,
        "-do",
        &dir.join("a.ses").to_string_lossy(),
        "-mp",
        "1",
        &format!("--router.result_json={}", a.display()),
    ]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(
        via_costs(&a),
        fifty,
        "no settings file: DefaultSettings' 50"
    );

    let b = dir.join("b.json");
    let (_, stderr, code) = run(&[
        "route",
        &dsn,
        "-o",
        &dir.join("b.ses").to_string_lossy(),
        "--settings",
        &json.to_string_lossy(),
        "--max-passes",
        "1",
        "--result-json",
        &b.display().to_string(),
    ]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(
        via_costs(&b),
        seventy_seven,
        "`--settings` on the native form must reach `SettingsInputs::json_file`"
    );

    let c = dir.join("c.json");
    let (_, stderr, code) = run(&[
        "-de",
        &dsn,
        "-do",
        &dir.join("c.ses").to_string_lossy(),
        "-mp",
        "1",
        "--settings",
        &json.to_string_lossy(),
        &format!("--router.result_json={}", c.display()),
    ]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(
        via_costs(&c),
        fifty,
        "ruling BG: the legacy path is bug-for-bug, and the jar ignores --settings"
    );
    assert!(
        stderr.contains("Unknown command line argument: --settings"),
        "{stderr}"
    );
    assert!(
        stderr.contains(&format!(
            "Unknown command line argument: {}",
            json.to_string_lossy()
        )),
        "{stderr}"
    );

    for (name, argv) in [
        (
            "D-legacy",
            vec![
                "-de".to_string(),
                dsn.to_string(),
                "-do".to_string(),
                dir.join("d.ses").to_string_lossy().into_owned(),
                "-mp".to_string(),
                "1".to_string(),
                format!("--router.result_json={}", dir.join("d.json").display()),
            ],
        ),
        (
            "E-native",
            vec![
                "route".to_string(),
                dsn.to_string(),
                "-o".to_string(),
                dir.join("e.ses").to_string_lossy().into_owned(),
                "--max-passes".to_string(),
                "1".to_string(),
                "--result-json".to_string(),
                dir.join("e.json").to_string_lossy().into_owned(),
            ],
        ),
    ] {
        let output = std::process::Command::new(PORT)
            .current_dir(&cwd)
            .args(&argv)
            .stdin(std::process::Stdio::null())
            .output()
            .expect("the port runs");
        assert_eq!(
            output.status.code(),
            Some(0),
            "{name}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let manifest = dir.join(if name == "D-legacy" {
            "d.json"
        } else {
            "e.json"
        });
        assert_eq!(
            via_costs(&manifest),
            fifty,
            "{name}: ruling BG — the jar ignores a working-directory freerouting.json, \
             so the port must too"
        );
    }
}

#[test]
fn a_stage_timeout_is_not_a_job_timeout() {
    if !parity::require_java_dir() {
        return;
    }
    let dir = scratch("stage-timeout");
    let dsn = small_dsn();
    let dsn = dsn.to_string_lossy();

    let manifest = dir.join("stage.json");
    let (_, stderr, code) = run(&[
        "-de",
        &dsn,
        "-do",
        &dir.join("stage.ses").to_string_lossy(),
        "-mp",
        "1",
        "--router.optimizer.timeout=0:00:00",
        &format!("--router.result_json={}", manifest.display()),
    ]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(
        final_state(&manifest),
        "COMPLETED",
        "a stage timeout reaches the finish log's details string, never job.state"
    );

    for timeout in ["0:00:00", "0:00:01"] {
        let manifest = dir.join(format!("job-{}.json", timeout.replace(':', "")));
        let (_, stderr, code) = run(&[
            "-de",
            &dsn,
            "-do",
            &dir.join("job.ses").to_string_lossy(),
            "-mp",
            "1",
            &format!("--router.job_timeout={timeout}"),
            &format!("--router.result_json={}", manifest.display()),
        ]);
        assert_eq!(code, 0, "{stderr}");
        assert_eq!(
            final_state(&manifest),
            "COMPLETED",
            "--router.job_timeout={timeout} on a sub-second board: the jar answers COMPLETED too"
        );
    }
}

#[test]
fn an_invalid_input_writes_no_manifest_because_java_never_reaches_the_writer() {
    if !parity::require_java_dir() {
        return;
    }
    let dir = scratch("final-state-invalid");
    let input = dir.join("board.dsn");
    std::fs::write(&input, b"(session previous)\n").unwrap();
    let manifest = dir.join("m.json");
    let (_, stderr, code) = run(&[
        "-de",
        &input.to_string_lossy(),
        "-do",
        &dir.join("out.ses").to_string_lossy(),
        &format!("--router.result_json={}", manifest.display()),
    ]);
    assert_eq!(code, 1, "{stderr}");
    assert!(
        !manifest.exists(),
        "the jar never reaches Freerouting.java:161 on this path, so neither may the port"
    );
}

#[test]
fn the_via_net_number_fixture_routes_instead_of_hanging() {
    let dir = scratch("via-net-numbers");
    let fixture =
        parity::workspace_root().join("crates/fr-dsn/tests/data/p8t13-via-net-numbers.dsn");
    let ses = dir.join("out.ses");
    let (_, stderr, code) = run(&[
        "-de",
        &fixture.to_string_lossy(),
        "-do",
        &ses.to_string_lossy(),
    ]);
    assert_eq!(
        code, 0,
        "the port must route the jar's hang fixture: {stderr}"
    );

    let bytes = std::fs::read(&ses).expect("the run must write a .ses");
    assert_eq!(
        bytes.len(),
        1_843,
        "the routed SES for the fixed reader; it was 2 024 with the padded net array"
    );

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
fn drc_exits_0_when_the_rules_file_is_missing() {
    if !parity::require_java_dir() {
        return;
    }
    let dir = scratch("drc-missing-rules");
    let dsn = drc_dsn();

    let out = dir.join("rules.json");
    let (_, stderr, code) = run(&[
        "drc",
        &dsn.to_string_lossy(),
        "--rules",
        &dir.join("nosuch.rules").to_string_lossy(),
        "-o",
        &out.to_string_lossy(),
    ]);
    assert_eq!(
        code, 0,
        "quirk #271: a missing .rules file only warns\n{stderr}"
    );
    assert!(stderr.contains("RULES file for DRC not found:"), "{stderr}");
    assert!(out.is_file(), "the report is still written");

    let out = dir.join("session.json");
    let (_, stderr, code) = run(&[
        "drc",
        &dsn.to_string_lossy(),
        "--ses",
        &dir.join("nosuch.ses").to_string_lossy(),
        "-o",
        &out.to_string_lossy(),
    ]);
    assert_eq!(
        code, 0,
        "quirk #271: a missing session file only warns\n{stderr}"
    );
    assert!(
        stderr.contains("Session file for DRC not found:"),
        "{stderr}"
    );
    assert!(out.is_file(), "the report is still written");

    let (_, _, code) = run(&[
        "drc",
        &dsn.to_string_lossy(),
        "-o",
        &dir.join("clean.json").to_string_lossy(),
    ]);
    assert_eq!(code, 0);
    let violations = report(&dir.join("clean.json"))["violations"]
        .as_array()
        .expect("violations is an array")
        .len();
    assert_eq!(
        violations, 10,
        "the fixture's own violation count, and it does not reach the exit code (quirk #271)"
    );
}

#[test]
fn drc_exits_1_when_the_input_is_unreadable() {
    if !parity::require_java_dir() {
        return;
    }
    let dir = scratch("drc-unreadable-input");
    let out = dir.join("r.json");
    let (_, stderr, code) = run(&[
        "drc",
        &dir.join("nosuch.dsn").to_string_lossy(),
        "-o",
        &out.to_string_lossy(),
    ]);
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains("Couldn't load the input file"), "{stderr}");
    assert!(
        !stderr.contains("aborting."),
        "Freerouting.java:109 is initializeCli's, and initializeDrc has no counterpart:\n{stderr}"
    );
    assert!(!out.exists(), "no report is written");
}

#[test]
fn drc_exits_1_when_the_board_will_not_load() {
    if !parity::require_java_dir() {
        return;
    }
    let dir = scratch("drc-board-will-not-load");
    let input = dir.join("session.dsn");
    std::fs::write(&input, b"(session previous)\n").expect("the scratch file is writable");
    let out = dir.join("r.json");
    let (_, stderr, code) = run(&[
        "drc",
        &input.to_string_lossy(),
        "-o",
        &out.to_string_lossy(),
    ]);
    assert_eq!(code, 1, "{stderr}");
    assert!(
        stderr.contains("only DSN and JSON formats are supported, got SES"),
        "quirk #274: the loader refuses, not the argument\n{stderr}"
    );
    assert!(
        stderr.contains("Failed to load board for DRC check"),
        "{stderr}"
    );
    assert!(!out.exists(), "no report is written");
}

#[test]
fn drc_exits_1_when_the_report_cannot_be_written() {
    if !parity::require_java_dir() {
        return;
    }
    let dir = scratch("drc-unwritable-report");
    let out = dir.join("nodir").join("r.json");
    let (_, stderr, code) = run(&[
        "drc",
        &drc_dsn().to_string_lossy(),
        "-o",
        &out.to_string_lossy(),
    ]);
    assert_eq!(code, 1, "{stderr}");
    assert!(
        stderr.contains("Couldn't save the DRC report to"),
        "{stderr}"
    );
    assert!(!out.exists());
}

#[test]
fn the_session_is_imported_after_the_rules() {
    if !parity::require_java_dir() {
        return;
    }
    let dir = scratch("drc-load-order");
    let dsn = parity::fixture("Issue593-BBD_Mars-64.dsn");
    let ses = parity::fixture("Issue593-BBD_Mars-64.ses");

    fn one_rule(dir: &Path, name: &str, clearance: f64, pair: Option<&str>) -> PathBuf {
        let path = dir.join(name);
        let rule = match pair {
            Some(pair) => format!("(clearance {clearance} (type {pair}))"),
            None => format!("(clearance {clearance})"),
        };
        std::fs::write(
            &path,
            format!("(rules PCB Issue593-BBD_Mars-64\n  (rule\n    {rule}\n  )\n)\n"),
        )
        .expect("the scratch file is writable");
        path
    }

    fn build(dsn: &Path, rules: &Path, ses: &Path, rules_first: bool) -> (u64, usize) {
        let mut job = fr_core::RoutingJob::new(fr_core::SessionId::NIL);
        job.set_input(dsn).expect("the fixture reads");
        let loaded = fr_core::load_board_if_needed(&mut job).expect("the fixture loads");
        let mut board = loaded.board;
        let transform = loaded.transform;
        if rules_first {
            freerouting::commands::drc::load_rules_file(Some(rules), &job, &mut board, &transform);
            freerouting::commands::drc::load_session_file(Some(ses), &mut board, &transform);
        } else {
            freerouting::commands::drc::load_session_file(Some(ses), &mut board, &transform);
            freerouting::commands::drc::load_rules_file(Some(rules), &job, &mut board, &transform);
        }
        let hash = board.structural_hash();
        let violations = fr_drc::DesignRulesChecker::new(&mut board)
            .get_all_clearance_violations()
            .len();
        (hash, violations)
    }

    let typed = one_rule(&dir, "smd_via.rules", 400.0, Some("smd_via"));
    let (rules_first_hash, rules_first_violations) = build(&dsn, &typed, &ses, true);
    let (session_first_hash, session_first_violations) = build(&dsn, &typed, &ses, false);
    assert_ne!(
        (rules_first_hash, rules_first_violations),
        (session_first_hash, session_first_violations),
        "quirk #273: a `via`/`pin`/`smd`/`area`-typed clearance pair makes the order observable"
    );
    assert_eq!(
        (rules_first_violations, session_first_violations),
        (15, 0),
        "the measured counts; the HEAD jar's own run on these three files reports 15 \
         `holeClearance` entries, i.e. the rules-first board"
    );

    let class_blind = one_rule(&dir, "plain.rules", 400.0, None);
    assert_eq!(
        build(&dsn, &class_blind, &ses, true),
        build(&dsn, &class_blind, &ses, false),
        "a class-blind clearance is order-insensitive — it writes no `defaultItemClearanceClasses`"
    );

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
    assert_eq!(code, 0, "{stderr}");
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
        clearance, rules_first_violations,
        "the CLI must build the rules-first board (Freerouting.java:277-294 then :296-329), and \
         session-first would have answered {session_first_violations}"
    );

    let rules_at = stderr
        .find("Loading RULES file for DRC:")
        .unwrap_or_else(|| panic!("Freerouting.java:281 is missing:\n{stderr}"));
    let session_at = stderr
        .find("Loading SES file for DRC:")
        .unwrap_or_else(|| panic!("Freerouting.java:309 is missing:\n{stderr}"));
    assert!(rules_at < session_at, "quirk #273's order:\n{stderr}");
}

#[test]
fn the_quality_score_uses_a_dsn_only_merge() {
    if !parity::require_java_dir() {
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
        "the `.rules` tier is priority 40 and the router path has it"
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
    assert_eq!(code, 0, "{stderr}");
    let without_rules = dir.join("without-rules.json");
    let (_, stderr, code) = run(&[
        "drc",
        &dsn.to_string_lossy(),
        "-o",
        &without_rules.to_string_lossy(),
    ]);
    assert_eq!(code, 0, "{stderr}");

    let mut with = report(&with_rules);
    let mut without = report(&without_rules);
    assert_eq!(
        with["quality_score"], without["quality_score"],
        "quirk #272: `-dr` never reaches Freerouting.java:344-347's merge"
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
    if !parity::require_java_dir() {
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
    assert_eq!(code, 0, "{stderr}");

    let text = std::fs::read_to_string(&out).expect("the report is readable");
    assert!(
        text.contains("\"quality_score\": 902.078369140625"),
        "the score must be Double.toString of the widened float, verbatim:\n{text}"
    );

    let score = report(&out)["quality_score"]
        .as_f64()
        .expect("quality_score is a number");
    #[allow(clippy::cast_possible_truncation)]
    let narrowed = score as f32;
    assert_eq!(
        f64::from(narrowed),
        score,
        "a score that does not survive f64 -> f32 -> f64 is one no jar could have written"
    );
}

#[test]
fn every_committed_reference_score_is_recomputed() {
    if !parity::require_java_dir() {
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
            parity::java_dir().join(&dsn).to_string_lossy().into_owned(),
        ];
        if !rules.is_empty() {
            argv.push("--rules".to_string());
            argv.push(
                parity::java_dir()
                    .join(&rules)
                    .to_string_lossy()
                    .into_owned(),
            );
        }
        if !ses.is_empty() {
            argv.push("--ses".to_string());
            argv.push(parity::java_dir().join(&ses).to_string_lossy().into_owned());
        }
        argv.push("-o".to_string());
        argv.push(out.to_string_lossy().into_owned());
        let argv_refs: Vec<&str> = argv.iter().map(String::as_str).collect();
        let (_, stderr, code) = run(&argv_refs);
        assert_eq!(code, 0, "{stem}: {stderr}");

        let actual = report(&out)["quality_score"]
            .as_f64()
            .unwrap_or_else(|| panic!("{stem}: the report carries no quality_score"));
        assert_eq!(
            actual, expected,
            "{stem}: the computed score must equal the jar's recorded one, bit for bit"
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
    if !parity::require_java_dir() {
        return;
    }
    let (stdout, stderr, code) = run(&["drc", &drc_dsn().to_string_lossy()]);
    assert_eq!(code, 0, "{stderr}");
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
        stderr.contains("Loading DSN file for DRC:"),
        "the log stays on stderr (quirk #261)\n{stderr}"
    );
}

#[test]
fn info_writes_the_board_summary_to_stdout() {
    if !parity::require_java_dir() {
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
    if !parity::require_java_dir() {
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
}

#[test]
fn the_cli_passes_kicad_flavor_explicitly() {
    if !parity::require_java_dir() {
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
    assert_eq!(code, 0, "{stderr}");
    let kicad_text = std::fs::read_to_string(&kicad).expect("readable");
    for key in [
        "\"coordinate_units\"",
        "\"kicad_version\"",
        "\"freerouting_version\"",
        "\"unconnected_items\"",
        "\"schematic_parity\"",
        "\"quality_score\"",
        "\"type\": \"hole_clearance\"",
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
            "HEAD's spelling must not appear: {key}"
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
    assert_eq!(code, 0, "{stderr}");
    let head_text = std::fs::read_to_string(&head).expect("readable");
    assert!(head_text.contains("\"coordinateUnits\""));
    assert!(head_text.contains("\"qualityScore\""));
    assert!(!head_text.contains("\"quality_score\""));
    assert!(parity::parse_drc_json(&head_text).is_ok());
    assert!(
        parity::parse_drc_json(&kicad_text).is_err(),
        "the two flavors must be genuinely different documents"
    );
}

fn climb_one(stem: &parity::CliStem) -> Result<(), String> {
    let dir = scratch(&format!("ref-{}", stem.name));
    let argv = parity::cli_argv(&stem.name, &dir);
    let argv_refs: Vec<&str> = argv.iter().map(String::as_str).collect();
    let (stdout, stderr, code) = parity::run_port_binary(Path::new(PORT), &argv_refs);

    let expected_code: i32 =
        std::fs::read_to_string(parity::cli_reference(&stem.name, "route.exit"))
            .map_err(|e| format!("route.exit: {e}"))?
            .trim()
            .parse()
            .map_err(|e| format!("route.exit is not a number: {e}"))?;
    if code != expected_code {
        return Err(format!(
            "exit code {code} != the jar's {expected_code}\n--- port stderr ---\n{}",
            String::from_utf8_lossy(&stderr)
        ));
    }

    let expected_ses = std::fs::read_to_string(parity::cli_reference(&stem.name, "route.ses"))
        .map_err(|e| format!("route.ses: {e}"))?;
    let expected_ses = parity::normalize_ses_head_tokens(&expected_ses);
    let actual_ses = std::fs::read_to_string(dir.join("route.ses"))
        .map_err(|e| format!("the port wrote no route.ses: {e}"))?;
    if actual_ses != expected_ses {
        let at = actual_ses
            .bytes()
            .zip(expected_ses.bytes())
            .position(|(a, b)| a != b)
            .unwrap_or_else(|| actual_ses.len().min(expected_ses.len()));
        return Err(format!(
            "SES differs at byte {at} (port {} B, jar {} B)\n  port: {:?}\n  jar : {:?}",
            actual_ses.len(),
            expected_ses.len(),
            actual_ses.get(at.saturating_sub(40)..(at + 40).min(actual_ses.len())),
            expected_ses.get(at.saturating_sub(40)..(at + 40).min(expected_ses.len())),
        ));
    }

    let expected_log = std::fs::read(parity::cli_reference(&stem.name, "route.log"))
        .map_err(|e| format!("route.log: {e}"))?;
    let expected_log = parity::normalize_log(&expected_log, b"");
    let actual_log = parity::normalize_log(&stdout, &stderr);
    if actual_log != expected_log {
        return Err(format!(
            "normalized log differs\n--- port ---\n{actual_log}--- jar ---\n{expected_log}"
        ));
    }

    let manifest = dir.join("manifest.json");
    let mut manifest_argv = argv.clone();
    manifest_argv.push(format!("--router.result_json={}", manifest.display()));
    let manifest_refs: Vec<&str> = manifest_argv.iter().map(String::as_str).collect();
    let (_, manifest_stderr, manifest_code) =
        parity::run_port_binary(Path::new(PORT), &manifest_refs);
    if manifest_code != expected_code {
        return Err(format!(
            "exit code {manifest_code} != the jar's {expected_code} on the manifest run\n{}",
            String::from_utf8_lossy(&manifest_stderr)
        ));
    }
    let expected_manifest =
        std::fs::read_to_string(parity::cli_reference(&stem.name, "manifest.json"))
            .map_err(|e| format!("manifest.json: {e}"))?;
    let actual_manifest = std::fs::read_to_string(&manifest)
        .map_err(|e| format!("the port wrote no manifest: {e}"))?;
    let expected_manifest = parity::normalize_manifest(&expected_manifest);
    let actual_manifest = parity::normalize_manifest(&actual_manifest);
    if actual_manifest != expected_manifest {
        return Err(format!(
            "manifest differs\n--- port ---\n{}\n--- jar ---\n{}",
            actual_manifest.to_pretty(),
            expected_manifest.to_pretty()
        ));
    }
    Ok(())
}

fn climb(ci_only: bool) {
    if !parity::require_java_dir() {
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
                "SKIP: cli-{} has no reference — run scripts/gen-cli-reference.sh {}",
                stem.name, stem.name
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
fn the_ci_stems_match_the_jars_reference() {
    climb(true);
}

#[test]
#[cfg_attr(debug_assertions, ignore)]
fn the_slow_stems_match_the_jars_reference() {
    if std::env::var_os("FR_SLOW_PARITY").is_none() {
        eprintln!("SKIP: set FR_SLOW_PARITY=1 to run the slow CLI reference lane");
        return;
    }
    climb(false);
}

#[test]
fn the_cli_refuses_an_unreadable_router_budget_with_exit_2() {
    if !parity::require_java_dir() {
        return;
    }
    let dir = scratch("bad-router-budget");
    let out = dir.join("route.ses");
    let output = std::process::Command::new(PORT)
        .env("FR_ROUTER_BUDGET", "banana")
        .args([
            "-de",
            &parity::java_dir()
                .join("fixtures/empty_board.dsn")
                .display()
                .to_string(),
            "-do",
            &out.display().to_string(),
        ])
        .output()
        .expect("the freerouting binary runs");

    assert_eq!(
        output.status.code(),
        Some(2),
        "an unreadable FR_ROUTER_BUDGET is `ExitCode::UsageError`, the same 2 the \
         `std::process::exit(2)` this replaced produced.\n--- stderr ---\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("FR_ROUTER_BUDGET") && stderr.contains("banana"),
        "the refusal must name the variable and the value: {stderr}"
    );
    assert!(
        !out.exists(),
        "a refused run must not write a session — it would be a board routed with a budget the \
         operator explicitly did not ask for"
    );
}

#[test]
fn two_runs_of_every_ci_stem_are_byte_identical() {
    if !parity::require_java_dir() {
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
            argv.push(format!("--router.result_json={}", manifest.display()));
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
                 {} B). This is the failure #234 was about: something in the run depends on the \
                 machine rather than on the board.",
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
            "route.log",
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
