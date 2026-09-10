KiCad 10.0.3 reference for `tests/solder_mask.rs`.

The 1 mm square PTH pad at (10, 10) mm has a 0.2 mm front mask expansion and
no back mask aperture. Three 0.2 mm tracks have 0.19 mm copper clearance:
foreign-net front and back tracks, and a same-net front track. Copper clearance
is 0.18 mm. KiCad reports one front mask bridge, with no copper-clearance error.
Other report entries concern the minimal fixture's missing outline/footprint
metadata and dangling tracks; they are not the mask oracle.

The Rust geometry is translated to the origin, with 10000 units per mm.

Regenerate the reference with:

```sh
kicad-cli pcb drc --format json -o drc.json mask.kicad_pcb
```
