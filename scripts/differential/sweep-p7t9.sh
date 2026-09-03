#!/usr/bin/env bash
# Run the `p7t9` differential over every batch stem of `tests/reference/router-fixtures.txt` in
# every mode, and print a per-row MATCH/XDIFF/DIFF/SKIP table with the wall clock each row cost.
#
# The `sweep-p5t1.sh` shape: compile both sides once through `run.sh p7t9` (which also smoke-runs
# the pair), then loop the built artifacts over every row, so the JVM and the Rust binary are
# launched per row but nothing is rebuilt. A regression is then **a row that changes**, not a wall
# of diff.
#
# Usage: sweep-p7t9.sh [stem ...] [--modes a,b,c]
# Env:   FREEROUTING_JAVA_DIR, JAVA25_HOME, FREEROUTING_JAR, P5T_HASH_MODE (all as in run.sh)
#        SWEEP_OUT         directory for the per-row outputs (default: a temp dir kept on failure)
#        P7T9_TIMEOUT      per-invocation wall-clock bound, seconds (default 3600)
#        SWEEP_RESUME=1    skip a row whose `.result` file already exists in SWEEP_OUT
#        SWEEP_OPT_PASSES  the `optimizer` modes' `optPasses` (default 1)
#        SWEEP_OPT_ITEMS   …and their `optItems` (default 20)
#
# ## Resumability
#
# The whole sweep is 8 stems x 4 modes x 2 sides = 64 whole-board runs, and the four slow stems
# route a real board twice per row. Every row writes `$SWEEP_OUT/<stem>.<mode>.result`, so
# `SWEEP_OUT=<dir> SWEEP_RESUME=1 sweep-p7t9.sh` after an interruption picks up where it stopped.
# Nothing is truncated: both sides' full stdout and stderr are kept per row (a MATCH row's
# transcripts are deleted, its `.result` is not).
#
# ## The four modes
#
#   router-only    the pass loop with fanout off and the optimizer off, on the pristine board
#   router+fanout  the same with the SMD fanout pre-pass on
#   optimizer      the routing prologue and then `BatchOptimizer.runBatchLoop` on a fresh flag,
#                  bounded to `optPasses = 1` and `optItems = 20` (see below)
#   batch          the jar's real `-de <dsn> -do <ses>` flow: the board loaded through
#                  `HeadlessBoardManager` (ruling AW), settings from the real two-merge ladder,
#                  and the whole `RoutingPipeline`
#
# ## Why the `optimizer` rows are bounded and the `batch` rows are not
#
# Mode `optimizer` hands the optimizer stage a **fresh** stop flag on both sides, which is the
# only way past quirk #227 — in the production shape every `optRouteItem` rejects immediately —
# and therefore the only mode in which the stage really re-routes. Unbounded that is the most
# expensive thing in this repository: `Issue730-DAC2020_bm11.dsn` at `optPasses = 2, optItems =
# all` was still running after **15 CPU-minutes per side** when this bound was added. `optPasses
# 1 / optItems 20` keeps the mode exercised — the same `ReadSortedRouteItems` order, the same
# `optRouteItem` on the same first twenty items — at a bounded cost, and `p7t8`'s own
# `<dsn> item <passes> <items>` rows are where the deeper optimizer walks live.
#
# The `batch` rows are **not** bounded, because there the optimizer runs with the CLI's own
# limits and the CLI's own stop flag, i.e. quirk #227's production shape: the stage visits every
# item and changes nothing, which is fast. That is the configuration the references encode.
#
# The `full` mode of Task 15 is deliberately **not** swept: `batch` is `full` on the board and the
# settings the jar actually uses, so sweeping both would double the cost to re-measure the same
# pipeline on a board no CLI run ever produces. `run.sh p7t9 <dsn> <n> full` still exists.
#
# Modes `router-only`/`router+fanout`/`optimizer` take the stem's `max_passes` from the fixture
# table; `batch` additionally takes its `fanout`/`optimizer` columns, so a row's argv is exactly
# the reference generator's.
#
# Exit status is 0 when every row matches, 1 otherwise. There is no expected-diff table: unlike
# `sweep-p5t1.sh`, nothing here is hash-dependent (`--verify-hash-modes` is the evidence), so any
# DIFF is a real finding.
set -uo pipefail

DIFF_ROOT="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$DIFF_ROOT/../.." && pwd)"
FREEROUTING_JAVA_DIR="${FREEROUTING_JAVA_DIR:-$ROOT/../freerouting}"
JAVA25_HOME="${JAVA25_HOME:-/opt/homebrew/opt/openjdk@25/libexec/openjdk.jdk/Contents/Home}"
FREEROUTING_JAR="${FREEROUTING_JAR:-$FREEROUTING_JAVA_DIR/build/libs/freerouting-current-executable.jar}"
JAVABIN="$JAVA25_HOME/bin/java"
CLASSES="$DIFF_ROOT/build/classes-p7t9"
RUST_BIN="$DIFF_ROOT/rust/target/release/p7t9"
FIXTURES="$ROOT/tests/reference/router-fixtures.txt"
SWEEP_OUT="${SWEEP_OUT:-$(mktemp -d "${TMPDIR:-/tmp}/p7t9-sweep.XXXXXX")}"
TIMEOUT_SECONDS="${P7T9_TIMEOUT:-3600}"
# The `optimizer` modes' two positionals. See "Why the `optimizer` rows are bounded" above.
SWEEP_OPT_PASSES="${SWEEP_OPT_PASSES:-1}"
SWEEP_OPT_ITEMS="${SWEEP_OPT_ITEMS:-20}"
mkdir -p "$SWEEP_OUT"
export FREEROUTING_JAR

MODES=(router-only router+fanout optimizer batch)
WANTED=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    --modes) IFS=',' read -r -a MODES <<< "$2"; shift 2 ;;
    --modes=*) IFS=',' read -r -a MODES <<< "${1#--modes=}"; shift ;;
    *) WANTED+=("$1"); shift ;;
  esac
done

# `-XX:hashCode=2` and the two locale properties: `run.sh`'s `P5T_JAVA_FLAGS`, so
# `P5T_HASH_MODE=0..4` sweeps this driver too.
P5T_HASH_MODE="${P5T_HASH_MODE:-2}"
JAVA_FLAGS=(
  -Djava.awt.headless=true
  -Duser.language=en
  -Duser.country=US
  -XX:+UnlockExperimentalVMOptions
  "-XX:hashCode=$P5T_HASH_MODE"
)

# The wall-clock bound. Running unbounded is allowed but announced, because the failure it guards
# (quirk #162's non-terminating room completion) is a hang and not an error.
TIMEOUT=()
if command -v timeout >/dev/null 2>&1; then
  TIMEOUT=(timeout "$TIMEOUT_SECONDS")
elif command -v gtimeout >/dev/null 2>&1; then
  TIMEOUT=(gtimeout "$TIMEOUT_SECONDS")
else
  # Quirk #162 is FIXED in the port at Plan 9 Task 8, so the port half of this bound is no
  # longer load-bearing. The **jar** half is: the HEAD jar still walks a simplex it derives
  # after the side numbers, and `calculateNewIncompleteRooms` still allocates a room per turn
  # until the JVM dies. The bound therefore stays until a Java-side fix lands.
  echo "warning: no timeout(1) on PATH; the jar's quirk-#162 hang will not be bounded" >&2
fi

wanted() {
  [[ ${#WANTED[@]} -eq 0 ]] && return 0
  local stem="$1" w
  for w in "${WANTED[@]}"; do [[ "$w" == "$stem" ]] && return 0; done
  return 1
}

echo "== compiling both sides (run.sh p7t9) =="
"$DIFF_ROOT/run.sh" p7t9 >/dev/null || { echo "smoke run failed" >&2; exit 1; }

printf '%-24s %-14s %-7s %-8s %s\n' "stem" "mode" "result" "seconds" "lines"
fail=0
total=0
skipped=0
start=$SECONDS
while IFS='|' read -r stem dsn _max_items _ripup max_passes fanout optimizer; do
  [[ -z "$stem" || "$stem" == \#* ]] && continue
  [[ -n "$max_passes" && "$max_passes" != "-" ]] || continue
  wanted "$stem" || continue
  for mode in "${MODES[@]}"; do
    total=$((total + 1))
    tag="$stem.$mode"
    if [[ "${SWEEP_RESUME:-0}" == "1" && -f "$SWEEP_OUT/$tag.result" ]]; then
      printf '%-24s %-14s %-7s %-8s %s\n' "$stem" "$mode" \
          "$(cut -d' ' -f1 < "$SWEEP_OUT/$tag.result")" "-" "(resumed)"
      [[ "$(cut -d' ' -f1 < "$SWEEP_OUT/$tag.result")" == "MATCH" ]] || fail=$((fail + 1))
      continue
    fi
    row_start=$SECONDS
    args=("$FREEROUTING_JAVA_DIR/$dsn" "$max_passes" "$mode")
    # `batch` is the only mode with the two `on`/`off` switches; the `optimizer` modes take the
    # optimizer-limit positionals the earlier tasks gave them, bounded here (see the header).
    if [[ "$mode" == batch* ]]; then
      args+=(--fanout "$fanout" --optimizer "$optimizer")
    elif [[ "$mode" == optimizer* ]]; then
      args+=("$SWEEP_OPT_PASSES" "$SWEEP_OPT_ITEMS")
    fi

    j="$SWEEP_OUT/$tag.j"
    r="$SWEEP_OUT/$tag.r"
    # Java's stderr goes to a side file rather than /dev/null: an uncaught exception's stack trace
    # is the only thing that distinguishes "Java crashed" from "Java disagreed".
    "${TIMEOUT[@]}" "$JAVABIN" "${JAVA_FLAGS[@]}" -cp "$CLASSES:$FREEROUTING_JAR" \
        app.freerouting.autoroute.pipeline.P7T9 "${args[@]}" >"$j" 2>"$j.err"
    j_rc=$?
    "${TIMEOUT[@]}" "$RUST_BIN" "${args[@]}" >"$r" 2>"$r.err"
    r_rc=$?
    seconds=$((SECONDS - row_start))

    if [[ "$j_rc" -eq 0 && "$r_rc" -eq 0 ]] && diff -q "$j" "$r" >/dev/null 2>&1; then
      result="MATCH"
      lines="$(wc -l < "$j" | tr -d ' ')"
      echo "MATCH $lines lines" > "$SWEEP_OUT/$tag.result"
      rm -f "$j" "$r" "$j.err" "$r.err"
    elif [[ "$j_rc" -ne 0 || "$r_rc" -ne 0 ]]; then
      result="SKIP"
      lines="java exit=$j_rc rust exit=$r_rc"
      echo "SKIP $lines" > "$SWEEP_OUT/$tag.result"
      skipped=$((skipped + 1))
    else
      result="DIFF"
      lines="first: $(diff "$j" "$r" | head -1)"
      echo "DIFF $lines" > "$SWEEP_OUT/$tag.result"
      fail=$((fail + 1))
    fi
    printf '%-24s %-14s %-7s %-8s %s\n' "$stem" "$mode" "$result" "$seconds" "$lines"
  done
done < "$FIXTURES"

echo
echo "rows: $total   diffs: $fail   skipped: $skipped   wall clock: $((SECONDS - start)) s"
echo "hash mode: -XX:hashCode=$P5T_HASH_MODE   outputs: $SWEEP_OUT"
[[ "$fail" -gt 0 || "$skipped" -gt 0 ]] && exit 1
exit 0
