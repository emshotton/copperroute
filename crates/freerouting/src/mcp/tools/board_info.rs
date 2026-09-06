use super::super::jsonrpc::RpcError;
use super::super::server::{ProgressWriter, State};
use crate::ops::info::{InfoRequest, info};
use crate::ops::load::LoadRequest;
use fr_core::CancelToken;
use serde_json::Value;

pub fn run(
    state: &State,
    args: Value,
    _progress: &ProgressWriter,
    _cancel: &CancelToken,
) -> Result<Value, RpcError> {
    let mut load = LoadRequest::for_board(super::board_source(&args)?);
    load.settings = state.overrides.clone();
    let summary = info(&InfoRequest { load }).map_err(super::rpc_error)?;
    serde_json::to_value(&summary).map_err(|error| {
        RpcError::internal(format!("the board summary would not serialize: {error}"))
    })
}
