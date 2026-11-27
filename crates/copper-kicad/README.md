# copper-kicad

Routes a KiCad board given as its `.kicad_pcb` text, with the project's `.kicad_pro` and
`.kicad_dru` when available, and returns the edits that turn it into the routed board: tracks
and vias to add, and the KiCad IDs of items that rip-up removed. Existing copper the router kept
is matched geometrically, so it is never written twice.

`route` takes a progress callback that can stop the run; `route_json` is the same call in JSON
form for other languages. The KiCad plugin uses it through `bindings/python`, and
`copperroute route board.kicad_pcb -o routed.kicad_pcb` uses it from the command line.
