# `fr-core`

The composition layer between `fr-router` and the `freerouting` binary — spec §4's
*"RoutingPipeline, CancelToken, ProgressSink, RoutingResult, BoardStatistics, result manifest"*.

If you want to **route a board**, this is the crate you call. Everything below it is the router,
the design-rule checker, the readers and the settings ladder.

> **State: Plan 8 Task 0.** The cancel/progress seams, `Ctx`/`RoutingResult`,
> `RoutingPipeline::run`, the timeout ladder, `PARITY_VERSION` and the whole `// not ported:`
> roster have landed. The job model (Task 1), the byte-scraping statistics twin (Task 2), the
> load/save sequence (Task 3), the manifest (Task 4) and the board summary (Task 12) are still to
> come. `scripts/audit-map/fr-core.map` records which task owes which rows.

## What this crate is not

**It is not a new home for anything `fr-router` owns.** Plan-8 ruling 1 (from plan-7 controller
answer 3) chose *re-export* over relocation: `fr_core::{BoardStatistics, RoutingEvent,
ProgressSink, PassRecord, TaskState, RouterStop, RouterBudget, …}` are `pub use fr_router::…`, and
`RoutingPipeline::run` **wraps** `fr_router::pipeline::run_pipeline`. Nothing in `fr-router` moved
file for Plan 8.

The cost is stated rather than hidden: this crate is *partly a façade*. A reader looking for where
a routing decision is made will find it in `fr-router`, even though the call came through here.
The benefit is that the last plan of the port did not touch the one thing that must not move.

That rule bites twice in Task 0 alone, and both are recorded in the task report:

* `TextManager.parseTimespanString` was **already ported** by Plan 7 Task 12, as
  `fr_router::pipeline::parse_timespan_seconds` (`BatchFanout` and `BatchOptimizer` are two more
  readers of the same method). `fr_core::parse_timespan_seconds` is a `pub use` of it, not a
  second implementation.
* `core/ProgressThrottler` and `core/StoppableThread` are likewise Plan 7's; this crate carries
  the audit rows and the mapping, not the code.

## `CancelToken` -> `RouterStop`

Controller ruling AP asked for an `Arc<AtomicBool>` copied into `RouterStop` at the poll sites.
Plan ruling 2 corrected the *shape*: `RouterStop` is a **three**-state machine, not a boolean, and
the difference is load bearing.

| `CancelToken` | `RouterStop` | Java | what it means |
|---|---|---|---|
| `cancel()` | `request_stop()` -> `ALL` | `StoppableThread.java:23-25` | the router **and** the optimizer stop |
| `cancel_auto_router()` | `request_stop_auto_router()` -> `NONE -> AUTO_ROUTER_ONLY` only | `:33-37` | the router stops; **the optimizer keeps running** |
| — | `is_stop_requested()` = `== ALL` | `:28-30` | what `RoutingPipeline.java:117` gates the optimizer stage on |
| — | `is_stop_auto_router_requested()` = `!= NONE` | `:40-42` | what the pass loop and the item loop read |

**Quirk #200 is the reason this matters.** `--max-items` reaches `requestStop()` and therefore
*silently disables the optimizer*; `--max-passes` reaches `requestStopAutoRouter()` and does not.
A single `AtomicBool` collapsing the two would change behaviour without changing a test — which is
precisely what Plan 7's hand-off warned Plan 8 against. The CLI's help text must say so
(plan-7 hand-off).

`apply_to()` writes `ALL` first and `AUTO_ROUTER_ONLY` second, because `request_stop_auto_router`
is a one-way upgrade from `None`: the order is the one Java's own two call sites can produce.

**An uncancelled token is a no-op.** `CancelToken::default().as_router_stop()` is
indistinguishable from `RouterStop::new()`, and `apply_to` on it writes nothing. That is what
keeps `batch_parity`, `p6t1` and `sweep-p7t9.sh` byte-identical once the poll seam is added.

### The poll seam is a later task

Scan ruling R3: the committed tree has **one** `poll_deadline` call site
(`crates/fr-router/src/pipeline/batch_loop.rs:303`), not the six the plan drafted, and
`run_pipeline` offers no closure hook. The *addition* of polls is an additive-and-wrapped,
driver-pinned `fr-router` change that **controller ruling BB assigns to Task 11** — the seam's
first consumer, with Task 12 the second. Task 0 builds the token and the two entry points that
seam calls (`apply_to`, `as_router_stop`), and records the gap as an `obligation:` at
`CancelToken::apply_to`: **until Task 11 lands it, a cancel that arrives after `run_pipeline` is
entered is not observed by that run.**

## The deadline, and Java's monitor thread

The port has **no watchdog thread** (ruling AI). `Deadline` is the monitor's *observable* effect,
which is two instants rather than one:

| Java | `Deadline` |
|---|---|
| `job.thread.requestStop()` at `RoutingJobSchedulerActionThread.java:75` | `stop_at` |
| `job.state = TIMED_OUT` at `:84`, one `GRACE_PERIOD` later | `timed_out_at` |

By the time anything can read `TIMED_OUT`, the flag is already `ALL` — which is why
`AutorouteBatchLoop.java:251-253`'s `requestStopAutoRouter()` is dead code (quirk #203).

The ladder that builds it, `threadAction:43-52`, is exactly: parse; clamp **from above only** at
`MAX_TIMEOUT` (`:24` = `24 * 60 * 60`); `startedAt.plusSeconds(timeout)`. There is **no lower
clamp**, so `--job-timeout -1` is a deadline in the past — measured, not reasoned
(`tests/data/p8t0-timespans.txt`, row `"-1"`). `GRACE_PERIOD` (`:25` = `30`) is *not* applied
there; it lands on `timed_out_at` and never on `stop_at`.

## `Ctx` has no rng seed and no `max_threads`

Spec §10 mentions both. The port has neither, and this is the record rather than two fields
nothing sets:

* **No seed.** Plan-6 ruling 5 forbids `rand`, and there is no randomness anywhere in the ported
  router. Java's one random knob, `ItemSelectionStrategy.RANDOM`, is GUI-only.
* **No `max_threads`.** Quirk #143, as extended by Plan 7 Task 17: `RouterSettings.maxThreads` and
  `optimizer.maxThreads` have **no live reader anywhere** in the Java tree — not merely none on
  the headless path. Controller ruling AQ keeps `-mt` parsed and dead. The port's only threads are
  the MCP transport's two, and they carry no routing policy.

`Ctx` also holds no board and no job, which is why `HeadlessBoardManager`'s `getRoutingBoard` /
`replaceRoutingBoard` / `getCurrentRoutingJob` are `// renamed:` rather than deferred: there is no
manager *object* in the port at all. The board is `RoutingPipeline::run`'s `&mut Board` parameter,
owned by whichever caller loaded it.

## `RoutingPipeline::run` composes; it does not decide

Three additions over `run_pipeline`, and **no fourth**:

| addition | Java |
|---|---|
| the `CancelToken` adaptation | ruling AP; `RoutingJobSchedulerActionThread.java:75` |
| the final board's DRC violations | `Freerouting.java:277-294`'s checker |
| the unrouted report | Plan 7's `build_unrouted_report` |

And one deliberate **non**-addition: `RoutingResult::stats` **is**
`PipelineResult::final_statistics`, never a second `BoardStatistics::compute`. `BoardStatistics`'
constructor runs `DesignRulesChecker` twice (`core/scoring/BoardStatistics.java:265-268`,
`:338-341`), and Plan 5's memoisation makes the call count observable — so recomputing would be a
behaviour change wearing the costume of a convenience.

Scan ruling R4: `RoutingResult` carries `unrouted_report: String`, the jar's own diagnostic text,
not a port-only structured `incompletes` vector. The **count** comes from
`stats.connections.incomplete_count`.

## `PARITY_VERSION`

One constant, read out of the HEAD jar rather than guessed, with the jar's revision beside it:

```
PARITY_VERSION      = "2.3.1-SNAPSHOT"                              (Constants.FREEROUTING_VERSION)
PARITY_BUILD_DATE   = "2026-09-01"                                  (Constants.FREEROUTING_BUILD_DATE)
PARITY_JAR_REVISION = "278fe14123c49376667239659c98d41a597acce9"    (META-INF/MANIFEST.MF Build-Revision)
```

It has **three readers**, and the SES/DSN is not one of them (plan ruling 5 correcting ruling AT):
the DRC report's `freerouting_version` (`drc/DesignRulesChecker.java:213`), the manifest's
`app_version` (`core/results/RoutingResultManifest.java:102`) and `BoardStatistics.host`'s
unreachable fallback (`:119`, quirk label AK). The SES/DSN `(hostCad …)`/`(hostVersion …)` are
echoed back **from the input file** and never carry the app version.

`SERVER_VERSION` (`CARGO_PKG_VERSION`) reaches the MCP `serverInfo` and nothing else: the port is
a new server and says so.

## The roster

The `// not ported:` block at the foot of `src/lib.rs` is ≈ 13 500 Java lines, in twelve sections,
**each with the grep that proves it** — re-run against the clone's HEAD at port time and pasted
into the Task 0 commit message (plan-7 ruling 6's precedent). A roster line with no evidence is a
defect.

| § | what | ruling | lines |
|---|---|---|---|
| 1 | `api/**` (incl. `JobControllerV1`'s provably-dead 659) | AU | 8 425 |
| 2 | `api/mcp/**` + the Java stdio bridge | AO | 2 105 + 108 |
| 3 | `analytics/**`, `util/VersionChecker` | spec §2 | 2 228 + 119 |
| 4 | `SessionManager`, `Session`, the scheduler's queue half, the monitor thread | quirk labels AA/AB/Y | 252 + ~500 |
| 5 | `core/events/**` | AK | 95 |
| 6 | `RoutingJobPriority`, `RoutingStage` | — | 30 |
| 7 | `HeadlessBoardManager`'s deferred/comparison/validation halves | quirk label AE | ~220 |
| 8 | `Freerouting.java`'s server, bridge and compare halves | AU/AO/AS | ~330 |
| 9 | `BoardComparator`, `BoardScoreBreakdown`, `ScoringWeightComparison` | AS | 1 182 |
| 10 | `gui/**`, `DebugControl`, `SessionToEagle` | spec §2 | 627 (+ GUI) |
| 11 | the settings and multithread families | Plan 4 / Plan 7 | cross-referenced |
| 12 | the same roster **method by method**, which is what `audit-port.sh` reads | — | — |

Section 12 exists because `audit-port.sh` is line-based and matches `not ported: <Method>` with the
Java method name on the same line (Plan 2 ruling 13). Sections 1–11 are the reasons; section 12 is
what the tool reads.

**Two counts in the plan were stale and the file wins:** `analytics/**` is **2 228** lines, not
2 100, and `core/events/**` is **95**, not 130. And `api/**`'s 8 425 already *includes*
`api/mcp/**`'s 2 105 — adding them double-counts.

## The job model (`job.rs`, `file_details.rs`)

`RoutingJob` in Java is three objects wearing one name. This crate ports **one** of them:

| half | where it went |
|---|---|
| the **file** object — input, rules, derived output, format detection | `job.rs` / `file_details.rs` (Task 1) |
| the **queue** object — `priority`, `compareTo`, the scheduler, the daemon | rostered, `src/lib.rs` §4/§6 (quirks #238, #239) |
| the **event** object — four listener lists, four `fire*` methods | replaced by spec §10's `ProgressSink` |

`FileFormat` is **`io/FileFormat.java`** and it has **nine** values, not the plan draft's seven
(scan ruling R17): `SCR` and `FRB` were missing and both are reachable from a `-de`/`-do` argv.
Its audit home stays `scripts/audit-map/fr-dsn.map` (`io/` is Plan 3's surface); its code home is
`job.rs`, beside its two producers.

`Session` collapses to `SessionId` (a 128-bit newtype) plus `validate_session_host`: the class
exists so `enqueueJob:315-318` can validate a `userId` it never uses (quirk #238), so the port
keeps the **validation** and drops the object.

### Six quirks, four of them totalised

| # | what | the port |
|---|---|---|
| **#241** | `getFileFormat(byte[])`'s shift loop never refills `buffer[5]`; six leading CR/LF bytes spin for ever | the loop is bounded at **five** iterations (exact) and answers `UNKNOWN`; `FileFormat::java_shift_loop_hangs` says which inputs, and the driver measures both sides |
| **#242** | `changeFileExtension` NPEs on every bare filename, and returns a *relative* string when the extension already matches | both nulls totalised to `""`; the asymmetry **reproduced**, because `setInputFromFile` depends on it |
| **#243** | `tryToSetOutputFile:390` registers the **input** listener on the **output** details | `// not reachable:` — the port has no events |
| **#244** | `isCliTerminalState` omits `INVALID`, so `-de x.ses` hangs the CLI for ever | `INVALID` is terminal (plan ruling 7); Task 6 maps it to exit 1 |
| **#245** | `Session`'s constructor assigns before it validates | the port validates first; the difference is **unobservable** and the row says so |
| **#246** | `setFilename`'s Windows-only surgery runs unconditionally, and `\\.$` strips a backslash **and the character after it** | reproduced verbatim, with `FILE_SEPARATOR` pinned to `'/'`; the `setFilename("/")` NPE is totalised |

`FILE_SEPARATOR` is pinned, and **so is the whole `java_path` module**: it hardcodes `/` as the
separator and `starts_with('/')` as the definition of an absolute path, so `change_file_extension`
(which every derived output name goes through), `set_input`'s absolutisation, `get_absolute_path`,
`get_file` and `from_file` are POSIX-only as well. That is the right call for a parity surface —
the reference is the HEAD jar as it runs on this project's POSIX host, and the committed transcript
is the contract — but **Windows support is a rewrite of `java_path` and a regenerated transcript,
not an unpinning of one constant**.

### There is no random id

Java mints `UUID.randomUUID()` for `RoutingJob.id` and `Session.id`. Plan 6 ruling 5 forbids
`rand` and the Global Constraints forbid static mutable state, so `RoutingJob::new` uses
`Uuid128::NIL` and `RoutingJob::with_id` takes one from a caller that has one (the MCP's
`job_id`, ruling AO). Nothing on a decision path reads an id: `RoutingResultManifest.fromJob`
never touches `job.id`, and the four `FRLogger` calls that do are rostered.


## Evidence

* `tests/data/p8t0-timespans.txt` — 30 timespan inputs through the HEAD jar, with
  `convertFromTimespanToDurationFormat`, `parseTimespanString`, the `MAX_TIMEOUT` clamp and the
  `startedAt.plusSeconds` offset. Regenerate and re-verify with
  `scripts/differential/run.sh p8t0` (**MATCH on all 30 rows**).
* `tests/timespan.rs` asserts against that table as literals, and separately re-reads the
  transcript so a regeneration cannot drift away from the table.
* `tests/pipeline.rs` re-runs plan-7 ruling 11's `a_recording_sink_changes_no_board_byte` through
  the **whole** pipeline, comparing SES bytes rather than a hash.
* `tests/cancel.rs` pins the three-state mapping, the no-op property and a real cross-thread
  cancel on `Issue143-rpi_splitter.dsn`.
* `tests/data/p8t1-job-model.txt` — 154 rows in eight tables through the **real** `RoutingJob`
  and `BoardFileDetails` on the HEAD jar (`P8T1Probe.java` declares `package
  app.freerouting.core` so it can read their `protected` fields, and reaches the private
  `changeFileExtension` by reflection). Regenerate and re-verify with
  `scripts/differential/run.sh p8t1probe` (**MATCH on all 162 lines**) — the driver is
  `p8t1probe`, not `p8t1`, because the plan reserves `p8t1` for Task 6's end-to-end SES-byte
  gate. The six rows Java cannot answer print `XDIFF java=… rust=…` on **both** sides, so the
  divergence is recorded without weakening the diff.
* `tests/job.rs` carries that transcript as literals, re-reads the committed file to check they
  still agree, re-derives every row from the port, and adds 36 named assertions for the branches
  that teach something (the ≥ 6 CR/LF bound, the leading space the loop does not strip, the UTF-8
  BOM, the per-character `(rul` fold, the backslash regex, the relative-return asymmetry).
