"""Independent KiCad scoring of a routed cell."""
from __future__ import annotations

from pathlib import Path

from bench import metrics
from bench.corpus import Board


def score_cell(board: Board, cell: Path) -> dict:
    """Write KiCad referee.json and metrics.json for one cell."""
    if board.referee != "kicad":
        raise ValueError(f"KiCad reference unavailable for: {board.id}")
    from bench.referee import kicad
    kicad.run(board, cell)
    return metrics.build(cell, board)


def identity_key(identity: dict | None):
    if identity is None:
        return None
    return identity.get("jar_sha256") or identity.get("exec") or identity
