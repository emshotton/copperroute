KiCad 10.0.3 reference for `mask_to_copper_rule_finds_tracks_outside_the_aperture`.

As in `../solder-mask-pad-track`, but foreign-net tracks move to x=10.83 mm
and the project requires 0.05 mm solder-mask-to-copper clearance. Copper gap
is 0.23 mm, aperture gap is 0.03 mm. KiCad reports one front mask bridge and
no copper-clearance error. The Rust test imports this project's rule and uses
the same pad and offending track translated to the origin (10000 units/mm).

Regenerate with `kicad-cli pcb drc --format json -o drc.json mask.kicad_pcb`.
