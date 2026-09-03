"""Tests for vendor/kicad/legacy_rules.py: pure-Python, no pcbnew, loaded by path like
vendor/kicad/ses_to_board.py's own tests (vendor/kicad isn't an importable package)."""
from __future__ import annotations

import importlib.util
import json
from pathlib import Path

VENDOR_KICAD = Path(__file__).parent.parent / "vendor" / "kicad"


def _load_legacy_rules():
    spec = importlib.util.spec_from_file_location("legacy_rules", VENDOR_KICAD / "legacy_rules.py")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


legacy_rules = _load_legacy_rules()

TWO_CLASSES = """
(kicad_pcb
  (net_class Default "This is the default net class."
    (clearance 0.149)
    (trace_width 0.15)
    (via_dia 0.45)
    (via_drill 0.25)
    (uvia_dia 0.3)
    (uvia_drill 0.1)
    (add_net +3V3)
    (add_net GND)
  )
  (net_class "USB Differential" ""
    (clearance 0.2)
    (trace_width 0.3)
    (via_dia 0.9)
    (via_drill 0.4)
    (uvia_dia 0.3)
    (uvia_drill 0.1)
    (add_net "USB_DP")
    (add_net "USB_DM")
  )
)
"""


def test_parse_legacy_netclasses_default_and_named_class():
    classes = legacy_rules.parse_legacy_netclasses(TWO_CLASSES)
    assert [c["name"] for c in classes] == ["Default", "USB Differential"]

    default = classes[0]
    assert default["clearance"] == 0.149
    assert default["track_width"] == 0.15
    assert default["via_diameter"] == 0.45
    assert default["via_drill"] == 0.25
    assert default["microvia_diameter"] == 0.3
    assert default["microvia_drill"] == 0.1
    assert default["nets"] == ["+3V3", "GND"]

    usb = classes[1]
    assert usb["clearance"] == 0.2
    assert usb["track_width"] == 0.3
    assert usb["nets"] == ["USB_DP", "USB_DM"]


def test_build_project_generates_patterns_for_non_default_classes_only():
    classes = legacy_rules.parse_legacy_netclasses(TWO_CLASSES)
    project = legacy_rules.build_project(classes, "board.kicad_pro")
    patterns = project["net_settings"]["netclass_patterns"]
    # Default needs no pattern (it's KiCad's fallback for unmatched nets); USB Differential's
    # two add_net members each become a pattern.
    assert patterns == [
        {"netclass": "USB Differential", "pattern": "USB_DP"},
        {"netclass": "USB Differential", "pattern": "USB_DM"},
    ]
    # "nets" is bench-internal (used to build patterns) and must not leak into the
    # .kicad_pro-shaped classes list.
    assert all("nets" not in c for c in project["net_settings"]["classes"])


def test_parse_legacy_netclasses_no_net_class_blocks_uses_kicad_defaults():
    classes = legacy_rules.parse_legacy_netclasses("(kicad_pcb (version 4) (host pcbnew))")
    assert len(classes) == 1
    assert classes[0]["name"] == "Default"
    assert classes[0]["clearance"] == 0.2
    assert classes[0]["track_width"] == 0.25
    assert classes[0]["nets"] == []


def test_build_project_json_shape():
    classes = legacy_rules.parse_legacy_netclasses(TWO_CLASSES)
    project = legacy_rules.build_project(classes, "board.kicad_pro")

    # Round-trips through JSON cleanly (this is what gets written to a .kicad_pro).
    project = json.loads(json.dumps(project))

    assert project["net_settings"]["classes"][0]["clearance"] == 0.149
    assert project["meta"]["filename"] == "board.kicad_pro"
    rules = project["board"]["design_settings"]["rules"]
    # Board-setup floors are all zeroed so only the net classes' own values apply -- a
    # legacy board never had these constraints.
    assert rules["min_track_width"] == 0.0
    assert rules["min_clearance"] == 0.0
    assert rules["min_hole_clearance"] == 0.0
    assert rules["min_copper_edge_clearance"] == 0.0


def test_build_project_defaults_case_json_shape():
    classes = legacy_rules.parse_legacy_netclasses("(kicad_pcb)")
    project = legacy_rules.build_project(classes, "board.kicad_pro")
    assert project["net_settings"]["classes"][0]["clearance"] == 0.2
    assert project["net_settings"]["classes"][0]["track_width"] == 0.25
    assert project["net_settings"]["netclass_patterns"] == []


def test_cli_skips_when_sibling_project_exists(tmp_path):
    board = tmp_path / "board.kicad_pcb"
    board.write_text(TWO_CLASSES)
    (tmp_path / "board.kicad_pro").write_text("{}")
    out = tmp_path / "out.kicad_pro"

    rc = legacy_rules.main([str(board), str(out)])
    assert rc == 0
    assert not out.exists()


def test_cli_generates_project_when_no_sibling(tmp_path):
    board = tmp_path / "board.kicad_pcb"
    board.write_text(TWO_CLASSES)
    out = tmp_path / "out.kicad_pro"

    rc = legacy_rules.main([str(board), str(out)])
    assert rc == 0
    project = json.loads(out.read_text())
    assert project["net_settings"]["classes"][0]["clearance"] == 0.149


WITH_SETUP = """
(kicad_pcb
  (setup
    (last_trace_width 0.25)
    (trace_clearance 0.127)
    (trace_min 0.15)
    (segment_width 0.2)
    (via_size 0.6)
    (via_drill 0.4)
    (via_min_size 0.45)
    (via_min_drill 0.25)
    (uvia_size 0.3)
    (uvia_min_size 0.2)
    (uvia_min_drill 0.1)
    (clearance_min 0.149)
    (pcb_text_width 0.3)
  )
  (net_class Default "This is the default net class."
    (clearance 0.149)
    (trace_width 0.15)
    (via_dia 0.45)
    (via_drill 0.25)
    (uvia_dia 0.3)
    (uvia_drill 0.1)
  )
)
"""


def test_parse_legacy_setup_maps_known_floors():
    floors = legacy_rules.parse_legacy_setup(WITH_SETUP)
    assert floors == {
        "min_track_width": 0.15,
        "min_via_diameter": 0.45,
        "min_through_hole_diameter": 0.25,
        "min_microvia_diameter": 0.2,
        "min_microvia_drill": 0.1,
        # clearance_min wins over the trace_clearance alias when both are present.
        "min_clearance": 0.149,
    }


def test_parse_legacy_setup_trace_clearance_alias_used_when_clearance_min_absent():
    text = "(kicad_pcb (setup (trace_clearance 0.127)))"
    assert legacy_rules.parse_legacy_setup(text) == {"min_clearance": 0.127}


def test_parse_legacy_setup_missing_setup_block_returns_empty():
    assert legacy_rules.parse_legacy_setup("(kicad_pcb (version 4))") == {}


def test_parse_legacy_setup_setup_block_without_floor_fields_returns_empty():
    text = "(kicad_pcb (setup (last_trace_width 0.25) (segment_width 0.2)))"
    assert legacy_rules.parse_legacy_setup(text) == {}


def test_build_project_carries_over_setup_floors_rest_stay_zero():
    classes = legacy_rules.parse_legacy_netclasses(WITH_SETUP)
    setup_floors = legacy_rules.parse_legacy_setup(WITH_SETUP)
    project = legacy_rules.build_project(classes, "board.kicad_pro", setup_floors)
    rules = project["board"]["design_settings"]["rules"]

    # Mapped legacy floors are carried over.
    assert rules["min_track_width"] == 0.15
    assert rules["min_via_diameter"] == 0.45
    assert rules["min_through_hole_diameter"] == 0.25
    assert rules["min_microvia_diameter"] == 0.2
    assert rules["min_microvia_drill"] == 0.1
    assert rules["min_clearance"] == 0.149

    # Rules legacy KiCad never had (no (setup ...) equivalent at all) stay 0.
    assert rules["min_hole_clearance"] == 0.0
    assert rules["min_hole_to_hole"] == 0.0
    assert rules["min_copper_edge_clearance"] == 0.0
    assert rules["min_via_annular_width"] == 0.0
    assert rules["min_silk_clearance"] == 0.0
    assert rules["min_text_height"] == 0.0
    assert rules["min_text_thickness"] == 0.0
    assert rules["solder_mask_to_copper_clearance"] == 0.0

    # None of WITH_SETUP's floors carry sub-micrometre precision, so rounding down leaves them
    # unchanged -- "setup_floors" (what actually landed in rules.min_*) and "setup_floors_raw"
    # (what was parsed off the board) are equal here.
    assert project["_bench"] == {"generated": True, "setup_floors": setup_floors,
                                 "setup_floors_raw": setup_floors}


def test_build_project_missing_setup_floors_defaults_to_all_zero():
    classes = legacy_rules.parse_legacy_netclasses(TWO_CLASSES)
    project = legacy_rules.build_project(classes, "board.kicad_pro")
    rules = project["board"]["design_settings"]["rules"]
    for key in legacy_rules._ZEROED_RULE_KEYS:
        assert rules[key] == 0.0
    assert project["_bench"] == {"generated": True, "setup_floors": {}, "setup_floors_raw": {}}


def test_round_down_um():
    assert legacy_rules._round_down_um(0.3302) == 0.330
    assert legacy_rules._round_down_um(0.6858) == 0.685
    assert legacy_rules._round_down_um(0.1524) == 0.152


def test_build_project_rounds_sub_micrometre_floors_down_and_keeps_raw():
    """A legacy floor with sub-micrometre precision (e.g. a mil-derived 13 mil = 0.3302mm)
    would be unsatisfiable by any routed board -- pcbnew's DSN exporter only ever writes
    via/pad sizes rounded to whole micrometres -- so build_project must round the floor DOWN
    before writing it into rules.min_*, while still recording the untouched original."""
    classes = legacy_rules.parse_legacy_netclasses(TWO_CLASSES)
    setup_floors = {"min_track_width": 0.3302, "min_via_diameter": 0.6858}
    project = legacy_rules.build_project(classes, "board.kicad_pro", setup_floors)
    rules = project["board"]["design_settings"]["rules"]

    assert rules["min_track_width"] == 0.330
    assert rules["min_via_diameter"] == 0.685
    assert project["_bench"]["setup_floors"] == {"min_track_width": 0.330, "min_via_diameter": 0.685}
    assert project["_bench"]["setup_floors_raw"] == setup_floors


def test_cli_generates_project_carries_setup_floors(tmp_path):
    board = tmp_path / "board.kicad_pcb"
    board.write_text(WITH_SETUP)
    out = tmp_path / "out.kicad_pro"

    rc = legacy_rules.main([str(board), str(out)])
    assert rc == 0
    project = json.loads(out.read_text())
    rules = project["board"]["design_settings"]["rules"]
    assert rules["min_track_width"] == 0.15
    assert rules["min_clearance"] == 0.149
    assert project["_bench"]["generated"] is True
    assert project["_bench"]["setup_floors"]["min_track_width"] == 0.15
