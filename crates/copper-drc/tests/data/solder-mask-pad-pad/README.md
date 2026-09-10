# KiCad 10.0.6 mask reference

Two 1 mm rectangular pads on different nets have centers 1.3 mm apart and front mask expansions of 0.2 mm each. Copper clearance is 0.3 mm, above the 0.18 mm rule. KiCad reports one front mask bridge.

Derived from the repository synthetic pad-track fixture. Regenerate with:

```sh
kicad-cli pcb drc --format json --all-track-errors -o drc.json mask.kicad_pcb
```

The Rust fixture translates the geometry to the origin at 10000 units/mm.
