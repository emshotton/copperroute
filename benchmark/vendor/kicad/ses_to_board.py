"""Import a Specctra SES into a KiCad PCB without pcbnew.ImportSpecctraSES.

pcbnew.ImportSpecctraSES returns False with 0 tracks imported on KiCad 10.0.3 when
run headless (both call signatures, absolute paths, matching board file name, a
wx.App created, wx logging enabled) -- verified 2026-08-27, no error surfaced. So
this script parses the SES itself and adds tracks/vias to the board directly via
the pcbnew BOARD API.

``parse_ses_routes`` has no pcbnew/wx dependency, so it can be imported and unit
tested with the ordinary bench Python (see tests/test_ses_to_board.py). Only
``main()`` needs KiCad's bundled Python (``pcbnew`` on sys.path):

    KP=/Applications/KiCad/KiCad.app/Contents/Frameworks/Python.framework/Versions/Current/bin/python3
    $KP vendor/kicad/ses_to_board.py stripped.kicad_pcb routed.ses routed.kicad_pcb

Prints ``{"wires": n, "vias": n, "skipped": [...]}`` to stdout on success (exit 0).
Exits 1 with a message on stderr for a parse failure or an unresolvable board layer.

SES/DSN conventions this relies on (confirmed against fixtures/Issue558 dev-board,
freerouting output, KiCad 10.0.3 export):

- ``(resolution <unit> <n>)`` in the SES: nm-per-SES-unit = {um: 1000, mm: 1e6,
  mil: 25400, inch: 25.4e6}[unit] / n. Board x_nm = value * nm_per_unit; board
  y_nm = -value * nm_per_unit (KiCad's internal Y grows downward, Specctra's
  grows upward).
- Routes live in ``(routes (network_out (net NAME (wire (path LAYER WIDTH x1 y1
  x2 y2 ...)) (via "PADSTACK" x y) ...) ...))``. Layer names match board layer
  names (F.Cu, B.Cu, In1.Cu, ...); resolve with ``board.GetLayerID(name)``. Nets
  are matched by name via ``board.FindNet(name)``.
- Via padstacks are named ``Via[<from>-<to>]_<diam>:<drill>_um`` (diameter/drill
  in micrometres) -- that's the primary, most reliable source of via geometry.
  Fall back to the ``(library_out (padstack NAME (shape (circle LAYER DIAM 0
  0)) ...)))`` diameter (in SES units) with drill = diameter / 2 when the name
  doesn't match. A via is through-hole when its layer span is 0..(N-1) for an
  N-copper-layer board; otherwise it's blind/buried.
"""
from __future__ import annotations

import json
import re
import sys
from pathlib import Path

# nm per "resolution unit" (i.e. per 1/n of <unit>), keyed by <unit>.
_UNIT_TO_NM_PER_UNIT_BASE = {"um": 1000.0, "mm": 1_000_000.0, "mil": 25400.0, "inch": 25_400_000.0}

_VIA_PADSTACK_RE = re.compile(r"Via\[(\d+)-(\d+)\]_(\d+):(\d+)_um")


class SesParseError(ValueError):
    pass


# Specctra SES is a plain S-expression, so a minimal tokenizer/parser suffices.

def _tokenize(text: str) -> list[str]:
    tokens: list[str] = []
    i, n = 0, len(text)
    while i < n:
        c = text[i]
        if c.isspace():
            i += 1
            continue
        if c in "()":
            tokens.append(c)
            i += 1
            continue
        if c == '"':
            j = i + 1
            while j < n and text[j] != '"':
                j += 1
            if j >= n:
                raise SesParseError(f"unterminated quoted string starting at offset {i}")
            tokens.append(text[i + 1:j])  # unquoted already
            i = j + 1
            continue
        j = i
        while j < n and not text[j].isspace() and text[j] not in "()":
            j += 1
        tokens.append(text[i:j])
        i = j
    return tokens


def _parse_form(tokens: list[str], i: int) -> tuple[list, int]:
    """Parse one parenthesised form starting at tokens[i] == '('. Returns (node, next_i)."""
    if i >= len(tokens) or tokens[i] != "(":
        raise SesParseError(f"expected '(' at token {i}")
    i += 1
    node: list = []
    while True:
        if i >= len(tokens):
            raise SesParseError("unexpected end of input (unbalanced parentheses)")
        if tokens[i] == ")":
            return node, i + 1
        if tokens[i] == "(":
            child, i = _parse_form(tokens, i)
            node.append(child)
        else:
            node.append(tokens[i])
            i += 1


def _find_first(node: list, head: str) -> list | None:
    for child in node[1:]:
        if isinstance(child, list) and child and child[0] == head:
            return child
    return None


def _find_all(node: list, head: str) -> list[list]:
    return [c for c in node[1:] if isinstance(c, list) and c and c[0] == head]


def parse_ses_routes(text: str) -> dict:
    """Parse the ``(routes ...)`` section of a Specctra SES.

    Returns::

        {"resolution": (unit, n),
         "nets": {name: {"wires": [(layer, width, [(x, y), ...]), ...],
                          "vias": [(padstack, x, y), ...]}},
         "padstacks": {name: diameter}}

    All coordinates/widths/diameters are in raw SES units (not yet scaled to nm).
    """
    tokens = _tokenize(text)
    if not tokens or tokens[0] != "(":
        raise SesParseError("SES text does not start with '('")
    root, _ = _parse_form(tokens, 0)

    routes = _find_first(root, "routes")
    if routes is None:
        raise SesParseError("no (routes ...) section in SES")

    res_node = _find_first(routes, "resolution")
    if res_node is None or len(res_node) < 3:
        raise SesParseError("no (resolution <unit> <n>) in (routes ...)")
    unit, n = res_node[1], int(res_node[2])

    padstacks: dict[str, float] = {}
    lib = _find_first(routes, "library_out")
    if lib is not None:
        for ps in _find_all(lib, "padstack"):
            if len(ps) < 2:
                continue
            name = ps[1]
            shape = _find_first(ps, "shape")
            circle = _find_first(shape, "circle") if shape is not None else None
            if circle is not None and len(circle) > 2:
                padstacks[name] = float(circle[2])

    nets: dict[str, dict] = {}
    net_out = _find_first(routes, "network_out")
    if net_out is not None:
        for net_node in _find_all(net_out, "net"):
            if len(net_node) < 2:
                continue
            name = net_node[1]
            wires: list[tuple[str, float, list[tuple[float, float]]]] = []
            for wire_node in _find_all(net_node, "wire"):
                for path in _find_all(wire_node, "path"):
                    if len(path) < 3:
                        continue
                    layer, width = path[1], float(path[2])
                    coords = [float(v) for v in path[3:]]
                    pts = list(zip(coords[0::2], coords[1::2]))
                    wires.append((layer, width, pts))
            vias: list[tuple[str, float, float]] = []
            for via_node in _find_all(net_node, "via"):
                if len(via_node) < 4:
                    continue
                vias.append((via_node[1], float(via_node[2]), float(via_node[3])))
            nets[name] = {"wires": wires, "vias": vias}

    return {"resolution": (unit, n), "nets": nets, "padstacks": padstacks}


def _via_geometry(padstack_name: str, padstacks: dict[str, float], nm_per_unit: float
                   ) -> tuple[int, int, int | None, int | None]:
    """(diameter_nm, drill_nm, layer_from, layer_to) -- layer_from/to are None if unknown."""
    m = _VIA_PADSTACK_RE.search(padstack_name)
    if m:
        layer_from, layer_to = int(m.group(1)), int(m.group(2))
        diam_um, drill_um = int(m.group(3)), int(m.group(4))
        return diam_um * 1000, drill_um * 1000, layer_from, layer_to
    diam_units = padstacks.get(padstack_name)
    if diam_units is None:
        diam_nm = round(0.6 * 1_000_000)  # 0.6mm fallback
        return diam_nm, diam_nm // 2, None, None
    diam_nm = round(diam_units * nm_per_unit)
    return diam_nm, diam_nm // 2, None, None


def _copper_layer_order(pcbnew, board) -> list:
    """Copper layers in Specctra layer-index order: F_Cu, In1_Cu, ..., B_Cu."""
    count = board.GetCopperLayerCount()
    if count <= 1:
        return [pcbnew.F_Cu]
    order = [pcbnew.F_Cu]
    for i in range(1, count - 1):
        order.append(getattr(pcbnew, f"In{i}_Cu"))
    order.append(pcbnew.B_Cu)
    return order


def main() -> int:
    import argparse

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("pcb", type=Path, help="Base .kicad_pcb to import routing into")
    parser.add_argument("ses", type=Path, help="Specctra session file")
    parser.add_argument("output", type=Path, help="Written .kicad_pcb after import")
    args = parser.parse_args()

    try:
        import wx  # type: ignore
        wx.App(False)
        wx.Log.EnableLogging(False)
    except Exception:
        pass

    import pcbnew  # type: ignore

    try:
        parsed = parse_ses_routes(args.ses.read_text(encoding="utf-8", errors="replace"))
    except SesParseError as e:
        print(f"failed to parse SES {args.ses}: {e}", file=sys.stderr)
        return 1

    unit, n = parsed["resolution"]
    base = _UNIT_TO_NM_PER_UNIT_BASE.get(unit)
    if base is None:
        print(f"unknown SES resolution unit: {unit!r}", file=sys.stderr)
        return 1
    nm_per_unit = base / n

    board = pcbnew.LoadBoard(str(args.pcb.resolve()))
    copper_order = _copper_layer_order(pcbnew, board)
    n_copper = len(copper_order)

    skipped: list[str] = []
    wires_added = 0
    vias_added = 0

    for net_name, data in parsed["nets"].items():
        net = board.FindNet(net_name)
        if net is None:
            skipped.append(f"net not found: {net_name}")
            continue
        for layer_name, width, pts in data["wires"]:
            layer_id = board.GetLayerID(layer_name)
            if layer_id < 0:
                print(f"unknown board layer {layer_name!r} (net {net_name})", file=sys.stderr)
                return 1
            width_nm = round(width * nm_per_unit)
            for (x0, y0), (x1, y1) in zip(pts, pts[1:]):
                t = pcbnew.PCB_TRACK(board)
                t.SetStart(pcbnew.VECTOR2I(round(x0 * nm_per_unit), round(-y0 * nm_per_unit)))
                t.SetEnd(pcbnew.VECTOR2I(round(x1 * nm_per_unit), round(-y1 * nm_per_unit)))
                t.SetWidth(width_nm)
                t.SetLayer(layer_id)
                t.SetNet(net)
                board.Add(t)
                wires_added += 1
        for padstack_name, x, y in data["vias"]:
            diam_nm, drill_nm, layer_from, layer_to = _via_geometry(padstack_name, parsed["padstacks"], nm_per_unit)
            v = pcbnew.PCB_VIA(board)
            v.SetPosition(pcbnew.VECTOR2I(round(x * nm_per_unit), round(-y * nm_per_unit)))
            v.SetWidth(diam_nm)
            v.SetDrill(drill_nm)
            v.SetNet(net)
            is_through = layer_from is None or layer_to is None or (layer_from == 0 and layer_to == n_copper - 1)
            if is_through:
                v.SetViaType(pcbnew.VIATYPE_THROUGH)
                v.SetLayerPair(pcbnew.F_Cu, pcbnew.B_Cu)
            else:
                v.SetViaType(pcbnew.VIATYPE_BLIND_BURIED)
                v.SetLayerPair(copper_order[layer_from], copper_order[layer_to])
            board.Add(v)
            vias_added += 1

    args.output.parent.mkdir(parents=True, exist_ok=True)
    board.Save(str(args.output))
    print(json.dumps({"wires": wires_added, "vias": vias_added, "skipped": skipped}))
    return 0


if __name__ == "__main__":
    sys.exit(main())
