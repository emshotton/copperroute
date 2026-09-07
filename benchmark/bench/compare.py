"""Compare routing quality before speed, using repeated runs to estimate noise."""
from __future__ import annotations

import json
import statistics
from datetime import datetime, timezone
from pathlib import Path

from bench import metrics, referee, runner
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


def collect(run_dirs: list[Path], names: list[str], metas: dict | None = None) -> tuple[dict, dict, list[str]]:
    """Return (cells[cand][board] -> list[metrics], cand_info[cand], warnings)."""
    cells: dict[str, dict[str, list[dict]]] = {n: {} for n in names}
    info: dict[str, dict] = {}
    warnings: list[str] = []
    unisolated = False
    identities = {}
    for rd in run_dirs:
        meta = metas[rd] if metas is not None else runner.load_meta(rd)
        rescoring = meta.get("referee_rescore", {}).get("status") == "incomplete"
        if rescoring:
            warnings.append(f"{meta['run_id']}: full referee rescore is incomplete; rerun bench referee without --only-missing")
        for c in meta["candidates"]:
            if c["name"] in names:
                identity = (c["sha"], c.get("kind"), c.get("extra_args", []))
                if c["name"] in identities and identities[c["name"]] != identity:
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
                if rescoring and m.get("referee", "").startswith("java-drc") and not m.get("referee_identity"):
                    m["referee_identity"] = {"unverified_during_rescore": True}
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


def _check_config(metas: dict[Path, dict], allow_mixed: bool) -> tuple[dict, list[str]]:
    configs = [(rd.name, meta["args"]) for rd, meta in metas.items()]
    keys = ["threads", "jobs", "max_passes", "timeout_s"]
    first_name, first = configs[0]
    warnings = []
    for name, args in configs[1:]:
        if any(_arg(args, k) != _arg(first, k) for k in keys):
            msg = f"incompatible runs: {first_name} has config {first!r} but {name} has config {args!r}"
            if not allow_mixed:
                raise IncompatibleRuns(msg)
            warnings.append(msg)
    hosts = [meta.get("host") for meta in metas.values() if meta.get("host")]
    if hosts and any(host != hosts[0] for host in hosts[1:]):
        warnings.append(f"host metadata differs: {hosts!r}; verify the measurements used the same hardware")
    seeds = sorted({args.get("seeds", 1) for _, args in configs})
    if len(seeds) > 1:
        warnings.append(f"different repetition counts across runs: {seeds}; noise uses actual baseline samples per board")
    config = {k: _arg(first, k) for k in keys}
    config.update(seeds=seeds[0] if len(seeds) == 1 else None, repetition_counts=seeds)
    return config, warnings


def _resolve_time_metric(metas: dict[Path, dict], time_metric: str) -> str:
    if time_metric in ("wall", "wall_s"):
        return "wall_s"
    if time_metric in ("cpu", "cpu_s"):
        return "cpu_s"
    if time_metric != "auto":
        raise ValueError(f"unknown time_metric: {time_metric!r}")
    return "wall_s" if all(m["args"].get("jobs", 1) == 1 for m in metas.values()) else "cpu_s"


def _judged(cells: list[dict]) -> list[dict]:
    """Exclude referee failures from routing verdicts; retain them in coverage and failures."""
    return [c for c in cells if not c.get("unjudged")]


def _expected(metas: dict[Path, dict], names: list[str], board_ids: set[str]) -> dict:
    expected = {name: {} for name in names}
    for meta in metas.values():
        for candidate in meta["candidates"]:
            name = candidate["name"]
            if name not in expected:
                continue
            ids = meta["args"].get("boards")
            if ids is None:
                ids = {e["board"] for e in meta["cells"] if e["candidate"] == name}
            for bid in set(ids) & board_ids:
                # A replacement run can finish an interrupted plan under a new run ID.
                expected[name][bid] = max(expected[name].get(bid, 0), meta["args"].get("seeds", 1))
    return expected


def _score_denominator(board: Board, cells: list[dict]) -> int | None:
    for n in (board.connections, board.nets):
        if n is not None and n > 0:
            return n
    return max((c["score_n"] for c in cells if c.get("score_n") is not None and c["score_n"] > 0), default=None)


def _normalize_scores(cells: list[dict], n: int | None) -> list[dict]:
    normalized = []
    for cell in cells:
        m = dict(cell)
        if n is not None and m.get("score_version") == metrics.SCORE_VERSION and m.get("score_n") is not None:
            if m.get("failed") and not m.get("unjudged"):
                m["unrouted"] = n
            m["score"] = metrics.score(n, m["unrouted"], m["violations"], m.get("bends") or 0,
                                       m["wirelength_mm"], m["vias"])
            m["score_n"] = n
        normalized.append(m)
    return normalized


def _scoring_issue(cells: list[dict]) -> str | None:
    scored = [c for c in cells if not c.get("failed")]
    for key in ("referee", "referee_identity"):
        values = set()
        for c in scored:
            value = c.get(key)
            if key == "referee_identity" and value:
                value = referee.identity_key(value)
            if value is not None:
                values.add(json.dumps(value, sort_keys=True))
        if len(values) > 1:
            return f"different {key}; rescore this board with the same referee"
    return None


def _performance_check(base: list[dict], other: list[dict], percent: float | None,
                       time_metric: str) -> dict:
    if percent is None:
        return {"losses": [], "unmeasured": []}
    losses, unmeasured = [], []
    for key in (time_metric, "peak_rss_mb"):
        b = [c[key] for c in base if not c.get("failed") and c.get(key) is not None]
        o = [c[key] for c in other if not c.get("failed") and c.get(key) is not None]
        if len(b) < 3 or len(o) < 3:
            unmeasured.append(key)
            continue
        median = statistics.median(b)
        band = 3 * (statistics.variance(b) + statistics.variance(o)) ** 0.5
        if statistics.median(o) - median > max(median * percent / 100, band):
            losses.append(key)
    return {"losses": losses, "unmeasured": unmeasured}


def compare(run_dirs: list[Path], baseline: str, against: list[str], boards: list[Board],
            tier: str | None = None, allow_mixed: bool = False, time_metric: str = "auto",
            performance_regression_percent: float | None = None) -> dict:
    run_dirs = list(dict.fromkeys(run_dirs))
    if not run_dirs:
        raise IncompatibleRuns("no runs selected")
    metas = {rd: runner.load_meta(rd) for rd in run_dirs}
    config, warnings = _check_config(metas, allow_mixed)
    time_metric = _resolve_time_metric(metas, time_metric)
    config["time_metric"] = time_metric
    config["score_basis"] = "positive manifest connections or nets, falling back to the largest recorded positive score_n"
    config["performance_regression_percent"] = performance_regression_percent
    cells, info, w2 = collect(run_dirs, [baseline, *against], metas)
    warnings += w2
    missing_names = set([baseline, *against]) - info.keys()
    if missing_names:
        raise IncompatibleRuns(f"candidates absent from selected runs: {', '.join(sorted(missing_names))}")
    by_id = {b.id: b for b in boards}
    expected = _expected(metas, [baseline, *against],
                         {b.id for b in boards if not tier or tier in b.tiers})
    coverage = {}
    for name in against:
        shared = sorted(expected[baseline].keys() & expected[name].keys())
        coverage[name] = {
            "shared_boards": shared,
            "baseline_only_boards": sorted(expected[baseline].keys() - expected[name].keys()),
            "candidate_only_boards": sorted(expected[name].keys() - expected[baseline].keys()),
            "incomplete_boards": [bid for bid in shared if any(
                len(_judged(cells[n].get(bid, []))) < expected[n][bid] for n in (baseline, name))],
            "skipped_boards": {},
        }
        cov = coverage[name]
        if cov["baseline_only_boards"] or cov["candidate_only_boards"]:
            warnings.append(f"{name}: comparing {len(shared)} shared boards; "
                            f"{len(cov['baseline_only_boards'])} baseline-only and "
                            f"{len(cov['candidate_only_boards'])} candidate-only boards excluded")
        if cov["incomplete_boards"]:
            warnings.append(f"{name}: missing or unjudged repetitions on {len(cov['incomplete_boards'])} shared board(s)")
        for bid in shared:
            if not _judged(cells[baseline].get(bid, [])) or not _judged(cells[name].get(bid, [])):
                cov["skipped_boards"][bid] = "no judged measurements for one or both candidates"
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

        score_n = _score_denominator(board, all_cells_for_board)
        base_cells = _normalize_scores(_judged(base_cells_all), score_n)
        if not base_cells:
            continue  # every baseline seed for this board was unjudged: no baseline aggregate to compare against
        base_agg, nf = aggregate(base_cells), noise(base_cells)
        if len(base_cells) < 3:
            warnings.append(f"{bid}: baseline has {len(base_cells)} judged repetition(s); noise is unmeasured")
        entry = {"tiers": board.tiers, "referee": board.referee, "noise": nf, "score_n": score_n, "baseline": base_agg, "against": {}}
        for name in against:
            oc = _normalize_scores(_judged(cells[name].get(bid, [])), score_n)
            if bid not in expected[baseline] or bid not in expected[name] or not oc:
                entry["against"][name] = None
                continue
            issue = _scoring_issue(base_cells + oc)
            if issue:
                coverage[name]["skipped_boards"][bid] = issue
                entry["against"][name] = None
                continue
            agg = aggregate(oc)
            comparable_base, comparable_other = dict(base_agg), dict(agg)
            versions = {c.get("score_version") for c in base_cells + oc if not c.get("failed")}
            score_comparable = len(versions) <= 1 and score_n is not None
            if not score_comparable:
                comparable_base["score"] = comparable_other["score"] = None
                reason = "different score versions" if len(versions) > 1 else "no positive score denominator"
                warnings.append(f"{bid}/{name}: {reason}; score comparison omitted")
            delta = {k: (agg[k] - comparable_base[k]) if agg.get(k) is not None and comparable_base.get(k) is not None else None
                     for k in NUMERIC + ["clean_pass_rate"]}
            entry["against"][name] = {"agg": agg, "delta": delta,
                                      "score_comparable": score_comparable,
                                      "verdict": verdict(comparable_base, comparable_other, nf, time_metric),
                                      "performance": _performance_check(base_cells, oc, performance_regression_percent, time_metric)}
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
        coverage[name]["compared_boards"] = len(rows)
        for bid, reason in coverage[name]["skipped_boards"].items():
            warnings.append(f"{bid}/{name}: excluded from comparison: {reason}")
        quality_losses = sum(1 for r in rows if r["verdict"]["result"] == "loss"
                             and r["verdict"]["level"] in HARD_LEVELS | {"score"})
        overall[name] = {"wins": wins, "losses": losses, "ties": len(rows) - wins - losses, "hard_losses": hard,
                         "quality_losses": quality_losses,
                         "performance_losses": sum(bool(r["performance"]["losses"]) for r in rows),
                         "performance_unmeasured": sum(bool(r["performance"]["unmeasured"]) for r in rows),
                         "verdict": "inconclusive" if not rows else "better" if hard == 0 and wins > losses else ("worse" if hard > 0 or losses > wins else "same")}
    return {"schema_version": 1, "created_at": datetime.now(timezone.utc).isoformat(timespec="seconds"),
            "baseline": baseline, "against": against, "candidates": info, "runs": [rd.name for rd in run_dirs],
            "config": config, "tier": tier, "boards": out_boards, "tiers": tiers, "overall": overall,
            "coverage": coverage, "disagreements": disagreements, "failures": failures, "warnings": warnings}


def gate_failures(cmp: dict, require_complete: bool = False) -> list[str]:
    """Why `--fail-on-regression` rejects this comparison; empty means it passes.

    One reason per line, named rather than counted, so a report can state the outcome
    without re-deriving it from the numbers.
    """
    reasons: list[str] = []
    for name, overall in cmp.get("overall", {}).items():
        if overall.get("verdict") == "inconclusive":
            reasons.append(f"{name}: no comparable boards")
        # `hard_losses` is the fallback for reports written before `quality_losses` existed:
        # every hard level is a quality level, so it under-reports rather than inventing one.
        quality = overall.get("quality_losses", overall.get("hard_losses", 0))
        if quality:
            reasons.append(f"{name}: {quality} routing-quality losses")
        if overall.get("performance_losses"):
            reasons.append(f"{name}: {overall['performance_losses']} performance losses")
        if overall.get("performance_unmeasured"):
            reasons.append(f"{name}: {overall['performance_unmeasured']} boards with too few "
                           f"performance samples to judge")
    if require_complete:
        for name, coverage in (cmp.get("coverage") or {}).items():
            incomplete, skipped = coverage["incomplete_boards"], coverage["skipped_boards"]
            if incomplete or skipped:
                reasons.append(f"{name}: {len(incomplete)} incomplete and {len(skipped)} "
                               f"skipped boards")
    return reasons
