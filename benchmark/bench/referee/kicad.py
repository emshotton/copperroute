"""Referee for PCBench boards: import the SES into the stripped KiCad board, run kicad-cli DRC.

Import uses ``vendor/kicad/ses_to_board.py``, a hand-rolled SES importer -- not
``pcbnew.ImportSpecctraSES``, which returns False with 0 tracks imported when run
headless on KiCad 10.0.3 (verified 2026-08-27; no error surfaced, both call
signatures tried, absolute paths, matching board file name, wx.App created, wx
logging enabled). See README.md "KiCad referee status" for the full spike record.

DRC needs the board's own KiCad project file (``.kicad_pro``): without one,
``kicad-cli pcb drc`` falls back to KiCad's default constraints (e.g. a 0.2mm
minimum track width), which can flag tracks that are perfectly legal under the
board's actual design rules. See _propagate_project_file and README.md.

Before DRC, ``routed.kicad_pcb`` also has its zones re-filled with KiCad's modern
``ZONE_FILLER`` (``vendor/kicad/refill_zones.py``, same as the reference board in
``bench/corpus_pcbench.py``) -- otherwise stale/legacy fills left over from the SES import
would surface as spurious ``clearance`` errors, and pours wouldn't reflect the candidate's
actual routed tracks. Only routing-type DRC violations count toward ``violations``/
``clean_pass`` -- see ROUTING_DRC_TYPES below.
"""
from __future__ import annotations

import json
import shutil
import subprocess
from pathlib import Path

from bench.corpus import Board
from bench.paths import VENDOR_KICAD, ToolMissing, kicad_cli, kicad_python

REFEREE_TIMEOUT_S = 600

# DRC violation types that reflect the *routing* itself -- clearance/width/connectivity
# problems a router could actually cause or fix -- as opposed to checks against artwork,
# footprints, or schematic parity that a router never touches and that PCBench's mostly-
# project-less legacy boards trip constantly as pre-existing noise (see README.md "KiCad
# referee status" and the PCBench section for measured counts, e.g. a 4-port USB hub board
# with 84 total DRC errors and only 3 routing-type ones).
ROUTING_DRC_TYPES = {
    "clearance", "hole_clearance", "hole_near_hole", "track_width", "annular_width",
    "via_diameter", "via_dangling", "track_dangling", "shorting_items", "tracks_crossing",
    "items_not_allowed", "copper_edge_clearance", "copper_sliver", "isolated_copper",
    "connection_width", "drill_out_of_range", "microvia_drill_out_of_range",
    "zones_intersect", "zone_has_empty_net", "starved_thermal", "npth_copper_clearance",
    "padstack", "unconnected_items",
}


def parse_kicad_drc(report: dict) -> dict:
    all_errors = [v for v in report.get("violations", []) if v.get("severity", "error") == "error"]
    routing_errors = [v for v in all_errors if v.get("type") in ROUTING_DRC_TYPES]
    by_type: dict[str, int] = {}
    for v in all_errors:
        by_type[v.get("type", "unknown")] = by_type.get(v.get("type", "unknown"), 0) + 1
    warnings = sum(1 for v in report.get("violations", []) if v.get("severity") == "warning")
    return {"unrouted": len(report.get("unconnected_items", [])), "violations": len(routing_errors),
            "violations_all": len(all_errors), "violations_by_type": by_type, "warnings": warnings}


def _failed(cell: Path, reason: str) -> dict:
    r = {"status": "referee_failed", "referee": "kicad", "reason": reason,
         "unrouted": None, "violations": None, "violations_all": None, "violations_by_type": {},
         "kicad_warnings": None, "vias": None, "wirelength_mm": None, "bends": None,
         "project_used": False, "zones_refilled": None, "import_skipped": [],
         "candidate_output": (cell / "out.ses").exists()}
    (cell / "referee.json").write_text(json.dumps(r, indent=2) + "\n")
    return r


def _run(argv: list[str], cell: Path, log: Path, stdout_path: Path | None = None) -> int:
    """Run argv (cwd=cell), logging the command line to log. stderr always goes to log;
    stdout goes to stdout_path if given, otherwise also to log. Returns -9 on timeout,
    -1 if the executable couldn't even be started (mirrors _run's callers, which only
    care whether the return code is 0)."""
    with open(log, "ab") as f:
        f.write((" ".join(argv) + "\n").encode())
    try:
        with open(log, "ab") as errlog:
            if stdout_path is not None:
                with open(stdout_path, "w") as out:
                    return subprocess.run(argv, cwd=cell, stdout=out, stderr=errlog,
                                           timeout=REFEREE_TIMEOUT_S).returncode
            return subprocess.run(argv, cwd=cell, stdout=errlog, stderr=subprocess.STDOUT,
                                   timeout=REFEREE_TIMEOUT_S).returncode
    except subprocess.TimeoutExpired:
        with open(log, "ab") as f:
            f.write(f"[timed out after {REFEREE_TIMEOUT_S}s]\n".encode())
        return -9
    except OSError as e:
        with open(log, "ab") as f:
            f.write(f"[could not start: {e}]\n".encode())
        return -1


def _propagate_project_file(board: Board, cell: Path, routed: Path) -> bool:
    """Copy the board's .kicad_pro (and sibling .kicad_prl, if any) next to the routed
    board so kicad-cli's DRC uses the board's real design rules instead of KiCad's
    built-in defaults. Returns whether a project file was found and copied.

    Must run AFTER zone refill (see `run`): pcbnew's `board.Save()` inside
    `refill_zones.py` can itself write a same-stem `.kicad_pro`/`.kicad_prl` using its own
    in-memory *default* constraints, which would silently clobber a project copied in first.
    """
    project_path = board.kicad_path("project")
    if not project_path:
        raw = board.kicad_path("raw")
        if raw:
            candidate = raw.with_suffix(".kicad_pro")
            if candidate.exists():
                project_path = candidate
    if not project_path or not project_path.exists():
        return False
    shutil.copyfile(project_path, routed.with_suffix(".kicad_pro"))
    prl_src = project_path.with_suffix(".kicad_prl")
    if prl_src.exists():
        shutil.copyfile(prl_src, routed.with_suffix(".kicad_prl"))
    return True


def _log_project_rules(board: Board, routed: Path, log: Path) -> None:
    """Log routed.kicad_pro's rules.min_hole_clearance, and -- when ground_truth.json says
    the project is a generated one (bench.corpus_pcbench._generate_project), not the board's
    own -- assert it's 0. Regression canary for the zone-refill-clobbers-the-project bug that
    `run`'s refill-before-propagate ordering fixes (see bench/corpus_pcbench.py's own version
    of this check)."""
    project_path = routed.with_suffix(".kicad_pro")
    if not project_path.exists():
        return
    try:
        rules = json.loads(project_path.read_text())["board"]["design_settings"]["rules"]
    except (ValueError, KeyError):
        return
    min_hole = rules.get("min_hole_clearance")
    with open(log, "ab") as f:
        f.write(f"routed.kicad_pro: rules.min_hole_clearance={min_hole}\n".encode())

    generated = False
    gt_path = board.kicad_path("ground_truth")
    if gt_path and gt_path.exists():
        try:
            generated = bool(json.loads(gt_path.read_text()).get("project_generated"))
        except ValueError:
            pass
    if generated:
        assert min_hole == 0, (
            f"routed.kicad_pro: generated project has non-zero min_hole_clearance={min_hole} -- "
            "zone refill's board.Save() may have clobbered it (see run's ordering)"
        )


def run(board: Board, cell: Path) -> dict:
    stripped = board.kicad_path("stripped")
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
    routed = cell / "routed.kicad_pcb"
    import_stdout = cell / "ses-import-stdout.json"
    if _run([str(py), str(VENDOR_KICAD / "ses_to_board.py"), str(stripped), str(ses), str(routed)],
            cell, log, stdout_path=import_stdout) != 0 or not routed.exists():
        return _failed(cell, "SES import into KiCad failed (see referee.log)")

    import_skipped: list = []
    if import_stdout.exists():
        try:
            import_skipped = json.loads(import_stdout.read_text()).get("skipped", [])
        except ValueError:
            pass

    # Re-fill zones against the candidate's actual routed tracks before DRC -- same reason
    # and same script as the reference board's import in bench/corpus_pcbench.py: without
    # this, a routed board's pours are still whatever (or nothing) the stripped board carried,
    # and any legacy fill strategy left over from the source board would flag as spurious
    # `clearance` errors. Guarded like the other steps: a failure here fails the referee run
    # rather than silently DRC-ing a stale/unfilled board. Runs BEFORE the project is
    # propagated -- see _propagate_project_file's docstring for why the order matters.
    zones_refilled = None
    refill_out = cell / "zone-refill-stdout.json"
    if _run([str(py), str(VENDOR_KICAD / "refill_zones.py"), str(routed), str(routed)],
            cell, log, stdout_path=refill_out) != 0:
        return _failed(cell, "refill_zones.py failed (see referee.log)")
    if refill_out.exists():
        try:
            zones_refilled = json.loads(refill_out.read_text()).get("zones")
        except ValueError:
            pass

    project_used = _propagate_project_file(board, cell, routed)
    _log_project_rules(board, routed, log)

    report = cell / "referee-drc.json"
    if _run([str(cli), "pcb", "drc", "--format", "json", "--all-track-errors", "--units", "mm",
             "-o", str(report), str(routed)], cell, log) != 0 or not report.exists():
        return _failed(cell, "kicad-cli drc failed (see referee.log)")

    stats_out = cell / "referee-stats.json"
    if _run([str(py), str(VENDOR_KICAD / "board_stats.py"), str(routed)], cell, log,
             stdout_path=stats_out) != 0:
        return _failed(cell, "board_stats.py failed (see referee.log)")

    try:
        drc = parse_kicad_drc(json.loads(report.read_text()))
        stats = json.loads(stats_out.read_text())
    except (ValueError, KeyError) as e:
        return _failed(cell, f"could not parse kicad-cli output: {e}")

    r = {"status": "ok", "referee": "kicad", "reason": "",
         "unrouted": drc["unrouted"], "violations": drc["violations"],
         "violations_all": drc["violations_all"], "violations_by_type": drc["violations_by_type"],
         "kicad_warnings": drc["warnings"],
         "vias": stats["vias"], "wirelength_mm": stats["wirelength_mm"], "bends": None,
         "project_used": project_used, "zones_refilled": zones_refilled,
         "import_skipped": import_skipped}
    (cell / "referee.json").write_text(json.dumps(r, indent=2) + "\n")
    return r
