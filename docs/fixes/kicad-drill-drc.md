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

Project clearances also need to reach the routing search. Previously the project
import attached DRC constraints after board preparation, and the resolved router
settings contained a zero hole-clearance default. Pipeline entry now refreshes
preparation when DRC constraints are present. Hole and copper-edge project minima
floor the routing settings, after conversion from board units to micrometres;
stricter router settings remain effective. Existing board preparation rebuilds
the search trees when the effective clearance changes.

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
