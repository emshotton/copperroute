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

# `p2t10` is the one driver that cannot be compiled from `geometry/planar` sources: it needs the
# whole board stack (BasicBoard, BoardRules, the search trees). It is compiled and run against
# the clone's own build output instead, which is class-file version 69 and therefore needs a
# JDK 25. Override either of these if your checkout differs.
JAVA25_HOME="${JAVA25_HOME:-/opt/homebrew/Cellar/openjdk@25/25.0.4.1/libexec/openjdk.jdk/Contents/Home}"
FREEROUTING_JAR="${FREEROUTING_JAR:-$FREEROUTING_JAVA_DIR/build/libs/freerouting-current-executable.jar}"

BUILD="$DIFF_ROOT/build"
OUT="$BUILD/classes"

usage() {
  echo "usage: $0 <driver> [args...]" >&2
  echo "  drivers: t14, t15, t16r, e15, d17, p2t3, p2t3r, p2t10, p2t11, p2t13, p2t15" >&2
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
  *) echo "unknown driver: $driver" >&2; usage ;;
esac

if [[ $# -gt 0 ]]; then
  args=("$@")
else
  args=("${default_args[@]}")
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
  "$JAVAC" -cp "$FREEROUTING_JAR" -d "$jar_out" "$DIFF_ROOT/java/$javaclass.java"

  j_out="$BUILD/$driver.j.out"
  r_out="$BUILD/$driver.r.out"

  echo "== building Rust twin ($driver) =="
  (cd "$DIFF_ROOT/rust" && cargo build --release --bin "$driver" --quiet)

  echo "== running ($driver ${args[*]:-}) =="
  "$JAVABIN" -cp "$jar_out:$FREEROUTING_JAR" "app.freerouting.$javapkg.$javaclass" ${args+"${args[@]}"} >"$j_out"
  "$DIFF_ROOT/rust/target/release/$driver" ${args+"${args[@]}"} >"$r_out"

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
"$JAVABIN" -cp "$OUT" "app.freerouting.$javapkg.$javaclass" ${args+"${args[@]}"} >"$j_out"
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
