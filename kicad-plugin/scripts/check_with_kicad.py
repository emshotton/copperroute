"""Routes example boards with the copperroute CLI and checks the results with KiCad's own DRC.

A routed board must not have more DRC errors than its source board, and must not have more
unconnected items. Usage: check_with_kicad.py COPPERROUTE BOARD.kicad_pcb [...]
"""

from __future__ import annotations

import json
import shutil
import subprocess
import sys
import tempfile
from collections import Counter
from pathlib import Path


def drc(board: Path, report: Path) -> tuple[Counter, int]:
    subprocess.run(
        ["kicad-cli", "pcb", "drc", "--format", "json", "--severity-error",
         "-o", str(report), str(board)],
        check=True,
        capture_output=True,
    )
    data = json.loads(report.read_text())
    errors = Counter(v["type"] for v in data.get("violations", []))
    return errors, len(data.get("unconnected_items", []))


def main(copperroute: str, boards: list[str]) -> int:
    failures = 0
    for source in map(Path, boards):
        with tempfile.TemporaryDirectory() as tmp:
            work = Path(tmp)
            for sibling in source.parent.glob(source.stem + ".*"):
                shutil.copy(sibling, work / sibling.name)
            board = work / source.name
            routed = work / f"{source.stem}-routed.kicad_pcb"
            for suffix in (".kicad_pro", ".kicad_dru"):
                if (work / f"{source.stem}{suffix}").exists():
                    shutil.copy(work / f"{source.stem}{suffix}", work / f"{routed.stem}{suffix}")
            subprocess.run(
                [copperroute, "route", str(board), "-o", str(routed), "--respect-zones",
                 "--max-passes", "10", "--timeout", "300"],
                check=True,
            )
            before, unconnected_before = drc(board, work / "before.json")
            after, unconnected_after = drc(routed, work / "after.json")
            new_errors = after - before
            print(f"{source.name}: errors {sum(before.values())} -> {sum(after.values())}, "
                  f"unconnected {unconnected_before} -> {unconnected_after}")
            if new_errors or unconnected_after > unconnected_before:
                failures += 1
                print(f"  new errors: {dict(new_errors)}")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1], sys.argv[2:]))
