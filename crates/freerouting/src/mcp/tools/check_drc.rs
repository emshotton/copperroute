use super::super::jsonrpc::RpcError;
use super::super::server::{ProgressWriter, State};
use crate::ops::drc::{DrcRequest, drc, report_date};
use crate::ops::load::LoadRequest;
use fr_core::CancelToken;
use fr_drc::report::DrcJsonFlavor;
use serde_json::Value;
use std::path::PathBuf;

pub fn run(
    state: &State,
    args: Value,
    _progress: &ProgressWriter,
    _cancel: &CancelToken,
) -> Result<Value, RpcError> {
    let mut load = LoadRequest::for_board(super::board_source(&args)?);
    load.session = super::optional_string(&args, "ses_path")?.map(PathBuf::from);
    load.rules = super::optional_string(&args, "rules_path")?.map(PathBuf::from);
    load.kicad_project = super::optional_string(&args, "kicad_project_path")?.map(PathBuf::from);
    load.settings = state.overrides.clone();

    let outcome = drc(&DrcRequest {
        load,
        flavor: DrcJsonFlavor::KiCad,
        date: report_date(std::time::SystemTime::now()),
    })
    .map_err(super::rpc_error)?;
    serde_json::from_str(&outcome.json).map_err(|error| {
        RpcError::internal(format!("the DRC report is not readable as JSON: {error}"))
    })
}
