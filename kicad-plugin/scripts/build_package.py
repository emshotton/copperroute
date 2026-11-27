"""Builds the Plugin and Content Manager zip and prints the fields its repository entry needs."""

from __future__ import annotations

import hashlib
import json
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PLUGIN = ROOT / "copperroute"


def main() -> Path:
    metadata = json.loads((ROOT / "metadata.json").read_text())
    version = metadata["versions"][0]["version"]
    dist = ROOT / "dist"
    dist.mkdir(exist_ok=True)
    target = dist / f"copperroute-kicad-{version}.zip"
    install_size = 0
    with zipfile.ZipFile(target, "w", zipfile.ZIP_DEFLATED) as archive:
        archive.write(ROOT / "metadata.json", "metadata.json")
        archive.write(PLUGIN / "icons" / "route-48.png", "resources/icon.png")
        for path in sorted(PLUGIN.rglob("*")):
            if path.is_file() and "__pycache__" not in path.parts:
                archive.write(path, f"plugins/{path.relative_to(PLUGIN).as_posix()}")
                install_size += path.stat().st_size
    data = target.read_bytes()
    print(json.dumps({
        "download_url": f"https://github.com/emshotton/copperroute/releases/download/kicad-plugin-{version}/{target.name}",
        "download_sha256": hashlib.sha256(data).hexdigest(),
        "download_size": len(data),
        "install_size": install_size,
    }, indent=2))
    return target


if __name__ == "__main__":
    main()
