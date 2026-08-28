#!/usr/bin/env bash
# Compile and run one Java-vs-Rust differential driver pair, then diff their
# stdout. See README.md for what each driver covers and how the harness works.
set -euo pipefail

DIFF_ROOT="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$DIFF_ROOT/../.." && pwd)"
JAVA_DIR="${FREEROUTING_JAVA_DIR:-$ROOT/../freerouting}/src/main/java/app/freerouting"
JAVA_HOME="${JAVA_HOME:-/opt/homebrew/Cellar/openjdk/23.0.2/libexec/openjdk.jdk/Contents/Home}"
JAVAC="$JAVA_HOME/bin/javac"
JAVABIN="$JAVA_HOME/bin/java"

BUILD="$DIFF_ROOT/build"
OUT="$BUILD/classes"

usage() {
  echo "usage: $0 <driver> [seed]" >&2
  echo "  drivers: t14, t15, t16r, e15, d17" >&2
  exit 1
}

driver="${1:-}"
seed="${2:-42}"
[[ -n "$driver" ]] || usage

case "$driver" in
  t14) javaclass=T14 ;;
  t15) javaclass=T15 ;;
  t16r) javaclass=T16R ;;
  e15) javaclass=E15 ;;
  d17) javaclass=D17 ;;
  *) echo "unknown driver: $driver" >&2; usage ;;
esac

if [[ ! -d "$JAVA_DIR/geometry/planar" ]]; then
  echo "error: real Java sources not found at $JAVA_DIR/geometry/planar" >&2
  echo "       set FREEROUTING_JAVA_DIR to the sibling freerouting checkout" >&2
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

echo "== running =="
case "$driver" in
  t14)
    "$JAVABIN" -cp "$OUT" "app.freerouting.geometry.planar.$javaclass" 200 >"$j_out"
    "$RUST_BIN" 200 >"$r_out"
    ;;
  t15)
    "$JAVABIN" -cp "$OUT" "app.freerouting.geometry.planar.$javaclass" 200 "$seed" >"$j_out"
    "$RUST_BIN" 200 "$seed" >"$r_out"
    ;;
  t16r)
    "$JAVABIN" -cp "$OUT" "app.freerouting.geometry.planar.$javaclass" 200 "$seed" 0 >"$j_out"
    "$RUST_BIN" 200 "$seed" 0 >"$r_out"
    ;;
  e15)
    "$JAVABIN" -cp "$OUT" "app.freerouting.geometry.planar.$javaclass" >"$j_out"
    "$RUST_BIN" >"$r_out"
    ;;
  d17)
    "$JAVABIN" -cp "$OUT" "app.freerouting.geometry.planar.$javaclass" 200 0 >"$j_out"
    "$RUST_BIN" 200 0 >"$r_out"
    ;;
esac

echo "== diffing =="
if diff -q "$j_out" "$r_out" >/dev/null; then
  echo "MATCH: $driver ($(wc -l <"$j_out" | tr -d ' ') lines)"
  exit 0
else
  echo "DIFF: $driver — see $j_out vs $r_out"
  diff "$j_out" "$r_out" | head -40
  exit 1
fi
