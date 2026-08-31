#!/usr/bin/env bash
# Generate the Java per-connection router references for `crates/fr-router/tests/reference_parity.rs`
# (Plan 6 Task 17, ruling 11).
#
# Sibling of `scripts/gen-drc-reference.sh`, and pinned to the same jar it is: the clone's **HEAD**
# build (`freerouting-current-executable.jar`). Plan 6's global constraints make HEAD the parity
# jar outright — `autoroute/**` has been refactored away from 2.3.0 (`MazeSearchAlgo` →
# `maze/MazeSearchEngine`, `LocateFoundConnectionAlgo*` → `path/FoundConnectionLocator*`, and
# `MazeExpansionEngine`/`MazeRipupResolver`/`AutorouteConnectionRouter` do not exist there at all),
# so a 2.3.0 reference would be a different algorithm rather than an older one.
#
# Unlike the DRC generator this one cannot drive the CLI: the CLI runs the whole pipeline (fanout,
# passes, the optimizer), and plan-6 ruling 2 cuts the port at step 5 of
# `AutorouteConnectionRouter.route`. So it compiles and runs `scripts/differential/java/P6T1.java`
# — the same driver `scripts/differential/run.sh p6t1` diffs against the Rust twin, so the
# reference and the differential can never describe different runs.
#
# Per stem in tests/reference/router-fixtures.txt it writes, into tests/reference/<stem>/:
#
#   router.jsonl      one JSON line per connection, the driver's stdout **minus its HEADER line**
#   java.log          the driver's stderr (its `java-version` line, plus anything the JVM said)
#                     (`--steps=1-8` writes router-steps18.jsonl / .meta.txt / java-steps18.log)
#   router.meta.txt   the jar's identity, `java -version`, the hash mode, the command line and the
#                     HEADER line, with every machine-specific prefix replaced by
#                     `<FREEROUTING_JAVA_DIR>` or `<workspace>` so the file is the same on every
#                     machine
#
# The HEADER line is the one thing that is *not* committed as-is: it names the jar by absolute
# path, its byte size and its mtime, which are properties of this checkout and not of the board.
# `run.sh p6t1` diffs it (that is what makes a run against the wrong jar a diff rather than a
# silent pass); the reference keeps it in `router.meta.txt` instead, where a reader can see which
# jar produced the file without the committed bytes moving between machines. Everything below the
# HEADER line is copied verbatim — no normalisation, no re-rendering, exactly as
# `gen-drc-reference.sh` copies `drc.json`.
#
# Usage:
#   scripts/gen-router-reference.sh [stem ...]        regenerate all stems, or just the named ones
#   scripts/gen-router-reference.sh --meta-only [stem ...]
#                                                     rewrite router.meta.txt from the existing
#                                                     router.jsonl without running the jar
#   scripts/gen-router-reference.sh --verify-hash-modes [stem ...]
#                                                     regenerate each stem once per
#                                                     -XX:hashCode=0..4 into a scratch dir and
#                                                     require five byte-identical files; writes
#                                                     nothing under tests/reference/
#   scripts/gen-router-reference.sh --steps=1-8 [stem ...]
#                                                     the same, but the driver runs
#                                                     `AutorouteConnectionRouter.route` **in full**
#                                                     (steps 1-8) and the outputs are named
#                                                     router-steps18.{jsonl,meta.txt}
#
# `--steps=1-8` (Plan 7 Task 8) adds step 6's `optChangedArea` on `ROUTED`, step 7's necked retry
# and step 8's strict-DRC rollback, through `scripts/differential/java/probes/P7T8Probe.java`
# (`AutorouteConnectionRouter` and `BatchAutorouter.autorouteItem` are both package-private in
# `app.freerouting.autoroute.pipeline`, so the probe declares that package and forwards). It
# combines with `--meta-only` and `--verify-hash-modes`. The `1-5` outputs are untouched by it,
# including their HEADER line, which is why the Plan 6 references did not have to be regenerated
# when the argument was added.
#
# A stem may carry a committed `tests/reference/<stem>/router-steps18.xdiff.txt`; its contents are
# appended verbatim to the generated `router-steps18.meta.txt`. That is how a known port-vs-jar
# divergence is recorded **without** the note being lost on the next regeneration — the note is a
# committed input, not generated output.
#
# `--verify-hash-modes` is the direct check of ruling 1's premise — that a single-threaded Java
# routing run is reproducible and does not depend on `Object.hashCode`. The survey verified it
# end-to-end on SES output; this verifies it per connection, over the attempt state, the ripped
# set, every inserted trace polyline and via, and the metric block.
#
# Environment: FREEROUTING_JAVA_DIR (default ../freerouting), FREEROUTING_JAR, JAVA, JAVAC,
#              ROUTER_HASH_MODE, ROUTER_TIMEOUT.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# `cd`/`pwd` rather than the raw value: `$ROOT/../freerouting` is not normalised, and the driver
# prints `dsn.toAbsolutePath().normalize()` in its HEADER line, so an un-normalised prefix would
# never match and `router.meta.txt` would keep a machine-specific path.
JAVA_DIR="$(cd "${FREEROUTING_JAVA_DIR:-$ROOT/../freerouting}" && pwd)"
JAR="${FREEROUTING_JAR:-$JAVA_DIR/build/libs/freerouting-current-executable.jar}"
JAVA_BIN="${JAVA:-/opt/homebrew/opt/openjdk@25/bin/java}"
JAVAC_BIN="${JAVAC:-/opt/homebrew/opt/openjdk@25/bin/javac}"
REF="$ROOT/tests/reference"
FIXTURES="$REF/router-fixtures.txt"
DRIVER="$ROOT/scripts/differential/java/P6T1.java"
CLASSES="$ROOT/scripts/differential/build/classes-gen-router"

# `-XX:hashCode=2` is the constant-hash mode — the only `Object.hashCode` source in the JVM that
# reproduces run to run *and* is not derived from an object address. The router's own containers
# are `TreeSet`/`TreeMap`/`LinkedHashMap` throughout (plan-6 ruling 4), so unlike the DRC's this
# output is not *expected* to depend on the mode; the sweep is what turns that expectation into
# evidence. Modes 0 and 5 are PRNG-seeded, 1 and 4 come from the address, 3 is a per-thread
# xorshift.
HASH_MODE="${ROUTER_HASH_MODE:-2}"

# Quirk #162 (`SortedRoomNeighbours.calculateNewIncompleteRooms`) does not terminate for a small
# fraction of room completions, and neither Java nor the port guards it — `isStopRequested` is not
# consulted on that path in either language, so the per-connection `TimeLimit` cannot end it. A
# corpus board that reaches it hangs the JVM, so the generator bounds the wall clock itself and
# reports the stem as failed rather than hanging the machine.
TIMEOUT_SECONDS="${ROUTER_TIMEOUT:-1800}"

# `-Duser.language=en -Duser.country=US` is hygiene here rather than load-bearing (the driver
# renders every number through `Double.toString` and `Integer.toString`, neither of which is
# locale-sensitive), but it is pinned so this generator's JVM flags are the same set
# `gen-drc-reference.sh` and `run.sh`'s `p6t*` drivers use.
LOCALE_FLAGS=(-Djava.awt.headless=true -Duser.language=en -Duser.country=US)

MODE=generate
# Plan 7 Task 8: `1-5` is Plan 6's slice and the default; `1-8` is the whole of
# `AutorouteConnectionRouter.route`. The two write different files and never collide.
STEPS=1-5
JSONL=router.jsonl
META=router.meta.txt
# A separate stderr file per slice: a `--steps=1-8` run must not clobber the `1-5` run's
# `java.log`, which is committed beside its own transcript.
JAVALOG=java.log
DRIVER_SUFFIX=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --verify-hash-modes) MODE=sweep; shift ;;
    --meta-only) MODE=meta; shift ;;
    --steps=1-5) STEPS=1-5; shift ;;
    --steps=1-8)
      STEPS=1-8
      JSONL=router-steps18.jsonl
      META=router-steps18.meta.txt
      JAVALOG=java-steps18.log
      DRIVER_SUFFIX=" --steps=1-8"
      shift
      ;;
    --steps=*) echo "error: steps must be 1-5 or 1-8, not ${1#--steps=}" >&2; exit 1 ;;
    *) break ;;
  esac
done
WANTED=("$@")
# The driver's fifth and sixth arguments. `neckWidthUm = 0` is `DefaultSettings.java:109`'s own
# value, so the committed `1-8` transcripts are the production configuration; a non-zero neck is a
# `run.sh p6t1 … 1-8 <um>` experiment, not a reference.
STEP_ARGS=()
[[ "$STEPS" == 1-8 ]] && STEP_ARGS=(- 1-8 0)

# --- preflight -----------------------------------------------------------------------------------
if [[ ! -f "$JAR" ]]; then
  echo "error: HEAD jar not found at $JAR" >&2
  echo "       build it in the Java clone (./gradlew build) or set FREEROUTING_JAR" >&2
  exit 1
fi
if [[ ! -x "$JAVAC_BIN" ]]; then
  echo "error: javac not found at $JAVAC_BIN (need JDK >= 25; set JAVAC)" >&2
  exit 1
fi
ver="$("$JAVA_BIN" -version 2>&1 | head -1 | sed -E 's/.*"([0-9]+).*/\1/')"
if [[ "${ver:-0}" -lt 25 ]]; then
  echo "error: Java 25+ required (found ${ver:-none}). On macOS: brew install openjdk@25" >&2
  echo "       then: export JAVA=/opt/homebrew/opt/openjdk@25/bin/java" >&2
  exit 1
fi

# The wall-clock bound. `timeout(1)` is coreutils'; on macOS it arrives as `timeout` or `gtimeout`
# from `brew install coreutils`. Running unbounded is allowed but announced, because the failure
# mode it guards is a hang and not an error.
TIMEOUT=()
if command -v timeout >/dev/null 2>&1; then
  TIMEOUT=(timeout "$TIMEOUT_SECONDS")
elif command -v gtimeout >/dev/null 2>&1; then
  TIMEOUT=(gtimeout "$TIMEOUT_SECONDS")
else
  echo "warning: no timeout(1) on PATH; a quirk-#162 hang will not be bounded" >&2
fi

wanted() {
  [[ ${#WANTED[@]} -eq 0 ]] && return 0
  local stem="$1" w
  for w in "${WANTED[@]}"; do [[ "$w" == "$stem" ]] && return 0; done
  return 1
}

# Machine-independent rendering of a path, `gen-drc-reference.sh`'s `portable`.
#
# `$ROOT` first, deliberately: in the default layout the Java clone is `…/freerouting` and this
# workspace is its sibling `…/freerouting-rs`, so `$JAVA_DIR` is a *prefix* of `$ROOT` and
# substituting it first would render `<workspace>/scripts/…` as `<FREEROUTING_JAVA_DIR>-rs/scripts/…`.
portable() {
  local p="$1"
  p="${p//$ROOT/<workspace>}"
  p="${p//$JAVA_DIR/<FREEROUTING_JAVA_DIR>}"
  printf '%s' "$p"
}

# One driver run. `$1` is the stdout target, `$2` the stderr target, `$3` the hash mode; the rest
# is the driver's argv.
run_driver() {
  local out="$1" log="$2" mode="$3"
  shift 3
  "${TIMEOUT[@]}" "$JAVA_BIN" "${LOCALE_FLAGS[@]}" \
      -XX:+UnlockExperimentalVMOptions "-XX:hashCode=$mode" \
      -cp "$CLASSES:$JAR" app.freerouting.autoroute.maze.P6T1 "$@" ${STEP_ARGS+"${STEP_ARGS[@]}"} \
      > "$out" 2> "$log"
}

write_meta() {
  local out="$1" dsn="$2" max_items="$3" ripup_pass_no="$4" header="$5"
  {
    echo "jar          $(portable "$JAR")"
    echo "jar size     $(wc -c < "$JAR" | tr -d ' ') bytes"
    echo "jar mtime    $(date -r "$JAR" '+%Y-%m-%d %H:%M:%S %z')"
    echo "jar version  Freerouting $(unzip -p "$JAR" app/freerouting/constants/Constants.class 2>/dev/null \
        | strings | grep -Eo '^[0-9]+\.[0-9]+\.[0-9]+(-SNAPSHOT)?$' | head -1)"
    echo "java         $("$JAVA_BIN" -version 2>&1 | head -1)"
    echo "hash mode    -XX:hashCode=$HASH_MODE"
    echo "driver       $(portable "$DRIVER")$DRIVER_SUFFIX"
    echo "connections  $(wc -l < "$out/$JSONL" | tr -d ' ')"
    printf 'command      java %s -XX:+UnlockExperimentalVMOptions -XX:hashCode=%s -cp <classes>:<jar> app.freerouting.autoroute.maze.P6T1 %s %s %s%s\n' \
        "${LOCALE_FLAGS[*]}" "$HASH_MODE" "$(portable "$JAVA_DIR/$dsn")" "$max_items" "$ripup_pass_no" \
        "${STEP_ARGS+ ${STEP_ARGS[*]}}"
    echo "header       $(portable "$header")"
    # A committed, hand-written note about this stem's known port-vs-jar divergence, appended
    # verbatim so regeneration cannot lose it. An `if` and not a `&&`: this is the last command in
    # the brace group, and a false `[[ ]]` would make the group — and `write_meta` — return 1,
    # which `set -e` turns into an abort after the first stem that has no note.
    if [[ -f "$out/${JSONL%.jsonl}.xdiff.txt" ]]; then
      cat "$out/${JSONL%.jsonl}.xdiff.txt"
    fi
  } > "$out/$META"
}

each_row() {
  local body="$1" stem dsn max_items ripup_pass_no
  # The fourth field is optional and defaults to 1 — the `ripupPassNo` every stem but
  # `router-dac2020-bm01-pass2` uses (Plan 6 Task 17b).
  while IFS='|' read -r stem dsn max_items ripup_pass_no || [[ -n "$stem" ]]; do
    [[ -z "$stem" || "$stem" == \#* ]] && continue
    wanted "$stem" || continue
    "$body" "$stem" "$dsn" "$max_items" "${ripup_pass_no:-1}"
  done < "$FIXTURES"
}

report_states() {
  awk '{
        if (match($0, /"state":"[A-Z_]+"/)) {
          s = substr($0, RSTART + 9, RLENGTH - 10);
          n[s]++;
        }
      }
      END {
        line = "";
        for (s in n) line = line " " s "=" n[s];
        printf "   %d connections%s\n", NR, line;
      }' "$1"
}

# --- compile the driver once ---------------------------------------------------------------------
compile_driver() {
  echo "== compiling $(portable "$DRIVER") against $(portable "$JAR")"
  rm -rf "$CLASSES"
  mkdir -p "$CLASSES"
  # `P6T1.java` imports `P7T8Probe` for its `--steps=1-8` path, so the probe is always compiled
  # alongside — under `1-5` it is loaded and never called.
  "$JAVAC_BIN" -cp "$JAR" -d "$CLASSES" "$DRIVER" \
      "$ROOT/scripts/differential/java/probes/P7T8Probe.java"
}

# --- the three modes -------------------------------------------------------------------------------
STATUS=0

generate_one() {
  local stem="$1" dsn="$2" max_items="$3" ripup_pass_no="$4" out="$REF/$1" tmp
  mkdir -p "$out"
  tmp="$out/router.raw.tmp"
  echo "== $stem"
  rm -f "$tmp"
  # Write to a temporary and move only after the JVM exits 0 *and* left a HEADER behind, so a
  # failed or timed-out run leaves the committed reference and its meta untouched, together.
  if ! run_driver "$tmp" "$out/$JAVALOG" "$HASH_MODE" "$JAVA_DIR/$dsn" "$max_items" "$ripup_pass_no" \
      || [[ ! -s "$tmp" ]]; then
    echo "   the driver failed for $stem; see $out/$JAVALOG ($JSONL left untouched)" >&2
    rm -f "$tmp"
    STATUS=1
    return 0
  fi
  local header
  header="$(head -1 "$tmp")"
  if [[ "$header" != HEADER\ * ]]; then
    echo "   $stem produced no HEADER line; refusing to overwrite the reference" >&2
    rm -f "$tmp"
    STATUS=1
    return 0
  fi
  tail -n +2 "$tmp" > "$out/$JSONL"
  rm -f "$tmp"
  write_meta "$out" "$dsn" "$max_items" "$ripup_pass_no" "$header"
  report_states "$out/$JSONL"
}

meta_one() {
  local stem="$1" dsn="$2" max_items="$3" ripup_pass_no="$4" out="$REF/$1"
  echo "== $stem"
  if [[ ! -f "$out/$JSONL" ]]; then
    echo "   no $JSONL to describe; run without --meta-only first" >&2
    STATUS=1
    return 0
  fi
  local header
  header="$(grep -m1 '^header  *' "$out/$META" 2>/dev/null | sed 's/^header  *//')"
  write_meta "$out" "$dsn" "$max_items" "$ripup_pass_no" "${header:-<unknown>}"
  echo "   $META rewritten ($JSONL untouched)"
}

sweep_one() {
  local stem="$1" dsn="$2" max_items="$3" ripup_pass_no="$4" mode raw digest
  echo "== $stem"
  local digests=() counts=()
  for mode in 0 1 2 3 4; do
    raw="$SCRATCH/$stem-h$mode.jsonl"
    if run_driver "$raw" "$SCRATCH/$stem-h$mode.log" "$mode" "$JAVA_DIR/$dsn" "$max_items" \
        "$ripup_pass_no"; then
      # The HEADER line is stripped for the same reason the reference strips it: it carries the
      # jar's mtime, which is the same in all five runs but is not a property of the board.
      tail -n +2 "$raw" > "$raw.body"
      digest="$(shasum -a 256 < "$raw.body" | cut -d' ' -f1)"
      digests+=("${digest:0:12}")
      counts+=("$(wc -l < "$raw.body" | tr -d ' ')")
    else
      # Never `continue`: the arrays are indexed by mode below, so a gap would misalign them.
      digests+=("FAILED------")
      counts+=("?")
      STATUS=1
    fi
  done
  local distinct
  distinct="$(printf '%s\n' "${digests[@]}" | sort -u | wc -l | tr -d ' ')"
  if [[ "$distinct" == "1" && "${digests[0]}" != "FAILED------" ]]; then
    echo "   5 modes agree (${digests[0]}, ${counts[0]} connections)"
  else
    echo "   $distinct distinct outputs over 5 modes:" >&2
    for mode in 0 1 2 3 4; do
      echo "     hashCode=$mode ${digests[$mode]} connections=${counts[$mode]}" >&2
    done
    STATUS=1
  fi
}

compile_driver
case "$MODE" in
  sweep)
    SCRATCH="$(mktemp -d)"
    trap 'rm -rf "$SCRATCH"' EXIT
    each_row sweep_one
    if [[ "$STATUS" -ne 0 ]]; then
      echo "hash-mode sweep found a disagreement — record it in tests/reference/README.md" >&2
    fi
    ;;
  meta)
    each_row meta_one
    ;;
  generate)
    each_row generate_one
    echo "done. router references in $REF"
    ;;
esac
exit "$STATUS"
