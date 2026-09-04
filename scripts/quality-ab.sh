#!/usr/bin/env bash
# The Plan 9 tier-G2 harness: the per-task stem quality **and time** A/B.
#
#   scripts/quality-ab.sh T<n> [--dry-run] [--repeats N] [--jobs N]
#                             [--claims-connectivity] [--update-baseline] [stem ...]
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
#   site (`crates/freerouting/src/commands/route.rs::run_budget`), and that variable is a **test
#   harness seam**, not a user surface — it is not a setting, it cannot be merged, and it does not
#   appear in a manifest's `settings_snapshot`.
#
#   **Task 1's #234 makes half of this seam redundant, and only half** (ruling BR). Since
#   `opt_changed_area_ms` defaults to `0`, an unset run and a `disabled` run already agree about
#   the pull-tight clock — the one clock that changes a routed board. What still separates them is
#   `fanout_ms_per_pin`, Java's `10000` against `disabled`'s `i32::MAX`: the fanout stage's per-pin
#   budget is live in a default run, it *does* change what gets routed on a big board, and taking
#   it out of a quality measurement is exactly ruling AI. So this lane keeps setting the variable.
#   Whether the seam should exist at all once nothing else needs it is **Task 24's** decision and
#   is parked there; nothing before Task 24 should remove it.
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
#   tree. It carries **quality only and no timing at all**, and it is rendered into every row's
#   `jar_*` columns so a reader can see where the jar stood without opening a second file. No rule
#   reads it, no flag comes from it and it cannot change the exit code. Until Task 1 writes it the
#   columns render `-` and the tsv header says `[absent]` — this script never affirms a reference
#   it did not read. **Task 1 must write that file in this script's own tsv shape**: a
#   `# gate-version:` header, a `# cols:` header, and `family`/`stem` keys matching these 29 rows.
#   This script does **not** read `tests/reference-frozen/` and must never learn how (BL8 as
#   amended by ruling BP1): the freeze is a historical artefact, written once and read by nothing.
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
#                                          **when the task declares a connectivity claim**
#                                          (`--claims-connectivity`) — ruling BZ, below
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
# ## The BP12 arm on "incompletes must not rise" (ruling BU(b))
#
# **On an escalated task the M-bench is the gate, and this script records.** The rule above was
# written for the parity era, when a rise in incompletes meant the port had diverged from the jar.
# Once the port's answer is allowed to be *better* than freerouting's, the rule cannot express a
# **trade of connectivity for legality** — which is exactly what a fix like R2 (#294) is: a
# connection that can only be closed with a sub-minimum trace should fail instead of being closed
# illegally, so the honest incomplete count goes **up** and the board's DRC-clean rate goes up with
# it.
#
# So: when a task has raised a **BP12 escalation** and the controller has adjudicated it against a
# **milestone bench** (M1/M2/M3 — 605 boards, the frozen `java-278fe14` view), the milestone's
# verdict is the acceptance decision and this script's flags are **evidence in it**, not a veto
# over it. Nothing here softens: the flag still prints, the row still names the stem, and the exit
# code is still non-zero. What changes is only *who decides*, and the answer is never the task —
# it is the controller, on the bench.
#
# Precedent, and the shape to follow: **Plan 9 Task 2**. R1 (#293) + R2 (#294) raised incompletes
# on 10 of the 21 routed rows over 6 stems (`grep -c INCOMPLETE-ROSE
# benchmark/baselines/ab/quality-ab-T2.tsv`), against sub-minimum-width traces falling 372 -> 2 and
# clearance violations staying 0. The task escalated with the numbers rather than deciding for
# itself; M1 answered corpus clean-pass 0.375 -> 0.550 and small-tier DRC-clean 0.727 -> 0.958; and
# **ruling BV accepted with the residual gap recorded**. An unescalated task with the same flags is
# still a stop-and-report — the arm is not a general licence for incompletes to rise.
#
# ## The scope of "must fall on at least one" (ruling BZ(a))
#
# The other half of that rule — *incompletes must fall on at least one stem* — is **not a property
# of every task**, and applying it to every task was a bug in the gate rather than a strict
# reading of it. Most of the plan's 25 tasks fix things that do not touch connectivity at all: a
# parser field, a DTO's spelling, a statistics counter. For such a task **bit-identical quality is
# the correct outcome**, and a gate that demands a connectivity win from it is demanding that the
# task change something it was written not to change.
#
# **Task 5 is the standing case.** Six fixes (#71/#106/#76/#211/#45/#105), none of them claiming a
# connectivity win; its A/B came back with every quality cell equal to Task 4's — exactly right —
# and the must-fall line fired anyway. Ruling BZ(a) granted the waiver and scoped the rule:
#
#   **The must-fall rule binds only a task that DECLARES a connectivity claim.**
#
# The declaration is this script's `--claims-connectivity` flag, and it is **off by default**:
#
# * **without the flag** the absence of a fall prints a `NOTE:` and changes nothing — not the flag
#   column, not the exit code. The task's fix list did not promise a connectivity win, so there is
#   nothing here for the gate to falsify.
# * **with the flag** the absence of a fall is a `REGRESSION:` and the script exits non-zero. A task
#   that claims a connectivity win and cannot show one on 29 stems has not shown it, and that is
#   precisely the case the rule was written for.
#
# **A task passes `--claims-connectivity` when its own fix list claims a connectivity win** —
# Task 8 (rooms/doors, in flight as this lands) is one, and the plan's acceptance text names the
# others as they come ("incompletes must fall on at least one stem": Tasks 7, 10 and the R-family
# groups). The flag is a *declaration by the task*, not a judgement by the script: the script
# cannot read a fix list, and a task that quietly omits the flag on a connectivity fix has
# mis-declared its own work — which is a review finding, exactly like a missing `goldens moved:`
# line. Nothing else about the rule moves: **rises are still gated unconditionally**, on every
# task, flag or no flag, subject only to the BP12 arm above.
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
# 7. `--jobs`: the quality+referee lanes fan out, the timing lane never does (W17)
# ==================================================================================================
#
# The multithreading survey's item 2 (`docs/plan-9-prep/multithreading-survey.md` §4.1). Per stem
# this script performs 1 quality run + 1 referee DRC + `REPEATS`(=3) timed runs, so a sweep is
# roughly 4x the corpus's 111 s of CPU. The run is therefore split into **two phases**:
#
# * **the quality phase — parallel.** The quality route, the referee DRC, `metrics_of` and
#   `neckdown_of`, one worker **process** per stem, `--jobs` of them at a time.
# * **the timing phase — strictly sequential, and it always will be.** `cpu_s` is a *measurement*,
#   and a measurement taken while N other routes fight this one for the same cores is not a
#   measurement of the stem. This phase also does all the printing and all the row-appending.
#
# That split is what caps the prize at **~1.25x**, and the cap is structural rather than an
# implementation shortfall: 3 of the 4 corpus-passes are the timing lane and stay serial, so the
# best possible outcome is collapsing the remaining pass to its longest stem. This is a harness
# convenience, not a speed result — do not quote it as one.
#
# **Why this is safe, and the assertion that keeps it safe.** Survey §4.1's determinism argument
# has three legs and each is a fact about this script rather than a hope:
#
#  1. **Separate processes with per-stem scratch paths.** Every artefact a stem writes is already
#     named `$SCRATCH/$family-$stem.*`; no two stems share a path, and the workers share no state.
#  2. **Canonical merge order.** Rows are not appended by the workers at all. Each worker writes
#     `$SCRATCH/parts/<family>-<stem>.part` (its metrics) and `.msg` (anything it had to say), and
#     the sequential phase walks the stem table in its own fixed order, replays each `.msg` and
#     appends each row. So the tsv **and the console log** come out in `(family, stem)` order at
#     any `--jobs`, and `--jobs 1` reproduces the serial run's bytes line for line.
#  3. **The quality lane's budget is disabled.** §3.1's contention hazard — `fanout_ms_per_pin`
#     being a *wall-clock* per-pin budget, so N concurrent routes lengthen each other's fanout and
#     a board that trips the limit under load routes differently — **cannot fire on a lane whose
#     budget is `disabled`** (`i32::MAX` per pin). This lane sets `FR_ROUTER_BUDGET=disabled`
#     already, for ruling AI's reasons, and W17 rides on that.
#
# Leg 3 is load-bearing, so the survey asks for it as **an assertion and not a comment**, and
# `QUALITY_LANE_BUDGET` below is it: it is the value the quality route is actually launched with,
# and `--jobs > 1` refuses to run unless it reads `disabled`. Anyone who later makes this lane run
# a live budget gets a refusal at the top of the script rather than a board that routes differently
# on a loaded machine. **The generators (`gen-*-reference.sh`) do not have this protection** and
# are survey item 3's separate problem — they run the *default* budget on purpose.
#
# Default: `min(4, cores / 2)`, floor 1. Half the machine, because the timing phase that follows
# wants a quiet one and because a stem's route is itself allowed to grow threads later; capped at 4
# because the win is bounded by the longest stem (~20 s) long before the core count is.
#
# ==================================================================================================
# 8. Environment
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
GATE_VERSION="g2"

# The `cpu_s` noise floor. `> 2x` and `> 20 %` are ruling BO's thresholds and are **not**
# tolerances; this is the floor beneath which a "slowdown" is the machine and not the code. A
# slowdown above it on an equal-quality change is a **rejection**, not a note.
#
# The floor is **per stem**, and it is the largest of four terms:
#
#     floor = max( CPU_NOISE_FLOOR,
#                  1 + prior_spread   / prior_cpu,     the baseline run's own measured spread
#                  1 + current_spread / prior_cpu,     THIS run's measured spread
#                  1 + CPU_EPSILON_S  / prior_cpu )    an absolute floor, for the fast stems
#
# 5 % is the global term and it is measured, not chosen: over the 29 stems of Task 0's dry run the
# largest median-of-3 spread was 0.556 s on a 10.996 s stem — **5.1 %** — and every other stem came
# in under 3.4 %. The two spread terms mean a stem that is genuinely noisy is judged against its
# own noise rather than the corpus's, and the *current* spread is in there because it is measured,
# sits in the same row, and is exactly as good evidence about this stem as the baseline's is.
#
# `CPU_EPSILON_S` is what stops a purely relative rule from being nonsense at the bottom of the
# range. Five of the 29 stems run in under 0.08 s and seven in under 0.35 s, where the measurement
# is process startup; on the review's own re-run `batch/router-ecc83-input` came in at 1.069x
# against a 1.06897x floor — a **2 ms** difference on a 29 ms stem, one floating-point hair from a
# REGRESSION and a non-zero exit. 5 ms is below the resolution at which a routing change is
# visible at all and well above the jitter of starting a process.
CPU_NOISE_FLOOR="1.05"
CPU_EPSILON_S="0.005"
# Ruling BO's two escalation thresholds.
CPU_STEM_ESCALATE="2.0"
CPU_CORPUS_ESCALATE="1.20"
# `normalized_score` is an `f32` and three of its terms are rounded (survey §9.1), so a bit-exact
# comparison would flag arithmetic noise as a regression. One part in 10^5 is well inside `f32`'s
# 7 significant digits and well below any score change a routing fix produces.
SCORE_NOISE="0.00001"

# The quality lane's budget, as a value rather than as a literal at the call site: the `--jobs`
# preflight below asserts on it (header §7, leg 3). `disabled` is ruling AI; nothing but Task 24
# may change it, and changing it to anything else makes `--jobs > 1` refuse rather than silently
# measure a board that routed differently because the machine was busy.
QUALITY_LANE_BUDGET="disabled"

# `min(4, cores / 2)`, floor 1 — header §7. `nproc` is coreutils and is not on a stock macOS;
# `sysctl -n hw.ncpu` is, and `getconf` is the last resort.
detect_cores() {
  local n=""
  if command -v nproc >/dev/null 2>&1; then
    n="$(nproc 2>/dev/null || true)"
  elif command -v sysctl >/dev/null 2>&1; then
    n="$(sysctl -n hw.ncpu 2>/dev/null || true)"
  fi
  [[ "$n" =~ ^[0-9]+$ ]] || n="$(getconf _NPROCESSORS_ONLN 2>/dev/null || true)"
  [[ "$n" =~ ^[0-9]+$ && "$n" -gt 0 ]] || n=1
  printf '%s' "$n"
}
default_jobs() {
  local cores half
  cores="$(detect_cores)"
  half=$(( cores / 2 ))
  [[ "$half" -lt 1 ]] && half=1
  [[ "$half" -gt 4 ]] && half=4
  printf '%s' "$half"
}

# Saved before the parse loop consumes it, for the `tee` re-exec below.
ORIGINAL_ARGV=("$@")

TASK=""
DRY_RUN=0
UPDATE_BASELINE=0
CLAIMS_CONNECTIVITY=0
REPEATS=3
JOBS=""
WANTED=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    --dry-run) DRY_RUN=1; shift ;;
    --update-baseline) UPDATE_BASELINE=1; shift ;;
    --claims-connectivity) CLAIMS_CONNECTIVITY=1; shift ;;
    --repeats) REPEATS="${2:?--repeats needs a count}"; shift 2 ;;
    --repeats=*) REPEATS="${1#--repeats=}"; shift ;;
    --jobs) JOBS="${2:?--jobs needs a count}"; shift 2 ;;
    --jobs=*) JOBS="${1#--jobs=}"; shift ;;
    -h|--help) sed -n '2,10p' "$0" >&2; exit 0 ;;
    --*) echo "error: unknown option $1" >&2; exit 1 ;;
    T[0-9]*) if [[ -z "$TASK" ]]; then TASK="$1"; else WANTED+=("$1"); fi; shift ;;
    *) WANTED+=("$1"); shift ;;
  esac
done
if [[ -z "$TASK" ]]; then
  echo "usage: $0 T<n> [--dry-run] [--repeats N] [--jobs N] [--claims-connectivity]" >&2
  echo "          [--update-baseline] [stem ...]" >&2
  exit 1
fi
[[ -n "$JOBS" ]] || JOBS="$(default_jobs)"
if ! [[ "$JOBS" =~ ^[0-9]+$ ]] || [[ "$JOBS" -lt 1 ]]; then
  echo "error: --jobs takes a positive integer, not '$JOBS'" >&2
  exit 1
fi
# Header §7, leg 3, as an assertion rather than a comment (survey §4.1's own instruction). The
# quality lane is parallelisable *because* its budget is disabled; a live `fanout_ms_per_pin` is a
# wall-clock per-pin budget and N concurrent routes would change each other's boards.
if [[ "$JOBS" -gt 1 && "$QUALITY_LANE_BUDGET" != "disabled" ]]; then
  echo "error: --jobs $JOBS asks for a parallel quality lane, but that lane's budget is" >&2
  echo "       '$QUALITY_LANE_BUDGET' and not 'disabled'. A live fanout_ms_per_pin is a" >&2
  echo "       wall-clock per-pin budget: concurrent routes lengthen each other's fanout and a" >&2
  echo "       board that trips the limit under load routes differently from one that did not" >&2
  echo "       (survey §3.1). Run with --jobs 1, or restore the disabled budget." >&2
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
# The port baseline is "the previous task's own tsv". Taken **literally** as `T<n-1>` that silently
# disables the whole quality comparison whenever a task lands out of numeric order: with the file
# absent, `read_tsv` answers an empty table, every row is scored `seed`, no rise is detected and no
# fall is detected — the gate quietly not gating, which is the exact failure mode `read_tsv`'s own
# `# cols:` note is written against.
#
# Task 8 is the standing case: it landed while Task 7 was still in flight, so `quality-ab-T7.tsv`
# does not exist and its first two runs compared against nothing at all. So the baseline is the
# **newest committed tsv at or below `n-1`**, and the header records which file that was. When
# `T<n-1>` exists this is bit-for-bit the old behaviour; it can only engage where the old code
# compared against nothing.
PREV_TSV=""
PREV_TSV_NOTE=""
if [[ "$TASK_N" =~ ^[0-9]+$ && "$TASK_N" -gt 0 ]]; then
  PREV_TSV="$AB_DIR/quality-ab-T$((TASK_N - 1)).tsv"
  if [[ ! -f "$PREV_TSV" ]]; then
    for (( n = TASK_N - 2; n >= 0; n-- )); do
      if [[ -f "$AB_DIR/quality-ab-T$n.tsv" ]]; then
        PREV_TSV="$AB_DIR/quality-ab-T$n.tsv"
        PREV_TSV_NOTE="  [T$((TASK_N - 1)) has not landed; this is the newest committed tsv at or below it]"
        break
      fi
    done
  fi
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

gate_version_of() {
  awk -F ': *' '$1 == "# gate-version" { print $2; exit }' "$1"
}

require_current_gate() {
  local path="$1"
  local description="$2"
  local version
  [[ -f "$path" ]] || return 0
  version="$(gate_version_of "$path")"
  if [[ -z "$version" ]]; then
    echo "error: $description ($path) carries no gate-version header." >&2
    exit 3
  fi
  if [[ "$version" != "$GATE_VERSION" ]]; then
    echo "error: $description ($path) carries gate-version $version and this run is $GATE_VERSION." >&2
    echo "       Re-cut the baseline before running this comparison." >&2
    exit 3
  fi
}

require_current_gate "$PREV_TSV" "the port baseline"
require_current_gate "$JAR_TSV" "the jar reference"
require_current_gate "$STEM_TIMES" "the rolling time baseline"

TIMEOUT_SECONDS="${QUALITY_AB_TIMEOUT:-3600}"
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
# The sha, and whether the binary that produced these numbers is actually *at* it. The five
# generators compute the same pair, and for the same reason: `port-sha` is the one provenance line
# the measurement spine has, and a bare HEAD sha on a dirty tree names a commit that does not
# contain the code that was measured. (Task 0's own first run is the standing example: it recorded
# the base sha, which does not carry `run_budget()` — at that sha `FR_ROUTER_BUDGET=disabled`
# is ignored and the quality lane would have run with the clock live.)
PORT_SHA="$(cd "$ROOT" && git rev-parse --short=12 HEAD 2>/dev/null || echo unknown)"
if ! (cd "$ROOT" && git diff --quiet HEAD -- crates 2>/dev/null); then
  PORT_SHA="$PORT_SHA +uncommitted-changes-under-crates"
fi

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
echo "== quality+referee lanes: $JOBS worker process(es); timing lane: sequential, $REPEATS repeats"
if [[ "$CLAIMS_CONNECTIVITY" -eq 1 ]]; then
  echo "== --claims-connectivity: the must-fall rule is ARMED for this task (ruling BZ)"
else
  echo "== --claims-connectivity not given: the must-fall rule is a NOTE for this task (ruling BZ)"
fi
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
  local t rc=0
  # The command's own output is redirected **inside** the braces, so what the substitution
  # captures is the timing line and nothing else. `rc` is carried out through a marker line
  # rather than lost to `|| true`: a repeat that failed or was killed by `timeout(1)` still burned
  # CPU, and folding that number into the median would quietly report a hung stem as a fast one.
  t="$( { TIMEFORMAT='%3U %3S'; time { "${TIMEOUT[@]}" "$@" > "$log" 2>&1 < /dev/null || echo "RC=$?"; } ; } 2>&1 )"
  if grep -q '^RC=' <<< "$t"; then
    printf 'FAILED'
    return 0
  fi
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

# `<ses>` -> the count of routed wire segments narrower than their own net's width, i.e. the
# **neckdown** count. Ruling BP15 owes this column to Task 14, and Task 2 is where it is first
# measured, because R2 (#294) is the fix that stops the micro-neckdown fanout fallback emitting
# sub-minimum traces.
#
# **The class width is read off the SES, not the DSN, and that is a deliberate limitation.** A
# DSN's `(rule (width W))` is in the file's own unit and the net-class table can override it per
# class and per layer, so recovering the board-unit class width outside the port means
# re-implementing `Structure.createBoard`'s scale factor and `NetClasses` in a shell script — a
# second, unvalidated reader of the same file. What this counts instead is self-contained and
# needs no unit arithmetic: for each net, the **widest** width that net uses in the routed SES is
# taken as its class width, and every wire narrower than it is one neckdown. On a net the router
# necked **everywhere** the count reads 0, which is the estimator's one blind spot and is named
# here rather than hidden; on every other net it is exact, because a neckdown is by construction
# narrower than the full-width segments beside it.
neckdown_of() {
  python3 - "$1" <<'PY'
import re, sys
from collections import defaultdict

text = open(sys.argv[1], encoding="utf-8", errors="replace").read()

# `(net <name> (wire (path <layer> <width> …) …) …)` — the SES writer emits one `net` scope per
# net and every `wire` inside it belongs to that net. Splitting on `(net ` is enough: no width or
# coordinate token can contain the literal, and a `wire` outside every `net` scope (which the
# writer never emits) is simply not counted rather than mis-attributed.
widths = defaultdict(list)
for chunk in text.split("(net ")[1:]:
    name = chunk.split(None, 1)[0].strip('"')
    for m in re.finditer(r"\(path\s+\S+\s+([0-9.eE+-]+)", chunk):
        try:
            widths[name].append(float(m.group(1)))
        except ValueError:
            pass

necked = 0
for values in widths.values():
    if not values:
        continue
    full = max(values)
    necked += sum(1 for v in values if v < full)
print(necked)
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

# The quality phase's hand-off to the timing phase (header §7, leg 2). One `.part` per stem —
# `metrics<TAB>neckdown`, written last and only on success, so its **presence** is the worker's
# success signal — and one `.msg`, anything the worker had to say, replayed by the timing phase in
# canonical order so a parallel run's console log reads exactly like a serial one's.
PARTS="$SCRATCH/parts"
mkdir -p "$PARTS"

# The referee's argv for a stem. Both phases need it (the DRC family times the referee run itself),
# and it is a pure function of the fixture row, so it is rebuilt rather than passed between them.
referee_argv_for() {
  local family="$1" stem="$2" board="$3" rules="$4" sesfile="$5"
  REFEREE_ARGV=(-de "$JAVA_DIR/$board")
  if [[ "$family" == drc ]]; then
    [[ -n "$sesfile" ]] && REFEREE_ARGV+=("$JAVA_DIR/$sesfile")
    [[ -n "$rules" ]] && REFEREE_ARGV+=(-dr "$JAVA_DIR/$rules")
  else
    REFEREE_ARGV+=("$SCRATCH/$family-$stem.ses")
  fi
  # Explicit, and load-bearing under `set -e`: the DRC branch's last statement is a `[[ … ]] && …`
  # that is legitimately false whenever a fixture's `rules` column is empty (`drc-issue593-ses`),
  # and a function whose last command is that test returns 1 and takes the whole script with it.
  return 0
}

# --------------------------------------------------------------------------------------------------
# Phase 1: the quality lane and the referee. **This is the phase `--jobs` fans out** (header §7).
# It runs in a worker process per stem, writes only to that stem's own paths, prints nothing, and
# never touches `$ROWS` or `$STATUS` — everything it has to report leaves through `.part`/`.msg`.
# --------------------------------------------------------------------------------------------------
quality_phase() {
  local family="$1" stem="$2" board="$3" key="$1/$2"
  local rules="$4" sesfile="$5" extra_args="$6"
  local ses="$SCRATCH/$family-$stem.ses"
  local manifest="$SCRATCH/$family-$stem.manifest.json"
  local report="$SCRATCH/$family-$stem.drc.json"
  local qlog="$SCRATCH/$family-$stem.quality.log"
  local rlog="$SCRATCH/$family-$stem.referee.log"
  local msg="$PARTS/$family-$stem.msg"
  local part="$PARTS/$family-$stem.part"
  : > "$msg"
  rm -f "$part"

  referee_argv_for "$family" "$stem" "$board" "$rules" "$sesfile"

  # --- the quality lane: the clock off (ruling AI) -------------------------------------------
  if [[ "$family" != drc ]]; then
    # shellcheck disable=SC2206  -- the fixture's extra_args is a space-separated argv fragment
    local extra=($extra_args)
    local -a route_argv=(-de "$JAVA_DIR/$board" -do "$ses" ${extra+"${extra[@]}"}
                         "--router.result_json=$manifest")
    if ! FR_ROUTER_BUDGET="$QUALITY_LANE_BUDGET" "${TIMEOUT[@]}" "$PORT_BIN" "${route_argv[@]}" \
        > "$qlog" 2>&1 < /dev/null || [[ ! -s "$ses" ]]; then
      echo "   FAILED: the quality run wrote no SES; see $qlog" >> "$msg"
      return 0
    fi
    if [[ ! -s "$manifest" ]]; then
      echo "   FAILED: the quality run wrote no manifest; see $qlog" >> "$msg"
      return 0
    fi
  else
    manifest="-"
  fi

  # --- the referee: the DRC document, and no fallback to the manifest ------------------------
  if ! run_referee "$report" "$rlog" "${REFEREE_ARGV[@]}"; then
    {
      echo "   FAILED: the referee DRC wrote no document for $key; see $rlog."
      echo "           This row has NO fallback: the manifest's own incomplete and violation"
      echo "           counts are not a legal input to this gate (survey §4.1)."
    } >> "$msg"
    return 0
  fi

  local metrics
  metrics="$(metrics_of "$report" "$manifest")"

  # The neckdown column (ruling BP15). A DRC stem routes nothing, so it has no SES of its own and
  # the cell is `-` rather than 0 — "not measured" and "measured zero" are different claims.
  local neckdown="-"
  if [[ "$family" != drc ]]; then
    neckdown="$(neckdown_of "$ses")"
  fi

  printf '%s\t%s\n' "$metrics" "$neckdown" > "$part"
  return 0
}

# --------------------------------------------------------------------------------------------------
# Phase 2: the printing, the timing lane and the row. **Strictly sequential, always** — `cpu_s` is
# a measurement and a measurement taken under contention is not one (header §7). It walks the stem
# table in its own fixed order, so the console log and `$ROWS` are `(family, stem)`-canonical at
# every `--jobs`.
# --------------------------------------------------------------------------------------------------
timing_phase() {
  local family="$1" stem="$2" board="$3" cap="$4" key="$1/$2"
  local rules="$5" sesfile="$6" extra_args="$7"
  local msg="$PARTS/$family-$stem.msg"
  local part="$PARTS/$family-$stem.part"

  printf '== %-38s cap=%s\n' "$key" "$cap"
  [[ -s "$msg" ]] && cat "$msg" >&2
  if [[ ! -f "$part" ]]; then
    if [[ ! -s "$msg" ]]; then
      echo "   FAILED: the quality phase produced no result for $key and said nothing — its" >&2
      echo "           worker died. See $SCRATCH/$family-$stem.*.log." >&2
    fi
    STATUS=1
    return 0
  fi

  # The worker wrote `metrics<TAB>neckdown` on one line; `neckdown` is the last field and the
  # metrics are everything before it. No field of either can contain a tab.
  local partline metrics neckdown
  partline="$(cat "$part")"
  neckdown="${partline##*$'\t'}"
  metrics="${partline%$'\t'*}"

  referee_argv_for "$family" "$stem" "$board" "$rules" "$sesfile"

  # --- the time lane: the budget in its normal configuration, median of N --------------------
  local times=() i
  for ((i = 0; i < REPEATS; i++)); do
    if [[ "$family" == drc ]]; then
      times+=("$(run_timed "$SCRATCH/$family-$stem.t$i.log" "$PORT_BIN" "${REFEREE_ARGV[@]}" \
          -drc "$SCRATCH/$family-$stem.t$i.json")")
    else
      # shellcheck disable=SC2206
      local time_extra=($extra_args)
      times+=("$(run_timed "$SCRATCH/$family-$stem.t$i.log" "$PORT_BIN" \
          -de "$JAVA_DIR/$board" -do "$SCRATCH/$family-$stem.t$i.ses" \
          ${time_extra+"${time_extra[@]}"})")
    fi
  done
  local failed=0 t
  for t in "${times[@]}"; do [[ "$t" == FAILED ]] && failed=1; done
  if [[ "$failed" -eq 1 ]]; then
    echo "   FAILED: a cpu_s repeat for $key exited non-zero or hit the ${TIMEOUT_SECONDS}s bound;" >&2
    echo "           its CPU time is not a measurement of this stem and is not folded into a" >&2
    echo "           median. See $SCRATCH/$family-$stem.t*.log." >&2
    STATUS=1
    return 0
  fi
  local cpu
  cpu="$(median_and_spread "${times[@]}")"

  printf '%s\t%s\t%s\t%s\t%s\t%s\n' "$family" "$stem" "$cap" "$metrics" "$neckdown" \
      "$cpu" >> "$ROWS"
  # shellcheck disable=SC2059
  printf '   incomplete=%s violations=%s clearance=%s hole_clearance=%s neckdown=%s cpu_s=%s (spread %s over %s runs)\n' \
      "$(cut -f1 <<< "$metrics")" "$(cut -f2 <<< "$metrics")" "$(cut -f3 <<< "$metrics")" \
      "$(cut -f4 <<< "$metrics")" "$neckdown" "$(cut -f1 <<< "$cpu")" "$(cut -f2 <<< "$cpu")" \
      "$REPEATS"
}

# `<family>|<stem>|<board>|<cap>|<rest>` -> the five positional arguments both phases take. The DRC
# family's `rest` is `rules|ses`; every other family's is a space-separated argv fragment.
split_row() {
  ROW_RULES=""; ROW_SES=""; ROW_EXTRA=""
  if [[ "$1" == drc ]]; then
    IFS='|' read -r ROW_RULES ROW_SES <<< "$2"
  else
    ROW_EXTRA="$2"
  fi
  return 0
}

# --- phase 1, fanned out over `$JOBS` worker processes ------------------------------------------
while IFS='|' read -r family stem board cap rest; do
  wanted "$family/$stem" || continue
  split_row "$family" "$rest"
  if [[ "$JOBS" -le 1 ]]; then
    quality_phase "$family" "$stem" "$board" "$ROW_RULES" "$ROW_SES" "$ROW_EXTRA"
  else
    # bash 3.2 has no `wait -n`, and this script must keep running on a stock macOS, so the
    # throttle polls. 50 ms against a stem that takes seconds is not a cost.
    while [[ "$(jobs -pr | wc -l | tr -d ' ')" -ge "$JOBS" ]]; do sleep 0.05; done
    quality_phase "$family" "$stem" "$board" "$ROW_RULES" "$ROW_SES" "$ROW_EXTRA" &
  fi
done < "$STEMS"
wait

# --- phase 2, sequential, in the stem table's own order -----------------------------------------
while IFS='|' read -r family stem board cap rest; do
  wanted "$family/$stem" || continue
  split_row "$family" "$rest"
  timing_phase "$family" "$stem" "$board" "$cap" "$ROW_RULES" "$ROW_SES" "$ROW_EXTRA"
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
    "$SCORE_NOISE" "$REPEATS" "$ROOT" "$CPU_EPSILON_S" "$CLAIMS_CONNECTIVITY" \
    "$PREV_TSV_NOTE" <<'PY'
import statistics, sys, os

(out_path, rows_path, gate, task, sha, prev_path, jar_path, times_path,
 noise_floor, stem_escalate, corpus_escalate, score_noise, repeats, root,
 cpu_epsilon, claims_connectivity, prev_note) = sys.argv[1:]

# Ruling BZ(a). The "must fall on at least one stem" half of G2's connectivity rule binds only a
# task that DECLARES a connectivity claim, because most tasks fix things that do not touch
# connectivity and for those bit-identical quality is the correct answer, not a gate failure.
# See the shell header's "The scope of `must fall on at least one`". Armed: a REGRESSION and a
# non-zero exit. Unarmed: a NOTE that changes nothing.
claims_connectivity = claims_connectivity == "1"


def rel(p):
    """Repo-relative, because this file is committed and an absolute path is one machine's."""
    if not p:
        return p
    return os.path.relpath(p, root) if p.startswith(root) else p


noise_floor = float(noise_floor); stem_escalate = float(stem_escalate)
corpus_escalate = float(corpus_escalate); score_noise = float(score_noise)
cpu_epsilon = float(cpu_epsilon)

# The 12 measured quality columns, then `cpu_s`/`cpu_spread_s`, then the **jar context columns**,
# then the flag.
#
# `neckdown_below_class_width` joined the list at Task 2 (ruling BP15, owed to Task 14): the count
# of routed wire segments narrower than their own net's widest, read off the port's own SES by
# `neckdown_of` in the shell half, which documents the estimator. It is **reported and is not a
# gate** — a neckdown is legitimate behaviour where the design rules allow one, and what R2
# (#294) fixes is the *sub-minimum* case the referee's DRC document already scores.
#
# The jar columns are the second of the two references brief item 6 requires every row to carry:
# the frozen HEAD numbers Task 1 derives once from the frozen references and writes to
# `benchmark/baselines/quality-baseline-java-head.tsv`. They are **context and never a gate** —
# nothing below reads them into `flag`, into a `REGRESSION:` line or into the exit code — and they
# carry **no timing**, because the frozen tree has none and a jar time on a different machine
# would be a number with no meaning. They exist so a reader looking at a row can see where the jar
# stood without opening a second file, which is the whole reason the brief asks for two references
# per row rather than two files.
#
# **They render `-` until that file exists**, and the `# jar-baseline:` header says which of those
# two worlds this tsv was written in. Nothing here ever affirms a reference it did not read.
QUALITY_COLUMNS = ["incomplete", "violations", "clearance_violations",
                   "hole_clearance_violations", "normalized_score", "trace_length_mm",
                   "via_total", "via_through", "via_blind", "via_buried", "bend_count",
                   "neckdown_below_class_width"]
JAR_COLUMNS = ["jar_incomplete", "jar_violations", "jar_clearance_violations",
               "jar_normalized_score"]
COLUMNS = (["family", "stem", "mp_cap"] + QUALITY_COLUMNS + ["cpu_s", "cpu_spread_s"]
           + JAR_COLUMNS + ["flag"])

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
    """A tsv written by this script: `# key: value` header lines, exactly one of which is
    `# cols: <tab-separated column names>`, then the rows.

    **`# cols:` is the ONLY column source, and its absence is a hard error.** The header used to
    take "the last `#` line that has no colon" as the column line, which worked only because the
    column line happened to be written last: adding one line of prose below it would have shifted
    every row one column and, worse, would have failed *silently* — no baseline found, every row
    scored `seed`, the gate quietly not gating. A named key cannot drift that way, and a file that
    exists but does not carry one is malformed rather than empty."""
    if not path or not os.path.exists(path):
        return None, {}
    headers, table = {}, {}
    cols = None
    for line in open(path, encoding="utf-8"):
        line = line.rstrip("\n")
        if line.startswith("#"):
            key, sep, value = line[1:].partition(":")
            if not sep:
                continue          # prose. Never a column source.
            key = key.strip()
            if key == "cols":
                cols = value.strip("\t").split("\t")
            else:
                headers[key] = value.strip()
        elif line and cols:
            rec = dict(zip(cols, line.split("\t")))
            table[(rec.get("family"), rec.get("stem"))] = rec
    if cols is None:
        sys.stderr.write(
            f"error: {path} exists but carries no `# cols:` header line, so its columns cannot\n"
            "       be identified and none of its rows can be read. Treating that as 'no\n"
            "       baseline' would make this gate pass by accident, so it is an error instead.\n"
            "       Re-cut the file with this script, or delete it if it is genuinely stale.\n")
        sys.exit(3)
    return headers, table


def check_gate(path, headers, what):
    if headers is None:
        return
    theirs = headers.get("gate-version")
    if theirs is None:
        sys.stderr.write(
            f"error: {what} ({path}) carries no gate-version header. Ruling BP8 makes the gate\n"
            "       version the precondition of every comparison, so an unversioned baseline is\n"
            "       one whose comparability nobody can establish — which is a refusal, not a\n"
            "       pass. Re-cut it with this script.\n")
        sys.exit(3)
    if theirs != gate:
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
    neckdown = r[14]
    cpu, spread = r[15], r[16]
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
        # `equal_quality` is a claim about the *quality* columns, so it can only be true when
        # there is a quality baseline to compare them against. A stem with a time baseline and no
        # quality baseline was previously judged by the strict equal-quality-not-slower rule with
        # nothing establishing that quality was equal; it now falls through to the escalation
        # thresholds, which is the honest reading of "we do not know whether this bought anything".
        equal_quality = base is not None
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
            # The per-stem floor: the largest of the corpus floor, the baseline's own measured
            # spread, THIS run's measured spread, and an absolute epsilon that keeps the rule
            # meaningful on a 30 ms stem. See CPU_NOISE_FLOOR's note in the shell half.
            try:
                current_spread = float(spread)
            except ValueError:
                current_spread = 0.0
            floor = max(
                noise_floor,
                1.0 + (prior_spread or 0.0) / prior_cpu,
                1.0 + current_spread / prior_cpu,
                1.0 + cpu_epsilon / prior_cpu,
            )
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
    # Context only. `jbase` is never consulted by any rule above, and a missing file or a missing
    # stem renders `-` rather than a zero — a jar number nobody measured is not a jar number.
    jbase = jar.get((family, stem))
    jar_cells = [jbase.get(name[len("jar_"):], "-") if jbase else "-" for name in JAR_COLUMNS]
    out.append([family, stem, cap] + [inc, viol, clear, hole, score, length, vt, vth, vbl, vbu,
                                      bends, neckdown] + [cpu, spread] + jar_cells + [flag])

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
    # The sha of the tree that was MEASURED. It is **not** in general an ancestor of the commit
    # that carries this file, and an earlier version of this line claimed it was "by
    # construction". It is not: the measurement has to finish before its tsv can be committed, and
    # the commit that then carries it is routinely `--amend`ed or rebased — which **rewrites** the
    # measured commit, leaving the sha recorded here pointing at a dangling object that is an
    # ancestor of nothing. Plan 9 Task 8 is the standing case: three of its A/B runs were stamped
    # with shas its own fix round and rebase rewrote away.
    #
    # What the field actually promises is narrower and is the useful thing: **this is the tree the
    # numbers came out of.** It may since have been rewritten; `git cat-file -p <sha>` finds it
    # while the object survives, and `+uncommitted-changes-under-crates` above is what says the
    # tree was not even that commit. What it must never do is name a commit that does *not*
    # contain the measured code.
    fh.write(f"# port-sha: {sha} (the tree the numbers came out of; it may since have been "
             f"rewritten by an amend or a rebase, so it is not necessarily an ancestor of the "
             f"commit carrying this file)\n")
    fh.write(f"# repeats: {repeats}\n")
    if not prev_path:
        fh.write("# port-baseline: (none — this run seeds the rolling baseline)\n")
    else:
        fh.write(f"# port-baseline: {rel(prev_path)}"
                 f"{'' if os.path.exists(prev_path) else '  [absent]'}{prev_note}\n")
    if os.path.exists(jar_path):
        fh.write(f"# jar-baseline: {rel(jar_path)}  [read; rendered in the jar_* columns as"
                 f" context, never a gate; {len(jar)} stem"
                 f"{'' if len(jar) == 1 else 's'}]\n")
    else:
        fh.write(f"# jar-baseline: {rel(jar_path)}  [absent — Task 1 writes it; the jar_* columns"
                 f" render `-`]\n")
    fh.write(f"# time-baseline: {rel(times_path)}"
             f"{'' if os.path.exists(times_path) else '  [absent — this run seeds it]'}\n")
    fh.write(f"# corpus-median-cpu-ratio: {corpus_note or '(no time baseline)'}\n")
    fh.write("# source-of-incomplete-and-violations: the referee's DRC document"
             " (`-de <board> <ses> -drc <report>`), NEVER the manifest (survey §4.1)\n")
    fh.write("# forbidden-inputs: traces.total_{vertical,horizontal,angled}_length (#195),"
             " board.bounding_box.width/height (#196)\n")
    fh.write("# quality-lane-budget: RouterBudget::disabled() (ruling AI);"
             " cpu_s lane: RouterBudget::default()\n")
    # Both branches cite the ruling, and the ARMED branch cites it too: BZ(a) is what defines
    # *both* halves of the scoping, not just the waiver. A reader of an armed tsv was previously
    # given no ruling reference at all, which made the armed state look like an ungoverned choice
    # by the task rather than the rule BZ(a) actually writes down.
    fh.write("# connectivity-claim: "
             + ("DECLARED (--claims-connectivity) — the must-fall rule was ARMED "
                "(ruling BZ(a): it binds a task that declares a connectivity claim)"
                if claims_connectivity else
                "not declared — the must-fall rule was a NOTE "
                "(ruling BZ(a): it binds only a task that declares a connectivity claim)")
             + "\n")
    fh.write("# cols:" + "\t".join(COLUMNS) + "\n")
    for row in out:
        fh.write("\t".join(str(c) for c in row) + "\n")

print()
print(f"wrote {rel(out_path)} ({len(out)} rows, gate-version {gate})")
if corpus_note:
    print(f"corpus-median cpu ratio against the previous task: {corpus_note}")
if prev and not fell_somewhere and not regressions:
    if claims_connectivity:
        regressions.append(
            "REGRESSION: no stem's incomplete count fell, and this run was invoked with "
            "--claims-connectivity. G2's rule is that incompletes must not rise on any stem AND "
            "must fall on at least one; a task whose fix list claims a connectivity win and shows "
            "no fall on 29 stems has not shown it (ruling BZ(a) scopes the rule to declaring "
            "tasks — this task declared).")
    else:
        print("NOTE: no stem's incomplete count fell. Ruling BZ(a) scopes G2's must-fall rule to "
              "tasks that DECLARE a connectivity claim (--claims-connectivity), and this run did "
              "not, so an unmoved incomplete column is a legitimate outcome and this line is a "
              "note rather than a gate. Incompletes RISING is still gated, on every task.")
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
    if line.startswith("# cols:"):
        cols = line[len("# cols:"):].strip("\t").split("\t")
    elif line and not line.startswith("#"):
        rows.append(dict(zip(cols, line.split("\t"))))
assert cols is not None, f"{tsv} carries no `# cols:` header line"

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
    fh.write("# cols:family\tstem\tcpu_s\tcpu_spread_s\n")
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
