# fr-router

A behavioral Rust port of freerouting's maze/expansion autorouter (clone HEAD):
`app.freerouting.autoroute.{,maze,expansion,drill,path}` plus the
`board/actions` and `board/optimize` classes the router drives, and the two
`ShapeSearchTree` methods (`completeShape`, `divideLargeRoom`) that only the
router calls.

HEAD is the authority for both the sources and the parity jar. HEAD's
`autoroute/**` has been refactored away from upstream 2.3.0 —
`MazeSearchAlgo` → `maze/MazeSearchEngine`, `LocateFoundConnectionAlgo*` →
`path/FoundConnectionLocator*`, and `MazeExpansionEngine`, `MazeRipupResolver`
and `AutorouteConnectionRouter` do not exist upstream at all — so the pinned
2.3.0 jar of plan-3 ruling 10 is **not** used anywhere in this plan: it would
be a different algorithm.

See `docs/superpowers/plans/2026-08-29-plan-6-router-maze.md` for the scope,
the Plan 6 / Plan 7 seam and the seventeen rulings.

## State: complete (Plan 6 Task 18 of 18, then Plan 7 Task 17 — the last of its 18)

**Plan 7 closed on 2026-09-01.** The crate is no longer a per-connection router:
it routes, fans out and optimises a **whole board**, and `run_pipeline` sequences
the two stages exactly as `RoutingPipeline.run` does. `docs/plan-7-handoff.md` is
what Plan 8 starts from; the sections below are in task order, Plan 6's first and
Plan 7's after them.

### What this crate does now, and what it does not

| in | out (Plan 8's) |
|---|---|
| the whole pipeline: `run_pipeline` → `BatchFanout::fanout_board`, `AutorouteBatchLoop::run`, `BatchOptimizer::run_batch_loop` | the **CLI** (`crates/freerouting`), which is untouched by Plan 7 |
| the score: `fr_router::score::BoardStatistics` and the three score methods | the **result manifest** and its Gson-compatible JSON |
| `ProgressSink` / `RoutingEvent` (spec §10's observer replacement, ruling AK) | the **MCP** surface |
| `RouterStop` / `StopRequestState` / `RouterBudget` (the stop machine, ruling AI) | `CancelToken` — it maps *onto* `RouterStop`, and the mapping must keep the three-state distinction (quirk #202/#214; see the hand-off) |
| SES byte parity against the HEAD jar on eight stems (ruling AM) | `BoardScoreBreakdown`, `ScoringWeightComparison` and `BoardStatistics(byte[], FileFormat)` (ruling 4) |

### `-mt` is dead **everywhere**, and must not become a threading policy

Quirk #143 said `-mt` is dead *headless*. Plan 7 Task 17 re-ran the greps and
extended the row: `BatchAutorouter.autoroutePassMultiThread:411-413` has **no
caller anywhere in `src/main` or `src/test`**, so `AutoroutePassRunner.runMultiThread`
is unreachable, `BatchAutorouterThread.java` (621 loc) has zero live callers, and
`RouterSettings.maxThreads` therefore has **no live reader at all** — its only
three are `AutoroutePassRunner.java:50, 53, 91`, inside `runMultiThread`. The
optimizer's twin (`BatchOptimizerMultiThreaded`, `OptimizeRouteTask`) is reachable
only from `BatchOptimizer.createForGui:56-66`. That is why there is no `rayon` in
this plan and why `src/lib.rs` rosters all five classes `// not ported:` with their
greps: **Plan 8 must not read the silence as an invitation.** A threaded maze would
also be non-deterministic, which would dissolve every acceptance criterion in
ruling 1.

### The five tables Plan 8 will want

| table | where |
|---|---|
| the acceptance ladder, per stem × rung | §"The whole-board acceptance ladder (Task 16, ruling 1 / ruling AM)" |
| ruling 5's ordered-container decisions (comparator totality, key mutability, `JavaTreeSet` sites) | §"Ruling 5's container decisions, confirmed against Java" — and §"`SortedRoomNeighbours` (Task 4), and why the crate has a `JavaTreeSet`" for the maze's |
| ruling AH's `structural_hash` audit | §"`p7t10` and the `structural_hash` audit (controller ruling AH)"; the row-per-field table itself is `crates/fr-board/src/board/snapshot.rs`'s module doc |
| ruling AI's budget knob | §"The budget: what the plan asked for, and what is actually possible" |
| the recovery boundaries | §"The recovery boundaries (ruling 7) — six from Plan 6, two from Plan 7" |

A sixth table Plan 8 needs is **not** here, deliberately: ruling AI's six deadline
read sites, and why a seventh is a bug, live in
`crates/fr-router/src/pipeline/stop.rs`'s module doc, next to the type that
implements them. The short version: the six sites are **two** Java mechanisms —
one job-level flag (`AutorouteBatchLoop:251`, `AutoroutePassRunner:203`) and four
per-stage clocks (`BatchFanout:111, :396`; `BatchOptimizer:172, :308`) that write
only their own `isTimedOut` and leave every other stage running. `poll_deadline`
requests `ALL` and has exactly **one** production call site, the job-level one. A
seventh read site is a bug unless it is job-level, and Plan 8's `CancelToken` joins
at these sites and must not flatten the split.

### Regenerating the references, and running the drivers

```sh
# the whole-board references (Task 16). Needs the clone + JDK 25; timeout(1) bounds
# each stem, because quirk #162 still does not terminate on ~0.4 % of room completions.
./scripts/gen-batch-reference.sh                     # tests/reference/<stem>/batch.{ses,passes.jsonl,meta.txt}
./scripts/gen-batch-reference.sh --meta-only         # rewrite batch.meta.txt from existing outputs
./scripts/gen-batch-reference.sh --verify-driver     # bare jar -de/-do vs P7T9.java, budget live on both
./scripts/gen-batch-reference.sh --verify-hash-modes # five -XX:hashCode modes, one digest each

# the per-connection references (Plan 6 Task 17, extended in Plan 7 Task 8)
./scripts/gen-router-reference.sh
./scripts/gen-router-reference.sh --steps=1-8

# the ten Plan 7 drivers (each `run.sh` half prints the jar it used in its header)
# (argument lists are run.sh's, not the plan's — several grew during execution)
./scripts/differential/run.sh p7t1  <dsn> [passNo]
./scripts/differential/run.sh p7t2  <dsn> [passNo] [maxItems|all]
./scripts/differential/run.sh p7t3  <dsn> [mode] [accuracy] [routeK]
./scripts/differential/run.sh p7t4  <dsn> [mode] [accuracy] [routeK]
./scripts/differential/run.sh p7t5  <dsn> [passNo|maxPasses] [sortingOrder] [order|pin|pass|board]
./scripts/differential/run.sh p7t6  [check|correct|swap|rand|edge]
./scripts/differential/run.sh p7t7  <dsn> [preset] [mode]
./scripts/differential/run.sh p7t8  <dsn> [mode] [routePasses] [items|all]
./scripts/differential/run.sh p7t9  <dsn> [maxPasses] [mode] [optPasses|all] [optItems|all] \
                                    [--fanout on|off] [--optimizer on|off] [--ses <path>] [--passes <path>]
./scripts/differential/run.sh p7t10 <dsn> [steps] [routeK] [warm|raw]
./scripts/differential/sweep-p7t9.sh                 # every stem x every mode, MATCH/XDIFF/SKIP
```

**Every parity run disables ruling AI's budget on both sides** — the Rust half
passes `RouterBudget::disabled()`, the Java half sets `fanout.maxMillisecondsPerPin`
and `optimizer.timeoutString` out of reach. The 1 000 ms `optChangedArea` constants
**cannot** be reflected away (quirk #234: `javac` inlines them, `javap -c -p` shows
`sipush 1000` and no `getstatic`), so the Java half runs with that one live and a
MATCH is the proof it never tripped. `--verify-driver` is what makes that claim
against the *bare jar* rather than against the driver.

### Plan 6's state, unchanged below this line

What exists is the data-model floor the other nine tasks build on, the
search-tree extension that turns a seed shape into expansion rooms, the three
neighbour sorters that turn a completed room into its door list, the
`AutorouteEngine` that owns all of it, the drill pages that manufacture its
layer changes, the four leaf types the maze search itself is written against
(the control block, the cost bound, the queue element and the guarded queue),
the pull-tight family controller ruling AB moved out of Plan 7,
and — from Tasks 9, 10 and 10b — the seam with `fr-board`: `RoutingBoardExt`
plus **both halves** of the four shove algorithms, the `check` family the maze
consults before it commits to a trace or a via and the mutating family that then
performs the shove and inserts the via. Task 11 adds the search's own frame:
`MazeSearchEngine`'s construction, `init` and pop loop; Task 12 adds its body —
the room-door expansion, the A\* cost model and the check-only
`MazeTraceShover`; and Task 13 adds the last two classes of `autoroute/maze` —
`MazeExpansionEngine` (the drill/layer expansion) and `MazeRipupResolver` (the
ripup decision and its cost model) — plus the first member of `autoroute/path`,
`Connection`. **No stub is left under `occupyNextElement`, and
`findConnection` runs end to end.** Task 14 adds the walk that reads its
answer: `FoundConnectionLocator` and its two angle-restricted overrides, which
turn a `MazeResult` into the corner lists `FoundConnectionInserter` (Task 15)
inserts. Tasks 15a and 15b, both controller ruling AB, add what that inserter
calls on every corner list: the pull-tight family, and
`RoutingBoard.insertForcedTracePolyline` / `insertForcedTraceSegment` with the
`TraceShover.springOverObstacles` they drive. **Task 16 closes the loop:**
`AutorouteEngine::autoroute_connection` runs a whole connection end to end —
maze search, locate, ripped-connection deletion, insert — and
`route_connection` is the Plan 6 half of the seam
(`AutorouteConnectionRouter.route` steps 1-5, ruling 2), which is the entry
point Plan 7's pass runner and Task 17's `p6t1` call. **Task 17** turns that loop
on real boards against the HEAD jar — 369 connections on five stems, all three
rungs of ruling 1's ladder, at `ripupPassNo` 1, 2 and 4 — and **Task 17b**
(controller ruling AD) closes the last divergence by reproducing quirk #74.
**Task 18 closes the plan:** the three in-scope Java suites are ported by name in
`tests/java_ports.rs`, `tests/fixtures.rs` is the single-pass stand-in for
`RoutingFixtureTest`'s assertion family (with spec §14.3's DAC2020 smoke test
running in ordinary CI), every audit invocation in the workspace exits 0 with no
`MISSING` and no `UNMAPPED`, and `docs/plan-6-handoff.md` is what Plan 7 starts
from.

**What this crate routes today:** one connection at a time, exactly as
`AutorouteConnectionRouter.route` steps 1-5 do — maze search, locate, delete the
ripped connections, insert, with the shove, spring-over and pull-tight families
underneath. **What it does not:** the pass loop, the fanout pre-pass, the
optimizer, `ViaOptimizer`, `optChangedArea`, `removeItemsAndPullTight` and the
necked retry / strict-DRC rollback / failure-log tail of `route` — all Plan 7's
(ruling 2), all listed in "What Plan 7 inherits" at the foot of this file.

| Item | Where | Java |
| --- | --- | --- |
| `Arena<T>` and its index newtypes | `src/arena.rs` | — (ruling 16) |
| `AutorouteAttemptState` | `src/autoroute/attempt.rs` | `AutorouteAttemptState.java:1-14` |
| `AutorouteAttemptResult` | `src/autoroute/attempt.rs` | `AutorouteAttemptResult.java:1-25` |
| `ItemAutorouteInfo`'s accessors | `src/autoroute/item_info.rs` | `ItemAutorouteInfo.java:10-105` |
| `RoomRef` / `ExpandableRef` | `src/autoroute/expansion/room.rs` | `ExpansionRoom.java`, `CompleteExpansionRoom.java`, `ExpandableObject.java` |
| `FreeSpaceExpansionRoom` | `src/autoroute/expansion/free_space_room.rs` | `FreeSpaceExpansionRoom.java:8-91` |
| `IncompleteFreeSpaceExpansionRoom` | `src/autoroute/expansion/incomplete_room.rs` | `IncompleteFreeSpaceExpansionRoom.java:8-42` |
| `CompleteFreeSpaceExpansionRoom` | `src/autoroute/expansion/complete_room.rs` | `CompleteFreeSpaceExpansionRoom.java:19-209` |
| `ObstacleExpansionRoom` | `src/autoroute/expansion/obstacle_room.rs` | `ObstacleExpansionRoom.java:14-158` |
| `ExpansionDoor` | `src/autoroute/expansion/door.rs` | `ExpansionDoor.java:11-201` |
| `TargetItemExpansionDoor` | `src/autoroute/expansion/target_door.rs` | `TargetItemExpansionDoor.java:11-74` |
| `ExpansionRoomStore` | `src/autoroute/expansion/mod.rs` | `AutorouteEngine`'s room lists + the heap |
| `MazeSearchElement` | `src/autoroute/maze/search_element.rs` | `MazeSearchElement.java:1-40` |
| `SortedRoomNeighbours` + `selectCalculationMode` | `src/autoroute/expansion/sorted_neighbours.rs` | `SortedRoomNeighbours.java:34-807` |
| `Sorted45DegreeRoomNeighbours` | `src/autoroute/expansion/sorted_neighbours_45.rs` | `Sorted45DegreeRoomNeighbours.java:22-982` |
| `SortedOrthogonalRoomNeighbours` | `src/autoroute/expansion/sorted_neighbours_orthogonal.rs` | `SortedOrthogonalRoomNeighbours.java:19-728` |
| `JavaTreeSet` | `src/java_tree_set.rs` | `java.util.TreeMap`'s red-black `put` |
| `AutorouteEngine` (the room lifecycle) | `src/autoroute/maze/engine.rs` | `AutorouteEngine.java:39-675`, minus `autorouteConnection` |
| `DrillPage` | `src/autoroute/drill/page.rs` | `DrillPage.java:21-193` |
| `DrillPageArray` | `src/autoroute/drill/page_array.rs` | `DrillPageArray.java:15-120` |
| `ExpansionDrill` | `src/autoroute/drill/expansion_drill.rs` | `ExpansionDrill.java:17-139` |
| `AutorouteControl` + `ViaMask` | `src/autoroute/maze/control.rs` | `AutorouteControl.java:18-311` |
| `DestinationDistance` | `src/autoroute/maze/destination_distance.rs` | `DestinationDistance.java:11-391` |
| `MazeListElement` | `src/autoroute/maze/list_element.rs` | `MazeListElement.java:11-114` |
| `MazeQueue` | `src/autoroute/maze/queue.rs` | `MazeSearchEngine.java:84-125` (the anonymous `TreeSet`) |
| `MazeSearchEngine`'s frame, `MazeResult`, `ShoveResult` | `src/autoroute/maze/search.rs` | `MazeSearchEngine.java:41-152,287-384,763-789,969-1103,1217-1256` |
| `MazeSearchEngine`'s room-door expansion and cost model | `src/autoroute/maze/expand.rs` | `MazeSearchEngine.java:390-966,1105-1215` |
| `MazeTraceShover`, `DoorSection` | `src/autoroute/maze/trace_shover.rs` | `MazeTraceShover.java:24-357` |
| `MazeExpansionEngine`, `Via.getAutorouteDrillInfo` | `src/autoroute/maze/expansion_engine.rs` | `MazeExpansionEngine.java:23-415`, `Via.java:203-217` |
| `MazeRipupResolver` | `src/autoroute/maze/ripup_resolver.rs` | `MazeRipupResolver.java:25-269` |
| `Connection` | `src/autoroute/path/connection.rs` | `Connection.java:11-155` |
| `AutorouteSearchTreeExt` | `src/autoroute/tree_ext.rs` | `ShapeSearchTree.java:580-693,701-811,1095-1118` + `…45Degree.java:38-86,95-281,288-298,305-486` + `…90Degree.java:38-191,198-322` |
| `RoutingBoardExt` | `src/board_ext/routing_board_ext.rs` | `RoutingBoard.java:96-118, 405-448, 882-905, 1240-1249` |
| `TraceShover` (the two `check`s + `springOver`) | `src/board_ext/trace_shover.rs` | `TraceShover.java:57-411, 592-603, 611-818` |
| `DrillItemMover` (`check` + `tryShoveViaPoints`) | `src/board_ext/drill_item_mover.rs` | `DrillItemMover.java:34-103, 256-325` |
| `ForcedPadRouter` (`checkForcedPad` + `calcFromSide` + `inFrontOfPad`) | `src/board_ext/forced_pad_router.rs` | `ForcedPadRouter.java:42-54, 57-212, 221-340, 471-500` |
| `ForcedViaInserter` (`checkLayer` + `check` + the two private helpers) | `src/board_ext/forced_via_inserter.rs` | `ForcedViaInserter.java:30-247, 363-461` |
| `AutorouteEngine::autoroute_connection` + `describe_connection` | `src/autoroute/maze/engine.rs` | `AutorouteEngine.java:130-280, 282-287` |
| `route_connection` (steps 1-5) | `src/autoroute/maze/engine.rs` | `AutorouteConnectionRouter.java:30-100` |
| `RouterError` | `src/error.rs` | ruling 7's six recovery boundaries |
| `ExpansionCostFactor` | re-exported from `fr-settings` | `AutorouteControl.java:287` |

Two cross-crate prerequisites landed with it:

* **`fr_geometry::JavaRandom`** (ruling 5) — `java.util.Random`, bit for bit,
  promoted out of `polygon_shape.rs` where it was private, and given
  `set_seed` and `next_double`. The maze router's ripup resolver seeds a
  `Random` with `ctrl.ripupCosts` (`MazeSearchEngine.java:63,79-80`) and
  multiplies a detour cost by `0.5 + r*r` (`MazeRipupResolver.java:158-163`),
  so the exact stream decides which item gets torn up. `rand`'s `StdRng`
  diverges on the first draw, which is why it is not a dependency.
* **`fr_board::AutorouteInfo`'s body** (ruling 15) — `start_info`,
  `precalculated_connection: Option<ConnectionId>` and
  `expansion_rooms: Vec<Option<ObstacleRoomId>>`, with the two new ids beside
  `RoomId` in `fr-board`'s `ids.rs`. Ids rather than objects, because
  `fr-board` cannot name `fr-router`'s `Connection` and
  `ObstacleExpansionRoom`, and because keeping the scratch on the item is what
  makes `Board::deep_copy` and `Item::clear_derived_data` drop it wholesale.

Task 2 also discharged **Plan 2's `TreeObject::Room` obligation**: rooms now go
into the same compensated tree as items, through `fr-board`'s additive
`ShapeSearchTree::insert_room` / `remove_room`
(`AutorouteEngine.java:534`, `CompleteFreeSpaceExpansionRoom.java:56-59`), and
the ordering that `TreeObject`'s `Ord` has encoded since Plan 2 — rooms before
items, descending id within each — is asserted against a real mixed tree in
`crates/fr-board/tests/expansion_room_tree.rs` and
`crates/fr-router/tests/expansion_rooms.rs`. `Board::item_shape_layer` was
added at the same time, for `ObstacleExpansionRoom.getLayer`. One obligation
remains: `ShapeSearchTree`'s `tree_shape_of` and `ignore_object` still panic on
a `TreeObject::Room`, so Task 4 must teach the compensated queries to resolve a
room's shape and layer before anything calls `overlapping_tree_entries` over a
tree that holds rooms.

`ExpansionDoor` is complete, `getSectionSegments` included: the door-section
arithmetic (`ExpansionDoor.java:104-172`) is the one piece of real geometry in
the class, and `AutorouteEngine.TRACE_WIDTH_TOLERANCE` — the `int = 2` it needs
— lives on `src/autoroute/maze/mod.rs`, which is where the engine reads it from
rather than declaring a second copy.

`ExpansionRoomStore::clear` takes the tree, because `AutorouteEngine.clear`
(`:306-317`) removes every complete room's leaf **before** it drops the lists;
skipping that would leave `TreeObject::Room` keys in the shared tree naming
arena slots that no longer exist.

## `AutorouteEngine` (Task 6), and the two containers Java has

`src/autoroute/maze/engine.rs` is the room half of `AutorouteEngine.java`:
construction, `initConnection`, `clear`, the incomplete/complete add-and-remove
pairs, `completeExpansionRoom`, `completeNeighbourRooms`, `removeAllDoors`,
`resetAllDoors`, `getRoomsWithTargetItems`, `validate`, `generateRoomIdNo` and
`isStopRequested`. `autorouteConnection` is Task 16's.

Three shape changes, all forced and all documented at the type:

* **`ExpansionRoomStore` is embedded**, not re-declared (Task 2's §3 note), so
  there is exactly one room-id counter.
* **The board is a parameter.** Java's `public final RoutingBoard board` is a
  back-pointer into the object that owns the engine, a cycle the port cannot
  express while keeping `Board: Send + Sync`.
* **`stoppableThread` is a parameter too.** `StopCheck` is a borrowed
  `&dyn Fn() -> bool`; storing one would put a lifetime on `AutorouteEngine`
  and on every type that holds one. Ruling 6 fixes the six cancellation sites,
  and each has the caller's stop flag in scope.

**`completeExpansionRooms` is a separate `Vec<RoomId>`, not the complete-room
arena** — quirk #165. `SortedRoomNeighbours.calculate` builds a room *before*
it knows whether it survives, and both the `edgeRemoved` retry and
`addCompleteRoom`'s dimension check abandon one with its doors still attached.
The probe's one-obstacle board constructs nine rooms and lists six. Every walk
Java writes over `completeExpansionRooms` — `clear`, `initConnection`,
`getRoomsWithTargetItems`, `validate`, `resetAllDoors` — walks the list; the
arena is Java's heap.

**Quirk #164 is why `remove_complete_expansion_room` calls
`other_complete_room`.** `ExpansionDoor` has two `otherRoom` overloads and the
declared parameter type picks one at compile time: `removeCompleteExpansionRoom`
takes a `CompleteFreeSpaceExpansionRoom`, so it binds the narrowing overload and
skips every incomplete neighbour — which is also what keeps its unchecked
`touchingSides[1]` from throwing. Every other site in the port keeps the wide
overload, matching its Java counterpart's declared parameter type.

**Ruling 7's first recovery boundary is `complete_expansion_room`.** Java's
`catch (Exception)` at `:518-521` returns a **fresh empty** `ArrayList`, not the
rooms completed so far — quirk #166, and the opposite of what the task brief
said. The port's `Err` *is* that empty collection: `.unwrap_or_default()`
reproduces `:520` exactly. The boundary is a `catch_unwind`, because the
exceptions the catch exists for are `NullPointerException`s in ported geometry.

**Quirk #162 is a hang, and the engine does not guard it.**
`calculateNewIncompleteRooms` fails to terminate on ~0.4 % of completions, and
`completeExpansionRoom` reaches it through `addCompleteRoom`. A guard would be a
divergence; the wall-clock bound belongs to the caller (Tasks 9 and 17), which
is also where Java's own `TimeLimit` is checked.

Every literal in `tests/engine_rooms.rs` — room ids, room shapes, door counts,
the room-instance counter, the surviving incomplete-room count, the tree's leaf
count — is read off the HEAD jar through
`scripts/differential/java/probes/P6T6Probe.java`, which is committed with its
`javac`/`java` invocation and reflects into the three private lists no public
method exposes.

Two methods are deferred at the site rather than written here:
`invalidate_drill_pages`'s body is Task 7's `DrillPageArray`, and
`initConnection`'s `additionalUpdateAfterChange` loop (`:111-117`) is Task 9's
`RoutingBoardExt` — its body is entirely engine work over a drill-page array
that does not exist yet. Both carry `added in Task N:` markers naming the Java
method.

## `AutorouteSearchTreeExt` (Task 3)

`ShapeSearchTree.completeShape` and `divideLargeRoom` are the two methods
`fr-board` could not carry, because both take and return an
`IncompleteFreeSpaceExpansionRoom`. They live here as an **extension trait**
(`src/autoroute/tree_ext.rs`) — the `RoutingBoardExt` precedent, plan-2 ruling
4 — so `fr-board` stays free of rooms apart from the `TreeObject::Room` key.
The dispatch is on `ShapeSearchTree::angle()`, because
`SearchTreeManager.getAutorouteTree` (`SearchTreeManager.java:147-161`) picks
the Java subclass from exactly that value; there is no subclass hierarchy.

The three regimes are **not** refinements of one another:

| regime | restrains with | divides? | result shapes |
| --- | --- | --- | --- |
| `AngleRestriction::None` (base) | half planes off the obstacle's `Simplex` border lines, ranked by `TileShape.distanceToTheLeft` | yes | whatever `TileShape.intersection` produces |
| `FortyFiveDegree` | one of eight `IntOctagon` ordinates, ranked by `signedLineDistance` (a raw coordinate difference, diagonals halved) | yes, then every shape is replaced by its bounding octagon | `IntOctagon` |
| `NinetyDegree` | one of four `IntBox` edges | **no** — it returns its raw result | `IntBox` |

Two arguments have no Java counterpart. Java reads `this.board` for the item
list and the bounding box, and reaches a stored room through the
`SearchTreeObject` interface; the port cannot, so `complete_shape` resolves
both kinds of stored object itself, out of an `ItemLookup` and an
`ExpansionRoomStore`. That is why it never calls `ShapeSearchTree`'s own
`overlapping_*` family and never reaches the `TreeObject::Room` panics those
still carry (see the obligation above).

The parity evidence is `scripts/differential/run.sh p6t2` — 2 000 random seed
rooms per regime against the HEAD jar, 0 diffs over eight seed/density
configurations — and it closes the gap `scripts/differential/README.md` has
documented since Plan 2: `p2t10` reached "every public `ShapeSearchTree` method
except `completeShape`/`divideLargeRoom`". The three fixed scripts in
`crates/fr-router/tests/tree_ext.rs` are transcribed from that driver's Java
output.

Everything else — the maze search, the neighbour sorting, the drill pages, the
path locators, `RoutingBoardExt` — arrives in Tasks 4-17. The deferral roster
at the foot of `src/lib.rs` names each class and the task or plan that owns it;
`grep -rn "added in Task" crates/fr-router/src` lists what is still owed.

## The five `getId()`s

Four of the five `getId()` implementations in `autoroute/expansion` are
**hashes, not identities**, and every one of the four overflows a Java `int`
silently. They matter because `ExpansionDoor.getId` is the third sort key of
`MazeListElement.compareTo` (ruling 4), so an id collision changes which maze
element is expanded first — a wrong answer with no crash.

| Object | Java | Formula |
| --- | --- | --- |
| `CompleteFreeSpaceExpansionRoom` | `:99-102` | the engine counter — the only true id |
| `ObstacleExpansionRoom` | `:48-51` | `(itemId << 10) \| indexInItem` — aliases, quirk #156 |
| `IncompleteFreeSpaceExpansionRoom` | `:37-41` | `31 * shape.getId() + layer`, shape mutable, quirk #158 |
| `ExpansionDoor` | `:184-190` | `min(id1,id2) * 31 + max(id1,id2)` |
| `TargetItemExpansionDoor` | `:70-74` | `31 * item.getId() + room.getId()` |

Each is transcribed with its `wrapping_*` and pinned by a test. Note that
`AutorouteEngine.generateRoomIdNo` ticks once per
`SortedRoomNeighbours.calculate` **call** — including calls that build no
complete room (`SortedRoomNeighbours.java:193`) and calls whose room the
`edgeRemoved` retry discards (`:111-114`) — so complete-room ids **skip**, and
`RoomId` (the arena index) is a different number. Both are minted in creation
order, which is what makes the arena index order the search tree exactly as
Java's id does.

## House rules

* **Single-threaded** (ruling 17). No `rayon`, no `std::thread`. `Board` stays
  `Send + Sync` for Plan 7's optimizer, but nothing here spawns. Java's
  `-mt` / `maxThreads` is parsed, clamped, mirrored — and then read by nothing
  on the headless path (`docs/java-quirks.md` #143, `docs/plan-4-handoff.md`
  §Plans 6/7), so there is no threading policy to reproduce. This is restated
  here so Plan 7 does not read the plan's silence as an invitation: a threaded
  maze would be non-deterministic and would dissolve every acceptance criterion
  in ruling 1.
* **No new workspace dependencies** (rulings 5, 16, 17). The crate depends on
  `fr-board`, `fr-geometry`, `fr-settings` and `thiserror`, and on nothing else
  (`fr-dsn`, `fr-drc` and `parity` are dev-dependencies, for the fixtures and
  the parity harness). Not `rand` (see `JavaRandom` above), not `slotmap` (see
  `Arena` below), not `rayon`.
* **Deterministic containers, transcribed rather than chosen** (ruling 4, and
  **ruling Y**). A sorted set, never `BinaryHeap` — Java pops
  `mazeExpansionList.iterator().next()` *and re-inserts mutated elements*.
  Comparators transcribe Java's `<`/`>` chains literally, never `total_cmp` and
  never `partial_cmp().unwrap()`: on NaN Java's `<` and `>` are both false and
  the comparison falls through to the next sort key, which both Rust idioms get
  wrong. **Which** sorted set is not a style choice: wherever the comparator is
  not a total order — `SortedRoomNeighbour` (quirk #160) and `MazeListElement`
  (quirk #171) — it is [`JavaTreeSet`](src/java_tree_set.rs), a port of
  `java.util.TreeMap`'s red-black tree, because `BTreeSet`'s B-tree visits a
  different subset of the elements on insert and therefore **drops a different
  one** and orders the rest differently. Measured on `run.sh p6t3` mode 3.
  `BTreeSet` stays, and is used, where the comparator provably is a total order.
  Two further rules come with it. **The maze queue's `push` is guarded**, not a
  plain insert: `MazeSearchEngine`'s anonymous `TreeSet` override
  (`MazeSearchEngine.java:84-125`) refuses any element outside the fanout escape
  window and Java **ignores the boolean it returns**, so `init` can report success
  with an empty queue (quirk #178) — `MazeQueue::push` reproduces both halves, and
  its return value is deliberately discarded at the one site Java discards it.
  And **there are three copies of the non-transitive neighbour comparator**, one
  per angle regime (`SortedRoomNeighbours.java:720-762`,
  `Sorted45DegreeRoomNeighbours.java:803+`, `SortedOrthogonalRoomNeighbours.java:598+`,
  quirk #160). All three were read and classified rather than assumed, and all
  three are non-transitive.
* **No GUI, no `FRLogger`, no observers, no clock, and no static mutable
  state — with one recorded exception**, controller **ruling AE**:
  `fr_geometry::Line` carries a private identity token drawn from a process-wide
  `AtomicU64`, because `PolylineTrace.change` compares `Line`s by **reference**
  (quirk #74) and the difference is board-observable. The contract Plan 7 must
  keep: **a new token wherever Java allocates a new `Line`, and `Copy` — the
  same token — wherever Java passes the same reference on.** The token has one
  reader (`Line::is_same_object`) with one caller (`Board::change_trace`);
  nothing reads its *value*, so no output, ordering, hash or serialised form can
  observe it and runs stay bit-reproducible. Anything else using it to stand in
  for `==` is a bug.
* **`#![forbid(unsafe_code)]`** in this crate root, and in every other workspace
  crate (`fr-geometry`, `fr-board`, `fr-dsn`, `fr-settings`, `fr-drc`,
  `tests/parity` and the `freerouting` binary's `main.rs`). The one `unsafe` left
  in the repository is the `static mut` PRNG in
  `scripts/differential/rust/src/bin/p2t13.rs`, a differential driver rather than
  a crate; `scripts/differential/README.md` names it.
* Deliberate Java bugs are reproduced rather than fixed, each with a
  `// Java bug:` marker at the site and a row in `docs/java-quirks.md`.
* **`// pub seam:`** — a `pub` item with **no caller anywhere in the workspace**
  says on its own line why it is `pub` and who will call it. `rustc`'s `dead_code`
  lint cannot see an uncalled `pub` item in a library, so nothing else would catch
  these. `grep -rn "pub seam:" crates/fr-router/src` finds **eight**, added by the
  Plan 6 final review (finding S7): `AutorouteControl::from_settings` (Plan 7's
  fanout pre-pass is the Java caller), `DrillPageArray::bounds`,
  `CompleteFreeSpaceExpansionRoom::{tree_leaf, room_id, tree_shape_count}`,
  `ExpansionRoomStore::incomplete_list_created`,
  `TraceTightener45::get_angle_restriction` and
  `AutorouteAttemptResult::is_routed`. A ninth leftover, a `Polyline`-returning
  `TraceTightener::pull_tight`, was **removed** in the same wave: it collapsed
  `pull_tight_opt`'s `None` — Java's `return this` — into a clone, which is exactly
  the reference identity quirk #74 depends on, and nothing called it.

## Cancellation: six sites, and why a seventh is a bug (ruling 6)

`AutorouteEngine.isStopRequested` (`autoroute/maze/AutorouteEngine.java:294-304`) is
`timeLimit.limitExceeded() || stoppableThread.isStopRequested()`. Java consults it
in **six** places inside Plan 6's scope, and the port consults it in exactly the
same six, each with a test:

| # | Java | port |
|---|---|---|
| 1-4 | `MazeSearchEngine.init` — the destination-set walk (`:975`), and three more inside the seeding loops (`:1002`, `:1040`, `:1073`) | `src/autoroute/maze/search.rs`, `MazeSearchEngine::init`; `each_of_the_four_init_stop_sites_aborts_where_java_does` |
| 5 | the pop loop, **before** the queue is touched (`:323`) | `MazeSearchEngine::find_connection`; `the_pop_loops_stop_check_aborts_before_the_queue_is_touched` |
| 6 | `DrillPage.getDrills` → `PolylineArea.splitToConvex(stoppableThread)` (`drill/DrillPage.java:103`) | `src/autoroute/drill/page.rs`; `split_to_convex_stops_when_the_stop_check_trips` |

**A seventh site is a bug, not a safety net.** Adding one makes a run stop earlier
than Java's loop shape implies, which changes the board and breaks the acceptance
ladder — the divergence is not "we stopped sooner", it is "we routed something
else". Two consequences are worth spelling out:

* **The inserter is handed `&|| false`** (controller ruling AC). Java checks no stop
  anywhere below `AutorouteEngine.java:265`, so threading the caller's check into
  `FoundConnectionInserter` would abort inserts Java completes. Quirk #76's hang
  stays reachable here exactly as it is in Java; Plan 7 owns the wall clock.
* **`Board::split_traces_checked` consults the check once**, inside the entry walk
  that does not terminate — not once per picked trace. The second site would be the
  seventh.

The `StopCheck`s that plan-3 ruling F threads through `Board::insert_via_checked` /
`insert_escape_via_checked` / `split_traces_checked` are the *same* mechanism, not
extra sites: they exist because `ForcedViaInserter.insert` reaches quirk #76's
machinery from inside the router.

## The recovery boundaries (ruling 7) — six from Plan 6, two from Plan 7

*(The nine numbered slots below run 1-9 because that is how Plan 6 filed them.
**Slot 8 is empty**: `BatchAutorouterThread.java:537` is on a class with zero live
callers in the whole Java tree, so Plan 7 delivered slots 7 and 9 and nothing else —
**two** boundaries, not three. The heading said "three" until Plan 7 Task 17's fix
round; see the paragraph on slot 8 at the foot of this section.)*

Java's `catch (Exception)` sites inside Plan 6's scope, and what each becomes:

| # | Java | port | what it answers |
|---|---|---|---|
| 1 | `AutorouteEngine.completeExpansionRoom:518-520` | `Result`, and **`Err` means an empty collection** (quirk #166) — every caller uses `unwrap_or_default()`, **never** `?` | Java's `return new ArrayList<>()` |
| 2 | `autorouteConnection:139` (maze construction) | `catch_unwind` | `FAILED` |
| 3 | `autorouteConnection:157` (`findConnection`) | `catch_unwind` | `FAILED` |
| 4 | `autorouteConnection:178-190` (the locator) | `catch_unwind` — this is what turns quirk #181's NPE back into Java's `FAILED` | `FAILED` |
| 5 | `AutorouteConnectionRouter.route:155-158` | `catch_unwind` around `route_connection` | a bare `FAILED` |
| 6 | `RoutingBoard.insertForcedTracePolyline:787-841` (ruling AB pulled this method into Plan 6) — the `try` covers `normalize` and `splitTracesAtKeepPoint` | **the `Result` channel, not the panic channel**: `src/board_ext/routing_board_ext.rs:780` and `:787` drop the `Err` and fall through, because `normalize_trace_checked` and `split_traces_at_keep_point` (`board_ext/tightener/base.rs`) are `Result`-returning all the way down and no path below them panics | Java's silent skip — it logs and continues, leaving `newTrace` as it was |

Boundary **7 is Plan 7's, and it landed in Task 9**: `AutoroutePassRunner`'s
per-pass catch. The line this paragraph used to name, `:144`, is the catch of the
**dead** `runMultiThread` (`:40-149`) — plan-7 scan ruling 9 was right about that
much, and wrong to conclude that `runSingleThread` therefore has none.
`runSingleThread` (`:151-336`) opens its **own** `try` at `:156` and closes it with
`catch (Exception e) { … return false; }` at `:331-335`; the `sed -n '140,155p'`
window ruling 9 quoted stops one line short of it. The port is a `catch_unwind`
around the whole of `AutoroutePassRunner::run_single_thread`'s body, degrading to
`Ok(false)` — the same *catch, produce a value, do not propagate* shape as 1-6, and
**not** a per-item boundary, which is the thing ruling 9 was right to forbid: a Java
exception inside the item loop ends the pass rather than skipping one item.

Boundary **9 is Plan 7's other one, and it landed in Task 10** — and it is the
**only one that propagates**. `AutorouteBatchLoop.run:44-56` is not a `catch` at
all: when no layer is both active in the settings and a signal layer, it fires a
`TaskState.CANCELLED` event (`:53-54`) and then **throws**
`IllegalArgumentException` at `:55`, which `RoutingPipeline.run` does not catch,
so it escapes to the job scheduler. Plan-7 ruling 7 makes it
`RouterError::NoRoutableLayer`, which `AutorouteBatchLoop::run` returns and Task
15's `run_pipeline` will pass on. The port fires the event first, as Java does —
`crates/fr-router/tests/batch_loop.rs`'s
`a_board_with_no_signal_layer_errors_and_reports_cancelled` asserts both halves,
and is the port of `RoutableLayersSafetyCheckTest.testRoutingFailsWhenAllLayersDisabledCurrent`.

Boundary **8** is `BatchAutorouterThread.java:537` (per item), on the dead
multithreaded path. **Plan 7 Task 17 closes it as non-existent rather than
deferred**: the class has zero live callers in the whole Java tree (the greps are
in `src/lib.rs`'s roster), so there is no live path for the boundary to sit on and
nothing for Plan 8 to build. Plan 7 therefore delivered **two** recovery
boundaries, not three — which is scan ruling 9's conclusion reached by a different
route than scan ruling 9's argument. All eight `catch` sites catch
`Exception`, not `Throwable`, so none recovers from a stack overflow — quirk #27
crashes both languages; boundary 9 is a `throw` rather than a `catch` and does not
recover from anything by design.

Everything *below* `AutorouteEngine.java:260` deliberately panics rather than
degrading, because Java has no handler there either and Plan 7's necked retry is
the real one.

## `Arena<T>`, and why not `slotmap`

Java's autoroute object graph is cyclic: a room holds its doors, a door holds
both of its rooms, a drill holds one room per layer. `Rc<RefCell<…>>` would
make the engine's borrow of a `Send + Sync` `Board` (plan-2 ruling 11)
unrepresentable, so every room, door, drill and page lives in an `Arena<T>` —
a `Vec<Option<T>>` addressed by a `u32` index — instead.

It has **no generation counter**, deliberately. `slotmap`'s generational keys
would turn a stale index into a `None` where Java reads a live object, which is
a divergence rather than a safety improvement. `Arena::remove` therefore leaves
a permanent hole and **indices are never reused**: `insert` always appends, so
an index stays valid, or permanently empty, for the arena's whole life. The
cost is that a long run's arena grows to the total number of objects ever
created rather than the live count — which is what Java's heap does too, minus
the garbage collector. `Arena::clear` drops the indices as well and is only
sound between connections, when no id from the old arena survives.

## Audit

`scripts/audit-map/fr-router.map` maps every class of the five Java packages
this crate ports (ruling 13). As of Task 7 the package-root invocation reaches
**zero MISSING and zero UNMAPPED**, over Task 1's three classes and over the
whole package root:

```sh
./scripts/audit-port.sh autoroute crates/fr-router/src \
    'AutorouteAttemptState.java AutorouteAttemptResult.java ItemAutorouteInfo.java' \
    scripts/audit-map/fr-router.map
./scripts/audit-port.sh autoroute crates/fr-router/src '*.java' \
    scripts/audit-map/fr-router.map
```

The map's header lists the six further invocations — one per subpackage, plus
`board/actions` and `board/optimize` — that Tasks 3-17 fill in and Task 18 must
drive to zero. Task 2 moved `autoroute/expansion` from 79 MISSING to **13**
(the three `Sorted*RoomNeighbours` classes, Tasks 4-5) and `autoroute/maze`
from 28 to **27**, both at zero UNMAPPED; Task 3 left both untouched, because
`completeShape`/`divideLargeRoom` are `board/searchtree` classes, audited from
`fr-board`. There the two Plan 6 deferral markers on
`crates/fr-board/src/searchtree/shape_search_tree.rs` became `renamed:` markers
naming `AutorouteSearchTreeExt`, and `./scripts/audit-port.sh board/searchtree
crates/fr-board/src` still exits 0. Task 4 took `autoroute/expansion` from 13
MISSING to **6** — exactly the two 45-degree/90-degree siblings, three methods
each — and Task 5 took it to **0**. Task 6 took `autoroute/maze` from 27 MISSING
to **11**: every one of `AutorouteEngine`'s sixteen rows is closed, by a real
`fn` except `autorouteConnection` (`added in Task 16:`), `emitDiagnostics` and
`describeShapeBounds` (`not ported:`, a GUI sink and an `FRLogger` formatter).
The eleven that remain are `AutorouteControl` (2, Task 8),
`DestinationDistance` (3, Task 8), `MazeListElement.compareTo` (Task 8),
`MazeSearchEngine` (4, Tasks 11-13) and `MazeTraceShover` (1, Task 12). Task 7
took `autoroute/drill` from 23 MISSING to **0**: every public method of the
three classes is a real `fn` except the three `emitDiagnostic`/`emitDiagnostics`
sinks, which carry `not ported:` markers naming a GUI overlay. Task 8 took
`autoroute/maze` from 11 MISSING to **4** — every one of its own seven rows is
closed (`AutorouteControl.rebuildViaInfo` by a real `fn`,
`AutorouteControl.ExpansionCostFactor` by a `renamed:` marker naming the
`fr-settings` re-export, `DestinationDistance`'s three by real `fn`s,
`MazeListElement.compareTo` by `compare_to`, and the anonymous `TreeSet`'s
`MazeSearchEngine.add` by a `renamed:` marker on `MazeQueue::push`) — leaving
`MazeSearchEngine` (3, Tasks 11-13) and `MazeTraceShover` (1, Task 12). Task 11
took `autoroute/maze` from 4 MISSING to **1**: `MazeSearchEngine.getInstance`,
`findConnection` and `occupyNextElement` are real `fn`s in
`src/autoroute/maze/search.rs`, and the one that remains is
`MazeTraceShover.checkShoveTraceLine`. Task 12 closed that one too, so
`autoroute/maze` is at **0 MISSING**: `checkShoveTraceLine` is a real `fn` in
`src/autoroute/maze/trace_shover.rs`, and the seven private
`MazeSearchEngine` methods it landed alongside it live in
`src/autoroute/maze/expand.rs`, which the class map already pointed at. Task 13
adds `MazeExpansionEngine` and `MazeRipupResolver`, whose mapped files the class
map already named, so `autoroute/maze` **stays at 0 MISSING with the two new
classes in scope**, and it takes `autoroute/path` from 7 MISSING to **4** —
`Connection`'s `get`, `getDetour` and `traceLength` are real `fn`s in
`src/autoroute/path/connection.rs`; the four that remain are
`FoundConnectionLocator`'s three and `FoundConnectionInserter.getInstance`
(Tasks 15-16). Task 14 takes `autoroute/path` from 4 MISSING to **1**:
`FoundConnectionLocator.getInstance` is a real `fn` in
`src/autoroute/path/locator.rs`, `emitDiagnostics` carries a `not ported:`
marker naming the `AutorouteDiagnostic` sink of ruling 13's roster, and the
nested class's constructor row (`FoundConnectionLocator.ResultItem`, which the
audit reads as a public method of the enclosing class) is closed by
`ResultItem::new` plus a `renamed:` marker. The last one,
`FoundConnectionInserter.getInstance`, was **Task 15's, and Task 15 closed it**:
`autoroute/path` now exits 0 with 0 MISSING / 0 UNMAPPED, so **all five
`autoroute` invocations exit 0**. Task 9 opened the two `board/*` invocations,
each restricted to the files it ports, and both exit 0 with zero MISSING and
zero UNMAPPED:

    ./scripts/audit-port.sh board/actions  crates/fr-router/src 'ForcedPadRouter.java ForcedViaInserter.java DrillItemMover.java' scripts/audit-map/fr-router.map
    ./scripts/audit-port.sh board/optimize crates/fr-router/src 'TraceShover.java TraceTightener.java TraceTightener90.java TraceTightener45.java TraceTightenerAnyAngle.java' scripts/audit-map/fr-router.map

The `board/optimize` glob was one file until controller ruling AB moved the
whole tightener family into Plan 6 (Tasks 15a/15b); the widened form above is
the one that actually covers it. The narrow `'TraceShover.java'` form still
exits 0, but it no longer audits everything this crate ports from that
directory.

The wide `board/actions` glob above (which also names `ForcedPadRouter.java` and
`ForcedViaInserter.java`) started passing in Task 10. There is deliberately **no**
`board/facade` invocation against this crate: `RoutingBoardExt` carries five of
`RoutingBoard`'s methods, and the class's other ~100 stay in `fr-board`, whose
own `board/facade` audit covers them — Task 9 turned the three Plan 6 deferral
markers there (`additionalUpdateAfterChange`, `initAutoroute`,
`checkForcedTracePolyline`) into `renamed:` markers naming this crate, and that
audit still exits 0.

### Every audit invocation in the workspace (Task 18 runs all of them to zero)

**Plan 7 Task 17 owns this section, and re-ran every invocation on the committed
tree.** The result, measured, is the acceptance for the whole plan:

| | count |
|---|---|
| invocations | **33** |
| exiting non-zero | **0** |
| `MISSING` lines | **0** |
| `UNMAPPED` lines | **0** |
| `ROSTERED` lines | **26**, across 10 of the 33 |

Both `MISSING` and `UNMAPPED` *set* the exit code — an `UNMAPPED` class means the
map rotted and the audit silently fell back to the weaker crate-wide search, so it
fails the run exactly as a `MISSING` method does. `ROSTERED` lines are
informational and do not: they name a class whose every public method is answered
by a `not ported:` / `added in Task|Plan N:` marker and none by a real `fn`.

The 26 `ROSTERED` lines, by invocation: `board/state` 3 (`BoardComparator`,
`BoardObserverAdaptor`, `CoordinateTransform`), `datastructures` (fr-board) 4
(`ArrayStack`, `BigIntAux`, `IdentifierType`, `IndentFileWriter`), `management` 1
(`HeadlessBoardManager` — nine methods, all `added in Plan 8:`; **Plan 8 Task 3
consumed all nine, so this line is now 25 and `management` prints nothing** —
six `renamed:` into `fr-core`, one `not ported:` for the dead `createBoard`
(quirk #253), three consumed by Task 0),
`io/specctra/parser` 1 (`SessionToEagle`), `settings/sources` **1**
(`GuiSettingsSource`; ~~**`JsonFileSettings`** — see the hand-off, this is a real
Plan 8 gap and not just a roster line~~ — **Plan 8 Task 5 closed that gap** under
scan ruling R7: `crates/fr-settings/src/sources/json_file.rs`), `util/gson` 3,
`io/kicad` 3, `core/scoring` (fr-router) 2
(`BoardScoreBreakdown`, `ScoringWeightComparison` — ruling 4's Plan 8 rows),
`autoroute` 2 (`BoardHistoryEntry`, `PerformanceProfiler`), `autoroute/pipeline`
**5** (`AutoroutePassRunner`, `BatchAutorouterThread`, `BatchOptimizerMultiThreaded`,
`NamedAlgorithm`, `OptimizeRouteTask`), `core` 0.

**How the count got from 29 to 33.** Plan 6 Task 18 closed with **29**. Plan 7
Task 1 added `core/scoring crates/fr-router/src` (ruling AG) → 30. Plan 7 Task 15b
added `management crates/fr-board/src 'HeadlessBoardManager.java'` (ruling AW) → 31,
the first time any plan audited `management/` at all, which is how the two
board-mutating clearance overrides went unported for six plans while
`applyCopperToEdgeClearanceOverride` mutated 15 of the 16 corpus boards at default
settings (quirk #231). **Task 17 adds the last two**: `autoroute/pipeline`, promoted
out of the "deliberately absent" table below now that it exits 0 with 0 `MISSING`
and 0 `UNMAPPED` (its five `ROSTERED` lines are the multithread family and
`NamedAlgorithm`), and `core`, **scoped by file glob to the four classes Plan 7
Task 4 ports** — `StopRequestState.java StoppableThread.java RouterCounters.java
ProgressThrottler.java`. The glob is not optional: `core '*.java'` would pull
`RoutingJob`, `Session`, `BoardFileDetails`, `RouterJobResourceUsage`,
`RoutingJobPriority`, `RoutingJobState` and `RoutingStage` — all **Plan 8's** — into
this crate's roster, which is exactly the pollution the four map rows at the foot of
`scripts/audit-map/fr-router.map` were written to avoid.

*(Two figures in the older prose were wrong and are corrected here rather than
left: `datastructures` (fr-board) has printed **4** `ROSTERED` lines, not 5, since
some point in Plan 7, and the pre-Task-15b total was therefore **20**, not 21.)*

**Two invocations named in the Task 17 brief are not in the list, and the reason is
that the maps in the tree say they are `fr-board`'s.** The brief asked for
`board/facade crates/fr-router/src 'RoutingBoardOperations.java'` and `board/trace
crates/fr-router/src 'PolylineTrace.java' … fr-board.map`. Measured: the first
prints `UNMAPPED RoutingBoardOperations` and exits 1 (`fr-router.map` has no row for
the class — `fr-board.map:82` maps it to `board/*.rs`), and the second prints **24
`MISSING`** and exits 1, because `fr-board.map:96-97` maps `PolylineTrace` to
`items/*.rs` and `board/trace_normalize.rs` **relative to `fr-board/src`**, which do
not exist under `fr-router/src`. Both classes are already covered, at 0/0, by the
existing crate-wide `board/facade` and `board/trace` invocations against
`crates/fr-board/src` in the nine-directory loop below — which is what the maps
express, and what the brief's own "confirm that is what the maps in the tree
actually express before running it" asked to be checked. Neither invocation is
added.

Copy-pasteable:

```sh
# fr-geometry (Plan 1; scripts/audit-geometry-port.sh is a thin alias for the first)
./scripts/audit-port.sh geometry/planar crates/fr-geometry/src

# fr-board — Plan 2's nine directories, now under the per-class map Task 18 wrote
# (this discharges the Plan 3 obligation; the 3-argument crate-wide form still
# exits 0 and is the weaker check).
for dir in board/model/items board/model/structure board/facade board/searchtree            board/trace board/state rules core/library datastructures; do
  ./scripts/audit-port.sh "$dir" crates/fr-board/src '*.java' scripts/audit-map/fr-board.map
done
./scripts/audit-port.sh drc crates/fr-board/src 'ClearanceViolation.java' scripts/audit-map/fr-drc.map
# fr-board's management slice (Plan 7 Task 15b): the clearance overrides' Java home.
# Silent since Plan 8 Task 3 consumed the nine `added in Plan 8:` markers (six became
# `renamed:` into fr-core, one `not ported:` for the dead `createBoard`); the three
# methods Task 15b ports are private and the script never names them.
./scripts/audit-port.sh management crates/fr-board/src 'HeadlessBoardManager.java' scripts/audit-map/fr-board.map

# fr-dsn (Plan 3)
./scripts/audit-port.sh io/specctra        crates/fr-dsn/src '*.java' scripts/audit-map/fr-dsn.map
./scripts/audit-port.sh io/specctra/parser crates/fr-dsn/src '*.java' scripts/audit-map/fr-dsn.map
./scripts/audit-port.sh io                 crates/fr-dsn/src \
    'CoordinateTransform.java BoardReadResult.java BoardMetadata.java FileFormat.java KiCadNetClassNames.java' \
    scripts/audit-map/fr-dsn.map
./scripts/audit-port.sh datastructures     crates/fr-dsn/src \
    'IdentifierType.java IndentFileWriter.java' scripts/audit-map/fr-dsn.map

# fr-settings (Plan 4)
settings_files='RouterSettings.java LayerSettings.java ScoringSettings.java
OptimizerSettings.java FanoutSettings.java DesignRulesCheckerSettings.java
DebugSettings.java SettingsSource.java SettingsMerger.java GlobalSettings.java'
./scripts/audit-port.sh settings crates/fr-settings/src "$settings_files" \
    scripts/audit-map/fr-settings.map
./scripts/audit-port.sh settings/sources crates/fr-settings/src '*.java' scripts/audit-map/fr-settings.map
./scripts/audit-port.sh util             crates/fr-settings/src 'ReflectionUtil.java' scripts/audit-map/fr-settings.map
./scripts/audit-port.sh util/gson        crates/fr-settings/src '*.java' scripts/audit-map/fr-settings.map

# fr-drc (Plan 5)
./scripts/audit-port.sh drc          crates/fr-drc/src '*.java' scripts/audit-map/fr-drc.map
./scripts/audit-port.sh io/kicad     crates/fr-drc/src '*.java' scripts/audit-map/fr-drc.map
./scripts/audit-port.sh core/scoring crates/fr-drc/src \
    'BoardStatisticsClearanceViolations.java' scripts/audit-map/fr-drc.map

# fr-router's own core/scoring slice (Plan 7 Task 1, ruling AG)
./scripts/audit-port.sh core/scoring crates/fr-router/src '*.java' scripts/audit-map/fr-router.map

# fr-router (Plan 6) — the five autoroute packages plus the two board/* file sets
./scripts/audit-port.sh autoroute           crates/fr-router/src '*.java' scripts/audit-map/fr-router.map
./scripts/audit-port.sh autoroute/maze      crates/fr-router/src '*.java' scripts/audit-map/fr-router.map
./scripts/audit-port.sh autoroute/expansion crates/fr-router/src '*.java' scripts/audit-map/fr-router.map
./scripts/audit-port.sh autoroute/drill     crates/fr-router/src '*.java' scripts/audit-map/fr-router.map
./scripts/audit-port.sh autoroute/path      crates/fr-router/src '*.java' scripts/audit-map/fr-router.map
./scripts/audit-port.sh board/actions  crates/fr-router/src \
    'ForcedViaInserter.java ForcedPadRouter.java DrillItemMover.java' scripts/audit-map/fr-router.map
./scripts/audit-port.sh board/optimize crates/fr-router/src \
    'TraceShover.java TraceTightener.java TraceTightener90.java TraceTightener45.java TraceTightenerAnyAngle.java ViaOptimizer.java' \
    scripts/audit-map/fr-router.map

# fr-router (Plan 7) — the pipeline package, promoted out of the "deliberately
# absent" table by Task 17 (0 MISSING, 0 UNMAPPED, 5 ROSTERED), and the four
# `core/**` classes Task 4 ports, scoped BY FILE GLOB so Plan 8's `core/` classes
# stay out of this crate's roster.
./scripts/audit-port.sh autoroute/pipeline crates/fr-router/src '*.java' scripts/audit-map/fr-router.map
./scripts/audit-port.sh core crates/fr-router/src \
    'StopRequestState.java StoppableThread.java RouterCounters.java ProgressThrottler.java' \
    scripts/audit-map/fr-router.map
```

*(Plan 7 Task 6 added `ViaOptimizer.java` to that last glob. It used to be one of
the three deliberately-absent invocations below, printing a single `ROSTERED`
line; now that `optViaLocation`, `optPlaneOrFanoutVia` and `isWithinTolerance` are
real `fn`s in `board_ext/via_optimizer.rs` — where the map row points — it prints
nothing and the invocation count is unchanged.)*

**One** invocation is deliberately **absent** — `autoroute/pipeline` was the second
until Plan 7 Task 17 promoted it into the list above, and its row is kept here
because the history is the argument for why the promotion is safe:

| invocation | what it prints today | exit |
|---|---|---|
| **absent:** `autoroute/events crates/fr-router/src '*.java' scripts/audit-map/fr-router.map` | **six `UNMAPPED` lines** (the three event classes and their three listener interfaces are not in `fr-router.map`) plus three `ROSTERED` lines. Deliberate: controller ruling AK replaces the whole observer mechanism with `pipeline::ProgressSink`, so mapping the six classes would assert a port that does not and must not exist | **1** |
| **now in the list:** `autoroute/pipeline crates/fr-router/src '*.java' scripts/audit-map/fr-router.map` | **five `ROSTERED` lines** as of **Plan 7 Task 15** (`AutoroutePassRunner`, `BatchAutorouterThread`, `BatchOptimizerMultiThreaded`, `NamedAlgorithm`, `OptimizeRouteTask`) — `RoutingPipeline` just dropped off the list: `run` is `run_pipeline` (a real `fn`), `createForHeadless` is a `renamed:` marker, and `createForGui`/`getAutorouter`/`getOptimizer`/`addStageListener`/`addBoardUpdatedEventListener`/`addTaskStateChangedEventListener` are all `not ported:` in `pipeline/run.rs`, so nothing of the class is `MISSING`. Six until Task 15 (Task 14's report §7); eight until **Plan 7 Task 11**, which gave `BatchFanout` a second map row pointing at `pipeline/fanout.rs` and `renamed:` markers for its three records, so the class drops off the list the way `BatchAutorouter` did; **Plan 7 Task 12** then ported `fanoutBoard`, the last public method it lacked, so the class is now wholly ported rather than merely off the list. Every pipeline class *is* mapped and every method is answered by the Plan 7/8 roster, so nothing is `UNMAPPED` either. It was nine until **Plan 7 Task 8** ported half of `BatchAutorouter`: the class now has **two** map rows (`lib.rs` for what is still deferred, `pipeline/batch_autorouter.rs` for what landed) and drops off the `ROSTERED` list because seven of its twelve public methods are real `fn`s or `renamed:` markers. **Plan 7 Task 9** kept it at eight and at exit 0, but moved `AutoroutePassRunner` onto the list from nowhere: `runSingleThread` is ported, and the class's *only* line `audit-port.sh` sees as a public method is `onBoardUpdatedEvent`, which is not a method of the class at all — it is the single method of an anonymous `BoardUpdatedEventListener` at `:78-85`, inside the dead `runMultiThread`, that the script's line-based extraction attributes to the enclosing file. `ROSTERED` there therefore means "every *public* surface the script can see is rostered", not "nothing landed". | 0 |

Before the Plan 6 final review, the second — and the third, which was
`board/optimize … 'ViaOptimizer.java'` until Plan 7 Task 6 ported the class and
folded it into the main `board/optimize` glob above — exited 0 with **no output at
all**, which is why the `ROSTERED` line exists: a wholly-deferred class must be
visible, not silently indistinguishable from a ported one. `audit-port.sh` does
not recurse, so `src/lib.rs`'s roster is still the only *gate* on these packages,
which is why it names every method rather than the class.

Two things the zero does **not** prove, restated because it is easy to over-read:
the script's positive `fn` match is by name, so under a map it proves the name is
ported *somewhere in the class's mapped files*, and all **eight** `board/facade`
classes map to a `board/*.rs` glob because `Board` is one Rust type assembled from
those eight Java classes (`Item` uses the same glob, so nine map rows resolve to
it). The per-class evidence is the Java citation in every ported body's doc
comment plus the differential drivers below.

## The probe roster

Every number in this README that is not read off a Java source line came from one
of these. All of them run against the clone's **HEAD** build (plan-6's global
constraints make HEAD the parity jar; the pinned 2.3.0 jar of plan-3 ruling 10 is
not used anywhere in this plan). `scripts/differential/run.sh` prints the jar it
used in its header line — read it.

| driver / probe | what it pins | run |
|---|---|---|
| `java/P6T2.java` + `rust/src/bin/p6t2.rs` | `completeShape` / `divideLargeRoom` in all three angle regimes (Task 3) | `./scripts/differential/run.sh p6t2` |
| `java/P6T3.java` + `rust/src/bin/p6t3.rs` | the three neighbour sorters, the non-transitive comparator and `JavaTreeSet` (Tasks 4, 5) | `./scripts/differential/run.sh p6t3` |
| `java/P6T1.java` + `rust/src/bin/p6t1.rs` | **the acceptance ladder** — a whole board, connection by connection (Tasks 17, 17b) | `./scripts/differential/run.sh p6t1 <dsn> <max_items> [ripup_pass_no] [rules]` |
| `java/P6T17bProbe.java` | quirk #74's `Line` reference comparison, and the `keepAt` counts it changes | see the `ripupPassNo > 1` section |
| `probes/P6T6Probe.java` | `AutorouteEngine`'s room lifecycle (Task 6) | committed transcript in `tests/data/` |
| `probes/P6T7Probe.java` | `DrillPage` / `DrillPageArray` / `ExpansionDrill` (Task 7) | ditto |
| `probes/P6T8Probe.java` | `AutorouteControl`, `DestinationDistance`, `MazeListElement`; mode `viadiv` is ruling H's | ditto |
| `probes/P6T10Probe.java` | `ForcedPadRouter` / `ForcedViaInserter`'s check half, 320 rows (Task 10) | ditto |
| `probes/P6T10bProbe.java` | the mutating half and the via-insertion chain (Task 10b) | ditto |
| `probes/P6T11Probe.java` | `MazeSearchEngine`'s frame, 19 modes (Task 11) | ditto |
| `probes/P6T12Probe.java` | the room-door expansion and the A\* cost model, 12 modes (Task 12) | ditto |
| `probes/P6T13Probe.java` | `MazeExpansionEngine`, `MazeRipupResolver`, `Connection`, 14 modes (Task 13) | ditto |
| `probes/P6T14Probe.java` | the three `FoundConnectionLocator` regimes, 9 modes (Task 14) | ditto |
| `probes/P6T15aProbe.java` | the pull-tight family, 11 modes incl. 3×256 random polylines (Task 15a) | ditto |
| `probes/P6T15bProbe.java` | `insertForcedTracePolyline` / `insertForcedTraceSegment` / `springOverObstacles`, 8 modes, 1 621 board dumps (Task 15b) | ditto |
| `probes/P6T15Probe.java` | `FoundConnectionInserter`, 8 modes (Task 15) | ditto |
| `probes/P6T16Probe.java` | `autorouteConnection` end to end, 14 modes × 3 regimes (Task 16) | ditto |
| `probes/P7T2Probe.java` | `BoardHistory`, 61 calls over five phases incl. the `BoardHistoryTest` replay (Plan 7 Task 2) | ditto |
| `probes/P7T4Probe.java` | the three-state stop's full 3x2 transition table, the two queries in every state, `StopRequestState`/`TaskState`/`NamedAlgorithmType`'s variant lists and `RouterCounters`' nine reflected fields (Plan 7 Task 4). **Carries no clock**, deliberately — ruling AI's deadline is asserted against Java's monitor-thread *code*, not against a timing measurement, so the transcript is byte-stable across runs | ditto |
| `java/P7T7.java` + `rust/src/bin/p7t7.rs` | `BoardStatistics`' score subset over a board optionally routed by `P6T1` (Plan 7 Task 1) | `./scripts/differential/run.sh p7t7 <dsn> [routeK] [ripupPassNo]` |
| `java/P7T10.java` + `rust/src/bin/p7t10.rs` | **ruling AH's decision parity** — `getHash`'s three decision sites over 2 000 scripted board mutations (Plan 7 Task 3) | `P7T10_HASH_MODE=0 ./scripts/differential/run.sh p7t10 <dsn> <steps> [routeK] [warm\|raw]` |
| `java/P7T5.java` + `rust/src/bin/p7t5.rs` | `BatchFanout`'s component/pin ordering for **all five** `pinSortingOrder` strings and `RoutingBoard.fanout` on every SMD pin (Plan 7 Task 11), plus one whole `fanoutPass` and the whole `fanoutBoard` (Task 12), each transcribed *and* called for real | `./scripts/differential/run.sh p7t5 <dsn> [passNo\|maxPasses] [sortingOrder] [order\|pin\|pass\|board]` |
| `java/P7T8.java` + `rust/src/bin/p7t8.rs` | `BatchOptimizer`'s item half over a board routed by the real `runBatchLoop()` — `ReadSortedRouteItems`' whole visit sequence (mode `sequence`) and `optRouteItem` driven item by item with its two ripped sets and its ripup costs transcribed beside each call (mode `item`), Plan 7 Task 13. **No reflection**: the driver is in `app.freerouting.autoroute.pipeline`, which is what makes `optimizer.new ReadSortedRouteItems()` legal | `./scripts/differential/run.sh p7t8 <dsn> [sequence\|item] [routePasses] [items\|all]` |
| `java/P7T9.java` + `rust/src/bin/p7t9.rs` | the whole `-dr`-equivalent run: `AutorouteBatchLoop.run` in modes `router-only` / `router+fanout` (Plan 7 Tasks 10 and 12), `BatchOptimizer.runBatchLoop` + `optRoutePass` in modes `optimizer` / `optimizer+fanout` / `optimizer-shared` (**Plan 7 Task 14**), each transcribed *and* called for real, and — from **Plan 7 Task 15** — mode `full`, `RoutingPipeline.createForHeadless(job).run()` against `run_pipeline`, driven directly rather than transcribed (`run_pipeline` is short and delegates to the other two, both already pinned end to end). `optimizer-shared` is the production stop-flag shape, where quirk #227 makes every item reject | `./scripts/differential/run.sh p7t9 <dsn> [maxPasses] [mode] [optPasses\|all] [optItems\|all]` |
| `probes/P7T15bProbe.java` | `HeadlessBoardManager`'s three clearance overrides — `applyCopperToEdgeClearanceOverride` (:466-552), `applyHoleClearanceOverride` (:346-396), `assignHoleKeepoutClearanceClass` (:404-464) — over **all sixteen** corpus boards × **nine** settings variants, through the real `loadFromSpecctraDsn` (Plan 7 Task 15b). Not a differential driver: no Rust twin, `run.sh` does not know it, the `P7T2Probe`/`P7T4Probe`/`P7T9Probe` pattern. Transcript committed as `tests/data/p7t15b-clearance-overrides.txt` (1 717 lines), replayed by `tests/clearance_override.rs` | committed transcript; regeneration command in the probe header |

**Regenerating the references.** `scripts/gen-router-reference.sh` writes
`tests/reference/<stem>/{router.jsonl,router.meta.txt,java.log}` from the table in
`tests/reference/router-fixtures.txt`; `--verify-hash-modes` re-runs every stem
under `-XX:hashCode=0,1,2,3,4` and requires five byte-identical files.
`cargo test -p fr-router --test reference_parity` is what checks the port against
them; the DAC2020 stems are `#[cfg_attr(debug_assertions, ignore)]`, so use
`--release` for the whole ladder.

## The drill package (Task 7)

`src/autoroute/drill/` is where the maze search gets its layer changes. A
`DrillPage` takes its rectangle, cuts every obstacle out of it, splits the
remainder into convex pieces and puts an `ExpansionDrill` at each piece's centre
of gravity; the drill then binds one expansion room per layer and is dropped
unless every layer resolves to exactly one. `DrillPageArray` is the index — the
board's bounding box tiled into pages of at most
`max(5 * defaultViaDiameter, 10000)`, the number `AutorouteEngine`'s
constructor computes at `AutorouteEngine.java:89-90`.

Four places the port's shape differs from Java's, each forced:

1. **The drills live in `ExpansionRoomStore::drills`, not on the page.** A drill
   is an `ExpandableObject`, so the maze search stores one in a
   `MazeSearchElement.backtrackDoor` — which is a `DrillId` here (ruling 16).
   The page keeps `Option<Vec<DrillId>>`, and the `Option` is Java's `null`: an
   empty list is a *memoised answer*, not "not calculated".
1b. **A page frees its own drill ids** on `invalidate` and on the recompute path — the two
   places Java replaces the list and lets the collector take it. `invalidateDrillPages` fires
   once per changed item, so leaving the slots would grow the arena for the whole run;
   `DrillPage::invalidate`'s docs carry the proof that no live holder is left dangling (the maze
   search cannot invalidate a page, and after it the only holder,
   `FoundConnectionLocator.backtrackArray`, is never read again). `ExpansionRoomStore::clear`
   still leaves the arena alone, because `AutorouteEngine.clear` leaves `drillPageArray` alone.

2. **A page is addressed by a flat `PageId`**, `j * columnCount + i`, where Java
   uses the object reference. The grid is built once and never resized, so the
   index is the identity.
3. **`AutorouteEngine::drill_page_drills` is a borrow bridge.**
   `DrillPage.getDrills(AutorouteEngine, boolean)` is a method on an object the
   engine owns that takes the engine. The bridge moves the page grid out of the
   array, runs `getDrills` and puts it back — on the unwind too, because
   `get_drills` panics where Java throws (quirk #168) and an engine left with an
   empty grid would fail its *next* `overlappingPages` instead.
4. **`ExpansionRoomStore` learned Java's null-ness for `incompleteExpansionRooms`.**
   `new_incomplete_room` is `addIncompleteExpansionRoom` and creates the list;
   `new_unlisted_incomplete_room` is the bare constructor
   `ExpansionDrill.calculateExpansionRooms:76-77` calls and does not. Without
   that distinction the port would build drills where Java builds none — see
   quirk #169.

Two transcriptions that look like tidying opportunities and are not: the
`ceil` chain of `DrillPageArray.java:37-41` recomputes `pageWidth` from
`columnCount` rather than reusing `maxPageWidth`, and `overlappingPages`'
loops compare an `int` counter against a **`double`** bound (`:81-88`).
`overlapping_pages_uses_javas_mixed_loop_bounds`
(`crates/fr-router/tests/drill.rs`) is the test that fails if the second is
"cleaned up" to an `int`.

`DrillPage::obstacle_cutout_trace` is the one added API with no Java
counterpart. It is a **view** of the cut-out loop `get_drills` runs, not a copy,
and it exists because the `prevObstacleShape` carry (`:87`) is otherwise
unobservable: cutting the same hole out of a `PolylineArea` twice is
idempotent, so dropping the carry changes no drill count on any board tried —
only the per-entry trace shows it.

## The maze's four leaf types (Task 8)

`AutorouteControl` is the settings block every method of the package reads. It
**copies** out of `RouterSettings` and holds no reference (plan-6 ruling 8):
Java's `public final RouterSettings settings` has exactly two readers in the
whole of `autoroute/{maze,expansion,drill,path}` — `MazeSearchEngine.java:96-97`
and `:111-112`, the fanout escape-length window — so the port carries those two
numbers as `fanout_max_escape_length` / `fanout_min_escape_length`. Both are
**already multiplied by 1000.0**; Java's own fall-backs (`3000.0`, `500.0`) are
in the scaled unit while the settings fields are millimetres, and the port
reproduces the arithmetic rather than tidying it, which is why the field names
drop the `_mm` the task brief used.

`DestinationDistance` is pure: three bounding boxes, a cost model derived once,
and 250 lines of one-to-four-layer path enumeration. `calculateCheapDistance`
(hazard J) mutates `minNormalViaCost` and restores it in Java; the port threads
the cost through as a parameter, so the method takes `&self`. It has **no caller
anywhere in the Java tree** — it is ported because the audit demands every
`public` member.

`MazeListElement` is **not** an `impl Ord`, and that is deliberate.
`compareTo`'s third key is `door.getId()`, a virtual call whose answer this port
has to look up in the engine's arenas — and, for a `DrillPage`, an answer that
*moves* while the element sits in the queue (quirk #167). So `compare_to` takes
a resolver, `JavaTreeSet::add_by` takes the comparator, and `MazeQueue::push`
closes one over `AutorouteEngine::expandable_id_no`. Snapshotting the id into
the struct would freeze a key Java re-reads on every comparison. The comparator
is not a total order anyway (quirk #170's NaN fall-through), so declaring `Ord`
would be a lie the compiler cannot catch.

`MazeQueue` is a `JavaTreeSet`, not a `BTreeSet` — plan-6 ruling Y/Z. Task 4
measured that the two keep and order **different** elements on a non-total
comparator, and quirk #171 (a four-key tie is silently dropped, payload
included) is reachable with no NaN at all. `JavaTreeSet` therefore grew
`TreeMap.deleteEntry` + `fixAfterDeletion` in this task, because the maze pops
through `iterator().next()` + `it.remove()` (`MazeSearchEngine.java:327-329`)
and red-black deletion changes the tree shape that later comparisons walk.

### Ruling H: the Java half, and where Task 17 took it

`AutorouteControl.rebuildViaInfo` is the consumer the register's re-pointing row
had been waiting for since Plan 3: `:236`, `:243-244`, `:247` and `:260` reach
the `ViaInfo` **through `viaRule.getVia(i)`**, and `ctrl.viaInfos[i].
attachSmdAllowed` is a routing gate at `MazeExpansionEngine.java:339`. Two
probes ran against the **HEAD** jar (2.3.0 is not used anywhere in Plan 6):

* `P6T8Probe viadiv ../freerouting/fixtures/Issue593-BBD_Mars-64.dsn
  crates/fr-router/tests/data/ruling-h-redeclare.rules` — after
  `RulesReader.applyViaInfo` replaces the fixture's
  only via info, `viaInfos.get(name)` says `attach=true` while **both** `default`
  via rules still reach the detached original (`attach=false`, `inList=false`).
  The mechanism survives at HEAD, so the port's index-based rule computes a
  different control block.
* the HEAD jar routing that fixture with `-mp 1 -mt 1 -oit 0`, once plain and
  once with `-dr crates/fr-router/tests/data/ruling-h-redeclare.rules`, emits
  **123** `(via …)` without the `.rules` file and **45** with it, and the two
  `.ses` files differ by 2192 diff lines. The file is nowhere near
  unobservable.

The comparison that actually decides the row — the port routing the same board
with the same `.rules`, against the jar doing the same — is **Task 17's, and it
ran**: see "Ruling H is decided, and it closes against the re-pointing" at the
foot of this file. The port and the jar differed, so `ViaRule` needed owned
`ViaInfo` copies; **Plan 7 Task 0 gave it them** and the same command now MATCHes
on all 50 connections. This fixture stays the regression test, and its transcript
is `crates/fr-router/tests/data/p7t0-ruling-h-match.txt`.


## `RoutingBoardExt` and the check-only shove (Task 9)

Plan-2 ruling 4 left five `RoutingBoard` methods out of `fr-board` because each
one needs an `AutorouteEngine`, which `fr-board` cannot name. Plan-6 ruling 3
puts them here as `RoutingBoardExt`, an extension trait over `fr_board::Board`
that Plan 7 extends further (`optChangedArea` and `ViaOptimizer`; the pull-tight
entry points and the tighteners themselves arrived in Task 15a and the two
forced-trace inserters in Task 15b, both controller ruling AB). The same reasoning brings `board.optimize.TraceShover` and
`board.actions.DrillItemMover` into this crate: both take a `RoutingBoard`, and
the router is their only caller.

**The engine is a value, not a field.** Java's `RoutingBoard` owns
`private transient AutorouteEngine autorouteEngine` (`RoutingBoard.java:70`);
the port cannot, so `init_autoroute` takes `Option<AutorouteEngine>` where Java
reads the field and returns the engine where Java writes it, and
`finish_autoroute` consumes it where Java nulls it. One consequence is recorded
rather than hidden: Java's `additionalUpdateAfterChange` returns at once while
`board.autorouteEngine == null` (`:100`), and only `initAutoroute` (`:892`) ever
sets it, so the port — which has no field to test — always runs the body. That
equals Java on every production path, because `initAutoroute` is the only
non-GUI caller of the `AutorouteEngine` constructor
(`gui/interactive/ExpandTestState.java:167` is the other, and no GUI is ported).

**The check half and the shove half.** `ForcedViaInserter.insert`,
`ForcedPadRouter.forcedPad`, `TraceShover.insert` and
`DrillItemMover.{insert, shoveVias}` are one chain Plan 6 needs, so
**controller ruling AA** put all five in a new **Task 10b** between Tasks 10
and 11, where they now live. Only `TraceShover.springOverObstacles`, which
nothing in Plan 6 reached, was left deferred at that point — and controller
ruling AB then pulled it back into Plan 6 Task 15b, so the class carries no
deferral marker at all today.
That the `check` half stays check-only is pinned by
`trace_shover_check_does_not_mutate_the_board` and
`check_forced_pad_does_not_mutate_the_board`: no `check` changes the board's
item set. They *do* write `shoveFailingObstacle` / `shoveFailingLayer` and burn
item ids on the substitute trace pieces they build, and Java's do too — none of
those is in `Board::structural_hash`.

**The shove half answers `Result<bool, BoardError>`, and only for cancellation.**
Java's four methods answer `boolean`; the port's answer a `Result` because
plan-6 ruling 6 threads a `StopCheck` into them, closing plan-3 ruling F:
`ForcedViaInserter::insert` ends in `Board::insert_via_checked`, which reaches
`split_traces` -> `PolylineTrace.split`, the walk that does not terminate on a
four-rung ladder (quirk #76). Every `Err` is either that cancellation
(`BoardError::Stopped`) or an error `fr-board` already surfaced to its Plan 3
callers; no method here consults the stop check itself, so no run stops at a
point Java's control flow does not reach. The two `catch (Exception e)` blocks
Java wraps `PolylineTrace.normalize` in (`ForcedPadRouter.forcedPad:446-450`,
`TraceShover.insert:571-575`) swallow every error except `Stopped`, which is not
a Java value at all.

**`springOver` returns Java's reference identity.** `TraceShover.check:382` and
`springOverObstacles:845` both branch on `!=` against the polyline they passed
in, so `SpringOverOutcome::{Unchanged, Changed}` replaces what would otherwise
be a lossy `Option<Polyline>`: a detour can be *equal* to the input without
being *identical* to it, and only the identical case leaves
`maxSpringOverRecursionDepth` unspent.

## `ForcedPadRouter` and `ForcedViaInserter` (Task 10), and the cycle closed

`DrillItemMover.check:86` calls `ForcedPadRouter.checkForcedPad`, and
`checkForcedPad` calls `DrillItemMover.check` back
(`ForcedPadRouter.java:269-278`) and `TraceShover.check` too (`:322-334`). That
is a **cycle**, so one half had to land without the other: Task 9 landed
`TraceShover.check` and `DrillItemMover.check` with the `checkForcedPad` call
site deferred to Task 10 behind a marker that panicked, and Task 10 replaced it
with the real call. `drill_item_mover_check_answers_the_arm_task_nine_left_unimplemented`
in `tests/forced_via.rs` is the test that proves it: probe row
`check viaId=6 delta=(300,0) result=true ignoreSize=1`, which Task 9 could
print from the JVM but not run in Rust.

**`inFrontOfPad` carries a typo (quirk #176).** `ForcedPadRouter.java:78`, the
third disjunct of `case 0`, reads `Math.min(lineA.x + lineA.y, lineB.x +
lineB.x)` — `lineB.x` twice, where every sibling case and both neighbouring
disjuncts read `x + y`. Reproduced with a `// Java bug:` marker; the visible
consequence is that at `fromSide = 0` the answer depends on which end point of
the line is `a`, so the same geometric line reversed gives the opposite answer.
`checkForcedPad` consults `inFrontOfPad` only when `checkOnlyFront` is true,
which is exactly the `DrillItemMover.check` path.

**`ForcedViaInserter.insert` is in Task 10b, with the rest of its chain.**
Task 10's brief asked for `insert` and also declared `ForcedPadRouter.forcedPad`
`// added in Plan 7:`. Those two cannot both hold: `insert`'s per-layer body is
three `forcedPad` calls (`:297`, `:317`, `:333`) before `BasicBoard.insertVia`
(`:348`), and `forcedPad` in turn reaches `DrillItemMover.shoveVias` (`:364`)
and `TraceShover.insert` (`:416`) — which Task 9 had deferred to Plan 7 under
plan-6 ruling 2 (both landed in Task 10b in the end). `task-10-report.md` §2.1 raised it as a
**NEEDS_CONTEXT** with three options, and **controller ruling AA** took option
B: a new **Task 10b** between Tasks 10 and 11 ports all five.

## The via-insertion chain (Task 10b)

`ForcedViaInserter::insert` -> `ForcedPadRouter::forced_pad` ->
`{DrillItemMover::shove_vias, TraceShover::insert}` -> `DrillItemMover::insert`
-> `forced_pad` again. Five methods, one cycle, and the entry point Task 15
calls at `FoundConnectionInserter.java:754`. `TraceShover.springOverObstacles`
(`:827-874`) is the one method of the four classes still deferred, and it stays
Plan 7's because nothing in Plan 6 reaches it.

**They answer `Result<bool, BoardError>` where Java answers `boolean`.** That is
plan-6 ruling 6 closing plan-3 ruling F: `ForcedViaInserter::insert` ends in
`Board::insert_via_checked`, which reaches `split_traces` ->
`PolylineTrace.split`, the walk that does not terminate on a four-rung ladder
(quirk #76). `Board::{insert_via, insert_escape_via, split_traces}` keep their
old signatures as delegating `|| false` wrappers, so every Plan 2–5 caller is
untouched and `p2t11`/`p2t15` stay MATCH. **No method in the chain consults the
stop check itself** — it is threaded only into the `fr-board` walks below it
(`split_trace_checked` under `normalize` and under `split_traces`,
`connection_items_checked` under the tail cleanup), so no run stops at a point
Java's control flow does not reach.

**Java's two `normalize` catches are reproduced, and they disagree with each
other.** `ForcedPadRouter.forcedPad:439-450` computes `optArea` as
`changedArea != null ? getArea(layer) : null` and normalizes; `TraceShover
.insert:571-575` writes `board.changedArea.getArea(layer)` with **no guard**, so
on a board that is not marking its changed area it throws a
`NullPointerException` that its own `catch (Exception e)` swallows — and the
substitute pieces stay un-normalized. That is quirk #177, pinned both ways by
`trace_shover_insert_swallows_the_null_changed_area_npe_and_leaves_the_pieces_unnormalized`.
Every other error at those two sites is dropped exactly as Java drops the
exception; `BoardError::Stopped` is the sole exception, because it is the port's
cancellation signal rather than one of Java's.

**`ShapeTraceEntries` had to learn to outlive `cutoutTraces`.** Java's
`EntryPoint.trace` (`ShapeTraceEntries.java:791`) is a live `PolylineTrace`
reference, and `nextSubstituteTracePiece` reads the trace's polyline through it
— *after* `cutoutTraces` has taken that trace off the board, which is the order
both `forcedPad:405-408` and `TraceShover.insert:511-514` run in. Keying by
`ItemId` alone lost it: the board lookup answered `None` and the detour piece
was silently never built, one item id short of Java. `fr-board`'s
`ShapeTraceEntries` now snapshots each stored trace when it inserts the entry
point, which is where Java takes its reference.

**Board-state parity is asserted item by item.**
`scripts/differential/java/probes/P6T10bProbe.java` rebuilds its board per row
and dumps the whole item list in `getItems()` order plus
`communication.idGenerator.maxGeneratedId()`; `crates/fr-router/tests/forced_via.rs`
rebuilds the same board and compares every line. 1 292 grid rows across four
modes, plus five random blocks of 120 that compare a whole board as one
`String.hashCode`.

## Quirk-register numbering

`docs/java-quirks.md` is allocated **contiguously, in the order rows are
written**. Plan 6's plan text labels its rows `#155`–`#168`, but `#155` was
already taken by Plan 5, so those labels are **not** row ids. Task 2 wrote the
first three Plan 6 rows and they landed as **#156, #157, #158**; Task 3 wrote
**#159** (`ShapeSearchTree90Degree.completeShape` drops a room the base class
and the 45-degree override keep); Task 4 wrote **#160** (the non-transitive
`SortedRoomNeighbour.compareTo` and its `TreeSet`'s silent drop) and **#161**
(the id tie-break subtracting a room id from an item id) and **#162** (an
unterminating `calculateNewIncompleteRooms`); Task 5 wrote **#163**
(`Sorted45DegreeRoomNeighbours.calculateEdgeIncompleteRoomsOfObstacleExpansionRoom`
never advances its `currentCorner`, so it always skips the last side of the
walk); and Task 6 wrote **#164** (`removeCompleteExpansionRoom` binds
`ExpansionDoor`'s narrowing `otherRoom` overload and so skips every incomplete
neighbour), **#165** (`completeExpansionRooms` is a strict subset of the
complete rooms that exist, and the abandoned ones keep their doors) and
**#166** (`completeExpansionRoom`'s `catch` returns a fresh empty collection
for rooms it has already committed); and Task 7 wrote **#167**
(`DrillPage.getId` hashes the `netNumber` that `getDrills` overwrites, so
recomputing a page changes the sort key it is stored under — plan-6 ruling 4's
hazard B), **#168** (a cancelled `splitToConvex` makes `getDrills` throw *and*
leaves the page memoised as having no drills) and **#169**
(`removeIncompleteExpansionRoom` dereferences a lazily created list with no null
guard, which silently costs every drill on an engine that has never had an
incomplete room added); and Task 8 wrote **#170** (`MazeListElement.compareTo`
compares `double`s with raw `<`/`>`, so a `NaN` falls through to the next sort
key — plan label #155), **#171** (a four-key tie answers `0` and `TreeSet.add`
then drops the new element whole, payload included — plan label #156), **#172**
(the two HEAD-only pure-SMD relaxations in `AutorouteControl.rebuildViaInfo`,
which force `attachSmdAllowed` on and scale the via cost by `0.1` — plan label
#159, an id already spent by Task 3) and **#173** (`initNet`'s null-net arm is
only reachable for `netNumber <= 0`; a positive unknown net throws two lines
later); and Task 9 wrote **#174** (`TraceShover.check`'s via arm returns
`false` without setting `shoveFailingObstacle`, so the field keeps a stale item
that the ripup resolver later reads) and **#175** (`DrillItemMover.check`
appends the drill item to the caller's own `ignoreItems` collection — a check
with a visible side effect); and Task 10 wrote **#176**
(`ForcedPadRouter.inFrontOfPad`'s `case 0` reads `lineB.x` twice where every
sibling reads `x + y`, so the same geometric line answers differently depending
on the order of its two defining points); and Task 10b wrote **#177**
(`TraceShover.insert` dereferences `board.changedArea` with no null check and
its own `catch` hides the `NullPointerException`, so the substitute traces are
inserted un-normalized on a board that is not marking its changed area — while
`ForcedPadRouter.forcedPad`, the same loop, guards the identical call); and
Task 11 wrote **#178** (`MazeSearchEngine.init` ignores the `boolean` its own
overridden `add` returns, so a fanout control whose escape window rejects every
seeded door still produces a live engine with an empty queue); and Task 12
wrote **#179** (`MazeSearchEngine.checkNeckDownAtDestPin` never asks whether the
pin is a *destination* pin and returns from inside its loop, so a start pin's
neckdown silently shrinks the trace the search plans through a room it is only
passing through). The next free id is **#180**. Every later
task must re-read the
register's last row rather than trust the plan's labels — the plan carries an
amendment saying so.

## `SortedRoomNeighbours` (Task 4), and why the crate has a `JavaTreeSet`

`crates/fr-router/src/autoroute/expansion/sorted_neighbours.rs` ports the
any-angle base class and `selectCalculationMode`; the two angle-restricted
siblings landed in Task 5 as `sorted_neighbours_45.rs` and
`sorted_neighbours_orthogonal.rs`, and `complete` now dispatches to all three. Java's entry points take an `AutorouteEngine`; the port takes apart
the five services they read off it (net number, tree id,
`generateRoomIdNo`, `removeAllDoors`, `addIncompleteExpansionRoom`), so Task 6's
engine can call it without any signature here changing.

**Plan-6 ruling 4 says the neighbour set is a `BTreeSet`. It could not be, and
Plan 9 Task 8 is what makes it possible again.** `SortedRoomNeighbour.compareTo`
was not a total order, and on such a comparator `std`'s `BTreeSet` and Java's
`TreeSet` keep *different* elements and iterate the survivors in *different*
orders — both measured, against the HEAD jar, by
`scripts/differential/run.sh p6t3 3`. The container is therefore
`crates/fr-router/src/java_tree_set.rs`'s `JavaTreeSet`, a transcription of
`java.util.TreeMap`'s red-black `put`, `fixAfterInsertion` and in-order
traversal. Quirk row #160 records the measurement.

**Task 8 (#160 + #161) made the comparator a total order** — see that row — so
the two containers now keep the same elements in the same order, which
`the_neighbour_comparator_is_a_total_order` asserts over 2 000 generated cases
in both. The `JavaTreeSet` **stays** here until Task 24 collects the swap; the
obstacle to it is gone, not the container. **Task 8's `MazeListElement` queue
answers the same question in the same commit series** (quirk #171).

`scripts/differential/run.sh p6t3` covers the class in ten modes: `0`/`4` are
`calculateNeighbours` over a random board (`4` snaps the obstacles to a grid,
which is the only way the **dimension-0** corner-touch branch is ever reached);
`5` is the whole of `complete` against a real Java `AutorouteEngine`; `1`/`2`/`3`
are the comparator probes. Mode 5 also found **quirk #162**: an unterminating
`calculateNewIncompleteRooms` — `:512` indexes `fromRoom.getShape().toSimplex()`
with side numbers computed against the *un-simplified* shape, and when
`toSimplex()` dropped the line `firstTouchingSideNo` names, the `for (;;)` at
`:562` allocates rooms for ever. It is reproduced, not guarded; the driver skips
those calls on both sides. Modes `6`-`9` are Task 5's, below.

## The two angle-restricted sorters (Task 5)

`Sorted45DegreeRoomNeighbours` and `SortedOrthogonalRoomNeighbours` are **not**
specialisations of the base class and share no code with it beyond the
package-private static `insertDoorOk`. Each declares its own inner
`SortedRoomNeighbour` with its own fields and its own `compareTo`, so the port
has three separate transcriptions in three files
(`sorted_neighbours{,_45,_orthogonal}.rs`) and three unrelated `Ord`s. Both
subclass comparators *are* total orders — five Java `int` keys, every refinement
entered on the same condition for both operands — unlike the base class's
(quirk #160); they still drop a tie, because the id tie-break crosses two id
spaces (quirk #161), so both sets are `JavaTreeSet`s too.

What differs from the base class and reaches the geometry:

* a 2-dimensional overlap is skipped **only for an obstacle room**
  (`Sorted45DegreeRoomNeighbours.java:132`,
  `SortedOrthogonalRoomNeighbours.java:168`); the base class skips every one;
* the door built for a touching neighbour comes from the **two-argument** `ExpansionDoor`
  constructor, which *computes* its dimension from the two rooms' shapes
  (`:164` / `:201`, `ExpansionDoor.java:35-39`); only the base class hard-codes `1`
  (`SortedRoomNeighbours.java:281`). The two differences compound: the computed
  `dimension == 2` door is exactly what `tryRemoveEdge` scans for when it picks
  `completeShape`'s `ignoreObject`, so hard-coding `1` would silently disable the
  room-enlargement path with no differential diff to show for it;
* target doors are built by `CompleteFreeSpaceExpansionRoom.calculateTargetDoors`
  **inside** the neighbour loop, one entry at a time and with an unconditional
  `setNetDependent()`, where the base class defers the own-net objects to a list
  and calls its own static namesake once at the end;
* the 45-degree `tryRemoveEdgeLine` removes **every** untouched border line at
  once and passes `completeShape` the largest 2-dimensional door to a free-space
  room as the object to ignore; the orthogonal one removes the first untouched
  line; the base class removes one line and ignores nothing;
* an obstacle room with no neighbours at all gets one incomplete room per side
  of the board's bounding box (orthogonal, no guards at all) or one per side of
  its own octagon minus one (45-degree — **quirk #163**).

`p6t3` modes `6`/`7` are mode 4 for the two regimes (the board is built with the
matching `AngleRestriction`, so `getAutorouteTree` answers the matching tree
subclass and `selectCalculationMode` the matching sorter) and modes `8`/`9` are
mode 5 for them — the whole of `complete` against a real Java `AutorouteEngine`.
Quirk #162's skip applies to mode 5 only: neither subclass walks a `Simplex`.
Modes `6`/`7` end with an `overlap` probe — a free-space room inserted so that it
overlaps the room under test 2-dimensionally — because the random loop cannot
reach the `&&` arm or the computed door dimension above: its seed rooms come from
`completeShape`, which restrains them against everything already in the tree.

## The `fr-board` obligation Task 4 discharges

`ShapeSearchTree::tree_shape_of` and `ignore_object` used to panic on a
`TreeObject::Room`. They now take a `RoomLookup` — the room counterpart of
`ItemLookup`, with `room_tree_shape` and `room_shape_layer`, implemented in this
crate by `ExpansionRoomStore`. The queries gained `*_with_rooms` twins
(`overlapping_tree_entries_with_rooms`, `overlapping_objects_with_rooms`); the
original signatures are delegating wrappers that pass `NoRooms`, so every Plan
2-5 caller and test is untouched and a board-level caller that reaches a room
leaf still panics, deliberately.

## The maze search's frame (Task 11)

`src/autoroute/maze/search.rs` is `MazeSearchEngine`'s *frame*: the struct, the
constructor, `getInstance`, `init`, `findConnection`, `occupyNextElement`,
`doorIsSmall`, `reduceTraceShapesAtTiePins`, `segmentProjection`,
`toImpactedPoints` and the two nested result types. Four things about it are
decisions rather than transcription.

**The engine is a `&mut` field and the board is a parameter.** Java reaches the
board through `autorouteEngine.board`; the port cannot, because
`AutorouteEngine` borrows the board per call. Keeping the engine as a field and
the board as a parameter is what makes `self.queue.push(e, self.ctrl,
self.engine, board)` type-check — three disjoint borrows of `self` and one of
`board` — which is exactly the arrangement `task-8-report.md` §8.1 said Tasks
11-13 had to preserve. Nothing caches a door id.

**Four private Java members are `pub` here.** `init`, `doorIsSmall`,
`reduce_trace_shapes_at_tie_pins` and `segment_projection` are `private` /
`private static` in Java, and Java's own ground-truth probe reaches all four
with `setAccessible(true)`. Rust integration tests have no reflection, and
plan-6 ruling 6 demands a test per cancellation site — but all four of `init`'s
sites collapse into `getInstance`'s single `None`, so an integration test that
could only call `getInstance` could not tell `:975` from `:1051`.

**`Set<Item>` is `BTreeSet<ItemId>`, walked backwards.** Every production caller
passes a `TreeSet<Item>` (`Item.getConnectedSet` / `getUnconnectedSet`), and
`Item.compareTo` is `other.id - this.id` — **descending** id. `init`'s two item
loops and `reduceTraceShapesAtTiePins`' two loops (the second over
`getNormalContacts()`, itself a `TreeSet`) therefore iterate `.rev()`. This is
observable: `init_creates_the_start_rooms_in_javas_descending_item_order` aborts
`init` on its fourth stop call and reads `incompleteExpansionRooms` in list
order, where the through pin's two rooms precede the SMD pin's. It is **not**
observable in the completed rooms of the same board — both orders answer the
same five rooms with the same ids, because both pin centres fall in the same
`completeShape` partition — which is why the test reads the incomplete list
instead.

**The three expanders panicked until Task 13.** `expandToDrillsOfPage` and
`expandToOtherLayers` (Task 13) and `expandToRoomDoors` (Task 12) were
`unimplemented!` stubs carrying `added in Task 12:` / `added in Task 13:`
markers with the Java method name on the same line. A stub that answered `true`
— "nothing expanded", the harmless value — would have made the pop loop look
healthy while routing nothing, and the first thing that would have noticed is
Task 17's fixture parity. Every Task 11 test is still built so the pop loop
terminates on a destination door or an occupied section; the three are now real
`fn`s (the two drill ones on `MazeExpansionEngine`, Java's own split).

### The fixture, and a JVM finding that is not a quirk

`tests/maze_search.rs` uses `P6T7Probe`'s two-pin board with **the two traces
replaced by one obstacle box**. The traces had to go, and the reason is worth
recording because it will bite any later task that hand-builds a routing
fixture: a pin's `getTraceConnectionShape` is a **bare point**
(`DrillItem.java:359-361`), so the start room only exists if that point is
outside every foreign obstacle's *compensated* tree shape. On the original board
the net-2 trace carries the "wide" clearance class, its compensated shape
reaches from x = -1220 to x = -380, and the start pin sits at (-500, 0) —
inside it. `ShapeSearchTree.completeShape` then answers **zero** candidate
rooms, `init` answers `false`, and `getInstance` answers `null` for a reason
that has nothing to do with the method under test. JVM-verified on the HEAD jar
before the fixture was changed.

## The room-door expansion and the cost model (Task 12)

`src/autoroute/maze/expand.rs` is `MazeSearchEngine`'s *body* — the seven
private methods one pop of the queue runs — and
`src/autoroute/maze/trace_shover.rs` is `MazeTraceShover`, the check-only shove
probe two of them consult. Together they close `autoroute/maze`'s last audit
row.

**The cost model has exactly one producer.** `expandToDoorSection` (`:791-965`)
is the only place outside `init` that builds a `MazeListElement`, so every
number in the A\* frontier is made there: a bend penalty of
`ctrl.bendCosts[layer]` charged when `crossProduct² > 0.01 · |prev|² · |next|²`
(a **normalised** test — `sin² > 0.01`, about 5.7°, and scale-independent);
`expansionValue = from.expansionValue + addCosts + bend + weightedDistance(...)`
under the layer's horizontal/vertical trace costs; `sortingValue` = that plus
`destinationDistance.calculate`; and `roomRipped` / `ripupCost` set only by a
positive `addCosts` with `Adjustment.NONE`, or `roomRipped` alone inherited from
an already-checked ripped parent. `the_bend_penalty_fires_exactly_above_sin_squared_one_percent`
straddles the threshold by **one unit** — `dy = 100` is below it and `dy = 101`
above, because with `dx = 1000` the test reduces to `99·dy² > 1000²`.

**The door snapshot is taken *after* completing the neighbours, not before.**
The task brief asked for a test named "the door snapshot is taken before
completing neighbours"; Java's `new LinkedList<>(nextRoom.getDoors())` is at
`:559` and `completeNeighbourRooms` at `:419`, so the snapshot guards the
*iteration*, not the completion. It is observable on this board: completing room
2's neighbours removes one door and adds another, and the round then expands
through the post-completion list (66, 67, 33) — door 67 did not exist when the
pop began. Java wins; the test is named for what Java does.

**`MazeTraceShover` never writes the board.** Despite the name it calls only
`RoutingBoard.checkTraceSegment` and the static `TraceShover.check`, and then
*collects* the door sections a successful shove would open. Every one of its
tests asserts the board's item count before and after. The collector itself
(`MazeTraceShover.java:236-312`) has **no ground truth**: on every board the
probe reaches, `TraceShover.check` refuses first and `:213-215` returns before
the loop, so nothing in the crate has ever produced a `MazeAdjustment::Left` or
`Right`. That is carried as an `obligation:` marker at the site naming **Task
17**, whose acceptance corpus must include a board on which a shove genuinely
succeeds with a candidate door on the shove side.

**Hazard N is two guards, not one.** The brief names `MazeTraceShover:64-66` as
"the silent `continue`"; it is a `return false`, and the genuine silent
`continue`s are `expandToTargetDoors`' pair at `:656-668`. Both are ported
verbatim and both are pinned — the first by shortening a trace's polyline
underneath a room that keeps its `indexInItem`, the second by forcing every
`treeEntryNo` past the item's current tree-shape count.

**All seven private Java methods are `pub` here**, for the reason the frame's
four are: Java's ground-truth probe reaches them with `setAccessible(true)`, and
the branches that matter — the layer-active gate, the small-door refusal, the
bend threshold, the two stale-index skips — are not separable through
`occupyNextElement` alone.

**The four Task 13 markers inside `expandToRoomDoors` are closed.**
`MazeRipupResolver.checkRipup` and `.checkLeavingRippedItem` (`:506`, `:519`)
and `MazeExpansionEngine.expandToDrillPage` / `.expandToDrill` (`:611`, `:620`,
the latter through `Via.getAutorouteDrillInfo`) now dispatch to the two Task 13
classes. Their branches are still guarded by `ctrl.ripupAllowed`,
`currentDoorIsSmall` and `ctrl.viasAllowed`, so a control with vias and ripup off
runs the whole file — which is what `tests/maze_expand.rs` and `P6T12Probe` do.

## The drill/layer expansion, the ripup cost model and `Connection` (Task 13)

`src/autoroute/maze/expansion_engine.rs` is `MazeExpansionEngine`,
`src/autoroute/maze/ripup_resolver.rs` is `MazeRipupResolver`, and
`src/autoroute/path/connection.rs` is `Connection`. Both `maze` classes are
**unit structs** whose associated functions take the `MazeSearchEngine` as their
leading parameter, because Java's single `private final MazeSearchEngine search`
field would otherwise have to be a second `&mut` borrow of an engine the search
already holds. The ground truth is
`scripts/differential/java/probes/P6T13Probe.java`, fourteen modes, transcript
`tests/data/p6t13-drills-ripup.txt`.

**Java wins over plan-6 ruling 8: `AutorouteControl.settings` has three readers,
not two.** `MazeRipupResolver.java:99` reads
`ctrl.settings.getStartRipupCosts()` to decide whether the pass is still early
enough to protect fanout vias. It is copied into
`AutorouteControl::start_ripup_costs` exactly as the two fanout escape lengths
are.

**`Via.getAutorouteDrillInfo` moved to `fr-router` and its field to
`AutorouteInfo`.** Java hangs `autorouteDrillInfo` off `Via` itself; the method
builds an `autoroute.drill.ExpansionDrill` and fills it from
`ItemAutorouteInfo.getExpansionRoom`, neither of which `fr-board` can name
(ruling 15). The **field** is `fr_board::AutorouteInfo::autoroute_drill_info`,
which is exactly equivalent: the only two writers of
`Via.autorouteDrillInfo = null` (`Via.clearDerivedData` `:223` and
`Via.clearAutorouteInfo` `:229`) are also the only two writers of
`Item.autorouteInfo = null` (`Item.java:1053`, `:1064`), and each calls its
`super` first. `fr-board`'s `Via::clear_autoroute_drill_info` carries the
`renamed:` marker; `DrillId` moved to `fr_board::ids` beside `ObstacleRoomId`
and `ConnectionId` and is re-exported from `crate::arena`.

**The memo is load-bearing, not an optimisation.** The via's `ExpansionDrill`
carries a `MazeSearchElement` per layer and `occupyNextElement` writes
`isOccupied` through it; a second call that built a fresh drill would let the
search expand the same via for ever.

**The ripup price, transcribed.** `costFactor` is the obstacle trace's half
width, or for a via the largest half width among its trace contacts times
`0.5 · (contactCount − 1)` — so a via with **one** contact is free and costs the
`max(…, 1)` floor whatever `ctrl.ripupCosts` is (JVM-pinned at 1 for
`ripupCosts` 1000, 100 000 and 2 000 000 000). `ripupCost = ripupCosts ·
costFactor / detour · fanoutViaCostFactor`, clamped into
`[1, Integer.MAX_VALUE / 100]`; `detour` comes from `Connection.getDetour()`
only when the obstacle is not already fanout-protected and the pass is not a
fanout pass.

**Plan 6's only randomness is pinned.** `checkRipup:158-163` draws
`search.randomGenerator.nextDouble()` when `ripupPassNo >= 4 && ripupPassNo % 3
!= 0` and scales the detour by `0.5 + r²`. `tests/ripup.rs`'
`pass_four_randomises_and_pass_six_does_not` runs passes 3, 4, 5, 6 and 7 **in
order on one engine** (29431 / 29303 / 35440 / 29431 / 21087), because the three
draws are consecutive values of the one generator seeded with `ctrl.ripupCosts`;
`the_random_draw_matches_the_jvm` pins the raw `JavaRandom` sequence for seeds
1000, 5000 and 17 against `jshell`.

**`Connection`'s `itemList` is ascending here and descending in Java.** Java's
is a `TreeSet<Item>` (`Item.compareTo` = `other.id - this.id`); a
`BTreeSet<ItemId>` is ascending. Nothing reads the order — the two consumers are
`size()` and the `traceLength()` sum, and the memo loop writes the same value
into every member.

**The attach-SMD half is pinned by a mode-local flag, not a second board.**
`ForcedPadRouter.checkForcedPad:281-287` answers `DRILLABLE_WITH_ATTACH_SMD` only
when copper sharing is allowed *and* one of the same-net obstacles is a `Pin`,
and `ForcedViaInserter.checkLayer:82` passes the `ViaInfo`'s own flag — so
flipping the rule's one `ViaInfo` to attach-on (probe mode `attachsmd`) is enough
to reach `checkLayerWithAnyMatchingVia:407-412`'s remember-and-answer path,
`expandToOtherLayers:276-282`'s `smdAttachedOnComponentSide` write and **both
halves** of `maskOk` (`:336-339`), all pinned by
`an_attach_smd_via_promotes_the_layer_and_the_via_mask_then_decides_the_span`.
Three arms are left without ground truth and carry `obligation:` markers naming
**Task 17**: `checkRipup`'s `roomWasShoved` branch (`:86-91`, which needs the
`MazeAdjustment::Left`/`Right` Task 12's own marker already owns), its
`ALREADY_RIPPED_COSTS` arm (`:92-94`) and the two `smdAttachedOnSolderSide`
writes (`:279-281`, `:306-308`, which need a bottom-side SMD pad).

**`findConnection` runs end to end.** `tests/maze_drills.rs`'
`find_connection_reaches_the_destination_door_in_seven_pops` pins the whole pop
sequence on a 2000-unit two-pin board: the seeded target door, a `DrillPage`,
two `ExpansionDrill`s, an `ExpansionDoor`, a second target door and then the
destination door (id 97), each with its expansion and sorting value and the
queue size after the pop; `find_connection_answers_the_result_the_pop_loop_leaves_behind`
pins the `MazeResult` and the three elements left in the queue.

## The found-connection locators (Task 14)

`src/autoroute/path/locator.rs` is `FoundConnectionLocator`,
`src/autoroute/path/locator_45.rs` is `FoundConnectionLocator45Degree` and
`src/autoroute/path/locator_any_angle.rs` is `FoundConnectionLocatorAnyAngle`.
The ground truth is `scripts/differential/java/probes/P6T14Probe.java`, nine
modes, transcript `tests/data/p6t14-locator.txt`. It declares
`package app.freerouting.autoroute.path` so it can call the package-private
`calculateAdditionalCorner` and read the `protected` `backtrackArray` and
`ResultItem`, and it reaches `P6T13Probe`'s boards by reflection rather than
declaring a fourth fixture.

**Two implementations, three regimes.** `getInstance` (`:196-205`) builds
`FoundConnectionLocator45Degree` for **both** `NINETY_DEGREE` and
`FORTYFIVE_DEGREE`, and `FoundConnectionLocatorAnyAngle` for everything else.
The two 90°/45° runs then diverge inside `calculateAdditionalCorner`
(`:390-404`), which switches on the angle-restriction **value**, not the class.
So the port is one `LocatorWalk` with a `LocatorKind` discriminant, and
`the_three_regimes_locate_three_different_corner_lists` runs one and the same
maze result through all three: `(400,0) (-132,0) (-132,-132) (-132,0) (-400,0)`,
`(400,0) (-132,0) (-132,-132) (-264,0) (-400,0)` and `(400,0) (-400,0)`.

**Java wins over the brief: `connectionItems` holds traces, and it is never
null.** The brief models an entry as a `ConnectionItem::Trace | ::Via` enum
whose `None` is "Java's SKIPPED signal". Java's `ResultItem` (`:542-551`) is a
corner list plus a layer, full stop; every via is `FoundConnectionInserter`'s to
derive from the layer change between two consecutive entries (`:66`) plus one
final via back to `startLayer` (`:74`). And the field is assigned an empty
`LinkedList` at `:101`, before both of the constructor's early returns, so the
`connectionItems == null` test that produces `SKIPPED`
(`AutorouteEngine.java:230-235`) is dead code — quirk #180, which Task 15 must
not resurrect. The port's field is a `Vec<ResultItem>`.

**Reference identity is load-bearing.** `calculateNextTrace:432` drops a corner
with `currentNextCorner != prevCorner`, Java's **reference** test.
`FoundConnectionLocatorAnyAngle` hands back `this.currentFromPoint` itself at
`:101` and `:184` (the "door completely passed" index advance) and
`right/leftTurnNextCorner` return their `fromCorner` argument on a null
tangential point, so a value comparison would keep corners Java drops. Every
`FloatPoint` in the walk therefore carries a `u64` identity token.

**The 90° regime emits spikes, and they are Java's.** The rounding loop
(`:450-459`) drops only *consecutive* duplicates, so the 90° single-room list
visits `(-132,0)` twice with `(-132,-132)` in between and the layer-change list
visits `(-130,0)` twice around `(-130,132)`. Both are pinned as literals.

**Five fixtures, and what each one reaches.** `simple_board` gives the
single-room case; `probe_board` searched pin 2 → pin 3 with vias gives the
layer change (three `ResultItem`s on layers 0, 1, 0 and **no** via entry);
the same board with `viasAllowed` off gives the eight-door walk round the
blocker (26 / 25 / 7 corners); `blocked_board` forces a ripup, so
`backtrack:316-323` fills `rippedItemList` with item 4 at cost 1 and an
`ObstacleExpansionRoom` appears in the backtrack array; and `probe_board`
searched **pin 3 → pin 2** is the only fixture that bends far enough left to
reach `FoundConnectionLocatorAnyAngle.leftTurnNextCorner` (`:391-408`).

**Every null in these files is either Java's own value or Java's crash — never
a third answer.** `doorLeftCorner`/`doorRightCorner` really do become `null`
mid-method (`FoundConnectionLocatorAnyAngle.java:92`, `:96`) and stay
`Option<FloatPoint>`. Everywhere else a null is an NPE that
`AutorouteEngine.autorouteConnection:189-195` catches into **`FAILED` for the
whole connection**, so the port panics rather than degrading: a locator that
quietly answered `None` and carried on would route a wire Java refuses to
route. That covers `calcDoorLeft/RightCorner`'s `fromRoom` (`:45`, `:57`),
`FloatPoint.sideOf`'s first dereference (`:87`, `:165`),
`FloatLine.segmentDistance` in the correction loop (`:293`, `:308`), the null
`diagonalCornerSegment` of `FoundConnectionLocator45Degree:59`/`:314`, and the
latent `:287` `FloatLine` of quirk #181. Ruling 7's `catch_unwind` around
`get_instance` is what turns each panic back into Java's `FAILED`. There is no
`// totalized:` site in these three files, because no degraded value here
matches Java.

**Both constructor warn branches are pinned.** Probe mode `warn` forges a
`MazeSearchEngine.Result` over the **live** rooms of a completed search — the
`ExpansionDoor` at `backtrackArray[1]` for `:130-135`, and a door section whose
`backtrackDoor` is null for `:103-111` — and prints
`connectionItems=n=0` for both, which is quirk #180's direct evidence.

**Two `obligation:` markers name Task 17**: the fanout arm (`:124-129` +
`:142-144`), which needs `ctrl.isFanout` and so no Plan 6 fixture reaches, and
the conduction-area shrink (`:167-175`), which needs a start item whose
trace-connection shape is 2-dimensional.


## The pull-tight family (Task 15a, controller ruling AB)

`board/optimize/TraceTightener.java` (547 lines) and its three regime
subclasses — `TraceTightener90` (169), `TraceTightener45` (674),
`TraceTightenerAnyAngle` (1004) — plus `PolylineTrace.pullTight`'s two overloads
(`board/trace/PolylineTrace.java:809-863` and `:869-890`) live in
`src/board_ext/tightener/`. Ruling 2 had put all of them in Plan 7. **Task 15
found that unworkable** (`task-15-report.md`): Java's trace insertion
pull-tightens *every* inserted polyline unconditionally —
`FoundConnectionInserter.java:185` passes `tidyWidth = Integer.MAX_VALUE`, so
`RoutingBoard.insertForcedTracePolyline:860`'s `tidyWidth > 0` holds;
`optNetNoArr` is empty, so `PolylineTrace.pullTight:821`'s net filter never
fires; and `NetClass.pullTight` defaults to `true` — so plan-6 ruling 1(b)'s
"same inserted item geometry" cannot be met without it. **Controller ruling AB**
moved exactly those five names into Plan 6. `ViaOptimizer`, `optChangedArea`'s
batch callers and `removeItemsAndPullTight` stay Plan 7's.

**Reference identity is the algorithm.** All three `pullTight` overrides loop
`while (newResult != prevResult)` — *object* identity — and
`PolylineTrace.pullTight:837` reads `newLines != lines` the same way to decide
whether the trace changed at all. Every step in this module therefore answers
`Option<Polyline>`, where `None` is Java's "returned the argument object": the
`Option`'s discriminant **is** Java's `!=`. Several steps rebuild a polyline
that happens to be value-equal to their input, so a value comparison would loop
where Java stops (and stop where Java loops).

**Quirk #34 is discharged here.** Java's four `Line.equals` call sites are
`Simplex.borderLineIndex` (done in Plan 1) and three tightener sites —
`TraceTightener.repositionLine:281`, `TraceTightenerAnyAngle.repositionLine:568`
and `:576` — all of which now use `Line::equals_geometric`. Each compares a
*translated* line with its original to detect a sub-unit translation that did
not move it, which is exactly where the geometric and the structural test
disagree. `tests/tightener.rs`'s `lineeq` rows (`onLine`, `subUnit`, `halfInt`)
pin all three: mutating any of them to `==` makes the test fail.

**Quirk #183 makes a whole branch dead.**
`TraceTightenerAnyAngle.smoothenEndCornerAtTrace:907-908` reads
`prevLineDirection` from the same line as `lineDirection`, and the `bend` arm
needs one projection `ZERO` and the other `POSITIVE` — impossible for two equal
directions. So `:988-1001` never runs. The `smooth` fixture carries the one
geometry that separates the two indices, so the quirk cannot be lost silently.

**The `additionalUpdateAfterChange` seam, closed in Task 15b.**
`PolylineTrace.change` calls `board.additionalUpdateAfterChange(this)`
(`PolylineTrace.java:944`), which needs an `AutorouteEngine`; Task 15a's entry
points took none, so `pull_tight_with` did not make that call. Task 15b added
`PolylineTraceExt::pull_tight_with_engine`, which takes
`Option<&mut AutorouteEngine>` — Java's nullable `RoutingBoard.autorouteEngine`
field, which `additionalUpdateAfterChange:100` tests before doing anything — and
makes the call, guarded by Java's own `isOnTheBoard()` test, immediately before
`Board::change_trace`. `pull_tight_with` is the `None` wrapper, so Task 15a's
callers and tests are unchanged. The call touches the engine's room/drill
database and never the board's item list, so no probe row on either side can see
it; it is threaded because `insertForcedTracePolyline` runs inside a live
`autorouteConnection`, not because a fixture catches it.
`PolylineTrace.{check,correct,swap}ConnectionToPin` stayed deferred to Plan 7 in
`crates/fr-board/src/items/trace.rs` through the whole of Plan 6: ruling AB named none of the three, and
`FoundConnectionInserter.insertTrace:140-141` sets `pinEdgeToTurnDist` to `-1`
for the whole insertion, so no Plan-6 path reached the branch at
`PolylineTrace.pullTight:841-861` that calls them. **Plan 7 Task 5 landed all
three** and turned the markers into `renamed:` ones — see
"[`optChangedArea` and the `ConnectionToPin` trio](#optchangedarea-and-the-connectiontopin-trio-plan-7-task-5)"
below. Probe mode `pinedge` recorded the JVM's answers on the Plan 6 fixture:
**8 `changed=true` rows and 4 `changed=false`**, and none of the eight is this
branch — they are the net-1 trace tightening through the ordinary
`newLines != lines` path at `:837`, which returns before `:841`. Only the four
net-2 rows reach the branch, and all four answer `false`. That is *not* because
the branch is inert, but because `P6T9Probe`'s pad is a **square** 100 × 100 on a
two-pin package, which `Pin.java:274-276` gives `padXyFactor = 3.0` and
`Padstack.getTraceExitDirections:182-193` therefore answers all four directions
for; with every direction matched `checkConnectionToPin` can still refuse on
length (`:1074-1076`), but nothing on that fixture ever does. `p7t6`'s fixture
uses a 400 × 100 pad on a four-pin package for exactly that reason, and its
`correct`/`swap` sections carry 102 `changed=true` rows.

**Where the numbers come from.** `scripts/differential/java/probes/P6T15aProbe.java`
(compiled with `P6T9Probe.java`, whose board it reuses) and its committed stdout
`tests/data/p6t15a-tightener.txt` — eleven modes, including one fixed
eleven-polyline table per regime and three 256-row `java.util.Random(4242)`
blocks the Rust side replays with `JavaRandom`. Every row is compared, not
sampled.

## The forced-trace inserters and `springOverObstacles` (Task 15b, controller ruling AB)

The same ruling that moved the tighteners moved three more names into Plan 6,
and Task 15b ported them:

| Java | lines | Rust |
|---|---|---|
| `board/facade/RoutingBoard.insertForcedTracePolyline` | `:456-876` | `RoutingBoardExt::insert_forced_trace_polyline` |
| `board/facade/RoutingBoard.insertForcedTraceSegment` | `:361-402` | `RoutingBoardExt::insert_forced_trace_segment` |
| `board/optimize/TraceShover.springOverObstacles` | `:827-874` | `TraceShover::spring_over_obstacles` |

`FoundConnectionInserter:176` calls the first on every segment of every routed
connection and `tryNeckDown` / `insertFanoutMicroNeckdown` call the second five
times, so neither could stay in Plan 7; `insertForcedTracePolyline:522-524`
calls the third on every polyline it inserts, which is what emptied
`board/optimize/TraceShover.java`'s deferral roster.

**`optChangedArea` is not reached from here.** Ruling AB asked for "only the
branch reached, and a marker on the rest" *if* the insertion tail called it. It
does not: the tail (`:773-875`) builds its own `TraceTightener`, calls
`splitTracesAtKeepPoint` and then a single `PolylineTrace.pullTight`. Java's
`optChangedArea` callers are `forcedVia:348`, `insertTrace:293`, `autoroute:962`
and `fanout:1101` — all Plan 7's — so the one deferral this module still carried
when Plan 6 closed was `TraceTightener.optChangedArea`'s, **consumed by Plan 7
Task 5**. `board_ext/` has no deferral marker left.

**Java's `==` on the returned corner is value equality here, and that is
measured.** `insertForcedTraceSegment:394-400` compares the polyline's answer
against `insertPolyline.firstCorner()` / `lastCorner()` by *reference*, and so
does `FoundConnectionInserter.tryNeckDown:492`. The port compares by value; the
probe prints both columns for all 452 rows of modes `poly`, `seg` and `rand` and
they agree on every one, which the method's doc comment derives from the three
possible answers.

**Quirk #185: `:756` dereferences a `newTrace` that `:791` guards.**
`newTrace.combine()` runs with no null test 35 lines before
`newTrace != null && newTrace.normalize(...)`, and
`insertTraceWithoutCleaning` really can answer `null` after the `:659-661`
resample. No `catch` covers `:756`, so the port panics and Task 14's ruling turns
that into `AutorouteConnectionRouter.route:155-158`'s bare `FAILED` — the same
degraded value Java produces. Latent on every fixture row.

**The one asymmetry that is *not* a bug.** `insertForcedTracePolyline:570`
computes its `ShapeEntrySide` index as the shove line's index **minus one**,
where `checkForcedTracePolyline:429` uses the index itself. Probe mode `side`
prints both indices and both `ShapeEntrySide.no` values for 45 shape rows and every
row says `agree=true`: `ShapeEntrySide` walks *down* from its argument for the
first border crossing, so both forms find the same *entry* side. The expression
is transcribed exactly all the same, with an `obligation:` marker for Task 17.

**Where the numbers come from.**
`scripts/differential/java/probes/P6T15bProbe.java` (compiled with
`P6T9Probe.java`, whose board it reuses) and its committed stdout
`tests/data/p6t15b-insert-forced.txt` — eight modes, 11 448 lines, every
mutating row followed by `maxId`, `shoveFailingObstacle`/`Layer` and the whole
item list. `tests/board_ext.rs` regenerates each mode and compares it line by
line.

## `FoundConnectionInserter` (Task 15)

`autoroute/path/inserter.rs` ports `FoundConnectionInserter.java:23-807`, the
only class in Plan 6 that mutates the board's item set. `get_instance`
(`:40-111`) walks the locator's `connectionItems`, derives one via from each
layer change between consecutive `ResultItem`s (`:66`) plus one final via back
to `startLayer` (`:74`), inserts each trace corner-run through
`RoutingBoardExt::insert_forced_trace_polyline`, connects to a trace
start/target item (`:79`, `:94`) and ends in `normalizeTraces` (`:108`).

**The return value is Java's, not the brief's.** The brief asked for
`Option<InsertedItems { traces, vias }>`. Java's `getInstance` answers **the
instance**, whose two fields are private with no accessor, and its one caller
(`AutorouteEngine.autorouteConnection:265-277`) tests `== null` and nothing
else. There is no list of inserted items anywhere in Java — the board *is* the
result — so the port answers
`Result<Option<FoundConnectionInserter>, BoardError>` and the acceptance is read
off `board.get_items()` plus `max_generated_id()`, which is exactly what
`tests/inserter.rs` compares against the JVM.

**`Err` propagates; it is never `None`.** `None` is Java's `null`, which
`:271-277` turns into a *message-carrying* `FAILED`. An `Err` is a Java throw,
and no `catch` covers this class — `autorouteConnection` wraps only
`FoundConnectionLocator.getInstance` (`:181-196`) and the call at `:265` is
outside every `try`. The nearest handler is
`AutorouteConnectionRouter.route:155-158`'s **bare** `FAILED`. Task 16 must keep
the two apart.

**Quirk #186: the connection's `targetItem`/`startItem` are live references to
traces the insert has already split away.** `:77`/`:92` pass the item *by object
reference* to `RoutingBoard.connectToTrace`, which reads its polyline, layer and
net numbers and then removes the trace tails at that polyline's two end corners.
By then the insert has usually split that very trace in two (through
`insertVia` → `splitTraces`), and Java's reference keeps the dead object alive:
the stub is inserted against the **original** polyline and *both halves* of the
split trace are deleted. An id lookup answers `None` and skips the block. The
port therefore snapshots both traces at the top of `get_instance` and hands the
snapshot to `Board::connect_to_trace_of`, a second entry point added to
`fr-board` that is Java's object reference made explicit. Probe mode `diag` is
the fixture: routing to a *slanted* target trace leaves Java with one combined
trace and the naive port with three.

**Quirk #187: each `connectToTrace` stub is sized from the *other* end's
layer.** `:82` inserts the stub onto `connection.targetItem` with
`ctrl.traceHalfWidth[connection.startLayer]` and `:97` inserts onto
`connection.startItem` with `[connection.targetLayer]` — but
`RoutingBoard.connectToTrace` takes no layer: `:1135` reads
`toTrace.getLayer()` and inserts the copper there. The two indices are crossed.
Probe mode `diag` shows it live (`startItem=3 startLayer=1 targetItem=4
targetLayer=0`: a stub on layer 0 sized from layer 1). It is latent on every
fixture here only because `buildSimple` gives both layers half width 30; a DSN
with per-layer widths separates them. Transcribed verbatim with a `// Java bug:`
marker at each site.

**The `LinkedHashSet` at `:462` is a `Vec` with a membership test**, not a
`BTreeSet`. `insertFanoutMicroNeckdown` takes the *first* candidate half width
that reaches its target, so the insertion order is load-bearing: with a null
`startPin` and a base half width of 100 the candidate list is `[69, 75, 60, 50]`
and the JVM inserts a **69**-wide trace, where any sorted set answers 50. Probe
mode `micro` measures it.

**Where the numbers come from.**
`scripts/differential/java/probes/P6T15Probe.java` (compiled with
`P6T11Probe`, `P6T13Probe` and `P6T14Probe`, whose boards it reuses) and its
committed stdout `tests/data/p6t15-inserter.txt` — eight modes, 719 lines, every
insert followed by `maxId` and the whole item list including each via's padstack
and layer span. `tests/inserter.rs` regenerates the six `getInstance` modes and
compares them line by line; modes `neck` and `micro` are regenerated by
`inserter.rs`' own `#[cfg(test)]` module, because `insertNeckdown` is
package-private in Java and `tryNeckDown` / `insertFanoutMicroNeckdown` are
private, so an integration test — a different crate — cannot reach them.

## `autorouteConnection` end to end, and the Plan 6 half of `route` (Task 16)

`AutorouteEngine::autoroute_connection` is
`AutorouteEngine.java:130-280` transcribed in Java's order, and
`route_connection` is `AutorouteConnectionRouter.java:36-90` — steps 1-5 of
ruling 2's seam. Steps 6-8 (`optChangedArea`, the necked retry, the strict-DRC
rollback) were deferred to Plan 7 by markers at the foot of the function;
**Plan 7 Task 8 consumed all three** and added [`route_connection_full`] beside
`route_connection` without changing its signature (plan-7 ruling 2).

**Three of ruling 7's six recovery boundaries live here, and two of them are
nested rather than sequential.** A `MazeSearchEngine` borrows the engine for its
whole life, so it cannot be carried out of boundary #2's `catch_unwind` and into
boundary #3's; the port therefore runs `find_connection`'s catch *inside* the
construction one and reads the two-level `Option<Option<MazeResult>>` back out —
`None` is Java's `mazeSearchAlgo == null` (`:142`, `:145`) and `Some(None)` is
its `searchResult == null` (`:207`). The inner catch fires first, so a panic in
`findConnection` never reaches the outer one and the two degraded messages stay
distinct. Boundary #4 wraps `FoundConnectionLocator::get_instance` (`:190`), and
boundary #5 wraps the whole of `route_connection` (`:154-158`).

**What is deliberately *not* caught.** `:260-266` — `removeItems`,
`removeTraceTails` and `FoundConnectionInserter.getInstance` — sits outside all
four of Java's `try` blocks, so a throw there reaches
`AutorouteConnectionRouter.route:155-158`'s **bare** `FAILED`. The port's two
`Result` channels at those lines therefore panic rather than return, and
boundary #5 converts them; returning a `FAILED` instead would be wrong for a
reason that only shows up in Plan 7, because `:123-125` runs the necked retry on
a returned `FAILED` and Java's exception path skips it.

**`:230-235`'s `SKIPPED` is dead code and is not ported as a branch** (quirk
#180, Task 14). `connectionItems` is a `final` field assigned at
`FoundConnectionLocator.java:101`, before both of the constructor's early
returns, so the `== null` test cannot hold; `tests/autoroute_connection.rs`
asserts that nothing in `engine.rs` can produce the state.

**`:221-228` needs a power plane, not a disabled signal layer.** Setting
`ctrl.layerActive[0] = false` on a signal layer makes
`MazeSearchEngine.expandToRoomDoors:396-399` return before expanding anything,
so the search answers null and `:207` fires instead. The only shape that reaches
the "layers are disabled" message is a layer whose `isSignal` is false:
`AutorouteControl.java:151-158` forces its `layerActive` off *and* `:397-399`'s
guard does not fire, so the located `startLayer` is a disabled one. Probe mode
`inactive` builds it.

**The two orders that are Java's `TreeSet<Item>`.** `describeConnection`
(`:282-287`) joins both sets and `:247` walks `rippedItemList`, and
`Item.compareTo` (Item.java:95-102) is `other.id - this.id`, so all three are
**descending** by id (quirk #44). The probe's `describe` mode pins it directly:
`describeConnection({2,4,6}, {3,5})` is `"polylinetrace, pin of component #2, pin
of component #1 and pin #1 of component #2, pin #1 of component #1"`.

**`route_connection` takes `&mut Option<AutorouteEngine>`, not
`&mut AutorouteEngine`.** `RoutingBoard.initAutoroute` (`:882-897`) reads *and
writes* the nullable field `RoutingBoard.autorouteEngine`; `&mut Option<_>` is
that field, and the write happens before `autorouteConnection` runs, so it
survives boundary #5's unwind exactly as Java's field assignment survives a
throw.

**Four of `route:38-47`'s five inputs are parameters, not derivations.** They are
per-`BatchAutorouter` fields, and the two production constructors disagree about
them: `BatchAutorouter.java:110-121` (the `RoutingJob` one) derives
`removeUnconnectedVias = !settings.isFanoutEnabled()`,
`traceCosts = getTraceCosts()` and
`startRipupCosts = settings.getStartRipupCosts()`, while `:253-261`
(`autoroutePassesForOptimizingItem`, the optimizer's autorouter — Plan 7's
`BatchOptimizer` path) passes `removeUnconnectedVias = true` **unconditionally**
and takes `startRipupCosts` from *its* caller. So `trace_costs`,
`start_ripup_costs`, `remove_unconnected_vias` and `retain_autoroute_database`
are all parameters here, and Plan 7 wires the second constructor without
changing this signature. The `RoutingJob` values are the documented defaults and
are what every call site in `tests/autoroute_connection.rs` passes;
`retain_autoroute_database` is `BatchAutorouter.isRetainAutorouteDatabase()`
(`:172-173`), the benchmark-only system property
`freerouting.benchmark.retain_autoroute_database`
(`BatchAutorouter.java:63-64,154`), which the sibling
`BatchAutorouterThread.java:90` hard-codes `false` as well — so it is `false` in
every production and parity run. The fifth, `getTracePullTightAccuracy`, is read only by step 6.

**`RoutingBoard.finishAutoroute` is deliberately absent.** Java calls it only
from `RoutingPipeline.java:110` and `RoutingBoardUndoFacade.java:55`, never from
`AutorouteConnectionRouter.route`, so a `route_connection` that called it would
clear a database the next connection is entitled to reuse.

**The five `additionalUpdateAfterChange` markers in `fr-board` are re-pointed to
Plan 7, with the measurement.** `Board::insert_item`, `Board::remove_item`,
`ShapeTraceEntries::fast_cutout_trace`, `Board::combine_trace` and
`Board::split` all sit on a path Plan 6 reaches, but the call needs an
`AutorouteEngine`, which `fr-board` cannot name (plan-2 ruling 4/11). The
`PolylineTrace.change` precedent — make the call at the `fr-router` call site —
works there because that method has exactly one `fr-router` caller. These five
have **no `fr-router` caller at all** — every one of them is reached only from
inside `fr-board`, and the two busiest, `Board::insert_item` and
`Board::remove_item`, are the funnel every typed inserter and every removal path
goes through — so the engine would have to be threaded through the whole
inserter family, the normaliser and the splitter, and therefore through every
Plan 2-5 caller and test. And the call is a no-op on every path either plan runs:
`RoutingBoard.additionalUpdateAfterChange:100-102` returns unless
`maintainDatabase`, which is `retainAutorouteDatabase`, which is the benchmark
property above. Plan 7 owns `BatchAutorouter` and is where the flag can first be
true.

**The insert is handed `&|| false`, not the caller's stop check** (controller
ruling AC). Java tests cancellation **nowhere** below `AutorouteEngine.java:265`:
plan-6 ruling 6's six sites are all in `MazeSearchEngine.init` / the pop loop
plus `DrillPage.java:103`, and `FoundConnectionInserter.getInstance`,
`ForcedViaInserter.insert`, `BasicBoard.insertVia`, `BasicBoard.splitTraces` and
`PolylineTrace.split` carry no test at HEAD. An earlier draft passed `stop`
down, and `P6T16Probe`'s `stopafter` mode measured what that costs: at
`limit = 20` on `buildSimple` the JVM answers `ROUTED` after 14 stop checks,
while the port made 21, aborted the insert with `BoardError::Stopped` and
answered `AutorouteConnectionRouter.route:155-158`'s bare `FAILED` — *after*
`:260` had already removed the ripped items, so the board was worse than either
outcome. Exact parity wins; the `limit = 20` row now routes on both sides and is
what pins the argument.

**Two consequences, both deliberate.** First, **quirk #76's ladder hang becomes
reachable from the router's insert path, exactly as it is in Java**:
`PolylineTrace.split`'s entry re-walk does not terminate on a four-rung ladder,
and the `StopCheck` plan-3 ruling F added to `Board::split_traces_checked` is
what a caller would have used to escape it. That check stays on the method for
`fr-dsn`'s reader, which is the caller ruling F was actually written about; the
router simply does not use it. Second, **the wall clock belongs to the layer
above**: `AutorouteConnectionRouter.route:71-74` builds a per-connection
`TimeLimit` that `AutorouteEngine.isStopRequested` consults at ruling 6's six
sites, and Plan 7's `AutorouteBatchLoop` owns everything outside that. Note that
Java does *not* discard a cancelled pass — `AutorouteBatchLoop.java:547` assigns
`job.board = router.board` and the only restore is the score-driven best-board
swap at `:525-537`, which never consults the stop flag — so an insert the port
aborted would have been kept, not thrown away. That is the second reason the
check does not belong here.

**Where the numbers come from.**
`scripts/differential/java/probes/P6T16Probe.java` (compiled with `P6T11Probe`,
`P6T13Probe`, `P6T14Probe` and `P6T15Probe`, whose boards and dump helpers it
reuses) and its committed stdout `tests/data/p6t16-autoroute-connection.txt` —
**14 modes over three angle regimes**, each printing the
`AutorouteAttemptResult` (state *and* details), the ripped ids, the per-item
ripup costs, `maxGeneratedId` before and after, and the whole item list.
`tests/autoroute_connection.rs`' 17 tests regenerate every one of them and
compare line by line; all 14 modes MATCH byte for byte.

**What no unit fixture reaches** (`grep -rn "obligation:"
crates/fr-router/src/autoroute/maze/engine.rs`, seven rows — six of them Task
16's, the seventh Task 6's `completeExpansionRoom` contract — all forwarded to
Task 17): the `:247` and `:260` iteration orders (no fixture rips more than one
item), `:241-245`'s `StopConnectionOption` and `:46`'s `removeUnconnectedVias`
(no fixture has a fanout via), `:45`'s `startRipupCosts * ripupPassNo` (the
ripup price saturates identically at passes 1, 2 and 4 on a one-trace obstacle)
and `:271-277`'s "could not be inserted" arm. That last one is the interesting
negative result: every lever that makes `FoundConnectionInserter.getInstance`
answer null from *outside* `autorouteConnection` — an empty `ctrl.viaRule`, a
user-fixed via on the drill location — also changes what the maze search finds,
because `ForcedViaInserter.check` reads the same rule. Measured: setting
`ctrl.viaRule = new ViaRule("empty")` before the connection makes the JVM route
*around* the drill and answer `ROUTED`.


## The acceptance ladder (Task 17, ruling 1)

Task 17 is the first time the port routes a **real KiCad board** against the HEAD
jar. Everything below the plan-6 seam runs: `AutorouteControl`, the plane swap,
the `TimeLimit`, `initAutoroute` and `autorouteConnection`, connection after
connection, on the board each previous connection left behind.

**The driver pair is `scripts/differential/run.sh p6t1`** —
`scripts/differential/java/P6T1.java` (package `app.freerouting.autoroute.maze`,
compiled against the HEAD jar on JDK 25) against
`scripts/differential/rust/src/bin/p6t1.rs`. Both print one JSON line per
connection: the attempt state and its `details`, the ripped-item id set, the
per-item ripup costs, `maxGeneratedId` before and after, **every inserted trace**
(id, layer, half width, polyline corner list) and **every inserted via** (id,
centre, padstack, layer span), then spec §9's metric block. Every coordinate is
rendered by `Double.toString` on the Java side and `java_double_to_string` on
this one, as a JSON *string*, so the rendering itself is the comparison surface.

Four choices make the two sides the same experiment, and each is load-bearing:

1. **The settings come from `DefaultSettings`**, sized and tuned for the board by
   `setLayerCount` + `applyBoardSpecificOptimizations` — the two board-dependent
   steps of `RouterSettings(RoutingBoard)`. A bare `RouterSettings::new()` would
   leave `automaticNeckdown` false (`DefaultSettings.java:103` sets it true) and
   `fanout.enabled` false, which takes `tryNeckDown` *and* the `FanoutVia` arm of
   `:241-245` out of the comparison — two of Task 15's and Task 16's obligations.
2. **`startMarkingChangedArea` runs before every connection**, because
   `AutoroutePassRunner.java:224` runs it there and quirk #177 makes the presence
   of `board.changedArea` observable inside `TraceShover.insert`.
3. **The connection list is Java's, computed once**: `board.getItems()` order —
   `itemList` in descending item id, quirk #63 — × each item's own net index
   order, keeping the `(item, net)` pairs whose `getUnconnectedSet` is non-empty.
   That is `AutoroutePassRunner`'s nested walk (`:202`, `:207`) with the pass
   loop and the plane-skipping filter (Plan 7's) left out.
4. **Nothing above the seam runs**: no `optChangedArea`, no `retryConnectionNecked`,
   no strict-DRC rollback, no `finishAutoroute`.

### The references, and the hash-mode premise

`scripts/gen-router-reference.sh` drives the same driver against the HEAD jar
and writes `tests/reference/<stem>/{router.jsonl,router.meta.txt,java.log}`;
`tests/reference/router-fixtures.txt` is the `stem|dsn|max_items` table both it
and `crates/fr-router/tests/reference_parity.rs` read, so the generator and the
tests cannot drift apart. `router.jsonl` is the driver's stdout **verbatim**
minus its `HEADER` line, which names the jar by absolute path and lives in
`router.meta.txt` instead.

`scripts/gen-router-reference.sh --verify-hash-modes` regenerates every stem
under `-XX:hashCode=0,1,2,3,4` and requires five byte-identical files. It
**passes on all five stems** — 8, 294, 45, 0 and 22 connections:

    == router-rpi-splitter    5 modes agree (f4f76285affd, 8 connections)
    == router-dac2020-bm01    5 modes agree (a7a7c0dc039c, 294 connections)
    == router-j2-reference    5 modes agree (b7198cefe3a2, 45 connections)
    == router-tutorial-board  5 modes agree (e3b0c44298fc, 0 connections)
    == router-ecc83-input     5 modes agree (64318fe24b56, 22 connections)

That is ruling 1's premise checked **per connection** rather than end-to-end on
SES output: a single-threaded Java routing run does not depend on
`Object.hashCode`, which is what makes an exact per-connection reference
meaningful at all. It is a stronger result than the DRC generator's, where one
of eight stems is genuinely hash-dependent (quirk #146).

### The result, per stem and per rung

| stem | board | connections | (a) state + ripped | (b) geometry | (c) metrics |
|---|---|---|---|---|---|
| `router-rpi-splitter` | `Issue143-rpi_splitter.dsn` | 8 (of 9) | **8/8** | **8/8** | **8/8** |
| `router-dac2020-bm01` | `Issue508-DAC2020_bm01.dsn` | 294 (all) | **294/294** | **294/294** | **294/294** |
| `router-j2-reference` | `Issue026-J2_reference.dsn` | 45 (all) | **45/45** | **45/45** | **45/45** |
| `router-tutorial-board` | `examples/tutorial_board/tutorial_board.dsn` | 0 | — | — | — |
| `router-ecc83-input` | `Issue649-kicad_ecc83-pp_input_board_v1.dsn` | 22 (all) | **22/22** | **22/22** | **22/22** |

**369 connections, and rung (b) — the strictest one — holds on every one of
them**, including the item ids the connection burned. The ladder's demotion path
(a connection that reaches (a)+(c) but not (b) becomes a row here rather than a
failure) is unused, and `geometry_is_required_where_it_was_reached` is the test
that stops it being quietly re-entered.

Three deviations from the plan's fixture table, all upward except the last:

* **`router-dac2020-bm01` routes the whole board, not the plan's two
  connections.** The plan set `max_items = 2` before it was known that the board
  matches exactly; every ripping connection in the corpus lives on it (15 of them
  rip two or three items at once), and truncating it would have left three of
  Task 16's obligations unreachable. 294 connections cost ~36 s wall for both
  sides together.
* **`router-j2-reference` and `router-ecc83-input` likewise route whole boards.**
  `router-ecc83-input` is new: the plan's table had no board with a `(plane …)`
  net, i.e. no `ConductionArea` on the search tree.
* **`router-tutorial-board` routes nothing, and that is the assertion.** The
  shipped example board's `(network …)` scope is 438 empty `@:no_net_N` nets, so
  no item has an unconnected set. The stem is kept as a regression guard on the
  DSN reader and on `Board::unconnected_set`: "the port agrees there is nothing
  to route here", with the zero pinned in `router.meta.txt`.

### The MISMATCH this found, and the fix

**`router-j2-reference` connection k = 19**, `:271-277`'s "could not be inserted"
message. Both sides compute the same destination set —
`[946,945,944,936,935,43,42,37,36]`, five traces and four pins — but the jar's
message names five `polylinetrace`s and the port's named four.

`describeConnection`'s inputs are Java `Set<Item>` — **object references** taken
at `AutorouteConnectionRouter.route:54-68` — while the port's are
`BTreeSet<ItemId>`. The failed insert removes one of those five traces, so an id
lookup against the board as it stands at `:271` silently drops it, where Java's
live reference still prints. Task 16 had recorded that as an accepted asymmetry
in `describe_connection`'s doc comment; the corpus proved it wrong. The fix is in
`autoroute/maze/engine.rs`: `autoroute_connection` snapshots both sets'
`Item.toString()`s at entry — before anything can remove an item — and every one
of the five `describeConnection` sites builds its message from the snapshot.
Snapshotting is exactly equivalent to Java's late evaluation, because
`Item.toString` (Item.java:1258-1269) and `Pin.toString` (`:676-692`) read only
`getClass().getSimpleName()`, `componentId` and `pinIndex`, none of which a
routing pass can change. `describe_connection(board, start, dest)` stays as the
public entry point for a caller that has not mutated the board (`P6T16Probe`'s
`describe` mode reaches it by reflection), and `router_j2_reference` is the
regression test.

### `ripupPassNo > 1` on the largest board — CLOSED (ruling AD, Task 17b, `ead7902`)

Every committed `ripupPassNo = 1` reference is joined by one at **2**:
`tests/reference/router-dac2020-bm01-pass2/`, whose fixture row carries the
optional fourth field `ripup_pass_no`. The ladder is green on all five stems at
passes **1, 2 and 4**:

    pass 1  rpi-splitter MATCH  dac2020 MATCH  j2 MATCH  tutorial MATCH  ecc83 MATCH
    pass 2  rpi-splitter MATCH  dac2020 MATCH  j2 MATCH  tutorial MATCH  ecc83 MATCH
    pass 4  rpi-splitter MATCH  dac2020 MATCH  j2 MATCH  tutorial MATCH  ecc83 MATCH

Reproduce any rung with

    scripts/differential/run.sh p6t1 \
        ../freerouting/fixtures/Issue508-DAC2020_bm01.dsn 100000 2

(`4` and the control `1` likewise). This is where
`AutorouteConnectionRouter.route:45`'s `startRipupCosts * ripupPassNo` and — at
pass 4 — `MazeRipupResolver`'s `randomize` draw (plan-6 ruling 5's bit-exact
`JavaRandom`, seeded with `ctrl.ripupCosts`) become live.

**What it was.** `router-dac2020-bm01` used to match k = 1..266 at passes 2 and
4 and then diverge: at k = 267 the output was identical corner for corner but
the port consumed **1 489** item ids against the jar's **1 483** (six extra
*transient* ones, `"maxIdAfter":438610` against `438604`), k = 268/269 differed
by that id shift alone, and k = 270 was the first real geometry difference.

**What it was not.** Task 17's hypothesis — an off-by-one in
`insertForcedTracePolyline` / `springOverObstacles` or in the shove's
substitute-piece loop (Task 15a/15b code), with the rational-vs-integer corner at
k = 270 as the lead — is **disproved**. Nothing in this crate was wrong.

**The cause.** `PolylineTrace.change` (PolylineTrace.java:960, :972) compares the
old and the new line arrays with `!=` on `Line` objects — **reference identity**.
The two indices it finds become `keepAtStartCount` / `keepAtEndCount`, which
decide how many search-tree leaves `ShapeSearchTree.changeEntries` reuses instead
of removing and re-inserting; a leaf re-inserted lands elsewhere in
`MinAreaTree`. The port compared by value (quirk **#74**, filed by Plan 2 as an
unavoidable divergence and given a production caller by Task 15b's
`pull_tight_with_engine`), kept more leaves, and so held the same leaves in a
different tree **shape** — first observably at connection 92.
`ShapeSearchTree45Degree.completeShape` walks that tree with an explicit stack, so
its obstacle order *is* the tree's shape: it met the obstacles in a different
order, `restrainShape` cut a different half-plane, one completed free-space room
came out `[(1548429,-1053804)..]` in the jar against `[(1553913,-1063354)..]` in
the port, and that moved a door, a `MazeListElement.sortingValue` and finally the
six ids.

**The fix** is `fr_geometry::Line`'s private identity token and
`Line::is_same_object`, with `Board::change_trace` its only caller — quirk #74 in
`docs/java-quirks.md`, now *reproduced* rather than open, and Plan 6 controller
ruling **AE** for the departure from "no static mutable state" it takes.
`scripts/differential/java/P6T17bProbe.java` and
`crates/fr-router/tests/data/p6t17b-dac2020-k267-pass2.txt` are the instrument and
the transcript that found it.

### Ruling H is decided, and it closes against the re-pointing

`scripts/differential/run.sh p6t1 ../freerouting/fixtures/Issue593-BBD_Mars-64.dsn
50 1 crates/fr-router/tests/data/ruling-h-redeclare.rules` is the comparison the
register row had been waiting for since Plan 3 — the port routing the board with
the `.rules` file, against the jar doing the same.

* **without** the `.rules` file the two agree on all 50 connections, byte for
  byte;
* **with** it they first differ at k = 6 (one extra item id, identical geometry)
  and genuinely diverge from k = 8 on: the jar lays four traces there and the
  port two, cumulative trace length `1401450.8259119983` against
  `1395031.4105961146`. The port routes *shorter*, which is exactly what
  `attachSmdAllowed = true` buys — a via attaching to an SMD pad the jar's
  detached `ViaInfo` forbids. Rungs (a) and (c) still hold on every connection;
  it is (b) that fails.

So the divergence is **not** unobservable and plan-6 ruling 9's default does not
apply. Per its other branch the fix is `ViaRule` owning its `ViaInfo`s (or
`ViaInfos` keeping tombstones), an `fr-board` change outside this task's file
list and named in the Task 18 hand-off. No acceptance fixture uses a `.rules`
file, so the reference set is unaffected.

**Landed in Plan 7 Task 0** (controller ruling AL): `fr_board::rules::ViaRule`
holds `Vec<ViaInfo>` — owned copies, this port's spelling of Java's
`List<ViaInfo>` of object references (`ViaRule.java:21`) — so `ViaInfos::remove`
cannot re-point a rule and `RulesReader.applyViaInfo` leaves every rule on the
detached original. The command above is now **MATCH on all 50 connections**, k = 6
and k = 8 included; the transcript is committed as
`crates/fr-router/tests/data/p7t0-ruling-h-match.txt`. The `obligation:` marker on
`AutorouteControl::rebuild_via_info` was rewritten to record the closure.
`ViaRule::{contains_padstack,get_layer_range}` lost their `&ViaInfos` argument and
`get_via` returns `&ViaInfo`, which is what Java's signatures always were. The
*via-rule* half of the register row — `Network.addViaRule` replacing a `ViaRule`
while `NetClass.viaRule` keeps the detached original — is **still open**.

### The 27 `obligation:` markers

`grep -rn "obligation:" crates/fr-router/src` answers 27 rows, and they come out
**14 discharged / 8 re-marked / 5 not this task's**.

Task 17 wrote this section when the grep answered **28**; the twenty-eighth was
ruling H's marker on `AutorouteControl::rebuild_via_info`, and **Plan 7 Task 0
closed it** — the block at `control.rs:385` is now a `# Ruling H, closed` record,
not an `obligation:`, so it is outside the total. Its row is at the foot of the
table below, kept for the trail and marked closed.

The twenty-seven:

* **14 discharged** — a corpus connection reaches the arm *and* the whole
  connection matches the jar, with the fixture and the connection index in the
  table below. Three of them are discharged twice over, by mutation as well as by
  coverage: `routing_board_ext.rs:796` and `:939` both DIFF `router-j2-reference`
  when mutated to the alternative Java could have been written with.
* **8 re-marked** — the measurement that says why no corpus board reaches them.
* **1 discharged by construction** (`engine.rs:761`, `completeExpansionRoom`'s
  `Err` contract: Tasks 11-16 consume it as an empty list, and the corpus never
  takes the `Err` arm at all), **2 already discharged in Task 6**
  (`maze/mod.rs:57`, `expansion/mod.rs:190`), and **2 that are not Task 17's** —
  `expansion/complete_room.rs:5` is Plan 2's `TreeObject::Room` obligation,
  discharged by the class existing, and `tightener/mod.rs:759` is a
  cross-reference to an `fr-board` marker in `board/trace_normalize.rs`.

| marker | Java | verdict | evidence |
|---|---|---|---|
| `board_ext/routing_board_ext.rs:730` | `insertForcedTracePolyline:777-782` | **re-marked** | 0 entries over 369 connections; every corpus call has `maxRecursionDepth > 0` |
| `board_ext/routing_board_ext.rs:796` | `insertForcedTracePolyline:826-833` | **discharged** | ≥2 candidate traces on rpi-splitter (2), j2 (4), dac2020 (6); mutating `next_back()` → `next()` DIFFs j2 at k = 44 |
| `board_ext/routing_board_ext.rs:939` | `insertForcedTracePolyline`'s `ShapeEntrySide` index | **discharged** | differs from `i + 1` on 50/508/5160/39 evaluations; mutating to `i + 1` DIFFs j2 and dac2020 |
| `board_ext/tightener/mod.rs:759` | `Board::change_trace`'s call site | not Task 17's | a cross-reference to `fr-board`'s own marker |
| `autoroute/path/inserter.rs:123` | `autorouteConnection:260-263` | **re-marked** | both endpoints resolve on the board in all 311 evaluations, the 97 ripping connections included |
| `autoroute/path/inserter.rs:298` | `insertTrace:151-162` | **re-marked** | the loop runs 820 times (564 of them find exactly one pin); the count is never more than 1 |
| `autoroute/path/inserter.rs:410` | `insertTrace:264` | **discharged** | VIOLATION_CORRECTED on the **last** corner: rpi-splitter 2, j2 6, dac2020 7 |
| `autoroute/path/inserter.rs:456` | `insertTrace:448-450` | **discharged** | the suppressed second write happens 6 / 8 / 85 times on rpi-splitter / j2 / dac2020 |
| `autoroute/path/inserter.rs:699` | `tryNeckDown:553` | **re-marked** | the gate is reached 116 times and is a strict inequality every time; nothing below it is reached |
| `autoroute/path/inserter.rs:748` | `tryNeckDown:586-588` | **re-marked** | 0 entries — `:553` returns on all 116 |
| `autoroute/path/inserter.rs:924` | `insertVia:687-696` | **discharged** | called with `fromLayer > toLayer` 2 / 4 / 34 times on rpi-splitter / j2 / dac2020 |
| `autoroute/path/locator.rs:267` | `FoundConnectionLocator:124-129` (fanout) | **re-marked** | 0 entries; `ctrl.isFanout` is `BatchFanout`'s, i.e. **Plan 7's** — unreachable below the seam |
| `autoroute/path/locator.rs:398` | `FoundConnectionLocator:167-175` | **re-marked** | max dimension seen is 1 (dac2020 14, j2 4); `router-ecc83-input`'s conduction areas give 0 |
| `autoroute/maze/mod.rs:57` | `AutorouteEngine.TRACE_WIDTH_TOLERANCE` | discharged in Task 6 | one definition, still |
| ~~`autoroute/maze/control.rs:381`~~ -> `control.rs:385`, **no longer an `obligation:`** | `AutorouteControl.rebuildViaInfo` | **closed in Plan 7 Task 0** (outside the 27) | ruling H closed against the re-pointing; `ViaRule` owns its `ViaInfo`s and the `.rules` repro is MATCH on all 50 connections (`crates/fr-router/tests/data/p7t0-ruling-h-match.txt`) — see above |
| `autoroute/maze/ripup_resolver.rs:170` | `checkRipup:86-91` (`roomWasShoved`) | **discharged** | entered 5 / 214 / 12 798 times on rpi-splitter / j2 / dac2020 |
| `autoroute/maze/ripup_resolver.rs:189` | `checkRipup:92-94` (`ALREADY_RIPPED_COSTS`) | **discharged** | returned 1 / 27 / 3 161 times on the same three |
| `autoroute/maze/trace_shover.rs:404` | `checkShoveTraceLine:236-312` | **discharged** | `:255` pushes 15 / 498 / 27 083, `:307` pushes 12 / 819 / 52 450 |
| `autoroute/maze/expansion_engine.rs:503` | `expandToOtherLayers:279-281` | **re-marked** | 0 entries; all five corpus boards are two-layer with no bottom-side SMD pad |
| `autoroute/maze/engine.rs:761` | `completeExpansionRoom`'s `Err` contract | discharged by construction | Tasks 11-16 consume it as an empty list; the corpus never takes the `Err` arm (0 panics in 369 connections) |
| `autoroute/maze/engine.rs:1374` | `autorouteConnection:241-245` | **discharged** | all 311 connections that reach it take the `FanoutVia` arm, which no unit fixture took |
| `autoroute/maze/engine.rs:1396` | `autorouteConnection:247` | **discharged** | dac2020 rips 2 items at k = 252 and 3 at k = 261/279/286/293 |
| `autoroute/maze/engine.rs:1423` | `BasicBoard.removeItems`' order | **discharged** | dac2020 removes 2 (×7), 3 (×22), 4 (×3) and 7 (×1) connection items at once |
| `autoroute/maze/engine.rs:1480` | `autorouteConnection:271-277` | **discharged** | 20 connections: rpi-splitter k = 3, 8; j2 k = 6, 9, 10, 12, 15, 19; dac2020 k = 23, 25, 26, 124, 125, 127, 162, 202, 246, 287, 288, 290 |
| `autoroute/maze/engine.rs:1718` | `route:45` (`startRipupCosts * ripupPassNo`) | **discharged** | passes 1, 2 and 4 MATCH on all five stems (Task 17b closed the dac2020 XDIFF — quirk #74, section above) |
| `autoroute/maze/engine.rs:1730` | `route:46` (`removeUnconnectedVias`) | **discharged** | the corpus runs it `false` on all 369 connections, the opposite of every unit fixture |
| `autoroute/expansion/mod.rs:190` | `AutorouteEngine.clear` | discharged in Task 6 | — |
| `autoroute/expansion/complete_room.rs:5` | Plan 2's `TreeObject::Room` | discharged by the class | — |

### The controller's Task 12 and 13 standing notes

Those notes named arms "no unit fixture discriminates" and asked which corpus
fixture hits which. Measured with counters over
`tests/reference/router-fixtures.txt` (rpi-splitter / j2-reference / dac2020-bm01
/ ecc83-input), every count on connections that match the HEAD jar:

| arm | Java | reached? | counts |
|---|---|---|---|
| `roomShapeIsThick`'s **`Via`** arm | `MazeSearchEngine.java:1115-1117` | **yes** | 33 / 163 / 6 564 / 0 |
| `expandToDrillPage` | `MazeExpansionEngine.java:115-143` | **yes** | 2 342 / 9 244 / **607 095** / 702 |
| `expandToDrill` | `MazeExpansionEngine.java:32-113` | **yes** | 4 671 / 5 442 / 198 542 / 0 |
| `MazeTraceShover`'s door-section collector | `:236-312` | **yes** | see obligation row `trace_shover.rs:404` |
| `checkRipup`'s `roomWasShoved` / `ALREADY_RIPPED_COSTS` | `:86-91` / `:92-94` | **yes** | rows `ripup_resolver.rs:170` / `:189` |
| both `withNeckdown` call sites | `AutorouteControl.java:168` | **yes** | `tryNeckDown` runs 116 times (rows `inserter.rs:699`/`:748` are about the *ties* inside it, which are not reached) |
| the **fanout** arms | `FoundConnectionLocator:124-129`, `BatchFanout` | **no** | 0 — `ctrl.isFanout` is Plan 7's |
| `DRILLABLE_WITH_ATTACH_SMD` on the solder side | `expandToOtherLayers:279-281` | **no** | 0 — row `expansion_engine.rs:503` |

`router-ecc83-input` builds 702 drill pages and expands to **no** drill: its via
padstack never fits, which is also why it is the only stem whose
`roomShapeIsThick` never sees a `Via`. Only the two "no" rows above are still
open, and both are named in the obligation table.

The eight re-marked rows fall into three groups, which is the useful thing to
say about them:

* **Plan 7's, not a fixture's.** `locator.rs:267`'s fanout arm needs
  `ctrl.isFanout`, which only `BatchFanout` sets, and `locator.rs:398` needs a
  connection whose *start item* is a conduction area, which
  `AutoroutePassRunner`'s plane-skipping item selection
  (`BatchAutorouter.java:383-389`) decides. Both live above the plan-6 seam; no
  board can discharge them here. The same is true of the half of
  `engine.rs:1374`'s question that survives its discharge — the corpus takes the
  `FanoutVia` arm on every connection but cannot show it *differing* from `None`,
  because that needs a fanout via.
* **A fixture the corpus does not have.** `expansion_engine.rs:503` needs a
  bottom-side SMD pad (all five boards are two-layer with none);
  `inserter.rs:298` needs two own-net pins sharing one trace end;
  `inserter.rs:699`/`:748` need a pin narrower than the trace with a clear
  diagonal run from it.
* **Deeper than 369 connections go.** `routing_board_ext.rs:730` needs a shove
  chain that exhausts the recursion budget, and `inserter.rs:123` needs a ripup
  that removes the located connection's own start or target trace.


## `BoardHistory`, the pass loop's best-board memory (Plan 7 Task 2, ruling AF)

`autoroute/BoardHistory.java` was rostered `// not ported:` by plan-6 ruling 13,
on spec §2's "undo store" exclusion. **Controller ruling AF overturns that**: it is
not an undo store, it is the memory `AutorouteBatchLoop` restores from at
`:306-320` and again at `:525-547`, and without it the board written to SES is the
*last* pass's rather than the *best* pass's. It lives at
`src/pipeline/board_history.rs`; the eleven roster lines it supersedes are gone,
and `scripts/audit-map/fr-router.map` now points the class at that file.

### Ruling 8's memory note: 30 live `Board`s, not 30 `byte[]`s — measured

Java stores each snapshot as `board.serialize(false)`, a `byte[]`. This port has no
`Serializable` (`global-constraints.md`), so ruling 8 stores a `Board` clone, and
`MAX_HISTORY_SIZE` is 30 either way. The plan named the fallback in advance — a
`Vec<u8>` of the port's own compact encoding — and required the cost to be measured
on `Issue730-DAC2020_bm11.dsn` rather than guessed.

**Measured** (release build, macOS `/usr/bin/time -l` maximum resident set size, the
fixture routed to completion first — 195 connections, 440 items, 179 traces,
27 vias):

| history | peak RSS |
|---|---|
| 0 clones (the routed board alone) | **26.9 MB** |
| 30 clones | **59.4 MB** |

So a full history costs **≈ 32.5 MB, ≈ 1.08 MB per board**, against a 26 MB DSN
that reads into a 27 MB process. That is comfortably affordable and the fallback
stays unbuilt. The number is recorded here so a later board that is an order of
magnitude larger has something to be compared against.

### What a restored board keeps, and what it loses

`restoreBoard:148` is `BasicBoard.deserialize(entry.board)` — a Java serialization
round trip — so the restored board is the snapshot with every `transient` field
reset by `readObject` (`BasicBoard.java:1388-1400`), and only those.
[`Board::deep_copy`] is that round trip, which is why the port takes a plain
`clone()` at `:196` (Java's `serialize`) and calls `deep_copy` at `:148` (Java's
`deserialize`), rather than the other way round. The pairing is asserted field by
field by `tests/board_history.rs`'s
`a_restored_board_is_javas_deserialize_round_trip`.

| kept, because Java's `serialize(false)` writes it | lost, because Java's field is `transient` |
|---|---|
| the item map — ids, geometry, nets, clearance classes, fixed states | every item's `autorouteInfo` (`Item.java:67`) |
| components, rules, library, bounding box | `changedArea` (`RoutingBoard.java:67`) |
| **the item-id counter** — `communication` is a `public final Communication` (`BasicBoard.java:88`) and `ItemIdGenerator.lastGeneratedId` is non-`transient`, so a restored board **re-issues the ids the discarded board burned** | `shoveFailingObstacle` (`:72`) |
| `failureLog` — `public final`, not `transient` (`RoutingBoard.java:64`) | `shoveFailingLayer` (`:73`) — back to **`0`**, not the `-1` a fresh board starts at, because deserialization runs no constructor (the reproduced Java bug is on `Board::deep_copy`) |
| | `normalizeSuppressedNetNos` (`BasicBoard.java:96`), `revision` (`:97`) |
| | the search-tree manager (`:94`); Java rebuilds it by reinserting every item in descending id order, this port clones it — `crates/fr-board/src/board/snapshot.rs`'s module doc is the argument that the two are indistinguishable |
| | `autorouteEngine` (`RoutingBoard.java:70`), which plan-6 ruling 3 puts outside `Board` altogether |

**Nothing had to be added to `fr-board`.** `deep_copy` already resets all four
transients, already clears the autoroute scratch — a no-op in JAVA's round trip
(`autorouteInfo` is `transient`, so a deserialized board's is null anyway) but
LOAD-BEARING in the port, whose `deep_copy` is a structural clone — and
already preserves the id counter. The one place the port and Java's clone differ is
`fr_geometry::Line`'s identity token (plan-6 ruling AE): Java's serialization mints
**new** `Line` objects, preserving reference sharing *within* the copy but never
between the copy and the original, whereas the port's `Clone` copies the token, so a
copied `Line` is `is_same_object` to the original's. That is unobservable —
`Line::is_same_object` has exactly one caller in the workspace,
`Board::change_trace` (quirk #74), which compares a trace's new lines against **that
same trace's** old lines inside one board.

### `p7t2` and the two recorded hash divergences

`scripts/differential/java/probes/P7T2Probe.java` drives the real `BoardHistory`
through **61** calls over five phases — a cap-3 history (32 calls), the default
cap-30 one (11), the `<=` eviction tie (4), `Float.compare`'s fourteen pairs, and a
replay of `src/test/java/app/freerouting/autoroute/BoardHistoryTest.java` (14) —
printing the list's whole contents after every call. Its stdout is
`tests/data/p7t2-board-history.txt`; `tests/board_history.rs` regenerates the
`boards` section and the first four phases — 291 lines — and compares them **line
for line**. Hashes print as labels `H0`, `H1`, …
in order of first appearance, so what is compared is the equality *pattern* across
entries, never the value (ruling AH).

The `BoardHistoryTest` replay is deliberately **outside** that byte comparison. It
found two divergences that ruling AH's Task 3 then settled — **in opposite
directions**:

| divergence | JVM | this port |
|---|---|---|
| `empty_board.dsn` (1 item) vs `Issue159-setonix_2hp-pcb.dsn` (199 items) — **neither has a trace or a via**, and `Board::structural_hash` hashed only traces and vias | different hashes | **closed by Task 3.** Before the widening the port gave both the same hash, so `contains` answered `true` where Java answers `false` and `add` refused a board Java accepts; three of the six ported `BoardHistoryTest` methods carried an `XDIFF:` assertion for it. All three now assert the JVM's values, and `trace_free_boards_are_distinguishable` is the inverted pin |
| `Issue143-rpi_splitter.dsn` after 1 connection vs after 2 — connection 2 **fails**, inserting nothing, so the two boards carry the same 38 items with the same 38 ids and the same geometry | different hashes (quirk #200: the failed attempt burned ids and filled `DrillItem.center`, a **non-transient** lazy cache that `serialize(true)` writes; 8 501 bytes against 8 566, first difference at offset 5 487) | the same hash — **the right answer, kept.** Task 3's widening deliberately leaves a pin's `DrillItem.center`, the three `precalculated*` memos and `Item.smallestClearance` out; `run.sh p7t10 … raw` measures what that costs against the jar |

Both are pinned by named tests rather than left as prose. The probe's `POOL_K`
still leaves `k = 2` and `k = 7` out of the transcript, and **Task 3 re-checked
whether they can come back: they cannot.** The gap is quirk #200, not the
trace-free collision — those two boards differ in the jar only because a failed
pass filled more pin centres — and since the widening deliberately does not
reproduce that, the port still calls them equal. `POOL_K` stays as Task 2 left it,
and the probe's comment now says so.

### `p7t10` and the `structural_hash` audit (controller ruling AH)

`Board::structural_hash` is the port's stand-in for `BasicBoard.getHash()`, which is
an **MD5 hex string over `serialize(true)`** — `board.getTraces()`,
`board.getVias()` **and `board.itemList`**, the three `writeObject` calls at
`BoardSnapshotManager.java:31-33` — i.e. the whole item graph, not the traces its own
javadoc claims (**quirk #201**). Ruling AH: do not reproduce the bytes or the digest;
cover the field set serialization covers, and prove **decision** parity at the three
sites where Java compares two hashes.

**What that digest reaches, stated in two halves, because the obvious stopping rule
is wrong.** `Item.board` is `transient` (Item.java:45), so the stream does not drag
in `BasicBoard`, `board.components`, `board.rules` or `board.library.packages`, and
no item field is typed `Component`, `BoardRules`, `Net` or `Package` either — a
`Pin` knows its component only as an `int`. But the closure escapes the item graph
**once**: `Via.padstack` is non-`transient` (Via.java:48), `Padstack` is
`Serializable` (Padstack.java:16) and holds `padstackList` (:33), so on any board
carrying a via the digest also writes the **whole padstack library** and the
`LayerStructure` under it (Padstacks.java:10,13,16; LayerStructure.java:6,8). The
audit table covers that subgraph *by reduction* to the port's `PadstackId`, with the
argument spelled out there; it is also where the fourth skipped `#200`-shaped cache
lives.

**The audit table lives in `crates/fr-board/src/board/snapshot.rs`'s module doc**,
one row per field `serialize(true)` reaches, each naming the port field that carries
it and the test in `crates/fr-board/tests/snapshot.rs` that pins it. It is not
duplicated here, because a second copy would rot; what belongs here is the summary
and the five rows that are **skipped**, which are the only judgement calls in it:

| skipped Java field | why it is out |
|---|---|
| `DrillItem.center` for a **pin** (DrillItem.java:28), and `precalculatedMinWidth`/`…FirstLayer`/`…LastLayer` (:34-46) | non-`transient`, filled **on demand** — quirk #200. Reproducing them would make a membership test depend on how often the board has been measured. A pin's centre is a pure function of its `componentId` and `pinIndex`, both covered; the memos are pure functions of the padstack, also covered. A **via**'s centre *is* covered: `Via`'s constructor sets it (Via.java:65) |
| `Padstack.cachedDrillRadius` (Padstack.java:44) | `private Double`, not `transient`, filled lazily by `getDrillRadius` (:99, :110) — the same shape again, and in principle the worst of them, because a `Padstack` is **shared**: filling it would move the digest of every board holding a via on it. It memoises a regex parse of the padstack's `final` `name`, which the reduction already covers. Unlike the others it needs no neutralising in `p7t10`: its headless readers all sit behind `calculateTreeShapes(DrillItem)` gated on `getHoleClearance() > 0`, so it is either never filled or filled while the DSN reader inserts the pins — before the run's first `getHash()`, and constant after |
| `IntOctagon.precalculatedToSimplex` (IntOctagon.java:54) | `private Simplex`, not `transient`, filled on the first `toSimplex()` (:560-568) — the **only** non-`transient` lazy cache in `geometry/planar`. Reachable from every `relativeArea`, `BoardOutline.shapes` and `Padstack.shapes` row. It is a pure function of the octagon's eight `final` `int` bounds, all of which the fold already hashes through `TileShape`'s derived `Hash` |
| `Item.smallestClearance` (Item.java:47) | `public double`, not `transient`, but `Item.clearanceViolations` (:451-453) only ever **lowers** it, so its value records how many DRC passes have run, not what the item is. `BoardHistory.add` runs one (`BoardHistory.java:198`) *after* taking the entry's hash at `:197`, so hashing it would make every board differ from the history entry it came from |
| `UndoableObjects`' `stackLevel`, `deletedObjectsStack`, `redoPossible` and every `UndoableObjectNode.level` | this port has no undo stack (`generateSnapshot`/`popSnapshot`/`undo`/`redo` are `not ported:` on `fr-board`'s `board/mod.rs`; a `board.clone()` stands in). The one headless caller that moves them is `BatchOptimizer.optRouteItem`, which brackets one item with `generateSnapshot()` (`:444`) and either `popSnapshot()` (`:503`) or `undo(null)` (`:508`) — **balanced**, with no `getHash()` call inside the window — so they are 0 at every comparison the pipeline makes |

One `covered` row carries a caveat, recorded rather than worked around:
**`ComponentOutline`'s area**. That variant is the only one whose *relative* area
`fr-board` does not expose — only the memoised absolute form — and the transform
behind it reads one board-level input, `components.flipStyleRotateFirst`, that
`serialize(true)` cannot reach (`Item.board` is `transient`). So the port's hash is
slightly **more** sensitive than the jar's here. It cannot move a decision: the flag
is set once when the board is built, and every `getHash` comparison the pipeline
makes is between two boards of one run.

Two things the widening **added** beyond "the other seven item kinds" are worth
naming, because both are cases where the old hash could not tell two boards apart
that Java can:

* **a trace hashes its `Polyline`'s `Line`s, not its corners.** Java serializes every
  `Line.a`/`Line.b` (Line.java:12-15 — the plan's table said `a`,`b`,`c`, which HEAD
  does not have), and two polylines can share their corners while their defining end
  points differ. Pinned by
  `two_polylines_with_equal_corners_but_different_lines_hash_differently`. The fold
  goes through `Line`'s `Hash`, which is `a`/`b` only, so **plan-6 ruling AE's
  identity token cannot reach it** — pinned by
  `the_line_identity_token_does_not_reach_the_hash`;
* **the walk is descending item id** (quirk #63), which is the order
  `itemList.startReadObject()` produces and therefore the order Java's byte stream is
  written in, and the fold is not commutative — pinned by
  `reordering_the_item_list_changes_the_hash`.

`fr-geometry`'s `Area`, `Shape` and `Vector` implement no `Hash` (they hold `f64`s
and shapes whose `Eq` is not derived) and Plan 7 makes no `fr-geometry` API change,
so `snapshot.rs` walks them structurally through their public accessors instead —
`TileShape` and `Circle` *are* `Hash`, `PolygonShape` exposes `corners()`,
`PolylineArea` exposes `get_border()`/`get_holes()`. The one value with neither is a
`Vector::Rational` (`BigInt` coordinates), which falls back to its derived `Debug`
written straight into the hasher by a zero-allocation `std::fmt::Write` adapter; no
corpus board has one. The first implementation used that `Debug` path for *all*
geometry: complete, but 4.4 ms per call on `tutorial_board.dsn` against the
structural walk's 0.58 ms.

**The decision-parity evidence is `scripts/differential/run.sh p7t10`.** It drives a
scripted mutation sequence (insert trace, remove trace, insert via, move via, insert
obstacle, re-fix an item, restore an earlier snapshot, no-op) over a corpus board and
prints, after every step, exactly three lines — `FANOUTSTOP` (`BatchFanout.java:152-156`),
`CONTAINS` (`BoardHistory.java:88-101`) and `RANK` (`:173-186`) — **decisions, never
hash values**. Its `warm` mode (the default) puts the by-product fields above into a
canonical state on the Java side first; its `raw` mode does not, and its diffs are
therefore a *measurement of quirk #200's exposure in the jar*, not a port bug.

**Result: 0 decision diffs on 10 runs over 8 DSNs × 2 000 steps** — `empty_board`,
`Issue143-rpi_splitter` (unrouted and routed), `Issue026-J2_reference` (unrouted and
whole-board), `Issue159-setonix_2hp-pcb`, `Issue649-kicad_ecc83`,
`Issue508-DAC2020_bm01` and `tutorial_board` (unrouted and routed) — and identical
output across `P7T10_HASH_MODE=0..4`. The `raw` runs diverge on 350 of 6 000 decision
lines (`Issue143-rpi_splitter`) and 187 of 6 000 (`Issue026-J2_reference`), always
with Java saying "not in the history" for a board it holds.

## `optChangedArea` and the `ConnectionToPin` trio (Plan 7 Task 5)

Plan 7 Task 5 lands the batch entry point above the tightener family and the two
`PolylineTrace` methods `PolylineTrace.pullTight:841-861` drives.

| Java | lines | Rust |
|---|---|---|
| `board/facade/RoutingBoard.optChangedArea` (both overloads) | `:151-161`, `:171-190` | `RoutingBoardExt::{opt_changed_area, opt_changed_area_with_keep_point}` |
| `board/facade/RoutingBoardOperations.optChangedArea` | `:52-79` | the body of `opt_changed_area_with_keep_point` |
| `board/optimize/TraceTightener.optChangedArea(ExpansionCostFactor[])` | `:121-169` | `TraceTightener::opt_changed_area` |
| `board/trace/PolylineTrace.checkConnectionToPin` | `:1013-1076` | `PolylineTraceExt::check_connection_to_pin` |
| `board/trace/PolylineTrace.correctConnectionToPin` | `:1082-1245` | `PolylineTraceExt::correct_connection_to_pin` |
| `board/trace/PolylineTrace.swapConnectionToPin` | `:1252-1313` | `PolylineTraceExt::swap_connection_to_pin` |

**`checkConnectionToPin` was not already ported.** The plan's scan ruling 5
records it as landed in Plan 6 and tells Task 5 to reuse it; it had not landed. A
workspace search for the name, for `TraceExitRestriction` anywhere in
`fr-router`, and for the method's body found only the two forward-deferral
markers in `crates/fr-board/src/items/trace.rs` (both now `// renamed:` lines
pointing at this crate). Java wins over plan text, so the
64 lines are transcribed with the pair that needs them —
`correctConnectionToPin`'s first statement is a call to it (`:1083`), and the
method is unimplementable without it. It is a **trait method** on
`PolylineTraceExt` rather than a private free function, because the pair's
callers reach it through the trait.

**One object per layer per outer iteration is not what the sweep does.** The two
trace arms of `:150-159` both `break`, but not on the same condition: the
`pullTight` arm breaks only when `splitTracesAtKeepPoint()` answers `true`
(`:153-155`), while the `smoothenEndCornersAtTrace` arm breaks unconditionally
(`:156-158`, "because items may be removed"). With no keep point — which is every
router caller — the split is a no-op answering `false`, so a layer whose objects
all merely *tighten* is walked to the end in one pass. The plan's prose folded the
two arms into one unconditional break; the port follows the source, and
`crates/fr-router/tests/opt_changed_area.rs`
`the_item_loop_does_not_break_after_a_plain_pull_tight` pins it.

**`:136` empties the layer before the work, not after.** The tightener that
follows re-marks whatever it moves (through `PolylineTrace.change`), so the outer
`while (somethingChanged)` sees the *new* region. A port that emptied the layer
afterwards would wipe those marks and end a pass early.

**`:78` nulls `board.changedArea`, and that is load-bearing.** The next
`startMarkingChangedArea` re-creates the store; leaving this one in place makes
the following sweep see a stale region.

**The `ViaOptimizer` arm is complete as of Task 7.** `:160-165` calls
`ViaOptimizer::opt_via_location` for real, and Task 7's three `repositionVia`
overloads closed the last gap under it, so `p7t3` **mode 4** (vias offered to the
optimiser) is 0 diffs on all three boards and lives inside
`opt_changed_area.rs`'s `the_whole_sweep_matches_the_jvm_on_a_real_board` loop.
Task 6's `#[should_panic]` sentinel is deleted. No `engine` is threaded into
`opt_via_location`: none of the three things it calls — `DrillItemMover::insert`,
`DrillItemMover::check`, `PolylineTraceExt::pull_tight` — takes one, because Java's
`DrillItemMover` has no such parameter and Java's three-argument `pullTight`
reads `board.autorouteEngine` internally (the port fixed that as `None` in Task 5).

**The budget is controller ruling AI's knob and nothing more.** `:147`'s
`isStopRequested()` reads both the `Stoppable` and the `TimeLimit` that
`TraceTightener`'s constructor builds when `timeLimit > 0`
(`TraceTightener.java:73-77`), and `RouterBudget::opt_changed_area_ms` — default
1000, Java's own `TIME_LIMIT_TO_PREVENT_ENDLESS_LOOP` — supplies it. A trip
returns with the rest of the board untightened *and* with `changedArea` already
emptied for every layer walked so far, so those regions are lost; that is Java's
control flow, and it is why every parity run passes `RouterBudget::disabled()`
(`0`, Java's own "no limit"). `the_budget_trips_the_sweep` is the Rust-only test
that proves the knob works; `a_tripped_stop_check_returns_mid_sweep_leaving_the_rest_untightened`
is the parity hazard, asserted as a fact.

**Ruling AJ.** The two `additionalUpdateAfterChange` sites this task's methods
reach *directly* — `correctConnectionToPin:1237`'s `change` and
`swapConnectionToPin:1313`'s `combine()` — are threaded exactly as
`pull_tight_with_engine` threads its own (Task 15b's contract). The sites
**inside** `fr-board` that `insertTrace` and `combineTrace` reach are rostered
`// not reachable:` per the ruling and are not wired.

**Two quirks.** #204 (`optChangedArea`'s clip-shape guard is a reference
comparison against the `IntOctagon.EMPTY` singleton — ruling 9's site) and #205
(`check`/`correctConnectionToPin` accept `pinEdgeToTurnDist == 0` while their only
caller demands `> 0`, so that band is dead acceptance).

**Where the numbers come from.** `scripts/differential/java/P7T3.java` (board
level: three real boards × modes 0-4, 0 diffs — mode 4 is the `ViaOptimizer`
one, closed by Task 7) and `scripts/differential/java/P7T6.java` (five modes, 0 diffs).
Both transcripts are committed —
`tests/data/p7t3-opt-changed-area.txt` and
`tests/data/p7t6-connection-to-pin.txt` — and replayed row by row by
`tests/opt_changed_area.rs` and `tests/connection_to_pin.rs`.

## `ViaOptimizer` (Plan 7 Tasks 6 and 7)

`board/optimize/ViaOptimizer.java` is 733 lines in three layers: an entry pair
(`optViaLocation:33-158`, `optPlaneOrFanoutVia:161-296`), three `repositionVia`
overloads (`:302-365`, `:367-429`, `:434-713`) and one predicate
(`isWithinTolerance:719-732`). **Task 6 ported the entry pair and the predicate;
Task 7 ported the three overloads.** The class is `board_ext/via_optimizer.rs`,
and `scripts/audit-map/fr-router.map`'s `ViaOptimizer` row moved from `lib.rs` to
it in Task 6, as plan ruling 13 says it should.

Java overloads on the arity and types of the argument list, so the port renames
the three:

| Java | port | who calls it |
|---|---|---|
| `repositionVia(board, via, IntPoint, int, int, int)` `:302-365` | `reposition_via_toward_location` | `optPlaneOrFanoutVia:216-217`, plus eight call expressions inside overload C |
| `repositionVia(board, via, IntPoint, int, int, int, IntPoint, int, int, int)` `:367-429` | `reposition_via_check_candidate` | **only** overload C's four decomposition arms (`:599`, `:627`, `:665`, `:696`) |
| `repositionVia(board, via, int, int, int, ExpansionCostFactor, Point, int, int, int, ExpansionCostFactor, Point)` `:434-713` | `reposition_via_general` | `optViaLocation:118-131` |

**The dispatch, corrected twice.** The plan's first draft placed it at `:100-140`
(that range is corner/tolerance computation) and called overload A "the
two-contact case". It is at **`:46-78`**, and overload A is the **one**-contact /
plane-or-fanout arm; overload C is the two-trace one. `p7t4`'s `classify` is a
read-only replica of `:39-106` on both sides, so a port that reached the wrong
overload would be a diff even where both answers agree.

### The three overloads mutate nothing

`checkTraceSegment` and `DrillItemMover.check` are read-only probes, and both of
`check`'s recursion depths are **zero** at all **four** of its call sites in the
class — `ViaOptimizer.java:244`, `:338`, `:358` and `:428`, of which the last three
are the ones inside the overloads (`via_optimizer.rs:364, 551, 574, 664`) — so no
shove is attempted and no item is inserted. *(An earlier draft of this section, of
report §2 and of Task 7's commit message said "nine call sites". Nine is the number
of call expressions into **overload A** — one from `optPlaneOrFanoutVia:216-217` plus
eight inside overload C — which is what `overload_b_is_reached_only_from_c` asserts;
the two counts were crossed.)* That is why `p7t4` modes 3-5 may call an
overload 27 times per via and still end on the board the routing prologue built,
and why each of the three modes prints that board afterwards: an inserted item or
a burned id would show as a `maxId=`/`item id=` diff in the dump. It is also why
Task 6 could stub overload C inertly — `optViaLocation:132-134` turns a `null`
into `return false` with nothing mutated.

`tests/via_optimizer_reposition.rs`'s
`the_general_case_leaves_the_board_untouched_when_no_candidate_improves` makes
the same statement locally, with `structural_hash`, for every via x every cost
pair, successes included.

### The candidate order: no scoring pass, and a tie is not a candidate

The plan's transcription note warned that "where two candidates tie, Java keeps
the **first** found in contact order; the port must not use a `max_by` that keeps
the last". **There is no `max_by` and no scoring pass anywhere in overload C.**
It is a sequence of *gated attempts*, each returning the moment it succeeds:

1. `:462-480` — the **overlapping-lines** arm (`sideOf == COLLINEAR` and a
   positive scalar product). It runs before every cost gate and **returns
   unconditionally**, `null` included, so the six later families are skipped
   whenever it fires. Note the crossed parameters: moving toward the *first*
   from-corner is probed with the *second* trace's half width, layer and
   clearance class, because it is the second trace that must be re-routed to the
   new via location. Every later arm crosses them the same way.
2. `:485-526` — two **weighted-distance** attempts, each asking "is this
   from-corner cheaper to reach under the *other* layer's costs?" (Java measures
   both distances to the same point under two cost pairs; that is deliberate, not
   a copy-paste slip).
3. `:528-578` — the **acute-angle** case, skipped under `NINETY_DEGREE`: shorten
   the longer leg to the shorter one's length, then try both endpoints, cheaper
   first, and take whichever answers.
4. `:581-711` — **decomposition into axis-parallel parts**, two attempts per
   non-orthogonal delta, each asking overload B whether the L-shaped detour is
   clear on *both* legs.

Every gate is a strict `>`, so equal weighted distances **skip the arm
entirely**. `Issue026-J2_reference`'s via 231 shows it end to end: with
`costs1 == costs2` overload C answers `null`; on the same board and the same
geometry it answers `(1228467,-826441)` under `(1.0, 2.0)`/`(2.0, 1.0)` and
`(1231088,-829062)` under `(2.0, 1.0)`/`(1.0, 2.0)`. Three answers, decided only
by the gates. `a_candidate_tie_keeps_the_first_in_contact_order` pins all three,
and recomputes the two gate expressions from `FloatPoint::weighted_distance` so
the strictness is observed rather than inferred.

### The Task 6 -> Task 7 sentinels, and how they flipped

Task 6 could not answer for overload A, and controller ruling B1 said a stubbed
arm must be **inert or loud**. The two stubs were not alike:

* overload **C** (`optViaLocation:118-131`) could answer `None` — Java's
  `:132-134` mutates nothing, so the port produced a board Java itself produces;
* overload **A** (`optPlaneOrFanoutVia:216-217`) could not. A `null` there falls
  through to the `:218-260` projection branch, which **inserts** at `:282`, and
  which Java reaches only when *its* overload A answered `null`. Measured on
  `Issue143-rpi_splitter` at `routeK = 12`, via 84: centre `(977281,2968339)`,
  Java `(1016000,3119161)`, `None`-stubbed port `(1016000,2968339)` — a board
  Java never produces. So it was an `unimplemented!`, with
  `ViaOptimizer::reaches_task_seven_guard` and the `TASK7_GUARD` rows both halves
  of `p7t4` printed so that no committed transcript row recorded a port-only
  move.

Task 7 deleted the guard, the predicate and the `TASK7_GUARD` rows, and flipped
every sentinel:

| sentinel | Task 6 | Task 7 |
|---|---|---|
| `opt_changed_area.rs` `mode_four_is_task_sevens_obligation` | `#[should_panic(expected = "repositionVia overload A")]` | **deleted**; mode 4 is inside `the_whole_sweep_matches_the_jvm_on_a_real_board`'s loop |
| `via_optimizer.rs` `the_only_divergence_is_repositionvia` | eight via ids named as expected divergences | **equality pin**: those same eight rows are asserted identical |
| `via_optimizer.rs` `a_plane_via_reaches_task_sevens_guard` | both entry points must panic | `a_plane_via_moves_through_overload_a`: via 187 -> `(932812,1011224)`, via 84 -> `(1016000,3119161)` |
| `via_optimizer.rs` `the_matching_runs_match_the_jvm_row_for_row` | seven of nine board sections | **all nine** |
| `via_optimizer.rs` `the_overload_dispatch_matches_javas_contact_counts` | the first via row of each run | **every** via row |
| `ViaOptimizer::reaches_task_seven_guard`, `P7T4.reachesOverloadA`, `takesPlaneArm` | the ruling-B1 guard | **deleted from both halves**, transcript regenerated |
| `tightener/mod.rs` `obligation: ViaOptimizer.repositionVia — Task 7` | open | **discharged** |

Via 84 is worth a second look: overload A answers `(1016000,3007058)`, which
**is** the check corner, so `optPlaneOrFanoutVia:292-294`'s
`newViaLocation.equals(checkCorner)` fires and the method recurses once more,
landing at `(1016000,3119161)`. The two numbers are one call apart, and
`a_one_contact_via_takes_overload_a` pins both.

### The measured evidence

`scripts/differential/run.sh p7t4 <dsn> <mode>` drives the class over **every
via** of a board the `P6T1` machinery actually routed (12 connections). Modes 0,
1 and 6 drive the entry pair and print, per via, the descending contact ids, the
dispatch class, the answer and the centre before and after, then the whole board.
Mode 2 drives `isWithinTolerance` over 10 256 scripted triples with no board.
**Modes 3, 4 and 5 are Task 7's, one per overload, each driven directly** through
`setAccessible` on the Java side: 27 scripted `toLocation`s per via for A, 11
`toLocation`s x 2 role assignments for B, and five cost-pair combinations for C
with the arguments `optViaLocation:118-131` builds. Mode 6 is mode 0 with
`traceCosts = null`; it is numbered **6, not 3**, because Task 6 reserved 3/4/5
for exactly this.

| fixture | vias | 0 | 1 | 2 | 3 (overload A) | 4 (overload B) | 5 (overload C) | 6 |
|---|---|---|---|---|---|---|---|---|
| `Issue649-kicad_ecc83-pp_input_board_v1` | 0 | **MATCH** (380) | **MATCH** | **MATCH** (10 769) | **MATCH** | **MATCH** | **MATCH** | **MATCH** |
| `Issue143-rpi_splitter` | 6 (2 one-contact, 4 `TWO_TRACES`) | **MATCH** (69) | **MATCH** (69) | **MATCH** | **MATCH** (224) | **MATCH** (194) | **MATCH** (84) | **MATCH** (69) |
| `Issue026-J2_reference` | 6, all `TWO_TRACES` | **MATCH** (125) | **MATCH** (124) | **MATCH** | **MATCH** (280) | **MATCH** (250) | **MATCH** (148) | **MATCH** (125) |

**21 of 21 MATCH** — Task 6 stood at 10 of 12 with every DIFF a `repositionVia`.
The five remaining corpus stems were run too, modes 0/3/4/5:

| stem | 0 | 3 | 4 | 5 |
|---|---|---|---|---|
| `tutorial_board` | **MATCH** (443) | **MATCH** (443) | **MATCH** (443) | **MATCH** (443) |
| `Issue103-Board-Unrouted` | **MATCH** (1 890) | **MATCH** (2 046) | **MATCH** (2 016) | **MATCH** (1 914) |
| `Issue413-test` | **MATCH** (47) | **MATCH** (151) | **MATCH** (131) | **MATCH** (63) |
| `Issue110-RelayModule` | **MATCH** (955) | **MATCH** (1 267) | **MATCH** (1 207) | **MATCH** (955) |
| `Issue753-CPU-85_r104` | **MATCH** (2 450) | **MATCH** (2 918) | **MATCH** (2 828) | **MATCH** (2 450) |

And `p7t3` — the `optChangedArea` sweep that *calls* the class — is **15 of 15
MATCH** across its five modes and three boards, mode 4 (vias offered to the
optimiser) included. That is the discharge of Task 5's `obligation:` marker.

### Two things the plan got wrong, and Java won

**`RoutingBoard.moveDrillItem` is not in this task's path at all.** Scan ruling 3
folded it in on the premise that "both `optViaLocation` and `optPlaneOrFanoutVia`
move vias through it". They do not: both call `DrillItemMover.insert(via, delta,
9, 9, null, board)` and `DrillItemMover.check(...)` **directly**
(`ViaOptimizer.java:136`, `:244`, `:282`), and Plan 6 Task 10b landed both. A
fresh `grep -rn moveDrillItem src/main/java src/test` finds two hits: the
declaration, and `board/actions/MoveComponent.java:156`, whose own only caller is
`gui/interactive/DragItemState.java:56-61` — a mouse drag. Under Plan 7's "No GUI"
constraint it is therefore `not ported:` with that evidence, at
`crates/fr-board/src/board/mod.rs`, rather than 44 lines nothing calls. The
ruling's own escape hatch anticipated this ("if `DrillItemMover` already does the
whole job, say so and skip the 44 lines"); the finding is one step stronger than
the hatch, because the method has no headless caller either.

**`isWithinTolerance` takes two `Point`s, not three `double`s.** The plan's
interface block types it `(value: f64, target: f64, tolerance: f64) -> bool`;
Java's is `(Point p1, Point p2, int tolerance)` with a null guard and a Manhattan
distance. And `optViaLocation`'s two `int` parameters are
`tracePullTightAccuracy` then `maxRecursionDepth` — the plan's
`min_translate_dist` / `accuracy` pair had them the wrong way round. The port
uses Java's names; `TraceTightener.optChangedArea:161-164` is the call site that
feeds `minTranslateDist` into the *accuracy* slot and the literal `10` into the
depth.

**Three more, from Task 7.** The brief's interface block types overload C as
`Result<bool, BoardError>`; Java returns a nullable `Point`, and none of the three
overloads can fail, so the port's signatures are `Option<Point>`, `bool` and
`Option<Point>`. The dispatch note said "overload C recurses (`:292-294`)";
`:292-294` is `optPlaneOrFanoutVia`'s recursion, which Task 6 already landed —
**overload C does not recurse into itself**, it calls A and B. And the note said
"overload C inserts/removes vias — every discarded attempt too"; it does not
(see "The three overloads mutate nothing" above), so there is no id-burn order to
reproduce beyond the entry pair's, which Task 6 pinned.

### Five `pub seam:` markers, and what they do to Task 17's count

`ViaOptimizer::{opt_plane_or_fanout_via, is_within_tolerance,
reposition_via_toward_location, reposition_via_check_candidate,
reposition_via_general}` are `private` in Java and `pub` here — an integration
test and a `scripts/differential` binary are both *outside* the crate, so
`pub(crate)` cannot reach them and the Java twin pays the same price with
`setAccessible`. All five carry a `// pub seam:` line (Plan 6 finding S7).
Task 6's sixth, `reaches_task_seven_guard`, is deleted. **This moves a counted
Task 17 gate**, and Task 17 reconciled it below rather than editing the number.

### The `pub seam:` gate, reconciled (Plan 7 Task 17)

The plan's checklist expects `grep -rn "pub seam:" crates/fr-router/src` to return
**six**: Plan 6 closed with eight, and scan ruling 4 predicted Tasks 11 and 9/10
would close two of them. **Measured on the committed tree it returns 14.** Here is
every marker and why, so the number is a record rather than a surprise. (The
whole-workspace `grep -rn "pub seam:" crates/` returns **19**; the other five are
this README's own prose about the convention.)

| # | site | what it is | Plan 6 → now |
|---|---|---|---|
| 1 | `autoroute/expansion/mod.rs:297` `incomplete_list_created` | the port's reader for `incompleteExpansionRooms == null` (quirk #169) | one of Plan 6's six pure-surface-fidelity seams — **unchanged** |
| 2 | `autoroute/expansion/complete_room.rs:88` `tree_leaf` | read half of `setSearchTreeEntries` | ditto |
| 3 | `autoroute/expansion/complete_room.rs:96` `room_id` | the arena index, not Java's `getId` | ditto |
| 4 | `autoroute/expansion/complete_room.rs:144` `tree_shape_count` | Java's caller is `ShapeTree.insert`; the store short-circuits it | ditto |
| 5 | `autoroute/drill/page_array.rs:234` `bounds` | a private Java field with no getter | ditto |
| 6 | `board_ext/tightener/tightener_45.rs:29` `get_angle_restriction` | "none, in Java or here" | ditto |
| 7 | `autoroute/attempt.rs:133` `is_routed` | scan ruling 4 said Tasks 9/10 would close it | **not closed, and deliberately so** — see below |
| — | `autoroute/maze/control.rs` `AutorouteControl::from_settings` | scan ruling 4's other prediction | **closed by Task 11**: the fanout pre-pass is the caller, and the marker is gone |
| 8-9 | `pipeline/stop.rs:70`, `:171` | **new, Task 4**: controller ruling AP puts Plan 8's `CancelToken` (`Arc<AtomicBool>`) at these poll sites. The module introduces no atomics — the state is a `Cell`, because the pipeline is single-threaded — so the seam is where the token joins, and it has to be written down or Plan 8 will fold it into `RouterStop` and lose the three-state distinction | added |
| 10-14 | `board_ext/via_optimizer.rs:232, 453, 504, 613, 714` | **new, Tasks 6 and 7**: `optPlaneOrFanoutVia`, `isWithinTolerance` and the three `repositionVia` overloads are `private` in Java and `pub` here, because `crates/fr-router/tests/via_optimizer*.rs` and `scripts/differential/rust/src/bin/p7t4.rs` are both *outside* the crate and `pub(crate)` cannot reach them. The Java twin pays the same price with `setAccessible` | added |

So: **8 − 1 + 7 = 14**, and the plan's "six" was arithmetic on a prediction that
was half right. The two additions are both *good* news — a seam that is written
down is a seam Plan 8 cannot mistake for an accident — and neither is a `pub` item
with no reason.

**Why `is_routed` stays open, measured.** `AutorouteAttemptResult` has no
`isRouted()` in Java; `state` is a public field and every caller compares it
directly. The port does the same. Two greps, both reproducible on the committed
tree — and both stated *outside* `autoroute/attempt.rs`, because the marker's own
text mentions the strings it greps for:

```sh
grep -rno "AutorouteAttemptState::" crates/fr-router/src | grep -v autoroute/attempt.rs
#   -> 30 hits in five files: autoroute/maze/engine.rs 15,
#      board_ext/routing_board_ext.rs 6, pipeline/fanout.rs 4,
#      pipeline/pass_runner.rs 4, pipeline/batch_autorouter.rs 1
grep -rn "is_routed()" crates/fr-router/src crates/fr-router/tests | grep -v autoroute/attempt.rs
#   -> nothing
```

Every one of the 30 is a `match` arm or an `==` against the enum, transcribing
Java's `result.state == ROUTED` chains; and the accessor has **zero production
callers**, in the crate or its tests. Routing those through a helper would be the
port inventing a shape Java does not have, so the accessor keeps its own-file unit
test as its only caller and its marker says so; Plan 8's manifest layer is free to
use it.

*(An earlier draft of both this paragraph and the marker said "13 sites" across a
list that included `AutorouteBatchLoop`. Both halves were wrong — the narrower
`::Routed` grep returns 14 including its own citation, and `pipeline/batch_loop.rs`
does not name the enum at all, because the pass loop reads `BatchLoopResult`, not
an attempt result. Corrected in Task 17's fix round.)*

### Two quirks

**#206** — `isWithinTolerance`'s javadoc and `optViaLocation:85-86` both claim it
"matches the logic in `DrillItem.getNormalContacts()`", which matches trace ends
**exactly** (`DrillItem.java:288-290`). So the tolerance never decides whether a
contact is usable; it only gives the `firstCorner`-first test order a chance to
pick the wrong end of a short trace. Reproduced, test order included.

**#207** — no `repositionVia` overload tests the board's trace angle restriction
**against the delta it produces**, yet `optPlaneOrFanoutVia:236-241` — the
fallback reached *only* when overload A answered `null` — refuses a projection
that is not orthogonal under `NINETY_DEGREE` or a multiple of 45 degrees under
`FORTYFIVE_DEGREE`. The same method therefore applies the restriction to one of
its two answers and not the other. Two overloads do *read* the restriction, and
neither reading is a test of the answer: B at `:388-390` (a `NONE`-only refusal
of moves shorter than 1.5) and C at `:528-529` (the acute-angle arm's
`!= NINETY_DEGREE` gate, which picks a family of candidates rather than checking
any candidate's delta). **Latent on the corpus**: overload A walks toward a trace corner, and the
trace already obeys the restriction, so every move the seven stems produce is
orthogonal or exactly diagonal. Reproduced as-is, with the measurement in the
register rather than a claim that the corpus clears it.

## The autoroute pass (Plan 7 Task 9)

`BatchAutorouter.getAutorouteItems` (`:345-409`), `autoroutePass` (`:415-421`),
`AutoroutePassRunner.runSingleThread` (`:151-336`) and `updateProgress` (`:489-516`),
plus three small classes that had been on the roster: `RoutingFailureLog` (161 loc),
`ItemRouteResult` (145 loc, in full) and `AutorouteAirlineCalculator.calculateAirline`.

### The micro-neckdown fanout fallback has a floor (Plan 9 Task 2, R2 — register row #294)

`FoundConnectionInserter.insertFanoutMicroNeckdown` (`:455-523`) retries a failed
2-point fanout insertion at a `LinkedHashSet` of up to five narrower half widths — the
start pin's `getTraceNeckdownHalfwidth(layer)`, the end pin's, then `max(1, base*3/4)`,
`max(1, base*3/5)` and `max(1, base/2)` of the class width — "keeping the same clearance
class". Its loop guard (`:473-476`) is `candidateHalfWidth <= 0 || >= baseHalfWidth` **and
nothing else**: no candidate is ever compared against the board's minimum track width.

`benchmark/reports/java-regressions-2026-09.md` §"Regression 2" measures the cost. On any
board whose net-class width *equals* its minimum width — very common — the fallback emits
sub-minimum traces: `track_width` violations on 31/146 of the report's small-tier boards,
DRC-clean 0.94 → 0.73 small and 0.93 → 0.40 large. Because a clean-pass metric weighs a
violation like an unrouted net, the fallback turns "one net open" into "the board fails
DRC", and on the small tier it recovers **no** connectivity at all. Its benefit is real
only on large boards (fully-connected 0.17 → 0.33).

So the port **guards rather than reverts**: a candidate below
`BoardRules::get_min_trace_half_width()` is skipped. That is the minimum over the declared
net-class widths (`BoardRules.java:94-96`) and **not** `Board::get_min_trace_half_width()`,
which is a running minimum over the traces already inserted and would ratchet itself down
the moment the fallback inserted one narrow trace. A class that already sits at the
minimum skips the fallback entirely and the connection fails honestly; a class above the
minimum still gets its neckdown.

### The work list is airline-sorted (Plan 9 Task 2, R1 — register row #293)

Plan 7 rostered three more `AutorouteAirlineCalculator` methods `// not ported:` on
correct evidence — `calculateItemDistance` (`:162-177`), `calculateMinDistance`
(`:179-202`) and `getItemReferencePoint` (`:204-213`) have **no caller anywhere** in
`src/main` or `src/test`, and `grep -rn calculateItemDistance src/main src/test` still
answers only the declaration. Plan 9 Task 2 ports all three and re-states those three
roster lines as ported rows, because the missing caller is a **measured regression**
rather than dead code: commit `933d2980` deleted
`autorouteItemList.sort(Comparator.comparingDouble(this::calculateItemDistance))` and
`benchmark/reports/java-regressions-2026-09.md` §"Regression 1" shows fully-connected
falling 0.81 → 0.75 at v2.2.0 and never recovering, with the sort restored alone at HEAD
bringing it back to 0.82 *and running slightly faster*.

`BatchAutorouter::autoroute_items_with_handled` therefore returns the list **sorted
ascending by `calculate_item_distance`**, unconditionally and stably: `f64::total_cmp`
is `Double.compare`'s order, and a stable sort leaves ties in the descending-id walk
order (quirk #63) that `getAutorouteItems` built, so the sort is a refinement of the old
order and not a second reordering. `autoroute_items` is the untouched wrapper, and
`AutoroutePassRunner::run_single_thread_body` (Java `:158`) consumes the sorted list
with no change of its own.

**`run.sh p7t1` MISMATCHes on the `ITEM` order by design.** The driver prints one
`ITEM` line per work-list entry in list order; the jar has no sort and the port
restores it, so the two disagree on that seam for as long as register row #293 reads
`fixed: T2`. A `p7t1` diff confined to the *order* of the `ITEM` lines is the fix
working. What must still agree, and what to check before calling such a diff a
defect: the **multiset** of `ITEM` lines (same ids, same multiplicities — quirk
#213's duplicates are membership, not order, and the sort is stable), the `HANDLED`
set, which is built before the sort, and every other line the driver prints. `p7t2`,
`p7t5`, `p7t9` and `p8t1` route a board and inherit the order downstream, so the same
reading applies to them. The two
remaining `// not ported:` rows — `nearestPointOnTrace` (`:42-83`) and
`findClosestPointsBetweenTraces` (`:85-160`) — are **not** resurrected: they are reached
only from each other and from `BatchAutorouterThread`, which has no caller at all.

### Quirk #213: an item enters the work list once per qualifying net

`:390`'s `autorouteItemList.add(currentItem)` sits **inside** the `for (int i = 0; i <
currentItem.netCount(); i++)` loop opened at `:363`, so a two-net item that satisfies
`:375` on both nets enters the `List<Item>` twice. `runSingleThread` then iterates the
list (`:202`) and, for each entry, runs a **fresh** `0..netCount()` walk (`:207`) — so
that item is routed **four** times in one pass, and the net index it is routed on has
no relation to the index that qualified it.

The repeats are not idle: each runs against the board the previous one left, so they
route real connections and rip real traces. The port reproduces it exactly, because
the corpus depends on it. `tests/pass_runner.rs`'s
`a_two_net_item_is_routed_four_times` and
`the_inner_index_is_a_net_index_not_the_qualifying_one` are the pins, and `p7t1`
prints the list in order so a duplicated entry is two `ITEM` lines with one id.
**Since Plan 9 Task 2 that order is the port's, not the jar's** — see the next
section — so read a `p7t1` diff as a multiset comparison: the duplicate is still
two lines with one id, but the lines need not sit where the jar put them.

### Java's list is a `List<Item>`, not a list of `(item, net)` pairs

The plan's first draft said otherwise. The net numbers `AutoroutePassRunner:207` loops
over come from the **item**, read off the live board — which is what makes #213 a bug
rather than a redundancy. The port's `Vec<ItemId>` is the faithful shape.

### A work-list item can never be ripped up during its own pass

`getAutorouteItems:358-359` keeps only items where `!isRoutable()`;
`MazeRipupResolver.checkRipup` (`:72-76`) and its `:205-212` twin refuse to rip any
item where `!isRoutable()`, returning before anything else; and `removeTails`
(`RoutingBoard.java:1197`) applies the same test. The two sets are **disjoint**. That
is what lets the port read `netCount()`/`getNetNumber(i)` off the live board where
Java reads them off an `Item` object that would survive removal from `itemList`.

### Two stop predicates in one method

`:203` and `:208` read `isStopAutoRouterRequested()` (`!= NONE`); the connection
itself and `removeTails` are handed `router.thread` and poll
`Stoppable.isStopRequested()` (`== ALL`, `AutorouteEngine.java:294-303`,
`TraceTightener.java:195-196`). So an `AUTO_ROUTER_ONLY` stop ends the pass *between*
connections but does not abort one already in flight; `maxItems`' `ALL` does both.

### Three `fireBoardUpdatedEvent` calls, one of them throttled

`:196` and `:321` are **ungated**; only `updateProgress`'s (`:507`) goes through
`shouldFireBoardUpdate`. All three become `RoutingEvent::BoardUpdated`.

### `RoutingFailureLog` is a parameter here and a board field in Java

`RoutingBoard.java:64` declares it `final` and `:91` constructs it; the port lifts it
to a caller-owned parameter of `run_single_thread`, because `fr-board` must not depend
on `fr-router`. The `Vec<String>` hook that stood in for the field is **deleted** — it
had no reader and no writer — and `crates/fr-board/src/board/mod.rs` carries the
`// renamed:` that says so.

### `ItemRouteResult` is ported whole, bug included (quirk #212)

`:63`'s `viaCountAfter / viaCountBefore` is an `int` division, so the via term
truncates while the trace term beside it is a `double`.
`BatchOptimizer.java:340-348` recomputes the same expression with a `(float)` cast and
gets the right answer, so the *field* is wrong and the *used* value is right; both are
ported (the second in Task 14). Pinned by `P7T9Probe`'s 500 scripted tuples.


## The pass loop (Plan 7 Task 10)

`AutorouteBatchLoop.run` (`:37-588`) — the 552-line method that decides how many
passes run, which board survives them, and when to give up — plus its five private
one-line delegates (`:590-608`), which the port takes by inlining because each
forwards to a `BatchAutorouter` member `run` can call directly.

It is `fr_router::pipeline::AutorouteBatchLoop::run`, and it answers a
`BatchLoopResult`: the `TaskState` (`:571-585`), Java's own return value (`:587`),
`currentPass` (`:520-522`) and ruling 1(a)'s per-pass `PassRecord` list. The board is
**not** in the result — Java's `:552` writes `job.board`, and the port's `&mut Board`
argument is that field. Two arms replace it wholesale with an older board out of
`BoardHistory`: the mid-loop restore (`:322`) and the final swap (`:535`).

`scripts/differential/run.sh p7t9 <dsn> <maxPasses> router-only` is the whole-board
evidence — a line-for-line transcription of `run`'s body beside the real
`runBatchLoop()`, with the final board in `P6T15aProbe`'s polyline format. **13 / 13
MATCH**: five corpus DSNs at `maxPasses in {1, 2}` and the three small stems at 8.

### Quirk #214: a normal end of routing reports `CANCELLED`

`:571` reports `FINISHED` only when the stop flag is still `NONE`, and **every**
ordinary exit raises it first — `maxPasses` (`:271`), "not able to improve" (`:311`),
the rank limit (`:318`) and both stagnation windows (`:474`, `:505`). The only path
that leaves it `NONE` is the `while` head's own `continueAutorouting == false`, i.e. a
pass that routed and failed nothing. So the CLI's normal case — route until
`--max-passes` — reports `CANCELLED`, and an API consumer cannot tell it from a user
cancellation. Measured: `p7t9 <rpi> 1` prints `RESULT state=CANCELLED` on a board whose
score is 799.98.

### Quirk #215: the fully-routed counter reset is on the wrong arm

`:509-517` is the `else` of `:422`'s `currentPass >= 8 && continueAutorouting`, so it
fires on passes 1-7 and never afterwards. Its own comment (`:511-514`) describes the
opposite rule. Since `:429`'s increment is itself inside the `>= 8` arm, the reset
protects a counter that is always zero and stops protecting it exactly when it can
start climbing — the counter first reaches 10 at pass 17, nine passes after the reset
went out of reach. `p7t9 <ecc83> 8` prints `ROUTED-RESET pass=1` and `pass=2`, nothing
later.

### Quirk #217: the board-rank break cannot fire

`:317` tests `boardToRestoreRank > BOARD_RANK_LIMIT`, and `BOARD_RANK_LIMIT` **is**
`BoardHistory.MAX_HISTORY_SIZE` (`BatchAutorouter.java:40`). `getRank` answers a
1-indexed position in a list `add` caps at that same number, so its range is
`{-1} ∪ 1..=30` and the test has no solution. The declaration's comment says the limit
"Must not exceed `BoardHistory.MAX_HISTORY_SIZE` so the check can actually fire" — for
it to fire the limit must be *strictly less than* the cap. One of the loop's five stop
reasons is therefore unreachable. Found while trying to write the brief's
`the_rank_limit_breaks_the_loop`, which cannot be written.

### Four arms lifted out of `run`, and why

`restore_gate` (`:298-300`), `rank_limit_exceeded` (`:317`), `stagnation_guard`
(`:422`) and `final_best_board_swap` (`:525-550`) are `pub fn`s called from exactly one
place each. Every one of them is either unreachable on the corpus (the last two) or
reachable only after eight real passes (the first two), so a test that had to route
eight passes to observe a boolean would be a slow test of the router rather than a test
of the loop. Nothing else moved: `run` reads as Java does with four names substituted
for four expressions.

### Two stubs, one loud and one inert (ruling B1) — the loud one is discharged

The **fanout pre-pass** (`:89-173`) was Task 12's, and skipping it on a board with SMD
pins would have answered a different board — so Task 10's `run` **asserted**
`!settings.is_fanout_enabled()`, as a real `assert!` rather than a `debug_assert!`,
because the parity runs are release builds. **Plan 7 Task 12 removed it** and put
`BatchFanout::fanout_board` in its place, with `:90-91`'s empty-SMD-pin skip, `:173`'s
`router.fanoutTimedOut = summary.isTimedOut()` and the new `BatchLoopResult::fanout`
field; `p7t9 <dsn> 1 router+fanout` is the whole-board evidence and
`tests/batch_loop.rs`'s `routing_with_fanout_enabled_runs_the_pre_pass` is the unit one
— the same test that used to assert the panic.

The **stagnation report** (`:456-476`, `:486-507`) is Task 15's and is still **inert**:
it is a log payload, both arms are live on a long run, and the
`requestStopAutoRouter(); break;` around it is complete here.

The one-shot **fanout recovery** (`:435-454`) changes status with the same commit: it was
unreachable while `is_fanout_enabled()` was asserted false, and it is now on a live path.
Its four-term guard is lifted out as `fanout_recovery_fires` and pinned term by term
(`tests/fanout.rs`), because firing it for real needs eight passes of a stagnating board
with fanout on, which no Plan 7 fixture produces.

## The fanout stage (Plan 7 Tasks 11 and 12)

`src/pipeline/fanout.rs` is the whole of `BatchFanout`. Task 11 landed the type, its
constructor and the `FanoutComponent` / `FanoutPin` pair it builds, plus
`RoutingBoardExt::fanout`, the per-pin escape router the loops call. **Task 12 landed
the loops**: `BatchFanout::fanout_board` (`:81-163`, both overloads collapsed into one),
`fanout_pass` (`:166-506`) and the two progress publishers (`:508-576`), and discharged
`AutorouteBatchLoop`'s loud stub.

Three arms of the loop are **lifted out** of the two methods, for the reason Task 11
lifted `sorted_unconnected_targets` out — each is a pure function of data a test can
build, and each has a case no corpus board reaches:

| lift-out | Java | the case the corpus cannot produce |
|---|---|---|
| `FanoutLoopState::{board_state, after_pass}` + `FanoutStop` | `:105-109`, `:125-156` | four passes with an identical `(routedCount, viaCount)` pair (quirk #222), and a pass that routes something without moving the board's hash (ruling AH's first decision site) |
| `fanout_ripup_costs` | `:173`, `:179-183` | `fanout.ripupAllowed = false`, which `DefaultSettings` never sets |
| `fanout_pin_can_use_vias` | `:238-259` | an SMD pin whose net number names no net — the `net == null` fall-through of quirk #223 |

Two more things are `pub` where Java is `private`, and for one reason: the differential
driver is a separate crate and cannot use reflection. `BatchFanout::fanout_pass` is
called by `p7t5 pass`'s `[real]` half, exactly where `P7T5.java` uses
`Method.setAccessible(true)`; `parse_timespan_seconds` — the port of
`TextManager.parseTimespanString` for `:94-99`, which `fr-settings` deferred to
Plan 8 because the settings path never parses a timeout, and which Plan 8 Task 0
landed as `fr_core::parse_timespan_seconds` (`crates/fr-settings/src/lib.rs:214`
is now the `renamed:` row) — is called by the test that pins quirk #224.

`scripts/differential/run.sh p7t5 <dsn> [passNo|maxPasses] [sortingOrder]
[order|pin|pass|board]` is the evidence. Modes `order` and `pin` are Task 11's; modes
`pass` and `board` are Task 12's and each has a transcribed half and a **real** half,
compared by a `getHash()` / `structural_hash` *decision* (`EQUALS-TRANSCRIPT`).
**32 runs, 32 MATCH** — eight DSNs (`Issue730-DAC2020_bm11`, `Issue558-dev-board`,
`Issue508-DAC2020_bm06`, `Issue143-rpi_splitter`, `Issue508-DAC2020_bm01`,
`Issue026-J2_reference`, `tutorial_board`, `Issue649-kicad_ecc83-pp_input_board_v1`)
× four modes, 0 diffs. Two of the eight carry no SMD pins at all and are therefore the
degenerate case rather than a measurement.

### The three clocks the stage carries, and what each parity run does with them

| clock | Java | how a parity run neutralises it |
|---|---|---|
| the per-pin `TimeLimit` (`:231-232`) | `settings.fanout.maxMillisecondsPerPin * (passNo + 1)`, narrowed with an `(int)` cast | both sides write `Integer.MAX_VALUE` into that **setting**; the port folds it into `RouterBudget::fanout_ms_per_pin`, which ruling AI already made the knob |
| the stage deadline (`:94-99`, read at `:111-112` and `:396`) | `fanoutStart + parseTimespanString(settings.fanout.timeout) * 1000` | `DefaultSettings` ships no `fanout.timeout`, so `deadlineMs` is never set. It is **not** `RouterStop`'s: `:113` writes `isTimedOut` and never the stop flag, which is Task 4's documented split, and `tests/fanout.rs`'s `the_stage_deadline_never_touches_the_stop_flag` is the pin |
| `ProgressThrottler(1000)` (`:28`, gating `:520`) | wall clock, per pin | no mode compares progress events at all. The port routes the throttler through `RouterBudget::progress_throttle_ms` so a caller *can* pin it, and ruling 11 makes the sink an observer no decision reads |

The 1000 ms `timeLimitToPreventEndlessLoop` inside `RoutingBoard.fanout` (`:1100`) is a
fourth, and it is the one neither side can disable on the Java half: it is a `javac`-
inlined local. The port runs `RouterBudget::disabled()` against Java's live limit, so a
MATCH *proves* it never trips on the corpus.

### Ruling 5's container decisions, confirmed against Java

| container | Java | comparator | decision |
|---|---|---|---|
| `BatchFanout.sortedComponents` (`:25`, filled `:53-61`) | `TreeSet<Component>` | `Component.compareTo` (`:682-693`) — pin count **descending**, then `boardComponent.id` **ascending**; both keys `final`, ids unique | **`BTreeSet<FanoutComponent>`**, with an `Ord`-consistent `Eq` because Java's `Component` declares no `equals` and its set membership is `compareTo` alone |
| `BatchFanout.Component.smdPins` (`:635`, filled `:673-677`) | `TreeSet<Pin>` | `Pin.compareTo` (`:742-777`) — a key chosen from `settings.fanout.pinSortingOrder` **at run time**, then `boardPin.pinIndex` | **`JavaTreeSet<FanoutPin>`** through `add_by`, carrying the order the way Java's inner class carries the outer field |

### Java wins: scan ruling 8's reason is half wrong, and the container is still right

The plan says an unrecognised `pinSortingOrder` "falls through with `result = 0`"
and concludes the comparator "can return `0` for two distinct pins" and is not a
total order. **`:773-775` is outside the `if`/`else if` chain**: the `pinIndex`
tie-break runs on every branch, including the one that matched nothing, so an
unrecognised string gives *pure `pinIndex`* order (quirk #220) and the comparator
answers `0` only for two pins of one component that share a `pinIndex` — which no
reader produces, `pinIndex` being "the index of the pin in its component"
(`board/model/items/Pin.java:49`).

The container stays a `JavaTreeSet` anyway, and deliberately: "no board duplicates
a pin index" is a property of the *input*, and `JavaTreeSet` is `java.util.TreeSet`
whether or not the input has it, while `BTreeSet` is defined only if it does.
`tests/fanout_order.rs`'s `an_unrecognised_sorting_order_collapses_pins_with_equal_pin_index`
and `equal_pin_index_and_an_equal_key_collapse_under_every_sorting_order` pin the
drop from both sides.

Three more places the plan's sketch disagreed with HEAD, all resolved for HEAD:
`Component.Pin` carries **four** sort keys, not two — `surroundingsDensity`
(`:700`, `:725-738`) is an `int` and one of the four `pinSortingOrder` branches;
`EscapeStatistics`' third component is `escapedPercentage`, not `pinsToEscape`
(which belongs to `BoardStatistics.BoardStatisticsFanout`); and `FanoutRunSummary`
has four components, the sketch's three plus `totalDurationMillis`.

### `AutorouteControl::via_rule` owns its rule, and so does `NetClass`

`fanout:1025-1044` assigns `ctrlSettings.viaRule` a `ViaRule` **that is in no
list** — a `new ViaRule(name + "_fallback")` merged at run time from the net
class's vias and `rules.viaRules.firstElement()`'s. A `ViaRuleId` cannot name it,
and pushing the synthetic rule onto `board.rules.via_rules` would put it in the DSN
writer's output — so the control block owns a `ViaRule`, and so does
`fr_board::NetClass` (which is also ruling H's via-rule half; see the obligation
register). The merge's dedup, `ViaRule::contains`, is Java's `==` and therefore
object identity — quirk **#218**, and the reason `ViaInfo` carries a per-`ViaInfos`
identity serial.

### Two arms lifted out of `fanout`, and why

`sorted_unconnected_targets` (`:1002-1021`) and `combined_fallback_via_rule`
(`:1026-1041`) are `pub fn`s called from exactly one place each. Both are pure
functions of data a test can build, and both have a case no corpus board reaches —
a tie in the target sort, and two value-equal-but-distinct `ViaInfo`s — so a test
that had to route a board to observe them would be a slow test of the router. The
rest of `fanout` reads as Java does with two names substituted for two blocks.

## The optimizer stage (Plan 7 Tasks 13 and 14)

`src/pipeline/optimizer.rs` is the whole of `BatchOptimizer`. Task 13 landed the item
half — the type, `createForHeadless` (`// renamed:` to `new`), `containsOnlyUnfixedTraces`,
`optRouteItem`, `getCurrentPosition`, `calculateIncompleteCount` and the protected inner
`ReadSortedRouteItems`. **Task 14 landed the stage half**: `runBatchLoop` (`:125-272`),
`optRoutePass` (`:279-385`), `normalizeAlgorithm` (`:68-78`) and the five `NamedAlgorithm`
identity overrides (`:527-550`) as five `renamed:` consts.

`BatchOptimizer::run_batch_loop` answers an `OptimizerResult`: `:252-255`'s
`completionStatus` as a `TaskState`, `currentPass`, `totalItemsOptimized`, `isTimedOut`
and ruling 1(a)'s per-pass ladder as a `Vec<OptimizerPassRecord>`. **The record is a
second type the plan's interface block did not name**, and it is here for the reason
`BatchLoopResult::per_pass` is: this task's acceptance asks for "the per-pass
score/incomplete tuple identical", and there is nowhere else to put it. Nothing in the
loop reads it (ruling 11's shape).

### The event says `FINISHED` even when the state does not

`:233-234` fires `TaskState.FINISHED` **unconditionally**, whatever ended the loop —
unlike `AutorouteBatchLoop`'s `:571-585`, which branches. The only place Java tells a
timeout from a cancel from a clean finish is the `completionStatus` string at `:252-255`,
a `job.logInfo` payload, and that is what `OptimizerResult::state` carries.
`tests/optimizer.rs`'s `a_max_items_router_stop_disables_this_stage` asserts both halves
at once: the recorded events are `STARTED` then `FINISHED` with nothing between them,
and the state is `Cancelled`.

### `useIncreasedRipupCosts` has two writers, on two different conditions

`optRoutePass:365-368` clears it when **no item improved** in the pass, and
`runBatchLoop:212-215` clears it when **the board score did not rise**. The first fires
first, and when it does the second's guard is already false — so a pass that improves
nothing costs the optimizer its increased ripup costs *and* takes `:220`'s threshold
exit, ending the stage one pass earlier than `:214`'s comment ("keep the optimizer
going to try with normal ripup costs") suggests. Pinned by
`a_pass_that_improves_nothing_clears_the_increased_ripup_costs_and_ends_the_stage`.

### Three arms lifted out, and why

| lift-out | Java | the case the corpus cannot produce |
|---|---|---|
| `optimizer_near_perfect_exit` | `:182-183` | the arm is a `float` product against `1000.0f`; a threshold between half an `f32` ulp of 1 and a whole one makes `f32` and `f64` disagree, and no settings file lands there |
| `BatchOptimizer::apply_pass_improvement` | `:209-218` | a second non-improving pass, which the corpus never reaches because the first one ends the stage |
| `optimizer_route_improved` | `:340-348` | the `(float)`-cast twin of quirk #212's integer-truncating `ItemRouteResult.improvementPercentage`; both are ported, and `the_improvement_recomputation_disagrees_with_the_scorecard_field` asserts the two disagreeing values on one item |

### Quirk #227: the stage runs and changes nothing after any ordinary router run

`RoutingPipeline.run` (`:81-85`) hands both stages the one `job.thread`, and
`grep -rn requestStop src/main` finds **no writer that lowers it**. Every ordinary exit
from `AutorouteBatchLoop.run` raises `AUTO_ROUTER_ONLY` (quirk #214); `runBatchLoop:171`
reads `ALL`, so the stage runs; but `BatchAutorouter.autoroutePassesForOptimizingItem:268`
reads `!= NONE`, so every `optRouteItem` in it routes **zero** passes, measures a worse
board and restores its snapshot. Measured on the JVM: `p7t9 <rpi> 1 optimizer-shared`
prints six `OPT-ITEM improved=false` lines and an `OPT-RESULT` board identical to the
`ROUTED` one. **Task 15's `run_pipeline` must reproduce that, not reset the flag** — the
port's `run_batch_loop` takes the caller's `RouterStop` and does not clear it.

### The stage's own clock, and ruling AI's obligation discharged

`:153-160` builds `deadlineMs` from `settings.optimizer.timeoutString` and `:172` /
`:308` read it; both write `isTimedOut` and **neither touches the stop flag**, because
`RoutingPipeline.java:117` gates this very stage on that flag. `BatchOptimizer::
is_deadline_reached` is the reader, and `pipeline/stop.rs`'s `obligation:` — declared in
Task 4, half-discharged for `BatchFanout` in Task 12 and for the field in Task 13 — is
**closed** here. `DefaultSettings` ships no `optimizer.timeout`, so no corpus run has a
deadline at all; `the_stage_deadline_times_out_without_touching_the_stop_flag` builds one
from `"0"` and asserts the flag stayed `NONE`.

`scripts/differential/run.sh p7t9 <dsn> 1 [optimizer|optimizer+fanout|optimizer-shared]
[optPasses|all] [optItems|all]` is the evidence: **15 runs, 15 MATCH** over
`Issue143-rpi_splitter`, `Issue026-J2_reference` and
`Issue649-kicad_ecc83-pp_input_board_v1`, plus `examples/tutorial_board` (both modes),
`Issue143-rpi_splitter` at router `maxPasses = 2` and `Issue508-DAC2020_bm01`.

### Two stems DIFF — **closed by Task 14c** (ruling 1's row; localised by Task 14b as quirk #229, fixed under ruling BA)

**Both rows are MATCH as of Task 14c.** The table below is kept as the record of what the two
DIFFs were and how they were read, because the fix is exactly the statement it names.

| stem | rung reached | first difference (before Task 14c) | diagnosis |
|---|---|---|---|
| `Issue558-dev-board` (`optimizer`, `optimizer+fanout`) | (a) pass tuple ✅ through item 13, then (b) fails | `p7t8 <dev-board> item 1 20`: `BOARD n=6 maxIdAfter=14378` (Java) vs `14377` (port) | `optRouteItem`'s **failed** attempt on item 6 burns 188 ids in Java and 187 in the port. Every board number is identical there — items, traces, vias, incompletes, cumulative length. The shift then flips `ReadSortedRouteItems`' strict-`<` tie (`:628-632`) over the descending-id walk (quirk #63) between two geometrically equal traces, and the runs diverge for real from item 14 |
| `Issue026-J2_reference` at router `maxPasses = 2` | same | `p7t8 <j2> item 2 30`: `BOARD n=4 maxIdAfter=5244` vs `5208` | the same shape, 36 ids |

**It is pre-existing and it is not the optimizer stage's.** `p7t8 item` is Plan 7 Task 13's driver
and runs `optRouteItem` with the stop conditions removed — no `runBatchLoop`, no `optRoutePass` on
the path. `p7t8 <dev-board> sequence` MATCHes (614 lines), so the *reader* agrees on the unmutated
board; `p7t9 <dev-board> 1 router-only` MATCHes including its `maxId=` line, so plain routing's id
trail agrees. Both sides are deterministic — three consecutive Java runs and two port runs are
byte-identical.

**Localised by Task 14b (ruling AZ): it is the snapshot restore, and it is quirk #229.** The
statement is `BatchOptimizer.optRouteItem:509`'s `routingBoard.undo(null)` against the port's
whole-board clone assignment (`crates/fr-router/src/pipeline/optimizer.rs:695-710`). Java's `undo`
replays the failed attempt's item changes **through the live search trees** — on `dev-board`, 54
`MinAreaTree` remove/insert ops at the tail of item `n=1`, which is the first `improved=false`
item — so Java's trees come back with the same leaves in a different topology; the port's clone
restores the topology exactly. `ShapeSearchTree45Degree.completeShape:152-274` then reads that
difference: its traversal prune shrinks as obstacles are consumed (`:157` against `:263-264`), so
topology decides *which* obstacles restrain a room, and one differently-shaped free-space room at
item `n=6` costs one extra `ShapeTraceEntries.nextSubstituteTracePiece` id. `J2_reference` is the
same statement — item `n=3` is `improved=false`, 84 Java-only tree ops, divergence at `n=4`.
The bisect, the committed ledgers (`P7T14B_MAT` / `_FP` / `_CS` / `_MAZE`, all off by default,
with Java twins in level 8 of `scripts/differential/java/p6t17b-bisect.patch`) and what a fix
re-opens are in `.superpowers/sdd/2026-08-30-plan-7-router-batch/task-14b-report.md`.
**Resolved by Task 14c (controller ruling BA), and the second of those two fixes is the one that
landed.** `crates/fr-board/src/board/snapshot.rs`' `Board::undo_from_snapshot` is the port of
`BasicBoard.undo` + `applyUndoRedoSideEffects`: it keeps the clone for the *item state* but takes
only `components` and the item map's difference from it, cancels every item the attempt inserted
or modified in place from the **live** trees in Java's descending-id order (`:1262`), re-inserts
the pre-attempt ones in Java's order — the modified ones descending, then the delete list in
*deletion* order (`:1276`) — and leaves every other field of the live board alone, including the
id generator, `revision`, `changedArea`, `min`/`maxTraceHalfWidth`, `normalizeSuppressedNetNos`,
`shoveFailingObstacle`/`shoveFailingLayer` and the autoroute scratch of every item the undo does
not restore. The order comes from `crate::board::snapshot::UndoJournal`, the one-level port of
`UndoableObjects`' `deletedObjectsStack` and `UndoableObjectNode.level`/`.undoObject`.

The proof is the same instrument that found the bug: with `P7T8B_IDS=1 P7T14B_MAT=1 P7T14B_FP=1`
on both sides, the `MAT`/`TREEFP`/`ITEMSEP` streams are now **byte-identical over the whole run** —
134 910 lines on `Issue558-dev-board` (`p7t8 item 1 3`) and 89 629 on `Issue026-J2_reference`
(`p7t8 item 2 6`), where Task 14b measured 54 and 84 Java-only ops. `p7t8 <dev-board> item 1 20`,
`p7t8 <j2> item 2 30` and `p7t9 <dev-board> 1 optimizer` are MATCH, and every row that was already
MATCH stayed MATCH. **Task 16's SES gate is clear.**

## What Plan 7 inherits

*(Historical: this is the table as Plan 6 handed it over. Every row's status
line below was written by the Plan 7 task that closed it, and the whole table is
now discharged — Plan 7 Task 17's gate,
`grep -rn "added in Plan 7" crates/*/src crates/*/tests`, returns nothing. The
forward-looking table is "What Plan 8 inherits" at the foot of this file.)*

Everything above `route_connection`, and nothing below it. Each row names the
Java it starts from and where this tree records it; `src/lib.rs`'s roster
carried the same list method by method while the deferrals were live.

| what | Java | where it is recorded here |
|---|---|---|
| `AutorouteConnectionRouter.route` **steps 6-8** — the necked retry, the strict-DRC rollback, the failure-log write | `autoroute/pipeline/AutorouteConnectionRouter.java:160-233` | `src/autoroute/maze/engine.rs:1818`; `src/lib.rs` roster; obligation register |
| ~~the **pass loop** and the per-pass / per-item recovery boundaries~~ — **DONE, Plan 7 Tasks 9 and 10**: `AutoroutePassRunner.runSingleThread` and its whole-body catch (boundary 7), and `AutorouteBatchLoop.run` with `:44-56`'s propagating throw (boundary 9). What is left of the row is `BatchAutorouterThread.java:537` (boundary 8), on the dead multithreaded path | `AutoroutePassRunner.java:156, :331-335`, `AutorouteBatchLoop.java:44-56`; `BatchAutorouterThread.java:537` | `src/pipeline/pass_runner.rs`, `src/pipeline/batch_loop.rs`; `src/lib.rs` roster for the one that is left |
| ~~the **fanout** pre-pass~~ — **DONE, both halves**: `RoutingBoard.fanout` and `BatchFanout`'s ordering in Plan 7 Task 11, and `fanoutBoard` / `fanoutPass` / `publishProgress` in **Task 12**, which also discharged `AutorouteBatchLoop`'s loud stub. With it `ctrl.isFanout` is set on a real run for the first time, so `locator.rs:267`'s fanout arm and `engine.rs:1374` are now on a live path | `BatchFanout.java:81-163`, `:166-576`, `RoutingBoard.java:978-1110`, `AutorouteBatchLoop.java:83-218` | `src/board_ext/routing_board_ext.rs`, `src/pipeline/fanout.rs`, `src/pipeline/batch_loop.rs` |
| ~~the **optimizer**~~ — **DONE, both halves**: the item half in Plan 7 Task 13 (`BatchOptimizer`'s type, `createForHeadless`, `containsOnlyUnfixedTraces`, `optRouteItem`, `getCurrentPosition`, `isTimedOut`, the protected inner `ReadSortedRouteItems` and `BatchAutorouter.autoroutePassesForOptimizingItem`) and the **stage half in Task 14** (`runBatchLoop`, `optRoutePass`, `normalizeAlgorithm` and the five identity consts), with `BatchOptimizerMultiThreaded` / `OptimizeRouteTask` / `createForGui` rostered `not ported:`. `ItemRouteResult` landed in Task 9 | `autoroute/pipeline/BatchOptimizer.java:125-272`, `:279-385`, `:395-514`, `:563-659`, `BatchAutorouter.java:245-281` | `src/pipeline/optimizer.rs`, `src/pipeline/batch_autorouter.rs`; `src/lib.rs` for the two multithread classes |
| ~~`ViaOptimizer`, whole~~ — **DONE**: `optViaLocation`, `optPlaneOrFanoutVia` and `isWithinTolerance` in Plan 7 Task 6, the three `repositionVia` overloads in Task 7 (which also deleted ruling B1's `unimplemented!` and its guard predicate) | `board/optimize/ViaOptimizer.java:33-158`, `:161-296`, `:302-365`, `:367-429`, `:434-713`, `:719-732` | `src/board_ext/via_optimizer.rs` (and the audit-map row, re-pointed there from `lib.rs` in Task 6) |
| ~~`RoutingBoard.optChangedArea` (both overloads)~~ — **DONE in Plan 7 Task 5**; `RoutingBoard.removeItemsAndPullTight` is still open | `RoutingBoard.java:151-190`, `:124-127`, `RoutingBoardOperations.java:52-79` | `RoutingBoardExt::{opt_changed_area, opt_changed_area_with_keep_point}`; `crates/fr-board/src/board/mod.rs`'s marker for it, now a `renamed:` onto `RoutingBoardExt::remove_items_and_pull_tight` (**DONE in Plan 7 Task 8**) |
| ~~`RoutingBoard.moveDrillItem`~~ — **rostered `not ported:` in Plan 7 Task 6**: the plan's scan ruling 3 said `ViaOptimizer` moves vias through it, and it does not (`ViaOptimizer.java:136`, `:244`, `:282` call `DrillItemMover` directly). Its only Java caller is `MoveComponent.insert:156`, whose only caller is `gui/interactive/DragItemState.java:56-61` | `RoutingBoard.java:252-295` | `crates/fr-board/src/board/mod.rs`'s `not ported:` marker, with the grep evidence |
| ~~the five `PolylineTrace.change` → `additionalUpdateAfterChange` call sites~~ — **CLOSED in Plan 7 Task 8 under controller ruling AJ**: `retainAutorouteDatabase` is a Java benchmark-only system property, so the hook is dead on every live path in both languages | `PolylineTrace.java:188`, `BoardItemRepository.java`, `ShapeTraceEntries.java:880` | five `// not reachable:` markers in `crates/fr-board/src/board/` |
| ~~the `ConnectionToPin` trio — `check`, `correct`, `swapConnectionToPin`~~ — **DONE in Plan 7 Task 5** (all three; the plan's scan ruling 5 wrongly recorded `check` as landed in Plan 6) | `board/trace/PolylineTrace.java:1013-1313` (`pinEdgeToTurnDist` is `-1` throughout Plan 6) | `PolylineTraceExt::{check,correct,swap}_connection_to_pin`; `src/board_ext/tightener/` module docs |
| `RoutingFailureLog` — `fr-board`'s `failure_log: Vec<String>` becomes the real type | `autoroute/RoutingFailureLog.java` | `crates/fr-board/src/board/mod.rs`'s field; `src/lib.rs` roster |
| ~~**the `fr-board` fix ruling H decided**: `ViaRule` must own its `ViaInfo`s~~ — **DONE, both halves**: the via-info half in Plan 7 Task 0 and the **via-rule** half in Plan 7 Task 11 (`NetClass` owns its `ViaRule`, controller ruling AN), measured on `Issue143-rpi_splitter.dsn` + `tests/data/ruling-h-viarule.rules` — DIFF on all eight connections before, MATCH after | `rules/ViaRule.java:21`, `rules/NetClass.java:28`, `io/specctra/RulesReader.java:340-357`, `io/specctra/parser/Network.java:413-417` | `src/autoroute/maze/control.rs:385`; `crates/fr-board/src/rules/{via.rs,net_class.rs}`; `tests/data/p7t11-ruling-h-viarule.txt`; obligation register |
| the eight **re-marked** coverage obligations | see the marker table above | `grep -rn "obligation:" crates/fr-router/src` |
| `max_passes == 0` means **unlimited** (quirk #140), and `-mt` is **not** a threading policy on the headless path (quirk #143) | `RouterSettings.validate`, `BatchOptimizer.createForHeadless:51-53` | House rules above; `docs/java-quirks.md` |
| ~~`RoutingPipeline`, the pipeline sequencer, and `AutorouteUnroutedReport`~~ — **DONE in Plan 7 Task 15**: `run` is `run_pipeline`, sequencing the two stages with quirk #227's no-reset preserved and the fanout-only settings-clone contract; `build` is `build_unrouted_report`, which also discharges `fr-drc`'s forward marker (now a `renamed:`) and `batch_loop.rs`'s stagnation-report stub. `createForHeadless`, `createForGui`, `getAutorouter`, `getOptimizer` and the three `add*Listener` methods are all accounted for in `pipeline/run.rs` (a `renamed:` and six `not ported:` markers) — Plan 8 has no `RoutingPipeline` surface left to wrap, only `PipelineResult` | `autoroute/pipeline/RoutingPipeline.java:81-129`, `AutorouteUnroutedReport.java:19-79` | `src/pipeline/run.rs`, `src/pipeline/unrouted_report.rs` |

**The one thing Plan 7 must not do** is re-implement steps 1-5. `route_connection`
is what 369 connections of byte-identical evidence attach to; a second
implementation above it would have none.

## `run_pipeline` and `build_unrouted_report` (Task 15, ruling AK)

`RoutingPipeline.run()` (`RoutingPipeline.java:81-129`) collapses into one
function, [`pipeline::run_pipeline`]: `routerEnabled = getRunRouter() &&
(maxPasses == null || maxPasses >= 0)` (`:88-91`) decides between the ordinary
router run, the fanout-only mode (`maxPasses` forced to `0` on a **settings
clone** — `run_pipeline` borrows `&RouterSettings`, so it cannot mutate and
restore the caller's object the way Java's `finally` does) or neither; either
way [`AutorouteBatchLoop::run`] does the real work, both stages hand the same
`RouterStop` (**quirk #227's no-reset**, inherited unchanged from Task 14), and
`:110`'s `job.board.finishAutoroute()` — the *only* caller of
`RoutingBoard.finishAutoroute` in the whole Java tree — is a no-op this port
cannot even reach: ruling AJ makes `retainAutorouteDatabase` permanently
`false`, so no `AutorouteEngine` ever survives a lower-level call into this
scope for `RoutingBoardExt::finish_autoroute` to consume. The optimizer stage
is skipped only on `isStopRequested()` (`ALL`, quirk #202/#227's other half),
never on an `AUTO_ROUTER_ONLY` stop — [`PipelineResult::optimizer_state`]
tells "never configured" (`None`) apart from "configured but skipped"
(`Some(TaskState::Idle)`) for exactly that reason.
`AutorouteUnroutedReport.build` is [`pipeline::build_unrouted_report`] — a
**consumer** of `fr-drc`, so it lives here rather than there (`fr-drc`'s own
marker at `src/lib.rs:143` is now a `// renamed:`) — and discharges
`batch_loop.rs`'s stagnation-report stub: both stagnation arms now build the
real report rather than an empty string, at Java's own cost (a fresh
`DesignRulesChecker`, `calculateAllIncompletes`, `getAllAirlines`) every time
either fires. `RoutingPipeline.createForGui`, `getAutorouter`, `getOptimizer`
and the three `add*Listener` methods are `not ported:` in `pipeline/run.rs`
(controller ruling AK replaces the listener mechanism with `ProgressSink`,
same as `NamedAlgorithm`'s); `createForHeadless` is `renamed:` into
`run_pipeline`'s own setup. `RoutingPipeline` therefore drops off the
`autoroute/pipeline` audit's `ROSTERED` list entirely (Audit section above),
and Plan 8 has no `RoutingPipeline` surface left to wrap — only
[`pipeline::PipelineResult`].

### A finding: `PipelineResult::passes_run` is not `job.getCurrentPass()`

`AutorouteBatchLoop.java:270-274`'s `maxPasses` cap check runs *before*
`:276`'s `job.setCurrentPass(currentPass)`, so on a capped exit the job's own
value is **one less** than the loop's local `currentPass` at the point its
final `TaskStateChangedEvent` fires (`:572-584`): the local was already
incremented past the cap by the completed prior iteration's `:521`, and the
aborted final iteration breaks before `job.setCurrentPass` runs again for it.
`p7t9 full`'s first run measured this directly — `job.getCurrentPass()` read
`1` against the port's `passes_run = 2` at `maxPasses = 1` on
`Issue143-rpi_splitter.dsn` — before the driver was corrected to read the
router's own last `TaskStateChangedEvent.getPassNumber()` instead, which
carries the same local `job.getCurrentPass()` does not. Not a quirk (both
numbers are Java's own, and neither is wrong — they simply answer different
questions), and not a bug in Task 10's field (its own doc already recorded the
`+1` behaviour); it is a mistake the plan text made in describing
`passes_run`'s source, corrected in `pipeline/run.rs`'s doc.

**`p7t9` mode `full`, both stages, all three acceptance stems at
`maxPasses ∈ {1, 2, 8}`: 9/9 MATCH.** Plus `tutorial_board`, `bm01` and
`empty_board.dsn` (which routes nothing but exercises the same sequencing) —
all MATCH.

## `prepare_board` and the clearance overrides (Task 15b, ruling AW)

`pipeline::prepare_board(&mut Board, &RouterSettings)` is the board half of
`HeadlessBoardManager.applyRouterSettingsForLoadedBoard`
(`management/HeadlessBoardManager.java:739-749`). That method has **four** steps;
`fr_settings::resolve_headless` already carried the first two (`:741-744`'s
`setLayerCount`, `:745`'s `applyBoardSpecificOptimizations`) because they mutate
the *settings*. The last two mutate the **board**, and until this task nothing in
the port applied them at all:

```java
  applyCopperToEdgeClearanceOverride();   // :746 -> :466-552
  applyHoleClearanceOverride();           // :747 -> :346-396 (-> :404-464)
```

The three methods are ported as `fr_board::Board::{apply_copper_to_edge_clearance_override,
apply_hole_clearance_override, assign_hole_keepout_clearance_class}`
(`crates/fr-board/src/board/clearance_override.rs`); `prepare_board` is only the
call pair, in Java's order. The order is load-bearing: both overrides append a
clearance class when they fire, so copper-first is what gives `board_edge` the
lower index.

**This is not a rarely-taken path.** `DEFAULT_COPPER_TO_EDGE_CLEARANCE_UM` is
500.0 and the `:501-507` guard early-returns only when the configured value *is*
the default **and** the outline carries an explicit, non-fallback DSN clearance
class. On **15 of the 16** parity-corpus boards the reader gives the outline the
fallback AREA class, so a plain `-de <dsn> -do <ses>` run appends a `board_edge`
class, writes 500 µm into its whole row and column on every layer, and re-points
the outline at it (quirk #231). Only `router-rpi-splitter` early-returns, through
its explicit `boundary` class. Measured on the HEAD jar: `Issue026-J2_reference`
routes to **15 254 B** of SES by default and **14 644 B** with
`--router.copper_to_edge_clearance_um=0`, differing from char 741.

**Nothing that existed before this task calls `prepare_board`.** Every driver,
probe and parity test in the tree loads through `fr_dsn::read_board` directly, as
`scripts/differential/java/P6T1.java:173` loads through `DsnReader.readBoard`, so
all of them work on the pristine board and their committed references encode it.
Adding this function changes none of them — asserted by re-running `p6t1`,
`p6t3`, `p7t8` and `p7t9` and diffing. Task 16's `batch.ses` references, which
come from the real CLI, are the first consumer; Plan 8's loader is the second.

**One call, not two.** Java runs the pair at *two* sites per DSN load —
`createBoard:342-343` on the itemless board and `:746-747` on the loaded one
(Plan 8 survey ruling AD). The port has no `createBoard` hook, so it runs the
pair once, after the load; `clearance_override.rs`'s module docs carry the
argument for why that lands the same board (measured on all 16 boards × 9
variants) and the one non-default case where it does not (quirk #232).

**Evidence.** `probes/P7T15bProbe.java` → `tests/data/p7t15b-clearance-overrides.txt`,
replayed by `tests/clearance_override.rs` (5 corpus stems in CI, all 16 under
`FR_SLOW_PARITY=1`; 5.5 s debug, 0.8 s release for the full sixteen). The arms no
corpus board reaches — a pre-declared `board_edge` class, an explicit outline
class, an odd board-unit value meeting `ClearanceMatrix.setValue`'s
round-up-to-even — are in `crates/fr-board/tests/clearance_override.rs`. And
`P7T15B_PREPARE=1 ./scripts/differential/run.sh p6t1 <J2> 45 1` routes 45 whole
connections with the override live on both sides: **MATCH**, and 44 of the 45
rows differ from the same run without the switch.

## The whole-board acceptance ladder (Task 16, ruling 1 / ruling AM)

`tests/reference/<stem>/batch.ses` is the **HEAD jar's verbatim SES** for a
whole-board `java -jar <jar> -de <dsn> -do <ses> -mp <n>` run and
`batch.passes.jsonl` its per-pass `PassRecord` tuples;
`scripts/gen-batch-reference.sh` writes both through
`scripts/differential/java/P7T9.java` mode `batch`, and
`crates/fr-router/tests/batch_parity.rs` climbs the ladder against them. The
port's side is the jar's own flow: `fr_settings::resolve_headless` on the same
`argv`, then `pipeline::prepare_board` (ruling AW), then `run_pipeline`, then
`fr_dsn::ses_writer::write`.

The rungs, per stem:

* **(a)** the pass count and every `PassRecord` tuple identical;
* **(b)** the item set after every pass identical;
* **(c)** byte-identical SES.

**Measured: all eight stems reach (a), (b) and (c).** Ruling AM's escape hatch —
an `XDIFF` row carrying the first differing byte and a normalised digest — is
unused, and `every_stem_reaches_rung_c` is what stops it being quietly re-entered.

| stem | dsn | `-mp` | fanout | optimizer | lane | (a) | (b) | (c) | passes | final SES | last pass tuple |
|---|---|---|---|---|---|---|---|---|---|---|---|
| `router-rpi-splitter` | `Issue143-rpi_splitter.dsn` | 8 | on | on | CI | ✅ | ✅ | ✅ | 3 | 3 654 B | score 999.9719, 0 incomplete, 0 violations, 9 vias, 16 traces |
| `router-dac2020-bm01` | `Issue508-DAC2020_bm01.dsn` | 2 | on | on | slow | ✅ | ✅ | ✅ | 2 | 66 387 B | score 825.6266, 34 incomplete, 0 violations, 130 vias, 432 traces |
| `router-j2-reference` | `Issue026-J2_reference.dsn` | 99 | on | on | CI | ✅ | ✅ | ✅ | 2 | 15 254 B | score 999.9879, 0 incomplete, 0 violations, 21 vias, 104 traces |
| `router-tutorial-board` | `examples/tutorial_board/tutorial_board.dsn` | 8 | on | on | slow | ✅ | ✅ | ✅ | 1 | 462 B | score 0.0 — the board's 438 `@:no_net_N` nets leave nothing to route |
| `router-ecc83-input` | `Issue649-kicad_ecc83-pp_input_board_v1.dsn` | 8 | on | on | CI | ✅ | ✅ | ✅ | 2 | 4 509 B | score 999.997, 0 incomplete, 0 violations, 0 vias, 18 traces |
| `router-fanout-bm11` | `Issue730-DAC2020_bm11.dsn` | 2 | on | **off** | slow | ✅ | ✅ | ✅ | 2 | 56 155 B | score 987.4912, 2 incomplete, 0 violations, 53 vias, 335 traces |
| `router-strict-drc-cnh` | `Issue555-CNH_Functional_Tester_1.dsn` | 2 | on | on | slow | ✅ | ✅ | ✅ | 2 | 40 915 B | score 930.0412, 11 incomplete, **16 violations**, 4 vias, 214 traces |
| `router-empty-board` | `empty_board.dsn` | 1 | off | off | CI | ✅ | ✅ | ✅ | 1 | 212 B | score 0.0 — no items at all |

`router-strict-drc-cnh`'s 16 violations are the board's **pre-existing** ones and
the count does not move: the port adds none, which is what `StrictDrcRoutingTest`
asserts and what `tests/fixtures.rs` pins independently.

**The one blind spot this ladder is known to have, and what covers it now.** All
eight stems stayed byte-identical to the jar while the fanout target sort broke
every distance tie the wrong way (the port seeded a `BTreeSet` ascending where
Java seeds a `TreeSet<Item>` descending — quirk #44), because **no corpus board
contains a fanout distance tie**. It took an external board with a symmetric
jumper to expose it, and the defect shipped. The gap is closed by a directed
fixture rather than by adopting another whole board:
`tests/fanout_tie_break.rs` routes the hand-written
`tests/data/p8-fanout-tie.dsn` — four SMD pads on the corners of a square, so
the two orthogonal neighbours of the first pad fanout reaches are at *exactly*
equal squared distance, plus a blocking pad that makes the tie decide the routed
output rather than mirror it — and requires byte-identity with the HEAD jar's own
`-de/-do` answer, transcribed in `tests/data/p8-fanout-tie-jar.txt`. It is
mutation-checked: removing the `.rev()` from `sorted_unconnected_targets` makes it
fail with two extra vias and a `B.Cu` detour the jar does not have. See quirk
#44's "port defect history" note and `docs/plan-8-handoff.md`'s §Errata.

**What each ✅ in the table is worth.** Rungs (a) and (c) are exactly what they
say: `batch_parity.rs` compares every `PassRecord` tuple against
`batch.passes.jsonl` field by field, and the whole SES against `batch.ses` byte by
byte. **Rung (b)'s committed half is counts, not sets** — `rung_b_item_sets`
compares the per-pass via and trace counts the tuples carry and the final SES's
`(wire`/`(via ` scope count, because the committed reference has no per-pass item
*sets* to compare against. The final item set is nevertheless pinned exactly, by
rung (c): a byte-identical SES enumerates the same items with the same geometry.
The full-strength per-pass statement — every item's id, layer, half width,
polyline and corner list after every pass — lives in `sweep-p7t9.sh`, whose
`batch-router` rows diff both sides' complete `[board]` dumps and are 32/32 MATCH.
So rung (b) is proven at full strength against the jar and pinned in CI at count
strength; the sweep is where a regression in the per-pass item set would surface.

**Two aggregate climbs, not one test per stem** — a deliberate deviation from the
plan's "one test per stem", recorded here because it changes what a failure tells
you. `the_ci_stems_climb_the_whole_ladder` walks the four CI stems and
`the_slow_stems_climb_the_whole_ladder` all eight, so a failure on the first stem
of a lane masks the rest of that lane's result until it is fixed; eight `#[test]`s
would report all eight independently. The aggregate shape was kept because ruling
AM's CI/slow lanes are a property of the *set* (`FR_SLOW_PARITY` gates one whole
lane, and `require_java_dir` skips both), because a whole-board stem costs seconds
to minutes and the eight are run for the ladder as a whole rather than
individually, and because `STEMS` is the single table the fixture-file
cross-check (`the_stem_table_matches_the_fixture_file`) and the two provenance
tests already iterate. Splitting them is mechanical if per-stem failure reporting
is later worth more than the shared table.

**The lane** is ruling AM as amended by scan ruling 13: eight stems, four in CI,
four `#[cfg_attr(debug_assertions, ignore)]` + `FR_SLOW_PARITY=1`. The whole slow
lane is ~2 min in release.

### The one normalisation, and why it is not a tolerance

`parity::normalize_ses_head_tokens` rewrites four `(parser …)` keyword literals on
the **reference** side, and nothing else is touched. The clone's HEAD camelCased
them — `(hostCad `, `(hostVersion `, `(stringQuote `, `(writeResolution ` — while
leaving its own lexer recognising only the snake_case tokens, so HEAD writes
Specctra it cannot read back; that is quirk **#92**, and Plan 3 ruling 1
consequently pins this port's writer to the 2.3.0 spelling and
`tests/reference/README.md` forbids regenerating the DSN/SES references from HEAD.
`batch.ses` is the one file in the tree written by **HEAD's** `SesWriter`, so it
carries HEAD's spelling of the two such keywords an SES contains. The set is
closed and enumerated (every camelCase string literal in `SesWriter.java` and
`parser/Parser.java` combined); everything else is compared byte for byte. The
DRC family has the same shape of problem and the same shape of answer
(`normalize_drc_json`, plan-5 ruling 3).

### The budget: what the plan asked for, and what is actually possible

The plan asks the driver to reflect `TIME_LIMIT_TO_PREVENT_ENDLESS_LOOP` to `0`
so that ruling AI's "budget disabled on both sides" holds. **It cannot be done.**
All four declarations are `static final int … = 1000` with a constant initialiser
(`AutorouteConnectionRouter.java:22`, `BatchAutorouter.java:43`,
`BatchAutorouterThread.java:38`, `AutoroutePassRunner.java:32`), so `javac`
inlines them: `javap -c -p` on the shipping jar shows `sipush 1000` immediately
before every `optChangedArea` call and no `getstatic` anywhere, and the fields are
dead. Writing them reflectively changes nothing.

So the arrangement is every other `p7t*` driver's: **Java runs with the live
1000 ms limit, the port with `RouterBudget::disabled()`**, and a byte-identical
SES is *evidence* that the limit never changed the result rather than an
assumption that it could not. The trips that **did** occur are counted from the jar's own logging: `TraceTightener.isStopRequested`
(`board/optimize/TraceTightener.java:202-211`) calls
`FRLogger.debug("TraceTightener.is_stop_requested: time limit exceeded")` on every
exceeded check, and `Log4j2ConfigurationFactory` gives the root logger `Level.ALL`
with a file appender at `DEBUG` writing to a caller-chosen path. So the count is
turning that appender on and running `grep -c`, and
`gen-batch-reference.sh --verify-driver` is what does it — on **both** the bare jar
and the driver (a difference between the two is explained by a trip on either),
and only after a comparison has already come back different, because turning
`DEBUG` on costs wall-clock time and wall-clock time is what the budget measures.

The two sides need different knobs: the driver takes the JVM system properties
`-Dfreerouting.logging.file.{location,level}`, the bare jar the program arguments
`--logging.file.{location,level}`, because `Freerouting.main`
(`Freerouting.java:1088-1098`) overwrites all five of those properties from its own
argument/environment parse before logging initialises — a `-D` on a `-jar` run is
silently discarded (measured on two boards).

A programmatic Log4j2 counting appender was tried first and received **zero**
events against this jar's `Log4j2ConfigurationFactory`, which is why the driver
has no flag of its own; the `-D` route is verified working (a plain
`router-rpi-splitter` run writes 113 `DEBUG` lines to the file, none of them a
trip).

**Measured for all eight stems, in their reference configuration, with the jar's
DEBUG log on** — which makes the run 2-6× slower and so *more* likely to trip:

| stem | budget trips | DEBUG lines | SES vs the committed `batch.ses` |
|---|---|---|---|
| `router-rpi-splitter` | 0 | 129 | identical |
| `router-dac2020-bm01` | 0 | 634 | identical |
| `router-j2-reference` | 0 | 111 | identical |
| `router-tutorial-board` | 0 | 60 | identical |
| `router-ecc83-input` | 0 | 92 | identical |
| `router-fanout-bm11` | 0 | 215 | identical |
| `router-strict-drc-cnh` | 0 | 421 | identical |
| `router-empty-board` | 0 | 42 | identical |

So the references sit on the side of the boundary where the limit never fires,
and that is a measurement rather than an inference.

**Where it does fire, and what that costs — quirk #234.** Push
`router-dac2020-bm01` past its ruling-AM cap and the jar stops being
reproducible: at `maxPasses = 3` pass 3 is `score=841.0115 incompletes=31
traces=408` and at `maxPasses = 20` it is `835.88336 / 32 / 407`, three runs each,
same board, same settings, same `-XX:hashCode=2` — and the `maxPasses = 20` run
*with* the DEBUG log is back to `841.0115` and records **one** trip.
`settings.maxPasses` is read nowhere on the routing path, so the clock is the only
run-to-run input. The port is self-consistent (`841.0115` at every `maxPasses`),
which is what a budget-free implementation must be. The ladder is unaffected: its
`-mp 2` for that stem is below the pass where the trip appears.

### What the ladder does not cover, and why it cannot be extended by raising `-mp`

Beyond `-mp 2` on `Issue508-DAC2020_bm01.dsn` there is no stable reference to
compare against. Measured: a `p7t9 … 3 batch-router` run of the **Java** driver
records one budget trip and answers `maxId=211817` plain and `maxId=211829` with
the DEBUG log on — its own id burn moves run to run — while the port answers
`212483` every time. The pass tuples agree in all three runs (`score=841.0115
incompletes=31 traces=408`), so the board's *metrics* are stable and its *item
ids and geometry* are not.

The port's 666-id gap is well outside Java's own 12-id spread, so a genuine
divergence at pass 3 is likely as well as a budget-driven one — but it cannot be
settled against a reference that is not reproducible. Anyone taking it further has
to stabilise the Java side first (a recompiled jar with the constant at `0` is the
only way, since nothing else reaches it), and only then reach for the level-8
`MAT`/`TREEFP`/`ITEMSEP` ledgers of `p6t17b-bisect.patch`. Raising ruling AM's
`-mp` column without that would turn a green ladder red for a reason that is not
the port's.

Everything below pass 3 on that board, and every pass of the other seven stems, is
byte-parity with **0** measured trips.

### `--verify-driver`: the driver **is** the jar

`scripts/gen-batch-reference.sh --verify-driver` runs the bare jar
(`java -jar <jar> -de … -do … -mp … --router.fanout.enabled=… --router.optimizer.enabled=…`)
and `P7T9 … batch` on the same `argv` and compares the two SES files byte for
byte. Its verdict is committed into each stem's `batch.meta.txt` and asserted by
`the_driver_matches_the_bare_jar`.

| stem | verdict |
|---|---|
| all eight | `bare-jar: identical` — 3 654 / 66 387 / 15 254 / 462 / 4 509 / 56 155 / 40 915 / 212 B |

Controller answer 1's failure case — a bare-jar difference with **zero** recorded
budget trips — did not arise on any stem, so no stem carries a trip count.

### `--verify-hash-modes`: eight stems × five modes, one digest each

`scripts/gen-batch-reference.sh --verify-hash-modes` regenerates each stem's SES
under `-XX:hashCode=0..4` and requires five byte-identical files. Plan 6's survey
verified the premise end to end on two boards; this extends it to all eight with
fanout and the optimizer live.

| stem | 5-mode digest (first 12) | bytes |
|---|---|---|
| `router-rpi-splitter` | `303d795592b5` | 3 654 |
| `router-dac2020-bm01` | `9176b5522415` | 66 387 |
| `router-j2-reference` | `53e3779af20a` | 15 254 |
| `router-tutorial-board` | `ac29268158c1` | 462 |
| `router-ecc83-input` | `6b3915069992` | 4 509 |
| `router-fanout-bm11` | `2191d2e6d715` | 56 155 |
| `router-strict-drc-cnh` | `b0e93e475914` | 40 915 |
| `router-empty-board` | `4e4af63f7388` | 212 |

## What Plan 8 inherits

**`docs/plan-7-handoff.md` is the authority**; this table is the crate-local view.
The marker inventory it rests on, measured on the committed tree at Plan 7 Task 17:

| grep | `crates/*/src` | `crates/*/tests` | `crates/` (adds README prose) |
|---|---|---|---|
| `added in Plan 7` | **0** (the gate) | **0** (the gate) | 3 — all past-tense narrative |
| `added in Plan 8` | 32, of which **30** are markers proper and 2 are prose | 0 | 40 |
| `obligation:` | 60 | 6 | 86 |
| `pub seam:` | 14 (all `fr-router`) | 0 | 22 |
| `not reachable:` | 18 | 2 | 23 |
| `item_tree_shape_ref` / `item_tile_shape_ref` in `crates/fr-router/` | **0 call sites**; 3 hits, all prose about the rule | | plan-6 ruling 10 still holds |

*(These counts are measured on the tree Task 17 committed and include this table's
own lines — the plan-6 hand-off's warning about `crates/` sweeping the READMEs
applies to every row. The `crates/*/src` column is the inventory.)*

> **Closed by Plan 8 Task 14.** `grep -rn "added in Plan 8" crates/*/src` and
> `crates/*/tests` both answer **nothing** on the committed tree: Tasks 0-13 consumed
> the markers incrementally and Task 14 consumed the last seven — the two
> `BoardComparator` twins and the five ruling-AS score rows, all re-pointed to
> `not ported:` — plus the five prose sentences that still quoted the marker text.
> No marker anywhere defers to a ninth plan; there is none.
> `docs/plan-8-handoff.md` §5 is where every surviving `obligation:` row is closed
> with a reason.

The 30 `added in Plan 8:` markers cluster in five places: `HeadlessBoardManager`'s
nine `fr-core` methods (`fr-board/src/board/clearance_override.rs:399-407` —
**all nine consumed by Plan 8 Tasks 0 and 3**, so a re-measurement today counts 21),
`BoardComparator` (`fr-board/src/board/mod.rs:57`, `fr-drc/src/lib.rs:144`), the
KiCad JSON family (`fr-drc/src/lib.rs:114-119`), `SessionToEagle`
(`fr-dsn/src/lib.rs:12`), and ruling 4's score surface
(`fr-router/src/score/mod.rs:51-58`) plus `NamedAlgorithm.job` ×2 and
`TextManager.parseTimespanString`.

| what | Java | where it is recorded here |
|---|---|---|
| ~~`RoutingPipeline.createForHeadless` and the rest of the pipeline wiring~~ — **moved to Plan 7 Task 15**: the whole class is ported/rostered there; Plan 8 wraps `PipelineResult` as its `RoutingResult` instead | `autoroute/pipeline/RoutingPipeline.java` | `src/pipeline/run.rs` |
| `CancelToken` → this crate's `StopCheck` (six checked sites, ruling 6) | `datastructures/Stoppable`, spec §10 | `src/autoroute/maze/` stop-check sites |
| `ProgressSink` replacing the dropped observers | `autoroute/events/**`, `NamedAlgorithm` | `src/lib.rs` roster (`not ported:` + `added in Plan 8:`) |
| `BoardStatistics` — the metric block `tests/fixtures.rs` and `p6t1` stand in for | `core/scoring/BoardStatistics.java:271` | `tests/fixtures.rs` module docs |
| ~~`Wiring.readViaScope`'s unchecked `insert_via` (the ladder hang's last line)~~ — **CLOSED, Plan 8 Task 3**: `read_via_scope` calls `Board::insert_via_checked` with a per-via `TimeLimit` stop built from `DsnReadOptions::normalize_time_limit`, and the p3t15/p6t1/batch_parity gates are unchanged | `io/specctra/parser/Wiring.java:706` | `crates/fr-dsn/src/parser/wiring.rs`'s `insert_via_checked` call |
| the CLI/MCP surface: legacy-flag value normalisation wiring, MCP concurrency | `GlobalSettings.java:675-731`; spec §13 | `crates/freerouting/src/{legacy.rs,mcp/}`; obligation register |

| ~~quirk #232 boundary~~ — **CLOSED, Plan 8 Task 3, and the premise was false**: there is no second override run (quirk #253 — `HeadlessBoardManager.createBoard` is unreachable from the DSN parser), so there is no boundary. Reproduced anyway: the transcript's `after_second_hole_override` stage shows a second `applyHoleClearanceOverride` moving nothing, tree-order digest included | `management/HeadlessBoardManager.java:310-344` vs `:739-749` | `crates/fr-core/tests/overrides.rs::the_second_hole_override_leaves_the_search_tree_alone`; quirks #232 and #253 |

Six more, added by Plan 7 and stated here because a Plan 8 survey that misses one
of them re-opens a closed parity hole:

| what | why Plan 8 must not break it |
|---|---|
| **`fr_router::{score, pipeline}` are re-exported, not moved** (controller answer 3) | spec §4 assigns them to `fr-core`; moving the files is a `use` change in every crate. Plan 8 owns only the JSON/manifest surface and the CLI/MCP wrappers |
| **`CancelToken` maps *onto* `RouterStop`, three-state** | `requestStop()` writes `ALL` and `requestStopAutoRouter()` writes `AUTO_ROUTER_ONLY` (quirk #202). Collapsing them to a bool makes `--max-items` stop the optimizer *and* `--max-passes` stop it, which is quirk #214's whole point. The CLI's `--max-items` must also **document** that it stops the optimizer |
| **quirk #227: nothing resets the stop flag between stages** | `RoutingPipeline.run` hands both stages one `job.thread` and `StoppableThread` has no `clear`. `run_pipeline` reproduces that. A Plan 8 wrapper that "helpfully" resets it changes every board |
| **the `Line` identity token (ruling AE, quirk #74)** | one reader, `Line::is_same_object`, one caller, `Board::change_trace`. Nothing else may use it to stand in for `==` |
| **`JavaTreeSet`, not `BTreeSet`, where the comparator is not total** (ruling Y, scan ruling 8) | `SortedRoomNeighbour`, `MazeListElement` and `BatchFanout.Component.Pin`. The two containers drop *different* elements |
| **`UndoJournal` (ruling BA, quirk #229)** | `Board::{begin_undo_journal, discard_undo_journal, undo_from_snapshot, journal_insert, journal_remove, save_for_undo}` in `fr-board/src/board/snapshot.rs` is `BasicBoard.{generateSnapshot, popSnapshot, undo}` + `applyUndoRedoSideEffects`. The optimizer's restore replays undo's **tree** side effects; a whole-board clone assignment is not equivalent, and that difference burned item ids on two stems |
