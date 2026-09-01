#!/usr/bin/env bash
# Compile and run one Java-vs-Rust differential driver pair, then diff their
# stdout. See README.md for what each driver covers, its default arguments,
# and how the harness works.
set -euo pipefail

DIFF_ROOT="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$DIFF_ROOT/../.." && pwd)"
FREEROUTING_JAVA_DIR="${FREEROUTING_JAVA_DIR:-$ROOT/../freerouting}"
JAVA_DIR="$FREEROUTING_JAVA_DIR/src/main/java/app/freerouting"
JAVA_HOME="${JAVA_HOME:-/opt/homebrew/Cellar/openjdk/23.0.2/libexec/openjdk.jdk/Contents/Home}"
JAVAC="$JAVA_HOME/bin/javac"
JAVABIN="$JAVA_HOME/bin/java"

# `p2t10`, `p2t11`, `p2t15` and `p3t2` cannot be compiled from `geometry/planar` sources: the
# first three need the whole board stack (BasicBoard, BoardRules, the search trees) and `p3t2`
# needs `io/specctra`'s package-private `SesWriter.formatPlacementRotation`. They are compiled
# and run against the clone's own build output instead, which is class-file version 69 and
# therefore needs a JDK 25. `JAVA25_HOME` points at Homebrew's version-independent `opt` symlink
# rather than a pinned `Cellar` directory, so a `brew upgrade` does not break the harness;
# override either of these if your checkout differs.
JAVA25_HOME="${JAVA25_HOME:-/opt/homebrew/opt/openjdk@25/libexec/openjdk.jdk/Contents/Home}"
FREEROUTING_JAR="${FREEROUTING_JAR:-$FREEROUTING_JAVA_DIR/build/libs/freerouting-current-executable.jar}"
# The pinned 2.3.0 release jar, which is the parity baseline `tests/reference/` was generated
# with (Plan 3 ruling 10): the `p3t*` drivers run against this one, not the clone's HEAD build.
FREEROUTING_JAR_230="${FREEROUTING_JAR_230:-$ROOT/tools/freerouting-2.3.0.jar}"

BUILD="$DIFF_ROOT/build"
OUT="$BUILD/classes"

usage() {
  echo "usage: $0 <driver> [args...]" >&2
  echo "  drivers: t14, t15, t16r, e15, d17, p2t3, p2t3r, p2t10, p2t11, p2t13, p2t15, p3t2," >&2
  echo "           p3t3, p3t15, p4t1, p5t1, p5t2, p6t1, p6t2, p6t3, p7t3, p7t4," >&2
  echo "           p7t5, p7t6, p7t7, p7t10, p7t1, p7t2, p7t9" >&2
  echo "  args default to a smoke run per driver (see README.md); pass your" >&2
  echo "  own (e.g. iteration count, seed, mode) to override them entirely." >&2
  exit 1
}

driver="${1:-}"
[[ -n "$driver" ]] || usage
shift

# `javapkg` is the driver's Java package; `extra_java_sources` lists any real
# Java sources beyond `geometry/planar` that the driver needs compiled with it.
javapkg="geometry.planar"
extra_java_sources=()
# Set by `p2t10`: compile against the clone's prebuilt jar with a JDK 25 instead of against the
# `geometry/planar` sources with a JDK 23.
needs_jar=0
# Set by `p2t13`: compiled from the `geometry/planar` sources like every other source-path driver,
# but with the JDK 25 the shipping jar is built for, because the driver's ground truth depends on
# `java.util.Collections.shuffle`/`java.util.Random` — runtime library code, not freerouting code.
needs_jdk25=0
# Set by `p3t3`: use the pinned 2.3.0 jar rather than the clone's HEAD build (ruling 10).
needs_jar_230=0
# Set by `p3t15`: extra driver sources to compile alongside `$javaclass.java` in jar mode (it
# delegates its mode 4 to `P3T3.main`).
extra_jar_sources=()
# Set by `p4t1`: extra `java` flags, and extra environment both sides read. `p4t1` pins
# `Runtime.getRuntime().availableProcessors()` with `-XX:ActiveProcessorCount`, because
# `DefaultSettings.java:106,134` and `RouterSettings.validate` all consult it (plan ruling 6) —
# the Rust twin's `HostEnvironment::with_processors` reads the same number out of
# `P4T1_PROCESSORS`, and both sides print it in their header line so a mismatch is a diff.
java_flags=()
# Set by `p6t1`: a wall-clock bound, in seconds, applied to **both** sides of the run (empty means
# no bound). `timeout(1)` is coreutils'; on macOS it comes from `brew install coreutils` as either
# `timeout` or `gtimeout`, and the harness falls back to running unbounded — with a warning — when
# neither is on the PATH.
run_timeout=""
# The two `p5t*` drivers need three JVM flags beyond the shared `-Djava.awt.headless=true`:
#
#   * `-Duser.language=en -Duser.country=US` is load-bearing, not hygiene. Every `%.4f` in a
#     violation description goes through `String.formatted`, which uses the default FORMAT locale,
#     so a German JVM writes `expected: 0,0500 mm` (plan-5 ruling 6). The port's formatter is
#     locale-free, so without these two flags every `p5t1` run on a comma-decimal machine would
#     diff. `scripts/gen-drc-reference.sh` pins the same pair.
#   * `-XX:hashCode=2` pins `Object.hashCode` to the constant mode, the only source in the JVM that
#     reproduces run to run *and* is not derived from an object address. `DesignRulesChecker`
#     iterates `HashSet<Item>` over a class with no `hashCode` override in three places
#     (`:118-123`, `:138-139`, NetIncompletes.java:295), so the Java side's answer moves between
#     runs without it (plan-5 rulings 3 and 4). It is the mode `tests/reference/*/drc.json` was
#     generated under, which is what makes the expected-diff tables in `sweep-p5t1.sh` and
#     `sweep-p5t2.sh` reproducible. Override with `P5T_HASH_MODE=0..4` to sweep the modes — that
#     is how a new diff is proven Java-side rather than a port bug.
P5T_HASH_MODE="${P5T_HASH_MODE:-2}"
P5T_JAVA_FLAGS=(
  -Duser.language=en
  -Duser.country=US
  -XX:+UnlockExperimentalVMOptions
  "-XX:hashCode=$P5T_HASH_MODE"
)
DRC_FIXTURES="$FREEROUTING_JAVA_DIR/fixtures"

# The `datastructures` classes the Plan 2 Task 3 drivers exercise.
shapetree_sources=(
  "$JAVA_DIR/datastructures/ShapeTree.java"
  "$JAVA_DIR/datastructures/MinAreaTree.java"
  "$JAVA_DIR/datastructures/ArrayStack.java"
)
case "$driver" in
  t14) javaclass=T14; default_args=(200) ;;
  t15) javaclass=T15; default_args=(200 42) ;;
  t16r) javaclass=T16R; default_args=(200 42 0) ;;
  e15) javaclass=E15; default_args=() ;;
  d17) javaclass=D17; default_args=(200 0) ;;
  p2t3)
    javaclass=P2T3
    javapkg="datastructures"
    default_args=()
    extra_java_sources=("${shapetree_sources[@]}")
    ;;
  p2t3r)
    javaclass=P2T3R
    javapkg="datastructures"
    default_args=(400 42 0)
    extra_java_sources=("${shapetree_sources[@]}")
    ;;
  p2t10)
    javaclass=P2T10
    javapkg="datastructures"
    default_args=(0)
    needs_jar=1
    ;;
  p2t11)
    javaclass=P2T11
    javapkg="datastructures"
    default_args=(0)
    needs_jar=1
    ;;
  p2t13)
    javaclass=P2T13
    javapkg="datastructures"
    default_args=(50 42 0)
    extra_java_sources=("$JAVA_DIR/datastructures/PlanarDelaunayTriangulation.java")
    needs_jdk25=1
    ;;
  p2t15)
    javaclass=P2T15
    javapkg="datastructures"
    default_args=(42 30)
    needs_jar=1
    ;;
  p3t2)
    # Declares `package app.freerouting.io.specctra` so it can call the package-private
    # `SesWriter.formatPlacementRotation`, so it compiles against the jar like `p2t10`.
    javaclass=P3T2
    javapkg="io.specctra"
    default_args=(100000 42 0)
    needs_jar=1
    ;;
  p3t3)
    # The Specctra lexer's token stream over one file. Runs against the pinned 2.3.0 jar
    # (ruling 10); the driver looks the scanner's entry point up reflectively because it is
    # `next_token` there and `nextToken` at the clone's HEAD.
    javaclass=P3T3
    javapkg="io.specctra"
    default_args=("$ROOT/tests/reference/tutorial_board/roundtrip.dsn")
    needs_jar=1
    needs_jar_230=1
    ;;
  p3t15)
    # The DSN reader plus all three writers over one fixture. Like `p3t3` it runs against the
    # pinned 2.3.0 jar (ruling 10) — it is the driver that must reproduce `tests/reference/`'s
    # bytes. `P3T3.java` is compiled with it because mode 4 delegates to `P3T3.main`.
    javaclass=P3T15
    javapkg="io.specctra"
    default_args=("$ROOT/tests/reference/Issue413-test/roundtrip.dsn" 1)
    needs_jar=1
    needs_jar_230=1
    extra_jar_sources=("$DIFF_ROOT/java/P3T3.java")
    ;;
  p4t1)
    # Java's real headless settings composition (two merges, the between-merges board pass and
    # the post-merge `RulesReader.read`) over the Plan 4 precedence matrix, against
    # `fr_settings::resolve_headless`. Declares `package app.freerouting.settings;` and runs
    # against the clone's HEAD jar (plan ruling 7 — `SettingsMerger` and `settings/sources/**`
    # are what is being compared, and the 2.3.0 jar's `RoutingBoard` lives in another package).
    javaclass=P4T1
    javapkg="settings"
    default_args=("$DIFF_ROOT/matrix/p4t1-cases.tsv" all 0)
    needs_jar=1
    java_flags=(-XX:ActiveProcessorCount=4)
    export P4T1_PROCESSORS=4
    export P4T1_FIXTURES="$FREEROUTING_JAVA_DIR/fixtures"
    export P4T1_DATA="$ROOT/crates/fr-settings/tests/data"
    ;;
  p5t1)
    # The DRC report: `DesignRulesChecker.generateReport` through `GsonProvider.GSON` against
    # `report_to_json(FreeroutingHead)`. Declares `package app.freerouting.drc;` and runs against
    # the clone's HEAD jar (plan-5 ruling 1 — the DRC port's sources and every file:line in the
    # plan are HEAD's; the 2.3.0 jar spells nine of the JSON keys in snake_case).
    javaclass=P5T1
    javapkg="drc"
    default_args=("$DRC_FIXTURES/Issue575-drc_dev-board_4_hole_clearance_violations.dsn")
    needs_jar=1
    java_flags=("${P5T_JAVA_FLAGS[@]}")
    ;;
  p6t2)
    # Plan 6 Task 3: `ShapeSearchTree.completeShape` / `divideLargeRoom` in all three angle
    # regimes — the two methods `p2t10` skipped. Declares `package app.freerouting.board.searchtree`
    # so it can call the protected `divideLargeRoom` directly, so it compiles against the clone's
    # HEAD jar like `p2t10` (plan-6 global constraints: HEAD is the parity jar).
    javaclass=P6T2
    javapkg="board.searchtree"
    default_args=(42 20 2000)
    needs_jar=1
    java_flags=(-Duser.language=en -Duser.country=US -XX:+UnlockExperimentalVMOptions -XX:hashCode=2)
    ;;
  p6t1)
    # Plan 6 Task 17: one real DSN board, its first `maxItems` connections routed through steps
    # 1-5 of `AutorouteConnectionRouter.route` (plan-6 ruling 2's seam) — the driver behind
    # `scripts/gen-router-reference.sh` and `crates/fr-router/tests/reference_parity.rs`.
    # Declares `package app.freerouting.autoroute.maze` (the brief's package, so the driver can
    # reach package-private members of the engine if it ever needs to) and compiles against the
    # clone's HEAD jar like `p6t2`/`p6t3`.
    #
    # `P6T1_TIMEOUT` bounds the wall clock on **both** sides: quirk #162
    # (`SortedRoomNeighbours.calculateNewIncompleteRooms`) does not terminate for a small fraction
    # of room completions and neither language guards it, so a corpus connection can hang in Java
    # and in the port alike. Without the bound the harness would hang rather than report.
    #
    # Plan 7 Task 8 added a fifth and sixth argument: `steps` (`1-5` | `1-8`) and `neckWidthUm`.
    # `1-5` is unchanged in every byte, including the HEADER line the committed
    # `tests/reference/*/router.meta.txt` records; `1-8` runs
    # `AutorouteConnectionRouter.route` in full through `probes/P7T8Probe.java`, which is
    # compiled alongside because `route` and `BatchAutorouter.autorouteItem` are both
    # package-private in `app.freerouting.autoroute.pipeline`.
    javaclass=P6T1
    javapkg="autoroute.maze"
    default_args=("$FREEROUTING_JAVA_DIR/fixtures/Issue143-rpi_splitter.dsn" 8 1)
    needs_jar=1
    extra_jar_sources=("$DIFF_ROOT/java/probes/P7T8Probe.java")
    # The `p5t*` flag set rather than a hard-coded `-XX:hashCode=2`, so `P5T_HASH_MODE=0..4`
    # sweeps this driver too. The default is `2`, which is the mode
    # `tests/reference/router-*/router.jsonl` was generated under, so nothing changes unless the
    # variable is set. Plan 7 Task 8 made the swap: `--steps=1-8` reaches `optChangedArea` and
    # `ViaOptimizer`, neither of which Plan 6's hash-mode sweep covered, so the sweep has to be
    # available here to tell a port bug from a Java hash-order dependency.
    java_flags=("${P5T_JAVA_FLAGS[@]}")
    run_timeout="${P6T1_TIMEOUT:-900}"
    ;;
  p7t7)
    # Plan 7 Task 1: `core/scoring/BoardStatistics`' computing constructor, `isPinEscaped` and the
    # three score methods, over a board optionally routed first by `P6T1`'s own machinery.
    # Declares `package app.freerouting.autoroute.maze` — not the brief's `core.scoring` — so it
    # can call `P6T1.loadBoard` / `pickConnections` / `route`, which are package-private statics;
    # nothing in `core/scoring` is package-private, so the brief's package would buy no access.
    # `P6T1.java` is compiled alongside it, the `p5t2`/`P5T1` pattern: the two drivers cannot then
    # describe different boards.
    javaclass=P7T7
    javapkg="autoroute.maze"
    default_args=("$FREEROUTING_JAVA_DIR/fixtures/Issue143-rpi_splitter.dsn" 0 1)
    needs_jar=1
    extra_jar_sources=("$DIFF_ROOT/java/P6T1.java" "$DIFF_ROOT/java/probes/P7T8Probe.java")
    # The `p5t*` flag set rather than a hard-coded `-XX:hashCode=2`, so `P5T_HASH_MODE=0..4`
    # sweeps this driver too: `BoardStatistics` reaches `DesignRulesChecker` twice, and that class
    # iterates `HashSet<Item>` over a type with no `hashCode` override (plan-5 rulings 3 and 4).
    # The sweep is what turns "the score does not depend on `Object.hashCode`" into evidence.
    java_flags=("${P5T_JAVA_FLAGS[@]}")
    run_timeout="${P7T7_TIMEOUT:-900}"
    ;;
  p7t10)
    # Plan 7 Task 3 (controller ruling AH): the three hash-equality **decisions**
    # (`BatchFanout.java:152-156`, `BoardHistory.contains`, `BoardHistory.getRank`) over a
    # scripted board-mutation sequence — decisions, never hash values, which are not comparable
    # across the two languages by construction.
    #
    # Declares `package app.freerouting.autoroute.maze` — not the plan's `autoroute` — for the
    # same reason `P7T7` does: it needs `P6T1.loadBoard`/`pickConnections`/`route`, which are
    # package-private statics, while `BoardHistory`'s package-private cap constructor is reached
    # with `setAccessible(true)` from any package (the `P7T2Probe` precedent). `P6T1.java` is
    # compiled alongside so the two cannot describe different boards.
    #
    # `<dsn> <steps> [routeK] [mode]`; `mode` is `warm` (the acceptance mode) or `raw` (the
    # quirk #200 exposure measurement — see `P7T10.java`'s class comment).
    #
    # `P7T10_HASH_MODE` (default 2, the shared harness value) overrides `-XX:hashCode`, and on
    # this driver it is a **performance** knob as well as a determinism one. `getHash()` is an MD5
    # over `serialize(true)`, and `ObjectOutputStream`'s back-reference `HandleTable` buckets by
    # `System.identityHashCode` — which mode 2 pins to the constant 1, so every insert collides and
    # the table degenerates to a linear scan. On a board with a few hundred items and large
    # component-outline polygons that is quadratic: `tutorial_board.dsn` needs **6 s** for 40 steps
    # at `-XX:hashCode=0` and does not reach step 20 in 150 s at `-XX:hashCode=2` (jstack: `main`
    # RUNNABLE in `ObjectOutputStream$HandleTable.lookup`). It is a JDK/flag interaction, not a
    # freerouting behaviour and not a port one — and none of this driver's three decisions depends
    # on `Object.hashCode` (they are hash-string equalities and list positions), which the mode
    # sweep in the README demonstrates rather than assumes. Run the big stems with
    # `P7T10_HASH_MODE=0`.
    javaclass=P7T10
    javapkg="autoroute.maze"
    default_args=("$FREEROUTING_JAVA_DIR/fixtures/Issue143-rpi_splitter.dsn" 2000 0 warm)
    needs_jar=1
    extra_jar_sources=("$DIFF_ROOT/java/P6T1.java" "$DIFF_ROOT/java/probes/P7T8Probe.java")
    java_flags=(
      -Duser.language=en
      -Duser.country=US
      -XX:+UnlockExperimentalVMOptions
      "-XX:hashCode=${P7T10_HASH_MODE:-2}"
    )
    run_timeout="${P7T10_TIMEOUT:-1800}"
    ;;
  p7t1)
    # Plan 7 Task 9, item-selection level: `BatchAutorouter.getAutorouteItems`
    # (BatchAutorouter.java:345-409) — the pass's work list, with its `handledItems` set traced
    # step by step. Declares `package app.freerouting.autoroute.pipeline` (the brief's), which is
    # what reaches the package-private `getAutorouteItems`, `autorouteItem`, `autoroutePass` and
    # `removeTails`; `P7T2.java` is compiled alongside because the two drivers share the board,
    # the settings and the router, and must not be able to describe different ones.
    #
    # `<dsn> [passNo]`. `passNo > 1` runs `passNo - 1` real autoroute passes first, because the
    # board a pass selects from is the board the previous passes left — see `P7T1.java`'s class
    # comment. Acceptance is 0 diffs on all six corpus stems x passes 1-3.
    #
    # The `p5t*` flag set rather than a hard-coded `-XX:hashCode=2`, so `P5T_HASH_MODE=0..4`
    # sweeps this driver too: a pass reaches `DesignRulesChecker` and `BoardStatistics`, both of
    # which iterate `HashSet<Item>` over a type with no `hashCode` override (plan-5 rulings 3-4).
    javaclass=P7T1
    javapkg="autoroute.pipeline"
    default_args=("$FREEROUTING_JAVA_DIR/fixtures/Issue143-rpi_splitter.dsn" 1)
    needs_jar=1
    extra_jar_sources=("$DIFF_ROOT/java/P7T2.java")
    java_flags=("${P5T_JAVA_FLAGS[@]}")
    run_timeout="${P7T1_TIMEOUT:-1800}"
    ;;
  p7t2)
    # Plan 7 Task 9, pass level: `AutoroutePassRunner.runSingleThread`
    # (AutoroutePassRunner.java:151-336) — one whole autoroute pass. Same package and the same
    # reason as `p7t1`; `P7T1.java` is *not* compiled alongside, because P7T2 owns the shared
    # helpers and P7T1 depends on it rather than the other way round.
    #
    # `<dsn> [passNo] [maxItems|all]`. The driver prints two halves — a line-for-line
    # transcription of `runSingleThread`'s loop with `p6t1`'s JSON line per `(item, net index)`,
    # and then the **real** method on a freshly loaded board, with the two boards compared by
    # `getHash()` / `structural_hash` (an equality *decision*, never a hash value — ruling AH).
    # See `P7T2.java`'s class comment for why both halves exist.
    #
    # Acceptance is 0 diffs on all six corpus stems x passes 1-3 x `maxItems` in {2, all}.
    javaclass=P7T2
    javapkg="autoroute.pipeline"
    default_args=("$FREEROUTING_JAVA_DIR/fixtures/Issue143-rpi_splitter.dsn" 1 all)
    needs_jar=1
    java_flags=("${P5T_JAVA_FLAGS[@]}")
    run_timeout="${P7T2_TIMEOUT:-3600}"
    ;;
  p7t9)
    # Plan 7 Task 10, whole-board level: `AutorouteBatchLoop.run` (AutorouteBatchLoop.java:37-588)
    # — the pass loop, its best-board policy and its two stagnation detectors, i.e. the whole
    # `-dr`-equivalent routing stage. Same package and the same reason as `p7t1`/`p7t2` — `run`
    # reads `router.board`, `router.thread`, `router.job`, `router.removeUnconnectedVias` and the
    # seven package-private constants of `BatchAutorouter`, none of which is reachable from
    # outside `app.freerouting.autoroute.pipeline`. `P7T2.java` is compiled alongside because
    # `P7T9` loads its board, builds its settings and builds its router through P7T2's four
    # shared statics, so the three drivers cannot describe different boards.
    #
    # `<dsn> [maxPasses] [mode]`. `mode` is `router-only` (`fanout.enabled = false`,
    # `runOptimizer = false`) or, from Plan 7 Task 12, `router+fanout`, which turns the SMD
    # fanout pre-pass on and disables its per-pin clock on both sides through
    # `settings.fanout.maxMillisecondsPerPin` (ruling AI). `maxPasses = 0` is Java's
    # "unlimited" (quirk #140), so bound it with `P7T9_TIMEOUT` before using it on a big stem.
    # The driver prints two halves — a line-for-line transcription of `run`'s body with the
    # per-pass `PassRecord` tuple and every decision arm, and then the **real**
    # `BatchAutorouter.runBatchLoop()` on a freshly loaded board, with the two boards compared by
    # `getHash()` / `structural_hash` (an equality *decision*, never a hash value — ruling AH) —
    # then the final board in `P6T15aProbe`'s polyline format. See `P7T9.java`'s class comment
    # for why the driver calls `runBatchLoop()` rather than `RoutingPipeline.run()`.
    #
    # The `p5t*` flag set rather than a hard-coded `-XX:hashCode=2`, so `P5T_HASH_MODE=0..4`
    # sweeps this driver too: a pass reaches `DesignRulesChecker` and `BoardStatistics`, both of
    # which iterate `HashSet<Item>` over a type with no `hashCode` override (plan-5 rulings 3-4).
    javaclass=P7T9
    javapkg="autoroute.pipeline"
    default_args=("$FREEROUTING_JAVA_DIR/fixtures/Issue143-rpi_splitter.dsn" 1 router-only)
    needs_jar=1
    extra_jar_sources=("$DIFF_ROOT/java/P7T2.java")
    java_flags=("${P5T_JAVA_FLAGS[@]}")
    run_timeout="${P7T9_TIMEOUT:-3600}"
    ;;
  p7t5)
    # Plan 7 Task 11: `BatchFanout`'s component/pin ordering (BatchFanout.java:35-78, :631-693,
    # :695-778) and `RoutingBoard.fanout` (RoutingBoard.java:978-1110). Declares
    # `package app.freerouting.autoroute.pipeline` — `BatchFanout`'s constructor, its
    # `sortedComponents` field and its two nested classes are all private, so the driver reads
    # them with `setAccessible(true)` the way `P7T2` reads `reusableHandledItems`, and being in
    # the package is what lets it see the class at all. `P7T2.java` is compiled alongside because
    # `P7T5` loads its board and builds its settings through P7T2's shared statics, so the four
    # `p7t*` drivers cannot describe different boards.
    #
    # `<dsn> [passNo|maxPasses] [sortingOrder] [order|pin|pass|board]`. Mode `order` prints
    # `sortedComponents x smdPins` for **all five** `pinSortingOrder` strings (the four the
    # comparator recognises plus one it does not) and ignores the `sortingOrder` argument; mode
    # `pin` walks that order and calls the real `RoutingBoard.fanout` on every SMD pin. Plan 7
    # Task 12 adds `pass` (one whole `BatchFanout.fanoutPass`) and `board` (the whole
    # `BatchFanout.fanoutBoard`), each in a transcribed half and a **real** half whose boards are
    # compared by a `getHash()` / `structural_hash` **decision**; in mode `board` the second
    # argument is `maxPasses` and `0` keeps the settings value. Acceptance is 0 diffs on the three
    # brief-named DSNs and the six corpus stems, all four modes.
    #
    # `P7T5_HASH_MODE=warm|raw` (default `warm`) is a **Java-side** knob for modes `pass`/`board`,
    # the same one `p7t10` carries: `warm` canonicalises the by-product fields quirk #200 moves
    # before every hash, and decision parity is defined against it (controller ruling AH, and the
    # Task 3 caveat that 3 of 350 raw steps are `FANOUTSTOP false -> true`). Both sides print it.
    #
    # The per-pin `TimeLimit` is `Integer.MAX_VALUE` on both sides (ruling AI): mode `pin` passes
    # it directly, modes `pass`/`board` write it into `settings.fanout.maxMillisecondsPerPin`,
    # which is where `fanoutPass:231-232` builds its own from. The 1000 ms
    # `timeLimitToPreventEndlessLoop` inside `fanout` is a `javac`-inlined local, so the port runs
    # `RouterBudget::disabled()` against this side's live limit and a MATCH proves it never trips.
    #
    # `P7T9.java` is compiled alongside for its `dumpBoard`, which mode `board` ends with.
    #
    # The `p5t*` flag set rather than a hard-coded `-XX:hashCode=2`, so `P5T_HASH_MODE=0..4`
    # sweeps this driver too: `boardShape` reaches `DesignRulesChecker`, which iterates
    # `HashSet<Item>` over a type with no `hashCode` override (plan-5 rulings 3-4).
    javaclass=P7T5
    javapkg="autoroute.pipeline"
    default_args=("$FREEROUTING_JAVA_DIR/fixtures/Issue143-rpi_splitter.dsn" 0 outer_first order)
    needs_jar=1
    extra_jar_sources=("$DIFF_ROOT/java/P7T2.java" "$DIFF_ROOT/java/P7T9.java")
    java_flags=("${P5T_JAVA_FLAGS[@]}")
    run_timeout="${P7T5_TIMEOUT:-3600}"
    ;;
  p7t3)
    # Plan 7 Task 5: `RoutingBoard.optChangedArea` and the `TraceTightener.optChangedArea` sweep,
    # over a real DSN board whose changed area was marked by real routing. Declares `package
    # app.freerouting.autoroute.maze` — not the brief's `board.optimize` — for the reason `P7T7`
    # and `P7T10` do: it needs `P6T1.loadBoard`/`pickConnections`/`route`, which are
    # package-private statics. `P6T1.java` is compiled alongside so the two cannot describe
    # different boards.
    #
    # `<dsn> [mode] [accuracy] [routeK]`. Modes 0-2 are the three angle regimes with no vias
    # offered to the optimiser, 3 widens `pinEdgeToTurnDist` so the `ConnectionToPin` pair fires
    # hard, and **mode 4 offers vias**, which opens `TraceTightener.optChangedArea:160-165`'s
    # `ViaOptimizer` arm. Task 6 landed the entry half and Task 7 the three `repositionVia`
    # overloads, so mode 4 is 0 diffs on all three boards; before Task 7 it panicked, by
    # controller ruling B1 (a stubbed arm must be inert or loud).
    #
    # The budget is disabled on both sides by passing `timeLimit = 0` / `RouterBudget::disabled()`,
    # which is Java's own "no limit" (`TraceTightener.java:73-77`): at this entry point the limit
    # is a parameter, so there is no constant to patch by reflection.
    javaclass=P7T3
    javapkg="autoroute.maze"
    default_args=("$FREEROUTING_JAVA_DIR/fixtures/Issue143-rpi_splitter.dsn" 0 500 6)
    needs_jar=1
    extra_jar_sources=("$DIFF_ROOT/java/P6T1.java" "$DIFF_ROOT/java/probes/P7T8Probe.java")
    java_flags=(-Duser.language=en -Duser.country=US -XX:+UnlockExperimentalVMOptions -XX:hashCode=2)
    run_timeout="${P7T3_TIMEOUT:-900}"
    ;;
  p7t4)
    # Plan 7 Tasks 6 and 7: `board.optimize.ViaOptimizer`'s `optViaLocation` (:33-158),
    # `optPlaneOrFanoutVia` (:161-296), `isWithinTolerance` (:719-732) and the three
    # `repositionVia` overloads — A (:302-365), B (:367-429), C (:434-713) — over a real DSN board
    # whose vias were placed by real routing. Declares `package app.freerouting.autoroute.maze`
    # — not the brief's `board.optimize` — for the reason `P7T3` does: it needs
    # `P6T1.loadBoard`/`pickConnections`/`route`, which are package-private statics there, and it
    # pays for the five private `ViaOptimizer` methods with `setAccessible` instead (the `P6T3`
    # precedent). `P6T1.java` is compiled alongside so the two cannot describe different boards.
    #
    # `<dsn> [mode] [accuracy] [routeK]`. Mode 0 is `optViaLocation`, 1 is `optPlaneOrFanoutVia`
    # driven directly, 2 is `isWithinTolerance` over 10 256 scripted triples (no board), 3/4/5 are
    # overloads A/B/C driven directly, and 6 is mode 0 with `traceCosts = null`. **Mode 6 is
    # numbered 6, not 3**: `task-7-brief.md:29` reserved 3/4/5 for one overload each, and Task 7
    # filled them. All 21 fixture/mode pairs are 0 diffs; Task 6's `TASK7_GUARD` rows and its
    # `reachesOverloadA` replica are gone from both halves.
    #
    # The three overloads mutate nothing (`checkTraceSegment` and `DrillItemMover.check` with both
    # recursion depths at zero are read-only probes), so modes 3-5 call them many times per via and
    # still print the board the routing prologue built — which is what pins "no id was burned".
    #
    # The budget is disabled on both sides: `ViaOptimizer` reads no clock, and the `pullTight`
    # calls inside it take Java's `null` `Stoppable` / the port's never-tripping `StopCheck`.
    javaclass=P7T4
    javapkg="autoroute.maze"
    default_args=("$FREEROUTING_JAVA_DIR/fixtures/Issue143-rpi_splitter.dsn" 2 500 12)
    needs_jar=1
    extra_jar_sources=("$DIFF_ROOT/java/P6T1.java" "$DIFF_ROOT/java/probes/P7T8Probe.java")
    java_flags=(-Duser.language=en -Duser.country=US -XX:+UnlockExperimentalVMOptions -XX:hashCode=2)
    run_timeout="${P7T4_TIMEOUT:-900}"
    ;;
  p7t6)
    # Plan 7 Task 5: `PolylineTrace`'s `ConnectionToPin` trio — `check` (the regression oracle for
    # the port's own new `check_connection_to_pin`), `correct` and `swap` — plus the two skips of
    # `pullTight:841-861`. Declares `package app.freerouting.board.trace` (the brief's package) and
    # compiles against the clone's HEAD jar; the fixture is hand-built, so no DSN is involved.
    #
    # `<mode>` is one of `check`, `correct`, `swap`, `rand`, `edge`; the default runs `check`.
    javaclass=P7T6
    javapkg="board.trace"
    default_args=(check)
    needs_jar=1
    java_flags=(-Duser.language=en -Duser.country=US -XX:+UnlockExperimentalVMOptions -XX:hashCode=2)
    ;;
  p6t3)
    # Plan 6 Tasks 4 and 5: the three neighbour sorters — the any-angle base class, its comparator
    # and the doors `calculateNeighbours` builds (modes 0-5), plus `Sorted45DegreeRoomNeighbours`
    # (modes 6 and 8) and `SortedOrthogonalRoomNeighbours` (modes 7 and 9). Declares `package
    # app.freerouting.autoroute.expansion` so it can reflect into the classes' private members (the
    # sorted sets are unobservable from outside), and compiles against the clone's HEAD jar like
    # `p6t2`.
    javaclass=P6T3
    javapkg="autoroute.expansion"
    default_args=(0 42 20 1000)
    needs_jar=1
    java_flags=(-Duser.language=en -Duser.country=US -XX:+UnlockExperimentalVMOptions -XX:hashCode=2)
    ;;
  p5t2)
    # The algorithm-level lists behind that report (plan-5 ruling 14). `P5T1.java` is compiled
    # alongside it: `P5T2` loads its board through `P5T1.loadBoard`, so the two drivers cannot
    # drift apart on their input.
    javaclass=P5T2
    javapkg="drc"
    default_args=("$DRC_FIXTURES/Issue575-drc_dev-board_4_hole_clearance_violations.dsn" - - 0)
    needs_jar=1
    extra_jar_sources=("$DIFF_ROOT/java/P5T1.java")
    java_flags=("${P5T_JAVA_FLAGS[@]}")
    ;;
  *) echo "unknown driver: $driver" >&2; usage ;;
esac

if [[ $# -gt 0 ]]; then
  args=("$@")
else
  args=("${default_args[@]}")
fi

if [[ "$needs_jar_230" -eq 1 ]]; then
  FREEROUTING_JAR="$FREEROUTING_JAR_230"
fi

if [[ "$needs_jar" -eq 1 ]]; then
  JAVAC="$JAVA25_HOME/bin/javac"
  JAVABIN="$JAVA25_HOME/bin/java"
  if [[ ! -f "$FREEROUTING_JAR" ]]; then
    echo "error: freerouting jar not found at $FREEROUTING_JAR" >&2
    echo "       build it in the sibling checkout (./gradlew build), or set FREEROUTING_JAR" >&2
    exit 1
  fi
  if [[ ! -x "$JAVAC" ]]; then
    echo "error: javac not found at $JAVAC (need JDK >= 25; set JAVA25_HOME)" >&2
    exit 1
  fi
  if [[ $# -gt 0 ]]; then args=("$@"); else args=("${default_args[@]}"); fi

  # A dedicated output directory: `$OUT` holds the local `FRLogger` stand-in the
  # `geometry/planar` drivers compile against, and it would shadow the jar's real one here.
  jar_out="$BUILD/classes-$driver"
  echo "== compiling Java ($javaclass) against $FREEROUTING_JAR =="
  rm -rf "$jar_out"
  mkdir -p "$jar_out"
  "$JAVAC" -cp "$FREEROUTING_JAR" -d "$jar_out" "$DIFF_ROOT/java/$javaclass.java" \
    ${extra_jar_sources+"${extra_jar_sources[@]}"}

  j_out="$BUILD/$driver.j.out"
  r_out="$BUILD/$driver.r.out"

  echo "== building Rust twin ($driver) =="
  (cd "$DIFF_ROOT/rust" && cargo build --release --bin "$driver" --quiet)

  echo "== running ($driver ${args[*]:-}) =="
  export FREEROUTING_JAR
  # `bound` is empty for every driver but `p6t1`, so this expands to nothing and the two commands
  # are exactly what they were before.
  bound=()
  if [[ -n "$run_timeout" ]]; then
    if command -v timeout >/dev/null 2>&1; then
      bound=(timeout "$run_timeout")
    elif command -v gtimeout >/dev/null 2>&1; then
      bound=(gtimeout "$run_timeout")
    else
      echo "warning: no timeout(1) on PATH; running $driver unbounded" >&2
    fi
  fi
  ${bound+"${bound[@]}"} "$JAVABIN" ${java_flags+"${java_flags[@]}"} -Djava.awt.headless=true -cp "$jar_out:$FREEROUTING_JAR" "app.freerouting.$javapkg.$javaclass" ${args+"${args[@]}"} >"$j_out"
  ${bound+"${bound[@]}"} "$DIFF_ROOT/rust/target/release/$driver" ${args+"${args[@]}"} >"$r_out"

  echo "== diffing =="
  if diff -q "$j_out" "$r_out" >/dev/null; then
    echo "MATCH: $driver ($(wc -l <"$j_out" | tr -d " ") lines)"
    exit 0
  else
    echo "DIFF: $driver — see $j_out vs $r_out"
    diff "$j_out" "$r_out" | head -40
    exit 1
  fi
fi

if [[ "$needs_jdk25" -eq 1 ]]; then
  JAVAC="$JAVA25_HOME/bin/javac"
  JAVABIN="$JAVA25_HOME/bin/java"
fi

# Fail loudly rather than silently compiling nothing / compiling stale
# classes: both javac and the Java sources it needs must be present before
# we do anything else.
if [[ ! -d "$JAVA_DIR/geometry/planar" ]]; then
  echo "error: real Java sources not found at $JAVA_DIR/geometry/planar" >&2
  echo "       (resolved from FREEROUTING_JAVA_DIR=$FREEROUTING_JAVA_DIR)" >&2
  echo "       set FREEROUTING_JAVA_DIR to a sibling freerouting checkout," >&2
  echo "       or check one out at $ROOT/../freerouting" >&2
  exit 1
fi
if [[ ! -x "$JAVAC" ]]; then
  echo "error: javac not found at $JAVAC (need JDK >= 23; set JAVA_HOME, or JAVA25_HOME for p2t13)" >&2
  exit 1
fi

echo "== compiling Java ($javaclass) against $JAVA_DIR =="
mkdir -p "$OUT"
javac_sources=(
  "$JAVA_DIR"/geometry/planar/*.java
  "$JAVA_DIR/datastructures/Signum.java"
  "$JAVA_DIR/datastructures/BigIntAux.java"
  "$JAVA_DIR/datastructures/Stoppable.java"
)
for extra in ${extra_java_sources+"${extra_java_sources[@]}"}; do
  javac_sources+=("$extra")
done
javac_sources+=("$DIFF_ROOT/java/support/FRLogger.java" "$DIFF_ROOT/java/$javaclass.java")
"$JAVAC" -d "$OUT" "${javac_sources[@]}"

j_out="$BUILD/$driver.j.out"
r_out="$BUILD/$driver.r.out"

echo "== building Rust twin ($driver) =="
(cd "$DIFF_ROOT/rust" && cargo build --release --bin "$driver" --quiet)
RUST_BIN="$DIFF_ROOT/rust/target/release/$driver"

echo "== running ($driver ${args[*]:-}) =="
# `e15` and `p2t3` take no arguments, so the array can legitimately be empty.
"$JAVABIN" -Djava.awt.headless=true -cp "$OUT" "app.freerouting.$javapkg.$javaclass" ${args+"${args[@]}"} >"$j_out"
"$RUST_BIN" ${args+"${args[@]}"} >"$r_out"

echo "== diffing =="
if diff -q "$j_out" "$r_out" >/dev/null; then
  echo "MATCH: $driver ($(wc -l <"$j_out" | tr -d ' ') lines)"
  exit 0
else
  echo "DIFF: $driver — see $j_out vs $r_out"
  diff "$j_out" "$r_out" | head -40
  exit 1
fi
