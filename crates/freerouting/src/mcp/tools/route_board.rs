//! `route_board` — spec §13's
//! `{ dsn_path | dsn_text, ses_path?, rules_path?, output_path?, settings? }
//!  → { ses_path | ses_text, stats, incompletes, drc_violation_count, timed_out }`.
//!
//! # Ruling AU: this is Java's **API** path, not its CLI path
//!
//! `commands::route` is `Freerouting.initializeCli` and reaches
//! [`fr_settings::resolve_headless`], whose premise is that **merge #1 already ran** — its result
//! is complete, so re-injecting it at priority 70 leaves merge #2's own `0..60` chain reachable
//! only where merge #1 left a field absent. An MCP call is not that shape. There is no command
//! line carrying `-mp`, and the `settings` argument is a *sparse* object the caller wrote by
//! hand, which is exactly `api/v1/JobInputResource.java:203-211`'s
//! `GSON.fromJson(requestBody, RouterSettings.class)` followed by `job.setSettings(...)`.
//!
//! So this runner composes [`SettingsMerger`] **directly**, in
//! `RoutingJobScheduler.scheduleJob`'s own order (`RoutingJobScheduler.java:91-186`), and that is
//! what discharges the port's half of the `obligation:` at `crates/fr-settings/src/resolve.rs:193`.
//! **That marker has two users** and the other one is not touched here: Task 7's DRC quality
//! score (`commands::drc::quality_score_settings`, quirk #272) is a *different* composition — the
//! prototype merger plus one `DsnFileSettings`, no `.rules` tier, no board — and the marker names
//! both.
//!
//! The order, with Java's line beside each step:
//!
//! | # | Java | here |
//! |---|---|---|
//! | 1 | `GSON.fromJson(body, RouterSettings.class)` `JobInputResource.java:203` | [`settings_payload`] — quirk #141's reader |
//! | 2 | `job.setSettings(...)` `:210` | `job.router_settings = …`, **before** the load |
//! | 3 | `HeadlessBoardManager.loadFromSpecctraDsn` `RoutingJobScheduler.java:96-101` | [`fr_core::parse_board_if_needed`] |
//! | 4 | `applyParsedBoardResult`'s two post-load passes `HeadlessBoardManager.java:733-734` | [`fr_core::apply_router_settings_for_loaded_board`] + [`fr_core::apply_immediate_post_load_processing`] |
//! | 5 | `settingsMergerProtype.clone()` `RoutingJobScheduler.java:104` | [`prototype_merger`] |
//! | 6 | `new DsnFileSettings(...)` at 20 `:106-110` | ↓ |
//! | 7 | the `.rules` resolution and `RulesFileSettings` at 40 `:113-156` | [`crate::commands::route::read_scheduler_rules`] |
//! | 8 | `new ApiSettings(job.routerSettings)` at 70 `:159-166` | ↓ — **the sparse tier** |
//! | 9 | `settingsMerger.merge()` `:169` | ↓ |
//! | 10 | `RulesReader.read(..., job.board, job.routerSettings)` `:172-183` | both halves, see step 10 below |
//! | 11 | `job.routerSettings.applyBoardSpecificOptimizations(job.board)` `:186` | ↓ |
//! | 12 | the session import `:189-234` | [`crate::commands::route::import_session_file`] |
//! | 13 | `pipeline.run()` | [`fr_core::RoutingPipeline::run`] |
//! | 14 | `setJobOutput` | [`crate::commands::route::set_job_output`] |
//!
//! **Step 4 is why the priority-70 payload is never empty even when `settings` is absent.**
//! `applyRouterSettingsForLoadedBoard` runs at *load* time on `job.routerSettings`, which on this
//! path is `new RouterSettings()` plus whatever the caller sent — so the layer count and the
//! board-tuned trace costs are written into the payload *before* it becomes `ApiSettings`, and
//! they outrank everything below. That is Java, and it is the structural difference from
//! `resolve_headless` the obligation is about: there the same pass runs on merge #1's complete
//! result.
//!
//! # Progress and cancellation
//!
//! This is the only tool that can run for minutes, so it is the only one that reports
//! (`notifications/progress`, through Task 11's [`ProgressWriter`]) and the only one whose
//! cancellation can arrive mid-run. The token reaches the router through controller ruling BB's
//! poll seam — [`fr_core::CancelToken::as_router_stop`] installs the closure, and the **four**
//! sites `RouterStop::poll_cancel` sits at copy the flag in: the three pass loop heads Task 11
//! landed, plus the per-**item** loop of `AutoroutePassRunner` that Task 12 added under ruling AI,
//! because one pass of a real board is 135 seconds and a cancellation that takes that long is not
//! one (`fr_router::pipeline::stop`'s module doc carries the table and the measurement).
//! **Java has neither progress nor cancellation** on any of its 28 tools (delta row 6).

use super::super::jsonrpc::RpcError;
use super::super::server::{ProgressWriter, State};
use crate::commands::route::{import_session_file, read_scheduler_rules, set_job_output};
use fr_core::{CancelToken, Ctx, FileFormat, RoutingJobState, RoutingPipeline, SyncProgressSink};
use fr_settings::sources::{
    ApiSettings, DefaultSettings, DsnFileSettings, EnvironmentVariablesSource, JsonFileSettings,
    RulesFileSettings,
};
use fr_settings::{HostEnvironment, RouterSettings, SettingsMerger, SettingsSource};
use serde_json::{Value, json};
use std::path::PathBuf;

/// Routes a board and answers the session.
///
/// # Errors
///
/// [`RpcError::invalid_params`] for a bad argument, an unreadable input, a `settings` object the
/// reader refuses (quirk #141), or a board that will not parse; [`RpcError::internal`] if the
/// pipeline fails, if no session was produced, or if `output_path` cannot be written.
pub fn run(
    state: &State,
    args: Value,
    progress: &ProgressWriter,
    cancel: &CancelToken,
) -> Result<Value, RpcError> {
    let ses = super::optional_string(&args, "ses_path")?.map(PathBuf::from);
    let rules = super::optional_string(&args, "rules_path")?.map(PathBuf::from);
    let output_path = super::optional_string(&args, "output_path")?.map(PathBuf::from);
    let payload = settings_payload(&args)?;

    // ── 1-2. the sparse payload becomes `job.routerSettings`, **before** the load ─────────────
    let mut job = super::board_input(&args)?;
    if let Some(payload) = payload {
        job.router_settings = payload;
    }
    // `RoutingJobScheduler.java:117-119` — the job's own rules bytes are the first branch of the
    // `.rules` resolution, and `-dr`/`rules_path` is the second. Setting both keeps this runner's
    // resolution identical to the CLI's, which is what `read_scheduler_rules` implements.
    if let Some(path) = rules.as_deref()
        && let Err(error) = job.set_rules(path)
    {
        tracing::warn!("Couldn't load rules file '{}': {error}", path.display());
    }
    let rules_bytes = read_scheduler_rules(&job, rules.as_deref());

    // ── 3. the parse ─────────────────────────────────────────────────────────────────────────
    let parsed = fr_core::parse_board_if_needed(&job)
        .map_err(|error| RpcError::invalid_params(error.to_string()))?;
    let mut board = parsed.board;
    let transform = parsed.transform;
    for warning in &parsed.warnings {
        tracing::warn!("{warning}");
    }

    // ── 4. the two post-load passes, on the payload — see the module docs ────────────────────
    // fixed: T10 (#231) — see `commands::route` for why this is `info!` and not `debug`.
    if fr_core::apply_router_settings_for_loaded_board(&mut board, &mut job.router_settings) {
        tracing::info!(
            copper_to_edge_clearance_um = ?job.router_settings.copper_to_edge_clearance_um,
            hole_clearance_um = ?job.router_settings.hole_clearance_um,
            "the clearance overrides changed the board's clearance matrix"
        );
    }
    fr_core::apply_immediate_post_load_processing(&mut board);

    // ── 5-9. merge #2, alone, with the sparse payload at priority 70 ─────────────────────────
    let host = HostEnvironment::detect();
    let mut merger = prototype_merger(&state.settings_argv, &host);
    // `:106-110` — the DSN at 20, and only for a DSN input; a KiCad-JSON job registers none.
    if job.get_input().map(|input| input.format) == Some(FileFormat::Dsn) {
        let input = job.get_input().expect("checked just above");
        merger.add_or_replace_sources(vec![Box::new(DsnFileSettings::new(
            input.get_data(),
            input.get_filename(),
        ))]);
    }
    // `:154-156` — the `.rules` at 40, from the bytes the scheduler resolved.
    if let Some(bytes) = rules_bytes.as_deref() {
        // Java's second argument is the file's own name (`:155`), and it reaches exactly one
        // place: `RulesFileSettings.getSourceName`, a display string no merge step reads. The
        // job's own `rules` filename is used where there is one — the adjacent-`<design>.rules`
        // probe has no name to offer, and could not change a merged value if it did.
        let label = job
            .rules
            .as_ref()
            .map_or_else(|| "rules".to_string(), |r| r.get_filename().to_string());
        merger.add_or_replace_sources(vec![Box::new(RulesFileSettings::new(bytes, &label))]);
    }
    // `:159-166` — **the sparse tier**. This is the whole of ruling AU.
    merger.add_or_replace_sources(vec![Box::new(ApiSettings::new(Some(
        job.router_settings.clone(),
    )))]);
    // `:169`.
    let mut settings = merger.merge(&host);

    // ── 10. the post-merge `RulesReader.read`, **both** halves ───────────────────────────────
    //
    // Java hands the reader `job.board` *and* `job.routerSettings` (`:172-183`), so one call
    // writes the clearances, net classes, padstacks, via rules and snap angle into the board and
    // the `(autoroute_settings …)` scopes into the settings. The port's two halves live in two
    // crates — `fr_dsn::rules_reader::read` owns the board and
    // `fr_settings::sources::rules_file::apply_rules_file_against_board` the settings, because
    // `fr-dsn` cannot see `RouterSettings` (quirk #142's second parse) — so both are called here.
    // `commands::route` calls only the first, because `resolve_headless` already ran the second.
    if let Some(bytes) = rules_bytes.as_deref() {
        fr_settings::prelude::apply_rules_file_against_board(bytes, &board, &mut settings);
        if let Err(error) =
            fr_dsn::rules_reader::read(bytes, &job.name, &mut board, &transform, None)
        {
            // `:180-182` — an error here logs and does not stop the run.
            tracing::error!("Failed to apply rules from rules file: {error}");
        }
    }

    // ── 11. `:186`, unguarded in Java (see `resolve_headless`'s `totalized:` note) ────────────
    settings.apply_board_specific_optimizations(&board);
    job.router_settings = settings.clone();

    // ── 12. the optional session import ──────────────────────────────────────────────────────
    import_session_file(ses.as_deref(), &mut board, &transform);

    // ── 13. the run ──────────────────────────────────────────────────────────────────────────
    job.started_at = Some(std::time::Instant::now());
    job.state = RoutingJobState::Running;
    // The **transport's** token, with the job deadline attached — never a fresh one, or an
    // inbound `notifications/cancelled` would have nothing to reach. See
    // `CancelToken::with_deadline_from`.
    // fixed: T1 (#224) — as on the CLI path: an unreadable job timeout is refused, with the
    // string named, rather than silently running unbounded.
    let cancel = match fr_core::job_timeout_deadline(settings.job_timeout_string.as_deref()) {
        Ok(Some(deadline)) => cancel.with_deadline_from(deadline),
        Ok(None) => cancel.clone(),
        Err(error) => return Err(RpcError::invalid_params(format!("job_timeout: {error}"))),
    };
    let job_deadline = cancel.clone();
    let sink = progress_sink(progress);
    // The CLI's budget, arrived at by the CLI's own function, because this tool is the CLI's other
    // face: `p8t1`'s SES bytes are what the end-to-end conversation test compares against, and a
    // different budget would be a different board. That includes honouring `opt_changed_area_ms`
    // when the caller's `settings` payload carries it (#234) — a setting that worked on one face
    // and not the other would be worse than no setting.
    //
    // The **failure** is where the two faces part company. `run_budget` answers a `Result` rather
    // than exiting precisely so that this call site can refuse the request and leave the server
    // up: a misconfigured `FR_ROUTER_BUDGET` must not kill a stdio server mid-JSON-RPC, which
    // would take every other in-flight call down with it and bypass the drain
    // (`the_drain_takes_no_new_work_after_a_write_failure`). The CLI still exits 2.
    let budget = crate::commands::route::run_budget(&settings).map_err(RpcError::invalid_params)?;
    let ctx = Ctx {
        settings: &settings,
        cancel,
        progress: &sink,
        budget,
    };
    let result = RoutingPipeline::run(&mut board, &ctx).map_err(|error| {
        RpcError::internal(format!(
            "Failed to set up routing job '{}', it will be terminated.: {error}",
            job.id.to_java_string()
        ))
    })?;
    job.finished_at = Some(std::time::Instant::now());
    // fixed: T9 (#267) — the two stages write two fields. Java has one `job.currentPass` and
    // both loops write it, so `RoutingResultManifest.fromJob:124-126` reported whichever wrote
    // last under a key that names the **autorouter**; `phases.optimizer.passes_completed` is
    // Task 20's to fill (quirk #254) and the number is now here waiting for it.
    job.set_current_pass(result.pipeline.router_passes_completed);
    job.set_optimizer_pass(result.pipeline.optimizer_passes_completed);
    // `TIMED_OUT` comes from the **job** deadline and from nothing else — a per-stage timeout
    // leaves the job `COMPLETED`. `commands::route`'s step 12 carries the measurement.
    job.state = if job_deadline.is_timed_out() {
        RoutingJobState::TimedOut
    } else {
        RoutingJobState::Completed
    };

    // ── 14. the session bytes ────────────────────────────────────────────────────────────────
    //
    // **The output format is pinned to `SES`, and that is a decision rather than a default.**
    // Spec §13's result member is `ses_path | ses_text`: a Specctra session, whatever went in.
    // Left alone, the format would come from the *input*'s extension (`RoutingJob.java:438-454`
    // derives `<input>.json` for a KiCad design JSON, and `setJobOutput:265-271` derives
    // `KICAD_SESSION_JSON` when there is no output at all), so a KiCad design JSON would come
    // back as a KiCad session JSON — a document this tool's result has no member to carry.
    //
    // fixed: T3 (#289) — the reason has narrowed to exactly that. Until Plan 9 Task 3 the
    // KiCad-session-JSON arm also handed back the board **as loaded, before any routing** (quirk
    // #289, label T), and pinning the format here was what kept that unreachable from this path.
    // `set_job_output` now serialises the final board on both arms, so the quirk is gone from
    // both programs; the pin stays because the *format* is still a spec §13 decision.
    //
    // `p8t7`'s rung (b) is where the KiCad-JSON-in, SES-out path is compared against the jar,
    // through the CLI, on the argv a user types.
    let base = job.get_input().map_or_else(
        || job.name.clone(),
        fr_core::BoardFileDetails::get_filename_without_extension,
    );
    let mut output = job.output.take().unwrap_or_default();
    output.format = FileFormat::Ses;
    // The **field**, not `set_filename`: that method re-derives `directory_path` from its
    // argument, and a bare name would blank the directory the input's own path put there.
    output.filename = format!("{base}.ses");
    job.output = Some(output);
    set_job_output(&mut job, &board, &transform);
    let output = job
        .output
        .as_ref()
        .filter(|output| !output.get_data().is_empty())
        .ok_or_else(|| RpcError::internal("the router produced no session document"))?;
    let bytes = output.get_data().to_vec();

    let mut answer = super::file_payload_fields(output);
    let object = answer.as_object_mut().expect("a JSON object");
    object.insert("job_id".into(), json!(job.id.to_java_string()));
    match output_path.as_deref() {
        // Spec §13's file-path variant: the bytes go to disk and the model gets a path, which is
        // what keeps a 3 MB session out of its context.
        Some(path) => {
            std::fs::write(path, &bytes).map_err(|error| {
                RpcError::internal(format!(
                    "Couldn't save the output file '{}': {error}",
                    path.display()
                ))
            })?;
            object.insert("ses_path".into(), json!(path.display().to_string()));
        }
        // …and the text variant, where `data` (Base64) rides along under
        // `BoardFilePayload`'s own name (`api/dto/BoardFilePayload.java:19-28`), for a caller
        // that wants the bytes rather than the text.
        None => {
            object.insert(
                "ses_text".into(),
                json!(String::from_utf8_lossy(&bytes).into_owned()),
            );
            object.insert("data".into(), json!(super::base64_encode(&bytes)));
        }
    }
    object.insert("stats".into(), fr_core::to_gson_json(&result.stats));
    // Scan ruling R4: `incompletes` is the **count**, from
    // `BoardStatistics.connections.incompleteCount`. The plan drafted a vector of names; Plan 7
    // pinned `build_unrouted_report` as *text*, and structuring it here would be a second,
    // port-only implementation of a byte-pinned method. The text is `unrouted_report`.
    object.insert("incompletes".into(), json!(result.incomplete_count()));
    object.insert("unrouted_report".into(), json!(result.unrouted_report));
    object.insert(
        "drc_violation_count".into(),
        json!(result.violation_count()),
    );
    object.insert("timed_out".into(), json!(result.timed_out));
    Ok(answer)
}

/// `Freerouting.java:1408-1413`'s `settingsMergerProtype`, rebuilt: `DefaultSettings(0)`,
/// `JsonFileSettings(10)`, `CliSettings(60)` and `EnvironmentVariablesSource(55)`.
///
/// Rebuilt rather than cloned, for the reason `commands::drc::quality_score_settings` records:
/// `SettingsMerger::clone` is `not ported:` because a `Vec<Box<dyn SettingsSource>>` cannot share
/// instances, and a merge never mutates a source, so a second merger over the same inputs is the
/// same value.
///
/// The argv is the **server's own** ([`State::settings_argv`]) — in Java the prototype merger is a
/// field of the running JVM, built from the command line the server was started with, and it is
/// the same field an API job's merge #2 clones.
fn prototype_merger(settings_argv: &[String], host: &HostEnvironment) -> SettingsMerger {
    let mut sources: Vec<Box<dyn SettingsSource>> = vec![Box::new(DefaultSettings::new(host))];
    // Priority 10, reachable only through `--settings <file>` on the native form (ruling BG —
    // see `commands::json_settings_path`, and note that `freerouting mcp` *is* the native form).
    if let Some(path) = crate::commands::json_settings_path(settings_argv) {
        sources.push(Box::new(JsonFileSettings::new(&path)));
    }
    // 60 and 55, in `:1411-1412`'s registration order (the merge sorts by priority, so the order
    // is cosmetic — kept as Java writes it).
    sources.push(Box::new(crate::commands::cli_settings(settings_argv)));
    let environment: std::collections::BTreeMap<String, String> = std::env::vars().collect();
    sources.push(Box::new(EnvironmentVariablesSource::new(&environment)));
    SettingsMerger::new(sources)
}

/// The `settings` argument, read the way Java reads a request body.
///
/// **Quirk #141 governs this reader.** `GsonProvider.GSON` is `Strictness.LENIENT`, so Java
/// accepts unquoted names, single quotes, `//` comments, a quoted scalar coerced into a typed
/// field (`{"max_passes": "42"}` → `42`), a duplicate key (last wins), a fractional literal
/// truncated into an `Integer`, and the non-finite literals `NaN`/`Infinity`. `serde_json`
/// refuses every one of them, and [`RouterSettings::from_json_str`] — `json.rs`'s named reader —
/// is what this calls, **not** `serde_json::from_value`, so the port has exactly one reader of
/// this type and the divergence is recorded in exactly one place.
///
/// The two non-finite literals are refused **both ways**: `serde_json` will not read `NaN` (the
/// whole `tools/call` line fails to parse first, with `-32700`), and `to_gson_string_pretty` will
/// not write one — Gson's own `IllegalArgumentException`.
///
/// Absent, `null` and `{}` are all "no override": absent and `null` leave `job.routerSettings` as
/// `RoutingJob`'s own `new RouterSettings()`, and `{}` reads back as the same value, because
/// Gson's `ObjectConstructor` allocates `fanout`/`optimizer`/`scoring` before any field is read
/// (`RouterSettings.java:119-124`) and the port's three `#[serde(default = …)]`s reproduce it.
///
/// # Errors
///
/// [`RpcError::invalid_params`] when `settings` is not an object, or is an object the strict
/// reader refuses.
fn settings_payload(args: &Value) -> Result<Option<RouterSettings>, RpcError> {
    let settings = match args.get("settings") {
        None | Some(Value::Null) => return Ok(None),
        Some(Value::Object(map)) => Value::Object(map.clone()),
        Some(_) => {
            return Err(RpcError::invalid_params(
                "settings must be an object; call list_settings for the field names",
            ));
        }
    };
    // Re-serialised to text and handed to the named reader, rather than
    // `serde_json::from_value`: the arguments arrived as a `Value` because the transport parses
    // whole lines, and routing them through `from_json_str` keeps `fr_settings::json` the single
    // reader of this type (quirk #141's note names this call site).
    let text = settings.to_string();
    RouterSettings::from_json_str(&text)
        .map(Some)
        .map_err(|error| {
            RpcError::invalid_params(format!(
                "settings is not a valid RouterSettings object: {error}"
            ))
        })
}

/// A [`SyncProgressSink`] that turns the pipeline's events into `notifications/progress`.
///
/// **Three of the five variants are reported and two are not**, and the choice is about rate
/// rather than about interest: `BoardUpdated` fires on a throttled board update and
/// `OptimizerImproved` fires **per item**, which on a real board is thousands of lines down a
/// pipe an agent reads one at a time. What is reported is the stage-level ladder — a stage
/// changing state, a fanout pass's pin counts, and a completed routing pass — which is the same
/// granularity the jar's own log lines have.
///
/// `progress` must be monotonic (MCP §Progress), so it is a plain counter rather than a
/// percentage: the number of passes a run will take is not known until it has taken them.
/// `total` is therefore omitted on every notification except the fanout's, where
/// `routed + pins_to_go` is a real denominator.
///
/// Ruling 11 — *no port decision reads the sink* — is what makes installing this safe: the board
/// this run produces is byte-identical to the one the CLI's `SyncProgressSink::noop()` produces,
/// which is what the end-to-end conversation test asserts against `p8t1`'s reference SES.
fn progress_sink(progress: &ProgressWriter) -> SyncProgressSink {
    if !progress.is_enabled() {
        // The drop-everything writer: a tool never *has* to branch, and this one does only to
        // avoid formatting a message nobody will read (`ProgressWriter::is_enabled`'s own doc).
        return SyncProgressSink::noop();
    }
    let progress = progress.clone();
    let mut step = 0.0f64;
    SyncProgressSink::new(move |event| {
        // `fr_core` re-exports the router's event enum (plan-8 ruling 1), so this crate needs
        // no `fr-router` dependency of its own.
        use fr_core::RoutingEvent;
        let (message, total) = match event {
            RoutingEvent::TaskStateChanged { algorithm, state } => {
                (format!("{algorithm:?}: {state:?}"), None)
            }
            RoutingEvent::FanoutProgress {
                pass,
                routed,
                pins_to_go,
            } => (
                format!("fanout pass #{pass}: {routed} routed, {pins_to_go} to go"),
                Some(f64::from(routed + pins_to_go)),
            ),
            RoutingEvent::BoardSnapshot { pass } => (format!("pass #{pass} complete"), None),
            RoutingEvent::BoardUpdated { .. } | RoutingEvent::OptimizerImproved { .. } => return,
        };
        step += 1.0;
        progress.progress(step, total, Some(&message));
    })
}
