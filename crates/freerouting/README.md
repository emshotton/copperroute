# `freerouting` — the command-line program

Two command lines reach the same program.

* The **native** form: `freerouting route board.dsn -o board.ses`.
* The **legacy** form, compatible with the classic freerouting command line:
  `freerouting -de board.dsn -do board.ses -mp 100`.

`legacy::is_legacy_form` decides which: **the native form starts with a
subcommand name; every other argv — including an empty one — is legacy.** So
`freerouting` with no arguments is a *legacy* command line, and it answers
`Both an input file and an output file must be specified …` and **exit 1**,
not a usage screen.

## Subcommands

| command | what it does |
|---|---|
| `route <input> -o <output>` | load a design, route it, write the session |
| `drc <input> [-o <report>]` | load a design (plus an optional session and rules file), run the design-rule check, write the KiCad DRC report — to stdout when `-o` is omitted |
| `info <input>` | print the board summary — layers, nets and components by name, the file's metadata, and the board statistics — as JSON on stdout |
| `mcp` | serve the four tools below over newline-delimited JSON-RPC on stdin/stdout |

Global flags: `-v`/`-vv`/`--verbose` and `--log-level <level>` set the log
level, `--settings <file>` names a settings JSON for the priority-10 tier.

`route` takes `--rules <file>`, `--ses <file>` (a session to apply before
routing), `--kicad-json <file>` and `--kicad-project <file>` (a KiCad board
and its project, for the KiCad-JSON path), `--max-passes`, `--timeout`
(seconds), `--result-json <file>` (the result manifest), the optimizer knobs
(`--optimizer-improvement-threshold`, `--update-strategy`, `--hybrid-ratio`,
`--item-selection`), `--ignore-net-classes`, `--threads` (parsed, and read by
nothing — the router is single-threaded), and the generic override
`--set <section>.<field>=<value>`, repeatable. `drc` takes `--ses`,
`--rules`, `--kicad-json`, `--kicad-project`, `-o` and
`--schema <kicad|freerouting>`.

## The exit ladder

| code | when |
|---|---|
| **0** | a completed run; `--help`; `drc` **unless** the input is unreadable, the board will not load or the report cannot be written — a missing `.rules`, a missing session, a failed quality score and **any number of violations** all leave the code at 0 |
| **1** | any failure — including every refusal on the legacy path |
| **2** | a usage error on the *native* subcommand form (clap's own) |

Exit 2 is native-form-only on purpose: the legacy path is bug-for-bug with
the classic command line, so `legacy::rewrite` never fails — it warns — and
every argv it builds parses. The same rule decides `--version`: the legacy
path refuses it (`Unknown command line argument: --version`, exit 1), and
clap's lives behind a subcommand: `freerouting route --version`.

## The MCP server

`freerouting mcp` speaks newline-delimited JSON-RPC 2.0 on stdin/stdout —
**one JSON object per line, no `Content-Length` framing**. Stdout carries the
protocol and nothing else; every log line goes to stderr. It is an
in-process server: there is no listener, no port and no credential; the
server is a child process on a pipe.

| what | behaviour |
|---|---|
| `initialize` | `protocolVersion` `"2025-06-18"`; `serverInfo` `{name: "freerouting", version: <crate version>}`; `capabilities` `{"tools": {"listChanged": false}}` — the tool list is fixed at compile time |
| `ping` | answered, `{}` |
| `tools/call` result | a text block **and** `structuredContent`, so a client reads the value instead of re-parsing prose. A tool that runs and fails is an `isError: true` result; a protocol failure is a JSON-RPC `error`; a tool that **panics** answers `-32603` and the server keeps serving (the single `catch_unwind` in this crate) |
| a notification | nothing is written back |
| a malformed line | `{"jsonrpc":"2.0","id":null,"error":{…}}` (`-32700`), compact like every other response; a blank line is skipped |
| `notifications/progress` and `notifications/cancelled` | both. A `tools/call` runs on its own thread with a `CancelToken`; `_meta.progressToken` turns on interim notifications, and an inbound `notifications/cancelled` flips the token **while** the tool runs |
| shutdown | every path **drains**: the loop stops taking new work and runs until the last tool thread has reported, so no response is truncated. **EOF does not cancel** — a client that closed stdin has no more *requests*, which is not a client that has stopped reading, so every in-flight `tools/call` finishes and answers, and the exit code is 0. A read failure (1) and a write failure (1) do cancel, because there the answers have nowhere to go |

### The four tools

Every one of them calls `fr-core` directly; **no tool spawns a process**, and
none needs a session, a job id or an API key.

| tool | arguments | answers |
|---|---|---|
| `route_board` | `dsn_path` \| `dsn_text`, `ses_path?`, `rules_path?`, `output_path?`, `settings?` | `ses_path` when `output_path` was given, else `ses_text` **and** `data` (Base64); plus `stats`, `incompletes`, `unrouted_report`, `drc_violation_count`, `timed_out`, and `job_id`/`size`/`crc32`/`format`/`filename`/`path`. **Always a Specctra session**, whatever went in |
| `check_drc` | `dsn_path` \| `dsn_text`, `ses_path?`, `rules_path?` | the KiCad DRC report — the same document `freerouting drc` writes, coordinates in `mm` |
| `board_info` | `dsn_path` \| `dsn_text` | the board summary — the same document `freerouting info` writes |
| `list_settings` | `{}` | `{"schema": …, "defaults": …}` — the `RouterSettings` schema with a description on every field, and the defaults resolved at call time |

`route_board` is the only one that reports `notifications/progress` and the
only one whose `CancelToken` can be flipped mid-run; the other three are a
load and a walk. **A cancelled route answers a result, not an error**: a
partial board with `timed_out: false`, because only the job deadline sets
that flag. `filename` and `path` name **the input's** directory and the
session's derived name, not where the file went; `ses_path` is the only
member that says where a file was written.

The `settings` argument is the **priority-70** tier, above every other
settings source, and it is *sparse*: naming one field of `scoring`
overrides that field and leaves the other ten alone. It is read by
`RouterSettings::from_json_str`, which is strict JSON — `NaN` and `Infinity`
are refused, and a transient field (`max_items`, `save_intermediate_stages`,
`ignore_net_classes`) is dropped. Call `list_settings` for the names the
reader accepts.

## Native-form-only spellings

Each of these is marked "native form only" in its own `--help` text, because
the legacy form does not accept it.

| spelling | what it is |
|---|---|
| `-v` / `-vv` / `--verbose`, `--log-level <level>` | the log level; the legacy form's is `-ll <level>` or `--logging.console.level=<level>` |
| `--settings <file>` | a settings JSON for the priority-10 tier. **There is no default file**: without the flag, nothing is read from the working directory or the user-data directory |
| `--version` / `-V` | print the version |
| `--kicad-json <file>`, `--kicad-project <file>` | a KiCad board file and its project, in their own slots; on the legacy form a `.json` takes the input slot |
| `--set <section>.<field>=<value>` | the generic settings override, repeatable. `clap` has no arm for a free-form `--<section>.<field>=<value>`, so this is the only way in on the native form |
| `--schema <kicad\|freerouting>` | which spelling of the KiCad DRC schema `drc` writes; default `kicad` |
| `drc` with no `-o` | write the report to **stdout** |
| `info <board>` | the board summary; the only subcommand with no legacy counterpart at all |

## The legacy form, and `--set`

The legacy form accepts `-de <input> [<session>] [<rules>]`, `-do <output>`,
`-dr <rules>`, `-drc [<report>]`, `-mp <passes>`, `-mt <threads>`, `-ll
<level>`, `-oit`, `-us`, `-is`, `-hr`, `-inc`, and the dotted override
`--<section>.<field>=<value>`; `src/legacy.rs`'s module docs give the parse
rules arm by arm, and `docs/cli-legacy-flags.md` every flag's value
normalisation and the `-de` slot rule. Seven of those flags are **parsed and
dead** — `-oit`, `-us`, `-is`, `-hr`, `-inc`, `-mt`, and `-drc`'s router
switch-off — writing a `LegacyBridge` that no routing path reads. Keeping the
parse is observable, because the value decides how far the cursor moves and
therefore which *other* arguments get warned about.

Every one of them is reachable through the generic override, but **the
spelling depends on the form, and neither form takes the other's**:

| form | spelling | why not the other one |
|---|---|---|
| **native** (`route`, `drc`, `info`, `mcp`) | `--set <section>.<field>=<value>` | `clap` owns this command line and answers `error: unexpected argument '--router.optimizer.max_threads' found`, exit **2** |
| **legacy** (`-de`/`-do`/…) | `--<section>.<field>=<value>` | the legacy path ignores `--set` twice over: no `=` on the flag, and its payload does not start with `-` |

```sh
# native form — `--set`, repeatable
freerouting route board.dsn -o board.ses \
    --set router.optimizer.optimization_improvement_threshold=0.005 \
    --set router.optimizer.board_update_strategy=GLOBAL_OPTIMAL \
    --set router.optimizer.item_selection_strategy=SEQUENTIAL \
    --set router.optimizer.hybrid_ratio=1:2 \
    --set router.max_items=4

# legacy form — `--section.field=value`, accepted unchanged
freerouting -de board.dsn -do board.ses --router.max_items=4
```

Beyond the spelling, the two are one code path:
`fr_settings::CliSettings::new_with_set_alias` splits the payload at its
**first** `=`, ignores a name that does not start with `router.`, and hands
the rest to the same `apply_router_setting` the dotted spelling reaches.
`tests/cli_e2e.rs::the_generic_override_is_set_on_native_and_dotted_on_legacy`
is the five-row truth table through the binary. Note that `--set
router.max_items=N` stops the **optimizer** as well as the router, where
`--max-passes` stops only the router; `--set`'s help text says so.

## Logging

Everything goes to **stderr**, and there is no log file. The output carries
no timestamp, so it is byte-reproducible. The console default is `INFO`;
`-ll <level>` moves it (matched by prefix, **last** occurrence wins), and an
unrecognised name silently means `INFO`. `src/logging.rs` carries the level
ladder and `MESSAGE_MAP`, the table of message templates the end-to-end tests
compare log output through.

## Tests

`cargo test -p freerouting` runs the unit tests plus three integration
suites:

* **`cli_e2e.rs`** runs the binary on every stem of
  `tests/reference/cli-fixtures.txt` and compares the session bytes, the exit
  code, the normalised log and — for a second run with `--result-json` — the
  normalised manifest against the committed `tests/reference/cli-<stem>/`
  outputs; on every stem of `tests/reference/drc-fixtures.txt` it compares
  the `drc` report after `parity::normalize_drc_json` and the recomputed
  `quality_score` against the committed `drc.json`. It also carries the argv
  rows: the refusals, the settings-file and `--set` truth tables, the
  stage-timeout-is-not-a-job-timeout case, and `de_a_ses_exits_1_instead_of_hanging`.
* **`legacy_cli.rs`** pins the legacy parse: slot classification,
  `LegacyBridge` fields, warnings and the exit code — **not routing**.
* **`mcp_stdio.rs`** drives the server through `initialize`, a notification,
  `ping`, `tools/list`, `tools/call`, a malformed line and a blank line, and
  pins the table above.

The stems marked `ci` in the fixture files run by default; the `slow` ones
are `#[cfg_attr(debug_assertions, ignore)]` and run under
`FR_SLOW_PARITY=1 cargo test --release`. Both lanes need the fixture corpus
in a sibling `../freerouting` checkout (`FREEROUTING_JAVA_DIR` overrides the
location) and skip cleanly without it.

```sh
# the whole workspace, no external tools needed
cargo nextest run --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check

# the slow lanes (release; the slow stems, the batch corpus)
FR_SLOW_PARITY=1 cargo test --release
```

**Regenerating the committed references.** Each generator rewrites a
directory under `tests/reference/` and writes a `meta.txt` per stem saying
how the reference was cut; none of them is run by the test suite.
`tests/reference/README.md` explains the lanes and when a family is re-cut.

| script | what it regenerates |
|---|---|
| `scripts/gen-cli-reference.sh` | `tests/reference/cli-<stem>/{argv.txt,route.ses,route.exit,route.log,manifest.json,drc.json,meta.txt}` for every row of `cli-fixtures.txt`; `--meta-only` rewrites `meta.txt` without re-running anything |
| `scripts/gen-drc-reference.sh` | `tests/reference/drc-<stem>/*` — the eight `drc` documents |
| `scripts/gen-batch-reference.sh` | the whole-board `batch.ses` and per-pass records `crates/fr-router`'s `batch_parity` reads |
| `scripts/gen-router-reference.sh` | the per-connection `router.jsonl` files `crates/fr-router`'s `reference_parity` reads |
| `scripts/gen-reference.sh` | the DSN/SES writer references `crates/fr-dsn`'s parity suites read |

`gen-cli-reference.sh` cross-checks each `route.ses` against `fr-router`'s
independently generated `batch.ses` for the same stem and fails loudly if
the two differ, which bounds the machine-speed dependency a run with a live
`RouterBudget` carries.
