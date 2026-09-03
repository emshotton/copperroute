#!/usr/bin/env bash
# The Plan 9 tier-G2 harness: the per-task stem quality **and time** A/B.
#
#   scripts/quality-ab.sh T<n> [--dry-run] [--repeats N] [stem ...]
#
# It runs the port over the 29 reference stems — **8 batch + 13 CLI + 8 DRC** — at each stem's
# own committed `-mp` cap, and writes one row per stem to
#
#   benchmark/baselines/ab/quality-ab-T<n>.tsv      (committed at task close, ruling BP15)
#   scripts/differential/out/quality-ab-T<n>.txt    (the console output, in full, never piped)
#
# ==================================================================================================
# 1. Where the numbers come from, and the one place they must never come from
# ==================================================================================================
#
# **Incompletes and violations are read from the referee's DRC document and never from the
# manifest.** The router's in-process statistics (`connections.incomplete_count`,
# `clearance_violations.total_count`, and the `normalized_score` they feed) disagree with the
# program's own DRC-only mode — `-de in.dsn out.ses -drc report.json` — on **45-63 % of boards**
# (survey §4.1; e.g. per-pin versus per-pair hole-clearance counting). An A/B scored on the
# manifest is an A/B scored on the thing several Plan 9 tasks are busy *fixing*, so it would move
# for reasons that have nothing to do with routing.
#
# This script therefore routes each stem, then re-loads the board **and its own SES** through
# `-drc` and reads that document. If the referee run fails or writes nothing, the row is a hard
# error and the script exits non-zero: **there is no fallback to the manifest.** That is what
# "fail loudly if handed a manifest number" means here — the manifest's own incomplete and
# violation counts are never read, never printed and never written to the tsv, so no later reader
# can mistake one for a gate input.
#
# The remaining quality columns are board statistics and are read from the manifest, because that
# is where they live and nothing disputes them: `normalized_score` (`fr_router::score::
# normalized_score`, an `f32` throughout and deliberately so — survey §9.1),
# `traces.total_length_mm`, `vias.total_count` split through-hole/blind/buried, and
# `bends.total_count`.
#
# **Forbidden inputs, until their fix lands** (G2's own list): `traces.total_{vertical,horizontal,
# angled}_length` do not sum to the total (#195, Task 19) and `board.bounding_box.width`/`height`
# hold the lower-left corner rather than a size (#196, Task 19). None is read here.
#
# ==================================================================================================
# 2. Two lanes per stem: quality with the clock off, time with the clock on
# ==================================================================================================
#
# * **The quality lane** runs with `RouterBudget::disabled()` — ruling AI, which survives the
#   Plan 9 switch: *time is out of every quality measurement*, because a live wall-clock budget
#   makes the routed board depend on how fast the machine is, and an A/B between two tasks would
#   then be comparing two machine loads. The CLI reads `FR_ROUTER_BUDGET=disabled` at exactly one
#   site (`crates/freerouting/src/commands/route.rs::harness_budget`); unset, it is Java's four
#   literals and every user, test and committed golden is in that case.
# * **The time lane** runs the budget in its **normal** configuration — what a user gets — and is
#   the `cpu_s` column. It is the **median of `--repeats` runs (default 3)** on an otherwise-quiet
#   machine, and the spread (max - min) is printed beside it. That spread is the only noise figure
#   this tier has, and a reader should compare any ratio against it before believing it.
#
# The two lanes are separate runs because they are separate configurations. The router is
# deterministic, so the quality lane needs one run and not three.
#
# ==================================================================================================
# 3. The two references, and which one is a gate
# ==================================================================================================
#
# * **The port column — the gate.** `benchmark/baselines/ab/quality-ab-T<n-1>.tsv`, the previous
#   task's own tsv. Every rule below is scored against it. Each task commits its own tsv at task
#   close (ruling BP15), so the rolling baseline is a committed artefact a later task *reads*
#   rather than re-derives from goldens that have since moved.
# * **The jar column — context, never a gate.** `benchmark/baselines/quality-baseline-java-head.tsv`,
#   which **Task 1** derives once from the frozen references and writes **outside** the frozen
#   tree. It carries **quality only and no timing at all**. This script does **not** read
#   `tests/reference-frozen/` and must never learn how (BL8 as amended by ruling BP1): the freeze
#   is a historical artefact, written once and read by nothing.
#
# **The rolling time baseline** is `benchmark/baselines/stem-times.tsv` — the **previous task's**
# `cpu_s`, never the jar's. At task close this script updates it in place, in the same commit as
# the tsv (`--update-baseline`, which `--dry-run` refuses).
#
# ==================================================================================================
# 4. The rules this script applies itself (ruling BO, encoded by BP4)
# ==================================================================================================
#
#   incomplete connections   referee DRC   must not rise on any stem; must fall on at least one
#   clearance violations     referee DRC   0 on every routed stem, always (a DRC stem routes
#                                          nothing and one routed stem's board arrives dirty —
#                                          see PRE_EXISTING in the scorer)
#   hole_clearance violations referee DRC  reported — Task 19 owns the counting rule (#195 et al.)
#   normalized score         manifest      must not fall by more than the f32 noise floor
#   total trace length       manifest      reported; down preferred
#   via count                manifest      reported; down preferred — I2 (#296) is watched here
#   bend count               manifest      reported
#   cpu_s                    this script   **equal quality: must not be slower.** Quality-winning:
#                                          tolerated, but > 2x on a stem or > 20 % on the corpus
#                                          median ESCALATES to the controller.
#
# **The exit code is the gate, and the escalation is not hidden in the last line.** A violated
# rule prints its row with a flag, prints an `ESCALATE:` or `REGRESSION:` line of its own, and the
# script exits non-zero — so a task that reads only the final line still fails. A task never
# decides for itself that a flag is acceptable; an `ESCALATE` is a controller ruling.
#
# ==================================================================================================
# 5. The gate version (ruling BP8)
# ==================================================================================================
#
# Every tsv header carries `gate-version`. It starts at **g1**; **Task 13 bumps it to g2** (#82 +
# #147 redefine `incomplete_count`) and **Task 19 bumps it to g3** (#195 + #196 make the length
# breakdown and the bounding box legal inputs). **An A/B is only ever compared within one gate
# version**: this script *refuses* to compare two tsvs whose `gate-version` fields differ, and
# says which task must re-cut which column. The task that bumps the version re-cuts the previous
# task's rows under the new gate in the same commit, so the next comparison is like-for-like.
#
# ==================================================================================================
# 6. What `--dry-run` is for
# ==================================================================================================
#
# Ruling BP13: this script is the evidence 22 tasks depend on, and "no golden moved, G1 green"
# cannot detect a broken one — so **Task 0 exercises it rather than letting Task 1 find out**.
# `--dry-run` is the read-only exercise: it measures every stem and writes its tsv and its console
# log, and it **writes nothing under `tests/`** — which it does not merely promise but *checks*,
# with `git status --porcelain tests/` taken before and after and required to be identical. It
# also refuses `--update-baseline`, so a dry run cannot move the rolling baseline it is supposed
# to be seeding.
#
# ==================================================================================================
# 7. Environment
# ==================================================================================================
#
# FREEROUTING_JAVA_DIR (default ../freerouting) — where the stems' input boards live.
# QUALITY_AB_TIMEOUT (default 3600) — per-run wall-clock bound; quirk #162 can hang a board and
#                    neither language guards it, so a hung stem must be reported, not waited on.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
JAVA_DIR="$(cd "${FREEROUTING_JAVA_DIR:-$ROOT/../freerouting}" && pwd)"
REF="$ROOT/tests/reference"
BASELINES="$ROOT/benchmark/baselines"
AB_DIR="$BASELINES/ab"
STEM_TIMES="$BASELINES/stem-times.tsv"
JAR_TSV="$BASELINES/quality-baseline-java-head.tsv"
PORT_BIN="$ROOT/target/release/freerouting"
OUT_DIR="$ROOT/scripts/differential/out"

# The gate version this build of the script emits and is willing to compare against. Task 13
# changes this to `g2`; Task 19 to `g3`. Changing it without re-cutting the previous task's rows
# is what the refusal below exists to catch.
GATE_VERSION="g1"

# The `cpu_s` noise floor. `> 2x` and `> 20 %` are ruling BO's thresholds and are **not**
# tolerances; this is the floor beneath which a "slowdown" is the machine and not the code. A
# slowdown above it on an equal-quality change is a **rejection**, not a note.
#
# 5 % is the global floor and it is measured, not chosen: over the 29 stems of Task 0's dry run
# the largest median-of-3 spread was 0.556 s on a 10.996 s stem — **5.1 %** — and every other stem
# came in under 3.4 %. The scorer additionally raises the floor **per stem** to that stem's own
# recorded spread whenever the spread is the larger of the two, so a stem that is genuinely noisy
# on this machine is judged against its own noise rather than against the corpus's.
CPU_NOISE_FLOOR="1.05"
# Ruling BO's two escalation thresholds.
CPU_STEM_ESCALATE="2.0"
CPU_CORPUS_ESCALATE="1.20"
# `normalized_score` is an `f32` and three of its terms are rounded (survey §9.1), so a bit-exact
# comparison would flag arithmetic noise as a regression. One part in 10^5 is well inside `f32`'s
# 7 significant digits and well below any score change a routing fix produces.
SCORE_NOISE="0.00001"

# Saved before the parse loop consumes it, for the `tee` re-exec below.
ORIGINAL_ARGV=("$@")

TASK=""
DRY_RUN=0
UPDATE_BASELINE=0
REPEATS=3
WANTED=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    --dry-run) DRY_RUN=1; shift ;;
    --update-baseline) UPDATE_BASELINE=1; shift ;;
    --repeats) REPEATS="${2:?--repeats needs a count}"; shift 2 ;;
    --repeats=*) REPEATS="${1#--repeats=}"; shift ;;
    -h|--help) sed -n '2,10p' "$0" >&2; exit 0 ;;
    --*) echo "error: unknown option $1" >&2; exit 1 ;;
    T[0-9]*) if [[ -z "$TASK" ]]; then TASK="$1"; else WANTED+=("$1"); fi; shift ;;
    *) WANTED+=("$1"); shift ;;
  esac
done
if [[ -z "$TASK" ]]; then
  echo "usage: $0 T<n> [--dry-run] [--repeats N] [--update-baseline] [stem ...]" >&2
  exit 1
fi
if [[ "$DRY_RUN" -eq 1 && "$UPDATE_BASELINE" -eq 1 && -f "$BASELINES/stem-times.tsv" ]]; then
  echo "error: --dry-run cannot --update-baseline once benchmark/baselines/stem-times.tsv" >&2
  echo "       exists; a dry run must not move the rolling baseline it is measuring against." >&2
  echo "       (Seeding it — Task 0, when the file does not exist yet — is allowed, because" >&2
  echo "       there is then no baseline to move and the seed is what a later task reads.)" >&2
  exit 1
fi
TASK_N="${TASK#T}"
PREV_TSV=""
if [[ "$TASK_N" =~ ^[0-9]+$ && "$TASK_N" -gt 0 ]]; then
  PREV_TSV="$AB_DIR/quality-ab-T$((TASK_N - 1)).tsv"
fi
TSV="$AB_DIR/quality-ab-$TASK.tsv"
CONSOLE="$OUT_DIR/quality-ab-$TASK.txt"

# The console copy. The Global Constraint is that a long run's **full** output goes to a file and
# is read from the file — never `| tail`, never `| head`, because a truncated gate is an unread
# gate. `tee` is the one pipe that satisfies that: nothing is dropped, the file is complete when
# the run ends, and a reader watching a 25-minute sweep still sees it happen. The re-exec is how
# the redirect covers the whole script including this header's own preflight, and `PIPESTATUS[0]`
# is what keeps the gate's exit code the script's exit code rather than `tee`'s.
if [[ -z "${QUALITY_AB_TEEING:-}" ]]; then
  mkdir -p "$OUT_DIR"
  export QUALITY_AB_TEEING=1
  set -o pipefail
  "$0" ${ORIGINAL_ARGV+"${ORIGINAL_ARGV[@]}"} 2>&1 | tee "$CONSOLE"
  exit "${PIPESTATUS[0]}"
fi

TIMEOUT_SECONDS="${QUALITY_AB_TIMEOUT:-3600}"
TIMEOUT=()
if command -v timeout >/dev/null 2>&1; then
  TIMEOUT=(timeout "$TIMEOUT_SECONDS")
elif command -v gtimeout >/dev/null 2>&1; then
  TIMEOUT=(gtimeout "$TIMEOUT_SECONDS")
else
  echo "warning: no timeout(1) on PATH; a quirk-#162 hang will not be bounded" >&2
fi

mkdir -p "$AB_DIR" "$OUT_DIR"
SCRATCH="$(mktemp -d "${TMPDIR:-/tmp}/quality-ab.XXXXXX")"
trap 'rm -rf "$SCRATCH"' EXIT

# The dry run's own proof that it wrote nothing under `tests/`.
TESTS_BEFORE=""
if [[ "$DRY_RUN" -eq 1 ]]; then
  TESTS_BEFORE="$(cd "$ROOT" && git status --porcelain tests/ 2>/dev/null || true)"
fi

echo "== building the port's binary (release)"
(cd "$ROOT" && cargo build --release --bin freerouting --quiet)
PORT_SHA="$(cd "$ROOT" && git rev-parse --short=12 HEAD 2>/dev/null || echo unknown)"

wanted() {
  [[ ${#WANTED[@]} -eq 0 ]] && return 0
  local key="$1" w
  for w in "${WANTED[@]}"; do [[ "$w" == "$key" || "$w" == "${key#*/}" ]] && return 0; done
  return 1
}

bool_of() { [[ "$1" == "on" ]] && printf 'true' || printf 'false'; }

# --------------------------------------------------------------------------------------------------
# The stem table: `family|stem|board|mp_cap|…`, built from the same three fixture tables the
# generators read, so the A/B and the references can never describe different runs.
#
# **`|`, not a tab, and that is load-bearing.** Tab is IFS *whitespace*, so `IFS=$'\t' read`
# collapses a run of consecutive tabs and shifts every field left — which silently turned the
# `drc-issue593-ses` row (empty `rules`, non-empty `ses`) into a rules-only run and made two
# different stems report the same numbers. `|` is not whitespace, so an empty field stays an
# empty field; it is also what the three fixture tables themselves use, and no corpus path or
# argv fragment in any of them contains one.
# --------------------------------------------------------------------------------------------------
STEMS="$SCRATCH/stems.tsv"
: > "$STEMS"

# 8 batch stems: `router-fixtures.txt`'s rows that carry the batch columns. The argv is the one
# `tests/reference/<stem>/batch.meta.txt`'s `bare jar` line records, which is what
# `gen-batch-reference.sh --verify-driver` proved the driver and the whole program agree on.
while IFS='|' read -r stem dsn _max_items _ripup max_passes fanout optimizer || [[ -n "$stem" ]]; do
  [[ -z "$stem" || "$stem" == \#* ]] && continue
  [[ -n "$max_passes" && "$max_passes" != "-" ]] || continue
  printf 'batch|%s|%s|%s|-mp %s --router.fanout.enabled=%s --router.optimizer.enabled=%s\n' \
      "$stem" "$dsn" "$max_passes" "$max_passes" "$(bool_of "$fanout")" "$(bool_of "$optimizer")" \
      >> "$STEMS"
done < "$REF/router-fixtures.txt"

# 13 CLI stems: `cli-fixtures.txt`, argv verbatim. `-` is a bare run — no `-mp`, which is
# `DefaultSettings.java:99`'s `maxPasses = 9999`, and the `mp_cap` column says `default` rather
# than inventing a number.
while IFS='|' read -r stem dsn extra _lane || [[ -n "$stem" ]]; do
  [[ -z "$stem" || "$stem" == \#* ]] && continue
  local_extra="$extra"
  [[ "$local_extra" == "-" ]] && local_extra=""
  cap="default"
  if [[ "$local_extra" =~ -mp\ ([0-9]+) ]]; then cap="${BASH_REMATCH[1]}"; fi
  printf 'cli|%s|%s|%s|%s\n' "$stem" "$dsn" "$cap" "$local_extra" >> "$STEMS"
done < "$REF/cli-fixtures.txt"

# 8 DRC stems: `drc-fixtures.txt`. These route nothing — `-drc` reads a board (optionally with a
# `.rules` file and a `.ses`) and checks it — so their row carries the referee's numbers and their
# `cpu_s`, and the routing columns read `-`. That is not a gap: the D family is a pure function of
# the board, and what a Plan 9 fix moves here is the DRC document itself (Task 19).
while IFS='|' read -r stem dsn rules ses || [[ -n "$stem" ]]; do
  [[ -z "$stem" || "$stem" == \#* ]] && continue
  printf 'drc|%s|%s|%s|%s|%s\n' "$stem" "$dsn" "n/a" "$rules" "$ses" >> "$STEMS"
done < "$REF/drc-fixtures.txt"

TOTAL="$(wc -l < "$STEMS" | tr -d ' ')"
echo "== $TOTAL stems (8 batch + 13 CLI + 8 DRC), gate-version $GATE_VERSION, task $TASK"
if [[ "$TOTAL" -ne 29 && ${#WANTED[@]} -eq 0 ]]; then
  echo "error: the three fixture tables yield $TOTAL stems, not the 29 the G2 tier is defined" >&2
  echo "       over. A stem was added or removed; that is a plan amendment, not a local fix." >&2
  exit 1
fi

# --------------------------------------------------------------------------------------------------
# Measurement
# --------------------------------------------------------------------------------------------------

# One run, returning its CPU time (user + sys, in seconds) on stdout. The bash `time` builtin is
# what measures it: `/usr/bin/time`'s output format differs between BSD and GNU, and `TIMEFORMAT`
# does not. The command's own output is redirected **inside** the braces, so what the substitution
# captures is the timing line and nothing else.
run_timed() {
  local log="$1"
  shift
  local t
  t="$( { TIMEFORMAT='%3U %3S'; time "${TIMEOUT[@]}" "$@" > "$log" 2>&1 < /dev/null; } 2>&1 )" || true
  awk '{ printf "%.3f", $1 + $2 }' <<< "$(tail -1 <<< "$t")"
}

# The referee. **No fallback**: if this writes nothing, the caller fails the row.
run_referee() {
  local report="$1"; shift
  local log="$1"; shift
  rm -f "$report"
  "${TIMEOUT[@]}" "$PORT_BIN" "$@" -drc "$report" > "$log" 2>&1 < /dev/null || true
  [[ -s "$report" ]]
}

# `<drc.json> <manifest.json|-> ` -> one tab-separated metric row. Both key spellings of the DRC
# document are accepted (the port writes 2.3.0's snake_case, quirk #92; the jar wrote camelCase),
# so this reads a port document today and a jar-cut one during triage.
metrics_of() {
  python3 - "$1" "$2" <<'PY'
import json, sys

drc = json.load(open(sys.argv[1], encoding="utf-8"))

def pick(doc, *names):
    for n in names:
        if n in doc:
            return doc[n]
    return None

violations = pick(drc, "violations") or []
unconnected = pick(drc, "unconnectedItems", "unconnected_items") or []
# `clearance` and `hole_clearance` are two different violation types and only the first is G2's
# "clearance violations" row. `holeClearance` is the jar's camelCase spelling of the second
# (quirk #92); both are accepted so a jar-cut document can be read during triage. The hole count
# is REPORTED and is not a gate: survey 4.1's 45-63 % self-report disagreement is largely
# per-pin-vs-per-pair hole-clearance counting, which is Task 19's fix, so gating on it today
# would gate on the metric rather than on the routing.
def kind(v):
    return str(v.get("type", "")).lower()

clearance = [v for v in violations if kind(v) == "clearance"]
hole = [v for v in violations if kind(v) in ("hole_clearance", "holeclearance")]

row = [len(unconnected), len(violations), len(clearance), len(hole)]

if sys.argv[2] == "-":
    row += ["-"] * 7
else:
    m = json.load(open(sys.argv[2], encoding="utf-8"))
    bs = m["board_statistics"]
    # NEVER read here: bs["connections"]["incomplete_count"] and
    # bs["clearance_violations"]["total_count"] -- the manifest numbers the referee replaces
    # (survey 4.1's 45-63 % disagreement). The three columns above are the referee's.
    traces, vias, bends = bs["traces"], bs["vias"], bs["bends"]
    row += [
        m["normalized_score"],
        traces["total_length_mm"],
        vias["total_count"],
        vias["through_hole_count"],
        vias["blind_count"],
        vias["buried_count"],
        bends["total_count"],
    ]
print("\t".join(str(x) for x in row))
PY
}

median_and_spread() {
  python3 - "$@" <<'PY'
import statistics, sys
xs = sorted(float(a) for a in sys.argv[1:])
print("%.3f\t%.3f" % (statistics.median(xs), xs[-1] - xs[0]))
PY
}

ROWS="$SCRATCH/rows.tsv"
: > "$ROWS"
STATUS=0

measure_one() {
  local family="$1" stem="$2" key="$family/$2"
  local ses="$SCRATCH/$family-$stem.ses"
  local manifest="$SCRATCH/$family-$stem.manifest.json"
  local report="$SCRATCH/$family-$stem.drc.json"
  local qlog="$SCRATCH/$family-$stem.quality.log"
  local rlog="$SCRATCH/$family-$stem.referee.log"
  local -a route_argv=() referee_argv=()
  local cap="$3"
  shift 3

  if [[ "$family" == drc ]]; then
    local rules="$1" sesfile="$2"
    referee_argv=(-de "$JAVA_DIR/${STEM_BOARD}")
    [[ -n "$sesfile" ]] && referee_argv+=("$JAVA_DIR/$sesfile")
    [[ -n "$rules" ]] && referee_argv+=(-dr "$JAVA_DIR/$rules")
    manifest="-"
  else
    # shellcheck disable=SC2206  -- the fixture's extra_args is a space-separated argv fragment
    local extra=($1)
    route_argv=(-de "$JAVA_DIR/${STEM_BOARD}" -do "$ses" ${extra+"${extra[@]}"}
                "--router.result_json=$manifest")
    referee_argv=(-de "$JAVA_DIR/${STEM_BOARD}" "$ses")
  fi

  printf '== %-38s cap=%s\n' "$key" "$cap"

  # --- the quality lane: the clock off (ruling AI) -------------------------------------------
  if [[ "$family" != drc ]]; then
    if ! FR_ROUTER_BUDGET=disabled "${TIMEOUT[@]}" "$PORT_BIN" "${route_argv[@]}" \
        > "$qlog" 2>&1 < /dev/null || [[ ! -s "$ses" ]]; then
      echo "   FAILED: the quality run wrote no SES; see $qlog" >&2
      STATUS=1
      return 0
    fi
    if [[ ! -s "$manifest" ]]; then
      echo "   FAILED: the quality run wrote no manifest; see $qlog" >&2
      STATUS=1
      return 0
    fi
  fi

  # --- the referee: the DRC document, and no fallback to the manifest ------------------------
  if ! run_referee "$report" "$rlog" "${referee_argv[@]}"; then
    echo "   FAILED: the referee DRC wrote no document for $key; see $rlog." >&2
    echo "           This row has NO fallback: the manifest's own incomplete and violation" >&2
    echo "           counts are not a legal input to this gate (survey §4.1)." >&2
    STATUS=1
    return 0
  fi

  local metrics
  metrics="$(metrics_of "$report" "$manifest")"

  # --- the time lane: the budget in its normal configuration, median of N --------------------
  local times=() i
  for ((i = 0; i < REPEATS; i++)); do
    if [[ "$family" == drc ]]; then
      times+=("$(run_timed "$SCRATCH/$family-$stem.t$i.log" "$PORT_BIN" "${referee_argv[@]}" \
          -drc "$SCRATCH/$family-$stem.t$i.json")")
    else
      times+=("$(run_timed "$SCRATCH/$family-$stem.t$i.log" "$PORT_BIN" \
          -de "$JAVA_DIR/${STEM_BOARD}" -do "$SCRATCH/$family-$stem.t$i.ses" \
          ${TIME_EXTRA+"${TIME_EXTRA[@]}"})")
    fi
  done
  local cpu
  cpu="$(median_and_spread "${times[@]}")"

  printf '%s\t%s\t%s\t%s\t%s\n' "$family" "$stem" "$cap" "$metrics" "$cpu" >> "$ROWS"
  # shellcheck disable=SC2059
  printf '   incomplete=%s violations=%s clearance=%s hole_clearance=%s cpu_s=%s (spread %s over %s runs)\n' \
      "$(cut -f1 <<< "$metrics")" "$(cut -f2 <<< "$metrics")" "$(cut -f3 <<< "$metrics")" \
      "$(cut -f4 <<< "$metrics")" "$(cut -f1 <<< "$cpu")" "$(cut -f2 <<< "$cpu")" "$REPEATS"
}

while IFS='|' read -r family stem board cap rest; do
  wanted "$family/$stem" || continue
  STEM_BOARD="$board"
  TIME_EXTRA=()
  if [[ "$family" == drc ]]; then
    IFS='|' read -r rules sesfile <<< "$rest"
    measure_one "$family" "$stem" "$cap" "$rules" "$sesfile"
  else
    # shellcheck disable=SC2206
    TIME_EXTRA=($rest)
    measure_one "$family" "$stem" "$cap" "$rest"
  fi
done < "$STEMS"

# --------------------------------------------------------------------------------------------------
# The tsv, the comparison and the exit code
# --------------------------------------------------------------------------------------------------
# `set +e` around it: the script's *own* exit code is this gate's verdict, and `set -e` would
# take the failure before `GATE_STATUS` could carry it past the baseline update and the dry run's
# proof — both of which must still run when a rule was violated.
set +e
python3 - "$TSV" "$ROWS" "$GATE_VERSION" "$TASK" "$PORT_SHA" "$PREV_TSV" "$JAR_TSV" \
    "$STEM_TIMES" "$CPU_NOISE_FLOOR" "$CPU_STEM_ESCALATE" "$CPU_CORPUS_ESCALATE" \
    "$SCORE_NOISE" "$REPEATS" "$ROOT" <<'PY'
import statistics, sys, os

(out_path, rows_path, gate, task, sha, prev_path, jar_path, times_path,
 noise_floor, stem_escalate, corpus_escalate, score_noise, repeats, root) = sys.argv[1:]


def rel(p):
    """Repo-relative, because this file is committed and an absolute path is one machine's."""
    if not p:
        return p
    return os.path.relpath(p, root) if p.startswith(root) else p


noise_floor = float(noise_floor); stem_escalate = float(stem_escalate)
corpus_escalate = float(corpus_escalate); score_noise = float(score_noise)

COLUMNS = ["family", "stem", "mp_cap", "incomplete", "violations", "clearance_violations",
           "hole_clearance_violations", "normalized_score", "trace_length_mm", "via_total",
           "via_through", "via_blind", "via_buried", "bend_count", "cpu_s", "cpu_spread_s",
           "flag"]

# The one routed stem in the corpus whose *input* board already violates its own clearance
# rules. `tests/reference/cli-fixtures.txt` names it: "router-strict-drc-cnh | slow | the only
# board with pre-existing clearance violations". It appears once per family it is in.
PRE_EXISTING = {("batch", "router-strict-drc-cnh"), ("cli", "router-strict-drc-cnh")}

rows = []
for line in open(rows_path, encoding="utf-8"):
    line = line.rstrip("\n")
    if not line:
        continue
    rows.append(line.split("\t"))


def read_tsv(path):
    """A tsv written by this script: `# key: value` headers, then a `#` column line, then rows."""
    if not path or not os.path.exists(path):
        return None, {}
    headers, table = {}, {}
    cols = None
    for line in open(path, encoding="utf-8"):
        line = line.rstrip("\n")
        if line.startswith("# ") and ":" in line and cols is None:
            k, _, v = line[2:].partition(":")
            headers[k.strip()] = v.strip()
        elif line.startswith("#"):
            cols = line[1:].split("\t")
        elif line and cols:
            cells = line.split("\t")
            rec = dict(zip(cols, cells))
            table[(rec.get("family"), rec.get("stem"))] = rec
    return headers, table


def check_gate(path, headers, what):
    if headers is None:
        return
    theirs = headers.get("gate-version")
    if theirs and theirs != gate:
        sys.stderr.write(
            f"error: {what} ({path}) carries gate-version {theirs} and this run is {gate}.\n"
            "       An A/B is only ever compared within one gate version (ruling BP8). The task\n"
            "       that bumped the version — Task 13 for g2 (#82 + #147 redefine\n"
            "       incomplete_count), Task 19 for g3 (#195 + #196 make the length breakdown and\n"
            "       the bounding box legal inputs) — must re-cut that baseline's columns under\n"
            "       the new gate, in the same commit, before this comparison means anything.\n")
        sys.exit(3)


prev_headers, prev = read_tsv(prev_path)
check_gate(prev_path, prev_headers, "the port baseline")
jar_headers, jar = read_tsv(jar_path)
check_gate(jar_path, jar_headers, "the jar reference")
times_headers, times = read_tsv(times_path)
check_gate(times_path, times_headers, "the rolling time baseline")

flags, escalations, regressions = [], [], []
fell_somewhere = False
ratios = []

out = []
for r in rows:
    family, stem, cap = r[0], r[1], r[2]
    inc, viol, clear, hole = r[3], r[4], r[5], r[6]
    score, length, vt, vth, vbl, vbu, bends = r[7:14]
    cpu, spread = r[14], r[15]
    flag = "ok"
    base = prev.get((family, stem))

    # Rule: **clearance violations are 0 on every routed stem, always** — a router that creates a
    # clearance violation has produced a board a fabricator will reject, and no score makes that
    # acceptable. Two carve-outs, and each is a fact about the fixture and not a softening:
    #
    #  * a **DRC stem routes nothing**. Its violation count is a property of the board it was
    #    handed — three of the eight fixtures are named for the violations they ship with — so it
    #    is reported and never gated.
    #  * a **routed stem whose input board already violates** carries its violations through. The
    #    corpus has exactly one (`router-strict-drc-cnh`, chosen as "the only board with
    #    pre-existing clearance violations"), and for it the rule degrades to "must not rise",
    #    scored against the port baseline like every other column. `PRE_EXISTING` below is that
    #    list, and a stem is only ever added to it with a measured reason.
    if family != "drc" and clear not in ("-", "0"):
        if (family, stem) in PRE_EXISTING:
            flag = "pre-existing" if flag == "ok" else flag
            if base is not None and base.get("clearance_violations") not in (None, "-")                     and int(clear) > int(base["clearance_violations"]):
                flag = "VIOLATIONS-ROSE"
                regressions.append(
                    f"REGRESSION: {family}/{stem} clearance violations "
                    f"{base['clearance_violations']} -> {clear}; this stem's board arrives with "
                    "violations, so the rule is that they must not rise")
        else:
            flag = "VIOLATIONS"
            regressions.append(f"REGRESSION: {family}/{stem} has {clear} clearance violations; "
                               "the rule is 0 on every routed stem, always")

    tbase = times.get((family, stem))
    if base is None and tbase is None:
        # No reference of any kind: this run *is* the seed of the rolling baseline. Not a pass and
        # not a failure — there is nothing yet to be better or worse than, and saying `seed` is
        # honest where saying `ok` would imply a comparison happened.
        flag = "seed" if flag == "ok" else flag
    else:
        equal_quality = True
        if base is not None:
            if inc != "-" and base["incomplete"] != "-" and int(inc) > int(base["incomplete"]):
                flag = "INCOMPLETE-ROSE"
                regressions.append(
                    f"REGRESSION: {family}/{stem} incomplete {base['incomplete']} -> {inc}; the "
                    "rule is that incompletes must not rise on any stem")
            if inc != "-" and base["incomplete"] != "-" and int(inc) < int(base["incomplete"]):
                fell_somewhere = True
                equal_quality = False
            if score != "-" and base["normalized_score"] not in ("-", None):
                a, b = float(score), float(base["normalized_score"])
                if b and (b - a) / abs(b) > score_noise:
                    flag = "SCORE-FELL" if flag == "ok" else flag
                    regressions.append(
                        f"REGRESSION: {family}/{stem} normalized_score {b} -> {a}, a fall larger "
                        f"than the f32 noise floor ({score_noise})")
                if abs(a - b) / (abs(b) or 1.0) > score_noise:
                    equal_quality = False
        prior_cpu = prior_spread = None
        source = tbase if (tbase is not None and tbase.get("cpu_s") not in (None, "-")) else base
        if source is not None and source.get("cpu_s") not in (None, "-"):
            prior_cpu = float(source["cpu_s"])
            try:
                prior_spread = float(source.get("cpu_spread_s", 0) or 0)
            except ValueError:
                prior_spread = 0.0
        if prior_cpu and float(cpu) > 0:
            ratio = float(cpu) / prior_cpu
            ratios.append(ratio)
            # The per-stem floor: this stem's own measured spread, when that is wider than the
            # corpus floor. See CPU_NOISE_FLOOR's note in the shell half.
            floor = max(noise_floor, 1.0 + (prior_spread or 0.0) / prior_cpu)
            if equal_quality and ratio > floor:
                flag = "SLOWER" if flag == "ok" else flag
                regressions.append(
                    f"REGRESSION: {family}/{stem} is {ratio:.2f}x slower at equal quality "
                    f"({prior_cpu:.3f}s -> {float(cpu):.3f}s, floor {floor:.3f}x). An "
                    "equal-quality change must not be slower (ruling BO); rework it or take a "
                    "controller ruling.")
            elif ratio > stem_escalate:
                flag = "ESCALATE" if flag == "ok" else flag
                escalations.append(f"ESCALATE: {family}/{stem} {ratio:.2f}x")

    if flag not in ("ok", "seed"):
        flags.append(f"{family}/{stem}={flag}")
    out.append([family, stem, cap, inc, viol, clear, hole, score, length, vt, vth, vbl, vbu,
                bends, cpu, spread, flag])

corpus_note = ""
if ratios:
    med = statistics.median(ratios)
    corpus_note = f"{med:.3f}"
    if med > corpus_escalate:
        escalations.append(f"ESCALATE: corpus-median cpu ratio {med:.3f} "
                           f"(> {corpus_escalate}) against the previous task")

with open(out_path, "w", encoding="utf-8") as fh:
    fh.write(f"# gate-version: {gate}\n")
    fh.write(f"# task: {task}\n")
    fh.write(f"# port-sha: {sha}\n")
    fh.write(f"# repeats: {repeats}\n")
    if not prev_path:
        fh.write("# port-baseline: (none — this run seeds the rolling baseline)\n")
    else:
        fh.write(f"# port-baseline: {rel(prev_path)}"
                 f"{'' if os.path.exists(prev_path) else '  [absent]'}\n")
    fh.write(f"# jar-baseline: {rel(jar_path)}"
             f"{'' if os.path.exists(jar_path) else '  [absent — Task 1 writes it]'}\n")
    fh.write(f"# time-baseline: {rel(times_path)}"
             f"{'' if os.path.exists(times_path) else '  [absent — this run seeds it]'}\n")
    fh.write(f"# corpus-median-cpu-ratio: {corpus_note or '(no time baseline)'}\n")
    fh.write("# source-of-incomplete-and-violations: the referee's DRC document"
             " (`-de <board> <ses> -drc <report>`), NEVER the manifest (survey §4.1)\n")
    fh.write("# forbidden-inputs: traces.total_{vertical,horizontal,angled}_length (#195),"
             " board.bounding_box.width/height (#196)\n")
    fh.write("# quality-lane-budget: RouterBudget::disabled() (ruling AI);"
             " cpu_s lane: RouterBudget::default()\n")
    fh.write("#" + "\t".join(COLUMNS) + "\n")
    for row in out:
        fh.write("\t".join(str(c) for c in row) + "\n")

print()
print(f"wrote {rel(out_path)} ({len(out)} rows, gate-version {gate})")
if corpus_note:
    print(f"corpus-median cpu ratio against the previous task: {corpus_note}")
if prev and not fell_somewhere and not regressions:
    print("NOTE: no stem's incomplete count fell. G2's rule is that incompletes must not rise on "
          "any stem AND must fall on at least one; a task whose fix list claims a connectivity "
          "win and shows no fall has not shown it.")
for line in regressions:
    print(line)
for line in escalations:
    print(line)
if regressions or escalations:
    print()
    print("This is a gate, not a report: the exit code is non-zero and a task does not close on "
          "its own judgement that a flag is acceptable.")
    sys.exit(1)
print("all rows within the G2 rules")
PY
GATE_STATUS=$?
set -e

# --------------------------------------------------------------------------------------------------
# The rolling time baseline, and the dry run's own proof
# --------------------------------------------------------------------------------------------------
if [[ "$UPDATE_BASELINE" -eq 1 ]]; then
  python3 - "$TSV" "$STEM_TIMES" "$GATE_VERSION" "$TASK" "$ROOT/benchmark/results/v1.0.0-rs" <<'PY'
import glob, json, statistics, sys
tsv, times, gate, task, v100 = sys.argv[1:]
cols, rows = None, []
for line in open(tsv, encoding="utf-8"):
    line = line.rstrip("\n")
    if line.startswith("#") and "\t" in line:
        cols = line[1:].split("\t")
    elif line and not line.startswith("#"):
        rows.append(dict(zip(cols, line.split("\t"))))

# The corpus position of the binary this branch is based on. `benchmark/results/` is gitignored,
# so this is read when it is there and quoted from the run of record when it is not; either way it
# is provenance in the header and never a per-stem number, because the two corpora are disjoint --
# see the note the header itself carries.
corpus = None
cells = sorted(
    m["cpu_s"]
    for p in glob.glob(f"{v100}/rs-main/*/seed-1/metrics.json")
    for m in [json.loads(open(p, encoding="utf-8").read())]
    if m.get("cpu_s") is not None
)
if cells:
    corpus = (len(cells), statistics.median(cells), statistics.fmean(cells), sum(cells))

with open(times, "w", encoding="utf-8") as fh:
    fh.write(f"# gate-version: {gate}\n")
    fh.write(f"# task: {task}\n")
    fh.write("# The rolling PORT cpu_s baseline (rulings BO / BP4) -- the PREVIOUS TASK's number,\n")
    fh.write("# never the jar's. scripts/quality-ab.sh reads it as the reference for its cpu_s\n")
    fh.write("# column and rewrites it in place at each task's close, in the same commit as\n")
    fh.write("# benchmark/baselines/ab/quality-ab-<task>.tsv.\n")
    fh.write("#\n")
    fh.write("# SEEDING, and why the per-stem numbers are measured rather than copied.\n")
    fh.write("# The plan seeds this file from the v1.0.0-rs run of record's per-board cpu_s. That\n")
    fh.write("# run is the port's measured pre-fix position and is the right provenance -- but its\n")
    fh.write("# 605 boards are the `pcbench` corpus (`pcbench-*`), and this file's 29 rows are the\n")
    fh.write("# `tests/reference/` stems. The two sets are disjoint: not one stem is a pcbench\n")
    fh.write("# board, so there is no per-stem number in that run to copy. The seed is therefore\n")
    fh.write("# Task 0's own median-of-3 measurement of the same binary generation (the branch's\n")
    fh.write("# base commit is what produced v1.0.0-rs), and the run of record is recorded here as\n")
    fh.write("# the corpus-level position every milestone's cpu ratio is taken against.\n")
    if corpus:
        n, med, mean, total = corpus
        fh.write(f"# v1.0.0-rs corpus cpu_s: cells {n}, median {med:.3f}, mean {mean:.3f}, "
                 f"total {total:.1f}\n")
    else:
        fh.write("# v1.0.0-rs corpus cpu_s: cells 605, median 3.040, mean 25.199, total 15245.2\n")
        fh.write("#   (quoted -- benchmark/results/ is gitignored and this checkout has no copy)\n")
    fh.write("#family\tstem\tcpu_s\tcpu_spread_s\n")
    for r in rows:
        fh.write(f"{r['family']}\t{r['stem']}\t{r['cpu_s']}\t{r['cpu_spread_s']}\n")
print(f"updated {times} from {tsv}")
PY
fi

if [[ "$DRY_RUN" -eq 1 ]]; then
  TESTS_AFTER="$(cd "$ROOT" && git status --porcelain tests/ 2>/dev/null || true)"
  if [[ "$TESTS_BEFORE" != "$TESTS_AFTER" ]]; then
    echo "error: --dry-run wrote under tests/ — that is the one thing it must not do" >&2
    diff <(printf '%s\n' "$TESTS_BEFORE") <(printf '%s\n' "$TESTS_AFTER") >&2 || true
    exit 1
  fi
  echo "dry run: git status --porcelain tests/ is unchanged (nothing was written under tests/)"
fi

if [[ "$STATUS" -ne 0 ]]; then
  echo "one or more stems failed to measure; the tsv above is incomplete" >&2
  exit "$STATUS"
fi
exit "$GATE_STATUS"
