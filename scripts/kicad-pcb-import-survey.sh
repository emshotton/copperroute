#!/usr/bin/env bash
# Report which corpus boards the native KiCad reader (`copperroute info`) accepts.
set -euo pipefail
shopt -s nullglob

root="${1:?usage: kicad-pcb-import-survey.sh <corpus root>}"
[[ -d "$root" ]] || { echo "error: $root is not a directory" >&2; exit 1; }

cargo build --release -p copperroute

boards=("$root"/*/stripped.kicad_pcb)
if [[ ${#boards[@]} -eq 0 ]]; then
    echo "no boards found under $root (expected <cell>/stripped.kicad_pcb)" >&2
    exit 0
fi

ok=0
fail=0
for board in "${boards[@]}"; do
    stem=$(basename "$(dirname "$board")")
    if ./target/release/copperroute info "$board" >/dev/null 2>&1; then
        printf 'ok    %s\n' "$stem"
        ok=$((ok + 1))
    else
        printf 'fail  %s: %s\n' "$stem" \
            "$(./target/release/copperroute info "$board" 2>&1 | tail -1)"
        fail=$((fail + 1))
    fi
done

total=$((ok + fail))
printf '\n%d/%d boards accepted (%d failed)\n' "$ok" "$total" "$fail"
