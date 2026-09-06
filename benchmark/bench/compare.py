"""Compare routing quality before speed, using repeated runs to estimate noise."""
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
    identities = {}
    for rd in run_dirs:
        meta = runner.load_meta(rd)
        for c in meta["candidates"]:
            if c["name"] in names:
                prev = info.get(c["name"])
                identity = (c["sha"], c.get("kind"), c.get("extra_args", []))
                if prev and identities[c["name"]] != identity:
                    raise IncompatibleRuns(
                        f"candidate {c['name']} has different commits or settings across runs; "
                        "use distinct candidate names for baseline and change")
                identities[c["name"]] = identity
                info[c["name"]] = {"sha": c["sha"], "version": c.get("version", "")}
        selected = set(names) & {c["name"] for c in meta["candidates"]}
        for e in meta["cells"]:
            if e["candidate"] not in selected:
                continue
            mp = runner.cell_dir(rd, e["candidate"], e["board"], e["seed"]) / "metrics.json"
            if mp.exists():
                m = json.loads(mp.read_text())
                m["_run"], m["_seed"], m["_candidate"] = meta["run_id"], e["seed"], e["candidate"]
                if not m.get("isolated_config", False):
                    unisolated = True
                cells[e["candidate"]].setdefault(e["board"], []).append(m)
    if unisolated:
        warnings.append("results may be affected by shared freerouting.json — rerun")
    return cells, info, warnings


def _arg(args: dict, key: str):
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
            msg = f"incompatible runs: {first_name} has config {first!r} but {name} has config {a!r}"
            if not allow_mixed:
                raise IncompatibleRuns(msg)
            warnings.append(msg)
    hosts = [(rd.name, runner.load_meta(rd).get("host")) for rd in run_dirs]
    known_hosts = [(name, host) for name, host in hosts if host]
    if known_hosts and any(host != known_hosts[0][1] for _, host in known_hosts[1:]):
        msg = f"incompatible hosts: {known_hosts!r}"
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
    """Exclude referee failures from routing verdicts; retain them in coverage and failures."""
    return [c for c in cells if not c.get("unjudged")]


def _coverage(run_dirs: list[Path], names: list[str], cells: dict, board_ids: set[str]) -> dict:
    expected = {name: {} for name in names}
    for rd in run_dirs:
        meta = runner.load_meta(rd)
        for candidate in meta["candidates"]:
            name = candidate["name"]
            if name not in expected:
                continue
            ids = meta["args"].get("boards")
            if ids is None:
                ids = {e["board"] for e in meta["cells"] if e["candidate"] == name}
            for bid in set(ids) & board_ids:
                expected[name][bid] = expected[name].get(bid, 0) + meta["args"].get("seeds", 1)
    ids = set().union(*(set(b) for b in expected.values()))
    coverage = {}
    for name in names:
        missing = []
        for bid in sorted(ids):
            count = len(_judged(cells[name].get(bid, [])))
            if count == 0 or count < expected[name].get(bid, 0):
                missing.append(bid)
        coverage[name] = {"expected_boards": len(ids), "incomplete_boards": missing}
    return coverage


def _check_scoring(cells: list[dict], bid: str) -> None:
    for key in ("referee", "score_version", "score_n"):
        values = {c.get(key) for c in cells if not c.get("failed")}
        if len(values) > 1:
            raise IncompatibleRuns(f"incompatible {key} for board {bid}: {values!r}; rescore with the same referee and corpus")


def compare(run_dirs: list[Path], baseline: str, against: list[str], boards: list[Board],
            tier: str | None = None, allow_mixed: bool = False, time_metric: str = "auto") -> dict:
    run_dirs = list(dict.fromkeys(run_dirs))
    if not run_dirs:
        raise IncompatibleRuns("no runs selected")
    config, warnings = _check_config(run_dirs, allow_mixed)
    time_metric = _resolve_time_metric(run_dirs, time_metric)
    config["time_metric"] = time_metric
    cells, info, w2 = collect(run_dirs, [baseline, *against])
    warnings += w2
    missing_names = set([baseline, *against]) - info.keys()
    if missing_names:
        raise IncompatibleRuns(f"candidates absent from selected runs: {', '.join(sorted(missing_names))}")
    by_id = {b.id: b for b in boards}
    coverage = _coverage(run_dirs, [baseline, *against], cells,
                         {b.id for b in boards if not tier or tier in b.tiers})
    for name, cov in coverage.items():
        if cov["incomplete_boards"]:
            warnings.append(f"{name}: missing or unjudged repetitions on "
                            f"{len(cov['incomplete_boards'])} board(s)")
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

        _check_scoring(all_cells_for_board, bid)
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
        incomplete = (not rows or coverage[baseline]["incomplete_boards"]
                      or coverage[name]["incomplete_boards"])
        overall[name] = {"wins": wins, "losses": losses, "ties": len(rows) - wins - losses, "hard_losses": hard,
                         "verdict": "inconclusive" if incomplete else "better" if hard == 0 and wins > losses else ("worse" if hard > 0 or losses > wins else "same")}
    return {"schema_version": 1, "created_at": datetime.now(timezone.utc).isoformat(timespec="seconds"),
            "baseline": baseline, "against": against, "candidates": info, "runs": [rd.name for rd in run_dirs],
            "config": config, "tier": tier, "boards": out_boards, "tiers": tiers, "overall": overall,
            "coverage": coverage, "disagreements": disagreements, "failures": failures, "warnings": warnings}
