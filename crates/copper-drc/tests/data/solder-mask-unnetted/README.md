# Unnetted pad mask oracle

Derived from the existing solder-mask-pad-track synthetic fixture by removing the pad net assignment. The two front copper tracks now both have different nets from the unnetted pad. KiCad 10.0.3 and 10.0.6 each report two front solder-mask bridges, zero copper-clearance findings, and no back-layer bridge.

The committed report is recorded with KiCad 10.0.6:

```sh
kicad-cli pcb drc --format json --all-track-errors -o drc.json mask.kicad_pcb
```

The Rust regression uses the same geometry translated to the origin at 10000 units/mm. The existing checker skipped unnetted pads and returned zero mask findings; the regression failed before removing that skip.
