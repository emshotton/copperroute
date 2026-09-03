import json
from pathlib import Path

import pytest

from bench.referee import java_drc
from bench.corpus import Board

DATA = Path(__file__).parent / "data"


def test_parse_ses_counts_vias_and_length_mm():
    vias, length = java_drc.parse_ses(DATA / "mini.ses")
    assert vias == 2
    assert length == pytest.approx(40.0)


def test_parse_ses_uses_resolution_after_routes_when_present(tmp_path):
    """A (resolution ...) appearing before (routes ...) (e.g. in a placement/parser
    preamble) must not be used for the routed coordinates -- only the one inside
    (routes ...) governs how those coordinates scale to mm."""
    ses = tmp_path / "two_resolutions.ses"
    ses.write_text(
        "(session x\n"
        "  (placement (resolution mm 1))\n"
        "  (routes\n"
        "    (resolution um 10)\n"
        "    (network_out\n"
        "      (net GND\n"
        "        (via \"Via\" 0 0)\n"
        "        (wire (path F.Cu 2500 0 0 100000 0))\n"
        "      )\n"
        "    )\n"
        "  )\n"
        ")\n"
    )
    vias, length = java_drc.parse_ses(ses)
    assert vias == 1
    # 100000 units at um/10 resolution = 100000 * (1e-3/10) mm = 10.0mm, not *1.0 (mm/1).
    assert length == pytest.approx(10.0)


def test_parse_ses_falls_back_to_first_resolution_without_routes(tmp_path):
    ses = tmp_path / "no_routes.ses"
    ses.write_text("(session x (resolution mm 1) (network_out (net G (via \"V\" 0 0))))\n")
    vias, length = java_drc.parse_ses(ses)
    assert vias == 1


def test_parse_drc_report():
    r = java_drc.parse_drc_report(json.loads((DATA / "drc_two.json").read_text()))
    assert r == {"unrouted": 1, "violations": 2,
                 "violations_by_type": {"clearance": 1, "hole_clearance": 1}, "warnings": 0}


def test_parse_drc_report_counts_only_errors_and_separately_counts_warnings():
    report = {"violations": [{"type": "clearance", "severity": "error"},
                             {"type": "clearance", "severity": "warning"},
                             {"type": "silk_over_copper"}],  # no severity -> defaults to "error"
              "unconnected_items": []}
    r = java_drc.parse_drc_report(report)
    assert r["violations"] == 2 and r["warnings"] == 1
    assert r["violations_by_type"] == {"clearance": 1, "silk_over_copper": 1}


def test_run_with_fake_java(tmp_path, monkeypatch):
    """A fake 'java' that writes the DRC report lets us test the plumbing without the jar."""
    fake = tmp_path / "fakejava.py"
    fake.write_text(
        "import sys,json,shutil\n"
        "args=sys.argv[1:]\n"
        "out=args[args.index('-drc')+1]\n"
        f"shutil.copyfile({str(DATA / 'drc_two.json')!r}, out)\n")
    cell = tmp_path / "seed-1"
    cell.mkdir()
    (cell / "in.dsn").write_text((DATA / "mini.dsn").read_text())
    (cell / "out.ses").write_text((DATA / "mini.ses").read_text())
    board = Board(id="mini", source="dsn/mini.dsn", origin="freerouting-fixtures", referee="java-drc",
                  tiers=[], nets=3, layers=2)
    import sys
    r = java_drc.run(board, cell, java_exec=[sys.executable, str(fake)])
    assert r["status"] == "ok" and r["referee"] == "java-drc"
    assert r["unrouted"] == 1 and r["violations"] == 2 and r["vias"] == 2
    assert r["wirelength_mm"] == pytest.approx(40.0)
    assert json.loads((cell / "referee.json").read_text()) == r


def test_run_isolates_referee_home(tmp_path):
    """The referee jar must be launched with HOME pointed inside the cell's own
    `referee-home/` dir (distinct from the candidate's own `cell/home`), not the real user
    config dir -- otherwise the referee invocation itself could read or rewrite the shared
    freerouting.json (see README's "Settings isolation" note)."""
    fake = tmp_path / "fakejava.py"
    fake.write_text(
        "import os,sys,json\n"
        "args=sys.argv[1:]\n"
        "out=args[args.index('-drc')+1]\n"
        "with open(out, 'w') as f:\n"
        "    json.dump({'violations': [], 'unconnected_items': [],\n"
        "               'home_seen': os.environ.get('HOME')}, f)\n")
    cell = tmp_path / "seed-1"
    cell.mkdir()
    (cell / "in.dsn").write_text((DATA / "mini.dsn").read_text())
    (cell / "out.ses").write_text((DATA / "mini.ses").read_text())
    board = Board(id="mini", source="dsn/mini.dsn", origin="freerouting-fixtures", referee="java-drc",
                  tiers=[], nets=3, layers=2)
    import sys
    java_drc.run(board, cell, java_exec=[sys.executable, str(fake)])
    home = cell / "referee-home"
    assert home.is_dir()
    assert home != cell / "home"  # distinct from the candidate's own isolated home
    seen = json.loads((cell / "referee-drc.json").read_text())["home_seen"]
    assert seen == str(home)
    env_json = json.loads((cell / "referee-env.json").read_text())
    assert env_json["HOME"] == str(home)


def test_run_missing_ses_is_referee_failed(tmp_path):
    cell = tmp_path / "seed-1"
    cell.mkdir()
    board = Board(id="mini", source="dsn/mini.dsn", origin="freerouting-fixtures", referee="java-drc",
                  tiers=[], nets=3, layers=2)
    r = java_drc.run(board, cell, java_exec=["true"])
    assert r["status"] == "referee_failed" and "out.ses" in r["reason"]
    assert r["vias"] is None
    assert r["candidate_output"] is False  # spec §7.1/§8 case (a): charged to the candidate


def test_run_with_uninvocable_java_is_referee_failed(tmp_path):
    cell = tmp_path / "seed-1"
    cell.mkdir()
    (cell / "in.dsn").write_text((DATA / "mini.dsn").read_text())
    (cell / "out.ses").write_text((DATA / "mini.ses").read_text())
    board = Board(id="mini", source="dsn/mini.dsn", origin="freerouting-fixtures", referee="java-drc",
                  tiers=[], nets=3, layers=2)
    r = java_drc.run(board, cell, java_exec=["/nonexistent/java"])
    assert r["status"] == "referee_failed"
    assert "could not start" in r["reason"]
    assert r["candidate_output"] is True  # spec §7.1/§8 case (b): out.ses exists, referee failed -> unjudged


def test_measure_connections_reads_maximum_count(tmp_path, monkeypatch):
    fake = tmp_path / "fakejava.py"
    fake.write_text(
        "import json, sys\n"
        "args = sys.argv[1:]\n"
        "out = next(a.split('=', 1)[1] for a in args if a.startswith('--router.result_json='))\n"
        "with open(out, 'w') as f:\n"
        "    json.dump({'board_statistics': {'connections': {'maximum_count': 42}}}, f)\n"
    )
    import sys

    import bench.corpus as corpus_mod
    monkeypatch.setattr(corpus_mod, "CORPUS", DATA)
    board = Board(id="mini", source="mini.dsn", origin="freerouting-fixtures", referee="java-drc",
                  tiers=[], nets=3, layers=2)
    n = java_drc.measure_connections(board, [sys.executable, str(fake)])
    assert n == 42


def test_measure_connections_returns_none_on_uninvocable_java(tmp_path):
    board = Board(id="mini", source="dsn/mini.dsn", origin="freerouting-fixtures", referee="java-drc",
                  tiers=[], nets=3, layers=2)
    assert java_drc.measure_connections(board, ["/nonexistent/java"], timeout_s=5) is None


@pytest.mark.slow
def test_real_jar_drc_on_fixture(tmp_path):
    from bench import paths
    try:
        jar = paths.java_jar()
    except paths.ToolMissing:
        pytest.skip("jar not built")
    fixtures = paths.JAVA_REPO / "fixtures"
    dsn = fixtures / "Issue143-rpi_splitter_mod.dsn"
    cell = tmp_path / "seed-1"
    cell.mkdir()
    (cell / "in.dsn").write_text(dsn.read_text())
    # route once to get a real SES
    import subprocess
    subprocess.run([paths.java_exe(), "-jar", str(jar), "-de", str(cell / "in.dsn"), "-do", str(cell / "out.ses"),
                    "-mp", "20", "--gui.enabled=false", "--api_server.enabled=false", "--mcp_server.enabled=false"],
                   check=True, capture_output=True, timeout=300)
    board = Board(id="issue143", source="x", origin="freerouting-fixtures", referee="java-drc", tiers=[], nets=0, layers=2)
    r = java_drc.run(board, cell, [paths.java_exe(), "-jar", str(jar)])
    assert r["status"] == "ok", r
    assert r["vias"] >= 0 and r["wirelength_mm"] > 0
