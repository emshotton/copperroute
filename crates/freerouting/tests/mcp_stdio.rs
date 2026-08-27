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
