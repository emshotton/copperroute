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

## State: Task 1 of 18

What exists is the data-model floor the other seventeen tasks build on:

| Item | Where | Java |
| --- | --- | --- |
| `Arena<T>` | `src/arena.rs` | — (ruling 16) |
| `AutorouteAttemptState` | `src/autoroute/attempt.rs` | `AutorouteAttemptState.java:1-14` |
| `AutorouteAttemptResult` | `src/autoroute/attempt.rs` | `AutorouteAttemptResult.java:1-25` |
| `ItemAutorouteInfo`'s accessors | `src/autoroute/item_info.rs` | `ItemAutorouteInfo.java:10-105` |
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

Everything else — the maze search, the expansion rooms, the drill pages, the
path locators, `RoutingBoardExt` — arrives in Tasks 2-17. The deferral roster
at the foot of `src/lib.rs` names each class and the task or plan that owns it;
`grep -rn "added in Task" crates/fr-router/src` lists what is still owed.

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
  `fr-board`, `fr-dsn`, `fr-geometry`, `fr-settings` and `thiserror`, and on
  nothing else. Not `rand` (see `JavaRandom` above), not `slotmap` (see
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
this crate ports (ruling 13). As of Task 1 the package-root invocation reaches
**zero MISSING and zero UNMAPPED**, over this task's three classes and over the
whole package root:

```sh
./scripts/audit-port.sh autoroute crates/fr-router/src \
    'AutorouteAttemptState.java AutorouteAttemptResult.java ItemAutorouteInfo.java' \
    scripts/audit-map/fr-router.map
./scripts/audit-port.sh autoroute crates/fr-router/src '*.java' \
    scripts/audit-map/fr-router.map
```

The map's header lists the six further invocations — one per subpackage, plus
`board/actions` and `board/optimize` — that Tasks 2-17 fill in and Task 18 must
drive to zero.
