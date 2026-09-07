import json
import sys
from pathlib import Path

from click.testing import CliRunner

from bench import cli, compare, corpus, paths, runner
from bench.corpus import Board

FAKE = Path(__file__).parent / "fake_router.py"
DATA = Path(__file__).parent / "data"


def _env(tmp_path, monkeypatch, board_source="dsn/mini.dsn"):
    monkeypatch.setattr(corpus, "CORPUS", tmp_path / "corpus")
    monkeypatch.setattr(runner, "RESULTS", tmp_path / "results")
    monkeypatch.setattr(paths, "REPORTS", tmp_path / "reports")
    monkeypatch.setattr(paths, "ROOT", tmp_path)
    (tmp_path / "corpus" / "dsn").mkdir(parents=True)
    (tmp_path / "corpus" / "dsn" / "mini.dsn").write_text((DATA / "mini.dsn").read_text())
    board = Board(id="mini", source=board_source, origin="freerouting-fixtures",
                  referee="java-drc", tiers=["canary"], nets=3, layers=2)
    corpus.save_manifest([board])
    (tmp_path / "candidates.toml").write_text(
        f'[candidates.fake]\nkind = "other"\nexec = ["{sys.executable}", "{FAKE}"]\nsha = "f"\n'
    )
    return board


def test_run_missing_board_input_raises_click_exception(tmp_path, monkeypatch):
    _env(tmp_path, monkeypatch, board_source="dsn/does-not-exist.dsn")
    r = CliRunner().invoke(cli.main, ["run", "--candidates", "fake", "--boards", "mini",
                                      "--no-referee", "--run-id", "t1"])
    assert r.exit_code != 0
    assert "mini" in r.output
    assert "bench corpus init" in r.output


def test_run_unknown_candidate_raises_click_exception(tmp_path, monkeypatch):
    _env(tmp_path, monkeypatch)
    r = CliRunner().invoke(cli.main, ["run", "--candidates", "nope", "--boards", "mini",
                                      "--no-referee", "--run-id", "t1"])
    assert r.exit_code != 0
    assert "nope" in r.output


def test_run_unknown_board_id_raises_click_exception(tmp_path, monkeypatch):
    _env(tmp_path, monkeypatch)
    r = CliRunner().invoke(cli.main, ["run", "--candidates", "fake", "--boards", "nonexistent-board",
                                      "--no-referee", "--run-id", "t1"])
    assert r.exit_code != 0
    assert "nonexistent-board" in r.output


def test_run_succeeds_when_input_present(tmp_path, monkeypatch):
    _env(tmp_path, monkeypatch)
    monkeypatch.setenv("FAKE_MODE", "ok")
    r = CliRunner().invoke(cli.main, ["run", "--candidates", "fake", "--boards", "mini",
                                      "--no-referee", "--run-id", "t1"])
    assert r.exit_code == 0, r.output


def test_run_candidates_file_option_overrides_default(tmp_path, monkeypatch):
    _env(tmp_path, monkeypatch)  # writes tmp_path/candidates.toml with only "fake"
    monkeypatch.setenv("FAKE_MODE", "ok")
    other_dir = tmp_path / "elsewhere"
    other_dir.mkdir()
    (other_dir / "other-candidates.toml").write_text(
        f'[candidates.other-fake]\nkind = "other"\nexec = ["{sys.executable}", "{FAKE}"]\nsha = "g"\n'
    )
    r = CliRunner().invoke(cli.main, ["run", "--candidates", "other-fake",
                                      "--candidates-file", str(other_dir / "other-candidates.toml"),
                                      "--boards", "mini", "--no-referee", "--run-id", "t1"])
    assert r.exit_code == 0, r.output
    meta = json.loads((tmp_path / "results" / "t1" / "meta.json").read_text())
    assert meta["candidates"][0]["name"] == "other-fake"


def test_run_bench_candidates_env_var_overrides_default(tmp_path, monkeypatch):
    _env(tmp_path, monkeypatch)
    monkeypatch.setenv("FAKE_MODE", "ok")
    other_dir = tmp_path / "elsewhere2"
    other_dir.mkdir()
    (other_dir / "env-candidates.toml").write_text(
        f'[candidates.env-fake]\nkind = "other"\nexec = ["{sys.executable}", "{FAKE}"]\nsha = "h"\n'
    )
    monkeypatch.setenv("BENCH_CANDIDATES", str(other_dir / "env-candidates.toml"))
    r = CliRunner().invoke(cli.main, ["run", "--candidates", "env-fake",
                                      "--boards", "mini", "--no-referee", "--run-id", "t2"])
    assert r.exit_code == 0, r.output
    meta = json.loads((tmp_path / "results" / "t2" / "meta.json").read_text())
    assert meta["candidates"][0]["name"] == "env-fake"


def test_compare_unknown_run_id_raises_click_exception(tmp_path, monkeypatch):
    _env(tmp_path, monkeypatch)
    r = CliRunner().invoke(cli.main, ["compare", "--baseline", "fake", "--against", "fake",
                                      "--runs", "no-such-run"])
    assert r.exit_code != 0
    assert "no-such-run" in r.output


def _write_run(root: Path, run_id: str, cands: dict, threads=1):
    run_dir = root / run_id
    meta = {"run_id": run_id, "status": "complete",
            "args": {"threads": threads, "max_passes": 100, "timeout_s": 300, "seeds": 3},
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


def _cell(score=900.0):
    return {"clean_pass": True, "unrouted": 0, "violations": 0, "vias": 4, "wirelength_mm": 100.0,
            "score": score, "wall_s": 10.0, "cpu_s": 10.0, "peak_rss_mb": 200.0, "passes": 3,
            "failed": False, "unjudged": False, "disagreement": False, "timed_out": False,
            "wirelength_ratio": None, "via_ratio": None, "referee": "java-drc"}


def test_compare_incompatible_runs_prints_both_full_configs(tmp_path, monkeypatch):
    board = _env(tmp_path, monkeypatch)
    board.tiers = ["canary"]
    corpus.save_manifest([board])
    _write_run(tmp_path / "results", "r1", {"fake": {"mini": [_cell()] * 3}}, threads=1)
    _write_run(tmp_path / "results", "r2", {"fake2": {"mini": [_cell()] * 3}}, threads=4)
    (tmp_path / "candidates.toml").write_text(
        f'[candidates.fake]\nkind = "other"\nexec = ["{sys.executable}", "{FAKE}"]\nsha = "f"\n'
        f'[candidates.fake2]\nkind = "other"\nexec = ["{sys.executable}", "{FAKE}"]\nsha = "f2"\n'
    )
    r = CliRunner().invoke(cli.main, ["compare", "--baseline", "fake", "--against", "fake2",
                                      "--runs", "r1,r2"])
    assert r.exit_code != 0
    assert isinstance(r.exception, SystemExit) or r.exit_code != 0
    # both runs' full config dicts must appear in the error message
    assert "'threads': 1" in r.output
    assert "'threads': 4" in r.output


def test_referee_jobs_scores_cells_concurrently(tmp_path, monkeypatch):
    _env(tmp_path, monkeypatch)
    run_dir = tmp_path / "results" / "r1"
    cells = []
    for seed in (1, 2):
        d = runner.cell_dir(run_dir, "fake", "mini", seed)
        d.mkdir(parents=True)
        cells.append({"candidate": "fake", "board": "mini", "seed": seed, "status": "ok"})
    meta = {"run_id": "r1", "status": "complete",
            "args": {"threads": 1, "max_passes": 100, "timeout_s": 300, "seeds": 2,
                     "jobs": 1, "tier": None, "boards": ["mini"]},
            "candidates": [{"name": "fake", "sha": "f", "version": "1"}], "cells": cells}
    run_dir.mkdir(parents=True, exist_ok=True)
    (run_dir / "meta.json").write_text(json.dumps(meta))

    calls = []

    def fake_score_cell(board, cell, java_exec):
        calls.append(cell)
        m = {"unrouted": 0, "violations": 0, "score": 1000.0, "clean_pass": True,
             "failed": False, "disagreement": False}
        (cell / "metrics.json").write_text(json.dumps(m))
        return m

    monkeypatch.setattr(cli.referee, "score_cell", fake_score_cell)
    r = CliRunner().invoke(cli.main, ["referee", "--run", "r1", "--jobs", "2"])
    assert r.exit_code == 0, r.output
    assert len(calls) == 2
    for seed in (1, 2):
        assert f"fake" in r.output and f"seed {seed}" in r.output
    assert "scored 2 cells: 2 clean pass, 0 failed, 0 disagreements" in r.output


def test_referee_rejects_jobs_below_one(tmp_path, monkeypatch):
    _env(tmp_path, monkeypatch)
    r = CliRunner().invoke(cli.main, ["referee", "--run", "nonexistent", "--jobs", "0"])
    assert r.exit_code != 0
    assert "--jobs must be >= 1" in r.output


def test_corpus_pcbench_rejects_jobs_below_one(tmp_path, monkeypatch):
    monkeypatch.setattr(corpus, "CORPUS", tmp_path / "corpus")
    r = CliRunner().invoke(cli.main, ["corpus", "pcbench", "--clone", str(tmp_path / "pcbench"),
                                      "--jobs", "0"])
    assert r.exit_code != 0
    assert "--jobs must be >= 1" in r.output


def test_export_then_plot_cli_pipeline(tmp_path, monkeypatch):
    _env(tmp_path, monkeypatch)
    monkeypatch.setenv("FAKE_MODE", "ok")
    r = CliRunner().invoke(cli.main, ["run", "--candidates", "fake", "--boards", "mini",
                                      "--no-referee", "--run-id", "t1"])
    assert r.exit_code == 0, r.output

    def fake_score_cell(board, cell, java_exec):
        m = {"unrouted": 0, "violations": 0, "score": 950.0, "clean_pass": True,
             "cpu_s": 1.0, "wall_s": 1.0, "peak_rss_mb": 100.0, "vias": 2,
             "wirelength_mm": 50.0, "wirelength_ratio": None, "via_ratio": None,
             "timed_out": False, "failed": False, "disagreement": False}
        (cell / "metrics.json").write_text(json.dumps(m))
        return m

    monkeypatch.setattr(cli.referee, "score_cell", fake_score_cell)
    r = CliRunner().invoke(cli.main, ["referee", "--run", "t1"])
    assert r.exit_code == 0, r.output

    r = CliRunner().invoke(cli.main, ["export", "--run", "t1", "--candidate", "fake"])
    assert r.exit_code == 0, r.output
    csv_path = tmp_path / "exports" / "t1-fake-f.csv"
    json_path = tmp_path / "exports" / "t1-fake-f.json"
    assert csv_path.exists() and json_path.exists()
    assert "1 board" in r.output
    lines = csv_path.read_text().splitlines()
    assert lines[0].split(",")[:2] == ["candidate", "sha"]
    assert len(lines) == 2

    out_html = tmp_path / "reports" / "plot.html"
    r = CliRunner().invoke(cli.main, ["plot", "--files", str(json_path), "--out", str(out_html)])
    assert r.exit_code == 0, r.output
    assert out_html.exists()
    page = out_html.read_text()
    assert "<svg" in page and "fake @ f" in page


def test_export_unknown_run_raises_click_exception(tmp_path, monkeypatch):
    _env(tmp_path, monkeypatch)
    r = CliRunner().invoke(cli.main, ["export", "--run", "nope", "--candidate", "fake"])
    assert r.exit_code != 0
    assert "nope" in r.output


def test_export_unknown_candidate_raises_click_exception(tmp_path, monkeypatch):
    _env(tmp_path, monkeypatch)
    monkeypatch.setenv("FAKE_MODE", "ok")
    r = CliRunner().invoke(cli.main, ["run", "--candidates", "fake", "--boards", "mini",
                                      "--no-referee", "--run-id", "t1"])
    assert r.exit_code == 0, r.output
    r = CliRunner().invoke(cli.main, ["export", "--run", "t1", "--candidate", "nope"])
    assert r.exit_code != 0
    assert "nope" in r.output


def test_plot_missing_file_raises_click_exception(tmp_path, monkeypatch):
    _env(tmp_path, monkeypatch)
    r = CliRunner().invoke(cli.main, ["plot", "--files", str(tmp_path / "nope.json")])
    assert r.exit_code != 0
    assert "nope.json" in r.output


def test_corpus_kicad_fixtures_rejects_jobs_below_one(tmp_path, monkeypatch):
    monkeypatch.setattr(corpus, "CORPUS", tmp_path / "corpus")
    fixtures = tmp_path / "fixtures"
    fixtures.mkdir()
    r = CliRunner().invoke(cli.main, ["corpus", "kicad-fixtures", "--fixtures", str(fixtures),
                                      "--jobs", "0"])
    assert r.exit_code != 0
    assert "--jobs must be >= 1" in r.output


def test_run_does_not_overwrite_previous_results(tmp_path, monkeypatch):
    _env(tmp_path, monkeypatch)
    args = ["run", "--candidates", "fake", "--boards", "mini", "--no-referee", "--run-id", "once"]
    assert CliRunner().invoke(cli.main, args).exit_code == 0
    meta_path = tmp_path / "results" / "once" / "meta.json"
    original = meta_path.read_bytes()
    repeated = CliRunner().invoke(cli.main, args)
    assert repeated.exit_code != 0 and "already exists" in repeated.output
    assert meta_path.read_bytes() == original


def test_selected_candidate_referee_override_is_used(tmp_path, monkeypatch):
    _env(tmp_path, monkeypatch)
    custom = tmp_path / "custom.toml"
    custom.write_text((tmp_path / "candidates.toml").read_text()
                      + '\n[referee.java]\nexec = ["custom-java"]\n')
    calls = []
    monkeypatch.setattr(cli.referee, "score_cell", lambda b, c, j: calls.append(j))
    result = CliRunner().invoke(cli.main, ["run", "--candidates", "fake", "--candidates-file", str(custom),
                                           "--boards", "mini", "--seeds", "1", "--run-id", "custom"])
    assert result.exit_code == 0, result.output
    assert calls == [["custom-java"]]


def test_regression_gate_writes_reports_before_failing(tmp_path, monkeypatch):
    _env(tmp_path, monkeypatch)
    _write_run(tmp_path / "results", "r", {"head": {"mini": [_cell()] * 3},
                                           "change": {"mini": [_cell(score=800)] * 3}})
    args = ["compare", "--baseline", "head", "--against", "change", "--runs", "r", "--out", "gate"]
    assert CliRunner().invoke(cli.main, args).exit_code == 0
    result = CliRunner().invoke(cli.main, [*args, "--fail-on-regression"])
    assert result.exit_code != 0 and "regression check failed" in result.output
    for suffix in ("json", "md", "html"):
        assert (tmp_path / "reports" / f"gate.{suffix}").exists()


def test_strict_coverage_gate_rejects_missing_repetitions(tmp_path, monkeypatch):
    _env(tmp_path, monkeypatch)
    _write_run(tmp_path / "results", "r", {"head": {"mini": [_cell()] * 3},
                                           "change": {"mini": [_cell()] * 3}})
    (runner.cell_dir(tmp_path / "results" / "r", "change", "mini", 3) / "metrics.json").unlink()
    result = CliRunner().invoke(cli.main, ["compare", "--baseline", "head", "--against", "change", "--runs", "r",
                                           "--out", "gate", "--fail-on-regression", "--require-complete"])
    assert result.exit_code != 0 and "regression check failed" in result.output


def test_candidate_missing_exec_is_a_configuration_error(tmp_path, monkeypatch):
    _env(tmp_path, monkeypatch)
    (tmp_path / "candidates.toml").write_text('[candidates.fake]\nexce = ["typo"]\n')
    r = CliRunner().invoke(cli.main, ["run", "--candidates", "fake", "--no-referee"])
    assert r.exit_code != 0
    assert "exec must be" in r.output and "unknown candidate" not in r.output


def test_kicad_only_run_and_rescore_do_not_resolve_java(tmp_path, monkeypatch):
    board = _env(tmp_path, monkeypatch)
    board.referee = "kicad"
    corpus.save_manifest([board])
    def no_java(*args):
        raise AssertionError("Java must not be resolved")
    monkeypatch.setattr(cli, "_java_exec", no_java)
    monkeypatch.setattr(cli.referee, "score_cell", lambda b, c, j: _cell())
    r = CliRunner().invoke(cli.main, ["run", "--candidates", "fake", "--run-id", "kicad"])
    assert r.exit_code == 0, r.output
    r = CliRunner().invoke(cli.main, ["referee", "--run", "kicad"])
    assert r.exit_code == 0, r.output


def test_missing_java_is_a_clean_cli_error(tmp_path, monkeypatch):
    _env(tmp_path, monkeypatch)
    def unavailable(*args):
        raise paths.ToolMissing("jar missing")
    monkeypatch.setattr(cli, "_java_exec", unavailable)
    r = CliRunner().invoke(cli.main, ["run", "--candidates", "fake"])
    assert r.exit_code != 0 and "Java referee unavailable: jar missing" in r.output


def test_top_up_reuses_recorded_referee_and_default_is_one_repetition(tmp_path, monkeypatch):
    _env(tmp_path, monkeypatch)
    custom = tmp_path / "custom.toml"
    custom.write_text((tmp_path / "candidates.toml").read_text() + '\n[referee.java]\nexec=["custom-java"]\n')
    calls = []
    def score(b, c, j):
        calls.append(j)
        return _cell()
    monkeypatch.setattr(cli.referee, "score_cell", score)
    r = CliRunner().invoke(cli.main, ["run", "--candidates", "fake", "--candidates-file", str(custom), "--run-id", "r"])
    assert r.exit_code == 0, r.output
    meta = runner.load_meta(tmp_path / "results" / "r")
    assert meta["args"]["seeds"] == 1
    assert meta["referee_java"]["exec"] == ["custom-java"]
    mp = runner.cell_dir(tmp_path / "results" / "r", "fake", "mini", 1) / "metrics.json"
    assert json.loads(mp.read_text())["referee_identity"]["exec"] == ["custom-java"]
    mp.unlink()
    custom.unlink()
    r = CliRunner().invoke(cli.main, ["referee", "--run", "r", "--only-missing"])
    assert r.exit_code == 0, r.output
    assert calls == [["custom-java"], ["custom-java"]]


def test_changed_referee_jar_cannot_silently_top_up(tmp_path, monkeypatch):
    _env(tmp_path, monkeypatch)
    jar = tmp_path / "referee.jar"
    jar.write_bytes(b"old-jar")
    command = ["java", "-jar", str(jar)]
    recorded = cli._java_identity(command)
    jar.write_bytes(b"new-jar")
    import pytest
    with pytest.raises(cli.click.ClickException, match="jar has changed"):
        cli._referee_config([corpus.load_manifest()[0]], recorded=recorded)


def test_timing_only_losses_are_advisory_unless_requested(tmp_path, monkeypatch):
    _env(tmp_path, monkeypatch)
    slow = dict(_cell(), wall_s=20, cpu_s=20)
    _write_run(tmp_path / "results", "r", {"head": {"mini": [_cell()] * 3}, "change": {"mini": [slow] * 3}})
    args = ["compare", "--baseline", "head", "--against", "change", "--runs", "r", "--out", "gate", "--fail-on-regression"]
    assert CliRunner().invoke(cli.main, args).exit_code == 0
    r = CliRunner().invoke(cli.main, [*args, "--performance-regression-percent", "10"])
    assert r.exit_code != 0


def _recorded_java_run(tmp_path, monkeypatch):
    _env(tmp_path, monkeypatch)
    jar = tmp_path / "referee.jar"
    jar.write_bytes(b"original")
    identity = cli._java_identity(["java", "-jar", str(jar)])
    run = _write_run(tmp_path / "results", "r", {"fake": {"mini": [_cell()] * 3}})
    meta = runner.load_meta(run)
    meta["referee_java"] = identity
    (run / "meta.json").write_text(json.dumps(meta))
    for seed in (1, 2, 3):
        mp = runner.cell_dir(run, "fake", "mini", seed) / "metrics.json"
        mp.write_text(json.dumps(dict(_cell(), referee_identity=identity)))
    def score(b, c, j):
        result = _cell()
        (c / "metrics.json").write_text(json.dumps(result))
        return result
    monkeypatch.setattr(cli.referee, "score_cell", score)
    return run, jar, identity


def test_full_rescore_accepts_rebuilt_jar_and_commits_identity_after_scoring(tmp_path, monkeypatch):
    run, jar, old = _recorded_java_run(tmp_path, monkeypatch)
    jar.write_bytes(b"rebuilt")
    def score(b, c, j):
        assert runner.load_meta(run)["referee_java"] == old
        assert runner.load_meta(run)["referee_rescore"]["status"] == "incomplete"
        return _cell()
    monkeypatch.setattr(cli.referee, "score_cell", score)
    result = CliRunner().invoke(cli.main, ["referee", "--run", "r"])
    assert result.exit_code == 0, result.output
    meta = runner.load_meta(run)
    assert meta["referee_java"]["jar_sha256"] != old["jar_sha256"]
    assert meta["referee_rescore"]["status"] == "complete"


def test_referee_noop_does_not_resolve_explicit_candidates_file(tmp_path, monkeypatch):
    run, jar, old = _recorded_java_run(tmp_path, monkeypatch)
    config = tmp_path / "local.toml"
    config.write_text(f'[referee.java]\nexec=["java", "-jar", "{jar}"]\n')
    jar.unlink()
    before = (run / "meta.json").read_bytes()
    result = CliRunner().invoke(cli.main, ["referee", "--run", "r", "--only-missing", "--candidates-file", str(config)])
    assert result.exit_code == 0, result.output
    assert "nothing left" in result.output
    assert (run / "meta.json").read_bytes() == before


def test_kicad_only_topup_preserves_recorded_java_identity(tmp_path, monkeypatch):
    run, jar, old = _recorded_java_run(tmp_path, monkeypatch)
    board = corpus.load_manifest()[0]
    board.referee = "kicad"
    corpus.save_manifest([board])
    (runner.cell_dir(run, "fake", "mini", 3) / "metrics.json").unlink()
    config = tmp_path / "local.toml"
    config.write_text('[referee.java]\nexec=["java", "-jar", "/missing.jar"]\n')
    result = CliRunner().invoke(cli.main, ["referee", "--run", "r", "--only-missing", "--candidates-file", str(config)])
    assert result.exit_code == 0, result.output
    assert runner.load_meta(run)["referee_java"] == old


def test_topup_accepts_local_copy_of_remote_jar(tmp_path, monkeypatch):
    run, jar, old = _recorded_java_run(tmp_path, monkeypatch)
    remote = dict(old, exec=["/remote/java", "-jar", "/remote/referee.jar"])
    meta = runner.load_meta(run)
    meta["referee_java"] = remote
    (run / "meta.json").write_text(json.dumps(meta))
    (runner.cell_dir(run, "fake", "mini", 3) / "metrics.json").unlink()
    config = tmp_path / "local.toml"
    config.write_text(f'[referee.java]\nexec=["java", "-jar", "{jar}"]\n')
    args = ["referee", "--run", "r", "--only-missing", "--candidates-file", str(config)]
    jar.write_bytes(b"different")
    rejected = CliRunner().invoke(cli.main, args)
    assert rejected.exit_code != 0 and "cannot change" in rejected.output
    jar.write_bytes(b"original")
    accepted = CliRunner().invoke(cli.main, args)
    assert accepted.exit_code == 0, accepted.output
    assert runner.load_meta(run)["referee_java"] == old


def test_interrupted_rescore_keeps_previous_identity_and_is_detected(tmp_path, monkeypatch):
    run, jar, old = _recorded_java_run(tmp_path, monkeypatch)
    jar.write_bytes(b"replacement")
    calls = []
    def score(b, c, j):
        calls.append(c)
        if len(calls) == 2:
            raise KeyboardInterrupt()
        return _cell()
    monkeypatch.setattr(cli.referee, "score_cell", score)
    result = CliRunner().invoke(cli.main, ["referee", "--run", "r"])
    assert result.exit_code != 0
    assert runner.load_meta(run)["referee_java"] == old
    assert runner.load_meta(run)["referee_rescore"]["status"] == "incomplete"
    cmp = compare.compare([run], "fake", ["fake"], corpus.load_manifest())
    assert any("rescore is incomplete" in w for w in cmp["warnings"])
    assert "mini" in cmp["coverage"]["fake"]["skipped_boards"]
    result = CliRunner().invoke(cli.main, ["referee", "--run", "r", "--only-missing"])
    assert result.exit_code != 0 and "rerun without" in result.output
    monkeypatch.setattr(cli.referee, "score_cell", lambda b, c, j: _cell())
    result = CliRunner().invoke(cli.main, ["referee", "--run", "r"])
    assert result.exit_code == 0, result.output
    assert runner.load_meta(run)["referee_rescore"]["status"] == "complete"


def test_referee_missing_jar_argument_is_a_clean_error(tmp_path, monkeypatch):
    _env(tmp_path, monkeypatch)
    config = tmp_path / "candidates.toml"
    config.write_text(config.read_text() + '\n[referee.java]\nexec=["java", "-jar"]\n')
    result = CliRunner().invoke(cli.main, ["run", "--candidates", "fake"])
    assert result.exit_code != 0
    assert "-jar requires a following jar path" in result.output


def test_requested_performance_gate_fails_insufficient_samples_and_explains_help(tmp_path, monkeypatch):
    _env(tmp_path, monkeypatch)
    _write_run(tmp_path / "results", "r", {"head": {"mini": [_cell()]}, "change": {"mini": [_cell()]}})
    args = ["compare", "--baseline", "head", "--against", "change", "--runs", "r", "--out", "gate",
            "--fail-on-regression", "--performance-regression-percent", "10"]
    result = CliRunner().invoke(cli.main, args)
    assert result.exit_code != 0
    report = json.loads((tmp_path / "reports" / "gate.json").read_text())
    assert report["overall"]["change"]["performance_unmeasured"] == 1
    assert "also fails" in CliRunner().invoke(cli.main, ["compare", "--help"]).output


def test_compare_writes_a_pasteable_pr_summary(tmp_path, monkeypatch):
    _env(tmp_path, monkeypatch)
    _write_run(tmp_path / "results", "r", {"head": {"mini": [_cell()] * 3},
                                           "change": {"mini": [_cell(score=950)] * 3}})
    result = CliRunner().invoke(cli.main, ["compare", "--baseline", "head", "--against", "change",
                                           "--runs", "r", "--out", "paste"])
    assert result.exit_code == 0, result.output
    summary = (tmp_path / "reports" / "paste.pr.md").read_text()
    assert summary.startswith("## Benchmark: change vs head")
    assert "regression gate passes" in summary
    assert "pr-summary --compare paste" in result.output


def test_pr_summary_prints_the_saved_comparison(tmp_path, monkeypatch):
    _env(tmp_path, monkeypatch)
    _write_run(tmp_path / "results", "r", {"head": {"mini": [_cell()] * 3},
                                           "change": {"mini": [_cell(score=950)] * 3}})
    CliRunner().invoke(cli.main, ["compare", "--baseline", "head", "--against", "change",
                                  "--runs", "r", "--out", "paste"])
    result = CliRunner().invoke(cli.main, ["pr-summary", "--compare", "paste"])
    assert result.exit_code == 0, result.output
    assert result.output == (tmp_path / "reports" / "paste.pr.md").read_text()


def test_pr_summary_rejects_an_unknown_comparison(tmp_path, monkeypatch):
    _env(tmp_path, monkeypatch)
    result = CliRunner().invoke(cli.main, ["pr-summary", "--compare", "nope"])
    assert result.exit_code != 0
    assert "bench compare" in result.output


def test_regression_gate_names_the_reason_it_failed(tmp_path, monkeypatch):
    _env(tmp_path, monkeypatch)
    _write_run(tmp_path / "results", "r", {"head": {"mini": [_cell()] * 3},
                                           "change": {"mini": [_cell(score=800)] * 3}})
    result = CliRunner().invoke(cli.main, ["compare", "--baseline", "head", "--against", "change",
                                           "--runs", "r", "--out", "gate", "--fail-on-regression"])
    assert result.exit_code != 0
    assert "change: 1 routing-quality losses" in result.output
    assert "the regression gate **fails**" in (tmp_path / "reports" / "gate.pr.md").read_text()
