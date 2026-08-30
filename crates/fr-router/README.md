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

## State: Task 13 of 18

What exists is the data-model floor the other nine tasks build on, the
search-tree extension that turns a seed shape into expansion rooms, the three
neighbour sorters that turn a completed room into its door list, the
`AutorouteEngine` that owns all of it, the drill pages that manufacture its
layer changes, the four leaf types the maze search itself is written against
(the control block, the cost bound, the queue element and the guarded queue),
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
`findConnection` runs end to end.**

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
| `RouterError` | `src/error.rs` | ruling 7's five recovery boundaries |
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
* **Deterministic containers, transcribed rather than chosen** (ruling 4).
  `BTreeSet`, never `BinaryHeap` — Java pops `mazeExpansionList.iterator()
  .next()` *and re-inserts mutated elements*. Comparators transcribe Java's
  `<`/`>` chains literally, never `total_cmp` and never
  `partial_cmp().unwrap()`: on NaN Java's `<` and `>` are both false and the
  comparison falls through to the next sort key, which both Rust idioms get
  wrong.
* **No GUI, no `FRLogger`, no observers, no static mutable state, no clock.**
* Deliberate Java bugs are reproduced rather than fixed, each with a
  `// Java bug:` marker at the site and a row in `docs/java-quirks.md`.

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
`fr-board`. There the two `// added in Plan 6:` markers on
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
(Tasks 15-16). All five `autoroute` invocations stay at zero UNMAPPED. Task 9 opened the two `board/*` invocations, each
restricted to the file it ports, and both exit 0 with zero MISSING and zero
UNMAPPED:

    ./scripts/audit-port.sh board/actions  crates/fr-router/src 'DrillItemMover.java' scripts/audit-map/fr-router.map
    ./scripts/audit-port.sh board/optimize crates/fr-router/src 'TraceShover.java'    scripts/audit-map/fr-router.map

The wide `board/actions` glob (which also names `ForcedPadRouter.java` and
`ForcedViaInserter.java`) starts passing in Task 10. There is deliberately **no**
`board/facade` invocation against this crate: `RoutingBoardExt` carries five of
`RoutingBoard`'s methods, and the class's other ~100 stay in `fr-board`, whose
own `board/facade` audit covers them — Task 9 turned the three `added in Plan 6:`
markers there (`additionalUpdateAfterChange`, `initAutoroute`,
`checkForcedTracePolyline`) into `renamed:` markers naming this crate, and that
audit still exits 0.

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

### Ruling H: the Java half is pinned, Task 17 closes the row

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

What is still missing is the port routing the same board with the same `.rules`,
which is the comparison that actually decides the row. The `obligation:` marker
on `AutorouteControl::rebuild_via_info` names it; Task 17's `p6t1` owns it. If
the port and the jar differ there, `ViaRule` needs owned `ViaInfo` copies (or
`ViaInfos` needs tombstones) and this fixture is the regression test.


## `RoutingBoardExt` and the check-only shove (Task 9)

Plan-2 ruling 4 left five `RoutingBoard` methods out of `fr-board` because each
one needs an `AutorouteEngine`, which `fr-board` cannot name. Plan-6 ruling 3
puts them here as `RoutingBoardExt`, an extension trait over `fr_board::Board`
that Plan 7 extends further (`optChangedArea`, the pull-tight entry points, the
tighteners). The same reasoning brings `board.optimize.TraceShover` and
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
nothing in Plan 6 reaches, is still an `// added in Plan 7:` marker.
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
and `TraceShover.insert` (`:416`) — already `// added in Plan 7:` markers from
Task 9 under plan-6 ruling 2. `task-10-report.md` §2.1 raised it as a
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

**Plan-6 ruling 4 says the neighbour set is a `BTreeSet`. It cannot be.**
`SortedRoomNeighbour.compareTo` is not a total order, and on such a comparator
`std`'s `BTreeSet` and Java's `TreeSet` keep *different* elements and iterate
the survivors in *different* orders — both measured, against the HEAD jar, by
`scripts/differential/run.sh p6t3 3`. The container is therefore
`crates/fr-router/src/java_tree_set.rs`'s `JavaTreeSet`, a transcription of
`java.util.TreeMap`'s red-black `put`, `fixAfterInsertion` and in-order
traversal. Quirk row #160 records the measurement; the test
`the_comparator_is_not_transitive_and_drops_the_same_neighbour_java_does` pins
both the Java answer and the `BTreeSet` answer so the choice cannot be
"simplified" away. **Task 8's `MazeListElement` queue should re-check the same
question** before reusing `BTreeSet`: ruling 4 also prescribes one there, and its
comparator has a documented five-way `Equal` (quirk #156).

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
