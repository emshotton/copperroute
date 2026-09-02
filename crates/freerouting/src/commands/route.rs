//! `freerouting route` — `Freerouting.initializeCli` (`Freerouting.java:79-187`) end to end,
//! together with the essential slice of the two classes the job would have travelled through.
//!
//! # The sixteen steps, and where each one is
//!
//! Java's CLI is three collaborating objects: `initializeCli` builds the job and then *blocks*
//! (`:151-158`) until a background `RoutingJobScheduler` has loaded the board, merged the settings
//! a second time and handed the job to a `RoutingJobSchedulerActionThread`, which runs the
//! pipeline and serialises the result back into `job.output`. The port has no scheduler and no
//! action thread — plan ruling 9 and controller ruling AK removed the event mechanism, and Task 0
//! removed the monitor thread — so all three collapse into this one function, **in Java's order**:
//!
//! | # | Java | here |
//! |---|---|---|
//! | 1 | the input/output guard `:80-86` | `// not reachable:` below — clap and `legacy::rewrite` both guarantee both slots |
//! | 2 | `job.setInput` `:101-106` | [`RoutingJob::set_input`] |
//! | 3 | `job.input == null` `:108-112` | the `Err` arm of step 2 |
//! | 4 | delete the existing output `:116-121` | [`delete_existing_output`] — quirk #265 |
//! | 5 | `tryToSetOutputFile`, return discarded `:123` | [`RoutingJob::try_to_set_output_file`] — quirk #268 |
//! | 6 | merge #1's sources `:125-144` | [`SettingsInputs`] |
//! | 7 | `merger.merge()` `:146` | ↓ |
//! | 8 | `drcSettings.clone()` `:147` | [`RoutingJob::drc_settings`] |
//! | 9 | the board load + merge #2 `RoutingJobScheduler.java:91-170` | [`fr_core::parse_board_if_needed`] + **one** [`resolve_headless`] |
//! | 10 | the post-merge `RulesReader.read` `:173-184` | `resolve_headless` (settings half) + [`fr_dsn::rules_reader::read`] (board half) |
//! | 11 | the session import `:189-234` | [`import_session_file`] |
//! | 12 | `pipeline.run()` `RoutingJobSchedulerActionThread.java:99-167` | [`RoutingPipeline::run`] |
//! | 13 | `setJobOutput` `:259-295` | [`set_job_output`] |
//! | 14 | `writeCliOutputIfAvailable` `:196-213` | [`write_cli_output_if_available`] |
//! | 15 | `computeCliExitCode` `:215-223` | [`compute_cli_exit_code`] |
//! | 16 | `writeCliResultManifestIfRequested` `:225-244` | [`write_cli_result_manifest_if_requested`] |
//!
//! # Steps 6-10 are one call to `resolve_headless`, and the parse had to move to make that work
//!
//! [`fr_settings::resolve_headless`] already linearises Java's whole two-merge ladder — merge #1,
//! `HeadlessBoardManager.applyRouterSettingsForLoadedBoard`'s two *settings* steps, the private
//! `boardSpecificTraceCostsApplied` flag, merge #2, the post-merge `.rules` re-apply and both
//! `validate()` calls — and Plan 4's `p4t1` pins it against the JVM over a 64-case matrix.
//! Convention 10 says fill [`SettingsInputs`] and call it once, which is what happens below.
//!
//! It takes the **board**, though, and Java's board load takes **merge #1's settings** (they are
//! what `applyCopperToEdgeClearanceOverride`/`applyHoleClearanceOverride` read at `:746-747`).
//! That is circular unless the parse is separable from the two post-load passes, so Task 6 split
//! [`fr_core::parse_board_if_needed`] out of `load_board_if_needed` and the order here is
//!
//! ```text
//! parse the board            HeadlessBoardManager.java:697-698 / applyParsedBoardResult:712-732
//! resolve_headless(&board)   Freerouting.java:125-146 + RoutingJobScheduler.java:93-186
//! the two post-load passes   HeadlessBoardManager.java:741-747, :755
//! ```
//!
//! **The one consequence, stated rather than hidden:** [`fr_router::pipeline::prepare_board`]
//! therefore sees the *resolved* settings where Java's sees merge #1's. It reads exactly two
//! fields, `copperToEdgeClearanceUm` and `holeClearanceUm`, and nothing between merge #1 and the
//! resolved answer can write either — merge #2's own chain reaches only fields merge #1 left
//! absent and `DefaultSettings.getSettings` fills both (`:107-108`, the assignments; `:78`/`:81`
//! are the constants they read), `applyBoardSpecificOptimizations` writes
//! trace costs and preferred directions, and the `(autoroute_settings …)` grammar the post-merge
//! re-apply parses has no member for either. The measurement is
//! `crates/fr-router/tests/batch_parity.rs`, which routes eight boards in exactly this order and
//! is byte-identical to the jar on all eight, and `tests/reference/cli-*`, which compares two
//! whole programs.
//!
//! # What is deliberately absent
//!
// not ported: Freerouting.initializeCli's SessionManager/session bookkeeping (:88-99, :114) —
//   `SessionManager.getInstance().createSession(...)` and `cliSession.addJob(job)` register the
//   job in a process-wide singleton whose only readers are the REST API's `/sessions` resources
//   (rostered in `crates/fr-core/src/lib.rs` §1) and the `shortName` log prefix. The port builds
//   the [`RoutingJob`] directly; [`RoutingJob::new`] still derives the `<session6>\<job6>` short
//   name, so a host that wants the prefix has it.
// not ported: Freerouting.initializeCli's blocking wait (:151-158) — `while
//   (!isCliTerminalState(job.state)) Thread.sleep(500)`. There is no scheduler thread to wait
//   for; the pipeline runs on this stack. Plan ruling 7 also *totalises* the predicate: `INVALID`
//   is terminal here and exits 1, where Java hangs for ever (quirk #244).
// not ported: Freerouting.initializeCli's donation banner (:164-183) — quirk #266 (label H).
//   It prints to **stdout** when the output was written, the persisted
//   `statistics.jobsCompleted >= 5` and `userProfileSettings.userEmail.isEmpty()`. It is
//   un-suppressible in Java (no flag reaches the `if`) and would corrupt a stdout-JSON mode, so
//   the port does not print it and `parity::normalize_log` strips it from the Java side.
//   `crates/freerouting/tests/cli_e2e.rs::no_donation_banner_on_stdout` pins the absence.
// not ported: Freerouting.initializeCli's `globalSettings.cliExitCode = cliExitCode` (:185) — a
//   field on the process-wide `GlobalSettings` singleton whose only other reader is
//   `Freerouting.main:1490`, which is this function's caller. The port returns the code.
// not ported: RoutingJobSchedulerActionThread's monitor thread (:55-90) — Task 0's [`CancelToken`]
//   carries its two observable instants (quirk #237); the CPU/memory sampling has no reader but
//   `resourceUsage`, which `p8t2`'s normaliser strips (plan ruling 8).
// not ported: RoutingJobSchedulerActionThread's `FRAnalytics.autorouterStarted/Finished` and
//   `routeOptimizerStarted/Finished` (:93, :127-133, :157, :162) — spec §2 drops analytics.
// not ported: RoutingPipeline.addBoardUpdatedEventListener(event -> setJobOutput(job)) (:100) —
//   plan ruling 9 / quirk #270: the SES is written **once**, not once per board-updated event. Replaced by
//   `fr_core::SyncProgressSink`. The final bytes are unaffected on this path, because `:168`'s
//   unconditional `setJobOutput(job)` overwrites whatever the last event wrote; on the KiCad JSON
//   path they are not (quirk label T, Task 10's).
// not ported: RoutingJobSchedulerActionThread's three `StageListener` callbacks (:102-165) — one
//   `FRLogger` line each plus analytics; controller ruling AK replaces the mechanism with
//   `ProgressSink` and ruling 11 says nothing downstream reads it.

use std::path::{Path, PathBuf};

use fr_core::{
    BoardFileDetails, Ctx, FileFormat, RoutingJob, RoutingJobState, RoutingPipeline,
    RoutingResultManifest, SessionId, SyncProgressSink,
};
use fr_settings::sources::{
    CliSettings, DsnFileSettings, EnvironmentVariablesSource, JsonFileSettings,
};
use fr_settings::{HostEnvironment, SettingsInputs, SettingsSource};

use crate::cli::RouteArgs;
use crate::legacy::ExitCode;

/// `Freerouting.initializeCli` (`Freerouting.java:79-187`), plus the scheduler and action-thread
/// slices it blocks on.
///
/// `settings_argv` is the **raw** argv, not the rewritten one — see [`crate::run`]'s "The raw
/// argv is the settings argv".
pub fn run(args: &RouteArgs, settings_argv: &[String]) -> ExitCode {
    // ── 1. the input/output guard (`:80-86`) ──────────────────────────────────────────────────
    //
    // not reachable: Freerouting.initializeCli:80-86 — `initialInputFile == null ||
    // initialOutputFile == null`. Both slots are non-optional here: clap requires the positional
    // input and `-o/--output` on the native form, and `legacy::rewrite` answers an **empty** argv
    // (which `crate::run` turns into exit 1) rather than building a `route` command line with a
    // slot missing. `legacy::rewrite` is what emits the `FRLogger.error` at `:81`, keyed
    // `Freerouting.java:81` in `crate::logging::MESSAGE_MAP`, so the message set is still Java's.

    let mut job = RoutingJob::new(SessionId::NIL);

    // ── 2-3. `job.setInput` (`:101-106`) and the null check (`:108-112`) ─────────────────────
    //
    // `setInputFromFile` reads the bytes *before* it assigns `this.input` (`RoutingJob.java:428`),
    // so an unreadable file is the only way `job.input` stays null — which is why Java's `catch`
    // at `:104` and its `if` at `:108` are two arms of one failure and not two failures.
    if let Err(error) = job.set_input(&args.input) {
        // :105 — `FRLogger.error("Couldn't load the input file '" + … + "'", e)`.
        tracing::error!(
            "Couldn't load the input file '{}': {error}",
            args.input.display()
        );
        // :109 — `FRLogger.warn("Couldn't read the input file '" + … + "', aborting.")`.
        tracing::warn!(
            "Couldn't read the input file '{}', aborting.",
            args.input.display()
        );
        return ExitCode::Failure;
    }
    //
    // not ported: Freerouting.initializeCli:103-105's `inputFileSettings` local — a
    // `DsnFileSettings` built inside the `try` and **never read**: `:126-127` constructs a second
    // one from the same two arguments for the merger. Dead in Java; the port builds the one the
    // merge uses, below.

    // ── 4. delete the existing output file (`:116-121`) — quirk #265 ─────────────────────────
    delete_existing_output(&args.output);

    // ── 5. `tryToSetOutputFile`, return value discarded (`:123`) — quirk #268 (label L) ───────
    //
    // Java ignores the `boolean`. `-do out.txt` therefore answers `false`, leaves `job.output` as
    // the `<input>.ses` `setInputFromFile:441` derived, and `writeCliOutputIfAvailable` then
    // writes the SES bytes to `out.txt` anyway. `-do out.dsn` answers `true` and sets the format
    // to `DSN`, which `setJobOutput` cannot serialise — so a 0-byte file is left behind and the
    // run exits 1. Both are reproduced; `crates/freerouting/tests/cli_e2e.rs` pins them.
    let _accepted = job.try_to_set_output_file(Some(&args.output));

    // ── 6. merge #1's sources (`:125-144`) ────────────────────────────────────────────────────
    //
    // The prototype merger (`Freerouting.java:1408-1413`) contributes `DefaultSettings(0)`,
    // `JsonFileSettings(10)`, `EnvironmentVariablesSource(55)` and `CliSettings(60)`; `:126-127`
    // adds the DSN at 20 and `:129-144` the `-dr` rules at 40.
    let input_data = job
        .get_input()
        .expect("step 2 assigned the input")
        .get_data()
        .to_vec();
    let input_filename = job
        .get_input()
        .expect("step 2 assigned the input")
        .get_filename()
        .to_string();
    let dsn_source = DsnFileSettings::new(&input_data[..], &input_filename);

    // `:129-144` — `job.setRules(initialRulesFile)`, whose own `rulesFile.exists()` guard
    // (`RoutingJob.java:302`) silently ignores a missing file, and then the priority-40 source
    // only when the bytes actually arrived (`:132`'s `job.rules != null && … getData() != null`).
    if let Some(rules_path) = args.rules.as_deref()
        && let Err(error) = job.set_rules(rules_path)
    {
        // :138 — `FRLogger.warn("Couldn't load rules file '" + … + "': " + e.getMessage())`.
        tracing::warn!(
            "Couldn't load rules file '{}': {error}",
            rules_path.display()
        );
    }
    let cli_rules_bytes: Option<Vec<u8>> = job
        .rules
        .as_ref()
        .map(|rules| rules.get_data().to_vec())
        .filter(|data| !data.is_empty());

    // The `.rules` the **scheduler** picks (`RoutingJobScheduler.java:115-152`), which is a
    // different decision from merge #1's: the job's own bytes, else `-dr` if it exists, else an
    // adjacent `<design>.rules` beside the DSN — and, quirk #269 (label V), a `-dr` naming a
    // **non-existent** file takes the second branch and stops there, silently disabling the
    // adjacent probe a user would otherwise have got.
    let scheduler_rules_bytes = read_scheduler_rules(&job, args.rules.as_deref());

    // Priority 10 (scan ruling R7): `--settings <file>` is `JsonFileSettings(Path)`, on the
    // **native** form only (ruling BG — see [`json_settings_path`]).
    //
    // **There is no default file, and that is measured rather than chosen.** Java's no-argument
    // `JsonFileSettings()` (`JsonFileSettings.java:27-29`) resolves
    // `GlobalSettings.getUserDataPath().resolve("freerouting.json")` — on macOS
    // `~/Library/Application Support/freerouting/freerouting.json`
    // (`AppPaths.resolveConfigDirectory:39-42`) — and that path is `static` mutable state
    // (`GlobalSettings.java:29-30`, `:164-167`) which spec §2 does not port. Task 5 made the
    // **working directory** stand in for it; ruling BG asked what the jar actually does with a
    // `freerouting.json` in the working directory, and the answer is **nothing**:
    //
    //   jar, run from a directory holding `{"router":{"scoring":{"via_costs":77}}}`  -> 50
    //   jar, `-Duser.home` at a home whose user-data file sets the same           -> 77
    //   jar, `-Duser.home` at a home with no such file (control)                  -> 50
    //
    // So the cwd stand-in was not a stand-in for anything the jar does; it was a second,
    // port-only default that a stray file in a build directory could use to change a routing
    // result silently. The tier is therefore reachable **only** through `--settings <file>`, and
    // `JsonFileSettings::from_working_directory` keeps no caller here — see its own doc.
    let json_source =
        super::json_settings_path(settings_argv).map(|path| JsonFileSettings::new(&path));

    let environment: std::collections::BTreeMap<String, String> = std::env::vars().collect();
    let env_source = EnvironmentVariablesSource::new(&environment);
    let cli_source = CliSettings::new(settings_argv);

    // ── 7 + 9 + 10. ONE `resolve_headless` (Convention 10) ───────────────────────────────────
    let inputs = SettingsInputs {
        json_file: json_source.as_ref().and_then(SettingsSource::get_settings),
        dsn: dsn_source.get_settings(),
        // Convention 9 / quirk #142: **bytes**, because Java parses the same file twice with two
        // different layer structures and both results reach the answer.
        cli_rules: cli_rules_bytes.as_deref(),
        scheduler_rules: scheduler_rules_bytes.as_deref(),
        env: env_source.get_settings(),
        cli: cli_source.get_settings(),
    };

    // ── 8. `job.drcSettings = globalSettings.drcSettings.clone()` (`:147`) ────────────────────
    //
    // The port's `GlobalSettings` is the command line itself, and no CLI flag writes
    // `drcSettings` on the route path (`-drc` selects DRC mode, which is Task 7's runner), so the
    // clone is of `DesignRulesCheckerSettings::default()` — which is what
    // `RoutingJob::default` already put there. Assigned explicitly so the step is visible.
    job.drc_settings = fr_settings::DesignRulesCheckerSettings::default();

    // ── 9a. the board parse (`RoutingJobScheduler.java:91-101`) ───────────────────────────────
    let parsed = match fr_core::parse_board_if_needed(&job) {
        Ok(parsed) => parsed,
        Err(error) => {
            // `BoardLoader.java:52`'s `FRLogger.error("Failed to load board", e)` and
            // `RoutingJobScheduler.java:252`'s "Only DSN and JSON formats are supported as an
            // input." both land here; the port carries whichever message the loader produced.
            tracing::error!("{error}");
            // Plan ruling 7: the job would be `INVALID` (`RoutingJobScheduler.java:83`, `:253`),
            // which Java's `isCliTerminalState` omits — so the jar spins in `:151-158` for ever.
            // The port totalises it and exits 1 (quirk #244).
            job.state = RoutingJobState::Invalid;
            return finish(&job, args, false, ExitCode::Failure, None);
        }
    };
    let mut board = parsed.board;
    let transform = parsed.transform;
    for warning in &parsed.warnings {
        tracing::warn!("{warning}");
    }

    // ── 9b/10a. merge #1 + the between-merges pass + merge #2 + the post-merge re-apply ───────
    let mut settings =
        fr_settings::resolve_headless(&inputs, Some(&board), &HostEnvironment::detect());

    // ── 9c. `applyRouterSettingsForLoadedBoard` (`:741-747`) and
    //        `applyImmediatePostLoadProcessing` (`:755`) ─────────────────────────────────────
    fr_core::apply_router_settings_for_loaded_board(&mut board, &mut settings);
    fr_core::apply_immediate_post_load_processing(&mut board);

    // ── 10b. the **board** half of the post-merge `RulesReader.read` (`:173-184`) ─────────────
    //
    // `resolve_headless` performs the settings half (`fr_settings::sources::rules_file::
    // apply_rules_file_against_board`, the `(autoroute_settings …)` arm); the clearances, net
    // classes, padstacks, via rules and snap angle the same call writes into the **board** are
    // `fr_dsn::rules_reader::read`'s, and only a caller that owns the `&mut Board` can run them.
    // Java's guard is `rulesData != null && job.board != null` (`:173`) and its failure arm is a
    // `FRLogger.error` that does not stop the run (`:180-182`).
    if let Some(bytes) = scheduler_rules_bytes.as_deref() {
        let design_name = job.name.clone();
        if let Err(error) =
            fr_dsn::rules_reader::read(bytes, &design_name, &mut board, &transform, None)
        {
            // :181 — `FRLogger.error("Failed to apply rules from rules file", e)`.
            tracing::error!("Failed to apply rules from rules file: {error}");
        }
    }

    job.router_settings = settings.clone();

    // ── 11. the optional session import (`:189-234`) ──────────────────────────────────────────
    import_session_file(args.ses.as_deref(), &mut board, &transform);

    // ── 12. the routing run (`RoutingJobSchedulerActionThread.java:37-167`) ───────────────────
    //
    // `:39` — `job.startedAt = Instant.now()`, and `:41-52`'s timeout ladder, which is Task 0's
    // [`fr_core::job_timeout_deadline`] over `router.job_timeout` (kept as a `String`, as Java
    // keeps it).
    job.started_at = Some(std::time::Instant::now());
    job.state = RoutingJobState::Running;
    let cancel = match fr_core::job_timeout_deadline(settings.job_timeout_string.as_deref()) {
        Some(deadline) => fr_core::CancelToken::with_deadline(deadline),
        None => fr_core::CancelToken::new(),
    };
    // A handle on the **job** deadline, kept because the token itself is moved into the `Ctx`
    // below and the state finalisation has to ask it a question no other value can answer — see
    // the `:174-184` block.
    let job_deadline = cancel.clone();
    //
    // not ported: RoutingJobSchedulerActionThread:92-94's `routerEnabled` local — `job
    // .routerSettings.getRunRouter() && (maxPasses == null || maxPasses >= 0)`. Its only readers
    // are the `FRAnalytics.autorouterStarted()` call at `:95` and the `afterRouting` listener's
    // early return at `:106-108`, both of which are analytics or logging. `getRunRouter()` itself
    // is honoured **inside** `run_pipeline` (Plan 7), which is where Java honours it too
    // (`RoutingPipeline.java:97`).
    let progress = SyncProgressSink::noop();
    let ctx = Ctx {
        settings: &settings,
        cancel,
        progress: &progress,
        // Ruling AI's wall clock, **live** — Java's four literals. The parity drivers disable it;
        // the CLI must not, because `tests/reference/cli-*` compares two whole programs and the
        // jar cannot switch its own (javac-inlined) budget off either. See
        // `scripts/gen-cli-reference.sh`'s header.
        budget: fr_core::RouterBudget::default(),
    };
    let result = match RoutingPipeline::run(&mut board, &ctx) {
        Ok(result) => result,
        Err(error) => {
            // `RoutingJobScheduler.java:243-249`'s `catch (Exception e)` → `TERMINATED`. Plan-7
            // ruling 7's `NoRoutableLayer` escapes `RoutingPipeline.run` in Java too.
            tracing::error!(
                "Failed to set up routing job '{}', it will be terminated.: {error}",
                job.id.to_java_string()
            );
            job.state = RoutingJobState::Terminated;
            job.finished_at = Some(std::time::Instant::now());
            return finish(&job, args, false, ExitCode::Failure, None);
        }
    };

    // `:174-184` — `finishedAt`, then the state finalisation. The port has no `STOPPING` arm to
    // take, because nothing outside this stack can request a stop, so the whole ladder reduces to
    // `RUNNING -> COMPLETED` unless the **job deadline** already made the state `TIMED_OUT`.
    job.finished_at = Some(std::time::Instant::now());
    // `job.currentPass` is **not** the routing stage's pass count: both loops write the same
    // field at the top of their own iteration (`AutorouteBatchLoop.java:276`,
    // `BatchOptimizer.java:196`) and the manifest reads whatever was written last
    // (`RoutingResultManifest.java:124-126`), so one completed optimizer pass overwrites a
    // three-pass routing stage with `1`. `PipelineResult::last_reported_pass` is that value.
    // Quirk #267; measured against the jar by `p8t2 e2e` on `router-dac2020-bm01` and
    // `router-strict-drc-cnh`, which were the two rows that caught it.
    job.set_current_pass(result.pipeline.last_reported_pass);
    // **`TIMED_OUT` comes from the job deadline and from nothing else.** In Java the only writer
    // of that state is the monitor thread (`RoutingJobSchedulerActionThread.java:84`), which
    // tests `job.timeoutAt` — the instant `:41-51` derives from `routerSettings.jobTimeoutString`
    // — and `:175-184` then leaves it alone, because neither the `RUNNING` nor the `STOPPING` arm
    // matches. A **per-stage** timeout does not reach the state at all: `isFanoutTimedOut()` and
    // `getOptimizer().isTimedOut()` (`:170-172`) reach only the finish log's details string
    // (`:175-177`), and the job finishes `COMPLETED`.
    //
    // So this is deliberately **not** `result.timed_out`, which
    // `fr_router::pipeline::PipelineResult` folds from three sources — the job deadline, the
    // fanout stage's per-pin budget and the optimizer stage's own `timeout` — because that is
    // what a *router* caller wants to know. Using it here would report `"final_state":
    // "TIMED_OUT"` where the jar reports `"COMPLETED"`.
    //
    // **Measured on the HEAD jar**, `Issue649-kicad_ecc83-pp_input_board_v1.dsn` with
    // `-mp 8 --router.optimizer.timeout=0:00:00`: the log says
    // `Optimizer stage timed out before starting pass #1`, then
    // `Job '…' finished with state: COMPLETED (optimizer stage timed out)`, and the manifest says
    // `"final_state": "COMPLETED"`. Pinned by
    // `crates/freerouting/tests/cli_e2e.rs::a_stage_timeout_is_not_a_job_timeout`.
    //
    // The port has no monitor thread (quirk #237), so the question is asked once, here, instead
    // of every second: a run that overran its deadline answers `true` either way, and a run that
    // did not answers `false` either way. The stop the monitor raises at `:87` is
    // `CancelToken::as_router_stop`'s deadline, which `RoutingPipeline::run` already installed.
    job.state = if job_deadline.is_timed_out() {
        RoutingJobState::TimedOut
    } else {
        RoutingJobState::Completed
    };

    // ── 13. `setJobOutput` (`:259-295`) ───────────────────────────────────────────────────────
    set_job_output(&mut job, &board, &transform);

    // ── 14-16. ───────────────────────────────────────────────────────────────────────────────
    let output_written = write_cli_output_if_available(&job, &args.output);
    let exit_code = compute_cli_exit_code(&job, output_written);
    finish(&job, args, output_written, exit_code, Some(&result.stats))
}

/// Steps 15-16 for every exit that reaches them, so that a failure before the router still writes
/// the manifest `--router.result_json` asked for — which is Java, because
/// `writeCliResultManifestIfRequested` is called unconditionally at `:161`.
fn finish(
    job: &RoutingJob,
    args: &RouteArgs,
    output_written: bool,
    exit_code: ExitCode,
    stats: Option<&fr_core::BoardStatistics>,
) -> ExitCode {
    write_cli_result_manifest_if_requested(job, args, output_written, exit_code, stats);
    exit_code
}

/// `Freerouting.initializeCli:116-121` — **quirk #265**: the desired output file is deleted before
/// anything is routed, and a delete that fails is only warned about.
///
/// A run that then hangs, is killed, or fails has destroyed the previous result and written
/// nothing in its place.
fn delete_existing_output(output: &Path) {
    // `:117` — `(desiredOutputFile != null) && desiredOutputFile.exists()`. `File.exists()` is
    // false for a broken symlink and for a path the process cannot stat, which is why the port
    // uses `Path::exists` (the same `stat`-and-swallow) rather than `symlink_metadata`.
    if !output.exists() {
        return;
    }
    // `:118` — `desiredOutputFile.delete()`, whose `boolean` is the whole error channel.
    // `File.delete()` removes an **empty directory** too; `std::fs::remove_file` does not, so the
    // directory case is retried with `remove_dir` to keep the predicate Java's.
    if std::fs::remove_file(output).is_ok() {
        return;
    }
    if output.is_dir() && std::fs::remove_dir(output).is_ok() {
        return;
    }
    // `:119` — `FRLogger.warn("Couldn't delete the file '" + … + "'")`.
    tracing::warn!("Couldn't delete the file '{}'", output.display());
}

/// `RoutingJobScheduler.java:113-152` — the `.rules` **the scheduler** resolved, as bytes.
///
/// [`fr_settings::resolve_scheduler_rules_path`] is the decision (job rules, else `-dr` if it
/// exists, else the adjacent `<design>.rules`), including quirk #269 (label V)'s `else if` trap; this
/// wrapper is the read, and Java's two `catch (IOException)` arms (`:122-130`, `:142-150`) warn
/// and continue with no rules at all.
fn read_scheduler_rules(job: &RoutingJob, cli_rules: Option<&Path>) -> Option<Vec<u8>> {
    // `:115-117` — the job's own bytes, already in hand; no `exists()` probe.
    if let Some(rules) = job.rules.as_ref() {
        let data = rules.get_data();
        if !data.is_empty() {
            return Some(data.to_vec());
        }
    }
    let dsn_path = job.get_input().and_then(|input| {
        (input.format == FileFormat::Dsn).then(|| PathBuf::from(input.get_absolute_path()))
    });
    let path = fr_settings::resolve_scheduler_rules_path(None, cli_rules, dsn_path.as_deref())?;
    match std::fs::read(&path) {
        Ok(bytes) => Some(bytes),
        Err(error) => {
            // `:127-129` / `:147-149`.
            tracing::warn!("Failed to read rules file: {}: {error}", path.display());
            None
        }
    }
}

/// `RoutingJobScheduler.java:189-234` — the optional session import.
///
/// `-di <file>` (`globalSettings.designSessionFilename`) is read onto the loaded board before the
/// router runs, so an incremental run starts from the previous result. A missing file is a
/// warning; a failure is an error; neither stops the run.
//
// obligation: Task 9/10 (`io/kicad/KiCadJsonReader.importSession`, `RoutingJobScheduler.java
//   :199-211`) — a `-di` whose name ends `.json` is a KiCad session import, which is not ported.
//   Until then the arm logs Java's own `FRLogger.error("Failed to load session file", e)` text
//   with the reason, rather than silently importing nothing.
fn import_session_file(
    session: Option<&Path>,
    board: &mut fr_board::Board,
    transform: &fr_dsn::CoordinateTransform,
) {
    let Some(session) = session else {
        return;
    };
    // `:193-196` — `sessionFile.exists()`.
    if !session.exists() {
        // `:229-231`.
        tracing::warn!("Session file not found: {}", session.display());
        return;
    }
    // `:197-201` — the extension test is `toLowerCase().endsWith(".json")`.
    if session.to_string_lossy().to_lowercase().ends_with(".json") {
        tracing::info!("Loading KiCad JSON session file: {}", session.display());
        tracing::error!(
            "Failed to load session file: the KiCad JSON session reader is not ported yet \
             (Plan 8 Task 10)"
        );
        return;
    }
    // `:213-227` — `SesReader.read(sesStream, job.board)`.
    tracing::info!("Loading SES file: {}", session.display());
    let file = match std::fs::File::open(session) {
        Ok(file) => file,
        Err(error) => {
            tracing::error!("Failed to load session file: {error}");
            return;
        }
    };
    match fr_dsn::ses_reader::read(file, board, transform) {
        Ok(summary) => {
            // `:220-227` — the message is built with the same three counters and the same
            // parenthesised suffix.
            let errors = if summary.errors_encountered > 0 {
                format!(" ({} errors)", summary.errors_encountered)
            } else {
                String::new()
            };
            tracing::info!(
                "SES file loaded: {} wires, {} vias imported{errors}",
                summary.wires_imported,
                summary.vias_imported
            );
        }
        Err(error) => {
            tracing::error!("Failed to load session file: {error}");
        }
    }
}

/// `RoutingJobSchedulerActionThread.setJobOutput` (`:259-295`), the once-only call at `:168`.
///
/// Java writes the bytes into `job.output.data`; the port answers them, because
/// [`BoardFileDetails::set_data`] re-sniffs what it is given (`:113`) and the round trip is quirk
/// label T's first half. `job.output` is still updated, so the manifest and
/// [`write_cli_output_if_available`] see Java's own object.
///
/// Writes **nothing** for every format that is neither `SES` nor `KICAD_SESSION_JSON` — which is
/// what leaves `-do out.dsn`/`out.scr` with an empty `output.getData()` and, one step later, a
/// 0-byte file and exit 1 (quirk #268, label L).
fn set_job_output(
    job: &mut RoutingJob,
    board: &fr_board::Board,
    transform: &fr_dsn::CoordinateTransform,
) {
    // `:260-272` — the `job.output == null` arm. `tryToSetOutputFile` and `setInputFromFile` have
    // both had their chance by now, so this is reachable only for an input that derives no
    // default output (`SES`, `RULES`, …) *and* an output path `tryToSetOutputFile` rejected.
    if job.output.is_none() {
        let base = job.get_input().map_or_else(
            || job.name.clone(),
            BoardFileDetails::get_filename_without_extension,
        );
        let mut output = BoardFileDetails::default();
        if job.get_input().map(|input| input.format) == Some(FileFormat::KicadDesignJson) {
            output.format = FileFormat::KicadSessionJson;
            output.set_filename(Some(&format!("{base}.json")));
        } else {
            output.format = FileFormat::Ses;
            output.set_filename(Some(&format!("{base}.ses")));
        }
        job.output = Some(output);
    }

    let format = job.output.as_ref().map(|output| output.format);
    let bytes = match format {
        // `:274-281` — Task 10's `KiCadJsonWriter.write`.
        Some(FileFormat::KicadSessionJson) => {
            // obligation: Task 10 (`io/kicad/KiCadJsonWriter`) replaces this arm with the real
            //   writer. Until then a `-de <board>.json` run reaches here and Java's own
            //   `FRLogger.error("Couldn't save the JSON output into the job object.", e)` text is
            //   what the caller sees; the output stays empty, so the run exits 1 rather than
            //   writing a file that is not a KiCad session.
            tracing::error!(
                "Couldn't save the JSON output into the job object.: the KiCad JSON writer is \
                 not ported yet (Plan 8 Task 10)"
            );
            None
        }
        // `:282-292` — `boardManager.saveAsSpecctraSessionSes(baos, job.name)`. The design name
        // is `job.name`, which `setInputFromFile:457` set to the input's base name.
        Some(FileFormat::Ses) => {
            let mut buffer = Vec::new();
            match fr_core::save_as_specctra_session_ses(board, transform, &job.name, &mut buffer) {
                Ok(()) => Some(buffer),
                Err(error) => {
                    // `:290-291`.
                    tracing::error!("Couldn't save the SES output into the job object.: {error}");
                    None
                }
            }
        }
        // Every other format falls off the end of `setJobOutput`'s `if`/`else if` with
        // `output.getData()` still null — quirk #268's `-do out.dsn` / `out.scr`.
        _ => None,
    };
    // `:279` / `:288` — `output.setData(...)` runs only when the serialisation produced bytes,
    // which is Java's `if (boardManager.saveAsSpecctraSessionSes(baos, job.name))` guard. That
    // matters: `setData` re-sniffs what it is given (`BoardFileDetails.java:112-113`), so an
    // empty write would turn the format into `UNKNOWN`.
    if let (Some(output), Some(bytes)) = (job.output.as_mut(), bytes) {
        output.set_data(bytes);
    }
}

/// `Freerouting.writeCliOutputIfAvailable` (`:196-213`).
///
/// The three gates in Java's order: `job.output` present (`:197-199`), a state of `COMPLETED` or
/// `TIMED_OUT` (`:200-203`), then `Files.write` followed by `Files.exists && size > 0`
/// (`:205-208`). The last is what makes a 0-byte write answer `false` — **and the empty file is
/// still left behind**, which is quirk #268's `-do out.dsn` behaviour.
///
// not reachable: Freerouting.writeCliOutputIfAvailable's `routingJob.output.getData() == null`
// half of the `:197` guard — `BoardFileDetails.dataBytes` is initialised to `new byte[0]`
// (`BoardFileDetails.java:53`) and `getData` wraps it in a fresh `ByteArrayInputStream`
// (`:100-102`), so the expression is never null in Java either. The port's `get_data` answers
// `&[u8]` for the same reason. It is the **empty** array, not a null, that produces the 0-byte
// file.
fn write_cli_output_if_available(job: &RoutingJob, output_path: &Path) -> bool {
    // `:197-199` — the reachable half.
    let Some(output) = job.output.as_ref() else {
        return false;
    };
    let data = output.get_data();
    // `:200-203`.
    if job.state != RoutingJobState::Completed && job.state != RoutingJobState::TimedOut {
        return false;
    }
    // `:205-206` — `Files.write(Path.of(globalSettings.initialOutputFile), …)`, the **CLI's**
    // path, not `job.output`'s. That is the whole of quirk #268's `-do out.txt` behaviour:
    // `job.output` still names `<input>.ses` and the bytes go to `out.txt`.
    if let Err(error) = std::fs::write(output_path, data) {
        // `:210`.
        tracing::error!(
            "Couldn't save the output file '{}': {error}",
            output_path.display()
        );
        return false;
    }
    // `:207`.
    std::fs::metadata(output_path).is_ok_and(|meta| meta.len() > 0)
}

/// `Freerouting.computeCliExitCode` (`:215-223`).
fn compute_cli_exit_code(job: &RoutingJob, output_written: bool) -> ExitCode {
    // `:216-221` — `COMPLETED && written` or `TIMED_OUT && written` is 0; everything else is 1.
    if output_written
        && matches!(
            job.state,
            RoutingJobState::Completed | RoutingJobState::TimedOut
        )
    {
        ExitCode::Ok
    } else {
        ExitCode::Failure
    }
}

/// `Freerouting.writeCliResultManifestIfRequested` (`:225-244`).
///
/// The guard is `routerSettings.resultJsonPath` non-null and not `isBlank()` (`:227-231`);
/// `RouterSettings::java_clone` deliberately does **not** carry the field (quirk #114), so it
/// reaches the answer only through the source that set it — `--router.result_json=<f>` on the
/// command line, or `FREEROUTING__ROUTER__RESULT_JSON` in the environment.
fn write_cli_result_manifest_if_requested(
    job: &RoutingJob,
    args: &RouteArgs,
    output_written: bool,
    exit_code: ExitCode,
    stats: Option<&fr_core::BoardStatistics>,
) {
    let path = result_json_path(job, args);
    let Some(path) = path else {
        return;
    };
    // `:233-237` — `RoutingResultManifest.fromJob(job, globalSettings.initialInputFile,
    // outputWritten, exitCode)`, then `write(Path.of(resultJsonPath), manifest)`.
    let manifest = RoutingResultManifest::from_job(
        job,
        Some(&args.input),
        output_written,
        exit_code.code(),
        // `:101` — `Instant.now().toString()`, hand-rolled by Task 6 (see
        // `fr_core::now_utc_iso8601` for the eleven jar-measured renderings it is pinned to).
        &fr_core::now_utc_iso8601,
        stats,
    );
    if let Err(error) = RoutingResultManifest::write(Path::new(&path), &manifest) {
        // `:240-243` — Java catches `IOException` only, and quirk #257 records the one input
        // (`-de /`) where an NPE escapes it instead.
        tracing::error!("Couldn't write routing result manifest to '{path}': {error}");
    }
}

/// `routerSettings.resultJsonPath` (`:228-230`), with the native `--result-json` spelling folded
/// in.
///
/// The job's own settings are the Java answer. `RouteArgs::result_json` is the port-only native
/// flag; it is consulted only when the settings ladder left the field blank, so a
/// `--router.result_json=` on the same command line still wins exactly as it does in Java.
fn result_json_path(job: &RoutingJob, args: &RouteArgs) -> Option<String> {
    let from_settings = job
        .router_settings
        .result_json_path
        .as_deref()
        .map(str::to_string)
        .filter(|path| !path.trim().is_empty());
    from_settings.or_else(|| {
        args.result_json
            .as_deref()
            .map(|path| path.to_string_lossy().into_owned())
    })
}
