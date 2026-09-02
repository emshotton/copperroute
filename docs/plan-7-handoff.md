# Plan 7 hand-off — the batch router (fanout, passes, optimizer)

Branch `plan-7-router-batch`, 18 tasks (0–17) plus four controller-inserted ones
(8b, 14b, 14c, 15b). Written by Task 17 on 2026-09-01. Format follows
`docs/plan-6-handoff.md`: every ruling with a one-line rationale and what
execution confirmed or corrected, then the obligation register for Plan 8.

Read this file first, then `crates/fr-router/README.md` (the crate-local
detail, in task order), then `docs/java-quirks.md` (the register, contiguous
1..235).

---

## 1. The headline: whole-board SES byte parity, on eight boards

Plan 6 proved **per-connection** parity: 369 connections on five boards,
byte-identical to the HEAD jar. Plan 7 proves the layer above it.

`crates/fr-router/tests/reference/<stem>/batch.ses` is the HEAD jar's verbatim
output for a whole `java -jar <jar> -de <dsn> -do <ses> -mp <n>` run — fanout,
every routing pass, the optimizer, the SES writer — and the port reproduces it
**byte for byte on all eight stems**, through the jar's own flow:
`fr_settings::resolve_headless` on the same `argv` → `pipeline::prepare_board`
(ruling AW) → `fr_router::pipeline::run_pipeline` → `fr_dsn::ses_writer::write`.

Ruling 1's ladder, per stem: **(a)** identical pass count and per-pass
`(normalized score, incomplete count, violation count, via count, trace count)`
tuple; **(b)** identical item set after every pass; **(c)** byte-identical SES.

| stem | dsn | `-mp` | fanout | optimizer | lane | (a) | (b) | (c) | passes | SES |
|---|---|---|---|---|---|---|---|---|---|---|
| `router-rpi-splitter` | `Issue143-rpi_splitter.dsn` | 8 | on | on | CI | ✅ | ✅ | ✅ | 3 | 3 654 B |
| `router-j2-reference` | `Issue026-J2_reference.dsn` | 99 | on | on | CI | ✅ | ✅ | ✅ | 2 | 15 254 B |
| `router-ecc83-input` | `Issue649-kicad_ecc83-pp_input_board_v1.dsn` | 8 | on | on | CI | ✅ | ✅ | ✅ | 2 | 4 509 B |
| `router-empty-board` | `empty_board.dsn` | 1 | off | off | CI | ✅ | ✅ | ✅ | 1 | 212 B |
| `router-dac2020-bm01` | `Issue508-DAC2020_bm01.dsn` | **2** | on | on | slow | ✅ | ✅ | ✅ | 2 | 66 387 B |
| `router-tutorial-board` | `examples/tutorial_board/tutorial_board.dsn` | 8 | on | on | slow | ✅ | ✅ | ✅ | 1 | 462 B |
| `router-fanout-bm11` | `Issue730-DAC2020_bm11.dsn` | 2 | on | **off** | slow | ✅ | ✅ | ✅ | 2 | 56 155 B |
| `router-strict-drc-cnh` | `Issue555-CNH_Functional_Tester_1.dsn` | 2 | on | on | slow | ✅ | ✅ | ✅ | 2 | 40 915 B |

Ruling AM's escape hatch — an `XDIFF` row carrying the first differing byte and
a normalised digest — is **unused**, and `every_stem_reaches_rung_c` is what
stops it being quietly re-entered.

Three qualifications, all of them load-bearing for Plan 8:

* **`router-dac2020-bm01`'s `-mp 2` is a ceiling, not a choice (quirk #234).**
  From pass 3 that board's own jar run is **not reproducible**: the four
  `TIME_LIMIT_TO_PREVENT_ENDLESS_LOOP = 1000` constants are `static final int`
  with constant initialisers, so `javac` inlines them (`javap -c -p` shows
  `sipush 1000` before every `optChangedArea` call and **no** `getstatic`) and
  no flag or reflection can switch them off. Measured, same board, same
  settings, same `-XX:hashCode=2`: `-mp 3` gives `score=841.0115 incompletes=31`
  three times, `-mp 20` gives `835.88336 incompletes=32` three times, and the
  same `-mp 20` run with the jar's DEBUG log on (which slows it) is back to
  `841.0115` with **one** recorded trip. The port is self-consistent at every
  `maxPasses` because it runs `RouterBudget::disabled()` — Java's own "off"
  value. So at `-mp 3` and above **a divergence on this board is likely and
  unmeasurable**: there is no jar answer to compare against. Capping at 2 is the
  honest bound, not a green result hiding a red one.
* **Rung (b)'s committed half is counts, not sets.** `batch_parity.rs` compares
  per-pass via and trace counts and the SES's `(wire`/`(via ` scope counts,
  because the committed reference has no per-pass item *sets*. The full-strength
  statement — every item's id, layer, half width, polyline and corner list after
  every pass — is `scripts/differential/sweep-p7t9.sh`, **32/32 MATCH**, and the
  final item set is pinned exactly by rung (c) anyway.
* **The references are probe-generated, and `--verify-driver` is what makes that
  safe** (controller answer 1). `P7T9.java` re-implements eight lines of
  `RoutingJobSchedulerActionThread` around `RoutingPipeline.createForHeadless`;
  `gen-batch-reference.sh --verify-driver` runs the **bare jar**'s `-de/-do` with
  the budget left at Java's 1000 and byte-compares. **8/8 `bare-jar: identical`,
  0 trips.** A bare-jar difference with zero recorded trips would be a failure —
  the probe would then not be the jar.

### Cross-platform determinism, proven not assumed

`em@workbench` (16-core x86_64 NixOS, OpenJDK 25.0.4 from `nix shell nixpkgs#jdk25`)
regenerated `router-rpi-splitter`'s `router.jsonl` reference **from scratch** and it
is **sha256-identical** to the file committed from arm64 macOS; `p6t3` and `p6t1`
MATCH there too. So the harness and the port are platform-independent, not merely
self-consistent on one machine. `RUNNER.md` on that box records the setup. Plan 8
can use it for long runs (`rsync` the repo first).

### Hash-mode independence

`gen-batch-reference.sh --verify-hash-modes` regenerates every stem under
`-XX:hashCode=0..4` and requires five byte-identical SES files. **8 stems × 5
modes, one digest each, all identical** — Plan 6's premise re-checked now that
fanout and the optimizer are live and both walk `HashSet<Item>` over a type with
no `hashCode` override.

---

## 2. Delivered

| area | Java | port |
|---|---|---|
| the score | `core/scoring/BoardStatistics.java` (score subset) + 10 DTOs, `calculateScore`, `getMaximumScore`, `getNormalizedScore` | `fr_router::score` (Task 1, ruling AG) |
| the best-board memory | `autoroute/BoardHistory.java` | `fr_router::pipeline::BoardHistory` (Task 2, ruling AF) |
| the board hash | `BasicBoard.getHash()` / `serialize(true)` | `Board::structural_hash`, widened (Task 3, ruling AH) |
| the stop machine | `core/StoppableThread.java`, `StopRequestState.java`, `RouterCounters.java`, `ProgressThrottler.java` | `pipeline::{RouterStop, StopRequestState, RouterCounters, ProgressThrottler, RouterBudget}` (Task 4, ruling AI) |
| progress | `NamedAlgorithm`'s 3 listener lists + `autoroute/events/**` | `pipeline::{ProgressSink, NoopProgressSink, RoutingEvent}` (Task 4, ruling AK) |
| the tightener sweep | `RoutingBoard.optChangedArea` ×2, `RoutingBoardOperations.optChangedArea`, `PolylineTrace.{check,correct,swap}ConnectionToPin` | `RoutingBoardExt::opt_changed_area{,_with_keep_point}`, `PolylineTraceExt::*` (Task 5) |
| via optimisation | `board/optimize/ViaOptimizer.java` **whole** | `board_ext::via_optimizer` (Tasks 6, 7) |
| the connection router's top half | `AutorouteConnectionRouter.route` steps 6-8, `retryConnectionNecked`, `applyStrictDrcAfterRoute` | `route_connection_full` (Task 8) |
| the board facade's batch callers | `RoutingBoard.removeItemsAndPullTight`, `RoutingBoard.fanout` | `RoutingBoardExt::{remove_items_and_pull_tight, fanout}` (Tasks 8, 11) |
| the autoroute pass | `AutoroutePassRunner.runSingleThread`, `BatchAutorouter.getAutorouteItems`/`autorouteItem`/`removeTails`, `RoutingFailureLog`, `ItemRouteResult`, `AutorouteAirlineCalculator.calculateAirline` | `pipeline::{AutoroutePassRunner, BatchAutorouter, RoutingFailureLog, ItemRouteResult, calculate_airline}` (Tasks 8, 9) |
| the pass loop | `AutorouteBatchLoop.run` (571 lines, one method) | `pipeline::AutorouteBatchLoop::run` + `BatchLoopResult` (Task 10) |
| the fanout stage | `BatchFanout.java` whole, incl. `fanoutBoard` ×2, `fanoutPass`, `Component`/`Pin` and the three records | `pipeline::fanout` (Tasks 11, 12) |
| the optimizer | `BatchOptimizer.java` whole, incl. `ReadSortedRouteItems`, `optRouteItem`, `runBatchLoop`, `optRoutePass` | `pipeline::{BatchOptimizer, ReadSortedRouteItems, OptimizerResult}` (Tasks 13, 14) |
| undo's tree side effects | `BasicBoard.{generateSnapshot, popSnapshot, undo}` + `applyUndoRedoSideEffects` | `Board::{begin_undo_journal, discard_undo_journal, undo_from_snapshot, journal_insert, journal_remove, save_for_undo}` + `UndoJournal` (Task 14c, ruling BA) |
| the pipeline | `RoutingPipeline.run`, `AutorouteUnroutedReport.build` | `pipeline::{run_pipeline, PipelineResult, build_unrouted_report}` (Task 15, ruling AK) |
| the clearance overrides | `HeadlessBoardManager.apply{CopperToEdge,Hole}ClearanceOverride`, `applyRouterSettingsForLoadedBoard` | `fr_board`'s `clearance_override.rs` + `pipeline::prepare_board` (Task 15b, ruling AW) |
| the reference family | — | `scripts/gen-batch-reference.sh`, `p7t9`, `sweep-p7t9.sh`, `tests/batch_parity.rs` (Task 16, ruling AM) |
| ownership fix | `ViaRule` / `ViaInfos` re-pointing (ruling H) | `ViaRule` holds `Vec<ViaInfo>` (Task 0, ruling AL); `NetClass` owns its rule (Task 11, ruling AN) |

**Not built, deliberately, each with the evidence at the marker:** the five
multithread classes (quirk #143, extended — see §7); `AutorouteRuntimeMetrics`'
JMX samplers; `autoroute/events/**`; `NamedAlgorithm`'s listener plumbing;
`RoutingPipeline.createForGui` and the GUI factories; `BatchOptimizer`'s three
JMX samplers and `BatchAutorouter`'s profile counters and `threadIndexToLetter`;
`AutoroutePassRunner`'s five `log*` helpers; `RoutingFailureLog`'s four
caller-less methods (quirk #235); `autoroute/BoardHistoryEntry.java` (quirk
#199); `RoutingBoard.forcedVia` and `moveDrillItem` (GUI-only, greps at the
markers); `BatchAutorouterDebugTest` (its 21 tests drive `debug/DebugControl`).

---

## 3. Public API surface (what Plan 8 calls)

```rust
// the whole pipeline — the one entry point Plan 8 wraps
pub fn run_pipeline(
    board: &mut Board,
    settings: &RouterSettings,
    stop: &RouterStop,
    budget: RouterBudget,
    progress: &mut dyn ProgressSink,
) -> Result<PipelineResult, RouterError>;

pub struct PipelineResult {
    pub router_state: TaskState,
    pub optimizer_state: Option<TaskState>,
    pub passes_run: i32,
    pub fanout: Option<FanoutRunSummary>,
    pub per_pass: Vec<PassRecord>,
    pub final_statistics: BoardStatistics,
    pub timed_out: bool,
}

// the three stages, if a caller ever needs one alone
impl AutorouteBatchLoop { pub fn run(board, settings, stop, budget, progress)
      -> Result<BatchLoopResult, RouterError>; }
impl<'a> BatchFanout<'a> { pub fn fanout_board(board, settings, stop, budget, progress)
      -> Result<FanoutRunSummary, RouterError>; }
impl<'a> BatchOptimizer<'a> { pub fn run_batch_loop(&mut self, board, stop, budget, progress)
      -> Result<OptimizerResult, RouterError>; }

// the stop machine — THREE states, not a bool
pub enum StopRequestState { None, AutoRouterOnly, All }
impl RouterStop {
    pub fn new() -> RouterStop;                    // no deadline
    pub fn with_deadline(limit_ms: i32) -> RouterStop;
    pub fn request_stop(&self);                    // -> All
    pub fn request_stop_auto_router(&self);        // -> AutoRouterOnly
    pub fn is_stop_requested(&self) -> bool;       // == All
    pub fn is_stop_auto_router_requested(&self) -> bool; // != None
    pub fn state(&self) -> StopRequestState;
    pub fn poll_deadline(&self) -> bool;           // job-level sites ONLY
    pub fn is_timed_out(&self) -> bool;
}

// ruling AI's budget knob — every parity run passes `disabled()`
impl RouterBudget {
    pub fn disabled() -> RouterBudget;
    pub fn opt_changed_area_limit(&self) -> Option<TimeLimit>;
    pub fn fanout_limit_for_pass(&self, pass_no: i32) -> TimeLimit;
    pub fn board_update_throttler(&self) -> ProgressThrottler;
    pub fn progress_throttler(&self) -> ProgressThrottler;
}

// spec §10's observer replacement (ruling AK, ruling 11: no port decision reads it)
pub trait ProgressSink { fn on_event(&mut self, event: RoutingEvent); }
pub struct NoopProgressSink;
pub enum RoutingEvent { /* one variant per `fire*` call the port keeps */ }
pub enum TaskState { /* Java's, with `ordinal()` */ }
pub enum NamedAlgorithmType { /* ditto */ }

// the score (ruling AG) — re-exported by Plan 8, never re-declared (ruling 4)
impl BoardStatistics {
    pub fn calculate_score(&self, scoring: &ScoringSettings) -> f32;
    pub fn maximum_score(&self, scoring: &ScoringSettings) -> f32;
    pub fn normalized_score(&self, scoring: &ScoringSettings) -> f32;
}

// board preparation — Plan 8's load path MUST call this (ruling AW)
pub fn prepare_board(/* … */);

// the unrouted report
pub fn build_unrouted_report(board: &Board) -> String;
pub fn normalize_router_algorithm(algorithm: &str) -> String;
```

`crates/fr-router/src/lib.rs` re-exports all of it and `prelude` gathers it.
`route_connection`'s signature is **unchanged** (ruling 2); `route_connection_full`
was added beside it, which is why all 369 connections of Plan 6 evidence still
attach.

---

## 4. Rulings, with what execution confirmed or corrected

### The controller's pre-plan rulings AF–AM

| ruling | one line | what execution said |
|---|---|---|
| **AF** — `BoardHistory` is ported | the best-board policy is visible in every SES; dropping it writes the *last* pass's board, not the *best* | **Confirmed.** Task 2 landed it and ported `BoardHistoryTest` by name; four quirk rows (#197–#200) came out of it. |
| **AG** — the score lands in Plan 7 as `fr_router::score` | Java-exact including its rounding; no new crate | **Confirmed**, and it moved a Plan 5 obligation: `getNormalizedScore` is no longer Plan 8's to *compute*, only to *wire*. Evidence: `p7t7` MATCH ×11 plus a five-way hash-mode sweep. |
| **AH** — `structural_hash` is audited and widened, **decision** parity not digest parity | reproducing MD5 bytes would be reproducing an implementation; what a board can observe is the three comparisons | **Confirmed, with a correction.** Task 3's first table missed that the closure escapes the item graph once — `Via.padstack` is non-`transient`, so a board with a via also hashes the whole padstack library and the `LayerStructure` under it. Covered by reduction to `PadstackId`, argued in the table. Evidence: `p7t10`, 10 boards × 8 scripts × 2000 steps, **0 decision diffs**, hash-mode independent. The table has **no `?` cells** and its five *skipped* rows are the only judgement calls. |
| **AI** — the wall clock is a poll at Java's own read points; no watchdog thread | a thread would be non-deterministic and would need a threading policy Java does not have | **Confirmed, and sharpened.** The six read sites are **two** mechanisms: one job-level flag (`AutorouteBatchLoop:251`, `AutoroutePassRunner:203`) and four per-stage clocks (`BatchFanout:111,:396`; `BatchOptimizer:172,:308`) that write only their own `isTimedOut`. `poll_deadline` requests `ALL` and has exactly **one** production call site, the job-level one. |
| **AJ** — `retainAutorouteDatabase` is permanently `false` | the five `additionalUpdateAfterChange` markers are dead, not deferred | **Confirmed, and it created a marker kind.** Task 8 found the property is *Java benchmark-only*, so the hook is dead on every live path in **both** languages — stronger than "false in the port". The five became `// not reachable:`, a new greppable marker distinct from `not ported:`. **No `fr-board` signature changed.** |
| **AK** — a minimal `run_pipeline` inside `fr-router` | Plan 7 delivers the stages *and* the sequencer; Plan 8 wraps the result | **Confirmed**, and it went further than the ruling asked: `RoutingPipeline` is now *wholly* accounted for (a `renamed:`, six `not ported:`), so Plan 8 has no `RoutingPipeline` surface left to build. `a_recording_sink_changes_no_board_byte` pins ruling 11. |
| **AL** — ruling H's `fr-board` fix is Task 0 | it changes an ownership model every later task builds on, so it goes first | **Confirmed.** `bde59ef`; the `Issue593` repro is MATCH on all 50 connections, k=6 and k=8 included. |
| **AM** — the acceptance budget and the CI/slow split | a whole-board stem costs seconds to minutes; CI must stay usable | **Confirmed and widened** by scan ruling 13 to eight stems, four in CI. |
| **AM/General** — no rayon; five multithread classes `not ported:` | `autoroutePassMultiThread` has no caller | **Confirmed and widened** — see §7's quirk #143 extension. Task 17 re-ran every grep at HEAD. |

### The plan's own rulings 1–14

| # | one line | outcome |
|---|---|---|
| 1 | parity target is **SES bytes**, with the per-pass tuple as the diagnostic ladder | **Reached on all eight stems.** The `XDIFF` fallback is unused. |
| 2 | the Plan 6/7 seam is unchanged; `route_connection`'s signature is final | **Held.** Task 8 added `route_connection_full` beside it. |
| 3 | `fr-drc` becomes a real dependency of `fr-router` | **Done** (`BoardStatistics`' constructor calls `new DesignRulesChecker(board, null)` twice). One graph change, as spec §4 already drew. |
| 4 | `fr_router::score` is a module, not a crate, and the only home of `getNormalizedScore` | **Held.** Plan 8 re-exports (controller answer 3), never re-declares. |
| 5 | every ordered container gets a recorded decision *before* it is written | **Held, and one hypothesis failed** — see §5. The table is in the README. |
| 6 | `AutorouteAirlineCalculator` is ported at exactly one method | **Confirmed** by a fresh grep: `calculateAirline`'s only live caller is `AutorouteConnectionRouter.java:70`. |
| 7 | one new recovery boundary, and it propagates | **Two, and only one propagates** — scan ruling 9 was right that `:144` is the dead `runMultiThread`'s catch, and wrong that `runSingleThread` therefore has none. See §5. |
| 8 | the optimizer's undo is a `Board` clone | **Corrected by ruling BA** — a clone restores the item state but not the tree topology, and topology decides which obstacles restrain a room. See §5. |
| 9 | `optChangedArea`'s `clipShape != IntOctagon.EMPTY` is a reference comparison | **Confirmed**; quirk #204. |
| 10 | `getAutorouteItems`' multi-net duplication is reproduced exactly, wrong inner index included | **Confirmed**; quirk #213, and `p7t1` is 18/18. |
| 11 | progress is an observer in the strict sense — no port decision reads it | **Held**, pinned by `a_recording_sink_changes_no_board_byte` and its fanout twin. |
| 12 | `ReadSortedRouteItems.next()` is transcribed as an O(n) rescan, twice per item | **Held.** Not memoised; the `currentMinCoor` tie-break depends on the two loops sharing it. |
| 13 | one audit map, extended, and the audit reaches zero for six more directories | **Held and exceeded**: 33 invocations, 0/0. |
| 14 | four ported Java suites plus the fixture assertion family | **All four accounted for** — see §8. |

### The seventeen pre-flight scan rulings

All seventeen were ruled on before Task 1 dispatched and applied to the plan in
place, each marked "(scan ruling N)" at its site
(`.superpowers/sdd/2026-08-30-plan-7-router-batch/preflight-scan.md`, commit
`a4b6331`). Listed here because five of them changed what a task built and three
turned out to be wrong.

| # | one line | outcome |
|---|---|---|
| **1** | quirk labels renumbered wholesale **#189–#212 → #194–#219** — the register was contiguous through #193, not #188, so all 24 plan labels collided with committed rows | **Applied.** Ids were still allocated in write order per ruling Z, so labels and ids diverge again; the complete map is §6. |
| **2** | the marker gate is **two greps** (`crates/*/src`, `crates/*/tests`), not `crates/`, which sweeps README prose; plus an owner table for ~36 orphans | **Applied and green.** Both greps empty. |
| **3** | Task 6 also ports `RoutingBoard.moveDrillItem:252-295`; reviewer sonnet → opus | **Premise false** (§5.4): the method is GUI-only and `ViaOptimizer` calls `DrillItemMover.{insert,check}` directly. Not ported. The reviewer upgrade stood. |
| **4** | `AutorouteControl::from_settings` replaces the non-existent `new_for_net`; with `is_routed` that makes the `pub seam:` gate **six**, not eight | **Half right** (§7): `from_settings` closed, `is_routed` deliberately did not, and seven seams were added. 14, reconciled. |
| **5** | Task 5 ports `correct` + `swap` only — `checkConnectionToPin` is "already ported in Plan 6"; ~400 → ~336 lines | **Wrong, and withdrawn** (§5.3). It had not landed; all three are Task 5's. The Task 5 review adjudicated for the implementer and the plan's amendment bullet was struck. |
| **6** | `PassRecord` moves to Task 4; `build_unrouted_report` gets a Task 10 `obligation:` stub, because Task 10's stagnation exits need both before Task 15 produces them | **Applied.** Both landed where the ruling put them; Task 15 discharged the stub. |
| **7** | a struct is declared by the **earliest** task that writes methods on it — `BatchFanout` + `FanoutRunSummary` → Task 11, `BatchOptimizer` → Task 13 | **Applied.** No type is declared twice; §3's inventory is the result. |
| **8** | `FanoutPin` is a **`JavaTreeSet`**, not a `BTreeSet` (run-time-keyed comparator, ruling Y's case); ruling 5's "total-and-`final`-keyed" hypothesis recorded as failed | **Right answer, half-wrong reason** (§5.5). The `pinIndex` tie-break runs on every branch, so an unrecognised order is *pure `pinIndex`*, not arbitrary. `JavaTreeSet` anyway. |
| **9** | ruling 7's second recovery boundary struck — `:144` is the dead `runMultiThread`'s catch, so there is no per-item `catch_unwind` | **Half wrong** (§5.2). `:144` really is the dead method's, but `runSingleThread` has its own `try` at `:156`/`:331-335`. Two boundaries, one propagating; the per-*item* prohibition was correct. |
| **10** | `RouterBudget.progress_throttle_ms` conflated 250 and 1000 — `shouldFireBoardUpdate` is 250 ms and `ProgressThrottler` has no constant at all | **Applied.** Task 4 landed two gates: `RouterBudget::board_update_throttler()` (250) and `progress_throttler()` (1000), with `ProgressThrottler::board_update_gate`. |
| **11** | controller answer 2's 19th task is **declined**: 18 tasks stand and Task 1 is not split | **Applied.** Task 1 landed whole (two commits, one task number). |
| **12** | Task 16's generator snippet rewritten — its five `IFS='\|'` fields and its `-Dfreerouting.opt_changed_area_ms=0` flag do not exist; `P7T9.java` is what runs | **Applied**, and quirk #234 later explained *why* no such flag can exist. |
| **13** | the stem set widens from ruling AM's five to **eight**, four in CI (`router-fanout-bm11`, `router-strict-drc-cnh`, `router-empty-board` added) | **Applied.** All eight reach rung (c); §1's table. |
| **14** | `remove_items_and_pull_tight` is "consumed by nobody" — Task 8's text wins over the index's three call sites | **Applied.** The index row was corrected; the method exists for the audit and for Plan 8. |
| **15** | `--verify-driver`'s "if it ever fails the driver is wrong" is struck; **controller answer 1 governs** | **Applied.** Trips are informational; a difference with **zero** trips is a failure. Result: 8/8 identical, 0 trips. |
| **16** | `docs/plan-6-handoff.md` §10 items 2/3/4 gain status lines, and §10.3's two coverage obligations must be recorded as discharged or open by Task 17 | **Applied.** Both **discharged**, with numbers, and each now carries a **standing directed test** rather than a removed counter (Task 17 fix round): `crates/fr-router/tests/locator.rs`'s `the_fanout_arm_is_reachable_and_ends_on_a_drill` and `crates/fr-board/tests/board.rs`'s `the_fanout_via_break_changes_the_connection_set`. |
| **17** | Task 0 is in flight and **blocks all dispatch** until it commits; the working tree did not compile | **Applied.** Task 0 committed (`bde59ef`) before Task 1 dispatched; the two relaxations (4 ∥ 1-3, 5-7 ∥ 1-3) were taken afterwards, with the ruling's own correction that **Task 5 depends on Task 4**. |

### Controller rulings made during execution (AN–BA)

| ruling | one line | where |
|---|---|---|
| **AN** | ruling H's open *via-rule* half folds into Task 11 — build the `(via_rule …)` fixture, measure, then own or close | **Closed**: it diverges; `NetClass` owns its `ViaRule`, `ViaRule::contains` is reference-faithful. Masked on 7/8 corpus boards by a double-default-rule quirk. |
| **AP** | Plan 8's `CancelToken` is an `Arc<AtomicBool>` joining at `RouterStop`'s poll sites | two `// pub seam:` lines in `pipeline/stop.rs`; this module introduces **no atomics** (the state is a `Cell`, the pipeline is single-threaded) |
| **AW** | risk-2 escape: `applyCopperToEdgeClearanceOverride` mutates 15 of 16 corpus boards at default settings, so the overrides must be ported before the SES references — Task 15b inserted | `fr_board`'s `clearance_override.rs`, `pipeline::prepare_board`; quirks #231, #232 |
| **AX** | the `k=175` dac2020 divergence gets a localisation-only task (8b) before Tasks 9/10 | it was **not** the `Line` identity model: `TraceTightener45.smoothenStartCornerAtTrace:481` walks a Java `TreeSet` **descending** and the port walked a `BTreeSet` ascending; last-match-wins picked a different contact |
| **AY** | fix it immediately, same agent | `contacts.iter().rev()`; three sibling sites audited (two already reversed, one order-independent), seven more swept. `p6t1 --steps=1-8` went 9/10 → **10/10**. Quirk #210 |
| **AZ** | the optimizer's failed-attempt id burn gets a localisation-only task (14b) before Task 16 | statement: `BatchOptimizer.java:509`'s `undo(null)` replays through the **live** trees; the port's clone assignment replaced the topology |
| **BA** | fix it: keep the clone for item state, port the **side effects** | Task 14c. Tree-op streams byte-identical over the whole run — 134 910 records on dev-board, 89 629 on J2, where 14b measured 54 and 84 Java-only ops. Five rows DIFF→MATCH, 46/46 drivers, **zero** reference transcripts regenerated |

### The five controller answers to the plan's questions

1. `--verify-driver` failing against the bare jar → **references stay probe-generated**; the check records `bare-jar: identical | budget-tripped (<n>)` per stem, the second informational, and a difference with **zero** trips is a failure. **Result: 8/8 identical, 0 trips.**
2. Task 1's size → **do not split** (superseded by scan ruling 11: 18 tasks stand). Landed as one task.
3. `fr_router::{score, pipeline}` and Plan 8 → **re-export**, do not move. Plan 8 owns only the JSON/manifest surface and the CLI/MCP wrappers.
4. `structural_hash` widening cost → **correctness over speed**; measure and record. Done; decision parity is the gate.
5. quirk #162 → **stays unfixed in Plan 7.** A guard Java lacks changes the room set and breaks parity. `timeout(1)` on the generators is the only protection; hangs are XDIFF-by-hang with the quirk cited. **Confirmed and still true.**

---

## 5. Corrections to the plan discovered during execution

Nine, in the order they cost the most:

1. **Ruling 8 was wrong, and it cost two extra tasks (14b, 14c).** "The optimizer's
   undo is a `Board` clone" restores the same *leaf set* with a different
   `MinAreaTree` *topology*, and `ShapeSearchTree45Degree.completeShape`'s
   shrinking-traversal prune (`:157` against `:263-264`) makes topology decide
   which obstacles restrain a free-space room. From the first `improved=false`
   item on, the two sides complete rooms differently and burn different
   `nextSubstituteTracePiece` ids (dev-board 188 vs 187; J2 511 vs 475). Quirk
   #229, ruling BA, `UndoJournal`. **Plan 8 must not replace it with a clone.**
2. **Scan ruling 9 was half wrong.** `AutoroutePassRunner.java:144` really is the
   dead `runMultiThread`'s catch — but `runSingleThread` (`:151-336`) opens its
   **own** `try` at `:156` and closes it with
   `catch (Exception e) { … return false; }` at `:331-335`. The `sed` window the
   scan quoted stopped one line short. So Plan 7 built **two** boundaries, not
   one, and only `RouterError::NoRoutableLayer` propagates.
3. **Scan ruling 5 was wrong.** `checkConnectionToPin` had **not** landed in Plan 6;
   a search for the name, for `TraceExitRestriction` and for the body found only
   the two deferral markers. All three `ConnectionToPin` methods are Task 5's.
4. **Scan ruling 3's premise was false.** `RoutingBoard.moveDrillItem` is **GUI-only**
   (`MoveComponent.insert` ← `DragItemState.java:56-61`); `ViaOptimizer` calls
   `DrillItemMover.{insert,check}` directly, and Plan 6 Task 10b already landed both.
   Not ported. Task 17 found the same for `RoutingBoard.forcedVia` (one caller,
   `gui/interactive/Route.java:294`), where the plan's owner table had expected a
   `renamed:` onto `ForcedViaInserter`.
5. **Scan ruling 8's *reason* was half wrong; the answer was right.**
   `BatchFanout.Component.Pin.compareTo`'s `pinIndex` tie-break at `:773-775` runs on
   **every** branch, so an unrecognised sorting order gives pure `pinIndex` order
   (quirk #220), not an arbitrary one. The container is a `JavaTreeSet` anyway,
   because "no board duplicates a pin index" is a property of the input.
6. **`TIME_LIMIT_TO_PREVENT_ENDLESS_LOOP` cannot be reflected** (quirk #234). Task 8's
   brief required it; `javac` inlines the constant. The port runs the budget disabled
   against the jar's live 1000 ms and **a MATCH is the proof it never tripped**.
7. **`AutorouteConnectionRouter.route`'s `:160-233` names no method.** `:160` is
   `route`'s closing brace. Corrected in place in `lib.rs` and `plan-6-handoff.md`
   §10.1; and "the failure-log write" is `AutoroutePassRunner.runSingleThread:260-289`,
   not this class's.
8. **`removeItemsAndPullTight`'s real signature** carries `tidyWidth`/`pullTightAccuracy`
   and a budget of **2000**, not 1000; `enforceStrictDrc` returns an
   `AutorouteAttemptResult`; `withPreferredDirections` is not a field; `:164-182` is
   five accessors; `getImpactedPoints`' `DrillItem` arm is dead.
9. **The plan's label #196 describes a bug HEAD does not have.**
   `BoardStatistics.java:626-633` opens `getNormalizedScore` with
   `if (maximumScore <= 0f) { return 0f; }`. The row was struck before it was written
   and the *inverse* is pinned (`an_empty_board_scores_zero_not_nan`).

Two **port** defects were found and fixed inside the plan, both recorded as register
rows rather than quietly repaired: **#210** (the tightener's ascending contact walk,
ruling AY) and **#211** (`reduceNetsOfRouteItems` breaking after one net where Java
removes every unsupported one). **#229** is the third, the largest, and is closed.

---

## 6. Quirks pinned in Plan 7 (`docs/java-quirks.md` #194–#235)

Forty-two rows. The register is **contiguous 1..235**, no gap and no duplicate
(`grep -o "^| [0-9]\+ |" docs/java-quirks.md`), and §Process notes names **#236**
as the next free id. The complete plan-label → landed-id map is in §Process
notes under "Plan 7's complete label → id map (Task 17, ruling Z)", including
the three labels that took **no id** (#196, struck; #219, a port-side citation
error corrected in place; #215, a performance observation, not a defect) and the
one that extended an existing row instead of taking a new one (#216 → quirk
**#143**).

The ones Plan 8 must read before writing a line of CLI:

* **#202** — `--max-items` writes `ALL` and `--max-passes` writes `AUTO_ROUTER_ONLY`,
  and `RoutingPipeline:117` tests only the first, so `--max-items` **silently
  disables the optimizer stage**. The CLI must document it.
* **#214** — a *normal* end of routing reports `CANCELLED`, not `FINISHED`. Only a
  fully-routed board reaches `FINISHED`.
* **#227** — the two stages share one stop flag and **nothing lowers it**; the
  optimizer therefore runs, visits every item and changes nothing after any ordinary
  router run. `run_pipeline` reproduces that. Do not "helpfully" reset it.
* **#230** — `job.getCurrentPass()` under-reports by one on a `maxPasses`-capped exit,
  disagreeing with the final `TaskStateChangedEvent`'s own `getPassNumber()`.
* **#231/#232** — the clearance overrides run **twice** per DSN load and the copper
  path is idempotent across the two while the hole path is not. #232's boundary
  (`router.hole_clearance_um > 0` + zero circular keepouts) is an open Plan 8
  obligation with an `obligation:` marker at the site.
* **#233** — `TestingSettings`' constructor zeroes `copperToEdgeClearanceUm`, so the
  whole Java fixture suite routes a board no real `-de/-do` run produces. The parity
  references deliberately do **not** carry it.
* **#234** — the inlined 1000 ms budget; see §1.
* **#143 (extended)** and **#235** — the dead multithread family; see §7. **Cite #143, not
  #216**: the plan's *label* for this finding was #216, but the register spent that id on
  `alreadyRoutedBoardHashes` (Task 10), so #216 is a different quirk. Every id in this
  bullet list is a landed register id.

---

## 7. The obligation register for Plan 8

### The marker inventory, measured on the committed tree

| grep | `crates/*/src` | `crates/*/tests` | `crates/` (adds README prose) |
|---|---|---|---|
| `added in Plan 7` | **0** ← the gate | **0** ← the gate | 3, all past-tense narrative |
| `added in Plan 8` | 32, of which **30** are markers proper and 2 are prose | 0 | 40 |
| `obligation:` | 60 | 6 | 86 |
| `pub seam:` | 14 (all `fr-router`) | 0 | 22 |
| `not reachable:` (new in Plan 7) | 18 | 2 | 23 |
| `item_tree_shape_ref`\|`item_tile_shape_ref` in `crates/fr-router/` | **0 call sites**; 3 hits, all prose about the rule | | plan-6 ruling 10 holds |

*(Measured on the committed tree, this file and the READMEs included — plan-6
§10's warning that `crates/` also sweeps prose about the markers applies to every
row. The `crates/*/src` column is the inventory.)*

The marker gate is **two greps** (`crates/*/src` and `crates/*/tests`), not
`crates/`, which additionally sweeps the READMEs' prose *about* the convention
and can never be driven to zero without deleting the explanation — plan-7 scan
ruling 2, correcting plan-6 §10. Both are empty.

### The 30 `// added in Plan 8:` markers, by home

| file | n | what |
|---|---|---|
| `fr-board/src/board/clearance_override.rs:399-407` | 9 | `HeadlessBoardManager`'s `fr-core` methods: `createBoard`, `loadFromSpecctraDsn`, `applyParsedBoardResult`, `loadFromKiCadJson`, `saveAsSpecctraSessionSes`, `calculateCrc32`, `getRoutingBoard`, `replaceRoutingBoard`, `getCurrentRoutingJob` |
| `fr-router/src/score/mod.rs:51-58` | 8 | ruling 4's score surface: `BoardStatistics(byte[], FileFormat)`, `countOccurrences`, the Gson `toString`, `BoardScoreBreakdown.of`/`toSummaryString`, `ScoringWeightComparison.compare`/`isCandidateBetter`/`toReportString` |
| `fr-drc/src/lib.rs:114-119, :144` | 7 | the KiCad JSON family (`KiCadJsonReader.readBoard`/`importSession`/2 helpers, `KiCadJsonWriter.write`, `KiCadBoardJson`) and `board/state/BoardComparator.java` |
| `fr-board/src/board/mod.rs:57` | 1 | `BoardComparator` from the `fr-board` side |
| `fr-dsn/src/lib.rs:12` | 1 | `SessionToEagle.getInstance` (627 loc) |
| `fr-router/src/pipeline/{batch_autorouter.rs:156, optimizer.rs:372}` | 2 | `NamedAlgorithm.job` — `core/RoutingJob`, Plan 8's type |
| `fr-settings/src/lib.rs:193` | 1 | `TextManager.parseTimespanString` |
| `fr-settings/src/sources/cli.rs:582` | 1 | the `legacy.rs` call site — the legacy-CLI value-normalisation **wiring** half |

### The `obligation:` markers that name Plan 8 explicitly

| site | what |
|---|---|
| `fr-board/src/board/clearance_override.rs:60` | quirk #232's boundary — `applyHoleClearanceOverride`'s second invocation with `router.hole_clearance_um > 0` and zero circular keepouts does one extra `reinsertTreeItems` (a #229-class tree-order shift). Pin or reproduce **before** the CLI exposes the setting. |
| `fr-drc/src/report/json.rs:57` | `DrcJsonFlavor`'s CLI default (plan-5 ruling 2/ruling W) |
| `fr-dsn/src/parser/wiring.rs:596` | `read_via_scope` still calls the unchecked `insert_via`; the last line of the ladder-hang obligation |
| `fr-settings/src/resolve.rs:193` | `RoutingJobScheduler.scheduleJob` — the API path composes the merge differently and must not call `resolve_headless` |
| `freerouting/src/mcp/{server.rs, stdio.rs}` | MCP concurrency: the handler can emit one message and holds no connection reference |

The other 55 `obligation:` markers in `src` are within-plan coverage notes, 15 of
which say "discharged" in place; `crates/fr-router/README.md` §"The 27
`obligation:` markers" tabulates the router's class by class.

### The `pub seam:` gate, reconciled

The plan expected **six**; the tree has **14**. One of scan ruling 4's two
predicted closures happened (`AutorouteControl::from_settings`, Task 11) and one
did not: `AutorouteAttemptResult::is_routed` stays open **on purpose**, because
the port reads the enum directly, exactly as Java's `result.state == ROUTED`
chains do. Two greps, both reproducible on the committed tree:
`grep -rno "AutorouteAttemptState::" crates/fr-router/src` minus
`autoroute/attempt.rs` gives **30 hits across five files**
(`autoroute/maze/engine.rs` 15, `board_ext/routing_board_ext.rs` 6,
`pipeline/fanout.rs` 4, `pipeline/pass_runner.rs` 4,
`pipeline/batch_autorouter.rs` 1), and `is_routed()` outside that file has
**zero** hits in the crate or its tests. Routing those through a helper would
invent a shape Java does not have. Seven were added — two for ruling AP's
`CancelToken` seam and five for `ViaOptimizer`'s Java-private methods that an
integration test and `p7t4.rs` both call from outside the crate. **8 − 1 + 7 = 14.**
The row-by-row table is `crates/fr-router/README.md` §"The `pub seam:` gate,
reconciled".

### What Plan 8 builds (from the draft in the session scratchpad)

A full 15-task plan (1 149 lines as of 2026-09-01; the ledger's 1 125 predates its
controller answers) is already drafted and cross-checked against these markers:
`2026-08-30-plan-8-core-cli-mcp.md` in the session scratchpad, with rulings
AO–AV and the risk-2 ruling in `plan8-rulings.md`, the survey in
`survey-plan8.md`, and banked evidence in `plan8-evidence/` (job 1: 15 of 16
corpus boards mutated by the clearance override; job 2: 87 argv shapes across
four disagreeing parsers; job 3: the Java MCP baseline — 28 tools, protocol
`2024-11-05`, auth and blank-line traps). Its shape: `fr-core` (`CancelToken`,
`ProgressSink` re-export, `Ctx`/`RoutingResult`, the timeout ladder), the job
model, the score's JSON surface, the load/save sequence, the result manifest, the
CLI (legacy shim + native subcommands + exit ladder), `freerouting route` and
`freerouting drc` end to end, the KiCad JSON reader/writer, and the MCP transport
and tools. **The MCP server is new code** — Java has only a stdin→HTTP bridge —
and the ruling on it is **no `schemars`; hand-written JSON schemas.**

**`post-parity-roadmap.md`** (session scratchpad, 721 lines) is Plan 8's other
import: the quirk-fix roadmap, four tiers (T1 crashes/hangs/OOM 31 rows, T2
routing quality 78, T3 DRC/report 18, T4 settings/CLI 30), each row with an
evidence bar and a `Compat` switch design. It is **not** a Plan 8 task list — it
is what comes after parity, and Plan 8 imports it rather than executing it.

### A gap the audit surfaced and nobody has claimed

`settings/sources/JsonFileSettings.java` prints `ROSTERED` on the
`settings/sources` invocation: **three public methods, all `not ported:`**. It is
the JSON settings *file* source — `freerouting.json` / `--settings <file>` — and
it is a real hole in the CLI's settings ladder, not a GUI class. Plan 8's Task 5
(the CLI surface) is where it belongs; it is not in the draft's task bodies today.

---

## 8. Test and evidence inventory

| what | count / result |
|---|---|
| workspace tests | see §11's verification block |
| `audit-port.sh` invocations | **33**, all exit 0, **0 `MISSING`, 0 `UNMAPPED`**, 26 `ROSTERED` across 10 *(as of Plan 7 Task 17; see the status line below)* |
| differential drivers added | 10 (`p7t1`–`p7t10`), each a genuine Java-vs-Rust pair, budget disabled on both sides |
| probes added | 5 (`P7T2Probe`, `P7T4Probe`, `P7T5Probe`, `P7T8Probe`, `P7T9Probe`) — Java-only, whose stdout a Rust test pins as literals |
| `p6t1` extended | a fifth argument, `--steps=1-8`; **10/10** after ruling AY |
| whole-board sweep | `sweep-p7t9.sh`, every stem × every mode, **32/32 MATCH** |
| bare-jar verification | `--verify-driver` **8/8 identical, 0 budget trips** |
| hash-mode verification | `--verify-hash-modes` **8 stems × 5 modes**, one digest each |
| Plan 6's ladder, still green | `reference_parity` — the per-stem connection counts unchanged (8 + 294 + 294 + 45 + 0 + 22) |
| `p3t15` sweep | 525 MATCH + 5 XDIFF, unchanged |

The named tests behind the rulings, so a reviewer does not have to find them:

| ruling | test |
|---|---|
| AI — the budget really does trip when it is *not* disabled | `crates/fr-router/tests/stop_and_progress.rs`'s `the_opt_changed_area_budget_trips_and_disabling_it_removes_the_trip`, `a_zero_opt_changed_area_budget_is_javas_no_limit`, `the_budget_defaults_are_javas_literals`, `the_disabled_budget_turns_every_wall_clock_off`, and `tests/opt_changed_area.rs`'s `the_budget_trips_the_sweep` |
| AJ — `retainAutorouteDatabase` has no setter and is false everywhere | `crates/fr-router/tests/batch_autorouter.rs`'s `retain_autoroute_database_has_no_setter` and `retain_autoroute_database_is_false_on_every_path`; **no `fr-board` signature changed** |
| AK — the sink is an observer in the strict sense | `crates/fr-router/tests/stop_and_progress.rs`'s `a_recording_sink_changes_no_board_byte` and `tests/fanout.rs`'s `a_recording_sink_changes_no_fanout_byte` |
| AM — the ladder's own guard rails | `tests/batch_parity.rs`'s `every_stem_reaches_rung_c`, `the_driver_matches_the_bare_jar`, `references_are_from_the_head_jar`, `the_stem_table_matches_the_fixture_file`, `the_ses_normaliser_touches_only_the_parser_keywords` |
| H — closed, both halves | Task 0's transcript `crates/fr-router/tests/data/p7t0-ruling-h-match.txt` (50/50 MATCH, k=6 and k=8 included) and Task 11's `tests/fanout_order.rs` (`the_combined_via_rule_appends_a_value_equal_via_from_a_second_rule`) |
| plan-6 §10.3 — the two discharged coverage obligations | `crates/fr-router/tests/locator.rs`'s `the_fanout_arm_is_reachable_and_ends_on_a_drill` and `crates/fr-board/tests/board.rs`'s `the_fanout_via_break_changes_the_connection_set` |

**Both §10.3 tests are directed, not corpus-derived, and that is deliberate.** Task 17
first measured each arm with a temporary counter (32 hits and 1 hit respectively) and
then, in its fix round, replaced the counters with tests. The corpus form was tried for
the second arm and **fails**: a fanned-out `Issue143-rpi_splitter` produces *zero*
disagreeing items, so "some board reaches it" is a property of one stem at one
connection and would be as brittle as the count. The directed form asserts what the
obligation actually filed — the arm is reachable and its effect is real — and cannot go
cold when the corpus changes. The standing guard that a *corpus* board still reaches
each arm **and gets the jar's answer there** is the SES byte-parity ladder, which is
strictly stronger than a hit counter and is already in CI.

**The four Java suites of ruling 14, each with its Java file:line in a doc comment:**

| Java suite | port |
|---|---|
| `autoroute/BoardHistoryTest.java` (`:27-141`) | `crates/fr-router/tests/board_history.rs:915-1060` (Task 2) |
| `autoroute/StrictDrcEnforcementTest.java` (`:23-79`) | `crates/fr-router/tests/strict_drc.rs` (Task 8) |
| `fixtures/RoutingPipelineComparisonTest.java` (`:49-76`) | `crates/fr-router/tests/pipeline.rs:104-105` (Task 15) |
| `autoroute/BatchAutorouterDebugTest.java` (521 loc) | **deliberately not ported** — `crates/fr-router/tests/pass_runner.rs`'s module doc carries the grep showing its 21 `@Test` methods drive `debug/DebugControl`, not the router; the one member with a headless counterpart, `DebugSettings.isNetPermitted`, is ported and tested in `fr-settings` |

---

## 9. Contracts Plan 8 must not break

1. **`CancelToken` maps *onto* `RouterStop` and the three states survive** — see §3
   and quirks #202/#214. A bool loses the `--max-items` / `--max-passes`
   distinction, which is board-observable.
2. **Quirk #227's no-reset.** `RoutingPipeline.run` hands both stages one
   `job.thread` and `StoppableThread` has no `clear`. Do not add one.
3. **The `Line` identity token (plan-6 ruling AE, quirk #74).** One reader,
   `Line::is_same_object`; one caller, `Board::change_trace`. Anything else using
   it to stand in for `==` is a bug. Nothing reads the counter's *value*, so runs
   stay bit-reproducible.
4. **`JavaTreeSet`, not `BTreeSet`, wherever the comparator is not a total order**
   (plan-6 ruling Y, plan-7 scan ruling 8): `SortedRoomNeighbour` (#160),
   `MazeListElement` (#171), `BatchFanout.Component.Pin` (#220). The two containers
   drop *different* elements and emit them in a different order.
5. **`UndoJournal` (ruling BA, quirk #229).** `Board::{begin_undo_journal,
   discard_undo_journal, undo_from_snapshot, journal_insert, journal_remove,
   save_for_undo}` in `crates/fr-board/src/board/snapshot.rs` is
   `BasicBoard.{generateSnapshot, popSnapshot, undo}` + `applyUndoRedoSideEffects`.
   A whole-board clone assignment is **not** equivalent.
6. **`pipeline::prepare_board` before routing** (ruling AW). 15 of the 16 corpus
   boards are mutated by `applyCopperToEdgeClearanceOverride` at default settings.
   Skipping it routes a different board, silently.
7. **`board.start_marking_changed_area()` before every `route_connection`**
   (plan-6 §10, quirk #177). Carried forward unchanged.
8. **`max_passes == 0` means unlimited** (quirk #140), and the headless path
   `validate()`s twice, which is not idempotent for exactly that field.
9. **`-mt` and `-oit` stay inert** (quirk #143, extended). No threading policy.
10. **`fr_router::{score, pipeline}` are re-exported, not moved** (controller
    answer 3).
11. **Quirk #82 (Delaunay in-circle on axis-aligned input) stays unfixed.** It now
    has four consumers.
12. **Quirk #162 stays unguarded** (controller answer 5). A guard Java lacks changes
    the room set. The generators' `timeout(1)` is the only protection, and a
    `--job-timeout` that is not a watchdog cannot interrupt it.

---

## 10. Parked residuals, by task

| task | residual |
|---|---|
| 0 | owned `ViaInfo` copies were taken over the `ViaInfos`-tombstone alternative; tombstones leak the removal into every index walk. |
| 1 | 20 `float`-cast sites and a Kahan compensated sum whose tail sign is `summands[0] - summands[1]`; `f64` diverges 865/200k, `f32` hides it. `java_min_f64`-style helpers live in `fr-geometry`. |
| 2 | 30 live `Board` clones, not 30 `byte[]`s — measured affordable on `Issue730-DAC2020_bm11`; the note is in the README. A restored board *loses* what Java's `transient` fields lose, matched deliberately. |
| 3 | the widened hash walks the item graph; the per-call cost is recorded in the task report. Controller answer 4 accepted it: decision parity is the gate, not wall clock. |
| 4 | `RouterCounters`' fields are `Option`; `RouterBudget::disabled()` uses `fanout_ms_per_pin = i32::MAX`. |
| 5 | `additional_update_after_change` is per-merge inside `combine()`, not once before it — invisible to every driver (engine is `None` on all of them), fixed anyway. |
| 6 | the stubbed `repositionVia` overload A was made **loud** (`unimplemented!`), never silently mutating — controller ruling B1, the Plan 6 Task 9 precedent. |
| 7 | quirk #207 is **latent on the corpus**: overload A walks toward a trace corner and the trace already obeys the angle restriction. Recorded as measured, not as cleared. |
| 8 | the `#184` stale-clip statement is hit **once** and only with a **null** clip shape; under the clip octagon the skip block is never entered. The row says so and an `obligation:` marker names what would close it. |
| 8b | the level-8 bisect instrument (`p6t17b-bisect.patch`, `P6T1_DUMP_BOARD`) stays committed and off by default. |
| 9 | `scripts/differential/rust` is not rustfmt-clean (160 diffs, 14 files) — Task 17's decision is in §12. A port bug was fixed in passing: `cumulative_trace_length`'s `.sum()` seeded `-0.0`. |
| 10 | four arms lifted out of the 571-line `run`; the fanout stub was a **hard assert**, so Task 12 had to precede Task 15. |
| 11 | ruling H's via-rule half closed here; the divergence was masked on 7/8 boards by a double-default-rule corpus quirk. |
| 12 | Java's stage clock **wraps** at the unreachable extreme where the port answers `None` — a documented divergence at a value no settings ladder produces. |
| 13 | the restored tree's *shape* pre-attempt was measured inert for `p7t8`'s item — and that reading was later **disproved** by Task 14b for the general case (see §5.1). |
| 14 | `OptimizerResult::per_pass` follows Task 10's pattern (controller-confirmed). Quirk #228's coverage is impossible and recorded as such. |
| 14b | localisation only; the fix is 14c. `P7T14B_MAT/_FP/_CS/_MAZE` and `P7T8B_IDS` stay committed and off. |
| 14c | one journal **level** is the whole requirement — `optRouteItem` is the only headless caller and never nests; `redo` has no headless caller at all. |
| 15 | `PipelineResult::passes_run` is **not** `job.getCurrentPass()` (quirk #230). `finishAutoroute` is unreachable per ruling AJ. |
| 15b | quirk #232's boundary is promoted to a Plan 8 obligation rather than pinned here. |
| 16 | two aggregate ladder climbs, not one test per stem — recorded as a deviation, with the reason and the cost. `--verify-driver`'s failure arm had never executed until the fix round forced it (two `set -e` bugs). |
| 17 | the `pub seam:` count is 14, not the plan's 6, and the reconciliation is a table rather than an edited number; the differential crate stays unformatted (§12). |

---

## 11. Evidence — the verification block

Commands run by Task 17 on the committed tree, from
`/Users/em/Development/freerouting/freerouting-rs`:

| # | command | result | exit |
|---|---|---|---|
| 1 | `cargo fmt --all --check` | no diff | **0** |
| 2 | `cargo clippy --workspace --all-targets -- -D warnings` | 0 warnings, 0 errors | **0** |
| 3 | `cargo nextest run --workspace` | **2 137 tests run, 2 137 passed, 54 skipped** (2 135 at the first commit; +2 are the two §10.3 standing assertions added in the fix round) | **0** |
| 4 | `cargo test --workspace --doc` | 7 doc-test targets, all ok | **0** |
| 5 | `cargo doc --workspace --no-deps` | 40 warnings, **all pre-existing** — the identical count on `git stash`ed HEAD, verified in the same session; none in a file this task touched | **0** |
| 6 | the **33** `audit-port.sh` invocations (script below) | **0 `MISSING`, 0 `UNMAPPED`, 26 `ROSTERED` across 10**; every invocation exit 0 *(as of Plan 7 Task 17; see the status line below)* | **0** |
| 7 | `grep -rn "added in Plan 7" crates/*/src` | **nothing** | 1 (no match) |
| 8 | `grep -rn "added in Plan 7" crates/*/tests` | **nothing** | 1 (no match) |
| 9 | `grep -rn "item_tree_shape_ref\|item_tile_shape_ref" crates/fr-router/` | 3 hits, **all prose**, 0 call sites — plan-6 ruling 10 holds | 0 |
| 10 | `grep -rn "pub seam:" crates/fr-router/src \| wc -l` | **14** — reconciled in §7, not edited to match the plan's 6 | 0 |
| 10a | `grep -rno "AutorouteAttemptState::" crates/fr-router/src \| grep -vc autoroute/attempt.rs` | **30** across five files — the evidence for leaving `is_routed` open | 0 |
| 10b | `grep -rn "is_routed()" crates/fr-router/src crates/fr-router/tests \| grep -vc autoroute/attempt.rs` | **0** — no production caller, in the crate or its tests | 1 |
| 11 | `grep -o "^\| [0-9]\+ \|" docs/java-quirks.md \| ...` | **235 rows, contiguous 1..235**, no gap, no duplicate | 0 |
| 12 | `#![forbid(unsafe_code)]` in every crate root | 6 libs + `crates/freerouting/src/main.rs`. `grep -rn unsafe crates/*/src` minus the attribute returns **one** line, `fr-router/src/lib.rs:84` — a doc comment saying where the repository's last `unsafe` lives (`scripts/differential/rust/src/bin/p2t13.rs`, a driver, not a crate). **No `unsafe` code in any workspace crate**; the grep is self-referential, like the marker tables | 0 |
| 13 | `(cd scripts/differential/rust && cargo fmt --check)` | **160 diffs across 14 files, unchanged** — deliberate, see §12.1. The crate is `exclude`d from the workspace (`Cargo.toml:6`) so this does not gate anything | 1 |

Test tooling: `cargo nextest` (user-approved install) with `sccache` as the rustc
wrapper and incremental off (`aeff2f9`). `cargo test --workspace` was used for the
doc tests, which nextest does not run.

The audit sweep, verbatim, so a reviewer can re-run it:

```sh
A=./scripts/audit-port.sh
$A geometry/planar crates/fr-geometry/src
for dir in board/model/items board/model/structure board/facade board/searchtree \
           board/trace board/state rules core/library datastructures; do
  $A "$dir" crates/fr-board/src '*.java' scripts/audit-map/fr-board.map
done
$A drc        crates/fr-board/src 'ClearanceViolation.java'   scripts/audit-map/fr-drc.map
$A management crates/fr-board/src 'HeadlessBoardManager.java' scripts/audit-map/fr-board.map
$A io/specctra        crates/fr-dsn/src '*.java' scripts/audit-map/fr-dsn.map
$A io/specctra/parser crates/fr-dsn/src '*.java' scripts/audit-map/fr-dsn.map
$A io                 crates/fr-dsn/src 'CoordinateTransform.java BoardReadResult.java BoardMetadata.java FileFormat.java KiCadNetClassNames.java' scripts/audit-map/fr-dsn.map
$A datastructures     crates/fr-dsn/src 'IdentifierType.java IndentFileWriter.java' scripts/audit-map/fr-dsn.map
$A settings         crates/fr-settings/src 'RouterSettings.java LayerSettings.java ScoringSettings.java OptimizerSettings.java FanoutSettings.java DesignRulesCheckerSettings.java DebugSettings.java SettingsSource.java SettingsMerger.java GlobalSettings.java' scripts/audit-map/fr-settings.map
$A settings/sources crates/fr-settings/src '*.java'             scripts/audit-map/fr-settings.map
$A util             crates/fr-settings/src 'ReflectionUtil.java' scripts/audit-map/fr-settings.map
$A util/gson        crates/fr-settings/src '*.java'             scripts/audit-map/fr-settings.map
$A drc          crates/fr-drc/src '*.java'                              scripts/audit-map/fr-drc.map
$A io/kicad     crates/fr-drc/src '*.java'                              scripts/audit-map/fr-drc.map
$A core/scoring crates/fr-drc/src 'BoardStatisticsClearanceViolations.java' scripts/audit-map/fr-drc.map
$A core/scoring        crates/fr-router/src '*.java' scripts/audit-map/fr-router.map
$A autoroute           crates/fr-router/src '*.java' scripts/audit-map/fr-router.map
$A autoroute/maze      crates/fr-router/src '*.java' scripts/audit-map/fr-router.map
$A autoroute/expansion crates/fr-router/src '*.java' scripts/audit-map/fr-router.map
$A autoroute/drill     crates/fr-router/src '*.java' scripts/audit-map/fr-router.map
$A autoroute/path      crates/fr-router/src '*.java' scripts/audit-map/fr-router.map
$A autoroute/pipeline  crates/fr-router/src '*.java' scripts/audit-map/fr-router.map   # +1, Task 17
$A board/actions  crates/fr-router/src 'ForcedViaInserter.java ForcedPadRouter.java DrillItemMover.java' scripts/audit-map/fr-router.map
$A board/optimize crates/fr-router/src 'TraceShover.java TraceTightener.java TraceTightener90.java TraceTightener45.java TraceTightenerAnyAngle.java ViaOptimizer.java' scripts/audit-map/fr-router.map
$A core           crates/fr-router/src 'StopRequestState.java StoppableThread.java RouterCounters.java ProgressThrottler.java' scripts/audit-map/fr-router.map   # +1, Task 17, BY FILE GLOB
```

> **Status update — 2026-09-01, Plan 8 Task 0 (`b40b1cc`).** The 33 invocations above still exit
> 0 at **0 `MISSING`, 0 `UNMAPPED`**, but the `ROSTERED` count is now **25 across 9**, not 26
> across 10. The one that changed is
> `audit-port.sh management crates/fr-board/src 'HeadlessBoardManager.java' scripts/audit-map/fr-board.map`,
> which printed *"ROSTERED HeadlessBoardManager (9 public methods, none ported: 0 'not ported:',
> 9 'added in Task|Plan N:')"* and now prints none. **Cause:** Plan 8 Task 0 consumed three of
> those nine deferral markers (`getRoutingBoard`, `replaceRoutingBoard`, `getCurrentRoutingJob` —
> `crates/fr-board/src/board/clearance_override.rs`), and the third is a `renamed:`, which
> `audit-port.sh` counts as **ported**, so the class is no longer *wholly* deferred. The first two
> are `not ported:` (they have no Rust `fn`; their live non-GUI callers are rostered elsewhere —
> see the SF5 note at that site). Nothing regressed: the drop is a class leaving the deferral
> roster, which is what a plan consuming its own markers looks like. Plan 8 Task 3 takes
> `HeadlessBoardManager` off the list permanently, and **Plan 8 Task 14's hand-off table must
> carry the then-current number with this reason**, superseding the two rows above. Measured by
> diffing the full `ROSTERED` line sets before and after, not by eye. `crates/fr-router/README.md`'s
> §Audit note recording the old one-ROSTERED-line result for that invocation is stale for the same
> reason.

**The `core` invocation must stay file-globbed.** `core '*.java'` would pull
`RoutingJob`, `Session`, `BoardFileDetails`, `RouterJobResourceUsage`,
`RoutingJobPriority`, `RoutingJobState` and `RoutingStage` — all **Plan 8's** —
into `fr-router`'s roster.

**Two invocations the Task 17 brief named are deliberately absent**, because the
maps in the tree say those classes are `fr-board`'s. `board/facade
crates/fr-router/src 'RoutingBoardOperations.java' … fr-router.map` prints
`UNMAPPED RoutingBoardOperations` and exits 1 (`fr-board.map:82` maps it to
`board/*.rs`); `board/trace crates/fr-router/src 'PolylineTrace.java' …
fr-board.map` prints **24 `MISSING`** and exits 1, because `fr-board.map:96-97`
maps `PolylineTrace` to paths relative to `fr-board/src`. Both classes are
already covered at 0/0 by the crate-wide `board/facade` and `board/trace`
invocations against `crates/fr-board/src` in the nine-directory loop. Measured,
not assumed.

**Not re-run by this task, and why:** the eight-stem ladder, `--verify-driver`,
`--verify-hash-modes` and `sweep-p7t9.sh` need the Java clone and a JDK-25 build
and were run to completion by Task 16 on the tree this task only added comments
and documents to. `cargo nextest run --workspace` includes
`tests/batch_parity.rs`'s **CI lane** — the ladder's four CI stems — and it ran and
passed here: `PASS [5.621s] fr-router::batch_parity the_ci_stems_climb_the_whole_ladder`,
alongside `every_stem_reaches_rung_c`, `the_driver_matches_the_bare_jar`,
`references_are_from_the_head_jar`, `the_ses_normaliser_touches_only_the_parser_keywords`
and `the_stem_table_matches_the_fixture_file`. The 54 skipped tests are the
`#[cfg_attr(debug_assertions, ignore)]` slow lane, which needs a release build plus
`FR_SLOW_PARITY=1`.

---

## 12. Open items for the user

1. **`scripts/differential/rust` is not rustfmt-clean and Task 17 decided to leave
   it that way.** 160 `cargo fmt --check` diffs across 14 pre-existing driver
   binaries, unchanged since the start of Plan 7. The crate is **excluded from the
   workspace** (`Cargo.toml:6`), so `cargo fmt --all` does not reach it and the
   gate is unaffected. The reason not to format: the drivers are transcriptions of
   Java bodies, laid out line-for-line beside the Java they mirror so a reviewer can
   diff them by eye, and rustfmt rewraps exactly the long argument lists and match
   arms that alignment depends on. One 160-diff commit through every driver buys
   nothing a reader wants, and every future transcription would fight the formatter.
   Recorded in `docs/java-quirks.md` §Process notes. **Say so if you want it
   formatted anyway** — it is one mechanical commit and transcripts are unaffected.
2. **`router-dac2020-bm01` above `-mp 2` is unmeasurable** (§1, quirk #234). If
   whole-board parity on that board at higher pass counts ever matters, the only
   route is a patched jar — which the constraint "HEAD is the authority" forbids —
   or a Java-side change upstream. Flagged, not worked around.
3. **`JsonFileSettings` is unported and unclaimed** (§7). It is the
   `--settings <file>` source. Plan 8's CLI task should take it.
4. **Quirk #162 is still a live hang** in both languages (controller answer 5).
   Plan 8's `--job-timeout` will not interrupt it, because ruling AI's deadline is a
   poll and there is no poll site below `AutorouteEngine.java:265` — Java has none
   there either.
