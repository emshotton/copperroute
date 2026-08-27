use super::jsonrpc::{PARSE_ERROR, Request, Response};
use super::server::{State, handle};
use serde_json::Value;
use std::io::{BufRead, Write};

/// Newline-delimited JSON-RPC over stdin/stdout. Returns the process exit code.
pub fn run() -> i32 {
    let mut state = State::new();
    run_with(
        &mut state,
        std::io::stdin().lock(),
        std::io::stdout().lock(),
    )
}

pub fn run_with<R: BufRead, W: Write>(state: &mut State, reader: R, mut writer: W) -> i32 {
    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        if line.trim().is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<Request>(&line) {
            Ok(req) => handle(state, req),
            Err(e) => Some(Response::err(
                Value::Null,
                PARSE_ERROR,
                format!("parse error: {e}"),
            )),
        };
        if let Some(resp) = response {
            let text = serde_json::to_string(&resp).expect("response serializes");
            if writeln!(writer, "{text}")
                .and_then(|_| writer.flush())
                .is_err()
            {
                break;
            }
        }
    }
    0
}
