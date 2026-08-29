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

## State: Task 7 of 18

What exists is the data-model floor the other twelve tasks build on, the
search-tree extension that turns a seed shape into expansion rooms, the three
neighbour sorters that turn a completed room into its door list, the
`AutorouteEngine` that owns all of it, and — from Task 7 — the drill pages that
manufacture its layer changes:

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
| `AutorouteSearchTreeExt` | `src/autoroute/tree_ext.rs` | `ShapeSearchTree.java:580-693,701-811,1095-1118` + `…45Degree.java:38-86,95-281,288-298,305-486` + `…90Degree.java:38-191,198-322` |
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
sinks, which carry `not ported:` markers naming a GUI overlay. All five
`autoroute` invocations stay at zero UNMAPPED, and only `autoroute/path` (7,
Tasks 14-15) is untouched.

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
incomplete room added). The next free id is **#170**. Every later
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
