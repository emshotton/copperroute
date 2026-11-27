"""Runs the plugin action against a stand-in for KiCad's API, with the real router."""

from __future__ import annotations

import sys
from pathlib import Path

import pytest

PLUGIN = Path(__file__).resolve().parents[1] / "copperroute"
REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(PLUGIN))

import copperroute_action  # noqa: E402


class FakeNet:
    def __init__(self, name):
        self.name = name


class FakeItem:
    def __init__(self, net):
        self.net = FakeNet(net)


class FakeProject:
    def __init__(self, path):
        self.path = str(path)


class FakeBoard:
    def __init__(self, text, project_dir, selection=()):
        self.text = text
        self.project_dir = project_dir
        self.name = "board.kicad_pcb"
        self.selection = list(selection)
        self.created = []
        self.removed = []
        self.commits = []
        self.refilled = False

    def get_project(self):
        return FakeProject(self.project_dir)

    def get_as_string(self):
        return self.text

    def get_selection(self):
        return self.selection

    def get_nets(self):
        from kipy.board_types import Net

        return [Net(name=name) for name in ("A", "B")]

    def begin_commit(self):
        return "commit"

    def push_commit(self, commit, message=""):
        self.commits.append(("push", message))

    def drop_commit(self, commit):
        self.commits.append(("drop", ""))

    def remove_items_by_id(self, ids):
        self.removed.extend(i.value for i in ids)

    def create_items(self, items):
        self.created.extend(items)
        return items

    def refill_zones(self):
        self.refilled = True


BOARD = (REPO / "crates/copper-kicad/tests/data/two-nets.kicad_pcb").read_text()


@pytest.fixture
def fake_kicad(monkeypatch, tmp_path):
    holder = {}

    class FakeKiCad:
        def __init__(self, *args, **kwargs):
            pass

        def get_board(self):
            return holder["board"]

    import kipy

    monkeypatch.setattr(kipy, "KiCad", FakeKiCad)

    def make(selection=()):
        holder["board"] = FakeBoard(BOARD, tmp_path, selection)
        return holder["board"]

    return make


def test_routing_adds_tracks_in_one_commit(fake_kicad):
    from kipy.board_types import Track

    board = fake_kicad()
    assert copperroute_action.main(selection_only=False, rip_up=False) == 0
    assert board.commits == [("push", "CopperRoute autoroute")]
    tracks = [item for item in board.created if isinstance(item, Track)]
    assert tracks and all(t.net.name in ("A", "B") for t in tracks)
    assert board.removed == []
    assert board.refilled


def test_reroute_of_the_selection_removes_only_its_unlocked_tracks(fake_kicad):
    board = fake_kicad(selection=[FakeItem("B")])
    assert copperroute_action.main(selection_only=None, rip_up=True) == 0
    assert board.removed == ["loose-b"]
    assert all(item.net.name == "B" for item in board.created)


def test_selection_only_needs_a_selection(fake_kicad):
    fake_kicad()
    assert copperroute_action.main(selection_only=True, rip_up=False) == 1


def test_settings_file_overrides_the_defaults(tmp_path):
    (tmp_path / "copperroute.json").write_text('{"maxPasses": 3, "respectZones": false}')
    settings = copperroute_action.load_settings(tmp_path)
    assert settings["maxPasses"] == 3 and settings["respectZones"] is False
    assert settings["timeoutSeconds"] == 300


def test_vias_get_a_padstack_kicad_accepts():
    from kipy.board_types import Net, Via

    changes = {
        "addTracks": [],
        "addVias": [
            {
                "position": {"x": 1.5, "y": 2.5},
                "diameter": 0.6,
                "drill": 0.3,
                "startLayer": "F.Cu",
                "endLayer": "B.Cu",
                "net": "A",
            }
        ],
    }
    (via,) = copperroute_action.build_items(changes, {"A": Net(name="A")})
    assert isinstance(via, Via)
    assert via.diameter == 600_000 and via.drill_diameter == 300_000
    assert (via.position.x, via.position.y) == (1_500_000, 2_500_000)
    assert via.net.name == "A"
