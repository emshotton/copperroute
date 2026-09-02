use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Deserialize)]
pub struct Request {
    // Present for protocol fidelity; not currently validated by `server::handle`.
    #[allow(dead_code)]
    pub jsonrpc: String,
    #[serde(default)]
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Response {
    pub jsonrpc: &'static str,
    pub id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

/// An outbound message with **no `id`** — the half of JSON-RPC 2.0 the skeleton never needed.
///
/// Plan 8 Task 11 adds one user of it, `notifications/progress` (MCP spec §Progress), written by
/// [`super::server::ProgressWriter`] from a tool thread while its `tools/call` is still running.
/// It is deliberately *not* a [`Response`] with a null id: a notification has no `id` **member**
/// at all, and [`Response::id`] is not an [`Option`].
///
/// Java has no counterpart. Its bridge is a request/response pump with one HTTP round trip per
/// line (`Freerouting.java:738-776`), so nothing can reach stdout between a request and its
/// response — the `notifications/progress` row of the delta table in
/// `crates/freerouting/README.md`.
#[derive(Debug, Clone, Serialize)]
pub struct Notification {
    pub jsonrpc: &'static str,
    pub method: &'static str,
    pub params: Value,
}

impl Notification {
    pub fn new(method: &'static str, params: Value) -> Self {
        Self {
            jsonrpc: "2.0",
            method,
            params,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

pub const PARSE_ERROR: i64 = -32700;
/// A well-formed envelope the server will not act on. One thing reaches it: a `tools/call` whose
/// id is **already in flight**, which MCP's id-uniqueness rule forbids and this transport's
/// cancellation map cannot represent — see [`super::stdio`]'s `spawn_tool_call`.
pub const INVALID_REQUEST: i64 = -32600;
pub const METHOD_NOT_FOUND: i64 = -32601;
pub const INVALID_PARAMS: i64 = -32602;
/// Ruling 4's code: the one thing that reaches it is a tool that **unwound**, rendered by
/// [`RpcError::internal`] at [`super::server::handle`]'s single `catch_unwind` site — the one
/// boundary Plan 8 adds, and the only one in this crate.
pub const INTERNAL_ERROR: i64 = -32603;

impl Response {
    pub fn ok(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: Some(result),
            error: None,
        }
    }
    pub fn err(id: Value, code: i64, message: impl Into<String>) -> Self {
        Self::from_error(
            id,
            RpcError {
                code,
                message: message.into(),
                data: None,
            },
        )
    }
    /// [`Response::err`] from an already-built [`RpcError`] — what ruling 4's boundary needs, so
    /// that a panic's `-32603` reaches the wire as a JSON-RPC **error** rather than being flattened
    /// into a tool-level `isError` result. See [`super::server::handle`]'s three-way split.
    pub fn from_error(id: Value, error: RpcError) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(error),
        }
    }
}

impl RpcError {
    /// A tool that rejects its arguments answers this; Task 12's four are its production callers,
    /// and `mcp::server`'s three-way-split test is its caller today.
    pub fn invalid_params(msg: impl Into<String>) -> Self {
        Self {
            code: INVALID_PARAMS,
            message: msg.into(),
            data: None,
        }
    }
    /// Ruling 4's rendering of a panic payload. ~~"and the only in-tree caller is that
    /// boundary"~~ — true when Task 11 wrote it, and false since Task 12 gave the four tools
    /// their bodies: `mcp/tools/route_board.rs` (×3), `check_drc.rs` (×2), `board_info.rs` and
    /// `list_settings.rs` all answer a failure with it, which is the intended use — a tool that
    /// cannot do its job reports `-32603` rather than panicking into the boundary.
    pub fn internal(msg: impl Into<String>) -> Self {
        Self {
            code: INTERNAL_ERROR,
            message: msg.into(),
            data: None,
        }
    }
}
