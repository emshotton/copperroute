//! `freerouting route` end to end, through the **binary** (Plan 8 Task 6).
//!
//! Two halves:
//!
//! * **the behaviour tests** — ten named cases, each pinning one quirk or one ruling of
//!   `Freerouting.initializeCli`. They run the port only; the jar's answer for each is either a
//!   committed reference or a measurement recorded in `docs/java-quirks.md`. Eight came from Task
//!   6's brief; Plan 8 Task 10 added the two the `.json` paths need — quirk #289's `-do out.json`
//!   (whose expected bytes are the jar's own, pasted in below) and quirk #290's non-ASCII
//!   session file;
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

const QUIRK_T_JAR_JSON: &str = r#"{
  "designName": "Issue143-rpi_splitter",
  "unit": "MIL",
  "resolution": 254.0,
  "layers": [
    {
      "index": 0,
      "name": "1#Top",
      "type": "signal"
    },
    {
      "index": 1,
      "name": "16#Bottom",
      "type": "signal"
    }
  ],
  "netClasses": [
    {
      "name": "default",
      "clearance": 120.0,
      "traceWidth": 160.0,
      "viaDiameter": 297.79527559055117,
      "viaDrill": 148.89763779527559,
      "netNames": [
        "D+",
        "D-",
        "N$5",
        "VBUS",
        "VCC"
      ]
    }
  ],
  "nets": [
    {
      "id": 1,
      "name": "D+",
      "className": "default",
      "containsPlane": false
    },
    {
      "id": 2,
      "name": "D-",
      "className": "default",
      "containsPlane": false
    },
    {
      "id": 3,
      "name": "N$5",
      "className": "default",
      "containsPlane": false
    },
    {
      "id": 4,
      "name": "VBUS",
      "className": "default",
      "containsPlane": false
    },
    {
      "id": 5,
      "name": "VCC",
      "className": "default",
      "containsPlane": false
    }
  ],
  "clearanceRules": [],
  "components": [],
  "outline": {
    "corners": [
      {
        "x": 0.0,
        "y": -0.0
      },
      {
        "x": 8370.07874015748,
        "y": -0.0
      },
      {
        "x": 8370.07874015748,
        "y": -16496.062992125986
      },
      {
        "x": 0.0,
        "y": -16496.062992125986
      }
    ],
    "clearance": 120.0
  },
  "traces": [],
  "vias": [],
  "conductionAreas": []
}"#;

// =================================================================================================
// The behaviour tests
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

/// **Quirk #289** (label T), the `-do out.json` output path
/// (`RoutingJobSchedulerActionThread.java:100`, `:168`, `:259-295`).
///
/// `setJobOutput` is registered as a **board-updated listener** at `:100` and called once more
/// after `pipeline.run()` at `:168`. On the KiCad-session-JSON path only the **first** of those
/// calls ever writes: `output.setData` re-sniffs the bytes (`BoardFileDetails.java:113` ->
/// `RoutingJob.getFileFormat:155-164`) and a document starting `{` re-detects as
/// `KICAD_DESIGN_JSON`, after which neither `:275`'s `== KICAD_SESSION_JSON` nor `:282`'s
/// `== SES` matches. The SES path escapes it because `(ses` re-detects as `SES`, so *its* last
/// write wins and that last write is `:168`'s, on the final board.
///
/// The first board-updated event fires from `BatchFanout.fanoutPass:203-217`, before the first
/// pin is processed, and `job.board` is still the object `BoardLoader` produced
/// (`AutorouteBatchLoop.java:552` reassigns it only after every pass). **So the jar's `out.json`
/// is the board as loaded, before any routing.**
///
/// **MEASURED at the pinned HEAD jar**, JDK 25, `-Djava.awt.headless=true -Duser.language=en
/// -Duser.country=US`, on `Issue143-rpi_splitter.dsn` with the argv below:
///
/// * `-mp 1`, `-mp 2` and `-mp 8` all produce the **same 1 540 bytes** (`md5 a20cafbe…`), and so
///   does a run with the router disabled;
/// * the same argv with `-do out.ses` produces 16 `(wire ` and 9 `(via ` scopes, so the board did
///   change during the run;
/// * `-de Issue649-kicad_ecc83-pp_input_board_v1.json -do out.json -mp 3 …` writes `"traces": []`
///   where the SES from the identical argv carries 9 wires;
/// * `-de Issue733-kicad_complex_hierarchy_output_session.json -do out.json -mp 2 …` writes the
///   input's **172** pre-existing traces back out — so it is the initial board, not an empty one.
///
/// The task brief's hypothesis, that the file keeps whatever the *last* mid-run event produced,
/// is refuted by the first bullet: `-mp 1` and `-mp 8` would then differ, and the fanout stage's
/// nine vias would appear.
///
/// The port **reproduces** it (it does not totalize it): `commands/route.rs`'s step 12b takes the
/// `KiCadJsonWriter.write` snapshot before `RoutingPipeline::run` and `set_job_output` writes
/// that. The literal below is the jar's own 1 540 bytes, pasted in.
#[test]
fn do_out_json_writes_the_pre_routing_board() {
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
    assert_eq!(
        code, 0,
        "`-do out.json` is accepted by tryToSetOutputFile:384-388"
    );
    let written = std::fs::read_to_string(&json).expect("out.json was written");
    assert_eq!(
        written, QUIRK_T_JAR_JSON,
        "byte for byte, the jar's own output"
    );

    // The board really did change during the run: the SES from the identical argv carries the
    // routed wiring the JSON does not.
    let (_, _, code) = run_to(&ses);
    assert_eq!(code, 0);
    let session = std::fs::read_to_string(&ses).expect("out.ses was written");
    assert_eq!(session.matches("(wire").count(), 16);
    assert_eq!(session.matches("(via ").count(), 9);
    assert!(
        written.contains("\"traces\": []"),
        "and the JSON has none of it"
    );
    assert!(written.contains("\"vias\": []"));
}

/// **Quirk #290** (label U), end to end: the `.json` session arm's charset.
///
/// `Freerouting.java:304` and `RoutingJobScheduler.java:201-202` both open the session with
/// `new java.io.FileReader(sessionFile)` — `Charset.defaultCharset()` — where every other JSON
/// path in the tree names UTF-8. The port decodes UTF-8 unconditionally.
///
/// **MEASURED at the pinned HEAD jar**, on the board and session this test writes
/// (`-de board.json session.json -do out.ses -mp 1`; the second `.json` becomes
/// `designSessionFilename` at `GlobalSettings.java:609-621`):
///
/// * **default charset** — the JVM reports `file.encoding = UTF-8` (JEP 400 made that the default
///   from JDK 18 regardless of locale), and the jar's SES carries
///   `(net "GND_é中" (via …) (wire …))`. The port's SES is byte-identical to it after quirk
///   #92's two `(parser …)` rewrites — **the divergence is unobservable here**;
/// * **`-Dfile.encoding=ISO-8859-1`** — the same run loads the session "successfully" and the
///   whole `(net "GND_é中" …)` scope is **gone** from the SES: the mis-decoded name matches no
///   net, `Nets.get` answers `null`, `netNumbers` is empty and the imported wire and via are
///   netless. Twelve lines of routed wiring silently dropped.
///
/// So the port's UTF-8 is Java's answer on every JVM the pinned jar supports (its class files are
/// version 69, i.e. JDK 25), and the divergence is reachable only by forcing a legacy charset.
/// This test pins the port's half; the register row carries the jar transcript.
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
    // The whole scope the ISO-8859-1 run loses.
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
// `freerouting drc` — the nine behaviour tests (Plan 8 Task 7)
//
// `Freerouting.initializeDrc` (`Freerouting.java:246-374`). Every jar answer asserted below was
// **measured on the pinned HEAD jar** and is re-measured on every run by `p8t3 e2e`'s five
// refusal rows, which drive the same five argv shapes through both programs; these tests are what
// makes the same facts available on a machine with no JDK.
// =================================================================================================

/// The DRC fixture with the largest score in the committed set (`902.078369140625`) and a small
/// board — `tests/reference/drc-dev-board`'s.
fn drc_dsn() -> PathBuf {
    parity::fixture("Issue575-drc_dev-board_4_hole_clearance_violations.dsn")
}

/// The DRC report a run wrote, parsed. **KiCad spelling** by default (ruling W), so the reader is
/// `serde_json::Value` rather than `parity::DrcReportDoc` (which is the HEAD-flavor projection).
fn report(path: &Path) -> serde_json::Value {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("the report is not JSON: {e}"))
}

/// A `.rules` file carrying **only** an `(autoroute_settings …)` block, so it changes the router's
/// settings and leaves the board alone.
///
/// `via_costs` is the field that makes it useful: it is a [`ScoringSettings`] member that
/// `BoardStatistics.calculateScore` reads (`BoardStatistics.java:611-613`,
/// `vias.totalCount * viaCosts`), and `RulesFileSettings` carries it at priority **40** — the one
/// tier `Freerouting.java:344-347`'s sub-merge does not have. `DefaultSettings.java` puts it at
/// 50, so a 999 that arrives is unmistakable.
fn autoroute_settings_rules(dir: &Path, via_costs: i32) -> PathBuf {
    let path = dir.join("scoring.rules");
    std::fs::write(
        &path,
        format!("(rules PCB scoring\n  (autoroute_settings\n    (via_costs {via_costs})\n  )\n)\n"),
    )
    .expect("the scratch file is writable");
    path
}

/// **Quirk #271** (label B, `Freerouting.java:289`): a `-dr` naming a file that does not exist is
/// a `FRLogger.warn` and **nothing else** — `initializeDrc` runs to completion, writes the report
/// and returns `true`, so the process exits **0**.
///
/// **Measured on the HEAD jar**: `-de <dsn> -dr <nonexistent>.rules -drc r.json` exits **0** and
/// writes a 20 460-byte report, with `WARN RULES file for DRC not found: …` in the log. `p8t3
/// e2e`'s `missing-rules` row re-measures it.
///
/// The sibling half — a missing **session** file (`:324`) — is the same shape and is asserted
/// here too, because the two arms are what quirk #271 is *about*: three of the four things that
/// can go wrong on this path do not move the exit code.
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

    // `:324` — the same, for the session slot.
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

    // And the violations themselves never reach the code: this fixture has ten of them.
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

/// **`Freerouting.java:266-267`**, the first of the three `System.exit(1)` sites: `drcJob
/// .setInput` threw, so the run stops before the loader.
///
/// **Measured on the HEAD jar**: exit **1**, `ERROR Couldn't load the input file '…'`, no report.
/// Note what is *absent* — `initializeCli`'s second message (`:109`, "Couldn't read the input
/// file '…', aborting.") has no counterpart here, because `System.exit` ends the process inside
/// the `catch`.
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

/// **`Freerouting.java:272-273`** and **quirk #274** (label S): `-de prev.ses -drc r.json` is
/// accepted by the argument parser, sniffed as `SES` by `RoutingJob.setInput`, and refused inside
/// `BoardLoader` (`BoardLoader.java:31-37`) — not at the argument.
///
/// The three steps are three different classifiers and it matters which one refuses: the `-de`
/// slot rule goes by **extension** (`GlobalSettings.java:564-648`), `setInput` goes by **bytes**
/// (`RoutingJob.java:431`), and only the loader has an opinion about what it can read. So a file
/// **named** `.dsn` that **contains** a session gets all the way to the loader.
///
/// **Measured on the HEAD jar**: exit **1**, and *two* errors — `Cannot load board: only DSN and
/// JSON formats are supported, got SES` (`BoardLoader.java:33`, Task 3's text, reproduced
/// verbatim) then `Failed to load board for DRC check` (`:272`). Both are asserted, because the
/// second alone would not prove which classifier stopped the run.
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

/// **`Freerouting.java:365-366`**, the third and last `System.exit(1)` site: the report itself
/// could not be written.
///
/// The whole check ran — the board loaded, `generateReport` produced a document and the quality
/// score was computed — and the run still exits 1, because `Files.write` threw. That is the only
/// failure on this path that happens *after* the work.
///
/// **Measured on the HEAD jar**: `-drc <dir>/nodir/r.json` with `nodir` absent exits **1** with
/// `ERROR Couldn't save the DRC report to '…'`.
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

/// **Quirk #273** (label D): the session is imported **after** the `.rules` file
/// (`Freerouting.java:277-294` then `:296-329`), so the session's wires and vias are **created** —
/// and then checked — against the clearance classes the rules file installed. Swapping the two
/// changes the violation list.
///
/// # The mechanism, and why it takes a *typed* clearance pair to see it
///
/// `RulesReader`'s `(rule …)` arm reaches `Structure.setClearanceRule`, which calls
/// `appendClearanceClass` for either half of a clearance-class **pair** that is not already in the
/// matrix (`Structure.java:756`, `:765`) — and `appendClearanceClass` (`:826-840`) writes the
/// default net class's `defaultItemClearanceClasses` for the four names `via`, `pin`, `smd`,
/// `area`. `SesReader.processViaScope` reads that field at **via-creation** time
/// (`SesReader.java:395-400`), so a via made before the write takes the old class and one made
/// after takes the new one.
///
/// A clearance with **no** `(type …)` never gets there: `setClearanceRule` returns at `:684-707`
/// after `setDefaultValue`, having touched only the matrix. That is why this test needs two rules
/// files and asserts opposite things about them — the difference is the headline, and the
/// class-blind file is the control that says *which* shape of rule causes it.
///
/// # What is asserted
///
/// 1. the two orders differ on the `(type smd_via)` file — **15** clearance violations rules-first,
///    **0** session-first, with different `Board::structural_hash`es;
/// 2. the two orders agree on the class-blind control — 463 either way;
/// 3. the **binary** takes Java's order: its report on the same three files carries the
///    rules-first count, not the session-first one;
/// 4. and it says so in the log, `:281` before `:309`.
///
/// The numbers are the jar's, not just the port's: `java -jar <jar> -de <dsn> <ses> -dr <smd_via>
/// -drc r.json` on the HEAD jar reports **15 `holeClearance`** entries (measured), which is the
/// rules-first board. `p8t3 e2e`'s `rules-and-session` row is the standing comparison.
#[test]
fn the_session_is_imported_after_the_rules() {
    if !parity::require_java_dir() {
        return;
    }
    let dir = scratch("drc-load-order");
    let dsn = parity::fixture("Issue593-BBD_Mars-64.dsn");
    let ses = parity::fixture("Issue593-BBD_Mars-64.ses");

    /// A one-rule `.rules` file. `pair` is the `(type …)` argument, or `None` for the class-blind
    /// form that `Structure.setClearanceRule:684-707` answers with an early return.
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

    /// One board through the runner's own two loaders in the given order: its structural hash and
    /// how many clearance violations the checker then finds.
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

    // 1. The difference. `smd_via` splits into `smd` and `via`; `via` is one of
    //    `appendClearanceClass`'s four magic names, so the pair moves
    //    `defaultItemClearanceClasses[VIA]` and every via `SesReader` makes afterwards takes the
    //    new class.
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

    // 2. The control: the same clearance with no `(type …)` reaches only
    //    `ClearanceMatrix.setDefaultValue` (`Structure.java:684-707`) and never an item class, so
    //    the order cannot matter. Without this row the test above would not say *which* shape of
    //    rule is responsible.
    let class_blind = one_rule(&dir, "plain.rules", 400.0, None);
    assert_eq!(
        build(&dsn, &class_blind, &ses, true),
        build(&dsn, &class_blind, &ses, false),
        "a class-blind clearance is order-insensitive — it writes no `defaultItemClearanceClasses`"
    );

    // 3. The binary takes Java's order. `violations` in the report is the clearance list followed
    //    by the dangling entries (`DesignRulesChecker.java:271-276`), so the clearance count is
    //    recovered by dropping the two dangling kinds.
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

    // 4. And the log says so, which is the half `p8t3 e2e`'s `rules-and-session` row compares
    //    against the jar's own two lines.
    let rules_at = stderr
        .find("Loading RULES file for DRC:")
        .unwrap_or_else(|| panic!("Freerouting.java:281 is missing:\n{stderr}"));
    let session_at = stderr
        .find("Loading SES file for DRC:")
        .unwrap_or_else(|| panic!("Freerouting.java:309 is missing:\n{stderr}"));
    assert!(rules_at < session_at, "quirk #273's order:\n{stderr}");
}

/// **Quirk #272** (label C): the quality score uses a **different settings merge** from the
/// router's — `Freerouting.java:344-347` is the prototype merger plus one `DsnFileSettings`, with
/// **no `RulesFileSettings` at priority 40**, no board pass and no merge #2.
///
/// Both halves are asserted with the *same file*, which is what makes it a quirk rather than a
/// detail:
///
/// * `freerouting route --rules <f>` reports `scoring.via_costs = 999` in its manifest — the tier
///   reaches the router;
/// * `freerouting drc --rules <f>` writes a report whose `quality_score` is **identical** to the
///   run without `--rules` — the same tier does not reach the score.
///
/// The file carries only `(autoroute_settings (via_costs 999))`, so it changes no clearance, no
/// net class and no padstack: the *whole document* is unchanged, not just the score, and that is
/// what is asserted.
#[test]
fn the_quality_score_uses_a_dsn_only_merge() {
    if !parity::require_java_dir() {
        return;
    }
    let dir = scratch("drc-separate-merge");
    let rules = autoroute_settings_rules(&dir, 999);
    let dsn = drc_dsn();

    // Half one: the same tier, on the router path, lands.
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

    // Half two: the same tier, on the DRC path, does not.
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
    // The `date` is the only field a second run may legitimately move.
    with["date"] = serde_json::Value::Null;
    without["date"] = serde_json::Value::Null;
    assert_eq!(
        with, without,
        "an `(autoroute_settings …)`-only rules file changes nothing on the DRC path at all"
    );
}

/// **`Freerouting.java:349`**: `report.qualityScore = (double) finalStats.getNormalizedScore(…)`.
///
/// `getNormalizedScore` returns a Java `float` (`BoardStatistics.java:623`) and
/// `KiCadDrcReport.qualityScore` is a `Double` (`:56-57`), so every score a jar can put in a DRC
/// document is a **widened `float`** — which is where `902.078369140625` comes from. A `f64`
/// computation would write `902.0784`, and a `f64` value that is not an exact `f32` is a number
/// no jar could ever produce.
///
/// The assertion is exactly that: the committed reference's value, its `Double.toString`
/// rendering **as text in the file**, and the round trip `f64 -> f32 -> f64` being lossless.
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

    // The literal `tests/reference/drc-dev-board/drc.json` carries — the jar's own bytes.
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

/// **All eight committed references' `quality_score`s, recomputed — with the clone alone.**
///
/// `the_quality_score_is_an_f32_widened_to_f64` pins one stem against one literal, and `p8t3 e2e`
/// pins all eight — but only on a machine with a JDK and a built jar. This closes that gap the way
/// the `route` reference lanes do: it reads the **committed** `tests/reference/drc-*/drc.json`
/// (the HEAD jar's verbatim output) and requires the CLI's *computed* score to equal the jar's
/// *recorded* one, stem for stem.
///
/// That is the whole of what Task 7 changed about the number. Plan 5 read `quality_score` out of
/// the reference and fed it back in (`crates/fr-drc/tests/reference_parity.rs::port_json`), so the
/// eight values asserted nothing about the port; `commands::drc::quality_score` now computes them
/// from `fr_router::score::BoardStatistics::normalized_score` through the merge of quirk #272, and
/// the same eight values became eight assertions.
///
/// The rows come from `tests/reference/drc-fixtures.txt`, so the tests and the generator cannot
/// drift apart, and `drc-natural-tone-preamp` is included: its **document** is quirk #146's
/// permanent `XDIFF`, and its **score** matches exactly, which is worth pinning precisely because
/// the two facts are easy to confuse.
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
        // The reference is the **HEAD** key spelling; the CLI ships KiCad's (ruling W). Read the
        // jar's value out of the committed document with `parity`'s HEAD-flavor projection.
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

/// **Ruling 6 / quirk #275, and this test is port-only.** `Freerouting.java:368-371`'s
/// `IO.println(drcReportJson)` is **dead code in the jar**: `main:1462` enters DRC mode on
/// `drcReportFile != null`, and the only assignment of that field is `GlobalSettings.java:664-668`
/// — which runs only when `-drc` was followed by a value. So `drcJob.drc` is non-null on every
/// reachable entry and the `else` can never run.
///
/// Spec §12 asks for the branch, so the port makes it live, reachable **only** from the native
/// subcommand form (`legacy::rewrite` always emits `-o <report>`). There is therefore no jar
/// answer to compare against and `p8t3` never exercises it — which is why this test exists and
/// says so.
///
/// Two things are asserted beyond "it prints": the document is **complete and parseable** (so the
/// branch is not a truncated echo), and **nothing else reaches stdout** — every log line goes to
/// stderr (quirk #261), which is what makes `freerouting drc board.dsn | jq` work.
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

// =================================================================================================
// `freerouting info` (Plan 8 Task 12)
// =================================================================================================

/// Spec §12's third subcommand, and the one CLI surface with **no Java counterpart at all** —
/// `Freerouting.main`'s mode ladder has GUI, DRC and CLI and nothing else, and `legacy::rewrite`
/// can never produce this argv. So there is nothing to compare against a jar; what is asserted is
/// that the document is the **same** one `board_info` answers (both call `fr_core::summarise`),
/// that it is complete and parseable, and that nothing but the document reaches stdout.
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

    // The five members, in declaration order (Convention 8) — asserted on the **text**, because
    // re-parsing into a `serde_json::Value` alphabetises and would hide the order this promises.
    let order: Vec<usize> = ["layers", "nets", "components", "statistics", "metadata"]
        .iter()
        .map(|key| {
            stdout
                .find(&format!("\n  \"{key}\""))
                .unwrap_or_else(|| panic!("no top-level `{key}` in\n{stdout}"))
        })
        .collect();
    assert!(order.windows(2).all(|w| w[0] < w[1]), "{order:?}");

    // Every count comes from `BoardStatistics` and the vectors carry only names.
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

/// The two failures are `drc`'s, because they are the same two calls: an unreadable input and a
/// board the loader refuses (quirk #274, label S — a `.ses` is not a board). Both exit **1**, and
/// nothing is written to stdout, so a caller piping into `jq` sees an empty stream rather than
/// half a document.
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

/// **Ruling W** (quirk #154): the CLI writes the **KiCad** key spelling by default — the one the
/// document's own `$schema` promises — while `fr_drc::DrcJsonFlavor`'s `Default` stays
/// `FreeroutingHead`, which is the *parity* choice `crates/fr-drc/tests/report_json.rs` pins
/// against the jar's Gson bytes.
///
/// The two are asserted to be genuinely different documents, and `--schema freerouting` is
/// asserted to be the way back to the jar's — which is what `p8t3 e2e`'s byte rung runs the port
/// with.
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
    // `--schema freerouting` is the HEAD-flavor projection `parity` can read; the default is not.
    assert!(parity::parse_drc_json(&head_text).is_ok());
    assert!(
        parity::parse_drc_json(&kicad_text).is_err(),
        "the two flavors must be genuinely different documents"
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
