use super::super::jsonrpc::RpcError;
use super::super::server::{ProgressWriter, State};
use fr_core::CancelToken;
use serde_json::Value;

pub fn run(
    _state: &State,
    args: Value,
    _progress: &ProgressWriter,
    _cancel: &CancelToken,
) -> Result<Value, RpcError> {
    let mut job = super::board_input(&args)?;
    let loaded = fr_core::load_board_if_needed(&mut job)
        .map_err(|error| RpcError::invalid_params(error.to_string()))?;
    let mut board = loaded.board;
    let summary = fr_core::summarise(&mut board, loaded.metadata.as_ref());
    serde_json::to_value(&summary).map_err(|error| {
        RpcError::internal(format!("the board summary would not serialize: {error}"))
    })
}
