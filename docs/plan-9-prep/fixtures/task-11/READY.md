# Task 11 — what the implementer gets

**Coverage: 10 of 10 fix rows have ground truth ready** (the row `#26, #88, #23, #11, #17, #18,
#32, #188` is one register row and eight sites; **8 of 8 sites** are covered).

| row | ground truth | confirmed at v1.0.0 |
|---|---|---|
| #5 | the projection formula re-derived from scratch + two numeric witnesses + a `det == 0` control | yes |
| #7 + #68 | round-trip contract from `border_line(i)`'s own definition, plus the orientation trap | yes (all four `None`) |
| #177 | "the sibling is the spec": trace count must not depend on `changed_area` marking | — |
| #186 | line-conservation invariant; the existing test's current failure **is** the target | — |
| #187 | **`fixtures/p9t11-per-layer-width.dsn` — built, routed, validated**; 8:1 layer width ratio; also #128's fixture | yes |
| #55 | four transform invariants incl. outline↔keepout agreement | — |
| #183 | bend-arm reachability count `0 → >0` | — |
| #48 + #57 | disagreement `= 360 − 2a`; **do not test at 180° or 0°** | — |
| #15, #16, #13 | measured tables; the survey's #15 sentence corrected; #13 grounded on transpose symmetry | yes (#15, #13) |
| #26 tail (8 sites) | see below | 6 of 8 measured |

### The #26 tail, site by site
| site | expected | today |
|---|---|---|
| #26 `PolygonShape::area` | square **10000**, L-shape **8400** (shoelace shown twice) | `0` |
| #88 `PolygonPath::bounding_box` | `[−100,−100 .. **1100**,1100]` | `[−100,−100 .. **1300**,1100]` |
| #23 `Polyline::from_two_points` | `lines[2] = (100,0)→(100,**−1**)` | `(100,0)→(100,**+1**)` |
| #11 `Simplex::cutout_from` | corner-cut and middle-cut piece counts must **differ** | identical (both merge branches dead) |
| #17 `IntOctagon::contains(FloatPoint)` | octagon and box must agree on all three border points | 3 of 3 disagree |
| #18 `IntBox::divide_into_sections` | degenerate box → **1** section, area conserved | **0** sections, shape lost |
| #32 `Circle::translate_by(RationalVector)` | translate **or** fail, like every sibling | returns `this` silently |
| #188 `Polyline::from_lines` | caller's `Vec` unchanged; **six** call sites named that must migrate | writes through |

## The three things to read first

1. **The survey's #15 mechanism sentence is backwards.** It says *"a corner at distance exactly
   0 is never nearest"*; in fact `0.0 < Double.MIN_VALUE` is **true**, so distance-0 is the
   *only* thing that fires and every non-zero distance is skipped. The plan's test name
   `nearest_corner_at_distance_zero_is_nearest` therefore names a case that **already passes**.
   Add `nearest_corner_at_non_zero_distance_is_nearest` with the measured `(9,9) → 0 → 2` row,
   which is the row that actually inverts.

2. **`#187`'s fixture is done and proven.** It routes at v1.0.0, forces a layer change on both
   nets, and the emitted session shows `F.Cu 40000` against `B.Cu 5000` — an 8:1 ratio, so a
   stub taking the wrong layer's width is a factor-of-eight error, not a rounding argument.
   Task 21's #128 reuses it as-is.

3. **Three rows carry a *decision*, not just a fix, and all three fail if fixed one-sidedly:**
   `#48+#57` (which angle is intended — fix all three sites in one commit),
   `#17` (inclusive or exclusive border — apply to box, octagon and simplex together), and
   `#32` (translate or throw). Record each decision in its commit message.

## What was NOT built here
* `#177`, `#186`, `#55`, `#183`, `#11` are invariant-shaped and need a board to run on; the
  invariants are written but the fixtures are corpus stems, not hand fixtures.
* No golden regeneration and no `p2t11` re-golden — those need the working tree.
* `#13`'s expected staircase is given modulo the `to_the_right` handedness flip; the
  implementer resolves it by running the fixed code in both orientations against the healthy
  `(0,0)→(20,7)` branch, which is measured here.

## Provenance
All measurements come from the repo at **tag `v1.0.0` (`ecc0abf`)**, cloned read-only into the
scratchpad and built there. The probe source is
`$SCRATCH/port-v100/crates/fr-geometry/examples/p9probe.rs`. The `plan-9-post-parity` working
tree was never written to.
