#!/usr/bin/env bash
# Reconstruct the historical java-278fe14 result view from an existing source run.
# Usage: scripts/make-java-view.sh <source-run-dir> [--run-id java-278fe14]
# This is an archive utility; current comparisons should measure a pinned baseline.
set -euo pipefail

HERE="$(cd "$(dirname "$0")/.." && pwd)"
RESULTS="$HERE/results"

EXPECT_RUN_ID="java-278fe14"
EXPECT_SHA="278fe14123c4"
EXPECT_BOARDS=605
EXPECT_CANDIDATE="java-current"
EXPECT_JOBS=12

SOURCE=""
RUN_ID="$EXPECT_RUN_ID"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --run-id) RUN_ID="${2:?--run-id needs a value}"; shift 2 ;;
    --run-id=*) RUN_ID="${1#--run-id=}"; shift ;;
    -h|--help) sed -n '2,4p' "$0" >&2; exit 0 ;;
    --*) echo "error: unknown option $1" >&2; exit 1 ;;
    *) SOURCE="$1"; shift ;;
  esac
done

if [[ -z "$SOURCE" ]]; then
  echo "usage: $0 <source-run-dir> [--run-id $EXPECT_RUN_ID]" >&2
  echo "       e.g. $0 results/full-rs-vs-java" >&2
  exit 1
fi
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

python3 - "$SOURCE/meta.json" "$VIEW/meta.json" "$RUN_ID" "$EXPECT_CANDIDATE" <<'PY'
import json, sys

src, dst, run_id, keep = sys.argv[1:]
meta = json.loads(open(src, encoding="utf-8").read())

names = [c["name"] for c in meta["candidates"]]
if keep not in names:
    sys.exit(f"error: {src} has candidates {names} and none of them is {keep!r}")

meta["run_id"] = run_id
meta["candidates"] = [c for c in meta["candidates"] if c["name"] == keep]
meta["cells"] = [c for c in meta["cells"] if c["candidate"] == keep]

with open(dst, "w", encoding="utf-8") as fh:
    json.dump(meta, fh, indent=2)
    fh.write("\n")
print(f"   meta.json: run_id={run_id}, candidates={[c['name'] for c in meta['candidates']]}, "
       f"cells={len(meta['cells'])} (filtered)")
PY

if [[ -e "$VIEW/$EXPECT_CANDIDATE" && ! -L "$VIEW/$EXPECT_CANDIDATE" ]]; then
  echo "error: $VIEW/$EXPECT_CANDIDATE exists and is not a symlink; refusing to replace real" >&2
  echo "       result data. Move it aside first if it is genuinely stale." >&2
  exit 1
fi
# -n replaces an existing directory symlink instead of creating a link inside it.
ln -sfn "$SOURCE/$EXPECT_CANDIDATE" "$VIEW/$EXPECT_CANDIDATE"
echo "   $EXPECT_CANDIDATE -> $(readlink "$VIEW/$EXPECT_CANDIDATE")"

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
    print("Fix the source run or the arguments and re-run.")
    sys.exit(1)
print()
print("the view reproduces the frozen java-278fe14 run of record "
      "(reports/v1.0.0-vs-java-clean.json's baseline)")
PY
