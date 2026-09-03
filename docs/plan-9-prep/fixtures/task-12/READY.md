# Task 12 — what the implementer gets

**Coverage: 2 of 2 fix rows (8 of 8 named tests) have hand-computed ground truth.**
There is no jar answer for any of them — the jar answers `null`, `0` or `StackOverflowError` —
so this is the task where the derivations *are* the deliverable.

| named test | expected value | exact? |
|---|---|---|
| `contains_on_border_distinguishes_inside_from_on_the_edge` | 5-row table on `SQ` + `L`'s reflex vertex + the internal-cut-line trap `(80,40) → false` | yes |
| `smallest_radius_of_a_polygon_is_not_zero` | **`L` → 20**, `SQ` → 50 | yes (minimum at a perpendicular foot, so rational) |
| `cutout_of_a_square_by_a_square_yields_the_expected_convex_pieces` | polyline reading: **2 pieces, length 100**; simplex reading: **4 pieces, area 9600** | yes |
| `enlarge_grows_every_edge_by_the_offset` | mitre: **14400** at `d=10`; plus the notch-shrinks-by-`2d` check that a scale cannot fake | yes |
| `border_distance_and_distance_agree_outside_and_differ_inside` | 6-row table; `(50,50) → 0` vs `50` is the inverting row | yes |
| `circle_nearest_point_approx_lands_on_the_circumference` | `(300,400) → (60,80)`, exact; centre and inside cases named | yes |
| `circle_cutout_of_a_box_has_the_expected_area` | **2 pieces, length 400** (and 440 for the oblique chord) | yes |
| `polygon_against_polygon_intersects_without_recursing` | 4 cases + a fifth that defeats any bounding-box shortcut | yes |

## The two shapes the whole bank rests on
* **`SQ`** = `(0,0) (100,0) (100,100) (0,100)` — area **10000**, CoG **(50,50)**.
* **`L`** = `(0,0) (100,0) (100,100) (80,100) (80,80) (0,80)` — area **8400**, CoG **(60,60)**,
  `smallest_radius` **20**.

Both were constructed against the v1.0.0 port, so their corner order, `dimension()` and centre
of gravity are **measured, not assumed** (`current-port-behavior.txt`).

## The three things to read first

1. **`centre_of_gravity` is the corner MEAN, not the area centroid.** `polyline_shape.rs:124-136`
   sums the corners and divides by the count. `L`'s area centroid is `(45.24, 45.24)`; its
   corner mean is `(60, 60)`, and that is what `smallest_radius` uses. `L`'s notch was
   deliberately placed so `(60,60)` lands **strictly inside** — most natural L-shapes put the
   corner mean *outside* the shape, which makes the expectation nonsense. If you change the
   shape, re-check that first.

2. **Two of the plan's test names describe a method the signature does not have.**
   `PolygonShape::cutout` and `Circle::cutout` both take a **`&Polyline`** ("cut the polyline,
   keep the pieces outside this shape"), not a shape and not an area. Ground truth is given for
   **both** readings; pick one and rename the test. The polyline reading is preferred — it
   matches the signature and every number in it is exact. The same applies to
   `circle_nearest_point_approx_lands_on_the_circumference`: `nearest_point_approx` returns the
   point itself when it is *inside*, so it is `nearest_border_point_approx` that always lands on
   the circumference.

3. **The "touching" case in #27 is a convention question, not a fact — do not hard-code it.**
   Task 11's #17 measured that the codebase disagrees with itself about whether a border point
   is contained (`IntOctagon` inclusive, `TileShape::Box` exclusive). Assert
   `intersects_polygon(touching) == <the convex-piece formula>` instead of a boolean, and land
   #17's decision first if it will change the answer.

## Bonus: the trap case worth keeping
`L` versus the 10 × 10 square at `(85,85)-(95,95)`. That square is inside `L`'s bounding box
**and** inside `L`'s convex hull — verified: `L.bounding_tile()` comes back as a **5-line
Simplex, the convex hull, not the L** — but it is outside `L`. **Expected `false`.** Any
implementation that shortcuts through `bounding_tile()` passes the plan's four named cases and
fails this one.

## Cost, and two mitigations to have ready
Seven constant-time stubs become real geometry on every polygon keepout — **pure added cost**,
none of it replacing work. Before measuring: memoise `smallest_radius` (a pure function of an
immutable corner list), and implement `contains_on_border` as a single O(corners) pass over the
polygon's own edges rather than over its convex decomposition — which is also the only way to
get the internal-cut-line case right.

## Provenance
Tag **`v1.0.0` (`ecc0abf`)**, cloned read-only into the scratchpad and built there. Probe source
at `$SCRATCH/port-v100/crates/fr-geometry/examples/p9poly.rs`. The `plan-9-post-parity` working
tree was never written to.
