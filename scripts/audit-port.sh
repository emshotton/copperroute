#!/usr/bin/env bash
# Generalised Java-vs-Rust public-API audit.
#
# For every Java class file under
# `$FREEROUTING_JAVA_DIR/src/main/java/app/freerouting/<java-subpath>/`, lists its public
# methods and checks that the Rust crate at `<rust-src-dir>` has a matching `fn` (snake_case),
# or one of these marker comments naming the method somewhere in that crate:
#   `not ported: <Method>`          — deliberately dropped (GUI, serialization, logging, ...)
#   `renamed: <Method>`             — ported under a different name
#   `added in Task N: <Method>`     — deferred to a named later task of the current plan
#   `added in Plan N: <Method>`     — deferred to a named later plan
#
# Three result lines, and what each does to the exit code:
#   `MISSING <Class>.<method>`  — no `fn` and no marker.                          exit 1
#   `UNMAPPED <Class>`          — a class map was supplied and does not name       exit 1
#                                 this class, so it fell back to the weaker
#                                 crate-wide search.
#   `ROSTERED <Class>`          — every public method of the class is satisfied    exit 0
#                                 by a `not ported:` / `added in Task|Plan N:`
#                                 marker and **none** by a real `fn`. The class
#                                 is on the deferral roster, not in the port.
#                                 Informational: it makes a wholly-deferred class
#                                 visible instead of letting it pass silently,
#                                 which is what `autoroute/pipeline`'s sixteen
#                                 classes and `board/optimize/ViaOptimizer.java`
#                                 do (all rostered to Plan 7/8 in `src/lib.rs`).
# A task number may carry a single lower-case suffix letter (`Task 10b`), for a task inserted
# between two numbered ones by a controller ruling after the plan was written.
# The last two are as specific as the first two (the task/plan number is required), so they
# record an obligation rather than waive the check: `grep -rn "added in Task"` lists everything
# still owed.
#
# Usage: audit-port.sh <java-subpath-under-app/freerouting> <rust-src-dir> [file-glob] [class-map]
#   <java-subpath>  e.g. `geometry/planar` or `board/model/structure`
#   <rust-src-dir>  path to the crate's `src/` dir, relative to the repo root (or absolute)
#   [file-glob]     optional space-separated list of filenames (globs allowed) to restrict the
#                   audit to, e.g. 'Layer.java LayerStructure.java'; defaults to `*.java`
#                   (every file in the Java subpath, non-recursive, matching Plan 1's script).
#   [class-map]     optional path to a class→file map (see below). Without it, every check below
#                   is crate-wide, exactly as when this argument does not exist — this keeps the
#                   3-argument invocation byte-compatible with Plan 1/2's scripted audits.
#
# Per-class matching (Plan 3 Task 1 obligation, `docs/java-quirks.md`). The crate-wide search
# above accepts any `fn read_scope` anywhere in the crate as satisfying *every* Java class with a
# `readScope` method — worthless once a crate has many classes sharing a method name (e.g.
# `io/specctra/parser`'s ~12 `readScope`/`writeScope` classes). The optional 4th argument names a
# map file of `<JavaClass> <rust-path-glob-relative-to-src>` lines (blank lines and `#` comments
# ignored), e.g.:
#   Structure                parser/structure.rs
#   Network                  parser/network.rs
#   SpecctraDsnStreamReader  lexer/*.rs
#   IdentifierType           format/identifier.rs
# When a map is supplied, a class listed in it has its methods and markers searched *only* under
# its mapped path(s) (relative to `<rust-src-dir>`, globs allowed); a class the map does not
# mention falls back to the crate-wide search and additionally prints `UNMAPPED <Class>` (once
# per class) so the map cannot silently rot as new classes come into scope.
#
# Self-test (both invocation forms; not executed by this script):
#   3-arg, unchanged:  ./scripts/audit-port.sh datastructures crates/copper-board/src
#   4-arg, per-class:  ./scripts/audit-port.sh datastructures crates/copper-dsn/src \
#                         'IdentifierType.java IndentFileWriter.java' scripts/audit-map/copper-dsn.map
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
FREEROUTING_JAVA_DIR="${FREEROUTING_JAVA_DIR:-$ROOT/../freerouting}"

if [[ $# -lt 2 ]]; then
  echo "usage: $0 <java-subpath-under-app/freerouting> <rust-src-dir> [file-glob] [class-map]" >&2
  exit 1
fi

JAVA_SUBPATH="$1"
RS="$2"
[[ "$RS" = /* ]] || RS="$ROOT/$RS"
FILE_GLOB="${3:-*.java}"
MAP_FILE="${4:-}"

if [[ -n "$MAP_FILE" ]]; then
  [[ "$MAP_FILE" = /* ]] || MAP_FILE="$ROOT/$MAP_FILE"
  if [[ ! -f "$MAP_FILE" ]]; then
    echo "error: class map file not found at $MAP_FILE" >&2
    exit 1
  fi
fi

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

# Looks up `cls` in $MAP_FILE (if any) and fills the global SCOPE_FILES array with every mapped
# file that actually exists, expanding globs relative to $RS. Sets CLASS_MAPPED=1 iff the map
# has at least one line for `cls`. No-op (SCOPE_FILES empty, CLASS_MAPPED=0) when MAP_FILE="".
SCOPE_FILES=()
CLASS_MAPPED=0
resolve_scope_files() {
  local cls="$1"
  SCOPE_FILES=()
  CLASS_MAPPED=0
  [[ -n "$MAP_FILE" ]] || return 0
  local map_cls map_path
  while read -r map_cls map_path; do
    [[ -z "$map_cls" || "$map_cls" == \#* ]] && continue
    [[ "$map_cls" == "$cls" ]] || continue
    CLASS_MAPPED=1
    for p in "$RS"/$map_path; do
      [[ -e "$p" ]] && SCOPE_FILES+=("$p")
    done
  done < "$MAP_FILE"
}

# Classes we've already printed an UNMAPPED line for, so it prints once per class, not once per
# method.
UNMAPPED_SEEN=""

missing=0
# An `UNMAPPED` class is a rotted map — the audit silently degraded to the crate-wide search for
# it — so it fails the run as loudly as a `MISSING` method does.
unmapped=0
seen_any=0
for pattern in $FILE_GLOB; do
  for f in "$JAVA"/$pattern; do
    [[ -f "$f" ]] || continue
    seen_any=1
    cls="$(basename "$f" .java)"

    resolve_scope_files "$cls"
    if [[ -n "$MAP_FILE" && "$CLASS_MAPPED" -eq 0 ]]; then
      case " $UNMAPPED_SEEN " in
        *" $cls "*) ;;
        *)
          echo "UNMAPPED $cls"
          UNMAPPED_SEEN="$UNMAPPED_SEEN $cls"
          unmapped=1
          ;;
      esac
    fi

    # Per-class tallies for the ROSTERED line below: how many public methods the class has, and
    # how many of them a real `fn` (or a `renamed:` marker, which also means ported) answered.
    cls_methods=0
    cls_ported=0
    cls_not_ported=0
    cls_deferred=0
    cls_missing=0

    # NOTE: this loop reads from a process substitution, not a pipe, so that
    # `missing=1` below is visible to the `exit $missing` after the loop — a
    # `| while read` here would run the body in a subshell and silently lose
    # every hit, making the script vacuously exit 0.
    while read -r m; do
      [[ "$m" == "$cls" ]] && continue   # constructors
      cls_methods=$((cls_methods + 1))
      snake="$(echo "$m" | sed -E 's/([a-zA-Z])([0-9])/\1_\2/g; s/([a-z0-9])([A-Z])/\1_\2/g; s/([A-Z])([A-Z][a-z])/\1_\2/g' | tr 'A-Z' 'a-z')"
      if [[ -n "$MAP_FILE" && "$CLASS_MAPPED" -eq 1 ]]; then
        # Per-class: search only under this class's mapped file(s). An empty SCOPE_FILES (the
        # mapped path glob matched nothing yet) means every method is reported MISSING, which is
        # the intended stricter behaviour, not a fallback.
        if [[ "${#SCOPE_FILES[@]}" -eq 0 ]]; then
          kind=missing
        elif grep -qE "fn ${snake}(_[a-z0-9_]+)?\s*[<(]" "${SCOPE_FILES[@]}" \
            || grep -qE "renamed: .*\b${m}\b" "${SCOPE_FILES[@]}"; then
          kind=ported
        elif grep -qE "not ported: .*\b${m}\b" "${SCOPE_FILES[@]}"; then
          kind=not_ported
        elif grep -qE "added in (Task|Plan) [0-9]+[a-z]?:.*\b${m}\b" "${SCOPE_FILES[@]}"; then
          kind=deferred
        else
          kind=missing
        fi
      else
        # Crate-wide (no map supplied, or class not in the map): identical to the original
        # 3-argument behaviour.
        if grep -rqE "fn ${snake}(_[a-z0-9_]+)?\s*[<(]" "$RS" \
            || grep -rqE "renamed: .*\b${m}\b" "$RS"; then
          kind=ported
        elif grep -rqE "not ported: .*\b${m}\b" "$RS"; then
          kind=not_ported
        elif grep -rqE "added in (Task|Plan) [0-9]+[a-z]?:.*\b${m}\b" "$RS"; then
          kind=deferred
        else
          kind=missing
        fi
      fi
      case "$kind" in
        ported)     cls_ported=$((cls_ported + 1)) ;;
        not_ported) cls_not_ported=$((cls_not_ported + 1)) ;;
        deferred)   cls_deferred=$((cls_deferred + 1)) ;;
        missing)
          echo "MISSING $cls.$m  (expected fn ${snake}*)"
          cls_missing=$((cls_missing + 1))
          missing=1
          ;;
      esac
    done < <(grep -hoE '^\s*public [^=(]*\b([a-zA-Z0-9_]+)\s*\(' "$f" | sed -E 's/.*[^a-zA-Z0-9_]([a-zA-Z0-9_]+)[[:space:]]*\($/\1/' | sort -u)

    # A class whose every public method is answered by a marker and none by an `fn` is not
    # ported — it is rostered. Say so rather than exiting 0 in silence: without this line
    # `autoroute/pipeline` and `board/optimize/ViaOptimizer.java` are indistinguishable from a
    # fully ported package. Informational only; the exit code is unaffected.
    if [[ "$cls_methods" -gt 0 && "$cls_ported" -eq 0 && "$cls_missing" -eq 0 ]]; then
      echo "ROSTERED $cls  ($cls_methods public methods, none ported: $cls_not_ported 'not ported:', $cls_deferred 'added in Task|Plan N:')"
    fi
  done
done

if [[ "$seen_any" -eq 0 ]]; then
  echo "error: file glob '$FILE_GLOB' matched no files under $JAVA" >&2
  exit 1
fi

if [[ "$missing" -ne 0 || "$unmapped" -ne 0 ]]; then
  exit 1
fi
exit 0
