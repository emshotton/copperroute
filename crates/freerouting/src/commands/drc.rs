
use std::path::Path;

use fr_core::{BoardFileDetails, BoardStatistics, FileFormat, RoutingJob, SessionId};
use fr_drc::DesignRulesChecker;
use fr_drc::report::{DrcCoordinates, DrcJsonFlavor, DrcReportOptions};
use fr_settings::sources::{
    DefaultSettings, DsnFileSettings, EnvironmentVariablesSource, JsonFileSettings,
};
use fr_settings::{HostEnvironment, RouterSettings, SettingsMerger, SettingsSource};

use crate::cli::{DrcArgs, DrcSchema};
use crate::legacy::ExitCode;

pub fn run(args: &DrcArgs, settings_argv: &[String]) -> ExitCode {

    let mut job = RoutingJob::new(SessionId::NIL);
    job.drc = args.output.as_deref().map(report_file_details);

    tracing::info!("Loading DSN file for DRC: {}", args.input.display());
    if let Err(error) = job.set_input(&args.input) {
        tracing::error!(
            "Couldn't load the input file '{}': {error}",
            args.input.display()
        );
        return ExitCode::Failure;
    }

    let loaded = match fr_core::load_board_if_needed(&mut job) {
        Ok(loaded) => loaded,
        Err(error) => {
            tracing::error!("{error}");
            tracing::error!("Failed to load board for DRC check");
            return ExitCode::Failure;
        }
    };
    let mut board = loaded.board;
    let transform = loaded.transform;
    for warning in &loaded.warnings {
        tracing::warn!("{warning}");
    }

    load_rules_file(args.rules.as_deref(), &job, &mut board, &transform);

    load_session_file(args.ses.as_deref(), &mut board, &transform);

    let coords = DrcCoordinates {
        board_unit: board.communication.unit,
        transform,
    };
    let options = DrcReportOptions {
        source: base_name(&args.input),
        coordinate_unit: "mm".to_string(),
        date: report_date(std::time::SystemTime::now()),
        freerouting_version: fr_core::PARITY_VERSION.to_string(),
        quality_score: None,
    };
    let mut checker = DesignRulesChecker::new(&mut board);
    let mut report = checker.generate_report(&coords, &options);

    report.quality_score = job
        .get_input()
        .and_then(|input| quality_score(&mut board, input, settings_argv))
        .map(f64::from);

    let flavor = match args.schema {
        DrcSchema::Kicad => DrcJsonFlavor::KiCad,
        DrcSchema::Freerouting => DrcJsonFlavor::FreeroutingHead,
    };
    let json = match report.to_json(flavor) {
        Ok(json) => json,
        Err(error) => {
            tracing::error!("Couldn't serialise the DRC report: {error}");
            return ExitCode::Failure;
        }
    };

    write_report(job.drc.as_ref(), &json)
}

fn report_file_details(path: &Path) -> BoardFileDetails {
    let mut details = BoardFileDetails::default();
    details.format = FileFormat::DrcJson;
    details.set_filename(Some(&path.to_string_lossy()));
    details
}

pub(crate) fn base_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

pub fn load_rules_file(
    rules: Option<&Path>,
    job: &RoutingJob,
    board: &mut fr_board::Board,
    transform: &fr_dsn::CoordinateTransform,
) {
    let Some(rules) = rules else {
        return;
    };
    if !rules.exists() {
        // `:289` — `FRLogger.warn("RULES file for DRC not found: " + …)`, and the run continues to
        tracing::warn!("RULES file for DRC not found: {}", rules.display());
        return;
    }
    tracing::info!("Loading RULES file for DRC: {}", rules.display());
    let design_name = job.name.clone();
    let file = match std::fs::File::open(rules) {
        Ok(file) => file,
        Err(error) => {
            tracing::error!("Failed to load RULES file for DRC: {error}");
            return;
        }
    };
    match fr_dsn::rules_reader::read(file, &design_name, board, transform, None) {
        Ok(_) => {
            tracing::info!("RULES file loaded for DRC successfully");
        }
        Err(error) => {
            tracing::error!("Failed to load RULES file for DRC: {error}");
        }
    }
}

pub fn load_session_file(
    session: Option<&Path>,
    board: &mut fr_board::Board,
    transform: &fr_dsn::CoordinateTransform,
) {
    let Some(session) = session else {
        return;
    };
    if !session.exists() {
        tracing::warn!("Session file for DRC not found: {}", session.display());
        return;
    }
    if session.to_string_lossy().to_lowercase().ends_with(".json") {
        tracing::info!(
            "Loading KiCad JSON session file for DRC: {}",
            session.display()
        );
        let bytes = match std::fs::read(session) {
            Ok(bytes) => bytes,
            Err(error) => {
                tracing::error!("Failed to load session file for DRC: {error}");
                return;
            }
        };
        match fr_dsn::kicad::import_session(&String::from_utf8_lossy(&bytes), board) {
            Ok(()) => {
                tracing::info!("KiCad JSON session file loaded for DRC successfully");
            }
            Err(error) => {
                tracing::error!("Failed to load session file for DRC: {error}");
            }
        }
        return;
    }
    tracing::info!("Loading SES file for DRC: {}", session.display());
    let file = match std::fs::File::open(session) {
        Ok(file) => file,
        Err(error) => {
            tracing::error!("Failed to load session file for DRC: {error}");
            return;
        }
    };
    match fr_dsn::ses_reader::read(file, board, transform) {
        Ok(summary) => {
            let errors = if summary.errors_encountered > 0 {
                format!(" ({} errors)", summary.errors_encountered)
            } else {
                String::new()
            };
            tracing::info!(
                "SES file loaded for DRC: {} wires, {} vias imported{errors}",
                summary.wires_imported,
                summary.vias_imported
            );
        }
        Err(error) => {
            tracing::error!("Failed to load session file for DRC: {error}");
        }
    }
}

pub fn quality_score(
    board: &mut fr_board::Board,
    input: &BoardFileDetails,
    settings_argv: &[String],
) -> Option<f32> {
    let router_settings = quality_score_settings(input, settings_argv);

    let stats = BoardStatistics::new(board);

    let Some(scoring) = router_settings.scoring.as_ref() else {
        tracing::warn!(
            "Failed to calculate quality score for DRC report: routerSettings.scoring is null"
        );
        return None;
    };
    Some(stats.normalized_score(scoring))
}

#[must_use]
pub fn quality_score_settings(
    input: &BoardFileDetails,
    settings_argv: &[String],
) -> RouterSettings {
    let host = HostEnvironment::detect();

    let mut sources: Vec<Box<dyn SettingsSource>> = vec![Box::new(DefaultSettings::new(&host))];
    if let Some(path) = super::json_settings_path(settings_argv) {
        sources.push(Box::new(JsonFileSettings::new(&path)));
    }
    sources.push(Box::new(super::cli_settings(settings_argv)));
    let environment: std::collections::BTreeMap<String, String> = std::env::vars().collect();
    sources.push(Box::new(EnvironmentVariablesSource::new(&environment)));
    let mut merger = SettingsMerger::new(sources);

    merger.add_or_replace_sources(vec![Box::new(DsnFileSettings::new(
        input.get_data(),
        input.get_filename(),
    ))]);

    merger.merge(&host)
}

fn write_report(drc: Option<&BoardFileDetails>, json: &str) -> ExitCode {
    let Some(drc) = drc else {
        println!("{json}");
        return ExitCode::Ok;
    };
    let output_file_name = drc.get_absolute_path();
    match std::fs::write(&output_file_name, json.as_bytes()) {
        Ok(()) => {
            tracing::info!("DRC report written to: {output_file_name}");
            ExitCode::Ok
        }
        Err(error) => {
            tracing::error!("Couldn't save the DRC report to '{output_file_name}': {error}");
            ExitCode::Failure
        }
    }
}

/// is a dependency (forbidden) or a `libc` call (`#![forbid(unsafe_code)]`). `Z` is a valid
pub(crate) fn report_date(time: std::time::SystemTime) -> String {
    let instant = fr_core::format_utc_iso8601(time);
    let body = instant.strip_suffix('Z').unwrap_or(&instant);
    let body = match body.split_once('.') {
        Some((head, fraction)) => {
            let trimmed = fraction.trim_end_matches('0');
            if trimmed.is_empty() {
                head.to_string()
            } else {
                format!("{head}.{trimmed}")
            }
        }
        None => body.to_string(),
    };
    let body = match body.strip_suffix(":00") {
        Some(without_seconds) if !body.contains('.') => without_seconds.to_string(),
        _ => body,
    };
    format!("{body}Z")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, UNIX_EPOCH};

                            #[test]
    fn the_date_is_iso_offset_date_time_not_iso_instant() {
        let base = 1_756_800_000u64; 
        for (nanos, expected) in [
            (0u32, "2025-09-02T08:00Z"),
            (100_000_000, "2025-09-02T08:00:00.1Z"),
            (120_000_000, "2025-09-02T08:00:00.12Z"),
            (123_000_000, "2025-09-02T08:00:00.123Z"),
            (406_471_000, "2025-09-02T08:00:00.406471Z"),
            (1, "2025-09-02T08:00:00.000000001Z"),
        ] {
            assert_eq!(
                report_date(UNIX_EPOCH + Duration::new(base, nanos)),
                expected,
                "nanos {nanos}"
            );
        }
        assert_eq!(
            report_date(UNIX_EPOCH + Duration::new(base + 7, 0)),
            "2025-09-02T08:00:07Z"
        );
    }

        #[test]
    fn the_source_is_the_inputs_base_name() {
        assert_eq!(base_name(Path::new("/a/b/board.dsn")), "board.dsn");
        assert_eq!(base_name(Path::new("board.dsn")), "board.dsn");
    }

            #[test]
    fn the_report_path_is_the_argument_rejoined() {
        assert_eq!(
            report_file_details(Path::new("/a/b/r.json")).get_absolute_path(),
            "/a/b/r.json"
        );
        assert_eq!(
            report_file_details(Path::new("r.json")).get_absolute_path(),
            "r.json"
        );
        assert_eq!(
            report_file_details(Path::new("/a/b/r.json")).format,
            FileFormat::DrcJson
        );
    }
}
