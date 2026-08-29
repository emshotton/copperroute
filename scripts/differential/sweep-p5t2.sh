#!/usr/bin/env bash
# Run the `p5t2` differential (the algorithm-level DRC lists) over the whole Java fixture corpus
# plus the eight `tests/reference/drc-fixtures.txt` rows, in every requested mode, and print a
# per-row MATCH/XDIFF/DIFF/SKIP table.
#
# Compiles both sides once through `run.sh p5t2`, then loops the built artifacts, so the JVM and
# the Rust binary are launched per (row, mode) but nothing is rebuilt.
#
# Usage: sweep-p5t2.sh [mode ...]      # default: 0 1 2 3 4
# Env:   FREEROUTING_JAVA_DIR, JAVA25_HOME, FREEROUTING_JAR, P5T_HASH_MODE (all as in run.sh)
#        SWEEP_OUT   directory for the raw outputs of a differing pair (default: a temp dir)
#        P5T2_UNION=1  in mode 2, also build a six-hash-mode JVM airline sample per differing row
#                      and print how many port airlines fall outside it (informational, five extra
#                      JVM launches per differing row)
#
# Exit status is 0 when every (row, mode) matches except the documented expected diffs below, 1
# otherwise. See README.md's "Known, expected diffs".
set -uo pipefail

DIFF_ROOT="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$DIFF_ROOT/../.." && pwd)"
FREEROUTING_JAVA_DIR="${FREEROUTING_JAVA_DIR:-$ROOT/../freerouting}"
JAVA25_HOME="${JAVA25_HOME:-/opt/homebrew/opt/openjdk@25/libexec/openjdk.jdk/Contents/Home}"
FREEROUTING_JAR="${FREEROUTING_JAR:-$FREEROUTING_JAVA_DIR/build/libs/freerouting-current-executable.jar}"
JAVABIN="$JAVA25_HOME/bin/java"
CLASSES="$DIFF_ROOT/build/classes-p5t2"
RUST_BIN="$DIFF_ROOT/rust/target/release/p5t2"
UNION_DIR="$ROOT/crates/fr-drc/tests/data"
SWEEP_OUT="${SWEEP_OUT:-$(mktemp -d "${TMPDIR:-/tmp}/p5t2-sweep.XXXXXX")}"
mkdir -p "$SWEEP_OUT"
export FREEROUTING_JAR

P5T_HASH_MODE="${P5T_HASH_MODE:-2}"
java_flags_for() {
  JAVA_FLAGS=(
    -Djava.awt.headless=true
    -Duser.language=en
    -Duser.country=US
    -XX:+UnlockExperimentalVMOptions
    "-XX:hashCode=$1"
  )
}

modes=("$@")
[[ ${#modes[@]} -gt 0 ]] || modes=(0 1 2 3 4)

# ------------------------------------------------------------------------------------------------
# The expected-diff table
# ------------------------------------------------------------------------------------------------
# `<file>:<variant>:<mode>|java=<ids>|rust=<ids>`, and every entry names ONE mode: excusing a
# fixture's mode 1 must not excuse its mode 0, because mode 0 matching everywhere is the evidence
# that the class below is confined to the dangling-trace phase.
#
# There is exactly one class here, and it is `sweep-p5t1.sh`'s: plan-5 ruling S / quirk #146, the
# hash-ordered `findRepresentativeItem` choice that decides which dangling trace the dedup at
# `DesignRulesChecker.java:160` drops. `sweep-p5t1.sh`'s header carries the full description and the
# `-XX:hashCode=0..4` evidence; the ids below are the same 17 fixtures' same items, seen one layer
# lower as `U track_dangling` lines instead of as `violations` entries. **Mode 0 has no entries at
# all** — the clearance list is hash-independent and matches on all 112 rows — and **mode 2's
# counters have none either**; only its `AL` block diverges, and that is graded separately (see
# below), not from this table.
declare -a EXPECTED_DIFFS=(
  "Issue022-AutoRouter_interrupted.dsn:dsn:1|java=3614|rust=3615"
  "Issue070-Autorouter_FQ101_PCB_2022-05-13.dsn:dsn:1|java=648,719,760,795,813,853,861|rust=718,767,796,815,837,854,862"
  "Issue093-interf_u.dsn:dsn:1|java=1305|rust=1308"
  "Issue113-Protein.dsn:dsn:1|java=1665|rust=1674"
  "Issue157-TeamAdapt-LinePCB.dsn:dsn:1|java=2127,2140,2162,2172|rust=2131,2143,2163,2173"
  "Issue214-freerouting.dsn:dsn:1|java=2518,2778|rust=2519,2779,2934"
  "Issue269-caniot-tiny-arm.dsn:dsn:1|java=1280,1288|rust=1281,1289"
  "Issue269-z10_module.dsn:dsn:1|java=1931|rust="
  "Issue575-drc_Natural_Tone_Preamp_7_unconnected_items.dsn:dsn:1|java=1242,1696,1909|rust="
  "Issue690-kit-dev-coldfire-xilinx_5213.dsn:dsn:1|java=3121,3247,3258,5010,5012,5017,5019,5023,5027,5033,5035,5058,5061,5063,5068,5071,5150,5170,5186,5191,5193|rust=5009,5011,5018,5028,5032,5034,5062,5072,5075,5140,5151,5169,5171,5185,5190,5192,5194,5210"
  "Issue721-Autorouter_CE2632_HarryMu_2026-6-15.dsn:dsn:1|java=401|rust="
  "Issue733-kicad_interf_u_input_design.dsn:dsn:1|java=1229|rust=1232"
  "Issue756-tomu-fpga.dsn:dsn:1|java=1276|rust="
  "Issue756-tomu-fpga11.dsn:dsn:1|java=1341|rust=1346"
  "Issue756-tomu-fpga7.dsn:dsn:1|java=1342|rust=1347"
  "Issue756-tomu-fpga8.dsn:dsn:1|java=1342|rust=1347"
  "Issue756-tomu-fpga9.dsn:dsn:1|java=1341|rust=1346"
)

# ------------------------------------------------------------------------------------------------
# Mode 2's counter exceptions
# ------------------------------------------------------------------------------------------------
# `<file>:<variant>:2`. The `MAXCONN`/`INCOMPLETE`/`NET`/`ALCOUNT` block is strict on every other
# row in the corpus; this is the one board where the two seed orders do not merely pick different
# edges but a different *number* of them.
#
#  * Issue269-z10_module.dsn — net 1 has 4 connected groups; the jar finds 3 airlines for it and
#    the port 2, so `INCOMPLETE` reads 117 against 116. Measured under `-XX:hashCode=0,1,2,3,4` and
#    the default, the jar answers `NET 1 3 4` in all six, so this is not the jar being
#    hash-dependent. It is quirk #82 meeting ruling 3: `PlanarDelaunayTriangulation`'s in-circle
#    degeneracy on axis-aligned input loses edges, so which edges exist depends on the corner
#    insertion order, and a spanning "tree" that leaves two groups unjoined is a normal outcome on
#    both sides — `count == groups - 1` fails on 13 of `Issue022-AutoRouter_interrupted.dsn`'s nets
#    identically on both sides. **Mode 3 is the proof**: with the seed order pinned the port's way,
#    Java answers `NET 1 2 4` too, and this fixture's mode 3 matches line for line.
declare -a COUNTER_XDIFFS=(
  "Issue269-z10_module.dsn:dsn:2"
)

# ------------------------------------------------------------------------------------------------
# Mode 2's airline budget table (plan-5 ruling 4)
# ------------------------------------------------------------------------------------------------
# `<file>:<variant>|<the number of differing AL lines>` under the default `-XX:hashCode=2`, which is
# reproducible because both sides are deterministic there. A row is XDIFF while its measured number
# is **at or below** its budget and DIFF above it, so the table is a ratchet: the ratsnest may get
# closer to a given JVM run's answer, never further from it. A row that is not listed has a budget
# of -1 and therefore fails on any `AL` difference at all — which is what the 60-odd rows whose
# airlines match exactly want.
#
# Regenerate after a deliberate ratsnest change with
#
#   SWEEP_OUT=/tmp/p5t2 ./scripts/differential/sweep-p5t2.sh 2
#   sort -u /tmp/p5t2/airline-diffs.txt | awk -F'|' '$2 > 0 { printf "  \"%s|%s\"\n", $1, $2 }'
#
# and read every number that grew before pasting it in.
declare -a AIRLINE_BUDGETS=(
  "Issue015-StackOverflow.dsn:dsn|4"
  "Issue022-AutoRouter_interrupted.dsn:dsn|50"
  "Issue026-J2_reference.dsn:dsn|4"
  "Issue027-zMRETestFixture.dsn:dsn|98"
  "Issue029-hw48na.dsn:dsn|10"
  "Issue034-Green14SegLED.dsn:dsn|4"
  "Issue035-ReadPlaceScope.dsn:dsn|16"
  "Issue039-bug-design.dsn:dsn|18"
  "Issue054-tairakb.dsn:dsn|130"
  "Issue066-Project_GP8B.dsn:dsn|120"
  "Issue070-Autorouter_FQ101_PCB_2022-05-13.dsn:dsn|28"
  "Issue093-interf_u.dsn:dsn|2"
  "Issue103-Board-Unrouted.dsn:dsn|74"
  "Issue107-freq_teiler_200kHz_kicad_bad.dsn:dsn|32"
  "Issue107-freq_teiler_200kHz_kicad.dsn:dsn|22"
  "Issue110-Паяльная станция.dsn:dsn|4"
  "Issue110-Pajalnaja_stancija.dsn:dsn|4"
  "Issue113-Protein.dsn:dsn|8"
  "Issue145-smoothieboard.dsn:dsn|60"
  "Issue153-wavefolder.dsn:dsn|2"
  "Issue157-TeamAdapt-LinePCB.dsn:dsn|8"
  "Issue159-setonix_2hp-pcb.dsn:dsn|16"
  "Issue178-KeebMaker_Sofle_Choc.dsn:dsn|38"
  "Issue179-Autorouter_PCB1_2023-3-24.dsn:dsn|8"
  "Issue209-split05.dsn:dsn|106"
  "Issue209-split10.dsn:dsn|106"
  "Issue214-freerouting.dsn:dsn|4"
  "Issue217-8088sbc.dsn:dsn|12"
  "Issue219-LogicBoard_smt.dsn:dsn|162"
  "Issue229-display-8-digit-hc595.dsn:dsn|2"
  "Issue230-CNH_Functional_Tester_1.dsn:dsn|18"
  "Issue269-caniot-tiny-arm.dsn:dsn|6"
  "Issue269-z10_module.dsn:dsn|11"
  "Issue289-Autorouter_PCB_FHT-8086_2024-03-08.dsn:dsn|88"
  "Issue289-Autorouter_PCB_FHT-VGA_2024-03-25.dsn:dsn|78"
  "Issue297-myboard.dsn:dsn|10"
  "Issue420-contribution-board.dsn:dsn|472"
  "Issue508-DAC2020_bm01.dsn:dsn|12"
  "Issue508-DAC2020_bm04.dsn:dsn|28"
  "Issue508-DAC2020_bm05.dsn:dsn|6"
  "Issue508-DAC2020_bm06.dsn:dsn|8"
  "Issue508-DAC2020_bm07.dsn:dsn|2"
  "Issue508-DAC2020_bm09.dsn:dsn|6"
  "Issue508-DAC2020_bm10.dsn:dsn|24"
  "Issue508-DAC2020_bm11.dsn:dsn|22"
  "Issue555-CNH_Functional_Tester_1.dsn:dsn|18"
  "Issue558-dev-board.dsn:dsn|12"
  "Issue575-drc_dev-board_4_hole_clearance_violations.dsn:dsn|6"
  "Issue575-drc_Natural_Tone_Preamp_7_unconnected_items.dsn:dsn|12"
  "Issue684-Autorouter_PCB1_2026-5-8.dsn:dsn|6"
  "Issue690-kit-dev-coldfire-xilinx_5213.dsn:dsn|26"
  "Issue721-Autorouter_CE2632_HarryMu_2026-6-15.dsn:dsn|24"
  "Issue730-DAC2020_bm11.dsn:dsn|22"
  "Issue732-CM5_MINIMA_3.dsn:dsn|28"
  "Issue732-DAC2020_bm10.dsn:dsn|24"
  "Issue732-RoyalBlue54L-Feather.dsn:dsn|14"
  "Issue733-kicad_complex_hierarchy_input_design.dsn:dsn|2"
  "Issue733-kicad_interf_u_input_design.dsn:dsn|2"
  "Issue753-CPU-85_r104.dsn:dsn|26"
)

# ------------------------------------------------------------------------------------------------
# The rows — identical construction to `sweep-p5t1.sh`; see that script's comment.
# ------------------------------------------------------------------------------------------------
CORPUS_RESULTS="$ROOT/crates/fr-dsn/tests/data/corpus-read-results.txt"
DRC_FIXTURES_TXT="$ROOT/tests/reference/drc-fixtures.txt"

rows_file="$SWEEP_OUT/rows.txt"
skipped_file="$SWEEP_OUT/skipped.txt"
: > "$rows_file"
: > "$skipped_file"

while IFS= read -r line; do
  [[ "$line" == FIXTURE\ * ]] || continue
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
# Mode 1's expected-diff checker
# ------------------------------------------------------------------------------------------------
# Delete from each side the `U track_dangling <kind> - <id>` lines whose id is listed, require the
# deleted sets to be exactly the listed ones, adjust `UCOUNT` and `UTYPE track_dangling` by the
# number deleted on that side, and require the two outputs to be equal afterwards. So the counters
# are still compared — as `total - deleted` — rather than skipped.
expected_diff_mode1() {
  python3 - "$@" <<'EOF'
import sys

out_j, out_r, want_j, want_r = sys.argv[1:5]

def ids(spec):
    return set(filter(None, spec.split(",")))

def strip(path, drop):
    """Deletes the listed `U track_dangling` lines and restates the two counters net of them."""
    removed = set()
    kept = []
    for raw in open(path, encoding="utf-8"):
        fields = raw.rstrip("\n").split(" ")
        if fields[0] == "U" and fields[1] == "track_dangling" and fields[-1] in drop:
            removed.add(fields[-1])
        else:
            kept.append(fields)
    adjusted = []
    for fields in kept:
        counter = (fields[0] == "UCOUNT"
                   or (fields[0] == "UTYPE" and fields[1] == "track_dangling"))
        if counter:
            fields = fields[:-1] + [str(int(fields[-1]) - len(removed))]
        adjusted.append(" ".join(fields))
    return removed, adjusted

drop_j, drop_r = ids(want_j), ids(want_r)
got_j, kept_j = strip(out_j, drop_j)
got_r, kept_r = strip(out_r, drop_r)
if got_j != drop_j or got_r != drop_r:
    sys.exit("expected to remove java=%s rust=%s, removed java=%s rust=%s"
             % (sorted(drop_j), sorted(drop_r), sorted(got_j), sorted(got_r)))
for a, b in zip(kept_j, kept_r):
    if a != b:
        sys.exit("first difference outside the listed entries: java=%r rust=%r" % (a, b))
if len(kept_j) != len(kept_r):
    sys.exit("outputs differ in length outside the listed entries: %d vs %d"
             % (len(kept_j), len(kept_r)))
EOF
}

# ------------------------------------------------------------------------------------------------
# Modes 2, 3 and 4: the ratsnest
# ------------------------------------------------------------------------------------------------
# The ratsnest has exactly one free choice in it, and the three modes separate it from everything
# else. `NetIncompletes.calculateNetItems` seeds its outer loop from `uniqueItems.iterator().next()`
# over a `HashSet<Item>` (NetIncompletes.java:295, :299) — identity-hash order, nothing to port —
# and plan-5 ruling 3 fixes the port's at the lowest item id. `P5T2.java` transcribes the whole
# constructor so that seed can be varied:
#
#   * **mode 4** is Java against Java: the transcription with Java's own `HashSet` seed, compared to
#     the jar's real `getAllAirlines()`. It must print `TRANSCRIPTION equal 0`, which is what says
#     the transcription is the same algorithm as the jar's. Strict, no table.
#   * **mode 3** is the transcription seeded the port's way, against the port. With the one free
#     choice pinned identically on both sides the airlines themselves — not merely their counts —
#     must match. **Strict, no table**: this is the ratsnest's parity surface, and on the corpus it
#     is 0 diffs on every row.
#   * **mode 2** is the port against the jar's *own* hash-seeded answer, through the real
#     `DesignRulesChecker` accessors. Its counters (`MAXCONN`, `INCOMPLETE`, every `NET` line,
#     `ALCOUNT`) are strict — ruling 4 measured them hash-independent — with the single exception
#     listed in `COUNTER_XDIFFS`; its `AL` block is graded against the recorded budget in
#     `AIRLINE_BUDGETS`, because two different seed orders legitimately produce two different
#     spanning trees.
#
# Why mode 2 needs a budget where mode 3 does not: ruling 4 recorded that the port's spanning tree
# can have a different total *weight* than a JVM run's, i.e. that a different insertion order
# changes the edge set and not only the choice among ties. Mode 3 removes that variable; mode 2
# measures it. Requiring `AL` equality in mode 2, or the controller's ruling-T containment in a
# six-run JVM union, would fail on boards where nothing is wrong: measured while writing Task 10,
# `Issue022-AutoRouter_interrupted.dsn` (235 port airlines) leaves 11 port airlines outside the
# six-run union and still 2 outside a 60-run one, while its mode 3 matches exactly.
#
# For the three fixtures that have a committed `crates/fr-drc/tests/data/<stem>.airlines-union.txt`
# — the union over six JVM runs taken in Task 5 — mode 2 additionally applies ruling T's containment
# test, strictly. Those are ruling 4's own three fixtures, the file costs no JVM run to read, and
# Task 5 established they pass.
#
# `P5T2_UNION=1` builds a six-hash-mode JVM sample per differing mode-2 row and prints how many port
# airlines fall outside it. Information for a reviewer, not a gate; five extra JVM launches per row.
union_from_committed() {
  local stem="$1" out="$2"
  local file="$UNION_DIR/$stem.airlines-union.txt"
  [[ -f "$file" ]] || return 1
  sed -E 's/^net=([0-9]+) a=([0-9]+) b=([0-9]+)$/AL \1 \2 \3/' "$file" | sort -u > "$out"
}

# ------------------------------------------------------------------------------------------------
# The sweep
# ------------------------------------------------------------------------------------------------
echo "== compiling both sides (run.sh p5t2) =="
"$DIFF_ROOT/run.sh" p5t2 >/dev/null || { echo "smoke run failed" >&2; exit 1; }

printf '%-62s %-6s %s\n' "fixture" "row" "$(printf 'mode%-2s ' "${modes[@]}")"
fail=0
expected=0
total=0
start=$SECONDS
while IFS='|' read -r name variant dsn rules ses; do
  row=""
  for m in "${modes[@]}"; do
    total=$((total + 1))
    key="$name:$variant:$m"
    want_j=""; want_r=""; is_expected=0
    for e in "${EXPECTED_DIFFS[@]}"; do
      if [[ "${e%%|*}" == "$key" ]]; then
        is_expected=1
        spec="${e#*|}"
        want_j="${spec%%|*}"; want_j="${want_j#java=}"
        want_r="${spec#*|}";  want_r="${want_r#rust=}"
      fi
    done

    j="$SWEEP_OUT/$name.$variant.$m.j"
    r="$SWEEP_OUT/$name.$variant.$m.r"
    java_flags_for "$P5T_HASH_MODE"
    "$JAVABIN" "${JAVA_FLAGS[@]}" -cp "$CLASSES:$FREEROUTING_JAR" app.freerouting.drc.P5T2 \
      "$dsn" "${rules:--}" "${ses:--}" "$m" >"$j" 2>"$j.err"
    j_rc=$?
    "$RUST_BIN" "$dsn" "${rules:--}" "${ses:--}" "$m" >"$r" 2>"$r.err"
    r_rc=$?

    result=""
    if [[ "$j_rc" -ne 0 || "$r_rc" -ne 0 ]]; then
      result="DIFF"
    elif diff -q "$j" "$r" >/dev/null 2>&1; then
      result="MATCH"
    elif [[ "$m" == "2" ]]; then
      # The strict half: the two outputs with every `AL` line removed.
      grep -v '^AL ' "$j" > "$j.head"; grep -v '^AL ' "$r" > "$r.head"
      counter_ok=1
      if ! diff -q "$j.head" "$r.head" >/dev/null 2>&1; then
        counter_ok=0
        for c in "${COUNTER_XDIFFS[@]}"; do
          [[ "$c" == "$key" ]] && counter_ok=2
        done
      fi
      if [[ "$counter_ok" -eq 0 ]]; then
        result="DIFF"
        echo "DIFF  $key: the hash-independent counters differ" >>"$SWEEP_OUT/exit-codes.txt"
        diff "$j.head" "$r.head" | head -20 | sed 's/^/        /' >>"$SWEEP_OUT/exit-codes.txt"
      else
        [[ "$counter_ok" -eq 2 ]] && echo "XDIFF $key: the counters differ (listed; mode 3 is the proof)" >>"$SWEEP_OUT/exit-codes.txt"
        grep '^AL ' "$j" | sort -u > "$j.al"
        grep '^AL ' "$r" | sort -u > "$r.al"
        differing=$(comm -3 "$j.al" "$r.al" | grep -c .)
        # Always recorded, whatever the verdict: this file is how `AIRLINE_BUDGETS` is regenerated.
        printf '%s|%s\n' "$name:$variant" "$differing" >> "$SWEEP_OUT/airline-diffs.txt"

        budget=-1
        for b in "${AIRLINE_BUDGETS[@]}"; do
          [[ "${b%%|*}" == "$name:$variant" ]] && budget="${b#*|}"
        done

        contained=1
        note=""
        union="$SWEEP_OUT/$name.$variant.committed-union"
        if union_from_committed "${name%.dsn}" "$union"; then
          outside=$(comm -23 "$r.al" "$union" | wc -l | tr -d ' ')
          note=", $outside outside the committed $(wc -l <"$union" | tr -d ' ')-triple union"
          [[ "$outside" -eq 0 ]] || contained=0
        fi
        if [[ "${P5T2_UNION:-0}" == "1" ]]; then
          sample="$SWEEP_OUT/$name.$variant.jvm-union"
          cp "$j.al" "$sample"
          for h in 0 1 3 4 5; do
            java_flags_for "$h"
            "$JAVABIN" "${JAVA_FLAGS[@]}" -cp "$CLASSES:$FREEROUTING_JAR" \
              app.freerouting.drc.P5T2 "$dsn" "${rules:--}" "${ses:--}" 2 2>/dev/null \
              | grep '^AL ' >> "$sample"
          done
          sort -u -o "$sample" "$sample"
          note+=" (informational: $(comm -23 "$r.al" "$sample" | wc -l | tr -d ' ') of $(wc -l <"$r.al" | tr -d ' ') port airlines outside a $(wc -l <"$sample" | tr -d ' ')-triple 6-run JVM sample)"
        fi

        if [[ "$differing" -le "$budget" && "$contained" -eq 1 ]]; then
          result="XDIFF"
          expected=$((expected + 1))
          echo "XDIFF $key: ruling 4 — $differing differing AL lines (budget $budget)$note" \
            >>"$SWEEP_OUT/exit-codes.txt"
        elif [[ "$contained" -eq 0 ]]; then
          result="DIFF"
          echo "DIFF  $key: ruling T — a port airline is outside the committed union$note" \
            >>"$SWEEP_OUT/exit-codes.txt"
        else
          result="DIFF"
          echo "DIFF  $key: $differing differing AL lines, budget $budget$note" \
            >>"$SWEEP_OUT/exit-codes.txt"
        fi
      fi
    elif [[ "$is_expected" -eq 1 && "$m" == "1" ]] \
        && expected_diff_mode1 "$j" "$r" "$want_j" "$want_r" \
             >"$SWEEP_OUT/$name.$variant.$m.check" 2>&1; then
      result="XDIFF"
      expected=$((expected + 1))
      echo "XDIFF $key: ruling S — track_dangling java=[$want_j] rust=[$want_r]" \
        >>"$SWEEP_OUT/exit-codes.txt"
    else
      result="DIFF"
      echo "DIFF  $key: java exit=$j_rc rust exit=$r_rc" >>"$SWEEP_OUT/exit-codes.txt"
      if [[ -s "$SWEEP_OUT/$name.$variant.$m.check" ]]; then
        sed 's/^/        /' "$SWEEP_OUT/$name.$variant.$m.check" >>"$SWEEP_OUT/exit-codes.txt"
      fi
    fi

    if [[ "$result" == "MATCH" ]]; then
      rm -f "$j" "$r" "$j.err" "$r.err"
    elif [[ "$result" == "DIFF" ]]; then
      fail=$((fail + 1))
    fi
    row+="$(printf '%7s' "$result")"
  done
  printf '%-62s %-6s %s\n' "$name" "$variant" "$row"
done < "$rows_file"

skipped=$(wc -l < "$skipped_file" | tr -d ' ')
echo
echo "rows: $((total / ${#modes[@]}))   pairs: $total   unexpected diffs: $fail   expected diffs (XDIFF): $expected   skipped: $skipped"
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
