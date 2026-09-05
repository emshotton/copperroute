"""Independent scoring of a routed cell. Dispatches on board.referee."""
from __future__ import annotations

from pathlib import Path

from bench import metrics
from bench.corpus import Board
from bench.referee import java_drc


def score_cell(board: Board, cell: Path, java_exec: list[str], allow_kicad: bool = True) -> dict:
    """Write referee.json and metrics.json for one cell; return the metrics dict."""
    if board.referee == "kicad" and allow_kicad:
        from bench.referee import kicad  # imported lazily: only needed for PCBench boards
        kicad.run(board, cell)
    else:
        java_drc.run(board, cell, java_exec)
    return metrics.build(cell, board)
