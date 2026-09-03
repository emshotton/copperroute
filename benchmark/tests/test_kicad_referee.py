import json
from pathlib import Path

import pytest

from bench.referee import kicad

KICAD_DRC = {"$schema": "https://schemas.kicad.org/drc.v1.json", "coordinate_units": "mm",
             "unconnected_items": [{"type": "unconnected_items", "description": "Missing connection"}],
             "violations": [{"type": "clearance", "severity": "error"}, {"type": "clearance", "severity": "error"},
                            {"type": "silk_over_copper", "severity": "warning"}],
             "schematic_parity": []}

# One routing-type error (clearance), one non-routing-type error (courtyards_overlap), and
# one warning -- exercises violations (routing-only) vs violations_all (every error) vs
# violations_by_type (breakdown of every error, unchanged from before routing-type filtering).
KICAD_DRC_MIXED = {"unconnected_items": [],
                    "violations": [{"type": "clearance", "severity": "error"},
                                   {"type": "courtyards_overlap", "severity": "error"},
                                   {"type": "silk_over_copper", "severity": "warning"}]}


def test_parse_kicad_drc_counts_errors_only():
    r = kicad.parse_kicad_drc(KICAD_DRC)
    assert r == {"unrouted": 1, "violations": 2, "violations_all": 2,
                 "violations_by_type": {"clearance": 2}, "warnings": 1}


def test_parse_kicad_drc_violations_is_routing_only():
    r = kicad.parse_kicad_drc(KICAD_DRC_MIXED)
    assert r["violations"] == 1  # only the clearance error
    assert r["violations_all"] == 2  # clearance + courtyards_overlap
    assert r["violations_by_type"] == {"clearance": 1, "courtyards_overlap": 1}
    assert "courtyards_overlap" not in kicad.ROUTING_DRC_TYPES
    assert "clearance" in kicad.ROUTING_DRC_TYPES


def test_run_without_kicad_source_is_referee_failed(tmp_path):
    from bench.corpus import Board
    b = Board(id="x", source="pcbench/x/unrouted.dsn", origin="pcbench", referee="kicad", tiers=[], nets=1, layers=2,
              kicad=None)
    cell = tmp_path / "seed-1"
    cell.mkdir()
    (cell / "out.ses").write_text("(session x)")
    r = kicad.run(b, cell)
    assert r["status"] == "referee_failed" and "stripped" in r["reason"]


def test_run_copies_project_file(tmp_path, monkeypatch):
    """Fake kicad_python/kicad_cli/VENDOR_KICAD so the project-file-propagation plumbing
    in kicad.run can be tested without a real KiCad install. One stub script stands in
    for both ses_to_board.py (copies the board through) and board_stats.py (prints a
    trivial stats blob); a separate executable stub stands in for kicad-cli (writes a
    clean DRC report to whatever -o points at)."""
    import sys

    from bench.corpus import Board

    vendor_dir = tmp_path / "vendor"
    vendor_dir.mkdir()

    (vendor_dir / "ses_to_board.py").write_text(
        "import json, shutil, sys\n"
        "pcb, ses, out = sys.argv[1], sys.argv[2], sys.argv[3]\n"
        "shutil.copyfile(pcb, out)\n"
        "print(json.dumps({'wires': 0, 'vias': 0, 'skipped': []}))\n"
    )
    (vendor_dir / "board_stats.py").write_text(
        "import json\n"
        "print(json.dumps({'vias': 0, 'wirelength_mm': 0}))\n"
    )
    (vendor_dir / "refill_zones.py").write_text(
        "import json\n"
        "print(json.dumps({'zones': 3}))\n"
    )

    fake_cli = tmp_path / "fake_kicad_cli.py"
    fake_cli.write_text(
        "#!/usr/bin/env python3\n"
        "import json, sys\n"
        "out = sys.argv[sys.argv.index('-o') + 1]\n"
        "with open(out, 'w') as f:\n"
        "    json.dump({'violations': [], 'unconnected_items': []}, f)\n"
    )
    fake_cli.chmod(0o755)

    monkeypatch.setattr(kicad, "VENDOR_KICAD", vendor_dir)
    monkeypatch.setattr(kicad, "kicad_python", lambda: Path(sys.executable))
    monkeypatch.setattr(kicad, "kicad_cli", lambda: fake_cli)

    stripped = tmp_path / "stripped.kicad_pcb"
    stripped.write_text("(kicad_pcb)")
    project = tmp_path / "stripped.kicad_pro"
    project.write_text("{}")

    b = Board(id="fake", source="pcbench/fake/fake.dsn", origin="pcbench", referee="kicad", tiers=[], nets=1,
              layers=2, kicad={"stripped": str(stripped), "project": str(project)})
    cell = tmp_path / "seed-1"
    cell.mkdir()
    (cell / "out.ses").write_text("(session fake)")

    r = kicad.run(b, cell)
    assert r["status"] == "ok", r
    assert r["project_used"] is True
    assert r["zones_refilled"] == 3
    assert r["violations_all"] == 0
    assert (cell / "routed.kicad_pro").exists()
    assert (cell / "routed.kicad_pro").read_text() == "{}"


def test_run_project_propagation_wins_over_stray_refill_project(tmp_path, monkeypatch):
    """Regression test for the zone-refill-clobbers-the-project ordering bug (see
    _propagate_project_file's docstring and README.md "KiCad referee status"): pcbnew's
    board.Save() inside refill_zones.py can silently write a same-stem routed.kicad_pro of its
    own, using KiCad's default (non-zero) constraints, next to routed.kicad_pcb. Here the stub
    refill_zones.py does exactly that (min_hole_clearance: 0.25) before kicad.run propagates the
    board's real project file -- which must still win: the real project (whose own
    min_hole_clearance is 0) must end up at routed.kicad_pro, not the bogus one, and
    project_used must be True."""
    import sys

    from bench.corpus import Board

    vendor_dir = tmp_path / "vendor"
    vendor_dir.mkdir()

    (vendor_dir / "ses_to_board.py").write_text(
        "import json, shutil, sys\n"
        "pcb, ses, out = sys.argv[1], sys.argv[2], sys.argv[3]\n"
        "shutil.copyfile(pcb, out)\n"
        "print(json.dumps({'wires': 0, 'vias': 0, 'skipped': []}))\n"
    )
    (vendor_dir / "board_stats.py").write_text(
        "import json\n"
        "print(json.dumps({'vias': 0, 'wirelength_mm': 0}))\n"
    )
    # Simulate pcbnew's board.Save() inside refill_zones.py: write a stray, bogus
    # routed.kicad_pro (KiCad's non-zero default min_hole_clearance) next to routed.kicad_pcb.
    (vendor_dir / "refill_zones.py").write_text(
        "import json, pathlib, sys\n"
        "routed = pathlib.Path(sys.argv[1])\n"
        "bogus = {'board': {'design_settings': {'rules': {'min_hole_clearance': 0.25}}}}\n"
        "routed.with_suffix('.kicad_pro').write_text(json.dumps(bogus))\n"
        "print(json.dumps({'zones': 3}))\n"
    )

    fake_cli = tmp_path / "fake_kicad_cli.py"
    fake_cli.write_text(
        "#!/usr/bin/env python3\n"
        "import json, sys\n"
        "out = sys.argv[sys.argv.index('-o') + 1]\n"
        "with open(out, 'w') as f:\n"
        "    json.dump({'violations': [], 'unconnected_items': []}, f)\n"
    )
    fake_cli.chmod(0o755)

    monkeypatch.setattr(kicad, "VENDOR_KICAD", vendor_dir)
    monkeypatch.setattr(kicad, "kicad_python", lambda: Path(sys.executable))
    monkeypatch.setattr(kicad, "kicad_cli", lambda: fake_cli)

    stripped = tmp_path / "stripped.kicad_pcb"
    stripped.write_text("(kicad_pcb)")
    project = tmp_path / "stripped.kicad_pro"
    # The board's real project -- distinct from the bogus one refill_zones.py writes -- with
    # its own (correctly zeroed) min_hole_clearance.
    project.write_text(json.dumps({"board": {"design_settings": {"rules": {"min_hole_clearance": 0.0}}}}))

    b = Board(id="fake3", source="pcbench/fake3/fake3.dsn", origin="pcbench", referee="kicad", tiers=[], nets=1,
              layers=2, kicad={"stripped": str(stripped), "project": str(project)})
    cell = tmp_path / "seed-1"
    cell.mkdir()
    (cell / "out.ses").write_text("(session fake3)")

    r = kicad.run(b, cell)
    assert r["status"] == "ok", r
    assert r["project_used"] is True

    routed_project = json.loads((cell / "routed.kicad_pro").read_text())
    assert routed_project["board"]["design_settings"]["rules"]["min_hole_clearance"] == 0


def test_run_fails_when_refill_zones_fails(tmp_path, monkeypatch):
    """Zone refill is guarded like the other pipeline steps: a nonzero exit fails the
    referee run instead of silently DRC-ing an unfilled/stale board."""
    import sys

    from bench.corpus import Board

    vendor_dir = tmp_path / "vendor"
    vendor_dir.mkdir()
    (vendor_dir / "ses_to_board.py").write_text(
        "import json, shutil, sys\n"
        "pcb, ses, out = sys.argv[1], sys.argv[2], sys.argv[3]\n"
        "shutil.copyfile(pcb, out)\n"
        "print(json.dumps({'wires': 0, 'vias': 0, 'skipped': []}))\n"
    )
    (vendor_dir / "refill_zones.py").write_text(
        "import sys\n"
        "print('boom', file=sys.stderr)\n"
        "sys.exit(1)\n"
    )

    monkeypatch.setattr(kicad, "VENDOR_KICAD", vendor_dir)
    monkeypatch.setattr(kicad, "kicad_python", lambda: Path(sys.executable))
    monkeypatch.setattr(kicad, "kicad_cli", lambda: Path("/bin/false"))

    stripped = tmp_path / "stripped.kicad_pcb"
    stripped.write_text("(kicad_pcb)")

    b = Board(id="fake2", source="pcbench/fake2/fake2.dsn", origin="pcbench", referee="kicad", tiers=[], nets=1,
              layers=2, kicad={"stripped": str(stripped)})
    cell = tmp_path / "seed-1"
    cell.mkdir()
    (cell / "out.ses").write_text("(session fake)")

    r = kicad.run(b, cell)
    assert r["status"] == "referee_failed"
    assert "refill_zones.py failed" in r["reason"]


@pytest.mark.slow
def test_kicad_referee_on_spike_board(tmp_path):
    from bench import paths
    from bench.corpus import Board
    spike = Path(__file__).parent / "data" / "spike"
    if not (spike / "stripped.kicad_pcb").exists() or not (spike / "routed.ses").exists():
        pytest.skip("spike artifacts missing")
    try:
        paths.kicad_cli()
        paths.kicad_python()
    except paths.ToolMissing:
        pytest.skip("kicad missing")
    b = Board(id="spike", source="pcbench/spike/spike.dsn", origin="pcbench", referee="kicad", tiers=[], nets=1,
              layers=2, kicad={"raw": str(spike / "stripped.kicad_pcb"), "stripped": str(spike / "stripped.kicad_pcb"),
                                "project": str(spike / "stripped.kicad_pro")})
    cell = tmp_path / "seed-1"
    cell.mkdir()
    (cell / "out.ses").write_text((spike / "routed.ses").read_text())
    r = kicad.run(b, cell)
    print(json.dumps(r, indent=2))
    assert r["status"] == "ok", r
    assert r["vias"] == 35
    assert r["wirelength_mm"] > 0
    assert r["unrouted"] is not None and r["unrouted"] <= 1
    assert r["project_used"] is True
    # freerouting's own DRC reports 2 clearance violations on this route. Without the
    # board's real .kicad_pro, kicad-cli falls back to its own default 0.2mm minimum
    # track width and flags 51 legitimate (per this board's actual rules) 0.15mm
    # escape traces as track_width errors; with the project file propagated, that false
    # signal is gone and the counts should be close to freerouting's own.
    assert r["violations"] is not None and r["violations"] <= 10
