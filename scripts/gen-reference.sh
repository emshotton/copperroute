#!/usr/bin/env bash
# Generate Java reference outputs for parity tests.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
JAVA_DIR="${FREEROUTING_JAVA_DIR:-$ROOT/../freerouting}"
JAR_VERSION="2.3.0"
JAR="$ROOT/tools/freerouting-$JAR_VERSION.jar"
JAR_URL="https://github.com/freerouting/freerouting/releases/download/v$JAR_VERSION/freerouting-$JAR_VERSION.jar"
REF="$ROOT/tests/reference"
JAVA_BIN="${JAVA:-java}"

# --- Java version check (jar targets JDK 25) ---
ver="$("$JAVA_BIN" -version 2>&1 | head -1 | sed -E 's/.*"([0-9]+).*/\1/')"
if [[ "${ver:-0}" -lt 25 ]]; then
  echo "error: Java 25+ required (found ${ver:-none}). On macOS: brew install openjdk@25" >&2
  echo "       then: export JAVA=/opt/homebrew/opt/openjdk@25/bin/java" >&2
  exit 1
fi

mkdir -p "$ROOT/tools"
if [[ ! -f "$JAR" ]]; then
  echo "downloading $JAR_URL"
  curl -fL --retry 3 -o "$JAR" "$JAR_URL"
fi

run_java() { # args...
  "$JAVA_BIN" -jar "$JAR" -da -dl "$@"
}

# Routing is disabled with the generic settings override `--router.enabled=false`
# (GlobalSettings.java:538-559 routes `--section.field=value` through `setValue`;
# CliSettings.java:51,79-82 honours it and suppresses the implicit
# "-de plus -do means route" force-on).
#
# NOT `-mp 0`: in Java `maxPasses == 0` means *unlimited*, not "no passes"
# (GlobalSettings.java:675-686 explicitly allows 0; RouterSettings.java:933-941
# turns it into Integer.MAX_VALUE with the comment "0 means no limit").
NO_ROUTING="--router.enabled=false"

while IFS='|' read -r stem src; do
  [[ -z "$stem" || "$stem" == \#* ]] && continue
  in="$JAVA_DIR/$src"
  out="$REF/$stem"
  mkdir -p "$out"
  echo "== $stem"
  # DSN round-trip (no routing) and unrouted SES.
  run_java -de "$in" -do "$out/roundtrip.dsn" "$NO_ROUTING" > "$out/java.log" 2>&1 || {
    echo "   java failed for roundtrip.dsn; see $out/java.log" >&2; }
  run_java -de "$in" -do "$out/unrouted.ses" "$NO_ROUTING" >> "$out/java.log" 2>&1 || {
    echo "   java failed for unrouted.ses; see $out/java.log" >&2; }
done < "$REF/fixtures.txt"

echo
echo "IMPORTANT (first run on JDK 25): verify that routing really was suppressed —"
echo "  each <stem>/unrouted.ses '(routes ...)' section must contain only the wiring"
echo "  already present in the input DSN, and no newly routed traces. If it does not,"
echo "  \$NO_ROUTING is not taking effect and the references are not a valid baseline."

echo "done. References in $REF"
