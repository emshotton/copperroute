"""Normalised per-cell metrics. Verdict-relevant numbers come from the referee only.

`score` follows Java's own formula (see `score()` below), where N is the board's
*connection* count (`board_statistics.connections.maximum_count` from a `result.json`,
i.e. Java's `RoutingResultManifest`), not the DSN net count recorded in the corpus
manifest -- a net can require several connections. `metrics.json` records which N was
used as `score_basis` (`"connections:self"`, `"connections:manifest"`, or `"nets"`,
in that preference order) and the N itself as `score_n`. `score` is only comparable to
a candidate's self-reported `normalized_score` when `score_basis` starts with
`"connections"` -- the `"nets"` fallback (no connection count available anywhere) uses
a different, smaller N and will not match.
"""
from __future__ import annotations

import json
from pathlib import Path

from bench.corpus import Board

# Copied verbatim from freerouting DefaultSettings (Java). Versioned: bump SCORE_VERSION if changed.
SCORE_VERSION = 1
UNROUTED_PENALTY = 5_000_000.0
VIOLATION_PENALTY = 1_000_000.0
BEND_PENALTY = 10.0
VIA_COST = 50.0
TRACE_COST_PER_MM = 1.0


def score(nets: int, unrouted: int, violations: int, bends: int, length_mm: float, vias: int) -> float:
    maximum = nets * UNROUTED_PENALTY
    if maximum <= 0:
        return 0.0
    penalties = unrouted * UNROUTED_PENALTY + violations * VIOLATION_PENALTY + bends * BEND_PENALTY
    costs = length_mm * TRACE_COST_PER_MM + vias * VIA_COST
    return max(0.0, (maximum - penalties - costs) / maximum) * 1000.0


def _load(cell: Path, name: str) -> dict:
    p = cell / name
    return json.loads(p.read_text()) if p.exists() else {}


def _self_report(result: dict) -> dict:
    bs = result.get("board_statistics") or {}
    g = lambda sec, key: (bs.get(sec) or {}).get(key)
    return {
        "unrouted": g("connections", "incomplete_count"),
        "nets": g("connections", "maximum_count"),
        "violations": g("clearance_violations", "total_count"),
        "vias": g("vias", "total_count"),
        "wirelength_mm": g("traces", "total_length_mm"),
        "bends": g("bends", "total_count"),
        "score": result.get("normalized_score"),
        "final_state": result.get("final_state"),
        "version": result.get("app_version"),
        "sha": result.get("git_sha"),
    }


def build(cell: Path, board: Board) -> dict:
    result, ref, t = _load(cell, "result.json"), _load(cell, "referee.json"), _load(cell, "time.json")
    self_ = _self_report(result)
    phases = result.get("phases") or {}
    passes = (phases.get("autorouter") or {}).get("passes_completed")
    failed = ref.get("status") != "ok"
    unjudged = failed and bool(ref.get("candidate_output", False))

    nets_for_score = self_["nets"] if self_["nets"] else (board.connections or board.nets)
    if self_["nets"]:
        score_basis = "connections:self"
    elif board.connections:
        score_basis = "connections:manifest"
    else:
        score_basis = "nets"
    nets = nets_for_score or 0

    if failed:
        unrouted, violations, vias, length = nets, 0, 0, 0.0
    else:
        unrouted, violations = int(ref["unrouted"]), int(ref["violations"])
        vias, length = int(ref["vias"]), float(ref["wirelength_mm"])
    # Verdict numbers come from the referee only: if it can't count bends, drop the bend
    # penalty rather than trust the candidate's self-report (self.bends stays visible for
    # informational display). Max effect: BEND_PENALTY (10) per bend against nets*5e6 — negligible.
    bends = ref.get("bends") if ref.get("bends") is not None else 0

    m = {
        "schema_version": 1, "score_version": SCORE_VERSION,
        "board": board.id, "referee": ref.get("referee", board.referee),
        "clean_pass": (not failed) and unrouted == 0 and violations == 0,
        "unrouted": unrouted, "violations": violations,
        "violations_by_type": ref.get("violations_by_type", {}),
        "vias": vias, "wirelength_mm": length, "bends": bends,
        "score": score(nets, unrouted, violations, int(bends), length, vias),
        "score_basis": score_basis, "score_n": nets,
        "passes": passes,
        "wall_s": t.get("wall_s"), "cpu_s": t.get("cpu_s"), "peak_rss_mb": t.get("peak_rss_mb"),
        "timed_out": t.get("timed_out", False), "exit_code": t.get("exit_code"),
        # False for runs made before per-cell HOME/XDG isolation existed (time.json has no
        # such key yet): those may have shared a real freerouting.json across candidates --
        # see bench.compare.collect's warning and README's "Settings isolation" note.
        "isolated_config": t.get("isolated_config", False),
        "failed": failed, "failure_reason": ref.get("reason", "") if failed else "",
        "unjudged": unjudged,
        "disagreement": (not failed) and (
            (self_["unrouted"] is not None and self_["unrouted"] != unrouted)
            or (self_["violations"] is not None and self_["violations"] != violations)
        ),
        "self": self_,
        "wirelength_ratio": None, "via_ratio": None,
    }
    gt_path = board.kicad_path("ground_truth")
    if gt_path and gt_path.exists() and not failed:
        gt = json.loads(gt_path.read_text())
        if gt.get("wirelength_mm"):
            m["wirelength_ratio"] = round(length / gt["wirelength_mm"], 4)
        if gt.get("vias"):
            m["via_ratio"] = round(vias / gt["vias"], 4)
    (cell / "metrics.json").write_text(json.dumps(m, indent=2) + "\n")
    return m
