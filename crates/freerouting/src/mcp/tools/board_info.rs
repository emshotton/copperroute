//! `board_info` — spec §13's `{ dsn_path | dsn_text } → layers/nets/components summary`.
//!
//! **The same document `freerouting info` writes**, because both call
//! [`fr_core::summarise`] and neither builds a summary of its own — see that module for what the
//! answer contains and where each half comes from. The jar has neither surface.

use super::super::jsonrpc::RpcError;
use super::super::server::{ProgressWriter, State};
use fr_core::CancelToken;
use serde_json::Value;

/// Loads the board and answers [`fr_core::BoardSummary`].
///
/// **Reports no progress and does not poll the token**, deliberately: this is one parse and one
/// walk, and both are over long before a `notifications/cancelled` could reach the transport. The
/// drop-everything [`ProgressWriter`] means saying so costs nothing — see `tools`' module docs.
///
/// The load is `commands::info`'s and `commands::drc`'s: [`fr_core::load_board_if_needed`], the
/// **whole** loader, with `job.router_settings` still `new RouterSettings()`. See
/// `commands::info`'s module docs for why the settings ladder is not consulted on a summary path.
///
/// # Errors
///
/// [`RpcError::invalid_params`] for a bad argument, an unreadable file or a board that will not
/// load — the last carrying `BoardLoader`'s own message text.
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
