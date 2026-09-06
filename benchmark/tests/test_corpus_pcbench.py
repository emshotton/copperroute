import json
import shutil
from pathlib import Path

from bench import corpus, corpus_pcbench

DATA = Path(__file__).parent / "data"


def test_assign_d3_tier():
    assert corpus.assign_d3_tier(2) == "d3-a"
    assert corpus.assign_d3_tier(13) == "d3-a"
    assert corpus.assign_d3_tier(14) == "d3-b"
    assert corpus.assign_d3_tier(42) == "d3-b"
    assert corpus.assign_d3_tier(43) == "d3-c"
    assert corpus.assign_d3_tier(451) == "d3-c"
    assert corpus.assign_d3_tier(1) is None and corpus.assign_d3_tier(1000) is None


def test_list_board_ids(tmp_path):
    for n in ["b2", "a1", "bad"]:
        d = tmp_path / "PCBs" / n
        d.mkdir(parents=True)
        if n != "bad":
            (d / "processed.kicad_pcb").write_text("")
            (d / "raw.kicad_pcb").write_text("")
            (d / "metadata.json").write_text("{}")
    assert corpus_pcbench.list_board_ids(tmp_path) == ["a1", "b2"]


def test_import_records_exclusion_when_tools_fail(tmp_path, monkeypatch):
    d = tmp_path / "PCBs" / "x"
    d.mkdir(parents=True)
    for f in ["processed.kicad_pcb", "raw.kicad_pcb"]:
        (d / f).write_text("(kicad_pcb)")
    (d / "metadata.json").write_text(json.dumps({"layers": 2, "kicad_version": "7"}))
    monkeypatch.setattr(corpus, "CORPUS", tmp_path / "corpus")
    monkeypatch.setattr(corpus_pcbench, "_kicad_step", lambda *a, **k: (1, "simulated failure"))

    # zone refill runs before the (mocked) steps list and would otherwise hit a real KiCad
    # install trying to load the fake "(kicad_pcb)" content -- stub it out (mirroring what the
    # real _refill_zones does: back up the pristine board as raw.orig.kicad_pcb, which
    # _generate_project reads) so the simulated `_kicad_step` failure below is what actually
    # excludes the board.
    def fake_refill_zones(py, raw_pcb, dst, log):
        shutil.copyfile(raw_pcb, dst / "raw.orig.kicad_pcb")
        return 0, ""

    monkeypatch.setattr(corpus_pcbench, "_refill_zones", fake_refill_zones)
    boards = corpus_pcbench.import_boards(tmp_path, ids=["x"])
    assert boards[0].status == "excluded" and "simulated failure" in boards[0].reason
    assert corpus.load_manifest()[0].id == "pcbench-x"


def _stub_successful_kicad_pipeline(monkeypatch):
    dsn_text = (DATA / "mini.dsn").read_text()
    calls = []

    def fake_kicad_step(argv, log):
        calls.append(argv)
        if "export_specctra_dsn.py" in argv[1]:
            Path(argv[3]).write_text(dsn_text)
        elif "drc" in argv:
            out = Path(argv[argv.index("-o") + 1])
            out.write_text(json.dumps({"violations": []}))
        return 0, ""

    def fake_run(argv, **kwargs):
        class Result:
            returncode = 0
            stdout = (json.dumps({"zones": 5}) if "refill_zones.py" in argv[1]
                     else json.dumps({"vias": 2, "wirelength_mm": 12.5}))
            stderr = ""

        return Result()

    monkeypatch.setattr(corpus_pcbench, "_kicad_step", fake_kicad_step)
    monkeypatch.setattr(corpus_pcbench.subprocess, "run", fake_run)
    return calls


def _write_pcbench_board(root, bid, layers=2):
    d = root / "PCBs" / bid
    d.mkdir(parents=True)
    for f in ["processed.kicad_pcb", "raw.kicad_pcb"]:
        (d / f).write_text("(kicad_pcb)")
    (d / "metadata.json").write_text(json.dumps({"layers": layers, "kicad_version": "7"}))


def test_import_boards_parallel_preserves_manifest_merge(tmp_path, monkeypatch):
    _write_pcbench_board(tmp_path, "a")
    _write_pcbench_board(tmp_path, "b")
    monkeypatch.setattr(corpus, "CORPUS", tmp_path / "corpus")
    _stub_successful_kicad_pipeline(monkeypatch)

    lines = []
    boards = corpus_pcbench.import_boards(tmp_path, ids=["a", "b"], jobs=2, progress=lines.append)

    assert [b.id for b in boards] == ["pcbench-a", "pcbench-b"]
    assert all(b.status == "ok" for b in boards)
    # the manifest on disk merges both boards (no writer race lost an entry) despite jobs=2
    manifest_ids = {b.id for b in corpus.load_manifest()}
    assert manifest_ids == {"pcbench-a", "pcbench-b"}

    # neither board ships a .kicad_pro, so one is generated from the raw board's own
    # (nonexistent, here) net_class rules, zones get refilled, and both facts are recorded
    # in ground_truth.json alongside the routing-vs-all DRC error split.
    for b in boards:
        gt = json.loads((corpus.CORPUS / b.kicad["ground_truth"]).read_text())
        assert gt["project_generated"] is True
        assert gt["zones_refilled"] == 5
        assert gt["drv_routing"] == 0
        assert gt["drv_all"] == 0
        assert (corpus.CORPUS / b.kicad["project"]).exists()
        raw_dir = (corpus.CORPUS / b.kicad["raw"]).parent
        assert (raw_dir / "raw.orig.kicad_pcb").exists()
    assert any("pcbench-a" in l for l in lines)
    assert any("pcbench-b" in l for l in lines)


def test_import_boards_skip_existing_then_force(tmp_path, monkeypatch):
    _write_pcbench_board(tmp_path, "a")
    monkeypatch.setattr(corpus, "CORPUS", tmp_path / "corpus")
    calls = _stub_successful_kicad_pipeline(monkeypatch)

    boards = corpus_pcbench.import_boards(tmp_path, ids=["a"])
    assert boards[0].status == "ok"
    calls_after_first_import = len(calls)
    assert calls_after_first_import > 0

    # default (--skip-existing) resumes without re-running the kicad pipeline
    lines = []
    boards = corpus_pcbench.import_boards(tmp_path, ids=["a"], progress=lines.append)
    assert boards[0].id == "pcbench-a" and boards[0].status == "ok"
    assert len(calls) == calls_after_first_import
    assert any("skipped 1" in l for l in lines)

    # --force (skip_existing=False) re-imports even though the board is already ok
    boards = corpus_pcbench.import_boards(tmp_path, ids=["a"], skip_existing=False)
    assert boards[0].status == "ok"
    assert len(calls) > calls_after_first_import


def test_import_board_project_overwrites_stray_refill_output(tmp_path, monkeypatch):
    """Regression test for the zone-refill-clobbers-the-project bug: pcbnew's board.Save()
    (inside refill_zones.py) can silently write its own same-stem .kicad_pro/.kicad_prl using
    KiCad's *default* (non-zero) constraints. _import_board must (re)write the real/generated
    project AFTER refill so it wins, and clean up any stray .kicad_prl left behind."""
    _write_pcbench_board(tmp_path, "z")
    monkeypatch.setattr(corpus, "CORPUS", tmp_path / "corpus")
    dsn_text = (DATA / "mini.dsn").read_text()

    def fake_kicad_step(argv, log):
        if "export_specctra_dsn.py" in argv[1]:
            Path(argv[3]).write_text(dsn_text)
        elif "drc" in argv:
            out = Path(argv[argv.index("-o") + 1])
            out.write_text(json.dumps({"violations": []}))
        return 0, ""

    def fake_refill_zones(py, raw_pcb, dst, log):
        shutil.copyfile(raw_pcb, dst / "raw.orig.kicad_pcb")
        # simulate pcbnew's board.Save() writing a stray project with KiCad's *default*
        # (non-zero) min_hole_clearance, plus a stray .kicad_prl -- exactly the bug this
        # ordering fixes.
        bogus = {"board": {"design_settings": {"rules": {"min_hole_clearance": 0.25}}}}
        (dst / "raw.kicad_pro").write_text(json.dumps(bogus))
        (dst / "raw.kicad_prl").write_text("{}")
        return 5, ""

    def fake_run(argv, **kwargs):
        class Result:
            returncode = 0
            stdout = json.dumps({"vias": 2, "wirelength_mm": 12.5})
            stderr = ""

        return Result()

    monkeypatch.setattr(corpus_pcbench, "_kicad_step", fake_kicad_step)
    monkeypatch.setattr(corpus_pcbench, "_refill_zones", fake_refill_zones)
    monkeypatch.setattr(corpus_pcbench.subprocess, "run", fake_run)

    boards = corpus_pcbench.import_boards(tmp_path, ids=["z"])
    assert boards[0].status == "ok", boards[0].reason

    dst = corpus.CORPUS / "pcbench" / "z"
    project = json.loads((dst / "raw.kicad_pro").read_text())
    assert project["board"]["design_settings"]["rules"]["min_hole_clearance"] == 0
    assert not (dst / "raw.kicad_prl").exists()


def test_import_board_reimport_recopies_raw_pcb_preserving_net_classes(tmp_path, monkeypatch):
    """Regression test for the force/resume net-class-fidelity bug: `_import_board` used to
    only copy `src_pcb` into `raw.kicad_pcb` when that file was missing, so on a forced or
    resumed re-import it reused whatever was already on disk -- which, after a first import's
    zone refill, is a board whose legacy `(net_class ...)` blocks are gone as literal text
    (pcbnew's `board.Save()` stops serialising them). `_generate_project` then read that
    net-class-less board and silently fell back to KiCad's generic Default class instead of
    the fixture's real net-class clearance. `_import_board` must now always (re)copy the
    pristine source before regenerating the project."""
    monkeypatch.setattr(corpus, "CORPUS", tmp_path / "corpus")
    dst = tmp_path / "corpus" / "kicad" / "widget"

    fixture_src = tmp_path / "widget.kicad_pcb"
    fixture_src.write_text(
        "(kicad_pcb\n"
        '  (net_class Default "d"\n'
        "    (clearance 0.181)\n"
        "    (trace_width 0.15)\n"
        "    (via_dia 0.45)\n"
        "    (via_drill 0.25)\n"
        "    (uvia_dia 0.3)\n"
        "    (uvia_drill 0.1)\n"
        "    (add_net GND)\n"
        "  )\n"
        ")\n"
    )

    dsn_text = (DATA / "mini.dsn").read_text()

    def fake_kicad_step(argv, log):
        if "export_specctra_dsn.py" in argv[1]:
            Path(argv[3]).write_text(dsn_text)
        elif "drc" in argv:
            out = Path(argv[argv.index("-o") + 1])
            out.write_text(json.dumps({"violations": []}))
        return 0, ""

    def fake_refill_zones(py, raw_pcb, dst_, log):
        # Mirror what the real refill_zones.py does: back up the board as it is right now
        # (still carrying the legacy net_class block, if this is the pristine copy) and then
        # rewrite raw_pcb into a "modern format" board with that block gone as literal text --
        # exactly what pcbnew's board.Save() does.
        shutil.copyfile(raw_pcb, dst_ / "raw.orig.kicad_pcb")
        raw_pcb.write_text('(kicad_pcb (version 20241229) (generator "pcbnew"))\n')
        return 5, ""

    def fake_run(argv, **kwargs):
        class Result:
            returncode = 0
            stdout = json.dumps({"vias": 2, "wirelength_mm": 12.5})
            stderr = ""

        return Result()

    monkeypatch.setattr(corpus_pcbench, "_kicad_step", fake_kicad_step)
    monkeypatch.setattr(corpus_pcbench, "_refill_zones", fake_refill_zones)
    monkeypatch.setattr(corpus_pcbench.subprocess, "run", fake_run)

    board1 = corpus_pcbench._import_board(fixture_src, None, dst, "kicad-widget",
                                          "freerouting-kicad", ["kicad-fixtures"])
    assert board1.status == "ok", board1.reason
    project1 = json.loads((dst / "stripped.kicad_pro").read_text())
    assert project1["net_settings"]["classes"][0]["clearance"] == 0.181

    # After the first import, raw.kicad_pcb on disk is the already-refilled, net_class-less
    # board left over from fake_refill_zones above -- exactly the stale state a forced/resumed
    # re-import would see.
    assert "net_class" not in (dst / "raw.kicad_pcb").read_text()

    board2 = corpus_pcbench._import_board(fixture_src, None, dst, "kicad-widget",
                                          "freerouting-kicad", ["kicad-fixtures"])
    assert board2.status == "ok", board2.reason
    project2 = json.loads((dst / "stripped.kicad_pro").read_text())
    assert project2["net_settings"]["classes"][0]["clearance"] == 0.181


def test_import_excludes_only_on_routing_type_drc_errors(tmp_path, monkeypatch):
    """A reference board with only non-routing DRC errors (e.g. courtyards_overlap) stays
    `ok`; one with even a single routing-type error (clearance) is excluded."""
    _write_pcbench_board(tmp_path, "nonrouting")
    _write_pcbench_board(tmp_path, "routing")
    monkeypatch.setattr(corpus, "CORPUS", tmp_path / "corpus")
    dsn_text = (DATA / "mini.dsn").read_text()

    def fake_kicad_step(argv, log):
        if "export_specctra_dsn.py" in argv[1]:
            Path(argv[3]).write_text(dsn_text)
        elif "drc" in argv:
            out = Path(argv[argv.index("-o") + 1])
            board_path = Path(argv[-1])
            if "nonrouting" in str(board_path):
                violations = [{"type": "courtyards_overlap", "severity": "error"}] * 40
            else:
                violations = [{"type": "clearance", "severity": "error"}] * 3
            out.write_text(json.dumps({"violations": violations}))
        return 0, ""

    def fake_run(argv, **kwargs):
        class Result:
            returncode = 0
            stdout = (json.dumps({"zones": 5}) if "refill_zones.py" in argv[1]
                     else json.dumps({"vias": 2, "wirelength_mm": 12.5}))
            stderr = ""

        return Result()

    monkeypatch.setattr(corpus_pcbench, "_kicad_step", fake_kicad_step)
    monkeypatch.setattr(corpus_pcbench.subprocess, "run", fake_run)

    boards = corpus_pcbench.import_boards(tmp_path, ids=["nonrouting", "routing"])
    by_id = {b.id: b for b in boards}

    nonrouting = by_id["pcbench-nonrouting"]
    assert nonrouting.status == "ok", nonrouting.reason
    gt = json.loads((corpus.CORPUS / nonrouting.kicad["ground_truth"]).read_text())
    assert gt["drv_routing"] == 0 and gt["drv_all"] == 40

    routing = by_id["pcbench-routing"]
    assert routing.status == "excluded"
    assert "3 routing DRC errors" in routing.reason
    gt = json.loads((corpus.CORPUS / routing.kicad["ground_truth"]).read_text())
    assert gt["drv_routing"] == 3 and gt["drv_all"] == 3


def test_import_kicad_fixtures_discovers_boards(tmp_path, monkeypatch):
    fixtures = tmp_path / "fixtures"
    good = fixtures / "IssueXYZ-widget"
    good.mkdir(parents=True)
    (good / "widget.kicad_pcb").write_text("(kicad_pcb)")
    (good / "widget.kicad_pro").write_text("{}")

    bad = fixtures / "IssueABC-nothing"
    bad.mkdir(parents=True)
    (bad / "nothing.kicad_pcb-bak").write_text("(kicad_pcb)")

    monkeypatch.setattr(corpus, "CORPUS", tmp_path / "corpus")

    dsn_text = (DATA / "mini.dsn").read_text()

    def fake_kicad_step(argv, log):
        if "export_specctra_dsn.py" in argv[1]:
            Path(argv[3]).write_text(dsn_text)
        elif "drc" in argv:
            out = Path(argv[argv.index("-o") + 1])
            out.write_text(json.dumps({"violations": []}))
        return 0, ""

    def fake_run(argv, **kwargs):
        class Result:
            returncode = 0
            stdout = (json.dumps({"zones": 5}) if "refill_zones.py" in argv[1]
                     else json.dumps({"vias": 2, "wirelength_mm": 12.5}))
            stderr = ""

        return Result()

    monkeypatch.setattr(corpus_pcbench, "_kicad_step", fake_kicad_step)
    monkeypatch.setattr(corpus_pcbench.subprocess, "run", fake_run)

    boards = corpus_pcbench.import_kicad_fixtures(fixtures)

    assert len(boards) == 1
    b = boards[0]
    assert b.id == "kicad-issuexyz-widget--widget"
    assert b.origin == "freerouting-kicad"
    assert b.kicad["project"] is not None and not Path(b.kicad["project"]).is_absolute()
    assert b.kicad_path("project") is not None and b.kicad_path("project").exists()
    assert "kicad-fixtures" in b.tiers
    # the board's own widget.kicad_pro was copied through, not generated -- and its content
    # (a real project's contents, "{}" here) is preserved, not overwritten by legacy_rules.
    assert b.kicad_path("project").read_text() == "{}"
    gt = json.loads((corpus.CORPUS / b.kicad["ground_truth"]).read_text())
    assert gt["project_generated"] is False
    assert gt["zones_refilled"] == 5


def test_reference_verdict_routing_errors():
    gt = {"drv_routing": 2, "drv_all": 5, "unconnected": 0, "wirelength_mm": 10.0}
    assert corpus_pcbench.reference_verdict(gt) == (
        "excluded", "reference board has 2 routing DRC errors (of 5 total)")


def test_reference_verdict_unconnected():
    gt = {"drv_routing": 0, "drv_all": 0, "unconnected": 3, "wirelength_mm": 10.0}
    assert corpus_pcbench.reference_verdict(gt) == (
        "excluded", "reference board has 3 unconnected items")


def test_reference_verdict_unrouted_reference_carve_out():
    # Zero routing -> every net is unconnected by construction, so the `unconnected`
    # exclusion doesn't apply; the board stays ok (reference_complete is what flags it as an
    # incomplete/unrouted reference, computed by the caller, not reference_verdict).
    gt = {"drv_routing": 0, "drv_all": 0, "unconnected": 40, "wirelength_mm": 0.0}
    assert corpus_pcbench.reference_verdict(gt) == ("ok", "")


def test_reference_verdict_routing_error_excludes_even_when_unrouted():
    # Regression test: a routing-type DRC error must exclude the board even when it has zero
    # routing (vias==0, wirelength==0) -- this used to slip through pcbench-nichiden27_PISCIUM
    # (d3-c, drv_routing==1) because the old carve-out exempted *any* drv_routing>0 exclusion
    # for an unrouted reference, not just the (new) unconnected-items one.
    gt = {"drv_routing": 1, "drv_all": 2, "unconnected": 118, "wirelength_mm": 0.0}
    status, reason = corpus_pcbench.reference_verdict(gt)
    assert status == "excluded"
    assert reason == "reference board has 1 routing DRC errors (of 2 total)"


def test_reference_verdict_clean():
    gt = {"drv_routing": 0, "drv_all": 0, "unconnected": 0, "wirelength_mm": 10.0}
    assert corpus_pcbench.reference_verdict(gt) == ("ok", "")


def test_import_board_records_unconnected_and_excludes_routed_reference(tmp_path, monkeypatch):
    _write_pcbench_board(tmp_path, "a")
    monkeypatch.setattr(corpus, "CORPUS", tmp_path / "corpus")
    dsn_text = (DATA / "mini.dsn").read_text()

    def fake_kicad_step(argv, log):
        if "export_specctra_dsn.py" in argv[1]:
            Path(argv[3]).write_text(dsn_text)
        elif "drc" in argv:
            out = Path(argv[argv.index("-o") + 1])
            out.write_text(json.dumps({
                "violations": [],
                "unconnected_items": [{"type": "unconnected_items"}] * 3,
            }))
        return 0, ""

    def fake_run(argv, **kwargs):
        class Result:
            returncode = 0
            stdout = (json.dumps({"zones": 5}) if "refill_zones.py" in argv[1]
                     else json.dumps({"vias": 2, "wirelength_mm": 12.5}))
            stderr = ""

        return Result()

    monkeypatch.setattr(corpus_pcbench, "_kicad_step", fake_kicad_step)
    monkeypatch.setattr(corpus_pcbench.subprocess, "run", fake_run)

    boards = corpus_pcbench.import_boards(tmp_path, ids=["a"])
    b = boards[0]
    assert b.status == "excluded"
    assert b.reason == "reference board has 3 unconnected items"

    gt = json.loads((corpus.CORPUS / b.kicad["ground_truth"]).read_text())
    assert gt["unconnected"] == 3
    assert gt["reference_complete"] is False
    assert gt["wirelength_mm"] == 12.5


def test_import_board_unrouted_reference_stays_ok_despite_unconnected_items(tmp_path, monkeypatch):
    """An unrouted reference (0 vias, 0 wirelength) has every net unconnected by nature -- it
    stays ok, flagged reference_complete: false, not excluded."""
    _write_pcbench_board(tmp_path, "a")
    monkeypatch.setattr(corpus, "CORPUS", tmp_path / "corpus")
    dsn_text = (DATA / "mini.dsn").read_text()

    def fake_kicad_step(argv, log):
        if "export_specctra_dsn.py" in argv[1]:
            Path(argv[3]).write_text(dsn_text)
        elif "drc" in argv:
            out = Path(argv[argv.index("-o") + 1])
            out.write_text(json.dumps({
                "violations": [],
                "unconnected_items": [{"type": "unconnected_items"}] * 40,
            }))
        return 0, ""

    def fake_run(argv, **kwargs):
        class Result:
            returncode = 0
            stdout = (json.dumps({"zones": 5}) if "refill_zones.py" in argv[1]
                     else json.dumps({"vias": 0, "wirelength_mm": 0.0}))
            stderr = ""

        return Result()

    monkeypatch.setattr(corpus_pcbench, "_kicad_step", fake_kicad_step)
    monkeypatch.setattr(corpus_pcbench.subprocess, "run", fake_run)

    boards = corpus_pcbench.import_boards(tmp_path, ids=["a"])
    b = boards[0]
    assert b.status == "ok", b.reason

    gt = json.loads((corpus.CORPUS / b.kicad["ground_truth"]).read_text())
    assert gt["unconnected"] == 40
    assert gt["reference_complete"] is False


def test_import_board_excludes_unrouted_reference_with_routing_drc_error(tmp_path, monkeypatch):
    """Regression test for the drv_routing==1-slips-through bug: an unrouted reference (0
    vias, 0 wirelength) with a genuine routing-type DRC error must still be excluded -- the
    unrouted carve-out only ever covered the unconnected-items exclusion."""
    _write_pcbench_board(tmp_path, "a")
    monkeypatch.setattr(corpus, "CORPUS", tmp_path / "corpus")
    dsn_text = (DATA / "mini.dsn").read_text()

    def fake_kicad_step(argv, log):
        if "export_specctra_dsn.py" in argv[1]:
            Path(argv[3]).write_text(dsn_text)
        elif "drc" in argv:
            out = Path(argv[argv.index("-o") + 1])
            out.write_text(json.dumps({
                "violations": [{"type": "clearance", "severity": "error"},
                              {"type": "courtyards_overlap", "severity": "error"}],
                "unconnected_items": [{"type": "unconnected_items"}] * 118,
            }))
        return 0, ""

    def fake_run(argv, **kwargs):
        class Result:
            returncode = 0
            stdout = (json.dumps({"zones": 0}) if "refill_zones.py" in argv[1]
                     else json.dumps({"vias": 0, "wirelength_mm": 0.0}))
            stderr = ""

        return Result()

    monkeypatch.setattr(corpus_pcbench, "_kicad_step", fake_kicad_step)
    monkeypatch.setattr(corpus_pcbench.subprocess, "run", fake_run)

    boards = corpus_pcbench.import_boards(tmp_path, ids=["a"])
    b = boards[0]
    assert b.status == "excluded"
    assert b.reason == "reference board has 1 routing DRC errors (of 2 total)"


_LEGACY_BOARD_WITH_TIGHT_FLOOR = """
(kicad_pcb
  (setup
    (trace_min 0.3302)
  )
  (net_class Default "d"
    (clearance 0.15)
    (trace_width 0.2)
    (via_dia 0.6)
    (via_drill 0.4)
    (uvia_dia 0.3)
    (uvia_drill 0.1)
  )
)
"""


def test_revalidate_flips_ok_to_excluded_on_new_unconnected_items(tmp_path, monkeypatch):
    _write_pcbench_board(tmp_path, "a")
    monkeypatch.setattr(corpus, "CORPUS", tmp_path / "corpus")
    _stub_successful_kicad_pipeline(monkeypatch)
    boards = corpus_pcbench.import_boards(tmp_path, ids=["a"])
    assert boards[0].status == "ok"

    dst = corpus.CORPUS / "pcbench" / "a"
    drc = json.loads((dst / "raw-drc.json").read_text())
    drc["unconnected_items"] = [{"type": "unconnected_items"}] * 3
    (dst / "raw-drc.json").write_text(json.dumps(drc))

    lines = []
    summary = corpus_pcbench.revalidate("pcbench", progress=lines.append)
    assert summary["checked"] == 1 and summary["skipped"] == 0
    assert summary["ok_to_excluded"] == [("pcbench-a", "reference board has 3 unconnected items")]
    assert summary["excluded_to_ok"] == []
    assert any("pcbench-a: ok -> excluded" in l for l in lines)

    gt = json.loads((dst / "ground_truth.json").read_text())
    assert gt["unconnected"] == 3 and gt["reference_complete"] is False

    manifest_board = {b.id: b for b in corpus.load_manifest()}["pcbench-a"]
    assert manifest_board.status == "excluded"
    assert manifest_board.reason == "reference board has 3 unconnected items"


def test_revalidate_flips_excluded_to_ok_when_drc_report_improves(tmp_path, monkeypatch):
    _write_pcbench_board(tmp_path, "a")
    monkeypatch.setattr(corpus, "CORPUS", tmp_path / "corpus")
    dsn_text = (DATA / "mini.dsn").read_text()

    def fake_kicad_step(argv, log):
        if "export_specctra_dsn.py" in argv[1]:
            Path(argv[3]).write_text(dsn_text)
        elif "drc" in argv:
            out = Path(argv[argv.index("-o") + 1])
            out.write_text(json.dumps({"violations": [{"type": "clearance", "severity": "error"}]}))
        return 0, ""

    def fake_run(argv, **kwargs):
        class Result:
            returncode = 0
            stdout = (json.dumps({"zones": 5}) if "refill_zones.py" in argv[1]
                     else json.dumps({"vias": 2, "wirelength_mm": 12.5}))
            stderr = ""

        return Result()

    monkeypatch.setattr(corpus_pcbench, "_kicad_step", fake_kicad_step)
    monkeypatch.setattr(corpus_pcbench.subprocess, "run", fake_run)

    boards = corpus_pcbench.import_boards(tmp_path, ids=["a"])
    assert boards[0].status == "excluded"

    dst = corpus.CORPUS / "pcbench" / "a"
    (dst / "raw-drc.json").write_text(json.dumps({"violations": []}))

    summary = corpus_pcbench.revalidate("pcbench")
    assert summary["excluded_to_ok"] == ["pcbench-a"]
    assert summary["ok_to_excluded"] == []
    assert corpus.load_manifest()[0].status == "ok"
    assert corpus.load_manifest()[0].reason == ""


def test_revalidate_skips_boards_without_raw_drc_json(tmp_path, monkeypatch):
    _write_pcbench_board(tmp_path, "a")
    monkeypatch.setattr(corpus, "CORPUS", tmp_path / "corpus")
    _stub_successful_kicad_pipeline(monkeypatch)
    corpus_pcbench.import_boards(tmp_path, ids=["a"])
    dst = corpus.CORPUS / "pcbench" / "a"
    (dst / "raw-drc.json").unlink()

    summary = corpus_pcbench.revalidate("pcbench")
    assert summary["checked"] == 1 and summary["skipped"] == 1
    assert summary["ok_to_excluded"] == [] and summary["excluded_to_ok"] == []
    # unaffected board's status is untouched
    assert corpus.load_manifest()[0].status == "ok"


def test_revalidate_only_touches_boards_of_the_given_origin(tmp_path, monkeypatch):
    _write_pcbench_board(tmp_path, "a")
    monkeypatch.setattr(corpus, "CORPUS", tmp_path / "corpus")
    _stub_successful_kicad_pipeline(monkeypatch)
    corpus_pcbench.import_boards(tmp_path, ids=["a"])

    summary = corpus_pcbench.revalidate("freerouting-kicad")
    assert summary["checked"] == 0 and summary["skipped"] == 0


def test_revalidate_regenerate_projects_rewrites_from_pristine_backup(tmp_path, monkeypatch):
    """--regenerate-projects picks up a vendor/kicad/legacy_rules.py fix (here: rounding a
    legacy setup floor down to the nearest micrometre) for an already-imported board, without
    re-running the KiCad pipeline -- by rebuilding the project from the pristine
    raw.orig.kicad_pcb backup using the *current* legacy_rules code."""
    d = tmp_path / "PCBs" / "a"
    d.mkdir(parents=True)
    for f in ["processed.kicad_pcb", "raw.kicad_pcb"]:
        (d / f).write_text(_LEGACY_BOARD_WITH_TIGHT_FLOOR)
    (d / "metadata.json").write_text(json.dumps({"layers": 2, "kicad_version": "7"}))
    monkeypatch.setattr(corpus, "CORPUS", tmp_path / "corpus")
    _stub_successful_kicad_pipeline(monkeypatch)

    boards = corpus_pcbench.import_boards(tmp_path, ids=["a"])
    assert boards[0].status == "ok", boards[0].reason
    dst = corpus.CORPUS / "pcbench" / "a"
    assert (dst / "raw.orig.kicad_pcb").exists()

    # Simulate a project generated by a pre-fix version of legacy_rules.py, still carrying
    # the unrounded (unsatisfiable) floor.
    stale = json.loads((dst / "raw.kicad_pro").read_text())
    stale["board"]["design_settings"]["rules"]["min_track_width"] = 0.3302
    (dst / "raw.kicad_pro").write_text(json.dumps(stale))

    summary = corpus_pcbench.revalidate("pcbench", regenerate_projects=True)
    assert summary["regenerated_projects"] == 1

    for name in ("raw.kicad_pro", "stripped.kicad_pro"):
        project = json.loads((dst / name).read_text())
        rules = project["board"]["design_settings"]["rules"]
        assert rules["min_track_width"] == 0.330
        assert project["_bench"]["setup_floors_raw"]["min_track_width"] == 0.3302


def test_revalidate_regenerate_projects_skips_boards_with_a_real_project(tmp_path, monkeypatch):
    """A board that shipped its own real .kicad_pro (project_generated: false) must not have
    its project rebuilt from legacy rules -- there's nothing bench-generated to fix."""
    d = tmp_path / "PCBs" / "a"
    d.mkdir(parents=True)
    for f in ["processed.kicad_pcb", "raw.kicad_pcb"]:
        (d / f).write_text("(kicad_pcb)")
    (d / "metadata.json").write_text(json.dumps({"layers": 2, "kicad_version": "7"}))
    (d / "a.kicad_pro").write_text(json.dumps({"real": True}))
    monkeypatch.setattr(corpus, "CORPUS", tmp_path / "corpus")
    _stub_successful_kicad_pipeline(monkeypatch)

    boards = corpus_pcbench.import_boards(tmp_path, ids=["a"])
    assert boards[0].status == "ok", boards[0].reason
    dst = corpus.CORPUS / "pcbench" / "a"
    before = (dst / "raw.kicad_pro").read_text()

    summary = corpus_pcbench.revalidate("pcbench", regenerate_projects=True)
    assert summary["regenerated_projects"] == 0
    assert (dst / "raw.kicad_pro").read_text() == before


def test_revalidate_rerun_drc_rewrites_report_and_recomputes(tmp_path, monkeypatch):
    _write_pcbench_board(tmp_path, "a")
    monkeypatch.setattr(corpus, "CORPUS", tmp_path / "corpus")
    _stub_successful_kicad_pipeline(monkeypatch)
    boards = corpus_pcbench.import_boards(tmp_path, ids=["a"])
    assert boards[0].status == "ok"

    monkeypatch.setattr(corpus_pcbench, "kicad_cli", lambda: Path("/fake/kicad-cli"))

    def fake_kicad_step(argv, log):
        assert "drc" in argv
        out = Path(argv[argv.index("-o") + 1])
        out.write_text(json.dumps({"violations": [{"type": "clearance", "severity": "error"}]}))
        return 0, ""

    monkeypatch.setattr(corpus_pcbench, "_kicad_step", fake_kicad_step)

    summary = corpus_pcbench.revalidate("pcbench", rerun_drc=True)
    assert summary["reran_drc"] == 1
    assert summary["ok_to_excluded"] == [
        ("pcbench-a", "reference board has 1 routing DRC errors (of 1 total)")]

    dst = corpus.CORPUS / "pcbench" / "a"
    drc = json.loads((dst / "raw-drc.json").read_text())
    assert drc["violations"][0]["type"] == "clearance"


def test_revalidate_rerun_drc_skips_board_missing_raw_pcb(tmp_path, monkeypatch):
    _write_pcbench_board(tmp_path, "a")
    monkeypatch.setattr(corpus, "CORPUS", tmp_path / "corpus")
    _stub_successful_kicad_pipeline(monkeypatch)
    corpus_pcbench.import_boards(tmp_path, ids=["a"])

    dst = corpus.CORPUS / "pcbench" / "a"
    (dst / "raw.kicad_pcb").unlink()

    monkeypatch.setattr(corpus_pcbench, "kicad_cli", lambda: Path("/fake/kicad-cli"))
    calls = []
    monkeypatch.setattr(corpus_pcbench, "_kicad_step", lambda *a, **k: calls.append(a) or (0, ""))

    summary = corpus_pcbench.revalidate("pcbench", rerun_drc=True)
    assert summary["skipped"] == 1
    assert calls == []


def test_board_license_reads_the_normalised_shape():
    meta = {"licenses": {"spdx_id": "CERN-OHL-P-2.0", "status": "licensed", "file": "LICENSE"}}
    assert corpus_pcbench.board_license(meta) == {"spdx_id": "CERN-OHL-P-2.0", "status": "licensed"}


def test_board_license_tolerates_the_old_github_and_kitspace_shapes():
    assert corpus_pcbench.board_license({"licenses": {"key": "mit", "spdx_id": "MIT"}}) == {"spdx_id": "MIT", "status": "licensed"}
    assert corpus_pcbench.board_license({"licenses": {"spdx_id": "NOASSERTION"}}) == {"spdx_id": None, "status": "unknown"}
    assert corpus_pcbench.board_license({"licenses": None}) == {"spdx_id": None, "status": "unknown"}
    assert corpus_pcbench.board_license({"licenses": [{"name": "GNU", "link": ""}]}) == {"spdx_id": None, "status": "unknown"}
    assert corpus_pcbench.board_license({}) == {"spdx_id": None, "status": "unknown"}


def test_import_records_the_board_license_in_the_manifest(tmp_path, monkeypatch):
    _write_pcbench_board(tmp_path, "a")
    (tmp_path / "PCBs" / "a" / "metadata.json").write_text(json.dumps(
        {"layers": 2, "kicad_version": "7", "licenses": {"spdx_id": "MIT", "status": "licensed"}}))
    monkeypatch.setattr(corpus, "CORPUS", tmp_path / "corpus")
    _stub_successful_kicad_pipeline(monkeypatch)

    boards = corpus_pcbench.import_boards(tmp_path, ids=["a"])
    assert boards[0].license == {"spdx_id": "MIT", "status": "licensed"}
    assert corpus.load_manifest()[0].license == {"spdx_id": "MIT", "status": "licensed"}


def test_import_boards_licensed_only_skips_unlicensed_boards(tmp_path, monkeypatch):
    _write_pcbench_board(tmp_path, "lic")
    _write_pcbench_board(tmp_path, "unlic")
    (tmp_path / "PCBs" / "lic" / "metadata.json").write_text(json.dumps(
        {"layers": 2, "licenses": {"spdx_id": "MIT", "status": "licensed"}}))
    (tmp_path / "PCBs" / "unlic" / "metadata.json").write_text(json.dumps(
        {"layers": 2, "licenses": {"spdx_id": None, "status": "unlicensed"}}))
    monkeypatch.setattr(corpus, "CORPUS", tmp_path / "corpus")
    _stub_successful_kicad_pipeline(monkeypatch)

    lines = []
    boards = corpus_pcbench.import_boards(tmp_path, licensed_only=True, progress=lines.append)
    assert [b.id for b in boards] == ["pcbench-lic"]
    assert [b.id for b in corpus.load_manifest()] == ["pcbench-lic"]
    assert any("unlicensed" in l for l in lines)

    boards = corpus_pcbench.import_boards(tmp_path)
    assert [b.id for b in boards] == ["pcbench-lic", "pcbench-unlic"]
