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
// Reserved for stricter request validation: nothing rejects a malformed *envelope* today, because
// `serde_json::from_str::<Request>` either produces a `Request` or fails as a `-32700`.
#[allow(dead_code)]
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
    /// Consumed by Task 12's tools; a tool that rejects its arguments answers this.
    #[allow(dead_code)]
    pub fn invalid_params(msg: impl Into<String>) -> Self {
        Self {
            code: INVALID_PARAMS,
            message: msg.into(),
            data: None,
        }
    }
    /// Ruling 4's rendering of a panic payload, and the only in-tree caller is that boundary.
    pub fn internal(msg: impl Into<String>) -> Self {
        Self {
            code: INTERNAL_ERROR,
            message: msg.into(),
            data: None,
        }
    }
}
