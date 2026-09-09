import json
from pathlib import Path

from bench import corpus
from bench.corpus import Board, dsn_info, init_from_fixtures, select

DATA = Path(__file__).parent / "data"


def test_dsn_info_counts_nets_and_signal_layers():
    nets, layers = dsn_info(DATA / "mini.dsn")
    assert nets == 3
    assert layers == 2


def test_dsn_info_ignores_wiring_nets_without_class():
    nets, layers = dsn_info(DATA / "mini_no_class.dsn")
    assert nets == 3
    assert layers == 2


def test_init_from_fixtures_copies_and_tags(tmp_path, monkeypatch):
    fixtures = tmp_path / "fixtures"
    fixtures.mkdir()
    (fixtures / "Issue508-DAC2020_bm01.dsn").write_text((DATA / "mini.dsn").read_text())
    (fixtures / "Issue143-rpi_splitter_mod.dsn").write_text((DATA / "mini.dsn").read_text())
    (fixtures / "Issue999-notes.json").write_text("{}")
    monkeypatch.setattr(corpus, "CORPUS", tmp_path / "corpus")
    boards = init_from_fixtures(fixtures)
    ids = {b.id for b in boards}
    assert ids == {"dac2020-bm01", "issue143-rpi_splitter_mod"}
    bm01 = next(b for b in boards if b.id == "dac2020-bm01")
    assert bm01.referee == "none"
    assert {"regression", "dac2020", "hard"} <= set(bm01.tiers)
    assert bm01.nets == 3 and bm01.layers == 2
    assert (tmp_path / "corpus" / "dsn" / "Issue508-DAC2020_bm01.dsn").exists()
    assert (tmp_path / "corpus" / "manifest.json").exists()


def test_select_by_tier_and_ids():
    a = Board(id="a", source="dsn/a.dsn", origin="freerouting-fixtures", referee="java-drc",
              tiers=["canary"], nets=1, layers=2)
    b = Board(id="b", source="dsn/b.dsn", origin="freerouting-fixtures", referee="java-drc",
              tiers=["hard"], nets=1, layers=2)
    x = Board(id="x", source="dsn/x.dsn", origin="pcbench", referee="kicad",
              tiers=["d3-a"], nets=1, layers=2, status="excluded")
    assert [s.id for s in select([a, b, x], tier="canary")] == ["a"]
    assert [s.id for s in select([a, b, x], ids=["b", "a"])] == ["b", "a"]
    assert [s.id for s in select([a, b, x])] == ["a", "b"]  # excluded boards dropped


def test_manifest_roundtrip(tmp_path, monkeypatch):
    monkeypatch.setattr(corpus, "CORPUS", tmp_path)
    boards = [Board(id="a", source="dsn/a.dsn", origin="freerouting-fixtures",
                    referee="java-drc", tiers=["canary"], nets=4, layers=2,
                    expected_duration_s=3.5)]
    corpus.save_manifest(boards)
    loaded = corpus.load_manifest()
    assert loaded == boards
    assert json.loads((tmp_path / "manifest.json").read_text())["boards"][0]["id"] == "a"


def test_save_manifest_is_atomic_no_temp_file_left_behind(tmp_path, monkeypatch):
    """save_manifest writes via a temp file + os.replace (mirrors bench.runner._save_meta) so
    a reader never sees a truncated manifest.json. Calling it twice in a row (standing in for
    concurrent-ish writers) must leave manifest.json.tmp cleaned up, not lying around."""
    monkeypatch.setattr(corpus, "CORPUS", tmp_path)
    boards_a = [Board(id="a", source="dsn/a.dsn", origin="freerouting-fixtures",
                      referee="java-drc", tiers=["canary"], nets=4, layers=2)]
    boards_b = [Board(id="a", source="dsn/a.dsn", origin="freerouting-fixtures",
                      referee="java-drc", tiers=["canary"], nets=4, layers=2),
                Board(id="b", source="dsn/b.dsn", origin="freerouting-fixtures",
                      referee="java-drc", tiers=["hard"], nets=5, layers=2)]

    corpus.save_manifest(boards_a)
    corpus.save_manifest(boards_b)

    assert not (tmp_path / "manifest.json.tmp").exists()
    assert sorted(tmp_path.glob("*.tmp")) == []
    loaded = corpus.load_manifest()
    assert {b.id for b in loaded} == {"a", "b"}


def test_connections_field_roundtrips_and_defaults_to_none(tmp_path, monkeypatch):
    monkeypatch.setattr(corpus, "CORPUS", tmp_path)
    boards = [
        Board(id="a", source="dsn/a.dsn", origin="freerouting-fixtures", referee="java-drc",
              tiers=[], nets=4, layers=2, connections=17),
        Board(id="b", source="dsn/b.dsn", origin="freerouting-fixtures", referee="java-drc",
              tiers=[], nets=4, layers=2),  # connections omitted -> defaults to None
    ]
    corpus.save_manifest(boards)
    loaded = {b.id: b for b in corpus.load_manifest()}
    assert loaded["a"].connections == 17
    assert loaded["b"].connections is None
    # backward compatible: a manifest written before `connections` existed (no key at all)
    # still loads, defaulting to None.
    raw = json.loads((tmp_path / "manifest.json").read_text())
    del raw["boards"][1]["connections"]
    (tmp_path / "manifest.json").write_text(json.dumps(raw))
    reloaded = {b.id: b for b in corpus.load_manifest()}
    assert reloaded["b"].connections is None


def test_kicad_path_resolves_relative_and_accepts_absolute(tmp_path, monkeypatch):
    monkeypatch.setattr(corpus, "CORPUS", tmp_path)
    b = Board(id="x", source="pcbench/x/unrouted.dsn", origin="pcbench", referee="kicad", tiers=[],
              nets=1, layers=2, kicad={"stripped": "pcbench/x/stripped.kicad_pcb",
                                      "raw": str(tmp_path / "elsewhere" / "raw.kicad_pcb"),
                                      "missing": None})
    assert b.kicad_path("stripped") == tmp_path / "pcbench" / "x" / "stripped.kicad_pcb"
    assert b.kicad_path("raw") == tmp_path / "elsewhere" / "raw.kicad_pcb"
    assert b.kicad_path("missing") is None
    assert b.kicad_path("nonexistent_key") is None
