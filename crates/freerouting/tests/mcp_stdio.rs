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
    assert!(list["result"]["tools"].as_array().unwrap().is_empty());

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
