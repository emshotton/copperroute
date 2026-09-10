# KiCad 10.0.6 mask reference

Two 1 mm rectangular pads on different nets have centers 1.5 mm apart and front mask expansions of 0.2 mm each. A 0.1 mm mask strip remains, below the 0.2 mm minimum stored in PCB setup. KiCad reports one front mask bridge. A historical project-only setting did not enable this rule in KiCad 10.0.6.

Derived from the repository synthetic pad-track fixture. Regenerate with:

```sh
kicad-cli pcb drc --format json --all-track-errors -o drc.json mask.kicad_pcb
```

The Rust fixture translates the geometry to the origin at 10000 units/mm.
