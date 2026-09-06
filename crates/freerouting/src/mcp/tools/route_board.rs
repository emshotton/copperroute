use super::super::jsonrpc::RpcError;
use super::super::server::{ProgressWriter, State};
use crate::commands::route::{import_session_file, read_scheduler_rules, set_job_output};
use fr_core::{
    CancelToken, Ctx, FileFormat, JobStopReason, RoutingJobState, RoutingPipeline, SyncProgressSink,
};
use fr_settings::sources::{
    ApiSettings, DefaultSettings, DsnFileSettings, EnvironmentVariablesSource, JsonFileSettings,
    RulesFileSettings,
};
use fr_settings::{HostEnvironment, RouterSettings, SettingsMerger, SettingsSource};
use serde_json::{Value, json};
use std::path::PathBuf;

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
        let label = job
            .rules
            .as_ref()
            .map_or_else(|| "rules".to_string(), |r| r.get_filename().to_string());
        merger.add_or_replace_sources(vec![Box::new(RulesFileSettings::new(bytes, &label))]);
    }
    merger.add_or_replace_sources(vec![Box::new(ApiSettings::new(Some(
        job.router_settings.clone(),
    )))]);
    // `:169`.
    let mut settings = merger.merge(&host);

    if let Some(bytes) = rules_bytes.as_deref() {
        fr_settings::prelude::apply_rules_file_against_board(bytes, &board, &mut settings);
        if let Err(error) =
            fr_dsn::rules_reader::read(bytes, &job.name, &mut board, &transform, None)
        {
            // `:180-182` — an error here logs and does not stop the run.
            tracing::error!("Failed to apply rules from rules file: {error}");
        }
    }

    settings.apply_board_specific_optimizations(&board);
    job.router_settings = settings.clone();

    // ── 12. the optional session import ──────────────────────────────────────────────────────
    import_session_file(ses.as_deref(), &mut board, &transform);

    // ── 13. the run ──────────────────────────────────────────────────────────────────────────
    job.started_at = Some(std::time::Instant::now());
    job.state = RoutingJobState::Running;
    let cancel = match fr_core::job_timeout_deadline(settings.job_timeout_string.as_deref()) {
        Ok(Some(deadline)) => cancel.with_deadline_from(deadline),
        Ok(None) => cancel.clone(),
        Err(error) => return Err(RpcError::invalid_params(format!("job_timeout: {error}"))),
    };
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
    job.set_current_pass(result.pipeline.router_passes_completed);
    job.set_optimizer_pass(result.pipeline.optimizer_passes_completed);
    job.state = match result.stop_reason {
        Some(JobStopReason::Deadline) => RoutingJobState::TimedOut,
        Some(JobStopReason::Cancelled) => RoutingJobState::Cancelled,
        None => RoutingJobState::Completed,
    };

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
        None => {
            object.insert(
                "ses_text".into(),
                json!(String::from_utf8_lossy(&bytes).into_owned()),
            );
            object.insert("data".into(), json!(super::base64_encode(&bytes)));
        }
    }
    object.insert("stats".into(), fr_core::to_gson_json(&result.stats));
    object.insert("incompletes".into(), json!(result.incomplete_count()));
    object.insert("unrouted_report".into(), json!(result.unrouted_report));
    object.insert(
        "drc_violation_count".into(),
        json!(result.violation_count()),
    );
    object.insert("timed_out".into(), json!(result.timed_out));
    Ok(answer)
}

fn prototype_merger(settings_argv: &[String], host: &HostEnvironment) -> SettingsMerger {
    let mut sources: Vec<Box<dyn SettingsSource>> = vec![Box::new(DefaultSettings::new(host))];
    if let Some(path) = crate::commands::json_settings_path(settings_argv) {
        sources.push(Box::new(JsonFileSettings::new(&path)));
    }
    sources.push(Box::new(crate::commands::cli_settings(settings_argv)));
    let environment: std::collections::BTreeMap<String, String> = std::env::vars().collect();
    sources.push(Box::new(EnvironmentVariablesSource::new(&environment)));
    SettingsMerger::new(sources)
}

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
