#![forbid(unsafe_code)]

//! The `freerouting` binary: read `argv`, hand it to [`freerouting::run`], exit with the code it
//! answers.
//!
//! Everything else is in the library half of this crate, so that the `p8t5` differential driver
//! and the integration tests exercise the same code the program runs — see `src/lib.rs`.

fn main() {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(freerouting::run(&raw).code());
}

// =================================================================================================
// `Freerouting.java`'s four server methods (plan §The audit; `scripts/audit-map/freerouting.map`
// sends `Freerouting` here and to five other files)
// =================================================================================================
//
// `Freerouting.main` is `freerouting::run` plus the `std::process::exit` above; `initializeCli`
// and `initializeDrc` are `commands/{route,drc}.rs`; the exit ladder, the legacy shim and the log
// map are `lib.rs`, `legacy.rs` and `logging.rs`. What is left of the class is the four methods
// that start and stop network servers, and the reason each is absent is below — the map's header
// calls for the cross-reference here, and the full roster of the Jetty/Jersey/OpenAPI stack it
// stands on is at the foot of `crates/fr-core/src/lib.rs` (Plan 8 Task 0).
//
// not ported: Freerouting.initializeAPI (Freerouting.java:394-521) — builds the Jetty server, the
//   Jersey servlet container, the CORS and rate-limit filters and the OpenAPI document for the
//   REST API. Spec §2 has no REST API, so `apiServerSettings.isEnabled` is permanently false here
//   and the `main:1415-1434` branch that would call this is unreachable.
// not ported: Freerouting.stopApiServer (Freerouting.java:378-390) — the matching shutdown, and
//   the only caller of it is `shutdownApplication`.
// not ported: Freerouting.initializeMCP (Freerouting.java:543-668) — the **HTTP** MCP transport:
//   the same Jetty stack again, plus the WebSocket endpoint and the OpenAPI-derived tool
//   registry. Controller ruling AO replaces it with spec §13's four tools over a native stdio
//   JSON-RPC server (`crate::mcp`), which needs no listener, no port and no authentication.
// renamed: Freerouting.startMcpStdioBridge -> `freerouting::mcp::stdio::run_with`
//   (Freerouting.java:681-788) — **landed in Plan 8 Task 11**. Java's is a pump: a daemon thread
//   forwarding each stdin line to the HTTP MCP server over `HttpClient.send` and printing the
//   reply with every `\r` and `\n` stripped from the body (`:770`, quirk label M — not
//   reproduced). Controller ruling AO deleted the server it pumps to, so what the method became is
//   a reader thread, a `Mutex`-guarded writer and an in-process dispatcher; the responsibility-by-
//   responsibility table is on `mcp::stdio::run`, and the five behavioural differences are the
//   Task 11 half of the delta table in `crates/freerouting/README.md`.
