#!/usr/bin/env bash
# Generalised Java-vs-Rust public-API audit.
#
# For every Java class file under
# `$FREEROUTING_JAVA_DIR/src/main/java/app/freerouting/<java-subpath>/`, lists its public
# methods and checks that the Rust crate at `<rust-src-dir>` has a matching `fn` (snake_case),
# or a `not ported: <Method>` / `renamed: <Method>` marker comment somewhere in that crate.
#
# Usage: audit-port.sh <java-subpath-under-app/freerouting> <rust-src-dir> [file-glob]
#   <java-subpath>  e.g. `geometry/planar` or `board/model/structure`
#   <rust-src-dir>  path to the crate's `src/` dir, relative to the repo root (or absolute)
#   [file-glob]     optional space-separated list of filenames (globs allowed) to restrict the
#                   audit to, e.g. 'Layer.java LayerStructure.java'; defaults to `*.java`
#                   (every file in the Java subpath, non-recursive, matching Plan 1's script).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
FREEROUTING_JAVA_DIR="${FREEROUTING_JAVA_DIR:-$ROOT/../freerouting}"

if [[ $# -lt 2 ]]; then
  echo "usage: $0 <java-subpath-under-app/freerouting> <rust-src-dir> [file-glob]" >&2
  exit 1
fi

JAVA_SUBPATH="$1"
RS="$2"
[[ "$RS" = /* ]] || RS="$ROOT/$RS"
FILE_GLOB="${3:-*.java}"

JAVA="$FREEROUTING_JAVA_DIR/src/main/java/app/freerouting/$JAVA_SUBPATH"

# Fail loudly rather than silently auditing zero files (a missing/misnamed
# sibling checkout would otherwise make the loop below iterate zero times and
# exit 0 having checked nothing).
if [[ ! -d "$JAVA" ]]; then
  echo "error: real Java sources not found at $JAVA" >&2
  echo "       (resolved from FREEROUTING_JAVA_DIR=$FREEROUTING_JAVA_DIR)" >&2
  echo "       set FREEROUTING_JAVA_DIR to a sibling freerouting checkout," >&2
  echo "       or check one out at $ROOT/../freerouting" >&2
  exit 1
fi

if [[ ! -d "$RS" ]]; then
  echo "error: Rust crate src dir not found at $RS" >&2
  exit 1
fi

missing=0
seen_any=0
for pattern in $FILE_GLOB; do
  for f in "$JAVA"/$pattern; do
    [[ -f "$f" ]] || continue
    seen_any=1
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
done

if [[ "$seen_any" -eq 0 ]]; then
  echo "error: file glob '$FILE_GLOB' matched no files under $JAVA" >&2
  exit 1
fi

exit $missing
