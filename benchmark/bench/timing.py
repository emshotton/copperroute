"""Run a command under /usr/bin/time and capture wall, CPU, peak RSS."""
from __future__ import annotations

import os
import re
import signal
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path

from bench.paths import time_exe


@dataclass(frozen=True)
class TimeResult:
    wall_s: float
    cpu_s: float | None
    peak_rss_mb: float | None
    exit_code: int
    timed_out: bool


_REAL = re.compile(r"([\d.]+)\s+real\s+([\d.]+)\s+user\s+([\d.]+)\s+sys")
_RSS = re.compile(r"(\d+)\s+maximum resident set size")
_GNU_RSS = re.compile(r"Maximum resident set size \(kbytes\): (\d+)")
_GNU_USER = re.compile(r"User time \(seconds\): ([\d.]+)")
_GNU_SYS = re.compile(r"System time \(seconds\): ([\d.]+)")


def parse_bsd_time(text: str) -> tuple[float, float, float]:
    """Return (wall_s, cpu_s, peak_rss_mb) from `time -l` (macOS) or `time -v` (GNU) output."""
    m = _REAL.search(text)
    if m:
        wall, user, sys_ = (float(x) for x in m.groups())
        rss = _RSS.search(text)
        return wall, round(user + sys_, 4), int(rss.group(1)) / (1024 * 1024) if rss else 0.0
    user = _GNU_USER.search(text)
    sys_ = _GNU_SYS.search(text)
    rss = _GNU_RSS.search(text)
    cpu = (float(user.group(1)) if user else 0.0) + (float(sys_.group(1)) if sys_ else 0.0)
    return 0.0, cpu, int(rss.group(1)) / 1024 if rss else 0.0


def run_timed(argv: list[str], *, cwd: Path, timeout_s: float, stdout: Path, stderr: Path,
             env: dict | None = None) -> TimeResult:
    """`env`, if given, is merged over a copy of `os.environ` for the child process (e.g.
    `bench.paths.isolated_env`'s HOME/XDG overrides) -- the parent's own PATH, JAVA_HOME,
    etc. are kept unless `env` itself overrides them. `env=None` (the default) runs the
    child with the parent's environment unchanged."""
    flag = "-l" if sys.platform == "darwin" else "-v"
    time_out = cwd / ".time.txt"
    cmd = [str(time_exe()), flag, "-o", str(time_out), *argv]
    proc_env = {**os.environ, **env} if env is not None else None
    start = time.monotonic()
    timed_out = False
    with open(stdout, "wb") as so, open(stderr, "wb") as se:
        proc = subprocess.Popen(cmd, cwd=cwd, stdout=so, stderr=se, start_new_session=True, env=proc_env)
        try:
            proc.wait(timeout=timeout_s)
        except subprocess.TimeoutExpired:
            timed_out = True
            try:
                os.killpg(proc.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass  # process group already gone (e.g. it exited between the timeout and the kill)
            proc.wait()
    wall = time.monotonic() - start
    exit_code = proc.returncode if proc.returncode is not None else -1
    if not time_out.exists():
        # `time` itself never ran/wrote (e.g. the wrapped process was killed before `time`
        # could report, or `time` itself failed to start) -- cpu/rss are simply unknown,
        # not zero.
        return TimeResult(wall_s=round(wall, 3), cpu_s=None, peak_rss_mb=None,
                          exit_code=exit_code, timed_out=timed_out)
    parsed_wall, cpu, rss = parse_bsd_time(time_out.read_text())
    return TimeResult(wall_s=round(parsed_wall or wall, 3), cpu_s=cpu, peak_rss_mb=round(rss, 1),
                      exit_code=exit_code, timed_out=timed_out)
