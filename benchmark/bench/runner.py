"""Execute candidate × board × seed cells and persist everything under results/<run-id>/."""
from __future__ import annotations

import json
import os
import platform
import shutil
import threading
from collections.abc import Callable
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path

from bench import metrics
from bench.candidates import Candidate
from bench.corpus import Board
from bench.paths import RESULTS, isolated_env  # RESULTS is a module attr so tests can monkeypatch bench.runner.RESULTS
from bench.pool import run_cells
from bench.timing import run_timed

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
    jobs: int = 1  # number of cells (candidate x board x seed) to run concurrently
    tier: str | None = None
    board_ids: list[str] = field(default_factory=list)
    referee_java: dict | None = None
    candidates_file: str | None = None
    grace_s: int = 60  # extra time (beyond timeout_s) the suite waits before killing the process group


def _now() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="seconds")


def cell_dir(run_dir: Path, cand_name: str, board_id: str, seed: int) -> Path:
    return run_dir / cand_name / board_id / f"seed-{seed}"


def load_meta(run_dir: Path) -> dict:
    return json.loads((run_dir / "meta.json").read_text())


def _save_meta(run_dir: Path, meta: dict) -> None:
    # os.replace is atomic on POSIX and Windows, so a concurrent reader never sees a
    # truncated/partial meta.json.
    tmp = run_dir / ".meta.json.tmp"
    tmp.write_text(json.dumps(meta, indent=2) + "\n")
    os.replace(tmp, run_dir / "meta.json")


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
    try:
        shutil.copyfile(board.path, in_dsn)
    except FileNotFoundError:
        return {"candidate": cand.name, "board": board.id, "seed": seed, "status": "missing_input"}
    argv = cand.argv(in_dsn=in_dsn, out_ses=cell / "out.ses", result_json=cell / "result.json",
                     max_passes=cfg.max_passes, timeout_s=cfg.timeout_s, threads=cfg.threads, seed=seed)
    (cell / "argv.json").write_text(json.dumps(argv, indent=2))
    # Per-cell HOME/XDG so the candidate's persisted freerouting.json can't leak settings
    # to/from any other candidate or run on this host (see README's "Settings isolation").
    home = cell / "home"
    home.mkdir(parents=True, exist_ok=True)
    env = isolated_env(home)
    (cell / "env.json").write_text(json.dumps(env, indent=2))
    try:
        t = run_timed(argv, cwd=cell, timeout_s=cfg.timeout_s + cfg.grace_s,
                      stdout=cell / "stdout.log", stderr=cell / "stderr.log", env=env)
    except OSError as e:
        # e.g. the candidate's executable doesn't exist: record as crashed rather than
        # raising, so one bad candidate can't abort cells running concurrently under jobs > 1.
        time_json = {"wall_s": 0.0, "cpu_s": None, "peak_rss_mb": None,
                     "exit_code": -1, "timed_out": False, "error": str(e), "isolated_config": True}
        (cell / "time.json").write_text(json.dumps(time_json, indent=2))
        return {"candidate": cand.name, "board": board.id, "seed": seed, "status": "crashed"}
    time_json = {"wall_s": t.wall_s, "cpu_s": t.cpu_s, "peak_rss_mb": t.peak_rss_mb,
                 "exit_code": t.exit_code, "timed_out": t.timed_out, "isolated_config": True}
    (cell / "time.json").write_text(json.dumps(time_json, indent=2))
    return {"candidate": cand.name, "board": board.id, "seed": seed, "status": _cell_status(time_json, cell)}


def _run_one_cell(cand: Candidate, board: Board, seed: int, cfg: RunConfig, run_dir: Path,
                  referee: RefereeHook | None, progress: Callable[[str], None] | None) -> dict:
    if progress:
        progress(f"{cand.name} × {board.id} × seed {seed}")
    try:
        entry = run_cell(cand, board, seed, cfg, run_dir)
        if referee is not None:
            cell = cell_dir(run_dir, cand.name, board.id, seed)
            try:
                referee(board, cell)
            except Exception as e:
                r = {"status": "referee_failed", "referee": board.referee,
                     "candidate_output": (cell / "out.ses").exists(),
                     "reason": f"referee raised {type(e).__name__}: {e}"}
                (cell / "referee.json").write_text(json.dumps(r, indent=2) + "\n")
                metrics.build(cell, board)
                entry["status"] = "referee_error"
        return entry
    except Exception as e:
        # Catch-all so one cell's failure (e.g. an extra_args template referencing an
        # unknown placeholder) can't take down cells running concurrently under jobs > 1.
        cell = cell_dir(run_dir, cand.name, board.id, seed)
        cell.mkdir(parents=True, exist_ok=True)
        (cell / "error.txt").write_text(f"{type(e).__name__}: {e}\n")
        return {"candidate": cand.name, "board": board.id, "seed": seed, "status": "error"}


def run(cfg: RunConfig, referee: RefereeHook | None,
        progress: Callable[[str], None] | None = None) -> Path:
    run_dir = RESULTS / cfg.run_id
    run_dir.mkdir(parents=True, exist_ok=False)
    meta = {
        "schema_version": 1, "run_id": cfg.run_id, "started_at": _now(), "finished_at": None,
        "status": "incomplete",
        "host": {"node": platform.node(), "machine": platform.machine()},
        "args": {"seeds": cfg.seeds, "max_passes": cfg.max_passes, "timeout_s": cfg.timeout_s,
                 "threads": cfg.threads, "jobs": cfg.jobs, "tier": cfg.tier,
                 "boards": [b.id for b in cfg.boards]},
        "candidates": [c.to_json() for c in cfg.candidates],
        "referee_java": cfg.referee_java,
        "candidates_file": cfg.candidates_file,
        "cells": [],
    }
    _save_meta(run_dir, meta)

    # Nested candidate/board/seed order so a partial run has complete candidate blocks;
    # run_cells collects results in this same submission order regardless of jobs, so
    # meta.json is rewritten gap-free (see bench.pool.run_cells).
    items = [(cand, board, seed)
             for cand in cfg.candidates for board in cfg.boards for seed in range(1, cfg.seeds + 1)]

    meta_lock = threading.Lock()  # defensive only: on_done runs in the submitting thread, never concurrently

    def _fn(item: tuple[Candidate, Board, int]) -> dict:
        cand, board, seed = item
        return _run_one_cell(cand, board, seed, cfg, run_dir, referee, progress)

    def _on_done(entry: dict) -> None:
        with meta_lock:
            meta["cells"].append(entry)
            _save_meta(run_dir, meta)

    run_cells(items, _fn, cfg.jobs, on_done=_on_done)

    meta["status"] = "complete"
    meta["finished_at"] = _now()
    _save_meta(run_dir, meta)
    return run_dir
