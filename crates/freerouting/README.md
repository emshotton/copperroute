# `freerouting` — the command-line program

Two command lines reach the same program.

* The **legacy** form is Java's, reproduced bug for bug:
  `freerouting -de board.dsn -do board.ses -mp 100`.
* The **native** form is this port's own: `freerouting route board.dsn -o board.ses`.

`legacy::is_legacy_form` decides which: **the native form starts with a subcommand name; every
other argv — including an empty one — is legacy.** So `freerouting` with no arguments is a *legacy*
command line, and it answers what the jar answers: `Both an input file and an output file must be
specified …` and **exit 1**, not a usage screen.

> **Status.** Plan 8 Task 5 landed the command line: the parse, the mode ladder, the exit ladder
> and the log surface. **Task 6 landed `route`** — the sixteen steps of `Freerouting.initializeCli`
> end to end, with SES byte parity against the HEAD jar on every board in the reference table
> below). **Task 7 landed `drc`** — the thirteen steps of `Freerouting.initializeDrc`, with report
> byte parity on seven of the eight committed `drc-*` stems and the eighth an `XDIFF` the jar
> cannot win either (see its own table below). **Task 11 landed the MCP transport** — the reader
> thread, the guarded writer, progress, cancellation and a tool boundary — and with it the MCP
> delta table below. **Task 12 landed the four tools, `freerouting info` and the last of the exit
> ladder** — spec §13's `route_board`, `check_drc`, `board_info` and `list_settings`, the sparse
> priority-70 settings tier they configure, and the board summary `info` and `board_info` share.
> **No subcommand answers exit 3 any more**, which
> `legacy::tests::no_command_runner_answers_not_implemented` keeps true. **Task 14 closed the
> plan**: the `p8t1`-`p8t7` acceptance table below, the regeneration recipes, the `api/mcp/**`
> roster that takes the audit to zero, and `docs/plan-8-handoff.md` — the project completion
> report, because there is no Plan 9.

---

## The exit ladder (plan ruling AR)

| code | when | Java |
|---|---|---|
| **0** | a completed run; `--help`; `drc` **unless** the input is unreadable, the board will not load or the report cannot be written (quirk **#271** — `initializeDrc` returns `true` whatever else happens: a missing `.rules`, a missing session, a failed quality score and **any number of violations** all leave the code at 0) | `Freerouting.java:1495`, `:1397`, `:373` |
| **1** | any failure — **including every refusal on the legacy path** | `Freerouting.java:1474` |
| **2** | **port only:** a usage error on the *native* subcommand form (clap's own) | — |
| **3** | **port only, reserved:** a subcommand not wired up yet | — |

Ruling AR is why 2 and 3 are native-form-only: *a command line the jar accepts must never come
back with a code the jar cannot produce.* `legacy::rewrite` therefore never fails — it warns — and
every argv it builds parses, so clap can only refuse an argv the user typed in the native form.

The same rule decided `--version`: the jar **refuses** `--version` and `-V`
(`Unknown command line argument: --version`, then `initializeCli`'s refusal, **exit 1** — measured
on the HEAD jar, and pinned by `p8t5`'s `version-long`/`version-short` rows). So the legacy path has
no `--version` arm, and clap's lives behind a subcommand: `freerouting route --version`.

---

## The MCP server, and the eleven ways it differs from the jar's

`freerouting mcp` speaks newline-delimited JSON-RPC 2.0 on stdin/stdout — **one JSON object per
line, no `Content-Length` framing**, which is the one transport decision kept from Java. Stdout
carries the protocol and nothing else; every log line goes to stderr.

The jar has an MCP server too, and it is a different program: `--mcp_server.stdio=true` starts a
**daemon thread that POSTs each stdin line to a Jetty server in the same JVM**
(`Freerouting.java:681-788`) and prints the HTTP response body. Controller ruling AO replaced that
arrangement with a native in-process server, so the two are not expected to agree byte for byte —
but *where* they disagree is a contract, not an accident. The rows below are that contract.
**A new delta is a defect, and a delta that disappears is a defect too**; `p8t6`
(`scripts/differential/run.sh p8t6`, Plan 8 Task 12) drives both programs through `initialize`, a
notification, `ping`, `tools/list`, two `tools/call`s, a malformed line and a blank line, and
asserts the difference is exactly this list. It makes **eighteen** observations — thirteen
covering rows 1-10, which must *differ*, and five agreements (the framing, the blank-input-line
skip, the unknown-tool error, `isError`, the EOF exit code) which must be *equal*. A recorded
delta the two programs now agree on is a `GONE` row; a difference this table does not record is a
`NEW` row. Either fails the driver.

Rows **1-10** are message-shape deltas — differences a transcript of that conversation can show,
and the ten scan ruling R18 fixed as the recorded set, so those are the ten `p8t6` asserts. Row
**11** is shutdown *timing*: no line of any transcript can carry it, and it is in the table anyway
because the table is the contract and a difference a client can observe does not become smaller by
being unassertable.

Java's side of every row was **measured**, not read: the jar was driven headlessly with
`--api_server.enabled=true --api_server.authentication.enabled=false
--mcp_server.authentication.enabled=false --mcp_server.enabled=true --mcp_server.stdio=true
--user_data_path=<scratch>` on JDK 25, and the transcript is
`docs/plan-8-prep/evidence/job3-mcp.jsonl` (30 records) with its reading at
`docs/plan-8-prep/evidence/job3-summary.md`. `--mcp_server.stdio=true` **alone does not start the
bridge**: `McpServerSettings.isEnabled` defaults to `false`, and every OpenAPI-derived tool is an
HTTP call into the REST API, which has its own `enabled` default of `false` (scan ruling R18).

| # | what | the jar | this port | Java |
|---|---|---|---|---|
| 1 | `initialize` result identity | `protocolVersion` `"2024-11-05"`; `serverInfo` `{name: "Freerouting MCP", version: <Constants.FREEROUTING_VERSION>}`; **plus non-spec top-level `serverName`/`serverVersion`**, duplicates of the two `serverInfo` members | `protocolVersion` `"2025-06-18"`; `serverInfo` `{name: "freerouting", version: <crate version>}`; **no top-level duplicates** | `McpControllerV1.java:285`, `:281-282`, `:286-287` |
| 2 | `capabilities` | `{"tools": {}}` | `{"tools": {"listChanged": false}}` — the list is fixed at compile time, and saying so is free | `McpControllerV1.java:277-278` |
| 3 | `tools/call` result body | **one text block** holding a pretty-printed `{status, contentType, body}` envelope, and **no `structuredContent`** — although all 28 tools declare an `outputSchema` | a text block **and** `structuredContent`, so a client reads the value instead of re-parsing prose | `McpControllerV1.java:333-356` |
| 4 | `ping` | `-32601 "Unknown method: ping"` — the method table has four cases and a default | answered, `{}` (MCP §Ping) | `McpControllerV1.java:189-198`, `:197` |
| 5 | response framing | the bridge prints `body.replace("\r","").replace("\n","")` — every newline stripped, **no re-escaping** (quirk **#292**, plan label **M** — the id was allocated by Task 14's
label→id sweep, which found that Task 11 had recorded the divergence in code without claiming a
register row). Survivable only because valid JSON has no raw newline inside a string; it visibly mangles the one pretty-printed response into collapsed, double-spaced JSON | compact JSON, one trailing `\n`, flushed. Nothing to strip | `Freerouting.java:770` |
| 6 | `notifications/progress` and `notifications/cancelled` | **neither exists.** The bridge is one blocking `HttpClient.send` with no timeout per line, so nothing can reach stdout between a request and its response, and there is no way to reach a running job | both. A `tools/call` runs on its own thread with a `CancelToken`; `_meta.progressToken` turns on interim notifications, and an inbound `notifications/cancelled` flips the token **while** the tool runs | `Freerouting.java:749-776`; `McpControllerV1.java:359-382` |
| 7 | authentication | **on by default**, and the stdio bridge never supplies an `Authorization` header — it only forwards one that arrived on the MCP request, which over stdio there is none. So every session/job tool answers HTTP 401 until `--api_server.authentication.enabled=false` is passed | none. There is no listener, no port and no credential; the server is a child process on a pipe | `ApiAuthenticationSettings.java:11` (`isEnabled = true`) |
| 8 | a notification's reply | prints a **blank line**. A request with no `id` gets HTTP 204, and the bridge sees a non-null empty body and `println`s it — so a line-oriented client that expects silence desynchronises | nothing at all is written | `McpControllerV1.java:176-177`, `:224-225`; `Freerouting.java:769-772` |
| 9 | the `-32700` reply | has **no `id` member** (Gson drops the JSON-null) and is the one response Jersey pretty-prints, because the branch returns the `JsonObject` rather than its `toString()` — so it arrives double-spaced after row 5's stripping | `{"jsonrpc":"2.0","id":null,"error":{…}}`, compact, like every other response | `McpControllerV1.java:152` |
| 10 | the tool set | **28**: 24 generated by a Swagger scan of `/v1/*`, wrapped as `{path, query, body}`, plus 4 hand-written "custom" tools with **flat** arguments — two conventions in one list. The registry is rebuilt by a fresh OpenAPI scan on **every** `tools/list` *and* every `tools/call` | **4**, spec §13's, flat arguments, registered once at start-up | `McpControllerV1.java:294-301` (`OpenApiMcpToolRegistry.fromApplication`, re-scanned at `:315` for every `tools/call` too); measured, `job3-summary.md` §4 |
| 11 | shutdown after the peer goes away | the bridge thread calls `System.exit(0)` the moment `readLine` answers `null`, killing the JVM out from under anything still running — including a response half written to stdout. A read failure on stdin is `System.exit(1)`, the same way | every path **drains**: the loop stops taking new work — enforced by a `draining` guard on the inbound-line arm, which matters only on the write-failure path, since the other two have already lost the reader thread — and runs until the last tool thread has reported, so no response is truncated. **EOF does not cancel** (controller ruling BH) — a client that closed stdin has no more *requests*, which is not a client that has stopped reading, so every in-flight `tools/call` finishes and answers, and the code is Java's 0. A read failure (1) and a **write** failure (1, which Java cannot reach because `PrintStream.println` swallows its errors) do cancel, because there the answers have nowhere to go. Not visible in any transcript line; visible as process-exit latency after stdin closes | `Freerouting.java:777-782`, `:771` |

Rows 1-5 and row 11 landed with the transport in Task 11; rows 6-10 are Task 12's and scan ruling
R18's, and are recorded here so that Task 12's driver has one list to check rather than two. Row
11's **port** half was rewritten in Task 12's fix round by controller ruling BH — EOF used to
cancel; it now drains.

### The four tools

Spec §13's, with **flat** arguments — not the `{path, query, body}` wrapper 20 of the jar's 28
tools publish (row 10). Every one of them calls `fr-core` directly; **no tool spawns a process**,
and none of them needs a session, a job id or an API key.

| tool | arguments | answers |
|---|---|---|
| `route_board` | `dsn_path` \| `dsn_text`, `ses_path?`, `rules_path?`, `output_path?`, `settings?` | `ses_path` when `output_path` was given, else `ses_text` **and** `data` (Base64); plus `stats`, `incompletes`, `unrouted_report`, `drc_violation_count`, `timed_out`, and `job_id`/`size`/`crc32`/`format`/`filename`/`path` under `api/dto/BoardFilePayload`'s own names (ruling AO). **Always a Specctra session**, whatever went in — a KiCad design JSON routes to a `.ses` here, where `route -de board.json -do out.json` reproduces quirk **#289** (label T) and writes the board *as loaded* |
| `check_drc` | `dsn_path` \| `dsn_text`, `ses_path?`, `rules_path?` | the KiCad DRC report — the same document `freerouting drc` writes, coordinates in `mm` (quirk #151; there is no unit option, and that is a recorded decision) |
| `board_info` | `dsn_path` \| `dsn_text` | the board summary — the same document `freerouting info` writes |
| `list_settings` | `{}` | `{"schema": …, "defaults": …}` — the `RouterSettings` schema with a description on every field, and `DefaultSettings`' own table resolved at call time |

`route_board` is the only one that reports `notifications/progress` and the only one whose
`CancelToken` can be flipped mid-run; the other three are a load and a walk. **A cancelled route
answers a result, not an error**: a partial board with `timed_out: false`, because only the job
deadline sets that flag.

**Closing the server's stdin is not a cancellation** (row 11, controller ruling BH). A script piped
from a file — `freerouting mcp < script.jsonl` — is answered in full and the process then exits 0;
what bounds the wait is what bounds any run (`max_passes`, `job_timeout`, the router's own budget).
Measured on `fixtures/Issue143-rpi_splitter.dsn`: the piped script and a live pipe both answer 3 656
bytes and 16 wires, which is also what `freerouting route` writes. Only a **cancellation** — or a
peer that stops *reading*, which is a broken stdout rather than a closed stdin — ends a run early.

`route_board`'s `filename` and `path` are `api/dto/BoardFilePayload`'s own members and name **the
input's** directory and the session's derived name; they are not where the file went. The only
member that says where a file was written is `ses_path`, and it appears only when `output_path` was
given.

The `settings` argument is the **priority-70** tier, above every other settings source, and it is
*sparse*: naming one field of `scoring` overrides that field and leaves the other ten alone. It is
read by `RouterSettings::from_json_str`, which is strict JSON where Gson's reader is lenient
(quirk #141) — `NaN` and `Infinity` are refused, and a `transient` field (`max_items`,
`save_intermediate_stages`, `ignore_net_classes`) is dropped, exactly as Gson drops it. Call
`list_settings` for the names the reader actually accepts; several differ from the Java field
names (`job_timeout`, `allowed_via_types`, `result_json`, `improvement_threshold`, `timeout`).

**What is *not* a delta.** The two exit *codes* Java can produce are reproduced: EOF on stdin is
**0** and a read failure on stdin is **1** (`Freerouting.java:778-782`). What happens on the way to
them — the drain, and which paths cancel — is row 11. So is skipping a blank input line
(`:739-741`) and so is one-object-per-line framing. And so is `tools/call`'s error split: a tool
that runs and fails is an `isError: true` result, a protocol failure is a JSON-RPC `error`. The
port adds a third case Java has no equivalent for — a tool that **panics** answers `-32603` and the
server keeps serving (plan ruling 4's boundary — the single `catch_unwind` Plan 8 adds, and the
only one in this crate; `fr-router`'s are plan-6 ruling 7's ports of Java `catch` blocks).

---

## Port-only spellings

Java has none of these. Each is marked "Port only" in its own `--help` text too, so that a reader
who learns this CLI does not type them at the jar.

| spelling | what it is | Java's equivalent |
|---|---|---|
| `-v` / `-vv` / `--verbose` | raise the log level | **none** — the jar's log-level flag is `-ll <level>` (quirk #260) |
| `--log-level <level>` | set it outright | `-ll <level>`, or `--logging.console.level=<level>` |
| `--settings <file>` | name a `freerouting.json` for the priority-10 tier. **Native form only** (rulings R7/BG) | **none** — the jar only reads the file under its OS-standard user-data path, and warns at `--settings` as an unknown argument |
| `--version` / `-V` | print the version (native form only) | **none**; the jar prints its version in the startup banner (`Freerouting.java:1120`) |
| `--kicad-json <file>` | a KiCad board file in its own slot (native form only, ruling 14) | **none** — on the legacy form a `.json` takes Java's own slot |
| `--set <section>.<field>=<value>` | the **native form's** generic settings override, repeatable (ruling BJ). `clap` has no arm for a free-form `--<section>.<field>=<value>`, so this is the only way in here | `--<section>.<field>=<value>`, which is what the **legacy** form takes — and the jar takes it too. Neither form accepts the other's spelling; see 'The dead legacy knobs' below |
| `--schema <kicad\|freerouting>` | which spelling of the KiCad DRC schema `drc` writes; **default `kicad`**, native form only (ruling W, quirk #154) | **none** — each jar hard-codes one spelling, and HEAD's disagrees with the `$schema` it advertises |
| `drc` with no `-o` | write the report to **stdout** (quirk #275, spec §12) | **none** — `Freerouting.java:368-371` is dead code, because a bare `-drc` is not DRC mode (quirk #263) |
| `info <board>` | print the board summary — layers, nets and components by name, the file's own metadata, and `BoardStatistics`' whole document — as JSON on stdout, and exit 0. **The only subcommand with no Java counterpart at all**: `Freerouting.main`'s mode ladder (`:1455-1467`) is GUI, DRC and CLI, and `legacy::rewrite` can never produce this argv | **none** |

`--settings` was accepted and unread through Task 5; **Task 6 wired it** — `SettingsInputs::json_file`
now carries the priority-10 tier into both of `resolve_headless`'s chains, and without the flag the
working directory's `freerouting.json` stands in for Java's OS-standard user-data path.

**Native form only, and there is no default file** — controller ruling **BG**, on the measurement
below. Round 1 of the Task 6 review left the flag working on the legacy form too and recorded that
as accepted; ~~"honouring a port-only flag on both command lines was judged less surprising than
refusing it on one"~~ was ruled out: scan ruling R7 scoped the flag to the native form and ruling
AR makes the legacy path bug-for-bug, so the legacy form now warns exactly as the jar does and
applies nothing.

| jar input | `scoring.via_costs` in its manifest |
|---|---|
| `--settings s.json` | **50** — two `Unknown command line argument` warnings (`GlobalSettings.java:833`, once for the flag and once for its argument — it is not a value-consuming arm), file ignored |
| started **in** a directory holding `freerouting.json` | **50** — ignored |
| `-Duser.home` at a home whose *user-data* `freerouting.json` sets `via_costs 77` | **77** — applied at priority 10 |
| the same, with no such file (control) | 50 |

The jar reads exactly one location, `GlobalSettings.getUserDataPath().resolve("freerouting.json")`
(macOS: `~/Library/Application Support/freerouting/freerouting.json`), which is `static` mutable
state spec §2 does not port. Task 5 had made the **working directory** stand in for it; the middle
two rows show it stood in for nothing the jar does, so ruling BG removed that default as well.
`--settings <file>` on the native form is now the whole surface.

Pinned by `cli_e2e.rs::a_settings_file_reaches_the_run` (five runs across both forms) and by
`p8t1`'s `settings-on-legacy` row, which compares the two warnings against the **live jar**.

---

## The dead legacy knobs, and `--set` (plan ruling AQ)

Seven legacy writes are **parsed and dead**, in Java and therefore here:

| flag | what it writes | who reads it |
|---|---|---|
| `-oit` | `routerSettings.optimizer.optimizationImprovementThreshold` | **nobody** |
| `-us` | `routerSettings.optimizer.boardUpdateStrategy` | **nobody** |
| `-is` | `routerSettings.optimizer.itemSelectionStrategy` | **nobody** |
| `-hr` | `routerSettings.optimizer.hybridRatio` | **nobody** |
| `-inc` | `routerSettings.ignoreNetClasses` | **nobody** |
| `-drc`'s `routerSettings.enabled = false` | the same object | **nobody** |
| `-mt` | `routerSettings.optimizer.maxThreads` | **nobody** (quirk #143: the whole multithreaded path is unreachable) |

All seven write Java's `@Deprecated public final RouterSettings routerSettings` bridge
(`GlobalSettings.java:51-53`), which no routing path consults. `docs/java-quirks.md` #131 and #143
are the rows; `crates/fr-settings/src/sources/cli.rs`'s `LegacyBridge` carries the writes with a
`// Java bug:` marker per field, and nothing in `resolve_headless` reads it.

**Ruling AQ: the port keeps them dead, and keeps parsing them.** Keeping the parse is not
pedantry — it is observable, because the value decides how far the cursor moves and therefore which
*other* arguments get warned about, and `p8t5` compares that. Wiring them up would make the port
**more capable than the jar**, which is a product decision and not a parity fix.

**Every one of them is reachable through a generic override — but the spelling depends on the
form, and neither form takes the other's.** All seven are fields of `RouterSettings`, so both
spellings go through `CliSettings` at priority 60, the parser that actually reaches the router.

| form | spelling | why not the other one |
|---|---|---|
| **native** (`route`, `drc`, `info`, `mcp`) | `--set <section>.<field>=<value>` | `clap` owns this command line and has **no arm** for a free-form `--<section>.<field>=<value>`: it answers `error: unexpected argument '--router.optimizer.max_threads' found` and exits **2** |
| **legacy** (`-de`/`-do`/…) | `--<section>.<field>=<value>` — Java's own | ruling AR makes this path bug-for-bug, and the jar ignores `--set` twice over: no `=` on the flag (`CliSettings.java:45`), and its payload does not start with `-` (`:58`). Honouring it here would route differently from the jar on the same argv |

```sh
# native form — `--set`, repeatable (controller ruling BJ)
freerouting route board.dsn -o board.ses \
    --set router.optimizer.optimization_improvement_threshold=0.005 \
    --set router.optimizer.board_update_strategy=GLOBAL_OPTIMAL \
    --set router.optimizer.item_selection_strategy=SEQUENTIAL \
    --set router.optimizer.hybrid_ratio=1:2 \
    --set router.optimizer.max_threads=4

# legacy form — Java's own `--section.field=value`, accepted unchanged
freerouting -de board.dsn -do board.ses --router.optimizer.max_threads=4
```

Three notes that are not hedges:

* **Beyond the spelling, the two are one code path.** `fr_settings::CliSettings::new_with_set_alias`
  splits the payload at its **first** `=` (`CliSettings.java:46`'s `split("=", 2)`), ignores a name
  that does not start with `router.` (`:54-56`), arms the `-de`/`-do` forcing guard on
  `router.enabled` (`:50-52`), and hands the rest to the same `apply_router_setting` the dotted
  spelling reaches. `src/commands::cli_settings` is the one place that chooses between the two
  constructors, on `legacy::is_legacy_form` — the same predicate `crate::run` dispatches on.
* **It is pinned three ways, because Task 14 got this wrong once.** Its first draft wrote "works on
  both forms" into `--help` and into `docs/plan-8-handoff.md`, and nothing in the tree contradicted
  it. Now: `tests/cli_e2e.rs::the_generic_override_is_set_on_native_and_dotted_on_legacy` is a
  five-row truth table through the binary (native `--set` → 77, native `--set=` → 77, native dotted
  → **exit 2**, legacy `--set` → ignored, legacy dotted → 77), read out of the manifest's
  `settings_snapshot`; `crates/fr-settings/tests/cli_source.rs` has the unit pair; and `p8t5`'s
  `set-on-legacy` row runs both programs on the legacy argv against the live jar.
* **Neither spelling is equivalent to the dead flags.** `--router.optimizer.max_threads=4`
  goes through `ReflectionUtil.setFieldValue` and therefore carries **no clamp**, where `-mt`
  clamps to `[0, 1024]` on the bridge (`GlobalSettings.java:692-697`). `-mt 99999` is 1024 there;
  `--router.optimizer.max_threads=99999` is 99999. Both are measured — `p8t5`'s `mt` and
  `router-optimizer-max-threads` rows. `--set router.optimizer.max_threads=99999` is the same
  99999, because it is the same code path.

---

## Logging

Everything goes to **stderr**, and there is no log file. That is a deliberate divergence from the
jar, which writes `INFO`/`WARN` to **stdout** and an `ERROR` three times over (stdout, the log
file, and stderr) — `docs/java-quirks.md` #261. The port also emits no timestamp, so its output is
byte-reproducible; `normalize_log` strips the jar's before comparing.

The level ladder is Java's: the console default is `INFO`, `-ll <level>` moves it (matched by
prefix, **last** occurrence wins), and an unrecognised name silently means `INFO`
(`Log4j2ConfigurationFactory.parseLevel:130-135`).

---

## Where the rest of it is written down

| what | where |
|---|---|
| every legacy flag's value normalisation, and the `-de` rule | `docs/cli-legacy-flags.md` |
| the parse rules, arm by arm, with the Java line for each | `src/legacy.rs`'s module docs |
| the log level ladder, the stderr divergence, `MESSAGE_MAP`, and the `logger/**` roster | `src/logging.rs` |
| the Java behaviours reproduced on purpose | `docs/java-quirks.md` #131, #143, #259-#264 |
| the differential that pins all of it against the jar | `scripts/differential/{run.sh p8t5, sweep-p8t5.sh}`, 87 argv shapes |


---

## `route`: the acceptance table (Plan 8 Task 6, controller ruling AV)

`freerouting route` is measured against the **HEAD jar as a whole program**, not against a method.
Two harnesses, one comparison:

* `scripts/differential/run.sh p8t1 [all]` runs `java -jar <jar> <argv>` and `freerouting <argv>`
  live, on the argv recorded in each `tests/reference/cli-<stem>/argv.txt`;
* `crates/freerouting/tests/cli_e2e.rs` runs the port against the **committed** outputs of the
  same jar runs (`scripts/gen-cli-reference.sh`), so a machine with no JDK checks the same thing.

Four rungs per stem: byte-identical SES, equal exit code, equal `parity::normalize_log`, and —
`p8t2 e2e`'s — equal `parity::normalize_manifest` for a second run with `--router.result_json=<f>`.

| stem | lane | argv beyond `-de`/`-do` | `p8t1` | `p8t2 e2e` |
|---|---|---|---|---|
| `router-rpi-splitter` | ci | `-mp 8` | MATCH | MATCH |
| `router-j2-reference` | ci | `-mp 99` | MATCH | MATCH |
| `router-ecc83-input` | ci | `-mp 8` | MATCH | MATCH |
| `router-empty-board` | ci | `-mp 1`, both stages off | MATCH | MATCH |
| `router-dac2020-bm01` | slow | `-mp 2` | MATCH | MATCH |
| `router-tutorial-board` | slow | `-mp 8` | MATCH | MATCH |
| `router-fanout-bm11` | slow | `-mp 2`, optimizer off | MATCH | MATCH |
| `router-strict-drc-cnh` | slow | `-mp 2` | MATCH | MATCH |
| `tutorial_board` | slow | *(bare)* | MATCH | MATCH |
| `Issue026-J2_reference` | slow | *(bare)* | MATCH | MATCH |
| `large-outline` | slow | `-mp 2`, optimizer off | MATCH | MATCH |
| `kicad-ecc83-json` | ci | `-mp 3`, both stages on — **a `.json` board**, Task 9 | MATCH | MATCH |
| `kicad-complex-hierarchy-json` | slow | *(bare)* — **a `.json` board** | MATCH | MATCH |

**Thirteen stems, and `tests/reference/cli-fixtures.txt` is the list this table follows** — a row
added there is a row here. The last two are Task 9's KiCad-JSON round trips, which is why the
counts in this file and in `docs/plan-8-handoff.md` moved after Task 9 landed. Plus five
**argv rows**, which is where the exit-code and log rungs earn their keep — every stem
above succeeds and emits no message `logging::MESSAGE_MAP` names, so on the stems the log rung
compares two empty projections.

| row | argv | `p8t1` | what it pins |
|---|---|---|---|
| `missing-input` | `-de <missing>.dsn -do a.ses` | MATCH | `Freerouting.java:105` + `:109`, and quirk #261's duplicated `ERROR` folding back into one |
| `no-files` | `-mp 1` | MATCH | `Freerouting.java:81`'s refusal, exit 1 |
| `do-out-dsn` | `-de <dsn> -do b.dsn -mp 1` | MATCH | quirk #268: a 0-byte file, exit 1 |
| `settings-on-legacy` | `-de <dsn> -do d.ses -mp 1 --settings s.json` | MATCH | ruling BG — two `GlobalSettings.java:562` warnings on both sides, exit 0, and the file ignored on both |
| `invalid-input-java-hangs` | session bytes under a `.dsn` name | **XDIFF** | quirk #244 / plan ruling 7 — **the jar hangs for ever**; the port exits 1. Not run against the jar, for the obvious reason |

### XDIFF rows

**One**, and it is the row above. `invalid-input-java-hangs` is a *totalisation* the plan asked
for (ruling 7), not a defect: `Freerouting.isCliTerminalState` (`:189-194`) omits
`RoutingJobState.INVALID`, which `RoutingJobScheduler.java:83`/`:253` assigns for an input that is
neither DSN nor KiCad JSON, so the jar sits in `:151-158`'s `while (…) Thread.sleep(500)` at 0 %
CPU with no output and no message. The port exits **1**. Pinned by
`cli_e2e.rs::de_a_ses_exits_1_instead_of_hanging`.

**No stem row is an XDIFF**, and there is no tolerance anywhere in the ladder. The one
normalisation the SES comparison applies is quirk **#92**'s closed set of four `(parser …)`
keyword literals, rewritten on the *jar* side — the same rewrite `crates/fr-router/tests/
batch_parity.rs` applies to `batch.ses`, and for the same reason (Plan 3 ruling 1 pins the port's
writer to the 2.3.0 spelling because HEAD writes Specctra its own lexer cannot read back).
Measured on `router-rpi-splitter`: those two lines are the **only** difference between the jar's
3 654 bytes and the port's 3 656.

### The budget is live on both sides

Every `p7t*` driver runs the port with `RouterBudget::disabled()` against a jar whose
`optChangedArea` limit is a javac-inlined constant. **This gate does not**: the port's CLI runs
`fr_core::RouterBudget::default()` — Java's own 1000 / 10000 / 250 / 1000 literals — because that
is what a user gets, and the comparison is between two whole programs. The cost is that a live
wall clock is a machine-speed dependency; the bound on it is
`scripts/gen-cli-reference.sh`'s `batch.ses` cross-check, which requires each CLI reference to be
byte-identical to Plan 7's independently generated one and fails loudly if it is not.

---

## `drc`: the acceptance table (Plan 8 Task 7)

`freerouting drc` is measured the same way `route` is — two whole programs on the same argv —
by `scripts/differential/run.sh p8t3 e2e`, over the eight rows of `tests/reference/drc-fixtures.txt`
plus six argv rows. Rungs per stem:

| rung | assertion |
|---|---|
| (a) | the two reports are byte-identical after `parity::normalize_drc_json` (which drops `date` and sorts each `unconnectedItems` entry's `items` by numeric uuid — plan-5 ruling 3, quirk #144) |
| (b) | `quality_score` is equal — asserted *before* the document, because it is the value Task 7 newly **computes** where Plan 5 injected it |
| (c) | the two exit codes are equal |
| (d) | `parity::normalize_log` of both sides is equal |
| (e) | the port's shipped default is the **KiCad** spelling: `quality_score`, not `qualityScore` (ruling W) |

Rung (a) needs the jar's key spelling, so the port is run a second time on the native form with
`--schema freerouting`; rungs (b)-(e) use the **legacy** argv, so the shim, the message set and the
exit ladder are compared on the command line a user actually types.

| stem | argv beyond `-de` | `p8t3 e2e` | `quality_score` |
|---|---|---|---|
| `drc-dev-board` | — | MATCH (20 374 B) | `902.078369140625` |
| `drc-bbd-mars-64` | — | MATCH (62 393 B) | `828.276123046875` |
| `drc-natural-tone-preamp` | — | **XDIFF** | `334.8545227050781` — **MATCH** |
| `drc-issue593-rules` | `-dr <rules>` | MATCH (42 010 B) | `0.0` |
| `drc-issue593-ses` | `<ses>` in the `-de` slot list | MATCH (44 501 B) | `556.5908203125` |
| `drc-issue753-cpu85` | — | MATCH (189 817 B) | `331.1068115234375` |
| `drc-issue110-relay` | — | MATCH (34 269 B) | `0.0` |
| `drc-tutorial-board` | — | MATCH (290 B) | `0.0` |

**All eight quality scores match the committed references exactly**, which is the acceptance the
plan asked for: Plan 5 *injected* that number from the reference, and Task 7 *computes* it from
`fr_router::score::BoardStatistics::normalized_score`, so the eight references became eight free
assertions the moment the injection was removed. They are available **without a JDK** too:
`cli_e2e.rs::every_committed_reference_score_is_recomputed` walks the same eight rows, runs the
binary, and compares its computed `quality_score` to the value the committed `drc.json` records —
the `route` reference lanes' pattern.

Plus six **argv rows**, which is where rungs (c) and (d) earn their keep — the stems all succeed,
and three of the five things that can go wrong on this path do not move the exit code (quirk #271):

| row | argv | `p8t3 e2e` | what it pins |
|---|---|---|---|
| `missing-rules` | `-dr <missing>.rules` | MATCH | `Freerouting.java:289` warns; **exit 0**, report written |
| `missing-session` | `<missing>.ses` in the `-de` slot list | MATCH | `:324` warns; **exit 0**, report written |
| `rules-and-session` | `<dsn> <ses> -dr <rules>` | MATCH | quirk #273 — the only run that fills both optional slots; the six-line log shows `:281`/`:286` before `:309`/`:312` on both sides |
| `missing-input` | `-de <missing>.dsn` | MATCH | `:266` + `:267`, **exit 1**, no report. The driver's row prints `Freerouting.java:105`, not `:266`: the two messages are byte-identical and `logging::MESSAGE_MAP` folds a line onto the **first** template that matches, so both programs resolve it to `initializeCli`'s site — which is what makes the row a comparison rather than a coincidence |
| `ses-input` | session bytes under a `.dsn` name | MATCH | quirk #274 — the **loader** refuses (`BoardLoader.java:33`), `:272` + `:273`, **exit 1** |
| `unwritable-report` | `-drc <missing-dir>/r.json` | MATCH | `:365` + `:366`, **exit 1** — the whole check ran and the run still fails |

`scripts/differential/run.sh p8t3` (no argument) is the **other** mode: a genuine Java-vs-Rust pair
over quirk #272's separate settings merge (`P8T3.java` against
`commands::drc::{quality_score_settings, quality_score}`), printing the seven scoring weights, the
six board counters and the score in raw IEEE bits for each of the eight stems. 25 lines, MATCH.

### The one XDIFF, and why no port can remove it

`drc-natural-tone-preamp` is quirk **#146**, and it is the case where *the jar does not match
itself*. `generateReport` folds `getAllUnconnectedItems`' `track_dangling` entries into
`violations` (`DesignRulesChecker.java:271-276`), and that phase's dedup (`:160`) drops whichever
dangling trace a net entry's **identity-hash-ordered** `firstItem` happens to be. Across
`-XX:hashCode=0..4` the jar produces 113-115 violations on this board; `-XX:hashCode=2` — the only
mode that reproduces run to run without being derived from an object address — gives 115, which is
the committed reference. The port's ascending-id representatives (plan-5 ruling 3) give **112**.

The three that differ are pinned **by uuid, never by count** — `1909`, `1696`, `1242` — here, in
`p8t3`'s row detail, and in `crates/fr-drc/tests/reference_parity.rs::natural_tone_preamp_is_the_
reference_minus_three_dangling_tracks`. **The grant is checked, not waived**, and it is that
test's shape: `p8t3` deletes exactly those three entries from the **jar's** document, requires all
three to have been present, and then requires everything left to be byte-identical to the port's —
so a **port**-side extra entry, or any other difference anywhere in the document, is still a
`DIFF`. Everything else, including the whole 44-entry `unconnectedItems` block **and the
`quality_score`**, is identical.


---

## The seven drivers: `p8t1`-`p8t7`, and what each one actually is

*(Plan 8 Task 14. This is the plan's acceptance ladder as it was actually built — including the
one driver that does not exist, which is recorded rather than quietly dropped.)*

| driver | what it pins | mode(s) | result on the committed tree |
|---|---|---|---|
| **`p8t1`** | the headline gate: SES **bytes**, exit code, `normalize_log`, two whole programs on one argv | `run.sh p8t1` (CI stems + the five argv rows), `p8t1 all` (adds the slow stems), `p8t1probe` | **`all`: 18 rows — 17 MATCH, 1 XDIFF, 0 DIFF. `ci`: 10 rows — 9 MATCH, 1 XDIFF, 0 DIFF.** The invariant, which is what to read if the counts move again: **every board stem MATCHes, and there is exactly one XDIFF in the whole driver** — `invalid-input-java-hangs`, quirk #244, plan ruling 7, ledgered and never run against the jar. `p8t1probe`: MATCH (162 lines) |
| **`p8t2`** | the result manifest, field for field after `normalize_manifest`; the `settings_snapshot` inside it is also the **resolved settings** through the binary | `run.sh p8t2` (Task 4's shape mode), `p8t2 e2e [all]`, `p8t2probe` | shape mode MATCH (762 lines); `e2e all` **13 rows: 13 MATCH, 0 DIFF** — one row per stem of `cli-fixtures.txt`, so this count follows that file |
| **`p8t3`** | the DRC report bytes after `normalize_drc_json`, the computed `quality_score`, the exit code and the log — plus the DSN → `.rules` → SES **load order** | `run.sh p8t3` (the merge driver, Java vs Rust), `p8t3 e2e` | merge: MATCH (25 lines). `e2e`: **14 rows: 13 MATCH, 1 XDIFF, 0 DIFF** — the XDIFF is quirk #146, where the jar does not match itself |
| **`p8t4`** | **does not exist, and this is the record of why.** The plan asked for "the resolved `RouterSettings` dumped as JSON from both sides — `p4t1`'s 64-case matrix re-run through the binary". Task 6 discharged that rung with the two artefacts that already existed rather than building a third: Plan 4's **`p4t1`** still runs the 64-case matrix against the JVM (MATCH, 5 728 lines), and the *through-the-binary* half is the manifest's `settings_snapshot`, which `p8t2 e2e` compares field for field on **every** stem of `cli-fixtures.txt` (13 today) and which `cli_e2e.rs::a_settings_file_reaches_the_run` reads on five more runs (its cases are labelled A-E). A separate `p8t4` would have re-derived `p4t1`'s matrix and compared the same numbers a second time | — | `p4t1` MATCH (5 728 lines); rung reached |
| **`p8t5`** | the legacy surface: slot classification, `LegacyBridge` fields, warnings and the exit code — **not routing** | `run.sh p8t5`, `sweep-p8t5.sh` | `run.sh`: MATCH (**2 124** lines — 2 096 until ruling BJ's row landed in the matrix). `sweep`: **87 rows: 87 MATCH, 0 XDIFF, 0 DIFF, 0 SKIP** — 86 until ruling BJ added `set-on-legacy`. Both counts follow `matrix/p8t5-argv.tsv`; the invariant is **every row MATCHes, no XDIFF, no SKIP** |
| **`p8t6`** | the **documented-delta** driver (ruling AO): the eleven-row MCP table above, asserted to be exactly itself | `run.sh p8t6` | **MATCH** — eighteen observations, thirteen required to differ (rows 1-10) and five required to agree. A `NEW` or `GONE` row fails it |
| **`p8t7`** | spec §1's acceptance: KiCad DSN → route → SES → re-read by `fr_dsn::ses_reader::read`; `-de board.json -do out.ses`; and quirk #289 (label T)'s measurement | `run.sh p8t7` | **MATCH** |

**One thing every `p8t*` header states, and it is not a tolerance.** Plan 7's drivers run the port
with `RouterBudget::disabled()` against a jar whose four
`TIME_LIMIT_TO_PREVENT_ENDLESS_LOOP = 1000` constants are `static final int` with constant
initialisers — javac inlines them, and **no flag or reflection can switch them off** (quirk #234).
`p8t1` and `p8t3` are different: they run the port's CLI with `fr_core::RouterBudget::default()`,
Java's own literals, because that is what a user gets and because the comparison is between two
whole programs rather than two methods. The bound on the resulting machine-speed dependency is
`scripts/gen-cli-reference.sh`'s `batch.ses` cross-check, which requires every CLI reference to be
byte-identical to Plan 7's independently generated one and fails loudly if it is not.

---

## Running and regenerating everything

**Prerequisites.** A sibling clone at `../freerouting` (or `FREEROUTING_JAVA_DIR`), its HEAD jar at
`../freerouting/build/libs/freerouting-current-executable.jar` (or `FREEROUTING_JAR`), the pinned
release jar at `tools/freerouting-2.3.0.jar`, and JDK 25 at `/opt/homebrew/opt/openjdk@25` (or
`JAVA25_HOME`). Every Java half runs with
`-Djava.awt.headless=true -Duser.language=en -Duser.country=US -XX:+UnlockExperimentalVMOptions -XX:hashCode=2`.
Without the clone, every jar-dependent test **skips cleanly** (`parity::require_java_dir`) — the
committed references keep the same assertions alive on a machine with no JDK.

```sh
# the whole suite, no JDK needed
cargo nextest run --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check

# the slow lanes (release; the four slow p8t1 stems, the batch corpus)
FR_SLOW_PARITY=1 cargo test --release

# the drivers (JDK 25 + the clone's HEAD jar)
scripts/differential/run.sh p8t1 all      # SES bytes, exit code, logs
scripts/differential/run.sh p8t2 e2e all  # the result manifest
scripts/differential/run.sh p8t3          # the DRC settings merge (Java vs Rust)
scripts/differential/run.sh p8t3 e2e      # the DRC document, end to end
scripts/differential/run.sh p8t5          # the legacy surface
scripts/differential/run.sh p8t6          # the MCP delta table
scripts/differential/run.sh p8t7          # spec §1's KiCad round trip
scripts/differential/sweep-p8t5.sh        # the whole 87-shape argv matrix
scripts/differential/sweep-p3t15.sh       # Plan 3's DSN corpus (106 fixtures, 530 pairs, 5 XDIFF)
scripts/differential/sweep-p5t1.sh        # Plan 5's DRC corpus
scripts/differential/sweep-p5t2.sh        # Plan 5's report corpus
scripts/differential/sweep-p7t9.sh        # Plan 7's pipeline corpus
```

**Regenerating the committed references.** Each generator drives a jar and rewrites a directory;
none of them is run by the test suite, and each says which jar it pins against.

| script | what it regenerates | against |
|---|---|---|
| `scripts/gen-cli-reference.sh` | `tests/reference/cli-<stem>/{argv.txt,route.ses,route.exit,route.log,manifest.json,drc.json,meta.txt}` for every row of `tests/reference/cli-fixtures.txt`, **driving the bare HEAD jar** | the clone's HEAD jar |
| `scripts/gen-drc-reference.sh` | `tests/reference/drc-<stem>/*` — the eight `-drc` documents | the clone's HEAD jar |
| `scripts/gen-batch-reference.sh` | Plan 7's batch stems (`batch.ses` and the pass transcripts) | the clone's HEAD jar |
| `scripts/gen-router-reference.sh` | Plan 6's per-connection router references | the clone's HEAD jar |
| `scripts/gen-reference.sh` | Plans 1-3's geometry/DSN references | **`tools/freerouting-2.3.0.jar`**, the pinned release (Plan 3 ruling 1: the port's SES writer follows 2.3.0's spelling, not HEAD's) |

`gen-cli-reference.sh` has two extra modes: `--meta-only` rewrites `meta.txt` from the existing
outputs without touching the jar, and **`--verify-hash-modes`** regenerates every stem under
`-XX:hashCode=0..4` and requires **five byte-identical SES files and five identical DRC reports**
— Plan 6's premise, re-checked through the CLI. It has **no `--verify-driver` mode and does not
need one**: Plan 7's generator needed that because its reference came from a *probe* that
reflected a constant to `0`, and this one runs the bare jar, so there is no driver to verify.

---

## The audit

`scripts/audit-port.sh` proves no public Java method of this crate's packages is unaccounted for.
Three invocations, all with the per-class map, all exit 0 with **zero `MISSING` and zero
`UNMAPPED`**:

```sh
./scripts/audit-port.sh api/mcp crates/freerouting/src '*.java'           scripts/audit-map/freerouting.map
./scripts/audit-port.sh logger  crates/freerouting/src '*.java'           scripts/audit-map/freerouting.map
./scripts/audit-port.sh .       crates/freerouting/src 'Freerouting.java' scripts/audit-map/freerouting.map
```

`api/mcp` prints **10 `ROSTERED`** lines and `logger` prints **5**, which is the correct outcome
and the point of the exercise: ruling AO drops the jar's HTTP/SSE/WebSocket MCP transport and its
OpenAPI-derived registry (2 105 lines), spec §2 drops `FRLogger` and the log files, and a
`ROSTERED` line is how a wholly-dropped class stays *visible* instead of passing in silence. The
one class in `api/mcp/**` that is **not** rostered is `McpControllerV1`, because one of its two
public methods is genuinely ported: `rpc` — the JSON-RPC dispatch — is `renamed:` to
`mcp::server::handle`. Its other, `events`, is the SSE back-channel a stdio peer does not need.
The reasoning blocks are at the foot of `src/mcp/server.rs`, `src/mcp/stdio.rs` and
`src/mcp/tools/schema.rs`.

The `.` invocation is the whole of `Freerouting.java` against this crate's five homes
(`main.rs`, `legacy.rs`, `logging.rs`, `commands/*.rs`, `mcp/stdio.rs`) and prints nothing at all.
