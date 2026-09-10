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
# also required for `bench run` when importing routed sessions for KiCad scoring.
#
# Deliberately nothing here about HOME/XDG_*/APPDATA: freerouting's persisted
# freerouting.json settings-leak isolation is done
# per cell, in bench.runner.run_cell -- each candidate
# invocation gets its own HOME under its own results/ cell dir -- not via a shared override
# here. A single HOME exported for the whole remote job would defeat the point: every
# candidate invocation in the job would collide on the same freerouting.json again.
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
  export COPPERROUTE_TIME=$remote_time
  export COPPERROUTE_KICAD_CLI=\$(command -v kicad-cli || true)
  export COPPERROUTE_KICAD_PYTHON=$remote_kicad_python
  export UV_PYTHON_DOWNLOADS=never
PREFIX
}

_remote_job_exists() {
  local job="$1" suffix
  for suffix in "" .remote.log .remote.pid .remote.log.exit .remote.lock; do
    [[ -e "results/$job$suffix" ]] && return 0
  done
  return 1
}

remote_launch_detached() {
  local host="$1" remote_dir="$2" job="$3" inner_cmd="$4"
  {
    printf 'remote_dir=%q\njob=%q\ninner_cmd=%q\n' "$remote_dir" "$job" "$inner_cmd"
    declare -f _remote_job_exists
    cat <<'LAUNCH'
set -u
cd "$remote_dir" || exit 1
mkdir -p results
log="results/$job.remote.log"
pid_file="results/$job.remote.pid"
exit_file="$log.exit"
if _remote_job_exists "$job"; then
  echo "error: job '$job' already exists; choose a new ID" >&2
  exit 1
fi
# mkdir arbitrates between launches that both passed the existence check.
if ! mkdir "results/$job.remote.lock" 2>/dev/null; then
  echo "error: job '$job' already launched; choose a new ID" >&2
  exit 1
fi
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
  } | ssh "$host" "bash -s"
}

# The login shell only starts Bash; expansions and control flow travel on stdin.
remote_poll() {
  local host="$1" remote_dir="$2" job="$3" meta_rel="$4"
  {
    printf 'remote_dir=%q\njob=%q\nmeta=%q\n' "$remote_dir" "$job" "$meta_rel"
    declare -f _remote_job_exists
    cat <<'POLL'
set -u
if [ ! -d "$remote_dir" ]; then
  echo 'EXISTS=0 ALIVE=0 STATUS=none DONE=0 TOTAL=0 EXITCODE=none'
  exit 0
fi
cd "$remote_dir" || exit 1
pid_file="results/$job.remote.pid"
log="results/$job.remote.log"
alive=0
exists=0
if _remote_job_exists "$job"; then
  exists=1
fi
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
echo "EXISTS=$exists ALIVE=$alive STATUS=$status DONE=$done TOTAL=$total EXITCODE=$ec"
POLL
  } | ssh "$host" "bash -s"
}
