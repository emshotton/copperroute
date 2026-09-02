//! `freerouting route` end to end, through the **binary** (Plan 8 Task 6).
//!
//! Two halves:
//!
//! * **the behaviour tests** — eight named cases from the task brief, each pinning one quirk or
//!   one ruling of `Freerouting.initializeCli`. They run the port only; the jar's answer for each
//!   is either a committed reference or a measurement recorded in `docs/java-quirks.md`;
//! * **the reference lanes** — every stem of `tests/reference/cli-fixtures.txt` run against the
//!   committed `tests/reference/cli-<stem>/` outputs the **bare HEAD jar** wrote
//!   (`scripts/gen-cli-reference.sh`). Four rungs per stem: byte-identical SES, equal exit code,
//!   equal [`parity::normalize_log`], and — a second run, with `--router.result_json=<f>` —
//!   equal [`parity::normalize_manifest`]. The `p8t1`/`p8t2 e2e` drivers run the *live* jar
//!   against the *live* port on the same argv; these tests are what make the same comparison
//!   available on a machine with no JDK, and what a regression trips first.
//!
//! # The one SES normalisation, and why it is not a tolerance
//!
//! [`parity::normalize_ses_head_tokens`] rewrites four `(parser …)` keyword literals on the
//! **reference** side. HEAD camelCased them (`(hostCad `, `(hostVersion `, `(stringQuote `,
//! `(writeResolution `) while leaving its own lexer recognising only the snake_case tokens, so it
//! writes Specctra it cannot read back; Plan 3 ruling 1 therefore pins the port's writer to the
//! 2.3.0 spelling. The set is closed, enumerated and documented as quirk **#92**, and it is the
//! same normalisation `crates/fr-router/tests/batch_parity.rs` applies to `batch.ses`. Everything
//! else is compared byte for byte — measured: on `router-rpi-splitter` those two lines are the
//! **only** difference between the jar's 3 654 bytes and the port's 3 656.
//!
//! # The lanes
//!
//! Plan 7's split, carried over unchanged: the four `ci` stems of `cli-fixtures.txt` run under a
//! plain `cargo test`, and the seven `slow` ones need `FR_SLOW_PARITY=1` (and are `ignore`d in a
//! debug build, where the router is minutes rather than seconds per board).

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

/// The binary under test — Cargo builds it for this package's integration tests and hands over
/// the path, so nothing here has to guess a profile directory.
const PORT: &str = env!("CARGO_BIN_EXE_freerouting");

/// A per-test scratch directory, removed and recreated so a rerun cannot see a stale file. Not
/// `std::env::temp_dir()` alone: two tests running in parallel must not share a `route.ses`.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("fr-cli-e2e").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir
}

/// Copies a corpus DSN into `dir` under `name`, so a test can put a `.rules` file beside it or
/// let the run delete things next to it.
fn stage_dsn(dir: &Path, source: &Path, name: &str) -> PathBuf {
    let target = dir.join(name);
    std::fs::copy(source, &target)
        .unwrap_or_else(|e| panic!("cannot stage {}: {e}", source.display()));
    target
}

/// The smallest corpus board that actually routes — `router-rpi-splitter`'s.
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

/// `RoutingResultManifest.finalState` (`:111`) — `job.state.name()`, as the manifest carries it.
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

/// The `settings_snapshot` of the manifest a run wrote, as a `serde_json::Value`.
///
/// This is how a test observes a **resolved setting through the binary**: nothing else the CLI
/// produces reports one. `--router.result_json=<f>` is Java's own flag for it
/// (`Freerouting.java:225-244`).
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

// =================================================================================================
// The eight behaviour tests
// =================================================================================================

/// **Quirk #265** (`Freerouting.java:116-121`): the desired output file is deleted **before**
/// anything is routed, so a run that then fails or is killed has destroyed the previous result
/// and written nothing in its place.
///
/// Asserted by failing *after* the delete: the input is a `.ses`, which `BoardLoader.java:31-37`
/// refuses, so the run exits 1 having deleted the pre-existing output and written no replacement.
#[test]
fn an_existing_output_file_is_deleted_before_routing() {
    if !parity::require_java_dir() {
        return;
    }
    let dir = scratch("delete-before-routing");
    // The input is named `.dsn` but **contains a session**: `RoutingJob::set_input` sniffs the
    // bytes first (`RoutingJob.java:431`), so `job.input.format` is `SES` and
    // `BoardLoader.java:31-37` refuses it — the run cannot reach the writer. The name has to end
    // `.dsn`, because the `-de` classifier routes a `.ses` argument to `designSessionFilename`
    // instead (`GlobalSettings.java:622-628`), which would leave no input file at all.
    let input = dir.join("board.dsn");
    std::fs::write(&input, b"(session previous)\n").unwrap();
    let output = dir.join("out.ses");
    std::fs::write(&output, b"PREVIOUS RESULT").unwrap();

    let (_, _, code) = run(&[
        "-de",
        &input.to_string_lossy(),
        "-do",
        &output.to_string_lossy(),
    ]);

    assert_eq!(code, 1, "a SES input is `BoardLoader.java:33`'s refusal");
    assert!(
        !output.exists(),
        "the previous result must be gone: `:118`'s delete runs before the load"
    );
}

/// **Quirk #268** (label L), half one (`Freerouting.java:123`, `RoutingJob.java:377-397`):
/// `tryToSetOutputFile`'s return value is discarded, so `-do out.txt` is *rejected* as an output
/// format, `job.output` keeps the `<input>.ses` `setInputFromFile:441` derived — and
/// `writeCliOutputIfAvailable` then writes the SES bytes to `out.txt` anyway, because `:206`
/// writes to `globalSettings.initialOutputFile` and not to `job.output`.
///
/// **Measured on the HEAD jar**, not inferred:
/// `java -jar <jar> -de Issue143-rpi_splitter.dsn -do out.txt -mp 1` exits **0** and leaves a
/// **2 626-byte** `out.txt` beginning `(session "Issue143-r`. The port's is 2 628 bytes and
/// byte-identical after quirk #92's two keyword literals — checked with
/// `diff <(sed 's/(host_cad /(hostCad /;s/(host_version /(hostVersion /' port) jar`.
#[test]
fn do_out_txt_still_receives_ses_bytes() {
    if !parity::require_java_dir() {
        return;
    }
    let dir = scratch("do-out-txt");
    let output = dir.join("out.txt");
    let (_, _, code) = run(&[
        "-de",
        &small_dsn().to_string_lossy(),
        "-do",
        &output.to_string_lossy(),
        "-mp",
        "1",
    ]);
    assert_eq!(code, 0);
    let bytes = std::fs::read(&output).expect("out.txt was written");
    assert!(
        bytes.starts_with(b"(session"),
        "out.txt holds the SES: {:?}",
        String::from_utf8_lossy(&bytes[..bytes.len().min(40)])
    );
    // And nothing was written beside the input, even though `job.output` names `<input>.ses`.
    assert!(
        !parity::fixture("Issue143-rpi_splitter.ses").exists(),
        "the derived <input>.ses must not be written — `:206` uses the CLI's path"
    );
}

/// **Quirk #268**, half two: `-do out.dsn` is *accepted* by `tryToSetOutputFile:384-388`, so
/// `job.output.format` becomes `DSN` — and `setJobOutput:274-292` serialises only
/// `KICAD_SESSION_JSON` and `SES`, so `output.getData()` stays the empty array
/// `BoardFileDetails.java:53` initialised it to. `writeCliOutputIfAvailable` then writes **zero
/// bytes**, `Files.size > 0` answers false, and the run exits 1 with an empty file left behind.
///
/// **Measured on the HEAD jar:** the same command with `-do out.dsn` exits **1** and leaves a
/// **0-byte** `out.dsn`.
#[test]
fn do_out_dsn_writes_zero_bytes_and_exits_1() {
    if !parity::require_java_dir() {
        return;
    }
    let dir = scratch("do-out-dsn");
    let output = dir.join("out.dsn");
    let (_, _, code) = run(&[
        "-de",
        &small_dsn().to_string_lossy(),
        "-do",
        &output.to_string_lossy(),
        "-mp",
        "1",
    ]);
    assert_eq!(code, 1, "`computeCliExitCode:222` — nothing was written");
    let meta = std::fs::metadata(&output).expect("the empty file is left behind");
    assert_eq!(meta.len(), 0, "`Files.write` wrote the empty array");
}

/// **Plan ruling 7** (`Freerouting.java:189-194`, quirk #244, quirk label S): `RoutingJobState.INVALID` is
/// missing from `isCliTerminalState`, so a job the scheduler rejects — an input that is neither
/// DSN nor KiCad JSON (`RoutingJobScheduler.java:251-254`) — leaves the jar spinning in
/// `:151-158`'s `while (!isCliTerminalState(...)) Thread.sleep(500)` **for ever**, at 0 % CPU,
/// with no output and no message. The port totalises the predicate and exits 1.
///
/// The test's real assertion is that it **terminates**; the exit code is the second half.
#[test]
fn de_a_ses_exits_1_instead_of_hanging() {
    if !parity::require_java_dir() {
        return;
    }
    let dir = scratch("de-a-ses");
    // Session bytes under a `.dsn` name — see
    // [`an_existing_output_file_is_deleted_before_routing`] for why the extension has to be
    // `.dsn` and the *content* is what makes the format `SES`.
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

/// **Quirk #269** (label V, `RoutingJobScheduler.java:118-131`): the scheduler's `else if` chain takes
/// the `globalSettings.initialRulesFile != null` branch whenever `-dr` was given, and the inner
/// `rf.exists()` then only decides whether the bytes are read — it does **not** fall through to
/// the adjacent `<design>.rules` probe. So a typo'd `-dr` silently disables a rules file the user
/// would otherwise have got.
///
/// Observed through the manifest: the adjacent file sets `scoring.via_costs = 99`, which
/// `DefaultSettings.java` puts at 50.
///
/// **Both halves measured on the HEAD jar**, with the same two runs this test makes:
/// `-de board.dsn -do a.ses -mp 1 --router.result_json=…` reports
/// `via_costs = 99, plane_via_costs = 7`, and adding `-dr <nonexistent>.rules` reports
/// `via_costs = 50, plane_via_costs = 5` — the defaults. The port answers the same pair to both.
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

    // Control: with no `-dr`, the adjacent file is discovered (`:132-152`).
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

    // The quirk: a `-dr` naming a file that does not exist disables that discovery.
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

/// **Quirk #266** (label H) (`Freerouting.java:164-183`): the jar prints a six-line
/// box-drawing donation banner to **stdout** when the output was written, the persisted
/// `statistics.jobsCompleted >= 5` and `userProfileSettings.userEmail` is empty. It is
/// un-suppressible — no flag reaches the `if` — and it would corrupt any stdout-JSON mode, so the
/// port does not print it.
///
/// The assertion is stronger than "no banner": the port writes **nothing at all** to stdout on a
/// successful run, which is what makes `freerouting route … | jq` safe here and not on the jar.
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

/// **Quirk #140** (`RouterSettings.java:932-941`): `validate()` maps `maxPasses == 0` to
/// `Integer.MAX_VALUE`, and the headless path calls `validate()` **twice** — merge #1's
/// (`SettingsMerger.java:189`) and merge #2's — so the second call sees `Integer.MAX_VALUE`,
/// finds it outside `[0, 9999]`, and clamps it to **9999**. A single merge would answer
/// `Integer.MAX_VALUE`.
///
/// `-mp 0` is therefore "unlimited" only in the sense that 9999 is: it is the maximum the
/// two-merge ladder can express, and the manifest reports it.
///
/// **Measured on the HEAD jar:** `-mp 0 --router.result_json=…` reports
/// `settings_snapshot.max_passes = 9999`.
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

/// **Quirk #142** (`RulesReader.java:112` against `:198`): the scheduler's `.rules` file is parsed
/// **twice**, with two different layer structures, and both results reach the answer — once at
/// priority 40 inside merge #2 (`RoutingJobScheduler.java:154-160`) and once *after* merge #2 as
/// a plain `applyNewValuesFrom` onto the finished object (`:173-184`).
///
/// # Why this is asserted through `via_costs` rather than through a read counter
///
/// The brief drafted "a read counter". A counter is not reachable through the binary, and an
/// in-process one would count `std::fs::read` calls rather than *parses* — the port reads the
/// bytes once and parses them twice, which is what
/// [`fr_settings::SettingsInputs::scheduler_rules`] taking a byte slice is for. So the assertion
/// is the stronger, observable one: **only the second parse can deliver this value.**
///
/// `DefaultSettings.java` writes `scoring.viaCosts = 50`, so it is non-null when merge #2's own
/// `0..60` chain runs, and that chain reaches merge #1's result only through
/// `RouterSettings::fill_absent_from` — it cannot overwrite 50. The adjacent `<design>.rules`
/// feeds **only** merge #2 and the post-merge re-apply (`Freerouting.java:129-144` registers the
/// `-dr` file and nothing else at merge #1). So `via_costs == 99` in the answer is a witness that
/// `:173-184` ran, i.e. that the file was read as bytes and parsed a second time.
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

/// **The priority-10 `freerouting.json` tier, through the binary** (the review's B1, as amended by
/// **controller ruling BG**).
///
/// `--settings <file>` is `JsonFileSettings(Path)`
/// (`settings/sources/JsonFileSettings.java:36-39`) at priority **10**. The *tier* is Java's, on
/// the prototype merger at `Freerouting.java:1410`; the *flag* is port-only, and scan ruling R7
/// scoped it to the **native** subcommand form.
///
/// # What the jar actually does — measured, and the reason for every row below
///
/// | jar input | `scoring.via_costs` in its manifest |
/// |---|---|
/// | `--settings s.json` on its (only) command line | **50** — two `Unknown command line argument` warnings, file ignored |
/// | started **in** a directory holding `freerouting.json` | **50** — ignored |
/// | `-Duser.home` at a home whose *user-data* `freerouting.json` sets `via_costs 77` | **77** — applied at priority 10 |
/// | the same, with no such file (control) | 50 |
///
/// So the jar reads exactly one location — `GlobalSettings.getUserDataPath()
/// .resolve("freerouting.json")`, `~/Library/Application Support/freerouting/freerouting.json` on
/// macOS (`AppPaths.resolveConfigDirectory:39-42`) — which is `static` mutable state spec §2 does
/// not port. It never reads the working directory, and it never honours a `--settings` it does
/// not know.
///
/// # What the port therefore does, on both forms
///
/// | run | form | `--settings` | cwd holds `freerouting.json` | `via_costs` |
/// |---|---|---|---|---|
/// | **A** | legacy | no | no | 50 |
/// | **B** | **native** | **yes** | no | **77** — the one way in |
/// | **C** | legacy | yes | no | **50** — ruling AR: the jar warns and ignores, so the port must |
/// | **D** | legacy | no | **yes** | **50** — ruling BG: the jar ignores a cwd file |
/// | **E** | native | no | **yes** | **50** — same rule on both forms |
///
/// Round 1 of the review left the port applying the file in runs C and D and recorded that as
/// accepted; ruling BG ruled it out. Run C also asserts the **warnings** are still Java's —
/// `p8t1`'s `settings-on-legacy` row is what compares them against the jar live.
#[test]
fn a_settings_file_reaches_the_run() {
    if !parity::require_java_dir() {
        return;
    }
    let dir = scratch("settings-file");
    let json = dir.join("s.json");
    std::fs::write(&json, r#"{"router": {"scoring": {"via_costs": 77}}}"#).unwrap();
    // A separate directory whose *own* `freerouting.json` is what runs D and E are started in.
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

    // Run A — legacy, no flag, no cwd file.
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

    // Run B — the **native** form with `--settings`, the one way into the tier.
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

    // Run C — the same flag on the **legacy** form. Ruling BG: warn like the jar, apply nothing.
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
    // ...and it says what the jar says while ignoring it (`GlobalSettings.java:833`, twice — the
    // flag is not a value-consuming arm, so its argument warns on its own).
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

    // Runs D and E — a `freerouting.json` in the **working directory**, on both forms. The jar
    // reads the OS user-data path and only that, so both must be 50.
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

/// **A per-stage timeout is not a job timeout** (the review's S1).
///
/// `RoutingJobState.TIMED_OUT` has exactly one writer in Java: the monitor thread
/// (`RoutingJobSchedulerActionThread.java:84`), which tests `job.timeoutAt` — the instant
/// `:41-51` derives from `routerSettings.jobTimeoutString`. The finish ladder at `:175-184` then
/// leaves that state alone, because neither its `RUNNING` nor its `STOPPING` arm matches.
///
/// A **stage** timeout never reaches the state at all. `isFanoutTimedOut()` and
/// `getOptimizer().isTimedOut()` are read at `:170-172` and spent on the finish log's details
/// string (`:175-177`), so the job finishes `COMPLETED` and its manifest says so.
///
/// # Measured on the HEAD jar
///
/// `-de Issue649-kicad_ecc83-pp_input_board_v1.dsn -do … -mp 8
/// --router.optimizer.timeout=0:00:00` (`BatchOptimizer.java:153-159` parses the string through
/// `TextManager.parseTimespanString`, so it must be a timespan the grammar accepts — `1ms` is
/// not, and silently leaves `deadlineMs` null):
///
/// ```text
/// INFO  Optimizer stage timed out before starting pass #1
/// INFO  Optimization stage completed with timeout: …
/// INFO  Job '…' finished with state: COMPLETED (optimizer stage timed out) …
/// manifest: "final_state": "COMPLETED", "exit_code": 0
/// ```
///
/// The port answered `"TIMED_OUT"` until this test existed, because it read
/// `fr_router::pipeline::PipelineResult::timed_out` — which deliberately folds the job deadline,
/// the fanout stage's per-pin budget **and** the optimizer's own deadline together, since that is
/// what a *router* caller wants to know. `commands::route` now reads the job deadline alone.
///
/// # What this test cannot reach, and says so
///
/// The `TimedOut` arm itself is not reachable through the CLI on a corpus board: every stem
/// finishes in under a second, and both programs give the job deadline a grace period before the
/// state is written (Java's monitor sleeps 1 s before its first check and then waits
/// `GRACE_PERIOD`; the port's `Deadline::is_timed_out_at` uses the same 30 s offset — Task 0's,
/// pinned by `crates/fr-core/tests/cancel.rs`). Measured: `--router.job_timeout=0:00:00` and
/// `=0:00:01` both answer `COMPLETED` **on both programs**, which is the second assertion below —
/// the port must not report a timeout merely because a deadline string was set.
#[test]
fn a_stage_timeout_is_not_a_job_timeout() {
    if !parity::require_java_dir() {
        return;
    }
    let dir = scratch("stage-timeout");
    let dsn = small_dsn();
    let dsn = dsn.to_string_lossy();

    // The optimizer's own deadline, already elapsed.
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

    // The job deadline, on a board that finishes long before either program's grace period.
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

/// Where the manifest **stops** being written, and why that is Java's answer too.
///
/// `Freerouting.java:161` calls `writeCliResultManifestIfRequested` unconditionally — but only
/// *after* the wait loop at `:151-158` has seen a terminal state. Two of the CLI's exits never
/// get there:
///
/// * **step 3's** `return false` at `:112` (the input could not be read) is before `:161`
///   entirely, so Java writes no manifest;
/// * **`INVALID`** — an input that is neither DSN nor KiCad JSON
///   (`RoutingJobScheduler.java:253`) — is omitted from `isCliTerminalState` (`:189-194`), so the
///   jar spins in `:151-158` for ever and writes no manifest either. Quirk #244; plan ruling 7
///   totalises the *hang* into exit 1, and this test pins where that totalisation stops: it does
///   **not** invent a manifest the jar never writes.
///
/// The port reaches the same answer through the same object: it bails before
/// `job.router_settings` is assigned, so `routerSettings.resultJsonPath` is still null and
/// `writeCliResultManifestIfRequested`'s own `:227-231` guard declines. The one asymmetry is the
/// port-only native `--result-json`, which is not a settings tier and would still be honoured;
/// no jar command line can reach it.
///
/// The manifest's `final_state` **is** pinned end to end, on every path that writes one: by
/// `p8t2 e2e` against the jar's own manifest on all eleven stems (`COMPLETED`), and by
/// [`a_stage_timeout_is_not_a_job_timeout`] for the state ladder's one live decision.
#[test]
fn an_invalid_input_writes_no_manifest_because_java_never_reaches_the_writer() {
    if !parity::require_java_dir() {
        return;
    }
    let dir = scratch("final-state-invalid");
    // Session bytes under a `.dsn` name — `BoardLoader.java:31-37` refuses them, which is
    // `RoutingJobScheduler.java:253`'s `INVALID`.
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

// =================================================================================================
// The reference lanes
// =================================================================================================

/// One stem against its committed `tests/reference/cli-<stem>/` outputs. `Ok(())` or the first
/// failing rung, named.
fn climb_one(stem: &parity::CliStem) -> Result<(), String> {
    let dir = scratch(&format!("ref-{}", stem.name));
    let argv = parity::cli_argv(&stem.name, &dir);
    let argv_refs: Vec<&str> = argv.iter().map(String::as_str).collect();
    let (stdout, stderr, code) = parity::run_port_binary(Path::new(PORT), &argv_refs);

    // Rung 2 first, because a wrong exit code explains a missing SES.
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

    // Rung 1: the SES bytes, with quirk #92's four keyword literals rewritten on the reference
    // side and nothing else touched.
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

    // Rung 3: the log projection.
    let expected_log = std::fs::read(parity::cli_reference(&stem.name, "route.log"))
        .map_err(|e| format!("route.log: {e}"))?;
    let expected_log = parity::normalize_log(&expected_log, b"");
    let actual_log = parity::normalize_log(&stdout, &stderr);
    if actual_log != expected_log {
        return Err(format!(
            "normalized log differs\n--- port ---\n{actual_log}--- jar ---\n{expected_log}"
        ));
    }

    // Rung 4 — `p8t2`'s: the same argv again with `--router.result_json=<f>`, compared field for
    // field against the manifest the jar's own second run wrote. It is a *fourth* rung here and a
    // separate driver there, because the reference carries both files and re-running the port is
    // cheap while re-running the jar is not.
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

/// Every stem in `lanes`, reported together so one failure does not hide the rest.
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

/// The four `ci` stems of `tests/reference/cli-fixtures.txt`.
#[test]
fn the_ci_stems_match_the_jars_reference() {
    climb(true);
}

/// Every stem, `ci` and `slow`. Ignored in a debug build (the router is minutes per board there)
/// and gated on `FR_SLOW_PARITY=1` otherwise — Plan 7's split, carried over.
#[test]
#[cfg_attr(debug_assertions, ignore)]
fn the_slow_stems_match_the_jars_reference() {
    if std::env::var_os("FR_SLOW_PARITY").is_none() {
        eprintln!("SKIP: set FR_SLOW_PARITY=1 to run the slow CLI reference lane");
        return;
    }
    climb(false);
}

/// The provenance guard: every stem of the fixture table has a reference directory, and every
/// reference directory has a stem. A stem added to the table without a reference would otherwise
/// be silently skipped by [`climb`].
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

/// The eight batch stems' CLI references must be byte-identical to Plan 7's `batch.ses`, which
/// `gen-batch-reference.sh --verify-driver` independently proved is the bare jar's output for the
/// same argv. Two references generated by two scripts, months and a task apart, agreeing is worth
/// more than either alone — and it is the signal a live-budget trip would break (see
/// `scripts/gen-cli-reference.sh`'s header).
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
