#!/usr/bin/env bash
# Run the EXPENSIVE Plan 9 gates on the workbench (ruling BW) instead of the laptop.
#
#   scripts/remote-gates.sh [--slot NAME] [--ref GITREF] -- <gate...>
#
# Gates: slow-lane | quality-ab TASKID [extra args] | nextest | identity
# What it does: pushes GITREF (default HEAD) to the workbench bare-ish checkout slot
# ~/gate-slots/NAME (default "main"), checks it out there, builds release with
# RUSTC_WRAPPER="" under `nix shell nixpkgs#gcc nixpkgs#jdk25`, links the shared corpus,
# runs the named gates sequentially, and prints each gate's tail + exit code. Timing
# gates (quality-ab's cpu lane) are ONLY valid on the workbench once stem-times.tsv is
# workbench-cut — see the ledger's ruling BW; a laptop-cut baseline file makes the cpu
# lane advisory-only and this script says so loudly when the tsv's `# host:` disagrees.
# Slots isolate concurrent agents (one slot per worktree agent; landing gates use "main").
set -euo pipefail
HOST="${BENCH_REMOTE_HOST:-em@workbench}"
SLOT="main"; REF="HEAD"
while [[ $# -gt 0 && "$1" != "--" ]]; do case "$1" in
  --slot) SLOT="$2"; shift 2;; --ref) REF="$2"; shift 2;;
  *) echo "unknown option: $1" >&2; exit 2;; esac; done
[[ "${1:-}" == "--" ]] && shift
[[ $# -ge 1 ]] || { echo "no gates named" >&2; exit 2; }
SHA=$(git rev-parse "$REF")
DIR="gate-slots/$SLOT"
echo ">> pushing $SHA to $HOST:$DIR"
ssh "$HOST" "mkdir -p $DIR && cd $DIR && { git rev-parse --is-inside-work-tree >/dev/null 2>&1 || git init -q .; }"
git push -q "$HOST:$DIR" "$SHA:refs/heads/gate-run" -f
ssh "$HOST" "ln -sfn ~/freerouting gate-slots/freerouting; cd $DIR && git checkout -qf $SHA && ln -sfn ~/freerouting-bench/corpus/pcbench benchmark/corpus/pcbench 2>/dev/null; RUSTC_WRAPPER='' nix shell nixpkgs#gcc nixpkgs#jdk25 nixpkgs#cargo-nextest --command cargo build --release -q 2>&1 | tail -2"
for g in "$@"; do :; done
# run gates (single ssh per gate so output/exit are attributable)
run_gate() { echo "== gate: $* =="; ssh "$HOST" "cd $DIR && export JAVA25_HOME=\$(nix shell nixpkgs#jdk25 --command sh -c 'dirname \$(dirname \$(readlink -f \$(which java)))') && RUSTC_WRAPPER='' nix shell nixpkgs#gcc nixpkgs#jdk25 nixpkgs#cargo-nextest --command $1"  && echo "== exit: 0 ==" || echo "== exit: FAILED ($?) =="; }
i=1; args=("$@")
while [[ $i -le ${#args[@]} ]]; do
  g="${args[$((i-1))]}"
  case "$g" in
    slow-lane) run_gate "bash -c 'set -o pipefail; FR_SLOW_PARITY=1 cargo nextest run --workspace --release --run-ignored all 2>&1 | tail -3'";;
    nextest)   run_gate "bash -c 'set -o pipefail; cargo nextest run --workspace 2>&1 | tail -3'";;
    identity)  run_gate "bash -c 'set -o pipefail; cargo nextest run -p freerouting -E \"test(two_runs_of_every_ci_stem_are_byte_identical)\" 2>&1 | tail -3'";;
    quality-ab) tid="${args[$i]}"; i=$((i+1)); run_gate "bash -c 'set -o pipefail; bash benchmark/scripts/quality-ab.sh $tid 2>&1 | tail -8'";;
    *) echo "unknown gate: $g" >&2; exit 2;;
  esac
  i=$((i+1))
done
