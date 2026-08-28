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

## Rust-side totalizations (`totalized`) — Java crashes, Rust returns a value

| Java behaviour | Rust behaviour | Location |
|---|---|---|
| `Simplex.EMPTY.cornerIsBounded(0)` throws AIOOBE | `false` | `simplex.rs` |
| `TileShape.getInstance(new Point[0])` throws | `Simplex::EMPTY` | `simplex.rs` `from_points` |
| `Simplex.EMPTY.offset(-1)` throws | `EMPTY` | `simplex.rs` |
| `distance`/`borderDistance`/`smallestRadius` on a shape with no border lines → NPE | `f64::MAX` | `tile_shape.rs` |
| `IntBox(5,5,0,0).divideIntoSections(3)` → array containing `null` | `[]` | `int_box.rs` |
| `IntBox.divideIntoSections` on empty box → negative array size | `[]` | `int_box.rs` |
| `FloatPoint.toString` on NaN/∞ | prints `NaN`/`∞` like Java (fixed after review) | `float_point.rs` |
| `Line.translateBy(RationalVector)` → ClassCastException | `None` (`translate_by_any`) | `line.rs` |
| `IntBox.translateBy(RationalVector)` / `IntOctagon` same → ClassCastException | `panic!` (documented) | `int_box.rs`, `int_octagon.rs` |

## Improvement candidates (`candidate`) — not Java bugs

| Idea | Why | Where |
|---|---|---|
| Replace `BigInteger` fallback with `i128` where products provably fit | Java promotes to `BigInteger` above 2^25; most intermediate products fit in `i128` — big speed win in `Line.intersection`, `perpendicularProjection`. | `line.rs`, `int_point.rs`, `rational_*` |
| Lawful `Ord` on `IntDirection`/`Line` once NULL is defined as minimum | Enables `BTreeMap`/`sort` without `sort_by` shims. | see #1, #6 |
| `pub(crate)` fields on `RationalPoint`/`BigIntDirection` | Java package-private; guards the `z >= 0` invariant. | `rational_point.rs` |
| Rename `RationalVector::determinant` → `numerator_determinant` | Ignores denominators; sign-correct only. | `rational_vector.rs` |
| Add `Hash` to `Vector`/`Direction` when a map key is needed | Java has no `hashCode` there; add only on demand. | `vector.rs`, `direction.rs` |
| `Display` for `FloatPoint` uses exact binary expansion, Java uses shortest-round-trip digits | Diverges above 2^53 and at 4th-digit ties; diagnostic only. | `float_point.rs` |
| Drop `precalculated*` memo fields (already done) | Keeps geometry types `Copy`/`Eq`/`Hash`; recompute is cheap. | `simplex.rs` |

## Process notes

- Reviews that extract the Java method bodies verbatim into a JDK-23 harness
  and diff against the Rust on random inputs have been the most effective
  verification (20k–525k comparisons per task). Prefer that over hand-tracing
  for any algorithm over ~100 lines.
- Every deliberate divergence must be greppable: `// not ported:` for skipped
  members, `// totalized:` for crash-to-value changes, `// Java bug:` for
  reproduced defects.
