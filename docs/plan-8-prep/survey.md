# Plan 8 survey — `fr-core`, the headless CLI, the stdio MCP server, KiCad end-to-end

Java root: `/Users/em/Development/freerouting/freerouting/src/main/java/app/freerouting/` (HEAD).
Rust root: `/Users/em/Development/freerouting/freerouting-rs`, branch `plan-7-router-batch`
(Plan 7 Task 1 committed as `b899468`; **Tasks 2–17 land before Plan 8 starts and are treated as
EXISTING throughout** — every interface named in
`docs/superpowers/plans/2026-08-30-plan-7-router-batch.md` §Interfaces and in each task's
*Interfaces produced* block is assumed present). Read-only survey. Structure matches
`scratchpad/survey-plan7.md`.

**Headline numbers.** Plan 8's Java port target is **≈ 4 300 lines across 19 classes**
(`Freerouting`'s CLI/DRC halves, `RoutingJob` + `BoardFileDetails`, `HeadlessBoardManager`'s
~340 real load/save lines, the `core/scoring` remainder, `RoutingResultManifest`, `io/kicad`'s
three classes, the scheduler's essential slice, two `TextManager` helpers), plus **≈ 1 150 lines
of NEW code with no Java counterpart** (the native stdio JSON-RPC MCP server, its four tools, the
`schemars` surface, `fr-core`'s job façade, `freerouting info`). **≈ 13 500 lines are
GUI/cloud/dead and rostered** (`api/**` 8 425 of which `JobControllerV1`'s 659 are dead code,
`analytics/**` 2 100, `management/sessions` 252, the scheduler queue ~600, the Jetty MCP
transport 2 105 + 390, `SessionToEagle` 627, `BoardScoreBreakdown` + `ScoringWeightComparison`
424 — see §1.3).

**The MCP finding, up front.** Java **does** have a stdio MCP mode, and it is **not** a JSON-RPC
server: `Freerouting.startMcpStdioBridge` (`Freerouting.java:681-788`) is a *pipe-to-HTTP bridge*.
It reads newline-delimited JSON from `System.in`, `POST`s each line to
`http://127.0.0.1:<port>/v1/mcp` on its own embedded Jetty server, and prints the HTTP response
body — with **every `\r` and `\n` stripped** — to a saved `originalSystemOut`. The protocol lives
in `api/mcp/McpControllerV1.java` (655) + `api/mcp/OpenApiMcpToolRegistry.java` (659), inside the
REST API that spec §2 drops. And it is a **thin** protocol: only four methods (`initialize`,
`notifications/initialized`, `tools/list`, `tools/call`) — **no `ping`, no
`notifications/progress`, no `notifications/cancelled`, no progress or cancellation of any kind**
(`McpControllerV1.java:189-198`; the agent card admits `"streamingToolCalls": false` at
`AgentCardController.java:128`). Its ~29 tools are **auto-derived from the OpenAPI scan of
`/v1/*`** with a bucketed `{path, query, body}` input shape, and model a **7-call stateful
session+job pipeline**. Plan 8's MCP server is therefore **new code implementing spec §13**, and
`p8t6` cannot be an equality driver. See §1.4, §2.5 and Q1.

---

## 1. Class inventory

### 1.1 In scope, not yet ported (the port target)

Line counts are whole-file (`wc -l`); method ranges are Java line ranges.

| # | Java file | LOC | What Plan 8 must port | Rust home (proposed) |
|---|---|---|---|---|
| 1 | `Freerouting.java` (CLI half) | 1497 | `initializeCli` `:79-187` (109), `isCliTerminalState` `:189-194` (6), `writeCliOutputIfAvailable` `:196-213` (18), `computeCliExitCode` `:215-223` (9), `writeCliResultManifestIfRequested` `:225-244` (20), plus `main`'s headless slice: the stdio pre-scan `:899-920`, the logging pre-parse `:1005-1105`, the merger prototype `:1408-1413`, the CLI-vs-DRC branch `:1458-1467`, the exit ladder `:1469-1495` | `crates/freerouting/src/commands/route.rs` + `fr-core` |
| 2 | `Freerouting.java` (DRC half) | — | **`initializeDrc` `:246-374` (129)** — session, `setInput` `:262-268`, `BoardLoader.loadBoardIfNeeded` `:271-274`, the `.rules` load `:277-294`, the `.json`-vs-`.ses` session branch `:296-329`, `DesignRulesChecker` `:332-333`, hard-coded `coordinateUnit = "mm"` `:336`, `generateReport` `:339-340`, the **quality-score sub-merge** `:342-352`, `GsonProvider.GSON.toJson` `:354`, the file-vs-stdout write `:356-371` | `crates/freerouting/src/commands/drc.rs` |
| 3 | `core/RoutingJob.java` | 548 | `getFileFormat(byte[])` `:151-227` (77, magic-byte sniffing), `getFileFormat(Path)` `:230-247` (18, extension switch), `setInput`×3 `:271-285`, `setRules`×3 `:288-310`, `tryToSetInput` `:335-349`, `changeFileExtension` `:352-374`, `tryToSetOutputFile` `:377-397` (the accept-list + the `KICAD_DESIGN_JSON → KICAD_SESSION_JSON` rewrite at `:391`), `setInputFromFile` `:425-461` (the output-filename derivation), `getCurrentPass`/`setCurrentPass` `:250-257`, `getDuration` `:260-268`, the four `log*` helpers `:526-547`, the state enum + terminal set | `crates/fr-core/src/job.rs` |
| 4 | `core/BoardFileDetails.java` | 219 | `BoardFileDetails(File)` `:59-67`, `(BasicBoard)` `:70-72`, `calculateCrc32(InputStream)` `:75-87`, `getAbsolutePath` `:96-98`, `setData` `:105-119` (size + CRC32 + re-sniff + text-scrape statistics), `getFile` `:127-132`, **`setFilename` `:149-197`** (49 — path split, Windows-only surgery, extension inference and default-extension append), `getFilenameWithoutExtension` `:200-205` | `crates/fr-core/src/file_details.rs` |
| 5 | `core/results/RoutingResultManifest.java` | 172 | **the whole file** — 12 `@SerializedName` fields `:28-65`, `FixtureInfo` `:68-74`, `PhaseMetrics` `:77-86`, `PhaseDetail` `:89-95`, `fromJob` `:98-135`, `write` `:138-144`, `resolveGitSha` `:147-161`, `sha256Hex` `:163-171` | `crates/fr-core/src/manifest.rs` |
| 6 | `core/scoring/BoardStatistics.java` (remainder) | 648 | **`BoardStatistics(byte[], FileFormat)` `:436-552` (117)** — the SES / DSN / KiCad-JSON text scraper; `countOccurrences` `:578-586`; `toString` `:589-591` (the Gson surface). Plan 7 Task 1 landed the rest | `crates/fr-core/src/stats_from_bytes.rs` |
| 7 | `management/HeadlessBoardManager.java` (the real slice) | 1011 | **~340 real lines, and a gap nothing has rostered** (§5.7): ctor `:168-170`, `getRoutingBoard` `:254-257`, `replaceRoutingBoard` `:276-278`, **`createBoard` `:309-344`** (called *by the DSN parser*, `Structure.java:1268`), **`applyHoleClearanceOverride` `:346-396`**, **`assignHoleKeepoutClearanceClass` `:404-464`**, **`applyCopperToEdgeClearanceOverride` `:466-552`**, `getCurrentRoutingJob` `:569-572`, `calculateCrc32ForBoard` `:607-618`, `loadFromSpecctraDsn` `:673-705`, `applyParsedBoardResult` `:711-737`, **`applyRouterSettingsForLoadedBoard` `:739-749`**, `applyImmediatePostLoadProcessing` `:751-757`, `loadFromKiCadJson` `:794-823`, `saveAsSpecctraSessionSes` `:862-877` | `crates/fr-core/src/load.rs` + `save.rs` (free functions; there is no manager object) |
| 8 | `io/kicad/KiCadJsonReader.java` | 1011 | `readBoard`, `importSession` (the `.json` branch of `initializeDrc:302-307`), `addPoint`, `boundingBox` | `crates/fr-dsn/src/kicad/reader.rs` |
| 9 | `io/kicad/KiCadJsonWriter.java` | 227 | `write` — the KiCad board/session JSON writer (`RoutingJobSchedulerActionThread.java:277-278` uses it for `-do out.json`) | `crates/fr-dsn/src/kicad/writer.rs` |
| 10 | `io/kicad/KiCadBoardJson.java` | 142 | the DTO tree the two exchange | `crates/fr-dsn/src/kicad/dto.rs` |
| 11 | `management/BoardLoader.java` | 57 | `loadBoardIfNeeded` `:19-56` — the DSN-vs-KiCad-JSON dispatch and the "only DSN and JSON" guard `:32-37`. **DRC-mode-only**; the route path loads through the scheduler | `crates/fr-core/src/load.rs` |
| 12 | `management/jobs/RoutingJobScheduler.java` (essential slice) | 578 | from the 235-line polling ctor, only: the board load `:91-101`, **the second settings merge `:103-170`** (already linearised by `resolve_headless`), the post-merge `RulesReader.read` `:173-184`, `applyBoardSpecificOptimizations` `:186`, the optional session import `:189-234`, the thread hand-off `:237-241` and the terminal-state assignments. The queue is §1.3 | `fr-core/src/pipeline.rs` |
| 13 | `management/jobs/RoutingJobSchedulerActionThread.java` | 296 | `threadAction` `:37-206`: `startedAt` `:39`, the timeout parse `:44-52` (`MAX_TIMEOUT = 24 h` at `:24`, `GRACE_PERIOD = 30 s` at `:25`), the monitor thread `:55-90`, `routerEnabled` `:92-94`, `createForHeadless` `:99`, the two `setJobOutput` calls `:100` and `:168`, `finishedAt` `:174`, state finalisation `:175-184`; **`setJobOutput` `:259-295`** | `fr-core/src/pipeline.rs` |
| 14 | `core/scoring/BoardScoreBreakdown.java` | 192 | `of` `:125-142`, private ctor `:84-114`, `validateWeights` `:144-161`, `toSummaryString` `:166-191`. **Dead in `main/`** — see §1.3 and Q6 | `crates/fr-core/src/score_breakdown.rs` — or roster |
| 15 | `core/scoring/ScoringWeightComparison.java` | 232 | `compare` `:41-47`, `Result` `:58-97`, `isCandidateBetter` `:100-102`, `toReportString` `:128-230`. **Dead in `main/`** | `crates/fr-core/src/score_compare.rs` — or roster |
| 16 | `util/TextManager.java` (four methods) | 335 | `parseTimespanString` `:83-93`, `convertFromTimespanToDurationFormat` `:101-118`, `removeQuotes` `:142-149` (used by the byte scraper), `unescapeUnicode` | `crates/fr-settings/src/timespan.rs` (Plan 4 reserved the marker at `lib.rs:193`) |
| 17 | `core/Session.java` | 62 | the ctor's host normalisation + `split("/")` validation `:27-43` only — collapse the rest to a `SessionId(Uuid)` (§8) | `fr-core/src/job.rs` |
| 18 | `board/state/BoardComparator.java` | 758 | `compare` + `ComparisonResult` — re-pointed here by plan-5 ruling 13. **Only Java caller is `Freerouting.compareBoardFiles:826-863`, the undocumented `--compare-boards` mode.** Q5 | roster, unless `--compare-boards` is wanted |
| 19 | `logger/FRLogger.java` + `Log4j2ConfigurationFactory.java` | 480 + 137 | **not a port**: the message set and level mapping the CLI must emit, and the pattern `"%d{yyyy-MM-dd HH:mm:ss.SSS} %-6level %msg%n"` (`Log4j2ConfigurationFactory.java:29`) that the log normaliser must strip | `crates/freerouting/src/logging.rs` |

**Subtotal (rows 1–13, 16, 17): ≈ 4 300 Java lines.** Rows 14/15 add 424 if the controller wants
them; row 18 adds 758.

### 1.2 Already landed (do not re-survey)

| Java | Rust |
|---|---|
| `io/specctra/**` readers/writers (DSN, SES, `.rules`) | `fr-dsn` — `read_board`, `read_metadata`, `write`, `ses_writer::write`, `ses_writer::snapped_endpoint`, `ses_reader::read`, `rules_reader::{read, read_router_settings, discover_layer_structure}`, `rules_writer::write` (plan-3 hand-off §Public API surface) |
| the whole settings ladder: both merges, both board passes, the private cost flag, the post-merge `.rules` re-apply, both `validate()` calls | `fr_settings::resolve_headless` + `SettingsInputs` (`resolve.rs`) |
| `GlobalSettings.applyCommandLineArguments`' value normalisation; the `-de` classifier | `fr_settings::{apply_command_line_arguments → LegacyBridge, classify_de_arguments}` (`sources/cli.rs`) — **built, but `crates/freerouting/src/legacy.rs` does not call either yet** |
| `drc/**`: `DesignRulesChecker`, `ClearanceViolation`, `NetIncompletes`, `UnconnectedItems`, `AirLine`, the KiCad DRC JSON report | `fr_drc::{DesignRulesChecker, generate_report, report_to_json, DrcJsonFlavor, KiCadDrcReport, DrcReportOptions, DrcCoordinates}` |
| `core/scoring/BoardStatistics`' computing ctor `:110-427`, `calculateScore` `:597-616`, `getMaximumScore` `:619-621`, `getNormalizedScore` `:624-635`, `isPinEscaped` `:555-576`, the ten DTOs | `fr_router::score::*` (Plan 7 Task 1, `b899468`) |
| `core/StoppableThread`, `StopRequestState`, `RouterCounters`, `ProgressThrottler` | Plan 7 Task 4 — `fr_router::pipeline::{RouterStop, StopRequestState, RouterBudget, RouterCounters, PassRecord, TaskState, NamedAlgorithmType, ProgressSink, RoutingEvent, NoopProgressSink}` |
| `autoroute/pipeline/RoutingPipeline.{run, runRoutingStage, runOptimizationStage, normalizeRouterAlgorithm}` | Plan 7 Task 15 — `fr_router::pipeline::run_pipeline` → `PipelineResult { router_state, optimizer_state, passes_run, fanout, per_pass, final_statistics, timed_out }` |
| `AutorouteUnroutedReport.{build, describeItem}` | Plan 7 Task 15 — `fr_router::pipeline::build_unrouted_report` |
| the whole batch router (fanout / passes / optimizer / via optimizer / tighteners) | Plan 7 Tasks 5–14 |
| `RoutingBoard.reduceNetsOfRouteItems` (`:1284-1356`) | `fr_board::Board::reduce_nets_of_route_items` (`board/query.rs:1070`) — **ported but never called; Plan 8's load sequence must call it** (`HeadlessBoardManager.java:755`) |

### 1.3 GUI-only, cloud-only or dead — roster as `// not ported:` (≈ 13 500 lines)

| Java | LOC | Evidence |
|---|---|---|
| `api/v1/JobControllerV1.java` | 659 | **Dead code.** Not in `FreeroutingApplication.getClasses()` (`api/FreeroutingApplication.java:36-63`), zero references outside three javadoc `@link`s calling it "the compatibility façade". Do not use as a spec source |
| `api/v1/{JobOutputResource 662, JobInputResource 550, JobProgressResource 440, SessionControllerV1 337, AnalyticsControllerV1 271, SystemControllerV1 155}` | 2 415 | Spec §2: REST API + sessions/job queue out of scope. **Their DTO field names are worth reading** — `api/dto/BoardFilePayload.java` is `job_id` + `data` (Base64) + inherited `size`/`crc32`/`format`/`statistics`/`filename`/`path`. `JobOutputResource.getDrcReport :589-661` makes the same `fr-drc` call the CLI does |
| `api/mcp/**` (`OpenApiMcpToolRegistry` 659, `McpControllerV1` 655, `McpWebSocketEndpoint` 130, `AgentCardController` 133, `McpApiKeyValidation{Service,Filter}` 216, `McpRealtimeBridge` 71, `McpContextListener` 70, `McpRateLimitFilter` 94, `McpApplication` 45, `McpWebSocketConfigurator` 32) | 2 105 | Spec §2 drops HTTP/SSE/WebSocket MCP transports. `McpControllerV1` is `@Path("/v1/mcp")` with SSE at `:250`; `McpWebSocketEndpoint` is `@ServerEndpoint("/v1/mcp/ws")`. Read for tool *shape*, not transport |
| `api/security/**`, `api/{ApiKeyValidationFilter, ApiUsageFilter, ApiAnalyticsFilter, EnvironmentHostValidationFilter, FixedWindowRateLimiter, ApiRateLimitFilter, CorrelationIdFilter, SwaggerUIResource, OpenApiResource, OpenApiConfig, BaseController, GsonMessageBodyHandler, JsonStringMessageBodyWriter, *ExceptionMapper, AppContextListener, FreeroutingApplication}`, `api/dev/**` (808) | 3 246 | API keys, Google Sheets, rate limiting, Swagger, Jersey wiring, mocks |
| `analytics/**` (`FRAnalytics` 714, `NetworkProxyConfig` 372, `AnalyticsErrorAggregator` 339, `BigQueryClient` 318, `FreeroutingAnalyticsClient` 216, `SegmentClient` 122, dto/*) | 2 100 | Spec §2. `main:1333-1385` + the flush shutdown hook `:1360-1362`; all network I/O |
| `util/VersionChecker.java` | 119 | Spec §2. `main:1388-1391`, HTTPS GET to `api.github.com` on a daemon thread |
| `management/sessions/SessionManager.java` + `core/Session.java`'s job list | 190 + ~40 | §8: the session exists only so `enqueueJob:315-318` can validate a `userId` it then never uses. `getMonitoredSessionId :32-34` has zero callers |
| `RoutingJobScheduler`'s queue half (`:48-282`'s poll loop, `saveJob :350-377`, `saveJobToDisk :379-452`, `getQueuePosition`, three `listJobs`, `getJob`, `clearJobs`, `cancelJob`) | ~450 | A single-shot CLI has one job. `saveJobToDisk`'s `Files.write` at `:445-450` is **not** the CLI output write — it is gated on `featureFlags.saveJobs` and called only from `api/v1/*` and `cancelJob` |
| `RoutingJobSchedulerActionThread.{monitorCpuAndMemoryUsage :208-257}` | 50 | Fills only `resource_usage.{cpu_time, max_memory, peak_memory}`; `io_read`/`io_written` are never written. Optional |
| `HeadlessBoardManager.{compareCounterpartBoardIfPresent :172-207, loadBoardFromFileForComparison :209-237, validatePowerPlanes :914-1010, conductionAreasOverlap :879-898, getConductionAreaNetNames :900-912, scheduleDeferredPostLoadProcessing :759-784}` | ~290 | Diagnostic only. `compareCounterpart*` loads a **second whole board from disk** to log a diff; `validatePowerPlanes` emits one `FRLogger.warn`; the deferred pass runs a virtual thread that races the router (Q22) |
| `core/events/**` (6 files) | 130 | All four listener registration points are in `api/v1/*` only. Spec §10's `ProgressSink` replaces them |
| `core/{RoutingJobPriority, RoutingStage}` | 30 | `RoutingJobPriority.value` is never read (ordering uses `ordinal()`, `RoutingJob.java:411-413`); `RoutingJob.stage` is initialised to `IDLE` and never reassigned in `main/` |
| `core/scoring/{BoardScoreBreakdown, ScoringWeightComparison}` | 424 | **Reachable only from `src/test/java/.../ScoringWeightComparisonTest.java`.** Nothing in the CLI, GUI or API path constructs either. They also *disagree* with the live formula (Q6). Q6/Q7 |
| `gui/**`, `debug/DebugControl.java` (324) | — | Spec §2 |
| `io/specctra/parser/SessionToEagle.java` | 627 | Spec §2 drops Eagle `.scr`. Marker at `crates/fr-dsn/src/lib.rs:12` |
| `Freerouting.{initializeAPI :399-529, stopApiServer :532-540, initializeMCP :549-673, startMcpStdioBridge :681-788, splitCommaSeparated :790-795}` | 396 | Jetty/CORS/WebSocket plumbing. `startMcpStdioBridge` is rostered `// renamed:` — spec §13 replaces the bridge |
| `Freerouting.{compareBoardFiles :826-863, loadBoardFromFile :865-892}` | 66 | The undocumented `--compare-boards=a,b` mode (`GlobalSettings.java:530-537`), exits 0/1 at `:1295`. Not in spec §12 — Q5 |
| `settings/{ApiServerSettings, ApiAuthenticationSettings, RateLimitSettings, NetworkSettings, GoogleSheetsProviderSettings, GuiApplicationSettings, UserProfileSettings, UsageAndDiagnosticDataSettings, McpServerSettings}` | 260 | Already rostered by Plan 4; `crates/fr-settings/src/lib.rs:169` names `McpServerSettings` specifically |

### 1.4 NEW code with no Java counterpart

| what | why it is new | size est. |
|---|---|---|
| **The native stdio JSON-RPC MCP server** | Java's stdio mode is a bridge to HTTP (`Freerouting.java:681-788`) over a controller that implements only four methods and has **no progress and no cancellation at all**. Spec §13 requires a native newline-delimited JSON-RPC 2.0 server with `initialize`, `ping`, `tools/list`, `tools/call`, outbound `notifications/progress`, inbound `notifications/cancelled`. The skeleton exists (`crates/freerouting/src/mcp/{jsonrpc,server,stdio}.rs`, 285 lines) and **both files carry explicit Plan 8 obligations saying the current shape cannot express either** (`server.rs:9-32`, `stdio.rs:18-28`) | ~450 new |
| **The four MCP tools** `route_board`, `check_drc`, `board_info`, `list_settings` | Java's tools are OpenAPI-derived from `api/v1/**` and model a 7-call stateful pipeline (`create_session` → `enqueue_job` → `upload_job_input_file` → `update_job_settings` → `start_job` → poll `get_job_details` → `download_job_output_file`). **There is no Java analogue for `board_info` or `list_settings`, and no single-shot route tool** | ~300 new |
| **`schemars`-generated tool input schemas** | Plan 4 ruling 3 deliberately did **not** add `schemars`; `RouterSettings` derives `Serialize`/`Deserialize` but not `JsonSchema` (`docs/plan-4-handoff.md` §Plan 8). Spec §13: schemas generated "so CLI and MCP cannot drift" | ~80 new + a derive sweep |
| **`fr-core`'s `RoutingPipeline` wrapper, `Ctx`, `RoutingResult`** | Spec §10's `RoutingResult { board, stats, incompletes, drc_violations, timed_out }` and `Ctx { settings, cancel, progress, rng seed }` are a port-side composition over Plan 7's `run_pipeline`; Java has no such struct | ~200 new |
| **`CancelToken`** (spec §10: `Arc<AtomicBool>` + optional `Instant` deadline) | Plan 7 delivers `RouterStop { state: Cell<StopRequestState>, … }` — **`Cell` is not `Sync`**, so it cannot be shared with an MCP reader thread. §3.6, Q2 | ~120 new |
| **`freerouting info`** | Spec §12 lists it; Java has no `info` mode. Closest is `BoardStatistics.toString()` (`:589-591`) | ~100 new |

---

## 2. Call graph

### 2.1 `main()` → exit, as Java runs it

`main` is a Java 25 **instance** method (`Freerouting.java:899`, `void main(String[] args)`); `IO.print`/
`IO.println` are `java.lang.IO` (JEP 512). The only static initialiser of note is
`bridgeToken = UUID.randomUUID()` at `:74`.

```
Freerouting.main(args)                                            Freerouting.java:899
  originalSystemOut = System.out                                  :900
  isStdioMode ← argv "--mcp_server.stdio=true|1" or env FREEROUTING__MCP_SERVER__STDIO  :901-917
  if isStdioMode: System.setOut(System.err)      ← STDOUT PROTECTED FOR THE PROTOCOL    :918-920
  user-data path: default → env FREEROUTING__USER_DATA_PATH → env …LOGGING__FILE__LOCATION
      → --user_data_path=; mkdirs; warnings via System.err.println   :928-1001
  logging PRE-PARSE (a second, earlier parse of argv):             :1005-1066
      defaults file=DEBUG console=INFO; 6 env vars; --logging.{file,console}.*=,
      --debug.enable_detailed_logging= (→ file TRACE + FRLogger.granularTraceEnabled),
      -dl (exact match here, startsWith in GlobalSettings), -ll (FIRST occurrence wins here)
  resolveLogPath                                                   :1069-1077 → :797-824
  log4j2 system properties (incl. log4j2.disableJndi=true)         :1086-1099
  ((LoggerContext) LogManager.getContext(false)).reconfigure()     :1105
  FRLogger.traceEntry("MainApplication.main()")                    :1108
  UIManager.setLookAndFeel(system L&F)          ← Swing touch on every headless run    :1110-1117
  GlobalSettings.load()  (freerouting.json)                        :1130-1157  ← spec §2 drops
  stale-log-path warning; mcp_server.stdio-in-JSON warning         :1165-1203
  version-mismatch → fresh GlobalSettings + saveAsJson             :1205-1242  ← FILE WRITE
  applyNonRouterEnvironmentVariables()                             :1245
  runtimeEnvironment.{...}                                         :1259-1288  ← log only
  globalSettings.applyCommandLineArguments(args)   ← THE FLAG TABLE  :1291
  --compare-boards → compareBoardFiles; System.exit(success?0:1)   :1293-1296
  FRAnalytics.* (incl. an unconditional Thread.sleep(1000))        :1330-1385
  VersionChecker daemon thread                                     :1388-1391
  -help → IO.print(localised help); System.exit(0)                 :1394-1398
  drcReportFile != null → gui=false, api=false, mcp=false          :1401-1405
  settingsMergerProtype = SettingsMerger(Default, JsonFile, Cli(args), Env)   :1408-1413
  api / mcp / gui start                                            :1416-1455
  if (!gui.isEnabled && !api.isRunning && !mcp.isRunning):         :1458-1467
      drcReportFile != null ? initializeDrc(gs) : initializeCli(gs)
  if (!cliResult && !api.isEnabled && !mcp.isEnabled) → shutdown; System.exit(1)  :1469-1475
  while (gui||api||mcp running) sleep(500)                         :1477-1485
  shutdownApplication()                                            :1487
  if (!gui.isEnabled && !api.isRunning && !mcp.isRunning) → System.exit(cliExitCode)  :1489-1493
  System.exit(0)                                                   :1495
```

### 2.2 `-de <in.dsn> -do <out.ses>` — the route flow

```
applyCommandLineArguments                       GlobalSettings.java:521-838
  -de  → the slot classifier                    :564-648
  -do  → initialOutputFile                      :655-659
  -dr  → initialRulesFile                       :670-674
  -mp/-mt/-oit/-us/-is/-hr/-inc                 :675-736  (five write the dead bridge, quirk #131)
initializeCli(globalSettings)                   Freerouting.java:79-187
  guard: initialInputFile && initialOutputFile  :80-86       → false ⇒ return false ⇒ exit 1
  SessionManager.createSession(userId, "Freerouting/"+version)   :89-93
  RoutingJob job = new RoutingJob(cliSession.id)                 :96
  job.setInput(initialInputFile)                :101-106     ← reads bytes, sniffs FileFormat,
                                                                derives a default output name
  if (job.input == null) abort                  :108-112
  cliSession.addJob(job) → RoutingJobScheduler.enqueueJob         :114 → RoutingJobScheduler.java:303
      ← this is what first touches the scheduler singleton and starts its daemon poll thread
  DELETE the existing output file               :116-121     ← FILE DELETE (quirk I)
  job.tryToSetOutputFile(new File(initialOutputFile))  ← RETURN VALUE IGNORED   :123 (quirk L)
  merger = settingsMergerProtype.clone()
      + DsnFileSettings(job.input)              :125-127     ← MERGE #1
      + RulesFileSettings(job.rules)  if -dr    :129-144
  job.routerSettings = merger.merge()           :146
  job.drcSettings = globalSettings.drcSettings.clone()            :147
  job.state = READY_TO_START                    :148
  ── the scheduler daemon (250 ms poll) picks it up ─────────────────────────────
  RoutingJobScheduler.java:53-277
      board load: HeadlessBoardManager.loadFromSpecctraDsn / loadFromKiCadJson    :91-101
          → HeadlessBoardManager.applyParsedBoardResult :711-737
              → applyRouterSettingsForLoadedBoard :739-749
                    setLayerCount when it disagrees          :741-744
                    applyBoardSpecificOptimizations(board)   :745
                    applyCopperToEdgeClearanceOverride()     :746   ← BOARD MUTATION, unported (§5.7)
                    applyHoleClearanceOverride()             :747   ← BOARD MUTATION, unported
              → applyImmediatePostLoadProcessing :751-757
                    board.reduceNetsOfRouteItems()  ← ported in fr-board, never called by the port
                    validatePowerPlanes()           ← log only
              → scheduleDeferredPostLoadProcessing :759-784  ← diagnostic virtual thread, drop
      MERGE #2, incl. rules discovery + ApiSettings(job.routerSettings) at 70     :103-170
      RulesReader.read onto the merged board     :173-184
      applyBoardSpecificOptimizations(board)     :186
      optional session import                    :189-234
      state = RUNNING; job.thread.start()        :237-241
  RoutingJobSchedulerActionThread.threadAction   :37-206
      job.startedAt = Instant.now()                                :39
      timeout = TextManager.parseTimespanString(jobTimeoutString)  :44
          cap at MAX_TIMEOUT (24 h, :24); job.timeoutAt = startedAt + timeout   :45-51
      monitor thread: requestStop() then GRACE_PERIOD 30 s, then TIMED_OUT      :55-90
      routerEnabled = runRouter && (maxPasses == null || >= 0)     :92-94
      RoutingPipeline.createForHeadless(job)                       :99   → fr_router::run_pipeline
      addBoardUpdatedEventListener(e -> setJobOutput(job))         :100  ← SES WRITE-OUT per update
      pipeline.run()                                              RoutingPipeline.java:81-85
      setJobOutput(job)                                            :168  ← SES WRITE-OUT (final)
      job.finishedAt = Instant.now(); state finalisation           :174-184
  setJobOutput                                   :259-295
      job.output ?= new BoardFileDetails(job.board)                :261
      SES branch: new HeadlessBoardManager(job); replaceRoutingBoard(job.board)  :283-284
                  saveAsSpecctraSessionSes(baos, job.name)         :288 → SesWriter.write
                  job.output.setData(baos.toByteArray())           :289
  ── back in initializeCli, polling every 500 ms ──────────────────────────────
  while (!isCliTerminalState(job.state)) sleep(500)                :151-158
  outputWritten = writeCliOutputIfAvailable(gs, job)               :160 → :196-213
      requires job.output != null && state ∈ {COMPLETED, TIMED_OUT}
      Files.write(initialOutputFile, job.output.getData().readAllBytes())   ← FILE WRITE: SES
      returns Files.exists && Files.size > 0
  cliExitCode = computeCliExitCode(job, outputWritten)             :161 → :215-223
      0 iff (COMPLETED || TIMED_OUT) && outputWritten; else 1
  writeCliResultManifestIfRequested(...)                           :162 → :225-244
      only if routerSettings.resultJsonPath non-blank
      → RoutingResultManifest.fromJob(job, inputPath, outputWritten, exitCode)
      → RoutingResultManifest.write(Path.of(resultJsonPath), manifest)   ← FILE WRITE: manifest
  the donation banner → IO.println(...)                            :164-183 ← STDOUT (quirk H)
  globalSettings.cliExitCode = cliExitCode; return cliExitCode == 0  :185-186
```

**Where Plan 7's entry points slot in.** `RoutingPipeline.createForHeadless` + `run` is exactly
`fr_router::pipeline::run_pipeline(&mut board, &settings, &stop, budget, &mut progress) ->
Result<PipelineResult, RouterError>`. `fr-core`'s `RoutingPipeline::run` wraps it and adds: the
`CancelToken → RouterStop` adaptation, a `Sync` `ProgressSink`, `BoardStatistics`
(`fr_router::score`), the DRC violations and incompletes (`fr_drc::DesignRulesChecker`), and
`RoutingResult`. `batch_fanout`/`batch_autoroute`/`batch_optimize` are **not** called directly —
`run_pipeline` owns both stages including the fanout-only `maxPasses = 0` mode.

### 2.3 `-drc [report.json]` — the DRC flow

```
applyCommandLineArguments: startsWith("-drc")   GlobalSettings.java:660-669
  routerSettings.enabled = false                :662   ← dead bridge (quirk #131)
  drcSettings.enabled    = true                 :663
  if next arg exists and does not start with '-':
      drcReportFile = BoardFileDetails{format = DRC_JSON, filename = <arg>}   :664-668
  ** a BARE `-drc` leaves drcReportFile null **  ← and therefore never enters DRC mode
main:1401-1405   drcReportFile != null ⇒ gui/api/mcp disabled
main:1462-1463   drcReportFile != null ⇒ initializeDrc, else initializeCli
initializeDrc                                   Freerouting.java:246-374
  guard: initialInputFile != null               :247-250   → false ⇒ return false ⇒ exit 1
  SessionManager.createSession(...)             :253-257   (the job is NEVER enqueued)
  RoutingJob drcJob = new RoutingJob(...); drcJob.drc = globalSettings.drcReportFile   :260-261
  drcJob.setInput(initialInputFile)             :262-268   → catch ⇒ System.exit(1)  :267
  BoardLoader.loadBoardIfNeeded(drcJob)         :271-274   → false ⇒ System.exit(1)  :273
      BoardLoader.java:19-56 — DSN → loadFromSpecctraDsn; KICAD_DESIGN_JSON → loadFromKiCadJson;
                               anything else → error (so `-de prev.ses` fails here, quirk S)
  STEP 2 — if initialRulesFile != null:         :277-294
      RulesReader.read(stream, designName = drcJob.name ?? "board",
                       drcJob.board, drcJob.routerSettings)                   :284-285
      missing file → warn :289; exception → error :291-293, DRC CONTINUES
  STEP 3 — if designSessionFilename != null:    :296-329   ← AFTER the rules, deliberately
      .json → KiCadJsonReader.importSession(new FileReader(f), drcJob.board)  :301-307
              (FileReader = PLATFORM DEFAULT CHARSET, quirk U)
      else  → SesReader.read(new FileInputStream(f), drcJob.board)            :309-322
      missing → warn :324; exception → error :326-328, DRC CONTINUES
  checker = new DesignRulesChecker(board, globalSettings.drcSettings)         :332-333
      (note: drcJob.drcSettings is never assigned here, unlike initializeCli:147)
  coordinateUnit = "mm"                         :336   ← hard-coded (quirk #151)
  sourceFileName = new File(initialInputFile).getName()                       :339
  report = checker.generateReport(sourceFileName, coordinateUnit)             :340
  quality score:                                :342-352   ← A SECOND, DIFFERENT MERGE
      merger = settingsMergerProtype.clone() + DsnFileSettings(drcJob.input)  :344-346
      routerSettings = merger.merge()           (no rules, no board pass, no merge #2)
      report.qualityScore = (double) drcJob.board.getStatistics()
                                     .getNormalizedScore(routerSettings.scoring)   :348-349
      failure → warn, qualityScore stays null   :350-352
  json = GsonProvider.GSON.toJson(report)       :354   (pretty-printed, HTML escaping off)
  if (drcJob.drc != null) Files.write(getAbsolutePath(), json.getBytes(UTF_8))  :356-368
      on IOException → System.exit(1)           :366
  else IO.println(json)                         :368-371  ← UNREACHABLE FROM THE CLI (quirk A)
  return true                                   :373      ← ALWAYS ⇒ -drc always exits 0 (quirk B)
```

**Three consequences Plan 8 must decide on, not inherit blindly.**
1. The stdout branch `:368-371` is dead from the command line: DRC mode is entered only when
   `drcReportFile != null` (`main:1462`), and `drcJob.drc` is that same object. Spec §12 says
   "`drc` and `info` write JSON to stdout when no `-o`" — so the port **is** more capable. Note
   `crates/freerouting/src/legacy.rs:120` currently makes `-drc` *require* a value, which matches
   Java's reachable behaviour but not spec §12.
2. `-drc` **always exits 0**: a missing `.rules` file (`:289`), a missing session file (`:324`), a
   failed quality score (`:350`) and any number of violations all leave the code at its `0` default
   (`GlobalSettings.java:122`). Only `:267`, `:273` and `:366` produce 1.
3. The quality-score merge at `:342-352` is **not** `resolve_headless`: merge #1 with only the DSN
   source, no `.rules`, no board pass, no merge #2. Reproducing it means calling `SettingsMerger`
   directly. Same shape as the API-path obligation at `crates/fr-settings/src/resolve.rs:184`.

### 2.4 `-dr <file.rules>`

Not a mode — a slot. `GlobalSettings.java:670-674`, matched **after** `-drc` on purpose (both are
`startsWith`, `:660` precedes `:670`). It feeds three different readers:
- route merge #1 as `RulesFileSettings` (`Freerouting.java:129-144`), priority 40, layer structure
  discovered *from the file* (`RulesReader.readRouterSettings`);
- the scheduler's merge #2 and post-merge re-apply (`RoutingJobScheduler.java:113-186`), layer
  structure from the **board** (`RulesReader.read`). Quirk #142 — two parses of the same bytes,
  which is why `SettingsInputs` takes byte slices;
- the DRC flow's step 2 (`Freerouting.java:277-294`), a direct `RulesReader.read` onto the board.

`-dr` with a **non-existent** path silently disables the scheduler's adjacent-`<design>.rules`
auto-discovery (`RoutingJobScheduler.java:132-152`, an `else if` chain) — quirk V.

### 2.5 `freerouting mcp` — what the port must build, and what Java actually does

Java: `main:1437-1445` starts Jetty (`initializeMCP:549-673`), then `startMcpStdioBridge:681-788`
spawns a daemon thread that:
1. polls every 50 ms until `server.getConnectors()[0].getLocalPort() > 0` (`:688-702`);
2. resolves a profile id from `FREEROUTING_PROFILE_ID` / `FREEROUTING__PROFILE__ID` /
   `userProfileSettings.userId`, defaulting to `"00000000-0000-0000-0000-000000000000"` (`:704-716`);
3. `POST`s each non-blank stdin line to `http://127.0.0.1:<port>/v1/mcp` with `Content-Type:
   application/json`, `X-Internal-Bridge-Token: <the UUID from :74>`, `Freerouting-Profile-ID`
   (**required — every method including `initialize` is authenticated**, `BaseController:70-111`),
   optionally `-Email` and `-Environment-Host` (`:735-762`);
4. prints `response.body().replace("\r","").replace("\n","")` to `originalSystemOut` + flush
   (`:764-771`);
5. EOF → `System.exit(0)` (`:777-779`); stdin `IOException` → `System.exit(1)` (`:781-782`).

The protocol behind it (`McpControllerV1.java:189-198`):

| what | Java | spec §13 / the port |
|---|---|---|
| methods | `initialize` `:190`, `notifications/initialized` `:191-194`, `tools/list` `:195`, `tools/call` `:196`; everything else → `-32601` `:197` | + `ping`, + outbound `notifications/progress`, + inbound `notifications/cancelled` |
| `protocolVersion` | `"2024-11-05"` (`:285`, echoed at `AgentCardController.java:66`) | the skeleton already says `"2025-06-18"` (`mcp/server.rs:5`) |
| `serverInfo` | `{name: "Freerouting MCP", version: Constants.FREEROUTING_VERSION}` `:281-282`, **plus** non-spec top-level `serverName`/`serverVersion` `:286-287` | `{name: "freerouting", version: CARGO_PKG_VERSION}` today — Q6 |
| `capabilities` | `{"tools":{}}` only `:277-278` | `{"tools":{"listChanged":false}}` today |
| tools | ~29: 25 auto-derived from the Swagger scan of `/v1/*` (`OpenApiMcpToolRegistry.java:54-118`, filter `:339-341`, **regenerated on every request**) + 4 hand-written (`encode_base64` `:146-159`, `decode_base64` `:188-201`, `upload_job_input_from_local_file` `:238-251`, `download_job_output_to_local_file` `:288-301`) | 4, flat-argument |
| tool input shape | a three-bucket wrapper `{path:{}, query:{}, body:{}}` (`buildInputSchema :474-551`) | flat |
| tool output shape | every generated tool declares the *same* `{status, contentType, body}` (`buildSuccessSchema :553-586`); the call result stringifies it into one text block (`:333-356`) with `isError = status >= 400` — **no `structuredContent`** | the skeleton already emits `structuredContent` (`mcp/server.rs:104-121`) |
| progress / cancel | **none**. `tools/call` is a blocking `HttpClient.send` with no timeout (`:359-382`); the only push is `McpRealtimeBridge.broadcast` to SSE/WS subscribers (`:341-344`), which a stdio client never sees | `notifications/progress` + `notifications/cancelled` |

The port replaces steps 1–3 with an in-process dispatch. **Keep from Java:** stdout reserved for
the protocol (Java achieves it with `System.setOut(System.err)` at `:918-920`; the port's
`main.rs:38-41` already writes `tracing` to stderr unconditionally, which is stronger) and one
JSON object per line, no Content-Length framing.

### 2.6 Every file write, its format and its parity jar

| # | write | producer | format | parity target |
|---|---|---|---|---|
| 1 | `-do <out.ses>` | `SesWriter.write` via `HeadlessBoardManager:865` → `job.output.setData` → `Files.write` (`Freerouting.java:207`) | Specctra SES, UTF-8 (`IndentFileWriter`), numbers pinned to `Locale.ENGLISH` (`SesWriter.java:262,264`) | **2.3.0 jar** for writer *syntax* (`tests/reference/README.md` §"Why the 2.3.0 jar"); **HEAD jar** for routed *content* (`gen-batch-reference.sh`, Plan 7 T16 → `tests/reference/<stem>/batch.ses`) |
| 2 | `-do <out.json>` | `KiCadJsonWriter.write` (`RoutingJobSchedulerActionThread.java:277-278`), UTF-8 | KiCad session JSON | HEAD jar — **and see quirk T: the final board is never written on this path** |
| 3 | `-do <out.dsn>` / `<out.scr>` | accepted by `tryToSetOutputFile:384-388` but `setJobOutput` serialises only `KICAD_SESSION_JSON` and `SES`, so `output.getData()` stays empty → 0 bytes written, `writeCliOutputIfAvailable` returns false → **exit 1 with an empty file left behind** | — | quirk L; already recorded in `tests/reference/README.md` |
| 4 | `-drc <report.json>` | `GsonProvider.GSON.toJson(KiCadDrcReport)` → `Files.write` UTF-8 (`Freerouting.java:362`) | KiCad DRC JSON, pretty-printed 2-space | **HEAD jar** (`scripts/gen-drc-reference.sh`); HEAD writes camelCase, 2.3.0 snake_case — plan-5 ruling W ships `DrcJsonFlavor::KiCad` as the *CLI* default while `DrcJsonFlavor::default()` stays `FreeroutingHead`, the *parity* default |
| 5 | `--router.result_json=<f>` | `RoutingResultManifest.write` (`:138-144`), Gson pretty, UTF-8, `createDirectories(parent)`, **no trailing newline** | manifest JSON | **HEAD jar only** (2.3.0 has no manifest) |
| 6 | `freerouting.log` | log4j2 File appender, `immediateFlush`, `bufferedIO`, **no rotation** (`Log4j2ConfigurationFactory.java:64-85`) | text | **not a parity surface** — spec §2 |
| 7 | `freerouting.json` | `GlobalSettings.saveAsJson` (`main:1221`), chmod `rw-------` | settings JSON | **not ported** — spec §2 |
| 8 | stdout: the donation banner | `IO.println` (`Freerouting.java:167-183`), gated on the **persisted** `statistics.jobsCompleted >= 5` | text | quirk H — must **not** be reproduced |
| 9 | stdout: the DRC report | `IO.println` (`:370`) | JSON | unreachable in Java; **the port's spec-§12 default when `-o` is absent** |
| 10 | stdout: the help text | `IO.print(ctm.getText("command_line_help"))` (`main:1396`), **localised** | text | clap generates the port's; not byte-parity |
| 11 | stdout: **the INFO log stream** | log4j2 Console appender targets `SYSTEM_OUT` (`Log4j2ConfigurationFactory.java:58`); ERROR is additionally duplicated to `SYSTEM_ERR` (`:88-96, 112-113`) | text | **the port sends all logs to stderr** — a deliberate divergence that the `p8t1` log normaliser must account for |
| 12 | `<userdata>/data/…/FRJ_*.json` | `RoutingJobScheduler.saveJobToDisk :379-452` | job JSON | **feature-flagged off and unreachable from the CLI** |

---

## 3. Determinism / parity analysis

### 3.1 Timestamps and identity that reach an OUTPUT FILE

| value | Java site | reaches | port answer |
|---|---|---|---|
| `Instant.now().toString()` | `RoutingResultManifest.java:101` → `generated_at` | **manifest JSON** | inject a clock; the driver pins a fixed instant |
| `Duration.between(startedAt, finishedAt)` | `RoutingResultManifest.java:129-131` → `phases.autorouter.duration_seconds` (**the whole job's** duration, quirk E) | **manifest JSON** | normalise out of every byte-diff |
| `git_sha` | `RoutingResultManifest.java:147-161` — env `FREEROUTING_GIT_SHA`, then `-Dfreerouting.git.sha`, then `-DFREEROUTING_GIT_SHA`, else `"unknown"`, each `.trim()`ed | **manifest JSON** | reproduce the three-step lookup verbatim |
| `app_version` = `Constants.FREEROUTING_VERSION` (`"2.3.1-SNAPSHOT"` today) | `:102` | **manifest JSON** | Q6 |
| `sha256` of the input, lower-case `HexFormat` | `:163-171`; **returns `null` on any failure, and Gson then omits the key** (quirk W) | **manifest JSON** | port verbatim including the omission |
| `resource_usage.{cpu_time, max_memory, peak_memory}` | `RoutingJobSchedulerActionThread.java:208-257`, 1 s MXBean polling; `io_read`/`io_written` never written | **manifest JSON** | normalise out, or omit the monitor entirely |
| `final_state` / `exit_code` flipping on wall clock | `:71`, `:84` (monitor) → `RoutingResultManifest.java:111-112` | **manifest JSON** | `RouterBudget::disabled()` on both sides of every driver |
| `settings_snapshot` = the whole `RouterSettings` through Gson | `:110` + `RouterSettingsTypeAdapterFactory` | **manifest JSON** | `fr_settings::json` already owns it; quirk #141 governs the strictness split |
| `board_statistics` + `normalized_score` — a **full recompute** including two `DesignRulesChecker` runs | `:117-120` → `BoardStatistics.java:110-427` | **manifest JSON** | `fr_router::score::BoardStatistics::new` |
| `ZonedDateTime.now().format(ISO_OFFSET_DATE_TIME)` | `io/kicad/KiCadDrcReport.java:70` → `date` | **DRC JSON** | plan-5 already flagged "inject a clock; `fr-drc` has none" |
| `"Freerouting " + Constants.FREEROUTING_VERSION` | `drc/DesignRulesChecker.java:213` → `freerouting_version` | **DRC JSON** | Q6 |
| `float → double` widening of the quality score | `Freerouting.java:349` — serialises as e.g. `999.9000244140625` | **DRC JSON** | keep the `f32` computation and widen at the boundary, exactly as plan-5 records |
| `Session.id`, `RoutingJob.id`, `createdAt`, `bridgeToken` (all `UUID.randomUUID()` / `Instant.now()`) | `Session.java:12`, `RoutingJob.java:39,43`, `Freerouting.java:74` | **log lines and the server-only job JSON only** — *not* the manifest, *not* the SES | keep out of output |

**The routed `.ses` itself carries no timestamp, UUID, hostname or username.** No
`InetAddress.getLocalHost`, no `System.getenv("USER")`, no `user.name` anywhere on these paths.
Its non-determinism is entirely the router's plus the wall-clock budgets.

### 3.2 Locale

- `main` never calls `Locale.setDefault`; **there is no `Locale.setDefault` anywhere in the tree.**
  It only *reads* the default into `runtimeEnvironment.systemLanguage` (`:1267-1268`) and passes it
  to `FRAnalytics.appStarted` (`:1375`) — log/analytics only.
- `-l <locale>` (`GlobalSettings.java:737-798`, the **only** flag matched with `equals`) sets
  `currentLocale` through a 27-entry lower-cased, `-`→`_`, **prefix** chain; an unmatched value
  silently leaves the previous locale. It drives only the GUI's `TextManager` and the help text
  (`main:1394-1396`). The port drops `-l` (`legacy.rs:135`) — correct.
- **The locale hazard that reaches output is quirk #145**: every `%.4f` in the DRC report goes
  through one-argument `String.format`/`.formatted`
  (`DesignRulesChecker.java:336-343, 346-353, 489`), i.e. `Locale.getDefault(Locale.Category.FORMAT)`,
  so a `de_DE` JVM writes `0,0500` into a KiCad-schema JSON document. Every parity driver already
  runs `-Duser.language=en -Duser.country=US`; **Plan 8's end-to-end drivers must too**, and the
  port emits `en_US` formatting unconditionally.
- The SES writer pins `Locale.ENGLISH` (`SesWriter.java:262,264`), `FRLogger` pins `Locale.US`
  (`FRLogger.java:24-27`), and the summary log line pins `Locale.US`
  (`RoutingJobSchedulerActionThread.java:141`). The **only** unpinned formatters are the DRC `%.4f`
  above and `BoardScoreBreakdown`/`ScoringWeightComparison` (dead, §1.3).

### 3.3 Exit-code semantics

| code | condition | Java site |
|---|---|---|
| 0 | route: `state ∈ {COMPLETED, TIMED_OUT}` **and** the output file exists with size > 0 | `computeCliExitCode:215-223` → `globalSettings.cliExitCode` → `main:1493` |
| 0 | drc: **always** (`initializeDrc` returns `true` at `:373`; `cliExitCode` stays at its `0` default) | `GlobalSettings.java:122` |
| 0 | `--help` | `main:1397` |
| 0/1 | `--compare-boards=a,b` | `main:1295` |
| 1 | route: any other terminal state, or the output was not written | `computeCliExitCode:222` |
| 1 | `initializeCli` guard failures (no input / no output / unreadable input) → `cliResult=false` → `main:1474` | `:80-86`, `:108-112` |
| 1 | drc hard exits: input unreadable `:267`, board load failed `:273`, report write failed `:366` — raw `System.exit(1)`, bypassing `shutdownApplication()` | |
| 0 | MCP stdio bridge stdin EOF (from a daemon thread, bypassing shutdown) | `:779` |
| 1 | MCP stdio bridge stdin `IOException` | `:782` |
| **∞** | **`isCliTerminalState` omits `INVALID`** (`:189-194`), and the scheduler sets `INVALID` for a null input or a non-DSN/JSON input (`RoutingJobScheduler.java:83`, `:253`) — the `:151-158` loop then spins at 500 ms **forever**. `-de x.ses -do y.ses` is the repro | quirk N |

Spec §12 states "0 iff COMPLETED or TIMED_OUT and output written; 1 else" — matches. The port's
current placeholders (`EXIT_NOT_IMPLEMENTED = 3` at `commands/mod.rs:6`, exit `2` on a legacy
rewrite error at `main.rs:16`) exist in no Java version — Q4.

### 3.4 The settings precedence chain — what the CLI must reproduce vs what `fr-settings` has

`fr_settings::resolve_headless(&SettingsInputs, Option<&Board>, &HostEnvironment)` already
linearises Java's **whole** two-merge headless ladder including both board passes, the private
`boardSpecificTraceCostsApplied` flag, the post-merge `.rules` re-apply and both `validate()`
calls (`crates/fr-settings/src/resolve.rs`, module docs, verified against `p4t1`'s 64-case matrix).
**Plan 8's CLI does not re-derive precedence — it fills `SettingsInputs` and calls it once.**
What remains:

| quirk | what it is | who owns it |
|---|---|---|
| **#140** | `validate()` is not idempotent: `max_passes == 0` → `Integer.MAX_VALUE` on the first call, `9999` on the second, and the headless path calls it twice (`Freerouting.java:146`, `RoutingJobScheduler.java:170`). | **`resolve_headless` reproduces it** (module docs §3). Plan 8 must not call `validate()` a third time, and must keep `max_passes == 0` meaning *unlimited* on the shim path |
| **#141** | Gson `Strictness.LENIENT` governs only the textual pass; the value conversion is strict and non-finite floats are refused both ways. | `fr_settings::json` reproduces it. Plan 8 inherits it for `settings_snapshot` **and for the MCP `settings` input object** (`json.rs:86` names that reader) |
| **#142** | The `.rules` file is parsed **twice** against two different layer structures. | `SettingsInputs::{cli_rules, scheduler_rules}` take **byte slices**. **Plan 8 must feed `std::fs::read(path)`, not a parsed object** — plan-4 hand-off §Plan 8 says so explicitly |
| **#143** | Neither `optimizer.max_threads` nor `router.max_threads` is read anywhere on the headless path. | Plan 4 stored both with distinct normalisations; Plan 7 confirmed no rayon. **`--threads` / `-mt` must stay inert** — Q3 |
| **#131** | `-oit`, `-us`, `-is`, `-hr`, `-inc` and `-drc`'s router switch write a bridge nothing reads. | `LegacyBridge` exists and is dead on purpose — Q3 |
| **#132–#137** | `-mt` two fields two clamps; prefix matching on the legacy table vs exact on `CliSettings`; `Integer.decode` vs `parseInt`; the `-oit` negative clamp unreachable; `--router.enabled=` reads as `false`; `-de` drops unknown extensions. | all in `fr_settings::sources::cli`. **`crates/freerouting/src/legacy.rs` still reproduces the `-de` shape itself and forwards raw values** — the one-call rewire to `classify_de_arguments` + `apply_command_line_arguments` is Plan 8's (`cli.rs:559`, `:582`) |
| **API path** | `resolve_headless`'s premise (merge #1's result is complete) fails for a job with a *sparse* priority-70 payload. **The MCP `settings?` argument of `route_board` is exactly that shape.** | `obligation:` at `crates/fr-settings/src/resolve.rs:184` — compose with `SettingsMerger` directly |
| **DRC quality score** | `initializeDrc:342-352` runs merge #1 with **only** the DSN source. | a third composition; `SettingsMerger` directly |
| **the pre-bootstrap parse** | Logging config and stdio mode are parsed **twice** — once raw before log4j init (`Freerouting.java:901-917`, `:1032-1065`), once in `applyCommandLineArguments`. The early pass is what takes effect; `-ll` takes the **first** occurrence there and the **last** in `GlobalSettings.java:825-830`. | port-side: one parse, documented divergence |

### 3.5 Hash-ordered / non-reproducible things that reach output

- **#144** — `DesignRulesChecker.getAllUnconnectedItems` iterates a `HashMap<Integer, List<Item>>`
  (`:95`, iterated `:104`) and identity-hashed `HashSet<Item>`s (`:114`, `:123`), so the DRC
  report's `unconnected_items` array order is not reproducible run to run in Java. `fr-drc` handles
  it; every parity run pins `-XX:hashCode=2`. **Plan 8's `-drc` driver must carry the same flags.**
- **#147 / #148** — `NetIncompletes.Edge.compareTo` non-injective; `AirLine.compareTo` compares the
  net name only. Already reproduced.
- `AutorouteUnroutedReport`'s `LinkedHashMap` is insertion-ordered by `getAllAirlines()` — Plan 7
  ruling 5 keeps a `Vec<(String, Vec<String>)>`.
- Gson serialises in **declaration order**, so the manifest's key order is
  `schema_version, generated_at, app_version, git_sha, fixture, settings_snapshot, phases,
  board_statistics, normalized_score, resource_usage, final_state, exit_code, output_written`, and
  **nulls are omitted**. `serde_json` over a struct matches — but any `HashMap` in the port's
  manifest would break it. Ordered structs or `BTreeMap` only.
- `BoardStatisticsBends`' JSON keys `"90_degree_count"` / `"45_degree_count"` begin with digits —
  legal JSON, needing an explicit `#[serde(rename)]`. `BoardStatisticsItems.componentOutlineCount`
  serialises as `"component_count"` (a name collision with `components.total_count`).

### 3.6 Threading and cancellation — the structural gap

Spec §10 wants `CancelToken: Arc<AtomicBool> + Option<Instant>`. Plan 7 Task 4 delivers
`RouterStop { state: Cell<StopRequestState>, deadline: Option<TimeLimit> }` — **`Cell` is not
`Sync`**, so a `RouterStop` cannot be shared with an MCP reader thread. Independently,
`crates/freerouting/src/mcp/stdio.rs:18-28` and `mcp/server.rs:9-32` state in the tree today that
the sequential read-handle-write loop must be replaced by a reader thread + `Mutex`-guarded writer
before cancellation or progress can work at all.

Options, in increasing cost: **(a)** an `Arc<AtomicBool>` external flag that the routing thread
polls and copies into its thread-local `RouterStop` at the six existing `poll_deadline` sites —
Plan 7's interface untouched; **(b)** reopen `pipeline/stop.rs` and make the field an `AtomicU8`;
**(c)** confine routing to one thread and accept that `notifications/cancelled` only takes effect
between requests, i.e. never. The task table assumes **(a)**. Q2.

Java's own answer is (c): its `tools/call` is a blocking `HttpClient.send` with no timeout and no
protocol cancellation at all; cancellation is a *tool* (`cancel_job` → `PUT /v1/jobs/{id}/cancel`).

### 3.7 What the end-to-end drivers need (`p8t*`)

Unlike `p7t*` (which reflect into jar internals), Plan 8's acceptance is **the two binaries'
observable behaviour**: invoke the jar and `freerouting-rs` with the *same* argv on the same
fixture and diff everything.

| driver | invocation | compares |
|---|---|---|
| `p8t1` | `-de <dsn> -do <ses>` vs `route <dsn> -o <ses>` | SES **bytes**, exit code, normalised stderr |
| `p8t2` | `… --router.result_json=<f>` | manifest JSON with `generated_at`, `duration_seconds`, `git_sha`, `resource_usage` normalised out |
| `p8t3` | `-de "<dsn>+<ses>+<rules>" -drc <report.json>` | DRC JSON byte-identical after `date` normalisation, plus the exit code — exercising the **DSN → .rules → SES load order** |
| `p8t4` | the flag matrix from `docs/cli-legacy-flags.md` × values that trip each normalisation, dumping the resolved `RouterSettings` as JSON from both sides | settings JSON field for field (`p4t1` re-run through the *binary*) |
| `p8t5` | argv shapes: `-de a.dsn b.rules`, `-de "a.dsn+b.ses"`, `-mpx 5`, `-de a.txt`, bare `-drc`, missing values, `--router.enabled=`, `-oit -5`, unknown flags | slot classification + exit code, not routing |
| `p8t6` | MCP over spawned pipes: `mcp` vs `--mcp_server.stdio=true`, driving `initialize` + `tools/list` + `tools/call` | **shape only, a documented-delta driver** — Java's tool list is OpenAPI-derived and cannot match (Q1) |
| `p8t7` | KiCad end-to-end: KiCad-exported DSN → route → SES → re-read by `fr_dsn::ses_reader::read` | spec §1's acceptance case |

**Normalisation the drivers need** (new `tests/parity` helpers): strip `generated_at`,
`phases.*.duration_seconds`, `git_sha`, `resource_usage`, `date`, absolute paths; strip the log
prefix (`%d{yyyy-MM-dd HH:mm:ss.SSS} %-6level `, `Log4j2ConfigurationFactory.java:29`) and account
for Java putting INFO on **stdout** where the port puts it on stderr;
`run_jar(argv)` / `run_port(argv) -> (stdout, stderr, code)`. `scripts/normalize-drc.py` already
exists for the DRC document.

---

## 4. Java bugs / quirks candidates (labels only; ids allocated later)

Ordered roughly by how much they change Plan 8's design.

| label | what | file:line |
|---|---|---|
| **A** | **The `-drc` stdout branch is unreachable.** DRC mode is entered only when `drcReportFile != null` (`main:1462`) and `drcJob.drc` is that same object, so `IO.println(drcReportJson)` can never run from a command line. A **bare `-drc` is not DRC mode at all**: it sets the two dead booleans and falls through to `initializeCli`, which dies with "Both an input file and an output file must be specified". | `Freerouting.java:368-371` with `:1462`, `GlobalSettings.java:660-669` |
| **B** | **`initializeDrc` returns `true` unconditionally**, so `-drc` exits 0 whatever happens: a missing `.rules` file, a missing session file and a failed quality score all only warn, and the violation count never affects the code. | `Freerouting.java:289`, `:324`, `:350`, `:373` |
| **C** | **The DRC quality score uses a different settings merge from the router's** — merge #1 with only `DsnFileSettings`, no `.rules`, no board pass, no merge #2. So `-dr x.rules -drc r.json` scores with weights the rules file never influenced while the *violations* were computed against the clearances it installed. | `Freerouting.java:342-352` vs `:277-294` |
| **D** | **The DRC session import happens after the `.rules` re-parse**, so the session's wires and vias are created and then checked against the clearances the `.rules` file installed. Swapping the two steps changes the violation list. (Recorded by plan-5; restated because it is Plan 8's to reproduce.) | `Freerouting.java:277-294` then `:296-329` |
| **E** | **`RoutingResultManifest.phases.fanout` and `.optimizer` are allocated and never written** — always `{}` — while `phases.autorouter.duration_seconds` is the **whole job's** duration. `isFanoutTimedOut()` / `isTimedOut()` are available three lines away (`RoutingJobSchedulerActionThread.java:170-172`) and reach only a log line. | `RoutingResultManifest.java:79`, `:85`, `:124-132` |
| **F** | **`BoardStatistics(byte[], DSN)` counts substrings**: `(layer` also counts `(layer_rule`, `(net` also counts `(network` and `(net_class`, `(via` also counts `(via_rule`, `(class` also counts `(class_class`. The SES branch's layer extraction splits on `"(path "` and takes `words[0]` of each chunk after the first. | `BoardStatistics.java:469-473`, `:514-519`, `countOccurrences :578-586` |
| **G** | **The DSN host scrape almost always fails.** `searchLimit` is the **first `)` after `(parser`** — in a real DSN that is the end of the first inner clause (e.g. `(string_quote ")`), so `(hostCad`/`(hostVersion` lie outside `parserScope` and `host` stays `null`. Compounded: the keywords are HEAD's **camelCase** (quirk #92's family), so a correct Specctra DSN with `(host_cad` never matches, and `substring(hcIdx+9, …)` / `(hvIdx+13, …)` hard-code exactly one space. | `BoardStatistics.java:480-503` |
| **H** | **`initializeCli` prints a donation banner to stdout** when the output was written, the *persisted* `statistics.jobsCompleted >= 5` and the user e-mail is empty. Un-suppressible, and it would corrupt any stdout-JSON mode. | `Freerouting.java:164-183` |
| **I** | **The output file is deleted before routing starts** and only warns on failure; a failed or hung run destroys the previous result and writes nothing. | `Freerouting.java:116-121` |
| **J** | **Two nested polling loops** (`initializeCli` 500 ms + the scheduler 250 ms) add ~0.75 s of latency to every single-shot run, and quantise the manifest's `duration_seconds`. | `Freerouting.java:151-158`, `RoutingJobScheduler.java:268` |
| **K** | **The SES write-out runs on every board-updated event and again at the end**, each time re-serialising the whole board, recomputing CRC32 and running a full text-scrape `BoardStatistics`. | `RoutingJobSchedulerActionThread.java:100`, `:168`; `BoardFileDetails.java:107-116` |
| **L** | **`tryToSetOutputFile`'s return value is discarded** (`Freerouting.java:123`). `-do out.txt` returns `false`, `job.output` keeps the input-derived `<input>.ses` details, and SES bytes are written to `out.txt` anyway. `-do out.dsn` / `out.scr` is accepted by the validator but `setJobOutput` serialises neither, so a **0-byte file is written and the run exits 1**. | `RoutingJob.java:377-397`, `RoutingJobSchedulerActionThread.java:275-293`, `Freerouting.java:196-213` |
| **M** | **The MCP stdio bridge strips every `\r` and `\n` from the response body** before printing. Safe for compact JSON, silently corrupting otherwise. | `Freerouting.java:766` |
| **N** | **`isCliTerminalState` omits `INVALID`**, and the scheduler assigns `INVALID` for a null or non-DSN/JSON input — so `-de x.ses -do y.ses` makes the CLI **spin forever**. | `Freerouting.java:189-194` with `RoutingJobScheduler.java:83`, `:253` |
| **O** | **`getFileFormat(byte[])`'s leading-newline shift loop never refills `buffer[5]`** — a file starting with ≥ 6 CR/LF bytes spins forever. The comment says `0x0A or 0x13`; the code tests `0x0A`/`0x0D`. | `RoutingJob.java:181-187` |
| **P** | **`tryToSetOutputFile` registers the *output* listener as `fireInputUpdatedEvent()`** — a copy-paste bug; `setInputFromFile:440,446,452` gets it right. | `RoutingJob.java:390` |
| **Q** | **`changeFileExtension` NPEs on a bare filename** (`filePath.getParent().toAbsolutePath()`), and when the extension already matches it returns the *original, possibly relative* string rather than the reconstructed absolute path — asymmetric with its two other returns. | `RoutingJob.java:352-374` |
| **R** | **`BoardFileDetails.setFilename`'s Windows-only string surgery runs unconditionally**, and `replaceAll("\\\\.$","")` is a regex bug: `\\.` is "backslash then any char", so it strips the last two characters of any path ending in backslash-plus-anything. `filename.contains(File.separator)` is platform-dependent. | `BoardFileDetails.java:158-166` |
| **S** | **`BoardLoader` accepts only DSN and KiCad design JSON**, so `-de prev.ses -drc r.json` fails with "only DSN and JSON formats are supported" rather than at the argument. | `BoardLoader.java:32-37` |
| **T** | **The KiCad JSON output path never writes the final board.** `setJobOutput` sets `format = KICAD_SESSION_JSON` (`:266`), then `setData` re-sniffs the bytes (`BoardFileDetails.java:113`) and, because JSON starts with `{`, resets it to `KICAD_DESIGN_JSON` (`RoutingJob.java:160-161`). On the second call (`:168`, after `pipeline.run()`) **neither branch matches**, so the output is whatever the last mid-run board-updated event produced. The SES path escapes this because `(ses` re-detects as `SES`. | `RoutingJobSchedulerActionThread.java:259-295` with `BoardFileDetails.java:113` |
| **U** | **`new FileReader(sessionFile)` uses the platform default charset** for a JSON input, while every other JSON path is explicit UTF-8. | `Freerouting.java:304`, `RoutingJobScheduler.java:202` |
| **V** | **`-dr` with a non-existent path silently disables adjacent-`<design>.rules` discovery** — the scheduler's `else if` chain takes the `initialRulesFile != null` branch and the inner `exists()` check then fails. | `RoutingJobScheduler.java:132-152` |
| **W** | **`sha256Hex` returns `null` on failure and Gson omits the key**, so an unreadable fixture is indistinguishable from a missing field. | `RoutingResultManifest.java:163-171` |
| **X** | **`BoardScoreBreakdown` and the live `calculateScore` disagree by the DSN resolution factor**: `of` reads `traces.totalLength` (raw board units) while its own javadoc says millimetres and `calculateScore:608-609` prefers `totalLengthMm`. Both classes are dead in `main/`. | `BoardScoreBreakdown.java:140` vs `BoardStatistics.java:608-609` |
| **Y** | **The timeout monitor thread never exits** — `while ((job != null) && (job.thread != null))` and `job.thread` is never nulled — so it keeps sampling MXBeans for the process lifetime; its `catch (Throwable t) { }` at `:253-256` is completely silent. | `RoutingJobSchedulerActionThread.java:58`, `:253-256` |
| **Z** | **`Session`'s constructor mutates before it validates**: it assigns `this.host` at `:34` and only then checks `host.split("/").length != 2` at `:37`, throwing from a partially-constructed object; `SessionManager.setPrimarySession` does the same (`:157` before `:162`). | `Session.java:30-42`, `SessionManager.java:155-170` |
| **AA** | **`enqueueJob` reads `session.userId` purely to validate it and never uses it** — the one line that makes `SessionManager` a hard dependency of the CLI (§8). | `RoutingJobScheduler.java:315-318` |
| **AB** | **`RoutingJobScheduler` reads its `LinkedList` outside the lock** at `:57` and `:74-78` while other methods mutate it under `synchronized (jobs)` — hence the defensive `removeIf(Objects::isNull)` at `:62`. The whole scheduler starts as a side effect of static field initialisation (`:43`). | `RoutingJobScheduler.java:43`, `:57`, `:62`, `:74-78` |
| **AC** | **The `applyCopperToEdgeClearanceOverride` early return makes the same numeric setting behave differently depending on provenance**: when the value equals `DEFAULT_COPPER_TO_EDGE_CLEARANCE_UM` (500.0) **and** the outline does not use the fallback AREA class, the override is skipped — so a *defaulted* 500 and an *explicitly requested* 500 diverge. | `HeadlessBoardManager.java:501-507` |
| **AD** | **The two clearance overrides run twice on a DSN load** — once from `createBoard` (invoked by the parser at `Structure.java:1268`) and again from `applyRouterSettingsForLoadedBoard:746-747` — and each can re-trigger `searchTreeManager.reinsertTreeItems()`. | `HeadlessBoardManager.java:342-343` vs `:746-747` |
| **AE** | **The deferred post-load virtual thread races the router**: `BoardStatistics(loadedBoard, …)` at `:770` and a full DSN re-serialisation at `:778` read the board while the router mutates it, and `compareCounterpartBoardIfPresent` loads a **second whole board from disk** to log a diff. | `HeadlessBoardManager.java:759-784`, `:172-237` |
| **AF** | **`main` calls `UIManager.setLookAndFeel(getSystemLookAndFeelClassName())` on every headless run** (`:1110-1117`), and **sleeps 1 s unconditionally** after starting analytics (`:1363-1367`), including for `-drc`. | `Freerouting.java:1110-1117`, `:1363-1367` |
| **AG** | **The CLI parser warns but never fails.** Unknown flags warn (`GlobalSettings.java:562`, `:833`); a missing argument is silently ignored and `i` is not advanced; a parse exception is caught at `:835-837` and the loop continues. Every short flag except `-l` is matched with `startsWith`, so `-decoy`/`-diff`/`-drcx` are accepted as `-de`/`-di`/`-drc`, and `-mp -5` is impossible to express. | `GlobalSettings.java:521-838` |
| **AH** | **The help text documents 10 of the 24 accepted flags.** Undocumented: `-drc`, `-oit`, `-dl`, `-da`, `-host`, `-inc`, `-dct`, `-ll`, `--compare-boards=`, and the whole `--section.field=value` form. **There is no `-v`/`--verbose`** — the log-level flag is `-ll`. Several locale translations contain machine-translation corruption (literal ```` ```json ```` fragments). | `Freerouting_en.properties:1`, `GlobalSettings.java:825-831` |
| **AI** | **Log output pollutes stdout and ERROR is triplicated.** The Console appender targets `SYSTEM_OUT` (`Log4j2ConfigurationFactory.java:58`), a second appender targets `SYSTEM_ERR` at `Level.ERROR` (`:88-96`, `:112-113`), and the file appender takes both — so an ERROR appears three times and INFO interleaves with any stdout JSON. The `stderr` appender also reuses `filePattern` rather than `PATTERN` (`:95`), so `--logging.file.pattern=` silently reformats stderr. | `Log4j2ConfigurationFactory.java:54-113` |
| **AJ** | **`mcp_server.stdio=true` in `freerouting.json` is silently ignored** (the stdout redirect must precede logging init) and Java only warns about it 250 lines later. `-dl` is `equals` in `main:1056` but `startsWith` in `GlobalSettings.java:799`; `-ll` takes the **first** occurrence in `main:1060-1065` and the **last** in `GlobalSettings.java:825-830`. | `Freerouting.java:901-920`, `:1056-1065`, `:1191-1203` |
| **AK** | **`BoardStatistics`' `host` fallback is unreachable** — `this.host` is assigned `x + "," + y`, which is at minimum `"null,null"`, never null or empty, so `"Freerouting,<version>"` can never fire. And `board.boundingBox` is built as `Rectangle2D.Float(ur.x, ur.y, ll.x, ll.y)`, i.e. `width`/`height` hold **lower-left coordinates**, which `:385-386` then unit-scales as if they were extents. | `BoardStatistics.java:114-121`, `:126-131`, `:385-386` |
| **AL** | **`instanceof Pin` is tested before `instanceof DrillItem`**, and `Pin` *is* a `DrillItem`, so `items.drill_item_count` never counts pins. | `BoardStatistics.java:165-168` |

---

## 5. Seams — exactly what Plan 8 consumes

### 5.1 From `fr-dsn` (Plan 3, exists)
`read_board(input, id_generator, design_name, &DsnReadOptions) -> BoardReadResult`,
`read_metadata(input)`, `write(board, &ct, out, design_name, compat_mode)`,
`ses_writer::write(board, &ct, out, design_name)`, `ses_writer::snapped_endpoint`,
`ses_reader::read(input, &mut board, &ct) -> Result<SesImportSummary, DsnError>`,
`rules_reader::{read, read_router_settings, discover_layer_structure}`, `rules_writer::write`.

**Ruling A obligation:** whatever holds a `Board` between a read and a write must also hold the
`CoordinateTransform` the read produced. `fr-core`'s job struct is that holder.
**Missing:** the four non-DSN entry points are module-qualified; a crate-root rename
(`write_dsn`/`write_ses`/…) is a Plan 8 decision. `BoardMetadata::router_settings` still needs a
`read_metadata` second pass (plan-3 obligation, re-pointed by plan-5 to "whoever wires `-drc`'s
settings"). `wiring.rs:596` — `read_via_scope` still calls the unchecked `insert_via`; the seam
exists, the wiring is Plan 8's (plan-6 §11 item 5).

### 5.2 From `fr-settings` (Plan 4, exists)
`resolve_headless`, `SettingsInputs { dsn, cli_rules: Option<&[u8]>, scheduler_rules: Option<&[u8]>,
env, cli }`, `SettingsMerger`, `SettingsSource`, `SourceKind`, `priority`, the eight
`sources::*`, `apply_command_line_arguments -> LegacyBridge`, `classify_de_arguments -> DeSlots`,
`set_field_value`/`FieldSpec` (the `--section.field=value` path), `RouterSettings`,
`ScoringSettings`, `OptimizerSettings`, `FanoutSettings`, `LayerSettings`,
`DesignRulesCheckerSettings`, `DebugSettings`, `HostEnvironment`, `json` (Gson-compatible).

**Missing:** `schemars` / `JsonSchema` on `RouterSettings` (ruling 3 left it out);
`TextManager::parseTimespanString` + the 24 h cap (`lib.rs:193` marker); the sparse-payload
composition (`resolve.rs:184` obligation).

### 5.3 From `fr-drc` (Plan 5, exists)
`DesignRulesChecker::new(&mut Board)`, `generate_report(&DrcReportOptions) -> KiCadDrcReport`,
`report_to_json(&report, DrcJsonFlavor) -> Result<String, DrcError>`,
`DrcJsonFlavor::{KiCad, FreeroutingHead}`, `DrcCoordinates::convert_coordinate` (all five arms
ported for exactly this path), `DrcReportOptions { quality_score: Option<f64>, source,
coordinate_unit, freerouting_version, date }`, `item_description`, `KiCadDrcReport` + DTOs.

**Missing:** `quality_score` is still injected — Plan 8 supplies
`fr_router::score::BoardStatistics::normalized_score` (a `f32` widened to `f64`); the eight
committed references pin the values, i.e. eight free acceptance cases. The CLI must pass
`DrcJsonFlavor::KiCad` **explicitly** (ruling W) because `default()` stays `FreeroutingHead`.
`io/kicad`'s reader/writer/DTOs are rostered `// added in Plan 8:` at `crates/fr-drc/src/lib.rs:114-119`.

### 5.4 From `fr-router` (Plans 6 + 7)
Existing today: `route_connection`, `AutorouteEngine`, `board_ext::*`,
`score::{BoardStatistics + 10 DTOs, calculate_score, maximum_score, normalized_score,
is_pin_escaped}`, `RouterError`, `ExpansionCostFactor`.
Promised by Plan 7 (treat as existing): `pipeline::{run_pipeline, PipelineResult, PassRecord,
RouterStop, StopRequestState, RouterBudget, RouterCounters, TaskState, NamedAlgorithmType,
ProgressSink, RoutingEvent, NoopProgressSink, build_unrouted_report}`, `AutorouteBatchLoop::run`,
`BatchFanout`, `BatchOptimizer`, `ViaOptimizer`, `FanoutRunSummary`.

**Missing / mismatched:**
1. **`RouterStop` is not `Sync`** (`Cell<StopRequestState>`) vs spec §10's `Arc<AtomicBool>`. §3.6, Q2.
2. **`ProgressSink` is `&mut dyn`, single-threaded**; the MCP needs it to write through a shared,
   `Mutex`-guarded stdout writer.
3. `run_pipeline` returns `PipelineResult` **without** the board's DRC violations or incompletes —
   `fr-core` composes those from `fr_drc`.
4. `normalize_router_algorithm` (Plan 7 T15 note 6) is ported "so Plan 8 has it" — confirm it is `pub`.

### 5.5 From `fr-board` / `fr-geometry`
`Board`, `Board::clone`, `structural_hash`, `TimeLimit`, `StopCheck<'a> = &'a dyn Fn() -> bool`,
`ItemId`, `ClearanceViolation`, `BoardRules::{get,set}_hole_clearance`, `ClearanceMatrix`,
`Communication::{unit, resolution}`, `Board::reduce_nets_of_route_items` (ported, **uncalled**).
Nothing missing for the pipeline; see §5.7 for what is missing around the *load*.

### 5.6 Harness seams
`tests/parity/src/lib.rs` has `workspace_root`, `java_dir`, `fixture`, `example`, `reference`,
`normalize_whitespace`, `require_java_dir`, `require_reference`, `assert_text_parity`,
`normalize_drc_json`, `parse_drc_json`, `normalize_drc_doc`, `DrcReportDoc`, `RouterMetrics`,
`parse_router_jsonl`. **Missing:** `ManifestDoc` + `normalize_manifest`, `normalize_log`, and a
`run_jar(argv)` / `run_port(argv) -> (stdout, stderr, code)` pair — the first driver builds them.
`scripts/audit-port.sh` + `scripts/audit-map/*.map`: Plan 8 needs **two new maps**
(`fr-core.map`, `freerouting.map`) and audit invocations over `core/`, `core/scoring/`,
`core/results/`, `core/events/`, `io/kicad/`, `management/`, `management/jobs/`,
`management/sessions/`, `api/`, `api/mcp/`, `logger/` and `Freerouting.java` itself. **`management/`
and `api/` have never been audited by any plan** — see §5.7.

### 5.7 The one gap nothing has rostered

`HeadlessBoardManager.applyRouterSettingsForLoadedBoard` (`:739-749`) has **four** steps:

```java
  int boardLayerCount = this.board.getLayerCount();
  if (routerSettings.getLayerCount() != boardLayerCount) routerSettings.setLayerCount(boardLayerCount);  // :741-744
  routerSettings.applyBoardSpecificOptimizations(this.board);                                            // :745
  applyCopperToEdgeClearanceOverride();                                                                  // :746
  applyHoleClearanceOverride();                                                                          // :747
```

`fr_settings::resolve_headless` models **only the first two** — it takes `&Board`, not `&mut Board`,
and its module docs name exactly `:741-744` and `:745`. The last two are **board mutations**
(≈ 200 lines: `:346-396`, `:404-464`, `:466-552`) that write `board.rules.holeClearance`, reclassify
KiCad's per-layer circular hole keepouts into a dedicated clearance class, rewrite the
`ClearanceMatrix`'s board-edge row, and call `searchTreeManager.reinsertTreeItems()`.
**`grep -rn "HeadlessBoardManager\|hole_clearance\|copper_to_edge" crates/ scripts/ docs/` finds the
`RouterSettings` *fields* and `BoardRules::{get,set}_hole_clearance`, but nothing that applies them
to a board, and no `// not ported:` / `// added in Plan N:` marker anywhere.** It is unaudited
because no plan has ever run `audit-port.sh` over `management/`.

Why parity has held so far: `DEFAULT_HOLE_CLEARANCE_UM = 0.0` (`DefaultSettings.java:81`) makes the
hole override a no-op on a board whose rules already say 0, and
`DEFAULT_COPPER_TO_EDGE_CLEARANCE_UM = 500.0` (`:78`) hits the early return at `:501-507` whenever
the outline carries an explicit DSN clearance class. **It is not a no-op in general** — a
KiCad-exported board whose outline uses the fallback AREA class, or any run with
`--router.hole_clearance_um=` set, diverges. Plan 8 must port both methods and add the audit
invocation that would have caught this.

---

## 6. Proposed task decomposition (13 tasks)

Sizes are "Java lines ported, or equivalent new-code scope"; each names its pinning evidence.

| # | task | scope | depends on | evidence |
|---|---|---|---|---|
| **0** | **`fr-core` skeleton + the `CancelToken` / `ProgressSink` seam.** New crate with spec §4's dependency direction. `CancelToken { flag: Arc<AtomicBool>, deadline: Option<Instant> }` plus the adapter that copies it into Plan 7's `RouterStop` at the six poll sites; a `Sync` `ProgressSink` wrapper; `Ctx`, `RoutingResult`, `RoutingPipeline::run` over `run_pipeline`. Roster `api/**`, `analytics/**`, `gui/**`, `SessionManager`, the scheduler queue, `core/events/**`. | ~200 new | Plan 7 T15 | a test that a cancel from a second thread stops a run mid-pass; `cargo test -p fr-router --test batch_parity` unchanged (the adapter must be a no-op when nothing cancels) |
| **1** | **`RoutingJob`, `BoardFileDetails`, `FileFormat`.** `getFileFormat(byte[])` `:151-227` (incl. quirk O's shift loop, **totalised**), `(Path)` `:230-247`, `setInput`/`setRules`/`tryToSetOutputFile`/`setInputFromFile`/`changeFileExtension`, `BoardFileDetails.{setData, setFilename, calculateCrc32, getAbsolutePath}`, the state enum and `isCliTerminalState` (quirk N recorded). `Session` collapses to `SessionId`. | ~530 | 0 | **`p8t5`**'s classification half: 40 argv/file shapes, diffing the resolved slots, `FileFormat`, derived output name and CRC32 against the jar |
| **2** | **`BoardStatistics(byte[], FileFormat)` + `countOccurrences` + `toString`.** The SES / DSN / KiCad-JSON scrapers verbatim; quirks F and G reproduced, not fixed. | ~140 | 0 | a **`P8T2Probe.java`** transcript: field-by-field jar output for every corpus `.dsn` and every committed `.ses`/`batch.ses`, pinned as literals |
| **3** | **The board load sequence — `HeadlessBoardManager`'s real slice (§5.7).** `loadFromSpecctraDsn`/`loadFromKiCadJson` → `applyParsedBoardResult` → **`applyCopperToEdgeClearanceOverride` `:466-552`, `applyHoleClearanceOverride` `:346-396`, `assignHoleKeepoutClearanceClass` `:404-464`** → `reduce_nets_of_route_items`; `saveAsSpecctraSessionSes`; `calculateCrc32ForBoard`. Roster the diagnostic half. Add the `management/` audit invocation. | ~370 | 1 | a **`P8T3Probe.java`** that dumps the clearance matrix, `holeClearance`, the reclassified keepout ids and the search-tree leaf count before/after each override, on a KiCad-exported fixture and on `--router.hole_clearance_um` ∈ {0, 100, 500} |
| **4** | **`RoutingResultManifest`.** All 12 fields in declaration order with their `@SerializedName`s, the three nested DTOs, `fromJob`, `write`, `resolveGitSha`, `sha256Hex`; quirks E and W reproduced. The Gson-compatible serialisation of `BoardStatistics` (ruling AG makes it Plan 8's) and of `RouterSettings` (reuse `fr_settings::json`). | ~250 | 1, 2 | **`p8t2`** — jar vs port manifest, byte-identical after normalising `generated_at`, `duration_seconds`, `git_sha`, `resource_usage` |
| **5** | **`TextManager::parseTimespanString` + the timeout ladder.** `:83-118` plus the 24 h cap (`RoutingJobSchedulerActionThread.java:24, 44-52`) and the monitor's observable effect (`requestStop()` 30 s before `TIMED_OUT`). Wire `RouterStop::with_deadline`. | ~90 | 0 | a probe over 30 timespan strings (`"1:30:00"`, `"90"`, `"1.5"`, `"1:2:3:4"`, `"x"`, `""`, `"25:00:00"`) diffed against the jar |
| **6** | **The legacy-shim rewire.** `legacy.rs` stops reproducing the `-de` rule and calls `fr_settings::classify_de_arguments`; values go through `apply_command_line_arguments` → `LegacyBridge`. Decide and record the five dead flags (Q3). Add the missing accepted flags (`--compare-boards=`, `-dct`, `-host`, `-da`; the `--section.field=value` form already routes to `--set`). Reconcile the port-only exit codes 2 and 3 (Q4). Record the `startsWith` prefix-matching and silent-missing-value behaviours (quirk AG). | ~150 (mostly deletion) | 1 | **`p8t5`** — argv-shape parity on ~60 command lines incl. `-mpx 5`, `-de a.txt`, `-oit -5`, `--router.enabled=`, bare `-drc`, `-mp` with no value, `-decoy` |
| **7** | **`freerouting route` end to end.** `initializeCli:79-187` minus the session/scheduler/banner: the guards, the output-file delete (quirk I), `SettingsInputs` assembly (**bytes** for both rules slots, quirk #142), `resolve_headless`, Task 3's load sequence, `run_pipeline`, `ses_writer::write` holding the `CoordinateTransform`, `writeCliOutputIfAvailable`, `computeCliExitCode`, the manifest hook. **`isCliTerminalState`'s `INVALID` hang is totalised** (quirk N) with a recorded decision. | ~260 | 3, 4, 5, 6 | **`p8t1`** — SES bytes + exit code + normalised stderr on all eight `router-fixtures.txt` stems; Plan 7 T16's CI/slow split carries over |
| **8** | **`freerouting drc` end to end.** `initializeDrc:246-374`: the DSN → `.rules` → session **order**, the `.json`/`.ses` branch (the `.json` arm delegates to Task 9), the hard-coded `"mm"`, `source` = base name, the injected clock, `DrcJsonFlavor::KiCad` as the CLI default (ruling W), the quality score from `normalized_score` (discharging plan-5's obligation), the **separate merge** of quirk C, file-vs-stdout (the spec-§12 divergence), the exit codes (quirks A and B recorded as deliberate divergences). | ~210 | 2, 7 | **`p8t3`** on the eight committed `drc-*` references plus `drc-issue593-rules` and `drc-issue593-ses`; the eight references' `quality_score` becomes a **computed** assertion instead of an injected one |
| **9a** | **`io/kicad` — the board reader.** `KiCadJsonReader.{readBoard, addPoint, boundingBox}` (~800). Shares `fr-dsn`'s `BoardReadResult`/`BoardMetadata`/`CoordinateTransform`. **Must not "fix" quirk #83** (the J-then-I clearance-matrix indexing). Resolves the `--kicad-json` design-input slot recorded in `docs/cli-legacy-flags.md`. | ~800 | 1, 3 | round-trip against the jar's own reader over any `.json` corpus fixtures; if none exist, a synthetic pair plus `-de board.json -do out.ses` diffed against the jar |
| **9b** | **`io/kicad` — the writer, the DTOs and `importSession`.** `KiCadJsonWriter.write` (227), `KiCadBoardJson` (142), `KiCadJsonReader.importSession`. **Quirk T** (the format re-sniff that loses the final board) is reproduced or totalised with a recorded decision. | ~450 | 9a, 8 | a `-de x.json -do y.json` run vs the jar, and the DRC `.json`-session branch of `p8t3` |
| **10** | **`freerouting info` + the board-summary JSON.** New: layers/nets/components from `BoardStatistics` + `read_metadata`; stdout JSON with a stable key order. | ~100 new | 2 | a golden file per corpus stem, cross-checked field-for-field against the corresponding `BoardStatistics` values |
| **11** | **MCP: the concurrent transport.** Discharge the two obligations already in the tree: reader thread → channel, `Mutex`-guarded writer, per-request `CancelToken`, a handler signature that can emit interim messages. `initialize`/`ping`/`tools/list`/`tools/call`/inbound `notifications/cancelled`/outbound `notifications/progress`. Java's bridge is rostered `// renamed:`; the tool registry `// not ported:`. | ~400 new | 0 | `crates/freerouting/tests/mcp_stdio.rs` extended: a long-running fake tool that reports progress and is cancelled mid-flight; the six existing skeleton tests stay green |
| **12** | **MCP: the four tools + `schemars`.** `route_board`, `check_drc`, `board_info`, `list_settings` per spec §13, with the `dsn_path | dsn_text` / `ses_path | ses_text` variants; `schemars` derived on `RouterSettings` so `list_settings` and the CLI cannot drift; the **sparse priority-70 composition** (`resolve.rs:184`) for the `settings?` argument. | ~320 new | 7, 8, 10, 11 | **`p8t6`** — an end-to-end MCP conversation over spawned pipes routing `tutorial_board.dsn`, asserting the SES equals `p8t1`'s; a schema snapshot test |
| **13** | **KiCad acceptance, audits, roster, quirk register, hand-off.** `scripts/gen-cli-reference.sh`, the `p8t*` sweep, two new audit maps and the **twelve** new `audit-port.sh` invocations of §5.6; `grep -rn "added in Plan 8" crates/` must return **nothing**; quirk ids allocated contiguously; `docs/plan-8-handoff.md`. | — | all | every driver green on the committed tree, run by the reviewer |

**Dispatch order:** 0 → 1 → 2 → 3 → 4 → 5 → 6 → 7 → 8 → 9a → 9b → 10 → 11 → 12 → 13.
Parallelisable after Task 1: {2, 5} share no file with {3}; **{11} is pure new code and can run
beside 3–10**. Sizing: **opus** for 0 (the concurrency seam), 3 (an unrostered 370-line gap that
mutates the board under every caller), 7, 8, 9a, 11, 12, 13; **sonnet** for 1, 2, 4, 5, 6, 9b, 10.
Reviewers: opus for 0, 3, 7, 8, 9a, 11, 12, 13. From Task 3 on, every review re-runs
`cargo test -p fr-router --test batch_parity` and `run.sh p6t1` on the five Plan 6 stems.

---

## 7. Open questions for the controller

1. **What should the MCP contract expose?** Java's ~29 tools are auto-derived from the OpenAPI scan
   of `/v1/*` (`OpenApiMcpToolRegistry.java:54-118`), use a bucketed `{path, query, body}` input
   shape, stringify their result into a single text block, and model a **7-call stateful pipeline**
   (`create_session` → `enqueue_job` → `upload_job_input_file` → `update_job_settings` →
   `start_job` → poll → `download_job_output_file`). There is **no** Java analogue for
   `board_info` or `list_settings` and no single-shot route tool, and Java's MCP has **no progress
   and no cancellation at all**. Recommendation: **implement spec §13's four tools verbatim with
   flat arguments, roster the registry `// not ported:`, and make `p8t6` a documented-delta driver,
   not a parity one.** Confirm — and confirm whether the tool *output* fields should follow
   `api/dto/BoardFilePayload`'s naming (`job_id`, `data`, `size`, `crc32`, `format`, `statistics`,
   `filename`, `path`) where they overlap, so an agent written against the Java server is not
   gratuitously broken.
2. **`RouterStop` is `Cell`-based and therefore not `Sync`; spec §10's `CancelToken` is
   `Arc<AtomicBool>`.** Does Plan 8 (a) add an external `Arc<AtomicBool>` that the routing thread
   copies into `RouterStop` at the six existing poll sites — Plan 7's interface untouched;
   (b) reopen Plan 7's `pipeline/stop.rs` and make the field an `AtomicU8`; or (c) confine routing
   to one thread and accept that `notifications/cancelled` never takes effect (which is what Java
   does)? The task table assumes **(a)**. This is the single largest structural decision.
3. **The five dead legacy flags (`-oit`, `-us`, `-is`, `-hr`, `-inc`), `-drc`'s router switch, and
   `-mt`/`--threads` — live or dead?** Plan 4 handed this over explicitly as a *product* decision
   (quirks #131, #143). Making them live makes the port more capable than Java and produces
   unattributable parity differences. Recommendation: **keep them dead on the legacy path**, and
   expose the same knobs through the native `--set section.field=value` form, where there is no jar
   to disagree with. Record it as a decision, not a detail.
4. **Must `freerouting-rs` accept the jar's exact flag syntax, and what happens to the port-only
   exit codes?** Java accepts 24 flags (10 documented), matches every short flag except `-l` by
   `startsWith`, silently ignores a missing value, warns-but-continues on an unknown flag, and only
   ever exits 0 or 1. The port today errors with **2** on a legacy-rewrite failure and returns **3**
   for "not implemented". Recommendation: **yes, accept the jar's syntax bug-for-bug on the legacy
   path** (that is what `p8t5` measures), **map every legacy-path failure onto exit 1**, and reserve
   2/3 for the *native* subcommand form, recorded as a port-only extension. Sub-questions: does
   `-di` stay unsupported (`legacy.rs:122`)? Does the port reproduce Java's **silent** no-op for a
   missing flag value, or error? And is there a `-v`/`--verbose` at all, given Java's is `-ll`?
5. **Is `board/state/BoardComparator.java` (758 lines) in scope, and with it `--compare-boards`?**
   It was re-pointed to Plan 8 as "the result-manifest/report layer", but nothing in
   `RoutingResultManifest`, the DRC report or spec §10 reads it — its only Java caller is
   `Freerouting.compareBoardFiles:826-863`, an undocumented mode absent from spec §12.
   Recommendation: **roster both `// not ported:`** and say so in the hand-off. Same question,
   separately, for `core/scoring/{BoardScoreBreakdown, ScoringWeightComparison}` (424 lines,
   **reachable only from a Java unit test**, and using a formula that disagrees with the live one by
   the DSN resolution factor — quirk X): recommendation **roster them too**, since porting a dead
   class with a known-divergent formula buys nothing and risks someone wiring it up.
6. **What are the port's version strings?** The manifest's `app_version`
   (`Constants.FREEROUTING_VERSION`, `"2.3.1-SNAPSHOT"`), `BoardStatistics.host`'s fallback
   (unreachable, quirk AK), the DRC report's `freerouting_version` (`"Freerouting " + …`), the
   SES/DSN writer's `(host …)`, and the MCP `serverInfo.version` (currently `CARGO_PKG_VERSION`,
   and `protocolVersion` currently `"2025-06-18"` against Java's `"2024-11-05"`) are five different
   strings. A byte-parity SES needs the *jar's* value; a truthful manifest wants the port's.
   Recommendation: **one `fr_core::PARITY_VERSION` used for every file-format field, the crate
   version only in `serverInfo`, and the newer MCP `protocolVersion` kept** (the port is a new
   server, not a re-implementation of Java's).
7. **Is the HTTP/REST API in scope?** Recommendation: **no.** Spec §2 drops it; it is 8 425 lines of
   Jersey/Jetty/auth/rate-limiting of which `JobControllerV1`'s 659 are provably dead; and the MCP
   server covers the "drive it from a program" use case. Roster `api/**` in full. Confirm, because
   it also decides whether `fr-settings`' `ApiSettings` priority-70 tier stays a test-only
   construct — and note that the MCP `route_board { settings? }` argument needs *exactly* that tier,
   composed by hand per `resolve.rs:184`.
8. **Which jar for the end-to-end drivers, and is a SES byte-diff the right gate for `p8t1`?**
   `tests/reference/README.md` pins DSN/SES *syntax* to the 2.3.0 jar (ruling 1) while Plan 7's
   `gen-batch-reference.sh` pins routed SES *content* to the HEAD jar; a `-de/-do` run against 2.3.0
   would exercise a different router. Recommendation: **`p8t1` diffs content against the HEAD jar and
   asserts separately that the token spelling is 2.3.0's** (`parity_ses.rs` already does the latter)
   — two assertions, not one byte-diff. Confirm, and confirm whether the eight Plan 7 stems are the
   right `p8t1` set, or whether the KiCad acceptance of spec §1 needs a genuinely KiCad-exported
   fixture the corpus does not have today — which is also the only way to exercise §5.7's
   copper-to-edge override on its non-early-return path.
