#!/usr/bin/env bash
# Route a benchmark cell from its KiCad board instead of the DSN the runner supplies.
#
# The runner copies each board's DSN to <cell>/in.dsn and invokes a candidate with that
# path. This stands in as the candidate's executable: it recovers the board id from the
# cell directory, looks the board's stripped .kicad_pcb and .kicad_pro up in the corpus
# manifest, and runs the real binary against those instead.
#
#   COPPERROUTE_NATIVE_BIN       the routing binary to exec (required)
#   COPPERROUTE_NATIVE_CORPUS    corpus root holding manifest.json (required)
#   COPPERROUTE_NATIVE_NO_PROJECT set to skip --kicad-project
set -uo pipefail

die() { printf 'route-native-kicad: %s\n' "$1" >&2; exit 64; }

[ -n "${COPPERROUTE_NATIVE_BIN:-}" ] || die "COPPERROUTE_NATIVE_BIN is not set"
[ -n "${COPPERROUTE_NATIVE_CORPUS:-}" ] || die "COPPERROUTE_NATIVE_CORPUS is not set"
manifest="$COPPERROUTE_NATIVE_CORPUS/manifest.json"
[ -f "$manifest" ] || die "no manifest at $manifest"

# <run>/<candidate>/<board id>/seed-N is the cell the runner created and chdir'd into.
board_id=$(basename "$(dirname "$PWD")")
[ -n "$board_id" ] || die "cannot read a board id from $PWD"

read -r stripped project < <(
    jq -r --arg id "$board_id" '
        .boards[] | select(.id == $id) | .kicad // {} |
        [(.stripped // ""), (.project // "")] | @tsv
    ' "$manifest"
) || die "manifest lookup failed for $board_id"

[ -n "${stripped:-}" ] || die "board $board_id has no stripped .kicad_pcb in the manifest"
board="$COPPERROUTE_NATIVE_CORPUS/$stripped"
[ -f "$board" ] || die "board $board_id: no file at $board"

# Swap the runner's in.dsn for the KiCad board, keeping every other argument as given.
args=()
for arg in "$@"; do
    case "$arg" in
        *in.dsn) args+=("$board") ;;
        *) args+=("$arg") ;;
    esac
done

if [ -z "${COPPERROUTE_NATIVE_NO_PROJECT:-}" ] && [ -n "${project:-}" ]; then
    project_path="$COPPERROUTE_NATIVE_CORPUS/$project"
    [ -f "$project_path" ] && args+=(--kicad-project "$project_path")
fi

printf '%s\n' "$COPPERROUTE_NATIVE_BIN" "${args[@]}" > native-argv.txt
exec "$COPPERROUTE_NATIVE_BIN" "${args[@]}"
