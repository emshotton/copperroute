"""Print {"vias": n, "wirelength_mm": x} for a .kicad_pcb. Run with KiCad's bundled python."""
import json
import sys

if len(sys.argv) != 2:
    print(f"usage: {sys.argv[0]} <board.kicad_pcb>", file=sys.stderr)
    sys.exit(2)

import pcbnew  # type: ignore

board = pcbnew.LoadBoard(sys.argv[1])
vias, length_nm = 0, 0
for t in board.GetTracks():
    if t.GetClass() == "PCB_VIA":
        vias += 1
    else:
        length_nm += t.GetLength()
print(json.dumps({"vias": vias, "wirelength_mm": round(length_nm / 1e6, 4)}))
