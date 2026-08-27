use super::jsonrpc::{METHOD_NOT_FOUND, Request, Response, RpcError};
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub const PROTOCOL_VERSION: &str = "2025-06-18";

pub type ToolHandler = Box<dyn Fn(Value) -> Result<Value, RpcError> + Send>;

#[derive(Debug, Clone)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

pub struct State {
    tools: BTreeMap<String, (ToolDef, ToolHandler)>,
    pub initialized: bool,
}

impl Default for State {
    fn default() -> Self {
        Self::new()
    }
}

impl State {
    pub fn new() -> Self {
        Self {
            tools: BTreeMap::new(),
            initialized: false,
        }
    }
    // Consumed by later plans registering real tools; exercised here by tests.
    #[allow(dead_code)]
    pub fn register_tool(&mut self, def: ToolDef, handler: ToolHandler) {
        self.tools.insert(def.name.clone(), (def, handler));
    }
}

pub fn handle(state: &mut State, req: Request) -> Option<Response> {
    let id = req.id.clone()?; // notifications: no id → no response
    let params = req.params.unwrap_or(Value::Null);
    let resp = match req.method.as_str() {
        "initialize" => {
            state.initialized = true;
            Response::ok(
                id,
                json!({
                    "protocolVersion": PROTOCOL_VERSION,
                    "capabilities": { "tools": { "listChanged": false } },
                    "serverInfo": { "name": "freerouting", "version": env!("CARGO_PKG_VERSION") },
                }),
            )
        }
        "ping" => Response::ok(id, json!({})),
        "tools/list" => {
            let tools: Vec<Value> = state
                .tools
                .values()
                .map(|(d, _)| {
                    json!({
                        "name": d.name, "description": d.description, "inputSchema": d.input_schema,
                    })
                })
                .collect();
            Response::ok(id, json!({ "tools": tools }))
        }
        "tools/call" => {
            let name = params.get("name").and_then(Value::as_str).unwrap_or("");
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            match state.tools.get(name) {
                None => Response::err(
                    id,
                    super::jsonrpc::INVALID_PARAMS,
                    format!("unknown tool: {name}"),
                ),
                Some((_, h)) => match h(args) {
                    Ok(v) => Response::ok(
                        id,
                        json!({
                            "content": [{ "type": "text", "text": v.to_string() }],
                            "structuredContent": v,
                            "isError": false,
                        }),
                    ),
                    Err(e) => Response::ok(
                        id,
                        json!({
                            "content": [{ "type": "text", "text": e.message }],
                            "isError": true,
                        }),
                    ),
                },
            }
        }
        m => Response::err(id, METHOD_NOT_FOUND, format!("method not found: {m}")),
    };
    Some(resp)
}

#[cfg(test)]
#[allow(clippy::redundant_closure)]
mod tests {
    use super::*;
    use serde_json::json;

    fn req(id: i64, method: &str, params: serde_json::Value) -> Request {
        Request {
            jsonrpc: "2.0".into(),
            id: Some(json!(id)),
            method: method.into(),
            params: Some(params),
        }
    }

    #[test]
    fn initialize_returns_server_info_and_tools_capability() {
        let mut st = State::new();
        let resp = handle(
            &mut st,
            req(
                1,
                "initialize",
                json!({"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "t", "version": "0"}}),
            ),
        )
        .unwrap();
        let r = resp.result.unwrap();
        assert_eq!(r["protocolVersion"], PROTOCOL_VERSION);
        assert_eq!(r["serverInfo"]["name"], "freerouting");
        assert!(r["capabilities"]["tools"].is_object());
    }

    #[test]
    fn ping_returns_empty_object() {
        let mut st = State::new();
        let resp = handle(&mut st, req(2, "ping", json!({}))).unwrap();
        assert_eq!(resp.result.unwrap(), json!({}));
    }

    #[test]
    fn notifications_get_no_response() {
        let mut st = State::new();
        let n = Request {
            jsonrpc: "2.0".into(),
            id: None,
            method: "notifications/initialized".into(),
            params: None,
        };
        assert!(handle(&mut st, n).is_none());
    }

    #[test]
    fn tools_list_is_empty_by_default_and_lists_registered() {
        let mut st = State::new();
        let resp = handle(&mut st, req(3, "tools/list", json!({}))).unwrap();
        assert_eq!(resp.result.unwrap()["tools"], json!([]));
        st.register_tool(
            ToolDef {
                name: "echo".into(),
                description: "echo".into(),
                input_schema: json!({"type": "object"}),
            },
            Box::new(|v| Ok(v)),
        );
        let resp = handle(&mut st, req(4, "tools/list", json!({}))).unwrap();
        assert_eq!(resp.result.unwrap()["tools"][0]["name"], "echo");
    }

    #[test]
    fn tools_call_dispatches_and_wraps_content() {
        let mut st = State::new();
        st.register_tool(
            ToolDef {
                name: "echo".into(),
                description: "echo".into(),
                input_schema: json!({"type": "object"}),
            },
            Box::new(|v| Ok(v)),
        );
        let resp = handle(
            &mut st,
            req(
                5,
                "tools/call",
                json!({"name": "echo", "arguments": {"a": 1}}),
            ),
        )
        .unwrap();
        let r = resp.result.unwrap();
        assert_eq!(r["isError"], false);
        assert_eq!(r["structuredContent"], json!({"a": 1}));
        assert_eq!(r["content"][0]["type"], "text");
    }

    #[test]
    fn unknown_method_and_unknown_tool_error() {
        let mut st = State::new();
        let resp = handle(&mut st, req(6, "nope", json!({}))).unwrap();
        assert_eq!(resp.error.unwrap().code, -32601);
        let resp = handle(
            &mut st,
            req(7, "tools/call", json!({"name": "missing", "arguments": {}})),
        )
        .unwrap();
        assert_eq!(resp.error.unwrap().code, -32602);
    }
}
