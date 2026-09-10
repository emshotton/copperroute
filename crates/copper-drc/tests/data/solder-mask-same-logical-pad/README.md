# Logical pad identity oracle

Both physical pads carry the same nonempty pad number in the same footprint. Both are unnetted. KiCad 10.0.6 reports 0 mask bridges. Derived from solder-mask-pad-pad with net assignments removed and pad numbers changed.

```sh
kicad-cli pcb drc --format json --all-track-errors -o drc.json mask.kicad_pcb
```

The Rust test models the browser adapter splitting physical pads into independently placed components, preserving their source footprint and source pad number.
