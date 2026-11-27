"""Runs CopperRoute on the board open in KiCad's PCB editor and applies the result as one
undoable change. Settings come from ``copperroute.json`` beside the project, if present."""

from __future__ import annotations

import json
import sys
import time
from pathlib import Path
from typing import Iterable, Optional

DEFAULT_SETTINGS = {
    "maxPasses": 20,
    "timeoutSeconds": 300,
    "optimize": True,
    "respectZones": True,
    "routeTunedNets": False,
    "refillZones": True,
}


def log(message: str) -> None:
    # KiCad 10.0.1 and later show plugin output in the PCB editor's status bar messages.
    print(f"CopperRoute: {message}", flush=True)


def load_settings(project_dir: Optional[Path]) -> dict:
    settings = dict(DEFAULT_SETTINGS)
    if project_dir is not None:
        path = project_dir / "copperroute.json"
        if path.is_file():
            try:
                settings.update(json.loads(path.read_text(encoding="utf-8")))
            except (OSError, ValueError) as error:
                log(f"ignoring {path.name}: {error}")
    return settings


def read_beside(project_dir: Optional[Path], stem: str, suffix: str) -> Optional[str]:
    if project_dir is None:
        return None
    path = project_dir / f"{stem}{suffix}"
    try:
        return path.read_text(encoding="utf-8") if path.is_file() else None
    except OSError as error:
        log(f"could not read {path.name}: {error}")
        return None


def selected_nets(items: Iterable) -> set[str]:
    nets: set[str] = set()
    for item in items:
        net = getattr(item, "net", None)
        if net is not None and getattr(net, "name", ""):
            nets.add(net.name)
        definition = getattr(item, "definition", None)
        for pad in getattr(definition, "pads", []) or []:
            if pad.net.name:
                nets.add(pad.net.name)
    return nets


class ProgressReporter:
    def __init__(self) -> None:
        self.last = 0.0

    def __call__(self, update: dict) -> bool:
        now = time.monotonic()
        if now - self.last >= 2.0:
            self.last = now
            log(f"pass {update['pass']}, {update['incomplete']} connections left")
        return False


def layer_id(name: str):
    from kipy.util.board_layer import layer_from_canonical_name

    return layer_from_canonical_name(name)


def build_items(changes: dict, nets_by_name: dict):
    from kipy.board_types import Track, Via
    from kipy.geometry import Vector2
    from kipy.proto.board.board_types_pb2 import BoardLayer, ViaType
    from kipy.util.units import from_mm

    items = []
    for change in changes["addTracks"]:
        track = Track()
        track.start = Vector2.from_xy_mm(change["start"]["x"], change["start"]["y"])
        track.end = Vector2.from_xy_mm(change["end"]["x"], change["end"]["y"])
        track.width = from_mm(change["width"])
        track.layer = layer_id(change["layer"])
        if change["net"] in nets_by_name:
            track.net = nets_by_name[change["net"]]
        items.append(track)
    for change in changes["addVias"]:
        via = Via()
        via.position = Vector2.from_xy_mm(change["position"]["x"], change["position"]["y"])
        start, end = layer_id(change["startLayer"]), layer_id(change["endLayer"])
        if (start, end) != (BoardLayer.BL_F_Cu, BoardLayer.BL_B_Cu):
            via.type = ViaType.VT_BLIND_BURIED
        via.padstack.drill.start_layer = start
        via.padstack.drill.end_layer = end
        if len(via.padstack.copper_layers) == 0:
            via.padstack.proto.copper_layers.add(layer=BoardLayer.BL_F_Cu)
        via.diameter = from_mm(change["diameter"])
        via.drill_diameter = from_mm(change["drill"])
        if change["net"] in nets_by_name:
            via.net = nets_by_name[change["net"]]
        items.append(via)
    return items


def main(selection_only: Optional[bool], rip_up: bool) -> int:
    try:
        import copperroute_kicad
        from kipy import KiCad
        from kipy.proto.common.types import KIID
    except ImportError as error:
        log(f"a dependency is missing ({error}); reinstall the plugin's environment")
        return 1

    kicad = KiCad(timeout_ms=60_000)
    board = kicad.get_board()
    project_dir = None
    try:
        project_dir = Path(board.get_project().path)
    except Exception:  # noqa: BLE001 - a board opened without a project has none
        pass
    stem = Path(board.name).stem
    settings = load_settings(project_dir)

    request = {
        "boardName": stem,
        "projectText": read_beside(project_dir, stem, ".kicad_pro"),
        "rulesText": read_beside(project_dir, stem, ".kicad_dru"),
        "ripUp": rip_up,
        "maxPasses": int(settings["maxPasses"]),
        "timeoutSeconds": int(settings["timeoutSeconds"]),
        "optimize": bool(settings["optimize"]),
        "respectZones": bool(settings["respectZones"]),
        "routeTunedNets": bool(settings["routeTunedNets"]),
    }
    if selection_only is not False:
        nets = selected_nets(board.get_selection())
        if nets:
            if rip_up:
                request["ripUpNets"] = sorted(nets)
            request["onlyNets"] = sorted(nets)
        elif selection_only:
            log("select tracks, pads or footprints first")
            return 1

    log("routing…")
    try:
        result = copperroute_kicad.route(
            board.get_as_string(), progress=ProgressReporter(), **request
        )
    except RuntimeError as error:
        log(f"the board could not be routed: {error}")
        return 1
    for warning in result["warnings"]:
        log(warning)

    changes = result["changes"]
    nets_by_name = {net.name: net for net in board.get_nets()}
    commit = board.begin_commit()
    try:
        if changes["removeIds"]:
            board.remove_items_by_id([KIID(value=uid) for uid in changes["removeIds"]])
        items = build_items(changes, nets_by_name)
        if items:
            board.create_items(items)
    except Exception as error:  # noqa: BLE001 - any failure must leave the board untouched
        board.drop_commit(commit)
        log(f"the routed copper could not be added, so nothing was changed: {error}")
        return 1
    board.push_commit(commit, "CopperRoute autoroute")
    if settings.get("refillZones") and request["respectZones"]:
        board.refill_zones()

    log(
        f"added {len(changes['addTracks'])} tracks and {len(changes['addVias'])} vias; "
        f"{result['incomplete']} connections left, {result['violations']} DRC findings"
        + (" (stopped at the time limit)" if result["timedOut"] else "")
    )
    return 0


if __name__ == "__main__":
    sys.exit(main(selection_only=False, rip_up=False))
