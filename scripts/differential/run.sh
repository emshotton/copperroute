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

BUILD="$DIFF_ROOT/build"
OUT="$BUILD/classes"

usage() {
  echo "usage: $0 <driver> [args...]" >&2
  echo "  drivers: t14, t15, t16r, e15, d17" >&2
  echo "  args default to a smoke run per driver (see README.md); pass your" >&2
  echo "  own (e.g. iteration count, seed, mode) to override them entirely." >&2
  exit 1
}

driver="${1:-}"
[[ -n "$driver" ]] || usage
shift

case "$driver" in
  t14) javaclass=T14; default_args=(200) ;;
  t15) javaclass=T15; default_args=(200 42) ;;
  t16r) javaclass=T16R; default_args=(200 42 0) ;;
  e15) javaclass=E15; default_args=() ;;
  d17) javaclass=D17; default_args=(200 0) ;;
  *) echo "unknown driver: $driver" >&2; usage ;;
esac

if [[ $# -gt 0 ]]; then
  args=("$@")
else
  args=("${default_args[@]}")
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
  echo "error: javac not found at $JAVAC (need JDK >= 23; set JAVA_HOME)" >&2
  exit 1
fi

echo "== compiling Java ($javaclass) against $JAVA_DIR =="
mkdir -p "$OUT"
"$JAVAC" -d "$OUT" \
  "$JAVA_DIR"/geometry/planar/*.java \
  "$JAVA_DIR"/datastructures/Signum.java \
  "$JAVA_DIR"/datastructures/BigIntAux.java \
  "$JAVA_DIR"/datastructures/Stoppable.java \
  "$DIFF_ROOT/java/support/FRLogger.java" \
  "$DIFF_ROOT/java/$javaclass.java"

echo "== building Rust twin ($driver) =="
(cd "$DIFF_ROOT/rust" && cargo build --release --bin "$driver" --quiet)
RUST_BIN="$DIFF_ROOT/rust/target/release/$driver"

j_out="$BUILD/$driver.j.out"
r_out="$BUILD/$driver.r.out"

echo "== running ($driver ${args[*]:-}) =="
"$JAVABIN" -cp "$OUT" "app.freerouting.geometry.planar.$javaclass" "${args[@]}" >"$j_out"
"$RUST_BIN" "${args[@]}" >"$r_out"

echo "== diffing =="
if diff -q "$j_out" "$r_out" >/dev/null; then
  echo "MATCH: $driver ($(wc -l <"$j_out" | tr -d ' ') lines)"
  exit 0
else
  echo "DIFF: $driver — see $j_out vs $r_out"
  diff "$j_out" "$r_out" | head -40
  exit 1
fi
