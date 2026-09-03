"""Generate a `.kicad_pro` project from a legacy (KiCad 4/5-era) `.kicad_pcb`'s own
``(net_class ...)`` blocks and ``(setup ...)`` board-wide floors.

PCBench boards are mostly pre-`.kicad_pro` era: running `kicad-cli pcb drc` on them without
a project falls back to KiCad's *default* board-setup constraints (0.25mm hole clearance,
0.5mm copper-edge clearance, 0.2mm min track width, ...), which these designs never had, so
DRC flags thousands of errors that are artefacts of the missing project rather than real
design problems. `(net_class NAME "desc" (clearance x) (trace_width y) (via_dia ...)
(via_drill ...) (uvia_dia ...) (uvia_drill ...) (add_net "n") ...)` blocks in the legacy
`.kicad_pcb` itself carry the board's real per-net clearance/track/via rules -- this module
parses those and builds an equivalent `.kicad_pro` with one `net_settings.classes` entry per
legacy net class (`netclass_patterns` reconstructed from each class's `add_net` members).

The legacy `(setup ...)` block can also carry board-wide DRC *floors* -- `(trace_min X)`,
`(via_min_size X)`, `(via_min_drill X)`, `(uvia_min_size X)`, `(uvia_min_drill X)`, and
occasionally `(clearance_min X)`/`(trace_clearance X)` -- which `parse_legacy_setup` maps onto
the matching modern `.kicad_pro` `rules.min_*` name (e.g. `trace_min` -> `min_track_width`).
Legacy floors are carried over; rules legacy KiCad never had (hole clearance, hole-to-hole,
copper-edge, annular width, silk, text, solder mask) are 0, since there is no legacy field
that ever constrained them.

**Floors are rounded DOWN to the nearest micrometre before being written into `rules.min_*`**
(`_round_down_um`, applied in `build_project`). Legacy `(setup ...)` values are frequently
mil-derived and carry sub-micrometre precision (e.g. `0.3302` mm = 13 mil exactly), but
`vendor/kicad/export_specctra_dsn.py` (via `pcbnew`) writes via/pad padstack sizes in the DSN
it exports rounded to whole micrometres -- so a routed board can never actually produce a via
at `0.3302`mm, only `0.330`mm or `0.331`mm. Left unrounded, such a floor is unsatisfiable by
construction and DRC reports thousands of `drill_out_of_range`/`via_diameter` violations of
0.2-0.8 um against every via/drill at the board's own (legacy) minimum size. Rounding down
(not to nearest) means a routed via that lands exactly on the exporter-rounded floor still
clears the rule; rounding to nearest could instead round the floor *up* past what the
exporter can ever produce, reintroducing the same problem from the other direction. Both the
rounded floors actually written into `rules.min_*` and the original, unrounded values parsed
from the board are recorded under `_bench` (`"setup_floors"` / `"setup_floors_raw"`
respectively) for traceability.

Pure Python, no `pcbnew` -- importable directly by the bench test suite and runnable outside
a KiCad install (unlike `refill_zones.py`, which needs KiCad's bundled Python).

CLI:

    python3 vendor/kicad/legacy_rules.py <board.kicad_pcb> <out.kicad_pro>

Does nothing (prints a "skipped" line, writes no file) if `board.kicad_pcb` already has a
sibling `.kicad_pro` -- i.e. a modern board that ships its own project needs no generated
one. A legacy board with no `(net_class` blocks at all still gets a project, built from a
single synthetic "Default" class using KiCad's own historical defaults (0.2mm clearance,
0.25mm track width) with the same board-setup floors (see above).

The generated project's own provenance -- that it was bench-generated, and which setup floors
were actually found and carried over -- is recorded under a top-level `"_bench"` key
(`{"generated": true, "setup_floors": {...}}`); verified (2026-08-27, `kicad-cli` 10.0.3) that
`kicad-cli pcb drc` ignores unknown top-level `.kicad_pro` keys -- a project with `"_bench"`
added produces byte-identical DRC violations to the same project without it.

See README.md "KiCad referee status" and the PCBench section for the before/after DRC-error
counts this fixes (e.g. 1Bitsy: 257 -> 46, all remaining non-routing).
"""
from __future__ import annotations

import json
import math
import re
import sys
from pathlib import Path

DEFAULT_CLEARANCE_MM = 0.2
DEFAULT_TRACK_WIDTH_MM = 0.25

# Board-setup ("Constraints" tab) minimums a modern `.kicad_pro` carries. A legacy board's
# per-net limits live in its net classes (parsed below) and its board-wide floors (if any) in
# its `(setup ...)` block (see `parse_legacy_setup`) -- every other floor here has no legacy
# equivalent at all (e.g. copper-to-edge clearance) and so stays 0, otherwise KiCad would
# silently re-impose its own non-zero defaults for it.
_ZEROED_RULE_KEYS = [
    "min_clearance", "min_connection", "min_copper_edge_clearance", "min_hole_clearance",
    "min_hole_to_hole", "min_microvia_diameter", "min_microvia_drill", "min_silk_clearance",
    "min_text_height", "min_text_thickness", "min_through_hole_diameter", "min_track_width",
    "min_via_annular_width", "min_via_diameter", "solder_mask_to_copper_clearance",
]

# Legacy `(setup ...)` field name -> modern `.kicad_pro` `rules.min_*` name it becomes a floor
# for. `clearance_min` and `trace_clearance` are alternate spellings for the same board-wide
# clearance floor seen across KiCad 4/5 files; if a board somehow has both, `clearance_min` (the
# more common one) wins -- see `parse_legacy_setup`.
_SETUP_FLOOR_FIELDS = [
    ("trace_min", "min_track_width"),
    ("via_min_size", "min_via_diameter"),
    ("via_min_drill", "min_through_hole_diameter"),
    ("uvia_min_size", "min_microvia_diameter"),
    ("uvia_min_drill", "min_microvia_drill"),
    ("clearance_min", "min_clearance"),
    ("trace_clearance", "min_clearance"),
]

_NET_CLASS_START_RE = re.compile(r"\(net_class\b")
_SETUP_START_RE = re.compile(r"\(setup\b")
_HEAD_RE = re.compile(r'\(net_class\s+("(?:[^"\\]|\\.)*"|\S+)\s*("(?:[^"\\]|\\.)*")?', re.S)
_FIELD_RE = lambda key: re.compile(r"\(" + re.escape(key) + r"\s+(-?[\d.]+(?:[eE][+-]?\d+)?)\)")
_ADD_NET_RE = re.compile(r'\(add_net\s+("(?:[^"\\]|\\.)*"|\S+)\)')


def _unquote(s: str | None) -> str | None:
    if s is None:
        return None
    if len(s) >= 2 and s[0] == '"' and s[-1] == '"':
        return s[1:-1]
    return s


def _balanced_end(text: str, start: int) -> int:
    """Index of the ``)`` that closes the ``(`` at ``text[start]``."""
    depth = 0
    i = start
    n = len(text)
    while i < n:
        c = text[i]
        if c == "(":
            depth += 1
        elif c == ")":
            depth -= 1
            if depth == 0:
                return i
        i += 1
    raise ValueError(f"unbalanced net_class block starting at offset {start}")


def _find_net_class_blocks(text: str) -> list[str]:
    blocks = []
    for m in _NET_CLASS_START_RE.finditer(text):
        start = m.start()
        end = _balanced_end(text, start)
        blocks.append(text[start : end + 1])
    return blocks


def _find_setup_block(text: str) -> str | None:
    m = _SETUP_START_RE.search(text)
    if not m:
        return None
    end = _balanced_end(text, m.start())
    return text[m.start() : end + 1]


def _default_class(nets: list[str] | None = None) -> dict:
    return {
        "name": "Default",
        "clearance": DEFAULT_CLEARANCE_MM,
        "track_width": DEFAULT_TRACK_WIDTH_MM,
        "via_diameter": 0.6,
        "via_drill": 0.4,
        "microvia_diameter": 0.3,
        "microvia_drill": 0.1,
        "diff_pair_gap": 0.25,
        "diff_pair_width": 0.2,
        "diff_pair_via_gap": 0.25,
        "bus_width": 12,
        "line_style": 0,
        "pcb_color": "rgba(0, 0, 0, 0.000)",
        "schematic_color": "rgba(0, 0, 0, 0.000)",
        "wire_width": 6,
        "nets": nets or [],
    }


def parse_legacy_netclasses(text: str) -> list[dict]:
    """Parse every ``(net_class ...)`` block in ``text`` (a `.kicad_pcb`'s contents).

    Returns one dict per class, matching a `.kicad_pro` ``net_settings.classes`` entry plus
    an extra ``"nets"`` key (the class's ``add_net`` members, for `build_project` to turn
    into ``netclass_patterns``) that `build_project` strips before writing JSON. A board with
    no ``(net_class`` blocks at all gets a single synthetic Default class with KiCad's
    historical defaults (0.2mm clearance, 0.25mm track width).
    """
    classes = []
    for block in _find_net_class_blocks(text):
        m = _HEAD_RE.match(block)
        if not m:
            continue
        name = _unquote(m.group(1)) or "Default"
        body = block[m.end() :]
        classes.append({
            "name": name,
            "clearance": float(_FIELD_RE("clearance").search(body).group(1)) if _FIELD_RE("clearance").search(body) else DEFAULT_CLEARANCE_MM,
            "track_width": float(_FIELD_RE("trace_width").search(body).group(1)) if _FIELD_RE("trace_width").search(body) else DEFAULT_TRACK_WIDTH_MM,
            "via_diameter": float(_FIELD_RE("via_dia").search(body).group(1)) if _FIELD_RE("via_dia").search(body) else 0.6,
            "via_drill": float(_FIELD_RE("via_drill").search(body).group(1)) if _FIELD_RE("via_drill").search(body) else 0.4,
            "microvia_diameter": float(_FIELD_RE("uvia_dia").search(body).group(1)) if _FIELD_RE("uvia_dia").search(body) else 0.3,
            "microvia_drill": float(_FIELD_RE("uvia_drill").search(body).group(1)) if _FIELD_RE("uvia_drill").search(body) else 0.1,
            "diff_pair_gap": 0.25,
            "diff_pair_width": 0.2,
            "diff_pair_via_gap": 0.25,
            "bus_width": 12,
            "line_style": 0,
            "pcb_color": "rgba(0, 0, 0, 0.000)",
            "schematic_color": "rgba(0, 0, 0, 0.000)",
            "wire_width": 6,
            "nets": [_unquote(n) for n in _ADD_NET_RE.findall(body)],
        })
    if not classes:
        classes.append(_default_class())
    return classes


def parse_legacy_setup(text: str) -> dict[str, float]:
    """Parse a legacy `.kicad_pcb`'s ``(setup ...)`` block for the board-wide DRC floors
    KiCad 4/5 stored there: ``(trace_min X)``, ``(via_min_size X)``, ``(via_min_drill X)``,
    ``(uvia_min_size X)``, ``(uvia_min_drill X)``, and occasionally ``(clearance_min X)`` or
    ``(trace_clearance X)`` (see `_SETUP_FLOOR_FIELDS`).

    Returns a dict keyed by the modern `.kicad_pro` ``rules.min_*`` name each legacy field
    maps to (e.g. ``trace_min`` -> ``min_track_width``), containing only the floors actually
    present in the text -- a board with no ``(setup ...)`` block, or none of these fields in
    it, returns ``{}``. `build_project` treats any rule missing from this dict as 0: legacy
    KiCad had no floor for it at all (hole clearance, hole-to-hole, copper-edge, annular
    width, silk, text, solder mask), so there is nothing to carry over.
    """
    block = _find_setup_block(text)
    if not block:
        return {}
    floors: dict[str, float] = {}
    for legacy_key, rule_key in _SETUP_FLOOR_FIELDS:
        if rule_key in floors:
            continue  # clearance_min already matched; ignore the trace_clearance alias
        m = _FIELD_RE(legacy_key).search(block)
        if m:
            floors[rule_key] = float(m.group(1))
    return floors


def _round_down_um(value: float) -> float:
    """Round a legacy floor (mm) DOWN to the nearest whole micrometre.

    `vendor/kicad/export_specctra_dsn.py` (via `pcbnew`) exports via/pad padstack sizes
    rounded to whole micrometres, so a sub-micrometre-precision legacy floor (e.g. `0.3302`mm,
    13 mil exactly) can never actually be met by anything a router outputs. Rounding down
    (never up) means a routed via that lands exactly on the exporter-rounded floor still
    clears the rule -- see module docstring for the full rationale.
    """
    return math.floor(value * 1000) / 1000


def build_project(classes: list[dict], filename: str, setup_floors: dict[str, float] | None = None) -> dict:
    """A `.kicad_pro`-shaped dict from `parse_legacy_netclasses`' output: one
    ``net_settings.classes`` entry per legacy class (minus its `nets` key), one
    ``netclass_patterns`` entry per non-Default class's net (Default needs no pattern -- it's
    KiCad's fallback for anything unmatched), and board-setup `rules.min_*` floors carried
    over from `setup_floors` (`parse_legacy_setup`'s output, rounded DOWN to the nearest
    micrometre by `_round_down_um` -- see module docstring) where present -- every other
    floor is 0 (see `_ZEROED_RULE_KEYS`), since legacy KiCad never had that constraint.

    Provenance is recorded under a top-level ``"_bench"`` key (``kicad-cli pcb drc`` ignores
    unknown top-level `.kicad_pro` keys -- verified 2026-08-27 against `kicad-cli` 10.0.3;
    see module docstring) so a generated project's carried-over floors are visible without
    reaching into `ground_truth.json`: `"setup_floors"` is what was actually written into
    `rules.min_*` (rounded), `"setup_floors_raw"` is what was parsed from the board (not).
    """
    patterns = [{"netclass": c["name"], "pattern": n}
                for c in classes if c["name"] != "Default" for n in c.get("nets", [])]
    out_classes = [{k: v for k, v in c.items() if k != "nets"} for c in classes]
    rules = {k: 0.0 for k in _ZEROED_RULE_KEYS}
    setup_floors_raw = setup_floors or {}
    setup_floors = {k: _round_down_um(v) for k, v in setup_floors_raw.items()}
    for rule_key, value in setup_floors.items():
        if rule_key in rules:
            rules[rule_key] = value
    rules["min_resolved_spokes"] = 0
    rules["use_height_for_length_calcs"] = True
    return {
        "_bench": {"generated": True, "setup_floors": setup_floors, "setup_floors_raw": setup_floors_raw},
        "board": {"design_settings": {"rules": rules}},
        "meta": {"filename": filename, "version": 1},
        "net_settings": {
            "classes": out_classes,
            "meta": {"version": 4},
            "net_colors": None,
            "netclass_assignments": None,
            "netclass_patterns": patterns,
        },
    }


def main(argv: list[str] | None = None) -> int:
    argv = sys.argv[1:] if argv is None else argv
    if len(argv) != 2:
        print("usage: legacy_rules.py <board.kicad_pcb> <out.kicad_pro>", file=sys.stderr)
        return 2
    board_path, out_path = Path(argv[0]), Path(argv[1])
    sibling = board_path.with_suffix(".kicad_pro")
    if sibling.exists():
        print(json.dumps({"skipped": "sibling .kicad_pro already exists", "path": str(sibling)}))
        return 0
    text = board_path.read_text(encoding="utf-8", errors="replace")
    classes = parse_legacy_netclasses(text)
    setup_floors = parse_legacy_setup(text)
    project = build_project(classes, out_path.name, setup_floors)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(project, indent=2) + "\n")
    print(json.dumps({"classes": len(classes), "patterns": len(project["net_settings"]["netclass_patterns"])}))
    return 0


if __name__ == "__main__":
    sys.exit(main())
