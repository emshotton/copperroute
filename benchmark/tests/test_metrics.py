import json
from pathlib import Path

import pytest

from bench import metrics
from bench.corpus import Board

DATA = Path(__file__).parent / "data"


def test_score_matches_java_formula():
    # max = 10 nets * 5e6 = 5e7; penalties: 1 unrouted (5e6) + 2 violations (2e6) + 6 bends (60)
    # costs: 123.4 mm * 1.0 + 4 vias * 50 = 323.4
    expected = max(0.0, (5e7 - 5e6 - 2e6 - 60 - 323.4) / 5e7) * 1000
    assert metrics.score(nets=10, unrouted=1, violations=2, bends=6, length_mm=123.4, vias=4) == pytest.approx(expected)


def test_score_clamps_to_zero():
    assert metrics.score(nets=1, unrouted=1, violations=5, bends=0, length_mm=0, vias=0) == 0.0


def _cell(tmp_path, referee: dict, result=None, time=None) -> Path:
    cell = tmp_path / "seed-1"
    cell.mkdir()
    (cell / "result.json").write_text(json.dumps(result if result is not None else json.loads((DATA / "result_ok.json").read_text())))
    (cell / "referee.json").write_text(json.dumps(referee))
    (cell / "time.json").write_text(json.dumps(time or {"wall_s": 1.5, "cpu_s": 1.2, "peak_rss_mb": 300.0, "exit_code": 0, "timed_out": False}))
    return cell


BOARD = Board(id="mini", source="dsn/mini.dsn", origin="freerouting-fixtures", referee="java-drc",
              tiers=[], nets=10, layers=2)


def test_build_uses_referee_numbers_and_flags_disagreement(tmp_path):
    cell = _cell(tmp_path, {"status": "ok", "referee": "java-drc", "unrouted": 1, "violations": 0,
                            "violations_by_type": {}, "vias": 4, "wirelength_mm": 100.0, "bends": None})
    m = metrics.build(cell, BOARD)
    assert m["unrouted"] == 1 and m["clean_pass"] is False
    assert m["disagreement"] is True          # self says 0 unrouted
    assert m["self"]["unrouted"] == 0
    assert m["passes"] == 3 and m["wall_s"] == 1.5
    assert m["score"] == pytest.approx(metrics.score(10, 1, 0, 0, 100.0, 4))
    assert m["bends"] == 0                    # referee didn't report bends: dropped, not self-reported
    assert m["self"]["bends"] == 6            # self-report still visible informationally
    assert (cell / "metrics.json").exists()


def test_build_clean_pass(tmp_path):
    cell = _cell(tmp_path, {"status": "ok", "referee": "java-drc", "unrouted": 0, "violations": 0,
                            "violations_by_type": {}, "vias": 4, "wirelength_mm": 123.4, "bends": None})
    m = metrics.build(cell, BOARD)
    assert m["clean_pass"] is True and m["disagreement"] is False and m["failed"] is False


def test_build_failed_cell_when_referee_failed(tmp_path):
    cell = _cell(tmp_path, {"status": "referee_failed", "referee": "java-drc", "reason": "jar exploded"})
    m = metrics.build(cell, BOARD)
    assert m["failed"] is True and m["clean_pass"] is False and m["unrouted"] == 10


def test_no_candidate_output_is_failed_but_not_unjudged(tmp_path):
    """No out.ses -- charged to the candidate (spec §7.1/§8 case a): failed, not unjudged."""
    cell = _cell(tmp_path, {"status": "referee_failed", "referee": "java-drc", "reason": "out.ses missing",
                            "candidate_output": False})
    m = metrics.build(cell, BOARD)
    assert m["failed"] is True and m["unjudged"] is False


def test_referee_failure_with_candidate_output_is_unjudged(tmp_path):
    """out.ses exists but the referee itself failed (spec §7.1/§8 case b): unjudged, excluded
    from compare aggregation rather than charged as a routing failure."""
    cell = _cell(tmp_path, {"status": "referee_failed", "referee": "java-drc", "reason": "jar crashed",
                            "candidate_output": True})
    m = metrics.build(cell, BOARD)
    assert m["failed"] is True and m["unjudged"] is True


def test_isolated_config_defaults_false_and_propagates_from_time_json(tmp_path):
    """`isolated_config` in metrics.json mirrors time.json's flag, defaulting to False for
    old runs made before per-cell HOME/XDG isolation existed (time.json has no such key)."""
    referee = {"status": "ok", "referee": "java-drc", "unrouted": 0, "violations": 0,
               "violations_by_type": {}, "vias": 4, "wirelength_mm": 100.0, "bends": None}
    old_base, new_base = tmp_path / "old", tmp_path / "new"
    old_base.mkdir()
    new_base.mkdir()

    old_cell = _cell(old_base, referee,
                     time={"wall_s": 1.5, "cpu_s": 1.2, "peak_rss_mb": 300.0, "exit_code": 0, "timed_out": False})
    assert metrics.build(old_cell, BOARD)["isolated_config"] is False

    new_cell = _cell(new_base, referee,
                     time={"wall_s": 1.5, "cpu_s": 1.2, "peak_rss_mb": 300.0, "exit_code": 0,
                           "timed_out": False, "isolated_config": True})
    assert metrics.build(new_cell, BOARD)["isolated_config"] is True


def test_score_basis_connections_self(tmp_path):
    """result_ok.json's board_statistics.connections.maximum_count is 10; that self-reported
    connection count -- not the manifest's DSN net count -- is the N used for score."""
    cell = _cell(tmp_path, {"status": "ok", "referee": "java-drc", "unrouted": 0, "violations": 0,
                            "violations_by_type": {}, "vias": 4, "wirelength_mm": 100.0, "bends": None})
    m = metrics.build(cell, BOARD)
    assert m["score_basis"] == "connections:self"
    assert m["score_n"] == 10


def test_score_basis_connections_manifest_when_self_absent(tmp_path):
    board = Board(id="mini2", source="dsn/mini2.dsn", origin="freerouting-fixtures", referee="java-drc",
                  tiers=[], nets=7, layers=2, connections=12)
    cell = _cell(tmp_path, {"status": "ok", "referee": "java-drc", "unrouted": 0, "violations": 0,
                            "violations_by_type": {}, "vias": 4, "wirelength_mm": 100.0, "bends": None},
                 result={})
    m = metrics.build(cell, board)
    assert m["score_basis"] == "connections:manifest"
    assert m["score_n"] == 12
    assert m["score"] == pytest.approx(metrics.score(12, 0, 0, 0, 100.0, 4))


def test_score_basis_falls_back_to_nets(tmp_path):
    board = Board(id="mini3", source="dsn/mini3.dsn", origin="freerouting-fixtures", referee="java-drc",
                  tiers=[], nets=7, layers=2, connections=None)
    cell = _cell(tmp_path, {"status": "ok", "referee": "java-drc", "unrouted": 0, "violations": 0,
                            "violations_by_type": {}, "vias": 4, "wirelength_mm": 100.0, "bends": None},
                 result={})
    m = metrics.build(cell, board)
    assert m["score_basis"] == "nets"
    assert m["score_n"] == 7


def test_disagreement_ignores_missing_self_report(tmp_path):
    """self.unrouted/violations are None (no result.json data) -- that's not a disagreement,
    it's an absent self-report."""
    cell = _cell(tmp_path, {"status": "ok", "referee": "java-drc", "unrouted": 1, "violations": 2,
                            "violations_by_type": {}, "vias": 4, "wirelength_mm": 100.0, "bends": None},
                 result={})
    m = metrics.build(cell, BOARD)
    assert m["self"]["unrouted"] is None and m["self"]["violations"] is None
    assert m["disagreement"] is False


def test_build_ground_truth_ratios(tmp_path):
    board = Board(id="pb", source="pcbench/pb/unrouted.dsn", origin="pcbench", referee="kicad", tiers=[],
                  nets=10, layers=2, kicad={"ground_truth": str(tmp_path / "gt.json")})
    (tmp_path / "gt.json").write_text(json.dumps({"wirelength_mm": 50.0, "vias": 2}))
    cell = _cell(tmp_path, {"status": "ok", "referee": "kicad", "unrouted": 0, "violations": 0,
                            "violations_by_type": {}, "vias": 4, "wirelength_mm": 100.0, "bends": None})
    m = metrics.build(cell, board)
    assert m["wirelength_ratio"] == 2.0 and m["via_ratio"] == 2.0
