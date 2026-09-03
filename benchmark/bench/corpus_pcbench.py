"""Import KiCad-sourced boards: strip routing, export unrouted DSN, record ground truth.

Two importers share one pipeline (``_import_board``):

- ``import_boards``/``import_one``: PCBench boards, which ship a separate ``processed.kicad_pcb``
  (routing-stripped-or-not input) and ``raw.kicad_pcb`` (the reference/"ideal" routed board used
  for ground truth DRC and stats).
- ``import_kicad_fixtures``: the Java repo's own KiCad fixtures under ``fixtures/*/*.kicad_pcb``,
  which have no separate processed/raw split -- the same file plays both roles.

Most PCBench boards are KiCad-4/5-era files with no ``.kicad_pro`` and legacy zone fills, which
without correction makes ``kicad-cli pcb drc`` both apply KiCad's generic default design rules
(0.25mm hole clearance, 0.5mm copper-edge clearance, 0.2mm min track -- rules these boards never
had) and report thousands of ``clearance`` errors that are purely an artefact of the stale legacy
fill strategy KiCad 7+ no longer treats as filled. So for a board with no project, ``_import_board``
generates one from the board's own legacy ``(net_class ...)`` rules (``vendor/kicad/legacy_rules.py``,
pure Python) before DRC-ing the reference board, and always re-fills zones first
(``vendor/kicad/refill_zones.py``, KiCad Python) to clear the stale-fill noise. The exclusion check
(``reference_verdict``) excludes a reference with any routing-type DRC error
(``bench.referee.kicad.ROUTING_DRC_TYPES``), and separately excludes one with unconnected nets
(``raw-drc.json``'s ``unconnected_items``) unless it's unrouted by nature (0 wirelength) -- see
README.md "KiCad referee status" and the PCBench section for the measured before/after counts and
the admission criteria.

``revalidate`` recomputes an already-imported board's ``drv_routing``/``drv_all``/``unconnected``/
``reference_complete`` (and, with ``--regenerate-projects``/``--rerun-drc``, its generated KiCad
project and/or DRC report) without re-running the full strip/export/stats pipeline, so a fix to
``reference_verdict`` or ``vendor/kicad/legacy_rules.py`` can be picked up across the whole corpus
in minutes -- see ``bench corpus revalidate``.
"""
from __future__ import annotations

import importlib.util
import json
import shutil
import subprocess
import time
from collections.abc import Callable
from pathlib import Path

from bench import corpus
from bench.corpus import Board
from bench.paths import VENDOR_KICAD, ToolMissing, kicad_cli, kicad_python
from bench.pool import run_cells
from bench.referee.kicad import ROUTING_DRC_TYPES

STEP_TIMEOUT_S = 600

Progress = Callable[[str], None]

_legacy_rules_module = None


def _legacy_rules():
    """Lazily load vendor/kicad/legacy_rules.py by path (it's pure Python, no pcbnew, but
    vendor/kicad isn't an importable package -- same reason tests/test_ses_to_board.py loads
    ses_to_board.py this way)."""
    global _legacy_rules_module
    if _legacy_rules_module is None:
        spec = importlib.util.spec_from_file_location("legacy_rules", VENDOR_KICAD / "legacy_rules.py")
        mod = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(mod)
        _legacy_rules_module = mod
    return _legacy_rules_module


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
        except OSError as e:
            return -1, str(e)


def _generate_project(raw_pcb: Path, dst: Path) -> None:
    """Write dst/raw.kicad_pro and dst/stripped.kicad_pro from raw_pcb's own legacy
    (net_class ...) blocks and (setup ...) board-wide floors (see
    vendor/kicad/legacy_rules.py). Used when no real project file ships with the board."""
    text = raw_pcb.read_text(encoding="utf-8", errors="replace")
    classes = _legacy_rules().parse_legacy_netclasses(text)
    setup_floors = _legacy_rules().parse_legacy_setup(text)
    for name in ("raw.kicad_pro", "stripped.kicad_pro"):
        project = _legacy_rules().build_project(classes, name, setup_floors)
        (dst / name).write_text(json.dumps(project, indent=2) + "\n")


def reference_verdict(gt: dict) -> tuple[str, str]:
    """Decide a reference board's manifest ``(status, reason)`` from its ``ground_truth.json``
    fields. Used by both ``_import_board`` (at import time) and ``revalidate`` (recomputing
    from an already-imported board's ``raw-drc.json`` on disk), so the exclusion rule lives
    in exactly one place.

    Two independent checks, in order:

    1. **Routing DRC errors** (``drv_routing``, from ``bench.referee.kicad.ROUTING_DRC_TYPES``)
       always exclude the board, whether or not the reference has any routing at all. This is
       deliberately unconditional: a board with zero vias/wirelength ("unrouted by nature", see
       below) can still trip a genuine routing-type DRC error unrelated to unconnected nets
       (e.g. a zone/footprint clearance issue that happens to fall under a routing DRC type),
       and that must not be waved through just because the board has no tracks -- previously
       it was, via a carve-out that (wrongly) covered this case too. See
       ``tests/test_corpus_pcbench.py::test_reference_verdict_routing_error_excludes_even_when_unrouted``.
    2. **Unconnected nets** (``unconnected``, the count of ``raw-drc.json``'s
       ``unconnected_items``) exclude the board *only* when it has some routing
       (``wirelength_mm > 0``). An unrouted reference (0 wirelength) has every one of its nets
       unconnected by construction -- that's expected for a bug-report fixture that was never
       actually routed, not a broken reference -- so it stays ``ok`` but is flagged
       ``reference_complete: false`` and carries no ratio ground truth (see
       ``ground_truth.json``'s ``reference_complete``).

    A clean, fully-connected (or legitimately unrouted) reference is ``("ok", "")``.
    """
    drv_routing = gt.get("drv_routing", 0)
    if drv_routing > 0:
        return ("excluded",
                f"reference board has {drv_routing} routing DRC errors (of {gt.get('drv_all', drv_routing)} total)")
    unconnected = gt.get("unconnected", 0)
    if unconnected > 0 and gt.get("wirelength_mm", 0) > 0:
        return "excluded", f"reference board has {unconnected} unconnected items"
    return "ok", ""


def _refill_zones(py: str, raw_pcb: Path, dst: Path, log: Path) -> tuple[int | None, str]:
    """Re-fill raw_pcb's zones in place (backing up the original as raw.orig.kicad_pcb)
    with vendor/kicad/refill_zones.py, clearing legacy-fill-strategy DRC noise before the
    reference DRC step. Returns (zones filled, "") on success, (None, reason) on failure."""
    shutil.copyfile(raw_pcb, dst / "raw.orig.kicad_pcb")
    try:
        p = subprocess.run([py, str(VENDOR_KICAD / "refill_zones.py"), str(raw_pcb), str(raw_pcb)],
                           capture_output=True, text=True, timeout=STEP_TIMEOUT_S)
    except (subprocess.TimeoutExpired, OSError) as e:
        return None, f"refill_zones.py failed on raw board: {e}"
    with open(log, "ab") as f:
        f.write((p.stdout + getattr(p, "stderr", "")).encode())
    if p.returncode != 0:
        return None, "refill_zones.py failed on raw board"
    try:
        return json.loads(p.stdout).get("zones"), ""
    except ValueError:
        return None, ""


def _import_board(src_pcb: Path, project: Path | None, dst: Path, board_id: str, origin: str,
                   tiers: list[str], layers_hint: int = 0, kicad_version: str | None = None,
                   raw_pcb_src: Path | None = None) -> Board:
    """Refill zones -> generate/copy a project -> strip ``src_pcb`` -> export unrouted DSN ->
    DRC+stats the reference board -> ground truth.

    ``dst / "raw.kicad_pcb"`` is the reference board DRC and stats run against, and is always
    (re)copied fresh from its real source on every call: ``raw_pcb_src`` if the caller supplies
    a distinct reference board (PCBench: the board's own ``raw.kicad_pcb``, separate from the
    routing-stripped ``processed.kicad_pcb`` passed as ``src_pcb``), otherwise ``src_pcb`` itself
    (repo KiCad fixtures: one file plays both roles). This copy is unconditional -- not "only if
    missing" -- because on a forced or resumed re-import, a stale ``raw.kicad_pcb`` left over
    from a previous run has already been zone-refilled (see below), which stops serialising the
    source board's legacy ``(net_class ...)`` blocks as literal text; reusing it instead of
    re-copying the pristine source would silently lose that net-class fidelity on every re-import
    after the first. Stale per-import artefacts from a previous run (``raw.orig.kicad_pcb``,
    ``raw.kicad_pro``, ``raw.kicad_prl``) are removed right after so nothing downstream
    accidentally reads leftovers instead of freshly regenerated output.

    Zone refill (``vendor/kicad/refill_zones.py``) runs FIRST, before any project file exists:
    its ``pcbnew`` ``board.Save()`` rewrites the board into KiCad's modern format, which (a) no
    longer serialises legacy ``(net_class ...)`` blocks as literal text -- so they must be parsed
    from the pre-refill backup (``raw.orig.kicad_pcb``), not the refilled file -- and (b) can
    itself write a same-stem ``.kicad_pro``/``.kicad_prl`` using pcbnew's own in-memory *default*
    constraints (``refill_zones.py`` cleans up anything it creates itself, but nothing protects a
    project that already existed beforehand). So the real/generated project is (re)written
    *after* refill, overwriting whatever pcbnew left behind, and any stray ``.kicad_prl`` is
    removed; when the project is generated, its ``rules.min_hole_clearance`` is logged and
    asserted to be 0 as a regression canary for this exact ordering bug. Only routing-type DRC
    errors (``bench.referee.kicad.ROUTING_DRC_TYPES``) count toward exclusion.
    """
    dst.mkdir(parents=True, exist_ok=True)
    log = dst / "import.log"
    raw_pcb = dst / "raw.kicad_pcb"
    shutil.copyfile(raw_pcb_src or src_pcb, raw_pcb)
    for stale in ("raw.orig.kicad_pcb", "raw.kicad_pro", "raw.kicad_prl"):
        (dst / stale).unlink(missing_ok=True)

    def _rel(p: Path) -> str:
        return p.relative_to(corpus.CORPUS).as_posix()

    kicad_meta = {"raw": _rel(raw_pcb), "stripped": _rel(dst / "stripped.kicad_pcb"),
                  "ground_truth": _rel(dst / "ground_truth.json")}
    source = (dst.relative_to(corpus.CORPUS) / "unrouted.dsn").as_posix()
    board = Board(id=board_id, source=source, origin=origin, referee="kicad", tiers=list(tiers),
                  layers=layers_hint, kicad=kicad_meta)

    try:
        py, cli = str(kicad_python()), str(kicad_cli())
    except ToolMissing as e:
        board.status, board.reason = "excluded", str(e)
        return board

    zones_refilled, reason = _refill_zones(py, raw_pcb, dst, log)
    if reason:
        board.status, board.reason = "excluded", reason
        return board
    pristine_raw = dst / "raw.orig.kicad_pcb"  # written by _refill_zones before it ran Save()

    project_generated = False
    if project and project.exists():
        shutil.copyfile(project, dst / "raw.kicad_pro")
        shutil.copyfile(project, dst / "stripped.kicad_pro")
    else:
        _generate_project(pristine_raw, dst)
        project_generated = True
    kicad_meta["project"] = _rel(dst / "stripped.kicad_pro")

    # A stray raw.kicad_prl (project *local* settings) from refill's board.Save() isn't
    # overwritten by copying/generating raw.kicad_pro above, and kicad-cli picks up a
    # same-stem .kicad_prl automatically -- so remove it if pcbnew left one behind.
    stray_prl = dst / "raw.kicad_prl"
    if stray_prl.exists():
        stray_prl.unlink()

    if project_generated:
        for name in ("raw.kicad_pro", "stripped.kicad_pro"):
            rules = json.loads((dst / name).read_text())["board"]["design_settings"]["rules"]
            min_hole = rules.get("min_hole_clearance")
            with open(log, "ab") as f:
                f.write(f"{name}: rules.min_hole_clearance={min_hole}\n".encode())
            assert min_hole == 0, (
                f"{name}: generated project has non-zero min_hole_clearance={min_hole} -- "
                "zone refill's board.Save() may have clobbered it (see _import_board ordering)"
            )

    steps = [
        [py, str(VENDOR_KICAD / "strip_kicad_routing.py"), str(src_pcb), str(dst / "stripped.kicad_pcb")],
        [py, str(VENDOR_KICAD / "export_specctra_dsn.py"), str(dst / "stripped.kicad_pcb"), str(dst / "unrouted.dsn")],
        [cli, "pcb", "drc", "--format", "json", "--units", "mm", "-o", str(dst / "raw-drc.json"), str(raw_pcb)],
    ]
    for argv in steps:
        rc, reason = _kicad_step(argv, log)
        if rc != 0:
            board.status, board.reason = "excluded", reason
            return board

    try:
        stats_rc = subprocess.run([py, str(VENDOR_KICAD / "board_stats.py"), str(raw_pcb)],
                                  capture_output=True, text=True, timeout=STEP_TIMEOUT_S)
    except (subprocess.TimeoutExpired, OSError) as e:
        board.status, board.reason = "excluded", f"board_stats.py failed on raw board: {e}"
        return board
    if stats_rc.returncode != 0:
        board.status, board.reason = "excluded", "board_stats.py failed on raw board"
        return board

    stats = json.loads(stats_rc.stdout)
    drc = json.loads((dst / "raw-drc.json").read_text())
    errors = [v for v in drc.get("violations", []) if v.get("severity", "error") == "error"]
    drv_all = len(errors)
    drv_routing = sum(1 for v in errors if v.get("type") in ROUTING_DRC_TYPES)
    # `unconnected_items` is a separate top-level array in kicad-cli's DRC report, not part of
    # `violations` -- see bench.referee.kicad.parse_kicad_drc, which reads it the same way for
    # candidate-routed boards.
    unconnected = len(drc.get("unconnected_items", []))
    nets, layers = corpus.dsn_info(dst / "unrouted.dsn")
    board.nets, board.layers = nets, layers or board.layers
    reference_complete = unconnected == 0 and stats["wirelength_mm"] > 0
    gt = {"wirelength_mm": stats["wirelength_mm"], "vias": stats["vias"],
          "drv_routing": drv_routing, "drv_all": drv_all, "unconnected": unconnected,
          "reference_complete": reference_complete,
          "project_generated": project_generated, "zones_refilled": zones_refilled,
          "nets": nets, "layers": board.layers, "kicad_version": kicad_version}
    (dst / "ground_truth.json").write_text(json.dumps(gt, indent=2) + "\n")

    board.status, board.reason = reference_verdict(gt)

    tier = corpus.assign_d3_tier(nets)
    if tier:
        board.tiers.append(tier)
    return board


def import_one(root: Path, bid: str) -> Board:
    src = root / "PCBs" / bid
    dst = corpus.CORPUS / "pcbench" / bid
    meta = json.loads((src / "metadata.json").read_text())
    project = next(iter(sorted(src.glob("*.kicad_pro"))), None)
    # raw_pcb_src is the board's own "ideal" reference board -- distinct from src_pcb
    # (processed.kicad_pcb, the routing-stripped input) -- so _import_board copies it into
    # dst/raw.kicad_pcb itself rather than reusing whatever's already there.
    return _import_board(src / "processed.kicad_pcb", project, dst, f"pcbench-{bid}", "pcbench",
                         ["pcbench"], layers_hint=int(meta.get("layers", 0) or 0),
                         kicad_version=meta.get("kicad_version"), raw_pcb_src=src / "raw.kicad_pcb")


def _already_imported(existing: dict[str, Board], board_id: str, dst: Path) -> bool:
    """True if `board_id`'s manifest entry is `ok` and its DSN + ground truth are both on
    disk -- i.e. a resumed import can skip re-running the (slow) KiCad pipeline for it."""
    b = existing.get(board_id)
    return bool(b and b.status == "ok"
                and (dst / "unrouted.dsn").exists() and (dst / "ground_truth.json").exists())


def _progress_line(b: Board, elapsed: float) -> str:
    status = "ok" if b.status == "ok" else f"excluded: {b.reason}"
    return f"{b.id}: {status} ({elapsed:.1f}s)"


def import_boards(root: Path, ids: list[str] | None = None, max_boards: int | None = None,
                  jobs: int = 1, skip_existing: bool = True,
                  progress: Progress | None = None) -> list[Board]:
    ids = ids or list_board_ids(root)
    if max_boards:
        ids = ids[:max_boards]
    existing = {b.id: b for b in corpus.load_manifest()}

    todo = []
    skipped = 0
    for bid in ids:
        board_id = f"pcbench-{bid}"
        if skip_existing and _already_imported(existing, board_id, corpus.CORPUS / "pcbench" / bid):
            skipped += 1
        else:
            todo.append(bid)

    def _fn(bid: str) -> tuple[Board, float]:
        t0 = time.monotonic()
        b = import_one(root, bid)
        return b, time.monotonic() - t0

    # `run_cells` invokes `on_done` from the main thread only, in submission order, as each
    # result becomes available -- so the manifest is read-modified-written here without any
    # extra locking even under `jobs > 1`, and every completed board is persisted before the
    # next one starts saving (see bench.pool.run_cells's docstring).
    def _on_done(result: tuple[Board, float]) -> None:
        b, elapsed = result
        existing[b.id] = b
        corpus.save_manifest(list(existing.values()))
        if progress:
            progress(_progress_line(b, elapsed))

    run_cells(todo, _fn, jobs, on_done=_on_done)

    if progress and skipped:
        progress(f"skipped {skipped} already-imported board(s)")

    return [existing[f"pcbench-{bid}"] for bid in ids]


def import_kicad_fixtures(fixtures_dir: Path, jobs: int = 1, skip_existing: bool = True,
                          progress: Progress | None = None) -> list[Board]:
    """Import the Java repo's own KiCad fixture boards (fixtures/<dir>/<name>.kicad_pcb)."""
    existing = {b.id: b for b in corpus.load_manifest()}

    cells: list[tuple[Path, Path | None, Path, str]] = []
    for d in sorted(p for p in fixtures_dir.iterdir() if p.is_dir()):
        for pcb in sorted(d.glob("*.kicad_pcb")):
            if pcb.stem.endswith("-bak"):
                continue
            name = pcb.stem
            project = pcb.with_suffix(".kicad_pro")
            dst = corpus.CORPUS / "kicad" / f"{d.name}--{name}"
            board_id = f"kicad-{d.name.lower()}--{name.lower()}"
            cells.append((pcb, project if project.exists() else None, dst, board_id))

    todo = []
    skipped = 0
    for cell in cells:
        _pcb, _project, dst, board_id = cell
        if skip_existing and _already_imported(existing, board_id, dst):
            skipped += 1
        else:
            todo.append(cell)

    def _fn(cell: tuple[Path, Path | None, Path, str]) -> tuple[Board, float]:
        pcb, project, dst, board_id = cell
        t0 = time.monotonic()
        b = _import_board(pcb, project, dst, board_id, "freerouting-kicad", ["kicad-fixtures"])
        return b, time.monotonic() - t0

    def _on_done(result: tuple[Board, float]) -> None:
        b, elapsed = result
        existing[b.id] = b
        corpus.save_manifest(list(existing.values()))
        if progress:
            progress(_progress_line(b, elapsed))

    run_cells(todo, _fn, jobs, on_done=_on_done)

    if progress and skipped:
        progress(f"skipped {skipped} already-imported board(s)")

    return [existing[board_id] for (_pcb, _project, _dst, board_id) in cells]


def _regenerate_project(dst: Path) -> bool:
    """Regenerate ``dst/raw.kicad_pro`` + ``dst/stripped.kicad_pro`` from
    ``dst/raw.orig.kicad_pcb`` -- the pristine, pre-refill backup ``_refill_zones`` wrote at
    import time, which (unlike ``raw.kicad_pcb`` after refill) still serialises the board's
    legacy ``(net_class ...)``/``(setup ...)`` blocks as literal text. Lets a
    ``vendor/kicad/legacy_rules.py`` fix (e.g. the setup-floor micrometre rounding) be picked
    up by ``revalidate`` without re-running the whole KiCad import pipeline. Returns False
    (no-op) if no backup exists on disk -- e.g. a board imported before ``_refill_zones``
    started writing one, or one whose zones were never refilled."""
    pristine = dst / "raw.orig.kicad_pcb"
    if not pristine.exists():
        return False
    _generate_project(pristine, dst)
    return True


def _revalidate_board(b: Board, cli: str | None, regenerate_projects: bool, rerun_drc: bool) -> dict:
    """Recompute one board's ground truth + verdict from what's already on disk -- optionally
    regenerating its KiCad project and/or re-running DRC first. Pure per-board work (no
    manifest read/write) so it can run inside a `run_cells` worker thread; the caller
    (`revalidate`) folds the result into the manifest from the main thread."""
    result = {"id": b.id, "skipped": None, "old_status": b.status, "old_reason": b.reason,
              "new_status": b.status, "new_reason": b.reason,
              "regenerated_project": False, "reran_drc": False}
    gt_path = b.kicad_path("ground_truth") if b.kicad else None
    if not gt_path or not gt_path.exists():
        result["skipped"] = "no ground_truth.json"
        return result
    dst = gt_path.parent
    gt = json.loads(gt_path.read_text())

    if regenerate_projects and gt.get("project_generated"):
        result["regenerated_project"] = _regenerate_project(dst)

    drc_path = dst / "raw-drc.json"
    if rerun_drc:
        raw_pcb = b.kicad_path("raw") or (dst / "raw.kicad_pcb")
        if not raw_pcb.exists():
            result["skipped"] = "no raw.kicad_pcb to re-DRC"
            return result
        rc, reason = _kicad_step(
            [cli, "pcb", "drc", "--format", "json", "--units", "mm", "-o", str(drc_path), str(raw_pcb)],
            dst / "import.log")
        if rc != 0:
            result["skipped"] = f"DRC re-run failed: {reason}"
            return result
        result["reran_drc"] = True
    elif not drc_path.exists():
        result["skipped"] = "no raw-drc.json"
        return result

    drc = json.loads(drc_path.read_text())
    errors = [v for v in drc.get("violations", []) if v.get("severity", "error") == "error"]
    drv_all = len(errors)
    drv_routing = sum(1 for v in errors if v.get("type") in ROUTING_DRC_TYPES)
    unconnected = len(drc.get("unconnected_items", []))
    reference_complete = unconnected == 0 and gt.get("wirelength_mm", 0) > 0
    gt.update(drv_routing=drv_routing, drv_all=drv_all, unconnected=unconnected,
              reference_complete=reference_complete)
    gt_path.write_text(json.dumps(gt, indent=2) + "\n")

    result["new_status"], result["new_reason"] = reference_verdict(gt)
    return result


def revalidate(origin: str, progress: Progress | None = None, regenerate_projects: bool = False,
               rerun_drc: bool = False, jobs: int = 1) -> dict:
    """Recompute ``drv_routing``/``drv_all``/``unconnected``/``reference_complete`` for every
    already-imported board of ``origin`` that has a ``ground_truth.json`` on disk, and
    re-apply `reference_verdict` to the manifest -- without re-running the (slow) strip/export/
    stats steps. By default only boards with an existing ``raw-drc.json`` are touched (pure
    Python, no KiCad needed -- safe to run on a host mid routing-run). ``regenerate_projects``
    additionally regenerates each project-generated board's ``.kicad_pro`` from its pristine
    ``raw.orig.kicad_pcb`` backup (see `_regenerate_project`); ``rerun_drc`` additionally
    re-runs ``kicad-cli pcb drc`` against the (possibly just-regenerated) project before
    recomputing -- both need KiCad's ``kicad-cli`` on ``$PATH`` (or ``$FREEROUTING_KICAD_CLI``).
    ``jobs`` parallelises per-board work the same way `import_boards` does; it only matters
    when ``rerun_drc`` is set, since the plain recompute is pure local file I/O.

    Returns a summary dict: ``checked`` (boards of this origin), ``skipped`` (no
    ground_truth.json / no raw-drc.json / DRC re-run failed), ``regenerated_projects``,
    ``reran_drc``, ``ok_to_excluded`` (list of ``(board_id, reason)``), ``excluded_to_ok``
    (list of ``board_id``).
    """
    manifest = {b.id: b for b in corpus.load_manifest()}
    targets = [b for b in manifest.values() if b.origin == origin]

    cli = str(kicad_cli()) if rerun_drc else None

    summary = {"checked": len(targets), "skipped": 0, "regenerated_projects": 0, "reran_drc": 0,
              "ok_to_excluded": [], "excluded_to_ok": []}

    def _fn(b: Board) -> dict:
        return _revalidate_board(b, cli, regenerate_projects, rerun_drc)

    # Same main-thread-only, submission-order `on_done` contract as `import_boards`/
    # `import_kicad_fixtures` -- see `bench.pool.run_cells`'s docstring -- so the manifest is
    # updated/saved here without extra locking even under `jobs > 1`.
    def _on_done(r: dict) -> None:
        if r["skipped"]:
            summary["skipped"] += 1
            if progress:
                progress(f"{r['id']}: skipped ({r['skipped']})")
            return
        if r["regenerated_project"]:
            summary["regenerated_projects"] += 1
        if r["reran_drc"]:
            summary["reran_drc"] += 1
        b = manifest[r["id"]]
        b.status, b.reason = r["new_status"], r["new_reason"]
        corpus.save_manifest(list(manifest.values()))
        if r["new_status"] != r["old_status"]:
            if r["old_status"] == "ok":
                summary["ok_to_excluded"].append((r["id"], r["new_reason"]))
            else:
                summary["excluded_to_ok"].append(r["id"])
            if progress:
                progress(f"{r['id']}: {r['old_status']} -> {r['new_status']}"
                         + (f" ({r['new_reason']})" if r["new_reason"] else ""))

    run_cells(targets, _fn, jobs, on_done=_on_done)
    return summary
