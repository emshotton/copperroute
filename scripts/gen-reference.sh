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

# --- Reference driver: read DSN with the real reader, write DSN + SES without routing ---
# The -de/-do job path cannot write DSN output headlessly and writes nothing when all
# routing stages are disabled, so we link a tiny driver against the jar instead.
DRIVER_SRC="$ROOT/scripts/gen-reference/RefWriter.java"
DRIVER_OUT="$ROOT/scripts/gen-reference/build"   # gitignored
JAVAC_BIN="${JAVAC:-$(dirname "$JAVA_BIN")/javac}"
mkdir -p "$DRIVER_OUT"
"$JAVAC_BIN" -cp "$JAR" -d "$DRIVER_OUT" "$DRIVER_SRC"

while IFS='|' read -r stem src || [[ -n "$stem" ]]; do
  [[ -z "$stem" || "$stem" == \#* ]] && continue
  in="$JAVA_DIR/$src"
  out="$REF/$stem"
  mkdir -p "$out"
  echo "== $stem"
  if "$JAVA_BIN" -Djava.awt.headless=true -cp "$JAR:$DRIVER_OUT" RefWriter \
       "$in" "$out/roundtrip.dsn" "$out/unrouted.ses" > "$out/java.log" 2>&1; then
    echo "   roundtrip.dsn $(wc -c < "$out/roundtrip.dsn") bytes, unrouted.ses $(wc -c < "$out/unrouted.ses") bytes, wires in ses: $(grep -c '(wire' "$out/unrouted.ses" || true)"
  else
    echo "   driver failed for $stem; see $out/java.log" >&2
  fi
done < "$REF/fixtures.txt"

echo "done. References in $REF"
