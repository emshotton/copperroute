# Easyduino USB locating-hole clearance investigation

Investigated on 2026-09-06 using the browser branch based on main's merged PR #13,
its rebuilt WASM, and KiCad CLI 10.0.3. Both browser examples produce eight
`hole_clearance` findings before routing and the identical eight afterward.

## Result

These are not all false positives, and the absence of findings in the original
KiCad reports is not proof that their geometry meets the project minimum.
Both projects specify 0.25 mm hole clearance and have empty DRC exclusion lists.

| Pads beside J1's two 0.65 mm locating holes | Uno: rectangular pads | Nano: rounded rectangular pads |
| --- | --- | --- |
| GND: A1/B12 and A12/B1 | 0.1751 mm; below minimum | 0.1944 mm; below minimum |
| Vbus: A4/B9 and A9/B4 | 0.2099 mm; below minimum | 0.2586 mm; above minimum |

Each slash-separated pair occupies the same location. Uno therefore has four
physical gaps below the minimum, represented by eight pad/hole reports. Nano
has two physical gaps below the minimum, represented by four reports; the other
four browser findings result from replacing rounded rectangles with rectangles.

The analytical distances use the source footprint dimensions: 0.6 x 1.24 mm
pads, a 0.325 mm hole radius, and center offsets of 1.12 mm in one axis and
0.31 mm (GND) or 0.49 mm (Vbus) in the other. Nano's roundrect ratio is 0.25,
so its corner radius is 0.15 mm. Point-to-rectangle/corner-circle distances give
the values above. KiCad's reported violating gaps agree to four decimal places.

## KiCad experiment

Copied each original PCB and its same-stem project into a temporary directory.
Ran `kicad-cli pcb drc --format json --severity-all` on the copies. Then moved
only the two NPTH pad records to the end of J1's footprint S-expression. Pad
positions, orientations, sizes, UUIDs, nets, footprint membership, and project
rules were unchanged.

| Input | KiCad 10.0.3 hole-clearance findings |
| --- | --- |
| Original Uno | 0 |
| Uno with NPTH pad records last | 8 |
| Original Nano | 0 |
| Nano with NPTH pad records last | 4 |

A preliminary experiment moved the holes into separate footprints without
moving their geometry. That yielded eight Uno findings and no Nano findings,
which initially suggested a footprint exemption. The within-footprint reorder
experiment disproved that explanation: pair visitation order matters.

KiCad 10.0.3's `testPadClearances` deduplicates pad pairs, while
`testPadAgainstItem` checks the first pad's copper against the second pad's hole
without the swapped pad-hole direction. If the hole is visited first, the pair
can be marked checked without testing the SMD copper against it. The newer
10.0 branch explicitly adds the swapped-direction check. There is no basis here
for suppressing every hole/copper pair within a footprint.

Sources:
- https://github.com/KiCad/kicad-source-mirror/blob/10.0.3/pcbnew/drc/drc_test_provider_copper_clearance.cpp
- https://github.com/KiCad/kicad-source-mirror/blob/10.0/pcbnew/drc/drc_test_provider_copper_clearance.cpp

## Rust/browser implications

`web/kicad.js` replaces roundrect pads with rectangles and creates one synthetic
component per pad to support individual pad rotation. Preserving original
footprint identity is useful, but would not justify a blanket same-footprint
exemption.

`fr-drc::checks::geometry::hole_from` bounds circular holes with octagons;
`gap_below` and `Board::calculate_clearance_between_two_shapes` use enlarged
polygon intersections. This is a conservative clearance measure, not exact
Euclidean distance: all eight reported gaps are approximately 0.1751 mm even
though Uno's Vbus gaps are approximately 0.2099 mm. `estimated: false` currently
refers to drill metadata, not the fidelity of those geometric approximations.

Follow-up work should preserve exact rounded-pad geometry, measure physical
circle/pad distances for DRC, and distinguish pre-existing footprint findings
from routing-induced findings. Retain the genuine GND findings and Uno's Vbus
findings under the current project rule. Changing the footprint or its approved
clearance requirement is a separate board-design decision.

This corrects earlier notes describing all eight findings as fixed-footprint
approximation artifacts or treating a clean KiCad 10.0.3 report as confirmation
of clearance compliance. No routing code or source example boards were changed
by this investigation.

## Implemented follow-up

The browser now passes `roundRectRatio` into the native importer. Rust retains
corner-radius metadata independently of the conservative routing rectangle and
uses physical circle/segment/pad distances for hole-clearance checks. Five-pass
browser regressions now report eight Uno and four Nano findings, all present
before routing, with zero new findings. The details panel shows baseline/new
counts, measured and required gaps, layers and positions. Routing and non-hole
DRC remain conservative for rounded pads. Via-in-pad attachment behavior is
unchanged by this work.
