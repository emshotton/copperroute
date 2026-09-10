# KiCad 10.0.6 pad-pair control

Derived from `solder-mask-pad-pad`. This variant changes the second pad to the same net, moves it to a clear diagonal corner, or permits mask bridges inside footprints (as indicated by the directory name). KiCad reports zero solder-mask bridges.

```sh
kicad-cli pcb drc --format json --all-track-errors -o drc.json mask.kicad_pcb
```

The Rust native-JSON test reproduces the geometry and permission. It caught the missing board-wide footprint exception before that setting was carried through the model.
