use fr_board::Board;
use fr_core::{BoardStatistics, RoutingJob};
use fr_drc::DesignRulesChecker;
use fr_drc::report::{DrcCoordinates, DrcJsonFlavor, DrcReportOptions, KiCadDrcReport};
use fr_settings::sources::DsnFileSettings;
use fr_settings::{HostEnvironment, SettingsSource};

use super::OpError;
use super::load::{LoadRequest, Loaded, load};
use super::settings::{self, SettingsOverrides};

pub struct DrcRequest {
    pub load: LoadRequest,
    pub flavor: DrcJsonFlavor,
    pub date: String,
}

#[derive(Debug)]
pub struct DrcOutcome {
    pub report: KiCadDrcReport,
    pub json: String,
    pub violation_count: usize,
}

pub fn drc(request: &DrcRequest) -> Result<DrcOutcome, OpError> {
    let Loaded {
        job,
        mut board,
        transform,
        ..
    } = load(&request.load)?;
    let source = job.get_input().map_or_else(
        || "board.dsn".to_string(),
        |input| input.get_filename().to_string(),
    );

    let coords = DrcCoordinates {
        board_unit: board.communication.unit,
        transform,
    };
    let options = DrcReportOptions {
        source,
        coordinate_unit: "mm".to_string(),
        date: request.date.clone(),
        freerouting_version: env!("CARGO_PKG_VERSION").to_string(),
        quality_score: None,
    };
    let mut report = DesignRulesChecker::new(&mut board).generate_report(&coords, &options);
    report.quality_score = quality_score(&mut board, &job, &request.load.settings).map(f64::from);

    let json = report
        .to_json(request.flavor)
        .map_err(|error| OpError::Load(format!("Couldn't serialise the DRC report: {error}")))?;
    let violation_count = report.violations.len();
    Ok(DrcOutcome {
        report,
        json,
        violation_count,
    })
}

fn quality_score(
    board: &mut Board,
    job: &RoutingJob,
    overrides: &SettingsOverrides,
) -> Option<f32> {
    let input = job.get_input()?;
    let dsn = DsnFileSettings::new(input.get_data(), input.get_filename());
    let settings = settings::resolve(
        overrides,
        dsn.get_settings(),
        None,
        None,
        None,
        &HostEnvironment::detect(),
    )
    .ok()?;
    let scoring = settings.scoring.as_ref()?;
    Some(BoardStatistics::new(board).normalized_score(scoring))
}

pub fn report_date(time: std::time::SystemTime) -> String {
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
