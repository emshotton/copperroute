"""Candidate routers: black-box commands, invoked in each kind's own command-line form."""
from __future__ import annotations

import subprocess
import tomllib
from dataclasses import dataclass, field
from pathlib import Path

from . import paths


@dataclass(frozen=True)
class Candidate:
    name: str
    kind: str
    exec: list[str]
    sha: str
    version: str = ""
    extra_args: list[str] = field(default_factory=list)

    @staticmethod
    def hms(seconds: int) -> str:
        h, rem = divmod(int(seconds), 3600)
        m, s = divmod(rem, 60)
        return f"{h:02d}:{m:02d}:{s:02d}"

    def argv(self, *, in_dsn: Path, out_ses: Path, result_json: Path,
             max_passes: int, timeout_s: int, threads: int, seed: int) -> list[str]:
        if self.kind == "rust":
            return self._native_argv(in_dsn=in_dsn, out_ses=out_ses, result_json=result_json,
                                     max_passes=max_passes, timeout_s=timeout_s, threads=threads, seed=seed)
        args = [
            *self.exec,
            "-de", str(in_dsn),
            "-do", str(out_ses),
            "-mp", str(max_passes),
            f"--router.job_timeout={self.hms(timeout_s)}",
            f"--router.max_threads={threads}",
            f"--router.result_json={result_json}",
            "--gui.enabled=false",
            "--api_server.enabled=false",
            "--mcp_server.enabled=false",
        ]
        args += [a.format(seed=seed) for a in self.extra_args]
        return args

    def _native_argv(self, *, in_dsn: Path, out_ses: Path, result_json: Path,
                     max_passes: int, timeout_s: int, threads: int, seed: int) -> list[str]:
        args = [
            *self.exec,
            "route", str(in_dsn),
            "-o", str(out_ses),
            "--max-passes", str(max_passes),
            "--timeout", self.hms(timeout_s),
            "--result-json", str(result_json),
            "--set", f"router.max_threads={threads}",
        ]
        for extra in self.extra_args:
            extra = extra.format(seed=seed)
            if extra.startswith("--router."):
                args += ["--set", extra[2:]]
            else:
                args.append(extra)
        return args

    def to_json(self) -> dict:
        return {"name": self.name, "kind": self.kind, "exec": self.exec,
                "sha": self.sha, "version": self.version, "extra_args": self.extra_args}


def _git_sha(repo: Path) -> str:
    try:
        out = subprocess.run(["git", "-C", str(repo), "rev-parse", "--short=12", "HEAD"],
                             capture_output=True, text=True, check=True)
        return out.stdout.strip()
    except (subprocess.CalledProcessError, FileNotFoundError):
        return "unknown"


def _check_exec(exec: list[str], base: Path) -> list[str]:
    """Resolve the first path-like element relative to `base` and verify it exists.
    Substitutes {java} placeholder with paths.java_exe() before path checking."""
    resolved = list(exec)
    for i, part in enumerate(resolved):
        if part == "{java}":
            resolved[i] = paths.java_exe()
    for i, part in enumerate(resolved):
        if "/" in part or part.endswith(".jar"):
            p = Path(part)
            if not p.is_absolute():
                p = (base / p).resolve()
            if not p.exists():
                raise FileNotFoundError(f"candidate executable/jar not found: {p}")
            resolved[i] = str(p)
    return resolved


def _sha_from_file(base: Path, rel: str) -> str:
    """Read a sha written by a sync step (e.g. scripts/remote-run.sh's
    `git rev-parse --short HEAD > binaries/freerouting-current.sha`), relative to `base`.
    Falls back to "unknown" if the file doesn't exist (e.g. the sync step hasn't run yet)."""
    try:
        return (base / rel).read_text().strip() or "unknown"
    except FileNotFoundError:
        return "unknown"


def load_candidates(path: Path, names: list[str] | None = None) -> dict[str, Candidate]:
    data = tomllib.loads(path.read_text())
    base = path.parent
    out: dict[str, Candidate] = {}
    table = data.get("candidates", {})
    selected = list(table) if names is None else list(dict.fromkeys(names))
    for name in selected:
        if name not in table:
            raise KeyError(name)
    for name in selected:
        c = table[name]
        command = c.get("exec")
        if not isinstance(command, list) or not command or not all(isinstance(a, str) for a in command):
            raise ValueError(f"candidate {name!r} in {path}: exec must be a nonempty array of strings")
        exec = _check_exec(command, base)
        if c.get("sha"):
            sha = c["sha"]
        elif c.get("sha_file"):
            sha = _sha_from_file(base, c["sha_file"])
        elif c.get("sha_from"):
            sha = _git_sha(base / c["sha_from"])
        else:
            sha = "unknown"
        out[name] = Candidate(name=name, kind=c.get("kind", "other"), exec=exec, sha=sha,
                              version=c.get("version", ""), extra_args=list(c.get("extra_args", [])))
    return out
