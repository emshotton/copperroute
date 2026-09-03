import json
import sys
from pathlib import Path

import pytest

from bench import corpus, runner
from bench.candidates import Candidate
from bench.corpus import Board

FAKE = Path(__file__).parent / "fake_router.py"
DATA = Path(__file__).parent / "data"


@pytest.fixture
def env(tmp_path, monkeypatch):
    monkeypatch.setattr(corpus, "CORPUS", tmp_path / "corpus")
    monkeypatch.setattr(runner, "RESULTS", tmp_path / "results")
    (tmp_path / "corpus" / "dsn").mkdir(parents=True)
    (tmp_path / "corpus" / "dsn" / "mini.dsn").write_text((DATA / "mini.dsn").read_text())
    board = Board(id="mini", source="dsn/mini.dsn", origin="freerouting-fixtures",
                  referee="java-drc", tiers=["canary"], nets=3, layers=2)
    cand = Candidate(name="fake", kind="other", exec=[sys.executable, str(FAKE)], sha="f")
    return board, cand


def cfg(board, cand, seeds=1, timeout_s=5, grace_s=60):
    return runner.RunConfig(run_id="t1", candidates=[cand], boards=[board], seeds=seeds,
                            max_passes=10, timeout_s=timeout_s, threads=1, tier="canary", grace_s=grace_s)


def test_run_writes_cell_files_and_meta(env, monkeypatch):
    board, cand = env
    monkeypatch.setenv("FAKE_MODE", "ok")
    run_dir = runner.run(cfg(board, cand, seeds=2), referee=None)
    cell = runner.cell_dir(run_dir, "fake", "mini", 1)
    for name in ["in.dsn", "out.ses", "result.json", "time.json", "stdout.log", "stderr.log", "argv.json"]:
        assert (cell / name).exists(), name
    meta = runner.load_meta(run_dir)
    assert meta["status"] == "complete"
    assert [c["seed"] for c in meta["cells"]] == [1, 2]
    assert meta["candidates"][0]["sha"] == "f"
    assert json.loads((cell / "time.json").read_text())["exit_code"] == 0


def test_run_cell_isolates_config_home(env, monkeypatch):
    """The candidate must be launched with HOME/XDG_CONFIG_HOME pointed inside the cell's
    own `home/` dir, not the real user config dir -- otherwise a persisted
    freerouting.json leaks settings between candidates/runs on the same host."""
    board, cand = env
    monkeypatch.setenv("FAKE_MODE", "ok")
    run_dir = runner.run(cfg(board, cand), referee=None)
    cell = runner.cell_dir(run_dir, "fake", "mini", 1)
    home = cell / "home"
    assert home.is_dir()

    env_json = json.loads((cell / "env.json").read_text())
    assert env_json["HOME"] == str(home)
    assert env_json["XDG_CONFIG_HOME"] == str(home / ".config")
    assert env_json["FREEROUTING_ISOLATED_HOME"] == "1"

    result = json.loads((cell / "result.json").read_text())
    assert result["fake_env"]["HOME"] == str(home)
    assert result["fake_env"]["XDG_CONFIG_HOME"] == str(home / ".config")

    time_json = json.loads((cell / "time.json").read_text())
    assert time_json["isolated_config"] is True


def test_meta_records_host(env, monkeypatch):
    board, cand = env
    monkeypatch.setenv("FAKE_MODE", "ok")
    run_dir = runner.run(cfg(board, cand), referee=None)
    meta = runner.load_meta(run_dir)
    assert meta["host"]["node"]  # non-empty hostname
    assert meta["host"]["machine"]  # non-empty arch string


def test_missing_board_input_is_recorded_and_run_continues(env, monkeypatch, tmp_path):
    board, cand = env
    monkeypatch.setenv("FAKE_MODE", "ok")
    # board.path points at a file that doesn't exist on disk
    missing_board = Board(id="ghost", source="dsn/does-not-exist.dsn", origin="freerouting-fixtures",
                          referee="java-drc", tiers=["canary"], nets=3, layers=2)
    run_dir = runner.run(cfg(missing_board, cand, seeds=1), referee=None)
    meta = runner.load_meta(run_dir)
    assert meta["status"] == "complete"
    assert [c["status"] for c in meta["cells"]] == ["missing_input"]
    cell = runner.cell_dir(run_dir, "fake", "ghost", 1)
    assert not (cell / "in.dsn").exists()


def test_crash_is_recorded_and_run_continues(env, monkeypatch):
    board, cand = env
    monkeypatch.setenv("FAKE_MODE", "crash")
    run_dir = runner.run(cfg(board, cand, seeds=2), referee=None)
    meta = runner.load_meta(run_dir)
    assert [c["status"] for c in meta["cells"]] == ["crashed", "crashed"]
    assert meta["status"] == "complete"


def test_hang_is_killed(env, monkeypatch):
    board, cand = env
    monkeypatch.setenv("FAKE_MODE", "hang")
    run_dir = runner.run(cfg(board, cand, timeout_s=1, grace_s=1), referee=None)
    t = json.loads((runner.cell_dir(run_dir, "fake", "mini", 1) / "time.json").read_text())
    assert t["timed_out"] is True
    assert runner.load_meta(run_dir)["cells"][0]["status"] == "timed_out"


def test_referee_hook_called_per_cell(env, monkeypatch):
    board, cand = env
    monkeypatch.setenv("FAKE_MODE", "ok")
    seen = []
    runner.run(cfg(board, cand), referee=lambda b, cell: seen.append((b.id, cell.name)))
    assert seen == [("mini", "seed-1")]


def test_referee_exception_is_recorded_and_run_continues(env, monkeypatch):
    board, cand = env
    monkeypatch.setenv("FAKE_MODE", "ok")

    def boom(b, cell):
        raise RuntimeError("referee blew up")

    run_dir = runner.run(cfg(board, cand, seeds=2), referee=boom)
    meta = runner.load_meta(run_dir)
    assert meta["status"] == "complete"
    assert [c["status"] for c in meta["cells"]] == ["referee_error", "referee_error"]
    for seed in (1, 2):
        cell = runner.cell_dir(run_dir, "fake", "mini", seed)
        ref = json.loads((cell / "referee.json").read_text())
        assert ref["status"] == "referee_failed"
        assert "RuntimeError" in ref["reason"] and "referee blew up" in ref["reason"]
        m = json.loads((cell / "metrics.json").read_text())
        assert m["failed"] is True


def test_run_parallel_preserves_cell_order_and_meta(env, monkeypatch):
    board, cand = env
    monkeypatch.setenv("FAKE_MODE", "ok")
    board2 = Board(id="mini2", source="dsn/mini.dsn", origin="freerouting-fixtures",
                   referee="java-drc", tiers=["canary"], nets=3, layers=2)
    boards = [board, board2]
    config = runner.RunConfig(run_id="par1", candidates=[cand], boards=boards, seeds=2,
                              max_passes=10, timeout_s=5, threads=1, tier="canary", grace_s=60, jobs=4)
    run_dir = runner.run(config, referee=None)
    meta = runner.load_meta(run_dir)
    assert meta["status"] == "complete"
    assert meta["args"]["jobs"] == 4
    expected_order = [(b.id, s) for b in boards for s in (1, 2)]
    actual_order = [(c["board"], c["seed"]) for c in meta["cells"]]
    assert actual_order == expected_order
    assert len(meta["cells"]) == 4
    for b in boards:
        for seed in (1, 2):
            cell = runner.cell_dir(run_dir, "fake", b.id, seed)
            for name in ["in.dsn", "out.ses", "result.json", "time.json"]:
                assert (cell / name).exists(), (b.id, seed, name)


def test_run_parallel_isolates_failures(env, monkeypatch):
    board, cand = env
    monkeypatch.setenv("FAKE_MODE", "ok")
    bad = Candidate(name="bad", kind="other", exec=["/no/such/executable-xyz"], sha="b")
    config = runner.RunConfig(run_id="par2", candidates=[cand, bad], boards=[board], seeds=2,
                              max_passes=10, timeout_s=5, threads=1, tier="canary", grace_s=60, jobs=4)
    run_dir = runner.run(config, referee=None)
    meta = runner.load_meta(run_dir)
    assert meta["status"] == "complete"
    by_cand = {(c["candidate"], c["seed"]): c["status"] for c in meta["cells"]}
    assert by_cand[("fake", 1)] == "ok" and by_cand[("fake", 2)] == "ok"
    assert by_cand[("bad", 1)] == "crashed" and by_cand[("bad", 2)] == "crashed"
    assert len(meta["cells"]) == 4


def test_meta_written_incrementally_under_parallel(env, monkeypatch):
    board, cand = env
    monkeypatch.setenv("FAKE_MODE", "ok")
    board2 = Board(id="mini2", source="dsn/mini.dsn", origin="freerouting-fixtures",
                   referee="java-drc", tiers=["canary"], nets=3, layers=2)

    def flaky_referee(b, cell):
        if b.id == "mini2":
            raise RuntimeError("referee blew up on mini2")

    config = runner.RunConfig(run_id="par3", candidates=[cand], boards=[board, board2], seeds=1,
                              max_passes=10, timeout_s=5, threads=1, tier="canary", grace_s=60, jobs=4)
    run_dir = runner.run(config, referee=flaky_referee)
    meta = runner.load_meta(run_dir)
    assert meta["status"] == "complete"
    by_board = {c["board"]: c["status"] for c in meta["cells"]}
    assert by_board["mini"] == "ok"
    assert by_board["mini2"] == "referee_error"
    assert len(meta["cells"]) == 2


def test_run_cell_programmer_error_isolated_from_other_cells(env, monkeypatch):
    board, cand = env
    monkeypatch.setenv("FAKE_MODE", "ok")
    # extra_args references a `.format()` placeholder Candidate.argv() never supplies, so
    # cand.argv() raises KeyError for every cell of this candidate.
    broken = Candidate(name="broken", kind="other", exec=[sys.executable, str(FAKE)], sha="x",
                       extra_args=["--router.seed={missing}"])
    config = runner.RunConfig(run_id="par4", candidates=[cand, broken], boards=[board], seeds=1,
                              max_passes=10, timeout_s=5, threads=1, tier="canary", grace_s=60, jobs=4)
    run_dir = runner.run(config, referee=None)
    meta = runner.load_meta(run_dir)
    assert meta["status"] == "complete"
    by_cand = {c["candidate"]: c["status"] for c in meta["cells"]}
    assert by_cand["fake"] == "ok"
    assert by_cand["broken"] == "error"
    assert len(meta["cells"]) == 2
    err_cell = runner.cell_dir(run_dir, "broken", "mini", 1)
    err_text = (err_cell / "error.txt").read_text()
    assert "KeyError" in err_text
