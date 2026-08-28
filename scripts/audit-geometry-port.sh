#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
FREEROUTING_JAVA_DIR="${FREEROUTING_JAVA_DIR:-$ROOT/../freerouting}"
JAVA="$FREEROUTING_JAVA_DIR/src/main/java/app/freerouting/geometry/planar"
RS="$ROOT/crates/fr-geometry/src"

# Fail loudly rather than silently auditing zero files (a missing/misnamed
# sibling checkout would otherwise make the `for` loop below iterate zero
# times and exit 0 having checked nothing).
if [[ ! -d "$JAVA" ]]; then
  echo "error: real Java sources not found at $JAVA" >&2
  echo "       (resolved from FREEROUTING_JAVA_DIR=$FREEROUTING_JAVA_DIR)" >&2
  echo "       set FREEROUTING_JAVA_DIR to a sibling freerouting checkout," >&2
  echo "       or check one out at $ROOT/../freerouting" >&2
  exit 1
fi

missing=0
for f in "$JAVA"/*.java; do
  cls="$(basename "$f" .java)"
  # NOTE: this loop reads from a process substitution, not a pipe, so that
  # `missing=1` below is visible to the `exit $missing` after the loop — a
  # `| while read` here would run the body in a subshell and silently lose
  # every hit, making the script vacuously exit 0.
  while read -r m; do
    [[ "$m" == "$cls" ]] && continue   # constructors
    snake="$(echo "$m" | sed -E 's/([a-zA-Z])([0-9])/\1_\2/g; s/([a-z0-9])([A-Z])/\1_\2/g; s/([A-Z])([A-Z][a-z])/\1_\2/g' | tr 'A-Z' 'a-z')"
    if ! grep -rqE "fn ${snake}(_[a-z0-9_]+)?\s*[<(]" "$RS" \
        && ! grep -rqE "not ported: .*\b${m}\b" "$RS" \
        && ! grep -rqE "renamed: .*\b${m}\b" "$RS"; then
      echo "MISSING $cls.$m  (expected fn ${snake}*)"
      missing=1
    fi
  done < <(grep -hoE '^\s*public [^=(]*\b([a-zA-Z0-9_]+)\s*\(' "$f" | sed -E 's/.*[^a-zA-Z0-9_]([a-zA-Z0-9_]+)[[:space:]]*\($/\1/' | sort -u)
done
exit $missing
