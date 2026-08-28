# Crate survey for `fr-geometry` — what could be off-the-shelf, and what it would cost

**Date:** 2026-08-27 · **Status:** exploration, not a recommendation to rewrite
**Scope:** `crates/fr-geometry` (18.9k LOC, 32 modules) against the parity contract in
`docs/superpowers/specs/2026-08-27-freerouting-rust-port-design.md` §3/§5/§6 and the 25 pinned
quirks in `docs/java-quirks.md`.

## 0. The constraints that decide almost every answer

Reading `line.rs`, `simplex.rs`, `int_octagon.rs`, `polyline.rs` and `polygon_shape.rs`, four
properties dominate:

1. **The code is not "geometry", it is "Java's geometry".** `Line::translate(f64)` rounds through
   `java_round` (half-up, not `f64::round`); `java_min`/`java_max` reproduce NaN/`-0.0` handling;
   `Simplex::remove_redundant_lines` sorts with a deliberately non-antisymmetric `compare_to`
   (quirk #1/#6); `TileShape::index_of_nearest_corner` seeds with `Double.MIN_VALUE` (#15).
   No crate reproduces these, and reproducing them is the product.
2. **`PolygonShape::split_to_convex` embeds a hand-ported `java.util.Random` LCG**
   (`polygon_shape.rs:41-59`, `JavaRandom::next(bits)`/`next_int(bound)`). Its output piece set is
   seed-dependent and is what lands in the search tree. Any third-party decomposition changes it.
3. **Outputs are *ordered sets of pieces*, not regions.** `Simplex::cutout_from` returns
   `Vec<Simplex>` in a specific order (with dead `prevDivisionLine` merge code, #11);
   `Polyline::offset_shapes` returns exactly `line_count - 2` overlapping convex tiles built by a
   dog-ear-cut heuristic. Downstream, order and count feed the search tree → maze expansion order →
   the route. "Same region, different pieces" is a parity failure, not a wash.
4. **The exact paths are already exact and already cheap.** `i32` coords, `i64` determinants,
   `BigInt` above `CRIT_INT`. There is no robustness bug to buy a crate for.

## 1. Landscape snapshot (checked via `cargo info` + crates.io API, 2026-08-27)

| Crate | Ver | Updated | License | Numeric domain | Relevance |
|---|---|---|---|---|---|
| `i_overlay` | 8.1.0 | 2026-08-16 | MIT/Apache | **i16/i32/i64** and f32/f64 | polygon booleans |
| `i_shape` / `i_float` | 4.0 / 4.1 | 2026-08 | MIT | int + fixed-point | support types for above |
| `i_triangle` | 0.48.0 | 2026-08-24 | MIT/Apache | int + float | triangulation, convex decomposition |
| `geo` | 0.33.1 | 2026-04-20 | MIT/Apache | f64 (generic, but algos assume float) | booleans, predicates |
| `geo-clipper` | 0.9.0 | 2025-02-08 | ISC | f64 API, i64 internal (Clipper1 FFI) | booleans |
| `clipper2` | 0.6.0 | 2026-05-06 | MIT/Apache | f64 API + scaling, C++ FFI | booleans, offsetting |
| `clipper2-rust` | 1.1.0 | 2026-07-23 | BSL-1.0 | f64 | pure-Rust Clipper2, low adoption |
| `cavalier_contours` | 0.9.0 | 2026-08-20 | MIT/Apache | f64 (arcs) | polyline offsetting |
| `polygon-offsetting` | 0.1.9 | 2024-06-15 | MIT/Apache | f64 | ~effectively abandoned (37 recent dl) |
| `robust` | 1.2.0 | 2025-05-10 | MIT/Apache | f64 only | Shewchuk exact-sign predicates |
| `geometry-predicates` | 0.3.0 | 2021-02-10 | MIT | f64 | same, unmaintained |
| `rstar` | 0.13.0 | 2026-05-24 | MIT/Apache | ints via `RTreeNum` | R*-tree, AABB envelopes only |
| `spade` | 2.15.1 | 2026-03-24 | MIT/Apache | f64 | CDT |
| `parry2d` / `-f64` | 0.30.2 | 2026-08-08 | Apache-2.0 | f32 / f64 | convex collision (GJK) |
| `earcutr` | 0.5.0 | 2025-05-29 | ISC | f64 | ear-clipping triangulation |
| `kurbo`, `flo_curves` | 0.13 / 0.8 | 2026-05 / 08 | MIT/Apache | f64 Béziers | not applicable |
| `euclid`, `glam` | 0.22 / 0.33 | 2026-03 / 08 | MIT/Apache | f32/f64 | vector types |
| `num-bigint` | **0.5.1** | 2026-07-05 | MIT/Apache | bigint | in use (workspace pins **0.4**) |
| `dashu` | 0.6.0 | 2026-08-09 | MIT/Apache | bigint/rational | faster small-value bigint |
| `malachite` | 0.11.0 | 2026-08-28 | **LGPL-3.0-only** | bigint/rational | fastest; copyleft |
| `ibig` | 0.3.6 | **2022-09-17** | MIT/Apache | bigint | unmaintained |
| `rug` | 1.30.0 | — | LGPL-3.0+ | GMP FFI | C dep, cross-compile pain |
| `num-rational` | 0.4.2 | 2025-01-25 | MIT/Apache | `Ratio<BigInt>` | normalises via gcd — wrong shape |

## 2. Area-by-area

### (i) Exact predicates & rational points — **No fit for code; Good fit as a test oracle**
`robust::orient2d` takes `Coord<f64>`; `i32` coordinates are exactly representable in `f64`, so its
sign is exact for our inputs. But `Line::side_of` already computes an exact `i64` determinant, and
the rational branch needs `BigInt` — `robust` has no integer or generic API. Replacing nothing.
Its real value is as an **independent oracle in property tests** for `side_of`,
`IntVector::determinant` sign, and `Simplex` orientation checks. `geometry-predicates` is the same
idea, last released 2021 — prefer `robust`.
`num-rational` is a poor match for `RationalPoint`: it normalises by gcd, while the port must keep
`(is_x, is_y, det)` unreduced with Java's exact `z == 1` fallback (`line.rs:265`) and quirk #4's
asymmetric `Point::from_big` reduction. Normalising changes observable state.

### (ii) Big-int fallback — **Partial (backend swap), plus one non-crate win**
The `BigInt` path is a thin, well-isolated layer (`bigint_aux.rs`, `rational_point.rs`,
`rational_vector.rs`, `bigint_direction.rs`). Swapping `num-bigint` → `dashu` (MIT/Apache, active,
faster on small magnitudes) is mechanically feasible, but three semantics must be re-verified or
parity breaks silently: `mod_floor` vs Java `BigInteger.mod` (non-negative remainder),
truncating division sign, and `to_f64` rounding (`big_to_f64` gates the `CRIT_INT` check in
`Line::intersection`). `malachite` is fastest but LGPL-3.0-only — compatible with the project's
GPL-3.0-or-later, yet it makes the geometry crate copyleft-encumbered for any future reuse.
`ibig` is unmaintained (2022). `rug` adds a GMP C dependency to a CLI meant to ship easily.

**The bigger win is not a crate.** `docs/java-quirks.md` already lists it as a candidate: in
`Line::intersection` the general case is provably `i128`-safe for `i32` inputs —
`|determinant| ≤ 2^63`, `|det · delta| ≤ 2^95`, `|is_x| ≤ 2^96 ≪ 2^127` — and `i128` integer
division/`rem_euclid` give **bit-identical** results to `BigInt`, so this is a pure speed change
with zero parity risk (the `Point::Rational` fallback still needs `BigInt` for the unreduced case).
Also worth noting the workspace pins `num-bigint = "0.4"` while 0.5.1 is current.

### (iii) Boxes / octagons / convex tiles — **No fit**
Nothing in the ecosystem models a convex tile as an *intersection of oriented `Line`s* that retains
line identity, nor an 8-parameter 45°-bounded octagon with Java's `normalize`, `cutout_from_box`
(≈250 LOC of case analysis) and `compare_octagon` edge-index protocol. `parry2d`'s convex shapes are
f32 vertex lists with GJK queries; intersection returns contact manifolds, not tile decompositions.
`geo`'s `Rect` has no octagon analogue. `Simplex::cutout_from` output order is search-tree input.

### (iv) `split_to_convex` — **No fit (pre-parity); Only post-parity at best**
`i_triangle` (convex decomposition), `earcutr`, `spade` CDT all produce valid decompositions and
none produces *Java's* decomposition. The port reproduces the LCG-driven division-point search;
different pieces → different search-tree contents → different maze expansion order → different
routes and different DRC/metric outputs. Even post-parity this is a routing-behaviour change, not a
refactor. Its legitimate use is as a **validator**: cross-check that the port's pieces are convex,
pairwise interior-disjoint, and sum to the input area.

### (v) `Polyline::offset_shapes` — **No fit**
`cavalier_contours`, `clipper2`'s `InflatePaths`, `polygon-offsetting` all produce *one* offset
outline, in f64, with joins (round/miter/square). Java produces `line_count - 2` **overlapping
convex tiles**, one per interior line, with a look-ahead "cut off outstanding corners" loop keyed on
`intersection_approx` and a `2·halfWidth²` distance test (`polyline.rs:520-565`). Different data
shape, different count, float-heuristic-dependent. Not substitutable.

### (vi) Polygon booleans for `ConductionArea` — **Good fit: `i_overlay`**
This is the one place §6 sanctions a crate, because Java itself uses `java.awt.geom.Area` and
parity is metric (area, point containment). `i_overlay` is the standout: **native `i32`/`i64`
integer API** (so results come back as exact `IntPoint` corners with no rounding step),
union/intersection/difference/xor, holes, self-intersections, even-odd/non-zero/positive/negative
fill rules, MIT/Apache, ~3.5M recent downloads, released 2026-08-16. Alternatives are all f64 at the
API boundary: `geo`'s `BooleanOps`, `geo-clipper` (ISC, Clipper1 FFI, last release 2025-02),
`clipper2` (C++ FFI), `clipper2-rust` (BSL-1.0, immature). **Caveat that keeps this off the
"free" list:** a `ConductionArea` boundary flows into `split_to_convex` → search tree. Different
vertex counts from `awt.geom.Area` therefore perturb routing even though the *region* matches.
Budget a metric-tolerance discussion, and pick the fill rule to match `Area`'s winding behaviour.

### (vii) Spatial index for Plan 2 — **No fit for the 45° regime; Partial elsewhere**
`rstar` does support integer scalars via `RTreeNum`, so `IntBox` keys are expressible. Two blockers:
(a) `RTreeObject::envelope` is an **AABB**; Java's `ShapeSearchTree` in the 45° regime keys entries
on `IntOctagon`, which an AABB cannot express — using bounding boxes instead changes which
candidates a query returns; (b) `rstar` uses the **R\* insertion strategy** (forced reinsertion,
overlap-minimising subtree choice) while Java's `MinAreaTree` is min-*area-increase* choose-subtree
with its own split. Different tree shape → different overlap-query result order → different maze
expansion order → different route. Port `MinAreaTree`. `rstar` is reasonable for
**order-insensitive auxiliary indexes** (e.g. DRC candidate-pair generation, where results are
sorted before reporting) and as a post-parity performance experiment.

### (viii) Small conveniences — **No fit / already covered**
`euclid`, `glam`, `nalgebra` are f32/f64 and generic-typed; `IntVector`/`IntDirection` carry Java
semantics (`turn_45_degree`'s negative-modulo bug #2, the non-antisymmetric `compare_to` #1) that a
generic vector type would erase. `i_float`'s `IntPoint` collides with ours for the same reason.
`num-traits`/`num-integer` are already dependencies and pull their weight.

## 3. What I'd actually consider, ordered by benefit ÷ risk

**Now — no parity impact**
1. **`robust` as a dev-dependency oracle** for `side_of` / determinant-sign property tests. Zero
   production code touched; catches a whole bug class.
2. **`i128` fast path in `Line::intersection`** (and `perpendicular_projection`). Not a crate;
   provably bit-identical; the largest single speed win available in the hot path.
3. **`i_overlay` as a dev-dependency validator** for `split_to_convex`: pieces convex, pairwise
   interior-disjoint, union area equals input. Also validates `Simplex::cutout_from` coverage.
4. **Bump `num-bigint` 0.4 → 0.5.1** (with the `mod`/`to_f64` semantics re-checked by test).

**When Plan 2 reaches `ConductionArea`**
5. **`i_overlay` in production** for copper-pour booleans — the sanctioned §6 slot. Prefer it over
   `geo`/Clipper wrappers specifically because it is integer-native. Log the vertex-count →
   `split_to_convex` → search-tree coupling as a metric-parity risk before wiring it in.

**Post-parity only, and each is a behaviour change, not a refactor**
6. `dashu` (or `malachite`, licence permitting) as the bigint backend, once a differential test
   against `num-bigint` over the fixture corpus is green.
7. `rstar` for auxiliary, order-insensitive indexes; never as the router's `ShapeSearchTree`.

**Explicit "no good fit"**: convex tiles (`Simplex`/`IntOctagon`) and their intersection/cutout,
`split_to_convex`, `offset_shapes`, rational points, and the router's search tree. In each case the
port's value *is* the specific output ordering and the reproduced Java defects; a crate that gets
the geometry right and the ordering different is strictly worse than the hand port.
