#!/usr/bin/env bash
# Run the `p8t5` differential **row by row** and print a per-row MATCH/XDIFF/SKIP table.
#
# `run.sh p8t5` compares the two whole transcripts, which answers "is the legacy parse still
# right?" with one bit. This script answers "which argv shape moved?", which is what a reviewer
# needs when it is not right — and it is the acceptance gate the task brief names.
#
# The shape is `sweep-p7t9.sh`'s: compile and build both sides once by way of `run.sh p8t5`
# (which also smoke-runs the pair), then split the two transcripts on their `[row] <label>`
# headers and diff row against row. Nothing is rebuilt per row and neither side is re-launched,
# because every row of this driver is a pure argv walk with no clock, no board and no filesystem.
#
# Usage: sweep-p8t5.sh [row-label ...]
#   With no arguments, every row of `matrix/p8t5-argv.tsv` is swept. With arguments, only the rows
#   whose label matches one of them.
# Env:  FREEROUTING_JAVA_DIR, JAVA25_HOME, FREEROUTING_JAR (all as in run.sh)
#       P8T5_MATRIX   the argv table (default: matrix/p8t5-argv.tsv)
#       SWEEP_OUT     directory for the per-row outputs (default: a temp dir, kept on failure)
#
# ## The three verdicts
#
#   MATCH   the row's block is byte-identical on both sides.
#   XDIFF   the row's block differs **and** the row is named in EXPECTED_XDIFF below, with the
#           ruling that authorises it. There are none today: the driver matches on every row.
#   SKIP    the row is in the matrix but absent from one side's transcript — which can only happen
#           if the two halves disagree about how to read the table, and is therefore a failure of
#           the harness rather than of the port. Acceptance is **zero SKIP**.
#
# Exit status is 0 when every row is MATCH or XDIFF and no row is SKIP; 1 otherwise.
set -uo pipefail

DIFF_ROOT="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$DIFF_ROOT/../.." && pwd)"
MATRIX="${P8T5_MATRIX:-$DIFF_ROOT/matrix/p8t5-argv.tsv}"
SWEEP_OUT="${SWEEP_OUT:-$(mktemp -d "${TMPDIR:-/tmp}/p8t5-sweep.XXXXXX")}"
mkdir -p "$SWEEP_OUT"

# Rows whose divergence is known, with the ruling that authorises it. One entry per line,
# `<label> <reason>`. Empty today — every row matches — and a row added here must cite its ruling
# or its `docs/java-quirks.md` id, never merely "expected".
EXPECTED_XDIFF=""

echo "== building both sides (run.sh p8t5) =="
if ! "$DIFF_ROOT/run.sh" p8t5 "$MATRIX" >"$SWEEP_OUT/run.log" 2>&1; then
  # A whole-transcript DIFF is not fatal here: the point of the sweep is to say *which* rows.
  echo "note: run.sh p8t5 reported a difference; see $SWEEP_OUT/run.log"
fi

J_OUT="$DIFF_ROOT/build/p8t5.j.out"
R_OUT="$DIFF_ROOT/build/p8t5.r.out"
for f in "$J_OUT" "$R_OUT"; do
  if [[ ! -s "$f" ]]; then
    echo "error: $f is missing or empty — see $SWEEP_OUT/run.log" >&2
    exit 1
  fi
done

# Split a transcript into one file per row, named by the row label.
split_rows() {
  local src="$1" dest="$2"
  mkdir -p "$dest"
  awk -v dest="$dest" '
    /^#/ { next }
    # `[row] ` is six characters, so the label starts at column 7.
    /^\[row\] / { label = substr($0, 7); file = dest "/" label ".txt"; }
    { if (file != "") print > file }
  ' "$src"
}

split_rows "$J_OUT" "$SWEEP_OUT/java"
split_rows "$R_OUT" "$SWEEP_OUT/rust"

# The labels to sweep: every non-comment row of the matrix, or the ones named on the command line.
mapfile -t all_labels < <(awk -F'\t' '/^#/ || NF == 0 { next } { print $1 }' "$MATRIX")
if [[ $# -gt 0 ]]; then
  labels=("$@")
else
  labels=("${all_labels[@]}")
fi

match=0
xdiff=0
skip=0
diff_count=0

printf '%-32s %s\n' "ROW" "VERDICT"
printf '%-32s %s\n' "--------------------------------" "-------"
for label in "${labels[@]}"; do
  j="$SWEEP_OUT/java/$label.txt"
  r="$SWEEP_OUT/rust/$label.txt"
  if [[ ! -f "$j" || ! -f "$r" ]]; then
    printf '%-32s %s\n' "$label" "SKIP (absent from one transcript)"
    skip=$((skip + 1))
    continue
  fi
  if diff -q "$j" "$r" >/dev/null; then
    printf '%-32s %s\n' "$label" "MATCH"
    match=$((match + 1))
    continue
  fi
  reason="$(printf '%s\n' "$EXPECTED_XDIFF" | awk -v l="$label" '$1 == l { $1 = ""; sub(/^ /, ""); print }')"
  if [[ -n "$reason" ]]; then
    printf '%-32s %s\n' "$label" "XDIFF ($reason)"
    xdiff=$((xdiff + 1))
  else
    printf '%-32s %s\n' "$label" "DIFF"
    diff "$j" "$r" | sed 's/^/    /'
    diff_count=$((diff_count + 1))
  fi
done

echo
echo "rows: ${#labels[@]}  MATCH: $match  XDIFF: $xdiff  DIFF: $diff_count  SKIP: $skip"
echo "per-row outputs: $SWEEP_OUT"

if [[ "$diff_count" -eq 0 && "$skip" -eq 0 ]]; then
  rm -rf "$SWEEP_OUT"
  exit 0
fi
exit 1
