# Task 11 — board geometry corrections: independent ground truth

Every number below was either derived by hand and then confirmed against the **v1.0.0** port,
or measured from the v1.0.0 port and then explained. Raw probe output is in
`current-port-behavior.txt`.

---

## #5 — `RationalPoint::perpendicularProjection` uses `add` where `IntPoint` uses `subtract`

### The correct formula, derived
Let the line be given by two `IntPoint`s `A`, `B`; `v = B - A`; `D = v.x² + v.y²`;
`det = determinant(A, B) = A.x·B.y − A.y·B.x`.

The line's implicit equation is `v.y·x − v.x·y = det`, because

    A.x·v.y − A.y·v.x = A.x(B.y − A.y) − A.y(B.x − A.x)
                      = A.x·B.y − A.x·A.y − A.y·B.x + A.y·A.x
                      = A.x·B.y − A.y·B.x = det.                                (1)

The foot of the perpendicular from `P = (px, py)` is `P − s·n` with unit-normal direction
`n = (v.y, −v.x)` and `s = (v.y·px − v.x·py − det) / D`. Expanding:

    Q.x = px − (v.y·px − v.x·py − det)·v.y / D = ( v.x²·px + v.x·v.y·py + det·v.y ) / D
    Q.y = py + (v.y·px − v.x·py − det)·v.x / D = ( v.x·v.y·px + v.y²·py − det·v.x ) / D    (2)

`IntPoint::perpendicular_projection` (`int_point.rs:310-354`) computes exactly (2):
`proj_x = vxvx·px + vxvy·py + det·v.y`, `proj_y = vxvy·px + vyvy·py **−** det·v.x`.

`RationalPoint::perpendicular_projection` (`rational_point.rs:178-224`) computes
`proj_y = vxvy·x + vyvy·y **+** det·v.x·z`. **The sign of the `det` term is wrong.**
The error is `2·det·v.x / D` in `Q.y`, which vanishes exactly when `det = 0` — i.e. **only for
a line through the origin**, which is why nothing caught it.

### Witnesses (hand-computed, then confirmed at v1.0.0)

**W1 — the horizontal line `y = 10`.** `A = (0,10)`, `B = (10,10)`, `P = (0,0)`.

    v = (10, 0)          D = 100 + 0 = 100
    det = 0·10 − 10·10 = −100
    Q.x = (100·0 + 0·0 + (−100)·0) / 100 =    0 / 100 =   0
    Q.y = (0·0 + 0·0 − (−100)·10)  / 100 = 1000 / 100 =  10       CORRECT: (0, 10)
    buggy Q.y = (0 + 0 + (−100)·10) / 100 = −1000 / 100 = −10     BUGGY:   (0, −10)

  v1.0.0 prints: `IntPoint(0,0) -> Int(0, 10)`, `Rational(0,0) -> Int(0, -10)`. ✔

**W2 — the slanted line through `(0,5)` and `(10,15)` (i.e. `y = x + 5`), `P = (10,5)`.**

    v = (10, 10)         D = 100 + 100 = 200
    det = 0·15 − 5·10 = −50
    Q.x = (100·10 + 100·5 + (−50)·10) / 200 = (1000 + 500 − 500) / 200 = 1000/200 =  5
    Q.y = (100·10 + 100·5 − (−50)·10) / 200 = (1000 + 500 + 500) / 200 = 2000/200 = 10
                                                                CORRECT: (5, 10)
    buggy Q.y = (1000 + 500 − 500) / 200 = 1000/200 = 5          BUGGY:   (5, 5)
    error = 2·det·v.x / D = 2·(−50)·10 / 200 = −5.    10 − 5 = 5.  ✔

  v1.0.0 prints: `IntPoint(10,5) -> Int(5, 10)`, `Rational(10,5) -> Int(5, 5)`. ✔

**W3 — the control, a line through the origin.** `A = (0,0)`, `B = (10,0)`, `P = (3,7)`.
`det = 0`, so both implementations agree: `(3, 0)`. v1.0.0 prints `(3,0)` for both. ✔

**Test shape:** the plan's `perpendicular_projection_agrees_with_int_point` over a generated
line set **not through the origin**. The generator must exclude `det == 0` or it will pass on
the bug. Post-fix, the assertion is exact equality of the two implementations for every point
representable as both — no tolerance.

---

## #7 + #68 — `borderLineIndex` is a stub returning `-1`, and both dog-ear cuts are guarded by an always-true reference comparison

### Current (v1.0.0)
`IntBox::border_line_index` (`int_box.rs:425-427`) and `IntOctagon::border_line_index`
(`int_octagon.rs:809-811`) are `pub fn border_line_index(&self, _line: &Line) -> Option<usize> { None }`.
Their own unit tests pin the stub: `int_box.rs:820` asserts
`x.border_line_index(&x.border_line(i)) == None` **for its own border lines**.

### The expectation, from `border_line` (`int_box.rs:410-418`) — the specification is already in the file
For `IntBox b = [ll.x, ll.y .. ur.x, ur.y]`:

    border_line(0) = Line( (0, ll.y),  (1, ll.y) )     lower boundary, directed +x
    border_line(1) = Line( (ur.x, 0),  (ur.x, 1) )     right boundary, directed +y
    border_line(2) = Line( (0, ur.y),  (-1, ur.y) )    upper boundary, directed −x
    border_line(3) = Line( (ll.x, 0),  (ll.x, −1) )    left  boundary, directed −y

Measured for `b = [0,0 .. 100,50]` at v1.0.0 (`current-port-behavior.txt`):

    border_line(0) = Line{a:(0,0),    b:(1,0)}     -> None   (must become Some(0))
    border_line(1) = Line{a:(100,0),  b:(100,1)}   -> None   (must become Some(1))
    border_line(2) = Line{a:(0,50),   b:(-1,50)}   -> None   (must become Some(2))
    border_line(3) = Line{a:(0,0),    b:(0,-1)}    -> None   (must become Some(3))

### The three-part contract the fix must satisfy — and the trap
1. **Round trip.** `b.border_line_index(&b.border_line(i)) == Some(i)` for `i ∈ 0..4`
   (0..8 for the octagon).
2. **Non-border → `None`.** e.g. `Line::from_coords(0, 0, 1, 1)` on the box above.
3. **⚠ Orientation matters.** The freerouting tile convention is *the shape lies on the RIGHT
   of every border line* (`int_octagon.rs:1716-1722` states it). So the **reversed** line
   `Line((−1, ll.y), (0, ll.y))` — same point set as `border_line(0)`, opposite direction —
   is **not** border line 0 and must answer `None`. An implementation that tests "is this line
   collinear with border line i" without checking direction will pass tests 1 and 2 and still
   be wrong at the caller.

   Note also that `border_line(0)` and `border_line(3)` of the box above **share the point
   `(0,0)`**. Equality by defining points alone is therefore not enough either: the right
   predicate is *same direction AND `line.side_of(border_line(i).a) == Collinear`*.

### #68 — the cut guard
Same file: both dog-ear cuts test whether the cut changed the shape by comparing **references**
(`newShape != oldShape` on the object), which is always true because the cut always allocates.
So `cutOffAtStart` / `cutOffAtEnd` are set even when the cut removed nothing, and the
`fromSide` search then hunts for a border line the shape does not have — which is exactly what
`borderLineIndex`'s `-1` makes unrecoverable. **Fix: compare the shapes by VALUE.**
Invariant: `cut_off_at_start` is true **iff** the cut shape differs from the input shape as a
value. `p2t11` mode 5 pins all four `(cut_off_at_start, cut_off_at_end)` combinations and is
the before/after evidence.

**Order:** #5 must land first. The projection is the entry *point*; `borderLineIndex` is the
entry *side*. A right side computed from a wrong point is still wrong.

---

## #177 — `TraceShover.insert` dereferences `board.changedArea` with no null check

**JVM-pinned literal:** an unmarked board leaves **three** traces where a marked board leaves
**one** — the substitute traces are inserted un-normalised, and `TraceShover.insert`'s own
`catch` swallows the NPE so nothing reports it.

**Ground truth without the jar:** the sibling is the specification. `ForcedPadRouter.forcedPad`
(`:439-444`) runs *the identical loop* and **guards the identical call**. So:

> **Invariant:** for the same board and the same shove, `TraceShover::insert` and
> `ForcedPadRouter::forced_pad` leave the board in the same normalisation state. Concretely:
> the number of traces after the shove is independent of whether `board.changed_area` is
> marked.

The plan's test `an_unmarked_changed_area_still_normalises` replaces the pinned literal
`3` with `1`. **1** is the *marked*-board answer, which the port already produces on a marked
board — so the fail-before/pass-after pair is measurable entirely inside the port:

    marked   board -> 1 trace   (today and after the fix)
    unmarked board -> 3 traces  (today)   ->   1 trace  (after the fix)

Narrow the `catch` in the same commit: today it catches `Exception`, which hides the NPE that
*is* the bug. After the fix there is nothing left for it to catch that is not a real fault.

---

## #186 — `FoundConnectionInserter` hands `connectToTrace` a trace the insert has already split away

**Mechanism, restated as a lifecycle:**

    1. the connection is inserted; the insert SPLITS trace T into T1 and T2 and REMOVES T
    2. connectToTrace is still holding the stale `Trace` object T
    3. the stub is inserted against T's polyline — a polyline the board no longer holds
    4. the two tail removals then delete BOTH halves, T1 and T2

**Measured:** both halves of trace 4 are gone, and its line survives only inside a combined
trace.

**Ground truth:** this one needs no external oracle at all, because the port's existing test
already fails **on the fixed behaviour**. The survey and the plan both say so:
*"`crates/fr-router/tests/inserter.rs` (fails today on the id lookup and the post-loop
snapshot — which is the fixed behaviour)"*. So the implementer's evidence is: make the
existing test pass.

**The invariant to add alongside it, because the snapshot alone is fragile:**
> After `FoundConnectionInserter` lands a connection mid-trace on trace `T`, the board holds
> **exactly the two halves** of `T` (or their normalised merge) and **no orphaned line**:
> `board.traces().filter(|t| t.net == T.net).flat_map(lines).count()` is conserved, and every
> line of `T` is reachable from some live trace.

**Fix:** look the trace up **by id** after the insert. The survey calls this "the most visible
geometry change in the register" — the plan's acceptance already requires a by-eye review of
one stem's diff. Do that review on a stem where a connection lands mid-trace; `router-j2-reference`
and `router-rpi-splitter` both have them.

---

## #187 — the stub is sized from the *other* end's layer

### The fixture — BUILT AND VALIDATED
`fixtures/p9t11-per-layer-width.dsn`. Two-layer 300 mm × 200 mm board, four pads:
`P1`/`P3` are **F.Cu-only** pads, `P2`/`P4` are **B.Cu-only** pads, so every connection
*must* change layer. The net class carries

    (layer_rule F.Cu (rule (width 4000)))     -> 4000 um  = 40000 DSN units
    (layer_rule B.Cu (rule (width  500)))     ->  500 um  =  5000 DSN units

an **8 : 1** ratio, so a stub sized from the wrong layer is unmistakable — not a rounding
argument. Validated against the v1.0.0 binary (exit 0); the emitted session carries

    (path F.Cu 40000  …)      on both nets
    (path B.Cu  5000  …)      on both nets
    (via VIA1 …)              on both nets

so the per-layer widths *are* read and *are* honoured on ordinary trace insertion. This is
also the fixture #128 needs in Task 21 — build it once, here.

### The expectation
> **Invariant:** a `connect_to_trace` stub inserted onto a target trace on layer `L` has half
> width `trace_half_width(L)`, for every `L`. Never the start layer's.

On this fixture the two possible answers are `40000` and `5000`, so the test reads:
the stub landing on B.Cu is `5000` wide (today: `40000`, the start layer's), and the stub
landing on F.Cu is `40000` (today: `5000`).

The survey's note *"latent only because every corpus board shares a width across layers"* is
confirmed: nothing in `tests/reference/` distinguishes the two, which is why this fixture is
required rather than optional.

---

## #55 — all four `BoardOutline` transforms assign to the loop variable

**Mechanism:** `for (Shape s : this.shapes) { s = s.translate_by(v); }` — Java's enhanced
`for` binds a *copy of the reference*, so `this.shapes` is never written. Only the lazily-built
outside-keepout follows the transform, so after any move/turn/rotate/mirror the outline's
curves and its keepout **disagree**, and `boundingBox` / `lineCount` / `getShape` and the
search-tree line bands all answer from the untransformed shapes.

**Ground truth — four assertions, no literals needed:**

> For each transform `t ∈ {translate_by(v), turn_90_degree(k, p), rotate_approx(a, p),
> mirror_vertical(x)}` and an outline `O`:
>
>   1. `O.transform(t).bounding_box() == t(O.bounding_box())`  — the outline moved.
>   2. `O.transform(t).shape(i) == t(O.shape(i))` for every `i` — every shape moved.
>   3. `O.transform(t).line_count() == O.line_count()`         — nothing was lost.
>   4. `O.transform(t).keepout_outside_outline()` agrees with `t(O.keepout_outside_outline())`
>      — the outline and the keepout stay together.
>
> Assertion 4 is the one that is false today for a *reason* rather than by accident: the keepout
> IS transformed (it is rebuilt lazily from the — untransformed — shapes only if it was not
> already built), so the two halves drift apart depending on build order.

**Consequence to expect:** *a board whose outline finally moves has a different routable
region.* The corpus re-baselines; `G` moves. This is a correctness win being paid for in
churn, not a regression.

**Simplest fixture:** any board with an outline, translated by `(1000, 0)`, is enough for all
four assertions. `tests/reference/large-outline` already exists and is the natural stem.

---

## #183 — the any-angle `bend` branch is unreachable

**Mechanism:** `TraceTightenerAnyAngle.smoothenEndCornerAtTrace` reads `prevLineDirection`
from `lines[endLineNo]` — **the same line** `lineDirection` comes from. The `bend` arm's
guard needs two *different* directions, and two reads of one line cannot supply them. So the
whole `bend` branch of the any-angle end-corner smoothener is dead code.

**Fix:** read `lines[endLineNo − 1]`.

**Ground truth — a reachability proof, not a geometry literal:**
> **Invariant:** on an any-angle board with an end corner whose two adjacent lines have
> **different** directions, the `bend` arm executes at least once.
>
> Instrument it (a counter in the test build, or assert on the *output*: the bend arm produces
> a corner the `no-bend` arm cannot). Today the counter is **0** for every input; after the
> fix it is **> 0** for any end corner with a real bend, which is *every* end corner on an
> any-angle board — a straight-through "end corner" is not an end corner.

The port's `crates/fr-router/tests/tightener.rs`'s `smooth` fixture already reaches the method.
Any-angle stems only, so `R`/`B` move only there.

---

## #48 + #57 — a back-side rotation splits the component from its outline

**Mechanism (identical in three places):** for a back-side item under `flipStyleRotateFirst`,

    Component.rotate:               stored rotation += (360 − angle)
                                    location        rotated by  angle
    ObstacleArea.rotateApprox:      same split
    ComponentOutline.rotateApprox:  same split

so rotation and location disagree by `360 − 2·angle`.

    angle =  90  ->  disagreement = 360 − 180 = 180 degrees
    angle =  45  ->  disagreement = 360 −  90 = 270 degrees
    angle = 180  ->  disagreement = 360 − 360 =   0 degrees   <-- the ONLY angle that agrees
    angle =   0  ->  disagreement = 360                       (i.e. 0 mod 360, also agrees)

So `180°` and `0°` are the two angles at which the bug is invisible. **A test at 180° proves
nothing.** Use **90°** (the largest, cleanest disagreement) or 45°.

**Ground truth — one invariant covering all three sites:**
> **Invariant:** for a back-side component `C` with pads `P` and an outline `O`, and any angle
> `a`, `rotate(C, a)` leaves `O`'s corners in the same relation to `P`'s centres as before:
>
>     for every pad p and outline corner c:  (rotate(c) − rotate(p)) == R(a) · (c − p)
>
> i.e. the outline and the pads are rotated by the **same** rotation. Today they differ by
> `R(360 − 2a)` for a ≠ 0, 180.

**The decision the implementer must record:** *which* angle is intended. `360 − angle` is the
flip-then-rotate convention (rotate in the mirrored frame); `angle` is the rotate-then-flip
one. **Fix all three at once** — fixing one makes the other two *disagree with it* rather than
with each other, which is strictly worse than the current state. The plan's single test
`a_back_side_rotation_keeps_the_outline_and_the_pads_together` is the right shape: one
assertion, all three sites.

---

## #15 — `indexOfNearestCorner` seeds with `Double.MIN_VALUE`

### ⚠ The survey's mechanism sentence is backwards — correct it before writing the test
The survey says *"a corner at distance **exactly 0** is never nearest"*. The opposite is true.
`Double.MIN_VALUE` is `4.9E-324`, the smallest **positive subnormal**, and the loop's test is
`current_distance < min_dist` (`tile_shape.rs:855-868`). `0.0 < 4.9E-324` is **true**, so a
corner at distance 0 *is* the only thing that can ever fire.

The real defect: **every corner at non-zero distance is skipped**, so the method answers
`0` for any point that is not exactly on a corner. The port's own doc says so
(`tile_shape.rs:850-854`) and its pinning test at `:1848` is named
`index_of_nearest_corner_only_moves_off_zero_at_distance_zero`.

### Measured at v1.0.0, box `[0,0 .. 10,10]`, corners `(0,0) (10,0) (10,10) (0,10)`

| point | true nearest corner | true distance | v1.0.0 answers | post-fix |
|---|---|---|---|---|
| `(9,9)`   | 2 = `(10,10)` | √2 ≈ 1.414 | **0** | 2 |
| `(1,9)`   | 3 = `(0,10)`  | √2 ≈ 1.414 | **0** | 3 |
| `(11,11)` | 2 = `(10,10)` | √2 ≈ 1.414 | **0** | 2 |
| `(5,5)`   | tie, all 4 at √50 | 7.071 | 0 | 0 (first minimum wins) |
| `(10,10)` | 2, distance 0 | 0 | 2 | 2 |
| `(0,10)`  | 3, distance 0 | 0 | 3 | 3 |

**Fix:** seed with `f64::MAX` (Java `Double.MAX_VALUE`). **Test name:** the plan's
`nearest_corner_at_distance_zero_is_nearest` describes the case that *already works*; rename it
or, better, keep it *and* add `nearest_corner_at_non_zero_distance_is_nearest` with the
`(9,9) -> 2` row, which is the row that actually inverts.

---

## #16 — `nearestBorderPointsApprox`' upward insertion shift copies the wrong element

The insertion sort's shift loop copies `arr[j]` into `arr[j]` (or `arr[j+1]` into `arr[j+1]`)
instead of `arr[j]` into `arr[j+1]`, so an element inserted above position 0 **overwrites** its
neighbour rather than displacing it.

> **Invariant:** `nearest_border_points_approx(p, k)` returns `k` points, **all distinct**
> (as positions in the border-point list), sorted by ascending distance from `p`.
> Today, for `k > 1`, an element is duplicated and one is lost.

**Fix, and the part the plan calls out separately: `check the count > 1 callers`.** `k == 1`
never enters the shift, so every caller passing 1 is unaffected and every caller passing more
than 1 has been getting a list with a duplicate. Enumerate those callers in the commit message
— their behaviour changes.

---

## #13 — `stairApproximation45` calls a function of *x* with a *y* coordinate

Port: `line_segment.rs:446-474`. The `function_of_y` branch (`abs_delta.x < abs_delta.y`) does

    current_y = start_point.y + i * stair_width;
    current_x = java_round(self.get_line().function_value_approx(current_y as f64)) as i32;
                                          ^^^^^^^^^^^^^^^^^^^^^ the x -> y function, fed a y

`function_in_y_value_approx` is the y → x function and is what `stairApproximation` (the
non-45 sibling) uses. So the bug is *visible by comparison with the sibling in the same file*.

### Measured at v1.0.0 — the output leaves the segment's own bounding box

    seg (0,0) -> (20,7), width 2, to_the_right   [function of X, the healthy branch]
      [(0,0) (4,0) (6,2) (10,2) (12,4) (16,4) (18,6) (19,6) (20,7)]
      -> every point is inside [0,20] x [0,7].                              OK

    seg (0,0) -> (7,20), width 2, to_the_right   [function of Y, the broken branch]
      [(0,0) (17,17) (17,6) (34,23) (34,12) (51,29) (51,18) (7,62) (7,20)]
      -> x reaches 51 on a segment whose x never exceeds 7;
         y reaches 62 on a segment whose y never exceeds 20.                BROKEN

**The expectation needs no jar and no hand-computed staircase**, because the two branches are
mirror images:

> **Invariant 1 (containment):** every point of `stair_approximation_45(seg, w, r)` lies within
> the segment's bounding box grown by `w` — a staircase approximation of a segment cannot leave
> the segment's neighbourhood.
>
> **Invariant 2 (symmetry — this is the strong one):** for a segment `s` and its transpose
> `s'` (x and y swapped), the staircases are transposes of each other:
> `stair_approximation_45(s', w, r) == transpose(stair_approximation_45(s, w, !r))`.
> The `(0,0)->(20,7)` and `(0,0)->(7,20)` pair above is exactly such a pair, and the healthy
> branch supplies the answer for the broken one.

Applying invariant 2 to the measured healthy output, the expected `(0,0)->(7,20)` staircase is
the transpose of the `(0,0)->(20,7)` one:

    [(0,0) (0,4) (2,6) (2,10) (4,12) (4,16) (6,18) (6,19) (7,20)]

(modulo the `to_the_right` flip, which the implementer must resolve by running the fixed code
in both orientations — the transpose swaps handedness). **Verify against 45° output**, as the
register's column says.

---

## #9's horizontal `circleCenter` case — DEFERRED TO TASK 13, and that deferral is right
`FloatPoint::circle_center` on horizontal input divides by zero. At v1.0.0 the port answers
`None` (Java answers `(x, NaN)`), and it is *order-dependent*:

    circle_center((0,0), (1000,0), (1000,1000))     -> None       (first pair horizontal)
    circle_center((1000,0), (1000,1000), (0,0))     -> None       (first pair vertical)
    circle_center((1000,1000), (0,0), (1000,0))     -> Some((500, 500))   <-- correct answer

So the circumcentre **exists and is computable**; only three of the six argument orders find
it. That is precisely why the fix is *"swap the point roles for the horizontal case"* and why
it must land with #82 — it **is** the mechanism of #82. Leave a `// deferred to Task 13 (#82)`
comment at the site, as the plan's Commit 9 requires.

---

## The #26 tail — eight sites, each with an exact witness

### #26 — `PolygonShape::area()` always returns 0
`polygon_shape.rs:454-471`: the guard is `if self.dimension() <= 2 { return 0.0 }` and
`dimension()` (`:476-484`) never exceeds 2. The shoelace body below it is **dead**.
Fix: `<= 2` → `< 2`. Also guard `corners[len − 2]` for a **1-corner** polygon
(`len − 2` underflows).

**Hand-computed expectations (both confirmed as `0` at v1.0.0):**

    square (0,0) (100,0) (100,100) (0,100):
        shoelace  = ½ |Σ (x_i·y_{i+1} − x_{i+1}·y_i)|
                  = ½ |0 + (100·100 − 100·0) + (100·100 − 0·100) + (0·0 − 0·100)|
                  = ½ |0 + 10000 + 10000 + 0| = ½ · 20000 = 10000
        AREA = 10000       (sanity: 100 × 100)

    L-shape (0,0) (100,0) (100,100) (80,100) (80,80) (0,80):
        (0,0)→(100,0)     :   0·0    − 100·0   =      0
        (100,0)→(100,100) : 100·100  − 100·0   =  10000
        (100,100)→(80,100): 100·100  −  80·100 =   2000
        (80,100)→(80,80)  :  80·80   −  80·100 =  −1600
        (80,80)→(0,80)    :  80·80   −   0·80  =   6400
        (0,80)→(0,0)      :   0·0    −   0·80  =      0
        Σ = 16800   ->  AREA = ½ · 16800 = 8400
        sanity: 100×80 (=8000) + 20×20 notch (=400) = 8400   ✔
        winding: Σ > 0, so the corner order is counter-clockwise — the constructor will not
                 reverse it, so the corner list survives unchanged.

**Why it matters beyond a unit test:** `area()` is reached from `DsnFile`, so **plane
autoroute settings derive from a zero board area**. #26 with #93 (already landed in Task 4)
changes derived board-level numbers — `G` moves and the corpus re-baselines.

### #88 — `PolygonPath::boundingBox` grows the upper x bound once per x coordinate
Port: `crates/fr-dsn/src/parser/geometry.rs:636-661`. `bounds[2] = bounds[2].max(*c) + offset`
— the `+ offset` is **outside** the `max` on the x axis (`PolygonPath.java:122`) and **inside**
it on the y axis (`:126`).

Recurrence: `b_k = max(b_{k−1}, x_{k−1}) + off`, hence `b_k = max_{j<k} ( x_j + (k−j)·off )`.

**Witness — a square path, width 200 (`offset = 100`), corners (0,0) (1000,0) (1000,1000) (0,1000):**

    coordinate_arr = [0, 0, 1000, 0, 1000, 1000, 0, 1000]   x coords in order: 0, 1000, 1000, 0
    bounds[2] starts at i32::MIN = −2147483648
      after x=0    : max(−2147483648, 0)    + 100 =  100
      after x=1000 : max(  100, 1000)       + 100 = 1100
      after x=1000 : max( 1100, 1000)       + 100 = 1200
      after x=0    : max( 1200,    0)       + 100 = 1300      CURRENT
    correct: max(x) + offset = 1000 + 100                    = 1100      EXPECTED

    the other three bounds are already right:
      bounds[0] = min(x − offset) =    0 − 100 =  −100
      bounds[1] = min(y − offset) =    0 − 100 =  −100
      bounds[3] = max(y + offset) = 1000 + 100 =  1100

    CURRENT  box = [−100, −100 ..  1300, 1100]      (over-wide by 200 = 2·offset)
    EXPECTED box = [−100, −100 ..  1100, 1100]      (square, as the path is)

The over-estimate is `(k − 2)·offset` for this path shape and grows **linearly with the corner
count** — a 100-corner keepout outline at width 200 is over-wide by ~9800 units.
Test: `bounding_box_is_square_for_a_square_path`, asserting the box's width equals its height.

### #23 — `Polyline(Point, Point)` repeats the start's closing direction
Measured at v1.0.0 for `Polyline::from_two_points((0,0), (100,0))`:

    lines[0] = Line{ a:(0,0),     b:(0,1)   }    start closing line, directed +y
    lines[1] = Line{ a:(0,0),     b:(100,0) }    the segment itself,  directed +x
    lines[2] = Line{ a:(100,0),   b:(100,1) }    end closing line,    directed +y   <-- SAME as [0]

The two closing lines are **parallel and co-directed**. `Polyline(Polygon)` gives the *opposite*
closing line at the far end (directed −y), which is the convention the rest of the code assumes
(the shape lies on the right of every line — with both closing lines pointing +y the polyline's
two ends are handed differently).

> **Invariant:** `Polyline::from_two_points(a, b)` and `Polyline::from_points(&[a, b])` produce
> the **same** line array. Today they do not, at index 2.
> Expected `lines[2] = Line{ a:(100,0), b:(100,−1) }`.

### #11 — `Simplex::cutoutFrom`'s `prevDivisionLine` is never assigned
Both merge branches are guarded on `prevDivisionLine != null`, and nothing ever assigns it, so
**both branches are dead**: `cutoutFrom` never merges two adjacent pieces and always returns
the maximal division.

> **Invariant (reachability):** for a cut-out whose division produces two adjacent pieces that
> *can* merge into one convex piece, the returned piece count is **smaller** than the number of
> division lines. Today it always equals it.
> Concretely: cutting a small square out of the **corner** of a big square gives pieces that
> merge; cutting it out of the **middle** gives pieces that do not. The two cases must differ
> after the fix and are identical today.

### #17 — `IntOctagon::contains(FloatPoint)` is inclusive where `IntBox`'s is exclusive
Measured at v1.0.0 (`current-port-behavior.txt`), octagon
`Oct[0,0,100,100,−100,100,0,200]` vs `TileShape::Box([0,0..100,100])`:

| point | on which border | `IntOctagon::contains_float` | `TileShape::Box::contains_float` |
|---|---|---|---|
| `(0, 50)`   | left   | **true**  | **false** |
| `(50, 0)`   | bottom | **true**  | **false** |
| `(100, 50)` | right  | **true**  | **false** |
| `(50, 50)`  | interior | true    | true |
| `(−1, 50)`  | outside  | false   | false |

Three of three border points disagree. **The decision is the implementer's and must be
recorded**: freerouting's `TileShape::contains(&Point)` (`tile_shape.rs:541`) is
`!is_outside`, i.e. **inclusive**, while `contains_float` (`:549`) is exclusive. The two
representations must not disagree *with each other*; whichever convention is chosen, apply it
to box, octagon and simplex in one commit — the same lesson as #48+#57.

### #18 — `IntBox::divideIntoSections` skips the base class's `dimension() == 2` filter
Measured at v1.0.0: a **degenerate** box `[0,0 .. 100,0]` (dimension 1) at
`divide_into_sections(30.0)` returns **0 sections** — the shape is lost entirely, because
`ycount = ceil(0 / 30) = 0` and the `for j in 0..ycount` loop never runs.

The base class filters `dimension() != 2` and returns the shape itself. Expected:
`[ IntBox[0,0..100,0] ]` — **1 section, area conserved**.

> **Invariant:** `divide_into_sections` never loses area:
> `Σ area(section) == area(shape)` and `⋃ section ⊇ shape`, for every shape and every
> `max_section_width > 0` — including degenerate shapes.

The healthy case is unaffected: `[−10000,−10000 .. 10000,10000]` at 10000 gives 4 sections
today and after (this is the same arithmetic Task 8's #159 depends on).

### #32 — `Circle::translateBy(RationalVector)` returns `this` unchanged
Port: `circle.rs:282-291`. The `Vector::Rational` arm falls through to `return *self` after a
dropped `FRLogger.warn`. **Every sibling shape throws** on the same input.

> **Invariant:** `shape.translate_by(v)` either translates or fails, for every shape and every
> vector kind. Silently returning the untranslated shape is the one answer no caller can detect.

Decide: implement rational translation (the circle's centre is an `IntPoint`, so a rational
translation cannot in general be represented — which is *why* Java gave up) **or** make it a
hard error like the siblings. Default: **error**, matching the siblings; record the choice.

### #188 — `new Polyline(Line[])` normalises the caller's array in place
The port already models this faithfully with two entry points
(`polyline.rs:159` `from_lines` — consumes; `:189` `from_lines_in_place` — writes back) and
`Polyline::build` returns a `writes_through` flag. The doc at `:163-188` names **six** Plan-6
call sites that read their own array back after construction and depend on the aliasing
(`TraceTightener.java:297→311`, `TraceTightener45.java:421→435`,
`TraceTightenerAnyAngle.java:158→186, 368→386, 451→465, 614→625`).

> **Invariant (the plan's `the_caller_s_array_is_not_normalised_in_place`):** constructing a
> `Polyline` from a `Vec<Line>` leaves the caller's `Vec` **unchanged**.
>
> **The migration this forces:** all six call sites above currently *rely* on the write-back.
> Making the constructor pure means each of the six must be rewritten to read the polyline's
> own `lines()` instead of its stale local array. Enumerate them in the commit message; the
> port has already located them, so no search is owed.

---

## Summary table — how each row is grounded

| # | ground truth | kind | confirmed at v1.0.0? |
|---|---|---|---|
| #5 | `(0,0)→y=10` gives `(0,−10)` not `(0,10)`; `(10,5)→(0,5)-(10,15)` gives `(5,5)` not `(5,10)` | derived formula + 2 witnesses | yes |
| #7+#68 | round-trip `border_line_index(border_line(i)) == Some(i)`; reversed line → `None` | contract from `border_line` | yes (all four `None`) |
| #177 | trace count independent of `changed_area` marking; sibling is the spec | invariant | — |
| #186 | line conservation; the existing test's failure **is** the fixed behaviour | invariant | — |
| #187 | `fixtures/p9t11-per-layer-width.dsn`, 8:1 width ratio | fixture built + validated | yes (40000 / 5000 in the SES) |
| #55 | four transform invariants + outline/keepout agreement | invariant | — |
| #183 | bend-arm execution count `0 → >0` | reachability | — |
| #48+#57 | disagreement is `360 − 2a`; use 90°, never 180° | arithmetic | — |
| #15 | `(9,9) → 0` today, `2` after; survey's sentence corrected | measured + derived | yes |
| #16 | `k` distinct points, sorted | invariant | — |
| #13 | containment + transpose symmetry; measured out-of-box output | invariant + measurement | yes |
| #26 | square 10000, L-shape 8400 | shoelace, twice | yes (`0` today) |
| #88 | `[−100,−100..1300,1100]` today, `[−100,−100..1100,1100]` expected | recurrence solved | source-verified |
| #23 | `lines[2]` must be `(100,0)→(100,−1)` | measured + convention | yes |
| #11 | corner-cut vs middle-cut piece counts must differ | reachability | — |
| #17 | 3 of 3 border points disagree between octagon and box | measured | yes |
| #18 | degenerate box → 0 sections today, 1 expected; area conservation | measured + invariant | yes |
| #32 | translate-or-fail | invariant | source-verified |
| #188 | caller's `Vec` unchanged; six call sites named | invariant | source-verified |
