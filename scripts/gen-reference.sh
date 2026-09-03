#!/usr/bin/env bash
# Generate the DSN/SES round-trip references (family G) for the `fr-dsn` parity tests.
#
# ## Two lanes (Plan 9 Task 0)
#
#   --jar         the 2.3.0 release jar, through `scripts/gen-reference/RefWriter.java`. This is
#                 the historical lane and it is still the **default**, so every invocation that
#                 predates Plan 9 means exactly what it meant before. It is what the frozen
#                 baseline (`tests/reference-frozen/java-head-2026-09/`, Task 1) was cut with and
#                 it is the triage lane: when a port-cut golden churns for a reason nobody can
#                 name, this is what it gets compared against.
#   --from-port   the **port**, through `scripts/differential/rust/src/bin/refwriter.rs` — the
#                 Rust twin of `RefWriter.java`, same three arguments, same two outputs. This is
#                 the lane that regenerates the committed references from Plan 9 Task 1 on, when
#                 the port's answer is allowed to be better than the jar's.
#
# **Byte parity with the jar may break from Plan 9 on, and that is the point.** The `--jar` lane
# is kept because a jar comparison is worth having as a *diagnosis*; it is no longer a gate.
#
# ## What it writes, per stem of tests/reference/fixtures.txt, into tests/reference/<stem>/
#
#   roundtrip.dsn   the board read and written straight back out, no routing
#   unrouted.ses    the same board as a session file, no routing
#   java.log        (`--jar`) the driver's stdout+stderr — absolute paths and wall-clock stamps,
#                   kept for debugging and read by nothing
#   port.log        (`--from-port`) the same, for the port lane
#   meta.txt        (`--from-port` only) the port's git sha, the Plan 9 task at that sha and the
#                   `RouterBudget` in force. **A golden cut before a later fix must be loudly
#                   invalid**, and that line is what makes it so: a reader who finds `task
#                   UNKNOWN` there, or a task number older than the fix they are chasing, knows
#                   the reference is stale without having to re-derive it.
#
# ## Usage
#
#   scripts/gen-reference.sh [--jar] [stem ...]
#   scripts/gen-reference.sh --from-port [--task T<n>] [stem ...]
#
# ## Environment
#
# FREEROUTING_JAVA_DIR (default ../freerouting), JAVA, JAVAC, PLAN9_TASK,
# REFERENCE_OUT_ROOT (default tests/reference — point it at a scratch tree to generate without
# touching the committed references; the fixture table is always read from tests/reference).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
JAVA_DIR="${FREEROUTING_JAVA_DIR:-$ROOT/../freerouting}"
JAR_VERSION="2.3.0"
JAR="$ROOT/tools/freerouting-$JAR_VERSION.jar"
JAR_URL="https://github.com/freerouting/freerouting/releases/download/v$JAR_VERSION/freerouting-$JAR_VERSION.jar"
REF="$ROOT/tests/reference"
# Outputs may be redirected to a scratch tree; inputs never are.
OUT_ROOT="${REFERENCE_OUT_ROOT:-$REF}"
JAVA_BIN="${JAVA:-java}"

LANE=jar
TASK="${PLAN9_TASK:-}"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --jar) LANE=jar; shift ;;
    --from-port) LANE=port; shift ;;
    --task) TASK="${2:?--task needs a task id, e.g. T1}"; shift 2 ;;
    --task=*) TASK="${1#--task=}"; shift ;;
    --*) echo "error: unknown option $1" >&2; exit 1 ;;
    *) break ;;
  esac
done
WANTED=("$@")

wanted() {
  [[ ${#WANTED[@]} -eq 0 ]] && return 0
  local stem="$1" w
  for w in "${WANTED[@]}"; do [[ "$w" == "$stem" ]] && return 0; done
  return 1
}

if [[ "$LANE" == jar ]]; then
  # --- Java version check (jar targets JDK 25) ---
  ver="$("$JAVA_BIN" -version 2>&1 | head -1 | sed -E 's/.*"([0-9]+).*/\1/')"
  if [[ "${ver:-0}" -lt 25 ]]; then
    echo "error: Java 25+ required (found ${ver:-none}). On macOS: brew install openjdk@25" >&2
    echo "       then: export JAVA=/opt/homebrew/opt/openjdk@25/bin/java" >&2
    exit 1
  fi

  mkdir -p "$ROOT/tools"
  if [[ ! -f "$JAR" ]]; then
    echo "downloading $JAR_URL"
    curl -fL --retry 3 -o "$JAR" "$JAR_URL"
  fi

  # --- Reference driver: read DSN with the real reader, write DSN + SES without routing ---
  # The -de/-do job path cannot write DSN output headlessly and writes nothing when all
  # routing stages are disabled, so we link a tiny driver against the jar instead.
  DRIVER_SRC="$ROOT/scripts/gen-reference/RefWriter.java"
  DRIVER_OUT="$ROOT/scripts/gen-reference/build"   # gitignored
  JAVAC_BIN="${JAVAC:-$(dirname "$JAVA_BIN")/javac}"
  mkdir -p "$DRIVER_OUT"
  "$JAVAC_BIN" -cp "$JAR" -d "$DRIVER_OUT" "$DRIVER_SRC"
else
  # The port's own `RefWriter` twin: same argv, same two files. Built in release for the same
  # reason `run.sh` builds the binary in release — a debug build of the DSN writer is minutes
  # rather than seconds on the wide boards.
  echo "== building the port's refwriter twin (release)"
  (cd "$ROOT/scripts/differential/rust" && cargo build --release --bin refwriter --quiet)
  PORT_DRIVER="$ROOT/scripts/differential/rust/target/release/refwriter"
  PORT_SHA="$(cd "$ROOT" && git rev-parse --short=12 HEAD 2>/dev/null || echo unknown)"
  PORT_DIRTY=""
  if ! (cd "$ROOT" && git diff --quiet HEAD -- crates 2>/dev/null); then
    PORT_DIRTY=" +uncommitted-changes-under-crates"
  fi
fi

# The three provenance lines a port-cut golden carries. `RefWriter` runs no router at all, so the
# budget line says so rather than naming a `RouterBudget` that never existed on this path — but
# the line is written anyway, in the same shape every other generator writes it, because what a
# reader checks is that the line is *there* and names a task.
write_port_meta() {
  local out="$1"
  {
    echo "lane         port (scripts/differential/rust/src/bin/refwriter.rs)"
    echo "port sha     ${PORT_SHA}${PORT_DIRTY}"
    if [[ -n "$TASK" ]]; then
      echo "plan 9 task  $TASK"
    else
      echo "plan 9 task  UNKNOWN — this golden names no task and is therefore INVALID as a"
      echo "             reference; re-cut it with --task T<n>. See this script's header."
    fi
    echo "budget       n/a — RefWriter reads and writes the board and never enters the router,"
    echo "             so no RouterBudget is constructed on this path."
    echo "driver       scripts/differential/rust/src/bin/refwriter.rs"
    echo "command      refwriter <in.dsn> <out>/roundtrip.dsn <out>/unrouted.ses"
  } > "$out/meta.txt"
}

status=0
while IFS='|' read -r stem src || [[ -n "$stem" ]]; do
  [[ -z "$stem" || "$stem" == \#* ]] && continue
  wanted "$stem" || continue
  in="$JAVA_DIR/$src"
  out="$OUT_ROOT/$stem"
  mkdir -p "$out"
  echo "== $stem  [$LANE]"
  if [[ "$LANE" == jar ]]; then
    if "$JAVA_BIN" -Djava.awt.headless=true -cp "$JAR:$DRIVER_OUT" RefWriter \
         "$in" "$out/roundtrip.dsn" "$out/unrouted.ses" > "$out/java.log" 2>&1; then
      echo "   roundtrip.dsn $(wc -c < "$out/roundtrip.dsn") bytes, unrouted.ses $(wc -c < "$out/unrouted.ses") bytes, wires in ses: $(grep -c '(wire' "$out/unrouted.ses" || true)"
    else
      echo "   driver failed for $stem; see $out/java.log" >&2
      status=1
    fi
  else
    if "$PORT_DRIVER" "$in" "$out/roundtrip.dsn" "$out/unrouted.ses" > "$out/port.log" 2>&1; then
      write_port_meta "$out"
      echo "   roundtrip.dsn $(wc -c < "$out/roundtrip.dsn") bytes, unrouted.ses $(wc -c < "$out/unrouted.ses") bytes, wires in ses: $(grep -c '(wire' "$out/unrouted.ses" || true)"
    else
      echo "   the port's refwriter failed for $stem; see $out/port.log" >&2
      status=1
    fi
  fi
done < "$REF/fixtures.txt"

echo "done. References in $OUT_ROOT"
exit "$status"
