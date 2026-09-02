use super::jsonrpc::{INVALID_REQUEST, PARSE_ERROR, Request, Response};
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
pub fn run(settings_argv: &[String]) -> i32 {
    // The registry is built **once**, here, before the state is shared — see
    // [`super::tools::register_all`] for the contrast with the jar, which rebuilds its own with a
    // fresh OpenAPI scan on every `tools/list` *and* every `tools/call`.
    //
    // `settings_argv` is the server process's own raw argv, which is the command line every
    // settings tier below priority 70 is built from; see [`State::settings_argv`].
    let mut state = State::with_settings_argv(settings_argv);
    super::tools::register_all(&mut state);
    // `stdin()`/`stdout()` rather than their `lock()`s: a `StdinLock`/`StdoutLock` borrows the
    // handle and is `!Send`, and the reader thread and the tool threads need owned, sendable
    // halves. Each handle carries its own internal lock, so the two are as exclusive as the
    // guards would have been.
    run_with(
        state,
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
/// # Shutdown — three ways the peer can vanish, and **two** meanings (controller ruling BH)
///
/// All three stop the loop taking new work and then **drain** — the loop keeps running until the
/// last tool thread has reported, so a response already being written is not truncated. What they
/// disagree about is whether the work still running is worth finishing, and the answer follows
/// from *which pipe* broke:
///
/// | how the peer vanished | in-flight `tools/call`s | exit | Java |
/// |---|---|---|---|
/// | **EOF on stdin** | **finished** — every one of them answers | `0` | `:778-779` |
/// | a read failure on stdin | cancelled | `1` | `:780-782` |
/// | a failed write to stdout | cancelled | `1` | unreachable |
///
/// **EOF on stdin does not mean nobody is reading.** It means the client has no more *requests*,
/// which is exactly what a batch client does: write every line, close stdin, read every answer.
/// Cancelling there turned `freerouting mcp < script.jsonl` into a program that answers **partial**
/// results with `timed_out: false` and no error — measured before ruling BH, a `route_board` on
/// `fixtures/Issue143-rpi_splitter.dsn` piped from a file answered **797 bytes and 0 wires** where
/// the same request over a live pipe answered **3 656 bytes and 16 wires**. A quietly wrong answer
/// is worse than a slow one, so EOF now drains without cancelling and the same piped script
/// answers the full 3 656 bytes. What bounds the drain is what bounds any run: `--max-passes`, the
/// job timeout, and the router's own budget.
///
/// A **read failure** and a **failed write** are the other thing. There the pipe the *answers* go
/// down is gone (or the process cannot learn anything more about it), so every running tool really
/// is working for no one, and both cancel every in-flight token before draining. Java cannot reach
/// the third case at all: `PrintStream.println` (`:771`) swallows its errors, so the jar writes
/// into the void and still exits `0`.
///
/// Java's own shutdown is neither: `System.exit(0)` (`:777-779`) kills its daemon thread mid-write.
/// The drain is a deliberate improvement over a shutdown that can lose a line. It is row 11 of the
/// delta table in `crates/freerouting/README.md`.
///
/// One join is conditional and the reason is written at it: on the write-failure path the reader
/// thread is still parked in `reader.lines()` on a stdin nobody has closed, so it is dropped
/// rather than joined — the receiver is closed first, which makes it self-terminating on its next
/// line. On the other two paths it has already returned and is joined.
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
    // Whether the reader thread has already returned. It has, on both `Eof` and `ReadFailed`, and
    // it has not on the write-failure path — which is the whole of why the join below is
    // conditional. See the shutdown section of this function's doc.
    let mut reader_ended = false;

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

                // Every line resolves to at most one message written from *this* thread; a
                // `tools/call` resolves to `None` here and writes from its own thread instead. One
                // write site, so the three ways a line can be answered cannot drift apart in how
                // they handle a stdout that has failed.
                let outbound: Option<Response> = match serde_json::from_str::<Request>(&line) {
                    // `McpControllerV1.java:152` — `-32700`, with `id` **null**. Java's answer has
                    // no `id` member at all (Gson drops a JSON-null) and arrives pretty-printed;
                    // the port's carries `"id":null` and is compact. The id-less `-32700` row of
                    // the delta table.
                    Err(e) => Some(Response::err(
                        Value::Null,
                        PARSE_ERROR,
                        format!("parse error: {e}"),
                    )),
                    Ok(req) => {
                        if req.method == "notifications/cancelled" {
                            cancel_in_flight(&in_flight, req.params.as_ref());
                            continue;
                        }
                        // A `tools/call` **with an id** is the only thing that gets a thread. A
                        // `tools/call` sent as a notification is answered by nobody, so running it
                        // would burn a thread for an outcome no one can read; `handle` returns
                        // `None` for it below.
                        if req.method == "tools/call"
                            && let Some(id) = req.id.clone()
                        {
                            // A **reused** id is refused rather than run: see
                            // [`spawn_tool_call`]'s "a reused id is refused" section.
                            if in_flight.contains_key(&request_key(&id)) {
                                Some(Response::err(id, INVALID_REQUEST, REUSED_ID))
                            } else {
                                let (key, thread) =
                                    spawn_tool_call(req, &state, &writer, &events, &mut in_flight);
                                tool_threads.push(thread);
                                debug_assert!(in_flight.contains_key(&key));
                                continue;
                            }
                        } else {
                            // Everything else is answered on this thread: it is a map lookup and a
                            // `json!`, and moving it to a thread would only make the ordering
                            // harder to reason about.
                            handle(
                                &state,
                                req,
                                &ProgressWriter::disabled(),
                                &CancelToken::new(),
                            )
                        }
                    }
                };

                if let Some(resp) = outbound
                    && write_line(&writer, &resp).is_err()
                {
                    // A stdout that will not take a line is a peer that has stopped **reading**,
                    // so every running tool is working for no one. Unchanged by ruling BH, which
                    // moved only the EOF-on-stdin path: see `run_with`'s shutdown table for why
                    // the two are not the same event.
                    exit_code = 1;
                    draining = true;
                    cancel_every_in_flight(&in_flight);
                }
                if draining && in_flight.is_empty() {
                    break;
                }
            }
            Event::Eof | Event::ReadFailed => {
                // Both paths: the reader thread has already returned, so the conditional join at
                // the foot of this function may wait on it, and no further line can arrive, so the
                // loop stops taking new work and runs only until the last tool thread reports.
                reader_ended = true;
                draining = true;
                // **Controller ruling BH**: the two paths part company here, and only here.
                //
                // A **read failure** means the process can learn nothing more about the pipe, so
                // the in-flight work is abandoned and the code is Java's `1` (`:780-782`).
                //
                // An **EOF** means the client has no more *requests* — not that it has stopped
                // reading answers. Cancelling there made `freerouting mcp < script.jsonl` answer
                // partial results with no error at all; see `run_with`'s shutdown table for the
                // measurement. So EOF drains and every in-flight call finishes, and the code is
                // Java's `0` (`:778-779`).
                if matches!(event, Event::ReadFailed) {
                    exit_code = 1;
                    cancel_every_in_flight(&in_flight);
                }
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

    // Closing the receiver first is what makes the reader thread self-terminating on the
    // write-failure path: its next `events.send` answers `Err` and it returns. It is already
    // blocked in `reader.lines()` at that moment, so "next" means "when the peer writes another
    // line or closes stdin" — which is why the join below is **conditional**. Tool threads only
    // ever `let _ = send`, so closing the channel under them is harmless.
    drop(inbox);
    for thread in tool_threads {
        let _ = thread.join();
    }
    // Joined **only** when it has already returned. On the write-failure path it is still parked
    // in `reader.lines()` on a stdin nobody has closed, and joining it would hang the process for
    // as long as the peer keeps that handle open. It owns nothing but its half of a closed
    // channel, so the honest move is to let the process outlive it rather than to wait.
    if reader_ended {
        let _ = reader_thread.join();
    } else {
        drop(reader_thread);
    }
    exit_code
}

/// `token.cancel()` for every call still in flight — the meaning "**nobody is reading the
/// answers**", shared by the read-failure and write-failure paths so the two cannot drift apart.
///
/// **Not the EOF path** (controller ruling BH): a client that closed stdin has no more requests,
/// which is not the same thing as a client that has stopped reading. See [`run_with`]'s shutdown
/// table.
fn cancel_every_in_flight(in_flight: &HashMap<String, CancelToken>) {
    for token in in_flight.values() {
        // `ALL`, never `AUTO_ROUTER_ONLY` — see [`run_with`]'s cancellation section.
        token.cancel();
    }
}

/// The `-32600` message a `tools/call` gets when its id is already in flight. A constant because
/// the caller sends it and [`spawn_tool_call`]'s doc explains it.
const REUSED_ID: &str =
    "request id is already in flight; MCP requires an id to be unique within a session";

/// Starts one `tools/call` on its own thread and records its [`CancelToken`] as in flight.
///
/// Everything the thread needs is moved into it: an `Arc<State>`, the shared writer, the request's
/// progress writer and its token. The parent keeps a *clone* of the token — two clones of one
/// token are one token — which is what makes `notifications/cancelled` reach a running tool.
///
/// # A reused id is refused, deliberately
///
/// MCP is explicit that a request id **must not** have been used before by the same requestor
/// within a session, and the whole of this transport's cancellation machinery keys on the id being
/// unique: a second call arriving under an id already in the map would silently displace the
/// first's token, making the first uncancellable, and the first `ToolDone` would then evict the
/// *survivor's* entry. So the second call is refused with [`INVALID_REQUEST`] and never runs —
/// which is the answer a client can act on, where overwriting is one it cannot even see.
///
/// The check covers what the map knows, which is precisely the set of calls that can be cancelled:
/// a reused id on a method answered inline is not tracked, cannot displace anything, and is
/// answered normally. It lives at the **call site**, because that is where the refusal has to be
/// written from; this function is reached only once it has passed.
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
    let done = ToolDoneGuard {
        events: events.clone(),
        key: Some(key.clone()),
    };
    let thread = std::thread::spawn(move || {
        // Bound to a name so it lives to the end of the closure rather than being dropped at the
        // end of this statement.
        let _done = done;
        if let Some(resp) = handle(&state, req, &progress, &cancel) {
            let _ = write_line(&writer, &resp);
        }
    });
    (key, thread)
}

/// Announces `ToolDone` from [`Drop`], so the main loop learns a call is over on **every** exit
/// from its thread and not only on the ordinary one.
///
/// Ruling 4's `catch_unwind` covers the handler, which is where a panic is expected; this covers
/// the rest of the thread — `write_line`'s `expect` on a value that will not serialize, say. Sent
/// as the thread's last statement instead, a panic there would strand the key in the in-flight map
/// and the EOF drain would then wait in `recv()` forever. Three lines buy the drain a termination
/// argument that does not depend on the boundary being complete.
struct ToolDoneGuard {
    events: Sender<Event>,
    /// `Option` only so [`Drop`] can move the key out; it is `Some` for the guard's whole life.
    key: Option<String>,
}

impl Drop for ToolDoneGuard {
    fn drop(&mut self) {
        if let Some(key) = self.key.take() {
            // The main loop may already have gone; that is the shutdown that does not need us.
            let _ = self.events.send(Event::ToolDone(key));
        }
    }
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

// =================================================================================================
// The `api/mcp/**` roster, part 2: the two HTTP back-channels — ruling AO, Plan 8 Task 14
// =================================================================================================
//
// `scripts/audit-map/freerouting.map` points these three classes at this file because this is the
// port's transport, and a transport is what they are. The reasoning block is at the foot of
// `mcp/server.rs`; the short form is that a stdio peer already owns both ends of one pipe, so an
// SSE stream and a WebSocket are two ways to build a back-channel this server was born with.
// [`run_with`]'s writer half is the whole of what all three exist to provide.

// --- McpRealtimeBridge (71) — the SSE/WebSocket fan-out -----------------------------------------
// not ported: McpRealtimeBridge.registerSseClient (api/mcp/McpRealtimeBridge.java:20-23) — adds an `SseEventSink` to the broadcast set.
// not ported: McpRealtimeBridge.removeSseClient (api/mcp/McpRealtimeBridge.java:25-28) — removes one.
// not ported: McpRealtimeBridge.registerWsClient (api/mcp/McpRealtimeBridge.java:30-33) — the WebSocket twin of the first.
// not ported: McpRealtimeBridge.removeWsClient (api/mcp/McpRealtimeBridge.java:35-38) — the WebSocket twin of the second.
// not ported: McpRealtimeBridge.broadcast (api/mcp/McpRealtimeBridge.java:40-70) — writes one named event to every registered client of both kinds. The port writes to exactly one peer, under the [`super::server::SharedWriter`] lock, so there is no set to walk and no partial-failure arm to reproduce.

// --- McpWebSocketEndpoint (130) and McpWebSocketConfigurator (32) — the WS half -----------------
// not ported: McpWebSocketEndpoint.onOpen (api/mcp/McpWebSocketEndpoint.java:28-54) — the handshake: reads the API key out of the stashed headers, closes with a policy violation when it fails, registers with the bridge otherwise.
// not ported: McpWebSocketEndpoint.onMessage (api/mcp/McpWebSocketEndpoint.java:69-129) — the WebSocket's own copy of the JSON-RPC dispatch. The port has one dispatch, [`super::server::handle`], and one reader loop.
// not ported: McpWebSocketEndpoint.onClose (api/mcp/McpWebSocketEndpoint.java:57-60) — deregisters the session.
// not ported: McpWebSocketEndpoint.onError (api/mcp/McpWebSocketEndpoint.java:63-66) — the same, on a `Throwable`. The port's three ways for a peer to vanish are Task 11's, in [`run_with`].
// not ported: McpWebSocketConfigurator.modifyHandshake (api/mcp/McpWebSocketConfigurator.java:18-22) — stashes the handshake headers in the endpoint config so `onOpen` can read the API key out of them.
// not ported: McpWebSocketConfigurator.getHeaders (api/mcp/McpWebSocketConfigurator.java:25-31) — the unchecked cast that reads them back.
