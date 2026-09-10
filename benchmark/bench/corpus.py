"""Board corpus: manifest model, tier selection, DSN introspection, fixture import."""
from __future__ import annotations

import json
import os
import re
import shutil
from dataclasses import asdict, dataclass, field
from pathlib import Path

from bench.paths import CORPUS  # re-exported so tests can monkeypatch bench.corpus.CORPUS

HARD_IDS = {"dac2020-bm01", "dac2020-bm05", "dac2020-bm06",
            "issue555-bbd_mars-64", "issue555-cnh_functional_tester_1"}
CANARY_IDS = {"dac2020-bm07", "dac2020-bm08", "issue558-dev-board", "issue143-rpi_splitter_mod"}

D3_RANGES = [("d3-a", 2, 13), ("d3-b", 14, 42), ("d3-c", 43, 451)]


def assign_d3_tier(nets: int) -> str | None:
    """Bucket a board by net count into the paper's D3 tiers (ranges overlap; the
    smallest containing range wins by being listed first)."""
    for name, lo, hi in D3_RANGES:
        if lo <= nets <= hi:
            return name
    return None


@dataclass
class Board:
    id: str
    source: str                         # relative to CORPUS
    origin: str                         # "freerouting-fixtures" | "pcbench"
    referee: str                        # "kicad" | "none"; historical manifests may use "java-drc"
    tiers: list[str] = field(default_factory=list)
    nets: int = 0
    layers: int = 0
    connections: int | None = None      # Java board_statistics.connections.maximum_count (see bench.metrics)
    expected_duration_s: float | None = None
    kicad: dict | None = None           # {"raw":..., "stripped":..., "ground_truth":..., "project":...}
                                         # values are CORPUS-relative posix paths (or absolute, still accepted)
    status: str = "ok"                  # "ok" | "excluded"
    reason: str = ""
    license: dict | None = None         # {"spdx_id": ..., "status": "licensed" | "licensed-unclassified"
                                         #  | "unlicensed" | "source-missing" | "unknown"} from the
                                         # board's PCBench metadata.json

    @property
    def path(self) -> Path:
        return CORPUS / self.source

    def kicad_path(self, key: str) -> Path | None:
        """Resolve a `self.kicad[key]` value to a Path. Values are stored CORPUS-relative
        (posix); an absolute value (e.g. from a test fixture) is accepted as-is."""
        value = (self.kicad or {}).get(key)
        if not value:
            return None
        p = Path(value)
        return p if p.is_absolute() else CORPUS / p


def manifest_path() -> Path:
    return CORPUS / "manifest.json"


def load_manifest() -> list[Board]:
    p = manifest_path()
    if not p.exists():
        return []
    data = json.loads(p.read_text())
    return [Board(**b) for b in data["boards"]]


def save_manifest(boards: list[Board]) -> None:
    CORPUS.mkdir(parents=True, exist_ok=True)
    payload = {"schema_version": 1, "boards": [asdict(b) for b in sorted(boards, key=lambda b: b.id)]}
    # os.replace is atomic on POSIX and Windows, so a concurrent reader never sees a
    # truncated/partial manifest.json. Mirrors bench.runner._save_meta.
    tmp = manifest_path().with_suffix(".json.tmp")
    tmp.write_text(json.dumps(payload, indent=2) + "\n")
    os.replace(tmp, manifest_path())


def select(boards: list[Board], tier: str | None = None, ids: list[str] | None = None) -> list[Board]:
    if ids:
        by_id = {b.id: b for b in boards}
        missing = [i for i in ids if i not in by_id]
        if missing:
            raise KeyError(f"unknown board ids: {missing}")
        return [by_id[i] for i in ids]
    out = [b for b in boards if b.status == "ok"]
    if tier:
        out = [b for b in out if tier in b.tiers]
    return out


_NET_RE = re.compile(r"\(net\s+(\"[^\"]*\"|[^\s()]+)")
_LAYER_RE = re.compile(r"\(layer\s+(\"[^\"]*\"|[^\s()]+)\s*\(type\s+signal\)")


def dsn_info(path: Path) -> tuple[int, int]:
    """(net count, signal layer count) by lightweight scanning of a DSN file."""
    text = path.read_text(errors="replace")
    net_start = text.find("(network")
    nets = 0
    if net_start >= 0:
        section = text[net_start:]
        cut_class = section.find("(class")
        cut_wiring = section.find("(wiring")
        if cut_class >= 0 and cut_wiring >= 0:
            cut = min(cut_class, cut_wiring)
        elif cut_class >= 0:
            cut = cut_class
        elif cut_wiring >= 0:
            cut = cut_wiring
        else:
            cut = -1
        if cut > 0:
            section = section[:cut]
        nets = len(_NET_RE.findall(section))
    layers = len(_LAYER_RE.findall(text))
    return nets, layers


def board_id_for(filename: str) -> str:
    stem = Path(filename).stem
    m = re.match(r"Issue508-DAC2020_(bm\d+)$", stem)
    if m:
        return f"dac2020-{m.group(1)}"
    return stem.lower()


def init_from_fixtures(fixtures_dir: Path) -> list[Board]:
    """Copy every *.dsn from the Java repo's fixtures into corpus/dsn and build manifest entries."""
    dst = CORPUS / "dsn"
    dst.mkdir(parents=True, exist_ok=True)
    existing = {b.id: b for b in load_manifest()}
    boards: list[Board] = [b for b in existing.values() if b.origin != "freerouting-fixtures"]
    for src in sorted(fixtures_dir.glob("*.dsn")):
        shutil.copyfile(src, dst / src.name)
        bid = board_id_for(src.name)
        nets, layers = dsn_info(src)
        tiers = ["regression"]
        if bid.startswith("dac2020-"):
            tiers.append("dac2020")
        if bid in HARD_IDS:
            tiers.append("hard")
        if bid in CANARY_IDS:
            tiers.append("canary")
        prev = existing.get(bid)
        boards.append(Board(id=bid, source=f"dsn/{src.name}", origin="freerouting-fixtures",
                            referee="none", tiers=tiers, nets=nets, layers=layers,
                            expected_duration_s=prev.expected_duration_s if prev else None))
    save_manifest(boards)
    return [b for b in boards if b.origin == "freerouting-fixtures"]
