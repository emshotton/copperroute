# load_dotenv FILE
#
# Loads KEY=VALUE lines from FILE (typically the repo root's .env -- see .env.example) into
# the environment, without overriding a variable already set in the real environment --
# real env beats .env, matching bench/paths.py's own `_load_dotenv` precedence exactly (a
# plain `. .env`/`source .env` would instead let .env clobber an already-exported variable,
# which is why this doesn't just do that). Blank lines and `#`-comments are ignored; a value
# may be wrapped in matching single/double quotes, which are stripped. No-op if FILE doesn't
# exist. Callers: scripts/remote-run.sh, scripts/remote-corpus.sh, scripts/remote-setup.sh,
# each via `load_dotenv "$HERE/.env"` before resolving HOST from $BENCH_REMOTE_HOST.
load_dotenv() {
  local file="$1" line key value quote
  [[ -f "$file" ]] || return 0
  while IFS= read -r line || [[ -n "$line" ]]; do
    line="${line#"${line%%[![:space:]]*}"}"
    line="${line%"${line##*[![:space:]]}"}"
    [[ -z "$line" || "$line" == \#* || "$line" != *=* ]] && continue
    key="${line%%=*}"
    value="${line#*=}"
    key="${key%"${key##*[![:space:]]}"}"
    key="${key#"${key%%[![:space:]]*}"}"
    [[ -z "$key" ]] && continue
    if [[ ${#value} -ge 2 ]]; then
      quote="${value:0:1}"
      if [[ ( "$quote" == '"' || "$quote" == "'" ) && "${value: -1}" == "$quote" ]]; then
        value="${value:1:${#value}-2}"
      fi
    fi
    if [[ -z "${!key+x}" ]]; then
      export "$key=$value"
    fi
  done < "$file"
}

# Shared nix-shell + FREEROUTING_* env preamble for scripts/remote-run.sh and
# scripts/remote-corpus.sh. Source this file, then call `remote_env_prefix` to print the
# text that opens the remote `bash -c '...'` block driving `uv run bench ...` -- everything
# up to (but not including) the actual bench command. The caller appends its own
# `uv run bench ...` invocation (plus, if it needs one, its own `export BENCH_CANDIDATES=...`)
# and the closing `'` before handing the whole thing to `ssh HOST "..."`.
#
# The printed text is captured via `$(remote_env_prefix)` inside a *double-quoted* string in
# the caller, so its `$(command -v java)`/`$PWD`/`$HOME` are NOT expanded locally -- command
# substitution captures this function's stdout as plain text and splices it in verbatim, it
# does not re-scan the result for further expansion -- and only get expanded once the whole
# command string reaches the remote shell over ssh. Don't backslash-escape the `$` in here.
#
# `nixpkgs#xvfb-run` is included, and the whole `bash -c '...'` block itself runs under one
# `xvfb-run -a` (not just relying on ~/freerouting-bench-env/kicad-python's own per-invocation
# fallback), so `bench corpus pcbench`/`kicad-fixtures` -- which spawn KiCad's wx-based
# scripts, once or twice per board -- work headless. This matters specifically under `--jobs
# N > 1`: each `kicad-python` invocation left to its own fallback (`exec xvfb-run -a "$PY"
# "$@"`, written by scripts/remote-setup.sh, triggered whenever $DISPLAY is unset) spins up
# its *own* Xvfb server, and running several of those concurrently races on `xvfb-run
# --auto-servernum`'s (non-atomic) scan for a free display number -- observed in practice as
# sporadic "Unable to access the X Display" failures under `bench corpus pcbench --jobs 6`.
# Establishing $DISPLAY once here, before any board import starts, means every subsequent
# `kicad-python` invocation (including ones running concurrently in different worker threads)
# finds $DISPLAY already set and skips its own fallback entirely, so only one Xvfb server
# ever exists for the whole `bench corpus`/`bench run` invocation -- no race. Harmless
# (and unused) for `bench run`, which doesn't need a display.
#
# Deliberately nothing here about HOME/XDG_*/APPDATA: freerouting's persisted
# freerouting.json settings-leak isolation (see README's "Settings isolation" note) is done
# per cell, in bench.runner.run_cell/bench.referee.java_drc -- each candidate/referee
# invocation gets its own HOME under its own results/ cell dir -- not via a shared override
# here. A single HOME exported for the whole remote job would defeat the point: every
# candidate and referee invocation in the job would collide on the same freerouting.json again.
#
# The nix package set and the two paths below (GNU time, the kicad-python wrapper) are host
# layout, not code -- callers control them via $BENCH_REMOTE_NIX_PACKAGES/$BENCH_REMOTE_TIME/
# $BENCH_REMOTE_KICAD_PYTHON (see .env.example), defaulting to this suite's own NixOS
# reference-host layout so a bare checkout still works there unmodified. These are read from
# *this* (local, invoking) process's environment -- typically populated from .env by the
# caller (scripts/remote-run.sh/scripts/remote-corpus.sh) -- and spliced into the text below as
# already-resolved values, not left as `$VAR` for the remote shell to expand (the remote host
# has no BENCH_REMOTE_* env of its own). `$(command -v java)`/`$PWD`/`$(command -v kicad-cli)`
# are `\$`-escaped so they stay literal here and only expand once on the remote host, same as
# before this function took any local inputs.
remote_env_prefix() {
  local nix_packages="${BENCH_REMOTE_NIX_PACKAGES:-nixpkgs#jdk25 nixpkgs#uv nixpkgs#python312 nixpkgs#xvfb-run}"
  local remote_time="${BENCH_REMOTE_TIME:-/run/current-system/sw/bin/time}"
  local remote_kicad_python="${BENCH_REMOTE_KICAD_PYTHON:-\$HOME/freerouting-bench-env/kicad-python}"
  cat <<PREFIX
nix shell $nix_packages --command xvfb-run -a bash -c '
  set -euo pipefail
  export FREEROUTING_JAVA=\$(command -v java)
  export FREEROUTING_JAR=\$PWD/binaries/freerouting-current-executable.jar
  export FREEROUTING_TIME=$remote_time
  export FREEROUTING_KICAD_CLI=\$(command -v kicad-cli || true)
  export FREEROUTING_KICAD_PYTHON=$remote_kicad_python
  export UV_PYTHON_DOWNLOADS=never
PREFIX
}

# remote_launch_detached HOST REMOTE_DIR JOB_NAME INNER_CMD
#
# Launches INNER_CMD (a complete, self-contained shell command string -- typically
# `$(remote_env_prefix)` plus a caller's own exports/`uv run bench ...` line(s) and a
# trailing closing `'`, exactly what scripts/remote-run.sh and scripts/remote-corpus.sh used
# to hand straight to `ssh HOST "..."` before this) detached on HOST under $REMOTE_DIR, so a
# dropped local process / ssh session can't take the remote job down with it, and echoes its
# PID to stdout.
#
# Conventions (relative to $REMOTE_DIR), all under results/ so a fresh results/ dir is the
# only thing that needs to exist -- callers create it via this function's own `mkdir -p`:
#   results/$JOB_NAME.remote.log        -- combined stdout+stderr of INNER_CMD
#   results/$JOB_NAME.remote.log.exit   -- INNER_CMD's exit code, written once it finishes
#   results/$JOB_NAME.remote.pid        -- the detached process's PID (same as this function's stdout)
#
# How INNER_CMD reaches the remote host without a second round of quoting hell: this
# function's own remote script (the `<<'LAUNCH' ... LAUNCH` heredoc below) is delivered over
# ssh's stdin -- untouched by any local expansion, since the heredoc delimiter is quoted --
# and JOB_NAME/INNER_CMD are passed to it as `bash -s --` *positional parameters*, each
# `printf %q`-encoded locally so it survives the trip as a single opaque argument no matter
# what quotes/newlines/`$`s it contains (INNER_CMD, built from `remote_env_prefix`, is
# exactly this: multiple lines with embedded single quotes). On the remote side the launch
# script never textually substitutes INNER_CMD into new source -- it hands it to a second
# `bash -c '...' bash "$inner_cmd" "$exit_file"` as *that* script's own $1/$2 and `eval`s $1,
# so INNER_CMD's `$(command -v java)`/`$PWD`/`$HOME` etc. expand exactly once, in the actual
# execution environment, same as they did when this same text used to be spliced directly
# into an `ssh HOST "..."` command line.
#
# `setsid nohup` (not just backgrounding with `&`) detaches the job from both the ssh
# session's process group and SIGHUP, so it survives the ssh connection dropping. The exit
# code is captured via `eval "$1"; ec=$?; echo "$ec" > "$2"` rather than relying on `$?`
# after the fact from outside -- by the time a poller can look, the detached process is long
# gone and there's nothing left to ask.
remote_launch_detached() {
  local host="$1" remote_dir="$2" job="$3" inner_cmd="$4"
  local quoted_dir quoted_job quoted_inner
  quoted_dir=$(printf '%q' "$remote_dir")
  quoted_job=$(printf '%q' "$job")
  quoted_inner=$(printf '%q' "$inner_cmd")
  ssh "$host" "cd $quoted_dir 2>/dev/null; bash -s -- $quoted_job $quoted_inner" <<'LAUNCH'
set -u
job="$1"
inner_cmd="$2"
mkdir -p results
log="results/$job.remote.log"
pid_file="results/$job.remote.pid"
exit_file="$log.exit"
rm -f "$exit_file"
setsid nohup bash -c '
  eval "$1"
  ec=$?
  echo "$ec" > "$2"
' bash "$inner_cmd" "$exit_file" > "$log" 2>&1 < /dev/null &
pid=$!
disown "$pid" 2>/dev/null || true
echo "$pid" > "$pid_file"
echo "$pid"
LAUNCH
}

# remote_poll HOST REMOTE_DIR JOB_NAME META_PATH
#
# One ssh round-trip that reports on a job launched by remote_launch_detached: whether its
# PID (results/$JOB_NAME.remote.pid) is still alive, and -- if META_PATH is non-empty (only
# scripts/remote-run.sh has one; scripts/remote-corpus.sh has no run-id/meta.json and passes
# "") -- the run's meta.json `"status"` plus a `cells done/total` count, parsed with grep/sed
# (no python dependency on the remote host). Prints one line:
#   ALIVE=<0|1> STATUS=<meta status|none|unknown> DONE=<n> TOTAL=<n> EXITCODE=<code|none>
# which callers `eval` directly (it's just space-separated `VAR=value` words) to populate
# $ALIVE/$STATUS/$DONE/$TOTAL/$EXITCODE in their own shell.
#
# meta.json parsing notes (see bench/runner.py's `run()` for the schema this depends on):
#   - `grep -m1 '"status"'` finds the run's *top-level* "status" field specifically, not one
#     of the many per-cell "status" fields nested under "cells" -- meta.json's top-level keys
#     (schema_version, run_id, ..., status, host, args, candidates, cells) are written in
#     that fixed order, and "status" is the only occurrence of the string before "cells"
#     starts, so the *first* match in the file is always the top-level one.
#   - `done` cells = count of `"candidate":` -- that key only ever appears inside a cell
#     entry (`{"candidate": ..., "board": ..., "seed": ..., "status": ...}`); the top-level
#     "candidates" list uses "name" instead (see Candidate.to_json), so there's no collision.
#   - `total` cells = (# of "name": keys, i.e. candidates) x (# of boards) x seeds, each
#     parsed straight out of meta.json's `args` block; boards is a bare JSON array of
#     strings (no "key": prefix), so its element count comes from an `awk` slice between the
#     `"boards": [` line and its closing `]` rather than a key-count grep like the others.
#   - EXITCODE reads results/$JOB_NAME.remote.log.exit, written by remote_launch_detached's
#     wrapper once INNER_CMD exits -- note this happens *after* meta.json's status flips to
#     "complete" (that happens inside the `uv run bench run` process itself, before it
#     exits), so a poll can legitimately observe STATUS=complete with EXITCODE=none for a
#     brief window; callers should re-poll a few times before giving up on the exit code.
remote_poll() {
  local host="$1" remote_dir="$2" job="$3" meta_rel="$4"
  local quoted_dir quoted_job quoted_meta
  quoted_dir=$(printf '%q' "$remote_dir")
  quoted_job=$(printf '%q' "$job")
  quoted_meta=$(printf '%q' "$meta_rel")
  ssh "$host" "cd $quoted_dir 2>/dev/null; bash -s -- $quoted_job $quoted_meta" <<'POLL'
set -u
job="$1"
meta="$2"
pid_file="results/$job.remote.pid"
log="results/$job.remote.log"
alive=0
if [ -f "$pid_file" ]; then
  p=$(cat "$pid_file" 2>/dev/null)
  if [ -n "$p" ] && kill -0 "$p" 2>/dev/null; then alive=1; fi
fi
status="none"; done=0; total=0
if [ -n "$meta" ] && [ -f "$meta" ]; then
  status=$(grep -m1 '"status"' "$meta" | sed -E 's/.*"status": *"([a-zA-Z_]+)".*/\1/')
  [ -n "$status" ] || status="unknown"
  done=$(grep -c '"candidate":' "$meta" 2>/dev/null); done=${done:-0}
  ncand=$(grep -c '"name":' "$meta" 2>/dev/null); ncand=${ncand:-0}
  seeds=$(grep -m1 '"seeds":' "$meta" | sed -E 's/.*"seeds": *([0-9]+).*/\1/'); seeds=${seeds:-0}
  nboards=$(awk '/"boards": \[/{f=1;next} f && /\]/{f=0} f' "$meta" | grep -c '"' 2>/dev/null); nboards=${nboards:-0}
  total=$((ncand * nboards * seeds))
else
  status="none"
fi
ec="none"
if [ -f "$log.exit" ]; then ec=$(cat "$log.exit" 2>/dev/null); [ -n "$ec" ] || ec="none"; fi
echo "ALIVE=$alive STATUS=$status DONE=$done TOTAL=$total EXITCODE=$ec"
POLL
}
