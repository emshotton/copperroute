#!/usr/bin/env bash
# Compile and run one Java-vs-Rust differential driver pair, then diff their
# stdout. See README.md for what each driver covers, its default arguments,
# and how the harness works.
#
# ## `--against-jar` — a triage tool, never a gate (Plan 9 Task 0)
#
# Plan 9 retires byte parity with the jar as the acceptance test: once the port's answer is
# allowed to be *better* than freerouting's, a live jar diff turns red for every fix, and a
# harness that is red by design is a harness nobody reads. Survey §7.3's converted drivers —
# `p6t1`, `p7t9`, `p8t1`, `p8t2 e2e`, `p8t3 e2e` — therefore stop comparing against a running
# jar and start comparing against the **committed port golden** in `tests/reference/`.
#
# `--against-jar` is the escape hatch that keeps the old comparison available for as long as a
# jar is buildable. It is how a surprising golden churn gets triaged: "did the jar move, did the
# port move, or did the reference?" is a question a live jar answers in one command, and it costs
# nothing to keep. **It is not a gate.** No task's acceptance may cite it, no CI job may run it,
# and a `DIFF` from it is a *finding to explain*, never a failure to fix — the port is expected to
# diverge from the jar wherever Plan 9 has fixed something.
#
# **At Task 0 the flag is accepted and is a no-op**, because the conversion has not happened yet:
# every driver here still runs the jar live, so "against the jar" is what the script already does.
# The flag lands now so that the tasks that convert a driver have a name to put the old behaviour
# behind, and so that no converted driver has to grow its own private spelling of it.
#
#   scripts/differential/run.sh --against-jar p6t1 [args...]
#   scripts/differential/run.sh p6t1 --against-jar [args...]
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
  echo "usage: $0 [--against-jar] <driver> [args...]" >&2
  echo "  drivers: t14, t15, t16r, e15, d17, p2t3, p2t3r, p2t10, p2t11, p2t13, p2t15, p3t2," >&2
  echo "           p3t3, p3t15, p4t1, p5t1, p5t2, p6t1, p6t2, p6t3, p7t3, p7t4," >&2
  echo "           p7t5, p7t6, p7t7, p7t8, p7t10, p7t1, p7t2, p7t9, p8t0, p8t1probe," >&2
  echo "           p8t2probe, p8t2, p8t5, p8t1, p8t3, p8t6, p8t7" >&2
  echo "  args default to a smoke run per driver (see README.md); pass your" >&2
  echo "  own (e.g. iteration count, seed, mode) to override them entirely." >&2
  exit 1
}

# `--against-jar` may appear before the driver or anywhere in its argument list; it is stripped
# out of both so no driver ever sees it. See the header for what it means and what it does not.
AGAINST_JAR=0
argv=()
for token in "$@"; do
  if [[ "$token" == "--against-jar" ]]; then
    AGAINST_JAR=1
  else
    argv+=("$token")
  fi
done
set -- ${argv+"${argv[@]}"}

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
# Set by `p8t1` and by `p8t2 e2e`: **there is no Java class to compile**, because what those two
# drive is the jar *as a program* — `java -jar <jar> -de … -do …` against `freerouting -de … -do
# …`. A `P8T1.java` could only re-implement `parity::normalize_log` a second time in a second
# language, and two copies of a harness rule can agree with each other while both being wrong. So
# the Rust binary owns the comparison, prints its own per-stem verdict table and exits non-zero on
# any divergence; this script builds it, builds the port's own binary in release, and hands the
# verdict through. See `scripts/differential/rust/src/bin/p8t1.rs`'s header and the Task 6 report.
rust_only=0
# Set by `p3t15`: extra driver sources to compile alongside `$javaclass.java` in jar mode (it
# delegates its mode 4 to `P3T3.main`).
extra_jar_sources=()
# Set by `p8t0`: the directory `$javaclass.java` is compiled from. Every driver before Plan 8 kept
# its Java half in `java/` and its probes in `java/probes/` as *extra* sources; `p8t0`'s Java half
# **is** a probe (`P8T0Probe.java`), because what it drives is two static methods rather than a
# board, so it needs the probe directory as its primary source dir. Defaults to `java/`, which is
# what every other driver gets.
java_src_dir="$DIFF_ROOT/java"
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
    # `<dsn> [maxPasses] [mode] [optPasses|all] [optItems|all] [--fanout on|off]
    # [--optimizer on|off] [--ses <path>] [--passes <path>]`.
    # `mode` is `router-only`
    # (`fanout.enabled = false`, `runOptimizer = false`) or, from Plan 7 Task 12,
    # `router+fanout`, which turns the SMD fanout pre-pass on and disables its per-pin clock on
    # both sides through `settings.fanout.maxMillisecondsPerPin` (ruling AI). `maxPasses = 0` is
    # Java's "unlimited" (quirk #140), so bound it with `P7T9_TIMEOUT` before using it on a big
    # stem.
    #
    # Plan 7 Task 14 adds three more modes — `optimizer`, `optimizer+fanout` and
    # `optimizer-shared` — which run the router stage and then `BatchOptimizer.runBatchLoop`
    # (BatchOptimizer.java:125-272) on its board, with `runOptimizer = true` so the shape is
    # `RoutingPipeline`'s. `optPasses`/`optItems` are `settings.optimizer.maxPasses`/`maxItems`,
    # where `all` is Java's `null` (the "no limit" arm of `:167-170` and `:318-320`), and
    # `optimizer.timeoutString` is cleared so `:153-160` builds no deadline. `optimizer-shared`
    # hands the stage the router's own stop flag, i.e. the production shape, where quirk #227
    # makes every item reject; the other two hand it a fresh one on both sides, which is the
    # only way the rest of the loop is reachable. See README.md for the line format.
    # The driver prints two halves — a line-for-line transcription of `run`'s body with the
    # per-pass `PassRecord` tuple and every decision arm, and then the **real**
    # `BatchAutorouter.runBatchLoop()` on a freshly loaded board, with the two boards compared by
    # `getHash()` / `structural_hash` (an equality *decision*, never a hash value — ruling AH) —
    # then the final board in `P6T15aProbe`'s polyline format. See `P7T9.java`'s class comment
    # for why the driver calls `runBatchLoop()` rather than `RoutingPipeline.run()`.
    #
    # Plan 7 Task 15 adds mode `full` — the real `RoutingPipeline.createForHeadless(job).run()`,
    # both stages, driven directly rather than transcribed.
    #
    # Plan 7 Task 16 adds modes `batch` and `batch-router`, which are the **only** two that go
    # through the jar's real `-de <dsn> -do <ses>` flow: the board is loaded through
    # `management/HeadlessBoardManager` (controller ruling AW — its two clearance overrides mutate
    # 15 of the 16 corpus boards on load, so every other mode routes a board no CLI run produces)
    # and the settings come from the real two-merge `SettingsMerger` ladder driven by the same
    # `argv` the bare jar is given, which is what makes `gen-batch-reference.sh --verify-driver`
    # an apples-to-apples comparison. `--fanout`/`--optimizer` are the fixture table's columns 6
    # and 7; `--ses` writes the run's SES through the real `SesWriter.write` and `--passes` the
    # per-pass `PassRecord` tuples as JSON lines — `tests/reference/<stem>/batch.{ses,passes.jsonl}`.
    # `mode batch` is the whole pipeline; `mode batch-router` is the routing stage transcribed, so
    # that each completed pass prints its tuple. `scripts/differential/sweep-p7t9.sh` runs every
    # batch stem in every mode.
    #
    # `TIME_LIMIT_TO_PREVENT_ENDLESS_LOOP` is a `static final int = 1000` that `javac` inlines
    # (`sipush 1000` at every call site, no `getstatic`), so **no flag and no reflection disables
    # it**: the port runs `RouterBudget::disabled()` against a live limit and a MATCH is what
    # proves the limit never trips. To count the trips a Java run took, add
    # `-Dfreerouting.logging.file.location=<path>.log -Dfreerouting.logging.file.level=DEBUG
    # -Dfreerouting.logging.console.enabled=false` **to this driver** and grep the file for
    # `TraceTightener.is_stop_requested: time limit exceeded`. A **bare-jar** run needs the
    # program arguments `--logging.file.location=…` instead, because `Freerouting.main`
    # (`Freerouting.java:1088-1098`) overwrites those system properties before logging
    # initialises; `gen-batch-reference.sh --verify-driver` does both.
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
  p7t8)
    # Plan 7 Task 13: `BatchOptimizer`'s item half — the protected inner class
    # `ReadSortedRouteItems` (BatchOptimizer.java:563-659), `optRouteItem` (:395-514),
    # `containsOnlyUnfixedTraces` (:85-92) and
    # `BatchAutorouter.autoroutePassesForOptimizingItem` (BatchAutorouter.java:245-281).
    # Declares `package app.freerouting.autoroute.pipeline` for the reason `P7T5` does:
    # `ReadSortedRouteItems` is a **protected inner class** and `optRouteItem` is `protected`, so
    # being in the package is what lets the driver write `optimizer.new ReadSortedRouteItems()`
    # and call the method with no reflection at all. `P7T2.java` is compiled alongside for
    # `loadBoard`/`buildSettings`/`newRouter`/`boardShape` and `P7T9.java` for `dumpBoard`.
    #
    # `<dsn> [mode] [routePasses] [items|all]`. Every mode starts with a routing prologue — the
    # real `BatchAutorouter.runBatchLoop()`, i.e. the call `p7t9` already pins byte for byte — so
    # the optimizer runs on a real routed board and a prologue divergence shows up on the
    # `ROUTED` line rather than inside the optimizer. Mode `sequence` walks a fresh
    # `ReadSortedRouteItems` to exhaustion with **no** mutation between calls, which is the pure
    # ordering of `:573-654`; mode `item` is `optRoutePass`' loop (`:327-331`) with the stop
    # conditions removed — `next()`, the real `optRouteItem`, then `next()` again on the board
    # that call mutated, which is plan-7 ruling 12's pin. Beside each call the driver
    # **transcribes** the two ripped sets (`:412-432`) and the ripup costs (`:453-463`), because
    # both are locals of the real method; the `RESULT` and `BOARD` lines are the method's own
    # answer. `items` defaults to 5 and bounds the walk, because one `optRouteItem` runs up to
    # `optimizer.maxAutoroutePasses` whole autoroute passes.
    #
    # Ruling AI: the port runs `RouterBudget::disabled()` against this side's live 1000 ms
    # `TIME_LIMIT_TO_PREVENT_ENDLESS_LOOP`, which `javac` inlines and no reflection reaches, so a
    # MATCH proves the limit never trips. `BatchOptimizer.deadlineMs` is never set — `runBatchLoop`
    # is Task 14's and this driver does not call it.
    #
    # The `p5t*` flag set rather than a hard-coded `-XX:hashCode=2`, so `P5T_HASH_MODE=0..4` sweeps
    # this driver too: `boardShape` and `BoardStatistics` both reach `DesignRulesChecker`, which
    # iterates `HashSet<Item>` over a type with no `hashCode` override (plan-5 rulings 3-4).
    #
    # Plan 7 Task 14b: with `P7T8B_IDS=1` both halves decorate the id generator and print one `ID`
    # line per allocation plus an `ITEMSEP n=<n> id=<id>` separator before every `optRouteItem`.
    # That, with level 8 of `java/p6t17b-bisect.patch` (`P7T14B_MAT` / `_FP` / `_CS` / `_MAZE`),
    # is the bisect for quirk #229 — the `Issue558-dev-board` 188-vs-187 id burn. All of it is
    # stderr-only and off by default, so `run.sh p7t8` is unaffected.
    javaclass=P7T8
    javapkg="autoroute.pipeline"
    default_args=("$FREEROUTING_JAVA_DIR/fixtures/Issue143-rpi_splitter.dsn" sequence 1 5)
    needs_jar=1
    extra_jar_sources=("$DIFF_ROOT/java/P7T2.java" "$DIFF_ROOT/java/P7T9.java")
    java_flags=("${P5T_JAVA_FLAGS[@]}")
    run_timeout="${P7T8_TIMEOUT:-3600}"
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
  p8t0)
    # Plan 8 Task 0: `TextManager.parseTimespanString` (`util/TextManager.java:83-93`), the
    # grammar in `convertFromTimespanToDurationFormat` (`:101-118`), and
    # `RoutingJobSchedulerActionThread.threadAction:43-52`'s `MAX_TIMEOUT` cap — thirty inputs,
    # each printed with its `CONV`/`PARSE`/`CAPPED`/`OFFSET` columns.
    #
    # Declares `package app.freerouting.util` so it sits beside `TextManager`; both methods it
    # drives are `public static`, so unlike `P7T2Probe` the package is convention rather than an
    # access requirement. The two literals are read out of
    # `management/jobs/RoutingJobSchedulerActionThread` by reflection (they are `private static
    # final`), so the transcript records what the jar holds rather than what the plan says.
    #
    # No wall clock and no board: `-XX:hashCode=2` and the locale pair are the shared `p5t*` set,
    # carried so a sweep across hash modes leaves this driver alone rather than skipping it.
    #
    # The committed transcript is `crates/fr-core/tests/data/p8t0-timespans.txt`, which
    # `crates/fr-core/tests/timespan.rs` asserts against row by row; this driver is what
    # regenerates and re-verifies it.
    javaclass=P8T0Probe
    javapkg="util"
    java_src_dir="$DIFF_ROOT/java/probes"
    default_args=()
    needs_jar=1
    java_flags=("${P5T_JAVA_FLAGS[@]}")
    ;;
  p8t1probe)
    # Plan 8 Task 1: the job model — `RoutingJob.getFileFormat(byte[])` (`:151-227`) and
    # `getFileFormat(Path)` (`:230-247`), `changeFileExtension` (`:352-374`), `tryToSetInput`
    # (`:335-349`), `tryToSetOutputFile` (`:377-397`), `setInputFromFile`'s default-output
    # derivation (`:425-461`), and `BoardFileDetails.setFilename` (`:149-197`) /
    # `calculateCrc32` (`:75-87`). Eight tables, 154 rows.
    #
    # Declares `package app.freerouting.core` because `BoardFileDetails.filename` and
    # `directoryPath` are `protected`; `changeFileExtension` is `private` and is reached by
    # reflection, while `tryToSetInput`/`setInputFromFile` are driven through their public
    # `setInput` overloads, which is how the CLI reaches them.
    #
    # Two rows Java cannot answer are printed as `XDIFF` on BOTH sides, so the diff still has to
    # be empty: the shift-loop hang (`:181-187`, quirk #241 — the Java half runs every SNIFF row
    # on a five-second watchdog) and `changeFileExtension`'s NPE on a bare filename (`:356`,
    # quirk #242). Absolute paths are normalised to `<SCRATCH>`/`<FIXTURES>`/`<CWD>` so the
    # transcript is portable.
    #
    # NAME: `p8t1probe`, not `p8t1` — the plan reserves `p8t1` for Task 6's end-to-end SES-byte
    # gate, which is a different driver against the same jar.
    #
    # The committed transcript is `crates/fr-core/tests/data/p8t1-job-model.txt`, which
    # `crates/fr-core/tests/job.rs` asserts against row by row; this driver regenerates and
    # re-verifies it.
    javaclass=P8T1Probe
    javapkg="core"
    java_src_dir="$DIFF_ROOT/java/probes"
    default_args=("$FREEROUTING_JAVA_DIR/fixtures" "$BUILD/p8t1probe-scratch")
    needs_jar=1
    java_flags=("${P5T_JAVA_FLAGS[@]}")
    ;;
  p8t2probe)
    # Plan 8 Task 2: the **text-scraping** `BoardStatistics(byte[], FileFormat)`
    # (`core/scoring/BoardStatistics.java:436-552`), its private `countOccurrences` (`:578-586`)
    # and the Gson JSON surface `toString` (`:589-591`). Five tables, 289 lines over 92 `BS`
    # rows: the fifty DTO fields and the byte-exact `toString()` for every corpus `.dsn`, every
    # committed `.ses`, every `batch.ses`, fifty-seven synthetic edge cases and three hand-built
    # statistics.
    #
    # Declares `package app.freerouting.core.scoring` because `countOccurrences` is `private
    # static` and is reached by reflection.
    #
    # Two `XDIFF` rows carry both answers on both sides, so the diff still has to be empty:
    # `(parser (hostCad))` throws `StringIndexOutOfBoundsException` out of the Java constructor
    # (quirk #250, totalised here), and `(parser (hostCad  ))` scrapes an *empty* `hostCad` where
    # the port spells Java's `null` the same way (quirk #251).
    #
    # Row 31 (`router-dac2020-bm01/batch.ses AS DSN`) is the one row where the host scrape
    # SUCCEEDS: a HEAD-written session file's parser scope is `reduced`, so it carries no
    # `(stringQuote ")` to truncate it, and HEAD's own keyword is the camelCase one the scrape
    # looks for (quirk #248's clause (b)).
    #
    # NAME: `p8t2probe`, not `p8t2` — the plan reserves `p8t2` for Task 4's result-manifest
    # driver, which is a different driver against the same jar.
    #
    # The committed transcript is `crates/fr-core/tests/data/p8t2-byte-statistics.txt`, which
    # `crates/fr-core/tests/stats.rs` asserts against row by row; this driver regenerates and
    # re-verifies it.
    javaclass=P8T2Probe
    javapkg="core.scoring"
    java_src_dir="$DIFF_ROOT/java/probes"
    default_args=("$FREEROUTING_JAVA_DIR" "$ROOT")
    needs_jar=1
    java_flags=("${P5T_JAVA_FLAGS[@]}")
    ;;
  p8t2)
    # Plan 8 Task 4: `core.results.RoutingResultManifest` — the thirteen `@SerializedName` fields and
    # their Gson key order (`:28-65`), `FixtureInfo`/`PhaseMetrics`/`PhaseDetail` (`:68-95`),
    # `fromJob` (`:98-135`), `write` (`:138-144`), `resolveGitSha` (`:147-161`) and the private
    # `sha256Hex` (`:163-171`), plus `core.RouterJobResourceUsage`, which `fromJob:114` copies
    # whole. Six tables: `[man]` (every manifest shape the CLI can produce, printed line by line),
    # `[dur]`, `[gitsha]`, `[sha256]`, `[write]` and `[norm]`.
    #
    # Declares `package app.freerouting.core.results` so it sits beside the class it drives;
    # `sha256Hex` is `private static` and is reached by reflection.
    #
    # TASK SPLIT — say it plainly: the plan's `p8t2` is the **end-to-end** manifest gate (run the
    # jar's `-de <dsn> -do <ses> --router.result_json=<f>`, run the port's equivalent, compare the
    # two manifests byte-identically after `normalize_manifest`). The port's binary does not grow
    # `--router.result_json` until **Task 6**. Task 4 therefore lands the whole Java half, the
    # normaliser on **both** sides — pinned row for row by the `[norm]` table, which runs a
    # *live* manifest (a real clock, a real git sha, a real duration, a real absolute path)
    # through it — and a **fixed-clock unit run** of every manifest shape, which is the `[man]`
    # table. Task 6 adds the `e2e` mode. This is not a gap; it is the split the brief asked for.
    #
    # The `[gitsha]` rows spawn a child process per row on both sides, because neither a JVM nor
    # this port can modify its own environment. Two rows are `XDIFF` on both sides: Java's
    # `Duration.between` can be negative where `std::time::Instant` is monotonic, and
    # `resolveGitSha:156-159`'s system property renames onto `:148`'s environment variable.
    #
    # The committed transcript is `crates/fr-core/tests/data/p8t2-manifest-shape.txt`, which
    # `crates/fr-core/tests/manifest.rs` asserts against row by row; this driver regenerates and
    # re-verifies it.
    #
    # **Task 6 added the `e2e` mode**, which is the plan's own `p8t2`: `p8t1`'s argv plus
    # `--router.result_json=<f>`, run through both whole programs, with the two manifests compared
    # field for field after `parity::normalize_manifest`. That comparison has no Java half for the
    # same reason `p8t1` has none, so `p8t2 e2e` switches this driver to `rust_only` while
    # `p8t2 shape` stays the Java-vs-Rust pair Task 4 built.
    javaclass=P8T2
    javapkg="core.results"
    default_args=("shape" "$FREEROUTING_JAVA_DIR/fixtures" "$BUILD/p8t2-scratch")
    needs_jar=1
    java_flags=("${P5T_JAVA_FLAGS[@]}")
    # An `if`, not `&&`: a failing `[[ ]]` as the last statement of a `case` arm is a non-zero
    # exit status, which `set -e` at the top of this script would treat as a failure.
    if [[ "${1:-}" == "e2e" ]]; then rust_only=1; fi
    ;;
  p8t1)
    # Plan 8 Task 6, controller ruling AV — **the plan's headline gate**. The HEAD jar and the
    # port, run as two whole programs on the argv recorded in each
    # `tests/reference/cli-<stem>/argv.txt`, compared on three rungs: byte-identical SES (after
    # quirk #92's four `(parser …)` keyword literals are rewritten on the jar side, the same
    # normalisation `batch_parity.rs` applies), equal exit code, equal `parity::normalize_log`.
    # No tolerance: a divergence is an `XDIFF` row in `crates/freerouting/README.md` with the
    # first differing byte and a one-line root cause.
    #
    # `rust_only=1` — see the flag's own comment above for why there is no `P8T1.java`.
    #
    # Default args are the four `ci` stems; `all` runs every stem of `cli-fixtures.txt` (the four
    # slow ones take about a minute each on both sides), and a list of stem names runs those.
    #
    # The **budget** is live on both sides here, unlike every `p7t*` driver: the port's CLI runs
    # `fr_core::RouterBudget::default()` because that is what a user gets, and the jar's
    # `optChangedArea` limit is a javac-inlined constant nothing can switch off.
    # `scripts/gen-cli-reference.sh`'s header states the difference and what bounds the risk.
    rust_only=1
    default_args=()
    ;;
  p8t3)
    # Plan 8 Task 7. **Two modes**, the `p8t2` shape:
    #
    #   * `merge` (the default) — a genuine Java-vs-Rust pair. `Freerouting.initializeDrc`'s
    #     quality-score block (`Freerouting.java:342-352`, quirk #272) over the eight rows of
    #     `tests/reference/drc-fixtures.txt`: the prototype merger plus one `DsnFileSettings` and
    #     nothing else, then `board.getStatistics().getNormalizedScore(scoring)`. Three lines per
    #     stem — the seven scoring weights, the six board counters and the score in both
    #     `Float.toString` and raw IEEE bits — so a divergence names a field instead of a float.
    #     Declares `package app.freerouting.settings` so it sits beside `SettingsMerger`, and runs
    #     against the clone's HEAD jar (plan ruling 7).
    #
    #   * `e2e` — the acceptance gate: `java -jar <jar> -de <dsn> [-dr <rules>] -drc <report>`
    #     against the port on the same argv, comparing the report byte-identically after
    #     `parity::normalize_drc_json`, plus the exit code, plus `parity::normalize_log`. That
    #     comparison has no Java half for the reason `p8t1` has none — a `P8T3.java` could only
    #     re-implement the normalisers a second time — so this mode switches the driver to
    #     `rust_only`.
    #
    # The Rust half links the **binary's own** library (`crates/freerouting`), the `p8t5`
    # convention: `commands::drc::{quality_score_settings, quality_score}` are what the program
    # runs, not a second copy written for the driver.
    #
    # The `p5t*` flag set, which is **mandatory** here rather than hygienic: `-XX:hashCode=2` is
    # quirk #144 (`getAllUnconnectedItems` iterates identity-hashed `HashSet<Item>`s, so the
    # `unconnectedItems` order moves between runs without it) and `-Duser.language=en
    # -Duser.country=US` is quirk #145 (every `%.4f` in a violation description goes through the
    # default FORMAT locale). Both are the modes `tests/reference/drc-*` was generated under, and
    # `parity::run_jar` carries the identical five flags for the `e2e` lane.
    javaclass=P8T3
    javapkg="settings"
    default_args=("merge" "$ROOT/tests/reference/drc-fixtures.txt" "$FREEROUTING_JAVA_DIR")
    needs_jar=1
    java_flags=("${P5T_JAVA_FLAGS[@]}")
    # An `if`, not `&&`: a failing `[[ ]]` as the last statement of a `case` arm is a non-zero
    # exit status, which `set -e` at the top of this script would treat as a failure.
    if [[ "${1:-}" == "e2e" ]]; then rust_only=1; fi
    ;;
  p8t5)
    # Plan 8 Task 5: the legacy command line — `GlobalSettings.applyCommandLineArguments`
    # (`settings/GlobalSettings.java:521-838`) over the 86 argv shapes of
    # `matrix/p8t5-argv.tsv`. Per row: the four filename slots plus `drcReportFile`,
    # `showHelpOption`, `logging.console.level`, every field of the `@Deprecated routerSettings`
    # bridge this method can write, `drcSettings.enabled`, and every `FRLogger` line the parse
    # emitted, read back out of `FRLogger.getLogEntries()`.
    #
    # Declares `package app.freerouting.settings` so it can read the package-private slot fields,
    # and runs against the clone's HEAD jar (plan ruling 7).
    #
    # The Java half redirects `System.out`/`System.err` to a null stream on its first line, before
    # any freerouting class is loaded, because log4j's Console appender targets SYSTEM_OUT and
    # would otherwise interleave itself with the transcript. The transcript goes to the saved
    # original stream.
    #
    # The Rust half links the **binary's own** library (`crates/freerouting`), so what is compared
    # is `legacy::resolve_slots` as the program runs it, not a second copy written for the driver.
    #
    # No wall clock, no board, no filesystem: every row is a pure argv walk. The matrix
    # deliberately holds no row whose answer depends on a file existing (`GlobalSettings.java
    # :571-572`), because that would make the transcript depend on the working directory; that
    # branch is pinned by `crates/fr-settings/tests/cli_source.rs` instead.
    javaclass=P8T5
    javapkg="settings"
    default_args=("$ROOT/scripts/differential/matrix/p8t5-argv.tsv")
    needs_jar=1
    java_flags=("${P5T_JAVA_FLAGS[@]}")
    ;;
  p8t6)
    # Plan 8 Task 12, controller ruling AO — **the MCP delta table, asserted**.
    #
    # The jar's MCP server and the port's are different programs (ruling AO replaced the HTTP
    # transport with a native one), so they are not expected to agree; *where* they disagree is the
    # eleven-row table in `crates/freerouting/README.md`, and this driver turns rows 1-10 into an
    # assertion. It makes two kinds of observation: **deltas**, which must differ, and
    # **agreements** (the framing, the blank-line skip, the unknown-tool error, `isError`, the EOF
    # exit code), which must be equal. A recorded delta that has vanished is a `GONE` row; a
    # difference the table does not record is a `NEW` row. Either fails.
    #
    # `rust_only=1` — there is no `P8T6.java`, for the reason `p8t1` has no `P8T1.java`: what is
    # under test is the **jar as a program**, and a Java class could only re-implement the delta
    # table a second time in a second language. See the driver's header.
    #
    # **Scan ruling R18: a missing jar is a defect, not a `SKIP`.** The launch is job 3's, verbatim
    # — `--mcp_server.stdio=true` alone does not start the bridge (`McpServerSettings.isEnabled`
    # defaults to false) and `--api_server.enabled=true` is effectively mandatory because every
    # generated tool is an HTTP call into the REST API. The driver builds the line itself so the
    # five flags live in exactly one place; `docs/plan-8-prep/evidence/job3-summary.md` §1 is where
    # they came from.
    #
    # Two jar launches, ~8 s each: the main run with authentication off (so the generated tools
    # answer at all) and row 7's run with it at its default (so the 401 can be observed).
    #
    #   scripts/differential/run.sh p8t6            the table
    #   scripts/differential/run.sh p8t6 verbose    ...and both raw transcripts
    rust_only=1
    default_args=()
    ;;
  p8t7)
    # Plan 8 Task 12 — **the KiCad end-to-end acceptance of spec §1**, on three rungs:
    #
    #   (a) a KiCad-exported DSN -> `route` -> SES, byte-identical to the jar's, and the SES
    #       **read back by `fr_dsn::ses_reader::read`** without error — a document the port writes
    #       and cannot read would satisfy every byte comparison in the suite and still be broken;
    #   (b) Task 9's `-de board.json -do out.ses` rung: the same board as a KiCad *design* JSON,
    #       through the port's own JSON reader, against the jar on the same argv;
    #   (c) Task 10's quirk-T measurement: `-do out.json` writes the board **as loaded**, before
    #       any routing, so the file is byte-identical for `-mp 1` and `-mp 8` and carries no
    #       trace the router produced.
    #
    # `rust_only=1`, for `p8t1`'s reason. The stems are `tests/reference/cli-fixtures.txt`'s two
    # KiCad rows plus the DSN twin of the same board.
    rust_only=1
    default_args=()
    ;;
  *) echo "unknown driver: $driver" >&2; usage ;;
esac

if [[ $# -gt 0 ]]; then
  args=("$@")
else
  args=("${default_args[@]}")
fi

# **The flag is wired, not merely parsed.** It is exported into every driver's environment, so a
# task that converts a driver to port-golden comparison reads `$AGAINST_JAR` from inside the
# driver and needs no new spelling of its own — and a converted driver that forgets to read it is
# a driver that ignores an environment variable it can see, which is a smaller mistake to find
# than a flag that reached nothing.
export AGAINST_JAR
if [[ "$AGAINST_JAR" -eq 1 ]]; then
  echo "== --against-jar: this run compares the port against a LIVE jar. It is a triage tool," >&2
  echo "   never a gate (Plan 9 Task 0) — a DIFF here is a finding to explain, not a failure." >&2
  echo "   At Task 0 the flag changes nothing: every driver still runs the jar live, so this is" >&2
  echo "   already what the harness does. AGAINST_JAR=1 is exported for the converted drivers." >&2
fi

if [[ "$needs_jar_230" -eq 1 ]]; then
  FREEROUTING_JAR="$FREEROUTING_JAR_230"
fi

if [[ "$rust_only" -eq 1 ]]; then
  if [[ ! -f "$FREEROUTING_JAR" ]]; then
    echo "error: freerouting jar not found at $FREEROUTING_JAR" >&2
    echo "       build it in the sibling checkout (./gradlew build), or set FREEROUTING_JAR" >&2
    exit 1
  fi
  # The **port's own binary**, in release: a debug build routes a whole board in minutes rather
  # than seconds, and the driver runs it once per stem on both lanes.
  echo "== building the port's binary (release) =="
  (cd "$ROOT" && cargo build --release --bin freerouting --quiet)
  echo "== building Rust driver ($driver) =="
  (cd "$DIFF_ROOT/rust" && cargo build --release --bin "$driver" --quiet)
  echo "== running ($driver ${args[*]:-}) =="
  export FREEROUTING_JAR FREEROUTING_JAVA_DIR
  export FREEROUTING_BIN="$ROOT/target/release/freerouting"
  export JAVA="$JAVA25_HOME/bin/java"
  # The driver prints its own per-stem verdict table and exits non-zero on any divergence, so
  # there is nothing to diff and its exit status is the verdict.
  "$DIFF_ROOT/rust/target/release/$driver" ${args+"${args[@]}"}
  status=$?
  if [[ "$status" -eq 0 ]]; then
    echo "MATCH: $driver"
  else
    echo "DIFF: $driver — see the table above"
  fi
  exit "$status"
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
  "$JAVAC" -cp "$FREEROUTING_JAR" -d "$jar_out" "$java_src_dir/$javaclass.java" \
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
