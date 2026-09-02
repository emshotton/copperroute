//! `fr-core` — the composition layer between `fr-router` and the `freerouting` binary.
//!
//! Spec §4: *"`fr-core` = RoutingPipeline, CancelToken, ProgressSink, RoutingResult,
//! BoardStatistics, result manifest"*. This crate is what a caller who wants to *route a board*
//! talks to; everything below it is the router, the DRC, the readers and the settings ladder.
//!
//! # Plan-8 ruling 1: it re-exports, it does not move
//!
//! Plan 7 controller answer 3 chose re-export over relocation. `fr_core::{BoardStatistics,
//! RoutingEvent, ProgressSink, PassRecord, TaskState, …}` are `pub use fr_router::…`, and
//! [`RoutingPipeline::run`] wraps [`fr_router::pipeline::run_pipeline`]. **Nothing in `fr-router`
//! moves file.** The cost, stated plainly rather than hidden: this crate is *partly a façade*, so
//! a reader looking for where a routing decision is made will find it in `fr-router` even though
//! the call came through here.
//!
//! # What is Task 0's, and what the rest of Plan 8 adds
//!
//! Task 0 lands [`CancelToken`], [`SyncProgressSink`], [`Ctx`]/[`RoutingResult`],
//! [`RoutingPipeline::run`], the timeout ladder ([`timespan`]), [`PARITY_VERSION`], and the whole
//! `// not ported:` roster at the foot of this file. Later tasks add the job model, the
//! byte-scraping statistics twin, the load/save sequence and the result manifest, each in its own
//! module.
//!
//! # The roster
//!
//! The `// not ported:` block at the foot of this file is ≈ 13 500 Java lines with **the grep
//! that proves each one**. It lands in this task, before the code that would otherwise be audited
//! against it, so that every later task's `scripts/audit-port.sh` run has something to check
//! against. A roster line with no evidence is a defect, not a shortcut.

#![forbid(unsafe_code)]

pub mod cancel;
pub mod ctx;
pub mod file_details;
pub mod job;
pub mod load;
pub mod manifest;
pub mod pipeline;
pub mod progress;
pub mod save;
pub mod stats_from_bytes;
pub mod stats_json;
pub mod timespan;

pub use cancel::{CancelToken, Deadline};
pub use ctx::{Ctx, RoutingResult};
pub use file_details::BoardFileDetails;
pub use job::{
    BINARY_FILE_EXTENSION, DSN_FILE_EXTENSION, EAGLE_SCRIPT_FILE_EXTENSION, FILE_SEPARATOR,
    FileFormat, JobId, RULES_FILE_EXTENSION, RoutingJob, RoutingJobState, RoutingStage,
    SES_FILE_EXTENSION, SessionId, Uuid128, validate_session_host,
};
pub use load::{
    LoadedBoard, ParsedBoard, apply_immediate_post_load_processing, apply_parsed_board_result,
    apply_router_settings_for_loaded_board, load_board_if_needed, load_from_kicad_json,
    load_from_specctra_dsn, parse_board_if_needed, parse_board_result, parse_from_specctra_dsn,
};
pub use manifest::{
    FixtureInfo, PhaseDetail, PhaseMetrics, RouterJobResourceUsage, RoutingResultManifest,
    SCHEMA_VERSION, format_utc_iso8601, now_utc_iso8601, resolve_git_sha, sha256_hex,
};
pub use pipeline::RoutingPipeline;
pub use progress::{SyncProgressSink, SyncProgressSinkView};
pub use save::{calculate_crc32_for_board, save_as_specctra_session_ses};
pub use stats_from_bytes::{BoardStatisticsExt, count_occurrences};
pub use stats_json::{GsonBoardStatistics, to_gson_json, to_gson_string};
pub use timespan::{
    GRACE_PERIOD_SECONDS, MAX_TIMEOUT_SECONDS, convert_from_timespan_to_duration_format,
    job_timeout_deadline, job_timeout_deadline_from, parse_timespan, parse_timespan_seconds,
};

// ── Ruling 1's re-export surface ────────────────────────────────────────────────────────────────
//
// `fr_router::{pipeline, score}` are re-exported, never moved (plan-7 controller answer 3). A
// caller of this crate should not have to know that the router lives one crate down.
pub use fr_router::pipeline::{
    NamedAlgorithmType, NoopProgressSink, PassRecord, PipelineResult, ProgressSink, RouterBudget,
    RouterCounters, RouterStop, RoutingEvent, StopRequestState, TaskState, build_unrouted_report,
    prepare_board, run_pipeline,
};
pub use fr_router::score::BoardStatistics;

// =================================================================================================
// Version strings — controller ruling AT, as corrected by plan ruling 5
// =================================================================================================

/// The jar value every FILE-FORMAT version field was pinned against.
///
/// # Three readers, and the SES/DSN is not one of them
///
/// Controller ruling AT asked for "one `PARITY_VERSION` for every FILE-FORMAT field" and listed
/// the SES/DSN `(hostCad …)`/`(hostVersion …)` among them. **Plan ruling 5 corrects that**: those
/// two are written by `io/specctra/parser/Parser.java:108-115` from
/// `ReadScopeParameter.{hostCad, hostVersion}` (`ReadScopeParameter.java:69`), which
/// `Parser.readScopeParameter:204` fills **from the input file**, and
/// `crates/fr-dsn/src/ses_writer.rs:297` already echoes them back through `board.communication`.
/// The real readers are exactly three:
///
/// | reader | Java |
/// |---|---|
/// | the DRC report's `freerouting_version` | `drc/DesignRulesChecker.java:213` — `"Freerouting " + Constants.FREEROUTING_VERSION` |
/// | the manifest's `app_version` | `core/results/RoutingResultManifest.java:102` |
/// | `BoardStatistics.host`'s fallback | `core/scoring/BoardStatistics.java:119` — **unreachable**, quirk label AK |
///
/// # Where the literal comes from
///
/// `Constants.java` is **build-generated** and absent from the clone's source tree; the symbol is
/// `app.freerouting.constants.Constants`. The value below was read out of the HEAD jar at port
/// time, not guessed:
///
/// ```text
/// $ unzip -p ../freerouting/build/libs/freerouting-current-executable.jar \
///       app/freerouting/constants/Constants.class | strings
/// …
/// FREEROUTING_VERSION
/// 2.3.1-SNAPSHOT
/// FREEROUTING_BUILD_DATE
/// 2026-09-01
/// ```
///
/// with that jar's `META-INF/MANIFEST.MF` recording
/// `Build-Revision: 278fe14123c49376667239659c98d41a597acce9` and `Build-Date: 2026-09-01`. Those
/// three facts travel together: a `PARITY_VERSION` without the revision it was measured at cannot
/// be re-derived.
pub const PARITY_VERSION: &str = "2.3.1-SNAPSHOT";

/// The HEAD jar's `Constants.FREEROUTING_BUILD_DATE`, read alongside [`PARITY_VERSION`] from the
/// same class file. Not a format field itself; recorded so a reader can tell which jar build the
/// version literal was pinned against.
pub const PARITY_BUILD_DATE: &str = "2026-09-01";

/// The HEAD jar's `Build-Revision` from `META-INF/MANIFEST.MF`, the git sha of the Java tree
/// [`PARITY_VERSION`] was measured against.
pub const PARITY_JAR_REVISION: &str = "278fe14123c49376667239659c98d41a597acce9";

/// The crate version, used **only** in the MCP `serverInfo` (controller ruling AT). The port is a
/// new server and says so; it does not claim to be the jar.
pub const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

// =================================================================================================
// `Error`
// =================================================================================================

/// What `fr-core`'s operations answer instead of throwing.
///
/// Java's headless path throws out of `RoutingJobSchedulerActionThread.threadAction` into the job
/// scheduler, which catches `Throwable` and writes `job.state = RoutingJobState.FAILED`. The port
/// answers a `Result` and lets the CLI's exit ladder decide (controller ruling AR: the legacy path
/// maps every failure to exit code **1**).
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The router or one of its stages failed — including plan-7 ruling 7's
    /// [`fr_router::RouterError::NoRoutableLayer`], which
    /// `RoutingPipeline.run` does not catch in Java either.
    #[error(transparent)]
    Router(#[from] fr_router::RouterError),

    /// A board operation failed.
    #[error(transparent)]
    Board(#[from] fr_board::BoardError),

    /// An I/O operation failed — reading a design, writing a session or a report.
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// The board load or save sequence failed — `BoardLoader.loadBoardIfNeeded`'s three
    /// `FRLogger.error` + `return false` exits (`management/BoardLoader.java:26`, `:32-35`,
    /// `:52`) and `HeadlessBoardManager.applyParsedBoardResult`'s two error arms (`:720-732`).
    /// **Java throws nothing here**: it logs and answers `false` or hands the failing
    /// `BoardReadResult` back. The port carries Java's message text verbatim, because that text
    /// is the only part of those exits a caller can observe.
    #[error("{0}")]
    Load(String),

    /// A job-model precondition failed. Today that is exactly one check: `core/Session.java:37`'s
    /// `host.split("/").length != 2`, ported by [`validate_session_host`]. The message is Java's,
    /// verbatim, because it is the only thing a caller can observe of it.
    #[error("{0}")]
    Session(String),
}

// =================================================================================================
// The `// not ported:` roster — ≈ 13 500 Java lines, each line with the grep that proves it
// =================================================================================================
//
// Controller rulings AO, AQ, AS, AU and spec §2 decide *what* is dropped; this block records *why*
// each one is dropped and how a reader re-derives it. Every grep below was re-run against the
// clone's HEAD at port time (`Build-Revision 278fe14123c49376667239659c98d41a597acce9`) and its
// output pasted into the Task 0 commit message, per plan-7 ruling 6's precedent.
//
// Line counts are `find … -name '*.java' -exec cat {} + | wc -l`, i.e. whole files including
// javadoc and imports — the same unit the plan's ≈ 13 500 budget uses. Where a count below
// disagrees with the plan's, the file is authoritative and the difference is noted on the line.
//
// -------------------------------------------------------------------------------------------------
// 1. The HTTP API — controller ruling AU. `api/**`, 45 files, 8 425 lines (which INCLUDES
//    `api/mcp/**`'s 2 105; the plan's "8 425 + 2 105" double-counts, and the file wins).
// -------------------------------------------------------------------------------------------------
//
// not ported: `api/**` in full — the Jersey/Jetty REST surface (`FreeroutingApplication`,
// `api/v1/*ControllerV1`, `api/v1/Job{Input,Output,Progress}Resource`, `api/dto/**`,
// `api/security/**`, the filters and the exception mappers). Ruling AU: the port has no HTTP
// server, and spec §2 puts one out of scope. **`ApiSettings`' priority-70 settings tier STAYS** —
// the MCP's `route_board { settings? }` argument composes exactly that tier, per the `obligation:`
// at `crates/fr-settings/src/resolve.rs:193`.
//   Evidence (the whole package is reachable only through a running Jetty):
//     $ grep -rn "FreeroutingApplication" src/main/java | grep -v '^src/main/java/app/freerouting/api/'
//     (nothing outside api/ but `Freerouting.initializeAPI`, itself rostered below)
//
// not ported: `api/v1/JobControllerV1` in full (659 lines) — **provably dead even inside the API**.
// It is a "compatibility façade" that `FreeroutingApplication.getClasses()` (`:36-63`) does not
// register, so Jersey never instantiates it and none of its endpoints is routable.
//   Evidence, re-run at port time:
//     $ grep -rn "JobControllerV1" src/main/java src/test/java
//     src/main/java/.../api/v1/JobInputResource.java:38:    * …the compatibility façade {@link JobControllerV1} so
//     src/main/java/.../api/v1/JobOutputResource.java:47:   * …the compatibility façade {@link JobControllerV1} so
//     src/main/java/.../api/v1/JobProgressResource.java:40:  * …the compatibility façade {@link JobControllerV1} so
//     src/main/java/.../api/v1/JobControllerV1.java:31,38,40,41   (the declaration itself)
//     src/test/java/.../api/v1/JobResourceContractTest.java:31:  assertFalse(registered.contains(JobControllerV1.class));
//   Three javadoc `@link`s and a Java test that asserts it is **not** registered. The plan cited
//   the three `@link`s; the test is stronger and was found by re-running the grep.
//
// -------------------------------------------------------------------------------------------------
// 2. The MCP's Java transport — controller ruling AO. `api/mcp/**`, 11 files, 2 105 lines.
// -------------------------------------------------------------------------------------------------
//
// not ported: `api/mcp/**` in full — `McpControllerV1`, `OpenApiMcpToolRegistry`,
// `AgentCardController`, the Jetty/WebSocket transports and their DTOs. Ruling AO: the port
// implements spec §13's **four** tools over a native stdio JSON-RPC server, not Java's
// OpenAPI-derived registry over HTTP. These files are read for tool *shape* — the port keeps
// `api/dto/BoardFilePayload`'s field names (`job_id`, `data`, `size`, `crc32`, `format`,
// `statistics`, `filename`, `path`) so an agent written against the Java server is not
// gratuitously broken — and never for transport. `p8t6` is the documented-delta driver.
//
// renamed: `Freerouting.startMcpStdioBridge` (`Freerouting.java:681-788`, 108 lines) ->
// `crates/freerouting/src/mcp` (**landed in Plan 8 Task 11**). Java's "stdio bridge" is a daemon
// thread that pipes stdin lines into an **HTTP** POST against its own locally-bound Jetty MCP
// endpoint and prints the response body with every `\r` and `\n` stripped (`:770` at the clone's
// HEAD — the plan and the brief both write `:766`, which is four lines stale; quirk label M).
// Spec §13 replaces the whole arrangement with a native stdio JSON-RPC server, so the *feature* is
// ported and the *mechanism* is not. The ten behavioural differences that survive are the delta
// table in `crates/freerouting/README.md`, which `p8t6` (Task 12) asserts is exactly itself.
//
// -------------------------------------------------------------------------------------------------
// 3. Telemetry and version checking — spec §2.
// -------------------------------------------------------------------------------------------------
//
// not ported: `analytics/**` in full — 13 files, **2 228** lines (the plan's "2 100" is stale;
// the file wins). `FRAnalytics` and its Segment/HTTP transport post usage events to a third-party
// endpoint. Spec §2 puts telemetry out of scope, and a port that phoned home would be a defect,
// not a parity gap.
//
// not ported: `util/VersionChecker` in full (119 lines) — a `Runnable` that fetches the latest
// release from GitHub in a background thread and logs a nag line.
//   Evidence (one construction site, itself on a rostered path):
//     $ grep -rn "VersionChecker" src/main/java | grep -v '^src/main/java/app/freerouting/util/VersionChecker.java'
//     src/main/java/app/freerouting/Freerouting.java:32:   import app.freerouting.util.VersionChecker;
//     src/main/java/app/freerouting/Freerouting.java:1388: VersionChecker checker = new VersionChecker(Constants.FREEROUTING_VERSION);
//
// -------------------------------------------------------------------------------------------------
// 4. Sessions and the job queue — quirk labels AA and AB (rows #238 and #239).
// -------------------------------------------------------------------------------------------------
//
// not ported: `management/sessions/SessionManager` in full (190 lines) and `core/Session`'s job
// list (`core/Session.java`, 62 lines). A single-shot CLI has one session by construction, and the
// only line that makes `SessionManager` a hard dependency of the queue is a validation that
// discards its own result: `RoutingJobScheduler.enqueueJob:315-318` reads `session.userId`, throws
// if it is null, and **never uses it** (quirk label AA, row #238). See also quirk label Z
// (`Session.java:30-42` mutates before it validates) — Task 1's row.
//
// not ported: `management/jobs/RoutingJobScheduler`'s **queue half** (~450 of its 578 lines) — the
// `:48-282` daemon poll loop started from a static field initialiser, `saveJob :350-377`,
// `saveJobToDisk :379-452`, `getQueuePosition :461-466`, the three `listJobs` overloads
// (`:468`, `:480`, `:495`), `getJob :528-537`, `clearJobs :539-548` and `cancelJob :550-`. The
// port calls the pipeline directly: there is no queue and no daemon (plan ruling 8, quirk label J
// — Java's two nested polling loops add ~0.75 s to every single-shot run and quantise the
// manifest's `duration_seconds`). What the CLI *does* need from this file — the `.rules`
// discovery and the settings merge of `:103-186` — is Plan 4's, already ported.
//   Evidence, re-run at port time (every caller is GUI or `api/v1`, and one method has none):
//     $ grep -rn "\.getQueuePosition(" src/main/java     -> (nothing)
//     $ grep -rn "\.listJobs(" src/main/java
//       gui/board/BoardToolbar.java:181 · api/v1/JobProgressResource.java:93,95 · api/v1/JobControllerV1.java:112
//     $ grep -rn "\.clearJobs(" src/main/java
//       gui/board/BoardFrameFileActions.java:57 · gui/board/BoardFrame.java:1143
//     $ grep -rn "\.cancelJob(" src/main/java
//       api/v1/JobProgressResource.java:285 · api/v1/JobControllerV1.java:219
//     $ grep -rn "\.saveJob(" src/main/java
//       api/v1/JobProgressResource.java:225 · api/v1/JobInputResource.java:117,121,122,123
//   and `saveJobToDisk`'s two `Files.write` calls (`:440`, `:450`) sit under
//   `saveJob :351`'s `if (globalSettings.featureFlags.saveJobs)`, so even the API path writes
//   nothing unless that flag is on.
//
// not ported: `management/jobs/RoutingJobSchedulerActionThread.monitorCpuAndMemoryUsage`
// (`:208-257`, 50 lines) — it samples `com.sun.management.ThreadMXBean` and fills only the
// manifest's `resource_usage`, which `p8t2` normalises out (plan ruling 8, quirk label J; the
// port therefore writes 0.0 for all five fields, and quirk #256 records that two of the five —
// `io_read` and `io_written` — are 0.0 in **Java** too, because nothing anywhere assigns them).
// Quirk label Y (row #237): the monitor
// thread that calls it never exits, because its loop condition is
// `while ((job != null) && (job.thread != null))` (`:58`) and `job.thread` is never nulled, and its
// `catch (Throwable t) {}` at `:253-256` is completely silent.
//
// not ported: `management/jobs/RoutingJobSchedulerActionThread`'s **monitor thread** (`:55-90`,
// 36 lines) — its *observable* effect is two instants and the port carries those on
// [`Deadline`]: `stop_at` (the `requestStop()` at `:75`, i.e. `ALL`, quirk #201 — which is why
// `AutorouteBatchLoop:251-253`'s `requestStopAutoRouter()` is always a no-op) and `timed_out_at`
// (`:84`'s `job.state = TIMED_OUT`, one `GRACE_PERIOD` later). `RouterStop::poll_deadline`'s one
// production site (`crates/fr-router/src/pipeline/batch_loop.rs:303`) is where they bite.
//
// -------------------------------------------------------------------------------------------------
// 5. The event mechanism — controller ruling AK's replacement, and its four registration points.
// -------------------------------------------------------------------------------------------------
//
// not ported: `core/events/**` — 6 files, **95** lines (the plan's "130" is stale; the file wins):
// `BoardFileDetailsUpdatedEvent`/`Listener`, `RoutingJobLogEntryAddedEvent`/`Listener`,
// `RoutingJobUpdatedEvent`/`Listener`. Spec §10's [`ProgressSink`] replaces the mechanism, and on
// the CLI path every list is empty anyway.
//   Evidence — the four registrations that add a **consumer** are all in `api/v1/*`:
//     $ grep -rn "\.addSettingsUpdatedEventListener(\|\.addInputUpdatedEventListener(\|\.addOutputUpdatedEventListener(\|\.addLogEntryAddedEventListener(" src/main/java
//     api/v1/JobInputResource.java:120,122,123 · api/v1/JobProgressResource.java:404
//   The six `addUpdatedEventListener` calls in `core/RoutingJob.java` (`:273`, `:390`, `:440`,
//   `:446`, `:452`) and `RoutingJobSchedulerActionThread.java:262` only **re-fire** into those same
//   four lists, so with no API running they deliver to nobody. (`:390` is quirk label P — the
//   *output* listener fires `fireInputUpdatedEvent()`, a copy-paste bug; Task 1's row.)
//
// -------------------------------------------------------------------------------------------------
// 6. Two job enums whose values nothing reads.
// -------------------------------------------------------------------------------------------------
//
// not ported: `core/RoutingJobPriority` (22 lines) — the seven-constant enum with a clamped
// `float value`. **`getValue()` has no caller anywhere in the tree**, and the ordering that does
// exist uses `ordinal()`, not the value:
//     $ grep -rn "getValue()" src/main/java | grep -i priorit
//     core/RoutingJobPriority.java:19:  public float getValue() {          (the declaration only)
//     $ grep -n "priority" core/RoutingJob.java
//     :81  public RoutingJobPriority priority = RoutingJobPriority.NORMAL;
//     :411,:413  if (this.priority.ordinal() < o.priority.ordinal()) …
//   So the clamp at `RoutingJobPriority.java:15` is arithmetic on a number nothing consumes, and a
//   single-shot CLI has no queue to order anyway (see the scheduler roster above).
//
// not ported: `core/RoutingStage` (8 lines) — **three assignments plus the field's initialiser,
// and no control-flow reader**. The plan
// said it "is initialised to IDLE and never reassigned in `main/`"; the file disagrees and wins:
//     $ grep -rn "RoutingStage\." src/main/java | grep -v core/RoutingStage.java
//     core/RoutingJob.java:77                      stage = RoutingStage.IDLE;         (the field)
//     autoroute/pipeline/RoutingPipeline.java:84   this.job.stage = RoutingStage.IDLE;
//     autoroute/pipeline/RoutingPipeline.java:94   this.job.stage = RoutingStage.ROUTING;
//     autoroute/pipeline/RoutingPipeline.java:121  this.job.stage = RoutingStage.OPTIMIZATION;
//     $ grep -rn "\.stage\b" src/main/java | grep -v core/RoutingStage.java
//     (the same three assignments, and nothing else — no comparison, no read)
//   **Qualified, because Task 1 ports `RoutingJob` next.** "No reader" is a statement about
//   hand-written control flow. The field is `@SerializedName("stage")` + `@Schema(...)` at
//   `core/RoutingJob.java:75-77`, so it IS read **reflectively** — by Gson, into the job JSON, and
//   by the OpenAPI schema generator. Both of those surfaces are `api/**`, which ruling AU rosters
//   in full (§1), so nothing the port emits needs `stage`; but Task 1 must not read this line as
//   "the field is unused" and drop it from a JSON surface it does port. The one other reader is a
//   Java test's diagnostic string (`src/test/java/.../fixtures/RoutingFixtureTest.java:194`).
//   The correction matters: it is *reassignment* that is absent from the port, not from Java, and
//   `crates/fr-router/src/pipeline/run.rs` already carries the three `// not ported:` write markers.
//
// -------------------------------------------------------------------------------------------------
// 7. `management/HeadlessBoardManager`'s comparison, validation and deferred halves —
//    quirk label AE (row #240).
// -------------------------------------------------------------------------------------------------
//
// not ported: `HeadlessBoardManager.scheduleDeferredPostLoadProcessing` (`:759-784`, 26 lines) —
// a **virtual thread** started at `:735` that reads the board while the router mutates it: it
// builds a whole `BoardStatistics`, re-serialises the design to DSN and calls
// `compareCounterpartBoardIfPresent`. Quirk label AE (row #240): a data race whose only output is
// log lines the port does not emit, and whose *timing* is not reproducible on either side.
// not ported: `HeadlessBoardManager.compareCounterpartBoardIfPresent` (`:172-207`, 36 lines) and
// `loadBoardFromFileForComparison` (`:209-237`, 29 lines) — they load a **second whole board from
// disk** to log a diff, through `board/state/BoardComparator` (rostered below).
// not ported: `HeadlessBoardManager.validatePowerPlanes` (`:914-1010`, 97 lines),
// `conductionAreasOverlap` (`:879-898`, 20) and `getConductionAreaNetNames` (`:900-912`, 13) —
// called once from `:756`, inside the same deferred pass, and producing only warnings.
//   What of this class the port DOES have: the three clearance overrides landed in Plan 7 Task 15b
//   (`crates/fr-board/src/board/clearance_override.rs`, driven by
//   `fr_router::pipeline::prepare_board`), and the load/save sequence is Plan 8 Task 3's.
//
// renamed: `HeadlessBoardManager.getRoutingBoard` (`:255-257`), `replaceRoutingBoard` (`:276-278`)
// and `getCurrentRoutingJob` (`:570-572`) — there is no manager **object** in the port. The board
// is the `&mut Board` parameter [`RoutingPipeline::run`] takes and the `Board` its caller owns; the
// job is Task 1's `RoutingJob`, passed around the call rather than held in a field. See
// [`Ctx`]'s doc comment, which records the same three.
//
// -------------------------------------------------------------------------------------------------
// 8. `Freerouting.java`'s server, bridge and board-comparison halves.
// -------------------------------------------------------------------------------------------------
//
// not ported: `Freerouting.initializeAPI` (`:399-529`, 131 lines) and `stopApiServer` (`:532-540`,
// 9) — they build and tear down the Jetty server for the rostered `api/**`. Ruling AU.
// not ported: `Freerouting.initializeMCP` (`:549-673`, 125 lines) — the same for `api/mcp/**`.
// Ruling AO; the port's server is native and is `crates/freerouting/src/mcp`.
// not ported: `Freerouting.splitCommaSeparated` (`:790-795`, 6 lines) — a CORS-origin helper whose
// only two call sites are `:459` and `:602`, both inside the two methods above.
// not ported: `Freerouting.compareBoardFiles` (`:826-863`, 38 lines) and `loadBoardFromFile`
// (`:865-892`, 28) with the `--compare-boards=` flag they serve (`:1294`). **Ruling AS.**
//
// -------------------------------------------------------------------------------------------------
// 9. Ruling AS's three classes — a board differ and a dead scoring pair.
// -------------------------------------------------------------------------------------------------
//
// not ported: `board/state/BoardComparator` in full (758 lines) — ruling AS. Its two callers are
// both rostered: `Freerouting.java:848-849` (the `--compare-boards=` path above) and
// `management/HeadlessBoardManager.java:193-194` (the deferred comparison pass above).
//     $ grep -rn "BoardComparator" src/main/java | grep -v board/state/BoardComparator.java
//     Freerouting.java:848,849 · management/HeadlessBoardManager.java:193,194
//   Cross-referenced by the Plan-8 deferral markers at `crates/fr-board/src/board/mod.rs:57`
//   and `crates/fr-drc/src/lib.rs:144`, which Task 14 re-points.
//
// not ported: `core/scoring/BoardScoreBreakdown` (192 lines) and
// `core/scoring/ScoringWeightComparison` (232 lines) — ruling AS, on **two** independent grounds.
//   (a) Reachability: their only caller outside each other is a JUnit test.
//     $ grep -rn "ScoringWeightComparison\|BoardScoreBreakdown" src/main/java src/test/java \
//         | grep -v 'core/scoring/BoardScoreBreakdown.java\|core/scoring/ScoringWeightComparison.java'
//     src/test/java/app/freerouting/core/scoring/ScoringWeightComparisonTest.java  (and nothing else)
//   Plan ruling 12 therefore does **not** port that suite: it is the sole reachability of the two
//   rostered classes, and porting it would manufacture the caller the roster says does not exist.
//   (b) A known-divergent formula, quirk label X (row #236): `BoardScoreBreakdown.of:140` reads
//   `stats.traces.totalLength` — **raw board units** — while its own javadoc says millimetres and
//   the live `BoardStatistics.calculateScore:608-609` prefers `totalLengthMm`. The two disagree by
//   the DSN resolution factor on every board whose resolution is not 1.
//   Cross-referenced by the five Plan-8 deferral markers at
//   `crates/fr-router/src/score/mod.rs:54-58`, which Task 14 re-points.
//
// -------------------------------------------------------------------------------------------------
// 10. Out of scope by spec §2 — the GUI, the Eagle writer, the debug console.
// -------------------------------------------------------------------------------------------------
//
// not ported: `gui/**` in full — 163 files, 35 691 lines. Spec §2: there is no GUI. (Not counted
// in the ≈ 13 500 budget, which is the *headless* roster; the GUI was never in scope for any plan.)
// not ported: `debug/DebugControl` in full (324 lines) — the interactive step/pause/fast-forward
// console. Its readers are `logger/FRLogger.java:422,429` (and `FRLogger` itself is dropped by
// spec §2) and six `gui/board/BoardToolbar.java` call sites. Cross-referenced by
// `crates/fr-router/src/lib.rs:243`'s existing `// not ported: DebugControl`.
// not ported: `io/specctra/parser/SessionToEagle` in full (627 lines) — spec §2 keeps the Specctra
// SES writer and drops every other export format. **Not dead in Java**: `io/specctra/SesReader.java:107`
// calls `SessionToEagle.getInstance(...)`, so this is an out-of-scope port decision, not a
// reachability claim, and the roster line says which. The marker at `crates/fr-dsn/src/lib.rs:12`
// was re-worded from its Plan-8 deferral marker to `not ported:` by this task.
//
// -------------------------------------------------------------------------------------------------
// 11. Already rostered elsewhere — cross-referenced, not re-rostered.
// -------------------------------------------------------------------------------------------------
//
// `settings/{ApiServerSettings, ApiServerEndpoint, McpServerSettings, McpServerEndpoint, …}`
// (~260 lines) are rostered by **Plan 4** in `crates/fr-settings/src/lib.rs`. Listed here so a
// reader of this roster does not conclude they were forgotten; the evidence lives with the marker
// that owns them.
//
// The five multithread classes (`BatchAutorouterThread`, `AutoroutePassRunner.runMultiThread`,
// `BatchAutorouter.autoroutePassMultiThread`, `BatchOptimizerMultiThreaded`, `OptimizeRouteTask`)
// are rostered by **Plan 7** in `crates/fr-router/src/lib.rs`, with quirk #143's greps. Controller
// ruling AQ closes the product question: `-mt` stays parsed and dead.

// -------------------------------------------------------------------------------------------------
// 12. The roster, method by method — what `scripts/audit-port.sh` actually reads
// -------------------------------------------------------------------------------------------------
//
// The prose above is the *reason*; `audit-port.sh` is line-based and matches
// `not ported: <Method>` / `renamed: <Method>` with the Java method name on the same line
// (Plan 2 ruling 13). So every public method of every class Task 0 rosters gets its own line
// here. A class whose rows are *not* here yet belongs to a later task — the map
// (`scripts/audit-map/fr-core.map`) says which — and reports `MISSING` until that task lands,
// which is the obligation working as designed.
//
// ── core/RoutingJobPriority — the value nothing reads (see §6) ──────────────────────────────────
// not ported: RoutingJobPriority.getValue — no caller anywhere; ordering uses `ordinal()`
// (`core/RoutingJob.java:411-413`).
//
// ── core/events/** — ruling AK's replaced mechanism (see §5) ────────────────────────────────────
// not ported: BoardFileDetailsUpdatedEvent.getDetails
// not ported: RoutingJobLogEntryAddedEvent.getJob
// not ported: RoutingJobLogEntryAddedEvent.getLogEntry
// not ported: RoutingJobUpdatedEvent.getJob
//
// ── core/scoring — ruling AS's dead pair (see §9) ───────────────────────────────────────────────
// not ported: BoardScoreBreakdown.toSummaryString
// not ported: ScoringWeightComparison.compare
// not ported: ScoringWeightComparison.isCandidateBetter
// not ported: ScoringWeightComparison.toReportString
//
// ── core/scoring/BoardStatistics — the four score methods are fr-router's, re-exported ──────────
// renamed: BoardStatistics.calculateScore -> `fr_router::score::calculate_score` (plan 7 Task 1).
// renamed: BoardStatistics.getMaximumScore -> `fr_router::score::maximum_score`.
// renamed: BoardStatistics.getNormalizedScore -> `fr_router::score::normalized_score`.
// renamed: BoardStatistics.isPinEscaped -> `fr_router::score::is_pin_escaped`.
// (`BoardStatistics.toString` and the byte-scraping constructor are **Task 2's**, and their
// deferral markers are at `crates/fr-router/src/score/mod.rs:51-53`.)
//
// ── management/HeadlessBoardManager — the manager-less accessor (see §7) ────────────────────────
// renamed: HeadlessBoardManager.getCurrentRoutingJob -> `fr_core::RoutingJob` (Task 1), passed to
// the call rather than held in a manager field. The other five public methods are **Task 3's**.
//
// ── management/jobs/RoutingJobScheduler — the queue half (see §4) ───────────────────────────────
// not ported: RoutingJobScheduler.getInstance — a static field initialiser that starts a daemon.
// not ported: RoutingJobScheduler.enqueueJob — quirk label AA (row #238).
// not ported: RoutingJobScheduler.saveJob — gated on `featureFlags.saveJobs`, called from api/v1.
// not ported: RoutingJobScheduler.getQueuePosition — **no caller anywhere**.
// not ported: RoutingJobScheduler.listJobs — gui/board + api/v1 only.
// not ported: RoutingJobScheduler.getJob — api/v1 only.
// not ported: RoutingJobScheduler.clearJobs — gui/board only.
// not ported: RoutingJobScheduler.cancelJob — api/v1 only.
//
// ── management/sessions/SessionManager — the whole class (see §4) ───────────────────────────────
// not ported: SessionManager.getInstance
// not ported: SessionManager.createSession
// not ported: SessionManager.getSession
// not ported: SessionManager.getSessions
// not ported: SessionManager.listSessionIds
// not ported: SessionManager.removeSession
// not ported: SessionManager.getActiveSessionsCount
// not ported: SessionManager.getPrimarySession
// not ported: SessionManager.setPrimarySession
// not ported: SessionManager.getMonitoredSessionId
// not ported: SessionManager.setMonitoredSessionId
//
// ── api/** — ruling AU (see §1). Every one of these is a Jersey/JAX-RS entry point. ─────────────
// not ported: ApiAnalyticsFilter.filter
// not ported: ApiExceptionMapper.toResponse
// not ported: ApiRateLimitFilter.filter
// not ported: ApiUsageFilter.filter
// not ported: ApiUsagePaths.isUsageTrackingExcluded
// not ported: ApiUsagePaths.normalizeRoute
// not ported: AppContextListener.contextInitialized
// not ported: AppContextListener.contextDestroyed
// not ported: BaseController.getAuthenticatedPrincipal
// not ported: BaseController.setHttpHeaders
// not ported: BaseController.setUserIdOverride
// not ported: CorrelationIdFilter.filter
// not ported: CorrelationIdFilter.resolveOrCreate
// not ported: EnvironmentHostValidationFilter.filter
// not ported: FixedWindowRateLimiter.check
// not ported: FixedWindowRateLimiter.Decision — a nested record, which `audit-port.sh` reads as a
// method of the outer class.
// not ported: FreeroutingApplication.getClasses — the registry `JobControllerV1` is absent from.
// not ported: GsonMessageBodyHandler.isReadable
// not ported: GsonMessageBodyHandler.isWriteable
// not ported: GsonMessageBodyHandler.readFrom
// not ported: GsonMessageBodyHandler.writeTo
// not ported: JsonStringMessageBodyWriter.isWriteable
// not ported: JsonStringMessageBodyWriter.writeTo
// not ported: NotFoundExceptionMapper.toResponse
// not ported: OpenApiResource.getOpenApiJson
// not ported: OpenApiResource.getOpenApiYaml
// not ported: SwaggerUIResource.getSwaggerUi
// not ported: SwaggerUIResource.getSwaggerUiIndex

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_parity_version_is_the_head_jars() {
        // Read from the jar at port time — see PARITY_VERSION's doc for the command.
        assert_eq!(PARITY_VERSION, "2.3.1-SNAPSHOT");
        assert_eq!(PARITY_BUILD_DATE, "2026-09-01");
        assert_eq!(PARITY_JAR_REVISION.len(), 40);
    }

    #[test]
    fn the_server_version_is_the_crates_own_and_not_the_jars() {
        // Ruling AT: CARGO_PKG_VERSION reaches the MCP `serverInfo` and nothing else.
        assert_eq!(SERVER_VERSION, env!("CARGO_PKG_VERSION"));
        assert_ne!(SERVER_VERSION, PARITY_VERSION);
    }
}
