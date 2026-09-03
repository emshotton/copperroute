"""Pure (no pcbnew) tests for the SES parser in vendor/kicad/ses_to_board.py.

Loaded by path with importlib since vendor/kicad isn't a package on the bench's
import path (and is meant to run standalone under KiCad's bundled Python).
"""
from __future__ import annotations

import importlib.util
from pathlib import Path

VENDOR_KICAD = Path(__file__).parent.parent / "vendor" / "kicad"
SPIKE = Path(__file__).parent / "data" / "spike"


def _load_ses_to_board():
    spec = importlib.util.spec_from_file_location("ses_to_board", VENDOR_KICAD / "ses_to_board.py")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def test_module_does_not_import_pcbnew_at_module_level():
    # If pcbnew (or wx) were imported at module scope this would already have
    # failed above under plain Python; assert main() is what pulls it in.
    mod = _load_ses_to_board()
    assert "import pcbnew" in Path(VENDOR_KICAD / "ses_to_board.py").read_text()
    assert callable(mod.main)


def test_parse_ses_routes_on_spike_fixture():
    mod = _load_ses_to_board()
    text = (SPIKE / "routed.ses").read_text()
    parsed = mod.parse_ses_routes(text)

    assert parsed["resolution"] == ("um", 10)

    total_vias = sum(len(d["vias"]) for d in parsed["nets"].values())
    assert total_vias == 35

    for data in parsed["nets"].values():
        for layer, width, pts in data["wires"]:
            assert layer in ("F.Cu", "B.Cu")
            assert width > 0
            assert len(pts) >= 2


def test_parse_ses_routes_rejects_garbage():
    mod = _load_ses_to_board()
    try:
        mod.parse_ses_routes("not an s-expression")
        assert False, "expected SesParseError"
    except mod.SesParseError:
        pass


def test_via_geometry_from_padstack_name():
    mod = _load_ses_to_board()
    diam_nm, drill_nm, layer_from, layer_to = mod._via_geometry("Via[0-1]_600:300_um", {}, 100.0)
    assert diam_nm == 600_000
    assert drill_nm == 300_000
    assert (layer_from, layer_to) == (0, 1)


def test_via_geometry_fallback_to_padstack_diameter():
    mod = _load_ses_to_board()
    diam_nm, drill_nm, layer_from, layer_to = mod._via_geometry("weird-name", {"weird-name": 6000.0}, 100.0)
    assert diam_nm == 600_000
    assert drill_nm == 300_000
    assert layer_from is None and layer_to is None
