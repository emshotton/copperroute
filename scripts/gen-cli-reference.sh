#!/usr/bin/env bash
# Generate the **whole-program** CLI references for `crates/freerouting/tests/cli_e2e.rs` and for
# the `p8t1`/`p8t2` differential drivers (Plan 8 Task 6, ruling AV — the plan's headline gate).
#
# Sibling of `scripts/gen-batch-reference.sh`, sharing its `portable()`, its preflight, its
# `timeout(1)` bound, its per-invocation scratch and the same jar: the clone's **HEAD** build
# (`freerouting-current-executable.jar`). It reads `tests/reference/cli-fixtures.txt`.
#
# Per stem it writes, into tests/reference/cli-<stem>/:
#
#   argv.txt      the argv, one token per line, with `<JAVA_DIR>` and `<OUT>` placeholders so the
#                 file is the same on every machine and both sides of `p8t1` build the same run
#   route.ses     the bare jar's SES, **verbatim**
#   route.exit    its exit code
#   route.log     its stdout followed by its stderr, with every machine-specific prefix replaced.
#                 `parity::normalize_log` is what compares it — it strips log4j2's
#                 `"%d{yyyy-MM-dd HH:mm:ss.SSS} %-6level "` prefix, drops the donation banner and
#                 keeps only the lines `freerouting::logging::MESSAGE_MAP` names, so the two
#                 streams may be concatenated in either order here without changing the answer
#   manifest.json the manifest a **second** run with `--router.result_json=<f>` wrote (`p8t2`)
#   meta.txt      the jar identity, `java -version`, the hash mode, the budget note, the two
#                 command lines and the `batch.ses` cross-check verdict
#
# ## The budget difference from Plan 7 — read this before wondering where the probe went
#
# `gen-batch-reference.sh` runs `P7T9.java` rather than the bare jar and spends a whole section of
# its header on `optChangedArea`'s `TIME_LIMIT_TO_PREVENT_ENDLESS_LOOP`: ruling AI wants the 1000 ms
# budget off on both sides, `javac` inlines the constant (`sipush 1000` at every call site) so no
# flag and no reflection reaches it, and Plan 7's answer is to run the **Java** side with the live
# limit and the **port** side with `RouterBudget::disabled()`, then treat a byte-identical SES as
# evidence that the limit never changed the result. `--verify-driver` counts the jar's own
# `TraceTightener.is_stop_requested: time limit exceeded` DEBUG lines to turn a difference into a
# diagnosis.
#
# **This generator needs none of that, and the reason is not that it was forgotten.** What `p8t1`
# compares is two *whole programs*: `java -jar <jar> -de … -do …` against
# `freerouting -de … -do …`. The port's CLI runs `fr_core::RouterBudget::default()` — Java's own
# four literals, 1000 / 10000 / 250 / 1000 — because that is what a user gets, so **both sides run
# the budget live** and there is nothing to disable on either. No probe, no
# `-Dfreerouting.logging.file.*` plumbing, no trip count. The asymmetry Plan 7 had to argue around
# does not exist here.
#
# The cost is stated plainly: a wall-clock budget that is live on both sides is a *machine-speed*
# dependency, so a stem whose optimizer genuinely trips the 1000 ms limit could answer different
# bytes on a slower host. That is a real risk and it is bounded by measurement rather than by
# hope — the `batch.ses` cross-check below fails loudly if this generator's bare-jar run and Plan
# 7's `RouterBudget::disabled()`-side reference ever disagree, which is exactly the signal a trip
# would produce.
#
# ## The `batch.ses` cross-check
#
# Eight of the stems (there are thirteen since Task 9's two KiCad-JSON boards; the file
# `tests/reference/cli-fixtures.txt` is the list) are Plan 7's batch stems run on **the same argv**
# `tests/reference/<stem>/batch.meta.txt`'s `bare jar` line records, and
# `gen-batch-reference.sh --verify-driver` already proved that argv's bare-jar output is
# byte-identical to `batch.ses` on all eight. So this generator asserts
# `cli-<stem>/route.ses == <stem>/batch.ses` and records the verdict in `meta.txt`. A mismatch is
# a **failure**, not a note: it means either the jar's answer moved or the argv drifted.
#
# ## Usage
#
#   scripts/gen-cli-reference.sh [stem ...]          generate all stems, or just the named
#   scripts/gen-cli-reference.sh --force [stem ...]  regenerate even where outputs exist (without
#                                                    it the run is resumable: a stem whose
#                                                    route.ses and route.exit are both present is
#                                                    skipped)
#   scripts/gen-cli-reference.sh --meta-only [stem ...]
#                                                    rewrite meta.txt from the existing outputs
#   scripts/gen-cli-reference.sh --verify-hash-modes [stem ...]
#                                                    regenerate each stem's SES once per
#                                                    -XX:hashCode=0..4 into a scratch dir and
#                                                    require five byte-identical files; writes
#                                                    nothing under tests/reference/
#
# ## Environment
#
# FREEROUTING_JAVA_DIR (default ../freerouting), FREEROUTING_JAR, JAVA, CLI_HASH_MODE, CLI_TIMEOUT.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
JAVA_DIR="$(cd "${FREEROUTING_JAVA_DIR:-$ROOT/../freerouting}" && pwd)"
JAR="${FREEROUTING_JAR:-$JAVA_DIR/build/libs/freerouting-current-executable.jar}"
JAVA_BIN="${JAVA:-/opt/homebrew/opt/openjdk@25/bin/java}"
REF="$ROOT/tests/reference"
FIXTURES="$REF/cli-fixtures.txt"

# `-XX:hashCode=2`, `gen-batch-reference.sh`'s constant-hash mode and the same argument for it:
# the router's own containers are `TreeSet`/`TreeMap`/`LinkedHashMap` throughout (plan-6 ruling 4),
# so the output is not *expected* to depend on the mode, and `--verify-hash-modes` is what turns
# that expectation into evidence.
HASH_MODE="${CLI_HASH_MODE:-2}"
TIMEOUT_SECONDS="${CLI_TIMEOUT:-7200}"
LOCALE_FLAGS=(-Djava.awt.headless=true -Duser.language=en -Duser.country=US)

MODE=generate
FORCE=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --verify-hash-modes) MODE=sweep; shift ;;
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
if [[ ! -x "$JAVA_BIN" ]]; then
  echo "error: java not found at $JAVA_BIN (need JDK >= 25; set JAVA)" >&2
  exit 1
fi
ver="$("$JAVA_BIN" -version 2>&1 | head -1 | sed -E 's/.*"([0-9]+).*/\1/')"
if [[ "${ver:-0}" -lt 25 ]]; then
  echo "error: Java 25+ required (found ${ver:-none}). On macOS: brew install openjdk@25" >&2
  exit 1
fi
if [[ ! -f "$FIXTURES" ]]; then
  echo "error: $FIXTURES not found" >&2
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

SCRATCH="$(mktemp -d "${TMPDIR:-/tmp}/gen-cli-reference.XXXXXX")"
cleanup() { [[ -n "$SCRATCH" ]] && rm -rf "$SCRATCH"; return 0; }
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

STATUS=0

# The argv for one stem, as an array, with `$1` the output path. Built in exactly one place so
# `argv.txt`, the two runs and `meta.txt` can never describe different command lines.
build_argv() {
  local out="$1" dsn="$2" extra="$3"
  ARGV=(-de "$JAVA_DIR/$dsn" -do "$out")
  if [[ "$extra" != "-" && -n "$extra" ]]; then
    # Word splitting is the point: `extra_args` is a space-separated argv fragment and no corpus
    # path or flag value in this table contains a space.
    # shellcheck disable=SC2206
    ARGV+=($extra)
  fi
}

# `argv.txt`: one token per line, with the two placeholders a consumer substitutes. `<JAVA_DIR>`
# is the Java checkout (so the DSN travels) and `<OUT>` is the SES path (so the consumer chooses
# its own scratch directory).
write_argv() {
  local file="$1"; shift
  local token
  : > "$file"
  for token in "$@"; do
    token="${token//$JAVA_DIR/<JAVA_DIR>}"
    printf '%s\n' "$token" >> "$file"
  done
}

# The bare jar, exactly as a user runs it. `$1` is the combined stdout+stderr log, `$2` the hash
# mode, the rest the program argv.
run_jar() {
  local log="$1" hash="$2"
  shift 2
  # `< /dev/null`, and it is load-bearing: `each_row` drives the fixture table through a
  # `while read` loop whose stdin is the file, and a JVM that reads stdin eats the rest of the
  # table. Measured — the first invocation without it swallowed every row after `tutorial_board`.
  "${TIMEOUT[@]}" "$JAVA_BIN" "${LOCALE_FLAGS[@]}" \
      -XX:+UnlockExperimentalVMOptions "-XX:hashCode=$hash" \
      -jar "$JAR" "$@" > "$log" 2>&1 < /dev/null
}

# The jar's stdout and stderr, separately, so `route.log` can carry them in that order. The jar
# writes INFO/WARN to stdout and duplicates ERROR to stderr (quirk #261), and `normalize_log`
# dedupes, so the concatenation is lossless for what is compared.
run_jar_split() {
  local out="$1" err="$2" hash="$3"
  shift 3
  "${TIMEOUT[@]}" "$JAVA_BIN" "${LOCALE_FLAGS[@]}" \
      -XX:+UnlockExperimentalVMOptions "-XX:hashCode=$hash" \
      -jar "$JAR" "$@" > "$out" 2> "$err" < /dev/null
}

# Make a log committable: replace every machine-specific prefix, exactly as `portable` does for
# the meta, so the committed file is the same on every machine.
commit_log() {
  local log="$1" tmp="$1.trunc"
  sed -e "s#$ROOT#<workspace>#g" -e "s#$JAVA_DIR#<FREEROUTING_JAVA_DIR>#g" \
      -e "s#$SCRATCH#<OUT>#g" "$log" > "$tmp" && mv "$tmp" "$log"
}

# `cli-<stem>` -> `<stem>/batch.ses`, the Plan 7 batch reference, when there is one.
#
# The `|| true` is not decoration: without it the `[[ -f … ]]` answers 1 for a stem with no batch
# reference, `batch="$(batch_dir_of …)"` inherits that status, and `set -e` kills the script
# mid-table — which is exactly what the first full run did, silently, after `tutorial_board`.
batch_ses_of() {
  local stem="$1"
  { [[ -f "$REF/$stem/batch.ses" ]] && printf '%s' "$REF/$stem/batch.ses"; } || true
}

write_meta() {
  local stem="$1" dsn="$2" extra="$3" lane="$4" out="$REF/cli-$1"
  build_argv "<OUT>/route.ses" "$dsn" "$extra"
  {
    echo "jar          $(portable "$JAR")"
    echo "jar size     $(wc -c < "$JAR" | tr -d ' ') bytes"
    echo "jar mtime    $(date -r "$JAR" '+%Y-%m-%d %H:%M:%S %z')"
    echo "jar revision $(unzip -p "$JAR" META-INF/MANIFEST.MF 2>/dev/null \
        | tr -d '\r' | sed -n 's/^Build-Revision: *//p' | head -1)"
    echo "java         $("$JAVA_BIN" -version 2>&1 | head -1)"
    echo "hash mode    -XX:hashCode=$HASH_MODE"
    echo "lane         $lane"
    echo "budget       LIVE ON BOTH SIDES. Unlike scripts/gen-batch-reference.sh, this generator"
    echo "             needs no probe and disables nothing: what p8t1 compares is two whole"
    echo "             programs, and the port's CLI runs fr_core::RouterBudget::default() —"
    echo "             Java's own 1000/10000/250/1000 literals — because that is what a user"
    echo "             gets. See this script's header for the full argument and for the risk"
    echo "             (a live wall clock is a machine-speed dependency) the batch.ses"
    echo "             cross-check below bounds."
    printf 'command      java %s -XX:+UnlockExperimentalVMOptions -XX:hashCode=%s -jar <jar> %s\n' \
        "${LOCALE_FLAGS[*]}" "$HASH_MODE" "$(portable "${ARGV[*]}")"
    printf 'manifest cmd java %s -XX:+UnlockExperimentalVMOptions -XX:hashCode=%s -jar <jar> %s --router.result_json=<OUT>/manifest.json\n' \
        "${LOCALE_FLAGS[*]}" "$HASH_MODE" "$(portable "${ARGV[*]}")"
    echo "exit code    $(cat "$out/route.exit")"
    echo "ses bytes    $(wc -c < "$out/route.ses" | tr -d ' ')"
    echo "ses sha256   $(shasum -a 256 < "$out/route.ses" | cut -d' ' -f1)"
    local batch
    batch="$(batch_ses_of "$stem")"
    if [[ -n "$batch" ]]; then
      if cmp -s "$out/route.ses" "$batch"; then
        echo "batch cross-check  identical to tests/reference/$stem/batch.ses"
      else
        echo "batch cross-check  **DIFFERS** from tests/reference/$stem/batch.ses — FAILURE"
        echo "                   $(cmp "$out/route.ses" "$batch" 2>&1 | head -1 || true)"
      fi
    else
      echo "batch cross-check  n/a (no tests/reference/$stem/batch.ses)"
    fi
  } > "$out/meta.txt"
}

generate_one() {
  local stem="$1" dsn="$2" extra="$3" lane="$4" out="$REF/cli-$1"
  mkdir -p "$out"
  if [[ "$FORCE" -eq 0 && -s "$out/route.ses" && -f "$out/route.exit" ]]; then
    echo "== cli-$stem (already generated; --force to redo)"
    return 0
  fi
  echo "== cli-$stem  [$lane]  $extra"
  local started; started="$(date +%s)"
  local ses="$SCRATCH/$stem.ses" jout="$SCRATCH/$stem.out" jerr="$SCRATCH/$stem.err"
  rm -f "$ses" "$jout" "$jerr"
  build_argv "$ses" "$dsn" "$extra"
  local exit_code=0
  run_jar_split "$jout" "$jerr" "$HASH_MODE" "${ARGV[@]}" || exit_code=$?
  if [[ ! -f "$ses" ]]; then
    echo "   the jar wrote no SES for $stem (exit $exit_code); see $jout" >&2
    STATUS=1
    return 0
  fi
  # `route.log` is stdout then stderr; `normalize_log` dedupes the ERROR lines the jar writes to
  # both (quirk #261), so the order of the concatenation does not reach the comparison.
  cat "$jout" "$jerr" > "$SCRATCH/$stem.log"
  commit_log "$SCRATCH/$stem.log"

  # The manifest run: the same argv plus `--router.result_json=<f>`, which is `p8t2`'s.
  local manifest="$SCRATCH/$stem.manifest.json" ses2="$SCRATCH/$stem.2.ses"
  build_argv "$ses2" "$dsn" "$extra"
  run_jar "$SCRATCH/$stem.manifest.log" "$HASH_MODE" \
      "${ARGV[@]}" "--router.result_json=$manifest" || true
  if [[ ! -s "$manifest" ]]; then
    echo "   the jar wrote no manifest for $stem; see $SCRATCH/$stem.manifest.log" >&2
    STATUS=1
    return 0
  fi
  # The manifest's `settings_snapshot.result_json` is the scratch path this script chose, so it
  # is rewritten to `<OUT>` for a reader. It does not reach the comparison either way:
  # `parity::normalize_manifest` drops it along with `generated_at`, `git_sha`, `resource_usage`,
  # the phase durations and the two host-derived `max_threads` fields
  # (`Runtime.getRuntime().availableProcessors() - 1`, which is the machine and not the port).
  sed -e "s#$SCRATCH#<OUT>#g" "$manifest" > "$manifest.portable" && mv "$manifest.portable" "$manifest"

  build_argv "<OUT>/route.ses" "$dsn" "$extra"
  write_argv "$out/argv.txt" "${ARGV[@]}"
  cp "$ses" "$out/route.ses"
  printf '%s\n' "$exit_code" > "$out/route.exit"
  mv "$SCRATCH/$stem.log" "$out/route.log"
  cp "$manifest" "$out/manifest.json"
  write_meta "$stem" "$dsn" "$extra" "$lane"
  if grep -q 'DIFFERS' "$out/meta.txt"; then
    echo "   batch.ses cross-check FAILED — see $(portable "$out/meta.txt")" >&2
    STATUS=1
  fi
  echo "   exit $exit_code, $(wc -c < "$out/route.ses" | tr -d ' ') B SES, $(( $(date +%s) - started ))s"
}

meta_one() {
  local stem="$1" dsn="$2" extra="$3" lane="$4" out="$REF/cli-$1"
  echo "== cli-$stem"
  if [[ ! -s "$out/route.ses" ]]; then
    echo "   no route.ses to describe; run without --meta-only first" >&2
    STATUS=1
    return 0
  fi
  write_meta "$stem" "$dsn" "$extra" "$lane"
  echo "   meta.txt rewritten (route.ses untouched)"
}

sweep_one() {
  local stem="$1" dsn="$2" extra="$3" lane="$4" mode ses
  echo "== cli-$stem"
  local digests=() sizes=()
  for mode in 0 1 2 3 4; do
    ses="$SCRATCH/$stem-h$mode.ses"
    build_argv "$ses" "$dsn" "$extra"
    if run_jar "$SCRATCH/$stem-h$mode.log" "$mode" "${ARGV[@]}" && [[ -s "$ses" ]]; then
      digests+=("$(shasum -a 256 < "$ses" | cut -c1-12)")
      sizes+=("$(wc -c < "$ses" | tr -d ' ')")
    else
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

each_row() {
  local body="$1" stem dsn extra lane
  while IFS='|' read -r stem dsn extra lane || [[ -n "$stem" ]]; do
    [[ -z "$stem" || "$stem" == \#* ]] && continue
    wanted "$stem" || continue
    "$body" "$stem" "$dsn" "$extra" "${lane:-ci}"
  done < "$FIXTURES"
}

case "$MODE" in
  sweep) each_row sweep_one ;;
  meta)  each_row meta_one ;;
  *)     each_row generate_one ;;
esac
exit "$STATUS"
