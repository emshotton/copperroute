#!/usr/bin/env bash
# Run the expensive test gates on the workbench instead of the laptop.
#
#   scripts/remote-gates.sh [--slot NAME] [--ref GITREF] -- <gate...>
#
# Gates: slow-lane | nextest | identity
# What it does: pushes GITREF (default HEAD) to the workbench checkout slot ~/gate-slots/NAME
# (default "main"), checks it out there, builds release with RUSTC_WRAPPER="" under
# `nix shell nixpkgs#gcc`, links the shared benchmark corpus, runs the named gates sequentially,
# and prints each gate's tail + exit code. Slots isolate concurrent agents.
set -euo pipefail
HOST="${BENCH_REMOTE_HOST:-em@workbench}"
GATE_FAILED=0
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
ssh "$HOST" "cd $DIR && git checkout -qf $SHA && ln -sfn ~/freerouting-bench/corpus/pcbench benchmark/corpus/pcbench 2>/dev/null; RUSTC_WRAPPER='' nix shell nixpkgs#gcc nixpkgs#cargo-nextest nixpkgs#python312 --command cargo build --release -q 2>&1 | tail -2"
# run gates (single ssh per gate so output/exit are attributable)
run_gate() { echo "== gate: $* =="; ssh "$HOST" "cd $DIR && RUSTC_WRAPPER='' nix shell nixpkgs#gcc nixpkgs#cargo-nextest nixpkgs#python312 --command $1"  && echo "== exit: 0 ==" || { echo "== exit: FAILED ($?) =="; GATE_FAILED=1; }; }
i=1; args=("$@")
while [[ $i -le ${#args[@]} ]]; do
  g="${args[$((i-1))]}"
  case "$g" in
    slow-lane) run_gate "bash -c 'set -o pipefail; COPPERROUTE_SLOW=1 cargo nextest run --workspace --release --run-ignored all 2>&1 | tail -3'";;
    nextest)   run_gate "bash -c 'set -o pipefail; cargo nextest run --workspace 2>&1 | tail -3'";;
    identity)  run_gate "bash -c 'set -o pipefail; cargo nextest run -p copperroute -E \"test(two_runs_of_every_ci_stem_are_byte_identical)\" 2>&1 | tail -3'";;
    *) echo "unknown gate: $g" >&2; exit 2;;
  esac
  i=$((i+1))
done
[[ "$GATE_FAILED" == "0" ]] || { echo ">> one or more gates FAILED"; exit 1; }
