//! at DEBUG.

use std::process::Command;

fn run(args: &[&str]) -> (i32, String, String) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_freerouting"));
    command.arg("-ll").arg("debug");
    command.args(args);
    let output = command.output().expect("the binary runs");
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let rewritten = stderr
        .lines()
        .find_map(|line| line.split_once("rewritten command line: "))
        .map(|(_, rest)| rest.to_string())
        .unwrap_or_default();
    (output.status.code().expect("no signal"), rewritten, stderr)
}

fn native(args: &[&str]) -> String {
    format!(
        "{:?}",
        args.iter().map(|a| (*a).to_string()).collect::<Vec<_>>()
    )
}

const ROUTE_MISSING_INPUT_EXIT: i32 = 1;

#[test]
fn global_settings_command_line_test_matrix() {
    let rows: &[(&str, &[&str], &[&str])] = &[
        (
            "singleDsnFile",
            &["-de", "myboard.dsn", "-do", "out.ses"],
            &["route", "myboard.dsn", "-o", "out.ses"],
        ),
        (
            "dsnAndSesWithPlusSeparator",
            &["-de", "myboard.dsn+myboard.ses", "-do", "out.ses"],
            &[
                "route",
                "myboard.dsn",
                "--ses",
                "myboard.ses",
                "-o",
                "out.ses",
            ],
        ),
        (
            "dsnAndRulesWithPlusSeparator",
            &["-de", "myboard.dsn+myboard.rules", "-do", "out.ses"],
            &[
                "route",
                "myboard.dsn",
                "--rules",
                "myboard.rules",
                "-o",
                "out.ses",
            ],
        ),
        (
            "allThreeFilesWithPlusSeparator",
            &[
                "-de",
                "myboard.dsn+myboard.ses+myboard.rules",
                "-do",
                "out.ses",
            ],
            &[
                "route",
                "myboard.dsn",
                "--ses",
                "myboard.ses",
                "--rules",
                "myboard.rules",
                "-o",
                "out.ses",
            ],
        ),
        (
            "filesInDifferentOrder",
            &[
                "-de",
                "myboard.rules+myboard.dsn+myboard.ses",
                "-do",
                "out.ses",
            ],
            &[
                "route",
                "myboard.dsn",
                "--ses",
                "myboard.ses",
                "--rules",
                "myboard.rules",
                "-o",
                "out.ses",
            ],
        ),
        (
            "spaceSeparatedFiles",
            &[
                "-de",
                "myboard.dsn",
                "myboard.ses",
                "myboard.rules",
                "-do",
                "out.ses",
            ],
            &[
                "route",
                "myboard.dsn",
                "--ses",
                "myboard.ses",
                "--rules",
                "myboard.rules",
                "-o",
                "out.ses",
            ],
        ),
        (
            "filenameWithSpaces",
            &["-de", "sonde xilinx.dsn", "-do", "out.ses"],
            &["route", "sonde xilinx.dsn", "-o", "out.ses"],
        ),
        (
            "mixedSeparators",
            &[
                "-de",
                "myboard.dsn+myboard.ses",
                "myboard.rules",
                "-do",
                "out.ses",
            ],
            &[
                "route",
                "myboard.dsn",
                "--ses",
                "myboard.ses",
                "--rules",
                "myboard.rules",
                "-o",
                "out.ses",
            ],
        ),
        (
            "filesWithPaths",
            &[
                "-de",
                "/path/to/myboard.dsn+/path/to/myboard.ses",
                "-do",
                "out.ses",
            ],
            &[
                "route",
                "/path/to/myboard.dsn",
                "--ses",
                "/path/to/myboard.ses",
                "-o",
                "out.ses",
            ],
        ),
        (
            "caseInsensitiveExtensions",
            &[
                "-de",
                "myboard.DSN+myboard.SES+myboard.RULES",
                "-do",
                "out.ses",
            ],
            &[
                "route",
                "myboard.DSN",
                "--ses",
                "myboard.SES",
                "--rules",
                "myboard.RULES",
                "-o",
                "out.ses",
            ],
        ),
        (
            "multipleDsnFilesUsesLast",
            &["-de", "board1.dsn+board2.dsn", "-do", "out.ses"],
            &["route", "board2.dsn", "-o", "out.ses"],
        ),
        (
            "withOtherArguments",
            &[
                "-de",
                "myboard.dsn+myboard.ses",
                "-do",
                "output.ses",
                "-mp",
                "10",
            ],
            &[
                "route",
                "myboard.dsn",
                "--ses",
                "myboard.ses",
                "-o",
                "output.ses",
            ],
        ),
    ];

    for (name, argv, expected) in rows {
        let (code, rewritten, stderr) = run(argv);
        assert_eq!(
            rewritten,
            native(expected),
            "{name}: argv {argv:?}\nstderr:\n{stderr}"
        );
        assert_eq!(code, ROUTE_MISSING_INPUT_EXIT, "{name}: stderr:\n{stderr}");
    }
}

#[test]
fn the_three_cases_that_assert_a_null_input_are_javas_cli_refusal() {
    for (name, argv) in [
        ("onlySesFile", &["-de", "myboard.ses"][..]),
        ("onlyRulesFile", &["-de", "myboard.rules"][..]),
        ("emptyArgument", &["-de"][..]),
    ] {
        let (code, rewritten, stderr) = run(argv);
        assert_eq!(code, 1, "{name}: stderr:\n{stderr}");
        assert_eq!(rewritten, "[]", "{name}: stderr:\n{stderr}");
        assert!(
            stderr.contains(
                "Both an input file and an output file must be specified with command line \
                 arguments if you are running in CLI mode."
            ),
            "{name}: stderr:\n{stderr}"
        );
    }
}

#[test]
fn the_session_and_rules_slots_survive_without_a_dsn() {
    let (_, rewritten, stderr) = run(&["-de", "myboard.ses", "myboard.dsn", "-do", "out.ses"]);
    assert_eq!(
        rewritten,
        native(&[
            "route",
            "myboard.dsn",
            "--ses",
            "myboard.ses",
            "-o",
            "out.ses"
        ]),
        "stderr:\n{stderr}"
    );
    let (_, rewritten, stderr) = run(&["-de", "myboard.rules", "myboard.dsn", "-do", "out.ses"]);
    assert_eq!(
        rewritten,
        native(&[
            "route",
            "myboard.dsn",
            "--rules",
            "myboard.rules",
            "-o",
            "out.ses"
        ]),
        "stderr:\n{stderr}"
    );
}

#[test]
fn p8t5_table_the_slots_and_the_two_matching_rules() {
    let (_, rewritten, _) = run(&["-de", "a.dsn", "b.rules", "-do", "o.ses"]);
    assert_eq!(
        rewritten,
        native(&["route", "a.dsn", "--rules", "b.rules", "-o", "o.ses"])
    );

    let (_, rewritten, _) = run(&["-de", "a.dsn+b.ses", "-do", "o.ses"]);
    assert_eq!(
        rewritten,
        native(&["route", "a.dsn", "--ses", "b.ses", "-o", "o.ses"])
    );

    let (_, rewritten, _) = run(&["-de", "board.json", "-do", "o.ses"]);
    assert_eq!(rewritten, native(&["route", "board.json", "-o", "o.ses"]));

    let (_, rewritten, _) = run(&["-de", "a.dsn", "prev.json", "-do", "o.ses"]);
    assert_eq!(
        rewritten,
        native(&["route", "a.dsn", "--ses", "prev.json", "-o", "o.ses"])
    );

    let (code, rewritten, stderr) = run(&["-de", "a.txt", "-do", "o.ses"]);
    assert_eq!(rewritten, "[]");
    assert_eq!(code, 1);
    assert!(
        stderr.contains(
            "Unknown file type in -de argument: a.txt. Expected .dsn, .json, .ses, or .rules"
        ),
        "stderr:\n{stderr}"
    );

    let (_, rewritten, _) = run(&["-decoy", "a.dsn", "-do", "o.ses"]);
    assert_eq!(rewritten, native(&["route", "a.dsn", "-o", "o.ses"]));
}

#[test]
fn p8t5_table_mpx_5_must_not_set_max_passes() {
    for argv in [
        &["-de", "a.dsn", "-do", "o.ses", "-mpx", "5"][..],
        &["-de", "a.dsn", "-do", "o.ses", "-mp", "5"][..],
    ] {
        let (_, rewritten, _) = run(argv);
        assert_eq!(rewritten, native(&["route", "a.dsn", "-o", "o.ses"]));
        assert!(!rewritten.contains("max-passes"), "argv {argv:?}");
    }
}

#[test]
fn p8t5_table_the_silent_no_ops_and_the_warn_and_continue() {
    let (code, rewritten, stderr) = run(&["-de", "a.dsn", "-drc"]);
    assert_eq!(rewritten, "[]");
    assert_eq!(code, 1);
    assert!(
        stderr.contains("Both an input file and an output file"),
        "{stderr}"
    );

    let (_, rewritten, _) = run(&["-de", "a.dsn", "-drc", "r.json"]);
    assert_eq!(rewritten, native(&["drc", "a.dsn", "-o", "r.json"]));

    let (_, rewritten, stderr) = run(&["-de", "a.dsn", "-do", "o.ses", "-mp"]);
    assert_eq!(rewritten, native(&["route", "a.dsn", "-o", "o.ses"]));
    assert!(
        !stderr.contains("Unknown command line argument"),
        "{stderr}"
    );

    let (_, rewritten, stderr) = run(&["-de", "a.dsn", "-do", "o.ses", "-mp", "-5"]);
    assert_eq!(rewritten, native(&["route", "a.dsn", "-o", "o.ses"]));
    assert!(
        stderr.contains("Unknown command line argument: -5"),
        "stderr:\n{stderr}"
    );

    let (_, rewritten, stderr) = run(&["-de", "a.dsn", "-do", "o.ses", "-oit", "-5"]);
    assert_eq!(rewritten, native(&["route", "a.dsn", "-o", "o.ses"]));
    assert!(
        stderr.contains("Unknown command line argument: -5"),
        "{stderr}"
    );
    let (_, rewritten, stderr) = run(&["-de", "a.dsn", "-do", "o.ses", "-oit", "0"]);
    assert_eq!(rewritten, native(&["route", "a.dsn", "-o", "o.ses"]));
    assert!(
        !stderr.contains("Unknown command line argument"),
        "{stderr}"
    );

    let (_, rewritten, stderr) = run(&["-de", "a.dsn", "-do", "o.ses", "--router.enabled="]);
    assert_eq!(rewritten, native(&["route", "a.dsn", "-o", "o.ses"]));
    assert!(
        !stderr.contains("Unknown command line argument"),
        "{stderr}"
    );

    let (code, rewritten, stderr) = run(&["-de", "a.dsn", "-do", "o.ses", "-zz"]);
    assert_eq!(rewritten, native(&["route", "a.dsn", "-o", "o.ses"]));
    assert_eq!(code, ROUTE_MISSING_INPUT_EXIT);
    assert!(
        stderr.contains("Unknown command line argument: -zz"),
        "{stderr}"
    );

    let (_, rewritten, stderr) = run(&["-di", "dir", "-de", "a.dsn", "-do", "o.ses"]);
    assert_eq!(rewritten, native(&["route", "a.dsn", "-o", "o.ses"]));
    assert!(stderr.contains("headless"), "stderr:\n{stderr}");
    assert!(
        !stderr.contains("Unknown command line argument"),
        "{stderr}"
    );

    for flag in ["-dl", "-dlx", "-da", "-dax"] {
        let (_, rewritten, stderr) = run(&["-de", "a.dsn", "-do", "o.ses", flag]);
        assert_eq!(
            rewritten,
            native(&["route", "a.dsn", "-o", "o.ses"]),
            "{flag}"
        );
        assert!(
            !stderr.contains("Unknown command line argument"),
            "{flag}: {stderr}"
        );
    }
}

#[test]
fn p8t5_table_ll_twice_takes_the_last() {
    let output = Command::new(env!("CARGO_BIN_EXE_freerouting"))
        .args([
            "-ll", "debug", "-ll", "error", "-de", "a.dsn", "-do", "o.ses",
        ])
        .output()
        .expect("the binary runs");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("rewritten command line"),
        "the last -ll must win: stderr:\n{stderr}"
    );
    // …and with `error` first, DEBUG survives.
    let output = Command::new(env!("CARGO_BIN_EXE_freerouting"))
        .args([
            "-ll", "error", "-ll", "debug", "-de", "a.dsn", "-do", "o.ses",
        ])
        .output()
        .expect("the binary runs");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("rewritten command line"),
        "stderr:\n{stderr}"
    );
}

#[test]
fn p8t5_table_no_arguments_at_all() {
    let output = Command::new(env!("CARGO_BIN_EXE_freerouting"))
        .output()
        .expect("the binary runs");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "stderr:\n{stderr}");
    assert!(
        stderr.contains("Both an input file and an output file"),
        "stderr:\n{stderr}"
    );
}

#[test]
fn help_exits_zero_on_both_forms() {
    for argv in [
        &["--help"][..],
        &["-h"][..],
        &["-help"][..],
        &["route", "--help"][..],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_freerouting"))
            .args(argv)
            .output()
            .expect("the binary runs");
        assert_eq!(output.status.code(), Some(0), "argv {argv:?}");
        assert!(!output.stdout.is_empty(), "argv {argv:?}");
    }
}

#[test]
fn version_is_javas_unknown_argument_on_the_legacy_form_and_clap_s_behind_a_subcommand() {
    for flag in ["--version", "-V"] {
        let output = Command::new(env!("CARGO_BIN_EXE_freerouting"))
            .arg(flag)
            .output()
            .expect("the binary runs");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{flag}: stderr:\n{stderr}");
        assert!(
            stderr.contains(&format!("Unknown command line argument: {flag}")),
            "{flag}: stderr:\n{stderr}"
        );
        assert!(
            stderr.contains("Both an input file and an output file"),
            "{flag}: stderr:\n{stderr}"
        );
        assert!(
            output.stdout.is_empty(),
            "{flag}: nothing may reach stdout on a refusal"
        );
    }

    let output = Command::new(env!("CARGO_BIN_EXE_freerouting"))
        .args(["route", "--version"])
        .output()
        .expect("the binary runs");
    assert_eq!(output.status.code(), Some(0));
    assert!(!output.stdout.is_empty());
}

#[test]
fn a_usage_error_is_exit_two_and_only_on_the_native_form() {
    let output = Command::new(env!("CARGO_BIN_EXE_freerouting"))
        .args(["route", "--no-such-flag"])
        .output()
        .expect("the binary runs");
    assert_eq!(output.status.code(), Some(2));

    let output = Command::new(env!("CARGO_BIN_EXE_freerouting"))
        .args(["--no-such-flag"])
        .output()
        .expect("the binary runs");
    assert_eq!(output.status.code(), Some(1));
}

#[test]
fn the_mcp_stdio_long_form_reaches_the_mcp_subcommand() {
    use std::io::Write;
    let mut child = Command::new(env!("CARGO_BIN_EXE_freerouting"))
        .arg("--mcp_server.stdio=true")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("the binary runs");
    child
        .stdin
        .as_mut()
        .expect("piped")
        .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\n")
        .expect("write");
    let output = child.wait_with_output().expect("wait");
    assert_eq!(output.status.code(), Some(0));
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("\"id\":1"),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
}
