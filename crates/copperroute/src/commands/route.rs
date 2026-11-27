use std::path::Path;

use copper_core::{RoutingJobState, RoutingResultManifest, SyncProgressSink};

use crate::ExitCode;
use crate::cli::{Cli, RouteArgs};
use crate::ops::load::{BoardSource, LoadRequest};
use crate::ops::route::{OutputTarget, RouteOutcome, RouteRequest, route};

pub fn run(cli: &Cli, args: &RouteArgs) -> ExitCode {
    if args
        .output
        .extension()
        .is_some_and(|ext| ext == "kicad_pcb")
    {
        return route_to_kicad_board(args);
    }
    let mut set = args.set.clone();
    if let Some(passes) = args.max_passes {
        set.push(format!("router.max_passes={passes}"));
    }
    if let Some(timeout) = args.timeout.as_deref() {
        set.push(format!("router.job_timeout={timeout}"));
    }

    let mut load = LoadRequest::for_board(BoardSource::Path(args.input.clone()));
    load.rules = args.rules.clone();
    load.discover_adjacent_rules = true;
    load.session = args.ses.clone();
    load.kicad_project = args.kicad_project.clone();
    load.kicad_import = copper_dsn::kicad::pcb::ImportOptions {
        rip_up: args.rip_up,
        rip_up_nets: (!args.rip_up_nets.is_empty())
            .then(|| args.rip_up_nets.iter().cloned().collect()),
        respect_zones: args.respect_zones,
        route_tuned_nets: args.route_tuned_nets,
    };
    load.kicad_rules = args.kicad_rules.clone();
    load.settings = super::overrides(cli, &set);

    let visualize = args.visualize.as_ref().map(|dir| {
        let mut options = copper_router::RoutingVisualizationOptions::new(dir.clone());
        options.every = args.visualize_every;
        options.max_frames = args.visualize_max_frames;
        options.width = args.visualize_width;
        options.height = args.visualize_height;
        options
    });

    let outcome = match route(RouteRequest {
        load,
        output: OutputTarget::File(args.output.clone()),
        cancel: copper_core::CancelToken::new(),
        progress: SyncProgressSink::noop(),
        visualize,
    }) {
        Ok(outcome) => outcome,
        Err(error) => {
            tracing::error!("{error}");
            return ExitCode::Failure;
        }
    };

    for warning in &outcome.warnings {
        tracing::warn!("{warning}");
    }

    if let Some(summary) = outcome.visualization.as_ref() {
        if let Some(error) = summary.error.as_ref() {
            tracing::warn!("routing visualization stopped after an I/O error: {error}");
        }
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

    let written = write_session(&outcome, &args.output);
    let exit_code = if written
        && matches!(
            outcome.state,
            RoutingJobState::Completed | RoutingJobState::TimedOut
        ) {
        ExitCode::Ok
    } else {
        ExitCode::Failure
    };
    write_manifest(args, &outcome, written, exit_code);
    exit_code
}

fn read_optional(path: Option<&Path>) -> Result<Option<String>, String> {
    path.map(|path| {
        std::fs::read_to_string(path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))
    })
    .transpose()
}

/// Routes a KiCad board the way the KiCad plugin does and writes the routed board.
fn route_to_kicad_board(args: &RouteArgs) -> ExitCode {
    if args.input.extension().is_none_or(|ext| ext != "kicad_pcb") {
        tracing::error!("A .kicad_pcb output needs a .kicad_pcb input.");
        return ExitCode::Failure;
    }
    if args.ses.is_some()
        || args.rules.is_some()
        || !args.set.is_empty()
        || args.visualize.is_some()
    {
        tracing::warn!(
            "--ses, --rules, --set and --visualize do not apply to a .kicad_pcb output."
        );
    }
    let board_text = match std::fs::read_to_string(&args.input) {
        Ok(text) => text,
        Err(error) => {
            tracing::error!("Couldn't read '{}': {error}", args.input.display());
            return ExitCode::Failure;
        }
    };
    let project = copper_settings::resolve_kicad_project_path(
        args.kicad_project.as_deref(),
        Some(&args.input),
    );
    let rules = args
        .kicad_rules
        .clone()
        .or_else(|| Some(args.input.with_extension("kicad_dru")).filter(|path| path.is_file()));
    let (project_text, rules_text) = match (
        read_optional(project.as_deref()),
        read_optional(rules.as_deref()),
    ) {
        (Ok(project), Ok(rules)) => (project, rules),
        (Err(error), _) | (_, Err(error)) => {
            tracing::error!("{error}");
            return ExitCode::Failure;
        }
    };
    let timeout_seconds = match args.timeout.as_deref() {
        None => 300,
        Some(text) => match copper_core::parse_timespan(text) {
            Some(duration) => u32::try_from(duration.as_secs()).unwrap_or(u32::MAX),
            None => {
                tracing::error!("Invalid --timeout '{text}'.");
                return ExitCode::Failure;
            }
        },
    };
    let request = copper_kicad::RouteRequest {
        boardText: board_text.clone(),
        boardName: args.input.file_stem().map_or_else(
            || "board".to_string(),
            |stem| stem.to_string_lossy().into_owned(),
        ),
        projectText: project_text,
        rulesText: rules_text,
        ripUp: args.rip_up,
        ripUpNets: (!args.rip_up_nets.is_empty()).then(|| args.rip_up_nets.clone()),
        onlyNets: None,
        respectZones: args.respect_zones,
        routeTunedNets: args.route_tuned_nets,
        maxPasses: args.max_passes.unwrap_or(20).max(1),
        timeoutSeconds: timeout_seconds,
        optimize: true,
    };
    let result = match copper_kicad::route(&request, &mut |progress| {
        tracing::info!(
            "pass {}: {} connections left",
            progress.pass,
            progress.incomplete
        );
        false
    }) {
        Ok(result) => result,
        Err(error) => {
            tracing::error!("{error}");
            return ExitCode::Failure;
        }
    };
    for warning in &result.warnings {
        tracing::warn!("{warning}");
    }
    let routed =
        match copper_dsn::kicad::changes::apply_changes_to_text(&board_text, &result.changes) {
            Ok(text) => text,
            Err(error) => {
                tracing::error!("{error}");
                return ExitCode::Failure;
            }
        };
    if let Err(error) = std::fs::write(&args.output, routed) {
        tracing::error!(
            "Couldn't save the output file '{}': {error}",
            args.output.display()
        );
        return ExitCode::Failure;
    }
    tracing::info!(
        "Added {} tracks and {} vias, removed {} items; {} connections left, {} DRC findings.",
        result.changes.addTracks.len(),
        result.changes.addVias.len(),
        result.changes.removeIds.len(),
        result.incomplete,
        result.violations
    );
    ExitCode::Ok
}

fn write_session(outcome: &RouteOutcome, path: &Path) -> bool {
    if !matches!(
        outcome.state,
        RoutingJobState::Completed | RoutingJobState::TimedOut
    ) {
        return false;
    }
    if let Err(error) = std::fs::write(path, &outcome.session) {
        tracing::error!(
            "Couldn't save the output file '{}': {error}",
            path.display()
        );
        return false;
    }
    !outcome.session.is_empty()
}

fn write_manifest(args: &RouteArgs, outcome: &RouteOutcome, written: bool, exit_code: ExitCode) {
    let from_settings = outcome
        .job
        .router_settings
        .result_json_path
        .as_deref()
        .map(str::to_string)
        .filter(|path| !path.trim().is_empty());
    let Some(path) = from_settings.or_else(|| {
        args.result_json
            .as_deref()
            .map(|path| path.to_string_lossy().into_owned())
    }) else {
        return;
    };
    let manifest = RoutingResultManifest::from_job(
        &outcome.job,
        Some(&args.input),
        written,
        exit_code.code(),
        &copper_core::now_utc_iso8601,
        Some(&outcome.result.stats),
    );
    if let Err(error) = RoutingResultManifest::write(Path::new(&path), &manifest) {
        tracing::error!("Couldn't write routing result manifest to '{path}': {error}");
    }
}
