KiCad 10.0.3 reference for the rounded-mask-corner regression.

A 1 mm square pad at (10,10) mm has 0.2 mm mask expansion. A 0.1 mm track
starts at (10.74,10.6) mm and runs horizontally outward. Its copper gap to
the pad corner is 0.21 mm, so it does not violate the mask clearance. KiCad
reports no solder-mask bridge. An octagonal enlargement approximation wrongly
reported a bridge; the new distance refinement uses the circular track end.

The Rust test reflects Y and translates the geometry to the origin. It also
covers a circular via near the corner.

Regenerate with `kicad-cli pcb drc --format json -o drc.json mask.kicad_pcb`.
