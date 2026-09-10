# Unassigned copper net mask oracle

Derived from `solder-mask-pad-track` by removing the two N2 track assignments. The pad retains N1. KiCad 10.0.6 reports 1 front mask bridge(s), zero copper-clearance findings, and no back mask bridge.

```sh
kicad-cli pcb drc --format json --all-track-errors -o drc.json mask.kicad_pcb
```

The Rust equivalent uses 10000 units/mm and is translated to the origin. These checks failed while the internal checker skipped tracks without a net.
