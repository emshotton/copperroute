use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

#[test]
fn initialize_ping_and_list_over_pipes() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_freerouting"))
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    fn send(stdin: &mut impl Write, stdout: &mut impl BufRead, s: &str) -> serde_json::Value {
        writeln!(stdin, "{s}").unwrap();
        let mut line = String::new();
        stdout.read_line(&mut line).unwrap();
        serde_json::from_str::<serde_json::Value>(&line).unwrap()
    }

    let init = send(
        &mut stdin,
        &mut stdout,
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"t","version":"0"}}}"#,
    );
    assert_eq!(init["result"]["serverInfo"]["name"], "freerouting");
    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","method":"notifications/initialized"}}"#
    )
    .unwrap();
    let ping = send(
        &mut stdin,
        &mut stdout,
        r#"{"jsonrpc":"2.0","id":2,"method":"ping"}"#,
    );
    assert_eq!(ping["result"], serde_json::json!({}));
    let list = send(
        &mut stdin,
        &mut stdout,
        r#"{"jsonrpc":"2.0","id":3,"method":"tools/list"}"#,
    );
    // Task 11 asserted this list was **empty**, because the binary registered no tool until
    // Task 12. It now carries spec §13's four, in `BTreeMap` order — and **four** is delta row
    // 10's whole point against the jar's 28 (`docs/plan-8-prep/evidence/job3-summary.md` §4).
    // `the_four_tools_over_spawned_pipes` below is where the list is checked properly; this line
    // keeps the transport's own smoke test honest about what a client sees.
    let names: Vec<&str> = list["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        vec!["board_info", "check_drc", "list_settings", "route_board"]
    );

    drop(stdin);
    let status = child.wait().unwrap();
    assert!(status.success());
}

// =================================================================================================
// Plan 8 Task 11 — the concurrent transport
// =================================================================================================
//
// The test above drives the **binary** over real pipes and is the end-to-end proof that `mcp` is
// wired up. Everything below drives `mcp::stdio::run_with` directly, because the six behaviours
// Task 11 adds all need a *registered tool*, and the binary registers none until Task 12.
//
// No test here leaves a thread running when it returns. `Harness::finish` joins the server thread,
// which joins everything it spawned; `a_dead_stdout_cancels_everything_and_exits_nonzero` is the
// one that does not call `finish` (it must not — `run_with` returns there *without* joining the
// reader), and it closes stdin by hand at the end for the same reason. `cargo nextest` reported a
// `LEAK` verdict once during review on `a_long_running_tool_reports_progress` (0.213 s against a
// 100 ms default leak-timeout, on a machine running eight invocations at once); it did not recur
// in 30 consecutive runs here, `LEAK` is a timing verdict rather than an assertion, and there is
// no detached thread for it to be about.
//
// The reader is scripted rather than a `Cursor`: a `Cursor` hands the transport every line at once
// and then EOF, and EOF cancels every in-flight call — so a cancellation test over a `Cursor`
// would pass even if `notifications/cancelled` did nothing at all. `Script` blocks until the test
// sends the next line, which is what makes `a_cancelled_tool_stops_mid_flight` a test of the
// notification rather than of the shutdown.

use freerouting::mcp::server::{State, ToolDef};
use freerouting::mcp::stdio::run_with;
use serde_json::{Value, json};
use std::io::Read;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// A blocking, scriptable stdin. `read` returns a line when one has been sent and `0` (EOF) once
/// the sender is dropped, which is exactly what a pipe does.
struct Script {
    lines: Receiver<String>,
    pending: Vec<u8>,
    at: usize,
    /// When set, the first `read` fails instead — Java's `IOException` on `System.in`
    /// (`Freerouting.java:780-782`), whose answer is exit **1**.
    fail: bool,
}

impl Read for Script {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        if self.fail {
            return Err(std::io::Error::other("stdin exploded"));
        }
        while self.at >= self.pending.len() {
            match self.lines.recv() {
                Ok(line) => {
                    self.pending = line.into_bytes();
                    self.at = 0;
                }
                Err(_) => return Ok(0),
            }
        }
        let n = (self.pending.len() - self.at).min(out.len());
        out[..n].copy_from_slice(&self.pending[self.at..self.at + n]);
        self.at += n;
        Ok(n)
    }
}

/// The transport's stdout, captured — and, when the second field is set, a stdout that starts
/// failing once it has accepted that many bytes. A peer that stops reading is the third way the
/// transport can learn that nobody is listening.
#[derive(Clone)]
struct Captured(Arc<Mutex<Vec<u8>>>, Option<usize>);

impl std::io::Write for Captured {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut sink = self.0.lock().unwrap();
        if self.1.is_some_and(|limit| sink.len() >= limit) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "the peer stopped reading",
            ));
        }
        sink.extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

struct Harness {
    lines: Option<Sender<String>>,
    out: Captured,
    server: Option<std::thread::JoinHandle<i32>>,
}

impl Harness {
    fn start(state: State) -> Harness {
        Harness::start_with(state, false, None)
    }

    /// `fail_reads` makes the first `read` fail; `fail_writes_after` makes stdout fail once that
    /// many bytes have been accepted, which is how the write-failure shutdown path is reached.
    fn start_with(state: State, fail_reads: bool, fail_writes_after: Option<usize>) -> Harness {
        let (tx, rx) = channel::<String>();
        let out = Captured(Arc::new(Mutex::new(Vec::new())), fail_writes_after);
        let reader = std::io::BufReader::new(Script {
            lines: rx,
            pending: Vec::new(),
            at: 0,
            fail: fail_reads,
        });
        let writer = out.clone();
        let server = std::thread::spawn(move || run_with(state, reader, writer));
        Harness {
            lines: Some(tx),
            out,
            server: Some(server),
        }
    }

    fn send(&self, line: Value) {
        self.lines
            .as_ref()
            .unwrap()
            .send(format!("{line}\n"))
            .unwrap();
    }

    fn send_raw(&self, line: &str) {
        self.lines
            .as_ref()
            .unwrap()
            .send(format!("{line}\n"))
            .unwrap();
    }

    /// Close stdin (EOF), join the transport, and answer its exit code and every stdout line.
    fn finish(mut self) -> (i32, Vec<String>) {
        drop(self.lines.take());
        let code = self.server.take().unwrap().join().unwrap();
        let text = String::from_utf8(self.out.0.lock().unwrap().clone()).unwrap();
        let lines = text.lines().map(str::to_string).collect();
        (code, lines)
    }
}

/// Spins until `done` or the deadline; a deadlock is a failure, not a hang.
fn wait_for(what: &str, done: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !done() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn tool(name: &str) -> ToolDef {
    ToolDef {
        name: name.into(),
        description: name.into(),
        input_schema: json!({"type": "object"}),
    }
}

fn parsed(lines: &[String]) -> Vec<Value> {
    lines
        .iter()
        .map(|l| serde_json::from_str::<Value>(l).unwrap_or_else(|e| panic!("{l:?}: {e}")))
        .collect()
}

/// A tool reports progress *while* its `tools/call` is outstanding, and the five notifications all
/// reach stdout **before** the response. Java cannot express this at all: its bridge is one
/// blocking HTTP round trip per line (`Freerouting.java:749-776`).
#[test]
fn a_long_running_tool_reports_progress() {
    let mut state = State::new();
    state.register_tool(
        tool("five"),
        Box::new(|_state, _args, progress, _cancel| {
            for step in 1..=5 {
                progress.progress(f64::from(step), Some(5.0), Some(&format!("step {step}")));
            }
            Ok(json!({"done": true}))
        }),
    );

    let h = Harness::start(state);
    h.send(json!({
        "jsonrpc": "2.0", "id": 7, "method": "tools/call",
        "params": {"name": "five", "arguments": {}, "_meta": {"progressToken": "tok-7"}},
    }));
    let (code, lines) = h.finish();

    assert_eq!(code, 0);
    let msgs = parsed(&lines);
    assert_eq!(msgs.len(), 6, "{lines:?}");
    for (i, msg) in msgs[..5].iter().enumerate() {
        assert_eq!(msg["jsonrpc"], "2.0");
        assert_eq!(msg["method"], "notifications/progress");
        assert!(msg.get("id").is_none(), "a notification carries no id");
        assert_eq!(msg["params"]["progressToken"], "tok-7");
        assert_eq!(msg["params"]["progress"], (i + 1) as f64);
        assert_eq!(msg["params"]["total"], 5.0);
        assert_eq!(msg["params"]["message"], format!("step {}", i + 1));
    }
    // The response is last, and it is the response to *this* request.
    assert_eq!(msgs[5]["id"], 7);
    assert_eq!(msgs[5]["result"]["isError"], false);
    assert_eq!(
        msgs[5]["result"]["structuredContent"],
        json!({"done": true})
    );
}

/// A request with no `_meta.progressToken` gets the drop-everything writer, so the same tool
/// answers with no notifications at all and never had to ask.
#[test]
fn a_tool_without_a_progress_token_emits_no_notifications() {
    let mut state = State::new();
    state.register_tool(
        tool("five"),
        Box::new(|_state, _args, progress, _cancel| {
            assert!(!progress.is_enabled());
            for step in 1..=5 {
                progress.progress(f64::from(step), None, None);
            }
            Ok(json!({"done": true}))
        }),
    );

    let h = Harness::start(state);
    h.send(json!({
        "jsonrpc": "2.0", "id": 8, "method": "tools/call",
        "params": {"name": "five", "arguments": {}},
    }));
    let (code, lines) = h.finish();
    assert_eq!(code, 0);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert_eq!(parsed(&lines)[0]["id"], 8);
}

/// The cancellation arrives **while** the tool is running and the tool observes it — the whole of
/// the obligation that stood at `mcp/stdio.rs:18-28`. The ordering is forced: the test does not
/// close stdin until the tool has already reported that it saw the flag, so the EOF-cancels-
/// everything path cannot be what set it.
#[test]
fn a_cancelled_tool_stops_mid_flight() {
    let started = Arc::new(AtomicBool::new(false));
    let observed = Arc::new(AtomicBool::new(false));

    let mut state = State::new();
    {
        let started = Arc::clone(&started);
        let observed = Arc::clone(&observed);
        state.register_tool(
            tool("spin"),
            Box::new(move |_state, _args, _progress, cancel| {
                started.store(true, Ordering::SeqCst);
                let deadline = Instant::now() + Duration::from_secs(10);
                while !cancel.is_cancelled() {
                    assert!(Instant::now() < deadline, "the cancellation never arrived");
                    std::thread::sleep(Duration::from_millis(2));
                }
                observed.store(true, Ordering::SeqCst);
                Ok(json!({"cancelled": true}))
            }),
        );
    }

    let h = Harness::start(state);
    h.send(json!({
        "jsonrpc": "2.0", "id": "call-1", "method": "tools/call",
        "params": {"name": "spin", "arguments": {}},
    }));
    wait_for("the tool to start", || started.load(Ordering::SeqCst));
    // Still running, still unanswered: the transport is not blocked on it.
    assert!(!observed.load(Ordering::SeqCst));
    h.send(json!({
        "jsonrpc": "2.0", "method": "notifications/cancelled",
        "params": {"requestId": "call-1", "reason": "user pressed stop"},
    }));
    wait_for("the tool to observe the cancellation", || {
        observed.load(Ordering::SeqCst)
    });

    let (code, lines) = h.finish();
    assert_eq!(code, 0);
    // One line: the tool's response. The notification itself is answered by nothing.
    assert_eq!(lines.len(), 1, "{lines:?}");
    let msgs = parsed(&lines);
    assert_eq!(msgs[0]["id"], "call-1");
    assert_eq!(
        msgs[0]["result"]["structuredContent"],
        json!({"cancelled": true})
    );
}

/// Ruling 4's boundary: the panicking tool's call comes back as `-32603` and the **next** request
/// is answered, on a server that is still running.
#[test]
fn a_panicking_tool_becomes_an_error_and_the_server_survives() {
    let mut state = State::new();
    state.register_tool(
        tool("boom"),
        Box::new(|_state, _args, _progress, _cancel| panic!("the tool exploded")),
    );
    state.register_tool(
        tool("fine"),
        Box::new(|_state, args, _progress, _cancel| Ok(args)),
    );

    let h = Harness::start(state);
    h.send(json!({
        "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": {"name": "boom", "arguments": {}},
    }));
    // Wait for the answer before sending the next line, so "the next request" really is next.
    wait_for("the panicking call to answer", || {
        !h.out.0.lock().unwrap().is_empty()
    });
    h.send(json!({
        "jsonrpc": "2.0", "id": 2, "method": "tools/call",
        "params": {"name": "fine", "arguments": {"still": "here"}},
    }));
    h.send(json!({"jsonrpc": "2.0", "id": 3, "method": "ping"}));

    let (code, lines) = h.finish();
    assert_eq!(code, 0);
    let msgs = parsed(&lines);
    assert_eq!(msgs.len(), 3, "{lines:?}");
    assert_eq!(msgs[0]["id"], 1);
    assert_eq!(msgs[0]["error"]["code"], -32603);
    assert!(
        msgs[0]["error"]["message"]
            .as_str()
            .unwrap()
            .contains("the tool exploded"),
        "{:?}",
        msgs[0]
    );
    assert!(msgs[0].get("result").is_none());
    // The other two are matched **by id, not by position**: the `fine` call runs on its own
    // thread while `ping` is answered on the main one, so the ping may legitimately overtake it.
    // That race is the transport working, and a client pairs responses to requests by id anyway.
    let by_id = |id: i64| {
        msgs.iter()
            .find(|m| m["id"] == id)
            .unwrap_or_else(|| panic!("no response for id {id} in {msgs:?}"))
    };
    assert_eq!(
        by_id(2)["result"]["structuredContent"],
        json!({"still": "here"})
    );
    assert_eq!(by_id(3)["result"], json!({}));
}

/// **Stdout is the protocol's.** Java gets there with `System.setOut(System.err)`
/// (`Freerouting.java:919`); the port's `main.rs` sends `tracing` to stderr unconditionally, which
/// is stronger — there is no global to redirect and a tool cannot reach the protocol stream by
/// accident. A tool that prints, logs and warns still leaves stdout carrying JSON-RPC and nothing
/// else, one object per line with no `Content-Length` framing (kept from Java).
#[test]
fn stdout_carries_only_protocol_lines() {
    let mut state = State::new();
    state.register_tool(
        tool("noisy"),
        Box::new(|_state, _args, progress, _cancel| {
            eprintln!("a tool printing to stderr");
            tracing::warn!("a tool logging through tracing");
            progress.progress(1.0, None, Some("chatter"));
            Ok(json!({"ok": true}))
        }),
    );

    let h = Harness::start(state);
    h.send_raw("");
    h.send_raw("   ");
    h.send(json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}));
    h.send(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
    h.send_raw("{not json at all");
    h.send(json!({
        "jsonrpc": "2.0", "id": 2, "method": "tools/call",
        "params": {"name": "noisy", "arguments": {}, "_meta": {"progressToken": 42}},
    }));
    let (code, lines) = h.finish();

    assert_eq!(code, 0);
    for line in &lines {
        assert!(!line.is_empty(), "a blank line reached stdout: {lines:?}");
        assert!(
            !line.contains('\r'),
            "a bare carriage return reached stdout: {line:?}"
        );
    }
    let msgs = parsed(&lines);
    for msg in &msgs {
        assert_eq!(msg["jsonrpc"], "2.0", "{msg}");
    }
    // initialize, the `-32700`, the progress notification, the tool's response — and nothing for
    // either blank line or for the notification.
    assert_eq!(msgs.len(), 4, "{lines:?}");
    assert_eq!(msgs[0]["id"], 1);
    assert_eq!(msgs[0]["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(msgs[1]["id"], Value::Null);
    assert_eq!(msgs[1]["error"]["code"], -32700);
    assert_eq!(msgs[2]["method"], "notifications/progress");
    assert_eq!(msgs[2]["params"]["progressToken"], 42);
    assert_eq!(msgs[3]["id"], 2);
}

/// `Freerouting.java:778-782`, reproduced: EOF on stdin is exit **0** and a read failure is exit
/// **1**. That much is Java's, and it is the only part of the bridge's shutdown the port keeps.
#[test]
fn eof_exits_zero() {
    let (code, lines) = Harness::start(State::new()).finish();
    assert_eq!(code, 0);
    assert!(lines.is_empty(), "{lines:?}");

    // The other arm: `IOException` reading `System.in` ⇒ `System.exit(1)`.
    let (code, lines) = Harness::start_with(State::new(), true, None).finish();
    assert_eq!(code, 1);
    assert!(lines.is_empty(), "{lines:?}");
}

/// A notification — no `id` — is answered by nothing at all. Java prints a **blank line** for one
/// instead (`McpControllerV1.java:176-177` answers HTTP 204, and `Freerouting.java:769-772` prints
/// the empty body), which desynchronises a line-oriented client. The blank-line row of the delta
/// table in `crates/freerouting/README.md`.
#[test]
fn a_notification_gets_no_response() {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut state = State::new();
    {
        let calls = Arc::clone(&calls);
        state.register_tool(
            tool("counted"),
            Box::new(move |_state, args, _progress, _cancel| {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok(args)
            }),
        );
    }

    let h = Harness::start(state);
    h.send(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
    h.send(
        json!({"jsonrpc": "2.0", "method": "notifications/cancelled",
                  "params": {"requestId": "nobody's"}}),
    );
    h.send(json!({"jsonrpc": "2.0", "method": "ping"}));
    // A `tools/call` sent as a notification is not run: nothing could read its answer.
    h.send(json!({"jsonrpc": "2.0", "method": "tools/call",
                  "params": {"name": "counted", "arguments": {}}}));
    h.send(json!({"jsonrpc": "2.0", "id": 1, "method": "ping"}));

    let (code, lines) = h.finish();
    assert_eq!(code, 0);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert_eq!(parsed(&lines)[0]["id"], 1);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

/// **SF2 + SF1.** A stdout that stops taking lines is a peer that is gone, and it means what an
/// EOF means: every in-flight tool is cancelled, and the run reports failure rather than success.
/// Java cannot reach this case — `PrintStream.println` (`Freerouting.java:771`) swallows its
/// errors, so the jar writes into the void and still exits `0`.
///
/// It also pins **SF1**: `run_with` returns without joining a reader thread that is still parked
/// on a stdin nobody has closed. `finish()` is never called here — the test drops the sender only
/// after the server thread has already ended — so a `join()` on that path would hang this test
/// rather than passing it.
#[test]
fn a_dead_stdout_cancels_everything_and_exits_nonzero() {
    let started = Arc::new(AtomicBool::new(false));
    let cancelled = Arc::new(AtomicBool::new(false));

    let mut state = State::new();
    {
        let started = Arc::clone(&started);
        let cancelled = Arc::clone(&cancelled);
        state.register_tool(
            tool("spin"),
            Box::new(move |_state, _args, _progress, cancel| {
                started.store(true, Ordering::SeqCst);
                let deadline = Instant::now() + Duration::from_secs(10);
                while !cancel.is_cancelled() {
                    assert!(Instant::now() < deadline, "the cancellation never arrived");
                    std::thread::sleep(Duration::from_millis(2));
                }
                cancelled.store(true, Ordering::SeqCst);
                Ok(json!({}))
            }),
        );
    }

    // Zero bytes of headroom: the very first write fails.
    let mut h = Harness::start_with(state, false, Some(0));
    h.send(json!({
        "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": {"name": "spin", "arguments": {}},
    }));
    wait_for("the tool to start", || started.load(Ordering::SeqCst));
    // A `ping`, whose response is what fails to write and so is what tells the loop the peer is
    // gone. **stdin is left open** for the rest of the test.
    h.send(json!({"jsonrpc": "2.0", "id": 2, "method": "ping"}));

    let code = h.server.take().unwrap().join().unwrap();
    assert_eq!(code, 1, "a peer that stopped reading is not a success");
    assert!(
        cancelled.load(Ordering::SeqCst),
        "the in-flight tool was left running for nobody"
    );
    assert!(h.out.0.lock().unwrap().is_empty());
    // Close stdin explicitly. `run_with` deliberately did **not** join its reader thread (that is
    // the point of this test), so this is what lets that thread's `recv` fail and the thread end
    // before the test process does — leaving no thread outliving the run for `nextest` to notice.
    drop(h.lines.take());
}

/// **N3.** MCP requires a request id to be unique within a session, and this transport's whole
/// cancellation machinery keys on it: a second `tools/call` under an id already in flight would
/// silently displace the first's token. It is refused with `-32600` and never runs — the answer a
/// client can act on — and the first call is untouched.
#[test]
fn a_reused_in_flight_id_is_refused_and_the_first_call_survives() {
    let started = Arc::new(AtomicBool::new(false));
    let calls = Arc::new(AtomicUsize::new(0));

    let mut state = State::new();
    {
        let started = Arc::clone(&started);
        let calls = Arc::clone(&calls);
        state.register_tool(
            tool("spin"),
            Box::new(move |_state, _args, _progress, cancel| {
                calls.fetch_add(1, Ordering::SeqCst);
                started.store(true, Ordering::SeqCst);
                let deadline = Instant::now() + Duration::from_secs(10);
                while !cancel.is_cancelled() {
                    assert!(Instant::now() < deadline, "the cancellation never arrived");
                    std::thread::sleep(Duration::from_millis(2));
                }
                Ok(json!({"first": true}))
            }),
        );
    }

    let h = Harness::start(state);
    let call = json!({
        "jsonrpc": "2.0", "id": 9, "method": "tools/call",
        "params": {"name": "spin", "arguments": {}},
    });
    h.send(call.clone());
    wait_for("the first call to start", || started.load(Ordering::SeqCst));
    h.send(call);
    wait_for("the refusal", || !h.out.0.lock().unwrap().is_empty());
    // EOF cancels the first call, which then answers.
    let (code, lines) = h.finish();

    assert_eq!(code, 0);
    let msgs = parsed(&lines);
    assert_eq!(msgs.len(), 2, "{lines:?}");
    assert_eq!(msgs[0]["id"], 9);
    assert_eq!(msgs[0]["error"]["code"], -32600);
    assert!(msgs[0].get("result").is_none());
    // The first call ran once, was never displaced, and still answered.
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        msgs[1]["result"]["structuredContent"],
        json!({"first": true})
    );
}

// =================================================================================================
// Plan 8 Task 12 — the four tools, over spawned pipes
// =================================================================================================
//
// Everything from here down drives the **binary**, because that is the only way to exercise the
// thing a client actually talks to: the registry the `mcp` subcommand builds, over a real pipe
// pair. `Harness` above cannot be reused — it drives `run_with` in-process with a scripted
// reader, which is right for the transport's own behaviours and wrong for a registry that only
// `stdio::run` assembles.
//
// **The pipe must stay live.** A `freerouting mcp < script.jsonl` invocation closes stdin the
// moment the last line is read, and EOF cancels every in-flight `tools/call` (delta row 11) — so
// a routing tool driven that way answers a *partial* result. [`Pipes`] therefore writes one
// request, reads its answer, and only closes stdin when the conversation is over. Measured while
// writing these tests: `route_board` on `Issue143-rpi_splitter.dsn` answers 3 656 bytes and 16
// wires over a live pipe and 797 bytes and 0 wires when the same request is piped from a file.

use std::process::{Child, ChildStdin, ChildStdout};

/// A live conversation with the `freerouting mcp` binary.
struct Pipes {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl Pipes {
    fn start() -> Pipes {
        let mut child = Command::new(env!("CARGO_BIN_EXE_freerouting"))
            .arg("mcp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("the freerouting binary starts");
        let stdin = child.stdin.take().expect("piped stdin");
        let stdout = BufReader::new(child.stdout.take().expect("piped stdout"));
        Pipes {
            child,
            stdin,
            stdout,
        }
    }

    /// Writes one message. Returns nothing — a notification has no answer.
    fn write(&mut self, message: &Value) {
        writeln!(self.stdin, "{message}").expect("the server is still reading");
        self.stdin.flush().expect("the server is still reading");
    }

    /// The next line, parsed.
    fn read(&mut self) -> Value {
        let mut line = String::new();
        let read = self
            .stdout
            .read_line(&mut line)
            .expect("stdout is readable");
        assert!(read > 0, "the server closed stdout before answering");
        serde_json::from_str(&line).unwrap_or_else(|e| panic!("not JSON: {line:?} ({e})"))
    }

    /// The next line that is a **response** — skipping any `notifications/progress` that arrive
    /// first, which is exactly what a client does.
    fn read_response(&mut self) -> Value {
        loop {
            let message = self.read();
            if message.get("id").is_some() {
                return message;
            }
        }
    }

    /// One request, its response.
    fn request(&mut self, id: i64, method: &str, params: Value) -> Value {
        self.write(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        let answer = self.read_response();
        assert_eq!(answer["id"], id, "responses are paired by id");
        answer
    }

    /// One `tools/call`, its `structuredContent`. Panics with the tool's own message on
    /// `isError`, so a failing tool names itself rather than failing an `is_object` assertion.
    fn call(&mut self, id: i64, name: &str, arguments: Value) -> Value {
        let answer = self.request(
            id,
            "tools/call",
            json!({"name": name, "arguments": arguments}),
        );
        let result = answer["result"].clone();
        assert!(
            !result["isError"].as_bool().unwrap_or(false),
            "{name} failed: {}",
            result["content"][0]["text"]
        );
        result["structuredContent"].clone()
    }

    /// Closes stdin and waits. The exit code is Java's EOF code, 0.
    fn finish(mut self) -> i32 {
        drop(self.stdin);
        self.child
            .wait()
            .expect("the server exits")
            .code()
            .unwrap_or(-1)
    }
}

/// The committed schema golden — see [`the_schemas_match_the_committed_golden`].
fn golden_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("data")
        .join("mcp-schemas.json")
}

/// Every schema this crate publishes, in one document.
fn all_schemas() -> Value {
    use freerouting::mcp::tools::schema;
    json!({
        "board_info": schema::board_info_schema(),
        "check_drc": schema::check_drc_schema(),
        "list_settings": schema::list_settings_schema(),
        "route_board": schema::route_board_schema(),
        "RouterSettings": schema::router_settings_schema(),
    })
}

/// **Anti-drift device 1 of 2** (see `mcp::tools::schema`'s module docs for why `schemars` is
/// refused and what these two tests buy back).
///
/// A schema cannot change without a reviewer seeing the diff. Set `FR_UPDATE_GOLDEN=1` to
/// rewrite the file after a deliberate change.
#[test]
fn the_schemas_match_the_committed_golden() {
    let rendered = serde_json::to_string_pretty(&all_schemas()).expect("the schemas serialize");
    let path = golden_path();
    if std::env::var_os("FR_UPDATE_GOLDEN").is_some() {
        std::fs::write(&path, format!("{rendered}\n")).expect("the golden is writable");
        return;
    }
    let golden = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: {e}\nrun with FR_UPDATE_GOLDEN=1", path.display()));
    assert_eq!(
        rendered.trim_end(),
        golden.trim_end(),
        "the tool schemas changed; review the diff and re-run with FR_UPDATE_GOLDEN=1"
    );
}

/// **Anti-drift device 2 of 2**, and the one that is strictly stronger than `schemars` for the
/// failure that actually happens: a settings field added later that nobody exposes.
///
/// Three checks, in both directions:
///
/// 1. every property the `RouterSettings` schema publishes is a key the **reader** accepts —
///    proved by feeding the reader `{"<property>": <a value of that type>}` and requiring the
///    result to differ from `RouterSettings::new()`, so a renamed or invented property fails;
/// 2. every key the **writer** emits for a fully-resolved settings object is in the schema — so a
///    new field with a `serde` name nobody added to the schema fails;
/// 3. the counts line up against each struct's own `FIELD_NAMES` minus its `transient` list — so
///    a new field that is neither in the schema nor declared `transient` fails even if it is
///    `#[serde(skip)]` and therefore invisible to (1) and (2).
#[test]
fn every_settings_field_is_in_the_schema_and_vice_versa() {
    use fr_settings::sources::DefaultSettings;
    use fr_settings::{
        FanoutSettings, HostEnvironment, LayerSettings, OptimizerSettings, RouterSettings,
        ScoringSettings, SettingsSource,
    };
    use freerouting::mcp::tools::schema;

    let root = schema::router_settings_schema();
    let properties = |node: &Value| -> Vec<String> {
        node["properties"]
            .as_object()
            .expect("a schema object has properties")
            .keys()
            .cloned()
            .collect()
    };

    // ── 1. every published property is a key the reader accepts ──────────────────────────────
    let sample = |node: &Value| -> Value {
        match node["type"].as_str() {
            Some("boolean") => json!(true),
            Some("integer") => json!(3),
            Some("number") => json!(1.5),
            Some("string") => json!("x"),
            Some("array") => json!([]),
            Some("object") => json!({}),
            other => panic!("a schema property with no usable type: {other:?}"),
        }
    };
    let blank = RouterSettings::new();
    for name in properties(&root) {
        let node = &root["properties"][&name];
        // A nested object needs a field of its own to carry, or the reader's answer is
        // indistinguishable from the constructor's three allocated objects.
        let value = if node["type"] == "object" {
            let inner = properties(node)
                .into_iter()
                .next()
                .expect("a nested schema object publishes at least one property");
            json!({ inner.clone(): sample(&node["properties"][&inner]) })
        } else if node["type"] == "array" && node.get("items").is_some() {
            json!([{}])
        } else {
            sample(node)
        };
        let document = json!({ name.clone(): value }).to_string();
        let parsed = RouterSettings::from_json_str(&document)
            .unwrap_or_else(|e| panic!("the reader refuses the schema's own `{name}`: {e}"));
        assert_ne!(
            parsed, blank,
            "`{name}` is published by the schema but the reader ignores it"
        );
    }

    // ── 2. every key the writer emits is published ───────────────────────────────────────────
    let resolved = DefaultSettings::new(&HostEnvironment::detect())
        .get_settings()
        .cloned()
        .expect("DefaultSettings always answers a table");
    let emitted = serde_json::to_value(&resolved).expect("the settings serialize");
    let published: std::collections::BTreeSet<String> = properties(&root).into_iter().collect();
    for key in emitted.as_object().expect("an object").keys() {
        assert!(
            published.contains(key),
            "`{key}` is written by RouterSettings but missing from the schema"
        );
    }
    for (nested, node) in [
        ("fanout", &root["properties"]["fanout"]),
        ("optimizer", &root["properties"]["optimizer"]),
        ("scoring", &root["properties"]["scoring"]),
    ] {
        let published: std::collections::BTreeSet<String> = properties(node).into_iter().collect();
        for key in emitted[nested].as_object().expect("an object").keys() {
            assert!(
                published.contains(key),
                "`{nested}.{key}` is written by RouterSettings but missing from the schema"
            );
        }
    }

    // ── 3. the counts, against each struct's own FIELD_NAMES ─────────────────────────────────
    for (label, fields, transient, node) in [
        (
            "RouterSettings",
            RouterSettings::FIELD_NAMES,
            schema::TRANSIENT_ROUTER_SETTINGS_FIELDS,
            &root,
        ),
        (
            "FanoutSettings",
            FanoutSettings::FIELD_NAMES,
            schema::TRANSIENT_FANOUT_FIELDS,
            &root["properties"]["fanout"],
        ),
        (
            "OptimizerSettings",
            OptimizerSettings::FIELD_NAMES,
            schema::TRANSIENT_OPTIMIZER_FIELDS,
            &root["properties"]["optimizer"],
        ),
        (
            "ScoringSettings",
            ScoringSettings::FIELD_NAMES,
            schema::TRANSIENT_SCORING_FIELDS,
            &root["properties"]["scoring"],
        ),
        (
            "LayerSettings",
            LayerSettings::FIELD_NAMES,
            schema::TRANSIENT_LAYER_FIELDS,
            &root["properties"]["layers"]["items"],
        ),
    ] {
        for name in transient {
            assert!(
                fields.contains(name),
                "{label}'s transient list names `{name}`, which is not one of its fields"
            );
        }
        assert_eq!(
            properties(node).len(),
            fields.len() - transient.len(),
            "{label}: the schema publishes {:?} for fields {fields:?} minus transient {transient:?}",
            properties(node)
        );
    }
}

/// A JVM checkout is needed for every board-driven test below.
fn dsn(relative: &str) -> String {
    parity::java_dir().join(relative).display().to_string()
}

/// **The end-to-end conversation** the task brief asks for, over spawned pipes: `initialize` →
/// `notifications/initialized` → `tools/list` → all four tools → the schema snapshot.
///
/// Two rungs tie the tools to the two whole-program gates:
///
/// * `route_board`'s session is **byte-identical to `p8t1`'s** `cli-tutorial_board/route.ses`,
///   after the same quirk #92 keyword rewrite `p8t1` and `cli_e2e.rs` apply to the jar side. That
///   is the measurement that says ruling AU's sparse composition and `resolve_headless` agree on
///   a real board — two different settings compositions, one SES.
/// * `check_drc`'s report is **byte-identical to the one `freerouting drc` writes**, which is
///   `p8t3 e2e`'s port lane, so the tool inherits that gate's comparison against the jar. Its
///   `violations` array is additionally compared against the committed **jar** reference.
#[test]
fn the_four_tools_over_spawned_pipes() {
    if !parity::require_java_dir() {
        return;
    }
    let board = dsn("examples/tutorial_board/tutorial_board.dsn");
    let mut pipes = Pipes::start();

    // ── initialize ───────────────────────────────────────────────────────────────────────────
    let init = pipes.request(
        1,
        "initialize",
        json!({"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "t", "version": "0"}}),
    );
    let result = &init["result"];
    assert_eq!(result["protocolVersion"], "2025-06-18");
    assert_eq!(result["serverInfo"]["name"], "freerouting");
    assert_eq!(result["capabilities"]["tools"]["listChanged"], false);
    // Delta row 1: Java's two non-spec top-level keys are absent.
    assert!(result.get("serverName").is_none() && result.get("serverVersion").is_none());

    pipes.write(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));

    // ── tools/list: four tools, flat schemas, matching the golden ────────────────────────────
    let list = pipes.request(2, "tools/list", json!({}));
    let tools = list["result"]["tools"].as_array().expect("an array");
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    // Delta row 10: **four**, not the jar's 28.
    assert_eq!(
        names,
        vec!["board_info", "check_drc", "list_settings", "route_board"]
    );
    for tool in tools {
        assert!(
            !tool["description"].as_str().unwrap_or_default().is_empty(),
            "{} has no description",
            tool["name"]
        );
        let schema = &tool["inputSchema"];
        assert_eq!(schema["type"], "object");
        // Ruling AO: **flat**. The jar's 24 generated tools publish `{path, query, body}`.
        for wrapper in ["path", "query", "body"] {
            assert!(
                schema["properties"].get(wrapper).is_none(),
                "{} publishes the jar's {wrapper} wrapper",
                tool["name"]
            );
        }
        assert_eq!(schema, &all_schemas()[tool["name"].as_str().unwrap()]);
    }

    // ── route_board: the SES is p8t1's, byte for byte ────────────────────────────────────────
    let routed = pipes.call(3, "route_board", json!({"dsn_path": board}));
    let expected = std::fs::read_to_string(parity::cli_reference("tutorial_board", "route.ses"))
        .expect("the p8t1 reference");
    let expected = parity::normalize_ses_head_tokens(&expected);
    let actual = routed["ses_text"].as_str().expect("ses_text");
    assert_eq!(
        actual, expected,
        "route_board's session differs from p8t1's cli-tutorial_board/route.ses"
    );
    // The `BoardFilePayload` members, under Java's own names (ruling AO).
    assert_eq!(routed["size"].as_u64(), Some(actual.len() as u64));
    assert_eq!(routed["format"], "SES");
    assert_eq!(routed["filename"], "tutorial_board.ses");
    assert!(routed["crc32"].is_number() && routed["path"].is_string());
    // `data` is offered only when the caller asked for text, and it is that text's Base64.
    assert_eq!(
        routed["data"].as_str().expect("data"),
        base64_reference(actual.as_bytes())
    );
    assert!(routed.get("ses_path").is_none(), "no output_path was given");
    // A real job id, from std-only entropy (ruling BC).
    let job_id = routed["job_id"].as_str().expect("job_id");
    assert_eq!(job_id.len(), 36);
    assert_ne!(job_id, "00000000-0000-0000-0000-000000000000");
    // Spec §13's own four members.
    assert_eq!(routed["incompletes"], 0);
    assert_eq!(routed["drc_violation_count"], 0);
    assert_eq!(routed["timed_out"], false);
    assert!(routed["stats"]["nets"]["total_count"].is_number());

    // …and the same tool on a KiCad **design JSON** answers `p8t1`'s `kicad-ecc83-json` session,
    // also byte for byte. The two inputs matter separately: the DSN path proves the sparse
    // composition, and the JSON path proves the **output format is pinned to SES** — left to the
    // input's own extension it would take the KiCad-session-JSON arm and hand back the board as
    // loaded, before any routing (quirk #289, label T). See `route_board`'s step 14.
    let kicad = pipes.call(
        7,
        "route_board",
        json!({"dsn_path": dsn("fixtures/Issue649-kicad_ecc83-pp_input_board_v1.json")}),
    );
    let expected = std::fs::read_to_string(parity::cli_reference("kicad-ecc83-json", "route.ses"))
        .expect("the p8t1 reference");
    assert_eq!(
        kicad["ses_text"].as_str().expect("ses_text"),
        parity::normalize_ses_head_tokens(&expected),
        "route_board on a KiCad design JSON differs from p8t1's cli-kicad-ecc83-json/route.ses"
    );
    assert_eq!(kicad["format"], "SES");
    assert!(
        kicad["filename"]
            .as_str()
            .is_some_and(|name| name.ends_with(".ses")),
        "the session is named as a session: {}",
        kicad["filename"]
    );

    // ── check_drc: the report is the one `freerouting drc` writes ────────────────────────────
    let mut report = pipes.call(4, "check_drc", json!({"dsn_path": board}));
    let cli_report = drc_through_the_binary(&board);
    let mut cli_report = cli_report;
    // The one field that cannot be equal: `date` is a wall clock, which both parity normalisers
    // drop for the same reason (plan-5 ruling 3).
    report.as_object_mut().expect("an object").remove("date");
    cli_report
        .as_object_mut()
        .expect("an object")
        .remove("date");
    assert_eq!(
        report, cli_report,
        "check_drc's report differs from the one `freerouting drc` writes"
    );
    // …and its violations are the jar's, from the committed p8t3 reference (`violations` is
    // spelled the same in both flavors — `fr_drc::report::json`'s `FlavorKeys`).
    let jar: Value = serde_json::from_str(
        &std::fs::read_to_string(parity::reference("drc-tutorial-board", "drc.json"))
            .expect("the drc reference"),
    )
    .expect("the reference is JSON");
    assert_eq!(report["violations"], jar["violations"]);

    // ── board_info ───────────────────────────────────────────────────────────────────────────
    let info = pipes.call(5, "board_info", json!({"dsn_path": board}));
    assert_eq!(
        info["layers"]
            .as_array()
            .expect("layers")
            .iter()
            .map(|l| l["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["F.Cu", "B.Cu"]
    );
    assert_eq!(info["metadata"]["host_cad"], "KiCad's Pcbnew");
    assert_eq!(info["metadata"]["unit"], "um");
    // The counts come from `BoardStatistics` and nowhere else.
    assert_eq!(
        info["statistics"]["nets"]["total_count"].as_u64(),
        Some(info["nets"].as_array().expect("nets").len() as u64)
    );

    // ── list_settings ────────────────────────────────────────────────────────────────────────
    let settings = pipes.call(6, "list_settings", json!({}));
    assert_eq!(settings["schema"], all_schemas()["RouterSettings"]);
    // The defaults are `DefaultSettings`' own, resolved at call time — not literals in the
    // schema, which is why the golden above is machine-independent and this is not.
    assert_eq!(settings["defaults"]["max_passes"], 9999);
    assert!(settings["defaults"]["max_threads"].is_number());

    assert_eq!(pipes.finish(), 0, "EOF exits 0 — Freerouting.java:778-779");
}

/// `output_path` writes the session to disk and answers a path instead of the bytes — spec §13's
/// "keeping large SES bodies out of the model context unless text is requested".
#[test]
fn an_output_path_answers_a_path_and_no_body() {
    if !parity::require_java_dir() {
        return;
    }
    let dir = std::env::temp_dir().join("fr-mcp-route-out");
    let _ = std::fs::create_dir_all(&dir);
    let out = dir.join("routed.ses");
    let _ = std::fs::remove_file(&out);

    let mut pipes = Pipes::start();
    let routed = pipes.call(
        1,
        "route_board",
        json!({
            "dsn_path": dsn("fixtures/Issue143-rpi_splitter.dsn"),
            "output_path": out.display().to_string(),
        }),
    );
    assert_eq!(pipes.finish(), 0);

    assert_eq!(routed["ses_path"], out.display().to_string());
    assert!(routed.get("ses_text").is_none());
    assert!(
        routed.get("data").is_none(),
        "no Base64 unless text is asked for"
    );
    let written = std::fs::read(&out).expect("the file was written");
    assert_eq!(routed["size"].as_u64(), Some(written.len() as u64));
    assert!(written.starts_with(b"(session "));
}

/// **Ruling AU's sparse tier, observed through the tool.**
///
/// Three independent observations that the `settings` argument lands at priority 70 and that a
/// nested object is merged *field by field* rather than replacing the tier:
///
/// 1. `{"enabled": false}` outranks `DefaultSettings`' `true` — the auto-routing stage is skipped
///    and only the fanout pre-pass's traces survive;
/// 2. `{"max_passes": 1}` outranks `DefaultSettings`' `9999` — a different, smaller board;
/// 3. `{"scoring": {"via_costs": 500}}` names **one** of `ScoringSettings`' eleven fields and the
///    run still completes with the full ratsnest routed. Had the sparse object replaced the tier,
///    `unrouted_net_penalty` and the rest would be `null` and the score the router steers by
///    could not be computed.
#[test]
fn a_sparse_settings_payload_composes_at_priority_70() {
    if !parity::require_java_dir() {
        return;
    }
    let board = dsn("fixtures/Issue143-rpi_splitter.dsn");
    let mut pipes = Pipes::start();
    let route = |pipes: &mut Pipes, id: i64, settings: Value| -> Value {
        let mut arguments = json!({"dsn_path": board});
        if !settings.is_null() {
            arguments["settings"] = settings;
        }
        pipes.call(id, "route_board", arguments)
    };

    let bare = route(&mut pipes, 1, Value::Null);
    let router_off = route(&mut pipes, 2, json!({"enabled": false}));
    let one_pass = route(&mut pipes, 3, json!({"max_passes": 1}));
    let via_costs = route(&mut pipes, 4, json!({"scoring": {"via_costs": 500}}));
    assert_eq!(pipes.finish(), 0);

    let wires = |v: &Value| v["ses_text"].as_str().unwrap().matches("(wire").count();
    // 1. The bare run routes the board; the override stops the auto-routing stage.
    assert_eq!(bare["incompletes"], 0);
    assert!(wires(&bare) > wires(&router_off));
    assert!(
        wires(&router_off) > 0,
        "the fanout pre-pass still ran; only the auto-router was disabled"
    );
    // 2. One pass leaves the board incomplete where the default 9999 does not.
    assert_ne!(one_pass["ses_text"], bare["ses_text"]);
    assert_ne!(one_pass["incompletes"], bare["incompletes"]);
    // 3. Naming one nested field did not null out the other ten.
    assert_eq!(via_costs["incompletes"], 0);
    assert_eq!(via_costs["timed_out"], false);
}

/// **Quirk #141**: Gson's reader is `Strictness.LENIENT` and accepts `NaN`/`Infinity` as number
/// literals; `serde_json` does not, so the whole line fails to parse and the port answers
/// `-32700` where the jar would have coerced a value into `RouterSettings`.
///
/// The refusal is at the **line**, not at the tool, and that is the honest place for it: the
/// transport parses whole lines, so a non-finite literal never reaches an argument. The port also
/// refuses to *write* one (`to_gson_string_pretty`, Gson's own `IllegalArgumentException`), which
/// is the "both ways" of the quirk.
///
/// A `settings` argument that is well-formed JSON but not a settings object is a *tool*-level
/// refusal instead — `isError`, with a message naming `list_settings`.
#[test]
fn a_non_finite_float_in_settings_is_refused() {
    let mut pipes = Pipes::start();

    // Gson reads this; `serde_json` does not, and the line never becomes a request. Written as
    // raw bytes because `serde_json` will not *produce* a non-finite literal either — the
    // "both ways" half of the quirk.
    pipes.stdin
        .write_all(
            br#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"route_board","arguments":{"dsn_text":"(pcb x)","settings":{"scoring":{"via_costs":NaN}}}}}
"#,
        )
        .expect("the server is still reading");
    pipes.stdin.flush().expect("the server is still reading");
    let refusal = pipes.read_response();
    assert_eq!(refusal["error"]["code"], -32700);
    // Delta row 9: the port's `-32700` carries `"id":null`; Java's has no `id` member at all.
    assert_eq!(refusal["id"], Value::Null);

    // …and a `settings` that is not an object is refused by the tool, not by the parser.
    let answer = pipes.request(
        2,
        "tools/call",
        json!({"name": "route_board", "arguments": {"dsn_text": "(pcb x)", "settings": 7}}),
    );
    assert_eq!(answer["result"]["isError"], true);
    let message = answer["result"]["content"][0]["text"].as_str().unwrap();
    assert!(message.contains("list_settings"), "{message}");

    assert_eq!(pipes.finish(), 0);
}

/// **Controller ruling BB's seam, and ruling AI's fourth site, end to end.**
///
/// The board is `Issue508-DAC2020_bm01.dsn`, whose **single** auto-routing pass takes 135 seconds
/// in a release build (measured, `--max-passes 1` with fanout and optimizer off). With ruling
/// BB's three pass-loop-head polls alone, a cancellation would wait that long; with ruling AI's
/// fourth site — the per-item loop, `AutoroutePassRunner.java:202-205` — it is observed in
/// milliseconds. This test is therefore also the measurement: it cannot pass in a reasonable time
/// unless the fourth site is there.
///
/// The cancellation is sent **after** the first `notifications/progress`, which is what makes
/// "mid-run" a fact rather than a hope: the tool cannot have reported progress before it started.
///
/// A cancelled run answers a **result**, not an error: it is a partial board, with `timed_out`
/// **false** — the job deadline is the only thing that sets that flag (`commands::route`'s step
/// 12 carries the measurement), and an operator's cancel is not a timeout.
#[test]
fn cancelling_route_board_mid_run_returns_timed_out_false_and_a_partial_result() {
    if !parity::require_java_dir() {
        return;
    }
    let mut pipes = Pipes::start();
    pipes.write(&json!({
        "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": {
            "name": "route_board",
            "arguments": {"dsn_path": dsn("fixtures/Issue508-DAC2020_bm01.dsn")},
            "_meta": {"progressToken": "tok"},
        },
    }));

    // The first progress notification proves the tool is running.
    let first = pipes.read();
    assert_eq!(
        first["method"], "notifications/progress",
        "the call answered before reporting any progress: {first}"
    );
    assert_eq!(first["params"]["progressToken"], "tok");
    assert!(first.get("id").is_none(), "a notification has no id");

    let cancelled_at = Instant::now();
    pipes.write(&json!({
        "jsonrpc": "2.0", "method": "notifications/cancelled",
        "params": {"requestId": 1, "reason": "the test asked"},
    }));
    let answer = pipes.read_response();
    let latency = cancelled_at.elapsed();
    assert_eq!(pipes.finish(), 0);

    // Ruling AI's site is what makes this bound hold: one pass of this board is 135 s.
    assert!(
        latency < Duration::from_secs(60),
        "the cancellation took {latency:?}; the per-item poll site is missing or ineffective"
    );
    let result = &answer["result"];
    assert_eq!(result["isError"], false, "a cancelled run is not an error");
    let content = &result["structuredContent"];
    assert_eq!(content["timed_out"], false, "a cancel is not a timeout");
    assert!(
        content["incompletes"].as_i64().unwrap_or(0) > 0,
        "the board is only partly routed"
    );
    assert!(
        content["ses_text"]
            .as_str()
            .unwrap()
            .starts_with("(session ")
    );
}

/// `freerouting drc <dsn>` to stdout, parsed — the document `p8t3 e2e` compares against the jar.
fn drc_through_the_binary(board: &str) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_freerouting"))
        .args(["drc", board])
        .output()
        .expect("the freerouting binary starts");
    assert!(output.status.success(), "drc exited {:?}", output.status);
    serde_json::from_slice(&output.stdout).expect("the DRC report is JSON")
}

/// A second, independent Base64 encoder, so the tool's answer is checked against something other
/// than the function that produced it. Deliberately the slow, obvious implementation: build the
/// whole bit string, take it six bits at a time, pad to a multiple of four.
fn base64_reference(bytes: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bits: String = bytes.iter().map(|b| format!("{b:08b}")).collect();
    let mut out = String::new();
    for chunk in bits.as_bytes().chunks(6) {
        let mut six = String::from_utf8(chunk.to_vec()).expect("ascii");
        while six.len() < 6 {
            six.push('0');
        }
        let index = usize::from_str_radix(&six, 2).expect("six bits");
        out.push(ALPHABET[index] as char);
    }
    while !out.len().is_multiple_of(4) {
        out.push('=');
    }
    out
}
