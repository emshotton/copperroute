"""Aggregate cells, compute noise floors, apply the lexicographic verdict (spec §9)."""
from __future__ import annotations

import json
import statistics
from datetime import datetime, timezone
from pathlib import Path

from bench import runner
from bench.corpus import Board

NUMERIC = ["unrouted", "violations", "vias", "wirelength_mm", "score", "wall_s", "cpu_s",
           "peak_rss_mb", "passes", "wirelength_ratio", "via_ratio"]
HARD_LEVELS = {"clean_pass_rate", "unrouted", "violations"}
NOISE_FACTOR = 2.0
DEFAULT_TIME_METRIC = "wall_s"


def _levels(time_metric: str = DEFAULT_TIME_METRIC) -> list[tuple[str, bool]]:
    """(metric, higher_is_better) in verdict order. Level 5 (time) is whichever metric
    the comparison chose -- wall_s when every run in it used jobs=1, else cpu_s, since
    wall time is contended and no longer a fair signal once cells run concurrently."""
    return [("clean_pass_rate", True), ("unrouted", False), ("violations", False),
            ("score", True), (time_metric, False), ("peak_rss_mb", False)]


class IncompatibleRuns(ValueError):
    pass


def _median(values: list) -> float | None:
    vals = [v for v in values if v is not None]
    return statistics.median(vals) if vals else None


def aggregate(cells: list[dict]) -> dict:
    agg = {"n": len(cells), "clean_pass_rate": sum(1 for c in cells if c.get("clean_pass")) / len(cells) if cells else 0.0,
           "failed": sum(1 for c in cells if c.get("failed"))}
    for k in NUMERIC:
        agg[k] = _median([c.get(k) for c in cells])
    return agg


def noise(cells: list[dict]) -> dict[str, float]:
    out: dict[str, float] = {"clean_pass_rate": 0.0}
    for k in NUMERIC:
        vals = [c.get(k) for c in cells if c.get(k) is not None]
        out[k] = statistics.stdev(vals) if len(vals) >= 3 else 0.0
    return out


def verdict(base: dict, other: dict, noise_floor: dict[str, float],
           time_metric: str = DEFAULT_TIME_METRIC) -> dict:
    levels = _levels(time_metric)
    last = levels[-1][0]
    for metric, higher_better in levels:
        b, o = base.get(metric), other.get(metric)
        if b is None or o is None:
            continue
        band = NOISE_FACTOR * noise_floor.get(metric, 0.0)
        if abs(o - b) <= band:
            continue
        better = o > b if higher_better else o < b
        return {"result": "win" if better else "loss", "level": metric}
    return {"result": "tie", "level": last}


def collect(run_dirs: list[Path], names: list[str]) -> tuple[dict, dict, list[str]]:
    """Return (cells[cand][board] -> list[metrics], cand_info[cand], warnings)."""
    cells: dict[str, dict[str, list[dict]]] = {n: {} for n in names}
    info: dict[str, dict] = {}
    warnings: list[str] = []
    unisolated = False
    for rd in run_dirs:
        meta = runner.load_meta(rd)
        for c in meta["candidates"]:
            if c["name"] in names:
                prev = info.get(c["name"])
                if prev and prev["sha"] != c["sha"]:
                    warnings.append(f"candidate {c['name']} has sha {prev['sha']} in one run and {c['sha']} in {meta['run_id']}")
                info[c["name"]] = {"sha": c["sha"], "version": c.get("version", "")}
        for e in meta["cells"]:
            if e["candidate"] not in names:
                continue
            mp = runner.cell_dir(rd, e["candidate"], e["board"], e["seed"]) / "metrics.json"
            if mp.exists():
                m = json.loads(mp.read_text())
                m["_run"], m["_seed"], m["_candidate"] = meta["run_id"], e["seed"], e["candidate"]
                if not m.get("isolated_config", False):
                    unisolated = True
                cells[e["candidate"]].setdefault(e["board"], []).append(m)
    if unisolated:
        # Not a refusal (spec: a warning, not IncompatibleRuns) -- a run made before
        # per-cell HOME/XDG isolation existed may have shared a real freerouting.json
        # across candidates/runs on the same host, silently skewing costs/ripup
        # costs/neckdown flags; flag it so the reader knows to rerun rather than trust it.
        warnings.append("results may be affected by shared freerouting.json — rerun")
    return cells, info, warnings


def _arg(args: dict, key: str):
    # Pre-parallel-jobs runs have no args["jobs"] at all; treat that the same as jobs=1
    # (what those runs actually did) rather than None, so an old run and a `--jobs 1` run
    # compare as compatible instead of tripping IncompatibleRuns on a key that didn't exist
    # yet when the old run was made.
    if key == "jobs":
        return args.get("jobs", 1)
    return args.get(key)


def _check_config(run_dirs: list[Path], allow_mixed: bool) -> tuple[dict, list[str]]:
    configs = [(rd.name, runner.load_meta(rd)["args"]) for rd in run_dirs]
    keys = ["threads", "jobs", "max_passes", "timeout_s"]
    first_name, first = configs[0]
    warnings = []
    for name, a in configs[1:]:
        if any(_arg(a, k) != _arg(first, k) for k in keys):
            # Full config dicts, not just the differing key(s), so the message is
            # self-contained (spec §12: "always print both configurations").
            msg = f"incompatible runs: {first_name} has config {first!r} but {name} has config {a!r}"
            if not allow_mixed:
                raise IncompatibleRuns(msg)
            warnings.append(msg)
    return {k: _arg(first, k) for k in keys + ["seeds"]}, warnings


def _resolve_time_metric(run_dirs: list[Path], time_metric: str) -> str:
    """Resolve the requested time metric ("auto" | "wall" | "wall_s" | "cpu" | "cpu_s") to
    an actual metric name. "auto" picks wall_s only if every run being compared used
    jobs=1 (otherwise wall time is contended and no longer comparable), else cpu_s."""
    if time_metric in ("wall", "wall_s"):
        return "wall_s"
    if time_metric in ("cpu", "cpu_s"):
        return "cpu_s"
    if time_metric != "auto":
        raise ValueError(f"unknown time_metric: {time_metric!r}")
    all_single_job = all(runner.load_meta(rd)["args"].get("jobs", 1) == 1 for rd in run_dirs)
    return "wall_s" if all_single_job else "cpu_s"


def _judged(cells: list[dict]) -> list[dict]:
    """Drop cells the referee couldn't score (candidate produced out.ses but the referee
    itself failed -- spec §7.1/§8 case b). Those are excluded from verdict aggregation but
    still visible in the failures list."""
    return [c for c in cells if not c.get("unjudged")]


def compare(run_dirs: list[Path], baseline: str, against: list[str], boards: list[Board],
            tier: str | None = None, allow_mixed: bool = False, time_metric: str = "auto") -> dict:
    config, warnings = _check_config(run_dirs, allow_mixed)
    time_metric = _resolve_time_metric(run_dirs, time_metric)
    config["time_metric"] = time_metric
    cells, info, w2 = collect(run_dirs, [baseline, *against])
    warnings += w2
    by_id = {b.id: b for b in boards}
    out_boards: dict[str, dict] = {}
    disagreements, failures = [], []
    for bid in sorted(cells.get(baseline, {})):
        board = by_id.get(bid)
        if board is None or (tier and tier not in board.tiers):
            continue
        base_cells_all = cells[baseline][bid]
        all_cells_for_board = base_cells_all + [m for n in against for m in cells[n].get(bid, [])]
        for m in all_cells_for_board:
            if m.get("disagreement"):
                disagreements.append({"board": bid, "candidate": m["_candidate"], "seed": m["_seed"],
                                      "self_unrouted": m["self"].get("unrouted") if m.get("self") else None,
                                      "referee_unrouted": m["unrouted"], "self_violations": m["self"].get("violations") if m.get("self") else None,
                                      "referee_violations": m["violations"]})
            if m.get("failed"):
                failures.append({"board": bid, "candidate": m["_candidate"], "seed": m["_seed"],
                                 "reason": m.get("failure_reason", ""), "unjudged": bool(m.get("unjudged"))})

        base_cells = _judged(base_cells_all)
        if not base_cells:
            continue  # every baseline seed for this board was unjudged: no baseline aggregate to compare against
        base_agg, nf = aggregate(base_cells), noise(base_cells)
        entry = {"tiers": board.tiers, "referee": board.referee, "noise": nf, "baseline": base_agg, "against": {}}
        for name in against:
            oc = _judged(cells[name].get(bid, []))
            if not oc:
                entry["against"][name] = None
                continue
            agg = aggregate(oc)
            delta = {k: (agg[k] - base_agg[k]) if agg.get(k) is not None and base_agg.get(k) is not None else None
                     for k in NUMERIC + ["clean_pass_rate"]}
            entry["against"][name] = {"agg": agg, "delta": delta,
                                      "verdict": verdict(base_agg, agg, nf, time_metric)}
        out_boards[bid] = entry

    tiers: dict[str, dict] = {}
    all_tiers = sorted({t for e in out_boards.values() for t in e["tiers"]} | {"all"})
    for t in all_tiers:
        ids = [b for b, e in out_boards.items() if t == "all" or t in e["tiers"]]
        tiers[t] = {}
        for name in against:
            pairs = [(b, out_boards[b]["against"].get(name)) for b in ids]
            pairs = [(b, r) for b, r in pairs if r]
            rows = [r for _, r in pairs]
            if not rows:
                continue
            res = [r["verdict"]["result"] for r in rows]
            ratios = [r["agg"][time_metric] / out_boards[b]["baseline"][time_metric] for b, r in pairs
                      if r["agg"].get(time_metric) and out_boards[b]["baseline"].get(time_metric)]
            tiers[t][name] = {"wins": res.count("win"), "losses": res.count("loss"), "ties": res.count("tie"),
                              "clean_pass_rate": sum(1 for r in rows if r["agg"]["clean_pass_rate"] == 1.0) / len(rows),
                              "median_score_delta": _median([r["delta"]["score"] for r in rows]),
                              "median_time_ratio": _median(ratios)}
    overall = {}
    for name in against:
        rows = [e["against"][name] for e in out_boards.values() if e["against"].get(name)]
        hard = sum(1 for r in rows if r["verdict"]["result"] == "loss" and r["verdict"]["level"] in HARD_LEVELS)
        wins = sum(1 for r in rows if r["verdict"]["result"] == "win")
        losses = sum(1 for r in rows if r["verdict"]["result"] == "loss")
        overall[name] = {"wins": wins, "losses": losses, "ties": len(rows) - wins - losses, "hard_losses": hard,
                         "verdict": "better" if hard == 0 and wins > losses else ("worse" if hard > 0 or losses > wins else "same")}
    return {"schema_version": 1, "created_at": datetime.now(timezone.utc).isoformat(timespec="seconds"),
            "baseline": baseline, "against": against, "candidates": info, "runs": [rd.name for rd in run_dirs],
            "config": config, "tier": tier, "boards": out_boards, "tiers": tiers, "overall": overall,
            "disagreements": disagreements, "failures": failures, "warnings": warnings}
