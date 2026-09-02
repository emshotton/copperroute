use super::jsonrpc::{PARSE_ERROR, Request, Response};
use super::server::{ProgressWriter, SharedWriter, State, handle, write_line};
use fr_core::CancelToken;
use serde_json::Value;
use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

/// Newline-delimited JSON-RPC over stdin/stdout. Returns the process exit code.
///
/// renamed: Freerouting.startMcpStdioBridge -> [`run_with`] (`Freerouting.java:681-788`). Java's
/// bridge is a *pump*: a daemon thread that reads a line, POSTs it to a Jetty server it started in
/// the same JVM (`:749-766`), and prints the HTTP response body. The port has no server to POST to
/// — controller ruling AO replaced the HTTP MCP transport with this one — so what the bridge's
/// responsibilities became is:
///
/// | `Freerouting.java` | here |
/// |---|---|
/// | the reader thread `:682-684` | [`run_with`]'s reader thread, which parses nothing and only ferries lines |
/// | the port hand-shake spin `:688-700` | gone; there is no listener |
/// | `while ((line = reader.readLine()) != null)` `:738` | the reader thread's `reader.lines()` |
/// | `if (line.trim().isEmpty()) continue;` `:739-741` | kept verbatim — and see the blank-line row of the delta table |
/// | the HTTP round trip `:743-766` | [`super::server::handle`], in process |
/// | `replace("\r","").replace("\n","")` `:770` | **not reproduced** (quirk label M) — see [`write_line`] |
/// | `originalOut.println` + `flush` `:771-772` | [`write_line`] |
/// | EOF ⇒ `System.exit(0)` `:778-779` | `0` |
/// | `IOException` ⇒ `System.exit(1)` `:780-782` | `1` |
///
/// not ported: Freerouting.stopMcpServer — the Jetty shutdown that pairs with `initializeMCP`,
/// itself `not ported:` in `src/main.rs`.
pub fn run() -> i32 {
    // `stdin()`/`stdout()` rather than their `lock()`s: a `StdinLock`/`StdoutLock` borrows the
    // handle and is `!Send`, and the reader thread and the tool threads need owned, sendable
    // halves. Each handle carries its own internal lock, so the two are as exclusive as the
    // guards would have been.
    run_with(
        State::new(),
        std::io::BufReader::new(std::io::stdin()),
        std::io::stdout(),
    )
}

/// What the main loop selects on. One channel rather than two, because `std::sync::mpsc` has no
/// `select` — a tool thread announcing its own completion and the reader thread announcing a line
/// are the same kind of event to this loop, so they travel the same wire.
enum Event {
    /// One line read from the peer, not yet parsed. Parsing is the **main thread's** job: a
    /// reader thread that parses is a reader thread that can be blocked by a pathological
    /// document while a `notifications/cancelled` waits behind it.
    Line(String),
    /// `reader.lines()` ended — `Freerouting.java:778`, `System.exit(0)`.
    Eof,
    /// `reader.lines()` failed — `:780-782`, `System.exit(1)`. Java's is an `IOException` on
    /// `System.in`; the port's is any [`std::io::Error`], invalid UTF-8 included.
    ReadFailed,
    /// A tool thread has written its response and is about to end. Carries the in-flight key so
    /// the main loop can forget the request's [`CancelToken`].
    ToolDone(String),
}

/// **Closes the obligation that stood at `mcp/stdio.rs:18-28`.** Ruling 3's shape, exactly:
///
/// * **one reader thread** ([`std::thread::spawn`], [`std::sync::mpsc::Sender<Event>`]) that does
///   nothing but ferry lines, so stdin is never blocked by whatever the last request is doing;
/// * **the main thread** dispatching — it owns the in-flight map, and it is the only thread that
///   ever inserts into or removes from it;
/// * **one tool thread per in-flight `tools/call`**, each holding a clone of the request's
///   [`CancelToken`], so an inbound `notifications/cancelled` flips the flag **while** the tool
///   runs. Under the skeleton's strictly sequential loop the cancellation was not even read until
///   the routing call had returned, which made it useless — that was the whole of the obligation.
/// * **the writer is an `Arc<Mutex<W>>`**, shared by the main thread and every tool thread, and
///   held across write-and-flush so a line is never half a line ([`write_line`]).
///
/// **Nothing below `crates/freerouting` sees a thread** (plan ruling 3). `fr-router` is
/// [`std::cell::Cell`]-based and single-threaded; what crosses into it is
/// `fr_core::CancelToken::as_router_stop`'s two atomics, through controller ruling BB's poll seam.
///
/// # Cancellation is three-state, and the MCP only ever issues one of them
///
/// `notifications/cancelled` carries a `requestId`; the main loop looks it up and calls
/// [`CancelToken::cancel`] — Java's `requestStop()`, i.e. `ALL`
/// (`core/StoppableThread.java:23-25`). It never calls `CancelToken::cancel_auto_router`, and that
/// arm is **not dead**: it is `AUTO_ROUTER_ONLY`, which `--max-passes` reaches
/// (`AutorouteBatchLoop.java:268-273`) where `--max-items` reaches `ALL`
/// (`AutoroutePassRunner.java:219`). Quirk #200/#202 rides on the difference — `ALL` silently
/// disables the optimizer stage through `RoutingPipeline.java:117` and `AUTO_ROUTER_ONLY` does not
/// — so collapsing the two would change a routed board without changing a test. Ruling 2 is why
/// the token carries two atomics; this paragraph is why a reader must not delete the arm the MCP
/// happens not to use.
///
/// # Shutdown
///
/// EOF cancels every in-flight call and then **drains**: the loop keeps running until the last
/// tool thread has reported, so a response already being written is not truncated. Java's
/// `System.exit(0)` (`:779`) kills its daemon thread mid-write instead; the port's drain is a
/// deliberate improvement over a shutdown that can lose a line, and it is bounded by the tools
/// observing the cancellation the drain begins with.
pub fn run_with<R: BufRead + Send + 'static, W: Write + Send + 'static>(
    state: State,
    reader: R,
    writer: W,
) -> i32 {
    let state = Arc::new(state);
    let writer: SharedWriter = Arc::new(Mutex::new(writer));
    let (events, inbox) = channel::<Event>();

    let reader_thread = {
        let events = events.clone();
        std::thread::spawn(move || {
            for line in reader.lines() {
                let event = match line {
                    Ok(line) => Event::Line(line),
                    Err(_) => {
                        let _ = events.send(Event::ReadFailed);
                        return;
                    }
                };
                if events.send(event).is_err() {
                    return; // the main loop is gone; nothing left to ferry to
                }
            }
            let _ = events.send(Event::Eof);
        })
    };

    // The main thread's own map, never locked because never shared: the only writer is this loop.
    let mut in_flight: HashMap<String, CancelToken> = HashMap::new();
    let mut tool_threads: Vec<JoinHandle<()>> = Vec::new();
    let mut exit_code = 0;
    let mut draining = false;

    while let Ok(event) = inbox.recv() {
        match event {
            Event::Line(line) => {
                // `Freerouting.java:739-741` — a blank line is skipped. Java skips it *before* the
                // round trip and so answers nothing either, which is the one place the two
                // programs agree about blank lines; what Java then does to a **notification** is
                // the delta (see the delta table's blank-line row).
                if line.trim().is_empty() {
                    continue;
                }
                let req = match serde_json::from_str::<Request>(&line) {
                    Ok(req) => req,
                    Err(e) => {
                        // `McpControllerV1.java:152` — `-32700`, with `id` **null**. Java's answer
                        // has no `id` member at all (Gson drops a JSON-null) and arrives
                        // pretty-printed; the port's carries `"id":null` and is compact. The
                        // id-less `-32700` row of the delta table.
                        if write_line(
                            &writer,
                            &Response::err(Value::Null, PARSE_ERROR, format!("parse error: {e}")),
                        )
                        .is_err()
                        {
                            draining = true;
                        }
                        if draining && in_flight.is_empty() {
                            break;
                        }
                        continue;
                    }
                };

                if req.method == "notifications/cancelled" {
                    cancel_in_flight(&in_flight, req.params.as_ref());
                    continue;
                }

                // A `tools/call` **with an id** is the only thing that gets a thread. A `tools/call`
                // sent as a notification is answered by nobody, so running it would burn a thread
                // for an outcome no one can read; `handle` returns `None` for it below.
                if req.method == "tools/call" && req.id.is_some() {
                    let (key, thread) =
                        spawn_tool_call(req, &state, &writer, &events, &mut in_flight);
                    tool_threads.push(thread);
                    debug_assert!(in_flight.contains_key(&key));
                    continue;
                }

                // Everything else is answered on this thread: it is a map lookup and a `json!`,
                // and moving it to a thread would only make the ordering harder to reason about.
                if let Some(resp) = handle(
                    &state,
                    req,
                    &ProgressWriter::disabled(),
                    &CancelToken::new(),
                ) && write_line(&writer, &resp).is_err()
                {
                    draining = true;
                }
                if draining && in_flight.is_empty() {
                    break;
                }
            }
            Event::Eof | Event::ReadFailed => {
                if matches!(event, Event::ReadFailed) {
                    exit_code = 1;
                }
                // The peer is gone: nothing can read another progress notification and nothing
                // will send another line, so every running tool is working for no one.
                for token in in_flight.values() {
                    token.cancel();
                }
                draining = true;
                if in_flight.is_empty() {
                    break;
                }
            }
            Event::ToolDone(key) => {
                in_flight.remove(&key);
                if draining && in_flight.is_empty() {
                    break;
                }
            }
        }
    }

    let _ = reader_thread.join();
    for thread in tool_threads {
        let _ = thread.join();
    }
    exit_code
}

/// Starts one `tools/call` on its own thread and records its [`CancelToken`] as in flight.
///
/// Everything the thread needs is moved into it: an `Arc<State>`, the shared writer, the request's
/// progress writer and its token. The parent keeps a *clone* of the token — two clones of one
/// token are one token — which is what makes `notifications/cancelled` reach a running tool.
fn spawn_tool_call(
    req: Request,
    state: &Arc<State>,
    writer: &SharedWriter,
    events: &Sender<Event>,
    in_flight: &mut HashMap<String, CancelToken>,
) -> (String, JoinHandle<()>) {
    let key = request_key(req.id.as_ref().expect("the caller checked for an id"));
    let cancel = CancelToken::new();
    in_flight.insert(key.clone(), cancel.clone());

    // MCP puts the progress token at `params._meta.progressToken`. Absent ⇒
    // `ProgressWriter::disabled`, which drops everything, so a tool never branches.
    let progress = ProgressWriter::new(
        req.params
            .as_ref()
            .and_then(|p| p.get("_meta"))
            .and_then(|meta| meta.get("progressToken"))
            .cloned(),
        Arc::clone(writer),
    );

    let state = Arc::clone(state);
    let writer = Arc::clone(writer);
    let events = events.clone();
    let done_key = key.clone();
    let thread = std::thread::spawn(move || {
        if let Some(resp) = handle(&state, req, &progress, &cancel) {
            let _ = write_line(&writer, &resp);
        }
        let _ = events.send(Event::ToolDone(done_key));
    });
    (key, thread)
}

/// `notifications/cancelled` (`{"requestId": …, "reason": …}`) — flip the named request's token.
///
/// An unknown `requestId` is **ignored**, which the spec requires: a cancellation always races the
/// response, and a server that complained about the race would complain on every well-behaved
/// client. `reason` is read by nobody; it is a human-facing string.
fn cancel_in_flight(in_flight: &HashMap<String, CancelToken>, params: Option<&Value>) {
    let Some(id) = params.and_then(|p| p.get("requestId")) else {
        return;
    };
    if let Some(token) = in_flight.get(&request_key(id)) {
        // `ALL`, never `AUTO_ROUTER_ONLY` — see [`run_with`]'s cancellation section.
        token.cancel();
    }
}

/// The in-flight map's key: the request id's JSON text.
///
/// JSON-RPC allows a string or a number, and `1` and `"1"` are different ids; their JSON texts are
/// `1` and `"1"`, so the encoding keeps them apart without the port having to decide which of the
/// two spellings a client meant.
fn request_key(id: &Value) -> String {
    id.to_string()
}
