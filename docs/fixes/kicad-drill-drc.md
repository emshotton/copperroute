# KiCad drill metadata and clearance measurements

## Import and routing rules

Padstacks carry optional drill diameters in board units, a drill-approximation
flag and a hole-only flag. KiCad JSON imports populate these for pads, existing
vias, session vias and net-class via templates. Padstack sharing includes drill
diameter, plating and approximation status. The writer exports explicit via
drills. Inputs without drill metadata use padstack-name heuristics or estimated
diameters. The JSON pad extensions `nonPlated` and `drillEstimated` default to
false. Non-plated holes are exempt from annular-ring checks.

Project import attaches DRC constraints after board preparation. Board
preparation and pipeline entry floor routing hole clearance and board-edge
clearance at the project minima, converting to board units. Larger explicit
routing clearances are preserved, and search trees are rebuilt when a minimum
raises a value.

## Hole-clearance geometry

Hole-to-hole DRC measures center distance minus the two drill radii.
Hole-to-copper DRC measures circular pad/via copper, trace centerlines plus half
width, and placed rectangular or rounded pad copper. Both directions of a
pad/hole pair are checked. Same-net and same-logical-pad exclusions apply; there
is no general same-footprint exemption. Bare holes participate in hole checks
but have no copper for copper-clearance or copper-edge checks. Real overlap is
reported even when the required clearance is zero.

KiCad JSON accepts `shape: "roundrect"` with an optional `roundRectRatio` between
zero and one half. When present, the ratio and positive finite pad dimensions
are validated. When absent, the pad retains the legacy rectangular
representation. Corner-radius metadata participates in padstack identity.

Rounded-corner measurements use the nearest point on an inset convex tile plus
the corner radius. This supports axis-aligned, 45-degree and arbitrary pad
rotations. If the inset cannot supply a nearest point, measurement falls back to
the enclosing polygon; the pad/hole pair is still checked. Coordinates are
rounded to board units. Trace measurements use the geometry crate's polyline
nearest-point implementation and run once per ordered copper/hole/layer tuple,
even if several segment tiles encounter the same hole. Violation markers are
clamped to the interval between the hole center and nearest geometry point.

## Limits

Routing and non-hole DRC use enclosing rectangles for rounded pads.
Oval copper remains a circumscribing octagon, so hole-to-oval gaps can be
underestimated near the curved ends. General polygonal pad geometry is measured
as imported. Slotted holes and drills marked estimated retain their
approximations. These limits mean that not every DRC measurement represents an
exact physical copper outline.

Via-in-pad attachment permissions are independent of these measurements.
Browser format translation, UI behavior and zone refilling are outside these
native checks.

## Regression coverage

Tests cover distinct padstack drill/radius identities, via drill export, legacy
roundrect payloads without radius metadata, invalid explicit radii, circular
hole spacing, trace clearance, both item orders, both board sides, rotations
including 45 and 135 degrees, zero-clearance overlap and project routing minima.

Analytical USB connector examples with a 0.25 mm requirement give these gaps:

| Pad geometry | GND gap (mm) | Vbus gap (mm) |
| --- | ---: | ---: |
| Rectangle (Uno) | 0.1751 | 0.2099 |
| Rounded rectangle (Nano) | 0.1944 | 0.2586 |

With the duplicated pad definitions in those footprints, these measurements
correspond to eight Uno and four Nano findings. They describe the imported
footprints, independently of any routed tracks or vias.
