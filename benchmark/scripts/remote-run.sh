#!/usr/bin/env bash
# Run `bench run` on a remote NixOS host and pull the results back.
#
#   scripts/remote-run.sh [HOST] [--remote-dir DIR] [--jobs N] [--poll-interval N]
#                         [--no-wait] [--attach RUN_ID] -- <bench run args…>
#
# HOST is optional if $BENCH_REMOTE_HOST is set (directly, or via .env -- see .env.example);
# a HOST given on the command line always wins over it.
#
# What it does:
#   1. Copies the locally built Java jar into binaries/ (so candidates.remote.toml can find
#      it) and records the exact ../freerouting commit it was built from into
#      binaries/freerouting-current.sha (candidates.remote.toml's java-current candidate
#      reads that file via `sha_file`, since the remote host has no ../freerouting checkout
#      of its own to resolve a sha from).
#   2. rsync the suite (code, corpus incl. gitignored dsn/ + kicad/, binaries/) to HOST:DIR.
#   3. Launches `uv run bench run --candidates-file candidates.remote.toml <args>` (via
#      $BENCH_CANDIDATES rather than --candidates-file, so a user's own --candidates-file
#      passed after `--` still wins) on HOST, *detached* (`setsid nohup` — see
#      scripts/lib/remote-env.sh's `remote_launch_detached`), inside the shared nix shell +
#      FREEROUTING_* env from scripts/lib/remote-env.sh (`nix shell nixpkgs#jdk25 nixpkgs#uv
#      nixpkgs#python312 nixpkgs#xvfb-run`). Detached means a multi-hour run survives this
#      script's own local process (or the ssh session that launched it) dying — the job
#      keeps running under nix/uv on HOST regardless. UV_PYTHON_DOWNLOADS=never makes uv use
#      the nix-provided python312 instead of trying to download its own standalone CPython
#      build, which NixOS can't run (see remote-setup.sh).
#   4. By default, polls HOST every `--poll-interval` seconds (60s) for the job's liveness
#      and results/<run-id>/meta.json's `"status"` (printing `cells done/total`, parsed via
#      grep/sed — see remote_poll), until the run completes or its process dies. `--no-wait`
#      skips this: it launches and returns immediately, printing how to `--attach` later.
#      `--attach RUN_ID` does the reverse — skip rsync-up and the launch entirely, and just
#      poll + pull results for a run-id already launched (by a previous `--no-wait`, or by a
#      prior invocation of this script whose local process/ssh session died — the remote job
#      itself is unaffected, since it's detached from both).
#   5. rsync results/<run-id>/ back into the local results/ tree, then exits with the remote
#      job's actual exit code where determinable (see results/<run-id>.remote.log.exit on
#      HOST, written by the detached wrapper once the job finishes).
#
# Requirements on HOST: nix with flakes, kicad (kicad-cli + the `kicad` wrapper that carries
# the pcbnew PYTHONPATH), rsync, GNU time (/run/current-system/sw/bin/time), and the
# ~/freerouting-bench-env/kicad-python wrapper (created by `scripts/remote-setup.sh HOST`).
# The run's `meta.json["host"]` records the actual remote hostname/arch (see bench/runner.py),
# so results from different hosts are traceable -- but note that timing is only comparable
# *within* a single host: cpu_s/wall_s reflect that host's CPU, not a portable number.
#
# This script does NOT import corpus boards on the host -- run `scripts/remote-corpus.sh
# HOST -- pcbench --clone ~/PCBench ...` (or `kicad-fixtures`) first if you need to (re)import
# PCBench/KiCad-fixture boards there; it pulls corpus/manifest.json + ground_truth.json back
# so this script's own corpus/manifest.json sync (below) picks up what it imported.
set -euo pipefail

usage() {
  cat <<'USAGE'
usage: scripts/remote-run.sh [HOST] [--remote-dir DIR] [--jobs N] [--poll-interval N]
                              [--no-wait] [--attach RUN_ID] [--print-host] -- <bench run args…>

Runs `bench run` on a remote NixOS host, detached (setsid nohup) so a dropped local process
or ssh session can't take the remote job down with it, then by default polls until it
finishes and pulls results/<run-id>/ back.

HOST may be omitted if $BENCH_REMOTE_HOST is set (directly, or via .env -- see .env.example
in this repo); a HOST given on the command line always wins over it.

  --remote-dir DIR     remote checkout directory (default: $BENCH_REMOTE_DIR, else freerouting-bench)
  --jobs N              cells to run concurrently on the host (default: 12)
  --poll-interval N     seconds between progress polls while waiting (default: 60)
  --no-wait             launch detached and return immediately, without polling
  --attach RUN_ID        skip rsync-up/launch entirely; just poll + pull an existing run-id
  --print-host           print the resolved HOST (from the argument or $BENCH_REMOTE_HOST) and exit 0
  -h, --help             print this message and exit 0

See the "Remote runs" section of README.md for the full walkthrough.
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
  echo "usage: scripts/remote-run.sh HOST [--remote-dir DIR] [--jobs N] -- <bench run args>" >&2
  echo "       (or set BENCH_REMOTE_HOST in .env -- copy .env.example to .env first)" >&2
  exit 1
fi

: "${BENCH_REMOTE_DIR:=freerouting-bench}"
REMOTE_DIR="$BENCH_REMOTE_DIR"
JOBS=12
POLL_INTERVAL=60
NO_WAIT=0
ATTACH_RUN_ID=""
while [[ $# -gt 0 && "$1" != "--" ]]; do
  case "$1" in
    --remote-dir) REMOTE_DIR="$2"; shift 2 ;;
    --jobs) JOBS="$2"; shift 2 ;;
    --poll-interval) POLL_INTERVAL="$2"; shift 2 ;;
    --no-wait) NO_WAIT=1; shift ;;
    --attach) ATTACH_RUN_ID="$2"; shift 2 ;;
    --print-host) echo "$HOST"; exit 0 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown option: $1" >&2; exit 2 ;;
  esac
done
[[ "${1:-}" == "--" ]] && shift
RUN_ARGS=("$@")

if [[ -n "$ATTACH_RUN_ID" ]]; then
  RUN_ID="$ATTACH_RUN_ID"
else
  # Extract --run-id (both `--run-id VALUE` and `--run-id=VALUE` forms) from the args -- needed
  # to pull results back; default to a timestamp and append it to RUN_ARGS so the remote run
  # actually uses the same id we're about to pull.
  RUN_ID=""
  for ((i = 0; i < ${#RUN_ARGS[@]}; i++)); do
    arg="${RUN_ARGS[$i]}"
    case "$arg" in
      --run-id=*) RUN_ID="${arg#--run-id=}" ;;
      --run-id) RUN_ID="${RUN_ARGS[$((i + 1))]:-}" ;;
    esac
  done
  if [[ -z "$RUN_ID" ]]; then
    RUN_ID="remote-$(date +%Y%m%d-%H%M%S)"
    RUN_ARGS+=(--run-id "$RUN_ID")
  fi

  poll_line=$(remote_poll "$HOST" "$REMOTE_DIR" "$RUN_ID" "results/$RUN_ID/meta.json")
  eval "$poll_line"
  if [[ "$EXISTS" == "1" || "$ALIVE" == "1" ]]; then
    echo "error: run-id '$RUN_ID' already exists on $HOST; choose a new --run-id" >&2
    echo "       To retrieve it instead: scripts/remote-run.sh $HOST --attach $RUN_ID" >&2
    exit 1
  fi

  mkdir -p binaries
  # $BENCH_REMOTE_JAVA_REPO is this suite's local ../freerouting checkout (default), used only
  # to find the locally-built jar and the commit it came from -- not to be confused with
  # anything on HOST, which has no Java repo checkout of its own (see the header comment).
  : "${BENCH_REMOTE_JAVA_REPO:=../../freerouting}"  # benchmark/ nests inside freerouting-rs
  case "$BENCH_REMOTE_JAVA_REPO" in
    /*) JAVA_REPO="$BENCH_REMOTE_JAVA_REPO" ;;
    *) JAVA_REPO="$HERE/$BENCH_REMOTE_JAVA_REPO" ;;
  esac
  JAR_LOCAL="${FREEROUTING_JAR:-$JAVA_REPO/build/libs/freerouting-current-executable.jar}"
  if [[ -f "$JAR_LOCAL" ]]; then
    cp -f "$JAR_LOCAL" binaries/freerouting-current-executable.jar
  else
    echo "warning: local jar not found at $JAR_LOCAL; java-current will be unavailable remotely" >&2
  fi
  # candidates.remote.toml's java-current candidate reads this file (sha_file) instead of
  # resolving a git sha itself, since the remote host has no ../freerouting checkout.
  if [[ -d "$JAVA_REPO/.git" ]]; then
    git -C "$JAVA_REPO" rev-parse --short=12 HEAD > binaries/freerouting-current.sha
  else
    echo "unknown" > binaries/freerouting-current.sha
    echo "warning: $JAVA_REPO is not a git checkout; recorded sha as 'unknown'" >&2
  fi

  echo ">> syncing to $HOST:$REMOTE_DIR"
  # --delete removes files under HOST:$REMOTE_DIR that no longer exist in the local source,
  # so the remote checkout doesn't accumulate stale files across repeated syncs. This is safe
  # here specifically because REMOTE_DIR is a suite-private working copy (never point it at a
  # shared or otherwise-important directory), and because rsync's default behavior for
  # --exclude'd paths is to leave them alone on the receiver rather than delete them (that
  # would require the separate --delete-excluded flag, which we do NOT pass) -- so results/
  # and reports/ from earlier remote runs survive this sync untouched; this script only ever
  # rsyncs results/<run-id> back afterwards, never up.
  # corpus/pcbench and corpus/kicad are excluded both from deletion and from being pushed:
  # they're produced ON the host by `scripts/remote-corpus.sh` (PCBench boards are a several-GB
  # checkout that never exists locally), so without this exclude a local tree that lacks them
  # would make `--delete` wipe out the host's already-imported boards on every sync.
  # corpus/manifest.json is NOT excluded -- it's small and IS meant to sync from local, which is
  # why `scripts/remote-corpus.sh` pulls it back locally after importing: run that first so this
  # sync pushes the manifest entries it produced back up alongside everything else.
  rsync -az --delete \
    --exclude .git --exclude .venv --exclude results --exclude reports \
    --exclude __pycache__ --exclude .superpowers --exclude '*.pyc' \
    --exclude corpus/pcbench --exclude corpus/kicad \
    ./ "$HOST:$REMOTE_DIR/"

  echo ">> launching on $HOST (jobs=$JOBS, run-id=$RUN_ID), detached"
  # See scripts/lib/remote-env.sh's `remote_env_prefix` for why `$(remote_env_prefix)` can be
  # spliced into this double-quoted string without escaping its own `$` -- it arrives as
  # already-rendered plain text via command substitution. The dynamic values (JOBS, and each
  # element of RUN_ARGS, which may contain spaces -- e.g. a `--boards "a board,other"` value)
  # are individually shell-quoted with `printf %q` before being spliced in, so they survive
  # the round trip to the remote shell intact; `remote_launch_detached` then `printf %q`s
  # this *entire* multi-line string once more to carry it over ssh as a single opaque
  # argument, so none of this needs re-escaping on its account.
  quoted_jobs=$(printf '%q' "$JOBS")
  quoted_run_args=$(printf '%q ' "${RUN_ARGS[@]}")
  inner_cmd="$(remote_env_prefix)
  export BENCH_CANDIDATES=\$PWD/candidates.remote.toml
  uv run bench run --jobs $quoted_jobs $quoted_run_args
'"

  PID=$(remote_launch_detached "$HOST" "$REMOTE_DIR" "$RUN_ID" "$inner_cmd")
  echo ">> launched: pid=$PID  log=$REMOTE_DIR/results/$RUN_ID.remote.log (on $HOST)"
  echo ">> pid file: $REMOTE_DIR/results/$RUN_ID.remote.pid  exit code (once finished): $REMOTE_DIR/results/$RUN_ID.remote.log.exit"
fi

if [[ "$NO_WAIT" == "1" ]]; then
  echo ">> --no-wait: not polling. resume with: scripts/remote-run.sh $HOST --attach $RUN_ID"
  exit 0
fi

echo ">> polling $HOST every ${POLL_INTERVAL}s for run-id=$RUN_ID ..."
while true; do
  poll_line=$(remote_poll "$HOST" "$REMOTE_DIR" "$RUN_ID" "results/$RUN_ID/meta.json")
  eval "$poll_line"
  if [[ "$TOTAL" -gt 0 ]]; then
    echo ">> [$(date +%H:%M:%S)] alive=$ALIVE status=$STATUS cells=$DONE/$TOTAL"
  else
    echo ">> [$(date +%H:%M:%S)] alive=$ALIVE status=$STATUS"
  fi
  if [[ "$STATUS" == "complete" || "$ALIVE" == "0" ]]; then
    break
  fi
  sleep "$POLL_INTERVAL"
done

# The detached wrapper writes results/$RUN_ID.remote.log.exit *after* meta.json's status
# flips to "complete" (that happens inside `bench run` itself, before the process exits), so
# there's a brief window where we can observe STATUS=complete before the exit file exists.
# Give it a few short extra polls to catch up before giving up on determining the exit code.
tries=0
while [[ "$EXITCODE" == "none" && $tries -lt 5 ]]; do
  sleep 2
  poll_line=$(remote_poll "$HOST" "$REMOTE_DIR" "$RUN_ID" "results/$RUN_ID/meta.json")
  eval "$poll_line"
  tries=$((tries + 1))
done

echo ">> pulling results/$RUN_ID"
mkdir -p results
rsync -az "$HOST:$REMOTE_DIR/results/$RUN_ID/" "results/$RUN_ID/"
echo ">> done: results/$RUN_ID"

if [[ "$EXITCODE" =~ ^-?[0-9]+$ ]]; then
  [[ "$EXITCODE" == "0" ]] || echo "warning: remote job exited with status $EXITCODE" \
       "(see $REMOTE_DIR/results/$RUN_ID.remote.log on $HOST for its output)" >&2
  exit "$EXITCODE"
else
  echo "warning: could not determine the remote job's exit status" \
       "($REMOTE_DIR/results/$RUN_ID.remote.log.exit missing on $HOST;" \
       "meta.json status=$STATUS, pid alive=$ALIVE) -- exiting nonzero to flag this" >&2
  exit 1
fi
