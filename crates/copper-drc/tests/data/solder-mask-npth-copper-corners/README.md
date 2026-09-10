# Non-plated pad mask oracle

A non-plated square pad retains copper at the corners outside its circular drill. KiCad reports two front mask bridges. Both use a 1 mm pad, 1 mm drill, 0.2 mm front mask expansion and two foreign front tracks, plus one back track. Derived from solder-mask-unnetted.

Recorded independently with KiCad 10.0.6:

```sh
kicad-cli pcb drc --format json --all-track-errors -o drc.json mask.kicad_pcb
```

The Rust test imports matching native JSON geometry and compares mask findings only; these fixtures also intentionally contain hole-clearance and non-routing findings.
