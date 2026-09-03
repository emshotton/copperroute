#!/usr/bin/env bash
# Rebuild the frozen `results/java-278fe14/` view — Plan 9 Task 0, ruling BP3.
#
#   benchmark/scripts/make-java-view.sh <source-run-dir> [--run-id java-278fe14]
#
# ## Why this script has to exist
#
# `benchmark/results/` is **gitignored**. `java-278fe14` is the Java run of record — 605 PCBench
# boards, `seeds 1`, `-mp 10`, `timeout 300`, `threads 1`, `jobs 12`, taken on `workbench` against
# jar sha `278fe14123c4` — and every Plan 9 milestone compares against it:
#
#   uv run bench compare --baseline java-current --against rs-main \
#     --runs java-278fe14,plan9-m<n> --tier pcbench --out plan9-m<n>-vs-java-278fe14
#
# Ruling BN forbids re-running `java-current`: it costs ~2h10 and a fresh run would compare the
# port against a *different* Java measurement than `reports/v1.0.0-vs-java-clean.json` — the
# baseline of record — used. So the view must survive a `results/` that a clean checkout does not
# have, and this script is how it is rebuilt. **The comparison it must be able to reproduce is
# `reports/v1.0.0-vs-java-clean.json`** (`runs: ["java-278fe14","v1.0.0-rs"]`, `baseline:
# java-current`, `against: ["rs-main"]`, `time_metric: "cpu_s"`).
#
# ## What it does, and why that is the whole mechanism
#
# `java-278fe14` is a **view** onto a two-candidate run, not a run of its own. It is built in two
# steps from the source run directory (the two-candidate `full-rs-vs-java` run):
#
#  1. `meta.json` is the source's `meta.json` with `run_id` rewritten and the **`candidates` array
#     filtered down to the single `java-current` entry**. That filter *is* the trick.
#     `bench/compare.py::collect` walks `meta["candidates"]` and records `sha`/`version` for every
#     name it was asked about, so a `candidates` array that still listed `rs-main` would hand the
#     compare the **frozen run's** rs-main sha for the *new* run's `rs-main` — a silent
#     sha-mismatch warning at best, and the wrong binary named in the report at worst.
#  2. `java-current` is a **symlink** at the source run's `java-current` cell directory. The
#     `cells` list is left exactly as it is, all 1210 entries, and that is deliberate: `collect`
#     also filters cells by candidate name, and for the `rs-main` half it then looks for
#     `results/java-278fe14/rs-main/<board>/seed-1/metrics.json`, which does not exist because
#     this view has no such symlink. A missing `metrics.json` is skipped, so the rs-main cells
#     cost nothing and rewriting them out would only add a way to get the file wrong.
#
# The script is **idempotent** — run it twice and the second run is a no-op — and it **verifies**
# afterwards, because a view that is silently wrong is worse than no view: a milestone would
# compare against a corpus of a different size or a jar of a different sha and report a verdict
# nobody could reproduce.
#
# ## Usage
#
#   cd benchmark
#   scripts/make-java-view.sh results/full-rs-vs-java
#
# The source run must be one that actually contains a `java-current` candidate and its cells.
set -euo pipefail

HERE="$(cd "$(dirname "$0")/.." && pwd)"
RESULTS="$HERE/results"

# The identity this view is frozen at. The verification step at the bottom asserts **ten** things
# about the rebuilt view: the candidate list, its sha, the board count, `seeds`, `max_passes`,
# `timeout_s`, `threads`, `jobs`, `tier`, and — the one that catches a `meta.json` promising boards
# that are not on disk — the count of cells whose `metrics.json` is readable through the symlink.
EXPECT_RUN_ID="java-278fe14"
EXPECT_SHA="278fe14123c4"
EXPECT_BOARDS=605
EXPECT_CANDIDATE="java-current"
# `jobs 12` is part of the run identity `bench compare` checks for compatibility
# (`compare.py::_check_config` keys on threads/jobs/max_passes/timeout_s), so it is asserted here
# rather than only named in this header.
EXPECT_JOBS=12

SOURCE=""
RUN_ID="$EXPECT_RUN_ID"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --run-id) RUN_ID="${2:?--run-id needs a value}"; shift 2 ;;
    --run-id=*) RUN_ID="${1#--run-id=}"; shift ;;
    -h|--help) sed -n '2,8p' "$0" >&2; exit 0 ;;
    --*) echo "error: unknown option $1" >&2; exit 1 ;;
    *) SOURCE="$1"; shift ;;
  esac
done

if [[ -z "$SOURCE" ]]; then
  echo "usage: $0 <source-run-dir> [--run-id $EXPECT_RUN_ID]" >&2
  echo "       e.g. $0 results/full-rs-vs-java" >&2
  exit 1
fi
# Relative paths are resolved against `benchmark/`, so `results/full-rs-vs-java` works from either
# `benchmark/` or `benchmark/scripts/`.
[[ "$SOURCE" == /* ]] || SOURCE="$HERE/$SOURCE"
if [[ ! -d "$SOURCE" ]]; then
  echo "error: $SOURCE is not a directory" >&2
  echo "       give this script the bench RUN directory that holds the jar's cells, e.g." >&2
  echo "       $0 results/full-rs-vs-java" >&2
  echo "       (benchmark/results/ is gitignored, so a clean checkout has none until a run is" >&2
  echo "        copied in — that is exactly the situation this script exists for.)" >&2
  exit 1
fi
SOURCE="$(cd "$SOURCE" && pwd)"

if [[ ! -f "$SOURCE/meta.json" ]]; then
  echo "error: $SOURCE has no meta.json — that is not a bench run directory" >&2
  exit 1
fi
if [[ ! -d "$SOURCE/$EXPECT_CANDIDATE" ]]; then
  echo "error: $SOURCE has no $EXPECT_CANDIDATE/ cell directory; this view cannot be built" >&2
  echo "       from a run that did not measure the jar" >&2
  exit 1
fi

VIEW="$RESULTS/$RUN_ID"
mkdir -p "$VIEW"

echo "== source  $SOURCE"
echo "== view    $VIEW"

# ---- 1. meta.json: rewrite run_id, filter candidates to the one name ------------------------------
python3 - "$SOURCE/meta.json" "$VIEW/meta.json" "$RUN_ID" "$EXPECT_CANDIDATE" <<'PY'
import json, sys

src, dst, run_id, keep = sys.argv[1:]
meta = json.loads(open(src, encoding="utf-8").read())

names = [c["name"] for c in meta["candidates"]]
if keep not in names:
    sys.exit(f"error: {src} has candidates {names} and none of them is {keep!r}")

meta["run_id"] = run_id
# The whole mechanism, in one line: compare.collect() walks meta["candidates"] and takes only the
# names it was asked for, so filtering this array is what makes the view a JAVA-ONLY run.
meta["candidates"] = [c for c in meta["candidates"] if c["name"] == keep]
# `cells` is deliberately NOT filtered -- see this script's header.

with open(dst, "w", encoding="utf-8") as fh:
    json.dump(meta, fh, indent=2)
    fh.write("\n")
print(f"   meta.json: run_id={run_id}, candidates={[c['name'] for c in meta['candidates']]}, "
       f"cells={len(meta['cells'])} (unfiltered)")
PY

# ---- 2. the cell directory, as a symlink ---------------------------------------------------------
# `ln -sfn` and not `ln -sf`: with an existing *symlink to a directory* as the target, plain
# `ln -sf` creates the new link INSIDE it rather than replacing it, so a second run would leave
# `java-current/java-current`. `-n` treats the existing link as a file and replaces it, which is
# what makes this script idempotent.
if [[ -e "$VIEW/$EXPECT_CANDIDATE" && ! -L "$VIEW/$EXPECT_CANDIDATE" ]]; then
  echo "error: $VIEW/$EXPECT_CANDIDATE exists and is not a symlink; refusing to replace real" >&2
  echo "       result data. Move it aside first if it is genuinely stale." >&2
  exit 1
fi
ln -sfn "$SOURCE/$EXPECT_CANDIDATE" "$VIEW/$EXPECT_CANDIDATE"
echo "   $EXPECT_CANDIDATE -> $(readlink "$VIEW/$EXPECT_CANDIDATE")"

# ---- 3. verify ------------------------------------------------------------------------------------
python3 - "$VIEW" "$EXPECT_SHA" "$EXPECT_BOARDS" "$EXPECT_CANDIDATE" "$EXPECT_JOBS" <<'PY'
import json, os, sys

view, expect_sha, expect_boards, candidate, expect_jobs = sys.argv[1:]
expect_boards = int(expect_boards)
expect_jobs = int(expect_jobs)
meta = json.loads(open(os.path.join(view, "meta.json"), encoding="utf-8").read())
args = meta["args"]

problems = []


def check(label, got, want):
    mark = "ok " if got == want else "BAD"
    print(f"   [{mark}] {label:<22} {got!r}" + ("" if got == want else f"   (want {want!r})"))
    if got != want:
        problems.append(f"{label}: {got!r} != {want!r}")


cands = meta["candidates"]
check("candidates", [c["name"] for c in cands], [candidate])
check("candidate sha", cands[0]["sha"] if cands else None, expect_sha)
check("boards", len(args["boards"]), expect_boards)
check("seeds", args.get("seeds"), 1)
check("max_passes", args.get("max_passes"), 10)
check("timeout_s", args.get("timeout_s"), 300)
check("threads", args.get("threads"), 1)
check("jobs", args.get("jobs"), expect_jobs)
check("tier", args.get("tier"), "pcbench")

# The cells the compare will actually read: one per board for the kept candidate, each with a
# metrics.json behind the symlink. This is the check that catches a view pointed at a run whose
# cell directory is incomplete -- a meta.json can promise 605 boards that are not on disk.
present = 0
for e in meta["cells"]:
    if e["candidate"] != candidate:
        continue
    p = os.path.join(view, candidate, e["board"], f"seed-{e['seed']}", "metrics.json")
    if os.path.exists(p):
        present += 1
check("readable cells", present, expect_boards)

if problems:
    print()
    print("the view is NOT the frozen java-278fe14 run of record:")
    for p in problems:
        print(f"  - {p}")
    print("Every Plan 9 milestone compares against this view; a wrong one would produce a "
          "verdict nobody can reproduce. Fix the source run or the arguments and re-run.")
    sys.exit(1)
print()
print("the view reproduces the frozen java-278fe14 run of record "
      "(reports/v1.0.0-vs-java-clean.json's baseline)")
PY
