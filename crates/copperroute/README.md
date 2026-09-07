# `copperroute` — the command-line program

```sh
copperroute route board.dsn -o board.ses
copperroute drc board.dsn --ses board.ses -o report.json
copperroute info board.dsn
copperroute mcp
```

Every subcommand builds a request for `ops` — the crate's one implementation of
load, route, check and summarise — and renders the outcome. The MCP tools are
the same operations behind a JSON-RPC front end.

## Subcommands

| command | what it does |
|---|---|
| `route <input> -o <output>` | load a design, route it, write the session. `<output>` must end in `.ses` (a Specctra session) or `.json` (a KiCad session); anything else is a usage error before the board is read |
| `drc <input> [-o <report>]` | load a design, optionally a session and rules, run the design-rule check, write the KiCad DRC report — to stdout when `-o` is omitted |
| `info <input>` | print the board summary — layers, nets and components by name, the file's metadata, and the board statistics — as JSON on stdout |
| `mcp` | serve the four tools below over newline-delimited JSON-RPC on stdin/stdout |

`<input>` is a Specctra DSN or a KiCad board JSON; the format is read from the
bytes, not the extension.

Global flags, accepted before or after the subcommand: `-v`/`-vv`/`--verbose`
and `--log-level <off|error|warn|info|debug|trace>` set the log level;
`--settings <file>` names a settings JSON.

`route` takes:

| flag | meaning |
|---|---|
| `--rules <file>` | a Specctra `.rules` file. Without it, a `.rules` beside the input is used when one exists |
| `--ses <file>` | a session to import before routing, so the run continues from it |
| `--kicad-project <file>` | a KiCad `.kicad_pro` whose design rules are applied before routing |
| `--max-passes <n>` | how many routing passes to run; `0` means unlimited |
| `--timeout <t>` | wall-clock budget for the whole job, as `hh:mm:ss` or seconds |
| `--result-json <file>` | write the result manifest here |
| `--set router.<section>.<field>=<value>` | override one setting; repeatable. `router.max_items=N` stops the optimizer as well as the router, where `--max-passes` stops only the router |
| `--visualize <dir>` | write one SVG frame per sampled maze step into an absent or empty directory; `--visualize-every N`, `--visualize-max-frames N`, `--visualize-width`, `--visualize-height` bound it (`docs/routing-visualizer.md`) |

`drc` takes `--ses`, `--rules`, `--kicad-project`, `-o <report>` and
`--schema <kicad|legacy>` (default `kicad`; the other spelling is the
camelCase variant of the same document).

## Settings

A run's settings are assembled from, lowest first: the defaults; the
`--settings` file; the design's own `(autoroute_settings …)` block; the rules
file; `--max-passes`, `--timeout` and `--set`; and, for the MCP `route_board`
tool, its `settings` argument. The rules file is read twice — once for its
own layer names, once against the loaded board — which is why `--rules` takes
a path rather than a parsed object.

`--set` names a field by its path under `router.`; `copperroute mcp`'s
`list_settings` tool prints every field with a description and the defaults
in force. The `--settings` file has the same shape:
`{"router": {"scoring": {"via_costs": 77}}}`. No settings file is read
without the flag.

## Exit codes

| code | when |
|---|---|
| **0** | a completed run; `--help`; a `drc` whose report is clean |
| **1** | any failure: an unreadable input, a board that will not load, a session that cannot be written; and a `drc` whose report carries at least one violation |
| **2** | a usage error: an unknown flag, a missing argument, an output path with the wrong extension |

`route` exits 0 on `COMPLETED` and on `TIMED_OUT` with the session written,
because a timed-out job still leaves a usable partial board.

## The MCP server

`copperroute mcp` speaks newline-delimited JSON-RPC 2.0 on stdin/stdout —
**one JSON object per line, no `Content-Length` framing**. Stdout carries the
protocol and nothing else; every log line goes to stderr. It is an
in-process server: there is no listener, no port and no credential.

| what | behaviour |
|---|---|
| `initialize` | `protocolVersion` `"2025-06-18"`; `serverInfo` `{name: "copperroute", version: <crate version>}`; `capabilities` `{"tools": {"listChanged": false}}` — the tool list is fixed at compile time |
| `ping` | answered, `{}` |
| `tools/call` result | a text block **and** `structuredContent`, so a client reads the value instead of re-parsing prose. A tool that runs and fails is an `isError: true` result; a protocol failure is a JSON-RPC `error`; a tool that **panics** answers `-32603` and the server keeps serving |
| a notification | nothing is written back |
| a malformed line | `{"jsonrpc":"2.0","id":null,"error":{…}}` (`-32700`); a blank line is skipped |
| `notifications/progress` and `notifications/cancelled` | both. A `tools/call` runs on its own thread with a `CancelToken`; `_meta.progressToken` turns on interim notifications, and an inbound `notifications/cancelled` flips the token **while** the tool runs |
| shutdown | every path **drains**: the loop stops taking new work and runs until the last tool thread has reported, so no response is truncated. **EOF does not cancel** — a client that closed stdin has no more requests, which is not a client that has stopped reading — so every in-flight `tools/call` finishes and answers, and the exit code is 0. A read failure (1) and a write failure (1) do cancel, because there the answers have nowhere to go |

### The four tools

| tool | arguments | answers |
|---|---|---|
| `route_board` | `dsn_path` \| `dsn_text`, `ses_path?`, `rules_path?`, `output_path?`, `settings?` | `ses_path` when `output_path` was given, else `ses_text` **and** `data` (Base64); plus `stats`, `incompletes`, `unrouted_report`, `drc_violation_count`, `timed_out`, and `job_id`/`size`/`crc32`/`format`/`filename`/`path`. Always a Specctra session, whatever went in |
| `check_drc` | `dsn_path` \| `dsn_text`, `ses_path?`, `rules_path?`, `kicad_project_path?` | the KiCad DRC report — the same document `copperroute drc` writes, coordinates in `mm` |
| `board_info` | `dsn_path` \| `dsn_text` | the board summary — the same document `copperroute info` writes |
| `list_settings` | `{}` | `{"schema": …, "defaults": …}` — the settings schema with a description on every field, and the defaults resolved at call time |

`route_board` is the only one that reports `notifications/progress` and the
only one whose `CancelToken` can be flipped mid-run; the other three are a
load and a walk. **A cancelled route answers a result, not an error**: a
partial board with `timed_out: false`, because only the job deadline sets
that flag. `filename` and `path` name **the input's** directory and the
session's derived name; `ses_path` is the only member that says where a file
was written.

The `settings` argument is applied above every other source and is
*sparse*: naming one field of `scoring` overrides that field and leaves the
rest alone. It is strict JSON — `NaN` and `Infinity` are refused — and a
transient field (`max_items`, `save_intermediate_stages`,
`ignore_net_classes`) is dropped. The server-wide `--settings <file>` given
to `copperroute mcp` sits below it.

## Logging

Everything goes to **stderr**, and there is no log file. The output carries
no timestamp, so it is byte-reproducible. The console default is `INFO`;
`-v` raises it to debug, `-vv` to trace, and `--log-level` sets it outright
(an unrecognised name means `INFO`).

## Layout

| module | what it holds |
|---|---|
| `cli.rs` | the clap surface |
| `lib.rs` | `run` and `ExitCode` |
| `ops/` | `SettingsOverrides` and the settings ladder (`settings.rs`); `BoardSource`, `LoadRequest`, `Loaded` and `load` (`load.rs`); `RouteRequest`, `RouteOutcome` and `route` (`route.rs`); `DrcRequest`, `DrcOutcome` and `drc` (`drc.rs`); `InfoRequest` and `info` (`info.rs`); `OpError` |
| `commands/` | the three CLI adapters: build a request, call `ops`, write files, pick the exit code |
| `mcp/` | the JSON-RPC transport (`stdio.rs`, `server.rs`, `jsonrpc.rs`) and the four tool adapters under `tools/` |
| `logging.rs` | the level ladder and the subscriber |

## Tests

`cargo test -p copperroute` runs the unit tests plus six integration
suites:

* **`ops_load.rs`, `ops_route.rs`, `ops_drc_info.rs`** pin the operations
  directly: settings precedence, rules discovery, session import, the output
  format, the job deadline, the DRC violation count and quality score.
* **`cli_e2e.rs`** runs the binary. Its scenario tests cover the refusals,
  `--set` and `--settings`, the timeouts, and the `drc` exit codes. Its
  reference tests run every stem of `tests/reference/cli-fixtures.txt` and
  compare the session bytes, the exit code and the normalised manifest
  against the committed `tests/reference/cli-<stem>/` outputs; the `ci` stems
  run by default, the `slow` ones under
  `COPPERROUTE_SLOW_PARITY=1 cargo test --release`.
* **`mcp_stdio.rs`** drives the server through `initialize`, notifications,
  `ping`, `tools/list`, `tools/call`, cancellation, a panicking tool, a
  malformed line and EOF, over in-process pipes and over the spawned binary.

The reference and corpus tests need the fixture corpus in a sibling
`../freerouting` checkout (`FREEROUTING_JAVA_DIR` overrides the location)
and skip cleanly without it.

```sh
cargo nextest run --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
COPPERROUTE_SLOW_PARITY=1 cargo test --release            # the slow lanes

# re-cut the CLI goldens from the binary, after a change that is meant to move them
COPPERROUTE_REGOLDEN=<label> cargo test -p copperroute --test cli_e2e
```

The other reference families have generators under `scripts/`:
`gen-drc-reference.sh` for the eight `drc-*` reports, `gen-batch-reference.sh`
for the whole-board `batch.*` files `copper-router`'s `batch_parity` reads,
`gen-router-reference.sh` for the per-connection `router.jsonl` files, and
`gen-reference.sh` for the DSN/SES writer references. `tests/reference/README.md`
explains the lanes and when a family is re-cut.
