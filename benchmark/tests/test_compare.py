import json
from pathlib import Path

import pytest

from bench import compare, runner
from bench.corpus import Board


def cell(clean=True, unrouted=0, violations=0, score=990.0, wall=10.0, rss=200.0, failed=False, disagree=False,
         unjudged=False):
    return {"clean_pass": clean, "unrouted": unrouted, "violations": violations, "vias": 4,
            "wirelength_mm": 100.0, "score": score, "wall_s": wall, "cpu_s": wall, "peak_rss_mb": rss,
            "passes": 3, "failed": failed, "disagreement": disagree, "unjudged": unjudged, "timed_out": False,
            "wirelength_ratio": None, "via_ratio": None, "referee": "java-drc"}


def test_aggregate_medians_and_rate():
    a = compare.aggregate([cell(score=900, wall=1), cell(score=950, wall=3), cell(clean=False, unrouted=1, score=800, wall=2)])
    assert a["n"] == 3 and a["score"] == 900 and a["wall_s"] == 2 and a["unrouted"] == 0
    assert a["clean_pass_rate"] == pytest.approx(2 / 3)


def test_noise_is_stddev_or_zero_below_three():
    assert compare.noise([cell(score=900), cell(score=950)])["score"] == 0.0
    n = compare.noise([cell(score=900), cell(score=950), cell(score=1000)])
    assert n["score"] == pytest.approx(50.0)  # sample stddev of 900,950,1000


def test_verdict_lexicographic_with_noise_ties():
    base = compare.aggregate([cell(score=900, wall=10)] * 3)
    noise = {"clean_pass_rate": 0, "unrouted": 0, "violations": 0, "score": 30, "wall_s": 1, "peak_rss_mb": 0}
    assert compare.verdict(base, compare.aggregate([cell(score=940, wall=10)] * 3), noise) == {"result": "tie", "level": "peak_rss_mb"}
    assert compare.verdict(base, compare.aggregate([cell(score=990, wall=10)] * 3), noise)["result"] == "win"
    assert compare.verdict(base, compare.aggregate([cell(score=900, wall=5)] * 3), noise) == {"result": "win", "level": "wall_s"}
    assert compare.verdict(base, compare.aggregate([cell(clean=False, unrouted=1, score=990)] * 3), noise) == {"result": "loss", "level": "clean_pass_rate"}


def _write_run(root: Path, run_id: str, cands: dict[str, dict[str, list[dict]]], threads=1, jobs=1):
    run_dir = root / run_id
    meta = {"run_id": run_id, "status": "complete",
            "args": {"threads": threads, "jobs": jobs, "max_passes": 100, "timeout_s": 300, "seeds": 3},
            "candidates": [{"name": c, "sha": f"sha-{c}", "version": "1"} for c in cands], "cells": []}
    for c, boards in cands.items():
        for b, cells in boards.items():
            for i, m in enumerate(cells, 1):
                d = runner.cell_dir(run_dir, c, b, i)
                d.mkdir(parents=True)
                (d / "metrics.json").write_text(json.dumps(m))
                meta["cells"].append({"candidate": c, "board": b, "seed": i, "status": "ok"})
    run_dir.mkdir(exist_ok=True)
    (run_dir / "meta.json").write_text(json.dumps(meta))
    return run_dir


def test_compare_end_to_end(tmp_path):
    boards = [Board(id="a", source="", origin="freerouting-fixtures", referee="java-drc", tiers=["canary"], nets=5, layers=2),
              Board(id="b", source="", origin="freerouting-fixtures", referee="java-drc", tiers=["canary", "hard"], nets=5, layers=2)]
    r1 = _write_run(tmp_path, "r1", {
        "java": {"a": [cell(score=900)] * 3, "b": [cell(score=900)] * 3},
        "rs":   {"a": [cell(score=990)] * 3, "b": [cell(clean=False, unrouted=2, score=600, disagree=True)] * 3}})
    cmp = compare.compare([r1], baseline="java", against=["rs"], boards=boards)
    assert cmp["boards"]["a"]["against"]["rs"]["verdict"]["result"] == "win"
    assert cmp["boards"]["b"]["against"]["rs"]["verdict"] == {"result": "loss", "level": "clean_pass_rate"}
    assert cmp["tiers"]["canary"]["rs"] == pytest.approx({"wins": 1, "losses": 1, "ties": 0, "clean_pass_rate": 0.5,
                                                          "median_score_delta": (90 - 300) / 2, "median_time_ratio": 1.0}, rel=1e-6)
    assert cmp["overall"]["rs"]["verdict"] == "worse"   # a hard-metric loss
    assert len(cmp["disagreements"]) == 3
    assert cmp["candidates"]["rs"]["sha"] == "sha-rs"


def test_compare_refuses_mixed_threads(tmp_path):
    boards = [Board(id="a", source="", origin="freerouting-fixtures", referee="java-drc", tiers=[], nets=5, layers=2)]
    r1 = _write_run(tmp_path, "r1", {"java": {"a": [cell()] * 3}}, threads=1)
    r2 = _write_run(tmp_path, "r2", {"rs": {"a": [cell()] * 3}}, threads=4)
    with pytest.raises(compare.IncompatibleRuns):
        compare.compare([r1, r2], baseline="java", against=["rs"], boards=boards)
    cmp = compare.compare([r1, r2], baseline="java", against=["rs"], boards=boards, allow_mixed=True)
    assert any("threads" in w for w in cmp["warnings"])


def test_compare_refuses_mixed_jobs(tmp_path):
    boards = [Board(id="a", source="", origin="freerouting-fixtures", referee="java-drc", tiers=[], nets=5, layers=2)]
    r1 = _write_run(tmp_path, "r1", {"java": {"a": [cell()] * 3}}, jobs=1)
    r2 = _write_run(tmp_path, "r2", {"rs": {"a": [cell()] * 3}}, jobs=4)
    with pytest.raises(compare.IncompatibleRuns):
        compare.compare([r1, r2], baseline="java", against=["rs"], boards=boards)
    cmp = compare.compare([r1, r2], baseline="java", against=["rs"], boards=boards, allow_mixed=True)
    assert any("jobs" in w for w in cmp["warnings"])


def test_compare_warns_when_any_cell_lacks_isolated_config(tmp_path):
    boards = [Board(id="a", source="", origin="freerouting-fixtures", referee="java-drc", tiers=["canary"],
                    nets=5, layers=2)]
    unisolated = cell()
    unisolated["isolated_config"] = False
    r1 = _write_run(tmp_path, "r1", {
        "java": {"a": [unisolated] * 3},
        "rs":   {"a": [cell(score=990)] * 3}})
    cmp = compare.compare([r1], baseline="java", against=["rs"], boards=boards)
    assert any("shared freerouting.json" in w for w in cmp["warnings"])


def test_compare_no_isolation_warning_when_all_cells_isolated(tmp_path):
    boards = [Board(id="a", source="", origin="freerouting-fixtures", referee="java-drc", tiers=["canary"],
                    nets=5, layers=2)]
    isolated = cell()
    isolated["isolated_config"] = True
    r1 = _write_run(tmp_path, "r1", {
        "java": {"a": [isolated] * 3},
        "rs":   {"a": [isolated] * 3}})
    cmp = compare.compare([r1], baseline="java", against=["rs"], boards=boards)
    assert not any("shared freerouting.json" in w for w in cmp["warnings"])


def test_compare_treats_missing_jobs_as_one(tmp_path):
    # A run from before `jobs` was added has no args["jobs"] at all; that must compare as
    # compatible with a run that explicitly used --jobs 1, not trip IncompatibleRuns.
    boards = [Board(id="a", source="", origin="freerouting-fixtures", referee="java-drc", tiers=[], nets=5, layers=2)]
    r1 = _write_run(tmp_path, "r1", {"java": {"a": [cell()] * 3}}, jobs=1)
    meta_path = r1 / "meta.json"
    meta = json.loads(meta_path.read_text())
    del meta["args"]["jobs"]
    meta_path.write_text(json.dumps(meta))
    r2 = _write_run(tmp_path, "r2", {"rs": {"a": [cell()] * 3}}, jobs=1)
    cmp = compare.compare([r1, r2], baseline="java", against=["rs"], boards=boards)  # no allow_mixed: must not raise
    assert cmp["config"]["jobs"] == 1
    assert not any("jobs" in w for w in cmp["warnings"])


def test_time_metric_auto_is_wall_when_all_jobs_one(tmp_path):
    boards = [Board(id="a", source="", origin="freerouting-fixtures", referee="java-drc", tiers=["canary"], nets=5, layers=2)]
    r1 = _write_run(tmp_path, "r1", {
        "java": {"a": [cell(score=900)] * 3},
        "rs":   {"a": [cell(score=990)] * 3}}, jobs=1)
    cmp = compare.compare([r1], baseline="java", against=["rs"], boards=boards)
    assert cmp["config"]["time_metric"] == "wall_s"


def test_time_metric_auto_is_cpu_when_jobs_greater_than_one(tmp_path):
    boards = [Board(id="a", source="", origin="freerouting-fixtures", referee="java-drc", tiers=["canary"], nets=5, layers=2)]
    r1 = _write_run(tmp_path, "r1", {
        "java": {"a": [cell(score=900)] * 3},
        "rs":   {"a": [cell(score=990)] * 3}}, jobs=4)
    cmp = compare.compare([r1], baseline="java", against=["rs"], boards=boards)
    assert cmp["config"]["time_metric"] == "cpu_s"


def test_time_metric_explicit_override(tmp_path):
    boards = [Board(id="a", source="", origin="freerouting-fixtures", referee="java-drc", tiers=["canary"], nets=5, layers=2)]
    r1 = _write_run(tmp_path, "r1", {
        "java": {"a": [cell(score=900)] * 3},
        "rs":   {"a": [cell(score=990)] * 3}}, jobs=1)
    cmp = compare.compare([r1], baseline="java", against=["rs"], boards=boards, time_metric="cpu")
    assert cmp["config"]["time_metric"] == "cpu_s"
    cmp2 = compare.compare([r1], baseline="java", against=["rs"], boards=boards, time_metric="wall")
    assert cmp2["config"]["time_metric"] == "wall_s"


def test_verdict_uses_chosen_time_metric_for_level_5():
    def cells_with_cpu(wall, cpu):
        out = []
        for _ in range(3):
            c = cell(score=900, wall=wall)
            c["cpu_s"] = cpu  # decouple cpu_s from wall_s, unlike the cell() default
            out.append(c)
        return out

    base = compare.aggregate(cells_with_cpu(wall=10, cpu=10))
    noise = {"clean_pass_rate": 0, "unrouted": 0, "violations": 0, "score": 30, "wall_s": 1, "cpu_s": 1, "peak_rss_mb": 0}
    # differs in wall_s but not cpu_s -> win at level 5 when time_metric=wall_s, tie
    # (falls through past a metric that doesn't differ) when time_metric=cpu_s
    other = compare.aggregate(cells_with_cpu(wall=5, cpu=10))
    assert compare.verdict(base, other, noise, time_metric="wall_s") == {"result": "win", "level": "wall_s"}
    assert compare.verdict(base, other, noise, time_metric="cpu_s") == {"result": "tie", "level": "peak_rss_mb"}


def test_tier_time_ratio_with_missing_board(tmp_path):
    boards = [Board(id="a", source="", origin="freerouting-fixtures", referee="java-drc", tiers=["canary"], nets=5, layers=2),
              Board(id="z", source="", origin="freerouting-fixtures", referee="java-drc", tiers=["canary"], nets=5, layers=2)]
    r1 = _write_run(tmp_path, "r1", {
        "java": {"a": [cell(wall=10)] * 3, "z": [cell(wall=20)] * 3},
        "rs":   {"z": [cell(wall=5)] * 3}})
    cmp = compare.compare([r1], baseline="java", against=["rs"], boards=boards)
    assert cmp["boards"]["a"]["against"]["rs"] is None
    assert cmp["tiers"]["canary"]["rs"]["median_time_ratio"] == pytest.approx(0.25)


def test_unjudged_cells_excluded_from_aggregate_one_of_three(tmp_path):
    boards = [Board(id="a", source="", origin="freerouting-fixtures", referee="java-drc", tiers=["canary"],
                    nets=5, layers=2)]
    r1 = _write_run(tmp_path, "r1", {
        "java": {"a": [cell(score=900)] * 3},
        "rs": {"a": [cell(score=900), cell(score=910),
                     cell(failed=True, unjudged=True, score=0, unrouted=5)]}})
    cmp = compare.compare([r1], baseline="java", against=["rs"], boards=boards)
    against = cmp["boards"]["a"]["against"]["rs"]
    assert against is not None
    assert against["agg"]["n"] == 2  # the unjudged seed is excluded from the aggregate...
    assert against["agg"]["score"] == pytest.approx(905.0)
    failure_entries = [f for f in cmp["failures"] if f["candidate"] == "rs"]
    assert len(failure_entries) == 1 and failure_entries[0]["unjudged"] is True  # ...but still listed as a failure


def test_all_seeds_unjudged_gives_none_and_failure_entries(tmp_path):
    boards = [Board(id="a", source="", origin="freerouting-fixtures", referee="java-drc", tiers=["canary"],
                    nets=5, layers=2)]
    r1 = _write_run(tmp_path, "r1", {
        "java": {"a": [cell(score=900)] * 3},
        "rs": {"a": [cell(failed=True, unjudged=True, score=0, unrouted=5)] * 3}})
    cmp = compare.compare([r1], baseline="java", against=["rs"], boards=boards)
    assert cmp["boards"]["a"]["against"]["rs"] is None
    failure_entries = [f for f in cmp["failures"] if f["candidate"] == "rs"]
    assert len(failure_entries) == 3
    assert all(f["unjudged"] for f in failure_entries)


def test_all_baseline_seeds_unjudged_skips_board(tmp_path):
    boards = [Board(id="a", source="", origin="freerouting-fixtures", referee="java-drc", tiers=["canary"],
                    nets=5, layers=2)]
    r1 = _write_run(tmp_path, "r1", {
        "java": {"a": [cell(failed=True, unjudged=True, score=0, unrouted=5)] * 3},
        "rs": {"a": [cell(score=900)] * 3}})
    cmp = compare.compare([r1], baseline="java", against=["rs"], boards=boards)
    assert "a" not in cmp["boards"]
    failure_entries = [f for f in cmp["failures"] if f["candidate"] == "java"]
    assert len(failure_entries) == 3
