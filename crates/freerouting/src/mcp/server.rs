use super::jsonrpc::{METHOD_NOT_FOUND, Notification, Request, Response, RpcError};
use fr_core::CancelToken;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::Write;
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// MCP revision this server speaks, and it is **not** Java's.
///
/// Java hard-codes `"2024-11-05"` (`api/mcp/McpControllerV1.java:285`). Controller ruling AT: the
/// port is a new server rather than a re-implementation of that one, so it advertises the revision
/// it actually implements — `notifications/progress`, `notifications/cancelled` and `ping`
/// included, none of which Java answers. The first row of the delta table in
/// `crates/freerouting/README.md`.
pub const PROTOCOL_VERSION: &str = "2025-06-18";

/// The shared, `Mutex`-guarded stdout every writer in this module goes through.
///
/// Plan ruling 3's shape: one writer object, one lock, and *nothing below `crates/freerouting`
/// sees a thread*. A `dyn` object rather than a generic because [`ProgressWriter`] appears in
/// [`ToolHandler`]'s signature, and a tool must not be generic over the transport's writer.
pub type SharedWriter = Arc<Mutex<dyn Write + Send>>;

/// A registered tool's implementation.
///
/// **Closes the obligation that stood at `mcp/server.rs:9-32`.** The Plan 1 skeleton's handler was
/// `Box<dyn Fn(Value) -> Result<Value, RpcError> + Send>`: it saw only its arguments, returned
/// exactly one `Result`, and held no reference to the connection, so it could neither emit an
/// interim message nor observe one. Spec §13 requires both. What landed, in the order the
/// obligation asked for it:
///
/// | the obligation asked for | what a handler now receives |
/// |---|---|
/// | server state (`list_settings`, a future board cache) | `&State` — **shared**, not `&mut`; see [`State`] |
/// | a progress sink carrying the request's `progressToken` | `&ProgressWriter` |
/// | the request's cancellation flag | `&CancelToken` (`fr_core`, spec §10) |
/// | *multiple* messages for one request | the handler writes them itself, through the sink |
///
/// The one deviation from the obligation's own sketch is `&State` where it wrote `&mut State`:
/// [`super::stdio::run_with`] runs each `tools/call` on its own thread against one `Arc<State>`,
/// so a `&mut` would have to be a lock held for the whole of a routing run — exactly the "never a
/// lock" ruling AP warns about. A tool that needs to mutate owns the interior mutability, which is
/// what [`State::initialized`] does with an [`AtomicBool`].
///
/// `Send + Sync` because the handler is called from a tool thread and reached through an
/// `Arc<State>` shared with the main loop.
pub type ToolHandler = Box<
    dyn Fn(&State, Value, &ProgressWriter, &CancelToken) -> Result<Value, RpcError> + Send + Sync,
>;

/// Writes `notifications/progress` through the shared, `Mutex`-guarded stdout writer.
///
/// **A request with no `_meta.progressToken` gets a writer that drops everything**, so a tool never
/// branches on whether progress was asked for — it reports, and the transport decides whether
/// anybody is listening. That is the MCP rule (a receiver must not send progress notifications for
/// a request that did not carry a token) expressed once, here, instead of once per tool.
///
/// Java has none of this. Its bridge does one blocking HTTP round trip per line
/// (`Freerouting.java:749-776`) and its controller returns a single `JsonObject`
/// (`McpControllerV1.java:185-198`), so no interim message is expressible; a long route reports
/// nothing until it is finished. The `notifications/progress` row of the delta table in
/// `crates/freerouting/README.md`.
///
/// [`Clone`] because a tool that reports from *inside* a callback needs its own handle:
/// `route_board` installs an `fr_core::SyncProgressSink`, whose closure is `'static`, so it moves
/// a clone in. Two clones write through the same `Arc<Mutex<_>>` and carry the same token, which
/// is the same guarantee two clones of a [`CancelToken`] give.
#[derive(Clone)]
pub struct ProgressWriter {
    /// `None` is the drop-everything writer.
    inner: Option<ProgressTarget>,
}

#[derive(Clone)]
struct ProgressTarget {
    /// The request's `params._meta.progressToken`, echoed verbatim. MCP allows a string or an
    /// integer and the port does not care which: it is copied, never parsed.
    token: Value,
    writer: SharedWriter,
}

impl ProgressWriter {
    /// The drop-everything writer — what a `tools/call` with no `_meta.progressToken` gets, and
    /// what [`handle`] is given by every caller that is not a tool thread.
    pub fn disabled() -> ProgressWriter {
        ProgressWriter { inner: None }
    }

    /// A writer bound to `token`, or the drop-everything one when `token` is absent.
    pub fn new(token: Option<Value>, writer: SharedWriter) -> ProgressWriter {
        ProgressWriter {
            inner: token.map(|token| ProgressTarget { token, writer }),
        }
    }

    /// Whether anything this writer is handed will actually be written. A tool should **not**
    /// branch on it — it exists so a tool can skip *computing* an expensive progress payload.
    pub fn is_enabled(&self) -> bool {
        self.inner.is_some()
    }

    /// One `notifications/progress`. `total` and `message` are the spec's optional members and are
    /// omitted when `None` rather than sent as `null`.
    ///
    /// A write failure is **dropped**: the peer's stdin is gone, the next response write will see
    /// the same error and end the loop, and a tool that has to decide what to do about a broken
    /// pipe halfway through a routing run is a worse design than one that does not.
    pub fn progress(&self, progress: f64, total: Option<f64>, message: Option<&str>) {
        let Some(target) = self.inner.as_ref() else {
            return;
        };
        let mut params = json!({ "progressToken": target.token, "progress": progress });
        if let Some(total) = total {
            params["total"] = json!(total);
        }
        if let Some(message) = message {
            params["message"] = json!(message);
        }
        let _ = write_line(
            &target.writer,
            &Notification::new("notifications/progress", params),
        );
    }
}

/// Serialises `message` and writes it as **one line**: compact JSON, one `\n`, flushed.
///
/// Java bug: `Freerouting.java:770` — the bridge prints
/// `responseBody.replace("\r", "").replace("\n", "")`, stripping every carriage return and newline
/// from the response body with no re-escaping (quirk label M). It is survivable only because valid
/// JSON never contains a raw newline *inside* a string, and it visibly mangles the one response
/// Jersey pretty-prints — the `-32700` branch (`McpControllerV1.java:152` returns the `JsonObject`
/// itself rather than `.toString()`), which arrives as collapsed, double-spaced JSON. The port
/// emits compact JSON and never has a newline to strip, so the mangling has nothing to act on.
/// Not reproduced; the fifth Task 11 row of the delta table in `crates/freerouting/README.md`.
pub(super) fn write_line<T: serde::Serialize>(
    writer: &SharedWriter,
    message: &T,
) -> std::io::Result<()> {
    let text = serde_json::to_string(message).expect("an outbound message serializes");
    // The lock is held across the write **and** the flush, which is what makes a tool thread's
    // progress notification and the main thread's response whole lines rather than interleaved
    // halves of two lines. A poisoned lock means a writer panicked mid-line; there is no honest
    // recovery, so the line is dropped and the caller sees the same broken-pipe path.
    let Ok(mut guard) = writer.lock() else {
        return Err(std::io::Error::other(
            "the stdout writer's lock is poisoned",
        ));
    };
    guard.write_all(text.as_bytes())?;
    guard.write_all(b"\n")?;
    guard.flush()
}

#[derive(Debug, Clone)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

/// The server's registry and its lifecycle flag.
///
/// Shared as an `Arc<State>` by the main loop and every tool thread, so everything mutable here is
/// interior — see [`ToolHandler`] for why that is the shape rather than `&mut State`.
pub struct State {
    tools: BTreeMap<String, (ToolDef, ToolHandler)>,
    /// Whether `initialize` has been answered. Nothing branches on it yet: MCP says a server
    /// *should* reject other requests before initialization, and the port answers them instead —
    /// which is Java's behaviour too (`McpControllerV1.java:189-198` has no such guard), so the
    /// leniency is not a delta.
    pub initialized: AtomicBool,
    /// The **server process's own** raw argv, which is the command line every settings tier below
    /// priority 70 is built from — `--settings <file>` at 10 and `--router.*` at 60 (scan ruling
    /// R19: the *raw* argv, not a rewritten one).
    ///
    /// This is `globalSettings.settingsMergerProtype`'s content (`Freerouting.java:1408-1413`),
    /// which in Java is a field of the running JVM and here is a field of the running server.
    /// `freerouting mcp` carries no router flags, so the tier is usually empty — but it is the
    /// tier an operator reaches to configure a server, and a tool that ignored it would answer a
    /// different board from the CLI on the same machine.
    pub settings_argv: Vec<String>,
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
            initialized: AtomicBool::new(false),
            settings_argv: Vec::new(),
        }
    }

    /// [`State::new`] with the server's own argv recorded — see [`State::settings_argv`].
    pub fn with_settings_argv(settings_argv: &[String]) -> Self {
        Self {
            settings_argv: settings_argv.to_vec(),
            ..Self::new()
        }
    }

    /// Registration happens before the state is shared, so this is the one `&mut self` on the
    /// type. Task 12 registers the four spec §13 tools here
    /// (`super::tools::register_all`).
    pub fn register_tool(&mut self, def: ToolDef, handler: ToolHandler) {
        self.tools.insert(def.name.clone(), (def, handler));
    }
}

/// Answers one parsed request, or `None` for a notification.
///
/// `progress` and `cancel` belong to *this* request: [`super::stdio::run_with`] builds them per
/// `tools/call` and hands every other method the drop-everything writer and a fresh token.
///
/// renamed: McpControllerV1.handleMcpRequest -> this function (`McpControllerV1.java:185-198`),
/// whose method table is Java's four cases plus `ping`. Java answers `-32601` for `ping`
/// (`:197`, measured — `docs/plan-8-prep/evidence/job3-summary.md` §6); spec §Ping requires it, so
/// the port implements it. The fourth Task 11 row of the delta table in
/// `crates/freerouting/README.md`.
pub fn handle(
    state: &State,
    req: Request,
    progress: &ProgressWriter,
    cancel: &CancelToken,
) -> Option<Response> {
    let id = req.id.clone()?; // notifications: no id → no response
    let params = req.params.unwrap_or(Value::Null);
    let resp = match req.method.as_str() {
        "initialize" => {
            state.initialized.store(true, Ordering::SeqCst);
            Response::ok(
                id,
                // `McpControllerV1.handleInitialize:277-287` builds the same three members and
                // **two more**: non-spec top-level `serverName`/`serverVersion` (`:286-287`),
                // duplicates of `serverInfo.name`/`.version`. Not reproduced — a client that reads
                // them is reading something the MCP schema does not define. `capabilities` is
                // `{"tools": {}}` there (`:277-278`) and carries the explicit `listChanged: false`
                // here, because this server's tool list is fixed at compile time and saying so is
                // free. Rows one and two of the delta table.
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
                // Three outcomes, three shapes, and the split is deliberate:
                //
                // * `Ok` → a `result` with `isError: false`. Java's is one **stringified** text
                //   block and no `structuredContent` (`McpControllerV1.java:333-356`), despite
                //   every one of its tools declaring an `outputSchema`; the port emits both, so a
                //   client can read the value instead of re-parsing prose. Third row of the delta
                //   table.
                // * `Err` → a `result` with `isError: true`. A tool that *ran* and failed is a
                //   tool-level failure, which is what MCP's `isError` is for — Java agrees
                //   (`:354`, `isError = status >= 400`).
                // * a **panic** → a JSON-RPC `error` with `-32603`. A tool that unwound is a
                //   server fault, not a result, and flattening it into `isError` would tell a
                //   client the tool answered when it did not.
                Some((_, h)) => match invoke(state, h, args, progress, cancel) {
                    Ok(Ok(v)) => Response::ok(
                        id,
                        json!({
                            "content": [{ "type": "text", "text": v.to_string() }],
                            "structuredContent": v,
                            "isError": false,
                        }),
                    ),
                    Ok(Err(e)) => Response::ok(
                        id,
                        json!({
                            "content": [{ "type": "text", "text": e.message }],
                            "isError": true,
                        }),
                    ),
                    Err(e) => Response::from_error(id, e),
                },
            }
        }
        m => Response::err(id, METHOD_NOT_FOUND, format!("method not found: {m}")),
    };
    Some(resp)
}

/// **Plan-8 ruling 4's boundary: the single `catch_unwind` this plan adds, and the only one in
/// this crate.**
///
/// (`fr-router` has its own, and they are a different thing: plan-6 ruling 7's six sites and
/// plan-7 Task 14's reproduce Java `catch` blocks that a Java `Exception` would have landed in,
/// so each of those is a *port* of a Java recovery. This one ports nothing.)
///
/// The convention Plan 8 keeps is *no `catch_unwind`*: a panic is a bug, and swallowing it
/// hides the bug. Ruling 4 makes exactly one exception, at exactly this site, for one reason —
/// the alternative is worse. A tool runs on its own thread; without this, a panicking tool takes
/// that thread down, the `tools/call` never answers, and the client waits forever on a request
/// the server has already forgotten. With it, the client gets `-32603` and the *next* request is
/// answered, which `a_panicking_tool_becomes_an_error_and_the_server_survives` pins.
///
/// The scope is one call and nothing else: not the parse, not the write, not the loop. A panic
/// anywhere else in the transport still aborts the way every other panic in this port does.
///
/// [`AssertUnwindSafe`] is the honest annotation rather than a workaround: `&State` is shared and
/// its only mutable cell is an [`AtomicBool`], the arguments are moved in, and the writer's lock
/// reports poisoning to the next writer (see [`write_line`]) instead of pretending a half-written
/// line never happened. `std::panic::catch_unwind` is a safe function, so `#![forbid(unsafe_code)]`
/// has nothing to say about this.
fn invoke(
    state: &State,
    handler: &ToolHandler,
    args: Value,
    progress: &ProgressWriter,
    cancel: &CancelToken,
) -> Result<Result<Value, RpcError>, RpcError> {
    std::panic::catch_unwind(AssertUnwindSafe(|| handler(state, args, progress, cancel))).map_err(
        // `payload.as_ref()`, not `&payload`: `&Box<dyn Any + Send>` coerces by *unsizing the box*,
        // so the `Any` a downcast then sees is the `Box` itself and every payload looks unknown.
        |payload| {
            RpcError::internal(format!(
                "tool panicked: {}",
                panic_message(payload.as_ref())
            ))
        },
    )
}

/// The two payload types `panic!` produces — `&'static str` for a literal, `String` for a format —
/// and an honest word for anything else (`panic_any` with a custom type).
fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&'static str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "a panic payload of an unknown type".to_string()
    }
}

#[cfg(test)]
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

    /// The six skeleton tests call [`handle`] directly, so they supply the two per-request values
    /// the transport supplies in production: a drop-everything progress writer and a fresh token.
    fn answer(state: &State, req: Request) -> Option<Response> {
        handle(state, req, &ProgressWriter::disabled(), &CancelToken::new())
    }

    fn echo_tool() -> (ToolDef, ToolHandler) {
        (
            ToolDef {
                name: "echo".into(),
                description: "echo".into(),
                input_schema: json!({"type": "object"}),
            },
            Box::new(|_state, args, _progress, _cancel| Ok(args)),
        )
    }

    #[test]
    fn initialize_returns_server_info_and_tools_capability() {
        let st = State::new();
        let resp = answer(
            &st,
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
        // Java's two non-spec top-level keys (`McpControllerV1.java:286-287`) are absent.
        assert!(r.get("serverName").is_none() && r.get("serverVersion").is_none());
    }

    #[test]
    fn ping_returns_empty_object() {
        let st = State::new();
        let resp = answer(&st, req(2, "ping", json!({}))).unwrap();
        assert_eq!(resp.result.unwrap(), json!({}));
    }

    #[test]
    fn notifications_get_no_response() {
        let st = State::new();
        let n = Request {
            jsonrpc: "2.0".into(),
            id: None,
            method: "notifications/initialized".into(),
            params: None,
        };
        assert!(answer(&st, n).is_none());
    }

    #[test]
    fn tools_list_is_empty_by_default_and_lists_registered() {
        let mut st = State::new();
        let resp = answer(&st, req(3, "tools/list", json!({}))).unwrap();
        assert_eq!(resp.result.unwrap()["tools"], json!([]));
        let (def, handler) = echo_tool();
        st.register_tool(def, handler);
        let resp = answer(&st, req(4, "tools/list", json!({}))).unwrap();
        assert_eq!(resp.result.unwrap()["tools"][0]["name"], "echo");
    }

    #[test]
    fn tools_call_dispatches_and_wraps_content() {
        let mut st = State::new();
        let (def, handler) = echo_tool();
        st.register_tool(def, handler);
        let resp = answer(
            &st,
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
        let st = State::new();
        let resp = answer(&st, req(6, "nope", json!({}))).unwrap();
        assert_eq!(resp.error.unwrap().code, -32601);
        let resp = answer(
            &st,
            req(7, "tools/call", json!({"name": "missing", "arguments": {}})),
        )
        .unwrap();
        assert_eq!(resp.error.unwrap().code, -32602);
    }

    /// The three-way split at [`handle`]'s `tools/call` arm, in one test: a tool that answers, a
    /// tool that fails, and a tool that unwinds are three different shapes on the wire.
    #[test]
    fn a_tool_failure_is_an_is_error_result_and_a_tool_panic_is_a_minus_32603() {
        let mut st = State::new();
        st.register_tool(
            ToolDef {
                name: "fails".into(),
                description: "fails".into(),
                input_schema: json!({"type": "object"}),
            },
            Box::new(|_, _, _, _| Err(RpcError::invalid_params("no good"))),
        );
        st.register_tool(
            ToolDef {
                name: "panics".into(),
                description: "panics".into(),
                input_schema: json!({"type": "object"}),
            },
            Box::new(|_, _, _, _| panic!("boom")),
        );

        let failed = answer(&st, req(8, "tools/call", json!({"name": "fails"}))).unwrap();
        let r = failed.result.unwrap();
        assert_eq!(r["isError"], true);
        assert_eq!(r["content"][0]["text"], "no good");
        assert!(r.get("structuredContent").is_none());

        // The default hook would print the panic to stderr; the test silences it so the run's
        // output stays readable. Stdout is untouched either way — it is the protocol's.
        let hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let panicked = answer(&st, req(9, "tools/call", json!({"name": "panics"}))).unwrap();
        std::panic::set_hook(hook);
        assert!(panicked.result.is_none());
        let e = panicked.error.unwrap();
        assert_eq!(e.code, super::super::jsonrpc::INTERNAL_ERROR);
        assert!(e.message.contains("boom"), "{}", e.message);
    }

    /// The drop-everything writer is what a `tools/call` with no `_meta.progressToken` gets, and
    /// reporting into it is a no-op rather than a branch the tool has to take.
    #[test]
    fn a_progress_writer_with_no_token_drops_everything() {
        let sink: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
        let disabled = ProgressWriter::disabled();
        assert!(!disabled.is_enabled());
        disabled.progress(1.0, Some(2.0), Some("ignored"));

        let enabled = ProgressWriter::new(Some(json!("tok")), shared(Arc::clone(&sink)));
        assert!(enabled.is_enabled());
        enabled.progress(1.0, None, None);
        enabled.progress(2.0, Some(5.0), Some("half"));

        let text = String::from_utf8(sink.lock().unwrap().clone()).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2, "{text}");
        let first: Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(first["method"], "notifications/progress");
        assert_eq!(first["params"]["progressToken"], "tok");
        assert_eq!(first["params"]["progress"], 1.0);
        // `total`/`message` are omitted, not null.
        assert!(first["params"].get("total").is_none());
        assert!(first["params"].get("message").is_none());
        // A notification has no `id` member at all.
        assert!(first.get("id").is_none());
        let second: Value = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(second["params"]["total"], 5.0);
        assert_eq!(second["params"]["message"], "half");
    }

    /// `Arc<Mutex<Vec<u8>>>` as the module's [`SharedWriter`].
    fn shared(buffer: Arc<Mutex<Vec<u8>>>) -> SharedWriter {
        struct Tee(Arc<Mutex<Vec<u8>>>);
        impl Write for Tee {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(buf);
                Ok(buf.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        Arc::new(Mutex::new(Tee(buffer)))
    }
}
