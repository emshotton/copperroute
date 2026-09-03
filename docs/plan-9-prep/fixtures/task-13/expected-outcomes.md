# Task 13 — airlines, incompletes and board history: independent ground truth

Everything below is derived first and measured second, against the port at **tag `v1.0.0`
(`ecc0abf`)** in a read-only scratchpad clone. Raw probe output: `current-port-behavior.txt`.

---

## #82 + #9 — the Delaunay triangulation loses edges

### The invariant that makes this testable without an oracle — **THE KEY RESULT**

For a **planar triangulation** of `n` points with `h` of them on the convex-hull **boundary**
(collinear hull-edge points included), Euler's formula gives

    F = 2n − h                      (faces, outer face included)
    triangles = 2n − h − 2
    E = 3n − 3 − h                                                              (★)

A `k × k` pad grid has `n = k²` and `h = 4k − 4`, so

    E(k) = 3k² − 3 − (4k − 4) = 3k² − 4k + 1 = **(3k − 1)(k − 1)**               (★★)

    k = 2 :  (5)(1)  =   5          k = 5 :  (14)(4) =  56
    k = 3 :  (8)(2)  =  16          k = 6 :  (17)(5) =  85
    k = 4 :  (11)(3) =  33          k = 7 :  (20)(6) = 120

**These are exactly the plan's binding literals: 5, 56, 85.** ✔

**Why (★) is the right invariant even though a square grid is degenerate.** Every unit cell of
a square grid has four cocircular corners, so the Delaunay triangulation is **not unique** —
either diagonal is legal. But (★) counts edges of *any* triangulation of the point set, and
which diagonal is chosen does not change the count. So the assertion is stable under the
degeneracy that causes the bug, which is exactly what a post-fix test needs.

`h` was also verified computationally, not assumed: the probe recomputes the hull-boundary
count from the points and gets `4k − 4` for `k = 2..7` (`current-port-behavior.txt`).

### Measured at v1.0.0 — and a warning about the survey's numbers

| k | n | h | expected `3n−3−h` | **v1.0.0 (pitch 1000, at the origin)** | deficit |
|---|---|---|---|---|---|
| 2 | 4  | 4  | **5**   | 4   | 1 |
| 3 | 9  | 8  | **16**  | 14  | 2 |
| 4 | 16 | 12 | **33**  | 32  | 1 |
| 5 | 25 | 16 | **56**  | 53  | 3 |
| 6 | 36 | 20 | **85**  | 82  | 3 |
| 7 | 49 | 24 | **120** | 116 | 4 |

> ### ⚠ The survey's `5×5 → 50/56` and `6×6 → 75/85` do **not** reproduce.
> At v1.0.0 the same grids give **53/56** and **82/85**. Worse, the deficit is
> **coordinate-dependent** — six different grid placements were tried:
>
> | placement | k=2 | k=5 | k=6 | k=7 |
> |---|---|---|---|---|
> | origin, pitch 1000 | 4/5 | 53/56 | 82/85 | 116/120 |
> | origin, pitch 1 | 4/5 | 53/56 | 82/85 | 116/120 |
> | origin, pitch 100 | 4/5 | **54**/56 | 82/85 | 116/120 |
> | offset (1,1), pitch 1000 | 4/5 | 53/56 | **83**/85 | **119**/120 |
> | straddling origin, pitch 1000 | 4/5 | **51**/56 | **83**/85 | **119**/120 |
> | far from the axes, pitch 1000 | 4/5 | 53/56 | 82/85 | **119**/120 |
>
> **Do not put a fail-before literal in the test.** The *post*-fix numbers (5, 56, 85, 120) are
> coordinate-independent — that is the whole point of (★) — and are the only literals worth
> asserting. If a fail-before number is wanted for the commit message, measure it on the exact
> grid the test uses, in this task, and say which grid it was.
>
> `k = 2 → 4/5` is the one figure that reproduces everywhere, and it is the one the port's own
> pinning test `square_pins_javas_four_edges` (`delaunay.rs:1439`) carries. Use that as the
> headline.

### The mechanism, confirmed at the arithmetic level

Two independent defects, and (★) is broken by both:

**(a) The bounding triangle is finite and touches the axes.** `delaunay.rs:309-322`:

    bounding_coor = CRIT_INT = 2^25 = 33_554_432
    B0 = ( 2^25,      0 )      <-- exactly on the x axis
    B1 = (     0,  2^25 )      <-- exactly on the y axis
    B2 = (−2^25, −2^25 )

Any input point with `y == 0` is **collinear with B0 and the origin**; any with `x == 0` is
collinear with B1. Ordinary axis-aligned pad rows sit on those lines. A bounding triangle
"pushed out where no input can be collinear with it" means: irrational-direction corners, or
corners chosen from the input's own bounding box with a guaranteed-general-position offset.

**(b) `FloatPoint::circle_center` degenerates to a non-answer on axis-aligned triples.**
Measured at v1.0.0 — the **same three points in three different orders**:

    circle_center((0,0),       (1000,0),    (1000,1000))  ->  None
    circle_center((1000,0),    (1000,1000), (0,0)      )  ->  None
    circle_center((1000,1000), (0,0),       (1000,0)   )  ->  Some((500, 500))   <-- CORRECT

The circumcentre of `(0,0), (1000,0), (1000,1000)` is `(500, 500)` (radius `500√2`), and it is
**computable** — only three of the six argument orders find it. In Java the two failing orders
return `(x, NaN)` rather than `null`, and `insideCircle` then answers `false`, i.e. *"legal, do
not flip"*. That is #9, and it **is** the mechanism of #82: this is why the two fixes are one
commit.

The port's own `square_pins_javas_four_edges` (`delaunay.rs:1439-1466`) already documents the
chain end to end for the 2×2 case: shuffled insertion order `[P2, P1, P4, P3]`, the flip test
`P4.insideCircle(B1, P3, P2)`, `P2`/`P3` sharing an x coordinate, `slope2 = (0−1000)/(1000−1000)`
infinite, centre `(NaN, NaN)`, "legal", top edge never built.

**Fix:** an **exact** `inCircle` determinant over `Point` using the `BigInt` machinery `sideOf`
already has. The classic 4×4 lift determinant

    | ax−dx  ay−dy  (ax−dx)²+(ay−dy)² |
    | bx−dx  by−dy  (bx−dx)²+(by−dy)² |   > 0  <=>  d is inside the circle through a,b,c
    | cx−dx  cy−dy  (cx−dx)²+(cy−dy)² |        (for a,b,c counter-clockwise)

is exact in `BigInt` over `IntPoint` coordinates — **no circumcentre is ever computed**, so #9's
division by zero simply ceases to exist on this path. Coordinates are bounded by `CRIT_INT = 2^25`,
so the differences fit 27 bits, the squares 54, and the 3×3 expansion ~163 bits — well inside
`BigInt` and far outside `i128`, so `BigInt` (not a widened primitive) is the right choice.

### The property that retires the private `JavaRandom`
> **Invariant:** once `in_circle` is exact and no input can be collinear with the bounding
> triangle, the Delaunay edge set is **independent of insertion order**.
> Assert: two different shuffles of the same point list give the same edge set (as a set of
> unordered coordinate pairs).

Today `delaunay.rs:305-307` shuffles with a private `JavaRandom(SEED)` copy *precisely because*
the order decides the answer. The property above is what lets Task 24 delete the PRNG.
**Task 13 must not delete it** (ruling BP6); it only makes the deletion sound.

### ⚠ The plan's witness-pad test is vacuous as written

The plan binds
`a_seven_by_seven_grid_straddling_the_origin_leaves_no_witness_pad_edgeless` — 2 000 dense
draws, **0 failures**. Measured at v1.0.0 over 2 000 dense random draws of 49 distinct points
in `[−3500, 3500]²`:

    draws with a pad that has NO incident edge : 0 / 2000   (0.00 %)   <-- ALREADY 0
    draws whose edge count is short of 3n−3−h  : 17 / 2000   (0.85 %)  <-- the real signal

**The edgeless-pad assertion passes today**, so as written it cannot fail before the fix and
proves nothing after it. The survey's *"~0.5 % of dense draws come apart"* matches the
**edge-count** rate (0.85 % measured), not the edgeless-pad rate.

**Replace it with the invariant version:**

> `a_seven_by_seven_dense_draw_keeps_every_edge` — 2 000 dense draws of 49 points straddling
> the origin; for each, assert `edges.len() == 3n − 3 − h` with `h` recomputed from the draw.
> Before: **17 / 2000 fail**. After: **0 / 2000**.

Keep the edgeless-pad check as a second, weaker assertion if a "witness pad" story is wanted
for the report — but the edge-count one is the test with teeth. If "witness pad" was meant in
the **airline/MST** sense (a `NetItem` with no incident *airline candidate* after
`NetIncompletes` runs) rather than the raw triangulation, that is a different measurement one
level downstream, and it should be stated as such.

A deterministic companion fixture is available too: the **regular 7×7 grid at pitch 1000
straddling the origin** gives **119 edges against 120** at v1.0.0 — one missing edge, no
randomness, reproducible.

---

## #147 — `NetIncompletes.Edge.compareTo` is not injective

The comparator's own comment says the four coordinate tie-breakers exist *"so that edges with
the same length are not skipped in the set"*. They do not achieve it, because they compare
**coordinates**, and two different `NetItem`s can sit at the same coordinates.

### Witness — a via stacked on a pad
    net N has items:  A = Pin  at (1000, 2000)
                      B = Via  at (1000, 2000)     <-- stacked on the pad, same point
                      C = Pin  at (5000, 2000)

    candidate airline 1 : A -> C      from (1000,2000)  to (5000,2000)  length² = 16e6
    candidate airline 2 : B -> C      from (1000,2000)  to (5000,2000)  length² = 16e6

    compareTo: lengths equal        -> tie
               from.x  1000 == 1000 -> tie
               from.y  2000 == 2000 -> tie
               to.x    5000 == 5000 -> tie
               to.y    2000 == 2000 -> tie
               -> 0.  TreeSet.add drops airline 2 whole.

The two airlines are between **different items** and both are real: A and B are separately
connectable and the MST needs both candidates to choose correctly. Two pads at one location
give the same collision.

**Fix:** break the remaining tie on **the two `NetItem` indices**, which are already to hand at
the construction site.

> **Invariant:** `|edge_set| == |edge_candidates|` whenever no two candidates are the *same
> pair of items*. Assert on the stacked-via case above: **2 edges, not 1.**

### The NaN half
`Signum::asInt` maps `NaN` to **0** (`signum.rs:35-43`: `if v > 0 {1} else if v < 0 {−1} else {0}`,
and both comparisons are false for NaN). So an edge with a NaN length compares **equal to
everything**, and a `TreeSet` containing it becomes arbitrary.

**Fix:** `Double.compare` — `f64::total_cmp` in the port (Task 23's `java_double_compare` row).
> **Invariant:** a NaN length does not compare equal to a finite one; `a.cmp(b) != Equal` for
> `a.length = NaN`, `b.length = 1.0`.

### Landing
Same channel as #82 — the edge set feeds the airline set feeds `AIRLINE_BUDGETS`. **One
commit**, and every number that grew is read and named, one line per stem. **Expect the honest
incomplete count to RISE.** That is survey §7.4's standing example: a fix that makes the score
worse and is still correct lands anyway, with the reason recorded.

---

## #197 + #198 — `BoardHistory`

Three invariants, all constructible without a board.

### #197 — `getMaxScore` seeds with `0`, not `−inf`
> **I1.** An **empty** history can trigger a restore.
> `BoardHistory::new().max_score()` must be `f32::NEG_INFINITY`, so *any* candidate is an
> improvement. Today it is `0.0`, so an empty history refuses every board with a
> non-positive score — and refuses to restore at all.
>
> **I2.** A history whose only entry scores `−5.0` restores against a board scoring `−3.0`.
> Today `max_score` answers `max(0.0, −5.0) = 0.0`, so `−3.0 > 0.0` is false and the
> restore never fires. After the fix `max_score = −5.0` and `−3.0 > −5.0` fires.
>
> Scores **are** negative in practice: `calculateScore` subtracts weighted penalties.

### #198 — `restoreBoard` sorts the list in place under a *read* lock; `getRank` reports the current position
> **I3.** `get_rank(b)` is **score order** from the first insertion, not insertion order until
> the first restore and score order afterwards.
>
> Assert: insert boards scoring `10, 30, 20` in that order.
>   * Today, before any restore: `get_rank` = 0, 1, 2 (insertion order) — so the board scoring
>     30 has rank 1 and the board scoring 10 has rank 0.
>   * After the first `restore_board` call: the list is sorted, and the ranks become
>     1, 0, 2 (score-descending) — **the same board's rank changed with no board change**.
>   * After the fix: 1, 0, 2 from the start; `restore_board` never re-sorts and never mutates
>     under a read lock.

**Why it matters:** the pass loop **breaks** when `get_rank > BOARD_RANK_LIMIT`. So *how many
restores have happened* changes when the router stops — i.e. it changes the final output of
every multi-pass run.

**A note the plan already carries and which must not be lost:** the rank break is **#217's dead
arm** (Task 9). Fixing #198 does **not** make it reachable — that was a separate decision,
already taken. Say so in the commit message so the next reader does not re-litigate it.

---

## #194 — the fanout block reads net index 0 only

### The fixture — BUILT, ROUTED, VALIDATED
`fixtures/p9t13-multi-net-smd-pin.dsn`. Four SMD pads (single-layer `F.Cu` padstacks, so they
really are SMD), two nets, and **one pad on both nets**:

    U1-1   nets: NFIRST, NSECOND     <-- the multi-net SMD pin
    U2-1   nets: NFIRST
    U3-1   nets: NSECOND
    U4-1   nets: NSECOND

    (wiring) carries a `(type protect)` trace U1-1 -> U2-1 on F.Cu, so at input time
    U1-1 is CONNECTED on its first net and UNCONNECTED on its second.

### The expectation, derived

The block (`crates/fr-router/src/score/statistics.rs:466-496`) is

    for pin in smd_pins:
        if pin.net_count() == 0: continue
        total_pins += 1
        net_number = pin.get_net_number(0)                      // <-- index 0 ONLY
        if board.unconnected_set(pin, net_number).is_empty():
            already_connected += 1
    pins_to_escape = total_pins - already_connected

    CURRENT:
      U1: net index 0 = NFIRST; NFIRST is fully connected -> already_connected += 1
      U2: NFIRST connected                                -> already_connected += 1
      U3: NSECOND unconnected                             -> no
      U4: NSECOND unconnected                             -> no
      total_pins = 4, already_connected = 2  ->  pins_to_escape = 2

    POST-FIX (loop every net index; a pin is already-connected only if it is connected on ALL):
      U1: NFIRST empty, NSECOND NOT empty                 -> NOT already_connected
      U2: yes.  U3, U4: no.
      total_pins = 4, already_connected = 1  ->  pins_to_escape = 3

**Measured at v1.0.0 on this exact fixture:**

    fanout.total_smd_pins = 4
    fanout.pins_to_escape = 2        <-- matches the derivation
    fanout.escaped_count  = 2

> **The binding expectation: `pins_to_escape` goes 2 → 3, `total_smd_pins` stays 4.**
> `BatchFanout` then no longer skips U1.

### ⚠ A second obligation the fix sketch does not name
`escaped_count` uses `is_pin_escaped` (`statistics.rs:514`), which is **net-blind**: it asks
only whether the pin has *any* clean trace/via contact. U1 has one (its NFIRST trace), so
`escaped_count` stays **2** even after the net loop is added. If `escaped_count` is meant to
answer *"how many pins need no escape"* it must become `is_pin_escaped(board, pin, net_number)`
and be evaluated per net index too. **Decide and record**: either
(a) fix only `pins_to_escape` (the plan's literal reading) — then `escaped_count` stays 2 and
its meaning is "pins with at least one escape", or
(b) make both net-aware — then `escaped_count` also moves, 2 → 1 on this fixture.
The plan's test name `a_pin_unconnected_on_its_second_net_needs_an_escape` reads as (a).

---

## #148 — `AirLine.compareTo` compares the net name alone

    compareTo(other) = this.net.name.compareTo(other.net.name)

So every airline of one net compares **equal** to every other airline of that net: a
`TreeSet<AirLine>` would collapse a net's 29 airlines into **1**. Plus an NPE waiting on a
`null` net.

**Latent** — nothing in the Java tree sorts or set-collects `AirLine`s, and the port already
effectively does not implement `Comparable`.

**Default (the plan's, and the right one): drop `Comparable` entirely**, and record that the
ordering is **not a contract**.
> **Test (`airline_is_not_comparable`):** the type has no `Ord`/`PartialOrd` impl — a
> compile-fail test, or simply the absence of the impl plus a doc comment saying why. If
> instead an ordering is kept, it must be name → both item ids → both corners, and the
> 29-airlines-to-1 collapse is the fail-before witness.

---

## Cross-cutting: the gate-version bump, and reading incompletes both ways

`incomplete_count` **changes meaning** in this task (ruling BP8): after #82 the ratsnest is
honest, so a *higher* number is a *better* board. The same commit re-cuts Task 11's affected
baseline columns under the new gate `g2` and commits both, and **G2 reports `incomplete_count`
both ways on the same board** — pre-#82 metric and post-#82 metric side by side — for this task
and the next. Without that, a later task's genuine improvement is indistinguishable from the
metric moving underneath it.

## Timing
Task 13 is one of the four tasks named in advance as likely to escalate: an exact `BigInt`
in-circle determinant replaces a float circumcentre **in the hot ratsnest path**. Gate `cpu_s`
against Task 11's `stem-times.tsv` with BO's thresholds; the escalation is pre-authorised.
A cheap mitigation worth trying first: a **floating-point filter** — compute the determinant in
`f64` with an error bound, and fall back to `BigInt` only when the bound straddles zero. That
is the standard shewchuk-style arrangement and typically keeps 95 %+ of calls in `f64`.

---

## Summary table

| # | ground truth | kind | measured at v1.0.0 |
|---|---|---|---|
| #82 | `E = 3n − 3 − h = (3k−1)(k−1)`; 5 / 56 / 85 / 120, coordinate-independent | Euler's formula, hull count verified computationally | yes, all six k and six placements |
| #82 (order) | edge set independent of insertion order | property | — |
| #9 | `(0,0),(1000,0),(1000,1000)` has circumcentre `(500,500)`; 3 of 6 orders find it | 3 orders measured | yes |
| #147 | stacked via/pad → **2** airlines, not 1; NaN ≠ finite | constructed witness | — |
| #197 | empty history `max_score = −inf`; `−3 > −5` restores | invariant | — |
| #198 | ranks `1,0,2` from the start, unchanged by a restore | invariant | — |
| #194 | `fixtures/p9t13-multi-net-smd-pin.dsn`: `pins_to_escape` **2 → 3**, `total_smd_pins` 4 | fixture built + derived + measured | **yes (2)** |
| #148 | 29 airlines collapse to 1; drop `Comparable` | arithmetic | — |
