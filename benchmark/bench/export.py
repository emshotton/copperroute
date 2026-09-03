"""Per-board metrics export: a run's `metrics.json` files, flattened to CSV/JSON.

Every exported row is self-describing (`candidate`, `sha`) so a CSV survives being
concatenated with another candidate's export or read on its own -- see `CSV_FIELDS`.
"""
from __future__ import annotations

import csv
import json
import re
import subprocess
from datetime import datetime, timezone
from pathlib import Path

from bench import paths, runner
from bench.corpus import Board

CSV_FIELDS = ["candidate", "sha", "board", "tier", "nets", "layers", "clean_pass", "unrouted",
              "violations", "score", "cpu_s", "wall_s", "peak_rss_mb", "vias", "wirelength_mm",
              "wirelength_ratio", "via_ratio", "timed_out"]

_SHA_UNSAFE = re.compile(r"[^A-Za-z0-9_.-]")


def _tier_for(board: Board) -> str:
    """The board's d3-* tier if it has one, else its first tier (empty string if none)."""
    for t in board.tiers:
        if t.startswith("d3-"):
            return t
    return board.tiers[0] if board.tiers else ""


def _candidate_info(meta: dict, candidate: str) -> dict:
    for c in meta["candidates"]:
        if c["name"] == candidate:
            return c
    raise KeyError(candidate)


def _board_ids_with_metrics(meta: dict, run_dir: Path, candidate: str) -> list[str]:
    """Board ids this candidate has at least one seed with a written metrics.json for,
    sorted. (A cell can be present in meta["cells"] without metrics.json -- e.g. it
    crashed, or `--no-referee` was used and `bench referee` hasn't run yet.)"""
    by_board: dict[str, set[int]] = {}
    for c in meta["cells"]:
        if c["candidate"] == candidate:
            by_board.setdefault(c["board"], set()).add(c["seed"])
    out = []
    for bid, seeds in by_board.items():
        for seed in sorted(seeds):
            if (runner.cell_dir(run_dir, candidate, bid, seed) / "metrics.json").exists():
                out.append(bid)
                break
    return sorted(out)


def export_run(run_dir: Path, candidate: str, boards: list[Board]) -> list[dict]:
    """Build export rows for one candidate of one run: one row per board that (a) has a
    metrics.json for `candidate` in this run and (b) is present with status "ok" in
    `boards` (typically the full corpus manifest) -- a board missing from that set (not
    present at all, or present but excluded) is silently skipped here; see
    `excluded_count` to report how many were skipped. Rows are sorted by board id and are
    self-describing (`candidate`, `sha` on every row). Pure: only reads run_dir/meta.json
    and this candidate's metrics.json files, never writes anything.
    """
    meta = runner.load_meta(run_dir)
    cand_info = _candidate_info(meta, candidate)
    sha = cand_info.get("sha", "")
    by_id = {b.id: b for b in boards if b.status == "ok"}
    rows = []
    for bid in _board_ids_with_metrics(meta, run_dir, candidate):
        board = by_id.get(bid)
        if board is None:
            continue
        # There may be several seeds with metrics.json; the exported row is a single
        # per-board summary, so take the first (lowest-numbered) seed that has one.
        for c in sorted((c for c in meta["cells"] if c["candidate"] == candidate and c["board"] == bid),
                        key=lambda c: c["seed"]):
            mp = runner.cell_dir(run_dir, candidate, bid, c["seed"]) / "metrics.json"
            if mp.exists():
                m = json.loads(mp.read_text())
                break
        else:
            continue  # pragma: no cover -- unreachable, bid came from _board_ids_with_metrics
        rows.append({
            "candidate": candidate, "sha": sha,
            "board": bid, "tier": _tier_for(board), "nets": board.nets, "layers": board.layers,
            "clean_pass": bool(m.get("clean_pass")), "unrouted": m.get("unrouted"),
            "violations": m.get("violations"), "score": m.get("score"),
            "cpu_s": m.get("cpu_s"), "wall_s": m.get("wall_s"), "peak_rss_mb": m.get("peak_rss_mb"),
            "vias": m.get("vias"), "wirelength_mm": m.get("wirelength_mm"),
            "wirelength_ratio": m.get("wirelength_ratio"), "via_ratio": m.get("via_ratio"),
            "timed_out": bool(m.get("timed_out")),
        })
    rows.sort(key=lambda r: r["board"])
    return rows


def excluded_count(run_dir: Path, candidate: str, boards: list[Board]) -> int:
    """Number of boards that have a metrics.json for `candidate` in this run but were
    skipped from `export_run` because they're absent from `boards`, or present with a
    status other than "ok" (i.e. excluded from the corpus manifest)."""
    meta = runner.load_meta(run_dir)
    _candidate_info(meta, candidate)  # raises KeyError for an unknown candidate, same as export_run
    by_id = {b.id: b for b in boards if b.status == "ok"}
    ids = _board_ids_with_metrics(meta, run_dir, candidate)
    return sum(1 for bid in ids if bid not in by_id)


def _csv_value(v):
    if isinstance(v, bool):
        return "true" if v else "false"
    if v is None:
        return ""
    return v


def write_csv(path: Path, rows: list[dict]) -> None:
    with path.open("w", newline="") as f:
        w = csv.writer(f)
        w.writerow(CSV_FIELDS)
        for r in rows:
            w.writerow([_csv_value(r[k]) for k in CSV_FIELDS])


def _corpus_commit() -> str | None:
    """Short git hash of the last commit that touched corpus/manifest.json, or None if it
    can't be determined (no git, not a repo, or the file has never been committed)."""
    try:
        out = subprocess.run(["git", "log", "-1", "--format=%h", "--", "corpus/manifest.json"],
                             cwd=paths.ROOT, capture_output=True, text=True, check=True)
        sha = out.stdout.strip()
        return sha or None
    except (subprocess.CalledProcessError, FileNotFoundError):
        return None


def _export_basename(run_id: str, candidate: str, sha: str) -> tuple[str, str | None]:
    """(basename, warning). Filenames include a filesystem-safe 7-char prefix of the
    candidate's resolved git sha (`exports/<run>-<candidate>-<sha7>`); when the sha is
    unresolved ("" or "unknown" -- see bench.candidates._git_sha), the suffix is omitted
    and a warning is returned instead of silently mislabeling the file."""
    if not sha or sha == "unknown":
        return f"{run_id}-{candidate}", (
            f"candidate {candidate!r} has no resolved git sha (sha={sha!r}); "
            f"export filename for run {run_id!r} omits the sha suffix")
    safe = _SHA_UNSAFE.sub("_", sha)[:7]
    return f"{run_id}-{candidate}-{safe}", None


def write_export(run_dir: Path, candidate: str, boards: list[Board], out_dir: Path) -> dict:
    """Write `<out_dir>/<run>-<candidate>[-<sha7>].{csv,json}` and return
    {"csv_path", "json_path", "rows", "skipped", "warning"}. Raises KeyError if
    `candidate` isn't in this run's meta.json."""
    meta = runner.load_meta(run_dir)
    cand_info = _candidate_info(meta, candidate)
    sha = cand_info.get("sha", "")
    rows = export_run(run_dir, candidate, boards)
    skipped = excluded_count(run_dir, candidate, boards)

    out_dir.mkdir(parents=True, exist_ok=True)
    base, warning = _export_basename(meta["run_id"], candidate, sha)
    csv_path, json_path = out_dir / f"{base}.csv", out_dir / f"{base}.json"
    write_csv(csv_path, rows)
    payload = {
        "schema_version": 1, "run": meta["run_id"],
        "candidate": {"name": cand_info["name"], "sha": sha, "version": cand_info.get("version", "")},
        "router_git_sha": sha,
        "config": meta.get("args", {}), "host": meta.get("host"),
        "exported_at": datetime.now(timezone.utc).isoformat(timespec="seconds"),
        "corpus_commit": _corpus_commit(),
        "rows": rows,
    }
    json_path.write_text(json.dumps(payload, indent=2) + "\n")
    return {"csv_path": csv_path, "json_path": json_path, "rows": rows, "skipped": skipped, "warning": warning}
