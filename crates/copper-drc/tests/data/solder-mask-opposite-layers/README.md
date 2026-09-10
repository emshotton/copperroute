# Opposite-side connector-pad mask apertures

Generated from a minimal two-pad connector fixture, matching the layer declaration used by the real fifogfx C64 cartridge edge connector: copper is on one side while mask openings are declared on both. Pads overlap in XY, have distinct nets, and occupy opposite copper sides. KiCad 10.0.3 reports two solder-mask bridges, one on each mask side.

KiCad also reports questionable-padstack warnings for this configuration. Those warnings, two silk findings, and the missing-outline error are retained in the unmodified report; they are not represented as a clean fabrication example. The test specifically verifies the two mask findings KiCad does produce. This is a checker-accuracy fixture, not justification for routing into invalid geometry.

Command: `kicad-cli pcb drc --format json --output drc.json mask.kicad_pcb`.

`control.kicad_pcb` declares each aperture only on its pad copper side. KiCad reports zero mask bridges and zero padstack warnings; `control-drc.json` retains its remaining unrelated findings.
