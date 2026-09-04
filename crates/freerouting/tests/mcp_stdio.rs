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


use freerouting::mcp::server::{State, ToolDef};
use freerouting::mcp::stdio::run_with;
use serde_json::{Value, json};
use std::io::Read;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

struct Script {
    lines: Receiver<String>,
    pending: Vec<u8>,
    at: usize,
            fail: bool,
                        eof: Arc<AtomicBool>,
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
                Err(_) => {
                    self.eof.store(true, Ordering::SeqCst);
                    return Ok(0);
                }
            }
        }
        let n = (self.pending.len() - self.at).min(out.len());
        out[..n].copy_from_slice(&self.pending[self.at..self.at + n]);
        self.at += n;
        Ok(n)
    }
}

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
        stdin_eof: Arc<AtomicBool>,
}

impl Harness {
    fn start(state: State) -> Harness {
        Harness::start_with(state, false, None)
    }

            fn start_with(state: State, fail_reads: bool, fail_writes_after: Option<usize>) -> Harness {
        let (tx, rx) = channel::<String>();
        let out = Captured(Arc::new(Mutex::new(Vec::new())), fail_writes_after);
        let stdin_eof = Arc::new(AtomicBool::new(false));
        let reader = std::io::BufReader::new(Script {
            lines: rx,
            pending: Vec::new(),
            at: 0,
            fail: fail_reads,
            eof: Arc::clone(&stdin_eof),
        });
        let writer = out.clone();
        let server = std::thread::spawn(move || run_with(state, reader, writer));
        Harness {
            lines: Some(tx),
            out,
            server: Some(server),
            stdin_eof,
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

        fn finish(mut self) -> (i32, Vec<String>) {
        drop(self.lines.take());
        let code = self.server.take().unwrap().join().unwrap();
        let text = String::from_utf8(self.out.0.lock().unwrap().clone()).unwrap();
        let lines = text.lines().map(str::to_string).collect();
        (code, lines)
    }
}

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
    assert_eq!(msgs[5]["id"], 7);
    assert_eq!(msgs[5]["result"]["isError"], false);
    assert_eq!(
        msgs[5]["result"]["structuredContent"],
        json!({"done": true})
    );
}

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
    assert_eq!(lines.len(), 1, "{lines:?}");
    let msgs = parsed(&lines);
    assert_eq!(msgs[0]["id"], "call-1");
    assert_eq!(
        msgs[0]["result"]["structuredContent"],
        json!({"cancelled": true})
    );
}

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
    assert_eq!(msgs.len(), 4, "{lines:?}");
    assert_eq!(msgs[0]["id"], 1);
    assert_eq!(msgs[0]["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(msgs[1]["id"], Value::Null);
    assert_eq!(msgs[1]["error"]["code"], -32700);
    assert_eq!(msgs[2]["method"], "notifications/progress");
    assert_eq!(msgs[2]["params"]["progressToken"], 42);
    assert_eq!(msgs[3]["id"], 2);
}

#[test]
fn eof_exits_zero() {
    let (code, lines) = Harness::start(State::new()).finish();
    assert_eq!(code, 0);
    assert!(lines.is_empty(), "{lines:?}");

    let (code, lines) = Harness::start_with(State::new(), true, None).finish();
    assert_eq!(code, 1);
    assert!(lines.is_empty(), "{lines:?}");
}

#[test]
fn eof_drains_an_in_flight_call_without_cancelling_it() {
    let started = Arc::new(AtomicBool::new(false));
    let saw_cancel = Arc::new(AtomicBool::new(false));

    let mut state = State::new();
    {
        let started = Arc::clone(&started);
        let saw_cancel = Arc::clone(&saw_cancel);
        state.register_tool(
            tool("slow"),
            Box::new(move |_state, _args, _progress, cancel| {
                started.store(true, Ordering::SeqCst);
                let until = Instant::now() + Duration::from_millis(300);
                while Instant::now() < until {
                    if cancel.is_cancelled() {
                        saw_cancel.store(true, Ordering::SeqCst);
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
                Ok(json!({"finished": true}))
            }),
        );
    }

    let h = Harness::start(state);
    h.send(json!({
        "jsonrpc": "2.0", "id": 4, "method": "tools/call",
        "params": {"name": "slow", "arguments": {}},
    }));
    wait_for("the call to start", || started.load(Ordering::SeqCst));
    let (code, lines) = h.finish();

    assert_eq!(code, 0, "EOF is Java's 0 — Freerouting.java:778-779");
    assert!(
        !saw_cancel.load(Ordering::SeqCst),
        "EOF must not cancel an in-flight call (ruling BH)"
    );
    let msgs = parsed(&lines);
    assert_eq!(msgs.len(), 1, "{lines:?}");
    assert_eq!(msgs[0]["id"], 4);
    assert_eq!(
        msgs[0]["result"]["structuredContent"],
        json!({"finished": true}),
        "the drained call answered in full"
    );
}

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
    h.send(json!({"jsonrpc": "2.0", "method": "tools/call",
                  "params": {"name": "counted", "arguments": {}}}));
    h.send(json!({"jsonrpc": "2.0", "id": 1, "method": "ping"}));

    let (code, lines) = h.finish();
    assert_eq!(code, 0);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert_eq!(parsed(&lines)[0]["id"], 1);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

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

    let mut h = Harness::start_with(state, false, Some(0));
    h.send(json!({
        "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": {"name": "spin", "arguments": {}},
    }));
    wait_for("the tool to start", || started.load(Ordering::SeqCst));
    h.send(json!({"jsonrpc": "2.0", "id": 2, "method": "ping"}));

    let code = h.server.take().unwrap().join().unwrap();
    assert_eq!(code, 1, "a peer that stopped reading is not a success");
    assert!(
        cancelled.load(Ordering::SeqCst),
        "the in-flight tool was left running for nobody"
    );
    assert!(h.out.0.lock().unwrap().is_empty());
    drop(h.lines.take());
}

#[test]
fn the_drain_takes_no_new_work_after_a_write_failure() {
    let started = Arc::new(AtomicBool::new(false));
    let spin_saw_cancel = Arc::new(AtomicBool::new(false));
    let release = Arc::new(AtomicBool::new(false));
    let second_calls = Arc::new(AtomicUsize::new(0));

    let mut state = State::new();
    {
        let started = Arc::clone(&started);
        let spin_saw_cancel = Arc::clone(&spin_saw_cancel);
        let release = Arc::clone(&release);
        state.register_tool(
            tool("spin"),
            Box::new(move |_state, _args, _progress, cancel| {
                started.store(true, Ordering::SeqCst);
                let deadline = Instant::now() + Duration::from_secs(10);
                while !release.load(Ordering::SeqCst) {
                    if cancel.is_cancelled() {
                        spin_saw_cancel.store(true, Ordering::SeqCst);
                    }
                    assert!(Instant::now() < deadline, "the test never released `spin`");
                    std::thread::sleep(Duration::from_millis(2));
                }
                Ok(json!({}))
            }),
        );
    }
    {
        let second_calls = Arc::clone(&second_calls);
        state.register_tool(
            tool("second"),
            Box::new(move |_state, _args, _progress, cancel| {
                second_calls.fetch_add(1, Ordering::SeqCst);
                let deadline = Instant::now() + Duration::from_secs(10);
                while !cancel.is_cancelled() {
                    assert!(
                        Instant::now() < deadline,
                        "`second` was spawned during the drain and nothing ever cancelled it"
                    );
                    std::thread::sleep(Duration::from_millis(2));
                }
                Ok(json!({}))
            }),
        );
    }

    let mut h = Harness::start_with(state, false, Some(0));
    h.send(json!({
        "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": {"name": "spin", "arguments": {}},
    }));
    wait_for("`spin` to start", || started.load(Ordering::SeqCst));

    h.send(json!({"jsonrpc": "2.0", "id": 2, "method": "ping"}));
    wait_for("the write failure to cancel the in-flight call", || {
        spin_saw_cancel.load(Ordering::SeqCst)
    });

    h.send(json!({
        "jsonrpc": "2.0", "id": 3, "method": "tools/call",
        "params": {"name": "second", "arguments": {}},
    }));

    drop(h.lines.take());
    wait_for("stdin to reach EOF", || h.stdin_eof.load(Ordering::SeqCst));

    release.store(true, Ordering::SeqCst);
    let code = h.server.take().unwrap().join().unwrap();

    assert_eq!(
        second_calls.load(Ordering::SeqCst),
        0,
        "a `tools/call` arriving after a write failure must not be spawned: the drain takes no \
         new work (run_with's shutdown table, README delta row 11)"
    );
    assert_eq!(
        code, 1,
        "a write failure exits 1, and a later EOF does not turn it into a success"
    );
    assert!(
        h.out.0.lock().unwrap().is_empty(),
        "the peer stopped reading before the first byte, so nothing was ever captured"
    );
}

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
    h.send(json!({
        "jsonrpc": "2.0", "method": "notifications/cancelled",
        "params": {"requestId": 9, "reason": "the test is done with it"},
    }));
    let (code, lines) = h.finish();

    assert_eq!(code, 0);
    let msgs = parsed(&lines);
    assert_eq!(msgs.len(), 2, "{lines:?}");
    assert_eq!(msgs[0]["id"], 9);
    assert_eq!(msgs[0]["error"]["code"], -32600);
    assert!(msgs[0].get("result").is_none());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        msgs[1]["result"]["structuredContent"],
        json!({"first": true})
    );
}


use std::process::{Child, ChildStdin, ChildStdout};

struct Pipes {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl Pipes {
    fn start() -> Pipes {
        Pipes::start_with_env(&[])
    }

                    fn start_with_env(env: &[(&str, &str)]) -> Pipes {
        let mut command = Command::new(env!("CARGO_BIN_EXE_freerouting"));
        command.arg("mcp");
        for (key, value) in env {
            command.env(key, value);
        }
        let mut child = command
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

        fn write(&mut self, message: &Value) {
        writeln!(self.stdin, "{message}").expect("the server is still reading");
        self.stdin.flush().expect("the server is still reading");
    }

        fn read(&mut self) -> Value {
        let mut line = String::new();
        let read = self
            .stdout
            .read_line(&mut line)
            .expect("stdout is readable");
        assert!(read > 0, "the server closed stdout before answering");
        serde_json::from_str(&line).unwrap_or_else(|e| panic!("not JSON: {line:?} ({e})"))
    }

            fn read_response(&mut self) -> Value {
        loop {
            let message = self.read();
            if message.get("id").is_some() {
                return message;
            }
        }
    }

        fn request(&mut self, id: i64, method: &str, params: Value) -> Value {
        self.write(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        let answer = self.read_response();
        assert_eq!(answer["id"], id, "responses are paired by id");
        answer
    }

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

        fn finish(mut self) -> i32 {
        drop(self.stdin);
        self.child
            .wait()
            .expect("the server exits")
            .code()
            .unwrap_or(-1)
    }
}

fn golden_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("data")
        .join("mcp-schemas.json")
}

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

#[test]
fn an_unreadable_router_budget_is_an_error_and_the_server_survives() {
    if !parity::require_java_dir() {
        return;
    }
    let board = dsn("fixtures/empty_board.dsn");
    let mut pipes = Pipes::start_with_env(&[("FR_ROUTER_BUDGET", "banana")]);

    pipes.request(
        1,
        "initialize",
        json!({"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "t", "version": "0"}}),
    );
    pipes.write(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));

    let answer = pipes.request(
        2,
        "tools/call",
        json!({"name": "route_board", "arguments": {"dsn_path": board}}),
    );
    assert_eq!(
        answer["result"]["isError"], true,
        "an unreadable FR_ROUTER_BUDGET must refuse the call: {answer}"
    );
    let message = answer["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default();
    assert!(
        message.contains("FR_ROUTER_BUDGET") && message.contains("banana"),
        "the refusal must name the variable and the value the operator set: {message}"
    );
    assert!(
        message.contains("disabled"),
        "…and the values it would have accepted: {message}"
    );

    assert_eq!(pipes.request(3, "ping", json!({}))["result"], json!({}));
    assert_tool_list(&pipes.request(4, "tools/list", json!({}))["result"]["tools"]);

    assert_eq!(
        pipes.finish(),
        0,
        "EOF exits 0 — the server was never killed by the env var"
    );
}

fn dsn(relative: &str) -> String {
    parity::java_dir().join(relative).display().to_string()
}

#[test]
fn the_tool_list_and_schemas_over_spawned_pipes() {
    let mut pipes = Pipes::start();

    let init = pipes.request(
        1,
        "initialize",
        json!({"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "t", "version": "0"}}),
    );
    let result = &init["result"];
    assert_eq!(result["protocolVersion"], "2025-06-18");
    assert_eq!(result["serverInfo"]["name"], "freerouting");
    assert_eq!(result["capabilities"]["tools"]["listChanged"], false);
    assert!(result.get("serverName").is_none() && result.get("serverVersion").is_none());

    pipes.write(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));

    let list = pipes.request(2, "tools/list", json!({}));
    assert_tool_list(&list["result"]["tools"]);

    let settings = pipes.call(3, "list_settings", json!({}));
    assert_eq!(settings["schema"], all_schemas()["RouterSettings"]);
    assert_eq!(settings["defaults"]["max_passes"], 9999);
    assert!(settings["defaults"]["max_threads"].is_number());

    assert_eq!(pipes.finish(), 0, "EOF exits 0 — Freerouting.java:778-779");
}

fn assert_tool_list(tools: &Value) {
    let tools = tools.as_array().expect("an array");
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
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
        for wrapper in ["path", "query", "body"] {
            assert!(
                schema["properties"].get(wrapper).is_none(),
                "{} publishes the jar's {wrapper} wrapper",
                tool["name"]
            );
        }
        assert_eq!(schema, &all_schemas()[tool["name"].as_str().unwrap()]);
    }
}

#[test]
#[cfg_attr(debug_assertions, ignore)]
fn the_four_tools_over_spawned_pipes() {
    if std::env::var_os("FR_SLOW_PARITY").is_none() {
        eprintln!("SKIP: set FR_SLOW_PARITY=1 to run the MCP board lane");
        return;
    }
    if !parity::require_java_dir() {
        return;
    }
    let board = dsn("examples/tutorial_board/tutorial_board.dsn");
    let mut pipes = Pipes::start();

    let init = pipes.request(
        1,
        "initialize",
        json!({"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "t", "version": "0"}}),
    );
    let result = &init["result"];
    assert_eq!(result["protocolVersion"], "2025-06-18");
    assert_eq!(result["serverInfo"]["name"], "freerouting");
    assert_eq!(result["capabilities"]["tools"]["listChanged"], false);
    assert!(result.get("serverName").is_none() && result.get("serverVersion").is_none());

    pipes.write(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));

    let list = pipes.request(2, "tools/list", json!({}));
    assert_tool_list(&list["result"]["tools"]);

    let routed = pipes.call(3, "route_board", json!({"dsn_path": board}));
    let expected = std::fs::read_to_string(parity::cli_reference("tutorial_board", "route.ses"))
        .expect("the p8t1 reference");
    let expected = parity::normalize_ses_head_tokens(&expected);
    let actual = routed["ses_text"].as_str().expect("ses_text");
    assert_eq!(
        actual, expected,
        "route_board's session differs from p8t1's cli-tutorial_board/route.ses"
    );
    assert_eq!(routed["size"].as_u64(), Some(actual.len() as u64));
    assert_eq!(routed["format"], "SES");
    assert_eq!(routed["filename"], "tutorial_board.ses");
    assert!(routed["crc32"].is_number() && routed["path"].is_string());
    assert_eq!(
        routed["data"].as_str().expect("data"),
        base64_reference(actual.as_bytes())
    );
    assert!(routed.get("ses_path").is_none(), "no output_path was given");
    let job_id = routed["job_id"].as_str().expect("job_id");
    assert_eq!(job_id.len(), 36);
    assert_ne!(job_id, "00000000-0000-0000-0000-000000000000");
    assert_eq!(routed["incompletes"], 0);
    assert_eq!(routed["drc_violation_count"], 0);
    assert_eq!(routed["timed_out"], false);
    assert!(routed["stats"]["nets"]["total_count"].is_number());

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

    let mut report = pipes.call(4, "check_drc", json!({"dsn_path": board}));
    let cli_report = drc_through_the_binary(&board);
    let mut cli_report = cli_report;
    report.as_object_mut().expect("an object").remove("date");
    cli_report
        .as_object_mut()
        .expect("an object")
        .remove("date");
    assert_eq!(
        report, cli_report,
        "check_drc's report differs from the one `freerouting drc` writes"
    );
    let jar: Value = serde_json::from_str(
        &std::fs::read_to_string(parity::reference("drc-tutorial-board", "drc.json"))
            .expect("the drc reference"),
    )
    .expect("the reference is JSON");
    assert_eq!(report["violations"], jar["violations"]);

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
    assert_eq!(
        info["statistics"]["nets"]["total_count"].as_u64(),
        Some(info["nets"].as_array().expect("nets").len() as u64)
    );

    let settings = pipes.call(6, "list_settings", json!({}));
    assert_eq!(settings["schema"], all_schemas()["RouterSettings"]);
    assert_eq!(settings["defaults"]["max_passes"], 9999);
    assert!(settings["defaults"]["max_threads"].is_number());

    assert_eq!(pipes.finish(), 0, "EOF exits 0 — Freerouting.java:778-779");
}

#[test]
fn an_output_path_answers_a_path_and_no_body() {
    if !parity::require_java_dir() {
        return;
    }
    let dir = std::env::temp_dir()
        .join("fr-mcp-stdio")
        .join("an_output_path_answers_a_path_and_no_body");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let out = dir.join("routed.ses");

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

#[test]
fn a_sparse_settings_payload_composes_at_priority_70() {
    if !parity::require_java_dir() {
        return;
    }
    let board = dsn("fixtures/Issue143-rpi_splitter.dsn");
    let slow_board = dsn("fixtures/Issue733-kicad_complex_hierarchy_input_design.json");
    let mut pipes = Pipes::start();
    let route = |pipes: &mut Pipes, id: i64, board: &str, settings: Value| -> Value {
        let mut arguments = json!({"dsn_path": board});
        if !settings.is_null() {
            arguments["settings"] = settings;
        }
        pipes.call(id, "route_board", arguments)
    };

    let bare = route(&mut pipes, 1, &board, Value::Null);
    let router_off = route(&mut pipes, 2, &board, json!({"enabled": false}));
    let slow_bare = route(&mut pipes, 3, &slow_board, Value::Null);
    let one_pass = route(&mut pipes, 4, &slow_board, json!({"max_passes": 1}));
    let via_costs = route(
        &mut pipes,
        5,
        &board,
        json!({"scoring": {"via_costs": 500}}),
    );
    assert_eq!(pipes.finish(), 0);

    let wires = |v: &Value| v["ses_text"].as_str().unwrap().matches("(wire").count();
    let length = |v: &Value| {
        v["stats"]["traces"]["total_length_mm"]
            .as_f64()
            .expect("stats.traces.total_length_mm")
    };
    let vias = |v: &Value| {
        v["stats"]["vias"]["total_count"]
            .as_u64()
            .expect("stats.vias.total_count")
    };
    assert_eq!(bare["incompletes"], 0);
    assert!(
        length(&bare) > length(&router_off),
        "the auto-routing stage lays copper the fanout pre-pass did not: {} mm vs {} mm",
        length(&bare),
        length(&router_off)
    );
    assert!(
        vias(&bare) > vias(&router_off),
        "…and places vias the pre-pass did not: {} vs {}",
        vias(&bare),
        vias(&router_off)
    );
    assert!(
        wires(&router_off) > 0,
        "the fanout pre-pass still ran; only the auto-router was disabled"
    );
    assert_ne!(
        one_pass["ses_text"], slow_bare["ses_text"],
        "max_passes = 1 must reach the pass loop — an ignored setting would route the same board"
    );
    assert!(
        one_pass["incompletes"].as_u64().expect("a count")
            >= slow_bare["incompletes"].as_u64().expect("a count"),
        "one routing pass cannot leave *fewer* connections open than the default budget: {} at \
         one pass against {} at the default",
        one_pass["incompletes"],
        slow_bare["incompletes"]
    );
    assert_eq!(via_costs["incompletes"], 0);
    assert_eq!(via_costs["timed_out"], false);
}

#[test]
fn a_non_finite_float_in_settings_is_refused() {
    let mut pipes = Pipes::start();

    pipes.stdin
        .write_all(
            br#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"route_board","arguments":{"dsn_text":"(pcb x)","settings":{"scoring":{"via_costs":NaN}}}}}
"#,
        )
        .expect("the server is still reading");
    pipes.stdin.flush().expect("the server is still reading");
    let refusal = pipes.read_response();
    assert_eq!(refusal["error"]["code"], -32700);
    assert_eq!(refusal["id"], Value::Null);

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

fn drc_through_the_binary(board: &str) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_freerouting"))
        .args(["drc", board])
        .output()
        .expect("the freerouting binary starts");
    assert!(output.status.success(), "drc exited {:?}", output.status);
    serde_json::from_slice(&output.stdout).expect("the DRC report is JSON")
}

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
