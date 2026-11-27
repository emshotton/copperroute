"""The CopperRoute autorouter for KiCad boards.

``route`` takes the board as KiCad file text and returns the edits to apply to it.
"""

from __future__ import annotations

import json
from typing import Any, Callable, Optional

from ._native import __version__, route_json

__all__ = ["__version__", "route"]


def route(
    board_text: str,
    *,
    progress: Optional[Callable[[dict], bool]] = None,
    **options: Any,
) -> dict:
    """Route ``board_text`` and return the changes and a summary.

    ``options`` are the fields of the routing request: ``projectText``, ``rulesText``,
    ``ripUp``, ``ripUpNets``, ``onlyNets``, ``respectZones``, ``routeTunedNets``,
    ``maxPasses``, ``timeoutSeconds`` and ``optimize``. ``progress`` receives each update and
    returns True to stop; the copper routed so far is still returned.
    """
    request = dict(options, boardText=board_text)
    callback = None
    if progress is not None:
        callback = lambda text: bool(progress(json.loads(text)))  # noqa: E731
    return json.loads(route_json(json.dumps(request), callback))
