"""Referee for DSN fixtures: the Java jar in DRC-only mode + SES parsing for vias/length.

Invocation (Freerouting.java DRC path; GlobalSettings treats the .ses after -de as the session):
    java -jar fr.jar -de in.dsn out.ses -drc referee-drc.json --gui.enabled=false
"""
from __future__ import annotations

import json
import math
import os
import re
import subprocess
import tempfile
from pathlib import Path

from bench.corpus import Board
from bench.paths import isolated_env

REFEREE_TIMEOUT_S = 600

_RESOLUTION = re.compile(r"\(resolution\s+(\w+)\s+(\d+)\)")
_VIA = re.compile(r"\(via\s+")
_PATH = re.compile(r"\(path\s+\S+\s+[\d.]+((?:\s+-?[\d.]+)+)\s*\)")
_UNIT_TO_MM = {"um": 1e-3, "mm": 1.0, "mil": 0.0254, "inch": 25.4, "cm": 10.0}


def parse_ses(path: Path) -> tuple[int, float]:
    """(via count, wirelength mm) from a Specctra session file."""
    text = path.read_text(errors="replace")
    routes_idx = text.find("(routes")
    # The session's own resolution (used for routed coordinates) is the one inside
    # `(routes ...)`; fall back to the first `(resolution ...)` if there's no routes
    # section at all (shouldn't happen for a real .ses, but keep this robust).
    m = _RESOLUTION.search(text[routes_idx:] if routes_idx >= 0 else text)
    unit, res = (m.group(1), int(m.group(2))) if m else ("um", 10)
    scale = _UNIT_TO_MM.get(unit, 1e-3) / res
    vias = len(_VIA.findall(text))
    length = 0.0
    for m in _PATH.finditer(text):
        nums = [float(x) for x in m.group(1).split()]
        pts = list(zip(nums[0::2], nums[1::2]))
        for (x0, y0), (x1, y1) in zip(pts, pts[1:]):
            length += math.hypot(x1 - x0, y1 - y0)
    return vias, round(length * scale, 4)


def parse_drc_report(report: dict) -> dict:
    violations = report.get("violations") or []
    errors = [v for v in violations if v.get("severity", "error") == "error"]
    by_type: dict[str, int] = {}
    for v in errors:
        by_type[v.get("type", "unknown")] = by_type.get(v.get("type", "unknown"), 0) + 1
    warnings = sum(1 for v in violations if v.get("severity") == "warning")
    unconnected = report.get("unconnected_items") or report.get("unconnectedItems") or []
    return {"unrouted": len(unconnected), "violations": len(errors), "violations_by_type": by_type,
            "warnings": warnings}


def _failed(cell: Path, reason: str) -> dict:
    r = {"status": "referee_failed", "referee": "java-drc", "reason": reason,
         "unrouted": None, "violations": None, "violations_by_type": {},
         "vias": None, "wirelength_mm": None, "bends": None,
         "candidate_output": (cell / "out.ses").exists()}
    (cell / "referee.json").write_text(json.dumps(r, indent=2) + "\n")
    return r


def run(board: Board, cell: Path, java_exec: list[str]) -> dict:
    ses, dsn = cell / "out.ses", cell / "in.dsn"
    if not ses.exists():
        return _failed(cell, "out.ses missing (candidate produced no output)")
    report = cell / "referee-drc.json"
    # Isolate the referee jar's HOME/XDG dirs too (under a name distinct from the
    # candidate's own `cell/home`, written by bench.runner.run_cell) -- the referee
    # launches the same jar and must not read or rewrite the real freerouting.json either.
    home = cell / "referee-home"
    home.mkdir(parents=True, exist_ok=True)
    env_overrides = isolated_env(home)
    env = {**os.environ, **env_overrides}
    argv = [*java_exec, "-de", str(dsn), str(ses), "-drc", str(report), "--gui.enabled=false"]
    (cell / "referee-argv.json").write_text(json.dumps(argv, indent=2))
    (cell / "referee-env.json").write_text(json.dumps(env_overrides, indent=2))
    with open(cell / "referee.log", "wb") as log:
        try:
            subprocess.run(argv, cwd=cell, stdout=log, stderr=subprocess.STDOUT, timeout=REFEREE_TIMEOUT_S, env=env)
        except subprocess.TimeoutExpired:
            return _failed(cell, f"java DRC timed out after {REFEREE_TIMEOUT_S}s")
        except OSError as e:
            return _failed(cell, f"could not start java DRC: {e}")
    if not report.exists():
        return _failed(cell, "java DRC wrote no report (see referee.log)")
    try:
        raw = json.loads(report.read_text())
        drc = parse_drc_report(raw)
        vias, length = parse_ses(ses)
    except (ValueError, KeyError) as e:
        return _failed(cell, f"could not parse referee output: {e}")
    r = {"status": "ok", "referee": "java-drc", "reason": "", **drc,
         "vias": vias, "wirelength_mm": length, "bends": None}
    quality_score = raw.get("qualityScore")
    if quality_score is not None:
        r["quality_score"] = quality_score
    (cell / "referee.json").write_text(json.dumps(r, indent=2) + "\n")
    return r


def measure_connections(board: Board, java_exec: list[str], timeout_s: int = REFEREE_TIMEOUT_S) -> int | None:
    """Java's `board_statistics.connections.maximum_count` for `board`, used as the
    manifest fallback for `metrics.score`'s N (see bench.corpus.Board.connections).

    The jar's `-drc`-only mode does not write `--router.result_json` (verified against
    the jar: it writes only the DRC report), so this runs a real (`-mp 0`) routing
    invocation into a scratch directory instead, purely to read back the self-reported
    board statistics; the scratch `.ses`/`.json` are discarded. Returns None on any
    failure (missing/uninvocable jar, timeout, unparsable output)."""
    with tempfile.TemporaryDirectory(prefix="bench-connections-") as tmp:
        tmp_path = Path(tmp)
        result_json = tmp_path / "result.json"
        # Same jar, same isolation concern as `run` above -- give it its own scratch HOME
        # under the temp dir so this real routing invocation can't read or rewrite the
        # real freerouting.json either.
        home = tmp_path / "home"
        home.mkdir(parents=True, exist_ok=True)
        env = {**os.environ, **isolated_env(home)}
        argv = [*java_exec, "-de", str(board.path), "-do", str(tmp_path / "out.ses"),
                "-mp", "0", f"--router.result_json={result_json}",
                "--gui.enabled=false", "--api_server.enabled=false", "--mcp_server.enabled=false"]
        try:
            subprocess.run(argv, cwd=tmp_path, capture_output=True, timeout=timeout_s, env=env)
        except (subprocess.TimeoutExpired, OSError):
            return None
        if not result_json.exists():
            return None
        try:
            data = json.loads(result_json.read_text())
        except ValueError:
            return None
        n = ((data.get("board_statistics") or {}).get("connections") or {}).get("maximum_count")
        return int(n) if n is not None else None
