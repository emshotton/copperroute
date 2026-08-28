# Java-vs-Rust differential harness

This directory preserves the harness used while porting `geometry/planar`
(Tasks 14-17): a small Java driver and a Rust "twin" binary that walk the same
seeded pseudo-random inputs through corresponding Java and Rust methods and
print one line of state per call. Diffing the two outputs is far more
effective at finding porting mistakes than hand-tracing, especially for
methods with dozens of branches.

## Layout

- `java/` — driver `.java` files, copied verbatim from the scratch harness
  reviewers built during Tasks 14-17. Each declares
  `package app.freerouting.geometry.planar;` and is compiled *alongside* the
  real, unmodified sources from the sibling `../freerouting` checkout — nothing
  from that repo is copied here.
  - `T14.java` — `Simplex` (Task 14). Twin: `t14`.
  - `T15.java` — `LineSegment` (Task 15). Twin: `t15`.
  - `T16R.java` — `Polyline` (Task 16). Twin: `t16r`.
  - `D17.java` — `PolygonShape` / `PolylineArea` / `Circle` (Task 17). Twin: `d17`.
  - `E15.java` — `LineSegment.stairApproximation` edge cases (Task 15). Twin: `e15`.
  - `P2T3.java` — `ShapeTree`/`MinAreaTree`, fixed 8-box script (Plan 2 Task 3).
    Twin: `p2t3`. `crates/fr-board/tests/min_area_tree.rs` was written from this
    driver's output. Declares `package app.freerouting.datastructures;` and is
    compiled alongside the real
    `datastructures/{ShapeTree,MinAreaTree,ArrayStack}.java`.
  - `P2T10.java` — `ShapeSearchTree`/`SearchTreeManager` (Plan 2 Task 10).
    Twin: `p2t10`. The **only** driver that is not compiled from
    `geometry/planar` sources: it builds a real
    `app.freerouting.board.facade.BasicBoard` (two layers, a two-pin component,
    two traces, an empty outline), so it drags in the whole board stack and is
    compiled and run against the clone's own build output —
    `../freerouting/build/libs/freerouting-current-executable.jar`, class-file
    version 69, which needs a **JDK 25** (`JAVA25_HOME`). It declares
    `package app.freerouting.datastructures;` so it can read the protected
    `ShapeTree.TreeNode.boundingShape` off each leaf. Prints, per tree: the
    key, `toArray()` with leaf bounds, every item's precalculated tree shapes,
    and the results of `overlappingObjects` /`overlappingTreeEntries` /
    `overlappingTreeEntriesWithClearance` / `overlappingItemsWithClearance`;
    mode 3 adds the clearance matrix, `clearanceCompensationValue`,
    `changeItemShape`, `changeEntries` and `setClearanceCompensationUsed`, and
    mode 4 covers obstacle/conduction areas, both `BoardOutline` branches and
    the synthesised drill-hole obstacle, mode 5 the three entry-surgery methods
    mode 6 `reduceTraceShapeAtTiePin` and mode 7 a skewed outline whose line
    bands are `Simplex`es. Together the eight modes reach every public
    `ShapeSearchTree` method except `completeShape`/`divideLargeRoom`.
    `crates/fr-board/tests/{board_builder,search_tree}.rs` were written from
    this driver's output.
  - `P2T13.java` — `PlanarDelaunayTriangulation` (Plan 2 Task 13). Twin:
    `p2t13`. Declares `package app.freerouting.datastructures;` and is compiled
    alongside the real `datastructures/PlanarDelaunayTriangulation.java` — and,
    uniquely among the source-path drivers, with a **JDK 25** (`JAVA25_HOME`),
    because its ground truth depends on `java.util.Random` and
    `java.util.Collections.shuffle`, which are runtime-library code rather than
    freerouting code. Prints the input corners, the permutation the class's
    fixed-seed shuffle produces, `getEdgeLines()` in iteration order and
    `validate()`.
  - `P2T15.java` — the ONE randomised board-level driver (Plan 2 Task 15).
    Twin: `p2t15`. Declares `package app.freerouting.datastructures;` and,
    like `P2T10.java`/`P2T11.java`, is compiled and run against the clone's own
    `../freerouting/build/libs/freerouting-current-executable.jar` on a
    **JDK 25** (`JAVA25_HOME`) rather than from source, because it needs the
    whole board stack. Builds a real
    `app.freerouting.board.facade.RoutingBoard` — two layers, two padstacks
    (`smd` layer-0-only, `thru` both layers) making up one two-pin package,
    four nets, two clearance classes (`1` default, `2` "wide") — then drives a
    shared xorshift stream (the same `state ^= state<<13; ^= state>>>7;
    ^= state<<17;` generator every other randomised driver uses) through `n`
    random `insertPin`/`insertVia`/`insertTraceWithoutCleaning` calls plus
    three fixed obstacle areas and three fixed conduction areas, calls
    `normalizeAllTraces`, and dumps: every item in board order (descending
    id, quirk #63) — id, kind, layer range, nets, clearance class, bounding
    box, tile shape count and each tile's bounding box; the default tree's
    `overlappingObjects` for 50 random query boxes on each of the two layers;
    `overlappingTreeEntriesWithClearance` as `(id, shapeIndex)` lists for 50
    random shapes/layers/clearance classes/ignore-net arrays; `deepCopy`, then
    the same 150 queries replayed against the copy; and a `hashEqual` boolean
    (`getHash().equals(...)` vs. `structural_hash() == structural_hash()` —
    compared as booleans, not values, because the two hash algorithms are not
    byte-comparable, `docs/java-quirks.md`). Deliberately never prints
    `toArray()`/tree internals (unlike `P2T10`/`P2T11`, which reach into
    `ShapeTree`'s protected fields for exactly that reason) — every query goes
    through public API, so quirk #77's tree-layout divergence never appears
    and the driver is zero-diff. `run.sh p2t15 <seed> <n>` diffs it against
    `p2t15.rs`.
  - `P2T3R.java` — `ShapeTree`/`MinAreaTree`, randomised (Plan 2 Task 3).
    Twin: `p2t3r`. Drives `insert(Storable)` (so the *tree* applies its bounding
    directions), `remove(Leaf[])` on arrays with deliberate `null` holes,
    in-place re-keying of live leaves the way `ShapeSearchTree` does, plus
    `overlaps`, `toArray` and `distanceToRoot`. Same `datastructures` sources as
    `P2T3.java`.
  - `support/FRLogger.java` — a minimal stand-in for
    `app.freerouting.logger.FRLogger` (the real class pulls in the Log4j/board
    stack that has nothing to do with geometry). Reproduces the handful of
    `warn`/`debug`/`trace`/`error` overloads the planar package calls.
  - `historical/` — earlier, superseded exploration scripts kept for
    provenance only. `T6.java` and `T6b.java` (Plan 2 Task 6) inline the bodies
    of `Pin.getShape`, `Pin.relativeLocation`, `Pin.getTraceExitRestrictions`,
    `Pin.nearestTraceExitCorner` and `Pin.calcNearestExitRestrictionDirection`
    over the *real* `geometry/planar` classes — `Pin` itself cannot be compiled
    standalone, since it drags in `BasicBoard` and the whole board stack — and
    print the values that `crates/fr-board/tests/drill_items.rs` asserts. Run
    them from *this* directory with
    `JD=../../../freerouting/src/main/java; javac -d out -sourcepath $JD \
      $JD/app/freerouting/geometry/planar/*.java java/support/FRLogger.java \
      java/historical/T6.java java/historical/T6b.java && \
      java -cp out app.freerouting.geometry.planar.T6`.
    `T7.java` and `T7b.java` (Plan 2 Task 7) do the same for
    `ObstacleArea.getArea`, `ObstacleArea.splitToConvex`, the three
    `ObstacleArea` transform bodies and `BoardOutline.getKeepoutArea`, and print
    the tile counts and bounding boxes that
    `crates/fr-board/tests/areas_and_outlines.rs` asserts (`ObstacleArea` and
    `BoardOutline` also drag in `BasicBoard`, so their bodies are inlined too).
    Same command, with `java/historical/T7.java java/historical/T7b.java` and
    `app.freerouting.geometry.planar.T7`.
    `T8.java` (Plan 2 Task 8) does the same for the whole of
    `board/trace/PolylineTraceGeometry.java`, `Trace.nearestEndPoint` and the
    geometry core of `PolylineTrace.split(Point)`, and prints the corners,
    lengths, bounding boxes, tile/offset/connection shapes and split pieces that
    `crates/fr-board/tests/polyline_trace.rs` asserts (`PolylineTrace` also drags
    in `BasicBoard`, so its bodies are inlined too). Same command, with
    `java/historical/T8.java` and `app.freerouting.geometry.planar.T8`.
    The older ones: `T17.java` and `T17b.java` are fixed print-statement
    dumps written before `D17.java` existed (no seeded/diffable format, no
    Rust twin). `RD.java`, `RD2.java` were `D17.java`'s drafts. `RV17.java`
    and `R.java` pin `java.util.Random(99).nextInt(bound)` sequences used
    elsewhere in the Java tree. (When they were written nothing in the Rust
    port re-implemented `java.util.Random`; two ports do now —
    `fr-geometry`'s `polygon_shape.rs` and `fr-board`'s `delaunay.rs` — and
    `P2T13.java`/`p2t13` diff the shuffle permutation those depend on
    directly, as the `perm=` line of their output.) None of these are wired
    into `run.sh`.
- `rust/` — a standalone Cargo package, `fr-geometry-differential`, **not** a
  member of the repo's workspace (see the root `Cargo.toml` `exclude` and this
  package's own `[workspace]` table). It depends on `fr-geometry` and
  `fr-board` by path and builds one `[[bin]]` per twin: `t14`, `t15`, `t16r`,
  `e15`, `d17`, `p2t3`, `p2t3r`, `p2t10`, `p2t11`, `p2t13`, `p2t15`.
- `run.sh <driver> [args...]` — compiles the requested Java driver against
  the real sources, builds the matching Rust binary, runs both (passing
  `args` through unchanged to each side, or a per-driver default smoke run
  if none are given), and diffs stdout.

## Running it

Requirements:
- JDK ≥ 23 (`javac`/`java`); set `JAVA_HOME`, or edit the default at the top
  of `run.sh` (it defaults to a Homebrew JDK 23 install).
- A sibling checkout of the Java repo at `../freerouting` relative to this
  repo's root (override with `FREEROUTING_JAVA_DIR`).
- For `p2t13`: a **JDK 25** (`JAVA25_HOME`) — it compiles from the
  `geometry/planar` sources like the other source-path drivers, but on the JDK
  the shipping jar targets, because its ground truth includes `java.util.Random`
  and `java.util.Collections.shuffle`.
- For `p2t10`/`p2t11`/`p2t15` only: a **JDK 25** (`JAVA25_HOME`) and the clone's built jar at
  `../freerouting/build/libs/freerouting-current-executable.jar`
  (`FREEROUTING_JAR`). Run `./gradlew build` in the clone if it is missing.

```sh
./scripts/differential/run.sh t15               # LineSegment, default smoke run (200 iters, seed 42)
./scripts/differential/run.sh t15 2000 12345    # LineSegment, 2000 iters, seed 12345
./scripts/differential/run.sh d17               # PolygonShape/PolylineArea/Circle, mode 0
./scripts/differential/run.sh d17 200 2         # ...mode 2 (Circle)
```

Extra arguments **replace** the whole default list, not just one field — for
`t15` that means passing a bare iteration count on its own
(`run.sh t15 2000`) leaves the required `seed` argument missing and Java
exits with `ArrayIndexOutOfBoundsException`. Pass every positional argument
the driver expects, or none at all.

`run.sh`:
1. Compiles the driver together with the real `geometry/planar/*.java`, the
   three `datastructures/{Signum,BigIntAux,Stoppable}.java` files it needs,
   any driver-specific extra sources (`p2t3`/`p2t3r` add
   `datastructures/{ShapeTree,MinAreaTree,ArrayStack}.java`), and the local
   `FRLogger` stand-in, into `build/classes/`.
2. Builds the matching Rust binary (`cargo build --release --bin <driver>`
   inside `rust/`).
3. Runs both, passing through whatever arguments follow `<driver>` on the
   command line to *both* sides unchanged, and diffs `build/<driver>.j.out`
   against `build/<driver>.r.out`. With no extra arguments each driver falls
   back to a fixed default — a **smoke run**, not full coverage — defined at
   the top of `run.sh`: `t14 200`, `t15 200 42`, `t16r 200 42 0`, `e15` (no
   arguments), `d17 200 0`.

`build/` and `rust/target/` are gitignored scratch output.

### Per-driver arguments and mode coverage

- `t14 <iters>` — `Simplex`. No mode/seed argument (the seed is a fixed
  constant baked into both the Java and Rust generators).
- `t15 <iters> <seed> [c]` — `LineSegment`. `seed` and the optional shape
  constant `c` are both read on the Java and the Rust side.
- `t16r <iters> <seed> <mode>` — `Polyline`. Modes `0`, `1`, and `4` all
  exercise the same general polyline-fuzzing path with different point-count
  profiles (verified: all three reproduce the same category of diffs, see
  below); mode `3` exercises a distinct, smaller "pool of shared points"
  scenario and — verified — matches Java exactly (0 diffs over 400 lines at
  seed 42).
- `e15` — `LineSegment.stairApproximation` edge cases. No arguments; it's a
  fixed sequence of edge-case calls, not seeded generation.
- `p2t3` — `ShapeTree`/`MinAreaTree`. No arguments; a fixed 8-box insert/query/
  remove script plus tie-break and edge cases.
- `p2t3r <ops> <seed> <mode> [dumpEvery] [insertPct]` — `ShapeTree`/`MinAreaTree`,
  randomised. `mode` is `0` for orthogonal (`IntBox`) bounds and `1` for
  45-degree (`IntOctagon`) bounds — run **both**, since only mode `1` proves the
  tree applies its own bounding directions rather than storing the box it was
  handed. `dumpEvery` (default 1) prints the whole tree every N ops; `insertPct`
  (default 62) is the share of insert ops, the rest split between re-keying and
  removal.
- `p2t10 <mode>` — `ShapeSearchTree`/`SearchTreeManager`. `mode` is `0` for a
  45-degree board, `1` for 90-degree, `2` for no angle restriction, `3` for the
  clearance matrix plus `changeItemShape`/`changeEntries`/
  `setClearanceCompensationUsed`, `4` for a three-layer board with a real
  outline polygon, an obstacle area, a conduction area and a via whose middle
  layer has no pad, `5` for `mergeEntriesAtEnd`/`mergeEntriesInFront`/
  `reuseEntriesAfterCutout`, `6` for `reduceTraceShapeAtTiePin`, and `7` for an
  outline whose edges run in none of the trees' directions (so the line bands
  are `Simplex`es the 45-degree override has to regularise). Run **all eight**:
  between them they reach every public `ShapeSearchTree` method except
  `completeShape`/`divideLargeRoom` (Plan 6). Modes 0-2 differ only in which
  subclass `getAutorouteTree` builds, which is the whole point of the
  angle-parameterised port.
- `p2t13 <n> <seed> <mode>` — `PlanarDelaunayTriangulation`. `mode` is `0` for
  `n` random points (one single-corner object each, coordinates in
  `[-100000, 100000)`), `1` for the four corners of a square, `2` for a
  collinear triple, `3` for a square plus three coincident interior points
  (the degenerate-edge path), `4` for `n` objects of *two* corners each (so
  `split`'s same-object short-circuit at line 157 is reachable), `5` for a 5x5
  integer grid (maximal cocircularity and collinearity), `6` for `n` random
  points in `[-20, 20)` (duplicates and collinear triples at a high rate), and
  `7` for `n` points on a circle of radius 30000 (every point cocircular, so
  every flip is decided by `insideCircle`'s `- 1.0` tolerance). All eight modes
  match exactly; verified additionally at `n` from 5 to 400 over seeds
  1/2/3/7/42/555/77/12345/999983/20260828.
- `d17 <cases> <mode>` — `PolygonShape`/`PolylineArea`/`Circle`. Java's
  `D17.java` only branches explicitly on mode `0` (polygon) and `1`
  (polyline area); every other mode value, including `2`, falls through to
  its final `else`, which is the `Circle` case (`D17.java`'s own comment
  reads `// 0 = polygon, 1 = polyline area, 2 = circle`, but `2` isn't a
  distinct branch — it's just what falls through). `d17.rs` mirrors that
  exact structure: explicit `mode == 0`/`mode == 1` branches, then an
  unconditional `else` that is `Circle`. **Verified** by running
  `run.sh d17 200 <mode>` for `mode` in `0`, `1`, `2`: all three **match
  exactly**. The one mode that does *not* match is `3` — see "Harness
  maintenance note" below; Java's placeholder for the never-real
  `splitPiecesForDiff` branch and Rust's `Circle` fallthrough disagree, so
  avoid `mode 3` when running `d17` by hand.
- `p2t15 <seed> <n>` — the ONE randomised board-level driver (Plan 2 Task
  15). `n` random pins/vias/traces plus three fixed obstacle areas and three
  fixed conduction areas, `normalizeAllTraces`, then every item, 100
  `overlappingObjects` queries (50 per layer), 50
  `overlappingTreeEntriesWithClearance` queries, the same 150 queries
  replayed against a `deepCopy`, and a `hashEqual` boolean. No mode argument:
  every run exercises the same mix. **Verified** at 10 seeds
  (1/2/3/7/42/555/77/12345/999983/20260828) × `n` in `{30, 120}` — all 20
  runs **match exactly**, 402-708 lines each depending on how normalisation
  folds the random traces together. Also re-verified that this sweep does
  not regress `p2t10` (all 9 modes), `p2t11` (all 11 modes reached by the
  differential driver — mode 11's one documented divergence unchanged) or
  `p2t13` (mode 0).

## Known, expected diffs

Verified at HEAD, default smoke-run arguments, JDK 23 — except `p2t10`,
`p2t11`, `p2t13` and `p2t15`, which need a JDK 25 (see Requirements above):

| driver | lines | diff lines | classification |
|---|---|---|---|
| `d17` (mode 0) | 200 | 0 | exact match |
| `e15` | 16 | 3 | `LineSegment.stairApproximation`/`45` on a non-finite width (quirk #21) |
| `t15` (seed 42) | 11747 | 44 | 42 cosmetic (`EXC:ArithmeticException` vs `EXC:panic`, see below) + 2 sign-of-zero in `LineSegment.startPointApprox`/`endPointApprox` (quirk #14) |
| `t16r` (mode 0, seed 42) | 9200 | 72 | all `lineSegment`/`offsetBox` fields: `LineSegment(Polyline, no)` with `no` out of its valid range (Java constructs a degenerate object with null internal lines that later NPEs; Rust's `LineSegment::from_polyline`/`Polyline::offset_box` return `None` up front — the `offsetBox` case is the `Polyline.offsetBox(halfWidth, no)` row in the `totalized` table) |
| `t14` | 17997 | 145 | `Simplex.EMPTY` / degenerate-shape edge cases already in the `totalized` table |
| `p2t3` | 132 | 0 | exact match |
| `p2t3r` (400 42 0) | 109940 | 0 | exact match |
| `p2t3r` (2000 42 1) | 2898938 | 0 | exact match (45-degree/`IntOctagon` bounds) |
| `p2t10` (mode 0) | 88 | 0 | exact match |
| `p2t10` (mode 1) | 88 | 0 | exact match (90-degree, `IntBox`-keyed tree) |
| `p2t10` (mode 2) | 88 | 0 | exact match (no angle restriction, base-class `enlarge`) |
| `p2t10` (mode 3) | 68 | 0 | exact match (mutators + clearance compensation) |
| `p2t10` (mode 4) | 80 | 0 | exact match (areas, both outline branches, drill-hole obstacle) |
| `p2t10` (mode 5) | 54 | 0 | exact match (the three entry-surgery methods) |
| `p2t10` (mode 6) | 33 | 0 | exact match (`reduceTraceShapeAtTiePin`) |
| `p2t10` (mode 7) | 17 | 0 | exact match (skewed outline: `Simplex` bands regularised) |
| `p2t10` (mode 8) | 39 | 0 | exact match (the `changeOrder` half of both merges: head-to-head, tail-to-tail) |
| `p2t11` (mode 0) | 59 | 0 | exact match (insert/remove protocol, item-list and search queries) |
| `p2t11` (mode 1) | 66 | 0 | exact match (the connectivity family) |
| `p2t11` (mode 2) | 30 | 0 | exact match (`checkTraceSegment`, the check queries, and the id `checkPolylineTrace` consumes) |
| `p2t11` (mode 3) | 41 | 0 | exact match (changed area, conduction latch, `moveBy`, the cold shape cache, net queries) |
| `p2t11` (mode 4) | 20 | 0 | exact match (host-CAD section width, clearance compensation, 90-degree checks) |
| `p2t11` (mode 5) | 31 | 0 | exact match (`ShapeTraceEntries`, `ShapeEntrySide`, `ShapeAndEntrySide`) |
| `p2t11` (mode 6) | 24 | 0 | exact match (cycles/overlaps, `removeIfCycle`, the remaining inserters) |
| `p2t11` (mode 7) | 35 | 0 | exact match (`PolylineTrace.combine`, both halves, both orders, every refusal) |
| `p2t11` (mode 8) | 47 | 0 | exact match (`split(IntOctagon)`, `change`, `normalize`, and quirk #22 out of `combineAtStart`) |
| `p2t11` (mode 9) | 35 | 0 | exact match (`combineTraces`/`normalizeTraces`/`normalizeAllTraces`/`splitTraces` and the five callers that end in one of them) |
| `p2t11` (mode 10) | 5 | 0 | exact match (the 4000-segment `CombineStackOverflowTest` fixture, rebuilt by hand) |
| `p2t11` (mode 11) | 20 | 2 | `treeArrayCopy`/`treeArraysEqual` only — the documented tree-rebuild-vs-clone divergence (Task 12, see below); every `transientBefore`/`transientOriginalAfterCopy`/`transientCopy`/`overlappingObjects`/`hashEqual`/`diffTraces` line matches |
| `p2t13` (mode 0, 50 points) | 141 | 0 | exact match |
| `p2t13` (modes 1-7, `30 7 <mode>`) | 7-172 | 0 | exact match (square, collinear triple, duplicates, two-corner objects, grid, tiny range, circle) |
| `p2t15` (seed 42, n=30, default) | 413 | 0 | exact match |
| `p2t15` (10 seeds × n∈{30,120}) | 402-708 | 0 | exact match at every one of the 20 seed/n combinations (see "`p2t15` sweep" below) |

Every diff line traces to an already-documented, deliberate divergence in
`docs/java-quirks.md`'s `pinned`/`totalized` tables, plus one purely cosmetic
difference that recurs across every driver: the Java drivers tag a caught
exception with its real class name (`EXC:ArithmeticException`,
`EXC:NullPointerException`, ...), while the Rust drivers catch a panic via
`catch_unwind` and can only report the generic `EXC:panic` (a Rust panic
doesn't carry a Java-style exception type). Neither is a regression; re-run
after any future `geometry/planar` change and compare new diff lines against
`docs/java-quirks.md` before treating them as bugs — and re-run with a
different seed/iteration count (not just the smoke-run default) for anything
touching the classes above, since the default only samples a few hundred
cases.

## A finding from `p2t10` (Plan 2 Task 10)

`board.itemList` iterates in **descending item id**, deterministically:
`UndoableObjects` stores its objects in a `ConcurrentSkipListMap<Storable, …>`
(UndoableObjects.java:21,37), a *sorted* map keyed by `Item.compareTo`, whose
subtraction is reversed (Item.java:98, quirk #44). Printing both
`board.itemList` and `board.getItems()` for the driver's board gives `5 4 3 2 1`
on every run. That order decides the *structure* of any tree built by
`SearchTreeManager.getAutorouteTree` or rebuilt by
`setClearanceCompensationUsed`, so the port must reproduce it — see the
`SearchTreeManager` docs. (Plan 2's Task 16 brief describes the item order as
"`ConcurrentHashMap` (JVM-dependent)"; for `UndoableObjects` that is not the
case.)

## `p2t11` (Plan 2 Task 11)

`p2t11` drives the *real* `app.freerouting.board.facade.RoutingBoard` and
`fr-board`'s `Board` over the same two-layer board — a 5000-square outline, a
two-pin component (SMD pad on layer 0, through pad on both), two traces, a via
joining them, an obstacle area and a conduction area — and prints every public
query's answer. Like `p2t10` it needs a JDK 25 (`JAVA25_HOME`) and the clone's
own `build/libs/freerouting-current-executable.jar` (`FREEROUTING_JAR`),
because it compiles against the built board stack rather than against the
`geometry/planar` sources.

The twelve modes cover:

* **0** — `BasicBoard`'s constructor (which inserts the `BoardOutline` as item
  1), the id the generator hands each typed inserter, `revision` after every
  step, all eleven item-list queries, the three overlap queries, and the
  removal protocol including the refusal to delete a `SYSTEM_FIXED` outline.
* **1** — `getNormalContacts`/`getAllContacts`/`getConnectedSet`/
  `getUnconnectedSet`/`getConnectionItems`/`normalContactPoint`/
  `getRatsnestCorners`/`isTail`/`isOverlap`/`isCycle`/`isFanoutVia`/
  `getConnectedSets`/`touchingPinsAtEndCorners`/`validate`, over a
  pin-trace-via-trace-pin chain that crosses layers.
* **2** — `checkTraceSegment` (free, blocked, own-net, foreign-net, degenerate,
  shove-filtered and wide-clearance), `checkShape`, `checkTraceShape` with and
  without a contact-pin set, `checkPolylineTrace`, `checkMoveItem`,
  `checkChangeNet`, `pickNearestRoutingItem`, `getTraceTail` — and the id
  sequence, because `checkPolylineTrace`'s temporary `PolylineTrace` draws one
  from the generator even though it is never inserted (Item.java:85-90).
* **4** — a 90-degree board whose `Communication` names a host CAD system at
  resolution 10, so `ShapeSearchTree.calculateTreeShapes(ObstacleArea)`'s
  section width drops from 50000 to `min(500 * 10, 50000) = 5000`
  (ShapeSearchTree.java:916-920) and an 18000-wide area comes back in four
  pieces; then `setClearanceCompensationUsed(true)` and the check queries
  again, which is what pins `checkPolylineTrace` taking its temporary trace's
  tile shapes from the tree (compensated) rather than from the bare polyline.
* **3** — the `ChangedArea` lifecycle, `changeConductionIsObstacle`'s latch
  (quirk #50), `unfillConductionAreas`, `moveBy`, `changeClearanceClassIndex`
  *followed immediately by two queries* (which is what proves `getTreeShape`
  recomputes a cache that `clearDerivedData` emptied without a re-insert),
  `makeConductive`, `generateKeepoutOutside` and the five `Net` board queries.

* **5** — the shove-support classes Plan 7 needs ready-made: all four
  `ShapeEntrySide` constructors, `ShapeAndEntrySide` in every combination of
  `orthogonal`/`inShoveCheck` (which is where quirk #7's `borderLineIndex`
  stub is visible — the cut lines are found, the index is not, so `fromSide`
  falls through to the polyline branch and picks up `intersectionApprox`'s
  parallel sentinel), and `ShapeTraceEntries.storeItems` /
  `nextSubstituteTracePiece` / `cutoutTrace` / `cutoutTraces` over three traces
  crossing one square.

* **6** — a board with a genuine cycle (two traces between one pair of vias)
  and a trace whose two ends both sit inside one conduction area, so
  `isOverlap`/`isCycle`/`removeIfCycle` have a positive case; then
  `reduceNetsOfRouteItems` (which reduces a net and still returns `false`,
  quirk #66), `deleteAllTracksAndVias`, and the five typed inserters modes 0-5
  do not reach — the escape via, the via keepout, the two component-owned
  overloads and the component outline.

Modes 7-10 are Task 9's, and each builds its own bare board (no components, one
padstack, two nets) — the shape `PolylineTraceSplitTest.createTestBoard` builds:

* **7** — `PolylineTrace.combine`: `combineAtStart` and `combineAtEnd`, each in
  straight and reversed order, with and without the `skipLine` shortcut; the
  four refusals (a forked contact point, a different half width, a different
  fixed state, a foreign net); a five-segment chain combined from the middle,
  which is the loop `CombineStackOverflowTest` forced; the "path 2 requires
  entries in the default tree" fallback of
  `PolylineTraceSplitTest.testCombineAtEndRecoversMissingDefaultTreeEntries`;
  and `ignoreAreas` dropping a conduction area from the contact set.
* **8** — `PolylineTrace.split(IntOctagon)`: the two board-dependent
  `PolylineTraceSplitTest` split cases, the `clipShape` filter, the `DrillItem`
  branch (quirk #73), the `ConductionArea` cycle branch, a non-normal net, two
  traces crossing at right angles, a `USER_FIXED` refusal; then `normalize` in
  its three outcomes, `change` on a live and on an off-board trace (quirk #74),
  and — scenarios S14-S16 — quirk #22 reached through `combineAtStart`, where
  the JVM throws `ArrayIndexOutOfBoundsException` out of `combine()` and the
  port answers `BoardError::Normalization`, with `insertTrace`'s own
  `catch (Exception)` swallowing it at the same place Java's does.
* **9** — `combineTraces` (one net, then all nets, then a no-op call),
  `normalizeTraces`, `normalizeAllTraces`, `splitTraces`, and the five methods
  Task 11 had to leave uncovered because their bodies end in normalisation:
  `insertTrace` (both overloads), `insertVia`'s `splitTraces` loop (on a
  *two*-layer board, since `fromLayer..toLayer` is empty for a one-layer
  padstack), `connectToTrace`, `removeTraceTails`' `combineTraces` tail and
  `DrillItem.moveBy`'s `insertTrace` tail.
* **10** — `fixtures/Issue723-CombineStackOverflow.dsn`, generated rather than
  parsed (the DSN reader is Plan 3): 4000 collinear 200-unit segments in a
  15-row boustrophedon, inserted with `insertTraceWithoutCleaning` and then
  `normalizeAllTraces`, which is the pair of board calls `Wiring.java` makes.
  Both engines fold them into one 31-line trace. Takes a segment count as a
  second argument (`run.sh p2t11 10 400`).
* **11** — Task 12's `deep_copy`/`structural_hash`/`diff_traces`, on the same
  board mode 0 builds. First drives all four `transient` fields
  `deepCopy()`/`deep_copy()` must reset besides the search tree and
  `normalizeSuppressedNetNos`/`normalize_suppressed_net_nos` to a non-default
  value (`startMarkingChangedArea`, `setShoveFailingObstacle`,
  `setShoveFailingLayer`; `revision` is already non-zero from `build()`'s
  inserts) and prints `revision`/`changedArea`/`shoveFailingObstacle`/
  `shoveFailingLayer` before the copy, on the original again afterwards
  (unchanged — `deepCopy` must not mutate `self`) and on the copy (all four
  reset — matching on both sides, including `shoveFailingLayer=0`, not `-1`,
  see below). Then prints the default tree's `toArray()` (as `id:shapeIndex`
  pairs) before `deepCopy()`/`deep_copy()`, prints it again on the original
  afterwards (unchanged in both languages) and on the copy, then an
  `overlappingObjects` probe, `getHash()`/`structural_hash()` equality and
  `diffTraces`/`diff_traces` — all matching — and finally mutates the original
  (two `removeItem` calls) to show the copy is unaffected. **This is the one
  mode `run.sh`'s exact-match check does not pass**, and by design:
  `RoutingBoardUndoFacade.deepCopy` round-trips through Java serialization,
  whose `readObject` (BasicBoard.java:1388-1400) rebuilds the search tree from
  scratch by reinserting every item in descending-id order rather than
  restoring it — so Java's own `toArray()` differs before and after its
  `deepCopy()` (`treeArraysEqual=false` in the Java output), even though
  nothing else changed. `Board::deep_copy` clones the tree arena instead
  (`board/snapshot.rs`'s module doc has the full argument for why that is the
  more faithful choice, not a shortcut), so the port's `treeArraysEqual` prints
  `true`. The two `treeArrayCopy`/`treeArraysEqual` lines are consequently the
  *only* diff (`diff scripts/differential/build/{p2t11.j.out,p2t11.r.out}`
  after `run.sh p2t11 11`), and every other line — including the transient-field
  reset and the two `overlappingObjects` probes taken on both sides of the
  `deepCopy` — matches, which is the evidence that the tree-shape divergence is
  invisible to every real query. `shoveFailingLayer` is itself a Java quirk
  (quirk #79, `docs/java-quirks.md`): its `= -1` field initializer never runs
  on deserialization, so it comes back as the plain `int` default `0`, not the
  `-1` sentinel a freshly built board starts with — reproduced, and pinned by
  `transientCopy`'s `shoveFailingLayer=0` matching on both sides.

Two Java findings came out of it, both now in `docs/java-quirks.md`:
`Item.getAllNetNames` joins `Net::toString` (`"Net #1 (N1)"`), not the bare
name; and `Item.getTileShape` really does lazily recompute the tree-shape cache
(Item.java:212-238) — which is what keeps every query working, not just
`validate()`, after `changeClearanceClassIndex` has cleared the derived data.
Mode 3 pins it with no `validate` in between, so nothing else warms the cache
first.

## `p2t15` sweep (Plan 2 Task 15)

`p2t15` consolidates Task 15's brief into the ONE randomised board-level
driver: a shared xorshift stream drives `n` random pin/via/trace insertions
plus three fixed obstacle areas and three fixed conduction areas through the
real `RoutingBoard`/`Board`, `normalizeAllTraces`/`normalize_all_traces`,
every item's fields (id, kind, layer range, nets, clearance class, bounding
box, tile shapes), 100 `overlappingObjects` queries, 50
`overlappingTreeEntriesWithClearance` queries, the same 150 queries replayed
against a `deepCopy`/`deep_copy`, and a `hashEqual` boolean. Swept at 10 seeds
× `n` ∈ {30, 120} — every one of the 20 runs matches exactly, zero diff lines:

| seed | n | lines | diff |
|---|---|---|---|
| 1 | 30 | 405 | 0 |
| 1 | 120 | 695 | 0 |
| 2 | 30 | 408 | 0 |
| 2 | 120 | 695 | 0 |
| 3 | 30 | 406 | 0 |
| 3 | 120 | 707 | 0 |
| 7 | 30 | 404 | 0 |
| 7 | 120 | 664 | 0 |
| 42 | 30 | 413 | 0 |
| 42 | 120 | 672 | 0 |
| 555 | 30 | 402 | 0 |
| 555 | 120 | 695 | 0 |
| 77 | 30 | 407 | 0 |
| 77 | 120 | 674 | 0 |
| 12345 | 30 | 407 | 0 |
| 12345 | 120 | 677 | 0 |
| 999983 | 30 | 405 | 0 |
| 999983 | 120 | 708 | 0 |
| 20260828 | 30 | 407 | 0 |
| 20260828 | 120 | 689 | 0 |

The line count varies with the seed because `normalizeAllTraces` folds
however many of the random traces happen to touch end-to-end into fewer,
longer traces before the driver dumps the item list — fewer surviving items
means fewer `item …`/`tile[…]` lines. No line in any of the 20 runs differs
from its Rust counterpart, so there is nothing in this driver's output to add
to `docs/java-quirks.md`: it reaches no code path Tasks 10-13 hadn't already
exercised, just through a randomised, board-scale lens instead of the fixed
scripts those tasks wrote by hand. The re-run of `p2t10` (all 9 modes),
`p2t11` (all 11 modes the differential driver reaches) and `p2t13` (mode 0)
alongside this sweep confirms Task 15 did not regress any earlier driver;
`p2t11` mode 11's one pre-existing, documented `treeArraysEqual` divergence
(the tree-rebuild-vs-clone question, see the `p2t11` section above) is
unchanged.

## Harness maintenance note (Task 18)

`D17.java`'s `mode == 3` branch originally called a `PolygonShape.splitPiecesForDiff()`
that never existed on the real, unmodified class (apparently a debug hook a
reviewer meant to add and never did, or reverted). It doesn't compile against
`../freerouting` as checked in, and `d17.rs` never implemented that mode
either, so it has been replaced with a short placeholder string rather than
inventing the method on the Java side. `T16R.java`'s Rust twin (`t16r.rs`)
needed a handful of call sites updated for the `Polyline` methods that now
return `Result<Polyline, PolylineError>` instead of `Polyline`/`Option` (see
`docs/java-quirks.md` quirk #22) — mapped `Err` to the same `"EXC"` tag the
Java side already used for the equivalent exception.
