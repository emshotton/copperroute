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
