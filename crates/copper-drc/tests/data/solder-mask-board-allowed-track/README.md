# Board-level permission does not excuse a foreign track

KiCad 10.0.6 reports one mask bridge on this pad-track fixture even with `allow_soldermask_bridges_in_footprints yes`. The permission applies to pairs inside the same footprint, not to a foreign routing track. This is used alongside the per-footprint permission oracle.

```sh
kicad-cli pcb drc --format json --all-track-errors -o drc.json mask.kicad_pcb
```
