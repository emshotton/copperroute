#!/usr/bin/env bash
# Run the `p3t15` differential (all five modes) over the whole Java fixture corpus and print a
# per-fixture MATCH/DIFF table.
#
# Compiles both sides once through `run.sh p3t15` (which also smoke-runs the pair), then loops
# the built artifacts over every `.dsn` in `$FREEROUTING_JAVA_DIR/fixtures` plus the tutorial
# board, so the JVM and the Rust binary are launched per (fixture, mode) but nothing is rebuilt.
#
# Usage: sweep-p3t15.sh [mode ...]      # default: 0 1 2 3 4
# Env:   FREEROUTING_JAVA_DIR, JAVA25_HOME, FREEROUTING_JAR_230 (all as in run.sh)
#        SWEEP_OUT   directory for the raw outputs of a differing pair (default: a temp dir)
#
# Exit status is 0 when every pair matches except the documented expected diffs below, 1
# otherwise. See README.md's "Known, expected diffs" table.
set -uo pipefail

DIFF_ROOT="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$DIFF_ROOT/../.." && pwd)"
FREEROUTING_JAVA_DIR="${FREEROUTING_JAVA_DIR:-$ROOT/../freerouting}"
JAVA25_HOME="${JAVA25_HOME:-/opt/homebrew/opt/openjdk@25/libexec/openjdk.jdk/Contents/Home}"
FREEROUTING_JAR_230="${FREEROUTING_JAR_230:-$ROOT/tools/freerouting-2.3.0.jar}"
JAVABIN="$JAVA25_HOME/bin/java"
CLASSES="$DIFF_ROOT/build/classes-p3t15"
RUST_BIN="$DIFF_ROOT/rust/target/release/p3t15"
SWEEP_OUT="${SWEEP_OUT:-$(mktemp -d "${TMPDIR:-/tmp}/p3t15-sweep.XXXXXX")}"
mkdir -p "$SWEEP_OUT"

# `<fixture>:<mode>` pairs known to differ for a reason that is NOT a port bug (`*` = every
# mode). Any other diff is a port bug until proven a Java-side difference. See README.md's
# "Known, expected diffs".
#
#  * Issue229-display-8-digit-hc595.dsn — Plan 3 controller ruling E. The 2.3.0 jar's
#    `DsnFile.readStringScope` has no resync loop where the clone's HEAD does, and the port
#    follows HEAD (the plan's Java source authority), so the two readers legitimately build
#    different boards from this file. Mode 4 (the raw token stream) still MATCHes, which is the
#    evidence that the divergence is in the parser and not in the scanner.
#  * empty_board.dsn:3 — Java throws. The file has no `(library …)` scope at all, so
#    `BoardLibrary.padstacks` stays `null` and `RulesWriter.writeRules` NPEs on
#    `padstacks.count()`. The port's field is a value, not a reference, so it writes a complete
#    `.rules` file. Recorded in `docs/java-quirks.md`'s totalization table.
declare -a EXPECTED_DIFFS=(
  "Issue229-display-8-digit-hc595.dsn:*"
  "empty_board.dsn:3"
)

modes=("$@")
[[ ${#modes[@]} -gt 0 ]] || modes=(0 1 2 3 4)

echo "== compiling both sides (run.sh p3t15) =="
"$DIFF_ROOT/run.sh" p3t15 >/dev/null || { echo "smoke run failed" >&2; exit 1; }

mapfile -t fixtures < <(find "$FREEROUTING_JAVA_DIR/fixtures" -maxdepth 1 -name '*.dsn' | sort)
fixtures+=("$FREEROUTING_JAVA_DIR/examples/tutorial_board/tutorial_board.dsn")

printf '%-46s %s\n' "fixture" "$(printf 'mode%-2s ' "${modes[@]}")"
fail=0
expected=0
total=0
for f in "${fixtures[@]}"; do
  name="$(basename "$f")"
  row=""
  for m in "${modes[@]}"; do
    is_expected=0
    for e in "${EXPECTED_DIFFS[@]}"; do
      [[ "$e" == "$name:$m" || "$e" == "$name:*" ]] && is_expected=1
    done
    total=$((total + 1))
    j="$SWEEP_OUT/$name.$m.j"
    r="$SWEEP_OUT/$name.$m.r"
    "$JAVABIN" -Djava.awt.headless=true -cp "$CLASSES:$FREEROUTING_JAR_230" \
      app.freerouting.io.specctra.P3T15 "$f" "$m" >"$j" 2>/dev/null
    "$RUST_BIN" "$f" "$m" >"$r" 2>/dev/null
    if diff -q "$j" "$r" >/dev/null 2>&1; then
      row+="  MATCH"
      rm -f "$j" "$r"
    elif [[ "$is_expected" -eq 1 ]]; then
      row+="  XDIFF"
      expected=$((expected + 1))
    else
      row+="   DIFF"
      fail=$((fail + 1))
    fi
  done
  printf '%-46s %s\n' "$name" "$row"
done

echo
echo "fixtures: ${#fixtures[@]}   pairs: $total   unexpected diffs: $fail   expected diffs (XDIFF): $expected"
if [[ "$fail" -gt 0 ]]; then
  echo "raw outputs of the differing pairs are in $SWEEP_OUT"
  exit 1
fi
[[ "$expected" -gt 0 ]] && echo "XDIFF outputs kept in $SWEEP_OUT"
exit 0
