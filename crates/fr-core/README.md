# `fr-core`

The composition layer between `fr-router` and the `freerouting` binary — spec §4's
*"RoutingPipeline, CancelToken, ProgressSink, RoutingResult, BoardStatistics, result manifest"*.

If you want to **route a board**, this is the crate you call. Everything below it is the router,
the design-rule checker, the readers and the settings ladder.

> **State: Plan 8 Task 4.** The cancel/progress seams, `Ctx`/`RoutingResult`,
> `RoutingPipeline::run`, the timeout ladder, `PARITY_VERSION` and the whole `// not ported:`
> roster (Task 0), the job model (Task 1), the byte-scraping statistics twin (Task 2), the
> **board load/save sequence** (Task 3, `load.rs` + `save.rs`) and the **result manifest**
> (Task 4, `manifest.rs`) have landed; `core`, `core/results`, `management/`, `management/jobs`
> and `management/sessions` are all at 0 MISSING and 0 UNMAPPED. The board summary (Task 12) is
> still to come. `scripts/audit-map/fr-core.map` records which task owes which rows.

## One Java fact this crate overturned

Plan 8's survey (ruling AD) and quirk register row #232 both said the two clearance overrides run
**twice** per DSN load — from `HeadlessBoardManager.createBoard:342-343` and again from
`applyRouterSettingsForLoadedBoard:746-747`. Task 3 measured it and they run **once**:
`HeadlessBoardManager.createBoard` is unreachable from the DSN parser, whose only
`BoardParserCallback` is `ReadScopeParameter$MinimalBoardManager`, which builds the board without
either override. So `fr_router::pipeline::prepare_board` is called once from
`apply_router_settings_for_loaded_board`, and that one call **is** Java's one call — there is no
divergence left to weigh. The evidence is `scripts/differential/java/probes/P8T3Probe.java`'s
`[createboard]` rows (`headless_create_board_calls=0`, through a counting subclass of the real
manager) and `tests/data/p8t3-clearance-overrides.txt`'s 54 stage blocks, which
`tests/overrides.rs` replays cell for cell — search-tree leaf count and `ShapeTree.toArray()`
order digest included. Quirk #253 records the dead method; quirk #232 is rewritten.

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
precisely what Plan 7's hand-off warned Plan 8 against. **Plan 7's hand-off asked that the CLI's
help text say so, and Plan 8 Task 14 discharged it**: the port has no `--max-items` flag of its
own, so the surface for it is the generic priority-60 override — `--set router.max_items=N` on the
native form and `--router.max_items=N` on the legacy one (controller ruling **BJ**; neither form
takes the other's spelling, because `clap` has no arm for the dotted one and the jar ignores
`--set`). The paragraph explaining that the two limits take different arms of the stop flag is on
`--set`'s help text in `crates/freerouting/src/cli.rs`, repeated in
`crates/freerouting/README.md`.

`apply_to()` writes `ALL` first and `AUTO_ROUTER_ONLY` second, because `request_stop_auto_router`
is a one-way upgrade from `None`: the order is the one Java's own two call sites can produce.

**An uncancelled token is a no-op.** `CancelToken::default().as_router_stop()` is
indistinguishable from `RouterStop::new()`, and `apply_to` on it writes nothing. That is what
keeps `batch_parity`, `p6t1` and `sweep-p7t9.sh` byte-identical once the poll seam is added.

### The poll seam, and the four sites that close it

Scan ruling R3: the committed tree has **one** `poll_deadline` call site
(`crates/fr-router/src/pipeline/batch_loop.rs:303`), not the six the plan drafted, and
`run_pipeline` offers no closure hook. The *addition* of polls is an additive-and-wrapped,
driver-pinned `fr-router` change that **controller ruling BB assigns to Task 11** — the seam's
first consumer, with Task 12 the second. Task 0 builds the token and the two entry points that
seam calls (`apply_to`, `as_router_stop`), and recorded the gap as an `obligation:` at
`CancelToken::apply_to`: *until Task 11 lands it, a cancel that arrives after `run_pipeline` is
entered is not observed by that run.*

**That obligation is CLOSED.** Task 11 landed ruling BB's three poll sites — the pass-loop heads
in `pipeline::{batch_loop, fanout, optimizer}` — and Task 12 added ruling AI's sanctioned
**fourth**, the item loop in `pipeline::pass_runner`, after measuring that one pass of
`fixtures/Issue508-DAC2020_bm01.dsn` costs **135 s** in release and an operator's
`notifications/cancelled` therefore took over two minutes to be observed. All four are additive
and wrapped: `batch_parity`, `run.sh p6t1` (all six rows) and `sweep-p7t9.sh` are byte-unchanged,
which is scan ruling R3's gate.

## The load sequence — the four steps, and which two `resolve_headless` owns

*(Survey §5.7's gap, closed. `management/` had never been audited by any plan, which is how it
survived six of them; `scripts/audit-port.sh management crates/fr-core/src` is now one of Task
14's twelve invocations, at zero `MISSING` and zero `UNMAPPED`.)*

Loading a board is `HeadlessBoardManager.loadFromSpecctraDsn` (`:673-705`) →
`applyParsedBoardResult` (`:711-737`) → **`applyRouterSettingsForLoadedBoard` (`:739-749`)** →
`applyImmediatePostLoadProcessing` (`:751-757`), plus `BoardLoader.loadBoardIfNeeded` (`:19-56`)
in front. The port carries all five as **free functions in `load.rs`** — there is no manager
object, because the class's other ≈ 640 lines are diagnostics and are rostered.

The third of those five is the one that matters, and it has four steps that split two and two:

| Java | step | who owns it in the port |
|---|---|---|
| `:740-744` | if the board's layer count differs from the settings', write the board's into the settings | **settings** — `fr_core::apply_router_settings_for_loaded_board`, and `fr_settings::resolve_headless` |
| `:745` | `routerSettings.applyBoardSpecificOptimizations(board)` | **settings** — the same two |
| `:746` | `applyCopperToEdgeClearanceOverride()` | **board** — `fr_router::pipeline::prepare_board` → `fr_board::Board::apply_copper_to_edge_clearance_override` |
| `:747` | `applyHoleClearanceOverride()` | **board** — the same, → `apply_hole_clearance_override` |

**The two settings steps have two callers and one implementation each.** `resolve_headless` runs
them at `crates/fr-settings/src/resolve.rs:274-284` because it models the whole scheduler flow
(merge #1 → this pass → merge #2); `load.rs` runs them because it is the loader. They are **not**
nested — `load.rs` does not call `resolve_headless`, whose signature needs a `SettingsInputs`
ladder and a `HostEnvironment` that no loader has, and calling it there would re-run both merges.
Task 6 settled the order at the one place the two meet, the CLI: parse the board, call
`resolve_headless` **once** against the parsed board, then run `load.rs`'s two board passes with
the resolved settings. `crates/fr-core/tests/load.rs::the_settings_pass_is_the_same_two_steps_resolve_headless_runs`
asserts the two agree, so the duplication cannot drift.

**The two board steps run ONCE per load, and the survey said twice.** Plan-8 survey label **AD**
and register row **#232** both claimed `HeadlessBoardManager.createBoard:342-343` runs the
overrides a first time, from the parser at `Structure.java:1268`. Measured at the pinned jar, that
is false and cannot be true: `Structure.java:1268` calls `scopeParameter.boardHandling.createBoard`
on a `final BoardParserCallback` field, `HeadlessBoardManager` implements `BoardManager` which does
**not** extend `BoardParserCallback`, and the field's one assignment in the whole tree is
`new MinimalBoardManager()` (`ReadScopeParameter.java:103`), whose `createBoard` calls neither
override. `P8T3Probe`'s `[createboard]` rows measure `headless_create_board_calls=0` on all three
fixtures. That is quirk **#253**, and #232's text was corrected in place around it.

## Job deadlines

`Deadline` has one instant: when routing must stop. `RoutingPipeline` records the reason that a
run stopped as either `Deadline` or `Cancelled`; stage-local fanout and optimizer limits remain
separate pipeline outcomes. CLI and MCP callers use that reason directly, so a cooperatively
stopped job reports `TIMED_OUT` as soon as it returns a partial board.

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

## The result manifest (`manifest.rs`)

`RoutingResultManifest` is the `--router.result_json=<path>` document: thirteen keys in Java's
field declaration order, written through the same `GsonProvider.GSON` the settings and the
statistics go through, with no trailing newline. **The Rust field order is the JSON key order** —
serde's derived `Serialize` streams a struct in declaration order — so reordering the struct
changes the file.

Three things about it are worth knowing before reading the code.

**It is serde, and that is not a contradiction of Task 2's warning.** The warning is about
`serde_json::Value`: its `Map` is a `BTreeMap` without `preserve_order` (keys come back
alphabetised) and its `serialize_f32` widens to `f64` (`Float.toString`'s shorter text is lost).
Neither loss is serde's — a derived `Serialize` fed straight into
`fr_dsn::format::json::to_gson_string_pretty` keeps both. The one subtree serde could not carry
alone, `board_statistics`, goes through Task 2's `GsonBoardStatistics` with `serialize_with`, so
its key order and float widths stay Task 2's. `normalized_score: 572.4359` in the committed
`p8t2` transcript is what proves the `f32` path end to end: an `f64` would have printed
`572.4359130859375`.

**The SHA-256 is hand-written, and the reason is in `Cargo.lock`.** `sha256Hex` needs one and the
Global Constraint forbids adding a dependency; the lock carries no `sha2`, `ring`, `digest` or
`openssl`, so the FIPS 180-4 core is sixty lines in `manifest.rs`, pinned against the three NIST
example vectors, the empty message and the 55/56/63/64/65-byte padding boundaries.

**`resolveGitSha`'s ladder loses one rung in translation.** Java walks env `FREEROUTING_GIT_SHA`,
then system property `freerouting.git.sha`, then system property `FREEROUTING_GIT_SHA`. The port
has no system properties, so both property arms become environment lookups **of the same names** —
and the third one then collides with the first, which makes it unreachable *and* promotes it above
the second. Two `p8t2` rows carry both answers as `XDIFF`s rather than hiding the rename. A third
detail that is not cosmetic: `String.isBlank()` and `String.trim()` do not agree with Rust's
`char::is_whitespace`/`str::trim` in either direction, so `java_is_blank` and `java_trim` are
written out. The difference is **eight characters and no more** — `U+001C`-`U+001F` one way,
`U+0085` and the three non-breaking spaces the other — and that list is a JVM sweep rather than a
guess: `whitespace_sets_are_the_measured_ones` asserts the predicate against
`Character.isWhitespace` at every code point. A git sha of one non-breaking space (or one
`U+0085`) is returned **unchanged** by Java and would have become the empty string here. The
`U+0085` half was missed in the first version and caught in review — which is why the pin is now
the whole set rather than the four cases somebody thought of.

### Five quirks, one totalised

| id | what |
|---|---|
| #254 | `phases.fanout` and `phases.optimizer` are allocated and never written, and `phases.autorouter.duration_seconds` is the **whole job's** duration |
| #255 | `sha256Hex` swallows every failure into `null` and Gson drops the key |
| #256 | `resource_usage.io_read`/`io_written` are never assigned by anything and are still always written as `0.0` |
| #257 | `fromJob:107` NPEs on the filesystem root, out of a `try` that catches `IOException` only — **totalised** |
| #258 | `new RouterSettings()` plus a board throws out of `getMaximumScore`, because `new ScoringSettings()` leaves every weight `null` |

#258 also records a **port** defect the same measurement found: `RoutingJob::default` used
`RouterSettings::default()` (every field `None`) where `RoutingJob.java:105` is
`new RouterSettings()` (three nested objects allocated). It is `RouterSettings::new()` now, and
the manifest's `settings_snapshot` is `{"fanout": {}, "optimizer": {}, "scoring": {}}` again
rather than `{}`.

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
