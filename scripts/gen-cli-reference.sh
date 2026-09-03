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
# `freerouting -de … -do …`, each with the budget it really ships with. No probe, no
# `-Dfreerouting.logging.file.*` plumbing, no trip count. The asymmetry Plan 7 had to argue around
# does not exist here.
#
# ## What the two sides' budgets are, and why they stopped being the same (Plan 9 Task 1, #234)
#
# Until Task 1 the port's CLI ran `fr_core::RouterBudget::default()` — Java's own four literals,
# 1000 / 10000 / 250 / 1000 — so both sides ran the same live budget and the paragraph here said
# so. The cost was stated plainly at the time: a wall-clock budget live on both sides is a
# *machine-speed* dependency, and a stem whose optimizer genuinely tripped the 1000 ms limit could
# answer different bytes on a slower host.
#
# **#234 removed that risk from the port's side.** `RouterBudget::default()` is now
# `opt_changed_area_ms = 0` — Java's own "off" value (`TraceTightener.java:73-77`'s `> 0` guard) —
# so the port's pull-tight always runs to completion and its bytes do not depend on the host. The
# other three literals are unchanged. The jar cannot be given the same treatment: its constant is
# `javac`-inlined and no flag or reflection reaches it, which is the whole of quirk #234.
#
# Measured, and it is why this was safe to do in the same task as the freeze: **the 1000 ms limit
# takes 0 trips on all eight whole-board stems** in their reference configuration, with the jar's
# DEBUG log on (`crates/fr-router/README.md`'s acceptance table), so the two budgets produce the
# same bytes on this corpus. Task 1 re-checked it from the other direction — it ran the whole
# regeneration, and **no golden moved**: every SES was byte-identical bar quirk #92's two head
# tokens. That measurement is why ruling BT deferred the regeneration itself. The `batch.ses` cross-check below is still the standing guard: it fails
# loudly if this generator's bare-jar run and Plan 7's `RouterBudget::disabled()`-side reference
# ever disagree, which is exactly the signal a trip would produce.
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
# ## Two lanes (Plan 9 Task 0)
#
#   --jar         `java -jar <jar> -de … -do …`. The historical lane and still the **default**.
#                 It cut the frozen baseline (`tests/reference-frozen/`, Task 1) and it is the
#                 triage lane for a port-cut golden nobody can explain.
#   --from-port   `target/release/freerouting` with **the same argv**. This is the whole lane
#                 switch: what `p8t1` compares is two whole programs on one command line, so
#                 cutting the reference from the port is a one-word substitution — the argv, the
#                 `-mp` cap, the two runs per stem and the `batch.ses` cross-check are unchanged.
#
# Byte parity with the jar may break from Plan 9 on, and that is the point; the `--jar` lane is a
# diagnosis, not a gate. **The `batch.ses` cross-check keeps its meaning in both lanes and stays a
# failure**: within a lane the two generators must still agree, and a lane switch that broke that
# agreement would mean the two families had drifted apart rather than moved together.
#
# ## Usage
#
#   scripts/gen-cli-reference.sh [--jar] [stem ...]   generate all stems, or just the named
#   scripts/gen-cli-reference.sh --force [stem ...]  regenerate even where outputs exist (without
#                                                    it the run is resumable: a stem whose
#                                                    route.ses and route.exit are both present is
#                                                    skipped)
#   scripts/gen-cli-reference.sh --from-port [--task T<n>] [stem ...]
#                                                    the same, driven by the port; meta.txt then
#                                                    carries the port's git sha, the Plan 9 task
#                                                    at that sha and the RouterBudget in force
#   scripts/gen-cli-reference.sh --meta-only [stem ...]
#                                                    rewrite meta.txt from the existing outputs
#   scripts/gen-cli-reference.sh --verify-hash-modes [stem ...]
#   scripts/gen-cli-reference.sh --from-port --verify-two-runs [stem ...]
#                                                    the PORT's determinism check: every stem run
#                                                    twice, byte-identical SES required. The
#                                                    successor to --verify-hash-modes, which has
#                                                    no meaning on a side with no hashCode axis.
#                                                    regenerate each stem's SES once per
#                                                    -XX:hashCode=0..4 into a scratch dir and
#                                                    require five byte-identical files; writes
#                                                    nothing under tests/reference/
#
# ## Environment
#
# FREEROUTING_JAVA_DIR (default ../freerouting), FREEROUTING_JAR, JAVA, CLI_HASH_MODE, CLI_TIMEOUT,
# PLAN9_TASK, REFERENCE_OUT_ROOT (default tests/reference — point it at a scratch tree to generate
# without touching the committed references; the fixture tables and the `batch.ses` cross-check
# are always read from tests/reference).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
JAVA_DIR="$(cd "${FREEROUTING_JAVA_DIR:-$ROOT/../freerouting}" && pwd)"
JAR="${FREEROUTING_JAR:-$JAVA_DIR/build/libs/freerouting-current-executable.jar}"
JAVA_BIN="${JAVA:-/opt/homebrew/opt/openjdk@25/bin/java}"
REF="$ROOT/tests/reference"
FIXTURES="$REF/cli-fixtures.txt"
# Outputs may be redirected to a scratch tree; inputs and the cross-check never are.
OUT_ROOT="${REFERENCE_OUT_ROOT:-$REF}"
PORT_BIN="$ROOT/target/release/freerouting"

# `-XX:hashCode=2`, `gen-batch-reference.sh`'s constant-hash mode and the same argument for it:
# the router's own containers are `TreeSet`/`TreeMap`/`LinkedHashMap` throughout (plan-6 ruling 4),
# so the output is not *expected* to depend on the mode, and `--verify-hash-modes` is what turns
# that expectation into evidence.
HASH_MODE="${CLI_HASH_MODE:-2}"
TIMEOUT_SECONDS="${CLI_TIMEOUT:-7200}"
LOCALE_FLAGS=(-Djava.awt.headless=true -Duser.language=en -Duser.country=US)

MODE=generate
FORCE=0
LANE=jar
TASK="${PLAN9_TASK:-}"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --verify-hash-modes) MODE=sweep; shift ;;
    --verify-two-runs) MODE=tworuns; shift ;;
    --meta-only) MODE=meta; shift ;;
    --force) FORCE=1; shift ;;
    --jar) LANE=jar; shift ;;
    --from-port) LANE=port; shift ;;
    --task) TASK="${2:?--task needs a task id, e.g. T1}"; shift 2 ;;
    --task=*) TASK="${1#--task=}"; shift ;;
    --*) echo "error: unknown option $1" >&2; exit 1 ;;
    *) break ;;
  esac
done
WANTED=("$@")
if [[ "$MODE" == tworuns && "$LANE" != port ]]; then
  echo "error: --verify-two-runs is the PORT's determinism check and needs --from-port." >&2
  echo "       The jar's analogue is --verify-hash-modes; the jar has an -XX:hashCode axis" >&2
  echo "       and the port has none, which is why they are two flags and not one." >&2
  exit 1
fi
if [[ "$LANE" == port && "$MODE" == sweep ]]; then
  echo "error: --verify-hash-modes is a JVM sweep and has no meaning in --from-port mode" >&2
  exit 1
fi

# --- preflight -----------------------------------------------------------------------------------
if [[ "$LANE" == jar ]]; then
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
else
  echo "== building the port's binary (release)"
  (cd "$ROOT" && cargo build --release --bin freerouting --quiet)
  PORT_SHA="$(cd "$ROOT" && git rev-parse --short=12 HEAD 2>/dev/null || echo unknown)"
  PORT_DIRTY=""
  if ! (cd "$ROOT" && git diff --quiet HEAD -- crates 2>/dev/null); then
    PORT_DIRTY=" +uncommitted-changes-under-crates"
  fi
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
  if [[ "$LANE" == port ]]; then
    "${TIMEOUT[@]}" "$PORT_BIN" "$@" > "$log" 2>&1 < /dev/null
  else
    "${TIMEOUT[@]}" "$JAVA_BIN" "${LOCALE_FLAGS[@]}" \
        -XX:+UnlockExperimentalVMOptions "-XX:hashCode=$hash" \
        -jar "$JAR" "$@" > "$log" 2>&1 < /dev/null
  fi
}

# The jar's stdout and stderr, separately, so `route.log` can carry them in that order. The jar
# writes INFO/WARN to stdout and duplicates ERROR to stderr (quirk #261), and `normalize_log`
# dedupes, so the concatenation is lossless for what is compared.
run_jar_split() {
  local out="$1" err="$2" hash="$3"
  shift 3
  if [[ "$LANE" == port ]]; then
    "${TIMEOUT[@]}" "$PORT_BIN" "$@" > "$out" 2> "$err" < /dev/null
  else
    "${TIMEOUT[@]}" "$JAVA_BIN" "${LOCALE_FLAGS[@]}" \
        -XX:+UnlockExperimentalVMOptions "-XX:hashCode=$hash" \
        -jar "$JAR" "$@" > "$out" 2> "$err" < /dev/null
  fi
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
# `parity::normalize_ses_head_tokens`, in `sed`: the four `(head …)`-vs-`(specctra …)` spellings
# `io/specctra/parser/Parser.java:102-135` differs on between the clone's HEAD and the 2.3.0 jar.
# Kept to those four and anchored to the token's own opening paren, so it can only ever rewrite a
# head token and never a net name or a component id.
ses_equal_mod_head_tokens() {
  local norm='s/(hostCad /(host_cad /; s/(hostVersion /(host_version /; s/(stringQuote /(string_quote /; s/(writeResolution /(write_resolution /'
  diff -q <(sed "$norm" "$1") <(sed "$norm" "$2") > /dev/null
}

batch_ses_of() {
  local stem="$1"
  { [[ -f "$REF/$stem/batch.ses" ]] && printf '%s' "$REF/$stem/batch.ses"; } || true
}

write_meta() {
  local stem="$1" dsn="$2" extra="$3" lane="$4" out="$OUT_ROOT/cli-$1"
  build_argv "<OUT>/route.ses" "$dsn" "$extra"
  {
    if [[ "$LANE" == port ]]; then
      # The three provenance lines a port-cut golden carries (Plan 9 Task 0). A golden cut before
      # a later fix must be *loudly* invalid, and this is what makes it so.
      echo "generated by port (target/release/freerouting)"
      echo "port sha     ${PORT_SHA}${PORT_DIRTY}"
      if [[ -n "$TASK" ]]; then
        echo "plan 9 task  $TASK"
      else
        echo "plan 9 task  UNKNOWN — this golden names no task and is therefore INVALID as a"
        echo "             reference; re-cut it with --task T<n>. See this script's header."
      fi
      echo "java         n/a (no JVM runs in this lane)"
      echo "hash mode    n/a (the port has no Object.hashCode)"
    else
      echo "jar          $(portable "$JAR")"
      echo "jar size     $(wc -c < "$JAR" | tr -d ' ') bytes"
      echo "jar mtime    $(date -r "$JAR" '+%Y-%m-%d %H:%M:%S %z')"
      echo "jar revision $(unzip -p "$JAR" META-INF/MANIFEST.MF 2>/dev/null \
          | tr -d '\r' | sed -n 's/^Build-Revision: *//p' | head -1)"
      echo "java         $("$JAVA_BIN" -version 2>&1 | head -1)"
      echo "hash mode    -XX:hashCode=$HASH_MODE"
    fi
    echo "lane         $lane"
    if [[ "$LANE" == port ]]; then
      echo "budget       fr_core::RouterBudget::default() — what a user gets. Since Plan 9"
      echo "             Task 1 (#234) that is opt_changed_area_ms = 0, Java's own off"
      echo "             value (TraceTightener.java:73-77), so the pull-tight runs to"
      echo "             completion and these bytes do not depend on how fast this host"
      echo "             is. The other three keep Java's literals: fanout_ms_per_pin"
      echo "             10000, board_update_throttle_ms 250, progress_throttle_ms 1000."
      echo "             Nothing is disabled — RouterBudget::disabled() is the parity"
      echo "             drivers' configuration and is stricter than this one."
    else
      echo "budget       LIVE. Unlike scripts/gen-batch-reference.sh, this generator needs"
      echo "             no probe and disables nothing: what it compares is two whole"
      echo "             programs, and the jar cannot switch its own javac-inlined"
      echo "             TIME_LIMIT_TO_PREVENT_ENDLESS_LOOP off (quirk #234). So this lane"
      echo "             runs Java's four literals, 1000/10000/250/1000, live — and the"
      echo "             machine-speed dependency that implies is bounded by measurement"
      echo "             (0 trips on all eight whole-board stems) and by the batch.ses"
      echo "             cross-check below, not by hope. See this script's header."
    fi
    if [[ "$LANE" == port ]]; then
      printf 'command      freerouting %s\n' "$(portable "${ARGV[*]}")"
      printf 'manifest cmd freerouting %s --router.result_json=<OUT>/manifest.json\n' \
          "$(portable "${ARGV[*]}")"
    else
      printf 'command      java %s -XX:+UnlockExperimentalVMOptions -XX:hashCode=%s -jar <jar> %s\n' \
          "${LOCALE_FLAGS[*]}" "$HASH_MODE" "$(portable "${ARGV[*]}")"
      printf 'manifest cmd java %s -XX:+UnlockExperimentalVMOptions -XX:hashCode=%s -jar <jar> %s --router.result_json=<OUT>/manifest.json\n' \
          "${LOCALE_FLAGS[*]}" "$HASH_MODE" "$(portable "${ARGV[*]}")"
    fi
    echo "exit code    $(cat "$out/route.exit")"
    echo "ses bytes    $(wc -c < "$out/route.ses" | tr -d ' ')"
    echo "ses sha256   $(shasum -a 256 < "$out/route.ses" | cut -d' ' -f1)"
    local batch
    batch="$(batch_ses_of "$stem")"
    if [[ -n "$batch" ]]; then
      if cmp -s "$out/route.ses" "$batch"; then
        echo "batch cross-check  identical to tests/reference/$stem/batch.ses"
      elif ses_equal_mod_head_tokens "$out/route.ses" "$batch"; then
        # Quirk #92: HEAD's writer spells four `(session (base_design …))` head tokens in
        # camelCase and the port's writes 2.3.0's snake_case, which survey §9.1 keeps as a
        # *decision* (HEAD's own lexer cannot read HEAD's own output back). Every port-vs-jar SES
        # comparison in the tree runs through `parity::normalize_ses_head_tokens` for that reason,
        # and this cross-check does the same. It is only ever reached while the two families sit
        # in **different** lanes — a port-cut `route.ses` against a jar-cut `batch.ses` — which
        # is the state from Plan 9 Task 0 until the first fix that moves one of the two families
        # (ruling BT: a family regenerates on measured movement, not on a schedule, so the two can
        # sit in different lanes for several tasks). Once both are port-cut the normalisation is a
        # no-op and the plain `cmp` above answers first.
        echo "batch cross-check  identical to tests/reference/$stem/batch.ses after quirk-#92"
        echo "                   head-token normalisation (this lane is $LANE and that reference"
        echo "                   was cut in the other one)"
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
  local stem="$1" dsn="$2" extra="$3" lane="$4" out="$OUT_ROOT/cli-$1"
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
    echo "   the $LANE lane wrote no SES for $stem (exit $exit_code); see $jout" >&2
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
    echo "   the $LANE lane wrote no manifest for $stem; see $SCRATCH/$stem.manifest.log" >&2
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
  local stem="$1" dsn="$2" extra="$3" lane="$4" out="$OUT_ROOT/cli-$1"
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

# `--verify-two-runs` — the port-side successor to `--verify-hash-modes` (Plan 9 Task 1).
#
# The jar's sweep asks "does the answer depend on `Object.hashCode`?", because `HashMap` iteration
# order feeds several of Java's routing collections. **The port has no such axis**: its collections
# are `BTreeMap`/`BTreeSet` by construction, so the same sweep would have one arm and prove
# nothing. What survives of the question is the half that still has teeth — *does this program
# answer the same bytes twice?* — and #234 is why it is worth asking, because a wall-clock budget
# is the one thing that could make the answer depend on how busy the machine is.
#
# Two runs here over all thirteen stems; `crates/freerouting/tests/cli_e2e.rs::
# two_runs_of_every_ci_stem_are_byte_identical` is the same assertion over the four CI stems, in
# the lane that runs on every `cargo nextest`. Two runs on one machine plus one run on CI is the
# equivalent of the jar's five-mode sweep, and it is part of G1 for the rest of Plan 9.
two_runs_one() {
  local stem="$1" dsn="$2" extra="$3" pass ses
  echo "== cli-$stem"
  local digests=() sizes=()
  for pass in 1 2; do
    ses="$SCRATCH/$stem-run$pass.ses"
    build_argv "$ses" "$dsn" "$extra"
    if run_jar "$SCRATCH/$stem-run$pass.log" "" "${ARGV[@]}" && [[ -s "$ses" ]]; then
      digests+=("$(shasum -a 256 < "$ses" | cut -c1-12)")
      sizes+=("$(wc -c < "$ses" | tr -d ' ')")
    else
      digests+=("FAILED------")
      sizes+=("?")
      STATUS=1
    fi
  done
  if [[ "${digests[0]}" == "${digests[1]}" && "${digests[0]}" != "FAILED------" ]]; then
    echo "   2 runs agree (${digests[0]}, ${sizes[0]} B)"
  else
    echo "   NOT REPRODUCIBLE — two runs of the same argv on the same binary disagree:" >&2
    echo "     run 1 ${digests[0]} bytes=${sizes[0]}" >&2
    echo "     run 2 ${digests[1]} bytes=${sizes[1]}" >&2
    echo "   Something in this run depends on the machine rather than on the board." >&2
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
  sweep)   each_row sweep_one ;;
  tworuns) each_row two_runs_one ;;
  meta)    each_row meta_one ;;
  *)       each_row generate_one ;;
esac
exit "$STATUS"
