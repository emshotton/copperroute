# Plan 7 — `fr-router` part 2 (the batch pipeline: fanout, passes, the optimizer and `optChangedArea`) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port everything *above* `AutorouteConnectionRouter.route`'s step 5 — `autoroute/pipeline/**` (the fanout pre-pass, the pass loop, the pass runner, the batch autorouter and the optimizer), `board/optimize/ViaOptimizer`, `TraceTightener.optChangedArea`, `RoutingBoard.{fanout, optChangedArea, removeItemsAndPullTight}`, `PolylineTrace`'s `ConnectionToPin` trio, `autoroute/{BoardHistory, RoutingFailureLog, ItemRouteResult}` and the score-relevant subset of `core/scoring/BoardStatistics` — so that **running the whole headless `-de/-do` pipeline over a fixture produces the same SES bytes as the Java HEAD jar**, pass by pass, on the fixture corpus. ≈ 7 500 Java LOC in scope (≈ 6 830 from the survey's §1.1 subtotal, ≈ 520 for the `BoardStatistics` score subset, ≈ 150 for the ruling-H `fr-board` fix), plus ≈ 1 240 lines rostered `// not ported:` with caller evidence.

**Architecture:** Spec §2 (scope line "Routing pipeline: fanout → autoroute passes → optimizer passes"), §4 (`fr-router` = "…batch fanout/autoroute/optimizer, tighteners, shover, via optimizer"), §9 (**Batch** and **Optimise** — the two groups Plan 6 explicitly deferred), §10 (`ProgressSink`, `CancelToken`), §14.3, §15 step 8 ("`fr-router` batch loop, fanout, optimizer, tighteners → metric parity" — strengthened to SES byte parity by ruling 1 below). Plan 7 adds no crate: `fr_router::score` (ruling AG) holds the score-relevant `BoardStatistics` subset that Plan 8's `fr-core` re-exports, and `fr_router::pipeline` holds `run_pipeline` (ruling AK) that Plan 8's `RoutingPipeline` wraps. `RoutingBoardExt` — created in Plan 6 (plan-6 ruling 3) — gains `opt_changed_area`, `remove_items_and_pull_tight` and `fanout`.

**Tech Stack:** Rust 2024; `fr-board` (Plan 2); `fr-settings` (Plan 4); `fr-geometry` (Plan 1); `fr-drc` (Plan 5 — **promoted from a dev-dependency to a real dependency**, ruling 3); `thiserror`. Dev-only: `fr-dsn` (Plan 3) for the acceptance harness, `parity`. **No `rand`** (plan-6 ruling 5), **no `rayon`** (plan-6 ruling 17 and ruling AM's "General" clause), **no `slotmap`** (plan-6 ruling 16), no `tracing`, no threads, no watchdog thread (ruling AI), no static mutable state (**one recorded exception: `fr_geometry::Line`'s identity counter, controller ruling AE — see plan-6's amendment block**), no clock beyond `fr_board::TimeLimit`.

**Spec:** `docs/superpowers/specs/2026-08-27-freerouting-rust-port-design.md` (§2, §3, §4, §9, §10, §14, §15). Also binding: `docs/plan-6-handoff.md` §Obligations → Plan 7 (written by Plan 6 Task 18; while it is in flight the same obligations are in `.superpowers/sdd/2026-08-29-plan-6-router-maze/task-16-report.md` §6 and `task-17-report.md` §4), `docs/plan-2-handoff.md`, `docs/plan-3-handoff.md` (rulings F and H), `docs/plan-4-handoff.md` (quirks #127/#139/#140/#143), `docs/plan-5-handoff.md`, `docs/java-quirks.md` (rows 1–193 and the obligation register).

**Later plans:** 8 `fr-core` + CLI/MCP (`RoutingPipeline` proper, `CancelToken`, the result manifest, the JSON/manifest surface of `BoardStatistics`).

> ### ⚠ Amended 2026-08-30 by the pre-flight conflict scan (17 controller rulings)
>
> A pre-flight scan of this plan against the tree, `docs/plan-6-handoff.md` and the Java clone found
> 31 real conflicts; the controller ruled on all 17 proposals. The scan table is
> `.superpowers/sdd/2026-08-30-plan-7-router-batch/preflight-scan.md`. **Every ruling is applied in
> place below and marked "*(scan ruling N)*" at the site.** The five that change what a task builds:
>
> * **Quirk ids run from #194, not #189** — the register is contiguous through #193 (ruling 1).
> * ~~**Task 5 ports `correct` + `swap` only.** `checkConnectionToPin` is already ported in Plan 6;
>   re-transcribing it would be verbatim duplication (ruling 5).~~ **STRUCK — the scan was wrong
>   and the Task 5 review adjudicated it (§A).** `checkConnectionToPin` was **not** ported: at
>   `92c214e`, `git grep -i connection_to_pin -- crates` was empty and every `ConnectionToPin` hit
>   was a marker or prose. The scanner almost certainly matched the deferral marker at
>   `tightener/mod.rs:771`. **Task 5 ports the trio**, and did. Ruling 5's *other* two corrections
>   — the marker is at `:771`, not `:775`, and it names two methods, not three — stand.
> * **Task 6 also ports `RoutingBoard.moveDrillItem`** — `Board::move_drill_item` does not exist,
>   though the plan's first draft consumed it as if it did (ruling 3).
> * **Ruling 7's second recovery boundary is struck.** `AutoroutePassRunner.java:144` is the catch of
>   the *dead* `runMultiThread`; `runSingleThread` has none, so the port must **not** `catch_unwind`
>   per item (ruling 9).
> * **`FanoutPin` is a `JavaTreeSet`, not a `BTreeSet`** — its comparator is keyed at run time by a
>   settings string (ruling 8). *(**Amended by Task 11.** The container is right; the stated
>   reason is not. `BatchFanout.java:773-775`'s `pinIndex` tie-break is **outside** the
>   `if`/`else if` chain, so an unrecognised string gives pure `pinIndex` order and the
>   comparator answers `0` only for two pins of one component sharing a `pinIndex` — which no
>   reader produces. The `JavaTreeSet` stands because that is a property of the *input*, not of
>   the comparator. Quirk #220; see the Task 11 report §2.1.)*
>
> Structural: `PassRecord` moves to Task 4 and `build_unrouted_report` gets a Task 10 stub (ruling 6);
> each struct is declared by the **earliest** task that writes methods on it (ruling 7);
> `AutorouteControl::from_settings` replaces the non-existent `new_for_net` and closes a `pub seam:`
> (ruling 4); the marker gate splits into a src gate and a test gate with an owner for every orphan
> (ruling 2); Task 1 is **not** split, so this stays an 18-task plan (ruling 11); Task 0 is in flight
> and blocks all dispatch until it commits (ruling 17).

> ### ⚠ Read rulings 1, 2 and 4, and controller rulings AF–AM, first
>
> **Plan 7 is where whole-board byte parity is claimed or lost.** Plan 6 proved the per-connection rung: 369 connections on five boards at `ripupPassNo` 1, 2 and 4, all three acceptance rungs, byte-identical under `-XX:hashCode=0..4`. Everything that stands between that and a `.ses` file is in this plan — the fanout pre-pass, the pass loop's best-board policy, `optChangedArea`'s tightener sweep and the optimizer.
>
> Three of those four are **wall-clock sensitive in Java** (§3.3 of the survey): `optChangedArea` carries a 1 000 ms `TIME_LIMIT_TO_PREVENT_ENDLESS_LOOP` that *changes the board's geometry* when it trips, and the fanout and the optimizer each carry a user deadline. Ruling AI is therefore load-bearing: the budget becomes a parameter, every `p7t*` parity run disables it **on both sides**, and a separate non-parity test proves it trips. A `p7t9` MATCH obtained with the budget live would be a coincidence, not evidence.
>
> The other headline is ruling AH: `BasicBoard.getHash()` is an **MD5 over `serialize(true)`**, which writes `board.getTraces()`, `board.getVias()` **and `board.itemList`** (`board/facade/BoardSnapshotManager.java:29-35`) — i.e. the whole item graph. The port's `Board::structural_hash -> u64` was justified as "a same-board membership test only" (`crates/fr-board/src/board/snapshot.rs:100-110`). Plan 7 makes it control flow in three places. Task 3 audits it field by field against that serialization and proves **decision parity**, not value parity.

## Global Constraints

All Plan 1–6 constraints and rulings remain in force (see each hand-off's §Rulings, and plan-6's amendment block for rulings X–AE). Additionally:

- **Java wins over plan text, and over the survey.** Every file:line in this document was read out of the clone's HEAD while writing the plan; the implementer re-derives it from Java and reports disagreement rather than trusting the plan. Places where this plan **already corrects the survey** are called out in rulings 6, 9 and 12 — that is the precedent, not the exception.
- **Java source authority is the clone's HEAD, and so is the parity jar** (`../freerouting/build/libs/freerouting-current-executable.jar`, JDK 25 at `/opt/homebrew/opt/openjdk@25/bin`, always `-Djava.awt.headless=true`). The pinned 2.3.0 jar (Plan 3 ruling 10) is **not** used anywhere in this plan: HEAD's `autoroute/pipeline/**` does not exist upstream in this shape (`AutorouteBatchLoop`, `AutoroutePassRunner`, `AutorouteConnectionRouter`, `BatchFanout`'s `Component`/`Pin` sort and `RoutingPipeline` are all HEAD-only), so a 2.3.0 reference would be a different algorithm.
- Behavioral port: reproduce Java bugs, with a `// Java bug:` marker at the site and a row in `docs/java-quirks.md`. `// totalized:` for crash→value changes, `// not ported:`, `// not reachable:`, `// renamed:`, `// added in Plan 8:`, `obligation:` per `docs/java-quirks.md` §Process notes. **The marker's Java method name must sit on the same line as the marker** (Plan 2 ruling 13 — `audit-port.sh` is line-based).
- No GUI, no `FRLogger`, no `TextManager`, no observers. Spec §10's `ProgressSink` replaces `NamedAlgorithm`'s three listener lists and `autoroute/events/**` (ruling AK); every `FRLogger.trace`/`info` payload and its guard is dropped, including `AutoroutePassRunner`'s five `log*` helpers (`:338-525`) and `BatchOptimizer`'s three JMX samplers (`:94-122`).
- `Result`/`Option` where Java throws or returns `null`; `catch_unwind`/`Result` only at documented boundaries. **Plan 6's five boundaries (plan-6 ruling 7) each degrade to a specific value and the port must produce that same degraded value, not propagate**; Plan 7 adds exactly **one** more (ruling 7 below — scan ruling 9 struck the second, which did not exist on the live path).
- **No static mutable state and no threads.** `max_threads` is dead in Java's headless path (quirk #143) **and, per the survey §3.4, on every path including the GUI's router half** — this crate is single-threaded and must not invent a threading policy. The one recorded exception to "no static mutable state" is `fr_geometry::Line`'s identity counter (controller ruling AE); Plan 7 **honours its contract** (ruling 4) rather than merely avoiding it.
- **`#![forbid(unsafe_code)]` at the top of every crate root this plan touches**, as Plan 6 Task 18 established (`crates/fr-router/src/lib.rs:1`). A new module never weakens it.
- **No new workspace dependencies.** `thiserror` is already a workspace dependency; `rand`, `rayon` and `slotmap` are **not** added. The one dependency-graph change is `fr-drc` moving from `[dev-dependencies]` to `[dependencies]` of `fr-router` (ruling 3), which spec §4's `fr-router → fr-drc → fr-board` line already anticipates. Any other need is a recorded ruling, not a silent `Cargo.toml` edit.
- Every task ends with `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and the `scripts/audit-port.sh` invocations from Task 17 (with `scripts/audit-map/fr-router.map`) run and their `MISSING` count recorded in the commit message — verified against the committed tree, not against a report.
- Commit trailer:
  ```
  Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_015f1ubhBBpiWHAooDsso97Z
  ```

### Conventions carried forward verbatim from Plan 6 (binding, not advisory)

1. **Java wins.** A disagreement between this plan and the clone's HEAD is resolved for HEAD, reported, and recorded as a plan amendment.
2. **Markers carry the Java method name on the same line** (`audit-port.sh` is line-based).
3. **Descending item id.** `board.itemList.startReadObject()` walks descending (quirk #63); the port's answer is `Board::get_items(..).rev()`. Every `TreeSet<Item>` is a `BTreeSet<ItemId>` iterated **descending** (quirk #44).
4. **`java_min` / `java_max`** (`fr_geometry`) wherever Java calls `Math.min`/`Math.max` on `double`s — they are NaN-propagating and `f64::min`/`max` are not (Plan 1 ruling 10; Plan 6 Task 8 fixed 29 sites).
5. **`java_round`** (`fr_geometry`) wherever Java calls `Math.round` — half-up on ties, not banker's. *(Plan 6's convention list also named `java_round_to_int`; **no such function exists** in the tree. If a site needs the `int`-returning overload, add it in the task that needs it and say so — do not assume it is already there.)*
6. **`JavaTreeSet` where a comparator is non-total or a key can mutate after insertion** (controller ruling Y). `std`'s `BTreeSet` and `java.util.TreeSet` **keep different elements and iterate the survivors in different orders** on a non-total comparator — measured, not assumed (`run.sh p6t3 3`). Plan 7 must record the decision **per container** (ruling 5).
7. **The `Line` identity-token contract** (controller ruling AE): every new `Line` comes from a constructor that mints a fresh token; a *copied* `Line` keeps its token, which is Java's aliasing. `Polyline::from_lines_in_place` (`board_ext/tightener/base.rs:43`, quirk #188) normalises the caller's array in place, as Java's `new Polyline(Line[])` does; every new `Polyline` construction site picks `new_polyline` vs `new_polyline_in_place` **deliberately**, with the Java line that decides it in a comment. The token takes no part in `PartialEq`/`Eq`/`Hash`/`Debug`.
8. **`.unwrap_or_default()` on `complete_expansion_room`** — quirk #166: `AutorouteEngine.completeExpansionRoom`'s `catch` at `:518` returns a **fresh empty** collection, not the rooms completed so far. The port's `Err` *is* the empty answer; never propagate it.
9. **Plan-6 ruling 7's catch boundaries degrade to a value.** Each of the five is a specific `AutorouteAttemptResult` or an unchanged room list, and the port produces that value. Plan 7's **one** addition (ruling 7) is not of that shape — it propagates, exactly as Java throws.
10. **`Err` propagation rules from Tasks 15/16.** `FoundConnectionInserter::get_instance` answers `Result<Option<_>, BoardError>`: `Err` propagates to `route_connection`'s `catch_unwind` and becomes a **bare** `FAILED` (no details); `Ok(None)` is a *message-carrying* `FAILED`. A chain `Err` is not only `Stopped` — `combine_traces` can raise quirk #109's. Below `AutorouteEngine.autorouteConnection:260` Java checks no stop flag, so the inserter is called with `&|| false` (controller ruling AC); Plan 7 does not change that.

## Rulings made while writing this plan (recorded here so they reach the user)

**The controller's pre-plan rulings AF–AM (`scratchpad/plan7-rulings.md`, 2026-08-30) are binding and are restated here so the implementer needs one document:**

- **AF — `BoardHistory` is PORTED** (Task 2). The best-board policy is visible in every SES; dropping it is a divergence, not a simplification. Supersedes plan-6 ruling 13's roster line and `crates/fr-router/src/lib.rs`'s eleven `// not ported: BoardHistory.*` markers.
- **AG — the score-relevant `BoardStatistics` subset (≈ 520 lines) lands in Plan 7 as `fr_router::score`** (Task 1), Java-exact including its rounding. Plan 8's `fr-core` re-exports it and owns the JSON/manifest surface. **No new crate in Plan 7.**
- **AH — `structural_hash` stays a membership test but is AUDITED and widened to cover exactly the field set Java's `serialize(true)` covers** (Task 3), with a Java-field → port-field table in the task. **Do not** attempt byte-exact Java serialization or MD5. A differential mode logs Java's three hash-equality **decisions** (`BatchFanout:152-156`, `BoardHistory.contains`, `BoardHistory.getRank`) beside the port's, and decision parity on the corpus is required. Where two distinct boards could collide only in the port, `Board::diff_traces` is the tie-break.
- **AI — wall clock: a deadline checked at exactly the points Java reads `job.state`/`isStopRequested`; no watchdog thread** (Task 4). Deterministic when not hit; when hit, the port stops at the same check Java would have (modulo timing). The 1 000 ms `optChangedArea` budget becomes a parameter (default 1 000 ms); **every `p7t*` driver runs with the budget disabled on BOTH sides** (the Java probe sets it through its own copy of the constant or by reflection, documented in the driver header) so parity is time-independent; a separate non-parity test proves the budget trips.
- **AJ — `retainAutorouteDatabase` is permanently `false` in the port** (folded into Task 8, which owns `BatchAutorouter`'s constants). The five `additionalUpdateAfterChange` markers are re-pointed to `// not reachable: RoutingBoard.additionalUpdateAfterChange (retainAutorouteDatabase is a Java benchmark-only system property)`; a test asserts the port exposes no setter. **No `fr-board` signature changes.**
- **AK — Plan 7 ports a minimal `run_pipeline(board, settings, stop, progress) -> PipelineResult` inside `fr-router`** (Task 15) sequencing fanout → autoroute passes → optimizer and calling `finish_autoroute`; Plan 8's `fr-core` wraps it. `NamedAlgorithm`'s listener lists are `// not ported:` in favour of spec §10's `ProgressSink` (a trait with a no-op default impl, **defined in Plan 7**).
- **AL — ruling H's `fr-board` fix is Plan 7 Task 0**, on the Plan 7 branch: `ViaRule` owns its `ViaInfo`s (or `ViaInfos` keeps tombstones). Pinned by the existing ruling-H `.rules` fixture + `p6t1` k=6/k=8 (which must become MATCH) + the `p3t15` sweep unchanged.
- **AM — acceptance budget.** CI (`cargo test --workspace`, `java_dir`-gated) runs `router-rpi-splitter`, `router-j2-reference` and `router-ecc83-input` end to end; `router-dac2020-bm01` and `router-tutorial-board` are `#[cfg_attr(debug_assertions, ignore)]` + `FR_SLOW_PARITY=1` in release. **`p7t9` compares SES bytes**; a normalised digest is allowed only as the recorded value of an `XDIFF` row with a root cause. Java-side full `-dr` runs are cached as `tests/reference/router-*/batch.{ses,meta.txt}` generated by `scripts/gen-batch-reference.sh` (sibling of `gen-router-reference.sh`).
- **AM/General — no rayon; all five multithread classes `// not ported:`** with the survey's evidence (`autoroutePassMultiThread:412` has zero callers; `BatchOptimizerMultiThreaded` is reachable only from `BatchOptimizer.createForGui:56-66`). Quirk ids from **#194**. Task size ≤ ~600 Java lines. **Exact parity per pass is the target**; time-limited stopping points are compared with the budget disabled.

**And the plan's own rulings:**

1. **Parity target is the SES bytes, per fixture, with a per-pass trace as the diagnostic ladder.** Acceptance for fixture *X* is, in order:
   **(a)** the same **pass count** and, per pass, the same `(normalized score, incomplete count, clearance-violation count, via count, trace count)` tuple;
   **(b)** the same **item set** after every pass — every trace's polyline corner list and layer, every via's location and padstack;
   **(c)** **byte-identical SES output** (ruling AM).
   A fixture that reaches (a)+(b) but not (c) is a row in `crates/fr-router/README.md` with the first differing byte and a one-line diagnosis, **not** a silent pass; a fixture that fails (a) is a port bug. *Reason:* spec §15 step 8 says "metric parity", but Plan 6 measured that a single-threaded Java run is byte-reproducible and hash-mode-independent, and ruling AM makes the SES bytes the target. A per-pass tuple is what turns "the SES differs" into "pass 6's score diverged by 0.4", which is the only way a 294-connection board is debuggable. *Cost if wrong:* the README records the demotion with its evidence — no re-plan.
2. **The Plan 6 / Plan 7 seam is unchanged and `route_connection`'s signature is final.** Plan 6 Task 16 fixed it with `trace_costs`, `start_ripup_costs`, `remove_unconnected_vias` and `retain_autoroute_database` as parameters precisely so `autoroutePassesForOptimizingItem`'s `removeUnconnectedVias = true` could be wired. Plan 7 **wraps** it as `route_connection_full` (Task 8) rather than extending it, so every Plan 6 test keeps its entry point and `p6t1` keeps working. *Cost if wrong:* nil — the wrapper is additive.
3. **`fr-drc` becomes a real dependency of `fr-router`.** `BoardStatistics`' constructor calls `new DesignRulesChecker(board, null)` twice (`BoardStatistics.java:265-268` for connections, `:338-341` for clearance violations), and `AutorouteBatchLoop`, `AutoroutePassRunner` and `BatchOptimizer` each call it directly. Spec §4 already draws `fr-router → fr-drc → fr-board`. *Reason:* the alternative — a callback trait so `fr-router` stays DRC-free — would put the port's most parity-sensitive number behind an indirection Java does not have. *Cost if wrong:* a dependency edge the spec already sanctioned.
4. **`fr_router::score` is a module of `fr-router`, not a crate, and it is the *only* home of `getNormalizedScore`.** Plan 8's `fr-core` re-exports `fr_router::score::{BoardStatistics, ScoreBreakdown}` and adds the Gson-compatible JSON. The `BoardStatistics(byte[], FileFormat)` constructor (`:436-554`, 119 lines — the SES/DSN text-scraping twin) and `countOccurrences`/`toString` (`:578-596`) are `// added in Plan 8:`, because their only readers are the manifest and the CLI. *Cost if wrong:* Plan 8 moves a module; the type does not change.
5. **Every ordered container gets a recorded decision, per container, before it is written** (plan-6 ruling 4 + controller ruling Y). The survey's §5.6 pre-classifies Plan 7's five candidates as total-and-`final`-keyed, i.e. `BTreeSet`-safe — **that classification is a hypothesis the implementer must confirm against Java, not a licence.** The table in Task 17's README is the deliverable. Specifically: `BatchFanout.sortedComponents` (`:52`), `BatchFanout.Component.smdPins` (`:673`), `BatchOptimizer.rippedItems`/`rippedConnections` (`:412`, `:428`), `AutoroutePassRunner.rippedItemList` (`:225`), `RoutingBoardOperations.changedNets` (`:93`), and — **the one the survey did not list** — `TraceTightener.optChangedArea`'s `board.overlappingObjects(changedRegion, i)` result (`:145`), which is a mixed room/item `TreeSet` whose `Ord` `fr-board` already encodes. *Cost if wrong:* a wrong-answer bug with no crash, visible only as a different route — which is exactly what ruling Y was created for.
6. **`AutorouteAirlineCalculator` is ported at exactly one method, and the plan says so rather than deferring the check. (This corrects the survey, which left "verify before porting" open.)** `calculateAirline` (`:16-40`) has one live caller, `AutorouteConnectionRouter.java:70`, and Plan 6 Task 16 already rostered that call `// not ported: AutorouteConnectionRouter.route's router.setAirLine (:70), a GUI progress sink` (`engine.rs:1647`). So the method's *value* is consumed by nothing on the headless path. Task 9 ports `calculateAirline` anyway — it is 25 lines and `AutorouteUnroutedReport` reads the DRC's airlines, so a reader will look for it — and rosters `:42-213` (172 lines) `// not ported:` with the `BatchAutorouterThread`-only caller evidence, **re-checked with a fresh grep at port time and the grep output pasted into the commit message.** *Cost if wrong:* 172 lines that no headless path reaches.
7. **One new recovery boundary, on top of Plan 6's five — and it propagates rather than degrading.** *(Amended by scan ruling 9.)* The boundary is `AutorouteBatchLoop.java`'s `anyRoutable` throw path (`:51-56`, the `throw` itself at `:55` — `TaskState.CANCELLED` plus an `IllegalArgumentException` that `RoutingPipeline.run` does **not** catch, so it escapes to the job scheduler). The port's `run_pipeline` answers `Result<PipelineResult, RouterError>` for it and `AutorouteBatchLoop::run` answers `Err(RouterError::NoRoutableLayer)`.

   **The originally-planned per-item boundary is struck.** The plan cited `AutoroutePassRunner.java:144` as "the per-item catch inside `runSingleThread`'s loop". It is not: `:144` is the **method-level `catch (Exception)` of `runMultiThread`** (`:40-149`), the dead multithreaded path this plan rosters `// not ported:` (quirk #216). `runSingleThread` (`:151-336`) has **no** try/catch at all, and the only genuine per-item catch in the tree is `BatchAutorouterThread.java:537`, also dead. **So Java does not finish a board on which one item throws — the pass aborts — and the port must not invent a `catch_unwind` that would make it finish.** Task 9 therefore adds no recovery boundary and lets a panic propagate. **`StackOverflowError` is caught by neither language (quirk #27) and stays fatal.** *Cost if wrong:* a port-only `catch_unwind` here would silently route a board Java abandons — a divergence with no failure signal, which is the exact class of bug ruling 1 exists to catch.
8. **The optimizer's undo is a `Board` clone, and the strict-DRC rollback is too.** `RoutingBoard.{generateSnapshot, popSnapshot, undo}` and `BasicBoard.{serialize, deserialize}` are all rostered `// not ported:` in `fr-board` (`board/mod.rs:170`, `:133`); spec §6 says `Board: Clone` replaces `deepCopy()`. `optRouteItem`'s snapshot is *pushed, then popped or restored within one call* (`BatchOptimizer.java:442, 501, 506`) and nothing else touches the stack, so a clone taken before `removeItems` and restored on failure is semantically identical. Same for `AutorouteConnectionRouter.java:84-85, 250-252`. **`BoardHistory` likewise stores `Board` clones, capped at 30 as Java is.** The memory difference (30 live boards vs 30 `byte[]`s) is recorded in Task 2's README section, not hidden. *Cost if wrong:* if a 30-board history proves too heavy on `Issue730-DAC2020_bm11`, the fallback is a `Vec<u8>` of the port's own compact encoding — a Task 2-local change, and the plan names it now so it is not an invention later.
9. **`optChangedArea`'s `clipShape != IntOctagon.EMPTY` is a *reference* comparison and the port reproduces it as "always true for a `None` clip shape". (This corrects the survey's cand. E, which read it as possibly-false.)** `RoutingBoardOperations.java:64` compares a `TileShape` reference against the `IntOctagon.EMPTY` singleton. Every router call site passes `null`, and `null != EMPTY`, so the branch **always** runs on the headless path. The port's `clip_shape: Option<IntOctagon>` maps `None` → runs, `Some(o)` → runs unless `o` **is** the `EMPTY` singleton — which the port cannot express, so `Some(IntOctagon::EMPTY)` also runs and the divergence is unreachable (no Plan 7 caller passes one). Quirk row, `// Java bug:` marker, and a test that `None` runs the branch. *Cost if wrong:* nil on the headless path; the row is what makes that visible.
10. **`getAutorouteItems`' multi-net duplication is reproduced exactly, including the wrong inner index.** `BatchAutorouter.java:363-403` appends `currentItem` once **per qualifying net index**, and `AutoroutePassRunner.java:202,207` then loops `for (int i = 0; i < currentItem.netCount(); i++)` over each appearance — so a 2-net item that qualifies on both nets is routed **four times** per pass, and the inner `i` is a *net index* rather than the index that qualified, so an item that qualified only on net #1 is still routed for net #0. Both halves are ported verbatim with `// Java bug:` markers and one quirk row. *Reason:* the visit count and order are the pass's identity; "fixing" it changes every SES. *Cost if wrong:* every multi-net board diverges from pass 1.
11. **Progress is a `ProgressSink` with a no-op default, and no port decision reads it.** Spec §10's `ProgressSink::on_event(RoutingEvent)` replaces `NamedAlgorithm`'s three `List<Listener>`s, `autoroute/events/**`, `ProgressThrottler`, `RouterCounters`' fire sites and `BatchAutorouter.shouldFireBoardUpdate`'s 250 ms gate. The **counters themselves** (`core/RouterCounters.java`, 48) are ported as a plain DTO because `AutoroutePassRunner.updateProgress` computes them and Task 16's per-pass trace asserts them; the *firing* is not. **A test asserts that swapping the sink for a recording one changes no board byte** (`structural_hash` equality across two runs). *Cost if wrong:* a progress callback that mutates the run is the one way an observer can break parity, and the test is what forbids it.
12. **`BatchOptimizer.ReadSortedRouteItems.next()` is transcribed as an O(n) rescan, twice per item, and is not memoised. (This corrects the survey's "recommendation" reading of quirk cand. K as a cleanup opportunity.)** `:573-654` rescans `board.itemList` for vias (`:577-604`) and then for traces (`:606-650`), sharing `currentMinCoor` so vias win ties — and it reads board state that the *previous* `optRouteItem` mutated. Any memoisation changes the sequence. The port keeps the rescan, keeps the `f64` lexicographic `(x, y, layer)` comparison exactly (`:587-596`, `:624-632`), and Task 13's `p7t8` proves the full sequence. *Cost if wrong:* the optimizer visits items in a different order and every optimized board differs — the single most order-fragile method in the plan.
13. **One audit map, extended, and the audit must reach zero for six more directories.** `scripts/audit-map/fr-router.map` gains rows for `autoroute/pipeline/*`, `board/optimize/ViaOptimizer.java` (**re-pointed from `lib.rs` in Task 6, not here**), `core/{RouterCounters,StopRequestState,StoppableThread,ProgressThrottler}.java` and `core/scoring/*`, with cross-crate rows for the `fr-board` and `fr-drc` additions in whichever form Task 1 of Plan 6 settled on. **What is in the tree:** `fr-router.map` has **no** cross-crate rows today and says why at `:45-49`; the live `../../` precedent is `fr-drc.map:45-46` and `fr-settings.map:40,42`. Follow that precedent — **do not re-litigate**. Task 17 runs `autoroute`, `autoroute/pipeline`, `board/optimize`, `core/scoring`, `board/facade` (the `RoutingBoard`/`RoutingBoardOperations` slices) and `board/trace` (the `PolylineTrace` slice) to **zero MISSING and zero UNMAPPED**. *Cost if wrong:* the audit is the only mechanical check that 7 500 lines arrived; weakening it is forbidden.
14. **Ported Java tests: four suites plus the fixture assertion family.** `autoroute/BoardHistoryTest.java` (143 loc → Task 2), `autoroute/StrictDrcEnforcementTest.java` (81 loc → Task 8), `fixtures/RoutingPipelineComparisonTest.java:49-76` (the only Java test that compares two runs of one board → Task 15, as the port's determinism test) and `BatchAutorouterDebugTest` (→ Task 9). `fixtures/RoutingFixtureTest`'s assertion family becomes the multi-pass harness in `crates/fr-router/tests/fixtures.rs`, extending Plan 6's single-pass one. **Quirk #157 still binds**: `TestingSettings.setMaxPasses` is first-writer-wins, so every Java fixture bound was measured at the test's own `maxPasses`, not the harness's 100. And `RoutingFixtureTest.java:71-76`'s `jobTimeoutString = "00:01:00"` default is a **wall clock the port's twin must not inherit** (ruling AI). *Cost if wrong:* the fixture bounds mean something other than what the plan claims.

**Open questions for the controller are at the foot of this document (§Controller questions).**

## File Structure

```
crates/fr-board/
  src/rules/via.rs                ViaRule owns its ViaInfos (Task 0, ruling AL)
  src/rules/mod.rs                replace_via_info_renumbering_rules loses its re-pointing note
  src/board/snapshot.rs           structural_hash widened; the audit table in the module doc (Task 3)
crates/fr-dsn/
  src/rules_reader.rs             apply_via_info stops renumbering rules (Task 0)
crates/fr-router/
  Cargo.toml                      fr-drc moves dev-deps -> deps (ruling 3)
  src/lib.rs                      pub use surface + the rewritten roster (Task 17)
  src/score/{mod,statistics,dtos,normalized}.rs   BoardStatistics' score subset (Task 1, ruling AG)
  src/pipeline/mod.rs             NamedAlgorithmType, TaskState, ProgressSink, RoutingEvent
  src/pipeline/stop.rs            RouterStop, StopRequestState, the deadline (Task 4, ruling AI)
  src/pipeline/counters.rs        RouterCounters (Task 4)
  src/pipeline/board_history.rs   BoardHistory + its entry (Task 2, ruling AF)
  src/pipeline/failure_log.rs     RoutingFailureLog (Task 9)
  src/pipeline/item_route_result.rs   ItemRouteResult (Task 9)
  src/pipeline/airline.rs         AutorouteAirlineCalculator.calculateAirline (Task 9)
  src/pipeline/batch_autorouter.rs    BatchAutorouter (Tasks 8, 9)
  src/pipeline/pass_runner.rs     AutoroutePassRunner.runSingleThread (Task 9)
  src/pipeline/batch_loop.rs      AutorouteBatchLoop.run (Task 10)
  src/pipeline/fanout.rs          BatchFanout (Tasks 11, 12)
  src/pipeline/optimizer.rs       BatchOptimizer (Tasks 13, 14)
  src/pipeline/unrouted_report.rs AutorouteUnroutedReport (Task 15)
  src/pipeline/run.rs             run_pipeline + PipelineResult (Task 15, ruling AK)
  src/board_ext/via_optimizer.rs  ViaOptimizer (Tasks 6, 7)
  src/board_ext/routing_board_ext.rs  + opt_changed_area, remove_items_and_pull_tight, fanout
  src/board_ext/tightener/mod.rs  + TraceTightener::opt_changed_area; the ConnectionToPin trio
  tests/*.rs                      ported Java tests + per-task unit tests + the acceptance harness
  README.md
scripts/audit-map/fr-router.map   + autoroute/pipeline, ViaOptimizer, core/*, core/scoring/*
scripts/gen-batch-reference.sh    HEAD-jar whole-board SES reference generator (ruling AM)
tests/reference/router-fixtures.txt          + the batch columns
tests/reference/<stem>/batch.ses             committed whole-board Java SES
tests/reference/<stem>/batch.meta.txt        jar identity, java -version, hash mode, command, flags
tests/reference/<stem>/batch.passes.jsonl    the per-pass tuple trace (ruling 1(a))
scripts/differential/java/P7T{1..10}.java
scripts/differential/java/probes/P7T{2,4,5,8,9}Probe.java
scripts/differential/rust/src/bin/p7t{1..10}.rs
scripts/differential/sweep-p7t9.sh
```

---

## Interfaces — the cross-task index

Every type below is **produced once** by the named task and **never re-declared**. A later task
extends an `impl` block; it does not restate a struct. The signatures here are the contract; each
task's own §Interfaces produced repeats it with the Java line ranges that justify each field.

> **Scan ruling 7 — the produce-once rule wins over the task split, and the *earliest* task declares
> the struct.** Three places had a later task declaring a struct an earlier task already writes
> methods on. The resolution: **`BatchFanout` is declared in Task 11** (which ports its ctor
> `:35-78`), **`BatchOptimizer` in Task 13** (which ports `optRouteItem`), and **`FanoutRunSummary`
> in Task 11** (which already declares it). Tasks 12 and 14 add `impl` blocks only. A task that finds
> itself writing `pub struct X` where an earlier task's §Interfaces produced already shows one has
> made an error — stop and report it, do not shadow.
>
> **Scan ruling 6 — two types moved earlier so nothing is consumed before it is produced.**
> `PassRecord` is now **Task 4's** (it is a plain DTO beside `RouterCounters`, and ruling 1(a) makes
> it Task **10**'s acceptance gate, five tasks before `run_pipeline`). `build_unrouted_report` stays
> Task 15's, and **Task 10 carries an explicit `obligation:` marker** for it rather than an
> unannounced forward reference.
>
> **The Task 2 / Task 3 ordering, stated once so it is not rediscovered.** Task 2 (`BoardHistory`)
> is built against the **narrow, Plan-2** `structural_hash`; Task 3 then **widens it**, which can
> change which two boards `BoardHistory::contains` calls equal. Task 3's steps therefore **re-run
> Task 2's `p7t2` transcript comparison** after the widening and the hash-equality *pattern* must
> still hold; if it does not, that is a Task 3 finding about the widening, not a Task 2 bug.
> `Board::hash_decision_equal` appeared in an earlier draft of this table and is **struck** — no task
> produces it and no task consumes it; `structural_hash` plus `diff_traces` are the whole surface.

| type / fn | produced by | consumed by |
|---|---|---|
| `ViaRule` (owning `ViaInfo`s) | Task 0 | Tasks 11, 12 (`RoutingBoard.fanout`'s combined rule) |
| `fr_router::score::{BoardStatistics, BoardStatisticsBoard, …Layers, …Items, …Components, …Pads, …Nets, …Connections, …Traces, …Bends, …Vias, …Fanout}`, `BoardStatistics::{normalized_score, is_pin_escaped}` | Task 1 | 2, 10, **11**, 12, 13, 14, 15, 16 |
| `BoardHistory`, `BoardHistoryEntry` | Task 2 | 10 |
| `Board::structural_hash` (widened) | Task 3 | 11, 12 — **and Task 2 only through the re-run in Task 3's steps, see the ordering note below** |
| `RouterStop`, `StopRequestState`, `RouterBudget`, `RouterCounters`, `PassRecord`, `TaskState`, `NamedAlgorithmType` | Task 4 | **5**, 8–16 |
| `ProgressSink`, `RoutingEvent`, `NoopProgressSink` | Task 4 | 9–16 |
| `RoutingBoardExt::opt_changed_area` | Task 5 | 6, 7, 8, 9, 11, 12 |
| `TraceTightener::opt_changed_area` | Task 5 | 6, 7 |
| `PolylineTraceExt::{check,correct,swap}_connection_to_pin` | Task 5 | (reached only through `pull_tight`) |
| `ViaOptimizer::opt_via_location` | Task 6 | 5 (discharges its stub), 7 |
| `BatchAutorouter::remove_tails` | Task 8 | 9, 10, 13 |
| `RoutingBoardExt::remove_items_and_pull_tight` | Task 8 | **nobody** — headless-dead, ported for the audit and for Plan 8's interactive surface (scan ruling 14; Task 8's own text governs) |
| `route_connection_full` (steps 1–8) | Task 8 | 9, **11** |
| `BatchAutorouter` (the struct and its accessors) | Task 8 | 9, 10, 13, 14 |
| `RoutingFailureLog`, `ItemRouteResult`, `calculate_airline`, `BatchAutorouter::autoroute_items` | Task 9 | 9 (pass runner), 13 |
| `AutoroutePassRunner::run_single_thread` | Task 9 | 10, 13 |
| `AutorouteBatchLoop::run` | Task 10 | 15 |
| `RoutingBoardExt::fanout`, **`BatchFanout` (the struct + its ctor)**, `FanoutComponent`, `FanoutPin`, `EscapeStatistics`, `FanoutPassStatus`, **`FanoutRunSummary`** | Task 11 | 10 (the type only, behind Task 10's stub), 12 |
| `BatchFanout::{fanout_board, fanout_pass}` (an `impl` extension — the struct is Task 11's, scan ruling 7) | Task 12 | 10 (discharges its stub), 15 |
| `ReadSortedRouteItems`, **`BatchOptimizer` (the struct + `new`)**, `BatchOptimizer::opt_route_item`, `BatchAutorouter::autoroute_passes_for_optimizing_item` | Task 13 | 14 |
| `BatchOptimizer::{run_batch_loop, opt_route_pass}` (an `impl` extension — the struct is Task 13's, scan ruling 7), `OptimizerResult` | Task 14 | 15 |
| `run_pipeline`, `PipelineResult`, `build_unrouted_report` (a free fn, not a type) | Task 15 | 10 (discharges Task 10's `obligation:` stub), 16, 17 |

```rust
// ── crates/fr-router/src/pipeline/stop.rs (Task 4, ruling AI) ───────────────────────────────────
/// Port of the three-state stop flag. The **enum is its own file**, `core/StopRequestState.java`
/// (NOT `StoppableThread.java:8`, which is the field — corrected by the pre-flight scan); the flag
/// and the four methods are `core/StoppableThread.java:8, 20-42`.
/// Two states are **not** interchangeable: `requestStop()` sets `ALL`, `requestStopAutoRouter()`
/// upgrades `NONE -> AUTO_ROUTER_ONLY` **only**, `isStopRequested()` is `== ALL` and
/// `isStopAutoRouterRequested()` is `!= NONE`. Quirk cand. C rides on the difference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum StopRequestState { None, AutoRouterOnly, All }          // core/StopRequestState.java

/// Ruling 1(a)'s per-pass tuple. Not a Java type — Java scatters these across FRLogger lines — so
/// it carries a `// renamed:` note and the five Java sites each field is read from. **Produced in
/// Task 4** (scan ruling 6) because Task 10's `BatchLoopResult` is where ruling 1(a)'s acceptance
/// ladder first bites, five tasks before `run_pipeline` exists.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PassRecord {
    pub pass: i32, pub score: f32, pub incomplete_count: usize,
    pub clearance_violations: usize, pub via_count: usize, pub trace_count: usize,
}

/// The port's single `Stoppable`, shared by the whole pipeline exactly as Java shares one
/// `StoppableThread`. It also carries ruling AI's deadline — the observable effect of Java's
/// per-job monitor thread (`RoutingJobSchedulerActionThread.java:55-90`), which the port has no
/// thread for.
pub struct RouterStop {
    state: std::cell::Cell<StopRequestState>,                    // :8
    deadline: Option<fr_board::TimeLimit>,                       // ruling AI
}
impl RouterStop {
    pub fn new() -> RouterStop;
    /// `job.timeoutAt` as a `TimeLimit`; `None` means "no job timeout", the CLI default.
    pub fn with_deadline(limit_ms: i32) -> RouterStop;
    pub fn request_stop(&self);                                  // :23-25  -> ALL
    pub fn request_stop_auto_router(&self);                      // :33-37  NONE -> AUTO_ROUTER_ONLY
    /// **The two queries are adjacent and easy to swap — an earlier draft of this plan had them
    /// the wrong way round.** `isStopRequested()` is `:28-30`; `isStopAutoRouterRequested()` is
    /// `:40-42`. Re-read both before transcribing.
    pub fn is_stop_requested(&self) -> bool;                     // :28-30  == ALL
    pub fn is_stop_auto_router_requested(&self) -> bool;         // :40-42  != NONE
    pub fn state(&self) -> StopRequestState;
    /// Ruling AI's deadline poll. Called at exactly the six sites Java reads `job.state` or the
    /// monitor thread would have flipped the flag: `AutorouteBatchLoop:251`, `BatchFanout:111`
    /// and `:396`, `BatchOptimizer:172` and `:308`, and the top of `AutoroutePassRunner`'s item
    /// loop. On expiry it performs Java's *observable* action — `request_stop()` (`ALL`), because
    /// the monitor calls `requestStop()` 30 s before it ever writes `TIMED_OUT`
    /// (`RoutingJobSchedulerActionThread.java:75, 84`; quirk cand. R).
    pub fn poll_deadline(&self) -> bool;
    /// Java's `job.state == TIMED_OUT`, which is what `AutorouteBatchLoop:251-253` reads.
    pub fn is_timed_out(&self) -> bool;
}
// The two `StopCheck`s are built at the call site, because `StopCheck<'a> = &'a dyn Fn() -> bool`
// cannot be returned from a method:
//     let auto: StopCheck<'_> = &|| stop.is_stop_auto_router_requested();
//     let all:  StopCheck<'_> = &|| stop.is_stop_requested();

/// Ruling AI's budget knob. Java's three wall-clock constants become explicit parameters so a
/// parity run can disable them on both sides; the defaults are Java's literals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RouterBudget {
    /// `TIME_LIMIT_TO_PREVENT_ENDLESS_LOOP` (AutorouteConnectionRouter.java:22,
    /// BatchAutorouter.java:43, RoutingBoard.java:961). Default **1000**; `0` disables the limit
    /// (`TraceTightener`'s ctor only builds a `TimeLimit` when `timeLimit > 0`, :73-77).
    pub opt_changed_area_ms: i32,
    /// `settings.fanout.maxMillisecondsPerPin` (BatchFanout.java:175-178). Default **10000**.
    pub fanout_ms_per_pin: i32,
    /// `BatchAutorouter.shouldFireBoardUpdate`'s **250 ms** gate (BatchAutorouter.java:335-343).
    /// Progress only (ruling 11), but a knob so a driver can pin it. Default **250**.
    pub board_update_throttle_ms: i32,
    /// `core.ProgressThrottler`'s interval. **Java has no constant here** — the interval is a
    /// constructor argument and every construction site supplies it; read the sites and use the
    /// value they pass as the default. Progress only. *(Scan ruling 10 split this from the field
    /// above, which had conflated two different Java literals into one knob defaulting to 1000, so
    /// the 250 ms gate would have silently become a 1000 ms gate.)*
    pub progress_throttle_ms: i32,
}
impl Default for RouterBudget { /* 1000, 10000, 250, <ProgressThrottler's ctor argument> */ }
impl RouterBudget { /// Every wall clock off — what every `p7t*` parity run uses, both sides.
                    pub fn disabled() -> RouterBudget; }

// ── crates/fr-router/src/pipeline/mod.rs (Task 4, ruling AK / spec §10) ─────────────────────────
/// Port of `autoroute/pipeline/NamedAlgorithmType.java` (7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum NamedAlgorithmType { Router, Optimizer }
/// Port of `autoroute/pipeline/TaskState.java` (11) — transcribe the constant list verbatim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum TaskState { /* read the 11-line file */ }

/// Spec §10's `ProgressSink`, which replaces `NamedAlgorithm`'s three listener lists
/// (NamedAlgorithm.java:30-96), `autoroute/events/**` (6 files) and `core/ProgressThrottler`.
/// **No port decision reads it** (ruling 11) — the parity test that pins that is
/// `a_recording_sink_changes_no_board_byte`.
pub trait ProgressSink {
    fn on_event(&mut self, event: &RoutingEvent) {}
}
/// The default: Java with every listener list empty, which is the headless CLI's state.
pub struct NoopProgressSink;
impl ProgressSink for NoopProgressSink {}

#[derive(Debug, Clone, PartialEq)]
pub enum RoutingEvent {
    TaskStateChanged { algorithm: NamedAlgorithmType, state: TaskState },   // NamedAlgorithm:121-125 (fireTaskStateChangedEvent)
    BoardUpdated { counters: RouterCounters },                              // BatchAutorouter:335-343
    BoardSnapshot { pass: i32 },                                            // AutorouteBatchLoop:606-608
    FanoutProgress { pass: i32, routed: i32, pins_to_go: i32 },             // BatchFanout:544-576
    OptimizerImproved { item: ItemId, score_before: f32, score_after: f32 },// BatchOptimizer:333-348
}

// ── crates/fr-router/src/pipeline/run.rs (Task 15, ruling AK) ───────────────────────────────────
/// The minimal `RoutingPipeline.run` (autoroute/pipeline/RoutingPipeline.java:81-129) that
/// Plan 8's `fr-core` wraps. It owns the two stages, the fanout-only `maxPasses = 0` mode
/// (`:99-107`) and the **only** call to `finishAutoroute` (`:110`).
pub fn run_pipeline(
    board: &mut Board,
    settings: &RouterSettings,
    stop: &RouterStop,
    budget: RouterBudget,
    progress: &mut dyn ProgressSink,
) -> Result<PipelineResult, RouterError>;

/// What Plan 8's `RoutingResult` is built from. `board` is left in `board`; this is the report.
#[derive(Debug, Clone, PartialEq)]
pub struct PipelineResult {
    pub router_state: TaskState,           // AutorouteBatchLoop.java:571-585 (quirk cand. A)
    pub optimizer_state: Option<TaskState>,// None when runOptimizer is false (RoutingPipeline:36)
    pub passes_run: i32,                   // job.setCurrentPass's last value (:275-277)
    pub fanout: Option<FanoutRunSummary>,  // BatchFanout.FanoutRunSummary (:625-629)
    pub per_pass: Vec<PassRecord>,         // ruling 1(a) — the diagnostic ladder
    pub final_statistics: BoardStatistics, // fr_router::score
    pub timed_out: bool,                   // ruling AI
}
```

---

### Task 0: `fr-board` — `ViaRule` owns its `ViaInfo`s (controller ruling AL closes ruling H)

**Files:** `crates/fr-board/src/rules/{via.rs,mod.rs}`, `crates/fr-dsn/src/rules_reader.rs`; **`crates/fr-dsn/tests/rules_round_trip.rs`** (the existing `re_declared_via_info_re_points_the_existing_via_rule_unlike_java` test **inverts** — rename it and say so); **`crates/fr-router/src/autoroute/maze/control.rs`** (the `rebuild_via_info` obligation block at **`:385-406`**). *(Both paths were wrong in the plan's first draft — the round-trip test is in `fr-dsn`, not `fr-board`, and the obligation block is in `src/…/control.rs`, not `tests/control.rs`; `tests/control.rs:381-400` is an unrelated net-number test.)*
**Java:** `rules/ViaRule.java (130)` — class `:15-130`, `list` `:21`, `appendVia` `:29-31`, `removeVia` `:34-36`, `viaCount` `:39-41`, `getVia` `:44-47`, `contains` `:55-62`, `containsPadstack` `:64-72`, `getLayerRange` `:78-86`, **`swap(ViaInfo, ViaInfo)` `:92-105`** (there is **no** `swapVias` — the plan's first draft invented the name and the range `:64-106`); `rules/ViaInfo.java (108)`; `rules/ViaInfos.java (88)` — `add`, `remove`, `get(name)`, `count`; `io/specctra/RulesReader.java:340-350` (`applyViaInfo`, 11). **≈ 337 Java lines re-read; ≈ 150 of port changed.**

**Interfaces consumed:** none (this is the first task).
**Interfaces produced:**
```rust
// crates/fr-board/src/rules/via.rs
/// Port of `rules.ViaRule` (ViaRule.java:15-130). **Owns** its `ViaInfo`s.
///
/// Java's field is `List<ViaInfo> list` (:21) — object *references*. `RulesReader.applyViaInfo`
/// (io/specctra/RulesReader.java:340-350) replaces a `ViaInfo` in `BoardRules.viaInfos` by name,
/// and every `ViaRule` that already held the old object keeps the **detached original**. An
/// index model cannot express that, which is what made ruling H's divergence unavoidable —
/// measured at HEAD by `P6T8Probe viadiv` (attach=true on the replacement vs attach=false in the
/// rule) and closed **against** re-pointing by Plan 6 Task 17 (123 vias vs 45 on
/// `Issue593-BBD_Mars-64.dsn`). Owned copies are the port of Java's aliasing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViaRule {
    pub name: String,          // :20
    vias: Vec<ViaInfo>,        // :21 — owned copies, not ViaInfoId indices
}
impl ViaRule {
    pub fn new(name: impl Into<String>) -> ViaRule;                    // :24-26
    pub fn append_via(&mut self, via: ViaInfo);                        // :29-31
    /// `removeVia` (:34-36) — Java's `List.remove(Object)` removes the **first** element equal to
    /// the argument. Java compares by reference; the port compares by value, which agrees
    /// wherever a rule cannot hold two equal `ViaInfo`s (asserted by a test).
    pub fn remove_via(&mut self, via: &ViaInfo) -> bool;               // :34-36
    pub fn via_count(&self) -> usize;                                  // :39-41
    /// `getVia(int)` (:44-47) returns **null** for an out-of-range index, so the port answers
    /// `Option` — the Global Constraint "`Result`/`Option` where Java … returns `null`", which the
    /// plan's first draft dropped here.
    pub fn get_via(&self, index: usize) -> Option<&ViaInfo>;            // :44-47
    pub fn contains(&self, via_info: &ViaInfo) -> bool;                 // :55-62
    pub fn iter(&self) -> std::slice::Iter<'_, ViaInfo>;
}

// crates/fr-board/src/rules/mod.rs — the renumbering machinery disappears
impl BoardRules {
    /// `RulesReader.applyViaInfo`'s replacement (io/specctra/RulesReader.java:340-350):
    /// `viaInfos.remove(old); viaInfos.add(new)`. **No rule is rewritten** — Java rewrites none
    /// either, which is the whole point. Replaces
    /// `replace_via_info_renumbering_rules` (Plan 3 Task 14), whose renumbering half was only
    /// ever needed by the index model.
    pub fn replace_via_info(&mut self, old_id: ViaInfoId, new_info: ViaInfo) -> ViaInfoId;
}
```

**Transcription notes.**
- The `ViaInfos` collection keeps its `ViaInfoId` index model; only `ViaRule` changes. `Network.addViaRule` (`fr-board/src/rules/via.rs:319`) and the DSN/`.rules` writers emit the via **name** (`Network.java:58-76, 78-91`), so no output byte moves. Confirm that with the `p3t15` sweep, do not assume it.
- Both `obligation:` markers at `crates/fr-board/src/rules/via.rs:260-265` are **deleted** (the renumbering one) and **closed** (the re-pointing one, with the Task 17 evidence pasted into the register row).
- The `rebuild_via_info` obligation block at `crates/fr-router/src/autoroute/maze/control.rs:381-400` is rewritten to record the closure, and `AutorouteControl::rebuild_via_info` reads `via_rule.get_via(i)` — now a `&ViaInfo` — with no change to its own logic.
- The alternative ruling AL allows (tombstoned `ViaInfos`, i.e. `remove` leaves a hole and the id stays valid) is **not** chosen here, and the plan says why: `ViaInfos.get(name)` must then skip tombstones, `count()` becomes ambiguous, and every `ViaInfoId` consumer inherits a "may be dead" case. Owned copies confine the change to one struct. **If the implementer finds a consumer that needs the shared identity, that is a Java-wins report, not a silent switch.**

**Tests.**
- `crates/fr-board/tests/rules_round_trip.rs`: rename `re_declared_via_info_re_points_the_existing_via_rule_unlike_java` → `re_declared_via_info_leaves_the_rule_on_the_detached_original_like_java`, invert its assertion, and keep the original name in a doc comment so the trail is one grep.
- `a_via_rule_holds_its_own_copy`: append a `ViaInfo`, mutate the one in `via_infos`, assert the rule's copy is unchanged.
- `remove_via_removes_the_first_equal_element` and `a_rule_cannot_hold_two_equal_via_infos` (the guard on the `remove_via` deviation).
- `every_dsn_and_rules_byte_is_unchanged`: round-trip `Issue593-BBD_Mars-64.dsn` and `crates/fr-router/tests/data/ruling-h-redeclare.rules` through the writers, compare against the committed goldens.

**JVM-pinned evidence (required).**
- `scripts/differential/run.sh p6t1 ../freerouting/fixtures/Issue593-BBD_Mars-64.dsn 50 1 crates/fr-router/tests/data/ruling-h-redeclare.rules` — **MATCH at k=6 and k=8**, the two connections where it currently diverges (`scripts/differential/README.md`'s `p6t1` row: "The `.rules` run above still diverges from connection 8"). Commit the transcript as `crates/fr-router/tests/data/p7t0-ruling-h-match.txt`.
- `scripts/differential/java/probes/P6T8Probe.java viadiv` regenerated; `crates/fr-router/tests/data/p6t8-ruling-h-viadiv.txt` **unchanged** (the Java side did not move) and the port's twin now agreeing with it.
- `scripts/differential/sweep-p3t15.sh` — **530 pairs, 525 MATCH + 5 XDIFF, unchanged**, pasted into the commit message.
- `run.sh p6t1` on all five reference stems still MATCH; `cargo test -p fr-router --test reference_parity` green.

**Steps:** invert the round-trip test → change `ViaRule` → change `apply_via_info` → delete the renumbering method → run `p6t1` with the `.rules` → 0 diffs at k=6/k=8 → `p3t15` sweep → close both register rows → fmt/clippy/test/audit → commit `fix(board): ViaRule owns its ViaInfos, closing ruling H against re-pointing`.

---

### Task 1: `fr_router::score` — `BoardStatistics`' score-relevant subset (ruling AG)

**Files:** `crates/fr-router/src/score/{mod,statistics,dtos,normalized}.rs`, `crates/fr-router/Cargo.toml` (`fr-drc` → `[dependencies]`, ruling 3), `crates/fr-router/src/lib.rs` (the `pub use score::…` surface); `crates/fr-router/tests/score.rs`; `scripts/differential/java/P7T7.java`, `scripts/differential/rust/src/bin/p7t7.rs`, `scripts/differential/{run.sh,README.md}`.
**Java:** `core/scoring/BoardStatistics.java` — the field block `:37-79`, the three delegating ctors `:81-108` (specifically `:84-86`, `:100-102`), **the computing ctor `:110-427` (318)**, `isPinEscaped` `:555-576` (22), `calculateScore` `:597-616` (20), `getMaximumScore` `:619-621` (3), `getNormalizedScore` `:624-635` (12), the nested `BoardStatisticsFanout` `:638-647` (10); **the file is 648 lines**; the DTOs `core/scoring/BoardStatistics{Board (15), Layers (14), Items (32), Components (11), Pads (11), Nets (14), Connections (14), Traces (49), Bends (20), Vias (24)}.java`. **≈ 520 ported lines.**
**Rostered here, not ported:** `BoardStatistics(byte[], FileFormat)` `:436-554` (119 — the SES/DSN text-scraping twin), `countOccurrences` `:578-586`, `toString` `:589-591`, `core/scoring/BoardScoreBreakdown.java` (192) and `ScoringWeightComparison.java` (232) — all `// added in Plan 8:` (ruling 4), each with the reader that makes it Plan 8's (the result manifest, the CLI's `--score` output).

**Interfaces consumed:** `fr_drc::DesignRulesChecker::{new, calculate_all_incompletes, max_connections, get_incomplete_count, get_all_clearance_violations}` (Plan 5); `fr_settings::ScoringSettings` (Plan 4); `Board::{get_traces, get_vias, get_pins, get_smd_pins, get_items, clearance_value, communication}` (Plan 2).
**Interfaces produced:**
```rust
// crates/fr-router/src/score/statistics.rs
/// Port of `core.scoring.BoardStatistics` (BoardStatistics.java:35-647) — **the score-relevant
/// subset only** (controller ruling AG). Plan 8's `fr-core` re-exports this type and adds the
/// Gson-compatible JSON surface and the `byte[]`/`FileFormat` constructor.
#[derive(Debug, Clone, PartialEq)]
pub struct BoardStatistics {
    pub host: String, pub unit: String,                       // :37-41
    pub board: BoardStatisticsBoard, pub layers: BoardStatisticsLayers,
    pub items: BoardStatisticsItems, pub components: BoardStatisticsComponents,
    pub pads: BoardStatisticsPads, pub nets: BoardStatisticsNets,
    pub connections: BoardStatisticsConnections, pub traces: BoardStatisticsTraces,
    pub bends: BoardStatisticsBends, pub vias: BoardStatisticsVias,
    /// Re-used from `fr-drc`, never redeclared (Plan 5 delivered it).
    pub clearance_violations: fr_drc::BoardStatisticsClearanceViolations,
    pub fanout: BoardStatisticsFanout,                        // :638-647
}
impl BoardStatistics {
    /// `BoardStatistics(BasicBoard)` (:84-86) — `unit = board.communication.unit`,
    /// `includeClearanceViolations = true`, `includeConnections = true`.
    pub fn new(board: &mut Board) -> BoardStatistics;
    /// `BoardStatistics(BasicBoard, Unit, boolean)` (:100-102).
    pub fn with_options(board: &mut Board, unit: Option<Unit>,
                        include_clearance_violations: bool) -> BoardStatistics;
    /// `BoardStatistics(BasicBoard, Unit, boolean, boolean)` (:110-427) — the computing ctor.
    pub fn compute(board: &mut Board, unit: Option<Unit>,
                   include_clearance_violations: bool, include_connections: bool)
        -> BoardStatistics;
    /// `calculateScore(ScoringSettings)` (:597-616). `f32`, and every intermediate is Java's
    /// `float`/`double` in Java's order — see the transcription notes.
    pub fn calculate_score(&self, scoring: &ScoringSettings) -> f32;
    /// `getMaximumScore(ScoringSettings)` (:619-621) —
    /// `connections.maximumCount * unroutedNetPenalty`. **A multiplication — it does not divide.**
    pub fn maximum_score(&self, scoring: &ScoringSettings) -> f32;
    /// `getNormalizedScore(ScoringSettings)` (:624-635) —
    /// `max(0, calculateScore / getMaximumScore) * 1000`. **The whole plan's loop condition.**
    pub fn normalized_score(&self, scoring: &ScoringSettings) -> f32;
    /// `isPinEscaped(Pin)` (:555-576) — `BoardStatisticsFanout.escapedCount`'s predicate and
    /// `EscapeStatistics`' (Task 12).
    pub fn is_pin_escaped(board: &Board, pin: ItemId) -> bool;
}
```

**Transcription notes, each with its Java line.**
- **`float` vs `f64` is load-bearing.** `calculateScore` (`:597-616`) and every DTO total are Java `float`; the intermediates inside the computing ctor are `double` and narrowed by `(float)` casts. **Find every cast yourself** (`grep -n '(float)' core/scoring/BoardStatistics.java`) — the plan's first draft named `:189-192`, `:255-256` and `:339`, of which only `:189` and `:261` carry one and `:339` is the `DesignRulesChecker` construction. Transcribe the type at each site — an `f64` that is never narrowed changes the score in the last bits, and `AutorouteBatchLoop:425`'s `> lastBestScore + 0.5` and `BatchOptimizer:220-230`'s `< improvementThreshold` are both threshold comparisons on it.
- **The unit normalisation is a HEAD-only correction with a comment explaining itself** (`:190-196`): `boardUnitToMmFactor = Unit.scale(1.0, board.communication.unit, MM) / (resolution > 0 ? resolution : 1)`, and the same for µm. Port it verbatim including the `resolution > 0` guard; `traces.totalLengthMm` is what `calculateScore` multiplies by `defaultPreferredDirectionTraceCost`.
- **`traces.totalWeightedLength`** (`:250-262`): `length * (halfWidth + board.clearanceValue(trace.clearanceClassIndex(), defaultClearanceClass, layer))`, **halved when `fixedState == SHOVE_FIXED`** (`:257-260`). `BatchOptimizer.optRoutePass:288` uses it as `minCumulativeTraceLength`, so this is a routing decision, not a report number.
- **Two `DesignRulesChecker` constructions per call** (`:265-268` behind `includeConnections`, `:338-341` behind `includeClearanceViolations`), each with `null` DRC settings, each doing its own full calculation. Do not share one — the two have different internal state after `calculateAllIncompletes`, and the call count is observable through Plan 5's memo behaviour.
- **The fanout block** (`:407-426`): walks `board.getSmdPins()`, counts `total` (netCount > 0), `alreadyConnected` (`pin.getUnconnectedSet(netNumber).isEmpty()`) and `escaped` (`isPinEscaped`), then sets `pinsToEscape = total - alreadyConnected`. Note it reads **net index 0 only** (`:415`) for a multi-net SMD pin — `// Java bug:` and a quirk row.
- `host` falls back to `"Freerouting," + FREEROUTING_VERSION` when the DSN carried none (`:118-120`) and runs through `TextManager.unescapeUnicode` (`:121`) — a `// not ported:` for the GUI text manager with the unescape transcribed inline (it is a `\\uXXXX` decoder; read it, do not guess).
- **The NaN, stated correctly.** `getMaximumScore` (`:619-621`) is a *multiplication* and returns `0` for a board with no connections; the **division is in `getNormalizedScore`** (`:624-635`), whose `calculateScore / getMaximumScore` is then `0/0 = NaN`, and `Math.max(0, NaN)` is `NaN`, not `0`. Reproduce with `java_max`. Quirk row — `empty_board.dsn` reaches it. *(The plan's first draft said "`getMaximumScore` divides by zero", which is wrong about which method divides; the quirk row below is corrected to match.)*

**Tests (`crates/fr-router/tests/score.rs`):** `normalized_score_matches_the_jvm_on_every_corpus_stem` (a table from `p7t7`); `a_shove_fixed_trace_is_weighted_at_half`; `the_unit_normalisation_uses_the_resolution_guard`; `an_empty_board_scores_nan_not_zero` (the quirk); `a_multi_net_smd_pin_is_counted_on_net_index_zero_only`; `float_narrowing_matches_java` (three values where `f32` and `f64` disagree in the last bit).

**JVM-pinned evidence (required).** **`p7t7`.** `scripts/differential/java/P7T7.java` declares `package app.freerouting.core.scoring;`, is run in `run.sh`'s `needs_jar=1` mode against the HEAD jar (JDK 25, `-XX:hashCode=2`, `-Djava.awt.headless=true -Duser.language=en -Duser.country=US`, `RouterBudget::disabled()`'s equivalent is irrelevant here — nothing in this task reads a clock), and prints, per fixture: every DTO field with `Double.toString`/`Integer.toString`, then `calculateScore`, `getMaximumScore` and `getNormalizedScore` for **four `ScoringSettings` presets** (the default, and one each with `unroutedNetPenalty`, `viaCosts` and `bendPenalty` moved off default). `p7t7.rs` prints the same. Acceptance: **0 diffs on all six corpus stems, plus one SES-reloaded routed board** (so the trace/via/bend blocks are exercised on a board that is actually routed, not just read).

**Steps:** `P7T7.java` → `p7t7.rs` stub → tests from the Java output → move `fr-drc` to `[dependencies]` → implement → 0 diffs → fmt/clippy/test/audit → commit `feat(router): fr_router::score — BoardStatistics' score subset and getNormalizedScore`.

---

### Task 2: `BoardHistory` and the clone-based board snapshot (ruling AF)

**Files:** `crates/fr-router/src/pipeline/{mod,board_history}.rs`, `crates/fr-router/src/lib.rs` (delete the eleven `// not ported: BoardHistory.*` markers, which ruling AF supersedes); `crates/fr-router/tests/board_history.rs`.
**Java:** `autoroute/BoardHistory.java (202)` — `MAX_HISTORY_SIZE` `:29` (`= 30`), fields `:31-34`, both ctors `:37-39` and `:42-45`, **`add` `:48-80`**, `clear` `:83-85`, `contains` `:88-101`, `remove` `:104-112`, `getMaxScore` `:115-128`, **`restoreBoard` `:134-155`**, `restoreBestBoard` `:158-160`, `size` `:163-170`, `getRank` `:173-186`, the **private nested** `BoardHistoryEntry` `:188-201` with its fields at `:190-193`; *(the plan's first draft was off by one on seven of these — re-derive rather than trusting either)* `autoroute/BoardHistoryEntry.java (33)` — the **top-level, shadowed, dead** twin. **≈ 235 Java lines.**

**Interfaces consumed:** `BoardStatistics::normalized_score` (Task 1); `Board::{structural_hash, deep_copy}` (Plan 2).
**Interfaces produced:**
```rust
// crates/fr-router/src/pipeline/board_history.rs
/// Port of `autoroute.BoardHistory` (BoardHistory.java:22-201): the ranked, capped cache of board
/// snapshots that `AutorouteBatchLoop`'s best-board policy restores from.
///
/// **Controller ruling AF** ports it; plan-6 ruling 13 had rostered it `// not ported:` on spec
/// §2's "undo store" exclusion, which it is not — it is the pass loop's best-board memory, and
/// without it the board written to SES is the *last* pass's rather than the *best* pass's.
///
/// Java's `synchronized` methods and `ReentrantReadWriteLock` (:32-34) are `// not ported:`: the
/// whole pipeline is single-threaded (survey §3.4).
pub struct BoardHistory {
    boards: Vec<BoardHistoryEntry>,   // :32 — a plain Vec; the sort at :143 mutates the order
    max_history_size: usize,          // :31, default Self::MAX_HISTORY_SIZE
    scoring: ScoringSettings,         // :33
}
impl BoardHistory {
    /// `MAX_HISTORY_SIZE` (:29) `= 30`. **An associated `const`, not only a field default** —
    /// Task 10's `BOARD_RANK_LIMIT` is written `BoardHistory::MAX_HISTORY_SIZE`, mirroring Java's
    /// `BatchAutorouter.java:40 static final int BOARD_RANK_LIMIT = BoardHistory.MAX_HISTORY_SIZE;`
    /// (scan ruling: the first draft declared only the field, so Task 10 referenced a const that
    /// did not exist).
    pub const MAX_HISTORY_SIZE: usize = 30;
}
/// Port of the **private nested** `BoardHistory.BoardHistoryEntry` (:188-201) — the one that is
/// actually used. `autoroute/BoardHistoryEntry.java`'s public top-level class of the same name is
/// **shadowed and dead** (quirk row).
pub struct BoardHistoryEntry {
    /// Java's `byte[] board = board.serialize(false)` (:190). Ruling 8: a `Board` clone.
    pub board: Board,
    pub hash: u64,                    // :191  board.getHash()  -> Board::structural_hash
    pub score: f32,                   // :192  new BoardStatistics(board).getNormalizedScore(...)
    pub restore_count: i32,           // :193
}
impl BoardHistory {
    pub fn new(scoring: &ScoringSettings) -> BoardHistory;                    // :37-39
    pub fn with_capacity(scoring: &ScoringSettings, max: usize) -> BoardHistory; // :42-45
    pub fn add(&mut self, board: &mut Board);                                 // :48-80
    pub fn clear(&mut self);                                                  // :83-85
    pub fn contains(&self, board: &Board) -> bool;                            // :88-101
    pub fn remove(&mut self, board: &Board);                                  // :104-112
    pub fn max_score(&self) -> f32;                                           // :115-128
    /// `restoreBoard(int)` (:134-155). **Sorts `boards` descending by score in place**, then
    /// answers the first entry whose `restoreCount <= maxAllowedRestoreCount`, **incrementing
    /// that entry's `restoreCount`**. `<= 0` means `Integer.MAX_VALUE` (:135-137).
    pub fn restore_board(&mut self, max_allowed_restore_count: i32) -> Option<Board>;
    pub fn restore_best_board(&mut self) -> Option<Board>;                     // :158-160
    pub fn size(&self) -> usize;                                              // :163-170
    /// `getRank(RoutingBoard)` (:173-186) — the **1-indexed position in the current list order**,
    /// or `-1`. Order-dependent on `restore_board`'s side-effecting sort (quirk row).
    pub fn rank(&self, board: &Board) -> i32;
}
```

**Transcription notes.**
- **`add` is two full score computations when the cap is reached and one when it is not** (`:48-80`): `contains` (hash) first, then — only at capacity — `new BoardStatistics(board).getNormalizedScore(...)` for the gate (`:56`), a linear scan for the worst entry (`:62-68`), an early return when `newScore <= worstScore` (`:72-75`), and then `new BoardHistoryEntry(...)` which computes the score **again** (`:192`). Under capacity there is **no score gate at all** — any distinct board enters. Reproduce both, including the double computation.
- **`getMaxScore` starts at `0`, not `-inf`** (**`:118`** — `:116` is the `readLock().lock()`), so an empty history answers `0`. `AutorouteBatchLoop:306` compares `bh.getMaxScore() > boardScoreAfter` with a strict `>`, so an empty history never triggers a restore — which is why the loop's `bh.size() >= STOP_AT_PASS_MINIMUM` guard at `:298` is belt-and-braces. Record it; do not "improve" it to `f32::NEG_INFINITY`.
- **`restoreBoard` mutates under a *read* lock** (`:139-141` takes `readLock`, `:143` sorts the list, **`:147`** increments `restoreCount`). Single-threaded, so unobservable — but it is the reason `getRank`'s answer depends on how many restores have happened, and `AutorouteBatchLoop:315-320` breaks the whole pass loop on `getRank(...) > BOARD_RANK_LIMIT`. Quirk row, and a test that pins the ordering dependence.
- `restoreBoard`'s sort is `(o1, o2) -> Float.compare(o2.score, o1.score)` — **descending, `Float.compare` semantics** (NaN sorts last, `-0.0 < 0.0`), and `List.sort` is a **stable** TimSort. The port uses `sort_by` (stable) with a `Float.compare` transcription, **not** `partial_cmp` and **not** `total_cmp` (they disagree on `-0.0`/`NaN` exactly where `Float.compare` is specified).
- **`autoroute/BoardHistoryEntry.java` (33 lines) is dead**: `BoardHistory` declares its own `private static class BoardHistoryEntry` at `:188`, which shadows the import. The top-level class holds a live `RoutingBoard`, a `BoardStatistics` and an `Instant.now()` and implements `Comparable` — none of which any caller reaches. Roster it `// not ported:` with that evidence and give it a quirk row.
- Ruling 8's memory note goes in this task's README paragraph: 30 live `Board` clones vs 30 `byte[]`s. Measure the peak RSS on `Issue730-DAC2020_bm11.dsn` and record the number.

**Tests (`crates/fr-router/tests/board_history.rs`)** — this file **is** the port of `src/test/java/app/freerouting/autoroute/BoardHistoryTest.java` (143 loc, ruling 14), method for method with its Java line in each doc comment, plus:
- `under_capacity_any_distinct_board_enters` and `at_capacity_a_worse_board_is_rejected` — asserting `size()` is unchanged **and** that the rejected board is absent by `contains`. *(Renamed from `…_without_serialising`: ruling 8 replaced the serialisation with a clone, so the original name asserted a mechanism the port does not have.)*
- `an_identical_board_is_rejected_by_hash`.
- `max_score_of_an_empty_history_is_zero_not_negative_infinity`.
- `restore_board_increments_the_restore_count_and_reorders_the_list`.
- `get_rank_depends_on_the_last_restore_sort` (the quirk's pin).
- `restore_board_zero_means_unlimited` (`:135-137`).
- `the_top_level_board_history_entry_class_is_unreachable` — **a roster-assertion test, and deliberately so**: it greps `crates/fr-router/src/lib.rs` for the `// not ported: BoardHistoryEntry` line, exactly as Plan 5 did. It asserts the roster, not behaviour; that is its whole job, and the doc comment says so, so a reviewer does not score it as an empty test.

**JVM-pinned evidence (required).** `scripts/differential/java/probes/P7T2Probe.java`, `package app.freerouting.autoroute;` (to reach the private nested entry by reflection), HEAD jar, JDK 25, `-XX:hashCode=2`, headless flags, `RouterBudget::disabled()` irrelevant (no clock). It drives a scripted sequence of 40 `add`/`contains`/`remove`/`restoreBoard`/`getRank`/`getMaxScore` calls over boards derived from `Issue143-rpi_splitter.dsn` by inserting and removing traces, and prints after each call: `size`, every entry's `(hash-prefix, score, restoreCount)` in list order, and the call's return value. Stdout committed as `crates/fr-router/tests/data/p7t2-board-history.txt`; the test asserts against it **byte-for-byte** (hashes compared as an *equality pattern* across entries, not as values — the two hash functions differ by construction, ruling AH).

**Steps:** `P7T2Probe.java` → transcript → tests → implement → transcript matches → fmt/clippy/test/audit → commit `feat(router): BoardHistory, the pass loop's best-board memory (ruling AF)`.

---

### Task 3: the `structural_hash` audit, and decision parity with Java's `getHash()` (ruling AH)

**Files:** `crates/fr-board/src/board/snapshot.rs` (the audit table in the module doc, the widened hash), `crates/fr-board/tests/snapshot.rs`; `scripts/differential/java/P7T10.java`, `scripts/differential/rust/src/bin/p7t10.rs`, `scripts/differential/{run.sh,README.md}`.
**Java:** `board/facade/BoardSnapshotManager.java` (101 lines) — `serialize(boolean)` `:26-43` (the three writes at `:29-35`), `getHash` `:58-72` (the `null` return at `:70`), `diffTraces` `:86-100`; `board/facade/BasicBoard.java:153-172` (the four delegating methods, the javadoc at `:163`, `getHash` `:164-166`, `diffTraces` `:169-171`); the three **decision** sites: `autoroute/pipeline/BatchFanout.java:152-156`, `autoroute/BoardHistory.java:87-100` (`contains`) and `:173-186` (`getRank`). **≈ 120 Java lines.**

**Interfaces consumed:** `BoardHistory` (Task 2).
**Interfaces produced:**
```rust
// crates/fr-board/src/board/snapshot.rs
impl Board {
    /// The port's stand-in for `BasicBoard.getHash()` (BasicBoard.java:164-166 →
    /// BoardSnapshotManager.java:58-72), which is an **MD5 hex string over `serialize(true)`**.
    ///
    /// `serialize(true)` writes `board.getTraces()`, `board.getVias()` **and `board.itemList`**
    /// (BoardSnapshotManager.java:29-35) through Java object serialization — i.e. the whole item
    /// graph, not just the traces the method's own comment claims. Controller ruling AH:
    /// **do not** reproduce the bytes or the digest; widen this hash until it covers exactly the
    /// field set that serialization covers, and prove *decision* parity at the three sites where
    /// Java compares two hashes. The field-by-field audit is the table in this module's docs.
    pub fn structural_hash(&self) -> u64;
    /// `BasicBoard.diffTraces` (BasicBoard.java:169-171 → BoardSnapshotManager.java:86-100) —
    /// the symmetric difference of the two boards' trace id sets. Ruling AH's tie-break for a
    /// port-only hash collision.
    ///
    /// **This already exists** at `crates/fr-board/src/board/snapshot.rs:245` (Plan 2). It is
    /// restated here only because ruling AH makes it load-bearing; **audit it against `:86-100`
    /// and change it only if it disagrees.** If it is unchanged, this task makes **one** `fr-board`
    /// API change (the widened hash), not two — say which in the commit message.
    pub fn diff_traces(&self, compare_to: &Board) -> usize;
}
```

**The audit table (the task's deliverable — one row per field Java's `serialize(true)` reaches).** Written into `snapshot.rs`'s module doc and reproduced in `crates/fr-router/README.md`:

| Java serialized field | reached via | covered by `structural_hash` today | action |
|---|---|---|---|
| `PolylineTrace.polyline` (every `Line`'s `a`,`b`,`c`) | `getTraces()` | ? | — |
| `PolylineTrace.halfWidth`, `.layer` | `getTraces()` | ? | — |
| `Item.netNos`, `.clearanceClassNo`, `.fixedState`, `.id`, `.component` | `itemList` | ? | — |
| `Via.location`, `.padstack`, `.firstLayer`/`.lastLayer` | `getVias()` | ? | — |
| `Pin`, `ObstacleArea`, `ConductionArea`, `ComponentOutline`, `BoardOutline` | `itemList` | ? | — |
| `UndoableObjects`' insert/delete generation counters | `itemList` | ? | — |
| `Line`'s identity token (ruling AE) | — | **must NOT be** | assert absent |

The `?` cells are the work: the implementer fills each with `yes` / `no` / `partially` from the current implementation (**`snapshot.rs:210-239`** — today it hashes **only** `Item::Trace` and `Item::Via`, with `_ => {}` for every other variant, in **ascending** `self.items` order, so most cells start at `no` and the iteration order itself has to flip per the note below) and, for every `no`, either widens the hash or records why the field cannot distinguish two boards the pipeline can reach. **The `Line` identity token must stay out** (plan-6 ruling AE: "no output, ordering, hash or serialised form can observe a token"), so widening must go through `Line`'s `PartialEq` fields, never its identity.

**Transcription notes.**
- Java's hash is over *serialized bytes*, so two boards with the same items in a different **`itemList` order** hash differently. The port's `structural_hash` must therefore fold in the item ids **in the port's own descending-id iteration order** (quirk #63) rather than a commutative combine, or `BoardHistory.contains` will accept a board Java rejects.
- The hash is a `String` in Java and `null` when the digest throws (`:68`); `BoardHistory.contains` then `NullPointerException`s on `entry.hash.equals(hash)`. Unreachable (MD5 is always available), but it is a `// totalized:` row: the port's `u64` has no null.
- **`getHash()` is *not* only about traces** despite the Javadoc at `BasicBoard.java:163` ("an MD5 hash of the board trace state") and `BoardSnapshotManager.java:56`. `// Java bug:` on the doc comment mismatch, and a quirk row — the wrong doc is exactly what justified the port's original narrow hash.

**Tests (`crates/fr-board/tests/snapshot.rs`, additive).**
- One test per `no` row the audit finds: construct two boards that differ **only** in that field and assert their `structural_hash`es differ.
- `reordering_the_item_list_changes_the_hash`.
- `the_line_identity_token_does_not_reach_the_hash` — clone a trace's polyline through a path that mints fresh tokens, assert the hash is unchanged (the ruling-AE contract).
- `diff_traces_is_the_symmetric_difference` (Java's `:84-99`, including that a trace present twice counts once).

**JVM-pinned evidence (required).** **`p7t10`** — the decision-parity driver ruling AH asks for. `P7T10.java` declares `package app.freerouting.autoroute;`, runs the HEAD jar with `RouterBudget::disabled()`'s Java equivalent (the `-Dfreerouting.opt_changed_area_ms=0` property the driver header documents), and for a fixture drives a **scripted board-mutation sequence** (insert trace, remove trace, insert via, move via, no-op) printing after each step three lines:
```
FANOUTSTOP  <bool>       # BatchFanout.java:152-156's currentBoardHash.equals(lastBoardHash)
CONTAINS    <bool>       # BoardHistory.contains against a 5-entry history
RANK        <int>        # BoardHistory.getRank against the same history
```
`p7t10.rs` prints the same three **decisions** from the port. Acceptance: **0 diffs over 2 000 mutation steps × the six corpus stems** — decisions, never hash values. Where the port's decision differs, `Board::diff_traces` is consulted and the row is recorded as an `XDIFF` with its root cause, per ruling AH.

**Steps:** write the audit table → `P7T10.java` → `p7t10.rs` → widen `structural_hash` field by field, one test per row → 0 decision diffs → re-run `p2t15` (its `hashEqual` boolean line must still be 0 diffs) → fmt/clippy/test/audit → commit `fix(board): widen structural_hash to serialize(true)'s field set, with decision parity (ruling AH)`.

---

### Task 4: the three-state stop, the deadline, the budget knob, `ProgressSink` and the small DTOs (ruling AI)

**Files:** `crates/fr-router/src/pipeline/{mod,stop,counters}.rs`, `crates/fr-router/src/lib.rs`; `crates/fr-router/tests/stop_and_progress.rs`.
**Java:** `core/StopRequestState.java` (**the enum's own file**); `core/StoppableThread.java (43)` — the flag `:8`, `requestStop` `:23-25`, `requestStopAutoRouter` `:33-37`, `isStopRequested` `:28-30`, `isStopAutoRouterRequested` `:40-42`; `core/RouterCounters.java (48)`; `core/ProgressThrottler.java (32)`; `autoroute/pipeline/NamedAlgorithmType.java (7)`; `autoroute/pipeline/TaskState.java (11)`; `autoroute/pipeline/NamedAlgorithm.java (126)` (the five identity accessors at **`:52-80`**, the three listener fields at `:26-31` and their `fire*` methods at `:88-93`, `:104-110`, `:121-125` rostered); `management/jobs/RoutingJobSchedulerActionThread.java:55-90` (36 — the monitor thread whose **effect** the deadline reproduces). *(Every line number in this block was re-derived from HEAD by the pre-flight scan; the plan's first draft had five of them wrong.)* **≈ 300 Java lines, of which ≈ 180 ported and ≈ 120 rostered.**

**Interfaces consumed:** `fr_board::{TimeLimit, StopCheck}` (Plan 2).
**Interfaces produced:** `StopRequestState`, `RouterStop`, `RouterBudget`, `NamedAlgorithmType`, `TaskState`, `ProgressSink`, `NoopProgressSink`, `RoutingEvent` — **exactly as written in §Interfaces above**, plus:
```rust
// crates/fr-router/src/pipeline/counters.rs
/// Port of `core.RouterCounters` (core/RouterCounters.java:1-48) — the per-pass counter DTO
/// `AutoroutePassRunner.updateProgress` fills and Java fires at every board-updated event.
/// Ported as a value (Task 16's per-pass trace asserts it); the **firing** is `ProgressSink`'s.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RouterCounters {
    /* Transcribe the field list **verbatim** from the 48-line file. HEAD's declaration order is
       `passCount, queuedToBeRoutedCount, routedCount, skippedCount, rippedCount,
        failedToBeRoutedCount, incompleteCount, phase, fanoutExtraViasCount` — nine fields, and
       `phase` and `fanoutExtraViasCount` are easy to miss. An earlier draft of this plan guessed
       six different names; none of them matched. Re-read the file, do not copy this comment. */
}
```

**Transcription notes.**
- **The two stop states are not interchangeable, and the difference is a quirk.** `AutoroutePassRunner.java:219` calls `thread.requestStop()` (**`ALL`**) when `maxItems` is reached, while `AutorouteBatchLoop.java:271` calls `requestStopAutoRouter()` (**`AUTO_ROUTER_ONLY`**) when `maxPasses` is reached — and `RoutingPipeline.java:117` tests `isStopRequested()`, so **hitting `--max-items` silently disables the optimizer stage while hitting `--max-passes` does not.** `// Java bug:` at the port's `maxItems` site plus a quirk row, and a test that asserts the optimizer does not run after a `maxItems` stop.
- **`requestStopAutoRouter` only upgrades `NONE → AUTO_ROUTER_ONLY`** (`:33-37`): once the state is `ALL` it is a no-op. That makes `AutorouteBatchLoop:251-253`'s `if (job.state == TIMED_OUT) thread.requestStopAutoRouter()` **always a no-op in production**, because the monitor thread already called `requestStop()` 30 s earlier (`RoutingJobSchedulerActionThread.java:75, 84`). Quirk row (survey cand. R); the port's `poll_deadline` therefore performs `request_stop()`, and `is_timed_out()` exists only so `AutorouteBatchLoop:578-584` can report `TaskState::TimedOut`.
- **Ruling AI's deadline is consulted at six sites and nowhere else** (the analogue of plan-6 ruling 6): `AutorouteBatchLoop:251`, `BatchFanout:111` and `:396`, `BatchOptimizer:172` and `:308`, and the top of `AutoroutePassRunner`'s item loop (`:203`). It is **threaded into `route_connection`'s `stop` closure** as well, so the six `AutorouteEngine.isStopRequested` sites see it — which is exactly what Java's shared `Stoppable` does (survey Appendix B). A seventh site makes a timed-out run stop earlier than Java's.
- **`RouterBudget` is a parameter, never a constant**, and Java's three literals are its defaults. `TraceTightener`'s ctor only builds a `TimeLimit` when `timeLimit > 0` (`TraceTightener.java:73-77`), so `opt_changed_area_ms = 0` is *exactly* Java's "no limit" and needs no new branch — record that, because it is why `RouterBudget::disabled()` is a faithful configuration rather than a port-only mode.
- `ProgressThrottler` (32) is `// not ported:`: its `shouldUpdate()` is a wall clock gating a progress event, and `ProgressSink` replaces the whole mechanism. **It carries no 1 000 ms constant** — the interval is a constructor argument (the plan's first draft cited a `core/ProgressThrottler.java:8` literal that does not exist); read the construction sites for the value. `NamedAlgorithm`'s `getId`/`getName`/`getVersion`/`getDescription`/`getType` (**`:52-80`**, not `:20-28`) are ported as five `const`s on the two algorithm structs; the three listener fields (`:26-31`) with their `add`/`remove`/`fire` triples (`:88-93`, `:104-110`, `:121-125`) are `// not ported:` naming `ProgressSink` (ruling AK).

**Tests (`crates/fr-router/tests/stop_and_progress.rs`):**
- `request_stop_auto_router_does_not_downgrade_all` and the full 3×2 transition table (`:23-42`).
- `max_items_stops_all_and_max_passes_stops_the_router_only` — the quirk's pin, asserted through `RouterStop`'s state.
- `the_deadline_requests_stop_all_like_the_monitor_thread`.
- `an_unexpired_deadline_is_invisible` — two runs of the same fixture, one with a deadline far in the future, `structural_hash` equal (ruling AI's determinism claim).
- **`a_recording_sink_changes_no_board_byte`** (ruling 11): route a fixture twice, once with `NoopProgressSink` and once with a sink that records every event, assert equal `structural_hash` and equal event count > 0.
- `the_budget_defaults_are_javas_literals` — **1000** (`AutorouteConnectionRouter.java:22`), **10000** (`BatchFanout.java:175-178`), **250** (`BatchAutorouter.java:335-343`) and `ProgressThrottler`'s constructor argument, each with its Java line in the assertion message. *(Scan ruling 10: four fields, not three, and 250 is one of them.)*
- `router_counters_field_list_matches_java` — asserts the port's field list against `p7t4-stop-and-counters.txt`'s reflected declaration order (**nine** fields), not against a doc comment.

**JVM-pinned evidence (required).** `scripts/differential/java/probes/P7T4Probe.java`, `package app.freerouting.core;`, HEAD jar, JDK 25, `-XX:hashCode=2`, headless flags. It drives `StoppableThread`'s transition table exhaustively (3 states × 2 requests × the two queries = every cell) and prints `RouterCounters`' field list by reflection, in declaration order, with each field's default. Stdout committed as `crates/fr-router/tests/data/p7t4-stop-and-counters.txt`. **This probe carries no clock**, deliberately: the deadline's behaviour is asserted against Java's *code* (`RoutingJobSchedulerActionThread.java:55-90`, read and quoted in the task) rather than against a timing measurement, because a timing measurement is not reproducible and ruling AI's whole point is that the parity runs never reach the deadline.

**Steps:** `P7T4Probe.java` → transcript → tests → implement → transcript matches → fmt/clippy/test/audit → commit `feat(router): the three-state stop, ruling AI's deadline and budget knob, and ProgressSink`.

---

### Task 5: `optChangedArea` and the `ConnectionToPin` **trio**

**Files:** `crates/fr-router/src/board_ext/{routing_board_ext,tightener/mod}.rs`, `crates/fr-board/src/{board/mod.rs,items/trace.rs}`; `crates/fr-router/tests/{opt_changed_area,connection_to_pin}.rs`; `scripts/differential/java/{P7T3.java,P7T6.java}`, `scripts/differential/rust/src/bin/{p7t3.rs,p7t6.rs}`.

**Markers this task owns (scan ruling 2 — exact, because Task 17's gate counts them):** `crates/fr-board/src/items/trace.rs` **`:96`** (`Trace.checkConnectionToPin`, abstract — becomes `// renamed:` pointing at the existing `fr-router` fn), **`:97`** (`PolylineTrace.checkConnectionToPin` — same), **`:98`**, **`:99`** (the two this task ports — `// renamed:` pointing at `PolylineTraceExt`), **`:101`** (`PolylineTrace.smoothenEndCornersFork` — its callee landed in Plan 6 Task 15a, so this becomes `// renamed:` pointing at `TraceTightener::smoothen_end_corners_at_trace`; **it was orphaned in the plan's first draft**); `crates/fr-board/src/board/mod.rs` **`:184`** (`RoutingBoard.optChangedArea`); and `crates/fr-router/src/board_ext/tightener/mod.rs` **`:47`** (`TraceTightener.optChangedArea`). `trace.rs:92` is prose about the marker convention and stays.
**Java:** `board/facade/RoutingBoardOperations.java` — `optChangedArea` `:52-79` (28); `board/facade/RoutingBoard.java` — `optChangedArea` overload 1 `:151-161` (11), overload 2 `:171-190` (20); `board/optimize/TraceTightener.java` — **`optChangedArea(ExpansionCostFactor[])` `:121-169`** (49, the class's last unported method; marker at `crates/fr-router/src/board_ext/tightener/mod.rs:47`); `board/trace/PolylineTrace.java` — **`correctConnectionToPin` `:1082-1245` (164)**, **`swapConnectionToPin` `:1252-1313` (62)**, and re-enabling the four call sites at `pullTight:844, 848, 853, 857`. **`checkConnectionToPin` `:1013-1076` (64) IS in scope** — the scan ruling that took it out was a false positive and the Task 5 review struck it (§A); Plan 6 never ported it. **≈ 400 Java lines.**

**Interfaces consumed:** `RouterStop`, `RouterBudget` (**Task 4 — so this task depends on Task 4**, which the dispatch note now records); `TraceTightener::{get_instance, split_traces_at_keep_point, smoothen_end_corners_at_trace}`, `PolylineTraceExt::{pull_tight, pull_tight_with, pull_tight_with_engine}` and **the existing private `check_connection_to_pin`** in `board_ext/tightener/mod.rs` (Plan 6 Tasks 15a/15b); `Board::overlapping_objects` (Plan 2) and the **public field** `Board::changed_area` (a field, not a method). **`Board::join_graphics_update_box` does not exist and is not needed** — it is the GUI repaint box this task rosters `// not ported:`.
**Interfaces produced:**
```rust
// crates/fr-router/src/board_ext/routing_board_ext.rs — RoutingBoardExt gains three methods
pub trait RoutingBoardExt {
    /* … the **seven** Plan 6 methods, unchanged: `init_autoroute`, `finish_autoroute`,
       `additional_update_after_change`, `clear_all_item_temporary_autoroute_data`,
       `check_forced_trace_polyline`, `insert_forced_trace_polyline`,
       `insert_forced_trace_segment`. (Both this plan's first draft and `plan-6-handoff.md` §3 say
       "five"; the trait has seven.) … */

    /// `RoutingBoard.optChangedArea(int[], TileShape, int, ExpansionCostFactor[], Stoppable, int)`
    /// (RoutingBoard.java:151-161 → :171-190 → RoutingBoardOperations.java:52-79). The batch
    /// pull-tight + via-optimise sweep every routed connection, every tail removal and every
    /// fanout pin runs.
    ///
    /// `time_limit_ms` is controller ruling AI's knob: Java's callers all pass the literal
    /// `TIME_LIMIT_TO_PREVENT_ENDLESS_LOOP = 1000`; `RouterBudget::opt_changed_area_ms` supplies
    /// it, and `0` is Java's own "no limit" (TraceTightener.java:73-77 only builds a `TimeLimit`
    /// when `timeLimit > 0`).
    #[allow(clippy::too_many_arguments)]
    fn opt_changed_area(
        &mut self,
        engine: Option<&mut AutorouteEngine>,   // PolylineTrace.change's additionalUpdateAfterChange
        only_net_no_arr: &[i32],
        clip_shape: Option<IntOctagon>,
        accuracy: i32,
        trace_costs: Option<&[ExpansionCostFactor]>,
        stop: StopCheck<'_>,
        time_limit_ms: i32,
    ) -> Result<(), BoardError>;

    /// The `keepPoint` overload (RoutingBoard.java:171-190) — `keep_point`/`keep_point_layer` are
    /// `null`/`0` from every router caller; only `RouteState` (GUI) passes them.
    #[allow(clippy::too_many_arguments)]
    fn opt_changed_area_with_keep_point(&mut self, /* … as above … */,
        keep_point: Option<Point>, keep_point_layer: i32) -> Result<(), BoardError>;
}

// crates/fr-router/src/board_ext/tightener/mod.rs
impl<'a> TraceTightener<'a> {
    /// Port of `TraceTightener.optChangedArea(ExpansionCostFactor[])` (TraceTightener.java:121-169)
    /// — the `while (somethingChanged)` sweep over `board.changedArea`, layer by layer.
    pub fn opt_changed_area(&mut self, board: &mut Board,
                            engine: Option<&mut AutorouteEngine>,
                            trace_costs: Option<&[ExpansionCostFactor]>) -> Result<(), BoardError>;
}

// crates/fr-router/src/board_ext/tightener/mod.rs — PolylineTraceExt gains the trio
pub trait PolylineTraceExt {
    /* … pull_tight, pull_tight_with, pull_tight_with_engine (Plan 6 Task 15a) … */

    /// `PolylineTrace.checkConnectionToPin(boolean)` (:1013-1076) and the abstract
    /// `Trace.checkConnectionToPin` (`board/model/items/Trace.java:376`) it overrides. **Not**
    /// ported in Plan 6 — the scan ruling that said so was a false positive (see the struck
    /// amendment bullet above). A **trait method**, not a private free fn: the pair's callers
    /// reach it through the trait, and `correctConnectionToPin:1083` is its only Java caller.
    fn check_connection_to_pin(board: &Board, trace: ItemId, at_start: bool) -> bool;

    /// `PolylineTrace.correctConnectionToPin(boolean, int)` (PolylineTrace.java:1082-1245) —
    /// the acid-trap correction; reaches `board.checkPolylineTrace` and `board.insertTrace`.
    fn correct_connection_to_pin(board: &mut Board, engine: Option<&mut AutorouteEngine>,
                                 trace: ItemId, at_start: bool, accuracy: i32)
        -> Result<bool, BoardError>;
    /// `PolylineTrace.swapConnectionToPin(boolean)` (PolylineTrace.java:1252-1313) — reaches
    /// `Pin.calcNearestExitRestrictionDirection` and `combine()`.
    fn swap_connection_to_pin(board: &mut Board, engine: Option<&mut AutorouteEngine>,
                              trace: ItemId, at_start: bool) -> Result<bool, BoardError>;
}
```

**Transcription notes.**
- **`optChangedArea`'s sweep, branch for branch** (`TraceTightener.java:121-169`): `while (somethingChanged)` (`:129`) → per layer `changedRegion = board.changedArea.getArea(i)`, skip empty (`:132-135`), **`board.changedArea.setEmpty(i)` before the work** (`:136`), `offset = 1.5 * (clearanceMatrix.maxValue(i) + 2 * rules.getMaxTraceHalfWidth())` (`:138-141`), `changedRegion.enlarge(offset)` (`:142`), `items = board.overlappingObjects(changedRegion, i)` — a **mixed room/item `TreeSet`** (ruling 5's sixth container) (`:145`), then per object: **`isStopRequested()` → `return`** (`:147-149`, the time limit's cut), `PolylineTrace → pullTight(this)` with `true → somethingChanged + splitTracesAtKeepPoint() + break` and `false → smoothenEndCornersAtTrace() + break` (`:150-159`), `Via && traceCosts != null → ViaOptimizer.optViaLocation(board, via, traceCosts, minTranslateDist, 10)` (**`:160-165`**). **Both arms `break` out of the item loop**, so one object is processed per layer per outer iteration — transcribe that, it is not a bug and it is not obvious.
- **The `ViaOptimizer` arm is stubbed to `false` in this task with an `obligation:` marker naming Task 6**, which discharges it. The stub must not silently skip: it is `// obligation: TraceTightener.optChangedArea's ViaOptimizer arm (:160-165) — Task 6`, and `p7t3`'s no-via modes are the ones that MATCH here. Note Task 6's `opt_via_location` takes an `engine: Option<&mut AutorouteEngine>` the Java call site has no argument for; thread the same `engine` this method already carries.
- **Ruling 9's reference comparison** (`RoutingBoardOperations.java:64`): port `clip_shape` as `Option<IntOctagon>` where `None` **runs** the branch, with the `// Java bug:` marker and the test.
- `RoutingBoardOperations.optChangedArea` returns early when `board.changedArea == null` (`:61-63`), then after the tightener joins `board.joinGraphicsUpdateBox(changedArea.surroundingBox())` (`:77`) — a GUI repaint box, `// not ported:` — and **sets `board.changedArea = null`** (`:78`). That last line is load-bearing: the next `startMarkingChangedArea` re-creates it, and forgetting it makes the second sweep see a stale region.
- **The trio re-enters through `pullTight`** (`PolylineTrace.java:841-861`), only when the polyline came back unchanged, the angle restriction is **not** 90°, and `board.rules.getPinEdgeToTurnDist() > 0` — then `swapConnectionToPin(true)` `:844`, `swapConnectionToPin(false)` `:848`, `correctConnectionToPin(true, …)` `:853`, `correctConnectionToPin(false, …)` `:857`, **each recursing into `pullTight` on success**. Plan 6 pinned `pinEdgeToTurnDist = -1` inside `FoundConnectionInserter.insertTrace:140-141` (restored at `:447`), which is why the branch was unreachable there and is reachable here: `optChangedArea` calls `pullTight` **outside** that window. Delete the **single-line** marker at **`crates/fr-router/src/board_ext/tightener/mod.rs:771`** — it names **two** methods (`swapConnectionToPin` and `correctConnectionToPin`) and the four calls at `:844-860`, which is exactly this task's scope; `:775` is a bare `false`, and the first draft's "marker block at `:775`" does not exist. Re-point the five `crates/fr-board/src/items/trace.rs` markers listed under **Files** above.
- **Quirk #182 (`avoidAcidTraps` disabled by `if (true) return`) and quirk #184 (`TraceTightener45.reduceCorners`' stale clip flag)** both live on this path. #184 is *reachable only with a non-null `clipShape`*, i.e. only from `removeItemsAndPullTight`/`RouteState`. Re-check that at port time and record the answer; if a Plan 7 caller does pass a clip shape, #184 becomes live and needs a test. **Task 5 re-checked it and it is dead there**: every `autoroute/pipeline` caller passes `null` (`AutorouteConnectionRouter:103`, `:223`, `BatchAutorouter:492`, `BatchAutorouterThread:527`, `:563`). **The obligation is Task 8's, not "a future task's"** — Task 8 owns `removeItemsAndPullTight` (index line 206, task heading line 869), whose `:117` call is the only Plan 7 path passing a non-null `clipShape`, so **#184 becomes live there and needs a test**.
- `correctConnectionToPin` (164 lines) calls `combine()` and `insertTrace`, which route through **`PolylineTrace.change` (`board/trace/PolylineTrace.java:937`** — *not* `:188`, which is inside `combine()`; the plan's first draft and `crates/fr-board/src/board/trace_normalize.rs:67` both cite `:188`) — the site quirk #74 is about. Ruling AE's contract (Conventions §7) applies at every `Polyline` construction inside it. (`Trace.java` lives at `board/model/items/`, not `board/trace/`.)

**Tests.**
- `crates/fr-router/tests/opt_changed_area.rs`: `a_null_changed_area_returns_immediately`; `the_changed_area_is_cleared_after_the_sweep`; `a_none_clip_shape_runs_the_branch` (ruling 9); `the_layer_region_is_emptied_before_the_work` (`:136`); `the_enlarge_offset_is_javas_formula` (three clearance matrices); `the_item_loop_breaks_after_one_object`; `a_tripped_stop_check_returns_mid_sweep_leaving_the_rest_untightened` (the parity hazard, asserted as a *fact* about the port, with `RouterBudget::disabled()` as the parity configuration).
- `crates/fr-router/tests/connection_to_pin.rs`: one test per **ported** method (`correct`, `swap`) per angle regime from `p7t6`'s literals, plus a regression test that the **existing** `check_connection_to_pin` still agrees with `p7t6` mode 0 now that it is reachable; `the_pair_is_skipped_at_ninety_degrees`; `the_pair_is_skipped_when_pin_edge_to_turn_dist_is_not_positive`; `a_successful_swap_recurses_into_pull_tight`.

**JVM-pinned evidence (required).**
- **`p7t3 <dsn> <mode> <accuracy>`** — `P7T3.java`, `package app.freerouting.board.optimize;`, HEAD jar, JDK 25, `-XX:hashCode=2`, headless flags, **`optChangedArea`'s budget disabled on both sides** (the Java half sets its own copy of `TIME_LIMIT_TO_PREVENT_ENDLESS_LOOP` to 0 via reflection on `RoutingBoard`/`BatchAutorouter`, documented in the driver header; the Rust half passes `RouterBudget::disabled()`). It marks a changed area, runs the sweep and dumps the board in `P6T15aProbe`'s polyline format. Modes: `0` no vias / 90°, `1` no vias / 45°, `2` no vias / any-angle, `3` `pinEdgeToTurnDist > 0` (the trio live), `4` vias present (**expected to diff until Task 6**, and the README says so). Acceptance here: **0 diffs on modes 0-2**.
- **`p7t6 <mode>`** — `P7T6.java`, `package app.freerouting.board.trace;`, drives `check/correct/swapConnectionToPin` (`check` as the **regression oracle for the existing port**, the other two as the new work) on hand-built traces at every `pinEdgeToTurnDist` × angle regime × start/end. Acceptance: **0 diffs, all modes**; then `p7t3 3` **0 diffs**.

**Steps:** `P7T3.java`/`P7T6.java` → the two Rust twins → tests from the Java output → implement `optChangedArea` with the `ViaOptimizer` arm stubbed → `p7t3 0-2` 0 diffs → **confirm the existing `check_connection_to_pin` against `p7t6` mode 0 before writing anything** → implement `correct` and `swap` → `p7t6` 0 diffs → `p7t3 3` 0 diffs → re-point the six markers → fmt/clippy/test/audit → commit `feat(router): optChangedArea's tightener sweep and PolylineTrace's correct/swapConnectionToPin`.

---

### Task 6: `ViaOptimizer` part A — `optViaLocation`, `optPlaneOrFanoutVia`, `isWithinTolerance`, and `RoutingBoard.moveDrillItem` (scan ruling 3)

**Files:** `crates/fr-router/src/board_ext/{mod,via_optimizer,drill_item_mover}.rs`, `crates/fr-board/src/board/mod.rs` (the **`:186`** `// added in Plan 7: RoutingBoard.moveDrillItem` marker becomes `// renamed:` pointing at `fr-router`), `scripts/audit-map/fr-router.map` (re-point the `ViaOptimizer` row from `lib.rs`); `crates/fr-router/tests/via_optimizer.rs`; `scripts/differential/java/P7T4.java`, `scripts/differential/rust/src/bin/p7t4.rs`.
**Java:** `board/optimize/ViaOptimizer.java` (**733 lines**) — `optViaLocation` `:33-158` (126), `optPlaneOrFanoutVia` `:161-296` (136), `isWithinTolerance` `:719-732` (14); **`board/facade/RoutingBoard.java` — `moveDrillItem` `:252-295` (44)**. **≈ 320 Java lines.**

> **Scan ruling 3 folds `RoutingBoard.moveDrillItem` into this task.** The plan's first draft listed `Board::move_drill_item` under §Interfaces consumed as a Plan 2 / Plan 6 Task 10b deliverable. **It does not exist** — it is itself an unconsumed `// added in Plan 7:` marker at `crates/fr-board/src/board/mod.rs:186`, and both `optViaLocation` and `optPlaneOrFanoutVia` move vias through it. Port it here, in `fr-router`'s `board_ext` beside Plan 6's `DrillItemMover` (whose *mutating* half controller ruling AB already landed), and re-point the marker. If the implementer finds Plan 6's `DrillItemMover` already does the whole job, that is a **Java-wins report**: say so, re-point the marker to it, and skip the 44 lines.

**Interfaces consumed:** `RoutingBoardExt::opt_changed_area` and `TraceTightener::opt_changed_area` (Task 5, whose `ViaOptimizer` arm this discharges); `PolylineTraceExt::pull_tight` (Plan 6 Task 15a); `Board::pick_items` and **`Board::normal_contacts`** (Plan 2 — the port's name; there is no `item_normal_contacts`); `DrillItemMover` (Plan 6 Task 10b). **`Board::move_drill_item` does not exist — this task creates its port** (scan ruling 3, above).
**Interfaces produced:**
```rust
// crates/fr-router/src/board_ext/via_optimizer.rs
/// Port of `board.optimize.ViaOptimizer` (ViaOptimizer.java:1-732) — the via-repositioning half
/// of the pull-tight sweep. **It is not the optimizer's**: `TraceTightener.optChangedArea:160-164`
/// calls it, so it runs on *every* `optChangedArea` with non-null trace costs — every routed
/// connection (AutorouteConnectionRouter:107), every tail removal (BatchAutorouter:496) and every
/// fanout pin (RoutingBoard.fanout:1105).
pub struct ViaOptimizer;
impl ViaOptimizer {
    /// `optViaLocation(RoutingBoard, Via, ExpansionCostFactor[], int, int)` (:33-158) — the only
    /// public entry. `true` when the via moved.
    pub fn opt_via_location(board: &mut Board, engine: Option<&mut AutorouteEngine>,
                            via: ItemId, trace_costs: &[ExpansionCostFactor],
                            min_translate_dist: i32, accuracy: i32)
        -> Result<bool, BoardError>;
    /// `optPlaneOrFanoutVia(...)` (:161-296) — private in Java; `pub(crate)` here so the tests and
    /// `p7t4` can reach it without reflection.
    pub(crate) fn opt_plane_or_fanout_via(/* … */) -> Result<bool, BoardError>;
    /// `isWithinTolerance(double, double, double)` (:719-732).
    pub(crate) fn is_within_tolerance(value: f64, target: f64, tolerance: f64) -> bool;
}
```

**Transcription notes.**
- `optViaLocation`'s **dispatch to the three `repositionVia` overloads is at `:46-78`**, not `:100-140` as the plan's first draft said (`:100-140` is corner/tolerance computation): `contacts.size() == 1` at `:47`, `!= 2` at `:51`, and `isPlaneOrFanoutVia → optPlaneOrFanoutVia` at `:76-78`. Task 7 inherits this correction.
- `optViaLocation` reads `via.getNormalContacts()` (a `TreeSet<Item>`) and `board.pickItems(...)` (a `TreeSet<Item>`) — both **descending item id** (`BTreeSet<ItemId>` iterated `.rev()`, Conventions §3), and both are ruling 5 containers whose decision is recorded.
- It calls `PolylineTrace.pullTight(true, tracePullTightAccuracy, null)` at `:144` and `:148` (and `optPlaneOrFanoutVia` at `:290`) — the **three-argument** overload, which is `PolylineTraceExt::pull_tight` (Plan 6 Task 15a), not `pull_tight_with`.
- `isWithinTolerance` is a `double` comparison: transcribe the operator chain literally, `java_min`/`java_max` where Java uses `Math.*` (Conventions §4).
- Both methods mutate via position through the drill-item mover (`RoutingBoard.moveDrillItem:252-295`, ported here); a failed move must leave the board **exactly** as it was (Java restores by re-inserting), and there is a `structural_hash`-before/after test for the refusal path.
- The audit-map row currently points `ViaOptimizer` at `lib.rs`; re-point it to `board_ext/via_optimizer.rs` in **this** task, not in Task 17.

**Tests (`crates/fr-router/tests/via_optimizer.rs`):** `opt_via_location_moves_the_via_to_javas_centre` (a table from `p7t4`); `a_refused_move_leaves_the_board_byte_identical`; `is_within_tolerance_matches_java_at_the_boundary` (three values straddling it); `the_normal_contacts_are_visited_descending`; `a_plane_via_takes_the_plane_arm`; **`the_overload_dispatch_matches_javas_contact_counts`** (`:46-78`); **`move_drill_item_matches_the_jvm`** (from `p7t4`, scan ruling 3).

**JVM-pinned evidence (required).** **`p7t4 <dsn> <viaIndex> <mode>`** — `P7T4.java`, `package app.freerouting.board.optimize;` (to reach the private overloads without reflection), HEAD jar, JDK 25, `-XX:hashCode=2`, headless flags, budget disabled both sides. Per via it prints the returned boolean, the via's centre before and after, its contact list (ids, descending) and the full board dump in `P6T15aProbe`'s format. Modes `0` = `optViaLocation`, `1` = `optPlaneOrFanoutVia` directly, `2` = `isWithinTolerance` over 10 000 scripted triples. Acceptance: **0 diffs over every via of the six corpus stems, modes 0 and 2** (mode 1 and the `repositionVia` arms are Task 7's, and the README records that they diff until then).

**Steps:** `P7T4.java` → `p7t4.rs` → tests → port `moveDrillItem` and re-point `board/mod.rs:186` → implement → `p7t4 0/2` 0 diffs → fmt/clippy/test/audit → commit `feat(router): ViaOptimizer's optViaLocation, the plane/fanout arm and RoutingBoard.moveDrillItem`.

---

### Task 7: `ViaOptimizer` part B — the three `repositionVia` overloads

**Files:** `crates/fr-router/src/board_ext/via_optimizer.rs`; `crates/fr-router/tests/via_optimizer_reposition.rs`; `scripts/differential/java/P7T4.java` (+ modes 3-5), `scripts/differential/rust/src/bin/p7t4.rs`.
**Java:** `board/optimize/ViaOptimizer.java` — `repositionVia` A `:302-365` (64), `repositionVia` B `:367-429` (63), **`repositionVia` C `:435-713` (279)**. **≈ 406 Java lines.**

**Interfaces consumed:** everything Task 6 produced.
**Interfaces produced:**
```rust
impl ViaOptimizer {
    /// `repositionVia(RoutingBoard, Via, IntPoint, int, int, int)` overload A (:302-365) — the
    /// **one-contact / plane-or-fanout** case: its only caller is `optPlaneOrFanoutVia:216-217`.
    /// *(Task 6 correction: the first draft called this "the two-contact case" and named it
    /// `reposition_via_two_contacts`. Verified with `grep -n repositionVia` on the Java file —
    /// overload **C** is the two-trace one. Task 6 landed the stub as
    /// `reposition_via_toward_location`, and as an `unimplemented!` rather than a `None`, per
    /// controller ruling B1; Task 7 replaces the body and keeps the name.)*
    pub(crate) fn reposition_via_toward_location(/* … */) -> Option<Point>;
    /// Overload B (:367-429) — the candidate check, reached **only** from inside overload C.
    pub(crate) fn reposition_via_check_candidate(/* … */) -> bool;
    /// Overload C (:435-713) — the general case, 279 lines of candidate enumeration. **The
    /// longest single method in Plan 7 and the one whose branch list must be transcribed rather
    /// than summarised.**
    pub(crate) fn reposition_via_general(/* … */) -> Result<bool, BoardError>;
}
```

**Transcription notes.**
- Java overloads on the **arity and types** of the contact list, not on a mode flag; the port renames the three (`// renamed:` on each) because Rust has no overloading. `optViaLocation` chooses between them by contact count — read **`:46-78`** (`size() == 1` at `:47`, `!= 2` at `:51`, the plane/fanout arm at `:76-78`) and reproduce the dispatch exactly, including which count falls to the general case. *(The first draft cited `:100-140`, which is corner/tolerance computation.)* **Task 6 correction to the dispatch map below:** overload **A** has exactly one caller, `optPlaneOrFanoutVia:216-217` — the **one**-contact arm; overload **C** is the two-trace one (`optViaLocation:118-131`); overload **B** has no caller outside C. The test names the first draft gave (`a_two_contact_via_takes_overload_a`, `…three_contact…b`, `…four_contact…c`) describe a dispatch that does not exist — rename them `a_one_contact_via_takes_overload_a`, `a_two_trace_via_takes_overload_c` and `overload_b_is_reached_only_from_c`.
- Overload C's candidate enumeration walks the trace directions of every contact and evaluates a weighted cost with `trace_costs[layer]` — every constant is read from Java, none is chosen. Where two candidates tie, Java keeps the **first** found in contact order (descending id); the port must not use a `max_by` that keeps the last.
- Ruling AE's `Line`/`Polyline` contract applies at every polyline rebuild in C.
- Discharge Task 5's `obligation:` marker on the `ViaOptimizer` arm here, and re-run `p7t3 4` (the vias-present mode) to 0 diffs.

**Tests (`crates/fr-router/tests/via_optimizer_reposition.rs`):** one fixed case per overload from `p7t4` modes 3-5; `a_two_contact_via_takes_overload_a`, `…three_contact…b`, `…four_contact…c` (the dispatch pin); `a_candidate_tie_keeps_the_first_in_contact_order`; `the_general_case_leaves_the_board_untouched_when_no_candidate_improves`.

**JVM-pinned evidence (required).** `p7t4` modes `3`/`4`/`5` (one per overload, each driven directly), **0 diffs over every via of the six corpus stems**; then **`p7t3` mode 4 (vias present) 0 diffs**, and `p7t3` modes 0-3 re-run and still 0 diffs. *(Task 6 note: modes 3/4/5 are free — Task 6's fourth mode is numbered **6**, not 3, precisely to reserve them. Task 6 also left two things for this task to **delete**: `ViaOptimizer::reaches_task_seven_guard` with the `TASK7_GUARD` rows both halves of `p7t4` print, and the `#[should_panic]` on `opt_changed_area.rs`'s `mode_four_is_task_sevens_obligation`, which becomes the parity assertion. Both exist because overload A's stub had to be an `unimplemented!` rather than a `None` — controller ruling B1.)*

**Steps:** `P7T4.java` modes 3-5 → tests → implement (removing overload A's `unimplemented!` and `reaches_task_seven_guard`) → `p7t4 3/4/5` **and** `0/1/2/6` 0 diffs → `p7t3 4` 0 diffs → discharge Task 5's marker → fmt/clippy/test/audit → commit `feat(router): ViaOptimizer's three repositionVia overloads, discharging the optChangedArea stub`.

---

### Task 8: `BatchAutorouter`'s scaffold, `removeTails`, `removeItemsAndPullTight`, and `route`'s steps 6-8 (ruling AJ folded in)

**Files:** `crates/fr-router/src/pipeline/batch_autorouter.rs`, `crates/fr-router/src/board_ext/routing_board_ext.rs`, `crates/fr-router/src/autoroute/maze/engine.rs` (the three `// added in Plan 7:` markers at **`:1647`, `:1797`, `:1825`** — **`:1647` and `:1825` are both `AutorouteConnectionRouter.retryConnectionNecked`; only `:1797` is the steps-6-8 marker**, which is not what the plan's first draft said), `crates/fr-board/src/board/{mod.rs,trace_normalize.rs,shape_trace_entries.rs}` (**ruling AJ's five markers, verified present at `board/mod.rs:404`, `:447`, `shape_trace_entries.rs:880`, `trace_normalize.rs:67`, `:625`**, plus `board/mod.rs:185`'s `RoutingBoard.removeItemsAndPullTight`); `crates/fr-router/tests/{batch_autorouter,strict_drc}.rs`.
**Java:**
- `autoroute/pipeline/BatchAutorouter.java` (**565 lines**) — the constants `:38-64` (27, incl. the two `Boolean.getBoolean` system properties at `:61-64` and `BOARD_RANK_LIMIT = BoardHistory.MAX_HISTORY_SIZE` at `:40`), fields `:66-107` (42), both ctors `:110-158` (49), `isBenchmarkProfileEnabled` `:160-162`, the six accessors `:164-182` (19), `getImpactedPoints` `:283-297` (15), **`enforceStrictDrc` `:305-329` (25)**, `isFanoutTimedOut` `:331-333`, `shouldFireBoardUpdate` `:335-343` (9), `runBatchLoop` `:479-481`, `buildUnroutedConnectionsReport` `:483-485`, **`removeTails` `:487-503` (17)**, `autorouteItem` `:507-514` (8), `threadIndexToLetter` `:536-554` (19), `calculateIncompleteCount` `:556-564` (9);
- `board/facade/RoutingBoardOperations.java` — `removeItemsAndPullTight` `:81-120` (40); `board/facade/RoutingBoard.java` — `removeItemsAndPullTight` `:124-127` (4);
- `autoroute/pipeline/AutorouteConnectionRouter.java` — **step 6** `:95-121` (27), **step 7** `:123-153` (31), **`retryConnectionNecked` `:162-241` (80)**, **`applyStrictDrcAfterRoute` `:243-254` (12)**.
**≈ 506 Java lines.** The profile counters `:184-238` (55) and every `System.nanoTime()` block are `// not ported:` (all guarded by `-Dfreerouting.benchmark.profile`, default false).

**Interfaces consumed:** `RoutingBoardExt::opt_changed_area` (Task 5); `route_connection` (Plan 6 Task 16); `RouterStop`, `RouterBudget` (Task 4); `Board::{remove_trace_tails, remove_items_marking_changed_area, combine_traces, get_connectable_items, deep_copy}` (Plan 2).
**Interfaces produced:**
```rust
// crates/fr-router/src/pipeline/batch_autorouter.rs
/// Port of `autoroute.pipeline.BatchAutorouter` (BatchAutorouter.java:36-564) — the object the
/// pass runner and the optimizer both drive. Its `NamedAlgorithm` identity is Task 4's five
/// consts; its three listener lists are `// not ported:` (ruling AK).
pub struct BatchAutorouter<'a> {
    // Every line number below was re-derived from HEAD by the pre-flight scan; the plan's first
    // draft had five of the seven wrong. Re-read `:66-120` rather than trusting either.
    pub board_is_optimizer_owned: bool,          // :99  isOptimizerAutorouter
    pub remove_unconnected_vias: bool,           // :115 = !settings.isFanoutEnabled()
    pub with_preferred_directions: bool,         // :116
    pub start_ripup_costs: i32,                  // :117
    pub trace_pull_tight_accuracy: i32,          // :118-120
    pub fanout_timed_out: bool,                  // :80, written by AutorouteBatchLoop:173
    pub total_items_routed: i32,                 // :79
    settings: &'a RouterSettings,
    /* … transcribe the field block :66-107 verbatim … */
}
impl<'a> BatchAutorouter<'a> {
    /// The two constructors (:110-135 and :138-158).
    pub fn new(settings: &'a RouterSettings, /* … */) -> BatchAutorouter<'a>;
    /// `removeTails(StopConnectionOption)` (:487-503): `startMarkingChangedArea`,
    /// `removeTraceTails(-1, option)`, then `optChangedArea(new int[0], null, accuracy,
    /// traceCosts, thread, 1000)` — the `1000` is `RouterBudget::opt_changed_area_ms`.
    pub fn remove_tails(&self, board: &mut Board, option: StopConnectionOption,
                        stop: StopCheck<'_>, budget: RouterBudget) -> Result<(), BoardError>;
    /// `enforceStrictDrc(RoutingBoard, int, int)` (:305-329) — walks the net's connectable items,
    /// keeps those with `id > maxItemIdBefore` that are a `Trace` or a `Via`, and removes them all
    /// if any has a clearance violation. `None` = keep, `Some(msg)` = rejected.
    pub fn enforce_strict_drc(board: &mut Board, net_no: i32, max_item_id_before: ItemId)
        -> Option<String>;
    /// `shouldFireBoardUpdate()` (:335-343) — the 250 ms gate, now `RouterBudget`'s.
    pub fn should_fire_board_update(&mut self, budget: RouterBudget) -> bool;
    /// `getImpactedPoints(...)` (:283-297) and `calculateIncompleteCount(...)` (:556-564).
    pub fn impacted_points(/* … */) -> Vec<FloatPoint>;
    pub fn calculate_incomplete_count(board: &mut Board) -> usize;
}

// crates/fr-router/src/board_ext/routing_board_ext.rs
pub trait RoutingBoardExt {
    /// `RoutingBoard.removeItemsAndPullTight(...)` (RoutingBoard.java:124-127 →
    /// RoutingBoardOperations.java:81-120): remove, collect the changed nets into a
    /// `TreeSet<Integer>` (:93 — **ascending**, ruling 5), `combineTraces(netNo)` in that order
    /// (:111-113), then `optChangedArea`.
    ///
    /// **Headless-dead**: the only Java caller is `gui/interactive/RouteState.java:340`
    /// (`RouteState.cancel()` with push enabled). Ported because the audit demands it and Plan 8's
    /// interactive surface may reach it; a doc line says so, and no Plan 7 code calls it.
    #[allow(clippy::too_many_arguments)]
    fn remove_items_and_pull_tight(&mut self, engine: Option<&mut AutorouteEngine>,
        items: &BTreeSet<ItemId>, clip_shape: Option<IntOctagon>, accuracy: i32,
        with_preferred_directions: bool, stop: StopCheck<'_>, time_limit_ms: i32)
        -> Result<(), BoardError>;
}

// crates/fr-router/src/autoroute/maze/engine.rs — ruling 2's wrapper, NOT a change to route_connection
/// The whole of `AutorouteConnectionRouter.route` (AutorouteConnectionRouter.java:30-160, in a
/// 255-line file): [`route_connection`]'s steps 1-5 plus **steps 6-8** — `optChangedArea` on
/// `ROUTED` (`:95-121`),
/// the necked retry (`:123-153`, `retryConnectionNecked` `:162-241`) and the strict-DRC rollback
/// (`:147-153`, `applyStrictDrcAfterRoute` `:243-254`). Plan 6's entry point is unchanged
/// (plan-7 ruling 2), so every Plan 6 test and `p6t1` keep working.
///
/// **The `:160-233` in `crates/fr-router/src/lib.rs`'s roster line and in `docs/plan-6-handoff.md`
/// §10.1 is wrong** — `:160` is `route`'s closing brace and nothing spans `:160-233` as a unit.
/// The decomposition above is the one read out of HEAD; re-point the roster line to it, and note
/// that the handoff's "and the failure-log write" belongs to Task 9's `runSingleThread` (`:260-289`),
/// not to this wrapper.
#[allow(clippy::too_many_arguments)]
pub fn route_connection_full(
    board: &mut Board, engine: &mut Option<AutorouteEngine>, item: ItemId, net_no: i32,
    settings: &RouterSettings, trace_costs: &[ExpansionCostFactor],
    ripped: &mut BTreeSet<ItemId>, ripup_costs: &mut BTreeMap<ItemId, i32>,
    ripup_pass_no: i32, start_ripup_costs: i32,
    remove_unconnected_vias: bool, budget: RouterBudget, stop: StopCheck<'_>,
) -> AutorouteAttemptResult;
```

**Transcription notes.**
- **Ruling AJ, here.** `retainAutorouteDatabase` is `BatchAutorouter.java:63-64`'s `Boolean.getBoolean("freerouting.benchmark.retain_autoroute_database")` read at `:151-154` — a benchmark-only system property, hard-coded `false` on every production and parity path. The port has **no setter**: `route_connection_full` passes `retain_autoroute_database = false` unconditionally, and the five `fr-board` markers become
  `// not reachable: RoutingBoard.additionalUpdateAfterChange (retainAutorouteDatabase is a Java benchmark-only system property)`
  at `board/mod.rs:404`, `board/mod.rs:447`, `board/shape_trace_entries.rs:880`, `board/trace_normalize.rs:67` and `board/trace_normalize.rs:625`. A test asserts the port exposes no way to set it (a `grep` test, as Plan 6 used for `item_tree_shape_ref`). **No `fr-board` signature changes.**
- **Step 6** (`:95-121`): on `ROUTED`, `board.optChangedArea(new int[0], null, getTracePullTightAccuracy(), ctrl.traceCosts, thread, 1000)`. The empty `int[0]` is "all nets"; `null` is the clip shape (ruling 9).
- **Step 7's necked retry** (`:123-153` → `:162-241`): fires only when the state is `FAILED` **or** `INSERT_ERROR` **and** `getNeckWidthUm() > 0`. Inside: `neckWidth = round(Unit.scale(neckWidthUm * resolution, UM, board unit))` (`:173-179`, `java_round`), `neckHalfWidth = max(1, neckWidth / 2)` (`:180`, integer division), `narrowerSomewhere = any(layerActive[i] && traceHalfWidth[i] > neckHalfWidth)` (`:182-190`) — **and if nothing is narrower the retry is skipped**; then a **fresh** `AutorouteControl` with the same ripup settings (`:192-197`), per layer `halfWidth = min(halfWidth, neckHalf)` with the **compensation preserved** (`:198-202`), `initAutoroute` again (`:204-210`) **reusing the same `TimeLimit` object the first attempt already consumed** (`:171`, `:209` — quirk cand. L: on a hard connection the retry inherits an exhausted budget and is a no-op precisely where it is wanted; `// Java bug:` + quirk row), `autorouteConnection` with the **same** start/dest/ripped/costs (`:212-214`), `!= ROUTED → return null` (`:218-220`), else `optChangedArea` with the neck control's trace costs (`:223-229`).
- **Step 8's strict-DRC rollback** (`:147-153` → `:243-254`): `!isStrictDrc() → null`; else `enforceStrictDrc(board, netNo, maxItemIdBefore)`; on a rejection **and** a non-null snapshot (the guard at `:250`), `board = (RoutingBoard) BasicBoard.deserialize(snapshot)` (`:251`). Ruling 8: the snapshot is a `Board` clone taken at `:84-85` (`isStrictDrc() ? board.serialize(false) : null`) and the rollback is a restore-from-clone. **Quirk cand. M**: Java assigns `router.board`, a field of `BatchAutorouter`, so `AutorouteBatchLoop.run`'s local `RoutingBoard board` (`:38`) goes stale — latent, because nothing reads it again. Record the row; the port has one board and cannot reproduce the staleness, which is a `// totalized:`.
- `enforceStrictDrc` (`:305-329`) uses `maxGeneratedId()` before the route (`:83`) and `id > maxItemIdBefore` after — the port's `ItemId` ordering is the same monotone counter, verified by Plan 6 Task 17's "the item ids each connection burned" acceptance line.
- `threadIndexToLetter` (`:536-554`) is a log-string helper for the dead multithread path: `// not ported:` with the caller evidence.
- **Ruling AJ, the five markers, verbatim.** Their exact sites are confirmed above; each becomes the single line `// not reachable: RoutingBoard.additionalUpdateAfterChange (retainAutorouteDatabase is a Java benchmark-only system property)` with the Java method name **on the marker's own line** (Conventions §2 — `audit-port.sh` is line-based).

**Tests.**
- `crates/fr-router/tests/strict_drc.rs` — **the port of `src/test/java/app/freerouting/autoroute/StrictDrcEnforcementTest.java` (81 loc, ruling 14)**, method for method, plus `a_rejected_connection_restores_the_pre_route_board_exactly` (`structural_hash` equality against the clone).
- `crates/fr-router/tests/batch_autorouter.rs`: `remove_tails_marks_and_opt_changed_areas`; `the_necked_retry_is_skipped_when_no_layer_is_wider_than_the_neck`; `the_necked_retry_reuses_the_exhausted_time_limit` (the quirk's pin — assert the second attempt sees an already-exceeded `TimeLimit`); **`retain_autoroute_database_is_false_on_every_path`** — call `route_connection_full` and assert, through an observable the flag controls (`additional_update_after_change` was **not** run), that it behaved as `false`; **plus** a `retain_autoroute_database_has_no_setter` grep assertion beside it, whose doc comment says it is a roster assertion, not a behavioural one *(ruling AJ asked for the grep; the pre-flight scan asks that a behavioural test sit beside it so the pair is not a bare source grep)*; `should_fire_board_update_uses_the_budget`; `remove_items_and_pull_tight_combines_traces_in_ascending_net_order`.

**JVM-pinned evidence (required).** Extend **`p6t1`** with a `steps` argument: `run.sh p6t1 <dsn> <maxItems> <ripupPassNo> <rules|-> <steps>` where `steps` is `1-5` (Plan 6's, the committed references) or **`1-8`** (this task's). The Java half calls `AutorouteConnectionRouter.route` in full instead of inlining steps 1-5; both halves run with `RouterBudget::disabled()`'s configuration (the Java side reflects `TIME_LIMIT_TO_PREVENT_ENDLESS_LOOP` to 0 and `neckWidthUm` from the fixture's settings). Acceptance: **`--steps=1-8` MATCH on all five reference stems at `ripupPassNo` 1 and 2**, and `--steps=1-5` still MATCH (the committed `router.jsonl` references are untouched). Commit the `1-8` transcripts as `tests/reference/<stem>/router-steps18.jsonl` with their own `.meta.txt`.

**Steps:** ruling AJ's five marker re-points + the grep test → `p6t1 --steps` on the Java side → tests → implement `removeTails`/`removeItemsAndPullTight`/the scaffold → implement steps 6-8 as `route_connection_full` → `p6t1 --steps=1-8` MATCH ×5 stems × 2 passes → port `StrictDrcEnforcementTest` → fmt/clippy/test/audit → commit `feat(router): BatchAutorouter's scaffold, removeTails, and route's steps 6-8 with the strict-DRC rollback`.

---

### Task 9: the pass — `getAutorouteItems`, `RoutingFailureLog`, `ItemRouteResult`, `calculateAirline` and `runSingleThread`

**Files:** `crates/fr-router/src/pipeline/{pass_runner,failure_log,item_route_result,airline,batch_autorouter}.rs`, `crates/fr-router/src/lib.rs` (the nine `// added in Plan 7: RoutingFailureLog.*` markers at `:189-197` and the twelve `ItemRouteResult.*` at `:200-211` — **every one gets a disposition, see the marker note below**), `crates/fr-board/src/board/mod.rs` (**`:182`** `RoutingBoard.autoroute` and **`:214`** `autoroute.RoutingFailureLog` — see the same note); `crates/fr-router/tests/{pass_runner,item_route_result}.rs`; `scripts/differential/java/{P7T1.java,P7T2.java}`, `scripts/differential/rust/src/bin/{p7t1.rs,p7t2.rs}`.
**Java:**
- `autoroute/pipeline/BatchAutorouter.java` — **`getAutorouteItems` `:345-409` (65)**, `autoroutePass` `:419-421`, `autorouteItem` `:507-514`;
- `autoroute/pipeline/AutoroutePassRunner.java` — **`runSingleThread` `:151-336` (186)**, `updateProgress` `:489-516` (28);
- `autoroute/RoutingFailureLog.java (161)` — `recordFailure` `:34-48` and `getFailureCount` `:95-101` **ported** (the only two with live callers, `AutoroutePassRunner.java:269, 272`), `ItemFailureInfo` `:109-160` ported as the map value;
- `autoroute/ItemRouteResult.java (145)` — the whole class; the improvement ladder `:39-57`, `improvementPercentage` `:59-65`, `compareTo` `:69-89`;
- `autoroute/pipeline/AutorouteAirlineCalculator.java` — `calculateAirline` `:16-40` (25).
**≈ 456 ported Java lines**; rostered here: `AutoroutePassRunner.runMultiThread` `:40-149` (110), its five `log*` helpers `:338-487` (150), `BatchAutorouter.autoroutePassMultiThread` `:411-413` (3), `RoutingFailureLog.{shouldSkip, shouldGiveUp, getUnroutableItems, hasUnroutableItems, clear, toString}`, `AutorouteAirlineCalculator:42-214` (173, the file is **214** lines — `:42-213` in the first draft left the closing brace unrostered) — **≈ 470 rostered lines, every one with its caller evidence.**

> **Marker dispositions this task owns (scan ruling 2 — Task 17's gate counts every one).**
> `lib.rs:189` `recordFailure` and `:192` `getFailureCount` → **ported**. `:190` `shouldSkip`,
> **`:191` `shouldGiveUp`**, `:193` `getUnroutableItems`, `:194` `hasUnroutableItems`, `:195` `clear`,
> **`:196` `toString`** → **`// not ported:`** with caller evidence (`shouldGiveUp` is at
> `RoutingFailureLog.java:147-149` **inside `ItemFailureInfo`**, not a top-level method — say so on
> the line). `:197` `ItemFailureInfo` → **ported** as the map value.
> `lib.rs:200-211`'s twelve `ItemRouteResult.*` → **all ported**; the class is 145 lines and the
> plan ports "the whole class", so `improvedOver`, `lengthReduced`, `viaCountReduced` and
> `updateImproved` are **not** optional — they were unaccounted for in the first draft.
> `crates/fr-board/src/board/mod.rs:182` (`RoutingBoard.autoroute`, `:911-971`) → **`// renamed:`**
> pointing at this task's `BatchAutorouter::autoroute_item`/`route_connection_full`, which is what
> that method's body is. `crates/fr-board/src/board/mod.rs:214` (`autoroute.RoutingFailureLog`, "the
> real element type") → **`// renamed:`** pointing at `fr_router::pipeline::failure_log`, and the
> existing `Vec<String>` hook `docs/plan-6-handoff.md` §10.4 describes is **deleted or re-pointed in
> the same commit** — say which.

**Interfaces consumed:** `route_connection_full`, `BatchAutorouter` (Task 8); `RouterStop`, `RouterCounters`, `ProgressSink` (Task 4); `BoardStatistics` (Task 1); `fr_drc::DesignRulesChecker`.
**Interfaces produced:**
```rust
// crates/fr-router/src/pipeline/batch_autorouter.rs
impl<'a> BatchAutorouter<'a> {
    /// `getAutorouteItems(RoutingBoard)` (BatchAutorouter.java:345-409) — the pass's work list.
    ///
    /// Walks `board.itemList` **descending** (:351-357, quirk #63), keeps `Connectable`
    /// non-routable items not already in `handledItems` (:357-360), and for each net index
    /// computes `getConnectedSet(netNo)` (:363-365), marking every connected item with
    /// `netCount() <= 1` as handled (:366-370). An item is appended when its connected set is
    /// smaller than `board.connectableItemCount(netNo)` and it has no ignored nets (:375), and
    /// **not** when the net contains a plane and the connected set already holds a
    /// `ConductionArea` (:383-389).
    ///
    /// **Java bug (plan-7 ruling 10):** the same item is appended **once per qualifying net
    /// index** (:390), and `AutoroutePassRunner:202,207` then loops over *every* net index of
    /// each appearance — so a 2-net item that qualifies twice is routed four times per pass, and
    /// the inner index is a net index rather than the one that qualified.
    pub fn autoroute_items(&mut self, board: &mut Board) -> Vec<ItemId>;
}

// crates/fr-router/src/pipeline/pass_runner.rs
/// Port of `autoroute.pipeline.AutoroutePassRunner` (AutoroutePassRunner.java:20-525) — one
/// autoroute pass.
///
/// not ported: `AutoroutePassRunner.runMultiThread` (:40-149) — its only caller is
/// `BatchAutorouter.autoroutePassMultiThread:412`, which has **zero callers** in `src/main` or
/// `src/test` (survey §3.4). No rayon, no threads (ruling AM).
pub struct AutoroutePassRunner;
impl AutoroutePassRunner {
    /// `runSingleThread(int)` (:151-336). Answers Java's `boolean`: `routed > 0 || notRouted > 0`
    /// (:330) — "there is still work to do".
    #[allow(clippy::too_many_arguments)]
    pub fn run_single_thread(
        board: &mut Board, router: &mut BatchAutorouter<'_>, failure_log: &mut RoutingFailureLog,
        pass_no: i32, stop: &RouterStop, budget: RouterBudget,
        progress: &mut dyn ProgressSink,
    ) -> Result<bool, RouterError>;
}

// crates/fr-router/src/pipeline/failure_log.rs
/// Port of `autoroute.RoutingFailureLog` (RoutingFailureLog.java:1-160). Java's
/// `ConcurrentHashMap<Integer, ItemFailureInfo>` (:19) is a `BTreeMap` here: both live readers —
/// `recordFailure` and `getFailureCount` — are keyed lookups, so the iteration-order difference
/// is unobservable (survey §3.1).
///
/// **Ownership note (scan ruling 2).** In Java the log is a **field of `RoutingBoard`** —
/// `AutoroutePassRunner.java:269, 272` reach it as `router.board.failureLog`. The port lifts it to a
/// caller-owned parameter of `run_single_thread` because `fr-board` must not depend on
/// `fr-router`; that is a **deliberate `// renamed:` divergence**, and the `Vec<String>` hook that
/// stands in for it in `fr-board` today goes away in this commit.
pub struct RoutingFailureLog { failures: BTreeMap<ItemId, ItemFailureInfo> }
impl RoutingFailureLog {
    pub fn record_failure(&mut self, item: ItemId, net_no: i32, reason: &str, pass_no: i32); // :34-48
    pub fn failure_count(&self, item: ItemId) -> i32;                                        // :95-101
}
/// Port of the nested `RoutingFailureLog.ItemFailureInfo` (:109-160).
pub struct ItemFailureInfo { pub count: i32, pub last_reason: String, pub last_pass: i32 /* … */ }

// crates/fr-router/src/pipeline/item_route_result.rs
/// Port of `autoroute.ItemRouteResult` (ItemRouteResult.java:1-144) — the optimizer's scorecard.
pub struct ItemRouteResult { /* Read the 145-line file and transcribe the field block verbatim.
                                HEAD has itemId, viaCountBefore/After, traceLengthBefore/After,
                                incompleteCountBefore/After, improved, improvementPercentage —
                                and **no** `minCumulativeTraceLength` field, which the plan's first
                                draft invented (it is `BatchOptimizer`'s local, :288). */ }
impl ItemRouteResult {
    /// `improved()` (:39-57) — the ladder: incompletes, then vias, then trace length.
    pub fn improved(&self) -> bool;
    /// `improvementPercentage()` (:59-65). **Java bug:** `viaCountAfter / viaCountBefore` is
    /// `int / int`, so the via term truncates to 0 or 1 while the trace term is a `double`.
    /// `BatchOptimizer.java:340-348` recomputes the same expression correctly with a `(float)`
    /// cast, so the field is wrong and the used value is right — both are ported.
    pub fn improvement_percentage(&self) -> f64;
    /// `compareTo` (:69-89) — used only by the GUI-only multithreaded optimizer's `PriorityQueue`;
    /// ported for the audit and pinned by a test, with a doc line saying it has no live caller.
    /// The remaining markers — `improvedOver`, `lengthReduced`, `viaCountReduced`, `updateImproved`,
    /// `itemId`, `incompleteCount`, `incompleteCountBefore`, `viaCount`, `traceLength` — are all
    /// **ported**, not rostered (scan ruling 2).
}

// crates/fr-router/src/pipeline/airline.rs
/// `AutorouteAirlineCalculator.calculateAirline(Set<Item>, Set<Item>)`
/// (AutorouteAirlineCalculator.java:16-40, in a 214-line file) — the one method with a live caller
/// (AutorouteConnectionRouter.java:70), whose *value* Plan 6 already rostered `// not ported:`
/// as a GUI progress sink (plan-7 ruling 6).
pub fn calculate_airline(board: &Board, start: &BTreeSet<ItemId>, dest: &BTreeSet<ItemId>)
    -> Option<FloatLine>;
```

**Transcription notes, in `runSingleThread`'s order.**
1. `autorouteItemList = router.getAutorouteItems(board)` (`:158`); **empty → `airLine = null; return false`** (`:163-166`).
2. `progressStatistics = new BoardStatistics(board, null, false)` (`:170`) — `unit = null`, `includeClearanceViolations = false`, and the **four-argument** ctor's `includeConnections` defaults to `true`, so this is a full DRC pass per pass.
3. `tempDrc.calculateAllIncompletes(); routerCounters.incompleteCount = …` (`:188-190`).
4. The item loop (`:202`): `isStopAutoRouterRequested() → break` (`:203-205`); per net index `i` (`:207`): the same check (`:208-210`); **`maxItems` reached → `thread.requestStop()`** (`:212-221` — **`ALL`**, quirk cand. C, Task 4's row); `router.totalItemsRouted++` (`:222`); `board.startMarkingChangedArea()` (`:223`); `rippedItemList = new TreeSet<>()` and `rippedItemCosts = new LinkedHashMap<>()` (`:225-226` — a `BTreeSet<ItemId>` descending and a `Vec`-backed insertion-ordered map, **not** a `BTreeMap`, because `logRippedItems` (**`:361`**) reads it by key only and Task 17 of Plan 6 established both sides may sort by item id).
5. `router.autorouteItem(item, netNo, ripped, costs, passNo)` (`:239-245`) → `BatchAutorouter.java:507-514` → `route_connection_full` (Task 8).
6. The result switch (`:260-289`): `ROUTED → ++routed`; `ALREADY_CONNECTED | NO_UNCONNECTED_NETS | CONNECTED_TO_PLANE → ++skipped`; **everything else** → `board.failureLog.recordFailure(...)`, `++notRouted`.
7. `if (netNo == 94) logNet94Items()` (`:256-258`) — a hard-coded net number gating a 49-line debug dump. `// not ported:` naming the method, and the row already exists as quirk #158's family; **add the `94` to that row rather than opening a new one.**
8. `updateProgress(...)` (`:292-293`) → `RouterCounters` + one `RoutingEvent::BoardUpdated`, gated by `shouldFireBoardUpdate` (Task 8).
9. **Tail removal** (`:298-302`): `removeUnconnectedVias ? removeTails(NONE) : removeTails(FANOUT_VIA)` — Task 8's method.
10. `boardStatistics = board.getStatistics()` (`:309`) and the return (`:330`).
11. **No recovery boundary here** *(scan ruling 9)*. The plan originally placed ruling 7's second boundary at `AutoroutePassRunner.java:144`; that line is the method-level catch of the **dead** `runMultiThread` (`:40-149`), and `runSingleThread` (`:151-336`) has no try/catch. **Do not wrap the per-item body in `catch_unwind`** — a panic propagates out of `run_single_thread`, which is what Java does when an item throws. Record the finding in the commit message with the `sed -n '140,155p'` output that shows which method `:144` closes.

**Tests.**
- `crates/fr-router/tests/pass_runner.rs`: `a_two_net_item_is_routed_four_times` (ruling 10's pin, with the Java visit list from `p7t1`); `the_inner_index_is_a_net_index_not_the_qualifying_one`; `a_plane_net_with_a_conduction_area_is_skipped` (`:383-389`); `an_item_with_ignored_nets_is_skipped` (`:375`); `max_items_requests_stop_all`; `an_empty_item_list_returns_false_without_touching_the_board`; `a_panicking_item_does_not_abort_the_pass` (ruling 7).
- `crates/fr-router/tests/item_route_result.rs`: the ladder's three rungs; `improvement_percentage_truncates_the_via_term` (the Java bug, with the value `BatchOptimizer:340-348` computes beside it); `compare_to_matches_the_jvm` — assert the **ordering** `compareTo` produces over `P7T9Probe`'s 500 scripted tuples (a behavioural test), with the "no live caller" fact stated in its doc comment rather than asserted by a grep.
- **`BatchAutorouterDebugTest`** (ruling 14) ports into `crates/fr-router/tests/pass_runner.rs` with its Java file:line in the doc comment. **It is 521 lines** (`src/test/java/app/freerouting/autoroute/BatchAutorouterDebugTest.java`) — budget for it, and if it does not fit inside this task's ceiling, port the assertions that bear on `getAutorouteItems`/`runSingleThread` here and roster the rest with a `// added in Plan 8:` naming what is left.

**JVM-pinned evidence (required).** Also consumes **`P7T9Probe.java`** (the probe table below) for `ItemRouteResult`'s ladder over 500 scripted tuples.
- **`p7t1 <dsn> <passNo>`** — `P7T1.java`, `package app.freerouting.autoroute.pipeline;`, HEAD jar, JDK 25, `-XX:hashCode=2`, headless flags, budget disabled both sides. Prints, for each entry of `getAutorouteItems`'s list **in order, before any routing**, the item id **and the `netCount()`/net-number list the pass runner will loop over** (`AutoroutePassRunner:207`), plus the `handledItems` set after each net index. **Java's list is `List<Item>`, so the port's `Vec<ItemId>` is the faithful shape** — the net numbers come from the item, not from the list, and the driver derives them the same way on both sides. *(The first draft said the list itself carries `(item, net)` pairs; it does not.)* Acceptance: **0 diffs on all six corpus stems × passes 1-3.**
- **`p7t2 <dsn> <passNo> <maxItems>`** — `P7T2.java`, same package, drives `AutoroutePassRunner.runSingleThread` and prints `p6t1`'s JSONL line per `(item, net)` — attempt state, ripped set, ripup costs, `maxIdBefore`/`maxIdAfter`, every inserted trace polyline and via — **plus a closing line with the tail-removal and `optChangedArea` deltas** (item count, trace count, via count, `structural_hash`-equality pattern). Acceptance: **0 diffs on all six stems × passes 1-3 × `maxItems ∈ {2, all}`.**

**Steps:** `P7T1.java` → `p7t1.rs` → `getAutorouteItems` → `p7t1` 0 diffs → `P7T2.java` → `p7t2.rs` → the failure log, the scorecard, the airline → `runSingleThread` → `p7t2` 0 diffs → roster the five dead methods with fresh grep output in the commit message → fmt/clippy/test/audit → commit `feat(router): the autoroute pass — item selection, the failure log, the scorecard and runSingleThread`.

---

### Task 10: `AutorouteBatchLoop.run` — the pass loop, the best-board policy and the stagnation detector

**Files:** `crates/fr-router/src/pipeline/batch_loop.rs`; `crates/fr-router/tests/batch_loop.rs`; `scripts/differential/java/P7T9.java`, `scripts/differential/rust/src/bin/p7t9.rs` (**first appearance — fanout and optimizer both off**).
**Java:** `autoroute/pipeline/AutorouteBatchLoop.java` — **`run` `:37-588` (552)**, `calculateIncompleteCount` `:590-592`, `removeTails` `:594-596`, `autoroutePass` `:598-600`, `buildUnroutedConnectionsReport` `:602-604`, `fireBoardSnapshotEvent` `:606-608`. **≈ 571 Java lines.** The `PerformanceProfiler.recordConfiguration` block `:68-81` and the per-net incomplete breakdown trace `:378-406` are `// not ported:` (already-rostered profiler / `FRLogger` only).

**Interfaces consumed:** `AutoroutePassRunner::run_single_thread` (Task 9); `BoardHistory` **and `BoardHistory::MAX_HISTORY_SIZE`** (Task 2); `BoardStatistics` (Task 1); `RouterStop`, `TaskState`, `ProgressSink`, **`PassRecord`** (Task 4, scan ruling 6); `BatchAutorouter::{remove_tails, autoroute_items}` (Tasks 8/9); `FanoutRunSummary` (Task 11 — the type only); `BatchFanout::fanout_board` (**Task 12 — stubbed here**); `build_unrouted_report` (**Task 15 — stubbed here**).
**Interfaces produced:**
```rust
// crates/fr-router/src/pipeline/batch_loop.rs
/// Port of `autoroute.pipeline.AutorouteBatchLoop` (AutorouteBatchLoop.java:20-608) — the pass
/// loop, its best-board policy and its two stagnation detectors. **`job.board = router.board` at
/// `:552` is the assignment that decides which board is written to SES.**
pub struct AutorouteBatchLoop;
impl AutorouteBatchLoop {
    /// `run()` (:37-588). Answers Java's `!thread.isStopAutoRouterRequested()` (:587).
    #[allow(clippy::too_many_arguments)]
    pub fn run(
        board: &mut Board, settings: &RouterSettings, stop: &RouterStop,
        budget: RouterBudget, progress: &mut dyn ProgressSink,
    ) -> Result<BatchLoopResult, RouterError>;
}
/// Not a Java type — Java writes its outputs into `job` and `router` fields. `// renamed:` with
/// the five Java sites each field is read from.
pub struct BatchLoopResult {
    pub state: TaskState,               // :571-585 (quirk cand. A)
    pub continue_routing: bool,         // :587
    pub passes_run: i32,                // :520-522
    pub fanout: Option<FanoutRunSummary>, // :123-173
    pub per_pass: Vec<PassRecord>,      // ruling 1(a)
}
/// The five constants, transcribed with their Java lines — every one reaches a `break`.
pub const STOP_AT_PASS_MINIMUM: usize = 8;            // :298
pub const STOP_AT_PASS_MODULO: i32 = 4;               // :299
pub const MAXIMUM_TRIES_ON_THE_SAME_BOARD: i32 = 3;   // :307
pub const BOARD_RANK_LIMIT: usize = BoardHistory::MAX_HISTORY_SIZE;  // :315-320 (= 30)
pub const STAGNATION_PASS_LIMIT: i32 = 10;            // :456
pub const FANOUT_RECOVERY_STAGNATION_PASSES: i32 = 3; // :435
```

**Transcription notes, in `run`'s order** — every line number below was read out of HEAD, and a mistake in any of them changes the pass count:
1. **`anyRoutable`** (`:44-50`): any `layerActive[i] && layers[i].isSignal`. False → `TaskState.CANCELLED` **and throw `IllegalArgumentException`** (`:51-56`, the `throw` itself at `:55`) — ruling 7's **sole** new boundary (scan ruling 9 renumbered it from "seventh"): `RoutingPipeline.run` does **not** catch it, so the port's `run` answers `Err(RouterError::NoRoutableLayer)` and `run_pipeline` propagates. `empty_board.dsn` reaches it.
2. Fire `STARTED` (`:58-59`); `sessionStartTime`, `initialUnroutedCount = DRC incompletes` (`:62-63`).
3. `BoardHistory bh = new BoardHistory(job.routerSettings.scoring)` (`:65`) — Task 2.
4. **The fanout pre-pass** (`:89-173`), behind `settings.isFanoutEnabled()`: skipped when `board.getSmdPins().isEmpty()` (`:90-91`); counts `netConnectedSmdPins`/`alreadyConnectedAtStart` (`:100-110`); `BatchFanout.fanoutBoard(...)` (`:123-172`); `router.fanoutTimedOut = summary.isTimedOut()` (`:173`). **Stubbed in this task** behind an `obligation:` marker naming Task 12, and every `p7t9` run here uses `fanout.enabled = false`.
5. `isRouterEnabled = getRunRouter() && (maxPasses == null || maxPasses >= 0)` (`:221-223`) — note `maxPasses == 0` means **unlimited** (quirk #140).
6. Loop state (`:236-242`): `currentPass = 1`, `consecutiveNoImprovementPasses = 0`, `lastBestScore = globalBestScore = -inf`, `passOfBestScore = 0`.
7. **`Set<String> alreadyRoutedBoardHashes = new HashSet<>()`** (`:249`) — **dead**: both readers (`:259`, `:266`) are commented out, and only `.clear()` survives at `:334`/`:446`. Roster it as a quirk row (cand. B), do **not** port the allocation.
8. **The `while` head** (`:250`): `continueAutorouting && !thread.isStopAutoRouterRequested()`; then `job.state == TIMED_OUT → requestStopAutoRouter()` (`:251-253`, always a no-op in production — Task 4's quirk cand. R); `currentBoardHash = board.getHash()` (`:255`, consumed by nothing since `:257-266` is commented out — record it); `maxPasses > 0 && currentPass > maxPasses → requestStopAutoRouter; break` (`:268-273`, **`AUTO_ROUTER_ONLY`**).
9. `boardScoreBefore = new BoardStatistics(board).getNormalizedScore(scoring)` (`:282-283`); **`bh.add(board)`** (`:284`).
10. `continueAutorouting = autoroutePass(currentPass)` (`:293`) → Task 9.
11. `boardStatisticsAfter` / `boardScoreAfter` (`:295-296`).
12. **The best-board restore** (`:298-344`): `if (bh.size() >= 8 || isStopAutoRouterRequested())` and then `if ((currentPass % 4 == 0 && currentPass >= 8) || isStopAutoRouterRequested())`; **`if (bh.getMaxScore() > boardScoreAfter)`** — a **strict** `>` (`:306`); `boardToRestore = bh.restoreBoard(3)` (`:307`); `null` → "was not able to improve" + `requestStopAutoRouter` + `break` (`:308-313`); `bh.getRank(boardToRestore) > 30` → `requestStopAutoRouter` + `break` (`:315-320`); otherwise — **a fall-through, not an `else`** — `router.board = boardToRestore; consecutiveNoImprovementPasses = 0` (`:322-334`).
13. `saveIntermediateStages → fireBoardSnapshotEvent` (`:408-410`) → `RoutingEvent::BoardSnapshot`.
13a. **The two stagnation reports** (`:456-476` and `:486-507`) call `buildUnroutedConnectionsReport` (`:602-604` → `AutorouteUnroutedReport.build`), which is **Task 15's**. **Stub it here behind `// obligation: AutorouteBatchLoop's stagnation report (:456-476, :486-507) — Task 15`**, returning an empty string, exactly as the fanout pre-pass is stubbed (scan ruling 6). The stub must not change control flow: the report is a log payload, so the `requestStopAutoRouter` + `break` around it is this task's and must be complete.
14. **The stagnation detector** (`:422-517`), all inside `if (currentPass >= 8 && continueAutorouting)`: `boardScoreAfter > lastBestScore + 0.5` → reset (`:425-427`), else `counter++` (`:429`); the **one-shot fanout recovery** (`:435-454`) when `isFanoutEnabled && !applied && incomplete > 0 && counter >= 3` → `removeTails(NONE)`, recompute the score, `counter = 0`; `counter >= 10` → report + `requestStopAutoRouter` + `break` (`:456-476`); the **global** tracker `after > globalBest + 0.5` → record the pass (`:482-485`), else `(currentPass - passOfBestScore) >= 10` → report + stop (`:486-507`); **and the `else if (incompleteCount == 0 && score > 0.5) counter = 0` at `:509-517`, which is attached to the `currentPass >= 8` guard — so on passes 1-7 a fully-routed board resets the counter and on pass ≥ 8 it does not, the opposite of the comment at `:511-514`** (quirk cand. N, `// Java bug:`).
15. `currentPass++` only when `continueAutorouting && !isStopAutoRouterRequested()` (`:520-522`).
16. **The final best-board swap** (`:528-550`): `currentFinalScore = BoardStatistics(board).getNormalizedScore`; `if (bh.getMaxScore() > currentFinalScore) { bestBoard = bh.restoreBestBoard(); if (bestBoard != null) router.board = bestBoard; }`.
17. **`job.board = router.board`** (`:552`) — the assignment that decides the SES.
18. `if (wasRouterRun && !(removeUnconnectedVias || continueAutorouting || isStopAutoRouterRequested())) removeTails(NONE)` (`:554-563`).
19. `bh.clear()` (`:565-569`).
20. **`TaskState` reporting** (`:571-585`): `FINISHED` only when `!isStopAutoRouterRequested()`, else `TIMED_OUT`/`CANCELLED`. **Every ordinary exit sets the flag** — max passes (`:271`), stagnation (`:474`, `:505`), "not able to improve" (`:311`), rank limit (`:318`) — so a **normal** end of routing reports `CANCELLED`, not `FINISHED` (quirk cand. A, `// Java bug:` + row). The return value (`:587`) has the same shape.

**Tests (`crates/fr-router/tests/batch_loop.rs`):**
- `a_board_with_no_signal_layer_errors_and_reports_cancelled` (`:44-56`, ruling 7).
- `a_normal_finish_reports_cancelled_not_finished` (quirk cand. A — the headline row).
- `max_passes_zero_is_unlimited` (quirk #140).
- `the_restore_gate_needs_eight_entries_and_a_pass_multiple_of_four`.
- `the_rank_limit_breaks_the_loop` — a scripted `BoardHistory` where `rank > 30`.
- `the_stagnation_counter_resets_only_below_pass_eight_for_a_routed_board` (quirk cand. N).
- `the_final_swap_takes_the_best_board_when_it_is_strictly_better`.
- `the_dead_hash_set_is_javas_only_allocation` — a **roster assertion**, and its doc comment says so: it greps `batch_loop.rs` for the `// not ported: alreadyRoutedBoardHashes` line. There is nothing behavioural to assert (the set has no live reader in Java either), which is exactly why the row exists; do not dress it up as a behaviour test.

**JVM-pinned evidence (required).** **`p7t9 <dsn> <maxPasses> <mode>`**, first appearance, **mode `router-only`** (`fanout.enabled = false`, `runOptimizer = false`). `P7T9.java` drives the jar's own `RoutingPipeline.createForHeadless` path with those settings, dumps the **per-pass tuple** (`PassRecord`) after every pass and the final board in `P6T15aProbe`'s polyline format; `p7t9.rs` prints the same from `AutorouteBatchLoop::run`. Both sides run with `RouterBudget::disabled()` (the Java half via the reflective property its header documents) and `-XX:hashCode=2`. Acceptance for **this** task: **0 diffs on `router-rpi-splitter`, `router-j2-reference` and `router-ecc83-input` at `maxPasses ∈ {1, 2, 8}`**, and the per-pass tuple identical on `router-dac2020-bm01` at `maxPasses = 2` (its full run is Task 16's, behind `FR_SLOW_PARITY=1`).

**Steps:** `P7T9.java` mode `router-only` → `p7t9.rs` → tests → implement → `p7t9` 0 diffs on three stems → fmt/clippy/test/audit → commit `feat(router): AutorouteBatchLoop's pass loop, best-board policy and stagnation detectors`.

---

### Task 11: `RoutingBoard.fanout` and `BatchFanout`'s component/pin ordering

**Files:** `crates/fr-router/src/board_ext/routing_board_ext.rs`, `crates/fr-router/src/pipeline/fanout.rs`, `crates/fr-board/src/board/mod.rs:183` (the `// added in Plan 7: RoutingBoard.fanout` marker becomes `// renamed:`); `crates/fr-router/tests/fanout_order.rs`; `scripts/differential/java/P7T5.java`, `scripts/differential/rust/src/bin/p7t5.rs`.
**Java:**
- `board/facade/RoutingBoard.java` — **`fanout` `:978-1110` (133)**;
- `autoroute/pipeline/BatchFanout.java` (**780 lines**) — the `sortedComponents` field `:25` and its 1-based fill `:53-61` (the plan's first draft put the field at `:52`), the ctor `:35-78` (44), `fanoutBoard` ×2 `:81-163` (83), `EscapeStatistics` `:591-606` (16), `FanoutPassStatus` `:609-622` (14), `FanoutRunSummary` `:625-629` (5), `FanoutProgressListener` `:578-583` (6, `// not ported:` → `ProgressSink`), **`Component` `:631-693` (63)** with its ctor `:642-678`, its `TreeSet<Pin>` at `:673` and its `compareTo` `:682-693`, **`Component.Pin` `:695-778` (84)** with `Double.MAX_VALUE` at `:711` and its `compareTo` `:742-777`.
**≈ 459 Java lines.**

**Interfaces consumed:** `route_connection_full`, `RoutingBoardExt::opt_changed_area` (Tasks 5/8); `ViaRule` (Task 0 — `fanout` builds a **combined** rule at run time, which is exactly the path ruling H was measured on); **`AutorouteControl::{from_settings, rebuild_via_info}`** (Plan 6 Task 8); `Board::structural_hash` (Task 3); `BoardStatistics::is_pin_escaped` (Task 1).

> **Scan ruling 4 — `AutorouteControl::new_for_net` does not exist.** The plan's first draft named it. HEAD's constructors are `AutorouteControl::new(board, net_no, settings, via_costs, trace_costs)` (`:181`) and **`AutorouteControl::from_settings(…)`** (`:202`), and `from_settings` carries a `// pub seam:` line reading *"Plan 7's fanout pre-pass"* — i.e. it was built for this call site. **Consuming it discharges that `pub seam:` marker**; say so in the commit message. The same applies to **`AutorouteAttemptResult::is_routed`**, whose `// pub seam:` names *"Plan 7's pass loop"* — Tasks 9 and 10 consume it and discharge it. Two of Plan 6's eight `pub seam:` markers are therefore Plan 7's to close; Task 17 checks that `grep -rn "pub seam:" crates/fr-router/src` is down to six with the two closures recorded.
**Interfaces produced:** `RoutingBoardExt::fanout`, **the `BatchFanout` struct and its constructor** (scan ruling 7 — Task 12 adds `impl` blocks only), `FanoutComponent`, `FanoutPin`, `EscapeStatistics`, `FanoutPassStatus`, `FanoutRunSummary`.
```rust
// crates/fr-router/src/board_ext/routing_board_ext.rs
pub trait RoutingBoardExt {
    /// `RoutingBoard.fanout(Pin, RouterSettings, int, Stoppable, TimeLimit)`
    /// (RoutingBoard.java:978-1110) — the per-pin escape router `BatchFanout` calls.
    fn fanout(&mut self, engine: &mut Option<AutorouteEngine>, pin: ItemId,
              settings: &RouterSettings, ripup_costs: i32, stop: StopCheck<'_>,
              time_limit: Option<TimeLimit>, budget: RouterBudget) -> AutorouteAttemptResult;
}

// crates/fr-router/src/pipeline/fanout.rs
/// Port of `BatchFanout.Component` (BatchFanout.java:631-693). `compareTo` (:682-693) is
/// **pin count descending, then `boardComponent.id` ascending** — total and deterministic, and
/// both keys are `final`, so a `BTreeSet` is safe (ruling 5's recorded decision).
pub struct FanoutComponent { pub component: ComponentId,
                             /// **`JavaTreeSet`, not `BTreeSet`** — scan ruling 8. `Pin.compareTo`
                             /// (:742-777) picks its comparison key from
                             /// `settings.fanout.pinSortingOrder` **at run time** and returns `0`
                             /// for an unrecognised string, so the ordering is neither a
                             /// compile-time `Ord` nor necessarily a total order. This is exactly
                             /// controller ruling Y's case; ruling 5's "BTreeSet-safe" hypothesis
                             /// **does not survive** and this is its recorded answer.
                             pub smd_pins: JavaTreeSet<FanoutPin>,
                             pub gravity_centre: FloatPoint, pub smd_pin_count: usize }
/// Port of `BatchFanout.Component.Pin` (:695-778). `compareTo` (:742-777) compares a `double`
/// selected by `settings.fanout.pinSortingOrder`, tie-broken by `boardPin.pinIndex`.
/// `distanceToClosestOnNet` is `Double.MAX_VALUE` when the net has no other pin (:711), and an
/// **unrecognised sorting-order string leaves `result = 0`**, so the order becomes pure
/// `pinIndex` — quirk row. Transcribe the `<`/`>` chain literally, never `total_cmp`.
pub struct FanoutPin { pub pin: ItemId, pub pin_index: usize,
                       pub distance_to_gravity_centre: f64, pub distance_to_closest_on_net: f64 }
/// Port of `autoroute.pipeline.BatchFanout` (BatchFanout.java:24-779) — the SMD escape pre-pass.
/// **Declared here** (scan ruling 7) because this task ports its constructor (`:35-78`) and the
/// `fanoutBoard` skeleton; Task 12 adds `impl` blocks to it and declares nothing.
pub struct BatchFanout<'a> {
    /// `sortedComponents` (field :25, filled 1-based over `board.components` at :53-61).
    sorted_components: BTreeSet<FanoutComponent>,
    settings: &'a RouterSettings,
    /* … the rest of the ctor's fields, :35-78, transcribed … */
}
impl<'a> BatchFanout<'a> {
    /// The constructor (:35-78).
    pub fn new(board: &Board, settings: &'a RouterSettings) -> BatchFanout<'a>;
}

/// Port of `BatchFanout.EscapeStatistics` (:591-606), `FanoutPassStatus` (:609-622) and
/// `FanoutRunSummary` (:625-629).
pub struct EscapeStatistics { pub total_smd_pins: i32, pub escaped_count: i32,
                              pub pins_to_escape: i32 /* … */ }
pub struct FanoutPassStatus { /* … */ }
pub struct FanoutRunSummary { pub timed_out: bool, pub passes_run: i32,
                              pub escape: EscapeStatistics /* … */ }
impl EscapeStatistics {
    /// `fromBoardStatistics(BoardStatistics)` (:594-600).
    pub fn from_board_statistics(stats: &BoardStatistics) -> EscapeStatistics;
}
```

**Transcription notes for `RoutingBoard.fanout` (`:978-1110`), branch by branch:**
- the single-layer/one-net pin guard `:984-996` and the unconnected-set guard `:997-1001`;
- **targets sorted by squared distance from the pin centre using bounding-box midpoints** `:1002-1021` — a `double` sort whose ties Java breaks by list order; transcribe with a **stable** sort;
- the **3-argument** `AutorouteControl(board, netNo, settings)` ctor (`AutorouteControl.java:117-120`) at `:1023`, then `isFanout = true` `:1024`;
- **the `fallbackToBoardVias` combined `ViaRule`** `:1025-1044`: a fresh `ViaRule` holding the net class's vias **plus** `rules.viaRules.firstElement()`'s, then `rebuildViaInfo` on it. **This is the run-time-synthesised rule that makes Task 0 a prerequisite** — with an index model the synthetic rule's `ViaInfo`s would re-point.
- `fanoutStartPinName/Center/Layer` `:1045-1052`; `removeUnconnectedVias = false` `:1053`; ripup `:1054-1057`; `initAutoroute(..., retain = false)` `:1059-1061`;
- **the ≤ 4-target two-attempt strategy** `:1064-1085`: closest target first, then the whole unconnected set — **with the *same* `rippedItemList`**, so items ripped by the failed first attempt are carried into the second attempt's accounting and reported to `BatchFanout` as this pin's rips (quirk cand. Q, `// Java bug:` + row); vs the > 4-target single attempt `:1086-1092`;
- `null → FAILED "No target items to route connection."` `:1093-1097`;
- `optChangedArea(new int[]{pinNetNo}, null, accuracy, traceCosts, thread, 1000)` on `ROUTED` `:1099-1108` — note the **net-filtered** `int[]{pinNetNo}` here, unlike step 6's empty array.

**Transcription notes for the ordering:** the `Component` ctor (`:642-678`) computes the gravity centre and builds the `TreeSet<Pin>` (`:673`); `sortedComponents` is a `TreeSet<Component>` over `board.components` **1-based, `1..=count()`** (field `:25`, fill `:53-61`).

**Ruling 5's recorded answer for both containers (scan ruling 8), so the implementer confirms rather than decides:**
- `sortedComponents` — `Component.compareTo` (`:682-693`) is pin count **descending** then `boardComponent.id` **ascending**, both `final` and both total. **`BTreeSet<FanoutComponent>` is safe.**
- `Component.smdPins` — `Pin.compareTo` (`:742-777`) selects its `double` key from `settings.fanout.pinSortingOrder` at run time and ~~**falls through with `result = 0`** for an unrecognised string~~ **— corrected by Task 11: `:773-775`'s `pinIndex` tie-break is outside the chain, so an unrecognised string gives *pure `pinIndex`* order and `0` is reachable only for two pins of one component at the same `pinIndex`. The `JavaTreeSet` decision below stands; its reason does not. Quirk #220.** A run-time-keyed comparator cannot be a static `Ord`, and a comparator that can return `0` for two distinct pins is **not** a total order over the element set — `std`'s `BTreeSet` would *drop* the second pin where `java.util.TreeSet` also drops it but may order the survivors differently, and neither is safe to assume. **Use `JavaTreeSet<FanoutPin>` with the comparator transcribed as Java's `<`/`>` chain, carrying the settings.** A test pins the drop: `an_unrecognised_sorting_order_collapses_pins_with_equal_pin_index`.

If the implementer's own reading of `:742-777` contradicts this, that is a **Java-wins report**, not a licence to switch back to `BTreeSet` quietly.

> **Task 11's interface sketch above is corrected by HEAD in four more places** (report §2.2-§2.6):
> `FanoutComponent.component` is an `i32` (there is no `ComponentId` type in this tree);
> `FanoutPin` has **four** sort keys — `surroundingsDensity` (`:700`, `:725-738`) is an `int`
> and one of the four `pinSortingOrder` branches (`:765-771`); `EscapeStatistics`' third
> component is `escapedPercentage`, not `pinsToEscape` (which belongs to
> `BoardStatistics.BoardStatisticsFanout`); and `FanoutRunSummary` has four components, the
> sketch's three plus `totalDurationMillis`.

**Tests (`crates/fr-router/tests/fanout_order.rs`):** `components_sort_by_pin_count_descending_then_id`; `pins_sort_by_the_selected_double_then_pin_index`; `an_unrecognised_sorting_order_falls_back_to_pin_index` and `an_unrecognised_sorting_order_collapses_pins_with_equal_pin_index` (the quirk and the `JavaTreeSet` drop, scan ruling 8); `a_net_with_one_pin_gets_double_max_value`; `the_four_target_strategy_carries_the_ripped_set_into_the_second_attempt` (quirk cand. Q); `the_combined_via_rule_holds_both_sources` (Task 0's payoff); `targets_are_sorted_by_squared_midpoint_distance_stably`.

**JVM-pinned evidence (required).** **`p7t5 <dsn> <passNo> <sortingOrder> <mode>`** — `P7T5.java`, `package app.freerouting.autoroute.pipeline;`, HEAD jar, JDK 25, `-XX:hashCode=2`, headless flags, budget disabled both sides. **Mode `order`** prints `sortedComponents × smdPins` as `(componentId, pinCount)` / `(pinId, pinIndex, distToCentre, distToClosestOnNet)` with `Double.toString`, for each of the four `pinSortingOrder` strings **plus one unrecognised string**. **Mode `pin`** calls `RoutingBoard.fanout` on each SMD pin in that order and prints the attempt result, the ripped set and the inserted geometry. Acceptance: **0 diffs on `Issue730-DAC2020_bm11.dsn`, `Issue558-dev-board.dsn`, `Issue508-DAC2020_bm06.dsn` and the six corpus stems, both modes.**

**Steps:** `P7T5.java` modes `order`/`pin` → `p7t5.rs` → tests → implement the ordering → `p7t5 order` 0 diffs → implement `RoutingBoard.fanout` → `p7t5 pin` 0 diffs → fmt/clippy/test/audit → commit `feat(router): RoutingBoard.fanout and BatchFanout's component/pin ordering`.

---

### Task 12: `BatchFanout` — the pass loop, the oscillation and hash stops, and `fanoutBoard`

**Files:** `crates/fr-router/src/pipeline/fanout.rs`, `crates/fr-router/src/pipeline/batch_loop.rs` (discharge Task 10's stub); `crates/fr-router/tests/fanout.rs`; `scripts/differential/java/P7T5.java` (+ modes `pass`/`board`), `scripts/differential/rust/src/bin/p7t5.rs`.
**Java:** `autoroute/pipeline/BatchFanout.java` — **`fanoutPass` `:166-506` (341)**, `maybePublishProgress` `:508-542` (35), `publishProgress` `:544-576` (33), and the `fanoutBoard` pass loop **`:110-157`** (48, whose skeleton Task 11 delivered; `:158-162` is the post-loop `EscapeStatistics` build, already cited separately, so the first draft's `:110-162` double-counted it). **≈ 404 Java lines.**

**Interfaces consumed:** everything Task 11 produced — **including the `BatchFanout` struct itself and its constructor** (scan ruling 7); `Board::structural_hash` (Task 3); `RouterStop`, `RouterBudget`, `ProgressSink` (Task 4).
**Interfaces produced:**
```rust
// crates/fr-router/src/pipeline/fanout.rs
// The `BatchFanout` struct and its constructor are **Task 11's** (scan ruling 7). This task adds
// an `impl` block and declares no type.
impl<'a> BatchFanout<'a> {
    /// `fanoutBoard(RoutingBoard, RouterSettings, Stoppable, FanoutProgressListener)`
    /// (:81-163) — both overloads collapse into one (`// renamed:`), the listener becoming
    /// `&mut dyn ProgressSink` (ruling AK). An associated fn, so call it as
    /// `BatchFanout::fanout_board(…)` and let `'a` be inferred from `settings`.
    pub fn fanout_board(board: &mut Board, settings: &'a RouterSettings, stop: &RouterStop,
                        budget: RouterBudget, progress: &mut dyn ProgressSink)
        -> Result<FanoutRunSummary, RouterError>;
    /// `fanoutPass(int, FanoutProgressListener)` (:166-506) — answers Java's `routedCount`.
    fn fanout_pass(&mut self, board: &mut Board, pass_no: i32, stop: &RouterStop,
                   budget: RouterBudget, progress: &mut dyn ProgressSink)
        -> Result<i32, RouterError>;
}
```

**Transcription notes.**
- **`fanoutBoard`'s pass loop** (`:110-157`), in order: deadline reached → `isTimedOut = true; break` (`:111-116`, ruling AI's site); `fanout.maxItems` reached → `break` (`:117-122`); `routedCount = fanoutPass(i, listener)` (`:123`); `routedCount == 0 → break` (`:125-127`); **the oscillation detector** `boardState = ((long) routedCount << 32) ^ board.getVias().size()`, identical **three passes in a row** → `break` (`:128-148` — quirk cand. O: the XOR collides for large via counts, and two passes with the same `(routedCount, viaCount)` are treated as identical progress even when the geometry differs; `// Java bug:` + row); `isTimedOut → break` (`:149-151`); **`board.getHash()` unchanged since the last pass → break** (`:152-156` — ruling AH's first decision site, and the reason Task 3 precedes this one); then `EscapeStatistics.fromBoardStatistics(new BoardStatistics(board, null, false))` (`:158-162`).
- **`deadlineMs = start + parseTimespanString(settings.fanout.timeoutString)`** (`:94-100`) and **`maxPasses = settings.fanout.maxPasses ?? 20`** (`:101-104`) — both `fr-settings` values; the deadline is `RouterStop`'s (ruling AI), not a second clock.
- **`fanoutPass`** (`:166-506`): `ripupCosts = settings.getStartRipupCosts() * (passNo + 1)` (`:173`); `baseMillisPerPin = settings.fanout.maxMillisecondsPerPin ?? 10000` (`:175-178`, `RouterBudget::fanout_ms_per_pin`); `effectiveRipupCosts = fanout.ripupAllowed ? ripupCosts : -1` (`:179-183`); then `for component in sortedComponents { for pin in component.smdPins { … } }` (`:220-221`) with, per pin: the `maxItems` gate (`:222-230`), `TimeLimit(baseMillisPerPin * (passNo + 1))` (`:231-232`), **the `canUseVias` gate** (`:238-259`) — `netClass.getViaRule().viaCount() > 0 || (fanout.fallbackToBoardVias && rules.viaRules.first().viaCount() > 0)`, else `--pinsToGo; continue` — `board.startMarkingChangedArea()` (`:279`), `board.fanout(pin, settings, effectiveRipupCosts, thread, timeLimit)` (`:281-283`, Task 11), the five-way counter switch (`:286-381`), the deadline check → `isTimedOut; return routedCount` (`:396-415`), and `thread.isStopAutoRouterRequested() → return routedCount` (`:416-433`).
- **Quirk cand. P** at `:239-249`: `hasBoardVias` reads `rules.viaRules.firstElement().viaCount() > 0`, but when `net == null` (`:239`) the **whole gate is skipped** and the pin is fanned out with whatever `AutorouteControl` derives — the null-net path has no via check at all. `// Java bug:` + row.
- `maybePublishProgress`/`publishProgress` (`:508-576`) become one `RoutingEvent::FanoutProgress`; the 1 000 ms `ProgressThrottler` gate is `RouterBudget::progress_throttle_ms`.
- **Discharge Task 10's `obligation:` marker**: `AutorouteBatchLoop.run`'s fanout pre-pass (`:89-173`) now calls the real `fanout_board`, and `router.fanoutTimedOut = summary.isTimedOut()` (`:173`) is wired.

**Tests (`crates/fr-router/tests/fanout.rs`):** `an_empty_smd_pin_set_skips_the_pre_pass`; `zero_routed_pins_ends_the_loop`; `three_identical_board_states_end_the_loop` (the oscillation detector, with a scripted state sequence); `an_unchanged_hash_ends_the_loop` (ruling AH's site); `a_null_net_pin_skips_the_via_gate` (quirk cand. P); `ripup_costs_scale_with_the_pass_number`; `the_fanout_recovery_in_the_batch_loop_fires_once` (Task 10's `:435-454`, now reachable).

**JVM-pinned evidence (required).** `p7t5` **mode `pass`** (one `fanoutPass` end to end: per pin the result, the counters, the changed-area delta) and **mode `board`** (the whole `fanoutBoard`, with the per-pass `(routedCount, viaCount, hash-equality decision, isTimedOut)` line). Acceptance: **0 diffs on `Issue730-DAC2020_bm11.dsn` (the only fanout-trace/escape-rate fixture), `Issue558-dev-board.dsn`, `Issue508-DAC2020_bm06.dsn` and the six corpus stems**; then **`p7t9` mode `router+fanout` 0 diffs** on `Issue730-DAC2020_bm11.dsn` and `Issue558-dev-board.dsn`, and `p7t9` mode `router-only` still 0 diffs on Task 10's three stems.

**Steps:** `P7T5.java` modes `pass`/`board` → tests → implement `fanoutPass` → `p7t5 pass` 0 diffs → implement the pass loop and the two stops → `p7t5 board` 0 diffs → discharge Task 10's stub → `p7t9 router+fanout` 0 diffs → fmt/clippy/test/audit → commit `feat(router): BatchFanout's pass loop, its oscillation and hash stops, and the escape statistics`.

---

### Task 13: `BatchOptimizer` part A — `ReadSortedRouteItems`, `optRouteItem` and the clone-based snapshot

**Files:** `crates/fr-router/src/pipeline/optimizer.rs`, `crates/fr-router/src/pipeline/batch_autorouter.rs` (`autoroutePassesForOptimizingItem`); `crates/fr-router/tests/optimizer_items.rs`; `scripts/differential/java/P7T8.java`, `scripts/differential/rust/src/bin/p7t8.rs`.
**Java:**
- `autoroute/pipeline/BatchOptimizer.java` — `containsOnlyUnfixedTraces` `:85-92` (8), **`optRouteItem` `:395-514` (120)**, `getCurrentPosition` `:520-525` (6), **the protected inner `ReadSortedRouteItems` `:563-659` (97)** with its ctor `:568-571` and its `next()` `:573-654`; the file is **660** lines;
- `autoroute/pipeline/BatchAutorouter.java` — **`autoroutePassesForOptimizingItem` `:245-281` (37)**.
**≈ 268 Java lines.**

**Interfaces consumed:** `AutoroutePassRunner::run_single_thread` and `BatchAutorouter` (Tasks 8/9); `ItemRouteResult` (Task 9); `BoardStatistics` (Task 1); `RouterStop` (Task 4); `Board::{deep_copy, remove_items, combine_traces, connection_items}` (Plan 2).
**Interfaces produced:**
```rust
// crates/fr-router/src/pipeline/optimizer.rs
/// Port of the protected inner class `BatchOptimizer.ReadSortedRouteItems`
/// (BatchOptimizer.java:563-659). **A full O(n) rescan of `board.itemList` per returned item,
/// twice** — vias first (:577-604), then traces (:606-650) sharing `currentMinCoor`, which is why
/// a via wins a tie with a trace at the same coordinate (the comment at :605 says so). It reads
/// board state that the previous `optRouteItem` mutated, so **it must not be memoised**
/// (plan-7 ruling 12).
pub struct ReadSortedRouteItems { min_coor: FloatPoint, min_layer: usize }
impl ReadSortedRouteItems {
    pub fn new() -> ReadSortedRouteItems;                                    // :568-571
    /// `next()` (:573-654) — the lexicographically-next `(x, y, layer)` strictly after the
    /// cursor. The comparison is on `FloatPoint` `double`s (:587-596, :624-632); transcribe the
    /// `<`/`>` chain literally, `java_min`/`java_max` where Java calls `Math.*`.
    ///
    /// **The struct above shows only the two cursor fields the plan could name;** Java keeps
    /// `currentMinCoor`/`currentMinLayer` **and** the previously-returned position that "strictly
    /// after" is measured from. Read `:566-571` (the ctor is `:568-571`) and transcribe the real
    /// field set.
    pub fn next(&mut self, board: &Board) -> Option<ItemId>;
}

/// Port of `autoroute.pipeline.BatchOptimizer` (BatchOptimizer.java:26-659, in a 660-line file) —
/// the optimizer stage. **Declared here** (scan ruling 7) because this task writes `&mut self`
/// methods on it; Task 14 adds `run_batch_loop`/`opt_route_pass` as an `impl` extension and
/// declares nothing.
pub struct BatchOptimizer<'a> { /* the field block :29-38, transcribed */ }
impl<'a> BatchOptimizer<'a> {
    /// `createForHeadless(RoutingJob, ...)` (:51-53) — `// renamed:` to `new`.
    pub fn new(settings: &'a RouterSettings) -> BatchOptimizer<'a>;
}

impl BatchOptimizer<'_> {
    /// `containsOnlyUnfixedTraces(Collection<Item>)` (:85-92).
    pub(crate) fn contains_only_unfixed_traces(board: &Board, items: &BTreeSet<ItemId>) -> bool;
    /// `optRouteItem(Item, boolean, boolean)` (:395-514) — rip one item's connections out and
    /// re-route them, keeping the result only if `ItemRouteResult::improved`.
    ///
    /// `disable_snapshots` is Java's parameter (:396); it is `false` from `optRoutePass:331` and
    /// `true` only from `OptimizeRouteTask:46`, which is GUI-only — so it is `false` on every
    /// path this port has, and the parameter is kept so the audit sees the method.
    pub(crate) fn opt_route_item(&mut self, board: &mut Board, item: ItemId,
                                 with_preferred_directions: bool, disable_snapshots: bool,
                                 stop: &RouterStop, budget: RouterBudget)
        -> Result<ItemRouteResult, RouterError>;
}

// crates/fr-router/src/pipeline/batch_autorouter.rs
impl<'a> BatchAutorouter<'a> {
    /// `autoroutePassesForOptimizingItem(RoutingJob, int, int, int, boolean, RoutingBoard,
    /// RouterSettings)` (BatchAutorouter.java:245-281) — the optimizer's own autorouter: a fresh
    /// `BatchAutorouter` with **`removeUnconnectedVias = true` unconditionally** (:253-261),
    /// `isOptimizerAutorouter = true` (:263), the pass loop `while (stillUnrouted &&
    /// !isStopAutoRouterRequested() && pass <= maxPassCount)` (:267-275) and a final
    /// `removeTails(NONE)` (:276).
    #[allow(clippy::too_many_arguments)]
    pub fn autoroute_passes_for_optimizing_item(
        board: &mut Board, settings: &RouterSettings, max_pass_count: i32, ripup_costs: i32,
        trace_pull_tight_accuracy: i32, with_preferred_directions: bool,
        stop: &RouterStop, budget: RouterBudget, progress: &mut dyn ProgressSink,
    ) -> Result<(), RouterError>;
}
```

**Transcription notes for `optRouteItem` (`:395-514`), in order:**
1. `statsBefore = new BoardStatistics(board, null, false)`; `incompleteCountBefore` from the DRC (`:404-406`).
2. `rippedItems = new TreeSet<>(); rippedItems.add(item)` (`:412-413`); for a `Trace`, add `getStartContacts()`/`getEndContacts()` **when `containsOnlyUnfixedTraces(contactList)`** (`:416-425`).
3. `rippedConnections = new TreeSet<>()`, unioned with `currentItem.getConnectionItems(StopConnectionOption.NONE)` (`:428-432`).
4. **Any `isUserFixed()` → return unimproved** (`:436-440`) — the early exit that protects fixed geometry.
5. `!disableSnapshots → routingBoard.generateSnapshot()` (`:442-445`) → **ruling 8: `let snapshot = board.deep_copy();`**.
6. `routingBoard.removeItems(rippedConnections)` (`:448`); then `combineTraces(netNo)` **for each net of the item** (`:449-451`).
7. `ripupCosts = settings.getStartRipupCosts()` × `optimizer.additionalRipupCostFactorAtStart` when `useIncreasedRipupCosts` (`:454-457`) × `optimizer.traceRipupCostFactor` **rounded** when the item is a `Trace` (`:460-463`, `java_round`).
8. `BatchAutorouter.autoroutePassesForOptimizingItem(...)` (`:466-473`).
9. `statsAfter`, `incompleteCountAfter` (`:477-479`); build `ItemRouteResult(itemId, viaBefore, viaAfter, minCumulativeTraceLength, traces.totalLength, incompleteBefore, incompleteAfter)` (`:485-493`).
10. `routeImproved = !thread.isStopRequested() && result.improved()` (`:494-495`) — note the **`ALL`** check here, not the auto-router one.
11. Improved → `minCumulativeTraceLength = min(…, totalWeightedLength)`, `popSnapshot()` (`:497-504`) → **drop the clone**; not improved → `routingBoard.undo(null)` (`:505-511`) → **restore from the clone**.
12. **Quirk cand. I**, at `BatchAutorouter.java:271-273`: `if (stillUnroutedItems && !isStopAutoRouterRequested() && updatedRoutingBoard == null) {}` — an **empty `if` body**, and `updatedRoutingBoard` is dereferenced unconditionally at `:256`, so the `== null` test can never be true without an earlier NPE. Roster + quirk row; the port omits the branch and says so.

**Transcription notes for `ReadSortedRouteItems.next()` (`:573-654`):** two loops over `board.itemList`, both walking **descending item id** (quirk #63) and both selecting the strictly-greater-than-cursor minimum in `(x, y, layer)` order; the trace loop runs **second** with the **same** `currentMinCoor`, so a via at the identical coordinate wins. Coordinates come from `getCurrentPosition` (`:520-525`) — a `FloatPoint`, hence `f64` comparisons; use the literal `<`/`>` chain. The cursor advances only when an item is returned.

**Tests (`crates/fr-router/tests/optimizer_items.rs`):**
- `the_item_sequence_matches_the_jvm` — the full sequence from `p7t8`, as literals.
- `a_via_wins_a_tie_with_a_trace_at_the_same_coordinate` (`:605`).
- `the_rescan_sees_items_the_previous_call_moved` (ruling 12's pin: mutate the board between two `next()` calls and assert the sequence changes as Java's does).
- `a_user_fixed_contact_returns_unimproved_without_touching_the_board` (`structural_hash` equality).
- `an_unimproved_item_restores_the_clone_byte_for_byte` (ruling 8).
- `the_trace_ripup_cost_factor_is_rounded_java_style`.
- `the_optimizer_autorouter_always_removes_unconnected_vias` (`:253-261`).

**JVM-pinned evidence (required).** **`p7t8 <dsn> <mode>`** — `P7T8.java`, `package app.freerouting.autoroute.pipeline;` (to reach the protected inner class without reflection), HEAD jar, JDK 25, `-XX:hashCode=2`, headless flags, budget disabled both sides. **Mode `sequence`** prints the full `ReadSortedRouteItems.next()` sequence of one optimizer pass — `(itemId, kind, x, y, layer)` with `Double.toString` — including the items produced *after* board mutations. **Mode `item`** drives `optRouteItem` per item and prints the `ItemRouteResult` fields, the ripped set, the improved flag and the board dump. Acceptance: **0 diffs on all six corpus stems, both modes.**

**Steps:** `P7T8.java` → `p7t8.rs` → tests → implement `ReadSortedRouteItems` → `p7t8 sequence` 0 diffs → implement `optRouteItem` + `autoroutePassesForOptimizingItem` with the clone substitute → `p7t8 item` 0 diffs → fmt/clippy/test/audit → commit `feat(router): the optimizer's item sequence, optRouteItem and the clone-based snapshot`.

---

### Task 14: `BatchOptimizer` part B — `runBatchLoop`, `optRoutePass`, and the multithread roster

**Files:** `crates/fr-router/src/pipeline/optimizer.rs`, `crates/fr-router/src/lib.rs` (the roster); `crates/fr-router/tests/optimizer.rs`; `scripts/differential/rust/src/bin/p7t9.rs` (mode `optimizer`).
**Java:** `autoroute/pipeline/BatchOptimizer.java` — the fields `:29-38` (10), the ctor `:45-48` (4), `createForHeadless` `:51-53` (3), `normalizeAlgorithm` `:68-78` (11), `isTimedOut` `:81-83` (3), **`runBatchLoop` `:125-272` (148)**, **`optRoutePass` `:279-385` (107)**, the five `NamedAlgorithm` overrides `:527-550` (24), `calculateIncompleteCount` `:552-556` (5). **≈ 315 Java lines.** Rostered here: `createForGui` `:56-66` (11, GUI), the three JMX samplers `:94-122` (29), `autoroute/pipeline/BatchOptimizerMultiThreaded.java` (366) and `autoroute/pipeline/OptimizeRouteTask.java` (100) — **≈ 506 rostered lines.**

**Interfaces consumed:** everything Task 13 produced; `BoardStatistics` (Task 1); `RouterStop`, `TaskState`, `ProgressSink` (Task 4).
**Interfaces produced:**
```rust
// crates/fr-router/src/pipeline/optimizer.rs
/// Port of `autoroute.pipeline.BatchOptimizer` (BatchOptimizer.java:26-659) — the optimizer
/// stage. `createForHeadless` (:51-53) **always** returns the single-threaded implementation;
/// `createForGui` (:56-66) is the only path to `BatchOptimizerMultiThreaded` and its only caller
/// is `RoutingPipeline.createForGui` (**`:40-42`**) ← `gui/workspace/progress/GuiRoutingJobWorker.java:212`.
///
/// not ported: `BatchOptimizerMultiThreaded` (366) — reachable only from `createForGui`.
/// not ported: `OptimizeRouteTask` (100) — constructed only by `BatchOptimizerMultiThreaded`.
/// not ported: `BatchOptimizer.createForGui` — the GUI factory (the Global Constraints: no GUI).
///
/// The struct and `new` are **Task 13's** (scan ruling 7); this task adds an `impl` block and
/// declares no type other than `OptimizerResult`.
impl<'a> BatchOptimizer<'a> {
    /// `runBatchLoop()` (:125-272) — the optimizer's whole termination condition.
    pub fn run_batch_loop(&mut self, board: &mut Board, stop: &RouterStop,
                          budget: RouterBudget, progress: &mut dyn ProgressSink)
        -> Result<OptimizerResult, RouterError>;
    /// `optRoutePass(int, boolean)` (:279-385).
    fn opt_route_pass(&mut self, board: &mut Board, pass_no: i32,
                      with_preferred_directions: bool, stop: &RouterStop,
                      budget: RouterBudget, progress: &mut dyn ProgressSink)
        -> Result<(), RouterError>;
    /// `normalizeAlgorithm(String)` (:68-78) and `isTimedOut()` (:81-83).
    pub fn normalize_algorithm(name: &str) -> String;
    pub fn is_timed_out(&self) -> bool;
}
pub struct OptimizerResult { pub state: TaskState, pub passes_run: i32,
                             pub items_optimized: i32, pub timed_out: bool }
```

**Transcription notes.**
- **`runBatchLoop`** (`:125-272`): `useIncreasedRipupCosts = true` (`:132`); `initialStats`/`initialScore` (`:135-138`); `deadlineMs = start + parseTimespanString(optimizer.timeoutString)` (`:153-160`, `RouterStop`'s deadline, ruling AI); fire `STARTED` (`:162-163`); then
  `while (currentPass < optimizer.maxPasses && totalItemsOptimized < optimizer.maxItems && !thread.isStopRequested())` (`:167-171` — **`ALL` only**, which is why a `maxItems` router stop disables this stage, Task 4's quirk cand. C);
  deadline → `isTimedOut; break` (`:172-176`); `++currentPass` (`:177`); `scoreBeforePass` (`:179`);
  **`scoreBeforePass * (1 + improvementThreshold) >= 1000 → break`** (`:182-193` — the "already near-perfect" exit; `1000` is `getNormalizedScore`'s ceiling);
  **`withPreferredDirections = currentPass % 2 != 0`** (`:200` — the passes *alternate*);
  `optRoutePass(currentPass, withPreferredDirections)` (`:201`);
  `passImprovement = (after - before) / before` (`:209-210`);
  `useIncreasedRipupCosts && after <= before → useIncreasedRipupCosts = false; scoreImprovement = -1` (`:212-215`);
  `scoreImprovement != -1 && < improvementThreshold → break` (`:220-230`).
- **`optRoutePass`** (`:279-385`): `sortedRouteItems = new ReadSortedRouteItems()` (`:287`, Task 13); `minCumulativeTraceLength = statsBefore.traces.totalWeightedLength` (`:288`); `maxConsecutiveFailures = optimizer.maxConsecutiveFailures ?? 50` (`:301-304`); then `while (true)`: deadline → `isTimedOut; return` (`:308-313`); `isStopRequested() → return` (`:314-317`); `maxItems` → `break` (`:318-326`); `currentItem = sortedRouteItems.next()`, `null → break` (`:327-330`); `optRouteItem(currentItem, withPreferredDirections, false)` (`:331`); improved → `consecutiveFailures = 0` and the improvement recomputation at `:340-348` (**the correct `(float)`-cast twin of `ItemRouteResult.improvementPercentage`'s integer-truncating bug** — port both, quirk cand. J); else `++consecutiveFailures >= max → break` (`:349-361`).
- The five `NamedAlgorithm` overrides (`:527-550`) become five `const`s (Task 4); the three JMX samplers (`:94-122`) are `// not ported:` with "every reader is a log string".
- `Freerouting.globalSettings.featureFlags.multiThreading` (`settings/FeatureFlagsSettings.java:11`) — the **one static mutable global in Plan 7's scope** — is read only at `:58` inside `createForGui`. Roster it with that evidence; the port has no global.

**Tests (`crates/fr-router/tests/optimizer.rs`):** `passes_alternate_preferred_directions` (`:200`); `a_near_perfect_board_exits_before_the_first_pass` (`:182-193`); `the_increased_ripup_costs_are_dropped_after_one_non_improving_pass` (`:212-215`); `consecutive_failures_break_the_pass` (`:349-361`); `a_max_items_router_stop_disables_this_stage` (the quirk's second half); `the_improvement_recomputation_disagrees_with_the_scorecard_field` (quirk cand. J, both values asserted); `the_multithreaded_optimizer_is_rostered_not_ported` (the roster assertion).

**JVM-pinned evidence (required).** **`p7t9` mode `optimizer`** (`runRouter = false`, `runOptimizer = true`, so the stage is driven over a board loaded from a routed SES) and mode `full` (both stages). Acceptance: **0 diffs on `router-rpi-splitter`, `router-j2-reference` and `router-ecc83-input`, mode `optimizer`, `optimizer.maxPasses ∈ {1, 2, all}`**, with the per-pass score/incomplete tuple identical; plus `p7t8` re-run and still 0 diffs.

**Steps:** tests → implement `optRoutePass` → implement `runBatchLoop` → `p7t9 optimizer` 0 diffs on three stems → roster the two multithread classes with fresh grep evidence in the commit message → fmt/clippy/test/audit → commit `feat(router): BatchOptimizer's pass loop and termination conditions; the multithread roster`.

---

### Task 15: `run_pipeline`, the `ProgressSink` seam and `AutorouteUnroutedReport` (ruling AK)

**Files:** `crates/fr-router/src/pipeline/{run,unrouted_report,mod}.rs`, `crates/fr-router/src/lib.rs` (the public surface); `crates/fr-drc/src/lib.rs:143` (the `// added in Plan 7: AutorouteUnroutedReport.build` marker becomes `// renamed:` pointing at `fr-router`); `crates/fr-router/tests/pipeline.rs`.
**Java:** `autoroute/pipeline/RoutingPipeline.java (143)` — `StageListener` `:16-25` (10, → `ProgressSink`), the ctor `:32-37` (6, with `optimizer` left `null` unless `getRunOptimizer()` at `:36`), `createForHeadless` `:45-47` (3), the accessors `:50-78` (29), `run` `:81-85` (5), **`runRoutingStage` `:87-114` (28)**, **`runOptimizationStage` `:116-129` (14, `isStopRequested()` confirmed at `:117`)**, `normalizeRouterAlgorithm` `:131-142` (12); `autoroute/pipeline/AutorouteUnroutedReport.java (80)` — `build` `:19-54` (36), `describeItem` `:60-79` (20); `autoroute/pipeline/BatchAutorouter.java:483-485` (`buildUnroutedConnectionsReport`). **≈ 225 Java lines.** Rostered here: `RoutingPipeline.createForGui` **`:40-42`** (`:39` is its javadoc) and `NamedAlgorithm`'s listener plumbing (fields `:26-31`, `fire*` `:88-93`/`:104-110`/`:121-125`).

**Interfaces consumed:** `AutorouteBatchLoop::run` (Task 10), `BatchOptimizer::run_batch_loop` (Task 14), `RoutingBoardExt::finish_autoroute` (Plan 6 Task 9), `fr_drc::DesignRulesChecker::get_all_airlines` (Plan 5).
**Interfaces produced:** `run_pipeline`, `PipelineResult`, `PassRecord` — **exactly as written in §Interfaces above** — plus:
```rust
// crates/fr-router/src/pipeline/unrouted_report.rs
/// Port of `autoroute.pipeline.AutorouteUnroutedReport` (AutorouteUnroutedReport.java:1-79) —
/// the diagnostic the batch loop emits when it stagnates. It is a **consumer** of `fr-drc`
/// (`new DesignRulesChecker(board, null)`, `calculateAllIncompletes()`, `getAllAirlines()` at
/// `:20-22`), which is why it lives here and not in `fr-drc` (that crate's marker at
/// `lib.rs:143` becomes a `// renamed:`).
///
/// The `LinkedHashMap<String, List<String>>` at `:28` is insertion-ordered by `getAllAirlines()`
/// order — a `Vec<(String, Vec<String>)>`, **not** a `BTreeMap` (ruling 5's recorded decision).
pub fn build_unrouted_report(board: &mut Board) -> String;                        // :19-54
pub(crate) fn describe_item(board: &Board, item: ItemId) -> String;              // :60-79
```

**Transcription notes for `run_pipeline` (`RoutingPipeline.java:81-129`):**
1. `run()` (`:81-85`) is `runRoutingStage(); runOptimizationStage();` — in that order, unconditionally.
2. **`runRoutingStage`** (`:87-114`): `routerEnabled = getRunRouter() && (maxPasses == null || maxPasses >= 0)` (`:88-91`); `if (routerEnabled || isFanoutEnabled()) job.stage = ROUTING` (`:93-95`); `if (routerEnabled && !thread.isStopAutoRouterRequested()) autorouter.runBatchLoop()` (`:97-98`); **`else if (isFanoutEnabled() && !isStopAutoRouterRequested()) { maxPasses = 0; runBatchLoop(); } finally restore` (`:99-108` — the fanout-only mode, which mutates the settings object and restores it in a `finally`)**; then **`job.board.finishAutoroute()` (`:110` — the ONLY caller in the whole tree)**; then `listener.afterRouting(autorouter)` (`:111-113`) → a `RoutingEvent`.
3. **`runOptimizationStage`** (`:116-129`): `if (optimizer == null || thread.isStopRequested()) return` (`:117-119` — **`ALL`**, not `AUTO_ROUTER_ONLY`); `job.stage = OPTIMIZATION` (`:121`); `optimizer.runBatchLoop()` (`:125`). `optimizer` is `null` unless `job.routerSettings.getRunOptimizer()` (`:36`), so the port's `Option<BatchOptimizer>` mirrors it.
4. The fanout-only mode's settings mutation is the one place the port cannot simply forward its borrowed `&RouterSettings`: **`run_pipeline`'s signature keeps `settings: &RouterSettings`** (it is the contract Plan 8 wraps) and this branch **clones it internally**, mutates `max_passes = 0` on the clone and drops it — with a `// renamed:` note saying Java mutates the shared object and restores it in a `finally`, and a test that the caller's settings are unchanged afterwards.
5. **Ruling 7's sole new boundary** *(scan ruling 9; was "seventh")*: `AutorouteBatchLoop.run`'s `IllegalArgumentException` (`:51-56`, the `throw` at `:55`) is **not** caught here, so `run_pipeline` propagates `Err(RouterError::NoRoutableLayer)`. A test on `empty_board.dsn` pins it.
6. `normalizeRouterAlgorithm` (`:131-142`) is a string-normalising helper the CLI reads; port it (12 lines) so Plan 8 has it.

**Tests (`crates/fr-router/tests/pipeline.rs`):**
- **The port of `src/test/java/app/freerouting/fixtures/RoutingPipelineComparisonTest.java:49-76`** (ruling 14) — the only Java test that compares two runs of one board; here it becomes `two_runs_of_the_same_board_are_identical`, asserting equal `incomplete_count`, `via_count`, violation count **and** `structural_hash` (stronger than Java's three counters, which is the point).
- `finish_autoroute_is_called_exactly_once` (a counting `Board` wrapper or an assertion on the engine's state) — `RoutingPipeline.java:110` is the **only** caller in the whole Java tree.
- `the_fanout_only_mode_sets_max_passes_to_zero_and_leaves_the_callers_settings_untouched`.
- `a_max_items_stop_skips_the_optimizer_stage_but_a_max_passes_stop_does_not` (the full quirk cand. C loop, end to end — this is where it becomes observable).
- `an_empty_board_errors_with_no_routable_layer`.
- `a_recording_sink_sees_the_stage_events_in_javas_order`.
- `the_unrouted_report_lists_airlines_in_getAllAirlines_order`.

**JVM-pinned evidence (required).** **`p7t9` mode `full`** — both stages, driven on the Java side through the jar's own `RoutingPipeline.createForHeadless` (i.e. the same code path a `-de/-do` run takes), with the per-pass tuple, the stage events and the final board dump; `p7t9.rs` drives `run_pipeline`. Acceptance for **this** task: **0 diffs on `router-rpi-splitter`, `router-j2-reference` and `router-ecc83-input`, mode `full`, `maxPasses ∈ {1, 2, 8}`**, plus a `P7T15Probe`-free `empty_board.dsn` case where both sides report the same failure. (The SES **bytes** are Task 16's rung.)

**Steps:** tests → implement `AutorouteUnroutedReport` → implement `run_pipeline`'s two stages → `p7t9 full` 0 diffs on three stems → port `RoutingPipelineComparisonTest` → fmt/clippy/test/audit → commit `feat(router): run_pipeline, the two stages, ProgressSink and the unrouted report (ruling AK)`.

---

### Task 16: `p7t9`, the whole-board SES reference family and the acceptance ladder (ruling AM)

**Files:** `scripts/gen-batch-reference.sh`, `scripts/differential/sweep-p7t9.sh`, `scripts/differential/java/P7T9.java`, `scripts/differential/rust/src/bin/p7t9.rs`, `scripts/differential/{run.sh,README.md}`, `tests/reference/router-fixtures.txt`, `tests/reference/<stem>/{batch.ses,batch.meta.txt,batch.passes.jsonl}`, `tests/parity/src/lib.rs` (a `BatchMetrics` helper); `crates/fr-router/tests/{batch_parity,fixtures}.rs`.

**Interfaces consumed:** `run_pipeline`, `PipelineResult`, `PassRecord` (Task 15); `fr_dsn::{read_board, write_ses}` (Plan 3).
**Interfaces produced:** none in `fr-router`; the deliverable is the reference family and the harness.

**`scripts/gen-batch-reference.sh`** — sibling of `gen-router-reference.sh`, sharing its `portable()`, its preflight, its `timeout(1)` bound and its `--meta-only` / `--verify-hash-modes` modes. Unlike `gen-router-reference.sh` it **does drive the CLI**, because ruling AK gives the port the whole pipeline:
```bash
JAR="${FREEROUTING_JAR:-$JAVA_DIR/build/libs/freerouting-current-executable.jar}"
JAVA_BIN="${JAVA:-/opt/homebrew/opt/openjdk@25/bin/java}"
# router-fixtures.txt rows are  stem|dsn|max_items|ripup_pass_no|max_passes|fanout|optimizer
# — the first FOUR fields already exist and gen-router-reference.sh reads exactly those, so the
# batch columns are 5, 6 and 7 and `ripup_pass_no` becomes mandatory-or-`-` on every row.
while IFS='|' read -r stem dsn _max_items _ripup max_passes fanout optimizer; do
  [ -n "$max_passes" ] || continue          # per-connection-only rows have no batch columns
  # P7T9.java, NOT the bare jar: ruling AI needs the optChangedArea budget disabled and the jar
  # has no flag for it (see the callout below). The driver reflects the constant to 0 itself.
  "$JAVA_BIN" -XX:+UnlockExperimentalVMOptions "-XX:hashCode=$HASH_MODE" \
      -Djava.awt.headless=true -Duser.language=en -Duser.country=US \
      -cp "$JAR:$DRIVER_CLASSES" app.freerouting.autoroute.pipeline.P7T9 \
      "$JAVA_DIR/$dsn" "$max_passes" full "$fanout" "$optimizer" \
      --ses "$REF/$stem/batch.ses" \
      > "$REF/$stem/java.batch.log" 2>&1
done < "$REF/router-fixtures.txt"
```
*(Scan ruling 12 rewrote this snippet: the first draft read five `IFS='|'` fields against a
six-or-seven-field row — so `max_passes` would have been read from the `max_items` column — and
invoked the bare jar with a `-Dfreerouting.opt_changed_area_ms=0` flag that the next paragraph
says does not exist.)*

plus `batch.meta.txt` per stem (jar path, size, mtime, jar version, `java -version`, hash mode, **the budget flag**, the full command line, every machine-specific prefix replaced) and `batch.passes.jsonl` scraped from the driver rather than from the log.

> **There is no property that disables the budget in Java, and the generator above therefore never runs the bare jar.** Ruling AI requires the budget disabled on both sides, and the jar hard-codes `TIME_LIMIT_TO_PREVENT_ENDLESS_LOOP = 1000`. So `P7T9.java` — **not** the bare jar — is what the generator runs for the reference: it re-implements `RoutingJobSchedulerActionThread`'s eight lines of headless wiring (`:92-99`, `:259-266`) around `RoutingPipeline.createForHeadless`, reflects the constant to `0` in `AutorouteConnectionRouter`, `BatchAutorouter` and `RoutingBoard`, and calls `SesWriter.write` itself. **The generator's first job is to prove that is faithful**: with the budget left at 1000, `P7T9.java`'s SES must be byte-identical to the bare jar's `-de/-do` output on all five stems. That check is `gen-batch-reference.sh --verify-driver`, it runs in CI's slow lane, and its output is committed to `batch.meta.txt`. **Controller answer 1 governs what a difference means** — a bare-jar difference **with** recorded `optChangedArea` budget trips is *informational*; a difference with **zero** trips is a failure, because the probe is then not the jar. *(Scan ruling 15 struck the earlier sentence "If it ever fails, the driver is wrong and no `p7t9` MATCH means anything", which contradicted answer 1.)*

**`tests/reference/router-fixtures.txt`** gains three columns — `max_passes|fanout|optimizer` at **positions 5, 6 and 7** — appended to the existing `stem|dsn|max_items|ripup_pass_no` rows. The 4th column is already **optional and already used** by one row, so this task makes it **mandatory-or-`-`** on every row first; `gen-router-reference.sh` reads fields 1-4 and keeps working unchanged. The three new stems below are **new rows**, and they carry `-` in the `max_items`/`ripup_pass_no` columns so `gen-router-reference.sh` skips them. The batch rows:

| stem | dsn | max_passes | fanout | optimizer | CI lane (ruling AM) |
|---|---|---|---|---|---|
| `router-rpi-splitter` | `fixtures/Issue143-rpi_splitter.dsn` | 8 | on | on | **CI** |
| `router-j2-reference` | `fixtures/Issue026-J2_reference.dsn` | 99 | on | on | **CI** |
| `router-ecc83-input` | `fixtures/Issue649-kicad_ecc83-pp_input_board_v1.dsn` | 8 | on | on | **CI** |
| `router-dac2020-bm01` | `fixtures/Issue508-DAC2020_bm01.dsn` | 2 | on | on | `#[cfg_attr(debug_assertions, ignore)]` + `FR_SLOW_PARITY=1` |
| `router-tutorial-board` | `examples/tutorial_board/tutorial_board.dsn` | 8 | on | on | `#[cfg_attr(debug_assertions, ignore)]` + `FR_SLOW_PARITY=1` |
| `router-fanout-bm11` | `fixtures/Issue730-DAC2020_bm11.dsn` | 2 | on | off | `FR_SLOW_PARITY=1` (the fanout-trace fixture) |
| `router-strict-drc-cnh` | `fixtures/Issue555-CNH_Functional_Tester_1.dsn` | 2 | on | on | `FR_SLOW_PARITY=1` (16 pre-existing violations, must add none) |
| `router-empty-board` | `fixtures/empty_board.dsn` | 1 | off | off | **CI** (the `NoRoutableLayer` path) |

> **Scan ruling 13 — the stem set is an authorised extension of ruling AM.** Ruling AM named five
> stems (three in CI, two slow). The three added here — `router-fanout-bm11` (the only fanout-trace
> fixture, so ruling AM's fanout claim is otherwise unmeasured), `router-strict-drc-cnh` (the only
> board with pre-existing violations, so Task 8's `enforceStrictDrc` is otherwise unmeasured) and
> `router-empty-board` (ruling 7's `NoRoutableLayer` path, which is cheap and belongs in CI) — are
> **authorised**: **eight stems, four in CI, four `#[cfg_attr(debug_assertions, ignore)]` +
> `FR_SLOW_PARITY=1`.** Ruling AM's CI/slow *mechanism* is unchanged.

**The acceptance ladder (ruling 1), recorded per stem in `crates/fr-router/README.md`:**
- **(a)** the pass count and every `PassRecord` tuple identical — **required for every stem**;
- **(b)** the item set after every pass identical — **required for every stem**;
- **(c)** **byte-identical SES** — required for every stem; a stem that reaches (a)+(b) but not (c) is an `XDIFF` row with the first differing byte, the offending item and a one-line diagnosis, and the README records the **normalised digest** that was compared instead (ruling AM's single escape hatch).

**`--verify-hash-modes`** regenerates each stem under `-XX:hashCode=0..4` and requires **five byte-identical SES files** (all **eight** stems, per scan ruling 13) — the direct check that Plan 6's premise still holds now that fanout and the optimizer are live. Plan 6's survey already verified this end-to-end on two boards; this task extends it to **all eight stems plus the fanout-heavy one** and pins the result in `batch.meta.txt`.

**`scripts/differential/sweep-p7t9.sh`** — the `sweep-p5t1.sh` shape: every stem × every mode (`router-only`, `router+fanout`, `optimizer`, `full`), reporting `MATCH` / `XDIFF` / `SKIP` per row with a recorded budget, so a regression is a row that changes rather than a wall of diff.

**`crates/fr-router/tests/batch_parity.rs`:** one test per stem, reading `tests/reference/<stem>/{batch.ses,batch.passes.jsonl}` through `parity::require_reference`, running `run_pipeline` with `RouterBudget::disabled()` and comparing (a)/(b)/(c); plus `references_are_from_the_head_jar` — every `batch.meta.txt` names `freerouting-current-executable.jar` and pins the jar's **`Build-Revision`** from its `META-INF/MANIFEST.MF`. **Do not assert a version string**: HEAD's manifest carries `Implementation-Version: unspecified`, so the `2.3.1-SNAPSHOT` line the first draft asked for cannot exist and `the_driver_matches_the_bare_jar` (asserting `--verify-driver`'s recorded result is present and clean).

**`crates/fr-router/tests/fixtures.rs`** — extend Plan 6's single-pass harness to the multi-pass one, adding `Dac2020BenchmarkRoutingTest.java`'s seven rows (`:31, 47, 63, 79, 95, 110, 125` — maxItems 2/43/61/111/151 → ≤194/161/147/134/126 incomplete; maxPasses 1 → ≤56; maxPasses 2 → ≤28), `J2ReferenceRoutingTest.java:19-31` (`maxPasses(99)`, ≤3 incomplete, 0 violations, `drillItemCount < 60`) and `StrictDrcRoutingTest.java:26-29` (16 pre-existing violations, add none, ≤30 incomplete). **Quirk #157 governs every one of those bounds** and the module doc says so.

**Steps:** `P7T9.java` (all four modes) → `--verify-driver` clean against the bare jar → `gen-batch-reference.sh` → `--verify-hash-modes` clean → `p7t9` MATCH on the three CI stems → then the slow lane → `sweep-p7t9.sh` → the README acceptance table → the fixture bounds → fmt/clippy/test/audit → commit `test(router): p7t9, the HEAD-jar whole-board SES references and the acceptance ladder`.

---

### Task 17: audits to zero, the roster, the README, quirks #194+, and the Plan 7 hand-off

**Files:** `crates/fr-router/src/lib.rs` (the rewritten roster), `crates/fr-router/README.md`, `scripts/audit-map/fr-router.map`, `docs/java-quirks.md`, `docs/plan-7-handoff.md`, and the obligation ticks in `docs/plan-2-handoff.md` / `plan-3-handoff.md` / `plan-4-handoff.md` / `plan-5-handoff.md` / `plan-6-handoff.md`.

**Audit to zero.** All of these exit 0 with **no `MISSING` and no `UNMAPPED`**:
```
scripts/audit-port.sh autoroute            crates/fr-router/src '*.java' scripts/audit-map/fr-router.map
scripts/audit-port.sh autoroute/pipeline   crates/fr-router/src '*.java' scripts/audit-map/fr-router.map
scripts/audit-port.sh autoroute/maze       crates/fr-router/src '*.java' scripts/audit-map/fr-router.map
scripts/audit-port.sh autoroute/expansion  crates/fr-router/src '*.java' scripts/audit-map/fr-router.map
scripts/audit-port.sh autoroute/drill      crates/fr-router/src '*.java' scripts/audit-map/fr-router.map
scripts/audit-port.sh autoroute/path       crates/fr-router/src '*.java' scripts/audit-map/fr-router.map
scripts/audit-port.sh board/optimize       crates/fr-router/src '*.java' scripts/audit-map/fr-router.map
scripts/audit-port.sh core/scoring         crates/fr-router/src '*.java' scripts/audit-map/fr-router.map
scripts/audit-port.sh core                 crates/fr-router/src \
  'RouterCounters.java StoppableThread.java ProgressThrottler.java' scripts/audit-map/fr-router.map
scripts/audit-port.sh board/facade         crates/fr-router/src \
  'RoutingBoardOperations.java' scripts/audit-map/fr-router.map
# NOTE the map: this one invocation pairs fr-router/src with fr-board.map, because PolylineTrace's
# methods are split across the two crates (the trio's callers are fr-router's, the class is
# fr-board's). Confirm that is what the maps in the tree actually express before running it; if
# fr-router.map is the right map, use it and say so in the commit message.
scripts/audit-port.sh board/trace          crates/fr-router/src \
  'PolylineTrace.java' scripts/audit-map/fr-board.map
```
**The marker gate, restated (scan ruling 2).** The plan's first draft demanded
`grep -rn "added in Plan 7" crates/` return nothing. That is unsatisfiable: at plan time the string
occurs **75 times in 15 files**, and **nine of them are prose in `crates/fr-router/README.md`**
explaining the convention — which `docs/plan-6-handoff.md` §10 explicitly warns about ("`crates/`
additionally sweeps the READMEs' prose about the markers"). The gate is therefore **two greps**:

```
# (1) the code gate — must return NOTHING
grep -rn "added in Plan 7" crates/*/src

# (2) the test gate — must return NOTHING
grep -rn "added in Plan 7" crates/*/tests

# (3) prose: crates/fr-router/README.md may discuss the convention, but only in the past tense
#     ("was added in Plan 7"), never as a live marker. Task 17 rewrites those nine occurrences.
```

Every marker is **consumed**, re-pointed to `// not reachable:` per ruling AJ, `// renamed:` at its
new home, or rewritten to `// added in Plan 8:` **with the reason, never deleted**. The four
occurrences in `crates/fr-router/tests/{control.rs,java_ports.rs,tightener.rs}` and the one in
`crates/fr-board/tests/board.rs` are **forward references written by Plan 6** — this task rewrites
each to name what actually landed, or to `// added in Plan 8:` where it did not.

**Owner table for every marker no earlier task claims** (the rest are claimed in their tasks' Files
lines, which this scan made exact):

| marker site | disposition | owner |
|---|---|---|
| `fr-board/src/board/mod.rs:182` `RoutingBoard.autoroute` | `// renamed:` → `route_connection_full` / `autoroute_item` | 9 |
| `fr-board/src/board/mod.rs:186` `RoutingBoard.moveDrillItem` | `// renamed:` → `fr-router` `board_ext` | 6 (scan ruling 3) |
| `fr-board/src/board/mod.rs:187` `RoutingBoard.forcedVia` | `// renamed:` → Plan 6's `ForcedViaInserter` if it is the whole body; otherwise `// added in Plan 8:` with the reason | **17** |
| `fr-board/src/board/mod.rs:214` `autoroute.RoutingFailureLog` | `// renamed:` → `fr_router::pipeline::failure_log`; the `Vec<String>` hook goes | 9 |
| `fr-board/src/items/trace.rs:101` `smoothenEndCornersFork` | `// renamed:` → `TraceTightener::smoothen_end_corners_at_trace` | 5 |
| `fr-board/src/items/trace.rs:92` | prose about the convention — **stays** | — |
| `fr-router/src/board_ext/mod.rs`, `…/routing_board_ext.rs` (1 each) | re-point to the method that landed | **17** |
| `fr-board/tests/board.rs` (1), `fr-router/tests/{control.rs ×2, java_ports.rs ×2, tightener.rs ×1}` | rewrite to what landed, or `// added in Plan 8:` | **17** |
| `fr-router/README.md` ×9 | reword to the past tense | **17** |
| `lib.rs:191` `RoutingFailureLog.shouldGiveUp`, `:196` `.toString` | `// not ported:` with caller evidence | 9 |

Plus `grep -rn "item_tree_shape_ref\|item_tile_shape_ref" crates/fr-router/` still returning nothing
(plan-6 ruling 10), and **`grep -rn "pub seam:" crates/fr-router/src` down from eight to six**, with
`AutorouteControl::from_settings` (Task 11) and `AutorouteAttemptResult::is_routed` (Tasks 9/10)
recorded as closed (scan ruling 4).

**The rewritten `// not ported:` roster in `lib.rs`,** one line each with the caller evidence:
- **The five multithread classes** (ruling AM's General clause), each with the grep that proves it:
  - `autoroute/pipeline/BatchAutorouterThread.java` (621) — **zero callers in the whole tree**; its only constructor site is `AutoroutePassRunner.runMultiThread:62`, whose only caller is `BatchAutorouter.autoroutePassMultiThread:412`, which `grep -rn "autoroutePassMultiThread" src/main src/test` shows is called from **nowhere**. It is a line-for-line duplicate of `AutoroutePassRunner.runSingleThread` + `AutorouteConnectionRouter.route` + `AutorouteAirlineCalculator`.
  - `AutoroutePassRunner.runMultiThread` `:40-149` (110) — same, no caller.
  - `BatchAutorouter.autoroutePassMultiThread` `:411-413` (3) — same.
  - `autoroute/pipeline/BatchOptimizerMultiThreaded.java` (366) — reachable only from `BatchOptimizer.createForGui:56-66` ← `RoutingPipeline.createForGui:41` ← `gui/workspace/progress/GuiRoutingJobWorker.java:212`. Headless goes `RoutingJobSchedulerActionThread.java:99` → `createForHeadless`.
  - `autoroute/pipeline/OptimizeRouteTask.java` (100) — constructed only by `BatchOptimizerMultiThreaded`.
  **`RouterSettings.maxThreads` therefore has no live reader anywhere** — its only three are `AutoroutePassRunner.java:50, 53, 91`, all inside `runMultiThread`. This **extends quirk #143** (which said `-mt` is dead *headless*); it is dead everywhere. The roster line says so and the README repeats it, so Plan 8 does not read the silence as an invitation.
- `autoroute/pipeline/AutorouteRuntimeMetrics.java` (66) — JMX CPU/heap samplers; every reader is a log string.
- `autoroute/events/**` (6 files, **115**) — observers, replaced by `ProgressSink` (ruling AK).
- `autoroute/pipeline/NamedAlgorithm.java:30-96` (67) — the three listener lists, same reason.
- `core/ProgressThrottler.java` (32) — a 1 000 ms wall clock gating progress.
- `autoroute/BoardHistoryEntry.java` (33) — **shadowed and dead** (Task 2's finding).
- `RoutingFailureLog.{shouldSkip, getUnroutableItems, hasUnroutableItems, clear}` (~35) — `shouldSkip`'s only caller is the dead `BatchAutorouterThread:324`; the other three have **no callers at all**, so the `FAILURE_THRESHOLD = 50` give-up policy the class documents is never applied (quirk row).
- `AutorouteAirlineCalculator:42-214` (173, to the file's last line) — reached only from `BatchAutorouterThread` (ruling 6's fresh grep).
- `BatchOptimizer.createForGui` `:56-66`, the three JMX samplers `:94-122`, `BatchAutorouter`'s profile counters `:184-238` and `threadIndexToLetter` `:536-554`.
- `AutoroutePassRunner`'s five `log*` helpers `:338-487` and the hard-coded net-94 gate `:256-258`.
- `BoardStatistics(byte[], FileFormat)` `:436-554`, `countOccurrences`, `toString`, `BoardScoreBreakdown`, `ScoringWeightComparison` — **`// added in Plan 8:`** (ruling 4), not "not ported".

**`docs/java-quirks.md` rows — numbering.** The register is contiguous through **#193** (Plan 6 Task 18 closed the plan with #189-#193; `docs/java-quirks.md` §Process notes states the next free id verbatim). **#194 is the first free label** — scan ruling 1 renumbered this whole table from the plan's original #189-#212, which collided with five committed rows. Re-check the register's last row before writing anyway, and if a late fix added more, renumber and say so in the commit message. The table below gives **labels, not ids**: as in Plan 6's amendment block, ids are allocated **contiguously, in the order rows are written**, so the first Plan 7 task to write a row takes #194 and each later task re-checks the register's last row before writing. Every task that lands a row **must** append an amendment line here mapping its plan label to the id it actually took.

| plan label | what | Java site | task |
|---|---|---|---|
| **#194** | `getHash()`'s Javadoc says "an MD5 hash of the board trace state", but `serialize(true)` writes `getTraces()`, `getVias()` **and `itemList`** — the whole item graph. The wrong doc is what justified the port's original narrow hash. | `BasicBoard.java:163`; `BoardSnapshotManager.java:29-35, 56` | 3 |

**Amendment log (plan label → landed id).** Task 17 requires one line per row here. The whole table was renumbered from the plan's original #189-#212 by pre-flight scan ruling 1, because `docs/java-quirks.md` is contiguous through **#193** and its §Process notes name **#194** as the next free id. Labels above are therefore already the intended ids; each task still re-checks the register's last row before writing and appends its landed id here.
| **#195** | `BoardStatistics`' fanout block (`:407-426`) reads **net index 0 only** for a multi-net SMD pin, so a pin escaped on its second net counts as unescaped. | `core/scoring/BoardStatistics.java:415` | 1 |
| **#196** | `getNormalizedScore` divides by `getMaximumScore()`, which is `0` for a board with no connections, so the board scores **NaN** — `Math.max(0, NaN)` is `NaN`, not 0. `empty_board.dsn` reaches it. | `BoardStatistics.java:619-621, 624-635` | 1 |
| **#197** | `BoardHistory.getMaxScore` starts at `0`, not `-inf`, so an empty history answers 0 and the strict `>` at the restore gate never fires. | `autoroute/BoardHistory.java:118` | 2 |
| **#198** | `BoardHistory.restoreBoard` **sorts the list and increments `restoreCount` (`:147`) under a *read* lock**, so `getRank`'s answer depends on how many restores have happened — and `getRank` gates the pass loop's `BOARD_RANK_LIMIT` break. | `BoardHistory.java:139-155, 173-186`; `AutorouteBatchLoop.java:315-320` | 2 |
| **#199** | `autoroute/BoardHistoryEntry.java` (33 loc, public, `Comparable`, holding a live board and an `Instant`) is **shadowed** by `BoardHistory`'s own private nested class of the same name and is unreachable. | `BoardHistory.java:188`; `autoroute/BoardHistoryEntry.java` | 2 |
| **#200** | `maxItems` calls `requestStop()` (**`ALL`**) while `maxPasses` calls `requestStopAutoRouter()` (**`AUTO_ROUTER_ONLY`**), and `RoutingPipeline:117` tests `isStopRequested()` — so hitting `--max-items` **silently disables the optimizer stage** while hitting `--max-passes` does not. | `AutoroutePassRunner.java:219` vs `AutorouteBatchLoop.java:271`; `RoutingPipeline.java:117` | 4, 15 |
| **#201** | The job monitor calls `requestStop()` (`ALL`) **30 s before** it writes `job.state = TIMED_OUT`, so `AutorouteBatchLoop:251-253`'s `requestStopAutoRouter()` is always a no-op — the state only changes which `TaskState` is *reported*. | `RoutingJobSchedulerActionThread.java:75, 84`; `AutorouteBatchLoop.java:251-253` | 4 |
| **#202** | `RoutingBoardOperations.optChangedArea` compares `clipShape != IntOctagon.EMPTY` by **reference** on a value object; every router caller passes `null`, so the branch always runs. Same shape as quirk #34. | `RoutingBoardOperations.java:64` | 5 |
| **#218** | `PolylineTrace`'s `ConnectionToPin` family is unreachable from the maze inserter because `FoundConnectionInserter.insertTrace` pins `pinEdgeToTurnDist = -1` for the duration of the insert (`:140-141`, restored `:447`); it becomes reachable only through `optChangedArea`'s `pullTight`, which runs outside that window. | `PolylineTrace.java:841-861`; `FoundConnectionInserter.java:140-141, 447` | 5 |
| **#203** | `retryConnectionNecked` reuses the **same `TimeLimit` object** the first attempt already consumed, so the necked retry inherits whatever budget is left — often none on a hard connection, making it a no-op precisely where it is wanted. | `AutorouteConnectionRouter.java:171, 204-214` | 8 |
| **#219** | `crates/fr-router/src/lib.rs`'s roster and `docs/plan-6-handoff.md` §10.1 both cite `AutorouteConnectionRouter.java:160-233` for "steps 6-8"; `:160` is `route`'s closing brace and no method spans that range. The real sites are `:95-121`, `:123-153`, `:162-241` and `:243-254`. A port-side documentation defect, recorded because it is what a reader looks up first. | `AutorouteConnectionRouter.java:30-160, 162-254` | 8 |
| **#204** | The strict-DRC rollback assigns `router.board`, a field of `BatchAutorouter`, so `AutorouteBatchLoop.run`'s local `RoutingBoard board` (`:38`) goes stale. Latent — nothing reads it again. | `AutorouteConnectionRouter.java:250-252`; `AutorouteBatchLoop.java:38` | 8 |
| **#205** | `getAutorouteItems` appends an item **once per qualifying net index**, and the pass runner then loops over **every** net index of each appearance — a 2-net item that qualifies twice is routed four times per pass, and the inner index is a net index rather than the one that qualified. | `BatchAutorouter.java:363-403`; `AutoroutePassRunner.java:202, 207` | 9 |
| **#206** | `ItemRouteResult.improvementPercentage` divides `viaCountAfter / viaCountBefore` as `int / int`, truncating the via term to 0 or 1 while the trace term is a `double`; `BatchOptimizer:340-348` recomputes the same expression correctly with a `(float)` cast, so the field is wrong and the used value is right. | `ItemRouteResult.java:59-65`; `BatchOptimizer.java:340-348` | 9, 14 |
| **#207** | A **normal** end of routing reports `CANCELLED`, not `FINISHED`: every ordinary exit from the pass loop calls `requestStopAutoRouter()` first, so `:571`'s `if (!isStopAutoRouterRequested())` is false. Only a fully-routed board reaches `FINISHED`. | `AutorouteBatchLoop.java:271, 311, 318, 474, 505, 571-585` | 10 |
| **#208** | The `else if` that resets the stagnation counter for a fully-routed board hangs off `if (currentPass >= 8 …)`, so on passes 1-7 a routed board resets the counter and on pass ≥ 8 it does not — the opposite of the comment three lines above. | `AutorouteBatchLoop.java:422, 509-517` | 10 |
| **#209** | `alreadyRoutedBoardHashes` is a live `HashSet<String>` whose only two readers are commented out; it is still allocated per run and `.clear()`ed twice. | `AutorouteBatchLoop.java:249, 257-267, 334, 446` | 10 |
| **#210** | `RoutingBoard.fanout`'s "≤ 4 targets" strategy calls `autorouteConnection` **twice with the same `rippedItemList`**, so items ripped by the failed first attempt are carried into the second attempt's accounting and reported as this pin's rips. | `RoutingBoard.java:1064-1085` | 11 |
| **#211** | `BatchFanout.Component.Pin.compareTo` leaves `result = 0` for an **unrecognised** `pinSortingOrder` string, so the order silently becomes pure `pinIndex` — and two pins that then compare equal are **dropped** from the `TreeSet`; `distanceToClosestOnNet` is `Double.MAX_VALUE` for a net with one pin. The run-time-keyed comparator is why the port uses `JavaTreeSet` here (controller ruling Y). | `BatchFanout.java:711, 742-777` | 11 |
| **#212** | The fanout oscillation key is `((long) routedCount << 32) ^ vias.size()`, so two passes with the same `(routedCount, viaCount)` are treated as identical progress even when the geometry differs — and the XOR collides for large via counts. | `BatchFanout.java:133` | 12 |
| **#213** | `canUseVias`' whole gate is skipped when `net == null`, so a null-net SMD pin is fanned out with no via check at all. | `BatchFanout.java:239-249` | 12 |
| **#214** | `autoroutePassesForOptimizingItem` contains an **empty `if` body** whose `updatedRoutingBoard == null` test can never be true, because the same variable is dereferenced unconditionally 15 lines earlier. | `BatchAutorouter.java:256, 271-273` | 13 |
| **#215** | `ReadSortedRouteItems.next()` rescans the whole `itemList` **twice per returned item** — O(n²) per pass — and the "vias beat traces at the same coordinate" tie-break depends on the two loops sharing `currentMinCoor`, which the code does not make obvious. | `BatchOptimizer.java:573-654, 605` | 13 |
| **#216** | The whole multithreaded autoroute path is **unreachable**: `autoroutePassMultiThread` has no caller in `src/main` or `src/test`, so `RouterSettings.maxThreads` has no live reader and `BatchAutorouterThread` is a 621-line dead duplicate. Extends #143 ("`-mt` is dead *headless*") to "dead everywhere". | `AutoroutePassRunner.java:40-149`; `BatchAutorouter.java:411-413`; `BatchAutorouterThread.java` | 17 |
| **#217** | `RoutingFailureLog`'s documented `FAILURE_THRESHOLD = 50` give-up policy is never applied: `shouldSkip`'s only caller is the dead `BatchAutorouterThread:324`, and `getUnroutableItems`/`hasUnroutableItems`/`clear` have no callers at all. | `RoutingFailureLog.java:56-63, 70-87, 103-106` | 9, 17 |

Also **carried forward and re-checked here**: **#162** (`calculateNewIncompleteRooms` non-termination — its "Improvement" column already says "Plan 7's ladder-hang work should take this row with it"; Task 16's `timeout(1)` bound is the current answer and the row records that), **#184** (`TraceTightener45.reduceCorners`' stale clip flag — reachable only with a non-null `clipShape`; Task 5 re-checks whether a Plan 7 caller passes one and records the answer), **#182** (`avoidAcidTraps` disabled by `if (true) return`), **#74/#34** (the `Line` identity token, honoured per Conventions §7), **#76** (the ladder hang — now escapable through ruling AI's deadline, which is what plan-6 ruling AC deferred to "the caller's wall clock"), **#140** (`max_passes == 0` is unlimited) and **#157** (the fixture bounds' first-writer-wins `setMaxPasses`).

**`crates/fr-router/README.md`** gains: what the crate does now (a whole board, both stages) and what it does not (the CLI, the manifest, the MCP surface — Plan 8); ruling 1's acceptance table filled in per stem with the (a)/(b)/(c) rung reached; **ruling 5's container decision table, one row per ordered container with the comparator's totality and key mutability recorded**; ruling AH's `structural_hash` audit table; ruling AI's budget knob and the rule that every parity run disables it on both sides; the **one** new recovery boundary beside Plan 6's five, and why the originally-planned second one was struck (scan ruling 9); the deadline's six sites and why a seventh is a bug; **quirk #143/#216's warning that `-mt` is dead everywhere and must not become a threading policy in Plan 8**; Task 2's memory note (30 `Board` clones); and how to regenerate the batch references and run `p7t1`–`p7t10`.

**`docs/plan-7-handoff.md`:** the delivered surface with every public signature; the fourteen plan rulings and the eight controller rulings AF–AM with what execution confirmed or corrected (rulings 1, 5, 8, AH and AI must each say what the evidence was); parked residuals per task; and the obligations:
- **Plan 8** — `fr-core` wraps `run_pipeline` as `RoutingPipeline::run`; `CancelToken` (`Arc<AtomicBool>` + deadline) maps onto `RouterStop`, and the mapping must preserve the **three**-state distinction, not collapse it to a bool (#200); `ProgressSink` moves to `fr-core` or is re-exported, unchanged; `fr_router::score::BoardStatistics` is re-exported and gains the Gson-compatible JSON, `BoardScoreBreakdown`, `ScoringWeightComparison` and the `byte[]`/`FileFormat` constructor (ruling 4); the result manifest; `-mt`/`-oit` stay inert (#216); the CLI's `--max-items` must document that it also stops the optimizer (#200).
- **Tick, in each earlier hand-off:** Plan 2's `RoutingBoardExt` obligation (**complete** — `opt_changed_area`, `remove_items_and_pull_tight`, `fanout`); Plan 2's `structural_hash` note (**complete** — ruling AH's audit); Plan 3's ruling H (**closed** by Task 0, against re-pointing, with the `p6t1` k=6/k=8 MATCH as evidence); Plan 4's `is_fanout_enabled` and quirks #127/#139/#140/#143 (**consumed**); Plan 5's `getNormalizedScore`-is-Plan-8 note (**amended** by ruling AG); Plan 6's whole Plan 7 list (steps 6-8, `optChangedArea`/pull-tight/the tighteners, quirk #34's `equals_geometric`, `TraceShover.insert`/`ForcedPadRouter`'s routing half/`DrillItemMover`'s mutating half — **all landed in Plan 6 Tasks 10b/15a/15b, so this hand-off records them as already-discharged rather than claiming them**; the pass/item recovery boundaries — **ruling 7, as amended by scan ruling 9: one boundary, not two, and the per-item catch Plan 6 promised does not exist on the live path**; `max_passes == 0` — **#140**; no threading policy — **#216**; the whole-board SES byte-parity headline — **Task 16**).

**Steps:** roster rewrite → audit to zero **without weakening the script** → README (all five tables) → quirks #194+ with the label→id amendment lines → hand-off + obligation ticks → fmt/clippy/test → commit `test(router): audit to zero, the README, quirks #194+ and the Plan 7 hand-off`.

---

## The differential harness — ten drivers, five probes, one reference generator

Same shape as Plans 2–6 (`scripts/differential/README.md`): a `run.sh` case per driver, a
`java/P7T<N>.java` half and a `rust/src/bin/p7t<n>.rs` twin, diffed line for line. Every Java half
declares the package it needs to reach package-private state, is compiled and run in `run.sh`'s
`needs_jar=1` mode against the **clone's HEAD jar** on **JDK 25**, and carries the same flag set:

```
-Djava.awt.headless=true -Duser.language=en -Duser.country=US
-XX:+UnlockExperimentalVMOptions -XX:hashCode=2
```

plus, **on every driver in this plan**, ruling AI's budget disabled on **both** sides: the Java half
reflects `TIME_LIMIT_TO_PREVENT_ENDLESS_LOOP` to `0` in `AutorouteConnectionRouter`,
`BatchAutorouter` and `RoutingBoard` (and sets `fanout.maxMillisecondsPerPin` /
`optimizer.timeoutString` out of reach), the Rust half passes `RouterBudget::disabled()`. Each
driver header states the exact reflection it performs; a driver that cannot disable the budget is
not a parity driver.

| driver | what it pins | Java package | args | task |
|---|---|---|---|---|
| **`p7t1`** | `getAutorouteItems`' `(item, net)` list of one pass, in order, before any routing | `autoroute.pipeline` | `<dsn> <passNo>` | 9 |
| **`p7t2`** | one pass end to end: per `(item, net)` the attempt result, ripped set, ripup costs, `maxId` before/after and inserted geometry, plus the tail-removal + `optChangedArea` delta | `autoroute.pipeline` | `<dsn> <passNo> <maxItems>` | 9 |
| **`p7t3`** | `optChangedArea` alone: a marked changed area, the tightener sweep, the `ViaOptimizer` arm | `board.optimize` | `<dsn> <mode> <accuracy>` | 5, 7 |
| **`p7t4`** | `ViaOptimizer`: `optViaLocation`, `optPlaneOrFanoutVia`, the three `repositionVia` overloads, `isWithinTolerance` | `board.optimize` | `<dsn> <viaIndex> <mode>` | 6, 7 |
| **`p7t5`** | `BatchFanout`: the component/pin visit order under all five sorting strings, then per pin the `fanout()` result and geometry, then a whole `fanoutPass` and a whole `fanoutBoard` | `autoroute.pipeline` | `<dsn> <passNo> <sortingOrder> <mode>` | 11, 12 |
| **`p7t6`** | `correct`/`swapConnectionToPin` at every `pinEdgeToTurnDist` × angle regime × start/end, plus `check` as a regression oracle | `board.trace` | `<mode>` | 5 |
| **`p7t7`** | `BoardStatistics` + `calculateScore`/`getMaximumScore`/`getNormalizedScore` under four `ScoringSettings` presets | `core.scoring` | `<dsn>` | 1 |
| **`p7t8`** | `ReadSortedRouteItems.next()`'s full sequence for one optimizer pass, and `optRouteItem` per item | `autoroute.pipeline` | `<dsn> <mode>` | 13 |
| **`p7t9`** | **whole-board**: the per-pass `PassRecord` trace, the stage events, the final board dump and the **SES bytes**, in four modes (`router-only`, `router+fanout`, `optimizer`, `full`) | `autoroute.pipeline` | `<dsn> <maxPasses> <mode>` | 10, 12, 14, 15, 16 |
| **`p7t10`** | ruling AH's three hash-equality **decisions** (`BatchFanout:152-156`, `BoardHistory.contains`, `BoardHistory.getRank`) over a scripted mutation sequence — decisions, never hash values | `autoroute` | `<dsn> <steps>` | 3 |

**Probes** (`scripts/differential/java/probes/`) — no Rust twin; a Java program whose stdout a Rust
*test* pins as literals, so the test asserts against the HEAD jar rather than against the port's own
opinion. Named by the task that consumes them, as Plan 6's are:

| probe | what it pins | package | transcript | task |
|---|---|---|---|---|
| `P7T2Probe.java` | 40 scripted `BoardHistory` calls: `size`, every entry's `(hash-pattern, score, restoreCount)` in list order, and each call's return | `autoroute` | `crates/fr-router/tests/data/p7t2-board-history.txt` | 2 |
| `P7T4Probe.java` | `StoppableThread`'s full 3×2 transition table + `RouterCounters`' field list by reflection | `core` | `…/p7t4-stop-and-counters.txt` | 4 |
| `P7T5Probe.java` | `EscapeStatistics.fromBoardStatistics` and `isPinEscaped` on hand-built SMD boards | `autoroute.pipeline` | `…/p7t5-escape-stats.txt` | 11 |
| `P7T8Probe.java` | `containsOnlyUnfixedTraces` and the ripped-connection set of `optRouteItem`, per fixed state | `autoroute.pipeline` | `…/p7t8-ripped-sets.txt` | 13 |
| `P7T9Probe.java` | `ItemRouteResult`'s ladder and `improvementPercentage` over 500 scripted tuples (the integer-truncation quirk). **Distinct from the `p7t9` driver** — a probe is one-sided and named for the task that consumes it, as Plan 6's are | `autoroute` | `…/p7t9-item-route-result.txt` | 9 |

Plus **`p6t1` gains a fifth argument**, `steps` (`1-5` | `1-8`), in Task 8 — the same driver, the
same references, one more rung. Plan 6's committed `router.jsonl` files are **not** regenerated.

**The reference generator.** `scripts/gen-batch-reference.sh` (Task 16) is the sibling of
`gen-router-reference.sh` and shares its `portable()` path rendering, its preflight, its
`timeout(1)` bound (quirk #162 still does not terminate on ~0.4 % of room completions) and its
three modes:

| mode | what it does |
|---|---|
| *(default)* | regenerates `tests/reference/<stem>/{batch.ses,batch.passes.jsonl,batch.meta.txt}` for every batch row of `tests/reference/router-fixtures.txt` |
| `--meta-only` | rewrites `batch.meta.txt` from the existing outputs without running the jar |
| `--verify-driver` | runs the **bare jar**'s `-de/-do` and `P7T9.java` **both with the budget left at Java's 1000**, and stamps the result into `batch.meta.txt` as `bare-jar: identical` **or** `bare-jar: budget-tripped (<n> optChangedArea calls hit the limit)`. Per **controller answer 1**: the second is *informational*; a difference with **zero** recorded trips **is a failure**, because the probe is then not the jar. Without this, a `p7t9` MATCH proves only that the port agrees with a driver, not with freerouting |
| `--verify-hash-modes` | regenerates each stem under `-XX:hashCode=0..4` and requires **five byte-identical SES files** — Plan 6's premise, re-checked now that fanout and the optimizer are live |

**The acceptance ladder (ruling 1 + ruling AM), and where each rung is first reached:**

| rung | what | first reached | required for |
|---|---|---|---|
| per-connection (a)(b)(c) | plan-6 ruling 1 — unchanged, still green | Plan 6 Task 17 | every task (regression gate) |
| steps 1-8 per connection | `p6t1 --steps=1-8` MATCH | Task 8 | 5 stems × passes 1, 2 |
| per-pass tuple | `p7t9`'s `PassRecord` trace identical | Task 10 | every stem |
| whole-board item set | `p7t9`'s board dump identical | Task 15 | every stem |
| **SES bytes** | `p7t9`'s `batch.ses` byte-identical | **Task 16** | every stem; an `XDIFF` needs a root cause and a recorded normalised digest |

**CI split (ruling AM, extended by scan ruling 13 to eight stems).**
`crates/fr-router/tests/batch_parity.rs` runs `router-rpi-splitter`, `router-j2-reference`,
`router-ecc83-input` and `router-empty-board` in normal CI (`java_dir`-gated, skipping cleanly when
the clone is absent, as Plan 5 established). `router-dac2020-bm01`, `router-tutorial-board`,
`router-fanout-bm11` and `router-strict-drc-cnh` are `#[cfg_attr(debug_assertions, ignore)]` and
additionally gated on `FR_SLOW_PARITY=1`, i.e. release mode with the env var set. `scripts/differential/sweep-p7t9.sh` runs every stem × every mode and
reports `MATCH`/`XDIFF`/`SKIP` against a recorded budget.

## Hand-off checklist (Task 17 signs each line off in `docs/plan-7-handoff.md`)

- [ ] `cargo test --workspace` green; `cargo clippy --workspace --all-targets -- -D warnings` clean; `cargo fmt --all --check` clean.
- [ ] All eleven `audit-port.sh` invocations of Task 17 exit 0 with **zero `MISSING`, zero `UNMAPPED`**, run on the committed tree by the reviewer.
- [ ] **`grep -rn "added in Plan 7" crates/*/src` returns nothing** and **`grep -rn "added in Plan 7" crates/*/tests` returns nothing**; every marker is consumed, `// not reachable:` (ruling AJ), `// renamed:` or `// added in Plan 8:` with a reason. `crates/fr-router/README.md`'s nine prose mentions are reworded to the past tense (scan ruling 2).
- [ ] **`grep -rn "pub seam:" crates/fr-router/src` returns six, not eight** — `AutorouteControl::from_settings` and `AutorouteAttemptResult::is_routed` closed by Tasks 11 and 9/10 (scan ruling 4).
- [ ] Quirk ids allocated from **#194** (not #189 — the register is contiguous through #193), with the label→id amendment block written.
- [ ] The two `plan-6-handoff.md` §10 obligations this plan supersedes (`BatchAutorouterThread` and the multithread optimizer family) carry their status line, and §10.3's two coverage obligations (`locator.rs:267`, `engine.rs:1374`) are recorded as discharged or still open (scan ruling 16).
- [ ] `grep -rn "item_tree_shape_ref\|item_tile_shape_ref" crates/fr-router/` returns nothing (plan-6 ruling 10 still holds).
- [ ] `#![forbid(unsafe_code)]` present in every crate root touched.
- [ ] Ruling H's register row **closed** with Task 0's `p6t1` k=6/k=8 MATCH transcript pasted in; `p3t15` sweep still 525 MATCH + 5 XDIFF.
- [ ] Ruling AH's `structural_hash` audit table complete — **no `?` cells** — and `p7t10` at 0 decision diffs.
- [ ] Ruling AI's budget knob: `RouterBudget::disabled()` used by every `p7t*` run on both sides; the separate non-parity test proving the 1 000 ms budget trips is green.
- [ ] Ruling AJ: the five `additionalUpdateAfterChange` markers re-pointed to `// not reachable:`; the "no setter" test green; **no `fr-board` signature changed**.
- [ ] Ruling AK: `run_pipeline` + `PipelineResult` + `ProgressSink` exported; `a_recording_sink_changes_no_board_byte` green.
- [ ] Ruling AM: `gen-batch-reference.sh --verify-driver` clean; `--verify-hash-modes` five-way identical on all eight stems; the CI/slow split in place.
- [ ] Ruling 5's container decision table complete in the README — one row per ordered container, with the comparator's totality and key mutability recorded, and any `JavaTreeSet` site named.
- [ ] The five multithread classes rostered `// not ported:` with their caller-evidence greps re-run at port time.
- [ ] Quirk rows written, ids allocated contiguously from **#194**, and every plan label mapped to its landed id in an amendment block.
- [ ] The four ported Java suites present and named (`BoardHistoryTest`, `StrictDrcEnforcementTest`, `RoutingPipelineComparisonTest`, `BatchAutorouterDebugTest`), each with its Java file:line in a doc comment.
- [ ] `crates/fr-router/README.md` carries all five tables (acceptance, containers, hash audit, budget, boundaries).
- [ ] Plan 2–6 obligation ticks written into each earlier hand-off.

## Dispatch order and sizing

**Eighteen tasks, 0–17** (scan ruling 11 declined the 1a/1b split; Task 1 may land as two commits):

0 → 1 → 2 → 3 → 4 → 5 → 6 → 7 → 8 → 9 → 10 → 11 → 12 → 13 → 14 → 15 → 16 → 17.

> **Scan ruling 17 — Task 0 is in flight and nothing else dispatches until it commits.** Task 0 was
> dispatched by the controller and is **mid-edit in the working tree**: `ViaRule.vias` is already
> `Vec<ViaInfo>` and the round-trip test is already renamed, but the tree **does not compile**
> (`crates/fr-dsn/src/rules_reader.rs` and `crates/fr-board/src/rules/via.rs` still call the removed
> `replace_via_info_renumbering_rules`). **Task 0 commits before Task 1 starts, and no other task —
> parallel or otherwise — is dispatched until it does.** The relaxations below apply only *after*
> that commit lands.

Two relaxations are then available and the controller may take them: **Task 4** (the stop machine,
the budget, `PassRecord`, `ProgressSink`) is pure and can run in parallel with Tasks 1–3; and
**Tasks 5–7** (the tightener sweep, `ViaOptimizer`, the `ConnectionToPin` pair) share no file with
Tasks 1–3. *(The first draft also claimed Tasks 5–7 are independent of Task 4 — they are not:*
**Task 5 consumes Task 4's `RouterStop` and `RouterBudget`**, *so 4 must precede 5.)*

Everything else is a hard dependency: Task 2 needs Task 1's score; **Task 3 widens the hash Task 2
was built against, so Task 3 re-runs Task 2's transcript** (see the ordering note under §Interfaces);
Task 5 needs Task 4; Task 8 needs Tasks 4 and 5; Task 9 needs Task 8; Task 10 needs Tasks 2, 4, 8
and 9 (and stubs 12 and 15); Task 11 needs Tasks 0, 5 and 7; Task 12 needs Tasks 3, 10 and 11;
Task 13 needs Task 9; Task 14 needs Task 13; Task 15 needs Tasks 10, 12 and 14; Tasks 16 and 17
need Task 15.

Sizing. **Task 10** (`AutorouteBatchLoop.run` — 571 lines in one method, five constants that each
reach a `break`, three quirk rows and the assignment that decides the SES), **Task 12**
(`BatchFanout.fanoutPass` — 341 lines plus the two stops), **Task 7** (`repositionVia` C alone is
279 lines of candidate enumeration) and **Task 16** (the reference family, whose Java driver is the
plan's one artefact inspection cannot verify) are the four largest: **opus, reviewed twice**.
**Tasks 1, 5, 8, 9, 11, 13, 14, 15** are large: **opus** (Task 1 because the `float` narrowing
reaches every threshold comparison in the plan; Task 5 because the trio is 290 lines of geometry
that Plan 6 could never reach; Task 8 because it changes five `fr-board` markers under every
caller; Task 9 because ruling 10's duplication is the pass's identity; Task 13 because ruling 12's
rescan is the most order-fragile method in the plan). **Tasks 0, 2, 3, 6** are medium: **opus** for
0 (it changes `fr-board` under Plans 3–6) and 3 (the audit is a judgement call per field),
**sonnet** for 2, and **opus** for 6 now that scan ruling 3 folds `RoutingBoard.moveDrillItem` into it. **Tasks 4, 17** are small-to-medium: **sonnet**, with Task 17's audit run by
the reviewer on the committed tree.

Reviewers: opus for 0, 1, 3, 5, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16; sonnet otherwise. From Task 5
on, every review runs the task's own driver **and** `run.sh p6t1` on the five Plan 6 stems, reading
the header line to confirm which jar it used. From Task 10 on, every review additionally runs
`cargo test -p fr-router --test reference_parity` (Plan 6's ladder — every stem's connection count unchanged: 8 + 294 + 294 + 45 + 0 + 22, i.e. 369 across the distinct stems. **The number is arithmetic, not an assertion in the file** — check the per-stem counts, not a `369` literal) and,
from Task 16 on, `cargo test -p fr-router --test batch_parity` on the committed tree, not on the
implementer's word.

## Plan self-review

**Spec coverage.** Spec §9's two remaining groups map to: **Batch** — `BatchFanout` (Tasks 11, 12),
`AutorouteBatchLoop` (10), `AutoroutePassRunner` (9), `BatchAutorouter` (8, 9),
`BatchOptimizer` (13, 14), `AutorouteConnectionRouter`'s steps 6–8 (8),
`AutorouteAirlineCalculator` (9, one method — ruling 6). **Optimise** — `TraceTightener*`
(**already landed in Plan 6 Task 15a** under controller ruling AB; only `optChangedArea` remains,
Task 5), `TraceShover` (**landed in Plan 6 Tasks 9/10b/15b**), `ViaOptimizer` (6, 7). The spec's
"`BatchOptimizer` (+ multithreaded via `rayon`)" is **deviated from, recorded**: the multithreaded
optimizer is GUI-only and the multithreaded autorouter is unreachable everywhere, so there is no
rayon and quirk #216 is the evidence. The spec's "Strategies as enums: item selection / board
update" is honoured by re-using `fr_settings::OptimizerSettings`' enums, whose only readers are the
GUI-only `BatchOptimizerMultiThreaded` — rostered, not re-declared. The spec's public entry points
`batch_fanout`/`batch_autoroute`/`batch_optimize` land as `BatchFanout::fanout_board`,
`AutorouteBatchLoop::run` and `BatchOptimizer::run_batch_loop`, with `run_pipeline` above them
(ruling AK). Spec §10 is Task 4 (`ProgressSink`, `RoutingEvent`) and Task 15 (`run_pipeline`);
`CancelToken` stays Plan 8's and maps onto `RouterStop`, with the hand-off insisting the three-state
distinction survives. Spec §14.1's remaining Java suites all land (ruling 14). Spec §14.3's
`Dac2020Bm01` smoke test is extended from Plan 6's single-pass bound to the seven-row ladder
(Task 16). Spec §14.2's reference clause is deviated from the same way Plans 5 and 6 deviated — a
sibling generator, `gen-batch-reference.sh` — with the reason recorded, and it adds
`--verify-driver`, which neither predecessor needed. Spec §15 step 8's "metric parity" is
**deliberately strengthened** to SES byte parity by ruling 1 and ruling AM; the metric wording
survives as the `XDIFF` fallback. Spec §4's crate list is honoured with **one** graph change
(`fr-drc` becomes a real dependency, ruling 3, which spec §4's own dependency line already draws).
Spec §2's "multithreaded optimizer" scope line is the one place this plan **narrows** the spec, and
§3.4 of the survey is why.

**Deliberately excluded, with citation:** `RoutingPipeline`'s GUI factory and `NamedAlgorithm`'s
listener plumbing (ruling AK); the five multithread classes (quirk #216, ruling AM); the JMX and
profiler blocks (`AutorouteRuntimeMetrics`, `BatchOptimizer:94-122`, `BatchAutorouter:184-238`,
`PerformanceProfiler` — already rostered in Plan 6); `AutoroutePassRunner`'s five `log*` helpers and
the hard-coded net-94 gate (quirk #158's family); `RoutingFailureLog`'s four caller-less methods
(quirk #217); `AutorouteAirlineCalculator:42-213` (ruling 6); `BoardStatistics`' file-scraping
constructor, `BoardScoreBreakdown` and `ScoringWeightComparison` (ruling 4 — Plan 8's);
`autoroute/BoardHistoryEntry.java` (quirk #199); the `alreadyRoutedBoardHashes` allocation
(quirk #209); every `FRLogger` call in 7 500 lines.

**Placeholder scan.** No "TBD", no "add error handling", no "similar to Task N". Every task names
its Java files with line ranges, the signatures it produces, the interfaces it consumes, its
JVM-pinned evidence with the exact acceptance, its tests and its commit message. **Six places name a
decision the implementer must make rather than one this plan makes, each with the default already
chosen and the deciding evidence named:** the `ViaInfos`-tombstone alternative to Task 0's owned
copies (default: owned copies; the switch is a Java-wins report, not a silent choice); every `?`
cell of ruling AH's audit table (Task 3 — the table is the deliverable and the rule for filling it
is written); whether quirk #184 becomes live because some Plan 7 caller passes a non-null
`clipShape` (Task 5 — re-checked, answer recorded); whether `BatchFanout`'s two comparators are
really total with `final` keys (Task 11 — ruling 5 says confirm, do not assume, and names
`JavaTreeSet` as the answer if not); whether `BoardHistory`'s 30 `Board` clones are affordable on
`Issue730-DAC2020_bm11` (Task 2 — measure and record; the fallback is named); and which stems reach
acceptance rung (c) (Task 16 — the ladder is defined, the README table is the deliverable). All six
are decisions-with-a-default. Two constant lists this plan deliberately does **not** transcribe —
`TaskState`'s (11 lines) and `RouterCounters`' (48 lines) — are marked "read the file" rather than
guessed, and that is recorded here so it is visibly a choice.

**Type consistency across tasks.** `ViaRule` changes shape once, in Task 0, and every later
consumer (`AutorouteControl::rebuild_via_info`, `RoutingBoard.fanout`'s combined rule) reads
`&ViaInfo` thereafter. `BoardStatistics` and its ten DTOs are fixed in Task 1 and only re-exported
after (never re-declared, ruling 4). `BoardHistory`/`BoardHistoryEntry` in Task 2. `RouterStop`,
`StopRequestState`, `RouterBudget`, `RouterCounters`, `TaskState`, `NamedAlgorithmType`,
`ProgressSink`, `NoopProgressSink`, `RoutingEvent` in Task 4, used unchanged by Tasks 8–16.
`RoutingBoardExt` is **extended, never re-declared** — Task 5 adds `opt_changed_area` (×2), Task 8
adds `remove_items_and_pull_tight`, Task 11 adds `fanout`. `TraceTightener` and `PolylineTraceExt`
are extended in Task 5. `ViaOptimizer` in Task 6, extended in Task 7. `BatchAutorouter` in Task 8,
extended in Tasks 9 and 13. `RoutingFailureLog`, `ItemRouteResult`, `AutoroutePassRunner` in
Task 9. `BatchLoopResult` in Task 10. `FanoutComponent`/`FanoutPin`/`EscapeStatistics`/
`FanoutRunSummary` in Task 11, consumed by Task 12. `ReadSortedRouteItems` in Task 13,
`BatchOptimizer`/`OptimizerResult` in Task 14. `PipelineResult`/`PassRecord`/`run_pipeline` in
Task 15. **`route_connection`'s signature is not changed** (ruling 2); Task 8 adds the wrapper
`route_connection_full` beside it, so every Plan 6 test and `p6t1 --steps=1-5` keep working. The
plan makes **two API changes to `fr-board`** (Task 0's `ViaRule`, Task 3's widened
`structural_hash` — both behaviour-affecting and both guarded by a named differential), **one to
`fr-dsn`** (Task 0's `apply_via_info`), **one dependency-graph change** (`fr-drc` dev → real), and
**none to `fr-geometry`, `fr-settings` or `fr-drc`'s API**. `crates/freerouting` is untouched —
the CLI is Plan 8's.

## Controller questions

Five, in descending order of what a wrong default costs. Each has a default already written into
the plan, so a silence is answerable; these are the places the survey left genuinely ambiguous.

1. **Task 16's `--verify-driver` is a new requirement the survey did not anticipate, and it may
   fail.** Ruling AI needs the 1 000 ms budget disabled on the Java side, and the bare jar has no
   flag for it — so the reference is generated by `P7T9.java`, which re-implements eight lines of
   `RoutingJobSchedulerActionThread` around `RoutingPipeline.createForHeadless`. The plan makes
   that faithful by requiring `P7T9.java`'s SES to be byte-identical to the bare jar's `-de/-do`
   with the budget left at 1000. **If that check fails** — because the scheduler wires something
   the driver does not — is the answer (a) fix the driver until it matches, blocking Task 16;
   (b) patch the jar's constant and rebuild the clone (which makes the parity jar a modified
   build, and the constraint says HEAD is the authority); or (c) accept a budget-live reference
   and demote ruling AM's SES-byte target to a per-pass-tuple target? **Default: (a).**
2. **`BoardStatistics`' computing constructor is 326 lines and Task 1 is ≈ 520 total** — at the
   ruling's "≤ ~600" ceiling but with no natural split, because the score reads nine of its ten
   DTOs. Splitting it (DTOs + the ctor's item walk, then the score trio + fanout) would give two
   ~260-line tasks and **19 tasks**, exceeding the 16–18 range. **Default: keep it as one task**
   and give it an opus reviewer twice. Confirm, or authorise the 19th task.
3. **Ruling AG says "no new crate in Plan 7", and ruling AK puts `run_pipeline` in `fr-router`.**
   That leaves `fr-router` owning `score` and `pipeline` modules that spec §4 assigns to `fr-core`.
   Plan 8 then either re-exports them (cheap, but `fr-core` becomes partly a facade) or moves them
   (a file move plus every `use` in the workspace). **Default: re-export**, stated in the hand-off.
   Confirm, so Plan 8's survey does not re-open it.
4. **Task 3's audit may find that `structural_hash` cannot be widened to full injectivity without
   hashing the whole item graph**, which would make `BoardHistory.add` (called once per pass) walk
   every item twice. Java pays that cost (MD5 over the serialized graph) and the port would be
   matching it, but the plan has not measured it. **Default: widen to whatever the audit requires
   and measure; if a corpus stem regresses by more than ~10 % wall clock, record it and ask.**
   Confirm that a performance regression is acceptable where it buys decision parity.
5. **Quirk #162's non-terminating `calculateNewIncompleteRooms` is still unguarded**, and Task 16
   runs whole boards where Plan 6 ran connections — so the exposure grows. The plan's answer is
   `timeout(1)` on the generator and a wall-clock bound on the slow tests, exactly as
   `gen-router-reference.sh` does. The register row says "Plan 7's ladder-hang work should take this
   row with it". **Default: do not fix it in Plan 7** — a guard Java does not have would change the
   room set and break parity. Confirm, or authorise a Plan 7 task to reproduce Java's own
   non-termination behind ruling AI's deadline (which is *not* consulted on that path in either
   language, so it would be a port-only guard and a divergence).

---

## Controller answers to the plan's questions (2026-08-30, binding)

1. **`gen-batch-reference.sh --verify-driver` failing against the bare jar.** Expected in principle: the bare jar has the 1000 ms `optChangedArea` budget live, so on a slow run its SES may differ from the probe's budget-disabled run. Ruling: references are **probe-generated** (budget disabled, ruling AI); `--verify-driver` compares against the bare jar and records the result per stem in `batch.meta.txt` as `bare-jar: identical | budget-tripped (<n> optChangedArea calls hit the limit)` — the second is informational, not a failure. A bare-jar difference with ZERO budget trips IS a failure (the probe is then not the jar).
2. **Task 1 size.** ~~A 19th task is authorised: split `fr_router::score` into 1a and 1b.~~ **Superseded by scan ruling 11: keep 18 tasks, 0-17, and do not split Task 1.** The task's implementer may land it as **two commits** (the counters/traversal ctor, then `getNormalizedScore` + the weights + `p7t7`) if that helps review, but the task number, the dispatch order and the §Interfaces index all continue to say **Task 1**.
3. **Plan 8 and `fr_router::{score, pipeline}`.** Re-export (default). Plan 8 owns only the JSON/manifest surface and the CLI/MCP wrappers.
4. **`structural_hash` widening cost.** Correctness over speed: widen to the full `serialize(true)` field set even if it walks the item graph; measure and record the per-call cost in the task report; a later optimisation is Plan 8's if the CLI needs it. Decision parity (ruling AH) is the gate, not wall clock.
5. **Quirk #162.** Confirmed: stays unfixed in Plan 7 (a guard Java lacks breaks parity). `timeout(1)`/the driver's wall-clock bound is the only protection; the hang rows are XDIFF-by-hang with the quirk citation, exactly as Plan 6 handled them.

**Controller amendment (ruling AW, 2026-08-30):** insert **Task 15b** (controller-authored brief in the SDD workspace: the `HeadlessBoardManager` clearance overrides) between Tasks 15 and 16 — Java-side measurement (scratchpad plan8-evidence/job1) proved `applyCopperToEdgeClearanceOverride` mutates 15/16 corpus boards, so Task 16's references are invalid without it. Dispatch order becomes … 15 → 15b → 16 → 17. Task 16 must call `fr_router::pipeline::prepare_board` in its driver exactly where the jar's `-dr` flow does.

> **Amendment (Task 13, 2026-09-01):** plan-7 ruling 8 (the clone-based optimizer snapshot) is corrected by measurement: `BasicBoard.undo` never touches `communication.idGenerator`, so a faithful restore must carry the LIVE `communication` across the clone-restore or every item id after the first unimproved item diverges. The port does so (`pipeline/optimizer.rs`, Task 13). The restored search-tree SHAPE differs from Java's remove-then-reinsert topology, and **Task 14b measured that it is not inert** — the Task 13 review's §4 insulation argument covers `MinAreaTree.overlaps` only, and `ShapeSearchTree45Degree.completeShape`'s traversal prune is order-dependent, so the two sides complete free-space rooms differently after the first unimproved item and later burn different numbers of item ids. Quirk **#229**, open; ruling AZ scoped Task 14b to localisation. See `.superpowers/sdd/2026-08-30-plan-7-router-batch/task-14b-report.md`.
