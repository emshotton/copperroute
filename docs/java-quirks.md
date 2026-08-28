# Java quirks, bugs, and improvement candidates

This port is **behavioral**: it reproduces freerouting (Java, v2.3.0 baseline)
exactly, including its bugs, so that parity against the Java outputs is the
acceptance test. Every deliberate reproduction of a Java defect is recorded
here, with the Java location, where the Rust port pins it (a test or comment),
and what a fix would look like once parity is established.

Rules:
- **Do not fix these in the port until parity is proven** on the fixture corpus.
  Fixing during porting makes porting bugs indistinguishable from intentional fixes.
- Every task that finds a new one **appends a row** here (the task reviewer
  checks for it).
- Status: `pinned` = reproduced and covered by a test/comment; `totalized` =
  Rust returns a value where Java crashes (documented divergence, not a bug
  fix); `candidate` = suggested improvement, not a Java bug.

## Reproduced Java bugs and quirks (`pinned`)

| # | Java location | What Java does | Rust location | Suggested fix (post-parity) |
|---|---|---|---|---|
| 1 | `Direction.compareTo` / `IntDirection.compareTo` | Not antisymmetric when one operand is `NULL`: `RIGHT.compareTo(NULL) == -1` but `NULL.compareTo(RIGHT) == 0`. Also `(Big,Int)` and `(Int,Int)` dispatch arms disagree at the zero direction (asymmetric negation count). | `int_direction.rs` `compare_to` (no `Ord` impl on purpose); `direction.rs` cross-arm test | Define NULL as strictly less than every real direction; then `Ord` becomes lawful. |
| 2 | `IntDirection.turn45Degree(factor)` | `factor % 8` is negative for negative factors → falls to `default` → returns `(0,0)`. | `int_direction.rs` `turn_45_degree`, test | Use floor-mod (`rem_euclid`). Check callers never pass negatives first. |
| 3 | `Direction.toString` | Tests `compareTo(RIGHT) == 0`, which is true for `NULL`, so `NULL` prints `"RIGHT"`; the `"NULL"` branch is dead. | `int_direction.rs` `Display` | Use `equals`; print `"NULL"`. Diagnostic only. |
| 4 | `Point.getInstance(BigInteger x,y,z)` / `Vector.getInstance` | Reduces by `z` after checking only `x mod z == 0`; `y` is integer-divided even when not divisible (truncation). | `point.rs` / `vector.rs` `from_big`, tests | Check both `x` and `y` before dividing. Audit callers for reliance. |
| 5 | `RationalPoint.perpendicularProjection` (line ~262) | `projY = tmp1.add(tmp2)` where `IntPoint` uses `subtract` — sign bug; wrong projection for lines not through the origin. Reachable via `TileShape.nearestBorderPoint(Point)` (`ShapeTraceEntries`, `ShapeEntrySide`). | `rational_point.rs`, pinning test | Change to `subtract`, matching `IntPoint`. |
| 6 | `Line.compareTo` | Inlined copy of `IntDirection.compareTo`; non-antisymmetric for degenerate lines (`a == b`) and returns `Equal` for any parallel same-direction lines. | `line.rs` `compare_to` (no `Ord`) | Only matters for sorting in `Simplex.removeRedundantLines`; keep, or tie-break on offset. |
| 7 | `IntBox.borderLineIndex`, `IntOctagon.borderLineIndex` | Stubs: log a warning and return `-1`. Live caller `ShapeAndEntrySide.java:59,63` receives `-1`. | `int_box.rs` / `int_octagon.rs` return `None` | Implement (compare against `borderLine(i)` geometrically) — verify `ShapeAndEntrySide` behaviour changes are wanted. |
| 8 | `IntBox.getId`, `IntOctagon.getId`, `Simplex.getId`, `IntPoint.getId`, `Line.getId` | Hash-style `31*a + b` with silent `int` overflow. | `wrapping_mul/add` everywhere | Fine as a hash; never use as an identity. |
| 9 | `FloatPoint.circleCenter` | Divides by zero when `this→p1` is horizontal → `(x, NaN)`; sole caller `insideCircle` then compares against NaN → `false`. | `float_point.rs` returns `None`; test | Handle the horizontal case explicitly (swap point roles). |
| 10 | `FloatPoint.roundToGrid` | Uses `Math.rint` (ties-to-even) while every other rounding site uses `Math.round` (half-up). | `float_point.rs` (documented) | Probably harmless; unify if grid snapping ever misbehaves at `.5`. |
| 11 | `Simplex.cutoutFrom` (~line 730/860) | `prevDivisionLine` is only ever read, never assigned → always `null`; both `mergePrevDivisionLine` branches are dead. | `simplex.rs` `cutout_from` (comment) | Either delete the dead code or finish the intended merge logic (likely fewer output pieces). |
| 12 | `Simplex.removeRedundantLines` via `Direction.equals` | Two distinct zero directions are *not* equal in Java (reference shortcut only), so a doubly-degenerate input keeps 4 lines / `dimension()==0`; the Rust structural `NULL == NULL` gives 3 lines / `dimension()==1`. Degenerate input only. | `line.rs` `fast_equals` doc; `int_direction.rs` `eq` | Accepted divergence (Rust `Eq` must be reflexive). |
| 13 | `LineSegment.stairApproximation45` (~line 432) | Calls `functionValueApprox(currentY)` — a function of *x* — with a y-coordinate. | `line_segment.rs` (comment) | Use `functionInYValueApprox`. Verify against 45° routing output. |
| 14 | `LineSegment.startPointApprox` / `endPointApprox` (84-103) | Result would depend on whether `startPoint()` was called earlier (memo), but review found no Java call site ever populates the memo first, so the uncached float-intersection branch is the only observable behaviour; forcing the cached branch differs only in the sign of zero. | `line_segment.rs` (cache dropped; reproduces Java exactly) | None needed. |
| 15 | `TileShape.indexOfNearestCorner` | Seeds the minimum with `Double.MIN_VALUE` (smallest subnormal), so a corner at distance exactly 0 is never "nearest". | `tile_shape.rs` uses `JAVA_DOUBLE_MIN_VALUE` | Seed with `Double.MAX_VALUE`. |
| 16 | `TileShape.nearestBorderPointsApprox` | Upward insertion shift copies the wrong element (off-by-one when inserting into the sorted result array). | `tile_shape.rs` (comment + test) | Fix the shift; check callers that take `count > 1`. |
| 17 | `IntOctagon.contains(FloatPoint)` | Inclusive on the border where the sibling `IntBox` version is exclusive. | `int_octagon.rs` (comment) | Align the two. |
| 18 | `IntBox.divideIntoSections` | Covariant override that skips the base class's `dimension()==2` filter, returning out-of-range/empty sections (e.g. 16 raw vs 9 filtered). | `tile_shape.rs` dispatches the override; test | Apply the same filter as the base algorithm. |
| 19 | `FloatLine.segmentProjection` | Second `CRIT_INT` guard sits outside the `if/else` so it also applies to the untouched `b` endpoint. | `float_line.rs` (kept) | Probably intended; document. |
| 21 | `LineSegment.stairApproximation` / `45` | `new IntPoint[2*stairCount+1]` throws `NegativeArraySizeException` for `width <= 0` or non-finite; zero callers in the Java tree. | `line_segment.rs` `.max(0)` (deferred minor: prefer assert) | Reject non-positive width up front. |
| 20 | `IntPoint.fortyfiveDegreeProjection` | Tie-breaks via a chain of `==` on doubles in a fixed order. | `int_point.rs` | Fine; note ordering dependence. |
| 22 | `Polyline(Line[])` → `removeOverlaps` (Polyline.java:147-155) | Once the loop has decremented `newLength` to 0, `tmpArr[newLength - 1]` reads index -1 → `ArrayIndexOutOfBoundsException`. Reached by ~11% of random line arrays drawn from a small pool of equal/opposite lines (280k-case sweep); the six lines `h, v, h, v, h, v` over the same two axes are a minimal case. **Not totalized:** `PolylineTrace.combine_at_end` (PolylineTrace.java:303-311) tests `joinedPolyline.lines.length != newLineCount` right after the constructor, so a zero-line polyline and a thrown exception are *different* observable outcomes — Java's pass-level `catch (Exception)` aborts the pass. | `polyline.rs` `remove_overlaps` / `from_lines` return `Err(PolylineError::NormalizationIndexUnderflow)`, so Plan 2's `combine_at_end` can abort the pass like Java's pass-level catch; pinning test | Guard the loop with `newLength >= 1`, as the trailing access at line 160 already is. |
| 23 | `Polyline(Point, Point)` (Polyline.java:69) | Recomputes the *end* closing direction as `fromCorner → toCorner`, a verbatim repeat of line 66, where `Polyline(Polygon)` (line 50) uses `last → second-last`. The two constructors therefore hand back opposite (geometrically identical) closing lines for the same pair of points. | `polyline.rs` `from_two_points`, test `the_closing_lines_are_perpendicular_to_the_end_segments` | `Direction.getInstance(toCorner, fromCorner)`; check nothing depends on the current orientation first. |
| 24 | `TileShape.rotateApprox` (TileShape.java:692-695) | The two-corner branch builds `new LineSegment(currentPolyline, 0)`, but that constructor's valid range starts at 1, so it stores three `null` lines and `toSimplex()` throws a `NullPointerException`. ~2% of random degenerate 2..4-line shapes reach it. | `tile_shape.rs` `rotate_approx` (returns `Simplex::EMPTY`) | Pass 1 instead of 0. |
| 25 | `Polyline.cornerCount()` (Polyline.java:178-181) | Returns -1 for an empty polyline, which then flows into `new IntPoint[cornerCount()]` in `rotateApprox` (`NegativeArraySizeException`) and into `boundingBox(0, -2)`. | `polyline.rs` `corner_count` saturates at 0 | Return 0, or make the empty polyline unrepresentable. |
| 26 | `PolygonShape.area()` (PolygonShape.java:411) | Guards with `if (dimension() <= 2) return 0;`, but `PolygonShape.dimension()` never exceeds 2 (PolygonShape.java:430-442), so **`area()` always returns 0** and the shoelace sum below it is dead code. Reachable from `DsnFile.java:70,89` through `Shape.area()`. | `polygon_shape.rs` `area` (`// Java bug:` comment, shoelace body kept); test `convexity_area_and_split` pins `square().area() == 0.0` | Change `<= 2` to `< 2`; then also guard the `corners[len - 2]` read for a 1-corner polygon. |
| 27 | `PolygonShape.intersects(Shape)` (PolygonShape.java:118-121) | `Shape.java` declares no `intersects(PolygonShape)` overload, so `shape.intersects(this)` binds to `intersects(Shape)`. Polygon-vs-tile and polygon-vs-circle bounce back and terminate, but **polygon-vs-polygon recurses until `StackOverflowError`** (verified in a JDK 23 harness). | `shape.rs` `Shape::intersects_polygon` panics with the reason instead of recursing; test `polygon_against_polygon_reproduces_the_java_stack_overflow` | Add an `intersects(PolygonShape)` to `Shape` (split-to-convex on both sides), or make `PolygonShape.intersects(Shape)` type-test its argument. |
| 28 | `PolygonShape.containsOnBorder` (PolygonShape.java:228-232) | Stub: the warning is commented out and the body is `return false`, so `containsInside(point) == contains(point)` for every polygon. | `polygon_shape.rs` `contains_on_border` / `contains_inside`; test `stubs_match_the_java_stubs` | Implement (test the point against each `borderLine`); check callers that rely on the current permissive answer. |
| 29 | `PolygonShape.cutout/enlarge/borderDistance/distance`, `Circle.nearestPointApprox/cutout` | Unimplemented stubs that warn and return `null` / `0`. `smallestRadius()` therefore always answers 0 for a polygon. | `polygon_shape.rs`, `circle.rs` (`Option` / `0.0`); test `stubs_match_the_java_stubs` | Implement; `smallestRadius` in particular is used for clearance heuristics. |
| 30 | `PolygonShape.splitToConvex` (PolygonShape.java:17-18, 530-548) | The `Random` that picks the concavity-scan start is a **`static` field shared by every instance**, reseeded to 99 at each cold call. Single-threaded that is deterministic, but the autorouter splits shapes from several threads, so concurrent calls interleave draws and the division becomes non-reproducible. | `polygon_shape.rs` builds a per-call `JavaRandom`, which is deterministic under concurrency too — the only intended divergence, and a strict improvement | Make the `Random` a local, as this port does. |
| 31 | `PolygonShape.DivisionPoint` (PolygonShape.java:674, 769) | Seeds `minProjectionDist` with `Integer.MAX_VALUE` (an `int` sentinel in a `double`) and detects "projection not found" by comparing back against it, so a legitimate projection at distance exactly 2^31-1 would be discarded. Unreachable while coordinates stay below `CRIT_INT` = 2^25. | `polygon_shape.rs` `DivisionPoint::new` (kept verbatim) | Use `Double.MAX_VALUE` and a `null` check. |
| 32 | `Circle.translateBy(Vector)` (Circle.java:249-252) | For a `RationalVector` it warns and returns **`this` unchanged** — a silently wrong shape rather than an exception, unlike `Line`/`IntBox`/`Polyline`, which throw. | `circle.rs` `translate_by` (ported as-is) | Round the rational vector, or throw like the siblings. |

## Rust-side totalizations (`totalized`) — Java crashes, Rust returns a value

| Java behaviour | Rust behaviour | Location |
|---|---|---|
| `Simplex.EMPTY.cornerIsBounded(0)` throws AIOOBE | `false` | `simplex.rs` |
| `TileShape.getInstance(new Point[0])` throws | `Simplex::EMPTY` | `simplex.rs` `from_points` |
| `Simplex.EMPTY.offset(-1)` throws | `EMPTY` | `simplex.rs` |
| `IntBox(5,5,0,0).divideIntoSections(3)` → array containing `null` | `[]` | `int_box.rs` |
| `IntBox.divideIntoSections` on empty box → negative array size | `[]` | `int_box.rs` |
| `FloatPoint.toString` on NaN/∞ | prints `NaN`/`∞` like Java (fixed after review) | `float_point.rs` |
| `Line.translateBy(RationalVector)` → ClassCastException | `None` (`translate_by_any`) | `line.rs` |
| `IntBox.translateBy(RationalVector)` / `IntOctagon` same → ClassCastException | `panic!` (documented) | `int_box.rs`, `int_octagon.rs` |
| `Polyline.cornerCount()` on an empty polyline → -1 | `0` | `polyline.rs` |
| `Polyline.cornerApprox(i)` on an empty polyline → NegativeArraySizeException | `None` | `polyline.rs` |
| `Polyline.rotateApprox` on an empty polyline → NegativeArraySizeException | empty `Polyline` | `polyline.rs` |
| `Polyline.distance` on a corner-less polyline → NPE | `f64::MAX` | `polyline.rs` |
| `Polyline.offsetBox(halfWidth, no)` out of range → NPE via a null-lined `LineSegment` | `None` | `polyline.rs` |
| `Polyline.translateBy(RationalVector)` → ClassCastException | `panic!` (documented), as `IntBox`/`Simplex` | `polyline.rs` |
| `Polyline(Polygon)` / `Polyline(Point[])` with rational corners → warning, then ClassCastException in every later call | `panic!` in the constructor | `polyline.rs` `int_point_of` |
| `Polyline.projectionLine(RationalPoint)` → warning from `new Line(point, dir)`, then a broken line every later call rejects | `panic!` mid-loop | `polyline.rs` `projection_line` |
| `TileShape.rotateApprox` two-corner branch → NPE (quirk #24) | `Simplex::EMPTY` | `tile_shape.rs` |
| `TileShape.cutout(Polyline)` with an empty polyline on a shape that has border lines → NPE | `[polyline]` — the same answer Java itself gives for a border-line-free shape, whose `containsInside` returns before dereferencing the null corner (TileShape.java:197-201) | `tile_shape.rs` `cutout_polyline` |

**Withdrawn in Task 17.** `TileShape.distance` / `borderDistance` / `smallestRadius` on a shape
with no border lines was totalized to `f64::MAX` in Task 14. Task 17's differential sweep reaches
it from `Circle.intersects(Simplex)` by way of `PolygonShape.intersects(Circle)` on a
self-intersecting polygon, where Java's NullPointerException and a plausible distance are
different observable outcomes, so the three methods now panic
(`tile_shape.rs`, three `#[should_panic]` tests).

## Improvement candidates (`candidate`) — not Java bugs

| Idea | Why | Where |
|---|---|---|
| Replace `BigInteger` fallback with `i128` where products provably fit | Java promotes to `BigInteger` above 2^25; most intermediate products fit in `i128` — big speed win in `Line.intersection`, `perpendicularProjection`. | `line.rs`, `int_point.rs`, `rational_*` |
| Lawful `Ord` on `IntDirection`/`Line` once NULL is defined as minimum | Enables `BTreeMap`/`sort` without `sort_by` shims. | see #1, #6 |
| `pub(crate)` fields on `RationalPoint`/`BigIntDirection` | Java package-private; guards the `z >= 0` invariant. | `rational_point.rs` |
| Rename `RationalVector::determinant` → `numerator_determinant` | Ignores denominators; sign-correct only. | `rational_vector.rs` |
| Add `Hash` to `Vector`/`Direction` when a map key is needed | Java has no `hashCode` there; add only on demand. | `vector.rs`, `direction.rs` |
| `Display` for `FloatPoint` uses exact binary expansion, Java uses shortest-round-trip digits | Diverges above 2^53 and at 4th-digit ties; diagnostic only. | `float_point.rs` |
| Drop `precalculated*` memo fields (already done) | Keeps geometry types `Copy`/`Eq`/`Hash`; recompute is cheap. In `Polyline` the memo is also *observably* equivalent: the `Polyline(Line[])` constructor fills it before flipping a line, and `intersectionApprox` negates numerator and denominator together, which is exact. | `simplex.rs`, `line_segment.rs`, `polyline.rs` |
| Memo cache for `PolygonShape` convex pieces / `PolylineArea` split — Java memoizes in `precalculatedConvexPieces`; port recomputes per call; `DrillPage`/`ConductionArea` overlap checks become O(n²) — Plan 2 adds a cache | Correctness parity is unaffected (recompute gives the same pieces), but the missing memo turns an amortized-cheap Java path into a quadratic one once real boards call it repeatedly. | `polygon_shape.rs`, `polyline_area.rs` |
| Pass-level recovery boundary — Java `AutoroutePassRunner.java:144` catches `Exception` per pass and `BatchAutorouterThread.java:584` catches `Throwable`; the port's panics (Java NPE/StackOverflow equivalents) need `catch_unwind` or `Result` at those boundaries or a panic aborts the whole run where Java aborts one pass — Plans 6/7 obligation | Several rows above (`PolygonShape.intersects`, `Polyline` normalisation, `TileShape.rotateApprox`) reproduce a Java exception as a Rust panic; Java's pass-level `catch` recovers and continues routing, so a bare panic in the port is strictly worse unless the autorouter loop wraps these calls. | `autoroute/` (Plan 6/7) |

## Process notes

- Reviews that extract the Java method bodies verbatim into a JDK-23 harness
  and diff against the Rust on random inputs have been the most effective
  verification (20k–525k comparisons per task). Prefer that over hand-tracing
  for any algorithm over ~100 lines.
- Every deliberate divergence must be greppable: `// not ported:` for skipped
  members, `// totalized:` for crash-to-value changes, `// Java bug:` for
  reproduced defects.
- The differential harness lives in `scripts/differential/`.
