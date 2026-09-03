#!/usr/bin/env bash
# Import corpus boards (PCBench, or the Java repo's KiCad fixtures) on a remote NixOS host
# and pull the manifest + ground truth back.
#
#   scripts/remote-corpus.sh [HOST] [--remote-dir DIR] [--poll-interval N] [--no-wait] \
#     -- <bench corpus args…>
#
# HOST is optional if $BENCH_REMOTE_HOST is set (directly, or via .env -- see .env.example);
# a HOST given on the command line always wins over it.
#
# e.g. (replace $BENCH_REMOTE_HOST with a literal host if you'd rather pass it explicitly)
#   scripts/remote-corpus.sh $BENCH_REMOTE_HOST -- pcbench --clone ~/PCBench --jobs 12
#   scripts/remote-corpus.sh $BENCH_REMOTE_HOST -- revalidate --origin pcbench
#   scripts/remote-corpus.sh $BENCH_REMOTE_HOST -- revalidate --origin pcbench --regenerate-projects --rerun-drc --jobs 8
#
# `revalidate` (recompute drv_routing/drv_all/unconnected/reference_complete -- and, with
# --regenerate-projects/--rerun-drc, the generated project and/or DRC report -- from what's
# already on disk; see `bench corpus revalidate --help`) works here unmodified: it's just
# another `bench corpus` subcommand, and this script forwards <bench corpus args…> generically
# rather than special-casing any of them. Bare `revalidate` (no --rerun-drc) is pure Python --
# no KiCad, no display -- so it's safe to run against an already-imported corpus even while a
# routing run is in progress on the same host; `--rerun-drc` shells out to `kicad-cli`, which
# (like the DRC step of `pcbench`/`kicad-fixtures` import) runs fine under this script's
# existing `xvfb-run` wrapper (scripts/lib/remote-env.sh) without needing any script changes.
#
# What it does:
#   1. rsync the suite (code, corpus incl. gitignored dsn/ + kicad/) up to HOST:DIR, using
#      the same excludes as scripts/remote-run.sh -- notably corpus/pcbench and corpus/kicad
#      are excluded both ways, since they're produced ON the host by this very script (a
#      PCBench checkout is several GB and never exists in this repo's local dev environment).
#   2. Launches `uv run bench corpus <args>` on HOST, *detached* (`setsid nohup` -- see
#      scripts/lib/remote-env.sh's `remote_launch_detached`), inside the shared nix shell +
#      FREEROUTING_* env from scripts/lib/remote-env.sh (BENCH_CANDIDATES deliberately left
#      unset -- `bench corpus` doesn't read a candidates file). Detached means a long import
#      survives this script's own local process (or the ssh session that launched it) dying.
#      There's no run-id for `bench corpus` the way there is for `bench run`, so the job gets
#      a log/pid name derived from the subcommand + a local timestamp instead, e.g.
#      `pcbench-20260101-120000` -- see results/<that name>.remote.{log,pid,log.exit} on HOST.
#   3. By default, polls HOST every `--poll-interval` seconds (60s) for the job's liveness
#      until it finishes (there's no meta.json to check a "status" against here -- only
#      `bench run` writes one -- so completion is PID-liveness only). `--no-wait` skips this:
#      it launches and returns immediately, printing the log/pid paths to check manually.
#   4. rsync `corpus/manifest.json` and every `corpus/pcbench/*/ground_truth.json` back into
#      the local corpus/ tree, so local `bench compare` (and `scripts/remote-run.sh`'s own
#      up-sync of corpus/manifest.json) see the boards this import produced. The boards'
#      actual KiCad/DSN files stay on the host -- `bench run`/`bench referee`'s kicad
#      referee work happens there too, via `scripts/remote-run.sh`.
#   5. Exits with the remote job's actual exit code where determinable (see
#      results/<job>.remote.log.exit on HOST, written by the detached wrapper once it
#      finishes).
#
# `--clone ~/PCBench` clones PCBench into that path on HOST if it isn't already there (see
# `bench corpus pcbench`'s own --clone handling) -- on the reference host it's kept pre-cloned
# at ~/PCBench so repeat imports don't re-clone. IMPORTANT: quote the tilde, e.g.
# `-- pcbench --clone '~/PCBench' ...` -- an unquoted `~/PCBench` is expanded by *your own*
# local shell before this script ever sees it, against your local $HOME, not HOST's. Quoted,
# it survives as literal text all the way to the `bench corpus pcbench` process running on
# HOST, which expands it there (see `root.expanduser()` in bench/cli.py's corpus_pcbench_cmd).
#
# Run this BEFORE scripts/remote-run.sh so its corpus/manifest.json up-sync (which pushes
# from local) carries the manifest entries this script just pulled back.
#
# Requirements on HOST: same as scripts/remote-run.sh, plus nixpkgs#xvfb-run available (see
# scripts/remote-setup.sh) -- PCBench/kicad-fixtures imports run KiCad's wx-based scripts
# headless.
set -euo pipefail

usage() {
  cat <<'USAGE'
usage: scripts/remote-corpus.sh [HOST] [--remote-dir DIR] [--poll-interval N] [--no-wait] \
                                 [--print-host] -- <bench corpus args…>

Runs `bench corpus <args>` on a remote NixOS host, detached (setsid nohup) so a dropped
local process or ssh session can't take the remote job down with it, then by default polls
until it finishes and pulls corpus/manifest.json + corpus/pcbench/*/ground_truth.json back.

HOST may be omitted if $BENCH_REMOTE_HOST is set (directly, or via .env -- see .env.example
in this repo); a HOST given on the command line always wins over it.

  --remote-dir DIR     remote checkout directory (default: $BENCH_REMOTE_DIR, else freerouting-bench)
  --poll-interval N     seconds between progress polls while waiting (default: 60)
  --no-wait             launch detached and return immediately, without polling
  --print-host           print the resolved HOST (from the argument or $BENCH_REMOTE_HOST) and exit 0
  -h, --help             print this message and exit 0

See the "Running on a remote NixOS host" section of README.md for the full walkthrough.
USAGE
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi

HERE="$(cd "$(dirname "$0")/.." && pwd)"
cd "$HERE"
source "$HERE/scripts/lib/remote-env.sh"
# Load .env from the repo root if present -- see .env.example. A variable already set in the
# real environment always wins (see load_dotenv in scripts/lib/remote-env.sh), same as
# bench/paths.py's own loader.
load_dotenv "$HERE/.env"

if [[ $# -gt 0 && "$1" != -* ]]; then
  HOST="$1"; shift
else
  HOST="${BENCH_REMOTE_HOST:-}"
fi
if [[ -z "$HOST" ]]; then
  echo "error: no remote host given and \$BENCH_REMOTE_HOST is not set." >&2
  echo "usage: scripts/remote-corpus.sh HOST [--remote-dir DIR] -- <bench corpus args>" >&2
  echo "       (or set BENCH_REMOTE_HOST in .env -- copy .env.example to .env first)" >&2
  exit 1
fi

: "${BENCH_REMOTE_DIR:=freerouting-bench}"
REMOTE_DIR="$BENCH_REMOTE_DIR"
POLL_INTERVAL=60
NO_WAIT=0
while [[ $# -gt 0 && "$1" != "--" ]]; do
  case "$1" in
    --remote-dir) REMOTE_DIR="$2"; shift 2 ;;
    --poll-interval) POLL_INTERVAL="$2"; shift 2 ;;
    --no-wait) NO_WAIT=1; shift ;;
    --print-host) echo "$HOST"; exit 0 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown option: $1" >&2; exit 2 ;;
  esac
done
[[ "${1:-}" == "--" ]] && shift
CORPUS_ARGS=("$@")
if [[ ${#CORPUS_ARGS[@]} -eq 0 ]]; then
  usage >&2
  exit 2
fi

# No run-id for `bench corpus` -- name this job's log/pid/exit files after the subcommand
# plus a local timestamp instead, so repeated imports (or different subcommands) don't
# collide.
SUBCOMMAND="${CORPUS_ARGS[0]}"
JOB_NAME="${SUBCOMMAND}-$(date +%Y%m%d-%H%M%S)"

echo ">> syncing to $HOST:$REMOTE_DIR"
# See scripts/remote-run.sh for the --delete/exclude rationale in full; corpus/pcbench and
# corpus/kicad are excluded here too so this up-sync can't delete boards this script (or an
# earlier run of it) already imported on the host.
rsync -az --delete \
  --exclude .git --exclude .venv --exclude results --exclude reports \
  --exclude __pycache__ --exclude .superpowers --exclude '*.pyc' \
  --exclude corpus/pcbench --exclude corpus/kicad \
  ./ "$HOST:$REMOTE_DIR/"

echo ">> launching on $HOST: bench corpus ${CORPUS_ARGS[*]} (detached, job=$JOB_NAME)"
# See scripts/lib/remote-env.sh for why `$(remote_env_prefix)` can be spliced into this
# double-quoted string without escaping its own `$` -- it arrives as already-rendered plain
# text via command substitution. BENCH_CANDIDATES is deliberately NOT exported here (unlike
# remote-run.sh): `bench corpus` doesn't read a candidates file.
quoted_corpus_args=$(printf '%q ' "${CORPUS_ARGS[@]}")
inner_cmd="$(remote_env_prefix)
  uv run bench corpus $quoted_corpus_args
'"

PID=$(remote_launch_detached "$HOST" "$REMOTE_DIR" "$JOB_NAME" "$inner_cmd")
echo ">> launched: pid=$PID  log=$REMOTE_DIR/results/$JOB_NAME.remote.log (on $HOST)"
echo ">> pid file: $REMOTE_DIR/results/$JOB_NAME.remote.pid  exit code (once finished): $REMOTE_DIR/results/$JOB_NAME.remote.log.exit"

if [[ "$NO_WAIT" == "1" ]]; then
  echo ">> --no-wait: not polling. check progress later with:"
  echo "     ssh $HOST tail -f $REMOTE_DIR/results/$JOB_NAME.remote.log"
  echo "   (this script has no --attach; corpus imports have no run-id to attach to -- just"
  echo "    re-run 'ssh $HOST tail -f ...' or wait, then re-run this script once it's done,"
  echo "    which is safe/resumable via --skip-existing as before)"
  exit 0
fi

echo ">> polling $HOST every ${POLL_INTERVAL}s for job=$JOB_NAME ..."
while true; do
  poll_line=$(remote_poll "$HOST" "$REMOTE_DIR" "$JOB_NAME" "")
  eval "$poll_line"
  echo ">> [$(date +%H:%M:%S)] alive=$ALIVE"
  if [[ "$ALIVE" == "0" ]]; then
    break
  fi
  sleep "$POLL_INTERVAL"
done

# The detached wrapper writes results/$JOB_NAME.remote.log.exit right after the process
# exits, essentially simultaneously with it no longer being kill -0-alive -- but give it a
# couple of short extra polls in case of a race, same as scripts/remote-run.sh.
tries=0
while [[ "$EXITCODE" == "none" && $tries -lt 5 ]]; do
  sleep 2
  poll_line=$(remote_poll "$HOST" "$REMOTE_DIR" "$JOB_NAME" "")
  eval "$poll_line"
  tries=$((tries + 1))
done

echo ">> pulling corpus/manifest.json and ground_truth.json files from $HOST"
mkdir -p corpus/pcbench
rsync -az "$HOST:$REMOTE_DIR/corpus/manifest.json" corpus/manifest.json
rsync -az --include='*/' --include='ground_truth.json' --exclude='*' \
  "$HOST:$REMOTE_DIR/corpus/pcbench/" corpus/pcbench/
echo ">> done: corpus/manifest.json + corpus/pcbench/*/ground_truth.json"

if [[ "$EXITCODE" =~ ^-?[0-9]+$ ]]; then
  [[ "$EXITCODE" == "0" ]] || echo "warning: remote job exited with status $EXITCODE" \
       "(see $REMOTE_DIR/results/$JOB_NAME.remote.log on $HOST for its output)" >&2
  exit "$EXITCODE"
else
  echo "warning: could not determine the remote job's exit status" \
       "($REMOTE_DIR/results/$JOB_NAME.remote.log.exit missing on $HOST)" \
       "-- check $REMOTE_DIR/results/$JOB_NAME.remote.log manually" >&2
  exit 1
fi
