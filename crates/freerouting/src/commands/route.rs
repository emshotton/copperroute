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
//! | 4 | delete the existing output `:116-121` | **not ported** — quirk #265 is *fixed*; see the roster below |
//! | 5 | `tryToSetOutputFile`, return discarded `:123` | [`RoutingJob::try_to_set_output_file`], **and the return is acted on** — quirk #268 is *fixed* |
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
// not ported: Freerouting.initializeCli's delete-before-run (:116-121) — quirk #265, and the one
//   `// not ported:` line in this roster that is a **fix** rather than a scope decision. The
//   `desiredOutputFile.delete()` and its warn-only failure arm are gone; `crate::logging::MESSAGE_MAP`
//   still carries `Freerouting.java:119`'s "Couldn't delete the file '{}'" because the map is the
//   **jar's** message set and `parity::normalize_log` reads it from both sides. See step 4.
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
//   plan ruling 9 / quirk #270: the SES is written **once**, not once per board-updated event.
//   Replaced by `fr_core::SyncProgressSink`. On the SES path the final bytes were unaffected
//   anyway, because `:168`'s unconditional `setJobOutput(job)` overwrites whatever the last event
//   wrote. On the KiCad-JSON path they were not — there only the *first* call ever wrote, so the
//   file held the board as loaded, which is quirk #289 (label T). fixed: T3 (#289): the port
//   serialises **once, after the pipeline**, on both paths, so the listener has nothing left to
//   reproduce and the KiCad JSON holds the same final board the SES does.
// not ported: RoutingJobSchedulerActionThread's three `StageListener` callbacks (:102-165) — one
//   `FRLogger` line each plus analytics; controller ruling AK replaces the mechanism with
//   `ProgressSink` and ruling 11 says nothing downstream reads it.

use std::path::{Path, PathBuf};

use fr_core::{
    BoardFileDetails, Ctx, FileFormat, RoutingJob, RoutingJobState, RoutingPipeline,
    RoutingResultManifest, SessionId, SyncProgressSink,
};
use fr_settings::sources::{DsnFileSettings, EnvironmentVariablesSource, JsonFileSettings};
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

    // ── 4. delete the existing output file (`:116-121`) — quirk #265, **not ported** ─────────
    //
    // fixed: T3 (#265) — Java's `:117-121` deletes the desired output file here, before
    // `job.setInput` has been validated for loadability, before both settings merges, before the
    // board load and before the router. A run that then fails, hangs (quirk #244), times out
    // without producing bytes or is refused by `BoardLoader` has therefore **destroyed the
    // previous result and written nothing in its place** — and `File.delete()` unlinks an empty
    // *directory* too, so `-do <an empty dir>` silently removes it. The delete serves nothing the
    // write does not already serve: `writeCliOutputIfAvailable`'s `Files.write` at `:206`
    // truncates. So the port deletes nothing, and the previous result survives every failure path
    // this function has. `crates/freerouting/tests/cli_e2e.rs::{a_failed_run_leaves_the_previous_result_on_disk,
    // an_empty_output_directory_is_not_unlinked}` are the two halves.

    // ── 5. `tryToSetOutputFile` (`:123`) — quirk #268 (label L), **fixed** ───────────────────
    //
    // fixed: T3 (#268) — Java discards the `boolean`, and the two halves of that go in opposite
    // directions. `-do out.txt` answers **false**, so `job.output` keeps the `<input>.ses`
    // `setInputFromFile:441` derived, and `writeCliOutputIfAvailable:206` writes to
    // `globalSettings.initialOutputFile` rather than to `job.output` — so the SES bytes land in
    // `out.txt` anyway and the run exits 0, having silently ignored what the user asked for.
    // `-do out.dsn` and `-do out.scr` answer **true** and set `job.output.format` to `DSN`/`SCR`,
    // which `setJobOutput:274-292` serialises with nothing — so `output.getData()` stays the
    // empty array, `Files.write` writes **zero bytes**, `Files.size > 0` answers false and the run
    // exits 1 with an empty file left on disk, over the previous result quirk #265 had already
    // deleted.
    //
    // So the return value is tested here, and it is not the whole test: `true` only means
    // `tryToSetOutputFile:384-388` recognised the extension, not that anything can fill the file.
    // The question the CLI actually has to ask is whether [`set_job_output`] can serialise the
    // resolved format, and [`WRITABLE_OUTPUT_FORMATS`] is that answer in one place, so the guard
    // and the writer cannot disagree. A path this program cannot write is refused **at the
    // argument** — before the settings merge, before the board load, before the router — and
    // **nothing is written and nothing is touched**.
    let accepted = job.try_to_set_output_file(Some(&args.output));
    let resolved = job.output.as_ref().map(|output| output.format);
    if !accepted || !resolved.is_some_and(|format| WRITABLE_OUTPUT_FORMATS.contains(&format)) {
        // Port-only: there is no `FRLogger` site to key this to, because Java does not refuse.
        // `crate::logging::MESSAGE_MAP` therefore does not carry it and `parity::normalize_log`
        // drops it from the comparison — which is what keeps `p8t1`'s `do-out-dsn` row a
        // comparison of the two exit codes (both 1) rather than of a message only one side has.
        tracing::error!(
            "Refusing to route: '{}' is not an output file this program can write. \
             The -do path must end in .ses (a Specctra session) or .json (a KiCad session JSON). \
             Nothing was written and nothing on disk was changed.",
            args.output.display()
        );
        return ExitCode::Failure;
    }

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
    let cli_source = super::cli_settings(settings_argv);

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
    // fixed: T1 (#224) — `jobTimeoutString` goes through `parseTimespanString`, so a job timeout
    // the port cannot read stops the run here rather than silently becoming "no timeout". Java
    // swallows the parse failure (`util/TextManager.java:91-93`) and runs unbounded, which is the
    // opposite of what the operator asked for.
    let cancel = match fr_core::job_timeout_deadline(settings.job_timeout_string.as_deref()) {
        Ok(Some(deadline)) => fr_core::CancelToken::with_deadline(deadline),
        Ok(None) => fr_core::CancelToken::new(),
        Err(error) => {
            eprintln!("--router.job_timeout: {error}");
            tracing::warn!("--router.job_timeout: {error}");
            return ExitCode::Failure;
        }
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
    // Ruling AI's wall clock. The three knobs that do not change a routed board keep Java's
    // literals; the one that does — `optChangedArea`'s inlined 1000 ms, quirk #234 — is off by
    // default, so a CLI run is reproducible. [`run_budget`] is the whole rule, precedence
    // included; read its doc before assuming this line is unconditional.
    //
    // **This** is where an unreadable `FR_ROUTER_BUDGET` becomes `exit 2`: a CLI process that was
    // told to disable a clock and could not must not route anyway. The MCP tool makes the other
    // choice — see `run_budget`'s doc — because killing a live stdio server is not a diagnosis.
    //
    // `UsageError` rather than `Failure`, and it is the same **2** the `std::process::exit(2)` this
    // replaced produced. The variant fits for the reason its own doc gives — it is the port-only
    // "you invoked me wrongly" code, and `FR_ROUTER_BUDGET` is a port-only seam Java has no
    // counterpart for — whereas `Failure` is Java's own single failure code and belongs to runs
    // that started. Nothing has been routed at this point.
    let budget = match run_budget(&settings) {
        Ok(budget) => budget,
        Err(message) => {
            eprintln!("{message}");
            tracing::warn!("{message}");
            return ExitCode::UsageError;
        }
    };
    let ctx = Ctx {
        settings: &settings,
        cancel,
        progress: &progress,
        budget,
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

    // ── 13. `setJobOutput` (`:259-295`) — the **only** serialisation, on the **final** board ──
    //
    // fixed: T3 (#289) — this call used to be handed a `pre_routing_json` snapshot taken at step
    // 12b, immediately before `RoutingPipeline::run`, because that is the document the jar's
    // `-do out.json` actually contains: `setJobOutput` is registered as a board-updated listener
    // (`RoutingJobSchedulerActionThread.java:100`) *and* called once more at `:168`, and on the
    // KiCad-session-JSON path only the **first** of those calls ever writes — `output.setData`
    // re-sniffed its own bytes (`BoardFileDetails.java:113`), a document starting `{` re-detected
    // as `KICAD_DESIGN_JSON`, and from then on neither `:275`'s `== KICAD_SESSION_JSON` nor
    // `:282`'s `== SES` matched again. The first event fires from `BatchFanout.fanoutPass:203-217`
    // before the first pin is processed, so what the jar writes is the board **as loaded**.
    //
    // MEASURED at the pinned HEAD jar, JDK 25, standard flags, and this is the transcript the fix
    // is measured *against*: `-de fixtures/Issue143-rpi_splitter.dsn -do out.json -mp {1,2,8}
    // --router.fanout.enabled=true --router.optimizer.enabled=true` produced the identical 1 540
    // bytes (md5 a20cafbec9796d7262c3a0cb2311f0ff) for all three pass counts and again with the
    // router off — `"traces": []`, `"vias": []` — while the same argv with `-do out.ses` produced
    // 16 `(wire ` and 9 `(via ` scopes. So `-do out.json` was silently useless on the jar.
    //
    // The port now serialises **once, here, after the pipeline**, on both paths, and
    // `BoardFileDetails::set_data` keeps the format it is given. The snapshot, and the third
    // `resolved_output_format` call that decided whether to take it, are gone.
    // `crates/freerouting/tests/cli_e2e.rs::do_out_json_writes_the_routed_board` is the gate and
    // `p8t7`'s rung (c) is the two-program measurement.
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

/// `RoutingJobScheduler.java:113-152` — the `.rules` **the scheduler** resolved, as bytes.
///
/// [`fr_settings::resolve_scheduler_rules_path`] is the decision (job rules, else `-dr` if it
/// exists, else the adjacent `<design>.rules`), including quirk #269 (label V)'s `else if` trap; this
/// wrapper is the read, and Java's two `catch (IOException)` arms (`:122-130`, `:142-150`) warn
/// and continue with no rules at all.
pub(crate) fn read_scheduler_rules(job: &RoutingJob, cli_rules: Option<&Path>) -> Option<Vec<u8>> {
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

/// `RoutingJobScheduler.java:189-234` — the optional session import; its `.json` arm is
/// `:194-207` and its SES arm `:208-225`.
///
/// `-di <file>` (`globalSettings.designSessionFilename`) is read onto the loaded board before the
/// router runs, so an incremental run starts from the previous result. A missing file is a
/// warning; a failure is an error; neither stops the run.
///
/// **`importSession` has TWO call sites and this is one of them**;
/// `commands/drc.rs::load_session_file` is the other (`Freerouting.java:304-306`, the `-drc`
/// path's `.json` session arm). Both go through [`fr_dsn::kicad::import_session`], and quirk
/// #290 (label U) — Java opens the file with `new FileReader`, i.e. the platform default charset,
/// where every other JSON path in the tree names UTF-8 — rides on both. That doc lives on the
/// DRC site.
pub(crate) fn import_session_file(
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
        // `:201-202` — `new FileReader(sessionFile)`, quirk #290's charset (see `drc.rs`).
        let bytes = match std::fs::read(session) {
            Ok(bytes) => bytes,
            Err(error) => {
                tracing::error!("Failed to load session file: {error}");
                return;
            }
        };
        // `:203-204` — `KiCadJsonReader.importSession(jsonReader, job.board)`.
        match fr_dsn::kicad::import_session(&String::from_utf8_lossy(&bytes), board) {
            // `:205-206`.
            Ok(()) => tracing::info!("KiCad JSON session file loaded successfully"),
            Err(error) => tracing::error!("Failed to load session file: {error}"),
        }
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

/// The output formats [`set_job_output`] can actually serialise — `setJobOutput:274-292`'s two
/// arms, `:275`'s `KICAD_SESSION_JSON` and `:282`'s `SES`, as data.
///
/// fixed: T3 (#268) — this exists so that step 5's refusal and the writer's `match` are the same
/// list. Java has no such list: `tryToSetOutputFile:384-388` accepts `DSN | FRB | SES | SCR |
/// KICAD_DESIGN_JSON`, `setJobOutput` fills two of those five, and nothing in between notices —
/// which is how `-do out.dsn` reaches `Files.write` with an empty array. A format added to the
/// writer must be added here, and a reviewer can check that by reading two adjacent things.
const WRITABLE_OUTPUT_FORMATS: [FileFormat; 2] = [FileFormat::Ses, FileFormat::KicadSessionJson];

/// `setJobOutput:260-271`'s format resolution, hoisted out of it.
///
/// Java asks the question twice — once inside `setJobOutput`'s `job.output == null` arm and,
/// implicitly, every time the `:275`/`:282` ladder re-reads `job.output.format`. The port asked it
/// a **third** time before the pipeline ran, to decide whether to take quirk #289's pre-routing
/// snapshot; fixed: T3 (#289) removed that call with the snapshot it existed for, so the function
/// is back to Java's two askers.
///
/// `job.output.format` when there is one — `tryToSetOutputFile:391` set it from the *output*
/// path's extension — and otherwise `:265-271`'s derivation from the **input** format.
fn resolved_output_format(job: &RoutingJob) -> FileFormat {
    if let Some(output) = job.output.as_ref() {
        return output.format;
    }
    // `:265-271`.
    if job.get_input().map(|input| input.format) == Some(FileFormat::KicadDesignJson) {
        FileFormat::KicadSessionJson
    } else {
        FileFormat::Ses
    }
}

/// `RoutingJobSchedulerActionThread.setJobOutput` (`:259-295`), the once-only call at `:168`.
///
/// Java writes the bytes into `job.output.data`; the port answers them, because
/// [`BoardFileDetails::set_data`] re-sniffs what it is given (`:113`) and the round trip is quirk
/// label T's first half. `job.output` is still updated, so the manifest and
/// [`write_cli_output_if_available`] see Java's own object.
///
/// Writes **nothing** for every format that is neither `SES` nor `KICAD_SESSION_JSON`. In Java
/// that is what leaves `-do out.dsn`/`out.scr` with an empty `output.getData()` and, one step
/// later, a 0-byte file and exit 1 (quirk #268, label L). The `_ => None` arm is kept, because it
/// is `setJobOutput`'s own shape and this function is also the MCP tool's writer — but on the CLI
/// path it is now **unreachable**: step 5 refuses every format outside
/// [`WRITABLE_OUTPUT_FORMATS`] at the argument, so the CLI never reaches here with one.
///
/// fixed: T3 (#289) — this used to take a `pre_routing_json: Option<String>`, the
/// `KiCadJsonWriter.write` snapshot the CLI took before `RoutingPipeline::run` because that is
/// the document the jar's `-do out.json` ends up holding. Both parameter and snapshot are gone:
/// the KiCad-JSON arm below serialises the **same board** the SES arm does, which is the board
/// this function is called with, which is the board the pipeline finished on. The mechanism that
/// made the jar different — `setData` re-sniffing `{` back to `KICAD_DESIGN_JSON` and turning
/// every later call into a no-op — is fixed one layer down, in
/// [`BoardFileDetails::set_data`](fr_core::BoardFileDetails::set_data).
pub(crate) fn set_job_output(
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
        output.format = resolved_output_format(job);
        // `:267` / `:270` — the extension follows the format.
        let extension = if output.format == FileFormat::KicadSessionJson {
            "json"
        } else {
            "ses"
        };
        output.set_filename(Some(&format!("{base}.{extension}")));
        job.output = Some(output);
    }

    let format = job.output.as_ref().map(|output| output.format);
    let bytes = match format {
        // `:275-281` — `KiCadJsonWriter.write(job.board, job.name)`, then
        // `output.setData(jsonStr.getBytes(UTF_8))`.
        //
        // fixed: T3 (#289) — `board` is the board the pipeline finished on, and this is now the
        // only call, so what `-do out.json` holds is the **routed** board. `:278` is
        // `jsonStr.getBytes(java.nio.charset.StandardCharsets.UTF_8)`, i.e. the **explicit** UTF-8
        // that quirk #290's `new FileReader` on the read side lacks; a Rust `String` is UTF-8 by
        // construction, so `into_bytes` is that.
        Some(FileFormat::KicadSessionJson) => {
            Some(fr_dsn::kicad::write(board, &job.name).into_bytes())
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
    // which is Java's `if (boardManager.saveAsSpecctraSessionSes(baos, job.name))` guard.
    //
    // fixed: T3 (#289) — and the format handed over is the format the bytes were serialised
    // **for**, not a re-sniff of them. In Java `:113` re-derived it: `{` became
    // `KICAD_DESIGN_JSON`, which is why a second call could never match `:275` again. Here the
    // two cannot disagree, because the `match` above is what chose both.
    if let (Some(output), Some(format), Some(bytes)) = (job.output.as_mut(), format, bytes) {
        output.set_data(bytes, format);
    }
}

/// `Freerouting.writeCliOutputIfAvailable` (`:196-213`).
///
/// The three gates in Java's order: `job.output` present (`:197-199`), a state of `COMPLETED` or
/// `TIMED_OUT` (`:200-203`), then `Files.write` followed by `Files.exists && size > 0`
/// (`:205-208`). The last is what makes a 0-byte write answer `false` — and in Java the empty
/// file is still left behind, which is quirk #268's `-do out.dsn` behaviour.
///
/// fixed: T3 (#268) — **the port never reaches this function with nothing to write.** Step 5
/// refuses at the argument every format [`set_job_output`] cannot fill, so `data` below is a
/// serialised board on every path that gets here, and the 0-byte `Files.write` has no caller.
/// The `size > 0` check is kept as Java's, because it is also how a full disk is caught.
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

/// The router's wall-clock budget for this run.
///
/// # The precedence, which is the whole of this function (controller ruling BR)
///
/// ```text
/// FR_ROUTER_BUDGET, if set    the test-harness seam  — whole-budget, coarse, wins outright
///   else router.opt_changed_area_ms, if set   the user's setting — one field
///     else RouterBudget::default()            the port's own budget
/// ```
///
/// The two upper levels are different kinds of thing and that is why one can outrank the other
/// without either being redundant. The setting names **one field**; the environment variable
/// selects a whole budget and exists so a harness can assert it took the clock out of a
/// measurement. A harness that set the variable and silently got the settings file's value back
/// would be measuring something it did not choose, so when the variable is set, it wins.
///
/// # `FR_ROUTER_BUDGET` is a test-harness seam, not a user surface
///
/// Ruling AI's rule is that *time is out of every quality measurement*: a wall-clock budget makes
/// the answer depend on how fast the machine is, so a comparison taken with a clock live is a
/// comparison of two machines as much as of two programs. Every parity driver therefore runs
/// [`fr_router::pipeline::RouterBudget::disabled`], and Plan 9's per-task quality A/B
/// (`scripts/quality-ab.sh`) must do the same — but that harness drives **the CLI**, as a whole
/// program, because that is what its 29 stems are references of. It needs one lever that reaches
/// inside a process it can only start, and this is that lever.
///
/// **#234 makes half of this seam redundant, and only half.** Since `opt_changed_area_ms` defaults
/// to `0`, an unset run and a `disabled` run already agree about the pull-tight clock — the one
/// clock that changes a routed board. What still separates them is
/// [`fr_router::pipeline::RouterBudget::fanout_ms_per_pin`]: Java's `10000` against `disabled`'s
/// [`i32::MAX`]. The fanout stage's per-pin budget is live in a default run, it *does* change what
/// gets routed on a big board (it is why `Issue420-contribution-board.dsn` was rejected as a
/// reference stem), and taking it out of a quality measurement is exactly ruling AI. So
/// `scripts/quality-ab.sh`'s quality lane still sets this variable. Whether the seam survives at
/// all once nothing needs it is Task 24's decision, parked there.
///
/// It is an environment variable read at exactly one site precisely so that it is *not* part of
/// the settings surface several Plan 9 tasks are busy making predictable: it cannot be set by a
/// settings file, it cannot be merged, it does not appear in the manifest's `settings_snapshot`,
/// and `EnvironmentVariablesSource` cannot see it (that source reads only `FREEROUTING__ROUTER__*`,
/// `settings/sources/EnvironmentVariablesSource.java:59-61`, and this name deliberately does not
/// start with that prefix). A user who wants the jar's pull-tight clock back asks for the setting
/// — `--router.opt_changed_area_ms=1000` — and never for this.
///
/// An earlier draft of this comment argued that a *flag* for the budget would be "a user-visible
/// setting the Java program does not have", and #234 then added exactly such a setting. Both are
/// right, and the contradiction was in treating them as the same object: `router.opt_changed_area_ms`
/// is a documented user setting that exists because Java's own constant is unreachable, and
/// `FR_ROUTER_BUDGET` is a harness seam that exists because a subprocess has no other way to be
/// told "no clocks at all". Neither is the other's flag.
///
/// # What is left when nothing is set
///
/// [`fr_router::pipeline::RouterBudget::default`] — Java's literals for the fanout per-pin budget
/// and the two progress throttles, and **0 for `opt_changed_area_ms`** (#234, Plan 9 Task 1), which
/// is Java's own "off" value and makes a plain CLI run reproducible. An unrecognised
/// `FR_ROUTER_BUDGET` value is a hard error rather than a silent fallback: a harness that thinks
/// it disabled the clock and did not would produce numbers nobody could trust, and that is worse
/// than a stopped run.
///
/// # Why it answers a `Result` rather than exiting
///
/// It used to call `std::process::exit(2)` itself, which was safe while the CLI was its only
/// caller. It is not any more: `mcp::tools::route_board` calls it too (#234 made the two faces
/// share a budget), and a **long-running stdio server** must not be killed mid-JSON-RPC by a
/// misconfigured environment variable — that would bypass the drain machinery
/// `the_drain_takes_no_new_work_after_a_write_failure` exists to protect. So the decision belongs
/// to the caller: **the CLI still exits 2**, and the MCP tool answers `invalid_params` and stays
/// up. That is the shape [`fr_core::job_timeout_deadline`] already uses two lines above each call
/// site, and the two now match.
pub(crate) fn run_budget(
    settings: &fr_settings::RouterSettings,
) -> Result<fr_core::RouterBudget, String> {
    use std::env::VarError;
    Ok(match std::env::var("FR_ROUTER_BUDGET").as_deref() {
        // Unset, or set to the empty string. Empty is treated as unset **deliberately**, and it is
        // the one place the "a set value is a set value" rule below is not applied: `FOO=` is how
        // a shell script unsets a variable it cannot `unset` (a `Makefile` recipe, a CI matrix
        // entry that leaves a cell blank), so reading it as "no budget named" is what the people
        // who write those files mean. It is also the only unrecognised value that cannot be a
        // *typo* for one of the four names.
        Err(VarError::NotPresent) | Ok("") => settings_budget(settings),
        // `default` names the port's default explicitly and therefore also overrides the setting;
        // a harness that asks for the default is asking for the default, not for whatever the
        // board's settings file happens to carry.
        Ok("default") => fr_core::RouterBudget::default(),
        Ok("java") => fr_core::RouterBudget::java_literals(),
        Ok("disabled") => fr_core::RouterBudget::disabled(),
        // A set-but-unreadable value is a *set* value. Folding `NotUnicode` into the unset arm
        // would hand a harness that believes it disabled the clock a run with the clock live,
        // which is the one failure mode this seam exists to make impossible.
        other => {
            let shown = match other {
                Ok(v) => format!("{v:?}"),
                Err(_) => "<not valid unicode>".to_string(),
            };
            return Err(format!(
                "FR_ROUTER_BUDGET={shown} is not a budget; use `default` (the port's own — \
                 Java's literals with the optChangedArea clock off), `java` (Java's four \
                 literals, quirk #234's 1000 ms included), or `disabled` (ruling AI's \
                 every-clock-off, what the parity drivers and scripts/quality-ab.sh run)"
            ));
        }
    })
}

/// [`fr_router::pipeline::RouterBudget::default`], with `router.opt_changed_area_ms` applied if
/// the resolved settings carry one.
///
// fixed: T1 (#234) — the setting the register asked for ("make the limit a settings field so a
// reproducible run is expressible"), reaching the budget. This is the only field of the four the
// settings surface exposes, because it is the only one of the four that changes a routed board.
fn settings_budget(settings: &fr_settings::RouterSettings) -> fr_core::RouterBudget {
    let mut budget = fr_core::RouterBudget::default();
    if let Some(ms) = settings.opt_changed_area_ms {
        budget.opt_changed_area_ms = ms;
    }
    budget
}
