//! `p8t6` — Plan 8 Task 12's **documented-delta driver** (controller ruling AO).
//!
//! The jar has an MCP server and so does the port, and controller ruling AO replaced the jar's
//! arrangement (a stdio pump POSTing each line to a Jetty server in the same JVM) with a native
//! in-process one. The two are therefore **not** expected to agree — but *where* they disagree is
//! a contract, written down as the table in `crates/freerouting/README.md`, and this driver is
//! what turns that table into an assertion.
//!
//! ```text
//! scripts/differential/run.sh p8t6            # the whole table
//! scripts/differential/run.sh p8t6 verbose    # …and print both transcripts
//! ```
//!
//! # The two failure modes it exists for
//!
//! * **A recorded delta that disappears is a failure.** Every row below names a jar value and a
//!   port value, and the driver requires them to actually differ. A row that has quietly become
//!   the same is a table that no longer describes the programs.
//! * **A new delta is a failure.** The rows are not the only observations: the driver also makes
//!   a set of **agreement** observations — the framing, the blank-input-line skip, the unknown-tool
//!   error, the `isError` member, the EOF exit code — and requires those to be *equal*. That is
//!   what makes "exactly this list" checkable rather than aspirational.
//!
//! # Why there is no `P8T6.java`
//!
//! Every driver in `scripts/differential/` is a Java/Rust pair whose two transcripts `run.sh`
//! diffs, because the thing under test is a Java *method* that has to be called from inside a
//! JVM. Here the thing under test is **the jar as a program**: R18's launch is a `java -jar` line
//! with five flags and a pipe, and a Java class could only re-implement this delta table a second
//! time in a second language — after which the two copies could agree with each other while both
//! being wrong, and `diff` would report a MATCH. The same argument is written at the head of
//! `p8t1.rs` and in `p8t3.rs`'s `e2e` mode, and both are `rust_only` for it. Recorded in the Task
//! 12 report as a deviation from the brief's file list.
//!
//! # Scan ruling R18 — the launch, verbatim, and no `SKIP`
//!
//! ```text
//! java -Djava.awt.headless=true -Duser.language=en -Duser.country=US \
//!   -jar <HEAD jar> --api_server.enabled=true --api_server.authentication.enabled=false \
//!   --mcp_server.authentication.enabled=false --mcp_server.enabled=true \
//!   --mcp_server.stdio=true --user_data_path=<scratch>
//! ```
//!
//! `--mcp_server.stdio=true` **alone does not start the bridge** — `McpServerSettings.isEnabled`
//! defaults to `false` — and `--api_server.enabled=true` is effectively mandatory because every
//! generated tool is an HTTP call into the REST API, whose own `enabled` also defaults to
//! `false`. A missing jar is an **error**, not a skip.
//!
//! Row 7 (authentication) needs a **second** jar run, without the two `authentication.enabled`
//! flags, because the whole point of the row is what happens by default; that run calls one
//! stateful tool and reads the `401` back out of the envelope. It is job 3's run B.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::time::Duration;

/// One line the server wrote. A blank line is **not** nothing: it is the jar's answer to a
/// notification (delta row 8), so it has to be representable.
#[derive(Debug, Clone, PartialEq)]
enum Line {
    Blank,
    /// The parsed value **and the raw text**. The raw text is not redundant: delta row 5 is about
    /// the *whitespace* the jar leaves in a line, which re-serialising the value would erase.
    Json(serde_json::Value, String),
    /// A line that is not JSON at all. Nothing produces one today; it exists so that a future
    /// regression shows up as a row rather than as a parse panic.
    Raw(String),
}

impl Line {
    fn json(&self) -> Option<&serde_json::Value> {
        match self {
            Line::Json(value, _) => Some(value),
            _ => None,
        }
    }
    /// The bytes the server actually wrote.
    fn text(&self) -> String {
        match self {
            Line::Blank => String::new(),
            Line::Json(_, raw) => raw.clone(),
            Line::Raw(text) => text.clone(),
        }
    }
}

/// A live conversation with one of the two servers.
struct Server {
    child: Child,
    stdin: Option<ChildStdin>,
    lines: Receiver<Option<String>>,
    /// Everything read, in order, including the blanks.
    seen: Vec<Line>,
}

impl Server {
    fn start(mut command: Command) -> Server {
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("the server starts");
        let stdin = child.stdin.take().expect("piped stdin");
        let stdout = child.stdout.take().expect("piped stdout");
        let (tx, lines) = channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                match line {
                    Ok(line) => {
                        if tx.send(Some(line)).is_err() {
                            return;
                        }
                    }
                    Err(_) => break,
                }
            }
            let _ = tx.send(None);
        });
        Server {
            child,
            stdin: Some(stdin),
            lines,
            seen: Vec::new(),
        }
    }

    fn write(&mut self, text: &str) {
        let stdin = self.stdin.as_mut().expect("stdin is still open");
        writeln!(stdin, "{text}").expect("the server is still reading");
        stdin.flush().expect("the server is still reading");
    }

    fn send(&mut self, message: &serde_json::Value) {
        self.write(&message.to_string());
    }

    /// The next line, or `None` if none arrives within `timeout` (which is how "the port answers
    /// nothing" is observed).
    fn next(&mut self, timeout: Duration) -> Option<Line> {
        match self.lines.recv_timeout(timeout) {
            Ok(Some(text)) => {
                let line = if text.trim().is_empty() {
                    Line::Blank
                } else {
                    match serde_json::from_str(&text) {
                        Ok(value) => Line::Json(value, text),
                        Err(_) => Line::Raw(text),
                    }
                };
                self.seen.push(line.clone());
                Some(line)
            }
            Ok(None) | Err(RecvTimeoutError::Disconnected) => None,
            Err(RecvTimeoutError::Timeout) => None,
        }
    }

    /// The next line that carries a response `id`, collecting the notifications that precede it.
    fn response(&mut self, timeout: Duration) -> (Option<serde_json::Value>, Vec<serde_json::Value>) {
        let mut notifications = Vec::new();
        loop {
            let Some(line) = self.next(timeout) else {
                return (None, notifications);
            };
            match line.json() {
                Some(value) if value.get("id").is_some() => return (Some(value.clone()), notifications),
                Some(value) if value.get("method").is_some() => notifications.push(value.clone()),
                _ => {}
            }
        }
    }

    /// Closes stdin and waits; the answer is the exit code.
    fn finish(mut self) -> i32 {
        drop(self.stdin.take());
        self.child
            .wait()
            .map(|status| status.code().unwrap_or(-1))
            .unwrap_or(-1)
    }
}

/// One row of the verdict table.
struct Row {
    number: &'static str,
    what: &'static str,
    /// `Delta` requires the two values to differ; `Agree` requires them to be equal.
    kind: Kind,
    jar: String,
    port: String,
}

#[derive(PartialEq)]
enum Kind {
    Delta,
    Agree,
}

impl Row {
    fn ok(&self) -> bool {
        match self.kind {
            Kind::Delta => self.jar != self.port,
            Kind::Agree => self.jar == self.port,
        }
    }
}

/// The Java checkout the fixtures live in — `run.sh` exports it.
fn java_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env_path(
        "FREEROUTING_JAVA_DIR",
        "../freerouting",
    ))
}

fn env_path(key: &str, fallback: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| fallback.to_string())
}

/// R18's launch line, verbatim. `auth` selects the two `authentication.enabled=false` flags: the
/// main run passes them (so the generated tools work at all) and the row-7 run does not.
fn jar_command(scratch: &std::path::Path, auth_off: bool) -> Command {
    let jar = env_path("FREEROUTING_JAR", "");
    assert!(
        !jar.is_empty() && std::path::Path::new(&jar).is_file(),
        "the HEAD jar is required (FREEROUTING_JAR={jar}); a missing jar is a defect, not a skip \
         (scan ruling R18)"
    );
    let java = env_path("JAVA", "/opt/homebrew/opt/openjdk@25/bin/java");
    let mut command = Command::new(java);
    command
        .arg("-Djava.awt.headless=true")
        .arg("-Duser.language=en")
        .arg("-Duser.country=US")
        .arg("-jar")
        .arg(&jar)
        .arg("--api_server.enabled=true");
    if auth_off {
        command
            .arg("--api_server.authentication.enabled=false")
            .arg("--mcp_server.authentication.enabled=false");
    }
    command
        .arg("--mcp_server.enabled=true")
        .arg("--mcp_server.stdio=true")
        .arg(format!("--user_data_path={}", scratch.display()));
    command
}

fn port_command() -> Command {
    let binary = env_path("FREEROUTING_BIN", "target/release/freerouting");
    let mut command = Command::new(binary);
    command.arg("mcp");
    command
}

/// The jar takes seconds to boot two Jetty servers and re-scans its whole OpenAPI surface on
/// every request; the port answers in microseconds. One generous bound for both.
const SLOW: Duration = Duration::from_secs(120);
/// The bound for "nothing is coming" — long enough that a slow answer is not mistaken for
/// silence, short enough that the driver is not the slowest thing in the suite.
const SILENCE: Duration = Duration::from_secs(5);

fn initialize() -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {"protocolVersion": "2025-06-18", "capabilities": {},
                   "clientInfo": {"name": "p8t6", "version": "0"}},
    })
}

/// What one server answered to the shared script.
struct Observed {
    protocol_version: String,
    server_name_keys: String,
    server_info_name: String,
    capabilities: String,
    ping: String,
    tool_count: usize,
    tool_shape: String,
    call_has_structured: String,
    call_has_is_error: String,
    call_progress_notifications: usize,
    notification_reply: String,
    parse_error_id: String,
    parse_error_pretty: String,
    blank_line_reply: String,
    unknown_tool: String,
    every_line_is_one_object: String,
    exit_code: i32,
    /// Every line the server wrote, in order, for `verbose`.
    transcript: Vec<String>,
}

/// Drives one server through the shared script. `call_tool` is the no-argument tool that server
/// has — the two names differ because the **tool sets are disjoint**, which is delta row 10
/// itself; there is no tool both programs offer.
fn observe(
    command: Command,
    call_tool: &str,
    call_arguments: serde_json::Value,
    progress_call: (&str, serde_json::Value),
) -> Observed {
    let mut server = Server::start(command);

    // 1. initialize.
    server.send(&initialize());
    let (init, _) = server.response(SLOW);
    let init = init.expect("initialize is answered");
    let result = init["result"].clone();

    // 2. a notification — delta row 8.
    server.send(&serde_json::json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
    let notification_reply = match server.next(SILENCE) {
        None => "nothing".to_string(),
        Some(Line::Blank) => "a blank line".to_string(),
        Some(other) => format!("a line: {}", other.text()),
    };

    // 3. ping — delta row 4.
    server.send(&serde_json::json!({"jsonrpc": "2.0", "id": 2, "method": "ping"}));
    let (ping, _) = server.response(SLOW);
    let ping = ping.expect("ping is answered one way or the other");
    let ping = match ping.get("error") {
        Some(error) => format!("error {}", error["code"]),
        None => format!("result {}", ping["result"]),
    };

    // 4. tools/list — delta row 10.
    server.send(&serde_json::json!({"jsonrpc": "2.0", "id": 3, "method": "tools/list"}));
    let (list, _) = server.response(SLOW);
    let tools = list.expect("tools/list is answered")["result"]["tools"]
        .as_array()
        .expect("an array of tools")
        .clone();
    let tool_count = tools.len();
    // The **input shape**: `{path, query, body}` wrappers versus flat arguments.
    let wrapped = tools
        .iter()
        .filter(|tool| {
            let properties = &tool["inputSchema"]["properties"];
            ["path", "query", "body"]
                .iter()
                .any(|bucket| properties.get(bucket).is_some())
        })
        .count();
    let tool_shape = format!("{wrapped} of {tool_count} wrapped in {{path,query,body}}");

    // 5. one `tools/call`, carrying a progress token — delta rows 3 and 6.
    server.send(&serde_json::json!({
        "jsonrpc": "2.0", "id": 4, "method": "tools/call",
        "params": {"name": call_tool, "arguments": call_arguments,
                   "_meta": {"progressToken": "p8t6"}},
    }));
    let (call, notifications) = server.response(SLOW);
    let call = call.expect("the tool call is answered");
    let call_result = call["result"].clone();
    let call_has_structured = call_result
        .get("structuredContent")
        .map_or("absent", |_| "present")
        .to_string();
    let call_has_is_error = call_result
        .get("isError")
        .map_or("absent", |_| "present")
        .to_string();
    let _ = &notifications;

    // 5b. the **longest-running tool each program has**, again with a progress token — delta row
    // 6. The two tools differ, and they have to: the tool sets are disjoint (that is delta row 10
    // itself), so there is no call both programs can be given. What is compared is therefore not
    // one call but each program's *best case*: the jar is asked for its slowest reachable stdio
    // tool and the port for the one tool that routes a board, and the jar still reports nothing —
    // because its controller returns a single `JsonObject` (`McpControllerV1.java:185-198`) and
    // its bridge is one blocking round trip per line (`Freerouting.java:749-776`), so no interim
    // message is expressible at all.
    server.send(&serde_json::json!({
        "jsonrpc": "2.0", "id": 7, "method": "tools/call",
        "params": {"name": progress_call.0, "arguments": progress_call.1,
                   "_meta": {"progressToken": "p8t6-progress"}},
    }));
    let (progress_answer, progress_notifications) = server.response(SLOW);
    assert!(progress_answer.is_some(), "the progress call is answered");
    let call_progress_notifications = progress_notifications
        .iter()
        .filter(|n| n["method"] == "notifications/progress")
        .count();

    // 6. a malformed line — delta rows 5 and 9.
    server.write(r#"{"jsonrpc":"2.0","id":8,"method":"tools/list""#);
    let parse_line = server.next(SLOW).expect("the parse error is answered");
    let parse_text = parse_line.text();
    let parse_error_id = match parse_line.json() {
        Some(value) if value.get("id").is_some() => format!("id: {}", value["id"]),
        Some(_) => "no id member".to_string(),
        None => "not JSON".to_string(),
    };
    // Quirk label M: the jar strips every `\r`/`\n` out of a body Jersey pretty-printed, which
    // leaves the runs of indent spaces behind. Nothing else in either transcript has a double
    // space inside it, so that is the observation.
    let parse_error_pretty = if parse_text.contains("  ") {
        "pretty-printed, newlines stripped".to_string()
    } else {
        "compact".to_string()
    };

    // 7. a blank input line — an **agreement**: both skip it.
    server.write("");
    let blank_line_reply = match server.next(SILENCE) {
        None => "nothing".to_string(),
        Some(other) => format!("a line: {}", other.text()),
    };

    // 8. an unknown tool — an **agreement**: a JSON-RPC error, not an `isError` result.
    server.send(&serde_json::json!({
        "jsonrpc": "2.0", "id": 9, "method": "tools/call",
        "params": {"name": "no_such_tool_xyz", "arguments": {}},
    }));
    let (unknown, _) = server.response(SLOW);
    let unknown = unknown.expect("the unknown tool is answered");
    let unknown_tool = if unknown.get("error").is_some() {
        "a JSON-RPC error".to_string()
    } else {
        format!("a result: {}", unknown["result"])
    };

    // The framing, over the whole transcript — an **agreement**: one JSON object per line, no
    // `Content-Length` header anywhere. The jar's blank lines are row 8's and are excluded.
    let every_line_is_one_object = if server
        .seen
        .iter()
        .all(|line| matches!(line, Line::Blank | Line::Json(..)))
    {
        "one JSON object per line".to_string()
    } else {
        "a line that is not one JSON object".to_string()
    };

    let capabilities = result["capabilities"].to_string();
    let protocol_version = result["protocolVersion"].to_string();
    let mut top_level: Vec<&str> = ["serverName", "serverVersion"]
        .into_iter()
        .filter(|key| result.get(*key).is_some())
        .collect();
    top_level.sort_unstable();
    let server_name_keys = if top_level.is_empty() {
        "none".to_string()
    } else {
        top_level.join(", ")
    };
    let server_info_name = result["serverInfo"]["name"].to_string();
    let transcript: Vec<String> = server.seen.iter().map(Line::text).collect();
    let exit_code = server.finish();

    Observed {
        protocol_version,
        server_name_keys,
        server_info_name,
        capabilities,
        ping,
        tool_count,
        tool_shape,
        call_has_structured,
        call_has_is_error,
        call_progress_notifications,
        notification_reply,
        parse_error_id,
        parse_error_pretty,
        blank_line_reply,
        unknown_tool,
        every_line_is_one_object,
        exit_code: exit_code.max(0),
        transcript,
    }
}

/// Row 7's own run: the jar with authentication left at its default, calling one stateful tool.
/// `ApiAuthenticationSettings.isEnabled` is `true` (`:11`) and the stdio bridge never supplies an
/// `Authorization` header, so the envelope comes back `401`.
fn jar_authentication(scratch: &std::path::Path) -> String {
    let mut server = Server::start(jar_command(scratch, false));
    server.send(&initialize());
    let _ = server.response(SLOW);
    server.send(&serde_json::json!({
        "jsonrpc": "2.0", "id": 2, "method": "tools/call",
        "params": {"name": "create_session", "arguments": {}},
    }));
    let (answer, _) = server.response(SLOW);
    let answer = answer.expect("create_session is answered");
    let _ = server.finish();
    let text = answer["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    if text.contains("\"status\": 401") || text.contains("\"status\":401") {
        "401 on a stateful tool".to_string()
    } else if answer["result"]["isError"].as_bool() == Some(true) {
        format!("an isError result: {text}")
    } else {
        "no authentication".to_string()
    }
}

fn main() {
    let verbose = std::env::args().any(|arg| arg == "verbose");
    let scratch = std::env::temp_dir().join("p8t6-userdata");
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&scratch).expect("a scratch user-data directory");

    // The jar's four hand-written "custom" tools are the only flat-argument, no-side-effect ones;
    // `encode_base64` needs no session, no job and no authentication.
    let jar = observe(
        jar_command(&scratch, true),
        "encode_base64",
        serde_json::json!({"text": "p8t6"}),
        // The jar's slowest tool a stdio client can reach without a session, a job or an API key:
        // `get_system_status` is a real HTTP round trip into the REST API, and the registry is
        // re-scanned from OpenAPI for it (`McpControllerV1.java:315`), which is why it is the
        // slowest of the four.
        ("get_system_status", serde_json::json!({})),
    );
    let port = observe(
        port_command(),
        "list_settings",
        serde_json::json!({}),
        // The port's only long-running tool, on the smallest board that actually routes.
        (
            "route_board",
            serde_json::json!({
                "dsn_path": java_dir().join("fixtures/Issue143-rpi_splitter.dsn").display().to_string()
            }),
        ),
    );
    let jar_auth = jar_authentication(&scratch);

    let rows = vec![
        Row {
            number: "1",
            what: "initialize: protocolVersion",
            kind: Kind::Delta,
            jar: jar.protocol_version.clone(),
            port: port.protocol_version.clone(),
        },
        Row {
            number: "1",
            what: "initialize: serverInfo.name",
            kind: Kind::Delta,
            jar: jar.server_info_name.clone(),
            port: port.server_info_name.clone(),
        },
        Row {
            number: "1",
            what: "initialize: non-spec top-level keys",
            kind: Kind::Delta,
            jar: jar.server_name_keys.clone(),
            port: port.server_name_keys.clone(),
        },
        Row {
            number: "2",
            what: "initialize: capabilities",
            kind: Kind::Delta,
            jar: jar.capabilities.clone(),
            port: port.capabilities.clone(),
        },
        Row {
            number: "3",
            what: "tools/call: structuredContent",
            kind: Kind::Delta,
            jar: jar.call_has_structured.clone(),
            port: port.call_has_structured.clone(),
        },
        Row {
            number: "4",
            what: "ping",
            kind: Kind::Delta,
            jar: jar.ping.clone(),
            port: port.ping.clone(),
        },
        Row {
            number: "5",
            what: "the -32700 line's whitespace",
            kind: Kind::Delta,
            jar: jar.parse_error_pretty.clone(),
            port: port.parse_error_pretty.clone(),
        },
        Row {
            number: "6",
            what: "notifications/progress for a _meta.progressToken",
            kind: Kind::Delta,
            jar: format!("{} notifications", jar.call_progress_notifications),
            port: format!("{} notifications", port.call_progress_notifications),
        },
        Row {
            number: "7",
            what: "authentication, at its default",
            kind: Kind::Delta,
            jar: jar_auth.clone(),
            port: "no authentication".to_string(),
        },
        Row {
            number: "8",
            what: "a notification's reply",
            kind: Kind::Delta,
            jar: jar.notification_reply.clone(),
            port: port.notification_reply.clone(),
        },
        Row {
            number: "9",
            what: "the -32700 line's id member",
            kind: Kind::Delta,
            jar: jar.parse_error_id.clone(),
            port: port.parse_error_id.clone(),
        },
        Row {
            number: "10",
            what: "the tool set",
            kind: Kind::Delta,
            jar: format!("{} tools", jar.tool_count),
            port: format!("{} tools", port.tool_count),
        },
        Row {
            number: "10",
            what: "the tool input shape",
            kind: Kind::Delta,
            jar: jar.tool_shape.clone(),
            port: port.tool_shape.clone(),
        },
        // ── the agreements: a NEW delta lands here ───────────────────────────────────────────
        Row {
            number: "=",
            what: "framing",
            kind: Kind::Agree,
            jar: jar.every_line_is_one_object.clone(),
            port: port.every_line_is_one_object.clone(),
        },
        Row {
            number: "=",
            what: "a blank input line",
            kind: Kind::Agree,
            jar: jar.blank_line_reply.clone(),
            port: port.blank_line_reply.clone(),
        },
        Row {
            number: "=",
            what: "an unknown tool",
            kind: Kind::Agree,
            jar: jar.unknown_tool.clone(),
            port: port.unknown_tool.clone(),
        },
        Row {
            number: "=",
            what: "tools/call carries isError",
            kind: Kind::Agree,
            jar: jar.call_has_is_error.clone(),
            port: port.call_has_is_error.clone(),
        },
        Row {
            number: "=",
            what: "the EOF exit code",
            kind: Kind::Agree,
            jar: jar.exit_code.to_string(),
            port: port.exit_code.to_string(),
        },
    ];

    println!("== p8t6: the MCP delta table, asserted against both programs");
    println!(
        "{:<4} {:<7} {:<44} {:<40} {}",
        "row", "verdict", "what", "the jar", "this port"
    );
    let mut failed = 0;
    for row in &rows {
        let verdict = if row.ok() {
            match row.kind {
                Kind::Delta => "DELTA",
                Kind::Agree => "SAME",
            }
        } else {
            failed += 1;
            match row.kind {
                // A recorded delta the two programs now agree on.
                Kind::Delta => "GONE",
                // Something the table says they agree on and they do not.
                Kind::Agree => "NEW",
            }
        };
        println!(
            "{:<4} {:<7} {:<44} {:<40} {}",
            row.number, verdict, row.what, row.jar, row.port
        );
    }
    println!(
        "rows: {}  deltas: {}  agreements: {}  failures: {}",
        rows.len(),
        rows.iter().filter(|r| r.kind == Kind::Delta).count(),
        rows.iter().filter(|r| r.kind == Kind::Agree).count(),
        failed
    );

    if verbose {
        for (label, transcript) in [("the jar", &jar.transcript), ("this port", &port.transcript)] {
            println!("\n-- {label}'s transcript ({} lines) --", transcript.len());
            for line in transcript {
                println!("{line}");
            }
        }
    }

    if failed > 0 {
        eprintln!(
            "p8t6: {failed} row(s) failed. GONE = a recorded delta the two programs now agree on \
             (the table in crates/freerouting/README.md is stale). NEW = a difference the table \
             does not record (add a row, with a Java file:line, or fix the port)."
        );
        std::process::exit(1);
    }
}
