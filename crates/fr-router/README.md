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

## State: Task 3 of 18

What exists is the data-model floor the other fifteen tasks build on, plus the
search-tree extension that turns a seed shape into expansion rooms:

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
— lives on `src/autoroute/maze/mod.rs` for Task 6's engine to re-export.

`ExpansionRoomStore::clear` takes the tree, because `AutorouteEngine.clear`
(`:306-317`) removes every complete room's leaf **before** it drops the lists;
skipping that would leave `TreeObject::Room` keys in the shared tree naming
arena slots that no longer exist.

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
this crate ports (ruling 13). As of Task 3 the package-root invocation reaches
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
crates/fr-board/src` still exits 0.

## Quirk-register numbering

`docs/java-quirks.md` is allocated **contiguously, in the order rows are
written**. Plan 6's plan text labels its rows `#155`–`#168`, but `#155` was
already taken by Plan 5, so those labels are **not** row ids. Task 2 wrote the
first three Plan 6 rows and they landed as **#156, #157, #158**; Task 3 wrote
**#159** (`ShapeSearchTree90Degree.completeShape` drops a room the base class
and the 45-degree override keep), so the next free id is **#160**. Every later
task must re-read the register's last row rather than trust the plan's labels —
the plan carries an amendment saying so.
