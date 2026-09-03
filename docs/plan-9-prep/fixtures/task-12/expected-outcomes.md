# Task 12 — polygon and circle implementations: the directed geometry suite, hand-computed

> *"These are **implementations**: five methods that are stubs today and one that recurses to a
> stack overflow. They get their own directed geometry suite with hand-computed expectations,
> because there is no jar answer worth copying — the jar's answer is `null`, `0` or a crash."*

Below is that suite, with every expectation derived. Two shapes carry the whole bank; both were
constructed against the v1.0.0 port so their corner order, `dimension()` and centre of gravity
are known rather than assumed (`current-port-behavior.txt`).

---

## The two shapes

### `SQ` — a 100 × 100 square
    corners (in the constructor's own order, verified):
        (0,0)  (100,0)  (100,100)  (0,100)
    dimension()          = 2
    centre_of_gravity()  = (50, 50)                          [verified at v1.0.0]
    bounding_box()       = [0,0 .. 100,100]
    perimeter            = 400
    area (shoelace)      = ½|0 + (100·100 − 100·0) + (100·100 − 0·100) + 0| = ½·20000 = 10000

### `L` — a 100 × 100 square with a 20 × 20 notch removed from the top-right
    corners (verified, and the constructor did NOT reorder them):
        (0,0)  (100,0)  (100,100)  (80,100)  (80,80)  (0,80)
    dimension()          = 2
    centre_of_gravity()  = (60, 60)                          [verified at v1.0.0]
    bounding_box()       = [0,0 .. 100,100]
    winding (shoelace Σ) = 0 + 10000 + 2000 − 1600 + 6400 + 0 = 16800 > 0, i.e. counter-clockwise
    area                 = ½ · 16800 = 8400
    sanity               = 100·80 + 20·20 = 8000 + 400 = 8400   ✔

> **The single most important fact for this whole suite:** `centre_of_gravity` is the
> **arithmetic mean of the corners**, not the area centroid (`polyline_shape.rs:124-136` —
> `x += corner.x; … x /= corner_count`). `L`'s area centroid is at `(45.24, 45.24)`; its
> **corner mean is `(60, 60)`**, which is what `smallest_radius` uses. Getting this wrong is
> the easiest way to write a wrong expectation, so `L`'s notch was placed so that `(60,60)`
> lands **strictly inside** the shape — many natural L-shapes put the corner mean outside.

---

## 1. `contains_on_border_distinguishes_inside_from_on_the_edge`

`PolygonShape::contains_on_border` (`polygon_shape.rs:267-270`) is a stub returning `false`,
so `contains_inside(p) == contains(p)` for **every** polygon and every point
(`:245-253` subtracts a predicate that is always false).

On `SQ`:

| point | position | `contains` | `contains_on_border` | `contains_inside` |
|---|---|---|---|---|
| `(50, 0)` | on the bottom edge | `true` | **`true`** (today `false`) | **`false`** (today `true`) |
| `(0, 0)` | at a vertex | `true` | **`true`** (today `false`) | **`false`** (today `true`) |
| `(100, 100)` | at a vertex | `true` | **`true`** (today `false`) | **`false`** (today `true`) |
| `(50, 50)` | strictly inside | `true` | `false` | `true` |
| `(150, 50)` | outside | `false` | `false` | `false` |

> **Invariant, stronger than the table:** for every point,
> `contains(p) == contains_inside(p) || contains_on_border(p)` and
> `contains_inside(p) && contains_on_border(p)` is never true.
> Today the first holds vacuously and the partition is degenerate.

Add `L`'s **reflex vertex `(80, 80)`** as a fifth case: `contains_on_border((80,80)) == true`.
A convex-piece-based implementation that forgets to exclude the *internal* cut edges will get
this one right and get an interior point on a cut edge wrong — so also assert
`contains_on_border((80, 40)) == false` (a point on the internal split line of any sensible
convex decomposition of `L`, but **not** on `L`'s own boundary).

---

## 2. `smallest_radius_of_a_polygon_is_not_zero`

`smallest_radius()` = `border_distance(centre_of_gravity())` (`:226-228`), and `border_distance`
is a stub returning `0.0` (`:220-222`). **So `smallest_radius()` is 0 for every polygon** —
verified for both shapes at v1.0.0 — and that number feeds the clearance heuristics.

### `L` — the expectation, computed edge by edge

CoG = `(60, 60)`, inside `L`. Distance from `(60,60)` to each of the six boundary segments:

| edge | segment | nearest point | distance |
|---|---|---|---|
| bottom | `y = 0`, `x ∈ [0,100]` | `(60, 0)` | `60` |
| right | `x = 100`, `y ∈ [0,100]` | `(100, 60)` | `40` |
| notch top | `y = 100`, `x ∈ [80,100]` | `(80, 100)` (endpoint — 60 < 80) | `√(20² + 40²) = √2000 ≈ 44.7214` |
| notch left | `x = 80`, `y ∈ [80,100]` | `(80, 80)` (endpoint — 60 < 80) | `√(20² + 20²) = √800 ≈ 28.2843` |
| **upper** | `y = 80`, `x ∈ [0,80]` | `(60, 80)` (perpendicular foot, in range) | **`20`** ← minimum |
| left | `x = 0`, `y ∈ [0,80]` | `(0, 60)` | `60` |

> ### `L.smallest_radius() == 20` — **exactly**, no tolerance.
> The minimum is attained at a **perpendicular foot on an edge**, not at a vertex, so the answer
> is a rational number and the assertion can be `assert_eq!(l.smallest_radius(), 20.0)`.
> That is deliberate: a vertex-attained minimum would be a surd and would need a stated
> tolerance.

### `SQ` — the control
CoG = `(50, 50)`; all four edges are 50 away. **`SQ.smallest_radius() == 50`**, exactly.

Include both: the square proves the happy path, the L proves the concave one.

---

## 3. `cutout_of_a_square_by_a_square_yields_the_expected_convex_pieces`

> ### ⚠ The test name and the method signature disagree — resolve this first
> `PolygonShape::cutout` takes a **`&Polyline`**, not a shape
> (`polygon_shape.rs:205-207`; the trait contract is `shape.rs:185-189`,
> *"cuts out the parts of `polyline` in the interior of this shape and returns the remaining
> pieces"*). The plan's test name — *"a square by a square … piece count and area sum"* —
> describes **`Simplex::cutout_from`**, which is a different method and belongs to **#11 in
> Task 11**. Ground truth for both readings follows; pick one and rename the test to match.

### Reading A — the actual signature, `Shape::cutout(&Polyline)`
Shape `SQ = [0,0 .. 100,100]`. Polyline = the straight segment `(−50, 50) → (150, 50)`.

    the polyline's total length                  = 200
    the part in SQ's interior is x ∈ (0, 100)    = 100
    remaining pieces:
        piece 0 : (−50, 50) -> (  0, 50)   length 50
        piece 1 : (100, 50) -> (150, 50)   length 50

> **Expected: 2 pieces, total length 100.** Today: `None`.

Second case, a polyline entirely outside: `(200,0) → (300,0)` → **1 piece, unchanged, length
100** (nothing to cut). Third case, entirely inside: `(10,50) → (90,50)` → **0 pieces**.
Those three cover the branch structure without any geometry beyond arithmetic.

### Reading B — `Simplex::cutout_from`, a square out of a square
Inner `[40,40 .. 60,60]` cut out of outer `[0,0 .. 100,100]`. The method *"divides the resulting
shape into simplices along the minimal distance lines from the vertices of the inner simplex to
the outer simplex"*. Four inner vertices, each projecting to its nearest outer edge:

> **Expected: 4 convex pieces, area sum = 100² − 20² = 10000 − 400 = 9600.**

Contrast case, the one that exercises #11's dead merge branches: cut the inner square out of a
**corner** — inner `[80,80 .. 100,100]` out of `[0,0 .. 100,100]`. Two of the inner vertices lie
on the outer boundary, so only two division lines are needed and two pieces are mergeable:

> **Expected after #11: fewer pieces than division lines. Area sum = 10000 − 400 = 9600.**
> The two cases must **differ in piece count**; today they cannot, because both merge branches
> in `cutoutFrom` are guarded on a `prevDivisionLine` that is never assigned.

The **area sum** is the assertion that is safe in either reading: whatever the decomposition,
the pieces must tile the difference.

---

## 4. `enlarge_grows_every_edge_by_the_offset`

`PolygonShape::enlarge` (`:211-216`) returns `Some(self.clone())` for a zero offset and `None`
otherwise. Verified: `sq.enlarge(10.0).is_none() == true`.

### ⚠ The area formula depends on the corner convention — choose it deliberately

For a convex polygon of area `A` and perimeter `P` offset outward by `d`:

| corner convention | area formula | `SQ` at `d = 10` |
|---|---|---|
| **mitred** (sharp corners) | `A + P·d + d²·Σ tan(θᵢ/2)` — for a rectangle, `A + P·d + 4d²` | `10000 + 4000 + 400 = 14400` (= 120², exact) |
| **rounded** (Minkowski sum with a disc) | `A + P·d + π d²` | `10000 + 4000 + 314.159… = 14314.159…` |
| **45°-chamfered** (the house convention) | see below | `14328` |

The house convention is the third: `IntBox::enlarge` (`int_box.rs:697-699`) is
`bounding_octagon().offset(d)` and returns an **`IntOctagon`**, not a box. Measured at v1.0.0:

    IntBox[0,0 .. 100,100].enlarge(10) = Oct[-10, -10, 110, 110, -114, 114, -14, 214]

Its area, by hand: the 120 × 120 outer box (14400) minus the four corner triangles the diagonals
cut off. At the corner `(110, −10)` the constraint `x − y ≤ 114` cuts a right isoceles triangle
with legs `120 − 114 = 6`, area `6²/2 = 18`; by symmetry all four corners cut 18.

    14400 − 4·18 = 14400 − 72 = 14328     ✔ (and 14314.16 < 14328 < 14400, as it must be)

**Recommendation:** `PolygonShape::enlarge` returns a `PolygonShape`, so **mitre** — the
polygon-in, polygon-out signature admits it and the formula is exact. Record the choice.

> **Assertion (mitre):** `sq.enlarge(10.0).area() == 14400`, exactly, and every edge of the
> result is parallel to and 10 units outside its original.
>
> **Assert the second half too**, not only the area: an implementation that scales the polygon
> about its centroid gets the area of a square right by accident. On `L`, a scale is instantly
> distinguishable from an offset, because the notch must **shrink** by `2d` in each direction
> under a true offset (`L.enlarge(5)`'s notch is 10 × 10, not 20 × 20 scaled up).

---

## 5. `border_distance_and_distance_agree_outside_and_differ_inside`

The two contracts, from the `TileShape` siblings that already work
(`tile_shape.rs:666-687`):

    distance(p)        = |p − nearest_point_approx(p)|,  and nearest_point_approx returns p
                         itself when p is contained  ->  0 INSIDE, distance-to-border OUTSIDE
    border_distance(p) = |p − nearest_border_point_approx(p)|
                                                      ->  distance-to-border ALWAYS

Both are stubs returning `0.0` today (`:220-222`, `:274-276`), so they agree everywhere —
**vacuously**, which is exactly the failure mode a naive test would miss.

On `SQ`:

| point | position | `distance` | `border_distance` | agree? |
|---|---|---|---|---|
| `(150, 50)` | outside, beyond the right edge | `50` | `50` | ✔ agree |
| `(150, 150)` | outside, beyond the corner | `√(50²+50²) = 50√2 ≈ 70.7107` | same | ✔ agree |
| `(−30, 50)` | outside, beyond the left edge | `30` | `30` | ✔ agree |
| `(100, 50)` | **on** the border | `0` | `0` | ✔ agree |
| `(50, 50)` | strictly inside | **`0`** | **`50`** | ✘ **differ** |
| `(90, 50)` | strictly inside, near the edge | **`0`** | **`10`** | ✘ **differ** |

> **Invariant:** `distance(p) == 0` ⟺ `contains(p)`; and
> `border_distance(p) >= distance(p)` for every `p`, with equality exactly outside and on the
> border. The two inside rows are the ones that invert.

Add `L` at `(60, 60)`: `distance = 0`, `border_distance = 20` — reusing the section-2
derivation, so one hand-computation serves two tests.

---

## 6. `circle_nearest_point_approx_lands_on_the_circumference`

`Circle::nearest_point_approx` (`circle.rs:293-296`) is a stub returning `None`. Verified.

Circle `C` = centre `(0, 0)`, radius `100`.

| point `p` | `|p|` | position | expected | tolerance |
|---|---|---|---|---|
| `(300, 400)` | `500` | outside | `p · 100/500 = (60, 80)` | **exact** — `60² + 80² = 10000 = 100²`, a scaled 3-4-5 triangle |
| `(300, 0)` | `300` | outside | `(100, 0)` | **exact** |
| `(−500, 1200)` | `1300` | outside | `p · 100/1300 = (−500/13, 1200/13) ≈ (−38.4615, 92.3077)` | `1e-9` (5-12-13, exact in ℚ but not in `f64`) |
| `(30, 40)` | `50` | **inside** | see the note below | — |
| `(0, 0)` | `0` | **the centre** | degenerate — every boundary point is equidistant | must not be `NaN` |

> **The two decisions the implementer must make and record:**
>
> 1. **Inside points.** `TileShape::nearest_point_approx` (`:706-711`) returns `from_point`
>    itself when the shape contains it. If `Circle` follows the sibling — and it should —
>    `nearest_point_approx((30, 40))` is **`(30, 40)`**, *not* on the circumference. The plan's
>    test **name** says "lands on the circumference", which is `nearest_border_point_approx`'s
>    contract, not this one. Either rename the test or assert on the border variant. For
>    `nearest_border_point_approx((30,40))` the answer **is** on the circumference:
>    `(30,40) · 100/50 = (60, 80)` — the same point as the `(300,400)` row, which is a pleasant
>    cross-check (both lie on the ray through `(3,4)`).
> 2. **The centre.** Pick a deterministic representative — `(radius, 0)` is the natural one —
>    and say so in a comment. Do not return `NaN` and do not leave it unspecified.
>
> **Invariant, whichever is chosen:** for every `p` outside, the answer satisfies
> `|answer| == radius` to the stated tolerance **and** lies on the ray from the centre through
> `p` (`answer × p == 0` as a cross product, and `answer · p > 0`). Asserting the ray as well
> as the radius is what catches a sign error.

---

## 7. `circle_cutout_of_a_box_has_the_expected_area`

`Circle::cutout` (`circle.rs:351-354`) is a stub returning `None`, and its signature is
`(&Polyline) -> Option<Vec<Polyline>>` — the same "cut the polyline, keep what is outside"
contract as §3 reading A. So, again, the test name and the signature disagree.

### Reading A — the actual signature
Circle centre `(0,0)`, radius `100`. Polyline = the straight segment `(−300, 0) → (300, 0)`.

    total length                      = 600
    the part inside the disc          = 200   (x ∈ (−100, 100))
    remaining pieces:
        piece 0 : (−300, 0) -> (−100, 0)   length 200
        piece 1 : ( 100, 0) -> ( 300, 0)   length 200

> **Expected: 2 pieces, total length 400.** Exact — the crossing points are on the axis and
> are integers.
>
> A second case with an oblique chord makes the tolerance question honest: the segment
> `(−300, 60) → (300, 60)` meets the circle at `x = ±√(100² − 60²) = ±80`, again exact
> (a 6-8-10 triangle). **Expected: 2 pieces, total length `(300−80)·2 = 440`.**
> Choosing Pythagorean intercepts keeps every number in this suite exact.

### Reading B — an area, if that is what is meant
Box `[−200,−200 .. 200,200]` (area `160000`) with the disc of radius 100 fully inside:

    area outside the disc = 160000 − π·100² = 160000 − 31415.9265… = 128584.0735…

> **State the tolerance in the test, do not infer it** (the plan says so explicitly). A circle
> in this codebase is a *curved* shape that any tile-based operation must approximate, so an
> exact area is the wrong assertion; a relative tolerance of `1e-6` against `160000 − π·10⁴` is
> the right one, **and the approximation's polygon count must be pinned** or the tolerance is
> meaningless.

Reading A is strongly preferred: it matches the signature and every number in it is exact.

---

## 8. `polygon_against_polygon_intersects_without_recursing` (#27)

### Today
`PolygonShape::intersects(&Shape)` double-dispatches to `shape.intersects_polygon(self)`
(`polygon_shape.rs:159-161`), and the `Shape::Polygon` arm has **no `PolygonShape` overload to
bind to**, so it recurses. Java raises `StackOverflowError`, which **neither language
recovers**: the handlers catch `Exception`, not `Throwable`. The port surfaces it as a named
panic instead of reproducing the unbounded recursion
(`shape.rs:605-611`), verified:

    sq.intersects(Shape::Polygon(disjoint))
      -> panicked: "PolygonShape.intersects(PolygonShape) recurses forever
                    (PolygonShape.java:118-121 has no PolygonShape overload to bind to)"

**Polygon keepouts are ordinary in KiCad exports**, so this is reachable from a real board.

### The fix, and why it makes the expectation derivable rather than guessable
Add an `intersects(PolygonShape)` overload that **splits both sides to convex** — exactly the
shape of the four sibling methods already in the file
(`intersects_circle`, `intersects_simplex`, `intersects_octagon`, `intersects_box`, all
`self.convex_pieces().iter().any(…)`):

    fn intersects_polygon(&self, other: &PolygonShape) -> bool {
        self.convex_pieces().iter().any(|a|
            other.convex_pieces().iter().any(|b| a.intersects_tile(b)))
    }

**So the answer is defined by machinery that already exists and is already tested.** The test
can assert against that definition rather than against a guessed boolean — which matters
because "touching" is the one case where the house convention is not obvious.

### The four cases (`SQ` = `[0,0 .. 100,100]` as a polygon)

| case | second polygon | expected | why |
|---|---|---|---|
| **disjoint** | `(200,0) (300,0) (300,100) (200,100)` | **`false`** | a 100-unit gap; no convex-piece pair can meet |
| **touching** | `(100,0) (200,0) (200,100) (100,100)` | **= the convex-piece answer** | they share the whole edge `x = 100`; see the note |
| **overlapping** | `(50,50) (150,50) (150,150) (50,150)` | **`true`** | the overlap is the 50 × 50 square `[50,50 .. 100,100]`, area 2500 |
| **containing** | `(25,25) (75,25) (75,75) (25,75)` | **`true`** | fully inside |

> **⚠ The touching case is a convention question, not a fact.** Task 11's #17 measured that
> `IntOctagon::contains(FloatPoint)` is **inclusive** on the border where `TileShape::Box`'s is
> **exclusive** — the codebase does not currently agree with itself. So do **not** hard-code a
> boolean here. Assert
> `sq.intersects_polygon(&touching) == <the convex-piece formula above>`, and land #17's
> decision first if it is going to change the answer. Note it in both commit messages.

Add a fifth case that no naive implementation gets right: **`L` against a polygon that fits
inside `L`'s notch** — `(85,85) (95,85) (95,95) (85,95)`. That square is inside `L`'s
*bounding box* and inside `L`'s *convex hull* (verified: `L.bounding_tile()` is the 5-line
convex hull, **not** `L` itself) but **outside `L`**. **Expected `false`.** An implementation
that shortcuts through `bounding_tile()` or `bounding_box()` answers `true` and passes all four
cases above.

---

## 9. `crates/fr-geometry/tests/stubs.rs` is DELETED

*"a test asserting they match Java's stubs is a test asserting the port is broken."*
Name the deletion in the commit message. Check first that nothing else in the tree imports its
helpers.

---

## Timing — Task 12 is one of the four pre-named escalation candidates

Seven constant-time `false` / `0` / `null` stubs become real geometry on **every** polygon
keepout. That is **pure added cost** — none of it replaces work that was being done. Gate
`cpu_s` against Task 16's `stem-times.tsv` with BO's thresholds; the escalation is
pre-authorised.

Two mitigations worth having in hand before measuring:
* **`smallest_radius` is called per keepout per clearance query.** Memoise it on the shape —
  it is a pure function of the corner list, and the corner list is immutable.
* **`contains_on_border` on a convex decomposition is O(pieces).** A single pass over the
  polygon's own edges is O(corners) and does not need the decomposition at all — and it is
  also the only way to get the `(80, 40)` case in §1 right.

## The G2 stop-and-report — read this before running the A/B

> *"violations must stay 0 and incompletes must not rise; a polygon keepout that finally has a
> non-zero `smallestRadius` makes the router avoid copper it used to route through, so **a small
> incomplete rise on one stem is a stop-and-report**, not a tolerance."*

If it happens, the keepout was being ignored, and the honest answer needs recording the way
#82's does. **First step in that case:** `grep` the fixture corpus for boards carrying a polygon
keepout and paste the count — the plan's `goldens moved:` line requires it anyway, and it tells
you immediately whether the rise is on a board that has one.

---

## Summary — the bank

| test | shape(s) | expected | exact? |
|---|---|---|---|
| `contains_on_border_distinguishes…` | SQ, L | 5-row table + the `(80,40)` cut-line trap | yes |
| `smallest_radius_of_a_polygon_is_not_zero` | L, SQ | **20**, **50** | yes |
| `cutout_of_a_square_by_a_square…` | SQ + polyline / simplex | 2 pieces len 100 (A); 4 pieces area 9600 (B) | yes |
| `enlarge_grows_every_edge_by_the_offset` | SQ, L | **14400** (mitre); notch shrinks by `2d` | yes |
| `border_distance_and_distance_agree…` | SQ, L | 6-row table; `(50,50) → 0 vs 50` | yes |
| `circle_nearest_point_approx…` | circle r=100 | `(300,400) → (60,80)`; centre case named | yes (Pythagorean) |
| `circle_cutout_of_a_box…` | circle r=100 | 2 pieces, len 400 / 440 | yes (Pythagorean) |
| `polygon_against_polygon_intersects…` | SQ, L | 4 cases + the notch trap; touching = convex-piece answer | yes |

Every number in this suite is a rational or a named surd, and every non-exact case
(`(−500,1200)`, the circle area) carries a stated tolerance. **No jar answer was consulted, and
none exists — the jar answers `null`, `0` or `StackOverflowError` for all seven.**
