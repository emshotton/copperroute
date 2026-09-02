//! The legacy command line, **through the binary**.
//!
//! Two tables:
//!
//! 1. **Plan ruling 12's port of `GlobalSettingsCommandLineTest.java`** (`:25-171`), case for case
//!    and name for name. Java asserts `GlobalSettings.{initialInputFile, designSessionFilename,
//!    initialRulesFile, initialOutputFile}` directly; the binary has no such accessor, so each
//!    case is observed through the native command line the rewrite produces — which *is* those
//!    four slots, in a fixed order (`route <input> [--ses S] [--rules R] -o <output>`). Where
//!    Java's argv names no output file, `-do out.ses` is appended so the slots become observable,
//!    and the row says so; the three cases that assert a **null** input (`onlySesFile`,
//!    `onlyRulesFile`, `emptyArgument`) are additionally run with their exact Java argv, where
//!    Java's own answer is `initializeCli`'s refusal and `System.exit(1)`.
//!
//!    The six `--key=value` cases of that class (`:177-259`) are **not** ported: they assert
//!    `apiServerSettings.{endpoints, corsOrigins, rateLimit}` and `mcpServerSettings.rateLimit`,
//!    none of which this port models (spec §2 — no REST API, and the MCP transport here is stdio
//!    with no listener). `crates/fr-settings/tests/cli_source.rs` already carries the `-de` matrix
//!    at unit level; this file is the same matrix one layer up.
//!
//! 2. **The `p8t5` table**, as literals: the eighteen argv shapes the task brief names, each with
//!    the rule it pins. The differential driver runs **86** shapes against the jar
//!    (`scripts/differential/matrix/p8t5-argv.tsv`); these are the ones a reader must be able to
//!    check without a JVM.
//!
//! Every row runs the real binary with `-ll debug` prepended (which the parse consumes like any
//! other valued flag) and reads back the `rewritten command line: [...]` line `crate::run` emits
//! at DEBUG.

use std::process::Command;

/// Runs the binary and answers `(exit code, the rewritten native argv, stderr)`.
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

/// The `{argv:?}` rendering of a native command line, for comparing against what the binary logs.
fn native(args: &[&str]) -> String {
    format!(
        "{:?}",
        args.iter().map(|a| (*a).to_string()).collect::<Vec<_>>()
    )
}

/// Every runnable row ends in the route stub, which is Task 6's to replace.
const ROUTE_STUB_EXIT: i32 = 3;

// =================================================================================================
// 1. GlobalSettingsCommandLineTest.java:25-171, case for case
// =================================================================================================

/// The Java case name, its argv, and the native line the rewrite must produce.
///
/// `+ -do out.ses` marks a row where the output flag is appended to Java's argv so the three input
/// slots are observable; Java's own case asserts them on the `GlobalSettings` object instead.
#[test]
fn global_settings_command_line_test_matrix() {
    let rows: &[(&str, &[&str], &[&str])] = &[
        // :26-33 singleDsnFile (+ -do out.ses)
        (
            "singleDsnFile",
            &["-de", "myboard.dsn", "-do", "out.ses"],
            &["route", "myboard.dsn", "-o", "out.ses"],
        ),
        // :36-43 dsnAndSesWithPlusSeparator
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
        // :46-53 dsnAndRulesWithPlusSeparator
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
        // :56-63 allThreeFilesWithPlusSeparator
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
        // :66-73 filesInDifferentOrder — the slot, not the position, decides
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
        // :76-83 spaceSeparatedFiles — `-de` consumes the whole run, not one argument
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
        // :86-93 filenameWithSpaces
        (
            "filenameWithSpaces",
            &["-de", "sonde xilinx.dsn", "-do", "out.ses"],
            &["route", "sonde xilinx.dsn", "-o", "out.ses"],
        ),
        // :96-103 mixedSeparators
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
        // :106-112 filesWithPaths
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
        // :115-122 caseInsensitiveExtensions — classified lower-cased, stored verbatim
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
        // :125-131 multipleDsnFilesUsesLast
        (
            "multipleDsnFilesUsesLast",
            &["-de", "board1.dsn+board2.dsn", "-do", "out.ses"],
            &["route", "board2.dsn", "-o", "out.ses"],
        ),
        // :163-171 withOtherArguments — `-mp 10` reaches the settings ladder, never this argv
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
        assert_eq!(code, ROUTE_STUB_EXIT, "{name}: stderr:\n{stderr}");
    }
}

/// `:134-140 onlySesFile`, `:143-150 onlyRulesFile` and `:153-160 emptyArgument` all assert a
/// **null** `initialInputFile`. Run with their exact Java argv, that is `initializeCli`'s refusal
/// (`Freerouting.java:80-86`) and `System.exit(1)`.
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

/// The same three, made runnable, to pin the slots Java asserts.
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

// =================================================================================================
// 2. The `p8t5` table, as literals
// =================================================================================================

#[test]
fn p8t5_table_the_slots_and_the_two_matching_rules() {
    // `-de a.dsn b.rules` — the run, not one argument (GlobalSettings.java:568-570).
    let (_, rewritten, _) = run(&["-de", "a.dsn", "b.rules", "-do", "o.ses"]);
    assert_eq!(
        rewritten,
        native(&["route", "a.dsn", "--rules", "b.rules", "-o", "o.ses"])
    );

    // `-de "a.dsn+b.ses"` — legacy `+` concatenation (:573-581).
    let (_, rewritten, _) = run(&["-de", "a.dsn+b.ses", "-do", "o.ses"]);
    assert_eq!(
        rewritten,
        native(&["route", "a.dsn", "--ses", "b.ses", "-o", "o.ses"])
    );

    // Ruling 14: `-de board.json` with no `.dsn` seen is the **design input** (:610-612).
    let (_, rewritten, _) = run(&["-de", "board.json", "-do", "o.ses"]);
    assert_eq!(rewritten, native(&["route", "board.json", "-o", "o.ses"]));

    // Ruling 14: `-de a.dsn prev.json` puts the `.json` in the **session** slot (:613-620).
    let (_, rewritten, _) = run(&["-de", "a.dsn", "prev.json", "-do", "o.ses"]);
    assert_eq!(
        rewritten,
        native(&["route", "a.dsn", "--ses", "prev.json", "-o", "o.ses"])
    );

    // `-de a.txt` — an unknown extension is warned about and **dropped**, never guessed into the
    // design slot (:638-644).
    let (code, rewritten, stderr) = run(&["-de", "a.txt", "-do", "o.ses"]);
    assert_eq!(rewritten, "[]");
    assert_eq!(code, 1);
    assert!(
        stderr.contains(
            "Unknown file type in -de argument: a.txt. Expected .dsn, .json, .ses, or .rules"
        ),
        "stderr:\n{stderr}"
    );

    // `-decoy a.dsn` — `startsWith`, so the design input IS set (the whole table but `-l`).
    let (_, rewritten, _) = run(&["-decoy", "a.dsn", "-do", "o.ses"]);
    assert_eq!(rewritten, native(&["route", "a.dsn", "-o", "o.ses"]));
}

#[test]
fn p8t5_table_mpx_5_must_not_set_max_passes() {
    // Scan ruling R19: `-mpx` is `-mp` to the prefix parser (which writes only the dead bridge)
    // and nothing at all to `CliSettings` (which reaches the router). Observable here as: the
    // rewritten argv carries no `--max-passes`, and never can — `-mp 5` does not either.
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
    // A bare `-drc` is NOT DRC mode (quirk #263): `:660-663`'s two booleans are dead and
    // `main:1462` needs `drcReportFile`.
    let (code, rewritten, stderr) = run(&["-de", "a.dsn", "-drc"]);
    assert_eq!(rewritten, "[]");
    assert_eq!(code, 1);
    assert!(
        stderr.contains("Both an input file and an output file"),
        "{stderr}"
    );

    // `-drc r.json` IS DRC mode, and needs no `-do`.
    let (_, rewritten, _) = run(&["-de", "a.dsn", "-drc", "r.json"]);
    assert_eq!(rewritten, native(&["drc", "a.dsn", "-o", "r.json"]));

    // `-mp` with no value at all: a silent no-op, nothing warned.
    let (_, rewritten, stderr) = run(&["-de", "a.dsn", "-do", "o.ses", "-mp"]);
    assert_eq!(rewritten, native(&["route", "a.dsn", "-o", "o.ses"]));
    assert!(
        !stderr.contains("Unknown command line argument"),
        "{stderr}"
    );

    // `-mp -5` is inexpressible: `-5` is never consumed and warns on its own (:833).
    let (_, rewritten, stderr) = run(&["-de", "a.dsn", "-do", "o.ses", "-mp", "-5"]);
    assert_eq!(rewritten, native(&["route", "a.dsn", "-o", "o.ses"]));
    assert!(
        stderr.contains("Unknown command line argument: -5"),
        "stderr:\n{stderr}"
    );

    // `-oit -5` — the same rule, which is why `-oit`'s `<= 0` clamp is unreachable from argv
    // (quirk #135). `-oit 0` IS reachable and is consumed.
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

    // `--router.enabled=` is a `--name=value` the settings ladder reads off the RAW argv; it
    // never reaches the rewritten one, and it never warns.
    let (_, rewritten, stderr) = run(&["-de", "a.dsn", "-do", "o.ses", "--router.enabled="]);
    assert_eq!(rewritten, native(&["route", "a.dsn", "-o", "o.ses"]));
    assert!(
        !stderr.contains("Unknown command line argument"),
        "{stderr}"
    );

    // An unknown flag warns and continues (:833); the run still happens.
    let (code, rewritten, stderr) = run(&["-de", "a.dsn", "-do", "o.ses", "-zz"]);
    assert_eq!(rewritten, native(&["route", "a.dsn", "-o", "o.ses"]));
    assert_eq!(code, ROUTE_STUB_EXIT);
    assert!(
        stderr.contains("Unknown command line argument: -zz"),
        "{stderr}"
    );

    // `-di` consumes its value like Java and warns that the directory is ignored.
    let (_, rewritten, stderr) = run(&["-di", "dir", "-de", "a.dsn", "-do", "o.ses"]);
    assert_eq!(rewritten, native(&["route", "a.dsn", "-o", "o.ses"]));
    assert!(stderr.contains("headless"), "stderr:\n{stderr}");
    assert!(
        !stderr.contains("Unknown command line argument"),
        "{stderr}"
    );

    // `-dl` and `-dlx` are the same switch (`startsWith`, :799) and neither takes a value.
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
    // Plan ruling 10: `GlobalSettings`' rule (last wins), not `main:1060-1065`'s (first wins) —
    // quirk #262. With `error` last, the DEBUG rewrite line is suppressed entirely.
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
    // Java: no GUI in a headless JVM, so `initializeCli` runs and refuses — exit 1, not a usage
    // screen and not exit 2.
    assert_eq!(output.status.code(), Some(1), "stderr:\n{stderr}");
    assert!(
        stderr.contains("Both an input file and an output file"),
        "stderr:\n{stderr}"
    );
}

// =================================================================================================
// The exit ladder (ruling AR)
// =================================================================================================

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

/// Controller ruling BF: `--version` is native-form only, because the jar **refuses** it.
///
/// Measured on the HEAD jar (`java -jar … --version`): `WARN Unknown command line argument:
/// --version`, then `ERROR Both an input file and an output file must be specified …`, **exit 1**.
/// Same for `-V`. `p8t5`'s `version-long`/`version-short` rows pin the parse half.
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

    // The native form keeps it, which is what `propagate_version` in `cli.rs` is for.
    let output = Command::new(env!("CARGO_BIN_EXE_freerouting"))
        .args(["route", "--version"])
        .output()
        .expect("the binary runs");
    assert_eq!(output.status.code(), Some(0));
    assert!(!output.stdout.is_empty());
}

#[test]
fn a_usage_error_is_exit_two_and_only_on_the_native_form() {
    // clap's own code, which ruling AR reserves for the native subcommand form.
    let output = Command::new(env!("CARGO_BIN_EXE_freerouting"))
        .args(["route", "--no-such-flag"])
        .output()
        .expect("the binary runs");
    assert_eq!(output.status.code(), Some(2));

    // The same nonsense on the legacy form is a warn-and-continue, then Java's exit 1.
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
