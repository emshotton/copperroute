# Plan 8 hand-off — the PROJECT COMPLETION REPORT

**There is no Plan 9, and this document has no successor to address.** Every earlier hand-off in
this directory was written for the plan that came next; this one is written for a reader who
arrives at the repository with no plan in hand and has to answer four questions on their own:
what is ported, what is deliberately not, where the port differs from the jar on purpose, and how
to re-run every check that proves it.

Nothing is deferred out of Plan 8. A thing that could not land is a **recorded, closed decision**
with its evidence — §5 and §7 — never a marker naming a future plan. Two greps are the gate:

```sh
grep -rn "added in Plan 8" crates/*/src crates/*/tests                                   # nothing
grep -rn "added in Plan 9\|added in Plan [0-9][0-9]" crates/*/src crates/*/tests scripts/  # nothing
```

Both return **nothing**, and the second is the one that matters: a *marker* lives in a source
comment, and there is not one anywhere in the code, the tests or the scripts that names a ninth or
later plan.

**The wider sweep the plan's checklist writes — `grep … crates/ docs/ scripts/` — is
self-referential and cannot return zero, which is worth saying out loud rather than quietly
narrowing.** It returns **nine** lines on this tree: eight in the two plan documents
(`docs/plan-8-prep/plan-8-draft.md:5,45,934,1068` and
`docs/superpowers/plans/2026-09-01-plan-8-core-cli-mcp.md:5,79,1033,1167`) and **one** in this
paragraph, which is the sweep command printed above. Every one of the nine is a document
**quoting the pattern in order to state the rule** — six of them are the checklist line that
defines the grep itself. A document that records a gate necessarily contains
the gate's pattern; a plan document is not edited after the fact; and none of the nine is a
marker. The code-scoped form above is the check with an answer.

**Status of the tree this describes.** Branch `plan-8-core-cli-mcp`, `cargo nextest run
--workspace` **2389 passed, 0 failed, 56 skipped**; `cargo clippy --workspace --all-targets -- -D
warnings` clean; `cargo fmt --all --check` clean; `cargo test -p fr-router --test batch_parity`
6 passed, 1 ignored. Quirk register contiguous **1..292**.

---

## 1. What is ported

Eight crates and **2 389** tests, against a Java tree of **130 497** lines of which **35 691**
are the GUI that spec §2 excludes outright. The port's own size, by the command that answers it:
`find crates/*/src -name '*.rs' | xargs cat | wc -l` → **138 776**. *(An earlier draft of this
paragraph also gave a test-source line count with no command beside it; it was not reproducible,
and a number in this document that a reader cannot re-derive is worse than no number. The test
**count** above is `cargo nextest run --workspace`'s own.)*

| crate | Java it ports | public surface | plan |
|---|---|---|---|
| **`fr-geometry`** | `geometry/planar/**` (11 832) | `IntPoint`/`FloatPoint`, `IntBox`/`IntOctagon`/`Simplex`/`PolygonShape`/`Circle`, `Line`, `Polyline`, `Direction`, `Vector`, the convex-split and intersection algebra | **1** |
| **`fr-board`** | `board/model/**`, `board/facade`, `board/searchtree`, `board/trace`, `board/state`, `rules/**`, `core/library`, `datastructures/**` (25 766) | `Board`, `Item` and its eight variants, `ShapeSearchTree`/`MinAreaTree`, `BoardRules`/`ClearanceMatrix`/`NetClass`/`ViaRule`, `Components`/`Packages`/`Padstacks`, `Communication`, the connectivity and normalisation passes | **2** |
| **`fr-dsn`** | `io/specctra/**`, `io/{CoordinateTransform,BoardReadResult,BoardMetadata,FileFormat,KiCadNetClassNames}.java`, `datastructures/{IdentifierType,IndentFileWriter}.java`, **and `io/kicad/{KiCadJsonReader,KiCadJsonWriter,KiCadBoardJson}`** (15 163 of `io/**`) | `read_board`/`read_metadata`, `dsn_writer::write`, `ses_reader`/`ses_writer`, `rules_reader`/`rules_writer`, `DsnScanner`, `CoordinateTransform`, and `kicad::{read_board, import_session, write}` plus its twelve DTOs | **3** (Specctra), **8** (KiCad) |
| **`fr-settings`** | `settings/**` and `settings/sources/**` (4 203), `util/ReflectionUtil`, `util/gson/**` | `RouterSettings` and its five nested structs, `SettingsMerger`, the six sources (`Default` 0, `JsonFile` 10, `DsnFile` 20, `RulesFile` 40, `EnvironmentVariables` 55, `Cli` 60), `resolve_headless` | **4** |
| **`fr-drc`** | `drc/**` (1 448), `io/kicad/KiCadDrc*` (the four report DTOs) | `DesignRulesChecker`, `generate_report`/`generate_report_json`, `KiCadDrcReport` and its three DTOs, `BoardStatisticsClearanceViolations` | **5** |
| **`fr-router`** | `autoroute/**` (16 203), `board/actions`, `board/optimize`, `autoroute/pipeline/**`, `core/scoring/BoardStatistics`, `core/{StopRequestState,StoppableThread,RouterCounters,ProgressThrottler}` | `run_pipeline`, `RouterStop`, `RouterBudget`, `ProgressSink`, `BatchAutorouter`/`BatchFanout`/`BatchOptimizer`, `AutorouteEngine`, `score::*`, `pipeline::prepare_board`, `board_ext::*` | **6** (maze), **7** (pipeline) |
| **`fr-core`** | `core/**` (3 487), `management/**` (2 316) | `RoutingPipeline::run`, `CancelToken`/`Deadline`/`SyncProgressSink`/`Ctx`, `RoutingResult`, `RoutingJob`/`RoutingJobState`/`BoardFileDetails`/`FileFormat`, `RoutingResultManifest`, the load sequence (`load.rs`), `PARITY_VERSION`, the `BoardStatistics` byte-scraping extension | **8** |
| **`freerouting`** | `Freerouting.java` (1 497), and the *decisions* of `api/mcp/**` and `logger/**` | the binary: the legacy shim, the four subcommands (`route`, `drc`, `info`, `mcp`), the exit ladder, the stderr log surface, the native stdio JSON-RPC MCP server and its four tools | **8** (rewritten; untouched since Plan 1) |

The dependency direction is spec §4's, honoured exactly:

```
freerouting -> fr-core -> fr-router -> fr-drc -> fr-board -> fr-geometry
                                fr-dsn -^          ^
                           fr-settings -^----------'
```

`fr-dsn` and `fr-settings` depend only on `fr-board`/`fr-geometry`. **One dev-only edge crosses
the direction**: `fr-drc`'s test binaries dev-depend on `fr-core`, so that `JAR_VERSION` is an
alias for `fr_core::PARITY_VERSION` rather than a second literal (controller sweep item N6). The
shipping `fr-drc` is unchanged.

Every crate root carries `#![forbid(unsafe_code)]`, `fr-core` and the rewritten `freerouting`
included, and no crate added a third-party dependency in this plan.

---

## 2. What is rostered dead, and why

≈ **13 500** headless Java lines are deliberately not ported. They are not silent: the roster is
the `// not ported:` block at the foot of `crates/fr-core/src/lib.rs`, in twelve sections, and
`scripts/audit-port.sh` **reads it** — every public method of every rostered class has its own
marker line, and a class answered entirely by markers prints a `ROSTERED` line so that a
wholly-dropped class is visible rather than passing in silence.

| group | Java | lines | ruling | the evidence that it is dead or out of scope |
|---|---|---|---|---|
| the REST API | `api/**` (incl. `api/mcp/**`) | 8 425 | **AU**, spec §2 | `grep -rn "JobControllerV1\|JobInputResource\|JobOutputResource" src/main/java` — every caller is inside `api/**` or `Freerouting.initializeAPI` (`:394-521`), itself rostered. Nothing in `core/**`, `management/**`, `autoroute/**`, `board/**`, `drc/**`, `io/**` or `settings/**` names any of them |
| the HTTP/SSE/WebSocket MCP transport and the OpenAPI tool registry | `api/mcp/**` | 2 105 (inside the 8 425) | **AO** | Same grep, plus the ten `ROSTERED` lines `audit-port.sh api/mcp crates/freerouting/src` prints. Spec §13 asks for four tools over stdio JSON-RPC; that is `crates/freerouting/src/mcp/**`. The one genuinely ported method is `McpControllerV1.rpc`, `renamed:` to `mcp::server::handle` |
| analytics and the version checker | `analytics/**`, `util/VersionChecker` | 2 228 + 119 | spec §2 | Out of scope by the spec. Their one effect on a CLI run is `Freerouting.main`'s unconditional 1 s sleep after starting them — quirk **#264** |
| the GUI and the debug console | `gui/**`, `debug/DebugControl` | 35 691 + 324 | spec §2 | Never in scope for any plan, and not counted in the 13 500. `DebugControl`'s readers are `FRLogger.java:422,429` and six `gui/board/BoardToolbar.java` sites |
| sessions and the job queue | `SessionManager`, `Session`, the scheduler's queue half, the monitor thread | 252 + ~500 | quirk labels **AA**/**AB**/**Y** (#238/#239/#237) | The CLI runs one job. `enqueueJob` reads `session.userId` **purely to validate it and never uses it** (#238) — the single line that makes `SessionManager` a dependency of the CLI at all. The monitor thread's observable effect is two instants, carried on `fr_core::Deadline` |
| the multithread family | `BatchAutorouterThread`, `AutoroutePassRunner.runMultiThread`, `BatchAutorouter.autoroutePassMultiThread`, `BatchOptimizerMultiThreaded`, `OptimizeRouteTask` | ~450 | **AQ**, quirk **#143** | `BatchAutorouter.autoroutePassMultiThread:411-413` **has no caller anywhere in the tree**; the live path is `AutorouteBatchLoop.java:293` → `:598-600` → `autoroutePass:419-421` → `runSingleThread`. The optimizer's twin is behind `createForGui`. Rostered in `crates/fr-router/src/lib.rs`, and the `autoroute/pipeline` audit prints five `ROSTERED` lines so the drop cannot go quiet |
| the Eagle writer | `io/specctra/parser/SessionToEagle` | 627 | spec §2 | **Not dead in Java** — `SesReader.java:107` calls it — so this is an out-of-scope decision, not a reachability claim, and the roster line says which |
| the two dead scoring classes | `BoardScoreBreakdown`, `ScoringWeightComparison` | 192 + 232 | **AS**, quirk **#236** | `grep -rn "ScoringWeightComparison\|BoardScoreBreakdown" src/main/java src/test/java`, with the two declarations excluded, answers **only** `src/test/java/.../ScoringWeightComparisonTest.java`. No caller in `src/main` at all. Plan ruling 12 therefore declines to port that suite: it is the pair's sole reachability, and porting it would manufacture the caller the roster says does not exist |
| `BoardComparator` | `board/state/BoardComparator` | 758 | **AS** | `grep -rn BoardComparator src/main/java` names only `Freerouting.compareBoardFiles`/`loadBoardFromFile` (the `--compare-boards=` developer flag) and `HeadlessBoardManager.compareCounterpartBoardIfPresent` (`:172-237`, quirk #240's racing diagnostic thread). Both rostered; neither on any `-de`/`-do`/`-drc` path. Plan 8 then **built** the result-manifest/report layer the deferral was pointing at and found no reader there either: `grep -n BoardComparator core/results/RoutingResultManifest.java` is empty |
| `HeadlessBoardManager`'s diagnostic half | six methods | ~220 | quirk **#240** | The deferred post-load pass is a *virtual thread* that reads the board while the router mutates it, and `compareCounterpartBoardIfPresent` loads a **second whole board from disk** to log a diff |
| `FRLogger` and `Log4j2ConfigurationFactory` | `logger/**` | 859 | spec §2 | Not a port — the *message set* is. `crates/freerouting/src/logging.rs` carries the `MESSAGE_MAP` and the five `ROSTERED` lines. The surviving divergence is quirk **#261** (§3) |
| `core/events/**`, `RoutingJobPriority`, `RoutingStage` | | 95 + 30 | **AK**, spec §10 | Replaced by `fr_core::ProgressSink`. `RoutingJobPriority.getValue` has no caller anywhere; ordering uses `ordinal()` (`core/RoutingJob.java:411-413`) |

Two counts in the plan were stale and the file wins: `analytics/**` is **2 228** lines, not 2 100,
and `core/events/**` is **95**, not 130. And `api/**`'s 8 425 already *includes* `api/mcp/**`'s
2 105 — adding them double-counts.

**The audit that proves the roster is complete.** Task 14 ran twelve invocations, three of them
over directories no plan had ever audited (`management/`, `api/`, `core/`) — which is exactly how
the survey's §5.7 gap survived six plans:

```sh
scripts/audit-port.sh core                crates/fr-core/src     '*.java' scripts/audit-map/fr-core.map
scripts/audit-port.sh core/scoring        crates/fr-core/src     '*.java' scripts/audit-map/fr-core.map
scripts/audit-port.sh core/results        crates/fr-core/src     '*.java' scripts/audit-map/fr-core.map
scripts/audit-port.sh core/events         crates/fr-core/src     '*.java' scripts/audit-map/fr-core.map
scripts/audit-port.sh management          crates/fr-core/src     '*.java' scripts/audit-map/fr-core.map
scripts/audit-port.sh management/jobs     crates/fr-core/src     '*.java' scripts/audit-map/fr-core.map
scripts/audit-port.sh management/sessions crates/fr-core/src     '*.java' scripts/audit-map/fr-core.map
scripts/audit-port.sh io/kicad            crates/fr-dsn/src      '*.java' scripts/audit-map/fr-dsn.map
scripts/audit-port.sh api                 crates/fr-core/src     '*.java' scripts/audit-map/fr-core.map
scripts/audit-port.sh api/mcp             crates/freerouting/src '*.java' scripts/audit-map/freerouting.map
scripts/audit-port.sh logger              crates/freerouting/src '*.java' scripts/audit-map/freerouting.map
scripts/audit-port.sh .                   crates/freerouting/src 'Freerouting.java' scripts/audit-map/freerouting.map
```

All twelve exit **0** with **zero `MISSING` and zero `UNMAPPED`**. `ROSTERED` counts: `core` 2,
`core/scoring` 2, `core/events` 3, `management/jobs` 2, `management/sessions` 1, `io/kicad` 1,
**`api` 16**, **`api/mcp` 10**, `logger` 5 — **42 in total, of which 26 are `api/**` + `api/mcp/**`**,
which is what makes the 8 425 consciously-dropped lines visible instead of silent.
`core/results`, `management` and the `Freerouting.java` invocation print nothing at all.

**Scan ruling R22 — the whole-repo sweep.** Two surfaces the twelve do not reach are proved by
re-running two *earlier* plans' invocations rather than by inventing new ones:
`settings/sources/JsonFileSettings.java` is under Plan 4's map and `io/FileFormat.java` under
Plan 3's. Both are now **MAPPED and ported** — scan ruling R7 landed `JsonFileSettings` as a live
priority-10 source (it is no longer `ROSTERED`; the one remaining `ROSTERED` under
`settings/sources` is `GuiSettingsSource`), and scan ruling R17 gave `FileFormat` all nine of its
Java values. The sweep is **32 invocations** covering every ported Java directory in the tree, all
exit 0, all `MISSING=0 UNMAPPED=0`; the copy-pasteable list is in `crates/fr-router/README.md`
under "the whole-repo audit", and Task 14 ran it before and after its own edits to prove the
`ROSTERED` counts did not move.

---

## 3. Every deliberate divergence

Each row is a place where the port knowingly does something the jar does not. Every one cites the
Java site and the ruling or quirk that authorised it, and every one is either pinned by a
differential row or recorded as unassertable.

| # | divergence | Java | authority | how it is held |
|---|---|---|---|---|
| 1 | **All logs go to stderr; there is no log file.** Java's Console appender targets `SYSTEM_OUT`, a second targets `SYSTEM_ERR` at `ERROR`, and the file appender takes both — so an `ERROR` is written three times and `INFO` pollutes stdout | `Log4j2ConfigurationFactory.java:54-113` | quirk **#261** (label AI) | `parity::normalize_log` merges the jar's two streams and dedupes the `ERROR`; every `p8t1`/`p8t3` row compares the two projections |
| 2 | **`drc` with no `-o` writes the report to stdout.** Java's equivalent branch is dead code: DRC mode is entered only when `drcReportFile != null`, so `Freerouting.java:368-371` can never run, and a bare `-drc` is not DRC mode at all | `Freerouting.java:368-371`, `:1462` | plan ruling **6**, quirks **#263**/**#275** | `cli_e2e.rs`, and `p8t3 e2e` compares the *file* mode where the jar has one |
| 3 | **`RoutingJobState.INVALID` is totalised: the port exits 1 where the jar spins for ever.** `isCliTerminalState` omits `INVALID`, which the scheduler assigns for a non-DSN/JSON input | `Freerouting.java:189-194`; `RoutingJobScheduler.java:83, 253` | plan ruling **7**, quirk **#244** | `p8t1`'s `invalid-input-java-hangs` row — the plan's one declared `XDIFF`, not run against the jar for the obvious reason |
| 4 | **The SES is serialised once.** Java re-serialises on every board-updated event (≤ every 250 ms) and again at the end, each time recomputing a CRC32 and a full text-scrape `BoardStatistics` | `RoutingJobSchedulerActionThread.java:100, 168` | plan ruling **9**, quirk **#270** | Not assumed: `p8t1` compares the two programs' SES bytes on **every board in `tests/reference/cli-fixtures.txt`** — thirteen today — and all of them MATCH |
| 5 | **argv is parsed once.** Java parses it **five** times and the passes disagree — `-dl` is `equals` in one and `startsWith` in another, `-ll` takes the first occurrence early and the last late, and `mcp_server.stdio=true` in `freerouting.json` is silently ignored because the stdout redirect must precede logging init | `Freerouting.java:901-920, 944-956, 1056-1065, 1191-1203` | plan ruling **10**, quirk **#262** | `p8t5` and `sweep-p8t5.sh` (86 rows, all MATCH) compare the *classification*, which is what one-parse-vs-five is observable through |
| 6 | **The MCP server is a different program**, in eleven recorded ways | `api/mcp/**`, `Freerouting.java:681-788` | ruling **AO** | The eleven-row table in `crates/freerouting/README.md`, asserted **to be exactly itself** by `run.sh p8t6`: a new delta and a vanished delta both fail the driver |
| 7 | **The seven dead legacy flags stay dead** — `-oit`, `-us`, `-is`, `-hr`, `-inc`, `-drc`'s router switch and `-mt` are parsed and write a bridge nothing reads | `GlobalSettings.java:51-53, 700-736` | ruling **AQ**, quirks **#131**/**#143** | A **product decision**, closed in both register rows. Wiring them would make the port more capable than the program it ports. Each has a live generic override behind it, but the spelling differs by form (row 22): `--set router.optimizer.optimization_improvement_threshold=0.005` on the native form, `--router.optimizer.optimization_improvement_threshold=0.005` on the legacy one |
| 8 | **Exit codes 2 and 3 are port-only, native-form-only.** 2 is clap's usage error; **3 is reserved and unreachable** | — | ruling **AR**, controller answer **3** | `legacy::tests::no_command_runner_answers_not_implemented` walks the crate's own sources and fails if any runner produces it. `grep -rn "ExitCode::NotImplemented" crates/*/src/commands crates/*/src/mcp` is empty |
| 9 | **`--kicad-json` exists only on the native form.** On the legacy form a `.json` takes Java's own slot — design input while no `.dsn` has been seen, session afterwards | `GlobalSettings.java:609-621` | plan ruling **14** | `p8t5`'s `de-json-first` and `de-json-after-dsn` rows, both MATCH |
| 10 | **`-do out.json` writes the board as it was *before* routing** — reproduced, after the plan's hypothesis was refuted by measurement. `setJobOutput`'s first call re-sniffs the format to `KICAD_DESIGN_JSON`, so every later call is a no-op and the file keeps what the **first** event produced | `RoutingJobSchedulerActionThread.java:259-295`; `BoardFileDetails.java:113` | quirk **#289** (label T) | `cli_e2e.rs::do_out_json_writes_the_pre_routing_board`; quirk #270's own text was corrected in place around the measurement |
| 11 | **The MCP bridge's `\r`/`\n` strip is not reproduced.** Java deletes every such byte from the response *body* to protect its line framing | `Freerouting.java:770` | quirk **#292** (label M) | `mcp::stdio::write_line` strips nothing: `serde_json::to_string` cannot emit a raw control character inside a string, so the framing is safe by construction |
| 12 | **Tool input schemas are hand-written, not `schemars`-generated.** Spec §13 asked for the derive | — | ruling **AO**/General | Two tests replace it: a byte-for-byte golden (`tests/data/mcp-schemas.json`) and a **two-way** field-name comparison against `RouterSettings::FIELD_NAMES` and its four nested structs', so neither a settings field nor a schema property can outlive the other |
| 13 | **No threads below `crates/freerouting`.** Exactly **two `std::thread` spawn sites**, both under `crates/freerouting/src/mcp/` | — | plan ruling **3**, scan note N9 | The two spawn sites are `crates/freerouting/src/mcp/stdio.rs:149` (the reader) and `:372` (one thread per in-flight `tools/call`, so N live threads from two sites). **The gate is a spawn-site grep, not a name grep**: `grep -rn "std::thread\|rayon" crates/fr-*/src` answers **8** lines, six of them prose saying the crate has neither, and two of them live code — `crates/fr-settings/src/host.rs:18,22` calls `std::thread::available_parallelism()` to read the processor count `DefaultSettings` needs. That is a *query*, not a thread. The check that answers the claim is `grep -rn "thread::spawn\|rayon::" crates/fr-*/src`, which returns nothing |
| 14 | **`Ctx` has no rng seed and no `max_threads`**, though spec §10 mentions both | `ItemSelectionStrategy.RANDOM` is GUI-only | plan-6 ruling **5**, quirk **#143** | There is no randomness anywhere in the ported router, and `rand` is forbidden |
| 15 | **`PARITY_VERSION` is one constant with three readers**, never `CARGO_PKG_VERSION` | `Constants.FREEROUTING_VERSION` | ruling **AT**/5 | The three are the DRC report's `freeroutingVersion` (`commands/drc.rs:191`), the MCP `check_drc` tool's (`mcp/tools/check_drc.rs:82`) and the manifest's `app_version` (`manifest.rs:332`). Read from the jar at port time (`2.3.1-SNAPSHOT`, `Build-Revision` recorded beside it), never guessed. **The SES/DSN correction**: `fr_dsn::kicad::reader.rs:466`'s host literals are *not* `PARITY_VERSION` (plan ruling 5) — they are the strings the jar writes into a board's metadata, and the port must write the same ones |
| 16 | **Two hand-written cryptographic primitives**, because the dependency rule forbids a crate for either | `RoutingResultManifest.sha256Hex`; Gson's Base64 | scan ruling **R12** | **SHA-256** (`fr-core`, Task 4) and **Base64** (`crates/freerouting/src/mcp/tools/mod.rs`, Task 12). Both are transcriptions of the published algorithm with the standard vectors as tests — Base64 against RFC 4648 §10's, SHA-256 against the FIPS 180-4 examples plus the manifest's own jar-measured digests. They are *deliberate divergences from good practice*, listed here so a reader knows they exist and knows they are not a general-purpose crypto surface |
| 17 | **The port reads UTF-8 where Java reads the platform default charset** for a `.json` session file | `Freerouting.java:304`; `RoutingJobScheduler.java:202` | quirk **#290** (label U) | Measured on a non-ASCII fixture |
| 18 | **The DRC report's `date` is UTC**; Java writes the JVM's default zone | `KiCadDrcReport.java:28-29` | quirk **#276** | A **port** divergence in an output field, invisible to every gate because both normalisers drop `date`. Recorded rather than hidden |
| 19 | **The shipped `drc` default is KiCad's snake_case spelling**, not the jar's camelCase | `KiCadDrcReport`'s `@SerializedName`s | ruling **W**, quirk **#154** | The jar advertises `$schema: https://schemas.kicad.org/drc.v1.json` and then writes keys that schema does not have. `--schema freerouting` is the way back to the jar's bytes, and it is what `p8t3` runs the port with |
| 20 | **The MCP protocol revision is `2025-06-18`**, not the jar's `2024-11-05` | `McpControllerV1.java:285` | ruling **AT** | The port answers `notifications/progress`, `notifications/cancelled` and `ping`, none of which the jar does; advertising the older revision would be a false claim. Delta row 1 |
| 21 | **The MCP job id comes from `/dev/urandom`**, and there is no `Random` anywhere else | `UUID.randomUUID()` | ruling **BC** | Task 12; the only entropy source in the port, and it never touches routing |
| 22 | **The two command lines have two different generic settings overrides, and neither takes the other's.** Native: `--set <section>.<field>=<value>`. Legacy: Java's own `--<section>.<field>=<value>` | `settings/sources/CliSettings.java:43-57` | ruling **BJ** | `clap` owns the native command line and has **no arm** for a free-form `--<section>.<field>=<value>` — it answers `error: unexpected argument` and exit **2**. And ruling AR makes the legacy path bug-for-bug, where the jar ignores `--set` twice over (no `=` on the flag at `CliSettings.java:45`; the payload does not start with `-` at `:58`). So one spelling cannot serve both forms without either breaking `clap` or diverging from the jar. Beyond the spelling they are the same code path — `CliSettings::new_with_set_alias` splits at the first `=`, filters on `router.`, and calls the same `apply_router_setting` at priority 60. The five-row truth table is `cli_e2e.rs::the_generic_override_is_set_on_native_and_dotted_on_legacy`; the jar's half of it is `p8t5`'s `set-on-legacy` row |

---

## 4. How to run every parity check in this repository

**Prerequisites.** A sibling clone of the Java project at `../freerouting` (or `FREEROUTING_JAVA_DIR`),
its HEAD jar at `../freerouting/build/libs/freerouting-current-executable.jar` (or `FREEROUTING_JAR`),
the pinned release jar at `tools/freerouting-2.3.0.jar`, and **JDK 25** at
`/opt/homebrew/opt/openjdk@25` (or `JAVA25_HOME`). Every Java half runs headless with

```
-Djava.awt.headless=true -Duser.language=en -Duser.country=US
-XX:+UnlockExperimentalVMOptions -XX:hashCode=2
```

`-XX:hashCode=2` is mandatory on `p8t3` (quirk #144's `HashMap`-ordered `unconnectedItems`) and
harmless elsewhere; the locale flags are mandatory on `p8t3` (quirk #145's `%.4f`) and on anything
that formats a number.

**Without a JDK, everything still runs.** `parity::require_java_dir` makes each jar-dependent test
skip cleanly, and the committed references under `tests/reference/` keep the same assertions alive.
That is the 56 skips in the summary above.

```sh
# Always available
cargo nextest run --workspace                              # 2389 passed, 56 skipped
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
cargo test --workspace --doc
cargo test -p fr-router --test batch_parity                # Plan 7's byte gate

# The slow lanes (release build)
FR_SLOW_PARITY=1 cargo test --release                      # the four slow p8t1 stems, the batch corpus
FR_SLOW_PARITY=1 cargo test -p freerouting --test mcp_stdio -- --ignored

# The whole-repo audit — 32 invocations, all exit 0, all MISSING=0 UNMAPPED=0
#   copy-pasteable list: crates/fr-router/README.md, "the whole-repo audit"
```

**The differential drivers** (`scripts/differential/README.md` has the harness shape). Each is a
`run.sh` case with a `java/P8T<N>.java` half and a `rust/src/bin/p8t<n>.rs` twin, diffed line for
line — except that most of Plan 8's do not reflect into jar internals at all: they run the **two
whole programs** (plan ruling 13).

| command | what it pins | recorded result |
|---|---|---|
| `run.sh p8t0` | `parseTimespanString` × 30 strings, the 24 h cap, `GRACE_PERIOD`, the round trip | MATCH (33 lines) |
| `run.sh p8t1probe` | `getFileFormat`, `changeFileExtension`, `setFilename`, `calculateCrc32` — five tables | MATCH (162 lines) |
| `run.sh p8t1 [all]` | **the headline gate** — SES bytes, exit code, `normalize_log` | `all`: **18 rows — 17 MATCH, 1 XDIFF, 0 DIFF**. `ci`: **10 rows — 9 MATCH, 1 XDIFF, 0 DIFF**. The **invariant**, which outlives the count: every board stem MATCHes, and the driver has exactly one XDIFF — `invalid-input-java-hangs`, quirk #244 |
| `run.sh p8t2probe` | `BoardStatistics(byte[], FileFormat)` field for field plus `toString()`'s exact JSON | MATCH (289 lines) |
| `run.sh p8t2 [e2e [all]]` | the result manifest, field for field after `normalize_manifest` | shape MATCH (762 lines); `e2e all` **13/13 MATCH, 0 DIFF** — one row per stem of `cli-fixtures.txt`, so the count follows that file and the invariant is that **all of them MATCH** |
| `run.sh p8t3 [e2e]` | the DRC settings merge (Java vs Rust); then the document, score, exit code and log | merge MATCH (25 lines); `e2e` **14 rows: 13 MATCH, 1 XDIFF** |
| `run.sh p8t5` | the legacy surface — slots, `LegacyBridge`, warnings, exit code | MATCH (**2 124** lines; 2 096 before ruling BJ's row landed in `matrix/p8t5-argv.tsv`) |
| `run.sh p8t6` | the eleven-row MCP delta table, asserted to be exactly itself | MATCH — 18 observations, 13 must differ, 5 must agree |
| `run.sh p8t7` | spec §1's KiCad DSN → route → SES → re-read, plus `-de board.json` and quirk #289 | MATCH |
| `sweep-p8t5.sh` | the whole legacy argv matrix, `matrix/p8t5-argv.tsv` | **87 rows: 87 MATCH, 0 XDIFF, 0 DIFF, 0 SKIP** (86 until ruling BJ added `set-on-legacy`) |
| `sweep-p3t15.sh` | Plan 3's DSN corpus — the sweep prints `fixtures: 106  pairs: 530` | **530 pairs, 5 XDIFF, 0 unexpected diffs** (= 525 MATCH + 5) |
| `sweep-p5t1.sh`, `sweep-p5t2.sh` | Plan 5's DRC and report corpora | green |
| `sweep-p7t9.sh` | Plan 7's pipeline corpus | byte-unchanged since Plan 7 |
| `run.sh p4t1`, `p6t1`, `p7t*` | the earlier plans' drivers, all still green on this tree | `p6t1`: **five boards, six rows** (`router-dac2020-bm01-pass2` included) |

**`p8t4` does not exist**, and that is a decision rather than an omission. The plan named it as
"the resolved `RouterSettings` dumped as JSON from both sides — `p4t1`'s 64-case matrix re-run
through the binary". Task 6 reached that rung with the two artefacts that already existed:
Plan 4's **`p4t1`** still runs the 64-case matrix against the JVM (MATCH, 5 728 lines), and the
*through-the-binary* half is the manifest's `settings_snapshot`, which `p8t2 e2e` compares field
for field on **every** stem of `tests/reference/cli-fixtures.txt` — thirteen today — and
`cli_e2e.rs::a_settings_file_reaches_the_run` reads on five more runs across both command lines. A separate driver would have re-derived `p4t1`'s matrix to compare
the same numbers a second time.

**Regenerating the committed references.** None of these is run by the test suite.

| script | regenerates | against |
|---|---|---|
| `scripts/gen-cli-reference.sh` | `tests/reference/cli-<stem>/{argv.txt,route.ses,route.exit,route.log,manifest.json,drc.json,meta.txt}` for every row of `tests/reference/cli-fixtures.txt`, driving the **bare** HEAD jar | the clone's HEAD jar |
| `scripts/gen-drc-reference.sh` | `tests/reference/drc-<stem>/*` — the eight `-drc` documents | the clone's HEAD jar |
| `scripts/gen-batch-reference.sh` | Plan 7's batch stems (`batch.ses`, the pass transcripts) | the clone's HEAD jar |
| `scripts/gen-router-reference.sh` | Plan 6's per-connection router references | the clone's HEAD jar |
| `scripts/gen-reference.sh` | Plans 1-3's geometry and DSN references | **`tools/freerouting-2.3.0.jar`** — Plan 3 ruling 1 pins the port's SES writer to 2.3.0's spelling, because HEAD writes Specctra its own lexer cannot read back (quirk #92) |

`gen-cli-reference.sh --meta-only` rewrites `meta.txt` without touching the jar;
`gen-cli-reference.sh --verify-hash-modes` regenerates every stem under `-XX:hashCode=0..4` and
requires **five byte-identical SES files and five identical DRC reports**. It has **no
`--verify-driver` mode and needs none**: Plan 7's generator needed one because its reference came
from a probe that reflected a constant to `0`; this one runs the bare jar.

---

## 5. The obligation register, CLOSED

Measured on the committed tree, so that the arithmetic below is re-derivable years from now:

| grep | answer |
|---|---|
| `grep -rn "obligation:" crates/*/src` | **62** lines — **2** struck (`~~obligation:~~`), 60 not |
| the same, minus the struck two | **60**, of which **18** say "discharged" or "closed" in their own text and are prose records of work already done |
| `grep -rn "obligation:" crates/*/tests` | **6** — every one a cross-reference from a test to the marker it discharges |
| `grep -rn "obligation:" crates/` | **90** — the three above plus the crate READMEs' narrative |

The 60 are grouped below, and **every group is closed with a reason** — there is no later plan to
inherit an open one.

### 5.1 The six this plan was chartered to close — all six discharged

| marker | what it owed | discharged by |
|---|---|---|
| `fr-settings/src/resolve.rs` (`:193` at plan time, `:204` today) | `RoutingJobScheduler.scheduleJob` — the API path composes the merge differently and must not call `resolve_headless` | **TWO independent closers, and the marker names both.** Task 7's `commands/drc.rs::quality_score_settings` (quirk #272's DRC quality score, pinned by `p8t3 merge`) and Task 12's `mcp/tools/route_board.rs` (ruling AU's MCP path, whose session is byte-identical to `p8t1`'s reference). Neither is a variant of the other |
| `fr-drc/src/report/json.rs:57` | `DrcJsonFlavor`'s CLI default (plan-5 ruling 2) | **Task 7**, controller ruling **W**: the CLI passes `DrcSchema::Kicad` explicitly at the call site, so a future change to the enum's `Default` cannot move it silently |
| `fr-dsn/src/parser/wiring.rs:596` | `read_via_scope` still called the unchecked `insert_via` — the last line of the ladder-hang obligation | **Task 3**: `insert_via_checked` with a per-via `TimeLimit`; `sweep-p3t15.sh` unchanged at 525 MATCH + 5 XDIFF |
| `fr-board/src/board/clearance_override.rs:60` | quirk #232's "second override run" boundary | **Task 3**, and *the premise was false*: quirk **#253** shows `HeadlessBoardManager.createBoard` is unreachable from the DSN parser, so there is no second run. Reproduced anyway — the transcript's `after_second_hole_override` stage shows a second `applyHoleClearanceOverride` moving nothing |
| `mcp/server.rs:9-32` | the Plan 1 skeleton's `ToolHandler` saw only its arguments and could emit nothing | **Task 11**: `Box<dyn Fn(&State, Value, &ProgressWriter, &CancelToken) -> Result<Value, RpcError> + Send + Sync>`, with the four things the obligation asked for named in a table at the site |
| `mcp/stdio.rs:18-28` | MCP concurrency — progress sink, cancel token, reader thread | **Task 11**, and the marker is gone |

`grep -rn "obligation:" crates/freerouting/src` now answers **three** lines — `cli.rs:49`,
`commands/drc.rs:469` and `mcp/tools/route_board.rs:17` — and every one is a *cross-reference* to a
marker elsewhere that is discharged, not a marker of its own.
`crates/fr-settings/src/resolve.rs`'s is struck through with its two closers named.

### 5.2 `fr-router`'s coverage obligations — CLOSED as coverage debt, not as unported code

**`grep -rn "obligation:" crates/fr-router/src` answers 36 lines.** That is not 36 obligations:
`crates/fr-router/README.md` §"The 27 `obligation:` markers" is the crate's own index and counts
**27**, because it excludes the prose lines that merely *reference* a marker and the ones whose own
text records a discharge. **27 is the number to read**; 36 is what the grep prints, and the two are
stated together here so a reader who runs the grep is not left wondering which is wrong. Every one names a Java arm that **is transcribed**;
what is missing is a *fixture that reaches it*, so the transcription is unpinned rather than
absent. Examples: `MazeRipupResolver.checkRipup`'s `roomWasShoved` and `ALREADY_RIPPED_COSTS` arms,
`FoundConnectionInserter.insertTrace:151-162` and `tryNeckDown:553`, `FoundConnectionLocator`'s
fanout arm and its shrink, `RoutingBoard.insertForcedTracePolyline:777-782`'s `maxRecursionDepth
<= 0` arm, `AutorouteEngine.autorouteConnection`'s `StopConnectionOption`, quirk #184's own
condition.

**Closed with reason:** each needs a *new synthetic fixture plus JVM ground truth*, and the
roadmap in §9 is where they are wanted — §9's A/B corpus needs exactly the board classes these
arms live on (a 90-degree board, a per-layer-width board, a large or circular-outline board, a
multi-net-SMD board, a signal-layer-pour board), so building that corpus is what will pin them.
Chasing them one at a time now would build five fixtures for five arms; building the corpus builds
them for the arms **and** for the quality measurements. Two of the roadmap's own rows (#185, #181)
say the same thing in their "Re-pinned by" column: *needs a new synthetic fixture*.

### 5.3 `fr-board`'s `ShapeTree` caller contracts — CLOSED: they are standing rules, not work

`grep -c "obligation:" crates/fr-board/src/datastructures/shape_tree.rs` answers **7** lines:
**four** are the contracts themselves (`:542`, `:550`, `:725`, `:928`, each opening
"obligation: Task 10 …") and three are prose pointing at them (`:34`, `:534`, `:1211`). The four
name **Plan 2 Task 10**, which landed years of commits ago; they are contracts *on the caller*
(`ShapeSearchTree` must insert through `insert_tiles`, must not overwrite stored entries when
`shapes` is `null`, must store entries as `Vec<Option<LeafId>>`, must re-key through this method).
`ShapeSearchTree` is the only caller in the tree and honours all four — `p2t3r` at 2 898 938 lines
and 0 diff over 2 000 randomised `insert_tiles`/`remove_opt`/re-keying operations is the evidence.
They are documentation of an invariant, and they stay.

### 5.4 `fr-dsn`'s `DsnRouterSettings` (2) — CLOSED: the type split is spec §4

`parser/autoroute_settings.rs:{34,388}` note that `AutorouteSettings.readScope` returns
`settings.RouterSettings` in Java while this crate defines a local `DsnRouterSettings` holding
exactly the fields the DSN and rules scopes read and write. **That is the dependency direction, not
a gap**: `fr-dsn` must not depend on `fr-settings`, and Plan 4 Task 6 landed the bridge
(`DsnFileSettings` at priority 20 reads the local struct into the real `RouterSettings`). Pinned by
`p4t1`'s 64-case matrix.

### 5.5 The remaining singletons

| marker | reason it is closed |
|---|---|
| `fr-board/src/rules/via.rs:342` — `ViaInfos.remove`'s renumbering half | **Decided, fix outstanding.** Plan 6 ruling **H** closed *against* re-pointing: `fr_board::rules::ViaRule` must own its `ViaInfo`s. Plan 7 Task 0 ran the port's twin and measured that the re-pointing **changes the route**, so this is a behaviour change, not a transcription gap — §9's Tier 2 is where it belongs |
| `fr-board/src/board/trace_normalize.rs:1038` — `PolylineTrace.change`'s `additionalUpdateAfterChange` | The Java call is a no-op on the headless path (`BasicBoard.additionalUpdateAfterChange` has an empty body; the GUI overrides it). Closed as unobservable |
| `fr-settings/src/router_settings.rs:471` — `AutorouteControl.ExpansionCostFactor` | **Discharged by Plan 6 Task 1** (ruling 8): `fr-router` re-exports the type and `crates/fr-router/tests/skeleton.rs` pins the two paths to one `TypeId`. The marker is the record of why the declaration lives in `fr-settings` |
| `fr-settings/src/router_settings.rs:938` — `AutorouteControl.java`'s maze consumer | Same shape; Plan 6 landed the consumer |
| `fr-board/src/items/area.rs:108`, `structure/board_outline.rs:75` — Plan 1's "memo cache for convex pieces" | Discharged in Plan 2; both lines say so |
| ~~**`--set` is declared and not wired**~~ (`crates/freerouting/src/cli.rs`) | **CLOSED by WIRING it — controller ruling BJ, in Task 14's review round.** Task 14's first pass found the flag declared, parsed by `clap` and read by nothing, wrote that down as a survivor, and named `--section.field=value` as the replacement. The review measured the replacement and it was **worse than the finding**: `clap` refuses a free-form `--<section>.<field>=<value>` on the native form (`error: unexpected argument`, exit 2), so before this the **native subcommand form had no generic settings override at all**. Ruling BJ therefore wired the alias rather than documenting the gap: `fr_settings::CliSettings::new_with_set_alias` gives `--set <section>.<field>=<value>` the very `apply_router_setting` path the dotted spelling reaches at priority 60, and `crates/freerouting/src/commands::cli_settings` picks that constructor on `legacy::is_legacy_form` so the legacy path stays bug-for-bug. Pinned three ways so the claim can never go unpinned again: `cli_e2e.rs::the_generic_override_is_set_on_native_and_dotted_on_legacy` (a five-row truth table through the binary, reading `settings_snapshot`), two unit tests in `crates/fr-settings/tests/cli_source.rs`, and `p8t5`'s `set-on-legacy` row against the live jar. **Nothing in this CLI is now accepted-but-inert except the seven flags the jar itself leaves dead** (row 7) |

---

## 6. The parked residuals, per task

| task | what is parked, and why it is not a defect |
|---|---|
| **0** | The `CancelToken` poll seam was declared but had no consumer; **Task 11 landed ruling BB's three sites and Task 12 ruling AI's fourth**. Nothing parked |
| **1** | Three totalisations (`getFileFormat`'s shift loop, `changeFileExtension`'s NPE, `getSubnets`' bound) each have a named bound at the site. The POSIX assumption in `java_path` is recorded at its real scope — the whole module — rather than at one function |
| **2** | `host` and `unit` are `String` where Java's are nullable, so the port cannot tell `null` from `""` — quirk **#251**, and **controller ruling BD schedules the fix**: change both to `Option<String>` in `fr_core::stats_from_bytes`/`GsonBoardStatistics` and the `fr-router` mirror named by scan rulings R3/R15. Measured blast radius at ruling time: **1 declaration, 3 writes, 6 reads**; only the `p8t2` transcript moves, by about 4 lines. Group it with the R3/R15 successors |
| **3** | Quirk #232's premise was falsified and every live document carrying it was struck in place, `docs/plan-7-handoff.md` and `docs/plan-8-prep/evidence/job1-summary.md` included |
| **4** | `generated_at` has no producer below `crates/freerouting`: there is no date library and the dependency rule forbids adding one. The CLI supplies the instant as a closure; the manifest normaliser drops the field on both sides |
| **5** | `--settings` was accepted and unread; **Task 6 wired it**, and controller ruling **BG** then removed the working-directory default because the jar has no such behaviour to stand in for |
| **6** | The CLI runs `RouterBudget::default()` (Java's own literals) rather than `disabled()`, so a live wall clock is a machine-speed dependency. Bounded by `gen-cli-reference.sh`'s `batch.ses` cross-check |
| **7** | `drc-natural-tone-preamp` is a **permanent XDIFF** on the document rung: the jar does not match itself across `-XX:hashCode` modes (quirk #146). Its `quality_score` matches. The grant is *checked, not waived* — `p8t3` deletes exactly three pinned uuids from the jar's document and requires the remainder byte-identical |
| **8** | `atan2`'s 2-ulp residual sits at the comparator, named, with the consequence spelled out (`PolygonShape` could drop different collinear corners). Not reached by any corpus board |
| **9** | The treeification guard is a `debug_assert!` and therefore silent in the release build the CLI ships. Said at the site, with who should revisit |
| **10** | Quirk #289 (label T) was **measured**, and the brief's hypothesis refuted; quirk #270's own sentence was corrected around it. `docs/superpowers/plans/2026-09-01-plan-8-core-cli-mcp.md:132,861` still carry the refuted prediction — a plan document is not edited after the fact, and the register's strike-in-place rule is satisfied by #270 and #289 |
| **11** | Residual cancellation latency was "one pass"; **Task 12 measured what one pass costs** (135 s on `Issue508-DAC2020_bm01.dsn`, release, `--max-passes 1`) and took ruling AI's fourth poll site. What remains is sub-pass latency, which no Java site polls either |
| **12** | `route_board`'s output format is pinned to `SES`, so quirk #289 is unreachable through the MCP tool. Deliberate: the tool has no reason to write a format the jar fills with a pre-routing board |
| **13** | **Controller question 1, answered by ruling BI**: the port keeps reproducing quirk #105 (parity-correct), and the jar hang it exposes becomes a Tier-1 roadmap row and an upstream-PR candidate. **Controller question 2** is speculative about code that does not exist and is not carried forward |

---

## 7. Known limitations — what this program will never do

Spec §2's out-of-scope list, restated as capabilities rather than exclusions. None of these is a
defect and none is deferred work; each is a decision with its evidence.

* **No GUI.** 35 691 lines of `gui/**`, plus `debug/DebugControl`. The port is a command-line
  program and an MCP server.
* **No REST API and no job queue.** 8 425 lines of `api/**`, `SessionManager`, `Session` and the
  scheduler's queue half. One command line runs one job.
* **No analytics and no version check.** 2 347 lines, and with them the unconditional 1 s startup
  sleep (quirk #264) that makes the jar measurably slower on a trivial board.
* **No Eagle export.** `SessionToEagle` (627 lines) is out of scope by the spec, not dead in Java.
* **No multi-threaded routing.** Java's is unreachable dead code (quirk #143) and `-mt` is inert
  in both programs. Real parallel passes are greenfield work, and §9 puts them **last**.
* **Quirk #113 — `(string_quote .)` diverges.** `Structure.setClearanceRule` splits at the
  string-quote character with `String.split`, i.e. a **regex**, while `Parser.readQuoteChar`
  accepts any string token. Closing it needs a regex engine (forbidden by the dependency rule) or
  a hand-rolled single-character regex emulation. Out of proportion to an input no exporter
  writes, and **closed with reason** rather than attempted.
* **Quirk #162 — `calculateNewIncompleteRooms` does not terminate**, and the port does not guard
  it. Java has no guard, and adding one breaks parity: it would change which doors a room gets on
  the 0.4 % of room completions that reach the condition. `scripts/gen-cli-reference.sh` and its
  siblings carry a `timeout(1)` bound for exactly this. **Closed with reason**; §9's Tier 1 is
  where the fix belongs, behind the `Compat` switch.

**Zero known-uncovered ported paths.** The four zero-coverage Plan 3 paths that stood on this list
through Plans 3-7 are **closed** — Plan 8 Task 13 gave each a directed synthetic fixture under
`crates/fr-dsn/tests/data/`, JVM ground truth from `scripts/differential/java/probes/P8T13Probe.java`
against the pinned HEAD jar on JDK 25, and a named test; all four matched the jar's bytes first
time and **no port behaviour changed**:

| path | Java | fixture | test |
|---|---|---|---|
| `SesWriter.writeWasIs`'s swap body | `SesWriter.java:188-215` | `p8t13-was-is.dsn` | `parity_ses.rs::ses_writer_writes_a_pins_line_for_every_swapped_pin` |
| `Component.readLockType`'s `(lock_type position)` arm | `Component.java:352-364` | `p8t13-lock-type.dsn` | `placement_scope.rs::the_lock_type_position_arm_survives_a_whole_file_read` |
| `SesWriter.writeConductionArea` (quirk #110) | `SesWriter.java:536-553` | `p8t13-conduction-area.dsn` | `parity_ses.rs::ses_writer_mixes_integer_boundary_and_double_hole_coordinates` |
| quirk #105 — `Wiring.readViaScope`'s net-number loop | `Wiring.java:684-687` | `p8t13-via-net-numbers.dsn` + its control | `dsn_reader.rs::read_via_scope_pads_a_multi_subnet_vias_net_numbers_with_zeros` |

`crates/fr-dsn/tests/plan_3_zero_coverage.rs` is the register that keeps them closed: it asserts,
per row, that the fixture exists, the transcript exists and carries a `[jar-cli]` run and a
`[read] Success`, and that the named test exists.

**The fourth row came with a finding, and it is ruling BI's.** `p8t13-via-net-numbers.dsn` is the
first input ever shown to make the **HEAD jar's CLI hang**: the padded zero net number reaches
`DesignRulesChecker.calculateAllIncompletes:558`, whose `rules.nets.get(0)` is `Vector.get(-1)`,
so every autoroute pass throws `ArrayIndexOutOfBoundsException` and `AutorouteBatchLoop.run`
retries for ever. The control fixture is committed beside it — the same file with
`(net NORDERED 1)` on the via — and its transcript records `[jar-cli] exit=0` with a 1 995-byte
routed SES. **One changed token separates a terminating run from a permanent one.** Ruling BI: the
port keeps reproducing quirk #105, because that is the parity-correct answer; the *jar's* fix is a
Tier-1 roadmap row (§9.2.1) and an upstream-PR candidate (§9.10). Full detail:
`.superpowers/sdd/2026-09-01-plan-8-core-cli-mcp/task-13-report.md` §4.

---

## 8. The obligation ticks written into the seven earlier hand-offs

Each of these has its status line written **into that hand-off**, not only here.

| hand-off | what it owed Plan 8 | status |
|---|---|---|
| `docs/plan-1-handoff.md` | MCP handler shape (progress sink, cancel token, reader thread); `id: null`; bare `-drc`; the legacy shim's `startsWith` matching | **All four discharged.** Tasks 11/12 (the handler and the transport), quirk #262's `-32700` reply carrying `"id":null` (delta row 9), quirk #263 (a bare `-drc` is not DRC mode), quirk #259 (every short flag but `-l` matched by prefix, reproduced bug-for-bug) |
| `docs/plan-2-handoff.md` | MCP concurrency and legacy-CLI value normalisation | **Both discharged** — Tasks 11/12 and Task 5 |
| `docs/plan-3-handoff.md` | wire the seven entry points; ruling **A** (whatever holds a `Board` must hold its `CoordinateTransform`); `io/kicad/**` and `SessionToEagle`; do not "fix" quirk #83; MCP concurrency; the four zero-coverage paths | **Six of seven discharged, one closed as out of scope.** `fr_core::LoadedBoard` is ruling A's holder; `io/kicad/**` landed in Tasks 8-10; `SessionToEagle` is `// not ported:` (spec §2, not a reachability claim); quirk #83's J-then-I indexing is untouched; the four paths are Task 13's |
| `docs/plan-4-handoff.md` | the `RoutingJobScheduler.scheduleJob` API-path composition; the legacy-CLI **wiring** half; three product decisions (the five dead flags, `-mt`, `DrcJsonFlavor`) | **All discharged.** Two independent closers for the composition (§5.1); Task 5 rewired `legacy.rs`; ruling **AQ** closed the first two product decisions and ruling **W** the third |
| `docs/plan-5-handoff.md` | the whole `-drc` command line; `DrcJsonFlavor`'s CLI default; `quality_score`'s computation; `coordinateUnit`'s unit flag; `io/kicad`'s board/session classes; the eight `// added in Plan 8:` score markers | **All eight discharged.** Task 7 landed `initializeDrc`, ruling W the default, and the eight committed references became eight free assertions the moment the injection was removed. The unit flag is a **recorded decision not to expose one** (quirk #151). The score markers are §5's ruling-AS rows, re-pointed to `not ported:` by Task 14 |
| `docs/plan-6-handoff.md` §11 | six items: the pipeline caller, `ProgressSink`'s registration points, ruling 4's score surface, the DSN reader's behaviour change, the CLI's `// Java bug:` convention, MCP concurrency | **All six discharged**, one of them by being **moved into Plan 7** (the pipeline caller, Task 15) before Plan 8 began |
| `docs/plan-7-handoff.md` §7 | the 30 `added in Plan 8:` markers; the `obligation:` markers naming Plan 8; ruling **AW**'s `prepare_board` call; the `--max-items` help text | **All discharged.** The marker grep is empty on `crates/*/src` and `crates/*/tests`; `fr_core::apply_router_settings_for_loaded_board` calls `fr_router::pipeline::prepare_board`; the `--max-items` sentence is on `--router.max_items`'s help text and in `crates/freerouting/README.md` |

---

## 9. The post-parity roadmap

**Imported verbatim from `docs/plan-8-prep/post-parity-roadmap.md`** (721 lines, scan ruling R14),
which is where it was written and reviewed. The only edits made on import are the ones this
document's structure requires or that ruling BI ordered, and each is marked where it appears:

1. every heading is demoted **two** levels (`#` → `###`, `##` → `####`) so the imported document
   nests under this section — its own section **numbers** are unchanged, so a citation of "§3.1"
   means the roadmap's §3.1, not this hand-off's;
2. **controller ruling BI** adds one Tier-1 row (quirk **#105**, the jar-hang Task 13 discovered)
   to the imported §2.1, labelled as an addition;
3. two paragraphs the draft wrote in the future tense — §6.5 and the register-hygiene note — carry
   a marked correction block underneath, because Plan 8 has since happened. The original text is
   kept, per the register's strike-in-place rule.

**The ordering is the user's, and it is binding**: *better routing beats speed*. Tier 1 is
crashes, hangs and silent data loss; Tier 2 is routing quality and is the heart of it; Tier 3 is
DRC and report accuracy for KiCad users; Tier 4 is settings and CLI predictability. **Performance
work — real parallel passes included — is explicitly last**, and the roadmap's §6.1 records it as
a non-goal for the first fixed release. Java's own multithreading is dead code (quirk #143), so
that work is greenfield and has no parity anchor of any kind.

**The `Compat::{Java, Fixed}` switch is the design the roadmap recommends and this hand-off
endorses.** Parity is the port's only acceptance evidence, and every fix below breaks it. The
switch keeps both answers alive in one binary, so that `Compat::Java` **preserves the whole
differential harness as the regression net** — every driver, every sweep, every committed
reference stays meaningful — while `Compat::Fixed` is where the fixes land and where the A/B
corpus measures them. Deleting parity to make room for correctness would throw away the only
instrument that can tell a fix from a regression.

---

### Post-parity roadmap — the imported document begins here

**Status:** draft for import into Plan 8's final hand-off, as its "what comes after parity" section.
**Binding direction (user, 2026-08-31):** *better routing beats speed.* Everything below is ordered by
routing-quality impact. Perf work — real parallelism included — is explicitly last, and is recorded in
§6 as a non-goal for the first fixed release.

Source of record for every row id cited here is `docs/java-quirks.md` (211 pinned rows, ~95
totalizations, the candidate table and the process notes). This document does not restate those rows;
it tiers them, corrects the ones whose "Suggested fix (post-parity)" column is stale, and says what
each fix must be measured against.

---

#### 1. Method

##### 1.1 The `Compat` switch

Parity is the port's only acceptance evidence. Every fix below breaks it. The switch keeps both
answers alive in one binary:

```rust
// crates/fr-settings/src/compat.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Compat {
    /// Reproduce freerouting (Java, clone HEAD) exactly, bugs included.
    /// This is the parity default and the mode the differential harness runs in.
    #[default]
    Java,
    /// Apply every landed post-parity fix.
    Fixed,
}
```

**Where it lives.** `fr-settings`, because it is already every crate's shared leaf dependency for
configuration and because the router, the DSN reader, the DRC and the CLI all need it.
`fr-geometry` and `fr-board` must **not** depend on `fr-settings`; the two geometry-level fixes that
need gating (#5, #9/#82) take a `Compat` **parameter** at the entry point that has one
(`ShapeTraceEntries`, `NetIncompletes`), not a global.

**Precedent, deliberately followed.** There are no Cargo features anywhere in this workspace and
none should be added: a `#[cfg(feature)]` fix cannot be A/B-tested inside one process, and the whole
point of §7's measurement is a same-binary A/B. The model is `fr_drc::report::json::DrcJsonFlavor`
(`crates/fr-drc/src/report/json.rs:47-66`) — a plain `Copy` enum, `Default` = the parity variant,
dispatch by `match` so a third variant is a compile error, and a `// Java bug:` marker on the
bug-compatible arm naming its register row. `WriteScopeParameter::compat_mode`
(`crates/fr-dsn/src/parser/scope_parameter.rs:214`) is the second precedent: a plain runtime bool
threaded through an options struct.

**How a fix is gated.** Three shapes, in order of preference:

1. **Branch at the site.** The Java body stays, verbatim, with its `// Java bug:` marker and row id;
   the fixed body sits beside it under `match compat { Java => …, Fixed => … }`. Preferred for
   everything small (#187's two swapped indices, #65/#69/#72's parentheses, #176's `lineB.y`).
2. **Two functions, one dispatcher.** Where the fix changes control flow enough that a branch would
   obscure both readings — #160's comparator, #82's in-circle predicate, #163's corner walk.
3. **An options field.** Where the fix is a *policy* rather than a correction and a user may want to
   choose: #172's pure-SMD relaxation, #182's acid-trap avoidance, #202's `--max-items` stop scope.
   These get their own setting, and `Compat::Fixed` only changes the *default*.

**How `Java` mode keeps the harness green.** Nothing about the existing evidence moves:

* `scripts/differential/run.sh` (27 drivers, `p2t3` … `p7t10`) runs the Java side against the jar and
  the Rust side in `Compat::Java`. Zero diffs stays the bar; a fix that moves a `Java`-mode driver is
  a bug in the gating, not in the fix.
* `crates/fr-router/tests/reference_parity.rs` keeps the six `tests/reference/router-*` rows and both
  golden documents (`router.jsonl`, `router-steps18.jsonl`) unchanged, and keeps plan-6 ruling 1's
  three-rung ladder — (a) attempt state + ripped set, (b) inserted geometry byte for byte, (c) spec §9
  metrics — asserted in `Java` mode on every stem.
* Every `// Java bug:` marker stays. `grep -rn "// Java bug:"` must still return one hit per pinned
  row that the port reproduces; the fix adds a `// fixed in Compat::Fixed:` line beside it, never
  replaces it.

**A `Compat::Java` binary must be byte-identical to the pre-`Compat` one.** This is the direct
analogue of Plan 7 ruling 11 ("observers must not perturb": swapping the `ProgressSink` for a
recording one must change no board byte, asserted by `structural_hash` equality across two runs).
The same test shape applies here — introducing the enum, before any fix lands, must leave every
`structural_hash`, every `router.jsonl` and every `drc.json` unmoved. Land the enum as its own commit
with that assertion, so a later diff can never be blamed on the plumbing.

**How `Fixed` mode gets its own fixtures.** A second golden set, generated by the port and reviewed,
not by the jar — the jar cannot produce it. Concretely:

* `tests/reference/router-fixtures.txt` grows a `mode` column (`java` | `fixed`); every existing row
  keeps `java`. `scripts/gen-router-reference.sh` learns `--mode=fixed`, and for that mode writes
  `router-fixed.jsonl` + `router-fixed.meta.txt` from the **Rust** binary, recording the port's git
  sha and the exact `Compat::Fixed` fix set in the meta (a fix landing after the golden was cut must
  invalidate it loudly).
* `Fixed`-mode goldens are **regression** evidence, not correctness evidence. Correctness evidence is
  §7's A/B numbers plus, per fix, at least one **directed** test that fails against `Compat::Java` —
  the pattern `crates/fr-router/tests/tightener.rs` already uses for #183/#184 and
  `crates/fr-router/tests/sorted_neighbours.rs` for #160.
* The DRC set is the cheap half: `tests/reference/drc-*` (8 stems) is a pure function of the board,
  so a `Fixed` DRC golden is one `scripts/gen-drc-reference.sh --mode=fixed` away and diffs
  legibly.
* **Pin every `Java`-vs-`Fixed` difference at both ends, with the XDIFF mechanism Plan 7 Task 8
  established.** A committed `tests/reference/<stem>/<file>.xdiff.txt` records a known divergence
  precisely enough that it *cannot widen or close in silence* — Task 8's pinned connection 175 with
  both `maxIdAfter` values, later **inverted into a MATCH pin** when ruling AY landed the fix. A
  `Fixed`-mode fix should produce exactly that artefact: an xdiff naming the first differing
  connection and the reason, converted to a MATCH pin when the golden is re-cut.

##### 1.2 The evidence bar, per fix

A fix does not land without all five:

1. **The register row id** it discharges, and — where the register's "Suggested fix" column is stale
   or wrong — an explicit correction of that column, written back into `docs/java-quirks.md` in the
   same commit. (§2-§5 flag every column this document corrects.)
2. **The observable effect**, stated as a sentence a user would recognise: not "the comparator becomes
   transitive" but "the room keeps the door it has, so the maze can route through it".
3. **A driver or test that pins the NEW behaviour** and fails in `Compat::Java`. Named in the row.
   Where a differential driver already covers the site, the fix reuses it with a mode axis
   (`p6t3` for the neighbour/room family, `p7t4` for the via optimizer, `p7t7` for the statistics,
   `p2t11` for the board mutators) — that is far cheaper than a new driver and it keeps the Java side
   as the control.
4. **A `Java`-mode green run** of the full harness and `reference_parity`.
5. **An A/B number** from §7. "It routes better" is not a claim this project accepts without the
   completion-rate / length / via-count / score table.

Three constraints inherited from the plan rulings, which apply unchanged to every fix here:

* **Ruling AI — time must be out of every measurement.** The 1000 ms `optChangedArea` budget is a
  parameter and every `p7t*` driver runs with it **disabled on both sides**; a separate, non-parity
  test proves the budget trips. A `Fixed`-mode A/B obtained with the budget live would be a
  coincidence, not evidence. The same applies to #208's `TimeLimit`: measure the necked-retry fix
  with the connection time limit disabled, then separately prove the retry is time-boxed.
* **Plan 7 ruling 7, as amended by scan ruling 9 — do not add recovery Java lacks.** This bites
  directly on §2.3: a bare `catch_unwind` around a crashing site would silently route a board Java
  abandons, "a divergence with no failure signal". So every Tier 1 crash row must be fixed as a
  **guard at the defect** — the null check, the bounds check, the corrected index — never as a
  handler wrapped around it. Where the register's column says "guard the field", that is the whole
  instruction and it is not negotiable.
* **Task 8 SF1's precedent — re-measure before rewriting a register row.** #184's "reachable only
  with a non-null clip shape" was re-measured (`clip_is_none=true` on every hit) and the row was
  rewritten honestly with an `obligation:` marker naming what would close it. Every "stale column"
  correction in §2-§5 below must be landed the same way: a measurement, not an argument.

Two standing rules that fall out of the register itself:

* **Never fix half of a paired row.** #48 and #57 are the same defect in the component and in its
  keepouts; fixing one alone makes them *disagree* where they currently agree. Same for #164 (the
  overload **and** the `touchingSides` length check — fixing only the overload turns a silent skip
  into an `ArrayIndexOutOfBoundsException`), #89/#94, #9/#82, and #126/#128 (#128's arrays start
  `null`, so #126's fix does not reach it — the register says so and it is right).
* **A fix whose only effect is to change an arbitrary order is not a fix.** #44, #63, #74 and #210
  are lawfulness repairs with no correctness argument behind them; they change routing and must be
  justified by §7's numbers or not landed. They are collected in §3.6 as *measure-only*.

---

#### 2. Tier 1 — crashes, hangs, OOM, silent data loss

The bar for this tier: a real board, imported from a real EDA tool, can make the program hang, die,
or silently lose part of the design. Ordered by (reachability × severity).

##### 2.1 Non-termination and OOM

| # | Effect on a real board | Fix | Risk | Re-pinned by |
|---|---|---|---|---|
| **#76** | A two-rail, four-rung ladder on one net makes `normalizeAllTraces` **never return**. `Wiring.readScope` ends every DSN read with that call, so *any imported design containing the pattern hangs the reader* — before routing even starts. Neither normalisation cap reaches it. | Bound the entry walk: clear the collection before the re-read (this is #71's fix) so the walk cannot revisit retired entries. The register's alternative ("give `split` an iteration budget") is the weaker option — it caps a symptom. **Register column is right; do #71 first and re-measure whether #76 still needs anything.** | Medium. Changes which traces `split` returns on any board with overlapping same-net traces, which is common. | `crates/fr-board/tests/trace_normalize.rs::a_four_rung_ladder_never_finishes_normalizing` — today `#[ignore]`d as an unbounded reproduction; in `Fixed` mode it becomes a **terminating** assertion with a literal answer. `p2t11` mode 8 (scenarios S1, S2, S10) is the `Java`-mode control. |
| **#106** | The mechanism under #76, and independently reachable: `Item.getConnectionItems`' walk along the contacts has **no visited set**, so a closed connection with no fork — the exact cycle `removeIfCycle` has just confirmed — is circled for ever. Instrumented, this is where the ladder actually spends its time (~60 entries retired per minute). | Add a visited set and break on revisit. Register column is right. | Medium — it changes which items `removeIfCycle` deletes on a cyclic net, i.e. it turns a hang into a **value nothing downstream has ever seen**. That value needs review, not just a green test. | `crates/fr-board/tests/trace_normalize.rs::a_four_rung_ladder_stops_when_the_stop_check_trips` gains a `Fixed`-mode sibling that terminates with **no** stop check armed, plus a new directed test asserting which items survive `remove_if_cycle` on a 3-, 4- and 5-rung ladder. |
| **#71** | `overlappingTreeEntries` **appends** to the caller's collection and never clears it; `PolylineTrace.split` re-reads into the same list and restarts the iterator. Quadratic on a dense net, unbounded on a ladder. | Clear before the re-read, **or** return a fresh collection. Prefer the second: it makes the aliasing unrepresentable rather than relying on one call site to remember. **This corrects the register's column, which offers them as equals.** | Low mechanically, high in blast radius — the re-walk order is an input to every shove decision downstream. | `p2t11` mode 8 with a `Fixed` axis; the entry-count assertion is the discriminator (Java re-walks, Fixed does not). |
| **#162** | `calculateNewIncompleteRooms` **does not terminate** and dies with `OutOfMemoryError` when a room's shape has more border lines than its `toSimplex()` does — an `IntOctagon` whose diagonals are implied comes back as a 4- or 5-line simplex. **Reachable from production**, measured at 0.4 % of room completions on a real board; `AutorouteEngine.completeExpansionRoom` hands `SortedRoomNeighbours.complete` exactly these rooms. | Compute `roomSimplex` **once, in the constructor**, and derive every `touchingSideNo` from it. The register also offers "bound the loop by `borderLineCount()` turns" — that is a crash guard, not a fix, because the walk would then terminate on the *wrong* side. **Take the first; the register is right to list it first but should not present them as equivalent.** | High. It changes which doors every affected room gets, i.e. maze connectivity, on 0.4 % of completions. This is the single riskiest Tier 1 fix. | `crates/fr-router/tests/sorted_neighbours.rs::the_room_shapes_that_make_calculate_new_incomplete_rooms_loop_for_ever` — today it pins the trigger **without running the loop**; in `Fixed` mode it runs it and asserts the door set. `p6t3` mode 5 currently *skips* those calls on both sides; the fix makes mode 5 a full-coverage run and that is the acceptance criterion. |
| **#86** | The DSN scanner's buffer is a fixed 16 MiB `char[]` that `nextString` indexes with **no refill** and no `zzEndRead` bound. A design over 16 MiB cannot be scanned at all; a token straddling a refill boundary reads whatever the buffer holds; a fresh scanner's second `nextString` throws `ArrayIndexOutOfBoundsException`. | Read the file into an exactly-sized buffer (which is what the port already does — `DsnScanner::new` converts the whole input at construction, `// not ported: zzRefill`). The remaining work is **raising or removing `DsnError::InputTooLarge`**, which today reproduces Java's 16 MiB ceiling on purpose. | Low. The port's lexer is already correct; the fix is deleting a deliberate limit. | `crates/fr-dsn/tests/lexer.rs::input_larger_than_the_java_buffer_is_rejected` gains a `Fixed`-mode sibling that **accepts** a 20 MiB input and lexes it to the same token stream a 15 MiB one gives. |
| **#27** | `PolygonShape.intersects(Shape)` binds to itself for polygon-vs-polygon and recurses to `StackOverflowError`. **Not recoverable**: the pass-level and item-level handlers catch `Exception`, not `Throwable`, so this kills the whole run in both languages. Reachable — Task 17's differential sweep got there from `Circle.intersects(Simplex)` via `PolygonShape.intersects(Circle)` on a self-intersecting polygon. | Add an `intersects(PolygonShape)` overload that splits both sides to convex. The register's alternative ("type-test the argument") is a band-aid that leaves the question unanswered. **Prefer the first.** | Medium. It makes a previously-fatal query answer, and polygon keepouts are common in KiCad exports. | `crates/fr-geometry`'s `polygon_against_polygon_reproduces_the_java_stack_overflow` (today a panic assertion) gains a `Fixed` sibling asserting the convex-split answer against a hand-computed result. |
| **#105** *(added by Plan 8 Task 14, controller ruling BI — the roadmap draft predates it)* | **A padded zero net number hangs the jar's CLI for ever.** `Wiring.readViaScope`'s net-number loop (`Wiring.java:684-687`) omits the `++currentIndex` its twin `readWireScope:441-445` has, so a via whose `getSubnets` answers more than one subnet gets its trailing slots padded with `0`. The reader is fine — that is all quirk #105 used to say. Downstream is not: `DesignRulesChecker.calculateAllIncompletes:558` does `rules.nets.get(0)`, which is `Vector.get(-1)`, so **every autoroute pass throws `ArrayIndexOutOfBoundsException` and `AutorouteBatchLoop.run` retries for ever.** Measured on the HEAD jar: `java -jar <jar> -de crates/fr-dsn/tests/data/p8t13-via-net-numbers.dsn -do out.ses` never terminates. | Add the `++currentIndex`, matching `readWireScope`. **This is a Java-side fix owed upstream; the port must keep reproducing it in `Java` mode** (ruling BI — the port is bug-compatible today and that is the parity-correct answer). In `Fixed` mode, add it and re-run the corpus. | **Low, and uniquely well-controlled.** `crates/fr-dsn/tests/data/p8t13-via-net-numbers-control.dsn` is the same file byte for byte except `(net NORDERED 1)` on the via: its via reads `nets=[1]`, its transcript records `[jar-cli] exit=0` and a 1 995-byte routed SES, and it reaches no throwable at all. **One changed token separates a terminating run from a permanent one**, which is what makes the hang attributable to the padded zero and to nothing else about the file. No corpus fixture reaches the loop, so the corpus is unaffected either way. | `crates/fr-dsn/tests/dsn_reader.rs::read_via_scope_pads_a_multi_subnet_vias_net_numbers_with_zeros` asserts both item graphs, both via rows and **both jar verdicts**; `crates/fr-dsn/tests/plan_3_zero_coverage.rs`'s register asserts the control fixture, its transcript and its `exit=0` exist. Full detail: `.superpowers/sdd/2026-09-01-plan-8-core-cli-mcp/task-13-report.md` §4 |

**Reachable totalization rows in the same family** — Java hangs or dies, the port already returns a
value. These need **no port work**; they belong here so the roadmap records the Java-side fixes owed
upstream and so nobody "restores parity" by re-introducing them:

* `Shape.readPolygonPathScope` / `readPolylinePathScope` loop for ever at EOF, appending `null` until
  the heap dies — reachable from any truncated DSN. Port: `read_tokens_to_close` → `None`.
* `Library.readPadstackScope`'s `while (nextToken != CLOSED_BRACKET)` never terminates at EOF.
* `Structure.readBoundaryScope` and `readLayerScope`'s `use_net` loop spin for ever at EOF.
* `Component.readLockType`'s `for (;;)` spins for ever at EOF.
* `NetIncompletes.calculateNetItems` never terminates when the seed item does not carry the net.
* `RulesWriter.writeRules` NPEs when `library.padstacks` is `null` — **corpus-confirmed**:
  `fixtures/empty_board.dsn` kills the 2.3.0 jar, and the port writes a valid 20-line file.
* `SpecctraDsnStreamReader.zzScanError` throws a bare `java.lang.Error` that no `catch (IOException)`
  stops, so it escapes `readBoard` uncaught.
* `Library.readScope` NPEs out of the whole read if a `library` scope precedes `structure`.
* `Wiring.readScope`/`readWireScope`/`readViaScope` NPE on a null board.
* `Package.readRotation` lets a `NumberFormatException` abort the entire DSN read on one malformed
  `(rotate …)`.
* `RulesReader.applyViaRule` throws `NoSuchElementException` out of a bare `(via_rule)`, killing the
  read.
* `MinAreaTree.removeLeaf` on an already-removed leaf **silently destroys the whole tree** — no
  exception, no log line. The port asserts instead. This is the worst silent-data-loss row in the
  register (#39) and it should be reported upstream regardless of what this port does.

##### 2.2 Silent data loss and silent input corruption

These lose part of the user's design with no error, no warning, and no visible symptom until the
routed board is wrong. They matter more than the crashes above, and the seed ranking omitted them.

| # | Effect | Fix | Risk | Re-pinned by |
|---|---|---|---|---|
| **#95** | `Structure.readScope` reads `autoroute_settings` **only when it is the first layer-structure consumer in the scope**. Any `keepout`, `via_keepout`, `place_keepout` or `plane` earlier in the same `structure` scope makes the whole `autoroute_settings` scope go unread *and unskipped* — so its closing bracket ends the **structure** scope one level early and everything after it is misparsed by the enclosing `pcb` loop. Exporters conventionally write keepouts first, so **this silently discards the router settings of a large share of real files** and corrupts the tail of the structure scope with them. | Hoist the `AutorouteSettings.readScope` call out of the `if`, leaving only the layer-structure creation inside. Register column is right. | Medium. It changes which boards get DSN-supplied router settings — for many corpus files, from "none" to "the ones the file actually specifies". Expect corpus-wide routing movement, in the right direction. | `crates/fr-dsn/tests/structure_scope.rs::an_autoroute_settings_scope_after_a_keepout_is_never_read` inverts in `Fixed` mode; `read_autoroute_settings_scope_reads_costs_and_layer_rules` is the control. Add a `p3t15`-style corpus sweep counting, per fixture, whether the settings were read. |
| **#94** (with **#89**, **#93**) | `Structure.createBoard`'s overflow loop divides an **`int`** `scaleFactor` by 10 until it truncates to **0** — for *any* boundary coordinate at or above **6,710,886 DSN units, whatever the resolution**. `new CoordinateTransform(0,0,0)` is then built without complaint; #89's IEEE division makes every written coordinate `Infinity`/`NaN` and every read coordinate `0`, so the board's whole outline degenerates to the bare `(-1000,-1000)..(1000,1000)` box. **JVM-verified to report `Success`.** #93 (a `(circle …)` outline's bounding box comes out 2× too wide and 2× too tall) *halves the threshold* for any circular-outline design. | #94: make `scaleFactor` a `double`, or clamp it to `>= 1`. #89: reject a zero or non-finite scale factor in the constructor. #93: use `coor[0] / 2` on all four bounds. **All three together** — #93 alone changes the board size, #94 alone changes which boards degenerate, #89 alone only converts a silent `Infinity` into a loud error. The register lists all three correctly but does not say they are one change; **that is the correction.** | High for #93/#94 — they change the board coordinates and the DSN scale factor of every over-large or circular-outline design, so every affected fixture re-baselines. | `crates/fr-dsn/tests/structure_scope.rs::the_overflow_loop_drives_scale_factor_to_zero_by_integer_division` and its control; `crates/fr-dsn/tests/geometry_scopes.rs::coordinate_transform_with_a_zero_scale_factor_gives_infinity` and `circle_bounding_box_is_javas_doubled_box`. All three invert in `Fixed` mode. Add a large-board fixture (≥ 6.71 M units) to the corpus — **there is not one today**, which is why this was never noticed. |
| **#112** | `RulesReader.applyRules` warns "layer not found" and does **not** return, leaving `layerIndex = -1` — which is the sentinel the two branches below read as **"all layers"**. A `(layer Foo (rule (width 500)))` naming a layer the board does not have therefore silently overwrites the **default trace width on the whole board**. Reachable from any `.rules` file written for a different stack-up, i.e. the ordinary way a rules file goes stale. | `return` after the warning. Better, and what the port is already positioned for: make the sentinel unrepresentable — the port's `Option<usize>` already spells `None` as "all layers", so `Fixed` mode turns that into a two-variant enum. **Register column is right and names both; take the second.** | Low. It stops a wrong rule from applying, and the "right" behaviour (drop it) is unambiguous. | `crates/fr-dsn/src/rules_reader.rs` `apply_rules`; add a directed test with a `.rules` file naming `B.Cu` against a 4-layer board and assert the default width is untouched in `Fixed`. |
| **#91** | `skipScope` returns `false` at EOF while skipping an unrecognised nested scope, every caller discards it, and `readScope`'s own loop then returns **`true`** from the next token. Net: a DSN truncated in the middle of an unrecognised scope is reported as a **successful** parse of a partial board — `BoardReadResult.Success`, not a parse error. | This is a **product decision, not a bug fix**, and the register says so correctly. `Fixed` mode should report a truncation, but the right shape is a third `BoardReadResult` variant that carries the partial board *and* the diagnostic, not a hard failure — callers may legitimately want the routing data that survived. **This corrects the register's column, which frames it as a binary.** | Low mechanically; it is an API change on `BoardReadResult`. | `crates/fr-dsn/tests/scopes.rs::read_scope_generic_returns_ok_true_when_truncated_inside_an_unknown_scope`; the `Fixed` sibling asserts the new variant. |
| **#90** | `DsnFile.readIntegerScope`'s failure branch returns `0` **and does not consume a second token**, so the scope's closing bracket is left unread and desyncs `AutorouteSettings.readScope`'s flat, depth-unaware loop — which then misreads that bracket as ending its own scope one field early. The totalized `0` reaches `RouterSettings` and is written back out, so it is observable. | Fix the token consumption **and** the caller-side loop together. The register is explicit and right that fixing one alone makes a malformed `autoroute` scope parse *worse*. | Medium — two files, and it interacts with #95. Land it after #95. | `crates/fr-dsn/tests/…::read_integer_scope_does_not_consume_a_second_token_after_a_bad_first_one`, inverted in `Fixed`, plus a round-trip test over a malformed `(autoroute_settings)`. |
| **#103** | `Network.insertComponent` `return`s — not `continue`s — when one pin names a padstack the library lacks, so the package contributes its first *n-1* pins and **none** of its keepouts, via keepouts, place keepouts or outlines. Item ids shift for everything after it. | `continue` past the pin, or reject the component before inserting anything. **The register offers them as equals; they are not.** Rejecting the component wholesale is the honest answer — a component missing a pad is not a component, and half-inserting one silently loses its keepouts. Emit a diagnostic either way. | Low — `Library.readScope` refuses the whole file for the same condition, so this is hard to reach from a DSN read. Include it for the KiCad-JSON reader Plan 8 wires, which has no such guard. | New directed test in `crates/fr-dsn`; there is none today. |
| **#211** | The trace arm of `reduceNetsOfRouteItems` puts its `if (somethingChanged) break;` **outside** the net loop where the via arm's is inside, so one visit can strip **every** net from a route item — a zero-net state the `netNumbers.length <= 1` guard exists to prevent. | Move the break inside the net loop, matching the via arm. Register column is right. **Note the port currently reproduces Java's placement deliberately** (this row is the record of a port defect found and fixed to *match* Java in Plan 7 Task 8b), so `Fixed` mode moves it back. | Low. No corpus item is on two nets, so this is latent — but the fix is unambiguous. | `crates/fr-board/tests/board.rs::one_visit_reduces_two_nets_because_the_trace_arm_breaks_outside_its_net_loop`, inverted; `p2t11` mode 6 is the JVM control. |
| **#45** | `Item.assignNetNo` on an item already on more than one net warns and then overwrites only `netNumbers[0]`, so the item ends up on the new net **plus** whatever else it was on. Its sibling `removeFromNet`'s search loop has no `break`, so a duplicated net number is removed at its **last** occurrence. | Replace the whole array or refuse the call; add the `break`. The register correctly says to check `reduceNetsOfRouteItems` first, since that is what creates multi-net items — and #211 is in the same method, so land them together. | Low. | `crates/fr-board`'s three `assign_net_no_*` / `remove_from_net_removes_the_last_duplicate` tests, inverted in `Fixed`. |

##### 2.3 Reachable crashes on a real board

Each of these is an unguarded dereference or index on a path a real design reaches. In the port most
are `.expect(…)`/panic reproductions naming the Java line, so `Fixed` mode is a guard, not a rewrite.

| # | Effect | Fix | Risk | Re-pinned by |
|---|---|---|---|---|
| **#67** | `Communication.hostIsOldKicad` runs `Integer.parseInt` on the first digit run of `host_version` with no guard. A digit run longer than an `int` throws `NumberFormatException` **out of a predicate every board load calls**. Directly KiCad-facing. | `Long.parseLong` with a clamp, or catch and answer `false`. Register right. | None. | `crates/fr-board/src/board/communication.rs`; add a directed test with `host_version "99999999999"`. |
| **#123** | `getHorizontalTraceCosts` / `getVerticalTraceCosts` dereference the cost arrays with no null guard where their two siblings answer `1.0`. Any `RouterSettings` with a non-empty `layers` and an unallocated `scoring` throws. | Give both the sibling guard. Register right. | None — it removes an inconsistency between four accessors. | The two `#[should_panic]` tests in `crates/fr-settings/src/router_settings.rs` gain non-panicking `Fixed` siblings asserting `1.0`. |
| **#173** | `AutorouteControl.initNet`'s null-net arm completes only for `netNumber <= 0`; a **positive unknown net** throws two lines later. `RoutingBoard.java:1023` builds a control from a pin's net number, so a stale net number is the reachable path — and it kills the connection. | Move the `netNumber > 0` test above the null lookup, **or** give the null-net arm its own half-width fallback instead of relying on net 1 existing. The second is better: the first still reads net 1's widths for `netNumber <= 0`, which is itself arbitrary. **Register lists both; prefer the second.** | Low. | `crates/fr-router/tests/control.rs::a_positive_net_the_board_does_not_have_throws_like_java`, inverted. |
| **#168** | A cancelled `splitToConvex` makes `DrillPage.getDrills` throw **and** leaves the page memoised as having **no drills** — because the net number and an empty list were installed before the work began. A page interrupted once answers "no drills here" for the rest of the connection, **silently removing every via candidate on it**. | Null-check `drillShapes` and set `this.drills = null` before rethrowing, or install the list only after the split succeeds. The second is strictly better and the register names it. | Low. | `crates/fr-router/tests/drill.rs::split_to_convex_stops_when_the_stop_check_trips`, whose "memoised empty list" and "second call" assertions invert. |
| **#169** | `removeIncompleteExpansionRoom` dereferences a lazily-created list with no null guard where its three sibling readers guard. `ExpansionDrill.calculateExpansionRooms` builds its seed room with the bare constructor, so the list is never created — the NPE is swallowed by #166's `catch` and read as "blocked". **On an engine that has never had an incomplete room added, no drill can ever be built** (JVM-verified: 0 drills vs 13 on the same page, 28 swallowed NPEs). | Guard the field, as the three siblings do — **and** have `ExpansionDrill` call `addIncompleteExpansionRoom` so the room is in the database it is about to be completed out of. Both; the register names both and is right. | Low. Real routing runs do not reach it only because the maze completes rooms first. | `crates/fr-router/tests/drill.rs::a_virgin_engine_yields_no_drills_at_all`, inverted. |
| **#185** | `insertForcedTracePolyline` dereferences a possibly-`null` `newTrace` at `:756` and guards the **same variable** at `:791`. No `catch` covers `:756`, so the nearest handler is `AutorouteConnectionRouter.route`'s bare `FAILED` — **the whole connection is abandoned where the guard would have skipped one segment.** | Move `:791`'s null test up to `:756`. Register right. | Low. It converts a lost connection into a lost segment. | `crates/fr-router/src/board_ext/routing_board_ext.rs`'s `expect`; a directed `Fixed` test needs a resample that brings a polyline's two ends together — latent on all 1 621 rows of `p6t15b-insert-forced.txt`, so this needs a **new synthetic fixture**. |
| **#181** | `FoundConnectionLocatorAnyAngle.calculateNextTraceCorners` builds a `FloatLine` from a possibly-null corner and dereferences it one line later; the author guarded the same value 40 lines further down. `autorouteConnection` catches it and degrades the whole connection to `FAILED`. | Hoist the `resultCorner != null` test to `:287` and skip the correction loop. Register right. | Low. | `crates/fr-router/src/autoroute/path/locator_any_angle.rs`'s panic; not reachable from any current fixture, so this also needs a **new synthetic fixture** (two parallel lines whose intersection is null). |
| **#47**, **#49**, **#42**, **#43**, **#52** | Five unguarded lookups that throw on ordinary designs: `Component.changeSide` on an unplaced component (NPE **after** it has already flipped `onFront`, leaving the component half-mutated — reachable for any design whose placement scope leaves a component unplaced); `Components.get(0)` on Java's own "no component" sentinel; `Packages.get`/`LogicalParts.get` with no bounds check where the sibling `Padstacks.get` has one; `BoardLibrary.removeViaPadstack`/`getMirroredViaPadstack` before any via padstack was ever added; `Pin.getTraceExitRestrictions` dereferencing the component **before** the `component == null` guard that exists for it. | All five: add the guard the sibling already has, return `Option`/`null`. For #47 also flip `onFront` **after** the guard, not before. Register columns are all right. | None. These are the cheapest rows in the whole register. | Each has a `#[should_panic]` test in `crates/fr-board`; all five gain non-panicking `Fixed` siblings. |
| **#22**, **#24**, **#25** | `Polyline`'s `removeOverlaps` reads index −1 once `newLength` hits 0 (reached by ~11 % of random line arrays from a small pool; six lines `h,v,h,v,h,v` is a minimal case) and **aborts the routing pass** through Java's pass-level catch. `TileShape.rotateApprox`'s two-corner branch builds a `LineSegment` at an invalid index and NPEs (~2 % of random degenerate 2..4-line shapes). `Polyline.cornerCount()` returns **−1** for an empty polyline, which flows into `new IntPoint[-1]`. | #22: guard the loop with `newLength >= 1`, as the trailing access already is. #24: pass `1` instead of `0`. #25: return `0`, or make the empty polyline unrepresentable — the second, since the port's `corner_count` already saturates. Register right on all three. | #22 medium (it changes what a degenerate polyline normalises to, and `PolylineTrace.combine_at_end` distinguishes "zero lines" from "threw"); #24/#25 low. | `polyline.rs`'s `remove_overlaps` error path and `tile_shape.rs`'s `rotate_approx`; all three have pinning tests today. |

**Tier 1 count: 31 rows** (6 non-termination + 12 reachable Java-side totalizations recorded but
needing no port work + 8 silent-loss + 17 reachable crashes, with #71/#89/#93 counted once each).

---

#### 3. Tier 2 — routing quality

*Every row where Java routes worse than a fixed version would.* This is the heart of the roadmap.
Ranked within-tier by expected board impact: **hang-adjacent room/door loss > wrong geometry on the
board > wrong obstacle decisions > dropped candidates and score > dead optimization arms > ordering
churn**. Rows excluded from this tier carry their reason.

##### 3.1 Rooms and doors the maze never gets (highest impact)

The maze can only route through doors that exist. Every row here deletes a door or a room silently,
so the search does not fail — it succeeds, worse.

| Rank | # | Effect | Fix | Risk | Re-pinned by |
|---|---|---|---|---|---|
| 1 | **#159** | `ShapeSearchTree90Degree.completeShape` **drops** a room it decided to ignore, where the base class and the 45-degree override keep it. It is not a rare arm: `completeExpansionRoom` passes exactly this pair on **every** room completion. **On a 90-degree board a room whose only overlap is the door it came through expands to nothing**, where the same board at 45 degrees expands normally. Measured `[4, 4, 0]` across the three regimes on one seed room. | Add the base class's fallthrough — the 45-degree override's line verbatim: `if (!ignoreShape.contains(currentShape)) { newResult.add(currentRoom); newBoundingShape = newBoundingShape.union(currentShape); } continue;`. Register right. | High: it changes every 90-degree board's routable space. But the current behaviour is indefensible, and the 45-degree sibling is the specification. | `crates/fr-router/tests/tree_ext.rs::only_the_90_degree_override_drops_a_room_it_ignores_by_shape` asserts `[4, 4, 0]` today and `[4, 4, 4]` in `Fixed`. `p6t2` (2 000 random seed rooms per regime, 0 diffs) is the `Java` control and the `Fixed` sweep. |
| 2 | **#160** + **#161** | `SortedRoomNeighbour.compareTo` is **not a total order**, and the `TreeSet` it feeds silently drops elements it decides are equal — **a door the room really has is never built**. Measured: **481 drops in 2 000 cases**. #161 is the same comparator's final tie-break subtracting a **room** id from an **item** id, two counters that collide constantly. | Make the comparator a total order: compare the last corners whenever the first-corner distances tie rather than only when the corners are equal, and either drop the `compareFrom` branch or apply it unconditionally; then compare the object **kind** before the id (or give the two id spaces disjoint ranges). Then `JavaTreeSet` can be replaced by `BTreeSet` here and no neighbour is lost. **Both rows are one fix.** | High: 481 restored doors per 2 000 completions is a large change in maze connectivity. This is the fix most likely to change every board — and most likely to improve them. | `crates/fr-router/tests/sorted_neighbours.rs::the_comparator_is_not_transitive_and_drops_the_same_neighbour_java_does` and `room_and_item_ids_are_compared_across_id_spaces`, both inverted. `p6t3` modes 1-3 with a `Fixed` axis; the drop count is the headline number. |
| 3 | **#164** | `removeCompleteExpansionRoom`'s `otherRoom(room)` binds the **narrowing** overload at compile time, so the `null` check below it skips **every door whose far side is an incomplete room — which is most of them**. Its javadoc describes the opposite. The skip is what keeps the method alive: the code past it indexes `touchingSides[1]` with no length check. | Take an `ExpansionRoom` parameter (or cast, as `completeNeighbourRooms` does) **and** length-check `touchingSides` before indexing. **Both. Fixing only the overload turns a silent skip into an `ArrayIndexOutOfBoundsException`** — the register says so and it is the most important sentence in the row. | High. The method starts doing what its javadoc promises, on every room removal. | `crates/fr-router/tests/engine_rooms.rs::init_connection_on_a_new_net_drops_the_net_dependent_rooms` (probe mode 3 verbatim — it *fails* on the wide overload today, which is how this was found). In `Fixed` it must assert the new room set. |
| 4 | **#163** | `Sorted45DegreeRoomNeighbours.calculateEdgeIncompleteRoomsOfObstacleExpansionRoom` never advances `currentCorner`, so the degenerate-side guard instead compares every side's end corner against the corner the walk **started** at. On the full `0..7` walk the **last side is always skipped** — an eight-sided obstacle room gets **seven** incomplete rooms and seven doors instead of eight. JVM-verified. | Add `currentCorner = nextCorner;` at the foot of the loop. Register right, and right that it is a routing change. | Medium-high: one extra door per obstacle room on every 45-degree board. | `crates/fr-router/tests/sorted_neighbours_regimes.rs::an_obstacle_room_with_no_neighbours_skips_the_last_side_of_its_octagon`, inverted (7 → 8). `p6t3` mode 8. |
| 5 | **#171** + **#170** | A four-key tie in `MazeListElement.compareTo` answers `0` and `TreeSet.add` **discards the newcomer whole** — a different backtrack path at the same cost is lost, not merged, and `door.getId()` is a hash so two *different* doors can collide into the tie. #170: a `NaN` `sortingValue` falls through to the next sort key instead of ordering, and the relation stops being transitive. | #171: add the remaining fields (`backtrackDoor`, `sectionNoOfBacktrackDoor`, `nextRoom`, `shapeEntry`, `roomRipped`, `adjustment`, `ripupCost`) to the comparison, or keep the cheaper backtrack explicitly. #170: order on `total_cmp`, **or** reject a non-finite `sortingValue` at the three `add` sites — prefer the second, since a NaN cost is a bug upstream, not a thing to sort. **This corrects the register, which lists them as equals for #170.** | High for #171 — it changes which path the maze finds for every tie, and ties are common. | `crates/fr-router/tests/maze_list_element.rs::a_full_tie_is_dropped_by_the_set` and `nan_sorting_value_falls_through_to_the_next_key`, both inverted. Needs a `p6t3`-class differential over the queue. |
| 6 | **#165** + **#166** | `completeExpansionRooms` is **not** the set of complete rooms that exist: `SortedRoomNeighbours` constructs rooms before it knows they survive, and two paths abandon them **still wired to live doors**. An abandoned room is never validated, never invalidated when the net changes, never removed from the search tree — and `completeExpansionRoom`'s scan can pick one as its `ignoreObject`, handing `completeShape` a room that is not in the tree it is querying. #166: the `catch` returns an **empty** collection for rooms it has already committed to the database and the tree. | #165: take the room id *after* the commit, and give `addCompleteRoom`'s `null` path a `removeAllDoors`. #166: hoist `result` out of the `try` and return it from the `catch`, or roll the committed rooms back. Register right on both; note it is also right that "returning an empty list while leaving them in the tree is the worst of the three". | Medium-high. | `crates/fr-router/tests/engine_rooms.rs::an_obstacle_splits_the_seed_into_the_java_room_set` (6 listed of 9 constructed) and `complete_expansion_room_answers_javas_empty_collection_on_an_injected_failure`, both inverted. |
| 7 | **#156** + **#167** + **#158** | Three room/page identities that are hashes over mutable state. #156: `ObstacleExpansionRoom.getId` packs the shape index into the item id with an **or**, so index ≥ 1024 aliases onto another item and item id ≥ 2^21 overflows a Java `int` — and that id is the third sort key of `MazeListElement.compareTo`. **A trace with 1024 segments or a board with 2 M items reaches it.** #167: `DrillPage.getId` hashes a field `getDrills` overwrites, so a page in the queue silently changes its own sort key and the set can neither find nor remove it. #158: `IncompleteFreeSpaceExpansionRoom.getId` NPEs on the whole-plane room and moves when the mutable shape is replaced. | Give `ExpansionRoom` a real identity — a per-engine counter, as `CompleteFreeSpaceExpansionRoom` already has. Key `DrillPage`'s id on the shape alone. Compute `IncompleteFreeSpaceExpansionRoom`'s id once, at construction, with a sentinel for the whole-plane room. **One fix: every expandable object gets a stable, injective id.** The register lists them as three rows with three local fixes; the correction is that they share a root cause and a single fix. | Medium. Ids feed the maze's tie-breaks, so every id change moves routing — but the current ids are provably wrong, not merely arbitrary. | `crates/fr-router/tests/expansion_rooms.rs::obstacle_room_id_aliases_above_1023_shapes`, `crates/fr-router/tests/drill.rs::get_drills_recomputes_when_the_net_changes_and_mutates_the_id`, and `incomplete_room.rs`'s two tests. All invert. |
| 8 | **#192** | `DrillPageArray.overlappingPages` mixes an `int` lower bound with a `double` upper bound in the same loop, so a shape whose upper edge lands exactly on a page boundary **stops one page short**. The page set feeds `DrillPage.getDrills`, whose ids order the maze queue, so this is an ordering input, not only a coverage one. | Compute both bounds the same way (`(int) Math.ceil` for the upper), and decide deliberately whether a shape ending exactly on a boundary touches the next page. Register right. | Low-medium. | `crates/fr-router/tests/maze_drills.rs`'s drill fixtures; `p6t1` on every connection that changes layer. |
| 9 | **#178** | `MazeSearchEngine.init` ignores the `boolean` its own overridden `add` returns, so `startOk` can be `true` with an **empty** queue — the caller pays for a whole engine construction, a `reduceTraceShapesAtTiePins` pass and a full round of room completion to learn the queue was empty from the start. | `if (mazeExpansionList.add(newListElement)) { startOk = true; }`. Register right. | Low. Mostly a wasted-work fix, but it makes `getInstance` answer `null` for a fanout window that is too tight, which is what the caller means by "initialisation failed". | `crates/fr-router/tests/maze_search.rs::the_fanout_gate_refuses_an_element_beyond_the_max_escape_length`, inverted. |

##### 3.2 Wrong geometry on the finished board

| Rank | # | Effect | Fix | Risk | Re-pinned by |
|---|---|---|---|---|---|
| 1 | **#177** | `TraceShover.insert` dereferences `board.changedArea` with no null check and its own `catch` **hides the NPE**, so on a board that is not marking its changed area the substitute traces are inserted **un-normalized** — un-split, un-combined, un-joined. JVM-pinned: **three** traces where a marked board leaves **one**. `ForcedPadRouter.forcedPad` — the *same loop* — guards the identical call, so the two mutating halves of the shove algorithm disagree about the same board state. | Compute `optArea` the way `ForcedPadRouter.forcedPad:439-444` already does, **and** narrow the `catch` so a real normalisation failure is not filed under the same message. Register right on both halves. | Medium. It changes the shape of every shove on an unmarked board — which is a lot of shoves. | `crates/fr-router/tests/forced_via.rs::trace_shover_insert_swallows_the_null_changed_area_npe_and_leaves_the_pieces_unnormalized` pins both sides as literals; it inverts. `p6t10b` mode `trace`, whose `changedArea=` axis exists for exactly this. |
| 2 | **#186** | `FoundConnectionInserter` hands `connectToTrace` a `Trace` **the insert has already split away**. Java's reference keeps the dead object alive, so the stub is inserted against a polyline the board no longer holds and the two tail removals then **delete both halves of the split trace**. Measured: the two halves of trace 4 are gone and their line survives only inside a combined trace, with two rational corners the stub produced. | Look the trace up by **id** after the insert, or re-derive the connection target from the board. Then the stub reaches a trace that exists and the tail removals stop deleting live copper. Register right. | High. This is one of the most visible geometry changes in the register — it changes what the board looks like where a connection lands in the middle of an existing trace. | `crates/fr-router/tests/inserter.rs::a_target_trace_the_connection_misses_gets_javas_connect_to_trace_stub` — it fails today on both the id lookup and a post-loop snapshot, which is precisely the `Fixed` behaviour. |
| 3 | **#55** | All four `BoardOutline` transforms assign the result to the **loop variable** of an enhanced `for`, so `this.shapes` is never written: **the outline does not move, turn, rotate or mirror**. Only the lazily-built keepout area follows, so after any of the four the outline's curves and its outside-keepout disagree — and `boundingBox`, `lineCount`, `getShape` and the search-tree line bands all keep answering from the untransformed shapes. Reachable from `BasicBoard.moveItems`/`changePlacementSide`. | Write back into the array. Register right, and right that the corpus must be re-run: *a board whose outline finally moves has a different routable region.* | High. | `crates/fr-board/tests/areas_and_outlines.rs::board_outline_transforms_leave_the_outline_shapes_where_they_were` pins both halves; it inverts. |
| 4 | **#5** | `RationalPoint.perpendicularProjection` uses `add` where `IntPoint` uses `subtract` — a sign bug giving a **wrong projection for any line not through the origin**. Reachable via `TileShape.nearestBorderPoint(Point)` from `ShapeTraceEntries` / `ShapeEntrySide`, i.e. the shove entry path. | Change to `subtract`, matching `IntPoint`. Register right. | Medium. It moves the shove entry point for every rational-coordinate projection, which is common after a tightening pass. This is the highest-value single-character fix in the register. | `crates/fr-geometry/src/rational_point.rs`'s pinning test, inverted, plus a `p2t11`-class differential over `ShapeTraceEntries`. |
| 5 | **#183** | `TraceTightenerAnyAngle.smoothenEndCornerAtTrace` reads `prevLineDirection` from the **same** line as `lineDirection` (the 45-degree sibling reads `length-2` and `length-3`; the any-angle *start*-corner method reads `startLineNo` and `+1`). The consequence is total: the `bend` arm needs two directions that two equal directions cannot satisfy, so **the whole `bend` branch of the any-angle end-corner smoothener is unreachable**. | Read `lines[endLineNo - 1]`. Register right: the branch starts firing and end-corner geometry changes. | Medium. Any-angle boards only, but every end corner on them. | `crates/fr-router/tests/tightener.rs`'s `smooth` fixture (two net-3 traces meeting at a right angle) — its row moves the moment the index is fixed, which is the `Fixed` assertion. |
| 6 | **#187** | `FoundConnectionInserter` sizes each `connectToTrace` stub from the **other** end's layer: the stub onto the target trace is inserted on the target's layer and sized from the *start* layer's `traceHalfWidth`, and vice versa. **There is no reading under which the width belongs to the layer the copper lands on.** Latent on the current corpus only because both layers happen to share a width. | Swap the two indices — or better, let `connectToTrace` take the width for the layer it has just computed. Register right; prefer the second. | Low mechanically, but it is invisible until a board has per-layer widths, which a DSN `(rule (width …))` per layer produces. | `crates/fr-router/tests/inserter.rs`'s `diag` test. Needs a **new fixture with per-layer trace widths** — there is none today, which is why the row is latent. |
| 7 | **#28** + **#29** | `PolygonShape.containsOnBorder` is a stub returning `false`, so `containsInside(p) == contains(p)` for every polygon; `cutout`, `enlarge`, `borderDistance`, `distance` and `Circle.nearestPointApprox`/`cutout` are unimplemented stubs returning `null`/`0`, so **`smallestRadius()` always answers 0 for a polygon** — and that feeds the clearance heuristics. KiCad exports polygon keepouts and zones routinely. | Implement them. `containsOnBorder`: test the point against each `borderLine`. `smallestRadius` in particular is what the register singles out and it is right to. | High — these are new implementations, not corrections, and every polygon keepout's clearance behaviour changes. Budget this as its own workstream. | New tests; `stubs_match_the_java_stubs` in `crates/fr-geometry` inverts. |
| 8 | **#7** + **#68** | `IntBox.borderLineIndex` and `IntOctagon.borderLineIndex` are stubs that log and return `-1`, and the live caller `ShapeAndEntrySide` receives that `-1`. In the same file, both dog-ear cuts are guarded by an **always-true** reference comparison, so a cut line that removed nothing still sets `cutOffAtStart`/`cutOffAtEnd` and the `fromSide` search then hunts for a border line that is not there. | #7: implement geometrically against `borderLine(i)`. #68: compare the shapes by **value**, which is what the clause was for. **They are one fix** — #68's `fromSide` search is exactly what #7's `-1` breaks. The register lists them separately and does not connect them; that is the correction. | Medium. Both sit on the shove entry path. | `p2t11` mode 5 pins all four `orthogonal`/`inShoveCheck` combinations; `Fixed` needs the new answers. |
| 9 | **#48** + **#57** | For a back-side item under `flipStyleRotateFirst`, `Component.rotate` adds `360 - angle` to the stored rotation and then rotates the **location** by `angle` — so the recorded rotation and the moved location disagree by `360 - 2·angle`. `ObstacleArea.rotateApprox` and `ComponentOutline.rotateApprox` have the identical split. The component's outline and its pads disagree after a back-side rotation. | Decide which of the two angles is intended and use it for both, **in all three methods at once**. Register right and explicit that fixing one without the other makes them disagree instead. | Medium. Back-side components only. | `crates/fr-board`'s three `rotate*` tests, all inverted together. |
| 10 | **#15**, **#16**, **#9**, **#13** | Four small geometry defects with real reach. #15: `indexOfNearestCorner` seeds with `Double.MIN_VALUE` (the smallest subnormal), so **a corner at distance exactly 0 is never "nearest"**. #16: `nearestBorderPointsApprox`' upward insertion shift copies the wrong element. #9: `FloatPoint.circleCenter` divides by zero on a horizontal input, giving `(x, NaN)` — the mechanism of #82. #13: `LineSegment.stairApproximation45` calls `functionValueApprox` (a function of *x*) with a **y**-coordinate. | #15 seed with `Double.MAX_VALUE`; #16 fix the shift and check callers taking `count > 1`; #9 handle the horizontal case by swapping the point roles; #13 use `functionInYValueApprox` and verify against 45° routing output. Register right on all four. | #15/#16 low; #9 lands with #82; #13 medium (45° output). | Each has a pinning test or comment today; all invert. #13 needs a new 45° directed case. |
| 11 | **#26**, **#88**, **#93**, **#23**, **#11**, **#17**, **#18**, **#32**, **#188** | The long tail of geometry defects with narrow or indirect reach: `PolygonShape.area()` **always returns 0** (its guard is `<= 2` where `dimension()` never exceeds 2) and is reached from `DsnFile` through `Shape.area()`, so plane autoroute settings are derived from a zero board area; `PolygonPath.boundingBox` applies `+ offset` to the running maximum on every even index, so the upper x bound grows by `width/2` **per coordinate**; #93 (see §2.2); `Polyline(Point,Point)` recomputes the end closing direction as a verbatim repeat of the start's, giving the opposite closing line from `Polyline(Polygon)`; `Simplex.cutoutFrom`'s `prevDivisionLine` is never assigned so both merge branches are dead (likely more output pieces than intended); `IntOctagon.contains(FloatPoint)` is inclusive on the border where `IntBox`'s is exclusive; `IntBox.divideIntoSections` skips the base class's `dimension()==2` filter; `Circle.translateBy(RationalVector)` returns **`this` unchanged** — a silently wrong shape where every sibling throws; `new Polyline(Line[])` normalises **the caller's array in place** and the flipped entries are fresh `Line` objects, which since #74 is board-observable. | As the register's columns state, all correct. #26's fix additionally needs the `corners[len-2]` read guarded for a 1-corner polygon — the register says so. | Low each; #26 and #93 medium because they change derived board-level numbers. | Existing pinning tests in `fr-geometry`/`fr-dsn`; all invert. |

##### 3.3 Wrong obstacle and clearance decisions

These do not change geometry directly — they change what the router thinks it may do, which is worse.

| Rank | # | Effect | Fix | Risk | Re-pinned by |
|---|---|---|---|---|---|
| 1 | **#65** | `ShapeTraceEntries.storeItems`' precedence bug (`&&` binds tighter than `\|\|`) means a `ComponentObstacleArea` is skipped **unconditionally**, so **a component keepout can never block a via placement**. The obvious intent — matching the comment and every sibling call — is that during a *pad* check both kinds are obstacles. | Parenthesise as `!isPadCheck && (a \|\| b)`. Register right, and right that the corpus must be re-run: *this decides whether component keepouts block forced vias.* | High. For a KiCad user this is the difference between a via landing inside a courtyard keepout and not. | New directed test placing a via inside a `ComponentObstacleArea`; `crates/fr-board/src/board/shape_trace_entries.rs`'s `// Java bug:` note inverts. |
| 2 | **#69** | `ShapeTraceEntries.storeTrace`'s three-way block test compares `contactItem.clearanceClassIndex() != contactTrace.clearanceClassIndex()` where `contactItem` **is** `contactTrace` — the disjunct is always false. By symmetry with the second disjunct the intent was `trace.clearanceClassIndex()`. **As written, a contact whose clearance class differs never blocks a shove.** | Change the third disjunct to `trace.clearanceClassIndex()`. Register right. | High. It decides whether a differently-classed contact trace stops a shove, i.e. whether the router shoves copper across a clearance-class boundary. | New directed test with two contacting traces in different clearance classes. |
| 3 | **#50** | `RoutingBoard.changeConductionIsObstacle`'s guard is `if (getIgnoreConduction() != value) return;` — so a call only does anything when the two are already **out of step** — and it ends by storing `setIgnoreConduction(!value)`, the negation of what it just wrote into every signal-layer conduction area. Together they make the flag a **latch that alternates** with the per-item flag rather than mirroring it. Non-signal-layer conduction areas are skipped entirely, so a power plane's `isObstacle` never changes here. **This flag decides whether copper pours obstruct foreign-net routing** — the single most user-visible boolean on a KiCad board with ground pours. | Decide what the flag means. If it is "the board-wide setting", the guard should be `==` and the store should be `value`. Register right, and right that the corpus must be re-run. **The register does not rank this; it should be near the top of Tier 2.** | High and highly visible. | `crates/fr-board`'s `Board::change_conduction_is_obstacle`; needs a new directed test over a board with a signal-layer pour and a foreign-net trace. |
| 4 | **#72** | `PolylineTrace.splitInsideDrillPadProhibited`'s precedence bug means the `lastCorner` half is tested for **this** trace too, and for a foreign trace whose first corner did not match — either of which answers "split allowed" even when a pad was found. **So a trace may be cut inside a pin pad.** | Parenthesise as `currentTrace != this && (first \|\| last)`. Register right. | Medium-high. | `crates/fr-board/src/board/trace_normalize.rs`; needs a directed test with a trace crossing a pin pad. |
| 5 | **#174** | `TraceShover.check`'s via arm returns `false` **without setting `shoveFailingObstacle`**, where every other refusal in the method records the culprit. `MazeRipupResolver` reads that field to decide what to tear up, so a caller that refuses here is handed a **stale item — possibly from a completely different `check` call on a different net.** The field is never cleared on entry either, so on a fresh board it can be `null`. | Set it to `currentShoveVia`, as the branch one level up already does. Register right. **Also clear the field on entry** — the register mentions the staleness but does not make that a fix; it should. | Medium-high. The router rips the wrong copper today. | `p6t9` mode `inst`, whose `failing=` column carries values left over from earlier rows — in `Fixed` it must not. |
| 6 | **#179** | `MazeSearchEngine.checkNeckDownAtDestPin` **never asks whether the pin is a destination pin**, and returns from inside the loop on the first `Pin` target door — start pin or destination pin, whichever the room lists first, and without looking at the rest if that one has no neckdown. Its caller narrows both `halfWidthAdd` **and** `halfWidth` to that value for the whole round, so **a start pin's neckdown silently shrinks the trace the search plans through a room it is only passing through.** | Test `isDestinationDoor()` inside the loop and `continue` rather than `return` when the pin has no neckdown. Register right, and right that then the name, the javadoc and the body agree. | Medium-high. It widens traces the router was needlessly necking down. | `crates/fr-router/tests/maze_expand.rs::check_neck_down_at_dest_pin_answers_the_first_pin_target_door_whichever_it_is`, inverted. |
| 7 | **#51**, **#60**, **#56**, **#58**, **#41** | Five stale-cache rows, one family: a memo whose input moved behind it. #51 `DrillItem.clearDerivedData` resets two layer memos but **not** `precalculatedMinWidth`, so a via that changes side keeps the minimum pad width of the padstack it no longer has **for the rest of its life** — and `minWidth()` feeds the autorouter's **neckdown** decisions. #60 `PolylineTrace.rotateApprox` alone among the four transforms does not `clearDerivedData()`, so a rotated trace keeps search-tree tile shapes for its pre-rotation position — *a stale tree shape is a wrong-answer obstacle, not just a slow one.* #56 `ComponentOutline.clearDerivedData` does not chain to `super`, so the autoroute scratch survives a move. #58 `setFlipStyleRotateFirst` clears no caches, so already-computed areas keep the previous flip style for ever. #41 `ShapeTree.insert` returns early for a zero-shape object **before** telling it, so a stale entry array survives an "insert". | Each is the one-line addition the register names. #51 and #60 first — they are the two that reach routing. | #51/#60 medium; the other three low. | Each has a pinning test in `crates/fr-board/tests`; all invert. |
| 8 | **#206**, **#207**, **#205** | Three `ViaOptimizer` rows. #206: `isWithinTolerance` is a **loose** Manhattan test standing in for a connectivity rule that is **exact**, and the tests are ordered `firstCorner` first — so a trace whose first corner is within a **37 821-unit** window of a via sitting exactly on its **last** corner reads the wrong end, and `repositionVia` is aimed at the wrong corner. #207: **no** `repositionVia` overload tests the board's angle restriction against the delta it produces, though the projection fallback in the calling method does — so the same method applies the restriction to one of its two answers and not the other. #205: `checkConnectionToPin`/`correctConnectionToPin` accept `pinEdgeToTurnDist == 0` but their only caller demands `> 0`, so the whole `== 0` band is dead acceptance. | #206: compare exactly, as `getNormalContacts` does, and delete the method. #207: apply the delta test to overload A's answer too. #205: make the three guards agree — `<= 0` at the two acceptors, since `pinEdgeToTurnDist == 0` means "no exit restriction". Register right on all three; it is right that #205's second option is the safe one. | #206 medium; #207 latent on the corpus but a correctness hole; #205 low. | `crates/fr-router/tests/via_optimizer.rs`'s two tests and `connection_to_pin.rs`'s; `p7t4` is the live diff. |
| 9 | **#35**, **#46**, **#175** | Three narrow ones. `LayerStructure.getSignalLayer(n)` out of range falls through and returns the **last layer of the whole stack**, signal or not, instead of failing. `ConductionArea.copy` returns **`null`** whenever `netCount() != 1`, including **zero** nets, where every other `Item.copy` always produces an item. `DrillItemMover.check` — a *check* — **mutates its caller's** `ignoreItems` collection, and the recursion re-enters. | Return `Option`/`null`; implement the multi-net copy (or at least the zero-net case, which needs no new logic); copy the collection unconditionally as `shoveVias` already does. Register right. | Low. #35's Java callers are all GUI; keep it for the port's own robustness. | Existing tests in `fr-board` and `crates/fr-router/tests/board_ext.rs`; all invert. |

##### 3.4 Dropped airlines, dropped candidates, wrong score

`BoardStatistics.connections.incomplete_count` comes from `DesignRulesChecker::get_incomplete_count`,
i.e. from `NetIncompletes`' Delaunay + Kruskal. `calculate_score` weights it by
`unroutedNetPenalty`, `normalized_score` feeds `BoardHistory`, and `AutorouteBatchLoop` steers the
whole pass loop off that. **So the two rows below are not report bugs — they change which board the
router keeps and when it stops.** The seed put #147 in Tier 3; it belongs here.

| Rank | # | Effect | Fix | Risk | Re-pinned by |
|---|---|---|---|---|---|
| 1 | **#82** (with **#9**) | Delaunay's bounding triangle is finite and sits with two corners **exactly on the axes**, and every edge flip is decided by a float circumcentre that silently degenerates (`NaN`) for three of the six coordinate coincidences — answering `false` = "legal, do not flip". Ordinary axis-aligned input triggers it. **Edges are lost at every grid size** (2×2 → 4/5, 3×3 → 15/16, 5×5 → 50/56, 6×6 → 75/85), i.e. on every pad row, column and BGA field. Usually the graph stays connected and the MST just picks a longer airline — but it can **come apart**: on a 7×7 grid straddling the origin, ~0.5 % of dense draws produce a disconnected result, and a witness pad gets **no incident edge at all**, so no airline is emitted for it and the net's incompleteness is **under-reported**. | Use an **exact** `inCircle` determinant over `Point` — the same `BigInt` machinery `sideOf` already uses — and push the bounding triangle out to a coordinate no input can be collinear with. #9's horizontal-case fix is part of it. Register right, and right that it must be post-parity and re-baselined in the same commit. | High. It changes the airline set, therefore the incomplete count, therefore the score, therefore which board every multi-pass run keeps. But the current behaviour under-reports incompleteness on exactly the boards KiCad users have. **This is the highest-value Tier 2 fix for a KiCad user.** | `crates/fr-board/src/datastructures/delaunay.rs`'s `square_pins_javas_four_edges` (4 edges) inverts to its own control's answer (5); `p2t13` modes 1 and 5 gain a `Fixed` axis; add grid sweeps at 2×2 … 7×7 asserting the full edge counts. |
| 2 | **#147** | `NetIncompletes.Edge.compareTo` is **not injective** — its own comment says the four coordinate tie-breakers exist "so that edges with the same length are not skipped in the set", and they do not achieve it. Two candidate airlines between **different** items whose corners coincide pairwise compare `0` and `TreeSet.add` drops the second. That is the case of **a via stacked on a pad, or two pads at one location** — utterly ordinary. The dropped edge is one fewer candidate for Kruskal, so the spanning tree joins through a longer edge or, if the drop took the only edge, does not join at all and **understates `incompleteCount`**. `Signum.asInt` also maps **NaN** to 0, so a NaN edge swallows every later edge or is itself dropped, depending on insertion order. | Break the remaining tie on the two `NetItem`s' identities — the array index is already to hand at the construction site, so `Integer.compare(fromIndex, otherFromIndex)` after the four coordinates makes it injective without changing any ordering that is currently meaningful. Guard the NaN separately with `Double.compare`. Register right on both halves. | Medium-high, same channel as #82. | The three unit tests in `crates/fr-drc/src/net_incompletes.rs` invert; `p5t1`/`p5t2` gain a `Fixed` axis. |
| 3 | **#197** + **#198** | `BoardHistory.getMaxScore` seeds its accumulator with **`0`, not `-inf`**, so an empty history can never trigger a board restore and a negatively-scored history can only trigger one against a board scoring below zero — `AutorouteBatchLoop`'s gate is a strict `>`. #198: `restoreBoard` **sorts the list in place** (under a *read* lock) and `getRank` reports the current list position, so a board's rank is its insertion order until the first restore and its score order after — and the pass loop **breaks entirely** when `getRank > BOARD_RANK_LIMIT`. **So how many restores have happened changes when the router stops.** | #197: seed with `Float.NEGATIVE_INFINITY`. #198: sort a copy, or keep the list in score order at insertion so `getRank` is stable. Register right, and right that either "changes which board every multi-pass run writes to SES". | High. These two decide the *final output* of every multi-pass run. | `crates/fr-router/tests/board_history.rs`'s `max_score_of_an_empty_history_is_zero_not_negative_infinity` (which already asserts the counterfactual), `restore_board_increments_the_restore_count_and_reorders_the_list` and `get_rank_depends_on_the_last_restore_sort`; `p7t2`'s `=== phase cap3 ===` block is the JVM control. |
| 4 | **#194** | `BoardStatistics`' fanout block decides `pinsToEscape` from `pin.getUnconnectedSet(pin.getNetNumber(0))` — **net index 0 only** — while `total` counts every SMD pin with `netCount() > 0` and `isPinEscaped` on the next line is net-blind. A pin connected on its first net and unconnected on its second is counted as needing no escape, and `BatchFanout` then **skips it**. | Loop the pin's net indices, as the item loops elsewhere in the same class. Register right. | Low-medium. Multi-net SMD pins are uncommon but not rare. | `crates/fr-router/tests/score.rs::a_multi_net_smd_pin_is_counted_on_net_index_zero_only`, inverted; `p7t7`. |

##### 3.5 Dead optimization arms

The brief names these explicitly. Each is a routing improvement the program was written to perform
and does not.

| Rank | # | Effect | Fix | Risk | Re-pinned by |
|---|---|---|---|---|---|
| 1 | **#208** | `AutorouteConnectionRouter.retryConnectionNecked` **re-uses the `TimeLimit` the failed first attempt already spent** — `TimeLimit` keeps its construction instant and is never reset. The connections the retry exists for are exactly the ones where the first attempt ran long and failed, so **the remaining budget is smallest precisely where the retry is wanted**, and on a hard board the second attempt is a no-op. It has no other clock. | Build a fresh `TimeLimit` in `retryConnectionNecked` (the parameter then disappears). Register right; its alternative ("say in a comment that the retry is deliberately time-boxed") is not a fix and should not be presented as one. **That is the correction.** | Medium. The necked retry starts running on hard boards, which is the whole point of it — expect more completed connections and more time spent. | `crates/fr-router/tests/batch_autorouter.rs::the_necked_retry_reuses_the_exhausted_time_limit` reads the retry's budget off `AutorouteEngine::time_limit`; it inverts, and the RED/GREEN measurement is already recorded (81 ms of a 96 ms call). |
| 2 | **#182** | `TraceTightener.avoidAcidTraps` is disabled by its own first statement — `if (true) { return polyline; }` — so 20 lines of `springOverObstacles` + `checkPolylineTrace` are **dead code** and all three `pullTight` overrides hand their argument straight back. **The port has no acid-trap avoidance at all**, because Java does not. | Restore it deliberately and re-measure — it changes every pull-tight result near a same-net pin. **This is not a bug fix, it is turning a feature on**, so it lands behind its own setting whose `Fixed` default is on. The register offers "delete the dead body" as an equal alternative; it is not — deleting it removes the option. **Correction.** | High and genuinely uncertain: nobody has ever run this code. Budget a measurement round before committing to the default. | `crates/fr-router/tests/tightener.rs`'s three regime tables all move; needs a directed acid-trap fixture (a trace approaching a same-net pin at an acute angle). |
| 3 | **#213** | `BatchAutorouter.getAutorouteItems` appends an item **once per qualifying net**, and the pass runner then runs a **fresh** `0..netCount()` walk for each appearance — so a two-net item is routed **four times in one pass**, on net indices with no relation to the ones that qualified it. The repeats are not idle: each runs against the board the previous one left, routing real connections and ripping real traces. | Append outside the net loop and carry the qualifying net with the item (`List<Map.Entry<Item,Integer>>` rather than `List<Item>`). Register right, and right that "that is a different program". | High. It changes what every pass does on any board with multi-net items. | `crates/fr-router/tests/pass_runner.rs::a_two_item_is_routed_four_times` and `the_inner_index_is_a_net_index_not_the_qualifying_one`, both inverted; `p7t1`/`p7t2` print the list in order so a duplicate is visible. |
| 4 | **#202** | `--max-items` calls `requestStop()` (**ALL**) where `--max-passes` calls `requestStopAutoRouter()` (**AUTO_ROUTER_ONLY**), and `RoutingPipeline.runOptimizationStage` returns early on the former. **So a run that stops on `--max-items` skips the optimizer stage entirely and writes an unoptimised board to the SES**, while a run that stops on `--max-passes` optimises normally. The log line says "Stopping auto-router" — the optimizer is not the auto-router. | Have the `maxItems` site call `requestStopAutoRouter()`, so both limits mean "stop routing" and neither silently cancels optimization. Register right. | Low mechanically; it makes `--max-items` runs slower and better, which is the point. | `crates/fr-router/tests/stop_and_progress.rs::max_items_stops_all_and_max_passes_stops_the_router_only` (which already asserts `optimizerStageWouldRun`) inverts; `crates/fr-router/tests/pass_runner.rs::max_items_requests_stop_all` inverts. |
| 5 | **#193** | Three HEAD-only guards that each admit, in their own comments, that an item's tree-shape indices go **stale while the search is running**. They silently `continue`, and one of them **resizes** `expansionRoomArr` mid-search. They are load-bearing — the corpus reaches all three. As the register puts it: *the guards convert a corrupted search into a quietly worse route.* | **Find out why the indices go stale and fix that.** This is a root-cause investigation, not a patch, and it is the single largest open question in the router. Until it is answered the guards must stay and the port must keep them. | Very high, and unbounded until scoped. Budget it as its own workstream with a discovery phase before any code. | No test can pin this yet. First deliverable is an instrumented run that records *which* mutation invalidated *which* index, over the six router stems. |

##### 3.6 Ordering churn — measure-only

Each of these is a lawfulness repair with **no correctness argument**: any total order is as valid as
any other, and "fixing" them changes routing without a prior reason to expect improvement. They are
listed so they are not mistaken for free wins, and they must not land without §7's numbers.

* **#44** — `Item.compareTo` subtracts the wrong way round, so items (and every `TreeSet` the search
  tree returns, and therefore the order the router visits overlapping items in) sort by **descending**
  id. The register's own column says the fix requires re-running the whole fixture corpus "because the
  visit order of overlapping items decides which shove attempt the router makes first". Exactly.
* **#63** — `UndoableObjects` is a sorted map keyed by #44's comparator, so every walk of the board's
  item list runs in descending id — and that decides the *structure* of every search tree built by
  `getAutorouteTree`, because `MinAreaTree`'s insertion heuristic is order-dependent. Same fix, same
  caveat. The register's "None; but a comment saying the order is load-bearing" is right.
* **#74** — `PolylineTrace.change` compares `Line`s by **object identity**, so a freshly built polyline
  always differs at index 0 and the "no change necessary" early returns are near-unreachable; the
  identity comparison also sets `keepAtStart/EndCount`, which decides how many search-tree leaves are
  reused rather than removed and re-inserted — and a re-inserted leaf lands elsewhere in
  `MinAreaTree`, so **the shape of the tree changes**. A value comparison disagreed on **1 403 of
  2 358** `change` calls on one board. Comparing by value would reuse *more* leaves (cheaper) and is
  arguably more correct, but it re-baselines everything. Measure first.
* **#210** — `TraceTightener45.smoothenStartCornerAtTrace` keeps the **last** matching contact of a set
  ordered by descending item id, so which contact shapes the new corner is decided by an id ordering
  with no geometric meaning (measured: contacts `{496, 495, 494}`, both 496 and 494 match, Java keeps
  494 and nothing geometric distinguishes them). **The Java-side fix is owed and the register says so
  correctly:** pick the contact on geometry — nearest, or smallest `translateDist`. Unlike #44/#63/#74
  this one *does* have a correctness argument, so it is the one row in this group that should land on
  its merits. **It is mis-grouped by the seed as a pure ordering bug.**
* **#1**, **#6**, **#2**, **#4** — geometry-level comparator and arithmetic repairs (NULL direction
  ordering, degenerate-line ordering, negative `factor % 8`, `y` truncated when only `x mod z` was
  checked). Cheap, lawful, and worth doing in `Fixed` **because they unblock lawful `Ord` impls**
  (the register's own candidate row), but they move sorting and therefore routing.
* **#61**, **#30**, **#75** — already fixed in the port and strictly better (a per-manager entry-id
  counter instead of a `private static int` shared by every board in the JVM; a per-call `Random`
  instead of a `static` one that makes shape division non-reproducible under concurrency; a
  `BTreeMap` instead of a bucket-ordered `HashMap`). **No work owed. Recorded because #30 is a
  precondition for §6's parallelism**, and because these must not be "restored" for parity.

##### 3.7 Input fidelity — wrong rules in, worse routing out

DSN/rules parsing defects that silently give the router the wrong constraints.

| # | Effect | Fix | Risk |
|---|---|---|---|
| **#102** | `Network.insertClassPairs` reuses the **outer** iterator for the inner loop, so the outer `while` runs exactly once: `(class_class (classes A B C) …)` writes only `(A,B)` and `(A,C)` — **`(B,C)` never gets the rule.** JVM-verified. | Take a fresh iterator positioned after `firstName`. Register right, and right that it adds clearance rules a design does not have today. | Medium |
| **#101** | `Network.readScope` assigns a net class's `useVia` list **by reference** when the structure scope named no via padstacks, so **the first net class's own list becomes the merged list** and every later class's `addAll` mutates it — the first class's via rule ends up holding every class's vias. JVM-verified. | `viaPadstackNames = new LinkedList<>(n.useVia);`. Register right. | Medium |
| **#96** | `Structure.setClearanceRule`'s two-entry branch discards the entry it is iterating and rebuilds the pair from `iterator.next()` twice, so a `(type a b)` rule is applied as the single pair `(a,b)` and written twice. Inside it, the quote-stripping loop tests `currentPair[1]` on both iterations, so a leading `_` can be stripped twice and `currentPair[0]`'s never. | Use `currentString` in that branch like the other two, and strip `currentPair[i]`'s underscore inside the `i` loop. Register right. | Medium |
| **#100** | `NetClass.readClassClassScope` has **no `skipScope` fallback** where its sibling has one, so an unrecognised sub-scope is re-scanned token by token as if its contents belonged to the `class_class` — a nested `(rule …)` inside an unknown scope becomes a rule of the outer class pair, and the unknown scope's `)` decrements nothing. | Give the loop the same `else { skipScope(scanner); }` its sibling has. Register right. | Low |
| **#99** | `Rule.readLayerRuleScope` accepts exactly **one** `(rule …)` per `layer_rule`; a second presents the loop with a `(`, Java warns and returns `null`, and `NetClass.readScope` stores that `null` in `layerRules`. | Make the rule loop read `(` then `rule` on every iteration. Register right. | Low |
| **#105** | `Wiring.readViaScope`'s net-number loop **never increments its index** (the line-for-line identical loop in `readWireScope` does), so a `(via … (net NAME))` with several subnets gets `[lastSubnetsNumber, 0, 0, …]`. The zeros are inert for connectivity, but `Item.netsEqual` compares element-wise so the duplicate check compares padded arrays, and `insertVia`'s `splitTraces` loop runs an extra pass per zero. | Add the `++currentIndex`. Register right; no fixture in the 105-file corpus reaches it, so this needs a **new fixture**. | Low |
| **#104** | `Network.createViaRule` takes an `attachAllowed` it never reads, so **a `(via_at_smd on)` control scope has no effect on a net class's `use_via` rule** — only on the via infos. | Use the parameter, or drop it. The register says dropping is behaviour-preserving; **using it is the fix, and the register does not distinguish them.** Correction. | Low |
| **#83** | *Not a bug — excluded.* `ClearanceMatrix.setValue` indexes J-then-I, and every Specctra writer writes both orders while `KiCadJsonReader` writes only one, so **a KiCad-sourced board's clearance matrix is genuinely asymmetric**. Java is internally consistent. Recorded here only as a standing warning for the KiCad reader Plan 8 wires: do **not** "fix" the asymmetry by writing both orders. | None. | — |
| **#92** | *Already fixed in the port — no work owed.* HEAD spells fifteen Specctra tokens in camelCase where the pinned 2.3.0 jar spells them snake_case, so **HEAD's own DSN writer output cannot be read back by HEAD's own lexer** — a re-read drops `host_cad`/`host_version`, every via rule, every per-type clearance rule and 26 wires, and shifts the bounding box. The port emits the 2.3.0 strings. Report upstream. | None here. | — |

##### 3.8 HEAD-only implicit overrides

| # | Effect | Fix | Risk |
|---|---|---|---|
| **#172** | `AutorouteControl.rebuildViaInfo` relaxes **two** routing gates for a pure-SMD net, overriding what the padstacks say: it forces `ctrl.attachSmdAllowed = true` while the per-via `ViaMask.attachSmdAllowed` keeps saying `false` (the two disagree inside the same object), and it multiplies `viaCostFactor` by `0.1`, making every via **ten times cheaper** for that net. HEAD-only, absent upstream. JVM-pinned: `minNormalViaCost` 400 vs 4000 on the same board. | Make the relaxation an **explicit router setting** rather than an implicit override of the DSN. Register right. This is a policy row: `Fixed` mode changes the *default*, it does not delete the behaviour. | High — dropping either half re-routes every pure-SMD fixture. |

**Tier 2 count: 78 rows.** (§3.1 = 15, §3.2 = 21, §3.3 = 17, §3.4 = 6, §3.5 = 5, §3.6 = 11,
§3.7 = 8 of which 2 need no work, §3.8 = 1; #9 and #93 cross-listed once each.)

**Excluded from Tier 2, with reasons:**

* **#62** — *not a bug, and the seed mis-tiers it.* The base `ShapeSearchTree` inflates a drill item's
  tree shape with `enlarge` and the 45-degree subclass with `offset`; on a diagonal line those differ
  by √2, so the same 100×100 pad inflated by 100 gives `-741/-259` from one and `-800/-200` from the
  other. The register is explicit: *"Not a bug — but it is the single easiest thing to get wrong when
  collapsing the three tree classes into one, so it is pinned."* The 45-degree subclass's comment
  gives the reason ("to avoid small corner cutoffs"). Nothing to fix.
* **#12**, **#14**, **#19**, **#20**, **#10**, **#8** — accepted divergences or "none needed" per the
  register's own column.
* **#3**, **#81**, **#150**, **#149**, **#190**, **#199** — diagnostic-only or log-only; §5/§6.
* **#77**, **#40**, **#191**, **#113**, **#117**, **#122**, **#129**, **#141**, **#64**, **#144**,
  **#145** — already fixed or deliberately not ported; the port is already the better of the two.
* **#209**, **#180**, **#157**, **#37**, **#38**, **#53**, **#21**, **#54**, **#107**, **#108**,
  **#97**, **#98**, **#31**, **#36**, **#39**(port side), **#79**, **#188**(port side) — latent,
  unreachable, or GUI-only; several appear in Tier 4 or §6 instead.

---

#### 4. Tier 3 — DRC and report accuracy for KiCad users

The DRC report is the artefact a KiCad user actually reads. These rows make it wrong, unstable, or
unparseable. None of them changes routing, which is why they sit below Tier 2 — but #154 and #152
are what a user notices first.

| Rank | # | Effect | Fix | Risk | Re-pinned by |
|---|---|---|---|---|---|
| 1 | **#154** | The report's first key is `"$schema": "https://schemas.kicad.org/drc.v1.json"` and **that schema is snake_case** — KiCad 9.0.1 writes `coordinate_units`, `unconnected_items`, `hole_clearance`, and freerouting **2.3.0 matched it**. HEAD does not: it writes `coordinateUnits`, `kicadVersion`, `unconnectedItems`, `holeClearance`. **So the document advertises a schema it does not validate against**, and any consumer written for 2.3.0 or for KiCad breaks. | Emit `DrcJsonFlavor::KiCad` — the spelling `$schema` promises — and keep the camelCase reader for one release. **Already decided (ruling W); Plan 8 wires the CLI default.** The enum's `Default` stays `FreeroutingHead` because that is the parity choice the crate's tests pin. | None. Both spellings are already carried and pinned. | `crates/fr-drc/tests/report_json.rs`'s `head_flavor_key_order`, `kicad_flavor_key_order` and `kicad_flavor_matches_the_real_kicad_schema` already exist. The work is CLI wiring. |
| 2 | **#152** | `isHole` calls **every `Pin` a hole, surface-mount pads included** — and its own comment admits it. A pad with no drill has no hole to keep clear of; KiCad reports the overlap as `clearance` and freerouting as `holeClearance`, and the description's first three words change with it. **BBD Mars-64 splits 64 `holeClearance` against 12 `clearance` on this predicate**, so it is load-bearing for the headline numbers. The information to tell them apart is one predicate away (`padstack.fromLayer() != padstack.toLayer()`). | `item instanceof Via \|\| (item instanceof Pin pin && pin.getPadstack().fromLayer() != pin.getPadstack().toLayer())`, and re-baseline. Register right, and right that it changes counts per `type`, not the total. | Low. Every DRC golden re-baselines, which is exactly what `Fixed`-mode goldens are for. | `crates/fr-drc/tests/report.rs::smd_pins_are_classified_as_holes` (a two-layer board with two overlapping layer-0-only pads), inverted. |
| 3 | **#153** | `Item.smallestClearance` is initialised to `-1.0` **once, at construction**, and only ever lowered — a monotone minimum over the **whole life of the item**, never reset. `getAllClearanceViolations` runs once per `generateReport` **and** once per `BoardStatistics` built with violations, and a routing job constructs those repeatedly between board mutations. **So by the end of a run every item's `smallestClearance` is the minimum over every pass of the whole session, not over the board as it now stands** — a board whose clearances were *improved* by routing still reports the worst any earlier call saw. | Reset the field at the top of `clearanceViolations()`, or drop it and let `ClearanceViolation.smallestClearance` fold over the returned list. The second; the field only exists because the GUI wanted a number the compute had already thrown away. Register right and prefers the same. | Low. | `crates/fr-board/tests/clearance_violations.rs::smallest_clearance_only_ever_falls_and_is_never_reset`, inverted (the raise case is already pinned on the Rust side). |
| 4 | **#146** | The dangling-trace dedup guards each candidate with `anyMatch(ui -> ui.firstItem == trace)` — the entries emitted **so far**, tested on `firstItem` alone — so a trace that is a net entry's `secondItem` or merely a member of its `allItems` is **reported twice**: once inside the net entry and again as its own `track_dangling`. The **via** phase has no guard at all. And because `firstItem` comes out of a `HashSet` (#144), which traces the guard catches is hash-dependent, so the emitted **count** moves between JVM runs. **Measured over the corpus: 17 of 112 rows reach it, and on every one the jar's `track_dangling` set moves across `-XX:hashCode=0..4`.** | Decide what the dedup is for. If a dangling trace should not also appear inside a net entry, test membership of `allItems` (a set built **once** before the phase, which also removes the O(n²) rescan); if it should, drop the guard and let the two phases be independent, as the via phase already is. Register right. | Low, and **partly already taken**: because the port fixed #144's hash-dependence (plan-5 ruling 3), it emits **112** violations on Natural Tone Preamp where the jar says 113-115 — a deliberate, documented divergence recorded in four places and explicitly *"not a number to tune"*. `Fixed` here completes it into a stable, deduplicated report; it must not be tuned toward any particular jar run. | `crates/fr-drc/tests/unconnected.rs`'s four tests (`the_dangling_dedup_only_checks_first_item`, `a_first_item_trace_is_the_one_case_the_dedup_catches`, `a_via_that_represents_its_net_is_still_reported_dangling`, `the_via_phase_has_no_dedup_at_all`), all inverted. |
| 5 | **#110** | `SesWriter.writeConductionArea` writes the boundary with `writeScopeInt` (every coordinate rounded, as the rest of the session file is) and each **hole** with plain `writeScope`, so a conduction area with holes emits `Double.toString` coordinates inside an otherwise all-integer SES file. **A session file's grid is integral by construction** and a consumer may reject it. | Give `Shape.writeHoleScope` an `int` variant and call it from `SesWriter`. Register right. | Low. No fixture in `tests/reference` has a signal-layer conduction area with holes, so this needs a **new fixture** — it is listed in the register's own coverage-debt row. | New fixture + `crates/fr-dsn/tests` round trip. |
| 6 | **#111** | `RulesWriter.writeRules` writes the design name **raw** where `DsnWriter.writePcbScope` quotes it, so a name holding a space produces a `.rules` header the reader's own `NAME` lexeme cannot read back as one token. `RulesReader.read` never validates the name, which is why nothing has noticed. | Use `identifierType.write(designName, file)`, matching `DsnWriter`. Register right, and right that the blast radius is other tools. | Low; it changes the first line of every `.rules` file, so the corpus re-baselines. | `crates/fr-dsn/tests/rules_round_trip.rs::rules_writer_matches_java_issue593`, inverted. |
| 7 | **#195** + **#196** | `BoardStatistics`' horizontal/vertical/angled breakdown walks `polyline.lines` and measures each **infinite line's two defining points** — including the two bounding lines that carry no segment at all, and after a shove a line's anchors have no relation to the segment it cuts out. **The three lengths do not sum to `totalLength`** (measured 121 606.75 against 130 610.65). #196: `bounding_box`'s `width` and `height` are handed the board's **lower-left corner**, so they read negative. Report-only — no score term reads either. | #195: walk `polyline.corner(i)`/`corner(i+1)`, which is what `totalSegmentCount` four lines above already counts. #196: pass `ur - ll`, or rename the fields and drop them from the `Unit.scale` block. Register right on both. | None. | Per-stem JVM tables in `crates/fr-router/tests/score.rs` and `p7t7`, both inverted. **§7 must not use these fields**; see the measurement caveat there. |
| 8 | **#144** | *Already fixed in the port — no work owed; Java-side fix owed upstream.* `getAllUnconnectedItems` iterates a `HashMap` and `HashSet`s of `Item`s that override neither `hashCode` nor `equals`, so the unconnected list is **identity-hash ordered** and not reproducible run to run — and with #146 the emitted `track_dangling` **count** moves (109/110/111 of 111 candidates across hash modes, and 110 vs 111 across two runs of the same mode). The port uses `BTreeMap`/`BTreeSet` and a lowest-id representative. | Java should give `Item` a `hashCode`/`equals` pair, or build `connectedSets` out of the `TreeSet` `getConnectedSet` already returns instead of copying it into a `HashSet` — **a one-word change**, as the register says. | None here. | Already pinned by four tests in `crates/fr-drc/tests/unconnected.rs`. |
| 9 | **#145** | *Already fixed in the port.* Every `%.4f` in the DRC report follows the JVM's default `FORMAT` locale, so a `de_DE` machine writes `expected: 0,0500 mm` into a **machine-readable JSON document whose `$schema` is KiCad's**, while every number that goes through Gson in the same document is locale-free. The port always writes `.`. Java should use `Locale.ROOT`. | None here. | None. | `crates/fr-drc/tests/report.rs::percent_four_f_uses_a_dot`. |
| 10 | **#151** | The DRC report's coordinate unit is hard-coded to `"mm"` at its only CLI call site, so four of `convertCoordinate`'s five branches are dead from the CLI — **including the fallback, the only arm that would honour a board's declared `(unit …)`**. Every Issue575 fixture declares `(unit um)`, so the report silently rescales by 1/1000 with no way to ask for anything else, and the `%.4f` means a `"um"` report would print four decimals of a micrometre while a `"mm"` one prints 0.1 µm — **the resolution of the text output changes with a parameter nothing can set.** | Give `-drc` a unit flag (or read `board.communication.unit`) and route it through `Unit.fromString`. Register right. Note the port already ports all five arms and keeps the `"MM"`-vs-`"mm"` asymmetry deliberately. | Low; it is CLI wiring plus one flag. | `crates/fr-drc/tests/report.rs::unknown_coordinate_unit_falls_back_to_the_board_unit` and `mil_and_inch_scale` already exercise the four unreachable arms. |
| 11 | **#148** | `AirLine.compareTo` compares **the net name alone**, so a `TreeSet<AirLine>` would collapse all 29 airlines of one net into one element, and `Collections.sort` over a mixed list is order-preserving only by accident of TimSort's stability. It is also an NPE waiting on a null `net`. Latent — nothing in the Java tree sorts or set-collects `AirLine`s. | Compare the net name, then `fromItem.getId()`, then `toItem.getId()`, then the four corners — or drop `Comparable` entirely, which the port already effectively does. Register right. | None. | `crates/fr-drc/tests/net_incompletes.rs::airline_compare_by_net_name_is_not_a_total_order`. |
| 12 | **#81**, **#212**, **#201** | Three diagnostics that lie. `PlanarDelaunayTriangulation.validate()` accumulates only on its leaf branch and throws away every inner node's answer, so the public "check the consistency of the triangles" **answers `true` unconditionally for every non-empty triangulation** and logs "check passed ok". `ItemRouteResult.improvementPercentage` divides two `int`s, so the via term truncates — a re-route that halves the via count scores the same as one that removes every via (the optimizer's own recomputation has the `(float)` cast and is right, so **the field is wrong and the number actually used is right**). `getHash`'s javadoc in three places says it hashes the **trace** state where it hashes the whole item graph — and that wrong comment is exactly what justified this port's original trace-and-via-only hash, under which **every trace-free board hashed alike** and three `BoardHistory` decisions diverged. | `result &= currentChild.validate();`; add the `(float)` cast; reword the three comments. All three register columns right. | None. | `validate_is_vacuous_on_an_inner_node`, `improvement_percentage_truncates_the_via_term`, and `crates/fr-board/tests/snapshot.rs::trace_free_boards_with_different_items_no_longer_collide`. |

Also here, needing no port work: **#149** (a hard-coded `focusNets = {98, 99}` debug block that walks
two arbitrary nets' item lists on **every** `calculateAllIncompletes`, to reach a commented-out call —
not ported), **#150** (the `getIncompleteCount` log line appends the `NetIncompletes[]` **array** where
it means the count, so each line reads `…: [Lapp/freerouting/drc/NetIncompletes;@1b6d3586 incomplete(s)`
— not ported), and **#155** (`DesignRulesChecker.drcSettings` is stored and **never read**; its
`includeWarnings`/`includeErrors` filter nothing, 12 of 14 construction sites pass `null`, and
`enabled` is `transient` so it does not even survive the Gson round-trip the class exists for — the
whole parameter is dropped in the port). #155's fix is in Tier 4, because it is a settings row.

**Tier 3 count: 18 rows** (12 ranked + #149, #150, #155 recorded, and #144/#145 already done).

---

#### 5. Tier 4 — settings and CLI predictability

A user sets a flag and the program does something else. Ordered by how often a real invocation hits it.

| Rank | # | Effect | Fix | Risk |
|---|---|---|---|---|
| 1 | **#128** | `DsnFileSettings` seeds the layer count via `setLayerCount`, which also **replaces both per-layer trace-cost arrays outright with fresh all-`1.0` arrays**. Its guard is true for essentially every real board, so the priority-20 DSN source contributes two fully-populated `double[]`s for almost every design — and `copyFields`' primitive-array rule is **first writer wins**, so from priority 20 onward those `1.0`s cannot be replaced. **A `.rules` file at 40, an environment variable at 55 and a `--router.*` flag at 60 all carry per-layer trace costs that are silently dropped.** The register calls it "the single most consequential quirk in Plan 4" and that is right. Per-layer trace costs are what steer direction preference, so this has a **Tier 2 spillover**. | Seed only `layers` — inline the array-sizing half of `setLayerCount`, or give it a variant that does not touch `scoring`. **Fixing #126 does not fix this** (here the arrays start `null`, so the reallocation branch runs anyway); the register says so and it is right. | Medium. Per-layer trace costs start reaching the router, which changes routing on any board whose rules file sets them. |
| 2 | **#119** | Navigating a property path through an array field sizes a `null` array at the **value's token count**, not the board's layer count, and writes `min(arrayLength, size)` elements with no error. Worse: `applyRouterSettingsForLoadedBoard` then calls `setLayerCount(boardLayerCount)` whenever the counts disagree, which reallocates and resets `routable`/`preferredDirectionHorizontal`/`bendCost` on **every** element. **So a `--router.layers.*` whose token count does not equal the board's layer count is silently discarded wholesale** — the user's per-layer settings never reach the router and nothing says so. JVM-verified. | Size the array from the board and **reject** a token count that disagrees, rather than trusting the value's arity. Register right. | Low, and it turns silence into an error message, which is the point. |
| 3 | **#120** + **#136** | `convertValue`'s boolean arm is `Boolean.parseBoolean`, i.e. `"true".equalsIgnoreCase(s)` — **every other string is `false`, silently**. So `--router.enabled=yes` **disables the router**, `--router.vias_allowed=on` **forbids vias**, and `--router.strict_drc=ture` reads as an explicit `false` rather than an error. It does not trim, so `" true "` is also false. Every other converter fails loudly. #136: `--router.enabled=` (empty value) is counted as an **explicit** choice from the property name alone, disarming the `-de`/`-do` batch-mode forcing, and then reads as `false` — **a trailing `=` turns routing off in an invocation that would otherwise have turned it on.** | #120: accept `true`/`false`/`0`/`1`, case-insensitively and trimmed, and throw otherwise — matching what the enum arm effectively does. #136: treat an empty value as "not provided", or reject it. Register right on both. | None. Pure predictability. |
| 4 | **#140** | `RouterSettings.validate` is **not idempotent**: it maps `maxPasses == 0` (the spelling for "no limit") to `Integer.MAX_VALUE`, and `Integer.MAX_VALUE > 9999` so the **next** call maps it to `9999`. The headless path merges **twice**, each merge ending in an unconditional `validate()`. So `--router.max_passes=0` yields `MAX_VALUE` from a single merge and `9999` from a real run — **a 200 000-fold difference in the routing budget, decided by how many merges happened to run.** | Make `validate` idempotent — a separate `unlimited` flag, or treat `MAX_VALUE` as in range — and stop calling it once per merge. Register right. | Low. |
| 5 | **#142** | The scheduler's `.rules` file is **parsed twice against two different layer structures**. At priority 40 the layer structure is discovered **from the file itself** (an insertion-ordered set of its own `(layer_rule …)` names), so a file naming `F.Cu` and `B.Cu` describes a *two*-layer stack; after the merge the identical bytes are re-parsed against `board.layerStructure`, so on a four-layer board that same `B.Cu` rule lands on layer **3**. Both results are applied. **One `.rules` file can set `layers[1]` on one path and `layers[3]` on the other, from a single rule.** Measured as 13 disagreeing rows of 84 before the port reproduced both parses. | Parse once, against the board when there is one — or make `readRouterSettings`' discovered structure explicit in the result so the caller can tell which indices it means. Register right. | Medium. It changes which layer a stale `.rules` file's rules land on, which is the same family as #112. |
| 6 | **#127** + **#126** | `boardSpecificTraceCostsApplied` is `private transient` and `copyFields` skips non-`public` fields, so **a merged `RouterSettings` inherits the source's tuned per-layer cost arrays with the flag back at `null`** — and `applyBoardSpecificOptimizations` then re-initialises exactly the costs the merge just carried across. #126: `setLayerCount` wipes every per-layer cost even when the count is **unchanged**, but only clears the applied flag when it reallocates — so it discards costs a `.rules` file just supplied and then still reports them as applied. | #127: make the flag `public` (it is already `transient`, so no JSON key appears), or copy it explicitly in `applyNewValuesFrom` as `clone()` does. #126: move the cost-array and per-layer resets inside the reallocation branch, or clear the flag unconditionally. Register right on both. | Low-medium; land after #128. |
| 7 | **#115** + **#116** | `copyFields` can **never merge a `false` or `0` primitive field**: `shouldCopy` requires `!sourceValue.equals(getDefaultValue(field))`, and `getDefaultValue` returns the *type's* default, not the field's initialiser. So a source that explicitly wants `include_warnings = false` cannot express it — the value is indistinguishable from "not set". #116: a `Set`/`List` field falls to the generic rule, which recurses into `HashSet`'s own fields, every one of which is `private`/`static`/`transient` — **the merge is a silent no-op**, so `DebugSettings.filterByNet` loaded from JSON never survives. | Box the primitives (`Boolean`/`Integer`) so `null` means unset, matching the nullable-wrapper convention the whole merger is built on; special-case `Collection`/`Map` fields, or make the field a `String[]` like its `operationFilters` neighbour, which does get the array rule. Register right on both. | None. |
| 8 | **#131** + **#133** + **#134** + **#135** + **#132** | The five legacy optimizer flags `-oit`, `-us`, `-is`, `-hr`, `-inc` and `-drc`'s router switch write a bridge **nothing reads**, so in Java they have **no effect at all** — `-oit 5` leaves the optimizer at 0.01, `-us global` leaves it `GREEDY`, `-inc GND` never ignores a net class. Compounding it: the bridge matches every flag by **prefix** (`-mpx 5` sets `maxPasses`) while the live path matches exactly, so a user's typo reaches the dead half and not the live one; `-mp` is `Integer.decode` on the bridge and `Integer.parseInt` on the live path, so `-mp 0x10` is 16 on one and silently absent on the other; `-oit -5` never reaches its clamp because the blanket rule refuses any argument starting with `-`; and one `-mt` token writes two different fields with two different clamps that never touch each other. | Wire the five flags onto the live `CliSettings` path, match flags exactly on both paths (or share one table), and use one parser. **Note this makes the port strictly more capable than Java** — a recorded product decision, not a drift. Plan 4 ruling 8 hands the decision to Plan 8; the register is right to flag it. | Low mechanically, but it is a capability change. `docs/cli-legacy-flags.md` carries the whole normalisation table. |
| 9 | **#121** + **#118** | `getFieldByNameOrSerializedName` does not filter by modifier and `setFieldValue` calls `setAccessible(true)`, so **`private` and `static final` fields are both reachable from a property path** — `--router.board_specific_trace_costs_applied=true` defeats the guard `applyBoardSpecificOptimizationsIfNeeded` reads, and `min_bend_cost` resolves to a `public static final` constant and throws. #118: the path splitter's character class is `[.:\-]`, so a hyphen is a **separator** — a property whose name contains one cannot be addressed, and `optimizer-max_passes` silently means `optimizer.maxPasses`. | Skip `static` fields and skip non-`public` ones as `copyFields` already does — **a property path is external input and should not reach a `private` invariant flag.** Drop `-` from the class. Register right on both. | None. |
| 10 | **#124** + **#125** + **#114** + **#139** | Four small ones. `validate` and `normalizeMaxThreads` disagree about `maxThreads == 0`: the setter treats 0 as "no limit" and gives every core, `validate` has no `== 0` arm, so **a `0` that reached the field through the merge engine survives and the router runs with a thread pool of size zero** (moot today — see #143 — but not once §6's parallelism lands). `validate` unboxes two `Integer` fields that are `null` on any `RouterSettings` that did not pass through `DefaultSettings`, so a merger without that source throws out of `merge()`. `RouterSettings.clone` re-copies every other scalar and **omits `resultJsonPath`**, so a clone's is always `null`. `getRunFanout` and `isFanoutEnabled` carry the **same javadoc and opposite defaults** (absent means run / absent means do not run). | The four one-liners the register names. #124 is a **precondition for §6's parallelism** and must land before it. | None. |
| 11 | **#155** + **#151** | `DesignRulesChecker.drcSettings` is stored and never read; `includeWarnings`/`includeErrors` (both defaulting `true`) are read nowhere in the tree, so **no violation is ever filtered out of a report**, and `enabled` is `transient` so it does not survive the round trip the class exists for. Compounded by #115: both are primitive `boolean`s, so a source could not turn either **off** even if a reader existed. #151 is the same shape one layer over (§4). | Either read the two flags in `generateReport` (filter by `severity`, already the string `"error"`/`"warning"`) or delete the class and the parameter. **If a reader is added, the flags must stop being primitives first**, or #115 makes `false` unreachable. Register right. | Low. |
| 12 | **#87** + **#84** + **#85** + **#137** + **#130** + **#138** | Six input/documentation predictability rows. `Keyword.GENERATED_BY_FREEROUTING` is unreachable from the scanner, so the read-side branch is dead and `dsnFileGeneratedByHost` stays `true` **even for a DSN freerouting itself wrote and stamped**. The lexer's skip/stop sets contain **8 (backspace) and not 9 (tab)**, although the code's own comments say "spaces, tabs" — so a tab is part of a DSN name and a backspace ends one. `nextToken`'s DFA and `nextDouble` implement **two different number grammars** in one file (`1e5` is 100000.0 as a token and 1.0 through `nextDouble`; `+5` is 5 and `null`). `-de board.brd` **drops** the file rather than treating it as the design, so the run fails later with a missing-input error instead of at the argument. `SesFileSettings`' javadoc says it reads SES files; `loadSettings` opens nothing and returns an empty object. `SettingsSource`'s javadoc spells the whole priority ladder with the GUI at **50** where the constant is **65** — and the two disagree about whether a `--router.*` flag beats the GUI's live state. | Add the missing lexer rule (then re-check every consumer of `dsnFileGeneratedByHost`); use `{9, 32}`; parse both with the DFA's grammar; fail at the argument with the extension named; delete the SES rung or fix its javadoc; fix the one wrong number. Register right on all six. | None. |

Also here: **#202** (cross-listed from §3.5 — `--max-items` silently skipping the optimizer stage is
as much a predictability bug as a routing one) and **#172** (cross-listed from §3.8 — the pure-SMD
relaxation should become an explicit setting).

**Tier 4 count: 30 rows.**

---

#### 6. Explicit non-goals for the first fixed release

##### 6.1 Performance — recorded, and **last**

Nothing here ships in release 1 or release 2. Recorded so the candidates are not lost and so nobody
mistakes them for quality work.

**Standing ruling, unchanged by this roadmap: no threading policy.** No rayon, no threads. *"A
threaded maze would be non-deterministic and would dissolve every acceptance criterion in ruling 1."*
That is a hard gate on all of §6.1, and it holds until someone replaces ruling 1's acceptance ladder
with something a non-deterministic router can satisfy. If multi-threading is ever wanted headless it
is a **recorded product decision**, not a task detail.

* **Real parallelism.** #143 is the anchor: **neither thread-count field is read anywhere in the
  headless path** — and the Plan 7 ledger widens it further, finding `-mt` dead *everywhere*, not
  merely headless. `optimizer.maxThreads`' only readers are behind a GUI-only `multiThreading &&
  > 1` branch; `RouterSettings.maxThreads`' only non-GUI reader is
  `BatchAutorouter.autoroutePassMultiThread`, which **has no caller anywhere in the tree**. So
  headless freerouting is single-threaded in both stages regardless of `-mt`, and #132's clamps are
  arithmetic on a value nothing consumes. Building a headless multi-threading policy on `-mt` would
  make the port more capable than Java — a recorded product decision, not a router-task detail.
  **Preconditions, all already satisfied or listed above:** #30 (the port's per-call `Random` in
  `splitToConvex`, where Java's `static` one makes shape division non-reproducible under concurrency),
  #61 (the port's per-manager entry-id counter, where Java's `private static int` makes a query's
  answer depend on process history), #198's read-lock-that-writes, and #124 (`maxThreads == 0`
  surviving `validate` as a zero-sized pool).
* **Quadratic and repeated work.** #71's re-walk (quadratic on a dense net); #146's growing-list
  `anyMatch` (110 entries × 44 net entries on one fixture); #153's `getAllClearanceViolations`
  re-running once per `BoardStatistics` between board mutations; #149's two-net walk on every
  `calculateAllIncompletes`; #178's engine construction for an empty queue; #213's four routes of one
  item; #105's extra `splitTraces` pass per padded zero. **Several of these are fixed as a side effect
  of a Tier 1/2 correctness fix** — #71 by the ladder fix, #146 by the dedup fix, #178 and #213 by
  their own rows. That is the only perf work release 1 gets, and it is incidental.
* **Micro-allocation.** #80 (three dead `Edge` objects per interior Delaunay insertion — mutation-
  verified as unobservable, so it is a pure allocation leak); the `BigInteger` → `i128` candidate
  (Java promotes above 2^25 and most intermediate products fit, "a big speed win in
  `Line.intersection`, `perpendicularProjection`"); the memoisation candidates already discharged at
  item level.
* **Profiling.** No profiling target is set. When one is, the first measurement should be the six
  `tests/reference/router-*` stems at fixed pass counts, because that is the only end-to-end workload
  with a committed baseline.

##### 6.2 Cosmetic, diagnostic, and log-only

Excluded because no user-visible behaviour changes. Fix them opportunistically, never as a work item:
**#3** (`Direction.toString` prints `"RIGHT"` for `NULL` because it tests `compareTo(RIGHT) == 0`;
the `"NULL"` branch is dead), **#8** (`getId` hash-style `31*a + b` with silent `int` overflow — fine
as a hash, never as an identity), **#10** (`rint` vs `round` at one site), **#70** (a stale `34` in
`AGENTS.md` where the constant is `16`), **#97** (a dead `readOk` flag), **#98** (`Keyword.PN` is a
singleton the DFA can never return, so the identity half of a two-armed test is dead), **#107** (an
off-by-one whose only call site is commented out), **#108** (a warning that dereferences the `null`
it just tested for — free to fix, since the branch cannot currently do anything but crash),
**#117** (a merge count that measures boxed-reference identity), **#150**, **#190** (six hard-coded
debug net numbers — `33`, `66`, `67`, `98`, `94` — gating `FRLogger.trace` calls **in the algorithm**,
left in the shipped jar), **#199** (`autoroute/BoardHistoryEntry.java` is shadowed by a nested class
of the same name in the same compilation unit and is entirely dead), **#201**.

##### 6.3 GUI-only

Out of scope for a headless port and for this roadmap: **#36** (`ClearanceMatrix.removeClass` drops
every row maximum and leaves the matrix-wide maxima belonging to the *removed* class — the only
caller is `WindowClearanceMatrix`), **#58**'s trigger (`setFlipStyleRotateFirst`'s only caller is the
GUI, though the stale cache it leaves is a real Tier 2 row), **#59** (~200 lines of
`java.awt.geom.Area` boolean algebra for the renderer, deliberately not ported), **#129**
(`isAssignableFrom` source replacement, both classes GUI-only), **#130**, **#138**, **#204**
(`optChangedArea`'s clip-shape guard is a reference comparison against the `IntOctagon.EMPTY`
singleton — the accidental reading is unreachable because only GUI callers pass the singleton, and
the port already ports the value comparison). **#35**'s three Java callers are GUI too, but the port
should still return `Option` for its own sake.

##### 6.4 Already better than Java — do not "restore parity"

Recorded so a future reader does not read these as omissions: **#12**, **#30**, **#39**, **#40**,
**#61**, **#64**, **#75**, **#77**, **#92**, **#113**, **#117**, **#122**, **#129**, **#141**,
**#144**, **#145**, **#191**, **#200**, and the ~95 totalizations. Each has a `// totalized:`,
`// not ported:` or explicit-divergence marker; `docs/java-quirks.md`'s process notes define which
earn a table row.

##### 6.5 Open Plan 8 wiring that is not a fix

These are Plan 8 delivery items already decided, listed so they are not re-litigated as roadmap
rows: the `fr-dsn` `read_via_scope` → `insert_via_checked` line (the last of the ladder-hang seam,
`// obligation:` at `crates/fr-dsn/src/parser/wiring.rs:596`); `DrcJsonFlavor::KiCad` as the CLI
default (ruling W); `quality_score` computed rather than injected (the eight committed DRC
references pin the values, so it is eight free acceptance cases); `Freerouting.initializeDrc`,
including a **clock injection** — `fr-drc` has none and the report's `date` is
`ZonedDateTime.now()`; the API-job settings composition, which cannot reuse `resolve_headless`'s
linearised two-merge CLI path; `CancelToken` → `RouterStop`, which **must preserve the three-state
stop distinction and not collapse to a bool** (that distinction is #202); `ProgressSink` replacing
the dropped observers; `BoardComparator`; the MCP server's concurrency redesign (progress
notifications and cancellation, neither expressible today); legacy CLI value normalisation in
`legacy.rs`; KiCad JSON I/O and `SessionToEagle`; and the `route` subcommand itself, a stub
returning exit 3. **Until `route` exists there is no A/B harness**, so §7 depends on it.

Also carried, and *not* roadmap rows: the still-open **via-rule** half of the via-info re-pointing
obligation (`Network.addViaRule` → `NetClass.viaRule`; the via-info half closed in Plan 7 Task 0 by
making `ViaRule` own its `ViaInfo`s), and the four zero-coverage Plan 3 paths that need synthetic
fixtures with JVM ground truth — `SesWriter.writeWasIs`' swap body, `Component.readLockType`'s
`(lock_type position)` arm, #110, and #105. Two of those four are Tier 3/§3.7 rows above and cannot
be landed until their fixtures exist.

> **Import note (Plan 8 Task 14): §6.5 is written in the future tense and Plan 8 has now happened.**
> The text above is kept verbatim because it is the record of what was expected; here is what
> landed. **Every delivery item in the first paragraph is done**: `wiring.rs:596`'s
> `insert_via_checked` (Task 3), `DrcJsonFlavor::KiCad` as the CLI default (Task 7, ruling W),
> `quality_score` computed rather than injected (Task 7 — all eight references matched),
> `initializeDrc` with its clock injected (Task 7; the report's `date` is supplied by the caller
> and the port writes UTC, quirk #276), the API-job settings composition (**two** independent
> composers, Tasks 7 and 12), `CancelToken` → `RouterStop` **with the three-state distinction
> intact** (Task 0, and `cancel_all_and_cancel_auto_router_are_distinct` is the test that keeps it
> so), `ProgressSink` (Task 0), the MCP concurrency redesign (Tasks 11 and 12), legacy value
> normalisation (Task 5), KiCad JSON I/O (Tasks 8-10) — and `route` itself (Task 6), so **the A/B
> harness this section says §7 depends on now exists**. `BoardComparator` did **not** land: Plan 8
> built the report layer it was being held for and found no reader there either, so controller
> ruling AS rosters it — see the completion report's §2. `SessionToEagle` is likewise rostered,
> out of scope by spec §2 rather than dead.
>
> The second paragraph: the **via-rule half** of the via-info re-pointing obligation is still open
> and is still not a roadmap row — it is a decided behaviour change (Plan 6 ruling H), and the
> completion report's §5.5 carries it. The **four zero-coverage Plan 3 paths are CLOSED** (Plan 8
> Task 13): each has a directed synthetic fixture, JVM ground truth from `P8T13Probe.java` and a
> named test, and all four matched the jar's bytes first time. The one whose closure produced new
> information is #105, which is now the last row of §2.1 above.

**Register hygiene.** ~~`docs/java-quirks.md` records the next free row id as **#212**~~ *(true when this document was drafted; the register is now contiguous **1..292** and the next free id is **#293** — Plan 8 landed #236-#292)*; the Plan 7
ledger uses plan-local labels that do **not** correspond to register ids (its "#216" is #143's
extension, its "#200" is #202). Any new row this roadmap produces takes the next free register id and
cites it as such — never a ledger label.

---

#### 7. Sequencing and measurement

##### 7.1 Release 1 — "it does not lose your board"

**Scope:** all of Tier 1, plus the four Tier 2 rows whose absence would make Tier 1's fixes
misleading, plus the two zero-risk Tier 3 rows.

* Tier 1 §2.1 in order: **#71 → #76 → #106** (one workstream — #71 is the mechanism), then **#162**,
  **#86**, **#27**.
* Tier 1 §2.2: **#95 → #90** (ordered; #90's caller-side desync is in the scope #95 fixes),
  **#93 → #89 → #94** (one commit), **#112**, **#91**, **#103**, **#211 + #45**.
* Tier 1 §2.3: all of it. These are guards; they are cheap and they are the ones that turn a lost
  connection into a lost segment.
* Tier 2 carried forward because Tier 1 would otherwise mislead: **#82 + #9** (without it, "no data
  loss" is false — airlines are still silently dropped on grid boards), **#159**, **#164**, **#50**.
* Tier 3 free wins: **#154** (already implemented, CLI wiring only), **#151**.

**Release 1 is not allowed to change routing on the six router stems** except where a listed fix
makes it. Every other stem-level movement is a bug in the gating.

##### 7.2 Release 2 — "it routes better"

All of Tier 2 §3.1-§3.5, in the rank order given, then Tier 3, then Tier 4. Three sequencing
constraints:

1. **#160/#161 before #171.** Both are `JavaTreeSet` drops; fixing the neighbour comparator changes
   which elements ever reach the maze queue, so measuring #171 first measures noise.
2. **#5 before #7/#68.** The shove entry point must be right before the entry *side* is.
3. **#193 is a discovery workstream, not a fix**, and it should start early because its answer may
   subsume several §3.1 rows. Nothing in §3.1 waits on it.

**Deferred to release 3 or later, by risk:** #182 (nobody has run that code), #213 ("a different
program"), #172 (a policy change), #44/#63/#74 (§3.6 measure-only), #28/#29 (new implementations).

##### 7.3 How "better routing" is measured

Objectively, with numbers, on a fixed corpus, same binary, `Compat::Java` vs `Compat::Fixed`.

**The corpus.** Correct the seed here: the **eight** stems are the DRC set
(`tests/reference/drc-fixtures.txt`: `drc-dev-board`, `drc-bbd-mars-64`, `drc-natural-tone-preamp`,
`drc-issue593-rules`, `drc-issue593-ses`, `drc-issue753-cpu85`, `drc-issue110-relay`,
`drc-tutorial-board`), and they are boards, not routing runs. The **routing** corpus is
`tests/reference/router-fixtures.txt` — **six rows over five distinct boards**, 8 / 294 / 45 / 0 / 22
/ 294 connections:

| stem | board | connections | pass |
|---|---|---|---|
| `router-rpi-splitter` | `Issue143-rpi_splitter.dsn` | 8 | 1 |
| `router-dac2020-bm01` | `Issue508-DAC2020_bm01.dsn` | 294 | 1 |
| `router-j2-reference` | `Issue026-J2_reference.dsn` | 45 | 1 |
| `router-tutorial-board` | `tutorial_board.dsn` | 0 (deliberately) | 1 |
| `router-ecc83-input` | `Issue649-kicad_ecc83-pp_input_board_v1.dsn` | 22 | 1 |
| `router-dac2020-bm01-pass2` | `Issue508-DAC2020_bm01.dsn` | 294 | 2 |

**That is not enough for an A/B.** Before release 1 the corpus must gain, at minimum:
a **90-degree** board (nothing in the set exercises #159's regime), a board with **per-layer trace
widths** (#187, #128 are latent without one), a board with a **circular or ≥ 6.71 M-unit outline**
(#93/#94), a board with **multi-net SMD pins** (#194), and a board with **signal-layer pours**
(#50). Each is a fixture-design job with JVM ground truth, exactly as the register's coverage-debt
row describes.

**The metric set**, all already computed by `fr_router::score::BoardStatistics`
(`crates/fr-router/src/score/statistics.rs`) and its `normalized.rs` companion, at **fixed pass
counts** so run length is not a free variable:

| metric | source | direction |
|---|---|---|
| **completion rate** | `1 − connections.incomplete_count / connections.maximum_count` | ↑ |
| **total trace length** | `traces.total_length_mm` | ↓ |
| **via count** | `vias.total_count` (with `through_hole`/`blind`/`buried` split) | ↓ |
| **clearance violations** | `clearance_violations.total_count` | must stay **0** |
| **bend count** | `bends.total_count` (with the 90°/45°/other split) | ↓ |
| **normalized score** | `normalized_score(&ScoringSettings)` — `max(0, calculate_score/maximum_score) * 1000` | ↑ |
| **wall time** | driver-reported | reported, never optimised in release 1-2 |

Three caveats that must be written into the harness or the numbers will lie:

* **Do not use `traces.total_vertical_length` / `total_horizontal_length` / `total_angled_length`.**
  #195: they measure each infinite `Line`'s two *defining* points, including the two bounding lines
  that carry no segment, and they **do not sum to `total_length`** (121 606.75 vs 130 610.65 on one
  stem). Fix #195 first if the breakdown is wanted.
* **Do not use `board.bounding_box.width`/`height`.** #196: they hold the lower-left corner and read
  negative.
* **`normalized_score` is an `f32` throughout, deliberately** — Java's `float` association, three
  separate roundings in the penalty term, one narrowing in the cost term. Both
  `AutorouteBatchLoop`'s `> lastBestScore + 0.5` and `BatchOptimizer`'s `< improvementThreshold` are
  threshold comparisons on it. Do not widen it to `f64` for "cleaner" reporting.
* **`incomplete_count` is itself a fix target** (#82, #147). Report it **both ways** during those two
  fixes — the `Java`-mode count and the `Fixed`-mode count on the same board — or the improvement
  will be indistinguishable from the metric changing under you.

**The unit of comparison is Plan 7 ruling 1's per-pass tuple, not a single end-of-run number.** The
ruling already specifies exactly this ladder for the batch pipeline, and the A/B should reuse it
rather than invent a second one:

> **(a)** same pass count and, **per pass**, the same `(normalized score, incomplete count,
> clearance-violation count, via count, trace count)` tuple; **(b)** same item set after every pass
> (every trace polyline + layer, every via location + padstack); **(c)** byte-identical SES.

For an A/B the ladder inverts: (a) is what we *report* as the improvement, (b) is what we *explain*,
and (c) is what tells us the run is otherwise reproducible. The ruling's own rationale is the reason
to keep it: *"a per-pass tuple is what turns 'the SES differs' into 'pass 6's score diverged by
0.4'."* A single end-of-run delta cannot do that, and a fix whose gain appears in pass 1 and is given
back by pass 8 is a fix we want to know about.

**Two rulings constrain how the run is set up.** Ruling AI: run with the `optChangedArea` budget
**disabled**, on both modes, or the numbers are timing artefacts. Ruling AM: the A/B belongs in the
release, `FR_SLOW_PARITY=1` tier — `router-dac2020-bm01` (294 connections, twice over) is already
`#[cfg_attr(debug_assertions, ignore)]` for exactly this reason, and it is the stem an A/B most needs.

**The harness.** `scripts/ab-route.sh <stem…> --passes N --modes java,fixed`, writing one row per
(stem, mode, pass) as JSONL, plus a summary table. It needs four things that do not exist yet:

1. **The `route` subcommand** (§6.5) — `crates/freerouting/src/commands/route.rs` is six lines
   returning exit 3.
2. **The `mode` column** in `router-fixtures.txt` and `--mode=fixed` in
   `scripts/gen-router-reference.sh`.
3. **The extra fixtures** listed above.
4. **`scripts/gen-batch-reference.sh` and the `p7t9` SES-byte driver** — both are already planned
   (Plan 7 Task 16) and neither has been run. They are the right vehicle for the (c) rung, and the
   A/B should wait for them rather than grow a parallel SES comparison.

Until `route` lands, `scripts/differential/rust/src/bin/p6t1.rs` is the stand-in — it already takes
`<dsn> [maxItems] [ripupPassNo] [rules|-] [1-5|1-8] [neckWidthUm]` and emits one JSON line per
connection, which is the right shape; it needs a `--compat=java|fixed` argument and a `p7t7`-style
statistics tail. Note that `scripts/gen-router-reference.sh` deliberately runs *the same* `P6T1.java`
driver `run.sh p6t1` diffs, "so the reference and the differential can never describe different
runs" — the `Fixed` generator must preserve that property against its Rust side.

**Do not build a new score comparator.** `BoardScoreBreakdown::of` / `toSummaryString` and
`ScoringWeightComparison::{compare, isCandidateBetter, toReportString}` are Java's own
board-comparison machinery, and all five are already rostered `// added in Plan 8:` at the foot of
`crates/fr-router/src/score/mod.rs`. Porting them *is* the A/B report renderer, and it comes with
Java semantics for "is this candidate better" that we would otherwise be guessing at. Port them for
this purpose.

> **Import note (Plan 8 Task 14).** ~~"already rostered `// added in Plan 8:`"~~ — those five
> markers were Plan-8 *deferrals* when this was drafted, on the theory that the CLI/manifest
> surface Plan 8 builds would be their reader. Plan 8 built it and **it is not**: controller ruling
> **AS** rosters both classes `// not ported:` (no caller in `src/main/java` at all, plus quirk
> **#236**'s unit divergence), and Task 14 re-pointed all five markers accordingly. **The
> recommendation above still stands and is unaffected** — porting them for the A/B report renderer
> is a *new reader*, which is exactly what the roster says does not exist today. Doing so would
> also be the moment to fix quirk #236: `BoardScoreBreakdown.of:140` reads `traces.totalLength` in
> raw board units where its own javadoc says millimetres and `calculateScore:608-609` prefers
> `totalLengthMm`, so a breakdown does not add up to the score it claims to explain on any board
> whose DSN resolution is not 1.

**Precedent for a measured number that is allowed to move: `AIRLINE_BUDGETS`.** `fr-drc` carries a
per-row differing-line budget for `p5t2` that is a *measured ratchet, not a fact about the
algorithm*, reproducible only because `-XX:hashCode=2` is deterministic, and whose standing
instruction is to **regenerate it after any deliberate ratsnest change and read every number that
grew**. #82 and #147 are exactly such changes. Treat the A/B table the same way: a committed
artefact that is regenerated deliberately and whose every movement is read, not a threshold that
silently passes.

**The acceptance rule.** A fix lands if, over the corpus at a fixed pass count:

1. completion rate does not fall on any stem, and rises on at least one;
2. clearance violations stay at 0 on every stem;
3. no stem's normalized score falls by more than the `f32` noise floor;
4. and — the one that matters — a **named, reviewed explanation** exists for every stem whose
   geometry moved. "The numbers went up" is necessary and not sufficient; the register's whole method
   is that a movement nobody can explain is a defect nobody has found yet.

A fix that improves the score but cannot explain its geometry changes stays behind `Compat::Fixed`
without becoming the default. A fix that makes the score worse and is still correct (#82 raising the
honest incomplete count is the obvious candidate) lands anyway, with the reason recorded — the score
is a proxy, and the roadmap's direction is better routing, not a better number.

---

## 10. Upstream-PR candidates

*(Plan 8 Task 14. Not part of the imported roadmap; this section is the completion report's own,
because several rows above are Java defects the **port** cannot fix for anyone else.)*

The port reproduces every one of these deliberately. Fixing them here would break parity and would
help nobody using the jar. They are listed because each is a small, well-evidenced change to
`freerouting/freerouting` that a maintainer could take, and because this repository already holds
the reproduction and the ground truth for each.

| candidate | Java | what to change | the evidence this repo holds |
|---|---|---|---|
| **quirk #105 — a padded zero net number hangs the CLI for ever** (**controller ruling BI**, and the strongest candidate on this list) | `io/specctra/parser/Wiring.java:684-687`, against its own twin at `:441-445`; the crash is `drc/DesignRulesChecker.calculateAllIncompletes:558`'s `rules.nets.get(0)` = `Vector.get(-1)` | add the missing `++currentIndex` to `readViaScope`'s net-number loop | `crates/fr-dsn/tests/data/p8t13-via-net-numbers.dsn` (the jar never terminates on it) **and its one-token control** `p8t13-via-net-numbers-control.dsn` (the jar exits 0 with a 1 995-byte SES). Test: `dsn_reader.rs::read_via_scope_pads_a_multi_subnet_vias_net_numbers_with_zeros`, which asserts both jar verdicts. Report: `task-13-report.md` §4 |
| **quirk #39 — `MinAreaTree.removeLeaf` on an already-removed leaf silently destroys the whole tree** | `datastructures/MinAreaTree.java` | guard the double-remove; the port asserts instead | The roadmap's own §2.1 says it "should be reported upstream regardless of what this port does" — it is the worst silent-data-loss row in the register |
| **quirk #144 — `getAllUnconnectedItems` is identity-hash ordered** | `drc/DesignRulesChecker` | give `Item` a stable `hashCode`/`equals`, or sort the output | Already fixed in the port (ascending id, plan-5 ruling 3). The jar's own instability is measured: 113-115 violations on `drc-natural-tone-preamp` across `-XX:hashCode=0..4`, which is why that stem is a permanent `XDIFF` |
| **the twelve reachable non-termination and NPE rows** the roadmap's §2.1 lists under *"Reachable totalization rows in the same family"* | `Shape.readPolygonPathScope`/`readPolylinePathScope`, `Library.readPadstackScope`, `Structure.readBoundaryScope`/`readLayerScope`, `Component.readLockType`, `NetIncompletes.calculateNetItems`, `RulesWriter.writeRules`, `SpecctraDsnStreamReader.zzScanError`, `Library.readScope`, `Wiring.readScope`, `Package.readRotation`, `RulesReader.applyViaRule` | each is a missing EOF or null guard; every one is a few lines | Each has a `// totalized:` marker in `fr-dsn` naming the Java line, and a test. `RulesWriter.writeRules` is **corpus-confirmed**: `fixtures/empty_board.dsn` kills the 2.3.0 jar where the port writes a valid 20-line file |
| **quirk #95 — `autoroute_settings` is silently discarded when a keepout precedes it** | `io/specctra/parser/Structure.readScope` | hoist the `AutorouteSettings.readScope` call out of the `if` | Exporters conventionally write keepouts first, so this discards the router settings of a large share of real files *and* corrupts the tail of the structure scope. `crates/fr-dsn/tests/structure_scope.rs::an_autoroute_settings_scope_after_a_keepout_is_never_read` |
| **quirks #93/#89/#94 — a board at or above 6 710 886 DSN units degenerates to a 2000×2000 box, reporting `Success`** | `Structure.createBoard`'s `int` overflow loop, `CoordinateTransform`'s IEEE division, the circle bounding box | one change, not three: `double` scale factor, reject a zero/non-finite factor, `coor[0] / 2` on the circle bounds | JVM-verified to report `Success`. Three tests in `crates/fr-dsn` pin the Java behaviour today. **The corpus has no board that large**, which is why this was never noticed |
| **quirk #112 — a `.rules` file naming an absent layer overwrites the default trace width on the whole board** | `io/specctra/RulesReader.applyRules` | `return` after the warning | `layerIndex = -1` is the sentinel the branches below read as "all layers". Reachable from any rules file written for a different stack-up — i.e. the ordinary way a rules file goes stale |
| **quirk #268 — `-do out.dsn` writes a 0-byte file over the previous result and exits 1** | `Freerouting.java:123`, `:196-213`; `RoutingJob.tryToSetOutputFile:377-397` | test `tryToSetOutputFile`'s return value, or widen the accepted set | Compounded by quirk #265, which deleted the previous result before routing began. `cli_e2e.rs::do_out_dsn_writes_zero_bytes_and_exits_1` asserts the 0-byte file exists |
| **quirk #271 — `-drc` exits 0 whatever it finds**, violation count included | `Freerouting.java:246-374` | a `--fail-on-violations` flag; changing the code silently would break every existing CI script | A board with 107 clearance violations exits 0 exactly like a clean one, because nothing reads `report.violations.size()`. Four `cli_e2e` tests and `p8t3 e2e`'s five argv rows |
| **quirk #154 — the DRC report advertises a schema it does not write** | `io/kicad/KiCadDrcReport`'s `@SerializedName`s | write KiCad's snake_case, which is what `$schema: https://schemas.kicad.org/drc.v1.json` promises and what freerouting 2.3.0 itself wrote | The port already ships KiCad's spelling by default (ruling W) and keeps `--schema freerouting` for byte comparison against the jar |

Two things a maintainer should know before taking any of these. **The port is not a reference
implementation of the fix** — it reproduces the bug, and the fixed behaviour lives behind the
roadmap's `Compat::Fixed` switch, which does not exist yet. And **every row here has a committed
fixture and a named test in this repository**, so the reproduction step of an upstream report is
already done.

---

*End of the project completion report. `docs/java-quirks.md` is the register (292 rows), the seven
earlier hand-offs are the per-plan record, and `crates/*/README.md` are the per-crate references.*
