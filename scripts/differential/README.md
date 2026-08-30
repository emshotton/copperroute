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
    a full re-dump of the copy's items plus the same 150 queries replayed
    against it; and a `hashEqual` boolean (`getHash().equals(...)` vs.
    `structural_hash() == structural_hash()` — compared as booleans, not
    values, because the two hash algorithms are not byte-comparable,
    `docs/java-quirks.md`). `normalizeAllTraces` **increases** the trace
    count on this driver's random input, it does not fold traces together:
    with a small, shared pool of four nets and a modest coordinate range,
    unplanned random segments on the same net frequently cross in the
    middle rather than meeting end-to-end, and each such crossing is a
    junction normalisation splits both traces at — verified directly (a
    temporary instrumented run, not a guess): 101 traces before
    `normalizeAllTraces` become 300 after at seed 11, n=300; 172 become 554
    at seed 5, n=500. Deliberately never prints `toArray()`/tree internals
    (unlike `P2T10`/`P2T11`, which reach into `ShapeTree`'s protected fields
    for exactly that reason) — every query goes through public API, so
    quirk #77's tree-layout divergence never appears and the driver is
    zero-diff. `run.sh p2t15 <seed> <n>` diffs it against `p2t15.rs`.

    What this driver does and does not prove — the tie-break counter's
    snapshot/restore, and the three coverage holes it deliberately does not
    claim — is written out once, under "`p2t15` sweep (Plan 2 Task 15)" below.
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
  - `P3T2.java` — Java number formatting (Plan 3 Task 2). Twin: `p3t2`.
    Declares `package app.freerouting.io.specctra;` so it can reach the
    package-private `SesWriter.formatPlacementRotation`, and is therefore
    compiled and run against the clone's own
    `../freerouting/build/libs/freerouting-current-executable.jar` on a
    **JDK 25** (`JAVA25_HOME`), like `P2T10.java`/`P2T11.java`/`P2T15.java`.
    Walks a seeded `java.util.Random` stream of doubles through
    `Double.toString`, `Float.toString` of the `float` cast and (in every mode
    but 0) `formatPlacementRotation`, one line per value, prefixed by the raw
    bits in hex. This is the driver behind `fr-dsn`'s
    `format::double` — the module every DSN/SES writer's byte-parity rests on.
  - `P3T3.java` — the Specctra DSN lexer's token stream (Plan 3 Task 3). Twin:
    `p3t3`. Takes a **file path** and prints one line per token —
    `<index> <TAG> <value> <lexicalStateAfterTheToken>`, with `TAG` one of
    `OPEN`/`CLOSE`/`KW`/`STR`/`INT`/`DBL` and a final `<index> EOF - <state>`
    line. A `KW`'s value is the *field name* of the `Keyword` singleton the
    scanner returned (found by reflection over `Keyword`'s public static
    fields), not `Keyword.getName()`: the field name is the identity the
    parser dispatches on, and it is identical between the pinned 2.3.0 jar and
    the clone's HEAD, whose fifteen renamed `getName()` strings are plan 3
    ruling 1's subject. Uniquely, it runs against
    **`tools/freerouting-2.3.0.jar`** (plan 3 ruling 10, `FREEROUTING_JAR_230`)
    rather than the clone's build; the scanner's entry point is looked up
    reflectively because 2.3.0 spells it `next_token` and HEAD `nextToken`.
    The driver also rebinds `System.out` before FRLogger can load, because
    action 2 of the scanner logs one `WARN` line per non-ANSI character and
    that would interleave with the token stream.
  - `P3T15.java` — the DSN reader and all three writers over one fixture
    (Plan 3 Task 15). Twin: `p3t15`. Takes `<file.dsn> <mode>`:
    - `0` — one line per board item in **ascending id** order:
      `item <id> <kind> layers=<first>..<last> nets=[…] cl=<class>
      fixed=<state> cmp=<component> bbox=(llx,lly,urx,ury) tiles=<count>`,
      preceded by `layers <n>` and followed by `itemcount <n>` and the
      `BoardReadResult` warnings.
    - `1` — `DsnWriter.write(board, out, stem, false)`, byte for byte.
    - `2` — `SesWriter.write(board, out, stem)`, byte for byte.
    - `3` — `RulesWriter.write(board, out, stem)`, byte for byte.
    - `4` — delegates to `P3T3.main`, so one driver can sweep the corpus.

    The read is exactly `scripts/gen-reference/RefWriter.java`'s —
    `DsnReader.readBoard(in, null, null, designName)` with `designName` the
    file name minus a trailing `.dsn` — which is also what
    `crates/fr-dsn/tests/parity_dsn.rs` does, so a diff here and a bit-parity
    failure there have the same cause. A `ParseError` prints
    `RESULT ParseError <location> | <detail>` on both sides and stops.
    Like `p3t3` it runs against **`tools/freerouting-2.3.0.jar`** (ruling 10);
    `P3T3.java` is compiled alongside it for mode 4.

    **`p3t15` is the only driver that covers the scanner's hand-rolled
    bypass.** `nextString`, `nextStringList` and `nextDouble` walk `zzBuffer`
    directly instead of running the DFA, so `p3t3`'s pure `next_token` loop
    never reaches them. Modes 0-3 drive the whole parser, which calls all
    three on nearly every scope, and any divergence surfaces as a wrong item,
    a wrong name, or a wrong number in the output.
  - `P4T1.java` — Java's **real** headless settings composition over the Plan 4
    precedence matrix (Plan 4 Task 9). Twin: `p4t1`. Declares
    `package app.freerouting.settings;` and runs against the clone's HEAD jar
    (plan 4 ruling 7 — `SettingsMerger` and `settings/sources/**` are what is
    being compared, and the 2.3.0 jar puts `RoutingBoard` in another package).
    Every merge-relevant class it calls is the real one out of the jar
    (`DefaultSettings`, `JsonFileSettings`, `CliSettings`,
    `EnvironmentVariablesSource`, `DsnFileSettings`, `RulesFileSettings`,
    `ApiSettings`, `SettingsMerger`, `RulesReader.read`,
    `RouterSettings.applyBoardSpecificOptimizations`); only the ~50 lines of
    plumbing that wire them are transcribed, because that plumbing lives inside
    `Freerouting.main`'s process lifecycle, a `RoutingJobScheduler` worker
    thread and a `HeadlessBoardManager` board load. See "the transcription
    risk" below for how to re-check it.

    Takes `<cases.tsv> <case-index|all> <mode>`:
    - `0` — the canonical dump: one `path=value` line per field of the merged
      `RouterSettings` (`ScoringSettings`/`OptimizerSettings`/`FanoutSettings`
      and each `LayerSettings` inlined by path), **sorted by path**, with
      `null` for null, `Double.toString`/`Float.toString` for the two float
      types and the enum's `name()` for enums. Arrays contribute a
      `<path>.length=N` line plus one line per element so that null and empty
      stay distinguishable. Transients are included, and so is the private
      `boardSpecificTraceCostsApplied` (through its accessor). The
      `copyFields` change count is deliberately **not** printed (plan 4
      ruling 2 — it is not reproducible and no caller reads it).
    - `1` — the same object through `GsonProvider.GSON` on the Java side and
      `RouterSettings::to_json_string_pretty` on the Rust side (Plan 4 Task 10).
      Byte-for-byte: two-space indent, `getDeclaredFields()` key order, `null`
      fields omitted (Gson's default `serializeNulls = false`), floats through
      `Double.toString`/`Float.toString`, and all **nine** `transient` fields
      absent (`RouterSettings.java:61,64,67,70`, `OptimizerSettings.java:80,84,91`,
      `ScoringSettings.java:30,35`). 84 cases, 4 287 lines, **0 diffs**.
    - `2` — every case, whatever `<case-index>` says. `all 0` and
      `<anything> 2` are the same run.

    Both sides read the same case table, `matrix/p4t1-cases.tsv`, and both
    print a `CASE <id>` line before each dump, so a diff names the row that
    moved. The first line is a header carrying the jar's real path, size and
    mtime and `Runtime.getRuntime().availableProcessors()`: the Java side
    derives the jar from `RouterSettings.class`'s code source and the Rust side
    from `$FREEROUTING_JAR`, and the processor count is pinned with
    `-XX:ActiveProcessorCount=4` against `HostEnvironment::with_processors(4)`
    (`$P4T1_PROCESSORS`), so running against the wrong build or an unpinned JVM
    is a diff rather than a silent assumption. The Rust side additionally
    prints its own binary's path and mtime **to stderr**, which `run.sh`
    neither captures nor diffs — the header proves which *jar* ran, and that
    line proves which Rust binary did. The second stdout line is
    `JSON_SOURCE_EMPTY`: the Java side builds `JsonFileSettings` on an **empty
    temporary directory** and aborts with `JSON_SOURCE_NOT_EMPTY` unless every
    leaf of its `getSettings()` is null, which turns spec §2's "no persistent
    config file" from an assumption into a check (and keeps a real
    `freerouting.json` in the user-data folder from leaking into every case).
  - `P5T1.java` — the DRC **report** (Plan 5 Task 10). Twin: `p5t1`. Declares
    `package app.freerouting.drc;` and runs against the clone's HEAD jar
    (plan 5 ruling 1 — the DRC port's sources and every file:line in that plan
    are HEAD's, and the 2.3.0 jar spells nine of the JSON keys in snake_case).
    Reads a `.dsn`, optionally applies a `.rules` file and a `.ses` session in
    `Freerouting.initializeDrc`'s order, builds the real
    `DesignRulesChecker(board, null)`, calls `generateReport(<base name>, "mm")`
    and prints `GsonProvider.GSON`'s bytes for it. Five normalisation rules,
    applied identically on both sides, take the three values the CLI *injects*
    (plan 5 ruling 5) and the one hash-ordered list (ruling 3) out of the
    comparison: `date` and `freeroutingVersion` are replaced by fixed literals
    (both are `final` fields, so this is a two-line edit of the serialised
    tree), `qualityScore` is pinned to `-1.0`, each `unconnectedItems` entry's
    `items` list is sorted by numeric uuid, and nothing else is touched — in
    particular `violations` is compared exactly, array *and* per-entry `items`.
  - `P5T2.java` — the **algorithm-level** DRC lists behind that report (Plan 5
    Task 10, ruling 14: a report match must not be able to mask a compensating
    pair of errors). Twin: `p5t2`. Same package, same jar, same three-file board
    load — it calls `P5T1.loadBoard`, and `run.sh` compiles `P5T1.java`
    alongside it, so the two drivers cannot drift apart on their input. Takes
    `<dsn> [rules|-] [ses|-] <mode>`, the mode last:
    - `0` — `V <firstId> <secondId> <layer> <expected> <actual>` per entry of
      `getAllClearanceViolations()`, in list order, floats through
      `Double.toString`, then `VCOUNT <n>`. A bit-parity surface.
    - `1` — `U <type> <firstKind> <secondKind|-> <ids>` per entry of
      `getAllUnconnectedItems()`, in list order, then `UCOUNT` and one `UTYPE`
      line per type. `ids` is `allItems` sorted ascending, with the trailing
      `null` a dangling entry carries (`UnconnectedItems.java:41`) dropped,
      because the port's `Vec<ItemId>` cannot hold one. The two representatives
      print as their **kind class** (`Pin`/`Trace`/`other`), which is the part
      of `findRepresentativeItem`'s answer that survives its `HashSet` order —
      the same projection `crates/fr-drc/tests/data/UnconnectedProbe.java`
      takes; nothing is lost on the dangling kinds, whose one item id is the
      `ids` column.
    - `2` — the ratsnest through the real accessors: `MAXCONN`, `INCOMPLETE`,
      one `NET <no> <airlines> <groups> <lengthViolation>` line per net,
      `ALCOUNT`, then one `AL <net> <lowId> <highId>` line per airline as the
      **unordered** pair, sorted — the convention of the committed
      `crates/fr-drc/tests/data/*.airlines-union.txt` files.
    - `3` — the same output, but with `NetIncompletes`' constructor,
      `calculateNetItems`, `joinConnectedSets`, `Edge` and `calcLengthViolation`
      transcribed into the driver so that the algorithm's **one free choice** —
      the seed of `calculateNetItems`' outer loop, a `HashSet<Item>` in Java
      (`NetIncompletes.java:295,:299`) and the lowest item id in the port
      (ruling 3) — can be pinned the port's way. This is the ratsnest's parity
      surface.
    - `4` — the transcription check: that same transcription with Java's own
      `HashSet` seed, compared *inside the JVM* against the real
      `getAllAirlines()`, printing `TRANSCRIPTION equal 0`. The Rust twin prints
      that line unconditionally, the way `p4t1`'s twin prints
      `JSON_SOURCE_EMPTY`: this side computes the fact, that side states it, and
      the harness's diff is the assertion.
  - `P6T2.java` — `ShapeSearchTree.completeShape` and `divideLargeRoom` in all
    three angle regimes (Plan 6 Task 3). Twin: `p6t2`. **This closes the gap
    `p2t10` documents**: `p2t10`'s eight modes reach "every public
    `ShapeSearchTree` method except `completeShape`/`divideLargeRoom`", and
    those two are the pair this driver covers. It declares `package
    app.freerouting.board.searchtree;` so it can call the `protected`
    `divideLargeRoom` directly rather than only through `completeShape`, and so
    it is compiled and run against the clone's HEAD jar with a **JDK 25**, like
    `p2t10`. It builds the `P2T10.java` board plus `n` random obstacle areas
    from the shared xorshift stream, seeds the autoroute tree with three
    `CompleteFreeSpaceExpansionRoom`s (so the `instanceof
    CompleteFreeSpaceExpansionRoom` branch of all three `completeShape`s is
    live), and prints, per returned room: the layer, the dimension, the shape
    and contained shape, and both corner lists as `Double.toString` ordinates.
    It calls `FRLogger.disableLogging()` first, because `completeShape` warns
    on stdout for every seed room whose shape is of the wrong class for the
    regime and the port drops every `FRLogger` payload.
  - `P6T3.java` — the three neighbour sorters (Plan 6 Tasks 4 and 5). Twin:
    `p6t3`. It declares `package app.freerouting.autoroute.expansion;` and
    reaches the classes' `private` members by reflection —
    `calculateNeighbours`, the `sortedNeighbours` / `ownNetObjects` /
    `completedRoom` / `edgeInteriorTouchesObstacle` fields, and all three
    `private class SortedRoomNeighbour`s, which are three different classes with
    three different field sets. There is no other way to see the sorted list:
    `calculate` consumes the instance and returns only the completed room, and
    every method above `calculateNeighbours` needs an `AutorouteEngine`, which
    the port does not have until Task 6. Compiled and run against the clone's
    HEAD jar with a **JDK 25**, like `p6t2`, with `FRLogger.disableLogging()`
    first.

    Ten modes, and each buys something different:

    - `0` — the brief's mode. The `P2T10.java` board plus `n` random obstacle
      areas, the autoroute tree seeded with three
      `CompleteFreeSpaceExpansionRoom`s (so `calculateNeighbours` really meets
      `TreeObject::Room` leaves, which is the arm `fr-board`'s
      `tree_shape_of`/`ignore_object` used to panic on), and `rooms` seed rooms
      that are either a `completeShape` output — what
      `AutorouteEngine.completeExpansionRoom` actually hands the sorter — or an
      obstacle room over a random item shape. A *random box* seed room is
      useless here: it almost never **touches** anything, so every interesting
      branch stays dead.
    - `4` — mode 0 with every obstacle snapped to a 500-unit grid. Without it a
      completed room's corner never lands exactly on an obstacle's and
      `calculateNeighbours`' whole **dimension-0** branch
      (`SortedRoomNeighbours.java:286-326` — `equalsCorner`,
      `containsOnBorderLineNo`, both corner flags) is never reached: mode 0
      produces no corner touch at all.
    - `5` — the **whole** of `SortedRoomNeighbours.complete` against a real
      `AutorouteEngine` (`new AutorouteEngine(routingBoard, 1, false)` plus
      `initConnection`): `tryRemoveEdge` and its `completeShape` retry,
      `calculateNewIncompleteRooms`, `calculateIncompleteRoomsWithEmptyNeighbours`
      and `calculateTargetDoors` — some 250 lines mode 0 cannot reach, because
      everything above `calculateNeighbours` mutates engine state. The port has
      no engine until Task 6, so it drives the same five services out of its
      `ExpansionRoomStore`; what is compared is the engine-visible result (the
      completed room and its shape, its doors and target doors, the seed room's
      shape after `tryRemoveEdge`, and every incomplete room on the engine's
      list). The room ids come from the engine's own counter on both sides so
      they stay in lockstep, and the incomplete list is reset between calls —
      nothing drains it here, and letting it grow makes the dump quadratic.
      **Both sides skip a call whose room shape has more border lines than its
      `toSimplex()` does**: that is quirk #162, an unterminating loop in
      `calculateNewIncompleteRooms` that kills the JVM with an
      `OutOfMemoryError` (seed 42 reaches it at `i=124`), and it is skipped
      rather than tolerated. It costs 4 of 1 000 completions.
    - `1`, `2`, `3` — the hazard-F probes, which build `SortedRoomNeighbour`s
      through the inner class's constructor and insert them into a `TreeSet`
      with no board in the way. `1` is random touches on all four sides; `2`
      puts every neighbour of a probe on the **same** side, so the comparator's
      first key always ties; `3` gives every neighbour `roomTouchIsCorner`, so
      both corners collapse to the room's own corner, every distance delta is 0
      and only the `Direction.compareFrom` branch and the id difference are
      left. Mode 3 is what found the divergence that made the port transcribe
      `java.util.TreeMap` (quirk #160): with the port on a `BTreeSet` it diffed
      in both directions — an element Java keeps that a `BTreeSet` drops, and a
      different survivor order.
    - `6`, `7` — mode 4 for the two **angle-restricted** sorters (Task 5). The
      board is built with `AngleRestriction.FORTYFIVE_DEGREE` / `NINETY_DEGREE`,
      so `searchTreeManager.getAutorouteTree(1)` answers the matching
      `ShapeSearchTree` subclass and `selectCalculationMode` the matching
      sorter; the driver then reflects into
      `Sorted45DegreeRoomNeighbours.calculateNeighbours` /
      `SortedOrthogonalRoomNeighbours.calculateNeighbours`. The dump is
      different from mode 0's, because their inner `SortedRoomNeighbour` is: a
      first *and* a last touching side rather than one side plus two corner
      flags, no memoized corners, and `edgeInteriorTouchesObstacle` — the array
      `tryRemoveEdge` reads — printed after the list. The target doors are
      printed too, because these two build them **inside** the neighbour loop
      (`CompleteFreeSpaceExpansionRoom.calculateTargetDoors`) where the base
      class defers them. Each ends with an `overlap` probe: a
      `CompleteFreeSpaceExpansionRoom` inserted so that it overlaps a hand-built incomplete room
      2-dimensionally, on an otherwise empty part of layer 1. That is the only way to reach the
      `dimension > 1 && completedRoom instanceof ObstacleExpansionRoom` **`&&`** the two
      subclasses have and the base class does not (`:132` / `:168`), and therefore the only way to
      reach the **two-argument** `ExpansionDoor` constructor's computed `dimension == 2`
      (`:164` / `:201`, `ExpansionDoor.java:35-39`) — which is the door `tryRemoveEdge` scans for
      when it picks `completeShape`'s `ignoreObject`. The random loop cannot produce it, because
      its seed rooms are `completeShape` output and so are restrained against everything already
      in the tree. The probe draws no random numbers and runs last, so modes 0-5, 8 and 9 are
      untouched.
    - `8`, `9` — mode 5 for the same two regimes: the whole of
      `SortedRoomNeighbours.complete` against a real `AutorouteEngine`, which
      dispatches on the tree subclass, so each subclass's `tryRemoveEdge`,
      `calculateNewIncompleteRooms`,
      `calculateEdgeIncompleteRoomsOfObstacleExpansionRoom` /
      `calculateIncompleteRoomsWithEmptyNeighbours` and `insertIncompleteRoom`
      all run. Quirk #162's skip is **mode 5's only**: neither subclass walks a
      `Simplex`, so neither can reach that loop. Mode 8 is what found
      **quirk #163** — `calculateEdgeIncompleteRoomsOfObstacleExpansionRoom`
      never advances its `currentCorner`, so an eight-sided obstacle room with
      no touching neighbour gets seven incomplete rooms, not eight.
- `java/probes/` — **ground-truth probes, not differential drivers.** A probe has
  no Rust twin and `run.sh` does not know it: it is a Java program whose stdout a
  Rust *test* pins as literals, so the test asserts against the HEAD jar rather
  than against the port's own opinion. Use one where the parity surface is a
  handful of deterministic, hand-built cases (so a randomised driver would add
  noise, not evidence) or where the state to compare is private and has to be
  reached by reflection. Each file's header carries the exact `javac`/`java`
  invocation, which mirrors `run.sh`'s jar mode (JDK 25, the clone's HEAD jar,
  `-Djava.awt.headless=true -Duser.language=en -Duser.country=US
  -XX:+UnlockExperimentalVMOptions -XX:hashCode=2`).
  - `P6T6Probe.java` — `AutorouteEngine`'s expansion-room lifecycle (Plan 6
    Task 6). Consumed by `crates/fr-router/tests/engine_rooms.rs`, whose room
    ids, room shapes, door counts, room-instance counter, surviving
    incomplete-room count and search-tree leaf count are all this probe's
    output. It declares `package app.freerouting.autoroute.maze;` so it can
    reflect into `AutorouteEngine`'s `private` `completeExpansionRooms`,
    `incompleteExpansionRooms` and `expansionRoomInstanceCount` — the state the
    tests assert on, which no public method exposes. Six modes:

    - `0` — an empty board. Pins that `completeExpansionRoom` yields **no**
      rooms and never ticks the counter, because `completeShape` returns at
      `ShapeSearchTree.java:589-591` when the tree has no root.
    - `1` — one obstacle, one seed. The headline case: nine rooms constructed,
      **six** listed (ids `1, 2, 6, 7, 8, 9`), doors `9, 3, 6, 9, 4, 2`,
      `counter=9 incomplete=25 treeSize=7`. It is what pins **quirk #165** —
      `completeExpansionRooms` is a strict subset of the complete rooms that
      exist — and, with mode 4, the "only the first dimension-2 candidate is
      added directly" rule of `AutorouteEngine.java:492-515`.
    - `2` — `completeNeighbourRooms`' iterator restart (`:573-584`):
      `complete 2→4, incomplete 4→5, counter 4→7, treeSize 4→6`.
    - `3` — `initConnection` with `maintainDatabase = true`: the net-dependent
      invalidation drops both rooms (`complete 2→0, incomplete 9→7,
      treeSize 4→2`), which also exercises `removeCompleteExpansionRoom`'s
      1-dimensional-neighbour regeneration.
    - `4` — `completeShape`'s **raw** candidates for mode 1's seed: eight, all
      of dimension 2, none of whose shapes survives into rooms 2 and 6.
    - `5` — the door-by-door trace of `removeCompleteExpansionRoom`, printing
      each neighbour's class, shape, intersection dimension and `touchingSides`
      result, and the engine's state after each individual removal. This is what
      found **quirk #164**: removing room 5
      walks four doors to incomplete rooms, two of which log
      `touching_side : dir2 not found` and answer an empty array, and the
      removal still succeeds — because `:383` binds `ExpansionDoor`'s
      *narrowing* `otherRoom(CompleteExpansionRoom)` overload and skips all four.
  - `P6T7Probe.java` — the drill pages, the page array and the expansion drills
    (Plan 6 Task 7). Consumed by `crates/fr-router/tests/drill.rs`, whose page
    grids, overlapping-page sets, drill counts, drill shapes and locations,
    `getId()` hashes and per-layer room ids are all this probe's output. It
    declares `package app.freerouting.autoroute.drill;` so it can reflect into
    `DrillPageArray`'s `private` `pages`/`columnCount`/`rowCount`/`pageWidth`/
    `pageHeight` and into `DrillPage`'s `private` `drills`/`netNumber`. Its
    board is `P6T3.build`'s any-angle board verbatim — two layers, a two-pin
    component whose first pad is **SMD** (one layer) and whose second is a
    **through** pad (the same octagon on both layers), and two traces on
    different nets — which is what makes modes 3 and 4 possible at all. Eleven
    modes:

    - `0` — the page grid for four bounding boxes. Pins the `ceil` chain of
      `DrillPageArray.java:37-41`, including that `pageWidth` is **recomputed**
      from `columnCount`: a 20 000-unit board over 7 000-unit pages gives
      `columnCount=3 pageWidth=6667`, not two columns of 7 000.
    - `1` — `overlappingPages`, printing `minJ`/`maxJ`/`minI`/`maxI` alongside
      each answer. The fractional `maxJ` of the first probe
      (`1.6499175041247938`) is what makes the mixed-type loop bounds of
      `:81-88` observable; a port that truncates `maxJ` answers 1 page where
      Java answers 4.
    - `2` / `3` — `getDrills(engine, false)` and `getDrills(engine, true)` on
      the component page: **13** drills against **11**, the difference being the
      three little shapes wedged around the SMD pad, which collapse into one
      when `attachSmd` skips it (`DrillPage.java:80-84`).
    - `4` — the obstacle cut-out loop entry by entry: the eight tree entries,
      which are drillable, which pin is `drillAllowed`, each tree shape, and
      whether it produced a cut-out. This is what pins the `prevObstacleShape`
      carry (`:87`): the through pin's two entries carry the **same** octagon
      and the second produces no cut-out. Four cut-outs without `attachSmd`,
      three with.
    - `5` — `calculateExpansionRooms` three ways: a location where
      `completeExpansionRoom` answers more than one room (false, nothing bound),
      the same location as a single-layer drill (true, one slot), and a location
      free on both layers, twice — the second call finds the rooms the first
      created instead of building new ones.
    - `6` — `getDrills` under a `Stoppable` that always answers true. Pins
      **quirk #168**: `NullPointerException at DrillPage.java:108`, and then
      `netNumber(field)=1 drills=0` with the next call answering the memoised
      empty list.
    - `7` — `getId` across a net change. Pins **quirk #167** (hazard B):
      `-29760001` fresh, `-29759999` on net 1, `-29759998` on net 2, with 13 and
      then 30 drills; `reset()` keeps the memo and `invalidate()` drops it
      without restoring `netNumber`.
    - `8` — `getDrills` on an engine whose `incompleteExpansionRooms` has never
      been created. Pins **quirk #169**: **zero** drills where mode 2 gets 13,
      with 28 swallowed `NullPointerException`s.
    - `9` — a drill whose upper layer alone is blocked by a keepout:
      `calculateExpansionRooms=false` with `roomArr[0]` bound and `roomArr[1]`
      null.
    - `10` — the engine's **own** array (`AutorouteEngine.java:91`) and its two
      drill hooks, on a board whose bounding box is one page wide. The full
      -10 000..10 000 board cannot be used here: completing a room inside a
      10 000-wide page trips **quirk #162**'s non-terminating
      `calculateNewIncompleteRooms` and the probe OOMs. Pins that
      `invalidateDrillPages` reaches the page, that a shape outside the board's
      bounding box invalidates nothing, and that the recomputation answers
      **ten** drills where the first answered nine — the rooms the first pass
      created are in the tree by then.
  - `P6T8Probe.java` — `AutorouteControl`, `DestinationDistance` and the
    ruling-H via-info re-pointing mechanism (Plan 6 Task 8). It declares
    `package app.freerouting.autoroute.maze;` so it can read
    `DestinationDistance`'s nine package-private cost fields, which the
    constructor derives and no public method exposes. Its stdout is committed
    verbatim as `crates/fr-router/tests/data/p6t8-destination-distance.txt` and
    `…/p6t8-autoroute-control.txt`, and both are **replayed row for row** by
    `crates/fr-router/tests/{destination_distance,control}.rs` — so a
    regenerated probe and a stale expectation cannot silently disagree. Four
    modes:

    - `dd` — `DestinationDistance` in seven configurations (four/three/two/one
      active layers, nothing joined, only the component-side box joined, and a
      two-layer board where the inner arm is unreachable) crossed with a fixed
      point/box grid: 234 value rows plus the seven cost rows. Between them they
      reach every early return of `calculate(IntBox, int)` — `activeLayerCount
      <= 1`, `== 2`, `== 3` and the four-layer fall-through on layer 0; `<= 2`
      and `== 3` on the solder side; the inner-layer arm; the `boxIsEmpty`
      short circuit; and the three `…BoxIsEmpty` guards. Each box row also
      prints `calculateCheapDistance` and then `calculate` **again**, which is
      how hazard J (the mutate-and-restore of `minNormalViaCost`) is pinned.
    - `ctrl <dsn> [netNo]` — loads a fixture through `HeadlessBoardManager`,
      builds `new RouterSettings(board)` and the real `AutorouteControl`, and
      prints every field. With no net number it scans for the first pure-SMD net
      and the first mixed one and dumps both, plus net 0 and a net number the
      board does not have. Run over `Issue593-BBD_Mars-64.dsn` and
      `Issue508-DAC2020_bm01.dsn`; it is what pins **quirk #172** (net 1 of
      Issue593 answers `attachSmdAllowed=true` from `viaInfos=[0..1
      attach=false]` and `minNormalViaCost=400.0` against net 0's `4000.0`) and
      **quirk #173** (`ctrl net=1094 threw java.lang.NullPointerException`).
    - `nan` — a NaN horizontal trace cost on layer 0, to pin that Java's
      `Math.min`/`Math.max` **propagate** it (`if (a != a) return a;`) where
      Rust's `f64::min`/`f64::max` absorb it. Every layer arm of `calculate`
      answers `NaN`. This is why all 29 `Math.min`/`Math.max` sites in
      `control.rs` and `destination_distance.rs` are `fr_geometry::java_min` /
      `java_max` — and it is what makes quirk #170's NaN reachable from this
      direction at all. Committed output:
      `crates/fr-router/tests/data/p6t8-nan-propagation.txt`.
    - `viadiv <dsn> <rules>` — the ruling-H mechanism at HEAD: reads the `.dsn`,
      applies the `.rules` file with `RulesReader.read`, and prints both the
      `viaInfos` list and what each `ViaRule` actually reaches, with
      `inList=` telling them apart. With
      `crates/fr-router/tests/data/ruling-h-redeclare.rules` on
      `Issue593-BBD_Mars-64.dsn` the list says `attach=true` and both rules say
      `attach=false inList=false` — the detached original. Committed output:
      `crates/fr-router/tests/data/p6t8-ruling-h-viadiv.txt`.
  - `P6T9Probe.java` — `RoutingBoard`'s five autoroute-facing methods, the
    check-only half of `board.optimize.TraceShover`, and
    `board.actions.DrillItemMover.check` / `.tryShoveViaPoints` (Plan 6 Task 9).
    It declares `package app.freerouting.board.optimize;` so it can call
    `TraceShover`'s package-private `getIgnoreItemsAtTiePins`, and reflects into
    `RoutingBoard.autorouteEngine` and `AutorouteEngine`'s three private room
    lists, none of which any public method exposes. Its stdout is committed
    verbatim as `crates/fr-router/tests/data/p6t9-board-ext.txt`, one
    `######## <mode>` section per mode, and every literal in
    `crates/fr-router/tests/board_ext.rs` is read off it. Six modes, the first
    on `P6T6Probe`'s bare board and the rest on `P6T7Probe`'s (which is
    `P6T3.build`'s any-angle board) plus the three nets `Trace.isShoveFixed`
    dereferences:

    - `upd` — the re-run `task-6-report.md` §8.1 asks for: `initAutoroute` ->
      `initConnection` **with an item on the new net**, which is the only way to
      reach `initConnection:111-117` -> `additionalUpdateAfterChange`. Its board
      carries a net-2 trace and **no** net-1 item, so the single completed room
      is `netDependent=false` and the `complete=1 -> 0`, `incomplete=5 -> 1`,
      `treeSize=2 -> 1` transition it prints can only have come from that loop.
      It also pins `initAutoroute:888-891`'s three-way reuse guard
      (`reusedTheEngine=true`, `rebuiltOnClassChange=true`,
      `rebuiltWhenRetainIsFalse=true`) and `finishAutoroute`.
    - `poly` — `checkForcedTracePolyline` over seven probe polylines x two half
      widths x both angle regimes, 28 rows.
    - `seg` — the **static** `TraceShover.check(RoutingBoard, LineSegment, …)`
      over the same lines x shove direction x half width, 24 rows, printing the
      `double` it answers (`MAX` for `Integer.MAX_VALUE`).
    - `inst` — the **instance** `TraceShover.check(TileShape, ShapeEntrySide, …)`
      at `maxRecursionDepth` 0/1/2/20 and `maxSpringOverRecursionDepth` 0/1/20,
      56 rows. The `twoSegments shape=0` block is the one that changes with the
      budget (`false` at depth 0, `true` from depth 1), which is what pins
      `:356-358`. Its `failing=` column is *not* asserted: it is
      `board.getShoveFailingObstacle()`, which quirk #174 leaves stale.
    - `drill` — `DrillItemMover.check` over a free via, a shove-fixed one and
      one sitting on a through-hole pin, plus `tryShoveViaPoints` on a box and
      an octagon x `extendedCheck` x both angle regimes. `ignoreSize` is what
      pins quirk #175.
    - `tie` — `getIgnoreItemsAtTiePins` over three shapes x three net arrays.
- `sweep-p5t1.sh` / `sweep-p5t2.sh` — the two Plan 5 corpus sweeps. Each
  compiles both sides once through `run.sh`, then loops the built artifacts over
  **112 rows**: every `.dsn` in `$FREEROUTING_JAVA_DIR/fixtures` whose reader
  result is `Success` according to `crates/fr-dsn/tests/data/corpus-read-results.txt`
  (104 of the 105; `Issue006-LPC18XX_43XX_SCH.dsn` is an OLE compound document
  and is SKIPped and counted), plus the eight `tests/reference/drc-fixtures.txt`
  rows, which add the `.rules` path, the `.ses` path and the tutorial board.
  Both pass `-XX:hashCode=$P5T_HASH_MODE` (default 2, the constant mode the DRC
  references were generated under) and `-Duser.language=en -Duser.country=US`;
  `P5T_HASH_MODE=0..4` is how a diff is proven Java-side. `SWEEP_OUT=<dir>`
  keeps the raw outputs. The expected-diff tables are in the scripts' headers,
  with their evidence.
- `matrix/p4t1-cases.tsv` — the `p4t1` case table, 84 rows, tab-separated:
  `name`, `dsn`, `cli_rules`, `scheduler_rules`, `env` (`K=V;K=V`), `argv`
  (space-separated) and `board`. `-` is "absent"; a rules path is `D:<name>`
  (`crates/fr-settings/tests/data`) or `F:<name>`
  (`$FREEROUTING_JAVA_DIR/fixtures`); `board` is either a layer count — the
  synthetic `BProbe` board, 2 000 000 × 1 000 000, all signal, layers named
  `F.Cu`/`In1.Cu`/`In2.Cu`/`B.Cu` — or `dsn`, meaning the fixture read back
  through the real DSN reader. The first 64 rows are exactly
  `crates/fr-settings/tests/matrix/mod.rs`'s cross product, with the same ids,
  so Task 8's Rust-side two-merge proof and this JVM proof cover the same
  ground; the 20 `x-*` rows are Task 9 additions (the `validate()`
  non-idempotence rows, four real corpus `.rules` files, and env/CLI shapes the
  64 do not carry).
- `sweep-p3t15.sh [mode ...]` — compiles both sides once through
  `run.sh p3t15`, then runs every requested mode (default: all five) over
  every `.dsn` in `$FREEROUTING_JAVA_DIR/fixtures` plus
  `examples/tutorial_board/tutorial_board.dsn`, and prints a per-fixture
  MATCH/DIFF table. A `<fixture>:<mode>` pair listed in the script's
  `EXPECTED_DIFFS` array prints `XDIFF` and does not fail the run; every other
  diff does. `SWEEP_OUT=<dir>` keeps the raw outputs of the differing pairs.

- `rust/` — a standalone Cargo package, `fr-geometry-differential`, **not** a
  member of the repo's workspace (see the root `Cargo.toml` `exclude` and this
  package's own `[workspace]` table). It depends on `fr-geometry` and
  `fr-board` by path and builds one `[[bin]]` per twin: `t14`, `t15`, `t16r`,
  `e15`, `d17`, `p2t3`, `p2t3r`, `p2t10`, `p2t11`, `p2t13`, `p2t15`, `p3t2`,
  `p3t3`, `p3t15`, `p4t1`, `p5t1`, `p5t2`, `p6t2`, `p6t3`. Since Plan 3 it also depends on
  `fr-dsn` by path (for `p3t2`, `p3t3` and `p3t15`), since Plan 4 on
  `fr-settings` (for `p4t1`), since Plan 5 on `fr-drc` (for `p5t1`/`p5t2`) and
  since Plan 6 on `fr-router` (for `p6t2` and `p6t3`).
  `p3t3` and `p3t15` share the token dump through `src/token_dump.rs`, included
  by both with `#[path]` — the Java side of mode 4 delegates to `P3T3.main`, so
  the two dumps must stay identical; `p5t1` and `p5t2` share the argument
  handling, the header line and the three-step board load through
  `src/drc_common.rs` the same way, mirroring `P5T2`'s call to
  `P5T1.loadBoard`.
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
- For `p2t10`/`p2t11`/`p2t15`/`p3t2`/`p6t2`/`p6t3` only: a **JDK 25** (`JAVA25_HOME`) and the clone's built jar at
  `../freerouting/build/libs/freerouting-current-executable.jar`
  (`FREEROUTING_JAR`). Run `./gradlew build` in the clone if it is missing.
- For `p3t3`/`p3t15` only: a **JDK 25** (`JAVA25_HOME`) and the pinned release jar at
  `tools/freerouting-2.3.0.jar` (`FREEROUTING_JAR_230`) — the pinned release
  jar, gitignored like the clone's build output, downloaded from the
  freerouting 2.3.0 release (it is also what `scripts/gen-reference.sh` uses).
  Plan 3 ruling 10 makes 2.3.0, not the clone's HEAD, the parity baseline for
  every `p3t*` driver that reads or writes a design file.
- For `p4t1` only: a **JDK 25** (`JAVA25_HOME`), the clone's HEAD jar
  (`FREEROUTING_JAR`) and the fixture corpus at
  `$FREEROUTING_JAVA_DIR/fixtures`. `run.sh` exports `FREEROUTING_JAR`,
  `P4T1_FIXTURES`, `P4T1_DATA` and `P4T1_PROCESSORS` for both sides and adds
  `-XX:ActiveProcessorCount=4` to the JVM; nothing else in the harness reads
  those.
- For `p5t1`/`p5t2` only: a **JDK 25** (`JAVA25_HOME`), the clone's HEAD jar
  (`FREEROUTING_JAR`, which `run.sh` exports for both sides) and the fixture
  corpus at `$FREEROUTING_JAVA_DIR/fixtures`. `run.sh` adds
  `-Duser.language=en -Duser.country=US` — load-bearing, not hygiene: every
  `%.4f` in a violation description goes through `String.formatted`, which uses
  the default FORMAT locale, so a German JVM writes `expected: 0,0500 mm`
  (plan 5 ruling 6) while the port's formatter is locale-free — and
  `-XX:+UnlockExperimentalVMOptions -XX:hashCode=${P5T_HASH_MODE:-2}`, because
  `DesignRulesChecker` iterates `HashSet<Item>` over a class with no `hashCode`
  override in three places and its answer otherwise moves between runs
  (plan 5 rulings 3 and 4).

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
  `completeShape`/`divideLargeRoom`, which `p6t2` covers (Plan 6 Task 3 — the
  gap this sentence has documented since Plan 2 is closed). Modes 0-2 differ only in which
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
  `overlappingTreeEntriesWithClearance` queries, `deepCopy` followed by a
  full re-dump of the copy's items plus the same 150 queries replayed
  against it, and a `hashEqual` boolean. No mode argument: every run
  exercises the same mix. **Verified** at 10 seeds
  (1/2/3/7/42/555/77/12345/999983/20260828) × `n` in `{30, 120}` — all 20
  runs **match exactly**, 499-1111 lines each depending on how many
  crossings `normalizeAllTraces` finds and splits among the random traces
  (see the driver description above — normalisation *increases* the trace
  count here, it does not fold traces together). Also re-verified that this
  sweep does not regress `p2t10` (all 9 modes), `p2t11` (all 11 modes
  reached by the differential driver — mode 11's one documented divergence
  unchanged) or `p2t13` (mode 0).

- `p3t2 <count> <seed> <mode>` — Java number formatting (Plan 3 Task 2), the
  randomised counterpart of `crates/fr-dsn/tests/number_format.rs`. One line
  per value: the raw bits in hex, `Double.toString`, `Float.toString` of the
  `float` cast, and — in every mode but 0 — `formatPlacementRotation`. Modes:
  `0` uniformly random 64-bit patterns (NaN and the infinities skipped: they
  are pinned by the unit tests, and 2^52 of the patterns are NaN; rotation
  formatting is omitted here because `%.0f` of a 1e300-scale double is a
  300-digit line), `1` `(nextInt(2000001) - 1000000) / 10^nextInt(7)` — the
  shape real DSN coordinates take, and the only mode that hands the rotation
  formatter negative values, `2` random integers in ±10^7, `3` random
  rotations in [0, 360) quantised to three decimals. **Verified** at
  `10000000 42 0` plus `1000000 42 <1|2|3>`, and again at seed `7` with the
  same counts: **26 M values, all four modes, zero diff lines.** The first
  smoke run at 100k found two real porting bugs (Java's even-last-digit tie
  rule — see "`p3t2`" below), so the volume is doing work.

- `p3t3 <file>` — the Specctra DSN lexer's token stream over one file (Plan 3
  Task 3): the driver behind `fr-dsn`'s `lexer` module, whose DFA tables are
  generated from the Java by `scripts/gen-lexer-tables.py`. The default
  argument is `tests/reference/tutorial_board/roundtrip.dsn`. **Verified** over
  the whole corpus — all 105 `.dsn`, 12 `.ses` and 7 `.rules` fixtures in
  `../freerouting/fixtures/` plus the 14 files under `tests/reference/`, 138
  files, **0 diff lines**. Task 15 re-ran the non-`.dsn` half directly (33
  files, 0 diffs) and the 105 `.dsn` half through `p3t15` mode 4, which
  delegates to `P3T3.main` on the Java side and shares `token_dump.rs` on the
  Rust side. To repeat
  the sweep without recompiling per file, run `run.sh p3t3` once and then loop
  the two binaries it left behind:

  ```sh
  ./scripts/differential/run.sh p3t3            # compiles both sides
  jar=tools/freerouting-2.3.0.jar
  for f in ../freerouting/fixtures/*.{dsn,ses,rules} tests/reference/*/*.{dsn,ses}; do
    java -cp "scripts/differential/build/classes-p3t3:$jar" \
      app.freerouting.io.specctra.P3T3 "$f" > /tmp/j.out
    ./scripts/differential/rust/target/release/p3t3 "$f" > /tmp/r.out
    diff -q /tmp/j.out /tmp/r.out || echo "DIFF $f"
  done
  ```

- `p3t15 <file.dsn> <mode>` — the DSN reader plus all three writers over one
  fixture (Plan 3 Task 15), and the only driver that reaches the scanner's
  hand-rolled `nextString`/`nextStringList`/`nextDouble` bypass (see the
  driver's entry under "Layout"). The default arguments are
  `tests/reference/Issue413-test/roundtrip.dsn 1`. **Verified** by
  `sweep-p3t15.sh` over all five modes × all 106 fixtures — 530 pairs, 525
  MATCH, 5 expected `XDIFF`s and **0 unexpected diffs** (table below):

  ```sh
  ./scripts/differential/sweep-p3t15.sh          # all five modes, all 106 fixtures
  ./scripts/differential/sweep-p3t15.sh 1        # just DsnWriter
  SWEEP_OUT=/tmp/sweep ./scripts/differential/sweep-p3t15.sh   # keep the differing outputs
  ```

- `p4t1 <cases.tsv> <case-index|all> <mode>` — Java's real headless settings
  composition against `fr_settings::resolve_headless` (Plan 4 Task 9). The
  default arguments are `matrix/p4t1-cases.tsv all 0`, which is the full run —
  84 cases, 5 728 lines, **0 diffs**. Measured on the reference machine
  (JDK 25, `-XX:ActiveProcessorCount=4`): 2.3 s for the JVM side, 0.6 s for the
  Rust side, ~15 s for the whole `run.sh` invocation once both sides are built
  (most of that is `javac`). Mode 1 — the Gson dump — is the same 84 cases,
  4 287 lines, 0 diffs (Plan 4 Task 10).

  ```sh
  ./scripts/differential/run.sh p4t1                    # all 84 cases, mode 0
  ./scripts/differential/run.sh p4t1 scripts/differential/matrix/p4t1-cases.tsv all 1  # the Gson dump
  ./scripts/differential/run.sh p4t1 scripts/differential/matrix/p4t1-cases.tsv 8 0   # one case
  ```

  Mode 1's Java output, with its two header lines stripped, is committed as
  `crates/fr-settings/tests/golden/p4t1-mode1/all.txt`, so
  `crates/fr-settings/tests/json.rs::p4t1_mode_1_parity` keeps the Gson parity
  pinned without a JVM. Regenerate it with

  ```sh
  ./scripts/differential/run.sh p4t1 scripts/differential/matrix/p4t1-cases.tsv all 1
  tail -n +3 scripts/differential/build/p4t1.j.out \
    > crates/fr-settings/tests/golden/p4t1-mode1/all.txt
  ```

  **The transcription risk, and how to re-check it.** `P4T1.java` is the only
  driver whose ground truth is a *sequence* of Java calls rather than one
  method, so a mis-transcribed step would make both sides agree on the wrong
  answer. The mitigation is that every statement carries the Java line it
  stands for, and the five ranges are short enough to read end to end:
  `Freerouting.java:1408-1413` (the prototype merger),
  `Freerouting.java:125-146` (merge #1),
  `HeadlessBoardManager.java:739-748` (the between-merges `setLayerCount` +
  `applyBoardSpecificOptimizations`, reached from
  `RoutingJobScheduler.java:93-96`), `RoutingJobScheduler.java:103-170`
  (merge #2, including `new ApiSettings(job.routerSettings)` at priority 70)
  and `RoutingJobScheduler.java:172-186` (the post-merge `RulesReader.read`
  and the final `applyBoardSpecificOptimizations`). Two substitutions are
  deliberate and are marked at the site: the board is `BProbe.java`'s
  synthetic recipe unless the row says `dsn` (`applyBoardSpecificOptimizations`
  reads only the bounding box, the layer count and each layer's `isSignal`),
  and `job.name` is null in the CLI path so the design name passed to
  `RulesReader.read` is the literal `"board"` — a header mismatch there is
  non-fatal (`RulesReader.java:100-110`).

  **One `.rules` file, two parses — this driver is what found it.** Java parses
  the scheduler's `.rules` file **twice**, with two different layer structures:
  at priority 40 through `RulesFileSettings` →
  `RulesReader.readRouterSettings`, whose structure is discovered from the file
  itself (`RulesReader.java:238-273`), and again after the merge through
  `RulesReader.read(…, board, settings)`, whose structure is the **board's**
  (`:112`). They disagree whenever the file names fewer layers than the board
  has — a two-`layer_rule` file on a four-layer board puts `B.Cu` at index 3 in
  the second parse and at index 1 in the first.

  An earlier revision of `SettingsInputs` took one pre-parsed `RouterSettings`
  for both slots, and `p4t1` fed it the board-structured parse; that diverged
  from the JVM on **13 of these 84 rows** — every `dsn4-*` row with a `.rules`
  file — in `layers[1]` versus `layers[3]`'s `routable` and
  `preferredDirectionHorizontal`, because the matrix's `.rules` files name only
  `F.Cu` and `B.Cu`. Controller ruling N fixed it at the root (Task 8 fix round
  2, quirk #142): `SettingsInputs.cli_rules` and `.scheduler_rules` are now the
  file's **bytes**, and `resolve_headless` performs both parses itself. This
  driver hands them over unparsed, so the two sides no longer share a parsing
  decision at all.

- `p5t1 <dsn> [rules|-] [ses|-]` — the DRC report (Plan 5 Task 10). The default
  argument is the dev board, `Issue575-drc_dev-board_4_hole_clearance_violations.dsn`:
  937 lines, **0 diffs**. Measured on the reference machine (JDK 25,
  `-XX:hashCode=2`): 0.7 s for the JVM side, 0.1 s for the Rust side.

  ```sh
  ./scripts/differential/run.sh p5t1                       # the dev board
  ./scripts/differential/run.sh p5t1 ../freerouting/fixtures/Issue593-BBD_Mars-64.dsn \
      ../freerouting/fixtures/Issue593-BBD_Mars-64.rules   # the .rules path
  ./scripts/differential/run.sh p5t1 ../freerouting/fixtures/Issue593-BBD_Mars-64.dsn \
      - ../freerouting/fixtures/Issue593-BBD_Mars-64.ses   # the session path
  ./scripts/differential/sweep-p5t1.sh                     # all 112 rows
  P5T_HASH_MODE=3 ./scripts/differential/sweep-p5t1.sh     # prove a diff Java-side
  ```

- `p5t2 <dsn> [rules|-] [ses|-] <mode 0-4>` — the algorithm-level lists (Plan 5
  Task 10). The default is the dev board, mode 0. Modes 0, 1, 3 and 4 are
  strict parity surfaces; mode 2 compares the port against the jar's own
  hash-seeded ratsnest and is graded against a budget (see below).

  ```sh
  ./scripts/differential/run.sh p5t2                       # the dev board, the clearance list
  ./scripts/differential/run.sh p5t2 ../freerouting/fixtures/Issue269-z10_module.dsn - - 3
  ./scripts/differential/sweep-p5t2.sh                     # all 5 modes × 112 rows
  ./scripts/differential/sweep-p5t2.sh 3                   # just the ratsnest parity surface
  P5T2_UNION=1 ./scripts/differential/sweep-p5t2.sh 2      # + the six-hash-mode airline union
  ```

- `p6t2 <seed> <n> <rooms>` — `AutorouteSearchTreeExt::{complete_shape,
  divide_large_room}` against `ShapeSearchTree.completeShape`/`divideLargeRoom`
  (Plan 6 Task 3). Defaults `42 20 2000`: 20 random obstacle areas on the
  `P2T10.java` board and 2 000 random seed rooms **per angle regime**, so one
  run is 6 000 `completeShape` calls and 6 000 `divideLargeRoom` calls. Each
  seed room draws its shape as a null (the whole plane), an `IntBox` or a
  clipped `IntOctagon`, so every regime sees both the shapes it accepts and the
  ones its `instanceof` guard rejects; the ignored object is drawn from
  `{null, one of the three seed rooms, one of the three pins}` and the ignore
  shape from `{null, a random box, one of the three seed rooms grown by a random
  margin}` — the last of those is what makes `ignoreShape.contains(intersection)`,
  the only branch of the three `completeShape`s that reads `ignoreShape` at all,
  fire often rather than never.

  ```sh
  ./scripts/differential/run.sh p6t2            # 42 20 2000 — 26 979 lines, 0 diffs
  ./scripts/differential/run.sh p6t2 7 40 2000  # a denser board
  ./scripts/differential/run.sh p6t2 5 5 2000   # a sparser one
  ```

- `p6t3 <mode> <seed> <n> <rooms>` — the three neighbour sorters against their
  Java originals, and the comparators behind them (Plan 6 Tasks 4 and 5).
  Defaults `0 42 20 1000`. Per call it prints every sorted neighbour (the
  touching side numbers, the corner flags or the first/last side pair, the
  corners, the neighbour object's kind and id, its shape and the intersection
  with their corner lists), the deferred own-net list or the target doors, and
  the completed room's door list (`first_room`, `second_room`, `dimension`,
  shape corners) — geometry and order, never counts.

  Modes `0`-`5` are the any-angle class, `6`/`8` the 45-degree one and `7`/`9`
  the orthogonal one; `6`/`7` drive `calculateNeighbours` and `8`/`9` the whole
  of `complete` against a real Java `AutorouteEngine`.

  ```sh
  ./scripts/differential/run.sh p6t3                  # 0 42 20 1000 — 7 597 lines, 0 diffs
  ./scripts/differential/run.sh p6t3 4 42 30 1000     # the grid board: corner touches
  ./scripts/differential/run.sh p6t3 3 42 0 2000      # the hazard-F probe (quirk #160)
  ./scripts/differential/run.sh p6t3 0 5 5 2000       # a sparser board
  ./scripts/differential/run.sh p6t3 5 42 20 1000     # the whole of `complete`, with an engine
  ./scripts/differential/run.sh p6t3 6 42 30 1000     # the 45-degree sorter
  ./scripts/differential/run.sh p6t3 7 42 30 1000     # the orthogonal sorter
  ./scripts/differential/run.sh p6t3 8 42 20 1000     # `complete` on a 45-degree tree
  ./scripts/differential/run.sh p6t3 9 42 20 1000     # `complete` on a 90-degree tree
  ```

  **What mode 3 buys, and why it exists.** The ratsnest has exactly one free
  choice in it: `NetIncompletes.calculateNetItems` seeds its outer loop from
  `uniqueItems.iterator().next()` over a `HashSet<Item>`
  (`NetIncompletes.java:295,:299`) whose element class has no `hashCode`
  override, so there is no Java order to port and plan 5 ruling 3 fixed the
  port's at the lowest item id. Comparing the port's airlines against *a*
  JVM run therefore compares two different algorithms' inputs, which is why
  ruling 4 declared the airline list informational. Mode 3 removes the variable
  instead of tolerating it: `P5T2.java` transcribes the constructor and makes
  the seed a parameter, so the port can be compared against Java-with-the-port's-
  seed-order — and then the endpoints match, exactly, on all 112 rows.

  Modes 3 and 4 print a second airline block for that comparison:
  `ALD <net> <fromId> <toId>`, one line per airline **in Kruskal's acceptance
  order** and with the edge's own direction, neither sorted nor normalised. Mode
  2's canonical `AL` block (the unordered pair, sorted by `(net, low, high)`)
  stays exactly as it was, because on mode 2 the two dropped artifacts — which
  end of an edge a run calls "from", and where an airline sits in the accepted
  sequence — are hash noise. With the seed pinned on both sides they are facts
  about the algorithm, so mode 3 gates on them: `AL` says what the airline set
  is, `ALD` says how it was built. `sweep-p5t2.sh 3` is 112 MATCH with both
  blocks in place.

  **The transcription risk, and how it is checked.** Like `P4T1.java`, mode 3's
  ground truth is transcribed rather than called, so a mis-transcription would
  make both sides agree on the wrong answer. Mode 4 is the guard, and it is a
  guard the harness runs rather than a claim in prose: the *same* transcription,
  seeded Java's way, is compared inside the JVM against the real
  `getAllAirlines()`, and prints `TRANSCRIPTION equal 0` only if every line
  agrees — `ALD` lines included, so the self-check covers the direction and the
  acceptance order mode 3 gates on. It does, on all 112 rows. Every statement additionally carries the
  `NetIncompletes.java` line it stands for; the ranges are `:80-116` (the
  filter), `:135-163` (grouping and the group count), `:166-205` (the
  triangulation, the `TreeSet<Edge>` and Kruskal), `:225` → `:259-275`
  (`calcLengthViolation`), `:293-322` (`calculateNetItems`), `:328-337`
  (`joinConnectedSets`) and `:344-397` (`Edge` and `NetItem`).

## Deferred coverage and cleanups

Recorded here rather than only in a task report, so they survive into the next
plan. None blocks anything; each is a known gap in this harness or in what it
covers.

| Item | Why it is open | Raised by |
|---|---|---|
| `p3t2` mode 0 never formats a rotation above `1e7` | `formatPlacementRotation` is exercised in modes 1-3, whose generators keep values inside DSN coordinate ranges and `[0, 360)`. `Double.toString` switches to `E` notation at `1e7`, and no mode drives a *rotation* across that boundary — so the `String.format("%.3f", …)` path is unproven for a value that large. Java only ever passes it a placement angle, so nothing reachable produces one; it is coverage debt, not a suspected bug. | Plan 3 Task 2 review |
| `crates/fr-dsn/src/format/double.rs` shadows `point` twice (`:148` `i32`, `:154` `usize`) | Deliberate — the first is signed so the "value below 1" branch can subtract, the second is the index the layout loop needs — but two bindings of one name in twelve lines is easy to misread. A rename (`point_signed` / `point`) is a safe, mechanical change nobody has had a reason to make yet. | Plan 3 Task 2 review |
| `JavaRandom` is copied into four driver binaries | `t15`, `t16r`, `p2t13` and `p3t2` each carry their own transcription of `java.util.Random`'s LCG. They agree today (every driver that uses one is zero-diff), but four copies is four chances to drift. The package has had a shared module since Plan 3 Task 15 (`src/token_dump.rs`, included with `#[path]`); the same mechanism would collapse these four. | Plan 3 Task 2 review |

## Known, expected diffs

Verified at HEAD, default smoke-run arguments, JDK 23 — except `p2t10`,
`p2t11`, `p2t13`, `p2t15`, `p3t2`, `p3t3` and `p3t15`, which need a JDK 25
(see Requirements above). `p3t3` and `p3t15` additionally run against the
pinned `tools/freerouting-2.3.0.jar`, not the clone's HEAD build (ruling 10).

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
| `p2t15` (seed 42, n=30, default) | 521 | 0 | exact match |
| `p6t2` (seed 42, n=20, 2000 rooms) | 26979 | 0 | exact match (6 000 `completeShape` + 6 000 `divideLargeRoom` calls, 2 000 per angle regime) |
| `p6t2` (seeds 7/123/999/20260829, n=40) | 26433-26933 | 0 | exact match |
| `p6t2` (seed 0 n=60, seed 42 n=120, seed 5 n=5) | 12809-29588 | 0 | exact match (denser and sparser boards) |
| `p6t3` mode 0 (seed 42, n=20, 1000 rooms) | 7597 | 0 | exact match (`calculateNeighbours`, its sorted set and its doors) |
| `p6t3` mode 0 (seeds 7/123/999/20260829/0/5) | 7135-20110 | 0 | exact match |
| `p6t3` mode 4 (grid obstacles, seeds 42/7/999/20260829) | 8192-17951 | 0 | exact match — the only mode that reaches the dimension-0 corner-touch branch |
| `p6t3` mode 1 (20 000 comparator probes) | 179296 | 0 | exact match (250 silent `TreeSet` drops, all the same ones) |
| `p6t3` mode 2 (30 000 same-side probes) | 262021 | 0 | exact match (8 374 drops) |
| `p6t3` mode 3 (30 000 corner-touch probes, seeds 20260829/7) | 292682, 293041 | 0 | exact match — **with `JavaTreeSet`**; on a `BTreeSet` this mode diffs (quirk #160) |
| `p6t3` mode 5 (the whole of `complete`, seeds 42/7/999/20260829/0) | 25740-30317 | 0 | exact match (`tryRemoveEdge`, `calculateNewIncompleteRooms`, `calculateTargetDoors`), minus the ~0.4 % of calls quirk #162 makes non-terminating |
| `p6t3` mode 6 (45-degree `calculateNeighbours`, seeds 42/7/999/20260829) | 8817-21050 | 0 | exact match (`Sorted45DegreeRoomNeighbours`, its own inner class, `edgeInteriorTouchesObstacle` and the `overlap` probe's computed `dim=2` door) |
| `p6t3` mode 7 (orthogonal `calculateNeighbours`, seeds 42/7/999/20260829) | 9166-21501 | 0 | exact match (`SortedOrthogonalRoomNeighbours`, likewise) |
| `p6t3` mode 8 (the whole of `complete` on a 45-degree tree, seeds 42/7/999/20260829) | 45661-53791 | 0 | exact match — and the mode that found quirk #163 |
| `p6t3` mode 9 (the whole of `complete` on a 90-degree tree, seeds 42/7/999/20260829) | 31052-35750 | 0 | exact match |
| `p2t15` (10 seeds × n∈{30,120}) | 499-1111 | 0 | exact match at every one of the 20 seed/n combinations (see "`p2t15` sweep" below) |
| `p3t2` (mode 0, seed 42) | 10000000 | 0 | exact match (`Double.toString`/`Float.toString` over random bit patterns) |
| `p3t2` (mode 1, seed 42) | 1000000 | 0 | exact match (DSN-coordinate-shaped values; adds `formatPlacementRotation`) |
| `p3t2` (mode 2, seed 42) | 1000000 | 0 | exact match (integers in ±10^7) |
| `p3t2` (mode 3, seed 42) | 1000000 | 0 | exact match (rotations in [0, 360), three decimals) |
| `p3t2` (all four modes, seed 7) | 13000000 | 0 | exact match (same counts, second seed) |
| `p3t3` (`tests/reference/tutorial_board/roundtrip.dsn`, the default) | 80308 | 0 | exact match |
| `p3t3` (all 105 `fixtures/*.dsn`) | 105 files, 73-205603 lines each | 0 | exact match on every file |
| `p3t3` (all 12 `fixtures/*.ses` + 7 `fixtures/*.rules`) | 19 files | 0 | exact match on every file |
| `p3t3` (the 14 `tests/reference/*/{roundtrip.dsn,unrouted.ses}`) | 14 files | 0 | exact match on every file (7 stems since Task 15's ruling-G additions) |
| `p3t15` (`tutorial_board.dsn`, modes 0/1/2/3/4) | 441 / 38869 / 27 / 94 / 76602 | 0 | exact match — items+warnings, `DsnWriter`, `SesWriter`, `RulesWriter`, tokens |
| `p3t15` (`Issue413-test.dsn`, modes 0/1/2/3/4) | 39 / 378 / 130 / 42 / 1097 | 0 | exact match — the fixture with traces, wiring vias, fixed states and SES `(wire` entries |
| `p3t15` sweep (all 5 modes × all 106 fixtures = **530 pairs**) | see below | **0 unexpected** | 525 MATCH + 5 `XDIFF` (`Issue229` modes 0-3, `empty_board` mode 3), both explained in the next two rows |
| `p3t15` (`Issue229-display-8-digit-hc595.dsn`, modes 0-3) | Java 3 / 59 / 18 / 0 vs Rust 502 / 3907 / 1922 / 55 | XDIFF | **Expected** (controller ruling E). The 2.3.0 jar's `DsnFile.readStringScope` has no resync loop where the clone's HEAD does, and the port follows HEAD (the plan's Java source authority), so the two readers legitimately build different boards from this file. The 2.3.0 reader gives up after one item where the port builds 501; mode 3 is Java 0 lines because the collapsed board also has no library, so `RulesWriter` NPEs on it as it does for `empty_board.dsn` below. Mode 4 — the raw token stream — still MATCHes on this file, which is the evidence that the divergence is in the parser and not in the scanner. Never "fixed" by changing the port. |
| `p3t15` (`empty_board.dsn`, mode 3) | Java 0 / Rust 20 | XDIFF | **Expected** — Java throws. The file has no `(library …)` scope at all, so `BoardLibrary.padstacks` stays `null` and `RulesWriter.writeRules` NPEs on `padstacks.count()` (`NullPointerException: Cannot invoke "app.freerouting.core.Padstacks.count()" because "p_par.board.library.padstacks" is null`). The port's field is a value, not a reference, so it writes a complete 20-line `.rules` file. New in Task 15; recorded in `docs/java-quirks.md`'s totalization table. |
| `p4t1` (`matrix/p4t1-cases.tsv`, `all 0`) | 5728 | 0 | exact match — 84 cases (Task 8's 64-case matrix + 20 Task 9 rows) of the real two-merge headless composition against `fr_settings::resolve_headless` |
| `p4t1` (`matrix/p4t1-cases.tsv`, `all 1`) | 4287 | 0 | exact match — the same 84 cases through `GsonProvider.GSON` vs `RouterSettings::to_json_string_pretty` (Plan 4 Task 10) |
| `p4t1` (`matrix/p4t1-cases.tsv`, `8 2`) | 5728 | 0 | exact match — mode 2 ignores the case index and runs the whole table as mode 0, so this is the `all 0` run reached the other way |
| `p5t1` (`Issue575-drc_dev-board…dsn`, the default) | 937 | 0 | exact match — the whole `-drc` JSON document through `GsonProvider.GSON` vs `KiCadDrcReport::to_json(FreeroutingHead)` |
| `p5t1` sweep (all **112 rows**) | 13-37000 per row, 482253 over the 104 corpus rows | **0 unexpected** | 94 MATCH + 18 `XDIFF`, all one class (ruling S, next row); 1 SKIP (`Issue006-LPC18XX_43XX_SCH.dsn`, which neither reader accepts). 110 s. |
| `p5t1` (17 fixtures, the ruling-S rows) | — | 1-39 `violations` entries | **Expected** — the hash-ordered `findRepresentativeItem` decides which dangling trace the dedup at `DesignRulesChecker.java:160` drops. Checked, not waived: `sweep-p5t1.sh` pins the differing item uuids per row and requires the rest of the document to be byte-identical. See "The `p5t*` sweeps" below for the `-XX:hashCode=0..4` evidence. |
| `p5t2` (mode 0, all 112 rows) | 2-1374 | 0 | exact match on every row — `getAllClearanceViolations()`, in list order, with `Double.toString` floats |
| `p5t2` (mode 1, all 112 rows) | 5-565 | 0 unexpected | 94 MATCH + 18 `XDIFF` — the same ruling-S class as `p5t1`, one layer lower |
| `p5t2` (mode 2, all 112 rows) | 4-1540 | 0 unexpected | 50 MATCH + 62 `XDIFF` — the counters are strict everywhere but one listed row; the `AL` block is graded against `sweep-p5t2.sh`'s recorded budget, because the port's seed order and the jar's are two different inputs to the same triangulation (ruling 4) |
| `p5t2` (mode 3, all 112 rows) | 4-1540 | **0** | exact match on every row — the ratsnest with the seed order pinned identically on both sides: `MAXCONN`, `INCOMPLETE`, every `NET` line, `ALCOUNT` **and every airline endpoint** |
| `p5t2` (mode 4, all 112 rows) | 2 | 0 | exact match on every row — `TRANSCRIPTION equal 0`, i.e. mode 3's transcription reproduces the jar's own `getAllAirlines()` when seeded the jar's way |

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

## `p3t2` (Plan 3 Task 2)

Two things Rust's own float formatting gets differently from Java, both found
by this driver rather than by reading the JDK spec, and both now pinned by
`crates/fr-dsn/tests/number_format.rs`:

* **`Double.toString`/`Float.toString` break an exact tie to an even last
  digit; Rust's shortest formatter does not.** When the value sits exactly
  halfway between the two nearest decimals of the shortest length, both
  round-trip, so the JDK 19+ spec's "the one whose least significant digit is
  even" decides. `1055987014896502.25` is `1.0559870148965022E15` in Java and
  `1.0559870148965023e15` from Rust's `{:e}`; `169903.625f` is `169903.62`
  in Java and `169903.63` from Rust. Both showed up inside the first 15k
  values of the mode-0 smoke run.
* **`String.format(Locale.ENGLISH, "%.Nf", d)` does not round the double's
  exact binary value.** `java.util.Formatter` takes the *shortest round-trip
  digits* — the same digits `Double.toString` prints — zero-pads them when
  more precision is asked for than they carry, and rounds them `HALF_UP` when
  less is. So `%.2f` of `8.475` is `8.48` (the digits are `8475`), even though
  the double is `8.47499999999999964…`, which rounds to `8.47`; and `%.3f` of
  `0.1235` is `0.124` where the exact expansion gives `0.123`. Rust's `{:.N}`
  is exact-and-half-to-even and disagrees on both counts. Confirmed on JDK 25
  over two million random doubles that the Formatter's digit string always
  equals `Double.toString`'s, so `fr-dsn` derives both from one routine.

## `p2t15` sweep (Plan 2 Task 15)

`p2t15` consolidates Task 15's brief into the ONE randomised board-level
driver: a shared xorshift stream drives `n` random pin/via/trace insertions
plus three fixed obstacle areas and three fixed conduction areas through the
real `RoutingBoard`/`Board`, `normalizeAllTraces`/`normalize_all_traces`,
every item's fields (id, kind, layer range, nets, clearance class, bounding
box, tile shapes), 100 `overlappingObjects` queries, 50
`overlappingTreeEntriesWithClearance` queries, `deepCopy`/`deep_copy`
followed by a full re-dump of the copy's items plus the same 150 queries
replayed against it, and a `hashEqual` boolean. Swept at 10 seeds × `n` ∈
{30, 120} — every one of the 20 runs matches exactly, zero diff lines:

| seed | n | lines | diff |
|---|---|---|---|
| 1 | 30 | 505 | 0 |
| 1 | 120 | 1085 | 0 |
| 2 | 30 | 511 | 0 |
| 2 | 120 | 1085 | 0 |
| 3 | 30 | 507 | 0 |
| 3 | 120 | 1109 | 0 |
| 7 | 30 | 503 | 0 |
| 7 | 120 | 1023 | 0 |
| 42 | 30 | 521 | 0 |
| 42 | 120 | 1039 | 0 |
| 555 | 30 | 499 | 0 |
| 555 | 120 | 1085 | 0 |
| 77 | 30 | 509 | 0 |
| 77 | 120 | 1043 | 0 |
| 12345 | 30 | 509 | 0 |
| 12345 | 120 | 1049 | 0 |
| 999983 | 30 | 505 | 0 |
| 999983 | 120 | 1111 | 0 |
| 20260828 | 30 | 509 | 0 |
| 20260828 | 120 | 1073 | 0 |

The line count varies with the seed because `normalizeAllTraces`
**increases** the trace count — it does not fold traces together. With only
four nets shared across `n` random segments in a modest coordinate range,
unplanned same-net traces frequently cross each other in the middle rather
than meeting end-to-end; each such crossing is a junction normalisation
splits both traces at, so the item list normally *grows* between insertion
and the dump (verified directly with a temporary instrumented run, not
inferred: 101 traces before `normalizeAllTraces` become 300 after at seed
11, n=300; 172 become 554 at seed 5, n=500) — more surviving trace items
means more `item …`/`tile[…]` lines, and how many crossings a given seed's
random layout happens to produce is what the line count actually tracks.
No line in any of the 20 runs differs from its Rust counterpart, so there
is nothing in this driver's output to add to `docs/java-quirks.md`: it
reaches no code path Tasks 10-13 hadn't already exercised, just through a
randomised, board-scale lens instead of the fixed scripts those tasks wrote
by hand. The re-run of `p2t10` (all 9 modes), `p2t11` (all 11 modes the
differential driver reaches) and `p2t13` (mode 0) alongside this sweep
confirms Task 15 did not regress any earlier driver; `p2t11` mode 11's one
pre-existing, documented `treeArraysEqual` divergence (the
tree-rebuild-vs-clone question, see the `p2t11` section above) is
unchanged.

Coverage this driver deliberately does not claim (a reviewer's finding,
recorded here so it is not mistaken for a gap that slipped through): the
per-item tile dump prints each tile's *bounding box*, not the tile shape
itself, so on its own it cannot tell an octagon apart from a box with the
same axis-aligned extent — `RegularTileShape::bounding_box()` reads
`leftX/rightX/bottomY/topY` straight off regardless of which bounding
directions cut the shape, so a 45-degree and a 90-degree tile can (and, on
`crates/fr-board/tests/consistency.rs`'s fixture, do) share a bounding box
while being genuinely different shapes; the overlap/clearance probes are
box shapes only, never octagons (already covered by `P2T10`/`P2T11`'s
fixed scripts, which exercise both); and every padstack and area shape this
driver builds is axis-aligned, so the 45-vs-90-degree tile-shape branch a
rotated pad or a diagonal-edged area would exercise is a no-op here. The
`overlappingTreeEntriesWithClearance` tie-break counter is snapshotted
before each query and restored after (matching `Board`'s own private
wrapper), rather than left running across all 50 calls; this is provably
output-neutral rather than a shortcut, since the counter is used only as a
monotonic tie-break *within* one query's own sort — its absolute starting
value never changes which entry wins a tie, only the numbers assigned to
each, and `P2T10`'s `query()` helper already resets the same counter to 0
on every call for the identical reason.

## The `p5t*` sweeps (Plan 5 Task 10)

Two drivers over the same 112 rows: `p5t1` compares the whole `-drc` JSON
document, `p5t2` the three raw lists behind it. Ruling 14 asks for both, so that
a report match cannot mask a compensating pair of errors — and the pair earned
that on the first run: `p5t2` mode 2 found a counter divergence
(`Issue269-z10_module.dsn`) that `p5t1` cannot see, because that fixture's
report has no airlines in it.

### The rows

104 corpus `.dsn` files plus the eight `tests/reference/drc-fixtures.txt` rows
(four of which repeat a corpus file, and which add the `.rules` path, the `.ses`
path and the tutorial board). Which corpus files are in is not the sweeps'
judgement: it is read out of `crates/fr-dsn/tests/data/corpus-read-results.txt`,
the golden of Plan 3's corpus read test, so a row is skipped only where the jar
itself rejects the file. Today exactly one is —
`Issue006-LPC18XX_43XX_SCH.dsn`, an OLE compound document, not a DSN.

Both sweeps run the JVM with `-XX:hashCode=2` and `-Duser.language=en
-Duser.country=US`. Wall clock on the reference machine (JDK 25, M-series):
`sweep-p5t1.sh` **110 s** (112 rows); `sweep-p5t2.sh` **425 s** (112 rows x 5 modes = 560
pairs).

### Mismatch class 1 — which dangling trace the dedup drops (ruling S, quirk #146)

`p5t1` on 17 fixtures; `p5t2` mode 1 on the same 17.

`generateReport` folds `getAllUnconnectedItems`' `track_dangling` entries into
`violations` (`DesignRulesChecker.java:271-276`). The *candidate* set — every
trace with a contact-free end (`:152-158`) — is hash-independent. The *emitted*
set is the candidates minus whichever of them the dedup at `:160` drops, namely
those that are some net entry's `firstItem`; and `firstItem` is
`findRepresentativeItem` over a `HashSet<Item>` (`:138`, `:186-200`), which
returns *a* Pin, or *a* Trace when the group holds no Pin — not a particular
one. Ruling 3 fixes the port's choice at the lowest item id. So on any board
with a Pin-free connected group holding several dangling traces, the port drops
a different trace than a given JVM run does.

**Evidence.** Each of the 17 fixtures was run against the HEAD jar under
`-XX:hashCode=0,1,2,3,4`, and on every one of them the Java side's own
`track_dangling` set moves between modes — 2 to 5 distinct sets over the five
runs — so no single JVM answer is "the" answer to port. Fifteen of the 17 also
satisfy the containment test against those five modes alone: every uuid the port
emits is one some JVM run emitted. The two that do not,
`Issue214-freerouting.dsn` and `Issue690-kit-dev-coldfire-xilinx_5213.dsn`,
satisfy it against a larger sample — 45 runs (modes 0, 1, 4 and the default, ten
times each, all four being PRNG- or address-seeded and therefore fresh on every
run) give 7 and 28 distinct sets and unions of 94 and 384 uuids, with the port's
set inside both.

The class is **checked, not waived**. `sweep-p5t1.sh` and `sweep-p5t2.sh` each
carry the differing item uuids per row and require the two documents to be equal
once exactly those entries are removed — same uuids, nothing else moved. On all
17 rows the `unconnectedItems` array, its `items` lists, every description,
every position and every non-`track_dangling` violation are byte-identical.

### Mismatch class 2 — which airlines the triangulation produces (ruling 4)

`p5t2` mode 2 on 61 of the 62 XDIFF rows (the 62nd is class 3); **not** mode 3, which matches
everywhere.

`NetIncompletes.calculateNetItems` seeds its outer loop off the same kind of
`HashSet<Item>` (`NetIncompletes.java:295,:299`), and that seed order is the
order the Delaunay corners are inserted in. With quirk #82 unfixed on both sides
— `PlanarDelaunayTriangulation`'s in-circle degeneracy on axis-aligned input,
which loses edges a real triangulation would have — a different insertion order
changes the edge *set*, not merely which of several equal-length edges Kruskal
accepts. Ruling 4 measured that and declared the endpoint list informational.

Mode 3 is Task 10's answer to it: `P5T2.java` transcribes the constructor so the
seed order becomes a parameter, and with it pinned the port's way **the whole
ratsnest matches on all 112 rows, endpoints included**. Mode 4 is the check on
that transcription — the same code seeded Java's way, compared inside the JVM
against the real `getAllAirlines()` — and prints `TRANSCRIPTION equal 0`
everywhere.

Mode 2 is kept because it is the port against the jar's *own* answer through the
real `DesignRulesChecker` accessors, which is what pins the counters. Its
counters are strict; its `AL` block is graded against `AIRLINE_BUDGETS`, the
recorded number of differing lines per row under `-XX:hashCode=2` (both sides
deterministic there, so the number reproduces). The table is a ratchet: a row
passes at or below its budget and fails above it.

**Why containment is not the corpus-wide gate.** The obvious stronger test —
every port airline must appear in the union of the JVM's six hash modes — does
not converge fast enough to be one. Measured on
`Issue022-AutoRouter_interrupted.dsn` (235 port airlines): the six-run union
holds 268 triples and leaves **11** port airlines outside it; a 60-run union
(modes 0-5, ten times each) holds 288 and still leaves **2**. That fixture's
mode 3 matches exactly, so nothing is wrong with it — the port's ascending-id
seed order is simply a point the JVM's identity-hash orders need not ever visit.
Containment *is* applied, strictly, for the three fixtures that have a committed
`crates/fr-drc/tests/data/*.airlines-union.txt` (ruling 4's own three, from Task
5); `P5T2_UNION=1` computes the six-mode sample for every differing row and
prints the number outside it, as information for a reviewer.

### Mismatch class 3 — one net where the two seed orders find a different *number* of airlines

`p5t2` mode 2, `Issue269-z10_module.dsn` only, listed in `COUNTER_XDIFFS`.

Net 1 of that board has 4 connected groups; the jar finds 3 airlines for it and
the port 2, so `INCOMPLETE` reads 117 against 116. The jar answers `NET 1 3 4`
under all six hash modes, so this is not the jar being hash-dependent. It is
class 2's cause reaching the count: a spanning "tree" that leaves two groups
unjoined is a normal outcome of the degenerate triangulation on *both* sides —
`count == groups - 1` fails on 13 of `Issue022-AutoRouter_interrupted.dsn`'s nets
identically on both sides. **Mode 3 is the proof**: with the seed order pinned
the port's way, Java answers `NET 1 2 4` too, and that fixture's mode 3 matches
line for line.

### The one class that must *not* appear here

`Issue229-display-8-digit-hc595.dsn` has a documented 2.3.0-vs-HEAD reader
divergence (`sweep-p3t15.sh`'s `EXPECTED_DIFFS`, controller ruling E of Plan 3):
the 2.3.0 jar's `DsnFile.readStringScope` has no resync loop where the clone's
HEAD does, so the two readers build different boards from that file. The `p5t*`
drivers run against **HEAD**, so it must not appear here — and it does not:
`p5t1` MATCHes on it, and `p5t2` MATCHes on modes 0, 1, 3 and 4, with only two
differing `AL` lines in mode 2. If that fixture ever starts diffing in `p5t1` or
in `p5t2` mode 0/1, the *reader* has regressed, not the DRC.

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
