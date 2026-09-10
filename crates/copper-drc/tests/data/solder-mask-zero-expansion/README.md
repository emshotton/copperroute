# Zero-expansion mask collision fixtures

These independent KiCad 10.0.3 inputs use two 1 mm rectangular pads on the front copper/mask side, different nets, and zero mask margin. Coincident pads produce one solder-mask bridge; pads exactly 1 mm apart (touching at their edges) also produce one. Pads 1.1 mm apart produce no mask bridge. All reports retain unrelated copper, outline and silk findings. These are diagnostic collision cases, not fabrication-ready boards.

Command: `kicad-cli pcb drc --format json --output <report>.json <board>.kicad_pcb`.

The Rust checker originally skipped overlapping rectangles because its unsigned core gap equals the zero required distance. Allowing that case exposed a second failure: its positive-area intersection check skipped exact contact. The change retains the existing gap/intersection tests and adds a zero-distance contact case specific to solder-mask checking.
