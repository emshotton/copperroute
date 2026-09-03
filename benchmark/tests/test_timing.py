import sys
from pathlib import Path

from bench.timing import parse_bsd_time, run_timed

SAMPLE = """        1.23 real         0.98 user         0.11 sys
            123456789  maximum resident set size
                   0  average shared memory size
"""


def test_parse_bsd_time():
    t = parse_bsd_time(SAMPLE)
    assert t == (1.23, 1.09, 123456789 / (1024 * 1024))


def test_run_timed_captures_exit_and_output(tmp_path: Path):
    out, err = tmp_path / "o.log", tmp_path / "e.log"
    r = run_timed([sys.executable, "-c", "import sys;print('hi');sys.exit(4)"], cwd=tmp_path,
                  timeout_s=10, stdout=out, stderr=err)
    assert r.exit_code == 4 and not r.timed_out
    assert out.read_text().strip() == "hi"
    assert r.wall_s >= 0 and r.peak_rss_mb > 0


def test_run_timed_times_out(tmp_path: Path):
    r = run_timed([sys.executable, "-c", "import time;time.sleep(30)"], cwd=tmp_path,
                  timeout_s=1, stdout=tmp_path / "o", stderr=tmp_path / "e")
    assert r.timed_out and r.wall_s < 10


def test_run_timed_survives_process_already_gone_on_kill(tmp_path: Path, monkeypatch):
    """os.killpg racing a process that already exited must not raise out of run_timed."""
    import bench.timing as timing_mod

    def fake_killpg(pid, sig):
        raise ProcessLookupError()

    monkeypatch.setattr(timing_mod.os, "killpg", fake_killpg)
    r = run_timed([sys.executable, "-c", "import time;time.sleep(2)"], cwd=tmp_path,
                  timeout_s=1, stdout=tmp_path / "o", stderr=tmp_path / "e")
    assert r.timed_out is True


def test_run_timed_merges_env_over_parent_environ(tmp_path: Path, monkeypatch):
    """`env` overrides/extends the parent's environment for the child rather than replacing
    it -- the child must still see an unrelated parent var (PATH-like) as well as the
    override."""
    monkeypatch.setenv("BENCH_TEST_PARENT_VAR", "from-parent")
    out, err = tmp_path / "o.log", tmp_path / "e.log"
    r = run_timed(
        [sys.executable, "-c",
         "import os;print(os.environ.get('HOME'));print(os.environ.get('BENCH_TEST_PARENT_VAR'))"],
        cwd=tmp_path, timeout_s=10, stdout=out, stderr=err, env={"HOME": str(tmp_path / "isolated-home")},
    )
    assert r.exit_code == 0
    lines = out.read_text().splitlines()
    assert lines[0] == str(tmp_path / "isolated-home")
    assert lines[1] == "from-parent"


def test_run_timed_records_none_cpu_and_rss_when_time_file_missing(tmp_path: Path, monkeypatch):
    """If `time` itself never writes its report (e.g. wrapped process killed first, or
    `time` fails to start), cpu_s/peak_rss_mb must be None, not a misleading 0.0."""
    import bench.timing as timing_mod

    monkeypatch.setattr(timing_mod, "time_exe", lambda: Path("/usr/bin/true"))
    r = run_timed([sys.executable, "-c", "pass"], cwd=tmp_path, timeout_s=10,
                  stdout=tmp_path / "o", stderr=tmp_path / "e")
    assert r.cpu_s is None
    assert r.peak_rss_mb is None
    assert r.wall_s >= 0
