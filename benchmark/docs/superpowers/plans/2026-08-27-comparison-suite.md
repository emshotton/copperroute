# Freerouting Comparison Suite Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A Python CLI (`bench`) that routes a board corpus with any number of black-box router candidates (Java freerouting, the Rust fork at any commit), scores outputs with an independent referee, and reports noise-aware comparisons as Markdown, JSON and an HTML dashboard.

**Architecture:** Candidates are shell commands invoked with the Java legacy flags; every (candidate × board × seed) cell writes files into `results/<run-id>/…`; a referee re-scores each cell (Java DRC-only mode for DSN fixtures, KiCad DRC for PCBench boards); `compare` aggregates medians, applies per-board noise bands and a lexicographic verdict, and renders reports.

**Tech Stack:** Python ≥ 3.11, `uv`, `click`, `jinja2`, `pytest`. Java 17+ and the freerouting jar for the Java candidate/referee. KiCad 10 (`kicad-cli` + bundled Python) for the KiCad referee.

**Spec:** `docs/superpowers/specs/2026-08-27-comparison-suite-design.md`

## Global Constraints

- Python `>=3.11`; only `click` and `jinja2` as runtime deps (tomllib is stdlib in 3.11).
- Routers are black boxes: never import from `../freerouting` or `../freerouting-rs`.
- Candidate invocation is identical for every `kind` (spec §3).
- Default `--threads 1`; verdicts use referee numbers only (spec §7.3, §9).
- Score formula constants copied verbatim from Java: `unrouted_penalty=5_000_000`, `violation_penalty=1_000_000`, `bend_penalty=10`, `via_cost=50`, `trace_cost_per_mm=1.0` (spec §8).
- HTML report is self-contained: no external requests, no charting library (spec §10).
- Paths default to macOS locations; every external tool path is overridable by env var (spec §12).
- Commit after every task with a conventional-commit message.

All commands below run from `freerouting-bench/` unless stated otherwise. `uv run pytest` runs the tests.

---

## File map

| File | Responsibility |
|---|---|
| `pyproject.toml` | uv project, `bench` console script, pytest config |
| `bench/paths.py` | project root, tool paths + env overrides |
| `bench/candidates.py` | `Candidate` dataclass, load `candidates.toml`, resolve sha/version, build argv |
| `bench/corpus.py` | `Board` dataclass, manifest read/write, tier selection, DSN introspection |
| `bench/corpus_pcbench.py` | PCBench clone → stripped board → unrouted DSN → ground truth |
| `bench/timing.py` | wrap a command with `/usr/bin/time`, parse wall/cpu/rss |
| `bench/runner.py` | run cells, write cell files, `meta.json` |
| `bench/metrics.py` | `Metrics` record, score formula, build from self-report + referee |
| `bench/referee/__init__.py` | dispatch by `board.referee`, `referee_failed` results |
| `bench/referee/java_drc.py` | Java jar DRC-only mode, SES parsing fallback |
| `bench/referee/kicad.py` | SES import + `kicad-cli pcb drc` |
| `bench/compare.py` | aggregate, noise floor, verdicts, compare JSON |
| `bench/report/markdown.py` | compare JSON → Markdown |
| `bench/report/html.py` + `templates/dashboard.html.j2` | compare JSON(s) → single HTML file |
| `bench/cli.py` | click commands |
| `vendor/kicad/*.py` | copied KiCad-python scripts |
| `tests/` | unit + integration tests, `fake_router.py` |

---

### Task 1: Project skeleton and candidates

**Files:**
- Create: `pyproject.toml`, `README.md`, `candidates.toml`, `bench/__init__.py`, `bench/paths.py`, `bench/candidates.py`, `bench/cli.py`
- Test: `tests/test_candidates.py`

**Interfaces:**
- Produces: `Candidate(name, kind, exec, sha, version, extra_args)`, `load_candidates(path) -> dict[str, Candidate]`, `Candidate.argv(in_dsn, out_ses, result_json, max_passes, timeout_s, threads, seed) -> list[str]`, `paths.ROOT`, `paths.java_jar()`, `paths.kicad_cli()`, `paths.kicad_python()`.

- [ ] **Step 1: Create `pyproject.toml` and install**

```toml
[project]
name = "freerouting-bench"
version = "0.1.0"
description = "Comparison suite for freerouting (Java) and freerouting-rs"
requires-python = ">=3.11"
dependencies = ["click>=8.1", "jinja2>=3.1"]

[project.scripts]
bench = "bench.cli:main"

[dependency-groups]
dev = ["pytest>=8"]

[build-system]
requires = ["hatchling"]
build-backend = "hatchling.build"

[tool.hatch.build.targets.wheel]
packages = ["bench"]

[tool.pytest.ini_options]
testpaths = ["tests"]
markers = ["slow: needs external tools (java jar, kicad)"]
```

Run: `uv sync` — expected: creates `.venv`, no errors. Create empty `bench/__init__.py`.

- [ ] **Step 2: Write `bench/paths.py`**

```python
"""Filesystem locations and external tool paths (env-overridable)."""
from __future__ import annotations

import os
import shutil
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CORPUS = ROOT / "corpus"
RESULTS = ROOT / "results"
REPORTS = ROOT / "reports"
VENDOR_KICAD = ROOT / "vendor" / "kicad"
JAVA_REPO = ROOT.parent / "freerouting"
RUST_REPO = ROOT.parent / "freerouting-rs"


class ToolMissing(RuntimeError):
    pass


def _tool(env: str, default: str, what: str) -> Path:
    p = Path(os.environ.get(env, default))
    if not p.exists():
        raise ToolMissing(f"{what} not found at {p} (override with ${env})")
    return p


def java_jar() -> Path:
    return _tool(
        "FREEROUTING_JAR",
        str(JAVA_REPO / "build" / "libs" / "freerouting-current-executable.jar"),
        "freerouting jar (build with `./gradlew executableJar` in ../freerouting)",
    )


def java_exe() -> str:
    return os.environ.get("FREEROUTING_JAVA", shutil.which("java") or "java")


def kicad_cli() -> Path:
    return _tool(
        "FREEROUTING_KICAD_CLI",
        "/Applications/KiCad/KiCad.app/Contents/MacOS/kicad-cli",
        "kicad-cli",
    )


def kicad_python() -> Path:
    return _tool(
        "FREEROUTING_KICAD_PYTHON",
        "/Applications/KiCad/KiCad.app/Contents/Frameworks/Python.framework/Versions/Current/bin/python3",
        "KiCad bundled python3",
    )


def time_exe() -> Path:
    return _tool("FREEROUTING_TIME", "/usr/bin/time", "GNU/BSD time")
```

- [ ] **Step 3: Write the failing candidates test**

`tests/test_candidates.py`:

```python
from pathlib import Path

import pytest

from bench.candidates import Candidate, load_candidates

TOML = """
[candidates.java-x]
kind = "java"
exec = ["java", "-jar", "{jar}"]
sha = "deadbeef"

[candidates.rs-x]
kind = "rust"
exec = ["{rs}"]
sha = "cafe0000"
extra_args = ["--router.seed={seed}"]
"""


@pytest.fixture
def toml_path(tmp_path: Path) -> Path:
    jar = tmp_path / "fr.jar"
    jar.write_text("")
    rs = tmp_path / "freerouting"
    rs.write_text("")
    p = tmp_path / "candidates.toml"
    p.write_text(TOML.format(jar=jar, rs=rs))
    return p


def test_load_candidates_resolves_exec_and_sha(toml_path):
    cands = load_candidates(toml_path)
    assert set(cands) == {"java-x", "rs-x"}
    assert cands["java-x"].kind == "java"
    assert cands["java-x"].sha == "deadbeef"
    assert cands["java-x"].exec[-1].endswith("fr.jar")


def test_argv_is_identical_shape_for_both_kinds(toml_path):
    cands = load_candidates(toml_path)
    common = dict(in_dsn=Path("a.dsn"), out_ses=Path("a.ses"), result_json=Path("r.json"),
                  max_passes=100, timeout_s=300, threads=1, seed=2)
    j = cands["java-x"].argv(**common)
    r = cands["rs-x"].argv(**common)
    tail = ["-de", "a.dsn", "-do", "a.ses", "-mp", "100", "--router.job_timeout=00:05:00",
            "--router.max_threads=1", "--router.result_json=r.json",
            "--gui.enabled=false", "--api_server.enabled=false", "--mcp_server.enabled=false"]
    assert j[3:] == tail
    assert r[1:1 + len(tail)] == tail
    assert r[-1] == "--router.seed=2"


def test_missing_exec_fails_fast(tmp_path):
    p = tmp_path / "c.toml"
    p.write_text('[candidates.bad]\nkind="rust"\nexec=["/nonexistent/bin"]\nsha="x"\n')
    with pytest.raises(FileNotFoundError):
        load_candidates(p)


def test_timeout_formatting():
    assert Candidate.hms(3661) == "01:01:01"
```

- [ ] **Step 4: Run test to verify it fails**

Run: `uv run pytest tests/test_candidates.py -v`
Expected: FAIL with `ModuleNotFoundError: No module named 'bench.candidates'`

- [ ] **Step 5: Write `bench/candidates.py`**

```python
"""Candidate routers: black-box commands invoked with the Java legacy CLI flags."""
from __future__ import annotations

import subprocess
import tomllib
from dataclasses import dataclass, field
from pathlib import Path


@dataclass(frozen=True)
class Candidate:
    name: str
    kind: str                     # "java" | "rust" | other (informational only)
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
    """Resolve the first path-like element relative to `base` and verify it exists."""
    resolved = list(exec)
    for i, part in enumerate(resolved):
        if "/" in part or part.endswith(".jar"):
            p = Path(part)
            if not p.is_absolute():
                p = (base / p).resolve()
            if not p.exists():
                raise FileNotFoundError(f"candidate executable/jar not found: {p}")
            resolved[i] = str(p)
    return resolved


def load_candidates(path: Path) -> dict[str, Candidate]:
    data = tomllib.loads(path.read_text())
    base = path.parent
    out: dict[str, Candidate] = {}
    for name, c in data.get("candidates", {}).items():
        exec = _check_exec(list(c["exec"]), base)
        sha = c.get("sha") or (_git_sha(base / c["sha_from"]) if c.get("sha_from") else "unknown")
        out[name] = Candidate(name=name, kind=c.get("kind", "other"), exec=exec, sha=sha,
                              version=c.get("version", ""), extra_args=list(c.get("extra_args", [])))
    return out


def load_referee_java(path: Path) -> list[str] | None:
    """Optional `[referee.java] exec = [...]` block; None means use paths.java_jar()."""
    data = tomllib.loads(path.read_text())
    exec = data.get("referee", {}).get("java", {}).get("exec")
    return _check_exec(list(exec), path.parent) if exec else None
```

- [ ] **Step 6: Run tests**

Run: `uv run pytest tests/test_candidates.py -v` — Expected: 4 passed.

- [ ] **Step 7: Write `candidates.toml`, `README.md`, and a stub `bench/cli.py`**

`candidates.toml`:

```toml
# Router candidates. `exec` paths are relative to this file.
# Both kinds are invoked with identical Java legacy flags (see spec §3).

[candidates.java-current]
kind = "java"
exec = ["java", "-Xmx4g", "-jar", "../freerouting/build/libs/freerouting-current-executable.jar"]
sha_from = "../freerouting"

[candidates.rs-main]
kind = "rust"
exec = ["../freerouting-rs/target/release/freerouting"]
sha_from = "../freerouting-rs"
```

`bench/cli.py`:

```python
"""`bench` command-line entry point."""
from __future__ import annotations

import click


@click.group()
def main() -> None:
    """Freerouting comparison suite."""


if __name__ == "__main__":
    main()
```

`README.md`: title, one paragraph from spec §1, "Setup: `uv sync`; build the jar with `./gradlew executableJar` in `../freerouting`", and the CLI summary from spec §11.

Run: `uv run bench --help` — Expected: prints the group help.

- [ ] **Step 8: Commit**

```bash
git add -A && git commit -m "feat: project skeleton, candidates loader and argv builder"
```

---

### Task 2: Corpus manifest and `corpus init`

**Files:**
- Create: `bench/corpus.py`, `corpus/manifest.json` (generated), `tests/test_corpus.py`, `tests/data/mini.dsn`
- Modify: `bench/cli.py`

**Interfaces:**
- Produces: `Board(id, source, origin, referee, tiers, nets, layers, expected_duration_s, kicad, status)`, `load_manifest() -> list[Board]`, `save_manifest(boards)`, `select(boards, tier=None, ids=None) -> list[Board]`, `dsn_info(path) -> tuple[int nets, int layers]`, `init_from_fixtures(fixtures_dir) -> list[Board]`, `Board.path -> Path` (absolute source path).

- [ ] **Step 1: Create a tiny DSN test fixture**

`tests/data/mini.dsn` (minimal but real Specctra structure — the parser only counts):

```
(pcb mini.dsn
  (parser (string_quote ") (space_in_quoted_tokens on) (host_cad "KiCad's Pcbnew"))
  (resolution um 10)
  (unit um)
  (structure
    (layer F.Cu (type signal) (property (index 0)))
    (layer B.Cu (type signal) (property (index 1)))
    (boundary (rect pcb 0 0 100000 100000))
    (via "Via[0-1]_800:400_um")
    (rule (width 250) (clearance 200))
  )
  (placement)
  (library)
  (network
    (net GND (pins U1-1 R1-1))
    (net "Net-(R1-Pad2)" (pins U1-2 R1-2))
    (net VCC (pins U1-3))
    (class kicad_default "" GND "Net-(R1-Pad2)" VCC (circuit (use_via Via[0-1]_800:400_um)) (rule (width 250) (clearance 200)))
  )
  (wiring)
)
```

- [ ] **Step 2: Write the failing corpus tests**

`tests/test_corpus.py`:

```python
import json
from pathlib import Path

from bench import corpus
from bench.corpus import Board, dsn_info, init_from_fixtures, select

DATA = Path(__file__).parent / "data"


def test_dsn_info_counts_nets_and_signal_layers():
    nets, layers = dsn_info(DATA / "mini.dsn")
    assert nets == 3
    assert layers == 2


def test_init_from_fixtures_copies_and_tags(tmp_path, monkeypatch):
    fixtures = tmp_path / "fixtures"
    fixtures.mkdir()
    (fixtures / "Issue508-DAC2020_bm01.dsn").write_text((DATA / "mini.dsn").read_text())
    (fixtures / "Issue143-rpi_splitter_mod.dsn").write_text((DATA / "mini.dsn").read_text())
    (fixtures / "Issue999-notes.json").write_text("{}")
    monkeypatch.setattr(corpus, "CORPUS", tmp_path / "corpus")
    boards = init_from_fixtures(fixtures)
    ids = {b.id for b in boards}
    assert ids == {"dac2020-bm01", "issue143-rpi_splitter_mod"}
    bm01 = next(b for b in boards if b.id == "dac2020-bm01")
    assert bm01.referee == "java-drc"
    assert {"regression", "dac2020", "hard"} <= set(bm01.tiers)
    assert bm01.nets == 3 and bm01.layers == 2
    assert (tmp_path / "corpus" / "dsn" / "Issue508-DAC2020_bm01.dsn").exists()
    assert (tmp_path / "corpus" / "manifest.json").exists()


def test_select_by_tier_and_ids():
    a = Board(id="a", source="dsn/a.dsn", origin="freerouting-fixtures", referee="java-drc",
              tiers=["canary"], nets=1, layers=2)
    b = Board(id="b", source="dsn/b.dsn", origin="freerouting-fixtures", referee="java-drc",
              tiers=["hard"], nets=1, layers=2)
    x = Board(id="x", source="dsn/x.dsn", origin="pcbench", referee="kicad",
              tiers=["d3-a"], nets=1, layers=2, status="excluded")
    assert [s.id for s in select([a, b, x], tier="canary")] == ["a"]
    assert [s.id for s in select([a, b, x], ids=["b", "a"])] == ["b", "a"]
    assert [s.id for s in select([a, b, x])] == ["a", "b"]  # excluded boards dropped


def test_manifest_roundtrip(tmp_path, monkeypatch):
    monkeypatch.setattr(corpus, "CORPUS", tmp_path)
    boards = [Board(id="a", source="dsn/a.dsn", origin="freerouting-fixtures",
                    referee="java-drc", tiers=["canary"], nets=4, layers=2,
                    expected_duration_s=3.5)]
    corpus.save_manifest(boards)
    loaded = corpus.load_manifest()
    assert loaded == boards
    assert json.loads((tmp_path / "manifest.json").read_text())["boards"][0]["id"] == "a"
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `uv run pytest tests/test_corpus.py -v` — Expected: FAIL, `No module named 'bench.corpus'`.

- [ ] **Step 4: Write `bench/corpus.py`**

```python
"""Board corpus: manifest model, tier selection, DSN introspection, fixture import."""
from __future__ import annotations

import json
import re
import shutil
from dataclasses import asdict, dataclass, field
from pathlib import Path

from bench.paths import CORPUS  # re-exported so tests can monkeypatch bench.corpus.CORPUS

HARD_IDS = {"dac2020-bm01", "dac2020-bm05", "dac2020-bm06",
            "issue555-bbd_mars-64", "issue555-cnh_functional_tester_1"}
CANARY_IDS = {"dac2020-bm07", "dac2020-bm08", "issue558-dev-board", "issue143-rpi_splitter_mod"}


@dataclass
class Board:
    id: str
    source: str                         # relative to CORPUS
    origin: str                         # "freerouting-fixtures" | "pcbench"
    referee: str                        # "java-drc" | "kicad"
    tiers: list[str] = field(default_factory=list)
    nets: int = 0
    layers: int = 0
    expected_duration_s: float | None = None
    kicad: dict | None = None           # {"raw":..., "stripped":..., "ground_truth":...}
    status: str = "ok"                  # "ok" | "excluded"
    reason: str = ""

    @property
    def path(self) -> Path:
        return CORPUS / self.source


def manifest_path() -> Path:
    return CORPUS / "manifest.json"


def load_manifest() -> list[Board]:
    p = manifest_path()
    if not p.exists():
        return []
    data = json.loads(p.read_text())
    return [Board(**b) for b in data["boards"]]


def save_manifest(boards: list[Board]) -> None:
    CORPUS.mkdir(parents=True, exist_ok=True)
    payload = {"schema_version": 1, "boards": [asdict(b) for b in sorted(boards, key=lambda b: b.id)]}
    manifest_path().write_text(json.dumps(payload, indent=2) + "\n")


def select(boards: list[Board], tier: str | None = None, ids: list[str] | None = None) -> list[Board]:
    if ids:
        by_id = {b.id: b for b in boards}
        missing = [i for i in ids if i not in by_id]
        if missing:
            raise KeyError(f"unknown board ids: {missing}")
        return [by_id[i] for i in ids]
    out = [b for b in boards if b.status == "ok"]
    if tier:
        out = [b for b in out if tier in b.tiers]
    return out


_NET_RE = re.compile(r"\(net\s+(\"[^\"]*\"|[^\s()]+)")
_LAYER_RE = re.compile(r"\(layer\s+(\"[^\"]*\"|[^\s()]+)\s*\(type\s+signal\)")


def dsn_info(path: Path) -> tuple[int, int]:
    """(net count, signal layer count) by lightweight scanning of a DSN file."""
    text = path.read_text(errors="replace")
    net_start = text.find("(network")
    nets = 0
    if net_start >= 0:
        section = text[net_start:]
        cut = section.find("(class")
        if cut > 0:
            section = section[:cut]
        nets = len(_NET_RE.findall(section))
    layers = len(_LAYER_RE.findall(text))
    return nets, layers


def board_id_for(filename: str) -> str:
    stem = Path(filename).stem
    m = re.match(r"Issue508-DAC2020_(bm\d+)$", stem)
    if m:
        return f"dac2020-{m.group(1)}"
    return stem.lower()


def init_from_fixtures(fixtures_dir: Path) -> list[Board]:
    """Copy every *.dsn from the Java repo's fixtures into corpus/dsn and build manifest entries."""
    dst = CORPUS / "dsn"
    dst.mkdir(parents=True, exist_ok=True)
    existing = {b.id: b for b in load_manifest()}
    boards: list[Board] = [b for b in existing.values() if b.origin != "freerouting-fixtures"]
    for src in sorted(fixtures_dir.glob("*.dsn")):
        shutil.copyfile(src, dst / src.name)
        bid = board_id_for(src.name)
        nets, layers = dsn_info(src)
        tiers = ["regression"]
        if bid.startswith("dac2020-"):
            tiers.append("dac2020")
        if bid in HARD_IDS:
            tiers.append("hard")
        if bid in CANARY_IDS:
            tiers.append("canary")
        prev = existing.get(bid)
        boards.append(Board(id=bid, source=f"dsn/{src.name}", origin="freerouting-fixtures",
                            referee="java-drc", tiers=tiers, nets=nets, layers=layers,
                            expected_duration_s=prev.expected_duration_s if prev else None))
    save_manifest(boards)
    return [b for b in boards if b.origin == "freerouting-fixtures"]
```

- [ ] **Step 5: Run tests**

Run: `uv run pytest tests/test_corpus.py -v` — Expected: 4 passed.

- [ ] **Step 6: Add `corpus init` and `corpus list` commands to `bench/cli.py`**

```python
from pathlib import Path

import click

from bench import corpus, paths


@click.group()
def main() -> None:
    """Freerouting comparison suite."""


@main.group("corpus")
def corpus_cmd() -> None:
    """Manage the board corpus."""


@corpus_cmd.command("init")
@click.option("--fixtures", type=click.Path(exists=True, path_type=Path),
              default=paths.JAVA_REPO / "fixtures", show_default=True)
def corpus_init(fixtures: Path) -> None:
    """Import the Java repo's DSN fixtures."""
    boards = corpus.init_from_fixtures(fixtures)
    click.echo(f"imported {len(boards)} boards into {corpus.CORPUS / 'dsn'}")


@corpus_cmd.command("list")
@click.option("--tier", default=None)
def corpus_list(tier: str | None) -> None:
    for b in corpus.select(corpus.load_manifest(), tier=tier):
        click.echo(f"{b.id:40} nets={b.nets:4} layers={b.layers:2} referee={b.referee:8} tiers={','.join(b.tiers)}")
```

Run: `uv run bench corpus init && uv run bench corpus list --tier canary`
Expected: "imported 105 boards…", then 4 canary lines.

- [ ] **Step 7: Commit** (manifest.json is committed; `corpus/dsn/` is gitignored)

```bash
git add -A && git commit -m "feat: corpus manifest, DSN introspection and fixture import"
```

---

### Task 3: Timing wrapper and runner

**Files:**
- Create: `bench/timing.py`, `bench/runner.py`, `tests/fake_router.py`, `tests/test_timing.py`, `tests/test_runner.py`
- Modify: `bench/cli.py`

**Interfaces:**
- Produces: `timing.run_timed(argv, cwd, timeout_s, stdout, stderr) -> TimeResult(wall_s, cpu_s, peak_rss_mb, exit_code, timed_out)`; `runner.RunConfig(run_id, candidates, boards, seeds, max_passes, timeout_s, threads)`; `runner.run(cfg, on_cell=None) -> Path` (run dir); `runner.cell_dir(run_dir, cand_name, board_id, seed) -> Path`; `runner.load_meta(run_dir) -> dict`.
- Cell files: `in.dsn`, `out.ses`, `result.json`, `time.json`, `stdout.log`, `stderr.log`, `argv.json`.
- `meta.json`: `{run_id, started_at, finished_at, status, args:{seeds,max_passes,timeout_s,threads,tier,boards}, candidates:[Candidate.to_json()], cells:[{candidate,board,seed,status}]}`.

- [ ] **Step 1: Write the fake router** (used by all runner tests; no Java needed)

`tests/fake_router.py`:

```python
"""A stand-in router for tests. Behaviour is chosen by FAKE_MODE env var:
ok (default) | crash | hang | misreport | no_output
Accepts the Java legacy flags and writes out.ses + result.json like a real candidate."""
import json
import os
import sys
import time


def main() -> int:
    args = sys.argv[1:]
    get = lambda flag: args[args.index(flag) + 1]
    out_ses = get("-do")
    result_json = next(a.split("=", 1)[1] for a in args if a.startswith("--router.result_json="))
    mode = os.environ.get("FAKE_MODE", "ok")
    if mode == "hang":
        time.sleep(3600)
    if mode == "crash":
        print("boom", file=sys.stderr)
        return 3
    if mode == "no_output":
        return 0
    with open(out_ses, "w") as f:
        f.write("(session fake (routes (network (net A (via VIA 1 2) (wire (path F.Cu 250 0 0 1000 0))))))\n")
    unrouted = 5 if mode == "misreport" else 0
    manifest = {
        "schema_version": 1, "app_version": "fake-1.0", "git_sha": "fake",
        "phases": {"fanout": {"duration_seconds": 0.0, "passes_completed": 0},
                   "autorouter": {"duration_seconds": 0.5, "passes_completed": 3},
                   "optimizer": {"duration_seconds": 0.1, "passes_completed": 1}},
        "board_statistics": {
            "connections": {"maximum_count": 10, "incomplete_count": unrouted},
            "traces": {"total_count": 8, "total_length_mm": 123.4},
            "vias": {"total_count": 4},
            "bends": {"total_count": 6},
            "clearance_violations": {"total_count": 0},
        },
        "normalized_score": 990.0, "final_state": "COMPLETED", "exit_code": 0, "output_written": True,
    }
    with open(result_json, "w") as f:
        json.dump(manifest, f)
    return 0


if __name__ == "__main__":
    sys.exit(main())
```

- [ ] **Step 2: Write failing timing tests**

`tests/test_timing.py`:

```python
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
```

- [ ] **Step 3: Run to verify failure**

Run: `uv run pytest tests/test_timing.py -v` — Expected: `No module named 'bench.timing'`.

- [ ] **Step 4: Write `bench/timing.py`**

```python
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
    cpu_s: float
    peak_rss_mb: float
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


def run_timed(argv: list[str], *, cwd: Path, timeout_s: float, stdout: Path, stderr: Path) -> TimeResult:
    flag = "-l" if sys.platform == "darwin" else "-v"
    time_out = cwd / ".time.txt"
    cmd = [str(time_exe()), flag, "-o", str(time_out), *argv]
    start = time.monotonic()
    timed_out = False
    with open(stdout, "wb") as so, open(stderr, "wb") as se:
        proc = subprocess.Popen(cmd, cwd=cwd, stdout=so, stderr=se, start_new_session=True)
        try:
            proc.wait(timeout=timeout_s)
        except subprocess.TimeoutExpired:
            timed_out = True
            os.killpg(proc.pid, signal.SIGKILL)
            proc.wait()
    wall = time.monotonic() - start
    text = time_out.read_text() if time_out.exists() else ""
    parsed_wall, cpu, rss = parse_bsd_time(text) if text else (0.0, 0.0, 0.0)
    return TimeResult(wall_s=round(parsed_wall or wall, 3), cpu_s=cpu, peak_rss_mb=round(rss, 1),
                      exit_code=proc.returncode if proc.returncode is not None else -1, timed_out=timed_out)
```

- [ ] **Step 5: Run timing tests** — `uv run pytest tests/test_timing.py -v` — Expected: 3 passed.

- [ ] **Step 6: Write failing runner tests**

`tests/test_runner.py`:

```python
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


def cfg(board, cand, seeds=1, timeout_s=5):
    return runner.RunConfig(run_id="t1", candidates=[cand], boards=[board], seeds=seeds,
                            max_passes=10, timeout_s=timeout_s, threads=1, tier="canary")


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
    run_dir = runner.run(cfg(board, cand, timeout_s=1), referee=None)
    t = json.loads((runner.cell_dir(run_dir, "fake", "mini", 1) / "time.json").read_text())
    assert t["timed_out"] is True
    assert runner.load_meta(run_dir)["cells"][0]["status"] == "timed_out"


def test_referee_hook_called_per_cell(env, monkeypatch):
    board, cand = env
    monkeypatch.setenv("FAKE_MODE", "ok")
    seen = []
    runner.run(cfg(board, cand), referee=lambda b, cell: seen.append((b.id, cell.name)))
    assert seen == [("mini", "seed-1")]
```

- [ ] **Step 7: Run to verify failure** — `uv run pytest tests/test_runner.py -v` — Expected: `No module named 'bench.runner'`.

- [ ] **Step 8: Write `bench/runner.py`**

```python
"""Execute candidate × board × seed cells and persist everything under results/<run-id>/."""
from __future__ import annotations

import json
import shutil
from collections.abc import Callable
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path

from bench.candidates import Candidate
from bench.corpus import Board
from bench.paths import RESULTS  # module attr so tests can monkeypatch bench.runner.RESULTS
from bench.timing import run_timed

GRACE_S = 60
RefereeHook = Callable[[Board, Path], None]


@dataclass
class RunConfig:
    run_id: str
    candidates: list[Candidate]
    boards: list[Board]
    seeds: int = 1
    max_passes: int = 100
    timeout_s: int = 300
    threads: int = 1
    tier: str | None = None
    board_ids: list[str] = field(default_factory=list)


def _now() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="seconds")


def cell_dir(run_dir: Path, cand_name: str, board_id: str, seed: int) -> Path:
    return run_dir / cand_name / board_id / f"seed-{seed}"


def load_meta(run_dir: Path) -> dict:
    return json.loads((run_dir / "meta.json").read_text())


def _save_meta(run_dir: Path, meta: dict) -> None:
    (run_dir / "meta.json").write_text(json.dumps(meta, indent=2) + "\n")


def _cell_status(time_json: dict, cell: Path) -> str:
    if time_json["timed_out"]:
        return "timed_out"
    if not (cell / "out.ses").exists() or not (cell / "result.json").exists():
        return "crashed" if time_json["exit_code"] != 0 else "no_output"
    return "ok"


def run_cell(cand: Candidate, board: Board, seed: int, cfg: RunConfig, run_dir: Path) -> dict:
    cell = cell_dir(run_dir, cand.name, board.id, seed)
    cell.mkdir(parents=True, exist_ok=True)
    in_dsn = cell / "in.dsn"
    shutil.copyfile(board.path, in_dsn)
    argv = cand.argv(in_dsn=in_dsn, out_ses=cell / "out.ses", result_json=cell / "result.json",
                     max_passes=cfg.max_passes, timeout_s=cfg.timeout_s, threads=cfg.threads, seed=seed)
    (cell / "argv.json").write_text(json.dumps(argv, indent=2))
    t = run_timed(argv, cwd=cell, timeout_s=cfg.timeout_s + GRACE_S,
                  stdout=cell / "stdout.log", stderr=cell / "stderr.log")
    time_json = {"wall_s": t.wall_s, "cpu_s": t.cpu_s, "peak_rss_mb": t.peak_rss_mb,
                 "exit_code": t.exit_code, "timed_out": t.timed_out}
    (cell / "time.json").write_text(json.dumps(time_json, indent=2))
    return {"candidate": cand.name, "board": board.id, "seed": seed, "status": _cell_status(time_json, cell)}


def run(cfg: RunConfig, referee: RefereeHook | None,
        progress: Callable[[str], None] | None = None) -> Path:
    run_dir = RESULTS / cfg.run_id
    run_dir.mkdir(parents=True, exist_ok=True)
    meta = {
        "schema_version": 1, "run_id": cfg.run_id, "started_at": _now(), "finished_at": None,
        "status": "incomplete",
        "args": {"seeds": cfg.seeds, "max_passes": cfg.max_passes, "timeout_s": cfg.timeout_s,
                 "threads": cfg.threads, "tier": cfg.tier, "boards": [b.id for b in cfg.boards]},
        "candidates": [c.to_json() for c in cfg.candidates],
        "cells": [],
    }
    _save_meta(run_dir, meta)
    for cand in cfg.candidates:
        for board in cfg.boards:
            for seed in range(1, cfg.seeds + 1):
                if progress:
                    progress(f"{cand.name} × {board.id} × seed {seed}")
                entry = run_cell(cand, board, seed, cfg, run_dir)
                if referee is not None:
                    referee(board, cell_dir(run_dir, cand.name, board.id, seed))
                meta["cells"].append(entry)
                _save_meta(run_dir, meta)
    meta["status"] = "complete"
    meta["finished_at"] = _now()
    _save_meta(run_dir, meta)
    return run_dir
```

- [ ] **Step 9: Run runner tests** — `uv run pytest tests/test_runner.py -v` — Expected: 4 passed.

- [ ] **Step 10: Add `run` command to `bench/cli.py`** (referee wired in Task 4; pass `None` for now)

```python
from datetime import datetime

from bench import runner
from bench.candidates import load_candidates


@main.command("run")
@click.option("--candidates", "cand_names", required=True, help="comma-separated names from candidates.toml")
@click.option("--tier", default=None)
@click.option("--boards", "board_ids", default=None, help="comma-separated board ids")
@click.option("--seeds", default=1, show_default=True)
@click.option("--max-passes", default=100, show_default=True)
@click.option("--timeout", "timeout_s", default=300, show_default=True)
@click.option("--threads", default=1, show_default=True)
@click.option("--run-id", default=None)
@click.option("--no-referee", is_flag=True)
def run_cmd(cand_names, tier, board_ids, seeds, max_passes, timeout_s, threads, run_id, no_referee):
    """Route boards with candidates and score the results."""
    cands = load_candidates(paths.ROOT / "candidates.toml")
    chosen = [cands[n] for n in cand_names.split(",")]
    boards = corpus.select(corpus.load_manifest(), tier=tier,
                           ids=board_ids.split(",") if board_ids else None)
    if not boards:
        raise click.ClickException("no boards selected")
    run_id = run_id or datetime.now().strftime("%Y%m%d-%H%M%S")
    cfg = runner.RunConfig(run_id=run_id, candidates=chosen, boards=boards, seeds=seeds,
                           max_passes=max_passes, timeout_s=timeout_s, threads=threads, tier=tier)
    run_dir = runner.run(cfg, referee=None, progress=click.echo)
    click.echo(f"run written to {run_dir}")
```

- [ ] **Step 11: Commit**

```bash
git add -A && git commit -m "feat: timed runner with per-cell files and meta.json"
```

---

### Task 4: Metrics and the Java DRC referee

**Files:**
- Create: `bench/metrics.py`, `bench/referee/__init__.py`, `bench/referee/java_drc.py`, `tests/test_metrics.py`, `tests/test_java_drc.py`, `tests/data/result_ok.json`, `tests/data/drc_two.json`, `tests/data/mini.ses`
- Modify: `bench/cli.py` (`run` gets the referee; add `referee` command)

**Interfaces:**
- Produces: `metrics.score(nets, unrouted, violations, bends, length_mm, vias) -> float`; `metrics.build(cell: Path, board: Board) -> dict` (writes `metrics.json`, returns it); `referee.score_cell(board, cell, java_exec) -> dict` (writes `referee.json`); `java_drc.parse_ses(path) -> tuple[int vias, float length_mm]`; `java_drc.run(board, cell, java_exec) -> dict`.
- `referee.json` shape: `{"status": "ok"|"referee_failed", "referee": "java-drc"|"kicad", "reason": str, "unrouted": int, "violations": int, "violations_by_type": {type: n}, "vias": int, "wirelength_mm": float, "bends": int|null}`.
- `metrics.json` shape (spec §8): `clean_pass, unrouted, violations, vias, wirelength_mm, score, passes, wall_s, cpu_s, peak_rss_mb, timed_out, exit_code, failed, disagreement, referee, self: {...}, wirelength_ratio, via_ratio`.

- [ ] **Step 1: Create test data**

`tests/data/result_ok.json` — the fake router's manifest with `incomplete_count: 0` (copy the dict from `tests/fake_router.py`, mode ok).

`tests/data/drc_two.json`:

```json
{"$schema": "https://schemas.kicad.org/drc.v1.json", "coordinate_units": "mm",
 "unconnected_items": [{"description": "Missing connection between U1-1 and R1-1", "items": []}],
 "violations": [
  {"description": "Clearance violation", "severity": "error", "type": "clearance", "items": []},
  {"description": "Hole clearance violation", "severity": "error", "type": "hole_clearance", "items": []}
 ], "schematic_parity": []}
```

`tests/data/mini.ses`:

```
(session mini.ses
  (base_design mini.dsn)
  (routes
    (resolution um 10)
    (parser (host_cad "KiCad's Pcbnew"))
    (network_out
      (net GND
        (via "Via[0-1]_800:400_um" 500000 500000)
        (via "Via[0-1]_800:400_um" 600000 500000)
        (wire (path F.Cu 2500 0 0 100000 0))
        (wire (path B.Cu 2500 100000 0 100000 200000 300000 200000))
      )
    )
  )
)
```

(With `resolution um 10`, coordinates are in 0.1 µm: lengths are 10 000 µm + (10 000 + 20 000) µm = 40 mm.)

- [ ] **Step 2: Write failing metrics tests**

`tests/test_metrics.py`:

```python
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
    assert m["score"] == pytest.approx(metrics.score(10, 1, 0, 6, 100.0, 4))
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


def test_build_ground_truth_ratios(tmp_path):
    board = Board(id="pb", source="pcbench/pb/unrouted.dsn", origin="pcbench", referee="kicad", tiers=[],
                  nets=10, layers=2, kicad={"ground_truth": str(tmp_path / "gt.json")})
    (tmp_path / "gt.json").write_text(json.dumps({"wirelength_mm": 50.0, "vias": 2}))
    cell = _cell(tmp_path, {"status": "ok", "referee": "kicad", "unrouted": 0, "violations": 0,
                            "violations_by_type": {}, "vias": 4, "wirelength_mm": 100.0, "bends": None})
    m = metrics.build(cell, board)
    assert m["wirelength_ratio"] == 2.0 and m["via_ratio"] == 2.0
```

- [ ] **Step 3: Run to verify failure** — `uv run pytest tests/test_metrics.py -v` — Expected: `No module named 'bench.metrics'`.

- [ ] **Step 4: Write `bench/metrics.py`**

```python
"""Normalised per-cell metrics. Verdict-relevant numbers come from the referee only."""
from __future__ import annotations

import json
from pathlib import Path

from bench.corpus import Board

# Copied verbatim from freerouting DefaultSettings (Java). Versioned: bump SCORE_VERSION if changed.
SCORE_VERSION = 1
UNROUTED_PENALTY = 5_000_000.0
VIOLATION_PENALTY = 1_000_000.0
BEND_PENALTY = 10.0
VIA_COST = 50.0
TRACE_COST_PER_MM = 1.0


def score(nets: int, unrouted: int, violations: int, bends: int, length_mm: float, vias: int) -> float:
    maximum = nets * UNROUTED_PENALTY
    if maximum <= 0:
        return 0.0
    penalties = unrouted * UNROUTED_PENALTY + violations * VIOLATION_PENALTY + bends * BEND_PENALTY
    costs = length_mm * TRACE_COST_PER_MM + vias * VIA_COST
    return max(0.0, (maximum - penalties - costs) / maximum) * 1000.0


def _load(cell: Path, name: str) -> dict:
    p = cell / name
    return json.loads(p.read_text()) if p.exists() else {}


def _self_report(result: dict) -> dict:
    bs = result.get("board_statistics") or {}
    g = lambda sec, key: (bs.get(sec) or {}).get(key)
    return {
        "unrouted": g("connections", "incomplete_count"),
        "nets": g("connections", "maximum_count"),
        "violations": g("clearance_violations", "total_count"),
        "vias": g("vias", "total_count"),
        "wirelength_mm": g("traces", "total_length_mm"),
        "bends": g("bends", "total_count"),
        "score": result.get("normalized_score"),
        "final_state": result.get("final_state"),
        "version": result.get("app_version"),
        "sha": result.get("git_sha"),
    }


def build(cell: Path, board: Board) -> dict:
    result, ref, t = _load(cell, "result.json"), _load(cell, "referee.json"), _load(cell, "time.json")
    self_ = _self_report(result)
    phases = result.get("phases") or {}
    passes = (phases.get("autorouter") or {}).get("passes_completed")
    failed = ref.get("status") != "ok"
    nets = board.nets or self_["nets"] or 0

    if failed:
        unrouted, violations, vias, length = nets, 0, 0, 0.0
    else:
        unrouted, violations = int(ref["unrouted"]), int(ref["violations"])
        vias, length = int(ref["vias"]), float(ref["wirelength_mm"])
    bends = ref.get("bends") if ref.get("bends") is not None else (self_["bends"] or 0)

    m = {
        "schema_version": 1, "score_version": SCORE_VERSION,
        "board": board.id, "referee": ref.get("referee", board.referee),
        "clean_pass": (not failed) and unrouted == 0 and violations == 0,
        "unrouted": unrouted, "violations": violations,
        "violations_by_type": ref.get("violations_by_type", {}),
        "vias": vias, "wirelength_mm": length, "bends": bends,
        "score": score(nets, unrouted, violations, int(bends), length, vias),
        "passes": passes,
        "wall_s": t.get("wall_s"), "cpu_s": t.get("cpu_s"), "peak_rss_mb": t.get("peak_rss_mb"),
        "timed_out": t.get("timed_out", False), "exit_code": t.get("exit_code"),
        "failed": failed, "failure_reason": ref.get("reason", "") if failed else "",
        "disagreement": (not failed) and (self_["unrouted"] != unrouted or self_["violations"] != violations),
        "self": self_,
        "wirelength_ratio": None, "via_ratio": None,
    }
    gt_path = (board.kicad or {}).get("ground_truth")
    if gt_path and Path(gt_path).exists() and not failed:
        gt = json.loads(Path(gt_path).read_text())
        if gt.get("wirelength_mm"):
            m["wirelength_ratio"] = round(length / gt["wirelength_mm"], 4)
        if gt.get("vias"):
            m["via_ratio"] = round(vias / gt["vias"], 4)
    (cell / "metrics.json").write_text(json.dumps(m, indent=2) + "\n")
    return m
```

- [ ] **Step 5: Run metrics tests** — `uv run pytest tests/test_metrics.py -v` — Expected: 6 passed.

- [ ] **Step 6: Write failing java_drc tests**

`tests/test_java_drc.py`:

```python
import json
from pathlib import Path

import pytest

from bench.referee import java_drc
from bench.corpus import Board

DATA = Path(__file__).parent / "data"


def test_parse_ses_counts_vias_and_length_mm():
    vias, length = java_drc.parse_ses(DATA / "mini.ses")
    assert vias == 2
    assert length == pytest.approx(40.0)


def test_parse_drc_report():
    r = java_drc.parse_drc_report(json.loads((DATA / "drc_two.json").read_text()))
    assert r == {"unrouted": 1, "violations": 2,
                 "violations_by_type": {"clearance": 1, "hole_clearance": 1}}


def test_run_with_fake_java(tmp_path, monkeypatch):
    """A fake 'java' that writes the DRC report lets us test the plumbing without the jar."""
    fake = tmp_path / "fakejava.py"
    fake.write_text(
        "import sys,json,shutil\n"
        "args=sys.argv[1:]\n"
        "out=args[args.index('-drc')+1]\n"
        f"shutil.copyfile({str(DATA / 'drc_two.json')!r}, out)\n")
    cell = tmp_path / "seed-1"
    cell.mkdir()
    (cell / "in.dsn").write_text((DATA / "mini.dsn").read_text())
    (cell / "out.ses").write_text((DATA / "mini.ses").read_text())
    board = Board(id="mini", source="dsn/mini.dsn", origin="freerouting-fixtures", referee="java-drc",
                  tiers=[], nets=3, layers=2)
    import sys
    r = java_drc.run(board, cell, java_exec=[sys.executable, str(fake)])
    assert r["status"] == "ok" and r["referee"] == "java-drc"
    assert r["unrouted"] == 1 and r["violations"] == 2 and r["vias"] == 2
    assert r["wirelength_mm"] == pytest.approx(40.0)
    assert json.loads((cell / "referee.json").read_text()) == r


def test_run_missing_ses_is_referee_failed(tmp_path):
    cell = tmp_path / "seed-1"
    cell.mkdir()
    board = Board(id="mini", source="dsn/mini.dsn", origin="freerouting-fixtures", referee="java-drc",
                  tiers=[], nets=3, layers=2)
    r = java_drc.run(board, cell, java_exec=["true"])
    assert r["status"] == "referee_failed" and "out.ses" in r["reason"]
```

- [ ] **Step 7: Run to verify failure** — `uv run pytest tests/test_java_drc.py -v` — Expected: import error.

- [ ] **Step 8: Write `bench/referee/java_drc.py`**

```python
"""Referee for DSN fixtures: the Java jar in DRC-only mode + SES parsing for vias/length.

Invocation (Freerouting.java DRC path; GlobalSettings treats the .ses after -de as the session):
    java -jar fr.jar -de in.dsn out.ses -drc referee-drc.json --gui.enabled=false
"""
from __future__ import annotations

import json
import math
import re
import subprocess
from pathlib import Path

from bench.corpus import Board

REFEREE_TIMEOUT_S = 600

_RESOLUTION = re.compile(r"\(resolution\s+(\w+)\s+(\d+)\)")
_VIA = re.compile(r"\(via\s+")
_PATH = re.compile(r"\(path\s+\S+\s+[\d.]+((?:\s+-?[\d.]+)+)\s*\)")
_UNIT_TO_MM = {"um": 1e-3, "mm": 1.0, "mil": 0.0254, "inch": 25.4, "cm": 10.0}


def parse_ses(path: Path) -> tuple[int, float]:
    """(via count, wirelength mm) from a Specctra session file."""
    text = path.read_text(errors="replace")
    m = _RESOLUTION.search(text)
    unit, res = (m.group(1), int(m.group(2))) if m else ("um", 10)
    scale = _UNIT_TO_MM.get(unit, 1e-3) / res
    vias = len(_VIA.findall(text))
    length = 0.0
    for m in _PATH.finditer(text):
        nums = [float(x) for x in m.group(1).split()]
        pts = list(zip(nums[0::2], nums[1::2]))
        for (x0, y0), (x1, y1) in zip(pts, pts[1:]):
            length += math.hypot(x1 - x0, y1 - y0)
    return vias, round(length * scale, 4)


def parse_drc_report(report: dict) -> dict:
    violations = report.get("violations") or []
    by_type: dict[str, int] = {}
    for v in violations:
        by_type[v.get("type", "unknown")] = by_type.get(v.get("type", "unknown"), 0) + 1
    unconnected = report.get("unconnected_items") or report.get("unconnectedItems") or []
    return {"unrouted": len(unconnected), "violations": len(violations), "violations_by_type": by_type}


def _failed(cell: Path, reason: str) -> dict:
    r = {"status": "referee_failed", "referee": "java-drc", "reason": reason}
    (cell / "referee.json").write_text(json.dumps(r, indent=2) + "\n")
    return r


def run(board: Board, cell: Path, java_exec: list[str]) -> dict:
    ses, dsn = cell / "out.ses", cell / "in.dsn"
    if not ses.exists():
        return _failed(cell, "out.ses missing (candidate produced no output)")
    report = cell / "referee-drc.json"
    argv = [*java_exec, "-de", str(dsn), str(ses), "-drc", str(report), "--gui.enabled=false"]
    (cell / "referee-argv.json").write_text(json.dumps(argv, indent=2))
    with open(cell / "referee.log", "wb") as log:
        try:
            subprocess.run(argv, cwd=cell, stdout=log, stderr=subprocess.STDOUT, timeout=REFEREE_TIMEOUT_S)
        except subprocess.TimeoutExpired:
            return _failed(cell, f"java DRC timed out after {REFEREE_TIMEOUT_S}s")
    if not report.exists():
        return _failed(cell, "java DRC wrote no report (see referee.log)")
    try:
        drc = parse_drc_report(json.loads(report.read_text()))
        vias, length = parse_ses(ses)
    except (ValueError, KeyError) as e:
        return _failed(cell, f"could not parse referee output: {e}")
    r = {"status": "ok", "referee": "java-drc", "reason": "", **drc,
         "vias": vias, "wirelength_mm": length, "bends": None}
    (cell / "referee.json").write_text(json.dumps(r, indent=2) + "\n")
    return r
```

- [ ] **Step 9: Write `bench/referee/__init__.py`**

```python
"""Independent scoring of a routed cell. Dispatches on board.referee."""
from __future__ import annotations

import json
from pathlib import Path

from bench import metrics
from bench.corpus import Board
from bench.referee import java_drc


def score_cell(board: Board, cell: Path, java_exec: list[str], allow_kicad: bool = True) -> dict:
    """Write referee.json and metrics.json for one cell; return the metrics dict."""
    if board.referee == "kicad" and allow_kicad:
        from bench.referee import kicad  # imported lazily: only needed for PCBench boards
        r = kicad.run(board, cell)
        if r["status"] != "ok" and (board.kicad or {}).get("stripped") is not None:
            # spec §7.2 fallback: score with java-drc but say so
            r = java_drc.run(board, cell, java_exec)
            r["referee"] = "java-drc(fallback)"
            (cell / "referee.json").write_text(json.dumps(r, indent=2) + "\n")
    else:
        r = java_drc.run(board, cell, java_exec)
    return metrics.build(cell, board)
```

(`bench/referee/kicad.py` is created in Task 7; until then `allow_kicad=False` is passed by the CLI for non-kicad boards, and the lazy import keeps unit tests independent of it.)

- [ ] **Step 10: Run java_drc tests** — `uv run pytest tests/test_java_drc.py -v` — Expected: 4 passed.

- [ ] **Step 11: Wire the referee into `bench/cli.py`**

Add helper and update `run_cmd`, add `referee` command:

```python
from bench import referee
from bench.candidates import load_candidates, load_referee_java


def _java_exec() -> list[str]:
    custom = load_referee_java(paths.ROOT / "candidates.toml")
    return custom or [paths.java_exe(), "-Xmx4g", "-jar", str(paths.java_jar())]


# in run_cmd, replace `referee=None` with:
    hook = None
    if not no_referee:
        java_exec = _java_exec()
        hook = lambda board, cell: referee.score_cell(board, cell, java_exec)
    run_dir = runner.run(cfg, referee=hook, progress=click.echo)


@main.command("referee")
@click.option("--run", "run_id", required=True)
@click.option("--only-missing", is_flag=True, help="skip cells that already have metrics.json")
def referee_cmd(run_id: str, only_missing: bool) -> None:
    """(Re)score every cell of a run."""
    run_dir = runner.RESULTS / run_id
    meta = runner.load_meta(run_dir)
    boards = {b.id: b for b in corpus.load_manifest()}
    java_exec = _java_exec()
    for c in meta["cells"]:
        cell = runner.cell_dir(run_dir, c["candidate"], c["board"], c["seed"])
        if only_missing and (cell / "metrics.json").exists():
            continue
        m = referee.score_cell(boards[c["board"]], cell, java_exec)
        click.echo(f"{c['candidate']:16} {c['board']:32} seed {c['seed']}: "
                   f"unrouted={m['unrouted']} viol={m['violations']} score={m['score']:.1f}"
                   + (" DISAGREE" if m["disagreement"] else "") + (" FAILED" if m["failed"] else ""))
```

- [ ] **Step 12: Slow integration test with the real jar** (skipped when absent)

Append to `tests/test_java_drc.py`:

```python
@pytest.mark.slow
def test_real_jar_drc_on_fixture(tmp_path):
    from bench import paths
    try:
        jar = paths.java_jar()
    except paths.ToolMissing:
        pytest.skip("jar not built")
    fixtures = paths.JAVA_REPO / "fixtures"
    dsn = fixtures / "Issue143-rpi_splitter_mod.dsn"
    cell = tmp_path / "seed-1"
    cell.mkdir()
    (cell / "in.dsn").write_text(dsn.read_text())
    # route once to get a real SES
    import subprocess
    subprocess.run([paths.java_exe(), "-jar", str(jar), "-de", str(cell / "in.dsn"), "-do", str(cell / "out.ses"),
                    "-mp", "20", "--gui.enabled=false", "--api_server.enabled=false", "--mcp_server.enabled=false"],
                   check=True, capture_output=True, timeout=300)
    board = Board(id="issue143", source="x", origin="freerouting-fixtures", referee="java-drc", tiers=[], nets=0, layers=2)
    r = java_drc.run(board, cell, [paths.java_exe(), "-jar", str(jar)])
    assert r["status"] == "ok", r
    assert r["vias"] >= 0 and r["wirelength_mm"] > 0
```

Build the jar first: `(cd ../freerouting && ./gradlew executableJar -q)`. Then run: `uv run pytest tests/test_java_drc.py -v -m slow` — Expected: passes. If `-drc` mode does not write the report, read `referee.log`, fix the argv in `java_drc.run` to whatever the Java CLI needs, and re-run — that resolves spec §15's open item; note the outcome in `README.md`.

- [ ] **Step 13: Commit**

```bash
git add -A && git commit -m "feat: metrics record, score formula and java-drc referee"
```

---

### Task 5: Compare and Markdown report

**Files:**
- Create: `bench/compare.py`, `bench/report/__init__.py`, `bench/report/markdown.py`, `tests/test_compare.py`, `tests/test_markdown.py`
- Modify: `bench/cli.py`

**Interfaces:**
- Produces: `compare.collect(run_dirs, candidate_names) -> dict[cand][board] -> list[metrics]`; `compare.aggregate(cells) -> dict` (medians, clean_pass_rate, n); `compare.noise(cells) -> dict[metric] -> stddev`; `compare.verdict(base_agg, other_agg, noise) -> {"result": "win"|"loss"|"tie", "level": str}`; `compare.compare(runs, baseline, against, boards, tier=None) -> dict` (the compare JSON, spec §9); `markdown.render(cmp: dict) -> str`.
- Compare JSON: `{schema_version, created_at, baseline, against:[...], candidates:{name:{sha,version}}, runs:[ids], config:{threads,max_passes,timeout_s,seeds}, boards:{id:{tiers, referee, baseline:agg, against:{name:{agg, delta, verdict}}}}, tiers:{tier:{name:{wins,losses,ties,clean_pass_rate, median_score_delta, median_time_ratio}}}, overall:{name:{verdict, wins, losses, ties, hard_losses}}, disagreements:[...], failures:[...], warnings:[...]}`.

- [ ] **Step 1: Write failing compare tests**

`tests/test_compare.py`:

```python
import json
from pathlib import Path

import pytest

from bench import compare, runner
from bench.corpus import Board


def cell(clean=True, unrouted=0, violations=0, score=990.0, wall=10.0, rss=200.0, failed=False, disagree=False):
    return {"clean_pass": clean, "unrouted": unrouted, "violations": violations, "vias": 4,
            "wirelength_mm": 100.0, "score": score, "wall_s": wall, "cpu_s": wall, "peak_rss_mb": rss,
            "passes": 3, "failed": failed, "disagreement": disagree, "timed_out": False,
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


def _write_run(root: Path, run_id: str, cands: dict[str, dict[str, list[dict]]], threads=1):
    run_dir = root / run_id
    meta = {"run_id": run_id, "status": "complete", "args": {"threads": threads, "max_passes": 100, "timeout_s": 300, "seeds": 3},
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
```

- [ ] **Step 2: Run to verify failure** — `uv run pytest tests/test_compare.py -v` — Expected: import error.

- [ ] **Step 3: Write `bench/compare.py`**

```python
"""Aggregate cells, compute noise floors, apply the lexicographic verdict (spec §9)."""
from __future__ import annotations

import json
import statistics
from datetime import datetime, timezone
from pathlib import Path

from bench import runner
from bench.corpus import Board

NUMERIC = ["unrouted", "violations", "vias", "wirelength_mm", "score", "wall_s", "cpu_s",
           "peak_rss_mb", "passes", "wirelength_ratio", "via_ratio"]
# (metric, higher_is_better) in verdict order
LEVELS = [("clean_pass_rate", True), ("unrouted", False), ("violations", False),
          ("score", True), ("wall_s", False), ("peak_rss_mb", False)]
HARD_LEVELS = {"clean_pass_rate", "unrouted", "violations"}
NOISE_FACTOR = 2.0


class IncompatibleRuns(ValueError):
    pass


def _median(values: list) -> float | None:
    vals = [v for v in values if v is not None]
    return statistics.median(vals) if vals else None


def aggregate(cells: list[dict]) -> dict:
    agg = {"n": len(cells), "clean_pass_rate": sum(1 for c in cells if c.get("clean_pass")) / len(cells) if cells else 0.0,
           "failed": sum(1 for c in cells if c.get("failed"))}
    for k in NUMERIC:
        agg[k] = _median([c.get(k) for c in cells])
    return agg


def noise(cells: list[dict]) -> dict[str, float]:
    out: dict[str, float] = {"clean_pass_rate": 0.0}
    for k in NUMERIC:
        vals = [c.get(k) for c in cells if c.get(k) is not None]
        out[k] = statistics.stdev(vals) if len(vals) >= 3 else 0.0
    return out


def verdict(base: dict, other: dict, noise_floor: dict[str, float]) -> dict:
    last = LEVELS[-1][0]
    for metric, higher_better in LEVELS:
        b, o = base.get(metric), other.get(metric)
        if b is None or o is None:
            continue
        band = NOISE_FACTOR * noise_floor.get(metric, 0.0)
        if abs(o - b) <= band:
            continue
        better = o > b if higher_better else o < b
        return {"result": "win" if better else "loss", "level": metric}
    return {"result": "tie", "level": last}


def collect(run_dirs: list[Path], names: list[str]) -> tuple[dict, dict, list[str]]:
    """Return (cells[cand][board] -> list[metrics], cand_info[cand], warnings)."""
    cells: dict[str, dict[str, list[dict]]] = {n: {} for n in names}
    info: dict[str, dict] = {}
    warnings: list[str] = []
    for rd in run_dirs:
        meta = runner.load_meta(rd)
        for c in meta["candidates"]:
            if c["name"] in names:
                prev = info.get(c["name"])
                if prev and prev["sha"] != c["sha"]:
                    warnings.append(f"candidate {c['name']} has sha {prev['sha']} in one run and {c['sha']} in {meta['run_id']}")
                info[c["name"]] = {"sha": c["sha"], "version": c.get("version", "")}
        for e in meta["cells"]:
            if e["candidate"] not in names:
                continue
            mp = runner.cell_dir(rd, e["candidate"], e["board"], e["seed"]) / "metrics.json"
            if mp.exists():
                m = json.loads(mp.read_text())
                m["_run"], m["_seed"], m["_candidate"] = meta["run_id"], e["seed"], e["candidate"]
                cells[e["candidate"]].setdefault(e["board"], []).append(m)
    return cells, info, warnings


def _check_config(run_dirs: list[Path], allow_mixed: bool) -> tuple[dict, list[str]]:
    configs = [(rd.name, runner.load_meta(rd)["args"]) for rd in run_dirs]
    keys = ["threads", "max_passes", "timeout_s"]
    first = configs[0][1]
    warnings = []
    for name, a in configs[1:]:
        for k in keys:
            if a.get(k) != first.get(k):
                msg = f"run {name} has {k}={a.get(k)} but {configs[0][0]} has {k}={first.get(k)}"
                if not allow_mixed:
                    raise IncompatibleRuns(msg)
                warnings.append(msg)
    return {k: first.get(k) for k in keys + ["seeds"]}, warnings


def compare(run_dirs: list[Path], baseline: str, against: list[str], boards: list[Board],
            tier: str | None = None, allow_mixed: bool = False) -> dict:
    config, warnings = _check_config(run_dirs, allow_mixed)
    cells, info, w2 = collect(run_dirs, [baseline, *against])
    warnings += w2
    by_id = {b.id: b for b in boards}
    out_boards: dict[str, dict] = {}
    disagreements, failures = [], []
    for bid in sorted(cells.get(baseline, {})):
        board = by_id.get(bid)
        if board is None or (tier and tier not in board.tiers):
            continue
        base_cells = cells[baseline][bid]
        base_agg, nf = aggregate(base_cells), noise(base_cells)
        entry = {"tiers": board.tiers, "referee": board.referee, "noise": nf, "baseline": base_agg, "against": {}}
        for name in against:
            oc = cells[name].get(bid, [])
            if not oc:
                entry["against"][name] = None
                continue
            agg = aggregate(oc)
            delta = {k: (agg[k] - base_agg[k]) if agg.get(k) is not None and base_agg.get(k) is not None else None
                     for k in NUMERIC + ["clean_pass_rate"]}
            entry["against"][name] = {"agg": agg, "delta": delta, "verdict": verdict(base_agg, agg, nf)}
        out_boards[bid] = entry
        for m in base_cells + [m for n in against for m in cells[n].get(bid, [])]:
            if m.get("disagreement"):
                disagreements.append({"board": bid, "candidate": m["_candidate"], "seed": m["_seed"],
                                      "self_unrouted": m["self"].get("unrouted") if m.get("self") else None,
                                      "referee_unrouted": m["unrouted"], "self_violations": m["self"].get("violations") if m.get("self") else None,
                                      "referee_violations": m["violations"]})
            if m.get("failed"):
                failures.append({"board": bid, "candidate": m["_candidate"], "seed": m["_seed"], "reason": m.get("failure_reason", "")})

    tiers: dict[str, dict] = {}
    all_tiers = sorted({t for e in out_boards.values() for t in e["tiers"]} | {"all"})
    for t in all_tiers:
        ids = [b for b, e in out_boards.items() if t == "all" or t in e["tiers"]]
        tiers[t] = {}
        for name in against:
            rows = [out_boards[b]["against"][name] for b in ids if out_boards[b]["against"].get(name)]
            if not rows:
                continue
            res = [r["verdict"]["result"] for r in rows]
            ratios = [r["agg"]["wall_s"] / out_boards[b]["baseline"]["wall_s"] for b, r in zip(ids, rows)
                      if r["agg"].get("wall_s") and out_boards[b]["baseline"].get("wall_s")]
            tiers[t][name] = {"wins": res.count("win"), "losses": res.count("loss"), "ties": res.count("tie"),
                              "clean_pass_rate": sum(1 for r in rows if r["agg"]["clean_pass_rate"] == 1.0) / len(rows),
                              "median_score_delta": _median([r["delta"]["score"] for r in rows]),
                              "median_time_ratio": _median(ratios)}
    overall = {}
    for name in against:
        rows = [e["against"][name] for e in out_boards.values() if e["against"].get(name)]
        hard = sum(1 for r in rows if r["verdict"]["result"] == "loss" and r["verdict"]["level"] in HARD_LEVELS)
        wins = sum(1 for r in rows if r["verdict"]["result"] == "win")
        losses = sum(1 for r in rows if r["verdict"]["result"] == "loss")
        overall[name] = {"wins": wins, "losses": losses, "ties": len(rows) - wins - losses, "hard_losses": hard,
                         "verdict": "better" if hard == 0 and wins > losses else ("worse" if hard > 0 or losses > wins else "same")}
    return {"schema_version": 1, "created_at": datetime.now(timezone.utc).isoformat(timespec="seconds"),
            "baseline": baseline, "against": against, "candidates": info, "runs": [rd.name for rd in run_dirs],
            "config": config, "tier": tier, "boards": out_boards, "tiers": tiers, "overall": overall,
            "disagreements": disagreements, "failures": failures, "warnings": warnings}
```

- [ ] **Step 4: Run compare tests** — `uv run pytest tests/test_compare.py -v` — Expected: 5 passed.

- [ ] **Step 5: Write failing markdown test**

`tests/test_markdown.py`:

```python
from bench.report import markdown


def test_render_contains_verdict_tables():
    cmp = {"created_at": "t", "baseline": "java", "against": ["rs"], "runs": ["r1"],
           "candidates": {"java": {"sha": "aaa", "version": "2.3"}, "rs": {"sha": "bbb", "version": "0.1"}},
           "config": {"threads": 1, "max_passes": 100, "timeout_s": 300, "seeds": 3}, "tier": None,
           "boards": {"a": {"tiers": ["canary"], "referee": "java-drc", "noise": {"score": 5.0},
                            "baseline": {"n": 3, "clean_pass_rate": 1.0, "unrouted": 0, "violations": 0, "score": 900.0, "wall_s": 10.0, "peak_rss_mb": 200.0, "vias": 4, "wirelength_mm": 100.0, "failed": 0},
                            "against": {"rs": {"agg": {"n": 3, "clean_pass_rate": 1.0, "unrouted": 0, "violations": 0, "score": 990.0, "wall_s": 5.0, "peak_rss_mb": 150.0, "vias": 3, "wirelength_mm": 90.0, "failed": 0},
                                               "delta": {"score": 90.0, "wall_s": -5.0, "unrouted": 0, "violations": 0, "vias": -1, "wirelength_mm": -10.0, "peak_rss_mb": -50.0, "clean_pass_rate": 0.0},
                                               "verdict": {"result": "win", "level": "score"}}}}},
           "tiers": {"all": {"rs": {"wins": 1, "losses": 0, "ties": 0, "clean_pass_rate": 1.0, "median_score_delta": 90.0, "median_time_ratio": 0.5}}},
           "overall": {"rs": {"verdict": "better", "wins": 1, "losses": 0, "ties": 0, "hard_losses": 0}},
           "disagreements": [], "failures": [], "warnings": ["w1"]}
    md = markdown.render(cmp)
    assert "# Comparison: java vs rs" in md
    assert "| all |" in md and "| a |" in md
    assert "**better**" in md and "win (score)" in md
    assert "w1" in md
```

- [ ] **Step 6: Write `bench/report/__init__.py` (empty) and `bench/report/markdown.py`**

```python
"""Render a compare JSON as Markdown."""
from __future__ import annotations


def _f(v, nd=1):
    if v is None:
        return "—"
    return f"{v:.{nd}f}" if isinstance(v, float) else str(v)


def _delta(v, nd=1):
    if v is None:
        return "—"
    sign = "+" if v > 0 else ""
    return f"{sign}{v:.{nd}f}" if isinstance(v, float) else f"{sign}{v}"


def render(cmp: dict) -> str:
    L: list[str] = []
    names = cmp["against"]
    L.append(f"# Comparison: {cmp['baseline']} vs {', '.join(names)}")
    L.append("")
    L.append(f"Created {cmp['created_at']} from runs {', '.join(cmp['runs'])}. "
             f"Config: threads={cmp['config'].get('threads')} max_passes={cmp['config'].get('max_passes')} "
             f"timeout={cmp['config'].get('timeout_s')}s seeds={cmp['config'].get('seeds')}"
             + (f" tier={cmp['tier']}" if cmp.get("tier") else ""))
    L.append("")
    L.append("| candidate | version | sha |")
    L.append("|---|---|---|")
    for n, i in cmp["candidates"].items():
        L.append(f"| {n} | {i.get('version') or '—'} | `{i.get('sha')}` |")
    L.append("")
    L.append("## Overall")
    L.append("")
    for n in names:
        o = cmp["overall"][n]
        L.append(f"- **{n}** vs {cmp['baseline']}: **{o['verdict']}** — {o['wins']} wins, {o['losses']} losses "
                 f"({o['hard_losses']} on hard metrics), {o['ties']} ties")
    L.append("")
    L.append("## Per tier")
    L.append("")
    L.append("| tier | candidate | wins | losses | ties | clean-pass rate | median Δscore | median time ratio |")
    L.append("|---|---|---|---|---|---|---|---|")
    for t, row in cmp["tiers"].items():
        for n, s in row.items():
            L.append(f"| {t} | {n} | {s['wins']} | {s['losses']} | {s['ties']} | {_f(s['clean_pass_rate'], 2)} | "
                     f"{_delta(s['median_score_delta'])} | {_f(s['median_time_ratio'], 2)} |")
    L.append("")
    L.append("## Per board")
    L.append("")
    L.append("| board | referee | candidate | clean | unrouted | viol | score | Δscore | noise | wall s | Δwall | vias | length mm | verdict |")
    L.append("|---|---|---|---|---|---|---|---|---|---|---|---|---|---|")
    for bid, e in cmp["boards"].items():
        b = e["baseline"]
        L.append(f"| {bid} | {e['referee']} | {cmp['baseline']} | {_f(b['clean_pass_rate'], 2)} | {_f(b['unrouted'])} | {_f(b['violations'])} | "
                 f"{_f(b['score'])} | | {_f(e['noise'].get('score'))} | {_f(b['wall_s'])} | | {_f(b['vias'])} | {_f(b['wirelength_mm'])} | baseline |")
        for n in names:
            a = e["against"].get(n)
            if not a:
                L.append(f"| {bid} | {e['referee']} | {n} | — | — | — | — | — | — | — | — | — | — | missing |")
                continue
            g, d, v = a["agg"], a["delta"], a["verdict"]
            L.append(f"| {bid} | {e['referee']} | {n} | {_f(g['clean_pass_rate'], 2)} | {_f(g['unrouted'])} | {_f(g['violations'])} | "
                     f"{_f(g['score'])} | {_delta(d['score'])} | | {_f(g['wall_s'])} | {_delta(d['wall_s'])} | {_f(g['vias'])} | "
                     f"{_f(g['wirelength_mm'])} | {v['result']} ({v['level']}) |")
    if cmp["disagreements"]:
        L += ["", "## Self-report disagreements", "", "| board | candidate | seed | self unrouted | referee unrouted | self viol | referee viol |", "|---|---|---|---|---|---|---|"]
        for d in cmp["disagreements"]:
            L.append(f"| {d['board']} | {d['candidate']} | {d['seed']} | {d['self_unrouted']} | {d['referee_unrouted']} | {d['self_violations']} | {d['referee_violations']} |")
    if cmp["failures"]:
        L += ["", "## Failures", ""]
        L += [f"- {f['board']} / {f['candidate']} / seed {f['seed']}: {f['reason']}" for f in cmp["failures"]]
    if cmp["warnings"]:
        L += ["", "## Warnings", ""]
        L += [f"- {w}" for w in cmp["warnings"]]
    return "\n".join(L) + "\n"
```

- [ ] **Step 7: Run** — `uv run pytest tests/test_markdown.py -v` — Expected: 1 passed.

- [ ] **Step 8: Add `compare` and `report` commands to `bench/cli.py`**

```python
import json

from bench import compare as compare_mod
from bench.report import markdown as md_report


def _latest_run_for(name: str) -> Path:
    runs = sorted(runner.RESULTS.glob("*/meta.json"), key=lambda p: p.stat().st_mtime, reverse=True)
    for m in runs:
        if any(c["name"] == name for c in json.loads(m.read_text())["candidates"]):
            return m.parent
    raise click.ClickException(f"no run contains candidate {name}")


@main.command("compare")
@click.option("--baseline", required=True)
@click.option("--against", required=True, help="comma-separated candidate names")
@click.option("--runs", default=None, help="comma-separated run ids (default: latest run per candidate)")
@click.option("--tier", default=None)
@click.option("--out", "out_name", default=None)
@click.option("--allow-mixed", is_flag=True)
def compare_cmd(baseline, against, runs, tier, out_name, allow_mixed):
    """Compare candidates and write reports/<id>.{json,md}."""
    names = against.split(",")
    if runs:
        run_dirs = [runner.RESULTS / r for r in runs.split(",")]
    else:
        run_dirs = []
        for n in [baseline, *names]:
            rd = _latest_run_for(n)
            if rd not in run_dirs:
                run_dirs.append(rd)
    cmp = compare_mod.compare(run_dirs, baseline, names, corpus.load_manifest(), tier=tier, allow_mixed=allow_mixed)
    out_name = out_name or f"{datetime.now().strftime('%Y%m%d-%H%M%S')}-{baseline}-vs-{'-'.join(names)}"
    paths.REPORTS.mkdir(exist_ok=True)
    (paths.REPORTS / f"{out_name}.json").write_text(json.dumps(cmp, indent=2) + "\n")
    (paths.REPORTS / f"{out_name}.md").write_text(md_report.render(cmp))
    for n in names:
        click.echo(f"{n} vs {baseline}: {cmp['overall'][n]['verdict']} "
                   f"({cmp['overall'][n]['wins']}W/{cmp['overall'][n]['losses']}L/{cmp['overall'][n]['ties']}T)")
    click.echo(f"reports written to {paths.REPORTS / out_name}.{{json,md}}")


@main.command("report")
@click.option("--compare", "compare_id", required=True)
def report_cmd(compare_id: str) -> None:
    """Regenerate Markdown (and HTML once available) from a compare JSON."""
    cmp = json.loads((paths.REPORTS / f"{compare_id}.json").read_text())
    (paths.REPORTS / f"{compare_id}.md").write_text(md_report.render(cmp))
    click.echo(f"wrote {paths.REPORTS / compare_id}.md")
```

- [ ] **Step 9: Commit**

```bash
git add -A && git commit -m "feat: compare with noise-aware lexicographic verdicts and Markdown report"
```

---

### Task 6: End-to-end with the Java jar

**Files:**
- Create: `tests/test_e2e.py`
- Modify: `README.md` (usage walkthrough)

**Interfaces:** consumes everything above via the CLI.

- [ ] **Step 1: Build the jar and init the corpus**

```bash
(cd ../freerouting && ./gradlew executableJar -q)
uv run bench corpus init
```

- [ ] **Step 2: Write the e2e test**

`tests/test_e2e.py`:

```python
import json
from pathlib import Path

import pytest
from click.testing import CliRunner

from bench import cli, corpus, paths, runner


@pytest.mark.slow
def test_java_vs_itself_is_all_ties(tmp_path, monkeypatch):
    try:
        paths.java_jar()
    except paths.ToolMissing:
        pytest.skip("jar not built")
    if not corpus.manifest_path().exists():
        pytest.skip("run `bench corpus init` first")
    monkeypatch.setattr(runner, "RESULTS", tmp_path / "results")
    monkeypatch.setattr(paths, "REPORTS", tmp_path / "reports")
    # a second candidate name pointing at the same jar
    toml = paths.ROOT / "candidates.toml"
    extra = toml.read_text() + '\n[candidates.java-twin]\nkind="java"\nexec=["java","-Xmx4g","-jar","../freerouting/build/libs/freerouting-current-executable.jar"]\nsha_from="../freerouting"\n'
    twin = tmp_path / "candidates.toml"
    twin.write_text(extra.replace('"../freerouting/', f'"{paths.JAVA_REPO}/'))
    monkeypatch.setattr(paths, "ROOT", tmp_path)
    r = CliRunner()
    res = r.invoke(cli.main, ["run", "--candidates", "java-current,java-twin", "--boards", "issue143-rpi_splitter_mod",
                              "--seeds", "3", "--max-passes", "20", "--timeout", "120", "--run-id", "e2e"])
    assert res.exit_code == 0, res.output
    meta = runner.load_meta(tmp_path / "results" / "e2e")
    assert meta["status"] == "complete" and all(c["status"] == "ok" for c in meta["cells"])
    res = r.invoke(cli.main, ["compare", "--baseline", "java-current", "--against", "java-twin", "--runs", "e2e", "--out", "e2e"])
    assert res.exit_code == 0, res.output
    cmp = json.loads((tmp_path / "reports" / "e2e.json").read_text())
    v = cmp["boards"]["issue143-rpi_splitter_mod"]["against"]["java-twin"]["verdict"]
    assert v["result"] == "tie", v
    assert cmp["disagreements"] == [], cmp["disagreements"]
```

- [ ] **Step 3: Run it** — `uv run pytest tests/test_e2e.py -v -m slow` — Expected: PASS in under ~2 min. If the verdict is not a tie, inspect `wall_s` noise: with 3 seeds the noise floor for wall time should absorb JVM jitter; if it does not, the board is too fast — this is real information, record it in README under "Noise".

If `disagreements` is non-empty, the Java referee and the Java self-report differ on the *same* SES — investigate `referee.log` before continuing; that is a referee bug, not a router bug.

- [ ] **Step 4: First real canary run and README walkthrough**

```bash
uv run bench run --candidates java-current --tier canary --seeds 3 --max-passes 100 --timeout 120 --run-id baseline-canary
uv run bench compare --baseline java-current --against java-current --runs baseline-canary --out sanity
```

Add to `README.md` a "Quick start" with these exact commands plus the fork variant (`--candidates java-current,rs-main`), and a "Reading the report" paragraph (verdict levels, noise band, disagreements).

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "test: end-to-end Java-vs-Java run; README quick start"
```

---

### Task 7: KiCad referee (spike, then implementation or documented fallback)

**Files:**
- Create: `vendor/kicad/strip_kicad_routing.py`, `vendor/kicad/export_specctra_dsn.py`, `vendor/kicad/import_specctra_ses.py` (copied from `../freerouting/scripts/pcbench/`), `vendor/kicad/board_stats.py`, `bench/referee/kicad.py`, `tests/test_kicad_referee.py`
- Modify: `README.md`

**Interfaces:**
- Produces: `kicad.run(board, cell) -> dict` (same `referee.json` shape as java_drc, `referee: "kicad"`); `kicad.parse_kicad_drc(report: dict) -> dict`; `vendor/kicad/board_stats.py <pcb> ` prints `{"vias": n, "wirelength_mm": x}` JSON.

- [ ] **Step 1: Vendor the scripts**

```bash
mkdir -p vendor/kicad
cp ../freerouting/scripts/pcbench/{strip_kicad_routing.py,export_specctra_dsn.py,import_specctra_ses.py} vendor/kicad/
```

Add `vendor/kicad/board_stats.py`:

```python
"""Print {"vias": n, "wirelength_mm": x} for a .kicad_pcb. Run with KiCad's bundled python."""
import json
import sys

import pcbnew  # type: ignore

board = pcbnew.LoadBoard(sys.argv[1])
vias, length_nm = 0, 0
for t in board.GetTracks():
    if t.GetClass() == "PCB_VIA":
        vias += 1
    else:
        length_nm += t.GetLength()
print(json.dumps({"vias": vias, "wirelength_mm": round(length_nm / 1e6, 4)}))
```

- [ ] **Step 2: The spike (≤ 2 h, time-boxed)**

Goal: does headless SES import work on KiCad 10.0.3? Use any small KiCad board (e.g. create one in KiCad with two footprints and one net, save as `tests/data/spike/spike.kicad_pcb`), then:

```bash
KP=/Applications/KiCad/KiCad.app/Contents/Frameworks/Python.framework/Versions/Current/bin/python3
$KP vendor/kicad/strip_kicad_routing.py tests/data/spike/spike.kicad_pcb /tmp/spike-stripped.kicad_pcb
$KP vendor/kicad/export_specctra_dsn.py /tmp/spike-stripped.kicad_pcb /tmp/spike.dsn
java -jar ../freerouting/build/libs/freerouting-current-executable.jar -de /tmp/spike.dsn -do /tmp/spike.ses -mp 20 --gui.enabled=false
$KP vendor/kicad/import_specctra_ses.py /tmp/spike-stripped.kicad_pcb /tmp/spike.ses /tmp/spike-routed.kicad_pcb; echo "exit=$?"
/Applications/KiCad/KiCad.app/Contents/MacOS/kicad-cli pcb drc --format json --all-track-errors --units mm -o /tmp/spike-drc.json /tmp/spike-routed.kicad_pcb
```

Things to try if import returns false with no tracks: (a) pass absolute paths; (b) the SES `(base_design ...)` name must match the board file name — copy the stripped board to `<dsn-stem>.kicad_pcb` before import; (c) call `pcbnew.ImportSpecctraSES(board, path)` vs the one-arg form; (d) `wx.App` must exist before `LoadBoard`. Record every attempt and outcome in `README.md` under "KiCad referee status".

- [ ] **Step 3: Write failing tests for the pure parts**

`tests/test_kicad_referee.py`:

```python
import json
from pathlib import Path

import pytest

from bench.referee import kicad

KICAD_DRC = {"$schema": "https://schemas.kicad.org/drc.v1.json", "coordinate_units": "mm",
             "unconnected_items": [{"type": "unconnected_items", "description": "Missing connection"}],
             "violations": [{"type": "clearance", "severity": "error"}, {"type": "clearance", "severity": "error"},
                            {"type": "silk_over_copper", "severity": "warning"}],
             "schematic_parity": []}


def test_parse_kicad_drc_counts_errors_only():
    r = kicad.parse_kicad_drc(KICAD_DRC)
    assert r == {"unrouted": 1, "violations": 2, "violations_by_type": {"clearance": 2}, "warnings": 1}


def test_run_without_kicad_source_is_referee_failed(tmp_path):
    from bench.corpus import Board
    b = Board(id="x", source="pcbench/x/unrouted.dsn", origin="pcbench", referee="kicad", tiers=[], nets=1, layers=2, kicad=None)
    cell = tmp_path / "seed-1"
    cell.mkdir()
    (cell / "out.ses").write_text("(session x)")
    r = kicad.run(b, cell)
    assert r["status"] == "referee_failed" and "stripped" in r["reason"]
```

- [ ] **Step 4: Run to verify failure** — `uv run pytest tests/test_kicad_referee.py -v` — Expected: import error.

- [ ] **Step 5: Write `bench/referee/kicad.py`**

```python
"""Referee for PCBench boards: import the SES into the stripped KiCad board, run kicad-cli DRC."""
from __future__ import annotations

import json
import shutil
import subprocess
from pathlib import Path

from bench.corpus import Board
from bench.paths import VENDOR_KICAD, ToolMissing, kicad_cli, kicad_python

REFEREE_TIMEOUT_S = 600


def parse_kicad_drc(report: dict) -> dict:
    errors = [v for v in report.get("violations", []) if v.get("severity", "error") == "error"]
    by_type: dict[str, int] = {}
    for v in errors:
        by_type[v.get("type", "unknown")] = by_type.get(v.get("type", "unknown"), 0) + 1
    warnings = sum(1 for v in report.get("violations", []) if v.get("severity") == "warning")
    return {"unrouted": len(report.get("unconnected_items", [])), "violations": len(errors),
            "violations_by_type": by_type, "warnings": warnings}


def _failed(cell: Path, reason: str) -> dict:
    r = {"status": "referee_failed", "referee": "kicad", "reason": reason}
    (cell / "referee.json").write_text(json.dumps(r, indent=2) + "\n")
    return r


def _run(argv: list[str], cell: Path, log: Path) -> int:
    with open(log, "ab") as f:
        f.write((" ".join(argv) + "\n").encode())
        try:
            return subprocess.run(argv, cwd=cell, stdout=f, stderr=subprocess.STDOUT, timeout=REFEREE_TIMEOUT_S).returncode
        except subprocess.TimeoutExpired:
            return -9


def run(board: Board, cell: Path) -> dict:
    stripped = (board.kicad or {}).get("stripped")
    if not stripped:
        return _failed(cell, "board has no stripped .kicad_pcb (board.kicad.stripped)")
    ses = cell / "out.ses"
    if not ses.exists():
        return _failed(cell, "out.ses missing (candidate produced no output)")
    try:
        py, cli = kicad_python(), kicad_cli()
    except ToolMissing as e:
        return _failed(cell, str(e))
    log = cell / "referee.log"
    # SES base_design must match the board name: copy stripped board under the DSN stem.
    base = cell / (Path(board.source).stem + ".kicad_pcb")
    shutil.copyfile(stripped, base)
    routed = cell / "routed.kicad_pcb"
    if _run([str(py), str(VENDOR_KICAD / "import_specctra_ses.py"), str(base), str(ses), str(routed)], cell, log) != 0:
        return _failed(cell, "SES import into KiCad failed (see referee.log)")
    report = cell / "referee-drc.json"
    if _run([str(cli), "pcb", "drc", "--format", "json", "--all-track-errors", "--units", "mm",
             "-o", str(report), str(routed)], cell, log) != 0 or not report.exists():
        return _failed(cell, "kicad-cli drc failed (see referee.log)")
    stats_out = cell / "referee-stats.json"
    with open(stats_out, "w") as f:
        rc = subprocess.run([str(py), str(VENDOR_KICAD / "board_stats.py"), str(routed)], stdout=f,
                            stderr=open(log, "ab"), timeout=REFEREE_TIMEOUT_S).returncode
    if rc != 0:
        return _failed(cell, "board_stats.py failed (see referee.log)")
    drc = parse_kicad_drc(json.loads(report.read_text()))
    stats = json.loads(stats_out.read_text())
    r = {"status": "ok", "referee": "kicad", "reason": "", "unrouted": drc["unrouted"], "violations": drc["violations"],
         "violations_by_type": drc["violations_by_type"], "kicad_warnings": drc["warnings"],
         "vias": stats["vias"], "wirelength_mm": stats["wirelength_mm"], "bends": None}
    (cell / "referee.json").write_text(json.dumps(r, indent=2) + "\n")
    return r
```

- [ ] **Step 6: Run** — `uv run pytest tests/test_kicad_referee.py -v` — Expected: 2 passed.

- [ ] **Step 7: Slow test on the spike board** (append to the test file)

```python
@pytest.mark.slow
def test_kicad_referee_on_spike_board(tmp_path):
    from bench import paths
    from bench.corpus import Board
    spike = Path(__file__).parent / "data" / "spike"
    if not (spike / "stripped.kicad_pcb").exists() or not (spike / "routed.ses").exists():
        pytest.skip("spike artifacts missing")
    try:
        paths.kicad_cli(); paths.kicad_python()
    except paths.ToolMissing:
        pytest.skip("kicad missing")
    b = Board(id="spike", source="pcbench/spike/spike.dsn", origin="pcbench", referee="kicad", tiers=[], nets=1, layers=2,
              kicad={"stripped": str(spike / "stripped.kicad_pcb")})
    cell = tmp_path / "seed-1"; cell.mkdir()
    (cell / "out.ses").write_text((spike / "routed.ses").read_text())
    r = kicad.run(b, cell)
    assert r["status"] == "ok", r
    assert r["vias"] >= 0 and r["wirelength_mm"] > 0
```

Commit the spike artifacts (`stripped.kicad_pcb`, `routed.ses`, `spike.dsn`) into `tests/data/spike/` if the spike succeeded. Run: `uv run pytest tests/test_kicad_referee.py -v -m slow`.

- [ ] **Step 8: Document the outcome**

In `README.md` add "KiCad referee status": either "working on KiCad 10.0.3 (verified YYYY-MM-DD)" with the invocation, or "SES import fails headless on 10.0.3 — attempts: …; PCBench boards fall back to `java-drc(fallback)`; tracked TODO". The fallback path already exists in `bench/referee/__init__.py`.

- [ ] **Step 9: Commit**

```bash
git add -A && git commit -m "feat: KiCad DRC referee with documented fallback"
```

---

### Task 8: PCBench corpus

**Files:**
- Create: `bench/corpus_pcbench.py`, `tests/test_corpus_pcbench.py`
- Modify: `bench/cli.py`, `bench/corpus.py` (add `assign_d3_tier`)

**Interfaces:**
- Produces: `corpus.assign_d3_tier(nets) -> str | None` (`d3-a` 2–13, `d3-b` 14–42, `d3-c` 43–451, else None; the paper's ranges overlap, the smallest containing range wins); `corpus_pcbench.import_boards(pcbench_root, ids=None, max_boards=None) -> list[Board]`; `corpus_pcbench.list_board_ids(pcbench_root) -> list[str]`.
- Per board on disk: `corpus/pcbench/<id>/{raw.kicad_pcb, stripped.kicad_pcb, unrouted.dsn, ground_truth.json}`; `ground_truth.json = {"wirelength_mm", "vias", "drv_count", "nets", "layers", "kicad_version"}`.

- [ ] **Step 1: Failing tests**

`tests/test_corpus_pcbench.py`:

```python
import json
from pathlib import Path

from bench import corpus, corpus_pcbench


def test_assign_d3_tier():
    assert corpus.assign_d3_tier(2) == "d3-a"
    assert corpus.assign_d3_tier(13) == "d3-a"
    assert corpus.assign_d3_tier(14) == "d3-b"
    assert corpus.assign_d3_tier(42) == "d3-b"
    assert corpus.assign_d3_tier(43) == "d3-c"
    assert corpus.assign_d3_tier(451) == "d3-c"
    assert corpus.assign_d3_tier(1) is None and corpus.assign_d3_tier(1000) is None


def test_list_board_ids(tmp_path):
    for n in ["b2", "a1", "bad"]:
        d = tmp_path / "PCBs" / n
        d.mkdir(parents=True)
        if n != "bad":
            (d / "processed.kicad_pcb").write_text("")
            (d / "raw.kicad_pcb").write_text("")
            (d / "metadata.json").write_text("{}")
    assert corpus_pcbench.list_board_ids(tmp_path) == ["a1", "b2"]


def test_import_records_exclusion_when_tools_fail(tmp_path, monkeypatch):
    d = tmp_path / "PCBs" / "x"
    d.mkdir(parents=True)
    for f in ["processed.kicad_pcb", "raw.kicad_pcb"]:
        (d / f).write_text("(kicad_pcb)")
    (d / "metadata.json").write_text(json.dumps({"layers": 2, "kicad_version": "7"}))
    monkeypatch.setattr(corpus, "CORPUS", tmp_path / "corpus")
    monkeypatch.setattr(corpus_pcbench, "_kicad_step", lambda *a, **k: (1, "simulated failure"))
    boards = corpus_pcbench.import_boards(tmp_path, ids=["x"])
    assert boards[0].status == "excluded" and "simulated failure" in boards[0].reason
    assert corpus.load_manifest()[0].id == "pcbench-x"
```

- [ ] **Step 2: Run to verify failure** — `uv run pytest tests/test_corpus_pcbench.py -v`.

- [ ] **Step 3: Add `assign_d3_tier` to `bench/corpus.py`**

```python
D3_RANGES = [("d3-a", 2, 13), ("d3-b", 14, 42), ("d3-c", 43, 451)]


def assign_d3_tier(nets: int) -> str | None:
    for name, lo, hi in D3_RANGES:
        if lo <= nets <= hi:
            return name
    return None
```

- [ ] **Step 4: Write `bench/corpus_pcbench.py`**

```python
"""Import PCBench boards: strip routing, export unrouted DSN, record ground truth."""
from __future__ import annotations

import json
import shutil
import subprocess
from pathlib import Path

from bench import corpus
from bench.corpus import Board
from bench.paths import VENDOR_KICAD, ToolMissing, kicad_cli, kicad_python

STEP_TIMEOUT_S = 600


def list_board_ids(root: Path) -> list[str]:
    out = []
    for d in sorted((root / "PCBs").iterdir()):
        if all((d / f).exists() for f in ["processed.kicad_pcb", "raw.kicad_pcb", "metadata.json"]):
            out.append(d.name)
    return out


def _kicad_step(argv: list[str], log: Path) -> tuple[int, str]:
    with open(log, "ab") as f:
        f.write((" ".join(argv) + "\n").encode())
        try:
            p = subprocess.run(argv, stdout=f, stderr=subprocess.STDOUT, timeout=STEP_TIMEOUT_S)
            return p.returncode, "" if p.returncode == 0 else f"{argv[1] if len(argv) > 1 else argv[0]} exited {p.returncode}"
        except subprocess.TimeoutExpired:
            return -9, f"{argv[0]} timed out"


def import_one(root: Path, bid: str) -> Board:
    src = root / "PCBs" / bid
    dst = corpus.CORPUS / "pcbench" / bid
    dst.mkdir(parents=True, exist_ok=True)
    log = dst / "import.log"
    shutil.copyfile(src / "raw.kicad_pcb", dst / "raw.kicad_pcb")
    meta = json.loads((src / "metadata.json").read_text())
    board = Board(id=f"pcbench-{bid}", source=f"pcbench/{bid}/unrouted.dsn", origin="pcbench", referee="kicad",
                  tiers=["pcbench"], layers=int(meta.get("layers", 0) or 0),
                  kicad={"raw": str(dst / "raw.kicad_pcb"), "stripped": str(dst / "stripped.kicad_pcb"),
                         "ground_truth": str(dst / "ground_truth.json")})
    try:
        py, cli = str(kicad_python()), str(kicad_cli())
    except ToolMissing as e:
        board.status, board.reason = "excluded", str(e)
        return board
    steps = [
        [py, str(VENDOR_KICAD / "strip_kicad_routing.py"), str(src / "processed.kicad_pcb"), str(dst / "stripped.kicad_pcb")],
        [py, str(VENDOR_KICAD / "export_specctra_dsn.py"), str(dst / "stripped.kicad_pcb"), str(dst / "unrouted.dsn")],
        [cli, "pcb", "drc", "--format", "json", "--units", "mm", "-o", str(dst / "raw-drc.json"), str(dst / "raw.kicad_pcb")],
    ]
    for argv in steps:
        rc, reason = _kicad_step(argv, log)
        if rc != 0:
            board.status, board.reason = "excluded", reason
            return board
    stats_rc = subprocess.run([py, str(VENDOR_KICAD / "board_stats.py"), str(dst / "raw.kicad_pcb")],
                              capture_output=True, text=True, timeout=STEP_TIMEOUT_S)
    if stats_rc.returncode != 0:
        board.status, board.reason = "excluded", "board_stats.py failed on raw board"
        return board
    stats = json.loads(stats_rc.stdout)
    drc = json.loads((dst / "raw-drc.json").read_text())
    drv = sum(1 for v in drc.get("violations", []) if v.get("severity", "error") == "error")
    nets, layers = corpus.dsn_info(dst / "unrouted.dsn")
    board.nets, board.layers = nets, layers or board.layers
    gt = {"wirelength_mm": stats["wirelength_mm"], "vias": stats["vias"], "drv_count": drv,
          "nets": nets, "layers": board.layers, "kicad_version": meta.get("kicad_version")}
    (dst / "ground_truth.json").write_text(json.dumps(gt, indent=2) + "\n")
    if drv > 0:
        board.status, board.reason = "excluded", f"reference board has {drv} DRC errors"
    tier = corpus.assign_d3_tier(nets)
    if tier:
        board.tiers.append(tier)
    return board


def import_boards(root: Path, ids: list[str] | None = None, max_boards: int | None = None) -> list[Board]:
    ids = ids or list_board_ids(root)
    if max_boards:
        ids = ids[:max_boards]
    existing = {b.id: b for b in corpus.load_manifest()}
    imported = []
    for bid in ids:
        b = import_one(root, bid)
        existing[b.id] = b
        imported.append(b)
        corpus.save_manifest(list(existing.values()))
    return imported
```

- [ ] **Step 5: Run tests** — `uv run pytest tests/test_corpus_pcbench.py -v` — Expected: 3 passed.

- [ ] **Step 6: CLI command**

```python
@corpus_cmd.command("pcbench")
@click.option("--clone", "root", type=click.Path(path_type=Path), required=True,
              help="PCBench checkout (cloned if missing)")
@click.option("--boards", "max_boards", type=int, default=None)
@click.option("--ids", default=None, help="comma-separated PCBench board dir names")
def corpus_pcbench_cmd(root: Path, max_boards, ids):
    """Import PCBench boards (strip → DSN → ground truth)."""
    from bench import corpus_pcbench
    if not (root / "PCBs").exists():
        click.echo(f"cloning PCBench into {root} …")
        subprocess.run(["git", "clone", "--depth", "1", "https://github.com/PCBench/PCBench", str(root)], check=True)
    boards = corpus_pcbench.import_boards(root, ids=ids.split(",") if ids else None, max_boards=max_boards)
    ok = sum(1 for b in boards if b.status == "ok")
    click.echo(f"imported {ok}/{len(boards)} boards")
    for b in boards:
        if b.status != "ok":
            click.echo(f"  excluded {b.id}: {b.reason}")
```

(add `import subprocess` at the top of `cli.py`). Try it: `uv run bench corpus pcbench --clone ../PCBench --boards 3`.

- [ ] **Step 7: Commit**

```bash
git add -A && git commit -m "feat: PCBench corpus import with ground truth and D3 tiers"
```

---

### Task 9: HTML dashboard with history

**Files:**
- Create: `bench/report/html.py`, `bench/report/templates/dashboard.html.j2`, `tests/test_html.py`
- Modify: `bench/cli.py` (`compare` and `report` also write `.html`), `pyproject.toml` (include templates in wheel: `[tool.hatch.build.targets.wheel] packages = ["bench"]` already includes package data files)

**Interfaces:**
- Produces: `html.render(cmp: dict, history: list[dict]) -> str`; `html.load_history(reports_dir) -> list[dict]` (every `*.json` compare in `reports/`, sorted by `created_at`); `html.bar_svg(values: list[tuple[str, float, float]], width=600) -> str` (label, baseline, candidate → grouped bars); `html.line_svg(series: dict[str, list[tuple[str, float]]]) -> str`.

- [ ] **Step 1: Failing tests**

`tests/test_html.py`:

First move the `cmp` dict from `tests/test_markdown.py` into `tests/data/compare_sample.json` (with `null` for `None`) and make `test_markdown.py` load it: `cmp = json.loads((DATA / "compare_sample.json").read_text())`. Then:

```python
import json
from pathlib import Path

from bench.report import html

DATA = Path(__file__).parent / "data"


def _cmp():
    return json.loads((DATA / "compare_sample.json").read_text())


def test_bar_svg_has_two_bars_per_label():
    svg = html.bar_svg([("a", 900.0, 990.0), ("b", 800.0, 700.0)])
    assert svg.startswith("<svg") and svg.count("<rect") == 4 and ">a<" in svg


def test_line_svg_draws_polyline_per_series():
    svg = html.line_svg({"rs": [("aaa", 0.5), ("bbb", 0.75)], "java": [("aaa", 0.8)]})
    assert svg.count("<polyline") == 2


def test_render_is_self_contained():
    page = html.render(_cmp(), history=[_cmp()])
    assert "<script src" not in page and "http" not in page.split("<body")[1].split("</body>")[0].replace("https://schemas", "")
    assert "java vs rs" in page and "win" in page and "<svg" in page
```

- [ ] **Step 2: Run to verify failure.**

- [ ] **Step 3: Write `bench/report/html.py`**

```python
"""Self-contained HTML dashboard for a compare JSON, with history across reports/."""
from __future__ import annotations

import json
from pathlib import Path

from jinja2 import Environment, FileSystemLoader, select_autoescape

TEMPLATES = Path(__file__).parent / "templates"
_env = Environment(loader=FileSystemLoader(str(TEMPLATES)), autoescape=select_autoescape(["html"]))


def _esc(s: str) -> str:
    return s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


def bar_svg(values: list[tuple[str, float, float]], width: int = 640, row_h: int = 22) -> str:
    """Horizontal grouped bars: for each (label, baseline, candidate)."""
    if not values:
        return "<svg></svg>"
    vmax = max(max(abs(b), abs(c)) for _, b, c in values) or 1.0
    label_w, h = 200, row_h * len(values) + 10
    parts = [f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{h}" font-family="system-ui" font-size="11">']
    for i, (label, b, c) in enumerate(values):
        y = 5 + i * row_h
        bw, cw = (width - label_w - 10) * b / vmax, (width - label_w - 10) * c / vmax
        parts.append(f'<text x="{label_w - 6}" y="{y + 14}" text-anchor="end">{_esc(label)}</text>')
        parts.append(f'<rect x="{label_w}" y="{y}" width="{bw:.1f}" height="8" fill="#8a8a8a"/>')
        parts.append(f'<rect x="{label_w}" y="{y + 10}" width="{cw:.1f}" height="8" fill="#2f7ed8"/>')
    parts.append("</svg>")
    return "".join(parts)


def line_svg(series: dict[str, list[tuple[str, float]]], width: int = 640, height: int = 200) -> str:
    colors = ["#2f7ed8", "#d84f2f", "#2fa84f", "#8a2fd8", "#d8a52f"]
    xs = sorted({x for pts in series.values() for x, _ in pts})
    if not xs:
        return "<svg></svg>"
    ys = [y for pts in series.values() for _, y in pts]
    ymin, ymax = min(ys), max(ys)
    span = (ymax - ymin) or 1.0
    pad = 30
    def px(i): return pad + (width - 2 * pad) * (i / max(len(xs) - 1, 1))
    def py(v): return height - pad - (height - 2 * pad) * ((v - ymin) / span)
    parts = [f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" font-family="system-ui" font-size="10">']
    for i, x in enumerate(xs):
        parts.append(f'<text x="{px(i):.1f}" y="{height - 8}" text-anchor="middle">{_esc(x[:7])}</text>')
    for k, (name, pts) in enumerate(series.items()):
        idx = {x: i for i, x in enumerate(xs)}
        coords = " ".join(f"{px(idx[x]):.1f},{py(y):.1f}" for x, y in pts)
        parts.append(f'<polyline fill="none" stroke="{colors[k % len(colors)]}" stroke-width="2" points="{coords}"/>')
        parts.append(f'<text x="{pad}" y="{12 + 12 * k}" fill="{colors[k % len(colors)]}">{_esc(name)}</text>')
    parts.append("</svg>")
    return "".join(parts)


def load_history(reports_dir: Path) -> list[dict]:
    items = []
    for p in reports_dir.glob("*.json"):
        try:
            d = json.loads(p.read_text())
        except ValueError:
            continue
        if d.get("schema_version") == 1 and "overall" in d:
            items.append(d)
    return sorted(items, key=lambda d: d["created_at"])


def render(cmp: dict, history: list[dict]) -> str:
    names = cmp["against"]
    score_bars = {n: bar_svg([(b, e["baseline"]["score"] or 0.0, (e["against"].get(n) or {}).get("agg", {}).get("score") or 0.0)
                              for b, e in cmp["boards"].items()]) for n in names}
    time_bars = {n: bar_svg([(b, e["baseline"]["wall_s"] or 0.0, (e["against"].get(n) or {}).get("agg", {}).get("wall_s") or 0.0)
                             for b, e in cmp["boards"].items()]) for n in names}
    cp_series: dict[str, list[tuple[str, float]]] = {}
    score_series: dict[str, list[tuple[str, float]]] = {}
    for h in history:
        for n in h["against"]:
            sha = h["candidates"].get(n, {}).get("sha", "?")
            t = h["tiers"].get("all", {}).get(n)
            if t:
                cp_series.setdefault(n, []).append((sha, t["clean_pass_rate"]))
                score_series.setdefault(n, []).append((sha, t["median_score_delta"] or 0.0))
    tpl = _env.get_template("dashboard.html.j2")
    return tpl.render(cmp=cmp, names=names, score_bars=score_bars, time_bars=time_bars,
                      cp_history=line_svg(cp_series), score_history=line_svg(score_series))
```

- [ ] **Step 4: Write `bench/report/templates/dashboard.html.j2`**

```html
<!doctype html>
<html><head><meta charset="utf-8"><title>{{ cmp.baseline }} vs {{ names|join(', ') }}</title>
<style>
body{font-family:system-ui,sans-serif;margin:2rem;color:#222;background:#fff}
h1,h2{margin-top:2rem} table{border-collapse:collapse;font-size:13px} td,th{border:1px solid #ddd;padding:4px 8px;text-align:right}
th:first-child,td:first-child{text-align:left} .cards{display:flex;gap:1rem;flex-wrap:wrap}
.card{border:1px solid #ddd;border-radius:6px;padding:1rem;min-width:180px} .win{background:#e6f4ea}.loss{background:#fbe9e7}.tie{background:#f3f3f3}
.better{color:#1b7f3b}.worse{color:#b3261e}.same{color:#555} .legend span{display:inline-block;width:12px;height:8px;margin-right:4px}
</style></head><body>
<h1>{{ cmp.baseline }} vs {{ names|join(', ') }}</h1>
<p>Created {{ cmp.created_at }} · runs {{ cmp.runs|join(', ') }} · threads={{ cmp.config.threads }} max_passes={{ cmp.config.max_passes }} timeout={{ cmp.config.timeout_s }}s seeds={{ cmp.config.seeds }}</p>
<p>{% for n, i in cmp.candidates.items() %}<b>{{ n }}</b> {{ i.version }} <code>{{ i.sha }}</code>{% if not loop.last %} · {% endif %}{% endfor %}</p>

<h2>Overall</h2>
<div class="cards">{% for n in names %}{% set o = cmp.overall[n] %}
<div class="card"><b>{{ n }}</b><br><span class="{{ o.verdict }}">{{ o.verdict }}</span><br>{{ o.wins }}W / {{ o.losses }}L / {{ o.ties }}T<br>hard losses: {{ o.hard_losses }}</div>
{% endfor %}</div>

<h2>Per tier</h2>
<table><tr><th>tier</th><th>candidate</th><th>wins</th><th>losses</th><th>ties</th><th>clean-pass rate</th><th>median Δscore</th><th>median time ratio</th></tr>
{% for t, row in cmp.tiers.items() %}{% for n, s in row.items() %}
<tr><td>{{ t }}</td><td>{{ n }}</td><td>{{ s.wins }}</td><td>{{ s.losses }}</td><td>{{ s.ties }}</td><td>{{ '%.2f'|format(s.clean_pass_rate) }}</td><td>{{ '%.1f'|format(s.median_score_delta or 0) }}</td><td>{{ '%.2f'|format(s.median_time_ratio or 0) }}</td></tr>
{% endfor %}{% endfor %}</table>

<h2>Win / loss matrix</h2>
<table><tr><th>board</th><th>referee</th>{% for n in names %}<th>{{ n }}</th>{% endfor %}</tr>
{% for b, e in cmp.boards.items() %}<tr><td>{{ b }}</td><td>{{ e.referee }}</td>
{% for n in names %}{% set a = e.against.get(n) %}<td class="{{ a.verdict.result if a else 'tie' }}">{{ a.verdict.result ~ ' (' ~ a.verdict.level ~ ')' if a else 'missing' }}</td>{% endfor %}</tr>{% endfor %}</table>

{% for n in names %}
<h2>Score per board — {{ cmp.baseline }} (grey) vs {{ n }} (blue)</h2>{{ score_bars[n]|safe }}
<h2>Wall time per board — {{ cmp.baseline }} (grey) vs {{ n }} (blue)</h2>{{ time_bars[n]|safe }}
{% endfor %}

<h2>History (all reports, by candidate sha)</h2>
<p>Clean-pass rate</p>{{ cp_history|safe }}
<p>Median Δscore vs baseline</p>{{ score_history|safe }}

{% if cmp.disagreements %}<h2>Self-report disagreements</h2>
<table><tr><th>board</th><th>candidate</th><th>seed</th><th>self unrouted</th><th>referee unrouted</th><th>self viol</th><th>referee viol</th></tr>
{% for d in cmp.disagreements %}<tr><td>{{ d.board }}</td><td>{{ d.candidate }}</td><td>{{ d.seed }}</td><td>{{ d.self_unrouted }}</td><td>{{ d.referee_unrouted }}</td><td>{{ d.self_violations }}</td><td>{{ d.referee_violations }}</td></tr>{% endfor %}</table>{% endif %}
{% if cmp.failures %}<h2>Failures</h2><ul>{% for f in cmp.failures %}<li>{{ f.board }} / {{ f.candidate }} / seed {{ f.seed }}: {{ f.reason }}</li>{% endfor %}</ul>{% endif %}
{% if cmp.warnings %}<h2>Warnings</h2><ul>{% for w in cmp.warnings %}<li>{{ w }}</li>{% endfor %}</ul>{% endif %}
</body></html>
```

- [ ] **Step 5: Run** — `uv run pytest tests/test_html.py -v` — Expected: 3 passed.

- [ ] **Step 6: Wire into CLI** — in `compare_cmd` and `report_cmd`, after writing `.md`:

```python
from bench.report import html as html_report
    (paths.REPORTS / f"{out_name}.html").write_text(html_report.render(cmp, html_report.load_history(paths.REPORTS)))
```

Run `uv run bench report --compare sanity` and open `reports/sanity.html` in a browser; verify cards, matrix and SVGs render.

- [ ] **Step 7: Run the whole suite and commit**

```bash
uv run pytest -q
git add -A && git commit -m "feat: self-contained HTML dashboard with history"
```

---

## Self-review

**Spec coverage:** §3 candidates → Task 1; §4 layout → Tasks 1–9; §5 corpus/tiers → Tasks 2, 8; §6 runner → Task 3; §7 referees + fallback + disagreement → Tasks 4, 7; §8 metrics → Task 4; §9 compare/verdict/noise → Task 5; §10 reports → Tasks 5, 9; §11 CLI → Tasks 2, 3, 4, 5, 8, 9; §12 error handling → paths.ToolMissing (T1), runner statuses (T3), referee_failed (T4/T7), IncompatibleRuns (T5); §13 testing → each task; §14 build order → task order. Not covered on purpose: `--router.seed` support depends on the fork (spec §15), handled by `extra_args` templating in Task 1.

**Placeholders:** none; Task 8 step 4 flags a dead block that must be deleted (explicitly).

**Type consistency:** `Board` fields and `runner.cell_dir` signature are used identically in Tasks 2–9; `referee.json` / `metrics.json` shapes fixed in Task 4 and consumed unchanged in Tasks 5, 7, 9; `compare` JSON shape fixed in Task 5 and consumed by Tasks 5 (md) and 9 (html).
