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


BOARD = Board(id="a", source="", origin="freerouting-fixtures", referee="java-drc",
              tiers=["canary"], nets=5, layers=2)


def test_compare_rejects_multiple_commits_under_one_name(tmp_path):
    r1 = _write_run(tmp_path, "r1", {"rs": {"a": [cell()] * 3}})
    r2 = _write_run(tmp_path, "r2", {"rs": {"a": [cell()] * 3}})
    meta = runner.load_meta(r2)
    meta["candidates"][0]["sha"] = "new-commit"
    (r2 / "meta.json").write_text(json.dumps(meta))
    with pytest.raises(compare.IncompatibleRuns, match="distinct candidate names"):
        compare.compare([r1, r2], "rs", ["rs"], [BOARD], allow_mixed=True)


@pytest.mark.parametrize("mode", ["missing-metrics", "unjudged", "unfinished", "no-boards"])
def test_partial_measurements_remain_comparable_with_explicit_coverage(tmp_path, mode):
    r = _write_run(tmp_path, "r", {"head": {"a": [cell()] * 3},
                                   "change": {"a": [cell(score=1000)] * 3}})
    if mode == "missing-metrics":
        (runner.cell_dir(r, "change", "a", 3) / "metrics.json").unlink()
    elif mode == "unjudged":
        p = runner.cell_dir(r, "change", "a", 3) / "metrics.json"
        p.write_text(json.dumps(cell(failed=True, unjudged=True)))
    elif mode == "unfinished":
        meta = runner.load_meta(r)
        meta["args"]["boards"] = ["a"]
        meta["cells"] = [e for e in meta["cells"] if e["candidate"] == "head"]
        meta["status"] = "incomplete"
        (r / "meta.json").write_text(json.dumps(meta))
    cmp = compare.compare([r], "head", ["change"], [] if mode == "no-boards" else [BOARD])
    if mode in ("unfinished", "no-boards"):
        assert cmp["overall"]["change"]["verdict"] == "inconclusive"
    else:
        assert cmp["overall"]["change"]["verdict"] == "better"
        assert cmp["coverage"]["change"]["incomplete_boards"] == ["a"]


def test_referee_mismatch_skips_only_affected_board(tmp_path):
    changed = cell()
    changed["referee"] = "java-drc(fallback)"
    r = _write_run(tmp_path, "r", {"head": {"a": [cell()] * 3, "b": [cell()] * 3},
                                   "change": {"a": [changed] * 3, "b": [cell()] * 3}})
    board_b = Board(id="b", source="", origin="pcbench", referee="kicad", tiers=[], nets=5, layers=2)
    cmp = compare.compare([r], "head", ["change"], [BOARD, board_b])
    assert cmp["coverage"]["change"]["compared_boards"] == 1
    assert "referee" in cmp["coverage"]["change"]["skipped_boards"]["a"]
    assert cmp["overall"]["change"]["verdict"] == "same"


def test_different_denominators_are_normalized_even_for_candidate_failure(tmp_path):
    base = dict(cell(), score_version=1, score_n=10)
    other = dict(cell(score=999), score_version=1, score_n=20)
    failure = dict(cell(clean=False, failed=True, unrouted=20), score_version=1, score_n=20)
    r = _write_run(tmp_path, "r", {"head": {"a": [base] * 3}, "change": {"a": [other, other, failure]}})
    cmp = compare.compare([r], "head", ["change"], [BOARD])
    row = cmp["boards"]["a"]
    assert row["baseline"]["score"] == row["against"]["change"]["agg"]["score"]
    assert row["against"]["change"]["verdict"]["level"] == "clean_pass_rate"
    assert cmp["coverage"]["change"]["compared_boards"] == 1
    assert json.loads((runner.cell_dir(r, "change", "a", 1) / "metrics.json").read_text())["score_n"] == 20


def test_score_version_mismatch_omits_score_without_discarding_quality(tmp_path):
    r = _write_run(tmp_path, "r", {"head": {"a": [cell()] * 3},
                                   "change": {"a": [dict(cell(), score_version=2)] * 3}})
    cmp = compare.compare([r], "head", ["change"], [BOARD])
    assert cmp["coverage"]["change"]["compared_boards"] == 1
    assert cmp["boards"]["a"]["against"]["change"]["delta"]["score"] is None


def test_network_hostname_change_is_advisory(tmp_path):
    runs = []
    for name in ("head", "change"):
        r = _write_run(tmp_path, name, {name: {"a": [cell()] * 3}})
        meta = runner.load_meta(r)
        meta["host"] = {"node": name, "machine": "arm64"}
        (r / "meta.json").write_text(json.dumps(meta))
        runs.append(r)
    cmp = compare.compare(runs, "head", ["change"], [BOARD])
    assert any("host metadata differs" in w for w in cmp["warnings"])


def test_compare_unknown_candidate_is_not_a_tie(tmp_path):
    r = _write_run(tmp_path, "r", {"head": {"a": [cell()] * 3}})
    with pytest.raises(compare.IncompatibleRuns, match="absent"):
        compare.compare([r], "head", ["typo"], [BOARD])


def test_replacement_run_finishes_interrupted_plan(tmp_path):
    first = _write_run(tmp_path, "first", {"head": {"a": [cell()] * 3}, "change": {}})
    meta = runner.load_meta(first)
    meta["args"]["boards"] = ["a"]
    meta["status"] = "incomplete"
    (first / "meta.json").write_text(json.dumps(meta))
    second = _write_run(tmp_path, "second", {"change": {"a": [cell()] * 3}})
    cmp = compare.compare([first, second], "head", ["change"], [BOARD])
    assert cmp["coverage"]["change"]["incomplete_boards"] == []
    assert cmp["overall"]["change"]["verdict"] == "same"


def test_shared_selection_does_not_require_candidate_only_board(tmp_path):
    b = Board(id="b", source="", origin="pcbench", referee="kicad", tiers=[], nets=5, layers=2)
    r = _write_run(tmp_path, "r", {"head": {"a": [cell()] * 3},
                                   "change": {"a": [cell()] * 3, "b": [cell()] * 3}})
    cmp = compare.compare([r], "head", ["change"], [BOARD, b])
    assert cmp["coverage"]["change"]["shared_boards"] == ["a"]
    assert cmp["coverage"]["change"]["candidate_only_boards"] == ["b"]
    assert cmp["overall"]["change"]["verdict"] == "same"


def test_repetition_reporting_uses_actual_baseline_and_is_run_order_independent(tmp_path, monkeypatch):
    r1 = _write_run(tmp_path, "first", {"head": {"a": [cell()]}})
    meta = runner.load_meta(r1)
    meta["args"]["seeds"] = 1
    (r1 / "meta.json").write_text(json.dumps(meta))
    r2 = _write_run(tmp_path, "second", {"change": {"a": [cell()] * 3}})
    real_load = runner.load_meta
    calls = []
    def load(path):
        calls.append(path)
        return real_load(path)
    monkeypatch.setattr(runner, "load_meta", load)
    one = compare.compare([r1, r2], "head", ["change"], [BOARD])
    assert calls == [r1, r2]
    two = compare.compare([r2, r1], "head", ["change"], [BOARD])
    assert one["config"]["seeds"] is None
    assert one["config"] == two["config"]
    assert one["boards"] == two["boards"]
    assert any("baseline has 1" in w for w in one["warnings"])


def test_optional_performance_gate_requires_margin_and_repeated_evidence(tmp_path):
    def run_case(name, times, percent):
        r = _write_run(tmp_path, name, {"head": {"a": [cell(wall=10)] * 3},
                                       "change": {"a": [cell(wall=t) for t in times]}})
        return compare.compare([r], "head", ["change"], [BOARD],
                               performance_regression_percent=percent)["overall"]["change"]
    assert run_case("noise", [10.1, 10.2, 10.3], 10)["performance_losses"] == 0
    assert run_case("slower", [15, 15, 15], 10)["performance_losses"] == 1
    assert run_case("short", [15], 10)["performance_unmeasured"] == 1
    assert run_case("advisory", [15], None)["quality_losses"] == 0
