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
# at DEBUG writing to a caller-chosen path. So the re-runs turn that appender on and grep the
# file; the verdict becomes `bare-jar: budget-tripped (bare=<n> driver=<m>)`.
#
# The two sides need **different knobs**, and it is not cosmetic: the driver takes the JVM system
# properties `-Dfreerouting.logging.file.{location,level}` /
# `-Dfreerouting.logging.console.enabled`, which the configuration factory reads directly, while
# the bare jar takes the program arguments `--logging.file.{location,level}` /
# `--logging.console.enabled`, because `Freerouting.main` (`Freerouting.java:1088-1098`)
# **overwrites** all five of those properties from its own argument/environment parse
# (`:1033-1066`) before logging initialises — so a `-D` on a `-jar` run is silently discarded
# (measured: no file at the requested path, on two boards). `resolveLogPath` (`:797-811`) also
# only treats a path as a file when it ends in `.log`.
#
# **Both sides are counted, and the bare jar leads**, because the trip count belongs to the run
# that differed and a difference *between* the two runs is explained by a trip on either of them.
# Two of the three outcomes are failures, and each says which it is:
#
#   * `bare` and `driver` both `0` — the driver is not the jar. FAILURE.
#   * either count `unmeasured` (the jar wrote no DEBUG log) — neither of answer 1's arms
#     applies, so the difference is unexplained rather than informational. FAILURE.
#   * any count > 0 — informational, exactly as answer 1 says.
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
# **Per invocation**, not a fixed path. `compile_driver` starts with `rm -rf "$CLASSES"`, so two
# concurrent invocations of this script sharing one directory delete each other's classes
# mid-run — observed live during the Task 16 review, and a resumable per-stem script invites
# exactly that. The directory is created after the preflight and removed by `cleanup`.
CLASSES=""

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

# One directory per invocation (see `CLASSES` above) and one scratch tree per invocation, both
# removed by a single EXIT trap so the two cannot overwrite each other's `trap` registration.
SCRATCH=""
mkdir -p "$ROOT/scripts/differential/build"
CLASSES="$(mktemp -d "$ROOT/scripts/differential/build/classes-gen-batch.XXXXXX")"
cleanup() {
  [[ -n "$CLASSES" ]] && rm -rf "$CLASSES"
  [[ -n "$SCRATCH" ]] && rm -rf "$SCRATCH"
  return 0
}
trap cleanup EXIT

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
# The path the jar's own DEBUG log goes to while a trip measurement is running; see
# `set_trip_log` below. Empty on every reference path.
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
  "${TIMEOUT[@]}" "$JAVA_BIN" "${LOCALE_FLAGS[@]}" ${TRIP_FLAGS+"${TRIP_FLAGS[@]}"} \
      -XX:+UnlockExperimentalVMOptions "-XX:hashCode=$hash" \
      -cp "$CLASSES:$JAR" app.freerouting.autoroute.pipeline.P7T9 "$@" \
      > "$log" 2>&1
}

# The exact message `TraceTightener.isStopRequested:209` logs on an exceeded budget.
TRIP_MESSAGE='TraceTightener.is_stop_requested: time limit exceeded'

# The bare jar, with the argv `P7T9.batchArgv` builds. Same JVM flags, same hash mode — and the
# same `TRIP_LOG` plumbing as `run_driver`, because answer 1's trip count belongs to the run that
# actually differed, which is this one.
run_bare_jar() {
  local log="$1" hash="$2" dsn="$3" max_passes="$4" fanout="$5" optimizer="$6" ses="$7"
  # `TRIP_ARGS` go after the routing switches and are `--logging.*`, which `CliSettings` ignores
  # (it reads `--router.*` only, `sources/CliSettings.java:53-55`), so they cannot move a setting.
  "${TIMEOUT[@]}" "$JAVA_BIN" "${LOCALE_FLAGS[@]}" \
      -XX:+UnlockExperimentalVMOptions "-XX:hashCode=$hash" \
      -jar "$JAR" \
      -de "$dsn" -do "$ses" -mp "$max_passes" \
      "--router.fanout.enabled=$fanout" \
      "--router.optimizer.enabled=$optimizer" \
      ${TRIP_ARGS+"${TRIP_ARGS[@]}"} \
      > "$log" 2>&1
}

# What makes the jar write its own DEBUG log to `$1`; see the header. Both are empty on every
# reference path, because turning DEBUG on costs wall-clock time and wall-clock time is what the
# budget measures.
#
# **The driver and the bare jar need different knobs, and that is not cosmetic.**
# `Log4j2ConfigurationFactory` reads *system properties*, so `-Dfreerouting.logging.*` is what
# reaches it — for the driver, which never enters `Freerouting.main`. The bare jar *does*, and
# `Freerouting.java:1088-1098` **overwrites all five of those properties** from its own
# command-line/environment parse (`:1033-1066`) before logging initialises, so a `-D` on a
# `-jar` run is silently discarded (measured: no file at the requested path, on two boards). The
# jar's own spellings are the `--logging.*` program arguments at `:1035-1051`; `resolveLogPath`
# (`:797-811`) treats a path as a file only when it ends in `.log`, which is why `set_trip_log`'s
# argument must.
TRIP_FLAGS=()
TRIP_ARGS=()
set_trip_log() {
  TRIP_LOG="$1"
  # `|| true`: an unwritable path must reach the `unmeasured` verdict below, not kill the script
  # under `set -e` before anything is written. Found by the unmeasured-arm probe in the fix round.
  rm -f "$TRIP_LOG" 2>/dev/null || true
  # The driver: JVM system properties, read straight by `Log4j2ConfigurationFactory`.
  TRIP_FLAGS=(
    -Dfreerouting.logging.console.enabled=false
    -Dfreerouting.logging.file.enabled=true
    "-Dfreerouting.logging.file.location=$TRIP_LOG"
    -Dfreerouting.logging.file.level=DEBUG
  )
  # The bare jar: program arguments, because `Freerouting.main` overwrites the properties.
  TRIP_ARGS=(
    --logging.console.enabled=false
    --logging.file.enabled=true
    "--logging.file.location=$TRIP_LOG"
    --logging.file.level=DEBUG
  )
}
clear_trip_log() {
  TRIP_LOG=""
  TRIP_FLAGS=()
  TRIP_ARGS=()
}

# The trip count of the run that just wrote `$1`, or `unmeasured` when the jar left no log —
# which `verify_driver_one` treats as a failure, not as a zero.
count_trips() {
  local log="$1"
  if [[ -s "$log" ]]; then
    grep -c "$TRIP_MESSAGE" "$log" || true
  else
    printf 'unmeasured'
  fi
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

# `JsonFileSettings` is priority 10 of the merge. Plan 8 Task 5 ported it (scan ruling R7), but
# `fr_settings::resolve_headless` does not take it yet — see the `// obligation:` in `resolve.rs` —
# so the reference generator's caveat still stands, with a narrower reason than the one it used to
# give: ~~"the one source the port has no counterpart for (spec §2 puts `freerouting.json` out of
# scope)"~~. It is machine state, so the meta records whether the file exists and whether its
# `router` scope is empty — the only shape in which the omission is a no-op.
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
  # **Every** output goes to a temporary and is moved into place only after both JVMs have exited
  # 0 and left their payload behind — the logs included. Writing the logs in place would let a
  # failed or timed-out run replace the committed, machine-normalised ones with raw failure
  # output, so a regeneration that aborted half way would show up as a `tests/reference/` diff
  # that has nothing to do with the reference. The four files move together or not at all.
  local tmp_ses="$out/batch.ses.tmp" tmp_passes="$out/batch.passes.jsonl.tmp"
  local tmp_log="$out/java.batch.log.tmp" tmp_router_log="$out/java.batch-router.log.tmp"
  rm -f "$tmp_ses" "$tmp_passes" "$tmp_log" "$tmp_router_log"
  if ! run_driver "$tmp_log" "$HASH_MODE" \
      "$JAVA_DIR/$dsn" "$max_passes" batch --fanout "$fanout" --optimizer "$optimizer" \
      --ses "$tmp_ses" || [[ ! -s "$tmp_ses" ]]; then
    echo "   the batch driver failed for $stem; its log is $(portable "$tmp_log")" >&2
    rm -f "$tmp_ses"
    STATUS=1
    return 0
  fi
  # The pass tuples: the same board and settings with the optimizer off, routing stage only.
  if ! run_driver "$tmp_router_log" "$HASH_MODE" \
      "$JAVA_DIR/$dsn" "$max_passes" batch-router --fanout "$fanout" --optimizer off \
      --passes "$tmp_passes" || [[ ! -f "$tmp_passes" ]]; then
    echo "   the batch-router driver failed for $stem; its log is $(portable "$tmp_router_log")" >&2
    rm -f "$tmp_ses" "$tmp_passes"
    STATUS=1
    return 0
  fi
  commit_log "$tmp_log"
  commit_log "$tmp_router_log"
  mv "$tmp_ses" "$out/batch.ses"
  mv "$tmp_passes" "$out/batch.passes.jsonl"
  mv "$tmp_log" "$out/java.batch.log"
  mv "$tmp_router_log" "$out/java.batch-router.log"
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
    #
    # **Which run is counted.** The trip count belongs to the run that differed, so the bare jar
    # is re-run first and is the number the verdict leads with. The driver is re-run too, because
    # a difference *between* the two runs is explained by a trip on **either** side: reporting
    # only one of them would answer "0 trips" — i.e. FAILURE — for a difference the other side's
    # trip caused. Both are re-runs rather than the original runs, because the DEBUG log the
    # measurement needs costs wall-clock time, and the original pair has to be measured with the
    # clock the references were generated under.
    local bare_trips driver_trips first
    # `|| true`: `cmp` exits 1 on a difference, which is the only way this branch is reached, and
    # `set -e` would otherwise kill the script here — before the verdict file is written and
    # before anything is printed. Found by the forced-difference probe in the Task 16 fix round;
    # the arm had never been executed.
    first="$(cmp "$bare" "$driver" 2>&1 | head -1 || true)"

    set_trip_log "$SCRATCH/$stem.bare-trips.jar.log"
    run_bare_jar "$SCRATCH/$stem.bare-trips.log" "$HASH_MODE" "$JAVA_DIR/$dsn" "$max_passes" \
        "$(bool_of "$fanout")" "$(bool_of "$optimizer")" "$SCRATCH/$stem.bare-trips.ses" || true
    bare_trips="$(count_trips "$TRIP_LOG")"

    set_trip_log "$SCRATCH/$stem.driver-trips.jar.log"
    run_driver "$SCRATCH/$stem.driver-trips.log" "$HASH_MODE" \
        "$JAVA_DIR/$dsn" "$max_passes" batch --fanout "$fanout" --optimizer "$optimizer" \
        --ses "$SCRATCH/$stem.driver-trips.ses" || true
    driver_trips="$(count_trips "$TRIP_LOG")"
    clear_trip_log

    {
      echo "bare-jar     budget-tripped (bare=$bare_trips driver=$driver_trips)"
      echo "             $first"
      if [[ "$bare_trips" == "unmeasured" || "$driver_trips" == "unmeasured" ]]; then
        # `unmeasured` is **not** "recorded trips": a difference whose trip count could not be
        # taken escapes both of answer 1's arms, so it is a failure of its own and says so.
        echo "             UNMEASURED trip count with a difference: neither of controller"
        echo "             answer 1's arms applies, because the jar wrote no DEBUG log. Rerun"
        echo "             --verify-driver, or fix the log knobs (-Dfreerouting.logging.file.*"
        echo "             for the driver, --logging.file.* for the bare jar). FAILURE."
      elif [[ "$bare_trips" == "0" && "$driver_trips" == "0" ]]; then
        echo "             ZERO trips on both sides with a difference: the driver is NOT the"
        echo "             jar. FAILURE."
      fi
    } > "$out/batch.verify-driver.txt"
    echo "   bare-jar: DIFFERS — $first; budget trips: bare=$bare_trips driver=$driver_trips" >&2
    if [[ "$bare_trips" == "unmeasured" || "$driver_trips" == "unmeasured" ]]; then
      echo "   trip count UNMEASURED — see $(portable "$out/batch.verify-driver.txt")" >&2
      STATUS=1
    elif [[ "$bare_trips" == "0" && "$driver_trips" == "0" ]]; then
      STATUS=1
    fi
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
    each_row sweep_one
    [[ "$STATUS" -ne 0 ]] && echo "hash-mode sweep found a disagreement" >&2
    ;;
  verify-driver)
    SCRATCH="$(mktemp -d)"
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
