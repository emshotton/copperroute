# KiCad drill metadata and routing clearance fixes

KiCad JSON imports discarded pad drill diameters, ignored net-class via drills,
and exported via holes as half the copper diameter. DRC then used a padstack-name
heuristic or a 45% estimate. For an Uno routing example, this produced 34 false
undersized-drill findings: 32 correctly specified 0.30 mm vias and two 0.65 mm
non-plated USB mounting holes, against a 0.30 mm minimum.

Padstacks now carry optional drill diameters in board units, an approximation
flag, and the existing hole-only flag. The importer sets these for pads, existing
vias, session vias, and the net-class via templates used by the router. Padstack
sharing includes drill diameter, plating and approximation status. The writer
exports explicit via drills. Legacy inputs without metadata retain their old
fallback behavior. Unplated holes do not require a copper annular ring. The JSON
pad extensions `nonPlated` and `drillEstimated` default to false.

Project clearances also need to reach the routing search. The project import
attaches DRC constraints after board preparation, and the resolved router
settings contain a zero hole-clearance default. Board preparation and pipeline
entry therefore raise the hole clearance and every board-edge clearance cell
that sits below the project minimum, in board units. Values already above the
minimum, whether from the router settings or a rules file, are left alone, and
the search trees are rebuilt only when something was raised.

DRC now uses physical pin/via shapes instead of search-tree shapes inflated for
routing hole clearance. Bare holes participate in hole-clearance checks, but do
not count as copper pads in copper-to-copper checks.

## Validation and limits

Focused regressions cover:

- Equal copper pads with different drills retain separate padstacks.
- Default and additional net-class via drills, and existing via drills, survive export.
- Exact compliant holes pass while a genuinely undersized via still fails.
- Non-plated mounting holes are exempt from annular-ring requirements.
- Changing routing obstacle margins does not change DRC's physical dimensions.
- Project minima floor default and explicit routing clearances with correct units.
- A real routing attempt detours around a mounting hole after constraints are
  attached following initial board preparation.

The reader golden changes because two previously conflated padstacks in
`interf-u` have different drill diameters. The writer golden changes for
`complex-hierarchy-design` because its actual drills replace estimates. Original
Java transcripts are unchanged; the port's documented divergence list explains
these differences.

The Uno investigation uses a four-layer board restricted to outer copper, five
routing passes, one worker and no optimizer. Board files and the browser adapter
are not part of this branch. The eight USB pad/hole findings already present in
the imported input remain; the browser adapter simplifies pad geometry, so they
must not be interpreted as eight new routing errors or suppressed generically.
A retained fanout via on an unfinished net may still produce a dangling-via
warning. Removing that warning or completing all nets is not the same as fixing
clearance enforcement. Slotted-hole geometry remains approximate.

No browser, WASM, UI, asset or dependency changes are included in this branch.

Independent validation of the revised Uno download with KiCad, after refilling
zones, reports no clearance/drill errors, one retained dangling-via warning, and
four unconnected items. The native Rust result contains only the eight fixed
USB-footprint findings, with no routing-induced DRC violations. The prior run
had one KiCad hole-clearance error and three unconnected items; enforcing the
minimum can leave more work for the router within the same five-pass budget.

## Nano follow-up

The same branch was checked with the Easyduino Nano, its project rules, all four
signal copper layers, five passes, one worker, a 60-second deadline and no
optimizer. The run completed all five passes before the deadline. Rust reported
seven track-only incomplete connections and ten DRC findings: the same eight
fixed USB pad/hole findings plus two trace-to-via clearances.

KiCad on the exported board, after zone refill, reported no clearance or drill
errors, six unconnected items, four dangling-via warnings, and three silkscreen
warnings. All three silkscreen warnings also occur on the untouched upstream
Nano, which has no unconnected items.

The two additional Rust clearance findings expose a separate importer geometry
issue identified during that investigation: `kicad::reader::via_shape` models round via
copper as a square. DRC reports approximately 0.0491 mm for `/PC5` vs `/PC1` and
`/PC3` vs `/PC4`; measuring the exported track segments against the actual round
0.50 mm vias gives approximately 0.1527 mm and 0.1526 mm. Both exceed the project
clearance of 0.128 mm, consistent with KiCad. The circular-via follow-up below addresses this geometry mismatch. No findings were suppressed to make these results look clean.

## Circular via copper fix

`kicad::reader::via_shape` now creates a centered `Shape::Circle`, rounding its
radius to board units. This shared constructor covers default/additional
net-class via templates, existing board vias and imported session vias. Drill
metadata remains unchanged. The routing engine can still form conservative
search shapes from the circle, while DRC receives the actual circular copper
shape rather than a square invented by the importer.

A diagonal trace/via regression rejects a genuinely insufficient gap but accepts
the Nano-like gap which the old square shape incorrectly rejected. Session
import explicitly checks circular copper and preserved drill dimensions. The
reader port golden updates only the changed via shape rows, with documented
divergences from the unchanged historical Java transcript.

The revised Nano run, with the same settings, has two incomplete connections and
eight Rust findings, all on fixed USB footprint geometry. The two routing-related
clearance false positives are gone. KiCad confirms zero clearance/drill errors,
two unconnected items, two dangling-via warnings and the same three pre-existing
silkscreen warnings. This is still a partial route, not a manufacturing-ready
board.

## Physical hole-clearance follow-up (2026-09-06)

The earlier clean KiCad 10.0.3 reports were misleading. Reordering only the two
NPTH pad records to the end of J1, with unchanged geometry and rules, produces
eight Uno and four Nano hole-clearance reports. KiCad 10.0.3 deduplicates pad
pairs but does not test both directions of a pad/hole pair; the newer KiCad 10.0
source adds the missing swapped-direction check. This is not evidence for a
blanket same-footprint waiver.

Both projects require 0.25 mm. Uno's rectangular GND/Vbus gaps are 0.1751/0.2099
mm, respectively. Nano's rounded pads give 0.1944/0.2586 mm. Overlapping USB pad
definitions count each physical gap twice, so the genuine footprint findings
are eight for Uno and four for Nano. Rounded-pad approximation caused the other
four Nano findings. These results supersede the earlier characterization of all
eight reports as footprint approximation artifacts.

KiCad JSON now accepts `shape: "roundrect"` with a `roundRectRatio` between zero
and one half. Padstacks preserve its corner radius in board units and include it
in padstack sharing. Routing and non-hole DRC still use enclosing rectangles;
hole-to-copper DRC measures the physical rounded corners. Circle/pad and
circle/trace distances also use physical geometry instead of inflated polygon
intersection thresholds. Original input drills marked estimated remain so;
slotted holes retain the documented approximation.

Native tests check the analytical USB gaps across both sides, arbitrary and
right-angle rotations, both pair orders, padstack radius separation, invalid
radii, and real overlap even when the nominal clearance is zero. No same-footprint
or net-specific exception was introduced, and the project minima are unchanged.
