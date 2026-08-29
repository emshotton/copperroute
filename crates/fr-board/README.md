# fr-board

A behavioral Rust port of freerouting's board-model package family (v2.3.0):
`board/model/{items,structure}`, `board/facade`, `board/searchtree`,
`board/trace`, `board/state`, `rules`, `core/library`, and the
board-model-relevant slice of `datastructures`. This is items, rules, the
component/net/padstack library, the search trees that index items
geometrically, and `Board` itself — everything a PCB design needs *before*
routing runs. Every method in scope is ported under its `snake_case` name in
Java's source order; deliberate Java bugs and edge-case crashes are
reproduced rather than fixed (see `docs/java-quirks.md`). Use
`fr_board::prelude::*` to bring in every public type.

This crate must not depend on `tracing` (`global-constraints.md`):
diagnostic `FRLogger` calls from the Java source are dropped during porting,
and invariant-guard logs become `debug_assert!`.

## What is *not* here

Everything interactive, observer-based, serialized, or belonging to a later
plan is marked in place rather than ported — search for `// not ported:`,
`// renamed:`, and `// added in Task N:`/`// added in Plan N:` throughout
`src/`. In outline:

- **No undo/redo.** Java's `UndoableObjects` stack (`BasicBoard.itemList`) is
  replaced by a plain `BTreeMap<ItemId, Item>` plus whole-board
  `Board::clone`/`Board::deep_copy` (`board/snapshot.rs`) standing in for
  `generateSnapshot`/`popSnapshot`/`undo`/`redo`.
- **No observers, no GUI, no serialization.** `Communication.observers`,
  `BasicBoard.startNotifyObservers`/`endNotifyObservers`, the Swing
  repaint-region bookkeeping (`updateBox`), `ItemSelectionFilter`, and every
  `writeObject`/`readObject` hook are dropped.
- **No back-pointers.** Java items read `this.board.library`,
  `this.board.rules`, etc. directly; every ported method that needs them
  takes an `ItemCtx` parameter instead.
- **Shove, forced-via, pull-tight, and the autoroute engine are later
  plans.** `RoutingBoard`'s shove/forced-via/pull-tight entry points land in
  Plan 7 as an extension trait (`RoutingBoardExt` in `fr-router`); the
  autoroute engine (`initAutoroute`, `autoroute`, `fanout`,
  `additionalUpdateAfterChange`) is Plan 6. `board/optimize/*` and
  `board/actions/*` are Plan 7/Plan 6/GUI.
- **I/O is Plan 3.** `IndentFileWriter`, `IdentifierType`, and the
  `specctra`/session file writers live in a future `io` crate.
- **Two `datastructures` classes live in `fr-geometry` instead.** `BigIntAux`
  and `Signum` are plain arithmetic helpers with no board dependency; they
  were ported alongside the geometry types that use them in Plan 1
  (`fr_geometry::bigint_aux`, `fr_geometry::Signum`).

`scripts/audit-port.sh` verifies this crate has zero unaccounted-for public
Java methods across all nine in-scope directories:

```sh
for dir in board/model/items board/model/structure board/facade \
           board/searchtree board/trace board/state rules core/library \
           datastructures; do
  ./scripts/audit-port.sh "$dir" crates/fr-board/src
done
```

Plan 5 Task 2 added a tenth, **per-class** invocation: `drc.ClearanceViolation`
lives here rather than in `fr-drc` (see the type mapping below), so its methods
are audited against the two files that hold them.

```sh
./scripts/audit-port.sh drc crates/fr-board/src 'ClearanceViolation.java' \
    scripts/audit-map/fr-drc.map
```

**What that zero does and does not prove.** The script's *positive* match is
crate-wide, not per class: for a Java `Foo.getBar`, it accepts any
`fn get_bar…` anywhere under `crates/fr-board/src`, with no check that the
`fn` it found belongs to the Rust counterpart of `Foo`. Java method names
repeat heavily across classes — 357 of the 750 distinct `class`/`method` pairs
these nine directories declare share their method name with at least one other
class in the same set — so for those the audit proves the name is ported
*somewhere*, collectively, rather than on the right type. It is still a real
check (a name nobody ported at all fails, and the `not ported:`/`renamed:`/
`added in Task N:`/`added in Plan N:` markers are exact), and the per-class
evidence for this crate is the Java citation in every ported body's doc
comment plus the differential drivers. Scoping the `fn` match to the Rust type
that stands in for each Java class is a **Plan 3 obligation**, recorded in
`docs/java-quirks.md`'s obligation table.

## Type mapping

| Java | Rust |
|---|---|
| `BasicBoard` + the non-shove half of `RoutingBoard` | `Board` |
| `Item` (abstract, 9 concrete subclasses) | `enum Item { Trace, Via, Pin, ObstacleArea, ConductionArea, ViaObstacleArea, ComponentObstacleArea, ComponentOutline, BoardOutline }` — see below |
| `Trace` + `PolylineTrace` (Java splits geometry across two classes) | one struct, `PolylineTrace` |
| `SearchTreeObject` | `enum TreeObject { Item(ItemId), Room(RoomId) }` — `RoomId` is reserved for Plan 6's autoroute expansion rooms |
| `ItemAutorouteInfo` | `struct AutorouteInfo` (`items/header.rs`) — the fields, as ids; the accessors are `fr-router`'s (plan-6 ruling 15) |
| `ShapeTree` (abstract) + `MinAreaTree` (its one concrete subclass) | one struct, `ShapeTree` |
| `ShapeSearchTree` | `ShapeSearchTree` (wraps a `ShapeTree<TreeObject>`) |
| `SearchTreeManager` | `SearchTreeManager` |
| `Connectable` (interface) | `Connectable` trait + `ConnectableRef` |
| `BoardRules` | `BoardRules` |
| `ClearanceMatrix` | `ClearanceMatrix` |
| `Nets`/`Net` | `Nets`/`Net` |
| `NetClasses`/`NetClass` | `NetClasses`/`NetClass` |
| `ViaInfos`/`ViaInfo`, `ViaRule` | `ViaInfos`/`ViaInfo`, `ViaRule` |
| `BoardLibrary` | `BoardLibrary` |
| `Packages`/`Package`/`PackagePin` | `Packages`/`Package`/`PackagePin` |
| `Padstacks`/`Padstack` | `Padstacks`/`Padstack` |
| `LogicalParts`/`LogicalPart`/`PartPin` | `LogicalParts`/`LogicalPart`/`PartPin` |
| `LayerStructure`/`Layer` | `LayerStructure`/`Layer` |
| `Components`/`Component` | `Components`/`Component` |
| `BoardOutline` | `BoardOutline` (also an `Item` variant — see `structure/board_outline.rs`) |
| `PlanarDelaunayTriangulation` | `PlanarDelaunayTriangulation` |
| `TimeLimit`/`Stoppable` | `TimeLimit`/`StopCheck` |
| `ItemIdGenerator implements IdGenerator` | `ItemIdGenerator` |
| `UndoableObjects` | not a type here — see "What is not here" |
| `BigIntAux`, `Signum` | not here — `fr_geometry::bigint_aux`, `fr_geometry::Signum` |
| `drc.ClearanceViolation` | `ClearanceViolation` (`items/clearance_violation.rs`) — the one `drc` type this crate declares, because `Item.clearanceViolations` returns it and `fr-board` cannot depend on `fr-drc` (plan-5 ruling 9); `fr-drc` re-exports it |

### The `Item` enum

`Item`'s nine variants are ordered as `Item.getBoardItemType` tests them
(Item.java:117-146): `Trace`, `Via`, `Pin`, `ObstacleArea`, `ConductionArea`,
`ViaObstacleArea`, `ComponentObstacleArea`, `ComponentOutline`,
`BoardOutline`. Every variant shares one `ItemHeader` (the base-class
state Java's `Item` gives every subclass for free: id, layer set, clearance
class, fixed state, net numbers, component number). Dispatch on the variant
replaces Java's virtual method calls; the `Item` inherent `impl` block in
`items/mod.rs` is the base-class method table, one function per
`Item.someMethod()`.

### `ItemCtx`

Not a Java type. Java items read `this.board.library`, `this.board.rules`,
`this.board.components`, and (rarely) `this.board.boundingBox` directly
through a back-pointer that `global-constraints.md` forbids here. `ItemCtx`
bundles the four immutable references a ported item method needs and is
passed as a parameter instead; `crate::board::item_ctx!` builds one from a
`&Board`. It is not `Board` itself — it excludes `items` and the search
trees, so an item method holding an `ItemCtx` cannot alias `Board::items`
while `Board` iterates it.

## Invariants

- **Descending item iteration for anything that mirrors a Java `itemList`
  walk.** Java's `board.itemList` is an `UndoableObjects` backed by a
  `ConcurrentSkipListMap<Storable, …>`, sorted by `Item.compareTo` — whose
  subtraction is reversed (`other.id - this.id`, quirk #44), so Java visits
  items in **descending id order**, deterministically. `Board::items` is a
  `BTreeMap<ItemId, Item>`, which iterates *ascending*; every port of a
  Java `itemList` walk therefore goes through
  `Board::items_in_board_order` (`items.values().rev()`), not a bare
  `.values()`. This is load-bearing, not cosmetic: `MinAreaTree`'s insertion
  heuristic makes the resulting tree shape a function of insertion order.
- **`OnceLock`, not `Cell`/`RefCell`, for Java's lazy memo fields**
  (`precalculatedAbsoluteArea`, convex-piece caches, drill-item tile-shape
  caches). Java recomputes-then-caches through a plain nullable field with
  no synchronization; the Rust port's memo fields are `OnceLock<T>` so
  `getArea`-equivalents can stay `&self` and the type stays `Send + Sync`
  without a lock, matching how the board is used (never mutably aliased
  while being read).
- **`ShapeTree::insert_tiles`/`insert_tiles_opt` return nullable entries.**
  Java's `ShapeTree.insert(Storable)` can return a `Leaf[]` with `null`
  holes — a shape with no bounding tile shape in the tree's angle
  restriction inserts nothing at that index but keeps the array the same
  length as the shape list. `insert_tiles` returns `Vec<Option<LeafId>>` for
  exactly this reason; do not `.flatten()` it away, since callers that
  zip it against the original shape list need the holes to line up.
- **No `Board` back-pointers anywhere** (see `ItemCtx` above and the "What
  is not here" list) — every item method that needs board state takes it as
  a parameter.
- **`Polyline::from_lines` is `Result<_, PolylineError>`, never silently
  swallowed.** Any Rust operation that re-runs the normalising `Polyline`
  constructor (`translate_by`, `turn_90_degree`, `change_placement_side`,
  trace normalisation, `PolylineTrace::split*`) threads the `Result` out
  rather than emptying the trace on failure; inside `Board::normalize_trace`
  an `Err` becomes `BoardError::Normalization`, which must propagate, not
  get discarded.

## Differential drivers

`scripts/differential/` holds Java-vs-Rust drivers that print one line of
state per call from seeded/scripted inputs on both languages, so the two
outputs can be diffed byte-for-byte. The board-model-relevant drivers
(all documented in detail in `scripts/differential/README.md`):

| Driver | Covers | Twin |
|---|---|---|
| `P2T3.java` / `p2t3` | `ShapeTree`/`MinAreaTree`, fixed 8-box insert/query/remove script | `p2t3` |
| `P2T3R.java` / `p2t3r` | `ShapeTree`/`MinAreaTree`, randomised insert/re-key/remove, both angle restrictions | `p2t3r` |
| `P2T10.java` / `p2t10` | `ShapeSearchTree`/`SearchTreeManager` — every public method except `completeShape`/`divideLargeRoom` (Plan 6), across 9 modes (angle restrictions, clearance matrix, areas/outline branches, entry-surgery, tie-pin reduction, skewed outlines) | `p2t10` |
| `P2T11.java` / `p2t11` | the real `RoutingBoard` — insert/remove protocol, connectivity, `checkTraceSegment`, changed area, conduction latch, `ShapeTraceEntries`, cycles/overlaps, `PolylineTrace.combine`/`split`/`normalize`, and the tree-rebuild-vs-clone divergence around `deepCopy`, across 12 modes | `p2t11` |
| `P2T13.java` / `p2t13` | `PlanarDelaunayTriangulation` — random points, square, collinear triple, degenerate edges, two-corner objects, grid, tiny range, circle, across 8 modes | `p2t13` |
| `P2T15.java` / `p2t15` | the ONE randomised board-level driver (Task 15) — `n` random pins/vias/traces plus fixed obstacle/conduction areas through the real `RoutingBoard`/`Board`, `normalizeAllTraces` (which *increases* the trace count here by splitting at same-net crossings, not folding), every item's fields, 150 overlap/clearance queries, `deepCopy` then a full re-dump plus the same queries replayed against it, and a `hashEqual` boolean | `p2t15` |

Requirements: JDK ≥ 23 (`JAVA_HOME`) for most drivers; `p2t10`, `p2t11`,
`p2t13` and `p2t15` additionally need a **JDK 25** (`JAVA25_HOME`), and
`p2t10`/`p2t11`/`p2t15` need the clone's built jar (`FREEROUTING_JAR`, from
`./gradlew build` in a sibling `../freerouting` checkout).

```sh
./scripts/differential/run.sh p2t3                    # fixed script, no arguments
./scripts/differential/run.sh p2t3r 400 42 0           # 400 ops, seed 42, orthogonal bounds
./scripts/differential/run.sh p2t3r 2000 42 1          # ...45-degree bounds
./scripts/differential/run.sh p2t10 0                  # mode 0 of 8 (45-degree board)
./scripts/differential/run.sh p2t11 0                  # mode 0 of 12 (insert/remove, item-list, queries)
./scripts/differential/run.sh p2t13 50 42 0            # 50 points, seed 42, mode 0 (random)
./scripts/differential/run.sh p2t15 42 30              # seed 42, 30 random items (the default)
```

All modes of all six drivers match Java exactly at HEAD except `p2t11`
mode 11, whose two diffing fields (`treeArrayCopy`/`treeArraysEqual`) are
the documented, deliberate divergence between Java's tree-rebuild-on-`clone`
and this port's tree-clone-on-`deep_copy` (Task 12) — every other field on
that mode matches, including `hashEqual` and `diffTraces`. `p2t15` is
zero-diff at every one of 10 seeds × `n` ∈ {30, 120} (`scripts/differential/README.md`
has the full sweep table), and `crates/fr-board/tests/consistency.rs` covers
the same properties (insert/remove round trips, `deep_copy`, 45- vs.
90-degree tile shapes — compared with `TileShape::contains_tile`, not
bounding boxes, since both angle families compute the same axis-aligned
`bounding_box()` — `normalize_traces` idempotence on a fixture that actually
needs normalising, descending item iteration, `Board: Send + Sync + Clone`)
as fast, seeded unit tests rather than a JVM-diffing driver.

## Package-private classes

Three Java classes in `board/trace` are package-private with **no `public`
members**, so `audit-port.sh` enumerates zero methods for each and exits 0
whether or not anything was ported — the audit is vacuous evidence for
these three specifically. This section is the real evidence.

### `PolylineTraceGeometry.java` (67 lines, all package-private)

Eleven one-liner methods, each inlined directly into the `PolylineTrace`
body (`crates/fr-board/src/items/trace.rs`) that calls it, with the Java
line number in the doc comment:

| Java method | Lines | Rust |
|---|---|---|
| `firstCorner` | 23-25 | `PolylineTrace::first_corner` |
| `lastCorner` | 27-29 | `PolylineTrace::last_corner` |
| `cornerCount` | 31-33 | `PolylineTrace::corner_count` |
| `length` | 35-37 | `PolylineTrace::get_length` |
| `boundingBox` | 39-41 | `PolylineTrace::bounding_box` |
| `tileShapeCount` | 43-45 | `PolylineTrace::tile_shape_count` |
| `translate` | 47-49 | `PolylineTrace::translate_by` |
| `turn90Degree` | 51-53 | `PolylineTrace::turn_90_degree` |
| `rotateApprox` | 55-57 | `PolylineTrace::rotate_approx` |
| `mirrorVertical` | 59-61 | `PolylineTrace::change_placement_side` |
| `connectionShape` | 63-66 | `Connectable::get_trace_connection_shape` |

### `PolylineTraceSearchTreeAdapter.java` (67 lines, all package-private)

Six stateless methods forwarding to `board.searchTreeManager`:

| Java method | Rust |
|---|---|
| `calculateTreeShapes` | `ShapeSearchTree::calculate_tree_shapes` |
| `hasDefaultEntries` | `Board::trace_has_default_entries` |
| `replaceGeometry` | `Board::replace_trace_geometry` |
| `mergeEntriesInFront` | `Board::merge_trace_entries_in_front` |
| `mergeEntriesAtEnd` | `Board::merge_trace_entries_at_end` |
| `changeEntries` | `Board::change_trace_entries` |

### `PolylineTraceNormalization.java` (133 lines, package-private)

One `static` entry point (`normalize(PolylineTrace, IntOctagon)`) and its
recursive private helper (`normalize(PolylineTrace, IntOctagon, int)`,
depth-limited by `MAX_NORMALIZATION_DEPTH = 16`); both are ported as
`Board::normalize_trace`/an internal recursive helper in
`crates/fr-board/src/board/trace_normalize.rs`, whose module doc walks the
whole class line-by-line.

No other Java class across the nine audited directories declares a class
without `public` on it — verified by grepping every `class` declaration
line in each directory for a missing `public` modifier.
