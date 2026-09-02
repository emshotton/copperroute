# Plan 8 — `fr-core`, the headless CLI and the native stdio MCP server Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

> **This is the FINAL plan of the port.** Task 13's hand-off is not "obligations for Plan 9" — there is no Plan 9. It is the **project completion report**: what is ported, what is rostered dead and why, every deliberate divergence from the jar, and how any future reader re-runs every parity check in the repository. Nothing may be deferred out of Plan 8 to a later plan; a thing that cannot land is a **recorded, closed decision** with its evidence, not a `// added in Plan 9:` marker.

**Goal:** Build the last layer — `fr-core` (`RoutingPipeline`, `CancelToken`, `ProgressSink`, `RoutingResult`, the job model, the result manifest, the byte-scraping `BoardStatistics` twin), the `freerouting` binary's real behaviour (`route`, `drc`, `info`, the bug-for-bug legacy `-de/-do/-dr/-drc` shim, the exit ladder), the `io/kicad` reader/writer, and the native stdio JSON-RPC MCP server of spec §13 — so that **`freerouting-rs -de <dsn> -do <ses>` produces the same SES bytes, the same exit code and the same normalised log stream as the Java HEAD jar given the same argv**, and the KiCad round trip of spec §1 closes. ≈ **3 300 Java method-lines across 19 classes** are ported (the survey's "≈ 4 300" is its whole-file reading of the same set; the per-task budgets below count method ranges and sum to ≈ 3 300), plus ≈ **1 250 lines of NEW code** with no Java counterpart (the MCP transport and its four tools, `fr-core`'s `Ctx`/`RoutingResult`/`CancelToken`, `freerouting info`, the `p8t*` harness), plus ≈ **13 500 lines rostered `// not ported:`** with caller evidence (`api/**` 8 425, `analytics/**` 2 100, the Jetty/WebSocket MCP transports 2 105, `SessionToEagle` 627, the scheduler queue ~450, `management/sessions` 252, `BoardScoreBreakdown` + `ScoringWeightComparison` 424, `BoardComparator` 758).

**Architecture:** Spec §4 (`fr-core` = "RoutingPipeline, CancelToken, ProgressSink, RoutingResult, BoardStatistics, result manifest"; `freerouting` = "clap CLI + legacy shim + stdio MCP"), §10 (the whole section), §12 (the CLI), §13 (the MCP), §14.4 (end-to-end), §15 steps 9 and 10. Plan 8 adds **exactly one crate**, `fr-core`, and touches `crates/freerouting` for the first time since Plan 1. `fr_router::{score, pipeline}` are **re-exported, never moved** (Plan 7 controller answer 3). `fr-dsn` gains the `kicad` module (Plan 3 hand-off §Plan 8: the KiCad reader traffics in `fr-dsn`'s own `BoardReadResult`/`BoardMetadata`/`CoordinateTransform`, not parallel types).

**Tech Stack:** Rust 2024; `fr-geometry` (Plan 1); `fr-board` (Plan 2); `fr-dsn` (Plan 3); `fr-settings` (Plan 4); `fr-drc` (Plan 5); `fr-router` (Plans 6+7); `clap`, `serde`, `serde_json`, `thiserror`, `tracing`, `tracing-subscriber` (all already workspace dependencies). **No `schemars`** (ruling AO/General — the four tool schemas are hand-written JSON literals). **No `rayon`, no `rand`, no `slotmap`, no new workspace dependency of any kind.** `std::thread` is used in **exactly two places**, both in `crates/freerouting/src/mcp/` (ruling 3 below); every crate below `freerouting` stays single-threaded.

**Spec:** `docs/superpowers/specs/2026-08-27-freerouting-rust-port-design.md` (§1, §2, §4, §10, §12, §13, §14.4, §15). Also binding: `docs/plan-3-handoff.md` §Plan 8, `docs/plan-4-handoff.md` §Plan 8, `docs/plan-5-handoff.md` §Plan 8, `docs/plan-6-handoff.md` §11, `docs/plan-7-handoff.md` §Obligations → Plan 8 **[verify at pre-flight — Plan 7 Task 17 writes it; while it is in flight the same obligations live in that plan's task reports]**, `docs/java-quirks.md` (every row and the obligation register), `docs/cli-legacy-flags.md`.

**Later plans:** none. See the callout above.

> ### ⚠ Plan 7 is still landing — every dependency on it is tagged `[verify at pre-flight]`
>
> At the time this plan was written the branch `plan-7-router-batch` was at **`b899468`** (Plan 7
> Task 1, `fr_router::score`). Tasks 2–17 of Plan 7 are **assumed present** exactly as
> `docs/superpowers/plans/2026-08-30-plan-7-router-batch.md` §Interfaces and each task's
> *Interfaces produced* block describe them. **Every line in this plan that consumes a Plan 7
> interface carries `[verify at pre-flight]`.** A pre-flight conflict scan re-checks each one
> against the committed tree before dispatch and amends this document in place — exactly as
> happened to Plan 7's own draft (its amendment block lists 17 rulings from that scan). An
> implementer who finds a tagged signature different from what is written here **stops and
> reports**; it is a plan amendment, never a silent adaptation.
>
> The tagged surface, in one list, so the scan has a checklist:
> `fr_router::pipeline::{run_pipeline, PipelineResult, PassRecord, RouterStop, StopRequestState,
> RouterBudget, RouterCounters, TaskState, NamedAlgorithmType, ProgressSink, RoutingEvent,
> NoopProgressSink, build_unrouted_report}`; `fr_router::score::{BoardStatistics + its ten DTOs,
> calculate_score, maximum_score, normalized_score, is_pin_escaped}`;
> `fr_router::RouterError::NoRoutableLayer`; `Board::structural_hash` (widened, Plan 7 Task 3);
> `ViaRule` owning its `ViaInfo`s (Plan 7 Task 0, landed as `bde59ef`);
> `tests/reference/<stem>/batch.{ses,passes.jsonl,meta.txt}` and the eight batch stems of Plan 7
> Task 16; `scripts/gen-batch-reference.sh`; `crates/fr-router/tests/batch_parity.rs`.

## Global Constraints

All Plan 1–7 constraints and rulings remain in force (see each hand-off's §Rulings, plan-6's amendment block for rulings X–AE, and plan-7's for rulings AF–AM and its 17 scan rulings). Additionally:

- **Java wins over plan text, and over the survey.** Every file:line in this document was read out of the clone's HEAD while writing the plan; the implementer re-derives it from Java and reports disagreement rather than trusting the plan. Places where this plan **already corrects the survey or a controller ruling** are called out in rulings 1, 5 and 8 — that is the precedent, not the exception.
- **Java source authority is the clone's HEAD, and so is the parity jar** (`../freerouting/build/libs/freerouting-current-executable.jar`, JDK 25 at `/opt/homebrew/opt/openjdk@25/bin`, always `-Djava.awt.headless=true`). Ruling AV governs which jar each surface uses: the **HEAD jar** for every `p8t*` end-to-end driver; the pinned 2.3.0 jar (Plan 3 ruling 10) is untouched and keeps owning the DSN/SES *syntax* assertions that `crates/fr-dsn/tests/parity_ses.rs` already makes.
- Behavioral port: reproduce Java bugs, with a `// Java bug:` marker at the site and a row in `docs/java-quirks.md`. `// totalized:` for crash→value changes, `// not ported:`, `// not reachable:`, `// renamed:`, `obligation:` per `docs/java-quirks.md` §Process notes. **There is no `// added in Plan 9:`** — the marker vocabulary loses its deferral arm in this plan (Task 13 enforces it). **The marker's Java method name must sit on the same line as the marker** (Plan 2 ruling 13 — `audit-port.sh` is line-based).
- No GUI, no `FRLogger`, no `TextManager` beyond the four methods ruling names, no observers. Spec §10's `ProgressSink` replaces `core/events/**` and `NamedAlgorithm`'s listener lists; every `FRLogger.trace`/`info` payload and its guard is dropped — **except** the message *set* the CLI must emit, which Task 6 reproduces on **stderr** through `tracing` (a deliberate divergence from Java's stdout Console appender, quirk label AI, accounted for by `p8t1`'s log normaliser).
- `Result`/`Option` where Java throws or returns `null`; `catch_unwind`/`Result` only at documented boundaries. **Plan 6's five boundaries and Plan 7's one are unchanged**; Plan 8 adds **exactly one** more (ruling 4 below — the MCP tool boundary, which turns a panic into a JSON-RPC error rather than killing the server).
- **No static mutable state.** The one recorded exception is `fr_geometry::Line`'s identity counter (controller ruling AE, `docs/java-quirks.md` §Process notes); Plan 8 honours its contract — the token has one reader, `Line::is_same_object`, with one caller, `Board::change_trace` — and adds no second exception.
- **Threads: exactly two, both in `crates/freerouting/src/mcp/`** (ruling 3). `fr-router` remains single-threaded and `max_threads` remains dead everywhere (quirks #143 / Plan 7's `-mt`-is-dead-everywhere row); the MCP's threads carry no routing policy, only the transport. Every crate below `freerouting` must still compile and behave identically with the MCP feature absent.
- **`#![forbid(unsafe_code)]` at the top of every crate root this plan touches** — including the **new** `crates/fr-core/src/lib.rs`. A new module never weakens it.
- **No new workspace dependencies.** `schemars` is specifically **refused** (ruling AO/General): the four MCP tool schemas are hand-written `serde_json::json!` literals with a snapshot test. Any other need is a recorded ruling, not a silent `Cargo.toml` edit. The one dependency-graph change is the **addition** of `fr-core` between `freerouting` and `fr-router`, which spec §4's dependency line already draws.
- Every task ends with `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and the `scripts/audit-port.sh` invocations from Task 13 (with `scripts/audit-map/{fr-core,freerouting}.map` once Task 0 creates them) run and their `MISSING` count recorded in the commit message — verified against the committed tree, not against a report.
- **From Task 3 on, every task's verification additionally runs `cargo test -p fr-router --test batch_parity` and `scripts/differential/run.sh p6t1` on the five Plan 6 stems** `[verify at pre-flight]`. Plan 8 touches the board *load*; a load change is invisible in Plan 8's own tests and lethal to Plan 7's.
- Commit trailer:
  ```
  Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_015f1ubhBBpiWHAooDsso97Z
  ```

### Conventions carried forward verbatim from Plans 6 and 7 (binding, not advisory)

1. **Java wins.** A disagreement between this plan and the clone's HEAD is resolved for HEAD, reported, and recorded as a plan amendment.
2. **Markers carry the Java method name on the same line** (`audit-port.sh` is line-based).
3. **Descending item id.** `board.itemList.startReadObject()` walks descending (quirk #63); the port's answer is `Board::get_items(..).rev()`. Every `TreeSet<Item>` is a `BTreeSet<ItemId>` iterated **descending** (quirk #44).
4. **`java_min` / `java_max`** (`fr_geometry`) wherever Java calls `Math.min`/`Math.max` on `double`s — NaN-propagating, unlike `f64::min`/`max` (Plan 1 ruling 10).
5. **`java_round`** (`fr_geometry`) wherever Java calls `Math.round` — half-up on ties, not banker's. *(No `java_round_to_int` exists; add one in the task that needs it and say so.)*
6. **`JavaTreeSet` where a comparator is non-total or a key can mutate after insertion** (controller ruling Y). Plan 8 must record the decision **per container**; its candidates are few and named in Task 4 and Task 9.
7. **The `Line` identity-token contract** (controller ruling AE): every new `Line` comes from a constructor that mints a fresh token; a *copied* `Line` keeps its token. The token takes no part in `PartialEq`/`Eq`/`Hash`/`Debug`. Plan 8's KiCad reader constructs `Line`s and `Polyline`s and must pick `new_polyline` vs `new_polyline_in_place` **deliberately**, with the Java line that decides it in a comment — exactly as `fr-dsn`'s DSN reader does.
8. **Gson declaration order is the JSON key order, and nulls are omitted.** Every manifest/report struct in this plan is an **ordered struct or a `BTreeMap`** — never a `HashMap` — and every optional field is `#[serde(skip_serializing_if = "Option::is_none")]`. Task 4 states this once and every JSON surface obeys it.
9. **Feed the rules slots bytes, never a parsed object** (quirk #142 — the `.rules` file is parsed twice against two different layer structures). `SettingsInputs::{cli_rules, scheduler_rules}` take `Option<&[u8]>` and Plan 8 supplies `std::fs::read(path)`.
10. **`resolve_headless` is called once per run and `validate()` is never called a third time** (quirk #140 — `validate()` is not idempotent: `max_passes == 0` becomes `Integer.MAX_VALUE` on the first call and `9999` on the second, and the headless path calls it exactly twice). `max_passes == 0` means *unlimited* everywhere in the port.

## Rulings made while writing this plan (recorded here so they reach the user)

**The controller's pre-plan rulings AO–AV, the risk-2 ruling and the General clause (`scratchpad/plan8-rulings.md`, 2026-08-30) are binding and are restated here so the implementer needs one document:**

- **AO — the MCP contract is spec §13's four tools verbatim, with flat arguments.** Java's OpenAPI-derived registry (`api/mcp/OpenApiMcpToolRegistry.java`, 659) is rostered `// not ported:`. Where the port's tool *outputs* overlap Java's DTOs, keep `api/dto/BoardFilePayload`'s field names — `job_id`, `data`, `size`, `crc32`, `format`, `statistics`, `filename`, `path` — so an agent written against the Java server is not gratuitously broken. **`p8t6` is a documented-delta driver** (each delta listed with the Java behaviour beside it), not a parity driver.
- **AP — cancellation is option (a):** an external `Arc<AtomicBool>` `CancelToken` that the routing thread copies into `RouterStop` at the six existing poll sites. **Plan 7's `RouterStop` interface is untouched.** If a poll site turns out to be missed, **add a poll, never a lock**.
- **AQ — the dead legacy flags stay dead.** `-oit`, `-us`, `-is`, `-hr`, `-inc`, `-drc`'s router switch and `-mt`/`--threads` reproduce quirks #131 and #143: parsed, normalised, stored on `LegacyBridge`, read by nothing. The same knobs are exposed **only** through the native `--set section.field=value` path, where no jar disagrees. Recorded as a **product decision** in the hand-off, not as a detail.
- **AR — CLI compatibility is bug-for-bug on the legacy path.** `startsWith` flag matching, silent missing-value no-op, warn-and-continue on an unknown flag, and **exit codes 0 and 1 only** — every legacy-path failure maps to **1**. Exit codes 2 and 3 are **reserved for the native subcommand form** and recorded as a port-only extension. `-di` stays unsupported. There is **no `-v`** on the legacy path (Java's log-level flag is `-ll`). **`p8t5` measures the legacy surface.**
- **AS — rostered, not ported:** `board/state/BoardComparator.java` (758) together with `--compare-boards`; `core/scoring/BoardScoreBreakdown.java` (192) and `ScoringWeightComparison.java` (232) — dead reachability plus a known-divergent formula (quirk label X). Stated in the hand-off.
- **AT — one `fr_core::PARITY_VERSION`** (the jar value each format surface was pinned against) for every FILE-FORMAT field; `CARGO_PKG_VERSION` only in the MCP `serverInfo`; the newer MCP `protocolVersion` is kept because the port is a new server. **Corrected at the site by ruling 5 below:** the SES/DSN `(hostCad …)`/`(hostVersion …)` fields are **input-derived** and never carry the app version, so the ruling's list reduces to the DRC report's `freerouting_version`, the manifest's `app_version` and `BoardStatistics.host`'s (unreachable) fallback.
- **AU — no HTTP API.** `api/**` is rostered in full (8 425 lines; `JobControllerV1`'s 659 are provably dead — not in `FreeroutingApplication.getClasses()`). **`ApiSettings`' priority-70 tier STAYS**: the MCP `route_board { settings? }` argument composes exactly that tier, per the `obligation:` at `crates/fr-settings/src/resolve.rs:184`.
- **AV — jars and gates.** Every end-to-end `p8t*` driver runs against the **HEAD jar** (it is what routes and DRCs; Plan 3's 2.3.0 pins already cover the DSN/SES format surfaces and stay untouched). **`p8t1`'s gate is the SES BYTE-diff plus the exit code plus a normalised-log comparison**; any divergence is an `XDIFF` row **with a root cause**, never a tolerance.
- **Risk 2 — the `HeadlessBoardManager` overrides (~370 lines) are Plan 8's, EARLY.** `applyCopperToEdgeClearanceOverride`, `applyHoleClearanceOverride` and `assignHoleKeepoutClearanceClass` run on every real DSN load and are **unrostered by any plan** — no `// not ported:` and no `// added in Plan N:` marker exists for them, because `management/` has never been audited. Task 3 owns them, **unless Plan 7 Task 16 found they fire on the batch corpus and gained an insert task** — in which case Plan 8 inherits only the audit and Task 3 shrinks to the load/save sequence. **A `management/` audit map is written either way.**
- **General:** quirk ids are assigned **at write time from the register's last row**, never from this document. Task size ≤ ~600 Java lines. New-code tasks (the MCP) are pinned by **contract tests plus the documented-delta driver**, not by equality. **No new dependency without a ruling — and the MCP schema need specifically may NOT add `schemars`;** hand-write the four tool schemas as JSON literals and revisit only if that proves error-prone.

**And the plan's own rulings:**

1. **`fr-core` is a thin composition layer and re-exports rather than moves.** Plan 7 controller answer 3 chose re-export: `fr_core::{BoardStatistics, RoutingEvent, ProgressSink, PassRecord, TaskState}` are `pub use fr_router::{score, pipeline}::…`, and `fr_core::RoutingPipeline::run` wraps `fr_router::pipeline::run_pipeline` `[verify at pre-flight]`. **Nothing in `fr-router` moves file.** *Reason:* a file move plus every `use` in the workspace, on the last plan, buys a tidier crate boundary and risks the one thing that must not move — the router. *Cost if wrong:* `fr-core` is partly a façade, which the hand-off states plainly.
2. **The `CancelToken` → `RouterStop` adapter preserves the THREE-state distinction and is a no-op when nothing cancels.** Ruling AP's `Arc<AtomicBool>` is a *bool*; `RouterStop` is a three-state machine and quirk #200 rides on the difference (`--max-items` hits `requestStop()` = `ALL` and **silently disables the optimizer**, while `--max-passes` hits `requestStopAutoRouter()` = `AUTO_ROUTER_ONLY` and does not). The port's `CancelToken` therefore carries **two** atomics — `cancel_all` and `cancel_auto_router` — plus the optional deadline, and the adapter maps them onto `RouterStop::{request_stop, request_stop_auto_router}` at the six existing `poll_deadline` sites. **A single `AtomicBool` collapsing the two would be a silent behaviour change**, which is exactly what Plan 7's hand-off warns Plan 8 against. *Cost if wrong:* an MCP cancel that stops the router but leaves the optimizer running, or vice versa — invisible without quirk #200's test.
3. **Two threads, both in `crates/freerouting/src/mcp/`, and the routing thread owns the board.** The stdio server becomes: a **reader thread** parsing stdin lines into an `mpsc` channel; the **main thread** dispatching and owning a `Mutex<W>` writer; a **tool thread** per `tools/call` that runs the routing and reports progress through a `Mutex`-guarded writer clone. `notifications/cancelled` arriving on the main thread flips the tool's `CancelToken`. **This is the only concurrency in the whole port**, it carries no routing policy, and `fr-router` never sees a thread. The two in-tree obligations that demand exactly this (`mcp/server.rs:9-32`, `mcp/stdio.rs:18-28`) are closed by Task 11 with their text rewritten to what landed. *Cost if wrong:* Java's own answer is option (c) — no cancellation at all — so the fallback is a documented delta, not a broken build.
4. **One new recovery boundary: the MCP tool call.** A panic inside a tool (a Java NPE equivalent surfacing through Plans 1–7's totalisations) must become a JSON-RPC error object on that request, not the death of the server — because a stdio MCP server that dies mid-conversation loses every subsequent request, and Java's blocking `HttpClient.send` does not have that failure mode. `catch_unwind` at exactly one site, `mcp::server::dispatch_tool`, with the panic payload rendered into `RpcError::internal` and a test that the server answers the *next* request. **The CLI gets no such boundary** — `route`/`drc` panic exactly where Plan 6/7's boundaries let them. *Cost if wrong:* a port-only recovery that hides a real bug; the test asserts the error is *reported*, never swallowed.
5. **`PARITY_VERSION` does not reach the SES or the DSN, and this plan says so rather than repeating ruling AT's list. (This corrects the ruling and the survey.)** `(hostCad …)` / `(hostVersion …)` are written by `io/specctra/parser/Parser.java:108-115` from `ReadScopeParameter.{hostCad, hostVersion}` (`:69`), which `Parser.readScopeParameter:204` fills **from the input file**; `crates/fr-dsn/src/ses_writer.rs:297` already does the same through `board.communication`. `grep -rn "FREEROUTING_VERSION" src/main/java` returns **five** sites, all in `Freerouting.java` (`:69`, `:1260`, `:1334`, `:1370`, `:1388`) — a log line, the runtime-environment string, and three analytics/version-check calls. So `PARITY_VERSION`'s real file-format readers are exactly three: the DRC report's `freerouting_version` (`drc/DesignRulesChecker.java:213`, `"Freerouting " + …`), the manifest's `app_version` (`RoutingResultManifest.java:102`) and `BoardStatistics.host`'s fallback (`:118-120`, **unreachable** — quirk label AK). **`Constants.java` is build-generated and is not in the clone's source tree**, so the value must be read out of the HEAD jar at pre-flight (`unzip -p <jar> app/freerouting/Constants.class | strings`, or a one-line reflective probe) and pinned as a literal with the jar's `Build-Revision` beside it. *Cost if wrong:* a manifest `app_version` that no jar ever wrote, and a DRC byte-diff against eight committed references.
6. **The `-drc` stdout branch is UNREACHABLE in Java and the port makes it live — a deliberate, recorded divergence, and it is the only one on this path.** `initializeDrc` is entered only when `drcReportFile != null` (`main:1462`) and `drcJob.drc` is that same object, so `IO.println(json)` at `:368-371` can never run from a command line (quirk label A). Spec §12 says "`drc` and `info` write JSON to stdout when no `-o`", so the port's `drc` with no `-o` prints to stdout. **Everything else on the `-drc` path is bug-for-bug**, including quirk B (`initializeDrc` returns `true` unconditionally → `-drc` exits 0 whatever happens) and quirk C (the quality score uses a *different* settings merge from the router's). `p8t3` therefore compares only invocations **with** a report path, and the stdout mode gets its own port-only test. *Cost if wrong:* a mode Java cannot reach cannot break parity; the risk is that a reader mistakes it for a bug fix, which the quirk row prevents.
7. **`isCliTerminalState`'s missing `INVALID` is TOTALISED, not reproduced.** Java's `:189-194` omits `INVALID`, the scheduler assigns it for a null or non-DSN/JSON input (`RoutingJobScheduler.java:83`, `:253`), and the `:151-158` poll loop then **spins forever** at 500 ms — `-de x.ses -do y.ses` is the repro (quirk label N). The port treats `INVALID` as terminal and exits **1**. *Reason:* an infinite hang is not a behaviour a parity harness can compare, and every other totalisation in this port turns a crash or hang into the value Java would have produced had it terminated. **`p8t5` records the divergence with a `timeout 10` on the Java side** and the row says "Java hangs; port exits 1". *Cost if wrong:* nil — no Java run ever completes here, so nothing can disagree.
8. **The two nested polling loops are NOT reproduced, and their observable effect is. (This corrects the survey, which listed quirk J only as latency.)** Java's `initializeCli` polls at 500 ms and the scheduler at 250 ms (quirk label J), adding ~0.75 s to every single-shot run and **quantising the manifest's `phases.autorouter.duration_seconds`**. The port calls the pipeline directly — there is no queue and no daemon. The consequence is that `duration_seconds` differs by up to 0.75 s, which is why **`p8t2` normalises it out** and why the manifest's timing fields are never a parity assertion. *Cost if wrong:* a manifest byte-diff that is pure scheduling noise.
9. **The SES write-out happens ONCE, and Java's per-event rewrite is a recorded divergence.** Java registers `addBoardUpdatedEventListener(e -> setJobOutput(job))` (`RoutingJobSchedulerActionThread.java:100`) and calls `setJobOutput` again at `:168`, re-serialising the whole board, recomputing CRC32 and running a full text-scrape `BoardStatistics` on **every** board update (quirk label K). The port writes once, after `run_pipeline` returns. **The final bytes are identical** because the last event's output is overwritten by the final call — *except* on the KiCad JSON path, where quirk label T means the final board is **never** written and the output is whatever the last mid-run event produced. **Task 10 owns that distinction**: on the `.json` output path the port must reproduce the *observable* answer, and Task 10's steps say how it is measured rather than assumed. *Cost if wrong:* a `-do out.json` run that writes a more-routed board than the jar does.
10. **The pre-bootstrap argv parse is collapsed to one parse, and the two behaviours that differ are recorded.** Java parses argv **twice** — once raw before log4j init (`Freerouting.java:901-917`, `:1032-1065`) and once in `applyCommandLineArguments` — and the two disagree: `-dl` is `equals` in `main:1056` but `startsWith` in `GlobalSettings.java:799`; `-ll` takes the **first** occurrence early and the **last** late (quirk label AJ). The port parses once and takes `GlobalSettings`' rule (`startsWith`, last-wins), because that is the rule that reaches the *settings*, and logging configuration is not a parity surface (spec §2 drops log files). Recorded, with `p8t5` asserting the settings-visible half. *Cost if wrong:* a log level that differs from the jar's on a doubled `-ll`, which no output file can observe.
11. **One audit map per new crate, and `management/`, `core/` and `api/` are audited for the first time in the project.** `scripts/audit-map/{fr-core.map, freerouting.map}` are new; Task 13 runs **twelve** invocations (§The audit, Task 13) over `core/`, `core/scoring/`, `core/results/`, `core/events/`, `io/kicad/`, `management/`, `management/jobs/`, `management/sessions/`, `api/`, `api/mcp/`, `logger/` and `Freerouting.java` itself, to **zero `MISSING` and zero `UNMAPPED`**. §5.7 of the survey exists because nobody ever ran the first of those. *Cost if wrong:* the audit is the only mechanical check that 3 300 lines arrived and 13 500 were consciously dropped; weakening it is forbidden.
12. **Ported Java tests: two suites plus the settings matrix.** `src/test/java/.../settings/GlobalSettingsCommandLineTest.java` becomes Task 5's argv table (Plan 4 already ported its `-de` half as `classify_de_arguments`' tests; Task 5 ports the rest through the **binary**). `src/test/java/.../core/scoring/ScoringWeightComparisonTest.java` is **not** ported — it is the sole reachability of ruling AS's two rostered classes, and the roster line cites it. Any `src/test/java/**` suite touching `RoutingJob`/`BoardFileDetails` is ported into Task 1; the implementer greps for it rather than trusting this sentence. *Cost if wrong:* free acceptance cases left on the table.
13. **Every end-to-end driver runs BOTH binaries with the same argv and diffs everything.** Unlike `p6t*`/`p7t*`, which reflect into jar internals, `p8t*` compares the two programs' **observable behaviour**: stdout, stderr (normalised), the exit code and every file written. `tests/parity` gains `run_jar(argv) -> (stdout, stderr, code)`, `run_port(argv) -> …`, `normalize_log`, `normalize_manifest` and `ManifestDoc`; **Task 6 builds them** and every later driver consumes them. *Cost if wrong:* seven drivers each re-inventing a process runner.
14. **The `.json` design-input divergence is CLOSED in this plan, in Java's favour.** `docs/cli-legacy-flags.md` records that Java has no dedicated KiCad-JSON slot (`.json` → design input when no `.dsn` was seen, session otherwise, `GlobalSettings.java:609-621`) while the port invented `--kicad-json`. **The legacy path adopts Java's rule exactly** (ruling AR is bug-for-bug), so `-de board.json -do out.ses` routes as it does in the jar; `--kicad-json` survives **only** on the native subcommand form, as a port-only extension. Task 5 makes the change, Task 9/10 make it work, and `docs/cli-legacy-flags.md`'s "Two consequences for Plan 8" paragraph is rewritten to say which way it went. *Cost if wrong:* the one legacy argv shape the port would still misclassify.

**Open questions for the controller are at the foot of this document (§Controller questions).**

## File Structure

```
crates/fr-core/                     NEW CRATE (spec §4)
  Cargo.toml                      deps: fr-router, fr-drc, fr-dsn, fr-settings, fr-board,
                                  fr-geometry, serde, serde_json, thiserror   (no clap, no tracing)
  src/lib.rs                      #![forbid(unsafe_code)]; PARITY_VERSION; the re-export surface;
                                  the ~13 500-line `// not ported:` roster (Task 0)
  src/cancel.rs                   CancelToken + the RouterStop adapter (Task 0, rulings AP + 2)
  src/progress.rs                 SyncProgressSink: a Sync wrapper over fr_router::ProgressSink (Task 0)
  src/ctx.rs                      Ctx, RoutingResult (Task 0, spec §10)
  src/pipeline.rs                 RoutingPipeline::run over fr_router::pipeline::run_pipeline (Task 0)
  src/timespan.rs                 TextManager.parseTimespanString + the 24 h cap + the 30 s grace (Task 0)
  src/job.rs                      RoutingJob, RoutingJobState, SessionId, FileFormat (Task 1)
  src/file_details.rs             BoardFileDetails (Task 1)
  src/stats_from_bytes.rs         BoardStatistics(byte[], FileFormat), countOccurrences (Task 2)
  src/stats_json.rs               the Gson-compatible JSON surface of fr_router::score (Task 2)
  src/load.rs                     loadFromSpecctraDsn / loadFromKiCadJson / applyParsedBoardResult /
                                  applyRouterSettingsForLoadedBoard / BoardLoader (Task 3)
  src/overrides.rs                applyCopperToEdgeClearanceOverride, applyHoleClearanceOverride,
                                  assignHoleKeepoutClearanceClass (Task 3, risk-2)
  src/save.rs                     saveAsSpecctraSessionSes, calculateCrc32ForBoard (Task 3)
  src/manifest.rs                 RoutingResultManifest + FixtureInfo/PhaseMetrics/PhaseDetail (Task 4)
  src/summary.rs                  the board-summary payload shared by `info` and `board_info` (Task 12)
  tests/*.rs                      per-task unit tests + the ported Java suites
  README.md
crates/fr-dsn/
  src/kicad/mod.rs                the module root (Task 8)
  src/kicad/dto.rs                KiCadBoardJson's DTO tree (Task 8)
  src/kicad/reader.rs             KiCadJsonReader.readBoard (Tasks 8, 9), importSession (Task 10)
  src/kicad/writer.rs             KiCadJsonWriter.write (Task 10)
  src/lib.rs                      the kicad re-exports; the SessionToEagle roster line re-worded
  src/parser/wiring.rs            :596 read_via_scope passes the checked insert (Task 3, plan-6 §11.5)
crates/fr-drc/
  src/lib.rs                      the six `added in Plan 8:` io/kicad lines consumed (Tasks 8-10)
crates/fr-settings/
  src/lib.rs                      the parseTimespanString marker consumed (Task 0)
  src/resolve.rs                  the :184 obligation closed by fr-core's sparse composer (Task 12)
crates/fr-board/
  src/board/mod.rs                :57 BoardComparator -> `// not ported:` with ruling AS (Task 13)
crates/fr-router/
  src/score/mod.rs                the eight `added in Plan 8:` lines consumed or re-pointed (Tasks 2, 13)
  src/lib.rs                      :243-244 RoutingPipeline/NamedAlgorithm lines consumed (Task 0)
crates/freerouting/
  Cargo.toml                      + fr-core, fr-dsn, fr-settings, fr-router, fr-drc
  src/main.rs                     the real exit ladder (Task 5)
  src/cli.rs                      the native subcommand form, finished (Task 5)
  src/legacy.rs                   rewritten: calls fr_settings::classify_de_arguments +
                                  apply_command_line_arguments; bug-for-bug (Task 5)
  src/logging.rs                  the message set + level mapping, all to stderr (Task 5)
  src/commands/route.rs           initializeCli's real body (Task 6)
  src/commands/drc.rs             initializeDrc's real body (Task 7)
  src/commands/info.rs            the info mode (Task 12)
  src/mcp/{jsonrpc,server,stdio}.rs   the concurrent transport (Task 11)
  src/mcp/tools/{mod,route_board,check_drc,board_info,list_settings,schema}.rs   (Task 12)
  tests/{legacy_cli,mcp_stdio,cli_e2e}.rs
  README.md
scripts/audit-map/fr-core.map     NEW (Task 0, filled by every task, closed by Task 13)
scripts/audit-map/freerouting.map NEW (Task 0)
scripts/gen-cli-reference.sh      NEW — the HEAD-jar end-to-end reference generator (Task 6)
scripts/differential/java/P8T{1..7}.java
scripts/differential/java/probes/P8T{0,1,2,3,8,10}Probe.java
scripts/differential/rust/src/bin/p8t{1..7}.rs
scripts/differential/sweep-p8t5.sh
tests/parity/src/lib.rs           + run_jar, run_port, normalize_log, normalize_manifest, ManifestDoc
tests/reference/cli-fixtures.txt  NEW — the end-to-end stems
tests/reference/cli-<stem>/{argv.txt,route.ses,route.exit,route.log,manifest.json,drc.json,meta.txt}
docs/plan-8-handoff.md            the PROJECT completion report (Task 13)
docs/java-quirks.md               Plan 8's rows; the register closed
docs/cli-legacy-flags.md          the `.json` divergence paragraph rewritten (ruling 14, Task 5)
```

---

## Interfaces — the cross-task index

Every type below is **produced once** by the named task and **never re-declared**. A later task extends an `impl` block; it does not restate a struct. A task that finds itself writing `pub struct X` where an earlier task's §Interfaces produced already shows one **has made an error — stop and report it, do not shadow** (Plan 7 scan ruling 7's rule, carried forward).

| type / fn | produced by | consumed by |
|---|---|---|
| `fr_core::PARITY_VERSION`, `fr_core::Error` | Task 0 | 2, 4, 6, 7, 11, 12 |
| `CancelToken`, `CancelToken::as_router_stop`, `Deadline` | Task 0 | 5, 6, 7, 11, 12 |
| `SyncProgressSink`, `RecordingSink` (test-only) | Task 0 | 6, 11, 12 |
| `Ctx`, `RoutingResult`, `RoutingPipeline::run` | Task 0 | 6, 12 |
| `parse_timespan`, `job_timeout_deadline` | Task 0 | 5, 6 |
| `RoutingJob`, `RoutingJobState`, `SessionId`, `FileFormat`, `BoardFileDetails` | Task 1 | 2, 3, 4, 6, 7, 12 |
| `BoardStatistics::from_bytes`, `count_occurrences`, `stats_json::{to_gson_json, to_gson_string}` | Task 2 | 4, 6, 7, 12 |
| `load::{load_from_specctra_dsn, load_from_kicad_json, load_board_if_needed, LoadedBoard}` | Task 3 | 6, 7, 9, 10, 12 |
| `overrides::{apply_copper_to_edge_clearance_override, apply_hole_clearance_override, assign_hole_keepout_clearance_class}` | Task 3 | 3 only (called from `load`) |
| `save::{save_as_specctra_session_ses, calculate_crc32_for_board}` | Task 3 | 6, 12 |
| `RoutingResultManifest`, `FixtureInfo`, `PhaseMetrics`, `PhaseDetail`, `manifest::write` | Task 4 | 6, 12 |
| `legacy::rewrite` (rewritten), `cli::{Cli, Command, RouteArgs, DrcArgs, InfoArgs}` (finished), `logging::init`, `ExitCode` | Task 5 | 6, 7, 11, 12 |
| `parity::{run_jar, run_port, normalize_log, normalize_manifest, ManifestDoc}` | Task 6 | 7, 8, 9, 10, 12, 13 |
| `commands::route::run` (real) | Task 6 | 12 (`route_board`) |
| `commands::drc::run` (real) | Task 7 | 12 (`check_drc`) |
| `fr_dsn::kicad::{KiCadBoardJson + its 12 DTOs, UnitJson}` | Task 8 | 9, 10 |
| `fr_dsn::kicad::read_board` | Tasks 8 (part A) + 9 (part B, an `impl`/fn-body extension — **the signature is Task 8's**) | 3, 10, 12 |
| `fr_dsn::kicad::{write, import_session}` | Task 10 | 6, 7, 12 |
| `mcp::server::{State, ToolHandler (new signature), dispatch, ProgressWriter}`, `mcp::stdio::run_with` (concurrent) | Task 11 | 12 |
| `mcp::tools::{route_board, check_drc, board_info, list_settings}`, `mcp::tools::schema::*`, `summary::BoardSummary` | Task 12 | 12 (`info`) |

```rust
// ── crates/fr-core/src/lib.rs (Task 0) ──────────────────────────────────────────────────────────
/// The jar value every FILE-FORMAT version field was pinned against (controller ruling AT, as
/// corrected by plan ruling 5). **Three readers only**: the DRC report's `freerouting_version`
/// (drc/DesignRulesChecker.java:213, which prefixes `"Freerouting "`), the manifest's
/// `app_version` (core/results/RoutingResultManifest.java:102) and `BoardStatistics.host`'s
/// unreachable fallback (core/scoring/BoardStatistics.java:118-120, quirk label AK).
/// **NOT** the SES/DSN `(hostCad …)`/`(hostVersion …)`, which io/specctra/parser/Parser.java:108-115
/// echoes back from the input (ReadScopeParameter.java:69, filled at Parser.java:204).
/// `Constants.java` is build-generated and absent from the clone's source; the literal below is
/// read out of the HEAD jar at pre-flight and the jar's `Build-Revision` is recorded beside it.
pub const PARITY_VERSION: &str = /* read from the HEAD jar; do not guess */ "";
/// The crate version, used **only** in the MCP `serverInfo` (ruling AT).
pub const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

// ── crates/fr-core/src/cancel.rs (Task 0, rulings AP + 2) ───────────────────────────────────────
/// Spec §10's `CancelToken`, shaped so it can cross a thread boundary to the MCP reader
/// (`Arc<AtomicBool>` + optional deadline) **while preserving Java's three-state stop**
/// (plan ruling 2). `RouterStop` (fr_router::pipeline) is `Cell`-based and therefore `!Sync`;
/// this type is what is shared, and `as_router_stop` is what the routing thread holds.
#[derive(Debug, Clone, Default)]
pub struct CancelToken {
    /// `StoppableThread.requestStop()` -> `ALL`. Set by `notifications/cancelled` and by a
    /// deadline expiry (RoutingJobSchedulerActionThread.java:75 calls requestStop() 30 s before
    /// it writes TIMED_OUT, quirk #201).
    cancel_all: Arc<AtomicBool>,
    /// `StoppableThread.requestStopAutoRouter()` -> `NONE -> AUTO_ROUTER_ONLY` only. Quirk #200
    /// rides on the difference: `--max-items` stops ALL and silently disables the optimizer,
    /// `--max-passes` does not.
    cancel_auto_router: Arc<AtomicBool>,
    deadline: Option<Deadline>,
}
/// `job.timeoutAt` (RoutingJobSchedulerActionThread.java:45-51) plus the monitor's 30 s
/// `GRACE_PERIOD` (:25) — the port has no monitor thread, so the grace is arithmetic here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Deadline { pub stop_at: std::time::Instant, pub timed_out_at: std::time::Instant }
impl CancelToken {
    pub fn new() -> CancelToken;
    /// `RoutingJobSchedulerActionThread.threadAction:44-52` — the parsed timespan, capped at
    /// `MAX_TIMEOUT` (:24, 24 h), plus `GRACE_PERIOD` (:25, 30 s) for the TIMED_OUT boundary.
    pub fn with_timeout(total: std::time::Duration) -> CancelToken;
    pub fn cancel(&self);                            // -> ALL
    pub fn cancel_auto_router(&self);                // -> AUTO_ROUTER_ONLY
    pub fn is_cancelled(&self) -> bool;
    /// True once `deadline.timed_out_at` has passed — Java's `job.state == TIMED_OUT`, which
    /// `AutorouteBatchLoop:251-253` reads.
    pub fn is_timed_out(&self) -> bool;
    /// Ruling AP's adapter. Builds the thread-local `RouterStop` the pipeline takes, and returns
    /// a poll closure the routing thread calls at Plan 7's six `poll_deadline` sites; the closure
    /// copies **both** flags across, never a lock. A `CancelToken` that is never cancelled makes
    /// the closure a no-op, which is what keeps `batch_parity` byte-identical.
    pub fn as_router_stop(&self) -> (fr_router::pipeline::RouterStop, impl Fn() + '_);  // [verify at pre-flight]
}

// ── crates/fr-core/src/ctx.rs (Task 0, spec §10) ────────────────────────────────────────────────
/// Spec §10's `Ctx`. **There is no rng seed field**: plan-6 ruling 5 forbids `rand` and the port
/// has no randomness, so a seed would be a field nothing reads (`ItemSelectionStrategy::Random`
/// is GUI-only — see the roster). Recorded here rather than silently omitted.
pub struct Ctx<'a> {
    pub settings: &'a fr_settings::RouterSettings,
    pub cancel: CancelToken,
    pub progress: &'a SyncProgressSink,
    pub budget: fr_router::pipeline::RouterBudget,        // [verify at pre-flight]
}
/// Spec §10's `RoutingResult`.
pub struct RoutingResult {
    pub stats: fr_router::score::BoardStatistics,          // [verify at pre-flight]
    pub incompletes: Vec<(String, Vec<String>)>,           // build_unrouted_report [verify at pre-flight]
    pub drc_violations: Vec<fr_board::ClearanceViolation>,
    pub timed_out: bool,
    pub pipeline: fr_router::pipeline::PipelineResult,     // [verify at pre-flight]
}
impl RoutingPipeline {
    /// `RoutingPipeline.createForHeadless` + `run` (autoroute/pipeline/RoutingPipeline.java:45-47,
    /// :81-129), wrapping Plan 7's `run_pipeline`. Adds: the CancelToken adaptation, the DRC
    /// violations and incompletes (fr_drc), and the final BoardStatistics.
    pub fn run(board: &mut fr_board::Board, ctx: &Ctx<'_>) -> Result<RoutingResult, fr_core::Error>;
}
```

The remaining signatures are declared in their producing task's §Interfaces produced, with the Java line ranges that justify each field.

---

### Task 0: the `fr-core` crate — `CancelToken`, `ProgressSink`, `Ctx`/`RoutingResult`, the timeout ladder, `PARITY_VERSION` and the 13 500-line roster

**Files:** `crates/fr-core/{Cargo.toml,README.md}`, `crates/fr-core/src/{lib,cancel,progress,ctx,pipeline,timespan}.rs`, `crates/fr-core/tests/{cancel,timespan,pipeline}.rs`; `Cargo.toml` (workspace members); `crates/fr-settings/src/lib.rs:193` (the `parseTimespanString` marker — **consumed**); `crates/fr-router/src/lib.rs:243-244` (the `RoutingPipeline.*` / `NamedAlgorithm.*` markers — **consumed or re-pointed**) `[verify at pre-flight]`; `scripts/audit-map/{fr-core.map,freerouting.map}` (created, seeded); `scripts/differential/java/probes/P8T0Probe.java`.

**Java:** `util/TextManager.java (335)` — `parseTimespanString` `:83-93` (11), `convertFromTimespanToDurationFormat` `:101-118` (18), `removeQuotes` `:142-149` (8), `unescapeUnicode` (read the file for the range — it is the `\uXXXX` decoder `BoardStatistics.java:121` also runs); `management/jobs/RoutingJobSchedulerActionThread.java (296)` — `MAX_TIMEOUT` `:24`, `GRACE_PERIOD` `:25`, the timeout parse `:44-52` (9), the monitor thread `:55-90` (36, **the observable effect only**); `autoroute/pipeline/RoutingPipeline.java` — `createForHeadless` `:45-47`, `run` `:81-129` (the wrapper's shape; Plan 7 ported the body). **≈ 120 ported Java lines; ≈ 220 new.**

**Rostered here** (the whole ≈ 13 500-line `// not ported:` block lands in `crates/fr-core/src/lib.rs` in this task, so every later task can be audited against it): `api/**` (8 425, ruling AU — with `JobControllerV1.java`'s 659 carrying the **dead-code** evidence: absent from `api/FreeroutingApplication.getClasses()` `:36-63`, and three javadoc `@link`s its only references); `api/mcp/**` (2 105, ruling AO — read for tool *shape*, never transport); `analytics/**` (2 100) and `util/VersionChecker.java` (119) (spec §2); `management/sessions/SessionManager.java` (190) + `core/Session.java`'s job list (~40) (quirk label AA — `enqueueJob:315-318` reads `session.userId` purely to validate it and never uses it); `RoutingJobScheduler`'s queue half (~450 — the `:48-282` poll loop, `saveJob :350-377`, `saveJobToDisk :379-452`, `getQueuePosition`, three `listJobs`, `getJob`, `clearJobs`, `cancelJob`, with the evidence that `saveJobToDisk`'s `Files.write :445-450` is gated on `featureFlags.saveJobs` and called only from `api/v1/*`); `RoutingJobSchedulerActionThread.monitorCpuAndMemoryUsage :208-257` (50 — fills only `resource_usage`, which `p8t2` normalises out, and quirk label Y: the thread never exits because `job.thread` is never nulled, `:58`, and its `catch (Throwable t) {}` at `:253-256` is silent); `core/events/**` (6 files, 130 — all four listener registration points are in `api/v1/*`); `core/{RoutingJobPriority, RoutingStage}` (30 — `priority.value` is never read, ordering uses `ordinal()` at `RoutingJob.java:411-413`; `stage` is initialised to `IDLE` and never reassigned in `main/`); `HeadlessBoardManager.{compareCounterpartBoardIfPresent :172-207, loadBoardFromFileForComparison :209-237, validatePowerPlanes :914-1010, conductionAreasOverlap :879-898, getConductionAreaNetNames :900-912, scheduleDeferredPostLoadProcessing :759-784}` (~290, quirk label AE — the deferred pass is a virtual thread that races the router); `Freerouting.{initializeAPI :399-529, stopApiServer :532-540, initializeMCP :549-673, splitCommaSeparated :790-795}` (~330) and **`startMcpStdioBridge :681-788`** as `// renamed:` → `fr_core`/`crates/freerouting/src/mcp` (spec §13 replaces the pipe-to-HTTP bridge with a native server); `Freerouting.{compareBoardFiles :826-863, loadBoardFromFile :865-892}` (66) and `board/state/BoardComparator.java` (758) (**ruling AS**); `core/scoring/{BoardScoreBreakdown.java (192), ScoringWeightComparison.java (232)}` (**ruling AS**, with the reachability evidence: `src/test/java/.../ScoringWeightComparisonTest.java` and nothing else, and quirk label X's formula divergence); `io/specctra/parser/SessionToEagle.java` (627, spec §2 — the marker at `crates/fr-dsn/src/lib.rs:12` is **re-worded from `added in Plan 8:` to `not ported:`**); `gui/**` and `debug/DebugControl.java` (spec §2); `settings/{ApiServerSettings, …, McpServerSettings}` (260 — already rostered by Plan 4, cross-referenced not re-rostered).

**Interfaces consumed:** `fr_router::pipeline::{run_pipeline, PipelineResult, RouterStop, StopRequestState, RouterBudget, ProgressSink, RoutingEvent, NoopProgressSink, TaskState, build_unrouted_report}` **[verify at pre-flight]**; `fr_router::score::BoardStatistics` **[verify at pre-flight]**; `fr_drc::DesignRulesChecker` (Plan 5); `fr_settings::RouterSettings` (Plan 4); `fr_board::{Board, ClearanceViolation, TimeLimit}` (Plan 2).
**Interfaces produced:** `PARITY_VERSION`, `SERVER_VERSION`, `Error`, `CancelToken`, `Deadline`, `SyncProgressSink`, `Ctx`, `RoutingResult`, `RoutingPipeline::run`, `parse_timespan`, `job_timeout_deadline` — signatures in §Interfaces above, plus:
```rust
// crates/fr-core/src/progress.rs
/// Spec §10's sink, made shareable. Plan 7's `ProgressSink` is `&mut dyn` and single-threaded
/// [verify at pre-flight]; the MCP needs a sink whose writes go through a `Mutex`-guarded stdout
/// writer on another thread. This wrapper owns an `Arc<Mutex<dyn FnMut(&RoutingEvent) + Send>>`
/// and implements `fr_router::pipeline::ProgressSink` for the `&mut` view the pipeline takes.
/// **No port decision reads it** (Plan 7 ruling 11) — `a_recording_sink_changes_no_board_byte`
/// is re-run here against the whole pipeline, not just the router.
pub struct SyncProgressSink { /* … */ }
impl SyncProgressSink {
    pub fn noop() -> SyncProgressSink;
    pub fn new(f: impl FnMut(&fr_router::pipeline::RoutingEvent) + Send + 'static) -> SyncProgressSink;
    pub fn as_pipeline_sink(&self) -> impl fr_router::pipeline::ProgressSink + '_;
}

// crates/fr-core/src/timespan.rs
/// `TextManager.parseTimespanString` (util/TextManager.java:83-93). Java returns a `Duration` and
/// **throws** on a malformed string; the port answers `Option<Duration>` and the caller reproduces
/// Java's observable outcome at the one call site (RoutingJobSchedulerActionThread.java:44, inside
/// the method's own try/catch — read it, do not assume).
pub fn parse_timespan(s: &str) -> Option<std::time::Duration>;
/// `threadAction:44-52` — parse, cap at `MAX_TIMEOUT` (:24, 24 h), add `GRACE_PERIOD` (:25, 30 s)
/// for the TIMED_OUT boundary. Returns `None` when the string is blank/unparseable, which is
/// Java's "no job timeout".
pub fn job_timeout_deadline(timeout_string: Option<&str>) -> Option<Deadline>;
```

**Transcription notes, each with its Java line.**
- **`parseTimespanString`'s grammar is 11 lines and it is not obvious.** `TextManager.java:83-93` — read it; do not infer it from the strings `"01:00:00"` and `"1.5"`. `P8T0Probe.java` pins 30 inputs including `"1:30:00"`, `"90"`, `"1.5"`, `"1:2:3:4"`, `"x"`, `""`, `"25:00:00"` (over the cap), `"-1"`, `" 1:00 "`, and whatever the method's own regex/split rejects.
- **`MAX_TIMEOUT` and `GRACE_PERIOD` are literals at `:24` and `:25`** — transcribe the values from the file, do not use "24 h" and "30 s" from this plan.
- **The monitor thread is NOT ported** (quirk label Y). Its *observable* effect is two instants: `stop_at` (when `requestStop()` fires — `ALL`, quirk #201, which is why `AutorouteBatchLoop:251-253`'s `requestStopAutoRouter()` is always a no-op) and `timed_out_at` (30 s later, when `job.state = TIMED_OUT` is written). `Deadline` carries both; `poll_deadline` `[verify at pre-flight]` is where they bite.
- **`Ctx` has no rng seed** (see the doc comment above) and no `max_threads`. Spec §10 mentions both; the port has neither, and the README says why rather than leaving a field nothing sets.
- **`RoutingPipeline::run` composes, it does not decide.** It calls `run_pipeline` **[verify at pre-flight]**, then `fr_drc::DesignRulesChecker::new(&mut board)` for the violations and `build_unrouted_report` **[verify at pre-flight]** for the incompletes. It must **not** re-run `BoardStatistics::compute` if `PipelineResult::final_statistics` already carries it — quirk-sensitive: `BoardStatistics`' constructor runs `DesignRulesChecker` twice (`BoardStatistics.java:265-268`, `:338-341`) and Plan 5's memo behaviour makes the call count observable.
- **The `// not ported:` roster is 60+ lines and every line carries the grep that proves it.** Where the evidence is a caller set, **re-run the grep at port time and paste its output into the commit message** (Plan 7 ruling 6's precedent). A roster line with no evidence is a defect.
- `crates/fr-dsn/src/lib.rs:12`'s `SessionToEagle` line and `crates/fr-router/src/lib.rs:243-244` are **`added in Plan 8:` markers that this task consumes**: re-word to `// not ported:` (SessionToEagle, `RoutingPipeline.createForGui`, the six `NamedAlgorithm` listener methods) or `// renamed:` (`RoutingPipeline.createForHeadless`/`run` → `fr_core::RoutingPipeline::run`). Do not delete them.

**Tests (`crates/fr-core/tests/`).**
- `cancel.rs`: `a_cancel_from_a_second_thread_stops_a_run_mid_pass` (spawn a routing thread on `router-dac2020-bm01`, cancel after the first `RoutingEvent::BoardSnapshot`, assert `passes_run` < the uncancelled count); `cancel_all_and_cancel_auto_router_are_distinct` (ruling 2 — asserts `RouterStop::state()` reaches `ALL` vs `AUTO_ROUTER_ONLY`); `an_uncancelled_token_is_a_no_op` (the adapter's closure never changes `RouterStop::state`).
- `timespan.rs`: the `P8T0Probe` table as literals, one case per row, including the cap and the malformed strings.
- `pipeline.rs`: `a_recording_sink_changes_no_board_byte` (Plan 7 ruling 11's test, re-run through `RoutingPipeline::run`); `routing_result_carries_the_drc_violations_and_the_incompletes`.
- **Regression gate:** `cargo test -p fr-router --test batch_parity` **[verify at pre-flight]** unchanged — the adapter must be invisible.

**JVM-pinned evidence (required).** **`P8T0Probe.java`** (`package app.freerouting.util;`), run in `run.sh`'s `needs_jar=1` mode against the **HEAD jar** on JDK 25 with `-Djava.awt.headless=true -Duser.language=en -Duser.country=US -XX:+UnlockExperimentalVMOptions -XX:hashCode=2`, printing for each of 30 timespan strings: the raw string, `parseTimespanString`'s result or the exception class, the capped value after `:45-51`'s arithmetic, and `convertFromTimespanToDurationFormat`'s round trip. Transcript committed as `crates/fr-core/tests/data/p8t0-timespans.txt`; the Rust test asserts against those literals. **Acceptance: MATCH on all 30 rows.**

**Steps:** create the crate + workspace member → `P8T0Probe.java` → the timespan table → `CancelToken` + the adapter → `SyncProgressSink` → `Ctx`/`RoutingResult`/`RoutingPipeline::run` → the roster (with greps pasted) → the two audit maps seeded → consume the three in-tree markers → `batch_parity` unchanged → fmt/clippy/test/audit → commit `feat(core): fr-core — CancelToken, ProgressSink, the pipeline wrapper and the roster`.

---

### Task 1: `RoutingJob`, `BoardFileDetails`, `FileFormat` and `SessionId` — the job model

**Files:** `crates/fr-core/src/{job.rs,file_details.rs,lib.rs}`, `crates/fr-core/tests/job.rs`; `scripts/differential/java/probes/P8T1Probe.java`; `scripts/audit-map/fr-core.map`.
**Java:** `core/RoutingJob.java (548)` — **`getFileFormat(byte[])` `:151-227` (77)**, `getFileFormat(Path)` `:230-247` (18), `getCurrentPass`/`setCurrentPass` `:250-257` (8), `getDuration` `:260-268` (9), `setInput`×3 `:271-285` (15), `setRules`×3 `:288-310` (23), `tryToSetInput` `:335-349` (15), **`changeFileExtension` `:352-374` (23)**, **`tryToSetOutputFile` `:377-397` (21)**, **`setInputFromFile` `:425-461` (37)**, the four `log*` helpers `:526-547` (22), the state enum and the `ordinal()`-based ordering at `:411-413`; `core/BoardFileDetails.java (219)` — ctor `(File)` `:59-67` (9), `(BasicBoard)` `:70-72` (3), `calculateCrc32(InputStream)` `:75-87` (13), `getAbsolutePath` `:96-98` (3), **`setData` `:105-119` (15)**, `getFile` `:127-132` (6), **`setFilename` `:149-197` (49)**, `getFilenameWithoutExtension` `:200-205` (6); `core/Session.java (62)` — **the ctor's host normalisation and `split("/")` validation `:27-43` only** (17). **≈ 400 ported Java lines** (the state enum and `RoutingJobState`'s terminal set are read from the file, not transcribed from here).

**Interfaces consumed:** `fr_core::{PARITY_VERSION, Error}` (Task 0); nothing else.
**Interfaces produced:**
```rust
// crates/fr-core/src/job.rs
/// Port of `core.RoutingJob`'s **file/format/state** half. The queue half is Task 0's roster.
/// `Session` collapses to a `SessionId(Uuid-shaped [u8;16])` — plan ruling: the whole class exists
/// so `enqueueJob:315-318` can validate a `userId` it never uses (quirk label AA), and its ctor
/// mutates before it validates (quirk label Z, `Session.java:30-42`). The **validation** is ported
/// (the `host.split("/").length != 2` check, `:37`), the object is not.
pub struct RoutingJob { /* fields with their Java lines */ }
#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum RoutingJobState { /* read core/RoutingJobState.java */ }
impl RoutingJobState {
    /// `Freerouting.isCliTerminalState` (:189-194) **plus `INVALID`** — plan ruling 7 totalises
    /// the omission, because Java's `:151-158` loop spins forever on it (quirk label N).
    pub fn is_cli_terminal(self) -> bool;
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum FileFormat { Dsn, Ses, Rules, KicadDesignJson, KicadSessionJson, DrcJson, Unknown /* read RoutingJob.java's enum */ }
impl FileFormat {
    /// `RoutingJob.getFileFormat(byte[])` (:151-227) — magic-byte sniffing, **including quirk
    /// label O's leading-newline shift loop (:181-187), which never refills `buffer[5]` and spins
    /// forever on a file starting with >= 6 CR/LF bytes. TOTALISED**: the port bounds the loop and
    /// the `// totalized:` marker names the bound. The comment at :181 says `0x0A or 0x13`; the
    /// code tests `0x0A`/`0x0D` — transcribe the CODE.
    pub fn sniff_bytes(buf: &[u8]) -> FileFormat;
    pub fn from_path(p: &std::path::Path) -> FileFormat;      // :230-247, extension switch
}
impl RoutingJob {
    pub fn new(session: SessionId) -> RoutingJob;
    pub fn set_input(&mut self, path: &std::path::Path) -> Result<(), Error>;      // :271-285 + :425-461
    pub fn set_rules(&mut self, path: &std::path::Path) -> Result<(), Error>;      // :288-310
    /// `:377-397`. **Returns `bool`, and every caller in this plan must decide, in writing,
    /// whether it ignores it** — `Freerouting.java:123` ignores it (quirk label L).
    pub fn try_to_set_output_file(&mut self, path: &std::path::Path) -> bool;
    /// `:352-374`. Quirk label Q: NPEs on a bare filename (`filePath.getParent().toAbsolutePath()`)
    /// and, when the extension already matches, returns the **original, possibly relative** string
    /// rather than the reconstructed absolute path — asymmetric with its two other returns.
    /// TOTALISED for the NPE (answers the bare name), REPRODUCED for the asymmetry.
    pub fn change_file_extension(path: &str, ext: &str) -> String;
}
// crates/fr-core/src/file_details.rs
pub struct BoardFileDetails { /* size, crc32, format, statistics, filename, path — the
    `api/dto/BoardFilePayload` names ruling AO keeps */ }
impl BoardFileDetails {
    /// `:105-119` — size, CRC32, **re-sniff of the bytes** and the text-scrape statistics.
    /// The re-sniff is what makes quirk label T possible; Task 10 owns the consequence.
    pub fn set_data(&mut self, data: Vec<u8>);
    /// `:149-197` (49 lines). Quirk label R: the **Windows-only string surgery runs
    /// unconditionally**, `filename.contains(File.separator)` is platform-dependent, and
    /// `replaceAll("\\\\.$", "")` is a regex bug — `\\.` is "backslash then ANY char", so it
    /// strips the last two characters of any path ending in backslash-plus-anything.
    /// **Reproduced verbatim**, with the platform separator pinned to Java's on the port's host.
    pub fn set_filename(&mut self, filename: &str);
    pub fn calculate_crc32(data: &[u8]) -> u32;                 // :75-87
}
```

**Transcription notes.**
- **`getFileFormat(byte[])` is 77 lines of byte comparisons; transcribe it, do not summarise it.** It decides `(ses` → SES, `{` → KiCad design JSON, `(pcb`/`(session` etc. Read every branch; the SES-vs-KiCad asymmetry at the heart of quirk label T lives here (`:160-161`).
- **`tryToSetOutputFile` registers the *output* listener as `fireInputUpdatedEvent()`** (`:390`) — a copy-paste bug; `setInputFromFile:440,446,452` gets it right. The port has no events (spec §10), so this is a **quirk row with a `// not reachable:` note**, not a reproduced behaviour. Say so at the site.
- **`setInputFromFile:425-461` derives the default output name**, which is what makes `-do out.txt` still receive SES bytes (quirk label L): `job.output` keeps the input-derived `<input>.ses` details while `Files.write` uses `initialOutputFile`. Task 6 consumes both halves; Task 1 just gets the derivation right.
- **`Session`'s ctor mutates before it validates** (`:30-42`, quirk label Z): `this.host` is assigned at `:34` and the `split("/").length != 2` check is at `:37`, throwing from a partially-constructed object. `SessionManager.setPrimarySession` does the same (`:157` before `:162`). The port validates first and the quirk row records the difference as **unobservable** (no caller reads the half-built object) — state that, do not hide it.
- **`RoutingJobPriority.value` is never read and `stage` is never reassigned** — Task 0 rosters both; Task 1 must not add them to the struct "for completeness".

**Tests (`crates/fr-core/tests/job.rs`):** the `P8T1Probe` table as literals — `sniff_bytes` over 40 byte prefixes (including the ≥ 6 CR/LF case, asserted against the port's bound with a `// totalized:` reference), `from_path` over 20 extensions, `change_file_extension` over 12 paths (bare name, relative, absolute, extension-already-matching), `set_filename` over 15 filenames (trailing backslash, backslash-plus-anything, no extension, dotted directory), `calculate_crc32` over the corpus files. Plus **ruling 12's grep**: `grep -rln "RoutingJob\|BoardFileDetails" ../freerouting/src/test/java` — port whatever it finds, and if it finds nothing, say so in the commit message.

**JVM-pinned evidence (required).** **`P8T1Probe.java`** (`package app.freerouting.core;`), HEAD jar, JDK 25, the standard flag set, printing the five tables above field for field. Transcript committed as `crates/fr-core/tests/data/p8t1-job-model.txt`. **Acceptance: MATCH on every row, with the two totalised rows recorded as `XDIFF` with the port's answer and the Java behaviour beside it** (the shift-loop hang and the `changeFileExtension` NPE) — a hang is timed out at 5 s on the Java side and the transcript says so.

**Steps:** `P8T1Probe.java` → the five tables → `FileFormat` → `BoardFileDetails` → `RoutingJob` → `SessionId` → the test-suite grep → fmt/clippy/test/audit → commit `feat(core): the job model — RoutingJob, BoardFileDetails, FileFormat, SessionId`.

---

### Task 2: `BoardStatistics(byte[], FileFormat)`, `countOccurrences`, and the Gson-compatible JSON surface

**Files:** `crates/fr-core/src/{stats_from_bytes.rs,stats_json.rs,lib.rs}`, `crates/fr-core/tests/stats.rs`; `crates/fr-router/src/score/mod.rs:51-53` (three `added in Plan 8:` markers — **consumed**) `[verify at pre-flight]`; `scripts/differential/java/probes/P8T2Probe.java`.
**Java:** `core/scoring/BoardStatistics.java (648)` — **`BoardStatistics(byte[], FileFormat)` `:436-552` (117)**, `countOccurrences` `:578-586` (9), `toString` `:589-591` (3, `GsonProvider.GSON.toJson(this)` — the JSON surface itself), the field block `:37-79` (for the serialisation order), the `host` fallback `:114-121` and the bounding-box assignment `:126-131`, `:385-386` (quirk label AK); `util/TextManager.unescapeUnicode` (used at `:121`); the ten DTO classes' field order (`BoardStatisticsBoard`, `…Layers`, `…Items`, `…Components`, `…Pads`, `…Nets`, `…Connections`, `…Traces`, `…Bends`, `…Vias`). **≈ 140 ported Java lines.**

**Interfaces consumed:** `fr_router::score::{BoardStatistics + the ten DTOs}` **[verify at pre-flight]**; `fr_core::{FileFormat, PARITY_VERSION}` (Tasks 1, 0).
**Interfaces produced:**
```rust
// crates/fr-core/src/stats_from_bytes.rs
impl BoardStatisticsExt for fr_router::score::BoardStatistics {         // an extension trait —
    /// `BoardStatistics(byte[], FileFormat)` (:436-552). The SES / DSN / KiCad-JSON **text
    /// scraper**, which is a different object from the computing constructor Plan 7 ported: it
    /// never builds a board.                                           // [verify at pre-flight]
    fn from_bytes(data: &[u8], format: FileFormat) -> fr_router::score::BoardStatistics;
}
/// `:578-586` — a **substring** count, which is the whole of quirk label F.
pub fn count_occurrences(haystack: &str, needle: &str) -> usize;
// crates/fr-core/src/stats_json.rs
/// `BoardStatistics.toString` (:589-591) = `GsonProvider.GSON.toJson(this)`: pretty-printed,
/// HTML escaping off, **declaration order**, **nulls omitted** (Convention 8).
pub fn to_gson_json(s: &fr_router::score::BoardStatistics) -> serde_json::Value;
pub fn to_gson_string(s: &fr_router::score::BoardStatistics) -> String;
```

**Transcription notes.**
- **Quirk label F is the point of the task: reproduce the substring counting, do not fix it.** `(layer` also counts `(layer_rule`; `(net` also counts `(network` and `(net_class`; `(via` also counts `(via_rule`; `(class` also counts `(class_class` (`:469-473`, `:514-519`). The SES branch's layer extraction splits on `"(path "` and takes `words[0]` of each chunk **after the first** — transcribe the off-by-one deliberately.
- **Quirk label G — the DSN host scrape almost always fails, and the port must fail the same way.** `searchLimit` is the **first `)` after `(parser`** (`:480-503`), which in a real DSN is the end of the first inner clause (e.g. `(string_quote ")`), so `(hostCad`/`(hostVersion` lie outside `parserScope` and `host` stays `null`. Compounded twice over: the keywords are HEAD's **camelCase**, so a correct Specctra `(host_cad` never matches; and `substring(hcIdx+9, …)` / `(hvIdx+13, …)` hard-code exactly one space. Three `// Java bug:` markers, one quirk row.
- **Quirk label AK — the `host` fallback is unreachable.** `this.host` is assigned `x + "," + y`, which is at minimum `"null,null"`, so `"Freerouting," + FREEROUTING_VERSION` can never fire. Port the fallback with a `// not reachable:` marker naming `PARITY_VERSION` as what it *would* have used (ruling AT/5's third reader).
- **The Gson surface's two naming traps.** `BoardStatisticsBends`' keys `"90_degree_count"` / `"45_degree_count"` begin with digits (legal JSON, needing an explicit `#[serde(rename)]`), and `BoardStatisticsItems.componentOutlineCount` serialises as **`"component_count"`** — a name collision with `components.total_count`. Both are in the DTO field blocks; **read them, do not guess the rename list**.
- **`unescapeUnicode` runs on `host` at `:121`** — it is a `\uXXXX` decoder; Plan 7 rostered it with the decoder transcribed inline `[verify at pre-flight]`. If Plan 7's copy landed, **re-use it**; if it did not, this task ports it and says so.

**Tests (`crates/fr-core/tests/stats.rs`):** the `P8T2Probe` table as literals, field for field, for **every corpus `.dsn`** (`tests/reference/fixtures.txt`'s stems), **every committed `.ses`** and every `tests/reference/<stem>/batch.ses` **[verify at pre-flight]**; `the_dsn_host_scrape_finds_nothing_on_a_real_dsn` (quirk G); `layer_rule_is_counted_as_a_layer` (quirk F); `bends_keys_start_with_digits`; `component_outline_count_serialises_as_component_count`; `the_json_key_order_is_declaration_order_and_nulls_are_omitted`.

**JVM-pinned evidence (required).** **`P8T2Probe.java`** (`package app.freerouting.core.scoring;`), HEAD jar, standard flags, printing for each input file: the `FileFormat` it was constructed with, then every DTO field with `Integer.toString`/`Double.toString`, then `toString()`'s exact JSON. Transcript committed as `crates/fr-core/tests/data/p8t2-byte-statistics.txt`. **Acceptance: 0 diffs on every corpus `.dsn`, every committed `.ses` and every `batch.ses`, including the byte-exact `toString()` JSON.**

**Steps:** `P8T2Probe.java` → the tables → `count_occurrences` → the three scraper branches → the Gson surface + renames → consume the three `score/mod.rs` markers → fmt/clippy/test/audit → commit `feat(core): the byte-scraping BoardStatistics twin and the Gson JSON surface`.

---

### Task 3: the board load/save sequence, and the three `HeadlessBoardManager` clearance overrides (risk-2)

> **Conditional scope.** Controller risk-2: Plan 8 owns `applyCopperToEdgeClearanceOverride`, `applyHoleClearanceOverride` and `assignHoleKeepoutClearanceClass` as an early task — **unless already landed by a Plan 7 insert task**, in which case this task keeps only the load/save sequence plus the `management/` audit map and the overrides' **audit**. **The pre-flight scan decides which, by `grep -rn "apply_copper_to_edge\|apply_hole_clearance\|assign_hole_keepout" crates/`;** the implementer does not decide it at port time.

**Files:** `crates/fr-core/src/{load.rs,overrides.rs,save.rs,lib.rs}`, `crates/fr-core/tests/{load,overrides}.rs`; `crates/fr-dsn/src/parser/wiring.rs:596` (plan-6 §11.5 — `read_via_scope` stops calling the unchecked `insert_via`); `scripts/audit-map/fr-core.map`; `scripts/differential/java/probes/P8T3Probe.java`.
**Java:** `management/HeadlessBoardManager.java (1011)` — ctor `:168-170` (3), `getRoutingBoard` `:255-257` (3), `replaceRoutingBoard` `:276-278` (3), **`createBoard` `:310-344` (35 — called *by the DSN parser*, `io/specctra/parser/Structure.java:1268`)**, **`applyHoleClearanceOverride` `:346-396` (51)**, **`assignHoleKeepoutClearanceClass` `:404-464` (61)**, **`applyCopperToEdgeClearanceOverride` `:466-552` (87)**, `getCurrentRoutingJob` `:570-572` (3), `calculateCrc32ForBoard` `:607-618` (12), `loadFromSpecctraDsn` `:673-705` (33), `applyParsedBoardResult` `:711-737` (27), **`applyRouterSettingsForLoadedBoard` `:739-749` (11)**, `applyImmediatePostLoadProcessing` `:751-757` (7), `loadFromKiCadJson` `:794-823` (30), `saveAsSpecctraSessionSes` `:862-877` (16); `management/BoardLoader.java (57)` — `loadBoardIfNeeded` `:19-56` (38, incl. the "only DSN and JSON" guard `:32-37`); `settings/DefaultSettings.java` — `DEFAULT_HOLE_CLEARANCE_UM` `:81`, `DEFAULT_COPPER_TO_EDGE_CLEARANCE_UM` `:78`. **≈ 370 ported Java lines** (≈ 170 if a Plan 7 insert task landed the three overrides).

**Interfaces consumed:** `fr_dsn::{read_board, read_metadata, BoardReadResult, BoardMetadata, CoordinateTransform, DsnReadOptions}` (Plan 3); `fr_dsn::kicad::read_board` — **stubbed here, discharged by Task 9**, with an explicit `obligation:` marker at the stub (Plan 7 scan ruling 6's precedent: no unannounced forward references); `fr_settings::{resolve_headless, SettingsInputs, RouterSettings}` (Plan 4); `fr_board::{Board, BoardRules::{get_hole_clearance, set_hole_clearance}, ClearanceMatrix}` (Plan 2); `Board::reduce_nets_of_route_items` (`board/query.rs:1070` — **ported in Plan 2 and never called; this task is its first caller**); `fr_core::{RoutingJob, BoardFileDetails}` (Task 1).
**Interfaces produced:**
```rust
// crates/fr-core/src/load.rs
/// `HeadlessBoardManager.loadFromSpecctraDsn` (:673-705) → `applyParsedBoardResult` (:711-737)
/// → `applyRouterSettingsForLoadedBoard` (:739-749) → `applyImmediatePostLoadProcessing`
/// (:751-757). **Free functions: there is no manager object** — its other 640 lines are
/// diagnostics, rostered in Task 0.
pub struct LoadedBoard { pub board: Board, pub transform: CoordinateTransform,
                         pub metadata: BoardMetadata, pub settings: RouterSettings }
pub fn load_from_specctra_dsn(bytes: &[u8], job: &mut RoutingJob, settings: &mut RouterSettings)
    -> Result<LoadedBoard, Error>;
pub fn load_from_kicad_json(text: &str, job: &mut RoutingJob, settings: &mut RouterSettings)
    -> Result<LoadedBoard, Error>;
/// `BoardLoader.loadBoardIfNeeded` (:19-56) — the DSN-vs-KiCad-JSON dispatch and the
/// **"only DSN and JSON formats are supported" guard (:32-37)**, which is why
/// `-de prev.ses -drc r.json` fails *here* rather than at the argument (quirk label S).
/// DRC-mode only; the route path loads through the pipeline.
pub fn load_board_if_needed(job: &mut RoutingJob) -> Result<LoadedBoard, Error>;
// crates/fr-core/src/overrides.rs   (risk-2)
pub fn apply_copper_to_edge_clearance_override(board: &mut Board, settings: &RouterSettings);  // :466-552
pub fn apply_hole_clearance_override(board: &mut Board, settings: &RouterSettings);            // :346-396
pub fn assign_hole_keepout_clearance_class(board: &mut Board, hole_clearance_board_units: i32) -> i32; // :404-464
// crates/fr-core/src/save.rs
pub fn save_as_specctra_session_ses(board: &Board, ct: &CoordinateTransform, design_name: &str,
                                    out: &mut impl std::io::Write) -> Result<(), Error>;       // :862-877
pub fn calculate_crc32_for_board(board: &Board, ct: &CoordinateTransform) -> u32;              // :607-618
```

**Transcription notes.**
- **`applyRouterSettingsForLoadedBoard` has FOUR steps and `fr_settings::resolve_headless` models only the first two.** `:741-744` (`setLayerCount` when it disagrees) and `:745` (`applyBoardSpecificOptimizations`) are Plan 4's, and `resolve.rs`'s module docs name exactly those lines. `:746` and `:747` are **board mutations** and are this task's. The port's `load` calls `resolve_headless` for the first two and `overrides::*` for the last two, **in Java's order**.
- **Quirk label AD — the two overrides run TWICE on a DSN load.** Once from `createBoard` (`:342-343`), which the **parser** invokes at `io/specctra/parser/Structure.java:1268`, and again from `applyRouterSettingsForLoadedBoard:746-747`. Each can re-trigger `searchTreeManager.reinsertTreeItems()`. **Reproduce the double application**; a single call is a divergence whenever the first pass changes what the second reads. The port's DSN reader has no manager callback, so `load_from_specctra_dsn` calls the pair explicitly at the point `Structure.java:1268` would have and again after — and the code comment says exactly that.
- **Quirk label AC — the early return makes a *defaulted* 500 and an *explicitly requested* 500 behave differently.** `:501-507`: when the value equals `DEFAULT_COPPER_TO_EDGE_CLEARANCE_UM` (`DefaultSettings.java:78`) **and** the outline does not use the fallback AREA class, the override is skipped. Transcribe both halves of the condition; this is why parity has held so far and why a KiCad-exported board breaks it.
- **`DEFAULT_HOLE_CLEARANCE_UM = 0.0`** (`DefaultSettings.java:81`) makes the hole override a no-op on a board whose rules already say 0 — which is every corpus fixture. **The task's evidence must therefore include a non-default run** (`--router.hole_clearance_um` ∈ {0, 100, 500}); a probe that only exercises the defaults proves nothing.
- **`assignHoleKeepoutClearanceClass` reclassifies KiCad's per-layer circular hole keepouts into a dedicated clearance class and returns its index** (`:404-464`). It writes the `ClearanceMatrix`; **quirk #83's J-then-I indexing must not be "fixed"** here any more than in the KiCad reader (Plan 3 hand-off §Plan 8).
- **`applyImmediatePostLoadProcessing:751-757` calls `board.reduceNetsOfRouteItems()`** (`RoutingBoard.java:1284-1356`), which `fr_board::Board::reduce_nets_of_route_items` ports at `board/query.rs:1070` and **which nothing in the port has ever called**. This task is its first caller. `validatePowerPlanes` on the next line is log-only and is Task 0's roster.
- **`saveAsSpecctraSessionSes` needs the `CoordinateTransform` the read produced** (Plan 3 ruling A). `LoadedBoard` is that holder — the hand-off's "whatever holds a `Board` between a read and a write must also hold the `CoordinateTransform`" is discharged by this struct, and the Plan 3 obligation row is ticked here.
- **`crates/fr-dsn/src/parser/wiring.rs:596`** (plan-6 §11.5): `read_via_scope` still calls the unchecked `insert_via`; it must pass the reader's own `normalize_time_limit`-backed check. **This changes the DSN reader's behaviour under the 105-file corpus**, which is why the controller re-pointed it here — so the change is made in the task that already re-runs the whole corpus, and `cargo test -p fr-dsn` plus `sweep-p3t15.sh` (**525 MATCH + 5 XDIFF, unchanged**) are the gate.

**Tests.**
- `crates/fr-core/tests/overrides.rs`: the `P8T3Probe` table as literals — the clearance matrix row for the board edge, `holeClearance`, the reclassified keepout item ids and the search-tree leaf count, **before and after each override**, on a KiCad-exported fixture and on `--router.hole_clearance_um` ∈ {0, 100, 500}; `the_override_runs_twice_on_a_dsn_load` (quirk AD, asserted through a call counter); `a_defaulted_500_and_an_explicit_500_diverge` (quirk AC).
- `crates/fr-core/tests/load.rs`: `only_dsn_and_json_are_accepted` (quirk S — `-de prev.ses` fails in the loader, with Java's message); `reduce_nets_of_route_items_is_called_exactly_once`; `the_loaded_board_carries_its_coordinate_transform`.
- **Regression gate (mandatory from here on):** `cargo test -p fr-router --test batch_parity` **[verify at pre-flight]**, `scripts/differential/run.sh p6t1` on the five Plan 6 stems, `cargo test -p fr-dsn`, and `scripts/differential/sweep-p3t15.sh` at 525 MATCH + 5 XDIFF — pasted into the commit message.

**JVM-pinned evidence (required).** **`P8T3Probe.java`** (`package app.freerouting.management;`), HEAD jar, standard flags, dumping — for each of {a KiCad-exported fixture, `Issue593-BBD_Mars-64.dsn`, `examples/tutorial_board/tutorial_board.dsn`} × {hole clearance 0, 100, 500} — the full clearance matrix, `board.rules.holeClearance`, every item whose clearance class the keepout pass changed (id + old class + new class), and the search tree's leaf count, at four points: after `createBoard`, after the parser returns, after `applyRouterSettingsForLoadedBoard`, and after `applyImmediatePostLoadProcessing`. Transcript committed as `crates/fr-core/tests/data/p8t3-clearance-overrides.txt`. **Acceptance: 0 diffs on every cell of every configuration** — this is the one gap no plan rostered, so a `SKIP` row is a failure.

**Steps:** the pre-flight conditional check → `P8T3Probe.java` → the tables → `overrides.rs` (three methods, both call sites) → `load.rs` (the four-step sequence, in order) → `save.rs` → the `wiring.rs:596` fix → `p3t15` + `p6t1` + `batch_parity` unchanged → the `management/` audit-map rows → fmt/clippy/test/audit → commit `feat(core): the board load sequence and the three HeadlessBoardManager clearance overrides`.

---

### Task 4: `RoutingResultManifest` — the twelve fields, the three nested DTOs, `resolveGitSha` and `sha256Hex`

**Files:** `crates/fr-core/src/{manifest.rs,lib.rs}`, `crates/fr-core/tests/manifest.rs`; `scripts/audit-map/fr-core.map`.
**Java:** `core/results/RoutingResultManifest.java (172)` — **the whole file**: the 12 `@SerializedName` fields `:28-65`, `FixtureInfo` `:68-74`, `PhaseMetrics` `:77-86`, `PhaseDetail` `:89-95`, **`fromJob` `:98-135` (38)**, `write` `:138-144` (7), `resolveGitSha` `:147-161` (15), `sha256Hex` `:163-171` (9); plus the two fields it reads from `RoutingJobSchedulerActionThread` (`startedAt :39`, `finishedAt :174`) and `RouterSettingsTypeAdapterFactory` (the `settings_snapshot` serialisation, which `fr_settings::json` already owns). **≈ 250 ported Java lines** (172 + the `fromJob` sources).

**Interfaces consumed:** `fr_core::{RoutingJob, BoardFileDetails, PARITY_VERSION}` (Tasks 0, 1); `fr_core::stats_json::to_gson_json` (Task 2); `fr_settings::json` (Plan 4, quirk #141's strictness split); `fr_router::score::BoardStatistics::normalized_score` **[verify at pre-flight]**.
**Interfaces produced:**
```rust
// crates/fr-core/src/manifest.rs
/// Port of `core.results.RoutingResultManifest` (RoutingResultManifest.java:20-171).
/// **Field order IS the JSON key order** (Gson serialises in declaration order, Convention 8):
/// schema_version, generated_at, app_version, git_sha, fixture, settings_snapshot, phases,
/// board_statistics, normalized_score, resource_usage, final_state, exit_code, output_written.
/// **Nulls are omitted** — `#[serde(skip_serializing_if = "Option::is_none")]` on every Option.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct RoutingResultManifest { /* the 12 fields, :28-65, each with its @SerializedName */ }
#[derive(Debug, Clone, PartialEq, serde::Serialize)] pub struct FixtureInfo { /* :68-74 */ }
#[derive(Debug, Clone, PartialEq, serde::Serialize)] pub struct PhaseMetrics { /* :77-86 */ }
#[derive(Debug, Clone, PartialEq, serde::Serialize)] pub struct PhaseDetail { /* :89-95 */ }
impl RoutingResultManifest {
    /// `:98-135`. The **clock is injected** (`generated_at` is `Instant.now().toString()` at :101).
    pub fn from_job(job: &RoutingJob, input_path: &std::path::Path, output_written: bool,
                    exit_code: i32, now: &dyn Fn() -> String) -> RoutingResultManifest;
    /// `:138-144` — Gson pretty, UTF-8, `createDirectories(parent)`, **no trailing newline**.
    pub fn write(path: &std::path::Path, m: &RoutingResultManifest) -> std::io::Result<()>;
}
/// `:147-161` — env `FREEROUTING_GIT_SHA`, then system property `freerouting.git.sha`, then
/// `FREEROUTING_GIT_SHA`, else `"unknown"`, **each `.trim()`ed**. The port has no system
/// properties: the two property lookups become env lookups of the same names, and the
/// `// renamed:` marker says so.
pub fn resolve_git_sha() -> String;
/// `:163-171` — lower-case `HexFormat`. **Returns `null` on any failure and Gson then omits the
/// key** (quirk label W), so an unreadable fixture is indistinguishable from a missing field.
pub fn sha256_hex(path: &std::path::Path) -> Option<String>;
```

**Transcription notes.**
- **Quirk label E — `phases.fanout` and `phases.optimizer` are allocated and never written** (always `{}`), while `phases.autorouter.duration_seconds` is the **whole job's** duration (`:79`, `:85`, `:124-132`). `isFanoutTimedOut()` / `isTimedOut()` are three lines away (`RoutingJobSchedulerActionThread.java:170-172`) and reach only a log line. **Reproduce both halves**: two empty objects and one wrong duration.
- **`sha256Hex` needs a SHA-256 and there is no dependency for one** — the Global Constraint forbids adding `sha2`. **Hand-write the 60-line FIPS-180-4 SHA-256 core in `manifest.rs`** with a NIST test-vector test (the three standard vectors plus an empty input) and a comment saying why it is not a dependency. *(If a workspace dependency already provides it, use that instead and say so — check `Cargo.lock` first.)*
- **`settings_snapshot` goes through `fr_settings::json`, not `serde_json::to_value`.** Quirk #141 governs the strictness split (Gson `Strictness.LENIENT` on the textual pass, strict value conversion, non-finite floats refused both ways). Plan 4 built exactly this; do not re-derive it.
- **`board_statistics` and `normalized_score` are a FULL recompute** including two `DesignRulesChecker` runs (`:117-120` → `BoardStatistics.java:110-427`). Task 6 supplies the already-computed `PipelineResult::final_statistics` **[verify at pre-flight]** — assert it equals a fresh recompute in a test, then use the cached one, and say so.
- **`app_version` is `PARITY_VERSION`** (ruling AT/5), never `CARGO_PKG_VERSION`.
- **`resource_usage.{io_read, io_written}` are never written by Java** — omit them, do not emit zeros. `p8t2` normalises the whole `resource_usage` object out anyway (quirk label J, plan ruling 8).

**Tests (`crates/fr-core/tests/manifest.rs`):** `the_key_order_is_declaration_order`; `nulls_are_omitted`; `fanout_and_optimizer_phases_are_empty_objects` (quirk E); `autorouter_duration_is_the_whole_job` (quirk E); `sha256_matches_the_nist_vectors`; `an_unreadable_input_omits_the_sha256_key` (quirk W); `resolve_git_sha_walks_all_three_sources_then_unknown`; `no_trailing_newline`; `the_parent_directory_is_created`.

**JVM-pinned evidence (required).** **`p8t2`** — the manifest driver (see §The differential harness). `P8T2.java` runs the HEAD jar's `-de <dsn> -do <ses> --router.result_json=<f>` and `p8t2.rs` runs the port's equivalent; both manifests are compared **byte-identical after `normalize_manifest`** strips `generated_at`, `phases.*.duration_seconds`, `git_sha`, `resource_usage` and absolute paths. Task 4 lands the driver against a **fixed-clock unit run** (the binary does not exist until Task 6), and **Task 6 turns it on end to end** — Task 4's own acceptance is the `P8T2.java` half plus the normaliser, with the transcript committed as `crates/fr-core/tests/data/p8t2-manifest-shape.txt`. Say this explicitly in the commit message so the split is not read as a gap.

**Steps:** `P8T2.java` + `normalize_manifest` → the DTO tree in declaration order → `from_job` → SHA-256 + NIST vectors → `resolve_git_sha` → `write` → the shape transcript → fmt/clippy/test/audit → commit `feat(core): RoutingResultManifest — the twelve fields, the git sha ladder and the sha256`.

---

### Task 5: the CLI surface — the legacy shim rewired bug-for-bug, the native subcommand form, the exit ladder and the logging map

**Files:** `crates/freerouting/src/{main.rs,cli.rs,legacy.rs,logging.rs}`, `crates/freerouting/Cargo.toml` (+ `fr-core`, `fr-settings`), `crates/freerouting/tests/legacy_cli.rs`; `docs/cli-legacy-flags.md` (the `.json` paragraph, ruling 14); `crates/fr-settings/src/sources/cli.rs:582` (the `added in Plan 8:` marker — **consumed**); `scripts/differential/java/P8T5.java`, `scripts/differential/rust/src/bin/p8t5.rs`, `scripts/differential/sweep-p8t5.sh`, `scripts/differential/{run.sh,README.md}`.
**Java:** `settings/GlobalSettings.java` — **`applyCommandLineArguments` `:521-838` (318)**, read for the *shape* only: Plan 4 already ported every value normalisation into `fr_settings::{apply_command_line_arguments, classify_de_arguments}`, so this task **deletes** the port's duplicate rather than porting again; specifically the `-de` classifier `:564-648`, `-do` `:655-659`, `-drc` `:660-669`, `-dr` `:670-674`, the numeric/strategy block `:675-736`, `-l` `:737-798` (the **only** flag matched with `equals`), `-dl` `:799-800`, `-da` `:801-802`, `-host` `:803-807`, `-inc` `:810-815`, `-dct` `:816-824`, `-ll` `:825-830`, the unknown-flag warn `:562`/`:833` and the loop's `catch` `:835-837`; `Freerouting.java` — the exit ladder `:1469-1495` (27), the `-help` exit `:1394-1398`, the DRC-vs-CLI branch `:1458-1467`, the `drcReportFile != null` disabling `:1401-1405`; `logger/{FRLogger.java (480), Log4j2ConfigurationFactory.java (137)}` — **not a port**: the message set, the level mapping, and the pattern `"%d{yyyy-MM-dd HH:mm:ss.SSS} %-6level %msg%n"` (`Log4j2ConfigurationFactory.java:29`) the normaliser strips. **≈ 150 ported Java lines, mostly deletion on the port side.**

**Interfaces consumed:** `fr_settings::{classify_de_arguments -> DeSlots, apply_command_line_arguments -> LegacyBridge, set_field_value, FieldSpec}` (Plan 4); `fr_core::{parse_timespan, CancelToken}` (Task 0).
**Interfaces produced:**
```rust
// crates/freerouting/src/legacy.rs — REWRITTEN
/// Rewrites a Java-freerouting command line into the subcommand form. **Ruling AR: bug-for-bug.**
/// - Every short flag except `-l` is matched with `startsWith` (GlobalSettings.java:521-838), so
///   `-decoy`/`-diff`/`-drcx` are accepted as `-de`/`-di`/`-drc` and `-mp -5` is inexpressible.
/// - A value is consumed only when the next argument exists **and does not start with `-`**;
///   otherwise the flag is a **silent no-op** and `i` is not advanced.
/// - An unknown flag **warns and continues** (:562, :833); a parse exception is caught (:835-837)
///   and the loop continues.
/// - **Every failure on this path maps to exit 1.** `LegacyError` loses its variants; the function
///   answers `Vec<String>` and records diagnostics for the caller to log.
/// - `-di` stays unsupported (warn + continue, which is what an unknown flag does).
/// - **There is no `-v`** on this path; Java's log-level flag is `-ll` (quirk label AH).
/// - Ruling 14: `.json` follows **Java's** slot rule — design input when no `.dsn` has been seen,
///   session otherwise (GlobalSettings.java:609-621). `--kicad-json` survives on the native form only.
pub fn rewrite(argv: &[String]) -> (Vec<String>, Vec<Diagnostic>);
/// The port-only exit codes, reserved for the **native** subcommand form (ruling AR).
pub enum ExitCode { Ok = 0, Failure = 1, UsageError = 2, NotImplemented = 3 }
// crates/freerouting/src/logging.rs
/// All logs to **stderr** (spec §12), which is a deliberate divergence from Java's Console
/// appender targeting SYSTEM_OUT (Log4j2ConfigurationFactory.java:58) with ERROR additionally
/// duplicated to SYSTEM_ERR (:88-96, :112-113) — quirk label AI, and the reason `p8t1`'s log
/// normaliser merges the two Java streams before comparing.
pub fn init(level: LogLevel);
/// The Java message set the CLI must emit, keyed by the FRLogger call site, so `normalize_log`
/// can map one to the other. **A table, not a translation layer.**
pub const MESSAGE_MAP: &[(&str, &str)];
```

**Transcription notes.**
- **This task DELETES more than it writes.** `legacy.rs` currently reproduces the `-de` rule itself (the `Path::exists` / `+`-splitting / extension-classification block) and forwards raw values. Plan 4 built `classify_de_arguments` and `apply_command_line_arguments` **precisely so this could be one call each** (plan-4 ruling 10, and the `added in Plan 8:` marker at `cli.rs:582`). Re-deriving either here would apply the normalisation twice or only on one path.
- **Ruling AQ at its site.** `-oit`, `-us`, `-is`, `-hr`, `-inc` and `-drc`'s `routerSettings.enabled = false` write `LegacyBridge` and **nothing reads it** (quirk #131). `-mt`/`--threads` likewise (quirk #143, extended by Plan 7 to "dead everywhere"). The port keeps them dead **and keeps parsing them**, because the parse is observable through `p8t4`'s resolved-settings dump. Each carries a `// Java bug:` marker naming the dead bridge, and the README says the same knobs are reachable through `--set`.
- **The exit ladder is Java's, with two additions.** `computeCliExitCode` (Task 6) gives 0/1; `--help` exits 0 (`main:1397`); `-drc` exits 0 unconditionally (quirk label B, Task 7). **2** is a *native-form* usage error (clap's own), **3** is reserved and, by the end of Task 12, must be **unreachable** — Task 13's checklist asserts `grep -rn "EXIT_NOT_IMPLEMENTED" crates/` returns nothing.
- **`-l <locale>` is dropped and that is correct** — it drives only the GUI's `TextManager` and the localised help text (`main:1394-1396`), and there is no `Locale.setDefault` anywhere in the Java tree. The port emits `en_US` formatting unconditionally; quirk #145's `%.4f` hazard (`DesignRulesChecker.java:336-343, 346-353, 489`) is Task 7's to state, and **every parity driver already runs `-Duser.language=en -Duser.country=US`**.
- **Plan ruling 10 at its site:** one argv parse, `GlobalSettings`' rule (`startsWith`, `-ll` last-wins). The two early-parse divergences (`-dl` `equals` at `main:1056`; `-ll` first-wins at `main:1060-1065`) get one quirk row and a `// not ported:` naming the pre-bootstrap pass.
- **`--mcp_server.stdio=true` is a `--section.field=value` form** that Java special-cases in its stdio pre-scan (`main:901-917`) and *ignores* from `freerouting.json` (quirk label AJ). The port's `mcp` subcommand is the supported spelling; the legacy form is accepted and rewritten to `mcp`, and `p8t6` drives both.

**Tests (`crates/freerouting/tests/legacy_cli.rs`):** ruling 12's port of `GlobalSettingsCommandLineTest.java` (grep the clone for its exact path and port every case **through the binary**); plus the `p8t5` table as literals — `-de a.dsn b.rules`, `-de "a.dsn+b.ses"`, `-de board.json` (ruling 14), `-de a.dsn prev.json`, `-mpx 5`, `-decoy a.dsn`, `-de a.txt`, bare `-drc`, `-mp` with no value, `-mp -5`, `--router.enabled=`, `-oit -5`, `-oit 0`, `-ll` twice, `-dl`/`-dlx`, an unknown flag, `-di`, no arguments at all.

**JVM-pinned evidence (required).** **`p8t5`** — the legacy-surface driver. `P8T5.java` (`package app.freerouting.settings;`) calls `applyCommandLineArguments` on each argv and dumps the resolved slots (`initialInputFile`, `initialOutputFile`, `initialRulesFile`, `designSessionFilename`, `drcReportFile.filename`), the `LegacyBridge` fields, and every warning line; `p8t5.rs` dumps the port's after `rewrite` + `apply_command_line_arguments`. **Acceptance: MATCH on ~60 argv shapes**, with each divergence an `XDIFF` row carrying its ruling (ruling 7's `INVALID` totalisation, ruling 14's `.json` change — which must be a **MATCH** after this task, since the port adopts Java's rule — and the port-only exit codes, which this driver does not exercise because it never runs a job). `sweep-p8t5.sh` reports `MATCH`/`XDIFF`/`SKIP` per row.

**Steps:** `P8T5.java` → the argv table → delete `legacy.rs`'s `-de` reproduction and call `classify_de_arguments` → `startsWith` matching + silent missing values + warn-and-continue → ruling 14's `.json` slot → the exit ladder → `logging.rs` + `MESSAGE_MAP` → `docs/cli-legacy-flags.md`'s paragraph rewritten → consume `cli.rs:582` → `p8t5` MATCH → fmt/clippy/test/audit → commit `feat(cli): the legacy shim rewired to fr-settings, bug-for-bug, with Java's exit ladder`.

---

### Task 6: `freerouting route` end to end — `initializeCli`, the settings chain, the SES write-out and the manifest hook

**Files:** `crates/freerouting/src/commands/route.rs`, `crates/freerouting/src/main.rs`, `crates/freerouting/tests/cli_e2e.rs`; `tests/parity/src/lib.rs` (**ruling 13's helpers — this task builds them**); `scripts/gen-cli-reference.sh`, `tests/reference/cli-fixtures.txt`, `tests/reference/cli-<stem>/*`; `scripts/differential/java/P8T{1,2}.java`, `scripts/differential/rust/src/bin/p8t{1,2}.rs`, `scripts/differential/{run.sh,README.md}`.
**Java:** `Freerouting.java` — **`initializeCli` `:79-187` (109)**, `writeCliOutputIfAvailable` `:196-213` (18), `computeCliExitCode` `:215-223` (9), `writeCliResultManifestIfRequested` `:225-244` (20); `management/jobs/RoutingJobScheduler.java (578)` — the **essential slice only**: the board load `:91-101` (11), **the second settings merge `:103-170` (68 — already linearised by `resolve_headless`)**, the post-merge `RulesReader.read` `:173-184` (12), `applyBoardSpecificOptimizations` `:186`, the optional session import `:189-234` (46), the thread hand-off `:237-241` and the terminal-state assignments; `management/jobs/RoutingJobSchedulerActionThread.java (296)` — `threadAction`'s live half `:37-206` minus the monitor (Task 0): `startedAt :39`, `routerEnabled :92-94`, `createForHeadless :99`, the two `setJobOutput` calls `:100` and `:168`, `finishedAt :174`, state finalisation `:175-184`, and **`setJobOutput` `:259-295` (37)**. **≈ 410 ported Java lines.**

**Interfaces consumed:** `fr_core::{RoutingPipeline::run, Ctx, RoutingResult, CancelToken, SyncProgressSink, RoutingJob, BoardFileDetails, RoutingResultManifest, load::*, save::*, job_timeout_deadline}` (Tasks 0–4); `fr_settings::{resolve_headless, SettingsInputs}` (Plan 4); `fr_dsn::ses_writer::write` (Plan 3); `legacy::rewrite`, `logging::init`, `ExitCode` (Task 5); `fr_router::pipeline::{RouterBudget, PipelineResult}` **[verify at pre-flight]**.
**Interfaces produced:** `commands::route::run(&RouteArgs) -> i32`; and in `tests/parity/src/lib.rs`:
```rust
/// Ruling 13's process runners. Both return the raw triple; **neither normalises** — the
/// normalisers are separate so a driver can assert on the raw bytes when it wants to.
pub fn run_jar(argv: &[&str]) -> (Vec<u8>, Vec<u8>, i32);
pub fn run_port(argv: &[&str]) -> (Vec<u8>, Vec<u8>, i32);
/// Strips log4j2's `"%d{yyyy-MM-dd HH:mm:ss.SSS} %-6level "` prefix
/// (Log4j2ConfigurationFactory.java:29), **merges Java's stdout INFO stream into its stderr**
/// (quirk label AI: the Console appender targets SYSTEM_OUT at :58 and ERROR is additionally
/// duplicated to SYSTEM_ERR at :88-96, so an ERROR appears three times), drops absolute paths,
/// durations and the donation banner (quirk label H), and maps through `MESSAGE_MAP`.
pub fn normalize_log(stdout: &[u8], stderr: &[u8]) -> String;
pub fn normalize_manifest(json: &str) -> ManifestDoc;
pub struct ManifestDoc { /* parsed, with generated_at / durations / git_sha / resource_usage removed */ }
```

**Transcription notes.**
- **The sequence, in Java's order, with nothing skipped:**
  1. the guard (`:80-86`) — no input **or** no output ⇒ return false ⇒ **exit 1**;
  2. `job.setInput` (`:101-106`), which reads bytes, sniffs the format and derives a default output name (Task 1);
  3. `job.input == null` ⇒ abort (`:108-112`) ⇒ exit 1;
  4. **DELETE the existing output file** (`:116-121`) — quirk label I: a failed or hung run destroys the previous result and writes nothing, and the failure only warns. **Reproduce it**;
  5. `job.tryToSetOutputFile` with the **return value discarded** (`:123`) — quirk label L;
  6. merge #1 (`:125-144`): `DsnFileSettings(job.input)` and, if `-dr`, `RulesFileSettings(job.rules)`;
  7. `merger.merge()` → `job.routerSettings` (`:146`) — `validate()` call **one**;
  8. `job.drcSettings = globalSettings.drcSettings.clone()` (`:147`);
  9. the board load (Task 3) and merge #2 (`RoutingJobScheduler.java:103-170`) — `validate()` call **two**, and quirk #140 says the two calls disagree about `max_passes == 0`;
  10. the post-merge `RulesReader.read` onto the merged board (`:173-184`) — **the second parse of the same `.rules` bytes**, quirk #142;
  11. the optional session import (`:189-234`);
  12. `run_pipeline` **[verify at pre-flight]**;
  13. `setJobOutput` (`:259-295`) → `SesWriter.write` → `output.setData`;
  14. `writeCliOutputIfAvailable` (`:196-213`) → `Files.write`, then `Files.exists && size > 0`;
  15. `computeCliExitCode` (`:215-223`);
  16. `writeCliResultManifestIfRequested` (`:225-244`).
- **Steps 6–10 are ONE call to `resolve_headless`.** `fr_settings::resolve_headless(&SettingsInputs, Option<&Board>, &HostEnvironment)` already linearises the whole two-merge ladder including both board passes, the private `boardSpecificTraceCostsApplied` flag, the post-merge `.rules` re-apply and both `validate()` calls, verified against `p4t1`'s 64-case matrix. **Do not re-derive precedence — fill `SettingsInputs` and call it once** (Convention 10), and **feed both rules slots `std::fs::read(path)`** (Convention 9, quirk #142). Quirks **#140–#143 are reproduced by that call**, and `p8t4` is what proves it through the binary.
- **Quirk label V — `-dr` with a non-existent path silently disables adjacent-`<design>.rules` discovery.** The scheduler's `else if` chain (`:132-152`) takes the `initialRulesFile != null` branch and the inner `exists()` then fails. `SettingsInputs` must be filled so that this is what happens; a test asserts it.
- **Plan ruling 9 at its site:** the SES is written **once**, not per board-updated event. The `addBoardUpdatedEventListener` line (`:100`) is `// not ported:` with `ProgressSink` named as its replacement, and the quirk row records that the final bytes are unaffected on the SES path (the last event's output is overwritten at `:168`) but **not** on the KiCad JSON path (quirk label T, Task 10).
- **Plan ruling 7 at its site:** `INVALID` is terminal and exits 1. The `// totalized:` marker names `Freerouting.isCliTerminalState` and the quirk row says "Java hangs".
- **Quirk label H — the donation banner is NOT reproduced.** `:164-183` prints to stdout when the output was written, the *persisted* `statistics.jobsCompleted >= 5` and the user e-mail is empty. It is un-suppressible in Java and would corrupt a stdout-JSON mode. `// not ported:` with the reason, and `normalize_log` strips it from the Java side.
- **`-do out.dsn` / `out.scr` writes a 0-byte file and exits 1** (quirk label L): `tryToSetOutputFile:384-388` accepts them, `setJobOutput` serialises only `KICAD_SESSION_JSON` and `SES`, so `output.getData()` stays empty. Reproduce it **including the empty file left behind** — `tests/reference/README.md` already records the behaviour.
- **The timeout ladder** is Task 0's `job_timeout_deadline`, fed from `router.job_timeout` (the string field Plan 4 kept as a `String`, as Java does), producing the `CancelToken`'s `Deadline`.

**Tests (`crates/freerouting/tests/cli_e2e.rs`):** `an_existing_output_file_is_deleted_before_routing` (quirk I); `do_out_txt_still_receives_ses_bytes` (quirk L); `do_out_dsn_writes_zero_bytes_and_exits_1` (quirk L); `de_a_ses_exits_1_instead_of_hanging` (ruling 7); `a_missing_dr_path_disables_rules_discovery` (quirk V); `no_donation_banner_on_stdout` (quirk H); `max_passes_zero_is_unlimited` (quirk #140); `the_rules_file_is_read_as_bytes_twice` (quirk #142, asserted through a read counter).

**JVM-pinned evidence (required).** **`p8t1` and `p8t2`, and this is the plan's headline gate (ruling AV).**
- **`p8t1`** runs `run_jar(["-de", dsn, "-do", ses])` and `run_port(["-de", dsn, "-do", ses])` for every stem of `tests/reference/cli-fixtures.txt` and asserts **(a)** the SES files are **byte-identical**, **(b)** the exit codes are equal, **(c)** `normalize_log` of both sides is equal. **No tolerance, ever** — a divergence is an `XDIFF` row in `crates/freerouting/README.md` with the first differing byte, the offending item and a one-line root cause.
- The stem set is **the eight Plan 7 batch stems** `[verify at pre-flight]` (`router-rpi-splitter`, `router-j2-reference`, `router-ecc83-input`, `router-empty-board` in CI; `router-dac2020-bm01`, `router-tutorial-board`, `router-fanout-bm11`, `router-strict-drc-cnh` behind `#[cfg_attr(debug_assertions, ignore)]` + `FR_SLOW_PARITY=1`) **plus the two Plan 3/5 round-trip stems** `tutorial_board` and `Issue026-J2_reference`. Plan 7's CI/slow split carries over unchanged.
- **`gen-cli-reference.sh`** is the sibling of `gen-batch-reference.sh` `[verify at pre-flight]`: it drives the **bare HEAD jar** (unlike Plan 7's generator, which needed a probe to disable the `optChangedArea` budget — **the CLI has no such need, because both sides run the budget live and the comparison is between two whole programs**), and writes `tests/reference/cli-<stem>/{argv.txt,route.ses,route.exit,route.log,manifest.json,meta.txt}` with the jar identity, `java -version`, the hash mode and every machine-specific prefix replaced. **State the budget difference from Plan 7 in the script header** so a reader does not think it was forgotten.
- **`p8t2`** adds `--router.result_json=<f>` to the same runs and compares `normalize_manifest` of both manifests **field for field**.
**Acceptance: `p8t1` MATCH on all four CI stems and all four slow stems; `p8t2` MATCH on the same set.**

**Steps:** the `tests/parity` helpers → `gen-cli-reference.sh` → the references → `route.rs`'s 16-step sequence → `p8t1` on the smallest stem → the rest → `p8t2` → the quirk rows → fmt/clippy/test/audit + `batch_parity` + `p6t1` → commit `feat(cli): freerouting route end to end — SES byte parity with the HEAD jar`.

---

### Task 7: `freerouting drc` end to end — the DSN → `.rules` → session order, the separate merge, the computed quality score

**Files:** `crates/freerouting/src/commands/drc.rs`, `crates/freerouting/tests/cli_e2e.rs`; `crates/fr-drc/src/report/json.rs:57` (the `obligation:` marker on `DrcJsonFlavor`'s CLI default — **closed**); `scripts/differential/java/P8T3.java`, `scripts/differential/rust/src/bin/p8t3.rs`.
**Java:** `Freerouting.java` — **`initializeDrc` `:246-374` (129)**: the guard `:247-250`, the session create `:253-257` (**the job is NEVER enqueued**), `drcJob.drc = globalSettings.drcReportFile` `:260-261`, `setInput` `:262-268` (**catch ⇒ `System.exit(1)` at `:267`**), `BoardLoader.loadBoardIfNeeded` `:271-274` (**false ⇒ `System.exit(1)` at `:273`**), the `.rules` load `:277-294`, the `.json`-vs-`.ses` session branch `:296-329`, `DesignRulesChecker` `:332-333`, `coordinateUnit = "mm"` `:336`, `sourceFileName` `:339`, `generateReport` `:340`, **the quality-score sub-merge `:342-352`**, `GsonProvider.GSON.toJson` `:354`, the file-vs-stdout write `:356-371` (**IOException ⇒ `System.exit(1)` at `:366`**), `return true` `:373`. **≈ 210 ported Java lines.**

**Interfaces consumed:** `fr_drc::{DesignRulesChecker::new, generate_report, report_to_json, DrcJsonFlavor, DrcReportOptions, DrcCoordinates, KiCadDrcReport}` (Plan 5); `fr_core::{load::load_board_if_needed, RoutingJob}` (Tasks 1, 3); `fr_dsn::{rules_reader::read, ses_reader::read}` (Plan 3); `fr_dsn::kicad::import_session` — **stubbed here, discharged by Task 10**, with an explicit `obligation:` marker; `fr_settings::{SettingsMerger, SettingsSource, priority}` (Plan 4); `fr_router::score::BoardStatistics::normalized_score` **[verify at pre-flight]**.
**Interfaces produced:** `commands::drc::run(&DrcArgs) -> i32`.

**Transcription notes.**
- **The load order is DSN → `.rules` → session, and it is load-bearing** (plan-5 hand-off, quirk label D). The `.rules` step **changes the clearance matrix and the net rules**, and the session's wires and vias are inserted *after* it, so they are created — and then checked — against the clearances the rules file installed. Swapping the last two steps yields a different violation list from the same three files. `tests/reference/drc-issue593-rules` and `drc-issue593-ses` are the committed references that exercise the two optional slots.
- **Quirk label C — the quality score uses a DIFFERENT settings merge from the router's** (`:342-352`): merge #1 with **only** `DsnFileSettings(drcJob.input)`, no `.rules`, no board pass, no merge #2. So `-dr x.rules -drc r.json` scores with weights the rules file never influenced while the *violations* were computed against the clearances it installed. **Reproduce it by composing `SettingsMerger` directly**, exactly as ruling AU's sparse-payload path does — **this is a second, independent user of the `resolve.rs:184` obligation** and the task must say so at the site so Task 12 does not think it is the only one.
- **Ruling W is discharged here:** the CLI passes `DrcJsonFlavor::KiCad` **explicitly**, because `DrcJsonFlavor::default()` stays `FreeroutingHead` — the *parity* default `fr-drc`'s own tests pin against the jar. The `obligation:` marker at `crates/fr-drc/src/report/json.rs:57` is **closed** with that sentence.
- **The quality score becomes a COMPUTATION** (plan-5 ruling 5): `fr_router::score::BoardStatistics::normalized_score` **[verify at pre-flight]** is an `f32`, widened to the report's `double` at `Freerouting.java:349` — which is why a value serialises as e.g. `999.9000244140625`. **Keep the `f32` computation and widen at the boundary.** The **eight committed `drc-*` references pin the values**, turning the injection into eight free acceptance cases.
- **Quirk label B — `initializeDrc` returns `true` unconditionally, so `-drc` exits 0 whatever happens.** A missing `.rules` file (`:289`), a missing session file (`:324`), a failed quality score (`:350`) and any number of violations all leave the code at its `0` default (`GlobalSettings.java:122`). **Only `:267`, `:273` and `:366` produce 1.** Reproduce all four exits exactly.
- **Ruling 6 at its site:** the stdout branch (`:368-371`) is unreachable in Java; the port makes it live per spec §12. `p8t3` therefore compares only invocations **with** a report path; the stdout mode gets `drc_with_no_output_prints_to_stdout` as a port-only test, and the quirk row says which half is which.
- **Quirk label S** — `-de prev.ses -drc r.json` fails inside `BoardLoader` with "only DSN and JSON formats are supported" (`BoardLoader.java:32-37`), not at the argument. Task 3 owns the message; this task owns the exit code (**1**, from `:273`).
- **`coordinateUnit` is hard-coded `"mm"`** (`:336`, quirk #151), which makes four of `convert_coordinate`'s five arms unreachable from `-drc`. Plan 5 ported all five because the MCP path can reach them. **The CLI does not expose a unit flag** — doing so would make the port more capable than Java; the decision is recorded, and the MCP `check_drc` tool does not expose one either (Task 12).
- **`date` is injected** (`KiCadDrcReport.java:70`, `ZonedDateTime.now().format(ISO_OFFSET_DATE_TIME)`); `freerouting_version` is `"Freerouting " + PARITY_VERSION` (`DesignRulesChecker.java:213`, ruling AT/5).
- **Quirk #144 — the `unconnected_items` array order is not reproducible in Java** (`getAllUnconnectedItems` iterates a `HashMap` at `:95`/`:104` and identity-hashed `HashSet<Item>`s at `:114`/`:123`). `fr-drc` handles it and **every parity run pins `-XX:hashCode=2`; `p8t3` must carry the same flags.**

**Tests:** `drc_exits_0_when_the_rules_file_is_missing` (quirk B); `drc_exits_1_when_the_input_is_unreadable` (`:267`); `drc_exits_1_when_the_board_will_not_load` (`:273`, quirk S); `drc_exits_1_when_the_report_cannot_be_written` (`:366`); `the_session_is_imported_after_the_rules` (quirk D — asserted by a violation-count difference between the two orders); `the_quality_score_uses_a_dsn_only_merge` (quirk C); `the_quality_score_is_an_f32_widened_to_f64`; `drc_with_no_output_prints_to_stdout` (ruling 6, port-only); `the_cli_passes_kicad_flavor_explicitly` (ruling W).

**JVM-pinned evidence (required).** **`p8t3`** — `run_jar(["-de", "<dsn>[+<ses>][+<rules>]", "-drc", report])` vs `run_port`, comparing the report **byte-identical after `normalize_drc_json`'s `date` normalisation** (`scripts/normalize-drc.py` already exists for the document), plus the exit code, plus `normalize_log`. Standard flags **including `-XX:hashCode=2`** (quirk #144) and `-Duser.language=en -Duser.country=US` (quirk #145). **Acceptance: MATCH on all eight committed `drc-*` stems, including `drc-issue593-rules` and `drc-issue593-ses`, with `quality_score` now COMPUTED rather than injected** — the eight references' existing values are the assertion.

**Steps:** `P8T3.java` → the eight stems → the DSN→rules→session order → the separate merge → the computed score → the four exit paths → the stdout mode → close the `json.rs:57` obligation → `p8t3` MATCH ×8 → fmt/clippy/test/audit + `batch_parity` + `p6t1` → commit `feat(cli): freerouting drc end to end — the load order, the separate merge and the computed quality score`.

---

### Task 8: `fr_dsn::kicad` part A — the DTO tree and `readBoard`'s sections 1–8 (units, layers, clearance matrix, outline, board construction, net classes)

**Files:** `crates/fr-dsn/src/kicad/{mod,dto,reader}.rs`, `crates/fr-dsn/src/lib.rs` (the `kicad` re-exports), `crates/fr-dsn/tests/kicad_reader.rs`; `crates/fr-drc/src/lib.rs:114,116,117,119` (four `added in Plan 8:` markers — **consumed**); `scripts/audit-map/fr-dsn.map`; `scripts/differential/java/probes/P8T8Probe.java`.
**Java:** `io/kicad/KiCadBoardJson.java (142)` — the whole DTO tree: `KiCadBoardJson` `:7-32`, `LayerJson` `:33-38`, `NetClassJson` `:40-49`, `NetJson` `:50-57`, `CustomClearanceRuleJson` `:58-64`, `ComponentJson` `:65-75`, `PadJson` `:76-87`, `OutlineJson` `:88-93`, `TraceJson` `:94-102`, `ViaJson` `:103-113`, `ConductionAreaJson` `:114-123`, `Point2D` `:124-141`, and `UnitJson`; `io/kicad/KiCadJsonReader.java (1011)` — the private ctor `:55`, **`readBoard`'s signature `:61-62` and sections 1–8, `:63-497` (435)**: `// 1. Deserialize JSON` `:77`, `// 2. Set up units and scaling` `:88` (incl. the `UnitJson.UM` arm `:92` and the `userUnit == Unit.UM` arm `:364`), `// 3. Layer Structure` `:104`, `// 4. Clearance Matrix` `:122`, `// 5. Board Outline / Boundary Shape Creation` `:165`, `// 6. Communication object setup` `:312` (incl. the `"KiCad"` / `"v10.0"` host fallbacks), `// 7. Construct RoutingBoard` `:326`, `// 8. Populate Net Classes & Netlist in Rules` `:340`. **≈ 577 ported Java lines** — at the ≤ ~600 ceiling, which is why sections 9–11 are Task 9.

**Interfaces consumed:** `fr_dsn::{BoardReadResult, BoardMetadata, CoordinateTransform, DsnReadOptions}` (Plan 3 — **the same types, not parallel ones**, plan-3 hand-off §Plan 8); `fr_board::{Board, BoardRules, ClearanceMatrix, LayerStructure, Communication, Net, NetClass, ViaInfo, ViaRule, DefaultItemClearanceClasses}` (Plan 2, with `ViaRule` owning its `ViaInfo`s per Plan 7 Task 0, landed as `bde59ef`); `fr_geometry::{IntPoint, PolygonShape, IntBox}` (Plan 1).
**Interfaces produced:**
```rust
// crates/fr-dsn/src/kicad/dto.rs — the twelve DTOs, serde-derived, field names EXACTLY Java's
pub struct KiCadBoardJson { /* :7-32 */ }   /* … the other eleven … */
#[derive(serde::Deserialize)] pub enum UnitJson { /* read the file */ }
// crates/fr-dsn/src/kicad/reader.rs
/// Port of `io.kicad.KiCadJsonReader.readBoard` (KiCadJsonReader.java:61-755). **Task 8 lands the
/// signature and sections 1-8 (:63-497); Task 9 lands sections 9-11 (:498-755) and the five
/// private helpers — a fn-body extension, NOT a second declaration.**
/// Answers `fr_dsn`'s own `BoardReadResult`, so `fr-core`'s load path is format-agnostic.
pub fn read_board(json: &str, id_generator: &mut IdGenerator) -> BoardReadResult;
```

**Transcription notes.**
- **The DTO field names are the JSON contract** — transcribe them, do not rename to snake_case unless Java's `@SerializedName` says so. `hostCad`, `hostVersion`, `netClasses`, `containsPlane`, `defaultItemClearanceClasses` and friends are what KiCad writes.
- **`clearanceClassCount = Math.max(2, additionalNetClasses.size() + 2)`** (`:127`) with `clearanceClassNames[0] = "null"` and `[1] = "default"` (`:129-130`) — the two reserved rows. **Quirk #83's J-then-I `ClearanceMatrix.setValue`/`getValue` indexing MUST NOT be "fixed" here** (plan-3 hand-off §Plan 8): writing both orders to "correct" the asymmetry changes clearances on every KiCad-sourced board and breaks parity. A test asserts the asymmetry survives.
- **The outline loop negates Y** (`:397-399`: `new IntPoint(round(pt.x * scale), round(-pt.y * scale))`) and offsets the bounding box by `1000` (`:403`). Use `java_round` (Convention 5), not `f64::round`.
- **The host fallbacks are `"KiCad"` and `"v10.0"`** (`:313-321`) — literal strings that reach `Communication.SpecctraParserInfo` and therefore the SES the port later writes. They are **not** `PARITY_VERSION` (ruling 5).
- **`CoordinateTransform::new(scale_factor, 0, 0)`** (`:313`) — the KiCad reader builds its own transform, and `LoadedBoard` (Task 3) is what carries it to the writer.
- **Section 8's via plumbing constructs a `ViaInfo`, a `ViaRule` and appends** (`:440-450`, `:455-470`). Plan 7 Task 0 made `ViaRule` **own** its `ViaInfo`s (`bde59ef`); the reader appends owned copies, and the `// Java bug:` aliasing note from ruling H does not apply here because the reader never re-declares a `ViaInfo`.
- **`nonDefaultNetClasses` and `findKiCadDefaultNetClass` are Task 9's** (`:926-945`) but section 4 calls them — **Task 8 declares them and Task 9 finishes the two that section 9 also needs**; whichever way it falls, the earliest task that writes the body owns it (Plan 7 scan ruling 7). Task 8's §Interfaces produced must list every helper it declares.
- **`net.setClass(boardRules.netClasses.get(clNo - 1))` with the comment "NetClass array indices are 0-based"** (`:466-467`) — an off-by-one that is *deliberate* in Java. Transcribe it with the comment.

**Tests (`crates/fr-dsn/tests/kicad_reader.rs`):** the `P8T8Probe` table as literals for sections 1–8 — layer structure (names, signal flags, order), the full clearance matrix, the outline's corner list and bounding box, the `Communication` parser info, and every net/net-class/via-rule the reader created; `the_clearance_matrix_stays_asymmetric` (quirk #83); `y_is_negated_and_java_rounded`; `the_host_fallbacks_are_kicad_and_v10`; `an_unknown_unit_falls_through_to_the_documented_arm`.

**JVM-pinned evidence (required).** **`P8T8Probe.java`** (`package app.freerouting.io.kicad;`), HEAD jar, standard flags. **The corpus has no `.json` board fixture** — `find ../freerouting/fixtures -name '*.json'` is the first step, and if it returns nothing the probe **generates** one by round-tripping a corpus DSN through `KiCadJsonWriter.write` (Task 10's Java side, which exists in the jar) and reads it back. The probe prints the whole board state after `readBoard`: layers, clearance matrix, outline, communication, nets, net classes, via rules, and the item count. Transcript committed as `crates/fr-dsn/tests/data/p8t8-kicad-read-a.txt`. **Acceptance: 0 diffs on every printed field, on at least three inputs (a KiCad-exported board if one exists in the corpus, plus two writer-generated ones).**

**Steps:** the `.json` fixture hunt (recorded either way) → the DTO tree → `P8T8Probe.java` → sections 1–8 → consume the four `fr-drc/src/lib.rs` markers that name them → fmt/clippy/test/audit + `batch_parity` + `p6t1` → commit `feat(dsn): the KiCad JSON DTO tree and readBoard's sections 1-8`.

---

### Task 9: `fr_dsn::kicad` part B — `readBoard`'s sections 9–11 (components, padstacks, conduction areas, wiring) and the five private helpers

**Files:** `crates/fr-dsn/src/kicad/reader.rs` (a **fn-body extension** — the signature is Task 8's), `crates/fr-dsn/tests/kicad_reader.rs`; `crates/fr-drc/src/lib.rs:115` (`importSession`) stays for Task 10; `scripts/differential/java/probes/P8T8Probe.java` (extended).
**Java:** `io/kicad/KiCadJsonReader.java` — **`// 9. Load Components & Library templates` `:498-646` (149)** (incl. the pad-shape arms `"oval"` `:515`, `"rect"`/`"rectangle"`, the fall-through `:526`, and the per-component `catch (Exception e)` at `:603`), **`// 10. Load conduction areas (copper pours)` `:647-665` (19)**, **`// 11. Load traces and vias (existing wiring)` `:666-745` (80)**, the method-level `catch (Throwable e)` `:746-755` (10); `getDescriptivePadstackName` `:857-891` (35), `arePackagePinsIdentical` `:892-925` (34), `findKiCadDefaultNetClass` `:926-935` (10), `nonDefaultNetClasses` `:936-946` (11), `applyKiCadNetClassParameters` `:947-963` (17), `resolveNetClassIndex` `:964-981` (18), the nested outline accumulator `:982-995` (`addPoint` `:985-989`, `boundingBox` `:990-994`). **≈ 383 ported Java lines.**

**Interfaces consumed:** everything Task 8 consumes, plus `fr_board::{Package, Padstack, PolylineTrace, Via, ConductionArea, ComponentOutline}` (Plan 2); `fr_geometry::{Polyline, Line}` (Plan 1, **Convention 7's identity-token contract applies**).
**Interfaces produced:** none new — `read_board` becomes complete, and the six helpers listed above.

**Transcription notes.**
- **Two recovery boundaries exist in Java on this path and both must be reproduced, not invented.** The per-component `catch (Exception e)` at `:603` **skips one component and continues**; the method-level `catch (Throwable e)` at `:746` returns a `BoardReadResult.ParseError`. **`Throwable` catches a `StackOverflowError`**, unlike every `catch (Exception)` elsewhere in this port (quirk #27's family) — say so at the site, because it is the one place in the whole port where Java recovers from one. The port's answer is `Result` at the method boundary and a per-component `continue`; **no `catch_unwind`** unless the implementer finds a panic that Java's `Exception` arm would have caught, in which case it is a reported finding.
- **Convention 7 at its site:** section 11 builds `Polyline`s from corner lists. Every construction picks `new_polyline` vs `new_polyline_in_place` **deliberately**, with the Java line that decides it in a comment — `fr-dsn`'s DSN wiring reader is the precedent to copy.
- **`getDescriptivePadstackName` is a name-generating function whose output reaches the SES** (via the padstack names the writer emits). Transcribe its string building exactly, including the layer-name comparisons at `:877-884`.
- **`arePackagePinsIdentical` decides package reuse** (`:892-925`), so getting it wrong duplicates or merges library packages — visible in the item count and in the SES. It is 34 lines of pairwise comparison; transcribe, do not paraphrase.
- **`resolveNetClassIndex` (`:964-981`) and section 8's `clNo - 1`** interact; Task 8 owns the latter and Task 9 the former. State the relationship in a comment on both.
- **Section 11's vias go through the checked insert** — Task 3 already made `crates/fr-dsn/src/parser/wiring.rs:596` pass the `normalize_time_limit`-backed check for the DSN path; the KiCad path must use the same seam, not the unchecked wrapper.

**Tests:** the extended `P8T8Probe` table — every component, package, padstack (with its generated name), pin, conduction area, trace polyline and via, in Java's iteration order; `a_bad_component_is_skipped_and_the_rest_load` (`:603`); `a_malformed_document_answers_parse_error` (`:746`); `identical_packages_are_reused` (`:892-925`); `the_generated_padstack_names_match_the_jar`.

**JVM-pinned evidence (required).** `P8T8Probe.java` extended to dump the full item graph. Transcript committed as `crates/fr-dsn/tests/data/p8t8-kicad-read-b.txt`. **Acceptance: 0 diffs on every item, on the same three-or-more inputs, PLUS a round trip: `-de board.json -do out.ses` against the jar produces byte-identical SES** — which is `p8t7`'s first rung and the reason `--kicad-json`'s loader gap in `docs/cli-legacy-flags.md` closes here.

**Steps:** `P8T8Probe.java` extended → sections 9–11 → the six helpers → the two Java recovery boundaries → the `-de board.json -do out.ses` round trip → fmt/clippy/test/audit + `batch_parity` + `p6t1` → commit `feat(dsn): the KiCad JSON reader completed — components, wiring and the helpers`.

---

### Task 10: `fr_dsn::kicad` — `importSession`, `KiCadJsonWriter.write`, and quirk T's output path

**Files:** `crates/fr-dsn/src/kicad/{reader,writer}.rs`, `crates/fr-dsn/tests/kicad_writer.rs`; `crates/fr-drc/src/lib.rs:115,118` (the last two `added in Plan 8:` markers — **consumed**); `crates/freerouting/src/commands/{route,drc}.rs` (the two `obligation:` stubs Tasks 6 and 7 left — **discharged**); `scripts/differential/java/probes/P8T10Probe.java`.
**Java:** `io/kicad/KiCadJsonReader.java` — **`importSession` `:757-856` (100)**, the `.json` branch of `Freerouting.initializeDrc:301-307` that calls it (**`new FileReader(f)` = PLATFORM DEFAULT CHARSET**, quirk label U); `io/kicad/KiCadJsonWriter.java (227)` — `write(RoutingBoard)` `:27-31`, **`write(RoutingBoard, String designName)` `:32-226` (195)**; `management/jobs/RoutingJobSchedulerActionThread.java:277-278` (the `-do out.json` call site) and **`setJobOutput` `:259-295`** re-read for quirk label T. **≈ 330 ported Java lines.**

**Interfaces consumed:** `fr_dsn::kicad::{read_board, the DTO tree}` (Tasks 8, 9); `fr_board::Board`; `fr_core::{BoardFileDetails::set_data, FileFormat::sniff_bytes}` (Task 1 — quirk T lives in their interaction).
**Interfaces produced:**
```rust
// crates/fr-dsn/src/kicad/reader.rs
/// `KiCadJsonReader.importSession` (:757-856) — the `.json` arm of the DRC session slot
/// (Freerouting.java:296-329). **Quirk label U: Java opens it with `new FileReader(f)`, i.e. the
/// PLATFORM DEFAULT CHARSET**, while every other JSON path in the tree is explicit UTF-8.
/// The port reads UTF-8 and the quirk row records the divergence as **unobservable on an ASCII
/// document and observable on a non-ASCII one**; the driver's fixture set includes one.
pub fn import_session(json: &str, board: &mut fr_board::Board) -> Result<(), DsnError>;
// crates/fr-dsn/src/kicad/writer.rs
/// `KiCadJsonWriter.write` (:32-226) — UTF-8, the KiCad session JSON.
pub fn write(board: &fr_board::Board, design_name: &str) -> String;
```

**Transcription notes.**
- **Quirk label T is this task's headline and it must be MEASURED, not assumed (plan ruling 9).** `setJobOutput` sets `format = KICAD_SESSION_JSON` (`:266`), then `setData` re-sniffs the bytes (`BoardFileDetails.java:113`) and — because JSON starts with `{` — resets it to `KICAD_DESIGN_JSON` (`RoutingJob.java:160-161`). On the **second** call (`:168`, after `pipeline.run()`) **neither branch matches**, so the file keeps whatever the last mid-run board-updated event produced. The SES path escapes this because `(ses` re-detects as `SES`. **The port writes once (ruling 9), so on this path it would write the FINAL board where the jar writes an intermediate one.** The steps below say how the answer is obtained: run the jar with `-do out.json` on a board that changes during the run, diff against the jar's own `-do out.ses` converted, and record which board the jar actually wrote. Then either reproduce it (by writing at the same event the jar last fired) or `// totalized:` it with the measurement pasted in. **Do not guess.**
- **Quirk label U's charset.** The port reads UTF-8; the driver includes a fixture with a non-ASCII component name so the divergence is *measured* rather than asserted to be harmless.
- **The writer is 195 lines of JSON assembly and its key order is the DTO declaration order** (Convention 8). Gson is the serialiser on the Java side, so the same rule applies as for the manifest.
- **`-do out.json` is accepted by `tryToSetOutputFile:384-388`** and, unlike `.dsn`/`.scr`, **is** serialised — so it exits 0. Task 6's quirk-L test set gets a `.json` row here.

**Tests (`crates/fr-dsn/tests/kicad_writer.rs`):** the `P8T10Probe` table as literals — the writer's output for three boards, byte for byte; `write_then_read_is_a_fixed_point` (round trip through `read_board`); `import_session_inserts_the_wires_and_vias`; `a_non_ascii_component_name_survives_utf8` (quirk U); and **the quirk-T decision test**, whose name is chosen once the measurement is in and whose doc comment carries the measurement.

**JVM-pinned evidence (required).** **`P8T10Probe.java`** (`package app.freerouting.io.kicad;`), HEAD jar, standard flags: dumps `write`'s exact output for three boards and the board state after `importSession` on two session documents. Transcript `crates/fr-dsn/tests/data/p8t10-kicad-writer.txt`. Plus the **quirk-T measurement**, run as part of `p8t7`: `run_jar(["-de", dsn, "-do", "out.json"])` on a board that routes in more than one pass, with the resulting document compared against both the initial and the final board. **Acceptance: 0 diffs on the writer output and the session import; the quirk-T row carries its measurement and the port's recorded answer.**

**Steps:** `P8T10Probe.java` → the writer → `import_session` → discharge Tasks 6 and 7's two stubs → the quirk-T measurement → the recorded decision + test → consume the last two `fr-drc` markers → fmt/clippy/test/audit + `batch_parity` + `p6t1` → commit `feat(dsn): the KiCad JSON writer and session import, and quirk T's measured answer`.

---

### Task 11: MCP — the concurrent transport (reader thread, guarded writer, progress, cancellation)

**Files:** `crates/freerouting/src/mcp/{jsonrpc,server,stdio}.rs`, `crates/freerouting/tests/mcp_stdio.rs`, `crates/freerouting/Cargo.toml`.
**Java:** **not a port.** `api/mcp/McpControllerV1.java (655)` is read for the protocol shape only — the four methods `:189-198` (`initialize` `:190`, `notifications/initialized` `:191-194`, `tools/list` `:195`, `tools/call` `:196`, everything else `-32601` at `:197`), the `initialize` result `:277-287` (`protocolVersion` `"2024-11-05"` `:285`, `capabilities {"tools":{}}` `:277-278`, `serverInfo` `:281-282` **plus** the non-spec top-level `serverName`/`serverVersion` `:286-287`), the `tools/call` result shape `:333-356` (**one text block, `isError = status >= 400`, no `structuredContent`**) and the blocking `HttpClient.send` with no timeout `:359-382`; `Freerouting.startMcpStdioBridge :681-788` is **rostered `// renamed:`** in Task 0. **0 ported Java lines; ≈ 400 new.**

**Interfaces consumed:** `fr_core::CancelToken` (Task 0); `ExitCode` (Task 5).
**Interfaces produced:**
```rust
// crates/freerouting/src/mcp/server.rs — the ToolHandler signature CHANGES (the in-tree obligation)
/// **Closes the obligation at `mcp/server.rs:9-32`.** A handler now receives the server state, the
/// request's arguments, a progress writer bound to the request's `progressToken`, and the
/// request's `CancelToken`. It may emit any number of `notifications/progress` before returning.
pub type ToolHandler = Box<dyn Fn(&State, Value, &ProgressWriter, &CancelToken)
    -> Result<Value, RpcError> + Send + Sync>;
/// Writes `notifications/progress` through the shared, `Mutex`-guarded stdout writer. A request
/// with no `_meta.progressToken` gets a writer that drops everything, so a tool never branches.
pub struct ProgressWriter { /* … */ }
/// **Closes the obligation at `mcp/stdio.rs:18-28`.** Reader thread -> channel; the main loop owns
/// `Arc<Mutex<W>>` and dispatches; a `tools/call` runs on its own thread with a `CancelToken` the
/// main loop keeps, so an inbound `notifications/cancelled` flips it **while** the tool runs.
pub fn run_with<R: BufRead + Send + 'static, W: Write + Send + 'static>(
    state: State, reader: R, writer: W) -> i32;
```

**Transcription notes.**
- **Ruling 3's shape, exactly:** one reader thread (`std::thread::spawn`, `mpsc::Sender<String>`), the main thread dispatching, and **one tool thread per in-flight `tools/call`**. The writer is `Arc<Mutex<W>>`, shared by the main thread and every tool thread. **Nothing below `crates/freerouting` sees a thread.**
- **Ruling 4's boundary:** `catch_unwind` at exactly one site, around the handler invocation, rendering the panic payload into `RpcError::internal`. A test asserts the server answers the **next** request after a panicking tool.
- **Cancellation (ruling AP + ruling 2):** `notifications/cancelled` carries a `requestId`; the main loop looks it up in an in-flight map and calls `CancelToken::cancel()`. The **three**-state distinction is preserved end to end — the MCP only ever issues `cancel()` (`ALL`), and the token's `cancel_auto_router()` arm exists for the `--max-passes` path; a comment says so, so a reader does not delete the unused arm.
- **`protocolVersion` stays `"2025-06-18"`** (ruling AT — the port is a new server, not a re-implementation of Java's `"2024-11-05"`), `serverInfo` is `{name: "freerouting", version: SERVER_VERSION}` and the **non-spec `serverName`/`serverVersion` top-level keys Java adds (`:286-287`) are NOT reproduced** — a documented delta in `p8t6`. `capabilities` is `{"tools": {"listChanged": false}}` (Java's is `{"tools": {}}`) — a second documented delta.
- **`structuredContent` is emitted** (the skeleton already does, `server.rs:104-121`); Java emits only a stringified text block. Third documented delta.
- **Stdout is reserved for the protocol.** Java achieves it with `System.setOut(System.err)` (`:918-920`); the port's `main.rs` already writes `tracing` to stderr unconditionally, which is stronger. **One JSON object per line, no Content-Length framing** — kept from Java.
- **`ping` is added** (spec §13); Java answers `-32601` for it. Fourth documented delta.
- **Quirk label M is not reproduced:** Java's bridge strips every `\r` and `\n` from the response body before printing (`:766`), which silently corrupts a non-compact document. The port emits compact JSON with a single trailing newline. Fifth documented delta, and a `// Java bug:` row.
- **EOF exits 0, a stdin `IOException` exits 1** (`:777-782`) — that much *is* Java's, reproduced.

**Tests (`crates/freerouting/tests/mcp_stdio.rs`):** the six existing skeleton tests **stay green**; plus `a_long_running_tool_reports_progress` (a fake tool that emits five progress notifications, asserted in order and interleaved correctly with the response); `a_cancelled_tool_stops_mid_flight` (the cancellation arrives while the tool is running and the tool observes it); `a_panicking_tool_becomes_an_error_and_the_server_survives` (ruling 4); `stdout_carries_only_protocol_lines`; `eof_exits_zero`; `a_notification_gets_no_response`.

**Evidence.** New code, so **contract tests plus the documented-delta driver** (ruling General). The five deltas above are written into `crates/freerouting/README.md` **in this task**, each with the Java file:line beside it, so `p8t6` (Task 12) only has to assert they are still the deltas.

**Steps:** the handler signature → `ProgressWriter` → the reader thread + guarded writer → the in-flight map + cancellation → ruling 4's boundary → rewrite the two obligation blocks to record what landed → the delta table in the README → fmt/clippy/test → commit `feat(mcp): the concurrent stdio transport — progress, cancellation and a tool boundary`.

---

### Task 12: MCP — the four tools, the hand-written schemas, the sparse settings tier, and `freerouting info`

**Files:** `crates/freerouting/src/mcp/tools/{mod,route_board,check_drc,board_info,list_settings,schema}.rs`, `crates/freerouting/src/commands/info.rs`, `crates/fr-core/src/summary.rs`, `crates/freerouting/tests/mcp_stdio.rs`, `crates/fr-core/tests/summary.rs`; `crates/fr-settings/src/resolve.rs:184` (the `obligation:` — **closed**); `scripts/differential/java/P8T6.java`, `scripts/differential/rust/src/bin/p8t6.rs`, `scripts/differential/java/P8T7.java`, `scripts/differential/rust/src/bin/p8t7.rs`.
**Java:** **not a port.** `api/dto/BoardFilePayload.java` is read for the **field names** ruling AO keeps (`job_id`, `data` (Base64), and the inherited `size`, `crc32`, `format`, `statistics`, `filename`, `path`); `api/mcp/OpenApiMcpToolRegistry.java (659)` is rostered (Task 0) and read only to confirm the delta list; `api/v1/JobOutputResource.getDrcReport :589-661` is noted as making the **same** `fr-drc` call `check_drc` does. **0 ported Java lines; ≈ 480 new** (four tools ~300, the four schemas ~80, the summary ~100).

**Interfaces consumed:** `mcp::server::{State, ToolHandler, ProgressWriter}` (Task 11); `commands::route::run` / `commands::drc::run`'s underlying `fr-core` calls (Tasks 6, 7 — the tools call **`fr-core`**, not the CLI functions, so a tool never spawns a process); `fr_settings::{SettingsMerger, SettingsSource, priority::API, json}` (Plan 4); `fr_core::{stats_json, load::*, save::*, BoardStatistics::from_bytes}` (Tasks 2, 3).
**Interfaces produced:**
```rust
// crates/fr-core/src/summary.rs
/// The board summary shared by `freerouting info` (spec §12) and the `board_info` MCP tool
/// (spec §13). **New — Java has no `info` mode**; the closest thing is
/// `BoardStatistics.toString()` (core/scoring/BoardStatistics.java:589-591), whose JSON this
/// embeds under `statistics` so the two surfaces cannot drift. Stable key order (Convention 8).
pub struct BoardSummary { pub layers: …, pub nets: …, pub components: …,
                          pub statistics: serde_json::Value, pub metadata: … }
pub fn summarise(board: &Board, metadata: &BoardMetadata) -> BoardSummary;
// crates/freerouting/src/mcp/tools/schema.rs
/// The four tool input schemas as hand-written `serde_json::json!` literals.
/// **`schemars` is refused** (controller ruling AO/General): adding it would mean deriving
/// `JsonSchema` on `RouterSettings` (Plan 4 ruling 3 deliberately did not), which is a
/// workspace-wide derive sweep on the last plan. The snapshot test below is what stops drift.
pub fn route_board_schema() -> serde_json::Value;  /* … three more … */
```

**The four tools, per spec §13 with flat arguments (ruling AO).**

| tool | arguments | result |
|---|---|---|
| `route_board` | `dsn_path` \| `dsn_text`, `ses_path?`, `rules_path?`, `output_path?`, `settings?` | `{ ses_path \| ses_text, stats, incompletes, drc_violation_count, timed_out }` — plus `size`, `crc32`, `format`, `filename`, `path` under the `BoardFilePayload` names where they overlap |
| `check_drc` | `dsn_path` \| `dsn_text`, `ses_path?`, `rules_path?` | the KiCad DRC report (the same document `-drc` writes, `DrcJsonFlavor::KiCad`) |
| `board_info` | `dsn_path` \| `dsn_text` | `BoardSummary` |
| `list_settings` | `{}` | the `RouterSettings` schema with defaults and descriptions |

**Transcription notes.**
- **Ruling AU's sparse tier is the one hard part.** `route_board { settings? }` is **exactly** the priority-70 `ApiSettings` payload that `resolve_headless`' premise does not cover: an API job never runs merge #1, so the payload is *sparse* and merge #2's own `0..60` chain does the work. **Compose it from `SettingsMerger` directly**, per the `obligation:` at `crates/fr-settings/src/resolve.rs:184`, and **close that obligation here**. Note that Task 7's DRC quality score is a *second* direct composer (quirk label C); the two are different compositions and neither is `resolve_headless`.
- **Quirk #141 governs the `settings` object's reader**: Gson `Strictness.LENIENT` on the textual pass, strict value conversion, non-finite floats refused both ways. `fr_settings::json` reproduces it and `json.rs:86` names this reader — use it, do not `serde_json::from_value`.
- **`route_board` reports progress** through Task 11's `ProgressWriter` and observes the `CancelToken`; `check_drc` and `board_info` are fast and report none. **Java's MCP has no progress and no cancellation at all** (`McpControllerV1.java:189-198`; the agent card admits `"streamingToolCalls": false` at `AgentCardController.java:128`) — deltas six and seven.
- **File-path variants write to disk and return paths** (spec §13), keeping large SES bodies out of the model context unless `ses_text` is requested. `data` (Base64) is offered only when the caller asks for text, and the field is named as `BoardFilePayload` names it.
- **`check_drc` exposes no coordinate-unit flag** (Task 7's recorded decision) — `"mm"`, as `Freerouting.java:336` hard-codes.
- **`freerouting info` writes `BoardSummary` to stdout as JSON** (spec §12) and exits 0; it is the only CLI surface with no Java counterpart, and the README says so.
- **The schema snapshot test is the anti-drift device.** A test serialises each schema and compares against a committed golden; a second test walks `RouterSettings`' field list and asserts every field named in `list_settings`' schema exists (and vice versa), so a settings field added later cannot silently vanish from the schema. **That pair is what `schemars` was going to buy**; the ruling's "revisit only if that proves error-prone" is discharged by writing it.

**Tests:** `crates/freerouting/tests/mcp_stdio.rs` gains an **end-to-end conversation over spawned pipes**: `initialize` → `notifications/initialized` → `tools/list` (four tools, names and schemas) → `tools/call route_board` on `examples/tutorial_board/tutorial_board.dsn` → assert the SES **equals `p8t1`'s byte for byte** → `tools/call check_drc` → assert the report equals `p8t3`'s → `tools/call board_info` → `tools/call list_settings` → the schema snapshot. Plus `a_sparse_settings_payload_composes_at_priority_70`; `a_non_finite_float_in_settings_is_refused` (quirk #141); `cancelling_route_board_mid_run_returns_timed_out_false_and_a_partial_result`.

**JVM-pinned evidence (required).**
- **`p8t6` — the documented-delta driver (ruling AO).** `P8T6.java` drives the **jar's** stdio mode (`--mcp_server.stdio=true`) through `initialize`, `tools/list` and one `tools/call`, capturing every line; `p8t6.rs` drives the port's `mcp` subcommand with the same script. The driver **asserts the deltas are exactly the recorded list** — protocol version, `serverName`/`serverVersion`, `capabilities`, `ping`, `structuredContent`, the `\r`/`\n` stripping, the tool set (29 OpenAPI-derived vs 4 flat), the input shape (`{path, query, body}` vs flat), progress and cancellation. **A NEW delta is a failure**; a delta that disappears is also a failure (the table is the contract). Java's stdio mode needs its Jetty port up, a `Freerouting-Profile-ID` header and the internal bridge token (`Freerouting.java:704-762`) — **the driver header states exactly how it starts the jar**, and if the jar's stdio mode cannot be driven headlessly the driver records that as a `SKIP` with the reason, which is itself the delta.
- **`p8t7` — the KiCad end-to-end acceptance of spec §1.** KiCad-exported DSN → `route` → SES → **re-read by `fr_dsn::ses_reader::read`** without error, and byte-identical to the jar's. Plus Task 9's `-de board.json -do out.ses` rung and Task 10's quirk-T measurement.
**Acceptance: `p8t6` reproduces the recorded delta table exactly; `p8t7` MATCH.**

**Steps:** the four schemas + the anti-drift pair → `BoardSummary` → `board_info` + `info` → `list_settings` → `check_drc` → `route_board` (with the sparse composer, closing `resolve.rs:184`) → the end-to-end conversation → `p8t6` → `p8t7` → fmt/clippy/test/audit → commit `feat(mcp): the four tools, the hand-written schemas and freerouting info`.

---

### Task 13: audits to zero, the roster closed, the READMEs, the quirk register closed, and the PROJECT hand-off

**Files:** `crates/fr-core/{src/lib.rs,README.md}`, `crates/freerouting/README.md`, `scripts/audit-map/{fr-core.map,freerouting.map,fr-dsn.map,fr-board.map,fr-router.map}`, `docs/java-quirks.md`, `docs/cli-legacy-flags.md`, **`docs/plan-8-handoff.md`**, and the obligation ticks in `docs/plan-{1,2,3,4,5,6,7}-handoff.md`.

**The audit — twelve invocations, all exiting 0 with no `MISSING` and no `UNMAPPED`.** Three of these directories (`management/`, `api/`, `core/`) have **never been audited by any plan**, which is why the survey's §5.7 gap existed:
```
scripts/audit-port.sh core                crates/fr-core/src '*.java' scripts/audit-map/fr-core.map
scripts/audit-port.sh core/scoring        crates/fr-core/src '*.java' scripts/audit-map/fr-core.map
scripts/audit-port.sh core/results        crates/fr-core/src '*.java' scripts/audit-map/fr-core.map
scripts/audit-port.sh core/events         crates/fr-core/src '*.java' scripts/audit-map/fr-core.map
scripts/audit-port.sh management          crates/fr-core/src '*.java' scripts/audit-map/fr-core.map
scripts/audit-port.sh management/jobs     crates/fr-core/src '*.java' scripts/audit-map/fr-core.map
scripts/audit-port.sh management/sessions crates/fr-core/src '*.java' scripts/audit-map/fr-core.map
scripts/audit-port.sh io/kicad            crates/fr-dsn/src  '*.java' scripts/audit-map/fr-dsn.map
scripts/audit-port.sh api                 crates/fr-core/src '*.java' scripts/audit-map/fr-core.map
scripts/audit-port.sh api/mcp             crates/freerouting/src '*.java' scripts/audit-map/freerouting.map
scripts/audit-port.sh logger              crates/freerouting/src '*.java' scripts/audit-map/freerouting.map
scripts/audit-port.sh .                   crates/freerouting/src 'Freerouting.java' scripts/audit-map/freerouting.map
```
`api/**` and `api/mcp/**` will report **`ROSTERED`** for nearly every class — that is the correct outcome (ruling AU/AO) and it is what makes 8 425 consciously-dropped lines *visible* instead of silent. **Record the `ROSTERED` count in the commit message.** If the last invocation's `.` subpath does not work with the script's argument shape, **read `scripts/audit-port.sh` and use the shape it actually accepts** — do not weaken the check to make it pass.

**The marker gate — three greps, and one of them is new to this plan.**
```
# (1) the code gate — must return NOTHING
grep -rn "added in Plan 8" crates/*/src
# (2) the test gate — must return NOTHING
grep -rn "added in Plan 8" crates/*/tests
# (3) THE FINAL GATE — there is no Plan 9. Must return NOTHING, anywhere:
grep -rn "added in Plan 9\|added in Plan [0-9][0-9]" crates/ docs/ scripts/
```
Every one of the **twenty-three** `added in Plan 8:` sites listed at plan time (`crates/fr-board/src/board/mod.rs:57`; `crates/fr-dsn/src/lib.rs:12`; `crates/fr-settings/src/lib.rs:193`; `crates/fr-settings/src/sources/cli.rs:582`; `crates/fr-drc/src/lib.rs:114-119` and `:144`; `crates/fr-router/src/lib.rs:243-244`; `crates/fr-router/src/score/mod.rs:51-58`; plus the prose mentions in `crates/fr-dsn/README.md:56` and `crates/fr-router/README.md:2051,2053`) is **consumed** — ported, re-pointed to `// not ported:` / `// not reachable:` / `// renamed:` **with its reason**, or reworded to the past tense in prose. **Re-run the grep at port time**: Plan 7 will have added more. Also: **`grep -rn "EXIT_NOT_IMPLEMENTED" crates/` returns nothing** (Task 5's ruling AR reservation is discharged by Task 12 landing the last subcommand), and **`grep -rn "obligation:" crates/` is down by the five this plan closes** (`resolve.rs:184`, `fr-drc/src/report/json.rs:57`, `fr-dsn/src/parser/wiring.rs:596`, `mcp/server.rs:9-32`, `mcp/stdio.rs:18-28`) with every survivor listed in the hand-off as a **closed-with-reason** row, not an open one — **there is no later plan to inherit an open obligation.**

**`docs/java-quirks.md`.** The register is contiguous through **#196** at plan time (Plan 7 Task 1 landed #194–#196; §Process notes names **#197** as the next free id) — **and Plan 7 will land more before Plan 8 starts, so the first Plan 8 task to write a row reads the register's last row and takes the next id.** The table below gives **labels, not ids**; ids are allocated **contiguously, in the order rows are written**, and **every task that lands a row appends one line to the amendment log below mapping its plan label to the id it actually took**.

| plan label | what | Java site | task |
|---|---|---|---|
| **A** | The `-drc` stdout branch is unreachable: DRC mode is entered only when `drcReportFile != null` (`main:1462`) and `drcJob.drc` is that same object. A **bare `-drc` is not DRC mode at all** — it sets two dead booleans and falls through to `initializeCli`, which dies with "Both an input file and an output file must be specified". | `Freerouting.java:368-371`, `:1462`; `GlobalSettings.java:660-669` | 5, 7 (ruling 6) |
| **B** | `initializeDrc` returns `true` unconditionally, so `-drc` exits 0 whatever happens: a missing `.rules`, a missing session and a failed quality score all only warn, and the violation count never affects the code. | `Freerouting.java:289, 324, 350, 373` | 7 |
| **C** | The DRC quality score uses a **different settings merge** from the router's — merge #1 with only `DsnFileSettings`, no `.rules`, no board pass, no merge #2 — so `-dr x.rules -drc r.json` scores with weights the rules file never influenced while the violations were computed against the clearances it installed. | `Freerouting.java:342-352` vs `:277-294` | 7 |
| **D** | The DRC session import happens **after** the `.rules` re-parse, so the session's wires and vias are created and then checked against the clearances the rules file installed. Swapping the two changes the violation list. | `Freerouting.java:277-294` then `:296-329` | 7 |
| **E** | `RoutingResultManifest.phases.fanout` and `.optimizer` are allocated and never written — always `{}` — while `phases.autorouter.duration_seconds` is the **whole job's** duration. `isFanoutTimedOut()`/`isTimedOut()` are three lines away and reach only a log line. | `RoutingResultManifest.java:79, 85, 124-132` | 4 |
| **F** | `BoardStatistics(byte[], DSN)` counts **substrings**: `(layer` also counts `(layer_rule`, `(net` also counts `(network`/`(net_class`, `(via` also counts `(via_rule`, `(class` also counts `(class_class`. The SES branch splits on `"(path "` and takes `words[0]` of each chunk after the first. | `BoardStatistics.java:469-473, 514-519, 578-586` | 2 |
| **G** | The DSN host scrape almost always fails: `searchLimit` is the first `)` after `(parser`, which in a real DSN ends the first inner clause, so `(hostCad`/`(hostVersion` lie outside the scope. Compounded by HEAD's camelCase keywords and by `substring(hcIdx+9,…)`/`(hvIdx+13,…)` hard-coding one space. | `BoardStatistics.java:480-503` | 2 |
| **H** | `initializeCli` prints a donation banner **to stdout** when the output was written, the persisted `statistics.jobsCompleted >= 5` and the e-mail is empty. Un-suppressible, and it would corrupt any stdout-JSON mode. **Not reproduced**; the log normaliser strips it. | `Freerouting.java:164-183` | 6 |
| **I** | The output file is **deleted before routing starts** and only warns on failure, so a failed or hung run destroys the previous result and writes nothing. | `Freerouting.java:116-121` | 6 |
| **J** | Two nested polling loops (`initializeCli` 500 ms + the scheduler 250 ms) add ~0.75 s to every single-shot run and **quantise the manifest's `duration_seconds`**. **Not reproduced** (ruling 8); the manifest's timing fields are normalised out. | `Freerouting.java:151-158`; `RoutingJobScheduler.java:268` | 6 |
| **K** | The SES write-out runs on **every** board-updated event and again at the end, each time re-serialising the board, recomputing CRC32 and running a full text-scrape `BoardStatistics`. **Written once** (ruling 9); identical bytes on the SES path, not on the JSON one (label T). | `RoutingJobSchedulerActionThread.java:100, 168`; `BoardFileDetails.java:107-116` | 6, 10 |
| **L** | `tryToSetOutputFile`'s return value is **discarded** (`Freerouting.java:123`): `-do out.txt` returns false, `job.output` keeps the input-derived `<input>.ses` details, and SES bytes go to `out.txt` anyway. `-do out.dsn`/`.scr` is accepted but serialised by neither branch, so a **0-byte file is written and the run exits 1**. | `RoutingJob.java:377-397`; `RoutingJobSchedulerActionThread.java:275-293`; `Freerouting.java:196-213` | 1, 6 |
| **M** | The MCP stdio bridge strips every `\r` and `\n` from the response body before printing — safe for compact JSON, silently corrupting otherwise. **Not reproduced**; documented delta. | `Freerouting.java:766` | 11 |
| **N** | `isCliTerminalState` omits `INVALID`, and the scheduler assigns `INVALID` for a null or non-DSN/JSON input, so `-de x.ses -do y.ses` makes the CLI **spin forever**. **Totalised** (ruling 7): the port exits 1. | `Freerouting.java:189-194`; `RoutingJobScheduler.java:83, 253` | 1, 6 |
| **O** | `getFileFormat(byte[])`'s leading-newline shift loop never refills `buffer[5]`, so a file starting with ≥ 6 CR/LF bytes spins forever. The comment says `0x0A or 0x13`; the code tests `0x0A`/`0x0D`. **Totalised**, bound named at the site. | `RoutingJob.java:181-187` | 1 |
| **P** | `tryToSetOutputFile` registers the *output* listener as `fireInputUpdatedEvent()` — a copy-paste bug; `setInputFromFile:440,446,452` gets it right. The port has no events, so `// not reachable:`. | `RoutingJob.java:390` | 1 |
| **Q** | `changeFileExtension` NPEs on a bare filename (`filePath.getParent().toAbsolutePath()`), and when the extension already matches it returns the **original, possibly relative** string rather than the reconstructed absolute path — asymmetric with its two other returns. **Totalised** for the NPE, reproduced for the asymmetry. | `RoutingJob.java:352-374` | 1 |
| **R** | `BoardFileDetails.setFilename`'s Windows-only string surgery runs **unconditionally**, and `replaceAll("\\\\.$","")` is a regex bug — `\\.` is "backslash then any char", so it strips the last two characters of any path ending in backslash-plus-anything. `filename.contains(File.separator)` is platform-dependent. | `BoardFileDetails.java:158-166` | 1 |
| **S** | `BoardLoader` accepts only DSN and KiCad design JSON, so `-de prev.ses -drc r.json` fails with "only DSN and JSON formats are supported" rather than at the argument. | `BoardLoader.java:32-37` | 3, 7 |
| **T** | The KiCad JSON output path **never writes the final board**: `setJobOutput` sets `KICAD_SESSION_JSON`, `setData` re-sniffs and resets it to `KICAD_DESIGN_JSON`, and on the second call neither branch matches — so the file is whatever the last mid-run event produced. The SES path escapes it because `(ses` re-detects as `SES`. **Measured, then reproduced or totalised** (ruling 9). | `RoutingJobSchedulerActionThread.java:259-295`; `BoardFileDetails.java:113`; `RoutingJob.java:160-161` | 10 |
| **U** | `new FileReader(sessionFile)` uses the **platform default charset** for a JSON input, while every other JSON path is explicit UTF-8. Port reads UTF-8; divergence measured on a non-ASCII fixture. | `Freerouting.java:304`; `RoutingJobScheduler.java:202` | 10 |
| **V** | `-dr` with a **non-existent** path silently disables adjacent-`<design>.rules` discovery — the scheduler's `else if` chain takes the `initialRulesFile != null` branch and the inner `exists()` then fails. | `RoutingJobScheduler.java:132-152` | 6 |
| **W** | `sha256Hex` returns `null` on failure and Gson omits the key, so an unreadable fixture is indistinguishable from a missing field. | `RoutingResultManifest.java:163-171` | 4 |
| **X** | `BoardScoreBreakdown` and the live `calculateScore` disagree by the DSN resolution factor: `of` reads `traces.totalLength` (raw board units) while its own javadoc says millimetres and `calculateScore:608-609` prefers `totalLengthMm`. Both classes are dead in `main/`. **Rostered** (ruling AS). | `BoardScoreBreakdown.java:140` vs `BoardStatistics.java:608-609` | 0 |
| **Y** | The timeout monitor thread never exits — `while ((job != null) && (job.thread != null))` and `job.thread` is never nulled — so it samples MXBeans for the process lifetime; its `catch (Throwable t) {}` is completely silent. **Rostered**; the port has no monitor thread. | `RoutingJobSchedulerActionThread.java:58, 253-256` | 0 |
| **Z** | `Session`'s constructor mutates before it validates (assigns `this.host` at `:34`, checks `split("/").length != 2` at `:37`), throwing from a partially-constructed object; `SessionManager.setPrimarySession` does the same. Port validates first; **unobservable**, and the row says so. | `Session.java:30-42`; `SessionManager.java:155-170` | 1 |
| **AA** | `enqueueJob` reads `session.userId` purely to validate it and **never uses it** — the one line that makes `SessionManager` a hard dependency of the CLI. | `RoutingJobScheduler.java:315-318` | 0 |
| **AB** | `RoutingJobScheduler` reads its `LinkedList` **outside the lock** at `:57` and `:74-78` while other methods mutate it under `synchronized (jobs)` — hence the defensive `removeIf(Objects::isNull)` at `:62`. The whole scheduler starts as a side effect of static field initialisation. | `RoutingJobScheduler.java:43, 57, 62, 74-78` | 0 |
| **AC** | `applyCopperToEdgeClearanceOverride`'s early return makes the same numeric setting behave differently by **provenance**: when the value equals `DEFAULT_COPPER_TO_EDGE_CLEARANCE_UM` (500.0) **and** the outline does not use the fallback AREA class, the override is skipped — so a *defaulted* 500 and an *explicitly requested* 500 diverge. | `HeadlessBoardManager.java:501-507`; `DefaultSettings.java:78` | 3 |
| **AD** | The two clearance overrides run **twice** on a DSN load — once from `createBoard` (invoked by the parser at `Structure.java:1268`) and again from `applyRouterSettingsForLoadedBoard:746-747` — and each can re-trigger `reinsertTreeItems()`. | `HeadlessBoardManager.java:342-343` vs `:746-747` | 3 |
| **AE** | The deferred post-load virtual thread **races the router**: `BoardStatistics(loadedBoard, …)` and a full DSN re-serialisation read the board while the router mutates it, and `compareCounterpartBoardIfPresent` loads a **second whole board from disk** to log a diff. **Rostered.** | `HeadlessBoardManager.java:759-784, 172-237` | 0 |
| **AF** | `main` calls `UIManager.setLookAndFeel(getSystemLookAndFeelClassName())` on **every headless run** and sleeps **1 s unconditionally** after starting analytics, including for `-drc`. **Not reproduced**; the reason the port is measurably faster on trivial boards. | `Freerouting.java:1110-1117, 1363-1367` | 5 |
| **AG** | The CLI parser **warns but never fails**: unknown flags warn, a missing argument is silently ignored and `i` is not advanced, a parse exception is caught and the loop continues. Every short flag except `-l` is matched with `startsWith`, so `-decoy`/`-diff`/`-drcx` are accepted as `-de`/`-di`/`-drc`, and `-mp -5` is impossible to express. **Reproduced bug-for-bug** (ruling AR). | `GlobalSettings.java:521-838` | 5 |
| **AH** | The help text documents **10 of the 24** accepted flags. Undocumented: `-drc`, `-oit`, `-dl`, `-da`, `-host`, `-inc`, `-dct`, `-ll`, `--compare-boards=` and the whole `--section.field=value` form. **There is no `-v`/`--verbose`** — the log-level flag is `-ll`. Several locale translations carry machine-translation corruption. | `Freerouting_en.properties:1`; `GlobalSettings.java:825-831` | 5 |
| **AI** | Log output **pollutes stdout and ERROR is triplicated**: the Console appender targets `SYSTEM_OUT`, a second targets `SYSTEM_ERR` at `Level.ERROR`, and the file appender takes both. The `stderr` appender also reuses `filePattern` rather than `PATTERN`, so `--logging.file.pattern=` silently reformats stderr. **Port sends everything to stderr**; `normalize_log` merges Java's two streams. | `Log4j2ConfigurationFactory.java:54-113` | 5, 6 |
| **AJ** | `mcp_server.stdio=true` in `freerouting.json` is **silently ignored** (the stdout redirect must precede logging init) and Java only warns about it 250 lines later. `-dl` is `equals` in `main:1056` but `startsWith` in `GlobalSettings.java:799`; `-ll` takes the **first** occurrence early and the **last** late. **One parse** (ruling 10). | `Freerouting.java:901-920, 1056-1065, 1191-1203` | 5 |
| **AK** | `BoardStatistics`' `host` fallback is **unreachable** — `this.host` is `x + "," + y`, at minimum `"null,null"` — so `"Freerouting,<version>"` can never fire. And `board.boundingBox` is a `Rectangle2D.Float(ur.x, ur.y, ll.x, ll.y)`, i.e. `width`/`height` hold **lower-left coordinates**, which `:385-386` unit-scales as if they were extents. | `BoardStatistics.java:114-121, 126-131, 385-386` | 2 |
| **AL** | `instanceof Pin` is tested before `instanceof DrillItem`, and `Pin` *is* a `DrillItem`, so `items.drill_item_count` never counts pins. | `BoardStatistics.java:165-168` | 2 |

**Amendment log (plan label → landed id).** One line per row above, written by the task that landed it. The register's next free id at plan time is **#197**; Plan 7 will have consumed more, so **read the last row, do not assume**.

**Also carried forward and re-checked here:** **#131** and **#143** (the dead legacy flags and `-mt`, whose product decision ruling AQ closes — the rows gain a "closed by Plan 8 ruling AQ" line), **#140** (`max_passes == 0` is unlimited, and `validate()` is called exactly twice), **#141** (the Gson strictness split, now also governing the MCP `settings` object), **#142** (the `.rules` file parsed twice, now fed as bytes from the CLI), **#144** (the `unconnected_items` hash order, still pinned with `-XX:hashCode=2`), **#145** (the DRC `%.4f` locale hazard, still pinned with `-Duser.language=en -Duser.country=US`), **#151** (`coordinateUnit` hard-coded `"mm"`, and the recorded decision not to expose a unit flag), **#154** (HEAD's camelCase DRC JSON vs KiCad's snake_case, and ruling W's split default), **#83** (the J-then-I clearance-matrix indexing the KiCad reader must not fix), **#200** (`--max-items` also stops the optimizer — **the CLI's help text must say so**, per Plan 7's hand-off).

**`crates/fr-core/README.md`** gains: what the crate is (a composition layer, ruling 1) and what it is not (a home for anything `fr-router` owns); the `CancelToken` → `RouterStop` mapping table with the three-state distinction spelled out (ruling 2, quirk #200); the load sequence's four steps and which two `resolve_headless` owns (§5.7's gap, closed); the manifest's key order; and the roster's structure.

**`crates/freerouting/README.md`** gains: the CLI surface with every accepted flag and which are **deliberately dead** (ruling AQ); the exit-code table (0/1 legacy, 2/3 native, ruling AR); the `p8t1`–`p8t7` acceptance table with the rung each stem reached and every `XDIFF`'s root cause; the **MCP delta table** from Task 11 with the Java file:line beside each of the seven-or-more deltas; how to regenerate `tests/reference/cli-*` and run every driver; and the log-divergence note (everything to stderr, quirk AI).

**`docs/plan-8-handoff.md` — the PROJECT COMPLETION REPORT.** Not a hand-off to a successor; a report to a reader who has none. It must contain, at minimum:
1. **What is ported** — every crate, its Java source packages, its public surface, and the plan that built it. A one-screen map from `app/freerouting/**` to `crates/**`.
2. **What is rostered dead, and why** — the ≈ 13 500 lines, grouped (REST API, analytics, GUI, sessions/queue, the multithread family, the Eagle writer, the two dead scoring classes, `BoardComparator`), **each group with the evidence grep that proves it dead or out of scope**, and the ruling that decided it (AS, AU, AQ, spec §2).
3. **Every deliberate divergence** — one table, every row citing its Java file:line and the ruling or quirk that authorised it: logs to stderr (AI), the `-drc` stdout mode (ruling 6), `INVALID` totalised (ruling 7), one SES write (ruling 9), one argv parse (ruling 10), the seven-plus MCP deltas (AO), the dead legacy flags (AQ), the port-only exit codes 2/3 (AR), `--kicad-json` on the native form only (ruling 14), the KiCad-JSON output path's quirk-T answer, no `schemars`, no threads below `crates/freerouting`, no rng seed in `Ctx`, `PARITY_VERSION`'s three readers (ruling 5).
4. **How to run every parity check in the repository** — the prerequisites (a sibling `../freerouting`, its HEAD jar, `tools/freerouting-2.3.0.jar`, JDK 25 at `/opt/homebrew/opt/openjdk@25`, `FREEROUTING_JAVA_DIR`/`FREEROUTING_JAR`/`JAVA25_HOME`), then the full command list: `cargo test --workspace`, `FR_SLOW_PARITY=1 cargo test --release`, every `scripts/differential/run.sh p{2,3,4,5,6,7,8}t*` case, the four `sweep-*.sh`, and the five `gen-*-reference.sh` regenerators with what each pins and against which jar.
5. **The obligation register, CLOSED** — every row either discharged (with the task and evidence) or **closed with a reason**; no row may say "Plan 9".
6. **The parked residuals** — per task, as every previous hand-off has done.
7. **Known limitations** — spec §2's out-of-scope list restated as *what this program will never do*, plus quirk #113 (the `(string_quote .)` regex divergence Plan 3 could not close without a regex engine), quirk #162 (the non-terminating `calculateNewIncompleteRooms`, still unguarded because a guard Java lacks breaks parity), and the four zero-coverage Plan 3 paths.
8. **The obligation ticks in every earlier hand-off** — Plan 3's seven entry points and ruling A (`LoadedBoard` is the holder), Plan 4's four Plan-8 items, Plan 5's eight, Plan 6's six (§11), Plan 7's list. Each gets its status line **written into that hand-off**, not only into this one.

**Steps:** the twelve audits to zero **without weakening the script** → the three marker greps clean → the roster closed with fresh evidence greps → both READMEs → the quirk rows with the label→id amendment lines → `docs/cli-legacy-flags.md`'s `.json` paragraph → the obligation register closed → `docs/plan-8-handoff.md` (all eight sections) → the ticks in seven earlier hand-offs → fmt/clippy/test + every driver → commit `test(core): audits to zero, the closed register, the READMEs and the project completion report`.

---

## The differential harness — seven drivers, six probes, one reference generator

Same shape as Plans 2–7 (`scripts/differential/README.md`): a `run.sh` case per driver, a `java/P8T<N>.java` half and a `rust/src/bin/p8t<n>.rs` twin, diffed line for line. **Unlike every earlier plan's, most of these drivers do not reflect into jar internals — they run the two whole programs** (ruling 13). Every Java half runs in `run.sh`'s `needs_jar=1` mode against the **clone's HEAD jar** (ruling AV) on **JDK 25**, with:

```
-Djava.awt.headless=true -Duser.language=en -Duser.country=US
-XX:+UnlockExperimentalVMOptions -XX:hashCode=2
```

`-XX:hashCode=2` is mandatory on `p8t3` (quirk #144's `HashMap`-ordered `unconnected_items`) and harmless elsewhere; the locale flags are mandatory on `p8t3` (quirk #145's `%.4f`) and on anything that formats a number. **Plan 7's `RouterBudget::disabled()` rule does NOT apply here**: `p8t*` compares two whole programs, both running their wall-clock budgets live, which is the only faithful comparison of a CLI. Each driver header states that explicitly so it is not read as an oversight.

| driver | what it pins | invocation | task |
|---|---|---|---|
| **`p8t1`** | **the headline gate** — SES **bytes**, exit code, normalised stderr | `-de <dsn> -do <ses>` vs `route <dsn> -o <ses>` | 6 |
| **`p8t2`** | the result manifest, field for field after `normalize_manifest` | `… --router.result_json=<f>` | 4 (shape), 6 (end to end) |
| **`p8t3`** | the DRC report bytes after `date` normalisation, plus the exit code — exercising the **DSN → `.rules` → SES load order** | `-de "<dsn>+<ses>+<rules>" -drc <report.json>` | 7 |
| **`p8t4`** | the **resolved `RouterSettings`** dumped as JSON from both sides — `p4t1`'s 64-case matrix re-run **through the binary**, so quirks #140–#143 are proved end to end | the flag matrix of `docs/cli-legacy-flags.md` × the values that trip each normalisation | 6 |
| **`p8t5`** | slot classification, `LegacyBridge` fields, warnings and the exit code — **not routing** | ~60 argv shapes (see Task 5) | 5 |
| **`p8t6`** | **a documented-delta driver** (ruling AO) — the recorded delta table, asserted to be exactly itself | `mcp` vs `--mcp_server.stdio=true`, driving `initialize` + `tools/list` + `tools/call` | 12 |
| **`p8t7`** | **spec §1's acceptance** — KiCad DSN → route → SES → re-read by `fr_dsn::ses_reader::read`; plus `-de board.json -do out.ses` and quirk T's measurement | | 9, 10, 12 |

**Probes** (`scripts/differential/java/probes/`) — no Rust twin; a Java program whose stdout a Rust *test* pins as literals, so the test asserts against the HEAD jar rather than against the port's own opinion. Named for the task that consumes them, as Plans 6 and 7's are:

| probe | what it pins | package | transcript | task |
|---|---|---|---|---|
| `P8T0Probe.java` | `parseTimespanString` × 30 strings, the 24 h cap and the round trip | `util` | `crates/fr-core/tests/data/p8t0-timespans.txt` | 0 |
| `P8T1Probe.java` | `getFileFormat(byte[]/Path)`, `changeFileExtension`, `setFilename`, `calculateCrc32` — five tables | `core` | `…/p8t1-job-model.txt` | 1 |
| `P8T2Probe.java` | `BoardStatistics(byte[], FileFormat)` field for field, plus `toString()`'s exact JSON | `core.scoring` | `…/p8t2-byte-statistics.txt` | 2 |
| `P8T3Probe.java` | the clearance matrix, `holeClearance`, the reclassified keepout ids and the search-tree leaf count **before/after each override**, at four points of the load | `management` | `…/p8t3-clearance-overrides.txt` | 3 |
| `P8T8Probe.java` | the whole board after `readBoard` — layers, matrix, outline, communication, nets, then the full item graph | `io.kicad` | `…/p8t8-kicad-read-{a,b}.txt` | 8, 9 |
| `P8T10Probe.java` | `KiCadJsonWriter.write`'s exact output and the board after `importSession` | `io.kicad` | `…/p8t10-kicad-writer.txt` | 10 |

**The reference generator.** `scripts/gen-cli-reference.sh` (Task 6) is the sibling of `gen-batch-reference.sh` `[verify at pre-flight]`, sharing its `portable()` path rendering, its preflight and its `timeout(1)` bound (quirk #162 still does not terminate on ~0.4 % of room completions). Three modes:

| mode | what it does |
|---|---|
| *(default)* | regenerates `tests/reference/cli-<stem>/{argv.txt,route.ses,route.exit,route.log,manifest.json,drc.json,meta.txt}` for every row of `tests/reference/cli-fixtures.txt`, **driving the bare HEAD jar** |
| `--meta-only` | rewrites `meta.txt` from the existing outputs without running the jar |
| `--verify-hash-modes` | regenerates each stem under `-XX:hashCode=0..4` and requires **five byte-identical SES files and five identical DRC reports** — Plan 6's premise, re-checked through the CLI |

**Unlike Plan 7's generator there is no `--verify-driver` mode, and the header says why:** Plan 7 needed one because its reference came from a *probe* that reflected a constant to `0`; `gen-cli-reference.sh` runs the **bare jar itself**, so there is no driver to verify.

**The acceptance ladder, and where each rung is first reached:**

| rung | what | first reached | required for |
|---|---|---|---|
| per-connection | plan-6 ruling 1 — unchanged, still green | Plan 6 Task 17 | every task (regression gate) |
| whole-board SES vs the probe | Plan 7 ruling 1(c) — unchanged, still green | Plan 7 Task 16 | every task from 3 on |
| **the clearance overrides** | `P8T3Probe` at 0 diffs on every configuration | **Task 3** | 6, 7, 12 |
| **CLI SES bytes + exit code + logs** | `p8t1` MATCH | **Task 6** | 7, 12, 13 |
| **the manifest** | `p8t2` MATCH | **Task 6** | 13 |
| **the DRC document + the computed score** | `p8t3` MATCH ×8 | **Task 7** | 12, 13 |
| **the settings chain through the binary** | `p8t4` MATCH | **Task 6** | 13 |
| **the legacy surface** | `p8t5` MATCH on ~60 shapes | **Task 5** | 13 |
| **the MCP delta table** | `p8t6` reproduces it exactly | **Task 12** | 13 |
| **spec §1's KiCad round trip** | `p8t7` MATCH | **Task 12** | 13 |

**CI split.** `crates/freerouting/tests/cli_e2e.rs` runs the four CI stems (`router-rpi-splitter`, `router-j2-reference`, `router-ecc83-input`, `router-empty-board`) `java_dir`-gated, skipping cleanly when the clone is absent (Plan 5's convention). The four slow stems are `#[cfg_attr(debug_assertions, ignore)]` + `FR_SLOW_PARITY=1`. `sweep-p8t5.sh` runs the whole argv matrix and reports `MATCH`/`XDIFF`/`SKIP` per row.

## Hand-off checklist (Task 13 signs each line off in `docs/plan-8-handoff.md`)

- [ ] `cargo test --workspace` green; `cargo clippy --workspace --all-targets -- -D warnings` clean; `cargo fmt --all --check` clean.
- [ ] `FR_SLOW_PARITY=1 cargo test --release` green, including the four slow `p8t1` stems.
- [ ] All **twelve** `audit-port.sh` invocations of Task 13 exit 0 with **zero `MISSING`, zero `UNMAPPED`**, run on the committed tree by the reviewer; the `ROSTERED` count for `api/**` recorded.
- [ ] **`grep -rn "added in Plan 8" crates/*/src` and `crates/*/tests` return nothing**; every one of the 23 plan-time sites is consumed with a reason; prose mentions reworded to the past tense.
- [ ] **`grep -rn "added in Plan 9\|added in Plan [0-9][0-9]" crates/ docs/ scripts/` returns nothing** — there is no successor plan.
- [ ] **`grep -rn "EXIT_NOT_IMPLEMENTED" crates/` returns nothing** (ruling AR's exit-code 3 reservation discharged).
- [ ] The five obligations this plan closes are closed with evidence (`resolve.rs:184`, `fr-drc/src/report/json.rs:57`, `fr-dsn/src/parser/wiring.rs:596`, `mcp/server.rs:9-32`, `mcp/stdio.rs:18-28`), and **every surviving `obligation:` row in the register is CLOSED-with-reason, none open**.
- [ ] `#![forbid(unsafe_code)]` present in every crate root touched, **including the new `fr-core`**.
- [ ] **`p8t1` MATCH** on all eight stems: SES bytes, exit code, normalised logs (ruling AV). Any `XDIFF` carries a root cause and the first differing byte.
- [ ] `p8t2`, `p8t3` (×8 stems), `p8t4`, `p8t5` (~60 shapes), `p8t7` MATCH; `p8t6` reproduces the recorded delta table **exactly** — no new delta, no missing delta.
- [ ] `P8T3Probe` at **0 diffs on every cell of every configuration** — the §5.7 gap, closed (risk-2).
- [ ] `gen-cli-reference.sh --verify-hash-modes` five-way identical on all eight stems.
- [ ] Plan 7's `cargo test -p fr-router --test batch_parity` and `run.sh p6t1` on the five Plan 6 stems still green on the committed tree `[verify at pre-flight]`; `sweep-p3t15.sh` still 525 MATCH + 5 XDIFF after Task 3's `wiring.rs:596` change.
- [ ] Ruling 2: the three-state stop survives — `cancel_all_and_cancel_auto_router_are_distinct` green, quirk #200 recorded, and the CLI help text says `--max-items` also stops the optimizer.
- [ ] Ruling 3: exactly two `std::thread` sites, both under `crates/freerouting/src/mcp/`; `grep -rn "std::thread\|rayon" crates/fr-*/src` returns nothing.
- [ ] Ruling AQ: the dead legacy flags recorded as a **product decision** in the hand-off, with the `--set` alternative named.
- [ ] Ruling AS: `BoardComparator`, `BoardScoreBreakdown` and `ScoringWeightComparison` rostered `// not ported:` with their reachability evidence, and the hand-off says so.
- [ ] Ruling AT/5: `PARITY_VERSION` read from the HEAD jar, its `Build-Revision` recorded, and its **three** readers named — with the SES/DSN correction stated.
- [ ] Quirk rows written, ids allocated contiguously from the register's last row, every plan label mapped to its landed id in the amendment block.
- [ ] Ruling 12's Java suites ported (or their absence recorded with the grep that proves it).
- [ ] Both READMEs carry their tables (CLI surface + exit codes + acceptance + MCP deltas + log divergence; `fr-core`'s cancel mapping + load sequence + manifest order).
- [ ] `docs/plan-8-handoff.md` carries **all eight sections** of the completion report, and the obligation ticks are written into all seven earlier hand-offs.

## Dispatch order and sizing

**Fourteen tasks, 0–13:**

0 → 1 → 2 → 3 → 4 → 5 → 6 → 7 → 8 → 9 → 10 → 11 → 12 → 13.

> **Dispatch note — nothing dispatches until Plan 7 has landed and the pre-flight scan has run.** Plan 8 consumes `run_pipeline`, `RouterStop`, `RouterBudget`, `ProgressSink`, the widened `structural_hash`, the eight batch reference stems and `batch_parity` — none of which exist on `main` at the time of writing (the branch is at Plan 7 Task 1, `b899468`). **The pre-flight scan's first job is to walk every `[verify at pre-flight]` tag in this document against the committed tree and amend the plan in place**, exactly as Plan 7's own pre-flight scan produced its 17 rulings. Its second job is **risk-2's conditional**: `grep -rn "apply_copper_to_edge\|apply_hole_clearance\|assign_hole_keepout" crates/` decides whether Task 3 ports the three overrides or only audits them. Its third is to re-read the quirk register's last row so the first Plan 8 task knows its starting id.

Two relaxations are available after Task 1 and the controller may take them:
- **Tasks 8–10 (the KiCad family) share no file with Tasks 4–7** and can run beside them, provided Task 3's `load.rs` stub is in place (it is, with its `obligation:` marker).
- **Task 11 (the MCP transport) is pure new code in `crates/freerouting/src/mcp/` and can run beside anything from Task 0 on** — it consumes only `CancelToken`.

Everything else is a hard dependency: Task 1 needs Task 0's `Error`/`PARITY_VERSION`; Task 2 needs 1; Task 3 needs 1 (and stubs 9); Task 4 needs 1, 2; Task 5 needs 0; **Task 6 needs 0, 1, 2, 3, 4, 5** and builds the `tests/parity` helpers every later driver uses; Task 7 needs 2, 3, 6 (and stubs 10); Task 9 needs 8; Task 10 needs 9 and discharges Tasks 6 and 7's stubs; Task 12 needs 6, 7, 11; Task 13 needs everything.

Sizing. **Task 3** (an unrostered 370-line gap that mutates the board under every caller, on a path Plans 3–7 all depend on), **Task 6** (the plan's headline gate — 410 Java lines, sixteen ordered steps, five quirks and the harness every later driver consumes), **Task 8** (577 Java lines at the ceiling, and quirk #83 sits in the middle of it) and **Task 13** (twelve first-ever audits plus the project completion report) are the four largest: **opus, reviewed twice**. **Tasks 0, 1, 7, 9, 11, 12** are large: **opus** (Task 0 because the concurrency seam and the three-state mapping decide whether cancellation works at all; Task 1 because three of its methods are totalisations; Task 7 because the load order and the separate merge are both silently wrong if transposed; Task 9 because the two Java recovery boundaries are easy to invent instead of reproduce; Tasks 11 and 12 because they are the port's only concurrency and its only new protocol). **Tasks 2, 4, 5, 10** are medium: **sonnet** — Task 2 because it is a transcription pinned field-for-field by a probe, Task 4 because the manifest is a DTO tree plus a hash, Task 5 because it deletes more than it writes, Task 10 because the writer is mechanical (its one judgement call, quirk T, is a *measurement* the steps prescribe).

Reviewers: **opus for 0, 1, 3, 6, 7, 8, 9, 11, 12, 13; sonnet otherwise.** **From Task 3 on, every review re-runs `cargo test -p fr-router --test batch_parity` and `run.sh p6t1` on the five Plan 6 stems** `[verify at pre-flight]`, reading the header line to confirm which jar it used. From Task 6 on, every review additionally runs `p8t1` on the four CI stems **on the committed tree, not on the implementer's word**. Task 13's audits are run by the reviewer.

## Plan self-review

**Spec coverage.** **§4's crate list is complete after this plan** — `fr-core` is the eighth and last crate, and its stated contents (`RoutingPipeline`, `CancelToken`, `ProgressSink`, `RoutingResult`, `BoardStatistics`, the result manifest) map to Tasks 0, 0, 0, 0, 2 and 4. The spec's dependency direction `freerouting → fr-core → fr-router → fr-drc → fr-board → fr-geometry` is honoured exactly; `fr-dsn` and `fr-settings` still depend only on `fr-board`/`fr-geometry`, and `fr-dsn` gaining the `kicad` module does not change that. **§10** is Tasks 0 (all four bullets) and 6/12 (the callers); **two deviations, both recorded**: `Ctx` has **no rng seed** (the port has no randomness — plan-6 ruling 5 forbids `rand`, and `ItemSelectionStrategy::Random` is GUI-only) and **no `max_threads`** (quirk #143, extended by Plan 7 to "dead everywhere"). **§12** is Tasks 5 (the shim, the flags, the exit codes), 6 (`route`), 7 (`drc`) and 12 (`info`, `mcp`); its `-v`/`--log-level` line survives **only on the native form** (ruling AR — Java has no `-v`, quirk AH), and its "drc and info write JSON to stdout when no `-o`" line is ruling 6's recorded divergence from a Java branch that is unreachable. **§13** is Tasks 11 and 12 in full — all four tools, all six methods, `notifications/progress` outbound and `notifications/cancelled` inbound — with **one deviation**: the tool input schemas are **hand-written, not `schemars`-generated** (controller ruling AO/General), and the anti-drift pair of tests in Task 12 is what replaces the derive. **§14.4's** three end-to-end cases (CLI `route` on `tutorial_board.dsn` re-read by `SesReader`, the legacy shim, MCP over spawned pipes) are `p8t7`, `p8t5` and Task 12's conversation test. **§15 steps 9 and 10** are this plan in full. **§3's parity contract is deliberately strengthened**, as Plans 5–7 each strengthened it: §3 asks for "metric parity" on the router and the port already had SES byte parity from Plan 7; ruling AV now demands SES **bytes plus exit code plus normalised logs** from the two whole programs. **§2's out-of-scope list is honoured without exception** and is what ruling AU (no HTTP API), ruling AS (no `BoardComparator`) and Task 0's roster enforce; **§2's "KiCad session JSON output" exclusion is the one line this plan re-opens**, because `-do out.json` is a live jar path that `p8t1`'s argv space reaches — Task 10 ports the writer and quirk T's row records why, which is a **narrowing of an exclusion, stated here so it is visibly a choice**.

**Deliberately excluded, with citation:** the whole REST API (`api/**`, 8 425, ruling AU, with `JobControllerV1`'s dead-code evidence); the HTTP/SSE/WebSocket MCP transports and the OpenAPI tool registry (`api/mcp/**`, 2 105, ruling AO); analytics and the version checker (2 219, spec §2); the GUI and `DebugControl` (spec §2); `SessionManager` and `Session`'s job list (230, quirk AA); the scheduler's queue half (~450, single-shot CLI); the CPU/memory monitor (50, quirk Y, and `p8t2` normalises its output away); `core/events/**` (130, spec §10's `ProgressSink`); `RoutingJobPriority`/`RoutingStage` (30, never read); `HeadlessBoardManager`'s six diagnostic methods (~290, quirk AE); `Freerouting`'s API/MCP/Jetty half (~330) and `startMcpStdioBridge` (`// renamed:`); `compareBoardFiles`/`loadBoardFromFile`/`BoardComparator` (824, ruling AS); `BoardScoreBreakdown`/`ScoringWeightComparison` (424, ruling AS, quirk X); `SessionToEagle` (627, spec §2); `FRLogger`/`Log4j2ConfigurationFactory` (617, not a port — the message set only); the donation banner (quirk H); the two nested polling loops (ruling 8); the per-event SES rewrite (ruling 9); the pre-bootstrap argv parse (ruling 10); `UIManager.setLookAndFeel` and the unconditional 1 s sleep (quirk AF).

**Placeholder scan.** No "TBD", no "add error handling", no "similar to Task N". Every task names its Java files with line ranges, the signatures it produces, the interfaces it consumes, its JVM-pinned evidence with the exact acceptance, its tests and its commit message. **Seven places name a decision the implementer must make rather than one this plan makes, each with the default already chosen and the deciding evidence named:** risk-2's conditional scope for Task 3 (decided by a pre-flight grep, not at port time); quirk T's answer in Task 10 (**measured** by a prescribed jar run, then reproduced or totalised — the plan refuses to guess); whether the corpus contains a `.json` board fixture at all (Task 8 — the fallback is prescribed: generate one with the jar's own writer); whether any `src/test/java` suite covers `RoutingJob`/`BoardFileDetails` (ruling 12 — grep, and record the absence if it is absent); `PARITY_VERSION`'s literal (read from the jar, never guessed — `Constants.java` is build-generated); whether Task 4's SHA-256 can reuse an existing workspace dependency (check `Cargo.lock` first, hand-write otherwise); and whether the jar's stdio MCP mode can be driven headlessly at all (Task 12 — a `SKIP` with the reason *is* a delta). All seven are decisions-with-a-default. **Two constant lists this plan deliberately does not transcribe** — `RoutingJobState`'s and the `UnitJson` enum's — are marked "read the file" rather than guessed, and that is recorded here so it is visibly a choice.

**Type consistency across tasks.** `CancelToken`, `Deadline`, `SyncProgressSink`, `Ctx`, `RoutingResult` are fixed in Task 0 and used unchanged by Tasks 5–12. `RoutingJob`, `RoutingJobState`, `FileFormat`, `BoardFileDetails`, `SessionId` in Task 1, consumed by 2, 3, 4, 6, 7, 12 and **never re-declared** — Task 2 extends `fr_router::score::BoardStatistics` through an **extension trait**, not a second struct (Plan 7's ruling 4 makes `fr_router::score` the only home of that type, and Plan 7 controller answer 3 makes `fr-core` re-export it). `LoadedBoard` in Task 3, which is also the discharge of Plan 3 ruling A's "whatever holds a `Board` must hold its `CoordinateTransform`". `RoutingResultManifest` and its three nested DTOs in Task 4. `ExitCode` and the rewritten `legacy::rewrite` in Task 5; **`LegacyError` is deleted**, because ruling AR maps every legacy failure to exit 1 and the shim no longer has failure modes. `fr_dsn::kicad`'s twelve DTOs and `read_board`'s **signature** in Task 8, its **body completed** in Task 9 (a body extension, not a redeclaration), `write`/`import_session` in Task 10. `ToolHandler`'s signature **changes once**, in Task 11, and Task 12 writes four handlers against the new one. `BoardSummary` in Task 12, shared by `info` and `board_info` so the two cannot drift. The plan makes **no API change to `fr-geometry`, `fr-board`, `fr-drc`, `fr-settings` or `fr-router`**; it makes **one additive change to `fr-dsn`** (the `kicad` module) and **one behavioural change** (`wiring.rs:596`'s checked insert, guarded by `sweep-p3t15.sh`), adds **one crate**, and rewrites `crates/freerouting` — which no plan since Plan 1 has touched.

**What could still go wrong, in order of cost.** (1) A `p8t1` `XDIFF` traced to Plan 7 rather than to Plan 8 — mitigated by the `batch_parity` regression gate on every task from 3 on, which localises it before Task 6 runs. (2) Task 3's overrides changing a board that Plan 7's references were generated against — the same gate catches it, and it is *expected* on a KiCad board, which is why `P8T3Probe` runs the non-default hole clearances. (3) A `[verify at pre-flight]` signature having moved — the scan's whole purpose. (4) The jar's stdio MCP mode proving undrivable — degrades `p8t6` to a one-sided contract driver, which ruling AO already anticipates by making it documented-delta rather than parity.

## Controller questions

Five, in descending order of what a wrong default costs. Each has a default already written into the plan, so a silence is answerable; these are the places the survey and the rulings left genuinely ambiguous.

1. **Spec §2 excludes "KiCad session JSON output", but `-do out.json` is a live jar path and `p8t1`'s argv space reaches it.** Task 10 therefore ports `KiCadJsonWriter.write` (227 lines) and Task 9's `-de board.json` closes `--kicad-json`'s loader gap — i.e. the plan **narrows a spec exclusion**. The alternative is to reject `-do *.json` with exit 1 and record it as a port limitation, saving ~330 lines and one quirk (label T, whose answer must be *measured* against the jar and may not be cheap to reproduce). **Default: port it**, because a KiCad-focused tool that cannot write the KiCad format is a strange place to stop. Confirm, or authorise dropping Task 10's writer half.
2. **`p8t1`'s stem set.** Ruling AV says "the router reference stems plus the batch stems Plan 7 Task 16 creates". Plan 7's Task 16 defines **eight** batch stems, four in CI and four behind `FR_SLOW_PARITY=1`; this plan adds the two Plan 3/5 round-trip stems (`tutorial_board`, `Issue026-J2_reference`) so the unrouted/SES-syntax surface is also exercised through the binary. **But none of the eight is a genuinely KiCad-exported board**, which is the only way to exercise §5.7's copper-to-edge override on its non-early-return path — the very gap risk-2 exists to close. **Default: add one KiCad-exported `.dsn` fixture to `cli-fixtures.txt`**, sourced from the clone's corpus if one qualifies (`Issue649-kicad_ecc83-pp_input_board_v1.dsn` is the candidate) and generated from KiCad otherwise. Confirm which, or confirm that a synthetic board whose outline uses the fallback AREA class is an acceptable substitute.
3. **Ruling AR reserves exit codes 2 and 3 for the native subcommand form, and Task 13's checklist then demands `EXIT_NOT_IMPLEMENTED` be unreachable.** That leaves **2** (clap's usage error) as the only port-only code, and **3** as a documented-but-unused reservation. **Default: keep 3 reserved and unused, documented in the hand-off's divergence table.** The alternative is to delete it and say the port has exactly three exit codes (0, 1, 2). Confirm — it is a one-line difference in a table a user reads.
4. **Ruling AQ keeps `-mt`/`--threads` inert, and ruling 3 introduces the port's only two threads (both in the MCP transport).** A reader will connect the two and ask whether `--threads` ought to control the MCP's concurrency. It must not — the MCP's threads are a transport, not a routing policy, and making `--threads` live would resurrect a flag Java reads nowhere. **Default: `--threads` stays inert on every path, and the README says the MCP's two threads are not configurable.** Confirm, because it is exactly the kind of "helpful" wiring a later reader would add.
5. **This is the last plan, so `docs/plan-8-handoff.md` doubles as the project completion report — but three known limitations will still be open when it is written:** quirk #113 (`(string_quote .)` needs a regex engine the dependency rule forbids), quirk #162 (the non-terminating `calculateNewIncompleteRooms`, unguarded because a guard Java lacks breaks parity) and the four zero-coverage Plan 3 paths (each needing a synthetic fixture plus JVM ground truth). **Default: record all three in §7 "Known limitations" as closed-with-reason and do not attempt them in Plan 8** — each is out of proportion to its risk and two of them would *create* divergence. Confirm, or authorise a fifteenth task for the four zero-coverage paths (the only one of the three that is a pure test-coverage exercise with no parity risk).

---

## Controller answers to the plan's questions (2026-08-30, binding)

1. **`KiCadJsonWriter`:** conditional — port the writer half ONLY if a headless jar flow reaches it (then parity is measurable and it serves the project's KiCad focus); the pre-flight scan verifies reachability from `main()` with the survey's call graph. If GUI-only, roster `// not ported:` and list it in the hand-off as the top post-parity candidate.
2. **KiCad-exported stem for `p8t1`:** required. Grep the fixture corpus for `(host_cad` headers naming KiCad (`grep -rl 'host_cad.*KiCad' ../freerouting/fixtures ../freerouting/examples`), pick one that exercises the copper-to-edge override's non-early-return path (measure with the Task-covering probe), and add it to the stem table. If none in the corpus qualifies, say so in the task report and pin the override path with a probe-built board instead.
3. **Exit code 3:** keep reserved-but-unused, documented in the CLI help and hand-off; revisit only if still unused at project close.
4. **`--threads`:** stays inert (ruling AQ). The MCP reader thread is transport, not routing; document that distinction where the flag is parsed.
5. **Quirks #113/#162:** ship as closed-with-reason limitations. The four zero-coverage Plan 3 paths: **15th task authorised** (small, coverage-only — directed fixtures per path, no behaviour change); the project hand-off must show zero known-uncovered ported paths or name each survivor with its reason.

## Banked evidence (2026-08-30, scratchpad/plan8-evidence/)
- `job1-{summary.md,overrides.txt}` — the clearance-override measurements. NOTE: the copper-to-edge port itself moved to **Plan 7 Task 15b (ruling AW)**; Plan 8's task on this shrinks to the `management/` audit + wiring `prepare_board` into the CLI loader, and the 100/500 µm hole-path pins.
- `job2-{summary.md,argv.txt}` — the 87-shape legacy argv baseline (p8t5's reference; four disagreeing parsers, quirk #132 confirmed, `FREEROUTING__USER_DATA_PATH` crash = new quirk label).
- `job3-{summary.md,mcp.jsonl}` — the Java stdio-MCP baseline (p8t6's documented-delta reference: 28 tools, `2024-11-05`, auth-on-by-default + blank-line desync + id-less `-32700` traps).
The pre-flight scan must verify these against the tree/jar at execution time.

## Post-parity direction (user priority, 2026-08-31)
The final task's project hand-off must include a **post-parity roadmap section ordered by ROUTING QUALITY, not speed** (user ruling): (1) hang/OOM quirk fixes (#162, #106, #86); (2) wrong-geometry fixes (#5, #62, #183, #210-class); (3) the DRC accuracy cluster (#144/#146–#148); (4) settings predictability (#140/#142, dead flags). Speed work (real parallel passes — Java's multithreading is dead code, so this is greenfield) is explicitly LAST. Recommend the `Compat::{Java, Fixed}` switch design in that section, with `Java` mode preserving the differential harness as the regression net.

## Controller answer 2 RESOLVED (2026-08-31): the KiCad-exported override-exercising stem for `p8t1` is `Issue508-DAC2020_bm01` — its DSN header carries `host_cad KiCad` (grep-verified) AND it is on plan8-evidence/job1's mutated list; it is already the `router-dac2020-bm01` reference stem. `Issue026-J2_reference.dsn` is the KiCad-exported backup. No probe-built board needed.

## Roadmap import note (2026-08-31)
`scratchpad/post-parity-roadmap.md` (721 lines) is the hand-off's post-parity section — import verbatim, do not re-derive. Two of its findings act on Plan 8 itself: (a) the routing reference corpus (six rows / five boards) lacks a 90-degree board, a per-layer-width board, a large/circular-outline board, a multi-net-SMD board and a signal-layer-pour board — the acceptance-ladder task should add stems for these classes where the fixture corpus has candidates (they also become the post-parity A/B corpus); (b) the `route` subcommand Plan 8 delivers is the A/B harness's dependency — note that in its task.
