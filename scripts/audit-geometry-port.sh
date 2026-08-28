#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
JAVA="${FREEROUTING_JAVA_DIR:-$ROOT/../freerouting}/src/main/java/app/freerouting/geometry/planar"
RS="$ROOT/crates/fr-geometry/src"
missing=0
for f in "$JAVA"/*.java; do
  cls="$(basename "$f" .java)"
  # NOTE: this loop reads from a process substitution, not a pipe, so that
  # `missing=1` below is visible to the `exit $missing` after the loop — a
  # `| while read` here would run the body in a subshell and silently lose
  # every hit, making the script vacuously exit 0.
  while read -r m; do
    [[ "$m" == "$cls" ]] && continue   # constructors
    snake="$(echo "$m" | sed -E 's/([a-z0-9])([A-Z])/\1_\2/g; s/([A-Z])([A-Z][a-z])/\1_\2/g' | tr 'A-Z' 'a-z')"
    if ! grep -rqE "fn ${snake}(_[a-z0-9_]+)?\s*[<(]" "$RS" && ! grep -rqE "not ported: .*\b${m}\b" "$RS"; then
      echo "MISSING $cls.$m  (expected fn ${snake}*)"
      missing=1
    fi
  done < <(grep -hoE '^\s*public [^=(]*\b([a-zA-Z0-9_]+)\s*\(' "$f" | sed -E 's/.*[^a-zA-Z0-9_]([a-zA-Z0-9_]+)[[:space:]]*\($/\1/' | sort -u)
done
exit $missing
