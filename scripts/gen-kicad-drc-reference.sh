#!/usr/bin/env bash
# Generate kicad-cli DRC references for crates/fr-drc/tests/kicad_oracle.rs.
#
# Per stem in tests/reference/kicad-drc-fixtures.txt: strip the routing from the KiCad board,
# import the session with the benchmark's vendored importer, refill zones, copy the project next
# to the board, run `kicad-cli pcb drc --all-track-errors --format json`, and store the report as
# tests/reference/<stem>/kicad-drc.json with a kicad-drc.meta.txt beside it.
#
# Usage: scripts/gen-kicad-drc-reference.sh [stem ...]
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
JAVA_DIR="${FREEROUTING_JAVA_DIR:-$ROOT/../freerouting}"
KICAD_CLI="${FREEROUTING_KICAD_CLI:-/Applications/KiCad/KiCad.app/Contents/MacOS/kicad-cli}"
KICAD_PY="${FREEROUTING_KICAD_PYTHON:-/Applications/KiCad/KiCad.app/Contents/Frameworks/Python.framework/Versions/Current/bin/python3}"
VENDOR="$ROOT/benchmark/vendor/kicad"
FIXTURES="$ROOT/tests/reference/kicad-drc-fixtures.txt"
WANTED=("$@")

for tool in "$KICAD_CLI" "$KICAD_PY"; do
  [[ -x "$tool" ]] || { echo "error: $tool is not executable; set FREEROUTING_KICAD_CLI / FREEROUTING_KICAD_PYTHON" >&2; exit 1; }
done

resolve() {
  local p="$1"
  if [[ "$p" == java:* ]]; then printf '%s/%s' "$JAVA_DIR" "${p#java:}"; else printf '%s/%s' "$ROOT" "$p"; fi
}

wanted() {
  [[ ${#WANTED[@]} -eq 0 ]] && return 0
  local w; for w in "${WANTED[@]}"; do [[ "$w" == "$1" ]] && return 0; done
  return 1
}

while IFS='|' read -r stem dsn ses pcb pro ignore || [[ -n "$stem" ]]; do
  [[ -z "$stem" || "$stem" == \#* ]] && continue
  wanted "$stem" || continue
  pcb_path="$(resolve "$pcb")"; ses_path="$(resolve "$ses")"; pro_path="$(resolve "$pro")"
  for f in "$pcb_path" "$ses_path" "$pro_path"; do
    [[ -f "$f" ]] || { echo "skip $stem: missing $f" >&2; continue 2; }
  done
  out_dir="$ROOT/tests/reference/$stem"
  work="$(mktemp -d)"
  mkdir -p "$out_dir"
  echo "== $stem"
  "$KICAD_PY" "$VENDOR/strip_kicad_routing.py" "$pcb_path" "$work/stripped.kicad_pcb"
  "$KICAD_PY" "$VENDOR/ses_to_board.py" "$work/stripped.kicad_pcb" "$ses_path" "$work/routed.kicad_pcb" > "$work/import.json"
  "$KICAD_PY" "$VENDOR/refill_zones.py" "$work/routed.kicad_pcb" "$work/routed.kicad_pcb" > "$work/refill.json"
  cp "$pro_path" "$work/routed.kicad_pro"
  "$KICAD_CLI" pcb drc --format json --all-track-errors --units mm -o "$work/kicad-drc.json" "$work/routed.kicad_pcb"
  cp "$work/kicad-drc.json" "$out_dir/kicad-drc.json"
  {
    echo "kicad-cli    $("$KICAD_CLI" version)"
    echo "generated    $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "dsn          $dsn"
    echo "ses          $ses"
    echo "kicad_pcb    $pcb"
    echo "kicad_pro    $pro"
    echo "import       $(cat "$work/import.json")"
    echo "refill       $(cat "$work/refill.json")"
    echo "command      kicad-cli pcb drc --format json --all-track-errors --units mm"
  } > "$out_dir/kicad-drc.meta.txt"
  rm -rf "$work"
done < "$FIXTURES"
