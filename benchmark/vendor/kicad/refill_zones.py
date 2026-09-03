"""Re-fill every copper zone in a `.kicad_pcb` with KiCad's modern `ZONE_FILLER`, then save.

PCBench/legacy KiCad-4/5 boards were saved with KiCad's old zone-fill strategy, which KiCad
7+ no longer treats as a valid fill -- `kicad-cli pcb drc` reports it as a swarm of
`clearance` errors ("Legacy zone fill strategy is not supported anymore") that are an
artefact of the stale fill data, not a real design problem. Re-filling with `ZONE_FILLER`
before DRC removes that noise (see README.md "KiCad referee status" and the PCBench section
for measured before/after counts, e.g. 1Bitsy: 254 -> 46 DRC errors, all the rest
non-routing).

Needs KiCad's bundled Python (`pcbnew` on `sys.path`), unlike `legacy_rules.py`:

    KP=/Applications/KiCad/KiCad.app/Contents/Frameworks/Python.framework/Versions/Current/bin/python3
    $KP vendor/kicad/refill_zones.py <in.kicad_pcb> <out.kicad_pcb>

`<in.kicad_pcb>` and `<out.kicad_pcb>` may be the same path (refill in place). Prints
``{"zones": n}`` to stdout on success (exit 0); exits 1 with a message on stderr on failure.

**`board.Save()` can silently write a same-stem `.kicad_pro`/`.kicad_prl` next to the output
board**, built from pcbnew's own in-memory *default* board-setup constraints (observed: a
board refilled with no project alongside it gets a fresh `.kicad_pro` with
`min_hole_clearance: 0.25`, KiCad's built-in default, even though the correct net-class
values -- e.g. this board's real 0.149mm clearance -- are read back accurately). If a caller
had already written a project there with different (e.g. deliberately zeroed) rules, this
would silently replace it and DRC would then run under the wrong constraints -- exactly the
bug this note exists to prevent recurring. So after `Save()`, this module removes any
`.kicad_pro`/`.kicad_prl` that *appeared* as a result of the call (didn't exist right before
it): callers that need a specific project in place for DRC must (re)write it *after* calling
this, not before -- see `bench/corpus_pcbench.py::_import_board` and
`bench/referee/kicad.py::run` for the two places that matters.
"""
from __future__ import annotations

import json
import sys
from pathlib import Path


def refill(src: str, dst: str) -> int:
    import pcbnew  # type: ignore

    dst_path = Path(dst)
    dst_pro = dst_path.with_suffix(".kicad_pro")
    dst_prl = dst_path.with_suffix(".kicad_prl")
    pro_existed = dst_pro.exists()
    prl_existed = dst_prl.exists()

    board = pcbnew.LoadBoard(src)
    zones = board.Zones()
    filler = pcbnew.ZONE_FILLER(board)
    filler.Fill(zones)
    board.Save(dst)

    removed = []
    if not pro_existed and dst_pro.exists():
        dst_pro.unlink()
        removed.append(str(dst_pro))
    if not prl_existed and dst_prl.exists():
        dst_prl.unlink()
        removed.append(str(dst_prl))
    if removed:
        print(f"removed stray project file(s) written by pcbnew's Save(): {removed}", file=sys.stderr)

    return len(zones)


def main(argv: list[str] | None = None) -> int:
    argv = sys.argv[1:] if argv is None else argv
    if len(argv) != 2:
        print("usage: refill_zones.py <in.kicad_pcb> <out.kicad_pcb>", file=sys.stderr)
        return 2
    try:
        n = refill(argv[0], argv[1])
    except Exception as e:  # noqa: BLE001 -- surface any pcbnew failure as a clean exit 1
        print(f"refill_zones.py failed on {argv[0]}: {e}", file=sys.stderr)
        return 1
    print(json.dumps({"zones": n}))
    return 0


if __name__ == "__main__":
    sys.exit(main())
