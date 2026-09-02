#!/usr/bin/env bash
# Generate the Java **whole-board** router references for `crates/fr-router/tests/batch_parity.rs`
# (Plan 7 Task 16, ruling AM as amended by scan ruling 13).
#
# Sibling of `scripts/gen-router-reference.sh`, sharing its `portable()`, its preflight, its
# `timeout(1)` bound, its `--meta-only` / `--verify-hash-modes` modes and the same jar: the clone's
# **HEAD** build (`freerouting-current-executable.jar`). It reads columns 5-7 of
# `tests/reference/router-fixtures.txt` — `max_passes|fanout|optimizer` — and skips every row that
# has none.
#
# Per stem it writes, into tests/reference/<stem>/:
#
#   batch.ses          the jar's **verbatim** SES for the whole-board run (ruling 1's rung (c))
#   batch.passes.jsonl one JSON object per completed routing pass — the `PassRecord` tuple,
#                      scraped from the driver rather than from the log (rung (a))
#   batch.meta.txt     the jar's identity, `java -version`, the hash mode, the budget flag, the
#                      full command line and the `--verify-driver` verdict, with every
#                      machine-specific prefix replaced by `<FREEROUTING_JAVA_DIR>`/`<workspace>`
#   java.batch.log     the `batch`-mode driver's stdout+stderr, **truncated at its `[board]`
#                      line** — everything above it is the run's decisions (the `ARGV`,
#                      `SETTINGS` and `BOARD-PREPARED` lines, the `EVENT`/`RESULT` block), and
#                      everything below it is a per-item dump of the same board `batch.ses`
#                      already carries verbatim. That is 14 committed lines instead of 2 324 on
#                      the largest stem, and nothing a reader can only get from the dump
#   java.batch-router.log  the `batch-router`-mode driver's, truncated the same way: its
#                      `FANOUT-SUMMARY`, per-pass `PASS` tuple and every decision arm the loop
#                      took, which is what the obligation sweep reads
#
# ## Why `P7T9.java` and not the bare jar — and the correction the plan text needs
#
# The plan asks for the reference to come from `P7T9.java` rather than from `java -jar … -de …
# -do …`, because ruling AI wants `optChangedArea`'s `TIME_LIMIT_TO_PREVENT_ENDLESS_LOOP` budget
# disabled on both sides and the jar has no flag for it, so "the driver reflects the constant to 0
# itself".
#
# **The driver cannot do that, and nothing can.** All four declarations are
# `static final int … = 1000` with a constant initialiser (`AutorouteConnectionRouter.java:22`,
# `BatchAutorouter.java:43`, `BatchAutorouterThread.java:38`, `AutoroutePassRunner.java:32`), so
# `javac` inlines them: `javap -c -p -cp <jar> app.freerouting.autoroute.pipeline.BatchAutorouter`
# shows `sipush 1000` immediately before the `optChangedArea` call and no `getstatic` anywhere.
# Writing the fields reflectively changes nothing. `scripts/differential/run.sh`'s `p7t8` case
# already recorded this ("which `javac` inlines and no reflection reaches"); Plan 7 Task 10's
# report records it too.
#
# So the arrangement is the one every `p7t*` driver already uses and the one the plan's own
# fallback describes: **the Java side runs with the live 1000 ms limit and the port with
# `RouterBudget::disabled()`**, and a byte-identical SES is then *evidence* that the limit never
# changed the result rather than an assumption that it could not. `--verify-driver`'s trip count
# (below) is what turns a difference into a diagnosis.
#
# `P7T9` is still what the generator runs, for a different and stronger reason: it is the driver
# `scripts/differential/run.sh p7t9` diffs against the Rust twin, so the reference and the
# differential can never describe different runs — `gen-router-reference.sh`'s argument, verbatim.
# `--verify-driver` is what makes that safe: it proves the driver's SES *is* the bare jar's.
#
# ## The two invocations per stem, and why there are two
#
# `mode batch` is the real `RoutingPipeline.createForHeadless(job).run()` on a board loaded through
# the real `HeadlessBoardManager` (controller ruling AW — 15 of 16 corpus boards are mutated on
# load by `applyCopperToEdgeClearanceOverride`/`applyHoleClearanceOverride`, so a reference
# generated without that layer is not the jar's output), with settings from the real two-merge
# `SettingsMerger` ladder driven by the same `argv` the bare jar is given. It answers `batch.ses`.
#
# `mode batch-router` is the same board and the same settings with the optimizer switched off and
# the routing stage **transcribed**, so every completed pass prints its `PassRecord` tuple. It
# answers `batch.passes.jsonl`. Java's pipeline exposes no per-pass tuple — the loop's own
# `TaskStateChangedEvent` carries only a pass number — so rung (a) needs this second run; the
# routing stage it runs is the one `mode batch`'s pipeline runs first, which is exactly what
# `run.sh p7t9 <dsn> <n> batch-router` MATCHing pins.
#
# ## Usage
#
#   scripts/gen-batch-reference.sh [stem ...]         generate all batch stems, or just the named
#   scripts/gen-batch-reference.sh --force [stem ...] regenerate even where outputs already exist
#                                                     (without it the run is **resumable**: a stem
#                                                     whose batch.ses and batch.passes.jsonl are
#                                                     both present is skipped)
#   scripts/gen-batch-reference.sh --meta-only [stem ...]
#                                                     rewrite batch.meta.txt from the existing
#                                                     outputs without running the jar
#   scripts/gen-batch-reference.sh --verify-driver [stem ...]
#                                                     run the **bare jar** (`-de`/`-do`) and the
#                                                     driver on the same argv and compare the two
#                                                     SES files byte for byte; the verdict is
#                                                     written to batch.verify-driver.txt, which
#                                                     write_meta appends verbatim
#   scripts/gen-batch-reference.sh --verify-hash-modes [stem ...]
#                                                     regenerate each stem's SES once per
#                                                     -XX:hashCode=0..4 into a scratch dir and
#                                                     require five byte-identical files; writes
#                                                     nothing under tests/reference/
#
# `--verify-driver`'s verdict follows controller answer 1. Identical is `bare-jar: identical`. A
# difference is re-measured by **counting the jar's own log**: `TraceTightener.isStopRequested`
# (`board/optimize/TraceTightener.java:202-211`) calls
# `FRLogger.debug("TraceTightener.is_stop_requested: time limit exceeded")` on every exceeded
# check, and `Log4j2ConfigurationFactory` builds a root logger at `Level.ALL` with a file appender
# at `-Dfreerouting.logging.file.level` (default DEBUG) writing to
# `-Dfreerouting.logging.file.location`. So the re-run adds those two properties plus
# `-Dfreerouting.logging.console.enabled=false` and greps the file; the verdict becomes
# `bare-jar: budget-tripped (n)`. **A difference with n = 0 is a failure**, because the driver is
# then not the jar.
#
# (A programmatic Log4j2 counting appender was tried first and received **zero** events against
# this jar's configuration factory — see `P7T9.java`'s "The budget" section. The `-D` route is the
# jar's own supported knob and is verified working: a plain `router-rpi-splitter` run writes 113
# DEBUG lines to the file, 0 of them trips.)
#
# ## Environment
#
# FREEROUTING_JAVA_DIR (default ../freerouting), FREEROUTING_JAR, JAVA, JAVAC, BATCH_HASH_MODE,
# BATCH_TIMEOUT.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# `cd`/`pwd` rather than the raw value, for `gen-router-reference.sh`'s reason: the driver prints
# `dsn.toAbsolutePath().normalize()`, so an un-normalised prefix would never match.
JAVA_DIR="$(cd "${FREEROUTING_JAVA_DIR:-$ROOT/../freerouting}" && pwd)"
JAR="${FREEROUTING_JAR:-$JAVA_DIR/build/libs/freerouting-current-executable.jar}"
JAVA_BIN="${JAVA:-/opt/homebrew/opt/openjdk@25/bin/java}"
JAVAC_BIN="${JAVAC:-/opt/homebrew/opt/openjdk@25/bin/javac}"
REF="$ROOT/tests/reference"
FIXTURES="$REF/router-fixtures.txt"
DRIVER="$ROOT/scripts/differential/java/P7T9.java"
CLASSES="$ROOT/scripts/differential/build/classes-gen-batch"

# `-XX:hashCode=2`, `gen-router-reference.sh`'s constant-hash mode and the same argument for it:
# the router's own containers are `TreeSet`/`TreeMap`/`LinkedHashMap` throughout (plan-6 ruling 4),
# so the output is not *expected* to depend on the mode, and `--verify-hash-modes` is what turns
# that expectation into evidence — now with fanout and the optimizer live.
HASH_MODE="${BATCH_HASH_MODE:-2}"

# The wall-clock bound. A whole-board run with the optimizer on is the longest thing in this
# repository's harness; quirk #162's non-terminating room completion is the failure this guards.
TIMEOUT_SECONDS="${BATCH_TIMEOUT:-7200}"

LOCALE_FLAGS=(-Djava.awt.headless=true -Duser.language=en -Duser.country=US)

MODE=generate
FORCE=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --verify-hash-modes) MODE=sweep; shift ;;
    --verify-driver) MODE=verify-driver; shift ;;
    --meta-only) MODE=meta; shift ;;
    --force) FORCE=1; shift ;;
    --*) echo "error: unknown option $1" >&2; exit 1 ;;
    *) break ;;
  esac
done
WANTED=("$@")

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
  exit 1
fi

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

# `gen-drc-reference.sh`'s `portable`, and `$ROOT` first for the same reason: in the default layout
# `$JAVA_DIR` is a prefix of `$ROOT`.
portable() {
  local p="$1"
  p="${p//$ROOT/<workspace>}"
  p="${p//$JAVA_DIR/<FREEROUTING_JAVA_DIR>}"
  printf '%s' "$p"
}

# One driver run. `$1` is the combined stdout+stderr target, `$2` the hash mode; the rest is the
# driver's argv. stdout and stderr are merged because the driver's stderr is one `java-version`
# line plus anything the JVM said, and a batch log is read by a human, not diffed.
#
# `TRIP_LOG`, when set, adds the three properties that make the jar write its own DEBUG log to
# that path — the budget-trip measurement; see the header. It is empty on every reference path,
# because turning DEBUG on costs wall-clock time and wall-clock time is what the budget measures.
TRIP_LOG=""
# Make a driver log committable: truncate it at its `[board]` line — everything below is a
# per-item dump of the board `batch.ses` already carries verbatim, and it is 99 % of the bytes on
# a big stem — and replace every machine-specific prefix, exactly as `portable` does for the meta,
# so the committed file is the same on every machine.
commit_log() {
  local log="$1" tmp="$1.trunc"
  awk '{ if ($0 == "[board]") { print "[board] (elided — the board is batch.ses)"; exit } print }' \
      "$log" \
    | sed -e "s#$ROOT#<workspace>#g" -e "s#$JAVA_DIR#<FREEROUTING_JAVA_DIR>#g" \
    > "$tmp" && mv "$tmp" "$log"
}

run_driver() {
  local log="$1" hash="$2"
  shift 2
  local trip_flags=()
  if [[ -n "$TRIP_LOG" ]]; then
    trip_flags=(
      -Dfreerouting.logging.console.enabled=false
      -Dfreerouting.logging.file.enabled=true
      "-Dfreerouting.logging.file.location=$TRIP_LOG"
      -Dfreerouting.logging.file.level=DEBUG
    )
  fi
  "${TIMEOUT[@]}" "$JAVA_BIN" "${LOCALE_FLAGS[@]}" ${trip_flags+"${trip_flags[@]}"} \
      -XX:+UnlockExperimentalVMOptions "-XX:hashCode=$hash" \
      -cp "$CLASSES:$JAR" app.freerouting.autoroute.pipeline.P7T9 "$@" \
      > "$log" 2>&1
}

# The exact message `TraceTightener.isStopRequested:209` logs on an exceeded budget.
TRIP_MESSAGE='TraceTightener.is_stop_requested: time limit exceeded'

# The bare jar, with the argv `P7T9.batchArgv` builds. Same JVM flags, same hash mode.
run_bare_jar() {
  local log="$1" hash="$2" dsn="$3" max_passes="$4" fanout="$5" optimizer="$6" ses="$7"
  "${TIMEOUT[@]}" "$JAVA_BIN" "${LOCALE_FLAGS[@]}" \
      -XX:+UnlockExperimentalVMOptions "-XX:hashCode=$hash" \
      -jar "$JAR" \
      -de "$dsn" -do "$ses" -mp "$max_passes" \
      "--router.fanout.enabled=$fanout" \
      "--router.optimizer.enabled=$optimizer" \
      > "$log" 2>&1
}

# `on`/`off` in the fixture table, `true`/`false` on the two command lines.
bool_of() { [[ "$1" == "on" ]] && printf 'true' || printf 'false'; }

write_meta() {
  local stem="$1" dsn="$2" max_passes="$3" fanout="$4" optimizer="$5" out="$REF/$1"
  {
    echo "jar          $(portable "$JAR")"
    echo "jar size     $(wc -c < "$JAR" | tr -d ' ') bytes"
    echo "jar mtime    $(date -r "$JAR" '+%Y-%m-%d %H:%M:%S %z')"
    echo "jar revision $(unzip -p "$JAR" META-INF/MANIFEST.MF 2>/dev/null \
        | tr -d '\r' | sed -n 's/^Build-Revision: *//p' | head -1)"
    # Deliberately **not** a version string: HEAD's manifest carries
    # `Implementation-Version: unspecified`, so `Build-Revision` is the only field that pins which
    # build produced these bytes (the plan text's `2.3.1-SNAPSHOT` line cannot exist there).
    echo "java         $("$JAVA_BIN" -version 2>&1 | head -1)"
    echo "hash mode    -XX:hashCode=$HASH_MODE"
    echo "budget       optChangedArea TIME_LIMIT_TO_PREVENT_ENDLESS_LOOP = 1000 ms, LIVE."
    echo "             The constant is a javac-inlined compile-time constant (\`sipush 1000\` at"
    echo "             every call site), so no flag and no reflection disables it on the Java"
    echo "             side. The port runs RouterBudget::disabled(); a byte-identical batch.ses"
    echo "             is therefore evidence that the limit never changed the result. Trips are"
    echo "             counted from the jar's own DEBUG log when --verify-driver needs them."

    echo "driver       $(portable "$DRIVER")"
    echo "passes       $(wc -l < "$out/batch.passes.jsonl" | tr -d ' ')"
    echo "ses bytes    $(wc -c < "$out/batch.ses" | tr -d ' ')"
    echo "ses sha256   $(shasum -a 256 < "$out/batch.ses" | cut -d' ' -f1)"
    printf 'command      java %s -XX:+UnlockExperimentalVMOptions -XX:hashCode=%s -cp <classes>:<jar> app.freerouting.autoroute.pipeline.P7T9 %s %s batch --fanout %s --optimizer %s --ses <ref>/batch.ses\n' \
        "${LOCALE_FLAGS[*]}" "$HASH_MODE" "$(portable "$JAVA_DIR/$dsn")" "$max_passes" "$fanout" "$optimizer"
    printf 'passes cmd   java %s -XX:+UnlockExperimentalVMOptions -XX:hashCode=%s -cp <classes>:<jar> app.freerouting.autoroute.pipeline.P7T9 %s %s batch-router --fanout %s --optimizer off --passes <ref>/batch.passes.jsonl\n' \
        "${LOCALE_FLAGS[*]}" "$HASH_MODE" "$(portable "$JAVA_DIR/$dsn")" "$max_passes" "$fanout"
    printf 'bare jar     java %s -XX:+UnlockExperimentalVMOptions -XX:hashCode=%s -jar <jar> -de %s -do <ses> -mp %s --router.fanout.enabled=%s --router.optimizer.enabled=%s\n' \
        "${LOCALE_FLAGS[*]}" "$HASH_MODE" "$(portable "$JAVA_DIR/$dsn")" "$max_passes" \
        "$(bool_of "$fanout")" "$(bool_of "$optimizer")"
    echo "json source  $(json_source_note)"
    # `--verify-driver`'s verdict, a committed input rather than generated output — the
    # `router-steps18.xdiff.txt` precedent, so a regeneration cannot lose it. An `if` and not a
    # `&&`, so a stem with no verdict does not make the brace group return 1 under `set -e`.
    if [[ -f "$out/batch.verify-driver.txt" ]]; then
      cat "$out/batch.verify-driver.txt"
    else
      echo "bare-jar     not measured — run scripts/gen-batch-reference.sh --verify-driver $stem"
    fi
  } > "$out/batch.meta.txt"
}

# `JsonFileSettings` is priority 10 of the merge and the one source the port has no counterpart for
# (spec §2 puts `freerouting.json` out of scope). It is machine state, so the meta records whether
# the file exists and whether its `router` scope is empty — the only shape in which the port's
# omission is a no-op.
json_source_note() {
  local json="$HOME/Library/Application Support/freerouting/freerouting.json"
  [[ -f "$json" ]] || json="$HOME/.freerouting/freerouting.json"
  if [[ ! -f "$json" ]]; then
    printf 'no freerouting.json (priority 10 contributes nothing)'
    return 0
  fi
  if python3 -c '
import json, sys
router = json.load(open(sys.argv[1])).get("router", {})
sys.exit(0 if all(not v for v in router.values()) and set(router) <= {"fanout", "optimizer", "scoring"} else 1)
' "$json" 2>/dev/null; then
    printf 'freerouting.json present, its (router …) scope empty (priority 10 contributes nothing)'
  else
    printf 'freerouting.json present and its (router …) scope is NOT empty — the port has no priority-10 source, so this machine cannot generate a faithful reference'
  fi
}

# --- compile the driver once ---------------------------------------------------------------------
compile_driver() {
  echo "== compiling $(portable "$DRIVER") against $(portable "$JAR")"
  rm -rf "$CLASSES"
  mkdir -p "$CLASSES"
  # `P7T2.java` carries the board/settings/router ladder every `p7t*` driver shares, so it is
  # compiled alongside exactly as `run.sh`'s `p7t9` case compiles it.
  "$JAVAC_BIN" -cp "$JAR" -d "$CLASSES" "$DRIVER" "$ROOT/scripts/differential/java/P7T2.java"
}

each_row() {
  local body="$1" stem dsn max_items ripup max_passes fanout optimizer
  while IFS='|' read -r stem dsn max_items ripup max_passes fanout optimizer || [[ -n "$stem" ]]; do
    [[ -z "$stem" || "$stem" == \#* ]] && continue
    # A row with no batch columns is a per-connection-only row (`gen-router-reference.sh`'s).
    [[ -n "$max_passes" && "$max_passes" != "-" ]] || continue
    wanted "$stem" || continue
    "$body" "$stem" "$dsn" "$max_passes" "$fanout" "$optimizer"
  done < "$FIXTURES"
}

STATUS=0

generate_one() {
  local stem="$1" dsn="$2" max_passes="$3" fanout="$4" optimizer="$5" out="$REF/$1"
  mkdir -p "$out"
  if [[ "$FORCE" -eq 0 && -s "$out/batch.ses" && -f "$out/batch.passes.jsonl" ]]; then
    echo "== $stem (already generated; --force to redo)"
    return 0
  fi
  echo "== $stem  maxPasses=$max_passes fanout=$fanout optimizer=$optimizer"
  local started
  started="$(date +%s)"
  # Into temporaries, moved only after the JVM exits 0 and left a non-empty SES behind, so a
  # failed or timed-out run leaves the committed reference untouched.
  local tmp_ses="$out/batch.ses.tmp" tmp_passes="$out/batch.passes.jsonl.tmp"
  rm -f "$tmp_ses" "$tmp_passes"
  if ! run_driver "$out/java.batch.log" "$HASH_MODE" \
      "$JAVA_DIR/$dsn" "$max_passes" batch --fanout "$fanout" --optimizer "$optimizer" \
      --ses "$tmp_ses" || [[ ! -s "$tmp_ses" ]]; then
    echo "   the batch driver failed for $stem; see $(portable "$out/java.batch.log")" >&2
    rm -f "$tmp_ses"
    STATUS=1
    return 0
  fi
  # The pass tuples: the same board and settings with the optimizer off, routing stage only.
  if ! run_driver "$out/java.batch-router.log" "$HASH_MODE" \
      "$JAVA_DIR/$dsn" "$max_passes" batch-router --fanout "$fanout" --optimizer off \
      --passes "$tmp_passes" || [[ ! -f "$tmp_passes" ]]; then
    echo "   the batch-router driver failed for $stem; see $(portable "$out/java.batch-router.log")" >&2
    rm -f "$tmp_ses" "$tmp_passes"
    STATUS=1
    return 0
  fi
  mv "$tmp_ses" "$out/batch.ses"
  mv "$tmp_passes" "$out/batch.passes.jsonl"
  commit_log "$out/java.batch.log"
  commit_log "$out/java.batch-router.log"
  write_meta "$stem" "$dsn" "$max_passes" "$fanout" "$optimizer"
  echo "   $(wc -c < "$out/batch.ses" | tr -d ' ') B SES, $(wc -l < "$out/batch.passes.jsonl" | tr -d ' ') passes, $(( $(date +%s) - started ))s"
}

meta_one() {
  local stem="$1" dsn="$2" max_passes="$3" fanout="$4" optimizer="$5" out="$REF/$1"
  echo "== $stem"
  if [[ ! -s "$out/batch.ses" ]]; then
    echo "   no batch.ses to describe; run without --meta-only first" >&2
    STATUS=1
    return 0
  fi
  write_meta "$stem" "$dsn" "$max_passes" "$fanout" "$optimizer"
  echo "   batch.meta.txt rewritten (batch.ses untouched)"
}

verify_driver_one() {
  local stem="$1" dsn="$2" max_passes="$3" fanout="$4" optimizer="$5" out="$REF/$1"
  mkdir -p "$out"
  echo "== $stem"
  local bare="$SCRATCH/$stem.bare.ses" driver="$SCRATCH/$stem.driver.ses"
  if ! run_bare_jar "$SCRATCH/$stem.bare.log" "$HASH_MODE" "$JAVA_DIR/$dsn" "$max_passes" \
      "$(bool_of "$fanout")" "$(bool_of "$optimizer")" "$bare" || [[ ! -s "$bare" ]]; then
    echo "   the bare jar failed for $stem; see $SCRATCH/$stem.bare.log" >&2
    STATUS=1
    return 0
  fi
  if ! run_driver "$SCRATCH/$stem.driver.log" "$HASH_MODE" \
      "$JAVA_DIR/$dsn" "$max_passes" batch --fanout "$fanout" --optimizer "$optimizer" \
      --ses "$driver" || [[ ! -s "$driver" ]]; then
    echo "   the driver failed for $stem; see $SCRATCH/$stem.driver.log" >&2
    STATUS=1
    return 0
  fi
  if cmp -s "$bare" "$driver"; then
    {
      echo "bare-jar     identical"
      echo "             P7T9 mode batch and \`java -jar <jar> -de … -do …\` produced the same"
      echo "             SES byte for byte ($(wc -c < "$bare" | tr -d ' ') B), so the driver is the jar."
    } > "$out/batch.verify-driver.txt"
    echo "   bare-jar: identical ($(wc -c < "$bare" | tr -d ' ') B)"
  else
    # Controller answer 1: a difference is informational only if the budget actually fired.
    local trips first
    first="$(cmp "$bare" "$driver" 2>&1 | head -1)"
    TRIP_LOG="$SCRATCH/$stem.jar.log"
    rm -f "$TRIP_LOG"
    run_driver "$SCRATCH/$stem.trips.log" "$HASH_MODE" \
        "$JAVA_DIR/$dsn" "$max_passes" batch --fanout "$fanout" --optimizer "$optimizer" \
        --ses "$SCRATCH/$stem.trips.ses" || true
    if [[ -s "$TRIP_LOG" ]]; then
      trips="$(grep -c "$TRIP_MESSAGE" "$TRIP_LOG" || true)"
    else
      trips=unmeasured
    fi
    TRIP_LOG=""
    {
      echo "bare-jar     budget-tripped ($trips)"
      echo "             $first"
      if [[ "$trips" == "0" ]]; then
        echo "             ZERO trips with a difference: the driver is NOT the jar. FAILURE."
      fi
    } > "$out/batch.verify-driver.txt"
    echo "   bare-jar: DIFFERS — $first; budget trips: $trips" >&2
    [[ "$trips" == "0" ]] && STATUS=1
  fi
  # Keep batch.meta.txt in step with the verdict just written, when there is one to describe.
  [[ -s "$out/batch.ses" ]] && write_meta "$stem" "$dsn" "$max_passes" "$fanout" "$optimizer"
  return 0
}

sweep_one() {
  local stem="$1" dsn="$2" max_passes="$3" fanout="$4" optimizer="$5" mode ses
  echo "== $stem"
  local digests=() sizes=()
  for mode in 0 1 2 3 4; do
    ses="$SCRATCH/$stem-h$mode.ses"
    if run_driver "$SCRATCH/$stem-h$mode.log" "$mode" \
        "$JAVA_DIR/$dsn" "$max_passes" batch --fanout "$fanout" --optimizer "$optimizer" \
        --ses "$ses" && [[ -s "$ses" ]]; then
      digests+=("$(shasum -a 256 < "$ses" | cut -c1-12)")
      sizes+=("$(wc -c < "$ses" | tr -d ' ')")
    else
      # Never `continue`: the arrays are indexed by mode below.
      digests+=("FAILED------")
      sizes+=("?")
      STATUS=1
    fi
  done
  local distinct
  distinct="$(printf '%s\n' "${digests[@]}" | sort -u | wc -l | tr -d ' ')"
  if [[ "$distinct" == "1" && "${digests[0]}" != "FAILED------" ]]; then
    echo "   5 modes agree (${digests[0]}, ${sizes[0]} B)"
  else
    echo "   $distinct distinct SES files over 5 modes:" >&2
    for mode in 0 1 2 3 4; do
      echo "     hashCode=$mode ${digests[$mode]} bytes=${sizes[$mode]}" >&2
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
    [[ "$STATUS" -ne 0 ]] && echo "hash-mode sweep found a disagreement" >&2
    ;;
  verify-driver)
    SCRATCH="$(mktemp -d)"
    trap 'rm -rf "$SCRATCH"' EXIT
    each_row verify_driver_one
    ;;
  meta)
    each_row meta_one
    ;;
  generate)
    each_row generate_one
    echo "done. batch references in $REF"
    ;;
esac
exit "$STATUS"
