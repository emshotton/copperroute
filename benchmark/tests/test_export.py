import csv
import json
from pathlib import Path

import pytest

from bench import export, runner
from bench.corpus import Board

CSV_HEADER = ("candidate,sha,board,tier,nets,layers,clean_pass,unrouted,violations,score,"
             "cpu_s,wall_s,peak_rss_mb,vias,wirelength_mm,wirelength_ratio,via_ratio,timed_out")


def _cell(board="x", **over):
    base = {"schema_version": 1, "score_version": 1, "board": board, "referee": "java-drc",
            "clean_pass": True, "unrouted": 0, "violations": 0, "vias": 4,
            "wirelength_mm": 100.0, "bends": 2, "score": 950.0, "score_basis": "nets", "score_n": 5,
            "passes": 3, "wall_s": 10.0, "cpu_s": 9.5, "peak_rss_mb": 200.0, "timed_out": False,
            "exit_code": 0, "isolated_config": True, "failed": False, "failure_reason": "",
            "unjudged": False, "disagreement": False, "self": {}, "wirelength_ratio": None, "via_ratio": None}
    base.update(over)
    return base


def _write_run(root: Path, run_id: str, cand: str, sha: str, boards: dict[str, dict]) -> Path:
    run_dir = root / run_id
    cells_meta = []
    for bid, m in boards.items():
        d = runner.cell_dir(run_dir, cand, bid, 1)
        d.mkdir(parents=True)
        (d / "metrics.json").write_text(json.dumps(m))
        cells_meta.append({"candidate": cand, "board": bid, "seed": 1, "status": "ok"})
    meta = {"schema_version": 1, "run_id": run_id, "status": "complete",
            "host": {"node": "test-host", "machine": "x86_64"},
            "args": {"seeds": 1, "max_passes": 100, "timeout_s": 300, "threads": 1, "jobs": 1,
                     "tier": None, "boards": list(boards)},
            "candidates": [{"name": cand, "kind": "other", "exec": ["x"], "sha": sha,
                           "version": "v1", "extra_args": []}],
            "cells": cells_meta}
    run_dir.mkdir(parents=True, exist_ok=True)
    (run_dir / "meta.json").write_text(json.dumps(meta))
    return run_dir


def _boards():
    return [
        Board(id="a", source="", origin="freerouting-fixtures", referee="java-drc",
             tiers=["regression", "d3-a"], nets=5, layers=2),
        Board(id="b", source="", origin="freerouting-fixtures", referee="java-drc",
             tiers=["hard"], nets=50, layers=4),
        Board(id="excluded-board", source="", origin="freerouting-fixtures", referee="java-drc",
             tiers=["regression"], nets=5, layers=2, status="excluded", reason="bad"),
    ]


def test_export_run_rows_sorted_and_tier_prefers_d3(tmp_path):
    boards = _boards()
    run_dir = _write_run(tmp_path, "r1", "cand", "sha1234567890", {
        "b": _cell(board="b", unrouted=1, violations=0, clean_pass=False, score=800.0),
        "a": _cell(board="a"),
    })
    rows = export.export_run(run_dir, "cand", boards)
    assert [r["board"] for r in rows] == ["a", "b"]
    assert rows[0]["tier"] == "d3-a"
    assert rows[1]["tier"] == "hard"
    assert all(r["candidate"] == "cand" and r["sha"] == "sha1234567890" for r in rows)


def test_export_run_skips_boards_excluded_from_manifest(tmp_path):
    boards = _boards()
    run_dir = _write_run(tmp_path, "r1", "cand", "sha1", {
        "a": _cell(board="a"),
        "excluded-board": _cell(board="excluded-board"),
        "not-in-manifest-at-all": _cell(board="not-in-manifest-at-all"),
    })
    rows = export.export_run(run_dir, "cand", boards)
    assert [r["board"] for r in rows] == ["a"]
    assert export.excluded_count(run_dir, "cand", boards) == 2


def test_export_run_unknown_candidate_raises_keyerror(tmp_path):
    boards = _boards()
    run_dir = _write_run(tmp_path, "r1", "cand", "sha1", {"a": _cell(board="a")})
    with pytest.raises(KeyError):
        export.export_run(run_dir, "nope", boards)


def test_write_export_csv_header_row_count_and_bool_rendering(tmp_path):
    boards = _boards()
    run_dir = _write_run(tmp_path, "r1", "cand", "sha1234567890", {
        "a": _cell(board="a", timed_out=True, wirelength_ratio=None, via_ratio=1.1),
        "b": _cell(board="b"),
    })
    result = export.write_export(run_dir, "cand", boards, tmp_path / "exports")
    lines = result["csv_path"].read_text().strip("\n").split("\n")
    assert lines[0] == CSV_HEADER
    assert len(lines) == 3  # header + 2 boards
    fields = CSV_HEADER.split(",")
    row_a = dict(zip(fields, lines[1].split(",")))
    assert row_a["board"] == "a"
    assert row_a["candidate"] == "cand"
    assert row_a["sha"] == "sha1234567890"
    assert row_a["clean_pass"] == "true"
    assert row_a["timed_out"] == "true"
    assert row_a["wirelength_ratio"] == ""  # missing value rendered empty
    assert result["skipped"] == 0
    assert result["warning"] is None


def test_write_export_filename_includes_short_sha(tmp_path):
    boards = _boards()
    run_dir = _write_run(tmp_path, "myrun", "cand", "abcdef0123456789", {"a": _cell(board="a")})
    result = export.write_export(run_dir, "cand", boards, tmp_path / "exports")
    assert result["csv_path"].name == "myrun-cand-abcdef0.csv"
    assert result["json_path"].name == "myrun-cand-abcdef0.json"


def test_write_export_unresolved_sha_omits_suffix_and_warns(tmp_path):
    boards = _boards()
    run_dir = _write_run(tmp_path, "myrun", "cand", "unknown", {"a": _cell(board="a")})
    result = export.write_export(run_dir, "cand", boards, tmp_path / "exports")
    assert result["csv_path"].name == "myrun-cand.csv"
    assert result["json_path"].name == "myrun-cand.json"
    assert result["warning"] is not None and "unknown" in result["warning"]


def test_write_export_json_roundtrips_types_and_carries_sha(tmp_path):
    boards = _boards()
    run_dir = _write_run(tmp_path, "r1", "cand", "sha1234567890", {
        "a": _cell(board="a", violations=1, clean_pass=False, wirelength_ratio=1.5, via_ratio=None),
    })
    result = export.write_export(run_dir, "cand", boards, tmp_path / "exports")
    payload = json.loads(result["json_path"].read_text())
    assert payload["schema_version"] == 1
    assert payload["run"] == "r1"
    assert payload["candidate"] == {"name": "cand", "sha": "sha1234567890", "version": "v1"}
    assert payload["router_git_sha"] == "sha1234567890"
    assert payload["config"]["max_passes"] == 100
    assert payload["host"] == {"node": "test-host", "machine": "x86_64"}
    assert "exported_at" in payload
    row = payload["rows"][0]
    assert isinstance(row["nets"], int) and isinstance(row["layers"], int)
    assert isinstance(row["clean_pass"], bool) and row["clean_pass"] is False
    assert isinstance(row["violations"], int) and row["violations"] == 1
    assert isinstance(row["score"], float)
    assert row["wirelength_ratio"] == 1.5
    assert row["via_ratio"] is None
    assert row["sha"] == "sha1234567890"


def test_write_export_csv_round_trips_adversarial_board_id(tmp_path):
    """A board id containing a comma and a double quote must survive CSV quoting/escaping
    intact -- exercised via csv.reader rather than naive string splitting."""
    boards = [Board(id='weird,board"id', source="", origin="freerouting-fixtures",
                   referee="java-drc", tiers=["regression"], nets=5, layers=2)]
    run_dir = _write_run(tmp_path, "r1", "cand", "sha1", {
        'weird,board"id': _cell(board='weird,board"id'),
    })
    result = export.write_export(run_dir, "cand", boards, tmp_path / "exports")
    with result["csv_path"].open(newline="") as f:
        rows = list(csv.reader(f))
    header, data_row = rows[0], rows[1]
    assert header == export.CSV_FIELDS
    assert data_row[header.index("board")] == 'weird,board"id'
    assert len(rows) == 2  # header + exactly one data row, despite the embedded comma/quote


def test_write_export_unknown_candidate_raises_keyerror(tmp_path):
    boards = _boards()
    run_dir = _write_run(tmp_path, "r1", "cand", "sha1", {"a": _cell(board="a")})
    with pytest.raises(KeyError):
        export.write_export(run_dir, "nope", boards, tmp_path / "exports")
