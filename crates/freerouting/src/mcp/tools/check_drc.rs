use super::super::jsonrpc::RpcError;
use super::super::server::{ProgressWriter, State};
use crate::commands::drc::{load_rules_file, load_session_file, quality_score, report_date};
use fr_core::CancelToken;
use fr_drc::DesignRulesChecker;
use fr_drc::report::{DrcCoordinates, DrcJsonFlavor, DrcReportOptions};
use serde_json::Value;
use std::path::PathBuf;

pub fn run(
    state: &State,
    args: Value,
    _progress: &ProgressWriter,
    _cancel: &CancelToken,
) -> Result<Value, RpcError> {
    let ses = super::optional_string(&args, "ses_path")?.map(PathBuf::from);
    let rules = super::optional_string(&args, "rules_path")?.map(PathBuf::from);
    let mut job = super::board_input(&args)?;

    let loaded = fr_core::load_board_if_needed(&mut job)
        .map_err(|error| RpcError::invalid_params(error.to_string()))?;
    let mut board = loaded.board;
    let transform = loaded.transform;

    load_rules_file(rules.as_deref(), &job, &mut board, &transform);
    load_session_file(ses.as_deref(), &mut board, &transform);

    let coords = DrcCoordinates {
        board_unit: board.communication.unit,
        transform,
    };
    let options = DrcReportOptions {
        source: job.get_input().map_or_else(
            || "board.dsn".to_string(),
            |details| details.get_filename().to_string(),
        ),
        coordinate_unit: "mm".to_string(),
        date: report_date(std::time::SystemTime::now()),
        freerouting_version: fr_core::PARITY_VERSION.to_string(),
        quality_score: None,
    };
    let mut checker = DesignRulesChecker::new(&mut board);
    let mut report = checker.generate_report(&coords, &options);

    report.quality_score = job
        .get_input()
        .and_then(|details| quality_score(&mut board, details, &state.settings_argv))
        .map(f64::from);

    let json = report.to_json(DrcJsonFlavor::KiCad).map_err(|error| {
        RpcError::internal(format!("Couldn't serialise the DRC report: {error}"))
    })?;
    serde_json::from_str(&json).map_err(|error| {
        RpcError::internal(format!("the DRC report is not readable as JSON: {error}"))
    })
}
