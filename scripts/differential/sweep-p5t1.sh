#!/usr/bin/env bash
# Run the `p5t1` differential (the DRC report) over the whole Java fixture corpus plus the eight
# `tests/reference/drc-fixtures.txt` rows, and print a per-row MATCH/XDIFF/DIFF/SKIP table.
#
# Compiles both sides once through `run.sh p5t1` (which also smoke-runs the pair), then loops the
# built artifacts over every row, so the JVM and the Rust binary are launched per row but nothing
# is rebuilt.
#
# Usage: sweep-p5t1.sh
# Env:   FREEROUTING_JAVA_DIR, JAVA25_HOME, FREEROUTING_JAR, P5T_HASH_MODE (all as in run.sh)
#        SWEEP_OUT   directory for the raw outputs of a differing pair (default: a temp dir)
#
# Exit status is 0 when every row matches except the documented expected diffs below, 1 otherwise.
# See README.md's "Known, expected diffs".
set -uo pipefail

DIFF_ROOT="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$DIFF_ROOT/../.." && pwd)"
FREEROUTING_JAVA_DIR="${FREEROUTING_JAVA_DIR:-$ROOT/../freerouting}"
JAVA25_HOME="${JAVA25_HOME:-/opt/homebrew/opt/openjdk@25/libexec/openjdk.jdk/Contents/Home}"
FREEROUTING_JAR="${FREEROUTING_JAR:-$FREEROUTING_JAVA_DIR/build/libs/freerouting-current-executable.jar}"
JAVABIN="$JAVA25_HOME/bin/java"
CLASSES="$DIFF_ROOT/build/classes-p5t1"
RUST_BIN="$DIFF_ROOT/rust/target/release/p5t1"
SWEEP_OUT="${SWEEP_OUT:-$(mktemp -d "${TMPDIR:-/tmp}/p5t1-sweep.XXXXXX")}"
mkdir -p "$SWEEP_OUT"
export FREEROUTING_JAR

# `-XX:hashCode=2` and the two locale properties: see the long comment in `run.sh`. Overriding
# `P5T_HASH_MODE` is how a diff is proven Java-side — run the sweep under 0..4 and watch the row
# move. The uuid table below was measured under mode 2, so under any other mode a listed row is
# graded by shape only (see `expected_diff` for exactly what that relaxation is).
P5T_HASH_MODE="${P5T_HASH_MODE:-2}"
JAVA_FLAGS=(
  -Djava.awt.headless=true
  -Duser.language=en
  -Duser.country=US
  -XX:+UnlockExperimentalVMOptions
  "-XX:hashCode=$P5T_HASH_MODE"
)

# ------------------------------------------------------------------------------------------------
# The expected-diff table
# ------------------------------------------------------------------------------------------------
# `<file>:<variant>|java=<uuids>|rust=<uuids>`. `<variant>` is `dsn`, `rules` or `ses` — how the
# row's board was built — so excusing the plain `.dsn` run of a fixture does not excuse its
# `.rules` or `.ses` sibling. There is deliberately no wildcard: a match is load-bearing evidence
# and must be able to fail.
#
# Every entry is ONE class of divergence, plan-5 **ruling S** (quirk #146), and every entry is
# *checked* rather than waived: the row passes only if the Java document minus the listed
# `track_dangling` entries equals the port's document minus its listed ones, exactly — same uuids,
# nothing else moved. A regression that changed which dangling trace is dropped, or that moved
# anything outside `violations`, fails here.
#
# ## What the class is
#
# `generateReport` folds `getAllUnconnectedItems`' `track_dangling` entries into `violations`
# (DesignRulesChecker.java:271-276). The *candidate* set — every trace with a contact-free end
# (`:152-158`) — is hash-independent. The *emitted* set is the candidates minus whichever of them
# the dedup at `:160` drops, namely those that are some net entry's `firstItem`; and `firstItem` is
# `findRepresentativeItem` over a `HashSet<Item>` (`:138`, `:186-200`), which returns *a* Pin, or
# *a* Trace when the group holds no Pin — not a particular one. Ruling 3 fixes the port's choice at
# the lowest item id. So on any board with a Pin-free connected group holding several dangling
# traces, the port drops a different trace than a given JVM run does, and the two `violations`
# arrays differ by exactly those entries.
#
# ## The evidence (measured while writing Task 10; see the task report for the full table)
#
# Each of the 17 rows below was run against the HEAD jar under `-XX:hashCode=0,1,2,3,4`, and on
# every one of them **the Java side's own `track_dangling` set moves between modes** (2 to 5
# distinct sets over the five runs) — so no single JVM answer is "the" answer to port. Fifteen of
# the 17 also satisfy the ruling-T containment test against the five modes alone: every uuid the
# port emits is one some JVM run emitted. The two that do not —
# `Issue214-freerouting` and `Issue690-kit-dev-coldfire-xilinx_5213` — satisfy it against a larger
# sample: 45 runs (modes 0, 1, 4 and the default repeated ten times each, all four being PRNG- or
# address-seeded and therefore fresh on every run) give 7 and 28 distinct sets and unions of 94 and 384
# uuids, and the port's set is inside both.
#
# The `unconnectedItems` array, its `items` lists, all descriptions, all positions and every
# non-`track_dangling` violation are **byte-identical** on all 17 rows. Nothing else diffs anywhere
# in the corpus.
declare -a EXPECTED_DIFFS=(
  "Issue022-AutoRouter_interrupted.dsn:dsn|java=3614|rust=3615"
  "Issue070-Autorouter_FQ101_PCB_2022-05-13.dsn:dsn|java=648,719,760,795,813,853,861|rust=718,767,796,815,837,854,862"
  "Issue093-interf_u.dsn:dsn|java=1305|rust=1308"
  "Issue113-Protein.dsn:dsn|java=1665|rust=1674"
  "Issue157-TeamAdapt-LinePCB.dsn:dsn|java=2127,2140,2162,2172|rust=2131,2143,2163,2173"
  "Issue214-freerouting.dsn:dsn|java=2518,2778|rust=2519,2779,2934"
  "Issue269-caniot-tiny-arm.dsn:dsn|java=1280,1288|rust=1281,1289"
  "Issue269-z10_module.dsn:dsn|java=1931|rust="
  "Issue575-drc_Natural_Tone_Preamp_7_unconnected_items.dsn:dsn|java=1242,1696,1909|rust="
  "Issue690-kit-dev-coldfire-xilinx_5213.dsn:dsn|java=3121,3247,3258,5010,5012,5017,5019,5023,5027,5033,5035,5058,5061,5063,5068,5071,5150,5170,5186,5191,5193|rust=5009,5011,5018,5028,5032,5034,5062,5072,5075,5140,5151,5169,5171,5185,5190,5192,5194,5210"
  "Issue721-Autorouter_CE2632_HarryMu_2026-6-15.dsn:dsn|java=401|rust="
  "Issue733-kicad_interf_u_input_design.dsn:dsn|java=1229|rust=1232"
  "Issue756-tomu-fpga.dsn:dsn|java=1276|rust="
  "Issue756-tomu-fpga11.dsn:dsn|java=1341|rust=1346"
  "Issue756-tomu-fpga7.dsn:dsn|java=1342|rust=1347"
  "Issue756-tomu-fpga8.dsn:dsn|java=1342|rust=1347"
  "Issue756-tomu-fpga9.dsn:dsn|java=1341|rust=1346"
)

# ------------------------------------------------------------------------------------------------
# The rows
# ------------------------------------------------------------------------------------------------
# `<display name>|<variant>|<dsn>|<rules-or-empty>|<ses-or-empty>`. The delimiter is `|`, not a tab:
# `read` collapses runs of IFS *whitespace*, so a tab-separated line with an empty `rules` field
# would shift `ses` into `rules`.
#
# The corpus half is every `.dsn` in `$FREEROUTING_JAVA_DIR/fixtures`. Which of them the reader
# accepts is not this script's judgement: it comes from
# `crates/fr-dsn/tests/data/corpus-read-results.txt`, the golden of Plan 3's corpus read test
# (`every_fixture_in_the_corpus_matches_javas_result_and_warnings`), captured from the jar. A row
# whose variant is neither `Success` nor `OutlineMissing` is SKIPped and counted — today that is
# exactly `Issue006-LPC18XX_43XX_SCH.dsn`, an OLE compound document that both readers reject.
CORPUS_RESULTS="$ROOT/crates/fr-dsn/tests/data/corpus-read-results.txt"
DRC_FIXTURES_TXT="$ROOT/tests/reference/drc-fixtures.txt"

rows_file="$SWEEP_OUT/rows.txt"
skipped_file="$SWEEP_OUT/skipped.txt"
: > "$rows_file"
: > "$skipped_file"

while IFS= read -r line; do
  [[ "$line" == FIXTURE\ * ]] || continue
  # `FIXTURE <name, possibly with spaces> <variant> warnings=<n>`.
  rest="${line#FIXTURE }"
  rest="${rest% warnings=*}"
  variant="${rest##* }"
  name="${rest% *}"
  if [[ "$variant" != "Success" && "$variant" != "OutlineMissing" ]]; then
    printf '%s\t%s\n' "$name" "$variant" >> "$skipped_file"
    continue
  fi
  printf '%s|dsn|%s||\n' "$name" "$FREEROUTING_JAVA_DIR/fixtures/$name" >> "$rows_file"
done < "$CORPUS_RESULTS"

# The eight reference rows, which add the `.rules` path, the `.ses` path and the tutorial board —
# the three things the fixture directory alone does not cover. Four of them repeat a corpus row;
# the repeat costs a second or two and is left in so the table reads the same as
# `tests/reference/drc-fixtures.txt`.
while IFS='|' read -r stem dsn rules ses || [[ -n "$stem" ]]; do
  [[ -z "$stem" || "$stem" == \#* ]] && continue
  variant=dsn
  [[ -n "$rules" ]] && variant=rules
  [[ -n "$ses" ]] && variant=ses
  printf '%s|%s|%s|%s|%s\n' \
    "${dsn##*/}" "$variant" "$FREEROUTING_JAVA_DIR/$dsn" \
    "${rules:+$FREEROUTING_JAVA_DIR/$rules}" "${ses:+$FREEROUTING_JAVA_DIR/$ses}" >> "$rows_file"
done < "$DRC_FIXTURES_TXT"

# ------------------------------------------------------------------------------------------------
# The expected-diff checker
# ------------------------------------------------------------------------------------------------
# Arguments: <java out> <rust out> <java uuids, comma-separated> <rust uuids> <strict 0|1>.
#
# Strict (the default, `-XX:hashCode=2`): delete from each side the `track_dangling` violations
# whose single item carries a listed uuid, require the deleted sets to be *exactly* the listed
# ones, and require the two documents to be equal afterwards.
#
# Non-strict (any other hash mode): the uuids were measured under mode 2 and do not apply, so the
# check is by shape — the two documents must be equal once every `track_dangling` entry that is
# present on one side and absent from the other is deleted, and nothing else may differ. That is
# still enough to fail a regression outside this class; it is not enough to fail a regression
# inside it, which is why mode 2 is the default.
expected_diff() {
  python3 - "$@" <<'EOF'
import json, sys

out_j, out_r, want_j, want_r, strict = sys.argv[1:6]
strict = strict == "1"

def load(path):
    with open(path, encoding="utf-8") as fh:
        fh.readline()  # the HEADER line
        return json.load(fh)

def uuids(spec):
    return set(filter(None, spec.split(",")))

def dangling(doc):
    return set(v["items"][0]["uuid"] for v in doc["violations"]
               if v["type"] == "track_dangling" and len(v["items"]) == 1)

java, rust = load(out_j), load(out_r)
if strict:
    drop_j, drop_r = uuids(want_j), uuids(want_r)
else:
    dj, dr = dangling(java), dangling(rust)
    drop_j, drop_r = dj - dr, dr - dj

def strip(doc, drop):
    kept, removed = [], set()
    for v in doc["violations"]:
        if (v["type"] == "track_dangling" and len(v["items"]) == 1
                and v["items"][0]["uuid"] in drop):
            removed.add(v["items"][0]["uuid"])
        else:
            kept.append(v)
    doc["violations"] = kept
    return removed

got_j, got_r = strip(java, drop_j), strip(rust, drop_r)
if got_j != drop_j or got_r != drop_r:
    sys.exit("expected to remove java=%s rust=%s, removed java=%s rust=%s"
             % (sorted(drop_j), sorted(drop_r), sorted(got_j), sorted(got_r)))
if java != rust:
    sys.exit("the documents still differ outside the listed track_dangling entries")
EOF
}

# ------------------------------------------------------------------------------------------------
# The sweep
# ------------------------------------------------------------------------------------------------
echo "== compiling both sides (run.sh p5t1) =="
"$DIFF_ROOT/run.sh" p5t1 >/dev/null || { echo "smoke run failed" >&2; exit 1; }

strict=0
[[ "$P5T_HASH_MODE" == "2" ]] && strict=1
[[ "$strict" == "0" ]] && echo "note: -XX:hashCode=$P5T_HASH_MODE — the uuid table is mode 2's, grading listed rows by shape"

printf '%-62s %-6s %s\n' "fixture" "row" "result"
fail=0
expected=0
total=0
start=$SECONDS
while IFS='|' read -r name variant dsn rules ses; do
  total=$((total + 1))
  key="$name:$variant"
  want_j=""
  want_r=""
  is_expected=0
  for e in "${EXPECTED_DIFFS[@]}"; do
    if [[ "${e%%|*}" == "$key" ]]; then
      is_expected=1
      spec="${e#*|}"                 # java=…|rust=…
      want_j="${spec%%|*}"; want_j="${want_j#java=}"
      want_r="${spec#*|}";  want_r="${want_r#rust=}"
    fi
  done

  j="$SWEEP_OUT/$name.$variant.j"
  r="$SWEEP_OUT/$name.$variant.r"
  # Java's stderr goes to a side file rather than /dev/null: an uncaught exception's stack trace is
  # the only thing that distinguishes "Java crashed" from "Java disagreed".
  "$JAVABIN" "${JAVA_FLAGS[@]}" -cp "$CLASSES:$FREEROUTING_JAR" app.freerouting.drc.P5T1 \
    "$dsn" "${rules:--}" "${ses:--}" >"$j" 2>"$j.err"
  j_rc=$?
  "$RUST_BIN" "$dsn" "${rules:--}" "${ses:--}" >"$r" 2>"$r.err"
  r_rc=$?

  if [[ "$j_rc" -eq 0 && "$r_rc" -eq 0 ]] && diff -q "$j" "$r" >/dev/null 2>&1; then
    result="MATCH"
    rm -f "$j" "$r" "$j.err" "$r.err"
  elif [[ "$is_expected" -eq 1 && "$j_rc" -eq 0 && "$r_rc" -eq 0 ]] \
      && expected_diff "$j" "$r" "$want_j" "$want_r" "$strict" \
           >"$SWEEP_OUT/$name.$variant.check" 2>&1; then
    result="XDIFF"
    expected=$((expected + 1))
    echo "XDIFF $key: java exit=$j_rc rust exit=$r_rc (ruling S: track_dangling java=[$want_j] rust=[$want_r])" \
      >>"$SWEEP_OUT/exit-codes.txt"
  else
    result="DIFF"
    echo "DIFF  $key: java exit=$j_rc rust exit=$r_rc" >>"$SWEEP_OUT/exit-codes.txt"
    if [[ -s "$SWEEP_OUT/$name.$variant.check" ]]; then
      sed 's/^/        /' "$SWEEP_OUT/$name.$variant.check" >>"$SWEEP_OUT/exit-codes.txt"
    fi
    fail=$((fail + 1))
  fi
  printf '%-62s %-6s %s\n' "$name" "$variant" "$result"
done < "$rows_file"

skipped=$(wc -l < "$skipped_file" | tr -d ' ')
echo
echo "rows: $total   unexpected diffs: $fail   expected diffs (XDIFF): $expected   skipped: $skipped"
echo "hash mode: -XX:hashCode=$P5T_HASH_MODE   wall clock: $((SECONDS - start)) s"
if [[ "$skipped" -gt 0 ]]; then
  echo "skipped (the reader does not report Success — see $CORPUS_RESULTS):"
  sed 's/^/  /' "$skipped_file"
fi
if [[ -f "$SWEEP_OUT/exit-codes.txt" ]]; then
  echo "expected and unexpected diffs ($SWEEP_OUT/exit-codes.txt):"
  sed 's/^/  /' "$SWEEP_OUT/exit-codes.txt"
fi
if [[ "$fail" -gt 0 ]]; then
  echo "raw outputs (and Java's stderr, in the matching .err files) are in $SWEEP_OUT"
  exit 1
fi
[[ "$expected" -gt 0 ]] && echo "XDIFF outputs kept in $SWEEP_OUT"
exit 0
