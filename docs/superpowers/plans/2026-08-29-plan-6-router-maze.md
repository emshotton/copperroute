# Plan 6 — `fr-router` part 1 (the maze/expansion autorouter: one connection at a time) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port everything reachable from `AutorouteEngine.autorouteConnection` inclusive — `autoroute/{maze,expansion,drill,path}/**`, `autoroute/ItemAutorouteInfo`, `autoroute/AutorouteAttempt{Result,State}`, and the `board/actions` + `RoutingBoard` slices they need (`ForcedViaInserter`, `ForcedPadRouter.checkForcedPad`, `DrillItemMover.check`, `TraceShover.check`, `RoutingBoard.{initAutoroute, finishAutoroute, additionalUpdateAfterChange, clearAllItemTemporaryAutorouteData, checkForcedTracePolyline}`, `ShapeSearchTree.{completeShape, divideLargeRoom}`) — as the new `fr-router` crate, so that **routing one net connection produces the same `AutorouteAttemptState` and the same inserted trace polylines and via positions as the Java HEAD jar**, connection by connection, on the fixture corpus. ~8 750 Java LOC in scope, the largest plan so far.

**Architecture:** Spec §9 (the router's named types — the spec's list is already the clone's HEAD list), §10 (cancellation), §14.3 (`Dac2020Bm01` as the in-CI smoke test), §15 step 7 ("`fr-router` maze + expansion + path (first routed boards)"). `fr-router` sits on `fr-board` (items, rules, search trees, `StopCheck`/`TimeLimit`, changed-area bookkeeping), `fr-settings` (`RouterSettings` accessors, `ExpansionCostFactor`) and `fr-geometry`; it dev-depends on `fr-drc` and `fr-dsn` for the acceptance harness only. It adds two extension traits over `fr-board` types — `AutorouteSearchTreeExt` (the two `// added in Plan 6:` markers at `crates/fr-board/src/searchtree/shape_search_tree.rs:1625-1643`) and `RoutingBoardExt` (Plan 2 ruling 4, **created here** and shared with Plan 7) — because rooms and shove state must not leak into `fr-board`. Everything above `autoroute_connection` (the pass loop, ripup escalation, fanout, optimizer, pull-tight, the mutating half of `TraceShover`) is **Plan 7**.

**Tech Stack:** Rust 2024; `fr-board` (Plan 2); `fr-settings` (Plan 4); `fr-geometry` (Plan 1); `thiserror`. Dev-only: `fr-drc` (Plan 5) and `fr-dsn` (Plan 3) for the acceptance harness, `parity`. **No `rand`** (ruling 5), **no `rayon`** (ruling 17), **no `slotmap`** (ruling 16), no `tracing`, no static mutable state, no clock beyond `fr_board::TimeLimit`.

**Spec:** `docs/superpowers/specs/2026-08-27-freerouting-rust-port-design.md` (§2, §3, §9, §10, §14, §15). Also binding: `docs/plan-2-handoff.md` (§Obligations → Plan 6 and → Plan 7), `docs/plan-3-handoff.md` (§Plans 6/7 — rulings F and H), `docs/plan-4-handoff.md` (§Plans 6/7 — `ExpansionCostFactor`, `is_fanout_enabled`, quirks #127/#139/#140/#143), `docs/plan-5-handoff.md` (quirk #82 stays unfixed; the ratsnest the metric harness reads), `docs/java-quirks.md`.

**Later plans:** 7 `fr-router` part 2 (pipeline, fanout, optimizer, tighteners, the mutating shove); 8 `fr-core` + CLI/MCP.

> ### ⚠ Read rulings 1, 2 and 4 first
>
> **Java's single-threaded router is byte-reproducible.** The survey's JVM probe ran the HEAD jar over `-XX:+UnlockExperimentalVMOptions -XX:hashCode=0,1,2,3,4` × 2 repeats on `Issue508-DAC2020_bm01.dsn` (`-mp 1 -mt 1 -oit 0`) and got **10 byte-identical SES files** (`md5 068063d09c0eec4ddbfbfe9a9df13b0a`, 320 wires); the same sweep on `Issue143-rpi_splitter.dsn` gave one digest across five hash modes. Nothing in `autoroute/{maze,expansion,drill,path}` iterates a hash container, and the one `Random` is seeded from `ctrl.ripupCosts`.
>
> That upgrades the spec's metric-parity target: this plan's per-connection acceptance is **exact** (ruling 1), with spec §9's metrics as the fallback that must hold where exactness cannot be demonstrated. It also means the deterministic-container work (ruling 4) is not defensive tidying — `TreeSet` order *is* the algorithm, and a `BinaryHeap` or a `HashMap` anywhere in this crate is a wrong answer, not a slow one.
>
> The whole-board SES byte-parity headline remains **Plan 7's**: passes, ripup escalation and the optimizer all sit above this plan's ceiling.

## Global Constraints

All Plan 1–5 constraints and rulings remain in force (see each hand-off's §Rulings). Additionally:

- **Java wins over plan text, and over the survey.** Every file:line in this document was read out of the clone's HEAD while writing the plan; the implementer re-derives it from Java and reports disagreement rather than trusting the plan. Three places where this plan **already corrects the survey** are called out in rulings 8, 10 and 16 — that is the precedent, not the exception.
- **Java source authority is the clone's HEAD, and so is the parity jar** (`../freerouting/build/libs/freerouting-current-executable.jar`, JDK 25 at `/opt/homebrew/opt/openjdk@25/bin`, always `-Djava.awt.headless=true`). The pinned 2.3.0 jar (Plan 3 ruling 10) is **not** used anywhere in this plan: HEAD's `autoroute/**` has been refactored away from upstream (`MazeSearchAlgo`→`maze/MazeSearchEngine`, `LocateFoundConnectionAlgo*`→`path/FoundConnectionLocator*`, and `MazeExpansionEngine`/`MazeRipupResolver`/`AutorouteConnectionRouter` do not exist upstream at all), so a 2.3.0 reference would be a different algorithm.
- Behavioral port: reproduce Java bugs, with a `// Java bug:` marker at the site and a row in `docs/java-quirks.md`. `// totalized:` for crash→value changes, `// not ported:`, `// renamed:`, `// added in Plan 7:`, `obligation:` per `docs/java-quirks.md` §Process notes. **The marker's Java method name must sit on the same line as the marker** (Plan 2 ruling 13 — `audit-port.sh` is line-based).
- No GUI, no `FRLogger`, no `TextManager`, no observers. `AutorouteEngine.autorouteConnection`'s observer bracketing (`AutorouteEngine.java:254-270`) is `// not ported:`; every `FRLogger.trace` payload and its guard is dropped, including the eight `describe*` helpers of `MazeSearchEngine` (`:206-296`) and `MazeFanoutDiagnostics` (44 loc).
- `Result`/`Option` where Java throws or returns `null`; `catch_unwind`/`Result` only at the five documented boundaries (ruling 7).
- **No static mutable state and no threads.** `max_threads` is dead in Java's headless path (quirk #143, `docs/plan-4-handoff.md` §Plans 6/7): this crate is single-threaded and must not invent a threading policy.
- **No new workspace dependencies.** `thiserror` is already a workspace dependency; `rand`, `rayon` and `slotmap` are **not** added (rulings 5, 16, 17). Any need for a new one is a recorded ruling, not a silent `Cargo.toml` edit.
- Every task ends with `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and the `scripts/audit-port.sh` invocations from Task 1 (with `scripts/audit-map/fr-router.map`) run and their `MISSING` count recorded in the commit message — verified against the committed tree, not against a report.
- Commit trailer:
  ```
  Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_015f1ubhBBpiWHAooDsso97Z
  ```

## Rulings made while writing this plan (recorded here so they reach the user)

1. **Parity target is exact per connection, with spec §9's metrics as the fallback.** Because a single-threaded Java run is byte-reproducible and hash-mode-independent (the probe above), the acceptance for routing connection *k* of fixture *X* is, in order:
   **(a)** the same `AutorouteAttemptState` (`ROUTED` / `FAILED` / `SKIPPED` / `ALREADY_CONNECTED` / `CONNECTED_TO_PLANE` / `NO_UNCONNECTED_NETS`) and the same ripped-item id set;
   **(b)** the same inserted item geometry — every new trace's polyline corner list and layer, every new via's location and padstack — up to the quirks this plan documents;
   **(c)** spec §9's metric parity (incompletes delta, via delta, clearance violations 0, trace length ±10 %), which must hold **even where (b) cannot be demonstrated**.
   Task 17 records, per fixture and per connection index, which of (a)/(b)/(c) was reached, in `crates/fr-router/README.md`. *Reason:* the spec set metric parity because the survey for the spec assumed the router was not reproducible; it is, and an exact target catches an off-by-one in a cost function that a ±10 % length band would swallow. *Cost if wrong:* if some fixture turns out not to be exactly reproducible after all (a `Polyline` normalisation difference inherited from Plan 1, say), the fallback is already specified and the README records the demotion with its evidence — no re-plan.
2. **The Plan 6 / Plan 7 seam is `AutorouteConnectionRouter.route` (`autoroute/pipeline/AutorouteConnectionRouter.java:30-160`), split at step 5.** Plan 6 owns its steps 1–5: build `AutorouteControl` (`:42-47`), compute the start/dest sets with the plane swap (`:49-68`), the `TimeLimit` (`:74`), `initAutoroute` (`:76-82`) and `autorouteConnection` (`:88-90`). Plan 7 owns steps 6–8: `optChangedArea`/pull-tight (`:103-109`), the necked retry (`:123-145`, `retryConnectionNecked` `:162`) and the strict-DRC rollback via `BasicBoard.deserialize` (`:243`). Everything in `autoroute/pipeline/**` other than those five steps is Plan 7, as are `board/optimize/**`'s mutating half and `RoutingFailureLog`. *Reason:* one call above `autoroute_connection` and one call below the pass runner is the only place where the cut needs no shared mutable state — Plan 7 consumes exactly `AutorouteEngine`, `AutorouteControl` and `RoutingBoardExt::opt_changed_area`. *Cost if wrong:* a step migrates across the seam in Plan 7; the hand-off names each one.
3. **`RoutingBoardExt` is created in Plan 6, not Plan 7** (amends `docs/plan-2-handoff.md`'s Plan 7 bullet). Plan 6 needs `check_forced_trace_polyline` (`RoutingBoard.java:408-430`, reached from `MazeSearchEngine.java:681`), `init_autoroute` (`:882-897`), `finish_autoroute` (`:900-905`), `additional_update_after_change` (`:96-118`) and `clear_all_item_temporary_autoroute_data` (`:1241`). Plan 7 extends the same trait with `opt_changed_area`, pull-tight and the tighteners. *Reason:* four of the five are on the `autoroute_connection` path; deferring the trait would mean Plan 6 inventing a second one and Plan 7 merging them. *Cost if wrong:* nil — the trait is additive.
4. **Deterministic containers, transcribed rather than chosen.** The maze queue is `BTreeSet<MazeListElement>`, never a `BinaryHeap` (Java pops `mazeExpansionList.iterator().next()` **and re-inserts mutated elements**, `MazeSearchEngine.java:327-329`). `MazeListElement`'s `Ord` transcribes `compareTo` (`maze/MazeListElement.java:80-113`) literally as a chain of `<`/`>` tests on `sortingValue`, then `expansionValue`, then `door.getId()`, then `sectionNoOfDoor`, then `Equal` — **not** `total_cmp`, **not** `partial_cmp().unwrap()`. On non-NaN inputs the three agree; on NaN Java's `<`/`>` are both false and the comparison **falls through to the next key**, which `total_cmp` (NaN sorts last) and `partial_cmp().unwrap()` (panic) both get wrong. NaN is reachable in principle through a degenerate `weightedDistance`; the fall-through is quirk #155 with a unit test, and the five-way tie returning `Equal` — where Java's `TreeSet` and Rust's `BTreeSet` both **silently drop** the element — is quirk #156. The overridden `TreeSet.add` (`MazeSearchEngine.java:84-125`, the fanout escape-length gates) becomes a guarded `MazeQueue::push(&mut self, ctrl, element) -> bool`, never a bare `insert`. Hazards B, C and F — the mutating sort keys of `DrillPage.getId` (`drill/DrillPage.java:190-193` + the `netNumber` write at `:64-65`) and `IncompleteFreeSpaceExpansionRoom.getId` (`expansion/IncompleteFreeSpaceExpansionRoom.java:38-41` + `FreeSpaceExpansionRoom.setShape:70`), and the non-transitive `SortedRoomNeighbour.compareTo` — are **reproduced exactly, including insertion order**, with quirks rows; none is "fixed" pre-parity. *Reason:* every one of these is an ordering that reaches the output. *Cost if wrong:* a wrong-answer bug with no crash, visible only as a different route.
5. **`java.util.Random` comes from `fr-geometry`, bit-exact; `rand` is not a dependency.** `MazeSearchEngine.java:63,79-80` constructs `new Random()` and immediately `setSeed(ctrl.ripupCosts)`; the only draw is `MazeRipupResolver.java:158-163` (`randomize = ripupPassNo >= 4 && ripupPassNo % 3 != 0`, then `detour *= 0.5 + r*r`). Task 1 promotes the private `JavaRandom` in `crates/fr-geometry/src/polygon_shape.rs:33-90` into a public `crates/fr-geometry/src/java_random.rs` and adds `next_double` (`(next(26) << 27 + next(27)) * 0x1.0p-53`) and `set_seed`; `polygon_shape.rs` keeps quirk #30's behaviour byte-for-byte by using the moved type. *Reason:* `StdRng` diverges on the first draw and the draw multiplies a ripup cost that decides which item is torn up. *Cost if wrong:* ripup passes ≥ 4 diverge — which is Plan 7's headline, so a wrong LCG here would surface as an unexplained Plan 7 failure.
6. **Cancellation is checked at exactly six sites and nowhere else** (spec §10). `AutorouteEngine.isStopRequested` (`maze/AutorouteEngine.java:294-304`) = `timeLimit.limitExceeded() || stoppableThread.isStopRequested()`, and Plan 6's callers are `MazeSearchEngine.java:323` (top of the pop loop — the only hot-path one), `:975`, `:1009`, `:1035`, `:1051` (the four `init` loops) plus `DrillPage.java:103`'s `splitToConvex(autorouteEngine.stoppableThread)`. `Thread.currentThread().isInterrupted()` appears nowhere in `autoroute/`. `StopCheck`/`TimeLimit` come from `fr-board`. Adding a seventh check makes a timed-out run stop earlier than Java's and changes the routed-connection count. **Plan 3's ruling F is closed here** (Task 15): `Board::insert_via`, `Board::insert_escape_via` and `Board::split_traces` gain a `StopCheck` parameter, because `ForcedViaInserter.insert` (`board/actions/ForcedViaInserter.java:249`) reaches `BasicBoard.insertVia` → `splitTraces` → `PolylineTrace.split` from inside the router — the "other caller" Plan 3 deferred to. *Cost if wrong:* an unbounded hang on a ladder board (quirk #76) with no way to cancel.
7. **Five recovery boundaries, each a `Result` or a `catch_unwind`, each with a test.** Java's `catch (Exception)` sites inside Plan 6's scope are `AutorouteEngine.completeExpansionRoom:518`, `autorouteConnection:139` (maze construction), `:157` (`findConnection`), `:190` (`FoundConnectionLocator.getInstance`) and `AutorouteConnectionRouter.route:156`. Each degrades to a specific value — `FAILED` with a specific message, or an unchanged room list — and the port must produce **that same degraded value**, not propagate. Where the Rust equivalent of Java's NPE is a panic in ported geometry (quirk #22's polyline underflow, quirk #24's `rotateApprox`), the boundary is `std::panic::catch_unwind` over an `AssertUnwindSafe` closure; where the port already returns `Result`, it is a `match`. `StackOverflowError` is caught by neither language (quirk #27) and stays fatal. *Reason:* these five catches are why Java routes 320 wires on a board where one connection throws. *Cost if wrong:* one bad connection aborts a whole fixture and the metric harness reports a nonsense delta.
8. **`AutorouteControl` copies out of `RouterSettings`; it does not hold one — and it must copy the two fanout escape lengths. (This corrects the survey's field table, which omitted `ctrl.settings`.)** Java's field is `public final RouterSettings settings` (`maze/AutorouteControl.java:20`), and the **only** reader anywhere in `autoroute/{maze,expansion,drill,path}` is the overridden `TreeSet.add` at `MazeSearchEngine.java:96-97,111-112`, which reads `ctrl.settings.fanout.maxEscapeLengthMm` and `minEscapeLengthMm` (defaults `3000.0` / `500.0` when absent). So:
   ```rust
   pub fn new(board: &Board, net_no: i32, settings: &RouterSettings,
              via_costs: i32, trace_costs: &[ExpansionCostFactor]) -> AutorouteControl;  // :123
   ```
   copies those two `Option<f64>`s alongside every other field and stores no reference. `ExpansionCostFactor` is **re-exported** from `fr-settings` (`crates/fr-settings/src/router_settings.rs:946`, the Plan 4 obligation), never redeclared. *Reason:* a borrowed `&RouterSettings` inside a struct the engine mutates would fight the borrow checker for two `f64`s, and Plan 6 must be testable with a hand-built control and no pipeline. *Cost if wrong:* if Plan 7's fanout needs a third settings field, it is one more copied value.
9. **Ruling H (via-info / via-rule re-pointing) is closed by probe, defaulting to "accept the re-pointing".** `AutorouteControl.rebuildViaInfo` (`:234-284`) reads `ViaInfo.attachSmdAllowed()`, `getPadstack()` and `getClearanceClassIndex()` **through `viaRule.getVia(i)`**, so a `.rules` file that re-declares an existing `(via …)` makes Java's rule keep the *detached original* while the port's index reaches the replacement (`docs/plan-3-handoff.md`, pinned by `rules_round_trip.rs::re_declared_via_info_re_points_the_existing_via_rule_unlike_java`). Task 8 runs the decisive probe: `Issue593-BBD_Mars-64.dsn` plus the one-line `.rules` from Plan 3's test, routed by the HEAD jar with `-mp 1 -mt 1 -oit 0` **twice** — once with the `.rules` and once without — and the port likewise; if the SES via list is identical either way, the divergence is unobservable by the router and the register row closes as **"re-pointing accepted"**, with the probe output pasted into the hand-off. If it is not identical, the row closes the other way and `ViaRule` gains owned `ViaInfo` copies in a follow-up `fr-board` change named in the hand-off. *Reason:* the register row has been open since Plan 3 precisely because no consumer existed; this is the consumer. *Cost if wrong:* one `fr-board` change, contained, with the fixture that proves it.
10. **The `&self` cold-cache recompute is resolved by banning it in `fr-router`, not by changing it. (This corrects the survey, which left it open.)** Java's `Item.getTreeShape(tree, index)` (`board/model/items/Item.java:212-226`) calls `clearDerivedData()` on an out-of-range index, and `clearDerivedData` sets `autorouteInfo = null` (`:1060-1064`) — dropping `startInfo`, `precalculatedConnection` and the whole `ObstacleExpansionRoom` array. `Board::item_tree_shape` (`crates/fr-board/src/board/mod.rs:1448-1457`) **already reproduces that** (`clear_derived_data()` then one refill), and `ItemHeader::clear_derived_data` already nulls `autoroute_info` (`crates/fr-board/src/items/header.rs:424-427`, test `clear_derived_data_drops_shapes_and_autoroute_info_but_not_leaves`). The `&self` twin `item_tree_shape_ref` cannot, so **`fr-router` never calls `item_tree_shape_ref`/`item_tile_shape_ref`** — Task 18's audit greps for them and fails on a hit, and Task 2 carries a test that a stale index drops the autoroute info. Hazard N's three HEAD-only "stale tree index during routing" guards (`MazeSearchEngine.java:655-668`, `ItemAutorouteInfo.java:57-79`, `MazeTraceShover.java:64-66`) are ported **verbatim**, silent `continue` included. *Reason:* the drop is router-observable (a dropped `startInfo` changes the maze's destination test) and the `&mut` path already has it. *Cost if wrong:* a `&self` call site sneaks in and one item keeps stale rooms — caught by the audit grep and by `p6t1`.
11. **Three differential drivers plus a reference generator.** `p6t1` is the per-connection driver (Java probe vs Rust twin, routing connection *k* of fixture *X* through ruling 2's steps 1–5 and dumping attempt state, ripped ids, every inserted item's geometry and the metric block); `p6t2` covers `complete_shape`/`divide_large_room` room geometry over random boards — the two methods `p2t10` explicitly skipped (`scripts/differential/README.md`: "every public `ShapeSearchTree` method except `completeShape`/`divideLargeRoom`"); `p6t3` covers `SortedRoomNeighbours` in all three angle regimes. References come from `scripts/gen-router-reference.sh` driving the HEAD jar (`-mp 1 -mt 1 -oit 0 -Djava.awt.headless=true -Duser.language=en -Duser.country=US`, `-XX:+UnlockExperimentalVMOptions -XX:hashCode=2` for hygiene). MATCH order: `Issue143-rpi_splitter.dsn` first (smallest board that actually routes), then `Issue508-DAC2020_bm01.dsn` items 1..k. *Reason:* a report-level match alone can hide compensating errors, and the two skipped tree methods are the single largest untested surface `fr-board` handed over. *Cost if wrong:* nil — three drivers is the same shape Plans 2, 3 and 5 used.
12. **Ported Java tests: three unit suites plus the fixture assertion family.** `autoroute/maze/MazeListElementTest`, `autoroute/expansion/SortedRoomNeighboursFactoryTest` and `autoroute/RoutableLayersSafetyCheckTest` port directly (all three exist at HEAD and are in Plan 6's scope). `src/test/java/app/freerouting/fixtures/RoutingFixtureTest`'s assertion family (`maxIncompleteConnections`/`exactIncompleteConnections`/`maxClearanceViolations`) becomes a **single-pass harness** in `crates/fr-router/tests/fixtures.rs`, with `Dac2020Bm01RoutingTest.java:23-36`'s numbers (`setMaxPasses(1)`, `setMaxItems(2)` → `maxIncompleteConnections(194)`) as the in-CI smoke test spec §14.3 asks for. `StrictDrcEnforcementTest` and `BatchAutorouterDebugTest` are Plan 7's (they drive the pass loop). **Note the Java harness quirk** the numbers depend on: `TestingSettings.setMaxPasses` is first-writer-wins (`src/test/java/app/freerouting/settings/sources/TestingSettings.java:52-55`, `if (this.settings.maxPasses == null)`), so `RoutingFixtureTest.getRoutingJob`'s own `setMaxPasses(100)` **does not** override the test's `setMaxPasses(1)` — quirk #157, and a plan that assumed 100 passes would mis-baseline every fixture number. *Cost if wrong:* the smoke test asserts the wrong bound; the quirk row is what stops that.
13. **One audit map for the crate, and Plan 3's `fr-board.map` obligation is discharged here.** `scripts/audit-map/fr-router.map` maps every class in `autoroute/{,maze,expansion,drill,path}` plus the `board/actions` and `board/optimize` classes this plan touches, with cross-crate rows (`../../fr-board/src/...`) for the `fr-board` additions — falling back to a `fr-board.map` row if `audit-port.sh`'s glob resolution rejects the `../` form, exactly as Plan 5 Task 3 recorded for `ClearanceViolation` (pick one and say which in the commit message). The audit must reach **zero MISSING and zero UNMAPPED** for `autoroute/`, `autoroute/maze`, `autoroute/expansion`, `autoroute/drill`, `autoroute/path` and the named `board/actions`+`board/optimize` files. Plan 7's classes get `// added in Plan 7:` markers naming the method. The not-ported roster is `BoardHistory` + `BoardHistoryEntry` (spec §2 puts the undo/history store out of scope), `autoroute/events/**` (6 files, observers — `ProgressSink` replaces them in Plan 8), `PerformanceProfiler`, `AutorouteDiagnostic` (a GUI overlay sink) and `MazeFanoutDiagnostics`. *Cost if wrong:* the audit is the only mechanical check that 8 750 LOC arrived; weakening it is forbidden.
14. **Hard-coded debug net numbers are dropped; the HEAD-only pure-SMD relaxations are ported.** `AutorouteEngine.java:164-176` (`ctrl.netNumber == 33 || 66 || 67`), `FoundConnectionLocator.java:78-100,238` and `AutoroutePassRunner.logNet94Items:439` gate `FRLogger.trace` **only** — no decision reads them; each site gets `// not ported:` naming the method and quirk row #158 records that production code ships with three hard-coded net numbers. Conversely `AutorouteControl.java:263-269` (`attachSmdAllowed = true` for a pure-SMD net when the padstacks all say otherwise) and `:277-281` (`viaCostFactor *= 0.1` for the same) are **HEAD-only divergences from upstream that change routing** and are ported from HEAD with `// Java bug:`-adjacent notes and quirk row #159, because the parity jar is HEAD. *Cost if wrong:* dropping the SMD relaxations makes every pure-SMD fixture route differently; keeping the debug nets costs nothing but noise.
15. **`AutorouteInfo` gets a body of ids only, and the objects live in the engine's arenas.** `autoroute/ItemAutorouteInfo.java` holds `boolean startInfo`, a `Connection` and an `ObstacleExpansionRoom[]` — the last two are `fr-router` types, and `fr-board` cannot name them. So `crates/fr-board/src/items/header.rs`'s placeholder becomes
    ```rust
    pub struct AutorouteInfo {
        pub start_info: bool,                                  // ItemAutorouteInfo.java:15
        pub precalculated_connection: Option<ConnectionId>,    // :17
        pub expansion_rooms: Vec<Option<ObstacleRoomId>>,      // :20
    }
    ```
    with `ObstacleRoomId` and `ConnectionId` added to `crates/fr-board/src/ids.rs` beside the already-reserved `RoomId`, and the arenas (`Arena<ObstacleExpansionRoom>`, `Arena<Connection>`) owned by `AutorouteEngine`. The HEAD-only resize-preserving branch of `getExpansionRoom` (`ItemAutorouteInfo.java:57-66`: resize to the current tree-shape count, `System.arraycopy` the overlap, `FRLogger.warn` + `null` for an out-of-range index) is ported verbatim, warn dropped. *Reason:* this keeps `Board::deep_copy`'s and `clear_derived_data`'s wholesale drop working (ruling 10) without `fr-board` learning about rooms — the same shape as `RoomId` being reserved in `fr-board` while `CompleteFreeSpaceExpansionRoom` lives here. *Cost if wrong:* if the id indirection proves painful the alternative is a `BTreeMap<ItemId, ItemAutorouteInfo>` side table in the engine — but that loses the two drop points, so it is a decision to revisit only with evidence.
16. **Hand-rolled arenas, not `slotmap`. (This corrects the spec's dependency list, which names `slotmap`.)** Java's autoroute object graph is cyclic — a room holds its doors, a door holds both rooms (`expansion/ExpansionDoor.java`), a drill holds one room per layer (`drill/ExpansionDrill.java:55-92`) — and `Rc<RefCell<…>>` would make `Board: Send + Sync` (Plan 2 ruling 11) unrepresentable for the engine that borrows it. Every room, door, drill and page therefore lives in a `crates/fr-router/src/arena.rs` `Arena<T> { items: Vec<Option<T>> }` with a `u32` index newtype, `get`/`get_mut`/`insert`/`remove`, and **no generation counter** — Java has none either, and a stale index is exactly Java's stale reference. `RoomId(u32)` (already in `fr-board`) indexes the complete-room arena so the search tree can store it. *Reason:* a new dependency needs a ruling, and `slotmap`'s generational keys would *diverge* from Java by turning a stale-reference read into a `None` where Java reads a live object. *Cost if wrong:* an `Arena` is ~60 lines; swapping it later is mechanical.
17. **Single-threaded, and the crate says so.** No `rayon`, no `std::thread`; `Board` stays `Send + Sync` for Plan 7's optimizer but nothing here spawns. Quirk #143 (`-mt` is parsed, clamped, mirrored and read by nothing headless) is restated in the crate README so Plan 7 does not read this plan's silence as an invitation. *Cost if wrong:* a threaded maze would be non-deterministic and every acceptance in ruling 1 would evaporate.

**Controller answers to the drafter's open questions (binding):** (1) ruling 1 confirmed — exact-per-connection parity with spec §9 metrics as the fallback; (2) rulings 5/16/17 confirmed — no `rand`, `slotmap` or `rayon`; (3) ruling 9's default (accept the re-pointing, close the register row) stands unless the JVM probe shows a routing difference; (4) Task 15's three `fr-board` signature changes are accepted **provided the existing signatures remain as delegating wrappers** (`|| false` stop check, as Plan 3 did for `normalize_all_traces`) so every Plan 2–5 caller and test is untouched and `p2t11`/`p2t15` stay MATCH; (5) tasks run sequentially, one implementer at a time — the optional parallel lanes are not used.

## File Structure

```
crates/fr-geometry/
  src/java_random.rs              JavaRandom, public, + next_double/set_seed (Task 1, ruling 5)
  src/polygon_shape.rs            uses the moved type; quirk #30 behaviour unchanged
crates/fr-board/
  src/ids.rs                      + ObstacleRoomId, ConnectionId (Task 1, ruling 15)
  src/items/header.rs             AutorouteInfo gains its real body (Task 1)
  src/board/mod.rs                insert_via/insert_escape_via take a StopCheck (Task 15, ruling 6)
  src/board/normalize.rs          split_traces takes a StopCheck (Task 15)
crates/fr-router/
  Cargo.toml                      deps: fr-board, fr-settings, fr-geometry, thiserror
                                  dev-deps: fr-dsn, fr-drc, parity
  src/lib.rs                      pub use surface + the `// not ported:` roster (Task 18)
  src/error.rs                    RouterError
  src/arena.rs                    Arena<T> + index newtypes (ruling 16)
  src/autoroute/mod.rs
  src/autoroute/attempt.rs        AutorouteAttemptState / AutorouteAttemptResult
  src/autoroute/item_info.rs      ItemAutorouteInfo accessors over fr-board's AutorouteInfo
  src/autoroute/tree_ext.rs       AutorouteSearchTreeExt: complete_shape / divide_large_room
  src/autoroute/expansion/{mod,room,free_space_room,incomplete_room,complete_room,
                           obstacle_room,door,target_door}.rs
  src/autoroute/expansion/{sorted_neighbours,sorted_neighbours_45,
                           sorted_neighbours_orthogonal}.rs
  src/autoroute/drill/{mod,page,page_array,expansion_drill}.rs
  src/autoroute/maze/{mod,control,destination_distance,list_element,search_element,queue,
                      engine,search,expand,expansion_engine,ripup_resolver,trace_shover}.rs
  src/autoroute/path/{mod,connection,locator,locator_45,locator_any_angle,inserter}.rs
  src/board_ext/{mod,routing_board_ext,trace_shover,drill_item_mover,forced_pad_router,
                 forced_via_inserter}.rs
  tests/*.rs                      ported Java tests + per-task unit tests + the acceptance harness
  README.md
scripts/audit-map/fr-router.map   per class, incl. cross-crate rows
scripts/audit-map/fr-board.map    Plan 3 obligation, written in Task 18
scripts/gen-router-reference.sh   HEAD-jar per-connection reference generator (ruling 11)
tests/reference/router-fixtures.txt          stem|dsn|max_items
tests/reference/<stem>/router.jsonl          committed per-connection reference
tests/reference/<stem>/router.meta.txt       jar identity, java -version, hash mode, command
scripts/differential/java/{P6T1.java,P6T2.java,P6T3.java}
scripts/differential/rust/src/bin/{p6t1.rs,p6t2.rs,p6t3.rs}
```

---

### Task 1: `fr-router` skeleton, the cross-crate prerequisites, and the audit map

**Files:** `crates/fr-router/{Cargo.toml,src/lib.rs,src/error.rs,src/arena.rs,src/autoroute/mod.rs,src/autoroute/attempt.rs,src/autoroute/item_info.rs}`, `crates/fr-geometry/src/{java_random.rs,polygon_shape.rs,lib.rs}`, `crates/fr-board/src/{ids.rs,items/header.rs,items/mod.rs,lib.rs}`, `scripts/audit-map/fr-router.map`, root `Cargo.toml` (nothing to change — `members = ["crates/*"]` picks the crate up); `crates/fr-router/tests/skeleton.rs`, `crates/fr-geometry/tests/java_random.rs`, `crates/fr-board/tests/autoroute_info.rs`.
**Java:** `autoroute/AutorouteAttemptState.java (14)`, `autoroute/AutorouteAttemptResult.java (25)`, `autoroute/ItemAutorouteInfo.java (105)`; `java.util.Random` (JDK, for `next_double`).

**Interfaces produced:**
```rust
// crates/fr-geometry/src/java_random.rs  (ruling 5)
/// `java.util.Random`, reproduced bit for bit: the 48-bit truncated LCG, `nextInt(bound)`'s
/// rejection loop and `nextDouble`'s two draws. Moved up out of `polygon_shape.rs`, where it was
/// private, because the maze router's ripup resolver draws from a `Random` seeded with
/// `ctrl.ripupCosts` (MazeSearchEngine.java:79-80, MazeRipupResolver.java:160).
pub struct JavaRandom { seed: i64 }
impl JavaRandom {
    pub fn new(seed: i64) -> JavaRandom;          // `new Random(seed)` — scrambles
    pub fn set_seed(&mut self, seed: i64);        // `setSeed` — same scramble
    pub fn next_int(&mut self, bound: i32) -> i32;
    /// `nextDouble()`: `(((long) next(26) << 27) + next(27)) * 0x1.0p-53`.
    pub fn next_double(&mut self) -> f64;
}

// crates/fr-board/src/ids.rs  (ruling 15)
/// An `autoroute.expansion.ObstacleExpansionRoom`'s identity — the room an item's tree shape
/// carries while a connection is being routed. Reserved here, like [`RoomId`], because
/// `ItemAutorouteInfo` stores it and `fr-board` cannot name `fr-router`'s types.
pub struct ObstacleRoomId(pub u32);
/// An `autoroute.path.Connection`'s identity (`ItemAutorouteInfo.precalculatedConnection`).
pub struct ConnectionId(pub u32);

// crates/fr-board/src/items/header.rs  — the placeholder gets a body
/// Port of `autoroute.ItemAutorouteInfo` (ItemAutorouteInfo.java:10-105): the per-run autoroute
/// scratch `Item.autorouteInfo` holds. Ids, not objects (plan-6 ruling 15).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AutorouteInfo {
    pub start_info: bool,                               // :15  isStartInfo/setStartInfo :30-40
    pub precalculated_connection: Option<ConnectionId>, // :17  :43-50
    pub expansion_rooms: Vec<Option<ObstacleRoomId>>,   // :20  the array getExpansionRoom resizes
}

// crates/fr-router/src/arena.rs  (ruling 16)
pub struct Arena<T> { items: Vec<Option<T>> }
impl<T> Arena<T> {
    pub fn new() -> Self;
    pub fn insert(&mut self, value: T) -> u32;
    pub fn get(&self, index: u32) -> Option<&T>;
    pub fn get_mut(&mut self, index: u32) -> Option<&mut T>;
    pub fn remove(&mut self, index: u32) -> Option<T>;
    pub fn len(&self) -> usize;                 // live entries
    pub fn iter(&self) -> impl Iterator<Item = (u32, &T)>;
}

// crates/fr-router/src/autoroute/attempt.rs
/// Port of `autoroute.AutorouteAttemptState` (AutorouteAttemptState.java:1-14).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutorouteAttemptState { Routed, Failed, Skipped, AlreadyConnected,
                                 ConnectedToPlane, NoUnconnectedNets, /* … as Java declares */ }
/// Port of `autoroute.AutorouteAttemptResult` (AutorouteAttemptResult.java:1-25).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutorouteAttemptResult { pub state: AutorouteAttemptState, pub details: Option<String> }

// crates/fr-router/src/lib.rs
pub use fr_settings::ExpansionCostFactor;   // NOT redeclared — plan-4 obligation, ruling 8
```

**Transcription notes.**
- Copy `AutorouteAttemptState`'s constant list **verbatim and in declaration order** out of the Java file; the plan deliberately does not guess it (the file is 14 lines — read it).
- `AutorouteAttemptResult`'s message strings are built by `AutorouteEngine.describeConnection` (`AutorouteEngine.java:282-287`, `String.join(", ", …map(Item::toString))`); Task 16 owns them. Here the field is `Option<String>` and the constructor pair mirrors Java's two (`(state)` and `(state, message)`).
- `ItemAutorouteInfo`'s accessors live in `fr-router` (`autoroute/item_info.rs`) as free functions over `&mut Board`, because `getExpansionRoom(index, tree)` needs `item.treeShapeCount(autorouteTree)` and the room arena: `pub fn expansion_room(board: &mut Board, engine: &mut AutorouteEngine, id: ItemId, index: usize) -> Option<ObstacleRoomId>` — Task 2 fills the body; here it is a `todo!()`-free stub returning `None` **only if** the test file says so. Prefer: land the whole `ItemAutorouteInfo` port here (it is 105 lines and self-contained), with the room construction behind a closure Task 2 supplies.
- `Arena::remove` leaves a `None` hole and **does not** reuse indices (Java never reuses an object identity either).

**Tests (write first):**
- `crates/fr-geometry/tests/java_random.rs`: `JavaRandom::new(1000).next_double()` matches the JVM (generate the expected values with `jshell -q` under JDK 25: `new java.util.Random(1000L).nextDouble()` ×5 — paste the literals into the test with the command in a comment); `set_seed` after construction reproduces `new Random(); setSeed(x)`; `next_int` is unchanged from the old private impl (the existing `polygon_shape` tests are the guard — **do not edit them**; if any has to move, the promotion was not verbatim, stop and say so).
- `crates/fr-board/tests/autoroute_info.rs`: `AutorouteInfo::default()` is `start_info == false`, empty vec, `None`; `Board::item_tree_shape` on an out-of-range index drops it (ruling 10 — the test that pins the whole ruling); `deep_copy` drops it (already covered by Plan 2's snapshot tests — assert it still holds with a populated body).
- `crates/fr-router/tests/skeleton.rs`: `Arena` insert/get/remove/hole semantics, including "remove then insert does not reuse the index"; `AutorouteAttemptResult` equality; `fr_router::ExpansionCostFactor` is `fr_settings::ExpansionCostFactor` (a `TypeId` assert or a value round-trip through both paths).

**`scripts/audit-map/fr-router.map`** — one line per Java class per Rust file; the shape (fill every class of the five directories, plus the `board/actions`/`board/optimize` rows):
```
# Class -> Rust path glob (relative to crates/fr-router/src). Blank lines and `#` ignored.
AutorouteAttemptState         autoroute/attempt.rs
AutorouteAttemptResult        autoroute/attempt.rs
ItemAutorouteInfo             autoroute/item_info.rs
AutorouteEngine               autoroute/maze/engine.rs
AutorouteControl              autoroute/maze/control.rs
DestinationDistance           autoroute/maze/destination_distance.rs
MazeListElement               autoroute/maze/list_element.rs
MazeSearchElement             autoroute/maze/search_element.rs
MazeSearchEngine              autoroute/maze/{search,expand,queue}.rs
MazeExpansionEngine           autoroute/maze/expansion_engine.rs
MazeRipupResolver             autoroute/maze/ripup_resolver.rs
MazeTraceShover               autoroute/maze/trace_shover.rs
ExpansionRoom                 autoroute/expansion/room.rs
FreeSpaceExpansionRoom        autoroute/expansion/free_space_room.rs
IncompleteFreeSpaceExpansionRoom  autoroute/expansion/incomplete_room.rs
CompleteFreeSpaceExpansionRoom    autoroute/expansion/complete_room.rs
CompleteExpansionRoom         autoroute/expansion/room.rs
ObstacleExpansionRoom         autoroute/expansion/obstacle_room.rs
ExpandableObject              autoroute/expansion/room.rs
ExpansionDoor                 autoroute/expansion/door.rs
TargetItemExpansionDoor       autoroute/expansion/target_door.rs
SortedRoomNeighbours          autoroute/expansion/sorted_neighbours.rs
Sorted45DegreeRoomNeighbours  autoroute/expansion/sorted_neighbours_45.rs
SortedOrthogonalRoomNeighbours    autoroute/expansion/sorted_neighbours_orthogonal.rs
DrillPage                     autoroute/drill/page.rs
DrillPageArray                autoroute/drill/page_array.rs
ExpansionDrill                autoroute/drill/expansion_drill.rs
Connection                    autoroute/path/connection.rs
FoundConnectionLocator        autoroute/path/locator.rs
FoundConnectionLocator45Degree    autoroute/path/locator_45.rs
FoundConnectionLocatorAnyAngle    autoroute/path/locator_any_angle.rs
FoundConnectionInserter       autoroute/path/inserter.rs
ForcedViaInserter             board_ext/forced_via_inserter.rs
ForcedPadRouter               board_ext/forced_pad_router.rs
DrillItemMover                board_ext/drill_item_mover.rs
TraceShover                   board_ext/trace_shover.rs
RoutingBoard                  board_ext/routing_board_ext.rs
ShapeSearchTree               autoroute/tree_ext.rs
ShapeSearchTree45Degree       autoroute/tree_ext.rs
ShapeSearchTree90Degree       autoroute/tree_ext.rs
BoardHistory                  lib.rs
BoardHistoryEntry             lib.rs
PerformanceProfiler           lib.rs
AutorouteDiagnostic           lib.rs
MazeFanoutDiagnostics         lib.rs
# Plan 7 (matched only by `// added in Plan 7:` markers in lib.rs):
BatchAutorouter               lib.rs
AutoroutePassRunner           lib.rs
AutorouteBatchLoop            lib.rs
AutorouteConnectionRouter     autoroute/maze/engine.rs
BatchFanout                   lib.rs
BatchOptimizer                lib.rs
…
```

**Steps:**
- [ ] Write the three test files → they fail to compile (no crate).
- [ ] `crates/fr-router/Cargo.toml` + `src/lib.rs` + `src/error.rs` + `src/arena.rs`.
- [ ] Move `JavaRandom` into `crates/fr-geometry/src/java_random.rs`, add `next_double`/`set_seed`, re-point `polygon_shape.rs`, export from `lib.rs`.
- [ ] Add `ObstacleRoomId`/`ConnectionId` to `fr-board`'s `ids.rs`; give `AutorouteInfo` its body; update the two `items/mod.rs` accessors and the prelude.
- [ ] Port `AutorouteAttempt{State,Result}` and `ItemAutorouteInfo`.
- [ ] Write `scripts/audit-map/fr-router.map`; run all five audits and record the `MISSING` counts.
- [ ] `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`.
- [ ] Commit `feat(router): fr-router skeleton, the arena, ItemAutorouteInfo and a public JavaRandom`.

---

### Task 2: Expansion rooms, doors and `ExpandableObject` — and `TreeObject::Room` becomes real

**Files:** `crates/fr-router/src/autoroute/expansion/{mod,room,free_space_room,incomplete_room,complete_room,obstacle_room,door,target_door}.rs`, `src/autoroute/maze/search_element.rs`, `src/autoroute/item_info.rs`; `crates/fr-router/tests/expansion_rooms.rs`.
**Java:** `expansion/{ExpansionRoom.java (35), CompleteExpansionRoom.java (18), ExpandableObject.java (35), FreeSpaceExpansionRoom.java (92), IncompleteFreeSpaceExpansionRoom.java (42), CompleteFreeSpaceExpansionRoom.java (210), ObstacleExpansionRoom.java (159), ExpansionDoor.java (202), TargetItemExpansionDoor.java (75)}`, `maze/MazeSearchElement.java (40)`.

**Interfaces produced:**
```rust
/// Port of the `ExpansionRoom` interface (ExpansionRoom.java:7-34) and `CompleteExpansionRoom`
/// (CompleteExpansionRoom.java:1-18). Java's three implementors are two free-space rooms and one
/// obstacle room; the port dispatches on this enum rather than on trait objects, because every
/// room is reached through an arena index anyway (plan-6 ruling 16).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RoomRef { Complete(RoomId), Obstacle(ObstacleRoomId), Incomplete(IncompleteRoomId) }

/// Port of `expansion.CompleteFreeSpaceExpansionRoom` (CompleteFreeSpaceExpansionRoom.java:19-209).
pub struct CompleteFreeSpaceExpansionRoom {
    pub shape: TileShape,          // FreeSpaceExpansionRoom.java:14
    pub layer: usize,              // :17
    pub doors: Vec<DoorId>,        // ExpansionRoom.getDoors
    pub target_doors: Vec<TargetDoorId>,   // CompleteFreeSpaceExpansionRoom.java:26
    pub id: u32,                   // :33 — AutorouteEngine.generateRoomIdNo (engine counter)
    pub net_dependent: bool,       // :36 isNetDependent
    pub tree_leaf: Option<LeafId>, // the entry in the compensated tree; None once removed
}
/// Port of `expansion.ObstacleExpansionRoom` (ObstacleExpansionRoom.java:14-158).
pub struct ObstacleExpansionRoom {
    pub item: ItemId, pub index_in_item: usize, pub doors: Vec<DoorId>, /* … */
}
/// Port of `expansion.ExpansionDoor` (ExpansionDoor.java:11-201).
pub struct ExpansionDoor {
    pub first_room: RoomRef, pub second_room: RoomRef, pub dimension: i32,
    pub precalculated_shape: Option<TileShape>,           // :30 memo
    pub maze_search_elements: Vec<MazeSearchElement>,     // :36  section per door section
}
/// Port of `expansion.TargetItemExpansionDoor` (TargetItemExpansionDoor.java:11-74).
pub struct TargetItemExpansionDoor { pub item: ItemId, pub tree_shape_index: usize,
                                     pub room: RoomRef, pub maze_search_element: MazeSearchElement }
/// Port of the `ExpandableObject` interface (ExpandableObject.java:7-34); Java's four implementors
/// are `ExpansionDoor`, `TargetItemExpansionDoor`, `ExpansionDrill` and `DrillPage`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ExpandableRef { Door(DoorId), TargetDoor(TargetDoorId), Drill(DrillId), Page(PageId) }
/// Port of `maze.MazeSearchElement` (MazeSearchElement.java:1-40).
#[derive(Debug, Clone, PartialEq)]
pub struct MazeSearchElement { pub is_occupied: bool, pub backtrack_door: Option<ExpandableRef>,
                               pub section_no_of_backtrack_door: i32, pub room_ripped: bool,
                               pub adjustment: MazeAdjustment, pub already_checked: bool }
```

**The five `getId()` implementations, transcribed with their overflow** (each is `wrapping_*`, each is a **hash, never an identity** — quirk #8's family):
| Object | Java | Formula |
|---|---|---|
| `CompleteFreeSpaceExpansionRoom` | `:100` | the engine counter (`AutorouteEngine.generateRoomIdNo:672`, `++expansionRoomInstanceCount`) — the only true id |
| `ObstacleExpansionRoom` | `:49` | `(itemId << 10) \| indexInItem` — **aliases** for `itemId ≥ 2²¹` or `indexInItem ≥ 1024` (hazard D, quirk **#156** as landed) |
| `IncompleteFreeSpaceExpansionRoom` | `:38-41` | `31 * shape.getId() + layer`, and `shape` is **mutable** (`FreeSpaceExpansionRoom.setShape:70`) — hazard C |
| `ExpansionDoor` | `:185-190` | `min(id1,id2) * 31 + max(id1,id2)` over the two room ids |
| `TargetItemExpansionDoor` | `:71-74` | `31 * item.getId() + room.getId()` |

**`TreeObject::Room` (Plan 2's obligation — discharge it here).** `CompleteFreeSpaceExpansionRoom implements SearchTreeObject` (`:19-20`), so rooms and items share the tree and the same ordered result sets. `crates/fr-board/src/ids.rs:60-88`'s `Ord` already encodes both `compareTo`s (rooms before items, descending id within each); this task **populates** it and asserts the ordering against a real mixed tree. Two caveats to record rather than fix:
1. `CompleteFreeSpaceExpansionRoom.compareTo:46-54` tests `instanceof FreeSpaceExpansionRoom` and casts to `CompleteFreeSpaceExpansionRoom` — an incomplete room in a sorted set would `ClassCastException`. Unreachable today (incomplete rooms never enter a tree); quirk **#157** as landed, with a comment at the port's `RoomRef` ordering.
2. Room ids and item ids **collide numerically** (the room counter is per engine), and the order is correct only because the type discriminator is the primary key — which `TreeObject { Item, Room }` already is. A test asserts a room with `id == 7` and an item with `id == 7` sort as room-then-item.

**The double-removal audit (Plan 2 ruling 8).** `ShapeTree::remove_leaf` panics on a second removal where Java corrupts silently (quirk #39). `AutorouteEngine.clear:307-317` iterates `completeExpansionRooms` calling `removeFromTree` on each, while `removeCompleteExpansionRoom:404-407` may already have removed one — but `:406`'s `completeExpansionRooms.remove(room)` runs in the same method, so the list can never hold a removed room. This task proves that by construction: `remove_complete_room` takes the room **out of the arena** and out of the tree in one operation, and `clear` drains the arena. A test removes a room twice through the public API and asserts the second call is a no-op returning `false`, not a panic.

**Tests (`crates/fr-router/tests/expansion_rooms.rs`):**
- `room_ids_are_the_engine_counter_and_items_are_not`: two rooms created back-to-back get consecutive ids from the engine, independent of the board's item ids.
- `obstacle_room_id_aliases_above_1023_shapes` (quirk **#156** as landed): `ObstacleExpansionRoom::id(ItemId(1), 1024) == ObstacleExpansionRoom::id(ItemId(2), 0)`.
- `door_id_is_symmetric_in_its_two_rooms` (`ExpansionDoor.java:185-190`).
- `rooms_sort_before_items_and_descending_among_themselves`: insert two rooms and two items into one compensated tree, query an overlapping box, assert the `BTreeSet<TreeObject>` order is `Room(hi), Room(lo), Item(hi), Item(lo)`.
- `a_room_and_an_item_with_the_same_numeric_id_do_not_collide`.
- `removing_a_room_twice_is_a_no_op_not_a_panic` (Plan 2 ruling 8).
- `expansion_room_array_resizes_and_preserves` (`ItemAutorouteInfo.java:57-66`): grow the item's tree-shape count, assert the overlap survives and the new slots are `None`; shrink it, assert the truncation; an out-of-range index answers `None` (Java's `FRLogger.warn` + `null`).
- `a_stale_index_drops_the_autoroute_info` (ruling 10, the load-bearing one): populate `AutorouteInfo`, shrink the item's shapes, call `Board::item_tree_shape` past the end, assert `get_autoroute_info_pur()` is `None`.

Steps: tests → fail → implement → pass → fmt/clippy/test/audit → commit `feat(router): expansion rooms, doors and TreeObject::Room in the compensated tree`.

---

### Task 3: `AutorouteSearchTreeExt::{complete_shape, divide_large_room}` — all three regimes

**Files:** `crates/fr-router/src/autoroute/tree_ext.rs`; `crates/fr-board/src/searchtree/shape_search_tree.rs` (delete the two `// added in Plan 6:` markers at `:1625-1643`, keep the `not ported:` block below them); `crates/fr-router/tests/tree_ext.rs`; `scripts/differential/java/P6T2.java`, `scripts/differential/rust/src/bin/p6t2.rs`, `scripts/differential/run.sh`, `scripts/differential/README.md`.
**Java:** `board/searchtree/ShapeSearchTree.java:580-693` (`completeShape`) + `:701-811` (private `restrainShape`) + `:1095-1118` (`divideLargeRoom`); `ShapeSearchTree45Degree.java:95-281` + `:288-298` + its `restrainShape`/`calcOutsideRestrainedShape`/`calcInsideRestrainedShape`/`obstacleSegmentTouchesInside`/`signedLineDistance` helpers; `ShapeSearchTree90Degree.java:38-191` and its own helpers.

**Interfaces produced:**
```rust
/// The two `ShapeSearchTree` methods `fr-board` could not carry, because both take and return
/// `autoroute.expansion.IncompleteFreeSpaceExpansionRoom` (Plan 2 hand-off; the markers at
/// crates/fr-board/src/searchtree/shape_search_tree.rs:1625-1643). An extension trait in
/// `fr-router` keeps `fr-board` free of rooms — the `RoutingBoardExt` precedent (plan-2 ruling 4).
pub trait AutorouteSearchTreeExt {
    /// Port of `ShapeSearchTree.completeShape` (ShapeSearchTree.java:580-693) with its two
    /// overrides (`…45Degree.java:95-281`, `…90Degree.java:38-191`), dispatched on
    /// `ShapeSearchTree::angle()` exactly as Java dispatches on the subclass.
    fn complete_shape(
        &self,
        room: &IncompleteFreeSpaceExpansionRoom,
        net_no: i32,
        ignore_object: Option<TreeObject>,
        ignore_shape: Option<&TileShape>,
        items: &impl ItemLookup,
        ctx: &ItemCtx<'_>,
    ) -> Vec<IncompleteFreeSpaceExpansionRoom>;

    /// Port of `ShapeSearchTree.divideLargeRoom` (ShapeSearchTree.java:1095-1118) and
    /// `…45Degree.divideLargeRoom` (:288-298). Called only from `complete_shape`.
    fn divide_large_room(
        &self,
        rooms: Vec<IncompleteFreeSpaceExpansionRoom>,
        board_bounds: &IntBox,
    ) -> Vec<IncompleteFreeSpaceExpansionRoom>;
}
impl AutorouteSearchTreeExt for ShapeSearchTree { … }
```

**Transcription notes.**
- The dispatch is on `self.angle()` (`AngleRestriction::{Ninety, FortyFive, None}`), because `SearchTreeManager.getAutorouteTree` (`SearchTreeManager.java:147-161`) chose the subclass from the same value — and `crates/fr-board/src/searchtree/manager.rs:249-281` already reproduces that choice. Do **not** add a subclass hierarchy.
- Java's `completeShape` walks the tree's overlapping leaves and restrains the room shape once per obstacle, returning the pieces that survive; the 45° and 90° overrides replace the restraining geometry wholesale (they are not refinements of the base — read all three).
- `not ported:` the six private diagnostic helpers of the two subclasses (`describeBounds`, `isCompleteShapeDebugAnchor`, `traceCompleteShapeFilter`, `traceCompleteShapeCandidate`, `traceCompleteShapeDecision`, `obstacleId`, `obstacleNets` — `ShapeSearchTree45Degree.java:543-647`, `ShapeSearchTree90Degree.java:324-432`); the marker block already in `fr-board` names them, and this task moves that text into `tree_ext.rs` so the audit finds it where the code is.
- `divide_large_room` is where hazard C bites: it *creates* incomplete rooms whose `getId` derives from a shape that later mutates. Port as written.

**`p6t2` — the differential `p2t10` skipped.** `P6T2.java` declares `package app.freerouting.board.searchtree;` (to reach the protected leaf bounds), is compiled and run **in `run.sh`'s `needs_jar=1` mode** against the HEAD jar on JDK 25, and:
- builds the same `BasicBoard` `P2T10.java` builds (two layers, a two-pin component, two traces, an empty outline) plus `n` random obstacles from the shared xorshift stream every other randomised driver uses;
- for each of the three angle regimes, gets `searchTreeManager.getAutorouteTree(clearanceClass)`, constructs an `IncompleteFreeSpaceExpansionRoom` per random seed box/layer, calls `completeShape(room, netNo, ignoreObject, ignoreShape)` and prints, per returned room: the layer, the shape's corner list (`Double.toString` each ordinate), the dimension, and the contained/ignored flags; then calls `divideLargeRoom` on the result and prints the same;
- `p6t2.rs` prints the same lines from `AutorouteSearchTreeExt`.
Acceptance: **0 diffs** over 2 000 random rooms per regime, and the README gains a row saying this closes `p2t10`'s documented gap.

**Tests (`crates/fr-router/tests/tree_ext.rs`):** the fixed scripts written from `p6t2`'s output — one per regime, each with the room list and shapes as literals; plus `an_empty_board_returns_the_seed_room_unchanged`, `an_obstacle_fully_containing_the_seed_returns_nothing`, `divide_large_room_splits_at_the_board_bounds` and `the_45_degree_override_is_not_the_base_algorithm` (a shape where the two disagree, to prove the dispatch is live).

Steps: write `P6T2.java` → `p6t2.rs` stub → tests from the Java output → implement → 0 diffs → fmt/clippy/test/audit → commit `feat(router): completeShape and divideLargeRoom for all three angle regimes`.

---

### Task 4: `SortedRoomNeighbours` — the any-angle base and the factory dispatch

**Files:** `crates/fr-router/src/autoroute/expansion/sorted_neighbours.rs`; `crates/fr-router/tests/sorted_neighbours.rs`; `scripts/differential/java/P6T3.java`, `scripts/differential/rust/src/bin/p6t3.rs`.
**Java:** `expansion/SortedRoomNeighbours.java (807)` — `complete` `:65-78`, `selectCalculationMode` `:80-88`, the constructor `:46-60`, `calculateNeighbours` `:204-213`, `addSortedNeighbour` `:~380-410`, `calculateDoors` `:~500-660`, and the private inner `SortedRoomNeighbour` `:665-806` with its comparator `:720-762`.

**Interfaces produced:**
```rust
/// Port of `expansion.SortedRoomNeighbours` (SortedRoomNeighbours.java:20-806): the neighbour
/// sorter that turns one completed room into its door list.
pub struct SortedRoomNeighbours { /* room, room_shape, sorted: BTreeSet<SortedRoomNeighbour>, … */ }
impl SortedRoomNeighbours {
    /// `complete(ExpansionRoom, AutorouteEngine, int, boolean)` (:65-78) — the entry point
    /// `AutorouteEngine.calculateDoors` calls; dispatches on the tree's angle restriction.
    pub fn complete(room: RoomRef, engine: &mut AutorouteEngine, board: &mut Board,
                    net_no: i32, ignore_net: bool) -> Option<RoomRef>;
    /// `selectCalculationMode(ShapeSearchTree)` (:80-88).
    pub fn select_calculation_mode(tree: &ShapeSearchTree) -> CalculationMode;
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalculationMode { AnyAngle, FortyFiveDegree, Orthogonal }
```

**Hazard F — the highest-risk parity site in the plan — transcribed, not fixed.** The inner comparator (`:720-762`) is **non-transitive**: it compares `touchingSideNoOfRoom` first, then a distance difference with a `c_dist_tolerance = 1` (`:667`) band, then (only when the first corners are equal) the last-corner distance under the same band, then — only when both neighbours touch at a corner — a `Direction.compareFrom` of the two border lines, then `Signum.asInt(deltaDistance)`, and finally `searchTreeObject.getId() - other.searchTreeObject.getId()`. Elements land in a `TreeSet` (`:54`, inserted at `:408`), so a comparison that answers `Equal` **drops** the neighbour — and because the tolerance band is not an equivalence, *which* neighbour is dropped depends on insertion order. The port:
- uses `BTreeSet<SortedRoomNeighbour>` with an `Ord` that is the comparator transcribed line for line, `Signum::as_int_f64` included;
- **preserves the insertion order** of `calculateNeighbours` (`:204-213`), which sorts the overlapping tree entries by `getId()` then shape index with a **stable** `List.sort` — and where room ids and item ids collide (hazard G), the stable sort leaves them in raw tree-query order, which `ShapeSearchTree::overlapping_tree_entries` already reproduces (`Vec`, `(object, shape index)` order, JVM-verified in Plan 2);
- records quirk #162 (non-transitive comparator + silent drop) and quirk #163 (the id tie-break compares a room id against an item id, two different id spaces);
- has a test that constructs three neighbours `a < b`, `b < c`, `a == c` under the tolerance and asserts the port drops the same one Java does, with the Java output pasted in.

**`p6t3` mode 0** (any-angle): `P6T3.java` builds a random board, completes a room, and prints the sorted neighbour list — `touchingSideNoOfRoom`, both corners, the neighbour object's kind and id, and then the resulting door list (`first_room`, `second_room`, `dimension`, shape corners). `p6t3.rs` prints the same. Acceptance: 0 diffs over 1 000 random rooms.

**Tests (`crates/fr-router/tests/sorted_neighbours.rs`):**
- `select_calculation_mode_dispatches_on_the_tree_subclass` — the port of `SortedRoomNeighboursFactoryTest` (ruling 12), one case per regime.
- `the_comparator_is_not_transitive_and_drops_the_same_neighbour_java_does` (quirk #162).
- `a_tie_on_geometry_falls_back_to_the_object_id` and `room_and_item_ids_are_compared_across_id_spaces` (quirk #163).
- `one_neighbour_yields_one_door`, `a_corner_touch_yields_a_one_dimensional_door`, `a_full_side_overlap_yields_a_two_dimensional_door` — fixed geometries from `p6t3`.

Steps: tests → `P6T3.java` mode 0 → implement → 0 diffs → fmt/clippy/test/audit → commit `feat(router): SortedRoomNeighbours, the any-angle neighbour sorter and its non-transitive comparator`.

---

### Task 5: `Sorted45DegreeRoomNeighbours` and `SortedOrthogonalRoomNeighbours`

**Files:** `crates/fr-router/src/autoroute/expansion/{sorted_neighbours_45,sorted_neighbours_orthogonal}.rs`; `crates/fr-router/tests/sorted_neighbours_regimes.rs`; `scripts/differential/java/P6T3.java` (+ modes 1 and 2), `scripts/differential/rust/src/bin/p6t3.rs`.
**Java:** `expansion/Sorted45DegreeRoomNeighbours.java (982)` — including its **own** inner `SortedRoomNeighbour` at `:803`; `expansion/SortedOrthogonalRoomNeighbours.java (728)` — its own inner class at `:598`.

**Interfaces produced:** the same `complete` entry point per regime, called from Task 4's dispatch:
```rust
/// Port of `expansion.Sorted45DegreeRoomNeighbours` (Sorted45DegreeRoomNeighbours.java:19-981),
/// including its own inner comparator (:803-981) — **not** the base class's.
pub(crate) fn complete_45(room: RoomRef, engine: &mut AutorouteEngine, board: &mut Board,
                          net_no: i32, ignore_net: bool) -> Option<RoomRef>;
/// Port of `expansion.SortedOrthogonalRoomNeighbours` (SortedOrthogonalRoomNeighbours.java:17-727),
/// inner comparator at :598-727.
pub(crate) fn complete_orthogonal(room: RoomRef, engine: &mut AutorouteEngine, board: &mut Board,
                                  net_no: i32, ignore_net: bool) -> Option<RoomRef>;
```

**Transcription notes.** Each regime has its own inner `SortedRoomNeighbour` with its own comparator and its own `firstCorner`/`lastCorner` memos — **three separate classes, three separate transcriptions**; do not share a comparator between them "because they look alike". Each is checked for the same non-transitivity as hazard F and its answer recorded in the quirks table (the base class's is confirmed non-transitive; the other two must be read and classified, not assumed). Both subclasses also carry their own door-splitting geometry (the 45° one is 982 lines because it enumerates octagon side pairs).

**Tests:** `p6t3` modes 1 (45°) and 2 (orthogonal), each 1 000 random rooms, 0 diffs; plus the fixed per-regime scripts and a `the_three_regimes_disagree_on_the_same_room` test that proves the dispatch reaches three different implementations.

Steps: tests → `P6T3.java` modes 1/2 → implement → 0 diffs → fmt/clippy/test/audit → commit `feat(router): the 45-degree and orthogonal neighbour sorters`.

---

### Task 6: `AutorouteEngine` — the room lifecycle

**Files:** `crates/fr-router/src/autoroute/maze/engine.rs`; `crates/fr-router/tests/engine_rooms.rs`.
**Java:** `maze/AutorouteEngine.java` — constructor `:83-94`, `initConnection` `:96-124`, `clear` `:307-318`, `addIncompleteExpansionRoom` `:341-350`, `getFirstIncompleteExpansionRoom` `:356-366`, `removeIncompleteExpansionRoom` `:368-372`, `removeCompleteExpansionRoom` `:377-412`, `completeExpansionRoom` `:418-522`, `addCompleteRoom` `:525-545`, `calculateDoors` `:559-565`, `completeNeighbourRooms` `:567-592`, `invalidateDrillPages` `:598-601`, `removeAllDoors` `:603-617`, `getRoomsWithTargetItems` `:620-635`, `validate` `:637-652`, `resetAllDoors` `:654-668`, `generateRoomIdNo` `:672-674`, `isStopRequested` `:294-304`.

**Interfaces produced:**
```rust
/// Port of `maze.AutorouteEngine` (AutorouteEngine.java:39-675) — owns every expansion room, door,
/// drill and page for one routing run, plus the handle to the compensated search tree.
pub struct AutorouteEngine {
    pub(crate) complete_rooms: Arena<CompleteFreeSpaceExpansionRoom>,   // :74 completeExpansionRooms
    pub(crate) incomplete_rooms: Arena<IncompleteFreeSpaceExpansionRoom>, // :71
    pub(crate) obstacle_rooms: Arena<ObstacleExpansionRoom>,
    pub(crate) doors: Arena<ExpansionDoor>,
    pub(crate) target_doors: Arena<TargetItemExpansionDoor>,
    pub(crate) connections: Arena<Connection>,
    pub(crate) drill_pages: DrillPageArray,                             // :89-91 (Task 7)
    pub(crate) tree: TreeId,                                            // :47 autorouteSearchTree
    pub(crate) maintain_database: bool,                                 // :53
    net_number: i32,                                                    // :65 (-1 until init)
    room_instance_count: u32,                                           // :77
    time_limit: Option<TimeLimit>,                                      // :68
}
impl AutorouteEngine {
    /// `AutorouteEngine(RoutingBoard, int, boolean)` (:83-94). Takes the board mutably because
    /// `getAutorouteTree` may build and fill a new compensated tree.
    pub fn new(board: &mut Board, trace_clearance_class: usize, maintain_database: bool) -> Self;
    /// `initConnection(int, Stoppable, TimeLimit)` (:96-124).
    pub fn init_connection(&mut self, board: &mut Board, net_no: i32, time_limit: Option<TimeLimit>);
    /// `isStopRequested()` (:294-304) — the six-site check of plan-6 ruling 6.
    pub fn is_stop_requested(&self, stop: StopCheck<'_>) -> bool;
    /// `completeExpansionRoom(IncompleteFreeSpaceExpansionRoom)` (:418-522). The whole body is
    /// wrapped in `catch (Exception)` at :518 — plan-6 ruling 7's first boundary.
    pub fn complete_expansion_room(&mut self, board: &mut Board, room: IncompleteRoomId)
        -> Result<Vec<RoomId>, RouterError>;
    /// `completeNeighbourRooms(CompleteExpansionRoom)` (:567-592).
    pub fn complete_neighbour_rooms(&mut self, board: &mut Board, room: RoomRef);
    /// `removeCompleteExpansionRoom(CompleteFreeSpaceExpansionRoom)` (:377-412).
    pub fn remove_complete_expansion_room(&mut self, board: &mut Board, room: RoomId) -> bool;
    /// `clear()` (:307-318) and `resetAllDoors()` (:654-668).
    pub fn clear(&mut self, board: &mut Board);
    pub(crate) fn reset_all_doors(&mut self);
    /// `getRoomsWithTargetItems(Set<Item>)` (:620-635) — a room-only `TreeSet`, so `BTreeSet<RoomId>`.
    pub(crate) fn rooms_with_target_items(&self, items: &BTreeSet<ItemId>) -> BTreeSet<RoomId>;
    /// `generateRoomIdNo()` (:672-674) — pre-increment, so the first room is 1.
    pub(crate) fn generate_room_id_no(&mut self) -> u32;
}
```

**Transcription notes, each with its Java line.**
1. `new`: `maxDrillPageWidth = max((int)(5 * board.rules.getDefaultViaDiameter()), 10000)` (`:89-90`) — Task 7 consumes it; `stoppableThread = null` (`:92`) is the port's absent `StopCheck`.
2. `init_connection` (`:96-124`): only when `maintain_database` **and** the net changed — remove every net-dependent complete room (`:99-110`), then call `additional_update_after_change` for every item of the new net (`:111-117`). Then set `net_number`, the stop check and the time limit.
3. `complete_expansion_room` (`:418-522`): pick a 2-dimensional door to an existing complete room as `fromDoorShape`/`ignoreObject` (`:426-434`); call `complete_shape` (`:450`, Task 3); **only the first dimension-2 candidate is added directly** and the rest are re-completed (`:492-515`); remove the incomplete room at `:469`. The `catch (Exception)` at `:518` returns the rooms completed so far — port the *value*, not just the catch.
4. `complete_neighbour_rooms` (`:567-592`) **restarts the door iterator after each completion**, because completing a neighbour mutates the door list. The port must not hold a borrow across the call: collect door ids into a `Vec` and re-read the arena each round, re-checking liveness.
5. `remove_complete_expansion_room` (`:377-412`) regenerates incomplete rooms for the 1-dimensional neighbours (`:397`) and invalidates the overlapping drill pages; `:406`'s list removal is what makes the double-removal unreachable (Task 2).
6. `rooms_with_target_items` returns a `BTreeSet<RoomId>` — Java's `TreeSet<CompleteFreeSpaceExpansionRoom>` is descending by id, so the port iterates `.rev()` (Plan 2 ruling 14's third rule).

**Tests (`crates/fr-router/tests/engine_rooms.rs`):**
- `completing_a_seed_room_on_an_empty_board_yields_one_room_covering_the_board`.
- `an_obstacle_splits_the_seed_into_the_java_room_set` — literals from `p6t2`.
- `only_the_first_two_dimensional_candidate_is_added_directly` (`:492-515`).
- `completing_a_neighbour_restarts_the_iterator` — a board where completing neighbour 1 creates neighbour 3; assert neighbour 3 is visited.
- `complete_expansion_room_returns_the_partial_list_on_an_injected_failure` (ruling 7): a synthetic `Err` from the tree ext leaves the rooms completed so far in place and answers them, exactly as Java's `catch` does.
- `init_connection_on_a_new_net_drops_the_net_dependent_rooms` and `…leaves_them_when_maintain_database_is_false`.
- `rooms_with_target_items_iterates_descending`.

Steps: tests → fail → implement → pass → fmt/clippy/test/audit → commit `feat(router): AutorouteEngine's expansion-room lifecycle and its first recovery boundary`.

---

### Task 7: Drill pages and expansion drills

**Files:** `crates/fr-router/src/autoroute/drill/{mod,page,page_array,expansion_drill}.rs`; `crates/fr-router/tests/drill.rs`.
**Java:** `drill/DrillPageArray.java (121)` — ctor `:34-62`, `invalidate` `:66-74`, `overlappingPages` `:76-97`, `reset` `:~100-120`; `drill/DrillPage.java (194)` — `getDrills` `:63-131`, `getId` `:190-193`, `invalidate`/`reset`; `drill/ExpansionDrill.java (140)` — `calculateExpansionRooms` `:55-92`, `getId` `:127-130`.

**Interfaces produced:**
```rust
/// Port of `drill.DrillPageArray` (DrillPageArray.java:20-120): the board's bounding box tiled
/// into pages of `max(5 * defaultViaDiameter, 10000)` (AutorouteEngine.java:89-90).
pub struct DrillPageArray { bounds: IntBox, column_count: i32, row_count: i32,
                            page_width: i32, page_height: i32, pages: Vec<Vec<DrillPage>> }
impl DrillPageArray {
    pub fn new(board: &Board, max_page_width: i32) -> Self;              // :34-62
    pub fn invalidate(&mut self, shape: &TileShape);                     // :66-74
    /// `overlappingPages(TileShape)` (:76-97) — note the **mixed-type loop bounds**
    /// (`int minJ`, `double maxJ`); reproduce them, do not "clean up" the types.
    pub fn overlapping_pages(&self, shape: &TileShape) -> Vec<PageId>;
}
/// Port of `drill.DrillPage` (DrillPage.java:21-193).
pub struct DrillPage { shape: IntBox, net_number: i32, drills: Option<Vec<DrillId>>, … }
impl DrillPage {
    /// `getDrills(AutorouteEngine, boolean)` (:63-131) — recomputes whenever the engine's net
    /// differs from the memoised one, **mutating `netNumber`, which is part of `getId`**
    /// (:190-193): hazard B, quirk #167 (the plan's "#164" label went to Task 6).
    pub fn get_drills(&mut self, engine: &mut AutorouteEngine, board: &mut Board,
                      attach_smd: bool, stop: StopCheck<'_>) -> Vec<DrillId>;
}
/// Port of `drill.ExpansionDrill` (ExpansionDrill.java:17-139).
pub struct ExpansionDrill { pub shape: TileShape, pub location: Point,
                            pub first_layer: usize, pub last_layer: usize,
                            pub rooms: Vec<Option<RoomRef>>, … }
impl ExpansionDrill {
    /// `calculateExpansionRooms(AutorouteEngine)` (:55-92): binds one room per layer, creating
    /// one where none exists; `false` when any layer is blocked.
    pub fn calculate_expansion_rooms(&mut self, engine: &mut AutorouteEngine, board: &mut Board)
        -> bool;
}
```

**Transcription notes.**
- `get_drills` (`:63-131`) is stop-check site #6 (ruling 6): `PolylineArea::split_to_convex(Some(stop))` at `:103`, which is also quirk #30's fixed-seed `Random` path — `crates/fr-geometry/src/polyline_area.rs:195` already takes `Option<&dyn Fn() -> bool>`, so the plumbing exists.
- The obstacle cut-out loop (`:74-96`) skips drillable items, skips SMD pins when `attachSmd && pin.drillAllowed()`, and uses `prevObstacleShape.contains(currentObstacleShape)` to avoid cutting the same via shape once per layer — transcribe the `prevObstacleShape` carry, including that it starts as `IntBox.EMPTY`.
- `ExpansionDrill.calculateExpansionRooms` consumes `ShapeSearchTree::overlapping_objects` — a **mixed** room/item `BTreeSet` (`:57-73`), one of the three live mixed-set sites Task 2's ordering test covers.
- `DrillPageArray::new`'s `ceil` chain (`:37-41`) mixes `double` length with `int` page width; transcribe the exact expression, including that `pageWidth` is recomputed from `columnCount` rather than reused.

**Tests (`crates/fr-router/tests/drill.rs`):**
- `page_grid_matches_java_for_a_known_bounding_box` — three boards (square, wide, one page) with the literal grid from a JVM probe.
- `overlapping_pages_uses_javas_mixed_loop_bounds` — a shape whose `maxJ` is fractional; assert the page set matches the JVM (this test is the one that fails if the types are "cleaned up").
- `get_drills_recomputes_when_the_net_changes_and_mutates_the_id` (quirk #164): assert `id()` before and after differ, and record in the test that a page already queued in the maze list would now sort differently.
- `an_smd_pin_is_cut_out_unless_attach_smd_and_drill_allowed`.
- `the_prev_obstacle_carry_suppresses_duplicate_cutouts_for_a_through_via`.
- `calculate_expansion_rooms_fails_when_one_layer_is_blocked`.
- `split_to_convex_stops_when_the_stop_check_trips` (ruling 6).

Steps: tests → fail → implement → pass → fmt/clippy/test/audit → commit `feat(router): drill pages, the page array and expansion drills`.

---

### Task 8: `AutorouteControl`, `DestinationDistance`, `MazeListElement` — and the ruling-H probe

**Files:** `crates/fr-router/src/autoroute/maze/{control,destination_distance,list_element,search_element,queue}.rs`; `crates/fr-router/tests/{control,destination_distance,maze_list_element}.rs`; `docs/java-quirks.md` (the ruling-H register row).
**Java:** `maze/AutorouteControl.java (311)` — public ctor `:117-121`/`:123-132`, private ctor `:134-187`, `isPureSmdNet` `:189-202`, `initNet` `:204-231`, `rebuildViaInfo` `:234-284`, `ExpansionCostFactor` `:287`, `ViaCost` `:290-297`, `ViaMask` `:299-311`; `maze/DestinationDistance.java (391)` — ctor `:45-99`, `join` `:102-114`, `calculate(FloatPoint,int)` `:117-128`, `calculate(IntBox,int)` `:130-379`, `calculateCheapDistance` `:382-390`; `maze/MazeListElement.java (114)`; `maze/MazeSearchElement.java (40)`; `MazeSearchEngine.java:84-125` (the queue's guarded `add`).

**Interfaces produced:**
```rust
/// Port of `maze.AutorouteControl` (AutorouteControl.java:18-311). Every field is copied out of
/// `RouterSettings`; the port holds no settings reference (plan-6 ruling 8).
pub struct AutorouteControl {
    pub trace_costs: Vec<ExpansionCostFactor>,   // :23   (re-exported from fr-settings)
    pub bend_costs: Vec<f64>,                    // :25   settings.getBendCost(i) (:145-147)
    pub with_neckdown: bool,                     // :26   settings.getAutomaticNeckdown() (:168)
    pub layer_active: Vec<bool>,                 // :29   :150-161 + :226-228
    pub layer_count: usize,                      // :31
    pub trace_half_width: Vec<i32>,              // :34   :217-222
    pub compensated_trace_half_width: Vec<i32>,  // :40   :223-225
    pub via_radii: Vec<f64>,                     // :42   :253-261
    pub add_via_costs: Vec<Vec<i32>>,            // :45   ViaCost[layer].toLayer[]  (all zero, :174-178)
    pub trace_clearance_class_index: usize,      // :48   :210/:213
    pub vias_allowed: bool,                      // :51   settings.getViasAllowed() (:140)
    pub attach_smd_allowed: bool,                // :54   :243-246 + the pure-SMD relaxation :263-269
    pub min_normal_via_cost: f64,                // :57   :282
    pub ripup_allowed: bool,                     // :59   default false (:184)
    pub ripup_costs: i32,                        // :60   default 1000 (:185)
    pub ripup_pass_no: i32,                      // :61   default 1 (:186)
    pub is_fanout: bool,                         // :64   set by Plan 7's BatchFanout
    pub fanout_start_pin_name: Option<String>,   // :67
    pub fanout_start_pin_center: Option<Point>,  // :70
    pub fanout_start_pin_layer: i32,             // :73   default -1
    pub remove_unconnected_vias: bool,           // :76   BatchAutorouter.java:115 = !isFanoutEnabled()
    pub via_rule: ViaRuleId,                     // :79   :211/:214
    pub net_number: i32,                         // :82
    pub via_clearance_class: usize,              // :85   :236-240
    pub via_infos: Vec<ViaMask>,                 // :88   :262
    pub via_lower_bound: usize,                  // :91   0            (:181)
    pub via_upper_bound: usize,                  // :94   layer_count  (:182)
    pub max_via_radius: f64,                     // :96   :272-275
    pub tidy_region_width: i32,                  // :99   i32::MAX     (:169)
    pub pull_tight_accuracy: i32,                // :102  500          (:170)
    pub max_shove_trace_recursion_depth: i32,    // :105  20           (:171)
    pub max_shove_via_recursion_depth: i32,      // :108  5            (:172)
    pub max_spring_over_recursion_depth: i32,    // :111  5            (:173)
    pub min_cheap_via_cost: f64,                 // :114  0.8 * min_normal (:283)
    // plan-6 ruling 8: the only two `ctrl.settings` reads in the whole package
    // (MazeSearchEngine.java:96-97,111-112), copied rather than borrowed.
    pub fanout_max_escape_length_mm: f64,        // default 3000.0 when settings.fanout is absent
    pub fanout_min_escape_length_mm: f64,        // default 500.0
}
impl AutorouteControl {
    /// `AutorouteControl(RoutingBoard, int, RouterSettings, int, ExpansionCostFactor[])` (:123-132).
    pub fn new(board: &Board, net_no: i32, settings: &RouterSettings,
               via_costs: i32, trace_costs: &[ExpansionCostFactor]) -> Self;
    /// `rebuildViaInfo(RoutingBoard, int, int)` (:234-284).
    pub fn rebuild_via_info(&mut self, board: &Board, via_costs: i32, net_no: i32);
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViaMask { pub from_layer: usize, pub to_layer: usize, pub attach_smd_allowed: bool } // :299

/// Port of `maze.DestinationDistance` (DestinationDistance.java:16-390): the maze's admissible
/// lower bound on the remaining cost.
pub struct DestinationDistance { … }
impl DestinationDistance {
    pub fn new(trace_costs: &[ExpansionCostFactor], layer_active: &[bool],
               min_normal_via_cost: f64, min_cheap_via_cost: f64) -> Self;          // :45-99
    pub fn join(&mut self, box_to_join: &IntBox, layer: usize);                     // :102-114
    pub fn calculate(&self, point: &FloatPoint, layer: usize) -> f64;               // :117-128
    /// `calculateCheapDistance` (:382-390) mutates and restores `minNormalViaCost` in Java;
    /// the port takes it as a parameter instead (hazard J, `// renamed:` note).
    pub fn calculate_cheap(&self, point: &FloatPoint, layer: usize, min_via_cost: f64) -> f64;
}

/// Port of `maze.MazeListElement` (MazeListElement.java:11-113).
#[derive(Debug, Clone, PartialEq)]
pub struct MazeListElement {
    pub door: ExpandableRef, pub section_no_of_door: i32,
    pub backtrack_door: Option<ExpandableRef>, pub section_no_of_backtrack_door: i32,
    pub expansion_value: f64, pub sorting_value: f64,
    pub next_room: Option<RoomRef>, pub shape_entry: FloatLine,
    pub room_ripped: bool, pub adjustment: MazeAdjustment, pub already_checked: bool,
}
// Java bug: `compareTo` (MazeListElement.java:80-113) compares two `double`s with raw `<`/`>`, so a
// NaN falls through to the next key instead of ordering; the port transcribes the fall-through
// rather than using `total_cmp` (plan-6 ruling 4, docs/java-quirks.md #155). A full tie returns 0
// and the set silently drops the element (#156).
impl Ord for MazeListElement { … }

/// The `TreeSet` subclass `MazeSearchEngine` installs (MazeSearchEngine.java:84-125): a sorted set
/// whose `add` refuses elements outside the fanout escape-length window.
pub struct MazeQueue { set: BTreeSet<MazeListElement> }
impl MazeQueue {
    /// The overridden `add` (:86-124). Returns Java's `boolean`.
    pub fn push(&mut self, element: MazeListElement, ctrl: &AutorouteControl,
                engine: &AutorouteEngine, board: &Board) -> bool;
    /// `iterator().next()` + `remove` (MazeSearchEngine.java:327-329).
    pub fn pop_first(&mut self) -> Option<MazeListElement>;
}
```

**Transcription notes.**
- The layer-active force-off (`:150-161`) fires when the layer is **not** a signal layer and the setting says active; Java logs a WARN and forces `false`. This is `RoutableLayersSafetyCheckTest`'s subject (ruling 12) — port the test.
- `initNet` (`:204-231`): net ≤ 0 falls back to net 1's trace half widths (`:217-221`); a `null` net gives `traceClearanceClassIndex = 1` and `viaRules.firstElement()` (`:213-215`); the `NetClass.isActiveRoutingLayer` AND happens **after** the settings force-off (`:226-228`).
- `rebuild_via_info` (`:234-284`): `viaClearanceClass` from via 0 only (`:236-240`); `viaRadii[j] = max(existing, 0.5 * shape.maxWidth())` per padstack layer, `null` shape → 0 (`:250-260`); then `viaRadii[j] = max(viaRadii[j], traceHalfWidth[j])` and `maxViaRadius` (`:272-275`); `viaCostFactor = max(maxViaRadius, 1)`, `×0.1` for a pure-SMD net (`:277-281`, ruling 14); `minNormalViaCost = viaCosts * viaCostFactor`, `minCheapViaCost = 0.8 ×` (`:282-283`).
- `isPureSmdNet` (`:189-202`): every connectable item of the net must be a `Pin` with `firstLayer() == lastLayer()`; an empty net is **not** pure SMD.
- `DestinationDistance.calculate(IntBox, layer)` (`:130-379`) is 250 lines of 1..4-layer path enumeration with `minNormalViaCost` per layer change — transcribe branch for branch; it is pure and fully unit-testable, which is why this task sits before the maze.
- `MazeQueue::push` reads `ctrl.fanout_start_pin_center`, `ctrl.fanout_start_pin_layer`, the two escape lengths and `board.communication.get_resolution(Unit::Um)` (`crates/fr-board/src/board/communication.rs:174`) — exactly Java's `:88-122`.

**The ruling-H probe (close the register row here).**
```sh
export PATH=/opt/homebrew/opt/openjdk@25/bin:$PATH
cd ../freerouting
# the .rules from crates/fr-dsn/tests/…re_declared_via_info… , re-declaring the (via …) with attach
java -Djava.awt.headless=true -jar build/libs/freerouting-current-executable.jar \
  -de fixtures/Issue593-BBD_Mars-64.dsn -do /tmp/h-without.ses -mp 1 -mt 1 -oit 0
java -Djava.awt.headless=true -jar build/libs/freerouting-current-executable.jar \
  -de fixtures/Issue593-BBD_Mars-64.dsn -dr /tmp/redeclare.rules -do /tmp/h-with.ses -mp 1 -mt 1 -oit 0
diff <(grep -c '(via' /tmp/h-without.ses) <(grep -c '(via' /tmp/h-with.ses)
```
Run the port's twin (`p6t1` with and without the rules). If the via lists agree, close the row as **"re-pointing accepted; unobservable to the router"** with the output pasted into `docs/java-quirks.md` and the hand-off. If they differ, close it the other way and file the `fr-board` follow-up (owned `ViaInfo`s on `ViaRule`) naming this fixture.

**Tests:**
- `crates/fr-router/tests/maze_list_element.rs` — the port of `MazeListElementTest` (ruling 12), method for method, **plus** `nan_sorting_value_falls_through_to_the_next_key` (quirk #155) and `a_full_tie_is_dropped_by_the_set` (quirk #156).
- `crates/fr-router/tests/control.rs` — the port of `RoutableLayersSafetyCheckTest`; `net_zero_falls_back_to_net_one_half_widths`; `a_null_net_uses_clearance_class_one_and_the_first_via_rule`; `pure_smd_relaxes_attach_and_scales_the_via_cost` (quirk #159, both halves); `an_empty_net_is_not_pure_smd`; `every_hard_coded_constant_matches_java` (the five at `:169-173`); `the_two_fanout_escape_lengths_are_copied_with_javas_defaults` (ruling 8).
- `crates/fr-router/tests/destination_distance.rs` — a table of (point, layer) → distance taken from a JVM probe over a 4-layer board, covering the 1-, 2-, 3- and 4-layer arms; `calculate_cheap_does_not_mutate_the_receiver` (hazard J).

Steps: tests → fail → implement → pass → run the ruling-H probe → fmt/clippy/test/audit → commit `feat(router): AutorouteControl, DestinationDistance and the maze list element`.

---

### Task 9: `RoutingBoardExt`, `TraceShover::check` and `DrillItemMover::check`

**Files:** `crates/fr-router/src/board_ext/{mod,routing_board_ext,trace_shover,drill_item_mover}.rs`; `crates/fr-router/tests/board_ext.rs`.
**Java:** `board/facade/RoutingBoard.java` — `additionalUpdateAfterChange` `:96-118`, `checkForcedTracePolyline` `:408-430`, `initAutoroute` `:882-897`, `finishAutoroute` `:900-905`, `clearAllItemTemporaryAutorouteData` `:1241-…`; `board/optimize/TraceShover.java` — the static `check` `:57-229` and the instance `check` `:231-416` (**only** these two; `insert` `:417-591` and `springOverObstacles` `:827-874` are `// added in Plan 7:`), `getIgnoreItemsAtTiePins` `:592-610`, `springOver` `:611-826` as far as `check` reaches it; `board/actions/DrillItemMover.java (326)` — `check` and `tryShoveViaPoints` only.

**Interfaces produced:**
```rust
/// The `RoutingBoard` methods `fr-board` deliberately left out (plan-2 ruling 4). Created in
/// Plan 6 and **shared with Plan 7** (plan-6 ruling 3), which adds `opt_changed_area`, the
/// pull-tight entry points and the tighteners.
pub trait RoutingBoardExt {
    /// `initAutoroute(int, int, Stoppable, TimeLimit, boolean)` (RoutingBoard.java:882-897):
    /// reuses the engine only when `retainAutorouteDatabase` **and** the compensated clearance
    /// class matches, then calls `AutorouteEngine::init_connection`.
    fn init_autoroute(&mut self, engine: Option<AutorouteEngine>, net_no: i32,
                      trace_clearance_class: usize, time_limit: Option<TimeLimit>,
                      retain: bool) -> AutorouteEngine;
    /// `finishAutoroute()` (:900-905) — drops the engine after `clear()`.
    fn finish_autoroute(&mut self, engine: AutorouteEngine);
    /// `additionalUpdateAfterChange(Item)` (:96-118): invalidate the drill pages of every tree
    /// shape and remove every complete room the shape overlaps.
    fn additional_update_after_change(&mut self, engine: &mut AutorouteEngine, item: ItemId);
    /// `clearAllItemTemporaryAutorouteData()` (:1241-…).
    fn clear_all_item_temporary_autoroute_data(&mut self);
    /// `checkForcedTracePolyline(...)` (:408-430) — reached from MazeSearchEngine.java:681.
    #[allow(clippy::too_many_arguments)]
    fn check_forced_trace_polyline(&mut self, polyline: &Polyline, half_width: i32, layer: usize,
                                   net_nos: &[i32], clearance_class: usize,
                                   max_recursion_depth: i32, max_via_recursion_depth: i32,
                                   max_spring_over_recursion_depth: i32) -> bool;
}
impl RoutingBoardExt for Board { … }

/// The **check-only** half of `board.optimize.TraceShover` (TraceShover.java:1-416).
// added in Plan 7: `TraceShover.insert` (TraceShover.java:417-591), `TraceShover.springOverObstacles`
// (:827-874) and every mutating helper they reach.
pub struct TraceShover;
impl TraceShover {
    /// The static `check(LineSegment, …)` (TraceShover.java:57-229), which
    /// `RoutingBoardSearchFacade.checkTraceSegment` and `MazeTraceShover` both reach.
    pub fn check_segment(board: &mut Board, …) -> f64;
    /// The instance `check(TileShape, ShapeEntrySide, …)` (:231-416).
    pub fn check(board: &mut Board, shape: &TileShape, from_side: &ShapeEntrySide, …) -> bool;
    /// `getIgnoreItemsAtTiePins` (:592-610).
    pub(crate) fn ignore_items_at_tie_pins(board: &mut Board, shape: &TileShape, layer: usize,
                                           net_nos: &[i32]) -> Vec<ItemId>;
}
/// The check half of `board.actions.DrillItemMover` (DrillItemMover.java) — `check` and
/// `tryShoveViaPoints`; the mover's mutating half is `// added in Plan 7:`.
pub struct DrillItemMover;
```

**Transcription notes.**
- `check_forced_trace_polyline` (`:408-430`) uses the **default** tree (not the compensated one), adds `clearanceCompensationValue`, and in 90° mode replaces each offset shape by its bounding box before building the `ShapeEntrySide` — the port's `ShapeTraceEntries` (`crates/fr-board/src/board/shape_trace_entries.rs`) already exists and is the whole reason this method can land now.
- `TraceShover::check` recursion: it calls `DrillItemMover::check` (`:281`-ish), `springOver` (`:315`-ish) and **itself** (`:340`-ish) up to `maxRecursionDepth`; transcribe the depth accounting exactly, since `ctrl.maxShoveTraceRecursionDepth = 20` is a hard-coded constant the maze depends on.
- Hazard P: confirm before wiring that `TraceShover` contains **no** `Line.equals` call site (quirk #34's `equals_geometric` obligation is about `TraceTightener*.repositionLine`, Plan 7). Record the grep result in the commit message.
- `additional_update_after_change` consumes a mixed room/item `BTreeSet` from `overlapping_objects` (`RoutingBoard.java:110-115`) — the third live mixed-set site.

**Tests (`crates/fr-router/tests/board_ext.rs`):**
- `init_autoroute_reuses_the_engine_only_on_a_matching_clearance_class` and `…never_reuses_when_retain_is_false`.
- `additional_update_after_change_removes_the_overlapping_rooms_and_invalidates_the_pages`.
- `check_forced_trace_polyline_uses_the_default_tree_and_the_bounding_box_in_ninety_degree_mode`.
- `trace_shover_check_refuses_at_the_recursion_limit` — depth 20 exactly.
- `trace_shover_check_does_not_mutate_the_board` — snapshot `structural_hash()` before and after (the property that makes the check-only split safe).
- `drill_item_mover_check_agrees_with_the_jvm` — a small table from a JVM probe.

Steps: tests → fail → implement → pass → fmt/clippy/test/audit → commit `feat(router): RoutingBoardExt and the check-only half of TraceShover/DrillItemMover`.

---

### Task 10: `ForcedPadRouter::check_forced_pad` and `ForcedViaInserter`

**Files:** `crates/fr-router/src/board_ext/{forced_pad_router,forced_via_inserter}.rs`; `crates/fr-router/tests/forced_via.rs`.
**Java:** `board/actions/ForcedPadRouter.java (500)` — `checkForcedPad` `:221-340` and `CheckDrillResult` `:494-500`; `board/actions/ForcedViaInserter.java (462)` — `checkLayer` `:30-129`, `check` `:131-247`, `insert` `:249-361`, `holeCheckShape` `:363-375`, `calculateFromSide` `:377-…`.

**Interfaces produced:**
```rust
/// Port of `board.actions.ForcedPadRouter.CheckDrillResult` (ForcedPadRouter.java:494-500).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckDrillResult { NotDrillable, Drillable, DrillableWithAttachSmd }
/// Port of `ForcedPadRouter.checkForcedPad` (:221-340) — the only method of the class Plan 6
/// needs; the rest of the class is `// added in Plan 7:` (it routes pads by shoving).
pub fn check_forced_pad(board: &mut Board, …) -> CheckDrillResult;

/// Port of `board.actions.ForcedViaInserter` (ForcedViaInserter.java:20-461).
pub struct ForcedViaInserter;
impl ForcedViaInserter {
    /// `checkLayer(...)` (:30-129) — reached from MazeExpansionEngine.java:391.
    pub fn check_layer(board: &mut Board, …) -> CheckDrillResult;
    /// `check(...)` (:131-247) — reached from FoundConnectionInserter.java:708.
    pub fn check(board: &mut Board, …) -> bool;
    /// `insert(...)` (:249-361) — reached from FoundConnectionInserter.java:754; this is the call
    /// that reaches `BasicBoard.insertVia` → `splitTraces` (plan-6 ruling 6 / plan-3 ruling F).
    pub fn insert(board: &mut Board, …, stop: StopCheck<'_>) -> Result<bool, BoardError>;
}
```

**Transcription notes.** `checkLayer` (`:30-129`) is a two-phase gate: a `checkForcedPad` for the **via** shape, then a second for the **trace** shape, with the `DRILLABLE_WITH_ATTACH_SMD` promotion when either says so (`:118-124`) and `NOT_DRILLABLE` short-circuits (`:88-95`, `:117-118`). `checkForcedPad` itself builds a `ShapeTraceEntries` over the pad shape, consults `DrillItemMover.tryShoveViaPoints` and `DrillItemMover.check`, then runs `TraceShover.check` per substitute trace piece (`:300-340`) — every one of which Task 9 delivered. `ForcedViaInserter::insert` is the only mutating method in this task and the only one that needs the `StopCheck`.

**Tests (`crates/fr-router/tests/forced_via.rs`):**
- `check_layer_promotes_to_attach_smd_when_either_phase_does` and `…short_circuits_on_the_first_not_drillable`.
- `check_agrees_with_insert_on_a_board_where_the_via_fits` / `…where_it_does_not`.
- `insert_splits_the_traces_it_crosses` — the ruling-F path, with the `StopCheck` never tripping.
- `insert_stops_when_the_stop_check_trips` — a ladder board (quirk #76's minimal repro from Plan 3) with a `StopCheck` that trips after n calls; assert `BoardError::Stopped`, not a hang.
- A JVM-probe table of `checkLayer` answers over 200 random (location, layer pair, net) triples on a 4-layer board — `0 diffs`.

Steps: tests → fail → implement → pass → fmt/clippy/test/audit → commit `feat(router): ForcedPadRouter.checkForcedPad and the ForcedViaInserter trio`.

---

### Task 11: `MazeSearchEngine` part A — construction, `init`, and the pop loop

**Files:** `crates/fr-router/src/autoroute/maze/{search,queue}.rs`; `crates/fr-router/tests/maze_search.rs`.
**Java:** `maze/MazeSearchEngine.java` — ctor `:75-133`, `getInstance` `:135-152`, `reduceTraceShapesAtTiePins` `:154-172`, `segmentProjection` `:173-205`, `toImpactedPoints` `:287-298`, `findConnection` `:300-312`, `occupyNextElement` `:314-384`, `doorIsSmall` `:763-789`, `init` `:969-1103`, `Result` `:1218-1232`, `ShoveResult` `:1233-…`.

**Interfaces produced:**
```rust
/// Port of `maze.MazeSearchEngine` (MazeSearchEngine.java:38-1257) — the A* over expansion doors.
pub struct MazeSearchEngine<'a> {
    engine: &'a mut AutorouteEngine,
    ctrl: &'a AutorouteControl,
    queue: MazeQueue,                        // :51  the overridden TreeSet (Task 8)
    destination_distance: DestinationDistance, // :57
    random: JavaRandom,                      // :63  seeded with ctrl.ripupCosts at :79-80
    destination_door: Option<ExpandableRef>, // :70
    section_no_of_destination_door: i32,     // :72
}
impl<'a> MazeSearchEngine<'a> {
    /// `getInstance(Set<Item>, Set<Item>, AutorouteEngine, AutorouteControl)` (:135-152) — `None`
    /// where Java returns `null` (an `init` that found no start door).
    pub fn get_instance(start: &BTreeSet<ItemId>, dest: &BTreeSet<ItemId>,
                        engine: &'a mut AutorouteEngine, board: &mut Board,
                        ctrl: &'a AutorouteControl, stop: StopCheck<'_>) -> Option<Self>;
    /// `findConnection()` (:300-312): `while (occupyNextElement());`
    pub fn find_connection(&mut self, board: &mut Board, stop: StopCheck<'_>) -> Option<MazeResult>;
    /// `occupyNextElement()` (:314-384).
    pub fn occupy_next_element(&mut self, board: &mut Board, stop: StopCheck<'_>) -> bool;
}
/// Port of the nested `MazeSearchEngine.Result` (:1218-1232).
pub struct MazeResult { pub destination_door: ExpandableRef, pub section_no_of_door: i32 }
```

**`init` (`:969-1103`), transcribed step by step** — this is where a mistake makes every later task's differential lie:
1. `reduceTraceShapesAtTiePins` on **both** sets (`:154-172` → `ShapeSearchTree::reduce_trace_shape_at_tie_pin`, `crates/fr-board/src/searchtree/shape_search_tree.rs:1567`).
2. Every destination item gets `setStartInfo(false)` and its tree-shape bounding boxes joined into the `DestinationDistance` (`:978-985`). **Stop check at `:975`.**
3. The fanout special case joins the whole board box on layers 0 and n−1 (`:988-994`).
4. Every start item gets `setStartInfo(true)` and one `IncompleteFreeSpaceExpansionRoom` per tree shape (`:1012-1022`). **Stop check at `:1009`.**
5. Each incomplete start room is completed (`:1038`, Task 6). **Stop check at `:1035`.**
6. Every `TargetItemExpansionDoor` of the completed start rooms that is **not** a destination door seeds the queue with `expansionValue = 0` and `sortingValue = destinationDistance.calculate(centre, layer)` (`:1048-1082`). **Stop check at `:1051`.**
7. `false` when no start door was seeded → `getInstance` answers `None` → `autoroute_connection` reports `FAILED`.

**`occupy_next_element` (`:314-384`), transcribed:** stop check at `:323`; pop `iterator().next()` + `remove` (`:327-329`) and skip elements whose section is already occupied; copy the backtrack/ripup fields onto the `MazeSearchElement` (`:342-346`); dispatch — `DrillPage` → `expand_to_drills_of_page`; a destination `TargetItemExpansionDoor` → terminate (`:353-359`); fanout terminates on the first drill-reached-from-a-drill (`:361-368`); an `ExpansionDrill` not reached from a drill → `expand_to_other_layers` (`:369-373`); otherwise `expand_to_room_doors` (`:376`). Tasks 12 and 13 supply the four expanders; here they are trait-object-free `pub(crate) fn` stubs whose signatures are fixed by this task's tests.

**Tests (`crates/fr-router/tests/maze_search.rs`):**
- `init_seeds_one_element_per_non_destination_target_door` — a two-pin board; assert the queue's contents and their `sortingValue`s against a JVM probe.
- `init_returns_none_when_no_start_door_exists`.
- `each_of_the_five_stop_sites_aborts_where_java_does` — five tests, each with a `StopCheck` that trips on the *n*-th call, asserting the partial state matches the Java site (this is ruling 6's evidence).
- `the_queue_pops_the_lowest_sorting_value_and_removes_it`.
- `an_already_occupied_section_is_skipped_without_expanding`.
- `the_fanout_gate_refuses_an_element_beyond_the_max_escape_length` (ruling 8's copied settings).

Steps: tests → fail → implement → pass → fmt/clippy/test/audit → commit `feat(router): the maze search engine's init, queue and pop loop`.

---

### Task 12: `MazeSearchEngine` part B — room-door expansion and the cost model; `MazeTraceShover`

**Files:** `crates/fr-router/src/autoroute/maze/{expand,trace_shover}.rs`; `crates/fr-router/tests/maze_expand.rs`.
**Java:** `MazeSearchEngine.java` — `expandToRoomDoors` `:390-626`, `expandToTargetDoors` `:629-705`, `expandToDoor` `:707-761`, `doorIsSmall` `:763-789`, `expandToDoorSection` `:791-966`, `roomShapeIsThick` `:1105-1128`, `shoveTraceRoom` `:1130-1205`, `checkNeckDownAtDestPin` `:1207-1216`; `maze/MazeTraceShover.java (357)` — `checkShoveTraceLine` `:31-…`, the `TraceShover.check` call at `:202`, the stale-index guard at `:64-66`.

**The cost model, transcribed with its constants** (`expandToDoorSection` `:791-966` — every number below is read from Java, not chosen):
- the bend penalty (`:855-873`) tests the cross product of the incoming and outgoing directions and charges `ctrl.bendCosts[layer]` when `sin² > 0.01`;
- `expansionValue = from.expansionValue + addCosts + bend + weightedDistance(prevMid, traceCosts[layer].horizontal(), traceCosts[layer].vertical())` (`:875-882`);
- `sortingValue = expansionValue + destinationDistance.calculate(mid, layer)` (`:883`);
- `roomRipped` propagation at `:885-887`.

**`expandToRoomDoors` (`:390-626`), the branch list:** the layer-active gate (`:396-401`); `doorIsSmall` (`:763`); neckdown at a destination pin (`:409`) and at a start pin (`:442-451`); `completeNeighbourRooms` (`:419`, Task 6); the `nextRoomIsThick` determination (`:453-477`); the split-plane drill guard (`:478-489`); `expandToTargetDoors` (`:629`); the ripup branch (`:513-557`, Task 13's `MazeRipupResolver`); `shoveTraceRoom` (`:1130`); then **a snapshot of the door list** is iterated (`:559`) skipping `toDoor == listElement.door` — take the snapshot, because completing a neighbour mid-iteration mutates the list; then the drill pages (`:601-623`).

**`MazeTraceShover`** is check-only: `checkShoveTraceLine` computes the candidate door sections and calls `TraceShover::check` (`:202`, Task 9), never a `shove*`. Its stale-index guard (`:64-66`) is hazard N — port the silent `continue` verbatim.

**Tests (`crates/fr-router/tests/maze_expand.rs`):**
- `the_bend_penalty_fires_exactly_above_sin_squared_one_percent` — three direction pairs straddling the threshold.
- `expansion_value_is_the_java_sum` — a table of (from, to, layer) → value from a JVM probe.
- `a_small_door_is_refused_in_each_of_the_three_regimes` (`doorIsSmall` measures by bounding box / bounding octagon / diagonal segment per regime).
- `an_inactive_layer_is_never_expanded_into`.
- `the_door_snapshot_is_taken_before_completing_neighbours` — a board where completing a neighbour appends a door; assert the new door is **not** visited in this round (Java's snapshot semantics).
- `shove_trace_room_does_not_mutate_the_board` (the check-only property).
- `a_stale_tree_index_is_skipped_silently` (hazard N).

Steps: tests → fail → implement → pass → fmt/clippy/test/audit → commit `feat(router): room-door expansion, the maze cost model and the check-only trace shover`.

---

### Task 13: `MazeExpansionEngine`, `MazeRipupResolver` and `Connection`

**Files:** `crates/fr-router/src/autoroute/maze/{expansion_engine,ripup_resolver}.rs`, `src/autoroute/path/connection.rs`; `crates/fr-router/tests/{maze_drills,ripup}.rs`.
**Java:** `maze/MazeExpansionEngine.java (415)` — `expandToDrill` `:31-114`, `expandToDrillPage` `:115-144`, `expandToDrillsOfPage` `:145-236`, `expandToOtherLayers` `:237-375`, `checkLayerWithAnyMatchingVia` `:377-414`; `maze/MazeRipupResolver.java (269)` — `calcFanoutViaRipupCostFactor` `:35-66`, `checkRipup` `:72-197`, `checkLeavingRippedItem` `:200-217`, `enterThroughSmallDoor` `:219-268`; `path/Connection.java (155)` — `get` `:39-…`, `getDetour`.

**Interfaces produced:**
```rust
/// Port of `maze.MazeExpansionEngine` (MazeExpansionEngine.java:20-414): the drill/layer half of
/// the expansion, extracted from `MazeSearchEngine` at HEAD (it does not exist upstream).
pub(crate) struct MazeExpansionEngine;
impl MazeExpansionEngine {
    pub(crate) fn expand_to_drill(...);            // :31-114
    pub(crate) fn expand_to_drill_page(...);       // :115-144
    pub(crate) fn expand_to_drills_of_page(...);   // :145-236
    pub(crate) fn expand_to_other_layers(...);     // :237-375
}
/// Port of `maze.MazeRipupResolver` (MazeRipupResolver.java:20-268).
pub(crate) struct MazeRipupResolver;
impl MazeRipupResolver {
    /// `checkRipup(...)` (:72-197). `-1` = not rippable; `ALREADY_RIPPED_COSTS = 1`
    /// (MazeSearchEngine.java:44) when the previous room held the same item (:92-94).
    pub(crate) fn check_ripup(...) -> i32;
    pub(crate) fn check_leaving_ripped_item(...) -> bool;   // :200-217
    pub(crate) fn enter_through_small_door(...) -> bool;    // :219-268
}
/// Port of `path.Connection` (Connection.java:14-154) — the memoised connection of an item, which
/// the ripup cost divides by; stored as `ConnectionId` in `ItemAutorouteInfo` (ruling 15).
pub struct Connection { pub items: Vec<ItemId>, pub detour: f64, … }
```

**The ripup cost model, transcribed (`:100-168`):** `costFactor` is the trace half width, or for a via `max(halfWidth) * 0.5 * (contacts - 1)` (`:100-126`); `ripupCost = ctrl.ripupCosts * costFactor` then `/ detour * fanoutViaCostFactor` (`:128-168`), clamped to `[1, i32::MAX / 100]`. `detour` comes from `Connection::get(obstacleItem).detour()` (`:135-137`), memoised through `ItemAutorouteInfo.precalculatedConnection`. **The one randomness source in Plan 6** is `:158-163`: `randomize = ctrl.ripupPassNo >= 4 && ctrl.ripupPassNo % 3 != 0`, then `detour *= 0.5 + r*r` with `r = search.randomGenerator.nextDouble()` — the `JavaRandom` seeded from `ctrl.ripupCosts` at `MazeSearchEngine.java:79-80` (ruling 5).

**`expand_to_other_layers` (`:237-375`)** computes the drillable layer span with `ForcedViaInserter::check_layer` (`:391`, Task 10) and matches `ctrl.viaInfos` masks (`:331-345`) — transcribe the mask loop's bounds, including `viaLowerBound`/`viaUpperBound`.

**Tests:**
- `crates/fr-router/tests/maze_drills.rs`: `the_layer_span_matches_the_via_mask`, `a_blocked_layer_truncates_the_span`, `expand_to_drills_of_page_expands_every_drill_once`, and a JVM-probe table of expansion values for drill transitions.
- `crates/fr-router/tests/ripup.rs`: `a_via_cost_factor_scales_with_its_contact_count`; `an_already_ripped_item_costs_one`; `the_cost_is_clamped_to_max_int_over_one_hundred`; `pass_four_randomises_and_pass_six_does_not` (the `ripupPassNo % 3` gate); **`the_random_draw_matches_the_jvm`** — seed 1000/5000/17, three draws each, literals from `jshell` with the command in a comment (ruling 5's evidence); `the_detour_is_memoised_through_the_item_autoroute_info`.

Steps: tests → fail → implement → pass → fmt/clippy/test/audit → commit `feat(router): drill/layer expansion, the ripup cost model and the memoised connection`.

---

### Task 14: `FoundConnectionLocator` — all three regimes

**Files:** `crates/fr-router/src/autoroute/path/{locator,locator_45,locator_any_angle}.rs`; `crates/fr-router/tests/locator.rs`.
**Java:** `path/FoundConnectionLocator.java (570)` — `getInstance` `:185-207`, `ninetyDegreeCorner` `:329-342`, `fortyfiveDegreeCorner` `:343-389`, `calculateAdditionalCorner` `:390-404`, the backtrack walk and `connectionItems` construction; `path/FoundConnectionLocator45Degree.java (357)`; `path/FoundConnectionLocatorAnyAngle.java (455)`.

**Interfaces produced:**
```rust
/// Port of `path.FoundConnectionLocator` (FoundConnectionLocator.java:30-569): walks the maze
/// result backwards and turns it into the list of items to insert.
pub struct FoundConnectionLocator {
    pub start_layer: usize, pub target_layer: usize,
    pub connection_items: Option<Vec<ConnectionItem>>,   // `null` is Java's SKIPPED signal
}
impl FoundConnectionLocator {
    /// `getInstance(Result, AutorouteControl, ShapeSearchTree, AngleRestriction, SortedSet<Item>,
    /// Map<Item,Integer>)` (:185-207). **90° and 45° share `FoundConnectionLocator45Degree`**;
    /// only any-angle uses `FoundConnectionLocatorAnyAngle`.
    pub fn get_instance(result: &MazeResult, ctrl: &AutorouteControl, engine: &AutorouteEngine,
                        board: &mut Board, angle: AngleRestriction,
                        ripped: &mut BTreeSet<ItemId>, ripup_costs: Option<&mut BTreeMap<ItemId, i32>>)
        -> Option<FoundConnectionLocator>;
}
/// One entry of Java's `connectionItems` list: a trace run on one layer, or a via.
pub enum ConnectionItem { Trace { corners: Vec<Point>, layer: usize }, Via { location: Point, layer: usize } }
```

**Transcription notes.** The corner insertion is where the three regimes differ: `calculateAdditionalCorner` (`:390-404`) dispatches to `ninetyDegreeCorner` (`:329`) or `fortyfiveDegreeCorner` (`:343`); the any-angle subclass overrides the whole walk. The `null` `connectionItems` is not an error — `AutorouteEngine.autorouteConnection:230-235` turns it into `SKIPPED` with the message "No new connections were made between …". `// not ported:` the hard-coded net-33/66/67/94/98 trace blocks at `:78-100` and `:238` (ruling 14).

**Tests (`crates/fr-router/tests/locator.rs`):**
- `ninety_and_fortyfive_share_one_implementation` (`:195-200`) and `any_angle_does_not`.
- `a_single_room_connection_yields_one_trace_with_the_java_corners` — literals from `p6t1`.
- `a_layer_change_yields_a_via_between_two_traces`.
- `an_empty_connection_item_list_is_skipped_not_failed`.
- One fixed case per regime over the same maze result, asserting the three corner lists differ as Java's do.

Steps: tests → fail → implement → pass → fmt/clippy/test/audit → commit `feat(router): the found-connection locators for all three angle regimes`.

---

### Task 15: `FoundConnectionInserter`, and Plan 3's ruling F closed

**Files:** `crates/fr-router/src/autoroute/path/inserter.rs`; `crates/fr-board/src/board/{mod.rs,normalize.rs}` (the `StopCheck` parameters), `crates/fr-dsn/src/parser/wiring.rs` (pass the reader's existing `StopCheck` through and **delete the `// obligation:` marker at `:596`**); `crates/fr-router/tests/inserter.rs`, `crates/fr-board/tests/insert_via_stop.rs`.
**Java:** `path/FoundConnectionInserter.java (807)` — `getInstance` `:40-110`, `insertVia` `:683-782` (with `ForcedViaInserter.check` at `:708` and `.insert` at `:754`), the trace insertion loop, `LinkedHashSet` at `:462`; `board/facade/BasicBoard.java:268-295` (`insertVia`), `:310-330` (`insertEscapeVia`), `:891-907` (`splitTraces`).

**Interfaces produced:**
```rust
/// Port of `path.FoundConnectionInserter` (FoundConnectionInserter.java:25-806) — the only class
/// in Plan 6 that mutates the board's item set.
pub struct FoundConnectionInserter;
impl FoundConnectionInserter {
    /// `getInstance(FoundConnectionLocator, RoutingBoard, AutorouteControl)` (:40-110). `None`
    /// where Java returns `null` (an insert that failed), which `autorouteConnection:271-277`
    /// turns into FAILED.
    pub fn insert(board: &mut Board, connection: &FoundConnectionLocator,
                  ctrl: &AutorouteControl, stop: StopCheck<'_>) -> Option<InsertedItems>;
}
/// What the insert produced — the acceptance surface of plan-6 ruling 1(b).
pub struct InsertedItems { pub traces: Vec<ItemId>, pub vias: Vec<ItemId> }

// crates/fr-board/src/board/mod.rs — ruling 6 / plan-3 ruling F
impl Board {
    /// Port of `BasicBoard.insertVia` (BasicBoard.java:268-295). **Takes a `StopCheck`**: the
    /// `fromLayer..toLayer` loop reaches `splitTraces` → `PolylineTrace.split`, quirk #76's
    /// machinery, and the router is the caller Plan 3 deferred to (plan-3 ruling F).
    pub fn insert_via(&mut self, …, stop: StopCheck<'_>) -> Result<ItemId, BoardError>;
    pub fn insert_escape_via(&mut self, …, stop: StopCheck<'_>) -> Result<ItemId, BoardError>;
    pub fn split_traces(&mut self, location: &Point, layer: usize, net_number: i32,
                        stop: StopCheck<'_>) -> Result<bool, BoardError>;
}
```

**Transcription notes.**
- The `StopCheck` addition is a breaking change to three `fr-board` signatures. Every existing caller passes `&|| false` **except** `fr-dsn`'s `Wiring.readViaScope` path, which passes the reader's existing `StopCheck` — that is what closes the obligation. `p2t11`/`p2t15` (the randomised board drivers) must be re-run to prove no behaviour changed with a never-tripping check: **record both drivers' `0 diffs` in the commit message.**
- `insertVia` (`:683-782`): `ForcedViaInserter.check` first (`:708`), then `.insert` (`:754`); a failed check is not an error, it is a `false` that makes the caller try the next candidate.
- `FoundConnectionInserter.java:462`'s `LinkedHashSet` is insertion-ordered — a `Vec` with a membership check, `// renamed:` noted, **not** a `BTreeSet` (which would reorder).

**Tests:**
- `crates/fr-board/tests/insert_via_stop.rs`: `insert_via_with_a_never_tripping_check_is_byte_identical_to_before` (compare `structural_hash()` against a golden from before the change); `insert_via_on_a_ladder_board_stops` (quirk #76's four-rung repro) → `BoardError::Stopped`, not a hang, with a wall-clock bound on the test.
- `crates/fr-router/tests/inserter.rs`: `inserting_a_two_corner_connection_produces_one_trace_with_javas_polyline`; `a_layer_change_produces_a_via_at_javas_location_with_javas_padstack`; `a_refused_forced_via_check_answers_none_not_an_error`; `the_insert_order_is_javas_insertion_order` (the `LinkedHashSet`).

Steps: tests → fail → thread the `StopCheck` → re-run `p2t11`/`p2t15` → implement the inserter → pass → fmt/clippy/test/audit → commit `feat(router): FoundConnectionInserter, and a stop-checked insert_via closing plan-3 ruling F`.

---

### Task 16: `autoroute_connection` end to end, and the Plan 6 half of `route`

**Files:** `crates/fr-router/src/autoroute/maze/engine.rs` (the `autoroute_connection` body), `src/lib.rs` (the exported surface); `crates/fr-router/tests/autoroute_connection.rs`.
**Java:** `maze/AutorouteEngine.java:130-280` (`autorouteConnection`) and `:282-287` (`describeConnection`); `autoroute/pipeline/AutorouteConnectionRouter.java:30-100` (steps 1–5 of `route`, ruling 2).

**Interfaces produced — this is the surface Plan 7 consumes:**
```rust
impl AutorouteEngine {
    /// Port of `AutorouteEngine.autorouteConnection(Set<Item>, Set<Item>, AutorouteControl,
    /// SortedSet<Item>, Map<Item,Integer>)` (AutorouteEngine.java:130-280).
    ///
    /// `ripped` is Java's `SortedSet<Item>` — a `BTreeSet<ItemId>` iterated **descending**
    /// (quirk #44); `ripup_costs` is Java's optional `Map<Item,Integer>`.
    pub fn autoroute_connection(
        &mut self, board: &mut Board,
        start: &BTreeSet<ItemId>, dest: &BTreeSet<ItemId>,
        ctrl: &AutorouteControl,
        ripped: &mut BTreeSet<ItemId>,
        ripup_costs: Option<&mut BTreeMap<ItemId, i32>>,
        stop: StopCheck<'_>,
    ) -> AutorouteAttemptResult;
}

/// Steps 1-5 of `AutorouteConnectionRouter.route` (AutorouteConnectionRouter.java:30-100) —
/// the Plan 6 half of the seam (plan-6 ruling 2). Steps 6-8 (optChangedArea, the necked retry,
/// the strict-DRC rollback) are Plan 7's and are marked as such at the end of this function.
// added in Plan 7: `AutorouteConnectionRouter.retryConnectionNecked` (AutorouteConnectionRouter.java:162)
pub fn route_connection(
    board: &mut Board, engine: &mut AutorouteEngine, item: ItemId, net_no: i32,
    settings: &RouterSettings, trace_costs: &[ExpansionCostFactor],
    ripped: &mut BTreeSet<ItemId>, ripup_costs: &mut BTreeMap<ItemId, i32>,
    ripup_pass_no: i32, stop: StopCheck<'_>,
) -> AutorouteAttemptResult;
```

**`autoroute_connection`, transcribed in Java's order** (each step with its line, each early return with its message):
1. `MazeSearchEngine::get_instance` inside recovery boundary #2 (`:138-143`); `None` → `FAILED` "…because the maze search algorithm could not be created."
2. `find_connection` inside boundary #3 (`:156-161`).
3. `FoundConnectionLocator::get_instance` inside boundary #4 (`:183-192`).
4. **Cleanup runs before every early return** (`:201-206`): `clear()` when `!maintain_database`, else `reset_all_doors()`. Getting this order wrong leaks rooms into the next connection and is invisible until a later fixture routes differently.
5. `searchResult == null` → `FAILED` "…because no connection was found between their nets." (`:208-213`); `autorouteResult == null` → `FAILED` (`:215-219`); an inactive start/target layer → `FAILED` "…because some of their layers are disabled." (`:221-228`); `connectionItems == null` → **`SKIPPED`** "No new connections were made between …" (`:230-235`).
6. Ripped-connection deletion (`:238-263`): `StopConnectionOption::None` when `ctrl.remove_unconnected_vias`, else `FanoutVia` (`:241-245`); collect `connection_items` for every ripped item and the changed net set; `board.remove_items(...)`; `remove_trace_tails(net, option)` per changed net.
7. `FoundConnectionInserter::insert` (`:265`); `None` → `FAILED` "…because the new connection could not be inserted." (`:271-277`); otherwise `ROUTED` (`:279`).
8. `// not ported:` the observer bracketing (`:254-270`) and the net-33/66/67 trace block (`:164-176`).

`route_connection` transcribes `AutorouteConnectionRouter.route:36-90`: `containsPlane` decides `getPlaneViaCosts()` vs `getViaCosts()` (`:38-40`); `AutorouteControl::new` then `ripup_allowed = true`, `ripup_costs = start_ripup_costs * ripup_pass_no`, `remove_unconnected_vias = !settings.is_fanout_enabled()` (`:44-47`, `BatchAutorouter.java:115`); an empty unconnected set → `NoUnconnectedNets` (`:49-52`); a plane net whose connected set holds a `ConductionArea` → `ConnectedToPlane` (`:57-61`); the plane swap of start/dest (`:62-68`); `TimeLimit::new(min(100000 * 2^(ripup_pass_no-1), i32::MAX))` (`:71-74`); `init_autoroute` (`:76-82`); `autoroute_connection` (`:88-90`). Boundary #5 wraps the whole function (`:156`).

**Tests (`crates/fr-router/tests/autoroute_connection.rs`):**
- `routing_the_first_connection_of_rpi_splitter_matches_java` — state, ripped set, and the inserted trace polylines/via positions as literals from `p6t1` (ruling 1(a)+(b)).
- `an_inactive_layer_fails_with_javas_message` and one test per early return, asserting **both** the state and the message.
- `cleanup_runs_before_every_early_return` — after a `FAILED`, the engine holds no complete rooms when `maintain_database` is false, and holds them with every door reset when it is true.
- `each_of_the_five_recovery_boundaries_degrades_like_java` (ruling 7): inject a panic/`Err` at each of the five sites and assert the exact degraded value.
- `the_plane_swap_reverses_start_and_dest`.
- `no_unconnected_items_answers_no_unconnected_nets`.

Steps: tests → fail → implement → pass → fmt/clippy/test/audit → commit `feat(router): autorouteConnection end to end and the Plan 6 half of the connection router`.

---

### Task 17: `p6t1`, the reference generator, and the acceptance ladder

**Files:** `scripts/differential/java/P6T1.java`, `scripts/differential/rust/src/bin/p6t1.rs`, `scripts/differential/{run.sh,README.md}`, `scripts/gen-router-reference.sh`, `tests/reference/router-fixtures.txt`, `tests/reference/<stem>/{router.jsonl,router.meta.txt}`, `tests/parity/src/lib.rs` (a `RouterMetrics` helper); `crates/fr-router/tests/reference_parity.rs`.

**`P6T1.java <dsn> <k> [ripupPassNo]`** — `package app.freerouting.autoroute.maze;` (to reach package-private members), compiled and run in `run.sh`'s `needs_jar=1` mode against the HEAD jar on JDK 25 with `-Djava.awt.headless=true -Duser.language=en -Duser.country=US`. It:
1. reads the board with the real `DsnReader.readBoard` and builds a `RoutingBoard`;
2. picks connections **deterministically without the pipeline** (Plan 7 owns item selection): walk `board.getItems()` in board order (descending id, quirk #63), keep connectable items with a non-empty `getUnconnectedSet(net)`, and take the first `k`;
3. for each, runs ruling 2's steps 1–5 exactly as `AutorouteConnectionRouter.route` does — `AutorouteControl(board, net, settings, viaCosts, settings.getTraceCosts())`, `ripupAllowed = true`, `ripupCosts = startRipupCosts * ripupPassNo`, `removeUnconnectedVias = !settings.isFanoutEnabled()`, `new TimeLimit(...)`, `board.initAutoroute(...)`, `autorouteEngine.autorouteConnection(...)`;
4. prints one JSON line per connection: `{"k":…,"item":…,"net":…,"state":"ROUTED","ripped":[…],"traces":[{"layer":…,"corners":[[x,y],…]}],"vias":[{"x":…,"y":…,"padstack":"…"}],"metrics":{"incompletes":…,"vias":…,"traceLength":…,"violations":…}}` with every `double` through `Double.toString`;
5. prints a header line naming the jar path, its mtime and `java -version`.

`p6t1.rs` prints the same lines from `fr_router::route_connection` over `fr_dsn::read_board`, with the metrics from `fr-drc` (`incompletes`, `violations`) and `Board` (`net_via_count`, `cumulative_trace_length`).

**`scripts/gen-router-reference.sh`** — sibling of `gen-drc-reference.sh`, pinned to the **HEAD** jar:
```bash
JAR="${FREEROUTING_JAR:-$JAVA_DIR/build/libs/freerouting-current-executable.jar}"
JAVA_BIN="${JAVA:-/opt/homebrew/opt/openjdk@25/bin/java}"
while IFS='|' read -r stem dsn max_items; do
  "$JAVA_BIN" -XX:+UnlockExperimentalVMOptions -XX:hashCode=2 \
      -Djava.awt.headless=true -Duser.language=en -Duser.country=US \
      -cp "$JAR:$BUILD" app.freerouting.autoroute.maze.P6T1 "$JAVA_DIR/$dsn" "$max_items" \
      > "$REF/$stem/router.jsonl" 2> "$REF/$stem/java.log"
done < "$REF/router-fixtures.txt"
```
plus `router.meta.txt` per stem (jar path, mtime, `java -version`, hash mode, command line, machine-specific prefixes replaced), and a `--verify-hash-modes` flag that regenerates each stem under `-XX:hashCode=0..4` and requires **five byte-identical** files — the direct check of ruling 1's premise, which the survey verified end-to-end on SES output and this task verifies per connection.

**`tests/reference/router-fixtures.txt`** — `stem|dsn|max_items`, starting with:

| stem | dsn | max_items | why |
|---|---|---|---|
| `router-rpi-splitter` | `fixtures/Issue143-rpi_splitter.dsn` | 8 | smallest board that actually routes (2.6 KB SES) — the MATCH-first fixture |
| `router-dac2020-bm01` | `fixtures/Issue508-DAC2020_bm01.dsn` | 2 | spec §14.3's smoke test, and `Dac2020Bm01RoutingTest.java:23-36`'s numbers |
| `router-j2-reference` | `fixtures/Issue026-J2_reference.dsn` | 8 | `J2ReferenceRoutingTest.java:29` asserts ≤ 3 incompletes over a full run |
| `router-tutorial-board` | `examples/tutorial_board/tutorial_board.dsn` | 8 | the CLI end-to-end board (spec §14.4) |

**Acceptance (ruling 1), recorded per stem and per connection in `crates/fr-router/README.md`:**
- **(a)** state + ripped set identical — required for **every** connection of every stem;
- **(b)** inserted geometry identical — required for `router-rpi-splitter`, and reported (not required) for the rest;
- **(c)** the metric block within spec §9's tolerance — required everywhere: incompletes delta equal, via delta equal, `violations == 0`, trace length within ±10 %.
A connection that reaches (a)+(c) but not (b) is **not** a failure; it is a row in the README with the first differing value and a one-line diagnosis. A connection that fails (a) or (c) is a port bug.

**Tests (`crates/fr-router/tests/reference_parity.rs`):** one test per stem (`#[cfg_attr(debug_assertions, ignore)]` for `router-dac2020-bm01`, following Plan 3's convention), reading `tests/reference/<stem>/router.jsonl` through `parity::require_reference`, routing the same connections and comparing (a)/(b)/(c) per the ladder; plus `references_are_from_the_head_jar` (every `router.meta.txt` names `freerouting-current-executable.jar` and a `2.3.1-SNAPSHOT` line).

Steps: write `P6T1.java` → `p6t1.rs` → generate → `--verify-hash-modes` clean → MATCH on `router-rpi-splitter` → then `router-dac2020-bm01` → README acceptance table → fmt/clippy/test/audit → commit `test(router): p6t1, the HEAD-jar per-connection references and the acceptance ladder`.

---

### Task 18: Ported Java tests, audit to zero, README, quirks, hand-off

**Files:** `crates/fr-router/tests/{java_ports,fixtures}.rs`, `crates/fr-router/README.md`, `crates/fr-router/src/lib.rs` (the roster), `scripts/audit-map/{fr-router.map,fr-board.map}`, `docs/java-quirks.md`, `docs/plan-6-handoff.md`, `docs/plan-2-handoff.md` / `docs/plan-3-handoff.md` / `docs/plan-4-handoff.md` / `docs/plan-5-handoff.md` (tick the Plan 6 obligations).

**The three ported Java suites** (ruling 12), each naming its source file:line in a doc comment: `autoroute/maze/MazeListElementTest` (Task 8 holds the assertions; this file is the named port so the trail is one grep), `autoroute/expansion/SortedRoomNeighboursFactoryTest` (Task 4), `autoroute/RoutableLayersSafetyCheckTest` (Task 8).

**`crates/fr-router/tests/fixtures.rs`** — the single-pass harness standing in for `RoutingFixtureTest`'s assertion family: for each fixture, route the first *k* connections with `max_passes = 1` and assert `incomplete_connections <= bound` and `clearance_violations == 0`. The in-CI smoke test is `dac2020_bm01_one_pass_two_items_leaves_at_most_194_incompletes` (`Dac2020Bm01RoutingTest.java:36`; spec §14.3). The rest are `#[cfg_attr(debug_assertions, ignore)]`. **Quirk #157** (`TestingSettings.setMaxPasses` is first-writer-wins, `TestingSettings.java:52-55`, so the harness's own `setMaxPasses(100)` never overrides the test's `1`) goes in this file's module doc, because it is the reason the bound means what it means.

**Audit to zero.** All of these exit 0 with no `MISSING` and no `UNMAPPED`:
```
scripts/audit-port.sh autoroute            crates/fr-router/src '*.java' scripts/audit-map/fr-router.map
scripts/audit-port.sh autoroute/maze       crates/fr-router/src '*.java' scripts/audit-map/fr-router.map
scripts/audit-port.sh autoroute/expansion  crates/fr-router/src '*.java' scripts/audit-map/fr-router.map
scripts/audit-port.sh autoroute/drill      crates/fr-router/src '*.java' scripts/audit-map/fr-router.map
scripts/audit-port.sh autoroute/path       crates/fr-router/src '*.java' scripts/audit-map/fr-router.map
scripts/audit-port.sh board/actions        crates/fr-router/src \
  'ForcedViaInserter.java ForcedPadRouter.java DrillItemMover.java' scripts/audit-map/fr-router.map
scripts/audit-port.sh board/optimize       crates/fr-router/src 'TraceShover.java' \
  scripts/audit-map/fr-router.map
```
plus `grep -rn "added in Plan 6" crates/` returning **nothing** (the four `fr-board` marker sites of Task 1/2 and the two tree markers of Task 3 are all consumed; the `additionalUpdateAfterChange` markers in `trace_normalize.rs`/`shape_trace_entries.rs`/`board/mod.rs` are consumed by Task 9's `RoutingBoardExt` — **if any cannot be, it is rewritten to `// added in Plan 7:` with the reason, not deleted**), and `grep -rn "item_tree_shape_ref\|item_tile_shape_ref" crates/fr-router/` returning **nothing** (ruling 10).

**Plan 3's `fr-board.map` obligation is discharged here** (ruling 13): write `scripts/audit-map/fr-board.map` covering Plan 2's nine directories and re-run those nine audits under the 4-argument form. Do **not** weaken the script; if a class genuinely has no single home, map it to a glob.

**The `// not ported:` roster in `lib.rs`**, one line each with its reason (ruling 13): `BoardHistory` (202) + `BoardHistoryEntry` (33) — spec §2 puts the undo/history store out of scope; `autoroute/events/**` (6 files, 130 loc) — observers, replaced by Plan 8's `ProgressSink`; `PerformanceProfiler` (168) — profiling with `ConcurrentHashMap` statics; `AutorouteDiagnostic` (32) — a GUI overlay sink; `MazeFanoutDiagnostics` (44) — `FRLogger.trace` payloads. Plus the `// added in Plan 7:` lines for `pipeline/**`, `ItemRouteResult`, `RoutingFailureLog`, `BoardUpdateStrategy`, `ItemSelectionStrategy`, `board/optimize/{TraceTightener*,ViaOptimizer}`, `TraceShover.insert`, `ForcedPadRouter`'s routing half and `DrillItemMover`'s mutating half.

**`docs/java-quirks.md` rows**, numbering continuing from **#154** (Plan 5's last row — re-check before writing; if Plan 5's final wave added more, renumber and say so in the commit message), so Plan 6's first row is **#155**:

> **AMENDMENT (Task 2, controller ruling — binding on every later task).** The re-check this
> paragraph asks for came back positive: Plan 5's final wave landed **#155**
> (`DesignRulesChecker.drcSettings`), so every number in the table below is a **label, not a row
> id**. The register is allocated **contiguously, in the order rows are written**, and Task 2 —
> the first Plan 6 task to write any — took the first three free ids:
>
> | plan label | landed as | row |
> |---|---|---|
> | #160 | **#156** | `ObstacleExpansionRoom.getId` aliases and overflows |
> | #161 | **#157** | `CompleteFreeSpaceExpansionRoom.compareTo` tests one type, casts to another |
> | #165 (+ the null-shape NPE the plan did not anticipate) | **#158** | `IncompleteFreeSpaceExpansionRoom.getId` over a mutable, nullable shape |
>
> **The next free row id is #170.** Task 3 landed **#159** (the 90° `completeShape` override drops
> a room it ignores by shape); Task 4 landed **#160** (the non-transitive
> `SortedRoomNeighbour.compareTo` and its `TreeSet`'s silent drop — plan label #162), **#161** (the
> id tie-break subtracting a room id from an item id — plan label #163) and **#162**, which the
> plan did not anticipate: `SortedRoomNeighbours.calculateNewIncompleteRooms` **does not
> terminate** when `fromRoom.getShape().toSimplex()` has fewer border lines than the shape the
> side numbers were computed against. Task 5 landed **#163**, which the plan did not anticipate
> either: `Sorted45DegreeRoomNeighbours.calculateEdgeIncompleteRoomsOfObstacleExpansionRoom`
> never advances its `currentCorner`, so the degenerate-side guard skips the walk's **last** side
> instead of the degenerate ones — an eight-sided obstacle room gets seven incomplete rooms.
> Task 6 landed **#164**, **#165** and **#166**, none of which the plan anticipated:
> `AutorouteEngine.removeCompleteExpansionRoom` binds `ExpansionDoor`'s **narrowing**
> `otherRoom(CompleteExpansionRoom)` overload and therefore skips every incomplete neighbour —
> which is also what keeps `touchingSides[1]` from throwing on an empty array;
> `completeExpansionRooms` is a **strict subset** of the complete rooms that exist, because
> `SortedRoomNeighbours` builds a room before it commits and both the `edgeRemoved` retry and
> `addCompleteRoom`'s dimension check abandon one with its doors still attached; and
> `completeExpansionRoom`'s `catch` returns a **fresh empty** collection rather than the rooms
> completed so far, which the task brief and the controller's ruling-7 note both had the other
> way round.
> Task 7 landed **#167**, **#168** and **#169**. #167 is the plan's own hazard B, which the plan
> text and the Task 7 brief both label "#164" — that id went to Task 6, so hazard B's row is
> **#167**: `DrillPage.getId` hashes the `netNumber` that `getDrills` overwrites at
> `DrillPage.java:65`, so recomputing a page changes the sort key it is already stored under.
> The plan's *own* label #167 ("`DrillPageArray.overlappingPages` mixes an `int` lower bound with
> a `double` upper bound") got **no row**: the mixture is transcribed and tested
> (`overlapping_pages_uses_javas_mixed_loop_bounds`), but it is not a defect — `j < maxJ` over an
> integer `j` agrees with `j < ceil(maxJ)`, so nothing observable differs from a correct reading;
> only a *truncating* port diverges, and the code comment plus the test carry that. The other two
> rows the plan did not anticipate: **#168**, a cancelled `PolylineArea.splitToConvex` makes
> `getDrills` throw at `DrillPage.java:108` **and** leaves the page memoised as having no drills,
> because `:65-66` install the fresh empty list before the work; and **#169**,
> `AutorouteEngine.removeIncompleteExpansionRoom:370` dereferences the lazily created
> `incompleteExpansionRooms` with no null guard, which — through
> `ExpansionDrill.calculateExpansionRooms` and `completeExpansionRoom`'s swallowing `catch` —
> silently costs **every drill** on an engine that has never had an incomplete room added.
> Tasks 8 and 12 must take the next free id *at the time they write*, re-checking
> `docs/java-quirks.md`'s last row first — **not** the labels below.
> Plan label #165 is **subsumed** by the landed #158 (hazard C and the NPE are one method and one
> row); do not write it again.
>
> **Amendment (Task 4) — hazard F's container.** Ruling 4 and Task 4's brief both prescribe a
> `BTreeSet` for `SortedRoomNeighbours.sortedNeighbours`. **It does not reproduce Java.** On a
> comparator that is not a total order, `std`'s `BTreeSet` (binary search inside a B-tree node) and
> `java.util.TreeSet` (a root-to-leaf walk of a red-black tree) compare different pairs, so they
> **keep different elements and iterate the survivors in different orders** — measured against the
> HEAD jar by `scripts/differential/run.sh p6t3 3`, which diffs in both of those ways with a
> `BTreeSet` and is byte-for-byte with a transcription of `java.util.TreeMap`
> (`crates/fr-router/src/java_tree_set.rs`). Task 8 must re-run the same question before putting
> `MazeListElement` in a `BTreeSet`: its `compareTo` has a documented five-way `Equal` (quirk #156)
> and a NaN fall-through (plan label #155), and neither is a total order either.

| new # | what | Java site |
|---|---|---|
| #155 | `MazeListElement.compareTo` compares `double`s with raw `<`/`>`, so a NaN sorting or expansion value falls **through** to the next tie-break instead of ordering; the port transcribes the fall-through rather than using `total_cmp`. | `maze/MazeListElement.java:80-113` |
| #156 | the same comparator returns `0` on a full four-key tie, and the `TreeSet` **silently drops** the element — one fewer expansion, with a comment in Java admitting it. | `maze/MazeListElement.java:104-112` |
| #157 | `TestingSettings.setMaxPasses` is first-writer-wins, so `RoutingFixtureTest.getRoutingJob`'s own `setMaxPasses(100)` never overrides a test's `setMaxPasses(1)`; every fixture bound in the Java suite depends on that. | `src/test/…/settings/sources/TestingSettings.java:52-55`; `fixtures/RoutingFixtureTest.java:76` |
| #158 | production routing code carries hard-coded debug net numbers (33/66/67 in the engine, 94/98 in the pass runner and the locator) gating `FRLogger.trace`. | `maze/AutorouteEngine.java:164-176`; `path/FoundConnectionLocator.java:78-100,238` |
| #159 (label — real id assigned at write time, ≥ #160) | HEAD-only pure-SMD relaxations with no upstream counterpart: `attachSmdAllowed` is forced true and the via cost factor is scaled by 0.1 when every item of the net is a single-layer pin. Routing-visible; ported from HEAD because HEAD is the parity jar. | `maze/AutorouteControl.java:263-269, 277-281` |
| #160 | `ObstacleExpansionRoom.getId` packs `(itemId << 10) \| indexInItem`, which aliases silently for item ids ≥ 2²¹ or more than 1024 tree shapes — and the id is a sort key. | `expansion/ObstacleExpansionRoom.java:49` |
| #161 | `CompleteFreeSpaceExpansionRoom.compareTo` tests `instanceof FreeSpaceExpansionRoom` and casts to `CompleteFreeSpaceExpansionRoom`; an incomplete room in a sorted set would `ClassCastException`. Unreachable today. | `expansion/CompleteFreeSpaceExpansionRoom.java:46-54` |
| #162 | `SortedRoomNeighbour.compareTo` is **non-transitive** — a ±1 distance tolerance band plus an id tie-break — and the neighbours go into a `TreeSet`, so which neighbour is dropped depends on insertion order. Three separate copies, one per angle regime. | `expansion/SortedRoomNeighbours.java:720-762`; `Sorted45DegreeRoomNeighbours.java:803+`; `SortedOrthogonalRoomNeighbours.java:598+` |
| #163 | the same comparator's final tie-break subtracts a *room* id from an *item* id — two id spaces that collide numerically, because room ids are a per-engine counter. | `expansion/SortedRoomNeighbours.java:757-760` |
| #164 | `DrillPage.getId` includes `netNumber`, and `getDrills` **mutates `netNumber`** — an ordering key that changes while the page may already sit in the maze queue. Same class of bug as #165. | `drill/DrillPage.java:64-65, 190-193` |
| #165 | `IncompleteFreeSpaceExpansionRoom.getId` is `31 * shape.getId() + layer` over a **mutable** shape (`FreeSpaceExpansionRoom.setShape`), and `ExpansionDoor.getId` derives from it. | `expansion/IncompleteFreeSpaceExpansionRoom.java:38-41`; `expansion/FreeSpaceExpansionRoom.java:70` |
| #166 | `DestinationDistance.calculateCheapDistance` temporarily **mutates and restores** the shared `minNormalViaCost` field — non-reentrant; the port passes it as a parameter (`// renamed:`). | `maze/DestinationDistance.java:382-390` |
| #167 | `DrillPageArray.overlappingPages` mixes an `int` lower bound with a `double` upper bound in the same loop; the page set depends on that mixture. | `drill/DrillPageArray.java:76-97` |
| #168 | HEAD-only "stale tree index during routing" guards that silently `continue`, in three places, with no upstream counterpart — load-bearing, ported verbatim. | `maze/MazeSearchEngine.java:655-668`; `autoroute/ItemAutorouteInfo.java:57-79`; `maze/MazeTraceShover.java:64-66` |

Plus anything Tasks 1–17 find, and — in the `candidate`/obligation register — the ruling-9 row **closed** with its probe output, the ruling-F row **closed** by Task 15, and a new **(Plan 7 obligation)** row for `AutorouteConnectionRouter`'s steps 6–8.

**`crates/fr-router/README.md`:** what the crate routes today (one connection) and what it does not (passes, fanout, optimizer — Plan 7); ruling 1's acceptance table filled in per stem and per connection; the six stop-check sites and why a seventh is a bug; the five recovery boundaries; the container rules (BTreeSet everywhere, the guarded push, the three non-transitive comparators); quirk #143's warning that `-mt` must not become a threading policy; how to regenerate the references and run `p6t1`/`p6t2`/`p6t3`.

**`docs/plan-6-handoff.md`:** the delivered surface with every public signature; the seventeen rulings with what execution confirmed or corrected (rulings 1, 4, 9 and 10 must each say what the evidence was); parked residuals per task; obligations:
- **Plan 7** — `AutorouteConnectionRouter.route` steps 6–8; `RoutingBoardExt` gains `opt_changed_area`/pull-tight/the tighteners (and quirk #34's `equals_geometric` at `TraceTightener*.repositionLine`, still open from Plan 2); quirk #74's `change_trace` early return, **still a decision**, now with a production caller in sight (`TraceShover.insert`); `TraceShover.insert`, `ForcedPadRouter`'s routing half and `DrillItemMover`'s mutating half; the pass/item recovery boundaries (`AutoroutePassRunner.java:144`, `BatchAutorouterThread.java:537`) on top of this plan's five; `max_passes == 0` means unlimited (quirk #140); no headless threading policy from `-mt` (quirk #143); the whole-board SES byte-parity headline.
- **Plan 8** — `RoutingPipeline.createForHeadless` wiring, `CancelToken` → this crate's `StopCheck`, `ProgressSink` replacing the dropped observers, and `BoardStatistics` (which the metric harness here stands in for).

Steps: ported tests → fixtures harness → audit to zero **without weakening the script** → `fr-board.map` → roster + README → quirks + hand-off + obligation ticks → fmt/clippy/test → commit `test(router): the ported Java suites, audit to zero, README, quirks #155+ and the Plan 6 hand-off`.

---

## Dispatch order and sizing

1 → 2 → 3 → 4 → 5 → 6 → 7 → 8 → 9 → 10 → 11 → 12 → 13 → 14 → 15 → 16 → 17 → 18.

Two relaxations are available and the controller may take them: **Task 8** (`AutorouteControl`/`DestinationDistance`/`MazeListElement` — pure, no rooms, no tree) can run in parallel with any of Tasks 3–7, and **Task 9** (`RoutingBoardExt` + `TraceShover::check`) can run in parallel with Tasks 3–8; they share no file. Everything else is a hard dependency: Task 3 needs Task 2's room types; Task 4 needs Task 3; Task 6 needs Tasks 3–5; Task 7 needs Task 6's engine; Tasks 11–13 need 6, 7, 8, 9 and 10; Task 14 needs 11–13; Task 15 needs 10 and 14; Task 16 needs everything; Tasks 17 and 18 need Task 16.

Sizing. **Task 3** (`completeShape`/`divideLargeRoom` ×3 regimes, ~1 200 Java LOC of restraining geometry, and the surface `p2t10` skipped), **Task 5** (1 710 LOC of regime-specific neighbour sorting with two more non-transitive comparators), **Task 12** (the cost model — every constant reaches the output) and **Task 16** (the end-to-end order, five early returns and five recovery boundaries) are the four largest: **opus, reviewed twice**. **Tasks 4, 6, 9, 10, 11, 13, 14, 15, 17** are large: **opus** (Task 4 because hazard F is the highest-risk parity site; Task 9/10 because ~1 800 LOC of check-only board actions arrive at once; Task 15 because it changes three `fr-board` signatures under every existing caller; Task 17 because the Java driver is the plan's one artefact that inspection cannot verify). **Tasks 2, 7, 8** are medium: **opus** for 8 (the 250-line `DestinationDistance.calculate`), **sonnet** for 2 and 7. **Tasks 1, 18** are small-to-medium: **sonnet**, with Task 18's audit run by the reviewer on the committed tree.

Reviewers: opus for 3, 4, 5, 6, 9, 10, 11, 12, 13, 14, 15, 16, 17; sonnet otherwise. From Task 3 on, every review runs `scripts/differential/run.sh p6t2` (and `p6t3` from Task 4) and reads the header line to confirm which jar it used. From Task 17 on, every review runs `cargo test -p fr-router --test reference_parity` and `run.sh p6t1 fixtures/Issue143-rpi_splitter.dsn 8` on the committed tree, not on the implementer's word.

## Plan self-review

**Spec coverage.** Spec §9's five named groups map to: **Maze** — `AutorouteEngine` (Tasks 6, 16), `MazeSearchEngine` (11, 12), `MazeExpansionEngine` (13), `MazeRipupResolver` (13), `MazeTraceShover` (12), `AutorouteControl` (8), `DestinationDistance` (8), the maze list/search elements (8, 2). **Expansion** — `ExpansionRoom`, `ExpansionDoor`, `CompleteFreeSpaceExpansionRoom` (2), `SortedRoomNeighbours` + the 45°/orthogonal siblings via `AngleRestriction` (4, 5). **Drill** — `DrillPage`, `DrillPageArray`, `ExpansionDrill` (7). **Path** — `FoundConnectionLocator` in three regimes (14), `FoundConnectionInserter` (15), `Connection` (13). **Batch** and **Optimise** are explicitly Plan 7's (ruling 2), as are the two strategy enums; the spec's "public entry points `batch_fanout`/`batch_autoroute`/`batch_optimize`" are Plan 7's surface, and this plan exports the one function they will call (`AutorouteEngine::autoroute_connection`) plus `route_connection`. Spec §10's cancellation clause ("checked wherever Java checks `is_stop_requested()`, including inside maze search") is ruling 6, enumerated to six sites with a test each; `CancelToken` itself is Plan 8's and maps onto this crate's `StopCheck`. Spec §14.1's ported Java tests: all three in-scope suites land (Task 18); `StrictDrcEnforcementTest`/`BatchAutorouterDebugTest`/`autoroute/pipeline/*` are Plan 7's and are named in the hand-off. Spec §14.3's "`Dac2020Bm01` runs in normal CI as the smoke test" is Task 18's non-ignored fixture test with `Dac2020Bm01RoutingTest.java:36`'s bound. Spec §14.2's reference-generation clause is deviated from the same way Plan 5 deviated (a sibling script, `gen-router-reference.sh`, because this plan needs the HEAD jar and a per-connection driver rather than SES output) with the reason recorded in ruling 11. Spec §4's dependency line is honoured (`fr-router → fr-drc → fr-board`; `fr-drc` and `fr-dsn` are dev-only here) and its external-crate list is **deviated from twice, both recorded**: no `rand` (ruling 5) and no `slotmap` (ruling 16). **Spec §3's router clause is deliberately strengthened** by ruling 1, with the metric wording kept as the fallback — the one place this plan asks for more than the spec, and the survey's 10/10 byte-identical probe is why.

**Deliberately excluded, with citation:** everything above `autoroute_connection` — `autoroute/pipeline/**` (2 900 loc), `board/optimize/**`'s mutating half, `RoutingBoard.optChangedArea`/pull-tight, `RoutingFailureLog`, strict-DRC rollback (ruling 2, Plan 7); `BoardHistory`/`BoardHistoryEntry` (spec §2); `autoroute/events/**` (observers; `ProgressSink` replaces them in Plan 8); `PerformanceProfiler`, `AutorouteDiagnostic`, `MazeFanoutDiagnostics` (Task 18's roster); the six private diagnostic helpers of the two tree subclasses (Task 3, the marker text already in `fr-board`); the hard-coded debug-net trace blocks (ruling 14, quirk #158); `AutorouteEngine`'s observer bracketing (`:254-270`); every `FRLogger` call in 8 750 lines.

**Placeholder scan.** No "TBD", no "add error handling", no "similar to Task N". Every task names its Java files with line ranges, the signatures it produces, its expected values with their provenance (a Java file:line, a JVM probe, or a differential run), its commands and its commit message. **Five places name a decision the implementer must make rather than one this plan makes, each with the default already chosen and the deciding evidence named:** the ruling-9 via re-pointing verdict (Task 8 — the probe commands are written out, the default is "accept re-pointing", both closures are specified); the audit map's cross-crate `../` glob vs a `fr-board.map` row (Task 1/18 — both forms given, "pick one and say which in the commit message", the Plan 5 Task 3 precedent named); whether the 45° and orthogonal comparators are also non-transitive (Task 5 — must be read and classified, not assumed, and quirk #162's row has a slot for the answer); whether the three `additionalUpdateAfterChange` markers in `fr-board` are consumed by Task 9 or re-pointed to Plan 7 (Task 18 — the re-pointing rule is written); and which connections reach acceptance rung (b) (Task 17 — the ladder is defined, the README table is the deliverable). All five are decisions-with-a-default, not gaps. The one number this plan deliberately does **not** transcribe is `AutorouteAttemptState`'s constant list (Task 1 says to read the 14-line file rather than trust a guess) — recorded here so it is visibly a choice.

**Type consistency across tasks.** `AutorouteInfo` (with `ObstacleRoomId`/`ConnectionId`) is fixed in Task 1 and unchanged after; `Arena<T>` and the index newtypes in Task 1; `AutorouteAttempt{State,Result}` in Task 1; `RoomRef`/`ExpandableRef`/`MazeSearchElement` in Task 2 and used unchanged by Tasks 6–16; `AutorouteSearchTreeExt` in Task 3; `CalculationMode` in Task 4; `AutorouteEngine` in Task 6, extended (never re-declared) by Tasks 7, 11–13 and 16; `DrillPageArray`/`DrillPage`/`ExpansionDrill` in Task 7; `AutorouteControl`/`ViaMask`/`DestinationDistance`/`MazeListElement`/`MazeQueue` in Task 8; `RoutingBoardExt`/`CheckDrillResult` in Tasks 9 and 10, extended by Plan 7; `MazeResult` in Task 11; `Connection` in Task 13; `FoundConnectionLocator`/`ConnectionItem` in Task 14; `InsertedItems` in Task 15; `route_connection` in Task 16. `ExpansionCostFactor` is **re-exported**, never redeclared (ruling 8). The plan makes **one API change to `fr-geometry`** (Task 1's `JavaRandom` promotion, behaviour-preserving, guarded by the untouched `polygon_shape` tests), **four to `fr-board`** (Task 1's two id newtypes and the `AutorouteInfo` body; Task 15's `StopCheck` on `insert_via`/`insert_escape_via`/`split_traces`, guarded by re-running `p2t11`/`p2t15`), **one to `fr-dsn`** (Task 15 passes the reader's existing `StopCheck` through and deletes the `// obligation:` marker), and **none to `fr-settings`** or `fr-drc`. Every Plan 1–5 test therefore stays green by construction, and `crates/freerouting` is untouched.
