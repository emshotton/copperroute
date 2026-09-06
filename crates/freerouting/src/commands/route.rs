use std::path::{Path, PathBuf};

use fr_core::{
    BoardFileDetails, Ctx, FileFormat, RoutingJob, RoutingJobState, RoutingPipeline,
    RoutingResultManifest, SessionId, SyncProgressSink,
};
use fr_settings::sources::{DsnFileSettings, EnvironmentVariablesSource, JsonFileSettings};
use fr_settings::{HostEnvironment, SettingsInputs, SettingsSource};

use crate::cli::RouteArgs;
use crate::legacy::ExitCode;

pub fn run(args: &RouteArgs, settings_argv: &[String]) -> ExitCode {
    let mut job = RoutingJob::new(SessionId::NIL);

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

    let accepted = job.try_to_set_output_file(Some(&args.output));
    let resolved = job.output.as_ref().map(|output| output.format);
    if !accepted || !resolved.is_some_and(|format| WRITABLE_OUTPUT_FORMATS.contains(&format)) {
        tracing::error!(
            "Refusing to route: '{}' is not an output file this program can write. \
             The -do path must end in .ses (a Specctra session) or .json (a KiCad session JSON). \
             Nothing was written and nothing on disk was changed.",
            args.output.display()
        );
        return ExitCode::Failure;
    }

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

    let scheduler_rules_bytes = read_scheduler_rules(&job, args.rules.as_deref());

    let json_source =
        super::json_settings_path(settings_argv).map(|path| JsonFileSettings::new(&path));

    let environment: std::collections::BTreeMap<String, String> = std::env::vars().collect();
    let env_source = EnvironmentVariablesSource::new(&environment);
    let cli_source = super::cli_settings(settings_argv);

    // ── 7 + 9 + 10. ONE `resolve_headless` (Convention 10) ───────────────────────────────────
    let inputs = SettingsInputs {
        json_file: json_source.as_ref().and_then(SettingsSource::get_settings),
        dsn: dsn_source.get_settings(),
        cli_rules: cli_rules_bytes.as_deref(),
        scheduler_rules: scheduler_rules_bytes.as_deref(),
        env: env_source.get_settings(),
        cli: cli_source.get_settings(),
    };

    job.drc_settings = fr_settings::DesignRulesCheckerSettings::default();

    let parsed = match fr_core::parse_board_if_needed(&job) {
        Ok(parsed) => parsed,
        Err(error) => {
            tracing::error!("{error}");
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

    if fr_core::apply_router_settings_for_loaded_board(&mut board, &mut settings) {
        tracing::info!(
            copper_to_edge_clearance_um = ?settings.copper_to_edge_clearance_um,
            hole_clearance_um = ?settings.hole_clearance_um,
            "the clearance overrides changed the board's clearance matrix"
        );
    }
    fr_core::apply_immediate_post_load_processing(&mut board);

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

    super::drc::load_kicad_project_file(args.kicad_project.as_deref(), &mut board, &transform);

    // ── 11. the optional session import (`:189-234`) ──────────────────────────────────────────
    import_session_file(args.ses.as_deref(), &mut board, &transform);

    job.started_at = Some(std::time::Instant::now());
    job.state = RoutingJobState::Running;
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
    let progress = SyncProgressSink::noop();
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
    let visualization = match args.visualize.as_ref() {
        Some(output_dir) => {
            let mut options = fr_router::RoutingVisualizationOptions::new(output_dir.to_path_buf());
            options.every = args.visualize_every;
            options.max_frames = args.visualize_max_frames;
            options.width = args.visualize_width;
            options.height = args.visualize_height;
            match fr_router::start_routing_visualization(options) {
                Ok(guard) => Some(guard),
                Err(error) => {
                    eprintln!("--visualize: {error}");
                    tracing::warn!("--visualize: {error}");
                    return ExitCode::UsageError;
                }
            }
        }
        None => None,
    };
    let result = match RoutingPipeline::run(&mut board, &ctx) {
        Ok(result) => result,
        Err(error) => {
            tracing::error!(
                "Failed to set up routing job '{}', it will be terminated.: {error}",
                job.id.to_java_string()
            );
            job.state = RoutingJobState::Terminated;
            job.finished_at = Some(std::time::Instant::now());
            return finish(&job, args, false, ExitCode::Failure, None);
        }
    };
    if let Some(visualization) = visualization {
        let summary = visualization.finish();
        if let Some(error) = summary.error {
            tracing::warn!("routing visualization stopped after an I/O error: {error}");
        }
        tracing::info!(
            frames = summary.frames_written,
            observed_steps = summary.observed_steps,
            capped = summary.reached_frame_limit,
            output = %summary.output_dir.display(),
            "routing visualization written"
        );
        eprintln!(
            "Visualization: {} SVG frames from {} routing steps in {}{}",
            summary.frames_written,
            summary.observed_steps,
            summary.output_dir.display(),
            if summary.reached_frame_limit {
                " (frame limit reached)"
            } else {
                ""
            }
        );
    }

    // `:174-184` — `finishedAt`, then the state finalisation. The port has no `STOPPING` arm to
    // take, because nothing outside this stack can request a stop, so the whole ladder reduces to
    // `RUNNING -> COMPLETED` unless the **job deadline** already made the state `TIMED_OUT`.
    job.finished_at = Some(std::time::Instant::now());
    job.set_current_pass(result.pipeline.router_passes_completed);
    job.set_optimizer_pass(result.pipeline.optimizer_passes_completed);
    job.state = if job_deadline.is_timed_out() {
        RoutingJobState::TimedOut
    } else {
        RoutingJobState::Completed
    };

    set_job_output(&mut job, &board, &transform);

    // ── 14-16. ───────────────────────────────────────────────────────────────────────────────
    let output_written = write_cli_output_if_available(&job, &args.output);
    let exit_code = compute_cli_exit_code(&job, output_written);
    finish(&job, args, output_written, exit_code, Some(&result.stats))
}

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

const WRITABLE_OUTPUT_FORMATS: [FileFormat; 2] = [FileFormat::Ses, FileFormat::KicadSessionJson];

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
        _ => None,
    };
    if let (Some(output), Some(format), Some(bytes)) = (job.output.as_mut(), format, bytes) {
        output.set_data(bytes, format);
    }
}

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
        &fr_core::now_utc_iso8601,
        stats,
    );
    if let Err(error) = RoutingResultManifest::write(Path::new(&path), &manifest) {
        tracing::error!("Couldn't write routing result manifest to '{path}': {error}");
    }
}

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

fn settings_budget(settings: &fr_settings::RouterSettings) -> fr_core::RouterBudget {
    let mut budget = fr_core::RouterBudget::default();
    if let Some(ms) = settings.opt_changed_area_ms {
        budget.opt_changed_area_ms = ms;
    }
    budget
}
