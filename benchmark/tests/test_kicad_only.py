from click.testing import CliRunner

from bench import cli, corpus, runner
from bench.corpus import Board
from test_cli import _env


def test_default_scored_run_selects_only_kicad(tmp_path, monkeypatch):
    legacy = _env(tmp_path, monkeypatch, referee="java-drc")
    kicad = Board(id="native", source=legacy.source, origin="pcbench",
                  referee="kicad", tiers=["canary"], nets=3, layers=2)
    corpus.save_manifest([legacy, kicad])
    selected = []
    monkeypatch.setattr(runner, "run", lambda cfg, **kw: selected.extend(cfg.boards))
    result = CliRunner().invoke(cli.main, ["run", "--candidates", "fake"])
    assert result.exit_code == 0, result.output
    assert [b.id for b in selected] == ["native"]


def test_explicit_dsn_scoring_rejected_before_routing(tmp_path, monkeypatch):
    _env(tmp_path, monkeypatch, referee="java-drc")
    calls = []
    monkeypatch.setattr(runner, "run", lambda *a, **kw: calls.append(a))
    result = CliRunner().invoke(cli.main, ["run", "--candidates", "fake", "--boards", "mini"])
    assert result.exit_code != 0
    assert "KiCad reference unavailable for: mini" in result.output
    assert not calls


def test_legacy_rescore_rejected_without_modifying_results(tmp_path, monkeypatch):
    _env(tmp_path, monkeypatch, referee="java-drc")
    from test_cli import _cell, _write_run
    run = _write_run(tmp_path / "results", "legacy", {"fake": {"mini": [_cell()]}})
    before = {p: p.read_bytes() for p in run.rglob("*.json")}
    result = CliRunner().invoke(cli.main, ["referee", "--run", "legacy"])
    assert result.exit_code != 0
    assert "KiCad reference unavailable for: mini" in result.output
    assert {p: p.read_bytes() for p in run.rglob("*.json")} == before


def test_dsn_diagnostics_can_still_route_without_referee(tmp_path, monkeypatch):
    _env(tmp_path, monkeypatch, referee="none")
    calls = []
    monkeypatch.setattr(runner, "run", lambda cfg, **kw: calls.append((cfg, kw)))
    result = CliRunner().invoke(cli.main, ["run", "--candidates", "fake", "--boards", "mini",
                                           "--no-referee"])
    assert result.exit_code == 0, result.output
    assert [b.id for b in calls[0][0].boards] == ["mini"]
    assert calls[0][1]["referee"] is None


def test_direct_scoring_has_no_non_kicad_fallback(tmp_path, monkeypatch):
    import pytest
    from bench import referee
    from bench.referee import kicad
    board = _env(tmp_path, monkeypatch, referee="java-drc")
    calls = []
    monkeypatch.setattr(kicad, "run", lambda *a: calls.append(a))
    with pytest.raises(ValueError, match="KiCad reference unavailable"):
        referee.score_cell(board, tmp_path)
    assert not calls
