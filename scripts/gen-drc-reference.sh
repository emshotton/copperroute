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
#   java.log      the CLI's stdout+stderr
#   drc.meta.txt  the jar path/size/mtime, `java -version`, the hash mode and the exact command
#
# The reference is deliberately the raw bytes rather than a normalised document: normalisation is
# `parity::normalize_drc_json`'s job and the parity test applies it to **both** sides, so the
# committed file stays an unedited jar artifact and the two sides never have to agree on how a
# `double` is re-rendered. `scripts/normalize-drc.py` is the Python twin used by
# `--verify-hash-modes` only.
#
# Usage:
#   scripts/gen-drc-reference.sh [stem ...]        regenerate all stems, or just the named ones
#   scripts/gen-drc-reference.sh --verify-hash-modes [stem ...]
#                                                  regenerate each stem once per
#                                                  -XX:hashCode=0..4 into a scratch dir and
#                                                  require the five normalised documents to be
#                                                  byte-identical; writes nothing under
#                                                  tests/reference/
#
# Environment: FREEROUTING_JAVA_DIR (default ../freerouting), FREEROUTING_JAR, JAVA, DRC_HASH_MODE.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
JAVA_DIR="${FREEROUTING_JAVA_DIR:-$ROOT/../freerouting}"
JAR="${FREEROUTING_JAR:-$JAVA_DIR/build/libs/freerouting-current-executable.jar}"
JAVA_BIN="${JAVA:-/opt/homebrew/opt/openjdk@25/bin/java}"
REF="$ROOT/tests/reference"
FIXTURES="$REF/drc-fixtures.txt"

# `-XX:hashCode=2` is the constant-hash mode: the only `Object.hashCode` source in the JVM that
# reproduces run to run *and* is not derived from an object address. Modes 0 and 5 are PRNG-
# seeded, 1 and 4 come from the address, 3 is a per-thread xorshift (deterministic only in a
# single-threaded run). The references are generated under it so that the one fixture whose
# report is genuinely hash-dependent — Natural Tone Preamp, see tests/reference/README.md — has a
# reproducible reference at all. See crates/fr-drc/tests/data/README.md for the measured table.
HASH_MODE="${DRC_HASH_MODE:-2}"
HASH_FLAGS=(-XX:+UnlockExperimentalVMOptions "-XX:hashCode=$HASH_MODE")

# `-Duser.language=en -Duser.country=US` is load-bearing, not hygiene: every `%.4f` in a violation
# description goes through `String.formatted`, which uses the default FORMAT locale, so a German
# JVM writes `expected: 0,0500 mm` (plan-5 ruling 6). The port's formatter is locale-free.
LOCALE_FLAGS=(-Djava.awt.headless=true -Duser.language=en -Duser.country=US)

VERIFY_HASH_MODES=0
if [[ "${1:-}" == "--verify-hash-modes" ]]; then
  VERIFY_HASH_MODES=1
  shift
fi
WANTED=("$@")

# --- preflight ---------------------------------------------------------------------------------
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

# Prints the argv the CLI is invoked with for one row, one argument per line.
# `-drc` is matched by `startsWith` *before* `-dr` in GlobalSettings' if-chain
# (GlobalSettings.java:660-674), so the two cannot be confused whatever the order on the command
# line; the SES has no flag of its own and rides in the `-de` slot list, joined to the DSN with
# `+` (GlobalSettings.java:573-579 splits on it when the joined string is not itself a file).
ARGS=()
drc_args() {
  local dsn="$1" rules="$2" ses="$3" out="$4"
  local de="$JAVA_DIR/$dsn"
  [[ -n "$ses" ]] && de="$de+$JAVA_DIR/$ses"
  ARGS=(-de "$de")
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
  local out="$1" log="$2" mode="$3"
  shift 3
  "$JAVA_BIN" "${LOCALE_FLAGS[@]}" -XX:+UnlockExperimentalVMOptions "-XX:hashCode=$mode" \
      -jar "$JAR" "$@" > "$log" 2>&1
}

# --- the hash-mode sweep -----------------------------------------------------------------------
if [[ "$VERIFY_HASH_MODES" -eq 1 ]]; then
  scratch="$(mktemp -d)"
  trap 'rm -rf "$scratch"' EXIT
  status=0
  while IFS='|' read -r stem dsn rules ses || [[ -n "$stem" ]]; do
    [[ -z "$stem" || "$stem" == \#* ]] && continue
    wanted "$stem" || continue
    echo "== $stem"
    digests=()
    for mode in 0 1 2 3 4; do
      raw="$scratch/$stem-h$mode.json"
      drc_args "$dsn" "$rules" "$ses" "$raw"
      run_drc "$raw" "$scratch/$stem-h$mode.log" "$mode" "${ARGS[@]}" \
        || { echo "   hashCode=$mode FAILED, see $scratch/$stem-h$mode.log" >&2; status=1; continue; }
      python3 "$ROOT/scripts/normalize-drc.py" "$raw" > "$scratch/$stem-h$mode.norm.json"
      digests+=("$(shasum -a 256 < "$scratch/$stem-h$mode.norm.json" | cut -d' ' -f1)")
    done
    distinct="$(printf '%s\n' "${digests[@]}" | sort -u | wc -l | tr -d ' ')"
    if [[ "$distinct" == "1" ]]; then
      echo "   5 modes agree (${digests[0]:0:12})"
    else
      echo "   $distinct DISTINCT normalised documents over 5 modes:" >&2
      for mode in 0 1 2 3 4; do
        echo "     hashCode=$mode ${digests[$mode]:0:12} violations=$(python3 -c \
          "import json,sys;print(len(json.load(open(sys.argv[1]))['violations']))" \
          "$scratch/$stem-h$mode.json")" >&2
      done
      status=1
    fi
  done < "$FIXTURES"
  if [[ "$status" -ne 0 ]]; then
    echo "hash-mode sweep found a disagreement — record it in tests/reference/README.md" >&2
  fi
  exit "$status"
fi

# --- generation ---------------------------------------------------------------------------------
while IFS='|' read -r stem dsn rules ses || [[ -n "$stem" ]]; do
  [[ -z "$stem" || "$stem" == \#* ]] && continue
  wanted "$stem" || continue
  out="$REF/$stem"
  mkdir -p "$out"
  echo "== $stem"
  drc_args "$dsn" "$rules" "$ses" "$out/drc.json"
  if ! run_drc "$out/drc.json" "$out/java.log" "$HASH_MODE" "${ARGS[@]}"; then
    echo "   the jar failed for $stem; see $out/java.log" >&2
    continue
  fi

  {
    echo "jar          $JAR"
    echo "jar size     $(wc -c < "$JAR" | tr -d ' ') bytes"
    echo "jar mtime    $(date -r "$JAR" '+%Y-%m-%d %H:%M:%S %z')"
    echo "jar version  $(grep -o 'Freerouting [0-9][^"]*' "$out/drc.json" | head -1)"
    echo "java         $("$JAVA_BIN" -version 2>&1 | head -1)"
    echo "hash mode    -XX:hashCode=$HASH_MODE"
    printf 'command      %s %s -jar <jar>' "$JAVA_BIN" "${LOCALE_FLAGS[*]} ${HASH_FLAGS[*]}"
    printf ' %s' "${ARGS[@]}"
    printf '\n'
  } > "$out/drc.meta.txt"

  python3 - "$out/drc.json" <<'EOF'
import collections, json, sys
report = json.load(open(sys.argv[1], encoding="utf-8"))
kinds = collections.Counter(v["type"] for v in report["violations"])
print("   violations %d %s, unconnectedItems %d, qualityScore %r"
      % (len(report["violations"]), dict(sorted(kinds.items())),
         len(report["unconnectedItems"]), report.get("qualityScore")))
EOF
done < "$FIXTURES"

echo "done. DRC references in $REF"
