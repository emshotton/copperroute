#!/usr/bin/env bash
# Generate the Java DRC references for `crates/fr-drc/tests/reference_parity.rs` (Plan 5 Task 9).
#
# Sibling of `scripts/gen-reference.sh`, not an extension of it (plan-5 ruling 10):
#
#   * that script is pinned to the downloaded **2.3.0** jar, because the `io/specctra` byte-parity
#     references have to be 2.3.0's (Plan 3 ruling 10). This one is pinned to the clone's **HEAD**
#     build, because the DRC port's sources and every file:line in Plan 5 are HEAD's (plan-5
#     ruling 1). Two jars behind one fixtures.txt would be an invisible skew;
#   * that script has to compile `RefWriter.java` against the jar, because the `-de/-do` job path
#     cannot write DSN headlessly. `-drc` needs no driver: it works from the CLI as-is, for a
#     plain DSN, for a DSN + `.rules` and for a DSN + `.ses`. Driving the real CLI is also what
#     gets `qualityScore` into the reference — `DesignRulesChecker` alone never computes it
#     (Freerouting.java:343-352, plan-5 ruling 5).
#
# Per stem in tests/reference/drc-fixtures.txt it writes, into tests/reference/<stem>/:
#
#   drc.json      the jar's report, **verbatim** — no post-processing at all
#   java.log      the CLI's stdout+stderr (absolute paths and wall-clock timestamps, like
#                 gen-reference.sh's; kept for debugging, read by nothing)
#   drc.meta.txt  the jar's identity, `java -version`, the hash mode and the command line, with
#                 every machine-specific prefix replaced by `<FREEROUTING_JAVA_DIR>` or
#                 `<workspace>` so the file is the same on every machine
#
# The reference is deliberately the raw bytes rather than a normalised document: normalisation is
# `parity::normalize_drc_json`'s job and the parity test applies it to **both** sides, so the
# committed file stays an unedited jar artifact and the two sides never have to agree on how a
# `double` is re-rendered. `scripts/normalize-drc.py` is the Python twin used by
# `--verify-hash-modes` only.
#
# ## Two lanes (Plan 9 Task 0)
#
#   --jar         the clone's HEAD jar, `java -jar <jar> -de … -drc <out>`. The historical lane
#                 and still the **default**, so every invocation that predates Plan 9 means what
#                 it meant before. It cut the frozen baseline (`tests/reference-frozen/`, Task 1)
#                 and it is the triage lane for a port-cut golden nobody can explain.
#   --from-port   `target/release/freerouting` with **the same argv**. `-drc` is the one mode
#                 where the two programs take literally the same command line, so this lane is a
#                 one-word substitution and nothing else about the run changes.
#
# Byte parity with the jar may break from Plan 9 on, and that is the point; the `--jar` lane is a
# diagnosis, not a gate.
#
# Usage:
#   scripts/gen-drc-reference.sh [--jar] [stem ...]  regenerate all stems, or just the named ones
#   scripts/gen-drc-reference.sh --from-port [--task T<n>] [stem ...]
#                                                  the same, driven by the port; drc.meta.txt then
#                                                  carries the port's git sha, the Plan 9 task at
#                                                  that sha and the RouterBudget in force
#   scripts/gen-drc-reference.sh --meta-only [stem ...]
#                                                  rewrite drc.meta.txt from the existing
#                                                  drc.json without running the jar — for when
#                                                  this script's meta format changes and the
#                                                  references themselves must not move
#   scripts/gen-drc-reference.sh --verify-hash-modes [stem ...]
#                                                  regenerate each stem once per
#                                                  -XX:hashCode=0..4 into a scratch dir and
#                                                  report how many distinct normalised documents
#                                                  come out; writes nothing under
#                                                  tests/reference/  (`--jar` only: the port has
#                                                  no `Object.hashCode` and no mode to sweep)
#
# Environment: FREEROUTING_JAVA_DIR (default ../freerouting), FREEROUTING_JAR, JAVA, DRC_HASH_MODE,
#              PLAN9_TASK, REFERENCE_OUT_ROOT (default tests/reference — point it at a scratch
#              tree to generate without touching the committed references; the fixture table is
#              always read from tests/reference).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
JAVA_DIR="${FREEROUTING_JAVA_DIR:-$ROOT/../freerouting}"
JAR="${FREEROUTING_JAR:-$JAVA_DIR/build/libs/freerouting-current-executable.jar}"
JAVA_BIN="${JAVA:-/opt/homebrew/opt/openjdk@25/bin/java}"
REF="$ROOT/tests/reference"
FIXTURES="$REF/drc-fixtures.txt"
# Outputs may be redirected to a scratch tree; inputs never are.
OUT_ROOT="${REFERENCE_OUT_ROOT:-$REF}"
PORT_BIN="$ROOT/target/release/freerouting"

# `-XX:hashCode=2` is the constant-hash mode: the only `Object.hashCode` source in the JVM that
# reproduces run to run *and* is not derived from an object address. Modes 0 and 5 are PRNG-
# seeded, 1 and 4 come from the address, 3 is a per-thread xorshift (deterministic only in a
# single-threaded run). The references are generated under it so that the one fixture whose
# report is genuinely hash-dependent — Natural Tone Preamp, see tests/reference/README.md — has a
# reproducible reference at all.
HASH_MODE="${DRC_HASH_MODE:-2}"

# `-Duser.language=en -Duser.country=US` is load-bearing, not hygiene: every `%.4f` in a violation
# description goes through `String.formatted`, which uses the default FORMAT locale, so a German
# JVM writes `expected: 0,0500 mm` (plan-5 ruling 6). The port's formatter is locale-free.
LOCALE_FLAGS=(-Djava.awt.headless=true -Duser.language=en -Duser.country=US)

MODE=generate
LANE=jar
TASK="${PLAN9_TASK:-}"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --verify-hash-modes) MODE=sweep; shift ;;
    --meta-only) MODE=meta; shift ;;
    --jar) LANE=jar; shift ;;
    --from-port) LANE=port; shift ;;
    --task) TASK="${2:?--task needs a task id, e.g. T1}"; shift 2 ;;
    --task=*) TASK="${1#--task=}"; shift ;;
    --*) echo "error: unknown option $1" >&2; exit 1 ;;
    *) break ;;
  esac
done
WANTED=("$@")
if [[ "$LANE" == port && "$MODE" == sweep ]]; then
  echo "error: --verify-hash-modes is a JVM sweep and has no meaning in --from-port mode" >&2
  exit 1
fi

# --- preflight ---------------------------------------------------------------------------------
if [[ "$LANE" == jar ]]; then
  if [[ ! -f "$JAR" ]]; then
    echo "error: HEAD jar not found at $JAR" >&2
    echo "       build it in the Java clone (./gradlew build) or set FREEROUTING_JAR" >&2
    exit 1
  fi
  ver="$("$JAVA_BIN" -version 2>&1 | head -1 | sed -E 's/.*"([0-9]+).*/\1/')"
  if [[ "${ver:-0}" -lt 25 ]]; then
    echo "error: Java 25+ required (found ${ver:-none}). On macOS: brew install openjdk@25" >&2
    echo "       then: export JAVA=/opt/homebrew/opt/openjdk@25/bin/java" >&2
    exit 1
  fi
else
  # The port's own binary, in release: a debug DRC over a wide board is minutes rather than
  # seconds, and every consumer of these references runs the release build.
  echo "== building the port's binary (release)"
  (cd "$ROOT" && cargo build --release --bin freerouting --quiet)
  PORT_SHA="$(cd "$ROOT" && git rev-parse --short=12 HEAD 2>/dev/null || echo unknown)"
  PORT_DIRTY=""
  if ! (cd "$ROOT" && git diff --quiet HEAD -- crates 2>/dev/null); then
    PORT_DIRTY=" +uncommitted-changes-under-crates"
  fi
fi

# Fills the global ARGS array with the CLI argv for one row.
#
# `-drc` is matched by `startsWith` *before* `-dr` in GlobalSettings' if-chain
# (GlobalSettings.java:660-674), so the two cannot be confused whatever the order on the command
# line. The SES has no flag of its own: `-de` swallows every following argument that does not
# start with `-` and sorts the list by extension (GlobalSettings.java:564-648), so the DSN and the
# SES are simply two arguments in that slot. (`<dsn>+<ses>` in a single argument reaches the same
# list through the `+`-split at `:573-579`; two arguments is the form that needs no split and
# survives a path containing `+`.)
ARGS=()
drc_args() {
  local dsn="$1" rules="$2" ses="$3" out="$4"
  ARGS=(-de "$JAVA_DIR/$dsn")
  [[ -n "$ses" ]] && ARGS+=("$JAVA_DIR/$ses")
  [[ -n "$rules" ]] && ARGS+=(-dr "$JAVA_DIR/$rules")
  ARGS+=(-drc "$out")
}

wanted() {
  [[ ${#WANTED[@]} -eq 0 ]] && return 0
  local stem="$1" w
  for w in "${WANTED[@]}"; do [[ "$w" == "$stem" ]] && return 0; done
  return 1
}

run_drc() {
  local log="$1" mode="$2"
  shift 2
  if [[ "$LANE" == port ]]; then
    # The same argv, and only the program in front of it changes. `-drc` is the one mode where
    # that substitution is the whole difference: no JVM flags to translate, no driver to compile,
    # no `-Xmx`. `< /dev/null` for `run_jar`'s reason — `each_row` drives the fixture table
    # through a `while read` whose stdin is the file.
    "$PORT_BIN" "$@" > "$log" 2>&1 < /dev/null
  else
    "$JAVA_BIN" "${LOCALE_FLAGS[@]}" -XX:+UnlockExperimentalVMOptions "-XX:hashCode=$mode" \
        -jar "$JAR" "$@" > "$log" 2>&1 < /dev/null
  fi
}

# Machine-independent rendering of a path: the two prefixes that differ between checkouts become
# tokens, so drc.meta.txt is byte-identical wherever it is regenerated.
portable() {
  local p="$1"
  p="${p//$JAVA_DIR/<FREEROUTING_JAVA_DIR>}"
  p="${p//$ROOT/<workspace>}"
  printf '%s' "$p"
}

# The three provenance lines a **port-cut** golden carries (Plan 9 Task 0). A golden cut before a
# later fix must be *loudly* invalid, and this is what makes it so: a reader who finds `task
# UNKNOWN`, or a task number older than the fix they are chasing, knows the reference is stale
# without re-deriving it.
port_meta_lines() {
  echo "lane         port"
  echo "port sha     ${PORT_SHA}${PORT_DIRTY}"
  if [[ -n "$TASK" ]]; then
    echo "plan 9 task  $TASK"
  else
    echo "plan 9 task  UNKNOWN — this golden names no task and is therefore INVALID as a"
    echo "             reference; re-cut it with --task T<n>. See this script's header."
  fi
  # The DRC path never constructs a `RouterBudget` — `-drc` reads a board and checks it — so the
  # line says that rather than naming a budget that was not in force. It is written anyway, in the
  # shape every other generator writes it, because what a reader checks is that the line is there.
  echo "budget       n/a — the -drc path checks a board and never enters the router, so no"
  echo "             RouterBudget is constructed on this path."
}

write_meta() {
  local out="$1"
  shift
  {
    if [[ "$LANE" == port ]]; then
      port_meta_lines
      echo "version      $(grep -o 'Freerouting [0-9][^"]*' "$out/drc.json" | head -1)"
      printf 'command      target/release/freerouting'
    else
      echo "jar          $(portable "$JAR")"
      echo "jar size     $(wc -c < "$JAR" | tr -d ' ') bytes"
      echo "jar mtime    $(date -r "$JAR" '+%Y-%m-%d %H:%M:%S %z')"
      echo "jar version  $(grep -o 'Freerouting [0-9][^"]*' "$out/drc.json" | head -1)"
      echo "java         $("$JAVA_BIN" -version 2>&1 | head -1)"
      echo "hash mode    -XX:hashCode=$HASH_MODE"
      printf 'command      java %s -XX:+UnlockExperimentalVMOptions -XX:hashCode=%s -jar <jar>' \
          "${LOCALE_FLAGS[*]}" "$HASH_MODE"
    fi
    local arg
    for arg in "$@"; do printf ' %s' "$(portable "$arg")"; done
    printf '\n'
  } > "$out/drc.meta.txt"
}

# Runs `body` for every selected row of the fixture table.
each_row() {
  local body="$1" stem dsn rules ses
  while IFS='|' read -r stem dsn rules ses || [[ -n "$stem" ]]; do
    [[ -z "$stem" || "$stem" == \#* ]] && continue
    wanted "$stem" || continue
    "$body" "$stem" "$dsn" "$rules" "$ses"
  done < "$FIXTURES"
}

# The jar's document spells its keys `unconnectedItems` / `qualityScore`; the port's spells them
# `unconnected_items` / `quality_score`, because the port writes 2.3.0's snake_case throughout
# (quirk #92 — HEAD's camelCase writer produces files HEAD's own lexer cannot read back, and
# survey §9.1 keeps the port's spelling as a decision). Both are read here so one summary line
# serves both lanes; `parity::normalize_drc_json` is what reconciles them where it matters.
report_counts() {
  python3 - "$1" <<'EOF'
import collections, json, sys
report = json.load(open(sys.argv[1], encoding="utf-8"))
def pick(*names):
    for n in names:
        if n in report:
            return report[n]
    return []
violations = pick("violations")
kinds = collections.Counter(v["type"] for v in violations)
print("   violations %d %s, unconnected %d, qualityScore %r"
      % (len(violations), dict(sorted(kinds.items())),
         len(pick("unconnectedItems", "unconnected_items")),
         pick("qualityScore", "quality_score") or None))
EOF
}

# --- the three modes -----------------------------------------------------------------------------
STATUS=0

generate_one() {
  local stem="$1" out="$OUT_ROOT/$1" tmp
  mkdir -p "$out"
  tmp="$out/drc.json.tmp"
  echo "== $stem"
  # `-drc <file>` opens and truncates its target before the check runs, so pointing it straight at
  # `drc.json` would destroy the committed reference the moment the jar is started and leave a
  # truncated or half-written document beside a `drc.meta.txt` that still describes the previous
  # run — a stale pair every parity test would then believe. Write to a temporary and `mv` only
  # after the jar exits 0 *and* left something behind; on any failure the existing reference and
  # its meta are untouched, together.
  rm -f "$tmp"
  drc_args "$2" "$3" "$4" "$tmp"
  local runlog="java.log"
  [[ "$LANE" == port ]] && runlog="port.log"
  if ! run_drc "$out/$runlog" "$HASH_MODE" "${ARGS[@]}" || [[ ! -s "$tmp" ]]; then
    echo "   the $LANE lane failed for $stem; see $out/$runlog (drc.json left untouched)" >&2
    rm -f "$tmp"
    STATUS=1
    return 0
  fi
  mv "$tmp" "$out/drc.json"
  # Re-derive `ARGS` against the final path: `drc.meta.txt`'s `command` line must be the command a
  # reader can re-run, not the one with the temporary in it.
  drc_args "$2" "$3" "$4" "$out/drc.json"
  write_meta "$out" "${ARGS[@]}"
  report_counts "$out/drc.json"
}

meta_one() {
  local stem="$1" out="$OUT_ROOT/$1"
  echo "== $stem"
  if [[ ! -f "$out/drc.json" ]]; then
    echo "   no drc.json to describe; run without --meta-only first" >&2
    STATUS=1
    return 0
  fi
  drc_args "$2" "$3" "$4" "$out/drc.json"
  write_meta "$out" "${ARGS[@]}"
  echo "   drc.meta.txt rewritten (drc.json untouched)"
}

sweep_one() {
  local stem="$1" mode raw digest
  echo "== $stem"
  local digests=() counts=()
  for mode in 0 1 2 3 4; do
    raw="$SCRATCH/$stem-h$mode.json"
    drc_args "$2" "$3" "$4" "$raw"
    if run_drc "$SCRATCH/$stem-h$mode.log" "$mode" "${ARGS[@]}"; then
      python3 "$ROOT/scripts/normalize-drc.py" "$raw" > "$SCRATCH/$stem-h$mode.norm.json"
      digest="$(shasum -a 256 < "$SCRATCH/$stem-h$mode.norm.json" | cut -d' ' -f1)"
      digests+=("${digest:0:12}")
      counts+=("$(python3 -c \
          "import json,sys;print(len(json.load(open(sys.argv[1]))['violations']))" "$raw")")
    else
      # Never `continue`: the two arrays are indexed by mode below, so a gap would misalign them.
      digests+=("FAILED------")
      counts+=("?")
      STATUS=1
    fi
  done
  local distinct
  distinct="$(printf '%s\n' "${digests[@]}" | sort -u | wc -l | tr -d ' ')"
  if [[ "$distinct" == "1" && "${digests[0]}" != "FAILED------" ]]; then
    echo "   5 modes agree (${digests[0]})"
  else
    echo "   $distinct distinct normalised documents over 5 modes:" >&2
    for mode in 0 1 2 3 4; do
      echo "     hashCode=$mode ${digests[$mode]} violations=${counts[$mode]}" >&2
    done
    STATUS=1
  fi
}

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
    echo "done. DRC references in $OUT_ROOT"
    ;;
esac
exit "$STATUS"
